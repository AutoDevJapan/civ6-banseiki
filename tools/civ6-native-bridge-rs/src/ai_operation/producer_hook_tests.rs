use super::super::tests::{
    ai_fixture_driver, ai_fixture_return, ai_fixture_target, ai_unwind_driver, ai_unwind_return,
    child, original_unwinds,
};
use super::*;

fn callers() -> [u64; 2] {
    [
        ai_fixture_return as *const () as u64,
        ai_unwind_return as *const () as u64,
    ]
}

#[test]
fn zero_generation_is_rejected_before_any_executable_patch() {
    if !child(
        "ai_operation::gateway::producer_hook::tests::zero_generation_is_rejected_before_any_executable_patch",
        "JAC_ROOT_ZERO_GENERATION_CHILD",
    ) {
        return;
    }
    let entry = unsafe { image(ai_fixture_target as *const () as usize) };
    let guard = INSTALL.lock().unwrap();
    assert_eq!(
        unsafe { prepare(entry as u64, 0, &guard) },
        Err(Failure::Lease)
    );
    assert!(HOOK.get().is_none());
    assert_eq!(Memory.copy::<8>(entry as u64 - 8), Some([0xcc; 8]));
    assert_eq!(Memory.copy::<5>(entry as u64), Some(HEAD));
    assert_eq!(AI_GATE_TARGETS[4].load(Ordering::Acquire), 0);
    let mut output = [0u8; 0x160];
    unsafe { ai_fixture_driver(output.as_mut_ptr(), entry) };
    assert_eq!(u64::from_le_bytes(output[88..96].try_into().unwrap()), 1);
}
unsafe fn prepare(
    entry: u64,
    generation: u32,
    guard: &std::sync::MutexGuard<'_, ()>,
) -> Result<(), Failure> {
    unsafe { prepare_set(&[(entry, HEAD, callers())], generation, guard) }
}

pub(super) unsafe fn image(target: usize) -> usize {
    unsafe { image_head(target, HEAD) }
}

pub(super) fn call_fixture(entry: usize) -> u64 {
    let mut output = [0u8; 0x160];
    unsafe { ai_fixture_driver(output.as_mut_ptr(), entry) };
    u64::from_le_bytes(output[0x100..0x108].try_into().unwrap())
}
unsafe fn image_head(target: usize, head: [u8; 5]) -> usize {
    let page = unsafe { VirtualAlloc(ptr::null(), 4096, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
        as *mut u8;
    assert!(!page.is_null());
    unsafe {
        ptr::write_bytes(page, 0xcc, 4096);
        let entry = page.add(0x100);
        ptr::copy_nonoverlapping(head.as_ptr(), entry, 5);
        ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), entry.add(5), 6);
        ptr::copy_nonoverlapping((target as u64).to_le_bytes().as_ptr(), entry.add(11), 8);
    }
    let mut old = 0;
    assert_ne!(
        unsafe { VirtualProtect(page.cast(), 4096, PAGE_EXECUTE_READ, &mut old) },
        0
    );
    assert_ne!(
        unsafe { FlushInstructionCache(GetCurrentProcess(), page.cast(), 4096) },
        0
    );
    page as usize + 0x100
}

#[unsafe(no_mangle)]
static PRODUCER_INNER_TARGET: AtomicU64 = AtomicU64::new(0);
#[unsafe(no_mangle)]
static PRODUCER_HOLD_AFTER_INNER: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
static PRODUCER_AFTER_INNER: AtomicU32 = AtomicU32::new(0);
core::arch::global_asm!(
    ".globl producer_outer_fixture",
    ".seh_proc producer_outer_fixture",
    "producer_outer_fixture:",
    "sub rsp,0x68",
    ".seh_stackalloc 0x68",
    ".seh_endprologue",
    "call qword ptr [rip+PRODUCER_INNER_TARGET]",
    ".globl producer_inner_return",
    "producer_inner_return:",
    "mov dword ptr [rip+PRODUCER_AFTER_INNER],1",
    "2:",
    "cmp dword ptr [rip+PRODUCER_HOLD_AFTER_INNER],0",
    "je 3f",
    "pause",
    "jmp 2b",
    "3:",
    "add rsp,0x68",
    "ret",
    ".seh_endproc"
);
unsafe extern "system" {
    fn producer_outer_fixture();
    fn producer_inner_return();
}

