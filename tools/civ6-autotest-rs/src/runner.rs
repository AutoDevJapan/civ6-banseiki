use crate::tuner::state_name_eq;
use crate::{
    checkpoint, judge, metrics, scenario,
    time::{Clock, SystemClock},
    Error, ErrorKind, Result,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

pub trait Backend: Send {
    fn live_lease_budget(&self) -> Result<Option<f64>> {
        Ok(None)
    }
    fn startup_cancel_check(&mut self) -> Result<()> {
        crate::time::check_interrupt()
    }
    fn pin_startup_process(&mut self, _hwnd: isize) -> Result<Option<crate::startup::GameProcess>> {
        Ok(self.single_process(0.0)?.map(Into::into))
    }
    fn startup_loading_evidence(
        &mut self,
        _game: crate::startup::GameProcess,
        _logs: &Path,
        _port: u16,
    ) -> Result<Value> {
        Err(Error::input(
            "backend cannot prove a new-process loading timeout",
        ))
    }
    fn startup_diagnostics(
        &mut self,
        _game: crate::startup::GameProcess,
        _output: &Path,
    ) -> Result<Value> {
        Ok(json!({"status":"unavailable","reason":"backend has no native startup diagnostics"}))
    }
    fn acquire_startup_lock(&mut self) -> Result<()> {
        Err(Error::input(
            "backend cannot prove shared startup lock ownership",
        ))
    }
    fn check_startup_lock(&mut self) -> Result<()> {
        Err(Error::input(
            "backend cannot prove shared startup lock ownership",
        ))
    }
    fn release_startup_lock(&mut self) {}
    fn check_startup_process(&mut self, game: crate::startup::GameProcess) -> Result<()> {
        if self
            .single_process(0.0)?
            .is_some_and(|p| p != (game.pid, game.started))
        {
            return Err(Error::input(
                "pinned startup process changed or another Civ6 appeared",
            ));
        }
        Ok(())
    }
    fn startup_display_recovery(&mut self, _game: crate::startup::GameProcess) -> Result<Value> {
        Ok(json!({"status":"skipped","reason":"backend cannot prove display recovery safe"}))
    }
    fn startup_finish_diagnostics(&mut self) -> Result<Value> {
        Ok(json!({"status":"all_exited"}))
    }
    fn connect(&mut self, _port: u16, _timeout: f64) -> Result<()> {
        Ok(())
    }
    fn disconnect(&mut self) {}
    fn list_states(&mut self, _timeout: f64) -> Result<Vec<String>> {
        Err(Error::tuner("backend has no FireTuner"))
    }
    fn run_lua(&mut self, _state: &str, _code: &str, _timeout: f64) -> Result<String> {
        Err(Error::tuner("backend has no FireTuner"))
    }
    fn require_single_process(&mut self) -> Result<()> {
        Err(Error::input("backend has no Civ VI process"))
    }
    fn single_process(&mut self, _settle: f64) -> Result<Option<(u32, f64)>> {
        Err(Error::input("backend has no Civ VI process"))
    }
    fn start_native_trace(
        &mut self,
        _output: &Path,
        _name: &str,
        _seconds: u64,
        _mode: &str,
    ) -> Result<()> {
        Err(Error::input(
            "backend cannot start native governor observation",
        ))
    }
    fn stop_native_trace(&mut self) -> Result<Option<PathBuf>> {
        Ok(None)
    }
    fn start_native_bridge(
        &mut self,
        _output: &Path,
        _name: &str,
        _seconds: u64,
        _mode: &str,
    ) -> Result<()> {
        Err(Error::input(
            "backend cannot start native CanEver bridge telemetry",
        ))
    }
    fn stop_native_bridge(&mut self) -> Result<Option<PathBuf>> {
        Ok(None)
    }
    fn memory(&mut self, _pid: u32) -> Result<Value> {
        Err(Error::input("backend has no process memory"))
    }
    fn launch(&mut self) -> Result<()> {
        Err(Error::input("backend cannot launch Steam"))
    }
    fn close_orphaned_reporters(&mut self) -> Result<Vec<u32>> {
        Ok(Vec::new())
    }
    fn wait_window(&mut self, _deadline: f64) -> Result<isize> {
        Err(Error::input("backend has no Civ VI window"))
    }
    fn require_dx11(&mut self, _hwnd: isize) -> Result<()> {
        Err(Error::input("backend has no DX11 window"))
    }
    fn click(&mut self, _fx: f64, _fy: f64) -> Result<(i32, i32)> {
        Err(Error::input("backend cannot click"))
    }
    fn press_key(&mut self, _key: u16, _deadline: f64) -> Result<()> {
        Err(Error::input("backend cannot send keys"))
    }
    fn request_close(&mut self) -> Result<()> {
        Err(Error::input("backend cannot close window"))
    }
    fn wait_exit(&mut self, _pid: u32, _deadline: f64) -> Result<bool> {
        Err(Error::input("backend cannot observe process exit"))
    }
    fn terminate(&mut self, _pid: u32, _start: f64) -> Result<()> {
        Err(Error::input("backend cannot terminate process"))
    }
    fn capture(&mut self, _path: &Path, _crop: Option<[f64; 4]>, _deadline: f64) -> Result<()> {
        Err(Error::capture("backend cannot capture window"))
    }
    fn mod_folders(&mut self, scenario: &Value, mods: &Path) -> Result<Vec<PathBuf>> {
        crate::restart::enabled_mod_folders(scenario, mods)
    }
    fn newest_mod(&mut self, folders: &[PathBuf]) -> Result<Option<(PathBuf, f64)>> {
        crate::restart::newest_mod_file(folders)
    }
    fn stale_art(&mut self, folders: &[PathBuf]) -> Result<Vec<String>> {
        crate::restart::missing_atlas_textures(folders)
    }
    fn manual(&mut self, prompt: &str) -> Result<()> {
        println!("MANUAL / Computer Use (no timeout; waits until Enter): {prompt}");
        print!("Press Enter after the action is complete: ");
        io::stdout().flush()?;
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut input = String::new();
            let _ = send.send(io::stdin().read_line(&mut input));
        });
        let count = loop {
            crate::time::check_interrupt()?;
            match receive.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(result) => break result?,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(e) => return Err(Error::scenario(e.to_string())),
            }
        };
        if count == 0 {
            return Err(Error::scenario(
                "End of input while waiting for manual action",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod native_trace_step_tests {
    use super::*;
    use std::sync::Mutex;

    struct TraceBackend(Arc<Mutex<Vec<String>>>);
    impl Backend for TraceBackend {
        fn start_native_trace(
            &mut self,
            _: &Path,
            name: &str,
            seconds: u64,
            mode: &str,
        ) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .push(format!("start:{name}:{seconds}:{mode}"));
            Ok(())
        }
        fn stop_native_trace(&mut self) -> Result<Option<PathBuf>> {
            self.0.lock().unwrap().push("stop".into());
            Ok(Some(PathBuf::from("fixture.jsonl")))
        }
    }

    #[test]
    fn runner_dispatches_start_and_stop_with_an_artifact() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let output = std::env::temp_dir().join("pr86-native-trace-dispatch");
        let mut runner = Runner::with_backend(
            json!({"name":"fixture","steps":[]}),
            output.clone(),
            output.clone(),
            output,
            4318,
            Box::new(TraceBackend(Arc::clone(&calls))),
            Arc::new(SystemClock),
        );
        let detail = runner
            .run_step(
                &json!({"action":"start_native_governor_trace","name":"governor","max_seconds":25}),
            )
            .unwrap();
        assert!(detail.contains("governor.jsonl"));
        let detail = runner
            .run_step(&json!({"action":"stop_native_governor_trace"}))
            .unwrap();
        assert!(detail.contains("fixture.jsonl"));
        assert_eq!(
            *calls.lock().unwrap(),
            ["start:governor:25:governor", "stop"]
        );
    }

    struct BridgeBackend(Arc<Mutex<Vec<String>>>);
    impl Backend for BridgeBackend {
        fn start_native_bridge(
            &mut self,
            _: &Path,
            name: &str,
            seconds: u64,
            mode: &str,
        ) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .push(format!("start:{name}:{seconds}:{mode}"));
            Ok(())
        }
        fn stop_native_bridge(&mut self) -> Result<Option<PathBuf>> {
            self.0.lock().unwrap().push("stop".into());
            Ok(Some(PathBuf::from("bridge.jsonl")))
        }
    }

    #[test]
    fn runner_dispatches_separate_can_ever_bridge_with_an_artifact() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let output = std::env::temp_dir().join("pr86-native-bridge-dispatch");
        let mut runner = Runner::with_backend(
            json!({"name":"fixture","steps":[]}),
            output.clone(),
            output.clone(),
            output,
            4318,
            Box::new(BridgeBackend(Arc::clone(&calls))),
            Arc::new(SystemClock),
        );
        let detail = runner
            .run_step(
                &json!({"action":"start_native_can_ever_trace","name":"can-ever","max_seconds":25}),
            )
            .unwrap();
        assert!(detail.contains("can-ever.jsonl"));
        let detail = runner
            .run_step(&json!({"action":"stop_native_can_ever_trace"}))
            .unwrap();
        assert!(detail.contains("bridge.jsonl"));
        assert_eq!(
            *calls.lock().unwrap(),
            ["start:can-ever:25:can-ever", "stop"]
        );
    }

    #[test]
    fn runner_dispatches_property_mode_to_the_bounded_bridge() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let output = std::env::temp_dir().join("pr86-native-property-dispatch");
        let mut runner = Runner::with_backend(
            json!({"name":"fixture","steps":[]}),
            output.clone(),
            output.clone(),
            output,
            4318,
            Box::new(BridgeBackend(Arc::clone(&calls))),
            Arc::new(SystemClock),
        );
        assert!(runner
            .run_step(&json!({"action":"start_native_property_trace",
            "name":"values","max_seconds":120}))
            .unwrap()
            .contains("values.jsonl"));
        let stopped = runner
            .run_step(&json!({"action":"stop_native_property_trace"}))
            .unwrap();
        assert!(stopped.contains("bridge.jsonl"));
        assert!(stopped.contains("mapping unverified"));
        assert_eq!(
            *calls.lock().unwrap(),
            ["start:values:120:property", "stop"]
        );
    }
}

