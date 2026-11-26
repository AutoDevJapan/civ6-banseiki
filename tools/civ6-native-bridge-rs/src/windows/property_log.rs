//! Runtime-only raw property log beside the unchanged CompositeTrace. It is
//! committed and touched before any hook exists and kept until process exit.
//! Each accepted call gets one 48-byte PropertyRow; nothing is compressed,
//! sampled, reset or overwritten. A full log counts drops and fails checks.
use super::*;
use civ6_bootstrap_rs::protocol as wire;

#[repr(C)]
pub struct PropertyLog {
    magic: u32,
    version: u32,
    capacity: u64,
    stride: u64,
    total_bytes: u64,
    pid: u64,
    created_filetime: u64,
    pub(super) next: AtomicU64,
    pub(super) dropped: AtomicU64,
    pub(super) in_flight: AtomicU64,
    pub(super) filtered: AtomicU64,
}
const _: () = {
    assert!(std::mem::size_of::<PropertyLog>() == wire::PROPERTY_LOG_HEADER);
    assert!(std::mem::offset_of!(PropertyLog, next) == wire::PROPERTY_LOG_NEXT);
    assert!(std::mem::offset_of!(PropertyLog, dropped) == wire::PROPERTY_LOG_DROPPED);
    assert!(std::mem::offset_of!(PropertyLog, in_flight) == wire::PROPERTY_LOG_IN_FLIGHT);
    assert!(std::mem::offset_of!(PropertyLog, filtered) == wire::PROPERTY_LOG_FILTERED);
    assert!(std::mem::size_of::<PropertyRow>() == wire::PROPERTY_LOG_STRIDE);
    assert!(std::mem::align_of::<PropertyRow>() <= 8);
    assert!(std::mem::offset_of!(PropertyTrace, rows) == 32);
};

/// Data export: controllers read its current value, the log address.
#[unsafe(no_mangle)]
pub static BRIDGE_RUNTIME_PROPERTY_LOG: AtomicPtr<PropertyLog> = AtomicPtr::new(null_mut());

/// Allocate, zero every page and publish the log. None leaves it unpublished.
pub(super) fn publish() -> Option<()> {
    let created = current_created_filetime()?;
    let base = unsafe {
        VirtualAlloc(
            null_mut(),
            wire::PROPERTY_LOG_BYTES,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    }
    .cast::<u8>();
    if base.is_null() {
        return None;
    }
    unsafe {
        // Touch all pages now; zero rows are unpublished PropertyRow values.
        ptr::write_bytes(base, 0, wire::PROPERTY_LOG_BYTES);
        base.cast::<PropertyLog>().write(PropertyLog {
            magic: wire::PROPERTY_LOG_MAGIC,
            version: wire::PROPERTY_LOG_VERSION,
            capacity: wire::PROPERTY_LOG_CAPACITY as u64,
            stride: wire::PROPERTY_LOG_STRIDE as u64,
            total_bytes: wire::PROPERTY_LOG_BYTES as u64,
            pid: u64::from(GetCurrentProcessId()),
            created_filetime: created,
            next: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            in_flight: AtomicU64::new(0),
            filtered: AtomicU64::new(0),
        });
    }
    BRIDGE_RUNTIME_PROPERTY_LOG.store(base.cast(), Ordering::Release);
    Some(())
}

pub(super) fn published() -> bool {
    !BRIDGE_RUNTIME_PROPERTY_LOG
        .load(Ordering::Acquire)
        .is_null()
}

/// Mark the bounded prefix so that no parser accepts it without the log.
pub(super) unsafe fn mark_prefix(prefix: *mut PropertyTrace) {
    unsafe {
        ptr::addr_of_mut!((*prefix).version).write(wire::PROPERTY_EXTERNAL_VERSION);
        ptr::addr_of_mut!((*prefix).reserved).write(wire::PROPERTY_LOG_MAGIC);
    }
}

pub(super) fn marked(prefix: &PropertyTrace) -> bool {
    published()
        && prefix.version == wire::PROPERTY_EXTERNAL_VERSION
        && prefix.reserved == wire::PROPERTY_LOG_MAGIC
        && prefix.scope == BridgeMode::ProductionAdmission as u32
}

/// Same filter, single native read and authority as the bounded production
/// path. Only the telemetry destination differs; authority never needs a row.
pub(super) fn record(
    log: *const PropertyLog,
    property: u64,
    key_id: u32,
    returned: u64,
    token: u64,
) {
    let header = unsafe { &*log };
    let sampled = runtime::sample_begin();
    if production_ignored(key_id) {
        if sampled {
            header.filtered.fetch_add(1, Ordering::Relaxed);
            runtime::sample_end();
        }
        return;
    }
    let slot = sampled.then(|| {
        header.in_flight.fetch_add(1, Ordering::AcqRel);
        header
            .next
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < wire::PROPERTY_LOG_CAPACITY as u64).then_some(n + 1)
            })
            .ok()
    });
    let read = read_property(returned);
    production_observe(property, key_id, token, read);
    // Unsampled calls keep authority but never touch the log.
    let Some(reserved) = slot else {
        return;
    };
    if let Some(slot) = reserved {
        let row = unsafe { &*log.add(1).cast::<PropertyRow>().add(slot as usize) };
        fill_row(row, property, key_id, returned, read);
        row.tag.store(
            production_tag(read.1, key_id, property, token),
            Ordering::Relaxed,
        );
        row.sequence.store(1, Ordering::Release);
    } else {
        header.dropped.fetch_add(1, Ordering::Relaxed);
    }
    header.in_flight.fetch_sub(1, Ordering::Release);
    runtime::sample_end();
}

#[cfg(test)]
pub(super) fn header_for_test() -> &'static PropertyLog {
    unsafe { BRIDGE_RUNTIME_PROPERTY_LOG.load(Ordering::Acquire).as_ref() }.unwrap()
}

/// Test copy of every log byte, read like a controller.
#[cfg(test)]
pub(super) fn snapshot_for_test() -> Vec<u8> {
    let log = BRIDGE_RUNTIME_PROPERTY_LOG.load(Ordering::Acquire);
    assert!(!log.is_null());
    let mut bytes = vec![0u8; wire::PROPERTY_LOG_BYTES];
    let mut copied = 0;
    assert_ne!(
        unsafe {
            ReadProcessMemory(
                GetCurrentProcess(),
                log.cast(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                &mut copied,
            )
        },
        0
    );
    assert_eq!(copied, bytes.len());
    bytes
}

#[cfg(test)]
pub(super) fn row_for_test(slot: usize) -> &'static PropertyRow {
    assert!(slot < wire::PROPERTY_LOG_CAPACITY);
    unsafe {
        &*BRIDGE_RUNTIME_PROPERTY_LOG
            .load(Ordering::Acquire)
            .add(1)
            .cast::<PropertyRow>()
            .add(slot)
    }
}
