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
    steam_marked: bool,
}
impl Backend for Probe {
    fn begin_steam_observation(&mut self, _: &std::path::Path) -> Result<()> {
        self.steam_marked = true;
        Ok(())
    }
    fn steam_sync_status(
        &mut self,
        _: civ6_autotest_rs::steam_sync::Stage,
    ) -> Result<civ6_autotest_rs::steam_sync::SyncStatus> {
        if !self.steam_marked {
            return Err(Error::input(
                "stale Steam observation; fresh marker required",
            ));
        }
        Ok(civ6_autotest_rs::steam_sync::SyncStatus::Idle)
    }
    fn oath_trace_exit_budget(&mut self) -> Result<Option<f64>> {
        let calls = self.calls.lock().unwrap();
        if !calls.contains(&Operation::Install) || !calls.contains(&Operation::Start) {
            return Err(Error::input("observer session missing"));
        }
        Ok(None)
    }
    fn oath_trace_finalize(&mut self, timeout: f64) -> Result<Value> {
        if timeout <= 0.0 || timeout > 300.0 {
            return Err(Error::input("invalid final budget"));
        }
        let mut calls = self.calls.lock().unwrap();
        if !calls.contains(&Operation::Install) || !calls.contains(&Operation::Start) {
            return Err(Error::input("observer session missing"));
        }
        calls.push(Operation::Finalize);
        Ok(
            json!({"snapshot_phase":"after_process_exit","process_exit_observed":!self.fail_operation,
            "controller_deadline_expired":false,"process_exit_code":0,
            "controller_remaining_seconds":0.25,
            "quantity_accepted":false,"product_accepted":false}),
        )
    }
    fn list_states(&mut self, _: f64) -> Result<Vec<String>> {
        Ok(vec![
            "FrontEnd".into(),
            "InGame".into(),
            "GameCore_Tuner".into(),
        ])
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
        assert_eq!(
            install.is_some(),
            matches!(op, Operation::Install | Operation::Prepare)
        );
        if self.fail_operation {
            return Err(Error::input("unknown native outcome"));
        }
        Ok(
            json!({"status":"observed","quantity_accepted":false,"installed_before_target_context":op==Operation::Ready}),
        )
    }
}
fn step(action: &str) -> Value {
    if matches!(action, "oath_trace_install" | "oath_trace_prepare") {
        json!({"action":action,"timeout":0.5,"dll":"owned.dll",
        "dll_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","max_seconds":300})
    } else {
        json!({"action":action,"timeout":0.5})
    }
}

