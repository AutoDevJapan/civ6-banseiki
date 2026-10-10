//! Contention leases are stored in the protected lock, shared by chain and tick.
use crate::{chain, read_json, ship::truthy, Paths, Runner};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

/// Advisory cross-process guard for the Unix lock-file protocol. Every
/// legitimate lock writer uses this persistent inode; never unlink it.
#[cfg(unix)]
pub(crate) struct UnixGuard {
    _file: File,
}

#[cfg(unix)]
impl UnixGuard {
    pub(crate) fn acquire(lock: &Path) -> Result<Self, String> {
        use std::{
            thread,
            time::{Duration, Instant},
        };

        let path = lock.with_extension("lock.guard");
        let parent = path.parent().ok_or("Unix lock guard has no parent")?;
        fs::create_dir_all(parent)
            .map_err(|e| format!("create Unix lock guard directory {}: {e}", parent.display()))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| format!("open Unix lock guard {}: {e}", path.display()))?;
        // The current Civ6 queue has few concurrent claimants. Owner checks can
        // include several process queries (30 s each); cap guard contention at
        // 180 s so a stalled checker cannot block another claimant forever.
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(std::fs::TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return Err(format!(
                            "acquire Unix lock guard {}: timed out",
                            path.display()
                        ));
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(std::fs::TryLockError::Error(e)) => {
                    return Err(format!("acquire Unix lock guard {}: {e}", path.display()))
                }
            }
        }
    }
}

pub const LEASE_SECONDS: f64 = 1800.;
pub const STEP_IDLE_SECONDS: f64 = 600.;

pub fn todo_path(p: &Paths) -> PathBuf {
    p.here.parent().unwrap_or(&p.here).join("todo/todo.json")
}
fn epoch(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| {
        chrono::DateTime::parse_from_rfc3339(v.as_str()?)
            .ok()
            .map(|t| t.timestamp() as f64)
    })
}
pub fn ready_item(i: &Value, now: f64) -> bool {
    i["status"] == "todo"
        && truthy(&i["dispatchable"])
        && truthy(&i["spec"])
        && !truthy(&i["scheduler"]["question_close_pending"])
        && i["scheduler"]["not_before"].as_f64().unwrap_or(0.) <= now
        && needs_civ6(i)
}
pub fn needs_civ6(i: &Value) -> bool {
    i["needs"]
        .as_array()
        .is_some_and(|a| a.iter().any(|n| n == "civ6"))
}
pub fn items(todo: &Path) -> Result<Vec<Value>, String> {
    match fs::read(todo) {
        Ok(b) => Ok(
            serde_json::from_slice::<Value>(&b).map_err(|e| e.to_string())?["items"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.to_string()),
    }
}
pub fn item_since(i: &Value, now: f64) -> f64 {
    epoch(&i["created_s"])
        .or_else(|| epoch(&i["created_at"]))
        .unwrap_or(now)
        .max(i["scheduler"]["not_before"].as_f64().unwrap_or(0.))
        .min(now)
}
pub fn same_owner(a: &Value, b: &Value) -> bool {
    for ka in ["job", "owner_job"] {
        for kb in ["job", "owner_job"] {
            if a[ka].as_str().is_some_and(|j| !j.is_empty()) && a[ka] == b[kb] {
                return true;
            }
        }
    }
    !a["pid"].is_null() && a["pid"] == b["pid"]
}

/// Earliest currently live competitor; never infer contention from a dead job.
pub fn competitor_since(
    r: &mut impl Runner,
    p: &Paths,
    owner: &Value,
    todo: &Path,
    now: f64,
) -> Result<Option<f64>, String> {
    let mut first: Option<f64> = None;
    match fs::read_dir(p.here.join("queue")) {
        Ok(entries) => {
            for entry in entries {
                let path = entry.map_err(|e| e.to_string())?.path();
                if path.extension().is_none_or(|e| e != "json") {
                    continue;
                }
                let info = match read_json(&path) {
                    Ok(v) => v,
                    Err(_) if !path.exists() => continue,
                    Err(e) => return Err(e),
                };
                if same_owner(&info, owner) || !chain::holder_alive(r, p, &info)? {
                    continue;
                }
                let since = info["since"].as_f64().unwrap_or(now).min(now);
                first = Some(first.map_or(since, |f| f.min(since)));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    for i in items(todo)? {
        let assigned = json!({"job":i["scheduler"]["job"]});
        if !same_owner(owner, &assigned) && ready_item(&i, now) {
            let since = item_since(&i, now);
            first = Some(first.map_or(since, |f| f.min(since)));
        }
    }
    Ok(first)
}

/// Short record reads deny concurrent writes/deletion, but permit recovery and
/// monitor readers. Never retain a write handle across external owner queries.
pub(crate) fn read_options() -> OpenOptions {
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.share_mode(1);
    }
    opts
}

pub(crate) fn write_options() -> OpenOptions {
    let mut opts = OpenOptions::new();
    opts.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.share_mode(1);
    }
    opts
}

fn record_bytes(file: &mut File) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(32769)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32768 {
        return Err("Civ6 lease record too large".into());
    }
    Ok(bytes)
}

#[cfg(windows)]
fn file_identity(file: &File) -> Result<[u64; 4], String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(format!(
            "identify Civ6 lock handle: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok([
        u64::from(info.dwVolumeSerialNumber),
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        (u64::from(info.ftCreationTime.dwHighDateTime) << 32)
            | u64::from(info.ftCreationTime.dwLowDateTime),
        0,
    ])
}

