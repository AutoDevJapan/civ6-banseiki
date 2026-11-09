//! Deferred serialization of an already-admitted message, not first receive.
//! Each slot retains an immutable packet receipt through move, free and pump.
use super::*;
#[path = "native_session_queue_fixed.rs"]
mod fixed_queue;
#[path = "native_session_queue_ledger.rs"]
mod slots;

static QUEUE_EXE: AtomicU64 = AtomicU64::new(0);
static QUEUE_NEXT: AtomicU64 = AtomicU64::new(0);
static QUEUE_HOOKS: Mutex<Vec<super::super::OwnedHook>> = Mutex::new(Vec::new());
static SLOTS: OnceLock<Mutex<slots::Ledger<Receipt>>> = OnceLock::new();
#[unsafe(no_mangle)]
static NATIVE_SESSION_QUEUE_CONTINUE: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];

struct Flight {
    id: u64,
    receipt: Receipt,
    hold: Option<Hold>,
    slot: u64,
    buffer: u64,
    decoded: u64,
}
struct QueueCall {
    serial: u64,
    kind: u64,
    id: u64,
    queue: u64,
    session: u64,
    source: u64,
    slot: u64,
    parent: u64,
    current: u64,
    hold: Option<Hold>,
    flights: Vec<Flight>,
    ids: Vec<u64>,
    addresses: Vec<u64>,
}
thread_local! {static QUEUE_CALLS:RefCell<Vec<QueueCall>>=const {RefCell::new(Vec::new())};}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Receipt {
    message: Option<Message>,
    packet: Option<PacketReceipt>,
}
enum Hold {
    Bound(u64),
    Unknown(u64),
}
fn enter(receipt: Receipt) -> Option<Hold> {
    if let Some(message) = receipt.message {
        return Some(Hold::Bound(
            enter_scope(message).unwrap_or_else(|_| fault()),
        ));
    }
    FRAMES.get().map(|frames| {
        let hold = frames
            .enter_with_provenance(true)
            .unwrap_or_else(|_| fault());
        if !frames.healthy() {
            fault();
        }
        Hold::Unknown(hold)
    })
}
fn leave(hold: Option<Hold>, exceptional: bool) {
    match hold {
        Some(Hold::Bound(id)) => {
            (SCOPE.get().unwrap_or_else(|| fault()).leave)(id, exceptional)
                .unwrap_or_else(|_| fault());
        }
        Some(Hold::Unknown(id)) => {
            let frames = FRAMES.get().unwrap_or_else(|| fault());
            if exceptional {
                frames.mark_unresolved();
            }
            frames.leave(id).unwrap_or_else(|_| fault());
        }
        None => {}
    }
}
fn slots() -> std::sync::MutexGuard<'static, slots::Ledger<Receipt>> {
    SLOTS
        .get_or_init(|| Mutex::new(slots::Ledger::new()))
        .lock()
        .unwrap_or_else(|_| std::process::abort())
}
fn poison() {
    if let Some(s) = SLOTS.get() {
        if let Ok(mut s) = s.try_lock() {
            s.unresolved();
        }
    }
    super::unresolved(None);
}
#[track_caller]
fn fault() -> ! {
    census::fault(
        census::module(module_path!()),
        std::panic::Location::caller().line(),
    );
    super::super::diagnostic(80);
    poison();
    std::process::abort()
}
fn serial() -> u64 {
    QUEUE_NEXT
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
        .unwrap_or_else(|_| fault())
        + 1
}
fn caller(raw: &Saved, rva: u64) -> bool {
    #[cfg(test)]
    if let Some(pc) = tests::caller_override(rva) {
        return raw.invocation.caller == pc;
    }
    raw.invocation.caller == QUEUE_EXE.load(Ordering::Acquire) + rva
}
fn admitted_receipt(queue: u64) -> Option<Receipt> {
    let receipt = super::CALLS.with(|s| {
        s.borrow()
            .iter()
            .rev()
            .find(|c| c.kind == 73)
            .map(|c| Receipt {
                message: c.message,
                packet: c.packet,
            })
    })?;
    let session = if let Some(message) = receipt.message {
        state().message_session(message).ok()?
    } else {
        state().snapshot(receipt.packet?.lease).ok()?.0
    };
    (session.checked_add(0xd8) == Some(queue)).then_some(receipt)
}
/// Some(None) is an exact deferred caller with an unrecorded slot. Do not
/// fall back to an unrelated outer incoming/current-World scope in that case.
pub(super) fn admission(raw: &Saved) -> Option<Option<(Lease, Message)>> {
    if !caller(raw, 0x83d888) {
        return None;
    }
    QUEUE_CALLS.with(|s| {
        let s = s.borrow();
        let pump = s.iter().rev().find(|c| c.kind == 82)?;
        if pump.session != raw.gpr[0] {
            fault();
        }
        let Some(f) = pump
            .flights
            .iter()
            .find(|f| f.id == pump.current && f.decoded == raw.gpr[1] && f.decoded != 0)
        else {
            return Some(None);
        };
        if !slots().storage_ready(f.id) {
            fault();
        }
        let Some(message) = f.receipt.message else {
            return Some(None);
        };
        let lease = state().message_lease(message).unwrap_or_else(|_| fault());
        Some(Some((lease, message)))
    })
}
pub(super) fn packet(raw: &Saved) -> Option<Option<PacketReceipt>> {
    if !caller(raw, 0x83d888) {
        return None;
    }
    QUEUE_CALLS.with(|calls| {
        let calls = calls.borrow();
        let pump = calls.iter().rev().find(|call| call.kind == 82)?;
        if pump.session != raw.gpr[0] {
            fault();
        }
        let flight = pump.flights.iter().find(|flight| {
            flight.id == pump.current && flight.decoded == raw.gpr[1] && flight.decoded != 0
        });
        if flight.is_some_and(|flight| !slots().storage_ready(flight.id)) {
            fault();
        }
        Some(flight.and_then(|flight| flight.receipt.packet))
    })
}
/// Local pending accounting only; not an upstream producer seal or certificate.
pub(super) fn pending() -> Result<usize, &'static str> {
    slots().pending().map_err(|_| "deferred queue unresolved")
}

