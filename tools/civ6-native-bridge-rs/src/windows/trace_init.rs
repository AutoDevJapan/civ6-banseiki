//! Initialize the caller's trace storage directly. No trace-sized temporary on
//! a controller-created thread (the game image reserves only 1 MiB).
use super::*;
macro_rules! header {
    ($target:ident, $($field:ident=$value:expr),+ $(,)?) => {$(unsafe {ptr::addr_of_mut!((*$target).$field).write($value)};)+};
}
macro_rules! rows {
    ($target:ident,$row:ty,$count:expr) => {{
        let rows = unsafe { ptr::addr_of_mut!((*$target).rows).cast::<$row>() };
        for n in 0..$count {
            unsafe { rows.add(n).write(<$row>::default()) };
        }
    }};
}
impl SharedTrace {
    pub(super) unsafe fn initialize_at(target: *mut Self) {
        header!(
            target,
            magic = TRACE_MAGIC,
            version = TRACE_VERSION,
            next = AtomicU32::new(0),
            dropped = AtomicU32::new(0),
            in_flight = AtomicU32::new(0),
            reserved = 0
        );
        rows!(target, TraceRow, TRACE_CAPACITY);
    }
}
impl CalibrationTrace {
    pub fn boxed() -> Box<Self> {
        let mut value = Box::<Self>::new_uninit();
        unsafe {
            Self::initialize_at(value.as_mut_ptr());
            value.assume_init()
        }
    }
    pub(super) unsafe fn initialize_at(target: *mut Self) {
        header!(
            target,
            magic = CALIBRATION_TRACE_MAGIC,
            version = CALIBRATION_TRACE_VERSION,
            next = AtomicU32::new(0),
            dropped = AtomicU32::new(0),
            in_flight = AtomicU32::new(0),
            reserved = 0
        );
        rows!(target, CalibrationRow, CALIBRATION_CAPACITY);
    }
}
impl PropertyTrace {
    pub(super) unsafe fn initialize_at(target: *mut Self, scope: u32) {
        header!(
            target,
            magic = PROPERTY_TRACE_MAGIC,
            version = PROPERTY_TRACE_VERSION,
            next = AtomicU32::new(0),
            dropped = AtomicU32::new(0),
            in_flight = AtomicU32::new(0),
            filtered = AtomicU32::new(0),
            scope = scope,
            reserved = 0
        );
        rows!(target, PropertyRow, TRACE_CAPACITY);
    }
}
impl JoinedTrace {
    pub fn boxed() -> Box<Self> {
        let mut value = Box::<Self>::new_uninit();
        unsafe {
            Self::initialize_at(value.as_mut_ptr(), BridgeMode::JoinedAdmission);
            value.assume_init()
        }
    }
    pub(super) unsafe fn initialize_at(target: *mut Self, scope: BridgeMode) {
        unsafe {
            CalibrationTrace::initialize_at(ptr::addr_of_mut!((*target).calibration));
            PropertyTrace::initialize_at(ptr::addr_of_mut!((*target).property), scope as u32);
        }
    }
}
impl PromotionTrace {
    pub(super) unsafe fn initialize_at(target: *mut Self) {
        header!(
            target,
            magic = PROMOTION_TRACE_MAGIC,
            version = PROMOTION_TRACE_VERSION,
            mode = BridgeMode::PromotionCall as u32,
            capacity = PROMOTION_TRACE_CAPACITY as u32,
            next = AtomicU32::new(0),
            dropped = AtomicU32::new(0),
            in_flight = AtomicU32::new(0),
            reserved = 0
        );
        rows!(target, PromotionRow, PROMOTION_TRACE_CAPACITY);
    }
}
#[cfg(test)]
#[path = "trace_init_tests.rs"]
mod tests;
