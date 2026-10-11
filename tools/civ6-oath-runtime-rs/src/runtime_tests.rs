//! Runtime entry tests: pure lifecycle and input rules, own-process refusals
//! through the real exports, and Win64 flows in owned child processes with a
//! synthetic gate. The real gate must reject this test executable.
use super::*;
use crate::oath_record::COPY_ALL;
use std::os::windows::process::CommandExt;
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Memory::{
    MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
};

unsafe extern "system" {
    /// Synthetic original defined by the trace tests (same test binary).
    fn oath_original_body();
}

const SUPPORTED: u64 = 0xa7_6a30;
const UNSUPPORTED: u64 = 0x1234;
/// The module the synthetic gate "proves": this very test executable.
fn base() -> u64 {
    unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(null_mut()) as u64 }
}

fn good_config() -> OathRuntimeConfig {
    OathRuntimeConfig {
        version: OATH_RUNTIME_VERSION,
        size: 48,
        game_pid: unsafe { GetCurrentProcessId() },
        max_seconds: 30,
        created_filetime: current_created_filetime().unwrap(),
        mode: OATH_MODE_DIAGNOSTIC,
        flags: 0,
        out_trace_address: 0,
        trace_size: 0,
        reserved: 0,
    }
}

fn install_export(config: &mut OathRuntimeConfig) -> u32 {
    OathRuntimeInstall((config as *mut OathRuntimeConfig).cast())
}

fn receipt() -> OathReceipt {
    let mut out = OathReceipt::default();
    assert_eq!(
        OathRuntimeReadReceipt((&mut out as *mut OathReceipt).cast()),
        OathStatus::Ok as u32
    );
    out
}

#[test]
fn the_config_and_receipt_blocks_have_the_published_size() {
    assert_eq!(std::mem::size_of::<OathRuntimeConfig>(), 48);
    assert_eq!(std::mem::size_of::<OathReceipt>(), 64);
}

#[test]
fn validation_rejects_every_field_that_is_not_exactly_the_diagnostic_request() {
    assert_eq!(validate(&good_config()), Ok(()));
    let bad: [fn(&mut OathRuntimeConfig); 12] = [
        |c| c.version ^= 1,
        |c| c.size = 40,
        |c| c.game_pid = 0,
        |c| c.created_filetime = 0,
        |c| c.max_seconds = 0,
        |c| c.max_seconds = OATH_MAX_ACTIVE_SECONDS + 1,
        |c| c.mode = 0,
        |c| c.mode = 2,
        |c| c.flags = 2,
        |c| c.out_trace_address = 1,
        |c| c.trace_size = 1,
        |c| c.reserved = 1,
    ];
    for (n, mutate) in bad.iter().enumerate() {
        let mut config = good_config();
        mutate(&mut config);
        assert_eq!(validate(&config), Err(OathStatus::InvalidInput), "case {n}");
    }
    let mut callee = good_config();
    callee.flags = OATH_CAPTURE_RECIPIENT;
    assert_eq!(validate(&callee), Ok(()));
}

#[test]
fn lifecycle_is_one_shot_over_every_operation_order() {
    #[derive(Clone, Copy)]
    enum Op {
        Installed,
        Failed,
        Start,
        Stop,
    }
    let ops = [Op::Installed, Op::Failed, Op::Start, Op::Stop];
    // All sequences up to length 6.
    let mut sequences = vec![vec![]];
    for _ in 0..6 {
        let mut next = vec![];
        for sequence in &sequences {
            for op in ops {
                let mut longer = sequence.clone();
                longer.push(op);
                next.push(longer);
            }
        }
        sequences.extend(next);
    }
    for sequence in sequences {
        let mut lifecycle = Lifecycle::new();
        let (mut installs, mut starts, mut stopped_seen) = (0, 0, false);
        for op in sequence {
            let before = lifecycle.phase();
            let result = match op {
                Op::Installed => lifecycle.installed(),
                Op::Failed => lifecycle.failed(),
                Op::Start => lifecycle.start(),
                Op::Stop => lifecycle.stop(),
            };
            if result.is_ok() {
                match op {
                    Op::Installed => installs += 1,
                    Op::Start => starts += 1,
                    _ => {}
                }
            }
            assert!(installs <= 1 && starts <= 1);
            if before == Phase::Stopped || before == Phase::Failed {
                // Terminal phases never leave, except that Stopped may stop again.
                assert!(matches!(lifecycle.phase(), Phase::Stopped | Phase::Failed));
                assert!(!matches!(op, Op::Start | Op::Installed) || result.is_err());
            }
            if lifecycle.phase() == Phase::Stopped {
                stopped_seen = true;
            }
            if stopped_seen {
                assert_ne!(lifecycle.phase(), Phase::Started);
                assert!(!lifecycle.can_install());
            }
        }
    }
}