#[test]
fn outer_scope_keeps_reader_after_inner_return_and_rejects_unowned_inner_call() {
    if !child(
        "ai_operation::gateway::producer_hook::tests::outer_scope_keeps_reader_after_inner_return_and_rejects_unowned_inner_call",
        "JAC_PRODUCER_OUTER_SCOPE_CHILD",
    ) {
        return;
    }
    let inner = unsafe { image_head(ai_fixture_target as *const () as usize, INNER_HEAD) };
    let outer = unsafe { image(producer_outer_fixture as *const () as usize) };
    PRODUCER_INNER_TARGET.store(inner as u64, Ordering::Release);
    let inner_return = producer_inner_return as *const () as u64;
    let guard = INSTALL.lock().unwrap();
    unsafe {
        prepare_set(
            &[
                (outer as u64, HEAD, callers()),
                (inner as u64, INNER_HEAD, [inner_return, inner_return]),
            ],
            1,
            &guard,
        )
    }
    .unwrap();
    drop(guard);
    let mut rejected = [0u8; 0x160];
    unsafe { ai_fixture_driver(rejected.as_mut_ptr(), inner) };
    assert_eq!(u64::from_le_bytes(rejected[88..96].try_into().unwrap()), 0);
    PRODUCER_HOLD_AFTER_INNER.store(1, Ordering::Release);
    let worker = std::thread::spawn(move || {
        let mut output = [0u8; 0x160];
        unsafe { ai_fixture_driver(output.as_mut_ptr(), outer) };
        assert_eq!(u64::from_le_bytes(output[88..96].try_into().unwrap()), 1);
        assert_eq!(
            u64::from_le_bytes(output[0x100..0x108].try_into().unwrap()),
            0x123456789abcdef0
        );
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while PRODUCER_AFTER_INNER.load(Ordering::Acquire) == 0 {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(
        AI_ROOT_PRODUCER.snapshot() as u32,
        1,
        "inner return cannot release the Player lifetime"
    );
    let (tx, rx) = std::sync::mpsc::channel();
    let closer = std::thread::spawn(move || tx.send(close_registered()).unwrap());
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(30))
            .is_err()
    );
    PRODUCER_HOLD_AFTER_INNER.store(0, Ordering::Release);
    worker.join().unwrap();
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(),
        Ok(())
    );
    closer.join().unwrap();
    rebuild_producer(2).unwrap();
    unsafe { ai_fixture_driver(rejected.as_mut_ptr(), outer) };
    assert_eq!(u64::from_le_bytes(rejected[88..96].try_into().unwrap()), 1);
    close_registered().unwrap();
}

#[test]
fn nested_native_seh_releases_only_at_outer_unwind() {
    if !child(
        "ai_operation::gateway::producer_hook::tests::nested_native_seh_releases_only_at_outer_unwind",
        "JAC_PRODUCER_NESTED_SEH_CHILD",
    ) {
        return;
    }
    let inner = unsafe { image_head(original_unwinds as *const () as usize, INNER_HEAD) };
    let outer = unsafe { image(producer_outer_fixture as *const () as usize) };
    PRODUCER_INNER_TARGET.store(inner as u64, Ordering::Release);
    let inner_return = producer_inner_return as *const () as u64;
    let guard = INSTALL.lock().unwrap();
    unsafe {
        prepare_set(
            &[
                (outer as u64, HEAD, callers()),
                (inner as u64, INNER_HEAD, [inner_return, inner_return]),
            ],
            1,
            &guard,
        )
    }
    .unwrap();
    drop(guard);
    let mut state = [0u64; 2];
    assert_eq!(unsafe { ai_unwind_driver(state.as_mut_ptr(), outer) }, 1);
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 0);
    assert_eq!(AI_ROOT_PRODUCER.snapshot() as u32, 0);
    assert_eq!(producer::current_generation(), None);
    assert_eq!(UNWOUND.load(Ordering::Acquire), 2);
    close_registered().unwrap();
    rebuild_producer(2).unwrap();
    assert_eq!(unsafe { ai_unwind_driver(state.as_mut_ptr(), outer) }, 1);
    close_registered().unwrap();
}

