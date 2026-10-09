//! Explicit resident bootstrap candidate. The owning driver supplies the fresh
//! MainMenu invocation; this library does not launch or inject into any process.
//! Successful preparation is not proof of first-AI or world-reset ordering.
#![deny(unsafe_op_in_unsafe_fn)]
pub mod observation;
pub mod protocol;

pub const VERSION: u32 = 0x0880_0003;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(any(test, all(windows, target_arch = "x86_64")))]
enum LoaderAction {
    Ignore,
    Install,
    Rebuild,
}

/// A retained XP2 module does not identify what an unrelated loader is doing.
/// Only Closed+the verified caller may reopen; initial preparation stays strict.
#[cfg(any(test, all(windows, target_arch = "x86_64")))]
fn loader_action(
    phase: Phase,
    result: usize,
    caller: Option<u64>,
) -> Result<LoaderAction, &'static str> {
    if result & 0xff == 0 || !matches!(phase, Phase::Prepared | Phase::Closed) {
        return Ok(LoaderAction::Ignore);
    }
    match (phase, caller) {
        (Phase::Closed, Some(0x24b52d)) => Ok(LoaderAction::Rebuild),
        (Phase::Closed, _) => Ok(LoaderAction::Ignore),
        (Phase::Prepared, Some(0x24b52d)) => Ok(LoaderAction::Install),
        _ => Err("initial loader caller differs before factory"),
    }
}

/// Values read at the pinned loader return, while STATE holds the transition.
/// Zero context/holder fields describe this boundary, not AI job quiescence.
#[cfg(any(test, all(windows, target_arch = "x86_64")))]
struct ReloadBoundary {
    loader_result: usize,
    caller: u64,
    close_tid: u32,
    current_tid: u32,
    pinned_core: usize,
    current_core: usize,
    context: usize,
    holder: usize,
    held_context: usize,
}

/// Reopen before factory/RESET, never at the later SetContext. The injected
/// action is the only Rebuild call site for reload; failure retains Closed.
#[cfg(any(test, all(windows, target_arch = "x86_64")))]
fn rebuild_at_loader(
    phase: &mut Phase,
    boundary: ReloadBoundary,
    rebuild: impl FnOnce() -> u32,
) -> Result<Option<u32>, &'static str> {
    if *phase != Phase::Closed || boundary.loader_result & 0xff == 0 {
        return Ok(None);
    }
    if boundary.caller != 0x24b52d
        || boundary.close_tid == 0
        || boundary.current_tid != boundary.close_tid
        || boundary.pinned_core == 0
        || boundary.current_core != boundary.pinned_core
        || boundary.context != 0
        || boundary.holder == 0
        || boundary.held_context != 0
    {
        return Err("reload boundary differs before factory");
    }
    let status = rebuild();
    if status == 0 {
        *phase = Phase::Open;
    }
    Ok(Some(status))
}

