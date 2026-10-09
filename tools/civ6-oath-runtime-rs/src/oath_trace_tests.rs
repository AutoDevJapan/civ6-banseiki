//! Win64 synthetic regression tests. Each scenario runs in an owned child
//! process because the observer is a process-lifetime singleton.
use super::*;
use crate::oath_install_state::{Binding, Bytes, Reach};
use std::os::windows::process::CommandExt;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Diagnostics::Debug::{RtlLookupFunctionEntry, CONTEXT};

// windows-sys gates this export behind a feature that returns an unused type;
// the plain kernel32 prototype is all the test needs.
unsafe extern "system" {
    fn RtlVirtualUnwind(
        handler_type: u32,
        image_base: u64,
        control_pc: u64,
        function_entry: *const c_void,
        context: *mut CONTEXT,
        handler_data: *mut *mut c_void,
        establisher_frame: *mut u64,
        context_pointers: *mut c_void,
    ) -> *mut c_void;
}

#[unsafe(no_mangle)]
static OATH_TEST_ENTRY: AtomicPtr<u8> = AtomicPtr::new(null_mut());
static OATH_TEST_RAISE: AtomicBool = AtomicBool::new(false);
static OATH_TEST_CALLS: AtomicU32 = AtomicU32::new(0);

/// Original body used by the unwind scenario: counts, then raises on demand.
unsafe extern "system-unwind" fn oath_test_hook(_effect: u64) -> u64 {
    OATH_TEST_CALLS.fetch_add(1, Ordering::SeqCst);
    if OATH_TEST_RAISE.load(Ordering::SeqCst) {
        panic!("injected failure inside the original oath wrapper");
    }
    0x77
}

// Synthetic original, reached after the real wrapper's "push rbx; sub rsp,30h;
// movups xmm0,[rip+disp]". The effect block (RCX) receives every input.
global_asm!(
    ".globl oath_original_body", "oath_original_body:",
    "inc qword ptr [rcx+8]", "mov [rcx+0x10], rcx", "mov [rcx+0x18], rdx",
    "mov [rcx+0x20], r8", "mov [rcx+0x28], r9", "mov [rcx+0x30], rax",
    "mov [rcx+0x38], r10", "mov [rcx+0x40], r11",
    "movups [rcx+0x50], xmm0", "movups [rcx+0x60], xmm1", "movups [rcx+0x70], xmm2",
    "movups [rcx+0x80], xmm3", "movups [rcx+0x90], xmm4", "movups [rcx+0xa0], xmm5",
    "mov [rcx+0xb0], rbx", "mov [rcx+0xb8], rbp", "mov [rcx+0xc0], rdi", "mov [rcx+0xc8], rsi",
    "mov [rcx+0xd0], r12", "mov [rcx+0xd8], r13", "mov [rcx+0xe0], r14", "mov [rcx+0xe8], r15",
    "mov rax, rsp", "and eax, 15", "mov [rcx+0xf0], rax",
    "mov rax, [rcx+0xf8]", "add rsp, 0x30", "pop rbx", "ret",
    ".globl oath_original_clobber", "oath_original_clobber:",
    "mov r12, 1", "pxor xmm7, xmm7", "mov rax, [rcx+0xf8]", "add rsp, 0x30", "pop rbx", "ret",
    ".globl oath_original_other", "oath_original_other:",
    "inc qword ptr [rcx+0x108]", "mov rax, [rcx+0xf8]", "add rsp, 0x30", "pop rbx", "ret",
    ".globl oath_original_raise", "oath_original_raise:",
    "add rsp, 0x30", "pop rbx", "jmp {hook}",
    hook = sym oath_test_hook,
);
unsafe extern "system" {
    fn oath_original_body();
    fn oath_original_clobber();
    fn oath_original_other();
    fn oath_original_raise();
}

// Caller with sentinels in every nonvolatile register and XMM6-15. After the
// call it stores a mismatch mask at effect+0x100 (GPR bit n, XMM bit 8+n).
global_asm!(
    ".macro OATH_CHK_GPR reg, imm, bit", "mov r9, \\imm", "cmp \\reg, r9", "setne r8b",
    "movzx r8d, r8b", "shl r8d, \\bit", "or edx, r8d", ".endm",
    ".macro OATH_CHK_XMM reg, imm, bit", "movdqu [rsp], \\reg", "mov r9, \\imm", "mov r8, [rsp]",
    "xor r8, r9", "mov r11, [rsp+8]", "xor r11, r9", "or r8, r11", "setne r8b",
    "movzx r8d, r8b", "shl r8d, \\bit", "or edx, r8d", ".endm",
    ".macro OATH_SET_XMM reg, imm", "mov rax, \\imm", "movq \\reg, rax", "punpcklqdq \\reg, \\reg", ".endm",
    ".globl oath_abi_fixture", ".seh_proc oath_abi_fixture", "oath_abi_fixture:",
    "push rbx", ".seh_pushreg rbx", "push rbp", ".seh_pushreg rbp", "push rdi", ".seh_pushreg rdi",
    "push rsi", ".seh_pushreg rsi", "push r12", ".seh_pushreg r12", "push r13", ".seh_pushreg r13",
    "push r14", ".seh_pushreg r14", "push r15", ".seh_pushreg r15",
    "sub rsp, 0xd8", ".seh_stackalloc 0xd8", ".seh_endprologue",
    "mov [rsp+0x28], rcx",
    "movdqu [rsp+0x30], xmm6", "movdqu [rsp+0x40], xmm7", "movdqu [rsp+0x50], xmm8",
    "movdqu [rsp+0x60], xmm9", "movdqu [rsp+0x70], xmm10", "movdqu [rsp+0x80], xmm11",
    "movdqu [rsp+0x90], xmm12", "movdqu [rsp+0xa0], xmm13", "movdqu [rsp+0xb0], xmm14",
    "movdqu [rsp+0xc0], xmm15",
    "mov rbx, 0x1111111111111111", "mov rbp, 0x2222222222222222", "mov rdi, 0x3333333333333333",
    "mov rsi, 0x4444444444444444", "mov r12, 0x5555555555555555", "mov r13, 0x6666666666666666",
    "mov r14, 0x7777777777777777", "mov r15, 0x8888888888888888",
    "OATH_SET_XMM xmm6, 0x6060606060606060", "OATH_SET_XMM xmm7, 0x6161616161616161",
    "OATH_SET_XMM xmm8, 0x6262626262626262", "OATH_SET_XMM xmm9, 0x6363636363636363",
    "OATH_SET_XMM xmm10, 0x6464646464646464", "OATH_SET_XMM xmm11, 0x6565656565656565",
    "OATH_SET_XMM xmm12, 0x6666666666666666", "OATH_SET_XMM xmm13, 0x6767676767676767",
    "OATH_SET_XMM xmm14, 0x6868686868686868", "OATH_SET_XMM xmm15, 0x6969696969696969",
    "mov rax, 0x11223344", "mov r10, 0x22334455", "mov r11, 0x33445566",
    "pcmpeqd xmm0, xmm0", "pcmpeqd xmm1, xmm1", "pcmpeqd xmm2, xmm2", "pcmpeqd xmm3, xmm3",
    "pcmpeqd xmm4, xmm4", "pcmpeqd xmm5, xmm5",
    "call qword ptr [rip+{entry}]",
    "mov r10, [rsp+0x28]", "xor edx, edx",
    "OATH_CHK_GPR rbx, 0x1111111111111111, 0", "OATH_CHK_GPR rbp, 0x2222222222222222, 1",
    "OATH_CHK_GPR rdi, 0x3333333333333333, 2", "OATH_CHK_GPR rsi, 0x4444444444444444, 3",
    "OATH_CHK_GPR r12, 0x5555555555555555, 4", "OATH_CHK_GPR r13, 0x6666666666666666, 5",
    "OATH_CHK_GPR r14, 0x7777777777777777, 6", "OATH_CHK_GPR r15, 0x8888888888888888, 7",
    "OATH_CHK_XMM xmm6, 0x6060606060606060, 8", "OATH_CHK_XMM xmm7, 0x6161616161616161, 9",
    "OATH_CHK_XMM xmm8, 0x6262626262626262, 10", "OATH_CHK_XMM xmm9, 0x6363636363636363, 11",
    "OATH_CHK_XMM xmm10, 0x6464646464646464, 12", "OATH_CHK_XMM xmm11, 0x6565656565656565, 13",
    "OATH_CHK_XMM xmm12, 0x6666666666666666, 14", "OATH_CHK_XMM xmm13, 0x6767676767676767, 15",
    "OATH_CHK_XMM xmm14, 0x6868686868686868, 16", "OATH_CHK_XMM xmm15, 0x6969696969696969, 17",
    "mov [r10+0x100], rdx",
    "movdqu xmm6, [rsp+0x30]", "movdqu xmm7, [rsp+0x40]", "movdqu xmm8, [rsp+0x50]",
    "movdqu xmm9, [rsp+0x60]", "movdqu xmm10, [rsp+0x70]", "movdqu xmm11, [rsp+0x80]",
    "movdqu xmm12, [rsp+0x90]", "movdqu xmm13, [rsp+0xa0]", "movdqu xmm14, [rsp+0xb0]",
    "movdqu xmm15, [rsp+0xc0]",
    "add rsp, 0xd8", "pop r15", "pop r14", "pop r13", "pop r12", "pop rsi", "pop rdi",
    "pop rbp", "pop rbx", "ret", ".seh_endproc",
    entry = sym OATH_TEST_ENTRY,
);
// Plain caller with unwind info; an exception in the original passes through
// the gateway frame and then this one.
global_asm!(
    ".globl oath_unwind_fixture", ".seh_proc oath_unwind_fixture", "oath_unwind_fixture:",
    "sub rsp, 0x28", ".seh_stackalloc 0x28", ".seh_endprologue",
    "call qword ptr [rip+{entry}]", "add rsp, 0x28", "ret", ".seh_endproc",
    entry = sym OATH_TEST_ENTRY,
);
unsafe extern "system" {
    fn oath_abi_fixture(effect: u64, subject: u64, r8: u64, r9: u64) -> u64;
}
unsafe extern "system-unwind" {
    fn oath_unwind_fixture(effect: u64, subject: u64, r8: u64, r9: u64) -> u64;
}

