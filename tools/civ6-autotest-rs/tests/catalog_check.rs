//! SYNTHETIC UNIT MODELS ONLY. No fixture is an actual capture or product pass.
use civ6_autotest_rs::{catalog_check, catalog_scenario};
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
    let normal: Value = serde_json::from_str(catalog_scenario::NORMAL).unwrap();
    let scenario = catalog_scenario::build(&normal).unwrap();
    let steps = scenario["steps"].as_array().unwrap();
    let players = normal["steps"][1]["players"].as_array().unwrap();
    // Deliberately irregular indices: never use an implicit repository offset.
    let index = |s: usize| 17 + s * 7;
    let id = |s: usize| {
        format!(
            "{}:{}:{}:{}:{}",
            s,
            (s + 1) % 22,
            index(s),
            players[s]["leader"].as_str().unwrap(),
            s % 3 + 1
        )
    };
    let ledger = |s: usize, phase: &str| {
        format!(
            "{}:{}:{}:{}:3:0",
            id(s),
            match phase {
                "closed" => "nil",
                "JOIN" => "GREAT_PERSON",
                _ => "EVOLVING",
            },
            if phase == "closed" {
                "nil".into()
            } else {
                (100 + s).to_string()
            },
            if phase == "closed" || phase == "JOIN" {
                "nil".into()
            } else {
                (s % 3).to_string()
            }
        )
    };
    let stable = format!(
        "CATALOG880 STABLE turn=1 catalog={} titles={}",
        (0..22)
            .map(|s| format!(
                "{}:{}:{}:{}:{}:{}:{}",
                s,
                players[s]["leader"].as_str().unwrap(),
                players[s]["civ"].as_str().unwrap(),
                index(s),
                1000 + s,
                s % 3 + 1,
                200 + s
            ))
            .collect::<Vec<_>>()
            .join(";"),
        (0..22)
            .map(|s| format!("{s}:3:0"))
            .collect::<Vec<_>>()
            .join(";")
    );
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
                    Some("CATALOG880_CLOSED")=>format!("CATALOG880 MATRIX identities=22 {}",(0..22).map(|s|ledger(s,"closed")).collect::<Vec<_>>().join(";")),
                    Some("CATALOG880_FINAL")=>format!("CATALOG880 MATRIX identities=22 {}",(0..22).map(|s|ledger(s,"EVOLVE")).collect::<Vec<_>>().join(";")),
                    Some(key)=> {
                        let (s,phase)=key.strip_prefix("CATALOG880_").unwrap().split_once('_').unwrap();
                        let s:usize=s.parse().unwrap();
                        format!("CATALOG880 {} {}",if phase=="JOIN" {"JOINED"} else {"EVOLVING"},(s..(s+4).min(22)).map(|s|ledger(s,phase)).collect::<Vec<_>>().join(";"))
                    }
                    None if i==5=>"CATALOG880 official civics only; UI automatic submission paused; no game turn requested".into(),
                    None if i==9=>"PAYMENT880 observed 484 existing closed Core qualifications after observer installation".into(),
                    None=> {
                        let marker=if step["state_name"]=="InGame" {"UI_NATIVE_QUALIFICATION"} else {"GAMEPLAY_PROJECTION"};
                        format!("positive=22 negative=462 {}",(0..22).map(|s|format!("CATALOG880 {marker} {} positive=1 negative=21 raw={}",id(s),(0..22).map(|o|if o==(s+1)%22 {'1'}else{'0'}).collect::<String>())).collect::<Vec<_>>().join(";"))
                    }
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
            trace.push(json!({"kind":"property_get","sequence":sequence,"property":format!("0x{receiver:016x}"),"key_id":key,"returned":"0x0000000000800000","thread_id":12,"read_state":1,"tag":1,"payload8":format!("0x{payload:016x}"),"flags14":0}));
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
    let (mut trace, mut steps) = model(Path::new("unit-model"));
    for row in &mut trace {
        if row["kind"] == "property_get" {
            row["denial_owner"] = json!(0);
            row["denial_index"] = json!(0);
            row["denial_checked"] = json!(false);
        }
    }
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