#[test]
fn hotpatch_normal_close_and_rebuild_preserve_native_abi_once() {
    if !child(
        "ai_operation::gateway::producer_hook::tests::hotpatch_normal_close_and_rebuild_preserve_native_abi_once",
        "JAC_ROOT_REBUILD_CHILD",
    ) {
        return;
    }
    let entry = unsafe { image(ai_fixture_target as *const () as usize) };
    let mut output = [0u8; 0x160];
    unsafe { ai_fixture_driver(output.as_mut_ptr(), entry) };
    let expected = output;
    let guard = INSTALL.lock().unwrap();
    unsafe { prepare(entry as u64, 1, &guard) }.unwrap();
    drop(guard);
    output.fill(0);
    unsafe { ai_fixture_driver(output.as_mut_ptr(), entry) };
    assert_eq!(
        output, expected,
        "real RX hotpatch/trampoline preserves GP/XMM/flags/home and RAX"
    );
    assert_eq!(u64::from_le_bytes(output[88..96].try_into().unwrap()), 1);
    close_registered().unwrap();
    assert!(stopped());
    output.fill(0);
    unsafe { ai_fixture_driver(output.as_mut_ptr(), entry) };
    assert_eq!(u64::from_le_bytes(output[88..96].try_into().unwrap()), 0);
    assert_eq!(
        u64::from_le_bytes(output[0x100..0x108].try_into().unwrap()),
        0
    );
    rebuild_producer(2).unwrap();
    output.fill(0);
    unsafe { ai_fixture_driver(output.as_mut_ptr(), entry) };
    assert_eq!(output, expected);
    assert_eq!(producer::current_generation(), None);
    close_registered().unwrap();
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 0);
    // Permanent entry/relay remains executable; it is not freed on Close.
}

#[test]
fn producer_seh_releases_reader_and_allows_normal_rebuild() {
    if !child(
        "ai_operation::gateway::producer_hook::tests::producer_seh_releases_reader_and_allows_normal_rebuild",
        "JAC_ROOT_SEH_CHILD",
    ) {
        return;
    }
    let entry = unsafe { image(original_unwinds as *const () as usize) };
    let guard = INSTALL.lock().unwrap();
    unsafe { prepare(entry as u64, 1, &guard) }.unwrap();
    drop(guard);
    let mut state = [0u64; 2];
    assert_eq!(unsafe { ai_unwind_driver(state.as_mut_ptr(), entry) }, 1);
    assert_eq!(producer::current_generation(), None);
    assert_eq!(AI_GATE_FLIGHTS.load(Ordering::Acquire), 0);
    close_registered().unwrap();
    rebuild_producer(2).unwrap();
    assert_eq!(unsafe { ai_unwind_driver(state.as_mut_ptr(), entry) }, 1);
    close_registered().unwrap();
}

static AT_FETCH: AtomicBool = AtomicBool::new(false);
static RELEASE: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU32 = AtomicU32::new(0);
unsafe extern "system" fn waits_before_fetch() -> u32 {
    assert!(producer::current_generation().is_some());
    CALLS.fetch_add(1, Ordering::AcqRel);
    AT_FETCH.store(true, Ordering::Release);
    while !RELEASE.load(Ordering::Acquire) {
        std::thread::yield_now();
    }
    73
}

#[test]
fn close_waits_for_reader_before_callback_fetch_and_new_arrival_is_rejected() {
    if !child(
        "ai_operation::gateway::producer_hook::tests::close_waits_for_reader_before_callback_fetch_and_new_arrival_is_rejected",
        "JAC_ROOT_FETCH_CHILD",
    ) {
        return;
    }
    let entry = unsafe { image(waits_before_fetch as *const () as usize) };
    let guard = INSTALL.lock().unwrap();
    unsafe { prepare(entry as u64, 1, &guard) }.unwrap();
    drop(guard);
    let worker = std::thread::spawn(move || {
        let mut output = [0u8; 0x160];
        unsafe { ai_fixture_driver(output.as_mut_ptr(), entry) };
        assert_eq!(
            u64::from_le_bytes(output[0x100..0x108].try_into().unwrap()),
            73
        );
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !AT_FETCH.load(Ordering::Acquire) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let closer = std::thread::spawn(move || tx.send(close_registered()).unwrap());
    while AI_ROOT_PRODUCER.snapshot() >> 63 != 0 {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(30))
            .is_err()
    );
    let mut late = [0u8; 0x160];
    unsafe { ai_fixture_driver(late.as_mut_ptr(), entry) };
    assert_eq!(
        u64::from_le_bytes(late[0x100..0x108].try_into().unwrap()),
        0
    );
    assert_eq!(CALLS.load(Ordering::Acquire), 1);
    RELEASE.store(true, Ordering::Release);
    worker.join().unwrap();
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(),
        Ok(())
    );
    closer.join().unwrap();
    rebuild_producer(2).unwrap();
    unsafe { ai_fixture_driver(late.as_mut_ptr(), entry) };
    assert_eq!(CALLS.load(Ordering::Acquire), 2);
    close_registered().unwrap();
}
