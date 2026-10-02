//! Final migration gates. Run with:
//! `cargo test --manifest-path tools/Cargo.toml -p civ6-migration-tests-rs`.
//! These tests inspect Rust syntax and the Git index; they never run Cargo.
//! The resolver supports the ported suites' module and simple_tests dialect;
//! unsupported macro import/export scopes fail closed.

#[cfg(test)]
mod inventory;
#[cfg(test)]
mod tests;
