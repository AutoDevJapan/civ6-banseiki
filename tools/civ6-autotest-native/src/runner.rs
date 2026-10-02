use crate::{
    logs, mods, platform, scripts,
    tuner::{TunerClient, TunerError},
};
use chrono::Utc;
use regex::Regex;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct RunError {
    message: String,
    unknown: bool,
    lua: bool,
}
impl RunError {
    fn new(s: impl Into<String>) -> Self {
        Self {
            message: s.into(),
            unknown: false,
            lua: false,
        }
    }
}
impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl From<String> for RunError {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}
impl From<std::io::Error> for RunError {
    fn from(e: std::io::Error) -> Self {
        Self::new(e.to_string())
    }
}
impl From<serde_json::Error> for RunError {
    fn from(e: serde_json::Error) -> Self {
        Self::new(e.to_string())
    }
}
impl From<TunerError> for RunError {
    fn from(e: TunerError) -> Self {
        let unknown = matches!(e, TunerError::Unknown(_));
        let lua = matches!(e, TunerError::Lua(_));
        Self {
            message: e.to_string(),
            unknown,
            lua,
        }
    }
}
type Result<T> = std::result::Result<T, RunError>;

fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| RunError::new(format!("{key} must be a string")))
}
fn string_list(v: &Value) -> Result<Vec<String>> {
    v.as_array()
        .ok_or_else(|| RunError::new("string array required"))?
        .iter()
        .map(|x| {
            x.as_str()
                .map(str::to_string)
                .ok_or_else(|| RunError::new("string array entry required"))
        })
        .collect()
}
fn duration(deadline: Instant, cap: f64) -> Result<Duration> {
    let left = deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| RunError::new("Step deadline expired"))?;
    Ok(left.min(Duration::from_secs_f64(cap)))
}
fn sleep(deadline: Instant, secs: f64) -> Result<()> {
    thread::sleep(duration(deadline, secs)?);
    Ok(())
}
fn safe_name(name: &str) -> bool {
    Regex::new(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,79}$")
        .unwrap()
        .is_match(name)
}
fn lua_syntax(code: &str) -> Result<()> {
    if code.contains('\0') || code.len() > 2 * 1024 * 1024 - 256 {
        return Err(RunError::new("invalid Lua command size or NUL"));
    }
    mlua::Lua::new()
        .load(code)
        .into_function()
        .map(|_| ())
        .map_err(|e| RunError::new(format!("Lua syntax: {e}")))
}

pub fn validate(s: &Value) -> Result<()> {
    if text(s, "name")?.is_empty() {
        return Err(RunError::new("scenario name is empty"));
    }
    if s.get("judge_profile")
        .and_then(Value::as_str)
        .unwrap_or("generic")
        != "generic"
    {
        return Err(RunError::new(
            "native runner currently requires generic judge",
        ));
    }
    let steps = s["steps"]
        .as_array()
        .ok_or_else(|| RunError::new("steps must be an array"))?;
    if steps.is_empty() || steps.len() > 10000 {
        return Err(RunError::new("steps must be nonempty and bounded"));
    }
    let types = Regex::new(r"^[A-Z][A-Z0-9_]+$").unwrap();
    let guid = Regex::new(r"^[0-9a-fA-F]{8}-(?:[0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12}$").unwrap();
    let map_file = Regex::new(r"^\{[0-9a-fA-F-]{36}\}Maps/[A-Za-z0-9_-]+\.Civ6Map$").unwrap();
    for (i, step) in steps.iter().enumerate() {
        let action = text(step, "action")?;
        if let Some(t) = step.get("timeout") {
            let x = t
                .as_f64()
                .ok_or_else(|| RunError::new("timeout number required"))?;
            if !x.is_finite() || x <= 0. || x > 86400. {
                return Err(RunError::new("invalid timeout"));
            }
        }
        // These scenarios contain no performance metrics. Reject unported fields rather than silently omitting them.
        if step["metrics"] == true
            || step["research_metrics"] == true
            || step["allow_human_elimination"] == true
        {
            return Err(RunError::new(
                "performance/elimination metrics are not supported by this native profile",
            ));
        }
        match action {
            "restart_game" | "start_game" | "exit_to_menu" => {}
            "enable_mods" => {
                let ids = string_list(&step["ids"])?;
                if ids.is_empty() || ids.iter().any(|s| !guid.is_match(s)) {
                    return Err(RunError::new("invalid MOD IDs"));
                }
                if let Some(d) = step.get("disable_ids")
                    && string_list(d)?.iter().any(|s| !guid.is_match(s))
                {
                    return Err(RunError::new("invalid disabled MOD IDs"));
                }
            }
            "configure" => {
                for k in ["ruleset", "map_size", "speed"] {
                    if !types.is_match(text(step, k)?) {
                        return Err(RunError::new(format!("invalid {k}")));
                    }
                }
                if step["seed"].as_u64().is_none() {
                    return Err(RunError::new("unsigned integer seed required"));
                }
                if let Some(m) = step.get("map_file")
                    && !map_file.is_match(m.as_str().unwrap_or(""))
                {
                    return Err(RunError::new("invalid map_file"));
                }
                if let Some(c) = step.get("city_states")
                    && c.as_u64().is_none_or(|x| x > 60)
                {
                    return Err(RunError::new("invalid city_states"));
                }
                if let Some(b) = step.get("build_setup")
                    && !b.is_boolean()
                {
                    return Err(RunError::new("boolean build_setup required"));
                }
                let players = step["players"]
                    .as_array()
                    .ok_or_else(|| RunError::new("players required"))?;
                if players.is_empty()
                    || players.len() > 62
                    || players.iter().filter(|p| p["human"] == true).count() != 1
                {
                    return Err(RunError::new(
                        "exactly one human and 1..62 players required",
                    ));
                }
                for p in players {
                    if !types.is_match(text(p, "leader")?)
                        || !types.is_match(text(p, "civ")?)
                        || !p["human"].is_boolean()
                    {
                        return Err(RunError::new("invalid player"));
                    }
                }
                lua_syntax(&scripts::configure(step))?;
                lua_syntax(&scripts::build_setup(players))?;
            }
            "ui_lua" => {
                text(step, "state_name")?;
                lua_syntax(text(step, "code")?)?;
                for key in ["capture_as", "equals_capture"] {
                    if let Some(v) = step.get(key)
                        && !safe_name(v.as_str().unwrap_or(""))
                    {
                        return Err(RunError::new("invalid capture name"));
                    }
                }
            }
            "wait_for" => {
                let c = &step["condition"];
                match text(c, "type")? {
                    "lua" => {
                        text(c, "state_name")?;
                        lua_syntax(text(c, "code")?)?;
                    }
                    "state" => {
                        text(c, "name")?;
                    }
                    "log_line" => {
                        if !logs::LOG_NAMES.contains(&text(c, "log")?)
                            || text(c, "contains")?.is_empty()
                        {
                            return Err(RunError::new("invalid log condition"));
                        }
                    }
                    _ => return Err(RunError::new("unsupported condition")),
                }
            }
            "save" | "load" => {
                if !safe_name(text(step, "name")?) {
                    return Err(RunError::new("invalid save name"));
                }
                if let Some(e) = step.get("exact_name")
                    && !e.is_boolean()
                {
                    return Err(RunError::new("boolean exact_name required"));
                }
            }
            "autoplay" | "end_turn" => {
                let turns = step
                    .get("turns")
                    .map(Value::as_i64)
                    .unwrap_or(Some(1))
                    .ok_or_else(|| RunError::new("integer turns required"))?;
                if !(1..=10000).contains(&turns) {
                    return Err(RunError::new("invalid turns"));
                }
                if let Some(f) = step.get("fast_forward") {
                    if !f.is_boolean() {
                        return Err(RunError::new("boolean fast_forward required"));
                    }
                    if f == true && s.get("test_mode").is_none() {
                        return Err(RunError::new("fast_forward requires test_mode"));
                    }
                }
            }
            "pause" => {
                if step["seconds"]
                    .as_f64()
                    .is_none_or(|v| !v.is_finite() || v < 0.)
                {
                    return Err(RunError::new("invalid pause"));
                }
            }
            _ => {
                return Err(RunError::new(format!(
                    "step {} unsupported action {action}",
                    i + 1
                )));
            }
        }
    }
    if let Some(m) = s.get("test_mode")
        && (m["skipped_turns"].as_u64().is_none() || m["continuous_performance_valid"] != false)
    {
        return Err(RunError::new("invalid test_mode"));
    }
    Ok(())
}

