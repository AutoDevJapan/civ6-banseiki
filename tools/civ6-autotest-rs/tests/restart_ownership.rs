mod support_driver;

use civ6_autotest_rs::{
    restart::ProcessRow,
    runner::{Backend, Runner},
    startup::{self, GameProcess},
    Error, Result,
};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use support_driver::{FakeBackend, Harness, ManualClock};

const CHAIN_CREATED: u64 = 116_444_736_000_000_000 + 100 * 10_000_000;
const OLD_GAME: (u32, f64) = (12, 300.0);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Change {
    None,
    DuringTuner,
    DuringIdentityCheck,
    BeforeWindowClose,
    BeforeForce,
    BeforeLaunch,
    ReusedGame,
}
struct State {
    record: Option<Value>,
    lock_path: String,
    caller: u32,
    rows: Vec<ProcessRow>,
    parent_created: u64,
    alive: Option<(u32, f64)>,
    calls: Vec<&'static str>,
    change: Change,
    force: bool,
    states_seen: bool,
    launches: usize,
    process_reads: usize,
}
impl State {
    fn owned() -> Self {
        Self {
            record: Some(json!({"live_chain":true,"pid":7,"chain_pid":7,
                "pid_created_filetime":CHAIN_CREATED,"acquired_at":200.0,"lock_token":"owner-a"})),
            lock_path: startup::SHARED_LOCK_PATH.into(),
            caller: 9,
            rows: vec![
                ProcessRow {
                    pid: 9,
                    parent_pid: 8,
                    name: "driver.exe".into(),
                },
                ProcessRow {
                    pid: 8,
                    parent_pid: 7,
                    name: "parent.exe".into(),
                },
            ],
            parent_created: CHAIN_CREATED + 10_000_000,
            alive: Some(OLD_GAME),
            calls: Vec::new(),
            change: Change::None,
            force: false,
            states_seen: false,
            launches: 0,
            process_reads: 0,
        }
    }
    fn revoke(&mut self) {
        self.record.as_mut().unwrap()["lock_token"] = json!("owner-b");
    }
    fn verify(&self, token: &str) -> Result<()> {
        startup::verify_control_path(Path::new(&self.lock_path), startup::SHARED_LOCK_PATH)?;
        startup::verify_control_record(
            self.record
                .as_ref()
                .ok_or_else(|| Error::input("no owner proof"))?,
            7,
            CHAIN_CREATED,
            token,
            1000.0,
        )?;
        startup::verify_control_ancestry(self.caller, 7, &self.rows, |pid| match pid {
            7 => Ok(CHAIN_CREATED),
            8 => Ok(self.parent_created),
            9 => Ok(CHAIN_CREATED + 20_000_000),
            _ => Err(Error::input("unknown caller")),
        })
    }
}
struct Game {
    state: Arc<Mutex<State>>,
    token: Option<String>,
}
impl Backend for Game {
    fn acquire_startup_lock(&mut self) -> Result<()> {
        let state = self.state.lock().unwrap();
        let token = state
            .record
            .as_ref()
            .and_then(|r| r["lock_token"].as_str())
            .ok_or_else(|| Error::input("no owner proof"))?;
        state.verify(self.token.as_deref().unwrap_or(token))?;
        self.token.get_or_insert_with(|| token.to_owned());
        Ok(())
    }
    fn check_startup_lock(&mut self) -> Result<()> {
        self.state.lock().unwrap().verify(
            self.token
                .as_deref()
                .ok_or_else(|| Error::input("lease not acquired"))?,
        )
    }
    fn release_startup_lock(&mut self) {
        self.token = None;
    }
    fn single_process(&mut self, _: f64) -> Result<Option<(u32, f64)>> {
        let mut s = self.state.lock().unwrap();
        s.process_reads += 1;
        if s.change == Change::DuringIdentityCheck && s.process_reads == 2 {
            s.revoke();
        }
        if s.change == Change::BeforeWindowClose && s.states_seen {
            s.revoke();
        }
        Ok(s.alive)
    }
    fn require_single_process(&mut self) -> Result<()> {
        Ok(())
    }
    fn list_states(&mut self, _: f64) -> Result<Vec<String>> {
        let mut s = self.state.lock().unwrap();
        s.states_seen = true;
        if s.change == Change::DuringTuner {
            s.revoke();
        }
        if s.change == Change::ReusedGame {
            s.alive = Some((OLD_GAME.0, OLD_GAME.1 + 1.0));
        }
        Ok(if s.force && s.launches == 0 {
            vec![]
        } else {
            vec!["FrontEnd".into(), "MainMenu".into()]
        })
    }
    fn run_lua(&mut self, _: &str, code: &str, _: f64) -> Result<String> {
        let mut s = self.state.lock().unwrap();
        if code.contains("UserConfirmedClose") {
            s.calls.push("ui_close");
            s.alive = None;
        }
        Ok("true".into())
    }
    fn request_close(&mut self, game: GameProcess) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        assert_eq!(s.alive, Some((game.pid, game.started)));
        s.calls.push("window_close");
        Err(Error::input("fixture window does not close"))
    }
    fn wait_exit(&mut self, _: u32, _: f64) -> Result<bool> {
        let mut s = self.state.lock().unwrap();
        if s.change == Change::BeforeForce {
            s.revoke();
        }
        Ok(s.alive.is_none())
    }
    fn terminate(&mut self, pid: u32, started: f64) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        assert_eq!(s.alive, Some((pid, started)));
        s.calls.push("terminate");
        s.alive = None;
        Ok(())
    }
    fn close_orphaned_reporters(&mut self) -> Result<Vec<u32>> {
        let mut s = self.state.lock().unwrap();
        if s.change == Change::BeforeLaunch {
            s.revoke();
        }
        Ok(vec![])
    }
    fn launch(&mut self) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        assert!(s.alive.is_none(), "a launch cannot adopt an existing game");
        s.calls.push("launch");
        s.launches += 1;
        s.alive = Some((13, 400.0));
        Ok(())
    }
    fn wait_window(&mut self, _: f64) -> Result<isize> {
        Ok(1)
    }
    fn require_dx11(&mut self, _: isize) -> Result<()> {
        Ok(())
    }
    fn click(&mut self, _: f64, _: f64) -> Result<(i32, i32)> {
        Ok((0, 0))
    }
}
fn fixture(configure: impl FnOnce(&mut State)) -> (tempfile::TempDir, Runner, Arc<Mutex<State>>) {
    let root = tempfile::tempdir().unwrap();
    let mut state = State::owned();
    configure(&mut state);
    let state = Arc::new(Mutex::new(state));
    let runner = Runner::with_backend(
        json!({}),
        root.path().into(),
        root.path().into(),
        root.path().into(),
        4318,
        Box::new(Game {
            state: state.clone(),
            token: None,
        }),
        Arc::new(ManualClock::default()),
    );
    (root, runner, state)
}

