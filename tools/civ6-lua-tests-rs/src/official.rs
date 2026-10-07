//! The original sengoku_cascade_audit database loader, ported without running
//! Python. CI uses a compressed, pre-MOD snapshot of the official GS database;
//! every test then applies the current worktree's actual Sengoku SQL.

use crate::{read_repo, repo_root};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use rusqlite::{functions::FunctionFlags, types::ValueRef, Connection, MAIN_DB};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub fn register_hash(db: &Connection) {
    let has_types: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='Types')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let mut assigned = HashMap::<String, i64>::new();
    if has_types {
        let mut statement = db.prepare("SELECT Type, Hash FROM Types").unwrap();
        for pair in statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
        {
            let (key, value) = pair.unwrap();
            assigned.insert(key, value);
        }
    }
    let next = assigned.values().copied().max().unwrap_or(0) + 1;
    let values = Arc::new(Mutex::new((assigned, next)));
    db.create_scalar_function("Make_Hash", 1, FunctionFlags::SQLITE_UTF8, move |ctx| {
        let name: String = ctx.get(0)?;
        let mut values = values.lock().unwrap();
        if let Some(&value) = values.0.get(&name) {
            return Ok(value);
        }
        let value = values.1;
        values.1 += 1;
        values.0.insert(name, value);
        Ok(value)
    })
    .unwrap();
}

/// Each call returns a fresh private in-memory database. Loading the SQL dump
/// is expensive and nextest runs every test in its own process, so the loaded
/// pre-MOD image is cached under the target dir, keyed by the dump's bytes.
pub fn official_db() -> Connection {
    let path = repo_root().join("tools/civ6-lua-tests-rs/fixtures/official/pre_sengoku.sql.gz");
    let target = target_for_executable(&std::env::current_exe().expect("locate test executable"));
    cached_snapshot(&path, &target.join("civ6-official-cache")).0
}

// Cargo runs test executables in <target>/<profile>/deps. Deriving the root
// from that path also respects config files, --target-dir, and relative env
// paths without interpreting them again from the test crate's working dir.
fn target_for_executable(executable: &Path) -> PathBuf {
    let parent = executable.parent().expect("executable has a directory");
    let profile = if parent.file_name().is_some_and(|name| name == "deps") {
        parent.parent().expect("deps has a profile directory")
    } else {
        parent
    };
    profile
        .parent()
        .expect("profile has a target directory")
        .to_owned()
}

fn read_snapshot(path: &Path) -> Connection {
    let db = load_sql(&read_source(path));
    register_hash(&db);
    db
}

