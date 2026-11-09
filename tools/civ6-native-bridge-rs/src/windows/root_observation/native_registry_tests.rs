use super::*;
use std::os::windows::process::CommandExt;
use std::ptr::null;
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlLookupFunctionEntry, IMAGE_RUNTIME_FUNCTION_ENTRY,
};
static FIXTURE: AtomicBool = AtomicBool::new(false);
static REGISTRY: AtomicU64 = AtomicU64::new(0);
static NODE: AtomicU64 = AtomicU64::new(0);
static SESSION: AtomicU64 = AtomicU64::new(0);
static OBSERVED: AtomicU64 = AtomicU64::new(0);
static HITS: AtomicU64 = AtomicU64::new(0);
static FIXTURE_LEASE: OnceLock<SessionLease> = OnceLock::new();
pub(super) fn caller_override(rva: u64) -> Option<u64> {
    if !FIXTURE.load(Ordering::Acquire) {
        return None;
    }
    match rva {
        0x7f929a => Some(registry_fixture_destructor_return as *const () as u64),
        0x7f8375 => Some(registry_fixture_destroy_return as *const () as u64),
        _ => None,
    }
}
fn child(name: &str) -> bool {
    const ENV: &str = "CIV6_NATIVE_REGISTRY_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    let mut p = std::process::Command::new(std::env::current_exe().unwrap())
        .creation_flags(0x08000000)
        .env(ENV, "1")
        .args([
            "--exact",
            &format!("windows::root_observation::native_frames::sessions::registry::tests::{name}"),
            "--nocapture",
        ])
        .spawn()
        .unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(exit) = p.try_wait().unwrap() {
            assert!(exit.success(), "registrychild{exit}");
            return false;
        }
        if Instant::now() >= until {
            let _ = p.kill();
            let _ = p.wait();
            panic!("registrychildtimeout")
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
extern "system" fn session_body(session: u64) -> u64 {
    let lease = *FIXTURE_LEASE.get().unwrap();
    let mut s = super::super::state();
    let mut bits = u64::from(s.born(REGISTRY.load(Ordering::Acquire), 7, 17).is_none());
    bits |= u64::from(s.reserve(lease) == Err(Error::Closed)) << 1;
    let new = s
        .birth(REGISTRY.load(Ordering::Acquire), Some(7), 9, 1)
        .unwrap();
    bits |= u64::from(s.factory(new, 17, 9, session, 0x991) == Err(Error::Busy)) << 2;
    s.factory(new, 17, 9, 0, 0).unwrap();
    OBSERVED.fetch_or(bits, Ordering::AcqRel);
    HITS.fetch_add(1, Ordering::AcqRel);
    unsafe { SetLastError(0x4455) };
    session
}
extern "system" fn before_node_free() {
    let lease = *FIXTURE_LEASE.get().unwrap();
    let s = super::super::state();
    assert!(!s.retired_slot(
        REGISTRY.load(Ordering::Acquire),
        17,
        SESSION.load(Ordering::Acquire)
    ));
    assert_eq!(s.born(REGISTRY.load(Ordering::Acquire), 7, 17), None);
    assert_eq!(
        s.snapshot(lease).unwrap().0,
        SESSION.load(Ordering::Acquire)
    );
}
extern "system" fn empty_kind4(session: u64) -> u64 {
    session
}
global_asm!(
".p2align 4",".globl registry_fixture_remove",".seh_proc registry_fixture_remove","registry_fixture_remove:",
"mov [rsp+8],rbx",".seh_savereg rbx,8","push rdi",".seh_pushreg rdi","sub rsp,0x30",".seh_stackalloc 0x30",".seh_endprologue",
"mov rdi,rcx","mov rbx,[rip+{node}]","mov rcx,[rbx+8]","mov edx,1","test rcx,rcx","jz 2f","call {destructor}",
".globl registry_fixture_destructor_return","registry_fixture_destructor_return:","mov qword ptr [rbx+8],0","call {before_free}",
"mov rax,[rdi+0x10]","mov qword ptr [rax],0","dec dword ptr [rdi+0x1c]","2:","mov eax,0x8877","mov rbx,[rsp+0x40]","lea rsp,[rsp+0x30]","pop rdi","ret",".seh_endproc",
".p2align 4",".globl registry_fixture_session",".seh_proc registry_fixture_session","registry_fixture_session:",
"mov [rsp+8],rbx",".seh_savereg rbx,8","push rdi",".seh_pushreg rdi","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
"call {session_body}","mov rbx,[rsp+0x30]","lea rsp,[rsp+0x20]","pop rdi","ret",".seh_endproc",
".p2align 4",".globl registry_fixture_kind4",".seh_proc registry_fixture_kind4","registry_fixture_kind4:",
"mov [rsp+8],rbx",".seh_savereg rbx,8","push rdi",".seh_pushreg rdi","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
"call {empty}","mov rbx,[rsp+0x30]","lea rsp,[rsp+0x20]","pop rdi","ret",".seh_endproc",
".globl registry_fixture_destroy",".seh_proc registry_fixture_destroy","registry_fixture_destroy:",
"push rbx",".seh_pushreg rbx","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue","mov rbx,rcx","mov rcx,[rip+{session}]","mov edx,1","call {destructor}",
".globl registry_fixture_destroy_return","registry_fixture_destroy_return:","mov rax,[rbx+0x10]","mov qword ptr [rax],0","call {before_free}","mov rax,rbx","lea rsp,[rsp+0x20]","pop rbx","ret",".seh_endproc",
node=sym NODE,session=sym SESSION,destructor=sym registry_fixture_session,session_body=sym session_body,before_free=sym before_node_free,empty=sym empty_kind4,
);
unsafe extern "system" {
    fn registry_fixture_remove(registry: u64, id: u32) -> u64;
    fn registry_fixture_session();
    fn registry_fixture_kind4();
    fn registry_fixture_destroy(registry: u64) -> u64;
    fn registry_fixture_destructor_return();
    fn registry_fixture_destroy_return();
    fn registry_remove_original_body();
    fn registry_session_original_body();
    fn registry_kind4_original_body();
}
struct Fixture {
    registry: u64,
    session: u64,
    lease: SessionLease,
    owned: Vec<*mut u8>,
}
impl Fixture {
    fn new() -> Self {
        let table =
            unsafe { VirtualAlloc(null(), 0x10000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
                as *mut u8;
        assert!(!table.is_null());
        let base = table as u64 - 0x1040000;
        EXE.store(base, Ordering::Release);
        super::super::EXE.store(base, Ordering::Release);
        let mut owned = vec![table];
        for _ in 0..4 {
            let p =
                unsafe { VirtualAlloc(null(), 0x1000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
                    as *mut u8;
            assert!(!p.is_null());
            owned.push(p);
        }
        let registry = owned[1] as u64;
        let session = owned[2] as u64;
        let buckets = owned[3] as u64;
        let node = owned[4] as u64;
        unsafe {
            *((registry + 0x10) as *mut u64) = buckets;
            *((registry + 0x18) as *mut u32) = 1;
            *((registry + 0x1c) as *mut u32) = 1;
            *(buckets as *mut u64) = node;
            *((buckets + 8) as *mut u64) = buckets + 8;
            *(node as *mut u32) = 17;
            *((node + 8) as *mut u64) = session;
            *(session as *mut u64) = exe(0x1045bd8);
        }
        REGISTRY.store(registry, Ordering::Release);
        NODE.store(node, Ordering::Release);
        SESSION.store(session, Ordering::Release);
        REGISTRIES.lock().unwrap().insert(registry, 7);
        let lease = {
            let mut s = super::super::state();
            let l = s.birth(registry, Some(7), 9, 1).unwrap();
            s.factory(l, 17, 9, session, 0x991).unwrap();
            s.published(l, 17).unwrap();
            l
        };
        FIXTURE_LEASE.set(lease).unwrap();
        let addresses = [
            registry_fixture_remove as *const () as usize,
            registry_fixture_session as *const () as usize,
            registry_fixture_kind4 as *const () as usize,
        ];
        let sites = std::array::from_fn(|i| {
            (
                addresses[i],
                copy_current::<8>(addresses[i] as u64).unwrap(),
                5,
            )
        });
        unsafe { install_at(0, &sites).unwrap() };
        EXE.store(base, Ordering::Release);
        FIXTURE.store(true, Ordering::Release);
        Self {
            registry,
            session,
            lease,
            owned,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        unsafe { restore().unwrap() };
        for p in &self.owned {
            unsafe { VirtualFree((*p).cast(), 0, MEM_RELEASE) };
        }
    }
}
#[test]
fn actual_remove_keeps_id_and_address_tombstone_through_destructor_and_node_free() {
    if !child("actual_remove_keeps_id_and_address_tombstone_through_destructor_and_node_free") {
        return;
    }
    let f = Fixture::new();
    unsafe { SetLastError(0x1234) };
    assert_eq!(unsafe { registry_fixture_remove(f.registry, 17) }, 0x8877);
    assert_eq!(unsafe { GetLastError() }, 0x4455);
    assert_eq!(OBSERVED.load(Ordering::Acquire), 7);
    assert_eq!(HITS.load(Ordering::Acquire), 1);
    assert!(super::super::state().retired_slot(f.registry, 17, f.session));
    let mut s = super::super::state();
    let next = s.birth(f.registry, Some(7), 9, 1).unwrap();
    s.factory(next, 17, 9, f.session, 0x991).unwrap();
    s.published(next, 17).unwrap();
    assert_ne!(next, f.lease);
}
#[test]
fn existing_registry_root_holds_every_session_until_full_wrapper_return() {
    if !child("existing_registry_root_holds_every_session_until_full_wrapper_return") {
        return;
    }
    let f = Fixture::new();
    let token = destroy_begin(f.registry, 7).unwrap();
    assert_eq!(unsafe { registry_fixture_destroy(f.registry) }, f.registry);
    assert!(!super::super::state().retired_slot(f.registry, 17, f.session));
    destroy_end(token, false).unwrap();
    assert!(super::super::state().retired_slot(f.registry, 17, f.session));
    assert_eq!(HITS.load(Ordering::Acquire), 1);
}
#[test]
fn tombstone_needs_every_typed_return_and_real_join_not_count_or_null() {
    let mut s = Ledger::default();
    let l = SessionLease(1);
    let t = SessionTransport {
        incarnation: 2,
        pointer: 3,
        network: 4,
        thread: 5,
    };
    let root = s
        .begin(
            8,
            9,
            vec![
                Target {
                    lease: l,
                    id: 17,
                    session: 6,
                    transport: Some(t),
                },
                Target {
                    lease: l,
                    id: 18,
                    session: 7,
                    transport: None,
                },
            ],
        )
        .unwrap();
    assert!(s.finish(root, 8, 9).is_err());
    s.enter(root, 6).unwrap();
    assert_eq!(s.returned(root, 6).err(), Some(Error::NotJoined));
    assert!(s.join(root, 6, 5, 0).is_err());
    s.join(root, 6, 5, 1).unwrap();
    s.returned(root, 6).unwrap();
    assert!(s.finish(root, 8, 9).is_err());
    s.enter(root, 7).unwrap();
    s.returned(root, 7).unwrap();
    assert_eq!(s.finish(root, 8, 9).unwrap().len(), 2);
}
#[test]
fn terminal_seh_retains_tombstone_and_rejects_next_incarnation() {
    let mut s = Ledger::default();
    let root = s
        .begin(
            1,
            2,
            vec![Target {
                lease: SessionLease(1),
                id: 3,
                session: 4,
                transport: None,
            }],
        )
        .unwrap();
    s.enter(root, 4).unwrap();
    s.unresolved();
    assert!(s.finish(root, 1, 2).is_err());
    assert!(s.begin(1, 2, vec![]).is_err());
}
#[test]
fn static_remove_session_shims_have_native_unwind_records() {
    for pc in [
        registry_remove_original_body as *const () as u64,
        registry_session_original_body as *const () as u64,
        registry_kind4_original_body as *const () as u64,
    ] {
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
#[test]
fn three_real_cas_entries_restore_original_bytes_and_protection() {
    if !child("three_real_cas_entries_restore_original_bytes_and_protection") {
        return;
    }
    let _f = Fixture::new();
    let originals: Vec<_> = HOOKS
        .lock()
        .unwrap()
        .iter()
        .map(|h| (h.hook.target as u64, h.hook.original, h.protection))
        .collect();
    assert_eq!(originals.len(), 3);
    for (address, original, _) in &originals {
        assert_ne!(copy_current::<8>(*address), Some(*original));
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
}
#[test]
fn three_displaced_shims_unwind_exact_return_rsp_and_rbx() {
    for pc in [
        registry_remove_original_body as *const () as u64,
        registry_session_original_body as *const () as u64,
        registry_kind4_original_body as *const () as u64,
    ] {
        let mut stack = [0u64; 32];
        let entry = unsafe { stack.as_mut_ptr().add(8) } as u64;
        unsafe {
            *(entry as *mut u64) = 0x123456;
            *((entry + 8) as *mut u64) = 0x881;
        }
        let mut c: CONTEXT = unsafe { std::mem::zeroed() };
        c.Rip = pc;
        c.Rsp = entry;
        let mut base = 0;
        let row = unsafe { RtlLookupFunctionEntry(c.Rip, &mut base, null_mut()) };
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
        assert_eq!((c.Rip, c.Rsp, c.Rbx), (0x123456, entry + 8, 0x881));
    }
}
static UNWIND_STATE: AtomicU64 = AtomicU64::new(0);
global_asm!(
".globl registry_unwind_driver",".seh_proc registry_unwind_driver","registry_unwind_driver:",
"push rbx",".seh_pushreg rbx","sub rsp,0x80",".seh_stackalloc 0x80",".seh_endprologue","mov rbx,rcx","mov [rcx],rsp","lea rax,[rip+registry_unwind_landed]","mov [rcx+8],rax","mov rcx,r8","call rdx","xor eax,eax","jmp 2f",
"registry_unwind_landed:","mov eax,1","2:","lea rsp,[rsp+0x80]","pop rbx","ret",".seh_endproc",
".p2align 4",".globl registry_unwind_remove",".seh_proc registry_unwind_remove","registry_unwind_remove:",
"mov [rsp+8],rbx",".seh_savereg rbx,8","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","mov rcx,[rip+{state}]","call registry_unwind_body","int3",".seh_endproc",
".globl registry_unwind_body",".seh_proc registry_unwind_body","registry_unwind_body:",
"sub rsp,0x528",".seh_stackalloc 0x528",".seh_endprologue","mov rdx,[rcx+8]","mov rcx,[rcx]","lea r10,[rsp+0x40]","mov r11d,0x9a",
"1:","mov qword ptr [r10],0","lea r10,[r10+8]","dec r11d","jnz 1b","lea r10,[rsp+0x40]","mov [rsp+0x20],r10","mov qword ptr [rsp+0x28],0","xor r8d,r8d","xor r9d,r9d","call {unwind_ex}","int3",".seh_endproc",
state=sym UNWIND_STATE,unwind_ex=sym RtlUnwindEx);
unsafe extern "system" {
    fn registry_unwind_remove();
}
#[test]
fn actual_remove_seh_keeps_key_tombstone_and_session_unresolved() {
    if !child("actual_remove_seh_keeps_key_tombstone_and_session_unresolved") {
        return;
    }
    let f = Fixture::new();
    let mut landing = [0u64; 2];
    UNWIND_STATE.store(landing.as_mut_ptr() as u64, Ordering::Release);
    NATIVE_REGISTRY_CONTINUE[0].store(
        registry_unwind_remove as *const () as u64 + 5,
        Ordering::Release,
    );
    // Driver must forward EDX=id to the remove gateway; retain registry in R8.
    assert_eq!(
        unsafe { registry_unwind_remove_call(landing.as_mut_ptr(), gateways()[0], f.registry, 17) },
        1
    );
    assert!(CALLS.with(|s| s.borrow().is_empty()));
    assert!(!super::super::state().retired_slot(f.registry, 17, f.session));
    assert_eq!(
        super::super::state().reserve(f.lease),
        Err(Error::Unresolved)
    );
}
unsafe extern "system" {
    fn registry_unwind_remove_call(state: *mut u64, target: usize, registry: u64, id: u32) -> u64;
}
global_asm!(
    ".globl registry_unwind_remove_call",
    ".seh_proc registry_unwind_remove_call",
    "registry_unwind_remove_call:",
    "push rbx",
    ".seh_pushreg rbx",
    "push rdi",
    ".seh_pushreg rdi",
    "sub rsp,0x88",
    ".seh_stackalloc 0x88",
    ".seh_endprologue",
    "mov rbx,rcx",
    "mov rdi,rdx",
    "mov [rcx],rsp",
    "lea rax,[rip+registry_unwind_call_landed]",
    "mov [rcx+8],rax",
    "mov rcx,r8",
    "mov edx,r9d",
    "call rdi",
    "xor eax,eax",
    "jmp 2f",
    "registry_unwind_call_landed:",
    "mov eax,1",
    "2:",
    "lea rsp,[rsp+0x88]",
    "pop rdi",
    "pop rbx",
    "ret",
    ".seh_endproc"
);
#[test]
fn fixed_terminal_bodies_and_chained_pdata_are_exact_primary_bytes() {
    let mut f = super::super::super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
    for body in fixed::FRAMES {
        let offset = pe_file_offset(&mut f, body.begin).unwrap();
        f.seek(SeekFrom::Start(offset)).unwrap();
        let mut b = vec![0; (body.end - body.begin) as usize];
        f.read_exact(&mut b).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&b)), body.sha);
        assert_eq!(super::super::unhex(body.specimen), b);
        let last = b.len() - 1;
        b[last] ^= 1;
        assert_ne!(format!("{:x}", Sha256::digest(&b)), body.sha);
    }
}
// Owned pinned bodies exercise the terminal carrier. Allocation/free observers,
// network/thread creation and empty child storage are fixture leaves; remove,
// typed Session destruction, transport close and98C650 wait execute originals.
static LIVE_TRANSPORT: AtomicU64 = AtomicU64::new(0);
static LIVE_NETWORK: AtomicU64 = AtomicU64::new(0);
static LIVE_THREAD: AtomicU64 = AtomicU64::new(0);
static LIVE_LIST: AtomicU64 = AtomicU64::new(0);
static LIVE_EVENT: AtomicU64 = AtomicU64::new(0);
static LIVE_BASE: AtomicU64 = AtomicU64::new(0);
static LIVE_FREE_HITS: AtomicU64 = AtomicU64::new(0);
static LIVE_WAIT_RETURNED: AtomicBool = AtomicBool::new(false);
static LIVE_TERMINAL_SOURCE: AtomicBool = AtomicBool::new(false);
#[link(name = "kernel32")]
unsafe extern "system" {
    fn RtlAddFunctionTable(table: *mut IMAGE_RUNTIME_FUNCTION_ENTRY, count: u32, base: u64) -> u8;
    fn RtlDeleteFunctionTable(table: *mut IMAGE_RUNTIME_FUNCTION_ENTRY) -> u8;
    #[link_name = "NATIVE_TRANSPORT_CONTINUE"]
    static LIVE_TRANSPORT_CONTINUE: [AtomicU64; 9];
    fn registry_live_network_body();
    fn registry_live_thread_body();
}
extern "system" fn live_allocator(size: u64) -> u64 {
    match size {
        0x48 => LIVE_TRANSPORT.load(Ordering::Acquire),
        0x120 => LIVE_NETWORK.load(Ordering::Acquire),
        0xd0 => LIVE_THREAD.load(Ordering::Acquire),
        0x10 => LIVE_LIST.load(Ordering::Acquire),
        _ => 0,
    }
}
extern "system" fn live_network_constructor(network: u64) -> u64 {
    unsafe {
        *(network as *mut u64) = LIVE_BASE.load(Ordering::Acquire) + 0x1049808;
    }
    network
}
extern "system" fn live_thread_constructor(thread: u64) -> u64 {
    unsafe {
        *(thread as *mut u64) = LIVE_BASE.load(Ordering::Acquire) + 0x106d030;
    }
    thread
}
extern "system" fn live_start(thread: u64, functor: u64, _: u64, _: u64) -> u8 {
    unsafe {
        *((thread + 8) as *mut u64) = read64(functor).unwrap();
        *((thread + 0x10) as *mut u64) = read64(functor + 8).unwrap();
        *((thread + 0xb0) as *mut u64) = LIVE_EVENT.load(Ordering::Acquire);
        *((thread + 0xce) as *mut u8) = 1;
        *((thread + 0xcd) as *mut u8) = 1;
    }
    1
}
extern "system" fn live_stop_probe(thread: u64) -> u64 {
    let lease = super::super::CALLS.with(|calls| {
        let calls = calls.borrow();
        assert!(!calls.iter().any(|c| c.kind == 74));
        let c = calls.last().expect("shared75 native wait frame");
        assert_eq!((c.kind, c.registry, c.thread), (75, 1, thread));
        c.lease.expect("terminal native lease")
    });
    let (_, target) = current_target().expect("active typed Session destructor");
    assert_eq!(target.lease, lease);
    assert_eq!(target.transport.unwrap().thread, thread);
    LIVE_TERMINAL_SOURCE.store(true, Ordering::Release);
    0
}
extern "system" fn live_noop() -> u64 {
    0
}
extern "system" fn live_delete_thread(thread: u64) -> u64 {
    assert_eq!(thread, LIVE_THREAD.load(Ordering::Acquire));
    // Original98C650 must wait successfully and clear B0 before the native
    //8573B0 destructor continuation can execute.
    assert_eq!(read64(thread + 0xb0).unwrap(), 0);
    assert_eq!(copy_current::<1>(thread + 0xcd), Some([0]));
    let call = super::super::CALLS.with(|s| {
        s.borrow()
            .iter()
            .rev()
            .find(|c| c.kind == 75)
            .map(|c| (c.registry, c.lease))
    });
    assert!(
        call.is_none(),
        "75 full gateway must have returned before native deletion"
    );
    LIVE_WAIT_RETURNED.store(true, Ordering::Release);
    thread
}
extern "system" fn live_free_object(pointer: u64, _size: u64) -> u64 {
    if pointer == SESSION.load(Ordering::Acquire) {
        assert!(LIVE_WAIT_RETURNED.load(Ordering::Acquire));
        assert!(!super::super::state().retired_slot(REGISTRY.load(Ordering::Acquire), 17, pointer));
        LIVE_FREE_HITS.fetch_or(1, Ordering::AcqRel);
    }
    pointer
}
extern "system" fn live_free_node(_allocator: u64, pointer: u64, size: u64) -> u64 {
    assert_eq!(pointer, NODE.load(Ordering::Acquire));
    assert_eq!(size, 0x18);
    assert_eq!(read64(pointer + 8).unwrap(), 0);
    assert!(LIVE_WAIT_RETURNED.load(Ordering::Acquire));
    assert!(!super::super::state().retired_slot(
        REGISTRY.load(Ordering::Acquire),
        17,
        SESSION.load(Ordering::Acquire)
    ));
    LIVE_FREE_HITS.fetch_or(2, Ordering::AcqRel);
    pointer
}
global_asm!(
    ".globl registry_live_network_body",".seh_proc registry_live_network_body","registry_live_network_body:",
    ".seh_savereg rbx,0x10","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "call {network_ctor}","mov rbx,[rsp+0x38]","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".globl registry_live_thread_body",".seh_proc registry_live_thread_body","registry_live_thread_body:",
    ".seh_savereg rbx,8","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "call {thread_ctor}","mov rbx,[rsp+0x30]","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    network_ctor=sym live_network_constructor,thread_ctor=sym live_thread_constructor,
);
struct LiveFixture {
    base: usize,
    pdata: *mut IMAGE_RUNTIME_FUNCTION_ENTRY,
    owned: Vec<*mut u8>,
    event: windows_sys::Win32::Foundation::HANDLE,
    registry: u64,
    session: u64,
    lease: SessionLease,
}
impl LiveFixture {
    fn new() -> Self {
        use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
        let mut file =
            super::super::super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
        let mut image = Vec::new();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.read_to_end(&mut image).unwrap();
        let u16_at = |p: usize| u16::from_le_bytes(image[p..p + 2].try_into().unwrap());
        let u32_at = |p: usize| u32::from_le_bytes(image[p..p + 4].try_into().unwrap());
        let nt = u32_at(0x3c) as usize;
        let opt = nt + 24;
        let count = u16_at(nt + 6) as usize;
        let section_table = opt + u16_at(nt + 20) as usize;
        let len = u32_at(opt + 56) as usize;
        let base =
            unsafe { VirtualAlloc(null(), len, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) } as usize;
        assert_ne!(base, 0);
        unsafe {
            ptr::copy_nonoverlapping(image.as_ptr(), base as *mut u8, u32_at(opt + 60) as usize);
        }
        let mut text = (0usize, 0usize);
        for i in 0..count {
            let row = section_table + i * 40;
            let rva = u32_at(row + 12) as usize;
            let raw = u32_at(row + 20) as usize;
            let n = u32_at(row + 16) as usize;
            assert!(rva + n <= len);
            assert!(raw + n <= image.len());
            unsafe {
                ptr::copy_nonoverlapping(image[raw..].as_ptr(), (base + rva) as *mut u8, n);
            }
            if &image[row..row + 5] == b".text" {
                text = (rva, n);
            }
        }
        // Native vtables contain preferred-base absolute pointers. Relocate
        // only this owned image before exercising its original virtual calls.
        let preferred = u64::from_le_bytes(image[opt + 24..opt + 32].try_into().unwrap());
        let delta = (base as u64).wrapping_sub(preferred);
        let reloc = u32_at(opt + 112 + 5 * 8) as usize;
        let reloc_end = reloc + u32_at(opt + 116 + 5 * 8) as usize;
        let mut pos = reloc;
        while pos < reloc_end {
            let page = unsafe { *((base + pos) as *const u32) } as usize;
            let size = unsafe { *((base + pos + 4) as *const u32) } as usize;
            assert!(size >= 8 && pos + size <= reloc_end);
            for at in (pos + 8..pos + size).step_by(2) {
                let entry = unsafe { *((base + at) as *const u16) };
                match entry >> 12 {
                    0 => {}
                    10 => {
                        let target = (base + page + usize::from(entry & 0xfff)) as *mut u64;
                        unsafe {
                            ptr::write_unaligned(
                                target,
                                ptr::read_unaligned(target).wrapping_add(delta),
                            );
                        }
                    }
                    other => panic!("unsupported owned relocation {other}"),
                }
            }
            pos += size;
        }
        // The fixed Host has absolute table slots without relocation rows.
        // Rebase only the typed slots exercised by this owned fixture.
        for (slot, target) in [
            (0x1045fe0, 0x83ade0),
            (0x1045bd8, 0x836a90),
            (0x1047eb0, 0x84e4e0),
            (0x1047eb8, 0x84ec00),
            (0x1047ea0, 0x84ebd0),
            (0x10494a8, 0x85f9f0),
            (0x106d038, 0x98c170),
            (0x106d060, 0x98c650),
        ] {
            let cell = (base + slot) as *mut u64;
            assert_eq!(unsafe { *cell }, preferred + target);
            unsafe {
                *cell = base as u64 + target;
            }
        }
        let pdata =
            (base + u32_at(opt + 112 + 3 * 8) as usize) as *mut IMAGE_RUNTIME_FUNCTION_ENTRY;
        assert_ne!(
            unsafe { RtlAddFunctionTable(pdata, u32_at(opt + 116 + 3 * 8) / 12, base as u64) },
            0
        );
        let mut owned = Vec::new();
        for _ in 0..8 {
            let p =
                unsafe { VirtualAlloc(null(), 0x1000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
                    as *mut u8;
            assert!(!p.is_null());
            owned.push(p);
        }
        let registry = owned[0] as u64;
        let session = owned[1] as u64;
        let buckets = owned[2] as u64;
        let node = owned[3] as u64;
        LIVE_BASE.store(base as u64, Ordering::Release);
        LIVE_TRANSPORT.store(owned[4] as u64, Ordering::Release);
        LIVE_NETWORK.store(owned[5] as u64, Ordering::Release);
        LIVE_THREAD.store(owned[6] as u64, Ordering::Release);
        LIVE_LIST.store(owned[7] as u64, Ordering::Release);
        let callback = session + 0x300;
        let event = unsafe { CreateEventW(null_mut(), 1, 1, null_mut()) };
        assert!(!event.is_null());
        LIVE_EVENT.store(event as u64, Ordering::Release);
        unsafe {
            *((registry + 0x10) as *mut u64) = buckets;
            *((registry + 0x18) as *mut u32) = 1;
            *((registry + 0x1c) as *mut u32) = 1;
            *(buckets as *mut u64) = node;
            *((buckets + 8) as *mut u64) = buckets + 8;
            *(node as *mut u32) = 17;
            *((node + 8) as *mut u64) = session;
            *(session as *mut u64) = base as u64 + 0x1045bd8;
            *((session + 0x30) as *mut u64) = callback;
            *(callback as *mut u64) = base as u64 + 0x10494a8;
            *((callback + 8) as *mut u32) = 17;
            *((base + 0xf97750) as *mut u64) = WaitForSingleObject as *const () as u64;
            *((base + 0xf979e8) as *mut u64) = GetLastError as *const () as u64;
        }
        REGISTRY.store(registry, Ordering::Release);
        SESSION.store(session, Ordering::Release);
        NODE.store(node, Ordering::Release);
        super::super::EXE.store(base as u64, Ordering::Release);
        REGISTRIES.lock().unwrap().insert(registry, 7);
        let lease = {
            let mut s = super::super::state();
            let l = s.birth(registry, Some(7), 9, 1).unwrap();
            s.factory(l, 17, 9, session, callback).unwrap();
            s.published(l, 17).unwrap();
            l
        };
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect((base + text.0) as _, text.1, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        unsafe {
            super::super::verify_loaded(base).unwrap();
            prepare_host(base).unwrap();
            transports::prepare_host(base).unwrap();
            super::super::install_at(base).unwrap();
        }
        // Resolve only fixture leaves. Verified bodies themselves remain the
        // pinned originals, including7F9220/836A90/836940/845BE0/98C650.
        unsafe {
            VirtualProtect(
                (base + text.0) as _,
                text.1,
                PAGE_EXECUTE_READWRITE,
                &mut old,
            );
        }
        for (rva, target) in [
            (0x9825d0, live_allocator as *const () as u64),
            (0x9825f0, live_noop as *const () as u64),
            (0xf64260, live_noop as *const () as u64),
            (0x98c170, live_start as *const () as u64),
            (0x159f10, live_free_object as *const () as u64),
            (0xa51f90, live_free_node as *const () as u64),
            (0x84ec50, live_noop as *const () as u64),
        ] {
            let mut code = vec![0xff, 0x25, 0, 0, 0, 0];
            code.extend_from_slice(&target.to_le_bytes());
            unsafe {
                ptr::copy_nonoverlapping(code.as_ptr(), (base + rva) as *mut u8, code.len());
            }
        }
        // Local transport close enters the original8573B0 and its exact75
        // caller8573C9; only destructor/no-op leaves are replaced.
        unsafe {
            *((base + 0x1047e60) as *mut u64) = live_noop as *const () as u64;
            *((base + 0x1047e78) as *mut u64) = live_noop as *const () as u64;
            *((base + 0x1049808) as *mut u64) = live_noop as *const () as u64;
            *((base + 0x1049810) as *mut u64) = live_noop as *const () as u64;
            *((base + 0x106d030) as *mut u64) = live_delete_thread as *const () as u64;
            *((base + 0x106d050) as *mut u64) = live_stop_probe as *const () as u64;
            *((base + 0x106d070) as *mut u64) = live_noop as *const () as u64;
            LIVE_TRANSPORT_CONTINUE[4].store(
                registry_live_network_body as *const () as u64,
                Ordering::Release,
            );
            LIVE_TRANSPORT_CONTINUE[7].store(
                registry_live_thread_body as *const () as u64,
                Ordering::Release,
            );
        }
        // The nativefactory must see one of its exact original return PCs.
        let mut driver = vec![
            0x53, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8b, 0xd9, 0x48, 0x8b, 0x01, 0xba, 1, 0, 0, 0,
        ];
        driver.resize(0x1d, 0x90);
        driver.extend_from_slice(&[0xff, 0x90, 0x08, 0x04, 0, 0]);
        assert_eq!(0x83fc40 + driver.len(), 0x83fc63);
        driver.extend_from_slice(&[
            0x48, 0x89, 0x43, 0x28, 0x48, 0x8b, 0xc8, 0xba, 1, 0, 0, 0, 0x45, 0x33, 0xc0, 0x45,
            0x33, 0xc9, 0xc7, 0x44, 0x24, 0x20, 0, 0, 0, 0,
        ]);
        let call = 0x83fc40 + driver.len();
        driver.push(0xe8);
        driver.extend_from_slice(&((0x84e480i64 - (call + 5) as i64) as i32).to_le_bytes());
        driver.extend_from_slice(&[0x48, 0x83, 0xc4, 0x30, 0x5b, 0xc3]);
        unsafe {
            ptr::copy_nonoverlapping(driver.as_ptr(), (base + 0x83fc40) as *mut u8, driver.len());
            VirtualProtect((base + text.0) as _, text.1, PAGE_EXECUTE_READ, &mut old);
            FlushInstructionCache(GetCurrentProcess(), (base + text.0) as _, text.1);
        }
        EXE.store(base as u64, Ordering::Release);
        Self {
            base,
            pdata,
            owned,
            event,
            registry,
            session,
            lease,
        }
    }
    fn create_transport(&self) {
        let driver: unsafe extern "system" fn(u64) -> u64 =
            unsafe { std::mem::transmute(self.base + 0x83fc40) };
        assert_eq!(unsafe { driver(self.session) } as u8, 1);
        let t = super::super::state().transport(self.lease).unwrap();
        assert_eq!(
            (t.pointer, t.network, t.thread),
            (
                LIVE_TRANSPORT.load(Ordering::Acquire),
                LIVE_NETWORK.load(Ordering::Acquire),
                LIVE_THREAD.load(Ordering::Acquire)
            )
        );
        assert_eq!(
            transports::from_network(t.network).unwrap(),
            Some((self.lease, t))
        );
    }
}
impl Drop for LiveFixture {
    fn drop(&mut self) {
        unsafe {
            restore().unwrap();
            transports::restore().unwrap();
            super::super::restore().unwrap();
            RtlDeleteFunctionTable(self.pdata);
            windows_sys::Win32::Foundation::CloseHandle(self.event);
            VirtualFree((self.base as *mut u8).cast(), 0, MEM_RELEASE);
        }
        for p in &self.owned {
            unsafe { VirtualFree((*p).cast(), 0, MEM_RELEASE) };
        }
    }
}
#[test]
fn original_remove_live_transport_uses_shared_75_wait_then_native_node_free_and_full_return() {
    if !child(
        "original_remove_live_transport_uses_shared_75_wait_then_native_node_free_and_full_return",
    ) {
        return;
    }
    let f = LiveFixture::new();
    f.create_transport();
    let remove: unsafe extern "system" fn(u64, u32) =
        unsafe { std::mem::transmute(f.base + 0x7f9220) };
    unsafe { remove(f.registry, 17) };
    assert!(LIVE_WAIT_RETURNED.load(Ordering::Acquire));
    assert!(LIVE_TERMINAL_SOURCE.load(Ordering::Acquire));
    assert_eq!(LIVE_FREE_HITS.load(Ordering::Acquire), 3);
    assert_eq!(read64(f.session + 0x28).unwrap(), 0);
    assert_eq!(read64(f.session + 0x30).unwrap(), 0);
    assert_eq!(read64(NODE.load(Ordering::Acquire) + 8).unwrap(), 0);
    assert_eq!(read64(f.registry + 0x10).and_then(read64), Ok(0));
    assert!(super::super::state().retired_slot(f.registry, 17, f.session));
    assert!(super::super::CALLS.with(|s| s.borrow().is_empty()));
    assert!(CALLS.with(|s| s.borrow().is_empty()));
}
