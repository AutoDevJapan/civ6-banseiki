//! One experimental lease owns appointment, natural property and assignment.
//! No default path enables this module. Catalog transport is synchronized
//! Gameplay -> natural Core getter -> copied bool, never controller metadata.
use super::*;
use crate::governor_payment::PaymentDefinition;
use crate::paid_catalog::{self, BITS, COMMIT_KEY, COUNT, WIDTH};

pub(super) static ENABLED: AtomicBool = AtomicBool::new(false);
static CATALOG: [AtomicJoinedWord; PRODUCTION_OWNER_LIMIT * BITS] =
    [const { AtomicJoinedWord::new() }; PRODUCTION_OWNER_LIMIT * BITS];
static COMMIT: [AtomicJoinedWord; PRODUCTION_OWNER_LIMIT] =
    [const { AtomicJoinedWord::new() }; PRODUCTION_OWNER_LIMIT];
pub(super) static ASSIGNMENT_HOOK: Mutex<Option<Hook>> = Mutex::new(None);
static TRACE: AtomicPtr<CompositeTrace> = AtomicPtr::new(null_mut());

/// The ordinary joined trace remains the prefix. Assignment decisions have
/// their own storage; mode7's diagnostic layout and raw RAX stay unchanged.
#[repr(C)]
pub struct CompositeTrace {
    pub joined: JoinedTrace,
    pub assignment: AssignCalibrationTrace,
    pub effective_rax: [AtomicU64; assignment::ASSIGN_CAPACITY],
}

impl CompositeTrace {
    pub fn boxed() -> Box<Self> {
        let mut value = Box::<Self>::new_uninit();
        unsafe {
            Self::initialize_at(value.as_mut_ptr());
            value.assume_init()
        }
    }
    pub(super) unsafe fn initialize_at(trace: *mut Self) {
        unsafe {
            ptr::addr_of_mut!((*trace).joined).write(JoinedTrace::default());
            ptr::addr_of_mut!((*trace).joined.property).write(PropertyTrace::production_scoped());
            AssignCalibrationTrace::initialize_at(ptr::addr_of_mut!((*trace).assignment));
            let rows = ptr::addr_of_mut!((*trace).effective_rax).cast::<AtomicU64>();
            for n in 0..assignment::ASSIGN_CAPACITY {
                rows.add(n).write(AtomicU64::new(0));
            }
        }
    }
}

pub(super) fn record_effective(token: u64, result: u64) {
    let token = token & 0xffff_ffff;
    if token > 0 && token <= assignment::ASSIGN_CAPACITY as u64 {
        if let Some(trace) = unsafe { TRACE.load(Ordering::Acquire).as_ref() } {
            trace.effective_rax[token as usize - 1].store(result, Ordering::Release);
        }
    }
}

pub(super) fn handles(key: u32) -> bool {
    paid_assignment::is_key(key) || key == COMMIT_KEY || paid_catalog::bit_index(key).is_some()
}

pub(super) fn begin(receiver: u64, key: u32) -> u64 {
    if !ENABLED.load(Ordering::Acquire) {
        return 0;
    }
    if paid_assignment::is_key(key) {
        return PaidAssignmentConsumer::begin_token(receiver, key);
    }
    // Allocate a normal owner-bound ticket, without publishing a payment word.
    if key == COMMIT_KEY || paid_catalog::bit_index(key).is_some() {
        return PaidAssignmentConsumer::begin_token(receiver, PAID_MANIFEST_BASE);
    }
    0
}

