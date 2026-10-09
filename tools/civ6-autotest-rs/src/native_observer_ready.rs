//! Finite handoff validation. No game mutations except the single fixed acknowledgement.
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

pub const ACK_LUA: &str = "local d=assert(ExposedMembers.JAC_LoyaltyNativeProbe);assert(d.HandlerRegistered and not d.MutationIssued and not d.Issued and not d.ObserverReady);assert(d.Owner==Game.GetLocalPlayer() and Players[d.Owner]:IsTurnActive() and Game.GetCurrentGameTurn()==d.Turn);d.ObserverReady=true;return 'native observer ready acknowledged'";
pub const CORE_SHA256: &str = "324C51E9EA3531758842E16C69E6CDDBEFBB226C5B675C3D6E60111646C2E98C";
pub const LOCK_PATH: &str = "D:/CC/Tools/civ6-live/civ6.lock";
const ROOT: &str = "D:/CC/Work/todo995/";
const MAX_RECORD: usize = 64 * 1024;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub run_tag: String,
    pub chain_pid: u32,
    pub chain_created: u64,
    pub lock_token: String,
    pub game_pid: u32,
    pub game_created: u64,
    pub observer_pid: u32,
    pub observer_created: u64,
    pub ready_path: String,
    pub core_sha256: String,
}

fn path_eq(a: &str, b: &str) -> bool {
    a.replace('\\', "/")
        .eq_ignore_ascii_case(&b.replace('\\', "/"))
}

pub fn validate_step(step: &Value) -> Result<()> {
    let path = step["handoff"]
        .as_str()
        .ok_or_else(|| Error::scenario("native observer needs a handoff path"))?;
    let normalized = path.replace('\\', "/");
    let tag = normalized
        .strip_prefix(ROOT)
        .and_then(|s| s.strip_suffix("-handoff.json"));
    if !tag.is_some_and(valid_tag)
        || step.get("timeout").is_some_and(|v| {
            !v.as_f64()
                .is_some_and(|n| n.is_finite() && n > 0.0 && n <= 240.0)
        })
        || step.as_object().is_none_or(|o| {
            o.keys()
                .any(|k| !matches!(k.as_str(), "action" | "handoff" | "timeout"))
        })
    {
        return Err(Error::scenario("native observer requires a unique handoff under D:/CC/Work/todo995 and timeout in (0,240]"));
    }
    Ok(())
}

fn valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 128
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_RECORD {
        return Err(Error::input("native observer record exceeds limit"));
    }
    // Do not include record contents (in particular the lease token) in errors.
    serde_json::from_slice(bytes).map_err(|_| Error::input("invalid native observer JSON record"))
}

pub fn parse_manifest(path: &Path, bytes: &[u8]) -> Result<Manifest> {
    let m: Manifest = json(bytes)?;
    if m.version != 1
        || !valid_tag(&m.run_tag)
        || !path_eq(
            &path.to_string_lossy(),
            &format!("{ROOT}{}-handoff.json", m.run_tag),
        )
        || !path_eq(
            &m.ready_path,
            &format!("{ROOT}{}-observer.jsonl.ready", m.run_tag),
        )
        || m.core_sha256 != CORE_SHA256
        || m.lock_token.trim().is_empty()
        || [m.chain_pid, m.game_pid, m.observer_pid].contains(&0)
        || m.chain_pid == m.game_pid
        || m.chain_pid == m.observer_pid
        || m.game_pid == m.observer_pid
        || [m.chain_created, m.game_created, m.observer_created].contains(&0)
    {
        return Err(Error::input("native observer manifest binding mismatch"));
    }
    Ok(m)
}

#[derive(Clone, Copy, Debug)]
pub struct Identity {
    pub pid: u32,
    pub created: u64,
    pub alive: bool,
}

/// The OS adapter supplies identity from each retained handle, never reopening a PID.
pub fn validate_evidence(
    m: &Manifest,
    lease: &[u8],
    ready: &[u8],
    identities: [Identity; 3],
    game_pid: u32,
) -> Result<()> {
    let lease: Value = json(lease)?;
    if lease["live_chain"] != true
        || lease["pid"].as_u64() != Some(u64::from(m.chain_pid))
        || lease["chain_pid"].as_u64() != Some(u64::from(m.chain_pid))
        || lease["pid_created_filetime"].as_u64() != Some(m.chain_created)
        || lease["lock_token"].as_str() != Some(m.lock_token.as_str())
        || game_pid != m.game_pid
    {
        return Err(Error::input(
            "native observer lease or game identity changed",
        ));
    }
    for (actual, (pid, created)) in identities.into_iter().zip([
        (m.chain_pid, m.chain_created),
        (m.game_pid, m.game_created),
        (m.observer_pid, m.observer_created),
    ]) {
        if !actual.alive || actual.pid != pid || actual.created != created {
            return Err(Error::input(
                "native observer process exited or identity changed",
            ));
        }
    }
    let ready: Value = json(ready)?;
    if ready["event"] != "ready"
        || ready["pid"].as_u64() != Some(u64::from(m.game_pid))
        || ready["created"].as_u64() != Some(m.game_created)
        || ready["module_file_sha256"].as_str() != Some(m.core_sha256.as_str())
        || ready["threads_installed"].as_u64().is_none_or(|n| n == 0)
        || !ready["ready_path"]
            .as_str()
            .is_some_and(|p| path_eq(p, &m.ready_path))
    {
        return Err(Error::input("native observer ready evidence mismatch"));
    }
    Ok(())
}

pub trait Session: Send {
    fn check(&mut self, manifest: &Manifest, game_pid: u32) -> Result<()>;
}

#[cfg(windows)]
pub fn read_manifest(path: &Path) -> Result<Option<Vec<u8>>> {
    use std::io::Read;
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take((MAX_RECORD + 1) as u64).read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}
#[cfg(not(windows))]
pub fn read_manifest(_path: &Path) -> Result<Option<Vec<u8>>> {
    Err(Error::input(
        "native observer operation unsupported outside Windows",
    ))
}

#[cfg(windows)]
#[path = "native_observer_ready_windows.rs"]
mod windows;
#[cfg(windows)]
pub use windows::pin;
#[cfg(not(windows))]
pub fn pin(_manifest: &Manifest) -> Result<Box<dyn Session>> {
    Err(Error::input(
        "native observer operation unsupported outside Windows",
    ))
}
