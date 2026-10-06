#![allow(dead_code)]

use civ6_autotest_rs::{
    runner::{Backend, Runner},
    time::Clock,
    Error, Result,
};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tempfile::TempDir;

pub type Calls = Arc<Mutex<Vec<(String, String)>>>;

pub fn object_keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

#[derive(Default)]
pub struct ManualClock {
    now: Mutex<f64>,
    min_sleep: Mutex<f64>,
}
impl ManualClock {
    pub fn at(&self) -> f64 {
        *self.now.lock().unwrap()
    }
    pub fn advance(&self, seconds: f64) {
        *self.now.lock().unwrap() += seconds;
    }
    pub fn set_min_sleep(&self, seconds: f64) {
        *self.min_sleep.lock().unwrap() = seconds;
    }
}
impl Clock for ManualClock {
    fn now(&self) -> f64 {
        self.at()
    }
    fn sleep(&self, seconds: f64) {
        self.advance(seconds.max(*self.min_sleep.lock().unwrap()).max(0.0));
    }
}

type Lua = Box<dyn FnMut(&str, &str) -> Result<String> + Send>;
type States = Box<dyn FnMut() -> Result<Vec<String>> + Send>;
type Process = Box<dyn FnMut(f64) -> Result<Option<(u32, f64)>> + Send>;
type Window = Box<dyn FnMut(f64) -> Result<isize> + Send>;

pub struct FakeBackend {
    pub calls: Calls,
    pub timeouts: Arc<Mutex<Vec<f64>>>,
    pub events: Arc<Mutex<Vec<String>>>,
    pub on_lua: Lua,
    pub on_states: States,
    pub on_process: Process,
    pub on_window: Window,
    pub on_dx11: Box<dyn FnMut(isize) -> Result<()> + Send>,
    pub on_memory: Box<dyn FnMut(u32) -> Result<Value> + Send>,
    pub on_mods: Box<dyn FnMut() -> Result<Vec<PathBuf>> + Send>,
    pub on_newest: Box<dyn FnMut() -> Result<Option<(PathBuf, f64)>> + Send>,
    pub on_stale: Box<dyn FnMut() -> Result<Vec<String>> + Send>,
    pub on_exit: Box<dyn FnMut(u32) -> Result<bool> + Send>,
    pub on_require: Box<dyn FnMut() -> Result<()> + Send>,
    pub on_launch: Box<dyn FnMut() -> Result<()> + Send>,
    pub on_close_orphans: Box<dyn FnMut() -> Result<Vec<u32>> + Send>,
}
impl FakeBackend {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            timeouts: Arc::new(Mutex::new(Vec::new())),
            events: Arc::new(Mutex::new(Vec::new())),
            on_lua: Box::new(|_, code| {
                if code.contains("return tostring(startTurn)") {
                    Ok("1|0".into())
                } else if code.contains("IsAlive()") {
                    Ok("1|true".into())
                } else if code.contains("Game.GetCurrentGameTurn()") {
                    Ok("1".into())
                } else {
                    Ok("true".into())
                }
            }),
            on_states: Box::new(|| Ok(vec!["InGame".into(), "FrontEnd".into()])),
            on_process: Box::new(|_| Ok(Some((123, 100.0)))),
            on_window: Box::new(|_| Ok(123)),
            on_dx11: Box::new(|_| Ok(())),
            on_memory: Box::new(|_| Ok(json!({"private_bytes":100,"working_set_bytes":80}))),
            on_mods: Box::new(|| Ok(vec![])),
            on_newest: Box::new(|| Ok(None)),
            on_stale: Box::new(|| Ok(vec![])),
            on_exit: Box::new(|_| Ok(true)),
            on_require: Box::new(|| Ok(())),
            on_launch: Box::new(|| Ok(())),
            on_close_orphans: Box::new(|| Ok(Vec::new())),
        }
    }
    pub fn lua(mut self, f: impl FnMut(&str, &str) -> Result<String> + Send + 'static) -> Self {
        self.on_lua = Box::new(f);
        self
    }
    pub fn states(mut self, f: impl FnMut() -> Result<Vec<String>> + Send + 'static) -> Self {
        self.on_states = Box::new(f);
        self
    }
    pub fn process(
        mut self,
        f: impl FnMut(f64) -> Result<Option<(u32, f64)>> + Send + 'static,
    ) -> Self {
        self.on_process = Box::new(f);
        self
    }
}
impl Backend for FakeBackend {
    fn connect(&mut self, _: u16, _: f64) -> Result<()> {
        self.events.lock().unwrap().push("connect".into());
        Ok(())
    }
    fn disconnect(&mut self) {
        self.events.lock().unwrap().push("disconnect".into());
    }
    fn list_states(&mut self, _: f64) -> Result<Vec<String>> {
        self.events.lock().unwrap().push("states".into());
        (self.on_states)()
    }
    fn run_lua(&mut self, state: &str, code: &str, timeout: f64) -> Result<String> {
        self.calls.lock().unwrap().push((state.into(), code.into()));
        self.timeouts.lock().unwrap().push(timeout);
        self.events.lock().unwrap().push(format!("lua:{state}"));
        (self.on_lua)(state, code)
    }
    fn require_single_process(&mut self) -> Result<()> {
        (self.on_require)()
    }
    fn single_process(&mut self, settle: f64) -> Result<Option<(u32, f64)>> {
        (self.on_process)(settle)
    }
    fn memory(&mut self, pid: u32) -> Result<Value> {
        (self.on_memory)(pid)
    }
    fn launch(&mut self) -> Result<()> {
        self.events.lock().unwrap().push("launch".into());
        (self.on_launch)()
    }
    fn close_orphaned_reporters(&mut self) -> Result<Vec<u32>> {
        (self.on_close_orphans)()
    }
    fn wait_window(&mut self, deadline: f64) -> Result<isize> {
        self.events.lock().unwrap().push("window".into());
        (self.on_window)(deadline)
    }
    fn require_dx11(&mut self, hwnd: isize) -> Result<()> {
        self.events.lock().unwrap().push(format!("dx11:{hwnd}"));
        (self.on_dx11)(hwnd)
    }
    fn click(&mut self, fx: f64, fy: f64) -> Result<(i32, i32)> {
        self.events.lock().unwrap().push(format!("click:{fx}:{fy}"));
        Ok((0, 0))
    }
    fn press_key(&mut self, key: u16, _: f64) -> Result<()> {
        self.events.lock().unwrap().push(format!("key:{key}"));
        Ok(())
    }
    fn request_close(&mut self) -> Result<()> {
        self.events.lock().unwrap().push("close".into());
        Ok(())
    }
    fn wait_exit(&mut self, pid: u32, _: f64) -> Result<bool> {
        self.events.lock().unwrap().push(format!("exit:{pid}"));
        (self.on_exit)(pid)
    }
    fn terminate(&mut self, pid: u32, start: f64) -> Result<()> {
        self.events
            .lock()
            .unwrap()
            .push(format!("terminate:{pid}:{start}"));
        Ok(())
    }
    fn mod_folders(&mut self, _: &Value, _: &Path) -> Result<Vec<PathBuf>> {
        self.events.lock().unwrap().push("mods".into());
        (self.on_mods)()
    }
    fn newest_mod(&mut self, _: &[PathBuf]) -> Result<Option<(PathBuf, f64)>> {
        self.events.lock().unwrap().push("newest".into());
        (self.on_newest)()
    }
    fn stale_art(&mut self, _: &[PathBuf]) -> Result<Vec<String>> {
        self.events.lock().unwrap().push("stale".into());
        (self.on_stale)()
    }
    fn manual(&mut self, _: &str) -> Result<()> {
        Err(Error::input("manual action not available in tests"))
    }
}

