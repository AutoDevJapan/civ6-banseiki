//! SYNTHETIC UNIT MODELS ONLY. No fixture is an actual capture or product pass.
use civ6_autotest_rs::catalog_check;
use serde_json::{json, Value};
use std::path::Path;

fn lines(rows: &[Value]) -> String {
    rows.iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}
fn model(directory: &Path) -> (Vec<Value>, Vec<Value>) {
    // Immutable 4cd contract captured before these checker fixes. Do not call
    // the production generator: unintended changes must fail the tests.
    let scenario: Value =
        serde_json::from_str(include_str!("fixtures/catalog-check/source-contract.json")).unwrap();
    let steps = scenario["steps"].as_array().unwrap();
    let receipts: Value =
        serde_json::from_str(include_str!("fixtures/catalog-check/runtime-receipts.json")).unwrap();
    // Raw model uses a separate fixed table, not the checker or Lua generator.
    let index = |s: usize| 17 + s * 7;
    let stable = receipts["stable"].as_str().unwrap().to_owned();
    let record = |action: String, status: &str, detail: String| json!({"at":"2026-10-06T00:00:00Z","action":action,"status":status,"detail":detail});
    let mut log = vec![record(
        "start".into(),
        "OK",
        scenario["name"].as_str().unwrap().into(),
    )];
    for (i, step) in steps.iter().enumerate() {
        let action = format!("{}/43 {}", i + 1, step["action"].as_str().unwrap());
        log.push(record(action.clone(), "START", step.to_string()));
        let detail=match step["action"].as_str().unwrap() {
            "start_native_production_admission"=>"native production-admission original-return telemetry ready; artifact normal-catalog22.jsonl; process={\"pid\":7,\"created_filetime\":123}".into(),
            "stop_native_production_admission"=>format!("native entry restored; artifact {}",directory.join("normal-catalog22.jsonl").display()),
            "exit_to_menu"=>"main menu reached".into(),
            "ui_lua"=> {
                let key=step["capture_as"].as_str().or_else(||step["equals_capture"].as_str());
                let value=match key {
                    Some("CATALOG880_STABLE")=>stable.clone(),
                    Some("CATALOG880_CLOSED")=>receipts["closed"].as_str().unwrap().into(),
                    Some("CATALOG880_FINAL")=>receipts["final"].as_str().unwrap().into(),
                    Some(key)=>receipts["batches"][key].as_str().unwrap().into(),
                    None if i==5=>"CATALOG880 official civics only; UI automatic submission paused; no game turn requested".into(),
                    None if i==9=>"PAYMENT880 observed 484 existing closed Core qualifications after observer installation".into(),
                    None=>receipts[if step["state_name"]=="InGame" {"ui"}else{"core"}].as_str().unwrap().into(),
                };
                format!("ran Lua in {}; return={value}",step["state_name"].as_str().unwrap())
            }
            _=>"synthetic unit model setup result; not live evidence".into(),
        };
        log.push(record(action, "OK", detail));
    }
    log.push(record("judge".into(), "OK", "6/6 checks passed".into()));
    let mut trace = vec![
        json!({"kind":"identity","pid":7,"created_filetime":123,"gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256,"mode":"production-admission","entry_rva":0x2dddd0,"can_ever_rva":0x2dddd0,"property_rva":0x99f5c0,"entry_bytes":"[48, 89, 5c, 24, 08, 48, 89, 6c]","property_entry_bytes":"[48, 8b, 41, 40, 4c, 8d, 49, 30]","prefix_bytes":"[cc, cc, cc, cc, cc]","bridge_sha256":"a".repeat(64),"duration_limit_seconds":300,"paid_assignment_experimental":false,"scope_key_ranges":[[900000,900255],[901000,901255],[903000,903255]],"reset_key_range":[902000,902063]}),
        json!({"kind":"install","status":0}),
        json!({"kind":"uninstall","status":0,"entry_restored":true,"assignment_restored":false,"prefix_retained_until_exit":true}),
    ];
    for s in 0..22 {
        let owner = (s + 1) % 22;
        let address = 0x10000 + owner * 0x1000;
        trace.push(json!({"kind":"calibration","sequence":s+1,"governor_index":index(s),"governors_this":format!("0x{:016x}",0x90000+owner*0x1000),"return_site_rva":0xba523,"thread_id":11,"owner_read_state":1,"player_address":format!("0x{address:016x}"),"player_id":owner,"property_receiver":format!("0x{:016x}",address+0x528),"original_al":s%2,"effective_al":1}));
    }
    let mut sequence = 0;
    for s in 0..22 {
        let owner = (s + 1) % 22;
        let receiver = 0x10528 + owner * 0x1000;
        for (key, payload) in [
            (902000 + owner, 1),
            (903000 + index(s), 1),
            (901000 + index(s), 0),
            (900000 + index(s), 1),
            (901000 + index(s), 1),
        ] {
            sequence += 1;
            let cell = (900000..=900255).contains(&key) || (901000..=901255).contains(&key);
            trace.push(json!({"kind":"property_get","sequence":sequence,"property":format!("0x{receiver:016x}"),"key_id":key,"returned":"0x0000000000800000","thread_id":12,"read_state":1,"tag":1,"payload8":format!("0x{payload:016x}"),"flags14":0,
                "denial_owner":if cell {owner}else{0},"denial_index":if cell {index(s)}else{0},"denial_checked":false}));
        }
    }
    trace.push(json!({"kind":"stop","rows":132,"observed_calls":132,"dropped":0,"filtered_calls":10,"getter_calls_seen":142,"counter_scope":"bounded_prefix_samples_not_complete_call_history","calibration_sample_limit":1024,"property_sample_limit":4096,"in_flight":0,"snapshot_stable":true,"quiescence_proven":false,"classification":"production_admission","prefix_retained_until_exit":true,"result":"ok"}));
    (trace, log)
}
fn check(trace: &[Value], steps: &[Value]) -> civ6_autotest_rs::Result<catalog_check::Report> {
    let e = catalog_check::evidence_from_steps(&lines(steps))?;
    catalog_check::verify(&lines(trace), &e)
}
fn rejected(trace: &[Value], steps: &[Value]) {
    assert!(check(trace, steps).map_or(true, |r| !r.qualification_positive22));
}

#[test]
fn synthetic_unit_model_positive22_is_never_product_or_payment_authority() {
    let (trace, steps) = model(Path::new("unit-model"));
    let r = check(&trace, &steps).unwrap();
    assert!(r.capture_valid && r.qualification_positive22);
    assert!(!r.product_acceptance && !r.payment_authority && !r.full_history_authority);
    assert!(!r.cross_ring_chronology_measured);
    assert_eq!(
        (
            r.ui_native_qualification.positive,
            r.ui_native_qualification.negative
        ),
        (22, 462)
    );
    assert_eq!(r.gameplay_projection.marker, "GAMEPLAY_PROJECTION");
    assert_eq!(r.pairs.iter().map(|p| p.rows).sum::<usize>(), 22);
    assert_eq!(
        r.pairs
            .iter()
            .map(|p| p.core_allow_true_rows)
            .sum::<usize>(),
        22
    );
    assert_eq!(r.pairs.len(), 484);
}

#[test]
fn raw_subject_source_value_and_lifecycle_mutations_are_rejected() {
    let (trace, steps) = model(Path::new("unit-model"));
    let end = trace.len() - 1;
    for (row, field, value) in [
        (0, "pid", json!(8)),
        (0, "created_filetime", json!(124)),
        (0, "gamecore_sha256", json!("0".repeat(64))),
        (0, "mode", json!("joined-admission")),
        (1, "status", json!(1)),
        (2, "entry_restored", json!(false)),
        (3, "player_id", json!(2)),
        (3, "governor_index", json!(18)),
        (3, "property_receiver", json!("0x0000000000015528")),
        (3, "sequence", json!(2)),
        (3, "original_al", json!(2)),
        (3, "owner_read_state", json!(0)),
        (25, "key_id", json!(902002)),
        (26, "tag", json!(2)),
        (26, "payload8", json!("NaN")),
        (26, "thread_id", json!(99)),
        (end, "result", json!("partial_raw")),
        (end, "in_flight", json!(1)),
        (end, "observed_calls", json!(133)),
        (end, "getter_calls_seen", json!(143)),
        (end, "dropped", json!(1)),
        (end, "counter_scope", json!("complete_history")),
        (end, "calibration_sample_limit", json!(1023)),
    ] {
        let mut bad = trace.clone();
        bad[row][field] = value;
        rejected(&bad, &steps);
    }
}
#[test]
fn missing_raw_positive_is_not_supplemented_by_ui_or_gameplay() {
    let (mut trace, steps) = model(Path::new("unit-model"));
    trace[3]["effective_al"] = json!(0);
    let r = check(&trace, &steps).unwrap();
    assert!(r.capture_valid);
    assert!(!r.qualification_positive22);
    assert_eq!(r.missing_positive_sources, vec![0]);
    trace[3]["effective_al"] = json!(1);
    trace[29]["payload8"] = json!("0x0000000000000000");
    assert_eq!(
        check(&trace, &steps).unwrap().missing_positive_sources,
        vec![0]
    );
}
#[test]
fn truncated_duplicate_junk_and_reordered_raw_are_rejected() {
    let (trace, steps) = model(Path::new("unit-model"));
    let e = catalog_check::evidence_from_steps(&lines(&steps)).unwrap();
    for text in [
        lines(&trace[..trace.len() - 1]),
        lines(&trace) + "junk\n",
        lines(&trace) + "{}\n",
        lines(&trace).replacen("\"pid\":7", "\"pid\":7,\"pid\":7", 1),
    ] {
        assert!(catalog_check::verify(&text, &e).is_err());
    }
    let mut bad = trace.clone();
    bad.insert(3, trace[0].clone());
    rejected(&bad, &steps);
    let mut bad = trace.clone();
    bad.swap(3, 4);
    rejected(&bad, &steps);
    let mut bad = trace.clone();
    bad.insert(4, trace[3].clone());
    rejected(&bad, &steps);
}
#[test]
fn explicit_receipt_start_source_and_capture_mutations_are_rejected() {
    let (trace, steps) = model(Path::new("unit-model"));
    for needle in [
        "UI_NATIVE_QUALIFICATION",
        "GAMEPLAY_PROJECTION",
        "positive=22 negative=462",
        "normal-catalog22.jsonl; process=",
        "CATALOG880_STABLE",
    ] {
        let text = lines(&steps).replace(needle, "BAD");
        assert!(
            catalog_check::evidence_from_steps(&text).is_err(),
            "{needle}"
        );
    }
    let mut bad = steps.clone();
    bad.remove(20);
    rejected(&trace, &bad);
    let mut bad = steps.clone();
    bad[20]["status"] = json!("FAIL");
    rejected(&trace, &bad);
    let mut bad = steps.clone();
    bad.insert(20, steps[20].clone());
    rejected(&trace, &bad);
    let mut bad = steps.clone();
    bad[steps.len() - 3]["detail"] = json!("main menu pending");
    rejected(&trace, &bad);
}
#[test]
fn manufactured_unsaturated_dropped_prefix_cannot_pass() {
    let (mut trace, steps) = model(Path::new("unit-model"));
    let stop = trace.last_mut().unwrap();
    stop["dropped"] = json!(80);
    stop["observed_calls"] = json!(212);
    stop["getter_calls_seen"] = json!(222);
    rejected(&trace, &steps);
}
#[test]
fn saturated_prefix_is_accepted_without_full_history_authority() {
    let (mut trace, steps) = model(Path::new("unit-model"));
    for seq in 23..=1024 {
        let mut row = trace[3].clone();
        row["sequence"] = json!(seq);
        trace.insert(seq + 2, row);
    }
    let stop = trace.last_mut().unwrap();
    stop["rows"] = json!(1134);
    stop["dropped"] = json!(80);
    stop["observed_calls"] = json!(1214);
    stop["getter_calls_seen"] = json!(1224);
    let r = check(&trace, &steps).unwrap();
    assert!(r.qualification_positive22);
    assert_eq!(r.calibration_rows, 1024);
    assert!(!r.full_history_authority);
}
#[test]
fn expanded_prefix_accepts_all_six_matrices_without_claiming_complete_history() {
    let (mut trace, steps) = model(Path::new("unit-model"));
    let count = 6 * 22 * 22;
    for seq in 23..=count {
        let mut row = trace[3].clone();
        row["sequence"] = json!(seq);
        trace.insert(seq + 2, row);
    }
    let stop = trace.last_mut().unwrap();
    stop["calibration_sample_limit"] = json!(8192);
    stop["rows"] = json!(count + 110);
    stop["observed_calls"] = json!(count + 110);
    stop["getter_calls_seen"] = json!(count + 120);
    let report = check(&trace, &steps).unwrap();
    assert_eq!(report.calibration_rows, count);
    assert_eq!(report.calibration_sample_limit, 8192);
    assert!(report.qualification_positive22);
    assert!(!report.full_history_authority && !report.product_acceptance);
    for capacity in [1024, 2048, 8193] {
        let mut bad = trace.clone();
        bad.last_mut().unwrap()["calibration_sample_limit"] = json!(capacity);
        rejected(&bad, &steps);
    }
}

#[test]
fn different_artifact_directory_or_stop_path_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let (trace, steps) = model(dir.path());
    let t = dir.path().join("normal-catalog22.jsonl");
    let s = dir.path().join("steps.jsonl");
    std::fs::write(&t, lines(&trace)).unwrap();
    std::fs::write(&s, lines(&steps)).unwrap();
    assert!(
        catalog_check::check_files(&t, &s)
            .unwrap()
            .qualification_positive22
    );
    let other = tempfile::tempdir().unwrap();
    let other_t = other.path().join("normal-catalog22.jsonl");
    std::fs::write(&other_t, lines(&trace)).unwrap();
    assert!(catalog_check::check_files(&other_t, &s).is_err());
    let (_, foreign_steps) = model(other.path());
    std::fs::write(&s, lines(&foreign_steps)).unwrap();
    assert!(catalog_check::check_files(&t, &s).is_err());
}
#[test]
fn trailing_transport_newlines_and_typed_optional_denial_fields() {
    let (trace, mut steps) = model(Path::new("unit-model"));
    for row in &mut steps {
        if row["status"] == "OK" && row["detail"].as_str().unwrap().starts_with("ran Lua in ") {
            row["detail"] = json!(format!("{}\r\n", row["detail"].as_str().unwrap()));
        }
    }
    assert!(check(&trace, &steps).unwrap().qualification_positive22);
}

#[test]
fn shipped_leading_restart_uses_actions_and_successful_reads_not_43_offsets() {
    let (trace, mut steps) = model(Path::new("unit-model"));
    for row in &mut steps {
        let a = row["action"].as_str().unwrap();
        if let Some((n, operation)) = a.split_once(' ') {
            let (n, _) = n.split_once('/').unwrap();
            row["action"] = json!(format!(
                "{}/44 {operation}",
                n.parse::<usize>().unwrap() + 1
            ));
        }
    }
    steps.insert(1,json!({"at":"2026-10-06T00:00:00Z","action":"1/44 restart_game","status":"START","detail":"{\"action\":\"restart_game\",\"timeout\":420}"}));
    steps.insert(2,json!({"at":"2026-10-06T00:00:00Z","action":"1/44 restart_game","status":"OK","detail":"synthetic restart model only"}));
    let r = check(&trace, &steps).unwrap();
    assert!(r.qualification_positive22);
    assert_eq!(r.ui_native_qualification.step, 40);
    let mut bad = steps.clone();
    bad[1]["detail"] = json!("{\"action\":\"restart_game\",\"code\":\"malicious\"}");
    rejected(&trace, &bad);
}

#[test]
fn partial_run_and_wrong_receipt_kind_cannot_supply_positive_evidence() {
    let (trace, steps) = model(Path::new("unit-model"));
    rejected(&trace, &steps[..25]);
    let mut partial = steps[..25].to_vec();
    partial.last_mut().unwrap()["status"] = json!("FAIL");
    partial.last_mut().unwrap()["detail"] = json!("bad argument #1 to tostring (value expected)");
    rejected(&trace, &partial);
    let mut swapped = steps.clone();
    let final_ledger = steps
        .iter()
        .find(|r| {
            r["status"] == "OK"
                && r["detail"].as_str().unwrap().starts_with(
                    "ran Lua in GameCore_Tuner; return=CATALOG880 MATRIX identities=22 0:1:",
                )
                && r["detail"].as_str().unwrap().contains(":EVOLVING:")
        })
        .unwrap()["detail"]
        .clone();
    for row in &mut swapped {
        if row["status"] == "OK"
            && row["detail"]
                .as_str()
                .unwrap()
                .starts_with("ran Lua in GameCore_Tuner; return=positive=")
        {
            row["detail"] = final_ledger.clone();
        }
    }
    rejected(&trace, &swapped);
}

#[test]
fn nil_and_numeric_nan_property_values_cannot_be_qualification() {
    let (trace, steps) = model(Path::new("unit-model"));
    for (field, value) in [
        ("tag", json!(0)),
        ("tag", json!(2)),
        ("payload8", json!("0x7ff8000000000000")),
        ("read_state", json!(0)),
        ("payload8", Value::Null),
    ] {
        let mut bad = trace.clone();
        bad[29][field] = value;
        rejected(&bad, &steps);
    }
}

#[test]
fn missing_close_ack_reset_or_manifest_never_grants_raw_positive() {
    let (trace, steps) = model(Path::new("unit-model"));
    for row in 25..29 {
        let mut bad = trace.clone();
        bad[row]["key_id"] = json!(903250);
        rejected(&bad, &steps);
    }
}

#[test]
fn explicit_native_denial_cannot_count_as_a_core_positive() {
    let (mut trace, steps) = model(Path::new("unit-model"));
    trace[29]["denial_checked"] = json!(true);
    trace[29]["denial_owner"] = json!(1);
    trace[29]["denial_index"] = json!(17);
    let r = check(&trace, &steps).unwrap();
    assert!(r.capture_valid);
    assert_eq!(r.missing_positive_sources, vec![0]);
    assert!(!r.qualification_positive22);
}

#[test]
fn failed_scenario_keeps_structural_capture_separate_from_unmeasured_qualification() {
    let dir = tempfile::tempdir().unwrap();
    let (mut trace, mut steps) = model(dir.path());
    // Unit model of a failure before the first UI native getter, not live data.
    steps.truncate(23);
    steps[22]["status"] = json!("FAIL");
    steps[22]["detail"] = json!("Lua error: bad argument #1 to tostring (value expected)");
    trace.drain(3..25);
    let stop = trace.last_mut().unwrap();
    stop["rows"] = json!(110);
    stop["observed_calls"] = json!(110);
    stop["getter_calls_seen"] = json!(120);
    let t = dir.path().join("normal-catalog22.jsonl");
    let s = dir.path().join("steps.jsonl");
    std::fs::write(&t, lines(&trace)).unwrap();
    std::fs::write(&s, lines(&steps)).unwrap();
    let r = catalog_check::check_files(&t, &s).unwrap();
    assert!(r.capture_valid);
    assert!(
        !r.scenario_complete && !r.successful_stop_step_verified && !r.qualification_positive22
    );
    assert!(!r.ui_native_qualification.measured && !r.gameplay_projection.measured);
    assert!(!r.gameplay_closed484);
    assert_eq!((r.calibration_rows, r.property_rows), (0, 110));
    assert_eq!(r.missing_positive_sources.len(), 22);
    assert_eq!(r.qualification_errors.len(), 1);
}

#[test]
fn signed_runtime_hashes_and_capital_ids_shared_by_different_owners_are_legal() {
    let (trace, mut steps) = model(Path::new("unit-model"));
    for row in &mut steps {
        if row["status"] != "OK" {
            continue;
        }
        let detail = row["detail"].as_str().unwrap();
        if !detail.contains("CATALOG880 STABLE turn=") {
            continue;
        }
        let (prefix, rest) = detail.split_once(" catalog=").unwrap();
        let (entries, suffix) = rest.split_once(" titles=").unwrap();
        let records = entries
            .split(';')
            .enumerate()
            .map(|(i, e)| {
                let mut fields: Vec<_> = e.split(':').map(str::to_owned).collect();
                fields[4] = (-1748467484i64 - i as i64).to_string();
                fields[6] = "42".into();
                fields.join(":")
            })
            .collect::<Vec<_>>()
            .join(";");
        row["detail"] = json!(format!("{prefix} catalog={records} titles={suffix}"));
    }
    let r = check(&trace, &steps).unwrap();
    assert!(r.qualification_positive22);
    assert_eq!(r.catalog[0].hash, -1748467484);
    assert_eq!(r.catalog[21].capital, 42);
}

#[test]
fn duplicate_runtime_index_and_source_catalog_receipts_are_rejected() {
    let (trace, steps) = model(Path::new("unit-model"));
    for field in [0, 3] {
        let mut bad = steps.clone();
        for row in &mut bad {
            if row["status"] != "OK" {
                continue;
            }
            let detail = row["detail"].as_str().unwrap();
            if !detail.contains("CATALOG880 STABLE turn=") {
                continue;
            }
            let (prefix, rest) = detail.split_once(" catalog=").unwrap();
            let (entries, suffix) = rest.split_once(" titles=").unwrap();
            let mut records: Vec<_> = entries.split(';').map(str::to_owned).collect();
            let mut fields: Vec<_> = records[1].split(':').map(str::to_owned).collect();
            fields[field] = if field == 0 { "0" } else { "17" }.into();
            records[1] = fields.join(":");
            row["detail"] = json!(format!(
                "{prefix} catalog={} titles={suffix}",
                records.join(";")
            ));
        }
        rejected(&trace, &bad);
    }
}

#[test]
fn false_denial_flag_still_requires_matching_owner_and_index() {
    let (trace, steps) = model(Path::new("unit-model"));
    for checked in [false, true] {
        for row in [27, 28, 29] {
            for (field, value) in [("denial_owner", json!(2)), ("denial_index", json!(18))] {
                let mut bad = trace.clone();
                bad[row]["denial_checked"] = json!(checked);
                bad[row][field] = value;
                assert!(
                    check(&bad, &steps).is_err(),
                    "{row} {field} checked={checked}"
                );
            }
        }
    }
}

#[test]
fn absent_or_zero_sentinel_metadata_never_supplies_positive_property_evidence() {
    let (trace, steps) = model(Path::new("unit-model"));
    for row in [27, 28, 29] {
        for absent in [false, true] {
            let mut unknown = trace.clone();
            if absent {
                for field in ["denial_owner", "denial_index", "denial_checked"] {
                    unknown[row].as_object_mut().unwrap().remove(field);
                }
            } else {
                unknown[row]["denial_owner"] = json!(0);
                unknown[row]["denial_index"] = json!(0);
                unknown[row]["denial_checked"] = json!(false);
            }
            let r = check(&unknown, &steps).unwrap();
            assert!(r.capture_valid);
            assert!(!r.qualification_positive22);
            assert_eq!(r.missing_positive_sources, vec![0]);
            assert_eq!(r.property_subject_unknown_rows, 1);
        }
    }
}

#[test]
fn auxiliary_startup_failure_returns_json_exit_one_without_panic() {
    for operation in ["launch", "auto_restart"] {
        let dir = tempfile::tempdir().unwrap();
        let (trace, mut steps) = model(dir.path());
        steps.truncate(1);
        for status in ["START", "FAIL"] {
            steps.push(json!({"at":"2026-10-06T00:00:00Z","action":operation,"status":status,"detail":"synthetic startup failure"}));
        }
        let t = dir.path().join("normal-catalog22.jsonl");
        let s = dir.path().join("steps.jsonl");
        std::fs::write(&t, lines(&trace)).unwrap();
        std::fs::write(&s, lines(&steps)).unwrap();
        let result = std::panic::catch_unwind(|| catalog_check::check_files(&t, &s));
        let error = result.expect("startup failure must not panic").unwrap_err();
        assert!(error.to_string().contains("startup failed"));
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_civ6-catalog-check"))
            .args(["check"])
            .arg(&t)
            .arg(&s)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let report: Value = serde_json::from_slice(&output.stdout).expect("CLI must emit JSON");
        assert_eq!(report["capture_valid"], false);
        assert_eq!(report["qualification_positive22"], false);
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
}

#[test]
fn immutable_contract_and_receipts_have_independent_pinned_hashes() {
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "fixtures/catalog-check/source-contract.json"
            ))
        ),
        "2ce6c4df27af3f82a9fd4f9f8d4bce52ddfba604241b4e17a1258d8d80d0f3f2"
    );
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "fixtures/catalog-check/runtime-receipts.json"
            ))
        ),
        "b3ab6a0fcda4ebcd1c0d241c7c6abbad6d560fa31abc0b86035b9183d70de7d0"
    );
}

