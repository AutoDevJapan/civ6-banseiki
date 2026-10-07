//! Diagnostic-only, process-memory correction of the audited XP2 removal paths.
//! The runner must call `validate` with its observed mode before calling `apply`.
//! No quantity is inferred from this action, and no game state is compensated.
//! Review the quantity contract together with this module: the original native
//! positive conversion is Q8 (10%=25/256, 15%=38/256), not decimal 0.10/0.15.
use crate::{Error, Result};
use serde_json::{json, Value};

pub const MODULE_SHA256: &str = "324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c";
pub const MODULE_NAME: &str = "GameCore_XP2_FinalRelease.dll";
const IMAGE_SIZE: usize = 0xc60000;
const HEADER_SIZE: usize = 0x400;
const FILE_SIZE: usize = 12_940_384;
const TIMESTAMP: u32 = 0x667c6f5b;

#[derive(Clone, Copy, Debug)]
struct Patch {
    name: &'static str,
    rva: usize,
    original: &'static [u8],
    replacement: &'static [u8],
    write_offset: usize,
    write_len: usize,
}

const PATCHES: [Patch; 4] = [
    Patch {
        name: "military_remove_neg",
        rva: 0x818a80,
        original: &[0xf7, 0xd8],
        replacement: &[0x90, 0x90],
        write_offset: 0,
        write_len: 2,
    },
    Patch {
        name: "military_remove_sub",
        rva: 0x818ab5,
        original: &[0x01, 0x0c, 0xaa],
        replacement: &[0x29, 0x0c, 0xaa],
        write_offset: 0,
        write_len: 3,
    },
    Patch {
        name: "growth_remove_positive_multiplier",
        rva: 0x8d97aa,
        original: &[0xf3, 0x0f, 0x59, 0x35, 0x8e, 0x0c, 0x12, 0x00],
        replacement: &[0xf3, 0x0f, 0x59, 0x35, 0xc6, 0xf1, 0x0e, 0x00],
        // Validate the whole instruction; write only its RIP displacement.
        write_offset: 4,
        write_len: 4,
    },
    Patch {
        name: "growth_remove_sub",
        rva: 0x8d97e7,
        original: &[0x01, 0x98, 0x84, 0x00, 0x00, 0x00],
        replacement: &[0x29, 0x98, 0x84, 0x00, 0x00, 0x00],
        write_offset: 0,
        write_len: 6,
    },
];

// Entire apply/remove functions: suspend-and-retry if any thread is in one.
const FUNCTIONS: [(usize, usize); 4] = [
    (0x817810, 0x817a62),
    (0x8188a0, 0x818af6),
    (0x8d8d60, 0x8d8dca),
    (0x8d9790, 0x8d97fa),
];
const CONSTANTS: [(usize, [u8; 4]); 3] = [
    (0x9c8978, [0x0a, 0xd7, 0x23, 0x3c]),
    (0x9fa440, [0x0a, 0xd7, 0x23, 0xbc]),
    (0x9c1b78, [0x00, 0x00, 0x80, 0x43]),
];

/// Parent integration gate. These booleans must come from runner/game checks,
/// not from arbitrary fields supplied inside the action itself.
pub(crate) fn validate(test_mode: bool, diagnostic_only: bool, multiplayer: bool) -> Result<()> {
    if !test_mode || !diagnostic_only || multiplayer {
        return Err(Error::scenario(
            "exact removal requires a test-mode, diagnostic-only single-player game",
        ));
    }
    Ok(())
}