#[cfg(unix)]
fn file_identity(file: &File) -> Result<[u64; 4], String> {
    use std::os::unix::fs::MetadataExt;
    let info = file.metadata().map_err(|e| e.to_string())?;
    Ok([
        info.dev(),
        info.ino(),
        info.ctime() as u64,
        info.ctime_nsec() as u64,
    ])
}

/// No handles survive the short read: even an attribute-only Windows handle
/// would keep deletion pending throughout a slow probe. Revalidate file identity,
/// record bytes and the acquisition token/generation under the mutation handle.
pub(crate) struct Snapshot {
    identity: [u64; 4],
    created_at: Option<f64>,
    pub(crate) bytes: Vec<u8>,
}
impl Snapshot {
    pub(crate) fn capture(file: &mut File, _path: &Path) -> Result<Self, String> {
        let identity = file_identity(file)?;
        let created_at = file
            .metadata()
            .ok()
            .and_then(|m| m.created().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|t| t.as_secs_f64());
        let bytes = record_bytes(file)?;
        Ok(Self {
            identity,
            bytes,
            created_at,
        })
    }
    pub(crate) fn open(path: &Path) -> Result<Option<Self>, String> {
        match Self::try_open(path) {
            Ok(snapshot) => Ok(Some(snapshot)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            #[cfg(windows)]
            Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) => Ok(None),
            Err(e) => Err(format!("open Civ6 lock snapshot {}: {e}", path.display())),
        }
    }
    pub(crate) fn try_open(path: &Path) -> std::io::Result<Self> {
        #[cfg(unix)]
        let _guard = UnixGuard::acquire(path).map_err(std::io::Error::other)?;
        let mut file = read_options().open(path)?;
        Self::capture(&mut file, path).map_err(std::io::Error::other)
    }
    pub(crate) fn info(&self) -> Result<Value, String> {
        serde_json::from_slice(&self.bytes).map_err(|e| format!("parse Civ6 lock snapshot: {e}"))
    }
    pub(crate) fn matches(&self, file: &mut File) -> Result<bool, String> {
        Ok(self.identity == file_identity(file)? && self.bytes == record_bytes(file)?)
    }
    pub(crate) fn same_file(&self, other: &Self) -> Result<bool, String> {
        #[cfg(windows)]
        {
            Ok(self.identity == other.identity)
        }
        #[cfg(unix)]
        {
            Ok(self.identity[..2] == other.identity[..2])
        }
    }
    pub(crate) fn matches_acquisition(&self, file: &mut File, path: &Path) -> Result<bool, String> {
        let current = Self::capture(file, path)?;
        Ok(self.same_file(&current)? && same_acquisition(&self.info()?, &current.info()?))
    }
}

/// Fairness fields may change under the same owner; these fields identify a
/// specific acquisition, including process generation and token.
pub(crate) fn same_acquisition(a: &Value, b: &Value) -> bool {
    [
        "job",
        "owner_job",
        "pid",
        "chain_pid",
        "pid_created_filetime",
        "live_chain",
        "lock_token",
        "acquired_at",
    ]
    .iter()
    .all(|key| a[*key] == b[*key])
}

