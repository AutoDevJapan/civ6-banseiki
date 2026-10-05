//! Independently verify the original persona scenario's recorded execution.
//! This proves scenario completion only, not product startup, AI payment or MP.
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const ORIGINAL: &str = include_str!("../../civ6-autotest/scenarios/leader-persona-15km.json");
const CHECKS: [&str; 6] = [
    "Generic: JAC Modding.log errors",
    "Generic: JAC Database.log errors",
    "Generic: [JAC_POC] errors",
    "Generic: JAC Runtime Error",
    "Generic: JAC missing texture",
    "Scenario: all Lua errors",
];

#[derive(Debug, Serialize)]
pub struct Report {
    pub scenario_completed: bool,
    pub completed_steps: usize,
    pub matched_save_load_snapshots: usize,
    pub early_ai_check: bool,
    pub final_ai_check: bool,
    pub official_lua_errors: usize,
    pub scenario_sha256: String,
    pub steps_sha256: String,
    pub result_sha256: String,
    pub product_acceptance: bool,
    pub scope: &'static str,
}

fn require(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeReceipt {
    pid: u32,
    created_filetime: u64,
}

fn lua_return<'a>(detail: &'a str, state: &str) -> Result<&'a str> {
    let raw = detail
        .strip_prefix(&format!("ran Lua in {state}; return="))
        .ok_or("Lua result has no raw return value")?;
    // Runtime observations append this receipt AFTER capture comparison. It is
    // not part of the Lua value and may be absent after the observer stops.
    if let Some((value, receipt)) = raw.rsplit_once("; native_process=") {
        let receipt: NativeReceipt = serde_json::from_str(receipt)?;
        require(
            receipt.pid > 0 && receipt.created_filetime > 0,
            "invalid native process receipt",
        )?;
        Ok(value)
    } else {
        Ok(raw)
    }
}

/// Read each input from one artifact directory, never from combined runs.
pub fn verify_directory(directory: &Path) -> Result<Report> {
    let steps = std::fs::read_to_string(directory.join("steps.jsonl"))?;
    let result = std::fs::read_to_string(directory.join("result.json"))?;
    // Require the original log slices, even when empty: absent is not error-free.
    let lua = std::fs::read_to_string(directory.join("Lua.log"))?;
    std::fs::read(directory.join("Database.log"))?;
    std::fs::read(directory.join("Modding.log"))?;
    verify(&steps, &result, &lua)
}

