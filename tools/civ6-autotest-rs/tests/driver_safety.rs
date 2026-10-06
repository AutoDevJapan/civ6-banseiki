mod support_safety;

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
    assert!(t.presses.is_empty());
}

#[test]
fn unknown_begin_game_reply_waits_for_readiness_without_resending_activation() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Ok(vec!["LoadScreen".into(), "InGame".into()]));
    let mut reads = 0;
    backend.on_lua = Box::new(move |_, code, _| {
        if code.contains("OnActivateButtonClicked()") {
            Err(Error::unknown("begin game acknowledgment lost"))
        } else if code.contains("Game.GetLocalPlayer") {
            reads += 1;
            Ok(if reads == 1 { "false" } else { "true" }.into())
        } else {
            Ok("true".into())
        }
    });
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    runner
        .enter_game_after_loadscreen(0, 5.0, Some(5.0), false)
        .unwrap();
    assert_eq!(lua_count(&f, "OnActivateButtonClicked()"), 1);
    assert_eq!(lua_count(&f, "Game.GetLocalPlayer"), 2);
    assert!(f.trace().presses.is_empty());
    let evidence = transition_observations(&f);
    let final_row = evidence.last().unwrap();
    assert_eq!(final_row["phase"], "ingame_ready");
    assert_eq!(
        final_row["observation"]["begin_game_response"]["status"],
        "unknown"
    );
    assert!(final_row["observation"]["begin_game_response"]["error"]
        .as_str()
        .unwrap()
        .contains("acknowledgment lost"));
}

#[test]
fn unknown_begin_game_reply_without_readiness_keeps_original_deadline_and_failure() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Ok(vec!["LoadScreen".into(), "InGame".into()]));
    backend.on_lua = Box::new(|_, code, _| {
        if code.contains("OnActivateButtonClicked()") {
            Err(Error::unknown("begin game acknowledgment lost"))
        } else {
            Ok("false".into())
        }
    });
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    let error = runner
        .enter_game_after_loadscreen(0, 600.0, Some(3.0), false)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Scenario);
    assert!(error.message.contains("outcome undecidable"), "{error}");
    assert!(error.message.contains("acknowledgment lost"), "{error}");
    assert_eq!(f.clock.now(), 3.0);
    assert_eq!(lua_count(&f, "OnActivateButtonClicked()"), 1);
    assert!(f.trace().presses.is_empty());
    let evidence = transition_observations(&f);
    assert_eq!(evidence.last().unwrap()["phase"], "timeout");
    assert_eq!(
        evidence.last().unwrap()["observation"]["begin_game_sent"],
        true
    );
    assert!(evidence.iter().all(|row| row["deadline"] == 3.0));
}

#[test]
fn confirmed_begin_game_lua_error_still_fails_without_readiness_or_resend() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Ok(vec!["LoadScreen".into(), "InGame".into()]));
    backend.on_lua = Box::new(|_, code, _| {
        assert!(code.contains("OnActivateButtonClicked()"));
        Err(Error::lua("begin game callback failed"))
    });
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    error_has(
        runner
            .enter_game_after_loadscreen(0, 5.0, Some(5.0), false)
            .unwrap_err(),
        ErrorKind::LuaExecution,
        "callback failed",
    );
    assert_eq!(lua_count(&f, "OnActivateButtonClicked()"), 1);
    assert_eq!(lua_count(&f, "Game.GetLocalPlayer"), 0);
    assert!(f.trace().presses.is_empty());
}