fn read_source(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn load_sql(gz: &[u8]) -> Connection {
    let mut sql = String::new();
    GzDecoder::new(gz).read_to_string(&mut sql).unwrap();
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(&sql).unwrap();
    db
}

fn cache_path(dir: &Path, gz: &[u8]) -> PathBuf {
    let mut hash = Sha256::new();
    for part in [
        &b"rusqlite-0.37 format-v2-sha256"[..],
        rusqlite::version().as_bytes(),
        gz,
    ] {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    dir.join(format!("pre_sengoku-{hex}.sqlite-cache"))
}

// Publish the checksum and image in one file so readers cannot observe a
// checksum from a different build. SQLite's structural checks alone do not
// detect changed cell values in an otherwise valid database.
fn load_cached_image(bytes: &[u8]) -> Result<Connection, String> {
    let (expected, image) = bytes
        .split_at_checked(32)
        .ok_or("cached image has no SHA-256 header")?;
    if &Sha256::digest(image)[..] != expected {
        return Err("cached image checksum mismatch".into());
    }
    load_image(image)
}

fn load_image(image: &[u8]) -> Result<Connection, String> {
    let mut db = Connection::open_in_memory().map_err(|e| e.to_string())?;
    db.deserialize_read_exact(MAIN_DB, image, image.len(), false)
        .map_err(|e| e.to_string())?;
    // Connection pragmas are not serialized; the SQL dump sets this first.
    db.execute_batch("PRAGMA foreign_keys=OFF;")
        .map_err(|e| e.to_string())?;
    let check: String = db
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if check != "ok" {
        return Err(check);
    }
    Ok(db)
}

/// Returns the database and whether this call built the cached image.
fn cached_snapshot(source: &Path, dir: &Path) -> (Connection, bool) {
    let gz = read_source(source);
    let path = cache_path(dir, &gz);
    let loaded = || {
        std::fs::read(&path)
            .ok()
            .and_then(|b| load_cached_image(&b).ok())
    };
    let (db, built) = match loaded() {
        Some(db) => (db, false),
        None => {
            std::fs::create_dir_all(dir).unwrap();
            let lock = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(path.with_extension("lock"))
                .unwrap();
            lock.lock().unwrap();
            // Another process may have published the image while we waited.
            match loaded() {
                Some(db) => (db, false),
                None => (build_image(&gz, &path), true),
            }
        }
    };
    register_hash(&db);
    (db, built)
}

/// Caller holds the cache lock. Replaces only a missing or malformed image.
fn build_image(gz: &[u8], path: &Path) -> Connection {
    let source = load_sql(gz);
    let foreign_keys: bool = source
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert!(
        !foreign_keys,
        "load_image assumes the dump turns foreign keys off"
    );
    let image = source.serialize(MAIN_DB).unwrap();
    let pending = path.with_extension(format!("pending-{}", std::process::id()));
    let publish = || -> std::io::Result<()> {
        // Under the lock, a pending file with our pid is left over from a crash.
        let _ = std::fs::remove_file(&pending);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)?;
        file.write_all(&Sha256::digest(&image[..]))?;
        file.write_all(&image[..])?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&pending, path)
    };
    if let Err(e) = publish() {
        let _ = std::fs::remove_file(&pending);
        eprintln!("official db cache not written to {}: {e}", path.display());
    }
    load_image(&image[..]).unwrap()
}

pub fn run_sengoku(db: &Connection) {
    db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    let source = read_repo("poc/JAC_PoC_SengokuTrees/JAC_PoC_SengokuTrees.modinfo");
    let doc = roxmltree::Document::parse(&source).unwrap();
    let actions = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("InGameActions"))
        .unwrap();
    for action in actions
        .children()
        .filter(|n| n.has_tag_name("UpdateDatabase"))
    {
        for file in action.children().filter(|n| n.has_tag_name("File")) {
            let path = file.text().unwrap();
            if !path.ends_with(".sql") {
                continue;
            }
            let sql = read_repo(format!("poc/JAC_PoC_SengokuTrees/{path}"));
            db.execute_batch(&format!(
                "BEGIN; PRAGMA defer_foreign_keys=ON;\n{sql}\nCOMMIT;"
            ))
            .unwrap_or_else(|e| panic!("Sengoku {path}: {e}"));
        }
    }
}

pub fn sengoku_db() -> Connection {
    let db = official_db();
    run_sengoku(&db);
    db
}

fn ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
fn text_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
fn load_xml(db: &Connection, path: &Path, tables: &HashSet<String>) {
    let source = std::fs::read_to_string(path).unwrap();
    let tables = tables.iter().cloned().collect();
    civ6_data_rs::cascade::load_xml(db, source.trim_start_matches('\u{feff}'), &tables)
        .unwrap_or_else(|error| panic!("{}: {error:#}", path.display()));
}