#[derive(Default)]
pub struct SystemBackend {
    client: Option<crate::tuner::TunerClient>,
    startup_lease: Option<crate::startup::RecoveryLease>,
    launch_floor: Option<f64>,
    startup_pin: Option<crate::startup::PinnedGame>,
    diagnostic_session: crate::startup::DiagnosticSession,
    native_trace: Option<crate::native_trace::Session>,
    native_bridge: Option<crate::native_bridge::Session>,
}
impl Backend for SystemBackend {
    fn live_lease_budget(&self) -> Result<Option<f64>> {
        crate::live_lease::environment_budget()
    }
    fn pin_startup_process(&mut self, hwnd: isize) -> Result<Option<crate::startup::GameProcess>> {
        let floor = self
            .launch_floor
            .ok_or_else(|| Error::input("no Steam launch request to pin"))?;
        let pin = crate::startup::pin_window_process(hwnd, floor)?;
        let game = pin.game;
        self.startup_pin = Some(pin);
        Ok(Some(game))
    }
    fn startup_loading_evidence(
        &mut self,
        game: crate::startup::GameProcess,
        logs: &Path,
        port: u16,
    ) -> Result<Value> {
        crate::startup::loading_evidence(game, logs, port, &mut self.diagnostic_session)
    }
    fn startup_diagnostics(
        &mut self,
        game: crate::startup::GameProcess,
        output: &Path,
    ) -> Result<Value> {
        crate::startup::collect_diagnostics(game, output, &mut self.diagnostic_session)
    }
    fn acquire_startup_lock(&mut self) -> Result<()> {
        self.startup_lease = Some(crate::startup::RecoveryLease::acquire()?);
        Ok(())
    }
    fn check_startup_lock(&mut self) -> Result<()> {
        self.startup_lease
            .as_mut()
            .ok_or_else(|| Error::input("shared startup lock was lost"))?
            .check()
    }
    fn release_startup_lock(&mut self) {
        self.startup_lease = None;
        self.startup_pin = None;
    }
    fn check_startup_process(&mut self, game: crate::startup::GameProcess) -> Result<()> {
        let pin = self
            .startup_pin
            .as_ref()
            .filter(|pin| pin.game == game)
            .ok_or_else(|| Error::input("startup process handle pin was lost"))?;
        pin.check_remaining()
    }
    fn startup_display_recovery(&mut self, game: crate::startup::GameProcess) -> Result<Value> {
        crate::startup::reset_display(
            self.startup_lease
                .as_mut()
                .ok_or_else(|| Error::input("shared startup lock was lost"))?,
            self.startup_pin
                .as_ref()
                .filter(|pin| pin.game == game)
                .ok_or_else(|| Error::input("startup process handle pin was lost"))?,
            &mut self.diagnostic_session,
        )
    }
    fn startup_finish_diagnostics(&mut self) -> Result<Value> {
        self.diagnostic_session.finish()
    }
    fn connect(&mut self, port: u16, timeout: f64) -> Result<()> {
        if self.client.is_none() {
            self.client = Some(
                crate::tuner::TunerClient::connect(port, timeout).map_err(|e| {
                    Error::tuner(format!(
                        "FireTuner に接続できません（Civ6 が起動していない可能性）: {e}"
                    ))
                })?,
            );
        }
        Ok(())
    }
    fn disconnect(&mut self) {
        if let Some(mut c) = self.client.take() {
            c.close();
        }
    }
    fn list_states(&mut self, timeout: f64) -> Result<Vec<String>> {
        self.client
            .as_mut()
            .ok_or_else(|| Error::tuner("FireTuner is not connected"))?
            .list_states(timeout)
    }
    fn run_lua(&mut self, state: &str, code: &str, timeout: f64) -> Result<String> {
        Ok(self
            .client
            .as_mut()
            .ok_or_else(|| Error::tuner("FireTuner is not connected"))?
            .run_lua(state, code, timeout)?
            .value)
    }
    fn require_single_process(&mut self) -> Result<()> {
        crate::click::require_single_civ_process()
    }
    fn single_process(&mut self, settle: f64) -> Result<Option<(u32, f64)>> {
        crate::restart::single_civ_process(settle)
    }
    fn start_native_trace(
        &mut self,
        output: &Path,
        name: &str,
        seconds: u64,
        mode: &str,
    ) -> Result<()> {
        if self.native_trace.is_some() {
            return Err(Error::scenario("native governor trace already active"));
        }
        if self.live_lease_budget()?.is_some_and(|remaining| {
            remaining <= civ6_native_probe_rs::startup_contract::TOTAL.as_secs_f64()
        }) {
            return Err(Error::input(
                "live_chain lease has less than 20 seconds for native observer startup",
            ));
        }
        self.require_single_process()?;
        let (pid, start) = self
            .single_process(0.0)?
            .ok_or_else(|| Error::input("Civ6 is absent"))?;
        self.native_trace = Some(crate::native_trace::Session::start(
            pid,
            start,
            output,
            name,
            seconds,
            mode,
            Box::new(crate::restart::terminate_civ_process),
        )?);
        Ok(())
    }
    fn stop_native_trace(&mut self) -> Result<Option<PathBuf>> {
        match self.native_trace.take() {
            Some(mut trace) => trace.stop().map(Some),
            None => Ok(None),
        }
    }
    fn start_native_bridge(
        &mut self,
        output: &Path,
        name: &str,
        seconds: u64,
        mode: &str,
    ) -> Result<()> {
        if self.native_bridge.is_some() || self.native_trace.is_some() {
            return Err(Error::scenario("another native observation is active"));
        }
        if self
            .live_lease_budget()?
            .is_some_and(|remaining| remaining <= seconds as f64 + 45.0)
        {
            return Err(Error::input(
                "live_chain lease cannot cover native bridge telemetry",
            ));
        }
        self.require_single_process()?;
        let (pid, start) = self
            .single_process(0.0)?
            .ok_or_else(|| Error::input("Civ6 is absent"))?;
        self.native_bridge = Some(crate::native_bridge::Session::start(
            pid,
            start,
            output,
            name,
            seconds,
            mode,
            Box::new(crate::native_bridge::close_pinned_game),
        )?);
        Ok(())
    }
    fn stop_native_bridge(&mut self) -> Result<Option<PathBuf>> {
        match self.native_bridge.as_mut() {
            Some(bridge) => {
                let result = bridge.stop().map(Some);
                if result.is_ok() {
                    self.native_bridge = None;
                }
                result
            }
            None => Ok(None),
        }
    }
    fn memory(&mut self, pid: u32) -> Result<Value> {
        metrics::process_memory(pid)
    }
    fn launch(&mut self) -> Result<()> {
        let floor = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::input("Steam launch timestamp unavailable"))?
            .as_secs_f64();
        self.launch_floor = None;
        crate::restart::request_steam_launch()?;
        self.launch_floor = Some(floor);
        Ok(())
    }
    fn close_orphaned_reporters(&mut self) -> Result<Vec<u32>> {
        crate::restart::close_orphaned_civ_reporters()
    }
    fn wait_window(&mut self, deadline: f64) -> Result<isize> {
        crate::restart::wait_for_civ_window(deadline)
    }
    fn require_dx11(&mut self, hwnd: isize) -> Result<()> {
        crate::restart::require_dx11_window(hwnd)
    }
    fn click(&mut self, fx: f64, fy: f64) -> Result<(i32, i32)> {
        crate::click::click_fraction(fx, fy)
    }
    fn press_key(&mut self, key: u16, deadline: f64) -> Result<()> {
        crate::click::press_key(key, Some(deadline))
    }
    fn request_close(&mut self) -> Result<()> {
        crate::click::request_civ_window_close()
    }
    fn wait_exit(&mut self, pid: u32, deadline: f64) -> Result<bool> {
        crate::restart::wait_for_exit(pid, deadline)
    }
    fn terminate(&mut self, pid: u32, start: f64) -> Result<()> {
        crate::restart::terminate_civ_process(pid, start)
    }
    fn capture(&mut self, path: &Path, crop: Option<[f64; 4]>, deadline: f64) -> Result<()> {
        crate::capture::capture_civ_window(path, crop, Some(deadline))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoplayStatus {
    pub turn: i64,
    pub active: bool,
    pub event: bool,
    pub remaining: i64,
    pub local_player: i64,
}
pub fn parse_autoplay_status(value: &str) -> Result<AutoplayStatus> {
    let parts: Vec<_> = value.split('|').collect();
    let error = || Error::scenario(format!("Unexpected autoplay status: {value:?}"));
    if parts.len() != 5
        || !["true", "false"].contains(&parts[1])
        || !["true", "false"].contains(&parts[2])
    {
        return Err(error());
    }
    Ok(AutoplayStatus {
        turn: parts[0].parse().map_err(|_| error())?,
        active: parts[1] == "true",
        event: parts[2] == "true",
        remaining: parts[3].parse().map_err(|_| error())?,
        local_player: parts[4].parse().map_err(|_| error())?,
    })
}
#[derive(Clone, Debug)]
pub struct AutoplayOptions {
    pub deadline: Option<f64>,
    pub record_metrics: bool,
    pub research_metrics: bool,
    pub allow_human_elimination: bool,
    pub metrics_group: String,
    pub start_turn_override: Option<i64>,
}
impl Default for AutoplayOptions {
    fn default() -> Self {
        Self {
            deadline: None,
            record_metrics: false,
            research_metrics: false,
            allow_human_elimination: false,
            metrics_group: "AUTOPLAY".into(),
            start_turn_override: None,
        }
    }
}
pub struct MetricsRun {
    pub recorder: metrics::TurnMetrics,
    pub pid: u32,
    pub requested_turns: i64,
    pub group: String,
}
pub struct Runner {
    pub scenario: Value,
    pub output: PathBuf,
    pub logs: PathBuf,
    pub saves: PathBuf,
    pub port: u16,
    pub auto_restart: bool,
    pub mods_dir: PathBuf,
    pub owner_start: bool,
    pub test_mode: bool,
    pub baseline: Option<Value>,
    pub log_step_baseline: Option<Value>,
    pub log_matches_consumed: BTreeMap<(String, String), usize>,
    pub metrics_runs: usize,
    pub turn_metrics: Vec<MetricsRun>,
    pub save_load_metrics: metrics::SaveLoadMetrics,
    pub result_passed: bool,
    pub completed_steps: usize,
    pub result_checks: Vec<judge::Check>,
    pub failure: Option<String>,
    pub lua_captures: BTreeMap<String, String>,
    pub ruleset: Option<String>,
    checkpoint_run_id: String,
    checkpoint_sequence: u64,
    pub backend: Box<dyn Backend>,
    pub clock: Arc<dyn Clock>,
}
impl Runner {
    pub fn new(scenario: Value, output: PathBuf, logs: PathBuf, saves: PathBuf, port: u16) -> Self {
        Self::with_backend(
            scenario,
            output,
            logs,
            saves,
            port,
            Box::<SystemBackend>::default(),
            Arc::new(SystemClock),
        )
    }
    pub fn with_backend(
        scenario: Value,
        output: PathBuf,
        logs: PathBuf,
        saves: PathBuf,
        port: u16,
        backend: Box<dyn Backend>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let save_load_metrics =
            metrics::SaveLoadMetrics::new(output.join("save-load-metrics.csv"), false);
        Self {
            scenario,
            output,
            logs,
            saves,
            port,
            auto_restart: true,
            mods_dir: crate::restart::default_mods_dir(),
            owner_start: false,
            test_mode: false,
            baseline: None,
            log_step_baseline: None,
            log_matches_consumed: BTreeMap::new(),
            metrics_runs: 0,
            turn_metrics: vec![],
            save_load_metrics,
            result_passed: false,
            completed_steps: 0,
            result_checks: vec![],
            failure: None,
            lua_captures: BTreeMap::new(),
            ruleset: None,
            checkpoint_run_id: uuid::Uuid::new_v4().simple().to_string(),
            checkpoint_sequence: 0,
            backend,
            clock,
        }
    }
    pub fn remaining(&self, deadline: f64) -> Result<f64> {
        crate::time::check_interrupt()?;
        let lease = self.backend.live_lease_budget()?;
        let v = deadline - self.clock.now();
        if v <= 0.0 {
            Err(Error::scenario("Step deadline expired"))
        } else {
            Ok(lease.map_or(v, |lease| v.min(lease)))
        }
    }
    fn budget(&self, deadline: f64, cap: f64) -> Result<f64> {
        Ok(self.remaining(deadline)?.min(cap))
    }
    fn sleep_until(&self, deadline: f64, seconds: f64) {
        self.clock
            .sleep(seconds.min((deadline - self.clock.now()).max(0.0)));
    }
    pub fn log(&mut self, action: &str, status: &str, detail: &str) -> Result<()> {
        if status == "FAIL" {
            self.failure = Some(format!("{action}: {detail}"));
        }
        println!("[{status}] {action}: {detail}");
        let mut handle = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.output.join("steps.jsonl"))?;
        writeln!(
            handle,
            "{}",
            json!({"at":crate::time::utc_now(),"action":action,"status":status,"detail":detail})
        )?;
        Ok(())
    }
    pub fn disconnect(&mut self) {
        self.backend.disconnect();
    }
    pub fn states(&mut self, timeout: f64) -> Result<Vec<String>> {
        self.backend.require_single_process()?;
        let deadline = self.clock.now() + timeout;
        let fetch = |this: &mut Self| -> Result<Vec<String>> {
            let remaining = this.remaining(deadline).map_err(|e| {
                if e.kind == ErrorKind::LiveLease {
                    e
                } else {
                    Error::tuner("FireTuner に接続できません（応答待ちがタイムアウト）")
                }
            })?;
            this.backend.connect(this.port, remaining)?;
            let remaining = this.remaining(deadline).map_err(|e| {
                if e.kind == ErrorKind::LiveLease {
                    e
                } else {
                    Error::tuner("FireTuner に接続できません（応答待ちがタイムアウト）")
                }
            })?;
            this.backend.list_states(remaining)
        };
        match fetch(self) {
            Err(e) if retryable(&e) => {
                self.disconnect();
                fetch(self)
            }
            other => other,
        }
    }
    pub fn lua(&mut self, state: &str, code: &str, timeout: f64) -> Result<String> {
        let deadline = self.clock.now() + timeout;
        self.states(self.remaining(deadline)?)?;
        let remaining = self.remaining(deadline).map_err(|e| {
            if e.kind == ErrorKind::LiveLease {
                e
            } else {
                Error::tuner("FireTuner の応答待ちがタイムアウトし、Lua コマンドは未送信です")
            }
        })?;
        self.backend.connect(self.port, remaining)?;
        // Connecting can consume the last budget; never mutate after it expired.
        let remaining = self.remaining(deadline)?;
        self.backend.run_lua(state, code, remaining)
    }
    fn captive_session_read(
        &mut self,
        label: &str,
        state: &str,
        code: &str,
        deadline: f64,
        per_lua: f64,
    ) -> Result<Option<String>> {
        let timeout = self.budget(deadline, per_lua)?;
        match self.lua(state, code, timeout) {
            Ok(value) => {
                self.log("captive_session_probe", "OK", &format!("{label}={value}"))?;
                Ok(Some(value))
            }
            Err(e)
                if matches!(
                    e.kind,
                    ErrorKind::LuaResponseUnknown
                        | ErrorKind::LuaExecution
                        | ErrorKind::Tuner
                        | ErrorKind::Io
                ) =>
            {
                self.log(
                    "captive_session_probe",
                    "UNAVAILABLE",
                    &format!("{label}: {e}"),
                )?;
                self.disconnect();
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
    fn observe_captive_session(&mut self, step: &Value, deadline: f64) -> Result<String> {
        let attacker = step["attacker"].as_u64().unwrap();
        let captor = step["captor"].as_u64().unwrap();
        let per_lua = step["lua_timeout"].as_f64().unwrap();
        let states = self.states(self.budget(deadline, per_lua)?)?;
        let in_game = states.iter().any(|s| state_name_eq(s, "InGame"));
        let deal_view = states.iter().any(|s| state_name_eq(s, "DiplomacyDealView"));
        self.log(
            "captive_session_probe",
            "OK",
            &format!("contexts InGame={in_game} DiplomacyDealView={deal_view}"),
        )?;
        let mut hidden = self.captive_session_read(
            "deal_view_hidden",
            "DiplomacyDealView",
            "return ContextPtr:IsHidden()",
            deadline,
            per_lua,
        )?;
        let snapshot_code="local b=ExposedMembers.JAC_PersonaBoundary; return b and b.CaptiveExchangeSessionReadOnly() or 'NONE'";
        let in_game_snapshot = self.captive_session_read(
            "ingame_snapshot",
            "InGame",
            snapshot_code,
            deadline,
            per_lua,
        )?;
        let deal_snapshot = self.captive_session_read(
            "deal_view_snapshot",
            "DiplomacyDealView",
            snapshot_code,
            deadline,
            per_lua,
        )?;
        let id_code=format!("local id=DiplomacyManager.FindOpenSessionID({attacker},{captor}); if id==nil then return 'nil' end; return type(id)=='number' and tostring(id) or 'TYPE:'..type(id)");
        let in_game_id =
            self.captive_session_read("ingame_session_id", "InGame", &id_code, deadline, per_lua)?;
        let deal_id = self.captive_session_read(
            "deal_view_session_id",
            "DiplomacyDealView",
            &id_code,
            deadline,
            per_lua,
        )?;
        let parsed_id =
            |value: &Option<String>| value.as_deref().and_then(|s| s.parse::<u64>().ok());
        let id_for_info = parsed_id(&in_game_id).or_else(|| parsed_id(&deal_id));
        let (in_game_from, deal_from) = if let Some(id) = id_for_info {
            let code=format!("local info=DiplomacyManager.GetSessionInfo({id}); if info==nil then return 'nil' end; return type(info.FromPlayer)=='number' and tostring(info.FromPlayer) or 'TYPE:'..type(info.FromPlayer)");
            (
                self.captive_session_read(
                    "ingame_from_player",
                    "InGame",
                    &code,
                    deadline,
                    per_lua,
                )?,
                self.captive_session_read(
                    "deal_view_from_player",
                    "DiplomacyDealView",
                    &code,
                    deadline,
                    per_lua,
                )?,
            )
        } else {
            self.log(
                "captive_session_probe",
                "UNAVAILABLE",
                "GetSessionInfo skipped: no numeric session ID observed",
            )?;
            (None, None)
        };
        let same_native_id =
            parsed_id(&in_game_id).is_some() && parsed_id(&in_game_id) == parsed_id(&deal_id);
        let snapshot_prefix = format!("{attacker}|{captor}|");
        let same_snapshot = in_game_snapshot.as_deref().is_some_and(|s| {
            s.starts_with(&snapshot_prefix) && s.contains("|true|false|") && s.ends_with("|nil")
        }) && in_game_snapshot == deal_snapshot;
        let expected_from = attacker.to_string();
        // Visibility is a read-only getter. The native session can become
        // responsive while its opening animation completes (live 105). Read
        // visibility once more after the other evidence, within the same
        // deadline; the final binding still revalidates the exact native ID.
        if in_game
            && deal_view
            && same_native_id
            && same_snapshot
            && in_game_from.as_deref() == Some(expected_from.as_str())
            && deal_from.as_deref() == Some(expected_from.as_str())
            && hidden.as_deref() != Some("false")
        {
            hidden = self.captive_session_read(
                "deal_view_hidden_recheck",
                "DiplomacyDealView",
                "return ContextPtr:IsHidden()",
                deadline,
                per_lua,
            )?;
        }
        if in_game
            && deal_view
            && hidden.as_deref() == Some("false")
            && same_native_id
            && same_snapshot
            && in_game_from.as_deref() == Some(expected_from.as_str())
            && deal_from.as_deref() == Some(expected_from.as_str())
        {
            let observed_id = parsed_id(&in_game_id).unwrap();
            let code=format!("return ExposedMembers.JAC_PersonaBoundary.PinObservedCaptiveExchangeSession({observed_id})");
            let ack = match self.lua("InGame", &code, self.budget(deadline, per_lua)?) {
                Ok(value) => value,
                Err(e) if e.kind == ErrorKind::LuaResponseUnknown => {
                    self.log(
                        "captive_session_probe",
                        "UNAVAILABLE",
                        &format!(
                            "binding acknowledgment unknown for native session {observed_id}: {e}"
                        ),
                    )?;
                    return Err(e);
                }
                Err(e) => return Err(e),
            };
            if ack.parse::<u64>().ok() != Some(observed_id) {
                return Err(Error::scenario(format!(
                    "native captive session binding returned {ack:?}, expected exact ID {observed_id}"
                )));
            }
            self.log(
                "captive_session_probe",
                "OK",
                &format!("bound native session {observed_id} to prepared captive item"),
            )?;
            return Ok(format!(
                "native captive diplomacy session {observed_id} observed and bound",
            ));
        }
        Err(Error::scenario(format!("captive session observation incomplete: visible={hidden:?}, native_ids={in_game_id:?}/{deal_id:?}, from={in_game_from:?}/{deal_from:?}, snapshots={in_game_snapshot:?}/{deal_snapshot:?}; observation made no proposal")))
    }
    pub fn log_since_same_generation(&self, name: &str, baseline: &Value) -> Result<String> {
        let path = self.logs.join(name);
        let mark = &baseline["files"][name];
        let mut file = match fs::File::open(&path) {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if mark["inode"].is_null() {
                    return Ok(String::new());
                }
                return Err(Error::scenario(format!(
                    "{name} disappeared during wait; outcome undecidable"
                )));
            }
            Err(e) => return Err(e.into()),
        };
        let (contents, inode) = judge::read_log_handle(&mut file)?;
        if judge::log_marker_replaced(&contents, mark, inode) {
            return Err(Error::scenario(format!(
                "{name} rotated or truncated during wait; outcome undecidable"
            )));
        }
        Ok(judge::slice_log_bytes(&contents, mark, inode))
    }
    pub fn wait(&mut self, condition: &Value, timeout: f64, close_diplomacy: bool) -> Result<()> {
        let end = self.clock.now() + timeout;
        let mut last = "not ready".to_string();
        let mut unblocked = 0;
        let mut diplomacy_closes = 0;
        let log_baseline = if condition["type"] == "log_line" {
            Some(match &self.log_step_baseline {
                Some(v) => v.clone(),
                None => judge::snapshot_logs(&self.logs)?,
            })
        } else {
            None
        };
        while self.clock.now() < end {
            if close_diplomacy {
                let closed = self.close_diplomacy(end)?;
                if !closed.is_empty() {
                    diplomacy_closes += closed.len();
                    self.log(
                        "end_turn_diplomacy",
                        "OK",
                        &format!("closed {}; total {diplomacy_closes}", closed.join(", ")),
                    )?;
                    if diplomacy_closes > 25 {
                        return Err(Error::scenario(
                            "AI diplomacy screens keep reopening during one end_turn",
                        ));
                    }
                }
            }
            let kind = condition["type"].as_str().unwrap_or("");
            let key = (
                text(condition, "log").to_string(),
                text(condition, "contains").to_string(),
            );
            let present: Result<bool> = match kind {
                "state" => {
                    let budget = self.budget(end, 5.0)?;
                    self.states(budget)
                        .map(|s| s.iter().any(|v| state_name_eq(v, text(condition, "name"))))
                }
                "lua" => {
                    let budget = self.budget(end, 5.0)?;
                    self.lua(
                        text(condition, "state_name"),
                        text(condition, "code"),
                        budget,
                    )
                    .map(|v| {
                        last = format!("Lua returned {v:?}");
                        v == condition
                            .get("equals")
                            .map(|v| python_value_string(v).to_lowercase())
                            .unwrap_or_else(|| "true".into())
                    })
                }
                _ => {
                    let content = self.log_since_same_generation(
                        text(condition, "log"),
                        log_baseline.as_ref().unwrap(),
                    )?;
                    Ok(content
                        .lines()
                        .filter(|line| line.contains(text(condition, "contains")))
                        .count()
                        > *self.log_matches_consumed.get(&key).unwrap_or(&0))
                }
            };
            match present {
                Ok(true) => {
                    self.remaining(end)?;
                    if kind == "log_line" {
                        *self.log_matches_consumed.entry(key).or_default() += 1;
                    }
                    return Ok(());
                }
                Ok(false) => {}
                Err(e) if e.kind == ErrorKind::LuaExecution => return Err(e),
                Err(e) if e.kind == ErrorKind::LuaResponseUnknown => {
                    if !self.unblock_ingame(
                        text(condition, "state_name"),
                        end,
                        unblocked,
                        condition["preserve_diplomacy_session"] == true,
                    )? {
                        return Err(e);
                    }
                    unblocked += 1;
                    continue;
                }
                Err(e) if retryable(&e) => last = e.to_string(),
                Err(e) => return Err(e),
            }
            self.sleep_until(end, 0.5);
        }
        Err(Error::scenario(format!(
            "Timed out after {timeout}s waiting for {condition}; last: {last}"
        )))
    }
    pub fn retry_idempotent<T>(
        &mut self,
        deadline: f64,
        mut action: impl FnMut(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let mut delay = 0.5;
        loop {
            match action(self) {
                Err(e) if retryable(&e) && e.kind != ErrorKind::LuaExecution => {
                    if self.clock.now() + delay >= deadline {
                        return Err(e);
                    }
                    println!("Transient FireTuner failure, retrying: {e}");
                    self.disconnect();
                    self.clock.sleep(delay);
                    delay = (delay * 2.0).min(4.0);
                }
                value => return value,
            }
        }
    }
    pub fn unblock_ingame(
        &mut self,
        state: &str,
        deadline: f64,
        attempts: usize,
        preserve_diplomacy_session: bool,
    ) -> Result<bool> {
        if !state_name_eq(state, "InGame") || attempts >= 3 {
            return Ok(false);
        }
        self.disconnect();
        if preserve_diplomacy_session {
            // A scenario expecting a native deal session may reconnect for a
            // read, but must never close the very view it is waiting for.
            return Ok(true);
        }
        let end = deadline.min(self.clock.now() + 10.0);
        let closed = match self.close_diplomacy_once(end) {
            Ok(v) => v,
            Err(_) => return Ok(false),
        };
        if !closed.is_empty() {
            self.log(
                "ingame_unblock",
                "OK",
                &format!(
                    "closed {} after an InGame timeout; retry {}/3",
                    closed.join(", "),
                    attempts + 1
                ),
            )?;
        }
        Ok(!closed.is_empty())
    }
    pub fn close_options_menu(&mut self, deadline: f64) -> Result<bool> {
        let budget = self.budget(deadline, 5.0)?;
        if !self
            .states(budget)?
            .iter()
            .any(|s| state_name_eq(s, "InGameTopOptionsMenu"))
        {
            return Ok(false);
        }
        let closed=self.lua("InGameTopOptionsMenu","if not ContextPtr:IsHidden() then OnReturn(); CloseImmediately(); return true end\nreturn false",self.budget(deadline,5.0)?)?=="true";
        if closed {
            self.wait(&json!({"type":"lua","state_name":"InGameTopOptionsMenu","code":"return ContextPtr:IsHidden()"}),self.budget(deadline,5.0)?,false)?;
            println!("Closed in-game options menu");
        }
        Ok(closed)
    }
    pub fn dismiss_advisor(&mut self, deadline: f64) -> Result<bool> {
        self.retry_idempotent(deadline, |r| r.dismiss_advisor_once(deadline))
    }
    pub fn dismiss_advisor_once(&mut self, deadline: f64) -> Result<bool> {
        if !self
            .states(self.budget(deadline, 5.0)?)?
            .iter()
            .any(|s| state_name_eq(s, "AdvisorPopup"))
        {
            return Ok(false);
        }
        let dismissed = self.lua(
            "AdvisorPopup",
            "if IsBlockingInput() then OnHideAdvisorDialog(); return true end\nreturn false",
            self.budget(deadline, 5.0)?,
        )? == "true";
        if dismissed {
            println!("Dismissed blocking Advisor popup");
        }
        Ok(dismissed)
    }
    pub fn close_diplomacy(&mut self, deadline: f64) -> Result<Vec<String>> {
        self.retry_idempotent(deadline, |r| r.close_diplomacy_once(deadline))
    }
    pub fn close_diplomacy_once(&mut self, deadline: f64) -> Result<Vec<String>> {
        let states = self.states(self.budget(deadline, 5.0)?)?;
        let mut closed = vec![];
        for (state, call, label) in POPUP_CLOSERS {
            if states.iter().any(|s| state_name_eq(s, state))
                && self.lua(
                    state,
                    &format!("if ContextPtr:IsHidden() then return false end\n{call}\nreturn true"),
                    self.budget(deadline, 5.0)?,
                )? == "true"
            {
                closed.push(label.to_string());
            }
        }
        Ok(closed)
    }
    pub fn autoplay_status(&mut self, deadline: f64) -> Result<AutoplayStatus> {
        let end = deadline.min(self.clock.now() + 30.0);
        self.retry_idempotent(end, |r| r.autoplay_status_once(deadline))
    }
    pub fn autoplay_status_once(&mut self, deadline: f64) -> Result<AutoplayStatus> {
        parse_autoplay_status(&self.lua(
            "InGame",
            AUTOPLAY_STATUS_CODE,
            self.budget(deadline, 5.0)?,
        )?)
    }
    pub fn research_query(
        &mut self,
        code: &str,
        deadline: f64,
        minimum_turn: Option<i64>,
    ) -> Result<String> {
        loop {
            let value = self.retry_idempotent(deadline, |r| {
                r.lua("InGame", code, r.budget(deadline, 10.0)?)
            })?;
            if let Some(minimum) = minimum_turn {
                if metrics::parse_research_sample(&value)?.0 < minimum {
                    self.clock.sleep(self.budget(deadline, 0.1)?);
                    continue;
                }
            }
            return Ok(value);
        }
    }
}
fn retryable(e: &Error) -> bool {
    matches!(
        e.kind,
        ErrorKind::Tuner | ErrorKind::LuaResponseUnknown | ErrorKind::LuaExecution | ErrorKind::Io
    )
}
fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
pub const LOADSCREEN_READY: &str = "LoadScreen: OnLoadGameViewStateDone";
pub const MAIN_MENU_READY: &str = r#"
if ContextPtr:IsHidden() then return false end
local stack = Controls.MainMenuOptionStack
if stack:GetSizeY() <= 0 or not Controls.MainButtonTrackAnim:IsStopped() or not stack:GetParent():IsStopped() then return false end
local function labelsReady(control)
    if control:GetID() == 'LabelAlphaAnim' and not control:IsStopped() then return false end
    for _, child in ipairs(control:GetChildren() or {}) do
        if not labelsReady(child) then return false end
    end
    return true
end
return labelsReady(stack)
"#;
// Match only the localized connection-error message. The official Escape handler
// acknowledges that message and clears its instance; login and legal dialogs stay open.
pub const DISMISS_2K_CONNECTION_MESSAGE: &str = r#"
if ContextPtr:IsHidden() then return false end
local function find(control, id)
    if control:GetID() == id then return control end
    for _, child in ipairs(control:GetChildren() or {}) do
        local match = find(child, id)
        if match then return match end
    end
end
local title = find(Controls.My2KBG, 'Title')
if not title or title:GetText() ~= Locale.Lookup('TXT_KEY_MY2K_ADDITION_CONNECTION_ISSUE') then return false end
InputHandler(KeyEvents.KeyUp, Keys.VK_ESCAPE, 0)
return true
"#;
pub const POPUP_CLOSERS: [(&str, &str, &str); 6] = [
    (
        "DiplomacyDealView",
        "OnLocalPlayerTurnEnd()",
        "AI deal proposal",
    ),
    ("DiplomacyActionView", "OnClose()", "AI diplomacy statement"),
    (
        "JAC_SengokuReignPopup",
        "OnClose()",
        "era-name change screen",
    ),
    ("DedicationPopup", "OnClose()", "house oath choice"),
    (
        "JAC_SengokuDarkPolicy",
        "OnClose()",
        "dark-age policy choice",
    ),
    // The official Close handler releases the disaster's GameCore playback
    // event and popup lock. Hiding the context alone leaves the turn paused.
    (
        "NaturalDisasterPopup",
        "OnClose()",
        "natural disaster report",
    ),
];
pub const AUTOPLAY_STATUS_CODE:&str="return tostring(Game.GetCurrentGameTurn()) .. '|' .. tostring(AutoplayManager.IsActive()) .. '|' .. tostring(JAC_AUTOTEST_AUTO_DONE == true) .. '|' .. tostring(AutoplayManager.GetTurns()) .. '|' .. tostring(Game.GetLocalPlayer())";
const HUMAN_TURN_READY: &str = "Game.GetLocalPlayer() >= 0 and Game.GetLocalPlayer() ~= PlayerTypes.OBSERVER and Players[Game.GetLocalPlayer()] ~= nil and Players[Game.GetLocalPlayer()]:IsTurnActive() and not UI.IsGameCoreBusy() and not UI.IsProcessingMessages()";
fn ready_turn_code(turn: i64) -> String {
    format!("return Game.GetCurrentGameTurn() == {turn} and {HUMAN_TURN_READY}")
}

#[derive(Default)]
struct StartupAttempt {
    launch_requested: bool,
    process: Option<crate::startup::GameProcess>,
    frontend_seen: bool,
    loading_timeout: bool,
}

impl Runner {
    pub fn wait_frontend_ready(&mut self, timeout: f64, skip_opening: bool) -> Result<()> {
        self.wait_startup_frontend(timeout, skip_opening, &mut StartupAttempt::default())
    }
    fn wait_startup_frontend(
        &mut self,
        timeout: f64,
        mut skip_opening: bool,
        attempt: &mut StartupAttempt,
    ) -> Result<()> {
        let deadline = self.clock.now() + timeout;
        let mut next_click = self.clock.now() + 12.0;
        let mut last = "FrontEnd not ready".into();
        while self.clock.now() < deadline {
            self.backend.startup_cancel_check()?;
            if skip_opening && self.clock.now() >= next_click {
                if self.backend.click(0.5, 0.15).is_err() {
                    skip_opening = false;
                }
                next_click = self.clock.now() + 12.0;
            }
            let probe = (|| -> Result<String> {
                let frontend = self.lua(
                    "FrontEnd",
                    "return UI.IsInFrontEnd()",
                    self.budget(deadline, 5.0)?,
                )?;
                if frontend != "true" {
                    return Ok(frontend);
                }
                attempt.frontend_seen = true;
                let menu = self.lua("MainMenu", MAIN_MENU_READY, self.budget(deadline, 5.0)?)?;
                if menu == "true" {
                    return Ok(menu);
                }
                let states = self.states(self.budget(deadline, 5.0)?)?;
                if states.iter().any(|s| state_name_eq(s, "My2K")) {
                    // Rechecking visibility/title makes a lost acknowledgment safe:
                    // a cleared instance cannot acknowledge the message a second time.
                    let dismissed = self.lua(
                        "My2K",
                        DISMISS_2K_CONNECTION_MESSAGE,
                        self.budget(deadline, 5.0)?,
                    )?;
                    if dismissed == "true" {
                        println!("Acknowledged the 2K connection-error notification");
                    }
                }
                Ok("main menu hidden or still animating".into())
            })();
            match probe {
                Ok(value) if value == "true" => return Ok(()),
                Ok(value) => last = format!("FrontEnd returned {value:?}"),
                Err(e) if e.kind == ErrorKind::LuaExecution => return Err(e),
                Err(e) if retryable(&e) => {
                    last = e.to_string();
                    self.disconnect();
                }
                Err(e) if e.kind == ErrorKind::Scenario && self.clock.now() >= deadline => break,
                Err(e) => return Err(e),
            }
            self.sleep_until(deadline, 0.5);
        }
        self.backend.startup_cancel_check()?;
        attempt.loading_timeout = !attempt.frontend_seen;
        Err(Error::scenario(format!("Main menu did not become ready after {timeout}s (Civ6 may be stuck while starting; check FireTuner is enabled and the game reached the main menu); last: {last}")))
    }
    pub fn launch_steam_and_wait(&mut self, deadline: f64) -> Result<()> {
        self.launch_startup_and_wait(deadline, false, false, &mut StartupAttempt::default())
    }
    fn launch_startup_and_wait(
        &mut self,
        deadline: f64,
        recovery: bool,
        pin_process: bool,
        startup: &mut StartupAttempt,
    ) -> Result<()> {
        let mut attempts = 0;
        let mut next_launch = 0.0;
        let mut last = "no Civ VI process yet".to_string();
        while self.clock.now() < deadline {
            self.backend.startup_cancel_check()?;
            if recovery {
                self.backend.check_startup_lock()?;
            }
            let now = self.clock.now();
            let present = match self.backend.single_process(0.0) {
                Ok(v) => v.is_some(),
                Err(e)
                    if e.kind == ErrorKind::Input
                        && e.message.starts_with("Multiple Civ VI processes found") =>
                {
                    true
                }
                Err(e) => return Err(e),
            };
            if !present && now >= next_launch && attempts < 4 {
                let closed = self.backend.close_orphaned_reporters()?;
                if !closed.is_empty() {
                    println!("Closed helpers from failed launch: {closed:?}");
                    self.clock.sleep(self.budget(deadline, 8.0)?);
                    match self.backend.single_process(0.0) {
                        Ok(Some(_)) => continue,
                        Err(e)
                            if e.kind == ErrorKind::Input
                                && e.message.starts_with("Multiple Civ VI processes found") =>
                        {
                            continue
                        }
                        Err(e) => return Err(e),
                        Ok(None) => {}
                    }
                }
                self.remaining(deadline)?;
                if recovery {
                    self.backend.check_startup_lock()?;
                }
                self.backend.launch()?;
                startup.launch_requested = true;
                attempts += 1;
                next_launch = now + 45.0;
                println!("Steam Civ VI launch attempt {attempts}");
            }
            match self
                .backend
                .wait_window(deadline.min(self.clock.now() + 5.0))
            {
                Ok(hwnd) => {
                    self.backend.require_dx11(hwnd)?;
                    if pin_process && startup.launch_requested {
                        startup.process = self.backend.pin_startup_process(hwnd)?;
                    }
                    return Ok(());
                }
                Err(e) if e.kind == ErrorKind::Input => last = e.to_string(),
                Err(e) => return Err(e),
            }
        }
        Err(Error::scenario(format!(
            "Civ VI window did not appear after {attempts} Steam launch attempts within the deadline; last: {last}"
        )))
    }
    pub fn restart_game(&mut self, timeout: f64) -> Result<String> {
        if !timeout.is_finite() || timeout <= 0.0 {
            return Err(Error::input("restart timeout must be finite and positive"));
        }
        self.backend.startup_cancel_check()?;
        let mut first = StartupAttempt::default();
        let error = match self.restart_game_attempt(timeout, false, &mut first) {
            Ok(detail) => return Ok(detail),
            Err(error) => error,
        };
        if !first.loading_timeout || !first.launch_requested || error.kind != ErrorKind::Scenario {
            return Err(error);
        }
        let Some(game) = first.process else {
            return Err(error);
        };
        let directory = self
            .output
            .join(format!("startup-recovery-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory)?;
        let mut report = json!({"schema_version":1,"reason":"loading_startup_timeout",
            "marker":crate::startup::RECOVERY_MARKER,"timeout_per_attempt_seconds":timeout,
            "process":game,"trigger":{"launch_requested":true,"frontend_seen":false},
            "recovery_attempted":false,"reset":{"status":"skipped","reason":"not authorized yet"},
            "retry":{"status":"not_started"},"result":"diagnosing"});
        println!(
            "{} {}",
            crate::startup::RECOVERY_MARKER,
            json!({"phase":"diagnosing","process":game,"artifact":directory,"max_retries":1})
        );
        self.diagnose_startup(game, &directory, &mut report, "attempt1")?;
        self.backend.startup_cancel_check()?;
        let evidence = match self
            .backend
            .startup_loading_evidence(game, &self.logs, self.port)
        {
            Ok(evidence) => evidence,
            Err(reason) => {
                self.backend.startup_cancel_check()?;
                report["result"] = json!("recovery_refused");
                report["reset"]["reason"] = json!(reason.to_string());
                self.write_startup_report(&directory, &report)?;
                if reason.kind == ErrorKind::Interrupted {
                    return Err(reason);
                }
                return Err(error);
            }
        };
        report["trigger"]["evidence"] = evidence;
        report["recovery_attempted"] = json!(true);
        self.write_startup_report(&directory, &report)?;
        println!(
            "{} {}",
            crate::startup::RECOVERY_MARKER,
            json!({"process":game,"artifact":directory,"max_retries":1})
        );
        let result = (|| -> Result<String> {
            self.backend.startup_cancel_check()?;
            self.backend.acquire_startup_lock()?;
            report["lock"] = json!({"status":"held"});
            self.stop_startup_process(game, &mut report)?;
            self.backend.startup_cancel_check()?;
            self.backend.check_startup_lock()?;
            self.backend.check_startup_process(game)?;
            report["reset"] = self.backend.startup_display_recovery(game)?;
            self.write_startup_report(&directory, &report)?;
            self.backend.startup_cancel_check()?;
            self.backend.check_startup_lock()?;
            report["helpers"] = self.backend.startup_finish_diagnostics()?;
            if !self.backend.wait_exit(game.pid, self.clock.now() + 30.0)? {
                return Err(Error::scenario("pinned startup process remains after display recovery; refusing another launch"));
            }
            if self.backend.single_process(0.0)?.is_some() {
                return Err(Error::input(
                    "a Civ6 process remains or appeared before the startup retry",
                ));
            }
            report["retry"] = json!({"status":"started","attempt":2});
            self.write_startup_report(&directory, &report)?;
            let mut second = StartupAttempt::default();
            let result = self.restart_game_attempt(timeout, true, &mut second);
            report["retry"]["process"] = json!(second.process);
            report["retry"]["status"] = json!(if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            });
            if let Err(ref failure) = result {
                report["retry"]["reason"] = json!(failure.to_string());
                if second.loading_timeout && failure.kind == ErrorKind::Scenario {
                    if let Some(process) = second.process {
                        let diagnosis =
                            self.diagnose_startup(process, &directory, &mut report, "attempt2");
                        // No reset or third launch, even if the retry has the same failure.
                        self.stop_startup_process(process, &mut report)?;
                        diagnosis?;
                    }
                }
            }
            result.map(|detail| format!("{detail}; startup recovery retried once"))
        })();
        // Also release after lost ownership, Ctrl-C, diagnostics failure, or launch failure.
        self.backend.release_startup_lock();
        report["lock_released"] = json!(report.get("lock").is_some());
        report["result"] = json!(if result.is_ok() {
            "recovered"
        } else {
            "failed"
        });
        if let Err(ref failure) = result {
            report["failure"] = json!(failure.to_string());
            if report["reset"]["reason"] == "not authorized yet" {
                report["reset"]["reason"] = json!(failure.to_string());
            }
        }
        self.write_startup_report(&directory, &report)?;
        result.map_err(|mut failure| {
            failure.message = format!(
                "{} [{}; diagnostics: {}]",
                failure.message,
                crate::startup::RECOVERY_MARKER,
                directory.display()
            );
            failure
        })
    }
    fn write_startup_report(&self, directory: &Path, report: &Value) -> Result<()> {
        fs::write(
            directory.join("recovery.json"),
            serde_json::to_vec_pretty(report)?,
        )?;
        Ok(())
    }
    fn diagnose_startup(
        &mut self,
        game: crate::startup::GameProcess,
        directory: &Path,
        report: &mut Value,
        name: &str,
    ) -> Result<()> {
        self.backend.startup_cancel_check()?;
        let output = directory.join(name);
        fs::create_dir(&output)?;
        report[name] = json!({"process":game,"collected_before_shutdown":true,
            "logs":crate::startup::snapshot_logs(&self.logs, &output, game)});
        self.write_startup_report(directory, report)?;
        report[name]["native"] = match self.backend.startup_diagnostics(game, &output) {
            Ok(value) => value,
            Err(e) if e.kind == ErrorKind::Interrupted => {
                report[name]["native"] = json!({"status":"interrupted","reason":e.to_string()});
                self.write_startup_report(directory, report)?;
                return Err(e);
            }
            Err(e) => json!({"status":"unavailable","reason":e.to_string()}),
        };
        self.write_startup_report(directory, report)
    }
    fn stop_startup_process(
        &mut self,
        game: crate::startup::GameProcess,
        report: &mut Value,
    ) -> Result<()> {
        self.backend.startup_cancel_check()?;
        self.backend.check_startup_lock()?;
        if self.backend.single_process(0.0)? != Some((game.pid, game.started)) {
            return Err(Error::input(
                "pinned startup process changed; refusing shutdown",
            ));
        }
        self.disconnect();
        self.backend.terminate(game.pid, game.started)?;
        let exited = self.backend.wait_exit(game.pid, self.clock.now() + 10.0)?;
        report["shutdown"] = json!({"status":if exited {"exited"}else{"pending"},"process":game});
        Ok(())
    }
    fn restart_game_attempt(
        &mut self,
        timeout: f64,
        recovery: bool,
        attempt: &mut StartupAttempt,
    ) -> Result<String> {
        self.backend.startup_cancel_check()?;
        if recovery {
            self.backend.check_startup_lock()?;
        }
        let deadline = self.clock.now() + timeout;
        let settle = self.budget(deadline, 30.0)?;
        let old = self.backend.single_process(settle)?;
        if recovery && old.is_some() {
            return Err(Error::input("Civ6 appeared before the startup retry"));
        }
        if let Some((pid, started)) = old {
            let graceful = (|| -> Result<bool> {
                let states = self.states(self.budget(deadline, 5.0)?)?;
                let state = states
                    .iter()
                    .find(|s| state_name_eq(s, "FrontEnd"))
                    .or_else(|| states.iter().find(|s| state_name_eq(s, "InGame")));
                if let Some(state) = state {
                    match self.lua(
                        state,
                        "Events.UserConfirmedClose()\nreturn true",
                        self.budget(deadline, 5.0)?,
                    ) {
                        Ok(_) => {}
                        Err(e) if e.kind == ErrorKind::LuaResponseUnknown => {}
                        Err(e) => return Err(e),
                    }
                    return Ok(true);
                }
                Ok(false)
            })();
            self.disconnect();
            let graceful = match graceful {
                Ok(v) => v,
                Err(e) if retryable(&e) || e.kind == ErrorKind::Input => {
                    println!("FireTuner quit unavailable: {e}");
                    false
                }
                Err(e) => return Err(e),
            };
            let settle = self.budget(deadline, 30.0)?;
            if !graceful && self.backend.single_process(settle)? == old {
                if let Err(e) = self.backend.request_close() {
                    println!("Civ VI window quit unavailable: {e}");
                }
            }
            if !self
                .backend
                .wait_exit(pid, deadline.min(self.clock.now() + 15.0))?
            {
                self.backend.terminate(pid, started)?;
                if !self
                    .backend
                    .wait_exit(pid, deadline.min(self.clock.now() + 10.0))?
                {
                    return Err(Error::scenario(format!(
                        "Civ VI process {pid} did not exit after termination"
                    )));
                }
            }
        }
        self.remaining(deadline)?;
        if old.is_some() {
            self.clock.sleep(self.budget(deadline, 8.0)?);
        }
        let settle = self.budget(deadline, 30.0)?;
        if self.backend.single_process(settle)?.is_some() {
            return Err(Error::scenario(
                "A Civ VI process appeared before Steam relaunch",
            ));
        }
        let closed = self.backend.close_orphaned_reporters()?;
        if !closed.is_empty() {
            println!("Closed orphan Civ VI helpers: {closed:?}");
            self.clock.sleep(self.budget(deadline, 8.0)?);
        }
        self.launch_startup_and_wait(deadline, recovery, true, attempt)?;
        if self.owner_start {
            self.disconnect();
            self.wait_startup_frontend(self.remaining(deadline)?, false, attempt)?;
            return Ok(
                "Civ VI relaunched via Steam; opening played to the end; main menu visible and ready".into(),
            );
        }
        self.clock.sleep(self.budget(deadline, 8.0)?);
        if let Err(e) = self.backend.click(0.5, 0.15) {
            println!("Opening skip click unavailable ({e}); waiting for the opening");
        }
        self.disconnect();
        self.wait_startup_frontend(self.remaining(deadline)?, true, attempt)?;
        Ok(
            "Civ VI relaunched via Steam; opening skipped with a left click; main menu visible and ready"
                .into(),
        )
    }
    pub fn loadscreen_count(&self) -> Result<usize> {
        Ok(self
            .log_since_same_generation(
                "Lua.log",
                self.baseline
                    .as_ref()
                    .ok_or_else(|| Error::scenario("No log baseline for transition"))?,
            )?
            .matches(LOADSCREEN_READY)
            .count())
    }
    pub fn enter_game_after_loadscreen(
        &mut self,
        before: usize,
        timeout: f64,
        deadline: Option<f64>,
        response_unknown: bool,
    ) -> Result<()> {
        let end = deadline.unwrap_or_else(|| self.clock.now() + timeout);
        let mut sent = false;
        let mut last = "loading screen not ready".to_string();
        while self.clock.now() < end {
            let count = self.loadscreen_count()?;
            if !sent {
                if count <= before {
                    self.sleep_until(end, 0.5);
                    continue;
                }
                self.remaining(end)?;
                let states = match self.states(self.budget(end, 5.0)?) {
                    Ok(v) => v,
                    Err(e) if retryable(&e) => vec![],
                    Err(e) => return Err(e),
                };
                if states.iter().any(|s| state_name_eq(s, "LoadScreen")) {
                    self.lua(
                        "LoadScreen",
                        "OnActivateButtonClicked()\nreturn true",
                        self.budget(end, 5.0)?,
                    )?;
                    last = "Begin Game pressed via LoadScreen Lua".into();
                } else {
                    self.remaining(end)?;
                    self.backend.press_key(crate::click::VK_ESCAPE, end)?;
                    last = "Esc sent on loading screen".into();
                }
                sent = true;
                self.disconnect();
            }
            let check = (|| -> Result<bool> {
                if self
                    .states(self.budget(end, 5.0)?)?
                    .iter()
                    .any(|s| state_name_eq(s, "InGame"))
                {
                    if self.lua(
                        "InGame",
                        "return Game ~= nil and Game.GetLocalPlayer() >= 0",
                        self.budget(end, 5.0)?,
                    )? == "true"
                    {
                        self.close_options_menu(end)?;
                        self.dismiss_advisor(end)?;
                        if self.lua(
                            "InGame",
                            "return not UI.IsGameCoreBusy()",
                            self.budget(end, 5.0)?,
                        )? == "true"
                        {
                            self.remaining(end)?;
                            return Ok(true);
                        }
                    }
                    last =
                        "InGame exists; waiting for the local player and an idle game core".into();
                } else {
                    last = "InGame state not available yet".into();
                    self.disconnect();
                }
                Ok(false)
            })();
            match check {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(e)
                    if matches!(
                        e.kind,
                        ErrorKind::LuaResponseUnknown | ErrorKind::LuaExecution
                    ) =>
                {
                    return Err(e)
                }
                Err(e) if retryable(&e) => {
                    last = e.to_string();
                    self.disconnect();
                }
                Err(e) => return Err(e),
            }
            self.sleep_until(end, 2.0);
        }
        let uncertain = if response_unknown {
            "; outcome undecidable after unknown Lua response"
        } else {
            ""
        };
        Err(Error::scenario(format!(
            "Timed out after {timeout}s entering the game; last: {last}{uncertain}"
        )))
    }
    pub fn save_path(&self, name: &str) -> PathBuf {
        self.saves.join(format!("{name}.Civ6Save"))
    }
    pub fn save_name(&self, step: &Value) -> Result<String> {
        let name = text(step, "name");
        if step["exact_name"] == true {
            return Ok(name.to_string());
        }
        let resolved = resolve_path(&self.output)?;
        let suffix = format!(
            "{:x}",
            Sha256::digest(resolved.to_string_lossy().as_bytes())
        );
        Ok(format!("{}-{}", &name[..name.len().min(67)], &suffix[..12]))
    }
    fn checkpoint_logical(&self, step: &Value) -> Result<String> {
        if step["exact_name"] != true {
            return Err(Error::scenario("checkpoint requires exact_name: true"));
        }
        let logical = text(step, "name");
        checkpoint::manifest_path(&self.saves, logical)?;
        Ok(logical.to_owned())
    }
    fn next_checkpoint_save_name(&mut self, logical: &str) -> Result<String> {
        let suffix_len = 1 + self.checkpoint_run_id.len() + 1 + 16;
        for _ in 0..1000 {
            self.checkpoint_sequence = self
                .checkpoint_sequence
                .checked_add(1)
                .ok_or_else(|| Error::scenario("Checkpoint save sequence exhausted"))?;
            let name = format!(
                "{}-{}-{:016x}",
                &logical[..logical.len().min(80 - suffix_len)],
                self.checkpoint_run_id,
                self.checkpoint_sequence
            );
            match fs::symlink_metadata(self.save_path(&name)) {
                Ok(_) => continue,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(name),
                Err(e) => return Err(e.into()),
            }
        }
        Err(Error::scenario(
            "Could not allocate a new checkpoint save name",
        ))
    }
    pub fn check_start_turn(&mut self, deadline: f64) -> Result<()> {
        if self.ruleset.as_deref() != Some(scenario::BANSEIKI_RULESET) {
            return Ok(());
        }
        let value = self.lua(
            "InGame",
            "return tostring(GameConfiguration.GetStartTurn())",
            self.budget(deadline, 5.0)?,
        )?;
        if value.trim().trim_matches('"') != "1" {
            return Err(Error::scenario(format!("{} start turn is {value}, expected 1: the calendar override was not applied (configure without build_setup?)",scenario::BANSEIKI_RULESET)));
        }
        Ok(())
    }
    pub fn end_turn(&mut self, timeout: f64, deadline: Option<f64>) -> Result<String> {
        let deadline = deadline.unwrap_or_else(|| self.clock.now() + timeout);
        self.lua("InGame", CLOSE_NOTEBOOKS, self.budget(deadline, 5.0)?)?;
        self.close_options_menu(deadline)?;
        self.dismiss_advisor(deadline)?;
        let start = parse_integer(&self.lua(
            "InGame",
            "return Game.GetCurrentGameTurn()",
            self.budget(deadline, 5.0)?,
        )?)?;
        self.close_diplomacy(deadline)?;
        self.lua(
            "InGame",
            "UI.RequestAction(ActionTypes.ACTION_ENDTURN)\nreturn true",
            self.budget(deadline, 5.0)?,
        )?;
        // A new turn number can be published before control returns to the
        // human. Match the official ActionPanel/UnitPanel readiness checks;
        // otherwise the next scenario action can run during AI processing.
        self.wait(
            &json!({"type":"lua","state_name":"InGame","code":ready_turn_code(start+1)}),
            self.remaining(deadline)?,
            true,
        )?;
        self.dismiss_advisor(deadline)?;
        self.remaining(deadline)?;
        Ok(format!("human turn {start}->{}", start + 1))
    }
    pub fn choose_dedication(&mut self, human: i64, deadline: f64) -> Result<Option<String>> {
        let code = format!("local h = {human}\n{DEDICATION_CODE}");
        let value = self.lua("InGame", &code, self.budget(deadline, 5.0)?)?;
        if value == "none" || value.is_empty() {
            return Ok(None);
        }
        self.log(
            "autoplay_dedication",
            "OK",
            &format!("human={human} chose commemoration indices={value}"),
        )?;
        // Close through the visible context. An unconditional Esc here opens
        // Options if the polling loop already closed the dedication popup.
        self.close_diplomacy(deadline)?;
        Ok(Some(value))
    }
    pub fn run_step(&mut self, step: &Value) -> Result<String> {
        let action = text(step, "action");
        if action == "manual" {
            self.backend.manual(text(step, "prompt"))?;
            return Ok(text(step, "prompt").into());
        }
        let timeout = step["timeout"]
            .as_f64()
            .unwrap_or(if action == "restart_game" {
                360.0
            } else {
                120.0
            });
        let deadline = self.clock.now() + timeout;
        match action {
            "start_native_can_ever_trace" | "start_native_property_trace" => {
                self.remaining(deadline)?;
                let mode = if action == "start_native_property_trace" {
                    "property"
                } else {
                    "can-ever"
                };
                self.backend.start_native_bridge(
                    &self.output,
                    text(step, "name"),
                    step["max_seconds"].as_u64().unwrap(),
                    mode,
                )?;
                Ok(format!(
                    "native {mode} original-return telemetry ready; artifact {}.jsonl",
                    text(step, "name")
                ))
            }
            "stop_native_can_ever_trace" | "stop_native_property_trace" => {
                self.remaining(deadline)?;
                let artifact = self
                    .backend
                    .stop_native_bridge()?
                    .ok_or_else(|| Error::scenario("native bridge was not active"))?;
                if action == "stop_native_property_trace" {
                    Ok(format!(
                        "native property entry restored; prefix retained until game exit; raw rows collected, key and value mapping unverified; artifact {}",
                        artifact.display()
                    ))
                } else {
                    Ok(format!(
                        "native entry restored; artifact {}",
                        artifact.display()
                    ))
                }
            }
            "start_native_governor_trace" => {
                self.remaining(deadline)?;
                self.backend.start_native_trace(
                    &self.output,
                    text(step, "name"),
                    step["max_seconds"].as_u64().unwrap(),
                    step["mode"].as_str().unwrap_or("governor"),
                )?;
                Ok(format!(
                    "native governor observer ready; artifact {}.jsonl",
                    text(step, "name")
                ))
            }
            "stop_native_governor_trace" => {
                self.remaining(deadline)?;
                let artifact = self
                    .backend
                    .stop_native_trace()?
                    .ok_or_else(|| Error::scenario("native governor trace was not active"))?;
                Ok(format!(
                    "native governor observer detached; artifact {}",
                    artifact.display()
                ))
            }
            "restart_game" => self.restart_game(self.remaining(deadline)?),
            "enable_mods" => {
                let ids = strings(&step["ids"]);
                let disabled = self.lua(
                    "FrontEnd",
                    &scenario::enable_mods_code(&ids, &strings(&step["disable_ids"])),
                    // Switching the selection rebuilds configuration databases.
                    // Wait within the scenario's deadline for this one mutation;
                    // a short query budget can lose its acknowledgment mid-rebuild.
                    self.remaining(deadline)?,
                )?;
                Ok(format!(
                    "enabled/verified {}; disabled {}",
                    ids.join(", "),
                    if disabled.is_empty() {
                        "none"
                    } else {
                        &disabled
                    }
                ))
            }
            "configure" => {
                self.lua(
                    "FrontEnd",
                    &scenario::configure_code(step),
                    self.budget(deadline, 5.0)?,
                )?;
                self.ruleset = Some(text(step, "ruleset").into());
                let build = scenario::builds_setup(step);
                if build {
                    self.lua(
                        "HostGame",
                        &scenario::build_setup_code(step["players"].as_array().unwrap()),
                        self.budget(deadline, 30.0)?,
                    )?;
                }
                self.lua("FrontEnd",&format!("GameConfiguration.SetValue('JAC_AUTOTEST', {})\nGameConfiguration.UpdateEnabledMods()\nreturn true",i32::from(self.test_mode)),self.budget(deadline,5.0)?)?;
                Ok(format!(
                    "{}, {}, {}, seed={}, players={}{}",
                    text(step, "ruleset"),
                    text(step, "map_size"),
                    text(step, "speed"),
                    step["seed"],
                    step["players"].as_array().unwrap().len(),
                    if build {
                        "; Create Game setup built and checked"
                    } else {
                        ""
                    }
                ))
            }
            "start_game" => {
                let before = self.loadscreen_count()?;
                if self.owner_start {
                    self.lua("FrontEnd","if not UI.IsInFrontEnd() then error('main menu required') end\nUIManager:QueuePopup(ContextPtr:LookUpControl('/FrontEnd/MainMenu/AdvancedSetup'), PopupPriority.Current)\nreturn true",self.budget(deadline,5.0)?)?;
                    println!("OWNER ACTION: press Start Game on the Create Game screen");
                    let wait = timeout.max(3600.0);
                    let owner_deadline = self.clock.now() + wait;
                    self.enter_game_after_loadscreen(before, wait, Some(owner_deadline), false)?;
                    self.check_start_turn(owner_deadline)?;
                    return Ok(
                        "owner pressed Start Game; single-player game reached InGame".into(),
                    );
                }
                let unknown=match self.lua("FrontEnd","if not UI.IsInFrontEnd() then error('main menu required') end\nNetwork.HostGame(ServerType.SERVER_TYPE_NONE)\nreturn true",self.budget(deadline,5.0)?) {Ok(_)=>false,Err(e) if e.kind==ErrorKind::LuaResponseUnknown=>{println!("HostGame response interrupted during transition: {e}");true},Err(e)=>return Err(e)};
                self.enter_game_after_loadscreen(before, timeout, Some(deadline), unknown)?;
                self.check_start_turn(deadline)?;
                Ok("single-player game reached InGame".into())
            }
            "autoplay" => {
                let options = AutoplayOptions {
                    deadline: Some(deadline),
                    record_metrics: step["metrics"] == true,
                    research_metrics: step["research_metrics"] == true,
                    allow_human_elimination: step["allow_human_elimination"] == true,
                    metrics_group: step["metrics_group"].as_str().unwrap_or("AUTOPLAY").into(),
                    start_turn_override: None,
                };
                let turns = step["turns"]
                    .as_i64()
                    .ok_or_else(|| Error::scenario("turns must be positive"))?;
                if step["fast_forward"] == true {
                    self.fast_forward_autoplay(turns, timeout, options)
                } else {
                    self.autoplay(turns, timeout, options)
                }
            }
            "end_turn" => {
                let turns = if self.test_mode {
                    step.get("test_turns")
                        .unwrap_or_else(|| step.get("turns").unwrap_or(&Value::Null))
                } else {
                    step.get("turns").unwrap_or(&Value::Null)
                };
                let turns = if turns.is_null() {
                    1
                } else {
                    turns
                        .as_i64()
                        .filter(|n| *n > 0)
                        .ok_or_else(|| Error::scenario("turns must be positive"))?
                };
                let mut results = vec![];
                for _ in 0..turns {
                    results.push(self.end_turn(self.remaining(deadline)?, Some(deadline))?);
                }
                Ok(results.join("; "))
            }
            "ui_lua" => {
                let mut attempts = 0;
                let lua_timeout = step["lua_timeout"].as_f64().unwrap_or(5.0);
                let value = loop {
                    match self.lua(
                        text(step, "state_name"),
                        text(step, "code"),
                        self.budget(deadline, lua_timeout)?,
                    ) {
                        Ok(v) => break v,
                        Err(e)
                            if matches!(
                                e.kind,
                                ErrorKind::LuaResponseUnknown | ErrorKind::LuaExecution
                            ) =>
                        {
                            return Err(e)
                        }
                        Err(e) if retryable(&e) => {
                            if !self.unblock_ingame(
                                text(step, "state_name"),
                                deadline,
                                attempts,
                                step["preserve_diplomacy_session"] == true,
                            )? {
                                return Err(e);
                            }
                            attempts += 1;
                        }
                        Err(e) => return Err(e),
                    }
                };
                if let Some(key) = step["equals_capture"].as_str() {
                    if self.lua_captures.get(key) != Some(&value) {
                        return Err(Error::scenario(format!(
                            "ui_lua snapshot mismatch for {key}: {value:?} != {:?}",
                            self.lua_captures.get(key)
                        )));
                    }
                }
                if let Some(key) = step["capture_as"].as_str() {
                    self.lua_captures.insert(key.into(), value.clone());
                }
                Ok(format!(
                    "ran Lua in {}; return={value}",
                    text(step, "state_name")
                ))
            }
            "screenshot" => {
                self.backend.require_single_process()?;
                let base = resolve_path(&self.output)?;
                let target =
                    resolve_path(&base.join(scenario::safe_relative_path(text(step, "path"))?))?;
                if !target.starts_with(&base) {
                    return Err(Error::scenario(format!(
                        "Screenshot path escapes output directory: {:?}",
                        text(step, "path")
                    )));
                }
                self.close_options_menu(deadline)?;
                self.dismiss_advisor(deadline)?;
                self.remaining(deadline)?;
                let crop = step["crop"].as_array().map(|a| {
                    [
                        a[0].as_f64().unwrap(),
                        a[1].as_f64().unwrap(),
                        a[2].as_f64().unwrap(),
                        a[3].as_f64().unwrap(),
                    ]
                });
                self.backend.capture(&target, crop, deadline)?;
                self.remaining(deadline)?;
                Ok(target.display().to_string())
            }
            "save" => self.save_step(step, deadline),
            "load" => self.load_step(step, timeout, deadline),
            "exit_to_menu" => {
                self.lua(
                    "InGame",
                    "Events.ExitToMainMenu()\nreturn true",
                    self.budget(deadline, 5.0)?,
                )?;
                self.disconnect();
                self.wait(
                    &json!({"type":"state","name":"FrontEnd"}),
                    self.remaining(deadline)?,
                    false,
                )?;
                self.wait(&json!({"type":"lua","state_name":"FrontEnd","code":"return UI.IsInFrontEnd()"}),self.remaining(deadline)?,false)?;
                Ok("main menu reached".into())
            }
            "wait_for" => {
                self.wait(&step["condition"], self.remaining(deadline)?, false)?;
                Ok(format!("condition met: {}", step["condition"]))
            }
            "observe_captive_session" => self.observe_captive_session(step, deadline),
            "pause" => {
                let seconds = step["seconds"].as_f64().unwrap();
                if seconds > self.remaining(deadline)? {
                    return Err(Error::scenario("Step deadline would expire during pause"));
                }
                self.clock.sleep(seconds);
                Ok(format!("paused {seconds}s for UI animation"))
            }
            _ => Err(Error::scenario(format!("Unknown action: {action}"))),
        }
    }
    fn save_step(&mut self, step: &Value, deadline: f64) -> Result<String> {
        let logical = if step["checkpoint"] == true {
            Some(self.checkpoint_logical(step)?)
        } else {
            None
        };
        let name = if let Some(logical) = &logical {
            self.next_checkpoint_save_name(logical)?
        } else {
            self.save_name(step)?
        };
        let target = self.save_path(&name);
        if target.exists() {
            return Err(Error::scenario(format!(
                "Save already exists (will not overwrite): {}",
                target.display()
            )));
        }
        self.close_options_menu(deadline)?;
        self.dismiss_advisor(deadline)?;
        let code=format!("JAC_AUTOTEST_SAVE_DONE = false\nif JAC_AUTOTEST_SAVE_HANDLER then Events.SaveComplete.Remove(JAC_AUTOTEST_SAVE_HANDLER) end\nJAC_AUTOTEST_SAVE_HANDLER = function() JAC_AUTOTEST_SAVE_DONE = true end\nEvents.SaveComplete.Add(JAC_AUTOTEST_SAVE_HANDLER)\nlocal result = Network.SaveGame({{Name={}, Location=SaveLocations.LOCAL_STORAGE, Type=Network.GetGameConfigurationSaveType(), IsAutosave=false, IsQuicksave=false}})\nif result == false then error('Network.SaveGame returned false') end\nreturn true",scenario::lua_string(&name));
        let recorded = step["metrics"] == true;
        let saved_turn = if recorded {
            parse_integer(&self.lua(
                "InGame",
                "return tostring(Game.GetCurrentGameTurn())",
                self.budget(deadline, 5.0)?,
            )?)?
        } else {
            0
        };
        let started = self.clock.now();
        self.lua("InGame", &code, self.budget(deadline, 30.0)?)?;
        self.wait(&json!({"type":"lua","state_name":"InGame","code":"return JAC_AUTOTEST_SAVE_DONE == true"}),self.remaining(deadline)?,false)?;
        self.lua("InGame","Events.SaveComplete.Remove(JAC_AUTOTEST_SAVE_HANDLER)\nJAC_AUTOTEST_SAVE_HANDLER = nil\nreturn true",self.budget(deadline,5.0)?)?;
        let until = deadline.min(self.clock.now() + 5.0);
        while !target.is_file() && self.clock.now() < until {
            self.sleep_until(until, 0.2);
        }
        if !target.is_file() || target.metadata()?.len() == 0 {
            return Err(Error::scenario(format!(
                "SaveComplete fired but save not found at {}; check --saves",
                target.display()
            )));
        }
        self.remaining(deadline)?;
        if let Some(logical) = &logical {
            checkpoint::publish(&self.saves, logical, &name)?;
        }
        if recorded {
            self.save_load_metrics.set_test_mode(self.test_mode);
            let row = self.save_load_metrics.record(
                "save",
                &name,
                saved_turn,
                target.metadata()?.len(),
                self.clock.now() - started,
            )?;
            return Ok(format!(
                "{}; save_bytes={} save_seconds={}",
                target.display(),
                row["save_bytes"],
                row["duration_seconds"]
            ));
        }
        Ok(target.display().to_string())
    }
    fn load_step(&mut self, step: &Value, timeout: f64, deadline: f64) -> Result<String> {
        let name = if step["checkpoint"] == true {
            checkpoint::resolve(&self.saves, &self.checkpoint_logical(step)?)?
        } else {
            self.save_name(step)?
        };
        let path = self.save_path(&name);
        if !path.is_file() {
            return Err(Error::scenario(format!(
                "Save not found: {}",
                path.display()
            )));
        }
        let code=format!("if not UI.IsInFrontEnd() then error('main menu required') end\nlocal ok = Network.LoadGame({{Name={}, Location=SaveLocations.LOCAL_STORAGE, Type=SaveTypes.SINGLE_PLAYER, Directory=SaveDirectories.DEFAULT, IsAutosave=false, IsQuicksave=false}}, ServerType.SERVER_TYPE_NONE)\nif ok == false then error('Network.LoadGame returned false') end\nreturn true",scenario::lua_string(&name));
        let before = self.loadscreen_count()?;
        let started = self.clock.now();
        let unknown = match self.lua("FrontEnd", &code, self.budget(deadline, 5.0)?) {
            Ok(_) => false,
            Err(e) if e.kind == ErrorKind::LuaResponseUnknown => {
                println!("LoadGame response interrupted during transition: {e}");
                true
            }
            Err(e) => return Err(e),
        };
        self.enter_game_after_loadscreen(before, timeout, Some(deadline), unknown)?;
        if step["metrics"] == true {
            let seconds = self.clock.now() - started;
            let turn = parse_integer(&self.lua(
                "InGame",
                "return tostring(Game.GetCurrentGameTurn())",
                self.budget(deadline, 5.0)?,
            )?)?;
            self.save_load_metrics.set_test_mode(self.test_mode);
            let row = self.save_load_metrics.record(
                "load",
                &name,
                turn,
                path.metadata()?.len(),
                seconds,
            )?;
            return Ok(format!(
                "loaded {name}; save_bytes={} load_seconds={}",
                row["save_bytes"], row["duration_seconds"]
            ));
        }
        Ok(format!("loaded {name}"))
    }
}
pub fn resolve_path(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut missing = vec![];
    let mut existing = absolute.as_path();
    while !existing.exists() {
        let name = existing
            .file_name()
            .ok_or_else(|| Error::scenario(format!("Cannot resolve path: {}", path.display())))?;
        missing.push(name.to_os_string());
        existing = existing
            .parent()
            .ok_or_else(|| Error::scenario("Path has no parent"))?;
    }
    let mut resolved = fs::canonicalize(existing)?;
    for component in missing.iter().rev() {
        resolved.push(component);
    }
    #[cfg(windows)]
    {
        let s = resolved.to_string_lossy();
        if let Some(unc) = s.strip_prefix("\\\\?\\UNC\\") {
            return Ok(PathBuf::from(format!("\\\\{unc}")));
        }
        if let Some(plain) = s.strip_prefix("\\\\?\\") {
            return Ok(PathBuf::from(plain));
        }
    }
    Ok(resolved)
}
fn parse_integer(value: &str) -> Result<i64> {
    value
        .parse()
        .map_err(|_| Error::scenario(format!("Invalid integer: {value:?}")))
}
fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}
fn python_value_string(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => s.clone(),
        Value::Number(n) if n.is_f64() => {
            let value = n.as_f64().unwrap();
            if value != 0.0 && (value.abs() < 1e-4 || value.abs() >= 1e16) {
                let scientific = format!("{value:e}");
                let (mantissa, exponent) = scientific.split_once('e').unwrap();
                format!("{mantissa}e{:+03}", exponent.parse::<i32>().unwrap())
            } else {
                let mut text = value.to_string();
                if !text.contains('.') {
                    text.push_str(".0");
                }
                text
            }
        }
        Value::Number(n) => n.to_string(),
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(python_value_repr)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(items) => format!(
            "{{{}}}",
            items
                .iter()
                .map(|(k, v)| format!(
                    "{}: {}",
                    python_value_repr(&Value::String(k.clone())),
                    python_value_repr(v)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}
fn python_value_repr(v: &Value) -> String {
    if let Value::String(s) = v {
        let quote = if s.contains('\'') && !s.contains('"') {
            '"'
        } else {
            '\''
        };
        format!(
            "{quote}{}{quote}",
            s.replace('\\', "\\\\")
                .replace(quote, &format!("\\{quote}"))
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t")
        )
    } else {
        python_value_string(v)
    }
}
const CLOSE_NOTEBOOKS: &str =
    "if LuaEvents.JAC_Notebooks_Close then LuaEvents.JAC_Notebooks_Close() end\nreturn true";
const DEDICATION_CODE:&str="if not (Game.GetEras and PlayerOperations and PlayerOperations.COMMEMORATE) then return 'none' end\nlocal eras = Game.GetEras()\nlocal n = eras:GetPlayerNumAllowedCommemorations(h)\nlocal choices = eras:GetPlayerCommemorateChoices(h)\nif n <= 0 or #choices == 0 then return 'none' end\nlocal picked = {}\nfor i = 1, math.min(n, #choices) do\n  local p = {}\n  p[PlayerOperations.PARAM_COMMEMORATION_TYPE] = choices[i]\n  UI.RequestPlayerOperation(h, PlayerOperations.COMMEMORATE, p)\n  table.insert(picked, tostring(choices[i]))\nend\nreturn table.concat(picked, ',')";

impl Runner {
    pub fn autoplay(
        &mut self,
        turns: i64,
        timeout: f64,
        options: AutoplayOptions,
    ) -> Result<String> {
        let deadline = options
            .deadline
            .unwrap_or_else(|| self.clock.now() + timeout);
        self.lua("InGame", CLOSE_NOTEBOOKS, self.budget(deadline, 5.0)?)?;
        self.close_options_menu(deadline)?;
        self.dismiss_advisor(deadline)?;
        let core_start = if options.start_turn_override.is_some() {
            options.start_turn_override
        } else if self
            .scenario
            .get("test_mode")
            .is_some_and(|v| v.is_object())
        {
            Some(parse_integer(&self.lua(
                "GameCore_Tuner",
                "return Game.GetCurrentGameTurn()",
                self.budget(deadline, 5.0)?,
            )?)?)
        } else {
            None
        };
        let start_line = core_start
            .map(|v| format!("local startTurn = {v}\n"))
            .unwrap_or_else(|| "local startTurn = Game.GetCurrentGameTurn()\n".into());
        let start_code=format!("if AutoplayManager.IsActive() then error('autoplay already active') end\n{start_line}local human = Game.GetLocalPlayer()\nif human < 0 then error('no local human player to return to') end\nif JAC_AUTOTEST_AUTO_HANDLER then LuaEvents.AutoPlayEnd.Remove(JAC_AUTOTEST_AUTO_HANDLER) end\nJAC_AUTOTEST_AUTO_DONE = false\nJAC_AUTOTEST_AUTO_HANDLER = function() JAC_AUTOTEST_AUTO_DONE = true end\nLuaEvents.AutoPlayEnd.Add(JAC_AUTOTEST_AUTO_HANDLER)\nAutoplayManager.SetTurns({turns})\nAutoplayManager.SetReturnAsPlayer(human)\nAutoplayManager.SetObserveAsPlayer(PlayerTypes.NONE)\nAutoplayManager.SetActive(true)\nreturn tostring(startTurn) .. '|' .. tostring(human)");
        let mut autoplay_started = false;
        let execution = (|| -> Result<(i64, AutoplayStatus, Option<i64>, Option<usize>, bool)> {
            let (pid, initial_memory, started) = if options.record_metrics {
                let process = self.backend.single_process(30.0)?.ok_or_else(|| {
                    Error::scenario("Civ VI process disappeared before metrics sampling")
                })?;
                (
                    Some(process.0),
                    self.backend.memory(process.0)?,
                    self.clock.now(),
                )
            } else {
                (None, Value::Null, 0.0)
            };
            let start_value = self.lua("InGame", &start_code, self.budget(deadline, 5.0)?)?;
            autoplay_started = true;
            let parts: Vec<_> = start_value.split('|').collect();
            if parts.len() != 2 {
                return Err(Error::scenario(format!(
                    "Unexpected autoplay start: {start_value:?}"
                )));
            }
            let start_turn = core_start.unwrap_or(parse_integer(parts[0])?);
            let human = parse_integer(parts[1])?;
            let target = start_turn
                .checked_add(turns)
                .ok_or_else(|| Error::scenario("Autoplay target overflow"))?;
            if options.record_metrics || options.research_metrics {
                self.metrics_runs += 1;
            }
            let run_number = self.metrics_runs;
            let recorder_index = if let Some(pid) = pid {
                let mut recorder = metrics::TurnMetrics::new(
                    self.output
                        .join(format!("autoplay-metrics-{run_number}.jsonl")),
                    start_turn,
                    started,
                    self.scenario.get("test_mode").is_some(),
                );
                recorder.start(initial_memory)?;
                let index = self.turn_metrics.len();
                self.turn_metrics.push(MetricsRun {
                    recorder,
                    pid,
                    requested_turns: turns,
                    group: options.metrics_group.clone(),
                });
                Some(index)
            } else {
                None
            };
            let mut research = if options.research_metrics {
                let mut recorder = metrics::ResearchMetrics::new(
                    self.output
                        .join(format!("autoplay-research-{run_number}.jsonl")),
                );
                let sample =
                    self.research_query(metrics::RESEARCH_SAMPLE_CODE, deadline, Some(start_turn))?;
                recorder.sample(&sample)?;
                Some(recorder)
            } else {
                None
            };
            let mut restarts = 0;
            let mut dedications = 0;
            let mut inactive_since = None;
            let mut status: Option<AutoplayStatus> = None;
            let mut elimination = None;
            let mut last_alive = start_turn;
            let mut last_alive_check = None;
            let mut diplomacy_closes = 0;
            let mut diplomacy_since_resume = false;
            let mut status_failure: Option<Error> = None;
            while self.clock.now() < deadline {
                let closed = self.close_diplomacy(deadline)?;
                if !closed.is_empty() {
                    diplomacy_closes += closed.len();
                    diplomacy_since_resume = true;
                    self.log(
                        "autoplay_diplomacy",
                        "OK",
                        &format!(
                            "closed {} (turn {}, total {diplomacy_closes})",
                            closed.join(", "),
                            status.as_ref().map(|s| s.turn).unwrap_or(start_turn)
                        ),
                    )?;
                    if diplomacy_closes as i64 > 20 + 5 * turns {
                        return Err(Error::scenario(format!("Closed {diplomacy_closes} AI diplomacy screens in one autoplay step; the game keeps reopening them")));
                    }
                } else if let Some(error) = status_failure.take() {
                    return Err(error);
                }
                status_failure = None;
                let current = match self.autoplay_status(deadline) {
                    Ok(v) => v,
                    Err(e) if e.kind == ErrorKind::LuaExecution => return Err(e),
                    Err(e) if retryable(&e) => {
                        status_failure = Some(e);
                        continue;
                    }
                    Err(e) => return Err(e),
                };
                status = Some(current.clone());
                if core_start.is_some() && current.turn < start_turn {
                    self.clock.sleep(self.budget(deadline, 0.1)?);
                    continue;
                }
                if let Some(index) = recorder_index {
                    if current.turn != self.turn_metrics[index].recorder.turn {
                        if self.backend.single_process(30.0)?.map(|v| v.0) != pid {
                            return Err(Error::scenario(
                                "Civ VI process changed during metrics sampling",
                            ));
                        }
                        let memory = self.backend.memory(pid.unwrap())?;
                        let now = self.clock.now();
                        self.turn_metrics[index]
                            .recorder
                            .sample_at(current.turn, memory, now)?;
                    }
                }
                if let Some(recorder) = &mut research {
                    if recorder.last_turn != Some(current.turn) {
                        let code = metrics::research_sample_code(
                            recorder
                                .slots
                                .as_ref()
                                .ok_or_else(|| Error::scenario("No research slots"))?,
                        )?;
                        let sample = self.research_query(&code, deadline, Some(current.turn))?;
                        recorder.sample(&sample)?;
                    }
                }
                self.remaining(deadline)?;
                if elimination.is_none()
                    && (last_alive_check != Some(current.turn) || current.local_player < 0)
                {
                    let alive = if let Some(recorder) = research
                        .as_ref()
                        .filter(|r| r.last_turn == Some(current.turn))
                    {
                        recorder
                            .rows
                            .iter()
                            .find(|r| r["slot"] == human)
                            .ok_or_else(|| {
                                Error::scenario(format!(
                                    "Human player {human} missing from research sample"
                                ))
                            })?["alive"]
                            .as_bool()
                            .unwrap()
                    } else {
                        let value = self.lua(
                            "InGame",
                            &format!("return tostring(Players[{human}]:IsAlive())"),
                            self.budget(deadline, 5.0)?,
                        )?;
                        if !["true", "false"].contains(&value.as_str()) {
                            return Err(Error::scenario(format!(
                                "Unexpected player alive status: {value:?}"
                            )));
                        }
                        value == "true"
                    };
                    last_alive_check = Some(current.turn);
                    if alive {
                        last_alive = current.turn;
                    } else {
                        elimination = Some(current.turn);
                        self.log(
                            "autoplay_elimination",
                            "OK",
                            &format!(
                                "human={human} first_observed_turn={} last_alive_turn={last_alive}",
                                current.turn
                            ),
                        )?;
                        if !options.allow_human_elimination {
                            return Err(Error::scenario(format!("Human player {human} was eliminated during autoplay (first observed at turn {}, target {target})",current.turn)));
                        }
                    }
                }
                if current.turn > target {
                    return Err(Error::scenario(format!(
                        "Autoplay overshot turn {target}: {current:?}"
                    )));
                }
                let returned = current.local_player == human
                    || (options.allow_human_elimination
                        && elimination.is_some()
                        && current.local_player < 0);
                if current.event && current.turn == target && !current.active && returned {
                    return Ok((
                        start_turn,
                        current,
                        elimination,
                        recorder_index,
                        research.is_some(),
                    ));
                }
                self.close_options_menu(deadline)?;
                self.dismiss_advisor(deadline)?;
                if !current.active && current.turn < target {
                    if let Some(since) = inactive_since {
                        if self.clock.now() - since >= 5.0 && returned {
                            let chosen = if dedications < 3 {
                                self.choose_dedication(human, deadline)?
                            } else {
                                None
                            };
                            if chosen.is_some() {
                                dedications += 1;
                            } else if restarts >= 2 && !diplomacy_since_resume {
                                return Err(Error::scenario(format!(
                                    "Autoplay stopped early at turn {}/{target}: {current:?}",
                                    current.turn
                                )));
                            }
                            let remaining = target - current.turn;
                            self.lua("InGame",&format!("JAC_AUTOTEST_AUTO_DONE = false\nAutoplayManager.SetTurns({remaining})\nAutoplayManager.SetReturnAsPlayer({human})\nAutoplayManager.SetObserveAsPlayer(PlayerTypes.NONE)\nAutoplayManager.SetActive(true)\nreturn true"),self.budget(deadline,5.0)?)?;
                            if chosen.is_none() && !diplomacy_since_resume {
                                restarts += 1;
                            } else if diplomacy_since_resume {
                                self.log("autoplay_diplomacy","OK",&format!("resumed autoplay for {remaining} turns after closing diplomacy"))?;
                            }
                            diplomacy_since_resume = false;
                            inactive_since = None;
                        }
                    } else {
                        inactive_since = Some(self.clock.now());
                    }
                } else {
                    inactive_since = None;
                }
                self.sleep_until(
                    deadline,
                    if recorder_index.is_some() || research.is_some() {
                        0.1
                    } else {
                        0.5
                    },
                );
            }
            Err(Error::scenario(format!(
                "Autoplay timed out at turn {} / {target}; last: {status:?}",
                status
                    .as_ref()
                    .map(|v| v.turn.to_string())
                    .unwrap_or_else(|| "?".into())
            )))
        })();
        let execution = execution.and_then(|value| {
            let mut attempts = 0;
            loop {
                match self.lua("InGame","if JAC_AUTOTEST_AUTO_HANDLER then LuaEvents.AutoPlayEnd.Remove(JAC_AUTOTEST_AUTO_HANDLER) end\nJAC_AUTOTEST_AUTO_HANDLER = nil\nreturn true",self.budget(deadline,5.0)?) {
                    Ok(_) => break,
                    Err(e) if e.kind == ErrorKind::LuaExecution => return Err(e),
                    Err(e) if retryable(&e) => {
                        if !self.unblock_ingame("InGame", deadline, attempts, false)? { return Err(e); }
                        attempts += 1;
                    }
                    Err(e) => return Err(e),
                }
            }
            self.remaining(deadline)?;
            Ok(value)
        });
        let (start_turn, status, elimination, recorder_index, research) = match execution {
            Ok(v) => v,
            Err(e)
                if e.kind == ErrorKind::Interrupted || crate::time::check_interrupt().is_err() =>
            {
                if autoplay_started {
                    if let Err(cleanup) =
                        crate::time::with_interrupt_cleanup(|| self.stop_autoplay())
                    {
                        println!("Autoplay cleanup failed: {cleanup}");
                    }
                }
                return Err(if e.kind == ErrorKind::Interrupted {
                    e
                } else {
                    Error::interrupted(format!("Interrupted while autoplay failed: {e}"))
                });
            }
            Err(e)
                if matches!(
                    e.kind,
                    ErrorKind::LuaExecution | ErrorKind::LuaResponseUnknown
                ) =>
            {
                return Err(e)
            }
            Err(e) => {
                if let Err(cleanup) = self.stop_autoplay() {
                    println!("Autoplay cleanup failed: {cleanup}");
                }
                return Err(e);
            }
        };
        let mut detail = format!(
            "autoplay completed {turns} turns ({start_turn}->{}; event={})",
            status.turn,
            if status.event { "True" } else { "False" }
        );
        if let Some(turn) = elimination {
            detail.push_str(&format!("; human_elimination_first_observed_turn={turn}"));
        }
        if let Some(index) = recorder_index {
            let recorder = &self.turn_metrics[index].recorder;
            detail.push_str(&format!(
                "; metrics=autoplay-metrics-{}.jsonl,autoplay-metrics-{}.csv observed={} missed={}",
                self.metrics_runs,
                self.metrics_runs,
                recorder.observed_turns,
                recorder.missed_turns
            ));
        }
        if research {
            detail.push_str(&format!(
                "; research=autoplay-research-{}.jsonl",
                self.metrics_runs
            ));
        }
        Ok(detail)
    }
    fn stop_autoplay(&mut self) -> Result<String> {
        self.lua("InGame", "if AutoplayManager.IsActive() then AutoplayManager.SetActive(false) end\nif JAC_AUTOTEST_AUTO_HANDLER then LuaEvents.AutoPlayEnd.Remove(JAC_AUTOTEST_AUTO_HANDLER) end\nJAC_AUTOTEST_AUTO_HANDLER = nil\nreturn true", 5.0)
    }
    fn era_state(&mut self, deadline: f64) -> Result<(i64, i64, i64, i64)> {
        let value=self.lua("InGame","local eras = Game.GetEras()\nreturn tostring(Game.GetCurrentGameTurn()) .. '|' .. tostring(eras:GetCurrentEraMinimumEndTurn()) .. '|' .. tostring(eras:GetCurrentEraMaximumEndTurn()) .. '|' .. tostring(eras:GetNextEraCountdown())",self.budget(deadline,5.0)?)?;
        let values: Vec<_> = value.split('|').map(str::parse::<i64>).collect();
        if values.len() != 4 || values.iter().any(|v| v.is_err()) {
            return Err(Error::scenario(format!(
                "Invalid fast-forward era state: {value:?}"
            )));
        }
        Ok((
            values[0].as_ref().copied().unwrap(),
            values[1].as_ref().copied().unwrap(),
            values[2].as_ref().copied().unwrap(),
            values[3].as_ref().copied().unwrap(),
        ))
    }
    pub fn fast_forward_autoplay(
        &mut self,
        turns: i64,
        timeout: f64,
        options: AutoplayOptions,
    ) -> Result<String> {
        scenario::validate_test_mode(&self.scenario["test_mode"])?;
        let deadline = options
            .deadline
            .unwrap_or_else(|| self.clock.now() + timeout);
        self.lua("InGame","if GameConfiguration.GetRuleSet() ~= 'RULESET_JAC_BANSEIKI' then error('test ruleset required') end\nif GameConfiguration.IsAnyMultiplayer() then error('single-player test required') end\nreturn true",self.budget(deadline,5.0)?)?;
        let (mut current, mut minimum, mut maximum, mut countdown) = self.era_state(deadline)?;
        let target = current
            .checked_add(turns)
            .ok_or_else(|| Error::scenario("Fast-forward target overflow"))?;
        let mut skipped = 0;
        while current < target {
            let boundary = if countdown > 0 {
                current + countdown
            } else {
                minimum
            };
            let limit = if maximum < 0 {
                target - 2
            } else {
                (target - 2).min(boundary - 2)
            };
            if limit > current {
                let code=format!("local ok, setter = pcall(function() return Game.SetCurrentGameTurn end)\nif not ok or type(setter) ~= 'function' then error('engine fast-forward unavailable') end\nif Game.GetCurrentGameTurn() ~= {current} then error('turn changed before fast-forward') end\nlocal target = {limit}\nGame.SetCurrentGameTurn(target)\nif Game.GetCurrentGameTurn() ~= target then error('engine fast-forward failed') end\nreturn tostring(target)");
                if let Err(e) = self.lua("GameCore_Tuner", &code, self.budget(deadline, 5.0)?) {
                    if e.kind == ErrorKind::LuaResponseUnknown {
                        let mode = &mut self.scenario["test_mode"];
                        mode["acknowledged_skipped_turns"] = mode["skipped_turns"].clone();
                        mode["skipped_turns"] = Value::Null;
                        mode["unconfirmed_jump"] = json!({"from":current,"to":limit});
                    }
                    return Err(e);
                }
                skipped += limit - current;
                let total = self.scenario["test_mode"]["skipped_turns"]
                    .as_i64()
                    .unwrap();
                self.scenario["test_mode"]["skipped_turns"] = (total + limit - current).into();
                current = limit;
            }
            let before = current;
            let mut suboptions = options.clone();
            suboptions.deadline = Some(deadline);
            suboptions.start_turn_override = Some(current);
            self.autoplay(2.min(target - current), timeout, suboptions)?;
            (current, minimum, maximum, countdown) = self.era_state(deadline)?;
            if current <= before || current > target {
                return Err(Error::scenario(format!("Fast-forward autoplay advanced unexpectedly: {before}->{current}, target={target}")));
            }
        }
        Ok(format!("TEST MODE: reached turn {target}; skipped {skipped} turns; era boundaries processed normally"))
    }
    pub fn run(&mut self) -> Result<i32> {
        scenario::validate_scenario(&self.scenario)?;
        // create_dir rejects an existing leaf while create_dir_all only prepares parents.
        if let Some(parent) = self.output.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir(&self.output)?;
        self.log("start", "OK", &text(&self.scenario, "name").to_string())?;
        let mut interruption = None;
        let mut code = match self.run_inner() {
            Ok(v) => v,
            Err(e) if e.kind == ErrorKind::Interrupted => {
                self.result_passed = false;
                self.failure = Some(format!("KeyboardInterrupt: {e}"));
                interruption = Some(e);
                130
            }
            Err(e) => {
                self.result_passed = false;
                self.log("run", "FAIL", &e.to_string())?;
                1
            }
        };
        // Also runs on a failed step. Session::Drop is the last fallback when
        // an early return or unwind skips this result path.
        if let Err(e) = self.backend.stop_native_trace() {
            code = 1;
            self.result_passed = false;
            self.failure = Some(format!("native governor observer cleanup: {e}"));
            println!("Native governor observer cleanup failed: {e}");
        }
        if let Err(e) = self.backend.stop_native_bridge() {
            code = 1;
            self.result_passed = false;
            self.failure = Some(format!("native CanEver bridge cleanup: {e}"));
            println!("Native CanEver bridge cleanup failed: {e}");
        }
        if !self.result_passed && self.failure.is_none() {
            self.failure = Some("Run did not finish".into());
        }
        if let Some(baseline) = &self.baseline {
            match judge::read_since(&self.logs, baseline) {
                Ok(logs) => {
                    for (name, content) in logs {
                        if let Err(e) = fs::write(self.output.join(name), content) {
                            println!("Could not save run log slices: {e}");
                        }
                    }
                }
                Err(e) => println!("Could not save run log slices: {e}"),
            }
        }
        self.disconnect();
        if let Err(e) = self.write_result() {
            println!("Could not save result summary: {e}");
            if self.result_passed {
                return Err(e);
            }
        }
        if let Some(e) = interruption {
            Err(e)
        } else {
            Ok(code)
        }
    }
    fn run_inner(&mut self) -> Result<i32> {
        let mut frontend_ready = false;
        if self.auto_restart {
            let folders = self.backend.mod_folders(&self.scenario, &self.mods_dir)?;
            let stale = self.backend.stale_art(&folders)?;
            if !stale.is_empty() {
                return Err(Error::scenario(format!("Icon atlas images missing from the installed mods' BLP packages (rebuild the installed mod's art with its Rust build tool): {}",stale.join(", "))));
            }
            let newest = self.backend.newest_mod(&folders)?;
            let process = self.backend.single_process(30.0)?;
            if let Some(reason) =
                crate::restart::restart_reason(newest.as_ref(), process.map(|v| v.1))
            {
                self.log("auto_restart", "START", &reason)?;
                let detail = self.restart_game(360.0)?;
                let folders = self.backend.mod_folders(&self.scenario, &self.mods_dir)?;
                let latest = self.backend.newest_mod(&folders)?;
                let relaunched = self.backend.single_process(30.0)?;
                if let Some(stale) =
                    crate::restart::restart_reason(latest.as_ref(), relaunched.map(|v| v.1))
                {
                    return Err(Error::scenario(format!(
                        "Civ VI is still stale after restart: {stale}"
                    )));
                }
                self.log("auto_restart", "OK", &detail)?;
                frontend_ready = true;
            } else {
                self.log(
                    "auto_restart",
                    "SKIP",
                    "Civ VI process is newer than all scenario mod files",
                )?;
            }
        }
        self.baseline = Some(judge::snapshot_logs(&self.logs)?);
        self.write_baseline()?;
        let steps = self.scenario["steps"].as_array().unwrap().clone();
        let first = &steps[0];
        if first["action"] != "restart_game" {
            if !frontend_ready && self.backend.single_process(30.0)?.is_none() {
                self.log("launch", "START", "Civ VI is not running")?;
                let detail = self.restart_game(360.0)?;
                self.log("launch", "OK", &detail)?;
                frontend_ready = true;
            }
            self.backend.require_single_process()?;
            if !frontend_ready && !(first["action"] == "ui_lua" && first["state_name"] == "InGame")
            {
                self.wait(&json!({"type":"lua","state_name":"FrontEnd","code":"return UI.IsInFrontEnd()"}),10.0,false)?;
            }
        }
        for (i, step) in steps.iter().enumerate() {
            let action = format!("{}/{} {}", i + 1, steps.len(), text(step, "action"));
            self.log(&action, "START", &serde_json::to_string(step)?)?;
            let result = (|| -> Result<String> {
                if step["action"] != "wait_for" {
                    self.log_step_baseline = Some(judge::snapshot_logs(&self.logs)?);
                    self.log_matches_consumed.clear();
                }
                let detail = self.run_step(step)?;
                if step["action"] == "restart_game" {
                    self.baseline = Some(judge::snapshot_logs(&self.logs)?);
                    self.write_baseline()?;
                    self.log_step_baseline = None;
                    self.log_matches_consumed.clear();
                }
                Ok(detail)
            })();
            match result {
                Ok(detail) => {
                    self.log(&action, "OK", &detail)?;
                    self.completed_steps += 1;
                }
                Err(e) => {
                    if e.kind == ErrorKind::Interrupted {
                        return Err(e);
                    }
                    self.log(&action, "FAIL", &e.to_string())?;
                    return Ok(if e.kind == ErrorKind::LiveLease {
                        75
                    } else {
                        1
                    });
                }
            }
        }
        let logs = judge::read_since(&self.logs, self.baseline.as_ref().unwrap())?;
        let checks = judge::judge(
            &logs,
            self.scenario["judge_profile"].as_str().unwrap_or("generic"),
            self.scenario["expected_units"].as_u64().map(|n| n as usize),
            self.scenario["strict_lua_errors"] == true,
        );
        judge::print_table(&checks);
        let passed = checks.iter().filter(|c| c.passed).count();
        self.result_passed = passed == checks.len();
        self.result_checks = checks;
        self.log(
            "judge",
            if self.result_passed { "OK" } else { "FAIL" },
            &format!("{passed}/{} checks passed", self.result_checks.len()),
        )?;
        Ok(if self.result_passed { 0 } else { 1 })
    }
    fn write_baseline(&self) -> Result<()> {
        fs::write(
            self.output.join("baseline.json"),
            serde_json::to_string_pretty(self.baseline.as_ref().unwrap())?,
        )?;
        Ok(())
    }
    pub fn write_result(&self) -> Result<()> {
        let runs: Vec<_> = self
            .turn_metrics
            .iter()
            .map(|r| {
                let mut summary = r.recorder.summary();
                let object = summary.as_object_mut().unwrap();
                object.insert("pid".into(), r.pid.into());
                object.insert("requested_turns".into(), r.requested_turns.into());
                object.insert("group".into(), r.group.clone().into());
                object.insert(
                    "completed".into(),
                    (r.recorder.turn == r.recorder.start_turn + r.requested_turns).into(),
                );
                summary
            })
            .collect();
        let rows: Vec<_> = self
            .turn_metrics
            .iter()
            .flat_map(|r| r.recorder.rows.iter().cloned())
            .collect();
        let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for r in &self.turn_metrics {
            groups
                .entry(r.group.clone())
                .or_default()
                .extend(r.recorder.rows.iter().cloned());
        }
        let groups: BTreeMap<_, _> = groups
            .into_iter()
            .map(|(name, rows)| (name, metrics::summarize_turns(&rows)))
            .collect();
        let result = json!({"scenario":self.scenario.get("name"),"passed":self.result_passed,"test_mode":self.scenario.get("test_mode"),"completed_steps":self.completed_steps,"failure":self.failure,"checks":self.result_checks,"metrics":{"autoplay":runs,"turn_summary":metrics::summarize_turns(&rows),"groups":groups,"save_load":self.save_load_metrics.summary()}});
        fs::write(
            self.output.join("result.json"),
            format!("{}\n", serde_json::to_string_pretty(&result)?),
        )?;
        Ok(())
    }
}
