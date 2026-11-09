//! Resident accounting for pinned synchronous native frames. No sampling
//! deadline can turn it off. Admitted calls execute their original body once;
//! a new root after closure terminates the owned process before receiver use.
//! These sites do not establish asynchronous dispatch provenance or coverage
//! of every producer, and their drain is not a native shutdown certificate.
use super::*;
use crate::native_flight::frames::NativeFrames;
use windows_sys::Win32::System::Diagnostics::Debug::{CONTEXT, EXCEPTION_RECORD};

static FRAMES: OnceLock<NativeFrames> = OnceLock::new();
struct OwnedHook {
    hook: Hook,
    protection: u32,
}
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
fn owns_hook_entries(owners: &Mutex<Vec<OwnedHook>>, expected: usize) -> bool {
    expected != 0
        && owners.lock().is_ok_and(|owners| {
            owners.len() == expected
                && owners.iter().all(|owned| {
                    copy_current::<8>(owned.hook.target as u64) == Some(owned.hook.patched)
                })
        })
}
fn all_hook_entries_owned() -> bool {
    HOST_PREPARED.load(Ordering::Acquire)
        && owns_hook_entries(&HOOKS, SITES.len())
        && jobs::hook_entries_owned()
        && consumers::hook_entries_owned()
        && worlds::hook_entries_owned()
        && control::hook_entries_owned()
        && operations::hook_entries_owned()
        && sessions::hook_entries_owned()
        && loads::hook_entries_owned()
        && wire::hook_entries_owned()
        && scheduler::hook_entries_owned()
}
static FAILED: AtomicBool = AtomicBool::new(false);
static HOST_PREPARED: AtomicBool = AtomicBool::new(false);
#[path = "native_census.rs"]
pub(super) mod census;
#[path = "native_consumers.rs"]
mod consumers;
#[path = "native_control.rs"]
mod control;
#[path = "native_jobs.rs"]
mod jobs;
#[path = "native_loads.rs"]
mod loads;
#[path = "native_operations.rs"]
mod operations;
#[path = "native_scheduler.rs"]
mod scheduler;
#[path = "native_sessions.rs"]
pub(crate) mod sessions;
#[path = "native_wire.rs"]
mod wire;
#[path = "native_worlds.rs"]
mod worlds;
const SITES: [(usize, &[u8], usize); 6] = [
    (
        0x35f70,
        &[0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b],
        6,
    ),
    (
        0x34840,
        &[0x40, 0x56, 0x48, 0x83, 0xec, 0x50, 0x48, 0x8b],
        6,
    ),
    (
        0x4afc0,
        &[0x48, 0x89, 0x5c, 0x24, 0x18, 0x55, 0x56, 0x57],
        5,
    ),
    (
        0x36180,
        &[0x40, 0x53, 0x55, 0x57, 0x48, 0x81, 0xec, 0x90, 0x02, 0, 0],
        11,
    ),
    (
        0x39920,
        &[
            0x40, 0x55, 0x41, 0x56, 0x48, 0x8d, 0xac, 0x24, 0x58, 0xfe, 0xff, 0xff,
        ],
        12,
    ),
    (
        0x4182b0,
        &[0x40, 0x53, 0x57, 0x48, 0x81, 0xec, 0xd8, 0, 0, 0],
        10,
    ),
];
#[unsafe(no_mangle)]
static NATIVE_FRAME_CONTINUE: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];

#[repr(C)]
pub struct FrameTrace([AtomicU64; 16]);
static TRACE: FrameTrace = FrameTrace([
    AtomicU64::new(1),
    AtomicU64::new(128),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
]);
#[unsafe(no_mangle)]
pub static BRIDGE_RUNTIME_NATIVE_FRAME_TRACE: &FrameTrace = &TRACE;
#[unsafe(no_mangle)]
pub static BRIDGE_RUNTIME_NATIVE_FRAME_TRACE_SIZE: u64 = 128;
pub(super) fn diagnostic(index: usize) {
    TRACE.0[13].store(index as u64, Ordering::Release);
}
#[track_caller]
fn fault() -> ! {
    census::fault(1, std::panic::Location::caller().line());
    std::process::abort()
}

extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        let frames = FRAMES.get().unwrap_or_else(|| fault());
        raw.invocation.token = frames
            .enter_with_provenance(sessions::NativeSessionApi::unproven_input_active())
            .unwrap_or_else(|_| fault());
        raw.invocation.cookie = COOKIE;
        if !frames.healthy() {
            FAILED.store(true, Ordering::Release);
            TRACE.0[6].store(1, Ordering::Release);
            // A drain may already have returned to the destructive caller.
            // Do not race it by dereferencing old native receivers. This is
            // the same owned-process fatal boundary as a failed Close, never
            // a fabricated native return or a successful cancellation.
            fault();
        }
        TRACE.0[2].fetch_add(1, Ordering::Relaxed);
        TRACE.0[5].fetch_add(1, Ordering::AcqRel);
    }))
    .is_err()
    {
        fault();
    }
    unsafe { SetLastError(error) };
}
fn finish(raw: &mut Saved, exceptional: bool) {
    if exceptional {
        census::terminal(1, raw.invocation.kind);
    }
    if raw.invocation.cookie != COOKIE {
        return;
    }
    raw.invocation.cookie = 0;
    if FRAMES
        .get()
        .is_none_or(|f| f.leave(raw.invocation.token).is_err())
    {
        FAILED.store(true, Ordering::Release);
        TRACE.0[6].store(1, Ordering::Release);
    }
    TRACE.0[if exceptional { 4 } else { 3 }].fetch_add(1, Ordering::Relaxed);
    TRACE.0[5].fetch_sub(1, Ordering::AcqRel);
}
extern "system" fn end(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        finish(unsafe { &mut *raw }, false)
    }))
    .is_err()
    {
        FAILED.store(true, Ordering::Release);
    }
    unsafe { SetLastError(error) };
}
unsafe extern "system" fn unwind(
    record: *const EXCEPTION_RECORD,
    frame: usize,
    _: *mut CONTEXT,
    _: *mut c_void,
) -> i32 {
    let error = unsafe { GetLastError() };
    if !record.is_null()
        && unsafe { (*record).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0
        && std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            finish(unsafe { &mut *((frame + 0x20) as *mut Saved) }, true)
        }))
        .is_err()
    {
        FAILED.store(true, Ordering::Release);
    }
    unsafe { SetLastError(error) };
    1
}

global_asm!(
    ".globl native_frame_outer_original",
    ".seh_proc native_frame_outer_original",
    "native_frame_outer_original:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    ".globl native_frame_outer_body",
    "native_frame_outer_body:",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_FRAME_CONTINUE]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl native_frame_dispatch_original",
    ".seh_proc native_frame_dispatch_original",
    "native_frame_dispatch_original:",
    "push rsi",
    ".seh_pushreg rsi",
    "sub rsp,0x50",
    ".seh_stackalloc 0x50",
    ".seh_endprologue",
    ".globl native_frame_dispatch_body",
    "native_frame_dispatch_body:",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_FRAME_CONTINUE+8]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl native_frame_pump_original",
    ".seh_proc native_frame_pump_original",
    "native_frame_pump_original:",
    "mov [rsp+0x18],rbx",
    ".seh_savereg rbx,0x18",
    ".seh_endprologue",
    ".globl native_frame_pump_body",
    "native_frame_pump_body:",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_FRAME_CONTINUE+16]",
    "mov [rsp+8],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+8]",
    ".seh_endproc",
    ".globl native_frame_loop_original",
    ".seh_proc native_frame_loop_original",
    "native_frame_loop_original:",
    "push rbx",
    ".seh_pushreg rbx",
    "push rbp",
    ".seh_pushreg rbp",
    "push rdi",
    ".seh_pushreg rdi",
    "sub rsp,0x290",
    ".seh_stackalloc 0x290",
    ".seh_endprologue",
    ".globl native_frame_loop_body",
    "native_frame_loop_body:",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_FRAME_CONTINUE+24]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl native_frame_repeat_original",
    ".seh_proc native_frame_repeat_original",
    "native_frame_repeat_original:",
    "push rbp",
    ".seh_pushreg rbp",
    "push r14",
    ".seh_pushreg r14",
    "lea rbp,[rsp-0x1a8]",
    ".seh_endprologue",
    ".globl native_frame_repeat_body",
    "native_frame_repeat_body:",
    "mov [rsp+0x18],rax",
    "mov rax,[rip+NATIVE_FRAME_CONTINUE+32]",
    "mov [rsp+0x20],rax",
    "mov rax,[rsp+0x18]",
    "jmp qword ptr [rsp+0x20]",
    ".seh_endproc",
    ".globl native_frame_dispatch_owner_original",
    ".seh_proc native_frame_dispatch_owner_original",
    "native_frame_dispatch_owner_original:",
    "push rbx",
    ".seh_pushreg rbx",
    "push rdi",
    ".seh_pushreg rdi",
    "sub rsp,0xd8",
    ".seh_stackalloc 0xd8",
    ".seh_endprologue",
    ".globl native_frame_dispatch_owner_body",
    "native_frame_dispatch_owner_body:",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_FRAME_CONTINUE+40]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
);
unsafe extern "system" {
    fn native_frame_outer_original();
    fn native_frame_dispatch_original();
    fn native_frame_pump_original();
    fn native_frame_outer_gateway();
    fn native_frame_dispatch_gateway();
    fn native_frame_pump_gateway();
    fn native_frame_loop_original();
    fn native_frame_repeat_original();
    fn native_frame_dispatch_owner_original();
    fn native_frame_loop_gateway();
    fn native_frame_repeat_gateway();
    fn native_frame_dispatch_owner_gateway();
}
gateway!(
    "native_frame_outer_gateway",
    "10",
    native_frame_outer_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_frame_dispatch_gateway",
    "11",
    native_frame_dispatch_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_frame_pump_gateway",
    "12",
    native_frame_pump_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_frame_loop_gateway",
    "13",
    native_frame_loop_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_frame_repeat_gateway",
    "14",
    native_frame_repeat_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_frame_dispatch_owner_gateway",
    "15",
    native_frame_dispatch_owner_original,
    begin,
    end,
    unwind
);

