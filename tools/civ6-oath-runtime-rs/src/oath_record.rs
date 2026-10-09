//! Portable record types for the Oath Apply/Remove completion observer.
//! Addresses and bytes are evidence for the pinned GameCore only; they are not
//! authority for any payment, rate, or save decision. This module has no
//! process access, so it also builds and is tested on non-Windows hosts.

/// Finite row capacity. Calls beyond it are counted in `dropped`, never wrapped.
pub const OATH_CAPACITY: usize = 1024;
/// RVA of the common Apply wrapper (three arguments, Win64 `system`).
pub const PINNED_OATH_APPLY_RVA: usize = 0x8a_75e0;
/// RVA of the common Remove wrapper (three arguments, Win64 `system`).
pub const PINNED_OATH_REMOVE_RVA: usize = 0x8a_8440;
/// `push rbx; sub rsp,30h` followed by the start of `movups xmm0,[rip+disp]`.
/// The RIP-relative displacement differs per wrapper and is never moved.
pub const OATH_ENTRY_HEAD: [u8; 8] = [0x40, 0x53, 0x48, 0x83, 0xec, 0x30, 0x0f, 0x10];
/// Both pinned wrappers have eight INT3 bytes before the entry; five are used.
pub const OATH_PREFIX_PAD: [u8; 5] = [0xcc; 5];
/// Bytes relocated into the trampoline: exactly `push rbx`.
pub const OATH_RELOCATED_LEN: usize = 2;
/// Offset of the effect vtable slot the wrapper tail-jumps through.
pub const OATH_APPLY_VTABLE_SLOT: u64 = 0x60;
pub const OATH_REMOVE_VTABLE_SLOT: u64 = 0x68;

pub const COPY_EFFECT_VTABLE: u32 = 1;
pub const COPY_SUBJECT_POINTER: u32 = 2;
pub const COPY_SUBJECT_OBJECT_PRESENT: u32 = 4;
pub const COPY_ALL: u32 = COPY_EFFECT_VTABLE | COPY_SUBJECT_POINTER | COPY_SUBJECT_OBJECT_PRESENT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum OathCall {
    Apply = 1,
    Remove = 2,
}

impl OathCall {
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Apply),
            2 => Some(Self::Remove),
            _ => None,
        }
    }
    pub const fn pinned_rva(self) -> usize {
        match self {
            Self::Apply => PINNED_OATH_APPLY_RVA,
            Self::Remove => PINNED_OATH_REMOVE_RVA,
        }
    }
    pub const fn vtable_slot(self) -> u64 {
        match self {
            Self::Apply => OATH_APPLY_VTABLE_SLOT,
            Self::Remove => OATH_REMOVE_VTABLE_SLOT,
        }
    }
}

/// Where one recorded call stands. `Incomplete` is a call that has not yet
/// returned (or whose frame vanished without unwinding); `Unwound` is a call
/// that an exception removed from the stack before the original returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OathState {
    Incomplete,
    Completed,
    Unwound,
}

impl OathState {
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Incomplete),
            2 => Some(Self::Completed),
            3 => Some(Self::Unwound),
            _ => None,
        }
    }
}

/// Original effect families, identified by their vtable RVA (pinned image).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OathEffectFamily {
    Military,
    Naval,
    Build,
    Growth,
}

impl OathEffectFamily {
    pub fn from_vtable_rva(rva: u64) -> Option<Self> {
        match rva {
            0xa7_6a30 => Some(Self::Military),
            0xa6_cdb8 => Some(Self::Naval),
            0xa9_0328 => Some(Self::Build),
            0xa9_e0f0 => Some(Self::Growth),
            _ => None,
        }
    }
    /// RVA of the virtual function the wrapper tail-jumps to on success.
    pub const fn callee_rva(self, call: OathCall) -> u64 {
        match (self, call) {
            (Self::Military, OathCall::Apply) => 0x81_7810,
            (Self::Military, OathCall::Remove) => 0x81_88a0,
            (Self::Naval, OathCall::Apply) => 0x7e_8560,
            (Self::Naval, OathCall::Remove) => 0x7e_8db0,
            (Self::Build, OathCall::Apply) => 0x89_47a0,
            (Self::Build, OathCall::Remove) => 0x89_5460,
            (Self::Growth, OathCall::Apply) => 0x8d_8d60,
            (Self::Growth, OathCall::Remove) => 0x8d_9790,
        }
    }
}

/// True only when the eight entry bytes and the five bytes before them are the
/// ones the hot-patch relies on.
pub fn hotpatch_site_matches(prefix: [u8; 5], head: [u8; 8]) -> bool {
    prefix == OATH_PREFIX_PAD && head == OATH_ENTRY_HEAD
}

/// One copied call. Every value is raw evidence copied at the wrapper boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OathRecord {
    /// 1-based reservation order; the caller order of concurrent calls.
    pub sequence: u32,
    pub call: OathCall,
    pub state: OathState,
    pub thread_id: u32,
    /// Low 32 bits of R8 at entry (the instance id).
    pub instance_id: u32,
    pub raw_r8: u64,
    pub effect: u64,
    /// RDX at entry: address of the subject shared_ptr.
    pub subject: u64,
    pub caller: u64,
    pub effect_vtable: u64,
    pub subject_object: u64,
    pub subject_control: u64,
    pub copy_flags: u32,
    /// Full RAX returned by the original; meaningful only when Completed.
    pub original_rax: u64,
    /// 1-based order in which calls returned; 0 until Completed.
    pub completion_order: u32,
}

