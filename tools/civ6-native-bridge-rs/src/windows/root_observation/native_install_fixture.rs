//! Process-owned entry data for installer ordering tests; never executed.
use std::ptr;
use windows_sys::Win32::System::Memory::{
    VirtualAlloc, VirtualFree, VirtualProtect, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};

pub(in super::super) struct Memory {
    pub(in super::super) base: usize,
    len: usize,
}
impl Memory {
    pub(in super::super) fn new(entries: &[(usize, [u8; 8])]) -> Self {
        let len = entries
            .iter()
            .map(|(rva, _)| rva.checked_add(8).unwrap())
            .max()
            .unwrap();
        let base =
            unsafe { VirtualAlloc(ptr::null(), len, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
                as usize;
        assert_ne!(base, 0);
        let memory = Self { base, len };
        for (rva, bytes) in entries {
            memory.write(*rva, bytes);
        }
        memory
    }
    pub(in super::super) fn write(&self, rva: usize, bytes: &[u8]) {
        assert!(rva
            .checked_add(bytes.len())
            .is_some_and(|end| end <= self.len));
        unsafe { write_at(self.base + rva, bytes) };
    }
    pub(in super::super) fn bytes(&self, rva: usize) -> [u8; 8] {
        assert!(rva.checked_add(8).is_some_and(|end| end <= self.len));
        unsafe { ptr::read_unaligned((self.base + rva) as *const [u8; 8]) }
    }
    pub(in super::super) fn protect_all(&self, protection: u32) {
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(self.base as _, self.len, protection, &mut old) },
            0
        );
    }
    pub(in super::super) fn protect(&self, rva: usize, protection: u32) {
        assert!(rva < self.len);
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect((self.base + rva) as _, 1, protection, &mut old) },
            0
        );
    }
}
/// # Safety
/// The full range must belong to the test's still-owned allocation.
pub(in super::super) unsafe fn write_at(address: usize, bytes: &[u8]) {
    let mut old = 0;
    assert_ne!(
        unsafe { VirtualProtect(address as _, bytes.len(), PAGE_READWRITE, &mut old) },
        0
    );
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, bytes.len()) };
    let mut replaced = 0;
    assert_ne!(
        unsafe { VirtualProtect(address as _, bytes.len(), old, &mut replaced) },
        0
    );
}
impl Drop for Memory {
    fn drop(&mut self) {
        assert_ne!(unsafe { VirtualFree(self.base as _, 0, MEM_RELEASE) }, 0);
    }
}
