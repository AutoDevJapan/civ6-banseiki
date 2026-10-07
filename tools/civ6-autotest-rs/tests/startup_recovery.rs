mod support_driver;
use civ6_autotest_rs::startup_idle::{
    self, AdapterCoverage, EngineReading, IdleProof, IdleSample, IDLE_SOURCE,
};
use civ6_autotest_rs::{
    runner::{Backend, Runner},
    startup::{self, GameProcess, GpuClient, GpuSnapshot},
    Error, ErrorKind, Result,
};
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
};
use support_driver::ManualClock;
fn clear_idle() -> IdleProof {
    IdleProof {
        software_adapters: Vec::new(),
        source: IDLE_SOURCE.into(),
        status: "ok".into(),
        adapters: vec![AdapterCoverage {
            id: "00000000:00000001".into(),
            nodes: 1,
            wddm_version: 3100,
        }],
        adapters_stable: true,
        samples: (0..3)
            .map(|_| IdleSample {
                interval_ms: 1000,
                engines: vec![EngineReading {
                    pid: 94500,
                    adapter: "00000000:00000001".into(),
                    physical: 0,
                    engine: 0,
                    kind: "3D".into(),
                    percent: 0.0,
                    valid: true,
                }],
            })
            .collect(),
        finished_unix: 100.0,
        detail: "fixture native PDH proof".into(),
    }
}

#[test]
fn stale_or_missing_helper_identity_cannot_pass_the_driver_protocol() {
    let good = json!({"protocol":1,"source_sha256":startup::helper_identity()});
    startup::verify_helper_identity(&good).unwrap();
    for bad in [
        json!({}),
        json!({"protocol":1,"source_sha256":"leftover-helper"}),
        json!({"protocol":2,"source_sha256":startup::helper_identity()}),
    ] {
        assert!(startup::verify_helper_identity(&bad)
            .unwrap_err()
            .to_string()
            .contains("does not match"));
    }
}

#[test]
fn a_replaced_window_in_the_same_process_cannot_supply_loading_evidence() {
    let evidence = json!({"hwnd":101,"status":"recognized"});
    startup::verify_capture_window(101, 101, &evidence).unwrap();
    for (expected, current, returned) in [
        (101, 102, evidence.clone()),
        (101, 101, json!({"hwnd":102})),
        (101, 101, json!({"hwnd":"101"})),
        (101, 101, json!({})),
        (0, 0, json!({"hwnd":0})),
    ] {
        assert!(startup::verify_capture_window(expected, current, &returned).is_err());
    }
}

