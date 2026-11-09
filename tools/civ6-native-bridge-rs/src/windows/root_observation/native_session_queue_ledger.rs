//! Serialized slot identities. Physical slot reuse cannot settle an old flight.
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Unknown,
    Busy,
    Stale,
    Unresolved,
    Exhausted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Storage {
    Pending,
    Retained,
    Freeing,
    Freed,
    Cancelling,
    Cancelled,
}
struct Slot<M> {
    queue: u64,
    address: u64,
    message: M,
    producer: bool,
    storage: Storage,
    execution: bool,
}
#[derive(Default)]
pub(super) struct Ledger<M> {
    next: u64,
    slots: HashMap<u64, Slot<M>>,
    by_address: HashMap<(u64, u64), u64>,
    unresolved: bool,
}
impl<M: Copy + Eq> Ledger<M> {
    pub fn new() -> Self {
        Self {
            next: 0,
            slots: HashMap::new(),
            by_address: HashMap::new(),
            unresolved: false,
        }
    }
    pub fn reserve(&mut self, queue: u64, message: M) -> Result<u64, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        if queue == 0 {
            return Err(Error::Stale);
        }
        self.next = self.next.checked_add(1).ok_or(Error::Exhausted)?;
        self.slots.insert(
            self.next,
            Slot {
                queue,
                address: 0,
                message,
                producer: true,
                storage: Storage::Pending,
                execution: false,
            },
        );
        Ok(self.next)
    }
    pub fn bind(&mut self, id: u64, address: u64) -> Result<(), Error> {
        let s = self.slots.get_mut(&id).ok_or(Error::Unknown)?;
        if !s.producer
            || s.address != 0
            || address == 0
            || self.by_address.contains_key(&(s.queue, address))
        {
            return Err(Error::Busy);
        }
        s.address = address;
        self.by_address.insert((s.queue, address), id);
        Ok(())
    }
    pub fn published(&mut self, id: u64) -> Result<(), Error> {
        let s = self.slots.get_mut(&id).ok_or(Error::Unknown)?;
        if !s.producer || s.address == 0 {
            return Err(Error::Stale);
        }
        s.producer = false;
        self.collect(id);
        Ok(())
    }
    pub fn retain(&mut self, queue: u64, address: u64) -> Result<Option<(u64, M)>, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let Some(id) = self.by_address.get(&(queue, address)).copied() else {
            return Ok(None);
        };
        let s = self.slots.get_mut(&id).ok_or(Error::Unknown)?;
        if s.storage != Storage::Pending {
            return Err(Error::Busy);
        }
        s.storage = Storage::Retained;
        s.execution = true;
        Ok(Some((id, s.message)))
    }
    pub fn free_begin(&mut self, id: u64) -> Result<(), Error> {
        let s = self.slots.get_mut(&id).ok_or(Error::Unknown)?;
        if s.storage != Storage::Retained && s.storage != Storage::Cancelling {
            return Err(Error::Stale);
        }
        s.storage = Storage::Freeing;
        // The original free may release/reallocate the same address before
        // it returns. Retain the old serial, release only its lookup key.
        if self.by_address.get(&(s.queue, s.address)) == Some(&id) {
            self.by_address.remove(&(s.queue, s.address));
        }
        Ok(())
    }
    pub fn free_returned(&mut self, id: u64) -> Result<(), Error> {
        let s = self.slots.get_mut(&id).ok_or(Error::Unknown)?;
        if s.storage != Storage::Freeing {
            return Err(Error::Stale);
        }
        s.storage = Storage::Freed;
        Ok(())
    }
    pub fn executed(&mut self, id: u64) -> Result<M, Error> {
        let s = self.slots.get_mut(&id).ok_or(Error::Unknown)?;
        if !s.execution || s.storage != Storage::Freed {
            return Err(Error::Busy);
        }
        s.execution = false;
        let message = s.message;
        self.collect(id);
        Ok(message)
    }
    pub fn clear_begin(&mut self, queue: u64) -> Result<Vec<u64>, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        if self
            .slots
            .values()
            .any(|s| s.queue == queue && (s.producer || s.execution))
        {
            return Err(Error::Busy);
        }
        let mut ids = Vec::new();
        for (&id, s) in self
            .slots
            .iter_mut()
            .filter(|(_, s)| s.queue == queue && s.storage == Storage::Pending)
        {
            s.storage = Storage::Cancelling;
            ids.push(id);
            if self.by_address.get(&(queue, s.address)) == Some(&id) {
                self.by_address.remove(&(queue, s.address));
            }
        }
        Ok(ids)
    }
    pub fn clear_free(&self, ids: &[u64], address: u64) -> Result<Option<u64>, Error> {
        Ok(ids
            .iter()
            .copied()
            .find(|id| self.slots.get(id).is_some_and(|s| s.address == address)))
    }
    pub fn clear_returned(&mut self, ids: &[u64]) -> Result<Vec<M>, Error> {
        let mut messages = Vec::new();
        for &id in ids {
            let s = self.slots.get_mut(&id).ok_or(Error::Unknown)?;
            if s.execution
                || s.producer
                || !matches!(s.storage, Storage::Cancelling | Storage::Freed)
            {
                return Err(Error::Busy);
            }
            s.storage = Storage::Cancelled;
            messages.push(s.message);
        }
        for &id in ids {
            self.collect(id);
        }
        Ok(messages)
    }
    pub fn queued_addresses(&self, queue: u64) -> Vec<u64> {
        self.slots
            .values()
            .filter(|s| s.queue == queue && s.address != 0 && s.storage == Storage::Pending)
            .map(|s| s.address)
            .collect()
    }
    pub fn queue_message(&self, queue: u64) -> Option<M> {
        self.slots
            .values()
            .find(|s| s.queue == queue)
            .map(|s| s.message)
    }
    pub fn storage_ready(&self, id: u64) -> bool {
        self.slots
            .get(&id)
            .is_some_and(|s| s.storage == Storage::Freed)
    }
    pub fn unresolved(&mut self) {
        self.unresolved = true;
    }
    pub fn pending(&self) -> Result<usize, Error> {
        if self.unresolved {
            Err(Error::Unresolved)
        } else {
            Ok(self.slots.len())
        }
    }
    fn collect(&mut self, id: u64) {
        if self.slots.get(&id).is_some_and(|s| {
            !s.producer && !s.execution && matches!(s.storage, Storage::Freed | Storage::Cancelled)
        }) {
            self.slots.remove(&id);
        }
    }
}
