//! Raw callee arguments only. Copies establish neither City lifetime nor quantity.
use crate::oath_wire::{u32_at, u64_at, Receipt};
use serde_json::{json, Value};

pub(crate) const HEADER: usize = 64;
pub(crate) const ROW: usize = 120;
pub(crate) const CAPACITY: usize = 1024;
pub(crate) const SIZE: usize = HEADER + ROW * CAPACITY;

pub(crate) fn validate(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != SIZE
        || u32_at(bytes, 0) != Some(0x4a41_4352)
        || u32_at(bytes, 4) != Some(1)
        || u32_at(bytes, 8).is_none_or(|n| n as usize > CAPACITY)
        || u32_at(bytes, 24).is_none_or(|n| n > 2)
        || u32_at(bytes, 52).is_none_or(|n| n > 1)
    {
        return Err("invalid callee trace header or size".into());
    }
    Ok(())
}

pub(crate) fn project(first: &[u8], second: &[u8], receipt: &Receipt) -> Result<Value, String> {
    validate(first)?;
    validate(second)?;
    let mut rows = Vec::new();
    let mut open = 0;
    let mut unstable = 0;
    let mut completed_rows = 0;
    let mut unwound_rows = 0;
    let count = u32_at(second, 8).unwrap() as usize;
    let first_count = u32_at(first, 8).unwrap() as usize;
    for index in 0..count {
        let at = HEADER + index * ROW;
        let row = &second[at..at + ROW];
        let stable = index < first_count && first[at..at + ROW] == *row;
        let state = u32_at(row, 0).unwrap();
        let slot = u32_at(row, 4).unwrap();
        if !stable {
            unstable += 1
        }
        if !stable || !matches!(state, 2 | 3) || slot >= 8 {
            open += 1
        }
        if stable && state == 2 {
            completed_rows += 1;
        }
        if stable && state == 3 {
            unwound_rows += 1;
        }
        let q = |offset| u64_at(row, offset).unwrap();
        let metadata = stable && matches!(state, 1..=3) && u32_at(row, 72) == Some(1);
        let hex = |offset| format!("0x{:x}", q(offset));
        rows.push(json!({"sequence":index+1,"state":if !stable { "unknown_unstable" } else { match state {2=>"completed",3=>"unwound",_=>"incomplete"} },
            "slot":slot,"family":match slot / 2 {0=>"military",1=>"naval",2=>"building",3=>"growth",_=>"unknown"},
            "call":if slot.is_multiple_of(2) {"apply"} else {"remove"},
            "thread_id":u32_at(row,8),"effect":hex(16),"actual_receiver_argument":hex(24),
            "raw_r8":hex(32),"caller_return":hex(40),"caller_rbp":hex(48),"caller_rdi":hex(56),
            "original_rax":if stable && state == 2 { json!(hex(64)) } else { Value::Null },
            "call_entry_metadata":if metadata { json!({"instance_id":u32_at(row,76),"instance":hex(80),"definition":hex(88),
                "city_wrapper":hex(96),"declared_percent":u32_at(row,104).unwrap() as i32,"owner":u32_at(row,108),"full_id":u32_at(row,112)}) } else { Value::Null },
            "quantity_accepted":false}));
    }
    let stopped = receipt.phase == 3 && u32_at(second, 24) == Some(2);
    let detached = u32_at(second, 52) == Some(1);
    let in_flight = u32_at(second, 16).unwrap();
    let dropped = u32_at(second, 12).unwrap();
    // The receipt and two remote copies are separate observations. Only
    // equal stopped counters can describe this snapshot as complete.
    let counters_match = receipt.trace_size as usize == SIZE
        && receipt.rows as usize == count
        && receipt.completed == u32_at(second, 20).unwrap()
        && receipt.dropped == dropped
        && receipt.in_flight == in_flight
        && receipt.filtered == 0
        && receipt.expired == u32_at(second, 48).unwrap()
        && completed_rows == receipt.completed
        && unwound_rows == u32_at(second, 28).unwrap()
        && completed_rows as usize + unwound_rows as usize == count;
    let complete = civ6_oath_runtime_rs::oath_record::observation_complete(
        stopped,
        in_flight,
        dropped,
        first == second,
        open,
    ) && detached
        && counters_match;
    Ok(
        json!({"capture":"callee","trace_version":1,"rows":rows,"row_count":count,
        "completed":u32_at(second,20),"unwound":u32_at(second,28),"in_flight":in_flight,"dropped":dropped,
        "stopped":stopped,"entry_slots_restored":detached,"complete":complete,"snapshot_consistent":complete,"unstable_rows":unstable,"receipt_counters_match":counters_match,
        "image_base":format!("0x{:x}",u64_at(second,32).unwrap()),"deadline_hit":u32_at(second,48)==Some(1),
        "quantity_accepted":false,"accepted":false,
        "limitations":["complete_is_point_in_time_snapshot_consistency","point_in_time_counts_do_not_authorize_gateway_retirement","city_identity_and_lifetime_not_established","no_quantity_adjustment"]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Vec<u8> {
        let mut bytes = vec![0; SIZE];
        for (at, value) in [
            (0, 0x4a41_4352u32),
            (4, 1),
            (8, 1),
            (20, 1),
            (24, 2),
            (52, 1),
            (64, 2),
        ] {
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[88..96].copy_from_slice(&0x12340000u64.to_le_bytes());
        bytes
    }
    fn receipt() -> Receipt {
        Receipt {
            phase: 3,
            trace_address: 8,
            trace_size: SIZE as u32,
            max_seconds: 30,
            rows: 1,
            completed: 1,
            dropped: 0,
            filtered: 0,
            in_flight: 0,
            expired: 0,
        }
    }
    #[test]
    fn receiver_copy_never_accepts_quantity_and_losses_or_unstable_rows_are_incomplete() {
        let bytes = sample();
        let r = receipt();
        let value = project(&bytes, &bytes, &r).unwrap();
        assert_eq!(value["complete"], true);
        assert_eq!(value["rows"][0]["actual_receiver_argument"], "0x12340000");
        assert_eq!(value["quantity_accepted"], false);
        for (at, value) in [(12, 1u32), (16, 1), (52, 0), (64, 1), (68, 8)] {
            let mut changed = bytes.clone();
            changed[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert_eq!(project(&changed, &changed, &r).unwrap()["complete"], false);
        }
        let mut changed = bytes.clone();
        changed[88] ^= 1;
        assert_eq!(project(&bytes, &changed, &r).unwrap()["complete"], false);
        assert!(validate(&bytes[..bytes.len() - 1]).is_err());
        let mut changed = bytes;
        changed[8..12].copy_from_slice(&1025u32.to_le_bytes());
        assert!(validate(&changed).is_err());
    }
    #[cfg(all(windows, target_arch = "x86_64"))]
    #[test]
    fn layout_matches_compiled_atomic_runtime() {
        assert_eq!(
            civ6_oath_runtime_rs::recipient_trace::RecipientTrace::wire_layout(),
            (HEADER, ROW, SIZE)
        );
    }

    #[test]
    fn stopped_receipt_must_describe_the_same_terminal_rows() {
        let bytes = sample();
        for mutate in [
            |r: &mut Receipt| r.rows = 0,
            |r: &mut Receipt| r.completed = 0,
            |r: &mut Receipt| r.dropped = 1,
            |r: &mut Receipt| r.in_flight = 1,
            |r: &mut Receipt| r.filtered = 1,
            |r: &mut Receipt| r.expired = 1,
            |r: &mut Receipt| r.trace_size -= 8,
        ] {
            let mut r = receipt();
            mutate(&mut r);
            assert_eq!(project(&bytes, &bytes, &r).unwrap()["complete"], false);
        }
        let mut bytes = bytes;
        bytes[20..24].copy_from_slice(&0u32.to_le_bytes());
        let mut r = receipt();
        r.completed = 0;
        assert_eq!(project(&bytes, &bytes, &r).unwrap()["complete"], false);
        bytes[64..68].copy_from_slice(&3u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&1u32.to_le_bytes());
        let value = project(&bytes, &bytes, &r).unwrap();
        assert_eq!(value["complete"], true);
        assert!(value["rows"][0]["original_rax"].is_null());
        assert!(value["rows"][0]["call_entry_metadata"].is_null());
    }
}
