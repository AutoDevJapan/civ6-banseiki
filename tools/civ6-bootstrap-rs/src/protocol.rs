//! Portable DTO/trace framing, not game or lifecycle proof.
use serde_json::{json, Value};
use std::path::PathBuf;
pub const HEADER: usize = 72;
pub const ROW: usize = 104;
pub const CAPACITY: usize = 128;
pub const TRACE_BYTES: usize = HEADER + ROW * CAPACITY;
pub const ROOT_PREPARE_VERSION: u32 = 0x0880_1001;
pub const ROOT_CONFIG_VERSION: u32 = 0x0880_1002;
pub const ROOT_MAGIC: u64 = 0x544f_4f52_3038_3830;
pub const ROOT_VERSION: u64 = 1;
pub const ROOT_HEADER: usize = 448;
pub const ROOT_ROW: usize = 192;
pub const ROOT_ROWS: usize = 128;
pub const ROOT_BYTES: usize = ROOT_HEADER + ROOT_ROW * ROOT_ROWS;
pub const ROOT_CLOSED: u64 = 1 << 63;
pub const SUBMISSION_PREPARE_VERSION: u32 = 0x0880_2001;
pub const SUBMISSION_CONFIG_VERSION: u32 = 0x0880_2002;
pub const SUBMISSION_MAGIC: u64 = 0x424f_4a53_3038_3830;
pub const SUBMISSION_V1_ROW: usize = 256;
pub const SUBMISSION_V1_BYTES: usize = ROOT_HEADER + SUBMISSION_V1_ROW * ROOT_ROWS;
pub const SUBMISSION_V2_ROW: usize = 272;
pub const SUBMISSION_V2_BYTES: usize = ROOT_HEADER + SUBMISSION_V2_ROW * ROOT_ROWS;
pub const SUBMISSION_V3_ROW: usize = 280;
pub const SUBMISSION_V3_BYTES: usize = ROOT_HEADER + SUBMISSION_V3_ROW * ROOT_ROWS;
pub const SUBMISSION_ROW: usize = 384;
pub const SUBMISSION_V6_HEADER: usize = 640;
pub const SUBMISSION_BIRTHS: usize = 16;
pub const SUBMISSION_V6_BYTES: usize =
    SUBMISSION_V6_HEADER + SUBMISSION_ROW * ROOT_ROWS + SUBMISSION_BIRTHS * 144;
pub const SUBMISSION_CHAIN_ROWS: usize = 12;
pub const SUBMISSION_ROWS: usize = SUBMISSION_CHAIN_ROWS * SUBMISSION_BIRTHS;
// Separate diagnostic pairs, bounded by the existing invocation depth (8).
pub const SUBMISSION_EXTRA_ROWS: usize = 2 * 8;
pub const SUBMISSION_HEADER: usize = 704;
pub const SUBMISSION_BIRTH_WORDS: usize = 22;
pub const SUBMISSION_BYTES: usize = SUBMISSION_HEADER
    + SUBMISSION_ROW * (SUBMISSION_ROWS + SUBMISSION_EXTRA_ROWS)
    + SUBMISSION_BIRTHS * SUBMISSION_BIRTH_WORDS * 8;

