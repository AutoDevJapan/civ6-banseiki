//! Receipt of the deferred Install/Start held at the fixed loader return.
//! It is a diagnostic code/phase receipt, not City lifetime or quantity proof.
#![allow(dead_code)]
use crate::oath_wire::{u32_at, u64_at};
use serde_json::{json, Value};

pub(crate) const SIZE: usize = 96;
pub(crate) const EXPORTS: [&str; 4] = [
    "Civ6OathColdPrepare",
    "Civ6OathColdReadReceipt",
    "Civ6OathLifecycleStop",
    "Civ6OathLifecyclePoll",
];

pub(crate) fn ready(first: &[u8], second: &[u8], pid: u32, created: u64) -> Result<Value, String> {
    if first.len() != SIZE || second.len() != SIZE || first != second {
        return Err("cold lifecycle receipt copies differ".into());
    }
    // The runtime publishes the state only after all fields have been written.
    if u32_at(second, 0) != Some(0x434f3134)
        || u32_at(second, 4) != Some(1)
        || u32_at(second, 8) != Some(2)
        || u32_at(second, 12) != Some(0)
        || u32_at(second, 16) != Some(pid)
        || u32_at(second, 20) == Some(0)
        || u64_at(second, 24) != Some(created)
        || u64_at(second, 32) == Some(0)
        || u64_at(second, 40) == Some(0)
        || u64_at(second, 48) != Some(0x24b532)
        || u64_at(second, 80) != Some(0)
        || u64_at(second, 88) != Some(0)
    {
        return Err("cold lifecycle Install/Start receipt is not ready for this game".into());
    }
    let prepared = u64_at(second, 56).unwrap();
    let ready = u64_at(second, 64).unwrap();
    let due = u64_at(second, 72).unwrap();
    if prepared == 0 || ready < prepared || due <= ready || due - prepared > 300_000 {
        return Err("cold lifecycle receipt has an invalid absolute deadline".into());
    }
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    Ok(
        json!({"format":"oath-cold-v1","pid":pid,"created_filetime":created,
        "thread_id":u32_at(second,20),"exe_base":u64_at(second,32),"core_base":u64_at(second,40),
        "caller_return_rva":u64_at(second,48),"prepared_tick":prepared,"ready_tick":ready,
        "deadline_tick":due,"installed_before_target_context":true,"raw_first":hex(first),
        "raw_second":hex(second),"first_root_observed":false,"native_lifetime_proved":false,
        "quantity_accepted":false,"product_accepted":false}),
    )
}

pub(crate) fn attach(trace: &mut Value, receipt: Option<&Value>) {
    let Some(receipt) = receipt else {
        return;
    };
    let caller = receipt["exe_base"]
        .as_u64()
        .and_then(|base| base.checked_add(0x24b5c5));
    let observed = trace["records"].as_array().is_some_and(|rows| {
        rows.iter().any(|row| {
            row["kind"] == "context_root"
                && row["state"] == 2
                && row["root_or_this"] != "0x0"
                && row["thread_id"] == receipt["thread_id"]
                && row["caller"]
                    .as_str()
                    .and_then(|v| v.strip_prefix("0x"))
                    .and_then(|v| u64::from_str_radix(v, 16).ok())
                    == caller
        })
    });
    trace["cold_receipt"] = receipt.clone();
    trace["cold_loader_context_root_observed"] = json!(observed);
    // A scoped return-site observation is not the first call on every thread.
}

#[cfg(test)]
mod tests {
    use super::*;
    fn receipt() -> Vec<u8> {
        let mut b = vec![0; SIZE];
        for (at, v) in [(0, 0x434f3134u32), (4, 1), (8, 2), (16, 7), (20, 9)] {
            b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (at, v) in [
            (24, 123u64),
            (32, 0x140000000),
            (40, 0x180000000),
            (48, 0x24b532),
            (56, 100),
            (64, 110),
            (72, 300100),
        ] {
            b[at..at + 8].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
    #[test]
    fn cold_ready_requires_complete_same_epoch_receipt_and_keeps_nonacceptance() {
        let good = receipt();
        let value = ready(&good, &good, 7, 123).unwrap();
        assert_eq!(value["installed_before_target_context"], true);
        assert_eq!(value["first_root_observed"], false);
        assert_eq!(value["quantity_accepted"], false);
        for at in [0, 4, 8, 12, 16, 24, 48, 80, 88] {
            let mut bad = good.clone();
            bad[at] ^= 0xff;
            assert!(ready(&bad, &bad, 7, 123).is_err(), "offset {at}");
            assert!(ready(&good, &bad, 7, 123).is_err());
        }
        for (at, len) in [(20, 4), (32, 8), (40, 8), (56, 8), (64, 8), (72, 8)] {
            let mut bad = good.clone();
            bad[at..at + len].fill(0);
            assert!(ready(&bad, &bad, 7, 123).is_err(), "missing offset {at}");
        }
        assert!(ready(&good[..SIZE - 1], &good, 7, 123).is_err());
    }
    #[test]
    fn ready_receipt_needs_the_real_nonnull_completed_root_on_the_held_loader_thread() {
        let b = receipt();
        let ready = ready(&b, &b, 7, 123).unwrap();
        let row = json!({"kind":"context_root","state":2,"root_or_this":"0x1234",
            "thread_id":9,"caller":"0x14024b5c5"});
        let mut good =
            json!({"records":[row],"first_root_observed":false,"quantity_accepted":false});
        attach(&mut good, Some(&ready));
        assert_eq!(good["cold_loader_context_root_observed"], true);
        for (field, value) in [
            ("kind", json!("application_teardown")),
            ("state", json!(1)),
            ("root_or_this", json!("0x0")),
            ("thread_id", json!(10)),
            ("caller", json!("0x14024b1ec")),
        ] {
            let mut bad = good.clone();
            bad["records"][0][field] = value;
            attach(&mut bad, Some(&ready));
            assert_eq!(bad["cold_loader_context_root_observed"], false, "{field}");
        }
        assert_eq!(good["first_root_observed"], false);
        assert_eq!(good["quantity_accepted"], false);
    }
}
