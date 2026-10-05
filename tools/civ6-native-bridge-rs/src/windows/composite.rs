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

pub(super) unsafe fn install(
    can: *mut u8,
    property: *mut u8,
    assign: *mut u8,
    trace: *mut JoinedTrace,
    can_bytes: [u8; 8],
    property_bytes: [u8; 8],
    assign_bytes: [u8; 8],
) -> Result<(), &'static str> {
    // Assignment is installed first, with ENABLED already true and no valid
    // catalog. Thus partial installation never temporarily permits personas.
    product_deactivate();
    ENABLED.store(true, Ordering::Release);
    let assignment = Box::into_raw(AssignCalibrationTrace::boxed());
    let result = (|| {
        unsafe { install_assign_calibration_at(assign, assignment, assign_bytes) }?;
        unsafe {
            install_property_at(
                property,
                ptr::addr_of_mut!((*trace).property),
                property_bytes,
            )
        }?;
        unsafe {
            install_hook(
                can,
                ptr::addr_of_mut!((*trace).calibration).cast(),
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
        let mut child = OwnedChild(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "windows::composite::tests::composite_synthetic_child",
                    "--nocapture",
                ])
                .env("JAC_PAID_COMPOSITE_CHILD", "1")
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
        let mut trace = Box::new(JoinedTrace::default());
        trace.property = PropertyTrace::production_scoped();
        unsafe {
            install(
                can,
                property,
                assign,
                trace.as_mut(),
                cbytes,
                PROPERTY_PROLOGUE,
                abytes,
            )
        }
        .unwrap();
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
        governor[0] = 2 | (77u64 << 32);
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
        let mut publish = |key, value: bool| {
            raw[1] = u64::from(value);
            assert_eq!(get(receiver, key), raw.as_ptr() as u64);
        };
        publish(PAID_RESET_BASE + 2, true);
        assert_eq!(can_call(this, 77), 0, "whole catalog starts closed");
        assert_eq!(assign_call(this, person, 0, sentinel), sentinel & !255);
        let mut words =
            std::array::from_fn::<_, COUNT, _>(|i| i as u64 | 256 | ((100 + i as u64) << 10));
        words[0] = 77 | (3 << 8) | (1 << 10) | (3 << 22) | (7 << 34);
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
        publish(paid_catalog::bit_key(BITS - 1), false);
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
        assert_eq!(can_call(this, 77), 1);
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
        unsafe {
            ptr::write_volatile(governor.as_mut_ptr(), 3 | (77u64 << 32));
        }
        assert_eq!(
            assign_call(this, person, 0, sentinel),
            sentinel & !255,
            "other owner cannot borrow payment"
        );
        unsafe {
            ptr::write_volatile(governor.as_mut_ptr(), 2 | (77u64 << 32));
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
        assert_eq!(trace.property.in_flight.load(Ordering::Acquire), 0);
        assert_eq!(trace.calibration.in_flight.load(Ordering::Acquire), 0);
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
                trace.as_mut(),
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
