use super::*;
use windows_sys::Win32::System::Diagnostics::Debug::{
    CONTEXT, EXCEPTION_RECORD, IMAGE_RUNTIME_FUNCTION_ENTRY, RtlLookupFunctionEntry,
};
// Exact Windows SDK signatures; local declarations avoid touching the shared
// crate feature list owned by the runtime integration writer. Optional output
// pointer types are opaque here and are always passed null.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn RtlUnwindEx(
        frame: *const std::ffi::c_void,
        ip: *const std::ffi::c_void,
        record: *const EXCEPTION_RECORD,
        value: *const std::ffi::c_void,
        context: *const CONTEXT,
        history: *const std::ffi::c_void,
    );
    fn RtlVirtualUnwind(
        kind: u32,
        base: u64,
        pc: u64,
        entry: *const IMAGE_RUNTIME_FUNCTION_ENTRY,
        context: *mut CONTEXT,
        data: *mut *mut std::ffi::c_void,
        frame: *mut u64,
        pointers: *mut std::ffi::c_void,
    ) -> usize;
}

core::arch::global_asm!(
    ".globl ai_fixture_target",
    "ai_fixture_target:",
    "mov [rcx],rdx",
    "mov [rcx+8],r8",
    "mov [rcx+16],r9",
    "mov [rcx+24],r10",
    "movdqu [rcx+32],xmm0",
    "movdqu [rcx+48],xmm5",
    "mov rax,[rsp+0x28]",
    "mov [rcx+64],rax",
    "mov rax,[rsp+0x60]",
    "mov [rcx+72],rax",
    "mov rax,[rsp+8]",
    "mov [rcx+96],rax",
    "mov rax,[rsp+0x20]",
    "mov [rcx+104],rax",
    "pushfq",
    "pop qword ptr [rcx+80]",
    "inc qword ptr [rcx+88]",
    "mov rax,0x123456789abcdef0",
    "mov rdx,0x9876",
    "mov r8,0x1234",
    "mov r9,0x5678",
    "mov r10,0xabc",
    "mov r11,0xdef",
    "pcmpeqb xmm0,xmm0",
    "pxor xmm5,xmm5",
    "stc",
    "ret",
    ".globl ai_fixture_driver",
    ".seh_proc ai_fixture_driver",
    "ai_fixture_driver:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x60",
    ".seh_stackalloc 0x60",
    ".seh_endprologue",
    "mov rbx,rcx",
    "mov r11,rdx",
    "mov rdx,0x5566",
    "mov r8,0x7788",
    "mov r9,0x99aa",
    "mov r10,0xbbcc",
    "mov qword ptr [rsp+0x20],0x11112222",
    "mov qword ptr [rsp],0x112233",
    "mov qword ptr [rsp+0x18],0x445566",
    "mov qword ptr [rsp+0x58],0x33334444",
    "pcmpeqb xmm0,xmm0",
    "pcmpeqb xmm5,xmm5",
    "stc",
    "call r11",
    ".globl ai_fixture_return",
    "ai_fixture_return:",
    "mov [rbx+0x100],rax",
    "mov [rbx+0x108],rdx",
    "mov [rbx+0x110],r8",
    "mov [rbx+0x118],r9",
    "mov [rbx+0x120],r10",
    "mov [rbx+0x128],r11",
    "movdqu [rbx+0x130],xmm0",
    "movdqu [rbx+0x140],xmm5",
    "pushfq",
    "pop qword ptr [rbx+0x150]",
    "add rsp,0x60",
    "pop rbx",
    "ret",
    ".seh_endproc"
);
unsafe extern "system" {
    pub(super) fn ai_fixture_target();
    pub(super) fn ai_fixture_driver(output: *mut u8, target: usize);
    pub(super) fn ai_fixture_return();
}