fn nil_adjusted_model(directory: &Path) -> (Vec<Value>, Vec<Value>) {
    let (trace, mut steps) = model(directory);
    let contract: Value = serde_json::from_str(include_str!(
        "fixtures/catalog-check/source-contract-nil-adjusted-068.json"
    ))
    .unwrap();
    for row in &mut steps {
        if row["status"] == "START" {
            let action = row["action"].as_str().unwrap();
            let n: usize = action.split_once('/').unwrap().0.parse().unwrap();
            row["detail"] = json!(contract["steps"][n - 1].to_string());
        }
    }
    (trace, steps)
}

#[test]
fn frozen_nil_adjusted_success_model_cli_zero_never_product_acceptance() {
    let dir = tempfile::tempdir().unwrap();
    let (trace, steps) = nil_adjusted_model(dir.path());
    let r = check(&trace, &steps).unwrap();
    assert_eq!(
        r.source_contract_version,
        catalog_check::SourceContractVersion::NilAdjusted068
    );
    assert_eq!(
        r.scenario_contract_sha256,
        "cbebf7fe0cb633e8d541205f667762b637c4cadd3ca19e90337a9738bec67e42"
    );
    assert!(r.scenario_complete && r.capture_valid && r.qualification_positive22);
    assert!(!r.product_acceptance && !r.payment_authority);
    let t = dir.path().join("normal-catalog22.jsonl");
    let s = dir.path().join("steps.jsonl");
    std::fs::write(&t, lines(&trace)).unwrap();
    std::fs::write(&s, lines(&steps)).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_civ6-catalog-check"))
        .arg("check")
        .arg(&t)
        .arg(&s)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["source_contract_version"], "NilAdjusted068");
    assert_eq!(report["qualification_positive22"], true);
    assert_eq!(report["product_acceptance"], false);
}