pub(super) fn observe(
    receiver: u64,
    key: u32,
    token: u64,
    state: u32,
    tag: u32,
    payload: u64,
    flags: u32,
) {
    if !ENABLED.load(Ordering::Acquire) || token == 0 {
        return;
    }
    if paid_assignment::is_key(key) {
        PaidAssignmentConsumer::observe(
            PaidAssignmentRead::from_token(receiver, key, token),
            state,
            tag,
            payload,
            flags,
        );
        return;
    }
    let owner = ((token >> 31) & 63) as usize;
    let session = (token >> 37) as u32;
    let ticket = token as u32 & JOINED_TICKET_MAX;
    let binding = PRODUCT_OWNERS[owner].load();
    if joined_receiver(binding) != receiver
        || joined_session(binding) != session
        || joined_ticket(binding) >= ticket
    {
        return;
    }
    if state != 1 || tag != 1 || flags != 0 || payload > 1 {
        product_publish_word(&COMMIT[owner], receiver, session, ticket, false);
        return;
    }
    let cell = if key == COMMIT_KEY {
        &COMMIT[owner]
    } else if let Some(bit) = paid_catalog::bit_index(key) {
        &CATALOG[owner * BITS + bit]
    } else {
        return;
    };
    product_publish_word(cell, receiver, session, ticket, payload == 1);
    if key == COMMIT_KEY && runtime::owned() {
        if payload == 0 {
            runtime::begin_catalog(owner, joined_ticket(binding));
        } else {
            let complete = catalog(owner, receiver, session).is_some();
            runtime::publication(owner, joined_ticket(binding), complete, complete);
        }
    }
}

pub(super) fn catalog(
    owner: usize,
    receiver: u64,
    session: u32,
) -> Option<[PaymentDefinition; COUNT]> {
    if owner >= PRODUCTION_OWNER_LIMIT
        || !PRODUCT_ACTIVE.load(Ordering::Acquire)
        || PRODUCT_EXHAUSTED.load(Ordering::Acquire)
    {
        return None;
    }
    let binding = PRODUCT_OWNERS[owner].load();
    let commit = COMMIT[owner].load();
    if joined_receiver(binding) != receiver
        || joined_session(binding) != session
        || joined_receiver(commit) != receiver
        || joined_session(commit) != session
        || joined_ticket(commit) <= joined_ticket(binding)
        || !joined_permitted(commit)
    {
        return None;
    }
    let mut words = [0u64; COUNT];
    for (bit, cell) in CATALOG[owner * BITS..(owner + 1) * BITS].iter().enumerate() {
        let value = cell.load();
        if joined_receiver(value) != receiver
            || joined_session(value) != session
            || joined_ticket(value) <= joined_ticket(binding)
            || joined_ticket(value) >= joined_ticket(commit)
        {
            return None;
        }
        words[bit / WIDTH] |= u64::from(joined_permitted(value)) << (bit % WIDTH);
    }
    let decoded = paid_catalog::decode(words)?;
    // A newer bit cannot belong to the old committed transaction. Repeat the
    // ticket fence after copying; no borrowed pointer survives the callback.
    if CATALOG[owner * BITS..(owner + 1) * BITS]
        .iter()
        .any(|cell| joined_ticket(cell.load()) >= joined_ticket(commit))
        || PRODUCT_OWNERS[owner].load() != binding
        || COMMIT[owner].load() != commit
        || PRODUCT_SESSION.load(Ordering::Acquire) != session
        || !PRODUCT_ACTIVE.load(Ordering::Acquire)
    {
        return None;
    }
    Some(decoded)
}

pub(super) fn payment_after_catalog(owner: usize, ticket: u32) -> bool {
    owner < PRODUCTION_OWNER_LIMIT && joined_ticket(COMMIT[owner].load()) < ticket
}

pub(super) fn ready_catalog(
    owner: usize,
    receiver: u64,
    session: u32,
) -> Option<[PaymentDefinition; COUNT]> {
    if !runtime::ready(owner) {
        return None;
    }
    let catalog = catalog(owner, receiver, session)?;
    runtime::ready(owner).then_some(catalog)
}