#[test]
fn unknown_readiness_reply_is_rechecked_without_repeating_begin_game() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Ok(vec!["LoadScreen".into(), "InGame".into()]));
    let mut reads = 0;
    backend.on_lua = Box::new(move |_, code, _| {
        if code.contains("Game.GetLocalPlayer") {
            reads += 1;
            if reads == 1 {
                return Err(Error::unknown("readiness reply lost"));
            }
        }
        Ok("true".into())
    });
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    runner
        .enter_game_after_loadscreen(0, 5.0, Some(5.0), false)
        .unwrap();
    assert_eq!(lua_count(&f, "OnActivateButtonClicked()"), 1);
    assert_eq!(lua_count(&f, "Game.GetLocalPlayer"), 2);
    assert!(f.trace().presses.is_empty());
    let evidence = transition_observations(&f);
    assert!(evidence.iter().any(|row| row["observation"]["last_error"]
        .as_str()
        .is_some_and(|error| error.contains("readiness reply lost"))));
    assert_eq!(evidence.last().unwrap()["phase"], "ingame_ready");
}

#[test]
fn permanently_unknown_readiness_reply_times_out_without_repeating_activation() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Ok(vec!["LoadScreen".into(), "InGame".into()]));
    backend.on_lua = Box::new(|_, code, _| {
        if code.contains("Game.GetLocalPlayer") {
            Err(Error::unknown("readiness reply lost"))
        } else {
            Ok("true".into())
        }
    });
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    let error = runner
        .enter_game_after_loadscreen(0, 600.0, Some(3.0), false)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Scenario);
    assert!(error.message.contains("readiness reply lost"), "{error}");
    assert_eq!(f.clock.now(), 3.0);
    assert_eq!(lua_count(&f, "OnActivateButtonClicked()"), 1);
    assert!(f.trace().presses.is_empty());
    assert_eq!(
        transition_observations(&f).last().unwrap()["phase"],
        "timeout"
    );
}

#[test]
fn confirmed_advisor_error_still_fails_once_after_readiness_confirmation() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| {
        Ok(vec![
            "LoadScreen".into(),
            "InGame".into(),
            "AdvisorPopup".into(),
        ])
    });
    backend.on_lua = Box::new(|state, _, _| {
        if state == "AdvisorPopup" {
            Err(Error::lua("advisor dismissal failed"))
        } else {
            Ok("true".into())
        }
    });
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    error_has(
        runner
            .enter_game_after_loadscreen(0, 5.0, Some(5.0), false)
            .unwrap_err(),
        ErrorKind::LuaExecution,
        "advisor dismissal failed",
    );
    assert_eq!(lua_count(&f, "OnActivateButtonClicked()"), 1);
    assert_eq!(lua_count(&f, "Game.GetLocalPlayer"), 1);
    assert_eq!(
        f.trace()
            .lua
            .iter()
            .filter(|call| call.0 == "AdvisorPopup")
            .count(),
        1
    );
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
    assert!(!f.trace().state_queries.is_empty());
    assert!(
        f.trace().lua.is_empty(),
        "pre-marker observation must be read-only"
    );
    assert!(f.trace().presses.is_empty());
}

