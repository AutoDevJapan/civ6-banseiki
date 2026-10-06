//! Shared finite controller/driver command. No execution or address parameters.
use crate::ai_once::Subject;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    pub stop: bool,
    pub subject: Subject,
}
impl Command {
    pub fn parse(value: &Value, pid: u32, created: u64) -> Result<Self, String> {
        let object = value.as_object().ok_or("AI diagnostic must be an object")?;
        let stop = match value["action"].as_str() {
            Some("start_native_ai_observation") => false,
            Some("stop_native_ai_observation") => true,
            _ => return Err("unknown AI diagnostic action".into()),
        };
        let keys = [
            "action",
            "owner",
            "generation",
            "index",
            "cost",
            "paid",
            "spent",
            "title",
            "timeout",
        ];
        if object.keys().any(|k| !keys.contains(&k.as_str())) {
            return Err("AI diagnostic has unknown fields (execution is unavailable)".into());
        }
        let number = |key: &str| {
            value[key]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| format!("AI diagnostic requires integer {key}"))
        };
        let subject = Subject {
            pid,
            created,
            owner: number("owner")?,
            generation: number("generation")?,
            index: number("index")?,
            cost: number("cost")?,
            paid: number("paid")?,
            spent: number("spent")?,
            title: number("title")?,
        };
        if !subject.valid() {
            return Err("AI diagnostic requires a partial TITLE subject, never APPOINT".into());
        }
        Ok(Self { stop, subject })
    }
}