core::arch::global_asm!(
    ".globl ai_unwind_driver",
    ".seh_proc ai_unwind_driver",
    "ai_unwind_driver:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x80",
    ".seh_stackalloc 0x80",
    ".seh_endprologue",
    "mov rbx,rcx",
    "mov [rcx],rsp",
    "lea rax,[rip+ai_unwind_landed]",
    "mov [rcx+8],rax",
    "call rdx",
    ".globl ai_unwind_return",
    "ai_unwind_return:",
    "mov eax,0",
    "jmp 2f",
    ".globl ai_unwind_landed",
    "ai_unwind_landed:",
    "mov eax,1",
    "2:",
    "add rsp,0x80",
    "pop rbx",
    "ret",
    ".seh_endproc"
);
unsafe extern "system" {
    pub(super) fn ai_unwind_driver(state: *mut u64, target: usize) -> u64;
    pub(super) fn ai_unwind_return();
}
// Windows non-local unwind must not skip a Rust extern-system activation.
// Use an explicit Win64 frame (including CONTEXT storage) for this fixture.
core::arch::global_asm!(
    ".globl original_unwinds", ".seh_proc original_unwinds", "original_unwinds:",
    "sub rsp,0x518", ".seh_stackalloc 0x518", ".seh_endprologue",
    "mov [rsp+0x30],rcx","call {hold}","mov rcx,[rsp+0x30]",
    "mov rdx,[rcx+8]", "mov rcx,[rcx]", "lea r10,[rsp+0x40]", "mov r11d,0x9a",
    "1:","mov qword ptr [r10],0","add r10,8","dec r11d","jnz 1b",
    "lea r10,[rsp+0x40]","mov [rsp+0x20],r10","mov qword ptr [rsp+0x28],0",
    "xor r8d,r8d","xor r9d,r9d","call {unwind}","add rsp,0x518","ret",".seh_endproc",
    unwind=sym RtlUnwindEx, hold=sym hold_original_unwind
);
static HOLD_UNWIND: AtomicBool = AtomicBool::new(false);
extern "system" fn hold_original_unwind() {
    if HOLD_UNWIND.load(Ordering::Acquire) {
        ORIGINAL_CALL_COUNT.fetch_add(1, Ordering::AcqRel);
        ORIGINAL_ENTERED.store(true, Ordering::Release);
        while !ORIGINAL_RELEASE.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
    }
}
unsafe extern "system-unwind" {
    pub(super) fn original_unwinds(state: *const u64);
}

