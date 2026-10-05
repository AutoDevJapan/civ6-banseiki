mod support_driver;

use civ6_autotest_rs::{runner::AutoplayOptions, ErrorKind};
use serde_json::json;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use support_driver::{FakeBackend, Harness};

fn run_case(statuses: &[&str], pending: &[&str]) -> (Harness, civ6_autotest_rs::Result<String>) {
    let statuses = Arc::new(Mutex::new(VecDeque::from(
        statuses.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )));
    let last = statuses.lock().unwrap().back().cloned().unwrap();
    let pending = Arc::new(Mutex::new(VecDeque::from(
        pending.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )));
    let fake = FakeBackend::new().lua(move |_, code| {
        if code.contains("return tostring(startTurn)") {
            return Ok("30|14".into());
        }
        if code.contains("tostring(AutoplayManager.IsActive())") {
            return Ok(statuses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| last.clone()));
        }
        if code.contains("GetPlayerCommemorateChoices") {
            return Ok(pending
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| "none".into()));
        }
        if code.contains("IsAlive()") {
            return Ok("true".into());
        }
        Ok("true".into())
    });
    let mut h = Harness::new(json!({}), fake);
    h.clock.set_min_sleep(3.0);
    let result = h.runner.autoplay(10, 1000.0, AutoplayOptions::default());
    (h, result)
}
fn count(h: &Harness, code: &str) -> usize {
    h.count_code(code)
}
fn keys(h: &Harness) -> usize {
    h.events().iter().filter(|s| s.starts_with("key:")).count()
}

#[test]
fn test_stall_with_pending_dedication_chooses_closes_and_resumes() {
    let (h, result) = run_case(
        &[
            "31|true|false|0|14",
            "31|false|false|10|14",
            "31|false|false|10|14",
            "31|false|false|10|14",
            "31|false|false|10|14",
            "40|false|true|0|14",
        ],
        &["7,9"],
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(keys(&h), 0);
    assert_eq!(count(&h, "GetPlayerCommemorateChoices"), 1);
    assert_eq!(count(&h, "SetActive(true)"), 2);
}

#[test]
fn test_repeated_dedication_is_bounded_then_fails() {
    let (h, result) = run_case(&["31|false|false|10|14"], &["1"; 50]);
    assert_eq!(result.unwrap_err().kind, ErrorKind::Scenario);
    assert_eq!(keys(&h), 0);
    assert_eq!(count(&h, "GetPlayerCommemorateChoices"), 3);
    assert_eq!(count(&h, "SetActive(true)"), 1 + 3 + 2);
}

#[test]
fn test_no_pending_dedication_keeps_old_restart_behaviour() {
    let (h, result) = run_case(&["31|false|false|10|14"], &[]);
    assert_eq!(result.unwrap_err().kind, ErrorKind::Scenario);
    assert_eq!(keys(&h), 0);
}

#[test]
fn dedication_closes_only_a_visible_popup_and_never_sends_escape() {
    for hidden in [true, false] {
        let closes = Arc::new(Mutex::new(0));
        let observed = closes.clone();
        let fake = FakeBackend::new()
            .states(|| Ok(vec!["InGame".into(), "DedicationPopup".into()]))
            .lua(move |state, code| {
                if state == "DedicationPopup" {
                    let lua = mlua::Lua::new();
                    lua.load(format!("closed = 0; ContextPtr = {{IsHidden = function() return {hidden} end}}; OnClose = function() closed = closed + 1 end")).exec().unwrap();
                    let result: bool = lua.load(code).eval().unwrap();
                    *observed.lock().unwrap() += lua.globals().get::<i32>("closed").unwrap();
                    return Ok(result.to_string());
                }
                Ok("7".into())
            });
        let mut h = Harness::new(json!({}), fake);
        assert_eq!(
            h.runner.choose_dedication(14, 100.0).unwrap(),
            Some("7".into())
        );
        assert_eq!(keys(&h), 0);
        assert_eq!(*closes.lock().unwrap(), i32::from(!hidden));
    }
}

#[test]
fn dedication_step_calls_the_existing_sender_and_visible_closer() {
    let fake = FakeBackend::new()
        .states(|| Ok(vec!["InGame".into(), "DedicationPopup".into()]))
        .lua(|state, code| {
            Ok(if code == "return Game.GetLocalPlayer()" {
                "0"
            } else if state == "DedicationPopup" {
                "true"
            } else {
                "7"
            }
            .into())
        });
    let mut h = Harness::new(json!({}), fake);
    assert_eq!(
        h.runner
            .run_step(&json!({"action":"choose_dedication","human":0}))
            .unwrap(),
        "human=0 chose commemoration indices=7"
    );
    assert_eq!(count(&h, "UI.RequestPlayerOperation"), 1);
    assert_eq!(count(&h, "OnClose()"), 1);
    let events = h.events();
    let send = events
        .iter()
        .position(|v| v.contains("UI.RequestPlayerOperation"))
        .unwrap();
    let close = events.iter().position(|v| v.contains("OnClose()")).unwrap();
    assert!(
        send < close,
        "dedication must submit before closing its visible screen"
    );
    assert_eq!(keys(&h), 0);
}

#[test]
fn dedication_step_rejects_another_local_player_without_submitting() {
    let fake = FakeBackend::new().lua(|_, _| Ok("1".into()));
    let mut h = Harness::new(json!({}), fake);
    let error = h
        .runner
        .run_step(&json!({"action":"choose_dedication","human":0}))
        .unwrap_err();
    assert!(error.to_string().contains("not the local player"));
    assert_eq!(count(&h, "UI.RequestPlayerOperation"), 0);
}

#[test]
fn dedication_step_fails_when_no_choice_is_pending() {
    let fake = FakeBackend::new().lua(|_, code| {
        Ok(if code == "return Game.GetLocalPlayer()" {
            "0"
        } else {
            "none"
        }
        .into())
    });
    let mut h = Harness::new(json!({}), fake);
    let error = h
        .runner
        .run_step(&json!({"action":"choose_dedication","human":0}))
        .unwrap_err();
    assert!(error.to_string().contains("No pending dedication"));
    assert_eq!(keys(&h), 0);
}

#[test]
fn dedication_step_requires_a_valid_player_number() {
    for human in [json!(0), json!(63)] {
        civ6_autotest_rs::scenario::validate_scenario(
            &json!({"name":"probe","steps":[{"action":"choose_dedication","human":human}]}),
        )
        .unwrap();
    }
    for human in [json!(-1), json!(64), json!(0.5), json!("0"), json!(null)] {
        assert!(civ6_autotest_rs::scenario::validate_scenario(
            &json!({"name":"probe","steps":[{"action":"choose_dedication","human":human}]})
        )
        .is_err());
    }
}
