//! Resident Host scheduler frames. Existing birth reservations are the only
//! epoch authority; raw scheduler inputs never create a new-world job.
//! This partial adapter does not certify the scheduler's other queues or seal.
use super::*;
use std::cell::RefCell;

static EXE: AtomicU64 = AtomicU64::new(0);
static CORE: AtomicU64 = AtomicU64::new(0);
static NEXT: AtomicU64 = AtomicU64::new(0);
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
pub(super) fn hook_entries_owned() -> bool {
    owns_hook_entries(&HOOKS, SITES.len())
}

#[cfg(test)]
static TEST_ARENA_CLASS: AtomicU64 = AtomicU64::new(0);
#[unsafe(no_mangle)]
static NATIVE_CONSUMER_CONTINUE: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];

struct Consumer {
    serial: u64,
    hold: Option<u64>,
    typed: bool,
}
struct Wait {
    serial: u64,
    set: u64,
    arena: u64,
    host_rsp: u64,
    native_rsp: u64,
    selected: bool,
    arena_returned: bool,
    external_arena: bool,
}
/// Normal-call correspondence only. An arena's current-arena early return,
/// pending publication and every copied task still need separate evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WaitReceipt {
    pub set: u64,
    pub arena: u64,
    pub host_rsp: u64,
}
thread_local! {
    static CONSUMERS: RefCell<Vec<Consumer>> = const { RefCell::new(Vec::new()) };
    static WAITS: RefCell<Vec<Wait>> = const { RefCell::new(Vec::new()) };
    static RECEIPT: RefCell<Option<WaitReceipt>> = const { RefCell::new(None) };
}
fn serial() -> u64 {
    NEXT.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
        .unwrap_or_else(|_| fault())
        + 1
}
fn unresolved() {
    if let Some(f) = FRAMES.get() {
        f.mark_unresolved();
    }
    FAILED.store(true, Ordering::Release);
    TRACE.0[6].store(1, Ordering::Release);
    RECEIPT.with(|r| *r.borrow_mut() = None);
}
#[track_caller]
fn fault() -> ! {
    census::fault(
        census::module(module_path!()),
        std::panic::Location::caller().line(),
    );
    diagnostic(50);
    unresolved();
    std::process::abort()
}

/// Called by the existing T execute/destructor gateways BEFORE reading T/P.
/// No native pointer is dereferenced here. A later loop iteration may attach
/// its already-bound T even if this scheduler's first input was unrelated.
pub(super) fn attach_current(task: u64) -> bool {
    // Early Host preparation publishes consumer and dispatcher hooks before
    // Core owns NativeFrames. Native tasks on those stacks stay unknown.
    let Some(frames) = FRAMES.get() else {
        return false;
    };
    CONSUMERS.with(|stack| {
        let mut stack = stack.borrow_mut();
        if stack.is_empty() {
            return false;
        }
        let mut attached = false;
        for scope in stack.iter_mut() {
            attached |= if let Some(hold) = scope.hold {
                frames
                    .consumer_attach(hold, task)
                    .unwrap_or_else(|_| fault())
            } else {
                scope.hold = frames.consumer_enter(task).unwrap_or_else(|_| fault());
                scope.hold.is_some()
            };
        }
        attached
    })
}

fn typed_scheduler(scheduler: u64) -> bool {
    let exe = EXE.load(Ordering::Acquire);
    let Some(vft) = copy_current::<8>(scheduler).map(u64::from_le_bytes) else {
        return false;
    };
    let target = if vft == exe + 0x1120678 {
        exe + 0xe42470
    } else if vft == exe + 0x11206b0 {
        exe + 0xe427f0
    } else {
        return false;
    };
    copy_current::<8>(vft + 0x30).map(u64::from_le_bytes) == Some(target)
}

/// Consume on the same thread at Core9922C0's normal return, using its saved
/// *original-call* RSP. Host entry is exactly40h below that RSP. This receipt
/// must not replace pending_on_set==0, consumer drainage or native seal proof.
pub(super) fn take_wait_receipt(set: u64, core_original_rsp: u64) -> Option<WaitReceipt> {
    RECEIPT
        .with(|r| r.borrow_mut().take())
        .filter(|r| r.set == set && core_original_rsp.checked_sub(0x40) == Some(r.host_rsp))
}
pub(super) fn require_wait_receipt() -> bool {
    #[cfg(test)]
    {
        CORE.load(Ordering::Acquire) != 0
    }
    #[cfg(not(test))]
    {
        true
    }
}