#[derive(Default)]
struct State {
    events: Vec<String>,
    alive: Option<(u32, f64)>,
    launches: usize,
    lock_held: bool,
}
#[derive(Clone, Copy)]
enum LockLoss {
    BeforeShutdown,
    AfterShutdown,
}
struct Game {
    state: Arc<Mutex<State>>,
    clock: Arc<ManualClock>,
    logs: std::path::PathBuf,
    ready_on_retry: bool,
    delay_on_retry: f64,
    launched_at: f64,
    hidden_menu: bool,
    interrupted: bool,
    cancel_after_diagnostics: bool,
    invalid_config: bool,
    wrong_executable: bool,
    launch_failure: bool,
    no_window: bool,
    changed_after_diagnostics: bool,
    change_start_only: bool,
    lost_lock_at: Option<LockLoss>,
    deny_lock: bool,
    resist_shutdown: bool,
    reset_clears_pending: bool,
    reset_exit_delay: f64,
    pending_exit_at: Option<f64>,
    gpu: GpuSnapshot,
    idle: IdleProof,
}
fn clear_gpu() -> GpuSnapshot {
    GpuSnapshot {
        status: "ok".into(),
        all_clients_known: true,
        allocated_mib: Some(0),
        utilization_percent: Some(0),
        ..Default::default()
    }
}
impl Backend for Game {
    fn startup_cancel_check(&mut self) -> Result<()> {
        if self.interrupted {
            Err(Error::interrupted("fixture cancellation"))
        } else {
            Ok(())
        }
    }
    fn single_process(&mut self, _: f64) -> Result<Option<(u32, f64)>> {
        Ok(self.state.lock().unwrap().alive)
    }
    fn require_single_process(&mut self) -> Result<()> {
        Ok(())
    }
    fn launch(&mut self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.events.push("launch".into());
        if self.launch_failure {
            return Err(Error::input("fixture Steam launch failed"));
        }
        state.launches += 1;
        let process = (10 + state.launches as u32, 100.0 + state.launches as f64);
        state.alive = Some(process);
        self.launched_at = self.clock.at();
        for name in startup::STARTUP_LOGS {
            fs::write(
                self.logs.join(name),
                format!("attempt {} pid {}\n", state.launches, process.0),
            )
            .unwrap();
        }
        Ok(())
    }
    fn wait_window(&mut self, end: f64) -> Result<isize> {
        if self.no_window {
            self.clock.advance((end - self.clock.at()).max(0.0));
            return Err(Error::input("fixture window absent"));
        }
        Ok(1)
    }
    fn require_dx11(&mut self, _: isize) -> Result<()> {
        if self.wrong_executable {
            Err(Error::input("fixture wrong executable"))
        } else {
            Ok(())
        }
    }
    fn list_states(&mut self, _: f64) -> Result<Vec<String>> {
        Ok(vec!["FrontEnd".into(), "MainMenu".into()])
    }
    fn run_lua(&mut self, state: &str, _: &str, _: f64) -> Result<String> {
        self.startup_cancel_check()?;
        if self.hidden_menu {
            return Ok((state == "FrontEnd").to_string());
        }
        let count = self.state.lock().unwrap().launches;
        Ok((self.ready_on_retry
            && count == 2
            && self.clock.at() >= self.launched_at + self.delay_on_retry)
            .to_string())
    }
    fn click(&mut self, _: f64, _: f64) -> Result<(i32, i32)> {
        Ok((0, 0))
    }
    fn startup_loading_evidence(&mut self, game: GameProcess, _: &Path, _: u16) -> Result<Value> {
        self.state
            .lock()
            .unwrap()
            .events
            .push(format!("evidence:{}", game.pid));
        if self.invalid_config {
            Err(Error::input("fixture FireTuner disabled"))
        } else {
            Ok(json!({"new_process":true,"tuner_enabled":true}))
        }
    }
    fn startup_diagnostics(&mut self, game: GameProcess, output: &Path) -> Result<Value> {
        assert_eq!(
            fs::read_to_string(output.join("Renderer.log")).unwrap(),
            format!(
                "attempt {} pid {}\n",
                self.state.lock().unwrap().launches,
                game.pid
            )
        );
        let mut state = self.state.lock().unwrap();
        state.events.push(format!("diagnostics:{}", game.pid));
        if self.changed_after_diagnostics {
            state.alive = Some((
                if self.change_start_only { game.pid } else { 99 },
                game.started + 1.0,
            ));
        }
        if self.cancel_after_diagnostics {
            self.interrupted = true;
        }
        Ok(json!({"gpu":self.gpu,"collected_before_shutdown":true}))
    }
    fn acquire_startup_lock(&mut self) -> Result<()> {
        self.state
            .lock()
            .unwrap()
            .events
            .push("lock:acquire".into());
        if self.deny_lock {
            Err(Error::input("fixture missing shared lock"))
        } else {
            self.state.lock().unwrap().lock_held = true;
            Ok(())
        }
    }
    fn check_startup_lock(&mut self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.events.push("lock:check".into());
        let lost = match self.lost_lock_at {
            Some(LockLoss::BeforeShutdown) => {
                state.events.iter().any(|e| e.starts_with("diagnostics:"))
            }
            Some(LockLoss::AfterShutdown) => {
                state.events.iter().any(|e| e.starts_with("terminate:"))
            }
            None => false,
        };
        if !state.lock_held || lost {
            Err(Error::input("fixture lock lost"))
        } else {
            Ok(())
        }
    }
    fn release_startup_lock(&mut self) {
        self.state.lock().unwrap().lock_held = false;
        self.state
            .lock()
            .unwrap()
            .events
            .push("lock:release".into());
    }
    fn terminate(&mut self, pid: u32, start: f64) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        assert_eq!(state.alive, Some((pid, start)));
        state.events.push(format!("terminate:{pid}:{start}"));
        if !self.resist_shutdown {
            state.alive = None;
        }
        Ok(())
    }
    fn wait_exit(&mut self, _: u32, end: f64) -> Result<bool> {
        if self.pending_exit_at.is_some_and(|at| at <= end) {
            let at = self.pending_exit_at.take().unwrap();
            self.clock.advance((at - self.clock.at()).max(0.0));
            self.state.lock().unwrap().alive = None;
            return Ok(true);
        }
        if self.resist_shutdown && self.state.lock().unwrap().alive.is_some() {
            self.clock.advance((end - self.clock.at()).max(0.0));
            return Ok(false);
        }
        Ok(true)
    }
    fn startup_display_recovery(&mut self, game: GameProcess) -> Result<Value> {
        let mut state = self.state.lock().unwrap();
        if let Some(reason) = startup_idle::idle_refusal(&self.idle) {
            state.events.push("reset:skipped".into());
            Ok(json!({"status":"skipped","reason":reason,"gpu":self.gpu,"pdh":self.idle}))
        } else {
            state.events.push("reset:sent".into());
            if self.reset_clears_pending {
                assert_eq!(state.alive, Some((game.pid, game.started)));
                if self.reset_exit_delay > 0.0 {
                    self.pending_exit_at = Some(self.clock.at() + self.reset_exit_delay);
                } else {
                    state.alive = None;
                }
            }
            Ok(json!({"status":"sent","method":"Win+Ctrl+Shift+B"}))
        }
    }
}
struct Fixture {
    root: tempfile::TempDir,
    runner: Runner,
    state: Arc<Mutex<State>>,
    clock: Arc<ManualClock>,
}
impl Fixture {
    fn new(configure: impl FnOnce(&mut Game)) -> Self {
        let root = tempfile::tempdir().unwrap();
        let logs = root.path().join("logs");
        let output = root.path().join("output");
        fs::create_dir(&logs).unwrap();
        fs::create_dir(&output).unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let clock = Arc::new(ManualClock::default());
        let mut game = Game {
            state: state.clone(),
            clock: clock.clone(),
            logs: logs.clone(),
            ready_on_retry: true,
            delay_on_retry: 0.0,
            launched_at: 0.0,
            hidden_menu: false,
            interrupted: false,
            cancel_after_diagnostics: false,
            invalid_config: false,
            wrong_executable: false,
            launch_failure: false,
            no_window: false,
            changed_after_diagnostics: false,
            change_start_only: false,
            lost_lock_at: None,
            deny_lock: false,
            resist_shutdown: false,
            reset_clears_pending: false,
            reset_exit_delay: 0.0,
            pending_exit_at: None,
            gpu: clear_gpu(),
            idle: clear_idle(),
        };
        configure(&mut game);
        let mut runner = Runner::with_backend(
            json!({}),
            output,
            logs,
            root.path().join("saves"),
            4318,
            Box::new(game),
            clock.clone(),
        );
        runner.owner_start = true;
        Self {
            root,
            runner,
            state,
            clock,
        }
    }
    fn events(&self) -> Vec<String> {
        self.state.lock().unwrap().events.clone()
    }
    fn report(&self) -> Value {
        let directory = fs::read_dir(&self.runner.output)
            .unwrap()
            .filter_map(|e| e.ok())
            .find(|e| e.file_type().unwrap().is_dir())
            .unwrap();
        serde_json::from_slice(&fs::read(directory.path().join("recovery.json")).unwrap()).unwrap()
    }
}

