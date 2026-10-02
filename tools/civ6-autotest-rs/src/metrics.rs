//! Turn timing, process memory, save/load measurements and research samples.
use crate::{time, Error, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
};

fn append_jsonl(path: &PathBuf, row: &Value) -> Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, row)?;
    file.write_all(b"\n")?;
    Ok(())
}
fn display(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::Bool(b)) => {
            if *b {
                "True".into()
            } else {
                "False".into()
            }
        }
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
    }
}
fn append_csv(path: &PathBuf, headers: &[&str], row: &[String]) -> Result<()> {
    let new = !path.exists();
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(file);
    if new {
        writer.write_record(headers)?;
    }
    writer.write_record(row)?;
    writer.flush()?;
    Ok(())
}
fn round3(n: f64) -> f64 {
    (n * 1000.0).round() / 1000.0
}
fn numeric(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}
fn statistics(values: &[Value]) -> Value {
    if values.is_empty() {
        return json!({"count":0,"min":null,"median":null,"p95":null,"max":null});
    }
    let mut ordered = values.to_vec();
    ordered.sort_by(|a, b| numeric(a).total_cmp(&numeric(b)));
    let len = ordered.len();
    let median = if len % 2 == 1 {
        ordered[len / 2].clone()
    } else {
        json!((numeric(&ordered[len / 2 - 1]) + numeric(&ordered[len / 2])) / 2.0)
    };
    json!({"count":len,"min":ordered[0],"median":median,"p95":ordered[(len*95).div_ceil(100)-1],"max":ordered[len-1]})
}
fn memory_stats(rows: &[Value], key: &str) -> Value {
    let values: Vec<_> = rows
        .iter()
        .filter_map(|r| r.get("memory").and_then(|m| m.get(key)).cloned())
        .collect();
    let mut result = statistics(&values);
    result["first"] = values.first().cloned().unwrap_or(Value::Null);
    result["last"] = values.last().cloned().unwrap_or(Value::Null);
    result["growth"] = if let (Some(a), Some(b)) = (values.first(), values.last()) {
        json!(b.as_i64().unwrap_or(0) - a.as_i64().unwrap_or(0))
    } else {
        Value::Null
    };
    result
}
pub fn summarize_turns(rows: &[Value]) -> Value {
    let turns: Vec<_> = rows.iter().filter(|r| r["phase"] == "turn").collect();
    let durations: Vec<_> = turns
        .iter()
        .filter_map(|r| r.get("duration_seconds").filter(|v| !v.is_null()).cloned())
        .collect();
    json!({"observed_turns":turns.iter().filter(|r|r["observed"]==true).count(),
        "missed_turns":turns.iter().filter(|r|r["observed"]==false).count(),
        "duration_seconds":statistics(&durations),"working_set_bytes":memory_stats(rows,"working_set_bytes"),
        "private_bytes":memory_stats(rows,"private_bytes")})
}

pub struct TurnMetrics {
    pub path: PathBuf,
    pub csv_path: PathBuf,
    pub start_turn: i64,
    pub turn: i64,
    pub observed_turns: usize,
    pub missed_turns: usize,
    pub rows: Vec<Value>,
    start_at: f64,
    last_at: f64,
    test_mode: bool,
}
impl TurnMetrics {
    pub const COLUMNS: [&str; 10] = [
        "phase",
        "at",
        "turn",
        "observed",
        "duration_seconds",
        "elapsed_seconds",
        "working_set_bytes",
        "private_bytes",
        "test_mode",
        "continuous_performance_valid",
    ];
    pub fn new(path: PathBuf, start_turn: i64, started_at: f64, test_mode: bool) -> Self {
        Self {
            csv_path: path.with_extension("csv"),
            path,
            start_turn,
            turn: start_turn,
            observed_turns: 0,
            missed_turns: 0,
            rows: Vec::new(),
            start_at: started_at,
            last_at: started_at,
            test_mode,
        }
    }
    fn write(&mut self, mut row: Value) -> Result<()> {
        row["test_mode"] = json!(self.test_mode);
        row["continuous_performance_valid"] = json!(!self.test_mode);
        append_jsonl(&self.path, &row)?;
        let values: Vec<_> = Self::COLUMNS
            .iter()
            .map(|key| {
                if *key == "working_set_bytes" || *key == "private_bytes" {
                    display(row.get("memory").and_then(|m| m.get(*key)))
                } else {
                    display(row.get(*key))
                }
            })
            .collect();
        append_csv(&self.csv_path, &Self::COLUMNS, &values)?;
        self.rows.push(row);
        Ok(())
    }
    pub fn start(&mut self, memory: Value) -> Result<()> {
        self.write(
            json!({"phase":"start","at":time::utc_now(),"turn":self.turn,"observed":true,
            "elapsed_seconds":0,"memory":memory}),
        )
    }
    pub fn sample_at(&mut self, turn: i64, memory: Value, now: f64) -> Result<()> {
        if turn < self.turn {
            return Err(Error::input(format!(
                "Turn moved backwards: {} -> {turn}",
                self.turn
            )));
        }
        if turn == self.turn {
            return Ok(());
        }
        let delta = turn - self.turn;
        for value in self.turn + 1..turn {
            self.write(
                json!({"phase":"turn","at":time::utc_now(),"turn":value,"observed":false,
                "duration_seconds":null,"elapsed_seconds":round3(now-self.start_at),"memory":null}),
            )?;
            self.missed_turns += 1;
        }
        self.write(
            json!({"phase":"turn","at":time::utc_now(),"turn":turn,"observed":true,
            "duration_seconds":if delta==1{json!(round3(now-self.last_at))}else{Value::Null},
            "elapsed_seconds":round3(now-self.start_at),"memory":memory}),
        )?;
        self.observed_turns += 1;
        self.turn = turn;
        self.last_at = now;
        Ok(())
    }
    pub fn summary(&self) -> Value {
        let mut result = summarize_turns(&self.rows);
        result["jsonl"] = json!(self.path.file_name().unwrap_or_default().to_string_lossy());
        result["csv"] = json!(self
            .csv_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy());
        result["start_turn"] = json!(self.start_turn);
        result["last_turn"] = json!(self.turn);
        result
    }
}