#[cfg(test)]
mod observed_submission_tests {
    use super::*;
    const CORE: u64 = 0x180000000;
    const EXE: u64 = 0x140000000;
    const P: u64 = 0x50000;
    const T: u64 = 0x60000;
    fn put(b: &mut [u8], word: usize, v: u64) {
        b[word * 8..word * 8 + 8].copy_from_slice(&v.to_le_bytes());
    }
    fn row_word(row: usize, field: usize) -> usize {
        SUBMISSION_V6_HEADER / 8 + row * SUBMISSION_ROW / 8 + field
    }
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; SUBMISSION_V6_BYTES];
        for (i, v) in [
            (0, SUBMISSION_MAGIC),
            (1, 4),
            (2, SUBMISSION_V6_BYTES as u64),
            (3, 128),
            (4, 384),
            (5, 23),
            (6, 29),
            (7, CORE),
            (8, EXE),
            (9, 1),
            (10, 12),
            (12, ROOT_CLOSED),
            (15, 63),
            (19, 3),
            (23, 2),
            (24, 12),
            (25, 63),
            (26, 63),
            (27, 3),
            (28, 1),
            (29, 1),
        ] {
            put(&mut b, i, v);
        }
        for (i, (rva, bytes)) in [
            (0x992210, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x6c]),
            (0x992280, [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b]),
            (0x4a79b0, [0x48, 0x89, 0x74, 0x24, 0x18, 0x57, 0x48, 0x83]),
            (0x985ec0, [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b]),
            (0x4a8530, [0x40, 0x57, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b]),
            (0x986060, [0x48, 0x83, 0xec, 0x38, 0x41, 0x0f, 0x10, 1]),
        ]
        .into_iter()
        .enumerate()
        {
            let w = u64::from_le_bytes(bytes);
            for (j, v) in [rva, w, w, 1, 1, 0, 0, 0].into_iter().enumerate() {
                put(&mut b, 32 + i * 8 + j, v);
            }
        }
        // Reservation order differs from event order, including execution on another thread.
        for (i, (kind, parent, rsp, caller, entry, exit)) in [
            (6, 0, 0x101d0, CORE + 0x1000, 1, 12),
            (4, 1, 0xff90, CORE + 0x4a114a, 2, 5),
            (9, 3, 0xfd80, CORE + 0x992267, 3, 4),
            (5, 1, 0xffd0, CORE + 0x4a7a58, 6, 7),
            (7, 0, 0x201d0, EXE + 0x1000, 8, 11),
            (8, 9, 0x1ffd0, EXE + 0x985ef4, 9, 10),
        ]
        .into_iter()
        .enumerate()
        {
            let mut a = [0; 48];
            a[1] = kind;
            a[2] = 1;
            a[3] = (i * 2 + 1) as u64;
            a[4] = parent;
            a[5] = if i < 4 { 31 } else { 41 };
            a[6] = if i < 4 { 37 } else { 43 };
            a[7] = 1;
            a[8] = caller;
            a[9] = rsp;
            a[23] = entry;
            a[29] = rsp - 0x1d0;
            a[35] = 1;
            a[36] = P;
            match kind {
                6 => a[10] = P - 0xd0,
                4 | 9 => {
                    a[10] = 2;
                    a[11] = P;
                    a[12] = 0x30000;
                }
                5 => {
                    a[10] = 2;
                    a[37] = T;
                }
                7 => {
                    a[10] = T;
                    a[37] = T;
                    a[38] = 16;
                    a[39] = 1;
                    a[40] = EXE + 0x106b3c8;
                    a[41] = P;
                }
                8 => {
                    a[10] = P;
                    a[37] = T;
                    a[38] = 64;
                    a[39] = 1;
                    a[40] = CORE + 0xa06648;
                }
                _ => unreachable!(),
            }
            if kind == 4 {
                a[13] = rsp + 0x28;
                a[14] = CORE + 0xa06648;
                a[18] = CORE + 0xa06920;
                a[19] = 0x1f00000000;
                a[24] = 32;
                a[26] = 16;
                a[30] = 1;
                a[31] = 1;
                a[32] = CORE + 0x4a7a50;
                a[33] = 0x108;
            }
            let mut z = a;
            z[2] = 2;
            z[23] = exit;
            if kind == 4 || kind == 9 {
                z[34] = T;
            }
            if kind == 4 {
                z[37] = T;
            }
            for (row, r) in [(i * 2, a), (i * 2 + 1, z)] {
                for (j, v) in r.into_iter().enumerate() {
                    put(&mut b, row_word(row, j), v);
                }
                put(&mut b, row_word(row, 0), row as u64 + 1);
            }
        }
        let ledger = (SUBMISSION_V6_HEADER + ROOT_ROWS * SUBMISSION_ROW) / 8;
        for (j, v) in [
            1,
            P,
            T,
            1,
            3,
            7,
            9,
            11,
            31,
            37,
            1,
            511 | 8192 | 16384,
            0x10000,
            0x30000,
            2,
            0,
            5,
            T,
        ]
        .into_iter()
        .enumerate()
        {
            put(&mut b, ledger + j, v);
        }
        b
    }
    #[test]
    fn observed_governor_actual_callee_and_both_execution_frames_are_sampling_only() {
        let v = submission_trace(&fixture(), 23, 29).unwrap();
        assert_eq!(v["capture_complete"], true);
        assert_eq!(v["complete_observed_p_t_execution_chains"], 1);
        assert_eq!(v["runtime_activated"], false);
        assert_eq!(v["world_permit_issued"], false);
    }
    #[test]
    fn actual_callee_missing_foreign_caller_identity_or_return_cannot_type_raw_r8() {
        for (row, field, value) in [
            (4, 4, 1),
            (4, 5, 99),
            (4, 6, 99),
            (4, 8, CORE + 0x99226a),
            (4, 9, 0xfd88),
            (5, 34, T + 8),
            (5, 23, 6),
            (5, 2, 3),
            (3, 34, T + 8),
            (2, 33, 0),
            (8, 41, P + 8),
            (10, 40, CORE + 0xa06650),
            (10, 4, 1),
            (11, 2, 3),
        ] {
            let mut b = fixture();
            put(&mut b, row_word(row, field), value);
            let v = submission_trace(&b, 23, 29).unwrap();
            assert_eq!(v["capture_complete"], false, "row{row} field{field}");
            assert_eq!(
                v["rows"][3]["raw_return_r8"],
                if row == 3 && field == 34 { T + 8 } else { T }
            );
        }
    }
    #[test]
    fn cold_resolution_needs_entry_witness_and_cached_aba_is_never_evidence() {
        // Cache values are deliberately absent from v4. A cold resolver reaches the
        // exact callee; known->other and ABA with no actual entry cannot manufacture it.
        assert_eq!(
            submission_trace(&fixture(), 23, 29).unwrap()["capture_complete"],
            true
        );
        for token in [0, 3, 7] {
            let mut b = fixture();
            let ledger = (SUBMISSION_V6_HEADER + ROOT_ROWS * SUBMISSION_ROW) / 8;
            put(&mut b, ledger + 16, token);
            let v = submission_trace(&b, 23, 29).unwrap();
            assert_eq!(v["capture_complete"], false);
            assert_eq!(v["rows"][3]["raw_return_r8"], T);
        }
    }
    #[test]
    fn observed_pending_reused_epoch_partial_sites_and_dropped_remain_unknown() {
        let ledger = (SUBMISSION_V6_HEADER + ROOT_ROWS * SUBMISSION_ROW) / 8;
        for (offset, value) in [
            (ledger + 7, 0),
            (ledger + 10, 2),
            (ledger + 15, 1),
            (15, 31),
            (11, 1),
            (13, 1),
            (32 + 5 * 8 + 5, 1),
            (32 + 3 * 8 + 4, 0),
            (25, 31),
            (28, 0),
        ] {
            let mut b = fixture();
            put(&mut b, offset, value);
            assert_eq!(
                submission_trace(&b, 23, 29).unwrap()["capture_complete"],
                false,
                "word{offset}"
            );
        }
    }
    pub(super) fn provisional_fixture() -> Vec<u8> {
        let mut b = fixture();
        put(&mut b, 1, 5);
        put(&mut b, 20, 100);
        put(&mut b, 21, 101);
        let ledger = (SUBMISSION_V6_HEADER + ROOT_ROWS * SUBMISSION_ROW) / 8;
        put(&mut b, ledger, 101);
        for row in 0..12 {
            put(&mut b, row_word(row, 35), 101);
            let event = word(&b, row_word(row, 23) * 8).unwrap() + 100;
            put(&mut b, row_word(row, 23), event);
        }
        put(&mut b, 24, 112);
        b
    }
    #[test]
    fn observed_provisional_no_submit_100_preserves_serial_and_one_stored_birth() {
        let v = submission_trace(&provisional_fixture(), 23, 29).unwrap();
        assert_eq!(v["capture_complete"], true);
        assert_eq!(v["entered_producer_serials"], 101);
        assert_eq!(v["stored_birth_count"], 1);
        assert_eq!(v["normal_no_submit_returns"], 100);
        assert_eq!(v["world_permit_issued"], false);
        let mut old = fixture();
        put(&mut old, 29, 32);
        assert!(
            submission_trace(&old, 23, 29)
                .unwrap_err()
                .contains("bounded count"),
            "original v4 pressure stays failure"
        );
    }
    #[test]
    fn observed_same_p_t_serial_reissue_needs_all_previous_normal_returns_before_new_entry() {
        let mut b = provisional_fixture();
        let ledger = (SUBMISSION_V6_HEADER + ROOT_ROWS * SUBMISSION_ROW) / 8;
        for row in 0..12 {
            let original = b[SUBMISSION_V6_HEADER + row * SUBMISSION_ROW
                ..SUBMISSION_V6_HEADER + (row + 1) * SUBMISSION_ROW]
                .to_vec();
            let next = 12 + row;
            b[SUBMISSION_V6_HEADER + next * SUBMISSION_ROW
                ..SUBMISSION_V6_HEADER + (next + 1) * SUBMISSION_ROW]
                .copy_from_slice(&original);
            for field in [0, 3] {
                let value = word(&b, row_word(next, field) * 8).unwrap() + 12;
                put(&mut b, row_word(next, field), value);
            }
            let parent = word(&b, row_word(next, 4) * 8).unwrap();
            if parent > 0 {
                put(&mut b, row_word(next, 4), parent + 12);
            }
            let event = word(&b, row_word(next, 23) * 8).unwrap() + 12;
            put(&mut b, row_word(next, 23), event);
            put(&mut b, row_word(next, 35), 102);
        }
        for field in 0..18 {
            let mut value = word(&b, (ledger + field) * 8).unwrap();
            if [3, 4, 5, 6, 7, 16].contains(&field) {
                value += 12;
            }
            if field == 0 {
                value = 102;
            }
            put(&mut b, ledger + 18 + field, value);
        }
        put(&mut b, 10, 24);
        put(&mut b, 24, 124);
        put(&mut b, 29, 2);
        put(&mut b, 21, 102);
        assert_eq!(
            submission_trace(&b, 23, 29).unwrap()["complete_observed_p_t_execution_chains"],
            2
        );
        assert_eq!(
            submission_trace(&b, 23, 29).unwrap()["capture_complete"],
            true
        );
        for (row, field, value) in [
            (1, 2, 3),
            (9, 2, 3),
            (12, 23, 111),
            (8, 41, P + 8),
            (22, 4, 1),
        ] {
            let mut bad = b.clone();
            put(&mut bad, row_word(row, field), value);
            assert_eq!(
                submission_trace(&bad, 23, 29).unwrap()["capture_complete"],
                false,
                "row{row} field{field}"
            );
        }
    }
    #[test]
    fn observed_v6_current_host_p_vft_read_witness_is_required_and_frozen() {
        let mut b = provisional_fixture();
        put(&mut b, 1, 6);
        put(&mut b, 30, 1000);
        for row in [8, 9] {
            for (field, value) in [(42, CORE + 0xa06648), (43, 8), (44, 0), (45, 1)] {
                put(&mut b, row_word(row, field), value);
            }
        }
        let v = submission_trace(&b, 23, 29).unwrap();
        assert_eq!(v["capture_complete"], true);
        assert_eq!(v["schema"], "todo880-native-submission-observed-trace-v6");
        assert_eq!(v["world_permit_issued"], false);
        assert_eq!(
            v["rows"][8]["entry_snapshot_words"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        for (field, value) in [(42, CORE + 0xa06650), (43, 0), (43, 4), (44, 299), (45, 0)] {
            let mut bad = b.clone();
            for row in [8, 9] {
                put(&mut bad, row_word(row, field), value);
            }
            assert_eq!(
                submission_trace(&bad, 23, 29).unwrap()["capture_complete"],
                false,
                "field{field}"
            );
        }
        let mut late = b.clone();
        put(&mut late, row_word(9, 42), CORE + 0xa06650);
        assert_eq!(
            submission_trace(&late, 23, 29).unwrap()["capture_complete"],
            false
        );
    }
}

#[cfg(test)]
mod submission_reservation_tests {
    use super::*;
    fn put(b: &mut [u8], offset: usize, v: u64) {
        b[offset..offset + 8].copy_from_slice(&v.to_le_bytes());
    }
    fn fixture(cycles: usize) -> Vec<u8> {
        let old = super::observed_submission_tests::provisional_fixture();
        let mut b = vec![0; SUBMISSION_BYTES];
        b[..640].copy_from_slice(&old[..640]);
        for (i, v) in [
            (1, 7),
            (2, SUBMISSION_BYTES as u64),
            (3, SUBMISSION_ROWS as u64),
            (10, (cycles * 12) as u64),
            (20, 100),
            (21, 100 + cycles as u64),
            (24, 100 + (cycles * 12) as u64),
            (29, cycles as u64),
        ] {
            put(&mut b, i * 8, v);
        }
        for (i, v) in [
            SUBMISSION_ROWS as u64,
            12,
            16,
            SUBMISSION_BIRTH_WORDS as u64,
            SUBMISSION_EXTRA_ROWS as u64,
            0,
            0,
            0,
        ]
        .into_iter()
        .enumerate()
        {
            put(&mut b, 640 + i * 8, v);
        }
        let ledger = SUBMISSION_HEADER + (SUBMISSION_ROWS + SUBMISSION_EXTRA_ROWS) * SUBMISSION_ROW;
        let old_ledger = SUBMISSION_V6_HEADER + ROOT_ROWS * SUBMISSION_ROW;
        for cycle in 0..cycles {
            for row in 0..12 {
                let pos = SUBMISSION_HEADER + (cycle * 12 + row) * SUBMISSION_ROW;
                b[pos..pos + SUBMISSION_ROW].copy_from_slice(
                    &old[SUBMISSION_V6_HEADER + row * SUBMISSION_ROW
                        ..SUBMISSION_V6_HEADER + (row + 1) * SUBMISSION_ROW],
                );
                for f in [0, 3] {
                    let v = word(&b, pos + f * 8).unwrap() + cycle as u64 * 12;
                    put(&mut b, pos + f * 8, v);
                }
                let p = word(&b, pos + 4 * 8).unwrap();
                if p > 0 {
                    put(&mut b, pos + 4 * 8, p + cycle as u64 * 12);
                }
                let event = word(&b, pos + 23 * 8).unwrap() + cycle as u64 * 12;
                put(&mut b, pos + 23 * 8, event);
                put(&mut b, pos + 35 * 8, 101 + cycle as u64);
                if row == 8 || row == 9 {
                    for (f, v) in [(42, 0x180a06648), (43, 8), (44, 0), (45, 1)] {
                        put(&mut b, pos + f * 8, v);
                    }
                }
            }
            for f in 0..18 {
                let mut v = word(&old, old_ledger + f * 8).unwrap();
                if [3, 4, 5, 6, 7, 16].contains(&f) {
                    v += cycle as u64 * 12;
                }
                if f == 0 {
                    v = 101 + cycle as u64;
                }
                put(&mut b, ledger + (cycle * SUBMISSION_BIRTH_WORDS + f) * 8, v);
            }
            for (f, v) in [(18, (cycle * 12) as u64), (19, 63), (20, 63), (21, 0)] {
                put(&mut b, ledger + (cycle * SUBMISSION_BIRTH_WORDS + f) * 8, v);
            }
        }
        b
    }
    #[test]
    fn submission_v7_sixteen_cycles_use_derived_192_rows_and_keep_v6_layout() {
        assert_eq!(SUBMISSION_ROWS, 16 * 12);
        assert_eq!(ROOT_ROWS, 128);
        assert_eq!(SUBMISSION_V6_BYTES, 52096);
        let v = submission_trace(&fixture(16), 23, 29).unwrap();
        assert_eq!(v["capture_complete"], true);
        assert_eq!(v["complete_observed_p_t_execution_chains"], 16);
        assert_eq!(v["reserved_rows"], 192);
        assert_eq!(v["world_permit_issued"], false);
        assert_eq!(
            submission_trace(
                &super::observed_submission_tests::provisional_fixture(),
                23,
                29
            )
            .unwrap()["capture_complete"],
            true
        );
    }
    #[test]
    fn submission_v7_unused_owned_pairs_are_unresolved_not_cancelled_or_completed() {
        let mut b = fixture(1);
        let ledger = SUBMISSION_HEADER + (SUBMISSION_ROWS + SUBMISSION_EXTRA_ROWS) * SUBMISSION_ROW;
        b[SUBMISSION_HEADER + 8 * SUBMISSION_ROW..SUBMISSION_HEADER + 12 * SUBMISSION_ROW].fill(0);
        for (f, v) in [
            (6, 0),
            (7, 0),
            (11, 1 | 2 | 4 | 8 | 16 | 8192 | 16384),
            (19, 15),
            (20, 15),
            (21, 1),
        ] {
            put(&mut b, ledger + f * 8, v);
        }
        let v = submission_trace(&b, 23, 29).unwrap();
        assert_eq!(v["capture_complete"], false);
        assert_eq!(v["reserved_unused_pair_starts"], json!([8, 10]));
        assert_eq!(v["births"][0]["unresolved_not_executed_t"], true);
        assert_eq!(v["births"][0]["reservation"]["sampling_flight_held"], true);
    }
    #[test]
    fn submission_v7_seh_wrong_owner_duplicate_mask_extra_and_partial_pairs_stay_unknown() {
        let good = fixture(1);
        let ledger = SUBMISSION_HEADER + (SUBMISSION_ROWS + SUBMISSION_EXTRA_ROWS) * SUBMISSION_ROW;
        for (offset, value) in [
            (ledger + 18 * 8, 12),
            (ledger + 19 * 8, 127),
            (ledger + 20 * 8, 31),
            (ledger + 21 * 8, 2),
            (SUBMISSION_HEADER + 9 * SUBMISSION_ROW + 2 * 8, 3),
            (688, 1),
            (SUBMISSION_HEADER + 11 * SUBMISSION_ROW, 0),
        ] {
            let mut b = good.clone();
            put(&mut b, offset, value);
            assert_eq!(
                submission_trace(&b, 23, 29).unwrap()["capture_complete"],
                false,
                "offset{offset}"
            );
        }
    }
    #[test]
    fn submission_v8_first_sixteen_seal_is_explicit_and_never_clears_existing_losses() {
        let mut b = fixture(16);
        put(&mut b, 8, 8);
        put(&mut b, 696, 1);
        let v = submission_trace(&b, 23, 29).unwrap();
        assert_eq!(v["capture_complete"], true);
        assert_eq!(v["sample_cycle_limit"], 16);
        assert_eq!(v["sample_limit_sealed"], true);
        assert_eq!(v["pending_observed_cycles"], 0);
        assert!(v["native_pending_jobs"].is_null());
        for (pos, val) in [(696, 0), (688, 22), (112, 30)] {
            let mut bad = b.clone();
            put(&mut bad, pos, val);
            assert_eq!(
                submission_trace(&bad, 23, 29).unwrap()["capture_complete"],
                false
            );
        }
        let old = fixture(16);
        assert_eq!(
            submission_trace(&old, 23, 29).unwrap()["schema"],
            "todo880-native-submission-observed-trace-v7"
        );
    }
    #[test]
    fn submission_v8_zero_admission_flights_do_not_erase_unresolved_book_cycles() {
        let mut b = fixture(1);
        put(&mut b, 8, 8);
        let ledger = SUBMISSION_HEADER + (SUBMISSION_ROWS + SUBMISSION_EXTRA_ROWS) * SUBMISSION_ROW;
        b[SUBMISSION_HEADER + 8 * SUBMISSION_ROW..SUBMISSION_HEADER + 12 * SUBMISSION_ROW].fill(0);
        for (f, v) in [
            (6, 0),
            (7, 0),
            (11, 1 | 2 | 4 | 8 | 16 | 8192 | 16384),
            (19, 15),
            (20, 15),
            (21, 0),
        ] {
            put(&mut b, ledger + f * 8, v);
        }
        let v = submission_trace(&b, 23, 29).unwrap();
        assert_eq!(v["accepted_calls_stopped"], true);
        assert_eq!(v["pending_observed_cycles"], 1);
        assert_eq!(v["unresolved_recorded_t"], 1);
        assert!(v["native_pending_jobs"].is_null());
        assert_eq!(v["capture_complete"], false);
    }
}

fn submission_observed_trace(b: &[u8], pid: u32, created: u64) -> Result<Value, String> {
    let version = word(b, 8)?;
    let v7 = version >= 7;
    let header = if v7 {
        SUBMISSION_HEADER
    } else {
        SUBMISSION_V6_HEADER
    };
    let capacity = if v7 { SUBMISSION_ROWS } else { ROOT_ROWS };
    let trace_size = if v7 {
        SUBMISSION_BYTES
    } else {
        SUBMISSION_V6_BYTES
    };
    let birth_words = if v7 { SUBMISSION_BIRTH_WORDS } else { 18 };
    let physical_rows = capacity + if v7 { SUBMISSION_EXTRA_ROWS } else { 0 };
    if b.len() != trace_size
        || word(b, 0)? != SUBMISSION_MAGIC
        || !matches!(version, 4..=8)
        || word(b, 16)? != trace_size as u64
        || word(b, 24)? != capacity as u64
        || word(b, 32)? != SUBMISSION_ROW as u64
        || word(b, 40)? != u64::from(pid)
        || word(b, 48)? != created
    {
        return Err("observed submission size/version/identity".into());
    }
    let core = word(b, 56)?;
    let exe = word(b, 64)?;
    let epoch = word(b, 72)?;
    let next = usize::try_from(word(b, 80)?).map_err(|_| "observed row count")?;
    let count = usize::try_from(word(b, 232)?).map_err(|_| "observed birth count")?;
    let extra = if v7 {
        usize::try_from(word(b, 680)?).map_err(|_| "extra count")?
    } else {
        0
    };
    if next > capacity
        || next % 2 != 0
        || count > SUBMISSION_BIRTHS
        || extra > SUBMISSION_EXTRA_ROWS
        || extra % 2 != 0
    {
        return Err("observed bounded count".into());
    }
    let mut unknown = Vec::<String>::new();
    if v7 {
        if (0..5)
            .map(|i| word(b, 640 + i * 8).unwrap_or(0))
            .collect::<Vec<_>>()
            != vec![
                SUBMISSION_ROWS as u64,
                12,
                16,
                SUBMISSION_BIRTH_WORDS as u64,
                SUBMISSION_EXTRA_ROWS as u64,
            ]
            || (if version == 8 {
                word(b, 696)? > 1
            } else {
                word(b, 696)? != 0
            })
            || next != count * SUBMISSION_CHAIN_ROWS
        {
            return Err("chain reservation layout/count mismatch".into());
        }
        if version == 8 && (word(b, 696)? == 1) != (count == SUBMISSION_BIRTHS) {
            unknown.push("first-16 sampling seal does not match stored cycle limit".into());
        }
        if extra > 0 || word(b, 688)? != 0 {
            unknown
                .push("unassigned/duplicate/unsupported calls or diagnostic loss recorded".into());
        }
    }
    let serial = if version >= 5 {
        word(b, 168)?
    } else {
        count as u64
    };
    let no_submit = if version >= 5 { word(b, 160)? } else { 0 };
    if version >= 5
        && (count as u64 > serial
            || no_submit
                .checked_add(count as u64)
                .is_none_or(|n| n > serial))
    {
        unknown.push("stored births/no-submit aggregates exceed entered serials".into());
    }
    let mut sites = Vec::new();
    for (i, (rva, bytes)) in [
        (0x992210, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x6c]),
        (0x992280, [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b]),
        (0x4a79b0, [0x48, 0x89, 0x74, 0x24, 0x18, 0x57, 0x48, 0x83]),
        (0x985ec0, [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b]),
        (0x4a8530, [0x40, 0x57, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b]),
        (0x986060, [0x48, 0x83, 0xec, 0x38, 0x41, 0x0f, 0x10, 1]),
    ]
    .into_iter()
    .enumerate()
    {
        let s = (0..8)
            .map(|j| word(b, 256 + i * 64 + j * 8))
            .collect::<Result<Vec<_>, _>>()?;
        let expected = u64::from_le_bytes(bytes);
        if s[..5] != [rva, expected, expected, 1, 1] || s[5..] != [0, 0, 0] {
            unknown.push(format!("site{i} incomplete/rollback/failure"));
        }
        sites.push(s);
    }
    let mut pairs = std::collections::BTreeMap::<u64, ([u64; 48], [u64; 48])>::new();
    let mut rows = Vec::new();
    let mut events = std::collections::BTreeSet::new();
    let mut unused = Vec::new();
    for i in (0..next)
        .step_by(2)
        .chain((capacity..capacity + extra).step_by(2))
    {
        let read = |row: usize| -> Result<[u64; 48], String> {
            let mut r = [0; 48];
            for (j, v) in r.iter_mut().enumerate() {
                *v = word(b, header + row * SUBMISSION_ROW + j * 8)?;
            }
            Ok(r)
        };
        let a = read(i)?;
        let z = read(i + 1)?;
        if v7 && i < capacity && a == [0; 48] && z == [0; 48] {
            unused.push(i);
            continue;
        }
        for r in [&a, &z] {
            rows.push(json!({"raw":r.to_vec(),"kind":r[1],"edge":r[2],"invocation_id":r[3],
                "parent":r[4],"tid":r[5],"thread_created_filetime":r[6],"observed_core_epoch":r[7],
                "caller":r[8],"entry_rsp":r[9],"native_entry_rsp":r[29],
                "observed_birth_id":r[35],"opaque_p":r[36],"opaque_t":r[37],
                "raw_return_r8":if r[2]==2 {Some(r[34])}else{None},"errors":r[20],
                "current_entry_object_copied":r[38],"current_entry_read_status":r[39],
                "entry_snapshot_words":if version>=6&&r[1]==7 {&r[40..42]}else{&r[40..48]},
                "current_host_p_vft":if version>=6&&r[1]==7 {Some(r[42])}else{None},
                "current_host_p_vft_copied":if version>=6&&r[1]==7 {Some(r[43])}else{None},
                "current_host_p_vft_read_error":if version>=6&&r[1]==7 {Some(r[44])}else{None},
                "current_host_p_vft_read_raw_bool":if version>=6&&r[1]==7 {Some(r[45])}else{None},
                "status":match r[2]{1=>"entry",2=>"normal_return",3=>"exception_unwind",_=>"unknown"},
                "snapshot_scope":"entry only; no pointer dereference after return"}));
        }
        let frozen = (4..48)
            .filter(|j| !matches!(j, 20 | 21 | 22 | 23 | 28 | 34 | 37))
            .all(|j| a[j] == z[j]);
        let valid = a[0] == i as u64 + 1
            && z[0] == i as u64 + 2
            && (4..=9).contains(&a[1])
            && a[1] == z[1]
            && a[2] == 1
            && z[2] == 2
            && a[3] == i as u64 + 1
            && z[3] == a[3]
            && a[5] > 0
            && a[6] > 0
            && a[7] == epoch
            && epoch > 0
            && core > 0
            && exe > 0
            && a[9].checked_sub(0x1d0) == Some(a[29])
            && a[20] == 0
            && z[20] == 0
            && frozen
            && a[23] > 0
            && z[23] > a[23]
            && z[23] <= word(b, 192)?
            && events.insert(a[23])
            && events.insert(z[23]);
        if valid {
            pairs.insert(a[3], (a, z));
        } else {
            unknown.push(format!(
                "invocation{} missing/exception/identity/read/order",
                i + 1
            ));
        }
    }
    let ledger = header + physical_rows * SUBMISSION_ROW;
    let mut births = Vec::new();
    let mut used_p = std::collections::BTreeSet::new();
    let mut used_t = std::collections::BTreeSet::new();
    let mut used_ids = std::collections::BTreeSet::new();
    let mut previous_p = std::collections::BTreeMap::<u64, u64>::new();
    let mut previous_t = std::collections::BTreeMap::<u64, u64>::new();
    let mut accounted_calls = std::collections::BTreeSet::new();
    let mut complete_chains = 0;
    let mut pending_observed = 0;
    let mut unresolved_bound_t = 0;
    for i in 0..SUBMISSION_BIRTHS {
        let mut w = [0u64; SUBMISSION_BIRTH_WORDS];
        for (j, v) in w.iter_mut().enumerate().take(birth_words) {
            *v = word(b, ledger + i * birth_words * 8 + j * 8)?;
        }
        if i >= count {
            if w != [0; SUBMISSION_BIRTH_WORDS] {
                unknown.push("unused birth slot nonzero".into());
            }
            continue;
        }
        if v7 {
            let expected = [w[3], w[4], w[16], w[5], w[6], w[7]];
            if w[18] != i as u64 * 12 || w[19] & !63 != 0 || w[20] & !w[19] != 0 || w[21] & !3 != 0
            {
                unknown.push("birth reservation/mask mismatch".into());
            }
            for (r, token) in expected.into_iter().enumerate() {
                let start = i * 12 + r * 2;
                let used = w[19] & (1 << r) != 0;
                if (used && (token != start as u64 + 1 || unused.contains(&start)))
                    || (!used && (token != 0 || !unused.contains(&start)))
                {
                    unknown.push("reserved role token/unused slots differ".into());
                }
            }
            if w[19] != 63 || w[20] != 63 || w[21] != 0 {
                unknown.push("reserved cycle lacks complete observed pair settlement; never-executed T remains unresolved".into());
            }
        }
        for token in [w[3], w[4], w[5], w[6], w[7], w[16]]
            .into_iter()
            .filter(|t| *t != 0)
        {
            if !accounted_calls.insert(token) {
                unknown.push("invocation assigned to multiple birth roles".into());
            }
            if let Some((a, z)) = pairs.get(&token) {
                if a[35] != w[0] || a[36] != w[1] || z[36] != w[1] {
                    unknown.push("birth/opaque P differs from invocation".into());
                }
            }
        }
        let unresolved = w[2] != 0 && w[11] & 256 == 0;
        if v7 && (w[19] != 63 || w[20] != 63 || w[21] != 0 || w[15] != 0) {
            pending_observed += 1;
            if w[2] != 0 {
                unresolved_bound_t += 1;
            }
        }
        births.push(json!({"raw":w[..birth_words].to_vec(),"observed_birth_id":w[0],"opaque_p":w[1],"opaque_t":w[2],
            "producer":w[3],"submit":w[4],"publication":w[5],"host_exec":w[6],"core_exec":w[7],
            "actual_push_host":w[16],"actual_push_host_return_r8":w[17],
            "errors":w[15],"unresolved_not_executed_t":unresolved,"world_generation":null,"world_permit_issued":false,
            "reservation":if v7 {Some(json!({"row_start":w[18],"entered_pair_mask":w[19],"settled_pair_mask":w[20],
                "sampling_flight_held":w[21]&1!=0,"terminal_exception":w[21]&2!=0}))}else{None}}));
        let Some((p, pr)) = pairs.get(&w[3]) else {
            unknown.push(format!("birth{} producer pair missing", w[0]));
            continue;
        };
        if (if version == 4 {
            w[0] != i as u64 + 1
        } else {
            w[0] == 0 || w[0] > serial || !used_ids.insert(w[0])
        }) || w[1] == 0
            || (version == 4 && !used_p.insert(w[1]))
            || w[10] != epoch
            || p[1] != 6
            || p[4] != 0
            || p[10].checked_add(0xd0) != Some(w[1])
            || p[35] != w[0]
            || p[36] != w[1]
            || p[29] != w[12]
            || p[5] != w[8]
            || p[6] != w[9]
            || w[15] != 0
        {
            unknown.push(format!("birth{} producer/reusedP identity mismatch", w[0]));
            continue;
        }
        if w[11] & 4096 != 0 {
            if w[4..8] != [0; 4] || w[2] != 0 || w[11] != (1 | 16 | 4096) {
                unknown.push("no-submit birth contains task evidence".into());
            }
            continue;
        }
        let Some((s, sr)) = pairs.get(&w[4]) else {
            unknown.push("submit pair missing".into());
            continue;
        };
        let Some((c, cr)) = pairs.get(&w[16]) else {
            unknown.push("actual986060 callee pair missing".into());
            continue;
        };
        let Some((u, ur)) = pairs.get(&w[5]) else {
            unknown.push("publication pair missing".into());
            continue;
        };
        let Some((h, hr)) = pairs.get(&w[6]) else {
            unknown.push("T never observed executing; recovery unresolved".into());
            continue;
        };
        let Some((x, xr)) = pairs.get(&w[7]) else {
            unknown.push("Core execution pair missing".into());
            continue;
        };
        let relation = s[1] == 4
            && s[4] == p[3]
            && s[5..8] == p[5..8]
            && core.checked_add(0x4a114a) == Some(s[8])
            && s[9].checked_add(0x70) == Some(p[29])
            && s[11] == w[1]
            && s[35] == w[0]
            && s[10] == w[14]
            && s[12] == w[13]
            && p[23] < s[23]
            && core.checked_add(0x4a7a50) == Some(s[32])
            && s[33] == 0x108
            && s[13] == s[9].checked_add(0x28).unwrap_or(0)
            && s[18] == core.checked_add(0xa06920).unwrap_or(0)
            && s[19] == 0x1f00000000
            && matches!(s[24], 16 | 32)
            && s[25] == 0
            && s[26] == 16
            && s[27] == 0
            && s[30] != 0
            && s[31] != 0
            && c[1] == 9
            && c[4] == s[3]
            && c[5..8] == s[5..8]
            && core.checked_add(0x992267) == Some(c[8])
            && c[9].checked_add(0x40) == Some(s[29])
            && c[10..13] == s[10..13]
            && c[35] == w[0]
            && s[23] < c[23]
            && cr[23] < sr[23]
            && cr[34] > 0
            && cr[34] == sr[34]
            && sr[34] == w[2]
            && w[17] == w[2]
            && u[1] == 5
            && u[4] == p[3]
            && u[5..8] == p[5..8]
            && core.checked_add(0x4a7a58) == Some(u[8])
            && u[9].checked_add(0x30) == Some(p[29])
            && u[10] == s[10]
            && u[35] == w[0]
            && sr[23] < u[23]
            && ur[23] < pr[23]
            && h[1] == 7
            && h[10] == w[2]
            && h[35] == w[0]
            && h[37] == w[2]
            && h[38] == 16
            && h[39] == 1
            && exe.checked_add(0x106b3c8) == Some(h[40])
            && h[41] == w[1]
            && (version < 6
                || (h[42] == core.checked_add(0xa06648).unwrap_or(0)
                    && h[43] == 8
                    && h[44] == 0
                    && h[45] != 0))
            && sr[23] < h[23]
            && x[1] == 8
            && x[4] == h[3]
            && x[5..8] == h[5..8]
            && exe.checked_add(0x985ef4) == Some(x[8])
            && x[9].checked_add(0x30) == Some(h[29])
            && x[10] == w[1]
            && x[35] == w[0]
            && x[37] == w[2]
            && x[38] == 64
            && x[39] == 1
            && core.checked_add(0xa06648) == Some(x[40])
            && h[23] < x[23]
            && xr[23] < hr[23]
            && w[11] == (511 | 8192 | 16384)
            && (version >= 5 || used_t.insert(w[2]))
            && (version == 4
                || (previous_p.get(&w[1]).is_none_or(|end| *end < p[23])
                    && previous_t.get(&w[2]).is_none_or(|end| *end < p[23])));
        if relation {
            complete_chains += 1;
            if version >= 5 {
                let ended = [pr[23], sr[23], ur[23], hr[23], xr[23], cr[23]]
                    .into_iter()
                    .max()
                    .unwrap();
                previous_p.insert(w[1], ended);
                previous_t.insert(w[2], ended);
            }
        } else {
            unknown.push(format!(
                "birth{} actual P/T/frame/normal-return chain incomplete",
                w[0]
            ));
        }
    }
    if pairs.keys().any(|token| !accounted_calls.contains(token)) {
        unknown.push("accepted invocation absent from birth ledger".into());
    }
    let stopped = word(b, 96)? == ROOT_CLOSED && word(b, 184)? == 2;
    let clean = stopped
        && word(b, 88)? == 0
        && word(b, 104)? == 0
        && word(b, 112)? == 0
        && word(b, 120)? == 63
        && word(b, 128)? == 0
        && word(b, 136)? == 0
        && word(b, 144)? == 0
        && word(b, 152)? == 3
        && word(b, 176)? == 0
        && word(b, 200)? == 63
        && word(b, 208)? == 63
        && word(b, 216)? == 3
        && word(b, 224)? == 1
        && unknown.is_empty();
    Ok(
        json!({"schema":match version {8=>"todo880-native-submission-observed-trace-v8",7=>"todo880-native-submission-observed-trace-v7",6=>"todo880-native-submission-observed-trace-v6",5=>"todo880-native-submission-observed-trace-v5",_=>"todo880-native-submission-observed-trace-v4"},"pid":pid,"created_filetime":created,
        "sample_cycle_limit":if version==8 {Some(SUBMISSION_BIRTHS)}else{None},
        "sample_limit_sealed":version==8&&word(b,696)?==1,
        "pending_observed_cycles":pending_observed,"unresolved_recorded_t":unresolved_bound_t,
        "native_pending_jobs":null,
        "reserved_unused_pair_starts":unused,"unassigned_diagnostic_rows":extra,
        "entered_producer_serials":serial,"stored_birth_count":count,"normal_no_submit_returns":no_submit,
        "observed_core_epoch":epoch,"reserved_rows":next,"site_witnesses":sites,"rows":rows,"births":births,
        "excluded_calls":word(b,240)?,"excluded_unreadable_current_t":word(b,248)?,
        "accepted_calls_stopped":stopped,"complete_observed_p_t_execution_chains":complete_chains,
        "capture_complete":clean && complete_chains>0,"unknown":unknown,
        "runtime_activated":false,"world_permit_issued":false,
        "scope":if version==8 {"first up to 16 accepted Governor dispatch cycles; new producers after the limit seal are unobserved"}else{"fixed Governor observed invocations only"},
        "not_established_by_this_capture":["World birth generation or World identity","native ownership/retain/release/cancel",
            "T destructor completion or never-executed T recovery","sameSetWait or zero fields do not settle pending T",
            "other jobs and domains were excluded, not drained","natural Close/Rebuild authority"]}),
    )
}
/// Copies made at entry only. Scalar addresses in these records are opaque;
/// this parser never follows them. Correspondence is not job ownership/join.
pub fn submission_trace(b: &[u8], pid: u32, created: u64) -> Result<Value, String> {
    let version = word(b, 8)?;
    if matches!(version, 4..=8) {
        return submission_observed_trace(b, pid, created);
    }
    let (row_bytes, trace_bytes) = match version {
        1 => (SUBMISSION_V1_ROW, SUBMISSION_V1_BYTES),
        2 => (SUBMISSION_V2_ROW, SUBMISSION_V2_BYTES),
        3 => (SUBMISSION_V3_ROW, SUBMISSION_V3_BYTES),
        _ => return Err("submission trace version".into()),
    };
    if b.len() != trace_bytes
        || word(b, 0)? != SUBMISSION_MAGIC
        || word(b, 16)? != trace_bytes as u64
        || word(b, 24)? != ROOT_ROWS as u64
        || word(b, 32)? != row_bytes as u64
        || word(b, 40)? != u64::from(pid)
        || word(b, 48)? != created
        || (if version == 1 {
            28..32
        } else if version == 2 {
            29..32
        } else {
            31..32
        })
        .any(|i| word(b, i * 8).ok() != Some(0))
        || (version >= 2 && word(b, 224)? & !1 != 0)
    {
        return Err("submission trace size/version/identity/reserved mismatch".into());
    }
    let next = usize::try_from(word(b, 80)?).map_err(|_| "submission count")?;
    if next > ROOT_ROWS || next % 2 != 0 {
        return Err("submission reservation count".into());
    }
    let core = word(b, 56)?;
    let epoch = word(b, 72)?;
    let mut unknown = Vec::<String>::new();
    if version == 3 {
        unknown.push(
            "legacy cached target is not a per-invocation callee witness; raw R8 is not typed T"
                .into(),
        );
    }
    let mut sites = Vec::new();
    for (i, (rva, bytes)) in [
        (0x992210, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x6c]),
        (0x992280, [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b]),
    ]
    .into_iter()
    .enumerate()
    {
        let s = (0..8)
            .map(|j| word(b, 256 + i * 64 + j * 8))
            .collect::<Result<Vec<_>, _>>()?;
        let expected = u64::from_le_bytes(bytes);
        if s[..5] != [rva, expected, expected, 1, 1] || s[5..] != [0, 0, 0] {
            unknown.push(format!("submission site {i} witness incomplete"));
        }
        sites.push(s);
    }
    if (0..8).any(|j| word(b, 384 + j * 8).ok() != Some(0)) {
        unknown.push("unused submission site is not zero".into());
    }
    let mut pairs = Vec::<([u64; 35], [u64; 35])>::new();
    let mut rows = Vec::new();
    let mut events = std::collections::BTreeSet::new();
    for i in (0..next).step_by(2) {
        let read = |row: usize| -> Result<[u64; 35], String> {
            let mut r = [0; 35];
            for (j, v) in r.iter_mut().enumerate().take(row_bytes / 8) {
                *v = word(b, ROOT_HEADER + row * row_bytes + j * 8)?;
            }
            Ok(r)
        };
        let a = read(i)?;
        let z = read(i + 1)?;
        for r in [&a, &z] {
            let copied_bytes = |words: &[u64], copied: u64| {
                words
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .take(usize::try_from(copied).unwrap_or(usize::MAX))
                    .collect::<Vec<_>>()
            };
            rows.push(
                json!({"raw":r[..row_bytes/8].to_vec(),"invocation_id":r[3],"kind":r[1],"edge":r[2],
                "tid":r[5],"thread_created_filetime":r[6],"core_epoch":r[7],
                "caller":r[8],"entry_rsp":r[9],"rcx_list":r[10],"rdx_opaque":r[11],
                "r8_set_opaque":r[12],"r9_metadata_opaque":r[13],
                "descriptor_bytes":copied_bytes(&r[14..18],r[24]),
                "metadata_bytes":copied_bytes(&r[18..20],r[26]),
                "snapshot_requested":r[1]==4,
                "status":match r[2] {1=>"entry",2=>"normal_return",3=>"exception_unwind",_=>"unknown"},
                "native_return_present":r[2]==2,
                "errors":r[20],"rax":r[21],"last_error":r[22],"event":r[23],
                "descriptor_copied":r[24],"descriptor_read_error":r[25],
                "metadata_copied":r[26],"metadata_read_error":r[27],"output_rdx":r[28],
                "descriptor_read_raw_bool":r[30],"metadata_read_raw_bool":r[31],
                "native_entry_rsp":r[29],"snapshot_scope":"entry_only_no_pointer_reread"}),
            );
            if version >= 2 {
                let last = rows.last_mut().unwrap();
                last["submit_wrapper_original_caller"] = json!(r[32]);
                last["submit_wrapper_caller_read"] = json!({"copied":r[33]&255,
                    "succeeded":r[33]&256!=0,"win32_error":r[33]>>32});
            }
        }
        if version == 3 {
            for (offset, r) in [&a, &z].into_iter().enumerate() {
                let index = rows.len() - 2 + offset;
                rows[index]["normal_return_r8_raw"] =
                    json!(if r[2] == 2 { Some(r[34]) } else { None });
                rows[index]["normal_return_r8_t"] = Value::Null;
            }
        }
        let valid = a[0] == i as u64 + 1
            && z[0] == i as u64 + 2
            && (a[1] == 4 || a[1] == 5)
            && a[2] == 1
            && z[2] == 2
            && a[3] == i as u64 + 1
            && a[3] == z[3]
            && a[4..21] == z[4..21]
            && a[24..28] == z[24..28]
            && a[1] == z[1]
            && a[29] == z[29]
            && a[5] > 0
            && a[6] > 0
            && a[7] == epoch
            && epoch > 0
            && core > 0
            && a[8] > 0
            && a[9] > 0
            && a[29] > 0
            && a[9].checked_sub(0x1d0) == Some(a[29])
            && a[20] == 0
            && z[20] == 0
            && a[23] > 0
            && z[23] > a[23]
            && z[23] <= word(b, 192)?
            && events.insert(a[23])
            && events.insert(z[23])
            && a[30..34] == z[30..34]
            && a[34] == 0
            && (version != 3 || a[1] != 4 || z[34] > 0)
            && if a[1] == 4 {
                a[30] != 0
                    && a[31] != 0
                    && matches!(a[24], 16 | 32)
                    && (a[24] == 32 || a[16..18] == [0, 0])
                    && a[26] == 16
                    && a[25] == 0
                    && a[27] == 0
            } else {
                a[14..20] == [0; 6] && a[24..28] == [0; 4] && a[30..34] == [0; 4]
            };
        if valid {
            pairs.push((a, z));
        } else {
            unknown.push(format!(
                "invocation {} incomplete/exception/identity/read mismatch",
                i + 1
            ));
        }
    }
    // These keys establish observed caller-stack correspondence, never full
    // frame lifetime. Manager Push subtracts38h and CALL pushes8; Spawn restores
    // its frame before tail-forwarding, so its rawRSP is submit.rawRSP+40h.
    let frame_key = |r: &[u64; 35]| -> Option<(u64, u64, u64, u64)> {
        let rsp = if r[1] == 4 && core.checked_add(0x4a114a) == Some(r[8]) {
            r[9].checked_add(0x40)?
        } else {
            r[9]
        };
        Some((r[5], r[6], r[7], rsp))
    };
    let mut matched = std::collections::BTreeSet::new();
    let mut matched_publications = std::collections::BTreeSet::new();
    let mut manager_matches = 0usize;
    for (publish, publish_return) in pairs.iter().filter(|(a, _)| a[1] == 5) {
        let governor =
            version >= 2 && core.checked_add(0x4a7a58) == Some(publish[8]) && word(b, 224)? == 1;
        let generic = core.checked_add(0x38888e) == Some(publish[8]);
        if (!generic && !governor) || publish[4] != 0 {
            continue;
        }
        let key = frame_key(publish);
        // Nested/interleaved publications cannot own the same observed batch.
        if pairs.iter().any(|(other, end)| {
            other[1] == 5
                && other[3] != publish[3]
                && frame_key(other) == key
                && other[23] < publish_return[23]
                && publish[23] < end[23]
        }) {
            unknown.push(format!(
                "publication {} overlaps another on the same caller stack",
                publish[3]
            ));
            continue;
        }
        let previous = pairs
            .iter()
            .filter(|(a, z)| a[1] == 5 && frame_key(a) == key && z[23] < publish[23])
            .map(|(_, z)| z[23])
            .max()
            .unwrap_or(0);
        let pending: Vec<_> = pairs
            .iter()
            .filter(|(a, z)| {
                a[1] == 4 && frame_key(a) == key && a[23] > previous && z[23] < publish[23]
            })
            .collect();
        // This producer submits exactly once before its next call. Two pending
        // submissions at one stack address are ambiguous, including SP reuse.
        if governor && pending.len() != 1 {
            continue;
        }
        for (submit, _) in pending {
            let origin = if governor {
                core.checked_add(0x4a114a) == Some(submit[8])
                    && core.checked_add(0x4a7a50) == Some(submit[32])
                    && submit[33] == 0x108
                    && submit[9].checked_add(0x28) == Some(submit[13])
                    && core.checked_add(0xa06920) == Some(submit[18])
                    && submit[19] == 0x1f00000000
                    && submit[12] > 0
            } else {
                core.checked_add(0x388871) == Some(submit[8])
            };
            if origin && submit[4] == 0 && submit[10] > 0 && submit[10] == publish[10] {
                matched.insert(submit[3]);
                matched_publications.insert(publish[3]);
                if governor {
                    manager_matches += 1;
                }
            }
        }
    }
    for (a, _) in &pairs {
        if (a[1] == 4 && !matched.contains(&a[3]))
            || (a[1] == 5 && !matched_publications.contains(&a[3]))
        {
            unknown.push(format!(
                "invocation {} has no unambiguous supported submission/publication correspondence",
                a[3]
            ));
        }
    }
    let submitted = pairs.iter().filter(|(a, _)| a[1] == 4).count();
    let stopped = word(b, 96)? == ROOT_CLOSED && word(b, 184)? == 2;
    let clean = stopped
        && (version != 3 || word(b, 232)? == 1)
        && word(b, 88)? == 0
        && word(b, 104)? == 0
        && word(b, 112)? == 0
        && word(b, 120)? == 3
        && word(b, 128)? == 0
        && word(b, 136)? == 0
        && word(b, 144)? == 0
        && word(b, 152)? == 3
        && word(b, 176)? == 0
        && word(b, 200)? == 3
        && word(b, 208)? == 3
        && word(b, 216)? == 1
        && unknown.is_empty();
    Ok(
        json!({"schema":match version {1=>"todo880-native-submission-trace-v1",2=>"todo880-native-submission-trace-v2",_=>"todo880-native-submission-trace-v3"},"pid":pid,
        "created_filetime":created,"core":core,"core_epoch":epoch,"reserved_rows":next,
        "rows":rows,"site_witnesses":sites,"accepted_calls_stopped":stopped,
        "submitted_normal_returns":submitted,"matched_published_submissions":matched.len(),
        "governor_analyzer_matched_submissions":manager_matches,
        "capture_complete":clean && submitted>0 && !matched.is_empty(),"unknown":unknown,
        "runtime_activated":false,"scope":"accepted_two_wrapper_calls_sampling_only",
        "not_established_by_this_capture":["descriptor C++ type and ownership","birth generation ticket",
        "retain/copy/release/cancel/reissue","native publication completion","job execution or Join",
        "World root identity/lifetime or all-domain drainage","runtime activation authority",
        "normal-return R8 is an opaque T observation, not ownership or execution completion",
        "caller-frame lifetime or stack address reuse outside the two observed wrappers",
        "992210 copies R9 metadata16; it forwards RDX unchanged"]}),
    )
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RootConfig {
    pub version: u32,
    pub size: u32,
    pub pid: u32,
    pub max_seconds: u32,
    pub created_filetime: u64,
    pub core: u64,
    pub trace_address: u64,
    pub trace_size: u32,
    pub reserved: u32,
}