fn transition_observations(f: &Fixture) -> Vec<Value> {
    fs::read_to_string(f.output.join("transition-observations.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn pre_marker_wait_records_changed_states_and_lua_errors_without_treating_ingame_as_success() {
    let f = Fixture::new();
    let mut backend = f.backend();
    let clock = f.clock.clone();
    backend.on_states = Box::new(move |_| {
        Ok(if clock.now() < 1.0 {
            vec!["FrontEnd".into(), "AdvancedSetup".into()]
        } else {
            vec!["InGame".into(), "LoadScreen".into()]
        })
    });
    let lua = f.logs.join("Lua.log");
    f.clock.on_sleep(move |_| {
        if !fs::read_to_string(&lua)
            .unwrap()
            .contains("host observer exception")
        {
            append(&lua, "FrontEnd: host observer exception\n");
        }
    });
    let mut runner = f.direct_runner(json!({}), backend);
    runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    let error = runner
        .enter_game_after_loadscreen(0, 3.0, Some(3.0), true)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Scenario);
    for expected in [
        "LoadScreen",
        "InGame",
        "host observer exception",
        "outcome undecidable",
    ] {
        assert!(error.message.contains(expected), "{error}");
    }
    let evidence = transition_observations(&f);
    let changed: Vec<_> = evidence
        .iter()
        .filter(|v| v["phase"] == "loading")
        .collect();
    assert_eq!(
        changed.len(),
        3,
        "stable polls must not append duplicate observations: {evidence:?}"
    );
    assert!(changed[0]["observation"]["states"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "FrontEnd"));
    assert_eq!(evidence.last().unwrap()["phase"], "timeout");
    for row in &evidence {
        assert_eq!(row["session_id"], f.output.to_string_lossy().as_ref());
        assert!(row["monotonic_seconds"].as_f64().unwrap() <= 3.0);
        assert_eq!(row["deadline"], 3.0);
    }
    assert!(f.trace().lua.is_empty() && f.trace().presses.is_empty());
    assert_eq!(f.clock.now(), 3.0);
}

#[test]
fn pre_marker_state_query_consumes_only_original_deadline_and_reports_its_last_error() {
    let f = Fixture::new();
    f.clock.set(10.0);
    let clock = f.clock.clone();
    let mut backend = f.backend();
    backend.on_states = Box::new(move |budget| {
        assert!(budget > 0.0 && budget <= 2.0);
        clock.advance(budget);
        Err(Error::tuner("last native state query failed"))
    });
    let mut runner = f.direct_runner(json!({}), backend);
    runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    let error = runner
        .enter_game_after_loadscreen(0, 600.0, Some(12.0), false)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Scenario);
    assert!(
        error.message.contains("last native state query failed"),
        "{error}"
    );
    assert_eq!(f.clock.now(), 12.0);
    assert_eq!(f.trace().state_queries, vec![2.0]);
    assert!(f.trace().lua.is_empty() && f.trace().presses.is_empty());
    assert_eq!(
        transition_observations(&f).last().unwrap()["deadline"],
        12.0
    );
}

#[test]
fn host_unknown_is_sent_once_and_its_error_reaches_loading_timeout_evidence() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Ok(vec!["FrontEnd".into(), "AdvancedSetup".into()]));
    backend.on_lua = Box::new(|_, code, _| {
        assert!(code.contains("Network.HostGame"));
        Err(Error::unknown("Host outcome unknown after command send"))
    });
    let mut runner = f.direct_runner(json!({}), backend);
    runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    let error = runner
        .run_step(&json!({"action":"start_game","timeout":3}))
        .unwrap_err();
    assert!(
        error
            .message
            .contains("Host outcome unknown after command send"),
        "{error}"
    );
    assert!(
        error.message.contains("AdvancedSetup") && error.message.contains("outcome undecidable"),
        "{error}"
    );
    assert_eq!(lua_count(&f, "Network.HostGame"), 1);
    assert!(f.trace().presses.is_empty());
    assert_eq!(f.clock.now(), 3.0);
    let rows = transition_observations(&f);
    assert!(rows.iter().any(|v| v["phase"] == "host_response"
        && v["observation"]["error"] == "Host outcome unknown after command send"));
}

