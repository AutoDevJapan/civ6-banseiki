//! One serialized owner of bytes, original page attributes and cache work.
//! A restored operand is only one part of restoration. Clones and recovery all
//! share the same states; the original protection is never resampled after RWX.
use super::*;
use std::sync::{Arc, MutexGuard};

pub(super) static INSTALL: Mutex<()> = Mutex::new(());
pub(super) struct Recovery(Mutex<Option<Registration>>);
impl Recovery {
    pub(super) fn get(&self) -> Result<Option<Registration>, Failure> {
        self.0
            .lock()
            .map(|slot| slot.clone())
            .map_err(|_| Failure::Patch)
    }
    pub(super) fn set(&self, registration: Registration) -> Result<(), Failure> {
        let mut slot = self.0.lock().map_err(|_| Failure::Patch)?;
        if slot.is_some() {
            return Err(Failure::AlreadyUsed);
        }
        *slot = Some(registration);
        Ok(())
    }
    pub(super) fn clear(&self) -> Result<(), Failure> {
        *self.0.lock().map_err(|_| Failure::Patch)? = None;
        Ok(())
    }
}
pub(super) static RECOVERY: Recovery = Recovery(Mutex::new(None));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Prepared,
    Installed,
    Restoring,
    Restored,
}

struct PatchState {
    patch: Patch,
    original_protection: u32,
    phase: Phase,
    bytes_pending: bool,
    protection_pending: bool,
    cache_pending: bool,
}

#[derive(Clone)]
pub struct Registration {
    states: Arc<Mutex<Vec<PatchState>>>,
}

/// Private injection seam; production always uses the actual Win32 APIs.
pub(super) trait PatchApi {
    fn protect(&self, address: usize, size: usize, protection: u32) -> bool;
    fn flush(&self, address: usize, size: usize) -> bool;
    fn before_cas(&self) {}
}
pub(super) struct SystemApi;
impl PatchApi for SystemApi {
    fn protect(&self, address: usize, size: usize, protection: u32) -> bool {
        let mut previous = 0;
        unsafe { VirtualProtect(address as *const _, size, protection, &mut previous) != 0 }
    }
    fn flush(&self, address: usize, size: usize) -> bool {
        unsafe { FlushInstructionCache(GetCurrentProcess(), address as *const _, size) != 0 }
    }
}

fn protection(address: usize, size: usize) -> Option<u32> {
    let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    let read =
        unsafe { VirtualQuery(address as *const _, &mut info, std::mem::size_of_val(&info)) };
    (read != 0
        && info.State == MEM_COMMIT
        && info.Protect & (PAGE_GUARD | PAGE_NOACCESS) == 0
        && address.checked_add(size)?
            <= (info.BaseAddress as usize).checked_add(info.RegionSize)?)
    .then_some(info.Protect)
}
fn bytes(p: &Patch) -> Option<u64> {
    match p.width {
        Width::Half => Memory
            .copy::<2>(p.address as u64)
            .map(u16::from_le_bytes)
            .map(u64::from),
        Width::Word => u32_at(p.address as u64).map(u64::from),
        Width::Wide | Width::PermanentPrefix => u64_at(p.address as u64),
    }
}
fn size(p: &Patch) -> usize {
    match p.width {
        Width::Half => 2,
        Width::Word => 4,
        Width::Wide | Width::PermanentPrefix => 8,
    }
}

impl Registration {
    pub(super) fn verified_installed(&self) -> bool {
        self.states.lock().is_ok_and(|states| {
            states.iter().all(|s| {
                s.phase == Phase::Installed
                    && !s.bytes_pending
                    && !s.protection_pending
                    && !s.cache_pending
                    && bytes(&s.patch) == Some(s.patch.new)
                    && protection(s.patch.address, size(&s.patch)) == Some(s.original_protection)
            })
        })
    }
    /// Capture ALL sites' original attributes before the first protection
    /// change. This also handles multiple operands sharing the same RX page.
    pub(super) fn prepare_locked(
        patches: Vec<Patch>,
        _guard: &MutexGuard<'_, ()>,
    ) -> Result<Self, Failure> {
        let mut states = Vec::new();
        for p in patches {
            let n = size(&p);
            if p.address % n != 0 || bytes(&p) != Some(p.old) {
                return Err(Failure::Patch);
            }
            let original_protection = protection(p.address, n).ok_or(Failure::Memory)?;
            states.push(PatchState {
                patch: p,
                original_protection,
                phase: Phase::Prepared,
                bytes_pending: false,
                protection_pending: false,
                cache_pending: false,
            });
        }
        Ok(Self {
            states: Arc::new(Mutex::new(states)),
        })
    }