#[cfg(test)]
mod submission_protocol_tests {
    use super::*;
    fn put(b: &mut [u8], word: usize, v: u64) {
        b[word * 8..word * 8 + 8].copy_from_slice(&v.to_le_bytes());
    }
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; SUBMISSION_V1_BYTES];
        for (i, v) in [
            SUBMISSION_MAGIC,
            1,
            SUBMISSION_V1_BYTES as u64,
            128,
            256,
            23,
            29,
            0x180000000,
            0x140000000,
            1,
            4,
            0,
            ROOT_CLOSED,
            0,
            0,
            3,
            0,
            0,
            0,
            3,
            0,
            0,
            0,
            2,
            4,
            3,
            3,
            1,
        ]
        .into_iter()
        .enumerate()
        {
            put(&mut b, i, v);
        }
        for (i, (rva, bytes)) in [
            (0x992210, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x6c]),
            (0x992280, [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b]),
        ]
        .into_iter()
        .enumerate()
        {
            for (j, v) in [
                rva,
                u64::from_le_bytes(bytes),
                u64::from_le_bytes(bytes),
                1,
                1,
            ]
            .into_iter()
            .enumerate()
            {
                put(&mut b, 32 + i * 8 + j, v);
            }
        }
        for i in 0..4 {
            let mut r = [0; 32];
            r[0] = i + 1;
            r[1] = if i < 2 { 4 } else { 5 };
            r[2] = if i % 2 == 0 { 1 } else { 2 };
            r[3] = if i < 2 { 1 } else { 3 };
            r[5] = 31;
            r[6] = 37;
            r[7] = 1;
            r[8] = 0x180000000 + if i < 2 { 0x388871 } else { 0x38888e };
            r[9] = 0x1000;
            r[10] = 7;
            if i < 2 {
                r[11] = 0x10000;
                r[12] = 0x20000;
                r[13] = 0x30000;
                r[14] = 0xaabb;
                r[18] = 0xccdd;
                r[24] = 32;
                r[26] = 16;
                r[30] = 1;
                r[31] = 1;
            }
            r[23] = i + 1;
            r[29] = 0xe30;
            for (j, v) in r.into_iter().enumerate() {
                put(&mut b, ROOT_HEADER / 8 + i as usize * 32 + j, v);
            }
        }
        b
    }
    #[test]
    fn submission_fixed_caller_pairs_are_sampling_only_and_root_rejects_them() {
        let b = fixture();
        let v = submission_trace(&b, 23, 29).unwrap();
        assert_eq!(v["capture_complete"], true);
        assert_eq!(v["matched_published_submissions"], 1);
        assert_eq!(v["runtime_activated"], false);
        assert!(root_trace(&b, 23, 29).is_err());
        assert!(
            v["not_established_by_this_capture"]
                .as_array()
                .unwrap()
                .len()
                > 6
        );
    }
    fn v2(b: &[u8]) -> Vec<u8> {
        let mut out = vec![0; SUBMISSION_V2_BYTES];
        out[..ROOT_HEADER].copy_from_slice(&b[..ROOT_HEADER]);
        put(&mut out, 1, 2);
        put(&mut out, 2, SUBMISSION_V2_BYTES as u64);
        put(&mut out, 4, SUBMISSION_V2_ROW as u64);
        put(&mut out, 28, 1);
        let next = word(b, 80).unwrap() as usize;
        for row in 0..next {
            out[ROOT_HEADER + row * SUBMISSION_V2_ROW
                ..ROOT_HEADER + row * SUBMISSION_V2_ROW + SUBMISSION_V1_ROW]
                .copy_from_slice(
                    &b[ROOT_HEADER + row * SUBMISSION_V1_ROW
                        ..ROOT_HEADER + (row + 1) * SUBMISSION_V1_ROW],
                );
        }
        out
    }
    fn governor() -> Vec<u8> {
        let mut b = v2(&fixture());
        for row in 0..4 {
            let p = ROOT_HEADER / 8 + row * (SUBMISSION_V2_ROW / 8);
            let submit = row < 2;
            let rsp = if submit { 0x1000 } else { 0x1040 };
            put(
                &mut b,
                p + 8,
                0x180000000 + if submit { 0x4a114a } else { 0x4a7a58 },
            );
            put(&mut b, p + 9, rsp);
            put(&mut b, p + 29, rsp - 0x1d0);
            if submit {
                for (field, value) in [
                    (13, rsp + 0x28),
                    (18, 0x180a06920),
                    (19, 0x1f00000000),
                    (32, 0x1804a7a50),
                    (33, 0x108),
                ] {
                    put(&mut b, p + field, value);
                }
            }
        }
        b
    }
    fn sequence(kinds: &[bool]) -> Vec<u8> {
        let template = governor();
        let mut b = template.clone();
        for (i, &publish) in kinds.iter().enumerate() {
            for edge in 0..2 {
                let source =
                    ROOT_HEADER + ((if publish { 2 } else { 0 }) + edge) * SUBMISSION_V2_ROW;
                let row = i * 2 + edge;
                let dest = ROOT_HEADER + row * SUBMISSION_V2_ROW;
                b[dest..dest + SUBMISSION_V2_ROW]
                    .copy_from_slice(&template[source..source + SUBMISSION_V2_ROW]);
                for (field, value) in [
                    (0, row as u64 + 1),
                    (3, (i * 2 + 1) as u64),
                    (23, row as u64 + 1),
                ] {
                    put(&mut b, dest / 8 + field, value);
                }
            }
        }
        put(&mut b, 10, (kinds.len() * 2) as u64);
        put(&mut b, 24, (kinds.len() * 2) as u64);
        b
    }
    #[test]
    fn submission_governor_fixed_parent_metadata_and_exact_wrapper_rsp_match() {
        let v = submission_trace(&governor(), 23, 29).unwrap();
        assert_eq!(v["capture_complete"], true);
        assert_eq!(v["governor_analyzer_matched_submissions"], 1);
        assert_eq!(v["schema"], "todo880-native-submission-trace-v2");
        let old = submission_trace(&fixture(), 23, 29).unwrap();
        assert_eq!(old["capture_complete"], true);
        assert_eq!(old["rows"][0]["raw"].as_array().unwrap().len(), 32);
        assert_eq!(
            submission_trace(&v2(&fixture()), 23, 29).unwrap()["capture_complete"],
            true
        );
    }
    #[test]
    fn submission_r8_normal_return_is_preserved_without_exit_heap_reads_or_world_certificate() {
        let old = governor();
        let mut b = vec![0; SUBMISSION_V3_BYTES];
        b[..ROOT_HEADER].copy_from_slice(&old[..ROOT_HEADER]);
        put(&mut b, 1, 3);
        put(&mut b, 2, SUBMISSION_V3_BYTES as u64);
        put(&mut b, 4, SUBMISSION_V3_ROW as u64);
        put(&mut b, 29, 1);
        for i in 0..4 {
            b[ROOT_HEADER + i * SUBMISSION_V3_ROW
                ..ROOT_HEADER + i * SUBMISSION_V3_ROW + SUBMISSION_V2_ROW]
                .copy_from_slice(
                    &old[ROOT_HEADER + i * SUBMISSION_V2_ROW
                        ..ROOT_HEADER + (i + 1) * SUBMISSION_V2_ROW],
                );
        }
        let slot = ROOT_HEADER / 8 + SUBMISSION_V3_ROW / 8 + 34;
        put(&mut b, slot, 0x11223344);
        let v = submission_trace(&b, 23, 29).unwrap();
        assert_eq!(v["capture_complete"], false);
        assert!(v["rows"][1]["normal_return_r8_t"].is_null());
        assert_eq!(v["rows"][1]["normal_return_r8_raw"], 0x11223344_u64);
        assert!(v["rows"][0]["normal_return_r8_t"].is_null());
        assert_eq!(v["runtime_activated"], false);
        put(&mut b, slot, 0);
        assert_eq!(
            submission_trace(&b, 23, 29).unwrap()["capture_complete"],
            false
        );
        put(&mut b, slot, 0x11223344);
        put(&mut b, 29, 0);
        let unknown = submission_trace(&b, 23, 29).unwrap();
        assert!(unknown["rows"][1]["normal_return_r8_t"].is_null());
        assert_eq!(unknown["rows"][1]["normal_return_r8_raw"], 0x11223344_u64);
        assert_eq!(
            submission_trace(&b, 23, 29).unwrap()["capture_complete"],
            false
        );
    }
    #[test]
    fn submission_governor_wrong_parent_caller_rsp_and_metadata_never_match() {
        for (row, field, value) in [
            (0, 32, 0x1804a7a51),
            (0, 33, 0),
            (0, 33, 0x107),
            (0, 33, (299 << 32) | 0x108),
            (0, 13, 0x1020),
            (0, 18, 0x180a06921),
            (0, 19, 31),
            (2, 8, 0x1804a7a59),
            (2, 9, 0x1038),
            (0, 8, 0x1804a114b),
            (2, 6, 999),
            (2, 7, 2),
        ] {
            let mut b = governor();
            for r in row..row + 2 {
                put(
                    &mut b,
                    ROOT_HEADER / 8 + r * (SUBMISSION_V2_ROW / 8) + field,
                    value,
                );
            }
            let v = submission_trace(&b, 23, 29).unwrap();
            assert_eq!(v["capture_complete"], false, "row{row} field{field}");
            assert!(!v["unknown"].as_array().unwrap().is_empty());
        }
        let mut b = governor();
        put(&mut b, 28, 0);
        assert_eq!(
            submission_trace(&b, 23, 29).unwrap()["capture_complete"],
            false
        );
    }
    #[test]
    fn submission_governor_multiple_publications_need_new_submit_and_sp_reuse_is_ambiguous() {
        assert_eq!(
            submission_trace(&sequence(&[false, true, false, true]), 23, 29).unwrap()
                ["capture_complete"],
            true
        );
        for kinds in [&[false, true, true][..], &[false, false, true][..]] {
            let v = submission_trace(&sequence(kinds), 23, 29).unwrap();
            assert_eq!(v["capture_complete"], false);
            assert!(!v["unknown"].as_array().unwrap().is_empty());
        }
    }
    #[test]
    fn submission_governor_interleaved_distinct_frames_match_but_overlapping_publications_do_not() {
        let mut b = sequence(&[false, false, true, true]);
        for (row, rsp) in [(2, 0x2000), (3, 0x2000), (4, 0x2040), (5, 0x2040)] {
            let p = ROOT_HEADER / 8 + row * (SUBMISSION_V2_ROW / 8);
            put(&mut b, p + 9, rsp);
            put(&mut b, p + 29, rsp - 0x1d0);
            if row < 4 {
                put(&mut b, p + 13, rsp + 0x28);
            }
        }
        assert_eq!(
            submission_trace(&b, 23, 29).unwrap()["capture_complete"],
            true
        );
        let mut b = sequence(&[false, true, true]);
        for (row, event) in [(2, 3), (3, 6), (4, 4), (5, 5)] {
            put(
                &mut b,
                ROOT_HEADER / 8 + row * (SUBMISSION_V2_ROW / 8) + 23,
                event,
            );
        }
        assert_eq!(
            submission_trace(&b, 23, 29).unwrap()["capture_complete"],
            false
        );
    }
    #[test]
    fn submission_governor_v1_unwitnessed_wrapper_or_unmatched_submit_stays_unknown() {
        let mut b = fixture();
        for row in 0..4 {
            put(
                &mut b,
                ROOT_HEADER / 8 + row * 32 + 8,
                0x180000000 + if row < 2 { 0x4a114a } else { 0x4a7a58 },
            );
        }
        assert_eq!(
            submission_trace(&b, 23, 29).unwrap()["capture_complete"],
            false
        );
        let mut b = governor();
        put(&mut b, 10, 2);
        put(&mut b, 24, 2);
        assert_eq!(
            submission_trace(&b, 23, 29).unwrap()["capture_complete"],
            false
        );
    }
    #[test]
    fn submission_wrong_caller_rsp_fte_epoch_overlap_and_event_order_never_complete() {
        for (row, field, value) in [
            (2, 8, 0x18038888f),
            (2, 9, 0x1008),
            (3, 6, 38),
            (3, 7, 2),
            (0, 7, 2),
            (1, 7, 2),
            (0, 4, 77),
            (1, 4, 77),
            (2, 23, 1),
            (3, 2, 3),
        ] {
            let mut b = fixture();
            put(&mut b, ROOT_HEADER / 8 + row * 32 + field, value);
            assert_eq!(
                submission_trace(&b, 23, 29).unwrap()["capture_complete"],
                false,
                "row{row} field{field}"
            );
        }
    }
    #[test]
    fn submission_empty_partial_dropped_read_errors_and_site_failure_never_complete() {
        for (offset, value) in [
            (10, 0),
            (11, 1),
            (13, 1),
            (15, 1),
            (16, 1),
            (19, 2),
            (25, 1),
            (26, 1),
            (27, 0),
            (ROOT_HEADER / 8 + 24, 15),
            (ROOT_HEADER / 8 + 24, 17),
            (ROOT_HEADER / 8 + 25, 299),
            (ROOT_HEADER / 8 + 26, 0),
            (ROOT_HEADER / 8 + 30, 0),
            (ROOT_HEADER / 8 + 31, 0),
            (32 + 4, 0),
            (32 + 5, 1),
            (40 + 7, 1),
        ] {
            let mut b = fixture();
            put(&mut b, offset, value);
            assert_eq!(
                submission_trace(&b, 23, 29).unwrap()["capture_complete"],
                false,
                "word{offset}"
            );
        }
        let mut b = fixture();
        b.pop();
        assert!(submission_trace(&b, 23, 29).is_err());
    }
    #[test]
    fn submission_entry_snapshot_changes_at_return_are_rejected() {
        for field in [10, 11, 12, 13, 14, 18, 24, 26, 29] {
            let mut b = fixture();
            put(&mut b, ROOT_HEADER / 8 + 32 + field, 999);
            assert_eq!(
                submission_trace(&b, 23, 29).unwrap()["capture_complete"],
                false
            );
        }
    }
    #[test]
    fn submission_return_copy_counts_and_read_errors_are_frozen_but_output_rdx_is_not() {
        assert_eq!(
            submission_trace(&fixture(), 23, 29).unwrap()["capture_complete"],
            true
        );
        for row in [1usize, 3] {
            for (field, value) in [
                (24, if row == 1 { 0 } else { 1 }),
                (25, 299),
                (26, if row == 1 { 0 } else { 1 }),
                (27, 299),
            ] {
                let mut b = fixture();
                put(&mut b, ROOT_HEADER / 8 + row * 32 + field, value);
                let trace = submission_trace(&b, 23, 29).unwrap();
                assert_eq!(
                    trace["capture_complete"], false,
                    "return row{row} field{field}"
                );
                assert!(!trace["unknown"].as_array().unwrap().is_empty());
                assert_eq!(
                    trace["rows"][row]["raw"][field], value,
                    "original changed bytes remain visible"
                );
            }
        }
        let mut b = fixture();
        for row in [1usize, 3] {
            put(&mut b, ROOT_HEADER / 8 + row * 32 + 28, 0x11223344);
        }
        let trace = submission_trace(&b, 23, 29).unwrap();
        assert_eq!(
            trace["capture_complete"], true,
            "native output RDX is not the frozen input copy"
        );
        assert_eq!(trace["rows"][1]["output_rdx"], 0x11223344_u64);
    }
    #[test]
    fn submission_selector_is_exact_and_requires_explicit_controller_lifetime() {
        for value in [
            json!(null),
            json!(true),
            json!("submission"),
            json!("native_submission "),
        ] {
            assert!(
                Request::root_scenario(&json!({"observation":value,"controller_seconds":60}))
                    .is_err()
            );
        }
        assert!(Request::root_scenario(&json!({"observation":"native_submission"})).is_err());
        assert!(Request::root_scenario(
            &json!({"observation":"native_submission","controller_seconds":60})
        )
        .unwrap());
        assert!(!Request::root_scenario(&json!({})).unwrap());
        let v = scenario_request(
            &json!({"observation":"native_submission","controller_seconds":60}),
            23,
            29,
            std::path::Path::new("capture"),
        );
        assert_eq!(v["schema"], "todo880-native-submission-controller-v1");
    }
}

