//! Bounded startup diagnostics and conservative display recovery policy.
//! A log suffix is never used to decide whether startup has timed out.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub const RECOVERY_MARKER: &str = "STARTUP_RECOVERY_ATTEMPTED";
pub const CHAIN_PID_ENV: &str = "CIV6_LIVE_CHAIN_PID";
pub const LOCK_PATH_ENV: &str = "CIV6_LIVE_LOCK_PATH";
pub const LOG_LIMIT: u64 = 4 * 1024 * 1024;
pub const STARTUP_LOGS: &[&str] = &[
    "Startup.log",
    "Renderer.log",
    "FiraxisLive.log",
    "Engine.log",
    "Lua.log",
    "Database.log",
    "Modding.log",
    "EOS.log",
    "UserInterface.log",
];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameProcess {
    pub pid: u32,
    /// The creation time returned by the lifecycle backend, not a later PID lookup.
    pub started: f64,
}
impl From<(u32, f64)> for GameProcess {
    fn from((pid, started): (u32, f64)) -> Self {
        Self { pid, started }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuClient {
    pub pid: u32,
    pub kind: String,
    pub allocated_mib: Option<u64>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuSnapshot {
    pub status: String,
    pub elapsed_ms: u64,
    /// Must cover ALL adapters and WSL allocations, since the shortcut is global.
    pub all_clients_known: bool,
    pub clients: Vec<GpuClient>,
    pub allocated_mib: Option<u64>,
    pub utilization_percent: Option<u32>,
    pub detail: String,
}

pub fn reset_refusal(gpu: &GpuSnapshot, game: GameProcess) -> Option<&'static str> {
    if gpu.status != "ok" {
        return Some("GPU probe failed or timed out");
    }
    if gpu.clients.iter().any(|c| c.pid != game.pid) {
        return Some("foreign GPU client or compute allocation present");
    }
    if gpu
        .clients
        .iter()
        .any(|c| c.kind != "G" || c.allocated_mib.is_none())
    {
        return Some("compute work or unknown GPU allocation present");
    }
    if !gpu.all_clients_known {
        return Some("all-adapter/WSL GPU client coverage is unknown");
    }
    if gpu.utilization_percent != Some(0) {
        return Some("GPU work active or utilization unknown");
    }
    let Some(allocated) = gpu.allocated_mib else {
        return Some("total GPU allocation unknown");
    };
    let owned = gpu
        .clients
        .iter()
        .try_fold(0u64, |n, c| n.checked_add(c.allocated_mib?));
    if owned != Some(allocated) {
        return Some("unattributed GPU allocation present");
    }
    None
}

pub fn parse_nvidia_xml(xml: &str) -> Result<GpuSnapshot> {
    let doc = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 65536,
            ..Default::default()
        },
    )
    .map_err(|_| Error::input("invalid GPU probe XML"))?;
    let root = doc.root_element();
    let devices: Vec<_> = root.children().filter(|n| n.has_tag_name("gpu")).collect();
    if !root.has_tag_name("nvidia_smi_log") || devices.is_empty() {
        return Err(Error::input("GPU probe contains no devices"));
    }
    let number =
        |text: Option<&str>| -> Option<u64> { text?.split_whitespace().next()?.parse().ok() };
    let mut snapshot = GpuSnapshot {
        status: "ok".into(),
        allocated_mib: Some(0),
        utilization_percent: Some(0),
        detail: "NVIDIA telemetry cannot prove all-adapter/WSL client coverage on Windows".into(),
        ..GpuSnapshot::default()
    };
    for device in devices {
        let memory = device
            .children()
            .find(|n| n.has_tag_name("fb_memory_usage"))
            .and_then(|n| n.children().find(|n| n.has_tag_name("used")))
            .and_then(|n| n.text());
        snapshot.allocated_mib = snapshot
            .allocated_mib
            .zip(number(memory))
            .and_then(|(a, b)| a.checked_add(b));
        let util = device
            .children()
            .find(|n| n.has_tag_name("utilization"))
            .and_then(|n| n.children().find(|n| n.has_tag_name("gpu_util")))
            .and_then(|n| n.text());
        snapshot.utilization_percent = snapshot
            .utilization_percent
            .zip(number(util))
            .and_then(|(a, b)| u32::try_from(b).ok().map(|b| a.max(b)));
        let Some(processes) = device.children().find(|n| n.has_tag_name("processes")) else {
            snapshot.status = "unknown".into();
            continue;
        };
        for process in processes
            .children()
            .filter(|n| n.has_tag_name("process_info"))
        {
            let field = |name| {
                process
                    .children()
                    .find(|n| n.has_tag_name(name))
                    .and_then(|n| n.text())
            };
            let pid = number(field("pid"))
                .and_then(|pid| u32::try_from(pid).ok())
                .filter(|pid| *pid != 0)
                .ok_or_else(|| Error::input("GPU client PID unavailable"))?;
            snapshot.clients.push(GpuClient {
                pid,
                kind: field("type").unwrap_or("unknown").into(),
                allocated_mib: number(field("used_memory")),
            });
        }
    }
    Ok(snapshot)
}

