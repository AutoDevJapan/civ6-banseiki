//! The original sengoku_cascade_audit database loader, ported without running
//! Python. CI uses a compressed, pre-MOD snapshot of the official GS database;
//! every test then applies the current worktree's actual Sengoku SQL.

use crate::{read_repo, repo_root};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use rusqlite::{functions::FunctionFlags, types::ValueRef, Connection};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;
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

pub fn official_db() -> Connection {
    let path = repo_root().join("tools/civ6-lua-tests-rs/fixtures/official/pre_sengoku.sql.gz");
    read_snapshot(&path)
}

fn read_snapshot(path: &Path) -> Connection {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut sql = String::new();
    GzDecoder::new(bytes.as_slice())
        .read_to_string(&mut sql)
        .unwrap();
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(&sql).unwrap();
    register_hash(&db);
    db
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