/// The real exports refuse everything that is not this very process, and a
/// DLL without the wired preflight can never install. Nothing changes.
#[test]
fn the_exports_refuse_a_foreign_or_unverified_request_without_writing() {
    let before = receipt();
    assert_eq!(before.phase, Phase::Fresh as u32);
    assert_eq!(before.trace_address, 0);
    let mut cases: Vec<(OathRuntimeConfig, OathStatus)> = vec![];
    let mut config = good_config();
    config.game_pid += 1;
    cases.push((config, OathStatus::ProcessIdentity));
    let mut config = good_config();
    config.created_filetime ^= 0x100;
    cases.push((config, OathStatus::ProcessIdentity));
    let mut config = good_config();
    config.mode = 7;
    cases.push((config, OathStatus::InvalidInput));
    let mut config = good_config();
    config.out_trace_address = 0x1000;
    cases.push((config, OathStatus::InvalidInput));
    // Right PID and creation time, but this is a test executable, not Civ6.
    cases.push((good_config(), OathStatus::ProcessIdentity));
    for (mut config, expected) in cases {
        let copy = config;
        assert_eq!(install_export(&mut config), expected as u32);
        assert_eq!(config, copy, "the caller's block is untouched");
    }
    assert_eq!(
        OathRuntimeInstall(null_mut()),
        OathStatus::InvalidInput as u32
    );
    let mut raw = [0u8; 64];
    let misaligned = unsafe { raw.as_mut_ptr().add(1) };
    assert_eq!(
        OathRuntimeInstall(misaligned.cast()),
        OathStatus::InvalidInput as u32
    );
    assert_eq!(
        OathRuntimeReadReceipt(null_mut()),
        OathStatus::InvalidInput as u32
    );
    assert_eq!(
        OathRuntimeStartObservation(null_mut()),
        OathStatus::WrongState as u32
    );
    assert_eq!(
        OathRuntimeStopObservation(null_mut()),
        OathStatus::WrongState as u32
    );
    assert_eq!(receipt(), before, "no state, no trace, no pin changed");
}

// ---- Win64 flows (owned child processes) -----------------------------------

