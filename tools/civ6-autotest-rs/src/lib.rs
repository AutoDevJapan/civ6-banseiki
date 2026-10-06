//! Civ VI scenario driver, with platform-independent protocol and artifact handling.
pub mod capture;
pub mod carrier_sync;
pub mod carrier_transport;
pub mod click;
pub mod error;
pub mod growth_sync;
pub mod hotseat_bridge;
pub mod judge;
pub mod metrics;
pub mod restart;
pub mod runner;
pub mod scenario;
pub mod session;
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
