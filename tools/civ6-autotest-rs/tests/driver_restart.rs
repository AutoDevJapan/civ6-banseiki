mod support_driver;

use civ6_autotest_rs::{
    restart::{self, LifecycleBackend},
    scenario, Error, ErrorKind, Result,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};
use support_driver::{FakeBackend, Harness, ManualClock};

fn processes(
    items: &[Option<(u32, f64)>],
    tail: Option<(u32, f64)>,
) -> impl FnMut(f64) -> Result<Option<(u32, f64)>> + Send {
    let mut queue = VecDeque::from(items.to_vec());
    move |_| Ok(queue.pop_front().unwrap_or(tail))
}
fn prepare_run(h: &mut Harness) {
    std::fs::remove_dir(&h.runner.output).unwrap();
    for name in ["Lua.log", "Database.log", "Modding.log"] {
        std::fs::write(h.runner.logs.join(name), "").unwrap();
    }
}
fn pause_scenario() -> Value {
    json!({"name":"restart fixture","steps":[{"action":"pause","seconds":0.1}]})
}
fn position(events: &[String], prefix: &str) -> usize {
    events.iter().position(|s| s.starts_with(prefix)).unwrap()
}

struct NativeFixture {
    ids: VecDeque<BTreeSet<u32>>,
    tail: BTreeSet<u32>,
    now: f64,
    windows: usize,
}
impl NativeFixture {
    fn new(rows: &[&[u32]]) -> Self {
        let ids: VecDeque<_> = rows.iter().map(|r| r.iter().copied().collect()).collect();
        let tail = ids.back().cloned().unwrap_or_default();
        Self {
            ids,
            tail,
            now: 0.0,
            windows: 0,
        }
    }
}
impl LifecycleBackend for NativeFixture {
    fn civ_ids(&mut self) -> Result<BTreeSet<u32>> {
        Ok(self.ids.pop_front().unwrap_or_else(|| self.tail.clone()))
    }
    fn process_start(&mut self, _: u32) -> Result<f64> {
        Ok(100.0)
    }
    fn terminate_civ(&mut self, _: u32, _: f64) -> Result<()> {
        Ok(())
    }
    fn reporter_ids(&mut self) -> Result<BTreeSet<u32>> {
        Ok(BTreeSet::new())
    }
    fn reporter_path(&mut self, _: u32) -> Result<Option<String>> {
        Ok(None)
    }
    fn terminate_reporter(&mut self, _: u32) -> Result<()> {
        Ok(())
    }
    fn launch_uri(&mut self, _: &str) -> Result<()> {
        Ok(())
    }
    fn find_window(&mut self) -> Result<isize> {
        self.windows += 1;
        Ok(123)
    }
    fn process_name(&mut self, _: isize) -> Result<String> {
        Ok("civilizationvi.exe".into())
    }
    fn now(&mut self) -> f64 {
        self.now
    }
    fn sleep(&mut self, seconds: f64) {
        self.now += seconds;
    }
}

#[test]
fn test_wrong_steam_executable_stops_before_click_or_tuner() {
    let mut fake = FakeBackend::new().process(processes(&[None, None, None], None));
    fake.on_dx11 = Box::new(|hwnd| {
        assert_eq!(hwnd, 123);
        Err(Error::input("expected CivilizationVI.exe"))
    });
    let mut h = Harness::new(json!({}), fake);
    let err = h.runner.restart_game(360.0).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Input);
    assert!(err.to_string().contains("expected CivilizationVI.exe"));
    let events = h.events();
    assert_eq!(events.iter().filter(|s| *s == "launch").count(), 1);
    assert!(events.contains(&"dx11:123".into()));
    assert!(!events
        .iter()
        .any(|s| s.starts_with("click:") || s.starts_with("lua:")));
}

#[test]
fn test_restart_action_validation() {
    assert!(serde_json::Number::from_f64(f64::INFINITY).is_none());
    scenario::validate_scenario(
        &json!({"name":"restart","steps":[{"action":"restart_game","timeout":300}]}),
    )
    .unwrap();
    for step in [
        json!({"action":"restart_game","timeout":0}),
        json!({"action":"restart_game","timeout":f64::INFINITY}),
        json!({"action":"restart_game","ids":["26c967d0-c10d-4eeb-8aaf-c83dc7e621e7"]}),
    ] {
        assert!(scenario::validate_scenario(&json!({"name":"restart","steps":[step]})).is_err());
    }
}

