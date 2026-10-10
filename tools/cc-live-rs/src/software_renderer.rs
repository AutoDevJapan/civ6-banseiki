//! CPU rendering scoped to the standard chain's actual owned lease.
use crate::Paths;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

fn options(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let mut found = [0; 2];
    let mut result = String::new();
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\r', '\n']);
        let selected = match bare.split_whitespace().next() {
            Some("DeviceID") => Some((0, "DeviceID 140")),
            Some("AllowSWRenderer") => Some((1, "AllowSWRenderer 1")),
            _ => None,
        };
        if let Some((index, replacement)) = selected {
            found[index] += 1;
            result.push_str(replacement);
            result.push_str(&line[bare.len()..]);
        } else {
            result.push_str(line);
        }
    }
    if found != [1, 1] {
        return Err("software renderer requires exactly one DeviceID and AllowSWRenderer".into());
    }
    Ok(result.into_bytes())
}

fn replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension(format!("cpu-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    #[cfg(windows)]
    let result = {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let from: Vec<_> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe {
            MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            Err(std::io::Error::last_os_error().to_string())
        } else {
            Ok(())
        }
    };
    #[cfg(not(windows))]
    let result = fs::rename(&temporary, path).map_err(|e| e.to_string());
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn publish_backup(path: &Path, bytes: &[u8]) -> Result<(), String> {
    // Publish only complete synced bytes. A process crash during write must
    // not leave a truncated original which a later ordinary run could restore.
    let temporary = path.with_extension(format!(
        "cpu-original-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    // Create-new atomic publication on the same filesystem; no overwrite of
    // an already published recovery record. The settings are still untouched.
    let published = fs::hard_link(&temporary, path).map_err(|e| e.to_string());
    let _ = fs::remove_file(&temporary);
    published
}

pub(crate) struct Profile {
    lock: File,
    token: String,
    settings: PathBuf,
    backup: PathBuf,
}
impl Profile {
    pub(crate) fn recover(paths: &Paths, token: &str) -> Result<Option<Self>, String> {
        let backup = paths.here.join("software-renderer-original.bin");
        match fs::read(&backup) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            // Any other read failure also needs a retained recovery guard.
            // The caller owns it BEFORE trying restoration, so an error cannot
            // drop protection and accidentally permit normal lock release.
            _ => {
                let mut open = OpenOptions::new();
                open.read(true);
                #[cfg(windows)]
                {
                    use std::os::windows::fs::OpenOptionsExt;
                    open.share_mode(3);
                }
                let mut profile = Self {
                    lock: open
                        .open(paths.here.join("civ6.lock"))
                        .map_err(|e| e.to_string())?,
                    token: token.into(),
                    settings: paths.profile.join(
                        "AppData/Local/Firaxis Games/Sid Meier's Civilization VI/AppOptions.txt",
                    ),
                    backup,
                };
                profile.validate_lease()?;
                Ok(Some(profile))
            }
        }
    }
    // The caller has closed Civ6 while owning the standard lock. A read handle
    // which denies DELETE keeps restoration ahead of releasing that lease.
    pub(crate) fn apply(paths: &Paths, token: &str) -> Result<Self, String> {
        let mut open = OpenOptions::new();
        open.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            open.share_mode(3);
        }
        let mut profile = Self {
            lock: open
                .open(paths.here.join("civ6.lock"))
                .map_err(|e| e.to_string())?,
            token: token.into(),
            settings: paths
                .profile
                .join("AppData/Local/Firaxis Games/Sid Meier's Civilization VI/AppOptions.txt"),
            backup: paths.here.join("software-renderer-original.bin"),
        };
        profile.validate_lease()?;
        // An interrupted prior CPU run keeps its original bytes. Restore them
        // first under this newly acquired lease, with the game already absent.
        let original = fs::read(&profile.settings).map_err(|e| e.to_string())?;
        let modified = options(&original)?;
        publish_backup(&profile.backup, &original)?;
        replace(&profile.settings, &modified)?;
        Ok(profile)
    }
    fn validate_lease(&mut self) -> Result<(), String> {
        self.lock
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        let mut text = String::new();
        self.lock
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        let owner: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if owner["lock_token"].as_str() != Some(self.token.as_str())
            || owner["live_chain"] != true
            || owner["chain_pid"].as_u64() != Some(u64::from(std::process::id()))
        {
            return Err("software renderer requires this standard chain's current lease".into());
        }
        Ok(())
    }
    // Called after verified game closure and before deleting the lock.
    pub(crate) fn restore(&mut self) -> Result<(), String> {
        self.validate_lease()?;
        let original = fs::read(&self.backup).map_err(|e| e.to_string())?;
        options(&original)?;
        replace(&self.settings, &original)?;
        if fs::read(&self.settings).map_err(|e| e.to_string())? != original {
            return Err("software renderer restoration bytes differ".into());
        }
        fs::remove_file(&self.backup).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn software_options_preserve_every_other_byte_and_reject_duplicate_fields() {
        let input = b"; test\r\nDeviceID 5056\r\nOther 7\r\nAllowSWRenderer 0";
        assert_eq!(
            options(input).unwrap(),
            b"; test\r\nDeviceID 140\r\nOther 7\r\nAllowSWRenderer 1"
        );
        assert!(options(b"DeviceID 1\nDeviceID 2\nAllowSWRenderer 0\n").is_err());
        assert!(options(b"DeviceID 1\n").is_err());
        assert!(options(&[255]).is_err());
    }
    #[test]
    fn owned_profile_restores_original_bytes_and_recovers_interrupted_previous_run() {
        let root = std::env::temp_dir().join(format!(
            "cc-live-cpu-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let paths = Paths {
            here: root.join("here"),
            profile: root.join("profile"),
            ..Paths::default()
        };
        fs::create_dir_all(&paths.here).unwrap();
        let settings = paths
            .profile
            .join("AppData/Local/Firaxis Games/Sid Meier's Civilization VI/AppOptions.txt");
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        let original = b"DeviceID 5056\r\nAllowSWRenderer 0\r\nOther 9\r\n";
        fs::write(&settings, original).unwrap();
        fs::write(paths.here.join("civ6.lock"),serde_json::json!({"live_chain":true,"chain_pid":std::process::id(),"lock_token":"fixture"}).to_string()).unwrap();
        assert!(Profile::apply(&paths, "wrong").is_err());
        assert_eq!(fs::read(&settings).unwrap(), original);
        let interrupted = Profile::apply(&paths, "fixture").unwrap();
        assert_eq!(fs::read(&settings).unwrap(), options(original).unwrap());
        drop(interrupted); // simulate process termination; retain original.
        let mut recovery = Profile::recover(&paths, "fixture").unwrap().unwrap();
        recovery.restore().unwrap();
        drop(recovery);
        assert_eq!(fs::read(&settings).unwrap(), original);
        let mut recovered = Profile::apply(&paths, "fixture").unwrap();
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            assert!(
                fs::remove_file(paths.here.join("civ6.lock")).is_err(),
                "lease must not release before restoration"
            );
            let reader = OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&settings)
                .unwrap();
            assert!(recovered.restore().is_err());
            assert_eq!(fs::read(&settings).unwrap(), options(original).unwrap());
            assert_eq!(fs::read(&recovered.backup).unwrap(), original);
            drop(reader);
        }
        recovered.restore().unwrap();
        drop(recovered);
        assert_eq!(fs::read(&settings).unwrap(), original);
        assert!(!paths.here.join("software-renderer-original.bin").exists());
        let backup = paths.here.join("complete-original.bin");
        publish_backup(&backup, original).unwrap();
        assert!(publish_backup(&backup, b"must not replace complete original").is_err());
        assert_eq!(fs::read(backup).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }
}
