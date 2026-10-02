//! Independent Windows scheduler evidence, including CUDA work reported as 3D.
//! Resident allocations are allowed only under this complete, fresh idle proof.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const IDLE_SOURCE: &str = "windows_pdh_all_engines";
pub const IDLE_LIMIT_PERCENT: f64 = 1.0;
pub const SHORTCUT_KEYS: [u8; 4] = [0x5b, 0x11, 0x10, 0x42];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterCoverage {
    pub id: String,
    pub nodes: u32,
    pub wddm_version: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EngineReading {
    pub pid: u32,
    pub adapter: String,
    pub physical: u32,
    pub engine: u32,
    pub kind: String,
    pub percent: f64,
    pub valid: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IdleSample {
    pub interval_ms: u64,
    pub engines: Vec<EngineReading>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IdleProof {
    pub source: String,
    pub status: String,
    pub adapters: Vec<AdapterCoverage>,
    /// Identified by the kernel's SoftwareDevice flag, never inferred from zero usage.
    #[serde(default)]
    pub software_adapters: Vec<String>,
    pub adapters_stable: bool,
    pub samples: Vec<IdleSample>,
    pub finished_unix: f64,
    pub detail: String,
}

pub fn idle_refusal(proof: &IdleProof) -> Option<&'static str> {
    if proof.source != IDLE_SOURCE || proof.status != "ok" {
        return Some("native PDH GPU engine proof failed or is unknown");
    }
    if !proof.adapters_stable || proof.adapters.is_empty() {
        return Some("GPU adapter coverage changed or is unknown");
    }
    let expected: BTreeMap<_, _> = proof.adapters.iter().map(|a| (&a.id, a.nodes)).collect();
    let software: BTreeSet<_> = proof.software_adapters.iter().collect();
    if software.len() != proof.software_adapters.len()
        || proof
            .software_adapters
            .iter()
            .any(|id| id.is_empty() || expected.contains_key(id))
    {
        return Some("software GPU identity overlaps or is unknown");
    }
    if expected.len() != proof.adapters.len()
        || proof
            .adapters
            .iter()
            .any(|a| a.nodes == 0 || a.nodes > 128 || a.wddm_version < 2000)
    {
        return Some("GPU adapter engine coverage is unknown or unsupported");
    }
    if proof.samples.len() != 3 {
        return Some("three complete one-second PDH samples are required");
    }
    for sample in &proof.samples {
        if !(1000..=2000).contains(&sample.interval_ms) || sample.engines.is_empty() {
            return Some("PDH sampling interval or client coverage is unknown");
        }
        let mut seen = BTreeSet::new();
        let mut totals = BTreeMap::new();
        for engine in &sample.engines {
            if !engine.valid
                || engine.pid == 0
                || engine.kind.is_empty()
                || !engine.percent.is_finite()
                || engine.percent < 0.0
            {
                return Some("invalid or unknown PDH GPU client reading");
            }
            if engine.physical != 0
                || (!software.contains(&engine.adapter)
                    && !expected
                        .get(&engine.adapter)
                        .is_some_and(|nodes| engine.engine < *nodes))
            {
                return Some("unrecognized GPU adapter or engine");
            }
            if engine.percent >= IDLE_LIMIT_PERCENT {
                return Some("GPU engine work is active (including 3D/CUDA/WSL)");
            }
            let key = (&engine.adapter, engine.engine);
            seen.insert(key);
            *totals.entry(key).or_insert(0.0) += engine.percent;
        }
        if totals
            .values()
            .any(|percent| *percent >= IDLE_LIMIT_PERCENT)
        {
            return Some("aggregate GPU engine work is active");
        }
        if proof
            .adapters
            .iter()
            .any(|adapter| (0..adapter.nodes).any(|node| !seen.contains(&(&adapter.id, node))))
        {
            return Some("PDH does not cover every kernel-reported GPU engine");
        }
    }
    None
}

/// Parse the complete counter instance, never assuming compute is named Compute.
pub fn engine_reading(name: &str, percent: f64, valid: bool) -> Result<EngineReading> {
    let parts: Vec<_> = name.splitn(11, '_').collect();
    let error = || Error::input("unknown PDH GPU engine instance format");
    if parts.len() != 11
        || parts[0] != "pid"
        || parts[2] != "luid"
        || parts[5] != "phys"
        || parts[7] != "eng"
        || parts[9] != "engtype"
    {
        return Err(error());
    }
    let high = u32::from_str_radix(parts[3].strip_prefix("0x").ok_or_else(error)?, 16)
        .map_err(|_| error())?;
    let low = u32::from_str_radix(parts[4].strip_prefix("0x").ok_or_else(error)?, 16)
        .map_err(|_| error())?;
    Ok(EngineReading {
        pid: parts[1].parse().map_err(|_| error())?,
        adapter: format!("{high:08x}:{low:08x}"),
        physical: parts[6].parse().map_err(|_| error())?,
        engine: parts[8].parse().map_err(|_| error())?,
        kind: parts[10].into(),
        percent,
        valid,
    })
}

pub trait DisplayInput {
    fn check_lock(&mut self) -> Result<()>;
    fn check_cancel(&mut self) -> Result<()> {
        crate::time::check_interrupt()
    }
    fn unix_now(&mut self) -> f64;
    fn key_is_down(&mut self, key: u8) -> bool;
    fn key_event(&mut self, key: u8, up: bool);
    fn sleep_100ms(&mut self);
}
pub fn send_recovery_shortcut(input: &mut impl DisplayInput, proof: &IdleProof) -> Result<Value> {
    input.check_cancel()?;
    input.check_lock()?;
    if let Some(reason) = idle_refusal(proof) {
        return Ok(json!({"status":"skipped","reason":reason,"pdh":proof}));
    }
    let age = input.unix_now() - proof.finished_unix;
    if !age.is_finite() || !(0.0..=0.5).contains(&age) {
        return Ok(json!({"status":"skipped","reason":"PDH idle proof is stale","pdh":proof}));
    }
    input.check_cancel()?;
    input.check_lock()?; // Immediately before input, with the read handle still pinned.
    for key in SHORTCUT_KEYS.into_iter().chain([0x5c, 0x12]) {
        if input.key_is_down(key) {
            return Ok(
                json!({"status":"skipped","reason":"modifier or shortcut key already pressed","pdh":proof}),
            );
        }
    }
    input.check_cancel()?;
    let age = input.unix_now() - proof.finished_unix;
    if !age.is_finite() || !(0.0..=0.5).contains(&age) {
        return Ok(
            json!({"status":"skipped","reason":"PDH idle proof became stale before input","pdh":proof}),
        );
    }
    // EXACT earlier keybd_event order, delay and reverse release.
    for key in SHORTCUT_KEYS {
        input.key_event(key, false);
    }
    input.sleep_100ms();
    for key in SHORTCUT_KEYS.into_iter().rev() {
        input.key_event(key, true);
    }
    let interrupted_after_input = input.check_cancel().is_err();
    Ok(
        json!({"status":"sent","method":"Win+Ctrl+Shift+B","pdh":proof,"interrupted_after_input":interrupted_after_input}),
    )
}
