use super::*;
pub(super) fn caller_override(rva:u64)->Option<u64>{
    if !FIXTURE.load(Ordering::Acquire){return None;}
    match rva {
        0x7f8cf5=>Some(session_fixture_factory_return as *const () as u64),
        0x8573c9=>Some(session_fixture_join_return as *const () as u64),
        _=>None,
    }
}
static FIXTURE:AtomicBool=AtomicBool::new(false);
use std::os::windows::process::CommandExt;
use std::sync::atomic::AtomicUsize;
use windows_sys::Win32::System::Diagnostics::Debug::{RtlLookupFunctionEntry,IMAGE_RUNTIME_FUNCTION_ENTRY};
#[link(name="kernel32")]
unsafe extern "system" {
    fn RtlVirtualUnwind(kind:u32,base:u64,pc:u64,entry:*const IMAGE_RUNTIME_FUNCTION_ENTRY,
        context:*mut CONTEXT,data:*mut *mut c_void,frame:*mut u64,pointers:*mut c_void)->usize;
    fn RtlUnwindEx(frame:*const c_void,ip:*const c_void,record:*const EXCEPTION_RECORD,
        value:*const c_void,context:*const CONTEXT,history:*const c_void);
}
fn child(name:&str)->bool {
    const ENV:&str="CIV6_NATIVE_SESSION_CHILD";
    if std::env::var_os(ENV).is_some(){return true;}
    let mut p=std::process::Command::new(std::env::current_exe().unwrap()).creation_flags(0x08000000)
        .env(ENV,"1").args(["--exact",&format!("windows::root_observation::native_frames::sessions::tests::{name}"),"--nocapture"])
        .spawn().unwrap();
    let deadline=Instant::now()+Duration::from_secs(30);
    loop {
        if let Some(s)=p.try_wait().unwrap(){assert!(s.success(),"owned session child {s}");return false;}
        if Instant::now()>=deadline {let _=p.kill();let _=p.wait();panic!("session child timeout");}
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn scope_enter(epoch:u64)->Result<u64,Error> {
    FRAMES.get().unwrap().enter_existing_epoch(epoch).map_err(|_|Error::Stale)
}
fn scope_leave(hold:u64,exceptional:bool)->Result<(),Error> {
    let f=FRAMES.get().unwrap();if exceptional {f.mark_unresolved();}f.leave(hold).map_err(|_|Error::Stale)
}
static SESSION:AtomicU64=AtomicU64::new(0);
static CALLBACK:AtomicU64=AtomicU64::new(0);
static SESSION_ID:AtomicU64=AtomicU64::new(17);
static BIRTH_HITS:AtomicUsize=AtomicUsize::new(0);
static JOIN_AL:AtomicU64=AtomicU64::new(1);
static LATE_CONTINUATION:AtomicBool=AtomicBool::new(false);
static ADMISSION_HITS:AtomicUsize=AtomicUsize::new(0);
static HOME:AtomicU64=AtomicU64::new(0);
static ARG5:AtomicU64=AtomicU64::new(0);
extern "system" fn factory_body(id:u32,owner:u64)->u64 {
    assert_eq!(unsafe{GetLastError()},0x1234);
    let session=SESSION.load(Ordering::Acquire);let callback=CALLBACK.load(Ordering::Acquire);
    assert_eq!(read64(owner+8).unwrap(),0,"original owner store must still be unpublished");
    let lease=CALLS.with(|s|s.borrow().last().unwrap().lease).unwrap();
    assert_eq!(state().snapshot(lease),Err(Error::Unbound));
    unsafe {
        *(session as *mut u64)=EXE.load(Ordering::Acquire)+0x1043088;
        *((session+0x30) as *mut u64)=callback;
        *(callback as *mut u64)=EXE.load(Ordering::Acquire)+0x10494a8;
        *((callback+8) as *mut u32)=id;
    }
    BIRTH_HITS.fetch_add(1,Ordering::AcqRel);session
}
extern "system" fn admission_body()->u64 {
    ADMISSION_HITS.fetch_add(1,Ordering::AcqRel);
    let world=NativeSessionApi::current_world();
    if world.is_some() {
        assert_eq!(FRAMES.get().unwrap().seal_and_wait(Instant::now()+Duration::from_secs(1)),
            Err(crate::native_flight::Error::Reentrant));
        assert_eq!(NativeSessionApi::drained(world.unwrap()),Err(Error::Busy));
    }
    unsafe {SetLastError(0x7788)};0x445566
}
extern "system" fn after_join(){LATE_CONTINUATION.store(true,Ordering::Release);}
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
    "mov rax,[rip+{join_al}]","mov rbx,[rsp+0x38]","lea rsp,[rsp+0x20]","pop rsi","ret",".seh_endproc",
    ".globl session_home_driver",".seh_proc session_home_driver","session_home_driver:",
    "sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","mov qword ptr [rsp],0x5a",
    "call {register}","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    factory=sym native_session_factory_gateway,admission=sym native_session_admission_gateway,
    join=sym native_session_join_gateway,register=sym native_session_register_gateway,
    factory_body=sym factory_body,admission_body=sym admission_body,after_join=sym after_join,
    home=sym HOME,arg5=sym ARG5,id=sym SESSION_ID,session=sym SESSION,join_al=sym JOIN_AL,
);
unsafe extern "system" {
    fn session_fixture_register();fn session_fixture_factory();fn session_fixture_incoming();
    fn session_fixture_admission();fn session_fixture_close();fn session_fixture_join();
    fn session_fixture_factory_return();fn session_fixture_join_return();
    fn session_home_driver(registry:u64,kind:u64,owner:u64)->u64;
}
struct Fixture { base:usize,registry:Box<[u64;4]>,buckets:Box<[u64;2]>,node:Box<[u64;3]>,
    owner:Box<[u64;2]>,session:Box<[u64;80]>,callback:Box<[u64;2]>,transport:Box<[u64;16]>,
    network:Box<[u64;26]>,thread:Box<[u64;28]> }
impl Fixture {
    fn new()->Self {
        FRAMES.get_or_init(NativeFrames::new);FIXTURE.store(true,Ordering::Release);
        let base=unsafe{VirtualAlloc(null_mut(),0x2360000,MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE)} as usize;
        assert_ne!(base,0);EXE.store(base as u64,Ordering::Release);
        CORE.store(1,Ordering::Release);
        assert!(SCOPE.set(SessionScopeHooks{enter_existing_epoch:scope_enter,leave:scope_leave}).is_ok());
        let mut x=Self {base,registry:Box::new([0;4]),buckets:Box::new([0;2]),node:Box::new([17,0,0]),
            owner:Box::new([0;2]),session:Box::new([0;80]),callback:Box::new([0;2]),transport:Box::new([0;16]),
            network:Box::new([0;26]),thread:Box::new([0;28])};
        x.registry[0]=base as u64+0x1000000;x.registry[2]=x.buckets.as_ptr() as u64;x.registry[3]=1;
        x.buckets[0]=x.node.as_ptr() as u64;
        x.session[0]=base as u64+0x1043088;x.session[5]=x.transport.as_ptr() as u64;
        x.transport[0]=base as u64+0x1047e60;x.transport[2]=x.network.as_ptr() as u64;
        x.transport[8]=0x10000000000; // byte+45=1
        x.network[24]=x.thread.as_ptr() as u64;x.thread[0]=base as u64+0x106d030;
        unsafe {
            *((base+0x1000018) as *mut u64)=(base+0x7f8ab0) as u64;
            *((base+0x10432c0) as *mut u64)=(base+0x842030) as u64;
            *((base+0x1047ea0) as *mut u64)=(base+0x84ebd0) as u64;
            *((base+0x1047eb8) as *mut u64)=(base+0x84ec00) as u64;
            *((base+0x106d060) as *mut u64)=(base+0x98c650) as u64;
            *((base+0x235b878) as *mut u64)=x.registry.as_ptr() as u64;
        }
        SESSION.store(x.session.as_ptr() as u64,Ordering::Release);
        CALLBACK.store(x.callback.as_ptr() as u64,Ordering::Release);
        for (i,p) in [session_fixture_register as *const (),session_fixture_factory as *const (),session_fixture_incoming as *const (),
            session_fixture_admission as *const (),session_fixture_close as *const (),session_fixture_join as *const ()].into_iter().enumerate() {
            NATIVE_SESSION_CONTINUE[i].store(p as u64+5,Ordering::Release);
        }
        x
    }
    fn birth(&mut self,lifetime:bool)->Option<Lease> {
        self.owner[1]=0;self.node[0]=17;self.node[1]=0;
        unsafe{SetLastError(0x1234)};
        let call=||unsafe{session_home_driver(self.registry.as_ptr() as u64,2,self.owner.as_ptr() as u64)};
        let id=if lifetime {NativeSessionApi::with_registry(self.registry.as_ptr() as u64,9,call).unwrap()}else{call()};
        assert_eq!(id,17);assert_eq!(HOME.load(Ordering::Acquire),0x5a);
        assert_eq!(BIRTH_HITS.load(Ordering::Acquire),1);
        assert_eq!(self.owner[1],self.session.as_ptr() as u64);
        assert_eq!(self.node[1],self.session.as_ptr() as u64);
        NativeSessionApi::born(self.registry.as_ptr() as u64,9,id as u32)
    }
    fn bind(&self,lease:Lease) {
        NativeSessionApi::bind_world(lease,WorldReceipt{binding_id:8,epoch:1}).unwrap();
        NativeSessionApi::bind_transport(lease,self.transport.as_ptr() as u64,self.network.as_ptr() as u64,self.thread.as_ptr() as u64).unwrap();
    }
    fn incoming(&self)->u64 {
        let f:unsafe extern "system" fn(u64,u64,u64,u64,u64)->u64=unsafe{std::mem::transmute(gateways()[2])};
        unsafe{f(self.callback.as_ptr() as u64,0,0,0,0x8877665544332211)}
    }
    fn close(&self) {
        let f:unsafe extern "system" fn(u64)=unsafe{std::mem::transmute(gateways()[4])};
        unsafe{f(self.session.as_ptr() as u64)};
    }
}
impl Drop for Fixture {fn drop(&mut self){assert_ne!(unsafe{VirtualFree(self.base as _,0,MEM_RELEASE)},0);}}

#[test]
fn actual_birth_before_publication_delayed_dispatch_join_and_stale_ticket(){
    if !child("actual_birth_before_publication_delayed_dispatch_join_and_stale_ticket"){return;}
    let mut x=Fixture::new();let lease=x.birth(true).unwrap();x.bind(lease);
    let ticket=NativeSessionApi::reserve_message(lease).unwrap();
    assert_eq!(NativeSessionApi::with_message(ticket,||x.incoming()).unwrap(),0x445566);
    assert_eq!(ARG5.load(Ordering::Acquire),0x8877665544332211);
    assert_eq!(unsafe{GetLastError()},0x7788);
    assert_eq!(ADMISSION_HITS.load(Ordering::Acquire),1);
    assert!(NativeSessionApi::current_world().is_none());
    x.close();assert!(LATE_CONTINUATION.load(Ordering::Acquire));
    assert_eq!(NativeSessionApi::drained(WorldReceipt{binding_id:8,epoch:1}),Ok(()));
    assert_eq!(NativeSessionApi::with_message(ticket,||()),Err(Error::Unknown));
    let f=FRAMES.get().unwrap();f.seal_and_wait(Instant::now()+Duration::from_secs(1)).unwrap();f.reopen().unwrap();
    assert_eq!(state().world(lease).unwrap().epoch,1);
    assert_eq!(NativeSessionApi::reserve_message(lease),Err(Error::Closed));
    assert!(CALLS.with(|s|s.borrow().is_empty()));
}
#[test]
fn actual_unbound_birth_and_unknown_dispatch_forward_original_once(){
    if !child("actual_unbound_birth_and_unknown_dispatch_forward_original_once"){return;}
    let mut x=Fixture::new();assert!(x.birth(false).is_none());
    assert_eq!(x.incoming(),0x445566);
    assert_eq!(ADMISSION_HITS.load(Ordering::Acquire),1);
    assert!(NativeSessionApi::current_world().is_none());
    assert_eq!(NativeSessionApi::drained(WorldReceipt{binding_id:8,epoch:1}),Err(Error::Unknown));
}
#[test]
fn unbound_birth_never_uses_message_or_current_world_as_authority(){
    let mut s=Ledger::default();
    let l=s.birth(0x10,None,0x20,2).unwrap();
    s.factory(l,7,0x20,0x30,0x40).unwrap();s.published(l,7).unwrap();
    assert_eq!(s.bind_world(l,WorldReceipt{binding_id:1,epoch:1}),Err(Error::Unbound));
    assert_eq!(s.reserve(l),Err(Error::Unbound));
    assert_eq!(s.drained(WorldReceipt{binding_id:1,epoch:1}),Err(Error::Unknown));
}