/// Layout/record correspondence only. A normal wrapper return is not a join
/// certificate; equal copies must be reported by the controller separately.
pub fn root_trace(b: &[u8], pid: u32, created: u64) -> Result<Value, String> {
    if b.len() != ROOT_BYTES
        || word(b, 0)? != ROOT_MAGIC
        || word(b, 8)? != ROOT_VERSION
        || word(b, 16)? != ROOT_BYTES as u64
        || word(b, 24)? != ROOT_ROWS as u64
        || word(b, 32)? != ROOT_ROW as u64
        || word(b, 40)? != pid as u64
        || word(b, 48)? != created
        || (25..32).any(|i| word(b, i * 8).ok() != Some(0))
    {
        return Err("root trace size/version/identity/reserved mismatch".into());
    }
    let next = usize::try_from(word(b, 80)?).map_err(|_| "root count")?;
    if next > ROOT_ROWS || next % 2 != 0 {
        return Err("root trace reservation count invalid".into());
    }
    let mut rows = Vec::new();
    let mut pairs = std::collections::BTreeMap::new();
    let mut unknown = Vec::new();
    let site_witnesses = (0..3)
        .map(|i| {
            (0..8)
                .map(|j| word(b, 256 + i * 64 + j * 8))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_sites = [
        (0x63e160, [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x80, 0xb9]),
        (0x4a1150, [0x48, 0x89, 0x5c, 0x24, 0x10, 0x56, 0x48, 0x83]),
        (0x9922c0, [0x48, 0x89, 0x5c, 0x24, 0x08, 0x57, 0x48, 0x83]),
    ];
    for (index, ((rva, bytes), witness)) in expected_sites.iter().zip(&site_witnesses).enumerate() {
        let expected = u64::from_le_bytes(*bytes);
        if witness[0] != *rva
            || witness[1] != expected
            || witness[2] != expected
            || witness[3] != 1
            || witness[4] != 1
            || witness[5] != 0
            || witness[6] != 0
            || witness[7] != 0
        {
            unknown.push(format!(
                "site {index} lacks verified fixed bytes/publication"
            ));
        }
    }
    if word(b, 152)? != 3 {
        unknown.push("root installation did not complete all three sites".into());
    }
    let mut events = std::collections::BTreeSet::new();
    for i in 0..next {
        let p = ROOT_HEADER + i * ROOT_ROW;
        let w = |j: usize| word(b, p + j * 8);
        if w(0)? != i as u64 + 1 {
            unknown.push(format!("uncommitted row {i}"));
            continue;
        }
        let kind = w(1)?;
        let edge = w(2)?;
        let id = w(3)?;
        if !(1..=3).contains(&kind)
            || !(1..=3).contains(&edge)
            || id == 0
            || id % 2 != 1
            || id > next as u64
            || w(23)? == 0
            || w(23)? > word(b, 192)?
            || !events.insert(w(23)?)
            || (edge == 1 && id != i as u64 + 1)
            || (edge != 1 && id != i as u64)
        {
            return Err("root row kind/edge/reservation invalid".into());
        }
        let raw: Vec<u64> = (0..24).map(w).collect::<Result<_, _>>()?;
        pairs.entry(id).or_insert_with(Vec::new).push(raw.clone());
        rows.push(json!({"sequence":raw[0],"kind":kind,"edge":edge,
            "invocation":id,"parent":raw[4],"tid":raw[5],"thread_created_filetime":raw[6],
            "core_epoch":raw[7],"original_caller":raw[8],"gateway_entry_rsp":raw[9],
            "world":raw[10],"manager":raw[11],"set_address":raw[12],"list_id":raw[13],
            "set_words":[raw[14],raw[15]],"metadata_words":[raw[16],raw[17]],
            "original_rax":raw[18],"read_or_binding_errors":raw[19],"last_error":raw[20],
            "native_entry_rsp":raw[21],"entry_rbx":raw[22],"event_sequence":raw[23]}));
    }
    let core = word(b, 56)?;
    let epoch = word(b, 72)?;
    let mut valid_pairs = std::collections::BTreeSet::new();
    let mut wait_pairs = 0usize;
    let mut local_wait_pairs = 0usize;
    for (id, pair) in &pairs {
        if pair.len() != 2 || pair[0][2] != 1 || pair[1][2] != 2 {
            unknown.push(format!("invocation {id} has no normal return"));
            continue;
        }
        let a = &pair[0];
        let z = &pair[1];
        if [1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 21, 22]
            .into_iter()
            .any(|j| a[j] != z[j])
            || a[5] == 0
            || a[6] == 0
            || epoch == 0
            || a[7] != epoch
            || a[19] != 0
            || z[19] != 0
            || a[23] >= z[23]
        {
            unknown.push(format!("invocation {id} binding/read mismatch"));
            continue;
        }
        valid_pairs.insert(*id);
    }
    // Validate each manager's scope before any Wait can borrow it. A local
    // manager is specifically the second fixed caller; world=0 never permits
    // manufacturing a parent world from a worker's RDI or matching pointers.
    let mut manager_scopes = std::collections::BTreeMap::new();
    let mut matched_managers = std::collections::BTreeSet::new();
    let primary_caller = core.checked_add(0x63e1d5).ok_or("core overflow")?;
    let local_caller = core.checked_add(0x63d448).ok_or("core overflow")?;
    let wait_caller = core.checked_add(0x4a1196).ok_or("core overflow")?;
    for id in &valid_pairs {
        let pair = &pairs[id];
        let m = &pair[0];
        let end = &pair[1];
        if m[1] != 2 {
            continue;
        }
        let local = m[8] == local_caller;
        let receiver = m[11] != 0 && m[12].checked_sub(0x400) == Some(m[11]);
        let valid = if local {
            receiver && m[4] == 0 && m[10] == 0
        } else if m[8] == primary_caller {
            pairs
                .get(&m[4])
                .filter(|_| valid_pairs.contains(&m[4]))
                .is_some_and(|world| {
                    let r = &world[0];
                    let exit = &world[1];
                    receiver
                        && r[1] == 1
                        && r[10] != 0
                        && r[4] == 0
                        && m[22] == r[10]
                        && m[9].checked_add(0x30) == Some(r[21])
                        && [5, 6, 7, 10].into_iter().all(|j| m[j] == r[j])
                        && r[23] < m[23]
                        && end[23] < exit[23]
                })
        } else {
            false
        };
        if valid {
            manager_scopes.insert(*id, local);
        } else {
            unknown.push(format!("manager {id} exact caller/scope mismatch"));
        }
    }
    // A parent's entry cannot supply authority when its own return belongs
    // to another thread lifetime, epoch or binding.
    for id in &valid_pairs {
        let pair = &pairs[id];
        let a = &pair[0];
        let z = &pair[1];
        if a[1] == 3 {
            let Some(parent) = pairs.get(&a[4]).filter(|_| valid_pairs.contains(&a[4])) else {
                unknown.push(format!("wait {id} has no complete manager"));
                continue;
            };
            let m = &parent[0];
            let Some(local) = manager_scopes.get(&a[4]) else {
                unknown.push(format!("wait {id} has no manager with a valid scope"));
                continue;
            };
            // Reservation row order is not execution order.
            let binding = m[1] == 2
                && parent[1][2] == 2
                && m[19] == 0
                && parent[1][19] == 0
                && a[8] == wait_caller
                && a[12].checked_sub(0x400) == Some(m[11])
                && a[9].checked_add(0x40) == Some(m[21])
                && a[22] == m[11]
                && [5, 6, 7, 10, 11, 12].into_iter().all(|j| a[j] == m[j])
                && m[23] < a[23]
                && a[23] < z[23]
                && z[23] < parent[1][23]
                && (!*local || (m[4] == 0 && m[10] == 0 && a[10] == 0));
            if binding {
                matched_managers.insert(a[4]);
                if *local {
                    local_wait_pairs += 1;
                } else {
                    wait_pairs += 1;
                }
            } else {
                unknown.push(format!("wait {id} exact parent mismatch"));
            }
        }
    }
    for id in manager_scopes.keys() {
        if !matched_managers.contains(id) {
            unknown.push(format!("manager {id} has no exact normal-return Wait pair"));
        }
    }
    // Keep the v1 byte geometry and existing world counter. Additional scope
    // fields never reinterpret an orphan Wait from an older capture.
    for row in &mut rows {
        let kind = row["kind"].as_u64().unwrap();
        let id = row["invocation"].as_u64().unwrap();
        let manager = if kind == 2 {
            id
        } else {
            row["parent"].as_u64().unwrap()
        };
        row["scope"] = json!(if kind == 1 {
            "world_shutdown"
        } else {
            match manager_scopes.get(&manager) {
                Some(true) => "local_manager",
                Some(false) => "world_shutdown",
                None => "unknown",
            }
        });
    }
    let stopped = word(b, 184)? == 2 && word(b, 96)? == ROOT_CLOSED;
    let clean = stopped
        && word(b, 88)? == 0
        && word(b, 104)? == 0
        && word(b, 112)? == 0
        && word(b, 120)? == 7
        && word(b, 128)? == 0
        && word(b, 136)? == 0
        && word(b, 144)? == 0
        && word(b, 176)? == 0
        && unknown.is_empty();
    Ok(
        json!({"schema":"todo880-root-observation-trace-v1","pid":pid,
        "created_filetime":created,"core":core,"core_epoch":word(b,72)?,
        "reserved_rows":next,"rows":rows,"accepted_calls_stopped":stopped,
        "same_manager_wait_normal_returns":wait_pairs,
        "local_manager_normal_returns":manager_scopes.values().filter(|local|**local).count(),
        "local_manager_wait_normal_returns":local_wait_pairs,
        "capture_complete":clean && wait_pairs > 0,"unknown":unknown,
        "site_witnesses":site_witnesses,
        "not_established_by_this_capture":["native producer admission seal","pending publication or cancellation",
            "rePush absence","host indirect target and arena join path","all six other domains exited",
            "synchronous4A7AD0/5E2D0 and higher callers exited","world lifetime","runtime activation authority",
            "local manager caller continues at63D448 through another call and conditional49C7C0; no world exit certificate"]}),
    )
}
pub struct Request {
    /// Explicit capture schemas only; ordinary requests never enable a shim.
    pub root_only: bool,
    pub submission_only: bool,
    pub pid: u32,
    pub created: u64,
    pub seconds: u32,
    /// Owning controller lifetime; native telemetry still uses `seconds`.
    pub controller_seconds: u32,
    pub runtime: PathBuf,
    pub runtime_sha: String,
    pub bootstrap: PathBuf,
    pub bootstrap_sha: String,
    pub output: PathBuf,
}
pub fn digest(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("SHA256 must have 64 hex characters".into());
    }
    let mut b = [0; 32];
    for (i, v) in b.iter_mut().enumerate() {
        *v = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).map_err(|_| "SHA256")?;
    }
    Ok(b)
}
impl Request {
    pub fn root_scenario(step: &Value) -> Result<bool, String> {
        match step.get("observation") {
            None => Ok(false),
            Some(Value::String(value))
                if value == "native_shutdown" || value == "native_submission" =>
            {
                if step.get("controller_seconds").is_none() {
                    return Err("root observation requires explicit controller_seconds".into());
                }
                Ok(true)
            }
            _ => Err("unknown bootstrap observation selector".into()),
        }
    }
    pub fn durations(v: &Value) -> Result<(u32, u32), String> {
        let seconds = v["max_seconds"]
            .as_u64()
            .filter(|n| (1..=300).contains(n))
            .ok_or("max_seconds must be 1..300")? as u32;
        let controller = match v.get("controller_seconds") {
            None => seconds,
            Some(value) => value
                .as_u64()
                .filter(|n| *n >= u64::from(seconds) && *n <= 1800)
                .ok_or("controller_seconds must cover telemetry and be at most 1800")?
                as u32,
        };
        Ok((seconds, controller))
    }