#[repr(C, align(16))]
struct Block([u64; 64]);
impl Block {
    fn new(vtable: u64, ret: u64) -> Self {
        let mut block = Block([0; 64]);
        block.0[0] = vtable;
        block.0[31] = ret;
        block
    }
    fn address(&mut self) -> u64 {
        self.0.as_mut_ptr() as u64
    }
}

const ENTRY_OFFSETS: [usize; 2] = [0x20, 0x80];
const XMM0_PATTERN: [u64; 2] = [0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210];

/// One page holding two wrappers with the pinned eight-byte head, a real
/// RIP-relative movups, and a far jump into the chosen synthetic body.
unsafe fn build_page(body: unsafe extern "system" fn()) -> *mut u8 {
    let page = unsafe {
        VirtualAlloc(
            null_mut(),
            CODE_PAGE,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        )
    }
    .cast::<u8>();
    assert!(!page.is_null());
    unsafe {
        ptr::write_bytes(page, 0xcc, CODE_PAGE);
        for offset in ENTRY_OFFSETS {
            let entry = page.add(offset);
            ptr::copy_nonoverlapping(
                [0x40, 0x53, 0x48, 0x83, 0xec, 0x30, 0x0f, 0x10, 0x05].as_ptr(),
                entry,
                9,
            );
            let disp = (0x200 - (offset as i32 + 13)).to_le_bytes();
            ptr::copy_nonoverlapping(disp.as_ptr(), entry.add(9), 4);
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), entry.add(13), 6);
            ptr::copy_nonoverlapping(
                (body as usize as u64).to_le_bytes().as_ptr(),
                entry.add(19),
                8,
            );
        }
        ptr::copy_nonoverlapping(XMM0_PATTERN.as_ptr().cast::<u8>(), page.add(0x200), 16);
    }
    page
}

fn entry_head(entry: *mut u8) -> [u8; 8] {
    unsafe { ptr::read(entry.cast()) }
}

/// The page the game's code really has: execute/read, not writable.
fn make_rx(page: *mut u8) {
    let mut old = 0u32;
    assert_ne!(
        unsafe { VirtualProtect(page.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) },
        0
    );
}

/// Runs one RtlVirtualUnwind step and returns (Rip, Rsp, Rbx), or None when
/// no function entry describes `rip`.
fn unwind_once(rip: u64, rsp: u64, rbx: u64) -> Option<(u64, u64, u64)> {
    let mut image_base = 0u64;
    let entry = unsafe { RtlLookupFunctionEntry(rip, &mut image_base, null_mut()) };
    if entry.is_null() {
        return None;
    }
    let mut context = unsafe { std::mem::zeroed::<CONTEXT>() };
    context.Rip = rip;
    context.Rsp = rsp;
    context.Rbx = rbx;
    let mut handler_data = null_mut();
    let mut establisher = 0u64;
    unsafe {
        RtlVirtualUnwind(
            0,
            image_base,
            rip,
            entry.cast(),
            &mut context,
            &mut handler_data,
            &mut establisher,
            null_mut(),
        )
    };
    Some((context.Rip, context.Rsp, context.Rbx))
}

/// A stack walk that stops with RIP inside the relay or the trampoline (before
/// or after its pushed RBX) must still find the real caller, RSP and RBX.
fn check_unwind_tables(code: *mut u8, prefix_slot: *mut u8) {
    let relay = code as u64;
    let trampoline = code as u64 + TRAMPOLINE_OFFSET as u64;
    let caller = 0x1234_5678_9abc_def0u64;
    let saved_rbx = 0x0bad_f00d_0bad_f00du64;
    let mut stack = [saved_rbx, caller, 0, 0];
    let top = stack.as_mut_ptr() as u64;
    // Before the copied push: [rsp] is the return address, RBX is live.
    assert_eq!(
        unwind_once(trampoline, top + 8, 0x5151),
        Some((caller, top + 16, 0x5151))
    );
    // After it: the jump back runs with RBX saved at [rsp].
    assert_eq!(
        unwind_once(trampoline + OATH_RELOCATED_LEN as u64, top, 0xdead),
        Some((caller, top + 16, saved_rbx))
    );
    // The relay is a described leaf.
    assert_eq!(
        unwind_once(relay + 6, top + 8, 0x5151),
        Some((caller, top + 16, 0x5151))
    );
    // Neither range spills over, and the padding jump stays an undescribed leaf.
    let mut base = 0;
    for outside in [relay + RELAY_LEN as u64, trampoline + TRAMPOLINE_LEN as u64] {
        assert!(unsafe { RtlLookupFunctionEntry(outside, &mut base, null_mut()) }.is_null());
    }
    assert!(unsafe { RtlLookupFunctionEntry(prefix_slot as u64, &mut base, null_mut()) }.is_null());
}

fn page_bytes(page: *mut u8) -> Vec<u8> {
    unsafe { std::slice::from_raw_parts(page, CODE_PAGE) }.to_vec()
}

fn leaked_trace() -> &'static OathTrace {
    Box::leak(OathTrace::boxed())
}

fn plain_call(entry: *mut u8) -> extern "system" fn(u64, u64, u64, u64) -> u64 {
    unsafe { std::mem::transmute(entry) }
}