// The common gateway never modifies RDI. Capture it before entering Rust,
// into this adapter's otherwise unused Invocation.entry_rbx field at+118h.
// Nothing is written into the original caller's home or stack-argument area.
global_asm!(
    ".globl native_session_queue_begin",".seh_proc native_session_queue_begin","native_session_queue_begin:",
    "sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "mov [rcx+0x118],rdi","call {begin}","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    begin=sym begin,
);
unsafe extern "system" {
    fn native_session_queue_begin(raw: *mut Saved);
}

extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        if raw.invocation.kind == 84 {
            super::input::free_begin(raw);
        }
        let mut c = QueueCall {
            serial: serial(),
            kind: raw.invocation.kind,
            id: 0,
            queue: 0,
            session: 0,
            source: 0,
            slot: 0,
            parent: 0,
            current: 0,
            hold: None,
            flights: Vec::new(),
            ids: Vec::new(),
            addresses: Vec::new(),
        };
        match c.kind {
            80 => {
                if !caller(raw, 0x842581) {
                    return;
                }
                c.queue = raw.gpr[0];
                c.source = raw.gpr[1];
                if let Some(parent) = admitted_receipt(c.queue) {
                    let message = parent
                        .message
                        .map(|message| state().fork_message(message).unwrap_or_else(|_| fault()));
                    c.id = slots()
                        .reserve(
                            c.queue,
                            Receipt {
                                message,
                                packet: parent.packet,
                            },
                        )
                        .unwrap_or_else(|_| fault());
                }
            }
            81 => {
                if !caller(raw, 0x8475b2) {
                    return;
                }
                let parent = QUEUE_CALLS.with(|s| {
                    s.borrow()
                        .iter()
                        .rev()
                        .find(|c| c.kind == 80)
                        .map(|c| (c.id, c.source))
                });
                let Some((id, source)) = parent else {
                    return;
                };
                if id != 0 && source != raw.gpr[1] {
                    fault();
                }
                c.id = id;
                c.slot = raw.gpr[0];
            }
            82 => {
                c.session = raw.gpr[0];
                c.queue = c.session.checked_add(0xd8).unwrap_or_else(|| fault());
            }
            83 => {
                if !caller(raw, 0x83d81e) {
                    return;
                }
                let pump = QUEUE_CALLS.with(|s| {
                    s.borrow()
                        .iter()
                        .rev()
                        .find(|c| c.kind == 82)
                        .map(|c| (c.serial, c.session, c.queue))
                });
                let Some((parent, session, queue)) = pump else {
                    return;
                };
                if raw.gpr[7] != session {
                    fault();
                }
                c.parent = parent;
                c.slot = raw.invocation.entry_rbx;
                if let Some((id, receipt)) =
                    slots().retain(queue, c.slot).unwrap_or_else(|_| fault())
                {
                    if let Some(message) = receipt.message {
                        state()
                            .dispatch_deferred(message, session)
                            .unwrap_or_else(|_| fault());
                    }
                    let hold = enter(receipt);
                    let buffer = read64(c.slot).unwrap_or_else(|_| fault());
                    c.id = id;
                    QUEUE_CALLS.with(|s| {
                        s.borrow_mut()
                            .iter_mut()
                            .rev()
                            .find(|c| c.serial == parent)
                            .unwrap_or_else(|| fault())
                            .flights
                            .push(Flight {
                                id,
                                receipt,
                                hold,
                                slot: c.slot,
                                buffer,
                                decoded: 0,
                            })
                    });
                }
                QUEUE_CALLS.with(|s| {
                    s.borrow_mut()
                        .iter_mut()
                        .rev()
                        .find(|c| c.serial == parent)
                        .unwrap_or_else(|| fault())
                        .current = c.id
                });
            }
            84 => {
                let address = raw.gpr[0].saturating_sub(0x18);
                if caller(raw, 0x83d855) {
                    let flight = QUEUE_CALLS.with(|s| {
                        s.borrow()
                            .iter()
                            .rev()
                            .find(|c| c.kind == 82)
                            .and_then(|c| c.flights.iter().find(|f| f.id == c.current))
                            .map(|f| (f.id, f.slot, f.buffer))
                    });
                    if let Some((id, slot, buffer)) = flight {
                        if slot != address || buffer != raw.gpr[1] {
                            fault();
                        }
                        slots().free_begin(id).unwrap_or_else(|_| fault());
                        c.id = id;
                    }
                } else if caller(raw, 0x8473e8) {
                    let id = QUEUE_CALLS.with(|s| {
                        s.borrow()
                            .iter()
                            .rev()
                            .find(|c| c.kind == 85)
                            .and_then(|c| {
                                slots()
                                    .clear_free(&c.ids, address)
                                    .unwrap_or_else(|_| fault())
                            })
                    });
                    if let Some(id) = id {
                        slots().free_begin(id).unwrap_or_else(|_| fault());
                        c.id = id;
                    }
                } else {
                    return;
                }
            }
            85 => {
                c.queue = raw.gpr[0];
                if let Some(message) = slots().queue_message(c.queue) {
                    c.hold = enter(message);
                }
                c.ids = slots().clear_begin(c.queue).unwrap_or_else(|_| fault());
            }
            86 => {
                c.queue = raw.gpr[0];
                c.addresses = slots().queued_addresses(c.queue);
                if let Some(message) = slots().queue_message(c.queue) {
                    c.hold = enter(message);
                }
            }
            _ => fault(),
        }
        raw.invocation.cookie = COOKIE;
        raw.invocation.token = c.serial;
        QUEUE_CALLS.with(|s| s.borrow_mut().push(c));
    }))
    .is_err()
    {
        fault();
    }
    unsafe { SetLastError(error) };
}
fn verify_growth(queue: u64, addresses: &[u64]) -> Result<(), Error> {
    if addresses.is_empty() {
        return Ok(());
    }
    let map = read64(queue + 8)?;
    let cap = read64(queue + 0x10)?;
    if cap == 0 || cap > 1_048_576 || !cap.is_power_of_two() {
        return Err(Error::Unbound);
    }
    let mut pending: std::collections::HashSet<u64> = addresses.iter().copied().collect();
    for i in 0..cap {
        pending.remove(&read64(map + i * 8)?);
    }
    if pending.is_empty() {
        Ok(())
    } else {
        Err(Error::Stale)
    }
}
fn finish(raw: &mut Saved, exceptional: bool) {
    if exceptional {
        census::terminal(census::module(module_path!()), raw.invocation.kind);
    }
    if raw.invocation.kind == 84 {
        super::input::free_end(raw, exceptional);
    }
    if raw.invocation.cookie != COOKIE {
        return;
    }
    raw.invocation.cookie = 0;
    let c = QUEUE_CALLS
        .with(|s| s.borrow_mut().pop())
        .unwrap_or_else(|| fault());
    if (c.kind, c.serial) != (raw.invocation.kind, raw.invocation.token) {
        fault();
    }
    if exceptional {
        if c.id != 0 || !c.flights.is_empty() || !c.ids.is_empty() || c.hold.is_some() {
            poison();
        }
        for f in c.flights {
            leave(f.hold, true);
        }
        leave(c.hold, true);
        return;
    }
    match c.kind {
        80 if c.id != 0 => slots().published(c.id).unwrap_or_else(|_| fault()),
        81 if c.id != 0 => {
            if raw.gpr[4] != c.slot {
                fault();
            }
            slots().bind(c.id, c.slot).unwrap_or_else(|_| fault());
        }
        82 => {
            for f in c.flights {
                let message = slots().executed(f.id).unwrap_or_else(|_| fault());
                if let Some(message) = message.message {
                    state().dispatched(message).unwrap_or_else(|_| fault());
                }
                leave(f.hold, false);
            }
        }
        83 if c.id != 0 => {
            QUEUE_CALLS.with(|s| {
                let mut s = s.borrow_mut();
                let pump = s
                    .iter_mut()
                    .rev()
                    .find(|p| p.serial == c.parent)
                    .unwrap_or_else(|| fault());
                let f = pump
                    .flights
                    .last_mut()
                    .filter(|f| f.id == c.id)
                    .unwrap_or_else(|| fault());
                f.decoded = raw.gpr[4];
                // The pinned pump skips A51F90 only for a null vector pointer.
                if f.buffer == 0 {
                    slots().free_begin(f.id).unwrap_or_else(|_| fault());
                    slots().free_returned(f.id).unwrap_or_else(|_| fault());
                }
            });
        }
        84 if c.id != 0 => slots().free_returned(c.id).unwrap_or_else(|_| fault()),
        85 => {
            for message in slots().clear_returned(&c.ids).unwrap_or_else(|_| fault()) {
                if let Some(message) = message.message {
                    state()
                        .cancel_before_dispatch(message)
                        .unwrap_or_else(|_| fault());
                }
            }
            leave(c.hold, false);
        }
        86 => {
            verify_growth(c.queue, &c.addresses).unwrap_or_else(|_| fault());
            leave(c.hold, false);
        }
        80 | 81 | 83 | 84 => {}
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
    if !r.is_null()
        && unsafe { (*r).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0
        && std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            finish(unsafe { &mut *((frame + 0x20) as *mut Saved) }, true)
        }))
        .is_err()
    {
        poison();
    }
    unsafe { SetLastError(e) };
    1
}

