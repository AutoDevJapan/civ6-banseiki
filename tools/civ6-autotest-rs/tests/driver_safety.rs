mod support_safety;

#[allow(dead_code)]
#[path = "../examples/read-group-refusal-62.rs"]
mod refusal_reader;

#[test]
fn refusal_read_wait_never_closes_diplomacy_after_an_unknown_response() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| {
        Ok(vec![
            "InGame".into(),
            "DiplomacyActionView".into(),
            "DiplomacyDealView".into(),
        ])
    });
    let mut polls = 0;
    backend.on_lua = Box::new(move |_, code, _| {
        if code.contains("ExposedMembers.JAC_PersonaBoundary~=nil") {
            polls += 1;
            if polls == 1 {
                return Err(Error::new(
                    ErrorKind::LuaResponseUnknown,
                    "unknown read response",
                ));
            }
        }
        if code == "return ContextPtr:IsHidden()" {
            return Ok("false".into());
        }
        Ok("true".into())
    });
    let mut runner = f.runner(json!({"name":"preserve refusal","steps":[]}), backend);
    runner
        .run_step(&refusal_reader::scene()["steps"][1])
        .unwrap();
    let trace = f.trace();
    assert_eq!(
        trace
            .lua
            .iter()
            .filter(|(_, c, _)| c.contains("ExposedMembers.JAC_PersonaBoundary~=nil"))
            .count(),
        2
    );
    assert!(
        trace.lua.iter().all(|(state, _, _)| state == "InGame"),
        "{:?}",
        trace.lua
    );
}

#[test]
fn diplomacy_preservation_rejects_malformed_flags_and_wrong_action_locations() {
    let valid = [
        serde_json::json!({"action":"ui_lua","state_name":"InGame","code":"return true","preserve_diplomacy_session":true}),
        serde_json::json!({"action":"wait_for","condition":{"type":"lua","state_name":"InGame","code":"return true","preserve_diplomacy_session":false}}),
        serde_json::json!({"action":"save","name":"OwnedDiplomacySave","preserve_diplomacy_session":true}),
    ];
    for step in valid {
        validate_scenario(&json!({"name":"preserved session","steps":[step]})).unwrap();
    }
    let invalid = [
        json!({"action":"ui_lua","state_name":"InGame","code":"return true","preserve_diplomacy_session":"true"}),
        json!({"action":"wait_for","condition":{"type":"lua","state_name":"InGame","code":"return true","preserve_diplomacy_session":1}}),
        json!({"action":"wait_for","preserve_diplomacy_session":true,"condition":{"type":"lua","state_name":"InGame","code":"return true"}}),
        json!({"action":"pause","seconds":1,"preserve_diplomacy_session":true}),
        json!({"action":"save","name":"OwnedDiplomacySave","preserve_diplomacy_session":"true"}),
        json!({"action":"wait_for","condition":{"type":"state","name":"InGame","preserve_diplomacy_session":true}}),
    ];
    for step in invalid {
        assert!(
            validate_scenario(&json!({"name":"bad preservation","steps":[step]}))
                .unwrap_err()
                .message
                .contains("preserve_diplomacy_session")
        );
    }
}

#[test]
fn a_save_callback_retry_keeps_owned_diplomacy_and_never_resends_the_save() {
    let f = Fixture::new();
    let target = f.saves.join("OwnedDiplomacySave.Civ6Save");
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| {
        Ok(vec![
            "InGame".into(),
            "DiplomacyActionView".into(),
            "DiplomacyDealView".into(),
        ])
    });
    let mut polls = 0;
    backend.on_lua = Box::new(move |_, code, _| {
        if code.contains("Network.SaveGame") {
            fs::write(&target, b"owned save bytes").unwrap();
        }
        if code.contains("JAC_AUTOTEST_SAVE_DONE == true") {
            polls += 1;
            if polls == 1 {
                return Err(Error::new(
                    ErrorKind::LuaResponseUnknown,
                    "callback response may have arrived",
                ));
            }
        }
        if code == "return ContextPtr:IsHidden()" {
            return Ok("false".into());
        }
        Ok("true".into())
    });
    let mut r = f.runner(json!({"name":"preserved save","steps":[]}), backend);
    r.run_step(&json!({"action":"save","name":"OwnedDiplomacySave","exact_name":true,"preserve_diplomacy_session":true})).unwrap();
    let trace = f.trace();
    assert_eq!(
        trace
            .lua
            .iter()
            .filter(|(_, code, _)| code.contains("Network.SaveGame"))
            .count(),
        1
    );
    assert_eq!(
        trace
            .lua
            .iter()
            .filter(|(_, code, _)| code.contains("JAC_AUTOTEST_SAVE_DONE == true"))
            .count(),
        2
    );
    assert!(
        trace.lua.iter().all(|(state, _, _)| state == "InGame"),
        "{:?}",
        trace.lua
    );
}

use civ6_autotest_rs::{
    judge,
    runner::{AutoplayOptions, Runner, LOADSCREEN_READY},
    scenario::{safe_relative_path, validate_scenario},
    time::Clock,
    Error, ErrorKind,
};
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
};
use support_safety::Fixture;

fn error_has(error: Error, kind: ErrorKind, part: &str) {
    assert_eq!(error.kind, kind, "{error}");
    assert!(error.message.contains(part), "{error}");
}

fn generation_error(error: Error) {
    assert!(error.message.contains("rotated or truncated"), "{error}");
    error_has(error, ErrorKind::Scenario, "undecidable");
}

fn append(path: &Path, content: &str) {
    OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(content.as_bytes())
        .unwrap();
}

fn ready(fixture: &Fixture, runner: &mut Runner) {
    runner.baseline = Some(judge::snapshot_logs(&fixture.logs).unwrap());
    append(
        &fixture.logs.join("Lua.log"),
        &format!("{LOADSCREEN_READY}\n"),
    );
}

fn lua_count(fixture: &Fixture, needle: &str) -> usize {
    fixture
        .trace()
        .lua
        .iter()
        .filter(|(_, code, _)| code.contains(needle))
        .count()
}

fn scenario(step: Value) -> Value {
    json!({"name":"safety","steps":[step]})
}

// Legacy source: test_safety.py::ScenarioSafetyTests. Keep the original names
// so the inventory and cargo --test driver_safety output can be compared.

#[test]
fn test_owner_start_opens_setup_without_hosting_and_waits_long() {
    let f = Fixture::new();
    let mut backend = f.backend();
    let logs = f.logs.clone();
    backend.on_lua = Box::new(move |_, code, _| {
        if code.contains("AdvancedSetup") {
            append(&logs.join("Lua.log"), &format!("{LOADSCREEN_READY}\n"));
        }
        Ok("true".into())
    });
    let mut runner = f.direct_runner(json!({}), backend);
    runner.owner_start = true;
    runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    runner
        .run_step(&json!({"action":"start_game","timeout":600}))
        .unwrap();
    let t = f.trace();
    let code = &t.lua[0].1;
    assert!(code.contains("/FrontEnd/MainMenu/AdvancedSetup"));
    assert!(!code.contains("HostGame"));
    assert!(!code.contains("SetToDefaults"));
    assert_eq!(
        t.presses,
        vec![(civ6_autotest_rs::click::VK_ESCAPE, 3600.0)]
    );
}

