use super::*;
use std::os::windows::process::CommandExt;
use std::ptr::null;
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlLookupFunctionEntry, IMAGE_RUNTIME_FUNCTION_ENTRY,
};
static FIXTURE: AtomicBool = AtomicBool::new(false);
static NETWORK: AtomicU64 = AtomicU64::new(0);
static POINTER: AtomicU64 = AtomicU64::new(0);
static THREAD: AtomicU64 = AtomicU64::new(0);
static START_OK: AtomicBool = AtomicBool::new(true);
static OBSERVED: AtomicU64 = AtomicU64::new(0);
static HIT: AtomicU64 = AtomicU64::new(0);
static EXTRA: AtomicU64 = AtomicU64::new(99);
pub(super) fn caller_override(rva: u64) -> Option<u64> {
    if !FIXTURE.load(Ordering::Acquire) {
        return None;
    }
    match rva {
        0x83fc63 => Some(transport_fixture_factory_return as *const () as u64),
        0x84e539 => Some(transport_fixture_network_return as *const () as u64),
        0x84e551 => Some(transport_fixture_start_return as *const () as u64),
        0x85739b => Some(transport_fixture_creator_return as *const () as u64),
        0x98c0d1 => Some(transport_fixture_thread_return as *const () as u64),
        _ => None,
    }
}
fn child(name: &str) -> bool {
    const ENV: &str = "CIV6_NATIVE_TRANSPORT_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    let mut p = std::process::Command::new(std::env::current_exe().unwrap())
        .creation_flags(0x08000000)
        .env(ENV, "1")
        .args([
            "--exact",
            &format!(
                "windows::root_observation::native_frames::sessions::transports::tests::{name}"
            ),
            "--nocapture",
        ])
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(s) = p.try_wait().unwrap() {
            assert!(s.success(), "transport child{s}");
            return false;
        }
        if Instant::now() >= deadline {
            let _ = p.kill();
            let _ = p.wait();
            panic!("transport fixture timeout")
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
extern "system" fn factory_body(session: u64) -> u64 {
    let p = POINTER.load(Ordering::Acquire);
    assert_eq!(value(session + 0x28), 0);
    unsafe {
        *(p as *mut u64) = exe(0x1047e60);
        *((p + 8) as *mut u64) = value(session + 0x30);
        *((p + 0x40) as *mut u32) = 1;
        *((p + 0x45) as *mut u8) = 1;
    }
    p
}
extern "system" fn network_body(network: u64) -> u64 {
    assert_eq!(network, NETWORK.load(Ordering::Acquire));
    unsafe { *(network as *mut u64) = exe(0x1049808) }
    network
}
extern "system" fn thread_body(thread: u64) -> u64 {
    assert_eq!(thread, THREAD.load(Ordering::Acquire));
    unsafe { *(thread as *mut u64) = exe(0x106d030) }
    thread
}
extern "system" fn before_start(functor: u64, thread: u64, extra: u64) -> u64 {
    EXTRA.store(extra, Ordering::Release);
    let id = parent(106).unwrap();
    let (lease, session, _, t) = state().snapshot(id).unwrap();
    assert_eq!(thread, t.thread);
    assert_eq!(value(functor), t.network);
    assert_eq!(super::super::state().transport(lease), Err(Error::Unbound));
    let mut bits = u64::from(value(t.network + 0xc0) == 0);
    bits |= u64::from(value(session + 0x28) == t.pointer) << 1;
    bits |= u64::from(value(t.pointer + 0x10) == t.network) << 2;
    bits |= u64::from(unsafe { GetLastError() } == 0x1234) << 3;
    OBSERVED.store(bits, Ordering::Release);
    if !START_OK.load(Ordering::Acquire) {
        return 0;
    }
    unsafe {
        *((thread + 8) as *mut u64) = value(functor);
        *((thread + 0x10) as *mut u64) = value(functor + 8);
        *((thread + 0xb0) as *mut u64) = 0x100;
    }
    // A receive frame can run BEFORE the creator returns and C0 is stored.
    unsafe { transport_fixture_receive(value(functor)) };
    thread
}
extern "system" fn receive_body(network: u64) -> u64 {
    let id = CALLS
        .with(|s| s.borrow().last().and_then(|c| c.birth))
        .unwrap();
    let (lease, _, _, t) = state().snapshot(id).unwrap();
    assert_eq!(network, t.network);
    assert_eq!(super::super::state().transport(lease), Err(Error::Unbound));
    // No packet authority is created at a late raw callback.
    assert!(super::super::NativeSessionApi::current_world().is_none());
    HIT.fetch_add(1, Ordering::AcqRel);
    unsafe { SetLastError(0x1234) };
    0x8899
}
extern "system" fn unused_body() -> u64 {
    0
}
global_asm!(
    ".p2align 4",".globl transport_fixture_driver",".seh_proc transport_fixture_driver","transport_fixture_driver:",
    "push rbx",".seh_pushreg rbx","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "mov rbx,rcx","call {factory}",".globl transport_fixture_factory_return","transport_fixture_factory_return:",
    "mov [rbx+0x28],rax","mov rcx,rax","mov edx,0x1234","call {local}",
    "lea rsp,[rsp+0x20]","pop rbx","ret",".seh_endproc",
    ".p2align 4",".globl transport_fixture_factory_a",".seh_proc transport_fixture_factory_a","transport_fixture_factory_a:",
    ".byte 0x40,0x53",".seh_pushreg rbx","sub rsp,0x30",".seh_stackalloc 0x30",".seh_endprologue",
    "call {unused}","lea rsp,[rsp+0x30]","pop rbx","ret",".seh_endproc",
    ".p2align 4",".globl transport_fixture_factory_b",".seh_proc transport_fixture_factory_b","transport_fixture_factory_b:",
    ".byte 0x40,0x53",".seh_pushreg rbx","sub rsp,0x30",".seh_stackalloc 0x30",".seh_endprologue",
    "call {unused}","lea rsp,[rsp+0x30]","pop rbx","ret",".seh_endproc",
    ".p2align 4",".globl transport_fixture_factory_c",".seh_proc transport_fixture_factory_c","transport_fixture_factory_c:",
    ".byte 0x40,0x53",".seh_pushreg rbx","sub rsp,0x30",".seh_stackalloc 0x30",".seh_endprologue",
    "call {factory_body}","lea rsp,[rsp+0x30]","pop rbx","ret",".seh_endproc",
    ".p2align 4",".globl transport_fixture_local",".seh_proc transport_fixture_local","transport_fixture_local:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","push rdi",".seh_pushreg rdi","sub rsp,0x30",".seh_stackalloc 0x30",".seh_endprologue",
    "mov rbx,rcx","mov rcx,[rip+{network}]","call {network_ctor}",".globl transport_fixture_network_return","transport_fixture_network_return:",
    "mov [rbx+0x10],rax","mov rcx,rax","call {start}",".globl transport_fixture_start_return","transport_fixture_start_return:",
    "mov eax,1","mov rbx,[rsp+0x40]","lea rsp,[rsp+0x30]","pop rdi","ret",".seh_endproc",
    ".p2align 4",".globl transport_fixture_network",".seh_proc transport_fixture_network","transport_fixture_network:",
    "mov [rsp+0x10],rbx",".seh_savereg rbx,0x10","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "call {network_body}","mov rbx,[rsp+0x38]","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".p2align 4",".globl transport_fixture_start",".seh_proc transport_fixture_start","transport_fixture_start:",
    ".byte 0x40,0x53",".seh_pushreg rbx","sub rsp,0x40",".seh_stackalloc 0x40",".seh_endprologue",
    "mov rbx,rcx","mov [rsp+0x30],rcx","mov rax,[rip+{callback}]","mov [rsp+0x38],rax",
    "lea rcx,[rsp+0x30]","xor edx,edx","xor r8d,r8d","xor r9d,r9d","mov qword ptr [rsp+0x20],0","call {creator}",
    ".globl transport_fixture_creator_return","transport_fixture_creator_return:",
    "mov [rbx+0xc0],rax","lea rsp,[rsp+0x40]","pop rbx","ret",".seh_endproc",
    ".p2align 4",".globl transport_fixture_creator",".seh_proc transport_fixture_creator","transport_fixture_creator:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","push rdi",".seh_pushreg rdi","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "mov rbx,rcx","mov rdi,[rsp+0x50]","mov rcx,[rip+{thread}]","call {thread_ctor}",
    ".globl transport_fixture_thread_return","transport_fixture_thread_return:",
    "mov rcx,rbx","mov rdx,rax","mov r8,rdi","call {before_start}",
    "mov rbx,[rsp+0x30]","lea rsp,[rsp+0x20]","pop rdi","ret",".seh_endproc",
    ".p2align 4",".globl transport_fixture_thread",".seh_proc transport_fixture_thread","transport_fixture_thread:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "call {thread_body}","mov rbx,[rsp+0x30]","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".p2align 4",".globl transport_fixture_receive",".seh_proc transport_fixture_receive","transport_fixture_receive:",
    "mov [rsp+0x10],rbx",".seh_savereg rbx,0x10","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "call {receive_body}","mov rbx,[rsp+0x38]","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    factory=sym transport_fixture_factory_c,local=sym transport_fixture_local,factory_body=sym factory_body,
    network=sym NETWORK,network_ctor=sym transport_fixture_network,network_body=sym network_body,
    start=sym transport_fixture_start,creator=sym transport_fixture_creator,callback=sym FIXTURE_CALLBACK,
    thread=sym THREAD,thread_ctor=sym transport_fixture_thread,thread_body=sym thread_body,
    before_start=sym before_start,receive_body=sym receive_body,unused=sym unused_body,
);
#[unsafe(no_mangle)]
static FIXTURE_CALLBACK: AtomicU64 = AtomicU64::new(0);
unsafe extern "system" {
    fn transport_fixture_driver(session: u64) -> u64;
    fn transport_fixture_factory_a();
    fn transport_fixture_factory_b();
    fn transport_fixture_factory_c();
    fn transport_fixture_local();
    fn transport_fixture_network();
    fn transport_fixture_start();
    fn transport_fixture_creator();
    fn transport_fixture_thread();
    fn transport_fixture_receive(network: u64) -> u64;
    fn transport_fixture_factory_return();
    fn transport_fixture_network_return();
    fn transport_fixture_start_return();
    fn transport_fixture_creator_return();
    fn transport_fixture_thread_return();
}
fn allocate() -> *mut u8 {
    let p = unsafe { VirtualAlloc(null(), 0x20000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
        as *mut u8;
    assert!(!p.is_null());
    p
}
fn setup() -> (u64, SessionLease, [usize; 9], Vec<*mut u8>) {
    let tables = unsafe { VirtualAlloc(null(), 0x40000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
        as *mut u8;
    assert!(!tables.is_null());
    let base = tables as u64 - 0x1040000;
    EXE.store(base, Ordering::Release);
    super::super::EXE.store(base, Ordering::Release);
    let session = allocate() as u64;
    let callback = allocate() as u64;
    let p = allocate() as u64;
    let n = allocate() as u64;
    let t = allocate() as u64;
    unsafe {
        *(session as *mut u64) = exe(0x1045bd8);
        *((session + 0x30) as *mut u64) = callback;
        *((exe(0x1045bd8) + 0x408) as *mut u64) = exe(0x83ade0);
        *((exe(0x106d030) + 8) as *mut u64) = exe(0x98c170);
    }
    POINTER.store(p, Ordering::Release);
    NETWORK.store(n, Ordering::Release);
    THREAD.store(t, Ordering::Release);
    FIXTURE_CALLBACK.store(exe(0x8570b0), Ordering::Release);
    let lease = {
        let mut s = super::super::state();
        let l = s.birth(0x100, Some(1), 0x200, 1).unwrap();
        s.factory(l, 17, 0x200, session, callback).unwrap();
        s.published(l, 17).unwrap();
        l
    };
    let addresses = [
        transport_fixture_factory_a as *const () as usize,
        transport_fixture_factory_b as *const () as usize,
        transport_fixture_factory_c as *const () as usize,
        transport_fixture_local as *const () as usize,
        transport_fixture_network as *const () as usize,
        transport_fixture_start as *const () as usize,
        transport_fixture_creator as *const () as usize,
        transport_fixture_thread as *const () as usize,
        transport_fixture_receive as *const () as usize,
    ];
    let sites = std::array::from_fn(|i| {
        (
            addresses[i],
            copy_current::<8>(addresses[i] as u64).unwrap(),
            SITES[i].2,
        )
    });
    unsafe { install_at(0, &sites).unwrap() };
    EXE.store(base, Ordering::Release);
    FIXTURE.store(true, Ordering::Release);
    (
        session,
        lease,
        addresses,
        vec![
            tables,
            session as *mut u8,
            callback as *mut u8,
            p as *mut u8,
            n as *mut u8,
            t as *mut u8,
        ],
    )
}
#[test]
fn actual_gateway_binds_thread_before_start_then_publishes_before_c0_and_disconnects_only_transport(
) {
    if !child("actual_gateway_binds_thread_before_start_then_publishes_before_c0_and_disconnects_only_transport"){return}
    let (session, lease, _, owned) = setup();
    unsafe { SetLastError(0x1234) };
    assert_eq!(unsafe { transport_fixture_driver(session) }, 1);
    assert_eq!(unsafe { GetLastError() }, 0x1234);
    assert_eq!(OBSERVED.load(Ordering::Acquire), 15);
    assert_eq!(HIT.load(Ordering::Acquire), 1);
    assert_eq!(EXTRA.load(Ordering::Acquire), 0);
    assert_eq!(
        census::event_count(census::SemanticEvent::TransportPublished),
        1
    );
    let t = super::super::state().transport(lease).unwrap();
    assert_eq!(value(t.network + 0xc0), t.thread);
    assert_eq!(from_network(t.network).unwrap(), Some((lease, t)));
    let Disconnect::Published(a, b) = disconnect_begin(session).unwrap() else {
        panic!("published transport close missing")
    };
    let pair = (a, b);
    assert_eq!(pair, (lease, t));
    assert_eq!(join_returned(lease, t, t.thread, 0), Err(Error::NotJoined));
    assert_eq!(disconnect_end(lease, t, false), Err(Error::NotJoined));
    join_returned(lease, t, t.thread, 1).unwrap();
    disconnect_end(lease, t, false).unwrap();
    assert_eq!(
        super::super::existing_transport_owner(session, value(session + 0x30)).unwrap(),
        Some(lease)
    );
    unsafe { restore().unwrap() };
    for p in owned {
        unsafe { VirtualFree(p.cast(), 0, MEM_RELEASE) };
    }
}
#[test]
fn nine_actual_cas_entries_restore_original_bytes_and_rx_protection() {
    if !child("nine_actual_cas_entries_restore_original_bytes_and_rx_protection") {
        return;
    }
    let (_, _, addresses, owned) = setup();
    let originals: Vec<_> = HOOKS
        .lock()
        .unwrap()
        .iter()
        .map(|h| (h.hook.target as u64, h.hook.original, h.protection))
        .collect();
    assert_eq!(originals.len(), 9);
    for (i, (address, original, protection)) in originals.iter().enumerate() {
        assert_eq!(*address, addresses[i] as u64);
        assert_ne!(copy_current::<8>(*address), Some(*original));
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        assert_ne!(
            unsafe {
                VirtualQuery(
                    (*address as *const u8).cast(),
                    &mut info,
                    std::mem::size_of_val(&info),
                )
            },
            0
        );
        assert_eq!(info.Protect, *protection);
        assert_eq!(info.Protect & 0xff, PAGE_EXECUTE_READ);
    }
    unsafe { restore().unwrap() };
    for (address, original, protection) in originals {
        assert_eq!(copy_current::<8>(address), Some(original));
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        assert_ne!(
            unsafe {
                VirtualQuery(
                    (address as *const u8).cast(),
                    &mut info,
                    std::mem::size_of_val(&info),
                )
            },
            0
        );
        assert_eq!(info.Protect, protection);
    }
    assert!(HOOKS.lock().unwrap().is_empty());
    for p in owned {
        unsafe { VirtualFree(p.cast(), 0, MEM_RELEASE) };
    }
}
#[test]
fn failed_native_start_preserves_null_and_never_publishes_a_transport() {
    if !child("failed_native_start_preserves_null_and_never_publishes_a_transport") {
        return;
    }
    let (session, lease, _, owned) = setup();
    START_OK.store(false, Ordering::Release);
    unsafe { transport_fixture_driver(session) };
    assert_eq!(value(NETWORK.load(Ordering::Acquire) + 0xc0), 0);
    assert_eq!(super::super::state().transport(lease), Err(Error::Unbound));
    assert_eq!(HIT.load(Ordering::Acquire), 0);
    unsafe { restore().unwrap() };
    for p in owned {
        unsafe { VirtualFree(p.cast(), 0, MEM_RELEASE) };
    }
}
extern "system" fn unpublished_close_original(session: u64) -> u64 {
    assert_eq!(unsafe { GetLastError() }, 0x87654321);
    HIT.fetch_add(1, Ordering::AcqRel);
    unsafe {
        *((session + 0x28) as *mut u64) = 0;
    }
    assert_ne!(
        unsafe { VirtualFree(POINTER.load(Ordering::Acquire) as *mut _, 0, MEM_RELEASE) },
        0
    );
    unsafe { SetLastError(0x02468ace) };
    0x1122334455667788
}
#[test]
fn failed_start_close_executes_original_without_join_certificate_or_frame_poison() {
    if !child("failed_start_close_executes_original_without_join_certificate_or_frame_poison") {
        return;
    }
    unproven_close_gateway_case(true);
}
#[test]
fn inactive_published_close_executes_original_without_inventing_a_join() {
    if child("inactive_published_close_executes_original_without_inventing_a_join") {
        unproven_close_gateway_case(false);
    }
}
fn unproven_close_gateway_case(failed_start: bool) {
    let frames = FRAMES.get_or_init(NativeFrames::new);
    let (session, lease, _, owned) = setup();
    unsafe {
        *((exe(0x1047e60) + 0x40) as *mut u64) = exe(0x84ebd0);
        *((exe(0x1047e60) + 0x58) as *mut u64) = exe(0x84ec00);
        *(exe(0x106d060) as *mut u64) = exe(0x98c650);
    }
    START_OK.store(!failed_start, Ordering::Release);
    unsafe { transport_fixture_driver(session) };
    if !failed_start {
        assert_eq!(
            super::super::snapshot_transport(lease),
            Ok(()),
            "active snapshot is a positive control before the flag changes"
        );
        unsafe {
            *((POINTER.load(Ordering::Acquire) + 0x45) as *mut u8) = 0;
        }
    }
    HIT.store(0, Ordering::Release);
    super::super::NATIVE_SESSION_CONTINUE[4].store(
        unpublished_close_original as *const () as u64,
        Ordering::Release,
    );
    let close: unsafe extern "system" fn(u64) -> u64 =
        unsafe { std::mem::transmute(super::super::native_session_close_gateway as *const ()) };
    unsafe { SetLastError(0x87654321) };
    let result = unsafe { close(session) };
    let error = unsafe { GetLastError() };
    let calls = HIT.load(Ordering::Acquire);
    let transport = super::super::state().transport(lease);
    let capture = from_network(NETWORK.load(Ordering::Acquire));
    let healthy = frames.healthy();
    super::super::NATIVE_SESSION_CONTINUE[4].store(0, Ordering::Release);
    unsafe { restore().unwrap() };
    for p in owned {
        unsafe { VirtualFree(p.cast(), 0, MEM_RELEASE) };
    }
    assert_eq!(result, 0x1122334455667788);
    assert_eq!(error, 0x02468ace);
    assert_eq!(calls, 1);
    assert_eq!(transport, Err(Error::Unbound));
    assert_eq!(capture, Ok(None));
    assert!(healthy && !FAILED.load(Ordering::Acquire));
    assert_eq!(
        census::event_count(census::SemanticEvent::CloseFlag0),
        u64::from(!failed_start)
    );
    assert_eq!(
        census::event_count(census::SemanticEvent::CloseFlag1),
        u64::from(failed_start)
    );
    assert_eq!(
        census::event_count(census::SemanticEvent::CloseUnpublished),
        u64::from(failed_start)
    );
    assert_eq!(
        census::event_count(census::SemanticEvent::TransportPublished),
        u64::from(!failed_start)
    );
    let mut later: Saved = unsafe { std::mem::zeroed() };
    super::super::super::begin(&mut later);
    super::super::super::finish(&mut later, false);
    assert_eq!(frames.observation_state_for_test().2, 0);
}
#[test]
fn staged_birth_rejects_wrong_receiver_duplicate_thread_and_receive_active_join() {
    let mut s = Ledger::default();
    let lease = SessionLease(123);
    let id = s.factory(lease, 1, 2).unwrap();
    assert_eq!(s.transport(id, 3, 9), Err(Error::Stale));
    s.transport(id, 3, 2).unwrap();
    s.network(id, 4).unwrap();
    assert_eq!(s.publish(id, 5), Err(Error::Stale));
    s.thread(id, 5).unwrap();
    assert_eq!(s.thread(id, 5), Err(Error::Stale));
    let t = s.publish(id, 5).unwrap();
    let entered = s.enter(4).unwrap().unwrap();
    let _ = s.close(1).unwrap();
    assert_eq!(s.joined(lease, t, 5, 1), Err(Error::Busy));
    s.leave(entered).unwrap();
    assert_eq!(s.joined(lease, t, 6, 1), Err(Error::Stale));
    s.joined(lease, t, 5, 1).unwrap();
    s.closed(lease, t).unwrap();
    assert_eq!(s.enter(4), Ok(None));
    let second = s.factory(lease, 1, 2).unwrap();
    assert_ne!(second, id);
}
#[test]
fn unproven_close_retires_only_after_original_return_and_never_implies_join() {
    let mut s = Ledger::default();
    let lease = SessionLease(123);
    let first = s.factory(lease, 1, 2).unwrap();
    s.transport(first, 3, 2).unwrap();
    s.network(first, 4).unwrap();
    let t = s.snapshot(first).unwrap().3;
    s.unproven_begin(first).unwrap();
    assert!(!s.capture_allowed(1));
    s.unproven_end(first, lease, t, false).unwrap();
    assert!(s.capture_allowed(1));
    assert!(s.snapshot(first).is_err());
    assert!(s.joined(lease, t, 0, 1).is_err());
    assert_eq!(s.closed(lease, t), Err(Error::NotJoined));
    let second = s.factory(lease, 1, 2).unwrap();
    assert_ne!(first, second);
    s.transport(second, 3, 2).unwrap();
    s.network(second, 4).unwrap();
    s.thread(second, 5).unwrap();
    let t = s.publish(second, 5).unwrap();
    s.close(1).unwrap();
    s.joined(lease, t, 5, 1).unwrap();
    s.closed(lease, t).unwrap();
    assert!(
        s.has_unproven_close(),
        "a later valid lifetime cannot erase an earlier coverage gap"
    );
}
#[test]
fn unproven_close_exception_retains_uncertainty_without_poisoning_other_births() {
    let mut s = Ledger::default();
    let lease = SessionLease(123);
    let id = s.factory(lease, 1, 2).unwrap();
    s.transport(id, 3, 2).unwrap();
    let t = s.snapshot(id).unwrap().3;
    s.unproven_begin(id).unwrap();
    s.unproven_end(id, lease, t, true).unwrap();
    assert!(!s.capture_allowed(1));
    assert!(s.has_unproven_close());
    assert!(s.factory(SessionLease(456), 10, 20).is_ok());
}
#[test]
fn unknown_receiver_has_no_birth_or_world_authority() {
    let mut s = Ledger::default();
    assert_eq!(s.enter(0x111), Ok(None));
    assert_eq!(s.pointer_birth(0x123), None);
    assert_eq!(s.network_birth(0x456), None);
}
#[test]
fn all_static_transport_original_shims_have_unwind_records() {
    let shims = [
        transport_factory_a_original as *const () as u64,
        transport_factory_b_original as *const () as u64,
        transport_factory_c_original as *const () as u64,
        transport_local_original as *const () as u64,
        transport_network_original as *const () as u64,
        transport_start_original as *const () as u64,
        transport_creator_original as *const () as u64,
        transport_thread_original as *const () as u64,
        transport_receive_original as *const () as u64,
    ];
    for pc in shims {
        let mut base = 0;
        assert!(!unsafe { RtlLookupFunctionEntry(pc, &mut base, null_mut()) }.is_null());
    }
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
unsafe extern "system" {
    fn transport_factory_a_original_body();
    fn transport_factory_b_original_body();
    fn transport_factory_c_original_body();
    fn transport_local_original_body();
    fn transport_network_original_body();
    fn transport_start_original_body();
    fn transport_creator_original_body();
    fn transport_thread_original_body();
    fn transport_receive_original_body();
    fn transport_unwind_driver(state: *mut u64, target: usize, argument: u64) -> u64;
    fn transport_unwind_receiver();
}
#[test]
fn nine_displaced_shims_restore_original_return_rsp_and_nonvolatile_registers() {
    let shims = [
        transport_factory_a_original_body as *const () as u64,
        transport_factory_b_original_body as *const () as u64,
        transport_factory_c_original_body as *const () as u64,
        transport_local_original_body as *const () as u64,
        transport_network_original_body as *const () as u64,
        transport_start_original_body as *const () as u64,
        transport_creator_original_body as *const () as u64,
        transport_thread_original_body as *const () as u64,
        transport_receive_original_body as *const () as u64,
    ];
    for (i, pc) in shims.into_iter().enumerate() {
        let mut stack = [0u64; 64];
        let entry = unsafe { stack.as_mut_ptr().add(32) } as u64;
        unsafe {
            *(entry as *mut u64) = 0xabcdef;
        }
        let mut c: CONTEXT = unsafe { std::mem::zeroed() };
        c.Rip = pc;
        c.Rsp = entry;
        if [0, 1, 2, 5].contains(&i) {
            unsafe {
                *((entry - 8) as *mut u64) = 0xb1;
            }
            c.Rsp -= if i == 5 { 0x48 } else { 0x38 };
        } else {
            let offset = if [4, 8].contains(&i) { 0x10 } else { 8 };
            unsafe {
                *((entry + offset) as *mut u64) = 0xb1;
            }
        }
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
        }
        assert_eq!(
            (c.Rip, c.Rsp, c.Rbx),
            (0xabcdef, entry + 8, 0xb1),
            "shim{i}"
        );
    }
}
static UNWIND_STATE: AtomicU64 = AtomicU64::new(0);
global_asm!(
    ".globl transport_unwind_driver",".seh_proc transport_unwind_driver","transport_unwind_driver:",
    "push rbx",".seh_pushreg rbx","sub rsp,0x80",".seh_stackalloc 0x80",".seh_endprologue",
    "mov rbx,rcx","mov [rcx],rsp","lea rax,[rip+transport_unwind_landed]","mov [rcx+8],rax","mov rcx,r8","call rdx","xor eax,eax","jmp 2f",
    "transport_unwind_landed:","mov eax,1","2:","lea rsp,[rsp+0x80]","pop rbx","ret",".seh_endproc",
    ".p2align 4",".globl transport_unwind_receiver",".seh_proc transport_unwind_receiver","transport_unwind_receiver:",
    "mov [rsp+0x10],rbx",".seh_savereg rbx,0x10","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "mov rcx,[rip+{state}]","call transport_unwind_body","int3",".seh_endproc",
    ".globl transport_unwind_body",".seh_proc transport_unwind_body","transport_unwind_body:",
    "sub rsp,0x528",".seh_stackalloc 0x528",".seh_endprologue","mov rdx,[rcx+8]","mov rcx,[rcx]","lea r10,[rsp+0x40]","mov r11d,0x9a",
    "1:","mov qword ptr [r10],0","lea r10,[r10+8]","dec r11d","jnz 1b",
    "lea r10,[rsp+0x40]","mov [rsp+0x20],r10","mov qword ptr [rsp+0x28],0","xor r8d,r8d","xor r9d,r9d","call {unwind_ex}","int3",".seh_endproc",
    state=sym UNWIND_STATE,unwind_ex=sym RtlUnwindEx,
);
#[test]
fn actual_receive_seh_keeps_native_birth_unresolved_and_cannot_issue_join() {
    if !child("actual_receive_seh_keeps_native_birth_unresolved_and_cannot_issue_join") {
        return;
    }
    let (session, lease, _, _owned) = setup();
    unsafe { transport_fixture_driver(session) };
    let t = super::super::state().transport(lease).unwrap();
    let mut landing = [0u64; 2];
    UNWIND_STATE.store(landing.as_mut_ptr() as u64, Ordering::Release);
    NATIVE_TRANSPORT_CONTINUE[8].store(
        transport_unwind_receiver as *const () as u64 + 5,
        Ordering::Release,
    );
    assert_eq!(
        unsafe { transport_unwind_driver(landing.as_mut_ptr(), gateways()[8], t.network) },
        1
    );
    assert!(CALLS.with(|s| s.borrow().is_empty()));
    assert_eq!(from_network(t.network), Err(Error::Unresolved));
    assert_eq!(
        super::super::existing_transport_owner(session, value(session + 0x30)),
        Err(Error::Unresolved)
    );
    unsafe { restore().unwrap() };
}
#[test]
#[ignore = "explicit licensed local primary-image acceptance"]
fn exact_native_bodies_pdata_prefixes_and_mutated_tails_are_required() {
    let mut file = super::super::super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
    for f in fixed::FRAMES {
        let offset = pe_file_offset(&mut file, f.begin).unwrap();
        file.seek(SeekFrom::Start(offset)).unwrap();
        let mut b = vec![0; (f.end - f.begin) as usize];
        file.read_exact(&mut b).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&b)), f.sha);
        let last = b.len() - 1;
        b[last] ^= 1;
        assert_ne!(format!("{:x}", Sha256::digest(&b)), f.sha);
        assert_eq!(
            super::super::unhex(f.specimen).len(),
            (f.end - f.begin) as usize
        );
    }
    for (rva, prefix, stolen) in SITES {
        assert!(stolen == 5 || stolen == 6);
        let offset = pe_file_offset(&mut file, rva as u32).unwrap();
        file.seek(SeekFrom::Start(offset)).unwrap();
        let mut b = [0u8; 8];
        file.read_exact(&mut b).unwrap();
        assert_eq!(b, prefix);
    }
}