#[test]
fn frozen_legacy_success_and_partial_models_keep_their_version() {
    let dir = tempfile::tempdir().unwrap();
    let (mut trace, mut steps) = model(dir.path());
    let r = check(&trace, &steps).unwrap();
    assert_eq!(
        r.source_contract_version,
        catalog_check::SourceContractVersion::Legacy4cd
    );
    assert_eq!(
        r.scenario_contract_sha256,
        "2ce6c4df27af3f82a9fd4f9f8d4bce52ddfba604241b4e17a1258d8d80d0f3f2"
    );
    // Complete native restoration but Lua fails before the first UI native read.
    steps.truncate(23);
    steps[22]["status"] = json!("FAIL");
    steps[22]["detail"] = json!("legacy zero-return tostring error");
    trace.drain(3..25);
    let stop = trace.last_mut().unwrap();
    stop["rows"] = json!(110);
    stop["observed_calls"] = json!(110);
    stop["getter_calls_seen"] = json!(120);
    let t = dir.path().join("normal-catalog22.jsonl");
    let s = dir.path().join("steps.jsonl");
    std::fs::write(&t, lines(&trace)).unwrap();
    std::fs::write(&s, lines(&steps)).unwrap();
    let r = catalog_check::check_files(&t, &s).unwrap();
    assert_eq!(
        r.source_contract_version,
        catalog_check::SourceContractVersion::Legacy4cd
    );
    assert!(r.capture_valid && !r.qualification_positive22);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_civ6-catalog-check"))
        .arg("check")
        .arg(&t)
        .arg(&s)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["capture_valid"], true);
    assert_eq!(report["qualification_positive22"], false);
}

