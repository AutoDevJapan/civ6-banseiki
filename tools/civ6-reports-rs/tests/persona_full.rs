use civ6_reports_rs::persona_full::{verify, verify_baseline_directory, verify_directory};
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
        "[139848,134357136841343557]",
        "{\"pid\":139848,\"pid\":139849,\"created_filetime\":134357136841343557}",
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

// The shared live_chain's ship::live adds exactly this restart and isolates
// every non-exact save/load name with the artifact directory's run ID.
fn chain_fixture(run_id: &str) -> (Vec<Value>, Value) {
    let (rows, mut result) = fixture();
    let mut out = vec![rows[0].clone()];
    out.push(json!({"action":"1/114 restart_game","status":"START","detail":"{\"action\":\"restart_game\",\"timeout\":420}"}));
    out.push(
        json!({"action":"1/114 restart_game","status":"OK","detail":"Civ VI relaunched via Steam"}),
    );
    for row in &rows[1..rows.len() - 1] {
        let mut row = row.clone();
        let action = row["action"].as_str().unwrap().to_owned();
        let (number, kind) = action.split_once(" ").unwrap();
        let index: usize = number.split('/').next().unwrap().parse().unwrap();
        row["action"] = json!(format!("{}/114 {kind}", index + 1));
        if row["status"] == "START" && matches!(kind, "save" | "load") {
            let mut step: Value = serde_json::from_str(row["detail"].as_str().unwrap()).unwrap();
            let name = step["name"].as_str().unwrap();
            step["name"] = json!(format!("{name}-{run_id}"));
            row["detail"] = json!(step.to_string());
        }
        out.push(row);
    }
    out.push(rows.last().unwrap().clone());
    result["completed_steps"] = json!(114);
    (out, result)
}

fn chain_directory(
    rows: &[Value],
    result: &Value,
    run_id: &str,
) -> (tempfile::TempDir, std::path::PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join(run_id);
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("steps.jsonl"), text(rows)).unwrap();
    std::fs::write(path.join("result.json"), result.to_string()).unwrap();
    for name in ["Lua.log", "Database.log", "Modding.log"] {
        std::fs::write(path.join(name), "").unwrap();
    }
    (temporary, path)
}

#[test]
fn shared_chain_restart_and_run_names_preserve_the_original_113_checks() {
    let (rows, result) = chain_fixture("leaderperson-407");
    let (_temporary, directory) = chain_directory(&rows, &result, "leaderperson-407");
    let report = verify_directory(&directory).unwrap();
    assert_eq!(report.completed_steps, 113);
    assert_eq!(report.matched_save_load_snapshots, 6);
    assert!(report.early_ai_check && report.final_ai_check);
    assert!(!report.product_acceptance);
}

#[test]
fn shared_chain_cannot_change_restart_timeout_save_namespace_or_ai_guard() {
    let (rows, result) = chain_fixture("leaderperson-407");
    for (action, field, value) in [
        ("1/114 restart_game", "timeout", json!(421)),
        (
            "16/114 save",
            "name",
            json!("JAC_PERSONA_GREATPERSON-other-01"),
        ),
        (
            "40/114 ui_lua",
            "code",
            json!("return 'native AI appointment gate verified after first turn'"),
        ),
    ] {
        let mut mutated = rows.clone();
        let row = mutated
            .iter_mut()
            .find(|r| r["action"] == action && r["status"] == "START")
            .unwrap();
        let mut step: Value = serde_json::from_str(row["detail"].as_str().unwrap()).unwrap();
        step[field] = value;
        row["detail"] = json!(step.to_string());
        let (_temporary, directory) = chain_directory(&mutated, &result, "leaderperson-407");
        assert!(
            verify_directory(&directory).is_err(),
            "accepted altered {action}/{field}"
        );
    }
    let (_temporary, directory) = chain_directory(&rows, &result, "other-01");
    assert!(verify_directory(&directory).is_err());
}

#[test]
fn extra_restart_and_mixed_save_load_names_do_not_complete_a_chain_run() {
    let (mut rows, mut result) = chain_fixture("leaderperson-407");
    rows.insert(3, rows[1].clone());
    result["completed_steps"] = json!(115);
    let (_temporary, directory) = chain_directory(&rows, &result, "leaderperson-407");
    assert!(verify_directory(&directory).is_err());

    let (mut rows, result) = chain_fixture("leaderperson-407");
    let row = rows
        .iter_mut()
        .find(|r| r["action"] == "18/114 load" && r["status"] == "START")
        .unwrap();
    let mut step: Value = serde_json::from_str(row["detail"].as_str().unwrap()).unwrap();
    step["name"] = json!("JAC-Persona-GreatPerson-leaderperson-406");
    row["detail"] = json!(step.to_string());
    let (_temporary, directory) = chain_directory(&rows, &result, "leaderperson-407");
    assert!(verify_directory(&directory).is_err());
}

