//! The scenario keeps its game alive until the observer records completed cleanup.
use crate::{Error, Result};
use serde_json::Value;
use std::{fs::File, io::Read, path::Path};

const MAX_OUTPUT_BYTES: u64 = 1024 * 1024;

pub fn read_completed_exit(path: &Path, game_pid: u32, game_created: u64) -> Result<Option<Value>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_OUTPUT_BYTES {
        return Err(Error::scenario(
            "native observer output exceeds finite limit",
        ));
    }
    // A writer can be publishing the last JSON line. Only newline-terminated
    // records are evidence; a partial final line never proves cleanup.
    let Some(end) = bytes.iter().rposition(|v| *v == b'\n') else {
        return Ok(None);
    };
    let complete = &bytes[..end];
    let last = complete
        .rsplit(|v| *v == b'\n')
        .find(|line| !line.iter().all(u8::is_ascii_whitespace));
    let Some(last) = last else { return Ok(None) };
    let record: Value = serde_json::from_slice(last)?;
    if record["event"] != "exit" {
        return Ok(None);
    }
    validate_exit(&record, game_pid, game_created)?;
    Ok(Some(record))
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
