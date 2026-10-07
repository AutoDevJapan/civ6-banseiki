//! Explicit resident bootstrap candidate. The owning driver supplies the fresh
//! MainMenu invocation; this library does not launch or inject into any process.
//! Successful preparation is not proof of first-AI or world-reset ordering.
#![deny(unsafe_op_in_unsafe_fn)]
pub mod protocol;

pub const VERSION: u32 = 0x0880_0002;
pub const EXE_SHA: &str = "e7450823cc8e00468cff7b9d7b97c63140eae38ae1d774ba4efa437556c42d63";
pub const CORE_SHA: &str = "324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c";
pub const MENU_CORE_SHA: &str = "d7ed520e1334947fc7b3be50b00c78eea55e69eb079583f24d87742e90f2a5c3";

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Prepare {
    pub version: u32,
    pub size: u32,
    pub pid: u32,
    pub observation_seconds: u32,
    pub created_filetime: u64,
    /// Already-loaded, independently source-hash-pinned Mode6 bridge.
    pub bridge_module: u64,
    pub bridge_sha256: [u8; 32],
    /// Observed Base-menu Context, not an XP2 Context. Target must be absent.
    pub expected_menu_context: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RuntimeConfig {
    pub version: u32,
    pub size: u32,
    pub pid: u32,
    pub max_seconds: u32,
    pub created_filetime: u64,
    pub trace_address: u64,
    pub trace_size: u32,
    pub mode: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(any(test, all(windows, target_arch = "x86_64")))]
enum Phase {
    Prepared,
    Open,
    Closing,
    Closed,
    UnsupportedInlineExchange,
}

/// A Close event, not pointer inequality, defines the next generation. An
/// allocator may return the very same object address after actual destruction.
#[cfg(any(test, all(windows, target_arch = "x86_64")))]
fn rebuild_before_set(phase: Phase, old: u64, incoming: u64, caller: u64) -> bool {
    phase == Phase::Closed && old == 0 && incoming != 0 && caller == 0x24b5d5
}

#[cfg(all(windows, target_arch = "x86_64"))]
mod windows;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_abi_is_exact_and_noop_cannot_rebuild() {
        assert_eq!(std::mem::size_of::<RuntimeConfig>(), 40);
        assert_eq!(std::mem::align_of::<RuntimeConfig>(), 8);
        assert_eq!(std::mem::offset_of!(RuntimeConfig, trace_address), 24);
        assert_eq!(std::mem::offset_of!(RuntimeConfig, mode), 36);
        for phase in [
            Phase::Prepared,
            Phase::Open,
            Phase::Closing,
            Phase::UnsupportedInlineExchange,
        ] {
            assert!(!rebuild_before_set(phase, 0, 7, 0x24b5d5));
        }
        assert!(!rebuild_before_set(Phase::Closed, 7, 7, 0x24b5d5));
        assert!(!rebuild_before_set(Phase::Closed, 0, 0, 0x24b5d5));
        assert!(!rebuild_before_set(Phase::Closed, 0, 7, 0x169982));
        // Symbolic policy test, not a fake successful runtime API call.
        assert!(rebuild_before_set(Phase::Closed, 0, 7, 0x24b5d5));
    }
}
