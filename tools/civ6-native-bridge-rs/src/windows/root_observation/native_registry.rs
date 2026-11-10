//! Native registry removal owns terminal Session tombstones through full return.
//! Transport disconnect alone never terminates a registry/session incarnation.
use super::*;
use std::cell::RefCell;
#[path = "native_registry_fixed.rs"]
mod fixed;
#[path = "native_registry_ledger.rs"]
mod ledger;
use ledger::{Ledger, Root, Target};
static EXE: AtomicU64 = AtomicU64::new(0);
static NEXT: AtomicU64 = AtomicU64::new(0);
static STATE: std::sync::LazyLock<Mutex<Ledger>> =
    std::sync::LazyLock::new(|| Mutex::new(Ledger::default()));
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
pub(super) fn hook_entries_owned() -> bool {
    owns_hook_entries(&HOOKS, SITES.len())
}

#[unsafe(no_mangle)]
static NATIVE_REGISTRY_CONTINUE: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
struct Call {
    serial: u64,
    kind: u64,
    registry: u64,
    lifetime: u64,
    id: u32,
    session: u64,
    root: Option<Root>,
    target: Option<Target>,
}
struct Destroy {
    token: Root,
    registry: u64,
    lifetime: u64,
}
thread_local! {
    static CALLS:RefCell<Vec<Call>>=const{RefCell::new(Vec::new())};
    static DESTROY:RefCell<Vec<Destroy>>=const{RefCell::new(Vec::new())};
}
fn state() -> std::sync::MutexGuard<'static, Ledger> {
    STATE.lock().unwrap_or_else(|_| fault())
}
#[track_caller]
fn fault() -> ! {
    census::fault(
        census::module(module_path!()),
        std::panic::Location::caller().line(),
    );
    diagnostic(113);
    if let Ok(mut s) = STATE.try_lock() {
        s.unresolved()
    }
    super::unresolved(None);
    std::process::abort()
}
fn serial() -> u64 {
    NEXT.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
        .unwrap_or_else(|_| fault())
        + 1
}
fn exe(rva: u64) -> u64 {
    EXE.load(Ordering::Acquire) + rva
}
fn caller(raw: &Saved, rva: u64) -> bool {
    #[cfg(test)]
    if let Some(address) = tests::caller_override(rva) {
        return raw.invocation.caller == address;
    }
    raw.invocation.caller == exe(rva)
}
/// Snapshot exactly the same native bucket/key, without calling the original.
fn native_slot(registry: u64, id: u32) -> Result<(u64, u64), Error> {
    let n = read32(registry + 0x18)?;
    if n == 0 || n > 1_048_576 {
        return Err(Error::Unbound);
    }
    let buckets = read64(registry + 0x10)?;
    let sentinel = read64(buckets + u64::from(n) * 8)?;
    let mut node = read64(buckets + u64::from(id % n) * 8)?;
    let mut seen = std::collections::HashSet::new();
    for _ in 0..4096 {
        if node == 0 || node == sentinel {
            return Ok((0, 0));
        }
        if !seen.insert(node) {
            return Err(Error::Unbound);
        }
        if read32(node)? == id {
            return Ok((node, read64(node + 8)?));
        }
        node = read64(node + 0x10)?;
    }
    Err(Error::Unbound)
}
fn start_root(registry: u64, lifetime: u64, mut targets: Vec<Target>) -> Result<Root, Error> {
    for t in &mut targets {
        if native_slot(registry, t.id)?.1 != t.session {
            return Err(Error::Stale);
        }
        if let Some(transport) = t.transport {
            transports::terminal_closing(t.lease, transport)?;
        } else if read64(t.session + 0x28).ok() != Some(0)
            || super::state().unproven_transport_marker(t.lease).is_some()
        {
            t.unproven = Some(transports::prepare_terminal_unproven(t.lease, t.session)?);
        }
    }
    state().begin(registry, lifetime, targets)
}
pub(super) fn destroy_begin(registry: u64, lifetime: u64) -> Result<u64, Error> {
    let targets = super::terminal_registry_begin(registry, lifetime)?
        .into_iter()
        .map(|(lease, id, session, transport)| Target {
            lease,
            id,
            session,
            transport,
            unproven: None,
        })
        .collect();
    let token = start_root(registry, lifetime, targets)?;
    DESTROY.with(|s| {
        s.borrow_mut().push(Destroy {
            token,
            registry,
            lifetime,
        })
    });
    Ok(token.0)
}
pub(super) fn destroy_end(token: u64, exceptional: bool) -> Result<(), Error> {
    let d = DESTROY
        .with(|s| s.borrow_mut().pop())
        .ok_or(Error::Unknown)?;
    if d.token != Root(token) {
        return Err(Error::Stale);
    }
    if exceptional {
        if state().only_unproven(d.token) {
            return abandon_unproven(d.token);
        }
        state().unresolved();
        super::unresolved(None);
        return Ok(());
    }
    let targets = state().finish(d.token, d.registry, d.lifetime)?;
    for target in targets {
        retire_target(target)?;
    }
    if super::terminal_registry_end(d.registry, d.lifetime) == Err(Error::Unresolved) {
        super::state().retired_registry_metadata(d.registry, d.lifetime)
    } else {
        super::terminal_registry_end(d.registry, d.lifetime)
    }
}
fn retire_target(target: Target) -> Result<(), Error> {
    if target.unproven.is_some() {
        let result = super::state().retire_unproven_native(target.lease);
        if result.is_ok() {
            census::event(census::SemanticEvent::UnprovenSessionRetired);
        }
        result
    } else {
        super::terminal_remove_end(target.lease)
    }
}
fn abandon_unproven(root: Root) -> Result<(), Error> {
    let targets = state().abandon_unproven(root)?;
    for target in targets {
        if let Some(token) = target.unproven {
            transports::unproven_disconnect_end(token, true)?;
            super::state().unproven_native_unwind(target.lease)?;
        }
    }
    Ok(())
}
fn current_target() -> Option<(Root, Target)> {
    CALLS.with(|s| {
        s.borrow()
            .iter()
            .rev()
            .find(|c| matches!(c.kind, 114 | 115))
            .and_then(|c| c.root.zip(c.target))
    })
}
pub(super) fn join_candidate(thread: u64) -> Option<(SessionLease, SessionTransport)> {
    let (_, target) = current_target()?;
    let t = target.transport?;
    (t.thread == thread).then_some((target.lease, t))
}
pub(super) fn unproven_join_candidate(thread: u64) -> bool {
    current_target()
        .and_then(|(_, target)| target.unproven)
        .is_some_and(|token| token.transport.thread == thread)
}
pub(super) fn join_returned(
    lease: SessionLease,
    t: SessionTransport,
    thread: u64,
    al: u8,
) -> Result<(), Error> {
    let (root, target) = current_target().ok_or(Error::Unknown)?;
    if target.lease != lease || target.transport != Some(t) {
        return Err(Error::Stale);
    }
    transports::join_returned(lease, t, thread, al)?;
    state().join(root, target.session, thread, al)?;
    Ok(())
}
extern "system" fn begin(raw: *mut Saved) {
    let e = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        let mut c = Call {
            serial: serial(),
            kind: raw.invocation.kind,
            registry: 0,
            lifetime: 0,
            id: 0,
            session: raw.gpr[0],
            root: None,
            target: None,
        };
        match c.kind {
            113 => {
                c.registry = raw.gpr[0];
                c.id = raw.gpr[1] as u32;
                let lifetime = REGISTRIES
                    .lock()
                    .unwrap_or_else(|_| fault())
                    .get(&c.registry)
                    .copied();
                if let Some(lifetime) = lifetime {
                    let (_, session) = native_slot(c.registry, c.id).unwrap_or_else(|_| fault());
                    c.session = session;
                    c.lifetime = lifetime;
                    if session != 0 {
                        if let Some((lease, transport)) =
                            super::terminal_remove_begin(c.registry, lifetime, c.id, session)
                                .unwrap_or_else(|_| fault())
                        {
                            let target = Target {
                                lease,
                                id: c.id,
                                session,
                                transport,
                                unproven: None,
                            };
                            c.root = Some(
                                start_root(c.registry, lifetime, vec![target])
                                    .unwrap_or_else(|_| fault()),
                            );
                            c.target = Some(target);
                        }
                    }
                }
            }
            114 | 115 => {
                let root = if caller(raw, 0x7f929a) {
                    CALLS.with(|s| {
                        s.borrow()
                            .last()
                            .filter(|c| c.kind == 113 && c.session == raw.gpr[0])
                            .and_then(|c| c.root)
                    })
                } else if caller(raw, 0x7f8375) {
                    DESTROY.with(|s| s.borrow().last().map(|d| d.token))
                } else {
                    None
                };
                if let Some(root) = root {
                    c.root = Some(root);
                    c.target = state().enter(root, raw.gpr[0]).unwrap_or_else(|_| fault());
                    if c.target.is_some() {
                        let table = read64(raw.gpr[0]).unwrap_or_else(|_| fault());
                        let allowed = if c.kind == 114 {
                            [exe(0x1043088), exe(0x1045bd8)]
                        } else {
                            [exe(0x10436e0), exe(0x10436e0)]
                        };
                        if raw.gpr[1] as u32 != 1 || !allowed.contains(&table) {
                            fault()
                        }
                    }
                }
            }
            _ => fault(),
        }
        raw.invocation.cookie = COOKIE;
        raw.invocation.token = c.serial;
        CALLS.with(|s| s.borrow_mut().push(c));
    }))
    .is_err()
    {
        fault()
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
    let c = CALLS
        .with(|s| s.borrow_mut().pop())
        .unwrap_or_else(|| fault());
    if c.serial != raw.invocation.token || c.kind != raw.invocation.kind {
        fault()
    }
    let Some(root) = c.root else { return };
    if exceptional {
        let unproven = state().only_unproven(root);
        if unproven {
            abandon_unproven(root).unwrap_or_else(|_| fault());
            return;
        }
        state().unresolved();
        super::unresolved(c.target.map(|t| t.lease));
        return;
    }
    let result = if c.kind == 113 {
        if native_slot(c.registry, c.id).unwrap_or_else(|_| fault()).1 != 0 {
            fault()
        }
        let targets = state()
            .finish(root, c.registry, c.lifetime)
            .unwrap_or_else(|_| fault());
        if targets.len() != 1 {
            fault()
        }
        retire_target(targets[0])
    } else if let Some(target) = c.target {
        if raw.gpr[4] != target.session {
            fault()
        }
        if let Some(token) = target.unproven {
            transports::unproven_disconnect_end(token, false).unwrap_or_else(|_| fault());
        } else if let Some(t) = target.transport {
            transports::disconnect_end(target.lease, t, false).unwrap_or_else(|_| fault());
        }
        state().returned(root, target.session).map(|_| ())
    } else {
        Ok(())
    };
    if result.is_err() {
        fault()
    }
}
extern "system" fn end(raw: *mut Saved) {
    let e = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        finish(unsafe { &mut *raw }, false)
    }))
    .is_err()
    {
        fault()
    }
    unsafe { SetLastError(e) }
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
        super::unresolved(None)
    }
    unsafe { SetLastError(e) };
    1
}
macro_rules! home_original {
    ($name:literal,$slot:literal,$offset:literal,$scratch:literal,$jump:literal) => {
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
            "\n.seh_endprologue\n.globl ",
            $name,
            "_body\n",
            $name,
            "_body:\nmov [rsp+",
            $scratch,
            "],rax\nmov rax,[rip+NATIVE_REGISTRY_CONTINUE+",
            $slot,
            "]\nmov [rsp+",
            $jump,
            "],rax\nmov rax,[rsp+",
            $scratch,
            "]\njmp qword ptr [rsp+",
            $jump,
            "]\n.seh_endproc\n"
        ));
    };
}

