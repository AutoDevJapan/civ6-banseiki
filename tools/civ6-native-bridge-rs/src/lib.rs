//! Separate, opt-in native bridge prototype. It does not share the read-only
//! `civ6-native-probe-rs` debugger contract or load into a game by itself.
//! Its first stage only records the caller/Index and tail-jumps to the
//! original CanEver function. No Lua state or native property value is read.

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{
    BridgeConfig, BridgeStatus, BridgeTrace, SharedTrace, TraceRow, GAMECORE, GAME_EXE,
    MAX_ACTIVE_SECONDS, PINNED_CAN_EVER_RVA, PINNED_GAMECORE_SHA256, TRACE_CAPACITY,
};

#[cfg(not(windows))]
pub const TRACE_CAPACITY: usize = 4096;
