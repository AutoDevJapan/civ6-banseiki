mod support_driver;
use civ6_autotest_rs::{
    native_observer_ready::{self as native, Identity, Manifest, Session, ACK_LUA, CORE_SHA256},
    scenario::validate_scenario,
    Error, Result,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use support_driver::{FakeBackend, Harness};

const HANDOFF: &str = "D:/CC/Work/todo995/native-1743-live05-handoff.json";
fn step() -> Value {
    json!({"action":"native_observer_ready","handoff":HANDOFF,"timeout":240})
}
fn manifest() -> Value {
    json!({"version":1,"run_tag":"native-1743-live05","chain_pid":123,"chain_created":134360000000000000_u64,"lock_token":"test-lease","game_pid":456,"game_created":134360000000000001_u64,"observer_pid":789,"observer_created":134360000000000002_u64,"ready_path":"D:/CC/Work/todo995/native-1743-live05-observer.jsonl.ready","core_sha256":CORE_SHA256})
}
#[derive(Clone)]
struct Evidence {
    lease: Value,
    ready: Value,
    processes: [Identity; 3],
}
impl Evidence {
    fn valid() -> Self {
        Self {
            lease: json!({"live_chain":true,"pid":123,"chain_pid":123,"pid_created_filetime":134360000000000000_u64,"lock_token":"test-lease"}),
            ready: json!({"event":"ready","pid":456,"created":134360000000000001_u64,"module_file_sha256":CORE_SHA256,"threads_installed":110,"ready_path":"D:/CC/Work/todo995/native-1743-live05-observer.jsonl.ready","semantics":"extra observer metadata is permitted"}),
            processes: [
                Identity {
                    pid: 123,
                    created: 134360000000000000,
                    alive: true,
                },
                Identity {
                    pid: 456,
                    created: 134360000000000001,
                    alive: true,
                },
                Identity {
                    pid: 789,
                    created: 134360000000000002,
                    alive: true,
                },
            ],
        }
    }
}
struct MockSession {
    evidence: Arc<Mutex<Evidence>>,
    events: Arc<Mutex<Vec<String>>>,
}
impl Session for MockSession {
    fn check(&mut self, manifest: &Manifest, game: u32) -> Result<()> {
        self.events.lock().unwrap().push("identity_check".into());
        let e = self.evidence.lock().unwrap();
        native::validate_evidence(
            manifest,
            &serde_json::to_vec(&e.lease).unwrap(),
            &serde_json::to_vec(&e.ready).unwrap(),
            e.processes,
            game,
        )
    }
}
fn fake_with(manifest: Vec<u8>, evidence: Arc<Mutex<Evidence>>) -> FakeBackend {
    let mut fake = FakeBackend::new().process(|_| Ok(Some((456, 100.0))));
    fake.on_native_manifest = Box::new(move |path| {
        assert_eq!(path.to_str().unwrap(), HANDOFF);
        Ok(Some(manifest.clone()))
    });
    let events = fake.events.clone();
    fake.on_native_session = Box::new(move |_| {
        Ok(Box::new(MockSession {
            evidence: evidence.clone(),
            events: events.clone(),
        }))
    });
    fake.on_lua = Box::new(|state, code| {
        assert_eq!(state, "InGame");
        assert_eq!(code, ACK_LUA);
        Ok("native observer ready acknowledged".into())
    });
    fake
}
fn valid_fake() -> (FakeBackend, Arc<Mutex<Evidence>>) {
    let evidence = Arc::new(Mutex::new(Evidence::valid()));
    (
        fake_with(serde_json::to_vec(&manifest()).unwrap(), evidence.clone()),
        evidence,
    )
}
fn harness(fake: FakeBackend) -> Harness {
    Harness::new(json!({"name":"native ready","steps":[step()]}), fake)
}
fn no_ack(h: &Harness) {
    assert_eq!(h.calls().len(), 0);
    assert!(!h
        .events()
        .iter()
        .any(|e| e == "connect" || e == "disconnect"));
}

#[test]
fn native_ready_acknowledges_once_on_existing_connection() {
    let (fake, _) = valid_fake();
    let mut h = harness(fake);
    assert_eq!(
        h.runner.run_step(&step()).unwrap(),
        "native observer ready acknowledged"
    );
    assert_eq!(h.calls(), vec![("InGame".into(), ACK_LUA.into())]);
    assert_eq!(
        h.events(),
        [
            "manifest",
            "pin",
            "identity_check",
            "existing_connection",
            "states",
            "identity_check",
            "lua:InGame"
        ]
    );
    assert!(h
        .runner
        .run_step(&step())
        .unwrap_err()
        .message
        .contains("already started"));
    assert_eq!(h.calls().len(), 1);
}
#[test]
fn native_ready_unknown_response_and_tuner_error_never_retry() {
    for error in [
        Error::unknown("ack result unknown"),
        Error::tuner("ack write unavailable"),
        Error::lua("ARM assertion failed"),
    ] {
        let (mut fake, _) = valid_fake();
        let expected = error.clone();
        fake.on_lua = Box::new(move |_, _| Err(error.clone()));
        let mut h = harness(fake);
        assert_eq!(h.runner.run_step(&step()).unwrap_err(), expected);
        assert!(h.runner.run_step(&step()).is_err());
        assert_eq!(h.calls().len(), 1);
        assert_eq!(h.events().iter().filter(|e| *e == "states").count(), 1);
        assert!(!h
            .events()
            .iter()
            .any(|e| e == "connect" || e == "disconnect"));
    }
}
#[test]
fn native_ready_only_missing_manifest_is_polled_for_at_most_240_seconds() {
    let (mut fake, _) = valid_fake();
    fake.on_native_manifest = Box::new(|_| Ok(None));
    let mut h = harness(fake);
    assert!(h.runner.run_step(&step()).is_err());
    assert!((h.clock.at() - 240.0).abs() < 0.000001);
    assert!(h.events().len() <= 1201);
    assert!(h.events().len() > 1);
    no_ack(&h);
    assert!(h.runner.run_step(&step()).is_err());
    no_ack(&h);
}
#[test]
fn native_ready_missing_then_atomic_publish_succeeds() {
    let (mut fake, _) = valid_fake();
    let mut polls = 0;
    fake.on_native_manifest = Box::new(move |_| {
        polls += 1;
        Ok(if polls < 3 {
            None
        } else {
            Some(serde_json::to_vec(&manifest()).unwrap())
        })
    });
    let mut h = harness(fake);
    h.runner.run_step(&step()).unwrap();
    assert_eq!(h.calls().len(), 1);
    assert_eq!(h.clock.at(), 0.4);
}
#[test]
fn native_ready_invalid_manifest_fails_immediately_without_ack() {
    let mut cases = vec![
        b"{".to_vec(),
        b"null".to_vec(),
        b"[]".to_vec(),
        vec![b' '; 65537],
    ];
    for (key, value) in [
        ("version", json!(2)),
        ("version", json!("1")),
        ("run_tag", json!("another-run")),
        ("run_tag", json!("../escape")),
        (
            "ready_path",
            json!("D:/CC/Work/todo995/native-old-observer.jsonl.ready"),
        ),
        ("game_pid", json!("456")),
        ("game_created", json!(134360000000000001.0)),
        ("observer_pid", json!(0)),
        ("observer_pid", json!(456)),
        ("core_sha256", json!("wrong")),
        ("lock_token", json!("")),
        ("unknown", json!(true)),
    ] {
        let mut m = manifest();
        m[key] = value;
        cases.push(serde_json::to_vec(&m).unwrap());
    }
    let mut missing = manifest();
    missing.as_object_mut().unwrap().remove("observer_created");
    cases.push(serde_json::to_vec(&missing).unwrap());
    for bytes in cases {
        let mut h = harness(fake_with(bytes, Arc::new(Mutex::new(Evidence::valid()))));
        assert!(h.runner.run_step(&step()).is_err());
        assert_eq!(h.clock.at(), 0.0);
        assert_eq!(h.events(), ["manifest"]);
        no_ack(&h);
    }
}
#[test]
fn native_ready_rejects_stale_or_invalid_ready_and_changed_lease() {
    for (ready, key, value) in [
        (true, "event", json!("exit")),
        (true, "pid", json!(457)),
        (true, "created", json!(134359000000000001_u64)),
        (true, "created", json!("134360000000000001")),
        (true, "module_file_sha256", json!("wrong")),
        (true, "threads_installed", json!(0)),
        (true, "threads_installed", json!(-1)),
        (true, "threads_installed", json!("110")),
        (
            true,
            "ready_path",
            json!("D:/CC/Work/todo995/old-observer.jsonl.ready"),
        ),
        (false, "live_chain", json!(false)),
        (false, "chain_pid", json!(124)),
        (false, "pid", json!(124)),
        (false, "pid_created_filetime", json!(134360000000000010_u64)),
        (false, "lock_token", json!("changed")),
        (false, "pid", json!("123")),
    ] {
        let (fake, evidence) = valid_fake();
        if ready {
            evidence.lock().unwrap().ready[key] = value;
        } else {
            evidence.lock().unwrap().lease[key] = value;
        }
        let mut h = harness(fake);
        assert!(h.runner.run_step(&step()).is_err(), "{key}");
        assert!(!h.events().iter().any(|e| e == "states"));
        no_ack(&h);
    }
}
#[test]
fn native_ready_rejects_each_ended_or_reused_process() {
    for index in 0..3 {
        for reuse in [false, true] {
            let (fake, evidence) = valid_fake();
            if reuse {
                evidence.lock().unwrap().processes[index].created += 1;
            } else {
                evidence.lock().unwrap().processes[index].alive = false;
            }
            let mut h = harness(fake);
            assert!(h.runner.run_step(&step()).is_err());
            no_ack(&h);
        }
    }
}
#[test]
fn native_ready_rechecks_after_states_before_ack() {
    for change in 0..5 {
        let (mut fake, evidence) = valid_fake();
        fake.on_states = Box::new(move || {
            let mut e = evidence.lock().unwrap();
            match change {
                0 => e.processes[2].alive = false,
                1 => e.lease["lock_token"] = json!("new"),
                2 => e.processes[1].created += 1,
                3 => e.processes[0].alive = false,
                _ => e.ready["threads_installed"] = json!(0),
            }
            Ok(vec!["InGame".into()])
        });
        let mut h = harness(fake);
        assert!(h.runner.run_step(&step()).is_err());
        assert_eq!(
            h.events().iter().filter(|e| *e == "identity_check").count(),
            2
        );
        no_ack(&h);
    }
}
#[test]
fn native_ready_rejects_changed_single_game_before_ack() {
    for change_after_states in [false, true] {
        let (mut fake, _) = valid_fake();
        let mut queries = 0;
        fake.on_process = Box::new(move |_| {
            queries += 1;
            Ok(Some((
                if !change_after_states || queries > 1 {
                    457
                } else {
                    456
                },
                100.0,
            )))
        });
        let mut h = harness(fake);
        assert!(h.runner.run_step(&step()).is_err());
        no_ack(&h);
    }
}
#[test]
fn native_ready_no_connection_and_states_failures_do_not_reconnect() {
    for case in 0..4 {
        let (mut fake, _) = valid_fake();
        match case {
            0 => {
                fake.on_native_connection = Box::new(|| Err(Error::tuner("no existing connection")))
            }
            1 => fake.on_states = Box::new(|| Err(Error::tuner("states timeout"))),
            2 => fake.on_states = Box::new(|| Ok(vec!["FrontEnd".into()])),
            _ => fake.on_states = Box::new(|| Ok(vec!["InGame".into(), "INGAME".into()])),
        }
        let mut h = harness(fake);
        assert!(h.runner.run_step(&step()).is_err());
        no_ack(&h);
    }
}
#[test]
fn native_ready_unexpected_reply_is_failure_without_resend() {
    let (mut fake, _) = valid_fake();
    fake.on_lua = Box::new(|_, _| Ok("false".into()));
    let mut h = harness(fake);
    assert_eq!(
        h.runner.run_step(&step()).unwrap_err().kind,
        civ6_autotest_rs::ErrorKind::LuaExecution
    );
    assert!(h.runner.run_step(&step()).is_err());
    assert_eq!(h.calls().len(), 1);
}
#[test]
fn native_ready_missing_or_ambiguous_game_is_failure() {
    for ambiguous in [false, true] {
        let (mut fake, _) = valid_fake();
        fake.on_process = Box::new(move |_| {
            if ambiguous {
                Err(Error::input("two games"))
            } else {
                Ok(None)
            }
        });
        let mut h = harness(fake);
        assert!(h.runner.run_step(&step()).is_err());
        no_ack(&h);
    }
}
#[test]
fn native_ready_first_step_does_not_launch_or_restart_or_query_frontend() {
    let (fake, _) = valid_fake();
    let mut h = harness(fake);
    h.runner.output = h.root.path().join("full-run-output");
    h.runner.run().unwrap();
    assert_eq!(h.calls(), vec![("InGame".into(), ACK_LUA.into())]);
    assert_eq!(h.runner.completed_steps, 1);
    assert!(!h
        .events()
        .iter()
        .any(|e| e == "connect" || e == "disconnect" || e == "launch" || e == "close"));
}
#[test]
fn native_ready_deadline_expiring_during_states_prevents_ack() {
    let clock = Arc::new(support_driver::ManualClock::default());
    let clock_in_states = clock.clone();
    let (mut fake, _) = valid_fake();
    fake.on_states = Box::new(move || {
        clock_in_states.advance(240.0);
        Ok(vec!["InGame".into()])
    });
    let mut h = harness(fake);
    h.runner.clock = clock;
    assert!(h.runner.run_step(&step()).is_err());
    assert!(h.events().iter().any(|e| e == "states"));
    no_ack(&h);
    assert!(h.runner.run_step(&step()).is_err());
}
#[test]
fn native_ready_scenario_and_lua51_preflight() {
    validate_scenario(&json!({"name":"native ready","steps":[step()]})).unwrap();
    for (key, value) in [
        ("timeout", json!(241)),
        ("timeout", json!(0)),
        ("timeout", json!("240")),
        ("handoff", json!("relative-handoff.json")),
        (
            "handoff",
            json!("D:/CC/Work/todo995/../escape-handoff.json"),
        ),
        ("code", json!("return true")),
    ] {
        let mut s = step();
        s[key] = value;
        assert!(validate_scenario(&json!({"name":"native ready","steps":[s]})).is_err());
    }
    let lua = mlua::Lua::new();
    lua.load(ACK_LUA).into_function().unwrap();
    let mut repeat = json!({"name":"repeat","steps":[{"action":"repeat_until","max_iterations":1,"condition":{"type":"lua","state_name":"InGame","code":"return true"},"steps":[{"action":"end_turn","turns":1},{"action":"wait_for","condition":{"type":"lua","state_name":"InGame","code":"return true"}},{"action":"ui_lua","state_name":"InGame","code":"return true"}]}]});
    validate_scenario(&repeat).unwrap();
    repeat["steps"][0]["steps"][2] = step();
    assert!(validate_scenario(&repeat)
        .unwrap_err()
        .message
        .contains("only accepts end_turn, wait_for and ui_lua"));
}
#[test]
fn native_ready_malformed_evidence_is_not_success() {
    let m = native::parse_manifest(
        std::path::Path::new(HANDOFF),
        &serde_json::to_vec(&manifest()).unwrap(),
    )
    .unwrap();
    let e = Evidence::valid();
    assert!(native::validate_evidence(
        &m,
        b"{",
        &serde_json::to_vec(&e.ready).unwrap(),
        e.processes,
        456
    )
    .is_err());
    assert!(native::validate_evidence(
        &m,
        &serde_json::to_vec(&e.lease).unwrap(),
        b"{",
        e.processes,
        456
    )
    .is_err());
}
#[cfg(not(windows))]
#[test]
fn native_ready_system_is_unsupported_outside_windows() {
    use civ6_autotest_rs::runner::{Backend, SystemBackend};
    let mut backend = SystemBackend::default();
    assert_eq!(
        backend
            .native_observer_manifest(std::path::Path::new(HANDOFF))
            .unwrap_err()
            .kind,
        civ6_autotest_rs::ErrorKind::Input
    );
}