pub struct SaveLoadMetrics {
    pub path: PathBuf,
    pub rows: Vec<Value>,
    test_mode: bool,
}
impl SaveLoadMetrics {
    pub const COLUMNS: [&str; 8] = [
        "at",
        "action",
        "save_name",
        "turn",
        "save_bytes",
        "duration_seconds",
        "test_mode",
        "continuous_performance_valid",
    ];
    pub fn new(path: PathBuf, test_mode: bool) -> Self {
        Self {
            path,
            rows: Vec::new(),
            test_mode,
        }
    }
    pub fn record(
        &mut self,
        action: &str,
        name: &str,
        turn: i64,
        size: u64,
        seconds: f64,
    ) -> Result<Value> {
        let row = json!({"at":time::utc_now(),"action":action,"save_name":name,"turn":turn,"save_bytes":size,
            "duration_seconds":round3(seconds),"test_mode":self.test_mode,"continuous_performance_valid":!self.test_mode});
        let values: Vec<_> = Self::COLUMNS
            .iter()
            .map(|key| display(row.get(*key)))
            .collect();
        append_csv(&self.path, &Self::COLUMNS, &values)?;
        self.rows.push(row.clone());
        Ok(row)
    }
    pub fn summary(&self) -> Value {
        let saves: Vec<_> = self
            .rows
            .iter()
            .filter(|r| r["action"] == "save")
            .map(|r| r["save_bytes"].clone())
            .collect();
        let loads: Vec<_> = self
            .rows
            .iter()
            .filter(|r| r["action"] == "load")
            .map(|r| r["duration_seconds"].clone())
            .collect();
        let mut save_stats = statistics(&saves);
        save_stats["growth"] = if let (Some(a), Some(b)) = (saves.first(), saves.last()) {
            json!(b.as_i64().unwrap_or(0) - a.as_i64().unwrap_or(0))
        } else {
            Value::Null
        };
        json!({"csv":if self.rows.is_empty(){Value::Null}else{json!(self.path.file_name().unwrap_or_default().to_string_lossy())},
            "samples":self.rows,"save_bytes":save_stats,"load_seconds":statistics(&loads)})
    }
}

pub const RESEARCH_SAMPLE_CODE: &str = r#"
local rows = {}
local configured = nil
local function current(info, index, key)
  if index == nil or index < 0 then return '' end
  local row = info[index]
  return row and row[key] or ''
