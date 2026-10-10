use super::*;
use std::io::{BufRead, Write};
use std::os::windows::process::CommandExt;
use windows_sys::Win32::System::Diagnostics::Debug::{RtlLookupFunctionEntry, CONTEXT};
static CALLS: AtomicU32 = AtomicU32::new(0);
static RECURSE: AtomicU32 = AtomicU32::new(0);
static SELF_STOP: AtomicU32 = AtomicU32::new(0);
static PARK: AtomicU32 = AtomicU32::new(0);
static BARRIERS: std::sync::OnceLock<[std::sync::Barrier; 2]> = std::sync::OnceLock::new();
type Call = unsafe extern "system-unwind" fn(u64, u64, u64, u64) -> u64;
unsafe extern "system-unwind" fn original(a: u64, b: u64, c: u64, d: u64) -> u64 {
    CALLS.fetch_add(1, Ordering::SeqCst);
    if PARK.swap(0, Ordering::SeqCst) == 1 {
        BARRIERS.get().unwrap()[0].wait();
        BARRIERS.get().unwrap()[1].wait();
    }
    if RECURSE
        .compare_exchange(1, 0, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        let f: Call = unsafe { std::mem::transmute(gateway_address(1)) };
        assert_eq!(unsafe { f(12, 13, 14, 15) }, 0);
    }
    if SELF_STOP.swap(0, Ordering::SeqCst) == 1 {
        assert_eq!(
            Civ6OathLifecycleStop(),
            0,
            "reentrant stop must not wait for itself"
        );
    }
    a ^ b ^ c ^ d
}
unsafe extern "system-unwind" fn throwing(_: u64, _: u64, _: u64, _: u64) -> u64 {
    CALLS.fetch_add(1, Ordering::SeqCst);
    panic!("synthetic lifecycle native unwind");
}
unsafe extern "system-unwind" fn floating(a: f64, b: f64, c: f64, d: f64) -> f64 {
    CALLS.fetch_add(1, Ordering::SeqCst);
    a + b * 2.0 + c * 3.0 + d * 4.0
}
#[test]
fn child() {
    let Ok(case) = std::env::var("CC_LIFECYCLE_CASE") else {
        return;
    };
    let t = Box::leak(LifecycleTrace::boxed());
    let target = match case.as_str() {
        "unwind" => throwing as *const () as u64,
        "float" => floating as *const () as u64,
        _ => original as *const () as u64,
    };
    let slots = [
        Box::leak(Box::new(AtomicU64::new(target))),
        Box::leak(Box::new(AtomicU64::new(target))),
    ];
    for (i, s) in slots.iter().enumerate() {
        unsafe { install_at(i, *s as *const _ as usize, target, t).unwrap() };
    }
    assert_eq!(t.deadline_ms.load(Ordering::Acquire), 0);
    if case == "installed_poll" {
        assert_eq!(Civ6OathLifecyclePoll(), 0);
        assert_eq!(t.stopped.load(Ordering::Acquire), 1);
        assert_eq!(t.deadline_ms.load(Ordering::Acquire), 0);
        assert_eq!(t.expired.load(Ordering::Acquire), 0);
        assert_eq!(t.detached.load(Ordering::Acquire), 0);
        SECONDS.store(300, Ordering::Release);
        assert_eq!(Civ6OathLifecycleStart(), 0);
        assert_eq!(t.stopped.load(Ordering::Acquire), 0);
        assert!(t.deadline_ms.load(Ordering::Acquire) > now());
        assert_eq!(Civ6OathLifecycleStop(), 0);
        return;
    }
    t.start(300).unwrap();
    let f: Call = unsafe { std::mem::transmute(slots[0].load(Ordering::Acquire)) };
    if let Some(stage) = case
        .strip_prefix("stage")
        .and_then(|s| s.parse::<u32>().ok())
    {
        INJECT.store(stage, Ordering::Release);
        assert!(std::panic::catch_unwind(|| unsafe { f(7, 0, 91, 19) }).is_err());
        assert_eq!(CALLS.load(Ordering::Acquire), u32::from(stage == 4));
        assert_eq!(t.flight.load(Ordering::Acquire), 0);
        assert_eq!(DEPTH.with(Cell::get), 0);
        if stage > 1 {
            assert_eq!(t.rows[0].state.load(Ordering::Acquire), 3);
        }
        t.stop();
        unsafe { uninstall().unwrap() };
        return;
    }
    match case.as_str() {
        "float" => {
            let ff: unsafe extern "system-unwind" fn(f64, f64, f64, f64) -> f64 =
                unsafe { std::mem::transmute(slots[0].load(Ordering::Acquire)) };
            assert_eq!(unsafe { ff(1.25, 2.5, 3.75, 4.0) }, 33.5);
        }
        "unwind" => {
            assert!(std::panic::catch_unwind(|| unsafe { f(7, 0, 91, 19) }).is_err());
            assert_eq!(t.rows[0].state.load(Ordering::Acquire), 3);
            assert_eq!(t.unwound.load(Ordering::Acquire), 1);
            assert_eq!(DEPTH.with(Cell::get), 0);
        }
        "reentry" | "selfstop" => {
            if case == "selfstop" {
                SELF_STOP.store(1, Ordering::Release);
            }
            RECURSE.store(1, Ordering::Release);
            assert_eq!(unsafe { f(7, 0, 91, 19) }, 7 ^ 91 ^ 19);
            assert_eq!(t.rows[0].depth.load(Ordering::Acquire), 1);
            assert_eq!(t.rows[1].depth.load(Ordering::Acquire), 2);
            assert_eq!(t.rows[1].root_or_this.load(Ordering::Acquire), 12);
            assert_eq!(
                t.rows[0].thread.load(Ordering::Acquire),
                t.rows[1].thread.load(Ordering::Acquire)
            );
            assert_eq!(t.rows[1].completion_order.load(Ordering::Acquire), 1);
            if case == "selfstop" {
                assert_eq!(t.stopped.load(Ordering::Acquire), 2);
                assert_eq!(t.detached.load(Ordering::Acquire), 1);
            }
        }
        "threads" => {
            let barrier = std::sync::Barrier::new(3);
            std::thread::scope(|s| {
                let a = s.spawn(|| {
                    barrier.wait();
                    unsafe { f(7, 0, 91, 19) }
                });
                let b = s.spawn(|| {
                    barrier.wait();
                    unsafe { f(7, 0, 91, 19) }
                });
                barrier.wait();
                a.join().unwrap();
                b.join().unwrap();
            });
            assert_ne!(
                t.rows[0].thread.load(Ordering::Acquire),
                t.rows[1].thread.load(Ordering::Acquire)
            );
            assert_eq!(t.rows[0].depth.load(Ordering::Acquire), 1);
            assert_eq!(t.rows[1].depth.load(Ordering::Acquire), 1);
        }
        "overflow" => {
            for _ in 0..=CAPACITY {
                assert_eq!(unsafe { f(7, 0, 91, 19) }, 7 ^ 91 ^ 19);
            }
            assert_eq!(t.next.load(Ordering::Acquire), CAPACITY as u32);
            assert_eq!(t.dropped.load(Ordering::Acquire), 1);
        }
        "deadline" => {
            t.deadline_ms.store(1, Ordering::Release);
            assert_eq!(unsafe { f(7, 0, 91, 19) }, 7 ^ 91 ^ 19);
            assert_eq!(t.expired.load(Ordering::Acquire), 1);
            assert_eq!(t.next.load(Ordering::Acquire), 0);
        }
        "park" => {
            BARRIERS
                .set([std::sync::Barrier::new(2), std::sync::Barrier::new(2)])
                .unwrap();
            PARK.store(1, Ordering::Release);
            let h = std::thread::spawn(move || unsafe { f(7, 0, 91, 19) });
            BARRIERS.get().unwrap()[0].wait();
            assert_eq!(t.flight.load(Ordering::Acquire), 1);
            assert_eq!(t.rows[0].state.load(Ordering::Acquire), 1);
            // Stop returns while an original is parked; no self/drain wait.
            t.stop();
            unsafe { uninstall().unwrap() };
            assert_eq!(slots[0].load(Ordering::Acquire), target);
            assert_eq!(t.flight.load(Ordering::Acquire), 1);
            BARRIERS.get().unwrap()[1].wait();
            assert_eq!(h.join().unwrap(), 7 ^ 91 ^ 19);
        }
        _ => {
            assert_eq!(unsafe { f(7, 0, 91, 19) }, 7 ^ 91 ^ 19);
            assert_eq!(t.rows[0].root_or_this.load(Ordering::Acquire), 0);
            assert_eq!(t.rows[0].rcx.load(Ordering::Acquire), 7);
            assert_ne!(t.rows[0].caller.load(Ordering::Acquire), 0);
        }
    }
    let before = CALLS.load(Ordering::Acquire);
    assert_eq!(
        before,
        match case.as_str() {
            "overflow" => CAPACITY as u32 + 1,
            "threads" | "reentry" | "selfstop" => 2,
            _ => 1,
        }
    );
    assert_eq!(t.flight.load(Ordering::Acquire), 0);
    let rows = t.next.load(Ordering::Acquire);
    t.stop();
    unsafe { uninstall().unwrap() };
    assert_eq!(slots[0].load(Ordering::Acquire), target);
    assert_eq!(slots[1].load(Ordering::Acquire), target);
    // A function pointer fetched before restoration still owns resident code/data.
    if case != "float" && case != "unwind" {
        assert_eq!(unsafe { f(7, 0, 91, 19) }, 7 ^ 91 ^ 19);
        assert_eq!(CALLS.load(Ordering::Acquire), before + 1);
    }
    assert_eq!(t.next.load(Ordering::Acquire), rows);
    assert_eq!(t.flight.load(Ordering::Acquire), 0);
    assert!(t.start(300).is_err());
    assert!(unsafe { install_at(0, slots[0] as *const _ as usize, target, t) }.is_err());
    if rows > 0 && case != "unwind" {
        assert_eq!(t.rows[0].state.load(Ordering::Acquire), 2);
    }
}
#[test]
fn synthetic_win64_cases() {
    for case in [
        "installed_poll",
        "normal",
        "float",
        "reentry",
        "selfstop",
        "threads",
        "overflow",
        "unwind",
        "deadline",
        "park",
        "stage1",
        "stage2",
        "stage3",
        "stage4",
    ] {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "lifecycle_trace::tests::child", "--nocapture"])
            .env("CC_LIFECYCLE_CASE", case)
            .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{case}: {} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
