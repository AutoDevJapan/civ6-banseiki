mod support_driver;
use civ6_autotest_rs::{
    runner::{Backend, Runner},
    scenario, Error, Result,
};
use serde_json::{json, Value};
use std::{
    fs,
    sync::{Arc, Mutex},
};
use support_driver::ManualClock;

struct ProbeBackend {
    mutations: Arc<Mutex<usize>>,
    ready: bool,
    wrong_ruleset: bool,
    scopes: Arc<Mutex<Vec<civ6_autotest_rs::oath_exact_removal::Scope>>>,
}
impl Backend for ProbeBackend {
    fn oath_exact_removal_scope(
        &mut self,
        scope: civ6_autotest_rs::oath_exact_removal::Scope,
    ) -> Result<Value> {
        self.scopes.lock().unwrap().push(scope);
        self.oath_exact_removal()
    }
    fn require_single_process(&mut self) -> Result<()> {
        Ok(())
    }
    fn list_states(&mut self, _: f64) -> Result<Vec<String>> {
        Ok(vec!["InGame".into(), "GameCore_Tuner".into()])
    }
    fn run_lua(&mut self, _: &str, code: &str, _: f64) -> Result<String> {
        if self.wrong_ruleset && code.contains("GetRuleSet") {
            return Err(Error::lua("test ruleset required"));
        }
        Ok(self.ready.to_string())
    }
    fn oath_exact_removal(&mut self) -> Result<Value> {
        *self.mutations.lock().unwrap() += 1;
        Ok(json!({"status":"applied","task_accepted":false}))
    }
}
fn data() -> Value {
    json!({"name":"exact removal","diagnostic_only":true,"steps":[{"action":"oath_exact_removal","timeout":0.5}]})
}
fn runner(
    root: &std::path::Path,
    data: Value,
    ready: bool,
    wrong_ruleset: bool,
) -> (Runner, Arc<Mutex<usize>>) {
    let mutations = Arc::new(Mutex::new(0));
    let output = root.join("artifacts");
    fs::create_dir(&output).unwrap();
    let mut runner = Runner::with_backend(
        data,
        output,
        root.join("logs"),
        root.join("saves"),
        4318,
        Box::new(ProbeBackend {
            mutations: mutations.clone(),
            ready,
            wrong_ruleset,
            scopes: Arc::new(Mutex::new(vec![])),
        }),
        Arc::new(ManualClock::default()),
    );
    runner.test_mode = true;
    (runner, mutations)
}
#[test]
fn schema_refuses_non_diagnostic_multiplayer_and_arbitrary_native_parameters() {
    scenario::validate_scenario(&data()).unwrap();
    for mutation in [
        "ordinary",
        "multiplayer",
        "bytes",
        "unknown_scope",
        "null_scope",
    ] {
        let mut value = data();
        match mutation {
            "ordinary" => value["diagnostic_only"] = json!(false),
            "multiplayer" => value["session_type"] = json!("hotseat"),
            "unknown_scope" => value["steps"][0]["scope"] = json!("all"),
            "null_scope" => value["steps"][0]["scope"] = Value::Null,
            _ => value["steps"][0]["bytes"] = json!([0x90]),
        }
        assert!(
            scenario::validate_scenario(&value).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn explicit_scope_dispatches_and_keeps_legacy_proof_separate() {
    use civ6_autotest_rs::oath_exact_removal::Scope;
    let dir = tempfile::tempdir().unwrap();
    let mut value = data();
    value["steps"][0]["scope"] = json!("naval-buildings-v1");
    scenario::validate_scenario(&value).unwrap();
    let step = value["steps"][0].clone();
    let (mut runner, calls) = runner(dir.path(), value, true, false);
    let observed = Arc::new(Mutex::new(vec![]));
    runner.backend = Box::new(ProbeBackend {
        mutations: calls.clone(),
        ready: true,
        wrong_ruleset: false,
        scopes: observed.clone(),
    });
    let legacy = runner.output.join("oath-exact-removal.json");
    fs::write(&legacy, b"legacy proof").unwrap();
    runner.run_step(&step).unwrap();
    assert_eq!(*observed.lock().unwrap(), vec![Scope::NavalBuildingsV1]);
    assert_eq!(*calls.lock().unwrap(), 1);
    assert_eq!(fs::read(&legacy).unwrap(), b"legacy proof");
    let path = runner
        .output
        .join("oath-exact-removal-naval-buildings-v1.json");
    let proof: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(proof, json!({"status":"applied","task_accepted":false}));
    assert!(runner.run_step(&step).is_err());
    assert_eq!(*calls.lock().unwrap(), 1);
    assert!(!runner.result_passed);
}
#[test]
fn dispatch_guards_run_before_native_memory_mutation() {
    for mode in [
        "not_test_mode",
        "not_diagnostic",
        "not_ready",
        "wrong_ruleset",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut value = data();
        if mode == "not_diagnostic" {
            value["diagnostic_only"] = json!(false);
        }
        let (mut runner, calls) = runner(
            dir.path(),
            value,
            mode != "not_ready",
            mode == "wrong_ruleset",
        );
        if mode == "not_test_mode" {
            runner.test_mode = false;
        }
        assert!(
            runner
                .run_step(&json!({"action":"oath_exact_removal","timeout":0.5}))
                .is_err(),
            "accepted {mode}"
        );
        assert_eq!(*calls.lock().unwrap(), 0, "native mutation under {mode}");
    }
}
#[test]
fn existing_proof_file_stops_before_mutation_and_is_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let (mut runner, calls) = runner(dir.path(), data(), true, false);
    let path = runner.output.join("oath-exact-removal.json");
    fs::write(&path, b"original evidence").unwrap();
    assert!(runner
        .run_step(&json!({"action":"oath_exact_removal","timeout":0.5}))
        .is_err());
    assert_eq!(*calls.lock().unwrap(), 0);
    assert_eq!(fs::read(path).unwrap(), b"original evidence");
}
#[test]
fn successful_dispatch_preserves_a_diagnostic_proof_without_accepting_the_task() {
    let dir = tempfile::tempdir().unwrap();
    let (mut runner, calls) = runner(dir.path(), data(), true, false);
    runner
        .run_step(&json!({"action":"oath_exact_removal","timeout":0.5}))
        .unwrap();
    assert_eq!(*calls.lock().unwrap(), 1);
    let proof: Value =
        serde_json::from_slice(&fs::read(runner.output.join("oath-exact-removal.json")).unwrap())
            .unwrap();
    assert_eq!(proof, json!({"status":"applied","task_accepted":false}));
    assert!(!runner.result_passed);
}
