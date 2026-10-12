use civ6_bootstrap_rs::protocol::{
    root_trace, ROOT_BYTES, ROOT_CLOSED, ROOT_HEADER, ROOT_MAGIC, ROOT_ROW, ROOT_ROWS, ROOT_VERSION,
};

const CORE: u64 = 0x6000_0000;
const EPOCH: u64 = 9;
const WORLD: u64 = 0x1234_0000;
const MANAGER: u64 = 0x2345_0000;

fn put(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
fn row(bytes: &mut [u8], index: usize, field: usize, value: u64) {
    put(bytes, ROOT_HEADER + index * ROOT_ROW + field * 8, value);
}

// Reservations are entry/exit pairs; event order models actual nested calls.
// The caller stack of the original manager is distinct from its gateway stack.
fn nested_return() -> Vec<u8> {
    let mut bytes = vec![0; ROOT_BYTES];
    for (offset, value) in [
        (0, ROOT_MAGIC),
        (8, ROOT_VERSION),
        (16, ROOT_BYTES as u64),
        (24, ROOT_ROWS as u64),
        (32, ROOT_ROW as u64),
        (40, 23),
        (48, 29),
        (56, CORE),
        (72, EPOCH),
        (80, 6),
        (96, ROOT_CLOSED),
        (120, 7),
        (184, 2),
        (192, 6),
    ] {
        put(&mut bytes, offset, value);
    }
    for (index, kind, edge, invocation, parent, event) in [
        (0, 1, 1, 1, 0, 1),
        (1, 1, 2, 1, 0, 6),
        (2, 2, 1, 3, 1, 2),
        (3, 2, 2, 3, 1, 5),
        (4, 3, 1, 5, 3, 3),
        (5, 3, 2, 5, 3, 4),
    ] {
        let (caller, gateway_rsp, native_rsp, rbx) = match kind {
            1 => (CORE + 0x471c2, 0x8100, 0x8000, 0x9999),
            2 => (CORE + 0x63e1d5, 0x7fd0, 0x7e00, WORLD),
            3 => (CORE + 0x4a1196, 0x7dc0, 0x7ca0, MANAGER),
            _ => unreachable!(),
        };
        for (field, value) in [
            (0, index as u64 + 1),
            (1, kind),
            (2, edge),
            (3, invocation),
            (4, parent),
            (5, 123),
            (6, 111),
            (7, EPOCH),
            (8, caller),
            (9, gateway_rsp),
            (10, WORLD),
            (11, MANAGER),
            (12, MANAGER + 0x400),
            (13, 17),
            (14, 0x3456_0000),
            (15, 4),
            (16, 71),
            (17, 72),
            (18, 0xaabb),
            (21, native_rsp),
            (22, rbx),
            (23, event),
        ] {
            row(&mut bytes, index, field, value);
        }
    }
    bytes
}

#[test]
fn nested_wait_return_is_a_wrapper_observation_with_explicit_limits() {
    let parsed = root_trace(&nested_return(), 23, 29).unwrap();
    assert_eq!(parsed["capture_complete"], true);
    assert_eq!(parsed["same_manager_wait_normal_returns"], 1);
    assert!(parsed["not_established_by_this_capture"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "pending publication or cancellation"));
    assert!(parsed["runtime_activation_authorized"].is_null());
}

#[test]
fn root_copy_identity_and_commit_damage_never_authorize_complete_capture() {
    let original = nested_return();
    assert!(root_trace(&original[..ROOT_BYTES - 1], 23, 29).is_err());
    assert!(root_trace(&original, 24, 29).is_err());
    assert!(root_trace(&original, 23, 30).is_err());
    for (offset, value) in [
        (88, 1),
        (104, 1),
        (112, 1),
        (128, 1),
        (176, 1),
        (96, ROOT_CLOSED | 1),
    ] {
        let mut bad = original.clone();
        put(&mut bad, offset, value);
        assert_eq!(root_trace(&bad, 23, 29).unwrap()["capture_complete"], false);
    }
    let mut uncommitted = original;
    row(&mut uncommitted, 5, 0, 0);
    assert_eq!(
        root_trace(&uncommitted, 23, 29).unwrap()["capture_complete"],
        false
    );
}

#[test]
fn wait_binding_requires_actual_nested_parent_generation_and_thread_lifetime() {
    let original = nested_return();
    for (index, field, value) in [
        (4, 8, CORE + 0x4a1195),
        (4, 9, 0x7dc8),
        (4, 12, MANAGER + 0x408),
        (4, 22, MANAGER + 8),
        (4, 5, 124),
        (4, 6, 112),
        (4, 7, EPOCH + 1),
        (2, 22, WORLD + 8),
        (2, 9, 0x7fd8),
        (3, 6, 112),
        (1, 5, 124),
    ] {
        let mut bad = original.clone();
        row(&mut bad, index, field, value);
        let parsed = root_trace(&bad, 23, 29).unwrap();
        assert_eq!(
            parsed["capture_complete"], false,
            "row={index} field={field}"
        );
        assert_eq!(
            parsed["same_manager_wait_normal_returns"], 0,
            "row={index} field={field}"
        );
    }
}

#[test]
fn foreign_capture_epoch_or_unknown_thread_lifetime_is_not_a_complete_pair() {
    for (field, value) in [(7, EPOCH + 1), (6, 0), (5, 0)] {
        let mut bad = nested_return();
        for index in 0..6 {
            row(&mut bad, index, field, value);
        }
        assert_eq!(
            root_trace(&bad, 23, 29).unwrap()["capture_complete"],
            false,
            "same bad field in every row cannot create authority: {field}"
        );
    }
}

#[test]
fn reservation_order_does_not_replace_event_order_and_seh_is_not_normal_return() {
    let mut wrong_order = nested_return();
    row(&mut wrong_order, 5, 23, 5);
    row(&mut wrong_order, 3, 23, 4);
    assert_eq!(
        root_trace(&wrong_order, 23, 29).unwrap()["capture_complete"],
        false
    );
    let mut exception = nested_return();
    row(&mut exception, 5, 2, 3);
    assert_eq!(
        root_trace(&exception, 23, 29).unwrap()["capture_complete"],
        false
    );
    let mut empty = nested_return();
    put(&mut empty, 80, 0);
    assert_eq!(
        root_trace(&empty, 23, 29).unwrap()["capture_complete"],
        false
    );
}
