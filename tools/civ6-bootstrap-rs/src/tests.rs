use super::*;
use std::{
    os::windows::process::CommandExt,
    process::{Child, Command},
    time::{Duration, Instant},
};

type CaptureReadyFixture = fn(usize, bool) -> Result<(), String>;
thread_local! {
    pub(super) static CORE_MODULE_OVERRIDE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    pub(super) static CAPTURE_READY_OVERRIDE: std::cell::Cell<Option<CaptureReadyFixture>> = const {std::cell::Cell::new(None)};
    static CAPTURE_FIXTURE_FAULT:std::cell::Cell<u32>=const {std::cell::Cell::new(0)};
}

// Only game identity/hash/loader publication dependencies are substituted.
// Exact public selector, config identity, API/data protection, size and pointer
// readiness validation run through production code and actual owned OS pages.
fn capture_fixture_ready(base: usize, submission: bool) -> Result<(), String> {
    let fault = CAPTURE_FIXTURE_FAULT.with(std::cell::Cell::get);
    capture_ready_symbols(base, submission, |name| {
        let install = if submission {
            b"BridgeSubmissionObservationInstall\0".as_slice()
        } else {
            b"BridgeRootObservationInstall\0".as_slice()
        };
        let stop = if submission {
            b"BridgeSubmissionObservationStop\0".as_slice()
        } else {
            b"BridgeRootObservationStop\0".as_slice()
        };
        let pointer = if submission {
            b"BRIDGE_SUBMISSION_OBSERVATION_TRACE\0".as_slice()
        } else {
            b"BRIDGE_ROOT_OBSERVATION_TRACE\0".as_slice()
        };
        let size = if submission {
            b"BRIDGE_SUBMISSION_OBSERVATION_TRACE_SIZE\0".as_slice()
        } else {
            b"BRIDGE_ROOT_OBSERVATION_TRACE_SIZE\0".as_slice()
        };
        if name == install {
            Ok(base + if fault == 2 { 0x100 } else { 0x1000 })
        } else if name == stop {
            if fault == 3 {
                Err("runtime export absent".into())
            } else {
                Ok(base + 0x1010)
            }
        } else if name == pointer {
            Ok(base + if fault == 4 { 0x1000 } else { 0x100 })
        } else if name == size {
            Ok(base + 0x108)
        } else {
            Err("runtime export absent".into())
        }
    })
    .map(|_| ())
}
fn capture_fixture_child(name: &str) -> bool {
    const ENV: &str = "CIV6_BOOTSTRAP_PREPARE_OWNED_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    struct Owned(Child);
    impl Drop for Owned {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
            }
            let _ = self.0.wait();
        }
    }
    let mut c = Owned(
        Command::new(std::env::current_exe().unwrap())
            .creation_flags(0x08000000)
            .env(ENV, "1")
            .args(["--exact", &format!("windows::tests::{name}"), "--nocapture"])
            .spawn()
            .unwrap(),
    );
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = c.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}
struct CaptureImage {
    base: usize,
}
impl CaptureImage {
    fn new(submission: bool) -> Self {
        let base = unsafe {
            VirtualAlloc(
                std::ptr::null(),
                0x20000,
                MEM_RESERVE | MEM_COMMIT,
                PAGE_READWRITE,
            )
        } as usize;
        assert_ne!(base, 0);
        unsafe {
            std::ptr::write((base + 0x100) as *mut u64, (base + 0x2000) as u64);
            std::ptr::write(
                (base + 0x108) as *mut u64,
                if submission {
                    protocol::SUBMISSION_BYTES as u64
                } else {
                    protocol::ROOT_BYTES as u64
                },
            );
            // Any accidental original/runtime API call is a hard failure.
            std::ptr::write((base + 0x1000) as *mut u8, 0xcc);
            std::ptr::write((base + 0x1010) as *mut u8, 0xcc);
        }
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect((base + 0x1000) as _, 0x1000, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        CAPTURE_READY_OVERRIDE.with(|v| v.set(Some(capture_fixture_ready)));
        Self { base }
    }
    fn input(&self, submission: bool) -> Prepare {
        Prepare {
            version: if submission {
                protocol::SUBMISSION_PREPARE_VERSION
            } else {
                protocol::ROOT_PREPARE_VERSION
            },
            size: size_of::<Prepare>() as u32,
            pid: unsafe { GetCurrentProcessId() },
            observation_seconds: 30,
            created_filetime: created(),
            bridge_module: self.base as u64,
            bridge_sha256: [0; 32],
            expected_menu_context: 1,
        }
    }
}
impl Drop for CaptureImage {
    fn drop(&mut self) {
        CAPTURE_READY_OVERRIDE.with(|v| v.set(None));
        unsafe {
            VirtualFree(self.base as _, 0, MEM_RELEASE);
        }
    }
}
fn prepare_diagnostic_words() -> Vec<u64> {
    PREPARE_DIAGNOSTIC
        .words
        .iter()
        .map(|v| v.load(Ordering::Acquire))
        .collect()
}
#[test]
fn fresh_submission_prepare_selector_checks_current_v4_exports_without_installing() {
    if !capture_fixture_child(
        "fresh_submission_prepare_selector_checks_current_v4_exports_without_installing",
    ) {
        return;
    }
    let image = CaptureImage::new(true);
    let input = image.input(true);
    assert_eq!(protocol::SUBMISSION_ROWS, 12 * 16);
    assert_eq!(protocol::ROOT_ROWS, 128);
    assert_eq!(protocol::SUBMISSION_BYTES, 83392);
    assert_eq!(RustBridgeBootstrapPrepareSubmissionObservation(&input), 0);
    let d = prepare_diagnostic_words();
    assert_eq!(&d[..5], &[1, 64, 2, 19, 0]);
    assert_eq!(d[5], u64::from(input.pid));
    assert_eq!(d[6], input.created_filetime);
    assert_eq!(d[7], u64::from(input.version));
    assert_eq!(
        read(image.base + 0x2000, protocol::SUBMISSION_BYTES).unwrap(),
        vec![0; protocol::SUBMISSION_BYTES]
    );
    assert!(STATE.lock().unwrap().is_none());
    assert_eq!(TRACE.reserved.load(Ordering::Acquire), 0);
}
#[test]
fn fresh_root_prepare_selector_keeps_original_root_size_without_installing() {
    if !capture_fixture_child(
        "fresh_root_prepare_selector_keeps_original_root_size_without_installing",
    ) {
        return;
    }
    let image = CaptureImage::new(false);
    let input = image.input(false);
    assert_eq!(RustBridgeBootstrapPrepareRootObservation(&input), 0);
    assert_eq!(&prepare_diagnostic_words()[..5], &[1, 64, 2, 19, 0]);
    assert!(STATE.lock().unwrap().is_none());
}
#[test]
fn fresh_submission_prepare_rejects_legacy_size_and_preserves_first_cause() {
    if !capture_fixture_child(
        "fresh_submission_prepare_rejects_legacy_size_and_preserves_first_cause",
    ) {
        return;
    }
    let image = CaptureImage::new(true);
    let input = image.input(true);
    unsafe {
        std::ptr::write(
            (image.base + 0x108) as *mut u64,
            protocol::SUBMISSION_V3_BYTES as u64,
        );
    }
    assert_eq!(RustBridgeBootstrapPrepareSubmissionObservation(&input), 1);
    assert_eq!(&prepare_diagnostic_words()[..5], &[1, 64, 3, 10, 7]);
    let first = prepare_diagnostic_words();
    assert_eq!(
        RustBridgeBootstrapPrepareSubmissionObservation(std::ptr::null()),
        1
    );
    assert_eq!(prepare_diagnostic_words(), first);
    assert!(STATE.lock().unwrap().is_none());
}
#[test]
fn fresh_submission_prepare_rejects_export_types_missing_symbols_and_pointer_bounds() {
    if !capture_fixture_child(
        "fresh_submission_prepare_rejects_export_types_missing_symbols_and_pointer_bounds",
    ) {
        return;
    }
    let image = CaptureImage::new(true);
    let input = image.input(true);
    for (fault, reason) in [(2, 5), (3, 4), (4, 6), (5, 8)] {
        // Separate synthetic diagnostic attempts; production retains first cause.
        for v in PREPARE_DIAGNOSTIC.words.iter().skip(2) {
            v.store(0, Ordering::Release);
        }
        CAPTURE_FIXTURE_FAULT.with(|v| v.set(fault));
        if fault == 5 {
            unsafe {
                std::ptr::write(
                    (image.base + 0x100) as *mut u64,
                    (image.base + 0x1fff8) as u64,
                );
            }
        }
        assert_eq!(RustBridgeBootstrapPrepareSubmissionObservation(&input), 1);
        assert_eq!(&prepare_diagnostic_words()[..5], &[1, 64, 3, 10, reason]);
        assert!(STATE.lock().unwrap().is_none());
    }
}
#[test]
fn fresh_submission_prepare_wrong_selector_records_config_refusal_before_dependencies() {
    if !capture_fixture_child(
        "fresh_submission_prepare_wrong_selector_records_config_refusal_before_dependencies",
    ) {
        return;
    }
    let image = CaptureImage::new(true);
    let mut input = image.input(true);
    input.version = protocol::ROOT_PREPARE_VERSION;
    CAPTURE_FIXTURE_FAULT.with(|v| v.set(3));
    assert_eq!(RustBridgeBootstrapPrepareSubmissionObservation(&input), 1);
    assert_eq!(&prepare_diagnostic_words()[..5], &[1, 64, 3, 1, 1]);
    assert!(STATE.lock().unwrap().is_none());
}

#[test]
fn fresh_normal_prepare_wrong_selector_preserves_numeric_refusal_before_dependencies() {
    if !capture_fixture_child(
        "fresh_normal_prepare_wrong_selector_preserves_numeric_refusal_before_dependencies",
    ) {
        return;
    }
    let image = CaptureImage::new(false);
    let input = image.input(false); // Root version is not accepted by ordinary runtime.
    assert_ne!(input.version, VERSION);
    assert_eq!(RustBridgeBootstrapPrepare(&input), 1);
    let words = prepare_diagnostic_words();
    assert_eq!(&words[..5], &[1, 64, 3, 1, 1]);
    assert_eq!(words[5], u64::from(input.pid));
    assert_eq!(words[6], input.created_filetime);
    assert_eq!(words[7], u64::from(input.version));
    assert!(STATE.lock().unwrap().is_none());
}

#[test]
fn real_loader_return_rejects_overlap_and_crossed_generation() {
    const CHILD: &str = "JAC_BOOTSTRAP_LOADER_OVERLAP_CHILD";
    if std::env::var_os(CHILD).is_none() {
        struct OwnedChild(Child);
        impl Drop for OwnedChild {
            fn drop(&mut self) {
                if self.0.try_wait().ok().flatten().is_none() {
                    let _ = self.0.kill();
                }
                let _ = self.0.wait();
            }
        }
        let mut child = OwnedChild(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "windows::tests::real_loader_return_rejects_overlap_and_crossed_generation",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .creation_flags(0x0800_0000)
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < deadline, "owned loader child deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
        return;
    }

    static API_CALLS: AtomicU64 = AtomicU64::new(0);
    unsafe extern "system" fn counted_api(_: *mut c_void) -> u32 {
        API_CALLS.fetch_add(1, Ordering::Relaxed);
        0
    }
    // Local process-owned memory for the actual E8 decoder/ReadProcessMemory.
    // The module override supplies only presence; any identity/path validation
    // would fail. Neither the real Core nor a fixture DLL is loaded or executed.
    let mut image = vec![0u8; 0x24b532];
    image[0x24b52d] = 0xe8;
    image[0x24b52e..0x24b532].copy_from_slice(&(LOADER as i32 - 0x24b532i32).to_le_bytes());
    let exe = image.as_ptr() as usize;
    let ret = exe + 0x24b532;
    *STATE.lock().unwrap() = Some(Resident {
        exe,
        core: 1,
        install: counted_api,
        stop: counted_api,
        close: counted_api,
        rebuild: counted_api,
        phase: Phase::Closing,
        generation: 7,
        close_tid: unsafe { GetCurrentThreadId() },
        config: RuntimeConfig {
            version: 0,
            size: 0,
            pid: 0,
            max_seconds: 0,
            created_filetime: 0,
            trace_address: 0,
            trace_size: 0,
            mode: 0,
        },
        _source: File::open(std::env::current_exe().unwrap()).unwrap(),
        _hooks: Vec::new(),
    });
    CORE_MODULE_OVERRIDE.with(|value| value.set(Some(1)));
    let closing = loader_begin(ret).unwrap();
    assert_eq!(closing.caller, Some(0x24b52d));
    let error = target_loader_return(1, ret, closing).unwrap_err();
    assert!(error.contains("previous context transition completed"));
    assert_eq!(API_CALLS.load(Ordering::Acquire), 0);
    assert_eq!(
        STATE.lock().unwrap().as_ref().unwrap().phase,
        Phase::Closing
    );

    STATE.lock().unwrap().as_mut().unwrap().phase = Phase::Open;
    let open = loader_begin(ret).unwrap();
    // Positive control uses the same production entry and return handlers.
    target_loader_return(1, ret, open).unwrap();
    STATE.lock().unwrap().as_mut().unwrap().generation = 8;
    assert!(target_loader_return(1, ret, open)
        .unwrap_err()
        .contains("crossed a context transition"));
    target_loader_return(0x100, ret, open).unwrap(); // Original AL=false.
    let current = loader_begin(ret).unwrap();
    let other_thread = LoaderInvocation { tid: 0, ..current };
    assert!(target_loader_return(1, ret, other_thread).is_err());
    STATE.lock().unwrap().as_mut().unwrap().phase = Phase::Closed;
    assert!(target_loader_return(1, ret, current).is_err());
    assert_eq!(API_CALLS.load(Ordering::Acquire), 0);
    *STATE.lock().unwrap() = None;
    CORE_MODULE_OVERRIDE.with(|value| value.set(None));
}

// No other test in this process admits bootstrap trace records. The native
// gateway fixtures run in their own child process and never use this trace.
#[test]
fn closed_bootstrap_observation_rejects_late_records_without_count_underflow() {
    let flight = RecordFlight::try_enter().expect("initial recording lease");
    OBSERVATION.close();
    assert!(
        !OBSERVATION.drained(),
        "accepted write must still be drained"
    );
    drop(flight);
    assert!(OBSERVATION.drained());
    let before = TRACE.reserved.load(Ordering::Acquire);
    for _ in 0..128 {
        assert!(RecordFlight::try_enter().is_none());
        record(1, 0, 0, 0, 0, 0, 0);
        assert!(OBSERVATION.drained(), "rejected lease must not decrement");
    }
    assert_eq!(TRACE.reserved.load(Ordering::Acquire), before);
    assert_eq!(TRACE.dropped.load(Ordering::Acquire), 0);
}

#[unsafe(no_mangle)]
static BOOT_FIXTURE_ORIGINAL_CALLS: AtomicU64 = AtomicU64::new(0);
#[unsafe(no_mangle)]
static BOOT_FIXTURE_SET_CALLS: AtomicU64 = AtomicU64::new(0);
#[unsafe(no_mangle)]
static BOOT_FIXTURE_LOADER_CALLS: AtomicU64 = AtomicU64::new(0);
core::arch::global_asm!(
    ".text",
    ".p2align 4",
    ".rept 16",
    "int3",
    ".endr",
    ".globl boot_fixture",
    ".seh_proc boot_fixture",
    "boot_fixture:",
    ".byte 0x40,0x53",
    ".seh_pushreg rbx",
    "sub rsp,32",
    ".seh_stackalloc 32",
    ".seh_endprologue",
    "lock inc qword ptr [rip + BOOT_FIXTURE_ORIGINAL_CALLS]",
    "test rdx,rdx",
    "jz 2f",
    "call boot_fixture_unwind",
    "2:",
    "lea rax,[rcx+7]",
    "add rsp,32",
    "pop rbx",
    "ret",
    ".seh_endproc",
    ".p2align 4",
    ".rept 16",
    "int3",
    ".endr",
    ".globl boot_loader_fixture",
    ".seh_proc boot_loader_fixture",
    "boot_loader_fixture:",
    "sub rsp,0x248",
    ".seh_stackalloc 0x248",
    ".seh_endprologue",
    "lock inc qword ptr [rip + BOOT_FIXTURE_LOADER_CALLS]",
    "test rdx,rdx",
    "jz 3f",
    "call boot_fixture_unwind",
    "3:",
    "lea rax,[rcx+17]",
    "add rsp,0x248",
    "ret",
    ".seh_endproc",
    ".p2align 4",
    ".rept 16",
    "int3",
    ".endr",
    ".globl boot_set_fixture",
    ".seh_proc boot_set_fixture",
    "boot_set_fixture:",
    "mov [rsp+8],rbx",
    ".seh_savereg rbx,8",
    "push rdi",
    ".seh_pushreg rdi",
    "sub rsp,32",
    ".seh_stackalloc 32",
    ".seh_endprologue",
    "lock inc qword ptr [rip + BOOT_FIXTURE_SET_CALLS]",
    "lea rax,[rcx+rdx+9]",
    "mov rbx,[rsp+48]",
    "add rsp,32",
    "pop rdi",
    "ret",
    ".seh_endproc",
);
extern "system-unwind" {
    fn boot_fixture(a: usize, b: usize) -> usize;
    fn boot_set_fixture(a: usize, b: usize) -> usize;
    fn boot_loader_fixture(a: usize, b: usize) -> usize;
}
extern "system-unwind" fn loader_fixture_wrapper(a: usize, b: usize, ret: usize) -> usize {
    assert_ne!(ret, 0);
    let result = unsafe { boot_loader_original(a, b) };
    assert!(BOOT_FIXTURE_LOADER_CALLS.load(Ordering::Acquire) > 0);
    result
}
extern "system-unwind" fn set_fixture_wrapper(a: usize, b: usize, ret: usize) -> usize {
    assert_ne!(ret, 0);
    (unsafe { boot_set_original(a, b) }) + 1
}
#[unsafe(no_mangle)]
extern "system-unwind" fn boot_fixture_unwind() {
    panic!("owned original unwind probe");
}
static WRAPPER_CALLS: AtomicU64 = AtomicU64::new(0);
extern "system-unwind" fn fixture_wrapper(a: usize, b: usize, ret: usize) -> usize {
    assert_ne!(ret, 0);
    WRAPPER_CALLS.fetch_add(1, Ordering::Relaxed);
    (unsafe { boot_release_original(a, b) }) + 1
}

#[test]
fn invalid_prepare_uses_actual_os_identity_and_never_installs() {
    assert_eq!(RustBridgeBootstrapPrepare(std::ptr::null()), 1);
    let input = Prepare {
        version: VERSION,
        size: size_of::<Prepare>() as u32,
        pid: unsafe { GetCurrentProcessId() },
        observation_seconds: 30,
        created_filetime: created(),
        bridge_module: 0,
        bridge_sha256: [0; 32],
        expected_menu_context: 0,
    };
    assert_eq!(RustBridgeBootstrapPrepare(&input), 1); // real EXE is test binary
    assert!(STATE.lock().unwrap().is_none());
    assert_eq!(TRACE.reserved.load(Ordering::Acquire), 0);
}

#[test]
#[ignore = "explicit owned OS child; parent test below always executes it"]
fn owned_hotpatch_child() {
    let entry = boot_fixture as *const () as usize;
    assert_eq!(unsafe { boot_fixture(3, 0) }, 10);
    assert_eq!(BOOT_FIXTURE_ORIGINAL_CALLS.load(Ordering::Acquire), 1);
    let bytes = read(entry, 6).unwrap();
    assert!(hotpatch::prepare(entry, &[0; 6], fixture_wrapper as *const () as usize).is_err());
    assert_eq!(read(entry, 6).unwrap(), bytes);
    BOOT_RELEASE_CONTINUE.store(entry + 2, Ordering::Release);
    let hook = hotpatch::prepare(
        entry,
        &[0x40, 0x53, 0x48, 0x83, 0xec, 0x20],
        fixture_wrapper as *const () as usize,
    )
    .unwrap();
    hook.publish().unwrap();
    assert_eq!(unsafe { boot_fixture(3, 0) }, 11);
    assert_eq!(BOOT_FIXTURE_ORIGINAL_CALLS.load(Ordering::Acquire), 2);
    assert_eq!(WRAPPER_CALLS.load(Ordering::Acquire), 1);
    assert!(std::panic::catch_unwind(|| unsafe { boot_fixture(3, 1) }).is_err());
    assert_eq!(BOOT_FIXTURE_ORIGINAL_CALLS.load(Ordering::Acquire), 3);
    assert_eq!(WRAPPER_CALLS.load(Ordering::Acquire), 2);
    hook.restore().unwrap();
    assert_eq!(read(entry, 6).unwrap(), bytes);
    assert_eq!(unsafe { boot_fixture(3, 0) }, 10);
    assert_eq!(BOOT_FIXTURE_ORIGINAL_CALLS.load(Ordering::Acquire), 4);
    let entry = boot_set_fixture as *const () as usize;
    BOOT_SET_CONTINUE.store(entry + 5, Ordering::Release);
    let bytes = read(entry, 10).unwrap();
    let hook = hotpatch::prepare(
        entry,
        &[0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83, 0xec, 0x20],
        set_fixture_wrapper as *const () as usize,
    )
    .unwrap();
    hook.publish().unwrap();
    assert_eq!(unsafe { boot_set_fixture(3, 4) }, 17);
    assert_eq!(BOOT_FIXTURE_SET_CALLS.load(Ordering::Acquire), 1);
    hook.restore().unwrap();
    assert_eq!(read(entry, 10).unwrap(), bytes);
    assert_eq!(unsafe { boot_set_fixture(3, 4) }, 16);
    for address in [
        boot_release_original as *const () as u64,
        boot_set_original as *const () as u64,
        boot_loader_original as *const () as u64,
    ] {
        let mut base = 0;
        assert!(!unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry(
                address,
                &mut base,
                std::ptr::null_mut(),
            )
        }
        .is_null());
    }
    let entry = boot_loader_fixture as *const () as usize;
    BOOT_LOADER_CONTINUE.store(entry + 7, Ordering::Release);
    let original = read(entry, 7).unwrap();
    let hook = hotpatch::prepare(
        entry,
        &[0x48, 0x81, 0xec, 0x48, 0x02, 0, 0],
        loader_fixture_wrapper as *const () as usize,
    )
    .unwrap();
    hook.publish().unwrap();
    assert_eq!(unsafe { boot_loader_fixture(3, 0) }, 20);
    assert_eq!(unsafe { boot_loader_fixture(239, 0) }, 256); // AL=0 stays AL=0.
    assert!(std::panic::catch_unwind(|| unsafe { boot_loader_fixture(3, 1) }).is_err());
    assert_eq!(BOOT_FIXTURE_LOADER_CALLS.load(Ordering::Acquire), 3);
    hook.restore().unwrap();
    assert_eq!(read(entry, 7).unwrap(), original);
    println!("owned_hotpatch_original_once=true entry_restored=true unreachable_prefix_retained_until_exit=true runtime_api_calls=0 game_live=false");
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
#[test]
fn actual_owned_hotpatch_original_once_and_reap() {
    let mut child = OwnedChild(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "windows::tests::owned_hotpatch_child",
                "--ignored",
                "--nocapture",
            ])
            .creation_flags(0x08000000)
            .spawn()
            .unwrap(),
    );
    let pid = child.0.id();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "owned child deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(child);
    println!("owned_child_pid={pid} reaped=true runtime_api_calls=0 game_live=false");
}

