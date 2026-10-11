//! Minimal Win64 helpers for the observer: guarded memory copy, committed-range
//! check, and a rel32-reachable executable page. Everything is crate-private.
pub(crate) use core::arch::global_asm;
pub(crate) use std::ffi::c_void;
pub(crate) use std::ptr::{self, null_mut};
pub(crate) use std::sync::atomic::{AtomicPtr, AtomicU16, AtomicU32, AtomicU64, Ordering};
pub(crate) use std::sync::Mutex;
pub(crate) use windows_sys::Win32::System::Diagnostics::Debug::{
    FlushInstructionCache, ReadProcessMemory,
};
pub(crate) use windows_sys::Win32::System::Memory::{
    VirtualAlloc, VirtualFree, VirtualProtect, VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT,
    MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY,
    PAGE_GUARD, PAGE_NOACCESS, PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY,
};
pub(crate) use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetCurrentThreadId};

pub(crate) const CODE_PAGE: usize = 4096;
pub(crate) const RELAY_OFFSET: usize = 0;
pub(crate) const TRAMPOLINE_OFFSET: usize = 32;
/// jmp -7: lands on the five byte jump placed in the padding before the entry.
pub(crate) const PROPERTY_SHORT_JUMP: [u8; 2] = [0xeb, 0xf9];

/// Copies N bytes from this process through the OS, so an unreadable address
/// is a `None` and never a fault.
pub(crate) fn copy_current<const N: usize>(address: u64) -> Option<[u8; N]> {
    if address == 0 {
        return None;
    }
    let mut bytes = [0u8; N];
    let mut read = 0usize;
    let ok = unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            address as *const c_void,
            bytes.as_mut_ptr().cast(),
            N,
            &mut read,
        )
    } != 0;
    (ok && read == N).then_some(bytes)
}

pub(crate) fn committed_range(
    address: usize,
    bytes: usize,
    alignment: usize,
    writable: bool,
) -> bool {
    if address == 0 || bytes == 0 || !address.is_multiple_of(alignment) {
        return false;
    }
    let Some(end) = address.checked_add(bytes) else {
        return false;
    };
    let mut cursor = address;
    while cursor < end {
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe {
            VirtualQuery(
                cursor as *const c_void,
                &mut info,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        } != std::mem::size_of::<MEMORY_BASIC_INFORMATION>()
            || info.State != MEM_COMMIT
            || info.Protect & (PAGE_GUARD | PAGE_NOACCESS) != 0
        {
            return false;
        }
        let access = info.Protect & 0xff;
        let permitted = if writable {
            matches!(
                access,
                PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY
            )
        } else {
            matches!(
                access,
                PAGE_READONLY
                    | PAGE_READWRITE
                    | PAGE_WRITECOPY
                    | PAGE_EXECUTE_READ
                    | PAGE_EXECUTE_READWRITE
                    | PAGE_EXECUTE_WRITECOPY
            )
        };
        let base = info.BaseAddress as usize;
        let Some(region_end) = base.checked_add(info.RegionSize) else {
            return false;
        };
        if !permitted || cursor < base || region_end <= cursor {
            return false;
        }
        cursor = region_end.min(end);
    }
    true
}

pub(crate) fn rel32_jump(from: usize, to: usize) -> Option<[u8; 5]> {
    let displacement = (to as i128) - (from as i128) - 5;
    let displacement = i32::try_from(displacement).ok()?;
    let mut jump = [0u8; 5];
    jump[0] = 0xe9;
    jump[1..].copy_from_slice(&displacement.to_le_bytes());
    Some(jump)
}

/// Commits one RWX page within rel32 range of `jump_from`. The hint is
/// allocation-granularity aligned; an occupied hint only advances the scan.
pub(crate) unsafe fn executable_near(jump_from: usize) -> Option<*mut u8> {
    let base = jump_from & !0xffffusize;
    for step in 1..=0x4000usize {
        let delta = step * 0x10000;
        for candidate in [base.checked_add(delta), base.checked_sub(delta)] {
            let Some(candidate) = candidate else { continue };
            if rel32_jump(jump_from, candidate).is_none() {
                continue;
            }
            let region = unsafe {
                VirtualAlloc(
                    candidate as *const c_void,
                    CODE_PAGE,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_EXECUTE_READWRITE,
                )
            };
            if !region.is_null() {
                return Some(region.cast());
            }
        }
    }
    None
}