/// Read installed assets once to refresh the CI fixture. Does not start Civ6.
pub fn installed_official_db(game: &Path) -> Connection {
    let base = game.join("Base/Assets/Gameplay/Data");
    let xp2 = game.join("DLC/Expansion2");
    let db = Connection::open_in_memory().unwrap();
    // Python sqlite3 starts with foreign keys off; bundled rusqlite may enable
    // them by default. Official XML is loaded before its references exist.
    db.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
    register_hash(&db);
    for file in ["Schema/01_GameplaySchema.sql", "Schema/02_AddTriggers.sql"] {
        let source = std::fs::read_to_string(base.join(file)).unwrap();
        db.execute_batch(source.trim_start_matches('\u{feff}'))
            .unwrap();
    }
    let meta = std::fs::read_to_string(xp2.join("Expansion2.modinfo")).unwrap();
    let doc = roxmltree::Document::parse(meta.trim_start_matches('\u{feff}')).unwrap();
    let actions = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("InGameActions"))
        .unwrap();
    let mut files = Vec::new();
    for component in [
        "Expansion2Core",
        "Expansion2CoreContent",
        "Expansion2MajorContent",
    ] {
        let action = actions
            .children()
            .find(|n| n.has_tag_name("UpdateDatabase") && n.attribute("id") == Some(component))
            .unwrap();
        let mut entries: Vec<_> = action
            .children()
            .filter(|n| n.has_tag_name("File"))
            .collect();
        entries.sort_by_key(|n| {
            -n.attribute("Priority")
                .unwrap_or("0")
                .parse::<i64>()
                .unwrap()
        });
        files.extend(entries.into_iter().map(|n| xp2.join(n.text().unwrap())));
    }
    let source = std::fs::read_to_string(&files[0]).unwrap();
    db.execute_batch(source.trim_start_matches('\u{feff}'))
        .unwrap();
    let tables: HashSet<String> = db
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let mut base_files: Vec<_> = std::fs::read_dir(&base)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "xml"))
        .collect();
    base_files.sort();
    for path in base_files.into_iter().chain(files.into_iter().skip(1)) {
        load_xml(&db, &path, &tables);
    }
    db
}

fn literal(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => "NULL".into(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => {
            assert!(v.is_finite());
            format!("{v:?}")
        }
        ValueRef::Text(v) => text_literal(std::str::from_utf8(v).unwrap()),
        ValueRef::Blob(v) => format!(
            "X'{}'",
            v.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ),
    }
}