unsafe extern "system" {
    fn RtlVirtualUnwind(
        kind: u32,
        base: u64,
        pc: u64,
        entry: *const c_void,
        context: *mut CONTEXT,
        data: *mut *mut c_void,
        frame: *mut u64,
        pointers: *mut c_void,
    ) -> *mut c_void;
}
#[test]
fn native_unwind_restores_caller_and_nonvolatile_registers() {
    for gateway in [
        lifecycle_root_gateway as *const () as u64,
        lifecycle_teardown_gateway as *const () as u64,
    ] {
        let mut base = 0;
        let entry = unsafe { RtlLookupFunctionEntry(gateway, &mut base, null_mut()) };
        assert!(!entry.is_null());
        let codes = unsafe {
            crate::native::copy_current::<4>(base + u64::from((*entry).Anonymous.UnwindData))
        }
        .unwrap();
        assert_eq!(codes[0] >> 3, 2, "UNW_FLAG_UHANDLER");
        let mut stack = [0u64; 40];
        stack[31] = 0x123456789abcdef0;
        let mut c: CONTEXT = unsafe { std::mem::zeroed() };
        c.Rip = gateway + u64::from(codes[1]);
        c.Rsp = stack.as_ptr() as u64;
        c.Rbx = 0x1122;
        c.Rbp = 0x3344;
        c.Rdi = 0x5566;
        let mut data = null_mut();
        let mut frame = 0;
        unsafe {
            RtlVirtualUnwind(
                0,
                base,
                c.Rip,
                entry.cast(),
                &mut c,
                &mut data,
                &mut frame,
                null_mut(),
            )
        };
        assert_eq!(c.Rip, stack[31]);
        assert_eq!(c.Rsp, stack.as_ptr() as u64 + 0x100);
        assert_eq!((c.Rbx, c.Rbp, c.Rdi), (0x1122, 0x3344, 0x5566));
    }
    assert_eq!(LifecycleTrace::wire_layout(), (48, 64, 32816));
}
#[test]
fn public_installer_cannot_skip_process_and_image_gate() {
    assert_eq!(
        Civ6OathLifecycleInstall(null_mut()),
        OathStatus::InvalidInput as u32
    );
    let mut c = OathRuntimeConfig {
        version: crate::runtime::OATH_RUNTIME_VERSION,
        size: 48,
        game_pid: unsafe { windows_sys::Win32::System::Threading::GetCurrentProcessId() },
        max_seconds: 300,
        created_filetime: crate::runtime::current_created_filetime().unwrap(),
        mode: crate::runtime::OATH_MODE_DIAGNOSTIC,
        ..OathRuntimeConfig::default()
    };
    let before = c;
    assert_eq!(
        Civ6OathLifecycleInstall((&mut c as *mut OathRuntimeConfig).cast()),
        OathStatus::ProcessIdentity as u32
    );
    assert_eq!(c, before, "test executable is not the fixed game EXE");
    c.flags = 1;
    let before = c;
    assert_eq!(
        Civ6OathLifecycleInstall((&mut c as *mut OathRuntimeConfig).cast()),
        OathStatus::InvalidInput as u32
    );
    assert_eq!(c, before);
}