#[test]
fn mixed_versions_at_every_distinct_step_are_rejected_in_both_directions() {
    let (trace, legacy) = model(Path::new("unit-model"));
    let (_, adjusted) = nil_adjusted_model(Path::new("unit-model"));
    let mut mutations = 0;
    for i in 0..legacy.len() {
        if legacy[i]["status"] == "START" && legacy[i]["detail"] != adjusted[i]["detail"] {
            let mut mixed = legacy.clone();
            mixed[i] = adjusted[i].clone();
            assert!(
                catalog_check::evidence_from_steps(&lines(&mixed)).is_err(),
                "legacy step {i}"
            );
            let mut mixed = adjusted.clone();
            mixed[i] = legacy[i].clone();
            assert!(
                catalog_check::evidence_from_steps(&lines(&mixed)).is_err(),
                "adjusted step {i}"
            );
            mutations += 1;
        }
    }
    assert_eq!(mutations, 34);
    // Choosing the opposite first code cannot be undone by subsequent codes.
    assert!(check(&trace, &adjusted).unwrap().qualification_positive22);
}

#[test]
fn unknown_future_header_code_is_not_admitted_by_current_generator() {
    for adjusted in [false, true] {
        let (_, mut steps) = if adjusted {
            nil_adjusted_model(Path::new("unit-model"))
        } else {
            model(Path::new("unit-model"))
        };
        let first = steps
            .iter_mut()
            .find(|r| r["status"] == "START" && r["action"].as_str().unwrap().ends_with(" ui_lua"))
            .unwrap();
        let mut source: Value = serde_json::from_str(first["detail"].as_str().unwrap()).unwrap();
        source["code"] = json!(format!(
            "{}\n-- unknown future contract",
            source["code"].as_str().unwrap()
        ));
        first["detail"] = json!(source.to_string());
        assert!(catalog_check::evidence_from_steps(&lines(&steps)).is_err());
    }
}

