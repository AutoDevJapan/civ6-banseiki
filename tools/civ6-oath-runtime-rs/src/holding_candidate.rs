//! Read-only diagnostic of the ALREADY INITIALIZED F4990 return path.
//! F4990 is NONLEAF. Only its nonnull branch returns the existing value of
//! *(EXE+22D6070), unchanged, without calling anything. The global slot's
//! address is not the helper address. Null never invokes the constructor.
//! Two equal reads prove neither a holding interval, City lifetime, permanent
//! drain nor product authority. Every model/admission flag remains false.
use crate::call_identity::Memory;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub const GLOBAL_RVA: u64 = 0x22d6070;
pub const OWNER_OFFSET: u64 = 0x4a0;
pub const DEPTH_OFFSET: u64 = 0x4a4;
pub const FIRST_VALID: u32 = 1;
pub const SECOND_VALID: u32 = 2;
pub const STABLE: u32 = 4;
pub const MODEL_FLAGS: u32 = 0;

// Pure expression proof for the fixed NONLEAF Getter. JNE+0x54 jumps from
// F49A0 to F49F4; that ADD RSP/RET leaves the global-loaded RAX untouched.
// This never authorizes executing either branch or choosing another source.
fn checked_nonnull_slot(getter: &[u8]) -> Option<u64> {
    (getter.len() == 0x69
        && getter[..16]
            == [
                0x48, 0x83, 0xec, 0x38, 0x48, 0x8b, 0x05, 0xd5, 0x16, 0x1e, 0x02, 0x48, 0x85, 0xc0,
                0x75, 0x54,
            ]
        && getter[0x10..0x15] == [0xe8, 0x4b, 0xf6, 0xe6, 0]
        && getter[0x64..] == [0x48, 0x83, 0xc4, 0x38, 0xc3])
    .then_some(GLOBAL_RVA)
}

