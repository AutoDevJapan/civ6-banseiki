//! Partial native session lifecycle. Birth identities precede publication;
//! World and transport relations require independent, exact event receipts.
//! Unknown pre-install sessions remain unknown. No all-domain seal is issued.
use super::*;
use std::cell::RefCell;
#[path = "native_session_queue.rs"]
mod deferred;
#[path = "native_sessions_fixed.rs"]
mod fixed;
#[path = "native_input.rs"]
mod input;
#[path = "native_sessions_ledger.rs"]
mod ledger;
#[path = "native_registry.rs"]
mod registry;
#[path = "native_transport.rs"]
pub(super) mod transports;
pub(super) use ledger::BootstrapTarget;
#[cfg(test)]
use ledger::WorldReceipt;
use ledger::{Error, Lease, Ledger, Message, TerminalTarget, Transport};
pub use ledger::{
    Error as SessionError, Lease as SessionLease, Message as SessionMessage,
    Transport as SessionTransport, WorldReceipt as SessionWorldReceipt,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PacketReceipt {
    pub serial: u64,
    pub lease: SessionLease,
    pub transport: u64,
    pub world: Option<SessionWorldReceipt>,
}
pub(super) fn message72_active() -> bool {
    CALLS.with(|calls| calls.borrow().iter().any(|call| call.kind == 72))
}
pub(super) fn admitted_source_world(session: u64) -> Option<SessionWorldReceipt> {
    CALLS
        .with(|calls| {
            calls
                .borrow()
                .iter()
                .rev()
                .find(|call| matches!(call.kind, 72 | 73))
                .and_then(|call| call.message)
        })
        .and_then(|message| {
            let state = state();
            (state.message_session(message).ok()? == session)
                .then(|| state.message_world(message).ok())
                .flatten()
        })
}
#[cfg(test)]
pub(super) fn fixture_snapshot_session(owner: u64, session: u64) -> SessionLease {
    let mut state = state();
    let lease = state.birth(1, Some(1), owner, 2).unwrap();
    state.factory(lease, 7, owner, session, 9).unwrap();
    state.published(lease, 7).unwrap();
    state
        .bind_world(
            lease,
            SessionWorldReceipt {
                binding_id: 10,
                epoch: 3,
            },
        )
        .unwrap();
    lease
}
#[cfg(test)]
pub(super) fn fixture_snapshot_message<R>(lease: SessionLease, body: impl FnOnce() -> R) -> R {
    let message = {
        let mut state = state();
        state.bind_transport(lease, 20, 30, 40).unwrap();
        let message = state.reserve(lease).unwrap();
        let (_, id, callback) = state.snapshot(lease).unwrap();
        state.dispatch(message, callback, id).unwrap();
        message
    };
    CALLS.with(|calls| {
        calls.borrow_mut().push(Call {
            serial: 1,
            kind: 73,
            lease: Some(lease),
            hold: None,
            message: Some(message),
            registry: 0,
            owner: 0,
            id: 0,
            transport: None,
            native_transport: false,
            unproven_transport: None,
            unproven_join: false,
            thread: 0,
            unproven_hold: None,
            packet: None,
        })
    });
    let result = body();
    CALLS.with(|calls| {
        calls.borrow_mut().pop();
    });
    state().dispatched(message).unwrap();
    result
}
pub(super) fn current_packet() -> Option<PacketReceipt> {
    CALLS
        .with(|calls| {
            calls
                .borrow()
                .iter()
                .rev()
                .find(|call| matches!(call.kind, 72 | 73))
                .map(|call| call.packet)
        })
        .unwrap_or_else(input::current_packet)
}

static EXE: AtomicU64 = AtomicU64::new(0);
static CORE: AtomicU64 = AtomicU64::new(0);
static NEXT: AtomicU64 = AtomicU64::new(0);
static LEDGER: OnceLock<Mutex<Ledger>> = OnceLock::new();
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
pub(super) fn hook_entries_owned() -> bool {
    owns_hook_entries(&HOOKS, SITES.len())
        && registry::hook_entries_owned()
        && transports::hook_entries_owned()
        && input::hook_entries_owned()
        && deferred::hook_entries_owned()
}

static SCOPE: OnceLock<SessionScopeHooks> = OnceLock::new();
static REGISTRIES: std::sync::LazyLock<Mutex<std::collections::HashMap<u64, u64>>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));
#[unsafe(no_mangle)]
static NATIVE_SESSION_CONTINUE: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];
/// Connects to the existing NativeFrames owner, never a second frame ledger.
/// Enter must accept only the supplied existing epoch and reject post-drain.
pub struct SessionScopeHooks {
    pub enter_existing_epoch: fn(u64) -> Result<u64, SessionError>,
    pub leave: fn(u64, bool) -> Result<(), SessionError>,
}
/// Rust integration endpoints. Receipt creation belongs to the binding owner;
/// this adapter checks identity and lifetime but cannot prove its provenance.
pub struct NativeSessionApi;
impl NativeSessionApi {
    /// Install while still in the cold Host preparation phase, before birth.
    /// Loaded code must be the pinned, unmodified original image.
    ///
    /// # Safety
    /// `exe` must be this process's pinned Host module and the lifecycle owner
    /// must serialize installation and retain this bridge for process lifetime.
    pub unsafe fn prepare_host(exe: usize) -> Result<(), &'static str> {
        let _pin = super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA)?;
        unsafe {
            verify_loaded(exe)?;
            registry::prepare_host(exe)?;
            transports::prepare_host(exe)?;
            input::prepare_host(exe)?;
            deferred::install(exe)?;
            install_at(exe)
        }
    }
    pub fn bind_core(core: u64, hooks: SessionScopeHooks) -> Result<(), SessionError> {
        if core == 0 || EXE.load(Ordering::Acquire) == 0 {
            return Err(Error::Unbound);
        }
        SCOPE.set(hooks).map_err(|_| Error::Busy)?;
        CORE.store(core, Ordering::Release);
        Ok(())
    }
    /// This lifetime token must originate at the registry's native birth.
    /// A call-site/current-World lookup is not such a token.
    pub fn with_registry<R>(
        registry: u64,
        lifetime: u64,
        f: impl FnOnce() -> R,
    ) -> Result<R, SessionError> {
        if registry == 0 || lifetime == 0 {
            return Err(Error::Unbound);
        }
        REGISTRY.with(|s| s.borrow_mut().push((registry, lifetime)));
        let guard = RegistryGuard;
        let value = f();
        drop(guard);
        Ok(value)
    }
    pub fn born(registry: u64, lifetime: u64, id: u32) -> Option<SessionLease> {
        state().born(registry, lifetime, id)
    }
    pub fn bind_world(lease: SessionLease, world: SessionWorldReceipt) -> Result<(), SessionError> {
        state().bind_world(lease, world)
    }
    /// Arguments are copied from the actual transport-publication event.
    /// A replacement transport always needs a different session incarnation.
    pub fn bind_transport(
        lease: SessionLease,
        pointer: u64,
        network: u64,
        thread: u64,
    ) -> Result<SessionTransport, SessionError> {
        state().bind_transport(lease, pointer, network, thread)
    }
    pub fn reserve_message(lease: SessionLease) -> Result<SessionMessage, SessionError> {
        state().reserve(lease)
    }
    /// The reservation must travel with its original queued input. Entering a
    /// delayed callback never reserves from a raw pointer or current Context.
    pub fn with_message<R>(
        message: SessionMessage,
        f: impl FnOnce() -> R,
    ) -> Result<R, SessionError> {
        state().pending(message)?;
        INPUT.with(|s| s.borrow_mut().push(message));
        let guard = InputGuard;
        let value = f();
        drop(guard);
        Ok(value)
    }
    pub fn cancelled_before_dispatch(message: SessionMessage) -> Result<(), SessionError> {
        state().cancel_before_dispatch(message)
    }
    /// Only accepted admission frames supply inheritance to Core operations.
    pub fn current_world() -> Option<SessionWorldReceipt> {
        CALLS
            .with(|s| {
                s.borrow()
                    .iter()
                    .rev()
                    .find(|s| s.kind == 73)
                    .and_then(|s| s.message)
            })
            .and_then(|message| state().message_world(message).ok())
    }
    pub fn admission_active() -> bool {
        CALLS.with(|s| s.borrow().iter().any(|s| s.kind == 72 || s.kind == 73))
    }
    pub fn unproven_input_active() -> bool {
        CALLS.with(|s| {
            s.borrow()
                .iter()
                .any(|c| matches!(c.kind, 72 | 73) && c.message.is_none())
        })
    }
    pub fn drained(world: SessionWorldReceipt) -> Result<(), SessionError> {
        if deferred::pending().map_err(|_| Error::Unresolved)? != 0
            || input::pending().map_err(|_| Error::Unresolved)? != 0
        {
            return Err(Error::Busy);
        }
        state().drained(world)
    }
    /// The lifecycle owner must first drain every native and bootstrap caller.
    /// Relay and unwind storage stays resident even after entry restoration.
    ///
    /// # Safety
    /// The caller must own the installed entries and ensure all native callers
    /// have exited before invoking restoration. Published relays stay resident.
    pub unsafe fn restore() -> Result<(), &'static str> {
        let input = unsafe { input::restore() };
        let registry = unsafe { registry::restore() };
        let transport = unsafe { transports::restore() };
        let queued = unsafe { deferred::restore() };
        let session = unsafe { restore() };
        input.and(registry).and(transport).and(queued).and(session)
    }
}
#[cfg(test)]
pub(super) fn fixture_unknown_input<R>(body: impl FnOnce() -> R) -> R {
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 72;
    begin(&mut raw);
    let result = body();
    finish(&mut raw, false);
    result
}
struct RegistryGuard;
impl Drop for RegistryGuard {
    fn drop(&mut self) {
        REGISTRY.with(|s| {
            s.borrow_mut().pop();
        });
    }
}
struct InputGuard;
impl Drop for InputGuard {
    fn drop(&mut self) {
        INPUT.with(|s| {
            s.borrow_mut().pop();
        });
    }
}
struct Call {
    serial: u64,
    kind: u64,
    lease: Option<Lease>,
    hold: Option<u64>,
    message: Option<Message>,
    registry: u64,
    owner: u64,
    id: u32,
    transport: Option<Transport>,
    native_transport: bool,
    unproven_transport: Option<transports::UnprovenClose>,
    unproven_join: bool,
    thread: u64,
    unproven_hold: Option<u64>,
    packet: Option<PacketReceipt>,
}
thread_local! {
    static REGISTRY:RefCell<Vec<(u64,u64)>>=const {RefCell::new(Vec::new())};
    static INPUT:RefCell<Vec<Message>>=const {RefCell::new(Vec::new())};
    static CALLS:RefCell<Vec<Call>>=const {RefCell::new(Vec::new())};
}
fn state() -> std::sync::MutexGuard<'static, Ledger> {
    LEDGER
        .get_or_init(|| Mutex::new(Ledger::default()))
        .lock()
        .unwrap_or_else(|_| fault())
}
pub(super) fn snapshot_owner(
    owner: u64,
    session: u64,
) -> Option<(SessionLease, Option<SessionWorldReceipt>)> {
    let s = state();
    let lease = s.snapshot_owner(owner, session)?;
    Some((lease, s.world(lease).ok()))
}
pub(super) fn loaded_world(
    lease: SessionLease,
    previous: Option<SessionWorldReceipt>,
    world: SessionWorldReceipt,
) -> Result<(), SessionError> {
    let result = state().loaded_world(lease, previous, world);
    census::event(if result.is_ok() {
        census::SemanticEvent::BindWorld
    } else {
        census::SemanticEvent::BindRejected
    });
    result
}
/// Only the exact native snapshot owner and a Session born in the observed
/// pre-World window can propose an initial binding. The input remains
/// unqualified; this target issues no packet or product authority.
pub(super) fn bootstrap_load_target(
    owner: u64,
    session: u64,
    packet: Option<PacketReceipt>,
) -> Option<BootstrapTarget> {
    let target = state().bootstrap_target(owner, session)?;
    if (NativeSessionApi::admission_active() || super::unproven_input_active()) && packet.is_none()
    {
        return None;
    }
    if packet.is_some_and(|p| {
        p.lease != target.lease
            || p.world.is_some()
            || !state().bootstrap_packet_matches(target.lease, p.transport)
    }) {
        return None;
    }
    Some(target)
}
#[cfg(test)]
pub(super) fn fixture_bootstrap_session(owner: u64, session: u64) -> SessionLease {
    let mut ledger = state();
    let lease = ledger.birth(1, Some(1), owner, 2).unwrap();
    ledger.record_birth_window(lease, worlds::fixture_first_birth_window());
    ledger
        .factory(lease, lease.0 as u32, owner, session, 0x9000 + lease.0)
        .unwrap();
    ledger.published(lease, lease.0 as u32).unwrap();
    lease
}
#[cfg(test)]
pub(super) fn fixture_bound_session_world(lease: SessionLease) -> Option<SessionWorldReceipt> {
    state().world(lease).ok()
}
#[cfg(test)]
pub(super) fn fixture_retire_load_world(
    lease: SessionLease,
    unproven_transport: bool,
) -> SessionWorldReceipt {
    let mut ledger = state();
    let previous = ledger.world(lease).unwrap();
    if unproven_transport {
        let transport = ledger.publish_transport(lease, 100, 50, 60, 70).unwrap();
        ledger
            .unproven_transport_close_begin(lease, transport)
            .unwrap();
        ledger
            .unproven_transport_close_end(lease, transport, false)
            .unwrap();
    }
    ledger.retire_world(previous).unwrap();
    previous
}
pub(super) fn bind_bootstrap_world(
    target: BootstrapTarget,
    world: worlds::WorldLease,
) -> Result<(), SessionError> {
    let result = worlds::with_observed_birth(world, || state().bind_bootstrap_world(target, world))
        .unwrap_or(Err(Error::Unresolved));
    census::event(if result.is_ok() {
        census::SemanticEvent::BindWorld
    } else {
        census::SemanticEvent::BindRejected
    });
    result
}
pub(super) fn retire_world(world: SessionWorldReceipt) -> Result<(), SessionError> {
    if deferred::pending().map_err(|_| Error::Unresolved)? != 0
        || input::pending().map_err(|_| Error::Unresolved)? != 0
    {
        return Err(Error::Busy);
    }
    state().retire_world(world)
}
pub(super) fn existing_transport_owner(
    session: u64,
    callback: u64,
) -> Result<Option<SessionLease>, SessionError> {
    state().existing_transport_owner(session, callback)
}
pub(super) fn publish_transport(
    lease: SessionLease,
    incarnation: u64,
    pointer: u64,
    network: u64,
    thread: u64,
) -> Result<SessionTransport, SessionError> {
    let result = state().publish_transport(lease, incarnation, pointer, network, thread);
    if result.is_ok() {
        census::event(census::SemanticEvent::TransportPublished);
    }
    result
}
pub(super) fn native_transport_reset_begin(
    lease: SessionLease,
    transport: SessionTransport,
) -> Result<(), SessionError> {
    state().native_reset_begin(lease, transport)
}
pub(super) fn native_transport_reset_returned(
    lease: SessionLease,
    transport: SessionTransport,
) -> Result<(), SessionError> {
    state().native_reset_returned(lease, transport)
}
pub(super) fn transport_disconnect_begin(
    lease: SessionLease,
) -> Result<Option<SessionTransport>, SessionError> {
    state().transport_disconnect_begin(lease)
}
pub(super) fn transport_join_result(
    lease: SessionLease,
    transport: SessionTransport,
    thread: u64,
    al: u8,
) -> Result<(), SessionError> {
    state().transport_join_result(lease, transport, thread, al)
}
pub(super) fn transport_disconnect_end(lease: SessionLease) -> Result<(), SessionError> {
    state().transport_disconnect_end(lease)
}
pub(super) fn terminal_remove_begin(
    registry: u64,
    lifetime: u64,
    id: u32,
    session: u64,
) -> Result<Option<(SessionLease, Option<SessionTransport>)>, SessionError> {
    state().terminal_remove_begin(registry, lifetime, id, session)
}
pub(super) fn terminal_registry_begin(
    registry: u64,
    lifetime: u64,
) -> Result<Vec<TerminalTarget>, SessionError> {
    state().terminal_registry_begin(registry, lifetime)
}
pub(super) fn terminal_remove_end(lease: SessionLease) -> Result<(), SessionError> {
    state().terminal_remove_end(lease)
}
pub(super) fn terminal_registry_end(registry: u64, lifetime: u64) -> Result<(), SessionError> {
    state().terminal_registry_end(registry, lifetime)
}
pub(super) fn reserve_input(
    lease: SessionLease,
    transport: SessionTransport,
) -> Result<Option<SessionMessage>, SessionError> {
    state().reserve_input(lease, transport)
}
pub(super) fn fork_input(message: SessionMessage) -> Result<SessionMessage, SessionError> {
    state().fork_input(message)
}
fn unresolved(lease: Option<Lease>) {
    // An error can be reported while the failing ledger guard is still live.
    // Fatal paths must never block recursively on that same mutex.
    if let Some(m) = LEDGER.get() {
        if let Ok(mut s) = m.try_lock() {
            s.unresolved(lease);
        }
    }
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
    diagnostic(70);
    unresolved(None);
    std::process::abort()
}
fn serial() -> u64 {
    NEXT.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
        .unwrap_or_else(|_| fault())
        + 1
}
fn read64(p: u64) -> Result<u64, Error> {
    copy_current::<8>(p)
        .map(u64::from_le_bytes)
        .ok_or(Error::Unbound)
}
fn read32(p: u64) -> Result<u32, Error> {
    copy_current::<4>(p)
        .map(u32::from_le_bytes)
        .ok_or(Error::Unbound)
}
fn caller(raw: &Saved, rva: u64) -> bool {
    #[cfg(test)]
    if rva == 0x8573c9 {
        if let Some(pc) = transports::fixture_reset_join_caller() {
            return raw.invocation.caller == pc;
        }
    }
    #[cfg(test)]
    if let Some(pc) = tests::caller_override(rva) {
        return raw.invocation.caller == pc;
    }
    raw.invocation.caller == EXE.load(Ordering::Acquire) + rva
}
/// Exact native hash-chain layout, bounded only for our observation. An
/// unreadable/cyclic/oversized chain is unknown; it is never an empty slot.
fn registry_slot(registry: u64, id: u32) -> Result<u64, Error> {
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
            return Ok(0);
        }
        if !seen.insert(node) {
            return Err(Error::Unbound);
        }
        if read32(node)? == id {
            return read64(node + 8);
        }
        node = read64(node + 0x10)?;
    }
    Err(Error::Unbound)
}
fn snapshot_transport(lease: Lease) -> Result<(), Error> {
    if snapshot_transport_active(lease)? {
        Ok(())
    } else {
        Err(Error::Unbound)
    }
}
fn validate_dispatch_owner(lease: Lease, id: u32) -> Result<(), Error> {
    let (session, _, _) = state().snapshot(lease)?;
    let registry = state().registry(lease)?;
    if read64(EXE.load(Ordering::Acquire) + 0x235b878)? != registry
        || registry_slot(registry, id)? != session
    {
        return Err(Error::Stale);
    }
    Ok(())
}
fn snapshot_transport_active(lease: Lease) -> Result<bool, Error> {
    let (session, _, _) = state().snapshot(lease)?;
    let t = state().transport(lease)?;
    let exe = EXE.load(Ordering::Acquire);
    if read64(session + 0x28)? != t.pointer
        || read64(t.pointer + 0x10)? != t.network
        || read64(t.network + 0xc0)? != t.thread
    {
        return Err(Error::Stale);
    }
    let table = read64(t.pointer)?;
    if ![exe + 0x1047e60, exe + 0x10482c8].contains(&table)
        || read64(table + 0x40)? != exe + 0x84ebd0
        || read64(table + 0x58)? != exe + 0x84ec00
        || read64(t.thread)? != exe + 0x106d030
        || read64(exe + 0x106d060)? != exe + 0x98c650
    {
        return Err(Error::Unbound);
    }
    Ok(copy_current::<1>(t.pointer + 0x45) == Some([1]))
}
fn enter_scope(message: Message) -> Result<u64, Error> {
    if CORE.load(Ordering::Acquire) == 0 {
        return Err(Error::Unbound);
    }
    let epoch = state().message_world(message)?.epoch;
    (SCOPE.get().ok_or(Error::Unbound)?.enter_existing_epoch)(epoch)
}
extern "system" fn begin(raw: *mut Saved) {
    let e = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        let mut c = Call {
            serial: serial(),
            kind: raw.invocation.kind,
            lease: None,
            hold: None,
            message: None,
            registry: 0,
            owner: 0,
            id: 0,
            transport: None,
            native_transport: false,
            unproven_transport: None,
            unproven_join: false,
            thread: 0,
            unproven_hold: None,
            packet: None,
        };
        match c.kind {
            70 => {
                census::event(match raw.gpr[1] as u32 {
                    0 => census::SemanticEvent::Birth0,
                    1 => census::SemanticEvent::Birth1,
                    2 => census::SemanticEvent::Birth2,
                    3 => census::SemanticEvent::Birth3,
                    4 => census::SemanticEvent::Birth4,
                    _ => census::SemanticEvent::BirthOther,
                });
                c.registry = raw.gpr[0];
                c.owner = raw.gpr[2];
                let table = read64(c.registry).ok();
                if table.and_then(|t| read64(t + 0x18).ok())
                    == Some(EXE.load(Ordering::Acquire) + 0x7f8ab0)
                {
                    let lifetime = REGISTRY
                        .with(|s| {
                            s.borrow()
                                .iter()
                                .rev()
                                .find(|(p, _)| *p == c.registry)
                                .map(|(_, id)| *id)
                        })
                        .or_else(|| REGISTRIES.lock().ok()?.get(&c.registry).copied());
                    c.lease = Some(
                        state()
                            .birth(c.registry, lifetime, c.owner, raw.gpr[1] as u32)
                            .unwrap_or_else(|_| fault()),
                    );
                    if let Some(window) = worlds::capture_birth_window() {
                        state().record_birth_window(c.lease.unwrap(), window);
                    }
                }
            }
            71 if caller(raw, 0x7f8cf5) => {
                let parent = CALLS.with(|s| {
                    s.borrow()
                        .last()
                        .filter(|s| s.kind == 70)
                        .map(|s| (s.lease, s.registry, s.owner))
                });
                if let Some((lease, registry, owner)) = parent {
                    if raw.gpr[3] != owner {
                        fault();
                    }
                    c.lease = lease;
                    c.registry = registry;
                    c.owner = owner;
                    c.id = raw.gpr[2] as u32;
                    let old = registry_slot(registry, c.id).unwrap_or_else(|_| fault());
                    if old != 0 && !state().retired_slot(registry, c.id, old) {
                        fault();
                    }
                }
            }
            72 => {
                c.packet = input::current_packet();
                // The explicit input ticket is acquired BEFORE ID lookup.
                // Missing upstream provenance stays unknown, even if a raw
                // callback address happens to name a newly bound session.
                // The real receive queue must retain the packet's original
                // receipt before this callback. A registry/callback match
                // proves the Session incarnation, never the input's World.
                let message = input::incoming(raw)
                    .unwrap_or_else(|| INPUT.with(|s| s.borrow().last().copied()));
                if let Some(message) = message {
                    let id = read32(raw.gpr[0] + 8).unwrap_or_else(|_| fault());
                    let reset_input = state()
                        .take_reset_input(message, raw.gpr[0], id)
                        .unwrap_or_else(|_| fault());
                    if let Some(lease) = reset_input {
                        validate_dispatch_owner(lease, id).unwrap_or_else(|_| fault());
                        c.packet = None;
                        diagnostic(2074);
                    } else {
                        let lease = state()
                            .dispatch(message, raw.gpr[0], id)
                            .unwrap_or_else(|_| fault());
                        let active = snapshot_transport_active(lease).unwrap_or_else(|_| fault());
                        validate_dispatch_owner(lease, id).unwrap_or_else(|_| fault());
                        if active {
                            c.hold = Some(enter_scope(message).unwrap_or_else(|_| fault()));
                            c.lease = Some(lease);
                            c.message = Some(message);
                        } else {
                            state()
                                .dispatched_unproven(message)
                                .unwrap_or_else(|_| fault());
                            c.packet = None;
                            diagnostic(2072);
                        }
                    }
                }
            }
            76 => {
                c.registry = raw.gpr[0];
            }
            77 => {
                c.registry = raw.gpr[0];
                if let Some(lifetime) = REGISTRIES
                    .lock()
                    .unwrap_or_else(|_| fault())
                    .get(&c.registry)
                    .copied()
                {
                    c.owner = lifetime;
                    c.thread =
                        registry::destroy_begin(c.registry, lifetime).unwrap_or_else(|_| fault());
                }
            }
            73 => {
                c.packet = deferred::packet(raw).unwrap_or_else(|| {
                    CALLS.with(|calls| {
                        calls
                            .borrow()
                            .iter()
                            .rev()
                            .find(|call| call.kind == 72)
                            .and_then(|call| call.packet)
                    })
                });
                let parent = deferred::admission(raw).unwrap_or_else(|| {
                    CALLS.with(|s| {
                        s.borrow()
                            .iter()
                            .rev()
                            .find(|s| s.kind == 72)
                            .and_then(|s| s.lease.zip(s.message))
                    })
                });
                if let Some((lease, message)) = parent {
                    let exe = EXE.load(Ordering::Acquire);
                    let vft = read64(raw.gpr[0]).unwrap_or_else(|_| fault());
                    if ![exe + 0x1043088, exe + 0x10436e0, exe + 0x1045bd8].contains(&vft)
                        || read64(vft + 0x238).ok() != Some(exe + 0x842030)
                    {
                        fault();
                    }
                    state()
                        .admitted(lease, raw.gpr[0])
                        .unwrap_or_else(|_| fault());
                    c.lease = Some(lease);
                    c.message = Some(message);
                }
            }
            74 => {
                // Diagnostic reads grant no ownership and never change the
                // close decision. Zero/unreadable remains distinct from AL1.
                census::event(match read64(raw.gpr[0].wrapping_add(0x28)).ok() {
                    Some(0) => census::SemanticEvent::CloseNull,
                    Some(pointer) => match copy_current::<1>(pointer.wrapping_add(0x45)) {
                        Some([0]) => census::SemanticEvent::CloseFlag0,
                        Some([1]) => census::SemanticEvent::CloseFlag1,
                        Some(_) => census::SemanticEvent::CloseFlagOther,
                        None => census::SemanticEvent::CloseFlagUnreadable,
                    },
                    None => census::SemanticEvent::CloseFlagUnreadable,
                });
                let close = transports::disconnect_begin(raw.gpr[0]).unwrap_or_else(|_| fault());
                #[cfg(test)]
                let manual = matches!(&close, transports::Disconnect::Untracked);
                match close {
                    transports::Disconnect::Untracked => {}
                    transports::Disconnect::Unproven(token) => c.unproven_transport = Some(token),
                    transports::Disconnect::Published(lease, transport) => {
                        snapshot_transport(lease).unwrap_or_else(|_| fault());
                        c.lease = Some(lease);
                        c.transport = Some(transport);
                        c.native_transport = true;
                    }
                }
                // Synthetic callers may exercise the public manual API. The
                // production transport close never terminally closes Session.
                #[cfg(test)]
                if manual {
                    let lease = state().session(raw.gpr[0]);
                    if let Some(lease) = lease {
                        // Unbound and alternate transports have no product authority.
                        if state().world(lease).is_ok() {
                            snapshot_transport(lease).unwrap_or_else(|_| fault());
                            c.transport = state().close(lease).unwrap_or_else(|_| fault());
                            c.lease = Some(lease);
                        }
                    }
                }
            }
            75 if caller(raw, 0x8573c9) => {
                if let Some((lease, transport)) = CALLS.with(|s| {
                    s.borrow()
                        .iter()
                        .rev()
                        .find(|s| s.kind == 74)
                        .and_then(|s| s.lease.zip(s.transport))
                }) {
                    if raw.gpr[0] != transport.thread || raw.gpr[1] as u32 != u32::MAX {
                        fault();
                    }
                    snapshot_transport(lease).unwrap_or_else(|_| fault());
                    c.lease = Some(lease);
                    c.transport = Some(transport);
                    c.thread = raw.gpr[0];
                    c.native_transport = CALLS.with(|s| {
                        s.borrow()
                            .iter()
                            .rev()
                            .find(|c| c.kind == 74)
                            .is_some_and(|c| c.native_transport)
                    });
                } else if let Some(token) = CALLS.with(|s| {
                    s.borrow()
                        .iter()
                        .rev()
                        .find(|call| call.kind == 74)
                        .and_then(|call| call.unproven_transport)
                }) {
                    // This is a safety guard only. Even AL1 does not grant a
                    // join/World certificate for an unproven transport.
                    c.unproven_join =
                        raw.gpr[0] == token.transport.thread && raw.gpr[1] as u32 == u32::MAX;
                } else if transports::reset_join_candidate(raw.gpr[0]) {
                    c.unproven_join = raw.gpr[1] as u32 == u32::MAX;
                    if !c.unproven_join {
                        fault();
                    }
                } else if registry::unproven_join_candidate(raw.gpr[0]) {
                    c.unproven_join = raw.gpr[1] as u32 == u32::MAX;
                } else if let Some((lease, transport)) = registry::join_candidate(raw.gpr[0]) {
                    if raw.gpr[1] as u32 != u32::MAX || raw.gpr[0] != transport.thread {
                        fault();
                    }
                    snapshot_transport(lease).unwrap_or_else(|_| fault());
                    c.lease = Some(lease);
                    c.transport = Some(transport);
                    c.thread = raw.gpr[0];
                    c.registry = 1;
                }
            }
            71 | 75 => {}
            _ => fault(),
        }
        if c.kind == 72 {
            census::event(if c.message.is_some() {
                census::SemanticEvent::AdmissionQualified
            } else {
                census::SemanticEvent::AdmissionUnqualified
            });
        }
        if matches!(c.kind, 72 | 73) && c.message.is_none() {
            if let Some(frames) = FRAMES.get() {
                c.unproven_hold = Some(
                    frames
                        .enter_with_provenance(true)
                        .unwrap_or_else(|_| fault()),
                );
                if !frames.healthy() {
                    fault();
                }
            }
        }
        raw.invocation.cookie = COOKIE;
        raw.invocation.token = c.serial;
        CALLS.with(|s| s.borrow_mut().push(c));
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
    let c = CALLS
        .with(|s| s.borrow_mut().pop())
        .unwrap_or_else(|| fault());
    if c.serial != raw.invocation.token || c.kind != raw.invocation.kind {
        fault();
    }
    if let Some(token) = c.unproven_transport {
        transports::unproven_disconnect_end(token, exceptional).unwrap_or_else(|_| fault());
    }
    if c.unproven_join && !exceptional && raw.gpr[4] as u8 != 1 {
        // Known AL0 must still stop before the destructive continuation.
        fault();
    }
    if exceptional {
        if c.lease.is_some() {
            unresolved(c.lease);
        }
        if c.kind == 74 && c.native_transport {
            let _ = transports::disconnect_end(c.lease.unwrap(), c.transport.unwrap(), true);
        }
        if c.kind == 77 && c.thread != 0 {
            let _ = registry::destroy_end(c.thread, true);
        }
    } else if c.kind == 76 {
        if raw.gpr[4] != c.registry
            || read64(c.registry).ok() != Some(EXE.load(Ordering::Acquire) + 0x1040200)
        {
            fault();
        }
        let mut regs = REGISTRIES.lock().unwrap_or_else(|_| fault());
        if regs.insert(c.registry, c.serial).is_some() {
            fault();
        }
    } else if c.kind == 77 && c.thread != 0 {
        registry::destroy_end(c.thread, false).unwrap_or_else(|_| fault());
        if REGISTRIES
            .lock()
            .unwrap_or_else(|_| fault())
            .remove(&c.registry)
            != Some(c.owner)
        {
            fault();
        }
    } else if let Some(lease) = c.lease {
        let result = match c.kind {
            70 => {
                let result = state().published(lease, raw.gpr[4] as u32);
                let publication = {
                    let state = state();
                    state
                        .snapshot(lease)
                        .is_ok()
                        .then(|| state.world(lease).is_ok())
                };
                if let Some(bound) = publication.filter(|_| result.is_ok()) {
                    census::event(if bound {
                        census::SemanticEvent::PublishedWorldBound
                    } else {
                        census::SemanticEvent::PublishedWorldUnbound
                    });
                }
                result
            }
            71 => {
                let session = raw.gpr[4];
                let callback = if session != 0 {
                    // Callback construction has already returned, but neither
                    // outer owner+8 nor registry node+8 has been published.
                    let callback = read64(session + 0x30).unwrap_or_else(|_| fault());
                    if callback != 0
                        && (read64(callback).ok() != Some(EXE.load(Ordering::Acquire) + 0x10494a8)
                            || read32(callback + 8).ok() != Some(c.id))
                    {
                        fault();
                    }
                    callback
                } else {
                    0
                };
                state().factory(lease, c.id, c.owner, session, callback)
            }
            72 => state().dispatched(c.message.unwrap_or_else(|| fault())),
            73 => state().admission_returned(lease),
            74 if c.native_transport => {
                transports::disconnect_end(lease, c.transport.unwrap_or_else(|| fault()), false)
            }
            74 => state().closed(lease),
            75 if c.registry == 1 => registry::join_returned(
                lease,
                c.transport.unwrap_or_else(|| fault()),
                c.thread,
                raw.gpr[4] as u8,
            ),
            75 if c.native_transport => transports::join_returned(
                lease,
                c.transport.unwrap_or_else(|| fault()),
                c.thread,
                raw.gpr[4] as u8,
            ),
            75 => state().join(
                lease,
                c.transport.unwrap_or_else(|| fault()),
                c.thread,
                raw.gpr[4] as u8,
            ),
            _ => Err(Error::Stale),
        };
        // In particular AL0 must not reach8573C9's unconditional destructor.
        if result.is_err() {
            unresolved(Some(lease));
            fault();
        }
    }
    if let Some(hold) = c.hold {
        if (SCOPE.get().unwrap_or_else(|| fault()).leave)(hold, exceptional).is_err() {
            fault();
        }
    }
    if let Some(hold) = c.unproven_hold {
        let frames = FRAMES.get().unwrap_or_else(|| fault());
        if exceptional {
            frames.mark_unresolved();
        }
        frames.leave(hold).unwrap_or_else(|_| fault());
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
        unresolved(None);
    }
    unsafe { SetLastError(e) };
    1
}

