use cc_live::{chain, process::CREATE_NO_WINDOW, Call, Output, Paths, Runner};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    paths: Paths,
}
impl Fixture {
    fn new(driver: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "cc-live-graceful-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let paths = Paths {
            here: root.join("here"),
            wt: root.join("wt"),
            jobs: root.join("jobs"),
            temp: root.join("temp"),
            profile: root.join("profile"),
            python: "unused".into(),
        };
        fs::create_dir_all(&paths.here).unwrap();
        if driver {
            let manifest = paths.live().join("tools/civ6-autotest-rs/Cargo.toml");
            fs::create_dir_all(manifest.parent().unwrap()).unwrap();
            fs::write(manifest, "fixture: never executed").unwrap();
            let binary = paths
                .live()
                .join("tools/target/release")
                .join(if cfg!(windows) {
                    "civ6-tuner.exe"
                } else {
                    "civ6-tuner"
                });
            fs::create_dir_all(binary.parent().unwrap()).unwrap();
            fs::write(binary, "fixture: never executed").unwrap();
        }
        Self { root, paths }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[derive(Clone, Copy)]
enum QuitResponse {
    Success,
    Nonzero,
    Lost,
}

// A process model, rather than a reply queue: tasklist observes process lifetime,
// the quit request schedules an exit, and taskkill independently may be denied.
struct Model {
    running: [bool; 2],
    time: f64,
    exit_at: Option<f64>,
    exit_delay: Option<f64>,
    states: String,
    states_fail: bool,
    response: QuitResponse,
    lua_duration: f64,
    denied: bool,
    query_failure_at: Option<f64>,
    query_duration: f64,
    reporter_failed: bool,
    sync_code: i32,
    mark_code: i32,
    restore_code: i32,
    calls: Vec<(f64, Call)>,
    sleeps: Vec<f64>,
    printed: Vec<String>,
    require_lock: Option<PathBuf>,
}
impl Model {
    fn new(running: [bool; 2]) -> Self {
        Self {
            running,
            time: 100.0,
            exit_at: None,
            exit_delay: Some(1.0),
            states: "Civilization VI\n1: InGame\n2: FrontEnd\n".into(),
            states_fail: false,
            response: QuitResponse::Success,
            lua_duration: 0.0,
            denied: false,
            query_failure_at: None,
            query_duration: 0.0,
            reporter_failed: false,
            sync_code: 0,
            mark_code: 0,
            restore_code: 0,
            calls: vec![],
            sleeps: vec![],
            printed: vec![],
            require_lock: None,
        }
    }
    fn update(&mut self) {
        if self.exit_at.is_some_and(|at| self.time >= at) {
            self.running = [false; 2];
        }
    }
    fn count(&self, command: &str) -> usize {
        self.calls
            .iter()
            .filter(|(_, call)| call.args[0] == command)
            .count()
    }
    fn tuner_calls(&self, command: &str) -> Vec<&Call> {
        self.calls
            .iter()
            .filter_map(|(_, call)| {
                (call.args.get(1).is_some_and(|arg| arg == command)).then_some(call)
            })
            .collect()
    }
}
impl Runner for Model {
    fn run(&mut self, call: Call) -> Result<Output, String> {
        self.update();
        assert_eq!(call.creationflags, CREATE_NO_WINDOW);
        assert!(call
            .timeout
            .is_some_and(|seconds| seconds > 0.0 && seconds <= 1200.0));
        if call.args[0] != "tasklist" {
            if let Some(lock) = &self.require_lock {
                let owner: serde_json::Value =
                    serde_json::from_slice(&fs::read(lock).unwrap()).unwrap();
                assert_eq!(owner["live_chain"], true);
                assert_eq!(owner["chain_pid"], std::process::id());
                assert!(owner["lock_token"].as_str().is_some_and(|s| !s.is_empty()));
            }
        }
        self.calls.push((self.time, call.clone()));
        match call.args[0].as_str() {
            _ if call.args.get(1).is_some_and(|arg| arg == "--mark") => Ok(Output {
                code: self.mark_code,
                stdout: json!({"cloud":{},"process":{}}).to_string(),
                ..Output::default()
            }),
            _ if call.args.get(1).is_some_and(|arg| arg == "--restore") => {
                assert!(!self.running.iter().any(|running| *running));
                if self.restore_code == 0 {
                    fs::remove_file(&call.args[3]).unwrap();
                }
                Ok(Output {
                    code: self.restore_code,
                    stderr: "restore fixture".into(),
                    ..Output::default()
                })
            }
            _ if call.args.get(1).is_some_and(|arg| arg == "--cursor") => {
                assert!(!self.running.iter().any(|running| *running));
                assert!(serde_json::from_str::<serde_json::Value>(&call.args[2]).is_ok());
                assert_eq!(call.args[3], "--journal");
                assert!(call.kill_tree);
                Ok(Output {
                    code: self.sync_code,
                    stderr: "sync fixture".into(),
                    ..Output::default()
                })
            }
            "tasklist" => {
                self.time += self.query_duration.min(call.timeout.unwrap());
                self.update();
                if self.query_failure_at.is_some_and(|at| self.time >= at) {
                    return Ok(Output {
                        code: 1,
                        stderr: "enumeration denied".into(),
                        ..Output::default()
                    });
                }
                let image = call.args[2]
                    .strip_prefix("IMAGENAME eq ")
                    .expect("image query");
                let index = chain::CIV_EXECUTABLES
                    .iter()
                    .position(|name| *name == image)
                    .unwrap();
                Ok(Output {
                    stdout: if self.running[index] {
                        format!("\"{image}\",\"42\"")
                    } else {
                        "INFO: no tasks".into()
                    },
                    ..Output::default()
                })
            }
            "taskkill" => {
                assert_eq!(call.args[1], "/IM");
                assert_eq!(call.args[3], "/F");
                let index = chain::CIV_EXECUTABLES
                    .iter()
                    .position(|name| *name == call.args[2])
                    .unwrap();
                if !self.denied {
                    self.running[index] = false;
                }
                Ok(Output {
                    code: if self.denied { 1 } else { 0 },
                    stderr: "access denied".into(),
                    ..Output::default()
                })
            }
            "powershell" => {
                assert!(
                    !self.running.iter().any(|running| *running),
                    "reporters before game exit"
                );
                assert_eq!(call.args[3], chain::REPORTER_COMMAND);
                Ok(Output {
                    code: if self.reporter_failed { 1 } else { 0 },
                    stderr: "reporter denied".into(),
                    ..Output::default()
                })
            }
            _ if call.args.get(1).is_some_and(|arg| arg == "states") => Ok(Output {
                code: if self.states_fail { 1 } else { 0 },
                stdout: self.states.clone(),
                stderr: "tuner unavailable".into(),
            }),
            _ if call.args.get(1).is_some_and(|arg| arg == "lua") => {
                assert_eq!(call.args[2], "--state");
                assert_eq!(call.args[4], "--code");
                assert_eq!(call.args[5], "Events.UserConfirmedClose()\nreturn true");
                self.exit_at = self.exit_delay.map(|delay| self.time + delay);
                self.time += self.lua_duration.min(call.timeout.unwrap());
                self.update();
                match self.response {
                    QuitResponse::Success => Ok(Output::default()),
                    QuitResponse::Nonzero => Ok(Output {
                        code: 1,
                        stderr: "socket closed".into(),
                        ..Output::default()
                    }),
                    QuitResponse::Lost => Err("FireTuner response lost".into()),
                }
            }
            _ if call.args.get(1).is_some_and(|arg| arg == "live") => {
                // End before scenario preparation; this exercises the CLI's
                // ownership-protected close and subsequent lock release.
                Ok(Output {
                    code: 1,
                    stderr: "fixture preparation stop".into(),
                    ..Output::default()
                })
            }
            _ => panic!("unexpected external command: {:?}", call.args),
        }
    }
    // Wall time deliberately jumps; deadlines must use the monotonic clock.
    fn now(&mut self) -> f64 {
        1_000_000.0 - self.time * 100.0
    }
    fn monotonic(&mut self) -> f64 {
        self.time
    }
    fn sleep(&mut self, seconds: f64) -> Result<(), String> {
        assert!(seconds > 0.0 && seconds <= 0.5);
        self.sleeps.push(seconds);
        self.time += seconds;
        self.update();
        Ok(())
    }
    fn print(&mut self, text: &str) -> Result<(), String> {
        self.printed.push(text.into());
        Ok(())
    }
}

#[test]
fn absent_game_does_not_resolve_or_start_tuner_or_kill() {
    let f = Fixture::new(false);
    let mut r = Model::new([false; 2]);
    assert_eq!(
        chain::ensure_responsive_civ6(&mut r, &f.paths).unwrap(),
        "absent"
    );
    assert_eq!(r.calls.len(), 3);
    assert_eq!(r.count("powershell"), 1);
    assert!(r.sleeps.is_empty());
    let mut r = Model::new([false; 2]);
    chain::close_civ6_with_paths(&mut r, &f.paths).unwrap();
    assert_eq!(r.calls.len(), 3);
}

#[test]
fn each_renderer_quits_through_official_frontend_event_before_reporter_cleanup() {
    let f = Fixture::new(true);
    for running in [[true, false], [false, true], [true, true]] {
        let mut r = Model::new(running);
        assert_eq!(
            chain::ensure_responsive_civ6(&mut r, &f.paths).unwrap(),
            "closed"
        );
        assert_eq!(r.count("taskkill"), 0);
        assert_eq!(r.count("powershell"), 1);
        let lua = r.tuner_calls("lua");
        assert_eq!(lua.len(), 1);
        assert_eq!(lua[0].args[3], "FrontEnd");
        assert!(PathBuf::from(&lua[0].args[0]).starts_with(f.paths.live()));
        assert_eq!(r.time, 101.0);
        assert_eq!(r.sleeps, [0.5, 0.5]);
    }
}

#[test]
fn exit_sync_barrier_runs_after_process_exit_and_propagates_failure() {
    let f = Fixture::new(true);
    let source = f
        .paths
        .live()
        .join("tools/civ6-autotest-rs/src/bin/civ6-steam-sync.rs");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(source, "fixture").unwrap();
    let binary = f
        .paths
        .live()
        .join("tools/target/release/civ6-steam-sync.exe");
    fs::write(&binary, "fixture: never executed").unwrap();
    for code in [0, 1] {
        let mut r = Model::new([true, false]);
        r.sync_code = code;
        let result = chain::close_civ6_with_paths(&mut r, &f.paths);
        assert_eq!(result.is_ok(), code == 0);
        assert_eq!(r.count("taskkill"), 0);
        assert_eq!(r.count(&cc_live::path(&binary)), 2);
        if let Err(error) = result {
            assert!(error.contains("Steam exit sync did not finish"));
        }
    }
    let mut r = Model::new([false, false]);
    chain::close_civ6_with_paths(&mut r, &f.paths).unwrap();
    assert_eq!(r.count(&cc_live::path(&binary)), 1);
}

#[test]
fn missing_sync_executable_closes_game_then_refuses_unobserved_completion() {
    let f = Fixture::new(true);
    let source = f
        .paths
        .live()
        .join("tools/civ6-autotest-rs/src/bin/civ6-steam-sync.rs");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(source, "fixture").unwrap();
    let mut r = Model::new([true, false]);
    let error = chain::close_civ6_with_paths(&mut r, &f.paths).unwrap_err();
    assert!(error.contains("executable is missing"));
    assert!(!r.running.iter().any(|running| *running));
    assert_eq!(r.count("taskkill"), 0);
}

#[test]
fn autosave_restore_follows_sync_barrier_even_when_sync_failed() {
    let f = Fixture::new(true);
    let source = f
        .paths
        .live()
        .join("tools/civ6-autotest-rs/src/bin/civ6-steam-sync.rs");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(source, "fixture").unwrap();
    let sync = f
        .paths
        .live()
        .join("tools/target/release/civ6-steam-sync.exe");
    fs::write(&sync, "fixture").unwrap();
    for code in [0, 1] {
        let state = f.paths.here.join("reliability-auto-state.json");
        fs::write(&state, "fixture").unwrap();
        let mut r = Model::new([true, false]);
        r.sync_code = code;
        let result = chain::close_civ6_with_paths(&mut r, &f.paths);
        assert_eq!(result.is_ok(), code == 0);
        assert!(!state.exists());
        let s = r
            .calls
            .iter()
            .position(|(_, c)| c.args.get(1).is_some_and(|a| a == "--cursor"))
            .unwrap();
        let restore = r
            .calls
            .iter()
            .position(|(_, c)| c.args.get(1).is_some_and(|a| a == "--restore"))
            .unwrap();
        assert!(s < restore);
        let mark = r
            .calls
            .iter()
            .position(|(_, c)| c.args.get(1).is_some_and(|a| a == "--mark"))
            .unwrap();
        let quit = r
            .calls
            .iter()
            .position(|(_, c)| c.args.get(1).is_some_and(|a| a == "lua"))
            .unwrap();
        assert!(mark < quit && quit < s);
    }
    let state = f.paths.here.join("reliability-auto-state.json");
    fs::write(&state, "fixture").unwrap();
    let mut r = Model::new([true, false]);
    r.mark_code = 1;
    assert!(chain::close_civ6_with_paths(&mut r, &f.paths)
        .unwrap_err()
        .contains("cannot mark"));
    assert!(!state.exists());
    assert!(!r.running.iter().any(|alive| *alive));
    assert!(!r
        .calls
        .iter()
        .any(|(_, c)| c.args.get(1).is_some_and(|a| a == "--cursor")));
}
#[test]
fn cleanup_only_restores_receipt_without_checkout_launch_or_touching_a_running_game() {
    let f = Fixture::new(false);
    let state = f.paths.here.join("reliability-auto-state.json");
    fs::write(&state, "fixture").unwrap();
    let mut r = Model::new([false, false]);
    r.require_lock = Some(f.paths.here.join("civ6.lock"));
    assert_eq!(
        chain::cli(&mut r, &f.paths, &["--cleanup-only".into()]).unwrap(),
        0
    );
    assert!(!state.exists());
    assert!(!f.paths.here.join("civ6.lock").exists());
    assert_eq!(r.count("taskkill"), 0);
    assert!(!r
        .calls
        .iter()
        .any(|(_, c)| c.args[0] == "git" || c.args[0].ends_with("ship.exe")));
    let mut r = Model::new([true, false]);
    assert!(chain::cli(&mut r, &f.paths, &["--cleanup-only".into()]).is_err());
    assert_eq!(r.count("taskkill"), 0);
    assert!(r.tuner_calls("lua").is_empty());
    assert!(r.running[0]);
    assert!(!f.paths.here.join("civ6.lock").exists());
}

#[test]
fn ingame_context_is_used_when_frontend_is_not_unique() {
    let f = Fixture::new(true);
    for states in ["1: InGame\n", "1: FrontEnd\n2: frontend\n3: ingame\n"] {
        let mut r = Model::new([true, false]);
        r.states = states.into();
        chain::close_civ6_with_paths(&mut r, &f.paths).unwrap();
        assert_eq!(r.tuner_calls("lua")[0].args[3], "InGame");
        assert_eq!(r.count("taskkill"), 0);
    }
}

#[test]
fn lost_or_failed_quit_response_still_waits_for_process_exit_without_resending() {
    let f = Fixture::new(true);
    for response in [QuitResponse::Nonzero, QuitResponse::Lost] {
        let mut r = Model::new([true, false]);
        r.response = response;
        r.exit_delay = Some(3.0);
        chain::close_civ6_with_paths(&mut r, &f.paths).unwrap();
        assert_eq!(r.tuner_calls("lua").len(), 1);
        assert_eq!(r.count("taskkill"), 0);
        assert_eq!(r.time, 103.0);
        assert!(r.printed.iter().any(|s| s.contains("response unavailable")));
    }
}

#[test]
fn forced_shutdown_is_only_used_after_the_graceful_deadline() {
    let f = Fixture::new(true);
    for response in [
        QuitResponse::Success,
        QuitResponse::Lost,
        QuitResponse::Nonzero,
    ] {
        let mut r = Model::new([true, true]);
        r.response = response;
        r.exit_delay = None;
        assert_eq!(
            chain::ensure_responsive_civ6(&mut r, &f.paths).unwrap(),
            "killed"
        );
        assert_eq!(r.tuner_calls("lua").len(), 1);
        let kills: Vec<_> = r
            .calls
            .iter()
            .filter(|(_, call)| call.args[0] == "taskkill")
            .collect();
        assert_eq!(kills.len(), 2);
        assert_eq!(kills[0].0, 130.0);
        assert_eq!(kills[1].0, 130.0);
        assert_eq!(r.count("powershell"), 1);
    }
}

#[test]
fn a_process_exiting_exactly_at_the_deadline_is_not_force_killed() {
    let f = Fixture::new(true);
    let mut r = Model::new([false, true]);
    r.exit_delay = Some(30.0);
    chain::close_civ6_with_paths(&mut r, &f.paths).unwrap();
    assert_eq!(r.time, 130.0);
    assert_eq!(r.count("taskkill"), 0);
}

#[test]
fn time_spent_in_tuner_and_process_queries_reduces_the_exit_wait() {
    let f = Fixture::new(true);
    for (running, forced_at) in [([true, false], 136.0), ([false, true], 142.0)] {
        let mut r = Model::new(running);
        r.exit_delay = None;
        r.lua_duration = 15.0;
        r.query_duration = 3.0;
        chain::close_civ6_with_paths(&mut r, &f.paths).unwrap();
        let kill = r
            .calls
            .iter()
            .find(|(_, c)| c.args[0] == "taskkill")
            .unwrap();
        // First image query consumes 3s; graceful deadline is then 133s. Only
        // the mandatory final recheck may consume another 3s before taskkill.
        assert_eq!(kill.0, forced_at);
        let bounded_query = r
            .calls
            .iter()
            .find(|(_, c)| c.args[0] == "tasklist" && c.timeout.unwrap() < 3.0)
            .unwrap();
        assert!(bounded_query.1.timeout.unwrap() > 0.0);
    }
}

#[test]
fn unavailable_tuner_or_context_falls_back_without_sending_lua() {
    for driver in [false, true] {
        let f = Fixture::new(driver);
        for states in [
            "",
            "0: NotFrontEnd\n1: InGameHelper\n",
            "0: InGame\n1: ingame\n",
            "identity: FrontEnd",
        ] {
            let mut r = Model::new([true, false]);
            r.states = states.into();
            chain::close_civ6_with_paths(&mut r, &f.paths).unwrap();
            assert!(r.tuner_calls("lua").is_empty());
            assert_eq!(r.count("taskkill"), 2);
            assert!(r.sleeps.is_empty());
        }
    }
    let f = Fixture::new(true);
    let mut r = Model::new([true, false]);
    r.states_fail = true;
    chain::close_civ6_with_paths(&mut r, &f.paths).unwrap();
    assert!(r.tuner_calls("lua").is_empty());
    assert_eq!(r.count("taskkill"), 2);
}

#[test]
fn failed_state_query_does_not_kill_a_game_that_exited_in_the_meantime() {
    let f = Fixture::new(true);
    let mut r = Model::new([true, false]);
    r.states_fail = true;
    r.exit_at = Some(101.0);
    r.query_duration = 0.5;
    chain::close_civ6_with_paths(&mut r, &f.paths).unwrap();
    assert_eq!(r.count("taskkill"), 0);
    assert_eq!(r.count("powershell"), 1);
}

#[test]
fn denied_force_kill_fails_after_a_bounded_wait_and_keeps_reporters() {
    let f = Fixture::new(false);
    let mut r = Model::new([false, true]);
    r.denied = true;
    let error = chain::close_civ6_with_paths(&mut r, &f.paths).unwrap_err();
    assert!(error.contains("did not exit after taskkill"));
    assert_eq!(r.time, 115.0);
    assert_eq!(r.count("taskkill"), 2);
    assert_eq!(r.count("powershell"), 0);
}

#[test]
fn enumeration_failure_never_proves_absence_or_triggers_force_kill() {
    let f = Fixture::new(true);
    for failure_at in [100.0, 100.5] {
        let mut r = Model::new([true, false]);
        r.query_failure_at = Some(failure_at);
        assert!(chain::close_civ6_with_paths(&mut r, &f.paths)
            .unwrap_err()
            .contains("Could not inspect Civ6"));
        assert_eq!(r.count("taskkill"), 0);
        assert_eq!(r.count("powershell"), 0);
    }
}

#[test]
fn reporter_cleanup_failure_is_propagated_after_normal_exit() {
    let f = Fixture::new(true);
    let mut r = Model::new([true, false]);
    r.reporter_failed = true;
    assert!(chain::close_civ6_with_paths(&mut r, &f.paths)
        .unwrap_err()
        .contains("reporter denied"));
    assert_eq!(r.count("taskkill"), 0);
    assert_eq!(r.count("powershell"), 1);
}

#[test]
fn cli_keeps_shared_lock_through_normal_close_and_releases_after_prepare_failure() {
    let f = Fixture::new(true);
    let mut r = Model::new([true, false]);
    let lock = f.paths.here.join("civ6.lock");
    r.require_lock = Some(lock.clone());
    assert_eq!(
        chain::cli(
            &mut r,
            &f.paths,
            &["fixture:scenario.json".into(), "--stop-on-failure".into()]
        )
        .unwrap(),
        1
    );
    assert_eq!(r.tuner_calls("lua").len(), 1);
    assert_eq!(r.count("taskkill"), 0);
    assert!(!lock.exists());
    assert!(r.printed.iter().any(|s| s.contains("prepare-failed")));
    // The CLI result is only about the model; it provides no live-game evidence.
    assert_eq!(json!(r.running), json!([false, false]));
}