    pub fn parse(v: &Value) -> Result<Self, String> {
        let map = v.as_object().ok_or("request object required")?;
        let fields = [
            "schema",
            "pid",
            "created_filetime",
            "max_seconds",
            "runtime_dll",
            "runtime_sha256",
            "bootstrap_dll",
            "bootstrap_sha256",
            "output",
            "controller_seconds",
        ];
        let fields = match v["schema"].as_str() {
            Some("todo880-bootstrap-controller-v2") => &fields[..9],
            Some("todo880-bootstrap-controller-v3") => &fields[..],
            Some("todo880-root-observation-controller-v1") => &fields[..],
            Some("todo880-native-submission-controller-v1") => &fields[..],
            _ => return Err("bootstrap request schema/fields".into()),
        };
        if map.len() != fields.len() || map.keys().any(|k| !fields.contains(&k.as_str())) {
            return Err("bootstrap request schema/fields".into());
        }
        let num = |k: &str| {
            v[k].as_u64()
                .filter(|n| *n > 0)
                .ok_or_else(|| format!("invalid {k}"))
        };
        let file = |k: &str| -> Result<PathBuf, String> {
            let p = PathBuf::from(v[k].as_str().ok_or(k)?);
            if !p.is_absolute() {
                return Err(format!("{k} must be absolute"));
            }
            Ok(p)
        };
        let sha = |k: &str| -> Result<String, String> {
            let s = v[k].as_str().ok_or(k)?;
            digest(s)?;
            Ok(s.to_ascii_lowercase())
        };
        let (seconds, controller_seconds) = Self::durations(v)?;
        Ok(Self {
            root_only: v["schema"] == "todo880-root-observation-controller-v1"
                || v["schema"] == "todo880-native-submission-controller-v1",
            submission_only: v["schema"] == "todo880-native-submission-controller-v1",
            pid: u32::try_from(num("pid")?).map_err(|_| "PID overflow")?,
            created: num("created_filetime")?,
            seconds,
            controller_seconds,
            runtime: file("runtime_dll")?,
            runtime_sha: sha("runtime_sha256")?,
            bootstrap: file("bootstrap_dll")?,
            bootstrap_sha: sha("bootstrap_sha256")?,
            output: file("output")?,
        })
    }
}