#[test]
fn slow_display_recovery_finishes_pinned_shutdown_before_one_retry() {
    let mut fixture = Fixture::new(|game| {
        game.resist_shutdown = true;
        game.reset_clears_pending = true;
        game.reset_exit_delay = 12.0;
    });
    fixture.runner.restart_game(20.0).unwrap();
    let events = fixture.events();
    assert_eq!(count(&events, "launch"), 2);
    assert_eq!(count(&events, "reset:sent"), 1);
    assert_eq!(fixture.report()["retry"]["status"], "succeeded");
}
fn count(events: &[String], value: &str) -> usize {
    events.iter().filter(|s| s.as_str() == value).count()
}
fn position(events: &[String], value: &str) -> usize {
    events.iter().position(|s| s == value).unwrap()
}

#[test]
fn loading_timeout_diagnoses_before_shutdown_and_retries_once_with_new_budget() {
    let mut fixture = Fixture::new(|game| game.delay_on_retry = 19.0);
    assert!(fixture
        .runner
        .restart_game(20.0)
        .unwrap()
        .contains("retried once"));
    let events = fixture.events();
    assert_eq!(count(&events, "launch"), 2);
    assert_eq!(count(&events, "reset:sent"), 1);
    assert!(position(&events, "diagnostics:11") < position(&events, "terminate:11:101"));
    assert!(position(&events, "lock:acquire") < position(&events, "terminate:11:101"));
    assert_eq!(events.last().unwrap(), "lock:release");
    assert_eq!(fixture.clock.at(), 39.0);
    let report = fixture.report();
    assert_eq!(report["result"], "recovered");
    assert_eq!(report["retry"]["attempt"], 2);
    assert_eq!(report["process"], json!({"pid":11,"started":101.0}));
    assert_eq!(report["marker"], "STARTUP_RECOVERY_ATTEMPTED");
    assert_eq!(report["attempt1"]["collected_before_shutdown"], true);
    let output = fs::read_dir(&fixture.runner.output)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        fs::read_to_string(output.join("attempt1/Engine.log")).unwrap(),
        "attempt 1 pid 11\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.runner.logs.join("Engine.log")).unwrap(),
        "attempt 2 pid 12\n"
    );
}