/// Wire offsets within one read: helper u64+0, owner u32+8, depth u32+12.
/// Unknown fields retain sentinels/raw partial data, never a valid zero read.
#[repr(C)]
pub struct HelperRead {
    pub helper: AtomicU64,
    pub owner: AtomicU32,
    pub depth: AtomicU32,
}
impl Default for HelperRead {
    fn default() -> Self {
        Self {
            helper: AtomicU64::new(0),
            owner: AtomicU32::new(u32::MAX),
            depth: AtomicU32::new(u32::MAX),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadSnapshot {
    pub helper: u64,
    pub owner: u32,
    pub depth: u32,
}
impl HelperRead {
    pub fn snapshot(&self) -> ReadSnapshot {
        ReadSnapshot {
            helper: self.helper.load(Ordering::Acquire),
            owner: self.owner.load(Ordering::Acquire),
            depth: self.depth.load(Ordering::Acquire),
        }
    }
    fn capture(&self, memory: &mut impl Memory, global: u64) -> bool {
        let mut pointer = [0; 8];
        if memory.copy(global, &mut pointer).is_err() {
            return false;
        }
        let helper = u64::from_le_bytes(pointer);
        self.helper.store(helper, Ordering::Relaxed);
        if helper == 0 {
            return false;
        }
        let Some(fields) = helper.checked_add(OWNER_OFFSET) else {
            return false;
        };
        let mut words = [0; 8];
        if memory.copy(fields, &mut words).is_err() {
            return false;
        }
        let owner = u32::from_le_bytes(words[..4].try_into().unwrap());
        let depth = u32::from_le_bytes(words[4..].try_into().unwrap());
        self.owner.store(owner, Ordering::Relaxed);
        self.depth.store(depth, Ordering::Relaxed);
        // Negative native signed recursion counts are unknown, not ownership.
        depth <= i32::MAX as u32
    }
}
/// v5 boundary wire: flags u32+0, reserved u32+4, tick u64+8,
/// first HelperRead+16, second HelperRead+32. Exactly 48 bytes, alignment 8.
/// Flags 1/2: that read is complete, nonnull and has nonnegative signed depth;
/// flag 4: both such snapshots are equal. Change/unknown => stable is clear.
#[repr(C)]
#[derive(Default)]
pub struct HelperProbe {
    pub flags: AtomicU32,
    pub reserved: AtomicU32,
    pub tick: AtomicU64,
    pub first: HelperRead,
    pub second: HelperRead,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeSnapshot {
    pub flags: u32,
    pub reserved: u32,
    pub tick: u64,
    pub first: ReadSnapshot,
    pub second: ReadSnapshot,
}
impl HelperProbe {
    /// Caller supplies a source-checked global slot, never the native Getter.
    /// No allocations, locks, target-method calls or reference acquisition.
    pub(crate) fn capture(&self, memory: &mut impl Memory, global: u64, tick: u64) {
        self.tick.store(tick, Ordering::Relaxed);
        let first = self.first.capture(memory, global);
        let second = self.second.capture(memory, global);
        let equal = first && second && self.first.snapshot() == self.second.snapshot();
        let flags = u32::from(first) | (u32::from(second) << 1) | (u32::from(equal) << 2);
        self.flags.store(flags, Ordering::Release);
    }
    pub fn snapshot(&self) -> ProbeSnapshot {
        ProbeSnapshot {
            flags: self.flags.load(Ordering::Acquire),
            reserved: self.reserved.load(Ordering::Acquire),
            tick: self.tick.load(Ordering::Acquire),
            first: self.first.snapshot(),
            second: self.second.snapshot(),
        }
    }
}
impl ProbeSnapshot {
    /// Diagnostic at these two reads only; never permission to use a pointer.
    pub fn owned_at_reads(&self, thread: u32) -> bool {
        self.flags == FIRST_VALID | SECOND_VALID | STABLE
            && self.reserved == 0
            && self.first == self.second
            && self.first.helper != 0
            && self.first.owner == thread
            && self.first.depth > 0
            && self.first.depth <= i32::MAX as u32
    }
}
const _: () = assert!(std::mem::size_of::<HelperRead>() == 16);
const _: () = assert!(std::mem::size_of::<HelperProbe>() == 48);
const _: () = assert!(std::mem::offset_of!(HelperProbe, first) == 16);
const _: () = assert!(std::mem::offset_of!(HelperProbe, second) == 32);

#[cfg(windows)]
mod source {
    use super::*;
    use crate::identity_windows as identity;
    use crate::runtime::OathStatus;
    use sha2::{Digest, Sha256};
    use std::fs::File;
    use std::sync::OnceLock;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    struct Source {
        exe: u64,
        _file: File,
    }
    static SOURCE: OnceLock<Result<Source, OathStatus>> = OnceLock::new();
    fn verify() -> Result<Source, OathStatus> {
        let exe = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
        if exe == 0
            || !identity::same_path(
                &identity::module_path(exe as _)?,
                crate::preflight::EXE_PATH,
            )
        {
            return Err(OathStatus::ProcessIdentity);
        }
        let (file, disk) = identity::pinned_disk(crate::preflight::EXE_PATH)?;
        if format!("{:x}", Sha256::digest(&disk)) != crate::preflight::EXE_SHA256
            || !identity::header_matches(exe, &disk)
            || crate::runtime::pin_module(exe) != Some(exe)
        {
            return Err(OathStatus::ImageIdentity);
        }
        // Entire nonleaf Getter through its last RET, including null branches,
        // is checked against the fixed source; nothing in it is invoked.
        let getter = identity::source_rva(&disk, 0xf4990, 0x69)?;
        if checked_nonnull_slot(getter) != Some(GLOBAL_RVA)
            || identity::source_rva(&disk, 0xf63ff0, 6)? != [0xb8, 1, 0, 0, 0, 0xc3]
            || identity::source_rva(&disk, 0xf4c2b, 6)? != [0xff, 0x87, 0xa4, 4, 0, 0]
            || identity::source_rva(&disk, 0xf4c39, 6)? != [0x89, 0x87, 0xa0, 4, 0, 0]
        {
            return Err(OathStatus::EntryBytes);
        }
        identity::match_function(exe, &disk, 0xf4990)?;
        for (rva, length) in [(0xf63ff0, 6), (0xf4c2b, 20)] {
            identity::match_window(exe, &disk, rva, length)?;
        }
        Ok(Source {
            exe: exe as u64,
            _file: file,
        })
    }
    pub(crate) fn prepare() -> Option<u64> {
        SOURCE.get_or_init(verify).as_ref().ok().map(|s| s.exe)
    }
    pub(crate) fn global(exe: u64) -> Option<u64> {
        let source = SOURCE.get()?.as_ref().ok()?;
        (source.exe == exe)
            .then(|| exe.checked_add(GLOBAL_RVA))
            .flatten()
    }
}
#[cfg(windows)]
pub(crate) use source::{global, prepare};

#[cfg(test)]
mod tests {
    use super::*;
    struct Mock {
        reads: usize,
        change: bool,
        missing: bool,
        null: bool,
        owner: u32,
        depth: u32,
    }
    impl Memory for Mock {
        fn copy(&mut self, at: u64, bytes: &mut [u8]) -> crate::call_identity::Result<()> {
            self.reads += 1;
            if self.missing {
                return Err(crate::call_identity::Error::input("missing"));
            }
            if at == 0x1000 {
                bytes.copy_from_slice(&(if self.null { 0u64 } else { 0x2000u64 }).to_le_bytes());
            } else if at == 0x24a0 {
                bytes[..4].copy_from_slice(&self.owner.to_le_bytes());
                bytes[4..].copy_from_slice(
                    &(self.depth + u32::from(self.change && self.reads > 2)).to_le_bytes(),
                );
            } else {
                return Err(crate::call_identity::Error::input("unexpected read"));
            }
            Ok(())
        }
    }
    #[test]
    fn valid_racing_missing_foreign_zero_and_negative_are_diagnostic_only() {
        for (change, missing, null, owner, depth, expected) in [
            (false, false, false, 7, 2, 7),
            (true, false, false, 7, 2, 3),
            (false, true, false, 7, 2, 0),
            (false, false, true, 7, 2, 0),
            (false, false, false, 8, 2, 7),
            (false, false, false, 7, 0, 7),
            (false, false, false, 7, u32::MAX, 0),
        ] {
            let probe = HelperProbe::default();
            let mut m = Mock {
                reads: 0,
                change,
                missing,
                null,
                owner,
                depth,
            };
            probe.capture(&mut m, 0x1000, 55);
            let s = probe.snapshot();
            assert_eq!(s.flags, expected);
            assert_eq!(s.tick, 55);
            assert_eq!(
                s.owned_at_reads(7),
                !change && !missing && !null && owner == 7 && depth > 0 && depth <= i32::MAX as u32
            );
            if missing || null {
                assert_eq!(s.first.depth, u32::MAX);
            }
            assert_eq!(MODEL_FLAGS, 0);
        }
    }
    #[test]
    fn compiled_wire_layout() {
        assert_eq!(std::mem::size_of::<HelperProbe>(), 48);
        assert_eq!(std::mem::offset_of!(HelperProbe, tick), 8);
        assert_eq!(std::mem::offset_of!(HelperProbe, first), 16);
        assert_eq!(std::mem::offset_of!(HelperProbe, second), 32);
        assert_eq!(std::mem::offset_of!(HelperRead, owner), 8);
        assert_eq!(std::mem::offset_of!(HelperRead, depth), 12);
    }
    #[test]
    fn nonleaf_nonnull_expression_rejects_changed_global_branch_call_and_return() {
        let mut getter = [0u8; 0x69];
        getter[..16].copy_from_slice(&[
            0x48, 0x83, 0xec, 0x38, 0x48, 0x8b, 0x05, 0xd5, 0x16, 0x1e, 0x02, 0x48, 0x85, 0xc0,
            0x75, 0x54,
        ]);
        getter[0x10..0x15].copy_from_slice(&[0xe8, 0x4b, 0xf6, 0xe6, 0]);
        getter[0x64..].copy_from_slice(&[0x48, 0x83, 0xc4, 0x38, 0xc3]);
        assert_eq!(checked_nonnull_slot(&getter), Some(GLOBAL_RVA));
        for at in [7, 14, 15, 16, 0x64, 0x68] {
            let mut changed = getter;
            changed[at] ^= 1;
            assert_eq!(checked_nonnull_slot(&changed), None);
        }
        assert_eq!(checked_nonnull_slot(&getter[..0x68]), None);
    }
}