fn run_child(name: &str) {
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("JAC_OATH_RUNTIME_CHILD", "1")
        .creation_flags(0x0800_0000)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "owned runtime child {name} failed: {status}"
            );
            return;
        }
        assert!(
            Instant::now() < deadline,
            "owned runtime child {name} timed out"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn in_child() -> bool {
    std::env::var("JAC_OATH_RUNTIME_CHILD").as_deref() == Ok("1")
}

#[test]
fn flows_run_in_owned_children() {
    for name in [
        "runtime::tests::flow_child",
        "runtime::tests::gate_refusal_child",
        "runtime::tests::half_install_child",
        "runtime::tests::wrong_module_child",
        "runtime::tests::deadline_child",
        "runtime::tests::recipient_runtime_child",
        "runtime::tests::recipient_half_install_child",
        "runtime::tests::recipient_restore_failure_child",
        "runtime::tests::recipient_deadline_child",
    ] {
        run_child(name);
    }
}

struct TestGate(Result<Verified, OathStatus>);
impl Gate for TestGate {
    fn verify(&self, config: &OathRuntimeConfig) -> Result<Verified, OathStatus> {
        own_process(config)?;
        self.0
    }
}

/// Execute/read page with two wrappers (head, padding, far jump to a
/// synthetic original) like the real ones.
fn synthetic_page() -> *mut u8 {
    let page = unsafe {
        VirtualAlloc(
            null_mut(),
            CODE_PAGE,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        )
    }
    .cast::<u8>();
    assert!(!page.is_null());
    unsafe {
        ptr::write_bytes(page, 0xcc, CODE_PAGE);
        for offset in [0x20usize, 0x80] {
            let entry = page.add(offset);
            ptr::copy_nonoverlapping(
                [0x40u8, 0x53, 0x48, 0x83, 0xec, 0x30, 0x0f, 0x10, 0x05].as_ptr(),
                entry,
                9,
            );
            let disp = (0x200 - (offset as i32 + 13)).to_le_bytes();
            ptr::copy_nonoverlapping(disp.as_ptr(), entry.add(9), 4);
            ptr::copy_nonoverlapping([0xffu8, 0x25, 0, 0, 0, 0].as_ptr(), entry.add(13), 6);
            ptr::copy_nonoverlapping(
                (oath_original_body as *const () as usize as u64)
                    .to_le_bytes()
                    .as_ptr(),
                entry.add(19),
                8,
            );
        }
        let mut old = 0;
        VirtualProtect(page.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old);
    }
    page
}

fn page_bytes(page: *mut u8) -> Vec<u8> {
    unsafe { std::slice::from_raw_parts(page, CODE_PAGE) }.to_vec()
}

fn verified(page: *mut u8) -> Verified {
    Verified {
        image_base: base() as usize,
        core_module: synthetic_page as *const () as usize,
        apply_entry: unsafe { page.add(0x20) } as usize,
        remove_entry: unsafe { page.add(0x80) } as usize,
        recipient_slots: None,
    }
}

#[repr(C, align(16))]
struct Block([u64; 64]);

fn call(entry: usize, vtable_rva: u64) -> u64 {
    let mut block = Block([0; 64]);
    block.0[0] = base() + vtable_rva;
    block.0[31] = 0x66;
    let mut object = [0u64; 2];
    let subject = [object.as_mut_ptr() as u64, 1];
    let function: extern "system" fn(u64, u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(entry) };
    let result = function(block.0.as_mut_ptr() as u64, subject.as_ptr() as u64, 7, 0);
    assert_eq!(block.0[1], 1, "the original ran exactly once");
    result
}

#[test]
fn flow_child() {
    if !in_child() {
        return;
    }
    let page = synthetic_page();
    let gate = TestGate(Ok(verified(page)));
    let mut config = good_config();
    assert_eq!(
        install_with(&gate, (&mut config as *mut OathRuntimeConfig).cast()),
        OathStatus::Ok
    );
    assert_ne!(config.out_trace_address, 0);
    assert_eq!(config.trace_size as usize, std::mem::size_of::<OathTrace>());
    let trace = unsafe { &*(config.out_trace_address as *const OathTrace) };
    assert_eq!(receipt().phase, Phase::Installed as u32);
    // Installed but not started: calls run the original and record nothing,
    // not even a filtered count.
    let (apply, remove) = (verified(page).apply_entry, verified(page).remove_entry);
    assert_eq!(call(apply, SUPPORTED), 0x66);
    assert_eq!((trace.counters(), trace.filtered()), ((0, 0, 0), 0));
    assert_eq!(start_observation(), OathStatus::Ok);
    assert_eq!(
        start_observation(),
        OathStatus::WrongState,
        "start is one-shot"
    );
    assert_eq!(call(apply, SUPPORTED), 0x66);
    assert_eq!(call(remove, UNSUPPORTED), 0x66);
    let rows = trace.records().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].call, rows[0].effect_vtable),
        (OathCall::Apply, base() + SUPPORTED)
    );
    assert_eq!(rows[0].copy_flags & COPY_ALL, rows[0].copy_flags);
    let live = receipt();
    assert_eq!(
        (
            live.phase,
            live.rows,
            live.completed,
            live.filtered,
            live.dropped,
            live.in_flight
        ),
        (Phase::Started as u32, 1, 1, 1, 0, 0)
    );
    assert_eq!(live.quantity_accepted, 0);
    assert_eq!(live.trace_address, config.out_trace_address);
    // Stop is final: nothing more is recorded, start and install are refused.
    assert_eq!(stop_observation(), OathStatus::Ok);
    assert_eq!(stop_observation(), OathStatus::Ok);
    assert_eq!(call(apply, SUPPORTED), 0x66);
    assert_eq!(call(remove, UNSUPPORTED), 0x66);
    assert_eq!(start_observation(), OathStatus::WrongState);
    let mut again = good_config();
    assert_eq!(
        install_with(&gate, (&mut again as *mut OathRuntimeConfig).cast()),
        OathStatus::WrongState
    );
    let done = receipt();
    assert_eq!(
        (done.phase, done.rows, done.filtered),
        (Phase::Stopped as u32, 1, 1)
    );
    assert_eq!(page_bytes(page).len(), CODE_PAGE);
}

