//! Read the chain's lease without acquiring, replacing or changing its lock.
use crate::{Error, ErrorKind, Result};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const LEASE_SECONDS: f64 = 1800.0;
fn refused(message: &str) -> Error {
    Error::new(ErrorKind::LiveLease, message)
}

pub fn record_budget(record: &Value, chain: u64, now: f64) -> Result<Option<f64>> {
    if chain == 0 || record["chain_pid"].as_u64() != Some(chain) || record["live_chain"] != true {
        return Err(refused(
            "live chain ownership changed; stopping before further game operations",
        ));
    }
    if record["competitor_since"].is_null() {
        return Ok(None);
    }
    let timestamp = |key: &str| {
        record[key]
            .as_f64()
            .filter(|v| v.is_finite() && *v >= 0.0 && *v <= now)
            .ok_or_else(|| refused("invalid live chain lease timestamp"))
    };
    let since = timestamp("competitor_since")?.max(timestamp("acquired_at")?);
    let remaining = LEASE_SECONDS - (now - since);
    if remaining <= 0.0 {
        return Err(refused("CIV6_LEASE_YIELD: a waiting run has reached the 30-minute lease; close this run and re-queue"));
    }
    Ok(Some(remaining))
}

pub fn environment_budget() -> Result<Option<f64>> {
    let Some(chain) = std::env::var_os(crate::startup::CHAIN_PID_ENV) else {
        return Ok(None);
    };
    let chain = chain
        .to_str()
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or_else(|| refused("invalid live chain identity"))?;
    let lock = PathBuf::from(
        std::env::var_os(crate::startup::LOCK_PATH_ENV)
            .ok_or_else(|| refused("live chain lock path missing"))?,
    );
    if lock.file_name().and_then(|s| s.to_str()) != Some("civ6.lock") {
        return Err(refused("unexpected live chain lock filename"));
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| refused("live chain clock unavailable"))?
        .as_secs_f64();
    directory_budget(&lock, chain, now, |waiter| {
        native_waiter_alive(waiter, &lock)
    })
}

/// Queue entries are the publication of new waiting chains. The owner updates
/// competitor_since only at scenario boundaries, so that field is not live.
pub fn directory_budget(
    lock: &Path,
    chain: u64,
    now: f64,
    mut alive: impl FnMut(&Value) -> Result<bool>,
) -> Result<Option<f64>> {
    let mut record: Value = serde_json::from_slice(
        &std::fs::read(lock).map_err(|_| refused("live chain lock unavailable"))?,
    )
    .map_err(|_| refused("live chain lock changed or is invalid"))?;
    // Validate our identity before observing competitors. Replace stale owner
    // bookkeeping with currently alive queue entries, without writing the lock.
    record
        .as_object_mut()
        .ok_or_else(|| refused("invalid live chain record"))?
        .remove("competitor_since");
    record_budget(&record, chain, now)?;
    let parent = lock
        .parent()
        .ok_or_else(|| refused("live chain directory missing"))?;
    let entries = match std::fs::read_dir(parent.join("queue")) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(refused("live chain queue unavailable")),
    };
    let mut first: Option<f64> = None;
    for entry in entries {
        let entry = entry.map_err(|_| refused("live chain queue entry unavailable"))?;
        if entry.path().extension().is_none_or(|e| e != "json")
            || !entry
                .file_type()
                .map_err(|_| refused("live queue entry type unavailable"))?
                .is_file()
        {
            continue;
        }
        // A newly created/deleted queue file can be temporarily incomplete.
        // Its original since timestamp is used once publication is complete.
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        let Ok(waiter) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let pid = waiter["chain_pid"]
            .as_u64()
            .or_else(|| waiter["pid"].as_u64());
        if !pid.is_some_and(|p| p > 0 && p <= u32::MAX as u64 && p != chain) {
            continue;
        }
        let Some(since) = waiter["since"]
            .as_f64()
            .filter(|s| s.is_finite() && *s >= 0.0 && *s <= now)
        else {
            continue;
        };
        if waiter["live_chain"] != true || !alive(&waiter)? {
            continue;
        }
        first = Some(first.map_or(since, |old| old.min(since)));
    }
    record["competitor_since"] = first.map_or(Value::Null, Value::from);
    record_budget(&record, chain, now)
}

#[cfg(not(windows))]
fn native_waiter_alive(_: &Value, _: &Path) -> Result<bool> {
    Ok(false)
}

