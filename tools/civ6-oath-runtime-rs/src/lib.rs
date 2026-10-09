//! Oath Apply/Remove completion observer, split out of the native bridge.
//!
//! What is here: the family constants and record types (portable), a finite
//! observation buffer, two Win64 gateways that call the original wrapper
//! exactly once and copy its inputs and full RAX, and a synthetic-target
//! installer used only by the Win64 tests.
//!
//! What is deliberately not here: any link to payment, rates, quantities, Lua
//! or persona state, and any public Install. The installer is crate-private
//! and unreachable from the DLL; a public Install needs fixed process,
//! FILETIME, image SHA-256, module and path preflight first. The low-level
//! Win64 ABI, SEH and global_asm stay in Rust because the gateway frames and
//! the trampoline unwind data must match the native unwinder exactly.

pub mod oath_install_state;
pub mod oath_record;
pub mod preflight;

#[cfg(windows)]
mod native;
#[cfg(windows)]
mod oath_trace;
#[cfg(windows)]
pub mod runtime;

#[cfg(windows)]
pub use oath_trace::OathTrace;
