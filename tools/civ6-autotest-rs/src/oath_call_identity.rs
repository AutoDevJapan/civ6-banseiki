//! Shared invocation-local model used by the native callback and its driver.
//! Matching copies do not establish ABA protection or city lifetime.
pub use civ6_oath_runtime_rs::call_identity::*;