fn stable_records(trace: &OathTrace) -> Vec<OathRecord> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match trace.records() {
            Ok(rows) => return rows,
            Err(error) => {
                assert!(Instant::now() < deadline, "{error}");
                std::thread::yield_now();
            }
        }
    }
}

fn wait_quiet(trace: &OathTrace) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while trace.in_flight() != 0 {
        assert!(Instant::now() < deadline, "calls remain in flight");
        std::thread::yield_now();
    }
}

fn run_child(name: &str) {
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("JAC_OATH_SYNTHETIC_CHILD", "1")
        .creation_flags(0x0800_0000)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "owned oath child {name} failed: {status}");
            return;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("owned oath child {name} exceeded 60 seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn in_child() -> bool {
    std::env::var("JAC_OATH_SYNTHETIC_CHILD").as_deref() == Ok("1")
}

#[test]
fn abi_contract_runs_in_an_owned_child() {
    run_child("oath_trace::tests::abi_child");
}
#[test]
fn concurrent_stop_and_restore_run_in_an_owned_child() {
    run_child("oath_trace::tests::concurrency_child");
}
#[test]
fn unwind_runs_in_an_owned_child() {
    run_child("oath_trace::tests::unwind_child");
}
#[test]
fn refusals_run_in_an_owned_child() {
    run_child("oath_trace::tests::refusal_child");
}

#[test]
fn gateway_frames_describe_the_saved_state() {
    // Both gateways: one fixed allocation of 0xf8 (rcx/rdx saved above the xmm area, stack arguments 7 and 8 at 30h/38h) and a language handler for
    // unwinding only, so an exception through the original reaches us.
    for gateway in [
        oath_apply_gateway as *const () as u64,
        oath_remove_gateway as *const () as u64,
    ] {
        let mut image_base = 0;
        let entry = unsafe { RtlLookupFunctionEntry(gateway, &mut image_base, null_mut()) };
        assert!(!entry.is_null());
        let unwind = unsafe {
            ptr::read((image_base + (*entry).Anonymous.UnwindData as u64) as *const [u8; 8])
        };
        assert_eq!(unwind[0] & 7, 1, "unwind version");
        assert_eq!(unwind[0] >> 3, 2, "UNW_FLAG_UHANDLER only");
        assert_eq!(unwind[1], 16, "prolog covers the token zeroing");
        assert_eq!(unwind[5] & 15, 1, "one alloc opcode");
        assert_eq!(u16::from_le_bytes([unwind[6], unwind[7]]), 0xf8 / 8);
    }
}

#[test]
fn abi_child() {
    if !in_child() {
        return;
    }
    // The checker itself must see a clobbered r12 and xmm7 (a failing original).
    let bad = unsafe { build_page(oath_original_clobber) };
    OATH_TEST_ENTRY.store(unsafe { bad.add(0x20) }, Ordering::Release);
    let mut probe = Block::new(0, 1);
    unsafe { oath_abi_fixture(probe.address(), 0, 0, 0) };
    assert_eq!(probe.0[32], (1 << 4) | (1 << 9), "fixture detects clobbers");

    let page = unsafe { build_page(oath_original_body) };
    make_rx(page);
    let trace = leaked_trace();
    let (apply, remove) = unsafe { (page.add(0x20), page.add(0x80)) };
    let before = entry_head(apply);
    unsafe { install_oath_at(apply, OathCall::Apply, trace, before) }.unwrap();
    unsafe { install_oath_at(remove, OathCall::Remove, trace, entry_head(remove)) }.unwrap();
    assert_eq!(unsafe { ptr::read(apply.cast::<[u8; 2]>()) }, [0xeb, 0xf9]);
    assert_eq!(
        &entry_head(apply)[2..],
        &before[2..],
        "only push rbx changed"
    );
    assert_eq!(unsafe { *apply.sub(5) }, 0xe9);
    assert!(unsafe { install_oath_at(apply, OathCall::Apply, trace, before) }.is_err());
    let code = OATH_HOOKS.lock().unwrap()[0].as_ref().unwrap().code;
    check_unwind_tables(code, unsafe { apply.sub(OATH_PREFIX_PAD.len()) });

    let mut object = [0u64; 4];
    let mut subject = [object.as_mut_ptr() as u64, 0xc0ffee];
    let subject_address = subject.as_mut_ptr() as u64;
    let vtable = 0x7ff6_0000_0000 + 0xa7_6a30;
    let sentinel = 0xaabb_ccdd_1122_3301;
    let raw_r8 = 0xdead_beef_0000_0042;
    OATH_TEST_ENTRY.store(apply, Ordering::Release);
    let mut effect = Block::new(vtable, sentinel);
    let effect_address = effect.address();
    let returned = unsafe { oath_abi_fixture(effect_address, subject_address, raw_r8, 0x5555) };
    assert_eq!(returned, sentinel, "full RAX is the original's");
    let seen = &effect.0;
    assert_eq!(seen[1], 1, "original ran exactly once");
    assert_eq!(
        &seen[2..6],
        &[effect_address, subject_address, raw_r8, 0x5555]
    );
    assert_eq!(&seen[6..9], &[0x1122_3344, 0x2233_4455, 0x3344_5566]);
    assert_eq!(
        &seen[10..12],
        &XMM0_PATTERN,
        "xmm0 is loaded by the original itself"
    );
    assert_eq!(
        &seen[12..22],
        &[u64::MAX; 10],
        "xmm1..xmm5 reach the original"
    );
    for (n, nibble) in (1u64..=8).enumerate() {
        assert_eq!(
            seen[22 + n],
            nibble * 0x1111_1111_1111_1111,
            "nonvolatile {n}"
        );
    }
    assert_eq!(
        seen[30], 0,
        "original sees an aligned stack after its prologue"
    );
    assert_eq!(seen[32], 0, "nonvolatile GPR and XMM6-15 preserved");

    effect.0[31] = 0xaabb_ccdd_1122_3300;
    assert_eq!(
        unsafe { oath_abi_fixture(effect_address, 0, raw_r8, 1) },
        0xaabb_ccdd_1122_3300
    );
    assert_eq!(effect.0[32], 0);
    // Remove, a null-object shared_ptr and an unreadable subject.
    let mut empty_subject = [0u64, 9];
    OATH_TEST_ENTRY.store(remove, Ordering::Release);
    unsafe { oath_abi_fixture(effect_address, empty_subject.as_mut_ptr() as u64, 3, 0) };
    unsafe { oath_abi_fixture(effect_address, 0x10, 4, 0) };
    assert_eq!(effect.0[1], 4);

    // Direct callback edges: unreadable effect, then unreadable both.
    let mut claim = 0u64;
    unsafe {
        oath_begin(
            0x10,
            subject_address,
            8,
            1,
            0x99,
            &mut claim,
            0x1111,
            0x2222,
        )
    };
    assert_ne!(claim, 0, "the frame slot owns the claim");
    assert_eq!(trace.in_flight(), 1);
    assert_eq!(
        trace.records().unwrap().last().unwrap().state,
        OathState::Incomplete
    );
    unsafe { oath_end(&mut claim, 0x1234) };
    assert_eq!(claim, 0, "end consumed the claim");
    unsafe { oath_begin(0x10, 0x20, 9, 2, 0x99, &mut claim, 0x1111, 0x2222) };
    unsafe { oath_end(&mut claim, 0) };
    assert_eq!(trace.in_flight(), 0);

    let rows = trace.records().unwrap();
    assert_eq!(rows.len(), 6);
    let r = &rows[0];
    assert_eq!(
        (r.call, r.state, r.sequence),
        (OathCall::Apply, OathState::Completed, 1)
    );
    assert_eq!((r.instance_id, r.raw_r8), (0x42, raw_r8));
    assert_eq!((r.effect, r.subject), (effect_address, subject_address));
    assert_eq!(
        (r.effect_vtable, r.subject_object, r.subject_control),
        (vtable, object.as_ptr() as u64, 0xc0ffee)
    );
    assert_eq!((r.copy_flags, r.original_rax), (COPY_ALL, sentinel));
    assert!(r.original_al_true() && r.inputs_resolved() && r.completion_order == 1);
    let fixture = oath_abi_fixture as *const () as u64;
    assert!(
        r.caller > fixture && r.caller < fixture + 0x400,
        "caller is the fixture"
    );
    assert_ne!(r.thread_id, 0);
    // The caller held distinct sentinels in RBP and RDI (the fixture sets all
    // nonvolatile registers); the rows copy exactly those, not each other.
    for row in &rows[..4] {
        assert_eq!(
            (row.caller_rbp, row.caller_rdi),
            (0x2222_2222_2222_2222, 0x3333_3333_3333_3333)
        );
    }
    assert_eq!((rows[4].caller_rbp, rows[4].caller_rdi), (0x1111, 0x2222));
    assert!(!rows[1].original_al_true() && rows[1].input_copy_failed());
    assert_eq!(rows[1].copy_flags, COPY_EFFECT_VTABLE);
    assert_eq!((rows[2].call, rows[2].instance_id), (OathCall::Remove, 3));
    assert_eq!(
        rows[2].copy_flags,
        COPY_EFFECT_VTABLE | COPY_SUBJECT_POINTER
    );
    assert!(!rows[2].inputs_resolved() && !rows[2].input_copy_failed());
    assert_eq!(rows[3].copy_flags, COPY_EFFECT_VTABLE);
    assert_eq!(
        rows[4].copy_flags,
        COPY_SUBJECT_POINTER | COPY_SUBJECT_OBJECT_PRESENT
    );
    assert_eq!(rows[4].original_rax, 0x1234);
    assert_eq!((rows[5].copy_flags, rows[5].call), (0, OathCall::Remove));
    assert!(rows[5].input_copy_failed());

    // Finite capacity: no wrap, every extra call still runs the original once.
    let call = plain_call(apply);
    let extra = OATH_CAPACITY as u64 + 76;
    let counter = effect.0[1];
    for _ in 0..extra {
        assert_eq!(
            call(effect_address, subject_address, 7, 0),
            0xaabb_ccdd_1122_3300
        );
    }
    assert_eq!(effect.0[1], counter + extra);
    assert_eq!(trace.dropped() as u64, 6 + extra - OATH_CAPACITY as u64);
    assert_eq!(trace.in_flight(), 0);
    let full = trace.records().unwrap();
    assert_eq!(full.len(), OATH_CAPACITY);
    assert_eq!(&full[..6], &rows[..], "earlier rows are never overwritten");
    assert!(full.iter().all(|row| row.state == OathState::Completed));

    // Restore: both heads return, the call still works, nothing is recorded.
    assert_eq!(unsafe { uninstall_oath() }, Ok(true));
    assert_eq!(entry_head(apply), before);
    assert_eq!(unsafe { uninstall_oath() }, Ok(false));
    check_unwind_tables(code, unsafe { apply.sub(OATH_PREFIX_PAD.len()) });
    assert_eq!(page_protection_of(apply), Some(PAGE_EXECUTE_READ));
    let dropped = trace.dropped();
    assert_eq!(
        call(effect_address, subject_address, 7, 0),
        0xaabb_ccdd_1122_3300
    );
    assert_eq!(effect.0[1], counter + extra + 1);
    assert_eq!(trace.dropped(), dropped);
}

