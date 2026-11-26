use super::*;
use std::os::windows::process::CommandExt;
use windows_sys::Win32::System::Diagnostics::Debug::{
    RtlLookupFunctionEntry, IMAGE_RUNTIME_FUNCTION_ENTRY,
};
static FLOW: AtomicBool = AtomicBool::new(false);
static FLOW_PATH: AtomicBool = AtomicBool::new(false);
static FLOW_BOOTSTRAP: AtomicBool = AtomicBool::new(false);
static FLOW_BOOTSTRAP_STALE: AtomicBool = AtomicBool::new(false);
static FLOW_DESCRIPTOR: AtomicU64 = AtomicU64::new(0);
static FLOW_QUEUE: AtomicU64 = AtomicU64::new(0);
static FLOW_PAYLOAD: AtomicU64 = AtomicU64::new(0);
static FLOW_OWNED: AtomicU64 = AtomicU64::new(0);
static FLOW_ENGINE: AtomicU64 = AtomicU64::new(0);
static FLOW_VIEW: AtomicU64 = AtomicU64::new(0);
static FLOW_TASK: AtomicU64 = AtomicU64::new(0);
static FLOW_ARENA: AtomicU64 = AtomicU64::new(0);
extern "system" fn flow_copy_body() {
    let destination = FLOW_OWNED.load(Ordering::Acquire);
    let mut born: Saved = unsafe { std::mem::zeroed() };
    born.invocation.kind = 92;
    born.invocation.caller = exe(0x166bcb);
    born.gpr[0] = destination;
    begin(&mut born);
    born.gpr[4] = destination;
    finish(&mut born, false);
    unsafe {
        *((exe(0x22d8d18)) as *mut u64) = FLOW_OWNED.load(Ordering::Acquire);
    }
}
extern "system" fn flow_allocate_body() -> u64 {
    FLOW_PAYLOAD.load(Ordering::Acquire)
}
extern "system" fn flow_path_body(path: *const u16, options: *const u8) {
    unsafe {
        std::ptr::copy_nonoverlapping(options, exe(0x1267a68) as *mut u8, 28);
        let mut len = 0usize;
        while len < 259 && *path.add(len) != 0 {
            *((exe(0x1267a9c) as *mut u16).add(len)) = *path.add(len);
            len += 1;
        }
        *((exe(0x1267a9c) as *mut u16).add(len)) = 0;
        *(exe(0x1267a98) as *mut u32) = len as u32;
    }
}
extern "system" fn flow_start_body() {
    let view = FLOW_VIEW.load(Ordering::Acquire);
    let task = FLOW_TASK.load(Ordering::Acquire);
    let mut r: Saved = unsafe { std::mem::zeroed() };
    r.invocation.kind = 23;
    r.invocation.caller = exe(0x985904);
    r.invocation.caller_rsp = 0x100;
    job_begin(&mut r);
    r.gpr[4] = task;
    job_finish(&mut r, false);
    assert_eq!(value(view + 0x250), 0, "P+8 publication has not occurred");
    assert_eq!(value(task + 8), 0, "T+8 publication has not occurred");
    assert!(state().tasks_by_address.contains_key(&task));
    unsafe {
        *(task as *mut u64) = exe(0x106c418);
        *((task + 8) as *mut u64) = view + 0x248;
        *((view + 0x250) as *mut u64) = task;
    }
}
extern "system" fn flow_worker_body() {
    let invocation = current_worker().unwrap();
    assert_eq!(state().ledger.request(invocation).unwrap().0, 1);
    FRAMES.get_or_init(NativeFrames::new);
    jobs::fixture_activate_core(0x180000000);
    let world = worlds::WorldLease {
        epoch: 1,
        serial: if FLOW_BOOTSTRAP.load(Ordering::Acquire)
            && !FLOW_BOOTSTRAP_STALE.load(Ordering::Acquire)
        {
            1
        } else {
            9
        },
        context: 10,
        game: 20,
        world: 30,
        observation: 0,
    };
    if FLOW_BOOTSTRAP.load(Ordering::Acquire) {
        assert_eq!(worlds::fixture_world_birth_after(world.serial - 1), world);
    }
    world_created(world);
    if FLOW_PATH.load(Ordering::Acquire) {
        // The pinned 85857 caller enters the shared input reader with both
        // arguments zero. For pathname loads its global input is a native file,
        // so it cannot satisfy a copied snapshot-stream incarnation.
        let mut reader: Saved = unsafe { std::mem::zeroed() };
        reader.invocation.kind = 88;
        begin(&mut reader);
        finish(&mut reader, false);
    }
    assert_eq!(
        FRAMES
            .get()
            .unwrap()
            .seal_executions_and_wait(Instant::now() + Duration::from_secs(1)),
        Err(crate::native_flight::Error::Reentrant)
    );
}
global_asm!(
    ".p2align 4",".globl load_flow_request",".seh_proc load_flow_request","load_flow_request:",
    "mov [rsp+0x20],r9","sub rsp,0x38",".seh_stackalloc 0x38",".seh_endprologue",
    "mov rcx,[rsp+0x60]","mov edx,2","call {copy}",
    ".globl load_flow_copy_return","load_flow_copy_return:","mov rcx,[rip+{queue}]","call {allocate}",
    ".globl load_flow_allocate_return","load_flow_allocate_return:","mov al,1","lea rsp,[rsp+0x38]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_path_request",".seh_proc load_flow_path_request","load_flow_path_request:",
    "mov [rsp+0x20],r9","sub rsp,0x38",".seh_stackalloc 0x38",".seh_endprologue",
    "mov rdx,r8","call {path_body}","mov rcx,[rip+{queue}]","call {allocate}",
    ".globl load_flow_path_allocate_return","load_flow_path_allocate_return:","mov al,1","lea rsp,[rsp+0x38]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_snapshot",".seh_proc load_flow_snapshot","load_flow_snapshot:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","sub rsp,0x38",".seh_stackalloc 0x38",".seh_endprologue",
    "xor ecx,ecx","mov edx,2","mov r8,[rip+{descriptor_input}]","xor r9d,r9d","mov qword ptr [rsp+0x20],0x2000","call {request}",
    "lea rsp,[rsp+0x38]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_copy",".seh_proc load_flow_copy","load_flow_copy:",
    ".byte 0x40,0x55",".seh_pushreg rbp","push rsi",".seh_pushreg rsi","push rdi",".seh_pushreg rdi","push r15",".seh_pushreg r15",
    "sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {copy_body}",
    "lea rsp,[rsp+0x28]","pop r15","pop rdi","pop rsi","pop rbp","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_allocate",".seh_proc load_flow_allocate","load_flow_allocate:",
    "mov eax,[rcx+0x800100]","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {allocate_body}","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_bank",".seh_proc load_flow_bank","load_flow_bank:",
    ".byte 0x40,0x56",".seh_pushreg rsi","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "mov rdx,r8","add rdx,0xa20","mov rcx,[rip+{payload}]","call {command}","lea rsp,[rsp+0x20]","pop rsi","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_command",".seh_proc load_flow_command","load_flow_command:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "mov rdx,rcx","mov rcx,[rip+{engine}]","call {select}",
    ".globl load_flow_select_return","load_flow_select_return:","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_select","load_flow_select:",
    "mov rax,[rcx+0x14f0]","mov [rcx+0x53f8],rax","ret",
    ".globl load_flow_transition",".seh_proc load_flow_transition","load_flow_transition:",
    "sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "mov rax,[rcx+0x53f8]","mov [rcx+0x5400],rax","mov qword ptr [rcx+0x53f8],0","mov rcx,rax","call {initialize}",
    ".globl load_flow_initialize_return","load_flow_initialize_return:","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_initialize",".seh_proc load_flow_initialize","load_flow_initialize:",
    ".byte 0x40,0x55",".seh_pushreg rbp","push r12",".seh_pushreg r12","push r14",".seh_pushreg r14",
    "sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue","add rcx,0x240","call {start}",
    ".globl load_flow_start_return","load_flow_start_return:","lea rsp,[rsp+0x20]","pop r14","pop r12","pop rbp","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_start",".seh_proc load_flow_start","load_flow_start:",
    "sub rsp,0x38",".seh_stackalloc 0x38","lea rax,[rip+load_flow_start]",".seh_endprologue","call {start_body}","lea rsp,[rsp+0x38]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_worker",".seh_proc load_flow_worker","load_flow_worker:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue",
    "call {worker_body}","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".globl load_flow_worker_driver",".seh_proc load_flow_worker_driver","load_flow_worker_driver:",
    "sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {worker}",
    ".globl load_flow_worker_return","load_flow_worker_return:","lea rsp,[rsp+0x28]","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_group",".seh_proc load_flow_group","load_flow_group:",
    "mov [rsp+8],rbx",".seh_savereg rbx,8","push rdi",".seh_pushreg rdi","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "mov rdi,rcx","mov rcx,[rcx+0x40]","call qword ptr [rip+{arena}]",
    ".globl load_flow_group_return","load_flow_group_return:","mov qword ptr [rdi+0x40],0","mov rbx,[rsp+0x30]","lea rsp,[rsp+0x20]","pop rdi","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_arena",".seh_proc load_flow_arena","load_flow_arena:",
    ".byte 0x40,0x55",".seh_pushreg rbp","push rbx",".seh_pushreg rbx","push rsi",".seh_pushreg rsi","lea rbp,[rsp-0x47]",".seh_endprologue",
    "pop rsi","pop rbx","pop rbp","ret",".seh_endproc",
    ".p2align 4",".globl load_flow_execute",".seh_proc load_flow_execute","load_flow_execute:",
    ".byte 0x40,0x53",".seh_pushreg rbx","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue",
    "mov rcx,[rcx+8]","sub rcx,0x248","call {worker_driver}",
    "lea rsp,[rsp+0x20]","pop rbx","ret",".seh_endproc",
    copy=sym native_load_copy_gateway,allocate=sym native_load_allocate_gateway,command=sym native_load_command_gateway,
    select=sym native_load_select_gateway,initialize=sym native_load_initialize_gateway,start=sym native_load_start_gateway,worker=sym native_load_worker_gateway,
    copy_body=sym flow_copy_body,allocate_body=sym flow_allocate_body,start_body=sym flow_start_body,worker_body=sym flow_worker_body,path_body=sym flow_path_body,
    queue=sym FLOW_QUEUE,payload=sym FLOW_PAYLOAD,engine=sym FLOW_ENGINE,arena=sym FLOW_ARENA,
    worker_driver=sym load_flow_worker_driver,
    descriptor_input=sym FLOW_DESCRIPTOR,request=sym native_load_request_gateway,
);
unsafe extern "system" {
    fn load_flow_copy_return();
    fn load_flow_allocate_return();
    fn load_flow_path_allocate_return();
    fn load_flow_select_return();
    fn load_flow_initialize_return();
    fn load_flow_start_return();
    fn load_flow_worker_return();
    fn load_flow_group_return();
    fn load_flow_group();
    fn load_flow_arena();
    fn load_flow_execute();
    fn load_flow_request();
    fn load_flow_path_request();
    fn load_flow_snapshot();
    fn load_flow_copy();
    fn load_flow_allocate();
    fn load_flow_bank();
    fn load_flow_command();
    fn load_flow_select();
    fn load_flow_initialize();
    fn load_flow_start();
    fn load_flow_worker();
    fn load_flow_transition(engine: u64);
    fn load_flow_worker_driver(view: u64);
}
pub(super) fn caller_override(rva: u64) -> Option<u64> {
    if !FLOW.load(Ordering::Acquire) {
        return None;
    }
    match rva {
        0x16bd51 => Some(load_flow_copy_return as *const () as u64),
        0x163dea if FLOW_PATH.load(Ordering::Acquire) => {
            Some(load_flow_path_allocate_return as *const () as u64)
        }
        0x163dea => Some(load_flow_allocate_return as *const () as u64),
        0xdb88d => Some(load_flow_select_return as *const () as u64),
        0x63fcd => Some(load_flow_initialize_return as *const () as u64),
        0x83144 => Some(load_flow_start_return as *const () as u64),
        0x985b63 => Some(load_flow_worker_return as *const () as u64),
        0x985bb0 => Some(load_flow_group_return as *const () as u64),
        _ => None,
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
}
fn child(name: &str) -> bool {
    const ENV: &str = "CIV6_NATIVE_LOAD_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    let mut p = std::process::Command::new(std::env::current_exe().unwrap())
        .creation_flags(0x08000000)
        .env(ENV, "1")
        .args([
            "--exact",
            &format!("windows::root_observation::native_frames::loads::tests::{name}"),
            "--nocapture",
        ])
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(s) = p.try_wait().unwrap() {
            assert!(s.success(), "load child {s}");
            return false;
        }
        if Instant::now() >= deadline {
            let _ = p.kill();
            let _ = p.wait();
            panic!("load child timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn accepted_request_with_world_order(unproven: bool, world_first: bool) {
    // Pure observer/ledger state: no executable allocation, hook installation,
    // copied native instructions, external process memory or game operation.
    let lease = sessions::fixture_snapshot_session(20, 30);
    let source = sessions::snapshot_owner(20, 30).unwrap();
    let previous = sessions::fixture_retire_load_world(lease, unproven);
    assert_eq!(source, (lease, Some(previous)));
    let world = worlds::WorldLease {
        serial: 11,
        epoch: 4,
        context: 80,
        game: 90,
        world: 100,
        observation: 0,
    };
    let frames = FRAMES.get_or_init(NativeFrames::new);
    for _ in 1..world.epoch {
        frames
            .seal_and_wait(Instant::now() + Duration::from_secs(1))
            .unwrap();
        frames.reopen().unwrap();
    }
    let (request, invocation) = {
        let mut s = state();
        let request = s.ledger.enter().unwrap();
        s.ledger.fixture_copied(request, 160).unwrap();
        s.ledger.issued(request, 120, 130).unwrap();
        s.ledger.consuming(120, 130).unwrap();
        s.ledger.selected(request, 120, 140, 150).unwrap();
        s.ledger.consumed(request, 120).unwrap();
        let invocation = s.ledger.initialize(140, 150, 160).unwrap().unwrap();
        s.ledger.initialized(invocation).unwrap();
        s.ledger.reserve_worker(invocation).unwrap();
        s.ledger.worker(150).unwrap();
        s.sources.insert(
            request.0,
            Source {
                session: Some(source),
                wire: None,
                bootstrap: None,
            },
        );
        (request, invocation)
    };
    let born = || {
        CALLS.with(|calls| {
            calls.borrow_mut().push(Call {
                serial: 2,
                kind: 87,
                source: None,
                request: None,
                invocation: Some(invocation),
                pointer: 150,
                descriptor: 0,
                engine: 140,
                allocation: None,
                joined: false,
            })
        });
        JOB_CALLS.with(|jobs| {
            jobs.borrow_mut().push(JobCall {
                rsp: 1,
                kind: 21,
                id: None,
                invocation: Some(invocation),
                view: 150,
                group: 1,
                hold: None,
            })
        });
        world_created(world);
        CALLS.with(|calls| {
            calls.borrow_mut().pop().unwrap();
        });
        let (hold, epoch) = JOB_CALLS.with(|jobs| jobs.borrow_mut().pop().unwrap().hold.unwrap());
        assert_eq!(epoch, world.epoch);
        frames.leave(hold).unwrap();
    };
    if world_first {
        born();
    }
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 81;
    raw.invocation.cookie = COOKIE;
    raw.invocation.token = 1;
    raw.gpr[4] = 0x1234_5601;
    CALLS.with(|calls| {
        calls.borrow_mut().push(Call {
            serial: 1,
            kind: 81,
            source: None,
            request: Some(request),
            invocation: None,
            pointer: 0,
            descriptor: 0,
            engine: 0,
            allocation: None,
            joined: false,
        })
    });
    finish(&mut raw, false);
    if !world_first {
        born();
    }
    assert_eq!(raw.gpr[4], 0x1234_5601);
    assert_eq!(raw.invocation.cookie, 0);
    let s = state();
    assert!(s.accepted.contains(&request.0));
    if unproven {
        assert_eq!(
            s.binding_rejections.get(&request.0),
            Some(&sessions::SessionError::Unresolved)
        );
        assert_eq!(sessions::fixture_bound_session_world(lease), None);
        assert!(sessions::snapshot_owner(20, 30).is_none());
        assert_eq!(
            NativeSessionApi::drained(previous),
            Err(sessions::SessionError::Unresolved)
        );
    } else {
        assert!(s.binding_rejections.is_empty());
        assert_eq!(
            sessions::fixture_bound_session_world(lease),
            Some(sessions::SessionWorldReceipt {
                binding_id: 11,
                epoch: 4
            })
        );
    }
    assert!(CALLS.with(|calls| calls.borrow().is_empty()));
    assert!(JOB_CALLS.with(|calls| calls.borrow().is_empty()));
    drop(s);
    let mut state = state();
    state.ledger.worker_returned(invocation).unwrap();
    state.ledger.release_worker(invocation).unwrap();
    state.ledger.joined(invocation).unwrap();
    drop(state);
    frames
        .seal_and_wait(Instant::now() + Duration::from_secs(1))
        .unwrap();
}
fn accepted_pre_world_packet_order(replace: bool) {
    let lease = sessions::fixture_bootstrap_session(10, 20);
    let packet = sessions::fixture_pre_world_packet(lease);
    let target = sessions::bootstrap_load_target(10, 20, Some(packet)).unwrap();
    if replace {
        sessions::fixture_replace_pre_world_transport(lease);
    }
    let world = worlds::fixture_world_birth();
    let request = {
        let mut state = state();
        let request = state.ledger.enter().unwrap();
        state.sources.insert(
            request.0,
            Source {
                session: None,
                wire: None,
                bootstrap: Some(target),
            },
        );
        state.worlds.insert(request.0, world);
        request
    };
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 81;
    raw.invocation.cookie = COOKIE;
    raw.invocation.token = 1;
    raw.gpr[4] = 0x1234_5601;
    CALLS.with(|calls| {
        calls.borrow_mut().push(Call {
            serial: 1,
            kind: 81,
            source: None,
            request: Some(request),
            invocation: None,
            pointer: 0,
            descriptor: 0,
            engine: 0,
            allocation: None,
            joined: false,
        })
    });
    finish(&mut raw, false);
    assert_eq!(
        raw.gpr[4], 0x1234_5601,
        "binding refusal changed original load success"
    );
    assert_eq!(raw.invocation.cookie, 0);
    assert!(state().accepted.contains(&request.0));
    assert!(CALLS.with(|calls| calls.borrow().is_empty()));
    if replace {
        assert_eq!(
            state().binding_rejections.get(&request.0),
            Some(&sessions::SessionError::Stale)
        );
        assert_eq!(sessions::fixture_bound_session_world(lease), None);
    } else {
        assert!(state().binding_rejections.is_empty());
        assert_eq!(
            sessions::fixture_bound_session_world(lease),
            Some(sessions::SessionWorldReceipt {
                binding_id: world.serial,
                epoch: world.epoch,
            })
        );
    }
}
#[test]
fn accepted_pre_world_packet_load_refuses_a_replaced_issuing_transport() {
    if !child("accepted_pre_world_packet_load_refuses_a_replaced_issuing_transport") {
        return;
    }
    accepted_pre_world_packet_order(true);
}
#[test]
fn accepted_pre_world_packet_load_binds_its_unchanged_issuing_transport() {
    if !child("accepted_pre_world_packet_load_binds_its_unchanged_issuing_transport") {
        return;
    }
    accepted_pre_world_packet_order(false);
}

#[test]
fn world_before_load_return_keeps_unproven_session_as_rejection() {
    if child("world_before_load_return_keeps_unproven_session_as_rejection") {
        accepted_request_with_world_order(true, true);
    }
}

#[test]
fn world_before_load_return_binds_a_proven_session() {
    if child("world_before_load_return_binds_a_proven_session") {
        accepted_request_with_world_order(false, true);
    }
}

#[test]
fn load_return_before_world_keeps_unproven_session_as_rejection() {
    if child("load_return_before_world_keeps_unproven_session_as_rejection") {
        accepted_request_with_world_order(true, false);
    }
}

#[test]
fn load_return_before_world_binds_a_proven_session() {
    if child("load_return_before_world_binds_a_proven_session") {
        accepted_request_with_world_order(false, false);
    }
}

#[test]
fn known_session_binding_result_keeps_identity_errors_and_only_records_observation_loss() {
    let mut state = State::default();
    let request = state.ledger.enter().unwrap();
    for error in [
        sessions::SessionError::Unknown,
        sessions::SessionError::Stale,
        sessions::SessionError::Closed,
        sessions::SessionError::Busy,
        sessions::SessionError::Exhausted,
        sessions::SessionError::Unbound,
        sessions::SessionError::NotJoined,
    ] {
        assert_eq!(
            state.known_session_binding_result(request, Err(error)),
            Err(error)
        );
        assert!(state.binding_rejections.is_empty());
    }
    assert_eq!(state.known_session_binding_result(request, Ok(())), Ok(()));
    assert!(state.binding_rejections.is_empty());
    assert_eq!(
        state.known_session_binding_result(request, Err(sessions::SessionError::Unresolved)),
        Ok(())
    );
    assert_eq!(
        state.binding_rejections.get(&request.0),
        Some(&sessions::SessionError::Unresolved)
    );
}

#[test]
fn pathname_capture_preserves_native_utf16_cap_and_keeps_stream_inputs_separate() {
    let mut name = vec![0x3042u16; 270];
    name.push(0);
    let mut options = [0u32; 7];
    let input = path_input(name.as_ptr() as u64, options.as_ptr() as u64).unwrap();
    assert_eq!(input.name, vec![0x3042u16; 259]);
    options[2] = 4;
    assert!(path_input(name.as_ptr() as u64, options.as_ptr() as u64).is_none());
    assert!(path_input(0, options.as_ptr() as u64).is_none());
}
#[test]
fn ordinary_path_load_retains_an_intent_before_command_publication() {
    if !child("ordinary_path_load_retains_an_intent_before_command_publication") {
        return;
    }
    let home = [0u64; 8];
    let path: Vec<u16> = "ordinary.Civ6Save\0".encode_utf16().collect();
    let descriptor = [2u32, 0, 0, 0, 0, 0, 0];
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 81;
    raw.invocation.caller_rsp = home.as_ptr() as u64;
    raw.gpr[0] = path.as_ptr() as u64;
    raw.gpr[1] = 2;
    raw.gpr[2] = descriptor.as_ptr() as u64;
    begin(&mut raw);
    let request = CALLS.with(|s| s.borrow().last().unwrap().request);
    assert!(
        request.is_some(),
        "ordinary pathname load lost its native intent"
    );
    assert_eq!(parent_request(), request);
    assert!(state().tasks.is_empty());
    assert!(state().worlds.is_empty());
    assert!(state()
        .sources
        .get(&request.unwrap().0)
        .unwrap()
        .session
        .is_none());
    // Entry retains an intent; it does not claim that the native busy branch
    // accepted or published it. A plain AL0 remains an ordinary rejection.
    raw.gpr[4] = 0;
    finish(&mut raw, false);
    assert!(CALLS.with(|s| s.borrow().is_empty()));
    assert!(state().accepted.is_empty());
    assert!(state().ledger.enter().is_ok());
    assert!(!FAILED.load(Ordering::Acquire));
}
#[test]
fn unsupported_path_load_and_unknown_terminal_unwind_keep_original_semantics() {
    if !child("unsupported_path_load_and_unknown_terminal_unwind_keep_original_semantics") {
        return;
    }
    let mut home = [0u64; 8];
    home[5] = 0;
    let descriptor = [2u32, 0, 0, 0];
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 81;
    raw.invocation.caller_rsp = home.as_ptr() as u64;
    raw.gpr[1] = 2;
    raw.gpr[2] = descriptor.as_ptr() as u64;
    begin(&mut raw);
    assert!(CALLS.with(|s| s.borrow().last().unwrap().request.is_none()));
    raw.gpr[4] = 1;
    finish(&mut raw, false);
    assert!(!FAILED.load(Ordering::Acquire));
    let mut unknown: Saved = unsafe { std::mem::zeroed() };
    unknown.invocation.kind = 87;
    begin(&mut unknown);
    assert!(CALLS.with(|s| s.borrow().last().unwrap().invocation.is_none()));
    finish(&mut unknown, true);
    assert!(!FAILED.load(Ordering::Acquire));
    assert!(state().ledger.enter().is_ok());
    let mut known: Saved = unsafe { std::mem::zeroed() };
    known.invocation.kind = 81;
    home[5] = 0x1000;
    let descriptor = [2u32, 0, 4, 0];
    known.invocation.caller_rsp = home.as_ptr() as u64;
    known.gpr[1] = 2;
    known.gpr[2] = descriptor.as_ptr() as u64;
    begin(&mut known);
    finish(&mut known, true);
    assert!(FAILED.load(Ordering::Acquire));
    assert_eq!(state().ledger.enter(), Err(ledger::Error::Unresolved));
}
#[test]
fn load_group_wait_uses_existing_arena_gateway_and_requires_external_normal_return() {
    const ENV: &str = "CIV6_LOAD_GROUP_CURRENT_NEGATIVE";
    if std::env::var_os(ENV).is_none()
        && !child("load_group_wait_uses_existing_arena_gateway_and_requires_external_normal_return")
    {
        return;
    }
    let negative = std::env::var_os(ENV).is_some();
    let base = 0x140000000;
    EXE.store(base, Ordering::Release);
    FLOW.store(true, Ordering::Release);
    let mut view = vec![0u64; 0x2e0 / 8];
    let vp = view.as_mut_ptr() as u64;
    view[0x240 / 8] = exe(0xfad3e0);
    view[0x280 / 8] = 0x123400;
    let invocation = {
        let mut s = state();
        let r = s.ledger.enter().unwrap();
        s.ledger.fixture_copied(r, 10).unwrap();
        s.ledger.issued(r, 20, 30).unwrap();
        s.ledger.returned(r, true).unwrap();
        s.ledger.consuming(20, 30).unwrap();
        s.ledger.selected(r, 20, 40, vp).unwrap();
        s.ledger.consumed(r, 20).unwrap();
        let v = s.ledger.initialize(40, vp, 10).unwrap().unwrap();
        s.ledger.reserve_worker(v).unwrap();
        s.ledger.initialized(v).unwrap();
        s.ledger.release_worker(v).unwrap();
        s.groups.insert(v.0, 0x123400);
        v
    };
    let arena =
        consumers::fixture_group_arena(load_flow_arena as *const () as usize + 9, !negative);
    FLOW_ARENA.store(arena as u64, Ordering::Release);
    NATIVE_LOAD_CONTINUE[11].store(load_flow_group as *const () as u64 + 5, Ordering::Release);
    let call: unsafe extern "system" fn(u64) = unsafe { std::mem::transmute(gateways()[11]) };
    unsafe {
        call(vp + 0x240);
    }
    if negative {
        panic!("current arena unexpectedly returned through join guard");
    }
    assert!(CALLS.with(|s| s.borrow().is_empty()));
    assert_eq!(state().ledger.joined(invocation), Err(ledger::Error::Busy));
    let out = crate::windows::test_child_output::run(std::process::Command::new(std::env::current_exe().unwrap()).creation_flags(0x08000000)
        .env(ENV,"1").args(["--exact","windows::root_observation::native_frames::loads::tests::load_group_wait_uses_existing_arena_gateway_and_requires_external_normal_return","--nocapture"]),Duration::from_secs(10)).unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("LOAD_GROUP_CURRENT_ARENA_REACHED"));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("current arena unexpectedly returned"));
}
#[test]
fn native_request_command_view_and_task_gateway_chain_retains_before_publication_carrier() {
    if !child(
        "native_request_command_view_and_task_gateway_chain_retains_before_publication_carrier",
    ) {
        return;
    }
    run_load_gateway_chain(false);
}
#[test]
fn ordinary_path_command_view_and_native_world_birth_need_no_stale_global_stream() {
    if !child("ordinary_path_command_view_and_native_world_birth_need_no_stale_global_stream") {
        return;
    }
    run_load_gateway_chain(true);
}
#[test]
fn bootstrap_owner_request_binds_only_its_observed_native_world_birth() {
    if !child("bootstrap_owner_request_binds_only_its_observed_native_world_birth") {
        return;
    }
    FLOW_BOOTSTRAP.store(true, Ordering::Release);
    run_load_gateway_chain(false);
}
#[test]
fn stale_bootstrap_birth_window_keeps_the_original_load_without_session_authority() {
    if !child("stale_bootstrap_birth_window_keeps_the_original_load_without_session_authority") {
        return;
    }
    FLOW_BOOTSTRAP.store(true, Ordering::Release);
    FLOW_BOOTSTRAP_STALE.store(true, Ordering::Release);
    run_load_gateway_chain(false);
}
fn run_load_gateway_chain(path_request: bool) {
    let size = 0x2400000;
    let base =
        unsafe { VirtualAlloc(null_mut(), size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) } as u64;
    assert_ne!(base, 0);
    EXE.store(base, Ordering::Release);
    FLOW.store(true, Ordering::Release);
    FLOW_PATH.store(path_request, Ordering::Release);
    let queue = exe(0x1ad4d00);
    let payload = queue + 0x90;
    let descriptor = exe(0x22d58a0);
    let mut engine = vec![0u64; 0x5410 / 8];
    let ep = engine.as_mut_ptr() as u64;
    let mut view = vec![0u64; 0x2e0 / 8];
    let vp = view.as_mut_ptr() as u64;
    let mut task = [0u64; 2];
    let tp = task.as_mut_ptr() as u64;
    FLOW_QUEUE.store(queue, Ordering::Release);
    FLOW_PAYLOAD.store(payload, Ordering::Release);
    FLOW_ENGINE.store(ep, Ordering::Release);
    FLOW_VIEW.store(vp, Ordering::Release);
    FLOW_TASK.store(tp, Ordering::Release);
    FLOW_OWNED.store(0x3000, Ordering::Release);
    unsafe {
        *((payload - 0x10) as *mut u32) = 0x280;
        *((payload - 0xc) as *mut u32) = 0x36;
        *((descriptor + 0x20) as *mut u64) = exe(0xef220);
        *((ep + 0x14f0) as *mut u64) = vp;
        *((exe(0x12c3c88)) as *mut u64) = ep;
        if path_request {
            *(exe(0x22d8d18) as *mut u64) = 0xdead;
        }
        *(vp as *mut u64) = exe(0xfad3f0);
        *((vp + 0x240) as *mut u64) = exe(0xfad3e0);
        *((vp + 0x248) as *mut u64) = exe(0x106c390);
        *((vp + 0x270) as *mut u64) = vp + 0x240;
        *((vp + 0x280) as *mut u64) = 0x123456;
        *((vp + 0x290) as *mut u64) = vp;
        *((vp + 0x298) as *mut u64) = exe(0x82da0);
    }
    let original = [
        (0, load_flow_snapshot as *const () as u64),
        (
            1,
            if path_request {
                load_flow_path_request as *const () as u64
            } else {
                load_flow_request as *const () as u64
            },
        ),
        (2, load_flow_copy as *const () as u64),
        (3, load_flow_allocate as *const () as u64),
        (4, load_flow_command as *const () as u64),
        (5, load_flow_select as *const () as u64),
        (6, load_flow_initialize as *const () as u64),
        (7, load_flow_worker as *const () as u64),
        (9, load_flow_start as *const () as u64),
        (10, load_flow_bank as *const () as u64),
    ];
    for (i, address) in original {
        NATIVE_LOAD_CONTINUE[i].store(address + SITES[i].2 as u64, Ordering::Release);
    }
    NATIVE_LOAD_ASYNC_LABEL.store(exe(0x106c4c4), Ordering::Release);
    let request: unsafe extern "system" fn(u64, u32, u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(gateways()[1]) };
    let descriptor_source = [2u32, 0, if path_request { 0 } else { 4 }, 0, 0, 0, 0];
    FLOW_DESCRIPTOR.store(descriptor_source.as_ptr() as u64, Ordering::Release);
    let path: Vec<u16> = "ordinary.Civ6Save\0".encode_utf16().collect();
    let mut snapshot_owner = [0u64; 4];
    snapshot_owner[0] = exe(0xfd35c8);
    snapshot_owner[1] = 0x7700;
    let bootstrap_lease = if FLOW_BOOTSTRAP.load(Ordering::Acquire) {
        unsafe {
            *(exe(0xfd36b0) as *mut u64) = exe(0x273f40);
        }
        Some(sessions::fixture_bootstrap_session(
            snapshot_owner.as_ptr() as u64,
            snapshot_owner[1],
        ))
    } else {
        None
    };
    assert_eq!(
        unsafe {
            if bootstrap_lease.is_some() {
                let snapshot: unsafe extern "system" fn(u64) -> u64 =
                    std::mem::transmute(gateways()[0]);
                snapshot(snapshot_owner.as_ptr() as u64)
            } else {
                request(
                    if path_request {
                        path.as_ptr() as u64
                    } else {
                        0
                    },
                    2,
                    descriptor_source.as_ptr() as u64,
                    0,
                    if path_request { 0 } else { 0x2000 },
                )
            }
        } as u8,
        1
    );
    assert!(state().sources.get(&1).unwrap().session.is_none());
    assert_eq!(
        state().ledger.consuming(payload, queue + 0x400080),
        Err(ledger::Error::Stale)
    );
    let bank: unsafe extern "system" fn(u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(gateways()[10]) };
    unsafe {
        bank(0, queue, descriptor - 0x36 * 0x30);
        load_flow_transition(ep);
    }
    assert_eq!(state().tasks_by_address.len(), 1);
    assert!(
        FRAMES.get().is_none(),
        "Host execute enters before Core runtime exists"
    );
    let execute: unsafe extern "system" fn(u64) -> u64 = unsafe {
        std::mem::transmute(jobs::fixture_early_load_execute(
            load_flow_execute as *const () as usize + 6,
        ))
    };
    unsafe {
        execute(tp);
    }
    FRAMES
        .get()
        .unwrap()
        .seal_executions_and_wait(Instant::now() + Duration::from_secs(1))
        .unwrap();
    let expected_serial =
        if bootstrap_lease.is_some() && !FLOW_BOOTSTRAP_STALE.load(Ordering::Acquire) {
            1
        } else {
            9
        };
    assert_eq!(state().worlds.get(&1).unwrap().serial, expected_serial);
    if let Some(lease) = bootstrap_lease {
        if FLOW_BOOTSTRAP_STALE.load(Ordering::Acquire) {
            assert_eq!(sessions::fixture_bound_session_world(lease), None);
            assert_eq!(
                state().binding_rejections.get(&1),
                Some(&sessions::SessionError::Stale)
            );
        } else {
            assert_eq!(
                sessions::fixture_bound_session_world(lease),
                Some(sessions::SessionWorldReceipt {
                    binding_id: 1,
                    epoch: 1
                })
            );
            assert!(state().binding_rejections.is_empty());
        }
    }
    let mut destruction: Saved = unsafe { std::mem::zeroed() };
    destruction.invocation.kind = 22;
    destruction.invocation.caller_rsp = 0x300;
    destruction.gpr[0] = tp;
    job_begin(&mut destruction);
    assert!(state().tasks_by_address.is_empty());
    job_finish(&mut destruction, false);
    assert!(CALLS.with(|s| s.borrow().is_empty()));
    assert!(JOB_CALLS.with(|s| s.borrow().is_empty()));
    assert!(!FAILED.load(Ordering::Acquire));
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
}
#[test]
fn host_allocator_null_releases_reservation_without_task_birth_or_world_authority() {
    if !child("host_allocator_null_releases_reservation_without_task_birth_or_world_authority") {
        return;
    }
    EXE.store(0x140000000, Ordering::Release);
    let mut view = vec![0u64; 0x2e0 / 8];
    let pointer = view.as_mut_ptr() as u64;
    view[0x280 / 8] = 0x9500;
    let invocation = {
        let mut s = state();
        let request = s.ledger.enter().unwrap();
        s.ledger.fixture_copied(request, 0x9000).unwrap();
        s.ledger.issued(request, 0x9100, 0x9200).unwrap();
        s.ledger.returned(request, true).unwrap();
        s.ledger.consuming(0x9100, 0x9200).unwrap();
        s.ledger.selected(request, 0x9100, 0x9300, pointer).unwrap();
        s.ledger.consumed(request, 0x9100).unwrap();
        s.ledger
            .initialize(0x9300, pointer, 0x9000)
            .unwrap()
            .unwrap()
    };
    CALLS.with(|s| {
        s.borrow_mut().push(Call {
            serial: 1,
            kind: 89,
            source: None,
            request: None,
            invocation: Some(invocation),
            pointer,
            descriptor: 0,
            engine: 0,
            allocation: None,
            joined: false,
        })
    });
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 23;
    raw.invocation.caller = exe(0x985904);
    raw.invocation.caller_rsp = 0x88000;
    job_begin(&mut raw);
    assert_eq!(state().groups.get(&invocation.0), Some(&0x9500));
    raw.gpr[4] = 0;
    job_finish(&mut raw, false);
    CALLS.with(|s| {
        s.borrow_mut().pop().unwrap();
    });
    let mut s = state();
    assert!(s.tasks.is_empty());
    assert!(s.tasks_by_address.is_empty());
    assert!(s.worlds.is_empty());
    assert!(!s.groups.contains_key(&invocation.0));
    assert!(JOB_CALLS.with(|s| s.borrow().is_empty()));
    assert!(!FAILED.load(Ordering::Acquire));
    // A later actual allocation can reserve this invocation. The failed
    // attempt has neither retained a worker nor manufactured a joined task.
    s.ledger.reserve_worker(invocation).unwrap();
}

#[test]
fn all_thirteen_native_load_entry_exchanges_execute_original_once_and_restore() {
    if !child("all_thirteen_native_load_entry_exchanges_execute_original_once_and_restore") {
        return;
    }
    let Some(_fixture) = crate::windows::atomic_fixture::require(
        "all_thirteen_native_load_entry_exchanges_execute_original_once_and_restore",
    ) else {
        return;
    };
    let size = 0x2400000;
    let base = unsafe { VirtualAlloc(null_mut(), size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
        as usize;
    assert_ne!(base, 0);
    for (i, (rva, expected, stolen)) in SITES.into_iter().enumerate() {
        let p = (base + rva) as *mut u8;
        unsafe {
            ptr::copy_nonoverlapping(expected.as_ptr(), p, 8);
        }
        let mut tail = vec![];
        // Atomically count exactly one original body. RCX is the fixture's
        // valid native receiver; all synthetic bodies share this counter.
        tail.extend_from_slice(&[0xf0, 0x48, 0xff, 0x81, 0, 0x60, 0, 0]);
        let result = if i == 1 { 0 } else { 0x1122_3344_5566_7788u64 };
        tail.extend_from_slice(&[0x48, 0xb8]);
        tail.extend_from_slice(&result.to_le_bytes());
        match i {
            2 => tail.extend_from_slice(&[0x41, 0x5f, 0x5f, 0x5e, 0x5d]),
            6 => tail.extend_from_slice(&[0x41, 0x5e, 0x41, 0x5c, 0x5d]),
            9 => tail.extend_from_slice(&[0x48, 0x83, 0xc4, 0x38]),
            10 => tail.extend_from_slice(&[0x48, 0x83, 0xc4, 0x20, 0x5e]),
            12 => tail.extend_from_slice(&[0x48, 0x83, 0xc4, 0x20, 0x5b]),
            _ => {}
        }
        tail.extend_from_slice(&[0xf9, 0xc3]);
        if i == 8 {
            // Both relocated branches must remain inside owned native code.
            unsafe {
                ptr::copy_nonoverlapping(tail.as_ptr(), (base + 0x16d29b) as *mut u8, tail.len());
            }
        }
        unsafe {
            ptr::copy_nonoverlapping(tail.as_ptr(), p.add(stolen), tail.len());
        }
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
    let mut synthetic = SITES;
    for (rva, expected, stolen) in &mut synthetic {
        let actual = copy_current::<8>((base + *rva) as u64).unwrap();
        assert_eq!(&actual[..(*stolen).min(8)], &expected[..(*stolen).min(8)]);
        *expected = actual;
    }
    unsafe { install_at(base, &synthetic) }.unwrap();
    let mut receiver = vec![0u64; 0x802000 / 8];
    let pointer = receiver.as_mut_ptr() as u64;
    unsafe {
        *((pointer + 0x14f0) as *mut u64) = 0x1234;
    }
    for (i, (rva, _, _)) in SITES.into_iter().enumerate() {
        let call: unsafe extern "system" fn(u64, u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(base + rva) };
        let before = receiver[0x6000 / 8];
        unsafe {
            SetLastError(0x123456);
        }
        let result = unsafe { call(pointer, if i == 8 { 0 } else { 2 }, 0, 0, 0xabcdef) };
        assert_eq!(
            result,
            if i == 1 { 0 } else { 0x1122_3344_5566_7788 },
            "site{i}"
        );
        assert_eq!(receiver[0x6000 / 8], before + 1, "original body count{i}");
        assert_eq!(unsafe { GetLastError() }, 0x123456, "LastError{i}");
        assert!(CALLS.with(|s| s.borrow().is_empty()));
        assert!(!FAILED.load(Ordering::Acquire));
    }
    unsafe { restore() }.unwrap();
    assert!(HOOKS.lock().unwrap().is_empty());
    for (rva, expected, _) in synthetic {
        assert_eq!(copy_current::<8>((base + rva) as u64), Some(expected));
    }
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
}
unsafe extern "system" {
    fn native_load_snapshot_original_body();
    fn native_load_copy_original_body();
    fn native_load_command_original_body();
    fn native_load_initialize_original_body();
    fn native_load_worker_original_body();
}
#[test]
fn load_shims_virtual_unwind_restore_nonvolatile_registers_and_original_return() {
    let sites = [
        native_load_snapshot_original_body as *const (),
        native_load_copy_original_body as *const (),
        native_load_command_original_body as *const (),
        native_load_initialize_original_body as *const (),
        native_load_worker_original_body as *const (),
    ];
    for (i, address) in sites.into_iter().enumerate() {
        let mut stack = [0u64; 64];
        let entry = unsafe { stack.as_mut_ptr().add(20) } as u64;
        unsafe {
            *(entry as *mut u64) = 0xabcdef;
        }
        let mut c: CONTEXT = unsafe { std::mem::zeroed() };
        c.Rip = address as u64;
        c.Rsp = entry;
        match i {
            1 => {
                c.Rsp -= 32;
                for (j, v) in [0xf15, 0xd1, 0x51, 0xb1].into_iter().enumerate() {
                    unsafe {
                        *((c.Rsp + 8 * j as u64) as *mut u64) = v;
                    }
                }
            }
            3 => {
                c.Rsp -= 24;
                for (j, v) in [0xe14, 0xc12, 0xb1].into_iter().enumerate() {
                    unsafe {
                        *((c.Rsp + 8 * j as u64) as *mut u64) = v;
                    }
                }
            }
            _ => unsafe {
                *((entry + 8) as *mut u64) = 0xb1;
            },
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
        assert_eq!(c.Rip, 0xabcdef, "return{i}");
        assert_eq!(c.Rsp, entry + 8, "rsp{i}");
        if i == 1 {
            assert_eq!((c.R15, c.Rdi, c.Rsi, c.Rbp), (0xf15, 0xd1, 0x51, 0xb1));
        } else if i == 3 {
            assert_eq!((c.R14, c.R12, c.Rbp), (0xe14, 0xc12, 0xb1));
        } else {
            assert_eq!(c.Rbx, 0xb1, "rbx{i}");
        }
    }
}
#[test]
#[ignore = "explicit licensed local primary-image acceptance"]
fn pinned_native_load_complete_bodies_partitions_and_entry_boundaries_match() {
    let mut f = pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
    for body in fixed::FRAMES {
        let offset = pe_file_offset(&mut f, body.begin).unwrap();
        f.seek(SeekFrom::Start(offset)).unwrap();
        let mut bytes = vec![0; (body.end - body.begin) as usize];
        f.read_exact(&mut bytes).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), body.sha);
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert_ne!(
            format!("{:x}", Sha256::digest(&bytes)),
            body.sha,
            "tail mutation"
        );
        for part in body.parts {
            let offset = data_file_offset(&mut f, part.unwind).unwrap();
            f.seek(SeekFrom::Start(offset)).unwrap();
            let expected: Vec<u8> = (0..part.hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&part.hex[i..i + 2], 16).unwrap())
                .collect();
            let mut bytes = vec![0; expected.len()];
            f.read_exact(&mut bytes).unwrap();
            assert_eq!(bytes, expected);
        }
    }
    for (rva, expected, _) in SITES {
        let offset = pe_file_offset(&mut f, rva as u32).unwrap();
        f.seek(SeekFrom::Start(offset)).unwrap();
        let mut bytes = [0; 8];
        f.read_exact(&mut bytes).unwrap();
        assert_eq!(bytes, expected);
    }
}
fn data_file_offset(file: &mut File, rva: u32) -> Option<u64> {
    file.seek(SeekFrom::Start(0x3c)).ok()?;
    let mut d = [0; 4];
    file.read_exact(&mut d).ok()?;
    let nt = u64::from(u32::from_le_bytes(d));
    file.seek(SeekFrom::Start(nt + 6)).ok()?;
    let mut w = [0; 2];
    file.read_exact(&mut w).ok()?;
    let n = u16::from_le_bytes(w);
    file.seek(SeekFrom::Start(nt + 20)).ok()?;
    file.read_exact(&mut w).ok()?;
    let opt = u64::from(u16::from_le_bytes(w));
    for i in 0..n {
        file.seek(SeekFrom::Start(nt + 24 + opt + u64::from(i) * 40))
            .ok()?;
        let mut s = [0; 40];
        file.read_exact(&mut s).ok()?;
        let start = u32::from_le_bytes(s[12..16].try_into().ok()?);
        let size = u32::from_le_bytes(s[16..20].try_into().ok()?);
        if rva >= start && rva.checked_sub(start)? < size {
            return Some(
                u64::from(u32::from_le_bytes(s[20..24].try_into().ok()?)) + u64::from(rva - start),
            );
        }
    }
    None
}
