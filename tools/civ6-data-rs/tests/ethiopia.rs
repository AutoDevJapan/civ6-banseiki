use civ6_data_rs::{ethiopia, Repository};
use roxmltree::{Document, Node};
use rusqlite::{params_from_iter, types::Value as Sql, Connection};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

type DatabaseSnapshot = BTreeMap<String, Vec<Vec<Sql>>>;
type ForeignKeyFailures = BTreeSet<(String, Option<i64>, String, i64)>;

fn prepared() -> String {
    let repo = Repository::checked_in();
    ethiopia::modinfo(
        &repo
            .read(format!("{}/{}", ethiopia::MOD_DIR, ethiopia::MODINFO))
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn prepared_mod_is_limited_to_enabled_ethiopia_and_banseiki_xp2() {
    let source = prepared();
    let doc = Document::parse(&source).unwrap();
    let criterion = doc
        .descendants()
        .find(|n| n.has_tag_name("Criteria") && n.attribute("id") == Some("JAC_Banseiki_Ethiopia"))
        .unwrap();
    assert_eq!(criterion.attribute("any"), None);
    let terms: BTreeMap<_, _> = criterion
        .children()
        .filter(Node::is_element)
        .map(|n| (n.tag_name().name(), n.text().unwrap()))
        .collect();
    assert_eq!(
        terms,
        BTreeMap::from([
            ("RuleSetInUse", "RULESET_JAC_BANSEIKI"),
            ("GameCoreInUse", "Expansion2"),
            ("ModIsEnabled", ethiopia::DLC_ID),
        ])
    );
    // No LeaderPlayable gate: non-selected Menelik must not disable shared districts.
    for ruleset in [
        "RULESET_JAC_BANSEIKI",
        "RULESET_STANDARD",
        "RULESET_EXPANSION_2",
    ] {
        for core in ["Expansion2", "Base"] {
            for enabled in [true, false] {
                let applies = terms.iter().all(|(key, value)| match *key {
                    "RuleSetInUse" => *value == ruleset,
                    "GameCoreInUse" => *value == core,
                    "ModIsEnabled" => enabled,
                    _ => panic!("unexpected criterion"),
                });
                assert_eq!(
                    applies,
                    ruleset == "RULESET_JAC_BANSEIKI" && core == "Expansion2" && enabled
                );
            }
        }
    }
    let action = doc
        .descendants()
        .find(|n| {
            n.has_tag_name("UpdateDatabase") && n.attribute("id") == Some("JAC_Banseiki_Ethiopia")
        })
        .unwrap();
    assert_eq!(action.attribute("criteria"), Some("JAC_Banseiki_Ethiopia"));
    assert_eq!(
        action
            .descendants()
            .find(|n| n.has_tag_name("LoadOrder"))
            .unwrap()
            .text(),
        Some("1")
    );
    assert_eq!(
        action
            .children()
            .find(|n| n.has_tag_name("File"))
            .unwrap()
            .text(),
        Some(ethiopia::SQL_FILE)
    );
    // Original optional DLC reference never becomes a dependency.
    assert!(!doc
        .descendants()
        .filter(
            |n| n.has_tag_name("Mod") && n.parent().is_some_and(|p| p.has_tag_name("Dependencies"))
        )
        .any(|n| n.attribute("id") == Some(ethiopia::DLC_ID)));
    assert!(ethiopia::modinfo(&source).is_err());
}

#[test]
fn xml_conversion_preserves_defaults_quotes_booleans_and_duplicate_safety() {
    let db = Connection::open_in_memory().unwrap();
    let schema = "CREATE TABLE Parent(Id TEXT PRIMARY KEY, Flag INTEGER DEFAULT 7, Note TEXT); CREATE TABLE Loose(Id TEXT, Flag INTEGER); CREATE TABLE Child(Id TEXT PRIMARY KEY, Parent TEXT REFERENCES Parent(Id)); PRAGMA foreign_keys=ON;";
    db.execute_batch(schema).unwrap();
    let source = r#"<GameInfo>
      <Parent><Row Id="a"><Flag>true</Flag><Note>original</Note></Row><Row Id="b"/><Row Id="quoted" Note="O'Brien &amp; friends"/></Parent>
      <Loose><Row Id="a" Flag="false"/></Loose>
      <Child><Row Id="c" Parent="a"/></Child>
      <Parent><Update><Where Id="a"/><Set Note="updated"/></Update><Delete Id="b"/></Parent>
    </GameInfo>"#;
    let sql = ethiopia::xml_sql(source).unwrap();
    db.execute_batch(&sql).unwrap();
    let first = snapshot(&db);
    let oracle = Connection::open_in_memory().unwrap();
    oracle.execute_batch(schema).unwrap();
    official_xml(&oracle, source);
    assert_eq!(first, snapshot(&oracle));
    assert_eq!(
        db.query_row(
            "SELECT Flag,Note FROM Parent WHERE Id='quoted'",
            [],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        )
        .unwrap(),
        (7, "O'Brien & friends".into())
    );
    db.execute_batch(&sql).unwrap();
    assert_eq!(snapshot(&db), first);
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM Loose", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT Flag FROM Parent WHERE Id='a'", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    db.execute("UPDATE Parent SET Flag=9 WHERE Id='a'", [])
        .unwrap();
    db.execute_batch(&sql).unwrap();
    assert_eq!(
        db.query_row("SELECT Flag FROM Parent WHERE Id='a'", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        9,
        "existing keyed rows are not replaced"
    );
    assert_no_new_foreign_keys(&db, &BTreeSet::new());
    for bad in [
        "<GameInfo><Parent><Replace Id='a'/></Parent></GameInfo>",
        "<GameInfo><Parent><Update><Set Id='a'/></Update></Parent></GameInfo>",
        "<GameInfo><Parent><Delete/></Parent></GameInfo>",
        "<GameInfo><Parent><Row Id='a'><Id>b</Id></Row></Parent></GameInfo>",
    ] {
        assert!(
            ethiopia::xml_sql(bad).is_err(),
            "unsupported data must fail: {bad}"
        );
    }
}

fn foreign_keys(db: &Connection) -> ForeignKeyFailures {
    // CollectionType/EffectType are registered by the game core, not source XML.
    // Keep ModifierType -> Types checked; independently compare every actual
    // DynamicModifiers field with the official XML oracle below.
    let engine_keys: BTreeSet<i64> = db
        .prepare("PRAGMA foreign_key_list(DynamicModifiers)")
        .unwrap()
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .filter(|(_, column)| column == "CollectionType" || column == "EffectType")
        .map(|(id, _)| id)
        .collect();
    db.prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .filter(|(table, _, _, id)| table != "DynamicModifiers" || !engine_keys.contains(id))
        .collect()
}

#[cfg(windows)]
fn gameplay_db(game: &std::path::Path) -> Connection {
    let db = civ6_data_rs::cascade::official_db().unwrap();
    // The legacy cascade loader ignores InsertOrIgnore, notably KIND_TRAIT.
    // Load official kind declarations rather than inventing DLC dependencies.
    for entry in std::fs::read_dir(game.join("Base/Assets/Gameplay/Data")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("xml") {
            continue;
        }
        let source = civ6_data_rs::read_utf8(&path).unwrap();
        let doc = Document::parse(&source).unwrap();
        for kind in doc
            .descendants()
            .filter(|n| n.parent().is_some_and(|p| p.has_tag_name("Kinds")))
        {
            if let Some(value) = kind.attribute("Kind") {
                db.execute("INSERT OR IGNORE INTO Kinds(Kind) VALUES (?1)", [value])
                    .unwrap();
            }
        }
    }
    // Match Modding.log: foreign keys enabled and initially deferred. Keep this
    // audit transaction open because intrinsic engine Types are unavailable.
    // Cascading deletes still run, unlike the legacy FK-disabled seed loader.
    db.execute_batch("PRAGMA foreign_keys=ON; BEGIN; PRAGMA defer_foreign_keys=ON;")
        .unwrap();
    db
}

fn assert_no_new_foreign_keys(db: &Connection, baseline: &ForeignKeyFailures) {
    let new: Vec<_> = foreign_keys(db).difference(baseline).cloned().collect();
    assert!(new.is_empty(), "new XML-defined FK failures: {new:?}");
}

fn snapshot(db: &Connection) -> DatabaseSnapshot {
    let tables: Vec<String> = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap()
        .query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    tables
        .into_iter()
        .map(|table| {
            let mut q = db
                .prepare(&format!("SELECT * FROM \"{table}\" ORDER BY rowid"))
                .unwrap();
            let width = q.column_count();
            let rows = q
                .query_map([], |r| {
                    (0..width)
                        .map(|i| r.get(i))
                        .collect::<rusqlite::Result<Vec<Sql>>>()
                })
                .unwrap()
                .map(Result::unwrap)
                .collect();
            (table, rows)
        })
        .collect()
}

/// An independent XML oracle: parameter binding, strict Row insertion and stock
/// Update/Delete operations. It does not call the compatibility SQL converter.
fn official_xml(db: &Connection, source: &str) {
    fn values(node: Node<'_, '_>) -> BTreeMap<String, Sql> {
        let scalar = |text: &str| match text.to_ascii_lowercase().as_str() {
            "true" => Sql::Integer(1),
            "false" => Sql::Integer(0),
            _ => Sql::Text(text.into()),
        };
        node.attributes()
            .map(|a| (a.name().to_owned(), scalar(a.value())))
            .chain(node.children().filter(Node::is_element).map(|n| {
                (
                    n.tag_name().name().to_owned(),
                    scalar(n.text().unwrap_or("")),
                )
            }))
            .collect()
    }
    fn wh(row: &BTreeMap<String, Sql>) -> String {
        row.keys()
            .map(|k| format!("\"{k}\"=?"))
            .collect::<Vec<_>>()
            .join(" AND ")
    }
    let doc = Document::parse(source.trim_start_matches('\u{feff}')).unwrap();
    for table in doc.root_element().children().filter(Node::is_element) {
        let table_name = table.tag_name().name();
        for op in table.children().filter(Node::is_element) {
            let (sql, args) = match op.tag_name().name() {
                "Row" => {
                    let row = values(op);
                    let columns = row
                        .keys()
                        .map(|k| format!("\"{k}\""))
                        .collect::<Vec<_>>()
                        .join(",");
                    (
                        format!(
                            "INSERT INTO \"{table_name}\" ({columns}) VALUES ({})",
                            vec!["?"; row.len()].join(",")
                        ),
                        row.into_values().collect::<Vec<_>>(),
                    )
                }
                "Delete" => {
                    let row = values(op);
                    (
                        format!("DELETE FROM \"{table_name}\" WHERE {}", wh(&row)),
                        row.into_values().collect(),
                    )
                }
                "Update" => {
                    let row = values(op.children().find(|n| n.has_tag_name("Where")).unwrap());
                    let set = values(op.children().find(|n| n.has_tag_name("Set")).unwrap());
                    let assignments = set
                        .keys()
                        .map(|k| format!("\"{k}\"=?"))
                        .collect::<Vec<_>>()
                        .join(",");
                    (
                        format!(
                            "UPDATE \"{table_name}\" SET {assignments} WHERE {}",
                            wh(&row)
                        ),
                        set.into_values().chain(row.into_values()).collect(),
                    )
                }
                other => panic!("unhandled official operation {other}"),
            };
            db.execute(&sql, params_from_iter(args))
                .unwrap_or_else(|e| panic!("{table_name}: {e}"));
        }
    }
}

/// On Windows the installed source is required, not silently skipped. The same
/// converter/oracle contracts above run in Linux CI without redistributing DLC.
#[cfg(windows)]
#[test]
fn installed_official_base_and_xp2_repair_missing_existing_and_repeated_loading() {
    use civ6_data_rs::read_utf8;
    use std::path::PathBuf;
    let game = std::env::var_os("CIV6_INSTALL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI",
            )
        });
    let manifest = read_utf8(&game.join("DLC/Ethiopia/Ethiopia.modinfo")).unwrap();
    let files = ethiopia::sources(&manifest).unwrap();
    assert_eq!(files[0], "Data/Ethiopia_RemoveData.xml");
    assert_eq!(files[8], "Data/Ethiopia_RemoveData_Expansion2.xml");
    let xml: Vec<_> = files
        .iter()
        .map(|f| read_utf8(&game.join("DLC/Ethiopia").join(f)).unwrap())
        .collect();
    let sql = ethiopia::gameplay_sql(&game).unwrap();
    let oracle = gameplay_db(&game);
    let baseline = foreign_keys(&oracle);
    for file in &xml {
        official_xml(&oracle, file);
    }
    assert_no_new_foreign_keys(&oracle, &baseline);
    let expected = snapshot(&oracle);
    for state in ["missing", "base", "base+xp2", "xp2-only"] {
        let db = gameplay_db(&game);
        match state {
            "base" => {
                for file in &xml[..8] {
                    official_xml(&db, file);
                }
            }
            "base+xp2" => {
                for file in &xml {
                    official_xml(&db, file);
                }
            }
            "xp2-only" => {
                for file in &xml[8..] {
                    official_xml(&db, file);
                }
                let added: Vec<_> = foreign_keys(&db).difference(&baseline).cloned().collect();
                assert_eq!(
                    added.len(),
                    42,
                    "reproduce random hotseat07 Gameplay FK failure"
                );
                assert!(added.iter().any(|v| v.0 == "DistrictModifiers"));
                assert!(added.iter().any(|v| v.0 == "Units_XP2"));
            }
            "missing" => {}
            _ => unreachable!(),
        }
        db.execute_batch(&sql).unwrap();
        assert_no_new_foreign_keys(&db, &baseline);
        // Rowids/hashes depend on insertion order; compare all actual data using
        // canonical sorted rows with auto-generated Hash columns excluded below.
        assert_same_data(&db, &expected, state);
        let once = snapshot(&db);
        db.execute_batch(&sql).unwrap();
        assert_eq!(snapshot(&db), once, "second execution: {state}");
        assert_no_new_foreign_keys(&db, &baseline);
        assert_eq!(
            db.query_row(
                "SELECT StrategicResource FROM Units WHERE UnitType='UNIT_ETHIOPIAN_OROMO_CAVALRY'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "RESOURCE_HORSES"
        );
        assert_eq!(db.query_row("SELECT ReplacesUnitType FROM UnitReplaces WHERE CivUniqueUnitType='UNIT_ETHIOPIAN_OROMO_CAVALRY'", [], |r| r.get::<_, String>(0)).unwrap(), "UNIT_COURSER");
    }
    let doc = Document::parse(&manifest).unwrap();
    let xp2 = doc
        .descendants()
        .find(|n| n.attribute("id") == Some("EthiopiaGameplay_XP2"))
        .unwrap();
    assert!(
        !xp2.descendants().any(|n| n.has_tag_name("LoadOrder")),
        "official XP2 has default order 0"
    );
    let broken = doc
        .descendants()
        .find(|n| n.has_tag_name("Criteria") && n.attribute("id") == Some("Ethiopia"))
        .unwrap();
    assert!(broken
        .descendants()
        .find(|n| n.has_tag_name("LeaderPlayable"))
        .unwrap()
        .text()
        .unwrap()
        .contains("LEADER_MENELIK Players:Expansion2_Players"));
}

#[cfg(windows)]
fn assert_same_data(db: &Connection, expected: &DatabaseSnapshot, state: &str) {
    let actual = snapshot(db);
    for (table, rows) in expected {
        let columns: Vec<String> = db
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let normalize = |rows: &[Vec<Sql>]| {
            let mut rows: Vec<String> = rows
                .iter()
                .map(|row| {
                    format!(
                        "{:?}",
                        row.iter()
                            .zip(&columns)
                            .filter(|(_, c)| *c != "Hash")
                            .map(|(v, _)| v)
                            .collect::<Vec<_>>()
                    )
                })
                .collect();
            rows.sort();
            rows
        };
        let actual_rows = normalize(&actual[table]);
        let expected_rows = normalize(rows);
        assert!(
            actual_rows == expected_rows,
            "{state}: {table}: {} actual / {} expected; first difference: {:?}",
            actual_rows.len(),
            expected_rows.len(),
            actual_rows.iter().zip(&expected_rows).find(|(a, b)| a != b)
        );
    }
}

#[cfg(windows)]
#[test]
fn installed_babylon_base_and_xp2_match_official_order_and_cross_dlc() {
    use civ6_data_rs::read_utf8;
    let game = std::env::var_os("CIV6_INSTALL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI",
            )
        });
    let dir = game.join("DLC/Babylon");
    let manifest = read_utf8(&dir.join("Babylon.modinfo")).unwrap();
    let paths = ethiopia::dlc_sources(&manifest, ethiopia::BABYLON).unwrap();
    assert_eq!(paths.len(), 9);
    assert_eq!(paths[0], "Data/Babylon_Civilizations.xml");
    assert_eq!(paths[8], "Data/Babylon_Expansion2.xml");
    assert!(paths.iter().all(|p| !p.contains("_MODE")));
    let xml: Vec<_> = paths
        .iter()
        .map(|p| read_utf8(&dir.join(p)).unwrap())
        .collect();
    let sql = ethiopia::dlc_gameplay_sql(&game, ethiopia::BABYLON).unwrap();
    let oracle = gameplay_db(&game);
    let baseline = foreign_keys(&oracle);
    for file in &xml {
        official_xml(&oracle, file);
    }
    assert_no_new_foreign_keys(&oracle, &baseline);
    let expected = snapshot(&oracle);
    for state in ["missing", "base", "base+xp2", "xp2-only"] {
        let db = gameplay_db(&game);
        let subset = match state {
            "base" => &xml[..8],
            "base+xp2" => &xml[..],
            "xp2-only" => &xml[8..],
            _ => &[],
        };
        for file in subset {
            official_xml(&db, file);
        }
        if state == "xp2-only" {
            let failures: Vec<_> = foreign_keys(&db).difference(&baseline).cloned().collect();
            assert!(!failures.is_empty());
            assert!(failures.iter().any(|f| f.0 == "TraitModifiers"));
            assert!(failures.iter().any(|f| f.0 == "NamedRiverCivilizations"));
        }
        db.execute_batch(&sql).unwrap();
        assert_no_new_foreign_keys(&db, &baseline);
        assert_same_data(&db, &expected, state);
        let once = snapshot(&db);
        db.execute_batch(&sql).unwrap();
        assert_eq!(snapshot(&db), once, "second execution: {state}");
        assert_no_new_foreign_keys(&db, &baseline);
        assert_eq!(
            db.query_row(
                "SELECT Description FROM Traits WHERE TraitType='TRAIT_LEADER_HAMMURABI'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "LOC_TRAIT_LEADER_HAMMURABI_XP1_DESCRIPTION"
        );
    }
    let doc = Document::parse(&manifest).unwrap();
    let xp2 = doc
        .descendants()
        .find(|n| n.attribute("id") == Some("BabylonGameplay_XP2"))
        .unwrap();
    assert!(!xp2.descendants().any(|n| n.has_tag_name("LoadOrder")));

    // The actual non-mode partner data supplies Observatory, never a made-up row.
    let maya_dir = game.join("DLC/GranColombia_Maya");
    let maya_manifest = read_utf8(&maya_dir.join("GranColombia_Maya.modinfo")).unwrap();
    let maya_doc = Document::parse(&maya_manifest).unwrap();
    let maya_action = maya_doc
        .descendants()
        .find(|n| n.attribute("id") == Some("GranColombiaMayaGameplay"))
        .unwrap();
    let maya_files: Vec<_> = maya_action
        .children()
        .filter(|n| n.has_tag_name("File"))
        .map(|n| read_utf8(&maya_dir.join(n.text().unwrap())).unwrap())
        .collect();
    let cross = read_utf8(&dir.join("Data/Babylon_GranColombia_Maya.xml")).unwrap();
    let cross_sql = ethiopia::xml_sql(&cross).unwrap();
    let cross_oracle = gameplay_db(&game);
    for file in &maya_files {
        official_xml(&cross_oracle, file);
    }
    for file in &xml {
        official_xml(&cross_oracle, file);
    }
    let cross_baseline = foreign_keys(&cross_oracle);
    official_xml(&cross_oracle, &cross);
    assert_no_new_foreign_keys(&cross_oracle, &cross_baseline);
    let cross_expected = snapshot(&cross_oracle);
    let db = gameplay_db(&game);
    for file in &maya_files {
        official_xml(&db, file);
    }
    db.execute_batch(&sql).unwrap();
    db.execute_batch(&cross_sql).unwrap();
    assert_same_data(
        &db,
        &cross_expected,
        "Maya before Babylon, cross after both",
    );
    let once = snapshot(&db);
    db.execute_batch(&sql).unwrap();
    db.execute_batch(&cross_sql).unwrap();
    assert_eq!(snapshot(&db), once);
}

struct PreparationFixture {
    // Standard tempfile fallback serves CI; the finite PC run supplies its recorded
    // worktree scratch root. No helper changes test ACLs or shared build caches.
    _temporary: Option<tempfile::TempDir>,
    root: PathBuf,
    repo: Repository,
    game: PathBuf,
}
impl PreparationFixture {
    fn new(label: &str) -> Self {
        let (temporary, root) = if let Some(root) = std::env::var_os("BANSEIKI_DLC_TEST_ROOT") {
            let path = PathBuf::from(root).join(format!("{label}-{}", std::process::id()));
            std::fs::create_dir(&path).unwrap();
            (None, path)
        } else {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().to_owned();
            (Some(temp), path)
        };
        let repo = Repository::new(root.join("repo"));
        let source = repo.path(ethiopia::MOD_DIR);
        std::fs::create_dir_all(source.join("Data")).unwrap();
        std::fs::write(
            source.join(ethiopia::MODINFO),
            Repository::checked_in()
                .read(format!("{}/{}", ethiopia::MOD_DIR, ethiopia::MODINFO))
                .unwrap(),
        )
        .unwrap();
        let original_source = Repository::checked_in().path(ethiopia::MOD_DIR);
        for entry in walkdir::WalkDir::new(&original_source) {
            let entry = entry.unwrap();
            if entry.file_type().is_file() {
                let dest = source.join(entry.path().strip_prefix(&original_source).unwrap());
                std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
                std::fs::copy(entry.path(), dest).unwrap();
            }
        }
        std::fs::write(source.join("Data/sentinel.sql"), "-- source version one\n").unwrap();
        let game = root.join("game");
        std::fs::create_dir_all(&game).unwrap();
        Self {
            _temporary: temporary,
            root,
            repo,
            game,
        }
    }
    fn install(&self, dlc: ethiopia::DlcDefinition) {
        let dir = self.game.join("DLC").join(dlc.name);
        std::fs::create_dir_all(dir.join("Data")).unwrap();
        let mut actions = String::new();
        for (id, count) in dlc.components {
            actions.push_str(&format!("<UpdateDatabase id='{id}'>"));
            for i in 0..count {
                let path = format!("Data/{id}_{i}.xml");
                std::fs::write(dir.join(&path), format!("<GameInfo><TestData><Row Id='{id}_{i}' Value='first'/></TestData></GameInfo>")).unwrap();
                actions.push_str(&format!("<File>{path}</File>"));
            }
            actions.push_str("</UpdateDatabase>");
        }
        let criterion = if dlc.name == "Babylon" {
            let path = "Data/cross.xml";
            std::fs::write(
                dir.join(path),
                "<GameInfo><TestData><Row Id='cross' Value='first'/></TestData></GameInfo>",
            )
            .unwrap();
            actions.push_str(&format!("<UpdateDatabase id='BabylonGameplay_GranColombia_Maya' criteria='Babylon_GranColombia_Maya'><File>{path}</File></UpdateDatabase>"));
            format!("<ActionCriteria><Criteria id='Babylon_GranColombia_Maya'><RuleSetInUse>RULESET_STANDARD,RULESET_EXPANSION_1,RULESET_EXPANSION_2</RuleSetInUse><ModInUse>{}</ModInUse></Criteria></ActionCriteria>", ethiopia::MAYA_ID)
        } else {
            String::new()
        };
        std::fs::write(
            dir.join(format!("{}.modinfo", dlc.name)),
            format!(
                "<Mod id='{}'>{criterion}<InGameActions>{actions}</InGameActions></Mod>",
                dlc.id
            ),
        )
        .unwrap();
    }
    fn prepare(&self, output: &Path, refresh: bool) -> anyhow::Result<()> {
        ethiopia::prepare_local_dlc_mod(&self.repo, &self.game, output, refresh)
    }
}

fn directory_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            (
                e.path().strip_prefix(root).unwrap().to_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}

#[test]
fn optional_dlc_manifests_keep_candidates_and_split_enabled_conditions() {
    for (case, installed) in [
        ("none", Vec::new()),
        ("ethiopia", vec![ethiopia::ETHIOPIA]),
        ("babylon", vec![ethiopia::BABYLON]),
        ("both", ethiopia::DLCS.to_vec()),
    ] {
        let fixture = PreparationFixture::new(case);
        for dlc in &installed {
            fixture.install(*dlc);
        }
        let output = fixture.root.join("output");
        fixture.prepare(&output, false).unwrap();
        let info = std::fs::read_to_string(output.join(ethiopia::MODINFO)).unwrap();
        let doc = Document::parse(&info).unwrap();
        let dependencies: Vec<_> = doc
            .descendants()
            .filter(|n| {
                n.has_tag_name("Mod") && n.parent().is_some_and(|p| p.has_tag_name("Dependencies"))
            })
            .filter_map(|n| n.attribute("id"))
            .collect();
        assert!(!dependencies.contains(&ethiopia::BABYLON.id));
        assert!(!dependencies.contains(&ethiopia::ETHIOPIA.id));
        assert!(dependencies.contains(&"4873eb62-8ccc-4574-b784-dda455e74e68"));
        assert!(
            !doc.descendants().any(|n| n.has_tag_name("LeaderPlayable")),
            "no candidate filtering"
        );
        let source = fixture
            .repo
            .read(format!("{}/{}", ethiopia::MOD_DIR, ethiopia::MODINFO))
            .unwrap();
        let original = Document::parse(&source).unwrap();
        let frontend = doc
            .descendants()
            .find(|n| n.has_tag_name("FrontEndActions"))
            .unwrap();
        let original_frontend = original
            .descendants()
            .find(|n| n.has_tag_name("FrontEndActions"))
            .unwrap();
        assert_eq!(&info[frontend.range()], &source[original_frontend.range()]);
        for dlc in ethiopia::DLCS {
            let id = format!("JAC_Banseiki_{}", dlc.name);
            let criterion = doc
                .descendants()
                .find(|n| n.has_tag_name("Criteria") && n.attribute("id") == Some(id.as_str()));
            let present = installed.iter().any(|i| i.name == dlc.name);
            assert_eq!(criterion.is_some(), present);
            if let Some(criterion) = criterion {
                let terms: BTreeMap<_, _> = criterion
                    .children()
                    .filter(Node::is_element)
                    .map(|n| (n.tag_name().name(), n.text().unwrap()))
                    .collect();
                assert_eq!(
                    terms,
                    BTreeMap::from([
                        ("RuleSetInUse", "RULESET_JAC_BANSEIKI"),
                        ("GameCoreInUse", "Expansion2"),
                        ("ModIsEnabled", dlc.id)
                    ])
                );
                assert_eq!(criterion.attribute("any"), None);
            }
        }
        let cross = doc.descendants().find(|n| {
            n.has_tag_name("Criteria")
                && n.attribute("id") == Some("JAC_Banseiki_Babylon_GranColombia_Maya")
        });
        if installed.iter().any(|d| d.name == "Babylon") {
            let cross = cross.unwrap();
            assert_eq!(
                cross
                    .children()
                    .find(|n| n.has_tag_name("ModInUse"))
                    .unwrap()
                    .text(),
                Some(ethiopia::MAYA_ID)
            );
            let action = doc
                .descendants()
                .find(|n| {
                    n.has_tag_name("UpdateDatabase")
                        && n.attribute("id") == Some("JAC_Banseiki_Babylon_GranColombia_Maya")
                })
                .unwrap();
            assert_eq!(
                action
                    .descendants()
                    .find(|n| n.has_tag_name("LoadOrder"))
                    .unwrap()
                    .text(),
                Some("2")
            );
        } else {
            assert!(cross.is_none());
        }
        for file in doc
            .descendants()
            .find(|n| n.has_tag_name("Files"))
            .unwrap()
            .children()
            .filter(|n| n.has_tag_name("File"))
        {
            assert!(output.join(file.text().unwrap()).is_file());
        }
    }
}

#[test]
fn refresh_is_deterministic_and_recompiles_source_and_installed_xml() {
    let fixture = PreparationFixture::new("refresh");
    fixture.install(ethiopia::BABYLON);
    let output = fixture.root.join("output");
    fixture.prepare(&output, false).unwrap();
    let once = directory_bytes(&output);
    fixture.prepare(&output, true).unwrap();
    assert_eq!(directory_bytes(&output), once);
    let other = fixture.root.join("other");
    fixture.prepare(&other, false).unwrap();
    assert_eq!(directory_bytes(&other), once);
    let source_file = fixture
        .repo
        .path(ethiopia::MOD_DIR)
        .join("Data/sentinel.sql");
    std::fs::write(&source_file, "-- source version two\n").unwrap();
    let source_info_path = fixture.repo.path(ethiopia::MOD_DIR).join(ethiopia::MODINFO);
    let source_info = std::fs::read_to_string(&source_info_path).unwrap().replace(
        "<EnabledByDefault>1</EnabledByDefault>",
        "<EnabledByDefault>0</EnabledByDefault>",
    );
    std::fs::write(source_info_path, &source_info).unwrap();
    let official = fixture.game.join("DLC/Babylon/Data/BabylonGameplay_0.xml");
    std::fs::write(
        &official,
        "<GameInfo><TestData><Row Id='changed' Value='second'/></TestData></GameInfo>",
    )
    .unwrap();
    fixture.prepare(&output, true).unwrap();
    assert!(std::fs::read_to_string(output.join(ethiopia::MODINFO))
        .unwrap()
        .contains("<EnabledByDefault>0</EnabledByDefault>"));
    assert_eq!(
        std::fs::read(output.join("Data/sentinel.sql")).unwrap(),
        std::fs::read(source_file).unwrap()
    );
    let sql = std::fs::read_to_string(output.join("Data/JAC_Banseiki_Babylon.sql")).unwrap();
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE TestData(Id TEXT PRIMARY KEY,Value TEXT);")
        .unwrap();
    db.execute_batch(&sql).unwrap();
    assert_eq!(
        db.query_row("SELECT Value FROM TestData WHERE Id='changed'", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "second"
    );
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM TestData WHERE Id='BabylonGameplay_0'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    let current = directory_bytes(&output);
    fixture.prepare(&output, true).unwrap();
    assert_eq!(directory_bytes(&output), current);
    let fresh_current = fixture.root.join("fresh-current");
    fixture.prepare(&fresh_current, false).unwrap();
    assert_eq!(directory_bytes(&fresh_current), current);
    assert!(fixture.prepare(&output, false).is_err());
    assert_eq!(directory_bytes(&output), current);
    std::fs::rename(
        fixture.game.join("DLC/Babylon"),
        fixture.game.join("DLC/Babylon-not-installed"),
    )
    .unwrap();
    fixture.prepare(&output, true).unwrap();
    let info = std::fs::read_to_string(output.join(ethiopia::MODINFO)).unwrap();
    let doc = Document::parse(&info).unwrap();
    assert!(!doc.descendants().any(|n| n
        .attribute("id")
        .is_some_and(|id| id.starts_with("JAC_Banseiki_Babylon"))));
    let without_dlc = fixture.root.join("fresh-no-dlc");
    fixture.prepare(&without_dlc, false).unwrap();
    assert_eq!(directory_bytes(&output), directory_bytes(&without_dlc));
}

#[test]
fn refresh_rejects_unowned_or_wrong_mod_outputs_before_any_write() {
    let fixture = PreparationFixture::new("reject");
    let foreign = fixture.root.join("foreign");
    std::fs::create_dir(&foreign).unwrap();
    std::fs::write(foreign.join("important.txt"), "keep").unwrap();
    let before = directory_bytes(&foreign);
    assert!(fixture.prepare(&foreign, true).is_err());
    assert_eq!(directory_bytes(&foreign), before);
    for mutation in ["owner", "id", "unowned"] {
        let output = fixture.root.join(mutation);
        fixture.prepare(&output, false).unwrap();
        match mutation {
            "owner" => {
                let mut owner: serde_json::Value = serde_json::from_slice(
                    &std::fs::read(output.join(ethiopia::OWNER_FILE)).unwrap(),
                )
                .unwrap();
                owner["source_mod_id"] = serde_json::json!("another-mod");
                std::fs::write(
                    output.join(ethiopia::OWNER_FILE),
                    serde_json::to_vec(&owner).unwrap(),
                )
                .unwrap();
            }
            "id" => {
                let info = std::fs::read_to_string(output.join(ethiopia::MODINFO))
                    .unwrap()
                    .replace(ethiopia::SOURCE_MOD_ID, "another-mod");
                std::fs::write(output.join(ethiopia::MODINFO), info).unwrap();
            }
            _ => std::fs::write(output.join("unowned.txt"), "keep").unwrap(),
        }
        let before = directory_bytes(&output);
        assert!(fixture.prepare(&output, true).is_err());
        assert_eq!(directory_bytes(&output), before);
    }
}

#[test]
fn legacy_cli_refresh_updates_its_owned_artifact_and_refuses_existing_fresh_output() {
    let fixture = PreparationFixture::new("cli");
    fixture.install(ethiopia::BABYLON);
    let output = fixture
        .root
        .join("nested/local-mods/JAC_PoC_BanseikiRuleset");
    let run = |refresh| {
        let mut command =
            std::process::Command::new(env!("CARGO_BIN_EXE_banseiki-ethiopia-prepare"));
        command
            .arg("--game-dir")
            .arg(&fixture.game)
            .arg("--output")
            .arg(&output);
        if refresh {
            command.arg("--refresh");
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command.output().unwrap()
    };
    let fresh = run(true); // parent's automatic first invocation also uses --refresh
    assert!(
        fresh.status.success(),
        "{}",
        String::from_utf8_lossy(&fresh.stderr)
    );
    let before = directory_bytes(&output);
    assert!(run(true).status.success());
    assert_eq!(directory_bytes(&output), before);
    let rejected = run(false);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("--refresh is required"));
    assert_eq!(directory_bytes(&output), before);
    let original = fixture.game.join("DLC/Babylon/Data/BabylonGameplay_0.xml");
    std::fs::write(
        original,
        "<GameInfo><TestData><Row Id='cli-changed' Value='new'/></TestData></GameInfo>",
    )
    .unwrap();
    assert!(run(true).status.success());
    assert!(
        std::fs::read_to_string(output.join("Data/JAC_Banseiki_Babylon.sql"))
            .unwrap()
            .contains("'cli-changed'")
    );
}

#[cfg(windows)]
#[test]
fn refresh_rejects_root_and_nested_junctions_without_touching_targets() {
    use std::os::windows::process::CommandExt;
    let fixture = PreparationFixture::new("junction");
    let target = fixture.root.join("target");
    fixture.prepare(&target, false).unwrap();
    let before = directory_bytes(&target);
    let alias = fixture.root.join("alias");
    let junction = |alias: &Path, target: &Path| {
        let result = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(alias)
            .arg(target)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    junction(&alias, &target);
    assert!(fixture.prepare(&alias, true).is_err());
    assert_eq!(directory_bytes(&target), before);
    std::fs::remove_dir(&alias).unwrap(); // unlink only this test's junction, not its target
    let output = fixture.root.join("output");
    fixture.prepare(&output, false).unwrap();
    let nested = output.join("linked");
    junction(&nested, &target);
    let own_before = directory_bytes(&output);
    assert!(fixture.prepare(&output, true).is_err());
    assert_eq!(directory_bytes(&output), own_before);
    assert_eq!(directory_bytes(&target), before);
    std::fs::remove_dir(nested).unwrap();
}
