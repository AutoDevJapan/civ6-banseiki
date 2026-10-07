//! In-memory game boundary for the scenario safety tests. No real process, input,
//! Steam launch, FireTuner connection, or wall-clock wait can pass this boundary.

use civ6_autotest_rs::{
    runner::{Backend, Runner},
    time::Clock,
    Error, Result,
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tempfile::TempDir;

#[derive(Default, Debug, Clone)]
pub struct Trace {
    pub events: Vec<String>,
    pub connects: Vec<f64>,
    pub state_queries: Vec<f64>,
    pub lua: Vec<(String, String, f64)>,
    pub presses: Vec<(u16, f64)>,
    pub captures: Vec<(PathBuf, f64)>,
    pub disconnects: usize,
    pub require_processes: usize,
    pub process_queries: usize,
    pub launches: usize,
    pub window_deadlines: Vec<f64>,
    pub manuals: Vec<String>,
}

#[derive(Default)]
pub struct FakeClock {
    now: Mutex<f64>,
    sleeps: Mutex<Vec<f64>>,
    reads: Mutex<usize>,
    on_sleep: Mutex<Option<Box<dyn FnMut(f64) + Send>>>,
}
impl FakeClock {
    pub fn set(&self, value: f64) {
        *self.now.lock().unwrap() = value;
    }
    pub fn advance(&self, value: f64) {
        *self.now.lock().unwrap() += value;
    }
    pub fn sleeps(&self) -> Vec<f64> {
        self.sleeps.lock().unwrap().clone()
    }
    pub fn reads(&self) -> usize {
        *self.reads.lock().unwrap()
    }
    pub fn on_sleep(&self, callback: impl FnMut(f64) + Send + 'static) {
        *self.on_sleep.lock().unwrap() = Some(Box::new(callback));
    }
}
impl Clock for FakeClock {
    fn now(&self) -> f64 {
        *self.reads.lock().unwrap() += 1;
        *self.now.lock().unwrap()
    }
    fn sleep(&self, seconds: f64) {
        assert!(seconds.is_finite() && seconds >= 0.0, "invalid fake sleep");
        self.advance(seconds);
        self.sleeps.lock().unwrap().push(seconds);
        if let Some(callback) = self.on_sleep.lock().unwrap().as_mut() {
            callback(seconds);
        }
    }
}

type StateFn = Box<dyn FnMut(f64) -> Result<Vec<String>> + Send>;
type LuaFn = Box<dyn FnMut(&str, &str, f64) -> Result<String> + Send>;
type ProcessFn = Box<dyn FnMut(f64) -> Result<Option<(u32, f64)>> + Send>;
type CaptureFn = Box<dyn FnMut(&Path, Option<[f64; 4]>, f64) -> Result<()> + Send>;

pub struct SafetyBackend {
    control_owned: bool,
    control_held: bool,
    pub trace: Arc<Mutex<Trace>>,
    pub on_connect: Option<Box<dyn FnMut(u16, f64) -> Result<()> + Send>>,
    pub on_states: StateFn,
    pub on_lua: LuaFn,
    pub on_process: ProcessFn,
    pub on_require_process: Option<Box<dyn FnMut() -> Result<()> + Send>>,
    pub on_launch: Option<Box<dyn FnMut() -> Result<()> + Send>>,
    pub on_wait_window: Option<Box<dyn FnMut(f64) -> Result<isize> + Send>>,
    pub on_press: Option<Box<dyn FnMut(u16, f64) -> Result<()> + Send>>,
    pub on_capture: Option<CaptureFn>,
}
impl SafetyBackend {
    pub fn new(trace: Arc<Mutex<Trace>>) -> Self {
        Self {
            control_owned: false,
            control_held: false,
            trace,
            on_connect: None,
            on_states: Box::new(|_| Ok(vec!["FrontEnd".into(), "InGame".into()])),
            on_lua: Box::new(|_, _, _| Ok("true".into())),
            on_process: Box::new(|_| Ok(Some((1234, 1.0)))),
            on_require_process: None,
            on_launch: None,
            on_wait_window: None,
            on_press: None,
            on_capture: None,
        }
    }
    pub fn owned_chain(mut self) -> Self {
        self.control_owned = true;
        self
    }
    fn event(&self, event: &str) {
        self.trace.lock().unwrap().events.push(event.into());
    }
}
impl Backend for SafetyBackend {
    fn acquire_startup_lock(&mut self) -> Result<()> {
        if !self.control_owned {
            return Err(Error::input("safety fixture has no live-chain ownership"));
        }
        self.control_held = true;
        self.check_startup_lock()
    }
    fn check_startup_lock(&mut self) -> Result<()> {
        if self.control_owned && self.control_held {
            Ok(())
        } else {
            Err(Error::input("safety fixture live-chain ownership was lost"))
        }
    }
    fn release_startup_lock(&mut self) {
        self.control_held = false;
    }
    fn connect(&mut self, port: u16, timeout: f64) -> Result<()> {
        self.event("connect");
        self.trace.lock().unwrap().connects.push(timeout);
        if let Some(callback) = &mut self.on_connect {
            callback(port, timeout)
        } else {
            Ok(())
        }
    }
    fn disconnect(&mut self) {
        self.event("disconnect");
        self.trace.lock().unwrap().disconnects += 1;
    }
    fn list_states(&mut self, timeout: f64) -> Result<Vec<String>> {
        self.event("states");
        self.trace.lock().unwrap().state_queries.push(timeout);
        (self.on_states)(timeout)
    }
    fn run_lua(&mut self, state: &str, code: &str, timeout: f64) -> Result<String> {
        self.event(&format!("lua:{state}"));
        self.trace
            .lock()
            .unwrap()
            .lua
            .push((state.into(), code.into(), timeout));
        (self.on_lua)(state, code, timeout)
    }
    fn require_single_process(&mut self) -> Result<()> {
        self.event("require_process");
        self.trace.lock().unwrap().require_processes += 1;
        if let Some(callback) = &mut self.on_require_process {
            callback()
        } else {
            Ok(())
        }
    }
    fn single_process(&mut self, settle: f64) -> Result<Option<(u32, f64)>> {
        self.event("single_process");
        self.trace.lock().unwrap().process_queries += 1;
        (self.on_process)(settle)
    }
    fn launch(&mut self) -> Result<()> {
        self.event("launch");
        self.trace.lock().unwrap().launches += 1;
        if let Some(callback) = &mut self.on_launch {
            callback()
        } else {
            Err(Error::input("test backend forbids a Steam launch"))
        }
    }
    fn wait_window(&mut self, deadline: f64) -> Result<isize> {
        self.event("wait_window");
        self.trace.lock().unwrap().window_deadlines.push(deadline);
        if let Some(callback) = &mut self.on_wait_window {
            callback(deadline)
        } else {
            Err(Error::input("test backend has no game window"))
        }
    }
    fn press_key(&mut self, key: u16, deadline: f64) -> Result<()> {
        self.event("press");
        self.trace.lock().unwrap().presses.push((key, deadline));
        if let Some(callback) = &mut self.on_press {
            callback(key, deadline)
        } else {
            Ok(())
        }
    }
    fn capture(&mut self, path: &Path, crop: Option<[f64; 4]>, deadline: f64) -> Result<()> {
        self.event("capture");
        self.trace
            .lock()
            .unwrap()
            .captures
            .push((path.to_path_buf(), deadline));
        if let Some(callback) = &mut self.on_capture {
            callback(path, crop, deadline)
        } else {
            Err(Error::capture("test backend forbids an actual capture"))
        }
    }
    fn mod_folders(&mut self, _scenario: &Value, _mods: &Path) -> Result<Vec<PathBuf>> {
        Ok(vec![])
    }
    fn newest_mod(&mut self, _folders: &[PathBuf]) -> Result<Option<(PathBuf, f64)>> {
        Ok(None)
    }
    fn stale_art(&mut self, _folders: &[PathBuf]) -> Result<Vec<String>> {
        Ok(vec![])
    }
    fn manual(&mut self, prompt: &str) -> Result<()> {
        self.trace.lock().unwrap().manuals.push(prompt.into());
        Ok(())
    }
}

pub struct Fixture {
    pub root: TempDir,
    pub logs: PathBuf,
    pub saves: PathBuf,
    pub output: PathBuf,
    pub trace: Arc<Mutex<Trace>>,
    pub clock: Arc<FakeClock>,
}
impl Fixture {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let logs = root.path().join("logs");
        let saves = root.path().join("saves");
        fs::create_dir(&logs).unwrap();
        fs::create_dir(&saves).unwrap();
        for name in ["Lua.log", "Database.log", "Modding.log"] {
            fs::write(logs.join(name), "").unwrap();
        }
        Self {
            output: root.path().join("output"),
            root,
            logs,
            saves,
            trace: Arc::new(Mutex::new(Trace::default())),
            clock: Arc::new(FakeClock::default()),
        }
    }
    pub fn backend(&self) -> SafetyBackend {
        SafetyBackend::new(self.trace.clone())
    }
    pub fn runner(&self, scenario: Value, backend: SafetyBackend) -> Runner {
        Runner::with_backend(
            scenario,
            self.output.clone(),
            self.logs.clone(),
            self.saves.clone(),
            4318,
            Box::new(backend),
            self.clock.clone(),
        )
    }
    pub fn direct_runner(&self, scenario: Value, backend: SafetyBackend) -> Runner {
        fs::create_dir_all(&self.output).unwrap();
        self.runner(scenario, backend)
    }
    pub fn trace(&self) -> Trace {
        self.trace.lock().unwrap().clone()
    }
    pub fn records(&self) -> Vec<Value> {
        fs::read_to_string(self.output.join("steps.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}