#[test]
fn nil_adjusted_contract_is_a_fixed_three_property_only_header_change() {
    use sha2::{Digest, Sha256};
    let old: Value =
        serde_json::from_str(include_str!("fixtures/catalog-check/source-contract.json")).unwrap();
    let new: Value = serde_json::from_str(include_str!(
        "fixtures/catalog-check/source-contract-nil-adjusted-068.json"
    ))
    .unwrap();
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "fixtures/catalog-check/source-contract-nil-adjusted-068.json"
            ))
        ),
        "cbebf7fe0cb633e8d541205f667762b637c4cadd3ca19e90337a9738bec67e42"
    );
    assert_eq!(old["name"], new["name"]);
    let a = old["steps"].as_array().unwrap();
    let b = new["steps"].as_array().unwrap();
    assert_eq!(a.len(), b.len());
    let mut changed = 0;
    for (a, b) in a.iter().zip(b) {
        let mut expected = a.clone();
        if let Some(code) = a["code"].as_str() {
            // Test-only explicit known patch, never acceptance normalization.
            let old_line=" return identity..':'..tostring(Game:GetProperty(prefix..'STATE'))..':'..tostring(Game:GetProperty(prefix..'UNIT'))..':'..tostring(Game:GetProperty(prefix..'EVOLUTION_SURCHARGE'))..':'..ownerGov:GetGovernorPoints()..':'..ownerGov:GetGovernorPointsSpent(),positives,negatives";
            let new_line=" -- Civ6 returns zero Lua values for an absent property. Parentheses adjust\n -- that to one nil only for display; the ledger assertions above stay strict.\n return identity..':'..tostring((Game:GetProperty(prefix..'STATE')))..':'..tostring((Game:GetProperty(prefix..'UNIT')))..':'..tostring((Game:GetProperty(prefix..'EVOLUTION_SURCHARGE')))..':'..ownerGov:GetGovernorPoints()..':'..ownerGov:GetGovernorPointsSpent(),positives,negatives";
            if code.contains(old_line) {
                assert_eq!(code.matches(old_line).count(), 1);
                expected["code"] = json!(code.replace(old_line, new_line));
                changed += 1;
            }
        }
        assert_eq!(expected, *b);
    }
    assert_eq!(changed, 34);
}

#[test]
fn nil_adjusted_shipping_restart_keeps_version_binding_before_and_after_native_start() {
    let (trace, mut steps) = nil_adjusted_model(Path::new("unit-model"));
    for row in &mut steps {
        let a = row["action"].as_str().unwrap();
        if let Some((n, operation)) = a.split_once(' ') {
            let (n, _) = n.split_once('/').unwrap();
            row["action"] = json!(format!(
                "{}/44 {operation}",
                n.parse::<usize>().unwrap() + 1
            ));
        }
    }
    steps.insert(1,json!({"at":"2026-10-06T00:00:00Z","action":"1/44 restart_game","status":"START","detail":"{\"action\":\"restart_game\",\"timeout\":420}"}));
    steps.insert(2,json!({"at":"2026-10-06T00:00:00Z","action":"1/44 restart_game","status":"OK","detail":"synthetic restart model only"}));
    let r = check(&trace, &steps).unwrap();
    assert_eq!(
        r.source_contract_version,
        catalog_check::SourceContractVersion::NilAdjusted068
    );
    assert!(r.qualification_positive22);
    let old: Value =
        serde_json::from_str(include_str!("fixtures/catalog-check/source-contract.json")).unwrap();
    for original_index in [6, 37] {
        let mut mixed = steps.clone();
        let row = mixed
            .iter_mut()
            .find(|r| {
                r["status"] == "START"
                    && r["action"]
                        .as_str()
                        .unwrap()
                        .starts_with(&format!("{}/44 ", original_index + 2))
            })
            .unwrap();
        row["detail"] = json!(old["steps"][original_index].to_string());
        assert!(catalog_check::evidence_from_steps(&lines(&mixed)).is_err());
    }
}

// Hybrid unit model: the five raw Ainu property rows and first positive native
// row come from a frozen subset. Resequencing below is ONLY for this unit model.
// The subset itself retains original sequences and cannot be a complete capture.
fn zero_subject_model(directory: &Path) -> (Vec<Value>, Vec<Value>) {
    let (mut trace, mut steps) = nil_adjusted_model(directory);
    let subset: Value = serde_json::from_str(include_str!(
        "fixtures/catalog-check/ainu-zero-subject-subset.json"
    ))
    .unwrap();
    for row in &mut steps {
        if row["status"] != "OK" {
            continue;
        }
        let detail = row["detail"].as_str().unwrap().to_owned();
        let adjusted = if let Some((prefix, rest)) = detail.split_once(" catalog=") {
            let (records, titles) = rest.split_once(" titles=").unwrap();
            let mut records: Vec<_> = records.split(';').map(str::to_owned).collect();
            let mut fields: Vec<_> = records[21].split(':').map(str::to_owned).collect();
            fields[3] = "0".into();
            records[21] = fields.join(":");
            format!("{prefix} catalog={} titles={titles}", records.join(";"))
        } else {
            detail.replace("21:0:164:", "21:0:0:")
        };
        row["detail"] = json!(adjusted);
    }
    trace[24] = subset["calibration"][0].clone();
    trace[24]["sequence"] = json!(22);
    for (offset, raw) in subset["properties"].as_array().unwrap().iter().enumerate() {
        trace[130 + offset] = raw.clone();
        trace[130 + offset]["sequence"] = json!(106 + offset);
    }
    (trace, steps)
}

#[test]
fn zero_subject_corroboration_uses_exact_reset_calibration_and_catalog_identity() {
    let (trace, steps) = zero_subject_model(Path::new("unit-model"));
    let r = check(&trace, &steps).unwrap();
    assert!(r.capture_valid && r.qualification_positive22);
    assert_eq!(r.property_zero_subject_corroborated_rows, 1);
    assert_eq!(r.property_subject_unknown_rows, 0);
    let p = r
        .pairs
        .iter()
        .find(|p| p.source == 21 && p.observed_owner == 0)
        .unwrap();
    assert_eq!(p.governor_index, 0);
    assert_eq!(p.original_zero_effective_one, 1);
    assert_eq!(p.core_allow_true_rows, 1);
    assert!(
        !r.full_history_authority
            && !r.cross_ring_chronology_measured
            && !r.payment_authority
            && !r.product_acceptance
    );
}

#[test]
fn zero_subject_with_missing_reset_foreign_key_missing_metadata_or_denial_is_rejected() {
    let (trace, steps) = zero_subject_model(Path::new("unit-model"));
    for (row, field, value) in [
        (130, "key_id", json!(903000)),
        (130, "key_id", json!(902001)),
        (130, "payload8", json!("0x0000000000000000")),
        (134, "key_id", json!(901001)),
        (134, "denial_owner", json!(1)),
        (134, "denial_index", json!(1)),
        (134, "property", json!("0x0000000000011528")),
        (134, "thread_id", json!(99)),
        (134, "denial_checked", json!(true)),
        (24, "effective_al", json!(0)),
    ] {
        let mut bad = trace.clone();
        bad[row][field] = value;
        rejected(&bad, &steps);
    }
    let mut bad = trace.clone();
    for field in ["denial_owner", "denial_index", "denial_checked"] {
        bad[134].as_object_mut().unwrap().remove(field);
    }
    let r = check(&bad, &steps).unwrap();
    assert!(!r.qualification_positive22);
    assert_eq!(r.missing_positive_sources, vec![21]);
    // Same reset moved after the allow row has no preceding binding.
    let mut bad = trace.clone();
    let mut reset = bad.remove(130);
    reset["sequence"] = json!(110);
    bad.insert(134, reset);
    for (i, row) in bad[130..135].iter_mut().enumerate() {
        row["sequence"] = json!(106 + i);
    }
    rejected(&bad, &steps);
}

