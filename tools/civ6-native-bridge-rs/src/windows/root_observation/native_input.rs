//! Native input events retain their issuing transport through storage copies.
//! Poll is a local observation frontier, never proof of OS or remote epoch.
use super::*;
use std::cell::RefCell;
#[path = "native_input_fixed.rs"]
mod fixed;
#[path = "native_input_ledger.rs"]
mod records;
use records::{Input, Receipt};
static INPUT_EXE: AtomicU64 = AtomicU64::new(0);
static INPUT_NEXT: AtomicU64 = AtomicU64::new(0);
static INPUT_HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
static RECORDS: std::sync::LazyLock<Mutex<records::Ledger<Ticket>>> =
    std::sync::LazyLock::new(|| Mutex::new(records::Ledger::new()));
#[unsafe(no_mangle)]
static NATIVE_INPUT_CONTINUE: [AtomicU64; 11] = [const { AtomicU64::new(0) }; 11];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ticket {
    message: Option<Message>,
    lease: Lease,
    transport: Transport,
}
#[derive(Clone, Copy)]
struct Pending {
    receiver: (u64, u64),
    ticket: Ticket,
    bytes: u64,
    len: u64,
    peer: u32,
    flags: u32,
}
struct InputCall {
    serial: u64,
    kind: u64,
    pointer: u64,
    receiver: Option<(u64, u64)>,
    ticket: Option<Ticket>,
    message: Option<Message>,
    taken: bool,
    selected: bool,
    old_start: u64,
    holds: Vec<Input>,
    copies: Vec<(Input, u64)>,
    retired: Vec<Input>,
}
thread_local! {
 static INPUT_CALLS:RefCell<Vec<InputCall>>=const{RefCell::new(Vec::new())};
 static POLLED:RefCell<Option<Pending>>=const{RefCell::new(None)};
 static FREE_CALLS:RefCell<Vec<(u64,Vec<Input>)>>=const{RefCell::new(Vec::new())};
}
fn records() -> std::sync::MutexGuard<'static, records::Ledger<Ticket>> {
    RECORDS.lock().unwrap_or_else(|_| fault())
}
fn poison() {
    if let Ok(mut s) = RECORDS.try_lock() {
        s.unresolved();
    }
    super::unresolved(None);
}
fn fault() -> ! {
    diagnostic(110);
    poison();
    std::process::abort()
}
fn serial() -> u64 {
    INPUT_NEXT
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
        .unwrap_or_else(|_| fault())
        + 1
}
fn exe(rva: u64) -> u64 {
    INPUT_EXE.load(Ordering::Acquire) + rva
}
fn value(p: u64) -> u64 {
    read64(p).unwrap_or_else(|_| fault())
}
fn caller(raw: &Saved, rva: u64) -> bool {
    #[cfg(test)]
    if let Some(pc) = tests::caller_override(rva) {
        return raw.invocation.caller == pc;
    }
    raw.invocation.caller == exe(rva)
}
fn receiver() -> Option<(u64, u64)> {
    #[cfg(test)]
    if tests::FLOW.load(Ordering::Acquire) {
        return Some((tests::NETWORK.load(Ordering::Acquire), 1));
    }
    transports::current_receiver()
}
fn after_callback(message: Option<Message>) {
    cancel(message);
}
fn network(p: u64) -> Result<Option<(Lease, Transport)>, Error> {
    #[cfg(test)]
    if tests::FLOW.load(Ordering::Acquire) {
        return Ok(tests::BOUND.get().copied().filter(|(_, t)| t.network == p));
    }
    transports::from_network(p)
}
fn reserve(lease: Lease, t: Transport) -> Result<Option<Message>, Error> {
    match super::reserve_input(lease, t) {
        // A normal same-transport reload can retire its issuing World. Keep
        // native polling transparent; Closed grants no replacement receipt.
        Err(Error::Closed) => Ok(None),
        result => result,
    }
}
fn fork(message: Option<Message>) -> Result<Option<Message>, Error> {
    message.map(super::fork_input).transpose()
}
fn cancel(message: Option<Message>) {
    if let Some(message) = message {
        state()
            .cancel_before_dispatch(message)
            .unwrap_or_else(|_| fault());
    }
}
fn collect(input: Input) {
    let receipt = records().collected(input).unwrap_or_else(|_| fault());
    if let Some(r) = receipt {
        cancel(r.message.message);
    }
}
fn release(ids: Vec<Input>) {
    for id in ids {
        records().returned(id).unwrap_or_else(|_| fault());
        collect(id);
    }
}
fn retain(record: u64) -> Option<(Input, Ticket)> {
    let mut ledger = records();
    let found = ledger.at(record)?;
    let entered = ledger
        .enter(record, found.1.transport)
        .unwrap_or_else(|_| fault())?;
    Some((entered.0, entered.1.message))
}
fn retire(record: u64) -> Option<Input> {
    let (id, _) = retain(record)?;
    if records().retire(record).unwrap_or_else(|_| fault()) != Some(id) {
        fault();
    }
    Some(id)
}
fn retain_vector(vector: u64) -> Vec<Input> {
    let (Ok(start), Ok(end)) = (read64(vector), read64(vector.saturating_add(8))) else {
        return Vec::new();
    };
    if end < start || !(end - start).is_multiple_of(0x40) || (end - start) / 0x40 > 1_048_576 {
        return Vec::new();
    }
    (0..(end - start) / 0x40)
        .filter_map(|i| retain(start + i * 0x40).map(|(id, _)| id))
        .collect()
}
/// Exact callback ancestry masks unknown storage instead of adopting an outer
/// unrelated INPUT/current Session. The ticket is consumed once by860000.
pub(super) fn incoming(raw: &Saved) -> Option<Option<Message>> {
    INPUT_CALLS.with(|s| {
        let mut s = s.borrow_mut();
        let c = s.iter_mut().rev().find(|c| c.kind == 120 && c.selected)?;
        let Some(ticket) = c.ticket else {
            return Some(None);
        };
        if raw.gpr[0] != value(ticket.transport.pointer + 8) || c.taken {
            fault();
        }
        c.taken = true;
        Some(c.message.take())
    })
}
pub(super) fn current_packet() -> Option<super::PacketReceipt> {
    INPUT_CALLS.with(|calls| {
        let calls = calls.borrow();
        let call = calls
            .iter()
            .rev()
            .find(|call| call.kind == 120 && call.selected)?;
        let ticket = call.ticket?;
        let input = call.holds.first()?;
        Some(super::PacketReceipt {
            serial: input.0,
            lease: ticket.lease,
            transport: ticket.transport.incarnation,
            world: ticket
                .message
                .map(|message| state().message_world(message).unwrap_or_else(|_| fault())),
        })
    })
}
/// Called by the existing A51F90 owner; never installs a competing free hook.
/// Growth releases old element storage inline, without857040 destructor calls.
pub(super) fn free_begin(raw: &Saved) {
    if INPUT_EXE.load(Ordering::Acquire) == 0 {
        return;
    }
    let mut ids = Vec::new();
    if caller(raw, 0x8572eb) {
        if let Some(id) = retire(raw.gpr[0].saturating_sub(0x20)) {
            ids.push(id);
        }
    }
    FREE_CALLS.with(|s| s.borrow_mut().push((raw.invocation.caller_rsp, ids)));
}
pub(super) fn free_end(raw: &Saved, exceptional: bool) {
    if INPUT_EXE.load(Ordering::Acquire) == 0 {
        return;
    }
    let (rsp, ids) = FREE_CALLS
        .with(|s| s.borrow_mut().pop())
        .unwrap_or_else(|| fault());
    if rsp != raw.invocation.caller_rsp {
        fault();
    }
    if exceptional && !ids.is_empty() {
        poison();
        return;
    }
    release(ids);
}
pub(super) fn pending() -> Result<usize, &'static str> {
    records().pending().map_err(|_| "input unresolved")
}
/// The full receive-frame owner calls this even when no Core is installed.
/// A poll ticket may not escape that exact native receiver invocation.
pub(super) fn receiver_returned(network: u64, invocation: u64, exceptional: bool) {
    let p = POLLED.with(|s| {
        let mut s = s.borrow_mut();
        if s.as_ref()
            .is_some_and(|p| p.receiver == (network, invocation))
        {
            s.take()
        } else {
            None
        }
    });
    if let Some(p) = p {
        if exceptional {
            poison();
        } else {
            cancel(p.ticket.message);
        }
    }
}
extern "system" fn begin(raw: *mut Saved) {
    let e = unsafe { GetLastError() };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = unsafe { &mut *raw };
        let mut c = InputCall {
            serial: serial(),
            kind: raw.invocation.kind,
            pointer: raw.gpr[0],
            receiver: None,
            ticket: None,
            message: None,
            taken: false,
            selected: false,
            old_start: 0,
            holds: Vec::new(),
            copies: Vec::new(),
            retired: Vec::new(),
        };
        match c.kind {
            110 if caller(raw, 0x8615a0) => {
                c.pointer = raw.gpr[1];
                c.receiver = receiver();
                if let Some((net, _)) = c.receiver {
                    if raw.gpr[0] != value(net + 0xd0) {
                        fault();
                    }
                    if let Some((lease, t)) = network(net).unwrap_or_else(|_| fault()) {
                        let message = reserve(lease, t).unwrap_or_else(|_| fault());
                        c.ticket = Some(Ticket {
                            message,
                            lease,
                            transport: t,
                        });
                    }
                }
            }
            111 if caller(raw, 0x8615e9) => {
                let receiver = receiver();
                let p = POLLED.with(|s| {
                    let mut s = s.borrow_mut();
                    if s.as_ref().is_some_and(|p| Some(p.receiver) == receiver) {
                        s.take()
                    } else {
                        None
                    }
                });
                if let Some(p) = p {
                    if raw.gpr[1] as u32 != 4
                        || raw.gpr[2] as u32 != p.peer
                        || raw.gpr[3] != p.bytes
                        || value(raw.invocation.caller_rsp + 0x28) != p.len
                        || value(raw.invocation.caller_rsp + 0x30) as u32 != p.flags
                    {
                        fault();
                    }
                    c.ticket = Some(p.ticket);
                }
            }
            112 | 113 => {
                if [0x8574ab, 0x8571af, 0x85729b, 0x8571f3, 0x85708c]
                    .into_iter()
                    .any(|pc| caller(raw, pc))
                {
                    let source = raw.gpr[1].saturating_sub(8);
                    let dest = raw.gpr[0].saturating_sub(8);
                    c.pointer = dest;
                    if source != dest {
                        if let Some(old) = retire(dest) {
                            c.retired.push(old);
                        }
                    }
                    if let Some((id, _)) = retain(source) {
                        c.holds.push(id);
                        if source != dest {
                            c.copies.push((id, dest));
                        }
                    }
                }
            }
            114 | 115 => {
                if [0x856e62, 0x85727a, 0x8572c6]
                    .into_iter()
                    .any(|pc| caller(raw, pc))
                {
                    let start = raw.gpr[1];
                    let end = raw.gpr[2];
                    let dest = raw.gpr[3];
                    if end < start
                        || !(end - start).is_multiple_of(0x40)
                        || (end - start) / 0x40 > 1_048_576
                    {
                        fault();
                    }
                    for i in 0..(end - start) / 0x40 {
                        if let Some((id, _)) = retain(start + i * 0x40) {
                            c.holds.push(id);
                            if dest != 0 {
                                c.copies.push((id, dest + i * 0x40));
                            }
                        }
                    }
                }
            }
            116 => {
                // Whole extract scope. Range helper binds each copy before index publication.
                if caller(raw, 0x84edf0) {
                    c.receiver = network(raw.gpr[0])
                        .unwrap_or_else(|_| fault())
                        .map(|_| (raw.gpr[0], c.serial));
                }
            }
            117 => {
                c.holds = retain_vector(raw.gpr[1]);
            } //114 binds the copied locations.
            118 => {
                if let Some(id) = retire(raw.gpr[0]) {
                    c.retired.push(id);
                }
            }
            119 => {
                c.old_start = read64(raw.gpr[0]).unwrap_or(0);
                c.holds = retain_vector(raw.gpr[0]);
                if let Ok(end) = read64(raw.gpr[0].saturating_add(8)) {
                    if end >= c.old_start
                        && (end - c.old_start).is_multiple_of(0x40)
                        && (end - c.old_start) / 0x40 <= 1_048_576
                    {
                        let ledger = records();
                        c.copies = (0..(end - c.old_start) / 0x40)
                            .filter_map(|i| {
                                let address = c.old_start + i * 0x40;
                                ledger.at(address).map(|(id, _)| (id, address))
                            })
                            .collect();
                    }
                }
            }
            120 if caller(raw, 0x84ee79) => {
                c.selected = true;
                let record = raw.gpr[7];
                if let Some((id, ticket)) = retain(record) {
                    c.holds.push(id);
                    if raw.gpr[0] != ticket.transport.pointer
                        || value(record) as u32 != 4
                        || value(value(raw.gpr[0]) + 0x68) != exe(0x84e9e0)
                        || value(value(value(raw.gpr[0] + 8)) + 0x28) != exe(0x860000)
                    {
                        fault();
                    }
                    if network(ticket.transport.network).unwrap_or_else(|_| fault())
                        != Some((ticket.lease, ticket.transport))
                    {
                        fault();
                    }
                    if raw.gpr[1] as u32 != read32(record + 4).unwrap_or_else(|_| fault())
                        || raw.gpr[2] != value(record + 8)
                        || raw.gpr[3] != value(record + 0x10) - value(record + 8)
                        || value(raw.invocation.caller_rsp + 0x28) as u8
                            != (read32(record + 0x28).unwrap_or_else(|_| fault()) & 1) as u8
                    {
                        fault();
                    }
                    c.ticket = Some(ticket);
                    c.message = fork(ticket.message).unwrap_or_else(|_| fault());
                }
            }
            110 | 111 | 120 => {}
            _ => fault(),
        }
        raw.invocation.cookie = COOKIE;
        raw.invocation.token = c.serial;
        INPUT_CALLS.with(|s| s.borrow_mut().push(c));
    }))
    .is_err()
    {
        fault();
    }
    unsafe { SetLastError(e) };
}
fn finish(raw: &mut Saved, exceptional: bool) {
    if raw.invocation.cookie != COOKIE {
        return;
    }
    raw.invocation.cookie = 0;
    let c = INPUT_CALLS
        .with(|s| s.borrow_mut().pop())
        .unwrap_or_else(|| fault());
    if (c.serial, c.kind) != (raw.invocation.token, raw.invocation.kind) {
        fault();
    }
    if exceptional {
        if c.ticket.is_some() || !c.holds.is_empty() || !c.retired.is_empty() {
            poison();
        }
        return;
    }
    match c.kind {
        110 => {
            if let Some(ticket) = c.ticket {
                if raw.gpr[4] as u32 as i32 > 0
                    && read32(c.pointer).unwrap_or_else(|_| fault()) == 4
                {
                    let packet = value(c.pointer + 0x18);
                    let peer = value(c.pointer + 8);
                    if packet == 0 || peer == 0 {
                        fault();
                    }
                    let p = Pending {
                        receiver: c.receiver.unwrap(),
                        ticket,
                        bytes: value(packet + 0x10),
                        len: value(packet + 0x18),
                        peer: read32(peer + 0xc).unwrap_or_else(|_| fault()),
                        flags: read32(packet + 8).unwrap_or_else(|_| fault()),
                    };
                    if POLLED.with(|s| s.borrow_mut().replace(p)).is_some() {
                        fault();
                    }
                } else {
                    cancel(ticket.message);
                }
            }
        }
        111 => {
            if let Some(ticket) = c.ticket {
                if raw.gpr[4] != c.pointer {
                    fault();
                }
                records()
                    .issued(
                        Receipt {
                            message: ticket,
                            transport: ticket.transport.incarnation,
                        },
                        c.pointer,
                    )
                    .unwrap_or_else(|_| fault());
            }
        }
        112..=115 => {
            for (id, dest) in c.copies {
                records().bind(id, dest).unwrap_or_else(|_| fault());
            }
        }
        120 => after_callback(c.message),
        119 if read64(c.pointer).ok() != Some(c.old_start) => {
            // Null byte vectors skip native free entirely. Retire only the
            // captured old serial after the original storage replacement;
            // never remove a newer object reusing that physical address.
            for (id, address) in c.copies {
                if records()
                    .at(address)
                    .is_some_and(|(current, _)| current == id)
                {
                    if let Some(retired) = retire(address) {
                        release(vec![retired]);
                    }
                }
            }
        }
        _ => {}
    }
    release(c.holds);
    release(c.retired);
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
    if !r.is_null() && unsafe { (*r).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0 {
        finish(unsafe { &mut *((frame + 0x20) as *mut Saved) }, true);
    }
    unsafe { SetLastError(e) };
    1
}
macro_rules! original_home {
    ($name:literal,$offset:literal,$reg:literal,$unwind:literal,$slot:literal,$a:literal,$b:literal) => {
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
            $reg,
            "\n",
            $unwind,
            ".seh_endprologue\nmov [rsp+",
            $a,
            "],rax\nmov rax,[rip+NATIVE_INPUT_CONTINUE+",
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
original_home!(
    "input_poll_original",
    "8",
    "rbx",
    ".seh_savereg rbx,8\n",
    "0",
    "0x10",
    "0x20"
);
original_home!(
    "input_ctor_original",
    "8",
    "rsi",
    ".seh_savereg rsi,8\n",
    "8",
    "0x18",
    "0x20"
);
original_home!(
    "input_copy_original",
    "8",
    "rbx",
    ".seh_savereg rbx,8\n",
    "16",
    "0x18",
    "0x20"
);
original_home!(
    "input_assign_original",
    "0x20",
    "rbx",
    ".seh_savereg rbx,0x20\n",
    "24",
    "8",
    "0x10"
);
original_home!("input_range_original", "0x18", "r8", "", "32", "8", "0x20");
original_home!(
    "input_growth_range_original",
    "0x20",
    "r9",
    "",
    "40",
    "8",
    "0x10"
);
original_home!(
    "input_extract_original",
    "8",
    "rbx",
    ".seh_savereg rbx,8\n",
    "48",
    "0x18",
    "0x20"
);
original_home!(
    "input_vector_original",
    "0x10",
    "rbx",
    ".seh_savereg rbx,0x10\n",
    "56",
    "8",
    "0x20"
);
original_home!(
    "input_growth_original",
    "0x20",
    "rbx",
    ".seh_savereg rbx,0x20\n",
    "72",
    "8",
    "0x10"
);
global_asm!(
    ".globl input_destroy_original",
    ".seh_proc input_destroy_original",
    "input_destroy_original:",
    ".seh_endprologue",
    "mov rdx,[rcx+8]",
    "test rdx,rdx",
    "jmp qword ptr [rip+NATIVE_INPUT_CONTINUE+64]",
    ".seh_endproc",
    ".globl input_callback_original",
    ".seh_proc input_callback_original",
    "input_callback_original:",
    ".seh_endprologue",
    "mov rcx,[rcx+8]",
    "test rcx,rcx",
    "jmp qword ptr [rip+NATIVE_INPUT_CONTINUE+80]",
    ".seh_endproc",
);
unsafe extern "system" {
    fn input_poll_original();
    fn input_ctor_original();
    fn input_copy_original();
    fn input_assign_original();
    fn input_range_original();
    fn input_growth_range_original();
    fn input_extract_original();
    fn input_vector_original();
    fn input_destroy_original();
    fn input_growth_original();
    fn input_callback_original();
    fn input_poll_gateway();
    fn input_ctor_gateway();
    fn input_copy_gateway();
    fn input_assign_gateway();
    fn input_range_gateway();
    fn input_growth_range_gateway();
    fn input_extract_gateway();
    fn input_vector_gateway();
    fn input_destroy_gateway();
    fn input_growth_gateway();
    fn input_callback_gateway();
}
gateway!(
    "input_poll_gateway",
    "110",
    input_poll_original,
    begin,
    end,
    unwind
);
// Original fifth/sixth arguments are copied into the new original CALL frame.
// Scratch is a literal MOV; original RAX and all input flags survive forwarding.
gateway!(
    "input_ctor_gateway",
    "111",
    input_ctor_original,
    begin,
    end,
    unwind,
    "mov rax,[rsp+0x1f0]\nmov [rsp+0x20],rax\nmov rax,[rsp+0x1f8]\nmov [rsp+0x28],rax\nmov rax,[rsp+0x40]\n"
);
gateway!(
    "input_copy_gateway",
    "112",
    input_copy_original,
    begin,
    end,
    unwind
);
gateway!(
    "input_assign_gateway",
    "113",
    input_assign_original,
    begin,
    end,
    unwind
);
gateway!(
    "input_range_gateway",
    "114",
    input_range_original,
    begin,
    end,
    unwind
);
gateway!(
    "input_growth_range_gateway",
    "115",
    input_growth_range_original,
    begin,
    end,
    unwind
);
gateway!(
    "input_extract_gateway",
    "116",
    input_extract_original,
    begin,
    end,
    unwind
);
gateway!(
    "input_vector_gateway",
    "117",
    input_vector_original,
    begin,
    end,
    unwind
);
gateway!(
    "input_destroy_gateway",
    "118",
    input_destroy_original,
    begin,
    end,
    unwind
);
gateway!(
    "input_growth_gateway",
    "119",
    input_growth_original,
    begin,
    end,
    unwind
);
gateway!(
    "input_callback_gateway",
    "120",
    input_callback_original,
    begin,
    end,
    unwind,
    "mov rax,[rsp+0x1f0]\nmov [rsp+0x20],rax\nmov rax,[rsp+0x40]\n"
);
fn gateways() -> [usize; 11] {
    [
        input_poll_gateway as *const () as usize,
        input_ctor_gateway as *const () as usize,
        input_copy_gateway as *const () as usize,
        input_assign_gateway as *const () as usize,
        input_range_gateway as *const () as usize,
        input_growth_range_gateway as *const () as usize,
        input_extract_gateway as *const () as usize,
        input_vector_gateway as *const () as usize,
        input_destroy_gateway as *const () as usize,
        input_growth_gateway as *const () as usize,
        input_callback_gateway as *const () as usize,
    ]
}
const SITES: [(usize, [u8; 8], usize); 11] = [
    (0xc66350, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x74], 5),
    (0x856e80, [0x48, 0x89, 0x74, 0x24, 8, 0x48, 0x89, 0x7c], 5),
    (0x509a00, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x74], 5),
    (
        0x50a3e0,
        [0x48, 0x89, 0x5c, 0x24, 0x20, 0x57, 0x48, 0x83],
        5,
    ),
    (
        0x81f450,
        [0x4c, 0x89, 0x44, 0x24, 0x18, 0x48, 0x89, 0x54],
        5,
    ),
    (
        0x81f2f0,
        [0x4c, 0x89, 0x4c, 0x24, 0x20, 0x4c, 0x89, 0x44],
        5,
    ),
    (0x8573f0, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x74], 5),
    (
        0x856df0,
        [0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x74],
        5,
    ),
    (0x857040, [0x48, 0x8b, 0x51, 8, 0x48, 0x85, 0xd2, 0x74], 7),
    (
        0x857160,
        [0x48, 0x89, 0x5c, 0x24, 0x20, 0x56, 0x57, 0x41],
        5,
    ),
    (0x84e9e0, [0x48, 0x8b, 0x49, 8, 0x48, 0x85, 0xc9, 0x74], 7),
];
fn code_matches(bytes: &[u8], frame: &FixedFrame) -> bool {
    bytes.len() == (frame.end - frame.begin) as usize
        && format!("{:x}", Sha256::digest(bytes)) == frame.sha
}
pub(super) unsafe fn prepare_host(module: usize) -> Result<(), &'static str> {
    let _pin = super::super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA)?;
    unsafe {
        verify(module)?;
        install_at(module, &SITES)
    }
}
unsafe fn verify(module: usize) -> Result<(), &'static str> {
    for f in fixed::FRAMES {
        let len = (f.end - f.begin) as usize;
        if !committed_range(module + f.begin as usize, len, 1, false) {
            return Err("input code range");
        }
        if !code_matches(
            unsafe { std::slice::from_raw_parts((module + f.begin as usize) as *const u8, len) },
            f,
        ) {
            return Err("input whole code differs");
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
                return Err("input pdata differs");
            }
            for (i, byte) in (0..p.hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&p.hex[i..i + 2], 16).unwrap())
                .enumerate()
            {
                if copy_current::<1>(module as u64 + u64::from(p.unwind) + i as u64) != Some([byte])
                {
                    return Err("input unwind differs");
                }
            }
        }
    }
    Ok(())
}
unsafe fn install_at(
    module: usize,
    sites: &[(usize, [u8; 8], usize); 11],
) -> Result<(), &'static str> {
    let mut hooks = INPUT_HOOKS.lock().map_err(|_| "input hook owner")?;
    if !hooks.is_empty() {
        return Err("input hooks already installed");
    }
    for &(rva, expected, _) in sites {
        if copy_current::<8>((module + rva) as u64) != Some(expected) {
            return Err("input entry differs");
        }
    }
    INPUT_EXE.store(module as u64, Ordering::Release);

    for (i, &(rva, expected, stolen)) in sites.iter().enumerate() {
        let target = (module + rva) as *mut u8;
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if !(target as usize).is_multiple_of(8)
            || unsafe { VirtualQuery(target.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
            || info.State != MEM_COMMIT
            || info.Protect & 0xff != PAGE_EXECUTE_READ
            || info.Protect & PAGE_GUARD != 0
        {
            return Err("input entry protection");
        }
        let relay = unsafe { executable_near(target as usize, target as usize, stolen) }
            .ok_or("input relay")?;
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
            return Err("input relay publication");
        }
        let mut patched = expected;
        patched[..5].copy_from_slice(
            &rel32_jump(target as usize, relay as usize).ok_or("input relay range")?,
        );
        patched[5..stolen.min(8)].fill(0x90);
        NATIVE_INPUT_CONTINUE[i].store((module + rva + stolen) as u64, Ordering::Release);
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
            return Err("critical: input publication unverified");
        }
    }
    Ok(())
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut hooks = INPUT_HOOKS.lock().map_err(|_| "input hook owner")?;
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
        Err("critical: input restoration unverified")
    }
}

#[cfg(test)]
#[path = "native_input_tests.rs"]
mod tests;