/// One schema builder for offline validation and the actual controller child.
pub fn scenario_request(step: &Value, pid: u32, created: u64, output: &std::path::Path) -> Value {
    let mut value = json!({"schema":"todo880-bootstrap-controller-v2","pid":pid,
        "created_filetime":created,"max_seconds":step["max_seconds"],
        "runtime_dll":step["runtime_dll"],"runtime_sha256":step["runtime_sha256"],
        "bootstrap_dll":step["bootstrap_dll"],"bootstrap_sha256":step["bootstrap_sha256"],
        "output":output});
    if let Some(controller) = step.get("controller_seconds") {
        value["schema"] = json!("todo880-bootstrap-controller-v3");
        value["controller_seconds"] = controller.clone();
    }
    if step["observation"] == "native_shutdown" {
        value["schema"] = json!("todo880-root-observation-controller-v1");
    }
    if step["observation"] == "native_submission" {
        value["schema"] = json!("todo880-native-submission-controller-v1");
    }
    value
}
pub fn word(b: &[u8], offset: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        b.get(offset..offset + 8)
            .ok_or("trace range")?
            .try_into()
            .unwrap(),
    ))
}
pub fn trace(b: &[u8], pid: u32, created: u64) -> Result<Value, String> {
    if b.len() != TRACE_BYTES
        || word(b, 0)? != 2
        || word(b, 8)? != CAPACITY as u64
        || word(b, 16)? != pid as u64
        || word(b, 24)? != created
        || word(b, 64)? != 0
    {
        return Err("trace version/identity/capacity/drop mismatch".into());
    }
    let count = usize::try_from(word(b, 56)?).map_err(|_| "trace count")?;
    if count == 0 || count > CAPACITY {
        return Err("trace empty or exhausted".into());
    }
    let fields = [
        "sequence",
        "kind",
        "tid",
        "receiver",
        "old",
        "incoming",
        "caller",
        "generation",
        "status",
        "calibration_next",
        "calibration_in_flight",
        "property_next",
        "property_in_flight",
    ];
    let mut rows = Vec::new();
    let mut prepared = false;
    let mut installed = false;
    for i in 0..count {
        let p = HEADER + i * ROW;
        if word(b, p)? != i as u64 + 1 {
            return Err("uncommitted/out-of-order trace row".into());
        }
        let kind = word(b, p + 8)?;
        if kind > 15 || kind == 14 {
            return Err("unknown trace kind".into());
        }
        if kind == 11 {
            prepared = true;
        }
        if [1, 3, 7].contains(&kind) && word(b, p + 64)? != 0 {
            return Err("runtime API failure".into());
        }
        if kind == 1 {
            installed = true;
        }
        if installed && (9..13).any(|j| word(b, p + j * 8).ok() == Some(u64::MAX)) {
            return Err("runtime checkpoint unknown".into());
        }
        let mut row = json!({});
        for (j, name) in fields.iter().enumerate() {
            row[*name] = json!(word(b, p + j * 8)?);
        }
        rows.push(row);
    }
    if !prepared || !installed || word(b, 32)? == 0 || word(b, 40)? == 0 || word(b, 48)? == 0 {
        return Err("prepared-only is not an installed observation".into());
    }
    Ok(
        json!({"schema":"todo880-bootstrap-trace-v2","pid":pid,"created_filetime":created,"core":word(b,32)?,"runtime_trace":word(b,40)?,"runtime_trace_size":word(b,48)?,"rows":rows,"frame_valid":true,"boundary_order_proven":false,"product_acceptance":false}),
    )
}
/// Version of the expanded, still bounded calibration prefix.
pub const CALIBRATION_VERSION: u32 = 5;
/// Six complete 22-owner/22-person matrices need 2,904 samples before
/// natural engine queries. This bounded prefix leaves room for those calls.
pub const CALIBRATION_CAPACITY: usize = 8192;
/// Offset of the PropertyTrace inside the current public CompositeTrace.
pub const COMPOSITE_PROPERTY: usize = 24 + CALIBRATION_CAPACITY * 72;

