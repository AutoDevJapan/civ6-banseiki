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

/// Exclusive write/delete protection matches the acquisition/reaper protocol.
pub struct Lock {
    pub file: File,
    pub info: Value,
    #[cfg(unix)]
    _guard: UnixGuard,
}
impl Lock {
    pub fn open(p: &Paths) -> Result<Option<Self>, String> {
        #[cfg(unix)]
        let guard = UnixGuard::acquire(&p.here.join("civ6.lock"))?;
        let mut opts = OpenOptions::new();
        opts.read(true).write(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            opts.share_mode(1);
        }
        let mut file = match opts.open(p.here.join("civ6.lock")) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            #[cfg(windows)]
            Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let mut b = vec![];
        file.read_to_end(&mut b).map_err(|e| e.to_string())?;
        let info = serde_json::from_slice(&b).map_err(|e| e.to_string())?;
        Ok(Some(Self {
            file,
            info,
            #[cfg(unix)]
            _guard: guard,
        }))
    }
    pub fn save(&mut self) -> Result<(), String> {
        self.file
            .seek(SeekFrom::Start(0))
            .and_then(|_| self.file.set_len(0))
            .and_then(|_| self.file.write_all(self.info.to_string().as_bytes()))
            .and_then(|_| self.file.sync_all())
            .map_err(|e| e.to_string())
    }
    pub fn expired(&mut self, r: &mut impl Runner, p: &Paths, todo: &Path) -> Result<bool, String> {
        let now = r.now();
        let competitor = competitor_since(r, p, &self.info, todo, now)?;
        let Some(since) = competitor else {
            if self
                .info
                .as_object_mut()
                .unwrap()
                .remove("competitor_since")
                .is_some()
            {
                self.save()?;
            }
            return Ok(false);
        };
        // A new holder gets a full lease even if someone was already queued.
        let acquired = self.info["acquired_at"]
            .as_f64()
            .or_else(|| {
                self.file
                    .metadata()
                    .ok()?
                    .created()
                    .ok()?
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|t| t.as_secs_f64())
                    .filter(|t| *t <= now)
            })
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
            self.save()?;
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