pub struct Harness {
    pub root: TempDir,
    pub runner: Runner,
    pub clock: Arc<ManualClock>,
    pub calls: Calls,
    pub timeouts: Arc<Mutex<Vec<f64>>>,
    pub events: Arc<Mutex<Vec<String>>>,
}
impl Harness {
    pub fn new(scenario: Value, fake: FakeBackend) -> Self {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("output");
        std::fs::create_dir(&output).unwrap();
        let logs = root.path().join("logs");
        let saves = root.path().join("saves");
        std::fs::create_dir(&logs).unwrap();
        std::fs::create_dir(&saves).unwrap();
        let clock = Arc::new(ManualClock::default());
        let calls = fake.calls.clone();
        let timeouts = fake.timeouts.clone();
        let events = fake.events.clone();
        let runner = Runner::with_backend(
            scenario,
            output,
            logs,
            saves,
            4318,
            Box::new(fake),
            clock.clone(),
        );
        Self {
            root,
            runner,
            clock,
            calls,
            timeouts,
            events,
        }
    }
    pub fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().unwrap().clone()
    }
    pub fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
    pub fn count_code(&self, needle: &str) -> usize {
        self.calls()
            .iter()
            .filter(|(_, c)| c.contains(needle))
            .count()
    }
    pub fn log(&self) -> String {
        std::fs::read_to_string(self.runner.output.join("steps.jsonl")).unwrap_or_default()
    }
    pub fn jsonl(&self, name: &str) -> Vec<Value> {
        std::fs::read_to_string(self.runner.output.join(name))
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    }
}

pub fn sequence(mut values: VecDeque<String>) -> impl FnMut(&str, &str) -> Result<String> + Send {
    let mut observed_turn = 1_i64;
    move |_, code| {
        if code.contains("tostring(AutoplayManager.IsActive())") {
            let status = values
                .pop_front()
                .ok_or_else(|| Error::scenario("status sequence exhausted"))?;
            observed_turn = status.split('|').next().unwrap().parse().unwrap();
            Ok(status)
        } else if code.contains("return tostring(startTurn)") {
            Ok("1|0".into())
        } else if code.contains("IsAlive()") {
            Ok(format!("{observed_turn}|true"))
        } else {
            Ok("true".into())
        }
    }
}
