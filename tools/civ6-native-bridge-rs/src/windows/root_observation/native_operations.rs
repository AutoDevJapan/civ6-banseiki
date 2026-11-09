//! Serialized operation queue reservations, separate from Host job-set work.
//! Whole original calls execute once. Queue node identity is bound before the
//! original enqueue links it, and inherited at the exact dequeue call site.
use super::*;
use std::cell::RefCell;
static CORE: AtomicU64 = AtomicU64::new(0);
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
#[unsafe(no_mangle)]
static NATIVE_OPERATION_CONTINUE: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
thread_local! {static STACK:RefCell<Vec<(u64,u64)>>=const {RefCell::new(Vec::new())};}
fn fault() -> ! {
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
        let core = CORE.load(Ordering::Acquire);
        let id = match raw.invocation.kind {
            40 => f.reserve_queue().unwrap_or_else(|_| fault()),
            41 => {
                if raw.invocation.caller != core + 0x3743a {
                    return;
                }
                let Some((40, id)) = STACK.with(|s| s.borrow().last().copied()) else {
                    fault();
                };
                id
            }
            42 => {
                if raw.invocation.caller == core + 0x37358 {
                    f.queue_execute(raw.gpr[7]).unwrap_or_else(|_| fault())
                } else {
                    raw.invocation.errors = 1;
                    f.enter().unwrap_or_else(|_| fault())
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
        40 => {
            // No node is created when the original stream size is zero.
            // A nonzero allocated node must have been bound by role41.
            match f.queue_published(raw.invocation.token) {
                Ok(()) => {}
                Err(_) => fault(),
            }
        }
        41 => f
            .bind_before_publication(raw.invocation.token, raw.gpr[4])
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
);
unsafe extern "system" {
    fn native_operation_enqueue_original();
    fn native_operation_allocate_original();
    fn native_operation_execute_original();
    fn native_operation_enqueue_gateway();
    fn native_operation_allocate_gateway();
    fn native_operation_execute_gateway();
}
gateway!(
    "native_operation_enqueue_gateway",
    "40",
    native_operation_enqueue_original,
    begin,
    end,
    unwind
);
gateway!("native_operation_allocate_gateway","41",native_operation_allocate_original,begin,end,unwind,
    "mov rax,[rsp+0x1f0]\nmov [rsp+0x20],rax\nmov rax,[rsp+0x1f8]\nmov [rsp+0x28],rax\nmov rax,[rsp+0x40]\n");
gateway!(
    "native_operation_execute_gateway",
    "42",
    native_operation_execute_original,
    begin,
    end,
    unwind
);
pub(super) unsafe fn install(core: usize) -> Result<(), &'static str> {
    CORE.store(core as u64, Ordering::Release);
    let mut o = HOOKS.lock().map_err(|_| "operation owner poisoned")?;
    let sites = [
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
    ];
    for (i, (rva, expected, n, gateway)) in sites.into_iter().enumerate() {
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
        patch[5..n].fill(0x90);
        NATIVE_OPERATION_CONTINUE[i].store((core + rva + n) as u64, Ordering::Release);
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
        let id = enqueue.invocation.token;
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
        assert!(!f.contains_task(123));
        f.seal_and_wait(Instant::now() + Duration::from_secs(2))
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
