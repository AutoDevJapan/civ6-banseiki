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

#[test]
fn pr87_restart_increment_maps_all_nine_original_methods_to_executable_tests() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("restart-incremental-parity.json")).unwrap();
    assert_eq!(rows.len(), 9);
    let original: BTreeSet<_> = [
        (
            "RestartActionTests",
            "test_failed_new_launch_releases_its_helper_before_next_steam_request",
        ),
        (
            "RestartActionTests",
            "test_helper_cleanup_exhausting_deadline_sends_no_steam_request",
        ),
        (
            "RestartActionTests",
            "test_four_launch_requests_still_wait_for_delayed_window_until_deadline",
        ),
        (
            "RestartActionTests",
            "test_exhausted_launch_requests_fail_at_the_supplied_deadline",
        ),
        (
            "OrphanReporterTests",
            "test_only_verified_orphan_helper_is_closed",
        ),
        (
            "OrphanReporterTests",
            "test_live_game_parent_preserves_reporter",
        ),
        (
            "OrphanReporterTests",
            "test_same_named_other_program_is_preserved",
        ),
        (
            "OrphanReporterTests",
            "test_game_appearing_during_inspection_preserves_reporter",
        ),
        (
            "OrphanReporterTests",
            "test_running_game_prevents_all_cleanup",
        ),
    ]
    .into_iter()
    .collect();
    let mapped: BTreeSet<_> = rows
        .iter()
        .map(|row| {
            (
                row["class"].as_str().unwrap(),
                row["test"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(mapped, original);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut targets = BTreeSet::new();
    for row in &rows {
        assert_eq!(row["source"], "tools/civ6-autotest/tests/test_restart.py");
        assert_eq!(row["rust_test"], row["test"]);
        let target = row["rust_target"].as_str().unwrap();
        assert_eq!(
            target,
            if row["class"] == "RestartActionTests" {
                "driver_restart"
            } else {
                "restart_parity"
            }
        );
        let name = row["rust_test"].as_str().unwrap();
        assert!(targets.insert((target, name)));
        let source = fs::read_to_string(root.join(format!("tests/{target}.rs"))).unwrap();
        let pattern = format!(r"#\[test\]\s*fn\s+{}\s*\(", regex::escape(name));
        assert!(
            regex::Regex::new(&pattern).unwrap().is_match(&source),
            "{target}::{name}"
        );
        assert!(!source.contains("#[ignore"));
    }
    assert_eq!(targets.len(), 9);
}