#[cfg(test)]
pub(super) unsafe fn fixture_install(exe: usize, core: usize) -> Result<(), &'static str> {
    EXE.store(exe as u64, Ordering::Release);
    CORE.store(core as u64, Ordering::Release);
    TEST_ARENA_CLASS.store(2, Ordering::Release);
    unsafe { install_at(exe) }
}
#[cfg(test)]
pub(super) fn fixture_current_arena() {
    TEST_ARENA_CLASS.store(1, Ordering::Release);
}
#[cfg(test)]
pub(super) fn fixture_early_worker(continuation: usize) -> usize {
    assert!(FRAMES.get().is_none());
    assert_eq!(CORE.load(Ordering::Acquire), 0);
    NATIVE_CONSUMER_CONTINUE[0].store(continuation as u64, Ordering::Release);
    native_consumer_first_gateway as *const () as usize
}
#[cfg(test)]
pub(super) fn fixture_group_arena(continuation: usize, external: bool) -> usize {
    assert!(
        FRAMES.get().is_none(),
        "early arena must precede Core frame ownership"
    );
    assert_eq!(CORE.load(Ordering::Acquire), 0);
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0xe50000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    for (rva, prefix, _) in SITES {
        unsafe {
            ptr::copy_nonoverlapping(prefix.as_ptr(), (base + rva) as *mut u8, 8);
        }
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
    NATIVE_CONSUMER_CONTINUE[3].store(continuation as u64, Ordering::Release);
    TEST_ARENA_CLASS.store(if external { 2 } else { 1 }, Ordering::Release);
    base + 0xe3eaf0
}

extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        raw.invocation.cookie = COOKIE;
        raw.invocation.token = serial();
        match raw.invocation.kind {
            50 | 51 => {
                let hold = FRAMES
                    .get()
                    .and_then(|f| f.consumer_enter(raw.gpr[2]).unwrap_or_else(|_| fault()));
                CONSUMERS.with(|s| {
                    s.borrow_mut().push(Consumer {
                        serial: raw.invocation.token,
                        hold,
                        typed: typed_scheduler(raw.gpr[0]),
                    })
                });
            }
            52 => {
                RECEIPT.with(|r| *r.borrow_mut() = None);
                let selected = raw.invocation.caller == CORE.load(Ordering::Acquire) + 0x992301;
                let set = raw.gpr[0];
                let arena = if selected {
                    let p = set.checked_add(8).unwrap_or_else(|| fault());
                    u64::from_le_bytes(copy_current::<8>(p).unwrap_or_else(|| fault()))
                } else {
                    0
                };
                WAITS.with(|s| {
                    s.borrow_mut().push(Wait {
                        serial: raw.invocation.token,
                        set,
                        arena,
                        selected,
                        host_rsp: raw.invocation.caller_rsp,
                        native_rsp: raw.invocation.native_rsp,
                        arena_returned: false,
                        external_arena: false,
                    })
                });
            }
            53 => {
                loads::group_arena_enter(raw, external_arena(raw.gpr[0]));
                let parent = WAITS.with(|s| {
                    s.borrow()
                        .last()
                        .map(|w| (w.serial, w.arena, w.native_rsp, w.selected))
                });
                if let Some((parent, arena, rsp, true)) = parent {
                    if raw.invocation.caller == EXE.load(Ordering::Acquire) + 0x986270 {
                        if raw.gpr[0] != arena
                            || rsp.checked_sub(0x30) != Some(raw.invocation.caller_rsp)
                        {
                            fault();
                        }
                        raw.invocation.parent = parent;
                        let external = external_arena(arena);
                        WAITS.with(|s| {
                            s.borrow_mut()
                                .last_mut()
                                .unwrap_or_else(|| fault())
                                .external_arena = external
                        });
                    }
                }
            }
            _ => fault(),
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
    match raw.invocation.kind {
        50 | 51 => {
            let scope = CONSUMERS
                .with(|s| s.borrow_mut().pop())
                .unwrap_or_else(|| fault());
            if scope.serial != raw.invocation.token {
                fault();
            }
            if let Some(hold) = scope.hold {
                if exceptional {
                    unresolved();
                }
                FRAMES
                    .get()
                    .unwrap_or_else(|| fault())
                    .consumer_exit(hold)
                    .unwrap_or_else(|_| fault());
            }
            // A recognized runtime table describes only its two audited +30
            // implementations. Other scheduler types stay outside coverage.
            let _typed = scope.typed;
        }
        52 => {
            let wait = WAITS
                .with(|s| s.borrow_mut().pop())
                .unwrap_or_else(|| fault());
            if wait.serial != raw.invocation.token {
                fault();
            }
            if exceptional {
                if wait.selected {
                    unresolved();
                }
            } else if wait.selected && wait.arena != 0 && wait.arena_returned && wait.external_arena
            {
                // Only opaque copied scalars survive. Do not read old set/T/P
                // here or infer complete native join from set+8 being zero.
                RECEIPT.with(|r| {
                    *r.borrow_mut() = Some(WaitReceipt {
                        set: wait.set,
                        arena: wait.arena,
                        host_rsp: wait.host_rsp,
                    })
                });
            }
        }
        53 => {
            loads::group_arena_return(raw, exceptional);
            if raw.invocation.parent != 0 {
                if exceptional {
                    unresolved();
                } else {
                    WAITS.with(|s| {
                        let mut s = s.borrow_mut();
                        let w = s.last_mut().unwrap_or_else(|| fault());
                        if w.serial != raw.invocation.parent {
                            fault();
                        }
                        w.arena_returned = true;
                    });
                }
            }
        }
        _ => fault(),
    }
}

// E3EB27 compares [input arena] with [TLS scheduler+20]. An equality takes
// the current-arena path, including early returns; never issue its receipt.
// Null/unknown TLS cannot be classified by a speculative native call here.
fn external_arena(arena: u64) -> bool {
    #[cfg(test)]
    if TEST_ARENA_CLASS.load(Ordering::Acquire) != 0 {
        return TEST_ARENA_CLASS.load(Ordering::Acquire) == 2;
    }
    let exe = EXE.load(Ordering::Acquire);
    let Some(index) = copy_current::<4>(exe + 0x357b190).map(u32::from_le_bytes) else {
        return false;
    };
    let tls = unsafe { windows_sys::Win32::System::Threading::TlsGetValue(index) } as u64;
    let Some(target) = copy_current::<8>(arena).map(u64::from_le_bytes) else {
        return false;
    };
    tls != 0
        && copy_current::<8>(tls + 0x20)
            .map(u64::from_le_bytes)
            .is_some_and(|current| current != target)
}
extern "system" fn end(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        finish(unsafe { &mut *raw }, false)
    }))
    .is_err()
    {
        fault();
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
        unresolved();
    }
    unsafe { SetLastError(error) };
    1
}

