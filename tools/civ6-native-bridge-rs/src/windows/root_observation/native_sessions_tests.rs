use super::*;
pub(super) fn caller_override(rva: u64) -> Option<u64> {
    if !FIXTURE.load(Ordering::Acquire) {
        return None;
    }
    match rva {
        0x7f8cf5 => Some(session_fixture_factory_return as *const () as u64),
        0x8573c9 => Some(session_fixture_join_return as *const () as u64),
        _ => None,
    }
}
static FIXTURE: AtomicBool = AtomicBool::new(false);
use std::os::windows::process::CommandExt;
use std::sync::atomic::AtomicUsize;
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlLookupFunctionEntry, IMAGE_RUNTIME_FUNCTION_ENTRY,
};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetErrorMode(mode: u32) -> u32;
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
fn child(name: &str) -> bool {
    const ENV: &str = "CIV6_NATIVE_SESSION_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    let mut p = std::process::Command::new(std::env::current_exe().unwrap())
        .creation_flags(0x08000000)
        .env(ENV, "1")
        .args([
            "--exact",
            &format!("windows::root_observation::native_frames::sessions::tests::{name}"),
            "--nocapture",
        ])
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(s) = p.try_wait().unwrap() {
            assert!(s.success(), "owned session child {s}");
            return false;
        }
        if Instant::now() >= deadline {
            let _ = p.kill();
            let _ = p.wait();
            panic!("session child timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
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
static SESSION: AtomicU64 = AtomicU64::new(0);
static CALLBACK: AtomicU64 = AtomicU64::new(0);
static SESSION_ID: AtomicU64 = AtomicU64::new(17);
static BIRTH_HITS: AtomicUsize = AtomicUsize::new(0);
static JOIN_AL: AtomicU64 = AtomicU64::new(1);
static LATE_CONTINUATION: AtomicBool = AtomicBool::new(false);
static ADMISSION_HITS: AtomicUsize = AtomicUsize::new(0);
static HOME: AtomicU64 = AtomicU64::new(0);
static ARG5: AtomicU64 = AtomicU64::new(0);
static FACTORY_OBSERVED: AtomicU64 = AtomicU64::new(0);
static ADMISSION_OBSERVED: AtomicU64 = AtomicU64::new(0);
extern "system" fn factory_body(id: u32, owner: u64) -> u64 {
    let session = SESSION.load(Ordering::Acquire);
    let callback = CALLBACK.load(Ordering::Acquire);
    let mut observed = u64::from(unsafe { GetLastError() } == 0x1234);
    observed |= u64::from(read64(owner + 8).ok() == Some(0)) << 1;
    let lease = CALLS.with(|s| s.borrow().last().and_then(|s| s.lease));
    observed |= u64::from(lease.is_some_and(|l| state().snapshot(l) == Err(Error::Unbound))) << 2;
    FACTORY_OBSERVED.store(observed, Ordering::Release);
    unsafe {
        *(session as *mut u64) = EXE.load(Ordering::Acquire) + 0x1043088;
        *((session + 0x30) as *mut u64) = callback;
        *(callback as *mut u64) = EXE.load(Ordering::Acquire) + 0x10494a8;
        *((callback + 8) as *mut u32) = id;
    }
    BIRTH_HITS.fetch_add(1, Ordering::AcqRel);
    session
}
extern "system" fn admission_body() -> u64 {
    ADMISSION_HITS.fetch_add(1, Ordering::AcqRel);
    let world = NativeSessionApi::current_world();
    if let Some(world) = world {
        let mut observed = 1;
        observed |= u64::from(
            FRAMES
                .get()
                .unwrap()
                .seal_and_wait(Instant::now() + Duration::from_secs(1))
                == Err(crate::native_flight::Error::Reentrant),
        ) << 1;
        observed |= u64::from(NativeSessionApi::drained(world) == Err(Error::Busy)) << 2;
        ADMISSION_OBSERVED.store(observed, Ordering::Release);
    }
    unsafe { SetLastError(0x7788) };
    0x445566
}
extern "system" fn after_join() {
    LATE_CONTINUATION.store(true, Ordering::Release);
    if std::env::var_os("CIV6_NATIVE_SESSION_EXPECT_AL0").is_some() {
        eprintln!("session-destructive-continuation-reached");
    }
}
extern "system" fn join_body() -> u64 {
    let al = JOIN_AL.load(Ordering::Acquire);
    if std::env::var_os("CIV6_NATIVE_SESSION_EXPECT_AL0").is_some() {
        eprintln!("session-original-thread-join-AL{al}-reached");
    }
    al
}
global_asm!(
    ".p2align 4",".globl session_fixture_register",".seh_proc session_fixture_register","session_fixture_register:",
    "mov [rsp+0x10],rbx",".seh_savereg rbx,0x10","push rdi",".seh_pushreg rdi",
    "sub rsp,0x50",".seh_stackalloc 0x50",".seh_endprologue",
    "mov rbx,rcx","mov rdi,r8","mov rax,[rsp+0x60]","mov [rip+{home}],rax",
    "mov r9,r8","mov r8d,[rip+{id}]","call {factory}",
    ".globl session_fixture_factory_return","session_fixture_factory_return:",
    "mov [rdi+8],rax","mov rdx,[rbx+0x10]","mov rdx,[rdx]","mov [rdx+8],rax",
    "mov eax,[rip+{id}]","mov rbx,[rsp+0x68]","lea rsp,[rsp+0x50]","pop rdi","ret",".seh_endproc",
    ".p2align 4",".globl session_fixture_factory",".seh_proc session_fixture_factory","session_fixture_factory:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "mov ecx,r8d","mov rdx,r9","call {factory_body}",
    "mov rbx,[rsp+0x30]","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".p2align 4",".globl session_fixture_incoming",".seh_proc session_fixture_incoming","session_fixture_incoming:",
    "mov [rsp+0x20],rbx",".seh_savereg rbx,0x20","push rdi",".seh_pushreg rdi",
    "push r14",".seh_pushreg r14","push r15",".seh_pushreg r15",
    "sub rsp,0x50",".seh_stackalloc 0x50",".seh_endprologue",
    "mov rax,[rsp+0x90]","mov [rip+{arg5}],rax",
    "mov rcx,[rip+{session}]","call {admission}",
    "mov rbx,[rsp+0x88]","lea rsp,[rsp+0x50]","pop r15","pop r14","pop rdi","ret",".seh_endproc",
    ".p2align 4",".globl session_fixture_admission",".seh_proc session_fixture_admission","session_fixture_admission:",
    ".byte 0x40,0x55",".seh_pushreg rbp","push rbx",".seh_pushreg rbx","push rsi",".seh_pushreg rsi",
    "push rdi",".seh_pushreg rdi","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "call {admission_body}","lea rsp,[rsp+0x28]","pop rdi","pop rsi","pop rbx","pop rbp","ret",".seh_endproc",
    ".p2align 4",".globl session_fixture_close",".seh_proc session_fixture_close","session_fixture_close:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","push rdi",".seh_pushreg rdi",
    "sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "mov rdi,rcx","mov rcx,[rcx+0x28]","mov rcx,[rcx+0x10]","mov rcx,[rcx+0xc0]",
    "mov edx,0xffffffff","call {join}",
    ".globl session_fixture_join_return","session_fixture_join_return:",
    "call {after_join}","mov rax,[rdi+0x28]","mov qword ptr [rax+0x10],0","mov qword ptr [rdi+0x28],0",
    "mov rbx,[rsp+0x30]","lea rsp,[rsp+0x20]","pop rdi","ret",".seh_endproc",
    ".p2align 4",".globl session_fixture_join",".seh_proc session_fixture_join","session_fixture_join:",
    "mov [rsp+0x10],rbx",".seh_savereg rbx,0x10","push rsi",".seh_pushreg rsi",
    "sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "call {join_body}","mov rbx,[rsp+0x38]","lea rsp,[rsp+0x20]","pop rsi","ret",".seh_endproc",
    ".globl session_home_driver",".seh_proc session_home_driver","session_home_driver:",
    "sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","mov qword ptr [rsp],0x5a",
    "call {register}","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    factory=sym native_session_factory_gateway,admission=sym native_session_admission_gateway,
    join=sym native_session_join_gateway,register=sym native_session_register_gateway,
    factory_body=sym factory_body,admission_body=sym admission_body,after_join=sym after_join,
    home=sym HOME,arg5=sym ARG5,id=sym SESSION_ID,session=sym SESSION,join_body=sym join_body,
);
unsafe extern "system" {
    fn session_fixture_register();
    fn session_fixture_factory();
    fn session_fixture_incoming();
    fn session_fixture_admission();
    fn session_fixture_close();
    fn session_fixture_join();
    fn session_fixture_factory_return();
    fn session_fixture_join_return();
    fn session_home_driver(registry: u64, kind: u64, owner: u64) -> u64;
}
struct Fixture {
    base: usize,
    registry: Box<[u64; 4]>,
    buckets: Box<[u64; 2]>,
    node: Box<[u64; 3]>,
    owner: Box<[u64; 2]>,
    session: Box<[u64; 80]>,
    callback: Box<[u64; 2]>,
    transport: Box<[u64; 16]>,
    network: Box<[u64; 26]>,
    thread: Box<[u64; 28]>,
}
impl Fixture {
    fn new() -> Self {
        FRAMES.get_or_init(NativeFrames::new);
        FIXTURE.store(true, Ordering::Release);
        let base = unsafe {
            VirtualAlloc(
                null_mut(),
                0x2360000,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        } as usize;
        assert_ne!(base, 0);
        EXE.store(base as u64, Ordering::Release);
        CORE.store(1, Ordering::Release);
        assert!(SCOPE
            .set(SessionScopeHooks {
                enter_existing_epoch: scope_enter,
                leave: scope_leave
            })
            .is_ok());
        let mut x = Self {
            base,
            registry: Box::new([0; 4]),
            buckets: Box::new([0; 2]),
            node: Box::new([17, 0, 0]),
            owner: Box::new([0; 2]),
            session: Box::new([0; 80]),
            callback: Box::new([0; 2]),
            transport: Box::new([0; 16]),
            network: Box::new([0; 26]),
            thread: Box::new([0; 28]),
        };
        x.registry[0] = base as u64 + 0x1000000;
        x.registry[2] = x.buckets.as_ptr() as u64;
        x.registry[3] = 1;
        x.buckets[0] = x.node.as_ptr() as u64;
        x.session[0] = base as u64 + 0x1043088;
        x.session[5] = x.transport.as_ptr() as u64;
        x.transport[0] = base as u64 + 0x1047e60;
        x.transport[2] = x.network.as_ptr() as u64;
        x.transport[8] = 0x10000000000; // byte+45=1
        x.network[24] = x.thread.as_ptr() as u64;
        x.thread[0] = base as u64 + 0x106d030;
        unsafe {
            *((base + 0x1000018) as *mut u64) = (base + 0x7f8ab0) as u64;
            *((base + 0x10432c0) as *mut u64) = (base + 0x842030) as u64;
            *((base + 0x1047ea0) as *mut u64) = (base + 0x84ebd0) as u64;
            *((base + 0x1047eb8) as *mut u64) = (base + 0x84ec00) as u64;
            *((base + 0x106d060) as *mut u64) = (base + 0x98c650) as u64;
            *((base + 0x235b878) as *mut u64) = x.registry.as_ptr() as u64;
        }
        SESSION.store(x.session.as_ptr() as u64, Ordering::Release);
        CALLBACK.store(x.callback.as_ptr() as u64, Ordering::Release);
        for (i, p) in [
            session_fixture_register as *const (),
            session_fixture_factory as *const (),
            session_fixture_incoming as *const (),
            session_fixture_admission as *const (),
            session_fixture_close as *const (),
            session_fixture_join as *const (),
        ]
        .into_iter()
        .enumerate()
        {
            NATIVE_SESSION_CONTINUE[i].store(p as u64 + 5, Ordering::Release);
        }
        x
    }
    fn birth(&mut self, lifetime: bool) -> Option<Lease> {
        self.owner[1] = 0;
        self.node[0] = 17;
        self.node[1] = 0;
        unsafe { SetLastError(0x1234) };
        let call = || unsafe {
            session_home_driver(self.registry.as_ptr() as u64, 2, self.owner.as_ptr() as u64)
        };
        let id = if lifetime {
            NativeSessionApi::with_registry(self.registry.as_ptr() as u64, 9, call).unwrap()
        } else {
            call()
        };
        assert_eq!(id, 17);
        assert_eq!(HOME.load(Ordering::Acquire), 0x5a);
        assert_eq!(BIRTH_HITS.load(Ordering::Acquire), 1);
        assert_eq!(
            FACTORY_OBSERVED.load(Ordering::Acquire),
            7,
            "factory before-publication and input LastError witness"
        );
        assert_eq!(self.owner[1], self.session.as_ptr() as u64);
        assert_eq!(self.node[1], self.session.as_ptr() as u64);
        let lifetime = if lifetime {
            Some(9)
        } else {
            REGISTRIES
                .lock()
                .unwrap()
                .get(&(self.registry.as_ptr() as u64))
                .copied()
        };
        lifetime.and_then(|lifetime| {
            NativeSessionApi::born(self.registry.as_ptr() as u64, lifetime, id as u32)
        })
    }
    fn bind(&self, lease: Lease) {
        NativeSessionApi::bind_world(
            lease,
            WorldReceipt {
                binding_id: 8,
                epoch: 1,
            },
        )
        .unwrap();
        NativeSessionApi::bind_transport(
            lease,
            self.transport.as_ptr() as u64,
            self.network.as_ptr() as u64,
            self.thread.as_ptr() as u64,
        )
        .unwrap();
    }
    fn incoming(&self) -> u64 {
        let f: unsafe extern "system" fn(u64, u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(gateways()[2]) };
        unsafe { f(self.callback.as_ptr() as u64, 0, 0, 0, 0x8877665544332211) }
    }
    fn close(&self) {
        let f: unsafe extern "system" fn(u64) = unsafe { std::mem::transmute(gateways()[4]) };
        unsafe { f(self.session.as_ptr() as u64) };
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_ne!(unsafe { VirtualFree(self.base as _, 0, MEM_RELEASE) }, 0);
    }
}

#[test]
fn actual_birth_before_publication_delayed_dispatch_join_and_stale_ticket() {
    if !child("actual_birth_before_publication_delayed_dispatch_join_and_stale_ticket") {
        return;
    }
    let mut x = Fixture::new();
    let lease = x.birth(true).unwrap();
    x.bind(lease);
    let ticket = NativeSessionApi::reserve_message(lease).unwrap();
    assert_eq!(
        NativeSessionApi::with_message(ticket, || x.incoming()).unwrap(),
        0x445566
    );
    assert_eq!(ARG5.load(Ordering::Acquire), 0x8877665544332211);
    assert_eq!(unsafe { GetLastError() }, 0x7788);
    assert_eq!(ADMISSION_HITS.load(Ordering::Acquire), 1);
    assert_eq!(
        ADMISSION_OBSERVED.load(Ordering::Acquire),
        7,
        "complete incoming hold remains active in admission"
    );
    assert!(NativeSessionApi::current_world().is_none());
    x.close();
    assert!(LATE_CONTINUATION.load(Ordering::Acquire));
    assert_eq!(
        NativeSessionApi::drained(WorldReceipt {
            binding_id: 8,
            epoch: 1
        }),
        Ok(())
    );
    assert_eq!(
        NativeSessionApi::with_message(ticket, || ()),
        Err(Error::Unknown)
    );
    let f = FRAMES.get().unwrap();
    f.seal_and_wait(Instant::now() + Duration::from_secs(1))
        .unwrap();
    f.reopen().unwrap();
    assert_eq!(state().world(lease).unwrap().epoch, 1);
    assert_eq!(NativeSessionApi::reserve_message(lease), Err(Error::Closed));
    assert!(CALLS.with(|s| s.borrow().is_empty()));
}
#[test]
fn actual_unbound_birth_and_unknown_dispatch_forward_original_once() {
    if !child("actual_unbound_birth_and_unknown_dispatch_forward_original_once") {
        return;
    }
    let mut x = Fixture::new();
    assert!(x.birth(false).is_none());
    assert_eq!(x.incoming(), 0x445566);
    assert_eq!(ADMISSION_HITS.load(Ordering::Acquire), 1);
    assert!(NativeSessionApi::current_world().is_none());
    assert_eq!(
        NativeSessionApi::drained(WorldReceipt {
            binding_id: 8,
            epoch: 1
        }),
        Err(Error::Unknown)
    );
}
#[test]
fn unbound_birth_never_uses_message_or_current_world_as_authority() {
    let mut s = Ledger::default();
    let l = s.birth(0x10, None, 0x20, 2).unwrap();
    s.factory(l, 7, 0x20, 0x30, 0x40).unwrap();
    s.published(l, 7).unwrap();
    assert_eq!(
        s.bind_world(
            l,
            WorldReceipt {
                binding_id: 1,
                epoch: 1
            }
        ),
        Err(Error::Unbound)
    );
    assert_eq!(s.reserve(l), Err(Error::Unbound));
    assert_eq!(
        s.drained(WorldReceipt {
            binding_id: 1,
            epoch: 1
        }),
        Err(Error::Unknown)
    );
}

#[test]
fn actual_al0_join_aborts_before_original_destructive_continuation() {
    const NAME: &str = "actual_al0_join_aborts_before_original_destructive_continuation";
    const ENV: &str = "CIV6_NATIVE_SESSION_EXPECT_AL0";
    if std::env::var_os(ENV).is_some() {
        unsafe { SetErrorMode(3) };
        let mut x = Fixture::new();
        let lease = x.birth(true).unwrap();
        x.bind(lease);
        JOIN_AL.store(0, Ordering::Release);
        x.close();
        panic!("AL0 unexpectedly reached normal close return");
    }
    let mut p = std::process::Command::new(std::env::current_exe().unwrap())
        .creation_flags(0x08000000)
        .env(ENV, "1")
        .args([
            "--exact",
            &format!("windows::root_observation::native_frames::sessions::tests::{NAME}"),
            "--nocapture",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if p.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = p.kill();
            let _ = p.wait();
            panic!("AL0 fixture timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let out = p.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "AL0 was accepted");
    assert!(
        stderr.contains("session-original-thread-join-AL0-reached"),
        "wrong failure: {stderr}"
    );
    assert!(
        !stderr.contains("session-destructive-continuation-reached"),
        "returned to unsafe original caller"
    );
    assert!(
        !stderr.contains("panicked"),
        "fixture assertion failed before intended fatal boundary: {stderr}"
    );
}

global_asm!(
    ".globl session_unwind_driver",".seh_proc session_unwind_driver","session_unwind_driver:",
    "push rbx",".seh_pushreg rbx","sub rsp,0x80",".seh_stackalloc 0x80",".seh_endprologue",
    "mov rbx,rcx","mov [rcx],rsp","lea rax,[rip+session_unwind_landed]","mov [rcx+8],rax",
    "mov rcx,r8","call rdx","xor eax,eax","jmp 2f",
    "session_unwind_landed:","mov eax,1","2:","lea rsp,[rsp+0x80]","pop rbx","ret",".seh_endproc",
    ".p2align 4",".globl session_unwind_body",".seh_proc session_unwind_body","session_unwind_body:",
    "mov [rsp+0x20],rbx",".seh_savereg rbx,0x20",
    "sub rsp,0x528",".seh_stackalloc 0x528",".seh_endprologue",
    "mov rdx,[rcx+8]","mov rcx,[rcx]","lea r10,[rsp+0x40]","mov r11d,0x9a",
    "1:","mov qword ptr [r10],0","lea r10,[r10+8]","dec r11d","jnz 1b",
    "lea r10,[rsp+0x40]","mov [rsp+0x20],r10","mov qword ptr [rsp+0x28],0",
    "xor r8d,r8d","xor r9d,r9d","call {unwind_ex}","int3",".seh_endproc",
    unwind_ex=sym RtlUnwindEx,
);
unsafe extern "system" {
    fn session_unwind_driver(state: *mut u64, target: usize, argument: *mut u64) -> u64;
    fn session_unwind_body();
    fn native_session_register_original_body();
    fn native_session_factory_original_body();
    fn native_session_incoming_original_body();
    fn native_session_admission_original_body();
    fn native_session_close_original_body();
    fn native_session_join_original_body();
}
#[test]
fn actual_terminal_seh_releases_frame_but_keeps_session_unresolved() {
    if !child("actual_terminal_seh_releases_frame_but_keeps_session_unresolved") {
        return;
    }
    let mut x = Fixture::new();
    let lease = x.birth(true).unwrap();
    x.bind(lease);
    let ticket = NativeSessionApi::reserve_message(lease).unwrap();
    // Use an explicitly retained ticket to isolate the existing adapter's
    // terminal unwind contract from later automatic receive wiring.
    let mut landing = [0u64; 2];
    let lease_callback = CALLBACK.load(Ordering::Acquire);
    // Keep the real callback for lease matching. A dedicated original body
    // obtains unwind state from process-local fixture storage instead.
    UNWIND_STATE.store(landing.as_mut_ptr() as u64, Ordering::Release);
    NATIVE_SESSION_CONTINUE[2].store(
        session_unwind_incoming as *const () as u64 + 5,
        Ordering::Release,
    );
    let result = NativeSessionApi::with_message(ticket, || unsafe {
        session_unwind_driver(
            landing.as_mut_ptr(),
            gateways()[2],
            lease_callback as *mut u64,
        )
    })
    .unwrap();
    assert_eq!(result, 1);
    assert!(CALLS.with(|s| s.borrow().is_empty()));
    assert!(!FRAMES.get().unwrap().healthy());
    assert_eq!(
        NativeSessionApi::drained(WorldReceipt {
            binding_id: 8,
            epoch: 1
        }),
        Err(Error::Unresolved)
    );
    assert_eq!(
        NativeSessionApi::reserve_message(lease),
        Err(Error::Unresolved)
    );
}
static UNWIND_STATE: AtomicU64 = AtomicU64::new(0);
global_asm!(
    ".p2align 4",".globl session_unwind_incoming",".seh_proc session_unwind_incoming","session_unwind_incoming:",
    "mov [rsp+0x20],rbx",".seh_savereg rbx,0x20","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "mov rcx,[rip+{state}]","call {body}","int3",".seh_endproc",
    state=sym UNWIND_STATE,body=sym session_unwind_body,
);
unsafe extern "system" {
    fn session_unwind_incoming();
}

#[test]
fn six_displaced_shims_virtual_unwind_restore_return_rsp_and_saved_registers() {
    let entries = [
        native_session_register_original_body as *const (),
        native_session_factory_original_body as *const (),
        native_session_incoming_original_body as *const (),
        native_session_admission_original_body as *const (),
        native_session_close_original_body as *const (),
        native_session_join_original_body as *const (),
    ];
    for (i, address) in entries.into_iter().enumerate() {
        let mut stack = [0u64; 48];
        let entry_rsp = unsafe { stack.as_mut_ptr().add(12) } as u64;
        unsafe {
            *(entry_rsp as *mut u64) = 0x12345678;
        }
        let mut c: CONTEXT = unsafe { std::mem::zeroed() };
        c.Rip = address as u64;
        c.Rsp = entry_rsp;
        if i == 3 {
            c.Rsp -= 32;
            for (j, value) in [0xdddd, 0xcccc, 0xbbbb, 0xaaaa].into_iter().enumerate() {
                unsafe {
                    *((c.Rsp + j as u64 * 8) as *mut u64) = value;
                }
            }
        } else {
            let offset = [0x10, 8, 0x20, 0, 8, 0x10][i];
            unsafe {
                *((entry_rsp + offset) as *mut u64) = 0xbbbb;
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
            )
        };
        assert_eq!(c.Rip, 0x12345678, "shim{i}");
        assert_eq!(c.Rsp, entry_rsp + 8, "shim{i}");
        assert_eq!(c.Rbx, 0xbbbb, "shim{i}");
        if i == 3 {
            assert_eq!((c.Rbp, c.Rsi, c.Rdi), (0xaaaa, 0xcccc, 0xdddd));
        }
    }
}

#[test]
fn all_real_atomic_entries_restore_original_bytes_and_protection() {
    if !child("all_real_atomic_entries_restore_original_bytes_and_protection") {
        return;
    }
    assert!(supports_atomic_promotion_fetch());
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0x1080000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    for (i, (rva, word, _)) in SITES.into_iter().enumerate() {
        let code = cas_stub(i, word);
        unsafe { ptr::copy_nonoverlapping(code.as_ptr(), (base + rva) as *mut u8, code.len()) };
    }
    let mut old = 0;
    assert_ne!(
        unsafe { VirtualProtect(base as _, 0x1080000, PAGE_EXECUTE_READ, &mut old) },
        0
    );
    unsafe { install_at(base) }.unwrap();
    for (rva, word, _) in SITES {
        assert_ne!(copy_current::<8>((base + rva) as u64), Some(word));
    }
    let mut receiver = [0u64; 4];
    for (i, (rva, _, _)) in SITES.into_iter().enumerate() {
        if i == 6 {
            receiver[0] = (base + 0x1040200) as u64;
        }
        unsafe { SetLastError(0x1234) };
        let mut result = [0u64; 2];
        unsafe { session_cas_driver(base + rva, receiver.as_mut_ptr(), result.as_mut_ptr()) };
        assert_eq!(
            result[0],
            if i == 6 {
                receiver.as_ptr() as u64
            } else {
                0x8877665544332211
            },
            "site{i} original RAX"
        );
        assert_eq!(result[1] & 1, 1, "site{i} original carry flag");
        assert_eq!(unsafe { GetLastError() }, 0x1234, "site{i} LastError");
        assert_eq!(
            receiver[3],
            i as u64 + 1,
            "site{i} original body must execute exactly once"
        );
    }
    unsafe { NativeSessionApi::restore() }.unwrap();
    for (rva, word, _) in SITES {
        assert_eq!(copy_current::<8>((base + rva) as u64), Some(word));
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        assert_ne!(
            unsafe { VirtualQuery((base + rva) as _, &mut info, std::mem::size_of_val(&info)) },
            0
        );
        assert_eq!(info.Protect, PAGE_EXECUTE_READ);
    }
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
}

fn cas_stub(i: usize, word: [u8; 8]) -> Vec<u8> {
    let mut code = word.to_vec();
    match i {
        0 => code.extend([0x24, 0x18]), // finish native MOV [home+18],RSI
        1 => code.extend([0x24, 0x10]),
        2 => {}
        3 => code.push(0x56), // finish native pushR14 after pushR12
        4 | 5 | 7 => code.extend([0xec, 0x20]),
        6 => code.extend([0xec, 0x60]),
        _ => panic!("unexpected new site needs an ABI stub"),
    }
    code.extend([0x48, 0xff, 0x41, 0x18]); // original body invocation
    if i == 6 {
        code.extend([0x48, 0x8b, 0xc1]);
    } else {
        code.extend([0x48, 0xb8]);
        code.extend(0x8877665544332211u64.to_le_bytes());
    }
    match i {
        0 => code.extend(unhex("488b5c2410488b742418")),
        1 => code.extend(unhex("488b5c2408488b742410")),
        2 => code.extend(unhex("488b5c2430415e5f")),
        3 => code.extend(unhex("415e415c5f5e5b5d")),
        4 | 7 => code.extend(unhex("488b5c2430488d6424205f")),
        5 => code.extend(unhex("488b5c2438488d6424205e")),
        6 => code.extend(unhex("488b5c2470488d6424605f")),
        _ => unreachable!(),
    }
    code.extend([0xf9, 0xc3]);
    code
}
global_asm!(
    ".globl session_cas_driver",
    ".seh_proc session_cas_driver",
    "session_cas_driver:",
    "push rbx",
    ".seh_pushreg rbx",
    "sub rsp,0x20",
    ".seh_stackalloc 0x20",
    ".seh_endprologue",
    "mov rbx,r8",
    "mov r10,rcx",
    "mov rcx,rdx",
    "xor edx,edx",
    "xor r8d,r8d",
    "xor r9d,r9d",
    "call r10",
    "mov [rbx],rax",
    "pushfq",
    "pop rax",
    "mov [rbx+8],rax",
    "lea rsp,[rsp+0x20]",
    "pop rbx",
    "ret",
    ".seh_endproc",
);
unsafe extern "system" {
    fn session_cas_driver(target: usize, receiver: *mut u64, result: *mut u64);
}

fn data_offset(file: &mut File, rva: u32) -> Option<u64> {
    file.seek(SeekFrom::Start(0x3c)).ok()?;
    let mut raw = [0; 4];
    file.read_exact(&mut raw).ok()?;
    let nt = u32::from_le_bytes(raw) as u64;
    let mut short = [0; 2];
    file.seek(SeekFrom::Start(nt + 6)).ok()?;
    file.read_exact(&mut short).ok()?;
    let count = u16::from_le_bytes(short);
    file.seek(SeekFrom::Start(nt + 20)).ok()?;
    file.read_exact(&mut short).ok()?;
    let table = nt + 24 + u64::from(u16::from_le_bytes(short));
    for i in 0..count {
        file.seek(SeekFrom::Start(table + u64::from(i) * 40 + 12))
            .ok()?;
        let mut row = [0; 12];
        file.read_exact(&mut row).ok()?;
        let start = u32::from_le_bytes(row[..4].try_into().unwrap());
        let len = u32::from_le_bytes(row[4..8].try_into().unwrap());
        if rva >= start && rva.checked_sub(start)? < len {
            return Some(u64::from(
                u32::from_le_bytes(row[8..].try_into().unwrap()) + rva - start,
            ));
        }
    }
    None
}
fn file_bytes(file: &mut File, rva: u32, len: usize) -> Vec<u8> {
    let off = data_offset(file, rva).expect("primary section RVA");
    file.seek(SeekFrom::Start(off)).unwrap();
    let mut b = vec![0; len];
    file.read_exact(&mut b).unwrap();
    b
}
#[test]
#[ignore = "explicit pinned primary image check, separate from owned gateway execution"]
fn fixed_primary_complete_frames_entries_unwind_and_stack_contract_match() {
    let mut file = super::super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
    for f in fixed::FRAMES {
        let b = file_bytes(&mut file, f.begin, (f.end - f.begin) as usize);
        assert_eq!(format!("{:x}", Sha256::digest(&b)), f.sha);
        assert_eq!(b, unhex(f.specimen));
        for p in f.parts {
            assert_eq!(
                file_bytes(&mut file, p.unwind, p.hex.len() / 2),
                unhex(p.hex)
            );
        }
    }
    for (rva, word, stolen) in SITES {
        assert_eq!(file_bytes(&mut file, rva as u32, 8), word);
        assert_eq!(stolen, 5);
    }
    assert_eq!(file_bytes(&mut file, 0x7f8d40, 6), unhex("440fb64c2460"));
    assert_eq!(
        file_bytes(&mut file, 0x8600be, 9),
        unhex("440fb6a42490000000")
    );
    assert_eq!(file_bytes(&mut file, 0x98c6b2, 6), unhex("ff1598b06000"));
    assert_eq!(file_bytes(&mut file, 0x8573c6, 3), unhex("ff5030"));
}
#[test]
fn complete_primary_specimens_reject_every_code_byte_mutation_and_truncation() {
    let mut mutations = 0;
    for f in fixed::FRAMES {
        let mut b = unhex(f.specimen);
        assert_eq!(b.len(), (f.end - f.begin) as usize);
        assert_eq!(format!("{:x}", Sha256::digest(&b)), f.sha);
        for i in 0..b.len() {
            b[i] ^= 1;
            assert_ne!(
                format!("{:x}", Sha256::digest(&b)),
                f.sha,
                "frame{:x} byte{i}",
                f.begin
            );
            b[i] ^= 1;
            mutations += 1;
        }
        assert_ne!(format!("{:x}", Sha256::digest(&b[..b.len() - 1])), f.sha);
    }
    assert!(mutations > 4000, "nonempty complete code mutation set");
}

#[test]
fn transport_rebinding_id_wrap_and_live_address_reuse_cannot_settle_old_lease() {
    let mut s = Ledger::default();
    let l = s.birth(0x10, Some(1), 0x20, 2).unwrap();
    s.factory(l, u32::MAX, 0x20, 0x30, 0x40).unwrap();
    s.published(l, u32::MAX).unwrap();
    let world = WorldReceipt {
        binding_id: 8,
        epoch: 1,
    };
    s.bind_world(l, world).unwrap();
    let t = s.bind_transport(l, 0x50, 0x60, 0x70).unwrap();
    assert_eq!(s.bind_transport(l, 0x51, 0x61, 0x71), Err(Error::Stale));
    let packet = s.reserve(l).unwrap();
    let newer = s.birth(0x10, Some(1), 0x20, 2).unwrap();
    assert_eq!(
        s.factory(newer, u32::MAX, 0x20, 0x31, 0x41),
        Err(Error::Busy),
        "same live registry key"
    );
    assert_eq!(
        s.factory(newer, 0, 0x20, 0x30, 0x41),
        Err(Error::Busy),
        "same live session address"
    );
    s.close(l).unwrap();
    assert_eq!(s.reserve(l), Err(Error::Closed));
    assert_eq!(s.join(l, t, 0x70, 0), Err(Error::NotJoined));
    assert_eq!(s.closed(l), Err(Error::NotJoined));
    assert_eq!(
        s.join(l, t, 0x70, 1),
        Err(Error::Busy),
        "pending message still owns old incarnation"
    );
    assert_eq!(s.dispatch(packet, 0x41, u32::MAX), Err(Error::Stale));
    s.cancel_before_dispatch(packet).unwrap();
    assert_eq!(
        s.join(l, Transport { thread: 0x71, ..t }, 0x71, 1),
        Err(Error::Stale)
    );
    s.join(l, t, 0x70, 1).unwrap();
    s.closed(l).unwrap();
    assert_eq!(s.drained(world), Ok(()));
    s.factory(newer, 0, 0x20, 0x30, 0x40).unwrap();
    s.published(newer, 0).unwrap();
    s.bind_world(
        newer,
        WorldReceipt {
            binding_id: 9,
            epoch: 2,
        },
    )
    .unwrap();
    let t2 = s.bind_transport(newer, 0x50, 0x60, 0x70).unwrap();
    assert_ne!(t, t2);
    assert_eq!(s.world(l).unwrap().epoch, 1);
    assert_eq!(s.world(newer).unwrap().epoch, 2);
    assert_eq!(s.dispatch(packet, 0x40, 0), Err(Error::Unknown));
}

#[test]
fn failed_native_join_and_terminal_exception_never_produce_drain_receipt() {
    for al in [0, 2, 0xff] {
        let mut s = Ledger::default();
        let l = s.birth(0x10, Some(1), 0x20, 2).unwrap();
        s.factory(l, 1, 0x20, 0x30, 0x40).unwrap();
        s.published(l, 1).unwrap();
        let w = WorldReceipt {
            binding_id: 8,
            epoch: 1,
        };
        s.bind_world(l, w).unwrap();
        let t = s.bind_transport(l, 0x50, 0x60, 0x70).unwrap();
        s.close(l).unwrap();
        assert_eq!(s.join(l, t, 0x70, al), Err(Error::NotJoined));
        assert_eq!(s.closed(l), Err(Error::NotJoined));
        assert_eq!(s.drained(w), Err(Error::Busy));
        s.unresolved(Some(l));
        assert_eq!(s.drained(w), Err(Error::Unresolved));
    }
}

extern "system" fn registry_birth_body(registry: u64) -> u64 {
    unsafe {
        *(registry as *mut u64) = EXE.load(Ordering::Acquire) + 0x1040200;
    }
    registry
}
global_asm!(
    ".p2align 4",".globl session_fixture_registry_birth",".seh_proc session_fixture_registry_birth","session_fixture_registry_birth:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "call {body}","mov rbx,[rsp+0x30]","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    body=sym registry_birth_body,
);
unsafe extern "system" {
    fn session_fixture_registry_birth();
}
#[test]
fn actual_native_registry_birth_and_automatic_incoming_need_no_manual_ticket_scope() {
    if !child("actual_native_registry_birth_and_automatic_incoming_need_no_manual_ticket_scope") {
        return;
    }
    let mut x = Fixture::new();
    unsafe {
        *((x.base + 0x1040218) as *mut u64) = (x.base + 0x7f8ab0) as u64;
    }
    NATIVE_SESSION_CONTINUE[6].store(
        session_fixture_registry_birth as *const () as u64 + 5,
        Ordering::Release,
    );
    let ctor: unsafe extern "system" fn(u64) -> u64 = unsafe { std::mem::transmute(gateways()[6]) };
    assert_eq!(
        unsafe { ctor(x.registry.as_ptr() as u64) },
        x.registry.as_ptr() as u64
    );
    let lifetime = REGISTRIES
        .lock()
        .unwrap()
        .get(&(x.registry.as_ptr() as u64))
        .copied()
        .unwrap();
    assert_ne!(lifetime, 0);
    assert!(REGISTRY.with(|s| s.borrow().is_empty()));
    let lease = x
        .birth(false)
        .expect("actual constructor must establish registry lifetime");
    assert_eq!(
        NativeSessionApi::born(x.registry.as_ptr() as u64, lifetime, 17),
        Some(lease)
    );
    x.bind(lease);
    assert!(INPUT.with(|s| s.borrow().is_empty()));
    assert_eq!(x.incoming(), 0x445566);
    assert_eq!(ADMISSION_HITS.load(Ordering::Acquire), 1);
    assert_eq!(
        ADMISSION_OBSERVED.load(Ordering::Acquire),
        7,
        "native auto-reserve must preserve original epoch"
    );
    x.close();
    assert_eq!(
        NativeSessionApi::drained(WorldReceipt {
            binding_id: 8,
            epoch: 1
        }),
        Ok(())
    );
    assert!(INPUT.with(|s| s.borrow().is_empty()));
    assert!(CALLS.with(|s| s.borrow().is_empty()));
}

extern "system" fn thread_noop(_: u64) -> u64 {
    0
}
#[test]
fn actual_fixed_join_body_waits_on_owned_os_handle_before_disconnect_receipt() {
    if !child("actual_fixed_join_body_waits_on_owned_os_handle_before_disconnect_receipt") {
        return;
    }
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
    let mut x = Fixture::new();
    let lease = x.birth(true).unwrap();
    x.bind(lease);
    let code = unhex(
        fixed::FRAMES
            .iter()
            .find(|f| f.begin == 0x98c650)
            .unwrap()
            .specimen,
    );
    let event = unsafe { CreateEventW(null_mut(), 1, 1, null_mut()) };
    assert!(!event.is_null());
    unsafe {
        ptr::copy_nonoverlapping(code.as_ptr(), (x.base + 0x98c650) as *mut u8, code.len());
        *((x.base + 0xf97750) as *mut u64) = WaitForSingleObject as *const () as u64;
        *((x.base + 0xf979e8) as *mut u64) = GetLastError as *const () as u64;
        *((x.base + 0x106d050) as *mut u64) = thread_noop as *const () as u64;
        *((x.base + 0x106d070) as *mut u64) = thread_noop as *const () as u64;
        *((x.thread.as_mut_ptr() as u64 + 0xce) as *mut u8) = 1;
        *((x.thread.as_mut_ptr() as u64 + 0xcd) as *mut u8) = 1;
        *((x.thread.as_mut_ptr() as u64 + 0xb0) as *mut u64) = event as u64;
    }
    let mut old = 0;
    assert_ne!(
        unsafe {
            VirtualProtect(
                (x.base + 0x98c000) as _,
                0x1000,
                PAGE_EXECUTE_READ,
                &mut old,
            )
        },
        0
    );
    assert_ne!(
        unsafe { FlushInstructionCache(GetCurrentProcess(), (x.base + 0x98c650) as _, code.len()) },
        0
    );
    NATIVE_SESSION_CONTINUE[5].store(x.base as u64 + 0x98c655, Ordering::Release);
    x.close();
    assert!(LATE_CONTINUATION.load(Ordering::Acquire));
    assert_eq!(read64(x.thread.as_ptr() as u64 + 0xb0), Ok(0));
    assert_eq!(
        copy_current::<1>(x.thread.as_ptr() as u64 + 0xcd),
        Some([0])
    );
    assert_eq!(
        NativeSessionApi::drained(WorldReceipt {
            binding_id: 8,
            epoch: 1
        }),
        Ok(())
    );
    assert_ne!(unsafe { CloseHandle(event) }, 0);
}