#[test]
fn recipient_runtime_child() {
    if !in_child() {
        return;
    }
    unsafe extern "system-unwind" fn original(effect: u64, city: u64, r8: u64) -> u64 {
        effect ^ city ^ r8
    }
    let target = original as *const () as u64;
    let slots = Box::leak(Box::new(std::array::from_fn::<AtomicU64, 8, _>(|_| {
        AtomicU64::new(target)
    })));
    let mut verified = verified(synthetic_page());
    verified.recipient_slots = Some(std::array::from_fn(|index| {
        (&slots[index] as *const _ as usize, target)
    }));
    let gate = TestGate(Ok(verified));
    let mut config = good_config();
    config.flags = OATH_CAPTURE_RECIPIENT;
    assert_eq!(
        install_with(&gate, (&mut config as *mut OathRuntimeConfig).cast()),
        OathStatus::Ok
    );
    assert_eq!(
        config.trace_size as usize,
        std::mem::size_of::<crate::recipient_trace::RecipientTrace>()
    );
    assert_eq!(start_observation(), OathStatus::Ok);
    for slot in slots.iter() {
        let function: unsafe extern "system-unwind" fn(u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(slot.load(Ordering::Acquire)) };
        assert_eq!(unsafe { function(7, 0x12340000, 91) }, 7 ^ 0x12340000 ^ 91);
    }
    assert_eq!(stop_observation(), OathStatus::Ok);
    let done = receipt();
    assert_eq!(
        (
            done.flags,
            done.rows,
            done.completed,
            done.dropped,
            done.in_flight,
            done.quantity_accepted
        ),
        (1, 8, 8, 0, 0, 0)
    );
    let trace =
        unsafe { &*(config.out_trace_address as *const crate::recipient_trace::RecipientTrace) };
    assert!(trace.detached());
    for slot in slots.iter() {
        assert_eq!(slot.load(Ordering::Acquire), target);
    }
    assert_eq!(start_observation(), OathStatus::WrongState);
    assert_eq!(stop_observation(), OathStatus::Ok);
    let mut config2 = good_config();
    config2.flags = OATH_CAPTURE_RECIPIENT;
    assert_eq!(
        install_with(&gate, (&mut config2 as *mut OathRuntimeConfig).cast()),
        OathStatus::WrongState
    );
}

#[test]
fn gate_refusal_child() {
    if !in_child() {
        return;
    }
    let page = synthetic_page();
    let snapshot = page_bytes(page);
    let mut config = good_config();
    for status in [
        OathStatus::ImageIdentity,
        OathStatus::EntryBytes,
        OathStatus::ProcessIdentity,
    ] {
        let gate = TestGate(Err(status));
        let before = config;
        assert_eq!(
            install_with(&gate, (&mut config as *mut OathRuntimeConfig).cast()),
            status
        );
        assert_eq!(config, before);
        assert_eq!(
            page_bytes(page),
            snapshot,
            "a refusing gate leaves every byte"
        );
        assert_eq!(receipt().phase, Phase::Fresh as u32);
        assert_eq!(receipt().trace_address, 0);
    }
    // A foreign pid is refused by the own-process check before the gate's verdict.
    let mut foreign = good_config();
    foreign.game_pid += 1;
    let gate = TestGate(Ok(verified(page)));
    assert_eq!(
        install_with(&gate, (&mut foreign as *mut OathRuntimeConfig).cast()),
        OathStatus::ProcessIdentity
    );
    assert_eq!(page_bytes(page), snapshot);
    // Refusals consumed nothing: a verified request still installs.
    assert_eq!(
        install_with(&gate, (&mut config as *mut OathRuntimeConfig).cast()),
        OathStatus::Ok
    );
}