#[test]
fn cold_preparation_checks_frontend_mode_and_receipt_is_a_separate_operation() {
    for (ready, reject) in [(true, false), (true, true), (false, false)] {
        let temp = tempfile::tempdir().unwrap();
        let (mut runner, calls) = runner(temp.path(), ready, reject, false);
        let mut prepare = step("oath_trace_prepare");
        prepare["capture"] = json!("cold_lifecycle");
        runner.scenario["steps"] = json!([prepare]);
        scenario::validate_scenario(&runner.scenario).unwrap();
        assert_eq!(runner.run_step(&prepare).is_ok(), ready && !reject);
        if ready && !reject {
            runner.completed_steps = 1;
            runner.run_step(&step("oath_trace_ready")).unwrap();
            assert_eq!(
                *calls.lock().unwrap(),
                vec![Operation::Prepare, Operation::Ready]
            );
            let proof: Value = serde_json::from_slice(
                &fs::read(
                    runner
                        .output
                        .join("oath-observation-2-oath_trace_ready.json"),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(proof["quantity_accepted"], false);
        } else {
            assert!(calls.lock().unwrap().is_empty());
        }
    }
}

#[test]
fn postgame_install_cannot_be_relabelled_as_cold_and_prepare_requires_that_mode() {
    let mut install = step("oath_trace_install");
    install["capture"] = json!("cold_lifecycle");
    assert!(civ6_autotest_rs::oath_observation::validate_step(&install).is_err());
    install["action"] = json!("oath_trace_prepare");
    civ6_autotest_rs::oath_observation::validate_step(&install).unwrap();
    for mode in [json!("lifecycle"), json!("callee"), json!(null)] {
        install["capture"] = mode;
        assert!(civ6_autotest_rs::oath_observation::validate_step(&install).is_err());
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
        steam_marked: false,
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

#[test]
fn finalization_uses_the_existing_session_and_reserves_evidence_without_live_lua() {
    let temp = tempfile::tempdir().unwrap();
    // All Lua calls on this backend fail. Finalization still requires an
    // installed/started session, but it must be usable after the game exits.
    let (mut runner, calls) = runner(temp.path(), false, true, false);
    assert!(runner.run_step(&step("oath_trace_finalize")).is_err());
    assert!(calls.lock().unwrap().is_empty());
    runner.completed_steps = 1;
    calls
        .lock()
        .unwrap()
        .extend([Operation::Install, Operation::Start]);
    let action = step("oath_trace_finalize");
    runner.scenario["steps"] = json!([action]);
    scenario::validate_scenario(&runner.scenario).unwrap();
    runner.run_step(&action).unwrap();
    assert_eq!(calls.lock().unwrap().last(), Some(&Operation::Finalize));
    let proof: Value = serde_json::from_slice(
        &fs::read(
            runner
                .output
                .join("oath-observation-2-oath_trace_finalize.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(proof["native_observation"]["process_exit_observed"], true);
    assert_eq!(proof["product_accepted"], false);
    let count = calls.lock().unwrap().len();
    assert!(runner.run_step(&action).is_err());
    assert_eq!(
        calls.lock().unwrap().len(),
        count,
        "same evidence is never overwritten or resent"
    );
}

#[test]
fn finalization_preserves_explicit_diagnostic_single_player_guards() {
    for mode in ["not_test", "not_diagnostic", "multiplayer"] {
        let temp = tempfile::tempdir().unwrap();
        let (mut runner, calls) = runner(temp.path(), true, false, false);
        // Establish a real positive mock path before testing each rejection.
        runner.run_step(&step("oath_trace_install")).unwrap();
        runner.completed_steps = 1;
        runner.run_step(&step("oath_trace_start")).unwrap();
        runner.completed_steps = 2;
        match mode {
            "not_test" => runner.test_mode = false,
            "not_diagnostic" => runner.scenario["diagnostic_only"] = json!(false),
            "multiplayer" => runner.scenario["session_type"] = json!("hotseat"),
            _ => unreachable!(),
        }
        assert!(runner.run_step(&step("oath_trace_finalize")).is_err());
        assert_eq!(
            *calls.lock().unwrap(),
            vec![Operation::Install, Operation::Start]
        );
    }
}

#[test]
fn unknown_final_process_exit_fails_the_step_but_keeps_its_raw_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let (mut runner, calls) = runner(temp.path(), false, true, true);
    calls
        .lock()
        .unwrap()
        .extend([Operation::Install, Operation::Start]);
    let error = runner.run_step(&step("oath_trace_finalize")).unwrap_err();
    assert!(error.message.contains("raw evidence preserved"));
    let proof: Value = serde_json::from_slice(
        &fs::read(
            runner
                .output
                .join("oath-observation-1-oath_trace_finalize.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(proof["native_observation"]["process_exit_observed"], false);
    assert_eq!(proof["product_accepted"], false);
    assert_eq!(calls.lock().unwrap().last(), Some(&Operation::Finalize));
}

struct ClosingProbe {
    events: Arc<Mutex<Vec<String>>>,
    pending: usize,
    cold_missing: bool,
}
impl Backend for ClosingProbe {
    fn require_single_process(&mut self) -> Result<()> {
        Ok(())
    }
    fn oath_trace_exit_budget(&mut self) -> Result<Option<f64>> {
        Ok(Some(60.0))
    }
    fn begin_steam_observation(&mut self, _: &std::path::Path) -> Result<()> {
        self.events.lock().unwrap().push("fresh_mark".into());
        Ok(())
    }
    fn list_states(&mut self, _: f64) -> Result<Vec<String>> {
        Ok(vec!["InGame".into()])
    }
    fn run_lua(&mut self, _: &str, code: &str, _: f64) -> Result<String> {
        if code.contains("UserConfirmedClose") {
            self.events.lock().unwrap().push("close_once".into());
            return Err(Error::unknown("game disconnected after sending exit"));
        }
        Ok("true".into())
    }
    fn oath_trace_finalize(&mut self, _: f64) -> Result<Value> {
        self.events.lock().unwrap().push("final_copy".into());
        let mut proof = json!({"process_exit_observed":true,"process_exit_code":0,
            "controller_deadline_expired":false,"controller_remaining_seconds":2.0,
            "raw_first":"first-copy","raw_second":"second-copy"});
        if self.cold_missing {
            proof["cold_receipt"] = json!({"installed_before_target_context":true});
            proof["cold_loader_context_root_observed"] = json!(false);
            proof["application_teardown_diagnostic"] = json!("unknown");
        }
        Ok(proof)
    }
    fn steam_sync_status(
        &mut self,
        stage: civ6_autotest_rs::steam_sync::Stage,
    ) -> Result<civ6_autotest_rs::steam_sync::SyncStatus> {
        assert_eq!(stage, civ6_autotest_rs::steam_sync::Stage::Exit);
        self.events.lock().unwrap().push("exit_sync".into());
        if self.pending > 0 {
            self.pending -= 1;
            Ok(civ6_autotest_rs::steam_sync::SyncStatus::Pending)
        } else {
            Ok(civ6_autotest_rs::steam_sync::SyncStatus::Idle)
        }
    }
}
#[test]
fn lost_close_reply_is_not_resent_and_fresh_steam_sync_is_required() {
    for pending in [1, usize::MAX] {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("artifacts");
        fs::create_dir(&output).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut runner = Runner::with_backend(
            json!({"name":"shared lifecycle close","diagnostic_only":true}),
            output.clone(),
            temp.path().join("logs"),
            temp.path().join("saves"),
            4318,
            Box::new(ClosingProbe {
                events: events.clone(),
                pending,
                cold_missing: false,
            }),
            Arc::new(ManualClock::default()),
        );
        runner.test_mode = true;
        let action = json!({"action":"oath_trace_finalize","timeout":60.0});
        let outcome = runner.run_step(&action);
        assert_eq!(outcome.is_ok(), pending == 1, "{outcome:?}");
        let events = events.lock().unwrap();
        assert_eq!(events.iter().filter(|e| *e == "close_once").count(), 1);
        assert_eq!(events.iter().filter(|e| *e == "final_copy").count(), 1);
        assert!(
            events.iter().position(|e| e == "fresh_mark").unwrap()
                < events.iter().position(|e| e == "close_once").unwrap()
        );
        assert!(
            events.iter().position(|e| e == "final_copy").unwrap()
                < events.iter().position(|e| e == "exit_sync").unwrap()
        );
        let proof: Value = serde_json::from_slice(
            &fs::read(output.join("oath-observation-1-oath_trace_finalize.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(proof["native_observation"]["raw_first"], "first-copy");
        assert_eq!(proof["native_observation"]["raw_second"], "second-copy");
        assert_eq!(proof["product_accepted"], false);
    }
}

#[test]
fn incomplete_cold_capture_keeps_raw_evidence_and_finishes_steam_wait_before_failing() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("artifacts");
    fs::create_dir(&output).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut runner = Runner::with_backend(
        json!({"name":"missing cold trace","diagnostic_only":true}),
        output.clone(),
        temp.path().join("logs"),
        temp.path().join("saves"),
        4318,
        Box::new(ClosingProbe {
            events: events.clone(),
            pending: 1,
            cold_missing: true,
        }),
        Arc::new(ManualClock::default()),
    );
    runner.test_mode = true;
    let error = runner
        .run_step(&json!({"action":"oath_trace_finalize","timeout":60.0}))
        .unwrap_err();
    assert!(error.message.contains("after Steam synchronization"));
    assert!(events
        .lock()
        .unwrap()
        .iter()
        .any(|event| event == "exit_sync"));
    let raw: Value = serde_json::from_slice(
        &fs::read(output.join("oath-observation-1-oath_trace_finalize.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(raw["native_observation"]["raw_first"], "first-copy");
    assert_eq!(
        raw["native_observation"]["cold_loader_context_root_observed"],
        false
    );
    assert_eq!(raw["product_accepted"], false);
}
