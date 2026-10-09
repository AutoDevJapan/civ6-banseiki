//! Accounting for an entire native caller and the work it issued.
//!
//! An adapter must enter at the outermost native frame, before fetching work,
//! and retain `Caller` until that frame's normal return or SEH termination.
//! Inner callbacks borrow that caller; they must never enter a newer world.
//! This module does not establish native entry coverage, stop the game's
//! producers, publish a native list, or perform its Wait. A drained accounting
//! result is not a native shutdown certificate. No runtime enables it yet.
use std::{
    collections::HashMap,
    marker::PhantomData,
    rc::Rc,
    sync::{Condvar, Mutex, MutexGuard},
    thread::{self, ThreadId},
    time::Instant,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Closed,
    Busy,
    Stale,
    Reentrant,
    Exhausted,
    Unresolved,
    Poisoned,
    Timeout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Open,
    Closing,
    Drained,
}

struct State {
    generation: u64,
    phase: Phase,
    callers: HashMap<ThreadId, usize>,
    dispatched: usize,
    pending: usize,
    published: usize,
    unresolved: bool,
}

/// One process-lifetime tracker. The mutex orders entry, issue, close and
/// rebuild together; zero jobs between callbacks cannot imply caller exit.
pub struct WorldFlights {
    state: Mutex<State>,
    changed: Condvar,
}

impl WorldFlights {
    pub fn new(generation: u64) -> Result<Self, Error> {
        if generation == 0 {
            return Err(Error::Exhausted);
        }
        Ok(Self {
            state: Mutex::new(State {
                generation,
                phase: Phase::Open,
                callers: HashMap::new(),
                dispatched: 0,
                pending: 0,
                published: 0,
                unresolved: false,
            }),
            changed: Condvar::new(),
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>, Error> {
        self.state.lock().map_err(|_| Error::Poisoned)
    }

    /// Reserve at the original dispatch, BEFORE handing work to native code.
    /// A delayed outer entry consumes this value, never a fresh generation
    /// snapshot. The native adapter must carry it with the actual work; this
    /// module does not discover native job identity or its lifetime.
    pub fn dispatch(&self) -> Result<Dispatched<'_>, Error> {
        let mut s = self.lock()?;
        if s.unresolved {
            return Err(Error::Unresolved);
        }
        if s.phase != Phase::Open {
            return Err(Error::Closed);
        }
        s.dispatched = s.dispatched.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(Dispatched {
            world: self,
            generation: s.generation,
            settled: false,
        })
    }

    /// Reject new outer frames, but let previously admitted outer frames
    /// finish their original work. Their pending issuance remains old-world
    /// work even when it becomes visible after this seal.
    pub fn close(&self) -> Result<Closing<'_>, Error> {
        let mut s = self.lock()?;
        if s.phase == Phase::Open {
            s.phase = Phase::Closing;
        }
        self.changed.notify_all();
        if s.callers.contains_key(&thread::current().id()) {
            return Err(Error::Reentrant);
        }
        if s.unresolved {
            return Err(Error::Unresolved);
        }
        Ok(Closing {
            world: self,
            generation: s.generation,
        })
    }

    /// Only an accounting drain obtained from this instance and generation
    /// may advance it. Native shutdown/receiver validation is still external.
    pub fn rebuild(&self, drained: Drained<'_>) -> Result<u64, Error> {
        if !std::ptr::eq(self, drained.world) {
            return Err(Error::Stale);
        }
        let mut s = self.lock()?;
        if s.generation != drained.generation {
            return Err(Error::Stale);
        }
        if s.unresolved {
            return Err(Error::Unresolved);
        }
        if s.phase != Phase::Drained
            || !s.callers.is_empty()
            || s.dispatched != 0
            || s.pending != 0
            || s.published != 0
        {
            return Err(Error::Busy);
        }
        let next = s.generation.checked_add(1).ok_or(Error::Exhausted)?;
        s.generation = next;
        s.phase = Phase::Open;
        Ok(next)
    }
}

/// A not-yet-entered outer dispatch. It may move to the native worker's thread
/// and still counts in every gap before that worker's first entry. Not Clone:
/// no scalar/current-generation value can replace its reservation.
pub struct Dispatched<'a> {
    world: &'a WorldFlights,
    generation: u64,
    settled: bool,
}

impl<'a> Dispatched<'a> {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// A dispatch delayed past the seal keeps its original generation and
    /// normal native execution. It must finish before old-world destruction.
    pub fn enter(mut self) -> Result<Caller<'a>, Error> {
        let mut s = self.world.lock()?;
        if s.generation != self.generation {
            return Err(Error::Stale);
        }
        if s.unresolved {
            return Err(Error::Unresolved);
        }
        let tid = thread::current().id();
        let count = s.callers.entry(tid).or_default();
        *count = count.checked_add(1).ok_or(Error::Exhausted)?;
        s.dispatched -= 1;
        self.settled = true;
        Ok(Caller {
            world: self.world,
            generation: self.generation,
            thread: tid,
            // A native stack frame may not migrate to another OS thread.
            _thread_bound: PhantomData,
        })
    }

    /// Only a dispatch proven never handed to native code may be cancelled.
    /// An already queued native frame is not cancelled by dropping its token.
    pub fn cancel_before_dispatch(mut self) -> Result<(), Error> {
        let mut s = self.world.lock()?;
        if s.generation != self.generation {
            return Err(Error::Stale);
        }
        s.dispatched -= 1;
        self.settled = true;
        self.world.changed.notify_all();
        Ok(())
    }
}