home_original!("registry_remove_original", "0", "8", "0x10", "0x18");
home_original!("registry_session_original", "8", "8", "0x10", "0x18");
home_original!("registry_kind4_original", "16", "8", "0x10", "0x18");
unsafe extern "system" {
    fn registry_remove_original();
    fn registry_session_original();
    fn registry_kind4_original();
    fn registry_remove_gateway();
    fn registry_session_gateway();
    fn registry_kind4_gateway();
}
gateway!(
    "registry_remove_gateway",
    "113",
    registry_remove_original,
    begin,
    end,
    unwind
);
gateway!(
    "registry_session_gateway",
    "114",
    registry_session_original,
    begin,
    end,
    unwind
);
gateway!(
    "registry_kind4_gateway",
    "115",
    registry_kind4_original,
    begin,
    end,
    unwind
);
const SITES: [(usize, [u8; 8], usize); 3] = [
    (0x7f9220, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x74], 5),
    (0x836a90, [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83], 5),
    (0x81bd80, [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83], 5),
];
fn gateways() -> [usize; 3] {
    [
        registry_remove_gateway as *const () as usize,
        registry_session_gateway as *const () as usize,
        registry_kind4_gateway as *const () as usize,
    ]
}
pub(super) unsafe fn prepare_host(module: usize) -> Result<(), &'static str> {
    let _pin = super::super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA)?;
    unsafe {
        fixed::verify(module)?;
        install_at(module, &SITES)
    }
}
unsafe fn install_at(
    module: usize,
    sites: &[(usize, [u8; 8], usize); 3],
) -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "registry hook owner")?;
    if !hooks.is_empty() {
        return Err("registry hooks already installed");
    }
    for &(rva, expected, _) in sites {
        if copy_current::<8>((module + rva) as u64) != Some(expected) {
            return Err("registry entry differs");
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
            return Err("registry entry protection");
        }
        let relay = unsafe { executable_near(target as usize, target as usize, stolen) }
            .ok_or("registry relay")?;
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
            return Err("registry relay publication");
        }
        let mut patched = expected;
        patched[..5].copy_from_slice(
            &rel32_jump(target as usize, relay as usize).ok_or("registry relay range")?,
        );
        patched[5..stolen.min(8)].fill(0x90);
        NATIVE_REGISTRY_CONTINUE[i].store((module + rva + stolen) as u64, Ordering::Release);
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
            return Err("critical: registry publication unverified");
        }
    }
    Ok(())
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "registry hook owner")?;
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
    if hooks.is_empty() {
        Ok(())
    } else {
        Err("critical: registry restoration unverified")
    }
}

#[cfg(test)]
#[path = "native_registry_tests.rs"]
mod tests;
