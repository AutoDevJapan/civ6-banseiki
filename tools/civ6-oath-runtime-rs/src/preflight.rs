//! Immutable pinned-DLL preflight. This has no process access or installer.
//! The Windows caller must independently pin process creation, files, loaded
//! image ownership and lifetime, and reject a described hotpatch prefix.

use crate::oath_record::{OathCall, OathEffectFamily, OATH_ENTRY_HEAD, OATH_PREFIX_PAD};
use sha2::{Digest, Sha256};

pub const CORE_SHA256: &str = "324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c";
pub const EXE_SHA256: &str = "e7450823cc8e00468cff7b9d7b97c63140eae38ae1d774ba4efa437556c42d63";
pub const CORE_PATH: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll";
pub const EXE_PATH: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/Base/Binaries/Win64Steam/CivilizationVI.exe";
const PREFERRED_BASE: u64 = 0x180000000;
const FILE_BYTES: usize = 12_940_384;
const IMAGE_BYTES: u64 = 0xc60000;

pub trait ImageMemory {
    /// Return Ok only after copying every requested byte. Partial or failed
    /// reads must return Err; untouched zero bytes are never an observation.
    fn copy(&mut self, address: u64, data: &mut [u8]) -> Result<(), &'static str>;
}

#[derive(Clone)]
struct Window {
    rva: u64,
    original: Vec<u8>,
}

/// Only `from_disk` can construct this plan outside this module. Matching this
/// plan does NOT establish ruleset, single-player mode, completed effects or
/// quantities. It describes the exact module's bytes and relocated pointers.
pub struct SourcePlan {
    windows: Vec<Window>,
    slots: Vec<(u64, u64)>,
}

fn disk_range(disk: &[u8], rva: u64, len: usize) -> Result<&[u8], &'static str> {
    let end = rva.checked_add(len as u64).ok_or("source RVA overflow")?;
    let offset = if rva >= 0x1000 && end <= 0x9b6627 {
        rva - 0xc00
    } else if rva >= 0x9b7000 && end <= 0xb5f124 {
        rva - 0x1400
    } else {
        return Err("source window outside audited sections");
    };
    let start = usize::try_from(offset).map_err(|_| "source offset overflow")?;
    let end = start.checked_add(len).ok_or("source offset overflow")?;
    disk.get(start..end).ok_or("source window incomplete")
}

impl SourcePlan {
    pub fn from_disk(disk: &[u8]) -> Result<Self, &'static str> {
        if disk.len() != FILE_BYTES || format!("{:x}", Sha256::digest(disk)) != CORE_SHA256 {
            return Err("unknown complete GameCore file identity");
        }
        Self::layout(disk)
    }

    fn layout(disk: &[u8]) -> Result<Self, &'static str> {
        let mut windows = Vec::new();
        for call in [OathCall::Apply, OathCall::Remove] {
            let rva = call.pinned_rva() as u64;
            let bytes = disk_range(disk, rva - 5, 5 + 0x58)?;
            if bytes[..5] != OATH_PREFIX_PAD || bytes[5..13] != OATH_ENTRY_HEAD {
                return Err("unknown complete City adapter entry or prefix");
            }
            let tail = if call == OathCall::Apply {
                [0x49, 0xff, 0x60, 0x60]
            } else {
                [0x49, 0xff, 0x60, 0x68]
            };
            if bytes[5 + 0x4c..5 + 0x50] != tail {
                return Err("unknown complete City adapter tail dispatch");
            }
            windows.push(Window {
                rva: rva - 5,
                original: bytes.to_vec(),
            });
        }
        for rva in [0x944e52, 0x944f24, 0x974284] {
            windows.push(Window {
                rva,
                original: disk_range(disk, rva, 8)?.to_vec(),
            });
        }
        for &(rva, len) in &[
            (0x044d60, 0xf),
            (0x9723d0, 0x68),
            (0x950a10, 0x60),
            (0x2c4360, 0x45),
            (0x1332b0, 0x25),
            (0x135890, 0x30),
        ] {
            windows.push(Window {
                rva,
                original: disk_range(disk, rva, len)?.to_vec(),
            });
        }
        let mut slots = Vec::new();
        for table in [0xa76a30, 0xa6cdb8, 0xa90328, 0xa9e0f0] {
            let family = OathEffectFamily::from_vtable_rva(table).ok_or("unknown effect family")?;
            for call in [OathCall::Apply, OathCall::Remove] {
                let rva = family.callee_rva(call);
                windows.push(Window {
                    rva,
                    original: disk_range(disk, rva, 0x80)?.to_vec(),
                });
            }
            for (offset, target) in [
                (0x28, 0x8a75e0),
                (0x30, 0x8a8440),
                (0x60, family.callee_rva(OathCall::Apply)),
                (0x68, family.callee_rva(OathCall::Remove)),
            ] {
                let at = table + offset;
                let observed = u64::from_le_bytes(disk_range(disk, at, 8)?.try_into().unwrap());
                if observed != PREFERRED_BASE + target {
                    return Err("unknown source effect dispatch");
                }
                slots.push((at, target));
            }
        }
        for &(table, offset, target) in &[
            (0xab4f58, 0x00, 0x941f90),
            (0xab4f58, 0x10, 0x9723d0),
            (0xab5020, 0x00, 0x950a10),
        ] {
            let at = table + offset;
            let observed = u64::from_le_bytes(disk_range(disk, at, 8)?.try_into().unwrap());
            if observed != PREFERRED_BASE + target {
                return Err("unknown wrapper vtable dispatch");
            }
            slots.push((at, target));
        }
        Ok(Self { windows, slots })
    }

    pub fn validate_image(
        &self,
        base: u64,
        memory: &mut impl ImageMemory,
    ) -> Result<(), &'static str> {
        if base == 0 || base & 0xffff != 0 || base.checked_add(IMAGE_BYTES).is_none() {
            return Err("invalid loaded image base");
        }
        for window in &self.windows {
            let mut bytes = vec![0; window.original.len()];
            memory.copy(base + window.rva, &mut bytes)?;
            if bytes != window.original {
                return Err("loaded City adapter or caller differs");
            }
        }
        for &(rva, target) in &self.slots {
            let mut bytes = [0; 8];
            memory.copy(base + rva, &mut bytes)?;
            if u64::from_le_bytes(bytes) != base + target {
                return Err("loaded effect dispatch differs");
            }
        }
        Ok(())
    }
}

