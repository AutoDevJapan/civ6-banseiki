//! Independently verify the original persona scenario's recorded execution.
//! This proves scenario completion only, not product startup, AI payment or MP.
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const ORIGINAL: &str = include_str!("../../civ6-autotest/scenarios/leader-persona-15km.json");
const BASELINE: &str =
    include_str!("../../civ6-autotest/scenarios/leader-persona-baseline-15km.json");
pub(crate) const CHECKS: [&str; 6] = [
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
    pub baseline_ready: bool,
    /// Number of original operations; the preparation restart is separate.
    pub completed_steps: usize,
    pub recorded_steps: usize,
    pub preparation_restart: bool,
    pub save_namespace: Option<String>,
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

/// Decode a recorded Lua value while validating the optional host process receipt.
pub fn lua_return<'a>(detail: &'a str, state: &str) -> Result<&'a str> {
    let raw = detail
        .strip_prefix(&format!("ran Lua in {state}; return="))
        .ok_or("Lua result has no raw return value")?;
    // Runtime observations append this receipt AFTER capture comparison. It is
    // not part of the Lua value and may be absent after the observer stops.
    if let Some((value, receipt)) = raw.rsplit_once("; native_process=") {
        // Keep the typed parse so duplicate fields fail. Struct sequences are
        // not the JSON object produced by the runner's wire protocol.
        require(
            receipt.trim_start().starts_with('{'),
            "native process receipt must be a JSON object",
        )?;
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
    verify_artifact(directory, false)
}

pub fn verify_baseline_directory(directory: &Path) -> Result<Report> {
    verify_artifact(directory, true)
}

fn verify_artifact(directory: &Path, baseline: bool) -> Result<Report> {
    let steps = std::fs::read_to_string(directory.join("steps.jsonl"))?;
    let result = std::fs::read_to_string(directory.join("result.json"))?;
    // Require the original log slices, even when empty: absent is not error-free.
    let lua = std::fs::read_to_string(directory.join("Lua.log"))?;
    let logs = BTreeMap::from([
        ("Lua.log".to_owned(), lua.clone()),
        (
            "Database.log".to_owned(),
            std::fs::read_to_string(directory.join("Database.log"))?,
        ),
        (
            "Modding.log".to_owned(),
            std::fs::read_to_string(directory.join("Modding.log"))?,
        ),
    ]);
    // Use the runner's actual pure judge, including its generated own-MOD
    // catalogue and multi-line source association, rather than a second regex.
    let checks = civ6_autotest_rs::judge::judge(&logs, "generic", None, !baseline);
    for check in checks {
        require(
            check.passed,
            format!("raw log check failed: {} ({})", check.name, check.detail),
        )?;
    }
    let run_id = directory.file_name().and_then(|n| n.to_str());
    verify_impl(&steps, &result, &lua, run_id, baseline)
}

/// Verify an untouched transcript and Lua text, for recorded/synthetic inputs.
/// This does not inspect Database/Modding logs or bind a live run namespace.
/// Live artifact acceptance must use `verify_directory` (as the CLI does).
pub fn verify(steps_text: &str, result_text: &str, lua: &str) -> Result<Report> {
    verify_impl(steps_text, result_text, lua, None, false)
}

/// Reproduce only ship::live's deterministic preparation. Never copy the
/// executed code, player configuration, timeout or capture keys into a contract.
pub(crate) fn prepared_contract(
    source: &str,
    steps_text: &str,
    count: u64,
    run_id: Option<&str>,
) -> Result<(Value, bool, bool)> {
    let mut expected: Value = serde_json::from_str(source)?;
    let steps = expected["steps"]
        .as_array_mut()
        .ok_or("missing original steps")?;
    let restart = count == steps.len() as u64 + 1;
    require(
        count == steps.len() as u64 || (restart && run_id.is_some()),
        "incomplete run",
    )?;
    let mut namespaced = false;
    if let Some(run_id) = run_id {
        // Choose between the untouched contract and exactly one allowed naming
        // transform; every later START still has to match the entire contract.
        if let Some((index, first_save)) = steps
            .iter()
            .enumerate()
            .find(|(_, s)| s["action"] == "save" && s["exact_name"] != true)
        {
            let action = format!("{}/{} save", index + 1 + usize::from(restart), count);
            for line in steps_text.lines() {
                let row: Value = serde_json::from_str(line)?;
                if row["action"] == action && row["status"] == "START" {
                    let executed: Value = serde_json::from_str(
                        row["detail"].as_str().ok_or("save START has no detail")?,
                    )?;
                    namespaced = executed["name"] != first_save["name"];
                    break;
                }
            }
        }
        if namespaced || restart {
            require(
                !run_id.is_empty()
                    && run_id.len() <= 240
                    && run_id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_')),
                "invalid artifact run ID",
            )?;
        }
        if namespaced {
            for step in steps.iter_mut() {
                if matches!(step["action"].as_str(), Some("save" | "load"))
                    && step["exact_name"] != true
                {
                    let name = step["name"].as_str().ok_or("save/load name missing")?;
                    step["name"] = Value::String(format!("{name}-{run_id}"));
                }
            }
        }
    }
    if restart {
        steps.insert(
            0,
            serde_json::json!({"action":"restart_game","timeout":420}),
        );
    }
    Ok((expected, restart, namespaced))
}

