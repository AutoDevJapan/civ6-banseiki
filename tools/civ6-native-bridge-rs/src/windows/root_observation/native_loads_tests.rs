use super::*;
use std::os::windows::process::CommandExt;
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlLookupFunctionEntry, IMAGE_RUNTIME_FUNCTION_ENTRY,
};
static FLOW: AtomicBool = AtomicBool::new(false);
static FLOW_QUEUE: AtomicU64 = AtomicU64::new(0);
static FLOW_PAYLOAD: AtomicU64 = AtomicU64::new(0);
static FLOW_OWNED: AtomicU64 = AtomicU64::new(0);
static FLOW_ENGINE: AtomicU64 = AtomicU64::new(0);
static FLOW_VIEW: AtomicU64 = AtomicU64::new(0);
static FLOW_TASK: AtomicU64 = AtomicU64::new(0);
extern "system" fn flow_copy_body() {
    unsafe {
        *((exe(0x22d8d18)) as *mut u64) = FLOW_OWNED.load(Ordering::Acquire);
    }
}
extern "system" fn flow_allocate_body() -> u64 {
    FLOW_PAYLOAD.load(Ordering::Acquire)
}
extern "system" fn flow_start_body() {
    let view = FLOW_VIEW.load(Ordering::Acquire);
    let task = FLOW_TASK.load(Ordering::Acquire);
    let mut r: Saved = unsafe { std::mem::zeroed() };
    r.invocation.kind = 23;
    r.invocation.caller = exe(0x985904);
    r.invocation.caller_rsp = 0x100;
    job_begin(&mut r);
    r.gpr[4] = task;
    job_finish(&mut r, false);
    assert_eq!(value(view + 0x250), 0, "P+8 publication has not occurred");
    assert_eq!(value(task + 8), 0, "T+8 publication has not occurred");
    assert!(state().tasks_by_address.contains_key(&task));
    unsafe {
        *(task as *mut u64) = exe(0x106c418);
        *((task + 8) as *mut u64) = view + 0x248;
        *((view + 0x250) as *mut u64) = task;
    }
}
extern "system" fn flow_worker_body() {
    let invocation = current_worker().unwrap();
    assert_eq!(state().ledger.request(invocation).unwrap().0, 1);
    world_created(worlds::WorldLease {
        epoch: 1,
        serial: 9,
        context: 10,
        game: 20,
        world: 30,
    });
}
global_asm!(
    ".p2align 4",".globl load_flow_request",".seh_proc load_flow_request","load_flow_request:",
    "mov [rsp+0x20],r9","sub rsp,0x38",".seh_stackalloc 0x38",".seh_endprologue",
    "mov rcx,[rsp+0x60]","mov edx,2","call {copy}",
    ".globl load_flow_copy_return","load_flow_copy_return:","mov rcx,[rip+{queue}]","call {allocate}",
    ".globl load_flow_allocate_return","load_flow_allocate_return:","mov al,1","lea rsp,[rsp+0x38]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_copy",".seh_proc load_flow_copy","load_flow_copy:",
    ".byte 0x40,0x55",".seh_pushreg rbp","push rsi",".seh_pushreg rsi","push rdi",".seh_pushreg rdi","push r15",".seh_pushreg r15",
    "sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {copy_body}",
    "lea rsp,[rsp+0x28]","pop r15","pop rdi","pop rsi","pop rbp","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_allocate",".seh_proc load_flow_allocate","load_flow_allocate:",
    "mov eax,[rcx+0x800100]","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {allocate_body}","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_bank",".seh_proc load_flow_bank","load_flow_bank:",
    ".byte 0x40,0x56",".seh_pushreg rsi","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "mov rdx,r8","add rdx,0xa20","mov rcx,[rip+{payload}]","call {command}","lea rsp,[rsp+0x20]","pop rsi","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_command",".seh_proc load_flow_command","load_flow_command:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "mov rdx,rcx","mov rcx,[rip+{engine}]","call {select}",
    ".globl load_flow_select_return","load_flow_select_return:","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_select","load_flow_select:",
    "mov rax,[rcx+0x14f0]","mov [rcx+0x53f8],rax","ret",
    ".globl load_flow_transition",".seh_proc load_flow_transition","load_flow_transition:",
    "sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "mov rax,[rcx+0x53f8]","mov [rcx+0x5400],rax","mov qword ptr [rcx+0x53f8],0","mov rcx,rax","call {initialize}",
    ".globl load_flow_initialize_return","load_flow_initialize_return:","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_initialize",".seh_proc load_flow_initialize","load_flow_initialize:",
    ".byte 0x40,0x55",".seh_pushreg rbp","push r12",".seh_pushreg r12","push r14",".seh_pushreg r14",
    "sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue","add rcx,0x240","call {start}",
    ".globl load_flow_start_return","load_flow_start_return:","lea rsp,[rsp+0x20]","pop r14","pop r12","pop rbp","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_start",".seh_proc load_flow_start","load_flow_start:",
    "sub rsp,0x38",".seh_stackalloc 0x38","lea rax,[rip+load_flow_start]",".seh_endprologue","call {start_body}","lea rsp,[rsp+0x38]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_worker",".seh_proc load_flow_worker","load_flow_worker:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "call {worker_body}","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".globl load_flow_worker_driver",".seh_proc load_flow_worker_driver","load_flow_worker_driver:",
    "sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {worker}",
    ".globl load_flow_worker_return","load_flow_worker_return:","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    copy=sym native_load_copy_gateway,allocate=sym native_load_allocate_gateway,command=sym native_load_command_gateway,
    select=sym native_load_select_gateway,initialize=sym native_load_initialize_gateway,start=sym native_load_start_gateway,worker=sym native_load_worker_gateway,
    copy_body=sym flow_copy_body,allocate_body=sym flow_allocate_body,start_body=sym flow_start_body,worker_body=sym flow_worker_body,
    queue=sym FLOW_QUEUE,payload=sym FLOW_PAYLOAD,engine=sym FLOW_ENGINE,
);
unsafe extern "system" {
    fn load_flow_copy_return();
    fn load_flow_allocate_return();
    fn load_flow_select_return();
    fn load_flow_initialize_return();
    fn load_flow_start_return();
    fn load_flow_worker_return();
    fn load_flow_request();
    fn load_flow_copy();
    fn load_flow_allocate();
    fn load_flow_bank();
    fn load_flow_command();
    fn load_flow_select();
    fn load_flow_initialize();
    fn load_flow_start();
    fn load_flow_worker();
    fn load_flow_transition(engine: u64);
    fn load_flow_worker_driver(view: u64);
}
pub(super) fn caller_override(rva: u64) -> Option<u64> {
    if !FLOW.load(Ordering::Acquire) {
        return None;
    }
    match rva {
        0x16bd51 => Some(load_flow_copy_return as *const () as u64),
        0x163dea => Some(load_flow_allocate_return as *const () as u64),
        0xdb88d => Some(load_flow_select_return as *const () as u64),
        0x63fcd => Some(load_flow_initialize_return as *const () as u64),
        0x83144 => Some(load_flow_start_return as *const () as u64),
        0x985b63 => Some(load_flow_worker_return as *const () as u64),
        _ => None,
    }
}
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
}
fn child(name: &str) -> bool {
    const ENV: &str = "CIV6_NATIVE_LOAD_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    let mut p = std::process::Command::new(std::env::current_exe().unwrap())
        .creation_flags(0x08000000)
        .env(ENV, "1")
        .args([
            "--exact",
            &format!("windows::root_observation::native_frames::loads::tests::{name}"),
            "--nocapture",
        ])
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(s) = p.try_wait().unwrap() {
            assert!(s.success(), "load child {s}");
            return false;
        }
        if Instant::now() >= deadline {
            let _ = p.kill();
            let _ = p.wait();
            panic!("load child timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn native_request_command_view_and_task_gateway_chain_retains_before_publication_carrier() {
    if !child(
        "native_request_command_view_and_task_gateway_chain_retains_before_publication_carrier",
    ) {
        return;
    }
    let size = 0x2400000;
    let base =
        unsafe { VirtualAlloc(null_mut(), size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) } as u64;
    assert_ne!(base, 0);
    EXE.store(base, Ordering::Release);
    FLOW.store(true, Ordering::Release);
    let queue = exe(0x1ad4d00);
    let payload = queue + 0x90;
    let descriptor = exe(0x22d58a0);
    let mut engine = vec![0u64; 0x5410 / 8];
    let ep = engine.as_mut_ptr() as u64;
    let mut view = vec![0u64; 0x2e0 / 8];
    let vp = view.as_mut_ptr() as u64;
    let mut task = [0u64; 2];
    let tp = task.as_mut_ptr() as u64;
    FLOW_QUEUE.store(queue, Ordering::Release);
    FLOW_PAYLOAD.store(payload, Ordering::Release);
    FLOW_ENGINE.store(ep, Ordering::Release);
    FLOW_VIEW.store(vp, Ordering::Release);
    FLOW_TASK.store(tp, Ordering::Release);
    FLOW_OWNED.store(0x3000, Ordering::Release);
    unsafe {
        *((payload - 0x10) as *mut u32) = 0x280;
        *((payload - 0xc) as *mut u32) = 0x36;
        *((descriptor + 0x20) as *mut u64) = exe(0xef220);
        *((ep + 0x14f0) as *mut u64) = vp;
        *((exe(0x12c3c88)) as *mut u64) = ep;
        *(vp as *mut u64) = exe(0xfad3f0);
        *((vp + 0x240) as *mut u64) = exe(0xfad3e0);
        *((vp + 0x248) as *mut u64) = exe(0x106c390);
        *((vp + 0x270) as *mut u64) = vp + 0x240;
        *((vp + 0x280) as *mut u64) = 0x123456;
        *((vp + 0x290) as *mut u64) = vp;
        *((vp + 0x298) as *mut u64) = exe(0x82da0);
    }
    let original = [
        (1, load_flow_request as *const () as u64),
        (2, load_flow_copy as *const () as u64),
        (3, load_flow_allocate as *const () as u64),
        (4, load_flow_command as *const () as u64),
        (5, load_flow_select as *const () as u64),
        (6, load_flow_initialize as *const () as u64),
        (7, load_flow_worker as *const () as u64),
        (9, load_flow_start as *const () as u64),
        (10, load_flow_bank as *const () as u64),
    ];
    for (i, address) in original {
        NATIVE_LOAD_CONTINUE[i].store(address + SITES[i].2 as u64, Ordering::Release);
    }
    NATIVE_LOAD_ASYNC_LABEL.store(exe(0x106c4c4), Ordering::Release);
    let request: unsafe extern "system" fn(u64, u32, u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(gateways()[1]) };
    assert_eq!(unsafe { request(0, 2, 0, 0, 0x2000) } as u8, 1);
    assert_eq!(
        state().ledger.consuming(payload, queue + 0x400080),
        Err(ledger::Error::Stale)
    );
    let bank: unsafe extern "system" fn(u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(gateways()[10]) };
    unsafe {
        bank(0, queue, descriptor - 0x36 * 0x30);
        load_flow_transition(ep);
    }
    assert_eq!(state().tasks_by_address.len(), 1);
    let mut execution: Saved = unsafe { std::mem::zeroed() };
    execution.invocation.kind = 21;
    execution.invocation.caller_rsp = 0x200;
    execution.gpr[0] = tp;
    job_begin(&mut execution);
    unsafe {
        load_flow_worker_driver(vp);
    }
    job_finish(&mut execution, false);
    assert_eq!(state().worlds.get(&1).unwrap().serial, 9);
    let mut destruction: Saved = unsafe { std::mem::zeroed() };
    destruction.invocation.kind = 22;
    destruction.invocation.caller_rsp = 0x300;
    destruction.gpr[0] = tp;
    job_begin(&mut destruction);
    assert!(state().tasks_by_address.is_empty());
    job_finish(&mut destruction, false);
    assert!(CALLS.with(|s| s.borrow().is_empty()));
    assert!(JOB_CALLS.with(|s| s.borrow().is_empty()));
    assert!(!FAILED.load(Ordering::Acquire));
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
}
#[test]
fn all_eleven_native_load_entry_exchanges_execute_original_once_and_restore() {
    if !child("all_eleven_native_load_entry_exchanges_execute_original_once_and_restore") {
        return;
    }
    assert!(
        supports_atomic_promotion_fetch(),
        "required atomic entry exchange unavailable"
    );
    let size = 0x2400000;
    let base = unsafe { VirtualAlloc(null_mut(), size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
        as usize;
    assert_ne!(base, 0);
    for (i, (rva, expected, stolen)) in SITES.into_iter().enumerate() {
        let p = (base + rva) as *mut u8;
        unsafe {
            ptr::copy_nonoverlapping(expected.as_ptr(), p, 8);
        }
        let mut tail = vec![];
        // Atomically count exactly one original body. RCX is the fixture's
        // valid native receiver; all9 synthetic bodies share this counter.
        tail.extend_from_slice(&[0xf0, 0x48, 0xff, 0x81, 0, 0x60, 0, 0]);
        let result = if i == 1 { 0 } else { 0x1122_3344_5566_7788u64 };
        tail.extend_from_slice(&[0x48, 0xb8]);
        tail.extend_from_slice(&result.to_le_bytes());
        match i {
            2 => tail.extend_from_slice(&[0x41, 0x5f, 0x5f, 0x5e, 0x5d]),
            6 => tail.extend_from_slice(&[0x41, 0x5e, 0x41, 0x5c, 0x5d]),
            9 => tail.extend_from_slice(&[0x48, 0x83, 0xc4, 0x38]),
            10 => tail.extend_from_slice(&[0x48, 0x83, 0xc4, 0x20, 0x5e]),
            _ => {}
        }
        tail.extend_from_slice(&[0xf9, 0xc3]);
        if i == 8 {
            // Both relocated branches must remain inside owned native code.
            unsafe {
                ptr::copy_nonoverlapping(tail.as_ptr(), (base + 0x16d29b) as *mut u8, tail.len());
            }
        }
        unsafe {
            ptr::copy_nonoverlapping(tail.as_ptr(), p.add(stolen), tail.len());
        }
    }
    for (rva, _, _) in SITES {
        let mut old = 0;
        assert_ne!(
            unsafe {
                VirtualProtect(
                    (base + (rva & !0xfff)) as _,
                    0x1000,
                    PAGE_EXECUTE_READ,
                    &mut old,
                )
            },
            0
        );
    }
    let mut synthetic = SITES;
    for (rva, expected, stolen) in &mut synthetic {
        let actual = copy_current::<8>((base + *rva) as u64).unwrap();
        assert_eq!(&actual[..(*stolen).min(8)], &expected[..(*stolen).min(8)]);
        *expected = actual;
    }
    unsafe { install_at(base, &synthetic) }.unwrap();
    let mut receiver = vec![0u64; 0x802000 / 8];
    let pointer = receiver.as_mut_ptr() as u64;
    unsafe {
        *((pointer + 0x14f0) as *mut u64) = 0x1234;
    }
    for (i, (rva, _, _)) in SITES.into_iter().enumerate() {
        let call: unsafe extern "system" fn(u64, u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(base + rva) };
        let before = receiver[0x6000 / 8];
        unsafe {
            SetLastError(0x123456);
        }
        let result = unsafe { call(pointer, if i == 8 { 0 } else { 2 }, 0, 0, 0xabcdef) };
        assert_eq!(
            result,
            if i == 1 { 0 } else { 0x1122_3344_5566_7788 },
            "site{i}"
        );
        assert_eq!(receiver[0x6000 / 8], before + 1, "original body count{i}");
        assert_eq!(unsafe { GetLastError() }, 0x123456, "LastError{i}");
        assert!(CALLS.with(|s| s.borrow().is_empty()));
        assert!(!FAILED.load(Ordering::Acquire));
    }
    unsafe { restore() }.unwrap();
    assert!(HOOKS.lock().unwrap().is_empty());
    for (rva, expected, _) in synthetic {
        assert_eq!(copy_current::<8>((base + rva) as u64), Some(expected));
    }
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
}
unsafe extern "system" {
    fn native_load_snapshot_original_body();
    fn native_load_copy_original_body();
    fn native_load_command_original_body();
    fn native_load_initialize_original_body();
    fn native_load_worker_original_body();
}
#[test]
fn load_shims_virtual_unwind_restore_nonvolatile_registers_and_original_return() {
    let sites = [
        native_load_snapshot_original_body as *const (),
        native_load_copy_original_body as *const (),
        native_load_command_original_body as *const (),
        native_load_initialize_original_body as *const (),
        native_load_worker_original_body as *const (),
    ];
    for (i, address) in sites.into_iter().enumerate() {
        let mut stack = [0u64; 64];
        let entry = unsafe { stack.as_mut_ptr().add(20) } as u64;
        unsafe {
            *(entry as *mut u64) = 0xabcdef;
        }
        let mut c: CONTEXT = unsafe { std::mem::zeroed() };
        c.Rip = address as u64;
        c.Rsp = entry;
        match i {
            1 => {
                c.Rsp -= 32;
                for (j, v) in [0xf15, 0xd1, 0x51, 0xb1].into_iter().enumerate() {
                    unsafe {
                        *((c.Rsp + 8 * j as u64) as *mut u64) = v;
                    }
                }
            }
            3 => {
                c.Rsp -= 24;
                for (j, v) in [0xe14, 0xc12, 0xb1].into_iter().enumerate() {
                    unsafe {
                        *((c.Rsp + 8 * j as u64) as *mut u64) = v;
                    }
                }
            }
            _ => unsafe {
                *((entry + 8) as *mut u64) = 0xb1;
            },
        }
        let mut base = 0;
        let row = unsafe { RtlLookupFunctionEntry(c.Rip, &mut base, null_mut()) };
        assert!(!row.is_null());
        let mut data = null_mut();
        let mut frame = 0;
        unsafe {
            RtlVirtualUnwind(
                0,
                base,
                c.Rip,
                row,
                &mut c,
                &mut data,
                &mut frame,
                null_mut(),
            );
        }
        assert_eq!(c.Rip, 0xabcdef, "return{i}");
        assert_eq!(c.Rsp, entry + 8, "rsp{i}");
        if i == 1 {
            assert_eq!((c.R15, c.Rdi, c.Rsi, c.Rbp), (0xf15, 0xd1, 0x51, 0xb1));
        } else if i == 3 {
            assert_eq!((c.R14, c.R12, c.Rbp), (0xe14, 0xc12, 0xb1));
        } else {
            assert_eq!(c.Rbx, 0xb1, "rbx{i}");
        }
    }
}
#[test]
fn pinned_native_load_complete_bodies_partitions_and_entry_boundaries_match() {
    let mut f = pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
    for body in fixed::FRAMES {
        let offset = pe_file_offset(&mut f, body.begin).unwrap();
        f.seek(SeekFrom::Start(offset)).unwrap();
        let mut bytes = vec![0; (body.end - body.begin) as usize];
        f.read_exact(&mut bytes).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), body.sha);
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert_ne!(
            format!("{:x}", Sha256::digest(&bytes)),
            body.sha,
            "tail mutation"
        );
        for part in body.parts {
            let offset = data_file_offset(&mut f, part.unwind).unwrap();
            f.seek(SeekFrom::Start(offset)).unwrap();
            let expected: Vec<u8> = (0..part.hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&part.hex[i..i + 2], 16).unwrap())
                .collect();
            let mut bytes = vec![0; expected.len()];
            f.read_exact(&mut bytes).unwrap();
            assert_eq!(bytes, expected);
        }
    }
    for (rva, expected, _) in SITES {
        let offset = pe_file_offset(&mut f, rva as u32).unwrap();
        f.seek(SeekFrom::Start(offset)).unwrap();
        let mut bytes = [0; 8];
        f.read_exact(&mut bytes).unwrap();
        assert_eq!(bytes, expected);
    }
}
fn data_file_offset(file: &mut File, rva: u32) -> Option<u64> {
    file.seek(SeekFrom::Start(0x3c)).ok()?;
    let mut d = [0; 4];
    file.read_exact(&mut d).ok()?;
    let nt = u64::from(u32::from_le_bytes(d));
    file.seek(SeekFrom::Start(nt + 6)).ok()?;
    let mut w = [0; 2];
    file.read_exact(&mut w).ok()?;
    let n = u16::from_le_bytes(w);
    file.seek(SeekFrom::Start(nt + 20)).ok()?;
    file.read_exact(&mut w).ok()?;
    let opt = u64::from(u16::from_le_bytes(w));
    for i in 0..n {
        file.seek(SeekFrom::Start(nt + 24 + opt + u64::from(i) * 40))
            .ok()?;
        let mut s = [0; 40];
        file.read_exact(&mut s).ok()?;
        let start = u32::from_le_bytes(s[12..16].try_into().ok()?);
        let size = u32::from_le_bytes(s[16..20].try_into().ok()?);
        if rva >= start && rva.checked_sub(start)? < size {
            return Some(
                u64::from(u32::from_le_bytes(s[20..24].try_into().ok()?)) + u64::from(rva - start),
            );
        }
    }
    None
}
