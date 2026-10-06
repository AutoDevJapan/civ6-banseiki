//! A producer admission is acquired before the original 4012F0 instruction,
//! hence before 401510 fetches a callable. Closing and acquiring share one CAS
//! word; an entry snapshot can never be upgraded to a later generation.
//! This does not authorize payment. The existing owner/ledger fence still does.
use std::{
    cell::Cell,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const OPEN: u64 = 1 << 63;
const READERS: u64 = u32::MAX as u64;
const GENERATION: u64 = 0x7fff_ffff_0000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Closed,
    Stale,
    Busy,
    Reentrant,
    Exhausted,
    Timeout,
}

#[repr(transparent)]
pub struct Producer {
    state: AtomicU64,
}

impl Producer {
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
        }
    }

    /// Only the owning runtime calls this, with its newly advanced generation,
    /// after closing/draining the previous registration and rebuilding its data.
    pub fn rebuild(&self, generation: u32) -> Result<(), Error> {
        if generation == 0 || generation >= (1 << 31) {
            return Err(Error::Exhausted);
        }
        let previous = self.state.load(Ordering::Acquire);
        if previous & (OPEN | READERS) != 0 {
            return Err(Error::Busy);
        }
        if u64::from(generation) << 32 <= previous & GENERATION {
            return Err(Error::Stale);
        }
        self.state
            .compare_exchange(
                previous,
                OPEN | (u64::from(generation) << 32),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(|_| ())
            .map_err(|_| Error::Busy)
    }

    pub fn snapshot(&self) -> u64 {
        self.state.load(Ordering::Acquire)
    }

    fn acquire(&self, snapshot: u64) -> Result<u32, Error> {
        if snapshot & OPEN == 0 {
            return Err(Error::Closed);
        }
        let epoch = snapshot & (OPEN | GENERATION);
        self.state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |now| {
                (now & (OPEN | GENERATION) == epoch && now & READERS != READERS).then(|| now + 1)
            })
            .map(|_| (snapshot >> 32) as u32 & 0x7fff_ffff)
            .map_err(|_| Error::Stale)
    }

    fn release(&self) {
        self.state.fetch_sub(1, Ordering::Release);
    }

    /// Closing linearizes before the wait. Reentrant teardown cannot wait for
    /// itself and is refused even if no added payment operation is in flight.
    pub fn close(&self) -> Result<(), Error> {
        self.state.fetch_and(!OPEN, Ordering::AcqRel);
        if CURRENT.with(|c| {
            c.get()
                .is_some_and(|r| r.producer == self as *const _ as usize)
        }) {
            return Err(Error::Reentrant);
        }
        let until = Instant::now() + Duration::from_secs(5);
        while self.state.load(Ordering::Acquire) & READERS != 0 {
            if Instant::now() >= until {
                return Err(Error::Timeout);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    pub fn closed_and_drained(&self) -> bool {
        self.snapshot() & (OPEN | READERS) == 0
    }
}

#[derive(Clone, Copy)]
struct Reader {
    producer: usize,
    generation: u32,
    player: u64,
}
thread_local! { static CURRENT: Cell<Option<Reader>> = const { Cell::new(None) }; }

/// An actual machine entry takes the snapshot before entering the recorder.
/// No public boolean/permit can create a reader or substitute for a caller.
pub(super) fn enter(producer: &'static Producer, snapshot: u64, player: u64) -> Result<u32, Error> {
    if CURRENT.with(Cell::get).is_some() {
        return Err(Error::Reentrant);
    }
    let generation = producer.acquire(snapshot)?;
    CURRENT.with(|c| {
        c.set(Some(Reader {
            producer: producer as *const _ as usize,
            generation,
            player,
        }))
    });
    Ok(generation)
}

pub(super) fn leave() {
    if let Some(reader) = CURRENT.with(|c| c.replace(None)) {
        // Producers are static and outlive all normal/SEH returns.
        unsafe { &*(reader.producer as *const Producer) }.release();
    }
}

pub(super) fn current_generation() -> Option<u32> {
    CURRENT.with(|c| c.get().map(|r| r.generation))
}
pub(super) fn current_player() -> Option<u64> {
    CURRENT.with(|c| c.get().map(|r| r.player))
}

impl Default for Producer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entry_snapshot_cannot_borrow_the_rebuilt_generation() {
        let producer = Box::leak(Box::new(Producer::new()));
        producer.rebuild(1).unwrap();
        let pending = producer.snapshot();
        producer.close().unwrap();
        producer.rebuild(2).unwrap();
        assert_eq!(enter(producer, pending, 0x1000), Err(Error::Stale));
        assert_eq!(enter(producer, producer.snapshot(), 0x1000), Ok(2));
        assert_eq!(producer.close(), Err(Error::Reentrant));
        leave();
        producer.close().unwrap();
        assert!(producer.closed_and_drained());
    }
}