pub(super) unsafe fn install(
    can: *mut u8,
    property: *mut u8,
    assign: *mut u8,
    trace: *mut CompositeTrace,
    can_bytes: [u8; 8],
    property_bytes: [u8; 8],
    assign_bytes: [u8; 8],
) -> Result<(), &'static str> {
    // Assignment is installed first, with ENABLED already true and no valid
    // catalog. Thus partial installation never temporarily permits personas.
    product_deactivate();
    ENABLED.store(true, Ordering::Release);
    TRACE.store(trace, Ordering::Release);
    let assignment = unsafe { ptr::addr_of_mut!((*trace).assignment) };
    let result = (|| {
        unsafe { install_assign_calibration_at(assign, assignment, assign_bytes) }?;
        unsafe {
            install_property_at(
                property,
                ptr::addr_of_mut!((*trace).joined.property),
                property_bytes,
            )
        }?;
        unsafe {
            install_hook(
                can,
                ptr::addr_of_mut!((*trace).joined.calibration).cast(),
                can_bytes,
                BridgeMode::ProductionAdmission,
            )
        }?;
        if !product_activate() {
            return Err("composite session exhausted");
        }
        Ok(())
    })();
    if let Err(error) = result {
        // Retain relay/trace storage for already-fetched jumps, like the other
        // hooks. All entries belong to the same bounded controller lease.
        let restored = unsafe { uninstall() };
        return if restored.is_ok() {
            Err(error)
        } else {
            Err("critical: composite rollback failed")
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    struct OwnedChild(std::process::Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    #[test]
    fn composite_win64_owned_child() {
        owned_child(false);
    }

    #[test]
    fn runtime_win64_owned_child() {
        owned_child(true);
    }

    fn owned_child(runtime: bool) {
        let mut child = OwnedChild(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "windows::composite::tests::composite_synthetic_child",
                    "--nocapture",
                ])
                .env("JAC_PAID_COMPOSITE_CHILD", "1")
                .env("JAC_RUNTIME_CHILD", if runtime { "1" } else { "0" })
                .creation_flags(0x08000000)
                .spawn()
                .unwrap(),
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if std::time::Instant::now() >= deadline {
                panic!("composite child exceeded deadline");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn composite_synthetic_child() {
        if std::env::var("JAC_PAID_COMPOSITE_CHILD").as_deref() != Ok("1") {
            return;
        }
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
        let can = unsafe { page.add(16) };
        let property = unsafe { page.add(128) };
        let assign = unsafe { page.add(256) };
        let can_code = [
            0x48, 0x89, 0x5c, 0x24, 0x08, 0xff, 0x41, 0x78, 0xb8, 1, 0, 0, 0, 0xc3,
        ];
        let property_code = [
            0x48, 0x8b, 0x41, 0x40, 0x4c, 0x8d, 0x49, 0x30, 0xff, 0x41, 0x48, 0xc3,
        ];
        let assign_code = [
            0x48, 0x89, 0x5c, 0x24, 0x08, 0x48, 0x89, 0x6c, 0x24, 0x10, 0xff, 0x41, 0x78, 0x4c,
            0x89, 0xc8, 0xc3,
        ];
        unsafe {
            ptr::copy_nonoverlapping(can_code.as_ptr(), can, can_code.len());
            ptr::copy_nonoverlapping(property_code.as_ptr(), property, property_code.len());
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), property.sub(5), 5);
            ptr::copy_nonoverlapping(assign_code.as_ptr(), assign, assign_code.len());
        }
        let cbytes = can_code[..8].try_into().unwrap();
        let abytes = assign_code[..8].try_into().unwrap();
        let is_runtime = std::env::var("JAC_RUNTIME_CHILD").as_deref() == Ok("1");
        // Runtime and controller both use this process-owned allocation. It
        // never drops when observation stops, including the failure case.
        let trace_pointer = if is_runtime {
            runtime::allocate_trace()
        } else {
            Box::into_raw(CompositeTrace::boxed())
        };
        unsafe {
            install(
                can,
                property,
                assign,
                trace_pointer,
                cbytes,
                PROPERTY_PROLOGUE,
                abytes,
            )
        }
        .unwrap();
        let trace = unsafe { &*trace_pointer };
        if is_runtime {
            runtime::start_observation(MAX_ACTIVE_SECONDS);
        }
        let can_call: extern "system" fn(u64, u32) -> u64 = unsafe { std::mem::transmute(can) };
        let get: extern "system" fn(u64, u32) -> u64 = unsafe { std::mem::transmute(property) };
        let assign_call: extern "system" fn(u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(assign) };
        let mut player = [0u64; 192];
        player[0xd8 / 8] = 2;
        let mut raw = [0u64; 3];
        raw[0] = 1;
        player[0x568 / 8] = raw.as_ptr() as u64;
        let receiver = player.as_ptr() as u64 + 0x528;
        let mut governor = [0u64; 16];
        governor[0] = 0x10001 | (77u64 << 32);
        governor[0x18 / 8] = 2;
        let mut flags = [0u8; 128];
        governor[0x68 / 8] = flags.as_ptr() as u64;
        governor[0x70 / 8] = flags.len() as u64;
        governor[0x78 / 8] = flags.len() as u64;
        let node = [governor.as_ptr() as u64, 0, 0];
        let mut governors = [0u64; 32];
        governors[0x58 / 8] = node.as_ptr() as u64;
        governors[0x70 / 8] = player.as_ptr() as u64;
        let this = governors.as_ptr() as u64;
        let person = governor.as_ptr() as u64;
        let sentinel = 0xaabb_ccdd_1122_3301;
        let raw_tag = raw.as_mut_ptr();
        let mut publish = |key, value: bool| {
            raw[1] = u64::from(value);
            assert_eq!(get(receiver, key), raw.as_ptr() as u64);
        };
        publish(PAID_RESET_BASE + 2, true);
        if is_runtime {
            assert_eq!(runtime::phase(2), 1, "reset enters rebuilding");
        }
        assert_eq!(can_call(this, 77), 0, "whole catalog starts closed");
        assert_eq!(assign_call(this, person, 0, sentinel), sentinel & !255);
        let mut words =
            std::array::from_fn::<_, COUNT, _>(|i| i as u64 | 256 | ((100 + i as u64) << 10));
        words[0] = 77 | (3 << 8) | (1 << 10) | (3 << 22) | (7 << 34);
        publish(COMMIT_KEY, false);
        for bit in 0..BITS - 1 {
            publish(
                paid_catalog::bit_key(bit),
                words[bit / WIDTH] & (1 << (bit % WIDTH)) != 0,
            );
        }
        publish(COMMIT_KEY, true);
        assert_eq!(
            can_call(this, 77),
            0,
            "21 and a partial record cannot initialize"
        );
        if is_runtime {
            assert_eq!(runtime::phase(2), 0, "failed commit closes runtime");
            publish(PAID_RESET_BASE + 2, true);
            publish(COMMIT_KEY, false);
            for bit in 0..BITS - 1 {
                publish(
                    paid_catalog::bit_key(bit),
                    words[bit / WIDTH] & (1 << (bit % WIDTH)) != 0,
                );
            }
        }
        publish(paid_catalog::bit_key(BITS - 1), false);
        publish(COMMIT_KEY, true);
        if is_runtime {
            assert_eq!(
                runtime::phase(2),
                2,
                "complete copied catalog becomes ready"
            );
        }
        for (base, value) in [
            (PRODUCTION_CLOSE_BASE, true),
            (PRODUCTION_ALLOW_BASE, false),
            (PRODUCTION_MANIFEST_BASE, true),
            (PRODUCTION_ALLOW_BASE, true),
            (PAID_CLOSE_BASE, true),
            (PAID_ALLOW_BASE, false),
            (PAID_MANIFEST_BASE, true),
            (PAID_ALLOW_BASE, true),
        ] {
            publish(base + 77, value);
        }
        assert_eq!(can_call(this, 77), 1);
        if is_runtime {
            unsafe {
                ptr::write_volatile(can.add(9), 0);
            }
            assert_eq!(
                can_call(this, 77),
                0,
                "runtime must not promote original false to true"
            );
            unsafe {
                ptr::write_volatile(can.add(9), 1);
            }
            unsafe {
                ptr::write_volatile(governor.as_mut_ptr(), 0x10001 | (200u64 << 32));
            }
            assert_eq!(
                can_call(this, 200),
                1,
                "classified ordinary governor keeps original result"
            );
            assert_eq!(assign_call(this, person, 0, sentinel), sentinel);
            unsafe {
                ptr::write_volatile(governor.as_mut_ptr(), 0x10001 | (77u64 << 32));
            }
        }
        assert_eq!(
            assign_call(this, person, 0, sentinel),
            sentinel & !255,
            "ledger true alone never pays"
        );
        unsafe {
            ptr::write_volatile(flags.as_mut_ptr().add(1), 1);
        }
        assert_eq!(
            assign_call(this, person, 0, sentinel),
            sentinel & !255,
            "cost3 base alone is unpaid"
        );
        unsafe {
            ptr::write_volatile(flags.as_mut_ptr().add(3), 1);
        }
        assert_eq!(
            assign_call(this, person, 0, sentinel),
            sentinel & !255,
            "cost3 first title alone is unpaid"
        );
        unsafe {
            ptr::write_volatile(flags.as_mut_ptr().add(7), 1);
        }
        let calls = governors[15];
        assert_eq!(assign_call(this, person, 0, sentinel), sentinel);
        assert_eq!(governors[15], calls + 1, "original is called once");
        assert_eq!(
            assign_call(this, person, 0, sentinel & !255),
            sentinel & !255
        );
        if is_runtime {
            // Fill real gateway samples; do not simulate capacity by editing
            // counters. Gate decisions still run on caller scratch/current
            // objects once all three finite record regions are exhausted.
            for _ in 0..TRACE_CAPACITY + 1 {
                publish(PAID_ALLOW_BASE + 77, true);
            }
            for _ in 0..CALIBRATION_CAPACITY + 1 {
                assert_eq!(can_call(this, 77), 1);
            }
            for _ in 0..assignment::ASSIGN_CAPACITY + 1 {
                assert_eq!(assign_call(this, person, 0, sentinel), sentinel);
            }
            assert_eq!(
                trace.joined.property.next.load(Ordering::Acquire),
                TRACE_CAPACITY as u32
            );
            assert_eq!(
                trace.joined.calibration.next.load(Ordering::Acquire),
                CALIBRATION_CAPACITY as u32
            );
            assert_eq!(
                trace.assignment.next.load(Ordering::Acquire),
                assignment::ASSIGN_CAPACITY as u32
            );
            assert!(trace.assignment.dropped.load(Ordering::Acquire) > 0);
            unsafe {
                ptr::write_volatile(flags.as_mut_ptr().add(7), 0);
            }
            assert_eq!(
                assign_call(this, person, 0, sentinel),
                sentinel & !255,
                "unpaid after capacity"
            );
            unsafe {
                ptr::write_volatile(flags.as_mut_ptr().add(7), 1);
            }
            runtime::expire_observation();
            assert_eq!(
                assign_call(this, person, 0, sentinel),
                sentinel,
                "paid after expiry"
            );
            assert_eq!(
                runtime::BridgeRuntimeStopObservation(null_mut()),
                BridgeStatus::Ok as u32
            );
            assert_eq!(
                BridgeUninstall(null_mut()),
                BridgeStatus::InvalidInput as u32
            );
            assert!(HOOK.lock().unwrap().is_some());
            assert!(ASSIGNMENT_HOOK.lock().unwrap().is_some());
            assert!(JOINED_PROPERTY_HOOK.lock().unwrap().is_some());
            assert_eq!(
                CALIBRATION_TRACE_PTR.load(Ordering::Acquire),
                ptr::addr_of!(trace.joined.calibration).cast_mut()
            );
            assert_eq!(
                ASSIGN_TRACE_PTR.load(Ordering::Acquire),
                ptr::addr_of!(trace.assignment).cast_mut()
            );
            unsafe {
                ptr::write_volatile(flags.as_mut_ptr().add(7), 0);
            }
            assert_eq!(
                assign_call(this, person, 0, sentinel),
                sentinel & !255,
                "unpaid after stop"
            );
            unsafe {
                ptr::write_volatile(flags.as_mut_ptr().add(7), 1);
            }
            assert_eq!(
                assign_call(this, person, 0, sentinel),
                sentinel,
                "paid after stop"
            );
            // The assignment decision is independent even of the telemetry
            // pointer. The actual runtime keeps it resident; test the fallback
            // callback path without redirecting it to another allocation.
            let assignment_trace = ASSIGN_TRACE_PTR.swap(null_mut(), Ordering::AcqRel);
            unsafe {
                ptr::write_volatile(flags.as_mut_ptr().add(7), 0);
            }
            assert_eq!(assign_call(this, person, 0, sentinel), sentinel & !255);
            unsafe {
                ptr::write_volatile(flags.as_mut_ptr().add(7), 1);
            }
            assert_eq!(assign_call(this, person, 0, sentinel), sentinel);
            ASSIGN_TRACE_PTR.store(assignment_trace, Ordering::Release);
            let calibration_trace = CALIBRATION_TRACE_PTR.swap(null_mut(), Ordering::AcqRel);
            assert_eq!(
                can_call(this, 77),
                0,
                "missing scratch trace must not bypass gate"
            );
            CALIBRATION_TRACE_PTR.store(calibration_trace, Ordering::Release);
            assert_eq!(
                assign_call(this, person, 0, sentinel & !255),
                sentinel & !255
            );
            // Capture both entry callbacks before reload; finish them only
            // after a new generation has rebuilt a complete catalog/payment.
            let old_assign =
                unsafe { assignment::assign_calibration_begin(this, person, 0, sentinel, 0) };
            let mut scratch = CalibrationRow::default();
            let old_can = unsafe { calibration_begin(this, 77, 0, &mut scratch) };
            let old_property = composite::begin(receiver, PAID_ALLOW_BASE + 77);
            assert_eq!(
                runtime::BridgeRuntimeRebuild(null_mut()),
                BridgeStatus::Ok as u32
            );
            assert_eq!(can_call(this, 77), 0, "reload begins closed");
            assert_eq!(assign_call(this, person, 0, sentinel), sentinel & !255);
            publish(PAID_RESET_BASE + 2, true);
            publish(COMMIT_KEY, false);
            for bit in 0..BITS {
                publish(
                    paid_catalog::bit_key(bit),
                    words[bit / WIDTH] & (1 << (bit % WIDTH)) != 0,
                );
            }
            publish(COMMIT_KEY, true);
            for (base, value) in [
                (PRODUCTION_CLOSE_BASE, true),
                (PRODUCTION_ALLOW_BASE, false),
                (PRODUCTION_MANIFEST_BASE, true),
                (PRODUCTION_ALLOW_BASE, true),
                (PAID_CLOSE_BASE, true),
                (PAID_ALLOW_BASE, false),
                (PAID_MANIFEST_BASE, true),
                (PAID_ALLOW_BASE, true),
            ] {
                publish(base + 77, value);
            }
            assert_eq!(assign_call(this, person, 0, sentinel), sentinel);
            assert_eq!(
                unsafe { assignment::assign_calibration_end(old_assign, sentinel, this, person) },
                sentinel & !255
            );
            assert_eq!(
                unsafe { calibration_end(old_can, sentinel, 77, this) },
                sentinel & !255
            );
            composite::observe(receiver, PAID_ALLOW_BASE + 77, old_property, 1, 1, 0, 0);
            assert_eq!(
                assign_call(this, person, 0, sentinel),
                sentinel,
                "old property cannot republish"
            );
            assert_eq!(
                trace.assignment.next.load(Ordering::Acquire),
                assignment::ASSIGN_CAPACITY as u32
            );
            // A malformed publication closes even after observation has ended.
            unsafe {
                ptr::write_volatile(raw_tag, 2);
            }
            assert_eq!(get(receiver, PAID_ALLOW_BASE + 77), raw_tag as u64);
            assert_eq!(runtime::phase(2), 0);
            assert_eq!(assign_call(this, person, 0, sentinel), sentinel & !255);
            unsafe {
                ptr::write_volatile(governor.as_mut_ptr(), 0x10001 | (200u64 << 32));
            }
            assert_eq!(
                assign_call(this, person, 0, sentinel),
                sentinel & !255,
                "closed runtime must not borrow an old ordinary classification"
            );
            unsafe {
                ptr::write_volatile(governor.as_mut_ptr(), 0x10001 | (77u64 << 32));
            }
            unsafe {
                ptr::write_volatile(raw_tag, 1);
            }
        }
        unsafe {
            ptr::write_volatile(governor.as_mut_ptr().add(0x18 / 8), 3);
        }
        assert_eq!(
            assign_call(this, person, 0, sentinel),
            sentinel & !255,
            "other owner cannot borrow payment"
        );
        unsafe {
            ptr::write_volatile(governor.as_mut_ptr().add(0x18 / 8), 2);
        }
        publish(PAID_RESET_BASE + 2, true);
        assert_eq!(
            assign_call(this, person, 0, sentinel),
            sentinel & !255,
            "reload closes stale true"
        );
        unsafe { uninstall() }.unwrap();
        assert_eq!(unsafe { ptr::read(can.cast::<[u8; 8]>()) }, cbytes);
        assert_eq!(
            unsafe { ptr::read(property.cast::<[u8; 8]>()) },
            PROPERTY_PROLOGUE
        );
        assert_eq!(unsafe { ptr::read(assign.cast::<[u8; 8]>()) }, abytes);
        assert_eq!(assign_call(this, person, 0, sentinel), sentinel);
        assert_eq!(trace.joined.property.in_flight.load(Ordering::Acquire), 0);
        assert_eq!(
            trace.joined.calibration.in_flight.load(Ordering::Acquire),
            0
        );
        if is_runtime {
            assert_eq!(
                trace.assignment.complete_rows().unwrap_err(),
                "assignment capacity exceeded"
            );
            assert!(trace
                .assignment
                .rows
                .iter()
                .all(|row| row.sequence.load(Ordering::Acquire) == 1));
        } else {
            assert_eq!(trace.assignment.complete_rows().unwrap().len(), 8);
        }
        assert_eq!(
            trace.effective_rax[0].load(Ordering::Acquire),
            sentinel & !255
        );
        assert_eq!(
            trace.effective_rax[if is_runtime { 5 } else { 4 }].load(Ordering::Acquire),
            sentinel
        );
        // A third-entry failure must restore the first two entries as well.
        unsafe {
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), property.sub(5), 5);
        }
        let mut wrong = cbytes;
        wrong[0] ^= 1;
        assert!(unsafe {
            install(
                can,
                property,
                assign,
                trace_pointer,
                wrong,
                PROPERTY_PROLOGUE,
                abytes,
            )
        }
        .is_err());
        assert_eq!(
            unsafe { ptr::read(property.cast::<[u8; 8]>()) },
            PROPERTY_PROLOGUE
        );
        assert_eq!(unsafe { ptr::read(assign.cast::<[u8; 8]>()) }, abytes);
        assert!(HOOK.lock().unwrap().is_none());
        assert!(ASSIGNMENT_HOOK.lock().unwrap().is_none());
        assert!(JOINED_PROPERTY_HOOK.lock().unwrap().is_none());
    }
}
