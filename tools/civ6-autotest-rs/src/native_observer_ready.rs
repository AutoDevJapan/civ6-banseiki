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

#[derive(Clone, PartialEq, Eq, Deserialize)]
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
    validate_lifecycle_evidence(m, lease, ready, identities, game_pid, true)
}

pub fn validate_finish_evidence(
    m: &Manifest,
    lease: &[u8],
    ready: &[u8],
    identities: [Identity; 3],
    game_pid: u32,
) -> Result<bool> {
    validate_lifecycle_evidence(m, lease, ready, identities, game_pid, false)?;
    Ok(identities[2].alive)
}

fn validate_lifecycle_evidence(
    m: &Manifest,
    lease: &[u8],
    ready: &[u8],
    identities: [Identity; 3],
    game_pid: u32,
    observer_must_be_alive: bool,
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
    for (index, (actual, (pid, created))) in identities
        .into_iter()
        .zip([
            (m.chain_pid, m.chain_created),
            (m.game_pid, m.game_created),
            (m.observer_pid, m.observer_created),
        ])
        .enumerate()
    {
        if ((!actual.alive) && (index != 2 || observer_must_be_alive))
            || actual.pid != pid
            || actual.created != created
        {
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
    /// The short lease file pin is only needed across the acknowledgement.
    fn after_ack(&mut self) {}
    /// Keep the original process handles while allowing the observer to exit.
    fn check_finish(&mut self, _manifest: &Manifest, _game_pid: u32) -> Result<bool> {
        Err(Error::input(
            "native observer session cannot prove completed cleanup",
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControllerCompletion {
    NotStarted,
    Exited(i32),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Done {
    version: u32,
    run_tag: String,
    chain_pid: u32,
    chain_created: u64,
    observer_started: bool,
    manifest: Option<Manifest>,
    exit_code: Option<i32>,
}

pub fn sidecar(path: &Path, suffix: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}{suffix}", path.display()))
}

pub fn parse_completion(
    path: &Path,
    bytes: &[u8],
    started: Option<&[u8]>,
) -> Result<(ControllerCompletion, u32, u64)> {
    let done: Done = json(bytes)?;
    let tag = path
        .file_name()
        .and_then(|p| p.to_str())
        .and_then(|p| p.strip_suffix("-handoff.json"))
        .ok_or_else(|| Error::input("native completion handoff name invalid"))?;
    if done.version != 1 || done.run_tag != tag || done.chain_pid == 0 || done.chain_created == 0 {
        return Err(Error::input(
            "native controller completion belongs to another request",
        ));
    }
    let result = if done.observer_started {
        let original = parse_manifest(
            path,
            started.ok_or_else(|| Error::input("native observer launch record missing"))?,
        )?;
        if done.manifest.as_ref() != Some(&original)
            || done.chain_pid != original.chain_pid
            || done.chain_created != original.chain_created
        {
            return Err(Error::input(
                "native observer completion differs from launch identity",
            ));
        }
        ControllerCompletion::Exited(
            done.exit_code
                .ok_or_else(|| Error::input("native observer exit code missing"))?,
        )
    } else {
        if done.manifest.is_some() || done.exit_code.is_some() || started.is_some() {
            return Err(Error::input(
                "native observer launch cannot be labelled not started",
            ));
        }
        ControllerCompletion::NotStarted
    };
    Ok((result, done.chain_pid, done.chain_created))
}

#[cfg(windows)]
pub fn request_observer(path: &Path) -> Result<()> {
    use std::io::Write;
    let tag = path
        .file_name()
        .and_then(|p| p.to_str())
        .and_then(|p| p.strip_suffix("-handoff.json"))
        .ok_or_else(|| Error::input("native request name invalid"))?;
    let tmp = sidecar(path, ".request.tmp");
    let target = sidecar(path, ".request");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    file.write_all(&serde_json::to_vec(
        &serde_json::json!({"version":1,"run_tag":tag}),
    )?)?;
    file.sync_all()?;
    drop(file);
    windows::publish_request(&tmp, &target)
}

#[cfg(not(windows))]
pub fn request_observer(_path: &Path) -> Result<()> {
    Err(Error::input(
        "native observer operation unsupported outside Windows",
    ))
}

#[cfg(windows)]
pub fn controller_completion(path: &Path) -> Result<Option<ControllerCompletion>> {
    let Some(bytes) = read_manifest(&sidecar(path, ".done"))? else {
        return Ok(None);
    };
    let started = read_manifest(&sidecar(path, ".started"))?;
    let (completion, chain_pid, chain_created) =
        parse_completion(path, &bytes, started.as_deref())?;
    windows::check_completion_chain(chain_pid, chain_created)?;
    Ok(Some(completion))
}

#[cfg(not(windows))]
pub fn controller_completion(_path: &Path) -> Result<Option<ControllerCompletion>> {
    Err(Error::input(
        "native observer operation unsupported outside Windows",
    ))
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
