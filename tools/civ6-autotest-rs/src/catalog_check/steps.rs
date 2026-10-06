//! Freeze generated action/code associations and consume only successful reads.
use super::{contracts::Contract, digest, json, require, SourceContractVersion};
use crate::{persona_payment::ProcessIdentity, Error, Result};
use civ6_native_bridge_rs::paid_catalog::COUNT;
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Clone, Debug, Serialize)]
pub struct CatalogEntry {
    pub source: u32,
    pub owner: u32,
    pub leader: String,
    pub civilization: String,
    pub index: u32,
    pub hash: i64,
    pub cost: u32,
    pub capital: u32,
}
#[derive(Clone, Debug, Serialize)]
pub struct Matrix {
    pub measured: bool,
    pub marker: &'static str,
    pub positive: usize,
    pub negative: usize,
    pub raw: Vec<String>,
    pub step: usize,
}
#[derive(Debug)]
pub struct Evidence {
    pub(super) process: ProcessIdentity,
    pub(super) artifact: String,
    pub(super) artifact_path: Option<PathBuf>,
    pub(super) catalog: Vec<CatalogEntry>,
    pub(super) ui: Matrix,
    pub(super) gameplay: Matrix,
    pub(super) steps_sha256: String,
    pub(super) scenario_contract_sha256: String,
    pub(super) source_contract_version: SourceContractVersion,
    pub(super) scenario_complete: bool,
    pub(super) gameplay_closed484: bool,
    pub(super) failure: Option<String>,
}
fn decimal<T: std::str::FromStr>(text: &str) -> Result<T> {
    require(
        !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()),
        "invalid receipt decimal",
    )?;
    text.parse()
        .map_err(|_| Error::input("receipt decimal overflow"))
}
fn catalog(value: &str, contract: &Value) -> Result<Vec<CatalogEntry>> {
    let rest = value
        .strip_prefix("CATALOG880 STABLE turn=")
        .ok_or_else(|| Error::input("stable receipt missing"))?;
    let (turn, rest) = rest
        .split_once(" catalog=")
        .ok_or_else(|| Error::input("stable catalog missing"))?;
    let _: u32 = decimal(turn)?;
    let (records, titles) = rest
        .split_once(" titles=")
        .ok_or_else(|| Error::input("stable titles missing"))?;
    let players = contract["steps"][2]["players"].as_array().unwrap();
    let mut entries = Vec::new();
    let mut indices = BTreeSet::new();
    let mut hashes = BTreeSet::new();
    let mut costs = BTreeSet::new();
    let records: Vec<_> = records.split(';').collect();
    let balances: Vec<_> = titles.split(';').collect();
    require(
        records.len() == COUNT && balances.len() == COUNT,
        "stable catalog must have exactly 22 identities and balances",
    )?;
    for (source, (record, balance)) in records.iter().zip(balances).enumerate() {
        let parts: Vec<_> = record.split(':').collect();
        let balance: Vec<_> = balance.split(':').collect();
        require(
            parts.len() == 7 && balance.len() == 3,
            "malformed stable identity/balance",
        )?;
        let index: u32 = decimal(parts[3])?;
        let hash: i64 = parts[4]
            .parse()
            .map_err(|_| Error::input("invalid runtime governor hash"))?;
        let cost: u32 = decimal(parts[5])?;
        let capital: u32 = decimal(parts[6])?;
        let _: u32 = decimal(balance[1])?;
        require(
            decimal::<usize>(parts[0])? == source
                && decimal::<usize>(balance[0])? == source
                && Value::from(parts[1]) == players[source]["leader"]
                && Value::from(parts[2]) == players[source]["civ"]
                && index < 256
                && indices.insert(index)
                && hashes.insert(hash)
                && (1..=3).contains(&cost)
                && decimal::<u32>(balance[2])? == 0,
            "stable source/catalog/unique runtime index/hash/titles differ",
        )?;
        costs.insert(cost);
        entries.push(CatalogEntry {
            source: source as u32,
            owner: ((source + 1) % COUNT) as u32,
            leader: parts[1].into(),
            civilization: parts[2].into(),
            index,
            hash,
            cost,
            capital,
        });
    }
    require(
        costs == BTreeSet::from([1, 2, 3]),
        "runtime catalog lacks the three costs",
    )?;
    for e in &entries {
        let fields: Vec<_> = titles
            .split(';')
            .nth(e.owner as usize)
            .unwrap()
            .split(':')
            .collect();
        require(
            decimal::<u32>(fields[1])? >= e.cost,
            "recipient lacks official titles",
        )?;
    }
    Ok(entries)
}

