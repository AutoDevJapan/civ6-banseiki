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
fn steam_sync_must_finish_before_relaunch_after_game_exit() {
    let mut fake = FakeBackend::new().process(processes(&[Some((12, 100.0)), None, None], None));
    let sync_checks = Arc::new(Mutex::new(0));
    let checks = sync_checks.clone();
    fake.on_sync = Box::new(move |_| {
        let mut n = checks.lock().unwrap();
        *n += 1;
        Ok(if *n < 4 {
            civ6_autotest_rs::steam_sync::SyncStatus::Pending
        } else {
            civ6_autotest_rs::steam_sync::SyncStatus::Idle
        })
    });
    let checks = sync_checks.clone();
    fake.on_launch = Box::new(move || {
        assert!(*checks.lock().unwrap() >= 4);
        Ok(())
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.restart_game(120.0).unwrap();
    assert_eq!(h.events().iter().filter(|e| *e == "launch").count(), 1);
    assert!(!h.events().iter().any(|e| e.starts_with("terminate:")));
    assert!(h.clock.at() >= 17.5);
}

#[test]
fn steam_sync_deadline_expiry_never_sends_a_launch() {
    let mut fake = FakeBackend::new().process(processes(&[Some((12, 100.0)), None], None));
    fake.on_sync = Box::new(|_| Ok(civ6_autotest_rs::steam_sync::SyncStatus::Pending));
    let mut h = Harness::new(json!({}), fake);
    assert!(h.runner.restart_game(30.0).is_err());
    assert!(!h.events().contains(&"launch".into()));
    assert_eq!(h.clock.at(), 30.0);
}

#[test]
fn an_active_steam_warning_suppresses_repeated_launch_requests() {
    let clock_holder = Arc::new(Mutex::new(None::<Arc<ManualClock>>));
    let clock = clock_holder.clone();
    let mut count = 0;
    let mut fake = FakeBackend::new().process(processes(&[None], None));
    fake.on_warning = Box::new(|| Ok(true));
    fake.on_window = Box::new(move |_| {
        count += 1;
        clock.lock().unwrap().as_ref().unwrap().advance(5.0);
        if count < 13 {
            Err(Error::input("Steam warning is still present"))
        } else {
            Ok(123)
        }
    });
    let mut h = Harness::new(json!({}), fake);
    *clock_holder.lock().unwrap() = Some(h.clock.clone());
    h.runner.restart_game(120.0).unwrap();
    assert_eq!(h.events().iter().filter(|e| *e == "launch").count(), 1);
    assert!(h.clock.at() >= 65.0);
}

#[test]
fn exit_and_prelaunch_failures_send_zero_uri_requests_for_every_failure_kind() {
    use civ6_autotest_rs::steam_sync::{Stage, SyncStatus};
    for old in [None, Some((12, 100.0))] {
        for failure in ["Failed", "Conflict", "Offline"] {
            let mut fake = FakeBackend::new().process(processes(&[old, None, None], None));
            fake.on_sync = Box::new(move |stage| {
                assert_eq!(
                    stage,
                    if old.is_some() {
                        Stage::Exit
                    } else {
                        Stage::Prelaunch
                    },
                    "{failure}"
                );
                Ok(SyncStatus::Failed)
            });
            let mut h = Harness::new(json!({}), fake);
            assert!(h
                .runner
                .restart_game(120.0)
                .unwrap_err()
                .message
                .contains("URI not sent"));
            assert!(!h.events().contains(&"launch".into()));
        }
    }
}

#[test]
fn issued_request_failure_allows_warning_recovery_without_uri_replay_after_45_seconds() {
    use civ6_autotest_rs::steam_sync::{Stage, SyncStatus};
    let clock = Arc::new(ManualClock::default());
    let waits = clock.clone();
    let mut fake = FakeBackend::new().process(|_| Ok(None));
    fake.on_sync = Box::new(|stage| {
        Ok(if stage == Stage::LaunchIssued {
            SyncStatus::Failed
        } else {
            SyncStatus::Idle
        })
    });
    fake.on_warning = Box::new(|| Ok(true));
    fake.on_window = Box::new(move |deadline| {
        waits.advance(deadline - waits.at());
        if waits.at() < 70.0 {
            Err(Error::input("Steam still starting"))
        } else {
            Ok(123)
        }
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.clock = clock;
    h.runner.launch_steam_and_wait(120.0).unwrap();
    assert_eq!(h.events().iter().filter(|e| *e == "launch").count(), 1);
}

#[test]
fn sync_failure_during_helper_cleanup_wait_sends_no_uri() {
    use civ6_autotest_rs::steam_sync::{Stage, SyncStatus};
    let clock = Arc::new(ManualClock::default());
    let observed = clock.clone();
    let mut fake = FakeBackend::new().process(|_| Ok(None));
    fake.on_close_orphans = Box::new(|| Ok(vec![99]));
    fake.on_sync = Box::new(move |stage| {
        assert_eq!(stage, Stage::Prelaunch);
        Ok(if observed.at() >= 8.0 {
            SyncStatus::Failed
        } else {
            SyncStatus::Idle
        })
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.clock = clock;
    assert!(h
        .runner
        .launch_steam_and_wait(120.0)
        .unwrap_err()
        .message
        .contains("URI not sent"));
    assert!(!h.events().contains(&"launch".into()));
}

#[test]
fn late_prelaunch_pending_waits_before_the_single_uri() {
    use civ6_autotest_rs::steam_sync::SyncStatus;
    let clock = Arc::new(ManualClock::default());
    let before_launch = clock.clone();
    let mut fake = FakeBackend::new().process(|_| Ok(None));
    let mut polls = 0;
    fake.on_sync = Box::new(move |_| {
        polls += 1;
        Ok(if polls == 2 {
            SyncStatus::Pending
        } else {
            SyncStatus::Idle
        })
    });
    fake.on_launch = Box::new(move || {
        assert!(
            before_launch.at() >= 0.5,
            "pending must wait without a busy launch loop"
        );
        Ok(())
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.clock = clock;
    h.runner.launch_steam_and_wait(120.0).unwrap();
    assert_eq!(
        h.events().iter().filter(|event| *event == "launch").count(),
        1
    );
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
    assert!(events.contains(&"mods".into()));
    assert!(events.contains(&"stale".into()));
    assert!(!events.contains(&"newest".into()));
    assert!(!events.contains(&"launch".into()));
}

#[test]
fn chain_mode_missing_mod_stops_before_any_launch_or_tuner_access() {
    let mut fake = FakeBackend::new();
    fake.on_mods = Box::new(|| {
        Err(Error::input(
            "Scenario mod is not installed: requested UUID",
        ))
    });
    let mut h = Harness::new(
        json!({"name":"missing mod","steps":[{"action":"restart_game"}]}),
        fake,
    );
    h.runner.auto_restart = false;
    prepare_run(&mut h);
    assert_eq!(h.runner.run().unwrap(), 1);
    assert_eq!(h.events(), ["mods", "disconnect"]);
    let failure: Value =
        serde_json::from_slice(&std::fs::read(h.runner.output.join("failure.json")).unwrap())
            .unwrap();
    assert_eq!(failure["stage"], "preflight");
    assert!(failure["message"]
        .as_str()
        .unwrap()
        .contains("requested UUID"));
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
fn test_frontend_background_does_not_count_as_ready_menu() {
    let fake = FakeBackend::new().lua(|state, _| Ok((state == "FrontEnd").to_string()));
    let mut h = Harness::new(json!({}), fake);
    let error = h.runner.wait_frontend_ready(2.0, false).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Scenario);
    assert_eq!(h.clock.at(), 2.0);
}

#[test]
fn test_frontend_waits_for_menu_animation() {
    let menu_probes = Arc::new(Mutex::new(0));
    let observed = menu_probes.clone();
    let fake = FakeBackend::new().lua(move |state, _| {
        if state == "MainMenu" {
            let mut count = observed.lock().unwrap();
            *count += 1;
            return Ok((*count >= 3).to_string());
        }
        Ok("true".into())
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.wait_frontend_ready(5.0, false).unwrap();
    assert_eq!(*menu_probes.lock().unwrap(), 3);
    assert_eq!(h.clock.at(), 1.0);
}

#[test]
fn test_menu_probe_requires_visible_options_and_finished_animation() {
    use civ6_autotest_rs::runner::MAIN_MENU_READY;
    let lua = mlua::Lua::new();
    lua.load(r#"
        ContextPtr={IsHidden=function() return HIDDEN end}
        local label={GetID=function() return 'LabelAlphaAnim' end,IsStopped=function() return LABEL_STOPPED end,GetChildren=function() return {} end}
        local option={GetID=function() return 'Top' end,GetChildren=function() return {label} end}
        local alpha={IsStopped=function() return ALPHA_STOPPED end}
        Controls={MainMenuOptionStack={GetID=function() return 'MainMenuOptionStack' end,GetSizeY=function() return HEIGHT end,GetParent=function() return alpha end,GetChildren=function() return {option} end},MainButtonTrackAnim={IsStopped=function() return STOPPED end}}
    "#).exec().unwrap();
    for (hidden, height, stopped, alpha, label, expected) in [
        (true, 306, true, true, true, false),
        (false, 0, true, true, true, false),
        (false, 306, false, true, true, false),
        (false, 306, true, false, true, false),
        (false, 306, true, true, false, false),
        (false, 306, true, true, true, true),
    ] {
        lua.globals().set("HIDDEN", hidden).unwrap();
        lua.globals().set("HEIGHT", height).unwrap();
        lua.globals().set("STOPPED", stopped).unwrap();
        lua.globals().set("ALPHA_STOPPED", alpha).unwrap();
        lua.globals().set("LABEL_STOPPED", label).unwrap();
        assert_eq!(lua.load(MAIN_MENU_READY).eval::<bool>().unwrap(), expected);
    }
}

fn connection_message_fixture(title: Option<&str>, translated_title: &str) -> mlua::Lua {
    let lua = mlua::Lua::new();
    lua.globals().set("TITLE", title).unwrap();
    lua.globals()
        .set("TRANSLATED_TITLE", translated_title)
        .unwrap();
    lua.load(r#"
        HIDDEN=false; RESPONSES=0
        ContextPtr={IsHidden=function() return HIDDEN end}
        Locale={Lookup=function(key) assert(key=='TXT_KEY_MY2K_ADDITION_CONNECTION_ISSUE'); return TRANSLATED_TITLE end}
        KeyEvents={KeyUp=1}; Keys={VK_ESCAPE=27}
        local title={GetID=function() return 'Title' end,GetText=function() return TITLE end,GetChildren=function() return {} end}
        local window={GetID=function() return 'WindowData' end,GetChildren=function() return TITLE and {title} or {} end}
        Controls={My2KBG={GetID=function() return 'My2KBG' end,GetChildren=function() return {window} end}}
        function InputHandler(event, key, value)
            assert(event==KeyEvents.KeyUp and key==Keys.VK_ESCAPE and value==0)
            RESPONSES=RESPONSES+1; HIDDEN=true; TITLE=nil
        end
    "#).exec().unwrap();
    lua
}

#[test]
fn test_connection_notification_acknowledges_once_in_each_language() {
    use civ6_autotest_rs::runner::DISMISS_2K_CONNECTION_MESSAGE;
    for title in ["Connection Issue", "接続の問題"] {
        let lua = connection_message_fixture(Some(title), title);
        assert!(lua
            .load(DISMISS_2K_CONNECTION_MESSAGE)
            .eval::<bool>()
            .unwrap());
        assert!(!lua
            .load(DISMISS_2K_CONNECTION_MESSAGE)
            .eval::<bool>()
            .unwrap());
        assert_eq!(lua.globals().get::<u32>("RESPONSES").unwrap(), 1);
    }
}

#[test]
fn test_connection_notification_keeps_other_dialogs_open() {
    use civ6_autotest_rs::runner::DISMISS_2K_CONNECTION_MESSAGE;
    for title in [
        None,
        Some("Sign In"),
        Some("Terms of Service"),
        Some("Logout"),
        Some("Other Error"),
    ] {
        let lua = connection_message_fixture(title, "Connection Issue");
        assert!(!lua
            .load(DISMISS_2K_CONNECTION_MESSAGE)
            .eval::<bool>()
            .unwrap());
        assert_eq!(lua.globals().get::<u32>("RESPONSES").unwrap(), 0);
        assert!(!lua.globals().get::<bool>("HIDDEN").unwrap());
    }
}

#[test]
fn test_menu_probe_rechecks_after_lost_connection_message_acknowledgment() {
    use civ6_autotest_rs::runner::DISMISS_2K_CONNECTION_MESSAGE;
    let acknowledged = Arc::new(Mutex::new(0));
    let seen = acknowledged.clone();
    let fake = FakeBackend::new()
        .states(|| Ok(vec!["FrontEnd".into(), "MainMenu".into(), "My2K".into()]))
        .lua(move |state, code| match state {
            "FrontEnd" => Ok("true".into()),
            "MainMenu" => Ok((*seen.lock().unwrap() > 0).to_string()),
            "My2K" => {
                assert_eq!(code, DISMISS_2K_CONNECTION_MESSAGE);
                let lua = connection_message_fixture(Some("Connection Issue"), "Connection Issue");
                assert!(lua.load(code).eval::<bool>().unwrap());
                *seen.lock().unwrap() += 1;
                Err(Error::unknown(
                    "notification closed but acknowledgment lost",
                ))
            }
            _ => panic!("unexpected state {state}"),
        });
    let mut h = Harness::new(json!({}), fake);
    h.runner.wait_frontend_ready(5.0, false).unwrap();
    assert_eq!(*acknowledged.lock().unwrap(), 1);
    assert_eq!(h.events().iter().filter(|s| *s == "disconnect").count(), 1);
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
fn test_steam_launch_waits_for_delayed_prior_session_without_replaying_uri() {
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
    assert_eq!(h.events().iter().filter(|s| *s == "launch").count(), 1);
}

#[test]
fn test_failed_new_launch_is_not_replayed_without_a_proven_new_attempt() {
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
    assert!(h.runner.launch_steam_and_wait(120.0).is_err());
    let events: Vec<_> = h
        .events()
        .into_iter()
        .filter(|event| event == "launch" || event == "closed failed launch helper")
        .collect();
    assert_eq!(events, ["launch"]);
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
fn test_single_launch_request_waits_for_delayed_window_until_deadline() {
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
    assert_eq!(events, ["cleanup", "launch"]);
}

#[test]
fn test_single_unresolved_launch_fails_at_the_supplied_deadline() {
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
        .contains("after 1 Steam launch attempts within the deadline"));
    assert_eq!(clock.at(), 420.0);
    assert_eq!(
        h.events().iter().filter(|event| *event == "launch").count(),
        1
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
    assert!(h.events().contains(&"mods".into()));
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
