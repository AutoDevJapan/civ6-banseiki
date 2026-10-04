mod support_fixture;
use civ6_autotest_rs::scenario::validate_scenario;
use mlua::Lua;
use serde_json::json;
use support_fixture::{action, compile_lua, scenario, steps};

#[test]
fn assignment_stage_a_keeps_manual_appointment_and_observes_only_after_joined_restore() {
    let data = scenario("leader-persona-assign-calibration-15km.json");
    validate_scenario(&data).unwrap();
    assert_eq!(data["strict_lua_errors"], true);
    let all = steps(&data);
    let starts: Vec<_> = all
        .iter()
        .enumerate()
        .filter(|(_, s)| action(s, "start_native_assign_calibration"))
        .collect();
    let stops: Vec<_> = all
        .iter()
        .enumerate()
        .filter(|(_, s)| action(s, "stop_native_assign_calibration"))
        .collect();
    assert_eq!(starts.len(), 1);
    assert_eq!(stops.len(), 1);
    let (start, step) = starts[0];
    let (stop, _) = stops[0];
    assert_eq!(step["max_seconds"], 90);
    assert!(all[..start]
        .iter()
        .any(|s| action(s, "stop_native_joined_admission")));
    assert_eq!(
        all.iter()
            .filter(|s| s["code"]
                .as_str()
                .is_some_and(|c| c.contains("JAC_AssignLeader(")))
            .count(),
        1
    );
    assert_eq!(
        all.iter()
            .filter(|s| s["code"]
                .as_str()
                .is_some_and(|c| c.contains("PlayerOperations.APPOINT_GOVERNOR")))
            .count(),
        1
    );
    assert!(all[start + 1..stop].iter().all(|s| !matches!(
        s["action"].as_str(),
        Some("end_turn" | "save" | "load" | "autoplay")
    )));
    let lua = Lua::new();
    for s in all {
        if let Some(c) = s["code"]
            .as_str()
            .or_else(|| s["condition"]["code"].as_str())
        {
            compile_lua(&lua, c);
        }
    }
    let mut wrong = data.clone();
    wrong["steps"][stop]["action"] = json!("stop_native_can_ever_trace");
    assert!(
        validate_scenario(&wrong).is_err(),
        "wrong ABI mode cannot close assignment observation"
    );
    let mut overlapping = data.clone();
    overlapping["steps"][start]["action"] = json!("stop_native_joined_admission");
    assert!(validate_scenario(&overlapping).is_err());
}

#[test]
fn assignment_observation_requires_restart_before_reuse() {
    let start = json!({"action":"start_native_assign_calibration","name":"first","max_seconds":90});
    let stop = json!({"action":"stop_native_assign_calibration"});
    let second =
        json!({"action":"start_native_assign_calibration","name":"second","max_seconds":90});
    let scene = |steps| json!({"name":"assignment process lifetime","steps":steps});
    let repeated = vec![start.clone(), stop.clone(), second.clone(), stop.clone()];
    let err = validate_scenario(&scene(repeated)).unwrap_err().to_string();
    assert!(err.contains("single-use until restart_game"), "{err}");
    validate_scenario(&scene(vec![
        start.clone(),
        stop.clone(),
        json!({"action":"restart_game"}),
        second.clone(),
        stop.clone(),
    ]))
    .unwrap();
    assert!(validate_scenario(&scene(vec![
        start,
        json!({"action":"restart_game"}),
        stop.clone(),
        second,
        stop
    ]))
    .is_err());
}
