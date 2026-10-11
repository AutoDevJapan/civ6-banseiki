//! Raw callee arguments only. Copies establish neither City lifetime nor quantity.
// The controller consumes this module on Win64; other hosts run its parser tests.
#![allow(dead_code)]
use crate::oath_wire::{u32_at, u64_at, Receipt};
use serde_json::{json, Value};

pub(crate) const HEADER: usize = 64;
pub(crate) const ROW: usize = 4320;
pub(crate) const CAPACITY: usize = 1024;
pub(crate) const SIZE: usize = HEADER + ROW * CAPACITY;
pub(crate) const ROW_V5: usize = ROW + 96;
pub(crate) const SIZE_V5: usize = HEADER + ROW_V5 * CAPACITY;

fn row_size(bytes: &[u8]) -> Result<usize, String> {
    match (u32_at(bytes, 4), bytes.len()) {
        (Some(4), SIZE) => Ok(ROW),
        (Some(5), SIZE_V5) => Ok(ROW_V5),
        _ => Err("callee trace version and size differ".into()),
    }
}

pub(crate) fn validate(bytes: &[u8]) -> Result<(), String> {
    row_size(bytes)?;
    if u32_at(bytes, 0) != Some(0x4a41_4352)
        || u32_at(bytes, 8).is_none_or(|n| n as usize > CAPACITY)
        || u32_at(bytes, 24).is_none_or(|n| n > 2)
        || u32_at(bytes, 52).is_none_or(|n| n > 1)
    {
        return Err("invalid callee trace header or size".into());
    }
    Ok(())
}

fn helper_boundary(row: &[u8], at: usize, thread: u32) -> Result<Value, String> {
    let flags = u32_at(row, at).unwrap();
    if flags & !7 != 0 || u32_at(row, at + 4) != Some(0) {
        return Err("invalid helper boundary flags or reserved field".into());
    }
    let sample = |offset: usize, valid: bool| {
        json!({"read_complete":valid,"helper":format!("0x{:x}",u64_at(row,offset).unwrap()),
            "owner_thread":u32_at(row,offset+8),
            "recursion":u32_at(row,offset+12).unwrap() as i32})
    };
    let first = sample(at + 16, flags & 1 != 0);
    let second = sample(at + 32, flags & 2 != 0);
    let stable = flags == 7 && first == second;
    let helper = u64_at(row, at + 32).unwrap();
    let recursion = u32_at(row, at + 44).unwrap() as i32;
    let owned = if stable && helper != 0 && helper.is_multiple_of(8) && recursion >= 0 {
        json!(thread != 0 && recursion > 0 && u32_at(row, at + 40) == Some(thread))
    } else {
        Value::Null
    };
    Ok(
        json!({"raw_flags":flags,"tick":u64_at(row,at+8),"first":first,"second":second,
        "copies_equal":stable,"helper_owned_at_boundary":owned,
        "continuous_holding_proved":false,"native_lifetime_proved":false,"quantity_accepted":false}),
    )
}

fn production(row: &[u8], at: usize) -> Option<Value> {
    let length = u32_at(row, at + 8)? as usize;
    let array = u64_at(row, at)?;
    if u32_at(row, at + 12)? != 1 || !(1..=512).contains(&length) || array == 0 || array & 3 != 0 {
        return None;
    }
    let coefficients: Vec<i32> = (0..length)
        .map(|i| u32_at(row, at + 16 + i * 4).unwrap() as i32)
        .collect();
    Some(
        json!({"array_address":format!("0x{array:x}"),"length":length,"coefficients_q8":coefficients}),
    )
}

