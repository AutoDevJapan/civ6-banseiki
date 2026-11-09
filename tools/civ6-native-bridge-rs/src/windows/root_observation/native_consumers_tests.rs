use super::*;
use crate::native_flight::Error;
use std::os::windows::process::CommandExt;
use std::sync::atomic::AtomicUsize;
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlLookupFunctionEntry, IMAGE_RUNTIME_FUNCTION_ENTRY,
};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn RtlVirtualUnwind(
        kind: u32,
        base: u64,
        pc: u64,
        entry: *const IMAGE_RUNTIME_FUNCTION_ENTRY,
        context: *mut CONTEXT,
        data: *mut *mut c_void,
        frame: *mut u64,
        pointers: *mut c_void,
    ) -> usize;
    fn RtlUnwindEx(
        frame: *const c_void,
        ip: *const c_void,
        record: *const EXCEPTION_RECORD,
        value: *const c_void,
        context: *const CONTEXT,
        history: *const c_void,
    );
}
fn child(name: &str) -> bool {
    const ENV: &str = "CIV6_NATIVE_CONSUMER_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    let mut p = std::process::Command::new(std::env::current_exe().unwrap())
        .creation_flags(0x08000000)
        .env(ENV, "1")
        .args([
            "--exact",
            &format!("windows::root_observation::native_frames::consumers::tests::{name}"),
            "--nocapture",
        ])
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = p.try_wait().unwrap() {
            assert!(status.success(), "owned consumer child {status}");
            return false;
        }
        if Instant::now() >= deadline {
            let _ = p.kill();
            let _ = p.wait();
            panic!("consumer fixture timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(2)
}
static BODY_HITS: AtomicUsize = AtomicUsize::new(0);
static INNER_DONE: AtomicBool = AtomicBool::new(false);
static NEXT_TASK: AtomicU64 = AtomicU64::new(0);
extern "system" fn body(task: u64, stage: u64) -> u64 {
    assert_eq!(unsafe { GetLastError() }, 0x1234);
    BODY_HITS.fetch_add(1, Ordering::AcqRel);
    let f = FRAMES.get().unwrap();
    let scope = CONSUMERS.with(|s| s.borrow().last().unwrap().hold);
    assert!(scope.is_some());
    if stage == 0 {
        let id = f.execute(task, 0x1100).unwrap();
        f.completed(id).unwrap();
        assert_eq!(f.seal_and_wait(deadline()), Err(Error::Reentrant));
        assert_eq!(f.reopen(), Err(Error::Busy));
        INNER_DONE.store(true, Ordering::Release);
    } else if stage == 1 {
        let later = NEXT_TASK.load(Ordering::Acquire);
        assert!(attach_current(later));
        let id = f.execute(later, 0x2200).unwrap();
        f.completed(id).unwrap();
        assert_eq!(f.seal_and_wait(deadline()), Err(Error::Reentrant));
    } else {
        assert!(attach_current(task));
        let receipt = f.retire_task(task).unwrap().unwrap();
        // Original T destructor has returned; only this scheduler is live.
        f.task_retired(receipt).unwrap();
        assert_eq!(f.seal_and_wait(deadline()), Err(Error::Reentrant));
        assert_eq!(f.reopen(), Err(Error::Busy));
    }
    unsafe { SetLastError(0x880) };
    0x55667788
}
global_asm!(
    ".p2align 4",".globl consumer_fixture_body",".seh_proc consumer_fixture_body","consumer_fixture_body:",
    "mov [rsp+0x10],rbx",".seh_savereg rbx,0x10",
    "mov [rsp+0x18],rsi",".seh_savereg rsi,0x18",
    "mov [rsp+0x20],rdi",".seh_savereg rdi,0x20",
    "push rbp",".seh_pushreg rbp","push r12",".seh_pushreg r12",
    "push r13",".seh_pushreg r13","push r14",".seh_pushreg r14","push r15",".seh_pushreg r15",
    "mov rbp,rsp","sub rsp,0x60",".seh_stackalloc 0x60",".seh_endprologue",
    "mov rcx,r8","call {body}",
    "mov rbx,[rsp+0x98]","mov rsi,[rsp+0xa0]","mov rdi,[rsp+0xa8]",
    "lea rsp,[rsp+0x60]","pop r15","pop r14","pop r13","pop r12","pop rbp","ret",".seh_endproc",
    ".p2align 4",".globl consumer_fixture_wait",".seh_proc consumer_fixture_wait","consumer_fixture_wait:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","push rdi",".seh_pushreg rdi",
    "sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "mov rdi,rcx","mov rcx,[rcx+8]","call {arena}",
    ".globl consumer_fixture_arena_return","consumer_fixture_arena_return:",
    "mov qword ptr [rdi+8],0","mov rax,0x9977",
    "mov rbx,[rsp+0x30]","lea rsp,[rsp+0x20]","pop rdi","ret",".seh_endproc",
    ".p2align 4",".globl consumer_fixture_arena",".seh_proc consumer_fixture_arena","consumer_fixture_arena:",
    ".byte 0x40,0x55",".seh_pushreg rbp","push rbx",".seh_pushreg rbx","push rsi",".seh_pushreg rsi",
    "lea rbp,[rsp-0x47]","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "inc qword ptr [rcx]","lea rsp,[rsp+0x20]","pop rsi","pop rbx","pop rbp","ret",".seh_endproc",
    ".globl consumer_fixture_core",".seh_proc consumer_fixture_core","consumer_fixture_core:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","push rdi",".seh_pushreg rdi",
    "sub rsp,0x30",".seh_stackalloc 0x30",".seh_endprologue",
    "mov rdi,rcx","call {wait}",
    ".globl consumer_fixture_host_return","consumer_fixture_host_return:",
    "mov rbx,[rsp+0x40]","lea rsp,[rsp+0x30]","pop rdi","ret",".seh_endproc",
    ".globl consumer_unwind_driver",".seh_proc consumer_unwind_driver","consumer_unwind_driver:",
    "push rbx",".seh_pushreg rbx","sub rsp,0x80",".seh_stackalloc 0x80",".seh_endprologue",
    "mov rbx,rcx","mov [rcx],rsp","lea rax,[rip+consumer_unwind_landed]","mov [rcx+8],rax",
    "mov rcx,r8","mov r8,[rcx+0x10]","call rdx","xor eax,eax","jmp 2f",
    "consumer_unwind_landed:","mov eax,1","2:","lea rsp,[rsp+0x80]","pop rbx","ret",".seh_endproc",
    ".p2align 4",".globl consumer_unwind_body",".seh_proc consumer_unwind_body","consumer_unwind_body:",
    "mov [rsp+0x10],rbx",".seh_savereg rbx,0x10",
    "sub rsp,0x528",".seh_stackalloc 0x528",".seh_endprologue",
    "mov rdx,[rcx+8]","mov rcx,[rcx]","lea r10,[rsp+0x40]","mov r11d,0x9a",
    "1:","mov qword ptr [r10],0","lea r10,[r10+8]","dec r11d","jnz 1b",
    "lea r10,[rsp+0x40]","mov [rsp+0x20],r10","mov qword ptr [rsp+0x28],0",
    "xor r8d,r8d","xor r9d,r9d","call {unwind_ex}","int3",".seh_endproc",
    body=sym body,arena=sym native_consumer_arena_gateway,wait=sym native_consumer_wait_gateway,
    unwind_ex=sym RtlUnwindEx,
);
unsafe extern "system" {
    fn consumer_fixture_body();
    fn consumer_fixture_wait();
    fn consumer_fixture_arena();
    fn consumer_fixture_core(set: *mut u64) -> u64;
    fn consumer_fixture_arena_return();
    fn consumer_fixture_host_return();
    fn consumer_unwind_driver(state: *mut u64, target: usize, argument: *mut u64) -> u64;
    fn consumer_unwind_body();
}
fn ready() {
    FRAMES.get_or_init(NativeFrames::new);
    NATIVE_CONSUMER_CONTINUE[0].store(
        consumer_fixture_body as *const () as u64 + 5,
        Ordering::Release,
    );
    NATIVE_CONSUMER_CONTINUE[1].store(
        consumer_fixture_body as *const () as u64 + 5,
        Ordering::Release,
    );
    NATIVE_CONSUMER_CONTINUE[2].store(
        consumer_fixture_wait as *const () as u64 + 5,
        Ordering::Release,
    );
    NATIVE_CONSUMER_CONTINUE[3].store(
        consumer_fixture_arena as *const () as u64 + 9,
        Ordering::Release,
    );
    EXE.store(
        consumer_fixture_arena_return as *const () as u64 - 0x986270,
        Ordering::Release,
    );
    CORE.store(
        consumer_fixture_host_return as *const () as u64 - 0x992301,
        Ordering::Release,
    );
}

#[test]
fn actual_gateway_inner_completion_keeps_full_consumer_frame_and_register_return() {
    if !child("actual_gateway_inner_completion_keeps_full_consumer_frame_and_register_return") {
        return;
    }
    ready();
    let f = FRAMES.get().unwrap();
    for (i, gateway) in gateways()[..2].iter().enumerate() {
        let task = 0x4400 + i as u64 * 0x100;
        let id = f.issue_on_set(0x1100, 0x3300).unwrap();
        f.bind_before_publication(id, task).unwrap();
        f.pushed(id, task).unwrap();
        let invoke: unsafe extern "system" fn(u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(*gateway) };
        unsafe { SetLastError(0x1234) };
        assert_eq!(unsafe { invoke(0, 0, task, 0) }, 0x55667788);
        assert_eq!(unsafe { GetLastError() }, 0x880);
        f.seal_and_wait(deadline()).unwrap();
        f.reopen().unwrap();
    }
    assert!(INNER_DONE.load(Ordering::Acquire));
    assert_eq!(BODY_HITS.load(Ordering::Acquire), 2);
    assert!(CONSUMERS.with(|s| s.borrow().is_empty()));
    f.seal_and_wait(deadline()).unwrap();
    f.reopen().unwrap();
}
#[test]
fn unknown_first_task_can_attach_known_later_task_without_new_epoch() {
    if !child("unknown_first_task_can_attach_known_later_task_without_new_epoch") {
        return;
    }
    ready();
    let f = FRAMES.get().unwrap();
    let id = f.issue_on_set(0x2200, 0x3300).unwrap();
    f.bind_before_publication(id, 0x5500).unwrap();
    f.pushed(id, 0x5500).unwrap();
    NEXT_TASK.store(0x5500, Ordering::Release);
    let mut scope: Saved = unsafe { std::mem::zeroed() };
    scope.invocation.kind = 51;
    begin(&mut scope);
    assert!(CONSUMERS.with(|s| s.borrow().last().unwrap().hold.is_none()));
    assert!(attach_current(0x5500));
    assert!(!attach_current(0xdead));
    let job = f.execute(0x5500, 0x2200).unwrap();
    f.completed(job).unwrap();
    assert_eq!(f.seal_and_wait(deadline()), Err(Error::Reentrant));
    finish(&mut scope, false);
    f.seal_and_wait(deadline()).unwrap();
    f.reopen().unwrap();
}
#[test]
fn actual_wait_gateways_correlate_same_set_and_reject_current_arena_receipt() {
    if !child("actual_wait_gateways_correlate_same_set_and_reject_current_arena_receipt") {
        return;
    }
    ready();
    let mut arena = 0u64;
    let mut set = [0u64, &mut arena as *mut u64 as u64];
    TEST_ARENA_CLASS.store(1, Ordering::Release);
    assert_eq!(unsafe { consumer_fixture_core(set.as_mut_ptr()) }, 0x9977);
    assert!(RECEIPT.with(|r| r.borrow().is_none()));
    set[1] = &mut arena as *mut u64 as u64;
    TEST_ARENA_CLASS.store(2, Ordering::Release);
    assert_eq!(unsafe { consumer_fixture_core(set.as_mut_ptr()) }, 0x9977);
    let expected = RECEIPT.with(|r| *r.borrow()).unwrap();
    assert_eq!(expected.set, set.as_ptr() as u64);
    assert!(take_wait_receipt(expected.set + 8, expected.host_rsp + 0x40).is_none());
    set[1] = &mut arena as *mut u64 as u64;
    unsafe { consumer_fixture_core(set.as_mut_ptr()) };
    let expected = RECEIPT.with(|r| *r.borrow()).unwrap();
    assert!(take_wait_receipt(expected.set, expected.host_rsp + 0x40).is_some());
    assert!(take_wait_receipt(expected.set, expected.host_rsp + 0x40).is_none());
    assert_eq!(arena, 3);
}
#[test]
fn actual_terminal_seh_keeps_unresolved_and_never_fabricates_return_or_wait() {
    if !child("actual_terminal_seh_keeps_unresolved_and_never_fabricates_return_or_wait") {
        return;
    }
    ready();
    let mut state = [0u64; 3];
    NATIVE_CONSUMER_CONTINUE[0].store(
        consumer_unwind_body as *const () as u64 + 5,
        Ordering::Release,
    );
    // No T reservation here: unknown scheduler scope still unwinds its TLS.
    assert_eq!(
        unsafe { consumer_unwind_driver(state.as_mut_ptr(), gateways()[0], state.as_mut_ptr()) },
        1
    );
    assert!(CONSUMERS.with(|s| s.borrow().is_empty()));
    assert!(RECEIPT.with(|r| r.borrow().is_none()));
    // Bound task scope must poison, not issue successful completion.
    let f = FRAMES.get().unwrap();
    let id = f.issue_on_set(0x1100, 0x3300).unwrap();
    f.bind_before_publication(id, 0x6600).unwrap();
    f.pushed(id, 0x6600).unwrap();
    state[2] = 0x6600;
    assert_eq!(
        unsafe { consumer_unwind_driver(state.as_mut_ptr(), gateways()[0], state.as_mut_ptr()) },
        1
    );
    assert!(CONSUMERS.with(|s| s.borrow().is_empty()));
    assert!(!f.healthy());
    assert_eq!(f.reopen(), Err(Error::Unresolved));
}
#[test]
fn shim_virtual_unwind_restores_displaced_nonvolatile_and_original_stack() {
    for (address, offset) in [
        (
            native_consumer_first_original as *const () as usize + 5,
            0x10,
        ),
        (
            native_consumer_second_original as *const () as usize + 5,
            0x10,
        ),
        (native_consumer_wait_original as *const () as usize + 5, 8),
    ] {
        let mut stack = [0u64; 32];
        stack[0] = 0x12345678;
        stack[offset / 8] = 0x55667788;
        let mut base = 0;
        let entry = unsafe { RtlLookupFunctionEntry(address as u64, &mut base, null_mut()) };
        assert!(!entry.is_null());
        let mut c: CONTEXT = unsafe { std::mem::zeroed() };
        c.Rip = address as u64;
        c.Rsp = stack.as_mut_ptr() as u64;
        let mut data = null_mut();
        let mut frame = 0;
        unsafe {
            RtlVirtualUnwind(
                0,
                base,
                c.Rip,
                entry,
                &mut c,
                &mut data,
                &mut frame,
                null_mut(),
            )
        };
        assert_eq!(c.Rip, 0x12345678);
        assert_eq!(c.Rbx, 0x55667788);
        assert_eq!(c.Rsp, stack.as_ptr() as u64 + 8);
    }
}
#[test]
fn four_real_atomic_entries_restore_bytes_protection_and_keep_relays() {
    if !child("four_real_atomic_entries_restore_bytes_protection_and_keep_relays") {
        return;
    }
    let Some(_fixture) = crate::windows::atomic_fixture::require(
        "four_real_atomic_entries_restore_bytes_protection_and_keep_relays",
    ) else {
        return;
    };
    ready();
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0xe50000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    for (rva, bytes, _) in SITES {
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), (base + rva) as *mut u8, 8) };
    }
    unsafe {
        *((base + 0xe3eaf8) as *mut u8) = 0xb9;
    }
    let mut old = 0;
    assert_ne!(
        unsafe { VirtualProtect(base as _, 0xe50000, PAGE_EXECUTE_READ, &mut old) },
        0
    );
    unsafe { install_at(base) }.unwrap();
    for (rva, bytes, _) in SITES {
        assert_ne!(copy_current::<8>((base + rva) as u64), Some(bytes));
    }
    unsafe { restore() }.unwrap();
    for (rva, bytes, _) in SITES {
        assert_eq!(copy_current::<8>((base + rva) as u64), Some(bytes));
    }
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
    let _install: unsafe fn(usize) -> Result<(), &'static str> = install;
}