fn shared_config(handle: u64) -> SharedLifecycleConfig {
    SharedLifecycleConfig {
        config: OathRuntimeConfig {
            version: crate::runtime::OATH_RUNTIME_VERSION,
            size: 48,
            game_pid: unsafe { windows_sys::Win32::System::Threading::GetCurrentProcessId() },
            max_seconds: 300,
            created_filetime: crate::runtime::current_created_filetime().unwrap(),
            mode: crate::runtime::OATH_MODE_DIAGNOSTIC,
            ..OathRuntimeConfig::default()
        },
        mapping_handle: handle,
        mapping_size: MAPPING_SIZE,
        reserved: 0,
    }
}
fn shared_export(c: &mut SharedLifecycleConfig) -> u32 {
    Civ6OathLifecycleInstallShared((c as *mut SharedLifecycleConfig).cast())
}
fn read32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn read64(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}
#[test]
fn shared_initialization_and_config_gate() {
    use crate::lifecycle_transport::SharedTrace;
    fn send<T: Send>() {}
    send::<SharedTrace>();
    let trace = SharedTrace::new().unwrap();
    let mut expected = vec![0u8; 32816];
    expected[..4].copy_from_slice(&0x4c43_3134u32.to_le_bytes());
    expected[4..8].copy_from_slice(&1u32.to_le_bytes());
    expected[28..32].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(
        trace.snapshot(),
        expected,
        "all 512 rows are initialized once"
    );
    let duplicate = trace
        .duplicate_into(unsafe { GetCurrentProcess() })
        .unwrap();
    let mut c = shared_config(duplicate);
    let fresh = c;
    assert_eq!(std::mem::size_of_val(&c), 64);
    assert_eq!(
        std::mem::offset_of!(SharedLifecycleConfig, mapping_size),
        56
    );
    assert_eq!(shared_export(&mut c), OathStatus::ProcessIdentity as u32);
    assert_eq!(c, fresh);
    assert_eq!(
        trace.snapshot(),
        expected,
        "identity rejection must not mutate storage"
    );
    assert!(TRACE.load(Ordering::Acquire).is_null());
    assert!(!*CONTROL.lock().unwrap());
    assert!(BINDINGS.lock().unwrap().iter().all(Option::is_none));
    assert_eq!(
        Civ6OathLifecycleInstallShared(null_mut()),
        OathStatus::InvalidInput as u32
    );
    for bad in 0..16 {
        c = fresh;
        match bad {
            0 => c.config.version ^= 1,
            1 => c.config.size = 64,
            2 => c.config.game_pid = 0,
            3 => c.config.created_filetime = 0,
            4 => c.config.max_seconds = 0,
            5 => c.config.max_seconds = 301,
            6 => c.config.mode = 0,
            7 => c.config.flags = 1,
            8 => c.config.out_trace_address = 1,
            9 => c.config.trace_size = MAPPING_SIZE,
            10 => c.config.reserved = 1,
            11 => c.mapping_handle = 0,
            12 => c.mapping_handle = u64::MAX,
            13 => c.mapping_size -= 1,
            14 => c.mapping_size += 1,
            15 => c.reserved = 1,
            _ => unreachable!(),
        }
        let before = c;
        assert_eq!(
            shared_export(&mut c),
            OathStatus::InvalidInput as u32,
            "case {bad}"
        );
        assert_eq!(c, before);
        assert_eq!(trace.snapshot(), expected);
    }
    c = fresh;
    c.config.created_filetime ^= 1;
    assert_eq!(shared_export(&mut c), OathStatus::ProcessIdentity as u32);
    assert!(TRACE.load(Ordering::Acquire).is_null());
    assert!(!*CONTROL.lock().unwrap());
    // Only this test's same-process duplicate may be closed here.
    unsafe { windows_sys::Win32::Foundation::CloseHandle(duplicate as _) };
    assert!(trace.duplicate_into(null_mut()).is_err());
}

