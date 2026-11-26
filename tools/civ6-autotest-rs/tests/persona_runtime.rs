use civ6_autotest_rs::{native_bridge::validate_runtime_receipt, scenario::validate_scenario};
use serde_json::{json, Value};

fn receipt() -> Vec<Value> {
    vec![
        json!({"kind":"identity","mode":"paid-assignment-runtime","pid":7,"created_filetime":123,
            "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256,"bridge_sha256":"a".repeat(64),
            "observation_limit_seconds":30,"runtime_lifetime":"process_exit",
            "reload_hook_observed":false,"official_ai_payment_observed":false}),
        json!({"kind":"runtime_install","status":0,"three_entries_published":true,
            "trace_address":4096,"trace_owner":"game_process","bounded_uninstall_allowed":false}),
        json!({"kind":"observation_stop","status":0,"runtime_retained":true,
            "three_entries_published":true,"trace_retained":true}),
    ]
}
fn text(rows: &[Value]) -> String {
    rows.iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn runtime_receipt_rejects_restoration_replaced_identity_missing_or_trailing_rows() {
    let original = receipt();
    validate_runtime_receipt(&text(&original), 7, 123).unwrap();
    assert!(validate_runtime_receipt(&text(&original), 8, 123).is_err());
    assert!(validate_runtime_receipt(&text(&original), 7, 124).is_err());
    for (row, field, value) in [
        (0, "mode", json!("production-admission")),
        (0, "reload_hook_observed", json!(true)),
        (1, "trace_owner", json!("controller")),
        (1, "trace_address", json!(4097)),
        (1, "bounded_uninstall_allowed", json!(true)),
        (2, "runtime_retained", json!(false)),
        (2, "trace_retained", json!(false)),
    ] {
        let mut bad = original.clone();
        bad[row][field] = value;
        assert!(
            validate_runtime_receipt(&text(&bad), 7, 123).is_err(),
            "{field}"
        );
    }
    for row in 0..3 {
        let mut bad = original.clone();
        bad.remove(row);
        assert!(validate_runtime_receipt(&text(&bad), 7, 123).is_err());
    }
    let mut bad = original.clone();
    bad.push(json!({"kind":"uninstall","status":0}));
    assert!(validate_runtime_receipt(&text(&bad), 7, 123).is_err());
}
fn start() -> Value {
    json!({"action":"start_native_paid_runtime","name":"runtime","max_seconds":30})
}
fn stop() -> Value {
    json!({"action":"stop_native_paid_runtime_observation"})
}
fn scenario(steps: Vec<Value>) -> Value {
    json!({"name":"resident runtime lifecycle","steps":steps})
}

#[test]
fn runtime_observation_stop_keeps_ownership_across_save_and_turn_but_not_unverified_load() {
    validate_scenario(&scenario(vec![
        json!({"action":"start_native_paid_runtime","name":"runtime-next","max_seconds":30}),
        stop(),
        json!({"action":"save","name":"runtime"}),
        json!({"action":"end_turn"}),
    ]))
    .unwrap();
    for action in [
        json!({"action":"load","name":"runtime"}),
        json!({"action":"exit_to_menu"}),
        start(),
        json!({"action":"start_native_assign_calibration","name":"other","max_seconds":30}),
        json!({"action":"start_native_governor_trace","name":"other","max_seconds":30}),
    ] {
        assert!(validate_scenario(&scenario(vec![start(), stop(), action])).is_err());
    }
    assert!(validate_scenario(&scenario(vec![
        start(),
        json!({"action":"stop_native_production_admission"})
    ]))
    .is_err());
    validate_scenario(&scenario(vec![
        start(),
        stop(),
        json!({"action":"restart_game"}),
        json!({"action":"start_native_paid_runtime","name":"runtime-after-restart","max_seconds":30}),
        stop(),
    ]))
    .unwrap();
}