impl OathRecord {
    pub fn original_al(&self) -> u8 {
        (self.original_rax & 0xff) as u8
    }
    /// The original returned true. AL false covers both a failed internal
    /// lookup and a false callee result; the wrapper boundary cannot tell them
    /// apart, so this is deliberately not named after either cause.
    pub fn original_al_true(&self) -> bool {
        self.state == OathState::Completed && self.original_al() != 0
    }
    /// Effect vtable and the subject shared_ptr were both readable and the
    /// shared_ptr held an object pointer at the time of the call.
    pub fn inputs_resolved(&self) -> bool {
        self.copy_flags == COPY_ALL
    }
    /// Copy-side failure the gateway itself saw, independent of the result.
    pub fn input_copy_failed(&self) -> bool {
        self.copy_flags & (COPY_EFFECT_VTABLE | COPY_SUBJECT_POINTER)
            != (COPY_EFFECT_VTABLE | COPY_SUBJECT_POINTER)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(flags: u32, state: OathState, rax: u64) -> OathRecord {
        OathRecord {
            sequence: 1,
            call: OathCall::Apply,
            state,
            thread_id: 1,
            instance_id: 7,
            raw_r8: 7,
            effect: 0x1000,
            subject: 0x2000,
            caller: 0x3000,
            effect_vtable: 0,
            subject_object: 0,
            subject_control: 0,
            copy_flags: flags,
            original_rax: rax,
            completion_order: 1,
        }
    }

    #[test]
    fn pinned_entry_facts_are_the_reviewed_ones() {
        assert_eq!(PINNED_OATH_APPLY_RVA, 0x8a75e0);
        assert_eq!(PINNED_OATH_REMOVE_RVA, 0x8a8440);
        assert_eq!(OATH_ENTRY_HEAD[..OATH_RELOCATED_LEN], [0x40, 0x53]);
        assert!(hotpatch_site_matches(OATH_PREFIX_PAD, OATH_ENTRY_HEAD));
        let mut head = OATH_ENTRY_HEAD;
        head[3] = 0x84;
        assert!(!hotpatch_site_matches(OATH_PREFIX_PAD, head));
        assert!(!hotpatch_site_matches(
            [0xcc, 0xcc, 0xcc, 0xcc, 0x90],
            OATH_ENTRY_HEAD
        ));
    }

    #[test]
    fn vtable_families_map_to_the_pinned_callees() {
        let table = [
            (0xa76a30, 0x817810, 0x8188a0),
            (0xa6cdb8, 0x7e8560, 0x7e8db0),
            (0xa90328, 0x8947a0, 0x895460),
            (0xa9e0f0, 0x8d8d60, 0x8d9790),
        ];
        for (vtable, apply, remove) in table {
            let family = OathEffectFamily::from_vtable_rva(vtable).unwrap();
            assert_eq!(family.callee_rva(OathCall::Apply), apply);
            assert_eq!(family.callee_rva(OathCall::Remove), remove);
        }
        assert!(OathEffectFamily::from_vtable_rva(0xa76a38).is_none());
        assert_eq!(OathCall::Apply.vtable_slot(), 0x60);
        assert_eq!(OathCall::Remove.vtable_slot(), 0x68);
    }

    #[test]
    fn al_is_the_only_result_claim() {
        // Upper RAX bits are copied but never change the boolean reading.
        assert!(record(COPY_ALL, OathState::Completed, 0xdead_0001).original_al_true());
        assert!(!record(COPY_ALL, OathState::Completed, 0xdead_0100).original_al_true());
        assert!(!record(COPY_ALL, OathState::Incomplete, 1).original_al_true());
        assert!(!record(COPY_ALL, OathState::Unwound, 1).original_al_true());
    }

    #[test]
    fn copy_failures_are_separate_from_the_result() {
        assert!(record(COPY_ALL, OathState::Completed, 0).inputs_resolved());
        let null_object = COPY_EFFECT_VTABLE | COPY_SUBJECT_POINTER;
        let r = record(null_object, OathState::Completed, 1);
        assert!(!r.inputs_resolved() && !r.input_copy_failed());
        let r = record(COPY_EFFECT_VTABLE, OathState::Completed, 1);
        assert!(r.input_copy_failed());
        assert!(record(0, OathState::Completed, 1).input_copy_failed());
    }

    #[test]
    fn state_and_call_decoding_rejects_unknown_values() {
        assert_eq!(OathState::from_u32(0), None);
        assert_eq!(OathState::from_u32(3), Some(OathState::Unwound));
        assert_eq!(OathState::from_u32(4), None);
        assert_eq!(OathCall::from_u32(0), None);
        assert_eq!(OathCall::from_u32(2), Some(OathCall::Remove));
    }
}
