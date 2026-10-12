use super::*;
use std::os::windows::process::CommandExt;
use std::time::{Duration, Instant};
pub(super) static PAUSE_END: AtomicBool = AtomicBool::new(false);
static PAUSE_ORIGINAL: AtomicBool = AtomicBool::new(false);
static ENTERED: AtomicBool = AtomicBool::new(false);
static RELEASE: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);
static RETURNED: AtomicU64 = AtomicU64::new(1);
static ENTRY_PANIC: AtomicBool = AtomicBool::new(false);
pub(in super::super) fn panic_in_begin() {
    if ENTRY_PANIC.swap(false, Ordering::AcqRel) {
        panic!("owned entry panic");
    }
}
pub(super) fn pause(at: &AtomicBool) {
    if at.swap(false, Ordering::AcqRel) {
        ENTERED.store(true, Ordering::Release);
        while !RELEASE.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
    }
}
fn child(name: &str) -> bool {
    const ENV: &str = "TODO880_RUNTIME_LIFECYCLE_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    let executable = std::env::current_exe().unwrap();
    let module = module_path!().split_once("::").unwrap().1;
    let exact_name = format!("{module}::{name}");
    let inventory = std::process::Command::new(&executable)
        .creation_flags(0x08000000)
        .args(["--list", "--exact", &exact_name])
        .output()
        .unwrap();
    assert!(inventory.status.success());
    assert!(
        String::from_utf8_lossy(&inventory.stdout).contains(&format!("{exact_name}: test")),
        "owned child test was not found"
    );
    let mut worker = std::process::Command::new(executable)
        .creation_flags(0x08000000)
        .env(ENV, "1")
        .args(["--exact", &exact_name, "--nocapture"])
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = worker.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = worker.kill();
            let _ = worker.wait();
            panic!("owned child timed out");
        }
        std::thread::yield_now();
    };
    assert!(status.success(), "owned child {name}: {status}");
    false
}
fn wait_for(check: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !check() {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::yield_now();
    }
    true
}
extern "system" fn original(_: u64, _: u32) -> u64 {
    CALLS.fetch_add(1, Ordering::AcqRel);
    pause(&PAUSE_ORIGINAL);
    RETURNED.load(Ordering::Acquire)
}
fn call_property() -> u64 {
    let call = unsafe {
        std::mem::transmute::<usize, unsafe extern "system" fn(u64, u32) -> u64>(
            super::super::property_gateway as *const () as usize,
        )
    };
    unsafe { call(0x1000, 0) }
}
fn setup() -> Box<PropertyTrace> {
    let mut trace = Box::new(PropertyTrace::default());
    PROPERTY_TRACE_PTR.store(trace.as_mut(), Ordering::Release);
    PROPERTY_TRAMPOLINE_PTR.store(original as *const () as *mut u8, Ordering::Release);
    runtime::fixture_invocation_runtime();
    trace
}
#[test]
fn runtime_owned_os_close_waits_through_original_return_and_final_borrow() {
    if !child("runtime_owned_os_close_waits_through_original_return_and_final_borrow") {
        return;
    }
    exercise_final_borrow(false);
}
#[test]
fn runtime_owned_os_expired_observation_still_waits_for_final_borrow() {
    if !child("runtime_owned_os_expired_observation_still_waits_for_final_borrow") {
        return;
    }
    exercise_final_borrow(true);
}
fn exercise_final_borrow(expired: bool) {
    let trace = setup();
    if expired {
        runtime::expire_observation();
    }
    let mut value = [0u8; 24];
    value[..2].copy_from_slice(&1u16.to_le_bytes());
    value[8] = 1;
    value[20..].copy_from_slice(&0x20000000u32.to_le_bytes());
    RETURNED.store(value.as_ptr() as u64, Ordering::Release);
    PAUSE_END.store(true, Ordering::Release);
    let reader = std::thread::spawn(call_property);
    let entered = wait_for(|| ENTERED.load(Ordering::Acquire));
    let closer = std::thread::spawn(|| runtime::BridgeRuntimeClose(null_mut()));
    let sealed = wait_for(runtime::fixture_invocation_closed);
    let was_waiting = !closer.is_finished();
    RELEASE.store(true, Ordering::Release);
    let result = reader.join().unwrap();
    let closed = closer.join().unwrap();
    assert!(entered && sealed && was_waiting);
    assert_eq!(result, value.as_ptr() as u64);
    assert_eq!(closed, BridgeStatus::Ok as u32);
    assert_eq!(CALLS.load(Ordering::Acquire), 1);
    assert_eq!(trace.next.load(Ordering::Acquire), u32::from(!expired));
    assert_eq!(trace.in_flight.load(Ordering::Acquire), 0);
}
#[test]
fn runtime_owned_os_closed_entry_is_not_imported_by_rebuild() {
    if !child("runtime_owned_os_closed_entry_is_not_imported_by_rebuild") {
        return;
    }
    let trace = setup();
    assert_eq!(
        runtime::BridgeRuntimeClose(null_mut()),
        BridgeStatus::Ok as u32
    );
    PAUSE_ORIGINAL.store(true, Ordering::Release);
    let reader = std::thread::spawn(call_property);
    let entered = wait_for(|| ENTERED.load(Ordering::Acquire));
    let rebuilt = runtime::BridgeRuntimeRebuild(null_mut());
    RELEASE.store(true, Ordering::Release);
    let result = reader.join().unwrap();
    let rows_before_fresh = trace.next.load(Ordering::Acquire);
    let fresh = call_property();
    assert!(entered);
    assert_eq!(rebuilt, BridgeStatus::Ok as u32);
    assert_eq!(result, 1);
    assert_eq!(
        rows_before_fresh, 0,
        "refused old call performs no post-Rebuild read/write"
    );
    assert_eq!(fresh, 1);
    assert_eq!(
        trace.next.load(Ordering::Acquire),
        1,
        "fresh generation still records its call"
    );
    assert_eq!(CALLS.load(Ordering::Acquire), 2);
    assert_eq!(
        runtime::BridgeRuntimeClose(null_mut()),
        BridgeStatus::Ok as u32
    );
}