/// Reserve before sending. Failed/unknown starts cannot be repeated, and a
/// failed stop cannot be resent. Process exit is the remaining cleanup route.
#[derive(Default)]
pub struct Session {
    subject: Option<Subject>,
    stopped: bool,
}
pub fn validate_receipt(value: &Value, command: Command) -> Result<(), String> {
    let s = command.subject;
    let pairs = [
        ("pid", u64::from(s.pid)),
        ("created_filetime", s.created),
        ("owner", u64::from(s.owner)),
        ("index", u64::from(s.index)),
        ("requested_generation", u64::from(s.generation)),
        ("generation", u64::from(s.generation)),
        ("cost", u64::from(s.cost)),
        ("paid", u64::from(s.paid)),
        ("spent", u64::from(s.spent)),
        ("title", u64::from(s.title)),
        ("status", 0),
        ("query_status", 0),
        (
            "execution_blockers",
            u64::from(crate::ai_once::EXECUTION_BLOCKERS),
        ),
    ];
    if value["kind"] != "ai_observation"
        || value.get("error").is_some()
        || pairs.into_iter().any(|(k, v)| value[k].as_u64() != Some(v))
        || value["stop"].as_bool() != Some(command.stop)
        || value["operation_proven"] != false
        || value["runtime_retained"] != true
        || value["producer_drained"] != false
        || value["callback_exit_proven"] != false
        || value["sampling_stop_only"].as_bool() != Some(command.stop)
        || value["dispatched"].as_u64() != Some(0)
        || value["verified"].as_u64() != Some(0)
        || value["record_present"].as_u64() != Some(0)
        || !matches!(value["attempt_state"].as_u64(), Some(5))
            && value["attempt_state"].as_u64() != Some(if command.stop { 6 } else { 0 })
        || value["raw_rows"].as_array().is_none_or(|rows| {
            rows.len() > 128 || value["observations"].as_u64() != Some(rows.len() as u64)
        })
    {
        return Err(format!(
            "AI diagnostic refused, unknown, or mismatched receipt: {value}"
        ));
    }
    Ok(())
}
impl Session {
    pub fn reserve(&mut self, command: Command) -> Result<(), String> {
        if command.stop {
            if self.subject != Some(command.subject) || self.stopped {
                return Err("AI stop identity differs or was already attempted".into());
            }
            self.stopped = true;
        } else {
            if self.subject.is_some() {
                return Err("AI registration already attempted; no retry".into());
            }
            self.subject = Some(command.subject);
        }
        Ok(())
    }
}
/// Driver step result, distinct from successful diagnostic transport/cleanup.
pub fn validate_matched_receipt(value: &Value, command: Command) -> Result<(), String> {
    validate_receipt(value, command)?;
    if value["attempt_state"].as_u64() == Some(crate::ai_once::State::Mismatch as u64)
        || value["reject_reason"].as_u64() != Some(0)
    {
        return Err(format!(
            "AI subject receipt mismatched; raw capture retained: {value}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Value {
        serde_json::json!({"action":"start_native_ai_observation","owner":2,
        "generation":4,"index":7,"cost":3,"paid":1,"spent":9,"title":8})
    }
    #[test]
    fn ai_diagnostic_requires_explicit_partial_receipt_and_rejects_execution() {
        let v = request();
        assert!(Command::parse(&v, 1, 2).is_ok());
        for key in [
            "owner",
            "generation",
            "index",
            "cost",
            "paid",
            "spent",
            "title",
        ] {
            let mut bad = v.clone();
            bad[key] = Value::Null;
            assert!(Command::parse(&bad, 1, 2).is_err());
        }
        for (key, value) in [
            ("paid", 0),
            ("paid", 3),
            ("execution_mode", 1),
            ("drained", 1),
        ] {
            let mut bad = v.clone();
            bad[key] = value.into();
            assert!(Command::parse(&bad, 1, 2).is_err());
        }
    }
    #[test]
    fn ai_diagnostic_unknown_start_is_not_retried_and_stop_matches_all_fields() {
        let start = Command::parse(&request(), 1, 2).unwrap();
        let mut session = Session::default();
        session.reserve(start).unwrap();
        assert!(session.reserve(start).is_err());
        let mut stop = Command {
            stop: true,
            ..start
        };
        stop.subject.generation += 1;
        assert!(session.reserve(stop).is_err());
        stop.subject = start.subject;
        session.reserve(stop).unwrap();
        assert!(session.reserve(stop).is_err());
        assert!(session.reserve(start).is_err());
    }
    #[test]
    fn ai_diagnostic_receipt_checks_identity_status_and_keeps_lifecycle_blocked() {
        let command = Command::parse(&request(), 1, 2).unwrap();
        let good = serde_json::json!({"kind":"ai_observation","stop":false,"pid":1,"created_filetime":2,
            "owner":2,"index":7,"requested_generation":4,"generation":4,"cost":3,"paid":1,"spent":9,"title":8,
            "status":0,"query_status":0,"execution_blockers":7,"operation_proven":false,"runtime_retained":true,
            "producer_drained":false,"callback_exit_proven":false,"sampling_stop_only":false,
            "dispatched":0,"verified":0,"record_present":0,"attempt_state":0,"raw_rows":[],"observations":0,"reject_reason":0});
        validate_receipt(&good, command).unwrap();
        validate_matched_receipt(&good, command).unwrap();
        let mut mismatch = good.clone();
        mismatch["attempt_state"] = 5.into();
        mismatch["reject_reason"] = 3.into();
        validate_receipt(&mismatch, command).unwrap();
        assert!(validate_matched_receipt(&mismatch, command).is_err());
        for key in [
            "pid",
            "created_filetime",
            "owner",
            "index",
            "requested_generation",
            "generation",
            "cost",
            "paid",
            "spent",
            "title",
            "status",
            "query_status",
            "execution_blockers",
        ] {
            let mut changed = good.clone();
            changed[key] = serde_json::json!(99);
            assert!(validate_receipt(&changed, command).is_err(), "{key}");
        }
        for key in ["stop", "operation_proven", "producer_drained"] {
            let mut changed = good.clone();
            changed[key] = serde_json::json!(true);
            assert!(validate_receipt(&changed, command).is_err(), "{key}");
        }
    }
}