end
for _, id in ipairs(configured or GameConfiguration.GetParticipatingPlayerIDs()) do
  local player = Players[id]
  if player and (configured or player:IsMajor()) then
    local techs, civics = {}, {}
    local research, culture = player:GetTechs(), player:GetCulture()
    for tech in GameInfo.Technologies() do
      if research:HasTech(tech.Index) then techs[#techs + 1] = tech.TechnologyType end
    end
    for civic in GameInfo.Civics() do
      if culture:HasCivic(civic.Index) then civics[#civics + 1] = civic.CivicType end
    end
    table.sort(techs)
    table.sort(civics)
    rows[#rows + 1] = table.concat({id,
      PlayerConfigurations[id]:GetLeaderTypeName(), tostring(player:IsAlive()),
      #techs, #civics,
      current(GameInfo.Technologies, research:GetResearchingTech(), 'TechnologyType'),
      current(GameInfo.Civics, culture:GetProgressingCivic(), 'CivicType'),
      table.concat(techs, ','), table.concat(civics, ',')}, '~')
  end
end
return tostring(Game.GetCurrentGameTurn()) .. '|' .. table.concat(rows, ';')
"#;

pub fn research_sample_code(slots: &BTreeSet<i64>) -> Result<String> {
    if slots.is_empty() || slots.iter().any(|id| *id < 0) {
        return Err(Error::input("Expected nonempty major slot IDs"));
    }
    let literal = slots
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    Ok(RESEARCH_SAMPLE_CODE.replacen(
        "local configured = nil",
        &format!("local configured = {{{literal}}}"),
        1,
    ))
}
fn type_id(value: &str) -> bool {
    Regex::new(r"^[A-Z][A-Z0-9_]+$").unwrap().is_match(value)
}
pub fn parse_research_sample(payload: &str) -> Result<(i64, Vec<Value>)> {
    let invalid = || {
        Error::input(format!(
            "Invalid research sample: {:?}",
            payload.chars().take(120).collect::<String>()
        ))
    };
    let (turn_text, entries) = payload.split_once('|').ok_or_else(invalid)?;
    let turn = turn_text.parse::<i64>().map_err(|_| invalid())?;
    if turn < 0 || entries.is_empty() {
        return Err(invalid());
    }
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in entries.split(';') {
        let parts: Vec<_> = entry.split('~').collect();
        if parts.len() != 9 {
            return Err(invalid());
        }
        let slot = parts[0].parse::<i64>().map_err(|_| invalid())?;
        let tech_count = parts[3].parse::<usize>().map_err(|_| invalid())?;
        let civic_count = parts[4].parse::<usize>().map_err(|_| invalid())?;
        let techs: Vec<_> = if parts[7].is_empty() {
            vec![]
        } else {
            parts[7].split(',').collect()
        };
        let civics: Vec<_> = if parts[8].is_empty() {
            vec![]
        } else {
            parts[8].split(',').collect()
        };
        let ids = std::iter::once(parts[1])
            .chain(techs.iter().copied())
            .chain(civics.iter().copied())
            .chain([parts[5], parts[6]].into_iter().filter(|s| !s.is_empty()));
        if slot < 0
            || !seen.insert(slot)
            || !matches!(parts[2], "true" | "false")
            || tech_count != techs.len()
            || civic_count != civics.len()
            || ids.into_iter().any(|id| !type_id(id))
        {
            return Err(invalid());
        }
        rows.push(json!({"slot":slot,"leader":parts[1],"alive":parts[2]=="true","tech_count":tech_count,
            "civic_count":civic_count,"current_tech":if parts[5].is_empty(){None}else{Some(parts[5])},
            "current_civic":if parts[6].is_empty(){None}else{Some(parts[6])},
            "completed_techs":techs,"completed_civics":civics}));
    }
    Ok((turn, rows))
}

pub struct ResearchMetrics {
    pub path: PathBuf,
    pub slots: Option<BTreeSet<i64>>,
    pub rows: Vec<Value>,
    pub last_turn: Option<i64>,
}
impl ResearchMetrics {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            slots: None,
            rows: Vec::new(),
            last_turn: None,
        }
    }
    pub fn sample(&mut self, payload: &str) -> Result<i64> {
        let (turn, rows) = parse_research_sample(payload)?;
        let slots: BTreeSet<_> = rows.iter().filter_map(|r| r["slot"].as_i64()).collect();
        if slots.is_empty() {
            return Err(Error::input("No major slots in research sample"));
        }
        if let Some(old) = &self.slots {
            if slots != *old {
                return Err(Error::input(format!(
                    "Major slots changed during research sampling: {old:?} -> {slots:?}"
                )));
            }
        }
        if let Some(old) = self.last_turn {
            if turn < old {
                return Err(Error::input(format!(
                    "Research turn moved backwards: {old} -> {turn}"
                )));
            }
            if turn == old {
                return Ok(turn);
            }
        }
        let phase = if self.last_turn.is_none() {
            "start"
        } else {
            "turn"
        };
        for row in &rows {
            let mut full = row.clone();
            full["phase"] = json!(phase);
            full["turn"] = json!(turn);
            full["observed"] = json!(true);
            append_jsonl(&self.path, &full)?;
        }
        self.slots = Some(slots);
        self.last_turn = Some(turn);
        self.rows = rows;
        Ok(turn)
    }
}

#[cfg(windows)]
pub fn process_memory(pid: u32) -> Result<Value> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
        if handle.is_null() {
            return Err(Error::input(format!(
                "Cannot open Civ VI process {pid} for memory sample"
            )));
        }
        let mut counters: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        let ok = GetProcessMemoryInfo(
            handle,
            &mut counters as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
            counters.cb,
        );
        CloseHandle(handle);
        if ok == 0 {
            return Err(Error::input(format!(
                "Cannot sample Civ VI process {pid} memory"
            )));
        }
        Ok(
            json!({"working_set_bytes":counters.WorkingSetSize,"private_bytes":counters.PrivateUsage}),
        )
    }
}
#[cfg(not(windows))]
pub fn process_memory(_: u32) -> Result<Value> {
    Err(Error::input("Process memory sampling requires Windows"))
}
