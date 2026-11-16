//! Only pairs the fixed cleanup frame's Wait; never authorizes native drainage.
use super::{number, positive};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) fn observations(rows: &[Value]) -> Result<Value, String> {
    let mut window = 0u64;
    let mut entries = BTreeMap::new();
    let mut calls = BTreeMap::new();
    let mut pairs = Vec::new();
    let mut unpaired = Vec::new();
    let mut raw = Vec::new();
    for row in rows {
        if row["type"] == "phase_gap" {
            window += 1;
        }
        if row["type"] != "instruction_completed" || row["phase"] != "native-shutdown" {
            continue;
        }
        let key = (
            window,
            positive(row, "tid")?,
            positive(row, "thread_lifetime")?,
            number(row, "rsp")?,
        );
        raw.push(row.clone());
        match row["site"].as_str() {
            Some("manager_cleanup_entry") => {
                // The pinned prologue pushes RSI then allocates 0x30 bytes.
                let wait_rsp = key.3.checked_sub(0x38).ok_or("manager stack underflow")?;
                entries.insert((key.0, key.1, key.2, wait_rsp), row.clone());
            }
            Some("manager_wait_call") if calls.insert(key, row.clone()).is_some() => {
                return Err("duplicate Wait call in one cleanup frame".into());
            }
            Some("manager_wait_return") => {
                let call = calls.remove(&key);
                let entry = entries.remove(&key);
                if let (Some(call), Some(entry)) = (&call, &entry) {
                    let manager = positive(entry, "rcx")?;
                    let set = manager.checked_add(0x400).ok_or("manager set overflow")?;
                    if number(call, "rbx")? != manager
                        || number(row, "rbx")? != manager
                        || number(call, "rcx")? != set
                    {
                        return Err("cleanup Wait manager/set differs within frame".into());
                    }
                    pairs.push(json!({"window":window,"tid":key.1,"thread_lifetime":key.2,
                        "manager_candidate":manager,"set_address":set,"entry_sequence":entry["sequence"],
                        "call_sequence":call["sequence"],"return_sequence":row["sequence"],
                        "return_rax":row["rax"],"list_before":entry["shutdown"]["list_slot"],
                        "list_after_wait":row["shutdown"]["list_slot"],
                        "wait_return_observed":true,"cleanup_return_observed":false}));
                } else {
                    unpaired.push(json!({"sequence":row["sequence"],"entry_present":entry.is_some(),"call_present":call.is_some()}));
                }
            }
            _ => {}
        }
    }
    Ok(
        json!({"wait_pairs":pairs,"unpaired_returns":unpaired,"pending_calls":calls.len(),
        "raw_completed_sites":raw,"all_producer_domains_covered":false,"pending_publication_drained":false,
        "same_set_wait_proven":false,"native_quiescence_proven":false,"runtime_activation_allowed":false,
        "scope":"fixed cleanup-frame candidate identities; missing sites are unobserved"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<Value> {
        [
            "manager_cleanup_entry",
            "manager_wait_call",
            "manager_wait_return",
        ]
        .iter()
        .enumerate()
        .map(|(i, site)| {
            json!({"type":"instruction_completed","phase":"native-shutdown","site":site,
                "sequence":i+1,"tid":12,"thread_lifetime":4,"rsp":if i==0{0x1000}else{0xfc8},
                "rcx":if i==1{0x2400}else{0x2000},"rbx":0x2000,"rax":7})
        })
        .collect()
    }
    #[test]
    fn wait_entry_is_not_wait_return_or_global_drainage() {
        let rows = fixture();
        let before = observations(&rows[..2]).unwrap();
        assert_eq!(before["pending_calls"], 1);
        assert_eq!(before["wait_pairs"].as_array().unwrap().len(), 0);
        let report = observations(&rows).unwrap();
        assert_eq!(report["wait_pairs"][0]["set_address"], 0x2400);
        assert_eq!(report["wait_pairs"][0]["return_rax"], 7);
        for key in [
            "same_set_wait_proven",
            "native_quiescence_proven",
            "runtime_activation_allowed",
        ] {
            assert_eq!(report[key], false);
        }
    }
    #[test]
    fn mismatched_set_is_rejected_and_lifetimes_or_windows_do_not_pair() {
        let mut rows = fixture();
        rows[1]["rcx"] = json!(0x2500);
        assert!(observations(&rows).unwrap_err().contains("manager/set"));
        let mut rows = fixture();
        rows[2]["thread_lifetime"] = json!(5);
        assert_eq!(
            observations(&rows).unwrap()["wait_pairs"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        let mut rows = fixture();
        rows.insert(2, json!({"type":"phase_gap"}));
        assert_eq!(
            observations(&rows).unwrap()["wait_pairs"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            observations(&[]).unwrap()["native_quiescence_proven"],
            false
        );
    }
}
