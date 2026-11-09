//! Native job reservations are born at original Push, before native T exists.
//! This does not interpret set fields as a seal or call native Wait itself.
use super::*;
use std::cell::RefCell;
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
pub(super) fn hook_entries_owned() -> bool {
    owns_hook_entries(&HOOKS, SITES.len())
}

static CORE: AtomicU64 = AtomicU64::new(0);
static EXE: AtomicU64 = AtomicU64::new(0);
#[unsafe(no_mangle)]
static NATIVE_JOB_CONTINUE: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
// The exact SHA-pinned host: Push export, complete execute wrapper, destructor.
const SITES: [(usize, [u8; 8], usize); 4] = [
    (0x986480, [0x48, 0x83, 0xec, 0x38, 0x41, 0x0f, 0x10, 1], 8),
    (
        0x9862e0,
        [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b],
        6,
    ),
    (
        0x9857c0,
        [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8d],
        6,
    ),
    (0xe3ccf0, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x6c], 5),
];
thread_local! {static STACK:RefCell<Vec<(u64,u64,u64)>>=const {RefCell::new(Vec::new())};}
#[track_caller]
fn fault() -> ! {
    census::fault(
        census::module(module_path!()),
        std::panic::Location::caller().line(),
    );
    diagnostic(20);
    FAILED.store(true, Ordering::Release);
    TRACE.0[6].store(1, Ordering::Release);
    std::process::abort()
}
extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        loads::job_begin(raw);
        // Host load jobs can be published before the first Core is loaded.
        // Their separate load carrier is retained above; Core accounting is
        // activated only when its original runtime owner binds the Core.
        let Some(f) = FRAMES.get() else {
            return;
        };
        if CORE.load(Ordering::Acquire) == 0 {
            return;
        }
        let (id, task) = match raw.invocation.kind {
            20 => {
                let core = CORE.load(Ordering::Acquire);
                let from_core = (core..core + 0xc80000).contains(&raw.invocation.caller);
                let vft =
                    u64::from_le_bytes(copy_current::<8>(raw.gpr[1]).unwrap_or_else(|| fault()));
                let core_payload = (core..core + 0xc80000).contains(&vft);
                if !from_core && !core_payload {
                    return;
                }
                (
                    f.issue_on_set_with_provenance(
                        raw.gpr[1],
                        raw.gpr[2],
                        super::unproven_input_active(),
                    )
                    .unwrap_or_else(|_| fault()),
                    0,
                )
            }
            21 => {
                let t = raw.gpr[0];
                scheduler::attach_current(t);
                let _attached = consumers::attach_current(t);
                let p = u64::from_le_bytes(
                    copy_current::<8>(t.checked_add(8).unwrap_or_else(|| fault()))
                        .unwrap_or_else(|| fault()),
                );
                // Native P is current input to this executing T, so this
                // read stays within the original invocation's lifetime.
                let vft = u64::from_le_bytes(copy_current::<8>(p).unwrap_or_else(|| fault()));
                let core = CORE.load(Ordering::Acquire);
                if !f.contains_payload(p) && !(core..core + 0xc80000).contains(&vft) {
                    return;
                }
                (f.execute(t, p).unwrap_or_else(|_| fault()), t)
            }
            22 => {
                scheduler::attach_current(raw.gpr[0]);
                let _attached = consumers::attach_current(raw.gpr[0]);
                let Some(id) = f.retire_task(raw.gpr[0]).unwrap_or_else(|_| fault()) else {
                    return;
                };
                (id, raw.gpr[0])
            }
            23 => {
                if raw.invocation.caller != EXE.load(Ordering::Acquire) + 0x985f9e {
                    return;
                }
                let Some((20, id, _)) = STACK.with(|s| s.borrow().last().copied()) else {
                    return;
                };
                (id, 0)
            }
            _ => fault(),
        };
        raw.invocation.cookie = COOKIE;
        raw.invocation.token = id;
        raw.invocation.world = task;
        STACK.with(|s| s.borrow_mut().push((raw.invocation.kind, id, task)));
    }))
    .is_err()
    {
        fault();
    }
    unsafe { SetLastError(error) };
}
fn finish(raw: &mut Saved, exceptional: bool) {
    if exceptional {
        census::terminal(census::module(module_path!()), raw.invocation.kind);
    }
    scheduler::allocated(raw, exceptional);
    loads::job_finish(raw, exceptional);
    if raw.invocation.cookie != COOKIE {
        return;
    }
    raw.invocation.cookie = 0;
    let expected = (
        raw.invocation.kind,
        raw.invocation.token,
        raw.invocation.world,
    );
    if STACK.with(|s| s.borrow_mut().pop()) != Some(expected) {
        fault();
    }
    let f = FRAMES.get().unwrap_or_else(|| fault());
    if exceptional {
        f.mark_unresolved();
        FAILED.store(true, Ordering::Release);
        TRACE.0[6].store(1, Ordering::Release);
        return; // Original SEH continues; this is not completion/cancellation.
    }
    match raw.invocation.kind {
        // checked wrapper R8 output T, with no post-return pointer dereference
        20 => f
            .pushed(raw.invocation.token, raw.gpr[2])
            .unwrap_or_else(|_| fault()),
        21 => f
            .completed(raw.invocation.token)
            .unwrap_or_else(|_| fault()),
        22 => f
            .task_retired(raw.invocation.token)
            .unwrap_or_else(|_| fault()),
        23 => f
            .bind_before_publication(raw.invocation.token, raw.gpr[4])
            .unwrap_or_else(|_| fault()),
        _ => fault(),
    }
}
extern "system" fn end(raw: *mut Saved) {
    let e = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        finish(unsafe { &mut *raw }, false)
    }))
    .is_err()
    {
        fault();
    }
    unsafe { SetLastError(e) };
}
unsafe extern "system" fn unwind(
    r: *const EXCEPTION_RECORD,
    frame: usize,
    _: *mut CONTEXT,
    _: *mut c_void,
) -> i32 {
    let e = unsafe { GetLastError() };
    if !r.is_null() && unsafe { (*r).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0 {
        finish(unsafe { &mut *((frame + 0x20) as *mut Saved) }, true);
    }
    unsafe { SetLastError(e) };
    1
}
global_asm!(
    ".globl native_job_push_original",
    ".seh_proc native_job_push_original",
    "native_job_push_original:",
    "sub rsp,0x38",
    ".seh_stackalloc 0x38",
    ".seh_endprologue",
    "movups xmm0,[r9]",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_JOB_CONTINUE]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl native_job_execute_original",
    ".seh_proc native_job_execute_original",
    "native_job_execute_original:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_JOB_CONTINUE+8]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl native_job_destroy_original",
    ".seh_proc native_job_destroy_original",
    "native_job_destroy_original:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_JOB_CONTINUE+16]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl native_job_allocate_original",
    ".seh_proc native_job_allocate_original",
    "native_job_allocate_original:",
    "mov [rsp+8],rbx",
    ".seh_savereg rbx,8",
    ".seh_endprologue",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_JOB_CONTINUE+24]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
);
unsafe extern "system" {
    fn native_job_push_original();
    fn native_job_execute_original();
    fn native_job_destroy_original();
    fn native_job_push_gateway();
    fn native_job_execute_gateway();
    fn native_job_destroy_gateway();
    fn native_job_allocate_original();
    fn native_job_allocate_gateway();
}
gateway!(
    "native_job_push_gateway",
    "20",
    native_job_push_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_job_allocate_gateway",
    "23",
    native_job_allocate_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_job_execute_gateway",
    "21",
    native_job_execute_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_job_destroy_gateway",
    "22",
    native_job_destroy_original,
    begin,
    end,
    unwind
);