fn verify_impl(
    steps_text: &str,
    result_text: &str,
    lua: &str,
    run_id: Option<&str>,
    baseline: bool,
) -> Result<Report> {
    let source_text = if baseline { BASELINE } else { ORIGINAL };
    let source: Value = serde_json::from_str(source_text)?;
    let original_count = if baseline { 5 } else { 113 };
    require(
        source["steps"]
            .as_array()
            .is_some_and(|s| s.len() == original_count),
        "original scenario contract changed",
    )?;
    if baseline {
        // The comparison must use the full run's actual map, seed and player
        // order. Only the Persona MOD moves from enabled to disabled.
        let full: Value = serde_json::from_str(ORIGINAL)?;
        require(
            source["steps"][1] == full["steps"][1],
            "baseline configuration differs from full run",
        )?;
        let sets = |value: &Value| -> Result<BTreeSet<String>> {
            value
                .as_array()
                .ok_or("MOD IDs absent")?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "MOD ID is not a string".into())
                })
                .collect()
        };
        let base_enabled = sets(&source["steps"][0]["ids"])?;
        let full_enabled = sets(&full["steps"][0]["ids"])?;
        let base_disabled = sets(&source["steps"][0]["disable_ids"])?;
        let full_disabled = sets(&full["steps"][0]["disable_ids"])?;
        let persona = BTreeSet::from(["91cf7b65-54bb-4f69-a452-19a65527cde2".to_owned()]);
        require(
            base_enabled.is_disjoint(&base_disabled)
                && full_enabled.is_disjoint(&full_disabled)
                && full_enabled
                    .difference(&base_enabled)
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    == persona
                && base_enabled.difference(&full_enabled).next().is_none()
                && base_disabled
                    .difference(&full_disabled)
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    == persona
                && full_disabled.difference(&base_disabled).next().is_none(),
            "baseline MOD isolation differs from full run",
        )?;
    }
    let result: Value = serde_json::from_str(result_text)?;
    let count = result["completed_steps"]
        .as_u64()
        .ok_or("missing completed steps")?;
    let (expected, preparation_restart, namespaced) =
        prepared_contract(source_text, steps_text, count, run_id)?;
    let steps = expected["steps"]
        .as_array()
        .ok_or("missing original steps")?;
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
        result["completed_steps"].as_u64() == Some(steps.len() as u64),
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
        names
            == CHECKS[..if baseline { 5 } else { 6 }]
                .iter()
                .copied()
                .collect(),
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
                    && completed == steps.len()
                    && status == "OK"
                    && detail
                        == if baseline {
                            "5/5 checks passed"
                        } else {
                            "6/6 checks passed"
                        },
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
            action == format!("{}/{} {step_action}", completed + 1, steps.len()),
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
                    if !baseline && completed == 38 + usize::from(preparation_restart) {
                        require(
                            value == "native AI appointment gate verified after first turn",
                            "early AI result differs",
                        )?;
                    }
                    if !baseline && completed == 112 + usize::from(preparation_restart) {
                        require(
                            value == "no unjoined AI governors or persona Great People",
                            "final AI result differs",
                        )?;
                    }
                    if baseline {
                        require(
                            value == "baseline has 22 factions and idle game core",
                            "baseline readiness result differs",
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
        judged && !pending && completed == steps.len(),
        "run never completed all steps and judge",
    )?;
    require(
        captures.len() == if baseline { 0 } else { 6 }
            && restored.len() == if baseline { 0 } else { 6 },
        "save/load snapshot count differs",
    )?;
    Ok(Report {
        scenario_completed: !baseline,
        baseline_ready: baseline,
        completed_steps: completed - usize::from(preparation_restart),
        recorded_steps: completed,
        preparation_restart,
        save_namespace: if namespaced {
            run_id.map(str::to_owned)
        } else {
            None
        },
        matched_save_load_snapshots: restored.len(),
        early_ai_check: !baseline,
        final_ai_check: !baseline,
        official_lua_errors: 0,
        scenario_sha256: sha(source_text.as_bytes()),
        steps_sha256: sha(steps_text.as_bytes()),
        result_sha256: sha(result_text.as_bytes()),
        product_acceptance: false,
        scope: if baseline {
            "baseline startup only; no persona, AI, turns, payment or save acceptance"
        } else {
            "original113 recorded scenario only; product runtime, AI payment, MP and neutralization require separate evidence"
        },
    })
}
