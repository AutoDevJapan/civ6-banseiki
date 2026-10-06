//! Two-byte publication after an unreachable five-byte padding relay is ready.
//! Same aligned entry-CAS/persistent-code contract as native bridge property
//! hooks. No fetched gateway or original continuation is freed on failure.
use super::*;
use std::sync::atomic::AtomicU16;

pub(super) struct Hook {
    pub entry: usize,
    pub original: [u8; 2],
}

fn jump(from: usize, to: usize) -> Result<[u8; 5], String> {
    let delta = i32::try_from(to as i128 - from as i128 - 5).map_err(|_| "rel32 range")?;
    let mut b = [0xe9, 0, 0, 0, 0];
    b[1..].copy_from_slice(&delta.to_le_bytes());
    Ok(b)
}

fn write_unreachable(address: usize, expected: &[u8], bytes: &[u8]) -> Result<(), String> {
    if expected.len() != bytes.len() || read(address, expected.len())? != expected {
        return Err("padding identity changed".into());
    }
    let mut protection = 0;
    if unsafe {
        VirtualProtect(
            address as _,
            bytes.len(),
            PAGE_EXECUTE_READWRITE,
            &mut protection,
        )
    } == 0
    {
        return Err("padding protection".into());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, bytes.len());
    }
    let mut ignored = 0;
    let restored =
        unsafe { VirtualProtect(address as _, bytes.len(), protection, &mut ignored) } != 0;
    let flushed =
        unsafe { FlushInstructionCache(GetCurrentProcess(), address as _, bytes.len()) } != 0;
    if !restored || !flushed {
        return Err("padding protection/cache restore failed".into());
    }
    Ok(())
}

pub(super) fn prepare(entry: usize, original: &[u8], callback: usize) -> Result<Hook, String> {
    if entry % 2 != 0
        || original.len() < 2
        || entry < 5
        || read(entry, original.len())? != original
        || read(entry - 5, 5)? != [0xcc; 5]
    {
        return Err("entry/prologue/padding mismatch".into());
    }
    let base = entry & !0xffff;
    let mut relay = 0usize;
    'find: for step in 1..=0x4000usize {
        for candidate in [
            base.checked_add(step * 0x10000),
            base.checked_sub(step * 0x10000),
        ]
        .into_iter()
        .flatten()
        {
            if jump(entry - 5, candidate).is_err() {
                continue;
            }
            let p = unsafe {
                VirtualAlloc(
                    candidate as _,
                    4096,
                    MEM_RESERVE | MEM_COMMIT,
                    PAGE_READWRITE,
                )
            };
            if !p.is_null() {
                relay = p as usize;
                break 'find;
            }
        }
    }
    if relay == 0 {
        return Err("no near relay".into());
    }
    // RCX/RDX are the two observed arguments. R8 carries the untouched entry
    // return address. RAX is not an input to either fixed function.
    let mut code = vec![0x4c, 0x8b, 0x04, 0x24, 0x48, 0xb8];
    code.extend_from_slice(&(callback as u64).to_le_bytes());
    code.extend([0xff, 0xe0]);
    unsafe {
        std::ptr::copy_nonoverlapping(code.as_ptr(), relay as *mut u8, code.len());
    }
    let mut previous = 0;
    if unsafe { VirtualProtect(relay as _, 4096, PAGE_EXECUTE_READ, &mut previous) } == 0
        || unsafe { FlushInstructionCache(GetCurrentProcess(), relay as _, code.len()) } == 0
    {
        return Err("relay publication failed; allocation retained".into());
    }
    write_unreachable(entry - 5, &[0xcc; 5], &jump(entry - 5, relay)?)?;
    Ok(Hook {
        entry,
        original: original[..2].try_into().unwrap(),
    })
}

impl Hook {
    fn exchange(&self, expected: [u8; 2], replacement: [u8; 2]) -> Result<(), String> {
        let mut protection = 0;
        if unsafe { VirtualProtect(self.entry as _, 2, PAGE_EXECUTE_READWRITE, &mut protection) }
            == 0
        {
            return Err("entry protection".into());
        }
        let changed = unsafe { &*(self.entry as *const AtomicU16) }
            .compare_exchange(
                u16::from_le_bytes(expected),
                u16::from_le_bytes(replacement),
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok();
        let mut ignored = 0;
        let restored = unsafe { VirtualProtect(self.entry as _, 2, protection, &mut ignored) } != 0;
        let flushed =
            unsafe { FlushInstructionCache(GetCurrentProcess(), self.entry as _, 2) } != 0;
        if !changed || !restored || !flushed {
            return Err("critical entry publication/restore result".into());
        }
        Ok(())
    }
    pub(super) fn publish(&self) -> Result<(), String> {
        self.exchange(self.original, [0xeb, 0xf9])
    }
    #[cfg(test)]
    pub(super) fn restore(&self) -> Result<(), String> {
        self.exchange([0xeb, 0xf9], self.original)
    }
}