#[test]
fn test_load_wait_sends_one_escape_even_if_ingame_state_exists() {
    let f = Fixture::new();
    let mut backend = f.backend();
    let answers = Arc::new(Mutex::new(vec!["false", "true"]));
    let answers_for_lua = answers.clone();
    backend.on_states = Box::new(|_| Ok(vec!["InGame".into()]));
    backend.on_lua = Box::new(move |_, code, _| {
        if code.contains("Game.GetLocalPlayer") {
            Ok(answers_for_lua.lock().unwrap().remove(0).into())
        } else {
            Ok("true".into())
        }
    });
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    runner
        .enter_game_after_loadscreen(0, 5.0, Some(5.0), false)
        .unwrap();
    assert_eq!(lua_count(&f, "Game.GetLocalPlayer"), 2);
    assert_eq!(lua_count(&f, "UI.IsGameCoreBusy"), 1);
    assert_eq!(
        f.trace().presses,
        vec![(civ6_autotest_rs::click::VK_ESCAPE, 5.0)]
    );
}

#[test]
fn test_load_wait_presses_begin_game_through_loadscreen_lua_without_focus() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Ok(vec!["LoadScreen".into(), "InGame".into()]));
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    runner
        .enter_game_after_loadscreen(0, 1.0, None, false)
        .unwrap();
    let t = f.trace();
    assert_eq!(t.lua[0].0, "LoadScreen");
    assert!(t.lua[0].1.contains("OnActivateButtonClicked()"));
    assert_eq!(lua_count(&f, "UI.IsGameCoreBusy"), 1);
    assert!(t.presses.is_empty());
}

#[test]
fn test_busy_core_cannot_admit_a_loaded_game_or_repeat_begin_game() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| {
        Ok(vec![
            "LoadScreen".into(),
            "InGame".into(),
            "InGameTopOptionsMenu".into(),
        ])
    });
    let busy_reads = Arc::new(Mutex::new(0_usize));
    let reads = busy_reads.clone();
    let mut menu_hidden = false;
    backend.on_lua = Box::new(move |_, code, _| {
        if code == "return ContextPtr:IsHidden()" {
            return Ok(menu_hidden.to_string());
        }
        if code.contains("CloseImmediately()") {
            menu_hidden = true;
        }
        if code.contains("UI.IsGameCoreBusy") {
            assert_eq!(code, "return not UI.IsGameCoreBusy()");
            let mut count = reads.lock().unwrap();
            *count += 1;
            // Model a newly visible options menu on each still-busy poll.
            // A default "true" response means hidden under main's new read-first contract.
            if *count < 3 {
                menu_hidden = false;
            }
            return Ok((*count >= 3).to_string());
        }
        Ok("true".into())
    });
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    runner
        .enter_game_after_loadscreen(0, 5.0, Some(5.0), false)
        .unwrap();
    assert_eq!(*busy_reads.lock().unwrap(), 3);
    assert_eq!(lua_count(&f, "OnActivateButtonClicked()"), 1);
    assert_eq!(lua_count(&f, "Game.GetLocalPlayer"), 3);
    assert_eq!(lua_count(&f, "CloseImmediately()"), 3);
    let codes: Vec<_> = f.trace().lua.into_iter().map(|(_, code, _)| code).collect();
    for (index, code) in codes.iter().enumerate() {
        if code == "return not UI.IsGameCoreBusy()" {
            assert!(codes[..index]
                .iter()
                .any(|code| code.contains("CloseImmediately()")));
        }
    }
}

#[test]
fn lost_begin_game_reply_is_observed_without_resending_the_transition() {
    for (reached_ingame, log_writable) in
        [(true, true), (true, false), (false, true), (false, false)]
    {
        let f = Fixture::new();
        let mut backend = f.backend();
        backend.on_states = Box::new(|_| Ok(vec!["LoadScreen".into(), "InGame".into()]));
        backend.on_lua = Box::new(move |_, code, _| {
            if code.contains("OnActivateButtonClicked") {
                Err(Error::unknown(
                    "loading context disappeared after Begin Game",
                ))
            } else if code.contains("Game.GetLocalPlayer") {
                Ok(reached_ingame.to_string())
            } else {
                Ok("true".into())
            }
        });
        let mut runner = f.direct_runner(json!({}), backend);
        if !log_writable {
            fs::create_dir(f.output.join("steps.jsonl")).unwrap();
        }
        ready(&f, &mut runner);
        let result = runner.enter_game_after_loadscreen(0, 3.0, None, false);
        if reached_ingame {
            result.unwrap();
        } else {
            error_has(
                result.unwrap_err(),
                ErrorKind::Scenario,
                "outcome undecidable",
            );
        }
        assert_eq!(lua_count(&f, "OnActivateButtonClicked"), 1);
        assert!(f.trace().presses.is_empty());
    }
}

#[test]
fn test_load_wait_never_sends_escape_before_load_complete_log() {
    let f = Fixture::new();
    let mut runner = f.direct_runner(json!({}), f.backend());
    runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    error_has(
        runner
            .enter_game_after_loadscreen(0, 0.02, None, false)
            .unwrap_err(),
        ErrorKind::Scenario,
        "entering the game",
    );
    assert!(f.trace().state_queries.is_empty());
    assert!(f.trace().presses.is_empty());
}

#[test]
fn test_load_wait_sends_begin_game_escape_only_once() {
    let f = Fixture::new();
    let mut backend = f.backend();
    let clock = f.clock.clone();
    backend.on_states = Box::new(move |_| {
        if civ6_autotest_rs::time::Clock::now(&*clock) >= 2.0 {
            Ok(vec!["InGame".into()])
        } else {
            Ok(vec![])
        }
    });
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    runner
        .enter_game_after_loadscreen(0, 5.0, None, false)
        .unwrap();
    assert_eq!(f.trace().presses.len(), 1);
}

#[test]
fn test_load_wait_reconnects_when_frontend_state_list_is_stale() {
    let f = Fixture::new();
    let mut backend = f.backend();
    let trace = f.trace.clone();
    backend.on_states = Box::new(move |_| {
        if trace.lock().unwrap().disconnects == 0 {
            Ok(vec!["FrontEnd".into()])
        } else {
            Ok(vec!["InGame".into()])
        }
    });
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    runner
        .enter_game_after_loadscreen(0, 1.0, None, false)
        .unwrap();
    assert_eq!(f.trace().presses.len(), 1);
    assert!(f.trace().disconnects >= 1);
}

#[test]
fn test_load_wait_timeout_never_repeats_escape() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Ok(vec![]));
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    error_has(
        runner
            .enter_game_after_loadscreen(0, 0.02, None, false)
            .unwrap_err(),
        ErrorKind::Scenario,
        "entering the game",
    );
    assert_eq!(f.trace().presses.len(), 1);
}