macro_rules! original {
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
            "_body:\n",
            "mov [rsp+",
            $scratch,
            "],rax\nmov rax,[rip+NATIVE_SESSION_CONTINUE+",
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
// Registration's home+8 input is already copied by its gateway; do not use it
// as scratch. The original save goes to+10 and is restored by the real body.
original!(
    "native_session_register_original",
    "0",
    "0x10",
    "0x18",
    "0x20"
);
original!("native_session_factory_original", "8", "8", "0x10", "0x18");
original!(
    "native_session_incoming_original",
    "16",
    "0x20",
    "8",
    "0x10"
);
original!("native_session_close_original", "32", "8", "0x10", "0x18");
original!("native_session_join_original", "40", "0x10", "8", "0x18");
original!("native_registry_birth_original", "48", "8", "0x10", "0x18");
original!(
    "native_registry_destroy_original",
    "56",
    "8",
    "0x10",
    "0x18"
);
global_asm!(
    ".globl native_session_admission_original",
    ".seh_proc native_session_admission_original",
    "native_session_admission_original:",
    "push rbp",
    ".seh_pushreg rbp",
    "push rbx",
    ".seh_pushreg rbx",
    "push rsi",
    ".seh_pushreg rsi",
    "push rdi",
    ".seh_pushreg rdi",
    ".seh_endprologue",
    ".globl native_session_admission_original_body",
    "native_session_admission_original_body:",
    // Four pushes: scratch at original home+8/+10, never savedRBP/return.
    "mov [rsp+0x28],rax",
    "mov rax,[rip+NATIVE_SESSION_CONTINUE+24]",
    "mov [rsp+0x30],rax",
    "mov rax,[rsp+0x28]",
    "jmp qword ptr [rsp+0x30]",
    ".seh_endproc",
);
unsafe extern "system" {
    fn native_session_register_original();
    fn native_session_factory_original();
    fn native_session_incoming_original();
    fn native_session_admission_original();
    fn native_session_close_original();
    fn native_session_join_original();
    fn native_registry_birth_original();
    fn native_registry_destroy_original();
    fn native_registry_birth_gateway();
    fn native_registry_destroy_gateway();
    fn native_session_register_gateway();
    fn native_session_factory_gateway();
    fn native_session_incoming_gateway();
    fn native_session_admission_gateway();
    fn native_session_close_gateway();
    fn native_session_join_gateway();
}
// Registration consumes entry home+8; incoming consumes arg5 at entry+28.
// Literal MOVs occur after GPR restore, followed by originalRAX restoration.
gateway!(
    "native_session_register_gateway",
    "70",
    native_session_register_original,
    begin,
    end,
    unwind,
    "mov rax,[rsp+0x1d0]\nmov [rsp],rax\nmov rax,[rsp+0x40]\n"
);
gateway!(
    "native_session_factory_gateway",
    "71",
    native_session_factory_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_session_incoming_gateway",
    "72",
    native_session_incoming_original,
    begin,
    end,
    unwind,
    "mov rax,[rsp+0x1f0]\nmov [rsp+0x20],rax\nmov rax,[rsp+0x40]\n"
);
gateway!(
    "native_session_admission_gateway",
    "73",
    native_session_admission_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_session_close_gateway",
    "74",
    native_session_close_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_session_join_gateway",
    "75",
    native_session_join_original,
    begin,
    end,
    unwind
);

gateway!(
    "native_registry_birth_gateway",
    "76",
    native_registry_birth_original,
    begin,
    end,
    unwind
);
gateway!(
    "native_registry_destroy_gateway",
    "77",
    native_registry_destroy_original,
    begin,
    end,
    unwind
);
const SITES: [(usize, [u8; 8], usize); 8] = [
    (
        0x7f8cd0,
        [0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x74],
        5,
    ),
    (0x7f8ab0, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x74], 5),
    (
        0x860000,
        [0x48, 0x89, 0x5c, 0x24, 0x20, 0x57, 0x41, 0x56],
        5,
    ),
    (
        0x842030,
        [0x40, 0x55, 0x53, 0x56, 0x57, 0x41, 0x54, 0x41],
        5,
    ),
    (0x845d30, [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83], 5),
    (
        0x98c650,
        [0x48, 0x89, 0x5c, 0x24, 0x10, 0x56, 0x48, 0x83],
        5,
    ),
    (0x7f80e0, [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83], 5),
    (0x7f8a70, [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83], 5),
];
fn gateways() -> [usize; 8] {
    [
        native_session_register_gateway as *const () as usize,
        native_session_factory_gateway as *const () as usize,
        native_session_incoming_gateway as *const () as usize,
        native_session_admission_gateway as *const () as usize,
        native_session_close_gateway as *const () as usize,
        native_session_join_gateway as *const () as usize,
        native_registry_birth_gateway as *const () as usize,
        native_registry_destroy_gateway as *const () as usize,
    ]
}
struct Fragment {
    begin: u32,
    end: u32,
    unwind: u32,
    hex: &'static str,
}
struct FixedFrame {
    begin: u32,
    end: u32,
    sha: &'static str,
    parts: &'static [Fragment],
    #[cfg(test)]
    specimen: &'static str,
}
fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
unsafe fn verify_loaded(exe: usize) -> Result<(), &'static str> {
    for f in fixed::FRAMES {
        let len = (f.end - f.begin) as usize;
        if !committed_range(exe + f.begin as usize, len, 1, false) {
            return Err("session code range");
        }
        let b = unsafe { std::slice::from_raw_parts((exe + f.begin as usize) as *const u8, len) };
        if format!("{:x}", Sha256::digest(b)) != f.sha {
            return Err("session complete body differs");
        }
        for p in f.parts {
            let mut base = 0;
            let row = unsafe {
                windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry(
                    exe as u64 + u64::from(p.begin),
                    &mut base,
                    null_mut(),
                )
            };
            let expected = [
                p.begin.to_le_bytes(),
                p.end.to_le_bytes(),
                p.unwind.to_le_bytes(),
            ]
            .concat();
            if base != exe as u64
                || copy_current::<12>(row as u64)
                    .as_ref()
                    .map(|b| b.as_slice())
                    != Some(expected.as_slice())
            {
                return Err("session complete pdata differs");
            }
            for (i, b) in unhex(p.hex).iter().enumerate() {
                if copy_current::<1>(exe as u64 + u64::from(p.unwind) + i as u64) != Some([*b]) {
                    return Err("session unwind differs");
                }
            }
        }
    }
    Ok(())
}
unsafe fn install_at(exe: usize) -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "session hook owner poisoned")?;
    if !hooks.is_empty() {
        return Err("session already installed");
    }
    // Validate every entry before publishing any branch.
    for (rva, expected, _) in SITES {
        if copy_current::<8>((exe + rva) as u64) != Some(expected) || !(exe + rva).is_multiple_of(8)
        {
            return Err("session entry differs");
        }
    }
    EXE.store(exe as u64, Ordering::Release);
    for (i, (rva, expected, stolen)) in SITES.into_iter().enumerate() {
        let p = (exe + rva) as *mut u8;
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(p.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
            || info.State != MEM_COMMIT
            || info.Protect & 0xff != PAGE_EXECUTE_READ
            || info.Protect & PAGE_GUARD != 0
        {
            return Err("session entry protection");
        }
        let relay = unsafe { executable_near(p as usize, p as usize, stolen) }
            .ok_or("session relay unavailable")?;
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
            return Err("session relay publication");
        }
        let (patched, prefix) = native_entry::plan(p, expected, relay)?;
        NATIVE_SESSION_CONTINUE[i].store((exe + rva + stolen) as u64, Ordering::Release);
        hooks.push(OwnedHook {
            hook: Hook {
                target: p,
                original: expected,
                patched,
                prefix,
                _code: relay,
            },
            protection: info.Protect,
        });
        unpublished.0 = null_mut();
        let r = unsafe {
            native_entry::publish(&hooks.last().ok_or("native hook owner missing")?.hook)
        }?;
        if !r.exchanged
            || !r.protection_restored
            || !r.cache_flushed
            || copy_current::<8>(p as u64) != Some(patched)
        {
            return Err("critical: session publication unverified");
        }
    }
    Ok(())
}
unsafe fn restore() -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "session hook owner poisoned")?;
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
        Err("critical: session restoration unverified")
    }
}
#[cfg(test)]
#[path = "native_sessions_tests.rs"]
mod tests;