fn recipient_fixture() -> (&'static [AtomicU64; 8], u64, TestGate, OathRuntimeConfig) {
    unsafe extern "system-unwind" fn original(a: u64, b: u64, c: u64) -> u64 {
        a ^ b ^ c
    }
    let target = original as *const () as u64;
    let slots = Box::leak(Box::new(std::array::from_fn(|_| AtomicU64::new(target))));
    let mut proof = verified(synthetic_page());
    proof.recipient_slots = Some(std::array::from_fn(|i| {
        (&slots[i] as *const _ as usize, target)
    }));
    let mut config = good_config();
    config.flags = OATH_CAPTURE_RECIPIENT;
    (slots, target, TestGate(Ok(proof)), config)
}

#[test]
fn recipient_half_install_child() {
    if !in_child() {
        return;
    }
    let (slots, target, mut gate, mut config) = recipient_fixture();
    gate.0.as_mut().unwrap().recipient_slots.as_mut().unwrap()[4].1 ^= 8;
    let before = config;
    assert_eq!(
        install_with(&gate, (&mut config as *mut OathRuntimeConfig).cast()),
        OathStatus::HookFailure
    );
    assert_eq!(config, before);
    assert!(slots.iter().all(|s| s.load(Ordering::Acquire) == target));
    assert_eq!(receipt().phase, Phase::Failed as u32);
    assert_eq!(start_observation(), OathStatus::WrongState);
}

#[test]
fn recipient_restore_failure_child() {
    if !in_child() {
        return;
    }
    let (slots, target, gate, mut config) = recipient_fixture();
    assert_eq!(
        install_with(&gate, (&mut config as *mut OathRuntimeConfig).cast()),
        OathStatus::Ok
    );
    assert_eq!(start_observation(), OathStatus::Ok);
    let foreign = target ^ 8;
    slots[3].store(foreign, Ordering::Release);
    assert_eq!(stop_observation(), OathStatus::Critical);
    assert_eq!(
        slots[3].load(Ordering::Acquire),
        foreign,
        "foreign writer is preserved"
    );
    assert_eq!(read_receipt(), Err(OathStatus::Critical));
    assert_eq!(start_observation(), OathStatus::Critical);
    let trace =
        unsafe { &*(config.out_trace_address as *const crate::recipient_trace::RecipientTrace) };
    assert!(!trace.detached());
    slots[3].store(target, Ordering::Release);
    assert_eq!(stop_observation(), OathStatus::Ok);
    assert!(trace.detached());
    assert_eq!(receipt().phase, Phase::Stopped as u32);
}

#[test]
fn recipient_deadline_child() {
    if !in_child() {
        return;
    }
    let (slots, target, gate, mut config) = recipient_fixture();
    config.max_seconds = 1;
    crate::recipient_trace::CLOCK_OVERRIDE.store(10_000, Ordering::Release);
    assert_eq!(
        install_with(&gate, (&mut config as *mut OathRuntimeConfig).cast()),
        OathStatus::Ok
    );
    assert_eq!(start_observation(), OathStatus::Ok);
    // Keep a fetched entrance across stop: its original and trace remain valid.
    let fetched: unsafe extern "system-unwind" fn(u64, u64, u64) -> u64 =
        unsafe { std::mem::transmute(slots[0].load(Ordering::Acquire)) };
    assert_eq!(unsafe { fetched(7, 9, 11) }, 7 ^ 9 ^ 11);
    crate::recipient_trace::CLOCK_OVERRIDE.store(11_000, Ordering::Release);
    assert_eq!(unsafe { fetched(7, 9, 11) }, 7 ^ 9 ^ 11);
    let stopped = receipt();
    assert_eq!(
        (
            stopped.phase,
            stopped.expired,
            stopped.rows,
            stopped.completed
        ),
        (3, 1, 1, 1)
    );
    assert!(slots.iter().all(|s| s.load(Ordering::Acquire) == target));
    assert_eq!(unsafe { fetched(7, 9, 11) }, 7 ^ 9 ^ 11);
    assert_eq!(receipt(), stopped);
    assert_eq!(start_observation(), OathStatus::WrongState);
}