#[test]
fn official_host_lua_marks_before_after_and_exception_with_the_same_session_and_clock() {
    for fail in [false, true] {
        let f = Fixture::new();
        f.clock.set(7.0);
        let output = f.output.to_string_lossy().into_owned();
        let executed = Arc::new(Mutex::new((0_usize, Vec::<String>::new())));
        let observed = executed.clone();
        let path = f.logs.join("Lua.log");
        let mut backend = f.backend();
        backend.on_lua = Box::new(move |_, code, _| {
            if !code.contains("Network.HostGame") {
                return Ok("true".into());
            }
            let lua = mlua::Lua::new();
            lua.load("UI={IsInFrontEnd=function() return true end};GameConfiguration={IsAnyMultiplayer=function() return false end};ServerType={SERVER_TYPE_NONE=0}").exec().unwrap();
            let host = observed.clone();
            let network = lua.create_table().unwrap();
            network
                .set(
                    "HostGame",
                    lua.create_function(move |_, _: i64| {
                        host.lock().unwrap().0 += 1;
                        if fail {
                            Err(mlua::Error::RuntimeError("official Host exception".into()))
                        } else {
                            Ok(())
                        }
                    })
                    .unwrap(),
                )
                .unwrap();
            lua.globals().set("Network", network).unwrap();
            let prints = observed.clone();
            lua.globals()
                .set(
                    "print",
                    lua.create_function(move |_, text: String| {
                        prints.lock().unwrap().1.push(text);
                        Ok(())
                    })
                    .unwrap(),
                )
                .unwrap();
            let response = lua.load(code).eval::<bool>();
            for text in &observed.lock().unwrap().1 {
                append(&path, &format!("{text}\n"));
            }
            if fail {
                Err(Error::lua(response.unwrap_err().to_string()))
            } else {
                response.unwrap();
                Err(Error::unknown("after host response disappeared"))
            }
        });
        let mut runner = f.direct_runner(json!({}), backend);
        runner.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
        let error = runner
            .run_step(&json!({"action":"start_game","timeout":1}))
            .unwrap_err();
        let observed = executed.lock().unwrap();
        assert_eq!(observed.0, 1);
        assert_eq!(
            observed.1.len(),
            2,
            "missing official FrontEnd markers: {:?}",
            observed.1
        );
        assert!(observed.1[0].contains("phase=before"));
        assert!(observed.1[1].contains(if fail {
            "phase=exception"
        } else {
            "phase=after"
        }));
        for marker in &observed.1 {
            assert!(
                marker.contains(&output) && marker.contains("monotonic=7"),
                "{marker}"
            );
        }
        assert_eq!(
            error.kind,
            if fail {
                ErrorKind::LuaExecution
            } else {
                ErrorKind::Scenario
            }
        );
        if fail {
            assert!(error.message.contains("official Host exception"));
        }
        assert_eq!(lua_count(&f, "Network.HostGame"), 1);
        assert!(f.trace().presses.is_empty());
    }
}

#[test]
fn post_marker_timeout_retains_last_game_predicate_and_does_not_reactivate_loadscreen() {
    let f = Fixture::new();
    let mut backend = f.backend();
    backend.on_states = Box::new(|_| Ok(vec!["LoadScreen".into(), "InGame".into()]));
    backend.on_lua = Box::new(|_, code, _| {
        Ok(if code.contains("Game.GetLocalPlayer") {
            "false"
        } else {
            "true"
        }
        .into())
    });
    let mut runner = f.direct_runner(json!({}), backend);
    ready(&f, &mut runner);
    let error = runner
        .enter_game_after_loadscreen(0, 3.0, Some(3.0), false)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Scenario);
    assert!(
        error.message.contains("local player") && error.message.contains("false"),
        "{error}"
    );
    assert_eq!(lua_count(&f, "OnActivateButtonClicked"), 1);
    assert_eq!(lua_count(&f, "Game.GetLocalPlayer"), 2);
    let rows = transition_observations(&f);
    assert!(rows
        .iter()
        .any(|v| v["observation"]["game_ready"] == "false"));
    assert_eq!(f.clock.now(), 3.0);
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
        assert!(!f.trace().state_queries.is_empty());
        assert!(
            f.trace().lua.is_empty(),
            "rotation must still stop before mutating input"
        );
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
    let last = f.records().pop().unwrap();
    assert_eq!(
        (last["action"].as_str(), last["status"].as_str()),
        (Some("run"), Some("FAIL"))
    );
}

#[test]
fn test_failed_cold_start_is_reported_before_tuner_access() {
    let f = Fixture::new();
    let mut backend = f.backend();
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
            error_has(error, ErrorKind::Scenario, &format!("Step 1:"));
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
fn interrupted_autoplay_stops_only_after_an_acknowledged_start() {
    for after_start in [true, false] {
        let f = Fixture::new();
        let mut backend = f.backend();
        backend.on_lua = Box::new(move |_, code, _| {
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
fn test_autoplay_lua_error_cleans_up_only_after_acknowledged_start() {
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
            assert_eq!(
                lua_count(&f, "SetActive(false)"),
                usize::from(fail_at == "status")
            );
        }
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
