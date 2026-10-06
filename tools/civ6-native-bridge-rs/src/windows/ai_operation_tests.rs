use super::*;
use std::sync::atomic::AtomicU32;
static TEST_LOCK: Mutex<()> = Mutex::new(());
static CONSTRUCTS: AtomicU32 = AtomicU32::new(0);
static DESTROYS: AtomicU32 = AtomicU32::new(0);
static EXECUTES: AtomicU32 = AtomicU32::new(0);
static PHASE: AtomicU32 = AtomicU32::new(1);
static UNWIND_STATE: AtomicU64 = AtomicU64::new(0);
unsafe extern "system" {
    fn ai_unwind_driver(state: *mut u64, target: usize) -> u64;
}
unsafe extern "system-unwind" {
    #[link_name = "original_unwinds"]
    fn native_unwind(state: *const u64);
}

unsafe extern "system-unwind" fn execute_unwind(_: *mut Wrapper) {
    EXECUTES.fetch_add(1, Ordering::SeqCst);
    unsafe {
        native_unwind(UNWIND_STATE.load(Ordering::Acquire) as *const u64);
    }
}
unsafe extern "system-unwind" fn native_operation_body(state: *const u64) {
    UNWIND_STATE.store(state as u64, Ordering::Release);
    let _guard = ActionGuard::enter().unwrap();
    let mut table = api();
    table.execute = execute_unwind;
    unsafe {
        table.promote(3, 77, 104, || PHASE.load(Ordering::SeqCst) == 1);
    }
}

