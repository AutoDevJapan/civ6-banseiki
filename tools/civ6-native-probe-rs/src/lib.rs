//! Read-only interpretation of a native governor trace. The Win32 debugger is
//! kept behind `windows`, so these rules are tested on every platform.
pub const CAN_EVER_RVA: u64 = 0x2d_ddd0;
pub const CAN_APPOINT_RVA: u64 = 0x2d_d820;
pub const EXPECTED_SHA256: &str =
    "324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c";
pub const MAX_SECONDS: u64 = 300;
pub const MAX_ROWS: usize = 20_000;

/// A return address identifies a call site only when its preceding instruction
/// is a direct E8 call to the observed entry. A tail jump has no new return
/// address; in that case this returns None rather than inventing a caller.
pub fn direct_call_rva(return_rva: u64, five_bytes: [u8; 5], target_rva: u64) -> Option<u64> {
    if return_rva < 5 || five_bytes[0] != 0xe8 {
        return None;
    }
    let displacement = i32::from_le_bytes(five_bytes[1..5].try_into().ok()?) as i64;
    let destination = (return_rva as i64).checked_add(displacement)?;
    (destination == target_rva as i64).then_some(return_rva - 5)
}

pub fn known_path(call_rva: Option<u64>, return_rva: Option<u64>, tail: bool) -> &'static str {
    match call_rva {
        Some(0xba51e) => "ui_getter_candidate",
        Some(0x30da56 | 0x2dcffe) => "official_appoint_candidate",
        Some(0x4a8114) => "ai_governors_candidate",
        Some(0x21b9e4) => "unclassified_static_candidate",
        Some(0xba30e) => "ui_can_appoint_candidate",
        _ if tail && return_rva == Some(0x30da5b) => "can_appoint_tail_candidate",
        _ => "unclassified",
    }
}

pub fn bool_from_al(al: u8) -> Option<bool> {
    match al {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
pub mod windows;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_call_requires_actual_destination_and_instruction() {
        // E8 FB FF FF FF at RVA 0x105 calls RVA 0x100.
        assert_eq!(
            direct_call_rva(0x105, [0xe8, 0xfb, 0xff, 0xff, 0xff], 0x100),
            Some(0x100)
        );
        assert_eq!(
            direct_call_rva(0x105, [0xe8, 0xfb, 0xff, 0xff, 0xff], 0x101),
            None
        );
        assert_eq!(
            direct_call_rva(0x105, [0xe9, 0xfb, 0xff, 0xff, 0xff], 0x100),
            None
        );
        assert_eq!(direct_call_rva(4, [0xe8, 0, 0, 0, 0], 4), None);
    }

    #[test]
    fn classification_does_not_promote_static_candidates_to_observed_ai_action() {
        assert_eq!(
            known_path(Some(0x4a8114), Some(0x4a8119), false),
            "ai_governors_candidate"
        );
        assert_eq!(
            known_path(None, Some(0x30da5b), true),
            "can_appoint_tail_candidate"
        );
        assert_eq!(known_path(None, None, false), "unclassified");
        assert_eq!(bool_from_al(0), Some(false));
        assert_eq!(bool_from_al(1), Some(true));
        assert_eq!(bool_from_al(2), None);
    }
}