#[test]
fn native_counts_read_runtime_log_and_never_report_an_empty_prefix() {
    use protocol::*;
    let put = |b: &mut [u8], o: usize, n: u32| b[o..o + 4].copy_from_slice(&n.to_le_bytes());
    let mut composite = vec![0u8; COMPOSITE_PROPERTY + 32];
    put(&mut composite, 0, 0x4a414341);
    put(&mut composite, 4, CALIBRATION_VERSION);
    put(&mut composite, 8, 7);
    put(&mut composite, COMPOSITE_PROPERTY, 0x4a414350);
    put(&mut composite, COMPOSITE_PROPERTY + 4, 3);
    let mut log = vec![0u8; PROPERTY_LOG_HEADER];
    put(&mut log, 0, PROPERTY_LOG_MAGIC);
    put(&mut log, 4, PROPERTY_LOG_VERSION);
    log[PROPERTY_LOG_NEXT..PROPERTY_LOG_NEXT + 8].copy_from_slice(&5000u64.to_le_bytes());
    let export = log.as_ptr() as u64;
    LOG_EXPORT.store(std::ptr::addr_of!(export) as usize, Ordering::Release);
    TRACE
        .runtime_trace_size
        .store(composite.len() as u64, Ordering::Release);
    TRACE
        .runtime_trace
        .store(composite.as_ptr() as u64, Ordering::Release);
    // A bounded prefix with next=0 is not evidence of zero runtime calls.
    assert_eq!(native_counts(), [u64::MAX; 4]);
    put(
        &mut composite,
        COMPOSITE_PROPERTY + 4,
        PROPERTY_EXTERNAL_VERSION,
    );
    put(&mut composite, COMPOSITE_PROPERTY + 28, PROPERTY_LOG_MAGIC);
    assert_eq!(native_counts(), [7, 0, 5000, 0]);
    put(&mut log, 0, 0);
    assert_eq!(native_counts(), [u64::MAX; 4]);
    TRACE.runtime_trace.store(0, Ordering::Release);
    TRACE.runtime_trace_size.store(0, Ordering::Release);
    LOG_EXPORT.store(0, Ordering::Release);
}