fn gateways() -> [usize; 6] {
    [
        native_frame_outer_gateway as *const () as usize,
        native_frame_dispatch_gateway as *const () as usize,
        native_frame_pump_gateway as *const () as usize,
        native_frame_loop_gateway as *const () as usize,
        native_frame_repeat_gateway as *const () as usize,
        native_frame_dispatch_owner_gateway as *const () as usize,
    ]
}

/// Installation precedes factory execution. The runtime has already pinned
/// the exact Core/module and source file hash before reaching this function.
pub(in super::super) unsafe fn install(core: usize) -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "native frame owner poisoned")?;
    if !hooks.is_empty() || FRAMES.set(NativeFrames::new()).is_err() {
        return Err("native frames already owned");
    }
    TRACE.0[8].store(unsafe { GetCurrentProcessId() } as u64, Ordering::Release);
    TRACE.0[9].store(
        current_created_filetime().ok_or("native frame process identity")?,
        Ordering::Release,
    );
    #[cfg(not(test))]
    unsafe {
        scheduler::bind_core(core)?;
        jobs::install(core)?;
        consumers::install(core)?;
        worlds::install(core)?;
        control::install(core)?;
        operations::install(core)?;
    }
    for (rva, expected, _) in SITES {
        if !expected
            .iter()
            .enumerate()
            .all(|(j, b)| copy_current::<1>((core + rva + j) as u64) == Some([*b]))
        {
            return Err("native frame entry differs");
        }
    }
    let callbacks = gateways();
    for (i, (rva, expected, displaced)) in SITES.into_iter().enumerate() {
        let expected: [u8; 8] = expected[..8].try_into().unwrap();
        let entry = (core + rva) as *mut u8;
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(entry.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
            || !entry.addr().is_multiple_of(8)
            || info.State != MEM_COMMIT
            || info.Protect & PAGE_GUARD != 0
            || info.Protect & 0xff != PAGE_EXECUTE_READ
        {
            return Err("native frame entry protection/alignment");
        }
        let code = unsafe { executable_near(entry as usize, entry as usize, displaced) }
            .ok_or("native frame relay unavailable")?;
        let mut unpublished = Unpublished(code);
        unsafe {
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), code, 6);
            ptr::copy_nonoverlapping((callbacks[i] as u64).to_le_bytes().as_ptr(), code.add(6), 8);
        }
        let mut old = 0;
        if unsafe { VirtualProtect(code.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
            || unsafe { FlushInstructionCache(GetCurrentProcess(), code.cast(), 14) } == 0
        {
            return Err("native frame relay publication");
        }
        let mut patched = expected;
        patched[..5].copy_from_slice(
            &rel32_jump(entry as usize, code as usize).ok_or("native frame relay range")?,
        );
        // Preserve suffix bytes when the original resumes inside the CAS
        // word; for larger whole instructions only the first 8 bytes change.
        patched[5..displaced.min(8)].fill(0x90);
        NATIVE_FRAME_CONTINUE[i].store((core + rva + displaced) as u64, Ordering::Release);
        hooks.push(OwnedHook {
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
        let result = unsafe {
            exchange_entry(
                entry,
                u64::from_le_bytes(expected),
                u64::from_le_bytes(patched),
            )
        }?;
        if !result.exchanged
            || !result.protection_restored
            || !result.cache_flushed
            || copy_current::<8>(entry as u64) != Some(patched)
        {
            return Err("critical: native frame publication unverified");
        }
        TRACE.0[7].fetch_or(1 << i, Ordering::Release);
    }
    census::installed();
    Ok(())
}

pub(in super::super) fn close(deadline: Instant) -> bool {
    #[cfg(test)]
    if FRAMES.get().is_none() {
        return true;
    } // existing process-local bridge-only fixtures
    let success = !FAILED.load(Ordering::Acquire)
        && FRAMES
            .get()
            .is_some_and(|f| f.seal_and_wait(deadline).is_ok());
    TRACE.0[10].fetch_add(1, Ordering::AcqRel);
    if success {
        TRACE.0[11].fetch_add(1, Ordering::Release);
    }
    if !success {
        TRACE.0[6].store(1, Ordering::Release);
    }
    success
}
pub(super) fn close_executions(deadline: Instant) -> bool {
    if FAILED.load(Ordering::Acquire) {
        return false;
    }
    let Some(frames) = FRAMES.get() else {
        return false;
    };
    let result = frames.seal_executions_and_wait(deadline);
    #[cfg(test)]
    if result == Err(crate::native_flight::Error::Reentrant) && counts()[3] == 6 {
        println!("SIX_NATIVE_FRAMES_REENTRANT_AT_WAIT");
        use std::io::Write;
        std::io::stdout().flush().unwrap();
    }
    !FAILED.load(Ordering::Acquire) && result.is_ok()
}
pub(in super::super) fn unproven_input_active() -> bool {
    sessions::NativeSessionApi::unproven_input_active()
        || FRAMES
            .get()
            .is_some_and(NativeFrames::unproven_on_current_thread)
}
pub(in super::super) fn enter_bridge() -> Result<Option<u64>, ()> {
    let Some(frames) = FRAMES.get() else {
        #[cfg(test)]
        return Ok(None);
        #[cfg(not(test))]
        return Err(());
    };
    let id = frames.enter().map_err(|_| ())?;
    if frames.healthy() {
        Ok(Some(id))
    } else {
        let _ = frames.leave(id);
        Err(())
    }
}
pub(in super::super) fn leave_bridge(hold: Option<u64>) -> bool {
    hold.is_none_or(|id| FRAMES.get().is_some_and(|frames| frames.leave(id).is_ok()))
}
fn session_enter(epoch: u64) -> Result<u64, sessions::SessionError> {
    FRAMES
        .get()
        .ok_or(sessions::SessionError::Unbound)?
        .enter_existing_epoch(epoch)
        .map_err(|_| sessions::SessionError::Stale)
}
fn session_leave(hold: u64, exceptional: bool) -> Result<(), sessions::SessionError> {
    let f = FRAMES.get().ok_or(sessions::SessionError::Unbound)?;
    if exceptional {
        f.mark_unresolved();
    }
    f.leave(hold)
        .map_err(|_| sessions::SessionError::Unresolved)
}
pub(in super::super) unsafe fn prepare_host(exe: usize) -> Result<(), &'static str> {
    TRACE.0[8].store(unsafe { GetCurrentProcessId() } as u64, Ordering::Release);
    TRACE.0[9].store(
        current_created_filetime().ok_or("native Host process identity")?,
        Ordering::Release,
    );
    census::identity(
        TRACE.0[8].load(Ordering::Acquire),
        TRACE.0[9].load(Ordering::Acquire),
    );
    unsafe {
        diagnostic(1001);
        sessions::NativeSessionApi::prepare_host(exe)?;
        diagnostic(1002);
        loads::prepare_host(exe)?;
        diagnostic(1003);
        wire::prepare_host(exe)?;
        diagnostic(1004);
        jobs::prepare_host(exe)?;
        diagnostic(1005);
        consumers::prepare_host(exe)?;
        diagnostic(1006);
        scheduler::prepare_host(exe)?;
    }
    diagnostic(1007);
    census::prepared();
    HOST_PREPARED.store(true, Ordering::Release);
    Ok(())
}
pub(in super::super) fn host_prepared() -> bool {
    HOST_PREPARED.load(Ordering::Acquire)
}
pub(in super::super) fn bind_host(core: usize) -> bool {
    sessions::NativeSessionApi::bind_core(
        core as u64,
        sessions::SessionScopeHooks {
            enter_existing_epoch: session_enter,
            leave: session_leave,
        },
    )
    .is_ok()
}
pub(in super::super) fn request_close() -> bool {
    #[cfg(test)]
    if FRAMES.get().is_none() {
        return true;
    }
    control::request_close();
    !FAILED.load(Ordering::Acquire) && FRAMES.get().is_some_and(|f| f.request_close().is_ok())
}
pub(in super::super) fn reopen() -> bool {
    #[cfg(test)]
    if FRAMES.get().is_none() {
        return true;
    }
    let success = !FAILED.load(Ordering::Acquire)
        && control::ready()
        && FRAMES.get().is_some_and(|f| f.reopen().is_ok());
    if success {
        control::reopened();
        TRACE.0[12].fetch_add(1, Ordering::Release);
    }
    success
}

/// Resume native bookkeeping with product permission still closed. This is
/// not a qualified Rebuild and never increments the certified reopen count.
pub(in super::super) fn resume_unqualified_observation() -> bool {
    resume_observation_with_owned_entries(all_hook_entries_owned())
}
fn resume_observation_with_owned_entries(owned: bool) -> bool {
    if !owned
        || !runtime::inactive_and_drained()
        || FAILED.load(Ordering::Acquire)
        || !control::observed_shutdown_complete()
        || FRAMES.get().is_none_or(|frames| frames.reopen().is_err())
    {
        return false;
    }
    control::reopened();
    census::observation_resumed();
    diagnostic(2001);
    true
}

#[cfg(test)]
pub(super) fn fixture(index: usize, continuation: usize) -> usize {
    FRAMES.get_or_init(NativeFrames::new);
    NATIVE_FRAME_CONTINUE[index].store(continuation as u64, Ordering::Release);
    gateways()[index]
}
#[cfg(test)]
pub(super) fn counts() -> [u64; 5] {
    std::array::from_fn(|i| TRACE.0[i + 2].load(Ordering::Acquire))
}
#[cfg(test)]
pub(in super::super) unsafe fn fixture_host_jobs(exe: usize) -> Result<(), &'static str> {
    unsafe { jobs::fixture_host_install(exe) }
}
#[cfg(test)]
pub(in super::super) fn fixture_host_job_sites() -> [(usize, [u8; 8], usize); 4] {
    jobs::fixture_host_sites()
}