#[test]
fn half_install_child() {
    if !in_child() {
        return;
    }
    let page = synthetic_page();
    let mut broken = verified(page);
    broken.remove_entry = unsafe { page.add(0x100) } as usize; // no wrapper head there
    let head = unsafe { ptr::read(page.add(0x20).cast::<[u8; 8]>()) };
    let gate = TestGate(Ok(broken));
    let mut config = good_config();
    let before = config;
    let status = install_with(&gate, (&mut config as *mut OathRuntimeConfig).cast());
    assert_eq!(status, OathStatus::HookFailure);
    assert_eq!(config, before, "no receipt for a failed install");
    assert_eq!(
        unsafe { ptr::read(page.add(0x20).cast::<[u8; 8]>()) },
        head,
        "the Apply entry that was published is restored"
    );
    assert_eq!(receipt().phase, Phase::Failed as u32);
    assert_eq!(start_observation(), OathStatus::WrongState);
    let gate = TestGate(Ok(verified(page)));
    assert_eq!(
        install_with(&gate, (&mut config as *mut OathRuntimeConfig).cast()),
        OathStatus::WrongState
    );
}

#[test]
fn deadline_child() {
    if !in_child() {
        return;
    }
    let page = synthetic_page();
    let gate = TestGate(Ok(verified(page)));
    let mut config = good_config();
    config.max_seconds = 1;
    assert_eq!(
        install_with(&gate, (&mut config as *mut OathRuntimeConfig).cast()),
        OathStatus::Ok
    );
    let trace = unsafe { &*(config.out_trace_address as *const OathTrace) };
    let apply = verified(page).apply_entry;
    crate::oath_trace::CLOCK_OVERRIDE.store(10_000, Ordering::SeqCst);
    assert_eq!(start_observation(), OathStatus::Ok);
    assert_eq!(call(apply, SUPPORTED), 0x66);
    // One tick before the deadline the call is still recorded.
    crate::oath_trace::CLOCK_OVERRIDE.store(10_999, Ordering::SeqCst);
    assert_eq!(call(apply, SUPPORTED), 0x66);
    assert_eq!(trace.counters().0, 2);
    // At the deadline the begin callback stops the run by itself: no reader
    // has called anything, the original still runs exactly once (call() asserts
    // it), and no row is added.
    crate::oath_trace::CLOCK_OVERRIDE.store(11_000, Ordering::SeqCst);
    assert_eq!(call(apply, SUPPORTED), 0x66);
    assert_eq!(call(verified(page).remove_entry, SUPPORTED), 0x66);
    assert_eq!(trace.counters(), (2, 2, 0));
    assert!(trace.is_stopped());
    assert_eq!(trace.in_flight(), 0);
    let late = receipt();
    assert_eq!(
        (late.phase, late.expired, late.rows),
        (Phase::Stopped as u32, 1, 2)
    );
    assert_eq!(start_observation(), OathStatus::WrongState);
}

#[test]
fn wrong_module_child() {
    if !in_child() {
        return;
    }
    let page = synthetic_page();
    let snapshot = page_bytes(page);
    let mut wrong = verified(page);
    wrong.image_base += 0x10000; // the pinned module is not the one the gate named
    let gate = TestGate(Ok(wrong));
    let mut config = good_config();
    let before = config;
    assert_eq!(
        install_with(&gate, (&mut config as *mut OathRuntimeConfig).cast()),
        OathStatus::ImageIdentity
    );
    assert_eq!(config, before);
    assert_eq!(page_bytes(page), snapshot, "nothing was written");
    assert_eq!(receipt().phase, Phase::Fresh as u32);
    assert_eq!(receipt().trace_address, 0);
    // The refusal consumed nothing: the right module still installs.
    let gate = TestGate(Ok(verified(page)));
    assert_eq!(
        install_with(&gate, (&mut config as *mut OathRuntimeConfig).cast()),
        OathStatus::Ok
    );
}
