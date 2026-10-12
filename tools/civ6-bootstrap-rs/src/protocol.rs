//! Portable DTO/trace framing, not game or lifecycle proof.
use serde_json::{Value, json};
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
pub const SUBMISSION_ROW: usize = 256;
pub const SUBMISSION_BYTES: usize = ROOT_HEADER + SUBMISSION_ROW * ROOT_ROWS;

/// Copies made at entry only. Scalar addresses in these records are opaque;
/// this parser never follows them. Correspondence is not job ownership/join.
pub fn submission_trace(b: &[u8], pid: u32, created: u64) -> Result<Value, String> {
    if b.len() != SUBMISSION_BYTES
        || word(b, 0)? != SUBMISSION_MAGIC
        || word(b, 8)? != 1
        || word(b, 16)? != SUBMISSION_BYTES as u64
        || word(b, 24)? != ROOT_ROWS as u64
        || word(b, 32)? != SUBMISSION_ROW as u64
        || word(b, 40)? != u64::from(pid)
        || word(b, 48)? != created
        || (28..32).any(|i| word(b, i * 8).ok() != Some(0))
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
    let mut pairs = Vec::<([u64; 32], [u64; 32])>::new();
    let mut rows = Vec::new();
    let mut events = std::collections::BTreeSet::new();
    for i in (0..next).step_by(2) {
        let read = |row: usize| -> Result<[u64; 32], String> {
            let mut r = [0; 32];
            for (j, v) in r.iter_mut().enumerate() {
                *v = word(b, ROOT_HEADER + row * SUBMISSION_ROW + j * 8)?;
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
                json!({"raw":r.to_vec(),"invocation_id":r[3],"kind":r[1],"edge":r[2],
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
            && a[30..] == z[30..]
            && if a[1] == 4 {
                a[30] != 0
                    && a[31] != 0
                    && matches!(a[24], 16 | 32)
                    && (a[24] == 32 || a[16..18] == [0, 0])
                    && a[26] == 16
                    && a[25] == 0
                    && a[27] == 0
            } else {
                a[14..20] == [0; 6] && a[24..28] == [0; 4] && a[30..] == [0, 0]
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
    // One fixed, fully checked direct-call loop. RSP is the native caller
    // frame identity; an equal list value alone cannot supply this relation.
    let mut matched = std::collections::BTreeSet::new();
    for (publish, publish_return) in pairs.iter().filter(|(a, _)| a[1] == 5) {
        if core.checked_add(0x38888e) != Some(publish[8]) {
            continue;
        }
        for (submit, returned) in pairs.iter().filter(|(a, _)| a[1] == 4) {
            if core.checked_add(0x388871)==Some(submit[8])
                && submit[4]==0 && publish[4]==0
                && submit[5..8]==publish[5..8] && submit[9]==publish[9]
                && submit[10]>0 && submit[10]==publish[10]
                && returned[23]<publish[23] && publish[23]<publish_return[23]
                // A later/interleaved publication for this frame cuts the batch.
                && !pairs.iter().any(|(other,_)|other[1]==5 && other[5..8]==submit[5..8]
                    && other[9]==submit[9] && returned[23]<other[23] && other[23]<publish[23])
            {
                matched.insert(submit[3]);
            }
        }
    }
    let submitted = pairs.iter().filter(|(a, _)| a[1] == 4).count();
    let stopped = word(b, 96)? == ROOT_CLOSED && word(b, 184)? == 2;
    let clean = stopped
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
        json!({"schema":"todo880-native-submission-trace-v1","pid":pid,
        "created_filetime":created,"core":core,"core_epoch":epoch,"reserved_rows":next,
        "rows":rows,"site_witnesses":sites,"accepted_calls_stopped":stopped,
        "submitted_normal_returns":submitted,"matched_published_submissions":matched.len(),
        "capture_complete":clean && submitted>0 && !matched.is_empty(),"unknown":unknown,
        "runtime_activated":false,"scope":"accepted_two_wrapper_calls_sampling_only",
        "not_established_by_this_capture":["descriptor C++ type and ownership","birth generation ticket",
        "retain/copy/release/cancel/reissue","native publication completion","job execution or Join",
        "World root identity/lifetime or all-domain drainage","runtime activation authority",
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
        let mut b = vec![0; SUBMISSION_BYTES];
        for (i, v) in [
            SUBMISSION_MAGIC,
            1,
            SUBMISSION_BYTES as u64,
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
        assert!(
            Request::root_scenario(
                &json!({"observation":"native_submission","controller_seconds":60})
            )
            .unwrap()
        );
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