#[test]
fn concurrency_child() {
    if !in_child() {
        return;
    }
    let page = unsafe { build_page(oath_original_body) };
    let trace = leaked_trace();
    let entries = [page as usize + 0x20, page as usize + 0x80];
    let running = Arc::new(AtomicBool::new(true));
    let total = Arc::new(AtomicU64::new(0));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let mut workers = Vec::new();
    for worker in 0..4u64 {
        let (running, total, ready_tx) = (running.clone(), total.clone(), ready_tx.clone());
        workers.push(std::thread::spawn(move || {
            let mut block = Block::new(0x1000 + worker, 0x55);
            let effect = block.address();
            let object = [worker; 2];
            let subject = [object.as_ptr() as u64, 1];
            let calls = [
                plain_call(entries[0] as *mut u8),
                plain_call(entries[1] as *mut u8),
            ];
            let mut made = 0u64;
            let mut ready = false;
            while running.load(Ordering::Acquire) {
                let r8 = ((0xabcd_0000 + worker) << 32) | (made & 0xffff_ffff);
                assert_eq!(
                    calls[(made & 1) as usize](effect, subject.as_ptr() as u64, r8, 0),
                    0x55
                );
                made += 1;
                total.fetch_add(1, Ordering::Relaxed);
                if !ready {
                    ready = true;
                    ready_tx.send(()).unwrap();
                }
            }
            (effect, made, block.0[1])
        }));
    }
    for _ in 0..4 {
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    // Install while four threads are inside the entries.
    for (n, call) in [OathCall::Apply, OathCall::Remove].into_iter().enumerate() {
        let entry = entries[n] as *mut u8;
        unsafe { install_oath_at(entry, call, trace, entry_head(entry)) }.unwrap();
    }
    std::thread::sleep(Duration::from_millis(30));
    assert!(!stable_records(trace).is_empty());
    // Stop while the callers keep running: counters freeze, calls continue.
    trace.stop();
    wait_quiet(trace);
    let (next, dropped, frozen) = (
        trace.next.load(Ordering::SeqCst),
        trace.dropped(),
        stable_records(trace),
    );
    let calls_at_stop = total.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(30));
    assert!(
        total.load(Ordering::Relaxed) > calls_at_stop,
        "callers still run"
    );
    assert_eq!(
        (trace.next.load(Ordering::SeqCst), trace.dropped()),
        (next, dropped)
    );
    // Remember the resident relays, then restore under load.
    let relays: Vec<*mut u8> = OATH_HOOKS
        .lock()
        .unwrap()
        .iter()
        .map(|hook| hook.as_ref().unwrap().code)
        .collect();
    assert_eq!(unsafe { uninstall_oath() }, Ok(true));
    std::thread::sleep(Duration::from_millis(10));
    running.store(false, Ordering::Release);
    let mut effects = Vec::new();
    for worker in workers {
        let (effect, made, original_ran) = worker.join().unwrap();
        assert_eq!(made, original_ran, "original ran exactly once per call");
        effects.push(effect);
    }
    assert_eq!(trace.in_flight(), 0);
    let rows = stable_records(trace);
    assert_eq!(rows, frozen, "nothing recorded after stop");
    let mut orders: Vec<u32> = rows.iter().map(|row| row.completion_order).collect();
    orders.sort_unstable();
    orders.dedup();
    assert_eq!(orders.len(), rows.len(), "completion order is unique");
    for (n, row) in rows.iter().enumerate() {
        assert_eq!(row.sequence as usize, n + 1);
        assert_eq!(row.state, OathState::Completed);
        let worker = effects
            .iter()
            .position(|e| *e == row.effect)
            .expect("known effect");
        assert_eq!(
            row.raw_r8 >> 32,
            0xabcd_0000 + worker as u64,
            "row stays with its caller"
        );
        assert_eq!(row.effect_vtable, 0x1000 + worker as u64);
        assert_eq!(row.original_rax, 0x55);
        assert_eq!(row.copy_flags, COPY_ALL);
    }
    assert!(rows.iter().any(|row| row.call == OathCall::Apply));
    assert!(rows.iter().any(|row| row.call == OathCall::Remove));
    // A thread that already entered a relay before restoration still finishes
    // on resident code, runs the original once and leaves the stopped trace alone.
    assert!(!OATH_TRACE_PTR.load(Ordering::SeqCst).is_null());
    assert!(!OATH_APPLY_TRAMPOLINE.load(Ordering::SeqCst).is_null());
    assert!(!OATH_REMOVE_TRAMPOLINE.load(Ordering::SeqCst).is_null());
    for relay in relays {
        let mut block = Block::new(0x1000, 0x66);
        let address = block.address();
        assert_eq!(plain_call(relay)(address, 0, 1, 0), 0x66);
        assert_eq!(block.0[1], 1);
    }
    assert_eq!(stable_records(trace), rows);
    assert_eq!(trace.in_flight(), 0);
}

