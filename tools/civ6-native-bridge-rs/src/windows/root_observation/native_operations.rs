//! Serialized operation queue reservations, separate from Host job-set work.
//! Whole original calls execute once. Queue node identity is bound before the
//! original enqueue links it, and inherited at the exact dequeue call site.
use super::scheduler::{FixedFrame, Fragment};
use super::*;
use std::cell::RefCell;
#[path = "native_history_clear_fixed.rs"]
mod history_clear;
static CORE: AtomicU64 = AtomicU64::new(0);
#[unsafe(no_mangle)]
static NATIVE_OPERATION_FLAG: AtomicU64 = AtomicU64::new(0);
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
pub(super) fn hook_entries_owned() -> bool {
    owns_hook_entries(&HOOKS, sites().len())
}

#[unsafe(no_mangle)]
static NATIVE_OPERATION_CONTINUE: [AtomicU64; 10] = [const { AtomicU64::new(0) }; 10];
thread_local! {static STACK:RefCell<Vec<(u64,u64)>>=const {RefCell::new(Vec::new())};}
type Site = (usize, [u8; 8], usize, usize);
#[track_caller]
fn fault() -> ! {
    census::fault(
        census::module(module_path!()),
        std::panic::Location::caller().line(),
    );
    diagnostic(40);
    FAILED.store(true, Ordering::Release);
    TRACE.0[6].store(1, Ordering::Release);
    std::process::abort()
}
extern "system" fn begin(raw: *mut Saved) {
    let e = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        let f = FRAMES.get().unwrap_or_else(|| fault());
        // Native input without a World receipt must keep executing. Retain
        // its queue work as explicitly unproven instead of adopting this
        // World's epoch or turning the original enqueue into an abort.
        let unproven = super::unproven_input_active();
        let core = CORE.load(Ordering::Acquire);
        let id = match raw.invocation.kind {
            48 if control::owns_operation_cleanup(raw.invocation.caller) => {
                raw.invocation.errors = 2;
                f.begin_native_cleanup().unwrap_or_else(|_| fault())
            }
            49 if control::owns_inert_cleanup() => {
                raw.invocation.errors = 2;
                f.begin_native_cleanup().unwrap_or_else(|_| fault())
            }
            40 | 43 | 44 | 46 | 47 | 48 | 49 => f
                .begin_queue_producer_with_provenance(unproven)
                .unwrap_or_else(|_| fault()),
            41 => {
                let caller = raw.invocation.caller.wrapping_sub(core);
                let producer_kind = match caller {
                    0x3743a | 0x374ce => 40,
                    0x377aa | 0x37980 => 43,
                    0x3657d => 44,
                    _ => return,
                };
                let Some((kind, id)) = STACK.with(|s| s.borrow().last().copied()) else {
                    fault();
                };
                if kind != producer_kind {
                    fault();
                }
                if kind == 44 {
                    raw.invocation.world = raw.gpr[7]; // current source RBX
                    raw.invocation.errors = 1;
                }
                id
            }
            45 => {
                let caller = raw.invocation.caller.wrapping_sub(core);
                if caller == 0x369b4 {
                    if STACK.with(|s| s.borrow().last().is_none_or(|(kind, _)| *kind != 49)) {
                        // A node free without the containing clear frame is
                        // not a full lifetime receipt. Preserve the original
                        // call, deny product authority, and do not certify it.
                        f.mark_unresolved();
                        FAILED.store(true, Ordering::Release);
                        TRACE.0[6].store(1, Ordering::Release);
                        return;
                    }
                } else if !matches!(caller, 0x373a2 | 0x38024 | 0x381a4 | 0x38204) {
                    return;
                }
                f.retire_queue(raw.gpr[1]).unwrap_or_else(|_| fault())
            }
            42 => {
                if raw.invocation.caller == core + 0x37358 {
                    f.queue_execute(raw.gpr[7]).unwrap_or_else(|_| fault())
                } else {
                    raw.invocation.errors = 1;
                    f.begin_queue_producer_with_provenance(unproven)
                        .unwrap_or_else(|_| fault())
                }
            }
            _ => fault(),
        };
        raw.invocation.cookie = COOKIE;
        raw.invocation.token = id;
        STACK.with(|s| s.borrow_mut().push((raw.invocation.kind, id)));
    }))
    .is_err()
    {
        fault();
    }
    unsafe { SetLastError(e) };
}
fn finish(raw: &mut Saved, exceptional: bool) {
    if exceptional {
        census::terminal(census::module(module_path!()), raw.invocation.kind);
    }
    if raw.invocation.cookie != COOKIE {
        return;
    }
    raw.invocation.cookie = 0;
    let f = FRAMES.get().unwrap_or_else(|| fault());
    if STACK.with(|s| s.borrow_mut().pop()) != Some((raw.invocation.kind, raw.invocation.token)) {
        fault();
    }
    if exceptional {
        f.mark_unresolved();
        FAILED.store(true, Ordering::Release);
        return;
    }
    match raw.invocation.kind {
        48 | 49 if raw.invocation.errors == 2 => {
            f.leave(raw.invocation.token).unwrap_or_else(|_| fault())
        }
        40 | 43 | 44 | 46 | 47 | 48 | 49 => f
            .queue_producer_returned(raw.invocation.token)
            .unwrap_or_else(|_| fault()),
        41 => {
            raw.invocation.set = f
                .queue_allocated(
                    raw.invocation.token,
                    raw.gpr[4],
                    (raw.invocation.errors == 1).then_some(raw.invocation.world),
                )
                .unwrap_or_else(|_| fault());
        }
        45 => f
            .queue_retired(raw.invocation.token)
            .unwrap_or_else(|_| fault()),
        42 => {
            if raw.invocation.errors == 1 {
                f.leave(raw.invocation.token).unwrap_or_else(|_| fault())
            } else {
                f.completed(raw.invocation.token)
                    .unwrap_or_else(|_| fault())
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
    ".globl native_operation_enqueue_original",
    ".seh_proc native_operation_enqueue_original",
    "native_operation_enqueue_original:",
    "push rbp",
    ".seh_pushreg rbp",
    "push rsi",
    ".seh_pushreg rsi",
    "push rdi",
    ".seh_pushreg rdi",
    "push r14",
    ".seh_pushreg r14",
    ".seh_endprologue",
    ".globl native_operation_enqueue_body",
    "native_operation_enqueue_body:",
    "mov [rsp+0x28],rax",
    "mov rax,[rip+NATIVE_OPERATION_CONTINUE]",
    "mov [rsp+0x30],rax",
    "mov rax,[rsp+0x28]",
    "jmp qword ptr [rsp+0x30]",
    ".seh_endproc",
    ".globl native_operation_allocate_original",
    ".seh_proc native_operation_allocate_original",
    "native_operation_allocate_original:",
    "mov r8d,[rsp+0x30]",
    ".seh_endprologue",
    "mov [rsp+8],rax",
    "mov rax,[rip+NATIVE_OPERATION_CONTINUE+8]",
    "mov [rsp+0x10],rax",
    "mov rax,[rsp+8]",
    "jmp qword ptr [rsp+0x10]",
    ".seh_endproc",
    ".globl native_operation_execute_original",
    ".seh_proc native_operation_execute_original",
    "native_operation_execute_original:",
    "push rbp",
    ".seh_pushreg rbp",
    "push rbx",
    ".seh_pushreg rbx",
    "push r12",
    ".seh_pushreg r12",
    "push r13",
    ".seh_pushreg r13",
    ".seh_endprologue",
    ".globl native_operation_execute_body",
    "native_operation_execute_body:",
    "mov [rsp+0x28],rax",
    "mov rax,[rip+NATIVE_OPERATION_CONTINUE+16]",
    "mov [rsp+0x30],rax",
    "mov rax,[rsp+0x28]",
    "jmp qword ptr [rsp+0x30]",
    ".seh_endproc",
    ".globl native_operation_replay_original",
    ".seh_proc native_operation_replay_original",
    "native_operation_replay_original:",
    "push rbp",
    ".seh_pushreg rbp",
    "push r13",
    ".seh_pushreg r13",
    "push r14",
    ".seh_pushreg r14",
    ".seh_endprologue",
    "mov [rsp+0x20],rax",
    "mov rax,[rip+NATIVE_OPERATION_CONTINUE+24]",
    "mov [rsp+0x28],rax",
    "mov rax,[rsp+0x20]",
    "jmp qword ptr [rsp+0x28]",
    ".seh_endproc",
    ".globl native_operation_copy_original",
    ".seh_proc native_operation_copy_original",
    "native_operation_copy_original:",
    "push rbp",
    ".seh_pushreg rbp",
    "push r12",
    ".seh_pushreg r12",
    "push r15",
    ".seh_pushreg r15",
    ".seh_endprologue",
    "mov [rsp+0x20],rax",
    "mov rax,[rip+NATIVE_OPERATION_CONTINUE+32]",
    "mov [rsp+0x28],rax",
    "mov rax,[rsp+0x20]",
    "jmp qword ptr [rsp+0x28]",
    ".seh_endproc",
    ".globl native_operation_free_original",
    ".seh_proc native_operation_free_original",
    "native_operation_free_original:",
    ".seh_endprologue",
    "mov rcx,rdx",
    "jmp qword ptr [rip+NATIVE_OPERATION_CONTINUE+40]",
    ".seh_endproc",
    ".globl native_operation_drain_original",
    ".seh_proc native_operation_drain_original",
    "native_operation_drain_original:",
    "mov [rsp+8],rbx",
    ".seh_savereg rbx,8",
    ".seh_endprologue",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_OPERATION_CONTINUE+48]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl native_operation_transfer_original",
    ".seh_proc native_operation_transfer_original",
    "native_operation_transfer_original:",
    "sub rsp,0x48",
    ".seh_stackalloc 0x48",
    ".seh_endprologue",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_OPERATION_CONTINUE+56]",
    "mov [rsp+0x18],rax",
    // cmp cl,[rip+...] is relocated without changing its flags or inputs.
    "mov rax,[rip+NATIVE_OPERATION_FLAG]",
    "cmp cl,byte ptr [rax]",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl native_operation_cleanup_original",
    ".seh_proc native_operation_cleanup_original",
    "native_operation_cleanup_original:",
    "mov [rsp+8],rbx",
    ".seh_savereg rbx,8",
    ".seh_endprologue",
    "mov [rsp+0x10],rax",
    "mov rax,[rip+NATIVE_OPERATION_CONTINUE+64]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+0x10]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
    ".globl native_operation_history_clear_original",
    ".seh_proc native_operation_history_clear_original",
    "native_operation_history_clear_original:",
    "mov [rsp+0x10],rbx",
    ".seh_savereg rbx,0x10",
    ".seh_endprologue",
    "mov [rsp+8],rax",
    "mov rax,[rip+NATIVE_OPERATION_CONTINUE+72]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+8]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
);
unsafe extern "system" {
    fn native_operation_enqueue_original();
    fn native_operation_allocate_original();
    fn native_operation_execute_original();
    fn native_operation_enqueue_gateway();
    fn native_operation_allocate_gateway();
    fn native_operation_execute_gateway();
    fn native_operation_replay_original();
    fn native_operation_copy_original();
    fn native_operation_free_original();
    fn native_operation_replay_gateway();
    fn native_operation_copy_gateway();
    fn native_operation_free_gateway();
    fn native_operation_drain_original();
    fn native_operation_transfer_original();
    fn native_operation_cleanup_original();
    fn native_operation_drain_gateway();
    fn native_operation_transfer_gateway();
    fn native_operation_cleanup_gateway();
    fn native_operation_history_clear_original();
    fn native_operation_history_clear_gateway();
}
gateway!(
    "native_operation_enqueue_gateway",
    "40",
    native_operation_enqueue_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_operation_allocate_gateway",
    "41",
    native_operation_allocate_original,
    begin,
    end,
    unwind,
    "mov rax,[rsp+0x1f0]\nmov [rsp+0x20],rax\nmov rax,[rsp+0x1f8]\nmov [rsp+0x28],rax\nmov rax,[rsp+0x40]\n"
);
gateway!(
    "native_operation_execute_gateway",
    "42",
    native_operation_execute_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_operation_replay_gateway",
    "43",
    native_operation_replay_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_operation_copy_gateway",
    "44",
    native_operation_copy_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_operation_free_gateway",
    "45",
    native_operation_free_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_operation_drain_gateway",
    "46",
    native_operation_drain_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_operation_transfer_gateway",
    "47",
    native_operation_transfer_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_operation_cleanup_gateway",
    "48",
    native_operation_cleanup_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_operation_history_clear_gateway",
    "49",
    native_operation_history_clear_original,
    begin,
    end,
    unwind
);
fn sites() -> [Site; 10] {
    [
        (
            0x373e0,
            [0x40, 0x55, 0x56, 0x57, 0x41, 0x56, 0x48, 0x8d],
            6,
            native_operation_enqueue_gateway as *const () as usize,
        ),
        (
            0x994970,
            [0x44, 0x8b, 0x44, 0x24, 0x30, 0x41, 0xb9, 0x16],
            5,
            native_operation_allocate_gateway as *const () as usize,
        ),
        (
            0x37b50,
            [0x40, 0x55, 0x53, 0x41, 0x54, 0x41, 0x55, 0x41],
            7,
            native_operation_execute_gateway as *const () as usize,
        ),
        (
            0x37690,
            [0x40, 0x55, 0x41, 0x55, 0x41, 0x56, 0x48, 0x8d],
            6,
            native_operation_replay_gateway as *const () as usize,
        ),
        (
            0x36520,
            [0x40, 0x55, 0x41, 0x54, 0x41, 0x57, 0x48, 0x83],
            6,
            native_operation_copy_gateway as *const () as usize,
        ),
        (
            0x36e20,
            [0x48, 0x8b, 0xca, 0xe9, 0xc8, 0x9a, 0x95, 0],
            8,
            native_operation_free_gateway as *const () as usize,
        ),
        (
            0x372f0,
            [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x74],
            5,
            native_operation_drain_gateway as *const () as usize,
        ),
        (
            0x37f70,
            [0x48, 0x83, 0xec, 0x48, 0x3a, 0x0d, 0xa0, 0x2a],
            10,
            native_operation_transfer_gateway as *const () as usize,
        ),
        (
            0x380b0,
            [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x74],
            5,
            native_operation_cleanup_gateway as *const () as usize,
        ),
        (
            0x36950,
            [0x48, 0x89, 0x5c, 0x24, 0x10, 0x56, 0x48, 0x83],
            5,
            native_operation_history_clear_gateway as *const () as usize,
        ),
    ]
}
pub(super) unsafe fn install(core: usize) -> Result<(), &'static str> {
    unsafe {
        scheduler::verify_frames(core, history_clear::FRAMES)?;
        install_at(core)
    }
}
unsafe fn install_at(core: usize) -> Result<(), &'static str> {
    CORE.store(core as u64, Ordering::Release);
    NATIVE_OPERATION_FLAG.store((core + 0xb8aa1a) as u64, Ordering::Release);
    if copy_current::<10>((core + 0x37f70) as u64)
        != Some([0x48, 0x83, 0xec, 0x48, 0x3a, 0x0d, 0xa0, 0x2a, 0xb5, 0])
    {
        return Err("operation transfer comparison differs");
    }
    let mut o = HOOKS.lock().map_err(|_| "operation owner poisoned")?;
    for (i, (rva, expected, n, gateway)) in sites().into_iter().enumerate() {
        let p = (core + rva) as *mut u8;
        if copy_current::<8>(p as u64) != Some(expected) {
            return Err("operation entry differs");
        }
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(p.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
            || info.Protect & 0xff != PAGE_EXECUTE_READ
        {
            return Err("operation protection");
        }
        let code = unsafe { executable_near(p as usize, p as usize, n) }
            .ok_or("operation relay unavailable")?;
        let mut unpublished = Unpublished(code);
        unsafe {
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), code, 6);
            ptr::copy_nonoverlapping((gateway as u64).to_le_bytes().as_ptr(), code.add(6), 8);
        }
        let mut old = 0;
        if unsafe { VirtualProtect(code.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
            || unsafe { FlushInstructionCache(GetCurrentProcess(), code.cast(), 14) } == 0
        {
            return Err("operation relay publication");
        }
        let mut patch = expected;
        patch[..5]
            .copy_from_slice(&rel32_jump(p as usize, code as usize).ok_or("operation range")?);
        patch[5..n.min(8)].fill(0x90);
        NATIVE_OPERATION_CONTINUE[i].store(
            (core + if i == 5 { 0x9908f0 } else { rva + n }) as u64,
            Ordering::Release,
        );
        o.push(OwnedHook {
            hook: Hook {
                target: p,
                original: expected,
                patched: patch,
                prefix: None,
                _code: code,
            },
            protection: info.Protect,
        });
        unpublished.0 = null_mut();
        let r =
            unsafe { exchange_entry(p, u64::from_le_bytes(expected), u64::from_le_bytes(patch)) }?;
        if !r.exchanged || !r.cache_flushed || !r.protection_restored {
            return Err("critical: operation publication");
        }
    }
    Ok(())
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut o = HOOKS.lock().map_err(|_| "operation owner poisoned")?;
    o.retain(|x| {
        let h = &x.hook;
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
        !(ok && unsafe { VirtualProtect(h.target.cast(), 8, x.protection, &mut old) } != 0)
    });
    if o.is_empty() {
        Ok(())
    } else {
        Err("critical: operation restore")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    fn child(name: &str) -> bool {
        const ENV: &str = "CIV6_NATIVE_OPERATION_OWNED_CHILD";
        if std::env::var_os(ENV).is_some() {
            return true;
        }
        let mut p = std::process::Command::new(std::env::current_exe().unwrap())
            .creation_flags(0x08000000)
            .env(ENV, "1")
            .args([
                "--exact",
                &format!("windows::root_observation::native_frames::operations::tests::{name}"),
                "--nocapture",
            ])
            .stdout(std::process::Stdio::piped())
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
                panic!("native operation child timeout");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = p.wait_with_output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("running 1 test") && stdout.contains(name),
            "owned child did not execute the selected test: {stdout}"
        );
        false
    }
    #[test]
    fn original_enqueue_allocator_dequeue_identity_is_kept_across_close() {
        if !child("original_enqueue_allocator_dequeue_identity_is_kept_across_close") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        CORE.store(0x180000000, Ordering::Release);
        let _install: unsafe fn(usize) -> Result<(), &'static str> = install;
        let mut enqueue: Saved = unsafe { std::mem::zeroed() };
        enqueue.invocation.kind = 40;
        begin(&mut enqueue);
        let mut allocate: Saved = unsafe { std::mem::zeroed() };
        allocate.invocation.kind = 41;
        allocate.invocation.caller = 0x18003743a;
        begin(&mut allocate);
        allocate.gpr[4] = 123;
        finish(&mut allocate, false);
        let id = allocate.invocation.set;
        finish(&mut enqueue, false);
        let f = FRAMES.get().unwrap();
        assert!(f.contains_task(123));
        assert_eq!(
            f.seal_and_wait(Instant::now()),
            Err(crate::native_flight::Error::Timeout)
        );
        let mut execute: Saved = unsafe { std::mem::zeroed() };
        execute.invocation.kind = 42;
        execute.invocation.caller = 0x180037358;
        execute.gpr[7] = 123;
        begin(&mut execute);
        assert_eq!(execute.invocation.token, id);
        finish(&mut execute, false);
        let mut free: Saved = unsafe { std::mem::zeroed() };
        free.invocation.kind = 45;
        free.invocation.caller = 0x1800373a2;
        free.gpr[1] = 123;
        begin(&mut free);
        finish(&mut free, false);
        assert!(!f.contains_task(123));
        f.seal_and_wait(Instant::now() + Duration::from_secs(2))
            .unwrap();
    }

    fn saved(kind: u64, caller: u64) -> Saved {
        let mut s: Saved = unsafe { std::mem::zeroed() };
        s.invocation.kind = kind;
        s.invocation.caller = 0x180000000 + caller;
        s
    }
    fn allocation(caller: u64, node: u64, source: u64) -> u64 {
        let mut a = saved(41, caller);
        a.gpr[7] = source;
        begin(&mut a);
        a.gpr[4] = node;
        finish(&mut a, false);
        a.invocation.set
    }
    fn retire(caller: u64, node: u64) {
        let mut r = saved(45, caller);
        r.gpr[1] = node;
        begin(&mut r);
        finish(&mut r, false);
    }
    #[test]
    fn history_clear_keeps_its_outer_frame_and_retires_only_history_nodes() {
        if !child("history_clear_keeps_its_outer_frame_and_retires_only_history_nodes") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        CORE.store(0x180000000, Ordering::Release);
        let f = FRAMES.get().unwrap();
        let mut enqueue = saved(40, 0);
        begin(&mut enqueue);
        allocation(0x3743a, 100, 0);
        allocation(0x374ce, 200, 0);
        finish(&mut enqueue, false);
        let mut clear = saved(49, 0);
        begin(&mut clear);
        // 3699F frees payload storage. It must not settle the node's receipt.
        retire(0x3699f, 300);
        assert!(f.contains_task(200));
        let mut node = saved(45, 0x369b4);
        node.gpr[1] = 200;
        begin(&mut node);
        assert_eq!(node.invocation.cookie, COOKIE);
        assert!(!f.contains_task(200));
        assert_eq!(
            f.seal_and_wait(Instant::now()),
            Err(crate::native_flight::Error::Reentrant)
        );
        finish(&mut node, false);
        // The original clear still has to update the native sentinel links.
        assert_eq!(
            f.seal_and_wait(Instant::now()),
            Err(crate::native_flight::Error::Reentrant)
        );
        finish(&mut clear, false);
        assert!(f.contains_task(100), "the first queue is independent");
        retire(0x373a2, 100);
        f.seal_and_wait(Instant::now() + Duration::from_secs(1))
            .unwrap();
        assert!(STACK.with(|s| s.borrow().is_empty()));
        assert!(!FAILED.load(Ordering::Acquire));
    }
    static HISTORY_FREES: AtomicU64 = AtomicU64::new(0);
    unsafe extern "system" fn history_original_free(node: u64) -> u64 {
        let frames = FRAMES.get().unwrap();
        assert!(
            !frames.contains_task(node),
            "opaque node lookup retires before native free"
        );
        assert_eq!(
            frames.pending_on_set(u64::MAX),
            Ok(1),
            "free return still owns the reservation"
        );
        assert!(STACK.with(|s| s.borrow().iter().map(|v| v.0).collect::<Vec<_>>() == [49, 45]));
        assert_eq!(
            frames.seal_and_wait(Instant::now()),
            Err(crate::native_flight::Error::Reentrant)
        );
        HISTORY_FREES.fetch_add(1, Ordering::Relaxed);
        unsafe {
            SetLastError(0x36950);
        }
        0x369b4
    }
    #[test]
    fn pinned_history_clear_gateway_frees_once_and_rejects_a_changed_complete_body() {
        if !child("pinned_history_clear_gateway_frees_once_and_rejects_a_changed_complete_body") {
            return;
        }
        if !supports_atomic_promotion_fetch() {
            assert!(HOOKS.lock().unwrap().is_empty());
            println!("NOT EXECUTED: history-clear entry exchange requires supported atomic instruction fetch");
            return;
        }
        use windows_sys::Win32::System::Diagnostics::Debug::{
            RtlAddFunctionTable, RtlDeleteFunctionTable, IMAGE_RUNTIME_FUNCTION_ENTRY,
        };
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        let size = 0xb8b000;
        let base =
            unsafe { VirtualAlloc(null_mut(), size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
                as usize;
        assert_ne!(base, 0);
        let decode = |hex: &str| -> Vec<u8> {
            (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect()
        };
        for (rva, bytes, _, _) in sites() {
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), (base + rva) as *mut u8, 8);
            }
        }
        let frame = &history_clear::FRAMES[0];
        let body = decode(frame.specimen);
        unsafe {
            ptr::copy_nonoverlapping(
                body.as_ptr(),
                (base + frame.begin as usize) as *mut u8,
                body.len(),
            );
            ptr::copy_nonoverlapping([0xb5u8, 0].as_ptr(), (base + 0x37f78) as *mut u8, 2);
            ptr::copy_nonoverlapping([0x48u8, 0xb8].as_ptr(), (base + 0x9908f0) as *mut u8, 2);
            ptr::copy_nonoverlapping(
                (history_original_free as *const () as u64)
                    .to_le_bytes()
                    .as_ptr(),
                (base + 0x9908f2) as *mut u8,
                8,
            );
            ptr::copy_nonoverlapping([0xffu8, 0xe0].as_ptr(), (base + 0x9908fa) as *mut u8, 2);
        }
        let mut table: Vec<[u32; 3]> = frame
            .parts
            .iter()
            .map(|p| [p.begin, p.end, p.unwind])
            .collect();
        assert_eq!(std::mem::size_of::<IMAGE_RUNTIME_FUNCTION_ENTRY>(), 12);
        for part in frame.parts {
            let bytes = decode(part.hex);
            unsafe {
                ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    (base + part.unwind as usize) as *mut u8,
                    bytes.len(),
                );
            }
        }
        assert!(unsafe {
            RtlAddFunctionTable(table.as_mut_ptr().cast(), table.len() as u32, base as u64)
        });
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(base as _, 0xa00000, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        // The actual production verifier checks the complete body and all
        // chained unwind partitions before publishing the ten owned entries.
        unsafe { install(base) }.unwrap();
        assert_eq!(HOOKS.lock().unwrap().len(), sites().len());
        let sentinel = (base + 0xb8aa00) as u64;
        let mut node = [sentinel, sentinel, 0, 0, 0, 0, 0];
        let pointer = node.as_mut_ptr() as u64;
        unsafe {
            *(sentinel as *mut u64) = pointer;
            *((sentinel + 8) as *mut u64) = pointer;
        }
        let mut enqueue: Saved = unsafe { std::mem::zeroed() };
        enqueue.invocation.kind = 40;
        begin(&mut enqueue);
        let mut allocation: Saved = unsafe { std::mem::zeroed() };
        allocation.invocation.kind = 41;
        allocation.invocation.caller = base as u64 + 0x374ce;
        begin(&mut allocation);
        allocation.gpr[4] = pointer;
        finish(&mut allocation, false);
        finish(&mut enqueue, false);
        let clear: unsafe extern "system" fn() -> u64 =
            unsafe { std::mem::transmute(base + 0x36950) };
        unsafe {
            SetLastError(0x1234);
        }
        assert_eq!(unsafe { clear() }, 0x369b4);
        assert_eq!(unsafe { GetLastError() }, 0x36950);
        assert_eq!(HISTORY_FREES.load(Ordering::Relaxed), 1);
        assert_eq!(unsafe { *(sentinel as *const u64) }, sentinel);
        assert_eq!(unsafe { *((sentinel + 8) as *const u64) }, sentinel);
        assert_eq!(FRAMES.get().unwrap().pending_on_set(u64::MAX), Ok(0));
        FRAMES
            .get()
            .unwrap()
            .seal_and_wait(Instant::now() + Duration::from_secs(1))
            .unwrap();
        unsafe { restore() }.unwrap();
        let mut protection = 0;
        let last = (base + frame.end as usize - 1) as *mut u8;
        assert_ne!(
            unsafe { VirtualProtect(last.cast(), 1, PAGE_READWRITE, &mut protection) },
            0
        );
        unsafe {
            *last ^= 1;
        }
        assert_ne!(
            unsafe { VirtualProtect(last.cast(), 1, protection, &mut old) },
            0
        );
        assert_eq!(
            unsafe { install(base) },
            Err("scheduler whole code differs")
        );
        assert!(HOOKS.lock().unwrap().is_empty());
        assert!(unsafe { RtlDeleteFunctionTable(table.as_ptr().cast()) });
        assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
    }
    #[test]
    fn second_node_replay_copy_and_original_free_have_distinct_lifetimes() {
        if !child("second_node_replay_copy_and_original_free_have_distinct_lifetimes") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        CORE.store(0x180000000, Ordering::Release);
        let f = FRAMES.get().unwrap();
        let mut enqueue = saved(40, 0);
        begin(&mut enqueue);
        let first = allocation(0x3743a, 100, 0);
        let history = allocation(0x374ce, 200, 0);
        assert_ne!(first, history);
        finish(&mut enqueue, false);
        let mut copy = saved(44, 0);
        begin(&mut copy);
        let copied = allocation(0x3657d, 300, 200);
        assert_ne!(history, copied);
        finish(&mut copy, false);
        retire(0x38024, 200);
        for (node, id) in [(100, first), (300, copied)] {
            let mut execute = saved(42, 0x37358);
            execute.gpr[7] = node;
            begin(&mut execute);
            assert_eq!(execute.invocation.token, id);
            finish(&mut execute, false);
            assert!(f.contains_task(node), "dequeue still has native cleanup");
            retire(0x373a2, node);
        }
        for (caller, node) in [(0x377aa, 400), (0x37980, 500)] {
            let mut replay = saved(43, 0);
            begin(&mut replay);
            allocation(caller, node, 0);
            finish(&mut replay, false);
            retire(0x38204, node);
        }
        f.seal_and_wait(Instant::now() + Duration::from_secs(1))
            .unwrap();
        f.reopen().unwrap();
    }

    #[test]
    fn exceptional_free_retains_its_reservation_and_cannot_reopen() {
        if !child("exceptional_free_retains_its_reservation_and_cannot_reopen") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        CORE.store(0x180000000, Ordering::Release);
        let mut enqueue = saved(40, 0);
        begin(&mut enqueue);
        allocation(0x3743a, 100, 0);
        finish(&mut enqueue, false);
        let mut r = saved(45, 0x381a4);
        r.gpr[1] = 100;
        begin(&mut r);
        finish(&mut r, true);
        let f = FRAMES.get().unwrap();
        assert_eq!(
            f.seal_and_wait(Instant::now() + Duration::from_secs(1)),
            Err(crate::native_flight::Error::Unresolved)
        );
        assert_eq!(f.reopen(), Err(crate::native_flight::Error::Unresolved));
    }

    #[test]
    fn ten_original_entry_patches_restore_without_losing_relocated_operands() {
        if !child("ten_original_entry_patches_restore_without_losing_relocated_operands") {
            return;
        }
        if !supports_atomic_promotion_fetch() {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        let size = 0xb8b000;
        let base =
            unsafe { VirtualAlloc(null_mut(), size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
                as usize;
        assert_ne!(base, 0);
        for (rva, expected, _, _) in sites() {
            unsafe {
                ptr::copy_nonoverlapping(expected.as_ptr(), (base + rva) as *mut u8, 8);
            }
        }
        unsafe {
            ptr::copy_nonoverlapping([0xb5u8, 0].as_ptr(), (base + 0x37f78) as *mut u8, 2);
        }
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(base as _, size, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        unsafe { install_at(base) }.unwrap();
        assert_eq!(HOOKS.lock().unwrap().len(), 10);
        for (i, (rva, _, n, _)) in sites().into_iter().enumerate() {
            assert_eq!(copy_current::<1>((base + rva) as u64), Some([0xe9]));
            assert_eq!(
                NATIVE_OPERATION_CONTINUE[i].load(Ordering::Acquire),
                (base + if i == 5 { 0x9908f0 } else { rva + n }) as u64
            );
        }
        assert_eq!(copy_current::<2>((base + 0x37f78) as u64), Some([0xb5, 0]));
        unsafe { restore() }.unwrap();
        assert!(HOOKS.lock().unwrap().is_empty());
        for (rva, expected, _, _) in sites() {
            assert_eq!(copy_current::<8>((base + rva) as u64), Some(expected));
        }
        assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
    }

    global_asm!(
        ".p2align 4",
        ".globl native_operation_probe_transfer_tail",
        ".seh_proc native_operation_probe_transfer_tail",
        "native_operation_probe_transfer_tail:",
        "sete al",
        "movzx eax,al",
        "lea rsp,[rsp+0x48]",
        "ret",
        ".seh_endproc",
    );
    unsafe extern "system" {
        fn native_operation_probe_transfer_tail();
    }
    #[test]
    fn relocated_transfer_compare_preserves_native_input_flags_and_result() {
        if !child("relocated_transfer_compare_preserves_native_input_flags_and_result") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        let flag = 1u8;
        NATIVE_OPERATION_FLAG.store(&flag as *const u8 as u64, Ordering::Release);
        NATIVE_OPERATION_CONTINUE[7].store(
            native_operation_probe_transfer_tail as *const () as u64,
            Ordering::Release,
        );
        let invoke: unsafe extern "system" fn(u64) -> u64 =
            unsafe { std::mem::transmute(native_operation_transfer_gateway as *const ()) };
        assert_eq!(unsafe { invoke(1) }, 1);
        assert_eq!(unsafe { invoke(0) }, 0);
        let f = FRAMES.get().unwrap();
        f.seal_and_wait(Instant::now() + Duration::from_secs(1))
            .unwrap();
    }
    global_asm!(
        ".p2align 4",
        ".globl native_operation_probe_allocate",
        ".seh_proc native_operation_probe_allocate",
        "native_operation_probe_allocate:",
        "mov r8d,[rsp+0x30]",
        ".seh_endprologue",
        "mov rax,[rsp+0x28]",
        "shl rax,32",
        "or rax,r8",
        "ret",
        ".seh_endproc",
    );
    unsafe extern "system" {
        fn native_operation_probe_allocate();
    }
    global_asm!(
        ".p2align 4",
        ".globl native_operation_probe_enqueue",
        ".seh_proc native_operation_probe_enqueue",
        "native_operation_probe_enqueue:",
        ".byte 0x40,0x55",
        ".seh_pushreg rbp",
        "push rsi",
        ".seh_pushreg rsi",
        "push rdi",
        ".seh_pushreg rdi",
        "push r14",
        ".seh_pushreg r14",
        ".seh_endprologue",
        "mov rax,0x11223344",
        "inc qword ptr [rcx]",
        "pop r14",
        "pop rdi",
        "pop rsi",
        "pop rbp",
        "ret",
        ".seh_endproc",
        ".p2align 4",
        ".globl native_operation_probe_execute",
        ".seh_proc native_operation_probe_execute",
        "native_operation_probe_execute:",
        ".byte 0x40,0x55",
        ".seh_pushreg rbp",
        "push rbx",
        ".seh_pushreg rbx",
        "push r12",
        ".seh_pushreg r12",
        "push r13",
        ".seh_pushreg r13",
        ".seh_endprologue",
        "mov rax,0x55667788",
        "inc qword ptr [rcx]",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "ret",
        ".seh_endproc",
    );
    unsafe extern "system" {
        fn native_operation_probe_enqueue();
        fn native_operation_probe_execute();
    }
    #[test]
    fn unknown_incoming_keeps_original_enqueue_return_without_issuing_world_authority() {
        if !child("unknown_incoming_keeps_original_enqueue_return_without_issuing_world_authority")
        {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        CORE.store(0x180000000, Ordering::Release);
        NATIVE_OPERATION_CONTINUE[0].store(
            native_operation_probe_enqueue as *const () as u64 + 6,
            Ordering::Release,
        );
        let invoke: unsafe extern "system" fn(*mut u64) -> u64 =
            unsafe { std::mem::transmute(native_operation_enqueue_gateway as *const ()) };
        let mut calls = 0;
        runtime::fixture_invocation_runtime();
        let value = sessions::fixture_unknown_input(|| unsafe { invoke(&mut calls) });
        assert_eq!(value, 0x11223344);
        assert_eq!(calls, 1);
        assert!(!FAILED.load(Ordering::Acquire));
        assert!(STACK.with(|s| s.borrow().is_empty()));
        assert!(sessions::NativeSessionApi::current_world().is_none());
        assert!(!FRAMES.get().unwrap().unproven_on_current_thread());
        let outside = runtime::invocation_start();
        assert!(runtime::invocation_admitted(outside));
        unsafe {
            runtime::invocation_finish(&AtomicU64::new(outside));
        }
        assert!(
            !runtime::invocation_admitted(outside),
            "settled opaque cookie cannot be reused"
        );
        let before_unknown = runtime::invocation_start();
        assert!(runtime::invocation_admitted(before_unknown));
        sessions::fixture_unknown_input(|| {});
        assert!(
            !runtime::invocation_admitted(before_unknown),
            "a bridge call that crossed unknown input must stay refused through its own return"
        );
        unsafe {
            runtime::invocation_finish(&AtomicU64::new(before_unknown));
        }
        sessions::fixture_unknown_input(|| {
            assert_eq!(runtime::invocation_start(), 0);
            assert!(!runtime::call_current(runtime::CallFence::default(), 0));
            assert_eq!(runtime::call_begin(0).ticket, 0);
        });
        // An unproven enqueue can publish work that executes after the raw
        // callback has returned. The saved node, rather than the new scope's
        // current World, controls the later product admission.
        sessions::fixture_unknown_input(|| {
            let mut enqueue = saved(40, 0);
            begin(&mut enqueue);
            allocation(0x3743a, 100, 0);
            allocation(0x374ce, 200, 0);
            finish(&mut enqueue, false);
        });
        let mut copy = saved(44, 0);
        begin(&mut copy);
        allocation(0x3657d, 300, 200);
        finish(&mut copy, false);
        for node in [100, 300] {
            let mut execute = saved(42, 0x37358);
            execute.gpr[7] = node;
            begin(&mut execute);
            assert_eq!(runtime::invocation_start(), 0);
            finish(&mut execute, false);
            retire(0x373a2, node);
        }
        retire(0x38024, 200);
        assert!(!super::unproven_input_active());
        assert!(sessions::NativeSessionApi::current_world().is_none());
        FRAMES
            .get()
            .unwrap()
            .seal_and_wait(Instant::now() + Duration::from_secs(1))
            .unwrap();
    }
    #[test]
    fn original_four_push_shims_preserve_saved_rbp_and_native_return() {
        if !child("original_four_push_shims_preserve_saved_rbp_and_native_return") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        CORE.store(0x180000000, Ordering::Release);
        for (i, body, gateway, value) in [
            (
                0,
                native_operation_probe_enqueue as *const () as u64,
                native_operation_enqueue_gateway as *const () as usize,
                0x11223344,
            ),
            (
                2,
                native_operation_probe_execute as *const () as u64,
                native_operation_execute_gateway as *const () as usize,
                0x55667788,
            ),
        ] {
            NATIVE_OPERATION_CONTINUE[i]
                .store(body + if i == 0 { 6 } else { 7 }, Ordering::Release);
            let invoke: unsafe extern "system" fn(*mut u64) -> u64 =
                unsafe { std::mem::transmute(gateway) };
            let mut n = 0;
            assert_eq!(unsafe { invoke(&mut n) }, value);
            assert_eq!(n, 1);
        }
    }
    #[test]
    fn allocator_gateway_preserves_fifth_and_sixth_native_stack_arguments() {
        if !child("allocator_gateway_preserves_fifth_and_sixth_native_stack_arguments") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        CORE.store(0x180000000, Ordering::Release);
        let _install: unsafe fn(usize) -> Result<(), &'static str> = install;
        NATIVE_OPERATION_CONTINUE[1].store(
            native_operation_probe_allocate as *const () as u64 + 5,
            Ordering::Release,
        );
        let invoke: unsafe extern "system" fn(u64, u64, u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(native_operation_allocate_gateway as *const ()) };
        assert_eq!(
            unsafe { invoke(1, 2, 3, 4, 0x55667788, 0xaabbccdd) },
            0x55667788aabbccdd
        );
    }
}