pub(super) fn child(test: &str, marker: &str) -> bool {
    if std::env::var_os(marker).is_some() {
        return true;
    }
    use std::os::windows::process::CommandExt;
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    let mut child = command
        .creation_flags(0x08000000)
        .env(marker, "1")
        .args(["--exact", test, "--nocapture"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("fixture deadline")
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

#[test]
fn seh_actual_unwind_cleans_thread_capture_and_preserves_outer_return() {
    if !child(
        "ai_operation::gateway::tests::seh_actual_unwind_cleans_thread_capture_and_preserves_outer_return",
        "CIV6_AI_UNWIND_CHILD",
    ) {
        return;
    }
    AI_GATE_TARGETS[0].store(original_unwinds as *const () as u64, Ordering::Release);
    let mut arena = vec![0u8; 0x400];
    let receiver = arena.as_ptr() as u64;
    let player = receiver + 0x200;
    arena[0x98..0xa0].copy_from_slice(&player.to_le_bytes());
    arena[0x2d8..0x2dc].copy_from_slice(&3u32.to_le_bytes());
    let mut created = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exit = created;
    let mut kernel = created;
    let mut user = created;
    assert_ne!(
        unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        },
        0
    );
    let context = Context {
        epoch: Epoch {
            pid: unsafe { GetCurrentProcessId() },
            created: u64::from(created.dwHighDateTime) << 32 | u64::from(created.dwLowDateTime),
            generation: 1,
        },
        thread: unsafe { GetCurrentThreadId() },
        module_base: 0x180000000,
    };
    let raw = civ6_native_bridge_rs::PromotionObservation {
        sequence: 1,
        thread_id: context.thread,
        rcx: receiver,
        rdx: 2,
        r8: 0,
        r9: 0,
        return_address: context.module_base + 0x401537,
    };
    let capture = Capture::begin(
        context,
        context.module_base + 0x4a7ad0,
        raw,
        PaymentDefinition {
            governor_index: 8,
            cost: 2,
            base: 3,
            titles: [Some(4), None],
        },
        4,
        &Memory,
    )
    .unwrap();
    CAPTURE.with(|c| *c.borrow_mut() = Some(capture));
    assert!(CAPTURE.with(|c| c.borrow().is_some()));
    DEPTH.with(|d| d.set(3));
    INVOCATION.with(|c| {
        c.set(Some(Invocation {
            stamp: LeaseStamp {
                pid: 1,
                created: 1,
                owner: 1,
                generation: 1,
                session: 1,
                begin_ticket: 1,
                arm_token: 1,
            },
            player: 1,
            receiver: 1,
            thread: 1,
        }))
    });
    let mut state = [0u64; 2];
    let result = unsafe { ai_unwind_driver(state.as_mut_ptr(), gateways()[0]) };
    assert_eq!(result, 1);
    assert_eq!(UNWOUND.load(Ordering::Acquire), 1);
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 0);
    assert_eq!(THREAD_FLIGHTS.with(Cell::get), 0);
    assert!(drain());
    assert_eq!(DEPTH.with(Cell::get), 0);
    assert!(INVOCATION.with(Cell::get).is_none());
    assert!(CAPTURE.with(|c| c.borrow().is_none()));
}

#[test]
fn gateway_child_preserves_original_once_registers_flags_and_true_stack() {
    const ENV: &str = "CIV6_AI_GATEWAY_FIXTURE_CHILD";
    if std::env::var_os(ENV).is_none() {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        use std::os::windows::process::CommandExt;
        let output=command.creation_flags(0x08000000).env(ENV,"1")
            .args(["--exact","ai_operation::gateway::tests::gateway_child_preserves_original_once_registers_flags_and_true_stack","--nocapture"])
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    for target in &AI_GATE_TARGETS {
        target.store(ai_fixture_target as *const () as u64, Ordering::Release)
    }
    let mut baseline = [0u8; 0x160];
    unsafe {
        ai_fixture_driver(
            baseline.as_mut_ptr(),
            ai_fixture_target as *const () as usize,
        )
    };
    for gateway in gateways() {
        let mut actual = [0u8; 0x160];
        unsafe { ai_fixture_driver(actual.as_mut_ptr(), gateway) };
        assert_eq!(actual, baseline, "raw registers/flags/stack differ");
        assert_eq!(u64::from_le_bytes(actual[88..96].try_into().unwrap()), 1);
        assert_eq!(
            u64::from_le_bytes(actual[64..72].try_into().unwrap()),
            0x11112222
        );
        assert_eq!(
            u64::from_le_bytes(actual[72..80].try_into().unwrap()),
            0x33334444
        );
        let mut base = 0;
        let entry = unsafe { RtlLookupFunctionEntry(gateway as u64, &mut base, ptr::null_mut()) };
        assert!(!entry.is_null(), "gateway needs actual registered pdata");
        let unwind = base + unsafe { (*entry).Anonymous.UnwindData as u64 };
        assert_eq!(Memory.copy::<1>(unwind).unwrap()[0] & 7, 1);
        let mut stack = [0u64; 64];
        stack[0x1b8 / 8] = 0x12345678;
        let mut context: CONTEXT = unsafe { std::mem::zeroed() };
        context.Rsp = stack.as_ptr() as u64;
        context.Rip = gateway as u64 + 8;
        let mut handler = ptr::null_mut();
        let mut establisher = 0;
        unsafe {
            RtlVirtualUnwind(
                0,
                base,
                context.Rip,
                entry,
                &mut context,
                &mut handler,
                &mut establisher,
                ptr::null_mut(),
            );
        }
        assert_eq!(context.Rip, 0x12345678);
        assert_eq!(context.Rsp, stack.as_ptr() as u64 + 0x1c0);
        // Use the actual loaded pre-RET instruction, not a fabricated RVA.
        let length = (base + unsafe { (*entry).EndAddress as u64 } - gateway as u64) as usize;
        let code = unsafe { std::slice::from_raw_parts(gateway as *const u8, length) };
        let epilogue = code
            .windows(9)
            .position(|w| w == [0x48, 0x8d, 0xa4, 0x24, 0xb8, 0x01, 0x00, 0x00, 0xc3])
            .expect("actual LEA/RET epilogue");
        stack[0] = 0x87654321;
        context.Rsp = stack.as_ptr() as u64;
        context.Rip = gateway as u64 + epilogue as u64 + 8;
        unsafe {
            RtlVirtualUnwind(
                0,
                base,
                context.Rip,
                entry,
                &mut context,
                &mut handler,
                &mut establisher,
                ptr::null_mut(),
            );
        }
        assert_eq!(context.Rip, 0x87654321);
        assert_eq!(context.Rsp, stack.as_ptr() as u64 + 8);
    }
}

#[test]
fn rel32_signed_boundaries_and_unaligned_sites_reject() {
    let call = 0x1_8000_0003usize;
    assert_eq!(
        operand(call, call + 5 + i32::MAX as usize),
        Ok(i32::MAX as u32)
    );
    assert_eq!(operand(call, call + 5 - 0x80000000), Ok(0x80000000));
    assert_eq!(operand(call, call + 5 + 0x80000000), Err(Failure::Range));
    assert_eq!(operand(call, call + 5 - 0x80000001), Err(Failure::Range));
    for site in [0x4a7ef0, 0x4a7f18, 0x30ec24] {
        assert_eq!(operand(site, 0x1000), Err(Failure::Alignment));
    }
    assert_eq!(operand(usize::MAX, 0), Err(Failure::Range));
}

#[test]
fn true_caller_span_is_bounded_and_unreadable_frame_has_no_copy() {
    #[repr(C)]
    struct Frame {
        out: [u8; 0x60],
        raw: Registers,
        saved: [u8; 0x60],
    }
    let mut source = [0u8; 0x68];
    for (i, b) in source.iter_mut().enumerate() {
        *b = i as u8;
    }
    let mut frame = Frame {
        out: [0xcc; 0x60],
        saved: [0; 0x60],
        raw: Registers {
            caller_rsp: source.as_ptr() as u64,
            ..Registers::default()
        },
    };
    observe(&mut frame.raw, 0);
    assert_eq!(frame.out, [0xcc; 0x60]);
    assert_eq!(&frame.saved, &source[8..0x68]);
    frame.out.fill(0xaa);
    frame.raw.caller_rsp = u64::MAX - 3;
    frame.raw.ticket = 10;
    observe(&mut frame.raw, 0);
    assert_eq!(frame.raw.ticket, 0);
    assert_eq!(frame.out, [0xaa; 0x60]);
}

struct Reject;
static ORIGINAL_ENTERED: AtomicBool = AtomicBool::new(false);
static ORIGINAL_RELEASE: AtomicBool = AtomicBool::new(false);
static ORIGINAL_CALL_COUNT: AtomicU32 = AtomicU32::new(0);
static ORIGINAL_SELF_DRAIN: AtomicBool = AtomicBool::new(true);
unsafe extern "system" fn original_waits(_: u64, _: u64, _: u64, _: u64) -> u64 {
    ORIGINAL_CALL_COUNT.fetch_add(1, Ordering::AcqRel);
    ORIGINAL_SELF_DRAIN.store(drain(), Ordering::Release);
    ORIGINAL_ENTERED.store(true, Ordering::Release);
    while !ORIGINAL_RELEASE.load(Ordering::Acquire) {
        std::thread::yield_now();
    }
    0x1234
}

#[test]
fn close_drain_waits_for_entire_original_callback_not_only_added_action() {
    if !child(
        "ai_operation::gateway::tests::close_drain_waits_for_entire_original_callback_not_only_added_action",
        "JAC_AI_WHOLE_CALLBACK_CHILD",
    ) {
        return;
    }
    AI_GATE_TARGETS[0].store(original_waits as *const () as u64, Ordering::Release);
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(|| {
        let mut output = [0u8; 0x160];
        unsafe {
            ai_fixture_driver(output.as_mut_ptr(), gateways()[0]);
        }
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !ORIGINAL_ENTERED.load(Ordering::Acquire) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 1);
    assert!(!ORIGINAL_SELF_DRAIN.load(Ordering::Acquire));
    let closer = std::thread::spawn(move || tx.send(close_registered()).unwrap());
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(30))
            .is_err()
    );
    ORIGINAL_RELEASE.store(true, Ordering::Release);
    worker.join().unwrap();
    closer.join().unwrap();
    assert_eq!(rx.recv().unwrap(), Ok(()));
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 0);
    assert_eq!(ORIGINAL_CALL_COUNT.load(Ordering::Acquire), 1);
}

