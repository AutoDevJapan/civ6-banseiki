use super::*;
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlLookupFunctionEntry, CONTEXT, EXCEPTION_RECORD, IMAGE_RUNTIME_FUNCTION_ENTRY,
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
    fn ai_fixture_target();
    fn ai_fixture_driver(output: *mut u8, target: usize);
    fn ai_fixture_return();
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
    fn ai_unwind_driver(state: *mut u64, target: usize) -> u64;
}
// Windows non-local unwind must not skip a Rust extern-system activation.
// Use an explicit Win64 frame (including CONTEXT storage) for this fixture.
core::arch::global_asm!(
    ".globl original_unwinds", ".seh_proc original_unwinds", "original_unwinds:",
    "sub rsp,0x518", ".seh_stackalloc 0x518", ".seh_endprologue",
    "mov rdx,[rcx+8]", "mov rcx,[rcx]", "lea r10,[rsp+0x40]", "mov r11d,0x9a",
    "1:","mov qword ptr [r10],0","add r10,8","dec r11d","jnz 1b",
    "lea r10,[rsp+0x40]","mov [rsp+0x20],r10","mov qword ptr [rsp+0x28],0",
    "xor r8d,r8d","xor r9d,r9d","call {unwind}","add rsp,0x518","ret",".seh_endproc",
    unwind=sym RtlUnwindEx
);
unsafe extern "system" {
    fn original_unwinds();
}

fn child(test: &str, marker: &str) -> bool {
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
        "gateway::tests::seh_actual_unwind_cleans_thread_capture_and_preserves_outer_return",
        "CIV6_AI_UNWIND_CHILD",
    ) {
        return;
    }
    AI_GATE_TARGETS[0].store(original_unwinds as *const () as u64, Ordering::Release);
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
            .args(["--exact","gateway::tests::gateway_child_preserves_original_once_registers_flags_and_true_stack","--nocapture"])
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

#[test]
fn atomic_registration_pointer_cas_and_restoration_are_exact() {
    let slot = Box::new(AtomicU64::new(0x1234));
    let patch = Patch {
        address: (&*slot as *const AtomicU64) as usize,
        old: 0x1234,
        new: 0x5678,
        wide: true,
    };
    unsafe { exchange(&patch, false) }.unwrap();
    assert_eq!(slot.load(Ordering::Acquire), 0x5678);
    assert_eq!(unsafe { exchange(&patch, false) }, Err(Failure::Patch));
    unsafe { exchange(&patch, true) }.unwrap();
    assert_eq!(slot.load(Ordering::Acquire), 0x1234);
}

struct Reject;
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
