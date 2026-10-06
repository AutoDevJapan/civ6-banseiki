use civ6_data_rs::{cascade, Repository};
use roxmltree::Document;
use rusqlite::{types::Value, Connection};
use std::collections::BTreeSet;

fn buildings(db: &Connection) -> Vec<Vec<Value>> {
    let mut statement = db
        .prepare("SELECT * FROM Buildings ORDER BY BuildingType")
        .unwrap();
    let columns = statement.column_count();
    statement
        .query_map([], |row| (0..columns).map(|i| row.get(i)).collect())
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn opt_in_capacity_fixture_changes_only_four_internal_flags_after_sengoku() {
    let repo = Repository::checked_in();
    let official = cascade::official_db_for(&repo).unwrap();
    let db = cascade::run_sengoku(&repo, &official, true).unwrap();
    let sql_path = "Data/JAC_PersonaBoundary_Capacity.sql";
    let mod_root = "poc/JAC_Test_PersonaBoundary";
    let modinfo = repo
        .read(format!("{mod_root}/JAC_Test_PersonaBoundary.modinfo"))
        .unwrap();
    let doc = Document::parse(&modinfo).unwrap();
    let actions: Vec<_> = doc
        .descendants()
        .filter(|n| {
            n.has_tag_name("UpdateDatabase")
                && n.children()
                    .find(|child| child.has_tag_name("File"))
                    .and_then(|file| file.text())
                    == Some(sql_path)
        })
        .collect();
    assert_eq!(actions.len(), 4);
    let maps: BTreeSet<_> = actions
        .iter()
        .map(|action| {
            assert_eq!(
                action
                    .children()
                    .find(|n| n.has_tag_name("File"))
                    .unwrap()
                    .text(),
                Some(sql_path)
            );
            assert_eq!(
                action
                    .descendants()
                    .find(|n| n.has_tag_name("LoadOrder"))
                    .unwrap()
                    .text(),
                Some("901")
            );
            let id = action.attribute("criteria").unwrap();
            let criterion = doc
                .descendants()
                .find(|n| n.has_tag_name("Criteria") && n.attribute("id") == Some(id))
                .unwrap();
            assert_eq!(
                criterion
                    .children()
                    .find(|n| n.has_tag_name("RuleSetInUse"))
                    .unwrap()
                    .text(),
                Some("RULESET_JAC_BANSEIKI")
            );
            let setting = criterion
                .children()
                .find(|n| n.has_tag_name("ConfigurationValueMatches"))
                .unwrap();
            assert_eq!(
                setting
                    .children()
                    .find(|n| n.has_tag_name("Group"))
                    .unwrap()
                    .text(),
                Some("Map")
            );
            assert_eq!(
                setting
                    .children()
                    .find(|n| n.has_tag_name("ConfigurationId"))
                    .unwrap()
                    .text(),
                Some("MAP_SCRIPT")
            );
            setting
                .children()
                .find(|n| n.has_tag_name("Value"))
                .unwrap()
                .text()
                .unwrap()
        })
        .collect();
    let persona_source = repo
        .read("poc/JAC_PoC_LeaderPersona/JAC_PoC_LeaderPersona.modinfo")
        .unwrap();
    let persona = Document::parse(&persona_source).unwrap();
    // These criteria loaded the real fixture in live120. Compare the full map
    // identity, including its mod GUID, rather than accepting bare filenames.
    let native_maps: BTreeSet<_> = persona
        .descendants()
        .filter(|n| n.has_tag_name("ConfigurationValueMatches"))
        .map(|n| {
            n.children()
                .find(|c| c.has_tag_name("Value"))
                .unwrap()
                .text()
                .unwrap()
        })
        .collect();
    assert_eq!(native_maps.len(), 4);
    assert_eq!(maps, native_maps);
    assert!(doc
        .descendants()
        .find(|n| n.has_tag_name("Files"))
        .unwrap()
        .children()
        .any(|n| n.has_tag_name("File") && n.text() == Some(sql_path)));
    let columns: Vec<String> = db
        .prepare("SELECT * FROM Buildings")
        .unwrap()
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let kind = columns.iter().position(|n| n == "BuildingType").unwrap();
    let flag = columns.iter().position(|n| n == "InternalOnly").unwrap();
    let changed = BTreeSet::from([
        "BUILDING_GOV_TALL",
        "BUILDING_GOV_WIDE",
        "BUILDING_GOV_CONQUEST",
        "BUILDING_GOV_SPIES",
    ]);
    let mut expected = buildings(&db);
    let mut observed = BTreeSet::new();
    for row in &mut expected {
        let Value::Text(name) = &row[kind] else {
            panic!("building name is not text")
        };
        if changed.contains(name.as_str()) {
            observed.insert(name.clone());
            assert_eq!(
                row[flag],
                Value::Integer(1),
                "Sengoku gate changed for {name}"
            );
            row[flag] = Value::Integer(0);
        }
    }
    assert_eq!(observed, changed.into_iter().map(str::to_owned).collect());
    db.execute_batch(&repo.read(format!("{mod_root}/{sql_path}")).unwrap())
        .unwrap();
    assert_eq!(
        buildings(&db),
        expected,
        "costs, prerequisites, effects and other buildings must stay intact"
    );
}

#[test]
fn opt_in_third_spy_civic_adds_one_native_grant_only_on_the_15km_fixture() {
    let repo = Repository::checked_in();
    let official = cascade::official_db_for(&repo).unwrap();
    let db = cascade::run_sengoku(&repo, &official, true).unwrap();
    let mod_root = "poc/JAC_Test_PersonaBoundary";
    let sql_path = "Data/JAC_PersonaBoundary_ThirdSpy.sql";
    let modinfo = repo
        .read(format!("{mod_root}/JAC_Test_PersonaBoundary.modinfo"))
        .unwrap();
    let doc = Document::parse(&modinfo).unwrap();
    let actions: Vec<_> = doc
        .descendants()
        .filter(|node| {
            node.has_tag_name("UpdateDatabase")
                && node
                    .children()
                    .find(|child| child.has_tag_name("File"))
                    .and_then(|file| file.text())
                    == Some(sql_path)
        })
        .collect();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].attribute("criteria"), Some("Boundary15"));
    assert_eq!(
        actions[0]
            .descendants()
            .find(|node| node.has_tag_name("LoadOrder"))
            .unwrap()
            .text(),
        Some("902")
    );
    assert!(doc
        .descendants()
        .find(|node| node.has_tag_name("Files"))
        .unwrap()
        .children()
        .any(|node| node.has_tag_name("File") && node.text() == Some(sql_path)));
    let spy_grants = || {
        let mut query = db
            .prepare(
                "SELECT CivicType, ModifierId FROM CivicModifiers WHERE ModifierId='CIVIC_GRANT_SPY' ORDER BY CivicType",
            )
            .unwrap();
        query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    assert_eq!(
        spy_grants(),
        vec![(
            "CIVIC_JAC_SENGOKU_LOCAL_LEAGUES_AND_INTELLIGENCE".to_owned(),
            "CIVIC_GRANT_SPY".to_owned()
        )]
    );
    let sql = repo.read(format!("{mod_root}/{sql_path}")).unwrap();
    db.execute_batch(&sql).unwrap();
    let granted = spy_grants();
    assert_eq!(
        granted,
        vec![
            (
                "CIVIC_JAC_SENGOKU_LOCAL_LEAGUES_AND_INTELLIGENCE".to_owned(),
                "CIVIC_GRANT_SPY".to_owned()
            ),
            (
                "CIVIC_JAC_SENGOKU_NANBAN_ENVOYS".to_owned(),
                "CIVIC_GRANT_SPY".to_owned()
            )
        ]
    );
    db.execute_batch(&sql).unwrap();
    assert_eq!(spy_grants(), granted);
}