#[test]
fn second_timeout_is_diagnosed_and_closed_without_recursive_recovery() {
    let mut fixture = Fixture::new(|game| game.ready_on_retry = false);
    let error = fixture.runner.restart_game(20.0).unwrap_err();
    assert!(error.message.contains("STARTUP_RECOVERY_ATTEMPTED"));
    let events = fixture.events();
    assert_eq!(count(&events, "launch"), 2);
    assert_eq!(count(&events, "reset:sent"), 1);
    assert!(position(&events, "diagnostics:12") < position(&events, "terminate:12:102"));
    assert_eq!(fixture.report()["retry"]["status"], "failed");
    assert_eq!(fixture.state.lock().unwrap().alive, None);
    assert_eq!(fixture.clock.at(), 40.0);
}

#[test]
fn foreign_compute_allocation_and_unknown_probe_refuse_reset_but_allow_one_locked_retry() {
    for gpu in [
        GpuSnapshot {
            clients: vec![
                GpuClient {
                    pid: 900,
                    kind: "C".into(),
                    allocated_mib: Some(23 * 1024),
                },
                GpuClient {
                    pid: 901,
                    kind: "C".into(),
                    allocated_mib: Some(5 * 1024),
                },
            ],
            allocated_mib: Some(28 * 1024),
            ..clear_gpu()
        },
        GpuSnapshot {
            status: "timeout".into(),
            detail: "nvidia-smi did not answer".into(),
            ..clear_gpu()
        },
        GpuSnapshot {
            all_clients_known: false,
            ..clear_gpu()
        },
    ] {
        let mut fixture = Fixture::new(|game| {
            game.gpu = gpu;
            game.idle.samples[1].engines[0].percent = 15.0;
        });
        fixture.runner.restart_game(20.0).unwrap();
        let events = fixture.events();
        assert_eq!(count(&events, "launch"), 2);
        assert_eq!(count(&events, "reset:sent"), 0);
        assert_eq!(count(&events, "reset:skipped"), 1);
        assert_eq!(fixture.report()["reset"]["status"], "skipped");
    }
}

