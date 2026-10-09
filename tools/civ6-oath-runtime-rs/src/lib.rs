//! Oath Apply/Remove completion observer, split out of the native bridge.
//!
//! What is here: the family constants and record types (portable), a finite
//! observation buffer, two Win64 gateways that call the original wrapper
//! exactly once and copy its inputs and full RAX, and bounded diagnostic
//! exports guarded by the current game's identity.
//!
//! Portable contribution bookkeeping, save-value encoding and quantity
//! arithmetic are also provided. Their inputs still require a separately
//! established native holding/lifecycle contract. There is no live quantity
//! hook, official property accessor, Lua registration or product bootstrap.
//! The diagnostic export requires fixed process creation,
//! complete image SHA-256, loaded module and path preflight before publishing
//! any gateway. Driver ownership/ruleset checks are an additional gate. The low-level
//! Win64 ABI, SEH and global_asm stay in Rust because the gateway frames and
//! the trampoline unwind data must match the native unwinder exactly.

pub mod call_identity;
pub mod contribution_ledger;
pub mod contribution_save;
pub mod oath_install_state;
pub mod oath_record;
pub mod preflight;
pub mod production_copy;
pub mod quantity;

#[cfg(windows)]
mod identity_windows;
#[cfg(windows)]
mod native;
#[cfg(windows)]
mod oath_trace;
#[cfg(windows)]
pub mod recipient_trace;
#[cfg(windows)]
pub mod runtime;

#[cfg(windows)]
pub use oath_trace::OathTrace;