fn identity(parts: &[&str], e: &CatalogEntry) -> Result<()> {
    require(
        parts.len() >= 5
            && decimal::<u32>(parts[0])? == e.source
            && decimal::<u32>(parts[1])? == e.owner
            && decimal::<u32>(parts[2])? == e.index
            && parts[3] == e.leader
            && decimal::<u32>(parts[4])? == e.cost,
        "receipt source/subject owner/runtime index/leader/cost mismatch",
    )
}
fn ledger(value: &str, entries: &[CatalogEntry], phase: &str) -> Result<()> {
    let records: Vec<_> = value.split(';').collect();
    require(
        records.len() == entries.len(),
        "ledger receipt count differs",
    )?;
    for (record, e) in records.iter().zip(entries) {
        let p: Vec<_> = record.split(':').collect();
        require(p.len() == 10, "incomplete ledger receipt")?;
        identity(&p, e)?;
        require(
            decimal::<u32>(p[8])? >= e.cost && decimal::<u32>(p[9])? == 0,
            "title balance changed",
        )?;
        match phase {
            "closed" => require(
                p[5..8] == ["nil", "nil", "nil"],
                "closed Gameplay ledger not empty",
            )?,
            "JOINED" => {
                require(
                    p[5] == "GREAT_PERSON" && p[7] == "nil",
                    "joined ledger differs",
                )?;
                let _: u32 = decimal(p[6])?;
            }
            "EVOLVING" => {
                require(
                    p[5] == "EVOLVING" && decimal::<u32>(p[7])? == e.cost - 1,
                    "evolution ledger differs",
                )?;
                let _: u32 = decimal(p[6])?;
            }
            _ => return Err(Error::input("unknown ledger phase")),
        }
    }
    Ok(())
}

fn raw_matrix(value: &str, entries: &[CatalogEntry], core: bool, step: usize) -> Result<Matrix> {
    let marker = if core {
        "GAMEPLAY_PROJECTION"
    } else {
        "UI_NATIVE_QUALIFICATION"
    };
    let raw = value
        .strip_prefix("positive=22 negative=462 ")
        .ok_or_else(|| Error::input("explicit final raw matrix totals missing"))?;
    let records: Vec<_> = raw.split(';').collect();
    require(
        records.len() == COUNT,
        "final raw matrix missing identities",
    )?;
    let mut bits = Vec::new();
    for (record, e) in records.iter().zip(entries) {
        let rest = record
            .strip_prefix(&format!("CATALOG880 {marker} "))
            .ok_or_else(|| Error::input("mixed UI/Gameplay receipt provenance"))?;
        let (id, bit) = rest
            .split_once(" positive=1 negative=21 raw=")
            .ok_or_else(|| Error::input("final per-person raw receipt missing"))?;
        let fields: Vec<_> = id.split(':').collect();
        require(fields.len() == 5, "invalid raw identity")?;
        identity(&fields, e)?;
        let expected: String = (0..COUNT)
            .map(|owner| if owner == e.owner as usize { '1' } else { '0' })
            .collect();
        require(bit == expected, "raw qualification bit matrix differs")?;
        bits.push(bit.to_owned());
    }
    Ok(Matrix {
        measured: true,
        marker,
        positive: 22,
        negative: 462,
        raw: bits,
        step,
    })
}

/// Every indexed START must contain the exact generated action/code; its OK
/// must immediately follow. Nothing in a START code string is a measurement.
pub fn evidence_from_steps(text: &str) -> Result<Evidence> {
    parse_steps(text, false)
}