#[test]
fn missing_or_lost_lock_prevents_shutdown_reset_and_retry_and_releases_guard() {
    for deny_lock in [true, false] {
        let mut fixture = Fixture::new(|game| {
            game.deny_lock = deny_lock;
            game.lost_lock_at = Some(LockLoss::BeforeShutdown);
        });
        fixture.runner.restart_game(20.0).unwrap_err();
        let events = fixture.events();
        assert_eq!(count(&events, "launch"), if deny_lock { 0 } else { 1 });
        assert!(!events
            .iter()
            .any(|e| e.starts_with("terminate:") || e.starts_with("reset:")));
        assert_eq!(events.last().unwrap(), "lock:release");
        if !deny_lock {
            assert_eq!(fixture.report()["retry"]["status"], "not_started");
        }
    }
}

#[test]
fn lock_loss_after_shutdown_prevents_reset_and_retry() {
    let mut fixture = Fixture::new(|game| game.lost_lock_at = Some(LockLoss::AfterShutdown));
    fixture.runner.restart_game(20.0).unwrap_err();
    let events = fixture.events();
    assert_eq!(count(&events, "terminate:11:101"), 1);
    assert!(!events.iter().any(|e| e.starts_with("reset:")));
    assert_eq!(count(&events, "launch"), 1);
    assert_eq!(events.last().unwrap(), "lock:release");
}

#[test]
fn changed_pid_or_creation_time_is_never_terminated() {
    for start_only in [true, false] {
        let mut fixture = Fixture::new(|game| {
            game.changed_after_diagnostics = true;
            game.change_start_only = start_only;
        });
        fixture.runner.restart_game(20.0).unwrap_err();
        let events = fixture.events();
        assert!(!events
            .iter()
            .any(|e| e.starts_with("terminate:") || e.starts_with("reset:")));
        assert_eq!(count(&events, "launch"), 1);
    }
}

#[test]
fn initial_cancellation_and_launch_or_window_or_executable_failure_do_not_recover() {
    for mode in 0..4 {
        let mut fixture = Fixture::new(|game| match mode {
            0 => game.interrupted = true,
            1 => game.launch_failure = true,
            2 => game.no_window = true,
            _ => game.wrong_executable = true,
        });
        let error = fixture.runner.restart_game(20.0).unwrap_err();
        if mode == 0 {
            assert_eq!(error.kind, ErrorKind::Interrupted);
        }
        let events = fixture.events();
        assert!(!events
            .iter()
            .any(|e| e.starts_with("diagnostics:") || e.starts_with("reset:")));
        if mode == 0 {
            assert!(!events.iter().any(|e| e.starts_with("lock:")));
        } else {
            assert_eq!(count(&events, "lock:acquire"), 1);
            assert_eq!(events.last().unwrap(), "lock:release");
        }
        assert!(fs::read_dir(&fixture.runner.output)
            .unwrap()
            .next()
            .is_none());
    }
}

#[test]
fn cancellation_after_diagnostics_does_not_reset_or_retry() {
    let mut fixture = Fixture::new(|game| game.cancel_after_diagnostics = true);
    assert_eq!(
        fixture.runner.restart_game(20.0).unwrap_err().kind,
        ErrorKind::Interrupted
    );
    let events = fixture.events();
    assert_eq!(count(&events, "diagnostics:11"), 1);
    assert_eq!(count(&events, "launch"), 1);
    assert!(!events
        .iter()
        .any(|e| e.starts_with("terminate:") || e.starts_with("reset:")));
}