fn require_chain() -> Result<()> {
    let lock: Value = serde_json::from_slice(&fs::read("D:/CC/Tools/civ6-live/civ6.lock")?)?;
    let job = std::env::var("CODEX_JOB_ID").map_err(|_| RunError::new("CODEX_JOB_ID required"))?;
    if job.is_empty()
        || lock["job"].as_str() != Some(&job)
        || lock["live_chain"] != true
        || lock["chain_pid"].as_u64().is_none()
    {
        return Err(RunError::new(
            "the standard live_chain must own the game resource",
        ));
    }
    Ok(())
}

pub fn cli(args: &[String]) -> Result<i32> {
    let mut a = args.to_vec();
    // Positional entry tags supplied by the existing Rust live_chain. No script is executed or imported.
    let tuner_tag = a
        .first()
        .is_some_and(|s| Path::new(s).file_name().is_some_and(|n| n == "tuner.py"));
    if a.first().is_some_and(|s| s.ends_with(".py")) {
        a.remove(0);
    }
    if tuner_tag || a.first().is_some_and(|s| s == "states") {
        if a.first().is_none_or(|s| s != "states") {
            return Err(RunError::new(
                "only states is supported by the compatibility entry",
            ));
        }
        require_chain()?;
        if platform::single_civ_process(Duration::ZERO)?.is_none() {
            return Err(RunError::new("Civ6 is absent"));
        }
        let mut c = TunerClient::connect(4318, Duration::from_secs(5))?;
        for (i, n) in c.list_state_pairs(Duration::from_secs(5))? {
            println!("{i}: {n}");
        }
        return Ok(0);
    }
    if a.is_empty() {
        return Err(RunError::new(
            "usage: civ6-autotest-native SCENARIO [--check] [--test-mode] --output DIRECTORY",
        ));
    }
    let scenario_path = PathBuf::from(a.remove(0));
    let mut scenario: Value = serde_json::from_slice(&fs::read(&scenario_path)?)?;
    let mut output = None;
    let mut log_dir = None;
    let mut saves = None;
    let mut port = 4318;
    let mut check = false;
    let mut test_mode = false;
    let mut idx = 0;
    while idx < a.len() {
        match a[idx].as_str() {
            "--check" => check = true,
            "--test-mode" => test_mode = true,
            "--no-auto-restart" => {}
            "--output" | "--logs" | "--saves" | "--port" => {
                let v = a
                    .get(idx + 1)
                    .ok_or_else(|| RunError::new("option value missing"))?;
                match a[idx].as_str() {
                    "--output" => output = Some(PathBuf::from(v)),
                    "--logs" => log_dir = Some(PathBuf::from(v)),
                    "--saves" => saves = Some(PathBuf::from(v)),
                    _ => {
                        port = v
                            .parse::<u16>()
                            .map_err(|_| RunError::new("invalid port"))?
                    }
                };
                idx += 1;
            }
            s => return Err(RunError::new(format!("unknown option {s}"))),
        }
        idx += 1;
    }
    if test_mode {
        scenario["test_mode"] = json!({"skipped_turns":0,"continuous_performance_valid":false});
        for step in scenario["steps"]
            .as_array_mut()
            .ok_or_else(|| RunError::new("steps required"))?
        {
            if step["action"] == "autoplay" && step["turns"].as_i64().unwrap_or(0) > 2 {
                step["fast_forward"] = json!(true);
            }
        }
    }
    validate(&scenario)?;
    if check {
        println!(
            "checked {} steps and all Lua syntax (Rust/Lua54; no Python)",
            scenario["steps"].as_array().unwrap().len()
        );
        return Ok(0);
    }
    require_chain()?;
    let profile = PathBuf::from(
        std::env::var("USERPROFILE").map_err(|_| RunError::new("USERPROFILE unavailable"))?,
    );
    let log_dir = log_dir.unwrap_or_else(|| {
        PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default())
            .join("Firaxis Games/Sid Meier's Civilization VI/Logs")
    });
    let saves = saves.unwrap_or_else(|| {
        profile.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single")
    });
    let output = output.ok_or_else(|| RunError::new("new --output directory required"))?;
    Runner::new(
        scenario,
        output,
        log_dir,
        saves,
        profile.join("Documents/My Games/Sid Meier's Civilization VI/Mods"),
        port,
    )?
    .run()
}

