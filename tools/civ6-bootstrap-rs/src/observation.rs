//! A one-shot observation closes admission and drains accepted writers.
//! This gates telemetry only; it must never gate gameplay authority.
use std::sync::atomic::{AtomicU32, Ordering};

const CLOSED: u32 = 1 << 31;

pub struct ObservationGate(AtomicU32);

impl ObservationGate {
    pub const fn new(open: bool) -> Self {
        Self(AtomicU32::new(if open { 0 } else { CLOSED }))
    }

    /// Initialize once, before making the owning install visible to stoppers.
    /// An installed observation is never restarted or redirected.
    pub fn initialize_before_publish(&self) {
        self.0.store(0, Ordering::Release);
    }

    /// A true result owns one writer until `release`, even if close races it.
    pub fn try_acquire(&self) -> bool {
        self.0
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| {
                (state < CLOSED - 1).then_some(state + 1)
            })
            .is_ok()
    }

    /// Call exactly once for a successful acquisition, after the final write.
    pub fn release(&self) {
        self.0.fetch_sub(1, Ordering::Release);
    }

    pub fn close(&self) {
        self.0.fetch_or(CLOSED, Ordering::AcqRel);
    }

    pub fn is_closed(&self) -> bool {
        self.0.load(Ordering::Acquire) & CLOSED != 0
    }

    pub fn drained(&self) -> bool {
        self.0.load(Ordering::Acquire) == CLOSED
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    #[test]
    fn close_rejects_late_writers_and_waits_for_prior_writes() {
        let gate = Arc::new(ObservationGate::new(true));
        let value = Arc::new(AtomicU32::new(0));
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker_gate = gate.clone();
        let worker_value = value.clone();
        let worker = std::thread::spawn(move || {
            assert!(worker_gate.try_acquire());
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            worker_value.store(17, Ordering::Relaxed);
            worker_gate.release();
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        gate.close();
        let closed = gate.is_closed();
        let pending = !gate.drained();
        let rejects_late = !gate.try_acquire();
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        assert!(closed && pending && rejects_late);
        assert!(gate.drained());
        assert_eq!(value.load(Ordering::Relaxed), 17);
        gate.close();
        assert!(gate.drained());
        assert!(!gate.try_acquire());
    }

    #[test]
    fn unpublished_gate_is_closed_until_initialization() {
        let gate = ObservationGate::new(false);
        assert!(gate.drained());
        assert!(!gate.try_acquire());
        gate.initialize_before_publish();
        assert!(gate.try_acquire());
        gate.close();
        assert!(!gate.drained());
        gate.release();
        assert!(gate.drained());
    }
}