macro_rules! original {
    ($name:literal,$slot:literal) => {
        global_asm!(concat!(
            ".globl ",
            $name,
            "\n.seh_proc ",
            $name,
            "\n",
            $name,
            ":\nmov [rsp+8],rbx\n.seh_savereg rbx,8\n.seh_endprologue\n.globl ",
            $name,
            "_body\n",
            $name,
            "_body:\n",
            "mov [rsp+0x10],rax\nmov rax,[rip+NATIVE_SESSION_QUEUE_CONTINUE+",
            $slot,
            "]\nmov [rsp+0x18],rax\nmov rax,[rsp+0x10]\njmp qword ptr [rsp+0x18]\n.seh_endproc\n"
        ));
    };
}
original!("native_session_queue_enqueue_original", "0");
original!("native_session_queue_move_original", "8");
original!("native_session_queue_clear_original", "40");
global_asm!(
    ".globl native_session_queue_pump_original",
    ".seh_proc native_session_queue_pump_original",
    "native_session_queue_pump_original:",
    "push rbp",
    ".seh_pushreg rbp",
    "push rbx",
    ".seh_pushreg rbx",
    "push rsi",
    ".seh_pushreg rsi",
    "push rdi",
    ".seh_pushreg rdi",
    ".seh_endprologue",
    ".globl native_session_queue_pump_original_body",
    "native_session_queue_pump_original_body:",
    "mov [rsp+0x28],rax",
    "mov rax,[rip+NATIVE_SESSION_QUEUE_CONTINUE+16]",
    "mov [rsp+0x30],rax",
    "mov rax,[rsp+0x28]",
    "jmp qword ptr [rsp+0x30]",
    ".seh_endproc",
    ".globl native_session_queue_decode_original",
    ".seh_proc native_session_queue_decode_original",
    "native_session_queue_decode_original:",
    "mov [rsp+0x18],r8d",
    ".seh_endprologue",
    ".globl native_session_queue_decode_original_body",
    "native_session_queue_decode_original_body:",
    "mov [rsp+8],rax",
    "mov rax,[rip+NATIVE_SESSION_QUEUE_CONTINUE+24]",
    "mov [rsp+0x20],rax",
    "mov rax,[rsp+8]",
    "jmp qword ptr [rsp+0x20]",
    ".seh_endproc",
    ".globl native_session_queue_free_original",
    ".seh_proc native_session_queue_free_original",
    "native_session_queue_free_original:",
    "mov rcx,rdx",
    ".seh_endprologue",
    ".globl native_session_queue_free_original_body",
    "native_session_queue_free_original_body:",
    "jmp qword ptr [rip+NATIVE_SESSION_QUEUE_CONTINUE+32]",
    ".seh_endproc",
    ".globl native_session_queue_growth_original",
    ".seh_proc native_session_queue_growth_original",
    "native_session_queue_growth_original:",
    "push rbx",
    ".seh_pushreg rbx",
    "push r14",
    ".seh_pushreg r14",
    "sub rsp,0x38",
    ".seh_stackalloc 0x38",
    ".seh_endprologue",
    ".globl native_session_queue_growth_original_body",
    "native_session_queue_growth_original_body:",
    "mov [rsp+0x50],rax",
    "mov rax,[rip+NATIVE_SESSION_QUEUE_CONTINUE+48]",
    "mov [rsp+0x58],rax",
    "mov rax,[rsp+0x50]",
    "jmp qword ptr [rsp+0x58]",
    ".seh_endproc",
);
unsafe extern "system" {
    fn native_session_queue_enqueue_original();
    fn native_session_queue_move_original();
    fn native_session_queue_pump_original();
    fn native_session_queue_decode_original();
    fn native_session_queue_free_original();
    fn native_session_queue_clear_original();
    fn native_session_queue_growth_original();
    fn native_session_queue_enqueue_gateway();
    fn native_session_queue_move_gateway();
    fn native_session_queue_pump_gateway();
    fn native_session_queue_decode_gateway();
    fn native_session_queue_free_gateway();
    fn native_session_queue_clear_gateway();
    fn native_session_queue_growth_gateway();
}
gateway!(
    "native_session_queue_enqueue_gateway",
    "80",
    native_session_queue_enqueue_original,
    native_session_queue_begin,
    end,
    unwind
);
gateway!(
    "native_session_queue_move_gateway",
    "81",
    native_session_queue_move_original,
    native_session_queue_begin,
    end,
    unwind
);
gateway!(
    "native_session_queue_pump_gateway",
    "82",
    native_session_queue_pump_original,
    native_session_queue_begin,
    end,
    unwind
);
gateway!(
    "native_session_queue_decode_gateway",
    "83",
    native_session_queue_decode_original,
    native_session_queue_begin,
    end,
    unwind
);
gateway!(
    "native_session_queue_free_gateway",
    "84",
    native_session_queue_free_original,
    native_session_queue_begin,
    end,
    unwind
);
gateway!(
    "native_session_queue_clear_gateway",
    "85",
    native_session_queue_clear_original,
    native_session_queue_begin,
    end,
    unwind
);
gateway!(
    "native_session_queue_growth_gateway",
    "86",
    native_session_queue_growth_original,
    native_session_queue_begin,
    end,
    unwind
);
fn gateways() -> [usize; 7] {
    [
        native_session_queue_enqueue_gateway as *const () as usize,
        native_session_queue_move_gateway as *const () as usize,
        native_session_queue_pump_gateway as *const () as usize,
        native_session_queue_decode_gateway as *const () as usize,
        native_session_queue_free_gateway as *const () as usize,
        native_session_queue_clear_gateway as *const () as usize,
        native_session_queue_growth_gateway as *const () as usize,
    ]
}
const SITES: [(usize, [u8; 8], usize); 7] = [
    (0x847510, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x74], 5),
    (0x527440, [0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83], 5),
    (
        0x83d570,
        [0x40, 0x55, 0x53, 0x56, 0x57, 0x41, 0x54, 0x41],
        5,
    ),
    (
        0x7f9c30,
        [0x44, 0x89, 0x44, 0x24, 0x18, 0x89, 0x54, 0x24],
        5,
    ),
    (
        0xa51f90,
        [0x48, 0x8b, 0xca, 0xe9, 0x58, 0x06, 0xf3, 0xff],
        8,
    ),
    (0x847390, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x74], 5),
    (
        0x846ef0,
        [0x40, 0x53, 0x41, 0x56, 0x48, 0x83, 0xec, 0x38],
        8,
    ),
];
fn code_matches(b: &[u8], f: &FixedFrame) -> bool {
    b.len() == (f.end - f.begin) as usize && format!("{:x}", Sha256::digest(b)) == f.sha
}
unsafe fn verify(exe: usize) -> Result<(), &'static str> {
    for f in fixed_queue::FRAMES {
        let len = (f.end - f.begin) as usize;
        if !committed_range(exe + f.begin as usize, len, 1, false) {
            return Err("deferred full code range");
        }
        let b = unsafe { std::slice::from_raw_parts((exe + f.begin as usize) as *const u8, len) };
        if !code_matches(b, f) {
            return Err("deferred full code differs");
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
                    .map(|r| r.as_slice())
                    != Some(expected.as_slice())
            {
                return Err("deferred full pdata differs");
            }
            for (i, b) in unhex(p.hex).iter().enumerate() {
                if copy_current::<1>(exe as u64 + u64::from(p.unwind) + i as u64) != Some([*b]) {
                    return Err("deferred full unwind differs");
                }
            }
        }
    }
    if copy_current::<8>((exe + 0xa51f90) as u64) != Some(SITES[4].1) {
        return Err("deferred leaf bytes differ");
    }
    Ok(())
}
pub(super) unsafe fn install(exe: usize) -> Result<(), &'static str> {
    unsafe {
        verify(exe)?;
        install_at(exe)
    }
}
unsafe fn install_at(exe: usize) -> Result<(), &'static str> {
    let mut hooks = QUEUE_HOOKS
        .lock()
        .map_err(|_| "deferred hook owner poisoned")?;
    if !hooks.is_empty() {
        return Err("deferred already installed");
    }
    for (rva, word, _) in SITES {
        if !(exe + rva).is_multiple_of(8) || copy_current::<8>((exe + rva) as u64) != Some(word) {
            return Err("deferred entry differs");
        }
    }
    QUEUE_EXE.store(exe as u64, Ordering::Release);
    for (i, (rva, word, stolen)) in SITES.into_iter().enumerate() {
        let p = (exe + rva) as *mut u8;
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(p.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
            || info.State != MEM_COMMIT
            || info.Protect & 0xff != PAGE_EXECUTE_READ
            || info.Protect & PAGE_GUARD != 0
        {
            return Err("deferred entry protection");
        }
        let relay = unsafe { executable_near(p as usize, p as usize, stolen) }
            .ok_or("deferred relay unavailable")?;
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
            return Err("deferred relay publication");
        }
        let mut patched = word;
        patched[..5].copy_from_slice(
            &rel32_jump(p as usize, relay as usize).ok_or("deferred relay range")?,
        );
        patched[5..stolen].fill(0x90);
        // A51F90 displaced its complete MOV+tailJMP; its continuation is the
        // original9825F0 target, not the next unrelated function's padding.
        let continuation = if i == 4 {
            exe + 0x9825f0
        } else {
            exe + rva + stolen
        };
        NATIVE_SESSION_QUEUE_CONTINUE[i].store(continuation as u64, Ordering::Release);
        hooks.push(super::super::OwnedHook {
            hook: Hook {
                target: p,
                original: word,
                patched,
                prefix: None,
                _code: relay,
            },
            protection: info.Protect,
        });
        unpublished.0 = null_mut();
        let r =
            unsafe { exchange_entry(p, u64::from_le_bytes(word), u64::from_le_bytes(patched)) }?;
        if !r.exchanged
            || !r.protection_restored
            || !r.cache_flushed
            || copy_current::<8>(p as u64) != Some(patched)
        {
            return Err("critical: deferred publication unverified");
        }
    }
    Ok(())
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut hooks = QUEUE_HOOKS
        .lock()
        .map_err(|_| "deferred hook owner poisoned")?;
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
        Err("critical: deferred restoration unverified")
    }
}
#[cfg(test)]
#[path = "native_session_queue_tests.rs"]
mod tests;
