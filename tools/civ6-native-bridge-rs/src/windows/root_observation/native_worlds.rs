//! Native Context -> CGame -> World births before their original publication.
//! These identities do not identify a network session or certify its seal.
use super::*;
use std::collections::HashMap;

static CORE: AtomicU64 = AtomicU64::new(0);
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
pub(super) fn hook_entries_owned() -> bool {
    owns_hook_entries(&HOOKS, SITES.len())
}

#[unsafe(no_mangle)]
static NATIVE_WORLD_CONTINUE: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WorldLease {
    pub epoch: u64,
    pub serial: u64,
    pub context: u64,
    pub game: u64,
    pub world: u64,
}
/// Captured at native Session birth, while no World is live. It describes
/// the next unpublished World, not a lookup of a later current World.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BirthWindow {
    serial: u64,
    context: u64,
    game: u64,
    epoch: u64,
}
impl BirthWindow {
    pub(super) fn matches(self, world: WorldLease) -> bool {
        world.serial == self.serial
            && world.epoch != 0
            && world.context != 0
            && world.game != 0
            && world.world != 0
            && (self.context == 0
                || (world.context == self.context
                    && world.game == self.game
                    && world.epoch == self.epoch))
    }
}
struct ContextBirth {
    serial: u64,
    epoch: u64,
    game: u64,
    world: Option<WorldLease>,
    closing: bool,
    closed: bool,
}
#[derive(Default)]
struct Bindings {
    next: u64,
    contexts: HashMap<u64, ContextBirth>,
    unobserved_close: bool,
    unobserved_birth: bool,
}
static BINDINGS: std::sync::LazyLock<Mutex<Bindings>> =
    std::sync::LazyLock::new(|| Mutex::new(Bindings::default()));
#[track_caller]
fn fault() -> ! {
    census::fault(
        census::module(module_path!()),
        std::panic::Location::caller().line(),
    );
    diagnostic(60);
    FAILED.store(true, Ordering::Release);
    TRACE.0[6].store(1, Ordering::Release);
    if let Some(f) = FRAMES.get() {
        f.mark_unresolved();
    }
    std::process::abort()
}
fn value(address: u64) -> u64 {
    copy_current::<8>(address)
        .map(u64::from_le_bytes)
        .unwrap_or_else(|| fault())
}
impl Bindings {
    fn coverage_gap(&self) -> bool {
        self.unobserved_close || self.unobserved_birth
    }
    fn birth_window(&self) -> Option<BirthWindow> {
        if self.coverage_gap() {
            return None;
        }
        let live: Vec<_> = self.contexts.iter().filter(|(_, b)| !b.closed).collect();
        if live.is_empty() {
            return Some(BirthWindow {
                serial: self.next.checked_add(1)?,
                context: 0,
                game: 0,
                epoch: 0,
            });
        }
        if let [(context, b)] = live.as_slice() {
            if !b.closing && b.world.is_none() {
                return Some(BirthWindow {
                    serial: b.serial,
                    context: **context,
                    game: b.game,
                    epoch: b.epoch,
                });
            }
        }
        None
    }
    fn context(&mut self, context: u64, game: u64, epoch: u64) -> Result<(), &'static str> {
        if context == 0
            || game == 0
            || epoch == 0
            || self.contexts.get(&context).is_some_and(|b| !b.closed)
            || self.contexts.values().any(|b| b.game == game && !b.closed)
        {
            return Err("unretired Context/Game address");
        }
        self.next = self.next.checked_add(1).ok_or("World identity exhausted")?;
        self.contexts.insert(
            context,
            ContextBirth {
                serial: self.next,
                epoch,
                game,
                world: None,
                closing: false,
                closed: false,
            },
        );
        Ok(())
    }
    fn world(&mut self, game: u64, world: u64, epoch: u64) -> Result<WorldLease, &'static str> {
        if world == 0
            || self
                .contexts
                .values()
                .any(|b| b.world.is_some_and(|w| w.world == world) && !b.closed)
        {
            return Err("unretired World address");
        }
        let Some((context, b)) = self
            .contexts
            .iter_mut()
            .find(|(_, b)| b.game == game && !b.closed && !b.closing)
        else {
            return Err("unborn Game");
        };
        if b.epoch != epoch || b.world.is_some() {
            return Err("World generation/owner differs");
        }
        let lease = WorldLease {
            epoch,
            serial: b.serial,
            context: *context,
            game,
            world,
        };
        b.world = Some(lease);
        Ok(lease)
    }
    fn close(&mut self, game: u64) -> Option<WorldLease> {
        let b = self
            .contexts
            .values_mut()
            .find(|b| b.game == game && !b.closed)?;
        if b.closing {
            return None;
        }
        b.closing = true;
        b.world
    }
    fn closed(&mut self, game: u64, serial: u64) -> bool {
        let Some(b) = self
            .contexts
            .values_mut()
            .find(|b| b.game == game && b.serial == serial && b.closing && !b.closed)
        else {
            return false;
        };
        b.closed = true;
        true
    }
    fn closing(&self, world: u64) -> Option<WorldLease> {
        self.contexts
            .values()
            .filter(|b| b.closing && !b.closed)
            .find_map(|b| b.world.filter(|w| w.world == world))
    }
}
pub(super) fn closing(world: u64) -> Option<WorldLease> {
    BINDINGS.lock().ok()?.closing(world)
}
pub(super) fn birth(world: u64) -> Option<WorldLease> {
    BINDINGS
        .lock()
        .ok()?
        .contexts
        .values()
        .filter(|b| !b.closed)
        .find_map(|b| b.world.filter(|w| w.world == world))
}
pub(super) fn installed() -> bool {
    CORE.load(Ordering::Acquire) != 0
}
pub(super) fn completed(lease: WorldLease) -> bool {
    BINDINGS.lock().is_ok_and(|bindings| {
        !bindings.coverage_gap()
            && bindings.contexts.get(&lease.context).is_some_and(|birth| {
                birth.serial == lease.serial
                    && birth.epoch == lease.epoch
                    && birth.game == lease.game
                    && birth.world == Some(lease)
                    && birth.closing
                    && birth.closed
            })
    })
}
/// Missing birth evidence denies a closure certificate. It does not mean an
/// owned executing frame or reservation was lost, and cannot poison ordinary
/// native frame accounting into an unrelated process abort.
pub(super) fn unobserved_close() {
    BINDINGS.lock().unwrap_or_else(|_| fault()).unobserved_close = true;
    diagnostic(2062);
}
fn unobserved_birth() {
    BINDINGS.lock().unwrap_or_else(|_| fault()).unobserved_birth = true;
    diagnostic(2063);
}
pub(super) fn capture_birth_window() -> Option<BirthWindow> {
    if !HOST_PREPARED.load(Ordering::Acquire) {
        return None;
    }
    BINDINGS.lock().ok()?.birth_window()
}
#[cfg(test)]
pub(super) fn fixture_first_birth_window() -> BirthWindow {
    Bindings::default().birth_window().unwrap()
}
#[cfg(test)]
pub(super) fn fixture_cold_observer(core: u64) {
    assert_eq!(CORE.swap(core, Ordering::AcqRel), 0);
}
#[cfg(test)]
pub(super) fn fixture_world_birth() -> WorldLease {
    let mut bindings = BINDINGS.lock().unwrap();
    bindings.context(10, 20, 1).unwrap();
    bindings.world(20, 30, 1).unwrap()
}
#[cfg(test)]
pub(super) fn fixture_world_close(lease: WorldLease) {
    let mut bindings = BINDINGS.lock().unwrap();
    assert_eq!(bindings.close(lease.game), Some(lease));
    assert!(bindings.closed(lease.game, lease.serial));
}

extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        let core = CORE.load(Ordering::Acquire);
        if matches!(raw.invocation.kind, 60 | 61)
            && BINDINGS.lock().unwrap_or_else(|_| fault()).coverage_gap()
        {
            // Missing lifetime evidence denies new tickets. Continue the
            // original engine path without adopting a newer birth instead.
            return;
        }
        match raw.invocation.kind {
            60 => {
                if raw.invocation.caller != core + 0x58fcc3 {
                    return;
                }
                raw.invocation.world = raw.gpr[0];
            }
            61 => {
                if raw.invocation.caller != core + 0x46d30 {
                    unobserved_birth();
                    return;
                }
                raw.invocation.manager = raw.gpr[0];
                if copy_current::<8>(raw.gpr[0]).map(u64::from_le_bytes) != Some(core + 0xa44f70)
                    || copy_current::<8>(raw.gpr[0] + 0x60).map(u64::from_le_bytes) != Some(0)
                {
                    unobserved_birth();
                    return;
                }
            }
            62 => {
                match copy_current::<1>(raw.gpr[0] + 0x40) {
                    Some([0]) => return,
                    Some([1]) => {}
                    _ => {
                        unobserved_close();
                        return;
                    }
                }
                if copy_current::<8>(raw.gpr[0]).map(u64::from_le_bytes) != Some(core + 0xa44f70) {
                    unobserved_close();
                    return;
                }
                let mut b = BINDINGS.lock().unwrap_or_else(|_| fault());
                let Some(birth) = b
                    .contexts
                    .values()
                    .find(|b| b.game == raw.gpr[0] && !b.closed)
                else {
                    b.unobserved_close = true;
                    diagnostic(2062);
                    return;
                };
                raw.invocation.parent = birth.serial;
                raw.invocation.manager = raw.gpr[0];
                let world = value(raw.gpr[0] + 0x60);
                if world == 0 {
                    return;
                }
                let identity = b
                    .contexts
                    .values()
                    .find_map(|r| r.world.filter(|w| w.world == world));
                let Some(identity) = identity else {
                    // A previously issued World receipt cannot disappear.
                    // Without one, this is missing coverage, not corruption.
                    if birth.world.is_some() {
                        fault();
                    }
                    b.unobserved_close = true;
                    diagnostic(2062);
                    return;
                };
                if identity.game != raw.gpr[0] {
                    fault();
                }
                let lease = b.close(raw.gpr[0]).unwrap_or_else(|| fault());
                if lease.world != world {
                    fault();
                }
                raw.invocation.world = world;
                raw.invocation.epoch = lease.epoch;
                raw.invocation.cookie = COOKIE;
                return;
            }
            _ => fault(),
        }
        let f = FRAMES.get().unwrap_or_else(|| fault());
        let (hold, epoch) = f.begin_native_birth().unwrap_or_else(|_| fault());
        raw.invocation.epoch = epoch;
        raw.invocation.token = hold;
        raw.invocation.cookie = COOKIE;
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
    if exceptional {
        if let Some(f) = FRAMES.get() {
            f.mark_unresolved();
        }
        FAILED.store(true, Ordering::Release);
    }
    if raw.invocation.kind == 62 {
        if !exceptional
            && !BINDINGS
                .lock()
                .unwrap_or_else(|_| fault())
                .closed(raw.invocation.manager, raw.invocation.parent)
        {
            fault();
        }
        return;
    }
    if !exceptional {
        let core = CORE.load(Ordering::Acquire);
        let mut b = BINDINGS.lock().unwrap_or_else(|_| fault());
        if b.coverage_gap() {
            // Another boundary may have become unobserved while this original
            // ran. Settle the owned frame below without issuing a birth ticket.
        } else if raw.invocation.kind == 60 {
            if raw.gpr[4] != raw.invocation.world || value(raw.gpr[4]) != core + 0xa101e8 {
                fault();
            }
            let game = value(raw.gpr[4] + 8);
            if value(game) != core + 0xa44f70 {
                fault();
            }
            b.context(raw.gpr[4], game, raw.invocation.epoch)
                .unwrap_or_else(|_| fault());
        } else {
            let world = raw.gpr[4];
            if value(world) != core + 0xa45160 || value(world + 0x270) != raw.invocation.manager {
                fault();
            }
            let lease = b
                .world(raw.invocation.manager, world, raw.invocation.epoch)
                .unwrap_or_else(|_| fault());
            // The native load carrier, if present, was retained before the
            // asynchronous command selected this view. Current globals alone
            // never create a Session/World relation.
            loads::world_created(lease);
        }
    }
    FRAMES
        .get()
        .unwrap_or_else(|| fault())
        .leave(raw.invocation.token)
        .unwrap_or_else(|_| fault());
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
    r: *const EXCEPTION_RECORD,
    frame: usize,
    _: *mut CONTEXT,
    _: *mut c_void,
) -> i32 {
    let error = unsafe { GetLastError() };
    if !r.is_null()
        && unsafe { (*r).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0
        && std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            finish(unsafe { &mut *((frame + 0x20) as *mut Saved) }, true)
        }))
        .is_err()
    {
        FAILED.store(true, Ordering::Release);
        if let Some(f) = FRAMES.get() {
            f.mark_unresolved();
        }
    }
    unsafe { SetLastError(error) };
    1
}
macro_rules! original {
    ($name:literal,$slot:literal,$stack:literal) => {
        global_asm!(concat!(
            ".globl ",
            $name,
            "\n.seh_proc ",
            $name,
            "\n",
            $name,
            ":\npush rbx\n.seh_pushreg rbx\nsub rsp,",
            $stack,
            "\n.seh_stackalloc ",
            $stack,
            "\n.seh_endprologue\n",
            "mov [rsp+0x10],rax\nmov rax,[rip+NATIVE_WORLD_CONTINUE+",
            $slot,
            "]\nmov [rsp+0x18],rax\nmov rax,[rsp+0x10]\njmp qword ptr [rsp+0x18]\n.seh_endproc\n"
        ));
    };
}
original!("native_world_context_original", "0", "0x30");
original!("native_world_factory_original", "8", "0x30");
original!("native_world_close_original", "16", "0x20");
unsafe extern "system" {
    fn native_world_context_original();
    fn native_world_factory_original();
    fn native_world_close_original();
    fn native_world_context_gateway();
    fn native_world_factory_gateway();
    fn native_world_close_gateway();
}
gateway!(
    "native_world_context_gateway",
    "60",
    native_world_context_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_world_factory_gateway",
    "61",
    native_world_factory_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_world_close_gateway",
    "62",
    native_world_close_original,
    begin,
    end,
    unwind
);
const SITES: [(usize, [u8; 8]); 3] = [
    (0x58fb40, [0x40, 0x53, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8d]),
    (0x7504c0, [0x40, 0x53, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8b]),
    (0x470b0, [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x80, 0x79]),
];
pub(super) unsafe fn install(core: usize) -> Result<(), &'static str> {
    for (rva, len, sha) in [
        (
            0x58fb40,
            0x74,
            "513c4b78c9f6adbd4ad3eb9a0daa6ce8cc1696aab01bb2cdea23c679a75eab08",
        ),
        (
            0x7504c0,
            0x52,
            "e6bac7ba15b93527c1c8c1f48f96b7899d2392b55d0c16cac89c2b842afb590c",
        ),
    ] {
        if !committed_range(core + rva, len, 1, false) {
            return Err("World complete function range");
        }
        let bytes = unsafe { std::slice::from_raw_parts((core + rva) as *const u8, len) };
        if format!("{:x}", Sha256::digest(bytes)) != sha {
            return Err("World complete function differs");
        }
    }
    if copy_current::<8>((core + 0xac387c) as u64) != Some([1, 6, 2, 0, 6, 0x52, 2, 0x30]) {
        return Err("World birth unwind differs");
    }
    unsafe { install_at(core) }
}
fn entry_protection(core: usize, rva: usize, expected: [u8; 8]) -> Result<u32, &'static str> {
    let entry = (core + rva) as *mut u8;
    if copy_current::<8>(entry as u64) != Some(expected) {
        return Err("World birth entry differs");
    }
    let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { VirtualQuery(entry.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
        || !entry.addr().is_multiple_of(8)
        || info.State != MEM_COMMIT
        || info.Protect & PAGE_GUARD != 0
        || info.Protect & 0xff != PAGE_EXECUTE_READ
    {
        return Err("World entry protection");
    }
    Ok(info.Protect)
}
unsafe fn install_at(core: usize) -> Result<(), &'static str> {
    let callbacks = [
        native_world_context_gateway as *const () as usize,
        native_world_factory_gateway as *const () as usize,
        native_world_close_gateway as *const () as usize,
    ];
    let mut hooks = HOOKS.lock().map_err(|_| "World hook owner")?;
    if !hooks.is_empty() {
        return Err("World hooks already installed");
    }
    for (rva, expected) in SITES {
        entry_protection(core, rva, expected)?;
    }
    // Every callback needs Core before its first entry can be published.
    // Preflight refusals above have changed neither Core nor native code.
    let previous_core = CORE.swap(core as u64, Ordering::AcqRel);
    let result = (|| -> Result<(), &'static str> {
        for (i, (rva, expected)) in SITES.into_iter().enumerate() {
            #[cfg(test)]
            tests::before_publication(core, i);
            let p = (core + rva) as *mut u8;
            let protection = entry_protection(core, rva, expected)?;
            let code =
                unsafe { executable_near(p as usize, p as usize, 6) }.ok_or("World relay")?;
            let mut unpublished = Unpublished(code);
            unsafe {
                ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), code, 6);
                ptr::copy_nonoverlapping(
                    (callbacks[i] as u64).to_le_bytes().as_ptr(),
                    code.add(6),
                    8,
                );
            }
            let mut old = 0;
            if unsafe { VirtualProtect(code.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
                || unsafe { FlushInstructionCache(GetCurrentProcess(), code.cast(), 14) } == 0
            {
                return Err("World relay publication");
            }
            let mut patch = expected;
            patch[..5].copy_from_slice(
                &rel32_jump(p as usize, code as usize).ok_or("World relay range")?,
            );
            patch[5] = 0x90;
            NATIVE_WORLD_CONTINUE[i].store((core + rva + 6) as u64, Ordering::Release);
            hooks.push(OwnedHook {
                hook: Hook {
                    target: p,
                    original: expected,
                    patched: patch,
                    prefix: None,
                    _code: code,
                },
                protection,
            });
            unpublished.0 = null_mut();
            #[cfg(test)]
            tests::before_exchange(core, i);
            let r = unsafe {
                exchange_entry(p, u64::from_le_bytes(expected), u64::from_le_bytes(patch))
            }?;
            if !r.exchanged || !r.cache_flushed || !r.protection_restored {
                return Err("critical: World hook publication");
            }
        }
        Ok(())
    })();
    // A retained owner may already have been fetched, even when publication
    // verification failed. Keep its Core and ordinary restore responsibility.
    if result.is_err() && hooks.is_empty() {
        CORE.store(previous_core, Ordering::Release);
    }
    result
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "World hook owner")?;
    hooks.retain(|o| {
        let h = &o.hook;
        let cur = copy_current::<8>(h.target as u64);
        let ok = cur == Some(h.original)
            || (cur == Some(h.patched)
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
    if hooks.is_empty() {
        Ok(())
    } else {
        Err("critical: World restore")
    }
}

#[cfg(test)]
#[path = "native_install_fixture.rs"]
pub(super) mod install_fixture;

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    #[test]
    fn entry_validation_rejects_unaligned_guarded_and_uncommitted_data() {
        install_fixture::rejects_invalid_entry_data(entry_protection);
    }
    static FAIL_SITE: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(usize::MAX);
    static FAIL_CAS_SITE: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(usize::MAX);
    pub(super) fn before_publication(core: usize, index: usize) {
        let failed = FAIL_SITE.load(Ordering::Acquire);
        if failed != usize::MAX || FAIL_CAS_SITE.load(Ordering::Acquire) != usize::MAX {
            assert_eq!(
                CORE.load(Ordering::Acquire),
                core as u64,
                "Core must be initialized before the first World CAS"
            );
            if index == failed {
                let (rva, mut bytes) = SITES[index];
                bytes[0] ^= 1;
                unsafe { install_fixture::write_at(core + rva, &bytes) };
            }
        }
    }
    pub(super) fn before_exchange(core: usize, index: usize) {
        if FAIL_CAS_SITE.load(Ordering::Acquire) == index {
            let (rva, mut bytes) = SITES[index];
            bytes[0] ^= 1;
            unsafe { install_fixture::write_at(core + rva, &bytes) };
        }
    }
    #[test]
    fn failed_world_cas_retains_core_and_all_published_owners() {
        if !child("failed_world_cas_retains_core_and_all_published_owners") {
            return;
        }
        let Some(_fixture) = crate::windows::atomic_fixture::require(
            "failed_world_cas_retains_core_and_all_published_owners",
        ) else {
            return;
        };
        let memory = install_fixture::Memory::new(&SITES);
        memory.protect_all(PAGE_EXECUTE_READ);
        FAIL_CAS_SITE.store(1, Ordering::Release);
        let result = unsafe { install_at(memory.base) };
        FAIL_CAS_SITE.store(usize::MAX, Ordering::Release);
        assert_eq!(result, Err("critical: World hook publication"));
        assert_eq!(CORE.load(Ordering::Acquire), memory.base as u64);
        assert_eq!(HOOKS.lock().unwrap().len(), 2);
        assert_ne!(memory.bytes(SITES[0].0), SITES[0].1);
        // Repair only the test's intervening write. Both retained records
        // still go through normal restore, including the failed CAS record.
        memory.write(SITES[1].0, &SITES[1].1);
        unsafe { restore() }.unwrap();
        assert!(HOOKS.lock().unwrap().is_empty());
        assert_eq!(memory.bytes(SITES[0].0), SITES[0].1);
    }
    #[test]
    fn partial_world_publication_keeps_initialized_core_until_restore() {
        if !child("partial_world_publication_keeps_initialized_core_until_restore") {
            return;
        }
        // A first recheck can fail after a successful preflight. No retained
        // entry means callback state can still return to its previous value.
        {
            let refused = install_fixture::Memory::new(&SITES);
            refused.protect_all(PAGE_EXECUTE_READ);
            FAIL_SITE.store(0, Ordering::Release);
            let result = unsafe { install_at(refused.base) };
            FAIL_SITE.store(usize::MAX, Ordering::Release);
            assert_eq!(result, Err("World birth entry differs"));
            assert_eq!(CORE.load(Ordering::Acquire), 0);
            assert!(HOOKS.lock().unwrap().is_empty());
        }
        let Some(_fixture) = crate::windows::atomic_fixture::require(
            "partial_world_publication_keeps_initialized_core_until_restore",
        ) else {
            return;
        };
        let memory = install_fixture::Memory::new(&SITES);
        memory.protect_all(PAGE_EXECUTE_READ);
        FAIL_SITE.store(1, Ordering::Release);
        let result = unsafe { install_at(memory.base) };
        FAIL_SITE.store(usize::MAX, Ordering::Release);
        assert_eq!(result, Err("World birth entry differs"));
        assert_eq!(CORE.load(Ordering::Acquire), memory.base as u64);
        assert_eq!(HOOKS.lock().unwrap().len(), 1);
        assert_ne!(memory.bytes(SITES[0].0), SITES[0].1);
        assert_eq!(
            unsafe { install_at(1) },
            Err("World hooks already installed")
        );
        assert_eq!(CORE.load(Ordering::Acquire), memory.base as u64);
        unsafe { restore() }.unwrap();
        assert!(HOOKS.lock().unwrap().is_empty());
        assert_eq!(memory.bytes(SITES[0].0), SITES[0].1);
    }
    #[test]
    fn preflight_rejection_keeps_world_core_and_entries_unchanged() {
        if !child("preflight_rejection_keeps_world_core_and_entries_unchanged") {
            return;
        }
        assert_eq!(CORE.load(Ordering::Acquire), 0);
        for (index, corrupted) in SITES.into_iter().map(Some).chain([None]).enumerate() {
            let memory = install_fixture::Memory::new(&SITES);
            if let Some((rva, mut expected)) = corrupted {
                expected[0] ^= 1;
                memory.write(rva, &expected);
            }
            memory.protect_all(PAGE_EXECUTE_READ);
            if corrupted.is_none() {
                memory.protect(SITES[1].0, PAGE_READWRITE);
            }
            let before = SITES.map(|(rva, _)| memory.bytes(rva));
            let result = unsafe { install_at(memory.base) };
            let core = CORE.load(Ordering::Acquire);
            let owners = HOOKS.lock().unwrap().len();
            let after = SITES.map(|(rva, _)| memory.bytes(rva));
            unsafe { restore() }.unwrap();
            CORE.store(0, Ordering::Release);
            assert!(result.is_err());
            assert_eq!(
                core, 0,
                "World preflight refusal published Core at case {index}"
            );
            assert_eq!(owners, 0, "World preflight refusal retained an entry");
            assert_eq!(after, before, "World preflight refusal patched an entry");
        }
    }
    #[test]
    fn session_birth_windows_name_only_the_next_unpublished_world() {
        let mut b = Bindings::default();
        let initial = b.birth_window().unwrap();
        b.context(10, 20, 1).unwrap();
        let pending = b.birth_window().unwrap();
        let first = b.world(20, 30, 1).unwrap();
        assert!(initial.matches(first) && pending.matches(first));
        assert!(b.birth_window().is_none());
        b.close(20).unwrap();
        assert!(b.birth_window().is_none(), "a closing World is still live");
        assert!(b.closed(20, first.serial));
        let next = b.birth_window().unwrap();
        assert!(!next.matches(first));
        b.context(10, 20, 2).unwrap();
        let second = b.world(20, 30, 2).unwrap();
        assert!(next.matches(second));
        assert!(!initial.matches(second) && !pending.matches(second));
    }
    fn child(name: &str) -> bool {
        const ENV: &str = "CIV6_NATIVE_WORLD_OWNED_CHILD";
        if std::env::var_os(ENV).is_some() {
            return true;
        }
        let mut p = std::process::Command::new(std::env::current_exe().unwrap())
            .creation_flags(0x08000000)
            .env(ENV, "1")
            .args([
                "--exact",
                &format!("windows::root_observation::native_frames::worlds::tests::{name}"),
                "--nocapture",
            ])
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(s) = p.try_wait().unwrap() {
                assert!(s.success(), "owned World fixture exited with {s}");
                break;
            }
            if Instant::now() >= deadline {
                let _ = p.kill();
                let _ = p.wait();
                panic!("World fixture timeout");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }
    #[test]
    fn reused_world_address_keeps_distinct_context_birth_and_closure() {
        let mut b = Bindings::default();
        b.context(10, 20, 1).unwrap();
        let old = b.world(20, 30, 1).unwrap();
        assert!(b.context(10, 20, 2).is_err());
        assert_eq!(b.close(20), Some(old));
        assert_eq!(b.closing(30), Some(old));
        assert!(!b.closed(20, old.serial + 1));
        assert!(b.closed(20, old.serial));
        assert_eq!(b.closing(30), None);
        b.context(10, 20, 2).unwrap();
        let new = b.world(20, 30, 2).unwrap();
        assert_ne!(new.serial, old.serial);
        assert_ne!(new.epoch, old.epoch);
        assert!(!b.closed(20, old.serial));
    }
    #[test]
    fn world_factory_never_adopts_an_unknown_game_or_another_epoch() {
        let mut b = Bindings::default();
        assert!(b.world(20, 30, 1).is_err());
        b.context(10, 20, 1).unwrap();
        assert!(b.world(20, 30, 2).is_err());
        b.world(20, 30, 1).unwrap();
        assert!(b.world(20, 40, 1).is_err());
    }
    #[test]
    fn unobserved_game_close_preserves_native_return_without_certifying_worlds() {
        if !child("unobserved_game_close_preserves_native_return_without_certifying_worlds") {
            return;
        }
        let frames = FRAMES.get_or_init(NativeFrames::new);
        let core = 0x180000000u64;
        CORE.store(core, Ordering::Release);
        let known = fixture_world_birth();
        fixture_world_close(known);
        assert!(completed(known));

        // Only an owned native stub executes. It counts calls, clears the
        // original's World field and preserves its full return value.
        let mut code = vec![0x48, 0xff, 0x41, 0x68, 0x48, 0xc7, 0x41, 0x60, 0, 0, 0, 0];
        code.extend([0x48, 0xb8]);
        code.extend(0x1122334455667788u64.to_le_bytes());
        code.extend([0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3]);
        let page =
            unsafe { VirtualAlloc(null_mut(), 4096, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) };
        assert!(!page.is_null());
        unsafe { ptr::copy_nonoverlapping(code.as_ptr(), page.cast(), code.len()) };
        let mut protection = 0;
        assert_ne!(
            unsafe { VirtualProtect(page, 4096, PAGE_EXECUTE_READ, &mut protection) },
            0
        );
        assert_ne!(
            unsafe { FlushInstructionCache(GetCurrentProcess(), page, code.len()) },
            0
        );
        NATIVE_WORLD_CONTINUE[2].store(page as u64, Ordering::Release);
        let close: unsafe extern "system" fn(*mut u64) -> u64 =
            unsafe { std::mem::transmute(native_world_close_gateway as *const ()) };
        let mut game = [0u64; 0x80 / 8];
        game[0] = core + 0xa44f70;
        // A native no-op without a World does not create a coverage gap.
        unsafe { SetLastError(0x12345678) };
        assert_eq!(unsafe { close(game.as_mut_ptr()) }, 0x1122334455667788);
        assert_eq!(unsafe { GetLastError() }, 0x12345678);
        assert_eq!(game[0x68 / 8], 1);
        assert!(completed(known));
        assert!(BINDINGS.lock().unwrap().birth_window().is_some());
        println!("UNOBSERVED_CLOSE_NOOP_PASSED");
        use std::io::Write;
        std::io::stdout().flush().unwrap();

        game[0x40 / 8] = 1;
        game[0x60 / 8] = 0x1234;
        unsafe { SetLastError(0x87654321) };
        assert_eq!(unsafe { close(game.as_mut_ptr()) }, 0x1122334455667788);
        assert_eq!(unsafe { GetLastError() }, 0x87654321);
        assert_eq!(game[0x68 / 8], 2, "each original runs exactly once");
        assert_eq!(game[0x60 / 8], 0);
        assert!(
            !completed(known),
            "an unseen lifetime denies all-World closure"
        );
        assert_eq!(TRACE.0[13].load(Ordering::Acquire), 2062);
        assert!(BINDINGS.lock().unwrap().birth_window().is_none());
        assert!(!FAILED.load(Ordering::Acquire));
        assert!(frames.healthy());
        // A coverage gap alone must not abort a later unrelated native frame.
        let mut later: Saved = unsafe { std::mem::zeroed() };
        super::super::begin(&mut later);
        super::super::finish(&mut later, false);
        assert_eq!(frames.observation_state_for_test().2, 0);
        assert!(!completed(known), "ordinary traffic cannot erase the gap");
        NATIVE_WORLD_CONTINUE[2].store(0, Ordering::Release);
        assert_ne!(unsafe { VirtualFree(page, 0, MEM_RELEASE) }, 0);
    }
    fn unproven_world_gateway_case(case: u8) {
        let frames = FRAMES.get_or_init(NativeFrames::new);
        let core = 0x180000000u64;
        CORE.store(core, Ordering::Release);
        let known = fixture_world_birth();
        fixture_world_close(known);
        let mut game = [0u64; 0x80 / 8];
        game[0] = if case == 1 {
            core + 0x123456
        } else {
            core + 0xa44f70
        };
        game[0x40 / 8] = 1;
        game[0x60 / 8] = if case == 0 { 0 } else { 0x1234 };
        if case == 2 {
            // Context/Game was observed, but this World was never observed.
            BINDINGS
                .lock()
                .unwrap()
                .context(0x1000, game.as_ptr() as u64, 2)
                .unwrap();
        }
        let mut code = vec![0x48, 0xff, 0x41, 0x68, 0x48, 0xb8];
        code.extend(0x1122334455667788u64.to_le_bytes());
        code.extend([
            0x48,
            0x83,
            0xc4,
            if case == 0 { 0x30 } else { 0x20 },
            0x5b,
            0xc3,
        ]);
        let page =
            unsafe { VirtualAlloc(null_mut(), 4096, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) };
        assert!(!page.is_null());
        unsafe { ptr::copy_nonoverlapping(code.as_ptr(), page.cast(), code.len()) };
        let mut protection = 0;
        assert_ne!(
            unsafe { VirtualProtect(page, 4096, PAGE_EXECUTE_READ, &mut protection) },
            0
        );
        assert_ne!(
            unsafe { FlushInstructionCache(GetCurrentProcess(), page, code.len()) },
            0
        );
        let slot = if case == 0 { 1 } else { 2 };
        NATIVE_WORLD_CONTINUE[slot].store(page as u64, Ordering::Release);
        let entry = if case == 0 {
            native_world_factory_gateway as *const ()
        } else {
            native_world_close_gateway as *const ()
        };
        let original: unsafe extern "system" fn(*mut u64) -> u64 =
            unsafe { std::mem::transmute(entry) };
        unsafe { SetLastError(0x87654321) };
        let returned = unsafe { original(game.as_mut_ptr()) };
        let error = unsafe { GetLastError() };
        let calls = game[0x68 / 8];
        NATIVE_WORLD_CONTINUE[slot].store(0, Ordering::Release);
        assert_ne!(unsafe { VirtualFree(page, 0, MEM_RELEASE) }, 0);
        assert_eq!(returned, 0x1122334455667788);
        assert_eq!(error, 0x87654321);
        assert_eq!(
            calls, 1,
            "unproven entry must still execute its original exactly once"
        );
        assert!(!completed(known));
        assert!(BINDINGS.lock().unwrap().birth_window().is_none());
        assert!(!FAILED.load(Ordering::Acquire));
        assert!(frames.healthy());
        // An unrelated ordinary frame must not inherit an uncertainty abort.
        let mut later: Saved = unsafe { std::mem::zeroed() };
        super::super::begin(&mut later);
        super::super::finish(&mut later, false);
        assert_eq!(frames.observation_state_for_test().2, 0);
    }
    #[test]
    fn unknown_world_factory_caller_keeps_original_and_denies_receipt() {
        if child("unknown_world_factory_caller_keeps_original_and_denies_receipt") {
            unproven_world_gateway_case(0)
        }
    }
    #[test]
    fn unrecognised_game_close_receiver_keeps_original_and_denies_receipt() {
        if child("unrecognised_game_close_receiver_keeps_original_and_denies_receipt") {
            unproven_world_gateway_case(1)
        }
    }
    #[test]
    fn observed_game_missing_world_keeps_original_and_denies_receipt() {
        if child("observed_game_missing_world_keeps_original_and_denies_receipt") {
            unproven_world_gateway_case(2)
        }
    }
    #[test]
    fn an_unobserved_boundary_settles_inflight_birth_without_poisoning_later_birth() {
        if !child("an_unobserved_boundary_settles_inflight_birth_without_poisoning_later_birth") {
            return;
        }
        let frames = FRAMES.get_or_init(NativeFrames::new);
        let core = 0x180000000u64;
        CORE.store(core, Ordering::Release);
        let mut original: Saved = unsafe { std::mem::zeroed() };
        original.invocation.kind = 60;
        original.invocation.caller = core + 0x58fcc3;
        original.gpr[0] = 0x1234;
        begin(&mut original);
        assert_eq!(original.invocation.cookie, COOKIE);
        unobserved_birth();
        // The output cannot be adopted after a concurrent coverage gap.
        // Its invalid address also detects an accidental post-return read.
        original.gpr[4] = 1;
        finish(&mut original, false);
        let mut later: Saved = unsafe { std::mem::zeroed() };
        later.invocation.kind = 61;
        later.invocation.caller = core + 0x46d30;
        later.gpr[0] = 1;
        begin(&mut later);
        assert_eq!(later.invocation.cookie, 0);
        finish(&mut later, false);
        assert!(BINDINGS.lock().unwrap().contexts.is_empty());
        assert!(capture_birth_window().is_none());
        assert!(!FAILED.load(Ordering::Acquire));
        assert!(frames.healthy());
        assert_eq!(frames.observation_state_for_test().2, 0);
    }
    #[test]
    fn original_context_and_world_return_bind_before_cgame_publication() {
        if !child("original_context_and_world_return_bind_before_cgame_publication") {
            return;
        }
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        let core = 0x180000000u64;
        CORE.store(core, Ordering::Release);
        let mut game = vec![0u64; 0x80 / 8];
        game[0] = core + 0xa44f70;
        game[0x40 / 8] = 1;
        let mut context = [core + 0xa101e8, game.as_mut_ptr() as u64];
        let mut c: Saved = unsafe { std::mem::zeroed() };
        c.invocation.kind = 60;
        c.invocation.caller = core + 0x58fcc3;
        c.gpr[0] = context.as_mut_ptr() as u64;
        begin(&mut c);
        c.gpr[4] = context.as_mut_ptr() as u64;
        finish(&mut c, false);
        let mut world = vec![0u64; 0x278 / 8];
        world[0] = core + 0xa45160;
        world[0x270 / 8] = game.as_mut_ptr() as u64;
        let mut w: Saved = unsafe { std::mem::zeroed() };
        w.invocation.kind = 61;
        w.invocation.caller = core + 0x46d30;
        w.gpr[0] = game.as_mut_ptr() as u64;
        begin(&mut w);
        w.gpr[4] = world.as_mut_ptr() as u64;
        finish(&mut w, false);
        assert_eq!(
            game[0x60 / 8],
            0,
            "original caller has not published World yet"
        );
        game[0x60 / 8] = world.as_mut_ptr() as u64;
        let mut close: Saved = unsafe { std::mem::zeroed() };
        close.invocation.kind = 62;
        close.gpr[0] = game.as_mut_ptr() as u64;
        begin(&mut close);
        assert_eq!(
            closing(world.as_ptr() as u64).unwrap().game,
            game.as_ptr() as u64
        );
        finish(&mut close, false);
        assert!(closing(world.as_ptr() as u64).is_none());
        FRAMES
            .get()
            .unwrap()
            .seal_and_wait(Instant::now() + Duration::from_secs(1))
            .unwrap();
    }
    #[test]
    fn three_native_world_entry_exchanges_restore_owned_bytes() {
        if !child("three_native_world_entry_exchanges_restore_owned_bytes") {
            return;
        }
        let Some(_fixture) = crate::windows::atomic_fixture::require(
            "three_native_world_entry_exchanges_restore_owned_bytes",
        ) else {
            return;
        };
        let size = 0x760000;
        let base =
            unsafe { VirtualAlloc(null_mut(), size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
                as usize;
        assert_ne!(base, 0);
        for (rva, bytes) in SITES {
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), (base + rva) as *mut u8, 8);
            }
        }
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(base as _, size, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        unsafe { install_at(base) }.unwrap();
        assert_eq!(HOOKS.lock().unwrap().len(), 3);
        for (i, (rva, _)) in SITES.into_iter().enumerate() {
            assert_eq!(copy_current::<1>((base + rva) as u64), Some([0xe9]));
            assert_eq!(
                NATIVE_WORLD_CONTINUE[i].load(Ordering::Acquire),
                (base + rva + 6) as u64
            );
        }
        unsafe { restore() }.unwrap();
        for (rva, bytes) in SITES {
            assert_eq!(copy_current::<8>((base + rva) as u64), Some(bytes));
        }
        assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
    }
    #[test]
    #[ignore = "explicit licensed local primary-image acceptance"]
    fn pinned_primary_context_and_world_factory_boundaries_match_install() {
        let mut file = super::super::super::pin(GAMECORE, civ6_bootstrap_rs::CORE_SHA).unwrap();
        for (rva, expected) in SITES {
            let offset = pe_file_offset(&mut file, rva as u32).unwrap();
            file.seek(SeekFrom::Start(offset)).unwrap();
            let mut raw = [0; 8];
            file.read_exact(&mut raw).unwrap();
            assert_eq!(raw, expected);
        }
        let _install: unsafe fn(usize) -> Result<(), &'static str> = install;
    }
    #[test]
    fn actual_birth_gateways_bind_original_outputs_before_caller_stores() {
        if !child("actual_birth_gateways_bind_original_outputs_before_caller_stores") {
            return;
        }
        let Some(_fixture) = crate::windows::atomic_fixture::require(
            "actual_birth_gateways_bind_original_outputs_before_caller_stores",
        ) else {
            return;
        };
        FRAMES.set(NativeFrames::new()).ok().unwrap();
        let size = 0x760000;
        let base =
            unsafe { VirtualAlloc(null_mut(), size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
                as usize;
        assert_ne!(base, 0);
        let put = |rva: usize, bytes: &[u8]| unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), (base + rva) as *mut u8, bytes.len())
        };
        let call = |rva: usize, target: usize| {
            let mut b = [0xe8; 5];
            b[1..].copy_from_slice(&((target as i64 - rva as i64 - 5) as i32).to_le_bytes());
            put(rva, &b);
        };
        let mut ctor = SITES[0].1[..6].to_vec();
        ctor.extend([0x48, 0x8d, 1, 0x48, 0xb8]);
        ctor.extend(((base + 0xa101e8) as u64).to_le_bytes());
        ctor.extend([
            0x48, 0x89, 1, 0x48, 0x89, 0x51, 8, 0x48, 0x8b, 0xc1, 0x48, 0x83, 0xc4, 0x30, 0x5b,
            0xc3,
        ]);
        put(0x58fb40, &ctor);
        let mut factory = SITES[1].1[..6].to_vec();
        factory.extend([0x48, 0x8b, 0xd9, 0x48, 0x8b, 0x41, 0x68, 0x48, 0xba]);
        factory.extend(((base + 0xa45160) as u64).to_le_bytes());
        factory.extend([
            0x48, 0x89, 0x10, 0x48, 0x89, 0x98, 0x70, 2, 0, 0, 0x48, 0x83, 0xc4, 0x30, 0x5b, 0xc3,
        ]);
        put(0x7504c0, &factory);
        let mut close = SITES[2].1[..6].to_vec();
        close.extend([
            0x80, 0x79, 0x40, 0, 0x48, 0xc7, 0x41, 0x60, 0, 0, 0, 0, 0xb8, 0x44, 0x33, 0, 0, 0x48,
            0x83, 0xc4, 0x20, 0x5b, 0xc3,
        ]);
        put(0x470b0, &close);
        for (start, at, target, tail) in [
            (
                0x58fcb0,
                0x58fcbe,
                0x58fb40,
                &[0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3][..],
            ),
            (
                0x46d20,
                0x46d2b,
                0x7504c0,
                &[0x48, 0x89, 0x43, 0x60, 0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3][..],
            ),
        ] {
            put(start, &[0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b, 0xd9]);
            unsafe {
                ptr::write_bytes((base + start + 8) as *mut u8, 0x90, at - start - 8);
            }
            call(at, target);
            put(at + 5, tail);
        }
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(base as _, size, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        unsafe { install_at(base) }.unwrap();
        let mut game = vec![0u64; 0x80 / 8];
        game[0] = (base + 0xa44f70) as u64;
        game[0x40 / 8] = 1;
        let mut context = [0u64; 2];
        let mut world = vec![0u64; 0x278 / 8];
        game[0x68 / 8] = world.as_mut_ptr() as u64;
        let context_ctor: unsafe extern "system" fn(*mut u64, *mut u64) -> u64 =
            unsafe { std::mem::transmute(base + 0x58fcb0) };
        assert_eq!(
            unsafe { context_ctor(context.as_mut_ptr(), game.as_mut_ptr()) },
            context.as_ptr() as u64
        );
        let factory: unsafe extern "system" fn(*mut u64) -> u64 =
            unsafe { std::mem::transmute(base + 0x46d20) };
        assert_eq!(unsafe { factory(game.as_mut_ptr()) }, world.as_ptr() as u64);
        assert_eq!(game[0x60 / 8], world.as_ptr() as u64);
        let close: unsafe extern "system" fn(*mut u64) -> u64 =
            unsafe { std::mem::transmute(base + 0x470b0) };
        assert_eq!(unsafe { close(game.as_mut_ptr()) }, 0x3344);
        assert_eq!(game[0x60 / 8], 0);
        assert!(closing(world.as_ptr() as u64).is_none());
        FRAMES
            .get()
            .unwrap()
            .seal_and_wait(Instant::now() + Duration::from_secs(1))
            .unwrap();
        unsafe { restore() }.unwrap();
        assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
    }
}
