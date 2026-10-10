use super::*;
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
        assert_eq!(unsafe { f(12, 13, 14, 15) }, 12 ^ 13 ^ 14 ^ 15);
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
