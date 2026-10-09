//! Opt-in, single-player diagnostic contract for the native Oath observer.
//! Scenario fields describe a request. The driver must still verify the
//! actual game mode and its fresh process ownership before invoking a DLL.
use crate::{Error, Result};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Install,
    Start,
    Snapshot,
    Stop,
}

#[derive(Clone, Debug)]
pub struct Install {
    pub dll: PathBuf,
    pub sha256: String,
    pub max_seconds: u32,
}

pub fn operation(action: &str) -> Result<Operation> {
    match action {
        "oath_trace_install" => Ok(Operation::Install),
        "oath_trace_start" => Ok(Operation::Start),
        "oath_trace_snapshot" => Ok(Operation::Snapshot),
        "oath_trace_stop" => Ok(Operation::Stop),
        _ => Err(Error::scenario("unknown oath observer action")),
    }
}

pub fn validate_mode(test_mode: bool, diagnostic: bool, multiplayer: bool) -> Result<()> {
    if !test_mode || !diagnostic || multiplayer {
        return Err(Error::scenario(
            "oath observer requires an opt-in single-player diagnostic",
        ));
    }
    Ok(())
}

pub fn validate_step(step: &Value) -> Result<Operation> {
    let object = step
        .as_object()
        .ok_or_else(|| Error::scenario("oath observer step must be an object"))?;
    let op = operation(step["action"].as_str().unwrap_or(""))?;
    if object.keys().any(|name| {
        name != "action"
            && name != "timeout"
            && !(op == Operation::Install
                && matches!(name.as_str(), "dll" | "dll_sha256" | "max_seconds"))
    }) {
        return Err(Error::scenario("unsupported oath observer field"));
    }
    if op == Operation::Install {
        install_request(step)?;
    }
    Ok(op)
}

pub fn install_request(step: &Value) -> Result<Install> {
    let dll = step["dll"]
        .as_str()
        .filter(|p| !p.is_empty())
        .ok_or_else(|| Error::scenario("oath observer DLL path required"))?;
    let digest = step["dll_sha256"]
        .as_str()
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| Error::scenario("oath observer complete DLL SHA256 required"))?;
    let seconds = step["max_seconds"]
        .as_u64()
        .filter(|n| (1..=300).contains(n))
        .ok_or_else(|| Error::scenario("oath observer max_seconds must be 1..300"))?;
    Ok(Install {
        dll: PathBuf::from(dll),
        sha256: digest.to_ascii_lowercase(),
        max_seconds: seconds as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn install() -> Value {
        json!({"action":"oath_trace_install", "dll":"owned-runtime.dll",
        "dll_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", "max_seconds":300})
    }
    #[test]
    fn only_explicit_test_diagnostic_single_player_mode_is_allowed() {
        for test in [false, true] {
            for diag in [false, true] {
                for mp in [false, true] {
                    assert_eq!(validate_mode(test, diag, mp).is_ok(), test && diag && !mp);
                }
            }
        }
    }
    #[test]
    fn each_operation_has_exact_fields_and_install_has_a_complete_digest() {
        assert_eq!(validate_step(&install()).unwrap(), Operation::Install);
        for action in ["oath_trace_start", "oath_trace_snapshot", "oath_trace_stop"] {
            validate_step(&json!({"action":action})).unwrap();
            assert!(validate_step(&json!({"action":action,"dll":"another.dll"})).is_err());
        }
        for field in ["dll", "dll_sha256", "max_seconds"] {
            let mut value = install();
            value.as_object_mut().unwrap().remove(field);
            assert!(validate_step(&value).is_err());
        }
        for digest in [
            "",
            "abcd",
            "g123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        ] {
            let mut value = install();
            value["dll_sha256"] = json!(digest);
            assert!(validate_step(&value).is_err());
        }
        for seconds in [json!(0), json!(301), json!(1.5), json!(-1)] {
            let mut value = install();
            value["max_seconds"] = seconds;
            assert!(validate_step(&value).is_err());
        }
        let mut value = install();
        value["quantity_accepted"] = json!(true);
        assert!(validate_step(&value).is_err());
    }
}
