//! Separate, opt-in native bridge prototype. It does not share the read-only
//! `civ6-native-probe-rs` debugger contract or load into a game by itself.
//! CanEver mode records caller/Index and tail-jumps to the original function.
//! Separate property mode calls the original getter once and copies bounded
//! raw value bytes after it returns. Neither mode enters the Lua VM.

/// Portable scenario limit; only the Windows module implements execution.
pub const MAX_ACTIVE_SECONDS: u32 = 300;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{
    BridgeConfig, BridgeMode, BridgeStatus, BridgeTrace, PropertyTrace, PropertyValueTrace,
    SharedTrace, TraceRow, GAMECORE, GAME_EXE, PINNED_CAN_EVER_RVA, PINNED_GAMECORE_SHA256,
    PINNED_PROPERTY_GET_RVA, PROPERTY_TEST_PHASE_KEY_ID, PROPERTY_TEST_VALUE_KEY_ID,
    TRACE_CAPACITY,
};

#[cfg(not(windows))]
pub const TRACE_CAPACITY: usize = 4096;