#[test]
fn shared_rejects_uninitialized_dirty_or_wrong_extent_mapping() {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Memory::{CreateFileMappingW, PAGE_READWRITE};
    for size in [MAPPING_SIZE, 4096, MAPPING_SIZE + 8192] {
        let handle = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                ptr::null(),
                PAGE_READWRITE,
                0,
                size,
                ptr::null(),
            )
        };
        assert!(!handle.is_null());
        let mut c = shared_config(handle as u64);
        let before = c;
        assert_eq!(shared_export(&mut c), OathStatus::InvalidInput as u32);
        assert_eq!(c, before);
        unsafe { CloseHandle(handle) };
    }
    let parent = crate::lifecycle_transport::SharedTrace::new().unwrap();
    let duplicate = parent
        .duplicate_into(unsafe { GetCurrentProcess() })
        .unwrap();
    let view = unsafe { MappedTrace::map(duplicate as _, MAPPING_SIZE).unwrap() };
    // An already used header or row is rejected, never reset by the receiver.
    for field in [&view.trace().next, &view.trace().rows[511].state] {
        field.store(1, Ordering::Release);
        let before = parent.snapshot();
        let mut c = shared_config(duplicate);
        assert_eq!(shared_export(&mut c), OathStatus::InvalidInput as u32);
        assert_eq!(parent.snapshot(), before);
        field.store(0, Ordering::Release);
    }
    unsafe { CloseHandle(duplicate as _) };
}

