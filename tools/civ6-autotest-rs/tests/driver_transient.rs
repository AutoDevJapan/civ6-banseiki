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
fn long_budget_mutations_with_unknown_response_are_never_sent_twice() {
    for step in [
        json!({"action":"ui_lua","state_name":"FrontEnd","code":"mutate()","timeout":90}),
        json!({"action":"enable_mods","ids":["7696e937-2044-4f3c-8e13-4e27632e2710"],"timeout":90}),
    ] {
        let fake = FakeBackend::new().lua(|_, _| Err(Error::unknown("response lost")));
        let mut h = Harness::new(json!({}), fake);
        let error = h.runner.run_step(&step).unwrap_err();
        assert_eq!(error.kind, ErrorKind::LuaResponseUnknown);
        assert_eq!(h.calls().len(), 1);
    }
}

#[test]
fn pause_releases_connection_and_next_lua_reconnects() {
    let mut h = Harness::new(json!({}), FakeBackend::new());
    h.runner.lua("InGame", "return true", 60.0).unwrap();
    h.events.lock().unwrap().clear();
    h.runner
        .run_step(&json!({"action":"pause","seconds":1}))
        .unwrap();
    assert_eq!(h.events(), ["disconnect"]);
    assert_eq!(h.clock.at(), 1.0);
    h.runner.lua("InGame", "return true", 60.0).unwrap();
    assert_eq!(
        h.events(),
        ["disconnect", "connect", "states", "connect", "lua:InGame"]
    );
}

