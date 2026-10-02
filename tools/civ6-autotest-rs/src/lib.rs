//! Civ VI scenario driver, with platform-independent protocol and artifact handling.
pub mod error;
pub mod time;
pub mod tuner;
pub mod judge;
pub mod metrics;
pub mod click;
pub mod capture;
pub mod restart;
pub use error::{Error, ErrorKind, Result};

pub fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}
