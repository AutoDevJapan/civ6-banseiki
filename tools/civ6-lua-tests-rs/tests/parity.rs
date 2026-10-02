use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Deserialize)]
struct Parity {
    source: String,
    #[serde(rename = "class")]
    class_name: String,
    test: String,
    rust_test: String,
}

#[test]
fn parity_inventory_preserves_all_358_original_methods() {
    let rows: Vec<Parity> = serde_json::from_str(include_str!("parity.json")).unwrap();
    assert_eq!(rows.len(), 358);
    let expected = BTreeMap::from([
        ("tests/test_assassin_lua.py", 87),
        ("tests/test_court_arbitration.py", 52),
        ("tests/test_religion_persistence.py", 71),
        ("tests/test_sengoku_ages.py", 63),
        ("tests/test_sengoku_ages_screens.py", 31),
        ("tests/test_house_names_lua.py", 7),
        ("tests/test_historical_events_lua.py", 18),
        ("tests/test_japan_1534_relations_lua.py", 4),
        ("tests/test_unification_pact_lua.py", 25),
    ]);
    let mut counts = BTreeMap::<&str, usize>::new();
    let mut originals = BTreeSet::new();
    let mut rust_tests = BTreeSet::new();
    for row in &rows {
        assert!(originals.insert((&row.source, &row.class_name, &row.test)));
        assert!(
            rust_tests.insert(&row.rust_test),
            "combined test {}",
            row.rust_test
        );
        *counts.entry(&row.source).or_default() += 1;
        let (target, _) = row.rust_test.split_once("::").unwrap();
        assert_eq!(row.rust_test.rsplit("::").next(), Some(row.test.as_str()));
        assert!(!row.class_name.is_empty());
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(format!("{target}.rs"));
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(
            source.contains(&row.test),
            "missing Rust test {}",
            row.rust_test
        );
        assert!(
            !source.contains("#[ignore"),
            "disabled tests in {}",
            path.display()
        );
    }
    assert_eq!(counts, expected);
}
