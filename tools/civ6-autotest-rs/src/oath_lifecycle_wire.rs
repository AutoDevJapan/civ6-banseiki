//! Diagnostic context-root and application-teardown records. A matching pair
//! of copies describes that read only; it never proves object lifetime/drain.
#![allow(dead_code)]
use crate::oath_wire::{u32_at, u64_at};
use serde_json::{json, Value};

pub(crate) const EXPORTS: [&str; 4] = [
    "Civ6OathLifecycleInstallShared",
    "Civ6OathLifecycleStart",
    "Civ6OathLifecycleStop",
    "Civ6OathLifecyclePoll",
];
pub(crate) const HEADER: usize = 48;
pub(crate) const ROW: usize = 64;
pub(crate) const CAPACITY: usize = 512;
pub(crate) const SIZE: usize = HEADER + ROW * CAPACITY;

pub(crate) fn validate(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != SIZE
        || u32_at(bytes, 0) != Some(0x4c43_3134)
        || u32_at(bytes, 4) != Some(1)
        || u32_at(bytes, 8).is_none_or(|n| n as usize > CAPACITY)
        || u32_at(bytes, 28).is_none_or(|n| n > 2)
        || u32_at(bytes, 40).is_none_or(|n| n > 1)
        || u32_at(bytes, 44).is_none_or(|n| n > 1)
    {
        return Err("invalid lifecycle trace header".into());
    }
    for row in bytes[HEADER..]
        .chunks_exact(ROW)
        .take(u32_at(bytes, 8).unwrap() as usize)
    {
        let state = u32_at(row, 0).unwrap();
        if state > 3 || u32_at(row, 60) != Some(0) {
            return Err("invalid lifecycle row state/reserved field".into());
        }
        if state != 0
            && (u32_at(row, 4).is_none_or(|n| n > 1)
                || u32_at(row, 8) == Some(0)
                || u32_at(row, 12) == Some(0))
        {
            return Err("invalid lifecycle row identity".into());
        }
        if state != 0 {
            let root = if u32_at(row, 4) == Some(0) { 32 } else { 24 };
            if u64_at(row, 16) != u64_at(row, root) {
                return Err("lifecycle receiver differs from recorded arguments".into());
            }
        }
    }
    Ok(())
}