#[test]
fn pause_fails_when_sleep_overshoots_step_deadline() {
    let mut h = Harness::new(json!({}), FakeBackend::new());
    h.runner.lua("InGame", "return true", 60.0).unwrap();
    h.events.lock().unwrap().clear();
    h.clock.set_min_sleep(3.0);
    let err = h
        .runner
        .run_step(&json!({"action":"pause","seconds":1,"timeout":2}))
        .unwrap_err();
    assert!(err.to_string().contains("deadline expired"));
    assert_eq!(h.events(), ["disconnect"]);
    assert_eq!(h.clock.at(), 3.0);
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
fn human_alive_read_retries_a_lost_reply_but_not_a_lua_error() {
    let mut h = Harness::new(
        json!({}),
        timeout_backend(1, Error::unknown("alive reply lost"), "1|true"),
    );
    assert_eq!(h.runner.human_alive(0, 60.0).unwrap(), (1, true));
    assert_eq!(h.calls().len(), 2);
    assert!(h.calls().iter().all(|(state, code)| {
        state == "InGame"
            && code
                == "return tostring(Game.GetCurrentGameTurn()) .. '|' .. tostring(Players[0]:IsAlive())"
    }));

    let mut h = Harness::new(
        json!({}),
        timeout_backend(1, Error::lua("alive script failed"), "1|true"),
    );
    assert_eq!(
        h.runner.human_alive(0, 60.0).unwrap_err().kind,
        ErrorKind::LuaExecution
    );
    assert_eq!(h.calls().len(), 1);
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

#[test]
fn options_state_lookup_recovers_after_both_socket_attempts_fail() {
    let mut attempts = 0;
    let fake = FakeBackend::new().states(move || {
        attempts += 1;
        if attempts <= 2 {
            Err(Error::tuner("No reply to LSQ from FireTuner"))
        } else {
            Ok(vec!["InGame".into()])
        }
    });
    let mut h = Harness::new(json!({}), fake);
    assert!(!h.runner.close_options_menu(60.0).unwrap());
    assert_eq!(h.events().iter().filter(|e| *e == "states").count(), 3);
    assert_eq!(h.clock.at(), 0.5);
    assert!(h.calls().is_empty());
}

#[test]
fn recovered_options_state_is_closed_once_and_verified_hidden() {
    let mut attempts = 0;
    let fake = FakeBackend::new()
        .states(move || {
            attempts += 1;
            if attempts <= 2 {
                Err(Error::tuner("No reply to LSQ from FireTuner"))
            } else {
                Ok(vec!["InGameTopOptionsMenu".into()])
            }
        })
        .lua(|_, _| Ok("true".into()));
    let mut h = Harness::new(json!({}), fake);
    assert!(h.runner.close_options_menu(60.0).unwrap());
    let calls = h.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "InGameTopOptionsMenu");
    assert!(calls[0].1.contains("OnReturn(); CloseImmediately()"));
    assert_eq!(calls[1].0, "InGameTopOptionsMenu");
    assert_eq!(calls[1].1, "return ContextPtr:IsHidden()");
    assert_eq!(h.clock.at(), 0.5);
}

#[test]
fn options_state_lookup_respects_shorter_step_deadline() {
    let fake = FakeBackend::new().states(|| Err(Error::tuner("no LSQ reply")));
    let mut h = Harness::new(json!({}), fake);
    assert_eq!(
        h.runner.close_options_menu(3.0).unwrap_err().kind,
        ErrorKind::Tuner
    );
    assert_eq!(h.clock.at(), 1.5);
    assert!(h.calls().is_empty());
}

#[test]
fn options_state_lookup_is_bounded_even_in_a_three_hour_step() {
    let fake = FakeBackend::new().states(|| Err(Error::tuner("no LSQ reply")));
    let mut h = Harness::new(json!({}), fake);
    assert_eq!(
        h.runner.close_options_menu(10800.0).unwrap_err().kind,
        ErrorKind::Tuner
    );
    assert_eq!(h.clock.at(), 27.5);
    assert!(h.calls().is_empty());
}

#[test]
fn options_close_with_lost_acknowledgement_is_not_replayed() {
    let fake = FakeBackend::new()
        .states(|| Ok(vec!["InGameTopOptionsMenu".into()]))
        .lua(|_, _| Err(Error::unknown("OnReturn acknowledgement lost")));
    let mut h = Harness::new(json!({}), fake);
    assert_eq!(
        h.runner.close_options_menu(60.0).unwrap_err().kind,
        ErrorKind::LuaResponseUnknown
    );
    assert_eq!(h.calls().len(), 1);
    assert!(h.calls()[0].1.contains("OnReturn(); CloseImmediately()"));
    assert_eq!(h.clock.at(), 0.0);
}

fn close_options_over_socket(fail_query: usize, lose_ack: bool) {
    use civ6_autotest_rs::{
        runner::{Backend, Runner},
        time::{Clock, SystemClock},
        tuner::{recv_frame, send_frame, TunerClient, COMMAND, HANDSHAKE},
        Result,
    };
    use std::{
        io,
        net::TcpListener,
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };

    struct SocketBackend(Option<TunerClient>);
    impl Backend for SocketBackend {
        fn require_single_process(&mut self) -> Result<()> {
            Ok(())
        }
        fn connect(&mut self, port: u16, timeout: f64) -> Result<()> {
            if self.0.is_none() {
                self.0 = Some(TunerClient::connect(port, timeout)?);
            }
            Ok(())
        }
        fn disconnect(&mut self) {
            if let Some(mut client) = self.0.take() {
                client.close();
            }
        }
        fn list_states(&mut self, timeout: f64) -> Result<Vec<String>> {
            self.0.as_mut().unwrap().list_states(timeout)
        }
        fn run_lua(&mut self, state: &str, code: &str, timeout: f64) -> Result<String> {
            self.0
                .as_mut()
                .unwrap()
                .run_lua(state, code, timeout)
                .map(|r| r.value)
        }
    }

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let mut queries = 0;
        let mut commands = Vec::new();
        let limit = Instant::now() + Duration::from_secs(12);
        for connection_index in 0..2 {
            let (mut connection, _) = loop {
                match listener.accept() {
                    Ok(pair) => break pair,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < limit, "missing retry connection");
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("accept failed: {e}"),
                }
            };
            connection.set_nonblocking(false).unwrap();
            while let Ok(frame) = recv_frame(
                &mut connection,
                Some(civ6_autotest_rs::time::monotonic() + 10.0),
            ) {
                match frame.payload.as_str() {
                    "APP:" => send_frame(&mut connection, HANDSHAKE, "Civilization VI").unwrap(),
                    "LSQ:" => {
                        queries += 1;
                        if queries == fail_query {
                            break; // Disconnect before any command bytes are received.
                        }
                        send_frame(&mut connection, HANDSHAKE, "0\nInGameTopOptionsMenu").unwrap();
                    }
                    _ => {
                        assert_eq!(frame.tag, COMMAND);
                        let start = frame.payload.find("JAC_AUTOTEST_").unwrap();
                        let token = &frame.payload[start..start + "JAC_AUTOTEST_".len() + 32];
                        let ack = format!("{token}|OK|true");
                        commands.push(frame.payload);
                        if lose_ack {
                            break;
                        }
                        send_frame(&mut connection, COMMAND, &ack).unwrap();
                    }
                }
            }
            if connection_index == 1 {
                break;
            }
        }
        commands
    });
    let root = tempfile::tempdir().unwrap();
    let clock = Arc::new(SystemClock);
    let mut runner = Runner::with_backend(
        json!({}),
        root.path().into(),
        root.path().into(),
        root.path().into(),
        port,
        Box::new(SocketBackend(None)),
        clock.clone(),
    );
    let result = runner.close_options_menu(clock.now() + 10.0);
    runner.disconnect();
    let commands = server.join().unwrap();
    assert_eq!(
        commands.iter().filter(|c| c.contains("OnReturn()")).count(),
        1
    );
    if lose_ack {
        assert_eq!(result.unwrap_err().kind, ErrorKind::LuaResponseUnknown);
        assert_eq!(commands.len(), 1);
    } else {
        assert!(result.unwrap());
        assert_eq!(commands.len(), 2);
        assert!(commands[1].contains("return ContextPtr:IsHidden()"));
    }
}

#[test]
fn options_close_recovers_when_runner_pre_send_state_query_disconnects() {
    close_options_over_socket(3, false);
}

#[test]
fn options_close_recovers_when_tuner_state_index_query_disconnects() {
    close_options_over_socket(4, false);
}

#[test]
fn options_close_after_state_index_recovery_never_replays_lost_ack() {
    close_options_over_socket(4, true);
}