macro_rules! original {
    ($name:literal,$slot:literal,$offset:literal,$scratch:literal) => {
        global_asm!(concat!(
            ".globl ",
            $name,
            "\n.seh_proc ",
            $name,
            "\n",
            $name,
            ":\n",
            "mov [rsp+",
            $offset,
            "],rbx\n.seh_savereg rbx,",
            $offset,
            "\n.seh_endprologue\n",
            // The original save of RBX remains intact. Scratch uses OTHER
            // home slots, never saved RBX, original return or stack arguments.
            "mov [rsp+",
            $scratch,
            "],rax\nmov rax,[rip+NATIVE_CONSUMER_CONTINUE+",
            $slot,
            "]\n",
            "mov [rsp+0x18],rax\nmov rax,[rsp+",
            $scratch,
            "]\njmp qword ptr [rsp+0x18]\n.seh_endproc\n"
        ));
    };
}
original!("native_consumer_first_original", "0", "0x10", "8");
original!("native_consumer_second_original", "8", "0x10", "8");
original!("native_consumer_wait_original", "16", "8", "0x10");
global_asm!(
    ".globl native_consumer_arena_original",
    ".seh_proc native_consumer_arena_original",
    "native_consumer_arena_original:",
    "push rbp",
    ".seh_pushreg rbp",
    "push rbx",
    ".seh_pushreg rbx",
    "push rsi",
    ".seh_pushreg rsi",
    "lea rbp,[rsp-0x47]",
    ".seh_endprologue",
    "mov [rsp+0x20],rax",
    "mov rax,[rip+NATIVE_CONSUMER_CONTINUE+24]",
    "mov [rsp+0x28],rax",
    "mov rax,[rsp+0x20]",
    "jmp qword ptr [rsp+0x28]",
    ".seh_endproc",
);
unsafe extern "system" {
    fn native_consumer_first_original();
    fn native_consumer_second_original();
    fn native_consumer_wait_original();
    fn native_consumer_arena_original();
    fn native_consumer_first_gateway();
    fn native_consumer_second_gateway();
    fn native_consumer_wait_gateway();
    fn native_consumer_arena_gateway();
}
gateway!(
    "native_consumer_first_gateway",
    "50",
    native_consumer_first_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_consumer_second_gateway",
    "51",
    native_consumer_second_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_consumer_wait_gateway",
    "52",
    native_consumer_wait_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_consumer_arena_gateway",
    "53",
    native_consumer_arena_original,
    begin,
    end,
    unwind
);

