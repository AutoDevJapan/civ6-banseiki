use serde_json::Value;
use std::{collections::BTreeSet, fs, path::PathBuf};

#[test]
fn every_legacy_method_has_an_executable_mapping_and_no_ignored_tests() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let baseline: Vec<Value> = serde_json::from_str(include_str!("legacy-inventory.json")).unwrap();
    let mapping: Vec<Value> = serde_json::from_str(include_str!("parity.json")).unwrap();
    assert_eq!(baseline.len(), 214);
    assert_eq!(mapping.len(), 214);
    let id =
        |row: &Value| ["source", "class", "test"].map(|key| row[key].as_str().unwrap().to_string());
    let old_ids: BTreeSet<_> = baseline.iter().map(id).collect();
    let mapped_ids: BTreeSet<_> = mapping.iter().map(id).collect();
    assert_eq!(old_ids.len(), 214);
    assert_eq!(mapped_ids, old_ids);
    for row in mapping {
        let target = row["rust_target"].as_str().unwrap();
        let test = row["rust_test"].as_str().unwrap();
        assert!(!test.is_empty());
        let path = if target == "lib" {
            root.join("src/tuner.rs")
        } else if test.starts_with("banseiki_ruleset::") {
            root.join("tests/fixtures/banseiki_ruleset.rs")
        } else {
            root.join(format!("tests/{target}.rs"))
        };
        let source = fs::read_to_string(&path).unwrap();
        let function = test.rsplit("::").next().unwrap();
        let pattern = format!(
            r"(?s)#\[test\]\s*(?:#\[[^\]]+\]\s*)?fn\s+{}\s*\(",
            regex::escape(function)
        );
        assert!(
            regex::Regex::new(&pattern).unwrap().is_match(&source),
            "{}::{} -> missing #[test] {} in {}",
            row["source"],
            row["test"],
            test,
            path.display()
        );
        assert!(
            !source.contains("#[ignore"),
            "{} contains an ignored test",
            path.display()
        );
    }
}
