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

/// Captured from one open handle; offsets alone cannot identify a log generation.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LogCursor {
    generation: Option<(u64, u64)>,
    offset: u64,
    prefix: Vec<u8>,
    tail: Vec<u8>,
}
impl LogCursor {
    pub fn mark(path: &Path) -> Result<Self> {
        match File::open(path) {
            Ok(mut file) => Self::capture(&mut file),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) if cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)) => {
                Err(Error::new(
                    crate::ErrorKind::SteamLogBusy,
                    format!("Steam log temporarily locked at {}: {e}", path.display()),
                ))
            }
            Err(e) => Err(e.into()),
        }
    }
    fn capture(file: &mut File) -> Result<Self> {
        let offset = file.metadata()?.len();
        let prefix = read_at(file, 0, offset.min(256))?;
        let tail = read_at(file, offset.saturating_sub(256), offset.min(256))?;
        Ok(Self {
            generation: Some(file_generation(file)?),
            offset,
            prefix,
            tail,
        })
    }
    fn validate(&self, file: &mut File) -> Result<u64> {
        let len = file.metadata()?.len();
        if self
            .generation
            .is_some_and(|id| file_generation(file).ok() != Some(id))
            || len < self.offset
            || read_at(file, 0, self.prefix.len() as u64)? != self.prefix
            || read_at(
                file,
                self.offset.saturating_sub(self.tail.len() as u64),
                self.tail.len() as u64,
            )? != self.tail
        {
            return Err(Error::input(
                "Steam log changed generation; re-mark required, no completion accepted",
            ));
        }
        Ok(len)
    }
}
fn read_at(file: &mut File, offset: u64, len: u64) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::new();
    file.take(len).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != len {
        return Err(Error::input("Steam log shortened during observation"));
    }
    Ok(bytes)
}
#[cfg(windows)]
fn file_generation(file: &File) -> Result<(u64, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok((
        info.dwVolumeSerialNumber as u64,
        ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
    ))
}
#[cfg(unix)]
fn file_generation(file: &File) -> Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let meta = file.metadata()?;
    Ok((meta.dev(), meta.ino()))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExitMark {
    pub cloud: LogCursor,
    pub process: LogCursor,
}
impl ExitMark {
    pub fn capture() -> Result<Self> {
        let path = cloud_log();
        Ok(Self {
            cloud: LogCursor::mark(&path)?,
            process: LogCursor::mark(&path.with_file_name("gameprocess_log.txt"))?,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Exit,
    Prelaunch,
    LaunchIssued,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    Idle,
    Pending,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitEvidence {
    Waiting,
    Failed,
    SyncComplete,
    ReleasedWithoutNewSyncRequest,
}

#[derive(Default, Debug)]
pub struct Monitor {
    path: PathBuf,
    cursor: LogCursor,
    partial: String,
    discard_partial: bool,
    pub states: BTreeMap<u32, SyncState>,
    pub evidence: Vec<String>,
}
impl Monitor {
    pub fn new(path: PathBuf) -> Result<Self> {
        let cursor = LogCursor::mark(&path)?;
        Ok(Self::from_cursor(path, cursor))
    }
    pub fn poll(&mut self) -> Result<()> {
        let mut file = match File::open(&self.path) {
            Ok(file) => file,
            Err(e)
                if e.kind() == std::io::ErrorKind::NotFound && self.cursor.generation.is_none() =>
            {
                return Ok(())
            }
            Err(e) => return Err(e.into()),
        };
        let len = self.cursor.validate(&mut file)?;
        if len - self.cursor.offset > 4 * 1024 * 1024 {
            return Err(Error::input(
                "Steam cloud log grew beyond bounded observation; refusing to discard sync events",
            ));
        }
        let bytes = read_at(&mut file, self.cursor.offset, len - self.cursor.offset)?;
        self.cursor.validate(&mut file)?;
        let mut cursor = self.cursor.clone();
        cursor.generation = Some(file_generation(&file)?);
        cursor.offset = len;
        cursor.prefix = read_at(&mut file, 0, len.min(256))?;
        cursor.tail = read_at(&mut file, len.saturating_sub(256), len.min(256))?;
        self.cursor = cursor;
        self.partial.push_str(&String::from_utf8_lossy(&bytes));
        if self.discard_partial {
            let Some(end) = self.partial.find('\n') else {
                return Ok(());
            };
            self.partial.drain(..=end);
            self.discard_partial = false;
        }
        if let Some(end) = self.partial.rfind('\n') {
            let lines = self.partial[..=end].to_owned();
            self.partial.drain(..=end);
            self.feed(&lines);
        }
        Ok(())
    }
    pub fn from_cursor(path: PathBuf, cursor: LogCursor) -> Self {
        let discard_partial = cursor.offset > 0 && cursor.tail.last() != Some(&b'\n');
        Self {
            path,
            cursor,
            discard_partial,
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
    pub fn status(&self) -> SyncStatus {
        if self.failed() {
            SyncStatus::Failed
        } else if self.pending() {
            SyncStatus::Pending
        } else {
            SyncStatus::Idle
        }
    }
    pub fn for_stage(&self, stage: Stage, path: &Path) -> Result<SyncStatus> {
        let status = self.status();
        if status == SyncStatus::Failed && stage != Stage::LaunchIssued {
            journal(
                path,
                if stage == Stage::Exit {
                    "exit_sync_failed"
                } else {
                    "prelaunch_sync_failed"
                },
                self,
            )?;
            return Err(Error::input(
                "Steam sync failed before launch; URI not sent; PC saves retained",
            ));
        }
        Ok(status)
    }
    pub fn exit_evidence(&self, elapsed: f64, released: bool, game_absent: bool) -> ExitEvidence {
        if self.failed() {
            return ExitEvidence::Failed;
        }
        if self.pending() || elapsed < 10.0 {
            return ExitEvidence::Waiting;
        }
        match self.states.get(&289070) {
            Some(SyncState::Complete) => ExitEvidence::SyncComplete,
            None if released && game_absent => ExitEvidence::ReleasedWithoutNewSyncRequest,
            _ => ExitEvidence::Waiting,
        }
    }
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
pub fn fresh_release(cursor: &LogCursor) -> Result<bool> {
    let path = cloud_log().with_file_name("gameprocess_log.txt");
    fresh_release_at(&path, cursor)
}
fn fresh_release_at(path: &Path, cursor: &LogCursor) -> Result<bool> {
    let mut f = File::open(path)?;
    let len = cursor.validate(&mut f)?;
    if len - cursor.offset > 1024 * 1024 {
        return Err(Error::input(
            "Steam running-list log changed generation or exceeded the observation bound",
        ));
    }
    let bytes = read_at(&mut f, cursor.offset, len - cursor.offset)?;
    cursor.validate(&mut f)?;
    let text = String::from_utf8_lossy(&bytes);
    let text = if cursor.offset > 0 && cursor.tail.last() != Some(&b'\n') {
        text.split_once('\n').map_or("", |(_, tail)| tail)
    } else {
        &text
    };
    let complete = text.rfind('\n').map_or("", |end| &text[..=end]);
    Ok(civ6_released(complete))
}

pub fn journal(path: &Path, phase: &str, monitor: &Monitor) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({"time":chrono::Utc::now(),"phase":phase,"cloud_log":monitor.path,"cursor":monitor.cursor,"states":monitor.states,"evidence":monitor.evidence}),
    )?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn sharing_violation_is_not_an_empty_cursor_and_reopen_preserves_original_bytes() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cloud_log.txt");
        std::fs::write(&path, b"original Steam bytes\n").unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        let error = LogCursor::mark(&path).unwrap_err();
        assert_eq!(error.kind, crate::ErrorKind::SteamLogBusy);
        assert!(error.message.contains("cloud_log.txt") && error.message.contains("os error 32"));
        drop(lock);
        let cursor = LogCursor::mark(&path).unwrap();
        assert!(cursor.generation.is_some());
        assert_eq!(cursor.offset, 21);
        assert_eq!(std::fs::read(&path).unwrap(), b"original Steam bytes\n");
    }
    #[test]
    fn stage_failures_are_journaled_and_only_issued_launch_can_recover() {
        for failure in [
            "Sync failed",
            "Sync conflict detected",
            "Cannot sync while offline",
        ] {
            for stage in [Stage::Exit, Stage::Prelaunch, Stage::LaunchIssued] {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("journal");
                let mut m = Monitor::default();
                m.feed(&format!("[AppID 241100] {failure}\n"));
                let result = m.for_stage(stage, &path);
                if stage == Stage::LaunchIssued {
                    assert_eq!(result.unwrap(), SyncStatus::Failed);
                } else {
                    assert!(result.unwrap_err().message.contains("URI not sent"));
                    let v: serde_json::Value =
                        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
                    assert_eq!(
                        v["phase"],
                        if stage == Stage::Exit {
                            "exit_sync_failed"
                        } else {
                            "prelaunch_sync_failed"
                        }
                    );
                    assert_eq!(v["evidence"][0], format!("[AppID 241100] {failure}"));
                }
            }
        }
    }
    #[test]
    fn controller_completion_requires_separate_civ6_evidence() {
        let mut m = Monitor::default();
        m.feed("[AppID 241100] Successfully synced\n");
        assert_eq!(m.exit_evidence(12.0, false, true), ExitEvidence::Waiting);
        assert_eq!(
            m.exit_evidence(12.0, true, true),
            ExitEvidence::ReleasedWithoutNewSyncRequest
        );
        assert_eq!(m.exit_evidence(9.0, true, true), ExitEvidence::Waiting);
        assert_eq!(m.exit_evidence(12.0, true, false), ExitEvidence::Waiting);
        m.feed("[AppID 289070] Starting sync\n");
        assert_eq!(m.exit_evidence(12.0, true, true), ExitEvidence::Waiting);
        m.feed("[AppID 289070] Successfully synced\n");
        assert_eq!(
            m.exit_evidence(12.0, true, true),
            ExitEvidence::SyncComplete
        );
        for app in [289070, 241100] {
            m.feed(&format!("[AppID {app}] Starting sync\n"));
            assert_eq!(m.exit_evidence(12.0, true, true), ExitEvidence::Waiting);
            m.feed(&format!("[AppID {app}] Sync failed\n"));
            assert_eq!(m.exit_evidence(12.0, true, true), ExitEvidence::Failed);
            m.feed(&format!("[AppID {app}] Successfully synced\n"));
        }
    }
    #[test]
    fn serialized_mark_rejects_exchange_before_first_poll_even_with_identical_prefix() {
        for same_prefix in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("cloud");
            let old = "old prefix padding\n".repeat(40);
            std::fs::write(&path, &old).unwrap();
            let cursor = LogCursor::mark(&path).unwrap();
            let encoded = serde_json::to_string(&cursor).unwrap();
            std::fs::rename(&path, dir.path().join("old")).unwrap();
            let prefix = if same_prefix {
                old.clone()
            } else {
                "new prefix padding\n".repeat(40)
            };
            std::fs::write(&path, prefix + "[AppID 289070] Successfully synced\n").unwrap();
            let mut m = Monitor::from_cursor(path, serde_json::from_str(&encoded).unwrap());
            assert!(m.poll().is_err());
            assert!(m.states.is_empty());
        }
    }
    #[test]
    fn cursor_rejects_same_length_replacement_and_truncation() {
        for replacement in ["same length\n", "short\n"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("cloud");
            std::fs::write(&path, "old content\n").unwrap();
            let mut m = Monitor::new(path.clone()).unwrap();
            std::fs::write(&path, replacement).unwrap();
            assert!(m.poll().is_err());
        }
    }
    #[test]
    fn mark_inside_a_line_never_treats_old_partial_as_a_fresh_event() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cloud");
        std::fs::write(&path, "old incomplete ").unwrap();
        let mut m = Monitor::new(path.clone()).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(b"[AppID 289070] Successfully synced\n[AppID 289070] Starting ")
            .unwrap();
        m.poll().unwrap();
        assert!(m.states.is_empty());
        file.write_all(b"sync\n").unwrap();
        m.poll().unwrap();
        assert!(m.pending());
    }
    #[test]
    fn process_mark_rejects_same_prefix_exchange_and_incomplete_release_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("process");
        std::fs::write(&path, "original\n").unwrap();
        let cursor = LogCursor::mark(&path).unwrap();
        std::fs::rename(&path, dir.path().join("old")).unwrap();
        std::fs::write(&path, "original\nRemove 289070 from running list\n").unwrap();
        assert!(fresh_release_at(&path, &cursor).is_err());
        use std::io::Write;
        let cursor = LogCursor::mark(&path).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(b"Remove 289070 from running list").unwrap();
        assert!(!fresh_release_at(&path, &cursor).unwrap());
        file.write_all(b"\n").unwrap();
        assert!(fresh_release_at(&path, &cursor).unwrap());
    }
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