#[test]
fn native_destructor_normal_return_keeps_consumer_hold_until_scheduler_return() {
    if !child("native_destructor_normal_return_keeps_consumer_hold_until_scheduler_return") {
        return;
    }
    ready();
    let f = FRAMES.get().unwrap();
    let id = f.issue_on_set(0x3300, 0x9900).unwrap();
    f.bind_before_publication(id, 0x8800).unwrap();
    f.pushed(id, 0x8800).unwrap();
    let invoke: unsafe extern "system" fn(u64, u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(gateways()[1]) };
    unsafe { SetLastError(0x1234) };
    assert_eq!(unsafe { invoke(0, 2, 0x8800, 0) }, 0x55667788);
    f.seal_and_wait(deadline()).unwrap();
    f.reopen().unwrap();
}

#[test]
fn nested_unknown_consumers_attach_every_active_outer_scope() {
    if !child("nested_unknown_consumers_attach_every_active_outer_scope") {
        return;
    }
    ready();
    let f = FRAMES.get().unwrap();
    let id = f.issue_on_set(0x3300, 0x9900).unwrap();
    f.bind_before_publication(id, 0x8800).unwrap();
    f.pushed(id, 0x8800).unwrap();
    let mut outer: Saved = unsafe { std::mem::zeroed() };
    outer.invocation.kind = 50;
    begin(&mut outer);
    let mut inner: Saved = unsafe { std::mem::zeroed() };
    inner.invocation.kind = 51;
    begin(&mut inner);
    assert!(attach_current(0x8800));
    assert!(CONSUMERS.with(|s| s.borrow().iter().all(|c| c.hold.is_some())));
    let r = f.retire_task(0x8800).unwrap().unwrap();
    f.task_retired(r).unwrap();
    finish(&mut inner, false);
    assert_eq!(f.seal_and_wait(deadline()), Err(Error::Reentrant));
    finish(&mut outer, false);
    f.seal_and_wait(deadline()).unwrap();
}