#[cfg(all(windows, target_arch = "x86_64"))]
mod windows;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_abi_is_exact() {
        assert_eq!(std::mem::size_of::<Prepare>(), 72);
        assert_eq!(std::mem::offset_of!(Prepare, expected_menu_context), 64);
        assert_eq!(VERSION, 0x0880_0003);
        assert_eq!(std::mem::size_of::<RuntimeConfig>(), 40);
        assert_eq!(std::mem::align_of::<RuntimeConfig>(), 8);
        assert_eq!(std::mem::offset_of!(RuntimeConfig, trace_address), 24);
        assert_eq!(std::mem::offset_of!(RuntimeConfig, mode), 36);
    }

    fn boundary() -> ReloadBoundary {
        // Symbolic test values; never passed to a native API or called receipts.
        ReloadBoundary {
            loader_result: 1,
            caller: 0x24b52d,
            close_tid: 7,
            current_tid: 7,
            pinned_core: 8,
            current_core: 8,
            context: 0,
            holder: 9,
            held_context: 0,
        }
    }

    #[test]
    fn reload_rebuild_precedes_factory_state_and_is_not_repeated_at_set() {
        let mut phase = Phase::Closed;
        let mut catalog = vec!["old world"];
        let mut rebuild_calls = 0;
        assert_eq!(
            rebuild_at_loader(&mut phase, boundary(), || {
                rebuild_calls += 1;
                catalog.clear();
                0
            }),
            Ok(Some(0))
        );
        assert_eq!(phase, Phase::Open);
        // The factory then populates fresh values. A late/double Rebuild would
        // erase these values, which is the actual regression being prevented.
        catalog.push("new factory RESET/COMMIT");
        assert_eq!(
            rebuild_at_loader(&mut phase, boundary(), || {
                rebuild_calls += 1;
                catalog.clear();
                0
            }),
            Ok(None)
        );
        assert_eq!(rebuild_calls, 1);
        assert_eq!(catalog, ["new factory RESET/COMMIT"]);
    }

    #[test]
    fn loader_failure_keeps_closed_until_one_successful_rebuild() {
        let mut phase = Phase::Closed;
        let mut calls = 0;
        for _ in 0..2 {
            let mut failed = boundary();
            failed.loader_result = 0x100; // ABI result is AL, not full RAX.
            assert_eq!(
                rebuild_at_loader(&mut phase, failed, || {
                    calls += 1;
                    0
                }),
                Ok(None)
            );
            assert_eq!(phase, Phase::Closed);
        }
        assert_eq!(
            rebuild_at_loader(&mut phase, boundary(), || {
                calls += 1;
                0
            }),
            Ok(Some(0))
        );
        assert_eq!(calls, 1); // Positive control exercises the same injected action.
        assert_eq!(phase, Phase::Open);
    }

    #[test]
    fn invalid_boundary_never_rebuilds_and_api_failure_stays_closed() {
        for index in 0..8 {
            let mut b = boundary();
            match index {
                0 => b.caller = 0x24b5d5, // the old late SetContext location
                1 => b.close_tid = 0,
                2 => b.current_tid = 8,
                3 => b.pinned_core = 0,
                4 => b.current_core = 10,
                5 => b.context = 11,
                6 => b.holder = 0,
                _ => b.held_context = 12,
            }
            let mut phase = Phase::Closed;
            assert!(rebuild_at_loader(&mut phase, b, || panic!("unexpected Rebuild")).is_err());
            assert_eq!(phase, Phase::Closed);
        }
        let mut phase = Phase::Closed;
        assert_eq!(rebuild_at_loader(&mut phase, boundary(), || 5), Ok(Some(5)));
        assert_eq!(phase, Phase::Closed);
    }

    #[test]
    fn initial_menu_or_incomplete_close_cannot_use_reload_action() {
        for mut phase in [
            Phase::Prepared,
            Phase::Open,
            Phase::Closing,
            Phase::UnsupportedInlineExchange,
        ] {
            assert_eq!(
                rebuild_at_loader(&mut phase, boundary(), || panic!("unexpected Rebuild")),
                Ok(None)
            );
        }
    }

    #[test]
    fn unrelated_loader_is_ignored_while_closed_but_prepare_is_strict() {
        for caller in [None, Some(0x245cea), Some(0x24b5d5)] {
            assert_eq!(
                loader_action(Phase::Closed, 1, caller),
                Ok(LoaderAction::Ignore)
            );
            assert!(loader_action(Phase::Prepared, 1, caller).is_err());
        }
        for phase in [Phase::Prepared, Phase::Closed] {
            assert_eq!(
                loader_action(phase, 0x100, Some(0x24b52d)),
                Ok(LoaderAction::Ignore)
            );
        }
        assert_eq!(
            loader_action(Phase::Closed, 1, Some(0x24b52d)),
            Ok(LoaderAction::Rebuild)
        );
        assert_eq!(
            loader_action(Phase::Prepared, 1, Some(0x24b52d)),
            Ok(LoaderAction::Install)
        );
        // Loader result re-entry after the first success cannot reopen again.
        assert_eq!(
            loader_action(Phase::Open, 1, Some(0x24b52d)),
            Ok(LoaderAction::Ignore)
        );
    }
}