#[test]
fn test_auto_restart_precedes_first_step_and_log_baseline() {
    let mut fake = FakeBackend::new().process(processes(
        &[
            Some((12, 100.0)),
            Some((12, 100.0)),
            None,
            None,
            Some((13, 300.0)),
        ],
        Some((13, 300.0)),
    ));
    fake.on_newest = Box::new(|| Ok(Some(("mod.dds".into(), 200.0))));
    let (log_path, slot) = (Arc::new(Mutex::new(None)), Arc::new(Mutex::new(false)));
    let path_ref = log_path.clone();
    let wrote = slot.clone();
    fake.on_window = Box::new(move |_| {
        let path = path_ref.lock().unwrap().clone().unwrap();
        std::fs::write(path, "after restart\n").unwrap();
        *wrote.lock().unwrap() = true;
        Ok(123)
    });
    let mut h = Harness::new(pause_scenario(), fake);
    *log_path.lock().unwrap() = Some(h.runner.logs.join("Lua.log"));
    prepare_run(&mut h);
    assert_eq!(h.runner.run().unwrap(), 0);
    assert!(*slot.lock().unwrap());
    let rows = h.jsonl("steps.jsonl");
    assert_eq!(
        rows.iter()
            .take(4)
            .map(|r| (r["action"].as_str().unwrap(), r["status"].as_str().unwrap()))
            .collect::<Vec<_>>(),
        [
            ("start", "OK"),
            ("auto_restart", "START"),
            ("auto_restart", "OK"),
            ("1/1 pause", "START")
        ]
    );
    assert!(rows[1]["detail"].as_str().unwrap().contains("mod.dds"));
    assert_eq!(
        h.runner.baseline.as_ref().unwrap()["files"]["Lua.log"]["offset"]
            .as_u64()
            .unwrap(),
        std::fs::metadata(h.runner.logs.join("Lua.log"))
            .unwrap()
            .len()
    );
    let events = h.events();
    assert!(
        position(&events, "launch") < events.iter().rposition(|s| s == "lua:FrontEnd").unwrap()
    );
}

#[test]
fn test_no_auto_restart_skips_freshness_query() {
    let fake = FakeBackend::new();
    let mut h = Harness::new(pause_scenario(), fake);
    h.runner.auto_restart = false;
    prepare_run(&mut h);
    assert_eq!(h.runner.run().unwrap(), 0);
    let events = h.events();
    assert!(!events.contains(&"mods".into()));
    assert!(!events.contains(&"newest".into()));
    assert!(!events.contains(&"launch".into()));
}

#[test]
fn test_still_stale_after_restart_stops_before_first_step() {
    let mut fake = FakeBackend::new().process(processes(
        &[
            Some((12, 100.0)),
            Some((12, 100.0)),
            None,
            None,
            Some((13, 300.0)),
        ],
        Some((13, 300.0)),
    ));
    fake.on_newest = Box::new(|| Ok(Some(("future.dds".into(), 500.0))));
    let mut h = Harness::new(pause_scenario(), fake);
    prepare_run(&mut h);
    assert_eq!(h.runner.run().unwrap(), 1);
    let rows = h.jsonl("steps.jsonl");
    assert_eq!(rows.last().unwrap()["action"], "run");
    assert_eq!(rows.last().unwrap()["status"], "FAIL");
    assert!(rows.last().unwrap()["detail"]
        .as_str()
        .unwrap()
        .contains("still stale"));
    assert!(!rows.iter().any(|r| r["action"] == "1/1 pause"));
}

#[test]
fn test_restart_action_launches_and_clicks_verified_window() {
    let fake = FakeBackend::new().process(processes(&[None, None, None], None));
    let mut h = Harness::new(json!({}), fake);
    h.runner
        .run_step(&json!({"action":"restart_game"}))
        .unwrap();
    let events = h.events();
    assert_eq!(events.iter().filter(|s| *s == "launch").count(), 1);
    assert_eq!(events.iter().filter(|s| *s == "window").count(), 1);
    assert_eq!(events.iter().filter(|s| *s == "click:0.5:0.15").count(), 1);
    assert_eq!(events.iter().filter(|s| *s == "dx11:123").count(), 1);
    assert!(position(&events, "dx11") < position(&events, "click:"));
}

