//! Natural shutdown control frames. Their successful normal returns do not
//! stand in for native producer coverage or a scheduler's full join proof.
use super::*;
use std::cell::RefCell;
static CLOSING: AtomicBool = AtomicBool::new(false);
static WAITED: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_RETURNED: AtomicBool = AtomicBool::new(false);
// Observed manager/list close evidence for the current World close. It is a
// necessary condition for ready() and never a native seal by itself.
static CLOSURE_COMPLETE: AtomicBool = AtomicBool::new(false);
static WORLD: AtomicU64 = AtomicU64::new(0);
static WORLD_BIRTH: Mutex<Option<worlds::WorldLease>> = Mutex::new(None);
static CORE: AtomicU64 = AtomicU64::new(0);
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
pub(super) fn hook_entries_owned() -> bool {
    owns_hook_entries(&HOOKS, NATIVE_CONTROL_CONTINUE.len())
}
#[unsafe(no_mangle)]
static NATIVE_CONTROL_CONTINUE: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
thread_local! {static STACK:RefCell<Vec<(u64,u64,u64)>>=const {RefCell::new(Vec::new())};}
#[track_caller]
fn fault() -> ! {
    census::fault(
        census::module(module_path!()),
        std::panic::Location::caller().line(),
    );
    diagnostic(30);
    FAILED.store(true, Ordering::Release);
    TRACE.0[6].store(1, Ordering::Release);
    std::process::abort()
}
pub(super) fn request_close() {
    CLOSING.store(true, Ordering::Release);
}
pub(super) fn owns_operation_cleanup(caller: u64) -> bool {
    caller == CORE.load(Ordering::Acquire) + 0x63e239 && owns_inert_cleanup()
}
/// A verified World shutdown is still inside its original destruction body,
/// after its own set Wait. Pure inert-storage cleanup may retain that same
/// lifetime; this does not reopen producer or product admission.
pub(super) fn owns_inert_cleanup() -> bool {
    if !CLOSING.load(Ordering::Acquire) || !WAITED.load(Ordering::Acquire) {
        return false;
    }
    STACK.with(|s| {
        s.borrow()
            .last()
            .is_some_and(|(kind, world, _)| *kind == 30 && *world == WORLD.load(Ordering::Acquire))
    })
}
pub(super) fn ready() -> bool {
    #[cfg(test)]
    if CORE.load(Ordering::Acquire) == 0 {
        return true;
    }
    // The installed adapters establish one path and tracked reservations.
    // They do not establish all producer admission/copy/cancel domains. Do
    // not convert their zero count or normal Wait return into new authority.
    native_coverage_complete() && observed_shutdown_complete()
}
/// Necessary local lifetime facts, deliberately separate from coverage.
pub(super) fn observed_shutdown_complete() -> bool {
    CORE.load(Ordering::Acquire) != 0
        && CLOSING.load(Ordering::Acquire)
        && WAITED.load(Ordering::Acquire)
        && SHUTDOWN_RETURNED.load(Ordering::Acquire)
        && CLOSURE_COMPLETE.load(Ordering::Acquire)
        && WORLD_BIRTH
            .lock()
            .is_ok_and(|birth| birth.is_some_and(worlds::completed))
}
fn native_coverage_complete() -> bool {
    false
}
pub(super) fn reopened() {
    CLOSING.store(false, Ordering::Release);
    WAITED.store(false, Ordering::Release);
    SHUTDOWN_RETURNED.store(false, Ordering::Release);
    CLOSURE_COMPLETE.store(false, Ordering::Release);
    WORLD.store(0, Ordering::Release);
    *WORLD_BIRTH.lock().unwrap_or_else(|_| fault()) = None;
}
extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        if raw.invocation.kind == 31
            && STACK.with(|s| s.borrow().last().is_none_or(|(kind, _, _)| *kind != 30))
        {
            // 4A1150 outside the tracked World close, such as a manager recycle
            // at 63D443. Recorded only; never a closing certificate.
            scheduler::closure::unowned_shutdown();
        }
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
                if worlds::installed() {
                    let birth = worlds::birth(raw.gpr[0]);
                    if birth != worlds::closing(raw.gpr[0]) || birth.is_none() {
                        fault();
                    }
                    *WORLD_BIRTH.lock().unwrap_or_else(|_| fault()) = birth;
                }
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
                let list = copy_current::<4>(raw.gpr[0] + 0x410)
                    .map(u32::from_le_bytes)
                    .unwrap_or_else(|| fault());
                scheduler::closure::close_begin(world, raw.gpr[0], list)
                    .unwrap_or_else(|v| scheduler::closure::fault(v));
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
    if exceptional {
        census::terminal(census::module(module_path!()), raw.invocation.kind);
    }
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
        CLOSURE_COMPLETE.store(false, Ordering::Release);
        return;
    }
    match raw.invocation.kind {
        32 => {
            // Native shutdown progressed through its own Wait. This gate is
            // BEFORE returning to4A1196 and its subsequent cleanup calls.
            // Jobs are settled by their original execute/destructor wrappers.
            // Inert serialized nodes remain for later original 380B0 cleanup;
            // their presence is not an executing job and cannot block Wait.
            let host_receipt = consumers::take_wait_receipt(
                raw.invocation.manager + 0x400,
                raw.invocation.native_rsp,
            );
            if consumers::require_wait_receipt() && host_receipt.is_none() {
                #[cfg(test)]
                if std::env::var_os("CIV6_NATIVE_CONTROL_CURRENT_ARENA_CHILD").is_some() {
                    println!("HOST_CURRENT_ARENA_RECEIPT_REFUSED");
                    use std::io::Write;
                    std::io::stdout().flush().unwrap();
                }
                fault();
            }
            scheduler::closure::wait_returned(raw.invocation.manager)
                .unwrap_or_else(|v| scheduler::closure::fault(v));
            if FRAMES
                .get()
                .is_none_or(|f| f.pending_on_set(raw.invocation.manager + 0x400) != Ok(0))
                || !super::close_executions(Instant::now() + Duration::from_secs(2))
            {
                fault();
            }
            WAITED.store(true, Ordering::Release);
        }
        30 => {
            if !WAITED.load(Ordering::Acquire)
                || !super::close(Instant::now() + Duration::from_secs(2))
            {
                fault();
            }
            let receipt = scheduler::closure::world_closed(raw.invocation.world)
                .unwrap_or_else(|v| scheduler::closure::fault(v));
            CLOSURE_COMPLETE.store(receipt.complete(), Ordering::Release);
            if let Some(birth) = worlds::birth(raw.invocation.world) {
                sessions::retire_world(sessions::SessionWorldReceipt {
                    binding_id: birth.serial,
                    epoch: birth.epoch,
                })
                .unwrap_or_else(|_| fault());
            }
            SHUTDOWN_RETURNED.store(true, Ordering::Release);
        }
        31 => {
            if !WAITED.load(Ordering::Acquire) {
                fault();
            }
            scheduler::closure::close_end(raw.invocation.manager)
                .unwrap_or_else(|v| scheduler::closure::fault(v));
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
    static MIXED_BASE: AtomicU64 = AtomicU64::new(0);
    static MIXED_PARALLEL: AtomicBool = AtomicBool::new(false);
    static SIX_STARTED: std::sync::OnceLock<std::sync::Barrier> = std::sync::OnceLock::new();
    static SIX_RELEASE: std::sync::OnceLock<std::sync::Barrier> = std::sync::OnceLock::new();
    static SIX_WORKER: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);
    extern "system" fn mixed_outer_control(world: *mut u64) {
        assert_eq!(super::super::counts()[3], 6);
        if MIXED_PARALLEL.load(Ordering::Acquire) {
            SIX_STARTED.get().unwrap().wait();
            SIX_RELEASE.get().unwrap().wait();
            return;
        }
        println!("SIX_NATIVE_OUTER_ACTIVE_BEFORE_CONTROL");
        use std::io::Write;
        std::io::stdout().flush().unwrap();
        let invoke: unsafe extern "system" fn(*mut u64) =
            unsafe { std::mem::transmute((MIXED_BASE.load(Ordering::Acquire) + 0x471b0) as usize) };
        unsafe { invoke(world) };
    }
    extern "system" fn mixed_native_wait() {
        assert_eq!(super::super::counts()[3], 6);
        SIX_RELEASE.get().unwrap().wait();
        SIX_WORKER.lock().unwrap().take().unwrap().join().unwrap();
        assert_eq!(super::super::counts()[3], 0);
    }
    #[test]
    fn same_stack_six_outer_gateways_cannot_authorize_destructive_wait_return() {
        let Some(_fixture) = crate::windows::atomic_fixture::require(
            "same_stack_six_outer_gateways_cannot_authorize_destructive_wait_return",
        ) else {
            return;
        };
        const ENV: &str = "CIV6_NATIVE_CONTROL_SIX_STACK_CHILD";
        if std::env::var_os(ENV).is_none() {
            let mut p=std::process::Command::new(std::env::current_exe().unwrap())
                .creation_flags(0x08000000).env(ENV,"1").args([
                    "--exact","windows::root_observation::native_frames::control::tests::same_stack_six_outer_gateways_cannot_authorize_destructive_wait_return","--nocapture"
                ]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                if p.try_wait().unwrap().is_some() {
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = p.kill();
                    let _ = p.wait();
                    panic!("mixed control child timeout");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let out = p.wait_with_output().unwrap();
            assert!(!out.status.success());
            assert!(
                String::from_utf8_lossy(&out.stdout)
                    .contains("SIX_NATIVE_OUTER_ACTIVE_BEFORE_CONTROL"),
                "did not reach six live original gateways: {:?}",
                out
            );
            assert!(
                String::from_utf8_lossy(&out.stdout)
                    .contains("SIX_NATIVE_FRAMES_REENTRANT_AT_WAIT"),
                "original control chain did not reach the expected Wait drain: {:?}",
                out
            );
            return;
        }
        mixed_fixture(false, false);
    }
    #[test]
    fn original_wait_joins_worker_through_six_full_gateways_before_cleanup() {
        if !child("original_wait_joins_worker_through_six_full_gateways_before_cleanup") {
            return;
        }
        let Some(_fixture) = crate::windows::atomic_fixture::require(
            "original_wait_joins_worker_through_six_full_gateways_before_cleanup",
        ) else {
            return;
        };
        mixed_fixture(true, false);
    }
    #[test]
    fn real_host_wait_receipt_reaches_control_after_six_worker_frames_return() {
        if !child("real_host_wait_receipt_reaches_control_after_six_worker_frames_return") {
            return;
        }
        let Some(_fixture) = crate::windows::atomic_fixture::require(
            "real_host_wait_receipt_reaches_control_after_six_worker_frames_return",
        ) else {
            return;
        };
        mixed_fixture(true, true);
    }
    #[test]
    fn current_arena_normal_return_never_reaches_destructive_control_cleanup() {
        let Some(_fixture) = crate::windows::atomic_fixture::require(
            "current_arena_normal_return_never_reaches_destructive_control_cleanup",
        ) else {
            return;
        };
        const ENV: &str = "CIV6_NATIVE_CONTROL_CURRENT_ARENA_CHILD";
        if std::env::var_os(ENV).is_none() {
            let mut p=std::process::Command::new(std::env::current_exe().unwrap())
                .creation_flags(0x08000000).env(ENV,"1").args([
                    "--exact","windows::root_observation::native_frames::control::tests::current_arena_normal_return_never_reaches_destructive_control_cleanup","--nocapture"
                ]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                if p.try_wait().unwrap().is_some() {
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = p.kill();
                    let _ = p.wait();
                    panic!("current arena fixture timeout");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let out = p.wait_with_output().unwrap();
            assert!(!out.status.success());
            assert!(
                String::from_utf8_lossy(&out.stdout).contains("HOST_CURRENT_ARENA_RECEIPT_REFUSED"),
                "missing exact control refusal: {:?}",
                out
            );
            return;
        }
        mixed_fixture(true, true);
        panic!("current arena returned through destructive control cleanup");
    }
    fn mixed_fixture(parallel: bool, with_host: bool) {
        MIXED_PARALLEL.store(parallel, Ordering::Release);
        if parallel {
            SIX_STARTED.set(std::sync::Barrier::new(2)).unwrap();
            SIX_RELEASE.set(std::sync::Barrier::new(2)).unwrap();
        }
        let size = if with_host { 0xe50000 } else { 0xa00000 };
        let base =
            unsafe { VirtualAlloc(null_mut(), size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
                as usize;
        assert_ne!(base, 0);
        MIXED_BASE.store(base as u64, Ordering::Release);
        let put = |rva: usize, bytes: &[u8]| unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), (base + rva) as *mut u8, bytes.len())
        };
        let call = |rva: usize, target: usize| {
            let mut b = [0xe8u8; 5];
            b[1..].copy_from_slice(&((target as i64 - rva as i64 - 5) as i32).to_le_bytes());
            put(rva, &b);
        };
        let extensions: [&[u8]; 6] = [
            &[0x48, 0x8b, 0xd9],
            &[0x48, 0x8b, 0xf1],
            &[0x55, 0x56, 0x57, 0x48, 0x83, 0xec, 0x20],
            &[],
            &[0x48, 0x83, 0xec, 0x28],
            &[],
        ];
        let tails: [&[u8]; 6] = [
            &[0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3],
            &[0x48, 0x83, 0xc4, 0x50, 0x5e, 0xc3],
            &[
                0x48, 0x83, 0xc4, 0x20, 0x5f, 0x5e, 0x5d, 0x48, 0x8b, 0x5c, 0x24, 0x18, 0xc3,
            ],
            &[0x48, 0x81, 0xc4, 0x90, 2, 0, 0, 0x5f, 0x5d, 0x5b, 0xc3],
            &[0x48, 0x83, 0xc4, 0x28, 0x41, 0x5e, 0x5d, 0xc3],
            &[0x48, 0x81, 0xc4, 0xd8, 0, 0, 0, 0x5f, 0x5b, 0xc3],
        ];
        for (i, (rva, prefix, n)) in super::super::SITES.into_iter().take(6).enumerate() {
            let mut code = prefix[..n].to_vec();
            code.extend(extensions[i]);
            if i < 5 {
                let pos = rva + code.len();
                code.push(0xe8);
                code.extend(
                    ((super::super::SITES[i + 1].0 as i64 - pos as i64 - 5) as i32).to_le_bytes(),
                );
            } else {
                code.extend([0x48, 0xb8]);
                code.extend((mixed_outer_control as *const () as u64).to_le_bytes());
                code.extend([0xff, 0xd0]);
            }
            code.extend(tails[i]);
            put(rva, &code);
        }
        super::super::outer_tests::seed_extra(base);
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
        if parallel {
            let mut wait_tail = vec![0x48, 0xb8];
            wait_tail.extend((mixed_native_wait as *const () as u64).to_le_bytes());
            wait_tail.extend([
                0xff, 0xd0, 0x48, 0x8b, 0x5c, 0x24, 0x40, 0x48, 0x83, 0xc4, 0x30, 0x5f, 0xc3,
            ]);
            put(0x9922ca, &wait_tail);
        }
        if with_host {
            unsafe {
                ptr::write_bytes((base + 0x9922ca) as *mut u8, 0x90, 0x40);
            }
            call(0x9922fc, 0x986240);
            put(
                0x992301,
                &[
                    0x48, 0x8b, 0x5c, 0x24, 0x40, 0x48, 0x83, 0xc4, 0x30, 0x5f, 0xc3,
                ],
            );
            for rva in [0xe417d0, 0xe41c00] {
                put(rva, &[0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x74]);
            }
            unsafe {
                ptr::write_bytes((base + 0x986240) as *mut u8, 0x90, 0x60);
            }
            put(
                0x986240,
                &[
                    0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b, 0xf9,
                    0x48, 0x8b, 0x49, 8,
                ],
            );
            call(0x98626b, 0xe3eaf0);
            put(
                0x986270,
                &[
                    0x48, 0xc7, 0x47, 8, 0, 0, 0, 0, 0x48, 0x8b, 0x5c, 0x24, 0x30, 0x48, 0x83,
                    0xc4, 0x20, 0x5f, 0xc3,
                ],
            );
            let mut arena = vec![
                0x40, 0x55, 0x53, 0x56, 0x48, 0x8d, 0x6c, 0x24, 0xb9, 0x48, 0x83, 0xec, 0x20, 0x48,
                0xb8,
            ];
            arena.extend((mixed_native_wait as *const () as u64).to_le_bytes());
            arena.extend([0xff, 0xd0, 0x48, 0x83, 0xc4, 0x20, 0x5e, 0x5b, 0x5d, 0xc3]);
            put(0xe3eaf0, &arena);
        }
        put(0x471b0, &[0x48, 0x83, 0xec, 0x28]);
        unsafe {
            ptr::write_bytes((base + 0x471b4) as *mut u8, 0x90, 9);
        }
        call(0x471bd, 0x63e160);
        put(0x471c2, &[0x48, 0x83, 0xc4, 0x28, 0xc3]);
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(base as _, size, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        unsafe { super::super::fixture_install(base) }.unwrap();
        unsafe { install(base) }.unwrap();
        if with_host {
            unsafe { consumers::fixture_install(base, base) }.unwrap();
            if std::env::var_os("CIV6_NATIVE_CONTROL_CURRENT_ARENA_CHILD").is_some() {
                consumers::fixture_current_arena();
            }
        }
        runtime::fixture_invocation_runtime();
        let mut world = vec![0u64; 0x2b0 / 8];
        let mut manager = vec![0u64; 0x500 / 8];
        let mut arena = 0u64;
        if with_host {
            manager[0x408 / 8] = &mut arena as *mut u64 as u64;
        }
        world[0x250 / 8] = manager.as_mut_ptr() as u64;
        world[0x2a0 / 8] = 1u64 << 32;
        let invoke: unsafe extern "system" fn(*mut u64) =
            unsafe { std::mem::transmute(base + super::super::SITES[0].0) };
        if parallel {
            let raw_world = world.as_mut_ptr() as usize;
            *SIX_WORKER.lock().unwrap() = Some(std::thread::spawn(move || unsafe {
                invoke(raw_world as *mut u64)
            }));
            SIX_STARTED.get().unwrap().wait();
        }
        assert_eq!(
            runtime::BridgeRuntimeClose(null_mut()),
            BridgeStatus::Ok as u32
        );
        if parallel {
            let control: unsafe extern "system" fn(*mut u64) =
                unsafe { std::mem::transmute(base + 0x471b0) };
            unsafe { control(world.as_mut_ptr()) };
            assert!(WAITED.load(Ordering::Acquire));
            assert!(SHUTDOWN_RETURNED.load(Ordering::Acquire));
            if with_host {
                assert_eq!(manager[0x408 / 8], 0);
                assert!(
                    consumers::take_wait_receipt(manager.as_ptr() as u64 + 0x400, 0).is_none(),
                    "control must consume once"
                );
            }
            assert_eq!(super::super::counts(), [6, 6, 0, 0, 0]);
            assert_eq!(
                runtime::BridgeRuntimeRebuild(null_mut()),
                BridgeStatus::Critical as u32
            );
            unsafe { restore() }.unwrap();
            unsafe { super::super::restore() }.unwrap();
            assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
            return;
        }
        unsafe { invoke(world.as_mut_ptr()) };
        panic!("destructive Wait returned while original outer frames were still active");
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
        let manager = vec![0u64; 0x500 / 8];
        let key = manager.as_ptr() as u64;
        world[0x250 / 8] = key;
        world[0x2a0 / 8] = 1u64 << 32;
        let mut w = raw(30, world.as_ptr() as u64, 0x1800471c2);
        begin(&mut w);
        let mut m = raw(31, key, 0x18063e1d5);
        begin(&mut m);
        let mut wait = raw(32, key + 0x400, 0x1804a1196);
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
    #[test]
    fn shutdown_outside_the_tracked_world_close_is_recorded_without_a_closing_certificate() {
        if !child(
            "shutdown_outside_the_tracked_world_close_is_recorded_without_a_closing_certificate",
        ) {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        CORE.store(0x180000000, Ordering::Release);
        let manager = vec![0u64; 0x500 / 8];
        let key = manager.as_ptr() as u64;
        // 63D443 calls 4A1150 outside 63E160. It is seen before and after
        // Close was requested, but never becomes a tracked close.
        let mut before = raw(31, key, 0x18063d448);
        begin(&mut before);
        assert_eq!(scheduler::closure::unowned_count(), 1);
        request_close();
        let mut after = raw(31, key, 0x18063d448);
        begin(&mut after);
        assert_eq!(scheduler::closure::unowned_count(), 2);
        finish(&mut after, false);
        assert!(scheduler::closure::world_closed(0x1000).is_err());
        assert!(!WAITED.load(Ordering::Acquire));
        assert!(!SHUTDOWN_RETURNED.load(Ordering::Acquire));
        assert!(!ready());
    }
    #[test]
    fn inert_operation_storage_survives_wait_until_owned_original_cleanup() {
        if !child("inert_operation_storage_survives_wait_until_owned_original_cleanup") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        CORE.store(0x180000000, Ordering::Release);
        let f = FRAMES.get().unwrap();
        let p = f.begin_queue_producer().unwrap();
        f.queue_allocated(p, 123, None).unwrap();
        f.queue_producer_returned(p).unwrap();
        request_close();
        let mut world = vec![0u64; 0x2b0 / 8];
        let manager = vec![0u64; 0x500 / 8];
        let key = manager.as_ptr() as u64;
        world[0x250 / 8] = key;
        world[0x2a0 / 8] = 1u64 << 32;
        let mut w = raw(30, world.as_ptr() as u64, 0x1800471c2);
        begin(&mut w);
        let mut m = raw(31, key, 0x18063e1d5);
        begin(&mut m);
        let mut wait = raw(32, key + 0x400, 0x1804a1196);
        begin(&mut wait);
        finish(&mut wait, false);
        assert!(WAITED.load(Ordering::Acquire));
        assert!(f.contains_task(123));
        finish(&mut m, false);
        assert!(!owns_operation_cleanup(0x180036fef));
        assert!(owns_operation_cleanup(0x18063e239));
        let cleanup = f.begin_native_cleanup().unwrap();
        let node = f.retire_queue(123).unwrap();
        f.queue_retired(node).unwrap();
        f.leave(cleanup).unwrap();
        finish(&mut w, false);
        assert!(SHUTDOWN_RETURNED.load(Ordering::Acquire));
        assert!(!ready(), "local drain is not all producer coverage");
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
    fn observation_resume_requires_the_exact_closed_world_and_settled_uses_without_product_grants()
    {
        if !child("observation_resume_requires_the_exact_closed_world_and_settled_uses_without_product_grants") {
            return;
        }
        // Policy unit fixture: these controlled lifetime facts are not a
        // production install/coverage acceptance or a native join proof.
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        runtime::fixture_invocation_runtime();
        CORE.store(0x180000000, Ordering::Release);
        let birth = worlds::fixture_world_birth();
        *WORLD_BIRTH.lock().unwrap() = Some(birth);
        WORLD.store(birth.world, Ordering::Release);
        CLOSING.store(true, Ordering::Release);
        WAITED.store(true, Ordering::Release);
        SHUTDOWN_RETURNED.store(true, Ordering::Release);
        CLOSURE_COMPLETE.store(true, Ordering::Release);
        assert!(
            !observed_shutdown_complete(),
            "World's outer return is still absent"
        );
        assert!(!super::super::resume_unqualified_observation());
        worlds::fixture_world_close(birth);
        assert!(observed_shutdown_complete());
        for flag in [&CLOSING, &WAITED, &SHUTDOWN_RETURNED, &CLOSURE_COMPLETE] {
            flag.store(false, Ordering::Release);
            assert!(!observed_shutdown_complete());
            flag.store(true, Ordering::Release);
        }
        *WORLD_BIRTH.lock().unwrap() = Some(worlds::WorldLease {
            serial: birth.serial + 1,
            ..birth
        });
        assert!(
            !observed_shutdown_complete(),
            "an address alias cannot finish the saved World"
        );
        *WORLD_BIRTH.lock().unwrap() = Some(birth);
        assert!(
            !super::super::resume_unqualified_observation(),
            "product permission is still open"
        );
        let frames = FRAMES.get().unwrap();
        let pending = frames.enter().unwrap();
        assert_eq!(
            runtime::BridgeRuntimeClose(null_mut()),
            BridgeStatus::Ok as u32
        );
        assert_eq!(
            frames.seal_and_wait(Instant::now() + Duration::from_secs(1)),
            Err(crate::native_flight::Error::Reentrant)
        );
        assert!(
            !super::super::resume_unqualified_observation(),
            "a retained native use is still live"
        );
        frames.leave(pending).unwrap();
        frames
            .seal_and_wait(Instant::now() + Duration::from_secs(1))
            .unwrap();
        assert_eq!(
            runtime::BridgeRuntimeRebuild(null_mut()),
            BridgeStatus::Critical as u32,
            "this policy unit fixture did not install native entry owners"
        );
        assert!(!super::super::resume_observation_with_owned_entries(false));
        // Exercise the policy only with an injected ownership fact. The
        // production wrapper above rejects this incomplete installation.
        assert!(super::super::resume_observation_with_owned_entries(true));
        assert!(runtime::fixture_product_inactive());
        assert!(runtime::fixture_invocation_closed());
        assert_eq!(runtime::invocation_start(), 0);
        assert_eq!(
            TRACE.0[12].load(Ordering::Acquire),
            0,
            "no qualified reopen was certified"
        );
        let next = frames.enter().unwrap();
        assert!(
            frames.healthy(),
            "ordinary native work can execute after the settled boundary"
        );
        frames.leave(next).unwrap();
        assert!(
            !observed_shutdown_complete(),
            "one close receipt cannot be replayed"
        );
        assert!(!super::super::resume_unqualified_observation());
    }

    #[test]
    fn unresolved_native_use_never_turns_into_a_transparent_rebuild_status() {
        if !child("unresolved_native_use_never_turns_into_a_transparent_rebuild_status") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        runtime::fixture_invocation_runtime();
        FRAMES.get().unwrap().mark_unresolved();
        assert_eq!(
            runtime::BridgeRuntimeRebuild(null_mut()),
            BridgeStatus::Critical as u32
        );
        assert!(runtime::fixture_product_inactive());
        assert!(runtime::fixture_invocation_closed());
    }
    #[test]
    fn actual_three_control_cas_chain_keeps_runtime_rebuild_closed_without_full_coverage() {
        if !child(
            "actual_three_control_cas_chain_keeps_runtime_rebuild_closed_without_full_coverage",
        ) {
            return;
        }
        let Some(_fixture) = crate::windows::atomic_fixture::require(
            "actual_three_control_cas_chain_keeps_runtime_rebuild_closed_without_full_coverage",
        ) else {
            return;
        };
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