#[test]
fn zero_subject_requires_unique_native_and_property_owner_receivers() {
    let (trace, steps) = zero_subject_model(Path::new("unit-model"));
    let mut bad = trace.clone();
    bad[24]["governors_this"] = bad[3]["governors_this"].clone();
    assert!(check(&bad, &steps).is_err());
    let mut bad = trace.clone();
    bad[24]["player_address"] = bad[3]["player_address"].clone();
    bad[24]["property_receiver"] = bad[3]["property_receiver"].clone();
    assert!(check(&bad, &steps).is_err());
    // No native receiver identifies this replacement reset; duplicate owner0
    // would otherwise allow a second stale alias to enter the property table.
    let mut bad = trace.clone();
    bad[30]["key_id"] = json!(902000);
    bad[30]["property"] = json!("0x00000000feed0528");
    assert!(check(&bad, &steps).is_err());
}

#[test]
fn frozen_ainu_subset_preserves_original_sequences_and_is_not_a_full_capture() {
    use sha2::{Digest, Sha256};
    let bytes = include_bytes!("fixtures/catalog-check/ainu-zero-subject-subset.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "62c61902dcefa42a4a33e76e019dbc5fffae7253e1e5d14a60610132d87125bc"
    );
    let v: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(v["calibration"][0]["sequence"], 87);
    assert_eq!(v["properties"][0]["sequence"], 1);
    assert_eq!(v["properties"][4]["sequence"], 2992);
    assert_eq!(v["calibration"].as_array().unwrap().len(), 4);
    assert_eq!(v["properties"].as_array().unwrap().len(), 5);
    assert!(v["scope"]
        .as_str()
        .unwrap()
        .contains("not a complete capture"));
}

// SYNTHETIC 851 unit model: non-local AI owners, slot 1 owns sources 0 and 21.
// Receipts come from this independent table, never from the checker or generator.
const NORMAL_AI: &str = include_str!("fixtures/catalog-check/source-contract-normal-ai-851.json");
const NORMAL_AI_SHA256: &str = "16b5dec69852d0713828273a0c009f1c66ae0cb972308279c430d25ecd2eaeea";
const AI: &str = "CATALOG_NORMAL_AI_V1";
const PROCESS: &str = "{\"pid\":7,\"created_filetime\":123}";

fn ai_owner(s: usize) -> usize {
    if s == 21 {
        1
    } else {
        s + 1
    }
}
fn ai_cost(s: usize) -> u32 {
    (s % 3 + 1) as u32
}
fn ai_titles() -> [u32; 22] {
    let mut titles = [0; 22];
    for s in 0..22 {
        titles[ai_owner(s)] += ai_cost(s);
    }
    titles
}
fn normal_ai_model(directory: &Path) -> (Vec<Value>, Vec<Value>) {
    normal_ai_model_with(directory, ai_titles())
}
fn normal_ai_model_with(directory: &Path, titles: [u32; 22]) -> (Vec<Value>, Vec<Value>) {
    let (legacy_trace, _) = model(directory);
    let scenario: Value = serde_json::from_str(NORMAL_AI).unwrap();
    let steps = scenario["steps"].as_array().unwrap();
    let players = steps[2]["players"].as_array().unwrap();
    let leader = |s: usize| players[s]["leader"].as_str().unwrap();
    let index = |s: usize| 17 + s * 7;
    let id = |s: usize| {
        format!(
            "{s}:{}:{}:{}:{}",
            ai_owner(s),
            index(s),
            leader(s),
            ai_cost(s)
        )
    };
    let ledger = |sources: std::ops::Range<usize>, phase: &str| {
        sources
            .map(|s| {
                let state = match phase {
                    "closed" => "nil:nil:nil".to_owned(),
                    "JOINED" => format!("GREAT_PERSON:{}:nil", 100 + s),
                    _ => format!("EVOLVING:{}:{}", 100 + s, ai_cost(s) - 1),
                };
                format!("{}:{state}:{}:0", id(s), titles[ai_owner(s)])
            })
            .collect::<Vec<_>>()
            .join(";")
    };
    let stable = format!(
        "{AI} STABLE turn=1 catalog={} titles={}",
        (0..22)
            .map(|s| format!(
                "{s}:{}:{}:{}:{}:{}:42",
                leader(s),
                players[s]["civ"].as_str().unwrap(),
                index(s),
                -1748467484i64 - s as i64,
                ai_cost(s)
            ))
            .collect::<Vec<_>>()
            .join(";"),
        (0..22)
            .map(|o| format!("{o}:{}:0", titles[o]))
            .collect::<Vec<_>>()
            .join(";"),
    );
    let raw = |marker: &str| {
        let rows: Vec<_> = (0..22)
            .map(|s| {
                let bits: String = (0..22)
                    .map(|o| if o == ai_owner(s) { '1' } else { '0' })
                    .collect();
                format!("{AI} {marker} {} positive=1 negative=21 raw={bits}", id(s))
            })
            .collect();
        format!("positive=22 negative=462 {}", rows.join(";"))
    };
    let record = |action: String, status: &str, detail: String| json!({"at":"2026-10-07T00:00:00Z","action":action,"status":status,"detail":detail});
    let mut log = vec![record(
        "start".into(),
        "OK",
        scenario["name"].as_str().unwrap().into(),
    )];
    for (i, step) in steps.iter().enumerate() {
        let action = format!("{}/43 {}", i + 1, step["action"].as_str().unwrap());
        log.push(record(action.clone(), "START", step.to_string()));
        let detail = match step["action"].as_str().unwrap() {
            "start_native_production_admission" => format!("native production-admission original-return telemetry ready; artifact normal-ai-observer-v1.jsonl; process={PROCESS}"),
            "stop_native_production_admission" => format!("native entry restored; artifact {}", directory.join("normal-ai-observer-v1.jsonl").display()),
            "exit_to_menu" => "main menu reached".into(),
            "ui_lua" => {
                let key = step["capture_as"].as_str().or_else(|| step["equals_capture"].as_str());
                let value = match key.map(|k| k.strip_prefix("CATALOG_NORMAL_AI_V1_").unwrap()) {
                    Some("STABLE") => stable.clone(),
                    Some("CLOSED") => format!("{AI} MATRIX identities=22 {}", ledger(0..22, "closed")),
                    Some("FINAL") => format!("{AI} MATRIX identities=22 {}", ledger(0..22, "EVOLVING")),
                    Some(batch) => {
                        let (first, phase) = batch.split_once('_').unwrap();
                        let first: usize = first.parse().unwrap();
                        let phase = if phase == "EVOLVE" { "EVOLVING" } else { "JOINED" };
                        format!("{AI} {phase} {}", ledger(first..(first + 4).min(22), phase))
                    }
                    None if i == 5 => format!("{AI} normal observer: official AI title budgets; local human owns no subject; UI callbacks unchanged; no game turn requested"),
                    None if i == 9 => "PAYMENT880 observed 484 existing closed Core qualifications after observer installation".into(),
                    None => raw(if step["state_name"] == "InGame" { "UI_NATIVE_QUALIFICATION" } else { "GAMEPLAY_PROJECTION" }),
                };
                // The 851 runner names the native process on receipts after start.
                let suffix = if (9..41).contains(&i) { format!("; native_process={PROCESS}") } else { String::new() };
                format!("ran Lua in {}; return={value}{suffix}", step["state_name"].as_str().unwrap())
            }
            _ => "synthetic unit model setup result; not live evidence".into(),
        };
        log.push(record(action, "OK", detail));
    }
    log.push(record("judge".into(), "OK", "6/6 checks passed".into()));
    let mut trace = legacy_trace[..3].to_vec();
    for s in 0..22 {
        let owner = ai_owner(s);
        let address = 0x10000 + owner * 0x1000;
        trace.push(json!({"kind":"calibration","sequence":s+1,"governor_index":index(s),"governors_this":format!("0x{:016x}",0x90000+owner*0x1000),"return_site_rva":0xba523,"thread_id":11,"owner_read_state":1,"player_address":format!("0x{address:016x}"),"player_id":owner,"property_receiver":format!("0x{:016x}",address+0x528),"original_al":s%2,"effective_al":1}));
    }
    let mut sequence = 0;
    for s in 0..22 {
        let owner = ai_owner(s);
        let receiver = 0x10528 + owner * 0x1000;
        for (key, payload) in [
            (902000 + owner, 1),
            (903000 + index(s), 1),
            (901000 + index(s), 0),
            (900000 + index(s), 1),
            (901000 + index(s), 1),
        ] {
            sequence += 1;
            let cell = (900000..=900255).contains(&key) || (901000..=901255).contains(&key);
            trace.push(json!({"kind":"property_get","sequence":sequence,"property":format!("0x{receiver:016x}"),"key_id":key,"returned":"0x0000000000800000","thread_id":12,"read_state":1,"tag":1,"payload8":format!("0x{payload:016x}"),"flags14":0,
                "denial_owner":if cell {owner}else{0},"denial_index":if cell {index(s)}else{0},"denial_checked":false}));
        }
    }
    trace.push(legacy_trace.last().unwrap().clone());
    (trace, log)
}
/// Replaces text in every successful receipt (never in START sources).
fn edit_receipts(steps: &[Value], from: &str, to: &str) -> Vec<Value> {
    let mut steps = steps.to_vec();
    for row in &mut steps {
        if row["status"] == "OK" && row["action"] != "start" {
            row["detail"] = json!(row["detail"].as_str().unwrap().replace(from, to));
        }
    }
    steps
}

