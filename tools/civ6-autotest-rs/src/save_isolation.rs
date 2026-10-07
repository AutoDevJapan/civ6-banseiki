//! Temporarily give the minimal reliability run an empty autosave directory.
//! Original files are renamed intact, never overwritten or deleted.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};
#[derive(Serialize, Deserialize)]
struct State {
    original: std::path::PathBuf,
    archive: std::path::PathBuf,
    existed: bool,
    original_digest: Option<String>,
}
fn digest(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut h = Sha256::new();
    let mut entries = fs::read_dir(path)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        h.update(e.file_name().to_string_lossy().as_bytes());
        h.update([0]);
        if e.path().is_dir() {
            h.update(digest(&e.path())?.as_bytes());
        } else {
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
    let original = profile.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single/auto");
    let original = original
        .parent()
        .ok_or_else(|| Error::input("autosave parent missing"))?
        .canonicalize()?
        .join("auto");
    let archive = archive.canonicalize()?;
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
        },
    )?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    if existed {
        fs::rename(&original, &backup)?;
    }
    fs::create_dir(&original)?;
    Ok(())
}
pub fn restore(profile: &Path, state: &Path) -> Result<()> {
    let bytes = fs::read(state)?;
    let s: State = serde_json::from_slice(&bytes)?;
    let expected = profile
        .join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single")
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
    // A crash before the first rename leaves the original folder untouched.
    if s.existed && !backup.exists() {
        if !s.original.exists() || Some(digest(&s.original)?) != s.original_digest {
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
        fs::rename(&s.original, &test)?;
    }
    if s.existed {
        no_links(&backup)?;
        fs::rename(&backup, &s.original)?;
    }
    fs::remove_file(state)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
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
