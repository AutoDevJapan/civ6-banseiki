//! The scenario keeps its game alive until the observer records completed cleanup.
use crate::{Error, Result};
use serde_json::Value;
use std::{fs::File, io::Read, path::Path};

const MAX_OUTPUT_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub enum ExitRead {
    Pending,
    SharingViolation(String),
    Complete(Value),
}

impl From<Option<Value>> for ExitRead {
    fn from(value: Option<Value>) -> Self {
        match value {
            Some(record) => Self::Complete(record),
            None => Self::Pending,
        }
    }
}

pub fn read_completed_exit(path: &Path, game_pid: u32, game_created: u64) -> Result<Option<Value>> {
    match read_exit_status(path, game_pid, game_created)? {
        ExitRead::Pending => Ok(None),
        ExitRead::Complete(record) => Ok(Some(record)),
        ExitRead::SharingViolation(error) => Err(Error::new(crate::ErrorKind::Io, error)),
    }
}

fn sharing_violation(error: &std::io::Error) -> bool {
    cfg!(windows) && error.raw_os_error() == Some(32)
}

pub fn read_exit_status(path: &Path, game_pid: u32, game_created: u64) -> Result<ExitRead> {
    let mut bytes = Vec::new();
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if sharing_violation(&error) => {
            return Ok(ExitRead::SharingViolation(error.to_string()));
        }
        Err(error) => return Err(error.into()),
    };
    if let Err(error) = file.take(MAX_OUTPUT_BYTES + 1).read_to_end(&mut bytes) {
        return if sharing_violation(&error) {
            Ok(ExitRead::SharingViolation(error.to_string()))
        } else {
            Err(error.into())
        };
    }
    if bytes.len() as u64 > MAX_OUTPUT_BYTES {
        return Err(Error::scenario(
            "native observer output exceeds finite limit",
        ));
    }
    // A writer can be publishing the last JSON line. Only newline-terminated
    // records are evidence; a partial final line never proves cleanup.
    let Some(end) = bytes.iter().rposition(|v| *v == b'\n') else {
        return Ok(ExitRead::Pending);
    };
    let complete = &bytes[..end];
    let last = complete
        .rsplit(|v| *v == b'\n')
        .find(|line| !line.iter().all(u8::is_ascii_whitespace));
    let Some(last) = last else {
        return Ok(ExitRead::Pending);
    };
    let record: Value = serde_json::from_slice(last)?;
    if record["event"] != "exit" {
        return Ok(ExitRead::Pending);
    }
    validate_exit(&record, game_pid, game_created)?;
    Ok(ExitRead::Complete(record))
}