#[cfg(windows)]
fn native_waiter_alive(waiter: &Value, lock: &Path) -> Result<bool> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE},
        System::Threading::{
            GetExitCodeProcess, GetProcessTimes, OpenProcess, QueryFullProcessImageNameW,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    };
    struct Owned(HANDLE);
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }
    let pid = waiter["chain_pid"]
        .as_u64()
        .or_else(|| waiter["pid"].as_u64())
        .unwrap() as u32;
    unsafe {
        let process = Owned(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid));
        if process.0.is_null() {
            if std::io::Error::last_os_error().raw_os_error() == Some(87) {
                return Ok(false);
            }
            return Err(refused("cannot verify live queue process"));
        }
        let mut exit = 0;
        if GetExitCodeProcess(process.0, &mut exit) == 0 {
            return Err(refused("cannot read live queue process status"));
        }
        if exit != 259 {
            return Ok(false);
        }
        let mut name = [0u16; 4096];
        let mut size = name.len() as u32;
        if QueryFullProcessImageNameW(process.0, 0, name.as_mut_ptr(), &mut size) == 0 {
            return Err(refused("cannot verify live queue executable"));
        }
        let actual = String::from_utf16_lossy(&name[..size as usize]);
        let expected = lock.parent().unwrap().join("live_chain.exe");
        if actual.replace('\\', "/").to_lowercase()
            != expected.to_string_lossy().replace('\\', "/").to_lowercase()
        {
            return Ok(false);
        }
        let mut created: FILETIME = std::mem::zeroed();
        let mut ended: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();
        if GetProcessTimes(process.0, &mut created, &mut ended, &mut kernel, &mut user) == 0 {
            return Err(refused("cannot verify live queue process creation time"));
        }
        let started = (((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
            as f64
            / 10_000_000.0
            - 11_644_473_600.0;
        // An unrelated process that reused a dead waiter's PID is not a waiter.
        Ok(started <= waiter["since"].as_f64().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn record() -> Value {
        json!({"chain_pid":7,"live_chain":true,"acquired_at":100,"competitor_since":200})
    }
    #[test]
    fn waiter_limits_the_budget_and_yields_at_exact_boundary() {
        assert_eq!(record_budget(&record(), 7, 1999.0).unwrap(), Some(1.0));
        assert_eq!(
            record_budget(&record(), 7, 2000.0).unwrap_err().kind,
            ErrorKind::LiveLease
        );
        assert_eq!(
            record_budget(&record(), 7, 2100.0).unwrap_err().kind,
            ErrorKind::LiveLease
        );
    }
    #[test]
    fn no_waiter_does_not_limit_the_current_run() {
        let mut r = record();
        r.as_object_mut().unwrap().remove("competitor_since");
        assert_eq!(record_budget(&r, 7, 10000.0).unwrap(), None);
    }
    #[test]
    fn new_holder_receives_its_own_lease_even_with_older_waiters() {
        let mut r = record();
        r["acquired_at"] = json!(1000);
        assert_eq!(record_budget(&r, 7, 2000.0).unwrap(), Some(800.0));
    }
    #[test]
    fn ownership_and_invalid_timestamps_stop_operations() {
        assert!(record_budget(&record(), 8, 2000.0).is_err());
        for value in [json!(null), json!("100"), json!(-1), json!(3000)] {
            let mut r = record();
            r["acquired_at"] = value;
            assert!(record_budget(&r, 7, 2000.0).is_err());
        }
    }

    #[test]
    fn queue_publication_is_observed_without_owner_lock_updates() {
        let root = tempfile::tempdir().unwrap();
        let lock = root.path().join("civ6.lock");
        let original = json!({"chain_pid":7,"live_chain":true,"acquired_at":100}).to_string();
        std::fs::write(&lock, &original).unwrap();
        assert_eq!(
            directory_budget(&lock, 7, 2000.0, |_| Ok(true)).unwrap(),
            None
        );
        let queue = root.path().join("queue");
        std::fs::create_dir(&queue).unwrap();
        std::fs::write(
            queue.join("later.json"),
            json!({"chain_pid":8,"live_chain":true,"since":250}).to_string(),
        )
        .unwrap();
        assert_eq!(
            directory_budget(&lock, 7, 2000.0, |_| Ok(true)).unwrap(),
            Some(50.0)
        );
        assert_eq!(
            directory_budget(&lock, 7, 2050.0, |_| Ok(true))
                .unwrap_err()
                .kind,
            ErrorKind::LiveLease
        );
        assert_eq!(std::fs::read_to_string(lock).unwrap(), original);
    }

    #[test]
    fn dead_own_and_incomplete_queue_entries_do_not_extend_or_create_waiters() {
        let root = tempfile::tempdir().unwrap();
        let lock = root.path().join("civ6.lock");
        std::fs::write(&lock, record().to_string()).unwrap();
        let queue = root.path().join("queue");
        std::fs::create_dir(&queue).unwrap();
        for (name, pid, since) in [("own", 7, 0), ("dead", 8, 250), ("alive", 9, 800)] {
            std::fs::write(
                queue.join(format!("{name}.json")),
                json!({"pid":pid,"live_chain":true,"since":since}).to_string(),
            )
            .unwrap();
        }
        std::fs::write(queue.join("incomplete.json"), "{").unwrap();
        assert_eq!(
            directory_budget(&lock, 7, 2000.0, |v| Ok(v["pid"] == 9)).unwrap(),
            Some(600.0)
        );
        assert_eq!(
            directory_budget(&lock, 7, 2000.0, |_| Ok(false)).unwrap(),
            None
        );
    }
}
