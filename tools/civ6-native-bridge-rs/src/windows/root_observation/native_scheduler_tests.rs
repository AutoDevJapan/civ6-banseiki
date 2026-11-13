use super::*;
use std::os::windows::process::CommandExt;
use windows_sys::Win32::System::Diagnostics::Debug::{
    IMAGE_RUNTIME_FUNCTION_ENTRY, RtlLookupFunctionEntry,
};
static ACTUAL_ALLOC: AtomicBool = AtomicBool::new(false);
static ALLOC_TARGET: AtomicU64 = AtomicU64::new(0);
pub(super) static SHARED_PUSH: AtomicBool = AtomicBool::new(false);
static FLOW: AtomicBool = AtomicBool::new(false);
static TASK: AtomicU64 = AtomicU64::new(0);
static ACTION: AtomicU64 = AtomicU64::new(0);
static OBSERVED: AtomicU64 = AtomicU64::new(0);
static EARLY_DISPATCH: AtomicU64 = AtomicU64::new(0);

#[test]
fn unbound_spawn_is_retained_until_full_return_before_wait() {
    if !child("unbound_spawn_is_retained_until_full_return_before_wait") {
        return;
    }
    // Rust observer calls with modeled arguments, no original native body.
    assert!(FRAMES.get().is_none());
    const MANAGER: u64 = 0x2000;
    const LIST: u32 = 7;
    const CORE: u64 = 0x100000;
    closure::bind_core(CORE);
    closure::init_returned(MANAGER, LIST).unwrap();
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 132;
    raw.gpr[0] = u64::from(LIST);
    unsafe { SetLastError(0x4321) };
    begin(&mut raw);
    assert_eq!(unsafe { GetLastError() }, 0x4321);
    assert!(SCOPES.with(|scopes| {
        let scopes = scopes.borrow();
        let scope = scopes.last().unwrap();
        scope.hold.is_none() && scope.job.is_none()
    }));
    closure::close_begin(0x1000, MANAGER, LIST).unwrap();
    eprintln!("SPAWN_ORIGINAL_STILL_ACTIVE");
    assert_eq!(
        closure::wait_returned(MANAGER),
        Err(closure::Violation::PublicationPending),
        "native Wait accepted a Spawn whose original frame has not returned"
    );
    raw.gpr[4] = 0x7788;
    // Model the original body's LastError after the assertion/diagnostic APIs.
    unsafe { SetLastError(0x4321) };
    end(&mut raw);
    assert_eq!((raw.gpr[4], unsafe { GetLastError() }), (0x7788, 0x4321));
    assert!(SCOPES.with(|scopes| scopes.borrow().is_empty()));
    closure::wait_returned(MANAGER).unwrap();
    closure::list_released(LIST, CORE + 0x4a120e, MANAGER).unwrap();
    closure::close_end(MANAGER).unwrap();
    let receipt = closure::world_closed(0x1000).unwrap();
    assert!(receipt.publication_overlap && !receipt.complete());

    // Returning before Wait lets the next normal incarnation make progress.
    closure::init_returned(MANAGER, LIST + 1).unwrap();
    raw.gpr[0] = u64::from(LIST + 1);
    begin(&mut raw);
    end(&mut raw);
    closure::close_begin(0x1001, MANAGER, LIST + 1).unwrap();
    closure::wait_returned(MANAGER).unwrap();
    closure::list_released(LIST + 1, CORE + 0x4a120e, MANAGER).unwrap();
    closure::close_end(MANAGER).unwrap();
    assert!(closure::world_closed(0x1001).unwrap().complete());
    assert!(!FAILED.load(Ordering::Acquire));
}

#[test]
fn unrelated_and_released_spawn_lists_remain_transparent() {
    if !child("unrelated_and_released_spawn_lists_remain_transparent") {
        return;
    }
    closure::bind_core(0x100000);
    closure::init_returned(0x2000, 7).unwrap();
    closure::close_begin(0x1000, 0x2000, 7).unwrap();
    closure::wait_returned(0x2000).unwrap();
    closure::list_released(7, 0x100000 + 0x4a120e, 0x2000).unwrap();
    for list in [0, 7, 99] {
        let mut raw: Saved = unsafe { std::mem::zeroed() };
        raw.invocation.kind = 132;
        raw.gpr[0] = list;
        begin(&mut raw);
        assert!(SCOPES.with(|scopes| scopes.borrow().last().unwrap().publication.is_none()));
        raw.gpr[4] = 0x7788;
        unsafe { SetLastError(0x4321) };
        end(&mut raw);
        assert_eq!((raw.gpr[4], unsafe { GetLastError() }), (0x7788, 0x4321));
    }
    closure::close_end(0x2000).unwrap();
    let receipt = closure::world_closed(0x1000).unwrap();
    assert!(!receipt.late_spawn && receipt.complete());
    assert!(!FAILED.load(Ordering::Acquire));
}

