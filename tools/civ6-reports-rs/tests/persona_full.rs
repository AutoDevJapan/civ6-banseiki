use civ6_reports_rs::persona_full::{verify, verify_directory};
use serde_json::{json, Value};

const SCENARIO: &str = include_str!("../../civ6-autotest/scenarios/leader-persona-15km.json");

// Synthetic protocol fixture, not a claimed live pass. Snapshot values describe
// six distinct stages and are independent of the verifier's parsing logic.
fn fixture() -> (Vec<Value>, Value) {
    let scenario: Value = serde_json::from_str(SCENARIO).unwrap();
    let snapshots = [
        (
            "JAC_PERSONA_GREATPERSON",
            "STATE=GREAT_PERSON|OWNER=0|UNIT=196609|MERIT=0",
        ),
        (
            "JAC_PERSONA_GOVERNOR",
            "STATE=GOVERNOR|OWNER=0|titles=2/2|governor=6|titlePayment1=true",
        ),
        (
            "JAC_PERSONA_ESTABLISHING",
            "STATE=GOVERNOR|OWNER=0|established=false|onSite=1|city=65536",
        ),
        (
            "JAC_PERSONA_MERIT_LOCKS",
            "STATE=GOVERNOR|OWNER=0|MERIT=0|CATEGORY=CONTACT|ENVOYS_1=1",
        ),
        (
            "JAC_PERSONA_REASSIGNED",
            "STATE=GOVERNOR|OWNER=0|established=false|onSite=0|city=131073",
        ),
        (
            "JAC_PERSONA_THREE_COSTS",
            "STATE=GOVERNOR|OWNER=0|titles=6/6|governor=21|titlePayment1=true|titlePayment2=true",
        ),
    ];
    let mut rows = vec![json!({"action":"start","status":"OK","detail":scenario["name"]})];
    for (index, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
        let action = format!("{}/113 {}", index + 1, step["action"].as_str().unwrap());
        rows.push(json!({"action":action,"status":"START","detail":step.to_string()}));
        let mut detail = "operation completed".to_owned();
        if step["action"] == "ui_lua" {
            let key = step["capture_as"]
                .as_str()
                .or(step["equals_capture"].as_str());
            let value = if let Some(key) = key {
                snapshots.iter().find(|(name, _)| *name == key).unwrap().1
            } else if index == 38 {
                "native AI appointment gate verified after first turn"
            } else if index == 112 {
                "no unjoined AI governors or persona Great People"
            } else {
                "true"
            };
            detail = format!(
                "ran Lua in {}; return={value}",
                step["state_name"].as_str().unwrap()
            );
        }
        rows.push(json!({"action":action,"status":"OK","detail":detail}));
    }
    rows.push(json!({"action":"judge","status":"OK","detail":"6/6 checks passed"}));
    // Deliberately keep this contract independent from the implementation list.
    let names = [
        "Generic: JAC Modding.log errors",
        "Generic: JAC Database.log errors",
        "Generic: [JAC_POC] errors",
        "Generic: JAC Runtime Error",
        "Generic: JAC missing texture",
        "Scenario: all Lua errors",
    ];
    let result = json!({"scenario":scenario["name"],"passed":true,"completed_steps":113,
        "failure":null,"test_mode":null,
        "checks":names.map(|name|json!({"name":name,"passed":true,"detail":"0 errors"}))});
    (rows, result)
}