pub(super) unsafe fn install(core: usize) -> Result<(), &'static str> {
    CORE.store(core as u64, Ordering::Release);
    let exe = unsafe { GetModuleHandleW(null_mut()) } as usize;
    EXE.store(exe as u64, Ordering::Release);
    let _pin = super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA)?;
    let prepared = HOOKS.lock().map_err(|_| "native job owner poisoned")?;
    if !prepared.is_empty() {
        if prepared.len() != 4
            || prepared
                .iter()
                .any(|o| copy_current::<8>(o.hook.target as u64) != Some(o.hook.patched))
        {
            return Err("native job early installation differs");
        }
        return Ok(());
    }
    drop(prepared);
    unsafe { install_at(exe) }
}
pub(super) unsafe fn prepare_host(exe: usize) -> Result<(), &'static str> {
    diagnostic(1100);
    let _pin = super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA)?;
    EXE.store(exe as u64, Ordering::Release);
    unsafe { install_at(exe) }
}
pub(super) fn push_reserved() -> bool {
    STACK.with(|s| s.borrow().last().is_some_and(|(kind, _, _)| *kind == 20))
}
#[cfg(test)]
pub(super) fn fixture_early_load_execute(continuation: usize) -> usize {
    CORE.store(0, Ordering::Release);
    NATIVE_JOB_CONTINUE[1].store(continuation as u64, Ordering::Release);
    native_job_execute_gateway as *const () as usize
}
#[cfg(test)]
pub(super) fn fixture_activate_core(core: u64) {
    CORE.store(core, Ordering::Release);
}
#[cfg(test)]
pub(super) unsafe fn fixture_host_install(exe: usize) -> Result<(), &'static str> {
    unsafe { install_at(exe) }
}
#[cfg(test)]
pub(super) fn fixture_host_sites() -> [(usize, [u8; 8], usize); 4] {
    SITES
}
unsafe fn install_at(exe: usize) -> Result<(), &'static str> {
    let callbacks = [
        native_job_push_gateway as *const () as usize,
        native_job_execute_gateway as *const () as usize,
        native_job_destroy_gateway as *const () as usize,
        native_job_allocate_gateway as *const () as usize,
    ];
    diagnostic(1101);
    let mut owners = HOOKS.lock().map_err(|_| "native job owner poisoned")?;
    if !owners.is_empty() {
        diagnostic(1102);
        return Err("native jobs already installed");
    }
    for (i, (rva, expected, displaced)) in SITES.into_iter().enumerate() {
        // Preserve the failed site's phase before the outer Prepare reports
        // a dependency refusal. A fresh process can then distinguish entry,
        // protection, relay allocation, and atomic publication failures.
        let stage = 1110 + i * 10;
        diagnostic(stage);
        let entry = (exe + rva) as *mut u8;
        if copy_current::<8>(entry as u64) != Some(expected) {
            return Err("native job entry differs");
        }
        diagnostic(stage + 1);
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(entry.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
            || info.Protect & 0xff != PAGE_EXECUTE_READ
        {
            return Err("native job protection");
        }
        diagnostic(stage + 2);
        let code = unsafe { executable_near(entry as usize, entry as usize, displaced) }
            .ok_or("native job relay unavailable")?;
        let mut unpublished = Unpublished(code);
        unsafe {
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), code, 6);
            ptr::copy_nonoverlapping((callbacks[i] as u64).to_le_bytes().as_ptr(), code.add(6), 8);
        }
        let mut old = 0;
        diagnostic(stage + 3);
        if unsafe { VirtualProtect(code.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
            || unsafe { FlushInstructionCache(GetCurrentProcess(), code.cast(), 14) } == 0
        {
            return Err("native job relay publication");
        }
        let mut patched = expected;
        patched[..5].copy_from_slice(
            &rel32_jump(entry as usize, code as usize).ok_or("native job relay range")?,
        );
        patched[5..displaced].fill(0x90);
        NATIVE_JOB_CONTINUE[i].store((exe + rva + displaced) as u64, Ordering::Release);
        owners.push(OwnedHook {
            hook: Hook {
                target: entry,
                original: expected,
                patched,
                prefix: None,
                _code: code,
            },
            protection: info.Protect,
        });
        unpublished.0 = null_mut();
        diagnostic(stage + 4);
        let o = unsafe {
            exchange_entry(
                entry,
                u64::from_le_bytes(expected),
                u64::from_le_bytes(patched),
            )
        }?;
        diagnostic(stage + 5);
        if !o.exchanged || !o.protection_restored || !o.cache_flushed {
            return Err("critical: native job publication");
        }
    }
    Ok(())
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut owners = HOOKS.lock().map_err(|_| "native job owner poisoned")?;
    owners.retain(|o| {
        let h = &o.hook;
        let current = copy_current::<8>(h.target as u64);
        let ok = current == Some(h.original)
            || (current == Some(h.patched)
                && unsafe {
                    exchange_entry(
                        h.target,
                        u64::from_le_bytes(h.patched),
                        u64::from_le_bytes(h.original),
                    )
                }
                .is_ok_and(|r| r.exchanged && r.cache_flushed && r.protection_restored));
        let mut old = 0;
        !(ok && unsafe { VirtualProtect(h.target.cast(), 8, o.protection, &mut old) } != 0)
    });
    if owners.is_empty() {
        Ok(())
    } else {
        Err("critical: native jobs restoration")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    fn child(name: &str) -> bool {
        const ENV: &str = "CIV6_NATIVE_JOBS_OWNED_CHILD";
        if std::env::var_os(ENV).is_some() {
            return true;
        }
        let mut p = std::process::Command::new(std::env::current_exe().unwrap())
            .creation_flags(0x08000000)
            .env(ENV, "1")
            .args([
                "--exact",
                &format!("windows::root_observation::native_frames::jobs::tests::{name}"),
                "--nocapture",
            ])
            .spawn()
            .unwrap();
        let d = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(s) = p.try_wait().unwrap() {
                assert!(s.success());
                break;
            }
            if Instant::now() >= d {
                let _ = p.kill();
                let _ = p.wait();
                panic!("native jobs owned child timeout");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }
    global_asm!(
        ".p2align 4",
        ".globl native_jobs_fixture_push",
        ".seh_proc native_jobs_fixture_push",
        "native_jobs_fixture_push:",
        "sub rsp,0x38",
        ".seh_stackalloc 0x38",
        ".seh_endprologue",
        "movups xmm0,[r9]",
        "mov r8,rcx",
        "mov [r8+8],rdx",
        "lea rsp,[rsp+0x38]",
        "ret",
        ".seh_endproc",
        ".p2align 4",
        ".globl native_jobs_fixture_execute",
        ".seh_proc native_jobs_fixture_execute",
        "native_jobs_fixture_execute:",
        ".byte 0x40,0x53",
        ".seh_pushreg rbx",
        "sub rsp,0x20",
        ".seh_stackalloc 0x20",
        ".seh_endprologue",
        "mov rax,[rcx+8]",
        "inc qword ptr [rax+8]",
        "mov rax,0x1122",
        "mov rbx,[rsp+0x20]",
        "lea rsp,[rsp+0x28]",
        "ret",
        ".seh_endproc",
        ".p2align 4",
        ".globl native_jobs_fixture_destroy",
        ".seh_proc native_jobs_fixture_destroy",
        "native_jobs_fixture_destroy:",
        ".byte 0x40,0x53",
        ".seh_pushreg rbx",
        "sub rsp,0x20",
        ".seh_stackalloc 0x20",
        ".seh_endprologue",
        "inc qword ptr [rcx]",
        "mov rax,0x3344",
        "mov rbx,[rsp+0x20]",
        "lea rsp,[rsp+0x28]",
        "ret",
        ".seh_endproc",
        ".globl native_jobs_fixture_driver",
        ".seh_proc native_jobs_fixture_driver",
        "native_jobs_fixture_driver:",
        "sub rsp,0x28",
        ".seh_stackalloc 0x28",
        ".seh_endprologue",
        "mov [rsp+0x20],r8",
        "mov r8,rcx",
        "lea r9,[rip+native_jobs_fixture_metadata]",
        "call qword ptr [rsp+0x20]",
        ".globl native_jobs_fixture_push_return",
        "native_jobs_fixture_push_return:",
        "mov rax,r8",
        "lea rsp,[rsp+0x28]",
        "ret",
        ".seh_endproc",
        ".section .rdata",
        ".p2align 4",
        "native_jobs_fixture_metadata:",
        ".quad 0,0",
        ".text",
    );
    unsafe extern "system" {
        fn native_jobs_fixture_push();
        fn native_jobs_fixture_execute();
        fn native_jobs_fixture_destroy();
        fn native_jobs_fixture_push_return();
        fn native_jobs_fixture_driver(t: *mut u64, p: *mut u64, entry: usize) -> u64;
    }
    #[test]
    fn actual_gateway_reservation_keeps_unexecuted_task_until_execution_or_native_destroy() {
        if !child(
            "actual_gateway_reservation_keeps_unexecuted_task_until_execution_or_native_destroy",
        ) {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        CORE.store(
            native_jobs_fixture_push_return as *const () as u64 - 0x992267,
            Ordering::Release,
        );
        for (i, body) in [
            native_jobs_fixture_push as *const () as u64,
            native_jobs_fixture_execute as *const () as u64,
            native_jobs_fixture_destroy as *const () as u64,
        ]
        .into_iter()
        .enumerate()
        {
            NATIVE_JOB_CONTINUE[i].store(body + if i == 0 { 8 } else { 6 }, Ordering::Release);
        }
        let invoke: unsafe extern "system" fn(*mut u64) -> u64 =
            unsafe { std::mem::transmute(native_job_execute_gateway as *const ()) };
        let destroy: unsafe extern "system" fn(*mut u64) -> u64 =
            unsafe { std::mem::transmute(native_job_destroy_gateway as *const ()) };
        let mut p = [0u64; 2];
        let mut t = [0u64; 2];
        assert_eq!(
            unsafe {
                native_jobs_fixture_driver(
                    t.as_mut_ptr(),
                    p.as_mut_ptr(),
                    native_job_push_gateway as *const () as usize,
                )
            },
            t.as_ptr() as u64
        );
        assert!(!super::close(Instant::now()));
        assert!(!super::reopen());
        assert_eq!(unsafe { invoke(t.as_mut_ptr()) }, 0x1122);
        assert_eq!(p[1], 1);
        assert!(super::close(Instant::now() + Duration::from_secs(2)));
        assert!(super::reopen());
        assert_eq!(
            unsafe {
                native_jobs_fixture_driver(
                    t.as_mut_ptr(),
                    p.as_mut_ptr(),
                    native_job_push_gateway as *const () as usize,
                )
            },
            t.as_ptr() as u64
        );
        assert_eq!(unsafe { destroy(t.as_mut_ptr()) }, 0x3344);
        assert!(super::close(Instant::now() + Duration::from_secs(2)));
    }
    #[test]
    fn real_primary_host_entry_words_and_r8_wrapper_are_pinned_before_install() {
        if !Path::new(GAME_EXE).is_file() {
            return;
        }
        let mut file = super::super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
        for (rva, expected, _) in SITES {
            let offset = pe_file_offset(&mut file, rva as u32).unwrap();
            file.seek(SeekFrom::Start(offset)).unwrap();
            let mut b = [0; 8];
            file.read_exact(&mut b).unwrap();
            assert_eq!(b, expected);
        }
        // Keep install's whole code reachable in the test build as well.
        let _install: unsafe fn(usize) -> Result<(), &'static str> = install;
        let _install_at: unsafe fn(usize) -> Result<(), &'static str> = install_at;
    }

    #[test]
    fn all_three_job_entry_cas_publish_restore_and_keep_continuations() {
        if !child("all_three_job_entry_cas_publish_restore_and_keep_continuations") {
            return;
        }
        let Some(_fixture) = crate::windows::atomic_fixture::require(
            "all_three_job_entry_cas_publish_restore_and_keep_continuations",
        ) else {
            return;
        };
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        let base = unsafe {
            VirtualAlloc(
                null_mut(),
                0xe40000,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        } as usize;
        assert_ne!(base, 0);
        for (rva, expected, _) in SITES {
            unsafe {
                ptr::copy_nonoverlapping(expected.as_ptr(), (base + rva) as *mut u8, 8);
            }
        }
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(base as _, 0xe40000, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        unsafe { install_at(base) }.unwrap();
        assert_eq!(HOOKS.lock().unwrap().len(), 4);
        for (i, (rva, _, n)) in SITES.into_iter().enumerate() {
            assert_eq!(
                NATIVE_JOB_CONTINUE[i].load(Ordering::Acquire),
                (base + rva + n) as u64
            );
            assert_eq!(copy_current::<1>((base + rva) as u64), Some([0xe9]));
        }
        unsafe { restore() }.unwrap();
        assert!(HOOKS.lock().unwrap().is_empty());
        for (rva, expected, _) in SITES {
            assert_eq!(copy_current::<8>((base + rva) as u64), Some(expected));
        }
        assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn original_allocator_return_binds_t_before_native_caller_can_cancel_it() {
        if !child("original_allocator_return_binds_t_before_native_caller_can_cancel_it") {
            return;
        }
        let f = FRAMES.get_or_init(NativeFrames::new);
        EXE.store(0x140000000, Ordering::Release);
        CORE.store(0x180000000, Ordering::Release);
        let id = f.issue_on_set(100, 10).unwrap();
        STACK.with(|s| s.borrow_mut().push((20, id, 0)));
        let mut allocation: Saved = unsafe { std::mem::zeroed() };
        allocation.invocation.kind = 23;
        allocation.invocation.caller = 0x140985f9e;
        begin(&mut allocation);
        allocation.gpr[4] = 200;
        finish(&mut allocation, false);
        assert!(f.contains_task(200));
        f.cancelled(200).unwrap();
        assert_eq!(f.pending_on_set(10), Ok(1));
        f.pushed(id, 200).unwrap();
        assert_eq!(f.pending_on_set(10), Ok(0));
        assert_eq!(STACK.with(|s| s.borrow_mut().pop()), Some((20, id, 0)));
    }

    #[test]
    fn destructor_callback_keeps_original_id_when_native_free_reuses_t() {
        if !child("destructor_callback_keeps_original_id_when_native_free_reuses_t") {
            return;
        }
        let f = FRAMES.get_or_init(NativeFrames::new);
        CORE.store(0x180000000, Ordering::Release);
        let old = f.issue_on_set(100, 10).unwrap();
        f.bind_before_publication(old, 123).unwrap();
        f.pushed(old, 123).unwrap();
        let mut destroy: Saved = unsafe { std::mem::zeroed() };
        destroy.invocation.kind = 22;
        destroy.gpr[0] = 123;
        begin(&mut destroy);
        assert_eq!(destroy.invocation.token, old);
        let new = f.issue_on_set(200, 10).unwrap();
        f.bind_before_publication(new, 123).unwrap();
        f.pushed(new, 123).unwrap();
        finish(&mut destroy, false);
        assert_eq!(f.pending_on_set(10), Ok(1));
        assert_eq!(f.execute(123, 200), Ok(new));
        f.completed(new).unwrap();
        f.seal_and_wait(Instant::now() + Duration::from_secs(1))
            .unwrap();
    }

    global_asm!(
        ".p2align 4",
        ".globl native_jobs_fixture_null_native_tail",
        ".seh_proc native_jobs_fixture_null_native_tail",
        "native_jobs_fixture_null_native_tail:",
        ".seh_endprologue",
        // Exact pinned caller985F9E..985FC5. The remainder only supplies the
        // still reached985FD6/985FEB null-derived store, with no game data.
        ".byte 0x4c,0x8b,0xc0,0x48,0x85,0xc0,0x74,0x14,0xc6,0x40,0xf5,0x01,0x48,0x8d,0x05,0x67,0x64,0x6e,0x00,0x49,0x89,0x00,0x49,0x89,0x78,0x08,0xeb,0x03,0x45,0x33,0xc0",
        "lea rcx,[r8-8]",
        "mov qword ptr [rcx],0",
        "ret",
        ".seh_endproc",
        ".p2align 4",
        ".globl native_jobs_fixture_null_native_entry",
        ".seh_proc native_jobs_fixture_null_native_entry",
        "native_jobs_fixture_null_native_entry:",
        ".seh_endprologue",
        "xor eax,eax",
        "jmp native_jobs_fixture_null_native_tail",
        ".seh_endproc",
    );
    unsafe extern "system" {
        fn native_jobs_fixture_null_native_entry();
    }
    #[test]
    fn pinned_allocator_null_branch_still_reaches_native_invalid_store() {
        const ENV: &str = "CIV6_NATIVE_PINNED_NULL_CALLER_CHILD";
        if std::env::var_os(ENV).is_some() {
            // A fixed leaf supplies RAX=0, without compiler-selected call
            // registers aliasing the allocation-result register.
            unsafe { native_jobs_fixture_null_native_entry() };
            unreachable!("the fixed native null path cannot return normally");
        }
        let mut p=std::process::Command::new(std::env::current_exe().unwrap())
            .creation_flags(0x08000000).env(ENV,"1").args([
                "--exact","windows::root_observation::native_frames::jobs::tests::pinned_allocator_null_branch_still_reaches_native_invalid_store","--nocapture"
            ]).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(s) = p.try_wait().unwrap() {
                assert_eq!(s.code().map(|x| x as u32), Some(0xc0000005));
                break;
            }
            if Instant::now() >= deadline {
                let _ = p.kill();
                let _ = p.wait();
                panic!("owned null caller timeout");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