pub(crate) fn project(first: &[u8], second: &[u8]) -> Result<Value, String> {
    validate(first)?;
    validate(second)?;
    let count = u32_at(second, 8).unwrap() as usize;
    let first_count = u32_at(first, 8).unwrap() as usize;
    let mut normal = 0;
    let mut unwound = 0;
    let mut unknown = 0;
    let mut entered = 0;
    let mut unpublished = 0;
    let mut changed = 0;
    let mut roots = 0;
    let mut nonnull_roots = 0;
    let mut teardowns = 0;
    let mut completion_orders = Vec::new();
    let records: Vec<Value> = (0..count)
        .map(|i| {
            let at = HEADER + i * ROW;
            let row = &second[at..at + ROW];
            let state = u32_at(row, 0).unwrap();
            if i >= first_count || row != &first[at..at + ROW] {
                changed += 1;
                unknown += 1;
                return json!({"index":i,"state":"unknown"});
            }
            if state == 0 {
                unpublished += 1;
                unknown += 1;
                return json!({"index":i,"state":"unknown"});
            }
            if state == 1 {
                entered += 1;
                unknown += 1;
            }
            normal += usize::from(state == 2);
            if state == 2 {
                if u32_at(row, 4) == Some(0) {
                    roots += 1;
                    nonnull_roots += usize::from(u64_at(row, 16) != Some(0));
                } else {
                    teardowns += 1;
                }
            }
            if state == 2 {
                completion_orders.push(u32_at(row, 56).unwrap());
            }
            unwound += usize::from(state == 3);
            let hex = |at| format!("0x{:x}", u64_at(row, at).unwrap());
            json!({"index":i,"state":state,
            "kind":if u32_at(row,4)==Some(0) {"context_root"} else {"application_teardown"},
            "thread_id":u32_at(row,8).unwrap(),"reentry_depth":u32_at(row,12).unwrap(),
            "root_or_this":hex(16),"rcx":hex(24),"rdx":hex(32),"caller":hex(40),
            "rax":if state==2 {json!(hex(48))} else {Value::Null},
            "completion_order":if state==2 {json!(u32_at(row,56).unwrap())} else {Value::Null}})
        })
        .collect();
    completion_orders.sort_unstable();
    let consistent = first == second
        && u32_at(second, 28) == Some(2)
        && u32_at(second, 44) == Some(1)
        && u32_at(second, 12) == Some(0)
        && u32_at(second, 16) == Some(0)
        && unknown == 0
        && normal + unwound == count
        && u32_at(second, 20) == Some(normal as u32)
        && u32_at(second, 24) == Some(unwound as u32);
    let consistent = consistent && completion_orders == (1..=normal as u32).collect::<Vec<_>>();
    // A stable prefix can omit a call because recording is still running,
    // expired, full, or interrupted. Only terminal, consistent coverage can
    // report absence. A copied completed call remains evidence of that call.
    let incomplete = !consistent || u32_at(second, 40) == Some(1) || unwound != 0;
    let diagnostic = |completed| {
        if completed > 0 {
            "observed"
        } else if incomplete {
            "unknown"
        } else {
            "not_observed"
        }
    };
    let hex = |b: &[u8]| b.iter().map(|n| format!("{n:02x}")).collect::<String>();
    Ok(json!({"format":"oath-lifecycle-v1","records":records,
        "in_flight":u32_at(second,16),"completed":u32_at(second,20),"unwound":u32_at(second,24),
        "dropped":u32_at(second,12),"stopped":u32_at(second,28),"expired":u32_at(second,40),
        "detached":u32_at(second,44),"unknown_rows":unknown,
        "entered_rows":entered,"unpublished_rows":unpublished,"changed_rows":changed,
        "diagnostic_capture_incomplete":incomplete,
        "completed_context_root_calls":roots,"completed_nonnull_context_root_calls":nonnull_roots,
        "completed_application_teardown_calls":teardowns,
        "context_root_diagnostic":diagnostic(nonnull_roots),
        "application_teardown_diagnostic":diagnostic(teardowns),
        "record_consistent_at_read":consistent,"permanent_drain_proved":false,
        "first_root_observed":false,"native_lifetime_proved":false,
        "quantity_accepted":false,"product_accepted":false,
        "raw_first":hex(first),"raw_second":hex(second)}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn layout_matches_the_compiled_lifecycle_runtime() {
        assert_eq!(
            civ6_oath_runtime_rs::lifecycle_trace::LifecycleTrace::wire_layout(),
            (HEADER, ROW, SIZE)
        );
        assert_eq!(
            std::mem::offset_of!(civ6_oath_runtime_rs::lifecycle_trace::Row, caller),
            40
        );
        assert_eq!(
            std::mem::offset_of!(civ6_oath_runtime_rs::lifecycle_trace::Row, completion_order),
            56
        );
    }
    fn put(bytes: &mut [u8], at: usize, value: u32) {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; SIZE];
        for (at, value) in [
            (0, 0x4c43_3134),
            (4, 1),
            (8, 1),
            (20, 1),
            (28, 2),
            (44, 1),
            (HEADER, 2),
            (HEADER + 8, 123),
            (HEADER + 12, 1),
            (HEADER + 56, 1),
        ] {
            put(&mut b, at, value);
        }
        b
    }
    #[test]
    fn valid_record_still_does_not_prove_first_root_or_lifetime() {
        let b = fixture();
        let p = project(&b, &b).unwrap();
        assert_eq!(p["records"][0]["kind"], "context_root");
        assert_eq!(p["record_consistent_at_read"], true);
        assert_eq!(p["completed_context_root_calls"], 1);
        assert_eq!(p["context_root_diagnostic"], "not_observed");
        assert_eq!(p["application_teardown_diagnostic"], "not_observed");
        for key in [
            "permanent_drain_proved",
            "first_root_observed",
            "native_lifetime_proved",
            "product_accepted",
        ] {
            assert_eq!(p[key], false);
        }
    }

    #[test]
    fn absent_null_or_teardown_only_records_do_not_report_root_reinitialization() {
        let mut b = fixture();
        for at in [HEADER + 16, HEADER + 32] {
            b[at..at + 8].copy_from_slice(&0x1234u64.to_le_bytes());
        }
        assert_eq!(
            project(&b, &b).unwrap()["context_root_diagnostic"],
            "observed"
        );
        put(&mut b, HEADER + 4, 1);
        b[HEADER + 24..HEADER + 32].copy_from_slice(&0x1234u64.to_le_bytes());
        let p = project(&b, &b).unwrap();
        assert_eq!(p["context_root_diagnostic"], "not_observed");
        assert_eq!(p["application_teardown_diagnostic"], "observed");
        put(&mut b, 8, 0);
        put(&mut b, 20, 0);
        let p = project(&b, &b).unwrap();
        assert_eq!(p["context_root_diagnostic"], "not_observed");
        assert_eq!(p["application_teardown_diagnostic"], "not_observed");
    }
    #[test]
    fn changed_unpublished_entered_or_missing_completion_is_not_complete() {
        let b = fixture();
        let mut changed = b.clone();
        put(&mut changed, HEADER + 40, 4);
        assert_eq!(
            project(&b, &changed).unwrap()["records"][0]["state"],
            "unknown"
        );
        for (at, n) in [(HEADER, 0), (HEADER, 1), (20, 0), (16, 1), (12, 1), (44, 0)] {
            let mut invalid = b.clone();
            put(&mut invalid, at, n);
            assert_eq!(
                project(&invalid, &invalid).unwrap()["record_consistent_at_read"],
                false
            );
        }
        let mut no_rows = b.clone();
        put(&mut no_rows, 8, 0);
        assert_eq!(
            project(&no_rows, &b).unwrap()["records"][0]["state"],
            "unknown"
        );
    }
    #[test]
    fn incomplete_capture_cannot_report_an_absent_call() {
        // None of these buffers contains a completed non-null root or teardown.
        // A stable pair is still insufficient when its coverage is incomplete.
        for (at, n) in [
            (HEADER, 0),
            (HEADER, 1),
            (HEADER, 3),
            (20, 0),
            (16, 1),
            (12, 1),
            (28, 0),
            (40, 1),
            (44, 0),
        ] {
            let mut b = fixture();
            put(&mut b, at, n);
            let p = project(&b, &b).unwrap();
            for diagnostic in ["context_root_diagnostic", "application_teardown_diagnostic"] {
                assert_eq!(p[diagnostic], "unknown", "{at}={n}: {diagnostic}");
            }
        }
        let mut first = fixture();
        let mut second = first.clone();
        put(&mut first, 12, 1);
        put(&mut second, 8, 0);
        put(&mut second, 20, 0);
        let p = project(&first, &second).unwrap();
        assert_eq!(p["context_root_diagnostic"], "unknown");
        assert_eq!(p["application_teardown_diagnostic"], "unknown");
    }
    #[test]
    fn incomplete_entry_remains_visible_and_completed_calls_are_still_observed() {
        let mut b = fixture();
        put(&mut b, HEADER, 1);
        put(&mut b, 20, 0);
        put(&mut b, 16, 1);
        let p = project(&b, &b).unwrap();
        assert_eq!(p["records"][0]["state"], 1);
        assert_eq!(p["entered_rows"], 1);
        assert_eq!(p["unknown_rows"], 1);

        let mut b = fixture();
        for at in [HEADER + 16, HEADER + 32] {
            b[at..at + 8].copy_from_slice(&0x1234u64.to_le_bytes());
        }
        put(&mut b, 12, 1);
        let p = project(&b, &b).unwrap();
        assert_eq!(p["context_root_diagnostic"], "observed");
        assert_eq!(p["application_teardown_diagnostic"], "unknown");
        assert_eq!(p["record_consistent_at_read"], false);
        assert_eq!(p["product_accepted"], false);
    }
    #[test]
    fn unknown_versions_dimensions_flags_and_rows_are_rejected() {
        let b = fixture();
        assert!(validate(&b[..SIZE - 1]).is_err());
        for (at, n) in [
            (0, 0),
            (4, 2),
            (8, 513),
            (28, 3),
            (40, 2),
            (44, 2),
            (HEADER, 4),
            (HEADER + 4, 2),
            (HEADER + 8, 0),
            (HEADER + 12, 0),
            (HEADER + 60, 1),
        ] {
            let mut bad = b.clone();
            put(&mut bad, at, n);
            assert!(validate(&bad).is_err(), "{at}");
        }
    }
}