#[test]
fn menu_already_reached_and_invalid_tuner_configuration_do_not_recover() {
    for invalid_config in [true, false] {
        let mut fixture = Fixture::new(|game| {
            game.invalid_config = invalid_config;
            game.hidden_menu = !invalid_config;
        });
        fixture.runner.restart_game(20.0).unwrap_err();
        let events = fixture.events();
        assert_eq!(count(&events, "launch"), 1);
        assert!(!events
            .iter()
            .any(|e| e.starts_with("reset:") || e.starts_with("terminate:")));
        if invalid_config {
            assert_eq!(fixture.report()["result"], "recovery_refused");
        }
    }
}

#[test]
fn invalid_timeout_and_failure_to_exit_do_not_launch_again() {
    for timeout in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        let mut fixture = Fixture::new(|_| {});
        assert_eq!(
            fixture.runner.restart_game(timeout).unwrap_err().kind,
            ErrorKind::Input
        );
        assert!(fixture.events().is_empty());
    }
    let mut fixture = Fixture::new(|game| game.resist_shutdown = true);
    fixture.runner.restart_game(20.0).unwrap_err();
    assert_eq!(count(&fixture.events(), "launch"), 1);
    assert_eq!(count(&fixture.events(), "reset:sent"), 1);
}

#[test]
fn pending_gpu_shutdown_is_cleared_by_reset_before_the_single_retry() {
    let mut fixture = Fixture::new(|game| {
        game.resist_shutdown = true;
        game.reset_clears_pending = true;
    });
    fixture.runner.restart_game(20.0).unwrap();
    let events = fixture.events();
    assert!(position(&events, "diagnostics:11") < position(&events, "terminate:11:101"));
    assert!(position(&events, "terminate:11:101") < position(&events, "reset:sent"));
    assert_eq!(count(&events, "launch"), 2);
    assert_eq!(count(&events, "reset:sent"), 1);
    assert_eq!(fixture.report()["shutdown"]["status"], "pending");
    assert_eq!(fixture.report()["result"], "recovered");
}

#[test]
fn pending_shutdown_with_busy_or_unknown_pdh_never_relaunches() {
    for unknown in [false, true] {
        let mut fixture = Fixture::new(|game| {
            game.resist_shutdown = true;
            if unknown {
                game.idle.status = "unknown".into();
            } else {
                game.idle.samples[2].engines[0].percent = 12.0;
            }
        });
        fixture.runner.restart_game(20.0).unwrap_err();
        let events = fixture.events();
        assert_eq!(count(&events, "launch"), 1);
        assert_eq!(count(&events, "reset:sent"), 0);
        assert_eq!(fixture.report()["reset"]["status"], "skipped");
        assert_eq!(fixture.report()["retry"]["status"], "not_started");
    }
}

#[test]
fn smi_timeout_with_independent_valid_pdh_proof_can_reset_and_retry() {
    let mut fixture = Fixture::new(|game| {
        game.gpu.status = "timeout".into();
        game.resist_shutdown = true;
        game.reset_clears_pending = true;
    });
    fixture.runner.restart_game(20.0).unwrap();
    assert_eq!(count(&fixture.events(), "reset:sent"), 1);
    assert_eq!(count(&fixture.events(), "launch"), 2);
}

#[test]
fn reset_policy_requires_complete_accounting_even_at_zero_utilization() {
    let game = GameProcess {
        pid: 11,
        started: 101.0,
    };
    assert_eq!(startup::reset_refusal(&clear_gpu(), game), None);
    for gpu in [
        GpuSnapshot {
            allocated_mib: Some(1),
            ..clear_gpu()
        },
        GpuSnapshot {
            utilization_percent: None,
            ..clear_gpu()
        },
        GpuSnapshot {
            all_clients_known: false,
            ..clear_gpu()
        },
        GpuSnapshot {
            clients: vec![GpuClient {
                pid: 11,
                kind: "C".into(),
                allocated_mib: Some(0),
            }],
            ..clear_gpu()
        },
        GpuSnapshot {
            clients: vec![GpuClient {
                pid: 11,
                kind: "G".into(),
                allocated_mib: None,
            }],
            ..clear_gpu()
        },
    ] {
        assert!(startup::reset_refusal(&gpu, game).is_some());
    }
}

