//! A hash-pinned licensed image copied into an owned child's memory. This
//! never resolves imports, invokes DllMain, or executes a native game body.
use super::*;
use std::io::{Read, Seek, SeekFrom};
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlAddFunctionTable, RtlDeleteFunctionTable, IMAGE_RUNTIME_FUNCTION_ENTRY,
};
pub(super) struct Image {
    pub(super) base: usize,
    len: usize,
    pdata: *mut IMAGE_RUNTIME_FUNCTION_ENTRY,
}
impl Image {
    pub(super) fn new(path: &str, sha: &str) -> Self {
        let mut file = super::super::pin(path, sha).unwrap();
        let mut bytes = Vec::new();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.read_to_end(&mut bytes).unwrap();
        let u16_at = |p: usize| u16::from_le_bytes(bytes[p..p + 2].try_into().unwrap());
        let u32_at = |p: usize| u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap());
        let nt = u32_at(0x3c) as usize;
        let opt = nt + 24;
        let preferred = u64::from_le_bytes(bytes[opt + 24..opt + 32].try_into().unwrap()) as usize;
        let len = u32_at(opt + 56) as usize;
        let base = unsafe {
            VirtualAlloc(
                preferred as _,
                len,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        } as usize;
        assert_eq!(base, preferred, "owned primary preferred range unavailable");
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), base as *mut u8, u32_at(opt + 60) as usize);
        }
        let mut text = (0, 0);
        for i in 0..usize::from(u16_at(nt + 6)) {
            let row = opt + usize::from(u16_at(nt + 20)) + i * 40;
            let start = u32_at(row + 12) as usize;
            let raw = u32_at(row + 20) as usize;
            let count = u32_at(row + 16) as usize;
            assert!(start + count <= len && raw + count <= bytes.len());
            unsafe {
                ptr::copy_nonoverlapping(bytes[raw..].as_ptr(), (base + start) as *mut u8, count);
            }
            if &bytes[row..row + 5] == b".text" {
                text = (start, count);
            }
        }
        assert!(text.1 > 0);
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect((base + text.0) as _, text.1, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        let pdata =
            (base + u32_at(opt + 112 + 3 * 8) as usize) as *mut IMAGE_RUNTIME_FUNCTION_ENTRY;
        assert!(unsafe { RtlAddFunctionTable(pdata, u32_at(opt + 116 + 3 * 8) / 12, base as u64) });
        Self { base, len, pdata }
    }
    /// Mutate only this owned copy, preserving its prior page protection.
    pub(super) fn replace_byte(&self, rva: usize, value: u8) -> u8 {
        assert!(rva < self.len);
        let address = (self.base + rva) as *mut u8;
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(address.cast(), 1, PAGE_EXECUTE_READWRITE, &mut old) },
            0
        );
        let previous = unsafe { ptr::read_volatile(address) };
        unsafe {
            ptr::write_volatile(address, value);
        }
        let mut ignored = 0;
        assert_ne!(
            unsafe { VirtualProtect(address.cast(), 1, old, &mut ignored) },
            0
        );
        assert_ne!(
            unsafe { FlushInstructionCache(GetCurrentProcess(), address.cast(), 1) },
            0
        );
        previous
    }
}
impl Drop for Image {
    fn drop(&mut self) {
        // Callers restore all entry owners before releasing this allocation.
        unsafe {
            let _ = RtlDeleteFunctionTable(self.pdata);
            let _ = VirtualFree(self.base as _, 0, MEM_RELEASE);
        }
    }
}
