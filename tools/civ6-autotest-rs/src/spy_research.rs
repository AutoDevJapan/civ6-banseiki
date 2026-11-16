//! Native research acceptance is distinct from the later capacity effect.
use crate::{spy_failures, Error, Result};
use serde::{Deserialize, Serialize};
use std::{fs::OpenOptions, io::Write, path::Path};

pub const PHASES: [&str; 4] = [
    "after_acceptance",
    "after_reload",
    "after_turn",
    "capacity_ready",
];

/// Only reads the existing native observers. Neither research nor a turn is sent.
pub fn read() -> String {
    format!(
        r#"local failure=function() {failure} end
local function quote(v)
 return '"'..string.gsub(v,'[%z\1-\31\\"]',function(ch)
  return string.format('\\u%04x',string.byte(ch))
 end)..'"'
end
local errors={{}}
local failureOK,failureRaw=pcall(failure)
if not failureOK then errors[#errors+1]=quote('failures: '..tostring(failureRaw)); failureRaw='null' end
local civicOK,s=pcall(function() return ExposedMembers.JAC_PersonaBoundary.ThirdSpyCivicStatus() end)
if not civicOK then errors[#errors+1]=quote('civic: '..tostring(s)); s=nil end
if civicOK and type(s)~='table' then errors[#errors+1]=quote('civic observer returned '..type(s)); s=nil end
local parts={{}}
for _,key in ipairs({{'stage','turn','civic','civicIndex','civicHash','prerequisiteOwned','owned','progressing','capacity','captured','intentToken'}}) do
 local value=s and s[key] or nil
 if s and s[key]==false then value=false end
 local kind=type(value)
 local encoded='null'
 if kind=='string' then encoded=quote(value)
 elseif kind=='boolean' then encoded=value and 'true' or 'false'
 elseif kind=='number' and value==value and math.abs(value)<math.huge then
  encoded=string.format('%.17g',value)
 elseif value~=nil then errors[#errors+1]=quote('unsupported Civic field '..key..' ('..kind..')') end
 parts[#parts+1]=quote(key)..':'..encoded
end
return '{{"failures":'..failureRaw..',"civic":{{'..table.concat(parts,',')..'}},"observationErrors":['..table.concat(errors,',')..']}}'
"#,
        failure = spy_failures::READ
    )
}

/// Preserve the actual response even if parsing or native observations fail.
pub fn preserve_raw(output: &Path, phase: &str, raw: &str) -> Result<()> {
    if !PHASES.contains(&phase) {
        return Err(problem("unknown inspection phase"));
    }
    let path = output.join(format!("native-spy-civic-{phase}-raw.json"));
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(raw.as_bytes())?;
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Civic {
    pub stage: String,
    pub turn: i64,
    pub civic: String,
    pub civic_index: i64,
    pub civic_hash: i64,
    pub prerequisite_owned: bool,
    pub owned: bool,
    pub progressing: i64,
    pub capacity: usize,
    pub captured: usize,
    pub intent_token: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Stage {
    Researching,
    EffectPending,
    CapacityReady,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub stage: Stage,
    pub civic: Civic,
    pub failures: spy_failures::Report,
}

fn problem(message: &str) -> Error {
    Error::scenario(format!("native third-spy research inspection: {message}"))
}

pub fn diagnose(raw: &str) -> Result<Report> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| problem(&e.to_string()))?;
    if value["observationErrors"]
        .as_array()
        .is_none_or(|errors| !errors.is_empty())
    {
        return Err(problem(
            "native observation errors were retained in the raw record",
        ));
    }
    let civic: Civic = serde_json::from_value(value["civic"].clone())
        .map_err(|e| problem(&format!("incomplete Civic snapshot: {e}")))?;
    let failures = spy_failures::diagnose(&value["failures"].to_string())?;
    if civic.civic != "CIVIC_JAC_SENGOKU_NANBAN_ENVOYS"
        || civic.civic_index < 0
        || civic.intent_token <= 0
        || !civic.prerequisite_owned
        || civic.captured != 2
        || failures.physical_count != 2
        || civic.turn != failures.raw["turn"].as_i64().unwrap_or(-1)
        || civic.capacity != failures.raw["capacity"].as_u64().unwrap_or(0) as usize
    {
        return Err(problem(
            "saved intent, turn, two failures or captive roster changed",
        ));
    }
    let stage = match (
        civic.owned,
        civic.capacity,
        civic.progressing == civic.civic_index,
    ) {
        (false, 2, true) => Stage::Researching,
        (true, 2, _) => Stage::EffectPending,
        (true, 3, _) => Stage::CapacityReady,
        _ => return Err(problem("native research acceptance has not been observed")),
    };
    let name = match stage {
        Stage::Researching => "RESEARCHING",
        Stage::EffectPending => "EFFECT_PENDING",
        Stage::CapacityReady => "CAPACITY_READY",
    };
    if civic.stage != name {
        return Err(problem("UI stage differs from independent native fields"));
    }
    Ok(Report {
        stage,
        civic,
        failures,
    })
}

/// A changed capacity is valid; changing the saved request or old failures is not.
pub fn check_reload(before: &serde_json::Value, after: &Report) -> Result<()> {
    let after = serde_json::to_value(after).map_err(|e| problem(&e.to_string()))?;
    for key in ["intentToken", "civic", "civicIndex", "civicHash", "turn"] {
        let old = before["civic"]
            .get(key)
            .ok_or_else(|| problem("prior Civic identity missing"))?;
        if old != &after["civic"][key] {
            return Err(problem("saved Civic intent changed after reload"));
        }
    }
    for key in ["failures", "archive", "spy", "attacker", "defender"] {
        let old = before["failures"]["raw"]
            .get(key)
            .ok_or_else(|| problem("prior native failure identity missing"))?;
        if old != &after["failures"]["raw"][key] {
            return Err(problem("native failed attempt changed after reload"));
        }
    }
    Ok(())
}