#[test]
fn unwind_child() {
    if !in_child() {
        return;
    }
    let page = unsafe { build_page(oath_original_raise) };
    let trace = leaked_trace();
    let apply = unsafe { page.add(0x20) };
    unsafe { install_oath_at(apply, OathCall::Apply, trace, entry_head(apply)) }.unwrap();
    OATH_TEST_ENTRY.store(apply, Ordering::Release);
    let (effect, subject) = (0x1000u64, 0x20u64);
    let raise = |armed: bool| {
        OATH_TEST_RAISE.store(armed, Ordering::SeqCst);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            oath_unwind_fixture(effect, subject, 5, 0)
        }))
    };
    assert_eq!(raise(false).unwrap(), 0x77);
    assert!(raise(true).is_err(), "the exception crossed the gateway");
    let rows = trace.records().unwrap();
    assert_eq!(
        (rows[0].state, rows[0].original_rax),
        (OathState::Completed, 0x77)
    );
    assert_eq!(rows[1].state, OathState::Unwound);
    assert_eq!(
        rows[1].original_rax, 0,
        "no result for a call that never returned"
    );
    assert_eq!(rows[1].completion_order, 0);
    assert_eq!(trace.in_flight(), 0);
    assert_eq!(OATH_TEST_CALLS.load(Ordering::SeqCst), 2);

    // An exception at every callback stage settles the claim exactly once
    // (a second settlement would wrap the counter, a missed one leave it at 1).
    let stage_case = |stage: u32, original_runs: bool| {
        let rows_before = trace.records().unwrap().len();
        let calls_before = OATH_TEST_CALLS.load(Ordering::SeqCst);
        OATH_STAGE.store(stage, Ordering::SeqCst);
        assert!(raise(false).is_err(), "stage {stage} raises");
        assert_eq!(OATH_STAGE.load(Ordering::SeqCst), 0, "stage {stage} fired");
        assert_eq!(trace.in_flight(), 0, "stage {stage} settled once");
        assert_eq!(
            OATH_TEST_CALLS.load(Ordering::SeqCst),
            calls_before + u32::from(original_runs),
            "stage {stage} original count"
        );
        (rows_before, trace.records().unwrap())
    };
    let (before, rows) = stage_case(STAGE_BEGIN_COUNTED, false);
    assert_eq!(rows.len(), before, "no row was reserved yet");
    let (before, rows) = stage_case(STAGE_BEGIN_RESERVED, false);
    let row = &rows[before];
    assert_eq!(
        (rows.len(), row.state, row.call),
        (before + 1, OathState::Unwound, OathCall::Apply)
    );
    assert_eq!(row.effect, 0, "reserved but never filled");
    let (before, rows) = stage_case(STAGE_BEGIN_PUBLISHED, false);
    let row = &rows[before];
    assert_eq!((rows.len(), row.state), (before + 1, OathState::Unwound));
    assert_eq!(
        (row.effect, row.copy_flags),
        (effect, 0),
        "filled, nothing readable"
    );
    let (before, rows) = stage_case(STAGE_END_ENTERED, true);
    let row = &rows[before];
    assert_eq!(
        (rows.len(), row.state, row.original_rax),
        (before + 1, OathState::Unwound, 0)
    );

    // An unwound call that found the buffer full also gives its count back.
    OATH_TEST_RAISE.store(false, Ordering::SeqCst);
    let call = plain_call(apply);
    for _ in 0..OATH_CAPACITY {
        assert_eq!(call(effect, subject, 5, 0), 0x77);
    }
    let dropped = trace.dropped();
    assert!(dropped > 0);
    let calls = OATH_TEST_CALLS.load(Ordering::SeqCst);
    assert!(raise(true).is_err());
    assert_eq!(trace.in_flight(), 0);
    assert_eq!(trace.dropped(), dropped + 1);
    assert_eq!(OATH_TEST_CALLS.load(Ordering::SeqCst), calls + 1);
    // A full-buffer claim (no row) is also settled once at both ends.
    OATH_STAGE.store(STAGE_BEGIN_COUNTED, Ordering::SeqCst);
    assert!(raise(false).is_err());
    assert_eq!((trace.in_flight(), trace.dropped()), (0, dropped + 1));
    OATH_STAGE.store(STAGE_END_ENTERED, Ordering::SeqCst);
    assert!(raise(false).is_err());
    assert_eq!((trace.in_flight(), trace.dropped()), (0, dropped + 2));
    assert_eq!(OATH_TEST_CALLS.load(Ordering::SeqCst), calls + 2);
    assert_eq!(stable_records(trace).len(), OATH_CAPACITY);
    assert_eq!(raise(false).unwrap(), 0x77);
    assert_eq!(trace.in_flight(), 0);
}

#[test]
fn refusal_child() {
    if !in_child() {
        return;
    }
    let page = unsafe { build_page(oath_original_body) };
    let apply = unsafe { page.add(0x20) };
    let good_head = entry_head(apply);
    let snapshot = page_bytes(page);
    let trace = leaked_trace();
    let mut wrong_head = good_head;
    wrong_head[7] = 0x11;
    let mut old_version = OathTrace::boxed();
    old_version.version = 0;
    let old_version: &'static OathTrace = Box::leak(old_version);
    let refused = |target: *mut u8, call, trace: &'static OathTrace, expected| {
        assert!(unsafe { install_oath_at(target, call, trace, expected) }.is_err());
        assert!(OATH_TRACE_PTR.load(Ordering::SeqCst).is_null());
        assert!(OATH_APPLY_TRAMPOLINE.load(Ordering::SeqCst).is_null());
    };
    refused(apply, OathCall::Apply, trace, wrong_head);
    refused(apply, OathCall::Apply, old_version, good_head);
    refused(unsafe { apply.add(1) }, OathCall::Apply, trace, good_head);
    refused(unsafe { page.add(2) }, OathCall::Apply, trace, good_head);
    refused(null_mut(), OathCall::Apply, trace, good_head);
    // Diverged bytes in the file: the movups opcode and the INT3 padding.
    unsafe { *apply.add(7) = 0x11 };
    refused(apply, OathCall::Apply, trace, wrong_head);
    unsafe { *apply.add(7) = 0x10 };
    unsafe { *apply.sub(1) = 0x90 };
    refused(apply, OathCall::Apply, trace, good_head);
    unsafe { *apply.sub(1) = 0xcc };
    assert_eq!(page_bytes(page), snapshot, "refusals changed no byte");

    unsafe { install_oath_at(apply, OathCall::Apply, trace, good_head) }.unwrap();
    let other = leaked_trace();
    let remove = unsafe { page.add(0x80) };
    let remove_bytes = unsafe { std::slice::from_raw_parts(page.add(0x70), 0x20) }.to_vec();
    assert!(
        unsafe { install_oath_at(remove, OathCall::Remove, other, entry_head(remove)) }.is_err()
    );
    assert_eq!(
        unsafe { std::slice::from_raw_parts(page.add(0x70), 0x20) },
        &remove_bytes[..],
        "a second trace is refused before any write"
    );
    assert_eq!(unsafe { uninstall_oath() }, Ok(true));
}