#[test]
fn test_frontend_probe_reconnects_after_unknown_read_only_reply() {
    let mut first = true;
    let fake = FakeBackend::new().lua(move |_, _| {
        if first {
            first = false;
            Err(Error::unknown("reply lost"))
        } else {
            Ok("true".into())
        }
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.wait_frontend_ready(10.0, false).unwrap();
    assert_eq!(h.count_code("return UI.IsInFrontEnd()"), 2);
    assert_eq!(h.events().iter().filter(|s| *s == "disconnect").count(), 1);
}

#[test]
fn test_frontend_probe_clicks_again_if_movie_still_plays() {
    let clicked = Arc::new(Mutex::new(false));
    let seen = clicked.clone();
    let fake = FakeBackend::new().lua(move |_, _| Ok(seen.lock().unwrap().to_string()));
    let mut h = Harness::new(json!({}), fake);
    // The first probe observes the movie, and the injected clock reaches the retry click.
    let events = h.events.clone();
    let clicked = clicked.clone();
    let clock = h.clock.clone();
    h.runner.clock = Arc::new(ClickClock {
        clock,
        events,
        clicked,
    });
    h.runner.wait_frontend_ready(30.0, true).unwrap();
    assert_eq!(
        h.events().iter().filter(|s| *s == "click:0.5:0.15").count(),
        1
    );
}
struct ClickClock {
    clock: Arc<ManualClock>,
    events: Arc<Mutex<Vec<String>>>,
    clicked: Arc<Mutex<bool>>,
}
impl civ6_autotest_rs::time::Clock for ClickClock {
    fn now(&self) -> f64 {
        self.clock.at()
    }
    fn sleep(&self, seconds: f64) {
        self.clock.advance(seconds);
        if self
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|s| s == "click:0.5:0.15")
        {
            *self.clicked.lock().unwrap() = true;
        }
    }
}

#[test]
fn test_steam_launch_retries_when_prior_session_blocks_first_uri() {
    let clock = Arc::new(ManualClock::default());
    let c = clock.clone();
    let mut windows = 0;
    let mut fake = FakeBackend::new().process(processes(&[], None));
    fake.on_window = Box::new(move |deadline| {
        c.advance((deadline - c.at()).max(0.0));
        windows += 1;
        if windows <= 9 {
            Err(Error::input("Steam has not started Civ VI"))
        } else {
            Ok(123)
        }
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.clock = clock;
    h.runner.launch_steam_and_wait(120.0).unwrap();
    assert_eq!(h.events().iter().filter(|s| *s == "launch").count(), 2);
}

#[test]
fn test_failed_new_launch_releases_its_helper_before_next_steam_request() {
    let clock = Arc::new(ManualClock::default());
    let state = Arc::new(Mutex::new((0, false)));
    let mut fake = FakeBackend::new().process(|_| Ok(None));
    let events = fake.events.clone();
    let cleanup_state = state.clone();
    fake.on_close_orphans = Box::new(move || {
        let mut state = cleanup_state.lock().unwrap();
        if state.1 {
            state.1 = false;
            events
                .lock()
                .unwrap()
                .push("closed failed launch helper".into());
            Ok(vec![99])
        } else {
            Ok(Vec::new())
        }
    });
    let launch_state = state.clone();
    fake.on_launch = Box::new(move || {
        let mut state = launch_state.lock().unwrap();
        assert!(!state.1, "Steam is still blocked by the crashed launch");
        state.0 += 1;
        state.1 = state.0 == 1;
        Ok(())
    });
    let wait_clock = clock.clone();
    fake.on_window = Box::new(move |deadline| {
        wait_clock.advance(deadline - wait_clock.at());
        if state.lock().unwrap().0 < 2 {
            Err(Error::input("first launch crashed"))
        } else {
            Ok(123)
        }
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.clock = clock;
    h.runner.launch_steam_and_wait(120.0).unwrap();
    let events: Vec<_> = h
        .events()
        .into_iter()
        .filter(|event| event == "launch" || event == "closed failed launch helper")
        .collect();
    assert_eq!(events, ["launch", "closed failed launch helper", "launch"]);
}

#[test]
fn test_helper_cleanup_exhausting_deadline_sends_no_steam_request() {
    let clock = Arc::new(ManualClock::default());
    let mut fake = FakeBackend::new().process(|_| Ok(None));
    fake.on_close_orphans = Box::new(|| Ok(vec![99]));
    let mut h = Harness::new(json!({}), fake);
    h.runner.clock = clock.clone();
    let error = h.runner.launch_steam_and_wait(5.0).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Scenario);
    assert!(error.message.contains("deadline expired"), "{error}");
    assert_eq!(clock.at(), 5.0);
    assert!(
        h.events().is_empty(),
        "no launch or window query after cleanup used the deadline"
    );
}

#[test]
fn test_four_launch_requests_still_wait_for_delayed_window_until_deadline() {
    let clock = Arc::new(ManualClock::default());
    let wait_clock = clock.clone();
    let mut fake = FakeBackend::new().process(|_| Ok(None));
    let events = fake.events.clone();
    fake.on_close_orphans = Box::new(move || {
        events.lock().unwrap().push("cleanup".into());
        Ok(Vec::new())
    });
    fake.on_window = Box::new(move |deadline| {
        wait_clock.advance(deadline - wait_clock.at());
        if wait_clock.at() < 230.0 {
            Err(Error::input("Steam launch is still pending"))
        } else {
            Ok(123)
        }
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.clock = clock.clone();
    h.runner.launch_steam_and_wait(420.0).unwrap();
    assert_eq!(clock.at(), 230.0);
    let events: Vec<_> = h
        .events()
        .into_iter()
        .filter(|event| event == "cleanup" || event == "launch")
        .collect();
    assert_eq!(
        events,
        ["cleanup", "launch", "cleanup", "launch", "cleanup", "launch", "cleanup", "launch"]
    );
}

#[test]
fn test_exhausted_launch_requests_fail_at_the_supplied_deadline() {
    let clock = Arc::new(ManualClock::default());
    let wait_clock = clock.clone();
    let mut fake = FakeBackend::new().process(|_| Ok(None));
    fake.on_window = Box::new(move |deadline| {
        wait_clock.advance(deadline - wait_clock.at());
        Err(Error::input("no window"))
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.clock = clock.clone();
    let error = h.runner.launch_steam_and_wait(420.0).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Scenario);
    assert!(error
        .message
        .contains("after 4 Steam launch attempts within the deadline"));
    assert_eq!(clock.at(), 420.0);
    assert_eq!(
        h.events().iter().filter(|event| *event == "launch").count(),
        4
    );
}

#[test]
fn helper_cleanup_manual_launch_prevents_duplicate_steam_request() {
    let clock = Arc::new(ManualClock::default());
    let cleaned = Arc::new(Mutex::new(false));
    let process_cleaned = cleaned.clone();
    let mut fake = FakeBackend::new().process(move |_| {
        Ok(if *process_cleaned.lock().unwrap() {
            Some((12, 100.0))
        } else {
            None
        })
    });
    fake.on_close_orphans = Box::new(move || {
        *cleaned.lock().unwrap() = true;
        Ok(vec![99])
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.clock = clock.clone();
    h.runner.launch_steam_and_wait(120.0).unwrap();
    assert_eq!(clock.at(), 8.0);
    assert_eq!(h.events(), ["window", "dx11:123"]);
}

#[test]
fn restart_cleans_helpers_before_waiting_for_steam_to_release_session() {
    let mut fake = FakeBackend::new().process(|_| Ok(None));
    let events = fake.events.clone();
    let mut first = true;
    fake.on_close_orphans = Box::new(move || {
        events.lock().unwrap().push("cleanup".into());
        let closed = if first { vec![99] } else { Vec::new() };
        first = false;
        Ok(closed)
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.restart_game(360.0).unwrap();
    assert_eq!(h.clock.at(), 16.0);
    let events: Vec<_> = h
        .events()
        .into_iter()
        .filter(|event| event == "cleanup" || event == "launch")
        .collect();
    assert_eq!(events, ["cleanup", "cleanup", "launch"]);
}

#[test]
fn test_single_process_waits_for_exiting_process_to_disappear() {
    let mut fake = NativeFixture::new(&[&[1, 2], &[1, 2], &[2]]);
    assert_eq!(
        restart::single_civ_process_with(&mut fake, 30.0).unwrap(),
        Some((2, 100.0))
    );
    assert_eq!(fake.now, 1.0);
}

#[test]
fn test_single_process_still_refuses_when_duplicates_persist() {
    let mut fake = NativeFixture::new(&[&[1, 2]]);
    assert_eq!(
        restart::single_civ_process_with(&mut fake, 0.0)
            .unwrap_err()
            .kind,
        ErrorKind::Input
    );
    assert!(restart::single_civ_process_with(&mut fake, 0.0)
        .unwrap_err()
        .to_string()
        .contains("Multiple Civ VI processes"));
}

#[test]
fn test_window_wait_ignores_transient_duplicate_processes() {
    let mut fake = NativeFixture::new(&[&[1, 2], &[12]]);
    assert_eq!(
        restart::wait_for_civ_window_with(&mut fake, 10.0).unwrap(),
        123
    );
    assert_eq!(fake.windows, 1);
}

#[test]
fn test_steam_wait_does_not_launch_again_during_process_overlap() {
    let clock = Arc::new(ManualClock::default());
    let c = clock.clone();
    let mut first = true;
    let mut fake = FakeBackend::new().process(move |_| {
        if first {
            first = false;
            Ok(None)
        } else {
            Err(Error::input("Multiple Civ VI processes found (2)"))
        }
    });
    let mut window_first = true;
    fake.on_window = Box::new(move |deadline| {
        c.advance((deadline - c.at()).max(0.0));
        if window_first {
            window_first = false;
            Err(Error::input("launcher still opening"))
        } else {
            Ok(123)
        }
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.clock = clock;
    h.runner.launch_steam_and_wait(120.0).unwrap();
    assert_eq!(h.events().iter().filter(|s| *s == "launch").count(), 1);
}

#[test]
fn test_first_restart_action_can_start_stopped_game_with_auto_disabled() {
    let fake = FakeBackend::new().process(processes(&[None, None, None], Some((13, 300.0))));
    let mut h = Harness::new(
        json!({"name":"manual restart","steps":[{"action":"restart_game"}]}),
        fake,
    );
    h.runner.auto_restart = false;
    prepare_run(&mut h);
    assert_eq!(h.runner.run().unwrap(), 0);
    assert_eq!(h.events().iter().filter(|s| *s == "launch").count(), 1);
    assert!(!h.events().contains(&"mods".into()));
}

#[test]
fn test_restart_requests_ui_quit_before_steam_launch() {
    let fake = FakeBackend::new().process(processes(&[Some((12, 100.0)), None, None], None));
    let mut h = Harness::new(json!({}), fake);
    h.runner.restart_game(360.0).unwrap();
    let calls = h.calls();
    assert!(calls
        .iter()
        .any(|(_, c)| c.contains("Events.UserConfirmedClose()")));
    let events = h.events();
    assert!(position(&events, "lua:FrontEnd") < position(&events, "launch"));
    assert!(!events.iter().any(|s| s.starts_with("terminate:")));
}

#[test]
fn test_restart_forces_only_same_process_after_grace() {
    let mut first_states = true;
    let mut fake = FakeBackend::new()
        .process(processes(
            &[Some((12, 100.0)), Some((12, 100.0)), None, None],
            None,
        ))
        .states(move || {
            if first_states {
                first_states = false;
                Err(Error::input("no window"))
            } else {
                Ok(vec!["FrontEnd".into()])
            }
        });
    let mut exited = false;
    fake.on_exit = Box::new(move |_| {
        let was = exited;
        exited = true;
        Ok(was)
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.restart_game(360.0).unwrap();
    let events = h.events();
    assert_eq!(events.iter().filter(|s| *s == "close").count(), 1);
    assert_eq!(
        events.iter().filter(|s| *s == "terminate:12:100").count(),
        1
    );
}

#[test]
fn test_restart_without_process_launches_then_waits_for_frontend() {
    let fake = FakeBackend::new().process(processes(&[None, None, None], None));
    let mut h = Harness::new(json!({}), fake);
    h.runner.restart_game(360.0).unwrap();
    let events = h.events();
    assert!(position(&events, "launch") < position(&events, "window"));
    assert!(position(&events, "window") < position(&events, "click:"));
    assert!(position(&events, "click:") < position(&events, "lua:FrontEnd"));
    assert!(!events
        .iter()
        .any(|s| s == "close" || s.starts_with("terminate:")));
}

#[test]
fn test_restart_still_refuses_when_several_processes_persist() {
    let fake =
        FakeBackend::new().process(|_| Err(Error::input("Multiple Civ VI processes found (2)")));
    let mut h = Harness::new(json!({}), fake);
    assert_eq!(
        h.runner.restart_game(360.0).unwrap_err().kind,
        ErrorKind::Input
    );
    assert!(!h
        .events()
        .iter()
        .any(|s| s == "launch" || s.starts_with("terminate:")));
}

#[test]
fn test_run_launches_civ_when_not_running_and_first_step_needs_game() {
    let fake = FakeBackend::new().process(processes(&[None, None, None, None], Some((13, 300.0))));
    let mut h = Harness::new(pause_scenario(), fake);
    h.runner.auto_restart = false;
    prepare_run(&mut h);
    assert_eq!(h.runner.run().unwrap(), 0);
    let rows = h.jsonl("steps.jsonl");
    assert!(rows
        .iter()
        .any(|r| r["action"] == "launch" && r["status"] == "START"));
    assert_eq!(h.events().iter().filter(|s| *s == "launch").count(), 1);
}
