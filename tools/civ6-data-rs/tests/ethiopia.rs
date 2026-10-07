use civ6_data_rs::{ethiopia, Repository};
use roxmltree::{Document, Node};
use rusqlite::{params_from_iter, types::Value as Sql, Connection};
use std::collections::{BTreeMap, BTreeSet};

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

fn foreign_keys(db: &Connection) -> BTreeSet<(String, Option<i64>, String, i64)> {
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

fn assert_no_new_foreign_keys(
    db: &Connection,
    baseline: &BTreeSet<(String, Option<i64>, String, i64)>,
) {
    let new: Vec<_> = foreign_keys(db).difference(baseline).cloned().collect();
    assert!(new.is_empty(), "new XML-defined FK failures: {new:?}");
}

fn snapshot(db: &Connection) -> BTreeMap<String, Vec<Vec<Sql>>> {
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
fn assert_same_data(db: &Connection, expected: &BTreeMap<String, Vec<Vec<Sql>>>, state: &str) {
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
        assert_eq!(
            normalize(&actual[table]),
            normalize(rows),
            "{state}: {table}"
        );
    }
}