unsafe extern "system" {
    fn calibration_gateway();
    fn root_unwind_driver(state: *mut u64, target: usize) -> u64;
    fn root_unwind_original();
}

#[test]
fn runtime_owned_os_close_timeout_keeps_admission_and_rebuild_closed() {
    if !child("runtime_owned_os_close_timeout_keeps_admission_and_rebuild_closed") {
        return;
    }
    let _trace = setup();
    PAUSE_ORIGINAL.store(true, Ordering::Release);
    let reader = std::thread::spawn(call_property);
    let entered = wait_for(|| ENTERED.load(Ordering::Acquire));
    let close = runtime::BridgeRuntimeClose(null_mut());
    let pending = runtime::fixture_pending_invocations();
    let rebuild = runtime::BridgeRuntimeRebuild(null_mut());
    RELEASE.store(true, Ordering::Release);
    let result = reader.join().unwrap();
    assert!(entered);
    assert_eq!(close, BridgeStatus::Critical as u32);
    assert_eq!(rebuild, BridgeStatus::Critical as u32);
    assert_eq!(pending, 1);
    assert_eq!(result, 1);
    assert_eq!(
        runtime::BridgeRuntimeClose(null_mut()),
        BridgeStatus::Ok as u32
    );
    assert_eq!(
        runtime::BridgeRuntimeRebuild(null_mut()),
        BridgeStatus::Ok as u32
    );
}

