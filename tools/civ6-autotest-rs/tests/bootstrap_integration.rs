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
fn insufficient_bootstrap_lease_refuses_before_frontend_or_controller_start() {
    struct Lease(f64);
    impl Backend for Lease {
        fn live_lease_budget(&self) -> Result<Option<f64>> {
            Ok(Some(self.0))
        }
        fn start_native_bootstrap(
            &mut self,
            _: &std::path::Path,
            _: &serde_json::Value,
            _: std::time::Instant,
        ) -> Result<()> {
            panic!("lease refusal must precede controller startup");
        }
    }
    let temp = tempfile::tempdir().unwrap();
    for (remaining, controller) in [
        (1.0, None),
        (104.99, None),
        (105.0, None),
        (400.0, Some(1500)),
        (1545.0, Some(1500)),
        (1580.0, Some(1500)),
    ] {
        let mut runner = Runner::with_backend(
            json!({"name":"pre-start lease refusal","steps":[]}),
            temp.path().into(),
            temp.path().into(),
            temp.path().into(),
            4318,
            Box::new(Lease(remaining)),
            std::sync::Arc::new(SystemClock),
        );
        let mut step = json!({"action":"start_native_bootstrap","max_seconds":60});
        if let Some(seconds) = controller {
            step["controller_seconds"] = json!(seconds);
        }
        let error = runner.run_step(&step).unwrap_err();
        assert_eq!(
            error.message,
            "live_chain lease cannot cover bootstrap observation"
        );
    }
}

#[test]
fn invalid_controller_budget_is_refused_before_bootstrap_start() {
    struct Never;
    impl Backend for Never {
        fn start_native_bootstrap(
            &mut self,
            _: &std::path::Path,
            _: &serde_json::Value,
            _: std::time::Instant,
        ) -> Result<()> {
            panic!("invalid duration must not start a controller");
        }
    }
    let temp = tempfile::tempdir().unwrap();
    for seconds in [
        json!(0),
        json!(59),
        json!(1801),
        json!("1500"),
        serde_json::Value::Null,
    ] {
        let mut runner = Runner::with_backend(
            json!({"name":"invalid duration","steps":[]}),
            temp.path().into(),
            temp.path().into(),
            temp.path().into(),
            4318,
            Box::new(Never),
            std::sync::Arc::new(SystemClock),
        );
        assert_eq!(
            runner
                .run_step(&json!({"action":"start_native_bootstrap",
            "max_seconds":60,"controller_seconds":seconds}))
                .unwrap_err()
                .message,
            "controller_seconds must cover telemetry and be at most 1800"
        );
    }
}

#[test]
fn no_waiter_cannot_hide_the_native_controller_acquisition_deadline() {
    struct NoWaiter;
    impl Backend for NoWaiter {
        fn live_lease_budget(&self) -> Result<Option<f64>> {
            Ok(None)
        }
        fn native_bootstrap_lease_budget(&self) -> Result<Option<f64>> {
            Ok(Some(civ6_autotest_rs::live_lease::bootstrap_record_budget(
                &json!({"pid":7,"chain_pid":7,"live_chain":true,"acquired_at":100}),
                7,
                340.0,
            )?))
        }
        fn start_native_bootstrap(
            &mut self,
            _: &std::path::Path,
            _: &serde_json::Value,
            _: std::time::Instant,
        ) -> Result<()> {
            panic!("acquisition deadline must refuse before controller startup");
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let mut runner = Runner::with_backend(
        json!({"name":"no waiter deadline","steps":[]}),
        temp.path().into(),
        temp.path().into(),
        temp.path().into(),
        4318,
        Box::new(NoWaiter),
        std::sync::Arc::new(SystemClock),
    );
    let error = runner
        .run_step(&json!({"action":"start_native_bootstrap",
        "max_seconds":300,"controller_seconds":1500,"timeout":40}))
        .unwrap_err();
    assert_eq!(
        error.message,
        "live_chain lease cannot cover bootstrap observation"
    );
}

#[test]
fn frontend_time_is_subtracted_from_the_same_bootstrap_preparation_deadline() {
    use std::sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    };
    struct Clock(AtomicU64);
    impl civ6_autotest_rs::time::Clock for Clock {
        fn now(&self) -> f64 {
            self.0.load(Ordering::Relaxed) as f64
        }
        fn sleep(&self, _: f64) {}
    }
    struct Prepared {
        clock: Arc<Clock>,
        seen: Arc<AtomicBool>,
    }
    impl Backend for Prepared {
        fn require_single_process(&mut self) -> Result<()> {
            Ok(())
        }
        fn list_states(&mut self, _: f64) -> Result<Vec<String>> {
            Ok(vec!["FrontEnd".into()])
        }
        fn run_lua(&mut self, _: &str, _: &str, _: f64) -> Result<String> {
            self.clock.0.fetch_add(2, Ordering::Relaxed);
            Ok("true".into())
        }
        fn start_native_bootstrap(
            &mut self,
            _: &std::path::Path,
            _: &serde_json::Value,
            deadline: std::time::Instant,
        ) -> Result<()> {
            let left = deadline
                .saturating_duration_since(std::time::Instant::now())
                .as_secs_f64();
            assert!(
                left > 2.0 && left <= 3.0,
                "frontend used two seconds of the five-second step"
            );
            self.seen.store(true, Ordering::Relaxed);
            Ok(())
        }
    }
    let clock = Arc::new(Clock(AtomicU64::new(0)));
    let seen = Arc::new(AtomicBool::new(false));
    let temp = tempfile::tempdir().unwrap();
    let mut runner = Runner::with_backend(
        json!({"name":"shared preparation deadline","steps":[]}),
        temp.path().into(),
        temp.path().into(),
        temp.path().into(),
        4318,
        Box::new(Prepared {
            clock: clock.clone(),
            seen: seen.clone(),
        }),
        clock,
    );
    runner
        .run_step(&json!({"action":"start_native_bootstrap","max_seconds":60,"timeout":5}))
        .unwrap();
    assert!(seen.load(Ordering::Relaxed));
}

#[test]
fn system_backend_refuses_expired_preparation_before_process_discovery() {
    let mut backend = SystemBackend::default();
    let dir = tempfile::tempdir().unwrap();
    let error = backend
        .start_native_bootstrap(
            dir.path(),
            &json!({}),
            std::time::Instant::now() - std::time::Duration::from_secs(1),
        )
        .unwrap_err();
    assert_eq!(
        error.message,
        "bootstrap preparation deadline expired before process discovery"
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
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