/// The version fixes the prefix geometry; never infer it from zero padding.
/// Legacy D21 captures keep their original 1,024-row layout.
pub fn calibration_layout(b: &[u8]) -> Result<(u32, usize, usize), String> {
    if u32_at(b, 0)? != 0x4a414341 {
        return Err("calibration magic differs".into());
    }
    let version = u32_at(b, 4)?;
    let capacity = match version {
        4 => 1024,
        CALIBRATION_VERSION => CALIBRATION_CAPACITY,
        _ => return Err("calibration version unknown".into()),
    };
    Ok((version, capacity, 24 + capacity * 72))
}
const PROPERTY_PREFIX_BYTES: usize = 32 + 4096 * 48;
/// Prefix PropertyTrace version meaning "rows are in the runtime property log".
/// Version 3 parsers reject it, so an empty prefix never passes alone.
pub const PROPERTY_EXTERNAL_VERSION: u32 = 4;
/// Runtime-owned raw property log, allocated once at runtime installation.
pub const PROPERTY_LOG_MAGIC: u32 = 0x4a41_434c; // JACL
pub const PROPERTY_LOG_VERSION: u32 = 1;
pub const PROPERTY_LOG_HEADER: usize = 80;
pub const PROPERTY_LOG_STRIDE: usize = 48;
/// Finite diagnostic budget: 2^20 rows (48 MiB), 8.8x the 119,451 accepted
/// property calls of live run 175. Overflow is counted and fails validation.
pub const PROPERTY_LOG_CAPACITY: usize = 1 << 20;
pub const PROPERTY_LOG_BYTES: usize =
    PROPERTY_LOG_HEADER + PROPERTY_LOG_CAPACITY * PROPERTY_LOG_STRIDE;
// Header: magic u32, version u32, then u64 capacity, stride, total bytes,
// PID, creation FILETIME, next, dropped, in_flight, filtered.
pub const PROPERTY_LOG_NEXT: usize = 48;
pub const PROPERTY_LOG_DROPPED: usize = 56;
pub const PROPERTY_LOG_IN_FLIGHT: usize = 64;
pub const PROPERTY_LOG_FILTERED: usize = 72;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyLog {
    pub rows: u64,
    pub filtered: u64,
}

fn u32_at(b: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        b.get(offset..offset + 4)
            .ok_or("trace range")?
            .try_into()
            .unwrap(),
    ))
}

/// Complete runtime property log: exact identity and capacity, every reserved
/// row published, the untouched tail still zero, no drop and no writer left.
pub fn property_log(b: &[u8], pid: u32, created: u64) -> Result<PropertyLog, String> {
    if b.len() != PROPERTY_LOG_BYTES
        || u32_at(b, 0)? != PROPERTY_LOG_MAGIC
        || u32_at(b, 4)? != PROPERTY_LOG_VERSION
        || word(b, 8)? != PROPERTY_LOG_CAPACITY as u64
        || word(b, 16)? != PROPERTY_LOG_STRIDE as u64
        || word(b, 24)? != PROPERTY_LOG_BYTES as u64
    {
        return Err("property log version/capacity/stride/size mismatch".into());
    }
    if word(b, 32)? != u64::from(pid) || word(b, 40)? != created {
        return Err("property log PID/FILETIME differs".into());
    }
    let next = word(b, PROPERTY_LOG_NEXT)?;
    let dropped = word(b, PROPERTY_LOG_DROPPED)?;
    let in_flight = word(b, PROPERTY_LOG_IN_FLIGHT)?;
    if dropped != 0 || in_flight != 0 || next > PROPERTY_LOG_CAPACITY as u64 {
        return Err(format!(
            "property log incomplete: next={next} dropped={dropped} in_flight={in_flight}"
        ));
    }
    let end = PROPERTY_LOG_HEADER + next as usize * PROPERTY_LOG_STRIDE;
    if (PROPERTY_LOG_HEADER..end)
        .step_by(PROPERTY_LOG_STRIDE)
        .any(|row| b[row..row + 4] != 1u32.to_le_bytes())
    {
        return Err("property log row unpublished".into());
    }
    if b[end..].iter().any(|byte| *byte != 0) {
        return Err("property log has bytes beyond its published rows".into());
    }
    Ok(PropertyLog {
        rows: next,
        filtered: word(b, PROPERTY_LOG_FILTERED)?,
    })
}

/// Runtime-owned property observation: the prefix must carry the external
/// mark and stay empty, and the separately captured log must be complete.
pub fn runtime_property(
    composite: &[u8],
    log: &[u8],
    pid: u32,
    created: u64,
) -> Result<PropertyLog, String> {
    let (_, _, property) = calibration_layout(composite)?;
    let prefix = composite
        .get(property..property + PROPERTY_PREFIX_BYTES)
        .ok_or("native trace truncated")?;
    if u32_at(prefix, 0)? != 0x4a414350
        || u32_at(prefix, 4)? != PROPERTY_EXTERNAL_VERSION
        || u32_at(prefix, 24)? != 6
        || u32_at(prefix, 28)? != PROPERTY_LOG_MAGIC
    {
        return Err("property prefix lacks the runtime log mark".into());
    }
    if prefix[8..24].iter().chain(&prefix[32..]).any(|b| *b != 0) {
        return Err("runtime property prefix was written".into());
    }
    property_log(log, pid, created)
}

