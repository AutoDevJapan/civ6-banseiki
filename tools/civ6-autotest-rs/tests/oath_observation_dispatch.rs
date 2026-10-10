mod support_driver;
use civ6_autotest_rs::{
    oath_observation::{Install, Operation},
    runner::{Backend, Runner},
    scenario, Error, Result,
};
use serde_json::{json, Value};
use std::{
    fs,
    sync::{Arc, Mutex},
};
use support_driver::ManualClock;

struct Probe {
    calls: Arc<Mutex<Vec<Operation>>>,
    ready: bool,
    reject_mode: bool,
    fail_operation: bool,
}
impl Backend for Probe {
    fn list_states(&mut self, _: f64) -> Result<Vec<String>> {
        Ok(vec!["InGame".into(), "GameCore_Tuner".into()])
    }
    fn require_single_process(&mut self) -> Result<()> {
        Ok(())
    }
    fn run_lua(&mut self, _: &str, code: &str, _: f64) -> Result<String> {
        if self.reject_mode && code.contains("GetRuleSet") {
            return Err(Error::lua("actual mode refused"));
        }
        Ok(self.ready.to_string())
    }
    fn oath_trace_operation(&mut self, op: Operation, install: Option<&Install>) -> Result<Value> {
        self.calls.lock().unwrap().push(op);
        assert_eq!(install.is_some(), op == Operation::Install);
        if self.fail_operation {
            return Err(Error::input("unknown native outcome"));
        }
        Ok(json!({"status":"observed","quantity_accepted":false}))
    }
}
fn step(action: &str) -> Value {
    if action == "oath_trace_install" {
        json!({"action":action,"timeout":0.5,"dll":"owned.dll",
        "dll_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","max_seconds":300})
    } else {
        json!({"action":action,"timeout":0.5})
    }
}
fn runner(
    root: &std::path::Path,
    ready: bool,
    reject_mode: bool,
    fail_operation: bool,
) -> (Runner, Arc<Mutex<Vec<Operation>>>) {
    let output = root.join("artifacts");
    fs::create_dir(&output).unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let data =
        json!({"name":"oath observer","diagnostic_only":true,"steps":[step("oath_trace_install")]});
    let probe = Probe {
        calls: calls.clone(),
        ready,
        reject_mode,
        fail_operation,
    };
    let mut runner = Runner::with_backend(
        data,
        output,
        root.join("logs"),
        root.join("saves"),
        4318,
        Box::new(probe),
        Arc::new(ManualClock::default()),
    );
    runner.test_mode = true;
    (runner, calls)
}

#[test]
fn positive_dispatch_reserves_before_call_and_preserves_nonacceptance() {
    let temp = tempfile::tempdir().unwrap();
    let (mut runner, calls) = runner(temp.path(), true, false, false);
    scenario::validate_scenario(&runner.scenario).unwrap();
    for (index, action) in [
        "oath_trace_install",
        "oath_trace_start",
        "oath_trace_snapshot",
        "oath_trace_stop",
    ]
    .iter()
    .enumerate()
    {
        runner.completed_steps = index;
        runner.run_step(&step(action)).unwrap();
        let file = runner
            .output
            .join(format!("oath-observation-{}-{}.json", index + 1, action));
        let proof: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
        assert_eq!(proof["quantity_accepted"], false);
        assert_eq!(proof["product_accepted"], false);
    }
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            Operation::Install,
            Operation::Start,
            Operation::Snapshot,
            Operation::Stop
        ]
    );
    assert!(!runner.result_passed);
}

#[test]
fn lifecycle_install_uses_the_same_mode_and_readiness_guards() {
    for mode in ["valid", "wrong_mode", "not_ready"] {
        let temp = tempfile::tempdir().unwrap();
        let (mut runner, calls) = runner(
            temp.path(),
            mode != "not_ready",
            mode == "wrong_mode",
            false,
        );
        let mut install = step("oath_trace_install");
        install["capture"] = json!("lifecycle");
        runner.scenario["steps"][0] = install.clone();
        scenario::validate_scenario(&runner.scenario).unwrap();
        assert_eq!(runner.run_step(&install).is_ok(), mode == "valid");
        assert_eq!(calls.lock().unwrap().len(), usize::from(mode == "valid"));
    }
}

#[test]
fn actual_mode_ready_and_explicit_optin_guards_precede_every_native_operation() {
    for action in [
        "oath_trace_install",
        "oath_trace_start",
        "oath_trace_snapshot",
        "oath_trace_stop",
    ] {
        for mode in [
            "not_test",
            "not_diagnostic",
            "multiplayer",
            "actual_wrong_mode",
            "not_ready",
        ] {
            if mode == "not_ready" && matches!(action, "oath_trace_snapshot" | "oath_trace_stop") {
                continue;
            }
            let temp = tempfile::tempdir().unwrap();
            let (mut runner, calls) = runner(
                temp.path(),
                mode != "not_ready",
                mode == "actual_wrong_mode",
                false,
            );
            match mode {
                "not_test" => runner.test_mode = false,
                "not_diagnostic" => runner.scenario["diagnostic_only"] = json!(false),
                "multiplayer" => runner.scenario["session_type"] = json!("hotseat"),
                _ => {}
            }
            assert!(
                runner.run_step(&step(action)).is_err(),
                "accepted {action}/{mode}"
            );
            assert!(
                calls.lock().unwrap().is_empty(),
                "native operation under {action}/{mode}"
            );
        }
    }
}

#[test]
fn preserved_evidence_and_unknown_native_outcome_prevent_resending() {
    for fail in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let (mut runner, calls) = runner(temp.path(), true, false, fail);
        let file = runner
            .output
            .join("oath-observation-1-oath_trace_install.json");
        if !fail {
            fs::write(&file, b"original proof").unwrap();
        }
        assert!(runner.run_step(&step("oath_trace_install")).is_err());
        assert_eq!(calls.lock().unwrap().len(), usize::from(fail));
        assert!(runner.run_step(&step("oath_trace_install")).is_err());
        assert_eq!(calls.lock().unwrap().len(), usize::from(fail));
        if fail {
            let proof: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
            assert_eq!(proof["status"], "reserved_before_native_call");
            assert_eq!(proof["quantity_accepted"], false);
        } else {
            assert_eq!(fs::read(file).unwrap(), b"original proof");
        }
    }
}
