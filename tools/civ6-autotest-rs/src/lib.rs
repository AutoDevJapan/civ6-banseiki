//! Civ VI scenario driver, with platform-independent protocol and artifact handling.
pub mod capture;
pub mod checkpoint;
pub mod click;
pub mod error;
pub mod judge;
pub mod live_lease;
pub mod metrics;
pub mod native_bridge;
pub mod native_trace;
pub mod payment_scenario;
pub mod persona_payment;
pub mod restart;
pub mod runner;
pub mod scenario;
pub mod spy_failures;
pub mod startup;
pub mod startup_idle;
pub mod startup_screen;
pub mod time;
pub mod tuner;
pub use error::{Error, ErrorKind, Result};

pub fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