#[test]
fn normal_ai_851_contract_is_byte_pinned_and_assigns_no_local_subject() {
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!("{:x}", Sha256::digest(NORMAL_AI.as_bytes())),
        NORMAL_AI_SHA256
    );
    let v: Value = serde_json::from_str(NORMAL_AI).unwrap();
    let legacy: Value =
        serde_json::from_str(include_str!("fixtures/catalog-check/source-contract.json")).unwrap();
    assert_ne!(v["name"], legacy["name"]);
    let steps = v["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 43);
    assert_eq!(steps[8]["name"], "normal-ai-observer-v1");
    let players = steps[2]["players"].as_array().unwrap();
    assert_eq!(players[0]["human"], true);
    assert!(players[1..].iter().all(|p| p["human"] == false));
    assert_eq!(ai_titles()[0], 0);
}

#[test]
fn frozen_normal_ai_851_qualifies_shared_owner_one_and_never_local_human() {
    let dir = tempfile::tempdir().unwrap();
    let (trace, steps) = normal_ai_model(dir.path());
    let r = check(&trace, &steps).unwrap();
    assert_eq!(
        r.source_contract_version,
        catalog_check::SourceContractVersion::NormalAi851
    );
    assert_eq!(r.scenario_contract_sha256, NORMAL_AI_SHA256);
    assert!(r.scenario_complete && r.capture_valid && r.qualification_positive22);
    assert!(r.gameplay_closed484);
    assert!(!r.product_acceptance && !r.payment_authority && !r.full_history_authority);
    let owners: Vec<_> = r.catalog.iter().map(|e| e.owner).collect();
    assert_eq!((owners[0], owners[21]), (1, 1));
    assert_eq!(owners.iter().filter(|o| **o == 1).count(), 2);
    assert!(!owners.contains(&0));
    assert_eq!(
        (
            r.ui_native_qualification.positive,
            r.ui_native_qualification.negative
        ),
        (22, 462)
    );
    let p = r
        .pairs
        .iter()
        .find(|p| p.source == 21 && p.observed_owner == 1)
        .unwrap();
    assert_eq!((p.rows, p.core_allow_true_rows), (1, 1));
    let t = dir.path().join("normal-ai-observer-v1.jsonl");
    let s = dir.path().join("steps.jsonl");
    std::fs::write(&t, lines(&trace)).unwrap();
    std::fs::write(&s, lines(&steps)).unwrap();
    assert!(
        catalog_check::check_files(&t, &s)
            .unwrap()
            .qualification_positive22
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_civ6-catalog-check"))
        .arg("check")
        .arg(&t)
        .arg(&s)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["source_contract_version"], "NormalAi851");
    assert_eq!(report["product_acceptance"], false);
    // The 851 start/stop receipts name their own artifact, not the legacy one.
    let legacy = dir.path().join("normal-catalog22.jsonl");
    std::fs::write(&legacy, lines(&trace)).unwrap();
    assert!(catalog_check::check_files(&legacy, &s).is_err());
}

#[test]
fn normal_ai_unknown_name_mixed_versions_edits_and_insertions_are_rejected() {
    let (_, steps) = normal_ai_model(Path::new("unit-model"));
    let parse = |s: &[Value]| catalog_check::evidence_from_steps(&lines(s));
    assert!(parse(&steps).is_ok());
    let legacy: Value =
        serde_json::from_str(include_str!("fixtures/catalog-check/source-contract.json")).unwrap();
    let nil: Value = serde_json::from_str(include_str!(
        "fixtures/catalog-check/source-contract-nil-adjusted-068.json"
    ))
    .unwrap();
    for name in [
        json!("Unknown future normal catalog"),
        legacy["name"].clone(),
    ] {
        let mut bad = steps.clone();
        bad[0]["detail"] = name;
        assert!(parse(&bad).is_err());
    }
    let mut mutations = 0;
    for other in [&legacy, &nil] {
        for (i, row) in steps.iter().enumerate() {
            if row["status"] != "START" {
                continue;
            }
            let n: usize = row["action"]
                .as_str()
                .unwrap()
                .split_once('/')
                .unwrap()
                .0
                .parse()
                .unwrap();
            let own: Value = serde_json::from_str(row["detail"].as_str().unwrap()).unwrap();
            if own != other["steps"][n - 1] {
                let mut bad = steps.clone();
                bad[i]["detail"] = json!(other["steps"][n - 1].to_string());
                assert!(parse(&bad).is_err(), "851 step {n}");
                mutations += 1;
            }
        }
    }
    // At least the 34 ledger-printing Lua codes differ from each older wire.
    assert!(mutations >= 2 * 34, "{mutations}");
    // The reverse: a legacy-named run cannot adopt one 851 source.
    let (_, old) = model(Path::new("unit-model"));
    let normal: Value = serde_json::from_str(NORMAL_AI).unwrap();
    for n in [6, 9] {
        let mut bad = old.clone();
        let row = bad
            .iter_mut()
            .find(|r| {
                r["status"] == "START"
                    && r["action"]
                        .as_str()
                        .unwrap()
                        .starts_with(&format!("{n}/43 "))
            })
            .unwrap();
        row["detail"] = json!(normal["steps"][n - 1].to_string());
        assert!(parse(&bad).is_err(), "legacy step {n}");
    }
    let at = steps
        .iter()
        .position(|r| r["status"] == "START" && r["action"] == "20/43 ui_lua")
        .unwrap();
    let mut edited = steps.clone();
    let mut source: Value = serde_json::from_str(edited[at]["detail"].as_str().unwrap()).unwrap();
    source["code"] = json!(format!("{}\n-- edited", source["code"].as_str().unwrap()));
    edited[at]["detail"] = json!(source.to_string());
    assert!(parse(&edited).is_err());
    // Hidden turn, payment or a repeated operation inserted mid-run.
    let payment = json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":"Players[1]:GetGovernors():ChangeGovernorPoints(-1)"});
    for (op, source) in [
        ("end_turn", json!({"action":"end_turn"})),
        ("ui_lua", payment),
    ] {
        let mut bad = steps.clone();
        let label = format!("20/43 {op}");
        bad.insert(
            at,
            json!({"at":"2026-10-07T00:00:00Z","action":label,"status":"OK","detail":"synthetic"}),
        );
        bad.insert(at, json!({"at":"2026-10-07T00:00:00Z","action":label,"status":"START","detail":source.to_string()}));
        assert!(parse(&bad).is_err(), "{op}");
    }
    let mut bad = steps.clone();
    bad.insert(at, steps[at + 1].clone());
    bad.insert(at, steps[at].clone());
    assert!(parse(&bad).is_err());
}

