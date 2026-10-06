//! 5E2D0 owns the reader through the last Player read; 4012F0 borrows it.
//! 401050 still reads its owner after 4012F0 returns, so that inner frame alone
//! cannot authorize destruction. Both entries remain installed while closed.
use super::super::producer::{self, Producer};
use super::*;

#[unsafe(no_mangle)]
static AI_ROOT_PRODUCER: Producer = Producer::new();
const OUTER: u64 = 0x5e2d0;
const INNER: u64 = 0x4012f0;
const HEAD: [u8; 5] = [0x48, 0x89, 0x5c, 0x24, 0x08];
const INNER_HEAD: [u8; 5] = [0x48, 0x89, 0x5c, 0x24, 0x20];
struct Entry {
    address: u64,
    prefix: u64,
    callers: [u64; 2],
}
struct Hook {
    entries: Vec<Entry>,
    _registration: Registration,
}
static HOOK: OnceLock<Hook> = OnceLock::new();

pub(super) fn installed() -> bool {
    HOOK.get().is_some_and(|h| {
        h._registration.verified_installed()
            && h.entries.iter().all(|e| {
                Memory.copy::<2>(e.address) == Some([0xeb, 0xf9])
                    && u64_at(e.address - 8) == Some(e.prefix)
            })
    })
}
pub(crate) fn close_producer() -> Result<(), Failure> {
    if HOOK.get().is_none() {
        return Ok(());
    }
    AI_ROOT_PRODUCER
        .close()
        .map_err(|_| Failure::CallerNotQuiescent)?;
    if !installed() {
        // Failed publication never opened a producer generation. Recover its
        // original entry bytes/attributes, retaining all published E9 pads.
        let hook = HOOK.get().ok_or(Failure::CriticalRestore)?;
        hook._registration.close()?;
        if AI_ROOT_PRODUCER.snapshot() >> 32 != 0 {
            return Err(Failure::CriticalRestore);
        }
    }
    Ok(())
}
pub(crate) fn rebuild_producer(generation: u32) -> Result<(), Failure> {
    if HOOK.get().is_none() {
        return if cfg!(feature = "ai-producer-candidate") {
            Err(Failure::Lease)
        } else {
            Ok(())
        };
    }
    if !installed() {
        return Err(Failure::CriticalRestore);
    }
    AI_ROOT_PRODUCER
        .rebuild(generation)
        .map_err(|_| Failure::Lease)
}
pub(super) fn stopped() -> bool {
    installed() && AI_ROOT_PRODUCER.closed_and_drained()
}
pub(super) fn current() -> Option<u32> {
    producer::current_generation()
}
pub(super) fn player() -> Option<u64> {
    producer::current_player()
}

/// # Safety
/// The owning bootstrap must install before the first Player producer. An
/// already running pre-install native frame cannot be retroactively acquired.
/// GameCore and bridge are pinned by the runtime. No code is freed on Close.
pub(crate) unsafe fn install_producer(base: u64, generation: u32) -> Result<(), Failure> {
    let guard = INSTALL.lock().map_err(|_| Failure::Patch)?;
    if HOOK.get().is_some() {
        return Err(Failure::AlreadyUsed);
    }
    if base == 0 || !civ6_native_bridge_rs::supports_atomic_promotion_fetch() {
        return Err(Failure::Identity);
    }
    let image = std::fs::read("C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll").map_err(|_| Failure::Identity)?;
    super::super::pinned::audit(&image).map_err(|_| Failure::Identity)?;
    for rva in [
        OUTER - 8,
        INNER - 8,
        0x349cc,
        0x4b16d,
        0x401145,
        0x344fb5,
        0x401510,
    ] {
        let actual = Memory
            .copy::<16>(base.checked_add(rva).ok_or(Failure::Range)?)
            .ok_or(Failure::Memory)?;
        if actual.as_slice() != &image[(rva - 0xc00) as usize..(rva - 0xc00 + 16) as usize] {
            return Err(Failure::Identity);
        }
    }
    unsafe {
        prepare_set(
            &[
                (base + OUTER, HEAD, [base + 0x349d1, base + 0x4b172]),
                (base + INNER, INNER_HEAD, [base + 0x40114a, base + 0x5cacd]),
            ],
            generation,
            &guard,
        )
    }
}

