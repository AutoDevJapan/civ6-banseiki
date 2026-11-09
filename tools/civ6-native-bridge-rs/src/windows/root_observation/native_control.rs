//! Natural shutdown control frames. Their successful normal returns do not
//! stand in for native producer coverage or a scheduler's full join proof.
use super::*;
use std::cell::RefCell;
static CLOSING: AtomicBool = AtomicBool::new(false);
static WAITED: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_RETURNED: AtomicBool = AtomicBool::new(false);
static WORLD: AtomicU64 = AtomicU64::new(0);
static CORE: AtomicU64 = AtomicU64::new(0);
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
#[unsafe(no_mangle)]
static NATIVE_CONTROL_CONTINUE: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
thread_local! {static STACK:RefCell<Vec<(u64,u64,u64)>>=const {RefCell::new(Vec::new())};}
fn fault() -> ! {
    diagnostic(30);
    FAILED.store(true, Ordering::Release);
    TRACE.0[6].store(1, Ordering::Release);
    std::process::abort()
}
pub(super) fn request_close() {
    CLOSING.store(true, Ordering::Release);
}
pub(super) fn ready() -> bool {
    #[cfg(test)]
    if CORE.load(Ordering::Acquire) == 0 {
        return true;
    }
    // The installed adapters establish one path and tracked reservations.
    // They do not establish all producer admission/copy/cancel domains. Do
    // not convert their zero count or normal Wait return into new authority.
    native_coverage_complete()
        && WAITED.load(Ordering::Acquire)
        && SHUTDOWN_RETURNED.load(Ordering::Acquire)
}
fn native_coverage_complete() -> bool {
    false
}
pub(super) fn reopened() {
    CLOSING.store(false, Ordering::Release);
    WAITED.store(false, Ordering::Release);
    SHUTDOWN_RETURNED.store(false, Ordering::Release);
    WORLD.store(0, Ordering::Release);
}
extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        if !CLOSING.load(Ordering::Acquire) {
            return;
        }
        let core = CORE.load(Ordering::Acquire);
        let parent = STACK.with(|s| s.borrow().last().copied());
        match raw.invocation.kind {
            30 => {
                // Other real callers stop/reinitialize this subsystem during
                // ordinary play. Only the fixed destruction caller belongs
                // to this control chain; none of the others supplies a reload
                // receipt or a reason to abort the original call.
                if raw.invocation.caller != core + 0x471c2 {
                    return;
                }
                // A no-op is an ordinary native return, never a failed Wait.
                // The caller's earlier guard read can race a prior shutdown.
                if copy_current::<1>(raw.gpr[0] + 0x2a4) != Some([1]) {
                    return;
                }
                raw.invocation.world = raw.gpr[0];
                if raw.gpr[0] == 0 {
                    fault();
                }
                let old = WORLD
                    .compare_exchange(0, raw.gpr[0], Ordering::AcqRel, Ordering::Acquire)
                    .unwrap_or_else(|old| old);
                if old != 0 && old != raw.gpr[0] {
                    fault();
                }
            }
            31 => {
                let Some((30, world, _)) = parent else {
                    return;
                };
                if raw.invocation.caller != core + 0x63e1d5
                    || copy_current::<8>(world + 0x250).map(u64::from_le_bytes) != Some(raw.gpr[0])
                {
                    fault();
                }
                raw.invocation.world = world;
                raw.invocation.manager = raw.gpr[0];
            }
            32 => {
                let Some((31, world, manager)) = parent else {
                    return;
                };
                if raw.invocation.caller != core + 0x4a1196 || raw.gpr[0] != manager + 0x400 {
                    fault();
                }
                raw.invocation.world = world;
                raw.invocation.manager = manager;
            }
            _ => fault(),
        }
        raw.invocation.cookie = COOKIE;
        STACK.with(|s| {
            s.borrow_mut().push((
                raw.invocation.kind,
                raw.invocation.world,
                raw.invocation.manager,
            ))
        });
    }))
    .is_err()
    {
        fault();
    }
    unsafe { SetLastError(error) };
}
fn finish(raw: &mut Saved, exceptional: bool) {
    if raw.invocation.cookie != COOKIE {
        return;
    }
    raw.invocation.cookie = 0;
    let expected = (
        raw.invocation.kind,
        raw.invocation.world,
        raw.invocation.manager,
    );
    if STACK.with(|s| s.borrow_mut().pop()) != Some(expected) {
        fault();
    }
    if exceptional {
        if let Some(f) = FRAMES.get() {
            f.mark_unresolved();
        }
        FAILED.store(true, Ordering::Release);
        TRACE.0[6].store(1, Ordering::Release);
        WAITED.store(false, Ordering::Release);
        SHUTDOWN_RETURNED.store(false, Ordering::Release);
        return;
    }
    match raw.invocation.kind {
        32 => {
            // Native shutdown progressed through its own Wait. This gate is
            // BEFORE returning to4A1196 and its subsequent cleanup calls.
            // Jobs are settled by their original execute/destructor wrappers,
            // never by assuming that an empty set or this return is a join.
            if FRAMES
                .get()
                .is_none_or(|f| f.pending_on_set(raw.invocation.manager + 0x400) != Ok(0))
                || !super::close(Instant::now() + Duration::from_secs(2))
            {
                fault();
            }
            WAITED.store(true, Ordering::Release);
        }
        30 => {
            if !WAITED.load(Ordering::Acquire) {
                fault();
            }
            SHUTDOWN_RETURNED.store(true, Ordering::Release);
        }
        31 => {
            if !WAITED.load(Ordering::Acquire) {
                fault();
            }
        }
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
    ".globl native_control_world_original",
    ".seh_proc native_control_world_original",
    "native_control_world_original:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_CONTROL_CONTINUE]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl native_control_manager_original",
    ".seh_proc native_control_manager_original",
    "native_control_manager_original:",
    "mov [rsp+0x10],rbx",
    ".seh_savereg rbx,0x10",
    ".seh_endprologue",
    "mov [rsp+8],rax",
    "mov rax,[rip+NATIVE_CONTROL_CONTINUE+8]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+8]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl native_control_wait_original",
    ".seh_proc native_control_wait_original",
    "native_control_wait_original:",
    "mov [rsp+8],rbx",
    ".seh_savereg rbx,8",
    ".seh_endprologue",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_CONTROL_CONTINUE+16]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
);
unsafe extern "system" {
    fn native_control_world_original();
    fn native_control_manager_original();
    fn native_control_wait_original();
    fn native_control_world_gateway();
    fn native_control_manager_gateway();
    fn native_control_wait_gateway();
}
gateway!(
    "native_control_world_gateway",
    "30",
    native_control_world_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_control_manager_gateway",
    "31",
    native_control_manager_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_control_wait_gateway",
    "32",
    native_control_wait_original,
    begin,
    end,
    unwind
);
pub(super) unsafe fn install(core: usize) -> Result<(), &'static str> {
    CORE.store(core as u64, Ordering::Release);
    let sites = [
        (
            0x63e160,
            [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x80, 0xb9],
            6,
            native_control_world_gateway as *const () as usize,
        ),
        (
            0x4a1150,
            [0x48, 0x89, 0x5c, 0x24, 0x10, 0x56, 0x48, 0x83],
            5,
            native_control_manager_gateway as *const () as usize,
        ),
        (
            0x9922c0,
            [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83],
            5,
            native_control_wait_gateway as *const () as usize,
        ),
    ];
    let mut owners = HOOKS.lock().map_err(|_| "native control owner poisoned")?;
    for (i, (rva, expected, n, gateway)) in sites.into_iter().enumerate() {
        let entry = (core + rva) as *mut u8;
        if copy_current::<8>(entry as u64) != Some(expected) {
            return Err("native control entry differs");
        }
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(entry.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
            || info.Protect & 0xff != PAGE_EXECUTE_READ
        {
            return Err("native control protection");
        }
        let code = unsafe { executable_near(entry as usize, entry as usize, n) }
            .ok_or("native control relay unavailable")?;
        let mut unpublished = Unpublished(code);
        unsafe {
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), code, 6);
            ptr::copy_nonoverlapping((gateway as u64).to_le_bytes().as_ptr(), code.add(6), 8);
        }
        let mut old = 0;
        if unsafe { VirtualProtect(code.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
            || unsafe { FlushInstructionCache(GetCurrentProcess(), code.cast(), 14) } == 0
        {
            return Err("native control relay publication");
        }
        let mut patched = expected;
        patched[..5].copy_from_slice(
            &rel32_jump(entry as usize, code as usize).ok_or("native control range")?,
        );
        patched[5..n].fill(0x90);
        NATIVE_CONTROL_CONTINUE[i].store((core + rva + n) as u64, Ordering::Release);
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
        let r = unsafe {
            exchange_entry(
                entry,
                u64::from_le_bytes(expected),
                u64::from_le_bytes(patched),
            )
        }?;
        if !r.exchanged || !r.protection_restored || !r.cache_flushed {
            return Err("critical: native control publication");
        }
    }
    Ok(())
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut o = HOOKS.lock().map_err(|_| "native control owner poisoned")?;
    o.retain(|x| {
        let h = &x.hook;
        let now = copy_current::<8>(h.target as u64);
        let ok = now == Some(h.original)
            || (now == Some(h.patched)
                && unsafe {
                    exchange_entry(
                        h.target,
                        u64::from_le_bytes(h.patched),
                        u64::from_le_bytes(h.original),
                    )
                }
                .is_ok_and(|r| r.exchanged && r.protection_restored && r.cache_flushed));
        let mut old = 0;
        !(ok && unsafe { VirtualProtect(h.target.cast(), 8, x.protection, &mut old) } != 0)
    });
    if o.is_empty() {
        Ok(())
    } else {
        Err("critical: native control restoration")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    fn child(name: &str) -> bool {
        const ENV: &str = "CIV6_NATIVE_CONTROL_OWNED_CHILD";
        if std::env::var_os(ENV).is_some() {
            return true;
        }
        let mut p = std::process::Command::new(std::env::current_exe().unwrap())
            .creation_flags(0x08000000)
            .env(ENV, "1")
            .args([
                "--exact",
                &format!("windows::root_observation::native_frames::control::tests::{name}"),
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
                panic!("native control child timeout");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }
    fn raw(kind: u64, input: u64, caller: u64) -> Saved {
        let mut raw: Saved = unsafe { std::mem::zeroed() };
        raw.gpr[0] = input;
        raw.invocation.kind = kind;
        raw.invocation.caller = caller;
        raw
    }
    #[test]
    fn native_shutdown_progresses_after_grant_close_and_gates_before_cleanup() {
        if !child("native_shutdown_progresses_after_grant_close_and_gates_before_cleanup") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        CORE.store(0x180000000, Ordering::Release);
        let _install: unsafe fn(usize) -> Result<(), &'static str> = install;
        let f = FRAMES.get().unwrap();
        let job = f.issue(100).unwrap();
        f.pushed(job, 200).unwrap();
        assert!(!ready());
        request_close(); // No blocking seal before native shutdown.
        let mut world = vec![0u64; 0x2b0 / 8];
        world[0x250 / 8] = 300;
        world[0x2a0 / 8] = 1u64 << 32;
        let mut w = raw(30, world.as_ptr() as u64, 0x1800471c2);
        begin(&mut w);
        let mut m = raw(31, 300, 0x18063e1d5);
        begin(&mut m);
        let mut wait = raw(32, 300 + 0x400, 0x1804a1196);
        begin(&mut wait);
        // Original natural Wait completes the already reserved native work.
        assert_eq!(f.execute(200, 100), Ok(job));
        f.completed(job).unwrap();
        finish(&mut wait, false);
        assert!(WAITED.load(Ordering::Acquire));
        assert!(!ready());
        finish(&mut m, false);
        assert!(!ready());
        finish(&mut w, false);
        assert!(
            !ready(),
            "one native path must not certify all publisher/copy/cancel domains"
        );
        assert!(!super::reopen());
    }
    global_asm!(
        ".p2align 4",
        ".globl native_control_probe_world",
        ".seh_proc native_control_probe_world",
        "native_control_probe_world:",
        ".byte 0x40,0x53",
        ".seh_pushreg rbx",
        "sub rsp,0x20",
        ".seh_stackalloc 0x20",
        ".seh_endprologue",
        "inc qword ptr [rcx]",
        "mov eax,0x1122",
        "mov rbx,[rsp+0x20]",
        "lea rsp,[rsp+0x28]",
        "ret",
        ".seh_endproc",
        ".p2align 4",
        ".globl native_control_probe_manager",
        ".seh_proc native_control_probe_manager",
        "native_control_probe_manager:",
        "mov [rsp+0x10],rbx",
        ".seh_savereg rbx,0x10",
        ".seh_endprologue",
        "inc qword ptr [rcx]",
        "mov eax,0x2233",
        "mov rbx,[rsp+0x10]",
        "ret",
        ".seh_endproc",
        ".p2align 4",
        ".globl native_control_probe_wait",
        ".seh_proc native_control_probe_wait",
        "native_control_probe_wait:",
        "mov [rsp+8],rbx",
        ".seh_savereg rbx,8",
        ".seh_endprologue",
        "inc qword ptr [rcx]",
        "mov eax,0x3344",
        "mov rbx,[rsp+8]",
        "ret",
        ".seh_endproc",
    );
    unsafe extern "system" {
        fn native_control_probe_world();
        fn native_control_probe_manager();
        fn native_control_probe_wait();
    }
    #[test]
    fn control_gateways_run_original_once_without_claiming_a_shutdown_receipt() {
        if !child("control_gateways_run_original_once_without_claiming_a_shutdown_receipt") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        let _install: unsafe fn(usize) -> Result<(), &'static str> = install;
        for (i, (body, gateway, result)) in [
            (
                native_control_probe_world as *const () as u64,
                native_control_world_gateway as *const () as usize,
                0x1122,
            ),
            (
                native_control_probe_manager as *const () as u64,
                native_control_manager_gateway as *const () as usize,
                0x2233,
            ),
            (
                native_control_probe_wait as *const () as u64,
                native_control_wait_gateway as *const () as usize,
                0x3344,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            NATIVE_CONTROL_CONTINUE[i].store(body + if i == 0 { 6 } else { 5 }, Ordering::Release);
            let invoke: unsafe extern "system" fn(*mut u64) -> u64 =
                unsafe { std::mem::transmute(gateway) };
            let mut n = 0;
            assert_eq!(unsafe { invoke(&mut n) }, result);
            assert_eq!(n, 1);
        }
        assert!(!WAITED.load(Ordering::Acquire));
        assert!(!SHUTDOWN_RETURNED.load(Ordering::Acquire));
    }

    #[test]
    fn unrelated_world_shutdown_caller_never_creates_a_reload_receipt() {
        if !child("unrelated_world_shutdown_caller_never_creates_a_reload_receipt") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        CORE.store(0x180000000, Ordering::Release);
        request_close();
        for caller in [0x180046a7b, 0x18063deb7, 0x180597999] {
            let mut w = raw(30, 1, caller);
            begin(&mut w);
            assert_eq!(w.invocation.cookie, 0);
            finish(&mut w, false);
            assert_eq!(WORLD.load(Ordering::Acquire), 0);
            assert!(!WAITED.load(Ordering::Acquire));
        }
        let world = vec![0u8; 0x2b0];
        let mut no_op = raw(30, world.as_ptr() as u64, 0x1800471c2);
        begin(&mut no_op);
        assert_eq!(no_op.invocation.cookie, 0);
        finish(&mut no_op, false);
        assert_eq!(WORLD.load(Ordering::Acquire), 0);
    }

    #[test]
    fn actual_three_control_cas_chain_keeps_runtime_rebuild_closed_without_full_coverage() {
        if !child(
            "actual_three_control_cas_chain_keeps_runtime_rebuild_closed_without_full_coverage",
        ) {
            return;
        }
        if !supports_atomic_promotion_fetch() {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        let base = unsafe {
            VirtualAlloc(
                null_mut(),
                0xa00000,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        } as usize;
        assert_ne!(base, 0);
        let put = |rva: usize, bytes: &[u8]| unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), (base + rva) as *mut u8, bytes.len())
        };
        let call = |rva: usize, target: usize| {
            let mut b = [0u8; 5];
            b[0] = 0xe8;
            b[1..].copy_from_slice(&((target as i64 - rva as i64 - 5) as i32).to_le_bytes());
            put(rva, &b);
        };
        // All native bodies are process-owned fixture instructions. The
        // original control gateways, CAS, Close and Rebuild are real code.
        unsafe {
            ptr::write_bytes((base + 0x63e160) as *mut u8, 0x90, 0x200);
            ptr::write_bytes((base + 0x4a1150) as *mut u8, 0x90, 0x80);
        }
        put(
            0x63e160,
            &[
                0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x80, 0xb9, 0xa4, 2, 0, 0, 0, 0x48, 0x8b, 0xd9,
            ],
        );
        put(0x63e1c9, &[0x48, 0x8b, 0x8b, 0x50, 2, 0, 0]);
        call(0x63e1d0, 0x4a1150);
        put(0x63e1d5, &[0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3]);
        put(
            0x4a1150,
            &[0x48, 0x89, 0x5c, 0x24, 0x10, 0x56, 0x48, 0x83, 0xec, 0x30],
        );
        put(0x4a117f, &[0x48, 0x81, 0xc1, 0, 4, 0, 0]);
        call(0x4a1191, 0x9922c0);
        put(
            0x4a1196,
            &[
                0x48, 0x8b, 0x5c, 0x24, 0x48, 0x48, 0x83, 0xc4, 0x30, 0x5e, 0xc3,
            ],
        );
        put(
            0x9922c0,
            &[0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83, 0xec, 0x30],
        );
        put(
            0x9922ca,
            &[
                0x48, 0x8b, 0x5c, 0x24, 0x40, 0x48, 0x83, 0xc4, 0x30, 0x5f, 0xc3,
            ],
        );
        put(0x471b0, &[0x48, 0x83, 0xec, 0x28]);
        unsafe {
            ptr::write_bytes((base + 0x471b4) as *mut u8, 0x90, 9);
        }
        call(0x471bd, 0x63e160);
        put(0x471c2, &[0x48, 0x83, 0xc4, 0x28, 0xc3]);
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(base as _, 0xa00000, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        unsafe { install(base) }.unwrap();
        runtime::fixture_invocation_runtime();
        assert_eq!(
            runtime::BridgeRuntimeClose(null_mut()),
            BridgeStatus::Ok as u32
        );
        let mut world = vec![0u64; 0x2b0 / 8];
        let mut manager = vec![0u64; 0x500 / 8];
        world[0x250 / 8] = manager.as_mut_ptr() as u64;
        world[0x2a0 / 8] = 1u64 << 32;
        let invoke: unsafe extern "system" fn(*mut u64) =
            unsafe { std::mem::transmute(base + 0x471b0) };
        unsafe {
            invoke(world.as_mut_ptr());
        }
        assert!(WAITED.load(Ordering::Acquire));
        assert!(SHUTDOWN_RETURNED.load(Ordering::Acquire));
        assert_eq!(
            runtime::BridgeRuntimeRebuild(null_mut()),
            BridgeStatus::Critical as u32
        );
        assert!(runtime::fixture_product_inactive());
        unsafe { restore() }.unwrap();
        assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
    }
}