const PROLOGUE: &[u8] = &[
    0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x74, 0x24, 0x18, 0x48, 0x89, 0x7c, 0x24, 0x20, 0x55,
    0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57, 0x48, 0x8b, 0xec, 0x48, 0x83, 0xec, 0x60,
];
const SITES: [(usize, [u8; 8], usize); 4] = [
    (
        0xe417d0,
        [0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x74],
        5,
    ),
    (
        0xe41c00,
        [0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x74],
        5,
    ),
    (0x986240, [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83], 5),
    (
        0xe3eaf0,
        [0x40, 0x55, 0x53, 0x56, 0x48, 0x8d, 0x6c, 0x24],
        9,
    ),
];
fn gateways() -> [usize; 4] {
    [
        native_consumer_first_gateway as *const () as usize,
        native_consumer_second_gateway as *const () as usize,
        native_consumer_wait_gateway as *const () as usize,
        native_consumer_arena_gateway as *const () as usize,
    ]
}
pub(super) unsafe fn prepare_host(exe: usize) -> Result<(), &'static str> {
    let _pin = super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA)?;
    unsafe {
        verify_host(exe)?;
        install_at(exe)
    }
}
pub(super) unsafe fn install(core: usize) -> Result<(), &'static str> {
    if HOOKS
        .lock()
        .map_err(|_| "consumer hook owner poisoned")?
        .is_empty()
    {
        unsafe {
            prepare_host(GetModuleHandleW(null_mut()) as usize)?;
        }
    }
    // Retain the identity validated before publication, including across the
    // later Core binding. A global module lookup does not replace its owner.
    let exe = EXE.load(Ordering::Acquire);
    let hooks = HOOKS.lock().map_err(|_| "consumer hook owner poisoned")?;
    if exe == 0
        || hooks.len() != SITES.len()
        || hooks.iter().zip(SITES).any(|(o, (rva, _, _))| {
            o.hook.target as u64 != exe + rva as u64
                || copy_current::<8>(o.hook.target as u64) != Some(o.hook.patched)
        })
    {
        return Err("consumer early installation differs");
    }
    CORE.store(core as u64, Ordering::Release);
    Ok(())
}
unsafe fn verify_host(exe: usize) -> Result<(), &'static str> {
    if !HOOKS
        .lock()
        .map_err(|_| "consumer hook owner poisoned")?
        .is_empty()
    {
        return Err("consumer already installed");
    }
    // Both complete consumer bodies, prologues and pinned unwind metadata
    // must match loaded memory before any relay is published.
    for (rva, len, hash) in [
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
        if !committed_range(exe + rva, len, 1, false) {
            return Err("consumer code range");
        }
        let code = unsafe { std::slice::from_raw_parts((exe + rva) as *const u8, len) };
        if !complete_code_matches(code, hash) {
            return Err("consumer complete code/prologue differs");
        }
    }
    let expected = [
        0x01, 0x1f, 0x0c, 0, 0x1f, 0x74, 0x15, 0, 0x1f, 0x64, 0x14, 0, 0x1f, 0x34, 0x13, 0, 0x1f,
        0xb2, 0x18, 0xf0, 0x16, 0xe0, 0x14, 0xd0, 0x12, 0xc0, 0x10, 0x50,
    ];
    if copy_current::<28>((exe + 0x123a8f8) as u64) != Some(expected) {
        return Err("consumer unwind differs");
    }
    for (slot, target) in [(0x11206a8, 0xe42470), (0x11206e0, 0xe427f0)] {
        if copy_current::<8>((exe + slot) as u64).map(u64::from_le_bytes)
            != Some((exe + target) as u64)
        {
            return Err("consumer scheduler +30 disk table differs");
        }
    }
    for (rva, expected) in [
        (
            0xe3eb27,
            &[
                0x48, 0x8b, 0x06, 0x48, 0x39, 0x43, 0x20, 0x0f, 0x85, 0x7d, 0, 0, 0,
            ][..],
        ),
        (
            0x986268,
            &[
                0x48, 0x8b, 0xcb, 0xe8, 0x80, 0x88, 0x4b, 0, 0x48, 0xc7, 0x47, 8, 0, 0, 0, 0,
            ][..],
        ),
    ] {
        if !expected
            .iter()
            .enumerate()
            .all(|(i, b)| copy_current::<1>((exe + rva + i) as u64) == Some([*b]))
        {
            return Err("consumer Wait branch/call differs");
        }
    }
    // Exact pdata rows for this loaded module. The scheduler has no
    // incoming stack argument beyond4: RBP=entry RSP-28h, so +30h is home+8.
    for (entry, end, unwind) in [
        (0xe417d0, 0xe41bf1, 0x123a8f8),
        (0xe41c00, 0xe41fae, 0x123a8f8),
    ] {
        let mut module = 0;
        let function = unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry(
                (exe + entry) as u64,
                &mut module,
                null_mut(),
            )
        };
        let row = copy_current::<12>(function as u64).ok_or("consumer pdata unreadable")?;
        if module != exe as u64
            || u32::from_le_bytes(row[..4].try_into().unwrap()) as usize != entry
            || u32::from_le_bytes(row[4..8].try_into().unwrap()) != end
            || u32::from_le_bytes(row[8..12].try_into().unwrap()) != unwind
        {
            return Err("consumer complete pdata differs");
        }
    }
    EXE.store(exe as u64, Ordering::Release);
    Ok(())
}
fn complete_code_matches(code: &[u8], sha: &str) -> bool {
    code.starts_with(PROLOGUE) && format!("{:x}", Sha256::digest(code)) == sha
}
unsafe fn install_at(exe: usize) -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "consumer hook owner poisoned")?;
    if !hooks.is_empty() {
        return Err("consumer already installed");
    }
    for (i, (rva, expected, stolen)) in SITES.into_iter().enumerate() {
        let p = (exe + rva) as *mut u8;
        if copy_current::<8>(p as u64) != Some(expected) {
            return Err("consumer entry differs");
        }
        if i == 3 && copy_current::<1>((p as u64) + 8) != Some([0xb9]) {
            return Err("consumer arena LEA differs");
        }
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(p.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
            || !(p as usize).is_multiple_of(8)
            || info.State != MEM_COMMIT
            || info.Protect & 0xff != PAGE_EXECUTE_READ
            || info.Protect & PAGE_GUARD != 0
        {
            return Err("consumer entry protection/alignment");
        }
        let relay = unsafe { executable_near(p as usize, p as usize, stolen) }
            .ok_or("consumer relay unavailable")?;
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
            return Err("consumer relay publication");
        }
        let mut patched = expected;
        patched[..5].copy_from_slice(
            &rel32_jump(p as usize, relay as usize).ok_or("consumer relay range")?,
        );
        patched[5..stolen.min(8)].fill(0x90);
        NATIVE_CONSUMER_CONTINUE[i].store((exe + rva + stolen) as u64, Ordering::Release);
        hooks.push(OwnedHook {
            hook: Hook {
                target: p,
                original: expected,
                patched,
                prefix: None,
                _code: relay,
            },
            protection: info.Protect,
        });
        unpublished.0 = null_mut();
        let r = unsafe {
            exchange_entry(p, u64::from_le_bytes(expected), u64::from_le_bytes(patched))
        }?;
        if !r.exchanged
            || !r.protection_restored
            || !r.cache_flushed
            || copy_current::<8>(p as u64) != Some(patched)
        {
            return Err("critical: consumer publication unverified");
        }
    }
    Ok(())
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "consumer hook owner poisoned")?;
    hooks.retain(|owned| {
        let h = &owned.hook;
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
        let protected =
            unsafe { VirtualProtect(h.target.cast(), 8, owned.protection, &mut old) } != 0;
        let flushed =
            unsafe { FlushInstructionCache(GetCurrentProcess(), h.target.cast(), 8) } != 0;
        !(restored && protected && flushed)
    });
    if hooks.is_empty() {
        Ok(())
    } else {
        Err("critical: consumer restoration unverified")
    }
}

#[cfg(test)]
#[path = "native_consumers_tests.rs"]
mod tests;
