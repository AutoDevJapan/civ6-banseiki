//! Reports over archived Civ6 artifacts, independent of the live driver.

pub mod arbitration;
pub mod persona_full;
pub mod persona_unjoined;
pub mod research;
pub mod scenario;
pub mod tuning;

pub mod cli;
mod common;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