#[test]
fn close_registered_waits_for_original_seh_cleanup() {
    if !child(
        "ai_operation::gateway::tests::close_registered_waits_for_original_seh_cleanup",
        "JAC_AI_CLOSE_SEH_CHILD",
    ) {
        return;
    }
    HOLD_UNWIND.store(true, Ordering::Release);
    AI_GATE_TARGETS[0].store(original_unwinds as *const () as u64, Ordering::Release);
    let worker = std::thread::spawn(|| {
        let mut state = [0u64; 2];
        assert_eq!(
            unsafe { ai_unwind_driver(state.as_mut_ptr(), gateways()[0]) },
            1
        );
        assert_eq!(THREAD_FLIGHTS.with(Cell::get), 0);
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !ORIGINAL_ENTERED.load(Ordering::Acquire) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 1);
    let (tx, rx) = std::sync::mpsc::channel();
    let closer = std::thread::spawn(move || tx.send(close_registered()).unwrap());
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(30))
            .is_err()
    );
    ORIGINAL_RELEASE.store(true, Ordering::Release);
    worker.join().unwrap();
    closer.join().unwrap();
    assert_eq!(rx.recv().unwrap(), Ok(()));
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 0);
    assert_eq!(UNWOUND.load(Ordering::Acquire), 1);
    assert_eq!(ORIGINAL_CALL_COUNT.load(Ordering::Acquire), 1);
}

