//! Separate, opt-in native bridge prototype. It does not share the read-only
//! `civ6-native-probe-rs` debugger contract or load into a game by itself.
//! CanEver mode records caller/Index and tail-jumps to the original function.
//! Separate property mode calls the original getter once and copies bounded
//! raw value bytes after it returns. Private and keyed admission modes read
//! only that natural getter result. No mode enters Lua.

/// Portable scenario limit; only the Windows module implements execution.
pub const MAX_ACTIVE_SECONDS: u32 = 300;

/// Identity used by both the Windows bridge and portable artifact validators.
pub const PINNED_GAMECORE_SHA256: &str =
    "324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c";

pub mod appointment_glue;
pub mod governor_payment;
pub mod paid_artifact;
pub mod paid_catalog;
pub mod paid_glue;
pub mod persona_finalization;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{
    supports_atomic_promotion_fetch, AssignCalibrationObservation, AssignCalibrationTrace,
    BridgeConfig, BridgeMode, BridgeStatus, BridgeTrace, CalibrationObservation, CalibrationTrace,
    CompositeTrace, JoinedTrace, PaidAssignmentConsumer, PaidAssignmentRead, PromotionObservation,
    PromotionTrace, PropertyTrace, PropertyValueTrace, SharedTrace, TraceRow,
    ASHINA_ADMISSION_KEY_ID, CALIBRATION_CAPACITY, EXPERIMENTAL_PAID_CONFIG_VERSION, GAMECORE,
    GAME_EXE, PAID_ALLOW_BASE, PAID_CLOSE_BASE, PAID_MANIFEST_BASE, PAID_RESET_BASE,
    PINNED_CAN_ASSIGN_RVA, PINNED_CAN_EVER_RVA, PINNED_PROMOTION_CALLEE_RVA,
    PINNED_PROMOTION_CALL_RVA, PINNED_PROPERTY_GET_RVA, PRODUCTION_ALLOW_BASE,
    PRODUCTION_CLOSE_BASE, PRODUCTION_INDEX_LIMIT, PRODUCTION_MANIFEST_BASE, PRODUCTION_RESET_KEY,
    PRODUCTION_RESET_LAST_KEY, PROMOTION_TRACE_CAPACITY, PROPERTY_TEST_PHASE_KEY_ID,
    PROPERTY_TEST_VALUE_KEY_ID, RUNTIME_CONFIG_VERSION, TRACE_CAPACITY,
};

#[cfg(not(windows))]
pub const TRACE_CAPACITY: usize = 4096;