#[test]
fn native_operation_foreign_unwind_destroys_tables_and_releases_action_scope() {
    const CHILD: &str = "JAC_NATIVE_OPERATION_UNWIND_CHILD";
    if std::env::var_os(CHILD).is_none() {
        use std::os::windows::process::CommandExt;
        let mut process=std::process::Command::new(std::env::current_exe().unwrap()).creation_flags(0x08000000)
            .env(CHILD,"1").args(["--exact","windows::ai_operation::tests::native_operation_foreign_unwind_destroys_tables_and_releases_action_scope","--nocapture"])
            .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while process.try_wait().unwrap().is_none() {
            if std::time::Instant::now() > deadline {
                let _ = process.kill();
                let _ = process.wait();
                panic!("native cleanup child deadline")
            };
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let output = process.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{:?} {} {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    ENABLED.store(true, Ordering::Release);
    PHASE.store(1, Ordering::SeqCst);
    CONSTRUCTS.store(0, Ordering::SeqCst);
    DESTROYS.store(0, Ordering::SeqCst);
    EXECUTES.store(0, Ordering::SeqCst);
    let mut state = [0u64; 2];
    assert_eq!(
        unsafe {
            ai_unwind_driver(
                state.as_mut_ptr(),
                native_operation_body as *const () as usize,
            )
        },
        1
    );
    assert_eq!(CONSTRUCTS.load(Ordering::SeqCst), 2);
    assert_eq!(NATIVE_UNWINDS.load(Ordering::Acquire), 1);
    assert_eq!(DESTROYS.load(Ordering::SeqCst), 3);
    assert_eq!(EXECUTES.load(Ordering::SeqCst), 1);
    assert_eq!(ACTION_THREAD.load(Ordering::Acquire), 0);
    assert!(close());
    assert!(ActionGuard::enter().is_none());
}
thread_local! {static VALUE:RefCell<[u64;3]>=const{RefCell::new([0;3])};}
unsafe extern "system-unwind" fn construct(p: *mut Dictionary) -> *mut Dictionary {
    CONSTRUCTS.fetch_add(1, Ordering::SeqCst);
    unsafe {
        (*p).0 = [0; 8];
    }
    p
}
unsafe extern "system-unwind" fn destroy(_: *mut Dictionary) {
    DESTROYS.fetch_add(1, Ordering::SeqCst);
}
unsafe extern "system-unwind" fn integer(p: *mut Dictionary, key: u32, value: u32) {
    unsafe {
        (*p).0[if key == 0xaf5e1625 {
            4
        } else if key == 0x6d4bd4ea {
            5
        } else {
            panic!("unexpected key")
        }] = u64::from(value);
    }
}
unsafe extern "system-unwind" fn parameter(p: *mut Dictionary, key: u32) -> u64 {
    VALUE.with(|v| {
        let mut v = v.borrow_mut();
        *v = [
            2,
            unsafe { (*p).0[if key == 0xaf5e1625 { 4 } else { 5 }] },
            0,
        ];
        v.as_ptr() as u64
    })
}
unsafe extern "system-unwind" fn operation(
    owner: u32,
    kind: u32,
    input: *mut Dictionary,
    op: *mut Operation,
) -> u8 {
    assert_eq!(owner, 3);
    assert_eq!(kind, 0x7d3da99d);
    unsafe {
        (*op).kind = kind;
        (*op).parameters.0 = (*input).0;
    }
    1
}
unsafe extern "system-unwind" fn filter(
    owner: u32,
    op: *mut Operation,
    bypass: u8,
    unused: u64,
) -> u8 {
    assert_eq!((owner, bypass, unused), (3, 0, 0));
    assert_eq!(unsafe { (*op).kind }, 0x7d3da99d);
    if PHASE.load(Ordering::SeqCst) == 2 {
        PHASE.store(0, Ordering::SeqCst);
    }
    1
}
unsafe extern "system-unwind" fn wrap(
    w: *mut Wrapper,
    owner: u32,
    op: *mut Operation,
) -> *mut Wrapper {
    unsafe {
        (*w).0[3] = u64::from(owner);
        (*w).0[4] = u64::from((*op).kind) | (u64::from((*op).value_type) << 32);
        ptr::copy_nonoverlapping(
            &(*op).parameters.0 as *const [u64; 8],
            (*w).0.as_mut_ptr().add(5).cast(),
            1,
        );
    }
    if PHASE.load(Ordering::SeqCst) == 3 {
        PHASE.store(0, Ordering::SeqCst);
    }
    w
}
unsafe extern "system-unwind" fn execute(w: *mut Wrapper) {
    assert_eq!(unsafe { (*w).0[3] }, 3);
    assert_eq!(unsafe { (*w).0[4] }, 0x27d3da99d);
    assert_eq!(unsafe { (*w).0[9] }, 77);
    assert_eq!(unsafe { (*w).0[10] }, 104);
    EXECUTES.fetch_add(1, Ordering::SeqCst);
}
unsafe extern "system-unwind" fn unused_skip(_: u64) -> u8 {
    panic!("fixture does not model engine ownership")
}
unsafe extern "system-unwind" fn unused_governors(_: u64) -> u64 {
    panic!("fixture does not model engine objects")
}
unsafe extern "system-unwind" fn unused_governor(_: u64, _: u32) -> u64 {
    panic!("fixture does not model engine objects")
}
unsafe extern "system-unwind" fn unused_spent(_: u64) -> u32 {
    panic!("fixture does not manufacture payment")
}
fn api() -> Api {
    Api {
        dictionary_vtable: 0,
        parameter,
        skip_ai: unused_skip,
        governors: unused_governors,
        governor: unused_governor,
        spent: unused_spent,
        construct,
        destroy,
        integer,
        operation,
        filter,
        wrap,
        execute,
    }
}

#[test]
fn native_operation_abi_passes_exact_keys_and_owner_once_and_destroys_every_dictionary() {
    let _lock = TEST_LOCK.lock().unwrap();
    for phase in 1..=3 {
        CONSTRUCTS.store(0, Ordering::SeqCst);
        DESTROYS.store(0, Ordering::SeqCst);
        EXECUTES.store(0, Ordering::SeqCst);
        PHASE.store(phase, Ordering::SeqCst);
        let sent = unsafe { api().promote(3, 77, 104, || PHASE.load(Ordering::SeqCst) != 0) };
        assert_eq!(sent, phase == 1);
        assert_eq!(EXECUTES.load(Ordering::SeqCst), u32::from(phase == 1));
        assert_eq!(CONSTRUCTS.load(Ordering::SeqCst), 2);
        assert_eq!(
            DESTROYS.load(Ordering::SeqCst),
            if phase == 2 { 2 } else { 3 }
        );
    }
}

#[test]
fn close_drains_native_action_and_rejects_same_thread_teardown() {
    let _lock = TEST_LOCK.lock().unwrap();
    ENABLED.store(true, Ordering::Release);
    let entered = std::sync::Arc::new(std::sync::Barrier::new(2));
    let release = std::sync::Arc::new(std::sync::Barrier::new(2));
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let e = entered.clone();
        let r = release.clone();
        let action = scope.spawn(move || {
            let _guard = ActionGuard::enter().unwrap();
            assert!(!close(), "same-thread close is deferred, never deadlocks");
            e.wait();
            r.wait();
        });
        entered.wait();
        let closer = scope.spawn(move || {
            tx.send(close()).unwrap();
        });
        assert!(rx
            .recv_timeout(std::time::Duration::from_millis(30))
            .is_err());
        release.wait();
        action.join().unwrap();
        closer.join().unwrap();
        assert_eq!(rx.recv().unwrap(), true);
    });
    assert!(ActionGuard::enter().is_none());
    assert_eq!(ACTION_THREAD.load(Ordering::Acquire), 0);
}