pub(super) fn parse_steps(text: &str, allow_failed_capture: bool) -> Result<Evidence> {
    let legacy = Contract::load(SourceContractVersion::Legacy4cd)?;
    let adjusted = Contract::load(SourceContractVersion::NilAdjusted068)?;
    let mut candidates = vec![legacy, adjusted];
    let mut shipping_restart = None;
    let mut expected = candidates[0].data["steps"].as_array().unwrap().clone();
    let mut next = 0;
    let mut pending = false;
    let mut started = false;
    let mut judged = false;
    let mut auxiliary: Option<String> = None;
    let mut captures: BTreeMap<String, String> = BTreeMap::new();
    let mut entries: Option<Vec<CatalogEntry>> = None;
    let mut process = None;
    let mut artifact_path = None;
    let mut ui = None;
    let mut gameplay = None;
    let mut last_time = None;
    let mut failure = None;
    for line in text.lines() {
        require(!judged, "records follow final judge")?;
        require(failure.is_none(), "records follow failed scenario step")?;
        let row = json(line)?;
        super::shape(&row, &["at", "action", "status", "detail"])?;
        let at = chrono::DateTime::parse_from_rfc3339(row["at"].as_str().unwrap_or(""))
            .map_err(|_| Error::input("step timestamp invalid"))?;
        require(
            last_time.is_none_or(|last| at >= last),
            "step time goes backwards",
        )?;
        last_time = Some(at);
        let action = row["action"]
            .as_str()
            .ok_or_else(|| Error::input("step action missing"))?;
        let status = row["status"]
            .as_str()
            .ok_or_else(|| Error::input("step status missing"))?;
        let detail = row["detail"]
            .as_str()
            .ok_or_else(|| Error::input("step detail missing"))?;
        require(
            matches!(status, "START" | "OK")
                || (allow_failed_capture && status == "FAIL")
                || (action == "auto_restart" && status == "SKIP" && next == 0 && !pending),
            "failed or unknown step status",
        )?;
        if action == "start" {
            require(
                !started
                    && next == 0
                    && status == "OK"
                    && candidates
                        .iter()
                        .all(|c| Value::from(detail) == c.data["name"]),
                "wrong/duplicate scenario start",
            )?;
            started = true;
            continue;
        }
        require(started, "scenario start missing")?;
        if matches!(action, "auto_restart" | "launch") && next == 0 && !pending {
            match status {
                "SKIP" => {
                    require(
                        auxiliary.is_none()
                            && detail == "Civ VI process is newer than all scenario mod files",
                        "invalid startup skip",
                    )?;
                }
                "START" => {
                    require(auxiliary.is_none(), "duplicate startup log")?;
                    auxiliary = Some(action.into());
                }
                "OK" => {
                    require(
                        auxiliary.as_deref() == Some(action) || action == "auto_restart",
                        "startup result without start",
                    )?;
                    auxiliary = None;
                }
                "FAIL" => return Err(Error::input(format!("startup failed: {action}: {detail}"))),
                _ => return Err(Error::input("unknown startup status")),
            }
            continue;
        }
        require(auxiliary.is_none(), "unfinished startup")?;
        if action == "judge" {
            require(
                next == expected.len() && !pending && status == "OK",
                "incomplete scenario judge",
            )?;
            let totals = detail
                .strip_suffix(" checks passed")
                .and_then(|v| v.split_once('/'))
                .ok_or_else(|| Error::input("invalid judge receipt"))?;
            require(
                decimal::<usize>(totals.0)? > 0
                    && decimal::<usize>(totals.0)? == decimal::<usize>(totals.1)?,
                "judge not completely successful",
            )?;
            judged = true;
            continue;
        }
        // Shipping may prepend a bounded restart to the generated scenario.
        // Recognize it by action and the complete START object, not fixed step
        // offsets. The native window and all generated code stay unchanged.
        if next == 0 && !pending && status == "START" {
            let (_, operation) = action
                .split_once(' ')
                .ok_or_else(|| Error::input("invalid numbered action"))?;
            if operation == "restart_game" && expected[0]["action"] != "restart_game" {
                let prefix = json(detail)?;
                require(
                    prefix["action"] == "restart_game"
                        && prefix.as_object().is_some_and(|o| {
                            o.keys().all(|k| matches!(k.as_str(), "action" | "timeout"))
                        })
                        && prefix
                            .get("timeout")
                            .is_none_or(|v| v.as_u64().is_some_and(|n| n > 0 && n <= 600)),
                    "invalid shipping restart",
                )?;
                shipping_restart = Some(prefix.clone());
                expected.insert(0, prefix);
            }
        }
        if status == "START" && !pending {
            let submitted = json(detail)?;
            candidates.retain(|c| {
                let source_steps = c.data["steps"].as_array().unwrap();
                let candidate = if let Some(prefix) = &shipping_restart {
                    if next == 0 {
                        Some(prefix)
                    } else {
                        source_steps.get(next - 1)
                    }
                } else {
                    source_steps.get(next)
                };
                candidate == Some(&submitted)
            });
            require(
                !candidates.is_empty(),
                "unknown or mixed source contract version",
            )?;
            expected = candidates[0].data["steps"].as_array().unwrap().clone();
            if let Some(prefix) = &shipping_restart {
                expected.insert(0, prefix.clone());
            }
        }
        let step = expected
            .get(next)
            .ok_or_else(|| Error::input("extra step after expected scenario"))?;
        let expected_action = format!(
            "{}/{} {}",
            next + 1,
            expected.len(),
            step["action"].as_str().unwrap()
        );
        require(
            action == expected_action,
            "step order/count/action differs from source contract",
        )?;
        if status == "START" {
            require(
                !pending && json(detail)? == *step,
                "duplicate START or changed/truncated scenario source",
            )?;
            pending = true;
            continue;
        }
        require(pending, "successful receipt has no matching START")?;
        pending = false;
        if status == "FAIL" {
            require(
                allow_failed_capture && process.is_some(),
                "scenario failed before native capture",
            )?;
            failure = Some(format!("{action}: {detail}"));
            continue;
        }
        match step["action"].as_str().unwrap() {
            "start_native_production_admission" => {
                require(process.is_none(), "duplicate native observation")?;
                let raw=detail.strip_prefix("native production-admission original-return telemetry ready; artifact normal-catalog22.jsonl; process=")
                    .ok_or_else(|| Error::input("native start process/artifact receipt missing"))?;
                let id: ProcessIdentity = serde_json::from_value(json(raw)?)?;
                require(
                    id.pid > 0 && id.created_filetime > 0,
                    "invalid native process identity",
                )?;
                process = Some(id);
            }
            "stop_native_production_admission" => {
                require(
                    process.is_some() && artifact_path.is_none(),
                    "native stop not associated with start",
                )?;
                let path = detail
                    .strip_prefix("native entry restored; artifact ")
                    .ok_or_else(|| Error::input("native restoration path missing"))?;
                let normalized = path.replace('\\', "/");
                require(
                    normalized.rsplit('/').next() == Some("normal-catalog22.jsonl"),
                    "stop names another artifact",
                )?;
                artifact_path = Some(PathBuf::from(normalized));
            }
            "exit_to_menu" => require(
                artifact_path.is_some() && detail == "main menu reached",
                "exit before restored capture",
            )?,
            "ui_lua" => {
                let state = step["state_name"].as_str().unwrap();
                let value = detail
                    .strip_prefix(&format!("ran Lua in {state}; return="))
                    .ok_or_else(|| Error::input("Lua successful return missing or wrong state"))?
                    .trim_end_matches(['\r', '\n']);
                let key = step["capture_as"]
                    .as_str()
                    .or_else(|| step["equals_capture"].as_str());
                let frozen_steps = candidates[0].data["steps"].as_array().unwrap();
                let is_prepare = step == &frozen_steps[5];
                let is_closed_read = step == &frozen_steps[9];
                if step["capture_as"] == "CATALOG880_STABLE"
                    || step["equals_capture"] == "CATALOG880_STABLE"
                {
                    let read = catalog(value, &candidates[0].data)?;
                    if entries.is_none() {
                        entries = Some(read);
                    }
                } else if matches!(key, Some("CATALOG880_CLOSED" | "CATALOG880_FINAL")) {
                    let key = step["capture_as"]
                        .as_str()
                        .or_else(|| step["equals_capture"].as_str())
                        .unwrap_or("");
                    let phase = if key == "CATALOG880_CLOSED" {
                        "closed"
                    } else {
                        "EVOLVING"
                    };
                    ledger(
                        value
                            .strip_prefix("CATALOG880 MATRIX identities=22 ")
                            .ok_or_else(|| Error::input("matrix ledger receipt missing"))?,
                        entries
                            .as_ref()
                            .ok_or_else(|| Error::input("stable catalog absent"))?,
                        phase,
                    )?;
                } else if key.is_none() && !is_prepare && !is_closed_read {
                    let core = state == "GameCore_Tuner";
                    let matrix = raw_matrix(
                        value,
                        entries
                            .as_ref()
                            .ok_or_else(|| Error::input("stable catalog absent"))?,
                        core,
                        next + 1,
                    )?;
                    let slot = if core { &mut gameplay } else { &mut ui };
                    require(slot.is_none(), "duplicate raw final receipt")?;
                    *slot = Some(matrix);
                } else if let Some(key) = step["capture_as"]
                    .as_str()
                    .or_else(|| step["equals_capture"].as_str())
                {
                    let rest = key
                        .strip_prefix("CATALOG880_")
                        .ok_or_else(|| Error::input("unknown capture key"))?;
                    let (source, phase) = rest
                        .split_once('_')
                        .ok_or_else(|| Error::input("batch capture key malformed"))?;
                    let first: usize = decimal(source)?;
                    let phase = if phase == "EVOLVE" {
                        "EVOLVING"
                    } else {
                        "JOINED"
                    };
                    let raw = value
                        .strip_prefix(&format!("CATALOG880 {phase} "))
                        .ok_or_else(|| Error::input("batch successful receipt missing"))?;
                    let entries = entries
                        .as_ref()
                        .ok_or_else(|| Error::input("stable catalog absent"))?;
                    ledger(raw, &entries[first..(first + 4).min(COUNT)], phase)?;
                } else if is_prepare {
                    require(value=="CATALOG880 official civics only; UI automatic submission paused; no game turn requested", "prepare receipt differs")?;
                } else if is_closed_read {
                    // READ_CLOSED_ADMISSION asserts the original synchronized
                    // close/reset read. It is not the initial UI closed484 test.
                    require(value=="PAYMENT880 observed 484 existing closed Core qualifications after observer installation", "closed admission receipt differs")?;
                } else {
                    return Err(Error::input(
                        "unexpected or missing explicit raw matrix return",
                    ));
                }
                if let Some(key) = step["equals_capture"].as_str() {
                    require(
                        captures.get(key).is_some_and(|old| old == value),
                        "successful reads differ from their captured value",
                    )?;
                }
                if let Some(key) = step["capture_as"].as_str() {
                    require(
                        captures.insert(key.into(), value.into()).is_none(),
                        "duplicate successful capture key",
                    )?;
                }
            }
            _ => (),
        }
        next += 1;
    }
    require(
        (judged && next == expected.len() && !pending)
            || (allow_failed_capture && failure.is_some()),
        "complete generated run and final judge required",
    )?;
    let unmeasured = |marker| Matrix {
        measured: false,
        marker,
        positive: 0,
        negative: 0,
        raw: vec![],
        step: 0,
    };
    let complete = judged && failure.is_none();
    require(
        candidates.len() == 1,
        "run has not selected a unique source contract version",
    )?;
    let contract = &candidates[0];
    require(
        !complete || (ui.is_some() && gameplay.is_some() && artifact_path.is_some()),
        "completed scenario missing final raw matrices or stop path",
    )?;
    Ok(Evidence {
        process: process.ok_or_else(|| Error::input("native process missing"))?,
        artifact: "normal-catalog22.jsonl".into(),
        artifact_path,
        catalog: entries.ok_or_else(|| Error::input("stable runtime catalog missing"))?,
        ui: ui.unwrap_or_else(|| unmeasured("UI_NATIVE_QUALIFICATION")),
        gameplay: gameplay.unwrap_or_else(|| unmeasured("GAMEPLAY_PROJECTION")),
        steps_sha256: digest(text),
        scenario_contract_sha256: contract.sha256.into(),
        source_contract_version: contract.version,
        scenario_complete: complete,
        gameplay_closed484: captures.contains_key("CATALOG880_CLOSED"),
        failure,
    })
}