fn validate_exit(record: &Value, game_pid: u32, game_created: u64) -> Result<()> {
    let cleanup = &record["cleanup"];
    if record["pid"].as_u64() != Some(u64::from(game_pid))
        || record["created"].as_u64() != Some(game_created)
        || record["ready_reached"] != true
        || record["cleanup_complete"] != true
        || record["reason"] != "deadline"
        || record.get("error") != Some(&Value::Null)
        || cleanup["continue_pending_ok"] != true
        || cleanup["debug_active_process_stop_ok"] != true
        || cleanup["process_exited"] != false
        || !cleanup["errors"].as_array().is_some_and(Vec::is_empty)
    {
        return Err(Error::scenario(
            "native observer exit does not prove cleanup for the acknowledged game",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn good() -> Value {
        json!({"event":"exit","pid":7,"created":134360214105011701u64,
            "reason":"deadline","error":null,"ready_reached":true,
            "cleanup_complete":true,"cleanup":{"continue_pending_ok":true,
            "debug_active_process_stop_ok":true,"process_exited":false,"errors":[]}})
    }

    #[cfg(windows)]
    #[test]
    fn exclusive_output_read_is_pending_io_until_the_lock_is_released() -> Result<()> {
        use std::os::windows::fs::OpenOptionsExt;
        let root = tempfile::tempdir()?;
        let path = root.path().join("completed.jsonl");
        std::fs::write(&path, format!("{}\n", good()))?;
        let original = std::fs::read(&path)?;
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)?;
        assert!(matches!(
            read_exit_status(&path, 7, 134360214105011701)?,
            ExitRead::SharingViolation(_)
        ));
        // The compatibility reader must retain the I/O failure, not claim a
        // missing or completed record to callers that do not poll statuses.
        assert_eq!(
            read_completed_exit(&path, 7, 134360214105011701)
                .unwrap_err()
                .kind,
            crate::ErrorKind::Io
        );
        drop(lock);
        assert!(matches!(
            read_exit_status(&path, 7, 134360214105011701)?,
            ExitRead::Complete(_)
        ));
        assert_eq!(std::fs::read(&path)?, original);
        Ok(())
    }

    #[test]
    fn missing_or_invalid_output_is_not_a_sharing_wait() -> Result<()> {
        let root = tempfile::tempdir()?;
        let missing = root.path().join("missing.jsonl");
        assert_eq!(
            read_exit_status(&missing, 7, 134360214105011701)
                .unwrap_err()
                .kind,
            crate::ErrorKind::Io
        );
        std::fs::write(&missing, b"{invalid}\n")?;
        assert_eq!(
            read_exit_status(&missing, 7, 134360214105011701)
                .unwrap_err()
                .kind,
            crate::ErrorKind::Scenario
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn byte_range_lock_is_an_immediate_io_error_not_a_sharing_wait() -> Result<()> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::LockFile;
        let root = tempfile::tempdir()?;
        let path = root.path().join("byte-locked.jsonl");
        let original = format!("{}\n", good());
        std::fs::write(&path, &original)?;
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)?;
        // SAFETY: live owned file handle; the byte range belongs to this fixture.
        assert_ne!(
            unsafe { LockFile(lock.as_raw_handle(), 0, 0, original.len() as u32, 0) },
            0
        );
        let error = read_exit_status(&path, 7, 134360214105011701).unwrap_err();
        assert_eq!(error.kind, crate::ErrorKind::Io);
        assert!(error.message.contains("33"), "{error}");
        drop(lock);
        assert!(matches!(
            read_exit_status(&path, 7, 134360214105011701)?,
            ExitRead::Complete(_)
        ));
        assert_eq!(std::fs::read_to_string(&path)?, original);
        Ok(())
    }

    #[test]
    fn partial_exit_waits_and_completed_exit_requires_exact_creation_time() -> Result<()> {
        let mut file = tempfile::NamedTempFile::new()?;
        writeln!(file, "{{\"event\":\"ready\"}}")?;
        let exit = serde_json::to_vec(&good())?;
        file.write_all(&exit[..exit.len() / 2])?;
        file.flush()?;
        assert!(read_completed_exit(file.path(), 7, 134360214105011701)?.is_none());
        file.write_all(&exit[exit.len() / 2..])?;
        writeln!(file)?;
        file.flush()?;
        assert!(read_completed_exit(file.path(), 7, 134360214105011701)?.is_some());
        assert!(read_completed_exit(file.path(), 7, 134360214105011702).is_err());
        assert!(read_completed_exit(file.path(), 8, 134360214105011701).is_err());
        Ok(())
    }

    #[test]
    fn ready_or_incomplete_restore_never_proves_finished_cleanup() {
        for (pointer, replacement) in [
            ("/ready_reached", json!(false)),
            ("/cleanup_complete", json!(false)),
            ("/reason", json!("second_chance_exception")),
            ("/error", json!("read failed")),
            ("/cleanup/continue_pending_ok", json!(false)),
            ("/cleanup/debug_active_process_stop_ok", json!(false)),
            ("/cleanup/process_exited", json!(true)),
            ("/cleanup/errors", json!(["restore failed"])),
            ("/cleanup/errors", Value::Null),
        ] {
            let mut exit = good();
            *exit.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                validate_exit(&exit, 7, 134360214105011701).is_err(),
                "{pointer}"
            );
        }
        let mut missing = good();
        missing.as_object_mut().unwrap().remove("error");
        assert!(validate_exit(&missing, 7, 134360214105011701).is_err());
    }
}