/// Our test executable is the only child, not a game process or a product caller.
/// Publication deliberately uses owned synthetic slots after the real public
/// gate has separately been proved to reject this executable.
#[test]
fn shared_child() {
    let Ok(case) = std::env::var("CC_SHARED_LIFECYCLE_CASE") else {
        return;
    };
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).unwrap();
    let handle = line.trim().parse::<u64>().unwrap();
    let view = unsafe { MappedTrace::map(handle as _, MAPPING_SIZE).unwrap() };
    assert!(view.trace().initialized());
    let t = view.retain();
    let targets = [original as *const () as u64, throwing as *const () as u64];
    let slots = targets.map(|target| Box::leak(Box::new(AtomicU64::new(target))));
    for (i, slot) in slots.iter().enumerate() {
        unsafe { install_at(i, *slot as *const _ as usize, targets[i], t).unwrap() };
    }
    t.start(300).unwrap();
    let f: Call = unsafe { std::mem::transmute(slots[0].load(Ordering::Acquire)) };
    assert_eq!(unsafe { f(7, 0, 91, 19) }, 7 ^ 91 ^ 19);
    if case == "unwind" {
        let throwing: Call = unsafe { std::mem::transmute(slots[1].load(Ordering::Acquire)) };
        assert!(std::panic::catch_unwind(|| unsafe { throwing(12, 13, 14, 15) }).is_err());
        assert_eq!(CALLS.load(Ordering::Acquire), 2);
        assert_eq!(t.rows[1].state.load(Ordering::Acquire), 3);
        assert_eq!(t.flight.load(Ordering::Acquire), 0);
    }
    let parked = if case == "entered" {
        BARRIERS
            .set([std::sync::Barrier::new(2), std::sync::Barrier::new(2)])
            .unwrap();
        PARK.store(1, Ordering::Release);
        let worker = std::thread::spawn(move || unsafe { f(12, 13, 14, 15) });
        BARRIERS.get().unwrap()[0].wait();
        assert_eq!(CALLS.load(Ordering::Acquire), 2);
        assert_eq!(t.flight.load(Ordering::Acquire), 1);
        assert_eq!(t.rows[1].state.load(Ordering::Acquire), 1);
        Some(worker)
    } else {
        None
    };
    // Serial libtest prints `test name ... ` without a newline before the
    // test body. Keep this exact protocol marker on its own line in both
    // cargo test and nextest's child environment.
    println!("\nLC_PUBLISHED");
    std::io::stdout().flush().unwrap();
    line.clear();
    std::io::stdin().read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "exit");
    if parked.is_some() {
        // End the OS process while the original is still parked. No final poll,
        // stop, Rust Drop or synthetic settlement can create a terminal row.
        std::process::exit(0);
    }
    assert_eq!(
        CALLS.load(Ordering::Acquire),
        if case == "unwind" { 2 } else { 1 }
    );
    // Normal exit intentionally performs no lifecycle Stop/Poll either.
}