#[test]
fn test_transition_lua_errors_stop_without_retrying_mutations() {
    for kind in [ErrorKind::LuaResponseUnknown, ErrorKind::LuaExecution] {
        let f = Fixture::new();
        let mut backend = f.backend();
        backend.on_states = Box::new(|_| Ok(vec!["InGame".into(), "InGameTopOptionsMenu".into()]));
        backend.on_lua = Box::new(move |_, code, _| {
            if code == "return ContextPtr:IsHidden()" {
                Ok("false".into())
            } else if code.contains("CloseImmediately") {
                Err(Error::new(kind, "menu command failed"))
            } else {
                Ok("true".into())
            }
        });
        let mut runner = f.direct_runner(json!({}), backend);
        ready(&f, &mut runner);
        error_has(
            runner
                .enter_game_after_loadscreen(0, 1.0, None, false)
                .unwrap_err(),
            kind,
            "menu command failed",
        );
        let trace = f.trace();
        let close = trace
            .lua
            .iter()
            .position(|(_, code, _)| code.contains("CloseImmediately"))
            .unwrap();
        assert!(trace.lua[..close]
            .iter()
            .any(|(_, code, _)| code == "return ContextPtr:IsHidden()"));
        assert!(trace.lua[close + 1..]
            .iter()
            .all(|(_, code, _)| code == "return ContextPtr:IsHidden()"));
        assert_eq!(lua_count(&f, "CloseImmediately"), 1);
        assert_eq!(f.trace().presses.len(), 1);
        if kind == ErrorKind::LuaExecution {
            assert!(f.clock.sleeps().is_empty());
        }
    }
}

#[test]
fn test_transition_detects_lua_log_rotation_and_truncation_before_input() {
    for rotate in [true, false] {
        let f = Fixture::new();
        let lua = f.logs.join("Lua.log");
        fs::write(&lua, "old loading history\n").unwrap();
        let mut runner = f.direct_runner(json!({}), f.backend());
        runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
        let before = runner.loadscreen_count().unwrap();
        let replacement = f.logs.join("replacement.log");
        f.clock.on_sleep(move |_| {
            if rotate {
                fs::rename(&lua, &replacement).unwrap();
                fs::write(&lua, "fresh\n").unwrap();
            } else {
                fs::write(&lua, "fresh\n").unwrap();
            }
        });
        generation_error(
            runner
                .enter_game_after_loadscreen(before, 1.0, None, false)
                .unwrap_err(),
        );
        assert!(f.trace().presses.is_empty());
        assert!(f.trace().state_queries.is_empty());
    }
}

#[test]
fn test_transition_detects_lua_log_rotation_after_escape() {
    let f = Fixture::new();
    let lua = f.logs.join("Lua.log");
    fs::write(&lua, "old loading history\n").unwrap();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Ok(vec![]));
    let mut runner = f.direct_runner(json!({}), backend);
    runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    let before = runner.loadscreen_count().unwrap();
    append(&lua, &format!("{LOADSCREEN_READY}\n"));
    let replacement = f.logs.join("replacement.log");
    f.clock.on_sleep(move |_| {
        fs::rename(&lua, &replacement).unwrap();
        fs::write(&lua, "fresh\n").unwrap();
    });
    generation_error(
        runner
            .enter_game_after_loadscreen(before, 3.0, None, false)
            .unwrap_err(),
    );
    assert_eq!(f.trace().presses.len(), 1);
}

#[test]
fn test_transition_rejects_same_inode_rewrite_even_if_log_grows() {
    let f = Fixture::new();
    let lua = f.logs.join("Lua.log");
    fs::write(
        &lua,
        [vec![b'P'; 256], vec![b'M'; 256], vec![b'T'; 256]].concat(),
    )
    .unwrap();
    let mut runner = f.direct_runner(json!({}), f.backend());
    runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    let inode = judge::file_inode(&lua).unwrap();
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&lua)
        .unwrap();
    file.write_all(&[vec![b'P'; 256], vec![b'R'; 256], vec![b'U'; 256]].concat())
        .unwrap();
    file.write_all(LOADSCREEN_READY.as_bytes()).unwrap();
    drop(file);
    assert_eq!(judge::file_inode(&lua).unwrap(), inode);
    generation_error(
        runner
            .enter_game_after_loadscreen(0, 1.0, None, false)
            .unwrap_err(),
    );
    assert!(f.trace().presses.is_empty());
}

#[test]
fn test_run_creates_missing_output_parents_but_rejects_existing_run() {
    let mut f = Fixture::new();
    f.output = f.root.path().join("missing/nested/run");
    let mut runner = f.runner(
        scenario(json!({"action":"wait_for","condition":{"type":"state","name":"FrontEnd"}})),
        f.backend(),
    );
    runner.auto_restart = false;
    assert_eq!(runner.run().unwrap(), 0);
    error_has(runner.run().unwrap_err(), ErrorKind::Io, "exists");
    assert!(f.output.join("steps.jsonl").is_file());
}

#[test]
fn test_multiple_civ_processes_stop_run_before_tuner_access() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_require_process = Some(Box::new(|| Err(Error::input("Multiple Civ VI processes"))));
    let mut runner = f.runner(scenario(json!({"action":"pause","seconds":0.1})), backend);
    runner.auto_restart = false;
    assert_eq!(runner.run().unwrap(), 1);
    let t = f.trace();
    assert!(t.connects.is_empty());
    assert!(t.state_queries.is_empty());
    let boundary: Value =
        serde_json::from_slice(&fs::read(f.output.join("failure.json")).unwrap()).unwrap();
    assert_eq!(boundary["stage"], "preflight");
    assert_eq!(boundary["message"], "Multiple Civ VI processes");
    let result: Value =
        serde_json::from_slice(&fs::read(f.output.join("result.json")).unwrap()).unwrap();
    assert!(result["failure"]
        .as_str()
        .unwrap()
        .contains("Multiple Civ VI processes"));
    let last = f.records().pop().unwrap();
    assert_eq!(
        (last["action"].as_str(), last["status"].as_str()),
        (Some("run"), Some("FAIL"))
    );
}

#[test]
fn failure_record_write_error_keeps_original_cause_and_disconnects() {
    for preflight in [true, false] {
        let f = Fixture::new();
        let output = f.output.clone();
        let mut backend = f.backend();
        if preflight {
            backend.on_require_process = Some(Box::new(move || {
                fs::create_dir(output.join("failure.json")).unwrap();
                Err(Error::input("original process failure"))
            }));
        } else {
            backend.on_lua = Box::new(move |_, _, _| {
                fs::create_dir(output.join("failure.json")).unwrap();
                Err(Error::lua("original Lua failure"))
            });
        }
        let mut runner = f.runner(
            scenario(json!({"action":"ui_lua","state_name":"InGame","code":"return true"})),
            backend,
        );
        runner.auto_restart = false;
        assert_eq!(runner.run().unwrap(), 1);
        assert!(f.trace().disconnects > 0);
        let result: Value =
            serde_json::from_slice(&fs::read(f.output.join("result.json")).unwrap()).unwrap();
        assert!(result["failure"].as_str().unwrap().contains(if preflight {
            "original process failure"
        } else {
            "original Lua failure"
        }));
        assert_eq!(result["passed"], false);
    }
}

#[test]
fn interrupted_step_keeps_its_original_boundary_through_finalization() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_lua = Box::new(|_, _, _| Err(Error::interrupted("stop during Lua request")));
    let mut runner = f.runner(
        scenario(json!({"action":"ui_lua","state_name":"InGame","code":"return true"})),
        backend,
    );
    runner.auto_restart = false;
    assert_eq!(runner.run().unwrap_err().kind, ErrorKind::Interrupted);
    let result: Value =
        serde_json::from_slice(&fs::read(f.output.join("result.json")).unwrap()).unwrap();
    assert_eq!(result["failure_boundary"]["step"], 1);
    assert_eq!(result["failure_boundary"]["action"], "ui_lua");
    assert_eq!(result["failure_boundary"]["error_kind"], "Interrupted");
    assert!(f.trace().disconnects > 0);
}