#[test]
fn normal_ai_cyclic_human_zero_or_edited_owner_is_rejected() {
    let (trace, steps) = normal_ai_model(Path::new("unit-model"));
    // The legacy ring would hand source 21 to local human slot 0.
    rejected(&trace, &edit_receipts(&steps, "21:1:164:", "21:0:164:"));
    rejected(&trace, &edit_receipts(&steps, "0:1:17:", "0:2:17:"));
    // A native positive for slot 0 instead of the frozen owner 1.
    let mut bad = trace.clone();
    bad[24]["player_id"] = json!(0);
    bad[24]["player_address"] = json!("0x0000000000010000");
    bad[24]["property_receiver"] = json!("0x0000000000010528");
    bad[24]["governors_this"] = json!("0x0000000000090000");
    let r = check(&bad, &steps).unwrap();
    assert!(!r.qualification_positive22);
    assert_eq!(r.missing_positive_sources, vec![21]);
    assert!(!r.qualification_errors.is_empty());
}

#[test]
fn normal_ai_receipts_must_name_the_started_native_process() {
    let (trace, steps) = normal_ai_model(Path::new("unit-model"));
    let at = steps
        .iter()
        .position(|r| {
            r["status"] == "OK" && r["detail"].as_str().unwrap().contains("native_process=")
        })
        .unwrap();
    let suffix = format!("; native_process={PROCESS}");
    for (from, to) in [
        ("\"pid\":7", "\"pid\":8"),
        ("123}", "124}"),
        ("123}", "123,\"mode\":1}"),
        (suffix.as_str(), ""),
    ] {
        let mut bad = steps.clone();
        bad[at]["detail"] = json!(bad[at]["detail"].as_str().unwrap().replace(from, to));
        rejected(&trace, &bad);
    }
    // A receipt before native start cannot claim a process.
    let prepare = steps
        .iter()
        .position(|r| r["status"] == "OK" && r["action"] == "6/43 ui_lua")
        .unwrap();
    let mut bad = steps.clone();
    bad[prepare]["detail"] = json!(format!(
        "{}{suffix}",
        bad[prepare]["detail"].as_str().unwrap()
    ));
    rejected(&trace, &bad);
    // Consistently renamed receipts still disagree with the raw trace identity.
    rejected(&trace, &edit_receipts(&steps, "\"pid\":7,", "\"pid\":9,"));
}

#[test]
fn normal_ai_missing_native_positive_or_partial_capture_is_never_qualification() {
    let (mut trace, steps) = normal_ai_model(Path::new("unit-model"));
    trace[24]["effective_al"] = json!(0);
    let r = check(&trace, &steps).unwrap();
    assert!(r.capture_valid && !r.qualification_positive22);
    assert_eq!(r.missing_positive_sources, vec![21]);
    trace[24]["effective_al"] = json!(1);
    trace[29]["payload8"] = json!("0x0000000000000000");
    assert_eq!(
        check(&trace, &steps).unwrap().missing_positive_sources,
        vec![0]
    );
    let (trace, steps) = normal_ai_model(Path::new("unit-model"));
    rejected(&trace, &steps[..30]);
    let e = catalog_check::evidence_from_steps(&lines(&steps)).unwrap();
    assert!(catalog_check::verify(&lines(&trace[..trace.len() - 1]), &e).is_err());
}

#[test]
fn normal_ai_shared_owner_budget_and_unchanged_balances_are_enforced() {
    // Owner 1 can cover either person alone, but not both reservations at once.
    let mut short = ai_titles();
    assert_eq!(short[1], ai_cost(0) + ai_cost(21));
    short[1] = ai_cost(0).max(ai_cost(21));
    let (_, steps) = normal_ai_model_with(Path::new("unit-model"), short);
    let error = catalog_check::evidence_from_steps(&lines(&steps)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("recipient lacks official titles"),
        "{error}"
    );
    let (trace, steps) = normal_ai_model(Path::new("unit-model"));
    let joined = "0:1:17:LEADER_JAC_HOJO_UJITSUNA:1:GREAT_PERSON:100:nil:2:0";
    let evolved = "21:1:164:LEADER_JAC_AINU_TARIKONA:1:EVOLVING:121:0:2:0";
    for (from, to) in [
        // Hidden grant or spend after the stable budget read.
        (joined, joined.replace(":nil:2:0", ":nil:3:0")),
        (joined, joined.replace(":nil:2:0", ":nil:2:1")),
        // Reservation corrupted for the second person of owner 1.
        (evolved, evolved.replace(":121:0:", ":121:1:")),
    ] {
        assert!(lines(&steps).contains(from));
        rejected(&trace, &edit_receipts(&steps, from, &to));
    }
    // A final stable re-read with another turn or balance differs from capture.
    let last = steps
        .iter()
        .rposition(|r| {
            r["status"] == "OK" && r["detail"].as_str().unwrap().contains(" STABLE turn=")
        })
        .unwrap();
    for (from, to) in [
        ("STABLE turn=1", "STABLE turn=2"),
        ("titles=0:0:0;1:2:0", "titles=0:0:0;1:3:0"),
    ] {
        let mut bad = steps.clone();
        bad[last]["detail"] = json!(bad[last]["detail"].as_str().unwrap().replace(from, to));
        rejected(&trace, &bad);
    }
}

#[test]
fn normal_ai_shipping_restart_prefix_keeps_851_binding() {
    let (trace, mut steps) = normal_ai_model(Path::new("unit-model"));
    for row in &mut steps {
        let a = row["action"].as_str().unwrap();
        if let Some((n, operation)) = a.split_once(' ') {
            let (n, _) = n.split_once('/').unwrap();
            row["action"] = json!(format!(
                "{}/44 {operation}",
                n.parse::<usize>().unwrap() + 1
            ));
        }
    }
    steps.insert(1,json!({"at":"2026-10-07T00:00:00Z","action":"1/44 restart_game","status":"START","detail":"{\"action\":\"restart_game\",\"timeout\":420}"}));
    steps.insert(2,json!({"at":"2026-10-07T00:00:00Z","action":"1/44 restart_game","status":"OK","detail":"synthetic restart model only"}));
    let r = check(&trace, &steps).unwrap();
    assert_eq!(
        r.source_contract_version,
        catalog_check::SourceContractVersion::NormalAi851
    );
    assert!(r.qualification_positive22);
}