core::arch::global_asm!(
    ".globl accepted_stage_driver",
    ".seh_proc accepted_stage_driver",
    "accepted_stage_driver:",
    "sub rsp,0x68",
    ".seh_stackalloc 0x68",
    ".seh_endprologue",
    "mov r11,rdx",
    "mov edx,r8d",
    "xor r8d,r8d",
    "call r11",
    ".globl accepted_stage_return",
    "accepted_stage_return:",
    "add rsp,0x68",
    "ret",
    ".seh_endproc"
);
unsafe extern "system" {
    fn accepted_stage_driver(receiver: u64, target: usize, phase: u32) -> u64;
    fn accepted_stage_return();
}
static FIXTURE_EPOCH: AtomicU32 = AtomicU32::new(1);
static FIXTURE_CALLS: AtomicU32 = AtomicU32::new(0);
static FIXTURE_ARMED: AtomicU32 = AtomicU32::new(0);
static FIXTURE_ACTION: AtomicU32 = AtomicU32::new(0);
static FIXTURE_ORIGINAL_PHASE: AtomicU32 = AtomicU32::new(u32::MAX);
struct DynamicLease {
    stamp: LeaseStamp,
    player: u64,
    thread: u32,
}
unsafe impl RuntimeLease for DynamicLease {
    fn begin(&self, owner: u32, player: u64) -> Option<LeaseStamp> {
        (owner == self.stamp.owner
            && player == self.player
            && unsafe { GetCurrentThreadId() } == self.thread)
            .then_some(LeaseStamp {
                generation: FIXTURE_EPOCH.load(Ordering::SeqCst),
                ..self.stamp
            })
    }
    fn current(&self, s: LeaseStamp, player: u64) -> bool {
        s == self.stamp
            && player == self.player
            && FIXTURE_EPOCH.load(Ordering::SeqCst) == s.generation
            && unsafe { GetCurrentThreadId() } == self.thread
    }
}
unsafe extern "system" fn accepted_original(receiver: u64, phase: u32) -> u64 {
    FIXTURE_ORIGINAL_PHASE.store(phase, Ordering::SeqCst);
    FIXTURE_CALLS.fetch_add(1, Ordering::SeqCst);
    if CAPTURE.with(|c| c.borrow().is_some()) {
        FIXTURE_ARMED.fetch_add(1, Ordering::SeqCst);
    }
    match FIXTURE_ACTION.swap(0, Ordering::SeqCst) {
        1 => unsafe {
            accepted_stage_driver(receiver, gateways()[0], 2);
        },
        2 => {
            FIXTURE_EPOCH.fetch_add(1, Ordering::SeqCst);
        }
        3 => {
            ORIGINAL_ENTERED.store(true, Ordering::Release);
            while !ORIGINAL_RELEASE.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
        }
        _ => {}
    }
    0x1234
}

fn accepted_fixture() -> (Vec<u8>, u64) {
    let mut memory = vec![0u8; 0x600];
    let receiver = memory.as_ptr() as u64;
    let player = receiver + 0x300;
    memory[0x98..0xa0].copy_from_slice(&player.to_le_bytes());
    memory[0x3d8..0x3dc].copy_from_slice(&3u32.to_le_bytes());
    let mut c = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut e = c;
    let mut k = c;
    let mut u = c;
    assert_ne!(
        unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) },
        0
    );
    let stamp = LeaseStamp {
        pid: unsafe { GetCurrentProcessId() },
        created: u64::from(c.dwHighDateTime) << 32 | u64::from(c.dwLowDateTime),
        owner: 3,
        generation: 1,
        session: 1,
        begin_ticket: 1,
        arm_token: 7,
    };
    let lease = Box::leak(Box::new(DynamicLease {
        stamp,
        player,
        thread: unsafe { GetCurrentThreadId() },
    }));
    // Explicit in-process layout fixture. This is NOT a parent RuntimeLease implementation.
    let return_address = accepted_stage_return as *const () as u64;
    let base = return_address - 0x401537;
    assert!(
        BINDING
            .set(Binding {
                base,
                callback_return: return_address,
                receiver,
                player,
                owner: 3,
                definition: PaymentDefinition {
                    governor_index: 8,
                    cost: 2,
                    base: 3,
                    titles: [Some(4), None]
                },
                lease,
                installed: stamp,
                promotion: 4
            })
            .is_ok()
    );
    ROWS.lock().unwrap().reserve_exact(CAPACITY);
    ACTIVE.store(true, Ordering::Release);
    AI_GATE_TARGETS[0].store(accepted_original as *const () as u64, Ordering::Release);
    (memory, receiver)
}