#[test]
fn runtime_table_identity_classifies_only_verified_virtual30_pairs() {
    if !child("runtime_table_identity_classifies_only_verified_virtual30_pairs") {
        return;
    }
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0x1130000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    EXE.store(base as u64, Ordering::Release);
    let mut receiver = [0u64; 1];
    for (table, target) in [(0x1120678, 0xe42470), (0x11206b0, 0xe427f0)] {
        receiver[0] = (base + table) as u64;
        unsafe {
            *((base + table + 0x30) as *mut u64) = (base + target) as u64;
        }
        assert!(typed_scheduler(receiver.as_ptr() as u64));
        unsafe {
            *((base + table + 0x30) as *mut u64) ^= 8;
        }
        assert!(!typed_scheduler(receiver.as_ptr() as u64));
    }
    receiver[0] = (base + 0x1120618) as u64;
    assert!(!typed_scheduler(receiver.as_ptr() as u64));
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
}

#[test]
fn complete_primary_code_match_rejects_middle_epilogue_and_prefix_mutations() {
    let mut bytes = PROLOGUE.to_vec();
    bytes.extend([0x90, 0x48, 0x83, 0xc4, 0x60, 0xc3]);
    let expected = format!("{:x}", Sha256::digest(&bytes));
    assert!(complete_code_matches(&bytes, &expected));
    for p in 0..bytes.len() {
        bytes[p] ^= 1;
        assert!(!complete_code_matches(&bytes, &expected), "code byte{p}");
        bytes[p] ^= 1;
    }
    assert!(!complete_code_matches(&bytes[..bytes.len() - 1], &expected));
}