fn baseline_fixture() -> (Vec<Value>, Value) {
    let baseline: Value = serde_json::from_str(include_str!(
        "../../civ6-autotest/scenarios/leader-persona-baseline-15km.json"
    ))
    .unwrap();
    let mut rows = vec![json!({"action":"start","status":"OK","detail":baseline["name"]})];
    rows.push(json!({"action":"1/6 restart_game","status":"START","detail":"{\"action\":\"restart_game\",\"timeout\":420}"}));
    rows.push(
        json!({"action":"1/6 restart_game","status":"OK","detail":"Civ VI relaunched via Steam"}),
    );
    for (index, step) in baseline["steps"].as_array().unwrap().iter().enumerate() {
        let action = format!("{}/6 {}", index + 2, step["action"].as_str().unwrap());
        rows.push(json!({"action":action,"status":"START","detail":step.to_string()}));
        let detail = if step["action"] == "ui_lua" {
            "ran Lua in InGame; return=baseline has 22 factions and idle game core"
        } else {
            "operation completed"
        };
        rows.push(json!({"action":action,"status":"OK","detail":detail}));
    }
    rows.push(json!({"action":"judge","status":"OK","detail":"5/5 checks passed"}));
    let names = [
        "Generic: JAC Modding.log errors",
        "Generic: JAC Database.log errors",
        "Generic: [JAC_POC] errors",
        "Generic: JAC Runtime Error",
        "Generic: JAC missing texture",
    ];
    let result = json!({"scenario":baseline["name"],"passed":true,"completed_steps":6,"failure":null,"test_mode":null,"checks":names.map(|name|json!({"name":name,"passed":true}))});
    (rows, result)
}

#[test]
fn startup_comparison_proves_only_the_matching_configuration_is_ready() {
    let (rows, result) = baseline_fixture();
    let (_temporary, directory) = chain_directory(&rows, &result, "leaderperson-407");
    let report = verify_baseline_directory(&directory).unwrap();
    assert!(report.baseline_ready && report.preparation_restart);
    assert_eq!(
        (
            report.completed_steps,
            report.recorded_steps,
            report.matched_save_load_snapshots
        ),
        (5, 6, 0)
    );
    assert!(
        !report.scenario_completed
            && !report.early_ai_check
            && !report.final_ai_check
            && !report.product_acceptance
    );
    assert!(verify_directory(&directory).is_err());
}

#[test]
fn contamination_changed_seed_or_skipped_readiness_cannot_pass_baseline() {
    let (rows, result) = baseline_fixture();
    for (action, field, value) in [
        (
            "2/6 enable_mods",
            "ids",
            json!(["91cf7b65-54bb-4f69-a452-19a65527cde2"]),
        ),
        ("3/6 configure", "seed", json!(620975)),
        (
            "5/6 ui_lua",
            "code",
            json!("return 'baseline has 22 factions and idle game core'"),
        ),
    ] {
        let mut mutated = rows.clone();
        let row = mutated
            .iter_mut()
            .find(|r| r["action"] == action && r["status"] == "START")
            .unwrap();
        let mut step: Value = serde_json::from_str(row["detail"].as_str().unwrap()).unwrap();
        step[field] = value;
        row["detail"] = json!(step.to_string());
        let (_temporary, directory) = chain_directory(&mutated, &result, "leaderperson-407");
        assert!(
            verify_baseline_directory(&directory).is_err(),
            "accepted altered {action}/{field}"
        );
    }
    let mut skipped = rows.clone();
    skipped[10]["status"] = json!("SKIP");
    let (_temporary, directory) = chain_directory(&skipped, &result, "leaderperson-407");
    assert!(verify_baseline_directory(&directory).is_err());
}

#[test]
fn successful_summary_cannot_hide_raw_log_errors_in_either_profile() {
    for baseline in [false, true] {
        let (rows, result) = if baseline {
            baseline_fixture()
        } else {
            chain_fixture("leaderperson-407")
        };
        for (name, error) in [
            ("Database.log", "[123] ERROR: no such table\n[123] from file Mods/JAC_PoC_LeaderPersona/Data/persona.sql\n"),
            ("Modding.log", "Loading JAC_PoC_LeaderPersona\n[123] Failed to apply component\n"),
            ("Lua.log", "[JAC_POC] operation failed\n"),
            ("Lua.log", "GovernorPanel: Runtime Error: bad argument\n  JAC_LeaderPersona_View.lua:37\n"),
            ("Lua.log", "Failed to load background image texture: JAC_BadTexture.dds\n"),
        ] {
            let (_temporary, directory) = chain_directory(&rows, &result, "leaderperson-407");
            std::fs::write(directory.join(name), error).unwrap();
            let verified = if baseline { verify_baseline_directory(&directory) } else { verify_directory(&directory) };
            assert!(verified.unwrap_err().to_string().contains("raw log check failed"), "accepted stale successful summary for {name}/baseline={baseline}");
        }
    }
}

#[test]
fn benign_log_lines_and_unrelated_database_errors_keep_the_runner_policy() {
    let (rows, result) = baseline_fixture();
    let (_temporary, directory) = chain_directory(&rows, &result, "leaderperson-407");
    std::fs::write(
        directory.join("Database.log"),
        "[123] ERROR: unrelated vanilla asset\nfrom file Base/Assets/Data/core.xml\n",
    )
    .unwrap();
    std::fs::write(
        directory.join("Lua.log"),
        "[JAC_POC] operation ok error=none\n[JAC_POC] operation ok error=nil\n",
    )
    .unwrap();
    assert!(
        verify_baseline_directory(&directory)
            .unwrap()
            .baseline_ready
    );
}