pub(crate) fn project(first: &[u8], second: &[u8], receipt: &Receipt) -> Result<Value, String> {
    validate(first)?;
    validate(second)?;
    let width = row_size(second)?;
    let version = u32_at(second, 4).unwrap();
    if first.len() != second.len() || u32_at(first, 4) != Some(version) {
        return Err("callee snapshots describe different wire versions".into());
    }
    let mut rows = Vec::new();
    let mut open = 0;
    let mut unstable = 0;
    let mut completed_rows = 0;
    let mut unwound_rows = 0;
    let count = u32_at(second, 8).unwrap() as usize;
    let first_count = u32_at(first, 8).unwrap() as usize;
    for index in 0..count {
        let at = HEADER + index * width;
        let row = &second[at..at + width];
        let stable = index < first_count && first[at..at + width] == *row;
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
        let name_length = u32_at(row, 120).unwrap() as usize;
        let name = if metadata && u32_at(row, 124) == Some(1) && (1..=64).contains(&name_length) {
            let bytes = &row[128..128 + name_length];
            if bytes
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            {
                std::str::from_utf8(bytes).ok()
            } else {
                None
            }
        } else {
            None
        };
        let hex = |offset| format!("0x{:x}", q(offset));
        let before = (metadata && stable && state == 2 && slot < 2)
            .then(|| production(row, 192))
            .flatten();
        let after = (metadata && stable && state == 2 && slot < 2)
            .then(|| production(row, 2256))
            .flatten();
        let difference = before.as_ref().zip(after.as_ref()).and_then(|(before, after)| {
            if before["length"] != after["length"] { return None; }
            let before = before["coefficients_q8"].as_array()?;
            let after = after["coefficients_q8"].as_array()?;
            Some(before.iter().zip(after).enumerate().filter_map(|(index,(b,a))| {
                let delta = a.as_i64()? - b.as_i64()?;
                (delta != 0).then(|| json!({"unit_index":index,"before_q8":b,"after_q8":a,"difference_q8":delta}))
            }).collect::<Vec<_>>())
        });
        let helper_before = if version == 5 && stable && matches!(state, 1..=3) {
            helper_boundary(row, ROW, u32_at(row, 8).unwrap())?
        } else {
            Value::Null
        };
        let helper_after = if version == 5 && stable && state == 2 {
            helper_boundary(row, ROW + 48, u32_at(row, 8).unwrap())?
        } else {
            Value::Null
        };
        let helper_unwind = if version == 5 && stable && state == 3 {
            helper_boundary(row, ROW + 48, u32_at(row, 8).unwrap())?
        } else {
            Value::Null
        };
        rows.push(json!({"sequence":index+1,"state":if !stable { "unknown_unstable" } else { match state {2=>"completed",3=>"unwound",_=>"incomplete"} },
            "slot":slot,"family":match slot / 2 {0=>"military",1=>"naval",2=>"building",3=>"growth",_=>"unknown"},
            "call":if slot.is_multiple_of(2) {"apply"} else {"remove"},
            "thread_id":u32_at(row,8),"effect":hex(16),"actual_receiver_argument":hex(24),
            "raw_r8":hex(32),"caller_return":hex(40),"caller_rbp":hex(48),"caller_rdi":hex(56),
            "original_rax":if stable && state == 2 { json!(hex(64)) } else { Value::Null },
            "call_entry_metadata":if metadata { json!({"instance_id":u32_at(row,76),"instance":hex(80),"definition":hex(88),
                "city_wrapper":hex(96),"declared_percent":u32_at(row,104).unwrap() as i32,"owner":u32_at(row,108),"full_id":u32_at(row,112),
                "definition_vtable_rva":if u32_at(row,116).unwrap() != 0 {json!(format!("0x{:x}",u32_at(row,116).unwrap()))} else {Value::Null},
                "definition_name_at_call_entry":name}) } else { Value::Null },
            "production_array_before":before,"production_array_after":after,
            "helper_boundary_before":helper_before,"helper_boundary_after":helper_after,
            "helper_boundary_unwind":helper_unwind,
            "native_holding_established":false,
            "observed_production_differences":difference,
            "production_attribution_established":false,
            "quantity_accepted":false}));
    }
    let stopped = receipt.phase == 3 && u32_at(second, 24) == Some(2);
    let detached = u32_at(second, 52) == Some(1);
    let in_flight = u32_at(second, 16).unwrap();
    let dropped = u32_at(second, 12).unwrap();
    // The receipt and two remote copies are separate observations. Only
    // equal stopped counters can describe this snapshot as complete.
    let counters_match = receipt.trace_size as usize == second.len()
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
        json!({"capture":"callee","trace_version":version,"rows":rows,"row_count":count,
        "completed":u32_at(second,20),"unwound":u32_at(second,28),"in_flight":in_flight,"dropped":dropped,
        "stopped":stopped,"entry_slots_restored":detached,"complete":complete,"snapshot_consistent":complete,"unstable_rows":unstable,"receipt_counters_match":counters_match,
        "image_base":format!("0x{:x}",u64_at(second,32).unwrap()),"deadline_hit":u32_at(second,48)==Some(1),
        "exe_base":if version==5 {json!(format!("0x{:x}",u64_at(second,56).unwrap()))} else {Value::Null},
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
            (4, 4),
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
        let mut with_table = bytes.clone();
        with_table[HEADER + 72..HEADER + 76].copy_from_slice(&1u32.to_le_bytes());
        assert!(
            project(&with_table, &with_table, &r).unwrap()["rows"][0]["call_entry_metadata"]
                ["definition_vtable_rva"]
                .is_null()
        );
        with_table[HEADER + 116..HEADER + 120].copy_from_slice(&0xab1230u32.to_le_bytes());
        assert_eq!(
            project(&with_table, &with_table, &r).unwrap()["rows"][0]["call_entry_metadata"]
                ["definition_vtable_rva"],
            "0xab1230"
        );
        let mut old_version = bytes.clone();
        old_version[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert!(validate(&old_version).is_err());
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
            (HEADER, ROW_V5, SIZE_V5)
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

    #[test]
    fn name_copy_is_bounded_and_unknown_values_or_unstable_rows_never_gain_a_label() {
        let mut bytes = sample();
        let label = b"JAC_OATH_ODA_TENKA_FUBU_E1";
        for (offset, value) in [(72, 1u32), (120, label.len() as u32), (124, 1)] {
            bytes[HEADER + offset..HEADER + offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[HEADER + 128..HEADER + 128 + label.len()].copy_from_slice(label);
        let r = receipt();
        let value = project(&bytes, &bytes, &r).unwrap();
        assert_eq!(
            value["rows"][0]["call_entry_metadata"]["definition_name_at_call_entry"],
            "JAC_OATH_ODA_TENKA_FUBU_E1"
        );
        assert_eq!(value["quantity_accepted"], false);
        for (offset, value) in [(120, 0u32), (120, 65), (124, 0), (124, 2)] {
            let mut changed = bytes.clone();
            changed[HEADER + offset..HEADER + offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(
                project(&changed, &changed, &r).unwrap()["rows"][0]["call_entry_metadata"]
                    ["definition_name_at_call_entry"]
                    .is_null()
            );
        }
        let mut changed = bytes.clone();
        changed[HEADER + 128] = 0xff;
        assert!(
            project(&changed, &changed, &r).unwrap()["rows"][0]["call_entry_metadata"]
                ["definition_name_at_call_entry"]
                .is_null()
        );
        assert!(project(&bytes, &changed, &r).unwrap()["rows"][0]["call_entry_metadata"].is_null());
    }

    #[test]
    fn production_changes_are_signed_and_missing_or_unstable_copies_remain_unknown() {
        let mut bytes = sample();
        bytes[HEADER + 72..HEADER + 76].copy_from_slice(&1u32.to_le_bytes());
        for at in [192, 2256] {
            let offset = HEADER + at;
            bytes[offset..offset + 8].copy_from_slice(&0x9004u64.to_le_bytes());
            bytes[offset + 8..offset + 12].copy_from_slice(&2u32.to_le_bytes());
            bytes[offset + 12..offset + 16].copy_from_slice(&1u32.to_le_bytes());
            bytes[offset + 16..offset + 20].copy_from_slice(&(-256i32).to_le_bytes());
        }
        bytes[HEADER + 2272..HEADER + 2276].copy_from_slice(&(-218i32).to_le_bytes());
        let r = receipt();
        let value = project(&bytes, &bytes, &r).unwrap();
        assert_eq!(
            value["rows"][0]["observed_production_differences"][0]["difference_q8"],
            38
        );
        assert_eq!(
            value["rows"][0]["production_attribution_established"],
            false
        );
        for (at, word) in [(204, 0u32), (2268, 0), (200, 513), (2264, 0)] {
            let mut changed = bytes.clone();
            changed[HEADER + at..HEADER + at + 4].copy_from_slice(&word.to_le_bytes());
            assert!(project(&changed, &changed, &r).unwrap()["rows"][0]
                ["observed_production_differences"]
                .is_null());
        }
        let mut changed = bytes.clone();
        changed[HEADER + 2272] ^= 1;
        let value = project(&bytes, &changed, &r).unwrap();
        assert!(value["rows"][0]["production_array_before"].is_null());
        assert!(value["rows"][0]["observed_production_differences"].is_null());
        bytes[HEADER..HEADER + 4].copy_from_slice(&3u32.to_le_bytes());
        assert!(
            project(&bytes, &bytes, &r).unwrap()["rows"][0]["production_array_after"].is_null()
        );
    }

    #[test]
    fn helper_v5_boundaries_never_grant_continuous_holding_and_v4_stays_separate() {
        let old = sample();
        let legacy = project(&old, &old, &receipt()).unwrap();
        assert_eq!(legacy["trace_version"], 4);
        assert!(legacy["rows"][0]["helper_boundary_before"].is_null());
        let mut bytes = vec![0; SIZE_V5];
        bytes[..HEADER + ROW].copy_from_slice(&old[..HEADER + ROW]);
        bytes[4..8].copy_from_slice(&5u32.to_le_bytes());
        bytes[56..64].copy_from_slice(&0x140000000u64.to_le_bytes());
        bytes[HEADER + 8..HEADER + 12].copy_from_slice(&77u32.to_le_bytes());
        let at = HEADER + ROW;
        bytes[at..at + 4].copy_from_slice(&7u32.to_le_bytes());
        for sample in [at + 16, at + 32] {
            bytes[sample..sample + 8].copy_from_slice(&0x8000u64.to_le_bytes());
            bytes[sample + 8..sample + 12].copy_from_slice(&77u32.to_le_bytes());
            bytes[sample + 12..sample + 16].copy_from_slice(&1u32.to_le_bytes());
        }
        let mut r = receipt();
        r.trace_size = SIZE_V5 as u32;
        let value = project(&bytes, &bytes, &r).unwrap();
        let probe = &value["rows"][0]["helper_boundary_before"];
        assert_eq!(value["trace_version"], 5);
        assert_eq!(probe["helper_owned_at_boundary"], true);
        assert_eq!(probe["continuous_holding_proved"], false);
        assert_eq!(probe["native_lifetime_proved"], false);
        assert_eq!(value["rows"][0]["native_holding_established"], false);
        assert!(value["rows"][0]["helper_boundary_after"]["helper_owned_at_boundary"].is_null());
        for (offset, word, expected) in [
            (at, 0u32, Value::Null),
            (at, 3u32, Value::Null),
            (at + 40, 78, Value::Null), // producer's stable bit cannot hide changed samples
            (at + 44, u32::MAX, Value::Null),
        ] {
            let mut changed = bytes.clone();
            changed[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
            assert_eq!(
                project(&changed, &changed, &r).unwrap()["rows"][0]["helper_boundary_before"]
                    ["helper_owned_at_boundary"],
                expected
            );
        }
        for owner in [0u32, 78] {
            let mut changed = bytes.clone();
            for offset in [at + 24, at + 40] {
                changed[offset..offset + 4].copy_from_slice(&owner.to_le_bytes());
            }
            assert_eq!(
                project(&changed, &changed, &r).unwrap()["rows"][0]["helper_boundary_before"]
                    ["helper_owned_at_boundary"],
                false
            );
        }
        let mut changed = bytes.clone();
        for offset in [at + 28, at + 44] {
            changed[offset..offset + 4].copy_from_slice(&0u32.to_le_bytes());
        }
        assert_eq!(
            project(&changed, &changed, &r).unwrap()["rows"][0]["helper_boundary_before"]
                ["helper_owned_at_boundary"],
            false
        );
        let mut invalid = bytes.clone();
        invalid[at + 4..at + 8].copy_from_slice(&1u32.to_le_bytes());
        assert!(project(&invalid, &invalid, &r).is_err());
        assert!(project(&old, &bytes, &r).is_err());
        invalid = bytes.clone();
        invalid[4..8].copy_from_slice(&4u32.to_le_bytes());
        assert!(validate(&invalid).is_err());
        changed = bytes.clone();
        changed[at + 24] ^= 1;
        assert!(
            project(&bytes, &changed, &r).unwrap()["rows"][0]["helper_boundary_before"].is_null()
        );
    }
}