#[test]
fn test_failed_cold_start_is_reported_before_tuner_access() {
    let f = Fixture::new();
    let mut backend = f.backend().owned_chain();
    backend.on_process = Box::new(|_| Ok(None));
    let clock = f.clock.clone();
    backend.on_launch = Some(Box::new(move || {
        clock.set(359.0);
        Ok(())
    }));
    backend.on_wait_window = Some(Box::new(|deadline| {
        assert_eq!(deadline, 360.0);
        Err(Error::scenario("Steam could not start Civ VI"))
    }));
    let mut runner = f.runner(scenario(json!({"action":"pause","seconds":0.1})), backend);
    runner.auto_restart = false;
    assert_eq!(runner.run().unwrap(), 1);
    let t = f.trace();
    assert_eq!(t.launches, 1);
    assert_eq!(t.window_deadlines, vec![360.0]);
    assert!(t.connects.is_empty());
    assert!(t.state_queries.is_empty());
    let last = f.records().pop().unwrap();
    assert_eq!(
        (last["action"].as_str(), last["status"].as_str()),
        (Some("run"), Some("FAIL"))
    );
    assert!(last["detail"]
        .as_str()
        .unwrap()
        .contains("Steam could not start Civ VI"));
    assert!(!last["detail"].as_str().unwrap().contains("Multiple"));
}

#[test]
fn test_unreachable_tuner_reports_connection_failure() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_connect = Some(Box::new(|_, _| {
        Err(Error::tuner(
            "FireTuner に接続できません: connection refused",
        ))
    }));
    let mut runner = f.direct_runner(json!({}), backend);
    error_has(
        runner.states(1.0).unwrap_err(),
        ErrorKind::Tuner,
        "connection refused",
    );
    assert_eq!(f.trace().connects.len(), 2);
}

#[test]
fn test_screenshot_paths_reject_drives_absolute_and_parent_escape() {
    for path in [
        "D:shot.png",
        "D:\\shot.png",
        "\\\\server\\share\\shot.png",
        "/absolute/shot.png",
        "\\rooted.png",
        "../shot.png",
        "folder/../shot.png",
        "folder\\..\\shot.png",
    ] {
        error_has(
            safe_relative_path(path).unwrap_err(),
            ErrorKind::Scenario,
            "relative",
        );
    }
    assert_eq!(
        safe_relative_path("screens/shot.png").unwrap(),
        Path::new("screens/shot.png")
    );
}

#[cfg(windows)]
fn link_directory(link: &Path, target: &Path) {
    if std::os::windows::fs::symlink_dir(target, link).is_ok() {
        return;
    }
    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "New-Item -ItemType Junction -Path $args[0] -Target $args[1] | Out-Null",
        ])
        .arg(link)
        .arg(target)
        .status()
        .unwrap();
    assert!(
        status.success() && link.exists(),
        "junction creation failed"
    );
}
#[cfg(unix)]
fn link_directory(link: &Path, target: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[test]
fn test_screenshot_rejects_resolved_path_outside_output() {
    let f = Fixture::new();
    let outside = f.root.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let mut runner = f.direct_runner(json!({}), f.backend());
    link_directory(&f.output.join("link"), &outside);
    error_has(
        runner
            .run_step(&json!({"action":"screenshot","path":"link/shot.png"}))
            .unwrap_err(),
        ErrorKind::Scenario,
        "escapes output",
    );
    assert!(f.trace().captures.is_empty());
}

#[test]
fn test_screenshot_fails_if_capture_finishes_after_deadline() {
    let f = Fixture::new();
    let mut backend = f.backend();
    f.clock.set(10.0);
    let clock = f.clock.clone();
    backend.on_capture = Some(Box::new(move |_, _, deadline| {
        assert!((deadline - 10.01).abs() < 1e-9);
        clock.set(deadline + 0.01);
        Ok(())
    }));
    let mut runner = f.direct_runner(json!({}), backend);
    error_has(
        runner
            .run_step(&json!({"action":"screenshot","path":"shot.png","timeout":0.01}))
            .unwrap_err(),
        ErrorKind::Scenario,
        "deadline expired",
    );
    assert_eq!(f.trace().captures.len(), 1);
}

#[test]
fn screenshot_capture_only_avoids_frontend_tuner_queries() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Err(Error::tuner("unexpected tuner query")));
    backend.on_capture = Some(Box::new(|_, _, _| Ok(())));
    let mut runner = f.direct_runner(json!({}), backend);
    runner
        .run_step(&json!({"action":"screenshot","path":"frontend.png","capture_only":true}))
        .unwrap();
    let trace = f.trace();
    assert_eq!(trace.captures.len(), 1);
    assert!(trace.state_queries.is_empty());
    assert!(trace.lua.is_empty());
    error_has(
        validate_scenario(&scenario(json!({
            "action":"screenshot","path":"frontend.png","capture_only":"true"
        })))
        .unwrap_err(),
        ErrorKind::Scenario,
        "capture_only must be boolean",
    );
}

#[test]
fn test_nonfinite_timeouts_are_rejected() {
    for value in [json!(true), json!(0), json!(-1), json!("2"), Value::Null] {
        error_has(
            validate_scenario(&scenario(
                json!({"action":"autoplay","turns":1,"timeout":value}),
            ))
            .unwrap_err(),
            ErrorKind::Scenario,
            "finite positive",
        );
    }
    assert!(serde_json::Number::from_f64(f64::INFINITY).is_none());
    assert!(serde_json::Number::from_f64(f64::NEG_INFINITY).is_none());
    assert!(serde_json::Number::from_f64(f64::NAN).is_none());
    assert!(serde_json::from_str::<Value>("{\"timeout\":1e999}").is_err());
    assert!(
        serde_json::from_str::<Value>(&format!("{{\"timeout\":1{}}}", "0".repeat(400))).is_err()
    );
    validate_scenario(&scenario(
        json!({"action":"autoplay","turns":1,"timeout":1.5}),
    ))
    .unwrap();
}

#[test]
fn test_scenario_validates_lua_code_before_running() {
    let max = civ6_autotest_rs::tuner::MAX_LUA_CODE_BYTES;
    for action in ["ui_lua", "wait_for"] {
        for (code, message) in [
            ("return\0true".to_string(), "NUL"),
            ("界".repeat(max / 3 + 1), "too large"),
        ] {
            let step = if action == "ui_lua" {
                json!({"action":"ui_lua","state_name":"InGame","code":code})
            } else {
                json!({"action":"wait_for","condition":{"type":"lua","state_name":"InGame","code":code}})
            };
            let error = validate_scenario(&scenario(step)).unwrap_err();
            error_has(error, ErrorKind::Scenario, "Step 1:");
            let error = validate_scenario(&scenario(if action == "ui_lua" {
                json!({"action":"ui_lua","state_name":"InGame","code":code})
            } else {
                json!({"action":"wait_for","condition":{"type":"lua","state_name":"InGame","code":code}})
            })).unwrap_err();
            assert!(error.message.contains(message), "{error}");
        }
    }
    validate_scenario(&scenario(
        json!({"action":"ui_lua","state_name":"InGame","code":"a".repeat(max)}),
    ))
    .unwrap();
}

