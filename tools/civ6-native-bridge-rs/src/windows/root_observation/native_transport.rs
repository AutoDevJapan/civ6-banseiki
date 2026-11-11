//! Native transport/network/thread birth carriers. No current-World mint.
//! Only the local1049808 network and exact native call stack are published.
use super::*;
use std::cell::RefCell;
#[path = "native_transport_fixed.rs"]
mod fixed;
#[path = "native_transport_ledger.rs"]
mod ledger;
use ledger::{Birth, Ledger};
#[derive(Clone, Copy)]
pub(super) struct UnprovenClose {
    birth: Option<Birth>,
    pub session: u64,
    pub lease: SessionLease,
    pub transport: SessionTransport,
}
pub(super) enum Disconnect {
    Untracked,
    Published(SessionLease, SessionTransport),
    Unproven(UnprovenClose),
}
static EXE: AtomicU64 = AtomicU64::new(0);
static NEXT: AtomicU64 = AtomicU64::new(0);
static STATE: std::sync::LazyLock<Mutex<Ledger>> =
    std::sync::LazyLock::new(|| Mutex::new(Ledger::default()));
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
pub(super) fn hook_entries_owned() -> bool {
    owns_hook_entries(&HOOKS, SITES.len())
}

#[unsafe(no_mangle)]
static NATIVE_TRANSPORT_CONTINUE: [AtomicU64; 9] = [const { AtomicU64::new(0) }; 9];
struct Call {
    serial: u64,
    kind: u64,
    birth: Option<Birth>,
    reset: Option<Birth>,
    pointer: u64,
    callback: u64,
    receive: Option<ReceiveScope>,
}
struct ReceiveScope {
    hold: super::ledger::ReceiveHold,
    source: Option<(SessionLease, SessionTransport)>,
}
thread_local! {static CALLS:RefCell<Vec<Call>>=const {RefCell::new(Vec::new())};}
fn state() -> std::sync::MutexGuard<'static, Ledger> {
    STATE.lock().unwrap_or_else(|_| fault())
}
#[track_caller]
fn fault() -> ! {
    census::fault(
        census::module(module_path!()),
        std::panic::Location::caller().line(),
    );
    diagnostic(100);
    if let Ok(mut s) = STATE.try_lock() {
        s.unresolved(None)
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
fn value(p: u64) -> u64 {
    read64(p).unwrap_or_else(|_| fault())
}
fn caller(raw: &Saved, rva: u64) -> bool {
    #[cfg(test)]
    if let Some(address) = tests::caller_override(rva) {
        return raw.invocation.caller == address;
    }
    raw.invocation.caller == exe(rva)
}
fn parent(kind: u64) -> Option<Birth> {
    CALLS.with(|s| {
        s.borrow()
            .iter()
            .rev()
            .find(|c| c.kind == kind)
            .and_then(|c| c.birth)
    })
}
pub(super) fn reset_join_candidate(thread: u64) -> bool {
    let id = CALLS.with(|calls| {
        calls
            .borrow()
            .iter()
            .rev()
            .find(|call| call.kind == 103)
            .and_then(|call| call.reset)
    });
    id.is_some_and(|id| {
        state()
            .snapshot(id)
            .is_ok_and(|(_, _, _, transport)| transport.thread != 0 && transport.thread == thread)
    })
}
#[cfg(test)]
pub(super) fn fixture_reset_join_caller() -> Option<u64> {
    tests::reset_join_caller()
}
fn complete_reset(id: Birth) -> Result<Birth, SessionError> {
    let (lease, _, _, old) = state().snapshot(id)?;
    if read64(old.pointer + 0x10)? != 0 {
        return Err(Error::Stale);
    }
    state().reset_ready(id)?;
    super::native_transport_reset_returned(lease, old)?;
    state().reset_returned(id)
}
/// Only the live original861550 invocation supplies this receiver carrier.
pub(in super::super) fn current_receiver() -> Option<(u64, u64)> {
    CALLS.with(|s| {
        s.borrow()
            .iter()
            .rev()
            .find(|c| c.kind == 108)
            .map(|c| (c.pointer, c.serial))
    })
}
fn check_published(id: Birth) -> Result<(SessionLease, SessionTransport), SessionError> {
    let (lease, session, callback, t) = state().snapshot(id)?;
    if read64(session + 0x28)? != t.pointer
        || read64(session + 0x30)? != callback
        || read64(t.pointer + 8)? != callback
        || read64(t.pointer + 0x10)? != t.network
    {
        return Err(Error::Stale);
    }
    Ok((lease, t))
}
fn enter_receive(network: u64) -> Result<(Option<Birth>, ReceiveScope), SessionError> {
    // Session retirement takes this same lock. Retain the observation before
    // allowing its completion check to pass, including the lookup interval.
    let mut sessions = super::state();
    let mut transports = state();
    let birth = if copy_current::<8>(network).map(u64::from_le_bytes) == Some(exe(0x1049808)) {
        transports.enter(network)?
    } else {
        None
    };
    let source = birth
        .map(|birth| {
            transports
                .snapshot(birth)
                .map(|(lease, _, _, transport)| (lease, transport))
        })
        .transpose()?;
    let hold = sessions.retain_receive(source)?;
    Ok((birth, ReceiveScope { hold, source }))
}
fn leave_receive(
    receive: ReceiveScope,
    birth: Option<Birth>,
    exceptional: bool,
) -> Result<(), SessionError> {
    // Keep the obligation visible through the transport's own return update.
    // Entry, return and retirement all serialize on the Session ledger first.
    let mut sessions = super::state();
    let mut transports = state();
    if exceptional {
        if let Some(birth) = birth {
            transports.unresolved(Some(birth));
            sessions.unresolved(None);
        } else {
            sessions.unobserved_receive_unwind();
        }
    } else if let Some(birth) = birth {
        transports.leave(birth)?;
    }
    sessions.release_receive(receive.hold, receive.source)
}
/// A captured native incarnation; no World or packet is issued by this lookup.
pub(in super::super) fn from_network(
    network: u64,
) -> Result<Option<(SessionLease, SessionTransport)>, SessionError> {
    let known = state().published_network(network)?;
    let Some((lease, t)) = known else {
        return Ok(None);
    };
    match copy_current::<1>(t.pointer + 0x45) {
        Some([1]) => {}
        Some([0]) => {
            census::event(census::SemanticEvent::ReceiveInactive);
            return Ok(None);
        }
        _ => {
            // Missing/foreign flag values are neither an active producer nor
            // an observed inactive one. Preserve native polling, issue no
            // input relationship and keep this distinct in diagnostics.
            diagnostic(2071);
            return Ok(None);
        }
    }
    let (_, session, callback, _) = state().snapshot(Birth(t.incarnation))?;
    if super::existing_transport_owner(session, callback)? != Some(lease)
        || read64(session + 0x28)? != t.pointer
        || read64(t.pointer + 8)? != callback
        || read64(t.pointer + 0x10)? != network
        || read64(network + 0xc0)? != t.thread
        || read64(network)? != exe(0x1049808)
    {
        return Err(Error::Stale);
    }
    Ok(Some((lease, t)))
}
extern "system" fn begin(raw: *mut Saved) {
    let e = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        let mut c = Call {
            serial: serial(),
            kind: raw.invocation.kind,
            birth: None,
            reset: None,
            pointer: raw.gpr[0],
            callback: 0,
            receive: None,
        };
        match c.kind {
            100..=102
                if [0x83905e, 0x83ef44, 0x83fc63]
                    .into_iter()
                    .any(|pc| caller(raw, pc)) =>
            {
                let table = value(c.pointer);
                let expected = [0x81b330, 0x81c680, 0x83ade0][(c.kind - 100) as usize];
                if ![exe(0x1043088), exe(0x10436e0), exe(0x1045bd8)].contains(&table)
                    || value(table + 0x408) != exe(expected)
                {
                    fault()
                }
                c.callback = value(c.pointer + 0x30);
                let capture_allowed = state().capture_allowed(c.pointer);
                if capture_allowed {
                    if let Some(lease) = super::existing_transport_owner(c.pointer, c.callback)
                        .unwrap_or_else(|_| fault())
                    {
                        c.birth = Some(
                            state()
                                .factory(lease, c.pointer, c.callback)
                                .unwrap_or_else(|_| fault()),
                        );
                    }
                }
            }
            103 => {
                let known = state().pointer_birth(c.pointer);
                if let Some(id) = known {
                    let (lease, session, callback, t) =
                        state().snapshot(id).unwrap_or_else(|_| fault());
                    if value(c.pointer + 8) != callback
                        || value(session + 0x28) != c.pointer
                        || value(value(c.pointer) + 0x58) != exe(0x84ec00)
                    {
                        fault();
                    }
                    if t.network != 0 {
                        if value(c.pointer + 0x10) != t.network {
                            diagnostic(2075);
                        }
                        // The pinned original calls its virtual reset before
                        // constructing another network. Revoke admission now,
                        // but retire this graph only at the witnessed return.
                        super::native_transport_reset_begin(lease, t).unwrap_or_else(|_| fault());
                        state().reset_begin(id).unwrap_or_else(|_| fault());
                        c.reset = Some(id);
                    } else if value(c.pointer + 0x10) != 0 {
                        fault();
                    } else if read32(c.pointer + 0x40).unwrap_or_else(|_| fault()) == 1 {
                        c.birth = Some(id)
                    }
                }
            }
            104 if caller(raw, 0x84e539) => {
                let reset = CALLS.with(|calls| {
                    calls
                        .borrow()
                        .iter()
                        .rev()
                        .find(|call| call.kind == 103)
                        .and_then(|call| call.reset)
                });
                if let Some(old) = reset {
                    let fresh = complete_reset(old).unwrap_or_else(|_| fault());
                    CALLS.with(|calls| {
                        let mut calls = calls.borrow_mut();
                        let parent = calls
                            .iter_mut()
                            .rev()
                            .find(|call| call.kind == 103 && call.reset == Some(old))
                            .unwrap_or_else(|| fault());
                        parent.birth = Some(fresh);
                        parent.reset = None;
                    });
                }
                c.birth = parent(103);
            }
            105 if caller(raw, 0x84e551) => {
                c.birth = parent(103);
                if let Some(id) = c.birth {
                    let (_, t) = check_published(id).unwrap_or_else(|_| fault());
                    if t.network != c.pointer || value(c.pointer) != exe(0x1049808) {
                        fault()
                    }
                }
            }
            106 if caller(raw, 0x85739b) => {
                c.birth = parent(105);
                if let Some(id) = c.birth {
                    let (_, _, _, t) = state().snapshot(id).unwrap_or_else(|_| fault());
                    if value(c.pointer) != t.network
                        || value(c.pointer + 8) != exe(0x8570b0)
                        || raw.gpr[1] != 0
                        || raw.metadata != 0
                    {
                        fault()
                    }
                }
            }
            107 if caller(raw, 0x98c0d1) => {
                c.birth = parent(106);
            }
            108 => {
                let (birth, receive) = enter_receive(c.pointer).unwrap_or_else(|_| fault());
                c.birth = birth;
                c.receive = Some(receive);
            }
            100..=102 | 104..=107 => {}
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
    let receiver = current_receiver();
    let c = CALLS
        .with(|s| s.borrow_mut().pop())
        .unwrap_or_else(|| fault());
    if c.serial != raw.invocation.token || c.kind != raw.invocation.kind {
        fault()
    }
    if c.kind == 108 && receiver != Some((c.pointer, c.serial)) {
        fault()
    }
    if c.kind == 108 {
        super::input::receiver_returned(c.pointer, c.serial, exceptional);
        leave_receive(c.receive.unwrap_or_else(|| fault()), c.birth, exceptional)
            .unwrap_or_else(|_| fault());
        if exceptional && c.birth.is_some() {
            super::unresolved(None);
        }
        return;
    }
    if c.kind == 103 && !exceptional {
        if let Some(old) = c.reset {
            complete_reset(old).unwrap_or_else(|_| fault());
        }
    }
    let Some(id) = c.birth else { return };
    if exceptional {
        state().unresolved(Some(id));
        super::unresolved(None);
        return;
    }
    let result = match c.kind {
        100..=102 => {
            let pointer = raw.gpr[4];
            if pointer != 0 {
                let expected = [0x10482c8, 0x1044478, 0x1047e60][(c.kind - 100) as usize];
                if value(pointer) != exe(expected)
                    || value(pointer + 8) != c.callback
                    || value(c.pointer + 0x28) != 0
                {
                    fault()
                }
            }
            state().transport(id, pointer, c.callback)
        }
        104 => {
            let pointer = raw.gpr[4];
            if pointer != c.pointer || pointer == 0 || value(pointer) != exe(0x1049808) {
                fault()
            }
            let (_, _, _, t) = state().snapshot(id).unwrap_or_else(|_| fault());
            if value(t.pointer + 0x10) != 0 {
                fault()
            }
            state().network(id, pointer)
        }
        107 => {
            let pointer = raw.gpr[4];
            if pointer != c.pointer
                || pointer == 0
                || value(pointer) != exe(0x106d030)
                || value(exe(0x106d038)) != exe(0x98c170)
            {
                fault()
            }
            let (_, _, _, t) = state().snapshot(id).unwrap_or_else(|_| fault());
            if value(t.network + 0xc0) != 0 {
                fault()
            }
            state().thread(id, pointer)
        }
        106 => {
            let pointer = raw.gpr[4];
            if pointer == 0 {
                state().start_failed(id)
            } else {
                let (lease, _, _, t) = state().snapshot(id).unwrap_or_else(|_| fault());
                if pointer != t.thread
                    || value(t.network + 0xc0) != 0
                    || value(pointer + 8) != t.network
                    || value(pointer + 0x10) != exe(0x8570b0)
                    || value(pointer + 0xb0) == 0
                {
                    fault()
                }
                let t = state().publish(id, pointer).unwrap_or_else(|_| fault());
                super::publish_transport(lease, t.incarnation, t.pointer, t.network, t.thread).map(
                    |actual| {
                        if actual != t {
                            fault()
                        }
                    },
                )
            }
        }
        105 => {
            let (_, _, _, t) = state().snapshot(id).unwrap_or_else(|_| fault());
            if t.thread != 0 && value(t.network + 0xc0) != t.thread {
                fault()
            };
            Ok(())
        }
        103 => Ok(()),
        _ => Err(Error::Stale),
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
/// Existing845D30 full-frame owner calls this before touching Session+28.
pub(super) fn terminal_closing(
    lease: SessionLease,
    t: SessionTransport,
) -> Result<(), SessionError> {
    let (saved_lease, session, _, actual) = state().snapshot(Birth(t.incarnation))?;
    if saved_lease != lease || actual != t || check_published(Birth(t.incarnation))? != (lease, t) {
        return Err(Error::Stale);
    }
    if state().close(session)? != Some((lease, t)) {
        return Err(Error::Stale);
    }
    Ok(())
}
pub(super) fn disconnect_begin(session: u64) -> Result<Disconnect, SessionError> {
    let pointer = read64(session + 0x28)?;
    let known = state().pointer_birth(pointer);
    let Some(id) = known else {
        if pointer != 0 {
            census::event(census::SemanticEvent::CloseUnknown);
        }
        return Ok(Disconnect::Untracked);
    };
    let (saved_lease, _, _, expected) = state().snapshot(id)?;
    let published = state().published(id)?;
    let inactive = copy_current::<1>(expected.pointer + 0x45) != Some([1]);
    if !published || inactive {
        if !published {
            census::event(census::SemanticEvent::CloseUnpublished);
        }
        super::state().unproven_transport_close_begin(saved_lease, expected)?;
        state().unproven_begin(id)?;
        diagnostic(2070);
        return Ok(Disconnect::Unproven(UnprovenClose {
            birth: Some(id),
            session,
            lease: saved_lease,
            transport: expected,
        }));
    }
    if from_network(expected.network)? != Some((saved_lease, expected)) {
        return Err(Error::Stale);
    }
    let Some((lease, t)) = state().close(session)? else {
        return Ok(Disconnect::Untracked);
    };
    if super::transport_disconnect_begin(lease)? != Some(t) {
        return Err(Error::Stale);
    }
    Ok(Disconnect::Published(lease, t))
}
pub(super) fn unproven_disconnect_end(
    token: UnprovenClose,
    exceptional: bool,
) -> Result<(), SessionError> {
    super::state().unproven_transport_close_end(token.lease, token.transport, exceptional)?;
    if let Some(birth) = token.birth {
        state().unproven_end(birth, token.lease, token.transport, exceptional)
    } else {
        state().unproven_session_end(token.session, exceptional)
    }
}
pub(super) fn prepare_terminal_unproven(
    lease: SessionLease,
    session: u64,
) -> Result<UnprovenClose, SessionError> {
    let pointer = read64(session + 0x28).unwrap_or(0);
    let marker = super::state().unproven_transport_marker(lease);
    let known = state().pointer_birth(pointer);
    let mut birth = None;
    let transport = if let Some(id) = known {
        let (owner, actual, _, transport) = state().snapshot(id)?;
        if owner != lease || actual != session {
            return Err(Error::Stale);
        }
        state().unproven_begin(id)?;
        birth = Some(id);
        transport
    } else {
        marker.unwrap_or(SessionTransport {
            incarnation: 0,
            pointer,
            network: 0,
            thread: 0,
        })
    };
    super::state().unproven_transport_close_begin(lease, transport)?;
    state().note_unproven();
    diagnostic(2070);
    Ok(UnprovenClose {
        birth,
        session,
        lease,
        transport,
    })
}
/// Existing98C650 owner supplies original AL under exact caller8573C9.
pub(super) fn join_returned(
    lease: SessionLease,
    t: SessionTransport,
    thread: u64,
    al: u8,
) -> Result<(), SessionError> {
    state().joined(lease, t, thread, al)?;
    super::transport_join_result(lease, t, thread, al)
}
pub(super) fn disconnect_end(
    lease: SessionLease,
    t: SessionTransport,
    exceptional: bool,
) -> Result<(), SessionError> {
    if exceptional {
        state().unresolved(Some(Birth(t.incarnation)));
        return Err(Error::Unresolved);
    }
    super::transport_disconnect_end(lease)?;
    state().closed(lease, t)
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
            "],rax\nmov rax,[rip+NATIVE_TRANSPORT_CONTINUE+",
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
macro_rules! stack_original {
    ($name:literal,$slot:literal,$size:literal) => {
        global_asm!(concat!(
            ".globl ",
            $name,
            "\n.seh_proc ",
            $name,
            "\n",
            $name,
            ":\npush rbx\n.seh_pushreg rbx\nsub rsp,",
            $size,
            "\n.seh_stackalloc ",
            $size,
            "\n.seh_endprologue\n.globl ",
            $name,
            "_body\n",
            $name,
            "_body:\nmov [rsp+0x20],rax\nmov rax,[rip+NATIVE_TRANSPORT_CONTINUE+",
            $slot,
            "]\nmov [rsp+0x28],rax\nmov rax,[rsp+0x20]\njmp qword ptr [rsp+0x28]\n.seh_endproc\n"
        ));
    };
}
stack_original!("transport_factory_a_original", "0", "0x30");
stack_original!("transport_factory_b_original", "8", "0x30");
stack_original!("transport_factory_c_original", "16", "0x30");
home_original!("transport_local_original", "24", "8", "0x10", "0x18");
home_original!("transport_network_original", "32", "0x10", "8", "0x20");
stack_original!("transport_start_original", "40", "0x40");
home_original!("transport_creator_original", "48", "8", "0x10", "0x18");
home_original!("transport_thread_original", "56", "8", "0x10", "0x18");
home_original!("transport_receive_original", "64", "0x10", "8", "0x20");
unsafe extern "system" {
    fn transport_factory_a_original();
    fn transport_factory_b_original();
    fn transport_factory_c_original();
    fn transport_local_original();
    fn transport_network_original();
    fn transport_start_original();
    fn transport_creator_original();
    fn transport_thread_original();
    fn transport_receive_original();
    fn transport_factory_a_gateway();
    fn transport_factory_b_gateway();
    fn transport_factory_c_gateway();
    fn transport_local_gateway();
    fn transport_network_gateway();
    fn transport_start_gateway();
    fn transport_creator_gateway();
    fn transport_thread_gateway();
    fn transport_receive_gateway();
}
gateway!(
    "transport_factory_a_gateway",
    "100",
    transport_factory_a_original,
    begin,
    end,
    unwind
);
gateway!(
    "transport_factory_b_gateway",
    "101",
    transport_factory_b_original,
    begin,
    end,
    unwind
);
gateway!(
    "transport_factory_c_gateway",
    "102",
    transport_factory_c_original,
    begin,
    end,
    unwind
);
gateway!(
    "transport_local_gateway",
    "103",
    transport_local_original,
    begin,
    end,
    unwind
);
gateway!(
    "transport_network_gateway",
    "104",
    transport_network_original,
    begin,
    end,
    unwind
);
gateway!(
    "transport_start_gateway",
    "105",
    transport_start_original,
    begin,
    end,
    unwind
);
gateway!(
    "transport_creator_gateway",
    "106",
    transport_creator_original,
    begin,
    end,
    unwind,
    "mov rax,[rsp+0x1f0]\nmov [rsp+0x20],rax\nmov rax,[rsp+0x40]\n"
);
gateway!(
    "transport_thread_gateway",
    "107",
    transport_thread_original,
    begin,
    end,
    unwind
);
gateway!(
    "transport_receive_gateway",
    "108",
    transport_receive_original,
    begin,
    end,
    unwind
);
const SITES: [(usize, [u8; 8], usize); 9] = [
    (
        0x81b330,
        [0x40, 0x53, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8b],
        6,
    ),
    (
        0x81c680,
        [0x40, 0x53, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8b],
        6,
    ),
    (
        0x83ade0,
        [0x40, 0x53, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8b],
        6,
    ),
    (0x84e4e0, [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83], 5),
    (
        0x860b10,
        [0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x74],
        5,
    ),
    (
        0x857360,
        [0x40, 0x53, 0x48, 0x83, 0xec, 0x40, 0x48, 0x8d],
        6,
    ),
    (0x98c070, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x6c], 5),
    (0x98bef0, [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83], 5),
    (
        0x861550,
        [0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x6c],
        5,
    ),
];
fn gateways() -> [usize; 9] {
    [
        transport_factory_a_gateway as *const () as usize,
        transport_factory_b_gateway as *const () as usize,
        transport_factory_c_gateway as *const () as usize,
        transport_local_gateway as *const () as usize,
        transport_network_gateway as *const () as usize,
        transport_start_gateway as *const () as usize,
        transport_creator_gateway as *const () as usize,
        transport_thread_gateway as *const () as usize,
        transport_receive_gateway as *const () as usize,
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
    sites: &[(usize, [u8; 8], usize); 9],
) -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "transport hook owner")?;
    if !hooks.is_empty() {
        return Err("transport hooks already installed");
    }
    for &(rva, expected, _) in sites {
        if copy_current::<8>((module + rva) as u64) != Some(expected) {
            return Err("transport entry differs");
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
            return Err("transport entry protection");
        }
        let relay = unsafe { executable_near(target as usize, target as usize, stolen) }
            .ok_or("transport relay")?;
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
            return Err("transport relay publication");
        }
        let (patched, prefix) = native_entry::plan(target, expected, relay)?;
        NATIVE_TRANSPORT_CONTINUE[i].store((module + rva + stolen) as u64, Ordering::Release);
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
            return Err("critical: transport publication unverified");
        }
    }
    Ok(())
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "transport hook owner")?;
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
        Err("critical: transport restoration unverified")
    }
}

#[cfg(test)]
#[path = "native_transport_tests.rs"]
mod tests;