/// Restore every owned entry, retaining each relay for already fetched jumps.
/// Failed compares retain responsibility and do not prevent later restores.
pub(in super::super) unsafe fn restore() -> Result<(), &'static str> {
    census::restored();
    let mut hooks = HOOKS.lock().map_err(|_| "native frame owner poisoned")?;
    let mut failed = unsafe { jobs::restore() }.is_err();
    failed |= unsafe { consumers::restore() }.is_err();
    failed |= unsafe { scheduler::restore() }.is_err();
    failed |= unsafe { worlds::restore() }.is_err();
    failed |= unsafe { control::restore() }.is_err();
    failed |= unsafe { operations::restore() }.is_err();
    failed |= unsafe { loads::restore() }.is_err();
    failed |= unsafe { wire::restore() }.is_err();
    failed |= unsafe { sessions::NativeSessionApi::restore() }.is_err();
    hooks.retain(|owned| {
        let hook = &owned.hook;
        let current = copy_current::<8>(hook.target as u64);
        let restored = current == Some(hook.original)
            || (current == Some(hook.patched)
                && unsafe {
                    exchange_entry(
                        hook.target,
                        u64::from_le_bytes(hook.patched),
                        u64::from_le_bytes(hook.original),
                    )
                }
                .is_ok_and(|r| r.exchanged && r.protection_restored && r.cache_flushed)
                && copy_current::<8>(hook.target as u64) == Some(hook.original));
        let mut old = 0;
        let protected =
            unsafe { VirtualProtect(hook.target.cast(), 8, owned.protection, &mut old) } != 0;
        let flushed =
            unsafe { FlushInstructionCache(GetCurrentProcess(), hook.target.cast(), 8) } != 0;
        let success = restored && protected && flushed;
        failed |= !success;
        !success
    });
    if failed {
        Err("critical: native frame restoration unverified")
    } else {
        HOST_PREPARED.store(false, Ordering::Release);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    #[test]
    fn retained_hook_ownership_refuses_empty_partial_restored_and_changed_entries() {
        let mut words = [0x1122u64, 0x3344];
        let owners = Mutex::new(
            words
                .iter_mut()
                .map(|word| OwnedHook {
                    hook: Hook {
                        target: (word as *mut u64).cast(),
                        original: 0u64.to_le_bytes(),
                        patched: word.to_le_bytes(),
                        prefix: None,
                        _code: null_mut(),
                    },
                    protection: PAGE_READWRITE,
                })
                .collect(),
        );
        assert!(owns_hook_entries(&owners, 2));
        assert!(!owns_hook_entries(&owners, 0));
        assert!(!owns_hook_entries(&owners, 1));
        unsafe { ptr::write_volatile(words.as_mut_ptr().add(1), 0x7788) };
        assert!(!owns_hook_entries(&owners, 2));
        unsafe { ptr::write_volatile(words.as_mut_ptr().add(1), 0x3344) };
        assert!(owns_hook_entries(&owners, 2));
        unsafe { ptr::write_volatile(words.as_mut_ptr().add(1), 0) };
        assert!(
            !owns_hook_entries(&owners, 2),
            "restored original is not an owned patch"
        );
        owners.lock().unwrap().pop();
        assert!(!owns_hook_entries(&owners, 2));
        owners.lock().unwrap().clear();
        assert!(!owns_hook_entries(&owners, 2));
    }
    #[test]
    fn six_real_entry_exchanges_restore_original_bytes_and_keep_frame_ownership() {
        const ENV: &str = "CIV6_NATIVE_FRAMES_CAS_CHILD";
        if std::env::var_os(ENV).is_none() {
            let mut child=std::process::Command::new(std::env::current_exe().unwrap()).creation_flags(0x08000000)
                .env(ENV,"1").args(["--exact","windows::root_observation::native_frames::tests::six_real_entry_exchanges_restore_original_bytes_and_keep_frame_ownership","--nocapture"])
                .spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success());
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("owned native CAS child timed out");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            return;
        }
        if !supports_atomic_promotion_fetch() {
            return;
        }
        let base = unsafe {
            VirtualAlloc(
                null_mut(),
                0x420000,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        } as usize;
        assert_ne!(base, 0);
        // Every original body is owned fixture memory. No game code runs.
        let tails: [&[u8]; 6] = [
            &[0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3],
            &[0x48, 0x83, 0xc4, 0x50, 0x5e, 0xc3],
            &[0x5f, 0x5e, 0x5d, 0x48, 0x8b, 0x5c, 0x24, 0x18, 0xc3],
            &[0x48, 0x81, 0xc4, 0x90, 2, 0, 0, 0x5f, 0x5d, 0x5b, 0xc3],
            &[0x41, 0x5e, 0x5d, 0xc3],
            &[0x48, 0x81, 0xc4, 0xd8, 0, 0, 0, 0x5f, 0x5b, 0xc3],
        ];
        let mut originals = Vec::new();
        for (i, (rva, prefix, displaced)) in SITES.into_iter().enumerate() {
            let mut bytes = prefix[..displaced].to_vec();
            if i < 2 {
                bytes.extend([0x48, 0x8b, if i == 0 { 0xd9 } else { 0xf1 }]);
            }
            if i == 2 {
                bytes.extend([0x55, 0x56, 0x57]);
            }
            bytes.extend([0x48, 0xff, 0x01, 0xb8, 0x22, 0x11, 0, 0]);
            bytes.extend(tails[i]);
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), (base + rva) as *mut u8, bytes.len());
            }
            originals.push(bytes);
        }
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(base as _, 0x420000, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        assert_ne!(
            unsafe { FlushInstructionCache(GetCurrentProcess(), base as _, 0x420000) },
            0
        );
        unsafe { install(base) }.unwrap();
        assert_eq!(TRACE.0[7].load(Ordering::Acquire), 63);
        for (i, (rva, _, _)) in SITES.into_iter().enumerate() {
            let invoke: unsafe extern "system" fn(*mut u64) -> u64 =
                unsafe { std::mem::transmute(base + rva) };
            let mut n = 0;
            assert_eq!(unsafe { invoke(&mut n) }, 0x1122);
            assert_eq!(n, 1, "site{i}");
        }
        assert_eq!(counts(), [6, 6, 0, 0, 0]);
        assert!(close(Instant::now() + Duration::from_secs(2)));
        assert!(reopen());
        unsafe { restore() }.unwrap();
        for (i, (rva, _, _)) in SITES.into_iter().enumerate() {
            let restored = unsafe {
                std::slice::from_raw_parts((base + rva) as *const u8, originals[i].len())
            };
            assert_eq!(restored, originals[i]);
        }
        assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
    }
}