#[test]
fn runtime_owned_os_entry_panic_retains_lease_until_original_returns() {
    if !child("runtime_owned_os_entry_panic_retains_lease_until_original_returns") {
        return;
    }
    let property = setup();
    let mut assignment = assignment::AssignCalibrationTrace::boxed();
    assignment::ASSIGN_TRACE_PTR.store(assignment.as_mut(), Ordering::Release);
    BRIDGE_TRAMPOLINE_PTR.store(original as *const () as *mut u8, Ordering::Release);
    assignment::ASSIGN_TRAMPOLINE_PTR.store(original as *const () as *mut u8, Ordering::Release);
    let gateways = [
        calibration_gateway as *const () as usize,
        super::super::property_gateway as *const () as usize,
        assignment::assign_calibration_gateway as *const () as usize,
    ];
    for (index, target) in gateways.into_iter().enumerate() {
        RELEASE.store(false, Ordering::Release);
        ENTERED.store(false, Ordering::Release);
        PAUSE_ORIGINAL.store(true, Ordering::Release);
        ENTRY_PANIC.store(true, Ordering::Release);
        let reader = std::thread::spawn(move || {
            let call = unsafe {
                std::mem::transmute::<usize, unsafe extern "system" fn(u64, u64, u64, u64) -> u64>(
                    target,
                )
            };
            unsafe { call(0x1000, 0, 0, 1) }
        });
        let entered = wait_for(|| ENTERED.load(Ordering::Acquire));
        let held_during_original = runtime::fixture_pending_invocations();
        let closer = std::thread::spawn(|| runtime::BridgeRuntimeClose(null_mut()));
        let inactive = wait_for(runtime::fixture_product_inactive);
        let still_held = !closer.is_finished();
        RELEASE.store(true, Ordering::Release);
        let result = reader.join().unwrap();
        let closed = closer.join().unwrap();
        assert_eq!(
            held_during_original, 1,
            "entry failure retains the actual gateway's original call"
        );
        assert!(
            entered && inactive && still_held,
            "gateway {index} released before original completed"
        );
        assert_eq!(closed, BridgeStatus::Ok as u32);
        assert_eq!(result, if index == 1 { 1 } else { 0 });
        if index < 2 {
            assert_eq!(
                runtime::BridgeRuntimeRebuild(null_mut()),
                BridgeStatus::Ok as u32
            );
        }
    }
    assert_eq!(CALLS.load(Ordering::Acquire), 3);
    assert_eq!(property.next.load(Ordering::Acquire), 0);
    assert_eq!(assignment.in_flight.load(Ordering::Acquire), 0);
    assert_eq!(assignment.rows[0].sequence.load(Ordering::Acquire), 2);
    assert_eq!(
        runtime::BridgeRuntimeStopObservation(null_mut()),
        BridgeStatus::Ok as u32
    );
}
#[test]
fn runtime_owned_os_real_unwind_settles_all_three_native_gateways() {
    if !child("runtime_owned_os_real_unwind_settles_all_three_native_gateways") {
        return;
    }
    runtime::fixture_invocation_runtime();
    let mut trace = assignment::AssignCalibrationTrace::boxed();
    assignment::ASSIGN_TRACE_PTR.store(trace.as_mut(), Ordering::Release);
    let original = root_unwind_original as *const () as *mut u8;
    BRIDGE_TRAMPOLINE_PTR.store(original, Ordering::Release);
    PROPERTY_TRAMPOLINE_PTR.store(original, Ordering::Release);
    assignment::ASSIGN_TRAMPOLINE_PTR.store(original, Ordering::Release);
    let gateways = [
        super::super::calibration_gateway as *const () as usize,
        super::super::property_gateway as *const () as usize,
        assignment::assign_calibration_gateway as *const () as usize,
    ];
    for (index, target) in gateways.into_iter().enumerate() {
        let mut state = [0u64; 16];
        let landed = unsafe { root_unwind_driver(state.as_mut_ptr(), target) };
        assert_eq!(landed, 1, "real RtlUnwindEx crosses gateway {index}");
        assert_eq!(
            runtime::BridgeRuntimeClose(null_mut()),
            BridgeStatus::Ok as u32
        );
        if index < 2 {
            assert_eq!(
                runtime::BridgeRuntimeRebuild(null_mut()),
                BridgeStatus::Ok as u32
            );
        }
    }
    assert_eq!(trace.in_flight.load(Ordering::Acquire), 0);
    assert_eq!(
        trace.rows[0].sequence.load(Ordering::Acquire),
        2,
        "exception is preserved, not a successful native return"
    );
    assert_eq!(
        runtime::BridgeRuntimeStopObservation(null_mut()),
        BridgeStatus::Ok as u32
    );
}
