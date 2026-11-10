//! Accepted native load requests through the command bank and LoadGameView.
//! Provisional publication is retained until the original outcome is known.
//! Unknown pre-install requests and unbound worker births remain unknown.
use super::*;
use std::{cell::RefCell, collections::HashMap, sync::MutexGuard};
#[path = "native_loads_fixed.rs"]
mod fixed;
#[path = "native_loads_ledger.rs"]
mod ledger;
use ledger::{Invocation as LoadInvocation, Ledger, Request};

static EXE: AtomicU64 = AtomicU64::new(0);
static NEXT: AtomicU64 = AtomicU64::new(0);
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
pub(super) fn hook_entries_owned() -> bool {
    owns_hook_entries(&HOOKS, SITES.len())
}

static ALLOCATION: Mutex<()> = Mutex::new(());
static STATE: std::sync::LazyLock<Mutex<State>> =
    std::sync::LazyLock::new(|| Mutex::new(State::default()));
#[unsafe(no_mangle)]
static NATIVE_LOAD_CONTINUE: [AtomicU64; 13] = [const { AtomicU64::new(0) }; 13];
#[derive(Clone, Copy)]
struct Source {
    session: Option<(
        sessions::SessionLease,
        Option<sessions::SessionWorldReceipt>,
    )>,
    wire: Option<wire::Snapshot>,
    bootstrap: Option<sessions::BootstrapTarget>,
}
#[derive(Default)]
struct State {
    ledger: Ledger,
    sources: HashMap<u64, Source>,
    // A worker must carry this invocation from its native publication. A raw
    // view address at an eventual callback is deliberately insufficient.
    tasks: HashMap<u64, Task>,
    tasks_by_address: HashMap<u64, u64>,
    accepted: std::collections::HashSet<u64>,
    worlds: HashMap<u64, worlds::WorldLease>,
    groups: HashMap<u64, u64>,
    binding_rejections: HashMap<u64, sessions::SessionError>,
}
struct Task {
    invocation: LoadInvocation,
    view: u64,
    pointer: u64,
    group: u64,
    active: bool,
    completed: bool,
}
struct JobCall {
    rsp: u64,
    kind: u64,
    id: Option<u64>,
    invocation: Option<LoadInvocation>,
    view: u64,
    group: u64,
    hold: Option<(u64, u64)>,
}
struct Call {
    serial: u64,
    kind: u64,
    source: Option<Source>,
    request: Option<Request>,
    invocation: Option<LoadInvocation>,
    pointer: u64,
    descriptor: u64,
    engine: u64,
    allocation: Option<MutexGuard<'static, ()>>,
    joined: bool,
}
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static JOB_CALLS: RefCell<Vec<JobCall>> = const { RefCell::new(Vec::new()) };
}
fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|_| fault())
}
#[track_caller]
fn fault() -> ! {
    census::fault(
        census::module(module_path!()),
        std::panic::Location::caller().line(),
    );
    diagnostic(80);
    if let Ok(mut s) = STATE.try_lock() {
        s.ledger.unresolved();
    }
    FAILED.store(true, Ordering::Release);
    TRACE.0[6].store(1, Ordering::Release);
    if let Some(f) = FRAMES.get() {
        f.mark_unresolved();
    }
    std::process::abort()
}
fn serial() -> u64 {
    NEXT.fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
        .unwrap_or_else(|_| fault())
        + 1
}
fn value(address: u64) -> u64 {
    copy_current::<8>(address)
        .map(u64::from_le_bytes)
        .unwrap_or_else(|| fault())
}
fn dword(address: u64) -> u32 {
    copy_current::<4>(address)
        .map(u32::from_le_bytes)
        .unwrap_or_else(|| fault())
}
fn path_input(pointer: u64, descriptor: u64) -> Option<ledger::PathInput> {
    if pointer == 0 {
        return None;
    }
    let options = copy_current::<28>(descriptor)?;
    if u32::from_le_bytes(options[8..12].try_into().unwrap()) == 4 {
        return None;
    }
    // 16B5C0 copies at most 259 UTF-16 units and then writes its terminator.
    // Capture the same values while the original caller still owns the input.
    let mut name = Vec::new();
    for index in 0..259u64 {
        let unit = u16::from_le_bytes(copy_current::<2>(pointer.checked_add(index * 2)?)?);
        if unit == 0 {
            break;
        }
        name.push(unit);
    }
    Some(ledger::PathInput { options, name })
}
fn configured_path() -> Option<ledger::PathInput> {
    let input = path_input(exe(0x1267a9c), exe(0x1267a68))?;
    if input.name.len() != dword(exe(0x1267a98)) as usize
        || copy_current::<2>(exe(0x1267a9c) + input.name.len() as u64 * 2) != Some([0, 0])
    {
        return None;
    }
    Some(input)
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
fn parent_request() -> Option<Request> {
    CALLS.with(|s| {
        s.borrow()
            .iter()
            .rev()
            .find(|c| c.kind == 81)
            .and_then(|c| c.request)
    })
}
fn current_worker() -> Option<LoadInvocation> {
    CALLS.with(|s| {
        s.borrow()
            .iter()
            .rev()
            .find(|c| c.kind == 87)
            .and_then(|c| c.invocation)
    })
}
/// Cooperates with the already-owned Host allocator/execute/destructor
/// gateways, so no native entry acquires two competing branch owners.
pub(super) fn job_begin(raw: &mut Saved) {
    if EXE.load(Ordering::Acquire) == 0 {
        return;
    }
    let mut c = JobCall {
        rsp: raw.invocation.caller_rsp,
        kind: raw.invocation.kind,
        id: None,
        invocation: None,
        view: 0,
        group: 0,
        hold: None,
    };
    match c.kind {
        23 if caller(raw, 0x985904) => {
            let parent = CALLS.with(|s| {
                s.borrow()
                    .iter()
                    .rev()
                    .find(|c| c.kind == 89)
                    .and_then(|c| c.invocation.map(|id| (id, c.pointer)))
            });
            if let Some((invocation, view)) = parent {
                state()
                    .ledger
                    .reserve_worker(invocation)
                    .unwrap_or_else(|_| fault());
                c.invocation = Some(invocation);
                c.view = view;
                c.group = value(view + 0x280);
                if c.group == 0 {
                    fault();
                }
                let mut s = state();
                if s.groups.insert(invocation.0, c.group).is_some() {
                    fault();
                }
            }
        }
        21 => {
            let mut s = state();
            if let Some(id) = s.tasks_by_address.get(&raw.gpr[0]).copied() {
                let task = s.tasks.get_mut(&id).unwrap_or_else(|| fault());
                if task.active
                    || task.completed
                    || value(task.pointer + 8) != task.view + 0x248
                    || value(task.view + 0x280) != task.group
                    || value(task.view + 0x248) != exe(0x106c390)
                    || value(task.view + 0x270) != task.view + 0x240
                    || value(task.view + 0x240) != exe(0xfad3e0)
                    || value(task.view + 0x290) != task.view
                    || value(task.view + 0x298) != exe(0x82da0)
                {
                    fault();
                }
                task.active = true;
                c.id = Some(id);
                c.invocation = Some(task.invocation);
                c.view = task.view;
            }
        }
        22 => {
            let mut s = state();
            if let Some(id) = s.tasks_by_address.remove(&raw.gpr[0]) {
                let task = s.tasks.get(&id).unwrap_or_else(|| fault());
                if task.active {
                    fault();
                }
                c.id = Some(id);
                c.invocation = Some(task.invocation);
                c.view = task.view;
            }
        }
        20 | 23 => {}
        _ => fault(),
    }
    JOB_CALLS.with(|s| s.borrow_mut().push(c));
    raw.invocation.errors |= 1 << 63;
}
/// Observe the existing complete arena gateway inside the original load-group
/// Wait. This does not install a second arena hook or call a scheduler API.
pub(super) fn group_arena_enter(raw: &mut Saved, external: bool) {
    if !caller(raw, 0x985bb0) {
        return;
    }
    CALLS.with(|s| {
        let mut s = s.borrow_mut();
        let Some(c) = s
            .iter_mut()
            .rev()
            .find(|c| c.kind == 91 && c.invocation.is_some())
        else {
            return;
        };
        if c.engine != raw.gpr[0] || c.pointer.checked_sub(0x30) != Some(raw.invocation.caller_rsp)
        {
            fault();
        }
        raw.invocation.list = c.serial;
        raw.invocation.set = u64::from(external);
    });
}
pub(super) fn group_arena_return(raw: &Saved, exceptional: bool) {
    if raw.invocation.list == 0 {
        return;
    }
    CALLS.with(|s| {
        let mut s = s.borrow_mut();
        let c = s
            .iter_mut()
            .rev()
            .find(|c| c.serial == raw.invocation.list)
            .unwrap_or_else(|| fault());
        if exceptional || raw.invocation.set != 1 {
            #[cfg(test)]
            if std::env::var_os("CIV6_LOAD_GROUP_CURRENT_NEGATIVE").is_some() {
                println!("LOAD_GROUP_CURRENT_ARENA_REACHED");
                use std::io::Write;
                std::io::stdout().flush().unwrap();
            }
            state().ledger.unresolved();
            FAILED.store(true, Ordering::Release);
            return;
        }
        c.joined = true;
    });
}
pub(super) fn job_finish(raw: &mut Saved, exceptional: bool) {
    if raw.invocation.errors & (1 << 63) == 0 {
        return;
    }
    raw.invocation.errors &= !(1 << 63);
    let c = JOB_CALLS
        .with(|s| s.borrow_mut().pop())
        .unwrap_or_else(|| fault());
    if c.rsp != raw.invocation.caller_rsp || c.kind != raw.invocation.kind {
        fault();
    }
    if exceptional {
        if c.invocation.is_some() {
            state().ledger.unresolved();
            FAILED.store(true, Ordering::Release);
        }
        if let Some((hold, _)) = c.hold {
            if let Some(f) = FRAMES.get() {
                f.mark_unresolved();
                f.leave(hold).unwrap_or_else(|_| fault());
            }
        }
        return;
    }
    let Some(invocation) = c.invocation else {
        return;
    };
    let mut s = state();
    match c.kind {
        23 => {
            let task = raw.gpr[4];
            // Host985904 explicitly branches around T publication on NULL.
            // Cancel this reservation and let that native fallback run.
            if task == 0 {
                s.ledger
                    .allocation_failed(invocation)
                    .unwrap_or_else(|_| fault());
                if s.groups.remove(&invocation.0) != Some(c.group) {
                    fault();
                }
                return;
            }
            if s.tasks_by_address.contains_key(&task) {
                fault();
            }
            let id = serial();
            s.tasks.insert(
                id,
                Task {
                    invocation,
                    view: c.view,
                    pointer: task,
                    group: c.group,
                    active: false,
                    completed: false,
                },
            );
            s.tasks_by_address.insert(task, id);
        }
        21 => {
            let t = s.tasks.get_mut(&c.id.unwrap()).unwrap();
            if !t.active {
                fault();
            }
            t.active = false;
            t.completed = true;
        }
        22 => {
            let task = s.tasks.remove(&c.id.unwrap()).unwrap();
            if task.active {
                fault();
            }
            s.ledger
                .release_worker(invocation)
                .unwrap_or_else(|_| fault());
        }
        _ => fault(),
    }
    if let Some((hold, _)) = c.hold {
        FRAMES
            .get()
            .unwrap_or_else(|| fault())
            .leave(hold)
            .unwrap_or_else(|_| fault());
    }
}
/// Called only at the Core factory's before-publication normal return.
pub(super) fn world_created(world: worlds::WorldLease) {
    let Some(invocation) = current_worker() else {
        return;
    };
    // This is a native before-publication birth, on an already retained
    // load-task invocation. Transfer its exact epoch into the runtime that
    // may have been installed while this original Host task was running.
    JOB_CALLS.with(|calls| {
        let mut calls = calls.borrow_mut();
        let job = calls
            .iter_mut()
            .rev()
            .find(|c| c.kind == 21 && c.invocation == Some(invocation))
            .unwrap_or_else(|| fault());
        if job.hold.is_some() {
            fault();
        }
        let hold = FRAMES
            .get()
            .unwrap_or_else(|| fault())
            .enter_existing_epoch(world.epoch)
            .unwrap_or_else(|_| fault());
        job.hold = Some((hold, world.epoch));
    });
    let mut s = state();
    let request = s
        .ledger
        .world(invocation, world.serial, world.epoch)
        .unwrap_or_else(|_| fault());
    let source = s
        .sources
        .get(&request.0)
        .copied()
        .unwrap_or_else(|| fault());
    s.worlds.insert(request.0, world);
    let accepted = s.accepted.contains(&request.0);
    drop(s);
    if source
        .wire
        .is_some_and(|snapshot| !snapshot.matches_session(source.session))
    {
        fault();
    }
    if let Some(target) = source.bootstrap.filter(|_| accepted) {
        if let Err(reason) = sessions::bind_bootstrap_world(target, world) {
            state().binding_rejections.insert(request.0, reason);
        }
    } else if let Some((lease, previous)) = source.session.filter(|_| accepted) {
        let result = sessions::loaded_world(
            lease,
            previous,
            sessions::SessionWorldReceipt {
                binding_id: world.serial,
                epoch: world.epoch,
            },
        );
        match result {
            Ok(()) => {}
            Err(sessions::SessionError::Unresolved) => {
                state()
                    .binding_rejections
                    .insert(request.0, sessions::SessionError::Unresolved);
            }
            Err(_) => fault(),
        }
    }
}
extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        let mut c = Call {
            serial: serial(),
            kind: raw.invocation.kind,
            source: None,
            request: None,
            invocation: None,
            pointer: 0,
            descriptor: 0,
            engine: 0,
            allocation: None,
            joined: false,
        };
        match c.kind {
            80 => {
                let owner = raw.gpr[0];
                if value(owner) == exe(0xfd35c8) && value(exe(0xfd36b0)) == exe(0x273f40) {
                    let session = value(owner + 8);
                    let wire = wire::request(raw);
                    let relation = match wire {
                        Some(snapshot) => snapshot.session,
                        None if !super::unproven_input_active() => {
                            sessions::snapshot_owner(owner, session)
                        }
                        None => None,
                    };
                    c.source = Some(Source {
                        session: relation.filter(|(_, world)| world.is_some()),
                        wire,
                        bootstrap: if relation.is_none_or(|(_, world)| world.is_none()) {
                            match wire {
                                Some(snapshot) => snapshot.bootstrap_target(owner, session),
                                None => sessions::bootstrap_load_target(owner, session, None),
                            }
                        } else {
                            None
                        },
                    });
                    if c.source
                        .and_then(|source| source.wire)
                        .is_some_and(|snapshot| {
                            !snapshot.matches_session(c.source.unwrap().session)
                        })
                    {
                        fault();
                    }
                }
            }
            81 if raw.gpr[1] as u32 == 2 => {
                // Fifth argument is the original source stream. The existing
                // native busy branch may reject without publishing anything.
                c.pointer = value(raw.invocation.caller_rsp + 0x28);
                let stream = copy_current::<4>(raw.gpr[2] + 8).map(u32::from_le_bytes) == Some(4);
                let path = if stream {
                    None
                } else {
                    path_input(raw.gpr[0], raw.gpr[2])
                };
                if (stream && c.pointer == 0) || (!stream && path.is_none()) {
                    raw.invocation.cookie = COOKIE;
                    raw.invocation.token = c.serial;
                    CALLS.with(|s| s.borrow_mut().push(c));
                    return;
                }
                let source = if stream {
                    CALLS
                        .with(|s| {
                            s.borrow()
                                .iter()
                                .rev()
                                .find(|c| c.kind == 80)
                                .and_then(|c| c.source)
                        })
                        .unwrap_or(Source {
                            session: None,
                            wire: None,
                            bootstrap: None,
                        })
                } else {
                    // A local pathname intent does not borrow the current
                    // Session, an outer packet, or an old World ticket.
                    Source {
                        session: None,
                        wire: None,
                        bootstrap: None,
                    }
                };
                let mut s = state();
                let request = if let Some(path) = path {
                    s.ledger.enter_path(path)
                } else {
                    s.ledger.enter()
                }
                .unwrap_or_else(|_| fault());
                s.sources.insert(request.0, source);
                c.source = Some(source);
                c.request = Some(request);
            }
            82 => {
                c.descriptor = state().ledger.begin_copy().unwrap_or_else(|_| fault());
                if caller(raw, 0x16bd51) {
                    if let Some(request) = parent_request() {
                        let source = CALLS
                            .with(|s| {
                                s.borrow()
                                    .iter()
                                    .rev()
                                    .find(|c| c.kind == 81)
                                    .map(|c| c.pointer)
                            })
                            .unwrap_or(0);
                        if raw.gpr[0] != source {
                            fault();
                        }
                        c.request = Some(request);
                    }
                }
            }
            92 if caller(raw, 0x166bcb) => {
                let parent = CALLS.with(|s| {
                    s.borrow()
                        .iter()
                        .rev()
                        .find(|c| c.kind == 82)
                        .map(|c| (c.descriptor, c.request))
                });
                if let Some((copy, request)) = parent {
                    c.descriptor = copy;
                    c.pointer = raw.gpr[0];
                    c.request = request;
                }
            }
            83 => {
                if let Some(request) = parent_request() {
                    // 165820 tail-calls this allocator, retaining163DEA as the
                    // original return address. The unrelated166262 producer
                    // must never inherit this load request.
                    if caller(raw, 0x163dea) {
                        let index = dword(exe(0x1ad4c84));
                        if raw.gpr[0] != exe(0x1ad4d00) + u64::from(index) * 0x800180 {
                            fault();
                        }
                        c.pointer = raw.gpr[0];
                        c.request = Some(request);
                        c.allocation = Some(ALLOCATION.lock().unwrap_or_else(|_| fault()));
                    }
                }
            }
            84 => {
                let allocation = ALLOCATION.lock().unwrap_or_else(|_| fault());
                let bank = CALLS.with(|s| {
                    s.borrow()
                        .iter()
                        .rev()
                        .find(|c| c.kind == 90)
                        .map(|c| (c.pointer, c.descriptor))
                });
                // Kind36's descriptor is selected by the actual native
                // bank consumer. Its table index is not the callback-list
                // index used during setup.
                let owner = bank.filter(|(bank, table)| {
                    raw.gpr[1] == table + 0x36 * 0x30
                        && raw.gpr[0] >= bank + 0x90
                        && raw.gpr[0] < bank + 0x400080
                });
                if let Some((bank, _)) = owner {
                    c.request = state()
                        .ledger
                        .consuming(raw.gpr[0], bank)
                        .unwrap_or_else(|_| fault());
                }
                if c.request.is_some() {
                    if value(raw.gpr[1] + 0x20) != exe(0xef220) {
                        fault();
                    }
                    c.pointer = raw.gpr[0];
                    c.descriptor = raw.gpr[1];
                }
                drop(allocation);
            }
            85 if caller(raw, 0xdb88d) => {
                let command = CALLS.with(|s| {
                    s.borrow()
                        .iter()
                        .rev()
                        .find(|c| c.kind == 84)
                        .and_then(|c| c.request.map(|r| (r, c.pointer)))
                });
                if let Some((request, payload)) = command {
                    if raw.gpr[1] != payload {
                        fault();
                    }
                    c.engine = raw.gpr[0];
                    c.pointer = value(c.engine + 0x14f0);
                    if c.pointer == 0 || value(c.pointer) != exe(0xfad3f0) {
                        fault();
                    }
                    state()
                        .ledger
                        .selected(request, payload, c.engine, c.pointer)
                        .unwrap_or_else(|_| fault());
                    c.request = Some(request);
                }
            }
            86 if caller(raw, 0x63fcd) => {
                c.engine = value(exe(0x12c3c88));
                c.pointer = raw.gpr[0];
                if c.engine != 0
                    && value(c.engine + 0x14f0) == c.pointer
                    && value(c.engine + 0x5400) == c.pointer
                {
                    let mut s = state();
                    c.invocation = if s.ledger.path_pending(c.engine) {
                        let input = configured_path().unwrap_or_else(|| fault());
                        s.ledger.initialize_path(c.engine, c.pointer, &input)
                    } else {
                        s.ledger
                            .initialize(c.engine, c.pointer, value(exe(0x22d8d18)))
                    }
                    .unwrap_or_else(|_| fault());
                }
            }
            87 => {
                c.pointer = raw.gpr[0];
                let retained = JOB_CALLS.with(|s| {
                    s.borrow()
                        .iter()
                        .rev()
                        .find(|c| c.kind == 21)
                        .and_then(|c| c.invocation.map(|id| (id, c.view)))
                });
                let mut s = state();
                if let Some((invocation, view)) = retained {
                    if c.pointer != view || !caller(raw, 0x985b63) {
                        fault();
                    }
                    if s.ledger.worker(c.pointer).unwrap_or_else(|_| fault()) != Some(invocation) {
                        fault();
                    }
                    c.invocation = Some(invocation);
                }
            }
            89 => {
                let parent = CALLS.with(|s| {
                    s.borrow()
                        .iter()
                        .rev()
                        .find(|c| c.kind == 86)
                        .and_then(|c| c.invocation.map(|id| (id, c.pointer)))
                });
                if let Some((invocation, view)) = parent {
                    if !caller(raw, 0x83144)
                        || raw.gpr[0] != view + 0x240
                        || value(view + 0x290) != view
                        || value(view + 0x298) != exe(0x82da0)
                    {
                        fault();
                    }
                    c.invocation = Some(invocation);
                    c.pointer = view;
                }
            }
            90 => {
                c.pointer = raw.gpr[1];
                c.descriptor = raw.gpr[2];
            }
            91 => {
                let view = raw.gpr[0].saturating_sub(0x240);
                let s = state();
                if let Some(invocation) = s.ledger.current(view) {
                    if let Some(group) = s.groups.get(&invocation.0).copied() {
                        if value(raw.gpr[0]) != exe(0xfad3e0) || value(raw.gpr[0] + 0x40) != group {
                            fault();
                        }
                        c.invocation = Some(invocation);
                        c.engine = group;
                        c.pointer = raw.invocation.native_rsp;
                    }
                }
            }
            88 => {
                if let Some(invocation) = current_worker() {
                    let (kind, input) = if raw.gpr[1] == 0 {
                        (dword(exe(0x22d8d20)), value(exe(0x22d8d18)))
                    } else {
                        (raw.gpr[0] as u32, raw.gpr[1])
                    };
                    state()
                        .ledger
                        .input_read(invocation, kind, input)
                        .unwrap_or_else(|_| fault());
                    c.invocation = Some(invocation);
                }
            }
            81 | 85 | 86 | 92 => {}
            _ => fault(),
        }
        raw.invocation.cookie = COOKIE;
        raw.invocation.token = c.serial;
        CALLS.with(|s| s.borrow_mut().push(c));
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
    let c = CALLS
        .with(|s| s.borrow_mut().pop())
        .unwrap_or_else(|| fault());
    if c.serial != raw.invocation.token || c.kind != raw.invocation.kind {
        fault();
    }
    if exceptional && (c.request.is_some() || c.invocation.is_some()) {
        state().ledger.unresolved();
        FAILED.store(true, Ordering::Release);
        if let Some(f) = FRAMES.get() {
            f.mark_unresolved();
        }
        return;
    }
    if exceptional {
        if c.kind == 82 {
            state().ledger.abandon_copy(c.descriptor);
        }
        return;
    }
    if c.kind == 92 && c.descriptor != 0 {
        if raw.gpr[4] != c.pointer {
            fault();
        }
        state()
            .ledger
            .stream_constructed(c.descriptor, c.pointer)
            .unwrap_or_else(|_| fault());
        return;
    }
    if c.kind == 82 {
        state()
            .ledger
            .end_copy(c.descriptor, c.request, value(exe(0x22d8d18)))
            .unwrap_or_else(|_| fault());
        return;
    }
    if let Some(request) = c.request {
        let mut s = state();
        match c.kind {
            81 => {
                let accepted = raw.gpr[4] as u8 == 1;
                let result = s.ledger.returned(request, accepted);
                if accepted && result.is_ok() {
                    s.accepted.insert(request.0);
                    if let Some(world) = s.worlds.get(&request.0).copied() {
                        let source = *s.sources.get(&request.0).unwrap();
                        if let Some(target) = source.bootstrap {
                            if let Err(reason) = sessions::bind_bootstrap_world(target, world) {
                                s.binding_rejections.insert(request.0, reason);
                            }
                        } else if let Some((lease, previous)) = source.session {
                            sessions::loaded_world(
                                lease,
                                previous,
                                sessions::SessionWorldReceipt {
                                    binding_id: world.serial,
                                    epoch: world.epoch,
                                },
                            )
                            .unwrap_or_else(|_| fault());
                        }
                    }
                }
                result
            }
            83 if raw.gpr[4] != 0 => {
                let payload = raw.gpr[4];
                // The original CAS selected the bank. Do not reuse a selector
                // sampled before that native read, as the consumer can flip it.
                let bank = [c.pointer, c.pointer + 0x400080]
                    .into_iter()
                    .find(|bank| payload >= bank + 0x90 && payload < bank + 0x400080)
                    .unwrap_or_else(|| fault());
                if dword(payload - 0x10) != 0x280 || dword(payload - 0xc) != 0x36 {
                    fault();
                }
                s.ledger.issued(request, payload, bank)
            }
            83 => Ok(()),
            84 => s.ledger.consumed(request, c.pointer),
            85 => {
                if value(c.engine + 0x53f8) == c.pointer {
                    Ok(())
                } else {
                    Err(ledger::Error::Stale)
                }
            }
            _ => Err(ledger::Error::Stale),
        }
        .unwrap_or_else(|_| fault());
    }
    if let Some(invocation) = c.invocation {
        match c.kind {
            86 => state()
                .ledger
                .initialized(invocation)
                .unwrap_or_else(|_| fault()),
            87 => state()
                .ledger
                .worker_returned(invocation)
                .unwrap_or_else(|_| fault()),
            88 | 89 => {}
            91 => {
                if !c.joined || state().tasks.values().any(|t| t.invocation == invocation) {
                    fault();
                }
                state()
                    .ledger
                    .joined(invocation)
                    .unwrap_or_else(|_| fault());
                state().groups.remove(&invocation.0);
            }
            _ => fault(),
        }
    }
    drop(c.allocation);
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
    }
    unsafe { SetLastError(error) };
    1
}