/// An optimistic snapshot, not a lifetime write lock. All external probes run
/// before the short exclusive update, which rechecks both object and contents.
pub struct Lock {
    pub info: Value,
    snapshot: Snapshot,
    path: PathBuf,
}
enum SaveError {
    Open(std::io::Error),
    Other(String),
}
impl Lock {
    pub fn open(p: &Paths) -> Result<Option<Self>, String> {
        let path = p.here.join("civ6.lock");
        let Some(snapshot) = Snapshot::open(&path)? else {
            return Ok(None);
        };
        let info = snapshot.info()?;
        Ok(Some(Self {
            info,
            snapshot,
            path,
        }))
    }
    pub fn save(&mut self) -> Result<(), String> {
        self.save_with_runner(&mut crate::process::RealRunner)
    }
    fn save_with_runner(&mut self, r: &mut impl Runner) -> Result<(), String> {
        #[cfg(not(windows))]
        {
            let _ = r;
            self.try_save().map_err(|error| match error {
                SaveError::Open(error) => format!(
                    "open Civ6 lock for fairness update {}: {error}",
                    self.path.display()
                ),
                SaveError::Other(error) => error,
            })
        }
        #[cfg(windows)]
        {
            let deadline = r.monotonic() + 2.0;
            loop {
                if r.monotonic() >= deadline {
                    return Err(
                        "Civ6 fairness update sharing conflict exceeded its deadline".into(),
                    );
                }
                match self.try_save() {
                    #[cfg(windows)]
                    Err(SaveError::Open(error))
                        if matches!(error.raw_os_error(), Some(32 | 33)) =>
                    {
                        // Other readers may overlap the snapshot-to-write handoff.
                        // Revalidate before each retry, never turn a new owner into ours.
                        self.verify()?;
                        let delay = (deadline - r.monotonic()).clamp(0.0, 0.05);
                        r.sleep(delay)?;
                    }
                    Err(SaveError::Open(error)) => {
                        return Err(format!(
                            "open Civ6 lock for fairness update {}: {error}",
                            self.path.display()
                        ))
                    }
                    Err(SaveError::Other(error)) => return Err(error),
                    Ok(()) => return Ok(()),
                }
            }
        }
    }
    fn try_save(&mut self) -> Result<(), SaveError> {
        #[cfg(unix)]
        let _guard = UnixGuard::acquire(&self.path).map_err(SaveError::Other)?;
        let mut file = write_options().open(&self.path).map_err(SaveError::Open)?;
        if !self.snapshot.matches(&mut file).map_err(SaveError::Other)? {
            return Err(SaveError::Other(
                "Civ6 lock ownership or snapshot changed before fairness update".into(),
            ));
        }
        let bytes = self.info.to_string().into_bytes();
        file.seek(SeekFrom::Start(0))
            .and_then(|_| file.set_len(0))
            .and_then(|_| file.write_all(&bytes))
            .and_then(|_| file.sync_all())
            .map_err(|e| {
                format!(
                    "write Civ6 lock fairness update {}: {e}",
                    self.path.display()
                )
            })
            .map_err(SaveError::Other)?;
        self.snapshot = Snapshot::capture(&mut file, &self.path).map_err(SaveError::Other)?;
        Ok(())
    }
    fn verify(&self) -> Result<(), String> {
        let current =
            Snapshot::open(&self.path)?.ok_or("Civ6 lock unavailable after fairness probe")?;
        if !self.snapshot.same_file(&current)? || current.bytes != self.snapshot.bytes {
            return Err("Civ6 lock ownership or snapshot changed during fairness probe".into());
        }
        Ok(())
    }
    pub fn expired(&mut self, r: &mut impl Runner, p: &Paths, todo: &Path) -> Result<bool, String> {
        let now = r.now();
        let competitor = competitor_since(r, p, &self.info, todo, now)?;
        self.verify()?;
        let Some(since) = competitor else {
            if self
                .info
                .as_object_mut()
                .unwrap()
                .remove("competitor_since")
                .is_some()
            {
                self.save_with_runner(r)?;
            }
            return Ok(false);
        };
        // A new holder gets a full lease even if someone was already queued.
        let acquired = self.info["acquired_at"]
            .as_f64()
            .or_else(|| self.snapshot.created_at.filter(|t| *t <= now))
            .unwrap_or(0.);
        let since = since.max(acquired);
        let first = self.info["competitor_since"]
            .as_f64()
            .unwrap_or(since)
            .min(since)
            .min(now);
        if self.info["competitor_since"].as_f64() != Some(first) {
            self.info["acquired_at"] = json!(acquired);
            self.info["competitor_since"] = json!(first);
            self.save_with_runner(r)?;
        }
        Ok(now - first >= LEASE_SECONDS)
    }
}

/// Scan only artifact directories, without following junctions back into worktrees.
pub fn newest_step(root: &Path) -> Result<Option<f64>, String> {
    let mut newest: Option<f64> = None;
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let typ = entry.file_type().map_err(|e| e.to_string())?;
        let found = if typ.is_dir() {
            newest_step(&entry.path())?
        } else if typ.is_file() && entry.file_name() == "steps.jsonl" {
            Some(
                entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .map_err(|e| e.to_string())?
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_secs_f64(),
            )
        } else {
            None
        };
        if let Some(t) = found {
            newest = Some(newest.map_or(t, |v| v.max(t)));
        }
    }
    Ok(newest)
}

pub fn newest_artifact_step(p: &Paths) -> Result<Option<f64>, String> {
    let mut newest = newest_step(&p.wt.join("tools/civ6-autotest/artifacts"))?;
    let entries = match fs::read_dir(&p.wt) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(newest),
        Err(e) => return Err(e.to_string()),
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        if let Some(t) = newest_step(&entry.path().join("tools/civ6-autotest/artifacts"))? {
            newest = Some(newest.map_or(t, |v| v.max(t)));
        }
    }
    Ok(newest)
}