#[test]
fn handler_contract_runs_in_an_owned_child() {
    run_child("oath_trace::tests::handler_child");
}
#[test]
fn install_failures_run_in_an_owned_child() {
    run_child("oath_trace::tests::install_failure_child");
}

#[test]
fn handler_child() {
    if !in_child() {
        return;
    }
    let page = unsafe { build_page(oath_original_body) };
    make_rx(page);
    let trace = leaked_trace();
    let apply = unsafe { page.add(0x20) };
    unsafe { install_oath_at(apply, OathCall::Apply, trace, entry_head(apply)) }.unwrap();
    // A stand-in gateway frame: the token slot lives at frame + 0x68.
    let mut frame = [0u64; 32];
    let frame_address = frame.as_mut_ptr() as u64;
    let slot = unsafe { frame.as_mut_ptr().add(13) };
    let mut unwinding = [0u8; 16];
    unwinding[4] = 2; // EXCEPTION_UNWINDING
    let searching = [0u8; 16];
    let handler = |record: &[u8; 16]| unsafe {
        oath_unwind_handler(record.as_ptr(), frame_address, null_mut(), null_mut())
    };
    let claim = || unsafe { ptr::read_volatile(slot) };

    // A call that finished leaves no claim for a late unwind to settle again.
    unsafe { oath_begin(0x10, 0x20, 5, 1, 0x99, slot, 0x1111, 0x2222) };
    assert_eq!(trace.in_flight(), 1);
    unsafe { oath_end(slot, 7) };
    assert_eq!((trace.in_flight(), claim()), (0, 0));
    assert_eq!(handler(&unwinding), 1);
    assert_eq!(trace.in_flight(), 0, "no double settlement after end");
    assert_eq!(trace.records().unwrap()[0].state, OathState::Completed);

    // The search pass does not settle; the unwind pass settles exactly once.
    unsafe { oath_begin(0x10, 0x20, 6, 2, 0x99, slot, 0x1111, 0x2222) };
    assert_eq!(handler(&searching), 1);
    assert_eq!((trace.in_flight(), claim() != 0), (1, true));
    assert_eq!(handler(&unwinding), 1);
    assert_eq!((trace.in_flight(), claim()), (0, 0));
    assert_eq!(handler(&unwinding), 1);
    assert_eq!(trace.in_flight(), 0, "a second unwind finds nothing");
    let rows = trace.records().unwrap();
    assert_eq!(
        (rows[1].state, rows[1].call),
        (OathState::Unwound, OathCall::Remove)
    );

    // A stopped trace claims nothing at all.
    trace.stop();
    unsafe { oath_begin(0x10, 0x20, 7, 1, 0x99, slot, 0x1111, 0x2222) };
    assert_eq!((trace.in_flight(), claim()), (0, 0));
    assert_eq!(unsafe { uninstall_oath() }, Ok(true));
}

#[test]
fn a_lost_entry_race_runs_in_an_owned_child() {
    run_child("oath_trace::tests::race_child");
}
#[test]
fn unverified_entry_publication_runs_in_owned_children() {
    run_child_case("oath_trace::tests::unverified_entry_child", "restore");
    run_child_case("oath_trace::tests::unverified_entry_child", "flush");
}
#[test]
fn uninstall_failures_run_in_owned_children() {
    for case in ["enable", "restore", "flush"] {
        run_child_case("oath_trace::tests::uninstall_failure_child", case);
    }
}
#[test]
fn frozen_after_prefix_runs_in_owned_children() {
    for case in ["flush", "restore", "entry_enable"] {
        run_child_case("oath_trace::tests::frozen_after_prefix_child", case);
    }
}
#[test]
fn receipt_retention_runs_in_owned_children() {
    for case in ["flush", "restore", "never"] {
        run_child_case("oath_trace::tests::rollback_retention_child", case);
    }
    run_child("oath_trace::tests::lost_race_retention_child");
}
#[test]
fn a_foreign_entry_window_never_unlatches_runs_in_an_owned_child() {
    run_child("oath_trace::tests::reachability_latch_child");
}
#[test]
fn a_reachable_binding_is_frozen_runs_in_an_owned_child() {
    run_child("oath_trace::tests::binding_freeze_child");
}

