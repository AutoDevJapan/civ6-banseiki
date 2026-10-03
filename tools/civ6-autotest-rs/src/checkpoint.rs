//! Append-only Civ VI saves with one atomic pointer per logical checkpoint.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const DIRECTORY: &str = ".jac-autotest-checkpoints";
const VERSION: u32 = 1;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    logical: String,
    physical: String,
}

fn basename(name: &str) -> Result<()> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 80
        || !bytes[0].is_ascii_alphanumeric()
        || !bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        return Err(Error::scenario(format!(
            "Invalid checkpoint basename: {name:?}"
        )));
    }
    Ok(())
}

pub fn manifest_path(saves: &Path, logical: &str) -> Result<PathBuf> {
    basename(logical)?;
    Ok(saves.join(DIRECTORY).join(format!("{logical}.json")))
}

fn save_path(saves: &Path, physical: &str) -> Result<PathBuf> {
    basename(physical)?;
    Ok(saves.join(format!("{physical}.Civ6Save")))
}

fn require_save(saves: &Path, physical: &str) -> Result<PathBuf> {
    let path = save_path(saves, physical)?;
    let metadata = fs::symlink_metadata(&path).map_err(|e| {
        Error::scenario(format!(
            "Checkpoint save missing at {}: {e}",
            path.display()
        ))
    })?;
    if !metadata.file_type().is_file() || metadata.len() == 0 {
        return Err(Error::scenario(format!(
            "Checkpoint save is empty or not a regular file: {}",
            path.display()
        )));
    }
    Ok(path)
}

/// Resolve the last published save. An exact-name save can seed a new checkpoint.
/// A present but invalid manifest never falls back to that seed.
pub fn resolve(saves: &Path, logical: &str) -> Result<String> {
    let path = manifest_path(saves, logical)?;
    let bytes = match fs::symlink_metadata(&path) {
        Ok(meta) => {
            if !meta.file_type().is_file() || meta.len() > 4096 {
                return Err(Error::scenario(format!(
                    "Invalid checkpoint manifest at {}",
                    path.display()
                )));
            }
            fs::read(&path).map_err(|e| {
                Error::scenario(format!(
                    "Cannot read checkpoint manifest {}: {e}",
                    path.display()
                ))
            })?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            require_save(saves, logical)?;
            return Ok(logical.to_owned());
        }
        Err(e) => {
            return Err(Error::scenario(format!(
                "Cannot read checkpoint manifest {}: {e}",
                path.display()
            )))
        }
    };
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| {
        Error::scenario(format!(
            "Invalid checkpoint manifest {}: {e}",
            path.display()
        ))
    })?;
    if manifest.version != VERSION || manifest.logical != logical {
        return Err(Error::scenario(format!(
            "Invalid checkpoint manifest version or logical name at {}",
            path.display()
        )));
    }
    require_save(saves, &manifest.physical)?;
    Ok(manifest.physical)
}

/// Publish only after the engine's SaveComplete and nonempty-file checks pass.
pub fn publish(saves: &Path, logical: &str, physical: &str) -> Result<()> {
    publish_with(saves, logical, physical, atomic_replace)
}

fn publish_with(
    saves: &Path,
    logical: &str,
    physical: &str,
    replace: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> Result<()> {
    let manifest_path = manifest_path(saves, logical)?;
    require_save(saves, physical)?;
    let directory = manifest_path.parent().unwrap();
    fs::create_dir_all(directory).map_err(|e| {
        Error::scenario(format!(
            "Cannot create checkpoint directory {}: {e}",
            directory.display()
        ))
    })?;
    let temp = directory.join(format!("{logical}.{}.tmp", uuid::Uuid::new_v4().simple()));
    let manifest = Manifest {
        version: VERSION,
        logical: logical.to_owned(),
        physical: physical.to_owned(),
    };
    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        serde_json::to_writer(&mut file, &manifest)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        replace(&temp, &manifest_path).map_err(|e| {
            Error::scenario(format!(
                "Cannot publish checkpoint manifest {}: {e}",
                manifest_path.display()
            ))
        })?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    write_result
}

#[cfg(windows)]
fn atomic_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>()
    };
    let source = wide(source);
    let target = wide(target);
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_atomic_replace_keeps_the_previous_pointer() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a.Civ6Save"), b"a").unwrap();
        fs::write(root.path().join("b.Civ6Save"), b"b").unwrap();
        publish(root.path(), "checkpoint", "a").unwrap();
        let before = fs::read(manifest_path(root.path(), "checkpoint").unwrap()).unwrap();
        let error = publish_with(root.path(), "checkpoint", "b", |_, _| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "blocked",
            ))
        })
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("Cannot publish checkpoint manifest"));
        assert_eq!(
            fs::read(manifest_path(root.path(), "checkpoint").unwrap()).unwrap(),
            before
        );
        assert_eq!(resolve(root.path(), "checkpoint").unwrap(), "a");
        assert_eq!(fs::read(root.path().join("b.Civ6Save")).unwrap(), b"b");
    }
}