impl Drop for Dispatched<'_> {
    fn drop(&mut self) {
        if !self.settled {
            let mut s = self.world.state.lock().unwrap_or_else(|e| e.into_inner());
            s.unresolved = true;
            self.world.changed.notify_all();
        }
    }
}

/// The complete outer frame. Inner/repeated callbacks use `issue` on this
/// same value rather than taking a fresh generation from the tracker.
pub struct Caller<'a> {
    world: &'a WorldFlights,
    generation: u64,
    thread: ThreadId,
    _thread_bound: PhantomData<Rc<()>>,
}

impl<'a> Caller<'a> {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Reserve BEFORE calling native Push. Keep the returned pending value
    /// alive through any publication delay; dropping it is unresolved work.
    pub fn issue(&self) -> Result<Pending<'a>, Error> {
        let mut s = self.world.lock()?;
        if s.generation != self.generation {
            return Err(Error::Stale);
        }
        if s.unresolved {
            return Err(Error::Unresolved);
        }
        s.pending = s.pending.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(Pending {
            world: self.world,
            generation: self.generation,
            settled: false,
        })
    }
}

impl Drop for Caller<'_> {
    fn drop(&mut self) {
        // Even a poisoned lock must retain/settle accounting without panicking
        // across an FFI/SEH cleanup. Public operations still reject poisoning.
        let mut s = self.world.state.lock().unwrap_or_else(|e| e.into_inner());
        if s.generation != self.generation {
            s.unresolved = true;
        } else if let Some(count) = s.callers.get_mut(&self.thread) {
            *count -= 1;
            if *count == 0 {
                s.callers.remove(&self.thread);
            }
        } else {
            s.unresolved = true;
        }
        self.world.changed.notify_all();
    }
}

/// Issued work not yet known to have been published to the original job list.
/// Not Clone: one publication/cancellation owns one reservation.
pub struct Pending<'a> {
    world: &'a WorldFlights,
    generation: u64,
    settled: bool,
}

impl<'a> Pending<'a> {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Call only after the original native publication is established. This
    /// transfers the reservation atomically, without an intermediate zero.
    pub fn publish(mut self) -> Result<Published<'a>, Error> {
        let mut s = self.world.lock()?;
        if s.generation != self.generation {
            return Err(Error::Stale);
        }
        if s.unresolved {
            return Err(Error::Unresolved);
        }
        let count = s.published.checked_add(1).ok_or(Error::Exhausted)?;
        s.pending -= 1;
        s.published = count;
        self.settled = true;
        Ok(Published {
            world: self.world,
            generation: self.generation,
            settled: false,
        })
    }

    /// Use only when native submission has not occurred (or native rollback
    /// has proved it absent). A timeout/unknown native result is not rollback.
    pub fn cancel_before_submission(mut self) -> Result<(), Error> {
        let mut s = self.world.lock()?;
        if s.generation != self.generation {
            return Err(Error::Stale);
        }
        s.pending -= 1;
        self.settled = true;
        self.world.changed.notify_all();
        Ok(())
    }
}

/// Published native work may outlive the caller that issued it. Native Wait
/// returning on a different set, or a mere empty list, cannot settle this.
pub struct Published<'a> {
    world: &'a WorldFlights,
    generation: u64,
    settled: bool,
}

impl Published<'_> {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Adapter obligation: the original same-world/same-set completion is
    /// established, including the actual wait path. This is accounting only.
    pub fn complete_after_native_join(mut self) -> Result<(), Error> {
        let mut s = self.world.lock()?;
        if s.generation != self.generation {
            return Err(Error::Stale);
        }
        s.published -= 1;
        self.settled = true;
        self.world.changed.notify_all();
        Ok(())
    }
}

impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if !self.settled {
            let mut s = self.world.state.lock().unwrap_or_else(|e| e.into_inner());
            s.unresolved = true;
            self.world.changed.notify_all();
        }
    }
}

impl Drop for Published<'_> {
    fn drop(&mut self) {
        if !self.settled {
            let mut s = self.world.state.lock().unwrap_or_else(|e| e.into_inner());
            s.unresolved = true;
            self.world.changed.notify_all();
        }
    }
}

pub struct Closing<'a> {
    world: &'a WorldFlights,
    generation: u64,
}

/// A non-Clone accounting result. It deliberately contains no native pointer
/// or execution permit and cannot assert coverage of unregistered callers.
pub struct Drained<'a> {
    world: &'a WorldFlights,
    generation: u64,
}

impl<'a> Closing<'a> {
    /// Wait until ALL old caller frames and their pending/published work have
    /// settled. The deadline starts at the caller, not after lock preparation.
    /// A timeout leaves the seal in place; it does not authorize destruction.
    pub fn wait(self, deadline: Instant) -> Result<Drained<'a>, Error> {
        let mut s = self.world.lock()?;
        loop {
            if s.generation != self.generation {
                return Err(Error::Stale);
            }
            if s.phase == Phase::Open {
                return Err(Error::Busy);
            }
            if s.unresolved {
                return Err(Error::Unresolved);
            }
            if s.callers.contains_key(&thread::current().id()) {
                return Err(Error::Reentrant);
            }
            if s.callers.is_empty() && s.dispatched == 0 && s.pending == 0 && s.published == 0 {
                s.phase = Phase::Drained;
                return Ok(Drained {
                    world: self.world,
                    generation: self.generation,
                });
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err(Error::Timeout);
            };
            let (next, _) = self
                .world
                .changed
                .wait_timeout(s, remaining)
                .map_err(|_| Error::Poisoned)?;
            s = next;
        }
    }
}

#[cfg(test)]
mod tests;