struct Runner {
    scenario: Value,
    output: PathBuf,
    log_dir: PathBuf,
    saves: PathBuf,
    mods_dir: PathBuf,
    port: u16,
    client: Option<TunerClient>,
    process: Option<platform::CivProcess>,
    baseline: Value,
    step_baseline: Value,
    captures: BTreeMap<String, String>,
    consumed: BTreeMap<(String, String), usize>,
    completed: usize,
    failure: Option<String>,
    checks: Value,
    passed: bool,
    ruleset: Option<String>,
}
impl Runner {
    fn new(
        scenario: Value,
        output: PathBuf,
        log_dir: PathBuf,
        saves: PathBuf,
        mods_dir: PathBuf,
        port: u16,
    ) -> Result<Self> {
        if output.exists() {
            return Err(RunError::new("output directory already exists"));
        }
        if let Some(p) = output.parent() {
            fs::create_dir_all(p)?;
        }
        fs::create_dir(&output)?;
        Ok(Self {
            scenario,
            output,
            log_dir,
            saves,
            mods_dir,
            port,
            client: None,
            process: None,
            baseline: Value::Null,
            step_baseline: Value::Null,
            captures: BTreeMap::new(),
            consumed: BTreeMap::new(),
            completed: 0,
            failure: None,
            checks: json!([]),
            passed: false,
            ruleset: None,
        })
    }
    fn log(&mut self, action: &str, status: &str, detail: &str) -> Result<()> {
        if status == "FAIL" {
            self.failure = Some(format!("{action}: {detail}"));
        }
        let line =
            json!({"at":Utc::now().to_rfc3339(),"action":action,"status":status,"detail":detail});
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.output.join("steps.jsonl"))?;
        writeln!(f, "{line}")?;
        f.flush()?;
        println!("[{status}] {action}: {detail}");
        std::io::stdout().flush()?;
        Ok(())
    }
    fn connect(&mut self, deadline: Instant) -> Result<&mut TunerClient> {
        if self.client.is_none() {
            self.client = Some(TunerClient::connect(self.port, duration(deadline, 5.)?)?);
        }
        Ok(self.client.as_mut().unwrap())
    }
    fn states(&mut self, deadline: Instant) -> Result<Vec<String>> {
        require_chain()?;
        let actual = platform::single_civ_process(Duration::ZERO)?
            .ok_or_else(|| RunError::new("Civ6 disappeared"))?;
        if let Some(expected) = &self.process
            && (actual.pid != expected.pid
                || actual.created != expected.created
                || actual.path != expected.path)
        {
            return Err(RunError::new("Civ6 process changed during the scenario"));
        }
        for attempt in 0..2 {
            let timeout = duration(deadline, 5.)?;
            match self.connect(deadline)?.list_state_pairs(timeout) {
                Ok(v) => return Ok(v.into_iter().map(|(_, n)| n).collect()),
                Err(e) => {
                    self.client = None;
                    if attempt == 1 {
                        return Err(e.into());
                    }
                }
            }
        }
        unreachable!()
    }
    fn lua(&mut self, state: &str, code: &str, deadline: Instant) -> Result<String> {
        self.states(deadline)?;
        let timeout = duration(deadline, 5.)?;
        let r = self.connect(deadline)?.run_lua(state, code, timeout);
        match r {
            Ok(v) => {
                let mut output = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(self.output.join("tuner-output.log"))?;
                writeln!(output, "{}", v.output)?;
                Ok(v.value)
            }
            Err(e) => {
                self.client = None;
                Err(e.into())
            }
        }
    }
    fn idempotent(&mut self, state: &str, code: &str, deadline: Instant) -> Result<String> {
        loop {
            match self.lua(state, code, deadline) {
                Ok(v) => return Ok(v),
                Err(e) if !e.lua => {
                    self.client = None;
                    if duration(deadline, 0.5).is_err() {
                        return Err(e);
                    }
                    sleep(deadline, 0.5)?;
                }
                Err(e) => return Err(e),
            }
        }
    }
    fn refresh_baseline(&mut self) -> Result<()> {
        self.baseline = logs::snapshot(&self.log_dir)?;
        fs::write(
            self.output.join("baseline.json"),
            serde_json::to_vec_pretty(&self.baseline)?,
        )?;
        Ok(())
    }
    fn read_logs(&self, baseline: &Value) -> Result<BTreeMap<String, String>> {
        Ok(logs::read_since(&self.log_dir, baseline)?)
    }
    fn loadscreen_count(&self) -> Result<usize> {
        Ok(self
            .read_logs(&self.baseline)?
            .get("Lua.log")
            .ok_or_else(|| RunError::new("Lua.log missing"))?
            .matches("LoadScreen: OnLoadGameViewStateDone")
            .count())
    }
    fn popups(&mut self, deadline: Instant) -> Result<Vec<String>> {
        let states = self.states(deadline)?;
        let mut closed = Vec::new();
        for (state, call) in scripts::POPUPS {
            if states.iter().any(|s| s.eq_ignore_ascii_case(state))
                && self.idempotent(
                    state,
                    &format!("if ContextPtr:IsHidden() then return false end\n{call}\nreturn true"),
                    deadline,
                )? == "true"
            {
                closed.push(state.to_string());
            }
        }
        Ok(closed)
    }
    fn dismiss(&mut self, deadline: Instant) -> Result<()> {
        let states = self.states(deadline)?;
        if states
            .iter()
            .any(|s| s.eq_ignore_ascii_case("InGameTopOptionsMenu"))
        {
            let closed=self.lua("InGameTopOptionsMenu","if not ContextPtr:IsHidden() then OnReturn(); CloseImmediately(); return true end\nreturn false",deadline)?;
            if closed == "true" {
                self.wait(&json!({"type":"lua","state_name":"InGameTopOptionsMenu","code":"return ContextPtr:IsHidden()"}),deadline,false)?;
            }
        }
        if states
            .iter()
            .any(|s| s.eq_ignore_ascii_case("AdvisorPopup"))
        {
            self.idempotent(
                "AdvisorPopup",
                "if IsBlockingInput() then OnHideAdvisorDialog(); return true end\nreturn false",
                deadline,
            )?;
        }
        Ok(())
    }
    fn wait(&mut self, condition: &Value, deadline: Instant, close_popups: bool) -> Result<()> {
        let kind = text(condition, "type")?;
        let mut last = "condition not ready".to_string();
        let mut unblocks = 0;
        let mut closes = 0;
        while Instant::now() < deadline {
            if close_popups {
                let closed = self.popups(deadline)?;
                closes += closed.len();
                if closes > 25 {
                    return Err(RunError::new("diplomacy repeatedly blocks one turn"));
                }
            }
            let ready: Result<bool> = match kind {
                "state" => self.states(deadline).map(|ss| {
                    ss.iter()
                        .any(|s| s.eq_ignore_ascii_case(condition["name"].as_str().unwrap()))
                }),
                "lua" => self
                    .lua(
                        text(condition, "state_name")?,
                        text(condition, "code")?,
                        deadline,
                    )
                    .map(|v| {
                        last = format!("Lua returned {v}");
                        v == condition
                            .get("equals")
                            .map(|x| {
                                x.as_str()
                                    .map(str::to_string)
                                    .unwrap_or_else(|| x.to_string())
                            })
                            .unwrap_or_else(|| "true".into())
                    }),
                "log_line" => {
                    let name = text(condition, "log")?;
                    let marker = text(condition, "contains")?;
                    let logs = self.read_logs(&self.step_baseline)?;
                    let count = logs
                        .get(name)
                        .ok_or_else(|| RunError::new("missing wait log"))?
                        .lines()
                        .filter(|s| s.contains(marker))
                        .count();
                    let key = (name.to_string(), marker.to_string());
                    let consumed = *self.consumed.get(&key).unwrap_or(&0);
                    if count > consumed {
                        self.consumed.insert(key, consumed + 1);
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                }
                _ => return Err(RunError::new("unsupported wait condition")),
            };
            match ready {
                Ok(true) => {
                    duration(deadline, 1.)?;
                    return Ok(());
                }
                Ok(false) => {}
                Err(e) if e.lua => return Err(e),
                Err(e) if e.unknown => {
                    // Only the scenario's declared read-only wait predicate may be re-polled after a diplomacy unblock.
                    if condition["state_name"]
                        .as_str()
                        .is_none_or(|s| !s.eq_ignore_ascii_case("InGame"))
                        || unblocks >= 3
                    {
                        return Err(e);
                    }
                    self.client = None;
                    if self.popups(deadline)?.is_empty() {
                        return Err(e);
                    }
                    unblocks += 1;
                }
                Err(e) => {
                    last = e.to_string();
                    self.client = None;
                }
            }
            sleep(deadline, 0.5)?;
        }
        Err(RunError::new(format!("wait condition timed out: {last}")))
    }
    fn restart(&mut self, deadline: Instant) -> Result<String> {
        self.client = None;
        self.process = None;
        if let Some(old) = platform::single_civ_process(Duration::from_secs(30))? {
            platform::terminate_civ_process(&old)?;
            sleep(deadline, 8.)?;
        }
        let mut launches = 0;
        let mut next_launch = Instant::now();
        let mut last = "no game process".to_string();
        loop {
            duration(deadline, 1.)?;
            let process = platform::single_civ_process(Duration::ZERO)?;
            if process.is_none() && launches < 4 && Instant::now() >= next_launch {
                platform::launch_via_steam()?;
                launches += 1;
                next_launch = Instant::now() + Duration::from_secs(45);
                println!("Steam DX11 launch request {launches}");
            }
            if process.is_some() {
                match platform::wait_for_window(
                    deadline.min(Instant::now() + Duration::from_secs(5)),
                ) {
                    Ok(()) => break,
                    Err(e) => last = e,
                }
            }
            if Instant::now() + Duration::from_millis(500) >= deadline {
                return Err(RunError::new(format!("DX11 launch failed: {last}")));
            }
            sleep(deadline, 0.5)?;
        }
        // The opening finishes naturally; no unverified click can affect a menu button.
        self.client = None;
        loop {
            if self.idempotent("FrontEnd", "return UI.IsInFrontEnd()", deadline)? == "true" {
                break;
            }
            sleep(deadline, 0.5)?;
        }
        let p = platform::single_civ_process(Duration::ZERO)?
            .ok_or_else(|| RunError::new("Civ6 vanished after launch"))?;
        let install = p
            .path
            .ancestors()
            .nth(4)
            .ok_or_else(|| RunError::new("cannot locate the running Civ6 install"))?;
        let folders =
            mods::enabled_custom_mod_folders(&self.scenario, &self.mods_dir, &install.join("DLC"))?;
        let missing = mods::missing_atlas_textures(&folders)?;
        if !missing.is_empty() {
            return Err(RunError::new(format!(
                "missing atlas textures: {}",
                missing.join(", ")
            )));
        }
        // Restart must leave a single fresh process. FILETIME uses 100 ns ticks from 1601.
        if let Some((file, time)) = mods::newest_mod_time(&folders)? {
            let ticks = time
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| RunError::new(e.to_string()))?
                .as_nanos()
                / 100
                + 116444736000000000;
            if ticks > p.created as u128 {
                return Err(RunError::new(format!(
                    "mod newer than relaunched Civ6: {}",
                    file.display()
                )));
            }
        }
        self.process = Some(p);
        Ok("Civ6 relaunched via Steam option2; DX11 and fresh MOD files verified; FrontEnd answered".into())
    }
    fn enter_game(&mut self, before: usize, deadline: Instant) -> Result<()> {
        let mut begun = false;
        loop {
            duration(deadline, 1.)?;
            if !begun {
                if self.loadscreen_count()? <= before {
                    sleep(deadline, 0.5)?;
                    continue;
                }
                let states = self.states(deadline).unwrap_or_default();
                if states.iter().any(|s| s.eq_ignore_ascii_case("LoadScreen")) {
                    self.lua(
                        "LoadScreen",
                        "OnActivateButtonClicked()\nreturn true",
                        deadline,
                    )?;
                } else {
                    platform::focus_and_key(0x1b, deadline)?;
                }
                begun = true;
                self.client = None;
            }
            match self.states(deadline) {
                Ok(states) if states.iter().any(|s| s.eq_ignore_ascii_case("InGame")) => match self
                    .lua(
                        "InGame",
                        "return Game ~= nil and Game.GetLocalPlayer() >= 0",
                        deadline,
                    ) {
                    Ok(v) if v == "true" => {
                        self.dismiss(deadline)?;
                        return Ok(());
                    }
                    Ok(_) => {}
                    Err(e) if e.unknown || e.lua => return Err(e),
                    Err(_) => {
                        self.client = None;
                    }
                },
                Ok(_) => {
                    self.client = None;
                }
                Err(e) if e.lua || e.unknown => return Err(e),
                Err(_) => {
                    self.client = None;
                }
            }
            sleep(deadline, 0.5)?;
        }
    }
    fn save_name(&self, step: &Value) -> Result<String> {
        let base = text(step, "name")?;
        if step["exact_name"] == true {
            return Ok(base.into());
        }
        let resolved = fs::canonicalize(&self.output)?;
        let hash = format!(
            "{:x}",
            Sha256::digest(resolved.to_string_lossy().as_bytes())
        );
        Ok(format!("{}-{}", &base[..base.len().min(67)], &hash[..12]))
    }
    fn save(&mut self, step: &Value, deadline: Instant) -> Result<String> {
        let name = self.save_name(step)?;
        let path = self.saves.join(format!("{name}.Civ6Save"));
        if path.exists() {
            return Err(RunError::new("save already exists; refusing overwrite"));
        }
        self.dismiss(deadline)?;
        self.lua("InGame",&format!("JAC_AUTOTEST_SAVE_DONE = false\nif JAC_AUTOTEST_SAVE_HANDLER then Events.SaveComplete.Remove(JAC_AUTOTEST_SAVE_HANDLER) end\nJAC_AUTOTEST_SAVE_HANDLER = function() JAC_AUTOTEST_SAVE_DONE = true end\nEvents.SaveComplete.Add(JAC_AUTOTEST_SAVE_HANDLER)\nlocal result = Network.SaveGame({{Name={}, Location=SaveLocations.LOCAL_STORAGE, Type=Network.GetGameConfigurationSaveType(), IsAutosave=false, IsQuicksave=false}})\nif result == false then error('Network.SaveGame returned false') end\nreturn true",scripts::quote(&name)),deadline)?;
        self.wait(&json!({"type":"lua","state_name":"InGame","code":"return JAC_AUTOTEST_SAVE_DONE == true"}),deadline,false)?;
        self.lua("InGame","Events.SaveComplete.Remove(JAC_AUTOTEST_SAVE_HANDLER)\nJAC_AUTOTEST_SAVE_HANDLER = nil\nreturn true",deadline)?;
        let end = deadline.min(Instant::now() + Duration::from_secs(5));
        while !path.is_file() && Instant::now() < end {
            sleep(end, 0.2)?;
        }
        if fs::metadata(&path)?.len() == 0 {
            return Err(RunError::new("SaveComplete produced empty save"));
        }
        duration(deadline, 1.)?;
        Ok(path.to_string_lossy().into_owned())
    }
    fn load(&mut self, step: &Value, deadline: Instant) -> Result<String> {
        let name = self.save_name(step)?;
        let path = self.saves.join(format!("{name}.Civ6Save"));
        if !path.is_file() || fs::metadata(&path)?.len() == 0 {
            return Err(RunError::new("save to load is absent or empty"));
        }
        let before = self.loadscreen_count()?;
        let code = format!(
            "if not UI.IsInFrontEnd() then error('main menu required') end\nlocal ok = Network.LoadGame({{Name={}, Location=SaveLocations.LOCAL_STORAGE, Type=SaveTypes.SINGLE_PLAYER, Directory=SaveDirectories.DEFAULT, IsAutosave=false, IsQuicksave=false}}, ServerType.SERVER_TYPE_NONE)\nif ok == false then error('Network.LoadGame returned false') end\nreturn true",
            scripts::quote(&name)
        );
        match self.lua("FrontEnd", &code, deadline) {
            Ok(_) => {}
            Err(e) if e.unknown => {
                println!(
                    "LoadGame acknowledgment interrupted; observing transition without replay"
                );
                self.client = None;
            }
            Err(e) => return Err(e),
        }
        self.enter_game(before, deadline)?;
        Ok(format!("loaded {name}"))
    }
    fn autoplay(
        &mut self,
        turns: i64,
        deadline: Instant,
        start_override: Option<i64>,
    ) -> Result<String> {
        self.lua("InGame","if LuaEvents.JAC_Notebooks_Close then LuaEvents.JAC_Notebooks_Close() end\nreturn true",deadline)?;
        self.dismiss(deadline)?;
        let start = match start_override {
            Some(v) => v,
            None => self
                .lua(
                    "GameCore_Tuner",
                    "return Game.GetCurrentGameTurn()",
                    deadline,
                )?
                .parse()
                .map_err(|_| RunError::new("invalid authoritative turn"))?,
        };
        let value = self.lua("InGame", &scripts::autoplay_start(turns, start), deadline)?;
        let first = parse_ints(&value, 2)?;
        if first[0] != start || first[1] < 0 {
            return Err(RunError::new(
                "autoplay did not start from the authoritative clock and human",
            ));
        }
        let human = first[1];
        let target = start
            .checked_add(turns)
            .ok_or_else(|| RunError::new("turn overflow"))?;
        let mut inactive = None;
        let mut resumes = 0;
        let mut dedications = 0;
        let mut closes = 0;
        let mut diplomacy_since = false;
        let mut alive_turn = None;
        let final_turn = loop {
            duration(deadline, 1.)?;
            let closed = self.popups(deadline)?;
            closes += closed.len();
            if !closed.is_empty() {
                diplomacy_since = true;
                self.log("autoplay_diplomacy", "OK", &closed.join(", "))?;
            }
            if closes > 20 + 5 * (turns as usize) {
                return Err(RunError::new("diplomacy keeps reopening during autoplay"));
            }
            let status = parse_autoplay(&self.idempotent(
                "InGame",
                scripts::AUTO_STATUS,
                deadline.min(Instant::now() + Duration::from_secs(30)),
            )?)?;
            if status.turn < start {
                sleep(deadline, 0.1)?;
                continue;
            }
            if status.turn > target {
                return Err(RunError::new(format!(
                    "autoplay overshot {target}: {}",
                    status.turn
                )));
            }
            if alive_turn != Some(status.turn) || status.local < 0 {
                let alive = self.lua(
                    "InGame",
                    &format!("return tostring(Players[{human}]:IsAlive())"),
                    deadline,
                )?;
                if alive != "true" {
                    return Err(RunError::new(format!(
                        "human eliminated or unknown alive state: {alive}"
                    )));
                }
                alive_turn = Some(status.turn);
            }
            if status.turn == target && !status.active && status.event && status.local == human {
                break status.turn;
            }
            self.dismiss(deadline)?;
            if !status.active && status.turn < target {
                let since = *inactive.get_or_insert_with(Instant::now);
                if since.elapsed() >= Duration::from_secs(5) && status.local == human {
                    let mut chose = false;
                    if dedications < 3 {
                        let choices = self.lua("InGame", &scripts::dedication(human), deadline)?;
                        if choices != "none" && !choices.is_empty() {
                            dedications += 1;
                            chose = true;
                            self.log("autoplay_dedication", "OK", &choices)?;
                            platform::focus_and_key(0x1b, deadline)?;
                        }
                    }
                    if !chose && resumes >= 2 && !diplomacy_since {
                        return Err(RunError::new(format!(
                            "autoplay stopped early at {}/{target}",
                            status.turn
                        )));
                    }
                    self.lua("InGame",&format!("JAC_AUTOTEST_AUTO_DONE = false\nAutoplayManager.SetTurns({})\nAutoplayManager.SetReturnAsPlayer({human})\nAutoplayManager.SetObserveAsPlayer(PlayerTypes.NONE)\nAutoplayManager.SetActive(true)\nreturn true",target-status.turn),deadline)?;
                    if !chose && !diplomacy_since {
                        resumes += 1;
                    }
                    diplomacy_since = false;
                    inactive = None;
                }
            } else {
                inactive = None;
            }
            sleep(deadline, 0.5)?;
        };
        self.idempotent("InGame", scripts::AUTO_CLEANUP, deadline)?;
        Ok(format!(
            "autoplay completed {turns} turns ({start}->{final_turn}; event=true)"
        ))
    }
    fn fast_forward(&mut self, turns: i64, deadline: Instant) -> Result<String> {
        if self.scenario.get("test_mode").is_none() {
            return Err(RunError::new("test mode required"));
        }
        self.lua("InGame","if GameConfiguration.GetRuleSet() ~= 'RULESET_JAC_BANSEIKI' then error('test ruleset required') end\nif GameConfiguration.IsAnyMultiplayer() then error('single-player test required') end\nreturn true",deadline)?;
        let mut state = parse_ints(&self.lua("InGame", scripts::ERA_STATE, deadline)?, 4)?;
        let target = state[0]
            .checked_add(turns)
            .ok_or_else(|| RunError::new("turn overflow"))?;
        let mut skipped = 0;
        while state[0] < target {
            let boundary = if state[3] > 0 {
                state[0] + state[3]
            } else {
                state[1]
            };
            let limit = if state[2] < 0 {
                target - 2
            } else {
                (target - 2).min(boundary - 2)
            };
            if limit > state[0] {
                let current = state[0];
                let code = format!(
                    "local ok, setter = pcall(function() return Game.SetCurrentGameTurn end)\nif not ok or type(setter) ~= 'function' then error('engine fast-forward unavailable') end\nif Game.GetCurrentGameTurn() ~= {current} then error('turn changed before fast-forward') end\nlocal target = {limit}\nGame.SetCurrentGameTurn(target)\nif Game.GetCurrentGameTurn() ~= target then error('engine fast-forward failed') end\nreturn tostring(target)"
                );
                match self.lua("GameCore_Tuner", &code, deadline) {
                    Ok(v) if v == limit.to_string() => {}
                    Ok(v) => {
                        return Err(RunError::new(format!(
                            "unexpected fast-forward response {v}"
                        )));
                    }
                    Err(e) if e.unknown => {
                        self.scenario["test_mode"]["acknowledged_skipped_turns"] =
                            self.scenario["test_mode"]["skipped_turns"].clone();
                        self.scenario["test_mode"]["skipped_turns"] = Value::Null;
                        self.scenario["test_mode"]["unconfirmed_jump"] =
                            json!({"from":current,"to":limit});
                        return Err(e);
                    }
                    Err(e) => return Err(e),
                }
                skipped += limit - current;
                let total = self.scenario["test_mode"]["skipped_turns"]
                    .as_i64()
                    .ok_or_else(|| RunError::new("invalid skip accounting"))?;
                self.scenario["test_mode"]["skipped_turns"] = json!(total + limit - current);
                state[0] = limit;
            }
            let before = state[0];
            self.autoplay(2.min(target - before), deadline, Some(before))?;
            state = parse_ints(&self.lua("InGame", scripts::ERA_STATE, deadline)?, 4)?;
            if state[0] <= before || state[0] > target {
                return Err(RunError::new("fast-forward turn advance mismatch"));
            }
        }
        Ok(format!(
            "TEST MODE: reached turn {target}; skipped {skipped} turns; era boundaries processed normally"
        ))
    }
    fn step(&mut self, step: &Value) -> Result<String> {
        let action = text(step, "action")?;
        let timeout = step
            .get("timeout")
            .and_then(Value::as_f64)
            .unwrap_or(if action == "restart_game" { 360. } else { 120. });
        let deadline = Instant::now() + Duration::from_secs_f64(timeout);
        match action {
            "restart_game" => self.restart(deadline),
            "enable_mods" => {
                let ids = string_list(&step["ids"])?;
                let disabled = step
                    .get("disable_ids")
                    .map(string_list)
                    .transpose()?
                    .unwrap_or_default();
                let result =
                    self.lua("FrontEnd", &scripts::enable_mods(&ids, &disabled), deadline)?;
                Ok(format!(
                    "enabled/verified {}; disabled {result}",
                    ids.join(", ")
                ))
            }
            "configure" => {
                self.lua("FrontEnd", &scripts::configure(step), deadline)?;
                let ruleset = text(step, "ruleset")?;
                if step
                    .get("build_setup")
                    .and_then(Value::as_bool)
                    .unwrap_or(ruleset == "RULESET_JAC_BANSEIKI")
                {
                    self.lua(
                        "HostGame",
                        &scripts::build_setup(step["players"].as_array().unwrap()),
                        deadline,
                    )?;
                }
                let flag = if self.scenario.get("test_mode").is_some() {
                    1
                } else {
                    0
                };
                self.lua("FrontEnd",&format!("GameConfiguration.SetValue('JAC_AUTOTEST', {flag})\nGameConfiguration.UpdateEnabledMods()\nreturn true"),deadline)?;
                self.ruleset = Some(ruleset.to_string());
                Ok(format!(
                    "{ruleset}; Create Game setup built and player slots checked"
                ))
            }
            "start_game" => {
                let before = self.loadscreen_count()?;
                match self.lua("FrontEnd","if not UI.IsInFrontEnd() then error('main menu required') end\nNetwork.HostGame(ServerType.SERVER_TYPE_NONE)\nreturn true",deadline){Ok(_)=>{},Err(e) if e.unknown=>{println!("HostGame response interrupted; observe transition without replay");self.client=None;},Err(e)=>return Err(e)}
                self.enter_game(before, deadline)?;
                if self.ruleset.as_deref() == Some("RULESET_JAC_BANSEIKI")
                    && self.lua(
                        "InGame",
                        "return tostring(GameConfiguration.GetStartTurn())",
                        deadline,
                    )? != "1"
                {
                    return Err(RunError::new("Banseiki must start at turn 1"));
                }
                Ok("single-player game reached InGame; start turn verified".into())
            }
            "ui_lua" => {
                let state = text(step, "state_name")?;
                let code = text(step, "code")?;
                let mut attempts = 0;
                let value = loop {
                    match self.lua(state, code, deadline) {
                        Ok(v) => break v,
                        Err(e)
                            if !e.unknown
                                && !e.lua
                                && state.eq_ignore_ascii_case("InGame")
                                && attempts < 3 =>
                        {
                            if self.popups(deadline)?.is_empty() {
                                return Err(e);
                            }
                            attempts += 1;
                        }
                        Err(e) => return Err(e),
                    }
                };
                if let Some(k) = step["equals_capture"].as_str()
                    && self.captures.get(k) != Some(&value)
                {
                    return Err(RunError::new(format!("ui_lua snapshot mismatch for {k}")));
                }
                if let Some(k) = step["capture_as"].as_str() {
                    self.captures.insert(k.into(), value.clone());
                }
                Ok(format!("ran Lua in {state}; return={value}"))
            }
            "wait_for" => {
                self.wait(&step["condition"], deadline, false)?;
                Ok(format!("condition met: {}", step["condition"]))
            }
            "save" => self.save(step, deadline),
            "load" => self.load(step, deadline),
            "exit_to_menu" => {
                match self.lua("InGame", "Events.ExitToMainMenu()\nreturn true", deadline) {
                    Ok(_) => {}
                    Err(e) if e.unknown => {
                        self.client = None;
                    }
                    Err(e) => return Err(e),
                }
                self.client = None;
                self.wait(&json!({"type":"state","name":"FrontEnd"}), deadline, false)?;
                self.wait(&json!({"type":"lua","state_name":"FrontEnd","code":"return UI.IsInFrontEnd()"}),deadline,false)?;
                Ok("main menu reached".into())
            }
            "autoplay" => {
                if step["fast_forward"] == true {
                    self.fast_forward(step["turns"].as_i64().unwrap(), deadline)
                } else {
                    self.autoplay(step["turns"].as_i64().unwrap(), deadline, None)
                }
            }
            "end_turn" => {
                let turns = if self.scenario.get("test_mode").is_some() {
                    step.get("test_turns").or_else(|| step.get("turns"))
                } else {
                    step.get("turns")
                }
                .and_then(Value::as_i64)
                .unwrap_or(1);
                let mut details = Vec::new();
                for _ in 0..turns {
                    self.lua("InGame","if LuaEvents.JAC_Notebooks_Close then LuaEvents.JAC_Notebooks_Close() end\nreturn true",deadline)?;
                    self.dismiss(deadline)?;
                    let start = self
                        .lua("InGame", "return Game.GetCurrentGameTurn()", deadline)?
                        .parse::<i64>()
                        .map_err(|_| RunError::new("invalid turn"))?;
                    self.popups(deadline)?;
                    self.lua(
                        "InGame",
                        "UI.RequestAction(ActionTypes.ACTION_ENDTURN)\nreturn true",
                        deadline,
                    )?;
                    self.wait(&json!({"type":"lua","state_name":"InGame","code":format!("return Game.GetCurrentGameTurn() == {} and Game.GetLocalPlayer() >= 0",start+1)}),deadline,true)?;
                    self.dismiss(deadline)?;
                    details.push(format!("human turn {start}->{}", start + 1));
                }
                Ok(details.join("; "))
            }
            "pause" => {
                let secs = step["seconds"].as_f64().unwrap();
                if Duration::from_secs_f64(secs) > duration(deadline, 86400.)? {
                    return Err(RunError::new("pause exceeds step deadline"));
                }
                thread::sleep(Duration::from_secs_f64(secs));
                Ok(format!("paused {secs}s"))
            }
            _ => Err(RunError::new("unimplemented action")),
        }
    }
    fn run(mut self) -> Result<i32> {
        let name = text(&self.scenario, "name")?.to_string();
        self.log("start", "OK", &name)?;
        self.log(
            "driver",
            "OK",
            "Rust native runner; no Python interpreter or module import",
        )?;
        let outcome = self.execute();
        if let Err(e) = &outcome {
            if self.failure.is_none() {
                self.log("run", "FAIL", &e.to_string())?;
            }
            self.passed = false;
        }
        self.client = None;
        let mut artifact_error = None;
        if !self.baseline.is_null() {
            match self.read_logs(&self.baseline) {
                Ok(logs) => {
                    for (name, bytes) in logs {
                        if let Err(e) = fs::write(self.output.join(name), bytes) {
                            artifact_error = Some(e.to_string());
                            break;
                        }
                    }
                }
                Err(e) => artifact_error = Some(e.to_string()),
            }
        }
        if let Some(error) = &artifact_error {
            self.passed = false;
            if self.failure.is_none() {
                self.failure = Some(format!("log artifacts: {error}"));
            }
        }
        let result = json!({"scenario":self.scenario["name"],"driver":"Rust native","passed":self.passed,"test_mode":self.scenario.get("test_mode"),"completed_steps":self.completed,"failure":self.failure,"artifact_error":artifact_error,"checks":self.checks,"metrics":{"autoplay":[],"groups":{},"save_load":{"samples":[]},"continuous_performance_measured":false}});
        fs::write(
            self.output.join("result.json"),
            serde_json::to_vec_pretty(&result)?,
        )?;
        Ok(if self.passed && outcome.is_ok() { 0 } else { 1 })
    }
    fn execute(&mut self) -> Result<()> {
        self.refresh_baseline()?;
        let steps = self.scenario["steps"].as_array().unwrap().clone();
        if steps[0]["action"] != "restart_game"
            && platform::single_civ_process(Duration::ZERO)?.is_none()
        {
            return Err(RunError::new(
                "explicit restart_game required when Civ6 is absent",
            ));
        }
        for (i, step) in steps.iter().enumerate() {
            let action = format!(
                "{}/{} {}",
                i + 1,
                steps.len(),
                step["action"].as_str().unwrap()
            );
            self.log(&action, "START", &step.to_string())?;
            if step["action"] != "wait_for" {
                self.step_baseline = logs::snapshot(&self.log_dir)?;
                self.consumed.clear();
            }
            match self.step(step) {
                Ok(detail) => {
                    if step["action"] == "restart_game" {
                        self.refresh_baseline()?;
                        self.step_baseline = self.baseline.clone();
                        self.consumed.clear();
                    }
                    self.log(&action, "OK", &detail)?;
                    self.completed += 1;
                }
                Err(e) => {
                    self.log(&action, "FAIL", &e.to_string())?;
                    return Err(e);
                }
            }
        }
        let logs = self.read_logs(&self.baseline)?;
        let required = self
            .scenario
            .get("require_logs")
            .map(string_list)
            .transpose()?
            .unwrap_or_default();
        let checks = logs::judge_generic(&logs, &required)?;
        if checks.len() < 3 {
            return Err(RunError::new(
                "generic judge must contain all original checks",
            ));
        }
        self.passed = checks.iter().all(|c| c.passed);
        for c in &checks {
            println!(
                "{} {} {}",
                if c.passed { "PASS" } else { "FAIL" },
                c.name,
                c.detail
            );
        }
        if self.scenario["strict_lua_errors"] == true {
            let errors = logs["Lua.log"]
                .lines()
                .filter(|s| {
                    ["Runtime Error", "UI.DataError", "EraComplete cannot show"]
                        .iter()
                        .any(|m| s.contains(m))
                })
                .count();
            if errors > 0 {
                self.passed = false;
            }
            self.checks = serde_json::to_value(&checks)?;
            self.checks.as_array_mut().unwrap().push(json!({"name":"Scenario: all Lua errors","passed":errors==0,"detail":format!("{errors} error lines including official UI")}));
        } else {
            self.checks = serde_json::to_value(&checks)?;
        }
        self.log(
            "judge",
            if self.passed { "OK" } else { "FAIL" },
            &format!("{} checks completed", self.checks.as_array().unwrap().len()),
        )?;
        Ok(())
    }
}