#[test]
fn spawn_publication_unwind_cannot_certify_wait_or_a_new_incarnation() {
    if !child("spawn_publication_unwind_cannot_certify_wait_or_a_new_incarnation") {
        return;
    }
    closure::init_returned(0x2000, 7).unwrap();
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 132;
    raw.gpr[0] = 7;
    begin(&mut raw);
    closure::close_begin(0x1000, 0x2000, 7).unwrap();
    finish(&mut raw, true);
    assert!(SCOPES.with(|scopes| scopes.borrow().is_empty()));
    assert_eq!(
        closure::wait_returned(0x2000),
        Err(closure::Violation::Unwound)
    );
    assert_eq!(
        closure::init_returned(0x2000, 7),
        Err(closure::Violation::Unwound)
    );
}

#[test]
fn unbound_publisher_is_retained_until_full_return_before_wait() {
    if !child("unbound_publisher_is_retained_until_full_return_before_wait") {
        return;
    }
    // Call the production observers directly. No installed native entries,
    // native game function, or FRAMES authority is needed for this gap.
    assert!(FRAMES.get().is_none());
    for kind in [130, 131] {
        let manager = 0x2000 + kind * 0x1000;
        let world = manager + 0x800;
        closure::init_returned(manager, 0).unwrap();
        let mut raw: Saved = unsafe { std::mem::zeroed() };
        raw.invocation.kind = kind;
        raw.gpr[if kind == 130 { 1 } else { 2 }] = manager + 0x400;
        unsafe { SetLastError(0x1234) };
        begin(&mut raw);
        assert_eq!(unsafe { GetLastError() }, 0x1234);
        assert!(SCOPES.with(|scopes| {
            let scopes = scopes.borrow();
            let scope = scopes.last().unwrap();
            scope.hold.is_none() && scope.job.is_none()
        }));
        closure::close_begin(world, manager, 0).unwrap();
        assert!(
            closure::wait_returned(manager).is_err(),
            "native Wait accepted an Add/Push whose original frame has not returned"
        );
        end(&mut raw);
        assert_eq!(unsafe { GetLastError() }, 0x1234);
        assert!(SCOPES.with(|scopes| scopes.borrow().is_empty()));
        closure::wait_returned(manager).unwrap();
        closure::close_end(manager).unwrap();
        let receipt = closure::world_closed(world).unwrap();
        assert!(receipt.publication_overlap && !receipt.complete());
        // The next real incarnation can complete when publication returns
        // before the native Wait, rather than losing progress permanently.
        closure::init_returned(manager, 0).unwrap();
        begin(&mut raw);
        end(&mut raw);
        closure::close_begin(world + 1, manager, 0).unwrap();
        closure::wait_returned(manager).unwrap();
        closure::close_end(manager).unwrap();
        assert!(closure::world_closed(world + 1).unwrap().complete());
    }
}

#[test]
fn unbound_publication_unwind_cannot_certify_wait_or_a_new_incarnation() {
    if !child("unbound_publication_unwind_cannot_certify_wait_or_a_new_incarnation") {
        return;
    }
    closure::init_returned(0x2000, 0).unwrap();
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 130;
    raw.gpr[1] = 0x2400;
    begin(&mut raw);
    closure::close_begin(0x1000, 0x2000, 0).unwrap();
    finish(&mut raw, true);
    assert!(SCOPES.with(|scopes| scopes.borrow().is_empty()));
    assert_eq!(
        closure::wait_returned(0x2000),
        Err(closure::Violation::Unwound)
    );
    assert_eq!(
        closure::init_returned(0x2000, 0),
        Err(closure::Violation::Unwound)
    );
}

#[test]
fn unrelated_publisher_keeps_original_return_without_a_manager_receipt() {
    if !child("unrelated_publisher_keeps_original_return_without_a_manager_receipt") {
        return;
    }
    closure::init_returned(0x2000, 0).unwrap();
    for kind in [130, 131] {
        let mut raw: Saved = unsafe { std::mem::zeroed() };
        raw.invocation.kind = kind;
        raw.gpr[if kind == 130 { 1 } else { 2 }] = 0x8400;
        begin(&mut raw);
        assert!(SCOPES.with(|scopes| scopes.borrow().last().unwrap().publication.is_none()));
        raw.gpr[4] = 0x12345678;
        unsafe { SetLastError(0x5678) };
        end(&mut raw);
        assert_eq!(
            (raw.gpr[4], unsafe { GetLastError() }),
            (0x12345678, 0x5678)
        );
    }
    assert!(!FAILED.load(Ordering::Acquire));
    closure::close_begin(0x1000, 0x2000, 0).unwrap();
    closure::wait_returned(0x2000).unwrap();
    closure::close_end(0x2000).unwrap();
    assert!(closure::world_closed(0x1000).unwrap().complete());
}

