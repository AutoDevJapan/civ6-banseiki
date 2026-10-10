use civ6_autotest_rs::{native_order, scenario::validate_scenario};
use serde_json::{json, Value};

fn start() -> Value {
    json!({"action":"start_native_order_capture","name":"order-front", "max_seconds":120,
        "max_hits":128,"front_prefix":4,"load_prefix":4,"phase":"front"})
}
fn scenario(middle: Vec<Value>) -> Value {
    let mut steps = vec![start()];
    steps.extend(middle);
    steps.push(json!({"action":"stop_native_order_capture"}));
    json!({"name":"capture only","steps":steps})
}
#[test]
fn bounded_capture_can_span_loads_but_cannot_overlap_or_survive_restart() {
    assert!(validate_scenario(&scenario(vec![
        json!({"action":"set_native_order_phase","phase":"front"})
    ]))
    .is_err());
    validate_scenario(&scenario(vec![
        json!({"action":"set_native_order_phase","phase":"load"}),
        json!({"action":"load","name":"SAVED_GAME"}),
        json!({"action":"set_native_order_phase","phase":"front"}),
    ]))
    .unwrap();
    for middle in [
        start(),
        json!({"action":"restart_game"}),
        json!({"action":"start_native_can_ever_trace","name":"other","max_seconds":10}),
    ] {
        assert!(validate_scenario(&scenario(vec![middle])).is_err());
    }
    let mut missing_stop = scenario(vec![]);
    missing_stop["steps"].as_array_mut().unwrap().pop();
    assert!(validate_scenario(&missing_stop).is_err());
    let mut reversed = scenario(vec![]);
    reversed["steps"].as_array_mut().unwrap().reverse();
    assert!(validate_scenario(&reversed).is_err());
}
#[test]
fn observer_inputs_and_phase_changes_remain_finite() {
    for (key, value) in [
        ("name", json!("../outside")),
        ("max_hits", json!(257)),
        ("max_seconds", json!(0)),
        ("front_prefix", json!(0)),
        ("load_prefix", json!(5)),
        ("phase", json!("arbitrary")),
        ("address", json!(4096)),
        ("pid", json!(1)),
    ] {
        let mut data = start();
        data[key] = value;
        assert!(native_order::validate_options(&data).is_err(), "{key}");
    }
    let changes = (0..8).map(|i| json!({"action":"set_native_order_phase","phase":if i%2==0 {"load"} else {"front"}})).collect::<Vec<_>>();
    validate_scenario(&scenario(changes.clone())).unwrap();
    let mut ninth = changes;
    ninth.push(json!({"action":"set_native_order_phase","phase":"load"}));
    assert!(validate_scenario(&scenario(ninth)).is_err());
    let mut source = start();
    source["source_dll"] = json!("D:/CC/Work/todo880/owned/candidate.dll");
    assert!(native_order::validate_options(&source).is_err());
    source["source_dll_sha256"] = json!("a".repeat(64));
    native_order::validate_options(&source).unwrap();
    source["source_dll"] = json!("relative.dll");
    assert!(native_order::validate_options(&source).is_err());
}

