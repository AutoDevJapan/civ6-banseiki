use super::*;
use windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry;

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
    "mov qword ptr [rsp+0x58],0x33334444",
    "pcmpeqb xmm0,xmm0",
    "pcmpeqb xmm5,xmm5",
    "stc",
    "call r11",
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

struct Reject;
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
        unsafe { register(0, 0, 0, d, &REJECT) },
        Err(Failure::Identity)
    ));
    assert!(!ACTIVE.load(Ordering::Acquire));
}
