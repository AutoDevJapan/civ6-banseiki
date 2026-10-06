//! One immutable subject, one terminal attempt. Observations cannot arm it.
//! Missing lifecycle evidence is a production blocker, never an input boolean.
use std::sync::atomic::{AtomicU32, Ordering};

pub const MISSING_INITIAL_INSTALL: u32 = 1;
pub const MISSING_NATIVE_WAIT: u32 = 2;
pub const MISSING_CALLBACK_EXIT: u32 = 4;
pub const EXECUTION_BLOCKERS: u32 =
    MISSING_INITIAL_INSTALL | MISSING_NATIVE_WAIT | MISSING_CALLBACK_EXIT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Subject {
    pub pid: u32,
    pub created: u64,
    pub owner: u32,
    pub generation: u32,
    pub index: u32,
    pub cost: u32,
    pub paid: u32,
    pub spent: u32,
    pub title: u32,
}
impl Subject {
    pub fn valid(self) -> bool {
        self.pid != 0
            && self.created != 0
            && self.owner < 64
            && self.generation != 0
            && self.index < 256
            && (2..=3).contains(&self.cost)
            && self.paid >= 1
            && self.paid < self.cost
            && self.spent >= self.paid
            && self.title < 4096
    }
}
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Observing = 0,
    Armed = 1,
    Claimed = 2,
    Verified = 3,
    Unknown = 4,
    Mismatch = 5,
    Stopped = 6,
}
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject {
    None = 0,
    Lifecycle = 1,
    Identity = 2,
    Receipt = 3,
    AlreadyUsed = 4,
}

pub struct Once {
    pub subject: Subject,
    state: AtomicU32,
    reason: AtomicU32,
    callbacks: AtomicU32,
}
impl Once {
    pub fn observing(subject: Subject) -> Result<Self, Reject> {
        if !subject.valid() {
            return Err(Reject::Identity);
        }
        Ok(Self {
            subject,
            state: AtomicU32::new(State::Observing as u32),
            reason: AtomicU32::new(0),
            callbacks: AtomicU32::new(0),
        })
    }
    pub fn state(&self) -> u32 {
        self.state.load(Ordering::Acquire)
    }
    pub fn reason(&self) -> u32 {
        self.reason.load(Ordering::Acquire)
    }
    pub fn callbacks(&self) -> u32 {
        self.callbacks.load(Ordering::Acquire)
    }
    pub fn observe(&self, pid: u32, created: u64, owner: u32, generation: u32, index: u32) {
        self.callbacks
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
            .ok();
        let s = self.subject;
        if (pid, created, owner, generation, index)
            != (s.pid, s.created, s.owner, s.generation, s.index)
        {
            self.reject(Reject::Identity);
        }
    }
    fn reject(&self, reason: Reject) {
        self.reason.store(reason as u32, Ordering::Release);
        self.state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |s| {
                (s == State::Observing as u32 || s == State::Armed as u32)
                    .then_some(State::Mismatch as u32)
            })
            .ok();
    }
    pub fn check_paid(&self, paid: Option<u32>) {
        if paid != Some(self.subject.paid) {
            self.reject(Reject::Receipt);
        }
    }
    pub fn check_spent(&self, spent: u64) {
        if spent != u64::from(self.subject.spent) {
            self.reject(Reject::Receipt);
        }
    }
    /// There is deliberately no production arm setter. A future real receipt
    /// adapter must replace this rejection; flags/config/JSON cannot do so.
    pub fn request_execution(&self) -> Result<(), Reject> {
        self.reason
            .store(Reject::Lifecycle as u32, Ordering::Release);
        Err(Reject::Lifecycle)
    }
    pub fn claim(&self, actual: Subject) -> Result<(), Reject> {
        if actual != self.subject {
            self.reject(Reject::Receipt);
            return Err(Reject::Receipt);
        }
        self.state
            .compare_exchange(
                State::Armed as u32,
                State::Claimed as u32,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(|_| ())
            .map_err(|_| Reject::AlreadyUsed)
    }
    pub fn finish(&self, verified: bool) {
        self.state
            .compare_exchange(
                State::Claimed as u32,
                if verified {
                    State::Verified as u32
                } else {
                    State::Unknown as u32
                },
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .ok();
    }
    pub fn stop(&self) {
        self.state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |s| match s {
                0 | 1 => Some(State::Stopped as u32),
                2 => Some(State::Unknown as u32),
                _ => None,
            })
            .ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn subject() -> Subject {
        Subject {
            pid: 1,
            created: 2,
            owner: 3,
            generation: 4,
            index: 5,
            cost: 3,
            paid: 1,
            spent: 7,
            title: 8,
        }
    }
    #[test]
    fn observation_and_execution_request_never_manufacture_a_permit() {
        let once = Once::observing(subject()).unwrap();
        for _ in 0..10 {
            once.observe(1, 2, 3, 4, 5);
        }
        assert_eq!(once.request_execution(), Err(Reject::Lifecycle));
        assert_eq!(once.state(), State::Observing as u32);
        assert!(once.claim(subject()).is_err());
        assert_eq!(once.callbacks(), 10);
    }
    #[test]
    fn actual_atomic_claim_is_once_and_unknown_never_rearms() {
        let once = std::sync::Arc::new(Once::observing(subject()).unwrap());
        // Explicit atomic-state fixture, not a real GameCore lifecycle receipt.
        once.state.store(State::Armed as u32, Ordering::Release);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let once = once.clone();
                let b = barrier.clone();
                std::thread::spawn(move || {
                    b.wait();
                    once.claim(subject()).is_ok()
                })
            })
            .collect();
        assert_eq!(
            threads
                .into_iter()
                .map(|t| usize::from(t.join().unwrap()))
                .sum::<usize>(),
            1
        );
        once.finish(false);
        assert_eq!(once.state(), State::Unknown as u32);
        assert!(once.claim(subject()).is_err());
        once.stop();
        assert_eq!(once.state(), State::Unknown as u32);
    }
    #[test]
    fn every_subject_and_paid_field_is_part_of_the_single_operation_identity() {
        for i in 0..9 {
            let once = Once::observing(subject()).unwrap();
            once.state.store(State::Armed as u32, Ordering::Release);
            let mut s = subject();
            match i {
                0 => s.pid += 1,
                1 => s.created += 1,
                2 => s.owner += 1,
                3 => s.generation += 1,
                4 => s.index += 1,
                5 => s.cost = 2,
                6 => s.paid += 1,
                7 => s.spent += 1,
                _ => s.title += 1,
            };
            assert_eq!(once.claim(s), Err(Reject::Receipt));
            assert!(once.claim(subject()).is_err());
        }
    }
    #[test]
    fn readonly_paid_mismatch_and_stop_are_terminal_without_dispatch() {
        for paid in [None, Some(0), Some(2)] {
            let once = Once::observing(subject()).unwrap();
            once.check_paid(paid);
            assert_eq!(once.state(), State::Mismatch as u32);
            assert_eq!(once.reason(), Reject::Receipt as u32);
            assert!(once.claim(subject()).is_err());
        }
        let once = Once::observing(subject()).unwrap();
        once.check_spent(8);
        assert_eq!(once.state(), State::Mismatch as u32);
        let once = Once::observing(subject()).unwrap();
        once.stop();
        assert_eq!(once.state(), State::Stopped as u32);
        assert!(once.claim(subject()).is_err());
    }
}
