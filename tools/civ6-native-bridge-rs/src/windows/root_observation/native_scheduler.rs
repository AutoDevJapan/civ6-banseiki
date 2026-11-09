//! Complete scheduler publication/reissue frames inherit existing reservations.
//! Unknown native work stays transparent. These frames do not seal producers.
use super::*;
use std::cell::RefCell;
pub(super) struct Fragment {
    pub(super) begin: u32,
    pub(super) end: u32,
    pub(super) unwind: u32,
    pub(super) hex: &'static str,
}
pub(super) struct FixedFrame {
    pub(super) begin: u32,
    pub(super) end: u32,
    pub(super) sha: &'static str,
    #[cfg(test)]
    pub(super) specimen: &'static str,
    pub(super) parts: &'static [Fragment],
}
#[path = "native_scheduler_audit.rs"]
mod audit;
// Natural-close ledger; keeps the scheduler's single owner of Host list entries.
#[path = "native_closure.rs"]
pub(super) mod closure;
#[path = "native_scheduler_fixed.rs"]
mod fixed;
static EXE: AtomicU64 = AtomicU64::new(0);
static NEXT: AtomicU64 = AtomicU64::new(0);
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
#[unsafe(no_mangle)]
static NATIVE_SCHEDULER_CONTINUE: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];
struct Scope {
    serial: u64,
    kind: u64,
    hold: Option<u64>,
    job: Option<u64>,
    task: u64,
    allocated: bool,
    // FreeJobList list id and native return address for the closure ledger.
    arg: u64,
    caller: u64,
    // RBX at the Host FreeJobList entry: the manager at the 4A1209 call.
    receiver: u64,
}
thread_local! {static SCOPES:RefCell<Vec<Scope>>=const{RefCell::new(Vec::new())};}
fn unresolved() {
    FAILED.store(true, Ordering::Release);
    TRACE.0[6].store(1, Ordering::Release);
    if let Some(f) = FRAMES.get() {
        f.mark_unresolved();
    }
}
#[track_caller]
fn fault() -> ! {
    census::fault(
        census::module(module_path!()),
        std::panic::Location::caller().line(),
    );
    diagnostic(130);
    unresolved();
    std::process::abort()
}
fn serial() -> u64 {
    NEXT.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
        .unwrap_or_else(|_| fault())
        + 1
}
fn value(p: u64) -> Option<u64> {
    copy_current::<8>(p).map(u64::from_le_bytes)
}
fn exe(rva: u64) -> u64 {
    EXE.load(Ordering::Acquire) + rva
}
fn caller(raw: &Saved, rva: u64) -> bool {
    #[cfg(test)]
    if let Some(pc) = tests::caller_override(rva) {
        return raw.invocation.caller == pc;
    }
    raw.invocation.caller == exe(rva)
}
fn shared_push_reserved() -> bool {
    #[cfg(test)]
    if tests::SHARED_PUSH.load(Ordering::Acquire) {
        return true;
    }
    jobs::push_reserved()
}
/// Called by the one existing T execute/destructor owner before its first
/// T/P read. Every outer scope keeps its hold after this T is settled.
pub(super) fn attach_current(task: u64) {
    let Some(f) = FRAMES.get() else {
        return;
    };
    SCOPES.with(|s| {
        for c in s.borrow_mut().iter_mut() {
            if let Some(h) = c.hold {
                f.consumer_attach(h, task).unwrap_or_else(|_| fault());
            } else {
                c.hold = f.consumer_enter(task).unwrap_or_else(|_| fault());
            }
        }
    });
}
/// Called only by the existing allocator owner after original normal/SEH
/// return, before its caller initializes or links the new native T.
pub(super) fn allocated(raw: &Saved, exceptional: bool) {
    if raw.invocation.kind != 23 {
        return;
    }
    SCOPES.with(|s| {
        let mut s = s.borrow_mut();
        let Some(c) = s.iter_mut().rev().find(|c| matches!(c.kind, 130 | 131)) else {
            return;
        };
        if !((c.kind == 130 && caller(raw, 0x985904)) || (c.kind == 131 && caller(raw, 0x985f9e))) {
            return;
        }
        let Some(id) = c.job else {
            return;
        };
        if exceptional {
            unresolved();
            return;
        }
        if c.allocated {
            fault();
        }
        c.allocated = true;
        c.task = raw.gpr[4];
        if c.task != 0 {
            FRAMES
                .get()
                .unwrap()
                .bind_before_publication(id, c.task)
                .unwrap_or_else(|_| fault());
        }
    });
}
fn typed(raw: &Saved) -> bool {
    let expected = if raw.invocation.kind == 134 {
        0x1120678
    } else {
        0x11206b0
    };
    value(raw.gpr[0]) == Some(exe(expected))
        && value(exe(expected + 0x30))
            == Some(exe(if raw.invocation.kind == 134 {
                0xe42470
            } else {
                0xe427f0
            }))
}
extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        let mut c = Scope {
            serial: serial(),
            kind: raw.invocation.kind,
            hold: None,
            job: None,
            task: 0,
            allocated: false,
            arg: 0,
            caller: 0,
            receiver: 0,
        };
        // Host Add/Push set, Spawn list and FreeJobList identity are observed
        // before the original runs. The ledger is empty until a manager is seen.
        match c.kind {
            130 => closure::submit(raw.gpr[1]),
            131 => closure::submit(raw.gpr[2]),
            132 => closure::spawn(raw.gpr[0] as u32),
            137 => {
                c.arg = raw.gpr[0];
                c.caller = raw.invocation.caller;
                c.receiver = raw.gpr[7];
                Ok(())
            }
            _ => Ok(()),
        }
        .unwrap_or_else(|v| closure::fault(v));
        if let Some(f) = FRAMES.get() {
            // LoadGameView's existing load carrier binds its World at native
            // birth. Attaching this creator to the old World would make its
            // normal reload close wait for the very worker doing the close.
            let delegated_load = c.kind == 130 && caller(raw, 0x98618b);
            // Dispatch entry's RDX is scheduler metadata, never a T or a World ticket.
            // Post-Wait set/list teardown is not execution admission. These
            // observers attach only if an actual recorded T destructor is
            // reached; sharing a native cleanup thread grants no authority.
            if !delegated_load
                && !matches!(c.kind, 136 | 137)
                && (!matches!(c.kind, 134 | 135) || typed(raw))
            {
                c.hold = f.enter_inherited().unwrap_or_else(|_| fault());
            }
            if c.kind == 133 && c.hold.is_none() {
                c.hold = f.consumer_enter(raw.gpr[1]).unwrap_or_else(|_| fault());
            }
            if c.hold.is_some() && matches!(c.kind, 130 | 131) {
                let existing = c.kind == 131 && caller(raw, 0x986497) && shared_push_reserved();
                if !existing {
                    let (payload, set) = if c.kind == 130 {
                        (raw.gpr[0], raw.gpr[1])
                    } else {
                        (raw.gpr[1], raw.gpr[2])
                    };
                    c.job = Some(f.issue_on_set(payload, set).unwrap_or_else(|_| fault()));
                }
            }
        }
        raw.invocation.cookie = COOKIE;
        raw.invocation.token = c.serial;
        SCOPES.with(|s| s.borrow_mut().push(c));
        if raw.invocation.kind == 133 {
            attach_current(raw.gpr[1]);
        }
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
    if !exceptional && matches!(raw.invocation.kind, 134 | 135) && raw.gpr[4] != 0 {
        // A newly selected existing T also retains the still-active outer
        // consumer/publication frame before this dispatcher returns to it.
        attach_current(raw.gpr[4]);
        consumers::attach_current(raw.gpr[4]);
    }
    let c = SCOPES
        .with(|s| s.borrow_mut().pop())
        .unwrap_or_else(|| fault());
    if (c.serial, c.kind) != (raw.invocation.token, raw.invocation.kind) {
        fault();
    }
    if c.kind == 137 {
        if exceptional {
            closure::unwound();
        } else {
            closure::list_released(c.arg as u32, c.caller, c.receiver)
                .unwrap_or_else(|v| closure::fault(v));
        }
    }
    if exceptional && (c.hold.is_some() || c.job.is_some()) {
        unresolved();
    }
    if !exceptional {
        if let Some(job) = c.job {
            if !c.allocated {
                fault();
            }
            let f = FRAMES.get().unwrap();
            if c.task == 0 {
                f.cancel_unpublished_job(job).unwrap_or_else(|_| fault());
            } else {
                f.pushed(job, c.task).unwrap_or_else(|_| fault());
            }
        }
    }
    if let Some(hold) = c.hold {
        FRAMES
            .get()
            .unwrap()
            .consumer_exit(hold)
            .unwrap_or_else(|_| fault());
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
    if !r.is_null()
        && unsafe { (*r).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0
        && std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            finish(unsafe { &mut *((frame + 0x20) as *mut Saved) }, true)
        }))
        .is_err()
    {
        unresolved();
    }
    unsafe { SetLastError(e) };
    1
}
macro_rules! home {
    ($name:literal,$slot:literal,$offset:literal,$a:literal,$b:literal) => {
        global_asm!(concat!(
            ".globl ",
            $name,
            "\n.seh_proc ",
            $name,
            "\n",
            $name,
            ":\nmov [rsp+",
            $offset,
            "],rbx\n.seh_savereg rbx,",
            $offset,
            "\n.seh_endprologue\nmov [rsp+",
            $a,
            "],rax\nmov rax,[rip+NATIVE_SCHEDULER_CONTINUE+",
            $slot,
            "]\nmov [rsp+",
            $b,
            "],rax\nmov rax,[rsp+",
            $a,
            "]\njmp qword ptr [rsp+",
            $b,
            "]\n.seh_endproc\n"
        ));
    };
}
home!("scheduler_add_original", "0", "0x10", "8", "0x20");
home!("scheduler_push_original", "8", "0x10", "8", "0x18");
home!("scheduler_publish_original", "24", "0x10", "8", "0x18");
home!("scheduler_dispatch_a_original", "32", "0x18", "8", "0x20");
home!("scheduler_dispatch_b_original", "40", "0x18", "8", "0x20");
home!("scheduler_destroy_original", "48", "8", "0x18", "0x20");
global_asm!(
    ".globl scheduler_spawn_original",
    ".seh_proc scheduler_spawn_original",
    "scheduler_spawn_original:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x30",
    ".seh_stackalloc 0x30",
    ".seh_endprologue",
    "mov [rsp+0x40],rax",
    "mov rax,[rip+NATIVE_SCHEDULER_CONTINUE+16]",
    "mov [rsp+0x48],rax",
    "mov rax,[rsp+0x40]",
    "jmp qword ptr [rsp+0x48]",
    ".seh_endproc",
    ".globl scheduler_free_original",
    ".seh_proc scheduler_free_original",
    "scheduler_free_original:",
    "push rbp",
    ".seh_pushreg rbp",
    "sub rsp,0x40",
    ".seh_stackalloc 0x40",
    ".seh_endprologue",
    "mov [rsp+0x50],rax",
    "mov rax,[rip+NATIVE_SCHEDULER_CONTINUE+56]",
    "mov [rsp+0x58],rax",
    "mov rax,[rsp+0x50]",
    "jmp qword ptr [rsp+0x58]",
    ".seh_endproc",
);
unsafe extern "system" {
    fn scheduler_add_original();
    fn scheduler_push_original();
    fn scheduler_spawn_original();
    fn scheduler_publish_original();
    fn scheduler_dispatch_a_original();
    fn scheduler_dispatch_b_original();
    fn scheduler_destroy_original();
    fn scheduler_free_original();
    fn scheduler_add_gateway();
    fn scheduler_push_gateway();
    fn scheduler_spawn_gateway();
    fn scheduler_publish_gateway();
    fn scheduler_dispatch_a_gateway();
    fn scheduler_dispatch_b_gateway();
    fn scheduler_destroy_gateway();
    fn scheduler_free_gateway();
}
gateway!(
    "scheduler_add_gateway",
    "130",
    scheduler_add_original,
    begin,
    end,
    unwind
);
gateway!(
    "scheduler_push_gateway",
    "131",
    scheduler_push_original,
    begin,
    end,
    unwind
);
gateway!(
    "scheduler_spawn_gateway",
    "132",
    scheduler_spawn_original,
    begin,
    end,
    unwind
);
gateway!(
    "scheduler_publish_gateway",
    "133",
    scheduler_publish_original,
    begin,
    end,
    unwind
);
gateway!(
    "scheduler_dispatch_a_gateway",
    "134",
    scheduler_dispatch_a_original,
    begin,
    end,
    unwind
);
gateway!(
    "scheduler_dispatch_b_gateway",
    "135",
    scheduler_dispatch_b_original,
    begin,
    end,
    unwind
);
gateway!(
    "scheduler_destroy_gateway",
    "136",
    scheduler_destroy_original,
    begin,
    end,
    unwind
);
gateway!(
    "scheduler_free_gateway",
    "137",
    scheduler_free_original,
    begin,
    end,
    unwind
);
fn gateways() -> [usize; 8] {
    [
        scheduler_add_gateway as *const () as usize,
        scheduler_push_gateway as *const () as usize,
        scheduler_spawn_gateway as *const () as usize,
        scheduler_publish_gateway as *const () as usize,
        scheduler_dispatch_a_gateway as *const () as usize,
        scheduler_dispatch_b_gateway as *const () as usize,
        scheduler_destroy_gateway as *const () as usize,
        scheduler_free_gateway as *const () as usize,
    ]
}
const SITES: [(usize, [u8; 8], usize); 8] = [
    (
        0x9858b0,
        [0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x74],
        5,
    ),
    (
        0x985f40,
        [0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x6c],
        5,
    ),
    (
        0x986010,
        [0x40, 0x53, 0x48, 0x83, 0xec, 0x30, 0x45, 0x33],
        6,
    ),
    (
        0xe3e8c0,
        [0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x6c],
        5,
    ),
    (
        0xe42470,
        [0x48, 0x89, 0x5c, 0x24, 0x18, 0x55, 0x56, 0x57],
        5,
    ),
    (
        0xe427f0,
        [0x48, 0x89, 0x5c, 0x24, 0x18, 0x55, 0x56, 0x57],
        5,
    ),
    (0x985a80, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x74], 5),
    (
        0x985bd0,
        [0x40, 0x55, 0x48, 0x83, 0xec, 0x40, 0x48, 0x8d],
        6,
    ),
];
fn code_matches(bytes: &[u8], frame: &FixedFrame) -> bool {
    bytes.len() == (frame.end - frame.begin) as usize
        && format!("{:x}", Sha256::digest(bytes)) == frame.sha
}
pub(super) unsafe fn prepare_host(module: usize) -> Result<(), &'static str> {
    let _pin = super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA)?;
    unsafe {
        verify(module)?;
        for (table, target) in [(0x1120678usize, 0xe42470usize), (0x11206b0, 0xe427f0)] {
            if value((module + table + 0x30) as u64) != Some((module + target) as u64) {
                return Err("scheduler typed slot differs");
            }
        }
        install_at(module, &SITES)
    }
}
unsafe fn verify(module: usize) -> Result<(), &'static str> {
    // Existing jobs/consumer owners already patch their own export entries.
    // Those complete originals remain disk-audit evidence, never rechecked
    // as live original bytes after their single owner published its gateway.
    unsafe { verify_frames(module, &fixed::FRAMES[..SITES.len()]) }
}
pub(super) unsafe fn verify_frames(
    module: usize,
    frames: &[FixedFrame],
) -> Result<(), &'static str> {
    for f in frames {
        let len = (f.end - f.begin) as usize;
        if !committed_range(module + f.begin as usize, len, 1, false) {
            return Err("scheduler code range");
        }
        if !code_matches(
            unsafe { std::slice::from_raw_parts((module + f.begin as usize) as *const u8, len) },
            f,
        ) {
            return Err("scheduler whole code differs");
        }
        for p in f.parts {
            let mut base = 0;
            let row = unsafe {
                windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry(
                    module as u64 + u64::from(p.begin),
                    &mut base,
                    null_mut(),
                )
            };
            if base != module as u64
                || copy_current::<12>(row as u64).map(|v| v.to_vec())
                    != Some(
                        [
                            p.begin.to_le_bytes(),
                            p.end.to_le_bytes(),
                            p.unwind.to_le_bytes(),
                        ]
                        .concat(),
                    )
            {
                return Err("scheduler pdata differs");
            }
            for (i, byte) in (0..p.hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&p.hex[i..i + 2], 16).unwrap())
                .enumerate()
            {
                if copy_current::<1>(module as u64 + u64::from(p.unwind) + i as u64) != Some([byte])
                {
                    return Err("scheduler unwind differs");
                }
            }
        }
    }
    Ok(())
}
/// Bind after early Host publication, before any Core entry is patched.
/// Resolver slots are allowed to be unresolved; a resolved slot must still
/// name the exact pinned Host export, never a replacement scheduler.
pub(super) unsafe fn bind_core(core: usize) -> Result<(), &'static str> {
    let _pin = super::super::pin(
        GAMECORE,
        "092abfbb2176f8df04b5212f819dc269dd83aea866ff05aaf489904a1ff7c41f",
    )?;
    unsafe {
        verify_frames(core, audit::core_frames())?;
    }
    let host = EXE.load(Ordering::Acquire);
    if host == 0 {
        return Err("scheduler Host not prepared");
    }
    audit::verify_flow(|is_core, r, n| {
        let base = if is_core { core as u64 } else { host };
        if !committed_range((base + u64::from(r)) as usize, n, 1, false) {
            return None;
        }
        let mut b =
            unsafe { std::slice::from_raw_parts((base + u64::from(r)) as *const u8, n) }.to_vec();
        if !is_core && n == 8 && [0x11206a8, 0x11206e0].contains(&r) {
            let target = u64::from_le_bytes(b.as_slice().try_into().ok()?);
            b = target
                .checked_sub(host)?
                .checked_add(0x140000000)?
                .to_le_bytes()
                .to_vec();
        }
        Some(b)
    })?;
    for (slot, target) in [
        (0xb90688, 0x986480),
        (0xb90690, 0x9864a0),
        (0xb90680, 0x986240),
        (0xb906a0, 0x986470),
    ] {
        let address = value(core as u64 + slot).ok_or("scheduler resolver slot unreadable")?;
        if address != 0 && address != host + target {
            return Err("scheduler resolved export differs");
        }
    }
    unsafe { closure::install(core)? };
    Ok(())
}
unsafe fn install_at(
    module: usize,
    sites: &[(usize, [u8; 8], usize); 8],
) -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "scheduler hook owner")?;
    if !hooks.is_empty() {
        return Err("scheduler hooks already installed");
    }
    for &(rva, expected, _) in sites {
        if copy_current::<8>((module + rva) as u64) != Some(expected) {
            return Err("scheduler entry differs");
        }
    }
    EXE.store(module as u64, Ordering::Release);

    for (i, &(rva, expected, stolen)) in sites.iter().enumerate() {
        let target = (module + rva) as *mut u8;
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if !(target as usize).is_multiple_of(8)
            || unsafe { VirtualQuery(target.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
            || info.State != MEM_COMMIT
            || info.Protect & 0xff != PAGE_EXECUTE_READ
            || info.Protect & PAGE_GUARD != 0
        {
            return Err("scheduler entry protection");
        }
        let relay = unsafe { executable_near(target as usize, target as usize, stolen) }
            .ok_or("scheduler relay")?;
        let mut unpublished = Unpublished(relay);
        unsafe {
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
            ptr::copy_nonoverlapping(
                (gateways()[i] as u64).to_le_bytes().as_ptr(),
                relay.add(6),
                8,
            );
        }
        let mut old = 0;
        if unsafe { VirtualProtect(relay.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
            || unsafe { FlushInstructionCache(GetCurrentProcess(), relay.cast(), 14) } == 0
        {
            return Err("scheduler relay publication");
        }
        let mut patched = expected;
        patched[..5].copy_from_slice(
            &rel32_jump(target as usize, relay as usize).ok_or("scheduler relay range")?,
        );
        patched[5..stolen.min(8)].fill(0x90);
        NATIVE_SCHEDULER_CONTINUE[i].store((module + rva + stolen) as u64, Ordering::Release);
        hooks.push(OwnedHook {
            hook: Hook {
                target,
                original: expected,
                patched,
                prefix: None,
                _code: relay,
            },
            protection: info.Protect,
        });
        unpublished.0 = null_mut();
        let result = unsafe {
            exchange_entry(
                target,
                u64::from_le_bytes(expected),
                u64::from_le_bytes(patched),
            )
        }?;
        if !result.exchanged || !result.protection_restored || !result.cache_flushed {
            return Err("critical: scheduler publication unverified");
        }
    }
    Ok(())
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "scheduler hook owner")?;
    let closure = unsafe { closure::restore() };
    hooks.retain(|o| {
        let h = &o.hook;
        let current = copy_current::<8>(h.target as u64);
        let restored = current == Some(h.original)
            || (current == Some(h.patched)
                && unsafe {
                    exchange_entry(
                        h.target,
                        u64::from_le_bytes(h.patched),
                        u64::from_le_bytes(h.original),
                    )
                }
                .is_ok_and(|r| r.exchanged && r.protection_restored && r.cache_flushed)
                && copy_current::<8>(h.target as u64) == Some(h.original));
        let mut old = 0;
        !(restored
            && unsafe { VirtualProtect(h.target.cast(), 8, o.protection, &mut old) } != 0
            && unsafe { FlushInstructionCache(GetCurrentProcess(), h.target.cast(), 8) } != 0)
    });
    if hooks.is_empty() && closure.is_ok() {
        Ok(())
    } else {
        Err("critical: scheduler restoration unverified")
    }
}

#[cfg(test)]
#[path = "native_scheduler_tests.rs"]
mod tests;