#[test]
fn test_manual_wait_has_no_step_deadline_or_timeout_option() {
    let step = json!({"action":"manual","prompt":"Inspect the game"});
    validate_scenario(&scenario(step.clone())).unwrap();
    error_has(
        validate_scenario(&scenario(
            json!({"action":"manual","prompt":"Inspect the game","timeout":1}),
        ))
        .unwrap_err(),
        ErrorKind::Scenario,
        "manual waits for Enter",
    );
    let f = Fixture::new();
    let mut runner = f.direct_runner(json!({}), f.backend());
    assert_eq!(runner.run_step(&step).unwrap(), "Inspect the game");
    assert_eq!(f.trace().manuals, vec!["Inspect the game"]);
    assert_eq!(f.clock.reads(), 0);
}

#[test]
fn test_wait_caps_state_and_lua_queries_to_remaining_time() {
    let f = Fixture::new();
    let mut runner = f.direct_runner(json!({}), f.backend());
    runner
        .wait(&json!({"type":"state","name":"InGame"}), 0.1, false)
        .unwrap();
    runner
        .wait(
            &json!({"type":"lua","state_name":"InGame","code":"return true"}),
            0.1,
            false,
        )
        .unwrap();
    let t = f.trace();
    assert_eq!(t.state_queries.len(), 2);
    assert_eq!(t.lua.len(), 1);
    for timeout in t
        .state_queries
        .iter()
        .chain(t.lua.iter().map(|(_, _, timeout)| timeout))
    {
        assert!(
            *timeout > 0.0 && *timeout <= 0.100001,
            "uncapped timeout: {timeout}"
        );
    }
}

#[test]
fn test_lua_wait_unknown_reply_fails_without_reexecution() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Ok(vec!["InGame".into(), "DiplomacyActionView".into()]));
    backend.on_lua = Box::new(|_, _, _| Err(Error::unknown("command may have run")));
    let mut runner = f.direct_runner(json!({}), backend);
    error_has(
        runner
            .wait(
                &json!({"type":"lua","state_name":"InGame","code":"return true"}),
                30.0,
                false,
            )
            .unwrap_err(),
        ErrorKind::LuaResponseUnknown,
        "may have run",
    );
    let t = f.trace();
    assert_eq!(
        t.lua
            .iter()
            .filter(|(state, _, _)| state == "InGame")
            .count(),
        1
    );
    assert_eq!(t.state_queries.len(), 3);
    assert_eq!(
        t.lua
            .iter()
            .map(|(state, _, _)| state.as_str())
            .collect::<Vec<_>>(),
        vec!["InGame", "DiplomacyActionView"]
    );
    assert!(f.clock.sleeps().is_empty());
}

#[test]
fn test_lua_wait_confirmed_error_fails_without_reexecution() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_lua = Box::new(|_, _, _| Err(Error::lua("Lua error in InGame: boom")));
    let mut runner = f.direct_runner(json!({}), backend);
    error_has(
        runner
            .wait(
                &json!({"type":"lua","state_name":"InGame","code":"error('boom')"}),
                30.0,
                false,
            )
            .unwrap_err(),
        ErrorKind::LuaExecution,
        "boom",
    );
    assert_eq!(f.trace().lua.len(), 1);
    assert!(f.clock.sleeps().is_empty());
}

#[test]
fn test_pre_send_framing_error_is_logged_as_step_failure() {
    for step in [
        json!({"action":"ui_lua","state_name":"InGame","code":"return true"}),
        json!({"action":"wait_for","condition":{"type":"lua","state_name":"InGame","code":"return true"}}),
    ] {
        let f = Fixture::new();
        let mut backend = f.backend();
        backend.on_lua = Box::new(|_, code, _| {
            if code.contains("UI.IsInFrontEnd()") {
                Ok("true".into())
            } else {
                Err(Error::input("FireTuner command is too large"))
            }
        });
        let action = step["action"].as_str().unwrap().to_string();
        let mut runner = f.runner(scenario(step), backend);
        runner.auto_restart = false;
        assert_eq!(runner.run().unwrap(), 1);
        assert_eq!(lua_count(&f, "return true"), 1);
        let last = f.records().pop().unwrap();
        assert_eq!(last["action"], format!("1/1 {action}"));
        assert_eq!(last["status"], "FAIL");
        assert_eq!(last["detail"], "FireTuner command is too large");
    }
}

#[test]
fn test_transition_pre_send_failure_is_bounded_by_the_step_deadline() {
    for action in ["start_game", "load"] {
        let f = Fixture::new();
        let mut backend = f.backend();
        backend.on_lua = Box::new(|_, code, _| {
            if code.contains("Network.HostGame") || code.contains("Network.LoadGame") {
                Err(Error::tuner("pre-send connection failed"))
            } else {
                Ok("true".into())
            }
        });
        let step = if action == "load" {
            fs::write(f.saves.join("existing.Civ6Save"), b"save").unwrap();
            json!({"action":"load","name":"existing","exact_name":true,"timeout":2})
        } else {
            json!({"action":"start_game","timeout":2})
        };
        let mut runner = f.runner(scenario(step), backend);
        runner.auto_restart = false;
        assert_eq!(runner.run().unwrap(), 1);
        let needle = if action == "load" {
            "Network.LoadGame"
        } else {
            "Network.HostGame"
        };
        assert!(lua_count(&f, needle) > 1);
        assert!(f.clock.now() <= 2.0);
        assert!(f.trace().presses.is_empty());
        let last = f.records().pop().unwrap();
        assert_eq!(last["status"], "FAIL");
        assert_eq!(last["detail"], "pre-send connection failed");
    }
}

#[test]
fn test_transition_reconnects_before_send_and_sends_once_after_recovery() {
    for action in ["start_game", "load"] {
        let f = Fixture::new();
        let mut backend = f.backend();
        let mut connects = 0;
        backend.on_connect = Some(Box::new(move |_, _| {
            connects += 1;
            if connects <= 2 {
                Err(Error::tuner("connection refused before send"))
            } else {
                Ok(())
            }
        }));
        let lua = f.logs.join("Lua.log");
        backend.on_lua = Box::new(move |_, code, _| {
            if code.contains("Network.HostGame") || code.contains("Network.LoadGame") {
                append(&lua, &format!("{LOADSCREEN_READY}\n"));
            }
            Ok("true".into())
        });
        let step = if action == "load" {
            fs::write(f.saves.join("existing.Civ6Save"), b"save").unwrap();
            json!({"action":"load","name":"existing","exact_name":true,"timeout":5})
        } else {
            json!({"action":"start_game","timeout":5})
        };
        let mut runner = f.direct_runner(json!({}), backend);
        runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
        runner.run_step(&step).unwrap();
        let needle = if action == "load" {
            "Network.LoadGame"
        } else {
            "Network.HostGame"
        };
        assert_eq!(lua_count(&f, needle), 1);
        assert!(f.clock.now() > 0.0 && f.clock.now() < 5.0);
    }
}