#[test]
fn close_waits_for_wrong_phase_original_without_accepted_action() {
    if !child(
        "ai_operation::gateway::tests::close_waits_for_wrong_phase_original_without_accepted_action",
        "JAC_AI_CLOSE_PHASE_CHILD",
    ) {
        return;
    }
    let (_memory, receiver) = accepted_fixture();
    let binding = BINDING.get().unwrap();
    assert!(binding.lease.begin(binding.owner, binding.player).is_some());
    FIXTURE_ACTION.store(3, Ordering::Release);
    let (tx, rx) = std::sync::mpsc::channel();
    let closer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !ORIGINAL_ENTERED.load(Ordering::Acquire) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        tx.send(close_registered()).unwrap();
    });
    let release = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while ACTIVE.load(Ordering::Acquire) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 1);
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(30))
                .is_err()
        );
        ORIGINAL_RELEASE.store(true, Ordering::Release);
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(),
            Err(Failure::CallerNotQuiescent)
        );
    });
    assert_eq!(
        unsafe { accepted_stage_driver(receiver, gateways()[0], 1) },
        0x1234
    );
    closer.join().unwrap();
    release.join().unwrap();
    assert_eq!(FIXTURE_CALLS.load(Ordering::Acquire), 1);
    assert_eq!(FIXTURE_ORIGINAL_PHASE.load(Ordering::Acquire), 1);
    assert_eq!(FIXTURE_ARMED.load(Ordering::Acquire), 0);
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 0);
    assert_eq!(THREAD_FLIGHTS.with(Cell::get), 0);
    let rows = observations().0;
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| !row.accepted && !row.capture_entry));
}

#[test]
fn cached_callback_before_entry_prevents_false_close_success() {
    if !child(
        "ai_operation::gateway::tests::cached_callback_before_entry_prevents_false_close_success",
        "JAC_AI_CLOSE_CACHED_CHILD",
    ) {
        return;
    }
    let (_memory, receiver) = accepted_fixture();
    let page = unsafe { VirtualAlloc(ptr::null(), 4096, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) };
    assert!(!page.is_null());
    let slot = unsafe { &*page.cast::<AtomicU64>() };
    let original = accepted_original as *const () as u64;
    slot.store(original, Ordering::Release);
    let guard = INSTALL.lock().unwrap();
    let registration = Registration::prepare_locked(
        vec![Patch {
            address: page as usize,
            old: original,
            new: gateways()[0] as u64,
            width: Width::Wide,
        }],
        &guard,
    )
    .unwrap();
    assert!(RECOVERY.set(registration.clone()).is_ok());
    registration.install_locked(&guard, &SystemApi).unwrap();
    drop(guard);
    // Exact race: the dispatching thread has read the callback slot, but has
    // not entered the gateway. Another OS thread restores that real slot.
    let cached = slot.load(Ordering::Acquire) as usize;
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 0);
    assert_eq!(
        std::thread::spawn(close_registered).join().unwrap(),
        Err(Failure::CallerNotQuiescent)
    );
    assert_eq!(slot.load(Ordering::Acquire), original);
    assert_eq!(FIXTURE_CALLS.load(Ordering::Acquire), 0);
    assert_eq!(
        unsafe { accepted_stage_driver(receiver, cached, 2) },
        0x1234
    );
    assert_eq!(FIXTURE_CALLS.load(Ordering::Acquire), 1);
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 0);
    assert_eq!(FIXTURE_ARMED.load(Ordering::Acquire), 0);
    assert!(observations().0.iter().all(|row| !row.accepted));
    assert_eq!(close_registered(), Err(Failure::CallerNotQuiescent));
    unsafe { VirtualFree(page, 0, MEM_RELEASE) };
}

