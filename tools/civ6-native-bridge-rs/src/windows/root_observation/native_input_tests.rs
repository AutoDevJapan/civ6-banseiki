use super::*;
use std::os::windows::process::CommandExt;
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlLookupFunctionEntry, IMAGE_RUNTIME_FUNCTION_ENTRY,
};
pub(super) static FLOW: AtomicBool = AtomicBool::new(false);
pub(super) static NETWORK: AtomicU64 = AtomicU64::new(0);
pub(super) static BOUND: OnceLock<(Lease, Transport)> = OnceLock::new();

static DEST: AtomicU64 = AtomicU64::new(0);
static OBSERVED: AtomicU64 = AtomicU64::new(0);
pub(super) fn caller_override(rva: u64) -> Option<u64> {
    if !FLOW.load(Ordering::Acquire) {
        return None;
    }
    Some(match rva {
        0x8615a0 => fixture_poll_return as *const () as u64,
        0x8615e9 => fixture_ctor_return as *const () as u64,
        0x8574ab => fixture_copy_return as *const () as u64,
        0x856e62 => fixture_range_return as *const () as u64,
        0x84ee79 => fixture_callback_return as *const () as u64,
        _ => return None,
    })
}
fn child(name: &str) -> bool {
    const ENV: &str = "CIV6_NATIVE_INPUT_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    let mut p = std::process::Command::new(std::env::current_exe().unwrap())
        .creation_flags(0x08000000)
        .env(ENV, "1")
        .args([
            "--exact",
            &format!("windows::root_observation::native_frames::sessions::input::tests::{name}"),
            "--nocapture",
        ])
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(s) = p.try_wait().unwrap() {
            assert!(s.success(), "input child{s}");
            return false;
        }
        if Instant::now() >= deadline {
            let _ = p.kill();
            let _ = p.wait();
            panic!("input timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
extern "system" fn ctor_body(
    record: u64,
    kind: u64,
    peer: u64,
    bytes: u64,
    len: u64,
    flags: u64,
) -> u64 {
    unsafe {
        *(record as *mut u32) = kind as u32;
        *((record + 4) as *mut u32) = peer as u32;
        *((record + 8) as *mut u64) = bytes;
        *((record + 0x10) as *mut u64) = bytes + len;
        *((record + 0x28) as *mut u32) = flags as u32;
    }
    record
}
extern "system" fn copy_body(destination: u64, source: u64) -> u64 {
    unsafe { ptr::copy_nonoverlapping(source as *const u8, destination as *mut u8, 0x18) };
    destination
}
extern "system" fn range_body(output: u64, source: u64, end: u64, dest: u64) -> u64 {
    unsafe {
        ptr::copy_nonoverlapping(
            source as *const u8,
            dest as *mut u8,
            (end - source) as usize,
        );
        *(output as *mut u64) = dest + end - source;
    }
    output
}
extern "system" fn callback_body(transport: u64, _: u64, _: u64, _: u64, _: u64) {
    if ACTUAL_INCOMING.load(Ordering::Acquire) {
        let call: unsafe extern "system" fn(u64, u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(super::super::gateways()[2]) };
        unsafe { call(value(transport + 8), 7, 0, 0, 0x55) };
        return;
    }
    let mut r: Saved = unsafe { std::mem::zeroed() };
    r.gpr[0] = value(transport + 8);
    let m = incoming(&r).unwrap().unwrap();
    let world = state().message_world(m).unwrap();
    OBSERVED.store(world.epoch, Ordering::Release);
    state().cancel_before_dispatch(m).unwrap();
}
static ACTUAL_INCOMING: AtomicBool = AtomicBool::new(false);
static OBSERVED_PACKET: Mutex<Option<super::super::PacketReceipt>> = Mutex::new(None);
extern "system" fn incoming_body() -> u64 {
    *OBSERVED_PACKET.lock().unwrap() = current_packet();
    let message = super::super::CALLS.with(|s| {
        s.borrow()
            .iter()
            .rev()
            .find(|c| c.kind == 72)
            .and_then(|c| c.message)
    });
    let epoch = message
        .and_then(|m| state().message_world(m).ok())
        .map(|w| w.epoch)
        .unwrap_or(0);
    OBSERVED.store(epoch, Ordering::Release);
    0x1234
}
fn scope_enter(epoch: u64) -> Result<u64, Error> {
    FRAMES
        .get()
        .unwrap()
        .enter_existing_epoch(epoch)
        .map_err(|_| Error::Stale)
}
fn scope_leave(hold: u64, exceptional: bool) -> Result<(), Error> {
    let f = FRAMES.get().unwrap();
    if exceptional {
        f.mark_unresolved();
    }
    f.leave(hold).map_err(|_| Error::Stale)
}
global_asm!(
".globl fixture_incoming_original",".seh_proc fixture_incoming_original","fixture_incoming_original:","mov [rsp+0x20],rbx",".seh_savereg rbx,0x20",".seh_endprologue","jmp {body}",".seh_endproc",
body=sym incoming_body,
);
unsafe extern "system" {
    fn fixture_incoming_original();
}
global_asm!(
".globl fixture_poll_driver",".seh_proc fixture_poll_driver","fixture_poll_driver:","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {poll}",".globl fixture_poll_return","fixture_poll_return:","lea rsp,[rsp+0x28]","ret",".seh_endproc",
".globl fixture_ctor_driver",".seh_proc fixture_ctor_driver","fixture_ctor_driver:","sub rsp,0x38",".seh_stackalloc 0x38",".seh_endprologue","mov rax,[rsp+0x60]","mov [rsp+0x20],rax","mov rax,[rsp+0x68]","mov [rsp+0x28],rax","call {ctor}",".globl fixture_ctor_return","fixture_ctor_return:","lea rsp,[rsp+0x38]","ret",".seh_endproc",
".globl fixture_copy_driver",".seh_proc fixture_copy_driver","fixture_copy_driver:","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {copy}",".globl fixture_copy_return","fixture_copy_return:","lea rsp,[rsp+0x28]","ret",".seh_endproc",
".globl fixture_range_driver",".seh_proc fixture_range_driver","fixture_range_driver:","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {range}",".globl fixture_range_return","fixture_range_return:","lea rsp,[rsp+0x28]","ret",".seh_endproc",
".globl fixture_callback_driver",".seh_proc fixture_callback_driver","fixture_callback_driver:","push rbx",".seh_pushreg rbx","sub rsp,0x30",".seh_stackalloc 0x30",".seh_endprologue","mov rbx,[rsp+0x60]","mov rax,[rsp+0x68]","mov [rsp+0x20],rax","call {callback}",".globl fixture_callback_return","fixture_callback_return:","lea rsp,[rsp+0x30]","pop rbx","ret",".seh_endproc",
// Synthetic continuations preserve the exact stolen home stores.
".globl fixture_ctor_original",".seh_proc fixture_ctor_original","fixture_ctor_original:","mov [rsp+8],rsi",".seh_savereg rsi,8",".seh_endprologue","jmp {ctor_body}",".seh_endproc",
".globl fixture_copy_original",".seh_proc fixture_copy_original","fixture_copy_original:","mov [rsp+8],rbx",".seh_savereg rbx,8",".seh_endprologue","jmp {copy_body}",".seh_endproc",
".globl fixture_range_original",".seh_proc fixture_range_original","fixture_range_original:","mov [rsp+0x18],r8",".seh_endprologue","jmp {range_body}",".seh_endproc",
".globl fixture_callback_original","fixture_callback_original:","mov rcx,[rcx+8]","test rcx,rcx","mov rcx,[rip+{dest}]","jmp {callback_body}",
poll=sym input_poll_gateway,ctor=sym input_ctor_gateway,copy=sym input_copy_gateway,range=sym input_range_gateway,callback=sym input_callback_gateway,
ctor_body=sym ctor_body,copy_body=sym copy_body,range_body=sym range_body,callback_body=sym callback_body,dest=sym DEST,
);
unsafe extern "system" {
    fn fixture_poll_return();
    fn fixture_ctor_return();
    fn fixture_copy_return();
    fn fixture_range_return();
    fn fixture_callback_return();
    fn fixture_ctor_original();
    fn fixture_copy_original();
    fn fixture_range_original();
    fn fixture_callback_original();
    fn fixture_ctor_driver(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> u64;
    fn fixture_copy_driver(a: u64, b: u64) -> u64;
    fn fixture_range_driver(a: u64, b: u64, c: u64, d: u64) -> u64;
    fn fixture_callback_driver(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64);
}
fn fixture_ticket() -> Ticket {
    let mut s = state();
    let lease = s.birth(1, Some(1), 2, 2).unwrap();
    s.factory(lease, 7, 2, 3, 4).unwrap();
    s.published(lease, 7).unwrap();
    s.bind_world(
        lease,
        WorldReceipt {
            binding_id: 8,
            epoch: 1,
        },
    )
    .unwrap();
    let t = s.publish_transport(lease, 1, 5, 6, 7).unwrap();
    let message = s.reserve_input(lease, t).unwrap().unwrap();
    Ticket {
        message: Some(message),
        lease,
        transport: t,
        transport_hold: None,
    }
}
#[test]
fn pre_world_native_storage_owns_transport_until_last_copy_is_released() {
    if !child("pre_world_native_storage_owns_transport_until_last_copy_is_released") {
        return;
    }
    let mut ledger = state();
    let lease = ledger.birth(1, Some(1), 2, 2).unwrap();
    ledger.factory(lease, 7, 2, 3, 4).unwrap();
    ledger.published(lease, 7).unwrap();
    let transport = ledger.publish_transport(lease, 900, 5, 6, 7).unwrap();
    drop(ledger);
    let ticket = reserve_ticket(lease, transport).unwrap();
    assert!(ticket.message.is_none());
    let input = records()
        .issued(
            Receipt {
                message: ticket,
                transport: transport.incarnation,
            },
            100,
        )
        .unwrap();
    records().bind(input, 200).unwrap();
    let world = WorldReceipt {
        binding_id: 1,
        epoch: 1,
    };
    state().bind_world(lease, world).unwrap();
    assert_eq!(state().retire_world(world), Err(Error::Busy));
    let first = retire(100).unwrap();
    release(vec![first]);
    assert_eq!(
        state().retire_world(world),
        Err(Error::Busy),
        "remaining native copy lost its ownership"
    );
    let last = retire(200).unwrap();
    release(vec![last]);
    assert_eq!(pending(), Ok(0));
    state().retire_world(world).unwrap();
    assert!(state()
        .release_input_transport(ticket.transport_hold.unwrap())
        .is_err());
}
#[test]
fn pending_input_uses_fixed_birth_world_and_unknown_never_mints() {
    if !child("pending_input_uses_fixed_birth_world_and_unknown_never_mints") {
        return;
    }
    let t = fixture_ticket();
    let copied = fork(t.message).unwrap();
    assert_eq!(state().message_world(copied.unwrap()).unwrap().epoch, 1);
    cancel(copied);
    cancel(t.message);
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    assert!(incoming(&raw).is_none());
    raw.invocation.kind = 111;
    begin(&mut raw);
    raw.gpr[4] = 0;
    finish(&mut raw, false);
    assert_eq!(pending(), Ok(0));
}
#[test]
fn original_ctor_six_args_copy_and_range_keep_old_receipt() {
    if !child("original_ctor_six_args_copy_and_range_keep_old_receipt") {
        return;
    }
    FLOW.store(true, Ordering::Release);
    INPUT_EXE.store(0x140000000, Ordering::Release);
    let ticket = fixture_ticket();
    let mut record = [0u64; 8];
    let p = record.as_mut_ptr() as u64;
    POLLED.with(|s| {
        *s.borrow_mut() = Some(Pending {
            receiver: (0, 1),
            ticket,
            bytes: 0x5000,
            len: 9,
            peer: 7,
            flags: 0x35,
        })
    });
    NATIVE_INPUT_CONTINUE[1].store(
        fixture_ctor_original as *const () as u64 + 5,
        Ordering::Release,
    );
    unsafe {
        SetLastError(0x123456);
        assert_eq!(fixture_ctor_driver(p, 4, 7, 0x5000, 9, 0x35), p);
    }
    assert_eq!(unsafe { GetLastError() }, 0x123456);
    assert_eq!(record[5] as u32, 0x35);
    assert_eq!(records().at(p).unwrap().1.message, ticket);
    let mut copied = [0u64; 8];
    let dest = copied.as_mut_ptr() as u64;
    NATIVE_INPUT_CONTINUE[2].store(
        fixture_copy_original as *const () as u64 + 5,
        Ordering::Release,
    );
    unsafe {
        fixture_copy_driver(dest + 8, p + 8);
    }
    assert_eq!(records().at(dest).unwrap().1.message, ticket);
    let mut moved = [0u64; 8];
    let mut out = 0u64;
    NATIVE_INPUT_CONTINUE[4].store(
        fixture_range_original as *const () as u64 + 5,
        Ordering::Release,
    );
    unsafe {
        fixture_range_driver(
            &mut out as *mut u64 as u64,
            dest,
            dest + 0x40,
            moved.as_mut_ptr() as u64,
        );
    }
    for address in [p, dest, moved.as_mut_ptr() as u64] {
        let id = retire(address).unwrap();
        release(vec![id]);
    }
    assert_eq!(pending(), Ok(0));
}

#[test]
fn typed_callback_enters_shared_incoming_with_saved_input() {
    if !child("typed_callback_enters_shared_incoming_with_saved_input") {
        return;
    }
    FLOW.store(true, Ordering::Release);
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0x2360000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    INPUT_EXE.store(base as u64, Ordering::Release);
    let mut tr = [0u64; 16];
    let mut cb = [0u64; 2];
    let mut record = [0u64; 8];
    let mut bytes = [1u8; 7];
    let mut net = Box::new([0u64; 26]);
    let mut session = Box::new([0u64; 10]);
    let mut thread = Box::new([0u64; 28]);
    let mut registry = Box::new([0u64; 4]);
    let mut buckets = [0u64; 2];
    let node = [7u64, session.as_ptr() as u64, 0];
    tr[0] = (base + 0x1047e60) as u64;
    tr[1] = cb.as_mut_ptr() as u64;
    cb[0] = (base + 0x10494a8) as u64;
    cb[1] = 7;
    tr[2] = net.as_ptr() as u64;
    tr[8] = 1u64 << 40;
    thread[0] = (base + 0x106d030) as u64;
    net[24] = thread.as_ptr() as u64;
    session[5] = tr.as_ptr() as u64;
    buckets[0] = node.as_ptr() as u64;
    registry[2] = buckets.as_ptr() as u64;
    registry[3] = 1;
    unsafe {
        *((base + 0x1047ec8) as *mut u64) = (base + 0x84e9e0) as u64;
        *((base + 0x10494d0) as *mut u64) = (base + 0x860000) as u64;
        *((base + 0x1047ea0) as *mut u64) = (base + 0x84ebd0) as u64;
        *((base + 0x1047eb8) as *mut u64) = (base + 0x84ec00) as u64;
        *((base + 0x106d060) as *mut u64) = (base + 0x98c650) as u64;
        *((base + 0x235b878) as *mut u64) = registry.as_ptr() as u64;
    }
    let mut s = state();
    let l = s.birth(registry.as_ptr() as u64, Some(1), 2, 2).unwrap();
    s.factory(l, 7, 2, session.as_ptr() as u64, cb.as_ptr() as u64)
        .unwrap();
    s.published(l, 7).unwrap();
    let world = WorldReceipt {
        binding_id: 8,
        epoch: 1,
    };
    s.bind_world(l, world).unwrap();
    let t = s
        .publish_transport(
            l,
            1,
            tr.as_ptr() as u64,
            net.as_ptr() as u64,
            thread.as_ptr() as u64,
        )
        .unwrap();
    let m = s.reserve_input(l, t).unwrap().unwrap();
    drop(s);
    let ticket = Ticket {
        message: Some(m),
        lease: l,
        transport: t,
        transport_hold: None,
    };
    BOUND.set((l, t)).unwrap();
    record[0] = (7u64 << 32) | 4;
    record[1] = bytes.as_mut_ptr() as u64;
    record[2] = record[1] + 7;
    let p = record.as_mut_ptr() as u64;
    records()
        .issued(
            Receipt {
                message: ticket,
                transport: t.incarnation,
            },
            p,
        )
        .unwrap();
    assert_eq!(super::super::NativeSessionApi::current_world(), None);
    super::super::EXE.store(base as u64, Ordering::Release);
    super::super::CORE.store(1, Ordering::Release);
    FRAMES.get_or_init(NativeFrames::new);
    assert!(super::super::SCOPE
        .set(SessionScopeHooks {
            enter_existing_epoch: scope_enter,
            leave: scope_leave
        })
        .is_ok());
    super::super::NATIVE_SESSION_CONTINUE[2].store(
        fixture_incoming_original as *const () as u64 + 5,
        Ordering::Release,
    );
    ACTUAL_INCOMING.store(true, Ordering::Release);
    DEST.store(tr.as_ptr() as u64, Ordering::Release);
    NATIVE_INPUT_CONTINUE[10].store(
        fixture_callback_original as *const () as u64 + 7,
        Ordering::Release,
    );
    unsafe {
        fixture_callback_driver(tr.as_ptr() as u64, 7, record[1], 7, p, 0);
    }
    assert_eq!(OBSERVED.load(Ordering::Acquire), 1);
    let known = OBSERVED_PACKET.lock().unwrap().unwrap();
    assert_eq!(known.lease, l);
    assert_eq!(known.transport, t.incarnation);
    assert_eq!(known.world, Some(world));
    let id = retire(p).unwrap();
    release(vec![id]);
    assert_eq!(pending(), Ok(0));
    let next = records()
        .issued(
            Receipt {
                message: Ticket {
                    message: None,
                    ..ticket
                },
                transport: t.incarnation,
            },
            p,
        )
        .unwrap();
    unsafe {
        fixture_callback_driver(tr.as_ptr() as u64, 7, record[1], 7, p, 0);
    }
    assert_eq!(OBSERVED.load(Ordering::Acquire), 0);
    let unknown = OBSERVED_PACKET.lock().unwrap().unwrap();
    assert_eq!(unknown.serial, next.0);
    assert_ne!(unknown.serial, known.serial);
    assert_eq!(unknown.lease, known.lease);
    assert_eq!(unknown.transport, known.transport);
    assert_eq!(unknown.world, None);
    assert!(super::super::NativeSessionApi::current_world().is_none());
    let id = retire(p).unwrap();
    release(vec![id]);
    assert_eq!(pending(), Ok(0));
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
}
#[test]
fn pending_packet_and_poll_after_rebind_never_use_new_session_world() {
    if !child("pending_packet_and_poll_after_rebind_never_use_new_session_world") {
        return;
    }
    let mut s = state();
    let l = s.birth(1, Some(1), 2, 2).unwrap();
    s.factory(l, 7, 2, 3, 4).unwrap();
    s.published(l, 7).unwrap();
    let old = WorldReceipt {
        binding_id: 8,
        epoch: 1,
    };
    let new = WorldReceipt {
        binding_id: 9,
        epoch: 2,
    };
    s.bind_world(l, old).unwrap();
    let t = s.publish_transport(l, 1, 5, 6, 7).unwrap();
    let m = s.reserve_input(l, t).unwrap().unwrap();
    assert_eq!(s.retire_world(old), Err(Error::Busy));
    s.cancel_before_dispatch(m).unwrap();
    s.retire_world(old).unwrap();
    s.loaded_world(l, Some(old), new).unwrap();
    assert_eq!(s.reserve_input(l, t), Err(Error::Closed));
    let unknown = s.birth(2, Some(1), 2, 2).unwrap();
    s.factory(unknown, 8, 2, 30, 40).unwrap();
    s.published(unknown, 8).unwrap();
    let ut = s.publish_transport(unknown, 2, 50, 60, 70).unwrap();
    s.bind_world(unknown, new).unwrap();
    assert_eq!(s.reserve_input(unknown, ut), Ok(None));
    drop(s);
    let before = census::event_count(census::SemanticEvent::RetiredInput);
    let retired = reserve_ticket(l, t).unwrap();
    assert!(retired.message.is_none());
    cancel_ticket(retired);
    assert_eq!(
        census::event_count(census::SemanticEvent::RetiredInput),
        before + 1
    );
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn RtlVirtualUnwind(
        kind: u32,
        base: u64,
        pc: u64,
        entry: *const IMAGE_RUNTIME_FUNCTION_ENTRY,
        context: *mut CONTEXT,
        data: *mut *mut c_void,
        frame: *mut u64,
        pointers: *mut c_void,
    ) -> usize;
    fn RtlUnwindEx(
        frame: *const c_void,
        ip: *const c_void,
        record: *const EXCEPTION_RECORD,
        value: *const c_void,
        context: *const CONTEXT,
        history: *const c_void,
    );
}
extern "system" fn fixture_poll_body(_: u64, out: u64, _: u64) -> u64 {
    let peer = PEER.load(Ordering::Acquire);
    let packet = PACKET.load(Ordering::Acquire);
    unsafe {
        *(out as *mut u32) = 4;
        *((out + 8) as *mut u64) = peer;
        *((out + 0x18) as *mut u64) = packet;
    }
    1
}
static PEER: AtomicU64 = AtomicU64::new(0);
static PACKET: AtomicU64 = AtomicU64::new(0);
global_asm!(
".globl fixture_poll_original",".seh_proc fixture_poll_original","fixture_poll_original:","mov [rsp+8],rbx",".seh_savereg rbx,8",".seh_endprologue","jmp {body}",".seh_endproc",
body=sym fixture_poll_body,
);
unsafe extern "system" {
    fn fixture_poll_original();
    fn fixture_poll_driver(a: u64, b: u64, c: u64) -> u64;
}
#[test]
fn actual_poll_reserves_before_body_and_binds_ctor_before_publication() {
    if !child("actual_poll_reserves_before_body_and_binds_ctor_before_publication") {
        return;
    }
    FLOW.store(true, Ordering::Release);
    INPUT_EXE.store(0x140000000, Ordering::Release);
    let mut net = [0u64; 28];
    net[0xd0 / 8] = 0x1234;
    NETWORK.store(net.as_ptr() as u64, Ordering::Release);
    let mut s = state();
    let l = s.birth(1, Some(1), 2, 2).unwrap();
    s.factory(l, 7, 2, 3, 4).unwrap();
    s.published(l, 7).unwrap();
    s.bind_world(
        l,
        WorldReceipt {
            binding_id: 8,
            epoch: 1,
        },
    )
    .unwrap();
    let t = s
        .publish_transport(l, 1, 5, net.as_ptr() as u64, 7)
        .unwrap();
    drop(s);
    BOUND.set((l, t)).unwrap();
    let peer = [0u32, 0, 0, 7];
    let packet = [0u64, 0x35, 0x5000, 9];
    PEER.store(peer.as_ptr() as u64, Ordering::Release);
    PACKET.store(packet.as_ptr() as u64, Ordering::Release);
    NATIVE_INPUT_CONTINUE[0].store(
        fixture_poll_original as *const () as u64 + 5,
        Ordering::Release,
    );
    NATIVE_INPUT_CONTINUE[1].store(
        fixture_ctor_original as *const () as u64 + 5,
        Ordering::Release,
    );
    let mut poll = [0u64; 6];
    let mut record = [0u64; 8];
    unsafe {
        assert_eq!(fixture_poll_driver(0x1234, poll.as_mut_ptr() as u64, 0), 1);
    }
    let message = POLLED.with(|s| s.borrow().unwrap().ticket.message.unwrap());
    assert_eq!(state().message_world(message).unwrap().epoch, 1);
    unsafe {
        fixture_ctor_driver(record.as_mut_ptr() as u64, 4, 7, 0x5000, 9, 0x35);
    }
    assert!(POLLED.with(|s| s.borrow().is_none()));
    assert_eq!(pending(), Ok(1));
    let id = retire(record.as_mut_ptr() as u64).unwrap();
    release(vec![id]);
    assert_eq!(pending(), Ok(0));
    let old = WorldReceipt {
        binding_id: 8,
        epoch: 1,
    };
    let new = WorldReceipt {
        binding_id: 9,
        epoch: 2,
    };
    state().retire_world(old).unwrap();
    state().loaded_world(l, Some(old), new).unwrap();
    // The same transport still has its old birth World. Its native poll and
    // event construction must continue, without issuing a new-World ticket.
    unsafe {
        assert_eq!(fixture_poll_driver(0x1234, poll.as_mut_ptr() as u64, 0), 1);
        fixture_ctor_driver(record.as_mut_ptr() as u64, 4, 7, 0x5000, 9, 0x35);
    }
    assert!(POLLED.with(|s| s.borrow().is_none()));
    let unknown = records().at(record.as_ptr() as u64).unwrap();
    assert_eq!(unknown.1.message.message, None);
    assert_eq!(pending(), Ok(1));
    NATIVE_INPUT_CONTINUE[2].store(
        fixture_copy_original as *const () as u64 + 5,
        Ordering::Release,
    );
    let mut copy = [0u64; 8];
    unsafe {
        fixture_copy_driver(
            copy.as_mut_ptr().add(1) as u64,
            record.as_ptr().add(1) as u64,
        );
    }
    assert_eq!(records().at(copy.as_ptr() as u64).unwrap().0, unknown.0);
    let id = retire(record.as_ptr() as u64).unwrap();
    release(vec![id]);
    assert_eq!(pending(), Ok(1));
    let id = retire(copy.as_ptr() as u64).unwrap();
    release(vec![id]);
    assert_eq!(pending(), Ok(0));
    assert_eq!(state().reserve_input(l, t), Err(Error::Closed));
}
#[test]
fn shared_free_keeps_old_serial_until_normal_return_despite_address_reuse() {
    if !child("shared_free_keeps_old_serial_until_normal_return_despite_address_reuse") {
        return;
    }
    INPUT_EXE.store(0x140000000, Ordering::Release);
    let ticket = fixture_ticket();
    let mut record = [0u64; 8];
    let p = record.as_mut_ptr() as u64;
    let old = records()
        .issued(
            Receipt {
                message: ticket,
                transport: ticket.transport.incarnation,
            },
            p,
        )
        .unwrap();
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.caller = exe(0x8572eb);
    raw.invocation.caller_rsp = 1;
    raw.gpr[0] = p + 0x20;
    free_begin(&raw);
    assert!(records().at(p).is_none());
    let new_message = fork(ticket.message).unwrap();
    let fresh = records()
        .issued(
            Receipt {
                message: Ticket {
                    message: new_message,
                    ..ticket
                },
                transport: ticket.transport.incarnation,
            },
            p,
        )
        .unwrap();
    assert_ne!(old, fresh);
    free_end(&raw, false);
    assert_eq!(records().at(p).unwrap().0, fresh);
    let id = retire(p).unwrap();
    release(vec![id]);
    assert_eq!(pending(), Ok(0));
}
#[test]
#[ignore = "explicit licensed local primary-image acceptance"]
fn pinned_input_whole_bytes_pdata_and_mutations_match() {
    let mut file = pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
    for f in fixed::FRAMES {
        let offset = file_offset(&mut file, f.begin);
        file.seek(SeekFrom::Start(offset)).unwrap();
        let mut bytes = vec![0u8; (f.end - f.begin) as usize];
        file.read_exact(&mut bytes).unwrap();
        assert!(code_matches(&bytes, f));
        let expected: Vec<u8> = (0..f.specimen.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&f.specimen[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(bytes, expected);
        let end = bytes.len() - 1;
        bytes[end] ^= 1;
        assert!(!code_matches(&bytes, f));
        for p in f.parts {
            let offset = file_offset(&mut file, p.unwind);
            file.seek(SeekFrom::Start(offset)).unwrap();
            let expected: Vec<u8> = (0..p.hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&p.hex[i..i + 2], 16).unwrap())
                .collect();
            let mut b = vec![0; expected.len()];
            file.read_exact(&mut b).unwrap();
            assert_eq!(b, expected);
        }
    }
    for (rva, prefix, _) in SITES {
        let offset = file_offset(&mut file, rva as u32);
        file.seek(SeekFrom::Start(offset)).unwrap();
        let mut b = [0; 8];
        file.read_exact(&mut b).unwrap();
        assert_eq!(b, prefix);
    }
}
fn file_offset(file: &mut File, rva: u32) -> u64 {
    file.seek(SeekFrom::Start(0x3c)).unwrap();
    let mut d = [0; 4];
    file.read_exact(&mut d).unwrap();
    let nt = u64::from(u32::from_le_bytes(d));
    file.seek(SeekFrom::Start(nt + 6)).unwrap();
    let mut w = [0; 2];
    file.read_exact(&mut w).unwrap();
    let n = u16::from_le_bytes(w);
    file.seek(SeekFrom::Start(nt + 20)).unwrap();
    file.read_exact(&mut w).unwrap();
    let opt = u64::from(u16::from_le_bytes(w));
    for i in 0..n {
        file.seek(SeekFrom::Start(nt + 24 + opt + u64::from(i) * 40))
            .unwrap();
        let mut b = [0; 40];
        file.read_exact(&mut b).unwrap();
        let start = u32::from_le_bytes(b[12..16].try_into().unwrap());
        let size = u32::from_le_bytes(b[16..20].try_into().unwrap());
        if rva >= start && rva - start < size {
            return u64::from(u32::from_le_bytes(b[20..24].try_into().unwrap()))
                + u64::from(rva - start);
        }
    }
    panic!("missing RVA{rva:x}")
}
#[test]
fn all_eleven_published_entries_preserve_original_once_and_restore() {
    if !child("all_eleven_published_entries_preserve_original_once_and_restore") {
        return;
    }
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0xd00000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    let mut synthetic = SITES;
    for (i, (rva, prefix, stolen)) in synthetic.iter_mut().enumerate() {
        let p = (base + *rva) as *mut u8;
        unsafe { ptr::copy_nonoverlapping(prefix.as_ptr(), p, 8) };
        let mut tail = vec![0xf0, 0x48, 0xff, 0x81, 0, 0x60, 0, 0, 0x48, 0xb8];
        tail.extend_from_slice(&0x1122334455667788u64.to_le_bytes());
        tail.extend_from_slice(&[0xf9, 0xc3]);
        unsafe {
            ptr::copy_nonoverlapping(tail.as_ptr(), p.add(*stolen), tail.len());
        }
        *prefix = copy_current::<8>((base + *rva) as u64).unwrap();
        assert!(i < 11);
    }
    for (rva, _, _) in SITES {
        let mut old = 0;
        assert_ne!(
            unsafe {
                VirtualProtect(
                    (base + (rva & !0xfff)) as _,
                    0x1000,
                    PAGE_EXECUTE_READ,
                    &mut old,
                )
            },
            0
        );
    }
    unsafe { install_at(base, &synthetic) }.unwrap();
    let mut object = vec![0u64; 0x8000 / 8];
    let p = object.as_mut_ptr() as u64;
    object[0] = p;
    object[1] = p;
    for (rva, _, _) in SITES {
        let call: unsafe extern "system" fn(u64, u64, u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(base + rva) };
        let before = object[0x6000 / 8];
        unsafe {
            SetLastError(0x112233);
            assert_eq!(call(p, p, 0, 0, 9, 0x35), 0x1122334455667788);
        };
        assert_eq!(unsafe { GetLastError() }, 0x112233);
        assert_eq!(object[0x6000 / 8], before + 1);
    }
    unsafe { restore() }.unwrap();
    for (rva, prefix, _) in synthetic {
        assert_eq!(copy_current::<8>((base + rva) as u64), Some(prefix));
    }
    assert!(INPUT_HOOKS.lock().unwrap().is_empty());
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
}
#[test]
fn home_shims_virtual_unwind_restore_original_return_and_saved_register() {
    let sites = [
        (input_poll_original as *const (), 8, 0),
        (input_ctor_original as *const (), 8, 1),
        (input_copy_original as *const (), 8, 0),
        (input_assign_original as *const (), 0x20, 0),
        (input_extract_original as *const (), 8, 0),
        (input_vector_original as *const (), 0x10, 0),
        (input_growth_original as *const (), 0x20, 0),
    ];
    for (addr, offset, reg) in sites {
        let mut stack = [0u64; 64];
        let entry = unsafe { stack.as_mut_ptr().add(20) } as u64;
        unsafe {
            *(entry as *mut u64) = 0xabcdef;
            *((entry + offset) as *mut u64) = 0xb1;
        };
        let mut c: CONTEXT = unsafe { std::mem::zeroed() };
        c.Rsp = entry;
        c.Rip = addr as u64 + 5;
        let mut base = 0;
        let row = unsafe { RtlLookupFunctionEntry(c.Rip, &mut base, null_mut()) };
        assert!(!row.is_null());
        let mut data = null_mut();
        let mut frame = 0;
        unsafe {
            RtlVirtualUnwind(
                0,
                base,
                c.Rip,
                row,
                &mut c,
                &mut data,
                &mut frame,
                null_mut(),
            );
        };
        assert_eq!(c.Rip, 0xabcdef);
        assert_eq!(c.Rsp, entry + 8);
        assert_eq!(if reg == 0 { c.Rbx } else { c.Rsi }, 0xb1);
    }
}

static UNWIND_STATE: AtomicU64 = AtomicU64::new(0);
global_asm!(
".globl input_unwind_driver",".seh_proc input_unwind_driver","input_unwind_driver:","push rbx",".seh_pushreg rbx","sub rsp,0x80",".seh_stackalloc 0x80",".seh_endprologue","mov rbx,rcx","mov [rcx],rsp","lea rax,[rip+input_unwind_landed]","mov [rcx+8],rax","mov rcx,r8","call rdx","xor eax,eax","jmp 2f","input_unwind_landed:","mov eax,1","2:","lea rsp,[rsp+0x80]","pop rbx","ret",".seh_endproc",
".globl input_unwind_original",".seh_proc input_unwind_original","input_unwind_original:","mov [rsp+8],rbx",".seh_savereg rbx,8","sub rsp,0x528",".seh_stackalloc 0x528",".seh_endprologue","mov r10,[rip+{state}]","mov rcx,[r10]","mov rdx,[r10+8]","lea r10,[rsp+0x40]","mov r11d,0x9a","1:","mov qword ptr [r10],0","lea r10,[r10+8]","dec r11d","jnz 1b","lea r10,[rsp+0x40]","mov [rsp+0x20],r10","mov qword ptr [rsp+0x28],0","xor r8d,r8d","xor r9d,r9d","call {unwind_ex}","int3",".seh_endproc",
state=sym UNWIND_STATE,unwind_ex=sym RtlUnwindEx,
);
unsafe extern "system" {
    fn input_unwind_driver(state: *mut u64, target: usize, argument: u64) -> u64;
    fn input_unwind_original();
}
#[test]
fn real_storage_unwind_preserves_unresolved_and_unknown_original_semantics() {
    if !child("real_storage_unwind_preserves_unresolved_and_unknown_original_semantics") {
        return;
    }
    let ticket = fixture_ticket();
    let mut record = [0u64; 8];
    let p = record.as_mut_ptr() as u64;
    // A whole growth frame retains its native vector while nested free/copy may
    // fail. The original SEH is unwound by Windows into the owning native frame.
    let mut vector = [p, p + 0x40, p + 0x40, 0];
    let mut landing = [0u64; 2];
    UNWIND_STATE.store(landing.as_mut_ptr() as u64, Ordering::Release);
    NATIVE_INPUT_CONTINUE[9].store(
        input_unwind_original as *const () as u64 + 5,
        Ordering::Release,
    );
    assert_eq!(
        unsafe {
            input_unwind_driver(
                landing.as_mut_ptr(),
                gateways()[9],
                vector.as_mut_ptr() as u64,
            )
        },
        1
    );
    assert_eq!(pending(), Ok(0));
    assert!(INPUT_CALLS.with(|s| s.borrow().is_empty()));
    records()
        .issued(
            Receipt {
                message: ticket,
                transport: ticket.transport.incarnation,
            },
            p,
        )
        .unwrap();
    assert_eq!(
        unsafe {
            input_unwind_driver(
                landing.as_mut_ptr(),
                gateways()[9],
                vector.as_mut_ptr() as u64,
            )
        },
        1
    );
    assert_eq!(pending(), Err("input unresolved"));
    assert!(INPUT_CALLS.with(|s| s.borrow().is_empty()));
}
#[test]
fn timeout_null_and_receiver_end_cancel_pending_without_new_authority() {
    if !child("timeout_null_and_receiver_end_cancel_pending_without_new_authority") {
        return;
    }
    let ticket = fixture_ticket();
    POLLED.with(|s| {
        *s.borrow_mut() = Some(Pending {
            receiver: (6, 7),
            ticket,
            bytes: 0,
            len: 0,
            peer: 0,
            flags: 0,
        })
    });
    receiver_returned(6, 8, false);
    assert!(POLLED.with(|s| s.borrow().is_some()));
    receiver_returned(6, 7, false);
    assert!(POLLED.with(|s| s.borrow().is_none()));
    assert_eq!(
        state().message_world(ticket.message.unwrap()),
        Err(Error::Unknown)
    );
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 111;
    raw.gpr[0] = 0;
    begin(&mut raw);
    raw.gpr[4] = 0;
    finish(&mut raw, false);
    assert_eq!(pending(), Ok(0));
}

extern "system" fn zero_growth_body(vector: u64, _: u64, _: u64) {
    let source = value(vector);
    let dest = DEST.load(Ordering::Acquire);
    let mut end = 0u64;
    unsafe {
        fixture_range_driver(&mut end as *mut u64 as u64, source, source + 0x40, dest);
        *(vector as *mut u64) = dest;
        *((vector + 8) as *mut u64) = end;
    }
}
global_asm!(
".globl zero_growth_original",".seh_proc zero_growth_original","zero_growth_original:","mov [rsp+0x20],rbx",".seh_savereg rbx,0x20",".seh_endprologue","jmp {body}",".seh_endproc",body=sym zero_growth_body,
);
unsafe extern "system" {
    fn zero_growth_original();
}
#[test]
fn zero_byte_growth_retires_old_location_without_a_native_free_call() {
    if !child("zero_byte_growth_retires_old_location_without_a_native_free_call") {
        return;
    }
    FLOW.store(true, Ordering::Release);
    let t = fixture_ticket();
    let mut old = [0u64; 8];
    let mut new = [0u64; 8];
    let source = old.as_mut_ptr() as u64;
    let dest = new.as_mut_ptr() as u64;
    let mut vector = [source, source + 0x40, source + 0x40, 0];
    let id = records()
        .issued(
            Receipt {
                message: t,
                transport: t.transport.incarnation,
            },
            source,
        )
        .unwrap();
    DEST.store(dest, Ordering::Release);
    NATIVE_INPUT_CONTINUE[4].store(
        fixture_range_original as *const () as u64 + 5,
        Ordering::Release,
    );
    NATIVE_INPUT_CONTINUE[9].store(
        zero_growth_original as *const () as u64 + 5,
        Ordering::Release,
    );
    let call: unsafe extern "system" fn(u64, u64, u64) =
        unsafe { std::mem::transmute(gateways()[9]) };
    unsafe {
        call(vector.as_mut_ptr() as u64, source, source);
    }
    assert!(records().at(source).is_none());
    assert_eq!(records().at(dest).unwrap().0, id);
    assert_eq!(pending(), Ok(1));
    let retired = retire(dest).unwrap();
    release(vec![retired]);
    assert_eq!(pending(), Ok(0));
}
