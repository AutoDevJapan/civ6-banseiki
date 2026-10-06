//! Routing/schema fixtures, not simulated native payment acceptance.
use super::*;
use std::sync::Mutex;
struct Route(Arc<Mutex<Vec<Value>>>);
impl Backend for Route {
    fn native_ai_diagnostic(&mut self, request: &Value) -> Result<Value> {
        self.0.lock().unwrap().push(request.clone());
        Err(Error::input("transport fixture: no native response"))
    }
}
fn start() -> Value {
    json!({"action":"start_native_ai_observation","owner":2,"generation":4,
    "index":7,"cost":3,"paid":1,"spent":9,"title":8})
}
#[test]
fn ai_diagnostic_common_runner_routes_start_stop_and_propagates_unknown() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let dir = tempfile::tempdir().unwrap();
    let mut runner = Runner::with_backend(
        json!({"name":"route","steps":[]}),
        dir.path().into(),
        dir.path().into(),
        dir.path().into(),
        4318,
        Box::new(Route(calls.clone())),
        Arc::new(SystemClock),
    );
    assert!(runner.run_step(&start()).is_err());
    assert!(calls.lock().unwrap().is_empty());
    runner.resident_runtime_identity = Some((1, 2));
    assert!(
        runner
            .run_step(&start())
            .unwrap_err()
            .to_string()
            .contains("no native response")
    );
    let mut stop = start();
    stop["action"] = json!("stop_native_ai_observation");
    assert!(
        runner
            .run_step(&stop)
            .unwrap_err()
            .to_string()
            .contains("no native response")
    );
    let mut bad = start();
    bad["execution_mode"] = json!(1);
    assert!(runner.run_step(&bad).is_err());
    assert_eq!(*calls.lock().unwrap(), vec![start(), stop]);
}
#[test]
fn ai_diagnostic_scenario_requires_resident_pair_same_subject_and_no_repeat() {
    let open = json!({"action":"start_native_paid_runtime","name":"fixture","max_seconds":120});
    let close = json!({"action":"stop_native_paid_runtime_observation"});
    let mut stop = start();
    stop["action"] = json!("stop_native_ai_observation");
    let validate = |steps: Vec<Value>| {
        crate::scenario::validate_scenario(&json!({"name":"AI diagnostic","steps":steps}))
    };
    assert!(validate(vec![open.clone(), start(), stop.clone(), close.clone()]).is_ok());
    assert!(validate(vec![start(), stop.clone()]).is_err());
    assert!(validate(vec![open.clone(), start(), close.clone()]).is_err());
    assert!(
        validate(vec![
            open.clone(),
            start(),
            stop.clone(),
            start(),
            stop.clone(),
            close.clone()
        ])
        .is_err()
    );
    stop["generation"] = json!(5);
    assert!(validate(vec![open, start(), stop, close]).is_err());
}