/// Read only the selected game's startup logs, retaining the tail of large files.
/// This runs before any shutdown and before another launch can overwrite the logs.
pub fn snapshot_logs(logs: &Path, output: &Path, game: GameProcess) -> Value {
    let mut rows = Vec::new();
    for name in STARTUP_LOGS {
        let result = (|| -> Result<Value> {
            let mut file = fs::File::open(logs.join(name))?;
            let metadata = file.metadata()?;
            let modified = metadata
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| Error::input(e.to_string()))?
                .as_secs_f64();
            let offset = metadata.len().saturating_sub(LOG_LIMIT);
            file.seek(SeekFrom::Start(offset))?;
            let mut bytes = Vec::new();
            file.take(LOG_LIMIT).read_to_end(&mut bytes)?;
            fs::write(output.join(name), &bytes)?;
            Ok(json!({"name":name,"status":"saved","bytes":bytes.len(),
                "tail_offset":offset,"modified":modified,"current_process":modified >= game.started}))
        })();
        rows.push(match result {
            Ok(row) => row,
            Err(e) => json!({"name":name,"status":"unavailable","reason":e.to_string()}),
        });
    }
    json!(rows)
}

pub fn tuner_enabled(options: &str) -> bool {
    let values: Vec<_> = options
        .lines()
        .filter_map(|line| {
            let line = line.trim().trim_start_matches('\u{feff}');
            let (key, value) = line.split_once(char::is_whitespace)?;
            (key == "EnableTuner").then_some(value.trim())
        })
        .collect();
    values == ["1"]
}

/// Only the PID and the two ownership flags are inspected; lock contents are not logged.
pub fn verify_lock_record(record: &Value, chain_pid: u32) -> Result<()> {
    if chain_pid == 0
        || record["live_chain"] != true
        || record["chain_pid"].as_u64() != Some(u64::from(chain_pid))
        || record["pid"].as_u64() != Some(u64::from(chain_pid))
    {
        return Err(Error::input(
            "shared Civ6 lock is not owned by the invoking live chain",
        ));
    }
    Ok(())
}

#[cfg(windows)]
#[path = "startup_windows.rs"]
mod platform;
#[cfg(windows)]
pub use platform::{
    collect_diagnostics, diagnostic_worker, loading_evidence, pin_window_process, reset_display,
    PinnedGame, RecoveryLease,
};
#[cfg(not(windows))]
pub fn pin_window_process(_: isize, _: f64) -> Result<PinnedGame> {
    Err(Error::input("startup window identity requires Windows"))
}
#[cfg(not(windows))]
pub struct PinnedGame {
    pub game: GameProcess,
}
#[cfg(not(windows))]
impl PinnedGame {
    pub fn check_remaining(&self) -> Result<()> {
        Err(Error::input("process pin requires Windows"))
    }
}

#[cfg(not(windows))]
pub struct RecoveryLease;
#[cfg(not(windows))]
impl RecoveryLease {
    pub fn acquire() -> Result<Self> {
        Err(Error::input("startup recovery lock requires Windows"))
    }
    pub fn check(&mut self) -> Result<()> {
        Err(Error::input("startup recovery lock requires Windows"))
    }
}
#[cfg(not(windows))]
pub fn loading_evidence(_: GameProcess, _: &Path, _: u16) -> Result<Value> {
    Err(Error::input("startup loading evidence requires Windows"))
}
#[cfg(not(windows))]
pub fn collect_diagnostics(_: GameProcess, _: &Path) -> Result<Value> {
    Ok(json!({"status":"unsupported","reason":"native startup diagnostics require Windows"}))
}
#[cfg(not(windows))]
pub fn reset_display(_: &mut RecoveryLease, _: &PinnedGame) -> Result<Value> {
    Ok(json!({"status":"skipped","reason":"display recovery requires Windows"}))
}
#[cfg(not(windows))]
pub fn diagnostic_worker(_: &[String]) -> Result<()> {
    Err(Error::input("native diagnostic worker requires Windows"))
}