/// Reviewable immutable instruction plan; it performs no native operation.
pub fn plan() -> Value {
    json!({
        "diagnostic_only": true, "accepted": false, "quantity_verified": false,
        "memory_only": true, "platform": "windows-x86_64", "module": MODULE_NAME,
        "module_sha256": MODULE_SHA256, "image_size": IMAGE_SIZE,
        "file_size": FILE_SIZE, "deadline_seconds": 10, "max_attempts": 16,
        "patches": PATCHES.iter().map(|p| json!({
            "name": p.name, "instruction_rva": p.rva,
            "original": p.original, "replacement": p.replacement,
            "write_rva": p.rva + p.write_offset, "write_bytes": p.write_len
        })).collect::<Vec<_>>(),
        "excluded_rip_ranges": FUNCTIONS,
        "shared_constants_unchanged": CONSTANTS.iter().map(|(rva, bytes)|
            json!({"rva": rva, "bytes": bytes})).collect::<Vec<_>>()
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchState {
    Original,
    Patched,
}

/// Pure preflight for the hash, remote PE identity and ALL instruction sites.
/// `apply` calculates the hash from a pinned disk handle and obtains the other
/// arguments from the suspended, pinned process; caller strings alone are not
/// authorization to write memory.
pub fn validate_snapshot(
    sha256: &str,
    remote_headers: &[u8],
    instructions: &[Vec<u8>],
) -> Result<PatchState> {
    if !sha256.eq_ignore_ascii_case(MODULE_SHA256) {
        return Err(Error::input("unknown GameCore module SHA256"));
    }
    let u16_at = |off| -> Option<u16> {
        Some(u16::from_le_bytes(
            remote_headers.get(off..off + 2)?.try_into().ok()?,
        ))
    };
    let u32_at = |off| -> Option<u32> {
        Some(u32::from_le_bytes(
            remote_headers.get(off..off + 4)?.try_into().ok()?,
        ))
    };
    if remote_headers.len() != HEADER_SIZE
        || remote_headers.get(..2) != Some(b"MZ".as_slice())
        || u32_at(0x3c) != Some(0x108)
        || remote_headers.get(0x108..0x10c) != Some(b"PE\0\0".as_slice())
        || u16_at(0x10c) != Some(0x8664)
        || u16_at(0x10e) != Some(8)
        || u32_at(0x110) != Some(TIMESTAMP)
        || u16_at(0x11c) != Some(240)
        || u16_at(0x11e) != Some(0x2022)
        || u16_at(0x120) != Some(0x20b)
        || u32_at(0x158) != Some(IMAGE_SIZE as u32)
        || u32_at(0x15c) != Some(HEADER_SIZE as u32)
    {
        return Err(Error::input("remote PE is not the audited AMD64 XP2 DLL"));
    }
    classify(instructions)
}

fn classify(instructions: &[Vec<u8>]) -> Result<PatchState> {
    if instructions.len() != PATCHES.len() {
        return Err(Error::input(
            "exact removal instruction snapshot is incomplete",
        ));
    }
    let mut original = 0;
    let mut patched = 0;
    for (p, observed) in PATCHES.iter().zip(instructions) {
        if observed.as_slice() == p.original {
            original += 1;
        } else if observed.as_slice() == p.replacement {
            patched += 1;
        } else {
            return Err(Error::input(format!(
                "unknown instruction bytes at {}",
                p.name
            )));
        }
    }
    match (original, patched) {
        (4, 0) => Ok(PatchState::Original),
        (0, 4) => Ok(PatchState::Patched),
        _ => Err(Error::input(
            "partially patched exact-removal instructions refused",
        )),
    }
}

pub(crate) fn apply(game: crate::startup::GameProcess) -> Result<Value> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        platform::apply(game)
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = game;
        Err(Error::input("exact removal supports only Windows AMD64"))
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[path = "oath_exact_removal_windows.rs"]
mod platform;

// This engine is shared with the native backend, so failures are exercised with
// the same transaction order without starting or attaching to a game.
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
mod transaction {
    use super::*;

    pub(super) trait Backend {
        fn check(&mut self) -> Result<()>;
        fn suspend(&mut self) -> Result<()>;
        fn resume(&mut self) -> Result<()>;
        fn busy(&mut self) -> Result<bool>;
        fn verify(&mut self) -> Result<PatchState>;
        fn read(&mut self, rva: usize, len: usize) -> Result<Vec<u8>>;
        fn writable(&mut self, rva: usize, len: usize) -> Result<u32>;
        fn restore(&mut self, rva: usize, len: usize, protection: u32) -> Result<()>;
        fn write(&mut self, rva: usize, bytes: &[u8]) -> Result<()>;
        fn flush(&mut self, rva: usize, len: usize) -> Result<()>;
        // A failed resume might have resumed some threads. Freeze again before
        // rollback, or report failure so only the already-pinned game is killed.
        fn prepare_rollback(&mut self) -> Result<()>;
        fn terminate(&mut self) -> Result<()>;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum Outcome {
        Busy,
        Applied,
        AlreadyPatched,
    }

    struct Entry {
        patch: Patch,
        protection: u32,
        attempted: bool,
    }

    struct Session<'a, B: Backend> {
        backend: &'a mut B,
        journal: Vec<Entry>,
        finished: bool,
    }

    impl<B: Backend> Session<'_, B> {
        fn work(&mut self) -> Result<Outcome> {
            self.backend.check()?;
            if self.backend.busy()? {
                return Ok(Outcome::Busy);
            }
            let state = self.backend.verify()?; // Includes all bytes, while suspended.
            if state == PatchState::Patched {
                return Ok(Outcome::AlreadyPatched);
            }
            for patch in PATCHES {
                self.backend.check()?;
                if self.backend.read(patch.rva, patch.original.len())? != patch.original {
                    return Err(Error::input(
                        "instruction changed after suspended preflight",
                    ));
                }
                let rva = patch.rva + patch.write_offset;
                let protection = self.backend.writable(rva, patch.write_len)?;
                // Capacity is reserved before any protection change. Journal
                // BEFORE WriteProcessMemory: a failed/short write can mutate.
                self.journal.push(Entry {
                    patch,
                    protection,
                    attempted: false,
                });
                self.backend.check()?;
                self.journal
                    .last_mut()
                    .expect("entry just inserted")
                    .attempted = true;
                self.backend.write(
                    rva,
                    &patch.replacement[patch.write_offset..patch.write_offset + patch.write_len],
                )?;
                self.backend.flush(rva, patch.write_len)?;
                if self.backend.read(patch.rva, patch.replacement.len())? != patch.replacement {
                    return Err(Error::input("exact removal write readback differs"));
                }
                self.backend.restore(rva, patch.write_len, protection)?;
            }
            self.backend.check()?;
            if self.backend.verify()? != PatchState::Patched {
                return Err(Error::input(
                    "exact removal final snapshot is not fully patched",
                ));
            }
            self.backend.check()?;
            Ok(Outcome::Applied)
        }

        fn rollback(&mut self) -> Vec<String> {
            let mut failures = Vec::new();
            if self.journal.is_empty() {
                return failures;
            }
            if let Err(e) = self.backend.prepare_rollback() {
                failures.push(format!("cannot freeze for rollback: {e}"));
                return failures;
            }
            for entry in self.journal.iter().rev() {
                let p = entry.patch;
                let rva = p.rva + p.write_offset;
                if entry.attempted {
                    match self.backend.writable(rva, p.write_len) {
                        Ok(_) => {
                            let old = &p.original[p.write_offset..p.write_offset + p.write_len];
                            if let Err(e) = self.backend.write(rva, old) {
                                failures.push(format!("{} rollback write: {e}", p.name));
                            }
                            if let Err(e) = self.backend.flush(rva, p.write_len) {
                                failures.push(format!("{} rollback flush: {e}", p.name));
                            }
                            match self.backend.read(p.rva, p.original.len()) {
                                Ok(bytes) if bytes == p.original => {}
                                other => failures
                                    .push(format!("{} rollback readback: {other:?}", p.name)),
                            }
                        }
                        Err(e) => failures.push(format!("{} rollback protection: {e}", p.name)),
                    }
                }
                // Always attempt to restore the FIRST protection, even after a
                // write/flush failure or a failed second writable() call.
                if let Err(e) = self.backend.restore(rva, p.write_len, entry.protection) {
                    failures.push(format!("{} protection restore: {e}", p.name));
                }
            }
            failures
        }

        fn abort(&mut self, cause: Error) -> Error {
            let rollback_errors = self.rollback();
            let mut terminate_error = None;
            let mut terminated = false;
            if !rollback_errors.is_empty() {
                match self.backend.terminate() {
                    Ok(()) => terminated = true,
                    Err(e) => terminate_error = Some(e.to_string()),
                }
            }
            let resume_error = self.backend.resume().err().map(|e| e.to_string());
            if resume_error.is_some() {
                if !terminated {
                    match self.backend.terminate() {
                        Ok(()) => terminated = true,
                        Err(e) => terminate_error = Some(e.to_string()),
                    }
                }
                // Resume is still attempted after termination, including when
                // it failed; never leave suspension ownership implicit.
                let _ = self.backend.resume();
            }
            self.finished = true;
            Error::new(
                cause.kind,
                json!({
                    "action": "oath_exact_removal", "status": "refused",
                    "diagnostic_only": true, "accepted": false, "quantity_verified": false,
                    "reason": cause.to_string(), "attempted_writes":
                        self.journal.iter().filter(|e| e.attempted).count(),
                    "rollback_completed": rollback_errors.is_empty(),
                    "rollback_errors": rollback_errors, "resume_error": resume_error,
                    "terminated_pinned_game": terminated, "terminate_error": terminate_error
                })
                .to_string(),
            )
        }
    }

    impl<B: Backend> Drop for Session<'_, B> {
        fn drop(&mut self) {
            if !self.finished {
                // Panic/unwind cleanup also ignores cancellation and attempts
                // every journaled byte/protection before resuming.
                let failed = !self.rollback().is_empty();
                if failed {
                    let _ = self.backend.terminate();
                }
                if self.backend.resume().is_err() {
                    let _ = self.backend.terminate();
                    let _ = self.backend.resume();
                }
            }
        }
    }

    pub(super) fn attempt<B: Backend>(backend: &mut B) -> Result<Outcome> {
        backend.check()?;
        let mut session = Session {
            backend,
            journal: Vec::with_capacity(PATCHES.len()),
            finished: false,
        };
        if let Err(error) = session.backend.suspend() {
            return Err(session.abort(error));
        }
        let result = session.work();
        match result {
            Ok(outcome) => match session.backend.resume() {
                Ok(()) => {
                    session.finished = true;
                    Ok(outcome)
                }
                Err(e) => Err(session.abort(e)),
            },
            Err(e) => Err(session.abort(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{transaction::*, *};
    use std::{
        collections::BTreeMap,
        panic::{catch_unwind, AssertUnwindSafe},
    };

    fn headers() -> Vec<u8> {
        let mut bytes = vec![0; HEADER_SIZE];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x108u32.to_le_bytes());
        bytes[0x108..0x10c].copy_from_slice(b"PE\0\0");
        for (offset, value) in [
            (0x10c, 0x8664u16),
            (0x10e, 8),
            (0x11c, 240),
            (0x11e, 0x2022),
            (0x120, 0x20b),
        ] {
            bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        for (offset, value) in [
            (0x110, TIMESTAMP),
            (0x158, IMAGE_SIZE as u32),
            (0x15c, HEADER_SIZE as u32),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    struct Mock {
        memory: BTreeMap<usize, u8>,
        protections: BTreeMap<usize, u32>,
        writes: Vec<(usize, Vec<u8>)>,
        events: Vec<&'static str>,
        sha: String,
        header: Vec<u8>,
        suspended: bool,
        busy: bool,
        terminated: bool,
        fail_write: Option<usize>,
        fail_flush: bool,
        fail_restore_once: bool,
        fail_rollback: bool,
        fail_resume_once: bool,
        fail_suspend_once: bool,
        fail_protect_once: bool,
        corrupt_write_once: bool,
        interrupt_after_protect: bool,
        panic_write: bool,
    }
    impl Mock {
        fn new() -> Self {
            let mut memory = BTreeMap::new();
            for p in PATCHES {
                for (offset, b) in p.original.iter().enumerate() {
                    memory.insert(p.rva + offset, *b);
                }
            }
            Self {
                memory,
                protections: [(0x818000, 0x20), (0x8d9000, 0x20)].into(),
                writes: vec![],
                events: vec![],
                sha: MODULE_SHA256.into(),
                header: headers(),
                suspended: false,
                busy: false,
                terminated: false,
                fail_write: None,
                fail_flush: false,
                fail_restore_once: false,
                fail_rollback: false,
                fail_resume_once: false,
                fail_suspend_once: false,
                fail_protect_once: false,
                corrupt_write_once: false,
                interrupt_after_protect: false,
                panic_write: false,
            }
        }
        fn sites(&self) -> Vec<Vec<u8>> {
            PATCHES
                .iter()
                .map(|p| {
                    (0..p.original.len())
                        .map(|i| self.memory[&(p.rva + i)])
                        .collect()
                })
                .collect()
        }
        fn restored(&self) {
            assert_eq!(classify(&self.sites()), Ok(PatchState::Original));
            assert!(self.protections.values().all(|v| *v == 0x20));
            assert!(!self.suspended);
        }
    }
    impl Backend for Mock {
        fn check(&mut self) -> Result<()> {
            if self.interrupt_after_protect && self.events.contains(&"protect") {
                Err(Error::interrupted("cancelled after protection change"))
            } else {
                Ok(())
            }
        }
        fn suspend(&mut self) -> Result<()> {
            self.events.push("suspend");
            self.suspended = true;
            if std::mem::take(&mut self.fail_suspend_once) {
                return Err(Error::input("partial suspension failure"));
            }
            Ok(())
        }
        fn resume(&mut self) -> Result<()> {
            self.events.push("resume");
            if std::mem::take(&mut self.fail_resume_once) {
                return Err(Error::input("resume failure"));
            }
            self.suspended = false;
            Ok(())
        }
        fn busy(&mut self) -> Result<bool> {
            assert!(self.suspended);
            Ok(self.busy)
        }
        fn verify(&mut self) -> Result<PatchState> {
            assert!(self.suspended);
            validate_snapshot(&self.sha, &self.header, &self.sites())
        }
        fn read(&mut self, rva: usize, len: usize) -> Result<Vec<u8>> {
            assert!(self.suspended);
            Ok((0..len).map(|i| self.memory[&(rva + i)]).collect())
        }
        fn writable(&mut self, rva: usize, _: usize) -> Result<u32> {
            assert!(self.suspended);
            self.events.push("protect");
            if std::mem::take(&mut self.fail_protect_once) {
                return Err(Error::input("protection change failure"));
            }
            Ok(self.protections.insert(rva & !0xfff, 0x40).unwrap())
        }
        fn restore(&mut self, rva: usize, _: usize, protection: u32) -> Result<()> {
            self.events.push("restore");
            if std::mem::take(&mut self.fail_restore_once) {
                return Err(Error::input("restore failure"));
            }
            self.protections.insert(rva & !0xfff, protection);
            Ok(())
        }
        fn write(&mut self, rva: usize, bytes: &[u8]) -> Result<()> {
            assert!(self.suspended);
            assert_eq!(self.protections[&(rva & !0xfff)], 0x40);
            self.events.push("write");
            self.writes.push((rva, bytes.to_vec()));
            if self.panic_write {
                self.panic_write = false;
                self.memory.insert(rva, bytes[0]);
                panic!("write panic");
            }
            if self.fail_write == Some(self.writes.len()) {
                self.memory.insert(rva, bytes[0]); // A failed native call can have changed bytes.
                self.fail_write = None;
                self.events.push("write_failed");
                return Err(Error::input("short WriteProcessMemory"));
            }
            if self.fail_rollback && self.events.contains(&"write_failed") {
                return Err(Error::input("rollback write failure"));
            }
            for (i, b) in bytes.iter().enumerate() {
                self.memory.insert(rva + i, *b);
            }
            if std::mem::take(&mut self.corrupt_write_once) {
                self.memory.insert(rva, bytes[0] ^ 0xff);
            }
            Ok(())
        }
        fn flush(&mut self, _: usize, _: usize) -> Result<()> {
            self.events.push("flush");
            if std::mem::take(&mut self.fail_flush) {
                Err(Error::input("flush failure"))
            } else {
                Ok(())
            }
        }
        fn prepare_rollback(&mut self) -> Result<()> {
            self.events.push("rollback");
            self.suspended = true;
            Ok(())
        }
        fn terminate(&mut self) -> Result<()> {
            self.events.push("terminate");
            self.terminated = true;
            Ok(())
        }
    }

    #[test]
    fn only_test_mode_single_player_diagnostics_are_admitted() {
        assert!(validate(true, true, false).is_ok());
        for mode in [
            (false, true, false),
            (true, false, false),
            (true, true, true),
        ] {
            assert!(validate(mode.0, mode.1, mode.2).is_err());
        }
    }
    #[test]
    fn audited_float_packing_and_inverse_keep_other_modifiers() {
        // Emulate the native floorf/fraction/cvttss2si packing, with independent
        // raw-value fixtures taken from the instruction investigation.
        let pack = |percent: i32| {
            let x = percent as f32 * 0.01f32;
            let integer = x.floor();
            ((integer as i32) << 8) | (((x - integer) * 256.0).trunc() as i32 & 255)
        };
        for (amount, added, removed) in [
            (1, 2, -3),
            (5, 12, -13),
            (10, 25, -26),
            (15, 38, -39),
            (25, 64, -64),
            (100, 256, -256),
        ] {
            assert_eq!(pack(amount), added);
            assert_eq!(pack(-amount), removed);
            let mut legacy = 307; // A nonzero independent production multiplier.
            let mut corrected = legacy;
            for _ in 0..16 {
                legacy += added + removed;
                corrected += added;
                corrected -= added;
            }
            assert_eq!(corrected, 307);
            assert_eq!(legacy, 307 + 16 * (added + removed));
        }
    }
    #[test]
    fn growth_changes_only_rip_displacement_and_keeps_shared_constants() {
        let p = PATCHES[2];
        assert_eq!(&p.original[..4], &p.replacement[..4]);
        let target = |bytes: &[u8]| {
            (p.rva + 8) as i64 + i32::from_le_bytes(bytes[4..8].try_into().unwrap()) as i64
        };
        assert_eq!(target(p.original), 0x9fa440);
        assert_eq!(target(p.replacement), 0x9c8978);
        assert_eq!(p.write_offset, 4);
        assert_eq!(p.write_len, 4);
        assert!(CONSTANTS.iter().all(|(rva, _)| PATCHES
            .iter()
            .all(|p| *rva < p.rva || *rva >= p.rva + p.original.len())));
    }
    #[test]
    fn unknown_hash_pe_instruction_or_partial_state_has_zero_writes() {
        for fixture in 0..5 {
            let mut m = Mock::new();
            match fixture {
                0 => m.sha = "00".repeat(32),
                1 => m.header[0x10c] = 0,
                2 => {
                    m.memory.insert(PATCHES[0].rva, 0xcc);
                }
                3 => {
                    for (i, b) in PATCHES[0].replacement.iter().enumerate() {
                        m.memory.insert(PATCHES[0].rva + i, *b);
                    }
                }
                _ => {
                    m.header.truncate(0x100);
                }
            }
            assert!(attempt(&mut m).is_err());
            assert!(m.writes.is_empty());
            assert!(!m.events.contains(&"protect"));
            assert!(!m.suspended);
            assert!(!m.terminated);
        }
    }
    #[test]
    fn all_four_apply_then_idempotence_performs_zero_additional_writes() {
        let mut m = Mock::new();
        assert_eq!(attempt(&mut m).unwrap(), Outcome::Applied);
        assert_eq!(m.writes.len(), 4);
        assert_eq!(m.writes[2].0, 0x8d97ae);
        assert_eq!(m.writes[2].1, [0xc6, 0xf1, 0x0e, 0]);
        assert_eq!(classify(&m.sites()), Ok(PatchState::Patched));
        assert!(!m.suspended);
        assert!(m.protections.values().all(|v| *v == 0x20));
        assert_eq!(attempt(&mut m).unwrap(), Outcome::AlreadyPatched);
        assert_eq!(m.writes.len(), 4);
    }
    #[test]
    fn each_failed_short_write_restores_every_attempted_site_and_protection() {
        for site in 1..=4 {
            let mut m = Mock::new();
            m.fail_write = Some(site);
            let error = attempt(&mut m).unwrap_err();
            let report: Value = serde_json::from_str(&error.message).unwrap();
            assert_eq!(report["rollback_completed"], true);
            assert_eq!(report["attempted_writes"], site);
            m.restored();
            assert!(!m.terminated);
            assert_eq!(m.writes.len(), site * 2);
            let last_resume = m.events.iter().rposition(|v| *v == "resume").unwrap();
            assert!(m.events[..last_resume].contains(&"restore"));
        }
    }
    #[test]
    fn flush_protection_and_resume_failures_all_rollback() {
        for fixture in 0..3 {
            let mut m = Mock::new();
            match fixture {
                0 => m.fail_flush = true,
                1 => m.fail_restore_once = true,
                _ => m.fail_resume_once = true,
            }
            assert!(attempt(&mut m).is_err());
            m.restored();
            assert!(!m.terminated);
        }
    }
    #[test]
    fn interruption_after_protection_does_not_write_and_still_restores() {
        let mut m = Mock::new();
        m.interrupt_after_protect = true;
        assert!(attempt(&mut m).is_err());
        assert!(m.writes.is_empty());
        m.restored();
    }
    #[test]
    fn rollback_failure_terminates_pinned_game_and_attempts_resume() {
        let mut m = Mock::new();
        m.fail_write = Some(2);
        m.fail_rollback = true;
        let error = attempt(&mut m).unwrap_err();
        let report: Value = serde_json::from_str(&error.message).unwrap();
        assert_eq!(report["rollback_completed"], false);
        assert_eq!(report["terminated_pinned_game"], true);
        assert!(m.terminated);
        assert!(!m.suspended);
        assert!(m.protections.values().all(|v| *v == 0x20));
        assert_eq!(m.events.last(), Some(&"resume"));
    }
    #[test]
    fn busy_function_writes_nothing_and_resumes() {
        let mut m = Mock::new();
        m.busy = true;
        assert_eq!(attempt(&mut m).unwrap(), Outcome::Busy);
        assert!(m.writes.is_empty());
        assert!(!m.suspended);
    }
    #[test]
    fn suspension_and_protection_failure_have_no_writes_and_resume() {
        for failure in 0..2 {
            let mut m = Mock::new();
            if failure == 0 {
                m.fail_suspend_once = true;
            } else {
                m.fail_protect_once = true;
            }
            assert!(attempt(&mut m).is_err());
            assert!(m.writes.is_empty());
            m.restored();
        }
    }
    #[test]
    fn successful_native_write_with_wrong_readback_is_rolled_back() {
        let mut m = Mock::new();
        m.corrupt_write_once = true;
        assert!(attempt(&mut m).is_err());
        m.restored();
        assert_eq!(m.writes.len(), 2);
        assert!(!m.terminated);
    }
    #[test]
    fn evidence_never_claims_quantity_acceptance() {
        let evidence = plan();
        assert_eq!(evidence["accepted"], false);
        assert_eq!(evidence["quantity_verified"], false);
        assert_eq!(evidence["memory_only"], true);
        assert_eq!(evidence["patches"].as_array().unwrap().len(), 4);
    }
    #[test]
    fn unwinding_after_partial_write_rolls_back_before_resume() {
        let mut m = Mock::new();
        m.panic_write = true;
        assert!(catch_unwind(AssertUnwindSafe(|| attempt(&mut m))).is_err());
        m.restored();
        assert!(!m.terminated);
    }
}
