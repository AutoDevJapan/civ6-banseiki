//! Process-owned entry data for installer ordering tests; never executed.
use std::ptr;
use windows_sys::Win32::System::Memory::{
    VirtualAlloc, VirtualFree, VirtualProtect, VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT,
    MEM_DECOMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_GUARD, PAGE_NOACCESS,
    PAGE_READWRITE,
};

pub(in super::super) struct Memory {
    pub(in super::super) base: usize,
    allocation: usize,
    len: usize,
}
impl Memory {
    pub(in super::super) fn new(entries: &[(usize, [u8; 8])]) -> Self {
        Self::with_bias(entries, 0)
    }
    pub(in super::super) fn with_bias(entries: &[(usize, [u8; 8])], bias: usize) -> Self {
        assert!(bias < 8);
        let len = entries
            .iter()
            .map(|(rva, _)| rva.checked_add(8).unwrap())
            .max()
            .unwrap();
        let allocation = unsafe {
            VirtualAlloc(
                ptr::null(),
                len.checked_add(bias).unwrap(),
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        } as usize;
        assert_ne!(allocation, 0);
        let memory = Self {
            base: allocation + bias,
            allocation,
            len,
        };
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
    pub(in super::super) fn decommit(&self, rva: usize) {
        assert!(rva < self.len);
        assert_ne!(
            unsafe { VirtualFree((self.base + rva) as _, 1, MEM_DECOMMIT) },
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
        assert_ne!(
            unsafe { VirtualFree(self.allocation as _, 0, MEM_RELEASE) },
            0
        );
    }
}

pub(in super::super) fn rejects_invalid_entry_data(
    validate: fn(usize, usize, [u8; 8]) -> Result<u32, &'static str>,
) {
    const RVA: usize = 0x1000;
    const BYTES: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    let valid = Memory::new(&[(RVA, BYTES)]);
    valid.protect_all(PAGE_EXECUTE_READ);
    assert_eq!(validate(valid.base, RVA, BYTES), Ok(PAGE_EXECUTE_READ));
    let unaligned = Memory::with_bias(&[(RVA, BYTES)], 1);
    unaligned.protect_all(PAGE_EXECUTE_READ);
    assert!(
        validate(unaligned.base, RVA, BYTES).is_err(),
        "unaligned entry was accepted"
    );
    for protection in [
        PAGE_READWRITE,
        PAGE_NOACCESS,
        PAGE_EXECUTE_READ | PAGE_GUARD,
    ] {
        let memory = Memory::new(&[(RVA, BYTES)]);
        memory.protect_all(PAGE_EXECUTE_READ);
        memory.protect(RVA, protection);
        assert!(
            validate(memory.base, RVA, BYTES).is_err(),
            "invalid protection was accepted: {protection}"
        );
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        assert_ne!(
            unsafe {
                VirtualQuery(
                    (memory.base + RVA) as _,
                    &mut info,
                    std::mem::size_of_val(&info),
                )
            },
            0
        );
        assert_eq!(
            info.Protect, protection,
            "entry validation consumed the page protection"
        );
    }
    let reserved = Memory::new(&[(RVA, BYTES)]);
    reserved.protect_all(PAGE_EXECUTE_READ);
    reserved.decommit(RVA);
    assert!(
        validate(reserved.base, RVA, BYTES).is_err(),
        "uncommitted entry was accepted"
    );
}