#[test]
fn accepted_capture_uses_dynamic_owner_generation_and_nested_thread_rejection() {
    if !child(
        "ai_operation::gateway::tests::accepted_capture_uses_dynamic_owner_generation_and_nested_thread_rejection",
        "CIV6_ACCEPTED_GATEWAY_CHILD",
    ) {
        return;
    }
    let (mut memory, receiver) = accepted_fixture();
    assert_eq!(
        unsafe { accepted_stage_driver(receiver, gateways()[0], 2) },
        0x1234
    );
    assert_eq!(FIXTURE_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(FIXTURE_ARMED.load(Ordering::SeqCst), 1);
    let rows = observations().0;
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|r| r.accepted && r.capture_entry && r.owner == Some(3))
    );
    // Correct owner/thread/ARM and original generation, but wrong native phase.
    // Enter through the same machine-code caller so only phase differs.
    let calls_before = FIXTURE_CALLS.load(Ordering::SeqCst);
    let armed_before = FIXTURE_ARMED.load(Ordering::SeqCst);
    assert_eq!(
        unsafe { accepted_stage_driver(receiver, gateways()[0], 1) },
        0x1234
    );
    assert_eq!(FIXTURE_CALLS.load(Ordering::SeqCst), calls_before + 1);
    assert_eq!(FIXTURE_ORIGINAL_PHASE.load(Ordering::SeqCst), 1);
    assert_eq!(FIXTURE_ARMED.load(Ordering::SeqCst), armed_before);
    let rows = observations().0;
    assert_eq!(rows.len(), 4);
    let rejected = &rows[2..4];
    assert_eq!(
        rejected.iter().filter(|r| r.accepted).count(),
        0,
        "wrong phase produces no accepted receipt"
    );
    assert!(
        rejected
            .iter()
            .all(|r| !r.capture_entry && r.owner.is_none())
    );
    assert_eq!(
        rejected[0].raw.rdx, 1,
        "the actual original argument is phase 1"
    );
    assert_eq!(DEPTH.with(Cell::get), 0);
    assert!(INVOCATION.with(Cell::get).is_none());
    assert!(CAPTURE.with(|c| c.borrow().is_none()));
    // Rejection does not strand thread-local context or prevent a valid call.
    assert_eq!(
        unsafe { accepted_stage_driver(receiver, gateways()[0], 2) },
        0x1234
    );
    assert_eq!(FIXTURE_CALLS.load(Ordering::SeqCst), calls_before + 2);
    assert_eq!(FIXTURE_ORIGINAL_PHASE.load(Ordering::SeqCst), 2);
    assert_eq!(FIXTURE_ARMED.load(Ordering::SeqCst), armed_before + 1);
    let rows = observations().0;
    assert_eq!(rows.len(), 6);
    assert!(
        rows[4..6]
            .iter()
            .all(|r| r.accepted && r.capture_entry && r.owner == Some(3))
    );
    assert_eq!(DEPTH.with(Cell::get), 0);
    assert!(INVOCATION.with(Cell::get).is_none());
    assert!(CAPTURE.with(|c| c.borrow().is_none()));
    memory[0x3d8..0x3dc].copy_from_slice(&4u32.to_le_bytes());
    unsafe {
        accepted_stage_driver(receiver, gateways()[0], 2);
    }
    let rows = observations().0;
    assert!(!rows[6].accepted && !rows[7].accepted);
    memory[0x3d8..0x3dc].copy_from_slice(&3u32.to_le_bytes());
    FIXTURE_ACTION.store(1, Ordering::SeqCst);
    unsafe {
        accepted_stage_driver(receiver, gateways()[0], 2);
    }
    let rows = observations().0;
    assert_eq!(FIXTURE_CALLS.load(Ordering::SeqCst), 6);
    assert!(!rows[9].accepted && !rows[10].accepted);
    assert!(rows[8].accepted && rows[11].accepted);
    std::thread::spawn(move || unsafe { accepted_stage_driver(receiver, gateways()[0], 2) })
        .join()
        .unwrap();
    let rows = observations().0;
    assert!(!rows[12].accepted && !rows[13].accepted);
    FIXTURE_ACTION.store(2, Ordering::SeqCst);
    unsafe {
        accepted_stage_driver(receiver, gateways()[0], 2);
    }
    let rows = observations().0;
    assert!(rows[14].accepted);
    assert!(!rows[15].accepted);
    unsafe {
        accepted_stage_driver(receiver, gateways()[0], 2);
    }
    let rows = observations().0;
    assert!(!rows[16].accepted && !rows[17].accepted);
    assert_eq!(FIXTURE_CALLS.load(Ordering::SeqCst), 9);
    assert_eq!(rows.len(), 18);
    let entries: Vec<_> = rows
        .iter()
        .filter(|r| !r.after)
        .map(|r| r.sequence)
        .collect();
    assert_eq!(entries, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 0);
    assert_eq!(THREAD_FLIGHTS.with(Cell::get), 0);
    assert_eq!(close_registered(), Err(Failure::CallerNotQuiescent));
    // A callable already read by the engine still reaches the immutable
    // original once after restoration/closure, without accepted authority.
    let before_late = FIXTURE_CALLS.load(Ordering::SeqCst);
    unsafe { accepted_stage_driver(receiver, gateways()[0], 2) };
    assert_eq!(FIXTURE_CALLS.load(Ordering::SeqCst), before_late + 1);
    let rows = observations().0;
    assert!(rows[18..20].iter().all(|row| !row.accepted));
    assert_eq!(close_registered(), Err(Failure::CallerNotQuiescent));
}
#[test]
fn registration_fence_rejects_other_identity_load_arm_and_old_ticket() {
    let original = LeaseStamp {
        pid: 10,
        created: 20,
        owner: 3,
        generation: 4,
        session: 5,
        begin_ticket: 6,
        arm_token: 7,
    };
    assert!(same_registration(
        LeaseStamp {
            begin_ticket: 8,
            ..original
        },
        original
    ));
    for field in 0..7 {
        let mut changed = original;
        match field {
            0 => changed.pid += 1,
            1 => changed.created += 1,
            2 => changed.owner += 1,
            3 => changed.generation += 1,
            4 => changed.session += 1,
            5 => changed.begin_ticket -= 1,
            _ => changed.arm_token += 1,
        };
        assert!(!same_registration(changed, original), "field {field}");
    }
}

