mod support_driver;

use civ6_autotest_rs::{metrics::TurnMetrics, runner::AutoplayOptions, ErrorKind};
use serde_json::json;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use support_driver::{FakeBackend, Harness};

#[test]
fn test_capture_and_compare_lua_results() {
    let mut replies = VecDeque::from(["22,56,7", "22,56,7", "21,55,8"]);
    let fake = FakeBackend::new().lua(move |_, _| Ok(replies.pop_front().unwrap().into()));
    let mut h = Harness::new(json!({}), fake);
    let step = json!({"action":"ui_lua","state_name":"InGame","code":"return 'state'","capture_as":"JAPAN_STATE"});
    h.runner.run_step(&step).unwrap();
    let compare = json!({"action":"ui_lua","state_name":"InGame","code":"return 'state'","equals_capture":"JAPAN_STATE"});
    h.runner.run_step(&compare).unwrap();
    assert_eq!(h.runner.lua_captures["JAPAN_STATE"], "22,56,7");
    assert!(h
        .runner
        .run_step(&compare)
        .unwrap_err()
        .to_string()
        .contains("snapshot mismatch"));
}

#[test]
fn test_turn_metrics_preserve_missing_samples() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("metrics.jsonl");
    let mut metrics = TurnMetrics::new(path.clone(), 1, 10.0, false);
    metrics
        .start(json!({"private_bytes":100,"working_set_bytes":80}))
        .unwrap();
    metrics
        .sample_at(2, json!({"private_bytes":110,"working_set_bytes":85}), 11.5)
        .unwrap();
    metrics
        .sample_at(4, json!({"private_bytes":120,"working_set_bytes":90}), 13.0)
        .unwrap();
    metrics
        .sample_at(4, json!({"private_bytes":120,"working_set_bytes":90}), 13.0)
        .unwrap();
    let rows: Vec<serde_json::Value> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(
        rows.iter()
            .map(|v| v["turn"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    assert_eq!(rows[1]["duration_seconds"], json!(1.5));
    assert!(rows[2]["duration_seconds"].is_null());
    assert!(rows[3]["duration_seconds"].is_null());
    assert_eq!((metrics.observed_turns, metrics.missed_turns), (2, 1));
}

#[test]
fn test_autoplay_writes_turn_and_memory_samples() {
    let mut statuses = VecDeque::from(["2|true|false|1|0", "3|false|true|0|0"]);
    let mut observed_turn = 1_i64;
    let fake = FakeBackend::new().lua(move |_, code| {
        if code.contains("tostring(AutoplayManager.IsActive())") {
            let status = statuses.pop_front().unwrap();
            observed_turn = status.split('|').next().unwrap().parse().unwrap();
            return Ok(status.into());
        }
        if code.contains("return tostring(startTurn)") {
            return Ok("1|0".into());
        }
        if code.contains("IsAlive()") {
            return Ok(format!("{observed_turn}|true"));
        }
        Ok("true".into())
    });
    let mut h = Harness::new(json!({}), fake);
    let detail = h
        .runner
        .autoplay(
            2,
            10.0,
            AutoplayOptions {
                record_metrics: true,
                ..Default::default()
            },
        )
        .unwrap();
    let rows = h.jsonl("autoplay-metrics-1.jsonl");
    assert_eq!(
        rows.iter()
            .map(|r| r["turn"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(rows[2]["memory"]["private_bytes"], 100);
    assert!(detail.contains("observed=2 missed=0"));
    assert_eq!(
        support_driver::object_keys(&rows[0]),
        [
            "at",
            "continuous_performance_valid",
            "elapsed_seconds",
            "memory",
            "observed",
            "phase",
            "test_mode",
            "turn"
        ]
    );
    assert_eq!(
        support_driver::object_keys(&rows[1]),
        [
            "at",
            "continuous_performance_valid",
            "duration_seconds",
            "elapsed_seconds",
            "memory",
            "observed",
            "phase",
            "test_mode",
            "turn"
        ]
    );
    let csv = std::fs::read_to_string(h.runner.output.join("autoplay-metrics-1.csv")).unwrap();
    assert_eq!(csv.lines().next().unwrap(), "phase,at,turn,observed,duration_seconds,elapsed_seconds,working_set_bytes,private_bytes,test_mode,continuous_performance_valid");
    h.runner.write_result().unwrap();
    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(h.runner.output.join("result.json")).unwrap())
            .unwrap();
    assert_eq!(
        support_driver::object_keys(&result),
        [
            "checks",
            "completed_steps",
            "failure",
            "metrics",
            "passed",
            "scenario",
            "test_mode"
        ]
    );
    assert_eq!(
        support_driver::object_keys(&result["metrics"]),
        ["autoplay", "groups", "save_load", "turn_summary"]
    );
    assert_eq!(
        result["metrics"]["autoplay"][0]["jsonl"],
        "autoplay-metrics-1.jsonl"
    );
    assert_eq!(
        result["metrics"]["autoplay"][0]["csv"],
        "autoplay-metrics-1.csv"
    );
    assert_eq!(result["metrics"]["autoplay"][0]["requested_turns"], 2);
    assert_eq!(result["metrics"]["autoplay"][0]["completed"], true);
    assert_eq!(
        support_driver::object_keys(&result["metrics"]["autoplay"][0]),
        [
            "completed",
            "csv",
            "duration_seconds",
            "group",
            "jsonl",
            "last_turn",
            "missed_turns",
            "observed_turns",
            "pid",
            "private_bytes",
            "requested_turns",
            "start_turn",
            "working_set_bytes"
        ]
    );
    assert_eq!(
        support_driver::object_keys(&result["metrics"]["turn_summary"]),
        [
            "duration_seconds",
            "missed_turns",
            "observed_turns",
            "private_bytes",
            "working_set_bytes"
        ]
    );
    assert_eq!(
        support_driver::object_keys(&result["metrics"]["save_load"]),
        ["csv", "load_seconds", "samples", "save_bytes"]
    );
}

#[test]
fn test_autoplay_reports_eliminated_human_instead_of_overshoot() {
    let fake = FakeBackend::new().lua(|_, code| {
        if code.contains("return tostring(startTurn)") {
            Ok("1|0".into())
        } else if code.contains("tostring(AutoplayManager.IsActive())") {
            Ok("52|false|true|0|-1".into())
        } else if code.contains("IsAlive()") {
            Ok("52|false".into())
        } else {
            Ok("true".into())
        }
    });
    let mut h = Harness::new(json!({}), fake);
    let err = h.runner.autoplay(50, 10.0, Default::default()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Scenario);
    assert!(err.to_string().contains("eliminated during autoplay"));
    assert!(!err.to_string().contains("overshot"));
}

#[test]
fn test_event_and_transient_inactive_state_do_not_complete_or_restart_early() {
    let statuses = Arc::new(Mutex::new(VecDeque::from([
        "1|true|true|3|0",
        "2|false|true|0|0",
        "3|false|true|0|0",
        "4|false|false|0|0",
        "4|false|true|0|0",
    ])));
    let mut observed_turn = 1_i64;
    let fake = FakeBackend::new().lua(move |_, code| {
        if code.contains("return tostring(startTurn)") {
            return Ok("1|0".into());
        }
        if code.contains("tostring(AutoplayManager.IsActive())") {
            let status = statuses.lock().unwrap().pop_front().unwrap();
            observed_turn = status.split('|').next().unwrap().parse().unwrap();
            return Ok(status.into());
        }
        if code.contains("IsAlive()") {
            return Ok(format!("{observed_turn}|true"));
        }
        Ok("true".into())
    });
    let mut h = Harness::new(json!({}), fake);
    let detail = h.runner.autoplay(3, 10.0, Default::default()).unwrap();
    assert!(detail.contains("1->4"));
    assert_eq!(h.count_code("SetActive(true)"), 1);
    assert_eq!(h.count_code("tostring(AutoplayManager.IsActive())"), 5);
}