pub fn verify(steps_text: &str, result_text: &str, lua: &str) -> Result<Report> {
    let expected: Value = serde_json::from_str(ORIGINAL)?;
    let steps = expected["steps"]
        .as_array()
        .ok_or("missing original steps")?;
    require(steps.len() == 113, "original 113-step contract changed")?;
    let result: Value = serde_json::from_str(result_text)?;
    require(
        result["scenario"] == expected["name"],
        "wrong scenario name",
    )?;
    require(result["passed"] == true, "run did not pass")?;
    require(
        result.get("failure") == Some(&Value::Null),
        "run contains a failure or lacks its final failure field",
    )?;
    require(
        result["completed_steps"].as_u64() == Some(113),
        "incomplete run",
    )?;
    // The original scenario contains real end_turn operations, not clock jumps.
    require(
        result.get("test_mode") == Some(&Value::Null),
        "fast-forwarded run is a different scenario",
    )?;
    let checks = result["checks"].as_array().ok_or("missing judge checks")?;
    let mut names = BTreeSet::new();
    for check in checks {
        let name = check["name"].as_str().ok_or("judge check has no name")?;
        require(check["passed"] == true, format!("judge failed: {name}"))?;
        require(names.insert(name), "duplicate judge check")?;
    }
    require(
        names == CHECKS.into_iter().collect(),
        "judge checks differ from strict generic profile",
    )?;
    require(
        !lua.lines().any(|line| {
            ["Runtime Error", "UI.DataError", "EraComplete cannot show"]
                .iter()
                .any(|pattern| line.contains(pattern))
        }),
        "Lua errors, including official UI",
    )?;

    let mut completed = 0;
    let mut started = false;
    let mut pending = false;
    let mut judged = false;
    let mut captures: BTreeMap<String, String> = BTreeMap::new();
    let mut restored = BTreeSet::new();
    for (line_number, line) in steps_text.lines().enumerate() {
        require(
            !line.trim().is_empty(),
            format!("empty record at line {}", line_number + 1),
        )?;
        let row: Value = serde_json::from_str(line)?;
        let action = row["action"].as_str().ok_or("record has no action")?;
        let status = row["status"].as_str().ok_or("record has no status")?;
        require(
            matches!(status, "START" | "OK" | "SKIP"),
            format!("non-success record: {action}/{status}"),
        )?;
        let detail = row["detail"].as_str().ok_or("record has no detail")?;
        if action == "start" {
            require(
                !started
                    && completed == 0
                    && !pending
                    && status == "OK"
                    && Some(detail) == expected["name"].as_str(),
                "run started twice or names differ",
            )?;
            started = true;
            continue;
        }
        require(started, "records precede run start")?;
        if action == "judge" {
            require(
                !judged
                    && !pending
                    && completed == 113
                    && status == "OK"
                    && detail == "6/6 checks passed",
                "judge absent, early, repeated or incomplete",
            )?;
            judged = true;
            continue;
        }
        // Auxiliary driver events never satisfy a numbered scenario operation.
        if !action.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        require(
            !judged && completed < steps.len(),
            "numbered records after completion",
        )?;
        let step = &steps[completed];
        let step_action = step["action"].as_str().ok_or("original action absent")?;
        require(
            action == format!("{}/113 {step_action}", completed + 1),
            format!("wrong order or action at step {}", completed + 1),
        )?;
        match status {
            "START" => {
                require(!pending, "step started twice")?;
                let actual: Value = serde_json::from_str(detail)?;
                require(
                    actual == *step,
                    format!("executed step {} differs from original", completed + 1),
                )?;
                pending = true;
            }
            "OK" => {
                require(pending, "step completion without start")?;
                if step_action == "ui_lua" {
                    let state = step["state_name"].as_str().ok_or("Lua state absent")?;
                    let value = lua_return(detail, state)?;
                    if let Some(key) = step["capture_as"].as_str() {
                        require(!value.is_empty(), "empty save snapshot")?;
                        require(
                            captures.insert(key.into(), value.into()).is_none(),
                            "capture overwritten",
                        )?;
                    }
                    if let Some(key) = step["equals_capture"].as_str() {
                        require(
                            captures.get(key).map(String::as_str) == Some(value),
                            format!("save/load snapshot mismatch: {key}"),
                        )?;
                        require(restored.insert(key.to_owned()), "snapshot restored twice")?;
                    }
                    if completed == 38 {
                        require(
                            value == "native AI appointment gate verified after first turn",
                            "early AI result differs",
                        )?;
                    }
                    if completed == 112 {
                        require(
                            value == "no unjoined AI governors or persona Great People",
                            "final AI result differs",
                        )?;
                    }
                }
                pending = false;
                completed += 1;
            }
            _ => return Err("numbered operation skipped".into()),
        }
    }
    require(
        judged && !pending && completed == 113,
        "run never completed all steps and judge",
    )?;
    require(
        captures.len() == 6 && restored.len() == 6,
        "six save/load snapshots required",
    )?;
    Ok(Report {
        scenario_completed: true,
        completed_steps: completed,
        matched_save_load_snapshots: restored.len(),
        early_ai_check: true,
        final_ai_check: true,
        official_lua_errors: 0,
        scenario_sha256: sha(ORIGINAL.as_bytes()),
        steps_sha256: sha(steps_text.as_bytes()),
        result_sha256: sha(result_text.as_bytes()),
        product_acceptance: false,
        scope: "original113 recorded scenario only; product runtime, AI payment, MP and neutralization require separate evidence",
    })
}
