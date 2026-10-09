//! Civ VI scenario driver, with platform-independent protocol and artifact handling.
pub mod capture;
mod carrier_mp;
pub mod carrier_sync;
pub mod carrier_transport;
pub mod click;
pub mod error;
pub mod growth_roster;
pub mod growth_sync;
pub mod hotseat_bridge;
pub mod judge;
pub mod metrics;
mod native_inventory;
pub mod native_carrier_witness;
pub mod reliability;
pub mod restart;
pub mod runner;
pub mod save_isolation;
pub mod scenario;
pub mod session;
pub mod startup;
pub mod startup_idle;
pub mod startup_screen;
pub mod steam_dialog;
pub mod steam_policy;
pub mod steam_sync;
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
