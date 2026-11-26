//! Full synchronous bridge calls. Engine-owned work remains with native Release.
use super::*;
use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
use windows_sys::Win32::System::Diagnostics::Debug::{CONTEXT, EXCEPTION_RECORD};

struct Completion(*const AtomicU64);
impl Drop for Completion {
    fn drop(&mut self) {
        unsafe { runtime::invocation_finish(self.0) };
    }
}
unsafe fn cookie_before(scratch: *const u8) -> *const AtomicU64 {
    unsafe { scratch.sub(0x10).cast() }
}
unsafe fn start(cookie: *const AtomicU64) -> bool {
    let held = runtime::invocation_start();
    unsafe { (*cookie).store(held, Ordering::Release) };
    runtime::invocation_admitted(held)
}
unsafe fn admitted(cookie: *const AtomicU64) -> bool {
    runtime::invocation_admitted(unsafe { (*cookie).load(Ordering::Acquire) })
}

unsafe extern "system" fn calibration_begin(
    governors: u64,
    index: u32,
    caller: u64,
    scratch: *mut CalibrationRow,
) -> *const CalibrationRow {
    let error = unsafe { GetLastError() };
    let cookie = unsafe { cookie_before(scratch.cast()) };
    let result = if unsafe { start(cookie) } {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            #[cfg(test)]
            tests::panic_in_begin();
            super::calibration_begin(governors, index, caller, scratch)
        }))
        .unwrap_or_else(|_| {
            unsafe { runtime::invocation_refuse(cookie) };
            null_mut()
        })
    } else {
        null_mut()
    };
    unsafe { SetLastError(error) };
    result
}
unsafe extern "system" fn calibration_end(
    row: *const CalibrationRow,
    original: u64,
    index: u32,
    governors: u64,
    cookie: *const AtomicU64,
) -> u64 {
    let error = unsafe { GetLastError() };
    let complete = Completion(cookie);
    let result = if unsafe { admitted(cookie) } {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            super::calibration_end(row, original, index, governors)
        }))
        .unwrap_or(original & !0xff)
    } else {
        original & !0xff
    };
    drop(complete);
    unsafe { SetLastError(error) };
    result
}
unsafe extern "system" fn property_begin(property: u64, key: u32, cookie: *const AtomicU64) -> u64 {
    let error = unsafe { GetLastError() };
    let result = if unsafe { start(cookie) } {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            #[cfg(test)]
            tests::panic_in_begin();
            super::joined_property_begin(property, key)
        }))
        .unwrap_or_else(|_| {
            unsafe { runtime::invocation_refuse(cookie) };
            0
        })
    } else {
        0
    };
    unsafe { SetLastError(error) };
    result
}
unsafe extern "system" fn property_end(
    property: u64,
    key: u32,
    returned: u64,
    token: u64,
    cookie: *const AtomicU64,
) {
    let error = unsafe { GetLastError() };
    let complete = Completion(cookie);
    #[cfg(test)]
    tests::pause(&tests::PAUSE_END);
    if unsafe { admitted(cookie) } {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            super::record_property(property, key, returned, token)
        }));
    }
    drop(complete);
    unsafe { SetLastError(error) };
}
unsafe extern "system" fn assignment_begin(
    governors: u64,
    governor: u64,
    city: u64,
    r9: u64,
    caller: u64,
    fence: *mut runtime::CallFence,
) -> u64 {
    let error = unsafe { GetLastError() };
    let cookie = unsafe { cookie_before(fence.cast()) };
    let result = if unsafe { start(cookie) } {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            assignment::assign_calibration_begin_scoped(
                governors,
                governor,
                city,
                r9,
                caller,
                fence,
                cookie.add(1),
            )
        }))
        .unwrap_or_else(|_| {
            unsafe { runtime::invocation_refuse(cookie) };
            0
        })
    } else {
        0
    };
    // The scoped begin publishes its sample reservation before any pointer read.
    unsafe { SetLastError(error) };
    result
}
unsafe extern "system" fn assignment_end(
    token: u64,
    original: u64,
    governors: u64,
    governor: u64,
    fence: *const runtime::CallFence,
) -> u64 {
    let error = unsafe { GetLastError() };
    let cookie = unsafe { cookie_before(fence.cast()) };
    let complete = Completion(cookie);
    let result = if unsafe { admitted(cookie) } {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            assignment::assign_calibration_end(token, original, governors, governor, fence)
        }))
        .unwrap_or(original & !0xff)
    } else {
        original & !0xff
    };
    let sample = unsafe { (*cookie.add(1)).swap(0, Ordering::AcqRel) };
    unsafe { assignment::unwind_sample(sample) };
    drop(complete);
    unsafe { SetLastError(error) };
    result
}
unsafe extern "system" fn unwind(
    record: *const EXCEPTION_RECORD,
    frame: usize,
    _: *mut CONTEXT,
    _: *mut c_void,
) -> i32 {
    let error = unsafe { GetLastError() };
    unsafe { runtime::invocation_unwind(record, (frame + 0xd0) as *const AtomicU64) };
    unsafe { SetLastError(error) };
    1
}
unsafe extern "system" fn assignment_unwind(
    record: *const EXCEPTION_RECORD,
    frame: usize,
    context: *mut CONTEXT,
    dispatcher: *mut c_void,
) -> i32 {
    let error = unsafe { GetLastError() };
    if !record.is_null() && unsafe { (*record).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0 {
        let token = unsafe { &*((frame + 0xd8) as *const AtomicU64) }.swap(0, Ordering::AcqRel);
        unsafe { assignment::unwind_sample(token) };
    }
    let result = unsafe { unwind(record, frame, context, dispatcher) };
    unsafe { SetLastError(error) };
    result
}

// Uniform fixed frame: cookie D0, sampled assignment token D8, scratch E0,
// original arguments 138/140, post-call RCX/RDX 148/150. FLAGS helpers are
// the existing fully described root helpers at this exact 1C0+8 stack shape.
macro_rules! gateway {
    ($name:literal, $pre:literal, $post:literal, $begin:expr, $end:expr, $original:expr, $unwind:expr) => {
        global_asm!(concat!(
            ".globl ", $name, "\n.seh_proc ", $name, "\n", $name, ":\n",
            "pushfq\n.seh_stackalloc 8\nsub rsp,0x1c0\n.seh_stackalloc 0x1c0\n",
            "mov qword ptr [rsp+0xd0],0\nmov qword ptr [rsp+0xd8],0\n",
            ".seh_handler {unwind}, @unwind\n.seh_endprologue\n",
            "mov [rsp+0x20],rcx\nmov [rsp+0x28],rdx\nmov [rsp+0x30],r8\nmov [rsp+0x38],r9\n",
            "mov [rsp+0x40],rax\nmov [rsp+0x48],r10\nmov [rsp+0x50],r11\n",
            "movdqu [rsp+0x70],xmm0\nmovdqu [rsp+0x80],xmm1\nmovdqu [rsp+0x90],xmm2\n",
            "movdqu [rsp+0xa0],xmm3\nmovdqu [rsp+0xb0],xmm4\nmovdqu [rsp+0xc0],xmm5\n",
            "mov [rsp+0x138],rcx\nmov [rsp+0x140],rdx\n", $pre, "\ncall {begin}\nmov [rsp+0x58],rax\n",
            "mov rcx,[rsp+0x138]\nmov rdx,[rsp+0x140]\nmov r8,[rsp+0x30]\nmov r9,[rsp+0x38]\n",
            "mov rax,[rsp+0x40]\nmov r10,[rsp+0x48]\nmov r11,[rsp+0x50]\n",
            "movdqu xmm0,[rsp+0x70]\nmovdqu xmm1,[rsp+0x80]\nmovdqu xmm2,[rsp+0x90]\n",
            "movdqu xmm3,[rsp+0xa0]\nmovdqu xmm4,[rsp+0xb0]\nmovdqu xmm5,[rsp+0xc0]\n",
            "call root_flags_restore\ncall qword ptr [rip+{original}]\n",
            "mov [rsp+0x148],rcx\nmov [rsp+0x150],rdx\nmov [rsp+0x30],r8\nmov [rsp+0x38],r9\n",
            "mov [rsp+0x40],rax\nmov [rsp+0x48],r10\nmov [rsp+0x50],r11\n",
            "movdqu [rsp+0x70],xmm0\nmovdqu [rsp+0x80],xmm1\nmovdqu [rsp+0x90],xmm2\n",
            "movdqu [rsp+0xa0],xmm3\nmovdqu [rsp+0xb0],xmm4\nmovdqu [rsp+0xc0],xmm5\n",
            "call root_flags_save\n", $post, "\n",
            "movdqu xmm0,[rsp+0x70]\nmovdqu xmm1,[rsp+0x80]\nmovdqu xmm2,[rsp+0x90]\n",
            "movdqu xmm3,[rsp+0xa0]\nmovdqu xmm4,[rsp+0xb0]\nmovdqu xmm5,[rsp+0xc0]\n",
            "mov rcx,[rsp+0x148]\nmov rdx,[rsp+0x150]\nmov r8,[rsp+0x30]\nmov r9,[rsp+0x38]\n",
            "mov rax,[rsp+0x40]\nmov r10,[rsp+0x48]\nmov r11,[rsp+0x50]\n",
            "call root_flags_restore\nlea rsp,[rsp+0x1c8]\nret\n.seh_endproc\n"
        ), begin=sym $begin, end=sym $end, original=sym $original, unwind=sym $unwind);
    };
}
gateway!("calibration_gateway",
    "mov r8,[rsp+0x1c8]\nlea r9,[rsp+0xe0]",
    "mov rcx,[rsp+0x58]\nmov rdx,[rsp+0x40]\nmov r8d,[rsp+0x140]\nmov r9,[rsp+0x138]\nlea rax,[rsp+0xd0]\nmov [rsp+0x20],rax\ncall {end}\nmov [rsp+0x40],rax",
    calibration_begin, calibration_end, BRIDGE_TRAMPOLINE_PTR, unwind);
gateway!("property_gateway",
    "lea r8,[rsp+0xd0]",
    "mov rcx,[rsp+0x138]\nmov edx,[rsp+0x140]\nmov r8,[rsp+0x40]\nmov r9,[rsp+0x58]\nlea rax,[rsp+0xd0]\nmov [rsp+0x20],rax\ncall {end}",
    property_begin, property_end, PROPERTY_TRAMPOLINE_PTR, unwind);
gateway!("assign_calibration_gateway",
    "mov rax,[rsp+0x1c8]\nmov [rsp+0x20],rax\nlea rax,[rsp+0xe0]\nmov [rsp+0x28],rax",
    "mov rcx,[rsp+0x58]\nmov rdx,[rsp+0x40]\nmov r8,[rsp+0x138]\nmov r9,[rsp+0x140]\nlea rax,[rsp+0xe0]\nmov [rsp+0x20],rax\ncall {end}\nmov [rsp+0x40],rax",
    assignment_begin, assignment_end, assignment::ASSIGN_TRAMPOLINE_PTR, assignment_unwind);

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
pub(super) mod tests;
