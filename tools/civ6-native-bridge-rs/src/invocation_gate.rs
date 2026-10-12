//! Bridge-owned synchronous invocations, independent of observation and engine jobs.
use std::sync::atomic::{AtomicU64, Ordering};

const CLOSED: u64 = 1 << 63;
const COUNT: u64 = u32::MAX as u64;
const MAX_GENERATION: u32 = i32::MAX as u32;

pub struct InvocationGate(AtomicU64);

impl Default for InvocationGate {
    fn default() -> Self {
        Self::new()
    }
}

impl InvocationGate {
    pub const fn new() -> Self {
        Self(AtomicU64::new(CLOSED))
    }

    /// Only a closed, drained generation can be opened. Never erase readers.
    pub fn open_next(&self) -> Option<u32> {
        let old = self.0.load(Ordering::Acquire);
        if old & (CLOSED | COUNT) != CLOSED {
            return None;
        }
        let generation = ((old >> 32) as u32 & MAX_GENERATION).checked_add(1)?;
        if generation > MAX_GENERATION {
            return None;
        }
        self.0
            .compare_exchange(
                old,
                u64::from(generation) << 32,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .ok()
            .map(|_| generation)
    }

    /// Reserve before the first bridge read. This generation stays invocation-local.
    pub fn enter(&self) -> Option<u32> {
        self.0
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                (old & CLOSED == 0 && old & COUNT < COUNT).then(|| old + 1)
            })
            .ok()
            .map(|old| (old >> 32) as u32)
    }

    /// A stale or duplicate release cannot settle a different generation.
    pub fn leave(&self, generation: u32) -> bool {
        self.0
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                (generation != 0
                    && ((old >> 32) as u32 & MAX_GENERATION) == generation
                    && old & COUNT != 0)
                    .then_some(old.wrapping_sub(1))
            })
            .is_ok()
    }

    pub fn close(&self) {
        self.0.fetch_or(CLOSED, Ordering::AcqRel);
    }
    pub fn drained(&self) -> bool {
        self.0.load(Ordering::Acquire) & (CLOSED | COUNT) == CLOSED
    }
    #[cfg(test)]
    pub(crate) fn pending_for_test(&self) -> u32 {
        (self.0.load(Ordering::Acquire) & COUNT) as u32
    }
    pub fn current(&self, generation: u32) -> bool {
        let state = self.0.load(Ordering::Acquire);
        state & CLOSED == 0 && (state >> 32) as u32 == generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};
    #[test]
    fn close_waits_for_the_final_read_and_does_not_reopen_over_it() {
        let gate = Arc::new(InvocationGate::new());
        let generation = gate.open_next().unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let reader = gate.clone();
        let work = std::thread::spawn(move || {
            let accepted = reader.enter().unwrap();
            entered_tx.send(accepted).unwrap();
            finish_rx.recv().unwrap();
            reader.leave(accepted)
        });
        let observed_generation = entered_rx.recv().unwrap();
        gate.close();
        let late = gate.enter();
        let before_drain = gate.drained();
        let before_open = gate.open_next();
        finish_tx.send(()).unwrap();
        let released = work.join().unwrap();
        assert_eq!(observed_generation, generation);
        assert!(released);
        assert!(late.is_none() && !before_drain && before_open.is_none());
        assert!(gate.drained());
        assert_eq!(gate.open_next(), Some(generation + 1));
        assert!(!gate.current(generation));
    }
    #[test]
    fn stale_release_cannot_settle_new_generation_or_duplicate() {
        let gate = InvocationGate::new();
        let old = gate.open_next().unwrap();
        assert_eq!(gate.enter(), Some(old));
        gate.close();
        assert!(gate.leave(old));
        assert!(!gate.leave(old));
        let next = gate.open_next().unwrap();
        assert_eq!(gate.enter(), Some(next));
        assert!(!gate.leave(old));
        gate.close();
        assert!(!gate.drained());
        assert!(gate.leave(next));
        assert!(gate.drained());
    }
    #[test]
    fn count_and_generation_exhaustion_do_not_wrap_into_open_state() {
        let gate = InvocationGate(AtomicU64::new((1 << 32) | COUNT));
        assert!(gate.enter().is_none());
        let gate = InvocationGate(AtomicU64::new(CLOSED | (u64::from(MAX_GENERATION) << 32)));
        assert!(gate.open_next().is_none());
        assert!(gate.enter().is_none());
        let gate = InvocationGate(AtomicU64::new(u64::MAX));
        assert!(gate.enter().is_none());
    }
}
