//! A delayed native entry may still name this module after entry restoration.
//! Pinning is deliberately process-long; an extra reference is insufficient.
use core::ffi::c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleExW(flags: u32, address: *const u16, module: *mut *mut c_void) -> i32;
}
#[cfg(test)]
thread_local! {
    static PIN_CALLS: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
}
#[cfg(test)]
pub(super) fn take_pin_calls() -> Vec<usize> {
    PIN_CALLS.with(|calls| core::mem::take(&mut *calls.borrow_mut()))
}

pub(super) fn pin_module(address: *const c_void) -> bool {
    #[cfg(test)]
    PIN_CALLS.with(|calls| calls.borrow_mut().push(address as usize));
    const FROM_ADDRESS: u32 = 0x4;
    const PIN: u32 = 0x1;
    let mut module = core::ptr::null_mut();
    unsafe { GetModuleHandleExW(FROM_ADDRESS | PIN, address.cast(), &mut module) != 0 }
}
