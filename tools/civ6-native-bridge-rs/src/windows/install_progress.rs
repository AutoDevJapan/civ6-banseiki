//! Five copied words for one bounded installation diagnostic, not authority.
use super::*;
#[unsafe(no_mangle)]
pub static BRIDGE_RUNTIME_INSTALL_PROGRESS:[AtomicU32;5]=[
    AtomicU32::new(1),
    AtomicU32::new((cfg!(feature="ai-observation-candidate") as u32) | ((cfg!(feature="ai-producer-candidate") as u32)<<1) | ((cfg!(debug_assertions) as u32)<<2)),
    AtomicU32::new(0),AtomicU32::new(0),AtomicU32::new(u32::MAX),
];
pub(super) fn enter() {
    BRIDGE_RUNTIME_INSTALL_PROGRESS[3].store(unsafe {GetCurrentThreadId()},Ordering::Release);
    stage(1);
}
pub(super) fn stage(value:u32) {BRIDGE_RUNTIME_INSTALL_PROGRESS[2].store(value,Ordering::Release);}
pub(super) fn returned(value:u32)->u32 {BRIDGE_RUNTIME_INSTALL_PROGRESS[4].store(value,Ordering::Release);value}
