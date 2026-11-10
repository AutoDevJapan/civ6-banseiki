//! Natural-close admission and Host list release ledger for one Core manager
//! incarnation. A closed manager remains a tombstone: only a real 49C7C0
//! normal return starts a new incarnation. Raw address or list-id reuse never
//! does. This is not an all-producer seal and never makes native coverage true.
use super::*;
use std::sync::MutexGuard;
#[path = "native_closure_fixed.rs"]
mod fixed;

const INIT: usize = 0x49c7c0;
const ENTRY: [u8; 8] = [0x48, 0x8b, 0xc4, 0x55, 0x53, 0x56, 0x57, 0x41];
const SET_OFFSET: u64 = 0x400;
const LIST_OFFSET: u64 = 0x410;
/// Return address of the manager's own FreeJobList call at 4A1209.
const RELEASE_RETURN: u64 = 0x4a120e;

static CORE: AtomicU64 = AtomicU64::new(0);
static ACTIVE: AtomicU64 = AtomicU64::new(0);
static LEDGER: Mutex<Ledger> = Mutex::new(Ledger::new());
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
pub(super) fn hook_entries_owned() -> bool {
    owns_hook_entries(&HOOKS, [ENTRY].len())
}

#[unsafe(no_mangle)]
static NATIVE_CLOSURE_CONTINUE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum Phase {
    Open,
    Closing,
    Waited,
    Returned,
    Closed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum Violation {
    DuplicateClose = 1,
    LateSubmit,
    WaitOutOfOrder,
    ReleaseOutOfOrder,
    ReleaseCaller,
    DoubleRelease,
    ListNotReleased,
    CloseOutOfOrder,
    WorldNotReturned,
    InitDuringClose,
    Unreadable,
    Unwound,
    Exhausted,
    Poisoned,
    ReleaseList,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Manager {
    manager: u64,
    serial: u64,
    world: u64,
    set: u64,
    list: u32,
    phase: Phase,
    init_observed: bool,
    repeated: bool,
    closing_submits: u32,
    released: bool,
    late_spawn: bool,
}
/// Evidence for one natural World close. It records only observed entry
/// events and is not a native shutdown certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct Receipt {
    pub world: u64,
    pub manager: u64,
    pub serial: u64,
    pub set: u64,
    pub list: u32,
    pub init_observed: bool,
    pub repeated: bool,
    pub list_released: bool,
    pub late_spawn: bool,
}
impl Receipt {
    /// Complete only for an incarnation whose 49C7C0 return was observed, that
    /// was closed once, and whose own list was released. Unowned 4A1150 calls
    /// and repeated closes can never complete it. It is not a native seal.
    pub(in super::super) fn complete(&self) -> bool {
        self.world != 0
            && self.manager != 0
            && self.serial != 0
            && self.set == self.manager.wrapping_add(SET_OFFSET)
            && self.init_observed
            && !self.repeated
            && !self.late_spawn
            && (self.list == 0 || self.list_released)
    }
}
struct Ledger {
    managers: Vec<Manager>,
    next: u64,
    unresolved: bool,
    // 4A1150 outside the tracked World close is recorded only. It is never a
    // World closing certificate.
    unowned: u32,
    #[cfg(test)]
    last: Option<Receipt>,
}
fn priority(m: &Manager) -> u8 {
    match m.phase {
        Phase::Waited => 3,
        Phase::Closing => 2,
        _ => 1,
    }
}
impl Ledger {
    const fn new() -> Self {
        Self {
            managers: Vec::new(),
            next: 0,
            unresolved: false,
            unowned: 0,
            #[cfg(test)]
            last: None,
        }
    }
    fn healthy(&self) -> Result<(), Violation> {
        if self.unresolved {
            Err(Violation::Unwound)
        } else {
            Ok(())
        }
    }
    fn bump(&mut self) -> Result<u64, Violation> {
        self.next = self.next.checked_add(1).ok_or(Violation::Exhausted)?;
        Ok(self.next)
    }
    fn index(&self, manager: u64) -> Option<usize> {
        self.managers.iter().position(|m| m.manager == manager)
    }
    /// A normal 49C7C0 return is the only event that starts an incarnation.
    fn init_returned(&mut self, manager: u64, list: u32) -> Result<u64, Violation> {
        self.healthy()?;
        let set = manager
            .checked_add(SET_OFFSET)
            .filter(|_| manager != 0)
            .ok_or(Violation::Unreadable)?;
        let existing = self.index(manager);
        if existing.is_some_and(|i| {
            matches!(
                self.managers[i].phase,
                Phase::Closing | Phase::Waited | Phase::Returned
            )
        }) {
            return Err(Violation::InitDuringClose);
        }
        let serial = self.bump()?;
        let fresh = Manager {
            manager,
            serial,
            world: 0,
            set,
            list,
            phase: Phase::Open,
            init_observed: true,
            repeated: false,
            closing_submits: 0,
            released: false,
            late_spawn: false,
        };
        match existing {
            Some(i) => self.managers[i] = fresh,
            None => self.managers.push(fresh),
        }
        Ok(serial)
    }
    fn close_begin(&mut self, world: u64, manager: u64, list: u32) -> Result<(), Violation> {
        self.healthy()?;
        let set = manager
            .checked_add(SET_OFFSET)
            .filter(|_| manager != 0 && world != 0)
            .ok_or(Violation::Unreadable)?;
        let Some(i) = self.index(manager) else {
            let serial = self.bump()?;
            self.managers.push(Manager {
                manager,
                serial,
                world,
                set,
                list,
                phase: Phase::Closing,
                init_observed: false,
                repeated: false,
                closing_submits: 0,
                released: false,
                late_spawn: false,
            });
            return Ok(());
        };
        let m = &mut self.managers[i];
        match m.phase {
            Phase::Open => {}
            // A second close of a tombstone has no new incarnation, so every
            // submit during it remains late.
            Phase::Closed => m.repeated = true,
            _ => return Err(Violation::DuplicateClose),
        }
        m.phase = Phase::Closing;
        m.world = world;
        m.list = list;
        m.released = false;
        m.late_spawn = false;
        m.closing_submits = 0;
        Ok(())
    }
    fn submit(&mut self, set: u64) -> Result<(), Violation> {
        let Some(m) = self.managers.iter_mut().find(|m| m.set == set) else {
            return Ok(());
        };
        match m.phase {
            Phase::Open => Ok(()),
            Phase::Closing if !m.repeated => {
                m.closing_submits = m.closing_submits.saturating_add(1);
                Ok(())
            }
            _ => Err(Violation::LateSubmit),
        }
    }
    fn spawn(&mut self, list: u32) -> Result<(), Violation> {
        if list == 0 {
            return Ok(());
        }
        // Only a list that the manager still owns can be attributed. A freed
        // id may already belong to another subsystem, so a released lease no
        // longer matches.
        let Some(m) = self
            .managers
            .iter_mut()
            .filter(|m| {
                m.list == list
                    && !m.released
                    && matches!(m.phase, Phase::Open | Phase::Closing | Phase::Waited)
            })
            .max_by_key(|m| priority(m))
        else {
            return Ok(());
        };
        match m.phase {
            Phase::Open => Ok(()),
            Phase::Closing if !m.repeated => {
                m.closing_submits = m.closing_submits.saturating_add(1);
                Ok(())
            }
            // Spawn starts no job by itself and 986010 has many Host callers.
            // A late one makes the receipt incomplete; late Add/Push stay fatal.
            _ => {
                m.late_spawn = true;
                Ok(())
            }
        }
    }
    fn wait_returned(&mut self, manager: u64) -> Result<(), Violation> {
        match self.managers.iter_mut().find(|m| m.manager == manager) {
            Some(m) if m.phase == Phase::Closing => {
                m.phase = Phase::Waited;
                Ok(())
            }
            _ => Err(Violation::WaitOutOfOrder),
        }
    }
    /// `owner_caller` is true only for the return address after 4A1209 and
    /// `receiver` is the manager that 4A1150 keeps in RBX from 4A1175 to its
    /// return (the 9921D0 thunk restores RBX before its tail jump). The owner
    /// identity is that pair. A bare list id is attributed only while the
    /// manager's lease is live, never after it was released and may have been
    /// recycled by another domain.
    fn list_released(
        &mut self,
        list: u32,
        owner_caller: bool,
        receiver: u64,
    ) -> Result<bool, Violation> {
        if list == 0 {
            return Ok(false);
        }
        if owner_caller {
            let Some(m) = self.managers.iter_mut().find(|m| m.manager == receiver) else {
                // Unknown native receiver: no attribution and no credit.
                return Ok(false);
            };
            return match m.phase {
                // 4A1150 outside the tracked World close. Observed, not certified.
                Phase::Open => {
                    if m.list == list {
                        m.list = 0;
                        self.unowned = self.unowned.saturating_add(1);
                    }
                    Ok(false)
                }
                Phase::Closing => Err(Violation::ReleaseOutOfOrder),
                Phase::Waited if m.released => Err(Violation::DoubleRelease),
                Phase::Waited if m.list != list => Err(Violation::ReleaseList),
                Phase::Waited => {
                    m.released = true;
                    Ok(true)
                }
                // No tracked close owns a shutdown after the close ended.
                Phase::Returned | Phase::Closed => Ok(false),
            };
        }
        let Some(m) = self
            .managers
            .iter_mut()
            .filter(|m| {
                m.list == list
                    && !m.released
                    && matches!(m.phase, Phase::Open | Phase::Closing | Phase::Waited)
            })
            .max_by_key(|m| priority(m))
        else {
            return Ok(false);
        };
        match m.phase {
            // The id may be stale after an unobserved release.
            Phase::Open => Ok(false),
            Phase::Closing => Err(Violation::ReleaseOutOfOrder),
            _ => Err(Violation::ReleaseCaller),
        }
    }
    fn close_end(&mut self, manager: u64) -> Result<(), Violation> {
        match self.managers.iter_mut().find(|m| m.manager == manager) {
            Some(m) if m.phase == Phase::Waited => {
                if m.list != 0 && !m.released {
                    return Err(Violation::ListNotReleased);
                }
                m.phase = Phase::Returned;
                Ok(())
            }
            _ => Err(Violation::CloseOutOfOrder),
        }
    }
    fn world_closed(&mut self, world: u64) -> Result<Receipt, Violation> {
        let m = self
            .managers
            .iter_mut()
            .find(|m| world != 0 && m.world == world && m.phase == Phase::Returned)
            .ok_or(Violation::WorldNotReturned)?;
        m.phase = Phase::Closed;
        let receipt = Receipt {
            world,
            manager: m.manager,
            serial: m.serial,
            set: m.set,
            list: m.list,
            init_observed: m.init_observed,
            repeated: m.repeated,
            list_released: m.released,
            late_spawn: m.late_spawn,
        };
        #[cfg(test)]
        {
            self.last = Some(receipt);
        }
        Ok(receipt)
    }
}

fn ledger() -> Result<MutexGuard<'static, Ledger>, Violation> {
    LEDGER.lock().map_err(|_| Violation::Poisoned)
}
fn sync(l: &Ledger) {
    ACTIVE.store(l.managers.len() as u64, Ordering::Release);
}
pub(super) fn bind_core(core: u64) {
    CORE.store(core, Ordering::Release);
}
fn release_caller(caller: u64) -> bool {
    let core = CORE.load(Ordering::Acquire);
    core != 0 && caller == core.wrapping_add(RELEASE_RETURN)
}
/// Core 49C7C0 normal return: a real manager initialization and list birth.
pub(in super::super) fn init_returned(manager: u64, list: u32) -> Result<u64, Violation> {
    let mut l = ledger()?;
    let result = l.init_returned(manager, list);
    sync(&l);
    result
}
/// Core 4A1150 entry inside the tracked 63E160 close.
pub(in super::super) fn close_begin(world: u64, manager: u64, list: u32) -> Result<(), Violation> {
    let mut l = ledger()?;
    let result = l.close_begin(world, manager, list);
    sync(&l);
    result
}
/// Host Add/Push set. Zero active managers is the cheap normal path.
pub(in super::super) fn submit(set: u64) -> Result<(), Violation> {
    if ACTIVE.load(Ordering::Acquire) == 0 {
        return Ok(());
    }
    ledger()?.submit(set)
}
/// Host Spawn/reissue list id (low 32 bits).
pub(in super::super) fn spawn(list: u32) -> Result<(), Violation> {
    if ACTIVE.load(Ordering::Acquire) == 0 {
        return Ok(());
    }
    ledger()?.spawn(list)
}
/// Core 9922C0 normal return for this manager's set.
pub(in super::super) fn wait_returned(manager: u64) -> Result<(), Violation> {
    ledger()?.wait_returned(manager)
}
/// Host FreeJobList normal return, its native return address and the RBX it
/// was entered with. At 4A1209 that RBX is the manager being shut down.
pub(in super::super) fn list_released(
    list: u32,
    caller: u64,
    receiver: u64,
) -> Result<bool, Violation> {
    if ACTIVE.load(Ordering::Acquire) == 0 {
        return Ok(false);
    }
    ledger()?.list_released(list, release_caller(caller), receiver)
}
pub(in super::super) fn close_end(manager: u64) -> Result<(), Violation> {
    ledger()?.close_end(manager)
}
pub(in super::super) fn world_closed(world: u64) -> Result<Receipt, Violation> {
    ledger()?.world_closed(world)
}
/// Record only. A 4A1150 outside the World close is not a closing receipt.
pub(in super::super) fn unowned_shutdown() {
    if let Ok(mut l) = LEDGER.lock() {
        l.unowned = l.unowned.saturating_add(1);
    }
}
#[cfg(test)]
pub(in super::super) fn unowned_count() -> u32 {
    LEDGER.lock().map_or(u32::MAX, |l| l.unowned)
}
/// A terminal exception through a ledger-owned native frame.
pub(in super::super) fn unwound() {
    if let Ok(mut l) = LEDGER.lock() {
        l.unresolved = true;
    }
    unresolved();
}
#[track_caller]
pub(in super::super) fn fault(violation: Violation) -> ! {
    census::fault(
        census::module(module_path!()),
        std::panic::Location::caller().line(),
    );
    diagnostic(150 + violation as usize);
    #[cfg(test)]
    {
        println!("CLOSURE_VIOLATION={violation:?}");
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
    unresolved();
    std::process::abort()
}

extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    // Entry RCX is the only receiver; output registers overwrite Saved later.
    let raw = unsafe { &mut *raw };
    raw.invocation.manager = raw.gpr[0];
    raw.invocation.cookie = COOKIE;
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
        unwound();
        return;
    }
    let manager = raw.invocation.manager;
    let list = manager
        .checked_add(LIST_OFFSET)
        .and_then(copy_current::<4>)
        .map(u32::from_le_bytes)
        .unwrap_or_else(|| fault(Violation::Unreadable));
    init_returned(manager, list).unwrap_or_else(|v| fault(v));
}
extern "system" fn end(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        finish(unsafe { &mut *raw }, false)
    }))
    .is_err()
    {
        fault(Violation::Poisoned);
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
    if !record.is_null() && unsafe { (*record).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0 {
        finish(unsafe { &mut *((frame + 0x20) as *mut Saved) }, true);
    }
    unsafe { SetLastError(error) };
    1
}
// The displaced 49C7C0 prologue is mov rax,rsp; push rbp; push rbx. RAX keeps
// this shim's entry RSP exactly as the original body expects. Scratch slots are
// the caller-owned shadow space above the return address.
global_asm!(
    ".globl native_closure_init_original",
    ".seh_proc native_closure_init_original",
    "native_closure_init_original:",
    "mov rax,rsp",
    "push rbp",
    ".seh_pushreg rbp",
    "push rbx",
    ".seh_pushreg rbx",
    ".seh_endprologue",
    "mov [rsp+0x18],rax",
    "mov rax,[rip+NATIVE_CLOSURE_CONTINUE]",
    "mov [rsp+0x20],rax",
    "mov rax,[rsp+0x18]",
    "jmp qword ptr [rsp+0x20]",
    ".seh_endproc",
);
unsafe extern "system" {
    fn native_closure_init_original();
    fn native_closure_init_gateway();
}
gateway!(
    "native_closure_init_gateway",
    "138",
    native_closure_init_original,
    begin,
    end,
    unwind
);

/// Verify the whole pinned body before publishing the only closure entry.
pub(super) unsafe fn install(core: usize) -> Result<(), &'static str> {
    bind_core(core as u64);
    unsafe {
        super::verify_frames(core, fixed::FRAMES)?;
        install_at(core)
    }
}
unsafe fn install_at(core: usize) -> Result<(), &'static str> {
    let mut owners = HOOKS.lock().map_err(|_| "closure hook owner poisoned")?;
    if !owners.is_empty() {
        return Err("closure init hook already installed");
    }
    let entry = (core + INIT) as *mut u8;
    if copy_current::<8>(entry as u64) != Some(ENTRY) {
        return Err("closure init entry differs");
    }
    let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    if !(entry as usize).is_multiple_of(8)
        || unsafe { VirtualQuery(entry.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
        || info.State != MEM_COMMIT
        || info.Protect & 0xff != PAGE_EXECUTE_READ
        || info.Protect & PAGE_GUARD != 0
    {
        return Err("closure init entry protection");
    }
    let relay = unsafe { executable_near(entry as usize, entry as usize, 5) }
        .ok_or("closure relay unavailable")?;
    let mut unpublished = Unpublished(relay);
    unsafe {
        ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
        ptr::copy_nonoverlapping(
            (native_closure_init_gateway as *const () as u64)
                .to_le_bytes()
                .as_ptr(),
            relay.add(6),
            8,
        );
    }
    let mut old = 0;
    if unsafe { VirtualProtect(relay.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
        || unsafe { FlushInstructionCache(GetCurrentProcess(), relay.cast(), 14) } == 0
    {
        return Err("closure relay publication");
    }
    let (patched, prefix) = native_entry::plan(entry, ENTRY, relay)?;
    NATIVE_CLOSURE_CONTINUE.store((core + INIT + 5) as u64, Ordering::Release);
    owners.push(OwnedHook {
        hook: Hook {
            target: entry,
            original: ENTRY,
            patched,
            prefix,
            _code: relay,
        },
        protection: info.Protect,
    });
    unpublished.0 = null_mut();
    let outcome =
        unsafe { native_entry::publish(&owners.last().ok_or("native hook owner missing")?.hook) }?;
    if !outcome.exchanged || !outcome.protection_restored || !outcome.cache_flushed {
        return Err("critical: closure init publication unverified");
    }
    Ok(())
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut owners = HOOKS.lock().map_err(|_| "closure hook owner poisoned")?;
    owners.retain(|owned| {
        let hook = &owned.hook;
        let now = copy_current::<8>(hook.target as u64);
        let restored = now == Some(hook.original)
            || (now == Some(hook.patched)
                && unsafe {
                    exchange_entry(
                        hook.target,
                        u64::from_le_bytes(hook.patched),
                        u64::from_le_bytes(hook.original),
                    )
                }
                .is_ok_and(|r| r.exchanged && r.protection_restored && r.cache_flushed));
        let mut old = 0;
        !(restored
            && unsafe { VirtualProtect(hook.target.cast(), 8, owned.protection, &mut old) } != 0)
    });
    if owners.is_empty() {
        Ok(())
    } else {
        Err("critical: closure init restoration unverified")
    }
}

#[cfg(test)]
#[path = "native_closure_tests.rs"]
mod tests;