/// Public CompositeTrace framing only. Semantic payment/publication checks
/// remain with the existing Mode6 validator; dropped/in-flight rows fail here.
/// A version 3 prefix keeps its bounded rows; a runtime prefix requires `log`.
pub fn native_trace(b: &[u8], log: Option<&[u8]>, pid: u32, created: u64) -> Result<(), String> {
    let (version, capacity, property) = calibration_layout(b)?;
    let assign = property + PROPERTY_PREFIX_BYTES;
    let external = u32_at(b, property + 4).ok() == Some(PROPERTY_EXTERNAL_VERSION);
    if external {
        runtime_property(b, log.ok_or("runtime property log missing")?, pid, created)?;
    } else if log.is_some() {
        return Err("property log supplied for a bounded prefix".into());
    }
    let tail = b
        .len()
        .checked_sub(assign + 24 + 1024 * 8)
        .ok_or("native trace truncated")?;
    if !tail.is_multiple_of(1024) {
        return Err("assignment stride differs".into());
    }
    let stride = tail / 1024;
    if !(96..=8192).contains(&stride) || !stride.is_multiple_of(8) {
        return Err("assignment row layout unknown".into());
    }
    for (offset, magic, version, capacity, header, row_size) in [
        (0, 0x4a414341u32, version, capacity, 24, 72),
        (property, 0x4a414350, 3, 4096, 32, 48),
        (assign, 0x4a414353, 2, 1024, 24, stride),
    ] {
        if external && offset == property {
            continue;
        }
        let u = |p: usize| u32_at(b, p);
        if u(offset)? != magic
            || u(offset + 4)? != version
            || u(offset + 12)? != 0
            || u(offset + 16)? != 0
        {
            return Err(format!(
                "native trace header at {offset:#x}: magic={:#x} version={} dropped={} in_flight={}",
                u(offset)?,
                u(offset + 4)?,
                u(offset + 12)?,
                u(offset + 16)?
            ));
        }
        let count = u(offset + 8)? as usize;
        if count > capacity {
            return Err("native trace exhausted".into());
        }
        for i in 0..count {
            if u(offset + header + i * row_size)? != 1 {
                return Err("native row uncommitted".into());
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_budget_is_independent_bounded_and_legacy_requests_stay_exact() {
        let temp = std::env::temp_dir();
        let mut step = json!({"max_seconds":300,"runtime_dll":temp.join("runtime.dll"),
            "runtime_sha256":"a".repeat(64),"bootstrap_dll":temp.join("bootstrap.dll"),
            "bootstrap_sha256":"b".repeat(64)});
        let output = temp.join("controller-budget.jsonl");
        let old = scenario_request(&step, 1, 2, &output);
        let parsed = Request::parse(&old).unwrap();
        assert_eq!((parsed.seconds, parsed.controller_seconds), (300, 300));
        for duration in [300, 1500, 1800] {
            step["controller_seconds"] = json!(duration);
            let request = scenario_request(&step, 1, 2, &output);
            let parsed = Request::parse(&request).unwrap();
            assert_eq!((parsed.seconds, parsed.controller_seconds), (300, duration));
            let mut wrong_version = request.clone();
            wrong_version["schema"] = old["schema"].clone();
            assert!(Request::parse(&wrong_version).is_err());
            let mut incomplete = request;
            incomplete
                .as_object_mut()
                .unwrap()
                .remove("controller_seconds");
            assert!(Request::parse(&incomplete).is_err());
        }
        for duration in [
            json!(0),
            json!(299),
            json!(1801),
            json!(-1),
            json!(1500.5),
            json!("1500"),
            Value::Null,
        ] {
            step["controller_seconds"] = duration;
            assert!(Request::parse(&scenario_request(&step, 1, 2, &output)).is_err());
        }
        step["controller_seconds"] = json!(1500);
        step["max_seconds"] = json!(301);
        assert!(
            Request::parse(&scenario_request(&step, 1, 2, &output)).is_err(),
            "controller time never extends native observation limits"
        );
        let mut unknown = old;
        unknown["controller_second"] = json!(1500);
        assert!(Request::parse(&unknown).is_err());
    }
    #[test]
    fn portable_request_requires_both_explicit_dll_hashes() {
        let p = std::env::temp_dir();
        let v = json!({"schema":"todo880-bootstrap-controller-v2","pid":1,"created_filetime":2,"max_seconds":30,"runtime_dll":p.join("r.dll"),"runtime_sha256":"a".repeat(64),"bootstrap_dll":p.join("b.dll"),"bootstrap_sha256":"b".repeat(64),"output":p.join("proof.jsonl")});
        assert!(Request::parse(&v).is_ok());
        for field in ["runtime_sha256", "bootstrap_sha256", "bootstrap_dll"] {
            let mut bad = v.clone();
            bad.as_object_mut().unwrap().remove(field);
            assert!(Request::parse(&bad).is_err());
        }
    }
    #[test]
    fn trace_unknown_dropped_or_partial_is_not_success() {
        let mut b = vec![0; TRACE_BYTES];
        let put = |b: &mut [u8], o: usize, n: u64| b[o..o + 8].copy_from_slice(&n.to_le_bytes());
        for (o, n) in [
            (0, 2),
            (8, 128),
            (16, 1),
            (24, 2),
            (32, 3),
            (40, 4),
            (48, 8),
            (56, 2),
            (HEADER, 1),
            (HEADER + 8, 11),
            (HEADER + ROW, 2),
            (HEADER + ROW + 8, 1),
        ] {
            put(&mut b, o, n);
        }
        assert!(trace(&b, 1, 2).is_ok());
        for (o, n) in [
            (64, 1),
            (HEADER + ROW, 0),
            (HEADER + ROW + 72, u64::MAX),
            (HEADER + ROW + 64, 1),
            (HEADER + ROW + 8, 12),
        ] {
            let mut bad = b.clone();
            put(&mut bad, o, n);
            assert!(trace(&bad, 1, 2).is_err());
        }
    }
    #[test]
    fn native_trace_dropped_inflight_or_uncommitted_is_rejected() {
        const PROPERTY: usize = 24 + 1024 * 72;
        const ASSIGN: usize = PROPERTY + 32 + 4096 * 48;
        let mut b = vec![0; ASSIGN + 24 + 1024 * 96 + 1024 * 8];
        let put = |b: &mut [u8], o: usize, n: u32| b[o..o + 4].copy_from_slice(&n.to_le_bytes());
        for (o, m, v) in [
            (0, 0x4a414341, 4),
            (PROPERTY, 0x4a414350, 3),
            (ASSIGN, 0x4a414353, 2),
        ] {
            put(&mut b, o, m);
            put(&mut b, o + 4, v);
        }
        assert!(native_trace(&b, None, 1, 2).is_ok());
        for offset in [
            12,
            16,
            PROPERTY + 12,
            PROPERTY + 16,
            ASSIGN + 12,
            ASSIGN + 16,
            8,
        ] {
            let mut bad = b.clone();
            put(&mut bad, offset, 1);
            assert!(native_trace(&bad, None, 1, 2).is_err());
        }
        // A bounded version 3 prefix never borrows a runtime log.
        assert!(native_trace(&b, Some(&[]), 1, 2).is_err());
    }

    #[test]
    fn full_six_484_matrices_fit_v5_and_loss_or_missing_returns_still_fail() {
        let capacity = 8192;
        let count = 6 * 22 * 22;
        let property = 24 + capacity * 72;
        let assignment = property + PROPERTY_PREFIX_BYTES;
        let mut b = vec![0; assignment + 24 + 1024 * 96 + 1024 * 8];
        let put = |b: &mut [u8], o: usize, n: u32| b[o..o + 4].copy_from_slice(&n.to_le_bytes());
        for (offset, magic, version) in [
            (0, 0x4a414341, 5),
            (property, 0x4a414350, 3),
            (assignment, 0x4a414353, 2),
        ] {
            put(&mut b, offset, magic);
            put(&mut b, offset + 4, version);
        }
        put(&mut b, 8, count as u32);
        for index in 0..count {
            put(&mut b, 24 + index * 72, 1);
        }
        assert!(native_trace(&b, None, 1, 2).is_ok());
        for (offset, value) in [(12, 1048), (16, 1), (24 + 1800 * 72, 0), (4, 6), (8, 8193)] {
            let mut bad = b.clone();
            put(&mut bad, offset, value);
            assert!(native_trace(&bad, None, 1, 2).is_err());
        }
    }

    #[test]
    fn legacy_runtime_prefix_keeps_its_geometry_and_dropped_receipt_rejected() {
        let (current, log) = runtime_pair();
        let old_property = 24 + 1024 * 72;
        let mut legacy = vec![0; old_property + current.len() - COMPOSITE_PROPERTY];
        legacy[..24].copy_from_slice(&current[..24]);
        legacy[4..8].copy_from_slice(&4u32.to_le_bytes());
        legacy[old_property..].copy_from_slice(&current[COMPOSITE_PROPERTY..]);
        assert!(native_trace(&legacy, Some(&log), 1, 2).is_ok());
        legacy[12..16].copy_from_slice(&1048u32.to_le_bytes());
        assert!(native_trace(&legacy, Some(&log), 1, 2).is_err());
        legacy[12..16].copy_from_slice(&0u32.to_le_bytes());
        legacy[4..8].copy_from_slice(&CALIBRATION_VERSION.to_le_bytes());
        assert!(
            native_trace(&legacy, Some(&log), 1, 2).is_err(),
            "version cannot relabel old bytes"
        );
    }

    fn runtime_pair() -> (Vec<u8>, Vec<u8>) {
        const ASSIGN: usize = COMPOSITE_PROPERTY + PROPERTY_PREFIX_BYTES;
        let mut b = vec![0; ASSIGN + 24 + 1024 * 96 + 1024 * 8];
        let put = |b: &mut [u8], o: usize, n: u32| b[o..o + 4].copy_from_slice(&n.to_le_bytes());
        for (o, m, v) in [
            (0, 0x4a414341, CALIBRATION_VERSION),
            (COMPOSITE_PROPERTY, 0x4a414350, PROPERTY_EXTERNAL_VERSION),
            (ASSIGN, 0x4a414353, 2),
        ] {
            put(&mut b, o, m);
            put(&mut b, o + 4, v);
        }
        put(&mut b, COMPOSITE_PROPERTY + 24, 6);
        put(&mut b, COMPOSITE_PROPERTY + 28, PROPERTY_LOG_MAGIC);
        let mut log = vec![0; PROPERTY_LOG_BYTES];
        let words = [
            PROPERTY_LOG_CAPACITY as u64,
            PROPERTY_LOG_STRIDE as u64,
            PROPERTY_LOG_BYTES as u64,
            1,
            2,
            5000,
            0,
            0,
            7,
        ];
        put(&mut log, 0, PROPERTY_LOG_MAGIC);
        put(&mut log, 4, PROPERTY_LOG_VERSION);
        for (i, w) in words.into_iter().enumerate() {
            log[8 + i * 8..16 + i * 8].copy_from_slice(&w.to_le_bytes());
        }
        for row in 0..5000 {
            let p = PROPERTY_LOG_HEADER + row * PROPERTY_LOG_STRIDE;
            put(&mut log, p, 1);
            put(&mut log, p + 4, 905_256 + row as u32);
        }
        (b, log)
    }

    #[test]
    fn runtime_prefix_requires_complete_matching_property_log() {
        let (b, log) = runtime_pair();
        assert_eq!(
            runtime_property(&b, &log, 1, 2),
            Ok(PropertyLog {
                rows: 5000,
                filtered: 7
            })
        );
        assert!(native_trace(&b, Some(&log), 1, 2).is_ok());
        assert!(native_trace(&b, None, 1, 2).is_err(), "empty prefix alone");
        assert!(native_trace(&b, Some(&log), 9, 2).is_err(), "other PID");
        assert!(
            native_trace(&b, Some(&log), 1, 3).is_err(),
            "other FILETIME"
        );
        assert!(native_trace(&b, Some(&log[..log.len() - 1]), 1, 2).is_err());
        let word = |o: usize, n: u64| {
            let mut bad = log.clone();
            bad[o..o + 8].copy_from_slice(&n.to_le_bytes());
            bad
        };
        for bad in [
            word(8, 4096),
            word(16, 40),
            word(PROPERTY_LOG_NEXT, 5001),
            word(PROPERTY_LOG_NEXT, PROPERTY_LOG_CAPACITY as u64 + 1),
            word(PROPERTY_LOG_DROPPED, 1),
            word(PROPERTY_LOG_IN_FLIGHT, 1),
            word(PROPERTY_LOG_HEADER + 4999 * PROPERTY_LOG_STRIDE, 0),
            word(PROPERTY_LOG_HEADER + 5000 * PROPERTY_LOG_STRIDE + 40, 1),
        ] {
            assert!(native_trace(&b, Some(&bad), 1, 2).is_err());
        }
        // Accepted calls are counted rows plus drops; overflow never passes.
        let mut full = word(PROPERTY_LOG_NEXT, PROPERTY_LOG_CAPACITY as u64);
        for row in 5000..PROPERTY_LOG_CAPACITY {
            full[PROPERTY_LOG_HEADER + row * PROPERTY_LOG_STRIDE] = 1;
        }
        assert_eq!(property_log(&full, 1, 2).unwrap().rows, 1 << 20);
        full[PROPERTY_LOG_DROPPED] = 1;
        assert!(property_log(&full, 1, 2).unwrap_err().contains("dropped=1"));
        for (offset, value) in [
            (COMPOSITE_PROPERTY + 4, 3),
            (COMPOSITE_PROPERTY + 8, 1),
            (COMPOSITE_PROPERTY + 20, 1),
            (COMPOSITE_PROPERTY + 24, 5),
            (COMPOSITE_PROPERTY + 28, 0),
            (COMPOSITE_PROPERTY + 32, 1),
        ] {
            let mut bad = b.clone();
            bad[offset..offset + 4].copy_from_slice(&(value as u32).to_le_bytes());
            assert!(native_trace(&bad, Some(&log), 1, 2).is_err());
        }
    }
}
