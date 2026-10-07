//! Merge-boundary contracts only; these tests never activate a game/runtime.
use civ6_autotest_rs::{
    runner::{Backend, Runner, SystemBackend},
    scenario::{validate_scenario, ACTIONS},
    time::SystemClock,
    Error, Result,
};
use serde_json::json;

#[test]
fn cold_start_without_either_observer_allows_the_existing_startup_budget() {
    let mut backend = SystemBackend::default();
    backend.check_native_order().unwrap();
    backend.native_bootstrap_health().unwrap();
    backend.check_native_bootstrap().unwrap();
}

#[test]
fn either_observer_failure_interrupts_the_shared_step_budget() {
    struct Ended(bool);
    impl Backend for Ended {
        fn check_native_order(&self) -> Result<()> {
            if self.0 {
                Err(Error::input("order observation ended"))
            } else {
                Ok(())
            }
        }
        fn native_bootstrap_health(&self) -> Result<()> {
            Err(Error::input("bootstrap controller ended"))
        }
    }
    let temp = tempfile::tempdir().unwrap();
    for (order_ended, expected) in [
        (true, "order observation ended"),
        (false, "bootstrap controller ended"),
    ] {
        let runner = Runner::with_backend(
            json!({"name":"health contract","steps":[]}),
            temp.path().into(),
            temp.path().into(),
            temp.path().into(),
            4318,
            Box::new(Ended(order_ended)),
            std::sync::Arc::new(SystemClock),
        );
        assert_eq!(runner.remaining(f64::MAX).unwrap_err().message, expected);
    }
}

#[test]
fn integrated_actions_keep_order_and_resident_modes_exclusive_with_bootstrap() {
    assert_eq!(ACTIONS.len(), 43);
    assert_eq!(
        ACTIONS
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        43
    );
    for action in [
        "start_native_order_capture",
        "set_native_order_phase",
        "stop_native_order_capture",
        "start_native_paid_runtime",
        "stop_native_paid_runtime_observation",
        "start_native_bootstrap",
        "stop_native_bootstrap",
    ] {
        assert!(ACTIONS.contains(&action));
    }
    let temp = tempfile::tempdir().unwrap();
    let bootstrap = json!({"action":"start_native_bootstrap","name":"bootstrap", "max_seconds":60,
        "runtime_dll":temp.path().join("runtime.dll"),"runtime_sha256":"a".repeat(64),
        "bootstrap_dll":temp.path().join("bootstrap.dll"),"bootstrap_sha256":"b".repeat(64)});
    for (native, stop) in [
        (
            json!({"action":"start_native_order_capture","name":"order","max_seconds":60,
            "max_hits":16,"front_prefix":4,"load_prefix":4,"phase":"front"}),
            "stop_native_order_capture",
        ),
        (
            json!({"action":"start_native_paid_runtime","name":"runtime","max_seconds":60}),
            "stop_native_paid_runtime_observation",
        ),
    ] {
        for pair in [
            [bootstrap.clone(), native.clone()],
            [native, bootstrap.clone()],
        ] {
            let scene = json!({"name":"exclusive","steps":[{"action":"restart_game"},
                pair[0],pair[1],{"action":stop},{"action":"stop_native_bootstrap"}]});
            assert!(validate_scenario(&scene).is_err());
        }
    }
}