/// Read-only audit of the fixed on-disk core, for the owning controller before
/// any game is started. Zero means the complete source plan was constructed,
/// never that a loaded image, game quantity or installation was accepted.
#[cfg(all(windows, target_arch = "x86_64"))]
#[unsafe(no_mangle)]
pub extern "system" fn OathRuntimeAuditSource() -> u32 {
    std::panic::catch_unwind(|| {
        use std::io::Read;
        let Ok(file) = std::fs::File::open(CORE_PATH) else {
            return 1;
        };
        let mut bytes = Vec::with_capacity(FILE_BYTES);
        if file
            .take(FILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .is_err()
        {
            return 1;
        }
        if SourcePlan::from_disk(&bytes).is_err() {
            return 2;
        }
        0
    })
    .unwrap_or(99)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    struct Fixture(BTreeMap<u64, u8>);
    impl Fixture {
        fn put(&mut self, at: u64, data: &[u8]) {
            self.0
                .extend(data.iter().enumerate().map(|(i, b)| (at + i as u64, *b)));
        }
    }
    impl ImageMemory for Fixture {
        fn copy(&mut self, at: u64, data: &mut [u8]) -> Result<(), &'static str> {
            for (i, byte) in data.iter_mut().enumerate() {
                *byte = *self.0.get(&(at + i as u64)).ok_or("fixture read failed")?;
            }
            Ok(())
        }
    }
    fn plan_and_memory(base: u64) -> (SourcePlan, Fixture) {
        let windows = vec![
            Window {
                rva: 0x8a75db,
                original: vec![0xcc; 5],
            },
            Window {
                rva: 0x944e52,
                original: vec![0xff, 0x50, 0x28, 0x84, 0xc0],
            },
        ];
        let slots = vec![(0xa76a90, 0x817810), (0xa76a98, 0x8188a0)];
        let mut memory = Fixture(BTreeMap::new());
        for window in &windows {
            memory.put(base + window.rva, &window.original);
        }
        for &(rva, target) in &slots {
            memory.put(base + rva, &(base + target).to_le_bytes());
        }
        (SourcePlan { windows, slots }, memory)
    }
    #[test]
    fn relocated_pointer_slots_are_checked_and_swaps_are_rejected() {
        for base in [0x180000000, 0x7ffb12340000] {
            let (plan, mut memory) = plan_and_memory(base);
            plan.validate_image(base, &mut memory).unwrap();
            memory.put(base + 0xa76a90, &(base + 0x8188a0).to_le_bytes());
            assert!(plan.validate_image(base, &mut memory).is_err());
        }
    }
    #[test]
    fn changed_prefix_caller_bytes_missing_copy_and_bad_base_are_rejected() {
        for rva in [0x8a75db, 0x944e52] {
            let (plan, mut memory) = plan_and_memory(0x180000000);
            memory.put(0x180000000 + rva, &[0x90]);
            assert!(plan.validate_image(0x180000000, &mut memory).is_err());
        }
        let (plan, mut memory) = plan_and_memory(0x180000000);
        memory.0.remove(&(0x180000000 + 0xa76a91));
        assert!(plan.validate_image(0x180000000, &mut memory).is_err());
        assert!(plan.validate_image(0, &mut memory).is_err());
        assert!(plan.validate_image(!0xffffu64, &mut memory).is_err());
    }
    #[test]
    fn source_identity_and_range_failures_are_not_a_matching_plan() {
        assert!(SourcePlan::from_disk(&[]).is_err());
        assert!(SourcePlan::from_disk(&vec![0; FILE_BYTES]).is_err());
        assert!(disk_range(&[], u64::MAX, 8).is_err());
        assert!(disk_range(&vec![0; 512], 0x8a75e0, 0x58).is_err());
        assert!(disk_range(&[], 0x9b6620, 8).is_err());
        assert!(disk_range(&[], 0xb5f120, 8).is_err());
    }
}