#[test]
fn test_transition_waits_after_unknown_reply_without_resending() {
    for action in ["start_game", "load"] {
        let f = Fixture::new();
        let mut backend = f.backend();
        let lua = f.logs.join("Lua.log");
        backend.on_lua = Box::new(move |_, code, _| {
            if code.contains("Network.HostGame") || code.contains("Network.LoadGame") {
                append(&lua, &format!("{LOADSCREEN_READY}\n"));
                Err(Error::unknown("command may have run"))
            } else {
                Ok("true".into())
            }
        });
        let step = if action == "load" {
            fs::write(f.saves.join("existing.Civ6Save"), b"save").unwrap();
            json!({"action":"load","name":"existing","exact_name":true})
        } else {
            json!({"action":"start_game"})
        };
        let mut runner = f.direct_runner(json!({}), backend);
        runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
        runner.run_step(&step).unwrap();
        let needle = if action == "load" {
            "Network.LoadGame"
        } else {
            "Network.HostGame"
        };
        assert_eq!(lua_count(&f, needle), 1);
        assert_eq!(f.trace().presses.len(), 1);
        assert_eq!(lua_count(&f, "Game.GetLocalPlayer"), 1);
    }
}

#[test]
fn test_unknown_transition_without_read_only_confirmation_is_undecidable() {
    let f = Fixture::new();
    let mut runner = f.direct_runner(json!({}), f.backend());
    runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    error_has(
        runner
            .enter_game_after_loadscreen(0, 0.02, None, true)
            .unwrap_err(),
        ErrorKind::Scenario,
        "outcome undecidable",
    );
    assert!(f.trace().presses.is_empty());
}

#[test]
fn test_confirmed_transition_lua_error_never_enters_wait() {
    for action in ["start_game", "load"] {
        let f = Fixture::new();
        let mut backend = f.backend();
        backend.on_lua = Box::new(|_, code, _| {
            if code.contains("Network.HostGame") || code.contains("Network.LoadGame") {
                Err(Error::lua("Lua ERR"))
            } else {
                Ok("true".into())
            }
        });
        let step = if action == "load" {
            fs::write(f.saves.join("existing.Civ6Save"), b"save").unwrap();
            json!({"action":"load","name":"existing","exact_name":true})
        } else {
            json!({"action":"start_game"})
        };
        let mut runner = f.direct_runner(json!({}), backend);
        runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
        error_has(
            runner.run_step(&step).unwrap_err(),
            ErrorKind::LuaExecution,
            "Lua ERR",
        );
        let needle = if action == "load" {
            "Network.LoadGame"
        } else {
            "Network.HostGame"
        };
        assert_eq!(lua_count(&f, needle), 1);
        assert!(f.trace().presses.is_empty());
    }
}

#[test]
fn test_exit_to_menu_waits_for_state_before_lua_predicate() {
    let f = Fixture::new();
    let mut runner = f.direct_runner(json!({}), f.backend());
    assert_eq!(
        runner.run_step(&json!({"action":"exit_to_menu"})).unwrap(),
        "main menu reached"
    );
    let t = f.trace();
    assert_eq!(t.lua.len(), 2);
    assert!(t.lua[0].1.contains("Events.ExitToMainMenu"));
    assert!(t.lua[1].1.contains("UI.IsInFrontEnd()"));
    assert_eq!(t.disconnects, 1);
    let after_disconnect = t
        .events
        .iter()
        .position(|event| event == "disconnect")
        .unwrap();
    let state_after = t.events[after_disconnect + 1..]
        .iter()
        .position(|event| event == "states")
        .unwrap();
    let predicate_after = t.events[after_disconnect + 1..]
        .iter()
        .position(|event| event == "lua:FrontEnd")
        .unwrap();
    assert!(state_after < predicate_after);
}

#[test]
fn test_log_wait_requires_new_line_and_does_not_reuse_a_match() {
    let f = Fixture::new();
    let lua = f.logs.join("Lua.log");
    fs::write(&lua, "[JAC_POC] notebooks opened\n").unwrap();
    let mut runner = f.direct_runner(json!({}), f.backend());
    runner.log_step_baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    let condition = json!({"type":"log_line","log":"Lua.log","contains":"notebooks opened"});
    error_has(
        runner.wait(&condition, 0.02, false).unwrap_err(),
        ErrorKind::Scenario,
        "Timed out",
    );
    append(&lua, "[JAC_POC] notebooks opened\n");
    runner.wait(&condition, 0.1, false).unwrap();
    assert_eq!(
        runner
            .log_matches_consumed
            .get(&("Lua.log".into(), "notebooks opened".into())),
        Some(&1)
    );
    error_has(
        runner.wait(&condition, 0.02, false).unwrap_err(),
        ErrorKind::Scenario,
        "Timed out",
    );
}

#[test]
fn test_log_wait_stops_if_generation_changes() {
    let f = Fixture::new();
    for name in ["Lua.log", "Database.log", "Modding.log"] {
        fs::write(f.logs.join(name), "old history\n").unwrap();
    }
    let mut runner = f.direct_runner(json!({}), f.backend());
    runner.log_step_baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    fs::write(f.logs.join("Lua.log"), "match\n").unwrap();
    generation_error(
        runner
            .wait(
                &json!({"type":"log_line","log":"Lua.log","contains":"match"}),
                1.0,
                false,
            )
            .unwrap_err(),
    );
    assert!(f.trace().lua.is_empty());
}

