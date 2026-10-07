//! Observe the immutable T62 save before any new exchange or mission.
#[path = "../src/spy_roster_read.rs"]
pub(crate) mod observer;
use serde_json::{json, Value};

pub(crate) fn scenario() -> Value {
    json!({"name":"Read independent UI and Core Unit rows from the preserved T62 failure save",
    "judge_profile":"generic","strict_lua_errors":true,"steps":[
      {"action":"load","name":"JAC-Enemy-Neutralization-ad65ffec7ebf491c8c2057b227df2304-0000000000000010","exact_name":true,"timeout":300},
      {"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"InGame","code":"return Players[0] and Players[0]:IsTurnActive() and ExposedMembers.JAC_PersonaBoundary~=nil"}},
      {"action":"ui_lua","state_name":"InGame","code":observer::ui_read()},
      {"action":"ui_lua","state_name":"GameCore_Tuner","code":observer::core_read()},
      {"action":"exit_to_menu"}
    ]})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let output = args.next().ok_or("output required")?;
    if output == "--inspect" {
        let input = args.next().ok_or("steps.jsonl required")?;
        let output = args.next().ok_or("report output required")?;
        let (ui, core) = observations(&std::fs::read_to_string(input)?)?;
        let report = observer::inspect_pair(&ui, &core)?;
        std::fs::write(output, serde_json::to_vec_pretty(&report)?)?;
        return Ok(());
    }
    std::fs::write(output, serde_json::to_vec_pretty(&scenario())?)?;
    Ok(())
}

/// Accept only returns paired with START records containing the exact scripts.
pub(crate) fn observations(transcript: &str) -> Result<(Value, Value), String> {
    let scene = scenario();
    let expected = scene["steps"].as_array().unwrap();
    let mut completed = 0;
    let mut pending: Option<String> = None;
    let mut startup_pending: Option<String> = None;
    let mut startup_seen = false;
    let mut startup_skip_seen = false;
    let mut header_seen = false;
    let mut judge_seen = false;
    let mut outputs = std::collections::BTreeMap::new();
    for line in transcript.lines() {
        if judge_seen {
            return Err("record after final judge".into());
        }
        let row: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let action = row["action"].as_str().ok_or("missing action")?;
        if !header_seen {
            if action == "start" && row["status"] == "OK" && row["detail"] == scene["name"] {
                header_seen = true;
                continue;
            }
            return Err("scenario header must precede startup and pinned load".into());
        }
        if action == "launch" || action == "auto_restart" {
            if completed != 0 || pending.is_some() {
                return Err("startup after pinned observation began".into());
            }
            let detail = row["detail"].as_str().ok_or("missing startup detail")?;
            match row["status"].as_str() {
                Some("START") => {
                    if startup_seen
                        || startup_pending.is_some()
                        || (action == "launch" && detail != "Civ VI is not running")
                        || (action == "auto_restart"
                            && (startup_skip_seen
                                || (detail != "Civ6 is not running"
                                    && !detail.starts_with("mod file newer than Civ6 process: "))))
                    {
                        return Err("duplicate or unknown startup".into());
                    }
                    startup_pending = Some(action.to_string());
                }
                Some("OK")
                    if startup_pending.as_deref() == Some(action)
                        && detail.starts_with("Civ VI relaunched via Steam") =>
                {
                    startup_pending = None;
                    startup_seen = true;
                }
                Some("SKIP")
                    if action == "auto_restart"
                        && !startup_seen
                        && !startup_skip_seen
                        && startup_pending.is_none()
                        && detail == "Civ VI process is newer than all scenario mod files" =>
                {
                    startup_skip_seen = true;
                }
                _ => return Err("failed or unpaired startup".into()),
            }
            continue;
        }
        if row["status"] == "START" {
            let detail: Value =
                serde_json::from_str(row["detail"].as_str().ok_or("missing START detail")?)
                    .map_err(|e| e.to_string())?;
            if completed == 0 && detail["action"] == "restart_game" {
                if pending.is_some() || startup_seen || startup_pending.is_some() {
                    return Err("duplicate explicit restart".into());
                }
                startup_pending = Some(action.to_string());
                continue;
            }
            if startup_pending.is_some()
                || pending.is_some()
                || expected.get(completed) != Some(&detail)
            {
                return Err("unexpected step or changed pinned load/read sequence".into());
            }
            pending = Some(action.to_string());
        } else if row["status"] == "OK" {
            if startup_pending.as_deref() == Some(action) {
                if !row["detail"]
                    .as_str()
                    .is_some_and(|d| d.starts_with("Civ VI relaunched via Steam"))
                {
                    return Err("explicit restart did not complete".into());
                }
                startup_pending = None;
                startup_seen = true;
                continue;
            }
            if pending.as_deref() != Some(action) {
                if action == "start"
                    && completed == 0
                    && !header_seen
                    && row["detail"] == scene["name"]
                {
                    header_seen = true;
                    continue;
                }
                if action == "judge" && completed == expected.len() && !judge_seen {
                    let detail = row["detail"].as_str().ok_or("missing judge result")?;
                    let counts = detail
                        .strip_suffix(" checks passed")
                        .and_then(|s| s.split_once('/'))
                        .ok_or("unknown judge result")?;
                    let passed: usize = counts.0.parse().map_err(|_| "invalid judge count")?;
                    let total: usize = counts.1.parse().map_err(|_| "invalid judge total")?;
                    if total == 0 || passed != total {
                        return Err("incomplete judge result".into());
                    }
                    judge_seen = true;
                    continue;
                }
                return Err("unpaired or duplicate observation result".into());
            }
            let step = &expected[completed];
            if step["action"] == "load" {
                let detail = row["detail"].as_str().ok_or("missing load result")?;
                if detail.strip_prefix("loaded ") != step["name"].as_str() {
                    return Err("pinned physical save did not load".into());
                }
            } else if step["action"] == "wait_for" {
                let raw = row["detail"]
                    .as_str()
                    .ok_or("missing wait result")?
                    .strip_prefix("condition met: ")
                    .ok_or("wait did not complete")?;
                let actual: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
                if actual != step["condition"] {
                    return Err("different readiness condition".into());
                }
            } else if step["action"] == "ui_lua" {
                let context = step["state_name"].as_str().unwrap();
                let prefix = format!("ran Lua in {context}; return=");
                let raw = row["detail"]
                    .as_str()
                    .ok_or("missing return")?
                    .strip_prefix(&prefix)
                    .ok_or("return context mismatch")?;
                let read: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
                if read["context"] != context || outputs.insert(context.to_string(), read).is_some()
                {
                    return Err("duplicate or mismatched observation return".into());
                }
            }
            completed += 1;
            pending = None;
        } else if row["status"] == "FAIL" {
            return Err("failed pinned observation run".into());
        } else {
            return Err("unknown observation status".into());
        }
    }
    if completed != expected.len()
        || pending.is_some()
        || startup_pending.is_some()
        || !header_seen
        || !judge_seen
    {
        return Err("pinned load/wait/UI/Core/exit run incomplete".into());
    }
    Ok((
        outputs
            .remove("InGame")
            .ok_or("UI raw observation missing")?,
        outputs
            .remove("GameCore_Tuner")
            .ok_or("Core raw observation missing")?,
    ))
}
