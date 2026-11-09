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