struct OwnedChild(std::process::Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
#[test]
fn shared_helper_process_publication_survives_os_exit() {
    use crate::lifecycle_transport::SharedTrace;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, CREATE_NO_WINDOW, PROCESS_DUP_HANDLE,
    };
    for case in ["normal", "unwind", "entered"] {
        let parent = SharedTrace::new().unwrap();
        let initial = parent.snapshot();
        let mut child = OwnedChild(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "lifecycle_trace::tests::shared_child",
                    "--nocapture",
                ])
                .env("CC_SHARED_LIFECYCLE_CASE", case)
                .env("RUST_TEST_THREADS", "1")
                .creation_flags(CREATE_NO_WINDOW)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        let process = unsafe { OpenProcess(PROCESS_DUP_HANDLE, 0, child.0.id()) };
        assert!(!process.is_null());
        let duplicated = parent.duplicate_into(process);
        unsafe { CloseHandle(process) };
        let mut input = child.0.stdin.take().unwrap();
        writeln!(input, "{}", duplicated.unwrap()).unwrap();
        input.flush().unwrap();
        let output = child.0.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in std::io::BufReader::new(output).lines() {
                if line.unwrap() == "LC_PUBLISHED" {
                    tx.send(()).unwrap();
                }
            }
        });
        let published = rx.recv_timeout(Duration::from_secs(15));
        if published.is_err() {
            child.0.kill().unwrap();
            child.0.wait().unwrap();
            reader.join().unwrap();
            panic!("{case}: child did not publish: {published:?}");
        }
        let live = parent.snapshot();
        assert_ne!(live, initial);
        assert_eq!(read32(&live, 8), if case == "normal" { 1 } else { 2 });
        assert_eq!(read32(&live, 12), 0);
        assert_eq!(read32(&live, 16), u32::from(case == "entered"));
        assert_eq!(read32(&live, 20), 1);
        assert_eq!(read32(&live, 24), u32::from(case == "unwind"));
        assert_eq!(read32(&live, 28), 0);
        assert_ne!(read64(&live, 32), 0);
        assert_eq!(read32(&live, 40), 0);
        assert_eq!(read32(&live, 44), 0, "no final poll/detach fabricated");
        assert_eq!(read32(&live, 48), 2);
        assert_eq!(read64(&live, 48 + 24), 7);
        assert_eq!(read64(&live, 48 + 32), 0);
        assert_ne!(read64(&live, 48 + 40), 0);
        assert_eq!(read64(&live, 48 + 48), 7 ^ 91 ^ 19);
        assert_eq!(read32(&live, 48 + 56), 1);
        if case != "normal" {
            assert_eq!(read32(&live, 112), if case == "unwind" { 3 } else { 1 });
            assert_eq!(read32(&live, 112 + 56), 0);
            assert_eq!(read64(&live, 112 + 48), 0);
        }
        writeln!(input, "exit").unwrap();
        drop(input);
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            // Parent loads safely while the child still owns a writable view.
            assert_eq!(parent.snapshot(), live);
            if let Some(status) = child.0.try_wait().unwrap() {
                assert!(status.success(), "{case}: {status}");
                break;
            }
            assert!(Instant::now() < deadline, "{case}: child exit timed out");
            std::thread::yield_now();
        }
        reader.join().unwrap();
        assert_eq!(
            parent.snapshot(),
            live,
            "{case}: parent retains rows after OS exit"
        );
    }
}
