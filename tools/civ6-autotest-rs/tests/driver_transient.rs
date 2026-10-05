mod support_driver;

use civ6_autotest_rs::{Error, ErrorKind};
use serde_json::json;
use support_driver::{FakeBackend, Harness};

#[test]
fn mod_selection_and_ui_lua_use_the_requested_step_budget_without_a_five_second_cap() {
    for action in ["enable_mods", "ui_lua"] {
        for timeout in [2.0, 90.0] {
            let fake = FakeBackend::new().lua(|_, _| Ok("true".into()));
            let budgets = fake.timeouts.clone();
            let mut h = Harness::new(json!({}), fake);
            let step = if action == "enable_mods" {
                json!({"action":action,"ids":["7696e937-2044-4f3c-8e13-4e27632e2710"],"timeout":timeout})
            } else {
                json!({"action":action,"state_name":"FrontEnd","code":"return true","timeout":timeout})
            };
            h.runner.run_step(&step).unwrap();
            assert_eq!(*budgets.lock().unwrap(), [timeout]);
            assert_eq!(h.calls().len(), 1);
        }
    }
}

#[test]
fn long_budget_ui_command_with_unknown_response_is_never_sent_twice() {
    let fake = FakeBackend::new().lua(|_, _| Err(Error::unknown("response lost")));
    let mut h = Harness::new(json!({}), fake);
    let error = h
        .runner
        .run_step(
            &json!({"action":"ui_lua","state_name":"FrontEnd","code":"mutate()","timeout":90}),
        )
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::LuaResponseUnknown);
    assert_eq!(h.calls().len(), 1);
}

fn timeout_backend(failures: usize, error: Error, reply: &str) -> FakeBackend {
    let mut remaining = failures;
    let reply = reply.to_owned();
    FakeBackend::new()
        .states(|| Ok(vec!["InGame".into(), "AdvisorPopup".into()]))
        .lua(move |_, _| {
            if remaining > 0 {
                remaining -= 1;
                Err(error.clone())
            } else {
                Ok(reply.clone())
            }
        })
}

#[test]
fn test_status_query_retries_after_timeout() {
    let fake = timeout_backend(
        1,
        Error::unknown("timed out after 0/8 bytes"),
        "42|true|false|10|0",
    );
    let mut h = Harness::new(json!({}), fake);
    assert_eq!(h.runner.autoplay_status(60.0).unwrap().turn, 42);
    assert_eq!(
        h.calls()
            .iter()
            .map(|(s, _)| s.as_str())
            .collect::<Vec<_>>(),
        ["InGame", "InGame"]
    );
    assert!(h.events().iter().any(|s| s == "disconnect"));
}

#[test]
fn test_advisor_dismissal_retries_after_timeout() {
    let mut h = Harness::new(
        json!({}),
        timeout_backend(1, Error::unknown("timed out"), "true"),
    );
    assert!(h.runner.dismiss_advisor(60.0).unwrap());
    assert_eq!(
        h.calls()
            .iter()
            .map(|(s, _)| s.as_str())
            .collect::<Vec<_>>(),
        ["AdvisorPopup", "AdvisorPopup"]
    );
}

#[test]
fn test_research_query_retries_after_timeout() {
    let mut h = Harness::new(
        json!({}),
        timeout_backend(1, Error::unknown("timed out"), "3|0~A~true~0~0~~~~"),
    );
    assert_eq!(
        h.runner.research_query("return 1", 60.0, None).unwrap(),
        "3|0~A~true~0~0~~~~"
    );
    assert_eq!(
        h.calls()
            .iter()
            .map(|(s, _)| s.as_str())
            .collect::<Vec<_>>(),
        ["InGame", "InGame"]
    );
}

#[test]
fn test_gives_up_only_after_deadline() {
    let mut h = Harness::new(
        json!({}),
        timeout_backend(1_000_000, Error::unknown("timed out"), "42|true|false|10|0"),
    );
    assert_eq!(
        h.runner.autoplay_status(3.0).unwrap_err().kind,
        ErrorKind::LuaResponseUnknown
    );
    assert!(h.calls().len() > 1);
    assert!(h.clock.at() <= 3.0);
}

#[test]
fn test_stale_research_snapshot_stops_at_deadline() {
    let mut h = Harness::new(
        json!({}),
        timeout_backend(0, Error::unknown("unused"), "1|0~LEADER_A~true~0~0~~~~"),
    );
    let err = h
        .runner
        .research_query("return snapshot", 1.0, Some(63))
        .unwrap_err();
    assert!(err.to_string().contains("deadline expired"));
    assert_eq!(h.clock.at(), 1.0);
    assert!(h.calls().len() > 1);
}

#[test]
fn test_lua_error_is_not_retried() {
    let mut h = Harness::new(
        json!({}),
        timeout_backend(1, Error::lua("boom"), "42|true|false|10|0"),
    );
    assert_eq!(
        h.runner.autoplay_status(60.0).unwrap_err().kind,
        ErrorKind::LuaExecution
    );
    assert_eq!(h.calls().len(), 1);
}

#[test]
fn test_state_changing_lua_is_not_replayed() {
    let mut h = Harness::new(
        json!({}),
        timeout_backend(1, Error::unknown("ack lost"), "true"),
    );
    assert_eq!(
        h.runner
            .lua("InGame", "AutoplayManager.SetActive(true)", 60.0)
            .unwrap_err()
            .kind,
        ErrorKind::LuaResponseUnknown
    );
    assert_eq!(h.calls().len(), 1);
}