fn run_child_case(name: &str, case: &str) {
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("JAC_OATH_SYNTHETIC_CHILD", "1")
        .env("JAC_OATH_CASE", case)
        .creation_flags(0x0800_0000)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "owned oath child {name}/{case} failed: {status}"
            );
            return;
        }
        assert!(
            Instant::now() < deadline,
            "owned oath child {name}/{case} timed out"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn case() -> String {
    std::env::var("JAC_OATH_CASE").unwrap_or_default()
}

fn uninstall_now() -> Result<bool, &'static str> {
    unsafe { uninstall_oath() }
}
fn mask(sites: &[u32]) -> u64 {
    sites.iter().fold(0, |m, s| m | 1u64 << s)
}
fn arm(sites: &[u32]) {
    OATH_FAULTS.store(mask(sites), Ordering::SeqCst);
}
fn arm_persistent(sites: &[u32]) {
    OATH_FAULTS_PERSIST.store(mask(sites), Ordering::SeqCst);
}
fn fresh_rx_page() -> *mut u8 {
    let page = unsafe { build_page(oath_original_body) };
    make_rx(page);
    page
}
fn install_apply(page: *mut u8, trace: &'static OathTrace) -> Result<(), &'static str> {
    unsafe { install_oath_at(page.add(0x20), OathCall::Apply, trace, OATH_ENTRY_HEAD) }
}
fn prefix_of(page: *mut u8) -> [u8; 5] {
    unsafe { ptr::read(page.add(0x1b).cast::<[u8; 5]>()) }
}
fn entry_of(page: *mut u8) -> [u8; 8] {
    entry_head(unsafe { page.add(0x20) })
}
fn poke_entry(entry: *mut u8, bytes: [u8; 2]) {
    let mut old = 0u32;
    unsafe {
        VirtualProtect(entry.cast(), 2, PAGE_EXECUTE_READWRITE, &mut old);
        ptr::write(entry.cast::<[u8; 2]>(), bytes);
        VirtualProtect(entry.cast(), 2, old, &mut old);
    }
}
fn hooks_idle() -> bool {
    OATH_HOOKS.lock().unwrap().iter().all(Option::is_none)
}
fn apply_ledger() -> Option<Ledger> {
    OATH_HOOKS.lock().unwrap()[0].as_ref().map(|r| r.ledger)
}
fn protection(page: *mut u8) -> Option<u32> {
    page_protection_of(page)
}

/// Failures before any byte of ours is written are undone completely:
/// bytes, protection, flush and every reservation come back.
#[test]
fn install_failure_child() {
    if !in_child() {
        return;
    }
    let trace = leaked_trace();
    for site in [SITE_TABLE_ADD, SITE_CODE_PROTECT, SITE_PREFIX_ENABLE] {
        let page = fresh_rx_page();
        let snapshot = page_bytes(page);
        arm(&[site]);
        assert!(install_apply(page, trace).is_err());
        assert_eq!(
            OATH_FAULTS.load(Ordering::SeqCst),
            0,
            "site {site} was reached"
        );
        assert_eq!(page_bytes(page), snapshot, "site {site} left bytes behind");
        assert_eq!(protection(page), Some(PAGE_EXECUTE_READ), "site {site}");
        assert!(hooks_idle());
        assert!(OATH_TRACE_PTR.load(Ordering::SeqCst).is_null());
        assert!(OATH_APPLY_TRAMPOLINE.load(Ordering::SeqCst).is_null());
    }

    // Another writer changed the padding after the checks: refused without
    // writing a byte, the foreign byte stays, and the reservations return.
    let page = fresh_rx_page();
    arm(&[SITE_PREFIX_RACE]);
    assert!(install_apply(page, trace)
        .unwrap_err()
        .contains("nothing was written"));
    assert_eq!(prefix_of(page), [0x90, 0xcc, 0xcc, 0xcc, 0xcc]);
    assert_eq!(protection(page), Some(PAGE_EXECUTE_READ));
    assert!(hooks_idle());
    assert!(OATH_TRACE_PTR.load(Ordering::SeqCst).is_null());
    assert!(OATH_APPLY_TRAMPOLINE.load(Ordering::SeqCst).is_null());

    // A released binding can take a different target: nothing could ever have
    // reached the old one.
    let page = fresh_rx_page();
    install_apply(page, trace).unwrap();
    assert_eq!(uninstall_now(), Ok(true));
    assert!(apply_ledger().unwrap().frozen());
}

/// From the first written prefix byte the reach is latched: every later
/// failure keeps the prefix, the binding and the reservations for good.
#[test]
fn frozen_after_prefix_child() {
    if !in_child() {
        return;
    }
    let trace = leaked_trace();
    let page = fresh_rx_page();
    let entry = unsafe { page.add(0x20) };
    arm(&[match case().as_str() {
        "flush" => SITE_PREFIX_FLUSH,
        "restore" => SITE_PREFIX_RESTORE,
        _ => SITE_ENTRY_ENABLE,
    }]);
    let error = install_apply(page, trace).unwrap_err();
    assert!(error.contains("frozen"), "{error}");
    assert_eq!(OATH_FAULTS.load(Ordering::SeqCst), 0);
    let ledger = apply_ledger().unwrap();
    assert_eq!(
        (ledger.reach(), ledger.binding()),
        (Reach::Possibly, Binding::Frozen)
    );
    assert_ne!(
        prefix_of(page),
        OATH_PREFIX_PAD,
        "the padding is never recycled"
    );
    assert_eq!(entry_of(page), OATH_ENTRY_HEAD, "entry never touched");
    assert_eq!(
        protection(page),
        Some(PAGE_EXECUTE_READ),
        "repaired after the failure"
    );
    assert!(!OATH_TRACE_PTR.load(Ordering::SeqCst).is_null());
    assert!(install_apply(page, trace).unwrap_err().contains("frozen"));
    // A foreign writer branches into the prefix and back to the original
    // bytes: the prefix still runs the original once and the latch holds.
    poke_entry(entry, PROPERTY_SHORT_JUMP);
    let mut block = Block::new(0x1000, 0x66);
    let address = block.address();
    assert_eq!(plain_call(entry)(address, 0, 1, 0), 0x66);
    assert_eq!((block.0[1], trace.records().unwrap().len()), (1, 1));
    poke_entry(entry, [0x40, 0x53]);
    assert_eq!(uninstall_now(), Ok(true));
    assert_ne!(prefix_of(page), OATH_PREFIX_PAD);
    assert!(apply_ledger().unwrap().frozen());
    assert_eq!(uninstall_now(), Ok(false));
}

/// A foreign writer owns the entry: its bytes stay, our prefix stays resident
/// and the binding is frozen.
#[test]
fn race_child() {
    if !in_child() {
        return;
    }
    let trace = leaked_trace();
    let page = fresh_rx_page();
    arm(&[SITE_ENTRY_RACE]);
    assert!(install_apply(page, trace).unwrap_err().contains("changed"));
    let ledger = apply_ledger().unwrap();
    assert_eq!(
        (ledger.reach(), ledger.binding()),
        (Reach::Possibly, Binding::Frozen)
    );
    assert_eq!(&entry_of(page)[..2], &[0x90, 0x90]);
    let published = prefix_of(page);
    assert_ne!(published, OATH_PREFIX_PAD);
    assert!(install_apply(page, trace).is_err());
    assert_eq!(uninstall_now(), Ok(true));
    assert_eq!(
        (&entry_of(page)[..2], prefix_of(page)),
        (&[0x90u8, 0x90][..], published)
    );
    assert_eq!(protection(page), Some(PAGE_EXECUTE_READ));
    assert_eq!(
        uninstall_now(),
        Ok(false),
        "a settled tombstone reports once"
    );
    assert!(!hooks_idle() && apply_ledger().unwrap().frozen());
}

/// Entry exchanged but protection or flush unverified: the entry is ours and
/// uninstall restores it with the protection.
#[test]
fn unverified_entry_child() {
    if !in_child() {
        return;
    }
    let trace = leaked_trace();
    let page = fresh_rx_page();
    let restore = case() == "restore";
    arm(&[if restore {
        SITE_ENTRY_RESTORE
    } else {
        SITE_ENTRY_FLUSH
    }]);
    assert!(install_apply(page, trace)
        .unwrap_err()
        .contains("unverified"));
    assert_eq!(OATH_FAULTS.load(Ordering::SeqCst), 0);
    let ledger = apply_ledger().unwrap();
    assert_eq!((ledger.entry.bytes, ledger.frozen()), (Bytes::Ours, true));
    assert!(!ledger.entry.confirm.complete());
    assert_eq!(&entry_of(page)[..2], &PROPERTY_SHORT_JUMP);
    assert_eq!(protection(page) == Some(PAGE_EXECUTE_READWRITE), restore);
    assert_eq!(uninstall_now(), Ok(true));
    assert_eq!(entry_of(page), OATH_ENTRY_HEAD);
    assert_eq!(protection(page), Some(PAGE_EXECUTE_READ));
    assert_eq!(uninstall_now(), Ok(false));
    assert!(install_apply(page, trace).unwrap_err().contains("frozen"));
}

/// Uninstall failures keep the receipt until the entry is verified.
#[test]
fn uninstall_failure_child() {
    if !in_child() {
        return;
    }
    let trace = leaked_trace();
    let page = fresh_rx_page();
    install_apply(page, trace).unwrap();
    let site = match case().as_str() {
        "enable" => SITE_UNINSTALL_ENABLE,
        "restore" => SITE_UNINSTALL_RESTORE,
        _ => SITE_UNINSTALL_FLUSH,
    };
    arm(&[site]);
    let first = uninstall_now();
    assert_eq!(OATH_FAULTS.load(Ordering::SeqCst), 0);
    // Without a writable page nothing can be restored; after a completed
    // exchange the same call re-verifies protection and flush by itself.
    assert_eq!(first.is_err(), case() == "enable");
    if case() == "enable" {
        assert_eq!(
            &entry_of(page)[..2],
            &PROPERTY_SHORT_JUMP,
            "nothing was written"
        );
    } else {
        // The exchange completed; protection or flush is still owed. A repair
        // inside the same call may already have paid it off.
        assert_eq!(entry_of(page), OATH_ENTRY_HEAD);
    }
    assert!(uninstall_now().is_ok());
    assert_eq!(entry_of(page), OATH_ENTRY_HEAD);
    assert_eq!(protection(page), Some(PAGE_EXECUTE_READ));
    assert!(apply_ledger().unwrap().frozen());
}

/// M1: a receipt outlives every unconfirmed byte, protection or flush.
#[test]
fn rollback_retention_child() {
    if !in_child() {
        return;
    }
    let trace = leaked_trace();
    let page = fresh_rx_page();
    match case().as_str() {
        // The flush of the published prefix keeps failing, also in repair.
        "flush" => {
            arm(&[SITE_PREFIX_FLUSH]);
            arm_persistent(&[SITE_REPAIR_PREFIX_FLUSH]);
            assert!(install_apply(page, trace).is_err());
            assert!(!apply_ledger().unwrap().prefix.confirm.flush);
            assert!(uninstall_now().is_err(), "flush unconfirmed");
            assert!(!hooks_idle());
            arm_persistent(&[]);
            assert_eq!(uninstall_now(), Ok(true));
            assert!(apply_ledger().unwrap().prefix.confirm.complete());
        }
        // The page stays writable: restoring failed on the write and on every
        // repair, so the receipt is kept until RX is confirmed.
        "restore" => {
            arm_persistent(&[
                SITE_PREFIX_RESTORE,
                SITE_REPAIR_PREFIX_RESTORE,
                SITE_REPAIR_ENTRY_RESTORE,
            ]);
            assert!(install_apply(page, trace).unwrap_err().contains("frozen"));
            assert_eq!(protection(page), Some(PAGE_EXECUTE_READWRITE));
            assert!(uninstall_now().is_err());
            assert_eq!(protection(page), Some(PAGE_EXECUTE_READWRITE));
            arm_persistent(&[]);
            assert_eq!(uninstall_now(), Ok(true));
            assert_eq!(protection(page), Some(PAGE_EXECUTE_READ), "RX confirmed");
        }
        // Nothing was written, yet the reservations wait for a confirmed RX.
        _ => {
            arm(&[SITE_PREFIX_RACE]);
            arm_persistent(&[SITE_PREFIX_RESTORE, SITE_REPAIR_PREFIX_RESTORE]);
            assert!(install_apply(page, trace).is_err());
            assert_eq!(protection(page), Some(PAGE_EXECUTE_READWRITE));
            assert!(!hooks_idle(), "bytes alone do not free the receipt");
            assert!(uninstall_now().is_err());
            assert!(!hooks_idle());
            arm_persistent(&[]);
            assert_eq!(uninstall_now(), Ok(true));
            assert_eq!(protection(page), Some(PAGE_EXECUTE_READ), "RX confirmed");
            assert!(hooks_idle());
            assert!(OATH_TRACE_PTR.load(Ordering::SeqCst).is_null());
            // The released binding is free for a clean install elsewhere.
            let other = fresh_rx_page();
            install_apply(other, trace).unwrap();
            assert_eq!(uninstall_now(), Ok(true));
        }
    }
}

/// M1: a lost entry race whose protection cannot be restored keeps the
/// receipt until a repair is confirmed.
#[test]
fn lost_race_retention_child() {
    if !in_child() {
        return;
    }
    let trace = leaked_trace();
    let page = fresh_rx_page();
    arm(&[SITE_ENTRY_RACE]);
    arm_persistent(&[SITE_ENTRY_RESTORE, SITE_REPAIR_ENTRY_RESTORE]);
    assert!(install_apply(page, trace).is_err());
    assert_eq!(protection(page), Some(PAGE_EXECUTE_READWRITE));
    assert!(uninstall_now().is_err(), "protection is still unconfirmed");
    assert_eq!(protection(page), Some(PAGE_EXECUTE_READWRITE));
    arm_persistent(&[]);
    assert_eq!(uninstall_now(), Ok(true));
    assert_eq!(protection(page), Some(PAGE_EXECUTE_READ));
}

/// M2: a foreign entry that could reach the prefix latches it for good, even
/// if the entry later shows the original bytes again (ABA).
#[test]
fn reachability_latch_child() {
    if !in_child() {
        return;
    }
    let trace = leaked_trace();
    let page = fresh_rx_page();
    let entry = unsafe { page.add(0x20) };
    arm(&[SITE_ENTRY_RACE]);
    assert!(install_apply(page, trace).is_err());
    let jump = prefix_of(page);
    assert_ne!(jump, OATH_PREFIX_PAD);
    let poke = |bytes: [u8; 2]| {
        let mut old = 0u32;
        unsafe {
            VirtualProtect(entry.cast(), 2, PAGE_EXECUTE_READWRITE, &mut old);
            ptr::write(entry.cast::<[u8; 2]>(), bytes);
            VirtualProtect(entry.cast(), 2, old, &mut old);
        }
    };
    // The foreign writer branches into our prefix, then puts the original back.
    poke(PROPERTY_SHORT_JUMP);
    let mut block = Block::new(0x1000, 0x66);
    let address = block.address();
    assert_eq!(plain_call(entry)(address, 0, 1, 0), 0x66);
    assert_eq!(
        (block.0[1], trace.records().unwrap().len()),
        (1, 1),
        "reachable prefix works"
    );
    poke([0x40, 0x53]);
    assert_eq!(entry_head(entry), OATH_ENTRY_HEAD);
    let _ = uninstall_now();
    assert_eq!(
        prefix_of(page),
        jump,
        "a once reachable prefix is never recycled"
    );
    let ledger = apply_ledger().unwrap();
    assert_eq!(
        (ledger.reach(), ledger.binding()),
        (Reach::Possibly, Binding::Frozen)
    );
    assert!(install_apply(page, trace).is_err());
}

/// M3: after the old relay could run, the call keeps its target forever.
#[test]
fn binding_freeze_child() {
    if !in_child() {
        return;
    }
    let trace = leaked_trace();
    let first = fresh_rx_page();
    let second = unsafe { build_page(oath_original_other) };
    make_rx(second);
    install_apply(first, trace).unwrap();
    let relay = OATH_HOOKS.lock().unwrap()[0].as_ref().unwrap().code;
    assert_eq!(uninstall_now(), Ok(true));
    assert!(
        install_apply(second, trace).is_err(),
        "another target is refused"
    );
    assert_eq!(entry_head(unsafe { second.add(0x20) }), OATH_ENTRY_HEAD);
    assert!(OATH_APPLY_TRAMPOLINE.load(Ordering::SeqCst) as usize != 0);
    // A late caller still inside the old relay reaches only the old original.
    let mut block = Block::new(0x1000, 0x66);
    let address = block.address();
    assert_eq!(plain_call(relay)(address, 0, 1, 0), 0x66);
    assert_eq!(
        (block.0[1], block.0[33]),
        (1, 0),
        "old original ran, new one did not"
    );
}

#[test]
fn the_shared_trace_follows_the_last_binding_runs_in_an_owned_child() {
    run_child("oath_trace::tests::shared_trace_child");
}

/// The trace belongs to the aggregate of both call types, not to whichever
/// install claimed it first: it is returned with the last binding.
#[test]
fn shared_trace_child() {
    if !in_child() {
        return;
    }
    let first_trace = leaked_trace();
    let apply_page = fresh_rx_page();
    let remove_page = fresh_rx_page();
    let install_remove = |page: *mut u8| unsafe {
        install_oath_at(
            page.add(0x80),
            OathCall::Remove,
            first_trace,
            OATH_ENTRY_HEAD,
        )
    };
    // Both kinds fail on a foreign padding byte and cannot confirm RX again.
    arm_persistent(&[SITE_PREFIX_RESTORE, SITE_REPAIR_PREFIX_RESTORE]);
    arm(&[SITE_PREFIX_RACE]);
    assert!(install_apply(apply_page, first_trace).is_err());
    arm(&[SITE_PREFIX_RACE]);
    assert!(install_remove(remove_page).is_err());
    assert!(OATH_HOOKS.lock().unwrap().iter().all(Option::is_some));
    assert_eq!(protection(apply_page), Some(PAGE_EXECUTE_READWRITE));
    assert_eq!(protection(remove_page), Some(PAGE_EXECUTE_READWRITE));
    // Both recover; Apply is released first, Remove (which never claimed the
    // trace) last. The trace must go with the last one.
    arm_persistent(&[]);
    assert_eq!(uninstall_now(), Ok(true));
    assert!(hooks_idle());
    assert_eq!(protection(apply_page), Some(PAGE_EXECUTE_READ));
    assert_eq!(protection(remove_page), Some(PAGE_EXECUTE_READ));
    assert!(
        OATH_TRACE_PTR.load(Ordering::SeqCst).is_null(),
        "no receipt is left, so no trace is claimed"
    );
    let second_trace = leaked_trace();
    let fresh = fresh_rx_page();
    install_apply(fresh, second_trace).unwrap();
    assert_eq!(uninstall_now(), Ok(true));
}
