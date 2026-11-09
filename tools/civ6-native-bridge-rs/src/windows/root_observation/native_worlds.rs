//! Native Context -> CGame -> World births before their original publication.
//! These identities do not identify a network session or certify its seal.
use super::*;
use std::collections::HashMap;

static CORE: AtomicU64 = AtomicU64::new(0);
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
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
    fn birth_window(&self) -> Option<BirthWindow> {
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

extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        let core = CORE.load(Ordering::Acquire);
        match raw.invocation.kind {
            60 => {
                if raw.invocation.caller != core + 0x58fcc3 {
                    return;
                }
                raw.invocation.world = raw.gpr[0];
            }
            61 => {
                if raw.invocation.caller != core + 0x46d30 {
                    fault();
                }
                raw.invocation.manager = raw.gpr[0];
                if value(raw.gpr[0]) != core + 0xa44f70 || value(raw.gpr[0] + 0x60) != 0 {
                    fault();
                }
            }
            62 => {
                if value(raw.gpr[0]) != core + 0xa44f70 {
                    fault();
                }
                if copy_current::<1>(raw.gpr[0] + 0x40) != Some([1]) {
                    return;
                }
                let mut b = BINDINGS.lock().unwrap_or_else(|_| fault());
                let Some(birth) = b
                    .contexts
                    .values()
                    .find(|b| b.game == raw.gpr[0] && !b.closed)
                else {
                    fault();
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
                    .find_map(|r| r.world.filter(|w| w.world == world))
                    .unwrap_or_else(|| fault());
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
        if raw.invocation.kind == 60 {
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
unsafe fn install_at(core: usize) -> Result<(), &'static str> {
    CORE.store(core as u64, Ordering::Release);
    let callbacks = [
        native_world_context_gateway as *const () as usize,
        native_world_factory_gateway as *const () as usize,
        native_world_close_gateway as *const () as usize,
    ];
    let mut hooks = HOOKS.lock().map_err(|_| "World hook owner")?;
    for (i, (rva, expected)) in SITES.into_iter().enumerate() {
        let p = (core + rva) as *mut u8;
        if copy_current::<8>(p as u64) != Some(expected) {
            return Err("World birth entry differs");
        }
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(p.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
            || info.Protect & 0xff != PAGE_EXECUTE_READ
        {
            return Err("World entry protection");
        }
        let code = unsafe { executable_near(p as usize, p as usize, 6) }.ok_or("World relay")?;
        let mut unpublished = Unpublished(code);
        unsafe {
            ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), code, 6);
            ptr::copy_nonoverlapping((callbacks[i] as u64).to_le_bytes().as_ptr(), code.add(6), 8);
        }
        let mut old = 0;
        if unsafe { VirtualProtect(code.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
            || unsafe { FlushInstructionCache(GetCurrentProcess(), code.cast(), 14) } == 0
        {
            return Err("World relay publication");
        }
        let mut patch = expected;
        patch[..5]
            .copy_from_slice(&rel32_jump(p as usize, code as usize).ok_or("World relay range")?);
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
            protection: info.Protect,
        });
        unpublished.0 = null_mut();
        let r =
            unsafe { exchange_entry(p, u64::from_le_bytes(expected), u64::from_le_bytes(patch)) }?;
        if !r.exchanged || !r.cache_flushed || !r.protection_restored {
            return Err("critical: World hook publication");
        }
    }
    Ok(())
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
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
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
                assert!(s.success());
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
        if !supports_atomic_promotion_fetch() {
            return;
        }
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
        if !supports_atomic_promotion_fetch() {
            return;
        }
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