#[test]
fn native_outer_capture_uses_the_same_bounded_scenario_and_excludes_runtime_hooks() {
    let mut native = start();
    native["phase"] = json!("native-outer");
    native_order::validate_options(&native).unwrap();
    let capture = json!({"name":"native outer capture only","steps":[native,
        {"action":"load","name":"SAVED_GAME"},
        {"action":"stop_native_order_capture"}]});
    validate_scenario(&capture).unwrap();
    validate_scenario(&scenario(vec![
        json!({"action":"set_native_order_phase","phase":"native-outer"}),
        json!({"action":"set_native_order_phase","phase":"load"}),
    ]))
    .unwrap();
    let mut overlap = capture;
    overlap["steps"].as_array_mut().unwrap().insert(
        1,
        json!({"action":"start_native_production_admission","name":"runtime","max_seconds":10}),
    );
    assert!(validate_scenario(&overlap).is_err());
}
#[test]
fn restored_success_needs_the_exact_process_and_complete_parent_receipt() {
    let good = json!({"type":"order_parent_receipt","schema":"todo880-order-v1","game_pid":12,
        "game_created_filetime":34,"source_dll_sha256":"abc","child_reaped":true,"child_pid":56,"permit_granted":true,"never_attached":false,"stop_writes":1,
        "cleanup_confirmed":true,"restored":true,"live_receipt":true,"child_exit_code":0,
        "error":null,"target_close_requested":false,"child_force_terminated":false});
    native_order::validate_receipt(&good, 12, 34, "abc", 56).unwrap();
    for key in [
        "game_pid",
        "game_created_filetime",
        "source_dll_sha256",
        "child_reaped",
        "child_pid",
        "permit_granted",
        "never_attached",
        "stop_writes",
        "cleanup_confirmed",
        "restored",
        "live_receipt",
        "child_exit_code",
        "error",
        "target_close_requested",
        "child_force_terminated",
    ] {
        let mut changed = good.clone();
        changed.as_object_mut().unwrap().remove(key);
        assert!(
            native_order::validate_receipt(&changed, 12, 34, "abc", 56).is_err(),
            "{key}"
        );
    }
    assert!(native_order::validate_receipt(&good, 13, 34, "abc", 56).is_err());
    assert!(native_order::validate_receipt(&good, 12, 35, "abc", 56).is_err());
    assert!(native_order::validate_receipt(&good, 12, 34, "other", 56).is_err());
    assert!(native_order::validate_receipt(&good, 12, 34, "abc", 57).is_err());
    assert!(native_order::validate_receipt(&good, 12, 34, "abc", 0).is_err());
    let mut closed = good;
    closed["target_close_requested"] = json!(true);
    assert!(native_order::validate_receipt(&closed, 12, 34, "abc", 56).is_err());
}
#[test]
fn branch_built_existing_helper_exposes_order_identity_without_game_access() {
    #[cfg(windows)]
    let _pin = {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(env!("CARGO_BIN_EXE_civ6-startup-diagnostics"))
            .unwrap()
    };
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_civ6-startup-diagnostics"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let out = command.arg("order-identity").output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        native_order::identity()
    );
}

#[test]
fn interrupted_observer_aborts_the_existing_step_budget_before_a_long_load_timeout() {
    use civ6_autotest_rs::{
        runner::{Backend, Runner},
        time::SystemClock,
        Error, Result,
    };
    struct Ended;
    impl Backend for Ended {
        fn check_native_order(&self) -> Result<()> {
            Err(Error::input("owned observer exited"))
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let runner = Runner::with_backend(
        json!({"name":"health","steps":[]}),
        temp.path().into(),
        temp.path().into(),
        temp.path().into(),
        4318,
        Box::new(Ended),
        std::sync::Arc::new(SystemClock),
    );
    assert!(runner
        .remaining(f64::MAX)
        .unwrap_err()
        .message
        .contains("owned observer exited"));
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn actual_completed_helper_is_not_reported_as_a_live_observer() {
    use std::os::windows::process::CommandExt;
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_civ6-startup-diagnostics"))
        .arg("order-identity")
        .stdout(std::process::Stdio::null())
        .creation_flags(0x08000000)
        .spawn()
        .unwrap();
    assert!(child.wait().unwrap().success());
    assert!(native_order::check_parent_active(&child)
        .unwrap_err()
        .message
        .contains("parent ended"));
}

#[test]
fn missing_log_line_does_not_hide_observer_exit_until_the_step_deadline() {
    use civ6_autotest_rs::{
        runner::{Backend, Runner},
        time::Clock,
        Error, Result,
    };
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    };
    struct Ticks(Arc<AtomicU64>);
    impl Clock for Ticks {
        fn now(&self) -> f64 {
            self.0.load(Ordering::Relaxed) as f64 / 1000.0
        }
        fn sleep(&self, seconds: f64) {
            self.0
                .fetch_add((seconds * 1000.0) as u64, Ordering::Relaxed);
        }
    }
    struct Exits(Arc<AtomicU64>);
    impl Backend for Exits {
        fn check_native_order(&self) -> Result<()> {
            if self.0.load(Ordering::Relaxed) >= 500 {
                Err(Error::input("observer ended after first poll"))
            } else {
                Ok(())
            }
        }
    }
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("Lua.log"), "").unwrap();
    let ticks = Arc::new(AtomicU64::new(0));
    let mut runner = Runner::with_backend(
        json!({"name":"log wait","steps":[]}),
        temp.path().into(),
        temp.path().into(),
        temp.path().into(),
        4318,
        Box::new(Exits(ticks.clone())),
        Arc::new(Ticks(ticks.clone())),
    );
    let error = runner
        .wait(
            &json!({"type":"log_line","log":"Lua.log","contains":"never emitted"}),
            240.0,
            false,
        )
        .unwrap_err();
    assert!(
        error.message.contains("observer ended after first poll"),
        "{}",
        error.message
    );
    assert_eq!(ticks.load(Ordering::Relaxed), 500);
}
