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
static NETWORK_ALLOC_OK: AtomicBool = AtomicBool::new(true);
static RESET_JOIN_ENABLED: AtomicBool = AtomicBool::new(false);
static RESET_JOIN_AL: AtomicU64 = AtomicU64::new(1);
pub(super) fn reset_join_caller() -> Option<u64> {
    (FIXTURE.load(Ordering::Acquire) && RESET_JOIN_ENABLED.load(Ordering::Acquire))
        .then_some(transport_fixture_reset_join_return as *const () as u64)
}
extern "system" fn reset_join_original(_: u64, _: u64) -> u64 {
    let error = unsafe { GetLastError() };
    let al = RESET_JOIN_AL.load(Ordering::Acquire);
    if std::env::var_os("CIV6_NATIVE_RESET_EXPECT_AL0").is_some() {
        eprintln!("reset-original-join-AL{al}-reached");
    }
    unsafe { SetLastError(error) };
    al
}
extern "system" fn reset_destructive_continuation() {
    if std::env::var_os("CIV6_NATIVE_RESET_EXPECT_AL0").is_some() {
        eprintln!("reset-destructive-continuation-reached");
    }
}
fn enable_reset_join() {
    RESET_JOIN_ENABLED.store(true, Ordering::Release);
    super::super::NATIVE_SESSION_CONTINUE[5]
        .store(reset_join_original as *const () as u64, Ordering::Release);
}
static OBSERVED: AtomicU64 = AtomicU64::new(0);
static HIT: AtomicU64 = AtomicU64::new(0);
static EXTRA: AtomicU64 = AtomicU64::new(99);
static UNQUALIFIED_RECEIVE: AtomicBool = AtomicBool::new(false);
static CHECK_RECEIVE_RETIREMENT: AtomicBool = AtomicBool::new(false);
static RECEIVE_RETIREMENT_OBSERVED: AtomicU64 = AtomicU64::new(0);
type PendingReset = (std::sync::mpsc::Sender<()>, std::thread::JoinHandle<()>);
static CONCURRENT_RESET: Mutex<Option<PendingReset>> = Mutex::new(None);
fn reset_between_poll_lookup_and_reservation(_: SessionLease, transport: SessionTransport) {
    let (ready, started) = std::sync::mpsc::channel();
    let (release, wait) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut raw: Saved = unsafe { std::mem::zeroed() };
        raw.invocation.kind = 103;
        raw.gpr[0] = transport.pointer;
        begin(&mut raw); // Actual reset observer revokes the Session source.
        ready.send(()).unwrap();
        wait.recv_timeout(Duration::from_secs(5)).unwrap();
        // The original reset's join would permit this destruction only after
        // the active receiver returned. This fixture never stops a real thread.
        unsafe {
            *((transport.pointer + 0x10) as *mut u64) = 0;
            *((transport.network + 0xc0) as *mut u64) = 0;
        };
        finish(&mut raw, false);
    });
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    *CONCURRENT_RESET.lock().unwrap() = Some((release, worker));
    println!("RESET_BETWEEN_ACTUAL_POLL_LOOKUP_AND_RESERVATION");
}
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
    if CHECK_RECEIVE_RETIREMENT.load(Ordering::Acquire) {
        let no_storage =
            super::super::input::pending() == Ok(0) && super::super::deferred::pending() == Ok(0);
        let refused = super::super::retire_world(SessionWorldReceipt {
            binding_id: 1,
            epoch: 1,
        }) == Err(Error::Busy);
        RECEIVE_RETIREMENT_OBSERVED.store(
            u64::from(no_storage) | (u64::from(refused) << 1),
            Ordering::Release,
        );
        HIT.fetch_add(1, Ordering::AcqRel);
        unsafe { SetLastError(0x1234) };
        return 0x8899;
    }
    if UNQUALIFIED_RECEIVE.load(Ordering::Acquire) {
        let birth = CALLS.with(|calls| calls.borrow().last().and_then(|call| call.birth));
        OBSERVED.store(u64::from(birth.is_none()), Ordering::Release);
        HIT.fetch_add(1, Ordering::AcqRel);
        return 0x8899;
    }
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
extern "system" fn reset_body(pointer: u64) -> u64 {
    let error = unsafe { GetLastError() };
    let network = value(pointer + 0x10);
    if network != 0 {
        unsafe {
            *((network + 0xc0) as *mut u64) = 0;
            *((pointer + 0x10) as *mut u64) = 0;
        }
    }
    unsafe { SetLastError(error) };
    u64::from(network != 0)
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
    "mov rbx,rcx","call {reset}","cmp byte ptr [rip+{network_alloc}],0","je 3f","mov rcx,[rip+{network}]","call {network_ctor}",".globl transport_fixture_network_return","transport_fixture_network_return:",
    "mov [rbx+0x10],rax","mov rcx,rax","test rcx,rcx","je 4f","call {start}",".globl transport_fixture_start_return","transport_fixture_start_return:",
    "4:","cmp qword ptr [rbx+0x10],0","setne al","movzx eax,al","mov rbx,[rsp+0x40]","lea rsp,[rsp+0x30]","pop rdi","ret",
    "3:","xor eax,eax","jmp transport_fixture_network_return",".seh_endproc",
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
    ".p2align 4",".globl transport_fixture_reset",".seh_proc transport_fixture_reset","transport_fixture_reset:",
    ".byte 0x40,0x53",".seh_pushreg rbx","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "mov rbx,rcx","cmp byte ptr [rip+{reset_join}],0","je 5f","mov rcx,[rbx+0x10]","test rcx,rcx","je 5f","mov rcx,[rcx+0xc0]","test rcx,rcx","je 5f",
    "mov edx,0xffffffff","call {join_gateway}",".globl transport_fixture_reset_join_return","transport_fixture_reset_join_return:","call {destructive}",
    "5:","mov rcx,rbx","call {reset_body}","lea rsp,[rsp+0x20]","pop rbx","ret",".seh_endproc",
    factory=sym transport_fixture_factory_c,local=sym transport_fixture_local,factory_body=sym factory_body,
    network=sym NETWORK,network_ctor=sym transport_fixture_network,network_body=sym network_body,
    start=sym transport_fixture_start,creator=sym transport_fixture_creator,callback=sym FIXTURE_CALLBACK,
    thread=sym THREAD,thread_ctor=sym transport_fixture_thread,thread_body=sym thread_body,
    before_start=sym before_start,receive_body=sym receive_body,unused=sym unused_body,
    reset=sym transport_fixture_reset,reset_body=sym reset_body,network_alloc=sym NETWORK_ALLOC_OK,
    reset_join=sym RESET_JOIN_ENABLED,join_gateway=sym super::super::native_session_join_gateway,destructive=sym reset_destructive_continuation,
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
    fn transport_fixture_reset();
    fn transport_fixture_reset_join_return();
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
    setup_with_transport_offset(0)
}
fn setup_with_transport_offset(offset: usize) -> (u64, SessionLease, [usize; 9], Vec<*mut u8>) {
    let tables = unsafe { VirtualAlloc(null(), 0x40000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
        as *mut u8;
    assert!(!tables.is_null());
    let base = tables as u64 - 0x1040000;
    EXE.store(base, Ordering::Release);
    super::super::EXE.store(base, Ordering::Release);
    let session = allocate() as u64;
    let callback = allocate() as u64;
    let transport_region = allocate();
    let p = transport_region as u64 + offset as u64;
    let n = allocate() as u64;
    let t = allocate() as u64;
    unsafe {
        *(session as *mut u64) = exe(0x1045bd8);
        *((session + 0x30) as *mut u64) = callback;
        *((exe(0x1045bd8) + 0x408) as *mut u64) = exe(0x83ade0);
        *((exe(0x1047e60) + 0x40) as *mut u64) = exe(0x84ebd0);
        *((exe(0x1047e60) + 0x58) as *mut u64) = exe(0x84ec00);
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
            transport_region,
            n as *mut u8,
            t as *mut u8,
        ],
    )
}
#[test]
fn input_relationship_requires_active_flag_one_and_rejects_other_values() {
    if !child("input_relationship_requires_active_flag_one_and_rejects_other_values") {
        return;
    }
    let (session, lease, _, owned) = setup();
    unsafe { transport_fixture_driver(session) };
    let transport = super::super::state().transport(lease).unwrap();
    let active = from_network(transport.network);
    unsafe { *((transport.pointer + 0x45) as *mut u8) = 0 };
    let inactive = from_network(transport.network);
    unsafe { *((transport.pointer + 0x45) as *mut u8) = 2 };
    let unknown = from_network(transport.network);
    unsafe { *((transport.pointer + 0x45) as *mut u8) = 1 };
    let active_again = from_network(transport.network);
    unsafe { restore().unwrap() };
    for pointer in owned {
        assert_ne!(unsafe { VirtualFree(pointer.cast(), 0, MEM_RELEASE) }, 0);
    }
    assert_eq!(active, Ok(Some((lease, transport))));
    assert_eq!(inactive, Ok(None));
    assert_eq!(
        active_again, active,
        "a refused observation must not permanently disable a valid transport"
    );
    assert_eq!(
        unknown,
        Ok(None),
        "an observed non-active value cannot issue an input relationship"
    );
}
#[test]
fn unreadable_active_flag_never_issues_an_input_relationship() {
    if !child("unreadable_active_flag_never_issues_an_input_relationship") {
        return;
    }
    let (session, lease, _, owned) = setup_with_transport_offset(4096 - 0x40);
    unsafe { transport_fixture_driver(session) };
    let transport = super::super::state().transport(lease).unwrap();
    let active = from_network(transport.network);
    let flag_page = (transport.pointer + 0x45) & !4095;
    let mut previous = 0;
    assert_ne!(
        unsafe { VirtualProtect(flag_page as _, 4096, PAGE_NOACCESS, &mut previous) },
        0
    );
    let observed = copy_current::<1>(transport.pointer + 0x45);
    let still_readable = read64(transport.pointer + 0x10);
    let unknown = from_network(transport.network);
    let mut replaced = 0;
    assert_ne!(
        unsafe { VirtualProtect(flag_page as _, 4096, previous, &mut replaced) },
        0
    );
    let active_again = from_network(transport.network);
    unsafe { restore().unwrap() };
    for pointer in owned {
        assert_ne!(unsafe { VirtualFree(pointer.cast(), 0, MEM_RELEASE) }, 0);
    }
    assert_eq!(active, Ok(Some((lease, transport))));
    assert_eq!(observed, None, "the real read must fail at the flag itself");
    assert_eq!(
        still_readable,
        Ok(transport.network),
        "unrelated fields remain readable"
    );
    assert_eq!(active_again, active);
    assert_eq!(
        unknown,
        Ok(None),
        "unknown is not a live producer observation"
    );
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
fn restart_case(published_first: bool) {
    let (session, lease, _, owned) = setup();
    if published_first {
        enable_reset_join();
    }
    START_OK.store(published_first, Ordering::Release);
    unsafe {
        SetLastError(0x1234);
        transport_fixture_driver(session)
    };
    let pointer = POINTER.load(Ordering::Acquire);
    let old = state().pointer_birth(pointer).unwrap();
    let original: unsafe extern "system" fn(u64, u64) -> u64 =
        unsafe { std::mem::transmute(transport_fixture_local as *const ()) };
    START_OK.store(true, Ordering::Release);
    unsafe { SetLastError(0x1234) };
    let result = unsafe { original(pointer, 0x1234) };
    let error = unsafe { GetLastError() };
    let current = super::super::state().transport(lease).unwrap();
    let old_snapshot = state().snapshot(old);
    let qualified = from_network(current.network);
    unsafe { restore().unwrap() };
    for region in owned {
        assert_ne!(unsafe { VirtualFree(region.cast(), 0, MEM_RELEASE) }, 0);
    }
    assert_eq!(result, 1);
    assert_eq!(error, 0x1234);
    assert_ne!(
        current.incarnation, old.0,
        "a normal reset needs a new binding identity even when addresses are reused"
    );
    assert_eq!(
        old_snapshot,
        Err(Error::Stale),
        "old metadata retirement is not a join certificate"
    );
    assert_eq!(qualified, Ok(Some((lease, current))));
    assert_eq!(
        census::event_count(census::SemanticEvent::TransportPublished),
        1 + u64::from(published_first)
    );
    assert!(!FAILED.load(Ordering::Acquire));
}
#[test]
fn local_restart_after_failed_start_reaches_original_and_publishes_a_fresh_binding() {
    if child("local_restart_after_failed_start_reaches_original_and_publishes_a_fresh_binding") {
        restart_case(false);
    }
}
#[test]
fn local_restart_after_publication_keeps_original_and_renews_the_binding() {
    if child("local_restart_after_publication_keeps_original_and_renews_the_binding") {
        restart_case(true);
    }
}
#[test]
fn local_restart_allocation_failure_keeps_the_next_observed_attempt_eligible() {
    if !child("local_restart_allocation_failure_keeps_the_next_observed_attempt_eligible") {
        return;
    }
    let (session, lease, _, owned) = setup();
    enable_reset_join();
    unsafe {
        SetLastError(0x1234);
        transport_fixture_driver(session)
    };
    let pointer = POINTER.load(Ordering::Acquire);
    let old = super::super::state().transport(lease).unwrap();
    let local: unsafe extern "system" fn(u64, u64) -> u64 =
        unsafe { std::mem::transmute(transport_fixture_local as *const ()) };
    NETWORK_ALLOC_OK.store(false, Ordering::Release);
    let failed = unsafe { local(pointer, 0x1234) };
    let next = state().pointer_birth(pointer).unwrap();
    let candidate = state().snapshot(next).unwrap().3;
    let no_publication = super::super::state().transport(lease);
    NETWORK_ALLOC_OK.store(true, Ordering::Release);
    unsafe { SetLastError(0x1234) };
    let success = unsafe { local(pointer, 0x1234) };
    let current = super::super::state().transport(lease).unwrap();
    unsafe { restore().unwrap() };
    for region in owned {
        assert_ne!(unsafe { VirtualFree(region.cast(), 0, MEM_RELEASE) }, 0);
    }
    assert_eq!(failed, 0);
    assert_eq!(success, 1);
    assert_eq!(candidate.network, 0);
    assert_eq!(candidate.thread, 0);
    assert_ne!(next.0, old.incarnation);
    assert_eq!(current.incarnation, next.0);
    assert_eq!(no_publication, Err(Error::Unbound));
    assert_eq!(
        census::event_count(census::SemanticEvent::TransportPublished),
        2
    );
}
fn repeated_restart_case(published_first: bool, allocation_failure: bool, failed_restart: bool) {
    let (session, lease, _, owned) = setup();
    super::super::state()
        .bind_world(
            lease,
            WorldReceipt {
                binding_id: 8,
                epoch: 1,
            },
        )
        .unwrap();
    if published_first {
        enable_reset_join();
    }
    START_OK.store(published_first, Ordering::Release);
    unsafe {
        SetLastError(0x1234);
        transport_fixture_driver(session);
    }
    let pointer = POINTER.load(Ordering::Acquire);
    let first = state().pointer_birth(pointer).unwrap();
    let old_ticket = if published_first {
        let old = super::super::state().transport(lease).unwrap();
        Some(
            super::super::state()
                .reserve_input(lease, old)
                .unwrap()
                .unwrap(),
        )
    } else {
        None
    };
    let local: unsafe extern "system" fn(u64, u64) -> u64 =
        unsafe { std::mem::transmute(transport_fixture_local as *const ()) };
    if allocation_failure {
        NETWORK_ALLOC_OK.store(false, Ordering::Release);
        assert_eq!(unsafe { local(pointer, 0x1234) }, 0);
        assert_eq!(super::super::state().transport(lease), Err(Error::Unbound));
        NETWORK_ALLOC_OK.store(true, Ordering::Release);
    }
    START_OK.store(!failed_restart, Ordering::Release);
    unsafe {
        SetLastError(0x1234);
    }
    // The native local-start return means that network allocation succeeded;
    // only actual publication establishes that its thread was started.
    assert_eq!(unsafe { local(pointer, 0x1234) }, 1);
    let middle = state().pointer_birth(pointer).unwrap();
    let middle_publication = super::super::state().transport(lease);
    assert_eq!(middle_publication.is_ok(), !failed_restart);
    START_OK.store(true, Ordering::Release);
    unsafe {
        SetLastError(0x1234);
    }
    eprintln!("REPEATED_RESET second reset after failed_restart={failed_restart} allocation_failure={allocation_failure}");
    assert_eq!(unsafe { local(pointer, 0x1234) }, 1);
    let error = unsafe { GetLastError() };
    let current = super::super::state().transport(lease).unwrap();
    assert!(current.incarnation > middle.0 && middle.0 > first.0);
    assert_eq!(state().snapshot(first), Err(Error::Stale));
    assert_eq!(state().snapshot(middle), Err(Error::Stale));
    assert_eq!(from_network(current.network), Ok(Some((lease, current))));
    if let Some(ticket) = old_ticket {
        let mut sessions = super::super::state();
        assert_eq!(
            sessions.message_world(ticket),
            Ok(WorldReceipt {
                binding_id: 8,
                epoch: 1
            })
        );
        assert_eq!(
            sessions.take_reset_input(ticket, value(session + 0x30), 17),
            Ok(Some(lease))
        );
        assert_eq!(
            sessions.take_reset_input(ticket, value(session + 0x30), 17),
            Err(Error::Unknown)
        );
        assert_eq!(
            sessions.drained(WorldReceipt {
                binding_id: 8,
                epoch: 1
            }),
            Err(Error::Unresolved)
        );
    }
    assert_eq!(error, 0x1234);
    assert!(!FAILED.load(Ordering::Acquire));
    unsafe {
        restore().unwrap();
    }
    for region in owned {
        assert_ne!(unsafe { VirtualFree(region.cast(), 0, MEM_RELEASE) }, 0);
    }
}
#[test]
fn published_failed_restart_can_reset_again_without_losing_old_packet_identity() {
    if child("published_failed_restart_can_reset_again_without_losing_old_packet_identity") {
        repeated_restart_case(true, false, true);
    }
}
#[test]
fn published_allocation_then_start_failure_can_reset_again_and_publish() {
    if child("published_allocation_then_start_failure_can_reset_again_and_publish") {
        repeated_restart_case(true, true, true);
    }
}
#[test]
fn published_repeated_successful_resets_keep_fresh_bindings() {
    if child("published_repeated_successful_resets_keep_fresh_bindings") {
        repeated_restart_case(true, false, false);
    }
}
#[test]
fn unpublished_repeated_failed_starts_keep_later_publication_valid() {
    if child("unpublished_repeated_failed_starts_keep_later_publication_valid") {
        repeated_restart_case(false, false, true);
    }
}
#[test]
fn local_restart_join_al0_stops_before_native_destructive_continuation() {
    const NAME: &str = "local_restart_join_al0_stops_before_native_destructive_continuation";
    const ENV: &str = "CIV6_NATIVE_RESET_EXPECT_AL0";
    if std::env::var_os(ENV).is_some() {
        unsafe { windows_sys::Win32::System::Diagnostics::Debug::SetErrorMode(3) };
        let (session, _, _, _owned) = setup();
        enable_reset_join();
        unsafe {
            SetLastError(0x1234);
            transport_fixture_driver(session)
        };
        RESET_JOIN_AL.store(0, Ordering::Release);
        let local: unsafe extern "system" fn(u64, u64) -> u64 =
            unsafe { std::mem::transmute(transport_fixture_local as *const ()) };
        unsafe { local(POINTER.load(Ordering::Acquire), 0x1234) };
        panic!("reset AL0 reached the original continuation");
    }
    let output = crate::windows::test_child_output::run(
        std::process::Command::new(std::env::current_exe().unwrap())
            .creation_flags(0x08000000)
            .env(ENV, "1")
            .args([
                "--exact",
                &format!(
                    "windows::root_observation::native_frames::sessions::transports::tests::{NAME}"
                ),
                "--nocapture",
            ]),
        Duration::from_secs(15),
    )
    .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code().map(|value| value as u32),
        Some(0xc0000409)
    );
    assert!(
        stderr.contains("reset-original-join-AL0-reached"),
        "wrong failure: {stderr}"
    );
    assert!(
        !stderr.contains("reset-destructive-continuation-reached") && !stderr.contains("panicked"),
        "unsafe or fixture failure: {stderr}"
    );
}
#[test]
fn unpublished_receive_executes_original_without_an_input_relationship() {
    if !child("unpublished_receive_executes_original_without_an_input_relationship") {
        return;
    }
    let (session, lease, _, owned) = setup();
    START_OK.store(false, Ordering::Release);
    unsafe { transport_fixture_driver(session) };
    UNQUALIFIED_RECEIVE.store(true, Ordering::Release);
    HIT.store(0, Ordering::Release);
    let result = unsafe { transport_fixture_receive(NETWORK.load(Ordering::Acquire)) };
    let qualified = from_network(NETWORK.load(Ordering::Acquire));
    let transport = super::super::state().transport(lease);
    unsafe { restore().unwrap() };
    for region in owned {
        assert_ne!(unsafe { VirtualFree(region.cast(), 0, MEM_RELEASE) }, 0);
    }
    assert_eq!(result, 0x8899);
    assert_eq!(HIT.load(Ordering::Acquire), 1);
    assert_eq!(OBSERVED.load(Ordering::Acquire), 1);
    assert_eq!(qualified, Ok(None));
    assert_eq!(transport, Err(Error::Unbound));
}
#[test]
fn reset_retirement_waits_for_old_executions_and_never_joins_the_old_binding() {
    let mut ledger = Ledger::default();
    let old = ledger.factory(SessionLease(1), 2, 3).unwrap();
    ledger.transport(old, 4, 3).unwrap();
    ledger.network(old, 5).unwrap();
    ledger.thread(old, 6).unwrap();
    let transport = ledger.publish(old, 6).unwrap();
    assert_eq!(ledger.enter(5), Ok(Some(old)));
    ledger.reset_begin(old).unwrap();
    assert_eq!(ledger.published_network(5), Ok(None));
    assert_eq!(ledger.reset_returned(old), Err(Error::Busy));
    ledger.leave(old).unwrap();
    let fresh = ledger.reset_returned(old).unwrap();
    assert_ne!(fresh, old);
    assert_eq!(
        ledger.joined(SessionLease(1), transport, 6, 1),
        Err(Error::Stale)
    );
    assert_eq!(ledger.snapshot(old), Err(Error::Stale));
    ledger.network(fresh, 5).unwrap();
    ledger.thread(fresh, 6).unwrap();
    let new = ledger.publish(fresh, 6).unwrap();
    assert_ne!(new.incarnation, transport.incarnation);
    assert_eq!(
        ledger.published_network(5),
        Ok(Some((SessionLease(1), new)))
    );
}
fn receive_before_poll_case(published: bool) {
    let mut network = [0u64; 32];
    network[0] = exe(0x1049808);
    let network_address = network.as_mut_ptr() as u64;
    let world = SessionWorldReceipt {
        binding_id: 1,
        epoch: 1,
    };
    let lease = {
        let mut ledger = super::super::state();
        let lease = ledger.birth(1, Some(2), 30, 2).unwrap();
        ledger.factory(lease, 7, 30, 40, 50).unwrap();
        ledger.published(lease, 7).unwrap();
        ledger.bind_world(lease, world).unwrap();
        lease
    };
    let mut transport_ledger = state();
    let birth = transport_ledger.factory(lease, 40, 50).unwrap();
    transport_ledger.transport(birth, 90, 50).unwrap();
    transport_ledger.network(birth, network_address).unwrap();
    transport_ledger.thread(birth, 110).unwrap();
    if published {
        let transport = transport_ledger.publish(birth, 110).unwrap();
        drop(transport_ledger);
        super::super::publish_transport(lease, transport.incarnation, 90, network_address, 110)
            .unwrap();
    } else {
        drop(transport_ledger);
    }
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 108;
    raw.gpr[0] = network_address;
    begin(&mut raw);
    assert_ne!(
        raw.invocation.cookie, 0,
        "the actual receive observer must start"
    );
    assert_eq!(
        current_receiver(),
        Some((network_address, raw.invocation.token))
    );
    assert_eq!(super::super::input::pending(), Ok(0));
    assert_eq!(super::super::deferred::pending(), Ok(0));
    // The complete original receive body is live, before any poll creates an
    // input ticket. Neither a storage counter nor a thread handle substitutes
    // for this frame returning.
    assert_eq!(super::super::retire_world(world), Err(Error::Busy));
    finish(&mut raw, false);
    assert_eq!(current_receiver(), None);
    super::super::retire_world(world).unwrap();
    let next = SessionWorldReceipt {
        binding_id: 2,
        epoch: 2,
    };
    super::super::state()
        .loaded_world(lease, Some(world), next)
        .unwrap();
    assert_eq!(super::super::state().world(lease), Ok(next));
}
#[test]
fn world_retirement_waits_for_the_published_receive_frame_before_poll() {
    if child("world_retirement_waits_for_the_published_receive_frame_before_poll") {
        receive_before_poll_case(true);
    }
}
#[test]
fn world_retirement_waits_for_the_receive_frame_before_transport_publication() {
    if child("world_retirement_waits_for_the_receive_frame_before_transport_publication") {
        receive_before_poll_case(false);
    }
}
#[test]
fn receive_gateway_keeps_original_return_and_blocks_retirement_until_it_returns() {
    if !child("receive_gateway_keeps_original_return_and_blocks_retirement_until_it_returns") {
        return;
    }
    let (session, lease, _, owned) = setup();
    let world = SessionWorldReceipt {
        binding_id: 1,
        epoch: 1,
    };
    super::super::state().bind_world(lease, world).unwrap();
    CHECK_RECEIVE_RETIREMENT.store(true, Ordering::Release);
    // Original start invokes the hooked receiver before publication.
    unsafe { SetLastError(0x1234) };
    assert_eq!(unsafe { transport_fixture_driver(session) }, 1);
    assert_eq!(unsafe { GetLastError() }, 0x1234);
    assert_eq!(RECEIVE_RETIREMENT_OBSERVED.load(Ordering::Acquire), 3);
    assert_eq!(HIT.load(Ordering::Acquire), 1);
    RECEIVE_RETIREMENT_OBSERVED.store(0, Ordering::Release);
    // The same real gateway after publication must retain exactly the same
    // original return semantics, without borrowing current-World authority.
    assert_eq!(
        unsafe { transport_fixture_receive(NETWORK.load(Ordering::Acquire)) },
        0x8899
    );
    assert_eq!(unsafe { GetLastError() }, 0x1234);
    assert_eq!(RECEIVE_RETIREMENT_OBSERVED.load(Ordering::Acquire), 3);
    assert_eq!(HIT.load(Ordering::Acquire), 2);
    assert_eq!(current_receiver(), None);
    super::super::retire_world(world).unwrap();
    let next = SessionWorldReceipt {
        binding_id: 2,
        epoch: 2,
    };
    super::super::state()
        .loaded_world(lease, Some(world), next)
        .unwrap();
    assert_eq!(super::super::state().world(lease), Ok(next));
    unsafe { restore().unwrap() };
    for pointer in owned {
        assert_ne!(unsafe { VirtualFree(pointer.cast(), 0, MEM_RELEASE) }, 0);
    }
}
#[test]
fn unknown_receive_frames_retain_observation_without_issuing_authority() {
    if !child("unknown_receive_frames_retain_observation_without_issuing_authority") {
        return;
    }
    let world = SessionWorldReceipt {
        binding_id: 1,
        epoch: 1,
    };
    let mut foreign = [0u64; 32];
    foreign[0] = exe(0x1049808) + 8;
    let mut first: Saved = unsafe { std::mem::zeroed() };
    first.invocation.kind = 108;
    first.gpr[0] = foreign.as_mut_ptr() as u64;
    begin(&mut first);
    let mut second: Saved = unsafe { std::mem::zeroed() };
    second.invocation.kind = 108;
    second.gpr[0] = 1; // Unreadable vtable, not a reason to stop native cleanup.
    begin(&mut second);
    CALLS.with(|calls| {
        let calls = calls.borrow();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().all(|call| call.birth.is_none()));
        assert!(calls
            .iter()
            .all(|call| call.receive.as_ref().unwrap().source.is_none()));
    });
    assert!(super::super::NativeSessionApi::current_world().is_none());
    assert_eq!(super::super::retire_world(world), Err(Error::Busy));
    finish(&mut second, false);
    assert_eq!(super::super::retire_world(world), Err(Error::Busy));
    finish(&mut first, false);
    assert!(CALLS.with(|calls| calls.borrow().is_empty()));
    super::super::retire_world(world).unwrap();
}
#[test]
fn an_unknown_receiver_cannot_adopt_a_later_publication_at_the_same_network_address() {
    const NAME: &str =
        "an_unknown_receiver_cannot_adopt_a_later_publication_at_the_same_network_address";
    if !child(NAME) {
        return;
    }
    let (session, lease, _, owned) = setup();
    let network = NETWORK.load(Ordering::Acquire);
    // A receiver arrived before a known transport/network was constructed.
    let mut old: Saved = unsafe { std::mem::zeroed() };
    old.invocation.kind = 108;
    old.gpr[0] = network;
    begin(&mut old);
    assert!(CALLS.with(|calls| calls
        .borrow()
        .last()
        .unwrap()
        .receive
        .as_ref()
        .unwrap()
        .source
        .is_none()));
    assert_eq!(super::super::input::fixture_poll_network(network), Ok(None));
    // The owned normal creation path now publishes this physical address.
    // That event must not change the earlier receiver's saved source.
    unsafe { transport_fixture_driver(session) };
    let current = super::super::state().transport(lease).unwrap();
    assert_eq!(from_network(network), Ok(Some((lease, current))));
    assert_eq!(current_receiver(), Some((network, old.invocation.token)));
    assert_eq!(super::super::input::fixture_poll_network(network), Ok(None));
    finish(&mut old, false);
    // A newly observed receiver of the actual publication remains usable.
    let mut fresh: Saved = unsafe { std::mem::zeroed() };
    fresh.invocation.kind = 108;
    fresh.gpr[0] = network;
    begin(&mut fresh);
    assert_eq!(
        super::super::input::fixture_poll_network(network),
        Ok(Some((lease, current)))
    );
    finish(&mut fresh, false);
    unsafe { restore().unwrap() };
    for pointer in owned {
        assert_ne!(unsafe { VirtualFree(pointer.cast(), 0, MEM_RELEASE) }, 0);
    }
}
#[test]
fn concurrent_reset_after_actual_poll_lookup_preserves_unqualified_original_progress() {
    const NAME: &str =
        "concurrent_reset_after_actual_poll_lookup_preserves_unqualified_original_progress";
    if !child(NAME) {
        return;
    }
    unsafe { windows_sys::Win32::System::Diagnostics::Debug::SetErrorMode(3) };
    let (session, lease, _, owned) = setup();
    unsafe { transport_fixture_driver(session) };
    let old = super::super::state().transport(lease).unwrap();
    let world = SessionWorldReceipt {
        binding_id: 1,
        epoch: 1,
    };
    super::super::state().bind_world(lease, world).unwrap();
    let mut receiver: Saved = unsafe { std::mem::zeroed() };
    receiver.invocation.kind = 108;
    receiver.gpr[0] = old.network;
    begin(&mut receiver);
    super::super::input::fixture_after_poll_lookup(reset_between_poll_lookup_and_reservation);
    unsafe { SetLastError(0x7931) };
    let result = super::super::input::fixture_poll_with_authority(old.network);
    let poll_error = unsafe { GetLastError() };
    eprintln!("CONCURRENT_RESET_POLL_RETURNED {result:?}");
    assert_eq!(
        result,
        Ok((Some((lease, old)), false)),
        "reset input retains its exact old source lifetime"
    );
    assert!(!FAILED.load(Ordering::Acquire));
    assert_eq!(poll_error, 0x7931);
    assert_eq!(super::super::retire_world(world), Err(Error::Busy));
    assert_eq!(super::super::input::pending(), Ok(0));
    finish(&mut receiver, false);
    eprintln!("CONCURRENT_RESET_RECEIVER_RETURNED");
    let (release, worker) = CONCURRENT_RESET.lock().unwrap().take().unwrap();
    release.send(()).unwrap();
    worker.join().unwrap();
    eprintln!("CONCURRENT_RESET_WORKER_RETURNED");
    let local: unsafe extern "system" fn(u64, u64) -> u64 =
        unsafe { std::mem::transmute(transport_fixture_local as *const ()) };
    assert_eq!(unsafe { local(POINTER.load(Ordering::Acquire), 0x1234) }, 1);
    eprintln!("CONCURRENT_RESET_NEW_PUBLICATION_RETURNED");
    let new = super::super::state().transport(lease).unwrap();
    assert_ne!(new.incarnation, old.incarnation);
    let mut next: Saved = unsafe { std::mem::zeroed() };
    next.invocation.kind = 108;
    next.gpr[0] = new.network;
    begin(&mut next);
    assert_eq!(
        super::super::input::fixture_poll_network(new.network),
        Ok(Some((lease, new)))
    );
    finish(&mut next, false);
    super::super::retire_world(world).unwrap();
    assert_eq!(
        super::super::NativeSessionApi::drained(world),
        Err(Error::Unresolved)
    );
    let fresh_world = SessionWorldReceipt {
        binding_id: 2,
        epoch: 2,
    };
    let mut sessions = super::super::state();
    let fresh_session = sessions.birth(1, Some(2), 130, 2).unwrap();
    sessions.factory(fresh_session, 8, 130, 140, 150).unwrap();
    sessions.published(fresh_session, 8).unwrap();
    sessions.bind_world(fresh_session, fresh_world).unwrap();
    sessions
        .publish_transport(fresh_session, 1000, 160, 170, 180)
        .unwrap();
    let packet = sessions.reserve(fresh_session).unwrap();
    assert_eq!(sessions.message_world(packet), Ok(fresh_world));
    sessions.cancel_before_dispatch(packet).unwrap();
    drop(sessions);
    unsafe { restore().unwrap() };
    for pointer in owned {
        assert_ne!(unsafe { VirtualFree(pointer.cast(), 0, MEM_RELEASE) }, 0);
    }
}
#[test]
fn receive_exception_settles_its_observation_but_never_certifies_completion() {
    if !child("receive_exception_settles_its_observation_but_never_certifies_completion") {
        return;
    }
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 108;
    raw.gpr[0] = 1;
    begin(&mut raw);
    finish(&mut raw, true);
    assert!(CALLS.with(|calls| calls.borrow().is_empty()));
    assert_eq!(current_receiver(), None);
    assert_eq!(
        super::super::retire_world(SessionWorldReceipt {
            binding_id: 1,
            epoch: 1
        }),
        Err(Error::Unresolved)
    );
    // An unowned receive's exception is missing close proof, not evidence that
    // a later unrelated native Session is corrupt. Keep its normal path live.
    assert!(!FAILED.load(Ordering::Acquire));
    let mut sessions = super::super::state();
    let next = sessions.birth(1, Some(2), 30, 2).unwrap();
    sessions.factory(next, 7, 30, 40, 50).unwrap();
    sessions.published(next, 7).unwrap();
    assert_eq!(sessions.snapshot(next), Ok((40, 7, 50)));
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
