//! Read the chain's lease without acquiring, replacing or changing its lock.
use crate::{Error, ErrorKind, Result};
use serde_json::Value;
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const LEASE_SECONDS: f64 = 1800.0;
fn refused(message: &str) -> Error {
    Error::new(ErrorKind::LiveLease, message)
}

pub fn record_budget(record: &Value, chain: u64, now: f64) -> Result<Option<f64>> {
    if chain == 0 || record["chain_pid"].as_u64() != Some(chain) || record["live_chain"] != true {
        return Err(refused(
            "live chain ownership changed; stopping before further game operations",
        ));
    }
    if record["competitor_since"].is_null() {
        return Ok(None);
    }
    let timestamp = |key: &str| {
        record[key]
            .as_f64()
            .filter(|v| v.is_finite() && *v >= 0.0 && *v <= now)
            .ok_or_else(|| refused("invalid live chain lease timestamp"))
    };
    let since = timestamp("competitor_since")?.max(timestamp("acquired_at")?);
    let remaining = LEASE_SECONDS - (now - since);
    if remaining <= 0.0 {
        return Err(refused("CIV6_LEASE_YIELD: a waiting run has reached the 30-minute lease; close this run and re-queue"));
    }
    Ok(Some(remaining))
}

pub fn environment_budget() -> Result<Option<f64>> {
    let Some(chain) = std::env::var_os(crate::startup::CHAIN_PID_ENV) else {
        return Ok(None);
    };
    let chain = chain
        .to_str()
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or_else(|| refused("invalid live chain identity"))?;
    let lock = PathBuf::from(
        std::env::var_os(crate::startup::LOCK_PATH_ENV)
            .ok_or_else(|| refused("live chain lock path missing"))?,
    );
    if lock.file_name().and_then(|s| s.to_str()) != Some("civ6.lock") {
        return Err(refused("unexpected live chain lock filename"));
    }
    let bytes = std::fs::read(lock).map_err(|_| refused("live chain lock unavailable"))?;
    let record = serde_json::from_slice(&bytes)
        .map_err(|_| refused("live chain lock changed or is invalid"))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| refused("live chain clock unavailable"))?
        .as_secs_f64();
    record_budget(&record, chain, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn record() -> Value {
        json!({"chain_pid":7,"live_chain":true,"acquired_at":100,"competitor_since":200})
    }
    #[test]
    fn waiter_limits_the_budget_and_yields_at_exact_boundary() {
        assert_eq!(record_budget(&record(), 7, 1999.0).unwrap(), Some(1.0));
        assert_eq!(
            record_budget(&record(), 7, 2000.0).unwrap_err().kind,
            ErrorKind::LiveLease
        );
        assert_eq!(
            record_budget(&record(), 7, 2100.0).unwrap_err().kind,
            ErrorKind::LiveLease
        );
    }
    #[test]
    fn no_waiter_does_not_limit_the_current_run() {
        let mut r = record();
        r.as_object_mut().unwrap().remove("competitor_since");
        assert_eq!(record_budget(&r, 7, 10000.0).unwrap(), None);
    }
    #[test]
    fn new_holder_receives_its_own_lease_even_with_older_waiters() {
        let mut r = record();
        r["acquired_at"] = json!(1000);
        assert_eq!(record_budget(&r, 7, 2000.0).unwrap(), Some(800.0));
    }
    #[test]
    fn ownership_and_invalid_timestamps_stop_operations() {
        assert!(record_budget(&record(), 8, 2000.0).is_err());
        for value in [json!(null), json!("100"), json!(-1), json!(3000)] {
            let mut r = record();
            r["acquired_at"] = value;
            assert!(record_budget(&r, 7, 2000.0).is_err());
        }
    }
}