unsafe fn prepare_set(
    definitions: &[(u64, [u8; 5], [u64; 2])],
    generation: u32,
    guard: &std::sync::MutexGuard<'_, ()>,
) -> Result<(), Failure> {
    if generation == 0 || generation >= (1 << 31) {
        return Err(Failure::Lease);
    }
    if definitions.is_empty() || definitions.len() > 2 {
        return Err(Failure::Identity);
    }
    let mut entries = Vec::new();
    let mut patches = Vec::new();
    for (index, &(entry, head, callers)) in definitions.iter().enumerate() {
        if entry < 8
            || entry % 8 != 0
            || callers.contains(&0)
            || Memory.copy::<8>(entry - 8) != Some([0xcc; 8])
            || Memory.copy::<5>(entry) != Some(head)
        {
            return Err(Failure::Identity);
        }
        let target = if index == 0 {
            ai_producer_gateway as *const () as usize
        } else {
            ai_producer_inner_gateway as *const () as usize
        };
        let page = unsafe { relay((entry - 5) as usize, target) }?;
        let trampoline = page + 0x20;
        // MOV [RSP+home], RBX changes neither RSP nor RBX. Its relocated leaf
        // unwinds to our compiled gateway; original+5 resumes the exact pushes.
        let displacement = i32::try_from(i128::from(entry + 5) - (trampoline + 10) as i128)
            .map_err(|_| Failure::Range)?;
        let mut code = [0u8; 10];
        code[..5].copy_from_slice(&head);
        code[5] = 0xe9;
        code[6..].copy_from_slice(&displacement.to_le_bytes());
        let mut old = 0;
        if unsafe { VirtualProtect(page as *const _, 4096, PAGE_READWRITE, &mut old) } == 0 {
            return Err(Failure::Patch);
        }
        unsafe { ptr::copy_nonoverlapping(code.as_ptr(), trampoline as *mut u8, code.len()) };
        if unsafe { VirtualProtect(page as *const _, 4096, PAGE_EXECUTE_READ, &mut old) } == 0
            || unsafe {
                FlushInstructionCache(GetCurrentProcess(), page as *const _, 0x20 + code.len())
            } == 0
        {
            return Err(Failure::Patch);
        }
        AI_GATE_TARGETS[index + 4].store(trampoline as u64, Ordering::Release);
        let mut prefix = [0xcc; 8];
        prefix[3] = 0xe9;
        prefix[4..].copy_from_slice(&operand((entry - 5) as usize, page)?.to_le_bytes());
        let prefix = u64::from_le_bytes(prefix);
        patches.extend([
            Patch {
                address: (entry - 8) as usize,
                old: u64::from_le_bytes([0xcc; 8]),
                new: prefix,
                width: Width::PermanentPrefix,
            },
            Patch {
                address: entry as usize,
                old: 0x8948,
                new: 0xf9eb,
                width: Width::Half,
            },
        ]);
        entries.push(Entry {
            address: entry,
            prefix,
            callers,
        });
    }
    let registration = Registration::prepare_locked(patches, guard)?;
    HOOK.set(Hook {
        entries,
        _registration: registration.clone(),
    })
    .map_err(|_| Failure::AlreadyUsed)?;
    if registration.install_locked(guard, &SystemApi).is_err() {
        let _ = registration.close_locked(guard, &SystemApi);
        return Err(Failure::CriticalRestore);
    }
    if AI_ROOT_PRODUCER.rebuild(generation).is_err() {
        let _ = registration.close_locked(guard, &SystemApi);
        return Err(Failure::CriticalRestore);
    }
    Ok(())
}

fn copy_entry(raw: &mut Registers, index: usize) -> Option<u64> {
    THREAD_FLIGHTS.with(|c| c.set(c.get().saturating_add(1)));
    let snapshot = raw.ticket;
    raw.ticket = 0;
    let entry = HOOK.get()?.entries.get(index)?;
    if !entry.callers.contains(&raw.return_address) {
        return None;
    }
    let source = raw
        .caller_rsp
        .checked_add(8)
        .and_then(|p| Memory.copy::<0x60>(p))?;
    unsafe {
        ptr::copy_nonoverlapping(
            source.as_ptr(),
            (raw as *mut Registers).cast::<u8>().add(0xc0),
            0x60,
        )
    };
    Some(snapshot)
}
pub(super) extern "system" fn producer_observe(raw: *mut Registers, after: u32) {
    let raw = unsafe { &mut *raw };
    if after != 0 {
        if raw.ticket != 0 {
            producer::leave();
        }
        THREAD_FLIGHTS.with(|c| c.set(c.get().saturating_sub(1)));
    } else if let Some(snapshot) = copy_entry(raw, 0) {
        if let Ok(generation) = producer::enter(&AI_ROOT_PRODUCER, snapshot, raw.rcx) {
            raw.ticket = u64::from(generation);
        }
    }
}
pub(super) extern "system" fn inner_observe(raw: *mut Registers, after: u32) {
    let raw = unsafe { &mut *raw };
    if after != 0 {
        // Inner normal/SEH returns must not release the outer reader early.
        THREAD_FLIGHTS.with(|c| c.set(c.get().saturating_sub(1)));
    } else if copy_entry(raw, 1).is_some() {
        if let Some(generation) = producer::current_generation() {
            raw.ticket = u64::from(generation);
        }
    }
}
pub(super) unsafe extern "system" fn producer_unwind(
    record: *const windows_sys::Win32::System::Diagnostics::Debug::EXCEPTION_RECORD,
    frame: usize,
    context: *mut windows_sys::Win32::System::Diagnostics::Debug::CONTEXT,
    dispatcher: *mut std::ffi::c_void,
) -> i32 {
    if !record.is_null() && unsafe { (*record).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0 {
        producer::leave();
    }
    unsafe { unwind_observer(record, frame, context, dispatcher) }
}

#[cfg(test)]
#[path = "producer_hook_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) unsafe fn fixture(target: usize, generation: u32) -> usize {
    let entry = unsafe { tests::image(target) };
    let guard = INSTALL.lock().unwrap();
    let ret = super::tests::ai_fixture_return as *const () as u64;
    unsafe { prepare_set(&[(entry as u64, HEAD, [ret, ret])], generation, &guard) }.unwrap();
    entry
}
#[cfg(test)]
pub(crate) fn call_fixture(entry: usize) -> u64 {
    tests::call_fixture(entry)
}