fn dump_sql(db: &Connection) -> String {
    let mut sql = String::from("-- Official GS pre-MOD data, captured by the Rust asset loader.\nPRAGMA foreign_keys=OFF;\nBEGIN;\n");
    let mut statement = db.prepare("SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap();
    let tables: Vec<(String, String)> = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    for (_, ddl) in &tables {
        sql.push_str(ddl);
        sql.push_str(";\n");
    }
    for (name, _) in &tables {
        let mut rows = db
            .prepare(&format!(
                "SELECT rowid, * FROM {} ORDER BY rowid",
                ident(name)
            ))
            .unwrap();
        let columns: Vec<_> = rows.column_names().iter().map(|s| ident(s)).collect();
        let count = rows.column_count();
        let prefix = format!(
            "INSERT INTO {} ({}) VALUES (",
            ident(name),
            columns.join(",")
        );
        let mut query = rows.query([]).unwrap();
        while let Some(row) = query.next().unwrap() {
            sql.push_str(&prefix);
            for i in 0..count {
                if i > 0 {
                    sql.push(',');
                }
                sql.push_str(&literal(row.get_ref(i).unwrap()));
            }
            sql.push_str(");\n");
        }
    }
    let mut statement = db.prepare("SELECT sql FROM sqlite_master WHERE type IN ('index','trigger','view') AND sql IS NOT NULL ORDER BY type,name").unwrap();
    for ddl in statement.query_map([], |r| r.get::<_, String>(0)).unwrap() {
        sql.push_str(&ddl.unwrap());
        sql.push_str(";\n");
    }
    sql.push_str("COMMIT;\n");
    sql
}

pub fn capture_snapshot(db: &Connection, output: &Path) {
    let sql = dump_sql(db);
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    // Validate a new snapshot before replacing the canonical fixture, so an
    // interrupted refresh or changed asset load cannot corrupt a working one.
    let pending = output.with_extension(format!("gz.pending-{}", std::process::id()));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
        .unwrap();
    let mut gzip = GzEncoder::new(file, Compression::best());
    gzip.write_all(sql.as_bytes()).unwrap();
    gzip.finish().unwrap();
    let restored = read_snapshot(&pending);
    assert_eq!(
        dump_sql(&restored),
        sql,
        "snapshot changed official schema or data"
    );
    run_sengoku(&restored);
    std::fs::rename(&pending, output).unwrap();
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    const SQL: &str = "PRAGMA foreign_keys=OFF;
        CREATE TABLE Types(Type TEXT PRIMARY KEY, Hash INTEGER);
        CREATE TABLE Parent(Id INTEGER PRIMARY KEY);
        CREATE TABLE Child(Id INTEGER REFERENCES Parent(Id), R REAL, B BLOB);
        INSERT INTO Types VALUES('A', 7);
        INSERT INTO Child VALUES(99, 1.5, X'00ff'), (NULL, NULL, NULL);
        CREATE INDEX ChildR ON Child(R);
        CREATE VIEW TypeView AS SELECT * FROM Types;
        CREATE TRIGGER ParentAdded AFTER INSERT ON Parent BEGIN SELECT 1; END;";

    struct Temp(PathBuf);
    impl Temp {
        fn new(name: &str) -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "civ6-official-cache-{name}-{}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir(&dir).unwrap();
            Temp(dir)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Writes a dump in the fixture format and returns the dumped SQL.
    fn fixture(path: &Path, sql: &str) -> String {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(sql).unwrap();
        let dump = dump_sql(&db);
        let mut gz = GzEncoder::new(std::fs::File::create(path).unwrap(), Compression::fast());
        gz.write_all(dump.as_bytes()).unwrap();
        gz.finish().unwrap();
        dump
    }

    fn count(db: &Connection, table: &str) -> i64 {
        db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn cache_root_follows_actual_cargo_output_directory() {
        let temp = Temp::new("target-dir");
        let target = temp.0.join("configured-output");
        for profile in ["debug", "release", "custom-profile"] {
            assert_eq!(
                target_for_executable(&target.join(profile).join("deps").join("test.exe")),
                target
            );
            assert_eq!(
                target_for_executable(&target.join(profile).join("capture.exe")),
                target
            );
        }
        let cross_target = target.join("x86_64-pc-windows-msvc");
        assert_eq!(
            target_for_executable(&cross_target.join("debug/deps/test.exe")),
            cross_target
        );
    }

    #[test]
    fn cached_image_matches_sql_loader_and_is_private() {
        let temp = Temp::new("roundtrip");
        let source = temp.0.join("a.sql.gz");
        let dump = fixture(&source, SQL);
        let (first, built) = cached_snapshot(&source, &temp.0);
        assert!(built);
        let image = std::fs::read(cache_path(&temp.0, &read_source(&source))).unwrap();
        let (second, built) = cached_snapshot(&source, &temp.0);
        assert!(!built);
        assert_eq!(dump_sql(&read_snapshot(&source)), dump);
        assert_eq!(dump_sql(&first), dump);
        assert_eq!(dump_sql(&second), dump);
        let fk: i64 = second
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 0);
        let hashes: (i64, i64) = second
            .query_row("SELECT Make_Hash('A'), Make_Hash('B')", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(hashes, (7, 8));

        first
            .execute("INSERT INTO Types VALUES('Z', 1)", [])
            .unwrap();
        assert_eq!(count(&first, "Types"), 2);
        assert_eq!(count(&second, "Types"), 1);
        assert_eq!(count(&cached_snapshot(&source, &temp.0).0, "Types"), 1);
        drop(first);
        let after = std::fs::read(cache_path(&temp.0, &read_source(&source))).unwrap();
        assert!(image == after, "cached image changed after use");
    }

    #[test]
    fn changed_source_uses_new_key_and_data() {
        let temp = Temp::new("source-change");
        let source = temp.0.join("a.sql.gz");
        fixture(&source, SQL);
        let old_key = cache_path(&temp.0, &read_source(&source));
        cached_snapshot(&source, &temp.0);
        let dump = fixture(&source, &format!("{SQL} INSERT INTO Types VALUES('C', 9);"));
        assert_ne!(cache_path(&temp.0, &read_source(&source)), old_key);
        let (db, built) = cached_snapshot(&source, &temp.0);
        assert!(built);
        assert_eq!(dump_sql(&db), dump);
        assert_eq!(count(&db, "Types"), 2);
    }

    #[test]
    fn malformed_image_is_rebuilt() {
        let temp = Temp::new("malformed");
        let source = temp.0.join("a.sql.gz");
        let dump = fixture(&source, SQL);
        let key = cache_path(&temp.0, &read_source(&source));
        cached_snapshot(&source, &temp.0);
        let good = std::fs::read(&key).unwrap();
        for bad in [&b""[..], b"not sqlite", &good[..good.len() / 2]] {
            std::fs::write(&key, bad).unwrap();
            let (db, built) = cached_snapshot(&source, &temp.0);
            assert!(built, "{} byte image was accepted", bad.len());
            assert_eq!(dump_sql(&db), dump);
            assert!(std::fs::read(&key).unwrap() == good);
        }
    }

    #[test]
    fn structurally_valid_content_corruption_is_rebuilt() {
        let temp = Temp::new("content-corruption");
        let source = temp.0.join("a.sql.gz");
        let dump = fixture(&source, SQL);
        let key = cache_path(&temp.0, &read_source(&source));
        cached_snapshot(&source, &temp.0);
        let good = std::fs::read(&key).unwrap();
        let altered = load_cached_image(&good).unwrap();
        altered
            .execute("UPDATE Types SET Hash = 999 WHERE Type = 'A'", [])
            .unwrap();
        let altered_image = altered.serialize(MAIN_DB).unwrap();
        assert!(
            load_image(&altered_image).is_ok(),
            "SQLite structural check accepts changed data"
        );
        let mut bad = good[..32].to_vec();
        bad.extend_from_slice(&altered_image);
        std::fs::write(&key, bad).unwrap();

        let (db, built) = cached_snapshot(&source, &temp.0);
        assert!(
            built,
            "content corruption must regenerate from the original SQL"
        );
        assert_eq!(dump_sql(&db), dump);
        assert_eq!(std::fs::read(&key).unwrap(), good);
        assert!(
            !cached_snapshot(&source, &temp.0).1,
            "repaired image must be reusable"
        );
    }

    #[test]
    fn concurrent_callers_build_once_without_partial_files() {
        let temp = Temp::new("concurrent");
        let source = temp.0.join("a.sql.gz");
        let dump = fixture(&source, SQL);
        let builds = std::thread::scope(|s| {
            let workers: Vec<_> = (0..8)
                .map(|_| {
                    s.spawn(|| {
                        let (db, built) = cached_snapshot(&source, &temp.0);
                        assert_eq!(dump_sql(&db), dump);
                        built
                    })
                })
                .collect();
            workers
                .into_iter()
                .map(|w| w.join().unwrap())
                .filter(|&built| built)
                .count()
        });
        assert_eq!(builds, 1);
        let mut names: Vec<_> = std::fs::read_dir(&temp.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        let key = cache_path(&temp.0, &read_source(&source));
        let image = key.file_name().unwrap().to_str().unwrap();
        let lock = key.with_extension("lock");
        let lock = lock.file_name().unwrap().to_str().unwrap();
        assert_eq!(names, ["a.sql.gz", lock, image]);
    }
}