fn parse_ints(value: &str, n: usize) -> Result<Vec<i64>> {
    let v = value
        .split('|')
        .map(|p| {
            p.parse::<i64>()
                .map_err(|_| RunError::new(format!("invalid integer response {value}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if v.len() != n {
        return Err(RunError::new(format!("expected {n} fields, got {value}")));
    }
    Ok(v)
}
#[derive(Debug)]
struct AutoplayStatus {
    turn: i64,
    active: bool,
    event: bool,
    remaining: i64,
    local: i64,
}
fn parse_autoplay(value: &str) -> Result<AutoplayStatus> {
    let v = value.split('|').collect::<Vec<_>>();
    if v.len() != 5 || !["true", "false"].contains(&v[1]) || !["true", "false"].contains(&v[2]) {
        return Err(RunError::new("invalid autoplay status"));
    }
    let num = |i: usize| {
        v[i].parse::<i64>()
            .map_err(|_| RunError::new("invalid autoplay integer"))
    };
    let status = AutoplayStatus {
        turn: num(0)?,
        active: v[1] == "true",
        event: v[2] == "true",
        remaining: num(3)?,
        local: num(4)?,
    };
    if status.remaining < 0 {
        return Err(RunError::new("negative remaining autoplay turns"));
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    const JAPAN: &str = include_str!("../../civ6-autotest/scenarios/absorption-japan.json");
    const HERO: &str = include_str!("../../civ6-autotest/scenarios/absorption-hero-recall.json");
    const PRIEST: &str =
        include_str!("../../civ6-autotest/scenarios/absorption-priest-diagnostic.json");
    #[test]
    fn all_acceptance_steps_and_lua_compile_without_running_engine() {
        for (source, count) in [(JAPAN, 127), (HERO, 101), (PRIEST, 13)] {
            let s: Value = serde_json::from_str(source).unwrap();
            assert_eq!(s["steps"].as_array().unwrap().len(), count);
            validate(&s).unwrap();
        }
    }
    #[test]
    fn reject_unported_actions_and_measurements_before_game_access() {
        let mut s: Value = serde_json::from_str(PRIEST).unwrap();
        s["steps"][0]["action"] = json!("screenshot");
        assert!(
            validate(&s)
                .unwrap_err()
                .to_string()
                .contains("unsupported action")
        );
        s = serde_json::from_str(PRIEST).unwrap();
        s["steps"][0]["metrics"] = json!(true);
        assert!(
            validate(&s)
                .unwrap_err()
                .to_string()
                .contains("not supported")
        );
    }
    #[test]
    fn bad_inputs_cannot_enter_lua_or_save_paths() {
        let mut s: Value = serde_json::from_str(PRIEST).unwrap();
        s["steps"][1]["players"][0]["leader"] = json!("');Network.HostGame();--");
        assert!(validate(&s).is_err());
        assert!(!safe_name("../overwrite"));
        assert!(!safe_name("C:\\save"));
        assert!(safe_name("jac-save-12"));
        assert!(lua_syntax("return (broken").is_err());
        assert!(lua_syntax("return '\0'").is_err());
    }
    #[test]
    fn autoplay_status_is_strict() {
        let s = parse_autoplay("84|false|true|0|1").unwrap();
        assert_eq!(s.turn, 84);
        assert!(!s.active);
        assert!(s.event);
        assert_eq!(s.local, 1);
        for bad in [
            "84|false|true|0",
            "84|FALSE|true|0|1",
            "84|false|true|-1|1",
            "bad|false|true|0|1",
        ] {
            assert!(parse_autoplay(bad).is_err());
        }
    }
    #[test]
    fn save_paths_preserve_matching_names_and_do_not_overwrite() {
        let dir = std::env::temp_dir().join(format!(
            "native-save-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let s: Value = serde_json::from_str(PRIEST).unwrap();
        let mut r = Runner::new(
            s,
            dir.clone(),
            dir.join("logs"),
            dir.join("saves"),
            dir.join("mods"),
            4318,
        )
        .unwrap();
        let save = json!({"name":"same-name"});
        let generated = r.save_name(&save).unwrap();
        assert!(safe_name(&generated));
        fs::create_dir_all(&r.saves).unwrap();
        let existing = r.saves.join(format!("{generated}.Civ6Save"));
        fs::write(&existing, b"existing save must survive").unwrap();
        let error = r
            .save(&save, Instant::now() + Duration::from_secs(1))
            .unwrap_err();
        assert!(error.to_string().contains("refusing overwrite"));
        assert_eq!(fs::read(&existing).unwrap(), b"existing save must survive");
        assert_eq!(
            r.save_name(&json!({"name":"exact-name","exact_name":true}))
                .unwrap(),
            "exact-name"
        );
        let resolved = fs::canonicalize(&dir).unwrap();
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        assert!(resolved.starts_with(&temp) && resolved != temp);
        fs::remove_dir_all(&resolved).unwrap();
    }
}