#[test]
fn nvidia_xml_retains_foreign_allocations_without_assuming_wsl_coverage() {
    let xml = r#"<?xml version="1.0"?><!DOCTYPE nvidia_smi_log SYSTEM "nvsmi_device_v12.dtd">
    <nvidia_smi_log><gpu><fb_memory_usage><used>28672 MiB</used></fb_memory_usage><utilization><gpu_util>0 %</gpu_util></utilization>
    <processes><process_info><pid>900</pid><type>C</type><used_memory>23552 MiB</used_memory></process_info>
    <process_info><pid>901</pid><type>C+G</type><used_memory>N/A</used_memory></process_info></processes></gpu></nvidia_smi_log>"#;
    let gpu = startup::parse_nvidia_xml(xml).unwrap();
    assert_eq!(
        gpu.clients,
        vec![
            GpuClient {
                pid: 900,
                kind: "C".into(),
                allocated_mib: Some(23552)
            },
            GpuClient {
                pid: 901,
                kind: "C+G".into(),
                allocated_mib: None
            }
        ]
    );
    assert_eq!(gpu.allocated_mib, Some(28672));
    assert_eq!(gpu.utilization_percent, Some(0));
    assert!(!gpu.all_clients_known);
    assert!(startup::parse_nvidia_xml("<nvidia_smi_log/>").is_err());
    assert!(startup::parse_nvidia_xml("N/A").is_err());
}

#[test]
fn tuner_configuration_and_lock_record_are_fail_closed() {
    for options in [
        "EnableTuner 0",
        "EnableTuner 1\nEnableTuner 0",
        "EnableTuner 1 # unknown",
        "",
        "EnableTuner 1\nEnableTuner 1",
    ] {
        assert!(!startup::tuner_enabled(options));
    }
    assert!(startup::tuner_enabled(
        "[Debug]\n; enable tuner\nEnableTuner 1\n"
    ));
    let lock = json!({"live_chain":true,"chain_pid":17,"pid":17,"job":"not logged"});
    startup::verify_lock_record(&lock, 17).unwrap();
    for bad in [
        json!({}),
        json!({"live_chain":true,"chain_pid":17,"pid":18}),
        json!({"live_chain":false,"chain_pid":17,"pid":17}),
    ] {
        assert!(startup::verify_lock_record(&bad, 17).is_err());
    }
    assert!(startup::verify_lock_record(&lock, 18).is_err());
}

#[test]
fn startup_log_tail_is_bounded_and_stale_logs_are_identified() {
    let fixture = Fixture::new(|_| {});
    let mut bytes = vec![b'a'; startup::LOG_LIMIT as usize + 100];
    bytes.extend_from_slice(b"diagnostic end\n");
    fs::write(fixture.runner.logs.join("Renderer.log"), &bytes).unwrap();
    let rows = startup::snapshot_logs(
        &fixture.runner.logs,
        fixture.root.path(),
        GameProcess {
            pid: 11,
            started: f64::MAX,
        },
    );
    let renderer = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "Renderer.log")
        .unwrap();
    assert_eq!(renderer["bytes"], startup::LOG_LIMIT);
    assert_eq!(renderer["current_process"], false);
    let saved = fs::read(fixture.root.path().join("Renderer.log")).unwrap();
    assert_eq!(saved.len(), startup::LOG_LIMIT as usize);
    assert!(saved.ends_with(b"diagnostic end\n"));
    assert_eq!(rows.as_array().unwrap().len(), startup::STARTUP_LOGS.len());
}