#[test]
fn resident_sampling_stop_tailcalls_all_originals_with_exact_abi_and_no_context_reads() {
    if !child("ai_operation::gateway::tests::resident_sampling_stop_tailcalls_all_originals_with_exact_abi_and_no_context_reads",
        "JAC_AI_RESIDENT_STOP_CHILD") {return;}
    let (memory,_receiver)=accepted_fixture();
    stop_observation();
    drop(memory); // The old Player is no longer a readable owned context.
    let sequence=NEXT.load(Ordering::Acquire);
    for target in &AI_GATE_TARGETS {target.store(ai_fixture_target as *const () as u64,Ordering::Release);}
    let mut baseline=[0u8;0x160];
    unsafe {ai_fixture_driver(baseline.as_mut_ptr(),ai_fixture_target as *const () as usize)};
    for gateway in gateways().into_iter().take(4) {
        let mut actual=[0u8;0x160];
        unsafe {ai_fixture_driver(actual.as_mut_ptr(),gateway)};
        assert_eq!(actual,baseline,"stopped original ABI");
        assert_eq!(u64::from_le_bytes(actual[88..96].try_into().unwrap()),1);
    }
    assert_eq!(NEXT.load(Ordering::Acquire),sequence,"Rust observer must not be entered");
    assert!(observations().0.is_empty());
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire),0);
    assert_eq!(THREAD_FLIGHTS.with(Cell::get),0);
    assert!(INVOCATION.with(Cell::get).is_none());
    assert_eq!(close_registered(),Err(Failure::CallerNotQuiescent));
    // With the resident gateway tail-called away, the engine's original SEH
    // unwinds directly to its real caller, without touching an old lease.
    AI_GATE_TARGETS[0].store(original_unwinds as *const () as u64,Ordering::Release);
    let mut state=[0u64;2];
    assert_eq!(unsafe {ai_unwind_driver(state.as_mut_ptr(),gateways()[0])},1);
    assert_eq!(UNWOUND.load(Ordering::Acquire),0);
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire),0);
}

#[test]
fn ai_diagnostic_sampling_stop_during_original_preserves_once_and_does_not_certify_drain() {
    if !child("ai_operation::gateway::tests::ai_diagnostic_sampling_stop_during_original_preserves_once_and_does_not_certify_drain",
        "JAC_AI_SAMPLE_STOP_CHILD") {return;}
    let (_memory,receiver)=accepted_fixture();
    FIXTURE_ACTION.store(3,Ordering::Release);
    let stopper=std::thread::spawn(|| {
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(2);
        while !ORIGINAL_ENTERED.load(Ordering::Acquire) {
            assert!(std::time::Instant::now()<deadline);
            std::thread::yield_now();
        }
        stop_observation();
        assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire),1);
        let count=observations().0.len();
        ORIGINAL_RELEASE.store(true,Ordering::Release);
        count
    });
    assert_eq!(unsafe {accepted_stage_driver(receiver,gateways()[0],2)},0x1234);
    let count=stopper.join().unwrap();
    assert_eq!(count,1);
    assert_eq!(observations().0.len(),count);
    assert_eq!(FIXTURE_CALLS.load(Ordering::Acquire),1);
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire),0);
    assert_eq!(unsafe {accepted_stage_driver(receiver,gateways()[0],2)},0x1234);
    assert_eq!(FIXTURE_CALLS.load(Ordering::Acquire),2);
    assert_eq!(observations().0.len(),count);
    assert_eq!(close_registered(),Err(Failure::CallerNotQuiescent));
}
unsafe impl RuntimeLease for Reject {
    fn begin(&self, _: u32, _: u64) -> Option<LeaseStamp> {
        None
    }
    fn current(&self, _: LeaseStamp, _: u64) -> bool {
        false
    }
}
#[test]
fn missing_address_never_installs_or_invokes() {
    static REJECT: Reject = Reject;
    let d = PaymentDefinition {
        governor_index: 8,
        cost: 2,
        base: 3,
        titles: [Some(4), None],
    };
    assert!(matches!(
        unsafe { register(0, 0, 0, d, 4, &REJECT) },
        Err(Failure::Identity)
    ));
    assert!(!ACTIVE.load(Ordering::Acquire));
}