macro_rules! home_original {
    ($name:literal,$offset:literal,$register:literal,$unwind:literal,$slot:literal,$scratch:literal,$jump:literal) => {
        global_asm!(concat!(
            ".globl ",
            $name,
            "\n.seh_proc ",
            $name,
            "\n",
            $name,
            ":\nmov [rsp+",
            $offset,
            "],",
            $register,
            "\n",
            $unwind,
            ".seh_endprologue\n.globl ",
            $name,
            "_body\n",
            $name,
            "_body:\nmov [rsp+",
            $scratch,
            "],rax\nmov rax,[rip+NATIVE_LOAD_CONTINUE+",
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
home_original!(
    "native_load_snapshot_original",
    "8",
    "rbx",
    ".seh_savereg rbx,8\n",
    "0",
    "0x10",
    "0x18"
);
home_original!(
    "native_load_group_original",
    "8",
    "rbx",
    ".seh_savereg rbx,8\n",
    "88",
    "0x10",
    "0x18"
);
home_original!(
    "native_load_request_original",
    "0x20",
    "r9",
    "",
    "8",
    "8",
    "0x10"
);
home_original!(
    "native_load_command_original",
    "8",
    "rbx",
    ".seh_savereg rbx,8\n",
    "32",
    "0x10",
    "0x18"
);
home_original!(
    "native_load_worker_original",
    "8",
    "rbx",
    ".seh_savereg rbx,8\n",
    "56",
    "0x10",
    "0x18"
);
global_asm!(
    ".globl native_load_copy_original",
    ".seh_proc native_load_copy_original",
    "native_load_copy_original:",
    "push rbp",
    ".seh_pushreg rbp",
    "push rsi",
    ".seh_pushreg rsi",
    "push rdi",
    ".seh_pushreg rdi",
    "push r15",
    ".seh_pushreg r15",
    ".seh_endprologue",
    ".globl native_load_copy_original_body",
    "native_load_copy_original_body:",
    "mov [rsp+0x28],rax",
    "mov rax,[rip+NATIVE_LOAD_CONTINUE+16]",
    "mov [rsp+0x30],rax",
    "mov rax,[rsp+0x28]",
    "jmp qword ptr [rsp+0x30]",
    ".seh_endproc",
    ".globl native_load_allocate_original",
    ".seh_proc native_load_allocate_original",
    "native_load_allocate_original:",
    ".seh_endprologue",
    "mov eax,[rcx+0x800100]",
    "mov [rsp+8],rax",
    "mov rax,[rip+NATIVE_LOAD_CONTINUE+24]",
    "mov [rsp+0x10],rax",
    "mov rax,[rsp+8]",
    "jmp qword ptr [rsp+0x10]",
    ".seh_endproc",
    ".globl native_load_stream_birth_original",
    ".seh_proc native_load_stream_birth_original",
    "native_load_stream_birth_original:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    "mov [rsp+8],rax",
    "mov rax,[rip+NATIVE_LOAD_CONTINUE+96]",
    "mov [rsp+0x10],rax",
    "mov rax,[rsp+8]",
    "jmp qword ptr [rsp+0x10]",
    ".seh_endproc",
    ".globl native_load_select_original",
    ".seh_proc native_load_select_original",
    "native_load_select_original:",
    ".seh_endprologue",
    "mov rax,[rcx+0x14f0]",
    "mov [rsp+8],rax",
    "mov rax,[rip+NATIVE_LOAD_CONTINUE+40]",
    "mov [rsp+0x10],rax",
    "mov rax,[rsp+8]",
    "jmp qword ptr [rsp+0x10]",
    ".seh_endproc",
    ".globl native_load_initialize_original",
    ".seh_proc native_load_initialize_original",
    "native_load_initialize_original:",
    "push rbp",
    ".seh_pushreg rbp",
    "push r12",
    ".seh_pushreg r12",
    "push r14",
    ".seh_pushreg r14",
    ".seh_endprologue",
    ".globl native_load_initialize_original_body",
    "native_load_initialize_original_body:",
    "mov [rsp+0x20],rax",
    "mov rax,[rip+NATIVE_LOAD_CONTINUE+48]",
    "mov [rsp+0x28],rax",
    "mov rax,[rsp+0x20]",
    "jmp qword ptr [rsp+0x28]",
    ".seh_endproc",
    // This leaf's original first branch is relative. Route it to its fixed
    // original target; the fallthrough resumes after the full5byte prefix.
    ".globl native_load_stream_original",
    ".seh_proc native_load_stream_original",
    "native_load_stream_original:",
    ".seh_endprologue",
    "test rdx,rdx",
    "jz 2f",
    "jmp qword ptr [rip+NATIVE_LOAD_STREAM_NONNULL]",
    "2:",
    "jmp qword ptr [rip+NATIVE_LOAD_CONTINUE+64]",
    ".seh_endproc",
    ".globl native_load_start_original",
    ".seh_proc native_load_start_original",
    "native_load_start_original:",
    "sub rsp,0x38",
    ".seh_stackalloc 0x38",
    ".seh_endprologue",
    "mov rax,[rip+NATIVE_LOAD_ASYNC_LABEL]",
    "mov [rsp+0x20],rax",
    "mov rax,[rip+NATIVE_LOAD_CONTINUE+72]",
    "mov [rsp+0x28],rax",
    "mov rax,[rsp+0x20]",
    "jmp qword ptr [rsp+0x28]",
    ".seh_endproc",
    ".globl native_load_bank_original",
    ".seh_proc native_load_bank_original",
    "native_load_bank_original:",
    "push rsi",
    ".seh_pushreg rsi",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    "mov [rsp+8],rax",
    "mov rax,[rip+NATIVE_LOAD_CONTINUE+80]",
    "mov [rsp+0x10],rax",
    "mov rax,[rsp+8]",
    "jmp qword ptr [rsp+0x10]",
    ".seh_endproc",
);
// The pinned label value is a module address, so restore the exact LEA target
// through a writable scalar, rather than redirecting the original metadata.
#[unsafe(no_mangle)]
static NATIVE_LOAD_ASYNC_LABEL: AtomicU64 = AtomicU64::new(0);
#[unsafe(no_mangle)]
static NATIVE_LOAD_STREAM_NONNULL: AtomicU64 = AtomicU64::new(0);
unsafe extern "system" {
    fn native_load_snapshot_original();
    fn native_load_request_original();
    fn native_load_copy_original();
    fn native_load_allocate_original();
    fn native_load_command_original();
    fn native_load_select_original();
    fn native_load_initialize_original();
    fn native_load_worker_original();
    fn native_load_stream_original();
    fn native_load_start_original();
    fn native_load_start_gateway();
    fn native_load_bank_original();
    fn native_load_bank_gateway();
    fn native_load_group_original();
    fn native_load_group_gateway();
    fn native_load_stream_birth_original();
    fn native_load_stream_birth_gateway();
    fn native_load_snapshot_gateway();
    fn native_load_request_gateway();
    fn native_load_copy_gateway();
    fn native_load_allocate_gateway();
    fn native_load_command_gateway();
    fn native_load_select_gateway();
    fn native_load_initialize_gateway();
    fn native_load_worker_gateway();
    fn native_load_stream_gateway();
}
gateway!(
    "native_load_snapshot_gateway",
    "80",
    native_load_snapshot_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_load_request_gateway",
    "81",
    native_load_request_original,
    begin,
    end,
    unwind,
    "mov rax,[rsp+0x1f0]\nmov [rsp+0x20],rax\nmov rax,[rsp+0x40]\n"
);
gateway!(
    "native_load_copy_gateway",
    "82",
    native_load_copy_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_load_allocate_gateway",
    "83",
    native_load_allocate_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_load_command_gateway",
    "84",
    native_load_command_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_load_select_gateway",
    "85",
    native_load_select_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_load_initialize_gateway",
    "86",
    native_load_initialize_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_load_worker_gateway",
    "87",
    native_load_worker_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_load_stream_gateway",
    "88",
    native_load_stream_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_load_start_gateway",
    "89",
    native_load_start_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_load_bank_gateway",
    "90",
    native_load_bank_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_load_group_gateway",
    "91",
    native_load_group_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_load_stream_birth_gateway",
    "92",
    native_load_stream_birth_original,
    begin,
    end,
    unwind
);
fn gateways() -> [usize; 13] {
    [
        native_load_snapshot_gateway as *const () as usize,
        native_load_request_gateway as *const () as usize,
        native_load_copy_gateway as *const () as usize,
        native_load_allocate_gateway as *const () as usize,
        native_load_command_gateway as *const () as usize,
        native_load_select_gateway as *const () as usize,
        native_load_initialize_gateway as *const () as usize,
        native_load_worker_gateway as *const () as usize,
        native_load_stream_gateway as *const () as usize,
        native_load_start_gateway as *const () as usize,
        native_load_bank_gateway as *const () as usize,
        native_load_group_gateway as *const () as usize,
        native_load_stream_birth_gateway as *const () as usize,
    ]
}
const SITES: [(usize, [u8; 8], usize); 13] = [
    (0x273f40, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x74], 5),
    (
        0x16b5c0,
        [0x4c, 0x89, 0x4c, 0x24, 0x20, 0x55, 0x53, 0x41],
        5,
    ),
    (
        0x166b40,
        [0x40, 0x55, 0x56, 0x57, 0x41, 0x57, 0x48, 0x8d],
        6,
    ),
    (0x162480, [0x8b, 0x81, 0, 1, 0x80, 0, 0x83, 0xe0], 6),
    (0xef220, [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83], 5),
    (0x6b7d0, [0x48, 0x8b, 0x81, 0xf0, 0x14, 0, 0, 0x48], 7),
    (0x82e50, [0x40, 0x55, 0x41, 0x54, 0x41, 0x56, 0x48, 0x8d], 6),
    (0x82da0, [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83], 5),
    (
        0x16d280,
        [0x48, 0x85, 0xd2, 0x75, 0x16, 0x48, 0x8b, 0x15],
        5,
    ),
    (
        0x986150,
        [0x48, 0x83, 0xec, 0x38, 0x48, 0x8d, 0x05, 0x69],
        11,
    ),
    (0x5f470, [0x40, 0x56, 0x48, 0x83, 0xec, 0x20, 0x8b, 2], 6),
    (0x985b80, [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83], 5),
    (
        0x991920,
        [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8d],
        6,
    ),
];
pub(super) unsafe fn prepare_host(module: usize) -> Result<(), &'static str> {
    let _pin = super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA)?;
    unsafe {
        fixed::verify(module)?;
        install_at(module, &SITES)
    }
}
unsafe fn install_at(
    module: usize,
    sites: &[(usize, [u8; 8], usize); 13],
) -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "load hook owner")?;
    if !hooks.is_empty() {
        return Err("load hooks already installed");
    }
    for &(rva, expected, _) in sites {
        if copy_current::<8>((module + rva) as u64) != Some(expected) {
            return Err("load entry differs");
        }
    }
    EXE.store(module as u64, Ordering::Release);
    NATIVE_LOAD_STREAM_NONNULL.store((module + 0x16d29b) as u64, Ordering::Release);
    NATIVE_LOAD_ASYNC_LABEL.store((module + 0x106c4c4) as u64, Ordering::Release);
    for (i, &(rva, expected, stolen)) in sites.iter().enumerate() {
        let target = (module + rva) as *mut u8;
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if !(target as usize).is_multiple_of(8)
            || unsafe { VirtualQuery(target.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
            || info.State != MEM_COMMIT
            || info.Protect & 0xff != PAGE_EXECUTE_READ
            || info.Protect & PAGE_GUARD != 0
        {
            return Err("load entry protection");
        }
        let relay = unsafe { executable_near(target as usize, target as usize, stolen) }
            .ok_or("load relay")?;
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
            return Err("load relay publication");
        }
        let (patched, prefix) = native_entry::plan(target, expected, relay)?;
        NATIVE_LOAD_CONTINUE[i].store((module + rva + stolen) as u64, Ordering::Release);
        hooks.push(OwnedHook {
            hook: Hook {
                target,
                original: expected,
                patched,
                prefix,
                _code: relay,
            },
            protection: info.Protect,
        });
        unpublished.0 = null_mut();
        let result = unsafe {
            native_entry::publish(&hooks.last().ok_or("native hook owner missing")?.hook)
        }?;
        if !result.exchanged || !result.protection_restored || !result.cache_flushed {
            return Err("critical: load publication unverified");
        }
    }
    Ok(())
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "load hook owner")?;
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
        Err("critical: load restoration unverified")
    }
}
#[cfg(test)]
#[path = "native_loads_tests.rs"]
mod tests;