#[test]
fn unbound_publication_wait_keeps_its_deadline_and_resumes_after_normal_return() {
    if !child("unbound_publication_wait_keeps_its_deadline_and_resumes_after_normal_return") {
        return;
    }
    publication_wait_case(131);
}

#[test]
fn spawn_publication_wait_keeps_its_deadline_and_resumes_after_normal_return() {
    if !child("spawn_publication_wait_keeps_its_deadline_and_resumes_after_normal_return") {
        return;
    }
    publication_wait_case(132);
}

fn publication_wait_case(kind: u64) {
    use std::sync::mpsc;
    let list = if kind == 132 { 7 } else { 0 };
    closure::bind_core(0x100000);
    closure::init_returned(0x2000, list).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let publisher = std::thread::spawn(move || {
        let mut raw: Saved = unsafe { std::mem::zeroed() };
        raw.invocation.kind = kind;
        if kind == 132 {
            raw.gpr[0] = u64::from(list);
        } else {
            raw.gpr[2] = 0x2400;
        }
        begin(&mut raw);
        entered_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        end(&mut raw);
    });
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    closure::close_begin(0x1000, 0x2000, list).unwrap();
    // An expired budget cannot create a Waited receipt or erase the hold.
    assert_eq!(
        closure::wait_returned_until(0x2000, Instant::now()),
        Err(closure::Violation::PublicationPending)
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let (complete_tx, complete_rx) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        complete_tx
            .send(closure::wait_returned_until(0x2000, deadline))
            .unwrap();
    });
    let ready_deadline = Instant::now() + Duration::from_secs(2);
    while closure::PUBLICATION_WAITERS.load(Ordering::Acquire) == 0 {
        assert!(
            Instant::now() < ready_deadline,
            "Wait observer did not reach its drain"
        );
        std::thread::yield_now();
    }
    // The publisher is explicitly parked; zero original jobs is not a drain.
    assert_eq!(
        closure::wait_returned(0x2000),
        Err(closure::Violation::PublicationPending)
    );
    assert!(matches!(
        complete_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    release_tx.send(()).unwrap();
    publisher.join().unwrap();
    assert_eq!(
        complete_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        Ok(())
    );
    waiter.join().unwrap();
    if list != 0 {
        closure::list_released(list, 0x100000 + 0x4a120e, 0x2000).unwrap();
    }
    closure::close_end(0x2000).unwrap();
    let receipt = closure::world_closed(0x1000).unwrap();
    assert!(receipt.publication_overlap && !receipt.complete());
}

pub(super) fn caller_override(rva: u64) -> Option<u64> {
    if !FLOW.load(Ordering::Acquire) {
        return None;
    }
    Some(match rva {
        0x986497 => fixture_shared_return as *const () as u64,
        0x985904 => {
            if ACTUAL_ALLOC.load(Ordering::Acquire) {
                fixture_real_alloc_return as *const () as u64
            } else {
                fixture_alloc_return as *const () as u64
            }
        }
        0x985f9e => fixture_alloc_return as *const () as u64,
        _ => return None,
    })
}
fn child(name: &str) -> bool {
    const ENV: &str = "CIV6_SCHEDULER_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    let full_name = format!("windows::root_observation::native_frames::scheduler::tests::{name}");
    let output = crate::windows::test_child_output::run(
        std::process::Command::new(std::env::current_exe().unwrap())
            .creation_flags(0x08000000)
            .env(ENV, "1")
            .args(["--exact", &full_name, "--include-ignored", "--nocapture"]),
        Duration::from_secs(20),
    )
    .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "scheduler child{}\n{stdout}\n{stderr}",
        output.status
    );
    assert!(
        stdout.contains("running 1 test") && stdout.contains(&format!("{full_name} ... ok")),
        "scheduler child did not execute its exact test: {stdout}\n{stderr}"
    );
    print!("{stdout}");
    false
}
extern "system" fn producer_body(payload: u64, _: u64, _: u64) -> u64 {
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 23;
    raw.invocation.caller = fixture_alloc_return as *const () as u64;
    raw.gpr[4] = TASK.load(Ordering::Acquire);
    allocated(&raw, false);
    let scope = SCOPES.with(|s| {
        let s = s.borrow();
        let c = s.last().unwrap();
        (c.job, c.hold, c.task)
    });
    OBSERVED.store(
        u64::from(scope.0.is_some()) | u64::from(scope.1.is_some()) << 1,
        Ordering::Release,
    );
    if scope.0.is_some() && scope.2 != 0 && ACTION.load(Ordering::Acquire) == 1 {
        let f = FRAMES.get().unwrap();
        let id = f.execute(scope.2, payload).unwrap();
        f.completed(id).unwrap();
        let retired = f.retire_task(scope.2).unwrap().unwrap();
        f.task_retired(retired).unwrap();
    }
    0x1122334455667788
}
extern "system" fn nested_body(_: u64, task: u64, _: u64) -> u64 {
    attach_current(task);
    let f = FRAMES.get().unwrap();
    let id = f.execute(task, 0x100).unwrap();
    f.completed(id).unwrap();
    OBSERVED.store(
        u64::from(
            f.seal_executions_and_wait(Instant::now() + Duration::from_millis(20))
                == Err(crate::native_flight::Error::Reentrant),
        ),
        Ordering::Release,
    );
    0x88
}
global_asm!(
".globl fixture_producer_original",".seh_proc fixture_producer_original","fixture_producer_original:","mov [rsp+0x10],rbx",".seh_savereg rbx,0x10",".seh_endprologue","jmp {producer}",".seh_endproc",
".globl fixture_nested_original",".seh_proc fixture_nested_original","fixture_nested_original:","mov [rsp+0x10],rbx",".seh_savereg rbx,0x10",".seh_endprologue","jmp {nested}",".seh_endproc",
".globl fixture_alloc_return","fixture_alloc_return:","ret",
producer=sym producer_body,nested=sym nested_body,
);
unsafe extern "system" {
    fn fixture_producer_original();
    fn fixture_nested_original();
    fn fixture_alloc_return();
    fn fixture_shared_return();
}
fn prepare() {
    FLOW.store(true, Ordering::Release);
    EXE.store(0x140000000, Ordering::Release);
    FRAMES.get_or_init(NativeFrames::new);
    NATIVE_SCHEDULER_CONTINUE[0].store(
        fixture_producer_original as *const () as u64 + 5,
        Ordering::Release,
    );
}
#[test]
fn add_job_reserves_before_publication_and_fullreturn_releases_producer() {
    if !child("add_job_reserves_before_publication_and_fullreturn_releases_producer") {
        return;
    }
    prepare();
    let f = FRAMES.get().unwrap();
    let root = f.enter().unwrap();
    TASK.store(0x200, Ordering::Release);
    ACTION.store(1, Ordering::Release);
    let call: unsafe extern "system" fn(u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(gateways()[0]) };
    unsafe {
        SetLastError(0x12345);
        assert_eq!(call(0x100, 0x400, 0), 0x1122334455667788);
    };
    assert_eq!(unsafe { GetLastError() }, 0x12345);
    assert_eq!(OBSERVED.load(Ordering::Acquire), 3);
    assert_eq!(f.pending_on_set(0x400), Ok(0));
    f.leave(root).unwrap();
    f.seal_executions_and_wait(Instant::now() + Duration::from_secs(1))
        .unwrap();
    assert!(SCOPES.with(|s| s.borrow().is_empty()));
}
#[test]
fn unknown_add_and_null_original_keep_no_authority_and_known_null_cancels_only_unbound() {
    if !child("unknown_add_and_null_original_keep_no_authority_and_known_null_cancels_only_unbound")
    {
        return;
    }
    prepare();
    let f = FRAMES.get().unwrap();
    let call: unsafe extern "system" fn(u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(gateways()[0]) };
    unsafe {
        call(0x100, 0x400, 0);
    };
    assert_eq!(OBSERVED.load(Ordering::Acquire), 0);
    assert_eq!(f.pending_on_set(0x400), Ok(0));
    let root = f.enter().unwrap();
    unsafe {
        call(0x100, 0x400, 0);
    };
    assert_eq!(OBSERVED.load(Ordering::Acquire), 3);
    assert_eq!(f.pending_on_set(0x400), Ok(0));
    f.leave(root).unwrap();
    assert!(f.healthy());
}
#[test]
fn complete_publication_hold_survives_completed_t_until_outerreturn() {
    if !child("complete_publication_hold_survives_completed_t_until_outerreturn") {
        return;
    }
    prepare();
    let f = FRAMES.get().unwrap();
    let id = f.issue_on_set(0x100, 0x400).unwrap();
    f.bind_before_publication(id, 0x200).unwrap();
    f.pushed(id, 0x200).unwrap();
    NATIVE_SCHEDULER_CONTINUE[3].store(
        fixture_nested_original as *const () as u64 + 5,
        Ordering::Release,
    );
    let call: unsafe extern "system" fn(u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(gateways()[3]) };
    assert_eq!(unsafe { call(0x300, 0x200, 0x1f8) }, 0x88);
    assert_eq!(OBSERVED.load(Ordering::Acquire), 1);
    assert_eq!(f.pending_on_set(0x400), Ok(0));
    f.seal_executions_and_wait(Instant::now() + Duration::from_secs(1))
        .unwrap();
}
#[test]
fn inherited_entry_refuses_epoch_after_drain_without_raw_address_adoption() {
    let f = NativeFrames::new();
    assert_eq!(f.enter_inherited(), Ok(None));
    let root = f.enter().unwrap();
    let h = f.enter_inherited().unwrap().unwrap();
    f.leave(root).unwrap();
    assert_eq!(
        f.seal_executions_and_wait(Instant::now() + Duration::from_millis(10)),
        Err(crate::native_flight::Error::Reentrant)
    );
    f.leave(h).unwrap();
    f.seal_executions_and_wait(Instant::now() + Duration::from_secs(1))
        .unwrap();
    assert_eq!(f.enter_inherited(), Ok(None));
    f.reopen().unwrap();
    assert_eq!(f.consumer_enter(0x200), Ok(None));
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
static UNWIND_STATE: AtomicU64 = AtomicU64::new(0);
global_asm!(
".globl fixture_shared_driver",".seh_proc fixture_shared_driver","fixture_shared_driver:","sub rsp,0x28",".seh_stackalloc 0x28",".seh_endprologue","call {push}",".globl fixture_shared_return","fixture_shared_return:","lea rsp,[rsp+0x28]","ret",".seh_endproc",
".globl scheduler_unwind_driver",".seh_proc scheduler_unwind_driver","scheduler_unwind_driver:","push rbx",".seh_pushreg rbx","sub rsp,0x80",".seh_stackalloc 0x80",".seh_endprologue","mov rbx,rcx","mov [rcx],rsp","lea rax,[rip+scheduler_unwind_landed]","mov [rcx+8],rax","mov rcx,r8","call rdx","xor eax,eax","jmp 2f","scheduler_unwind_landed:","mov eax,1","2:","lea rsp,[rsp+0x80]","pop rbx","ret",".seh_endproc",
".globl scheduler_unwind_original",".seh_proc scheduler_unwind_original","scheduler_unwind_original:","mov [rsp+0x10],rbx",".seh_savereg rbx,0x10","sub rsp,0x528",".seh_stackalloc 0x528",".seh_endprologue","mov r10,[rip+{state}]","mov rcx,[r10]","mov rdx,[r10+8]","lea r10,[rsp+0x40]","mov r11d,0x9a","1:","mov qword ptr [r10],0","lea r10,[r10+8]","dec r11d","jnz 1b","lea r10,[rsp+0x40]","mov [rsp+0x20],r10","mov qword ptr [rsp+0x28],0","xor r8d,r8d","xor r9d,r9d","call {unwind_ex}","int3",".seh_endproc",
push=sym scheduler_push_gateway,state=sym UNWIND_STATE,unwind_ex=sym RtlUnwindEx,
);
unsafe extern "system" {
    fn fixture_shared_driver(a: u64, b: u64, c: u64, d: u64) -> u64;
    fn scheduler_unwind_driver(state: *mut u64, target: usize, arg: u64) -> u64;
    fn scheduler_unwind_original();
}
#[test]
fn existing_push_reservation_is_not_issued_a_second_time() {
    if !child("existing_push_reservation_is_not_issued_a_second_time") {
        return;
    }
    prepare();
    let f = FRAMES.get().unwrap();
    let root = f.enter().unwrap();
    let id = f.issue_on_set(0x100, 0x400).unwrap();
    SHARED_PUSH.store(true, Ordering::Release);
    NATIVE_SCHEDULER_CONTINUE[1].store(
        fixture_producer_original as *const () as u64 + 5,
        Ordering::Release,
    );
    assert_eq!(
        unsafe { fixture_shared_driver(1, 0x100, 0x400, 0) },
        0x1122334455667788
    );
    assert_eq!(OBSERVED.load(Ordering::Acquire), 2);
    assert_eq!(f.pending_on_set(0x400), Ok(1));
    f.cancel_unpublished_job(id).unwrap();
    f.leave(root).unwrap();
}
#[test]
fn actual_unwind_does_not_cancel_a_known_reservation_or_poison_unknown() {
    if !child("actual_unwind_does_not_cancel_a_known_reservation_or_poison_unknown") {
        return;
    }
    prepare();
    let mut landing = [0u64; 2];
    UNWIND_STATE.store(landing.as_mut_ptr() as u64, Ordering::Release);
    NATIVE_SCHEDULER_CONTINUE[0].store(
        scheduler_unwind_original as *const () as u64 + 5,
        Ordering::Release,
    );
    assert_eq!(
        unsafe { scheduler_unwind_driver(landing.as_mut_ptr(), gateways()[0], 0x100) },
        1
    );
    assert!(FRAMES.get().unwrap().healthy());
    let root = FRAMES.get().unwrap().enter().unwrap();
    // Publication scope (RDX=T) has a known frame but never owns cancellation.
    NATIVE_SCHEDULER_CONTINUE[3].store(
        scheduler_unwind_original as *const () as u64 + 5,
        Ordering::Release,
    );
    assert_eq!(
        unsafe { scheduler_unwind_driver(landing.as_mut_ptr(), gateways()[3], 0x100) },
        1
    );
    assert!(!FRAMES.get().unwrap().healthy());
    assert!(SCOPES.with(|s| s.borrow().is_empty()));
    FRAMES.get().unwrap().leave(root).unwrap();
}
fn offset(file: &mut File, rva: u32) -> u64 {
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
        let begin = u32::from_le_bytes(b[12..16].try_into().unwrap());
        let size = u32::from_le_bytes(b[16..20].try_into().unwrap());
        if rva >= begin && rva - begin < size {
            return u64::from(u32::from_le_bytes(b[20..24].try_into().unwrap()))
                + u64::from(rva - begin);
        }
    }
    panic!("missing RVA{rva:x}")
}
fn span(file: &mut File, rva: u32, n: usize) -> Vec<u8> {
    let off = offset(file, rva);
    file.seek(SeekFrom::Start(off)).unwrap();
    let mut b = vec![0; n];
    file.read_exact(&mut b).unwrap();
    b
}
#[test]
#[ignore = "explicit licensed local Host/Core primary-image acceptance"]
fn primary_host_core_frames_and_exact_fieldflow_match() {
    let mut host = super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
    let mut core = super::super::pin(
        GAMECORE,
        "092abfbb2176f8df04b5212f819dc269dd83aea866ff05aaf489904a1ff7c41f",
    )
    .unwrap();
    for (is_core, frames) in [(false, fixed::FRAMES), (true, audit::core_frames())] {
        for f in frames {
            let file = if is_core { &mut core } else { &mut host };
            let mut b = span(file, f.begin, (f.end - f.begin) as usize);
            assert!(code_matches(&b, f));
            let expected: Vec<u8> = (0..f.specimen.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&f.specimen[i..i + 2], 16).unwrap())
                .collect();
            assert_eq!(b, expected);
            let end = b.len() - 1;
            b[end] ^= 1;
            assert!(!code_matches(&b, f));
            for p in f.parts {
                let expected: Vec<u8> = (0..p.hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&p.hex[i..i + 2], 16).unwrap())
                    .collect();
                assert_eq!(span(file, p.unwind, expected.len()), expected);
            }
        }
    }
    audit::verify_flow(|is_core, r, n| {
        Some(span(if is_core { &mut core } else { &mut host }, r, n))
    })
    .unwrap();
    let info = audit::scope();
    assert_eq!(info["certificate_issued"], false);
    assert_eq!(info["product_acceptance"], false);
}
#[test]
#[ignore = "explicit licensed local Host/Core primary-image mutation acceptance"]
fn flow_mutation_cannot_satisfy_wrong_virtualslot_or_publication_edge() {
    let mut host = super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
    let mut core = super::super::pin(
        GAMECORE,
        "092abfbb2176f8df04b5212f819dc269dd83aea866ff05aaf489904a1ff7c41f",
    )
    .unwrap();
    for bad in [0x11206a8, 0xe41ad8, 0x985f99, 0x4a1145, 0x9860fb] {
        assert!(
            audit::verify_flow(|is_core, r, n| {
                let mut b = span(if is_core { &mut core } else { &mut host }, r, n);
                if r == bad {
                    let end = b.len() - 1;
                    b[end] ^= 1;
                }
                Some(b)
            })
            .is_err(),
            "mutation{bad:x}"
        );
    }
}
#[test]
fn eight_installed_gateways_run_original_once_and_restore() {
    if !child("eight_installed_gateways_run_original_once_and_restore") {
        return;
    }
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0xe50000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    let mut synthetic = SITES;
    for (rva, prefix, stolen) in &mut synthetic {
        let p = (base + *rva) as *mut u8;
        unsafe { ptr::copy_nonoverlapping(prefix.as_ptr(), p, 8) };
        let mut tail = vec![0xf0, 0x48, 0xff, 0x81, 0, 0x60, 0, 0, 0x48, 0xb8];
        tail.extend_from_slice(&0x1122334455667788u64.to_le_bytes());
        if *rva == 0x986010 {
            tail.extend_from_slice(&[0x48, 0x83, 0xc4, 0x30, 0x5b]);
        }
        if *rva == 0x985bd0 {
            tail.extend_from_slice(&[0x48, 0x83, 0xc4, 0x40, 0x5d]);
        }
        tail.extend_from_slice(&[0xf9, 0xc3]);
        unsafe { ptr::copy_nonoverlapping(tail.as_ptr(), p.add(*stolen), tail.len()) };
        *prefix = copy_current::<8>((base + *rva) as u64).unwrap();
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
    let mut receiver = vec![0u64; 0x7000 / 8];
    let p = receiver.as_mut_ptr() as u64;
    for (rva, _, _) in SITES {
        let before = receiver[0x6000 / 8];
        let call: unsafe extern "system" fn(u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(base + rva) };
        unsafe {
            SetLastError(0x123456);
            assert_eq!(call(p, 0, 0, 0), 0x1122334455667788)
        };
        assert_eq!(unsafe { GetLastError() }, 0x123456);
        assert_eq!(receiver[0x6000 / 8], before + 1);
    }
    unsafe { restore() }.unwrap();
    for (rva, prefix, _) in synthetic {
        assert_eq!(copy_current::<8>((base + rva) as u64), Some(prefix));
    }
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
}
#[test]
fn displaced_shims_have_exact_virtual_unwind_and_no_extra_incoming_args() {
    let sites = [
        (scheduler_add_original as *const (), 0x10, 0),
        (scheduler_push_original as *const (), 0x10, 0),
        (scheduler_spawn_original as *const (), 0, 0x38),
        (scheduler_publish_original as *const (), 0x10, 0),
        (scheduler_dispatch_a_original as *const (), 0x18, 0),
        (scheduler_dispatch_b_original as *const (), 0x18, 0),
        (scheduler_destroy_original as *const (), 8, 0),
        (scheduler_free_original as *const (), 0, 0x48),
    ];
    for (addr, offset, below) in sites {
        let mut stack = [0u64; 80];
        let entry = unsafe { stack.as_mut_ptr().add(32) } as u64;
        unsafe {
            *(entry as *mut u64) = 0xabcdef;
            *((if below == 0 {
                entry + offset
            } else {
                entry - 8
            }) as *mut u64) = 0xb1;
        };
        let mut c: CONTEXT = unsafe { std::mem::zeroed() };
        c.Rip = addr as u64 + if below == 0 { 5 } else { 6 };
        c.Rsp = entry - below;
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
        assert_eq!(c.Rsp, entry + 8);
        assert_eq!(c.Rip, 0xabcdef);
        assert_eq!(if below == 0x48 { c.Rbp } else { c.Rbx }, 0xb1);
    }
}

#[test]
fn binding_unmapped_core_cannot_activate_scheduler_proof() {
    assert!(unsafe { bind_core(0) }.is_err());
}

#[test]
fn inherited_unknown_scheduler_stays_zero_across_cookie_and_pendingtask_returns() {
    if !child("inherited_unknown_scheduler_stays_zero_across_cookie_and_pendingtask_returns") {
        return;
    }
    prepare();
    let f = FRAMES.get().unwrap();
    let outer = f.enter_with_provenance(true).unwrap();
    TASK.store(0x200, Ordering::Release);
    let call: unsafe extern "system" fn(u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(gateways()[0]) };
    unsafe {
        call(0x100, 0x400, 0);
    }
    assert_eq!(f.pending_on_set(0x400), Ok(1));
    f.leave(outer).unwrap();
    assert!(!f.unproven_on_current_thread());
    let consumer = f.consumer_enter(0x200).unwrap().unwrap();
    let task = f.execute(0x200, 0x100).unwrap();
    f.completed(task).unwrap();
    assert!(f.unproven_on_current_thread());
    f.consumer_exit(consumer).unwrap();
    assert!(!f.unproven_on_current_thread());
    f.seal_executions_and_wait(Instant::now() + Duration::from_secs(1))
        .unwrap();
}
#[test]
fn load_creator_uses_its_existing_birth_carrier_without_oldworld_reservation() {
    if !child("load_creator_uses_its_existing_birth_carrier_without_oldworld_reservation") {
        return;
    }
    prepare();
    let f = FRAMES.get().unwrap();
    let outer = f.enter().unwrap();
    let mut raw: Saved = unsafe { std::mem::zeroed() };
    raw.invocation.kind = 130;
    raw.invocation.caller = exe(0x98618b);
    raw.gpr[0] = 0x100;
    raw.gpr[1] = 0x400;
    begin(&mut raw);
    assert!(SCOPES.with(|s| {
        let s = s.borrow();
        let c = s.last().unwrap();
        c.hold.is_none() && c.job.is_none()
    }));
    finish(&mut raw, false);
    assert_eq!(f.pending_on_set(0x400), Ok(0));
    f.leave(outer).unwrap();
}

extern "system" fn allocator_publication_observer(payload: u64, task: u64) -> u64 {
    let f = FRAMES.get().unwrap();
    let before =
        value(payload + 8) == Some(0) && value(task + 8) == Some(0) && f.contains_task(task);
    OBSERVED.store(u64::from(before), Ordering::Release);
    unsafe {
        *(task as *mut u64) = exe(0x106c418);
        *((task + 8) as *mut u64) = payload;
        *((payload + 8) as *mut u64) = task;
    }
    0x5522
}
global_asm!(
".globl scheduler_actual_allocator_original",".seh_proc scheduler_actual_allocator_original","scheduler_actual_allocator_original:","mov [rsp+0x10],rbx",".seh_savereg rbx,0x10","push rdi",".seh_pushreg rdi","sub rsp,0x20",".seh_stackalloc 0x20",".seh_endprologue","mov rdi,rcx","call qword ptr [rip+{allocator}]",".globl fixture_real_alloc_return","fixture_real_alloc_return:","mov rdx,rax","mov rcx,rdi","call {observe}","mov rbx,[rsp+0x38]","lea rsp,[rsp+0x20]","pop rdi","ret",".seh_endproc",
allocator=sym ALLOC_TARGET,observe=sym allocator_publication_observer,
);
unsafe extern "system" {
    fn scheduler_actual_allocator_original();
    fn fixture_real_alloc_return();
}
#[test]
fn shared_jobs_allocator_gateway_binds_before_original_task_or_payload_publication() {
    if !child("shared_jobs_allocator_gateway_binds_before_original_task_or_payload_publication") {
        return;
    }
    prepare();
    let f = FRAMES.get().unwrap();
    let root = f.enter().unwrap();
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            0xe40000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(base, 0);
    let mut payload = [0u64; 8];
    let mut task = [0u64; 2];
    let p = payload.as_mut_ptr() as u64;
    let t = task.as_mut_ptr() as u64;
    for (rva, prefix, _) in jobs::fixture_host_sites() {
        unsafe { ptr::copy_nonoverlapping(prefix.as_ptr(), (base + rva) as *mut u8, 8) };
        if rva == 0xe3ccf0 {
            // The shared owner compares eight bytes; keep the complete next
            // home store before the synthetic allocator result instruction.
            let mut tail = vec![0x48, 0x89, 0x6c, 0x24, 0x10, 0x48, 0xb8];
            tail.extend_from_slice(&t.to_le_bytes());
            tail.push(0xc3);
            unsafe {
                ptr::copy_nonoverlapping(tail.as_ptr(), (base + rva + 5) as *mut u8, tail.len());
            }
        }
    }
    let mut old = 0;
    assert_ne!(
        unsafe { VirtualProtect(base as _, 0xe40000, PAGE_EXECUTE_READ, &mut old) },
        0
    );
    unsafe { jobs::fixture_host_install(base) }.unwrap();
    ALLOC_TARGET.store((base + 0xe3ccf0) as u64, Ordering::Release);
    ACTUAL_ALLOC.store(true, Ordering::Release);
    NATIVE_SCHEDULER_CONTINUE[0].store(
        scheduler_actual_allocator_original as *const () as u64 + 5,
        Ordering::Release,
    );
    let call: unsafe extern "system" fn(u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(gateways()[0]) };
    assert_eq!(unsafe { call(p, 0x400, 0) }, 0x5522);
    assert_eq!(
        OBSERVED.load(Ordering::Acquire),
        1,
        "binding precedes original T/P stores"
    );
    assert_eq!(payload[1], t);
    assert_eq!(task[1], p);
    assert_eq!(f.pending_on_set(0x400), Ok(1));
    let id = f.execute(t, p).unwrap();
    f.completed(id).unwrap();
    f.leave(root).unwrap();
    f.seal_executions_and_wait(Instant::now() + Duration::from_secs(1))
        .unwrap();
    unsafe { jobs::restore() }.unwrap();
    assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
}

global_asm!(
    ".globl scheduler_early_worker_original",
    ".seh_proc scheduler_early_worker_original",
    "scheduler_early_worker_original:",
    "mov [rsp+0x10],rbx",
    ".seh_savereg rbx,0x10",
    "sub rsp,0x28",
    ".seh_stackalloc 0x28",
    ".seh_endprologue",
    "call qword ptr [rip+{dispatch}]",
    "mov rbx,[rsp+0x38]",
    "lea rsp,[rsp+0x28]",
    "ret",
    ".seh_endproc",
    ".globl scheduler_early_dispatch_original",
    ".seh_proc scheduler_early_dispatch_original",
    "scheduler_early_dispatch_original:",
    "mov [rsp+0x18],rbx",
    ".seh_savereg rbx,0x18",
    ".seh_endprologue",
    "mov eax,0x112233",
    "ret",
    ".seh_endproc",
    dispatch=sym EARLY_DISPATCH,
);
unsafe extern "system" {
    fn scheduler_early_worker_original();
    fn scheduler_early_dispatch_original();
}
#[test]
fn early_coreless_full_worker_and_nested_dispatch_return_unknown_task_transparently() {
    if !child("early_coreless_full_worker_and_nested_dispatch_return_unknown_task_transparently") {
        return;
    }
    assert!(FRAMES.get().is_none());
    NATIVE_SCHEDULER_CONTINUE[4].store(
        scheduler_early_dispatch_original as *const () as u64 + 5,
        Ordering::Release,
    );
    EARLY_DISPATCH.store(gateways()[4] as u64, Ordering::Release);
    let target =
        consumers::fixture_early_worker(scheduler_early_worker_original as *const () as usize + 5);
    let invoke: unsafe extern "system" fn(u64, u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(target) };
    unsafe { SetLastError(0x880123) };
    assert_eq!(unsafe { invoke(0, 0, 0, 0) }, 0x112233);
    assert_eq!(unsafe { GetLastError() }, 0x880123);
    assert!(FRAMES.get().is_none());
    assert!(SCOPES.with(|s| s.borrow().is_empty()));
    assert!(!FAILED.load(Ordering::Acquire));
}

#[path = "native_scheduler_prepare_tests.rs"]
mod early_prepare;