#[test]
fn missing_proof_other_lock_other_caller_and_reused_ancestors_cannot_close_or_launch() {
    for case in [
        "missing",
        "other_lock",
        "other_caller",
        "other_owner",
        "reused_chain",
        "reused_parent",
        "stale_acquisition",
    ] {
        let (_root, mut runner, state) = fixture(|s| match case {
            "missing" => s.record = None,
            "other_lock" => s.lock_path = "D:/CC/Tools/other/civ6.lock".into(),
            "other_caller" => s.caller = 10,
            "other_owner" => s.record.as_mut().unwrap()["chain_pid"] = json!(8),
            "reused_chain" => {
                s.record.as_mut().unwrap()["pid_created_filetime"] = json!(CHAIN_CREATED - 1)
            }
            "reused_parent" => s.parent_created = CHAIN_CREATED + 30_000_000,
            _ => s.record.as_mut().unwrap()["acquired_at"] = json!(99.0),
        });
        assert!(runner.restart_game(120.0).is_err(), "{case}");
        let s = state.lock().unwrap();
        assert_eq!(s.alive, Some(OLD_GAME), "{case}");
        assert!(s.calls.is_empty(), "{case}: {:?}", s.calls);
        assert_eq!(s.launches, 0);
    }
}

#[test]
fn loss_or_reuse_before_ui_and_window_close_sends_no_shutdown_and_no_launch() {
    for change in [
        Change::DuringTuner,
        Change::DuringIdentityCheck,
        Change::BeforeWindowClose,
        Change::ReusedGame,
    ] {
        let (_root, mut runner, state) = fixture(|s| {
            s.change = change;
            s.force = change == Change::BeforeWindowClose;
        });
        assert!(runner.restart_game(120.0).is_err(), "{change:?}");
        assert!(state.lock().unwrap().calls.is_empty(), "{change:?}");
    }
}

#[test]
fn lock_loss_during_exit_wait_blocks_force_and_any_relaunch() {
    let (_root, mut runner, state) = fixture(|s| {
        s.force = true;
        s.change = Change::BeforeForce;
    });
    assert!(runner.restart_game(120.0).is_err());
    let s = state.lock().unwrap();
    assert_eq!(s.calls, ["window_close"]);
    assert_eq!(s.alive, Some(OLD_GAME));
    assert_eq!(s.launches, 0);
}

#[test]
fn changed_acquisition_before_steam_launch_cannot_launch_or_retry() {
    let (_root, mut runner, state) = fixture(|s| {
        s.alive = None;
        s.change = Change::BeforeLaunch;
    });
    assert!(runner.restart_game(120.0).is_err());
    assert!(state.lock().unwrap().calls.is_empty());
}

#[test]
fn legitimate_chain_keeps_graceful_and_forced_owned_game_retirement() {
    for force in [false, true] {
        let (_root, mut runner, state) = fixture(|s| s.force = force);
        runner.restart_game(120.0).unwrap();
        let s = state.lock().unwrap();
        assert_eq!(
            s.calls,
            if force {
                vec!["window_close", "terminate", "launch"]
            } else {
                vec!["ui_close", "launch"]
            }
        );
        assert_eq!(s.launches, 1);
        assert_eq!(s.alive, Some((13, 400.0)));
    }
}

#[test]
fn legacy_fake_backend_has_no_implicit_game_control_permission() {
    for alive in [None, Some(OLD_GAME)] {
        for restart in [true, false] {
            let mut h = Harness::new(json!({}), FakeBackend::new().process(move |_| Ok(alive)));
            let result = if restart {
                h.runner.restart_game(120.0).map(|_| ())
            } else {
                h.runner.launch_steam_and_wait(120.0)
            };
            assert!(result.is_err());
            assert!(h.events().is_empty());
            assert!(h.calls().is_empty());
        }
    }
}
