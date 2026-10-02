//! Final migration gates. Run with:
//! `cargo test --manifest-path tools/Cargo.toml -p civ6-migration-tests-rs`.
//! These tests inspect Rust syntax and the Git index; they never run Cargo.

#[cfg(test)]
mod inventory;
#[cfg(test)]
mod tests;