fn text(rows: &[Value]) -> String {
    rows.iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn complete_trace_matches_six_distinct_snapshots_without_claiming_product_acceptance() {
    let (rows, result) = fixture();
    let report = verify(&text(&rows), &result.to_string(), "official UI loaded\n").unwrap();
    assert_eq!(
        (report.completed_steps, report.matched_save_load_snapshots),
        (113, 6)
    );
    assert!(report.early_ai_check && report.final_ai_check);
    assert!(!report.product_acceptance);
}

#[test]
fn forged_success_summary_does_not_hide_final_ai_failure_or_missing_records() {
    let (mut rows, result) = fixture();
    rows[226]["status"] = json!("FAIL");
    rows[226]["detail"] = json!("AI appointed unjoined persona");
    assert!(verify(&text(&rows), &result.to_string(), "")
        .unwrap_err()
        .to_string()
        .contains("non-success record"));
    rows.truncate(225);
    assert!(verify(&text(&rows), &result.to_string(), "").is_err());
}

#[test]
fn restore_drift_is_rejected_even_when_driver_and_summary_say_ok() {
    let (mut rows, result) = fixture();
    // The first restore is step19: a real unit's ID drift must not be masked.
    rows[38]["detail"] =
        json!("ran Lua in InGame; return=STATE=GREAT_PERSON|OWNER=0|UNIT=196610|MERIT=0");
    assert!(verify(&text(&rows), &result.to_string(), "")
        .unwrap_err()
        .to_string()
        .contains("save/load snapshot mismatch"));
}

#[test]
fn altered_ai_guard_or_reordered_duplicate_steps_cannot_supply_original_evidence() {
    let (mut rows, result) = fixture();
    let mut altered: Value = serde_json::from_str(rows[77]["detail"].as_str().unwrap()).unwrap();
    altered["code"] = json!("return 'native AI appointment gate verified after first turn'");
    rows[77]["detail"] = json!(altered.to_string());
    assert!(verify(&text(&rows), &result.to_string(), "")
        .unwrap_err()
        .to_string()
        .contains("differs from original"));
    let (mut rows, result) = fixture();
    rows.swap(1, 3);
    assert!(verify(&text(&rows), &result.to_string(), "").is_err());
    let (mut rows, result) = fixture();
    rows.insert(3, rows[2].clone());
    assert!(verify(&text(&rows), &result.to_string(), "").is_err());
}

#[test]
fn official_ui_error_and_missing_strict_check_are_not_error_free() {
    let (rows, mut result) = fixture();
    assert!(verify(
        &text(&rows),
        &result.to_string(),
        "DiplomacyDealView: Runtime Error: nil value"
    )
    .is_err());
    result["checks"].as_array_mut().unwrap().pop();
    assert!(verify(&text(&rows), &result.to_string(), "").is_err());
}

#[test]
fn mixed_runs_clock_jumps_and_missing_log_slices_fail_closed() {
    let (mut rows, result) = fixture();
    rows.insert(
        40,
        json!({"action":"start","status":"OK","detail":result["scenario"]}),
    );
    assert!(verify(&text(&rows), &result.to_string(), "").is_err());
    let (rows, mut result) = fixture();
    result["test_mode"] = json!({"skipped_turns":0});
    assert!(verify(&text(&rows), &result.to_string(), "").is_err());
    let (rows, result) = fixture();
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("steps.jsonl"), text(&rows)).unwrap();
    std::fs::write(directory.path().join("result.json"), result.to_string()).unwrap();
    assert!(verify_directory(directory.path()).is_err());
    for name in ["Lua.log", "Database.log", "Modding.log"] {
        std::fs::write(directory.path().join(name), "").unwrap();
    }
    assert!(verify_directory(directory.path()).is_ok());
}

#[test]
fn stopped_observer_receipt_is_separate_from_unchanged_lua_snapshots() {
    let (mut rows, result) = fixture();
    // Current runtime records a process receipt while observing. Real event324
    // artifacts have one before save and none on both later snapshot reads.
    for index in [28, 54, 80, 132, 160, 198] {
        let detail = rows[index]["detail"].as_str().unwrap();
        rows[index]["detail"] = json!(format!(
            "{detail}; native_process={{\"pid\":139848,\"created_filetime\":134357136841343557}}"
        ));
    }
    let report = verify(&text(&rows), &result.to_string(), "").unwrap();
    assert_eq!(report.matched_save_load_snapshots, 6);
    rows[38]["detail"] = json!("ran Lua in InGame; return=STATE=GREAT_PERSON|OWNER=0|UNIT=196610|MERIT=0; native_process={\"pid\":139848,\"created_filetime\":134357136841343557}");
    assert!(verify(&text(&rows), &result.to_string(), "")
        .unwrap_err()
        .to_string()
        .contains("save/load snapshot mismatch"));
}

#[test]
fn incomplete_or_invalid_appended_receipt_is_not_silently_discarded() {
    for receipt in [
        "{}",
        "{\"pid\":139848}",
        "{\"pid\":0,\"created_filetime\":1}",
        "{\"pid\":139848,\"created_filetime\":0}",
        "{\"pid\":139848,\"created_filetime\":134357136841343557,\"extra\":true}",
        "{\"pid\":139848,\"created_filetime\":134357136841343557} trailing",
    ] {
        let (mut rows, result) = fixture();
        let before = rows[28]["detail"].as_str().unwrap();
        rows[28]["detail"] = json!(format!("{before}; native_process={receipt}"));
        assert!(
            verify(&text(&rows), &result.to_string(), "").is_err(),
            "accepted {receipt}"
        );
    }
}
