//! Observe fresh Steam Cloud events. Cache-file writes are not sync success.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    Pending,
    Complete,
    Failed,
    Conflict,
    Offline,
}

#[derive(Default, Debug)]
pub struct Monitor {
    path: PathBuf,
    offset: u64,
    partial: String,
    prefix: Vec<u8>,
    prefix_initialized: bool,
    pub states: BTreeMap<u32, SyncState>,
    pub evidence: Vec<String>,
}
impl Monitor {
    pub fn new(path: PathBuf) -> Result<Self> {
        let offset = match std::fs::metadata(&path) {
            Ok(meta) => meta.len(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            prefix: read_prefix(&path, 256)?,
            prefix_initialized: true,
            path,
            offset,
            ..Self::default()
        })
    }
    pub fn poll(&mut self) -> Result<()> {
        let mut file = match File::open(&self.path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let len = file.metadata()?.len();
        let prefix = read_prefix(
            &self.path,
            if self.prefix_initialized {
                self.prefix.len() as u64
            } else {
                256
            },
        )?;
        if len < self.offset || (self.prefix_initialized && prefix != self.prefix) {
            if self.pending() {
                return Err(Error::input(
                    "Steam cloud log rotated during pending sync; completion cannot be proven",
                ));
            }
            self.offset = 0;
            self.partial.clear();
            self.states.clear();
            self.evidence
                .push("cloud_log rotated; observing new generation".into());
            self.prefix = read_prefix(&self.path, 256)?;
        } else if !self.prefix_initialized {
            self.prefix = prefix;
        }
        self.prefix_initialized = true;
        if len - self.offset > 4 * 1024 * 1024 {
            return Err(Error::input(
                "Steam cloud log grew beyond bounded observation; refusing to discard sync events",
            ));
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut bytes = Vec::new();
        file.take(len - self.offset).read_to_end(&mut bytes)?;
        self.offset += bytes.len() as u64;
        self.partial.push_str(&String::from_utf8_lossy(&bytes));
        if let Some(end) = self.partial.rfind('\n') {
            let lines = self.partial[..=end].to_owned();
            self.partial.drain(..=end);
            self.feed(&lines);
        }
        Ok(())
    }
    pub fn from_offset(path: PathBuf, offset: u64) -> Self {
        Self {
            path,
            offset,
            ..Self::default()
        }
    }
    pub fn feed(&mut self, text: &str) {
        for line in text.lines() {
            let Some((app, state)) = classify(line) else {
                continue;
            };
            self.states.insert(app, state);
            self.evidence.push(line.to_owned());
        }
    }
    pub fn pending(&self) -> bool {
        self.states.values().any(|s| *s == SyncState::Pending)
    }
    pub fn failed(&self) -> bool {
        self.states.values().any(|s| {
            matches!(
                s,
                SyncState::Failed | SyncState::Conflict | SyncState::Offline
            )
        })
    }
}

fn read_prefix(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut prefix = Vec::new();
    file.take(limit).read_to_end(&mut prefix)?;
    Ok(prefix)
}

pub fn classify(line: &str) -> Option<(u32, SyncState)> {
    let rest = line.split_once("[AppID ")?.1;
    let (id, text) = rest.split_once(']')?;
    let id: u32 = id.parse().ok()?;
    // 241100 is controller configuration: it can block Civ6 even when game
    // saves use LOCAL_STORAGE. Other apps must never block this harness.
    if !matches!(id, 289070 | 241100) {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    let state = if lower.contains("conflict detected")
        || lower.contains("sync conflict")
        || lower.contains("files are in conflict")
    {
        SyncState::Conflict
    } else if lower.contains("sync offline") || lower.contains("cannot sync while offline") {
        SyncState::Offline
    } else if lower.contains("byieldingbuildfilelisttosync failed")
        || lower.contains("syncfailed")
        || lower.contains("sync failed")
        || lower.contains("upload failed")
        || lower.contains("download failed")
    {
        SyncState::Failed
    } else if lower.contains("upload complete in build list")
        || lower.contains("download complete in build list")
        || lower.contains("successfully synced")
    {
        SyncState::Complete
    } else if lower.contains("starting sync") {
        SyncState::Pending
    } else {
        return None;
    };
    Some((id, state))
}

pub fn cloud_log() -> PathBuf {
    std::env::var_os("CIV6_STEAM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:/Program Files (x86)/Steam"))
        .join("logs/cloud_log.txt")
}
/// A fresh Steam running-list removal proves the client observed the game exit.
/// A later tracked process invalidates that observation.
pub fn civ6_released(text: &str) -> bool {
    let mut released = false;
    for line in text.lines() {
        if line.contains("AppID 289070 adding PID ") {
            released = false;
        }
        if line.ends_with("Remove 289070 from running list") {
            released = true;
        }
    }
    released
}
pub fn fresh_release(offset: u64) -> Result<bool> {
    let path = cloud_log().with_file_name("gameprocess_log.txt");
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    if len < offset || len - offset > 1024 * 1024 {
        return Err(Error::input(
            "Steam running-list log changed generation or exceeded the observation bound",
        ));
    }
    f.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::new();
    f.take(len - offset).read_to_end(&mut bytes)?;
    Ok(civ6_released(&String::from_utf8_lossy(&bytes)))
}

pub fn journal(path: &Path, phase: &str, monitor: &Monitor) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({"time":chrono::Utc::now(),"phase":phase,"cloud_log":monitor.path,"offset":monitor.offset,"states":monitor.states,"evidence":monitor.evidence}),
    )?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn running_list_release_requires_civ6_and_is_invalidated_by_a_new_process() {
        assert!(!civ6_released("[date] Remove 123 from running list"));
        assert!(!civ6_released(
            "[date] AppID 289070 no longer tracking PID 10, exit code 0"
        ));
        assert!(civ6_released("[date] Remove 289070 from running list"));
        assert!(!civ6_released("[date] Remove 289070 from running list\n[date] AppID 289070 adding PID 11 as a tracked process"));
    }
    #[test]
    fn controller_failure_is_not_a_save_conflict_or_success() {
        let mut m = Monitor::default();
        m.feed("[AppID 241100] Starting sync (AC Launch,down,)\n[AppID 241100] BYieldingBuildFileListToSync Failed for 'AC Launch,down,'\n[AppID 241100] YldWriteCacheDirectoryToFile - saved to 'cache'\n");
        assert_eq!(m.states[&241100], SyncState::Failed);
        assert!(m.failed());
        assert!(!m.pending());
        assert!(!m.states.contains_key(&289070));
    }
    #[test]
    fn unrelated_sync_and_autocloud_scan_completion_do_not_release_pending() {
        let mut m = Monitor::default();
        m.feed("[AppID 289070] Starting sync (up,)\n[AppID 289070] AutoCloud complete\n[AppID 7] Successfully synced to ChangeNumber 5\n");
        assert!(m.pending());
        m.feed("[AppID 289070] Upload complete in build list\n");
        assert!(!m.pending());
        assert!(!m.failed());
    }
    #[test]
    fn fresh_cursor_ignores_stale_failure_and_handles_split_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cloud_log.txt");
        std::fs::write(&path, "[AppID 241100] Failed\n").unwrap();
        let mut m = Monitor::new(path.clone()).unwrap();
        m.poll().unwrap();
        assert!(m.states.is_empty());
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        f.write_all(b"[AppID 289070] Starting ").unwrap();
        m.poll().unwrap();
        assert!(!m.pending());
        f.write_all(b"sync (up,)\n").unwrap();
        m.poll().unwrap();
        assert!(m.pending());
        std::fs::write(&path, "[AppID 289070] Successfully synced\n").unwrap();
        assert!(m.poll().is_err());
        assert!(m.pending());
    }
    #[test]
    fn longer_replacement_log_cannot_clear_pending_sync() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cloud_log.txt");
        std::fs::write(&path, "old generation\n").unwrap();
        let mut m = Monitor::new(path.clone()).unwrap();
        m.feed("[AppID 289070] Starting sync (up,)\n");
        std::fs::write(
            path,
            "new generation with longer prefix\n[AppID 289070] Upload complete in build list\n",
        )
        .unwrap();
        assert!(m.poll().is_err());
        assert!(m.pending());
    }
    #[test]
    fn filenames_and_scan_messages_are_not_sync_failures() {
        for line in [
            "[AppID 289070] Persisting file JAC-failed-conflict-offline.Civ6Save to the cloud",
            "[AppID 241100] Failed to find optional controller template",
        ] {
            assert_eq!(classify(line), None);
        }
        assert_eq!(
            classify("[AppID 289070] Sync conflict detected"),
            Some((289070, SyncState::Conflict))
        );
        assert_eq!(
            classify("[AppID 289070] Cannot sync while offline"),
            Some((289070, SyncState::Offline))
        );
    }
}