#[test]
#[ignore = "explicit primary-image acceptance; never substituted for owned gateway fixtures"]
fn fixed_primary_host_complete_consumers_and_unwind_match_runtime_constants() {
    let mut file = super::super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
    for (rva, len, sha) in [
        (
            0xe417d0,
            0x421,
            "9cfb21b3ac5a6673fd3de84483fd5bc7a250f58c13a02eee87c0e9c802a8d36f",
        ),
        (
            0xe41c00,
            0x3ae,
            "8a383bb62e2a13f1e78fba6e3e06139fb35330a2ae984736944639ae8a527251",
        ),
    ] {
        let off = pe_file_offset(&mut file, rva).unwrap();
        file.seek(SeekFrom::Start(off)).unwrap();
        let mut code = vec![0; len];
        file.read_exact(&mut code).unwrap();
        assert!(complete_code_matches(&code, sha));
        code[len / 2] ^= 1;
        assert!(!complete_code_matches(&code, sha));
    }
    for (slot, target) in [(0x11206a8, 0xe42470u64), (0x11206e0, 0xe427f0)] {
        // The shared helper intentionally maps .text only. Read the real
        // section table for data, rather than borrowing that code-only helper.
        let off = data_file_offset(&mut file, slot).unwrap();
        file.seek(SeekFrom::Start(off)).unwrap();
        let mut raw = [0; 8];
        file.read_exact(&mut raw).unwrap();
        assert_eq!(u64::from_le_bytes(raw), 0x140000000 + target);
    }
    let off = data_file_offset(&mut file, 0x123a8f8).unwrap();
    file.seek(SeekFrom::Start(off)).unwrap();
    let mut raw = [0; 28];
    file.read_exact(&mut raw).unwrap();
    assert_eq!(
        raw,
        [
            0x01, 0x1f, 0x0c, 0, 0x1f, 0x74, 0x15, 0, 0x1f, 0x64, 0x14, 0, 0x1f, 0x34, 0x13, 0,
            0x1f, 0xb2, 0x18, 0xf0, 0x16, 0xe0, 0x14, 0xd0, 0x12, 0xc0, 0x10, 0x50
        ]
    );
}

fn data_file_offset(file: &mut File, rva: u32) -> Option<u64> {
    file.seek(SeekFrom::Start(0x3c)).ok()?;
    let mut raw = [0; 4];
    file.read_exact(&mut raw).ok()?;
    let nt = u32::from_le_bytes(raw) as u64;
    file.seek(SeekFrom::Start(nt + 6)).ok()?;
    let mut n = [0; 2];
    file.read_exact(&mut n).ok()?;
    let count = u16::from_le_bytes(n);
    file.seek(SeekFrom::Start(nt + 20)).ok()?;
    file.read_exact(&mut n).ok()?;
    let table = nt + 24 + u64::from(u16::from_le_bytes(n));
    for i in 0..count {
        file.seek(SeekFrom::Start(table + u64::from(i) * 40)).ok()?;
        let mut s = [0; 40];
        file.read_exact(&mut s).ok()?;
        let start = u32::from_le_bytes(s[12..16].try_into().ok()?);
        let size = u32::from_le_bytes(s[16..20].try_into().ok()?);
        if rva >= start && rva - start < size {
            return Some(
                u64::from(u32::from_le_bytes(s[20..24].try_into().ok()?)) + u64::from(rva - start),
            );
        }
    }
    None
}
