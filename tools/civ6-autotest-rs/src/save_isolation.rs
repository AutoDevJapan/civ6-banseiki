//! Temporarily give the minimal reliability run an empty autosave directory.
//! Original files are renamed intact, never overwritten or deleted.
use crate::session::SessionType;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};
/// Only Windows sharing/lock violations are retryable. Each retry revalidates
/// the lease/game and recovery state, with one original deadline for all work.
pub fn retry_recovery_with(
    clock: &dyn crate::time::Clock,
    seconds: f64,
    mut check: impl FnMut() -> Result<()>,
    mut recover: impl FnMut() -> Result<()>,
) -> Result<()> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(Error::input(
            "recovery retry budget must be finite and positive",
        ));
    }
    let end = clock.now() + seconds;
    let mut last_error = None;
    loop {
        if clock.now() >= end {
            return Err(
                last_error.unwrap_or_else(|| Error::input("autosave recovery deadline expired"))
            );
        }
        check()?;
        let result = recover();
        match result {
            Err(e)
                if e.kind == crate::ErrorKind::Io
                    && (e.message.contains("(os error 32)")
                        || e.message.contains("(os error 33)")) =>
            {
                let left = end - clock.now();
                if left <= 0.0 {
                    return Err(e);
                }
                eprintln!("Autosave recovery sharing violation; waiting within the original deadline: {e}");
                last_error = Some(e);
                clock.sleep(left.min(0.25));
            }
            other => return other,
        }
    }
}
#[derive(Serialize, Deserialize)]
struct State {
    original: std::path::PathBuf,
    archive: std::path::PathBuf,
    existed: bool,
    original_digest: Option<String>,
    original_identity: Option<(u64, u64)>,
}
#[cfg(windows)]
fn identity(path: &Path) -> Result<(u64, u64)> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(0x02000000)
        .open(path)?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok((
        u64::from(info.dwVolumeSerialNumber),
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    ))
}
#[cfg(unix)]
fn identity(path: &Path) -> Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let m = fs::metadata(path)?;
    Ok((m.dev(), m.ino()))
}
#[cfg(windows)]
fn volume(path: &Path) -> Result<String> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
    };
    let wide = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut mount = vec![0u16; 32768];
    let mut name = vec![0u16; 128];
    if unsafe { GetVolumePathNameW(wide.as_ptr(), mount.as_mut_ptr(), mount.len() as u32) } == 0
        || unsafe {
            GetVolumeNameForVolumeMountPointW(mount.as_ptr(), name.as_mut_ptr(), name.len() as u32)
        } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    let n = name.iter().position(|n| *n == 0).unwrap_or(name.len());
    Ok(String::from_utf16_lossy(&name[..n]))
}
#[cfg(unix)]
fn volume(path: &Path) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    Ok(fs::metadata(path)?.dev().to_string())
}
fn digest(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut h = Sha256::new();
    let mut entries = fs::read_dir(path)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        h.update((name.len() as u64).to_le_bytes());
        h.update(name.as_bytes());
        if e.path().is_dir() {
            h.update([1]);
            h.update(digest(&e.path())?.as_bytes());
        } else {
            h.update([0]);
            h.update(e.metadata()?.len().to_le_bytes());
            let mut f = fs::File::open(e.path())?;
            let mut buf = [0; 65536];
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
        }
    }
    Ok(format!("{:x}", h.finalize()))
}
fn no_links(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(Error::input("autosave isolation refuses a linked path"));
    }
    if path.is_dir() {
        for e in fs::read_dir(path)? {
            no_links(&e?.path())?;
        }
    }
    Ok(())
}
pub fn begin(profile: &Path, archive: &Path, state: &Path) -> Result<()> {
    begin_for_session(profile, archive, state, SessionType::SinglePlayer)
}
pub fn begin_for_session(
    profile: &Path,
    archive: &Path,
    state: &Path,
    session: SessionType,
) -> Result<()> {
    let original = profile
        .join("Documents/My Games/Sid Meier's Civilization VI/Saves")
        .join(session.save_folder())
        .join("auto");
    let original = original
        .parent()
        .ok_or_else(|| Error::input("autosave parent missing"))?
        .canonicalize()?
        .join("auto");
    let archive = archive.canonicalize()?;
    if volume(original.parent().unwrap())? != volume(&archive)? {
        return Err(Error::input(
            "autosave isolation requires an archive on the same volume; state was not created",
        ));
    }
    no_links(&archive)?;
    let backup = archive.join("owner-auto-original");
    let test = archive.join("test-auto");
    if backup.exists() || test.exists() {
        return Err(Error::input(
            "autosave archive already exists; refusing overwrite",
        ));
    }
    let existed = original.exists();
    if existed {
        no_links(&original)?;
    }
    let original_digest = if existed {
        Some(digest(&original)?)
    } else {
        None
    };
    let original_identity = if existed {
        Some(identity(&original)?)
    } else {
        None
    };
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(state)?;
    serde_json::to_writer(
        &mut file,
        &State {
            original: original.clone(),
            archive: archive.clone(),
            existed,
            original_digest,
            original_identity,
        },
    )?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    if existed {
        if let Err(e) = fs::rename(&original, &backup) {
            fs::remove_file(state).map_err(|cleanup| {
                Error::input(format!(
                    "autosave rename failed: {e}; planned state cleanup failed: {cleanup}"
                ))
            })?;
            return Err(e.into());
        }
    }
    // Immutable first line plus an append-only phase: a crash cannot truncate
    // the original recovery paths. Any partial second line is uncertain moved.
    let mut phase = fs::OpenOptions::new().append(true).open(state)?;
    phase.write_all(b"{\"phase\":\"renamed\"}\n")?;
    phase.sync_all()?;
    drop(phase);
    fs::create_dir(&original)?;
    Ok(())
}
pub fn restore(profile: &Path, state: &Path) -> Result<()> {
    let bytes = fs::read(state)?;
    let Some(split) = bytes.iter().position(|b| *b == b'\n') else {
        // begin cannot rename until this complete first record is synced.
        fs::remove_file(state)?;
        return Ok(());
    };
    let s: State = serde_json::from_slice(&bytes[..split])?;
    let restored = bytes[split + 1..]
        .split(|b| *b == b'\n')
        .rfind(|line| !line.is_empty())
        .and_then(|line| serde_json::from_slice::<serde_json::Value>(line).ok())
        .is_some_and(|v| v["phase"] == "restored");
    let moved_or_uncertain = bytes[split + 1..].iter().any(|b| !b.is_ascii_whitespace());
    if !moved_or_uncertain && !s.archive.join("owner-auto-original").exists() {
        // No rename completed, including a later user removal of auto/archive.
        fs::remove_file(state)?;
        return Ok(());
    }
    // The immutable original path already identifies the saved session. Old
    // Single receipts retain their format; never trust an arbitrary parent.
    let session =
        if s.original.parent().and_then(Path::file_name) == Some(std::ffi::OsStr::new("Hotseat")) {
            SessionType::Hotseat
        } else {
            SessionType::SinglePlayer
        };
    let expected = profile
        .join("Documents/My Games/Sid Meier's Civilization VI/Saves")
        .join(session.save_folder())
        .canonicalize()?
        .join("auto");
    if s.original != expected {
        return Err(Error::input(
            "autosave recovery path differs from the profile",
        ));
    }
    no_links(&s.archive)?;
    let backup = s.archive.join("owner-auto-original");
    let test = s.archive.join("test-auto");
    if restored {
        if backup.exists() || (s.existed && !s.original.exists()) {
            return Err(Error::input(
                "completed autosave recovery record contradicts filesystem",
            ));
        }
        fs::remove_file(state)?;
        return Ok(());
    }
    // A crash before the first rename leaves the original folder untouched.
    if s.existed && !backup.exists() {
        if !s.original.exists()
            || (moved_or_uncertain && Some(identity(&s.original)?) != s.original_identity)
        {
            return Err(Error::input("original autosave archive is missing and current folder is not the original; keeping recovery record"));
        }
        fs::remove_file(state)?;
        return Ok(());
    }
    if s.existed {
        no_links(&backup)?;
        if Some(digest(&backup)?) != s.original_digest {
            return Err(Error::input(
                "original autosave archive changed; refusing unverified recovery",
            ));
        }
    }
    if s.original.exists() {
        no_links(&s.original)?;
        if test.exists() {
            return Err(Error::input(
                "test autosave archive already exists; refusing overwrite",
            ));
        }
        fs::rename(&s.original, &test).map_err(|e| {
            Error::new(
                crate::ErrorKind::Io,
                format!(
                    "move test autosave folder {} to {}: {e}",
                    s.original.display(),
                    test.display()
                ),
            )
        })?;
    }
    if s.existed {
        no_links(&backup)?;
        fs::rename(&backup, &s.original).map_err(|e| {
            Error::new(
                crate::ErrorKind::Io,
                format!(
                    "restore original autosave folder {} to {}: {e}",
                    backup.display(),
                    s.original.display()
                ),
            )
        })?;
        if Some(digest(&s.original)?) != s.original_digest
            || Some(identity(&s.original)?) != s.original_identity
        {
            return Err(Error::input(
                "restored original autosave hash or identity differs; retaining recovery receipt",
            ));
        }
    }
    let mut phase = fs::OpenOptions::new().append(true).open(state)?;
    phase.write_all(b"{\"phase\":\"restored\"}\n")?;
    phase.sync_all()?;
    drop(phase);
    fs::remove_file(state)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hotseat_isolation_preserves_other_sessions_and_recovers_from_saved_path() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let saves = p.join("Documents/My Games/Sid Meier's Civilization VI/Saves");
        for folder in ["Single", "Hotseat", "Multi"] {
            fs::create_dir_all(saves.join(folder).join("auto/prev")).unwrap();
            fs::write(saves.join(folder).join("auto/save"), folder.as_bytes()).unwrap();
            fs::write(saves.join(folder).join("auto/prev/save"), b"previous").unwrap();
        }
        let archive = p.join("archive");
        fs::create_dir(&archive).unwrap();
        let state = p.join("state");
        begin_for_session(p, &archive, &state, SessionType::Hotseat).unwrap();
        assert_eq!(fs::read_dir(saves.join("Hotseat/auto")).unwrap().count(), 0);
        for folder in ["Single", "Multi"] {
            assert_eq!(
                fs::read(saves.join(folder).join("auto/save")).unwrap(),
                folder.as_bytes()
            );
        }
        fs::write(saves.join("Hotseat/auto/save"), b"test hotseat save").unwrap();
        restore(p, &state).unwrap();
        assert!(!state.exists());
        assert_eq!(
            fs::read(saves.join("Hotseat/auto/save")).unwrap(),
            b"Hotseat"
        );
        assert_eq!(
            fs::read(saves.join("Hotseat/auto/prev/save")).unwrap(),
            b"previous"
        );
        assert_eq!(
            fs::read(archive.join("test-auto/save")).unwrap(),
            b"test hotseat save"
        );
        for folder in ["Single", "Multi"] {
            assert_eq!(
                fs::read(saves.join(folder).join("auto/save")).unwrap(),
                folder.as_bytes()
            );
        }
    }
    #[derive(Default)]
    struct RetryClock(std::sync::Mutex<f64>);
    impl crate::time::Clock for RetryClock {
        fn now(&self) -> f64 {
            *self.0.lock().unwrap()
        }
        fn sleep(&self, n: f64) {
            *self.0.lock().unwrap() += n;
        }
    }
    #[test]
    fn retry_recovers_transient_sharing_but_never_extends_deadline() {
        use crate::time::Clock;
        let clock = RetryClock::default();
        let mut calls = 0;
        let mut checks = 0;
        retry_recovery_with(
            &clock,
            1.0,
            || {
                checks += 1;
                Ok(())
            },
            || {
                calls += 1;
                if calls < 3 {
                    Err(Error::new(crate::ErrorKind::Io, "locked (os error 32)"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
        assert_eq!(calls, 3);
        assert_eq!(checks, 3);
        assert_eq!(clock.now(), 0.5);
        let clock = RetryClock::default();
        let mut calls = 0;
        assert!(retry_recovery_with(
            &clock,
            1.0,
            || Ok(()),
            || {
                calls += 1;
                Err(Error::new(crate::ErrorKind::Io, "locked (os error 33)"))
            }
        )
        .is_err());
        assert_eq!(clock.now(), 1.0);
        assert_eq!(calls, 4);
    }
    #[test]
    fn retry_stops_on_nonsharing_error_or_lost_lease() {
        let clock = RetryClock::default();
        let mut calls = 0;
        assert!(retry_recovery_with(
            &clock,
            1.0,
            || Ok(()),
            || {
                calls += 1;
                Err(Error::new(crate::ErrorKind::Io, "denied (os error 5)"))
            }
        )
        .is_err());
        assert_eq!(calls, 1);
        let mut calls = 0;
        assert!(retry_recovery_with(
            &clock,
            1.0,
            || Err(Error::input("lost lease")),
            || {
                calls += 1;
                Ok(())
            }
        )
        .is_err());
        assert_eq!(calls, 0);
    }
    #[test]
    fn original_identity_clears_leftover_receipt_after_restore_even_when_owner_data_changes() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let original = p.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single/auto");
        fs::create_dir_all(&original).unwrap();
        fs::write(original.join("save"), b"owner").unwrap();
        let archive = p.join("archive");
        fs::create_dir(&archive).unwrap();
        let state = p.join("state");
        begin(p, &archive, &state).unwrap();
        fs::rename(&original, archive.join("test-auto")).unwrap();
        fs::rename(archive.join("owner-auto-original"), &original).unwrap();
        fs::write(original.join("save"), b"later owner save").unwrap();
        restore(p, &state).unwrap();
        assert!(!state.exists());
        assert_eq!(
            fs::read(original.join("save")).unwrap(),
            b"later owner save"
        );
    }
    #[test]
    fn lost_backup_never_mistakes_test_folder_for_restored_original() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let original = p.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single/auto");
        fs::create_dir_all(&original).unwrap();
        fs::write(original.join("save"), b"owner").unwrap();
        let archive = p.join("archive");
        fs::create_dir(&archive).unwrap();
        let state = p.join("state");
        begin(p, &archive, &state).unwrap();
        fs::remove_file(archive.join("owner-auto-original/save")).unwrap();
        fs::remove_dir(archive.join("owner-auto-original")).unwrap();
        fs::write(original.join("save"), b"test").unwrap();
        assert!(restore(p, &state).is_err());
        assert!(state.exists());
        assert_eq!(fs::read(original.join("save")).unwrap(), b"test");
    }
    #[test]
    fn incomplete_first_record_is_cleared_without_touching_any_save() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let state = p.join("state");
        let save = p.join("normal.Civ6Save");
        fs::write(&state, b"{\"original\":").unwrap();
        fs::write(&save, b"owner").unwrap();
        restore(p, &state).unwrap();
        assert!(!state.exists());
        assert_eq!(fs::read(save).unwrap(), b"owner");
    }
    #[test]
    fn planned_record_survives_user_removal_of_empty_auto_and_archive() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let original = p.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single/auto");
        fs::create_dir_all(&original).unwrap();
        let archive = p.join("archive");
        fs::create_dir(&archive).unwrap();
        let state = p.join("state");
        let s = State {
            original: original.canonicalize().unwrap(),
            archive: archive.clone(),
            existed: true,
            original_digest: Some(digest(&original).unwrap()),
            original_identity: Some(identity(&original).unwrap()),
        };
        fs::write(&state, serde_json::to_string(&s).unwrap() + "\n").unwrap();
        fs::remove_dir(&original).unwrap();
        fs::remove_dir(&archive).unwrap();
        restore(p, &state).unwrap();
        assert!(!state.exists());
        assert!(!original.exists());
    }
    #[cfg(windows)]
    #[test]
    fn completed_recovery_receipt_does_not_block_new_owner_saves_after_delete_failure() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let original = p.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single/auto");
        fs::create_dir_all(&original).unwrap();
        fs::write(original.join("save"), b"owner").unwrap();
        let archive = p.join("archive");
        fs::create_dir(&archive).unwrap();
        let state = p.join("state");
        begin(p, &archive, &state).unwrap();
        fs::write(original.join("save"), b"test").unwrap();
        let pin = fs::OpenOptions::new()
            .read(true)
            .share_mode(3)
            .open(&state)
            .unwrap();
        assert!(restore(p, &state).is_err());
        assert_eq!(fs::read(original.join("save")).unwrap(), b"owner");
        fs::write(original.join("save"), b"later owner save").unwrap();
        drop(pin);
        restore(p, &state).unwrap();
        assert!(!state.exists());
        assert_eq!(
            fs::read(original.join("save")).unwrap(),
            b"later owner save"
        );
    }
    #[cfg(windows)]
    #[test]
    fn first_rename_denied_by_windows_removes_only_planned_receipt() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let original = p.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single/auto");
        fs::create_dir_all(&original).unwrap();
        fs::write(original.join("save"), b"owner").unwrap();
        let archive = p.join("archive");
        fs::create_dir(&archive).unwrap();
        let state = p.join("state");
        let pin = fs::OpenOptions::new()
            .read(true)
            .share_mode(3)
            .custom_flags(0x02000000)
            .open(&original)
            .unwrap();
        assert!(begin(p, &archive, &state).is_err());
        assert!(!state.exists());
        assert_eq!(fs::read(original.join("save")).unwrap(), b"owner");
        assert!(!archive.join("owner-auto-original").exists());
        drop(pin);
    }
    #[test]
    fn planned_crash_record_does_not_block_later_legitimate_autosaves() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let original = p.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single/auto");
        fs::create_dir_all(&original).unwrap();
        fs::write(original.join("save"), b"before").unwrap();
        let archive = p.join("archive");
        fs::create_dir(&archive).unwrap();
        let state = p.join("state");
        let s = State {
            original: original.canonicalize().unwrap(),
            archive,
            existed: true,
            original_digest: Some(digest(&original).unwrap()),
            original_identity: Some(identity(&original).unwrap()),
        };
        fs::write(&state, serde_json::to_string(&s).unwrap() + "\n").unwrap();
        fs::write(original.join("save"), b"later legitimate owner save").unwrap();
        restore(p, &state).unwrap();
        assert!(!state.exists());
        assert_eq!(
            fs::read(original.join("save")).unwrap(),
            b"later legitimate owner save"
        );
    }
    #[test]
    fn changed_original_backup_is_retained_and_never_replaces_the_current_folder() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let original = p.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single/auto");
        fs::create_dir_all(&original).unwrap();
        fs::write(original.join("save"), b"owner").unwrap();
        let archive = p.join("archive");
        fs::create_dir(&archive).unwrap();
        let state = p.join("state");
        begin(p, &archive, &state).unwrap();
        fs::write(archive.join("owner-auto-original/save"), b"changed").unwrap();
        fs::write(original.join("save"), b"test").unwrap();
        assert!(restore(p, &state).is_err());
        assert!(state.exists());
        assert_eq!(fs::read(original.join("save")).unwrap(), b"test");
        assert_eq!(
            fs::read(archive.join("owner-auto-original/save")).unwrap(),
            b"changed"
        );
    }
    #[test]
    fn autosaves_never_overwrite_original_bytes_and_are_restored() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let original = p.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single/auto");
        fs::create_dir_all(&original).unwrap();
        fs::write(original.join("same.Civ6Save"), b"owner").unwrap();
        let archive = p.join("archive");
        fs::create_dir(&archive).unwrap();
        let state = p.join("state.json");
        begin(p, &archive, &state).unwrap();
        assert!(!original.join("same.Civ6Save").exists());
        fs::write(original.join("same.Civ6Save"), b"test").unwrap();
        restore(p, &state).unwrap();
        assert_eq!(fs::read(original.join("same.Civ6Save")).unwrap(), b"owner");
        assert_eq!(
            fs::read(archive.join("test-auto/same.Civ6Save")).unwrap(),
            b"test"
        );
        assert!(!state.exists());
    }
    #[test]
    fn existing_archive_and_registry_are_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let original = p.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single/auto");
        fs::create_dir_all(&original).unwrap();
        fs::write(original.join("save"), b"owner").unwrap();
        let archive = p.join("archive");
        fs::create_dir_all(archive.join("owner-auto-original")).unwrap();
        assert!(begin(p, &archive, &p.join("state")).is_err());
        assert_eq!(fs::read(original.join("save")).unwrap(), b"owner");
    }
}