    pub(super) fn install_locked(
        &self,
        _guard: &MutexGuard<'_, ()>,
        api: &impl PatchApi,
    ) -> Result<(), Failure> {
        let mut states = self.states.lock().map_err(|_| Failure::Patch)?;
        if states.iter().any(|s| s.phase != Phase::Prepared) {
            return Err(Failure::AlreadyUsed);
        }
        for state in &mut *states {
            state.phase = Phase::Installed;
            state.bytes_pending = true;
            if transition(state, false, api).is_err() {
                return Err(Failure::Patch);
            }
        }
        Ok(())
    }

    pub fn close(&self) -> Result<(), Failure> {
        let guard = INSTALL.lock().map_err(|_| Failure::Patch)?;
        self.close_locked(&guard, &SystemApi)
    }

    /// Install failure calls this while already owning INSTALL. Never reacquire
    /// that lock recursively. Recovery and public close take it exactly once.
    pub(super) fn close_locked(
        &self,
        _guard: &MutexGuard<'_, ()>,
        api: &impl PatchApi,
    ) -> Result<(), Failure> {
        let mut states = self.states.lock().map_err(|_| Failure::Patch)?;
        // A retained, already restored handle cannot close a later binding.
        if states.iter().all(|s| s.phase == Phase::Restored) {
            return Ok(());
        }
        ACTIVE.store(false, Ordering::Release);
        let mut success = true;
        for state in states.iter_mut().rev() {
            if state.phase != Phase::Restored {
                state.phase = Phase::Restoring;
                state.bytes_pending = true;
                success &= transition(state, true, api).is_ok();
            }
        }
        // No success from bytes alone, even if another retry restored them.
        if success
            && states.iter().all(|s| {
                s.phase == Phase::Restored
                    && !s.bytes_pending
                    && !s.protection_pending
                    && !s.cache_pending
            })
        {
            Ok(())
        } else {
            Err(Failure::Patch)
        }
    }
}

fn transition(state: &mut PatchState, restore: bool, api: &impl PatchApi) -> Result<(), Failure> {
    let p = &state.patch;
    let n = size(p);
    // A fetched EB F9 can arrive after entry restoration and cache flushing.
    // Its published E9 landing pad therefore remains for the process lifetime.
    let (expected, desired) = if restore && !matches!(p.width, Width::PermanentPrefix) {
        (p.new, p.old)
    } else {
        (p.old, p.new)
    };
    let current = bytes(p).ok_or(Failure::Memory)?;
    if current == desired {
        state.bytes_pending = false;
    } else if current == expected {
        if !api.protect(p.address, n, PAGE_EXECUTE_READWRITE) {
            return Err(Failure::Patch);
        }
        state.protection_pending = true;
        api.before_cas();
        let exchanged = match p.width {
            Width::Wide | Width::PermanentPrefix => unsafe { &*(p.address as *const AtomicU64) }
                .compare_exchange(expected, desired, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok(),
            Width::Word => unsafe { &*(p.address as *const AtomicU32) }
                .compare_exchange(
                    expected as u32,
                    desired as u32,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_ok(),
            Width::Half => unsafe { &*(p.address as *const AtomicU16) }
                .compare_exchange(
                    expected as u16,
                    desired as u16,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_ok(),
        };
        if exchanged {
            state.bytes_pending = false;
            state.cache_pending = true;
        }
    }
    // Another site on this same page may have left it writable. All original
    // attributes were captured together before installation, so restore them
    // even if this particular site's bytes never changed.
    if protection(p.address, n) != Some(state.original_protection) {
        state.protection_pending = true;
    }
    // Even when the CAS fails, finish all possible cleanup with ORIGINAL attrs.
    if state.protection_pending && api.protect(p.address, n, state.original_protection) {
        state.protection_pending = false;
    }
    if state.cache_pending && api.flush(p.address, n) {
        state.cache_pending = false;
    }
    let correct =
        bytes(p) == Some(desired) && protection(p.address, n) == Some(state.original_protection);
    if correct && !state.bytes_pending && !state.protection_pending && !state.cache_pending {
        if restore {
            state.phase = Phase::Restored;
        }
        Ok(())
    } else {
        Err(Failure::Patch)
    }
}

/// Recovery retains the same mutable per-site state as the public handle.
pub fn recover_failed_registration() -> Result<(), Failure> {
    let guard = INSTALL.lock().map_err(|_| Failure::Patch)?;
    RECOVERY
        .get()?
        .ok_or(Failure::AlreadyUsed)?
        .close_locked(&guard, &SystemApi)
}

#[cfg(test)]
#[path = "patch_state_tests.rs"]
mod tests;