#[test]
fn test_later_step_cannot_reuse_previous_steps_log_line() {
    let f = Fixture::new();
    let mut backend = f.backend();
    let lua = f.logs.join("Lua.log");
    let emitted = Arc::new(Mutex::new(false));
    let emitted_for_lua = emitted.clone();
    backend.on_lua = Box::new(move |state, code, _| {
        if state == "InGame" && code == "return true" {
            let mut sent = emitted_for_lua.lock().unwrap();
            if !*sent {
                append(&lua, "ready once\n");
                *sent = true;
            }
        }
        Ok("true".into())
    });
    let trigger = json!({"action":"ui_lua","state_name":"InGame","code":"return true"});
    let condition = json!({"type":"log_line","log":"Lua.log","contains":"ready once"});
    let mut runner = f.runner(
        json!({"name":"log age","steps":[
            trigger, {"action":"wait_for","condition":condition,"timeout":1},
            trigger, {"action":"wait_for","condition":condition,"timeout":1}
        ]}),
        backend,
    );
    runner.auto_restart = false;
    assert_eq!(runner.run().unwrap(), 1);
    let statuses: Vec<_> = f
        .records()
        .iter()
        .map(|record| {
            (
                record["action"].as_str().unwrap().to_string(),
                record["status"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert!(statuses.contains(&("2/4 wait_for".into(), "OK".into())));
    assert_eq!(
        statuses.last(),
        Some(&("4/4 wait_for".into(), "FAIL".into()))
    );
    assert_eq!(f.trace().launches, 0);
}

#[test]
fn test_oserror_after_autoplay_start_attempts_cleanup_without_replay() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_lua = Box::new(|_, code, _| {
        if code.contains("AutoplayManager.SetActive(true)") {
            Err(Error::new(ErrorKind::Io, "connection reset"))
        } else {
            Ok("true".into())
        }
    });
    let mut runner = f.direct_runner(json!({}), backend);
    error_has(
        runner
            .autoplay(3, 10.0, AutoplayOptions::default())
            .unwrap_err(),
        ErrorKind::Io,
        "connection reset",
    );
    assert_eq!(lua_count(&f, "AutoplayManager.SetActive(true)"), 1);
    assert_eq!(lua_count(&f, "AutoplayManager.SetActive(false)"), 1);
}

#[test]
fn interrupted_autoplay_reconciles_the_owner_after_a_start_attempt() {
    for after_start in [true, false] {
        let f = Fixture::new();
        let mut backend = f.backend();
        backend.on_lua = Box::new(move |_, code, _| {
            if code.starts_with("return JAC_AUTOTEST_AUTO_OWNER ==") {
                return Ok(after_start.to_string());
            }
            if code.contains("return tostring(startTurn)") {
                return if after_start {
                    Ok("1|0".into())
                } else {
                    Err(Error::interrupted("cancelled before acknowledgment"))
                };
            }
            if code.contains("tostring(AutoplayManager.IsActive())") {
                return Err(Error::interrupted("cancelled during autoplay"));
            }
            Ok("true".into())
        });
        let mut runner = f.direct_runner(json!({}), backend);
        assert_eq!(
            runner
                .autoplay(3, 10.0, AutoplayOptions::default())
                .unwrap_err()
                .kind,
            ErrorKind::Interrupted
        );
        assert_eq!(lua_count(&f, "SetActive(true)"), 1);
        assert_eq!(lua_count(&f, "SetActive(false)"), usize::from(after_start));
    }
}

#[test]
fn autoplay_lua_errors_reconcile_attempted_start_and_stop_once() {
    for fail_at in ["start", "status"] {
        for kind in [ErrorKind::LuaResponseUnknown, ErrorKind::LuaExecution] {
            let f = Fixture::new();
            let mut backend = f.backend();
            backend.on_lua = Box::new(move |_, code, _| {
                if fail_at == "start" && code.contains("SetActive(true)") {
                    return Err(Error::new(kind, "autoplay may have run"));
                }
                if code.contains("return tostring(startTurn)") {
                    return Ok("1|0".into());
                }
                if fail_at == "status" && code.contains("tostring(AutoplayManager.IsActive())") {
                    return Err(Error::new(kind, "autoplay may have run"));
                }
                Ok("true".into())
            });
            let mut runner = f.direct_runner(json!({}), backend);
            error_has(
                runner
                    .autoplay(3, 10.0, AutoplayOptions::default())
                    .unwrap_err(),
                kind,
                "may have run",
            );
            assert_eq!(lua_count(&f, "SetActive(true)"), 1);
            assert_eq!(lua_count(&f, "SetActive(false)"), 1);
        }
    }
}

fn check_autoplay_start_cleanup(
    start_error: ErrorKind,
    foreign_active: bool,
    lose_owner_reply: bool,
    lose_stop_reply: bool,
) {
    #[derive(Default)]
    struct State {
        active: bool,
        owner: Option<String>,
        handler: bool,
        starts: i64,
        stops: i64,
        removals: i64,
    }
    let state = Arc::new(Mutex::new(State {
        active: foreign_active,
        owner: foreign_active.then(|| "another-run".into()),
        handler: foreign_active,
        ..State::default()
    }));
    let f = Fixture::new();
    let mut backend = f.backend();
    let observed = state.clone();
    let clock = f.clock.clone();
    backend.on_lua = Box::new(move |_, code, timeout| {
        if !code.contains("JAC_AUTOTEST_AUTO_OWNER") {
            return Ok("true".into());
        }
        let mut s = observed.lock().unwrap();
        let lua = mlua::Lua::new();
        let g = lua.globals();
        g.set("active", s.active).unwrap();
        g.set("JAC_AUTOTEST_AUTO_OWNER", s.owner.clone()).unwrap();
        g.set("has_handler", s.handler).unwrap();
        g.set("starts", s.starts).unwrap();
        g.set("stops", s.stops).unwrap();
        g.set("removals", s.removals).unwrap();
        lua.load(
            r#"
Game = {GetCurrentGameTurn=function() return 1 end, GetLocalPlayer=function() return 0 end}
PlayerTypes = {NONE=-1}
LuaEvents = {AutoPlayEnd={Add=function(_) end, Remove=function(_) removals=removals+1 end}}
AutoplayManager = {
 IsActive=function() return active end,
 SetTurns=function(_) end, SetReturnAsPlayer=function(_) end, SetObserveAsPlayer=function(_) end,
 SetActive=function(v) active=v; if v then starts=starts+1 else stops=stops+1 end end
}
if has_handler then JAC_AUTOTEST_AUTO_HANDLER=function() end end
"#,
        )
        .exec()
        .unwrap();
        // Execute the runner's actual command before dropping its reply. The
        // game-side changes survive even when Rust never acknowledges them.
        let result = lua.load(code).eval::<mlua::Value>();
        s.active = g.get("active").unwrap();
        s.owner = g.get("JAC_AUTOTEST_AUTO_OWNER").unwrap();
        s.handler = matches!(
            g.get::<mlua::Value>("JAC_AUTOTEST_AUTO_HANDLER").unwrap(),
            mlua::Value::Function(_)
        );
        s.starts = g.get("starts").unwrap();
        s.stops = g.get("stops").unwrap();
        s.removals = g.get("removals").unwrap();
        let value = result.map_err(|e| Error::new(ErrorKind::LuaExecution, e.to_string()))?;
        if code.contains("return tostring(startTurn)") {
            return Err(Error::new(start_error, "start reply lost after execution"));
        }
        if (lose_owner_reply && code.starts_with("return JAC_AUTOTEST_AUTO_OWNER =="))
            || (lose_stop_reply && code.contains("SetActive(false)"))
        {
            clock.advance(timeout);
            return Err(Error::unknown("cleanup reply lost after execution"));
        }
        Ok(match value {
            mlua::Value::Boolean(v) => v.to_string(),
            mlua::Value::String(v) => v.to_str().unwrap().to_string(),
            other => panic!("unexpected Lua result: {other:?}"),
        })
    });
    let mut runner = f.direct_runner(json!({}), backend);
    let error = runner
        .autoplay(3, 10.0, AutoplayOptions::default())
        .unwrap_err();
    assert_eq!(
        error.kind,
        if foreign_active {
            ErrorKind::LuaExecution
        } else {
            start_error
        }
    );
    if !foreign_active {
        assert!(error.message.contains("start reply lost after execution"));
    }
    let s = state.lock().unwrap();
    assert_eq!(s.active, foreign_active);
    assert_eq!(s.starts, i64::from(!foreign_active));
    assert_eq!(s.stops, i64::from(!foreign_active));
    assert_eq!(s.removals, i64::from(!foreign_active));
    assert_eq!(s.handler, foreign_active);
    assert_eq!(s.owner.as_deref(), foreign_active.then_some("another-run"));
    assert_eq!(lua_count(&f, "SetActive(true)"), 1);
    assert_eq!(
        lua_count(&f, "SetActive(false)"),
        usize::from(!foreign_active)
    );
}

#[test]
fn lost_autoplay_start_reply_stops_the_executed_lua_once() {
    for kind in [
        ErrorKind::LuaResponseUnknown,
        ErrorKind::LuaExecution,
        ErrorKind::Interrupted,
    ] {
        check_autoplay_start_cleanup(kind, false, false, false);
    }
}

#[test]
fn autoplay_cleanup_lost_replies_do_not_replay_start_or_stop() {
    for lose_owner in [false, true] {
        for lose_stop in [false, true] {
            check_autoplay_start_cleanup(
                ErrorKind::LuaResponseUnknown,
                false,
                lose_owner,
                lose_stop,
            );
        }
    }
}

#[test]
fn rejected_autoplay_start_preserves_another_runs_active_game_and_handler() {
    check_autoplay_start_cleanup(ErrorKind::LuaExecution, true, false, false);
}

#[test]
fn natural_disaster_report_releases_turn_playback_only_when_visible() {
    for hidden in [false, true] {
        let f = Fixture::new();
        let mut backend = f.backend();
        let state = Arc::new(Mutex::new((false, 0_usize)));
        backend.on_states = Box::new(|_| Ok(vec!["InGame".into(), "NaturalDisasterPopup".into()]));
        let state_for_lua = state.clone();
        backend.on_lua = Box::new(move |name, code, _| {
            if name == "NaturalDisasterPopup" {
                let closed = state_for_lua.lock().unwrap().1 > 0;
                let lua = mlua::Lua::new();
                lua.load(format!(
                    "hidden={}; closes=0; ContextPtr={{IsHidden=function() return hidden end}}; \
                     OnClose=function() if hidden then error('hidden report closed') end \
                     closes=closes+1; hidden=true end",
                    hidden || closed
                ))
                .exec()
                .unwrap();
                let result: bool = lua.load(code).eval().unwrap();
                state_for_lua.lock().unwrap().1 += lua.globals().get::<usize>("closes").unwrap();
                return Ok(result.to_string());
            }
            if code == "return Game.GetCurrentGameTurn()" {
                return Ok("7".into());
            }
            if code.contains("ACTION_ENDTURN") {
                state_for_lua.lock().unwrap().0 = true;
            }
            if code.contains("== 8") {
                let (sent, closes) = *state_for_lua.lock().unwrap();
                return Ok((sent && (hidden || closes == 1)).to_string());
            }
            Ok("true".into())
        });
        let mut runner = f.direct_runner(json!({}), backend);
        assert_eq!(runner.end_turn(10.0, None).unwrap(), "human turn 7->8");
        assert_eq!(*state.lock().unwrap(), (true, usize::from(!hidden)));
        assert_eq!(lua_count(&f, "ACTION_ENDTURN"), 1);
    }
}

#[test]
fn test_end_turn_closes_successive_ai_statements_without_repeating_action() {
    let f = Fixture::new();
    let mut backend = f.backend();
    let state = Arc::new(Mutex::new((false, 0_usize)));
    let state_for_states = state.clone();
    backend.on_states = Box::new(move |_| {
        let (action_sent, closed) = *state_for_states.lock().unwrap();
        let mut states = vec!["InGame".into()];
        if action_sent && closed < 2 {
            states.push("DiplomacyActionView".into());
        }
        Ok(states)
    });
    let state_for_lua = state.clone();
    backend.on_lua = Box::new(move |name, code, _| {
        if code == "return Game.GetCurrentGameTurn()" {
            return Ok("7".into());
        }
        if code.contains("ACTION_ENDTURN") {
            state_for_lua.lock().unwrap().0 = true;
        }
        if name == "DiplomacyActionView" && code.contains("OnClose()") {
            state_for_lua.lock().unwrap().1 += 1;
        }
        if code.contains("== 8") {
            return Ok((state_for_lua.lock().unwrap().1 == 2).to_string());
        }
        Ok("true".into())
    });
    let mut runner = f.direct_runner(json!({}), backend);
    assert_eq!(runner.end_turn(10.0, None).unwrap(), "human turn 7->8");
    assert_eq!(*state.lock().unwrap(), (true, 2));
    assert_eq!(lua_count(&f, "ACTION_ENDTURN"), 1);
    assert_eq!(
        f.records()
            .iter()
            .filter(|record| record["action"] == "end_turn_diplomacy")
            .count(),
        2
    );
}

#[test]
fn test_end_turn_reopening_diplomacy_has_a_fixed_limit() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Ok(vec!["InGame".into(), "DiplomacyActionView".into()]));
    backend.on_lua = Box::new(|name, _, _| Ok((name == "DiplomacyActionView").to_string()));
    let mut runner = f.direct_runner(json!({}), backend);
    error_has(
        runner
            .wait(
                &json!({"type":"lua","state_name":"InGame","code":"return false"}),
                20.0,
                true,
            )
            .unwrap_err(),
        ErrorKind::Scenario,
        "keep reopening",
    );
    assert_eq!(lua_count(&f, "OnClose()"), 26);
    assert_eq!(
        f.records()
            .iter()
            .filter(|record| record["action"] == "end_turn_diplomacy")
            .count(),
        26
    );
}

#[test]
fn test_end_turn_honours_turns_and_rejects_invalid_values() {
    let f = Fixture::new();
    let mut backend = f.backend();
    let turn = Arc::new(Mutex::new(7_i64));
    let turn_for_lua = turn.clone();
    backend.on_lua = Box::new(move |_, code, _| {
        if code == "return Game.GetCurrentGameTurn()" {
            return Ok(turn_for_lua.lock().unwrap().to_string());
        }
        if code.contains("ACTION_ENDTURN") {
            *turn_for_lua.lock().unwrap() += 1;
        }
        Ok("true".into())
    });
    let mut runner = f.direct_runner(json!({}), backend);
    runner
        .run_step(&json!({"action":"end_turn","turns":3,"timeout":60}))
        .unwrap();
    assert_eq!(lua_count(&f, "ACTION_ENDTURN"), 3);
    runner
        .run_step(&json!({"action":"end_turn","timeout":60}))
        .unwrap();
    assert_eq!(lua_count(&f, "ACTION_ENDTURN"), 4);
    for turns in [json!(0), json!(-1), json!("2"), json!(1.5)] {
        error_has(
            validate_scenario(&scenario(json!({"action":"end_turn","turns":turns}))).unwrap_err(),
            ErrorKind::Scenario,
            "turns must be positive",
        );
    }
}

#[test]
fn test_step_deadline_prevents_late_autoplay_and_end_turn_side_effects() {
    let f = Fixture::new();
    let mut backend = f.backend();
    let clock = f.clock.clone();
    backend.on_lua = Box::new(move |_, code, _| {
        if code.contains("JAC_Notebooks_Close") {
            clock.advance(0.025);
        }
        Ok("true".into())
    });
    let mut runner = f.direct_runner(json!({}), backend);
    error_has(
        runner
            .autoplay(3, 0.01, AutoplayOptions::default())
            .unwrap_err(),
        ErrorKind::Scenario,
        "deadline",
    );
    error_has(
        runner.end_turn(0.01, None).unwrap_err(),
        ErrorKind::Scenario,
        "deadline",
    );
    assert_eq!(lua_count(&f, "JAC_Notebooks_Close"), 2);
    assert_eq!(lua_count(&f, "AutoplayManager.SetActive(true)"), 0);
    assert_eq!(lua_count(&f, "UI.RequestAction"), 0);
}
