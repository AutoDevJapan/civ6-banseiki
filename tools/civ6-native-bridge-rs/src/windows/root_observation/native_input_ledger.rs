//! Native input record copies retain their issuing transport and World.
//! Buffer and vector addresses are storage locations, never packet identity.
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Input(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Unknown,
    Stale,
    Busy,
    Unresolved,
    Exhausted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Receipt<M> {
    pub message: M,
    pub transport: u64,
}
struct Record<M> {
    receipt: Receipt<M>,
    locations: usize,
    active: usize,
}
pub(super) struct Ledger<M> {
    next: u64,
    records: HashMap<u64, Record<M>>,
    locations: HashMap<u64, Input>,
    unresolved: bool,
}
impl<M: Copy + Eq> Ledger<M> {
    pub fn new() -> Self {
        Self {
            next: 0,
            records: HashMap::new(),
            locations: HashMap::new(),
            unresolved: false,
        }
    }
    pub fn at(&self, record: u64) -> Option<(Input, Receipt<M>)> {
        let id = *self.locations.get(&record)?;
        Some((id, self.records.get(&id.0)?.receipt))
    }
    pub fn bind(&mut self, input: Input, destination: u64) -> Result<(), Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        if destination == 0 || self.locations.contains_key(&destination) {
            return Err(Error::Busy);
        }
        let r = self.records.get_mut(&input.0).ok_or(Error::Unknown)?;
        r.locations = r.locations.checked_add(1).ok_or(Error::Exhausted)?;
        self.locations.insert(destination, input);
        Ok(())
    }
    pub fn issued(&mut self, receipt: Receipt<M>, record: u64) -> Result<Input, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        if record == 0 || receipt.transport == 0 || self.locations.contains_key(&record) {
            return Err(Error::Stale);
        }
        self.next = self.next.checked_add(1).ok_or(Error::Exhausted)?;
        let input = Input(self.next);
        self.records.insert(
            input.0,
            Record {
                receipt,
                locations: 1,
                active: 0,
            },
        );
        self.locations.insert(record, input);
        Ok(input)
    }
    #[cfg(test)]
    pub fn copy(&mut self, source: u64, destination: u64) -> Result<Option<Input>, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let Some(input) = self.locations.get(&source).copied() else {
            return Ok(None);
        };
        if source == destination {
            return Ok(Some(input));
        }
        if destination == 0 || self.locations.contains_key(&destination) {
            return Err(Error::Busy);
        }
        let r = self.records.get_mut(&input.0).ok_or(Error::Unknown)?;
        r.locations = r.locations.checked_add(1).ok_or(Error::Exhausted)?;
        self.locations.insert(destination, input);
        Ok(Some(input))
    }
    /// Executing frame takes an immutable receipt before the original helper
    /// can release or reuse its physical record. Storage free is separate.
    pub fn enter(
        &mut self,
        record: u64,
        transport: u64,
    ) -> Result<Option<(Input, Receipt<M>)>, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let Some(input) = self.locations.get(&record).copied() else {
            return Ok(None);
        };
        let r = self.records.get_mut(&input.0).ok_or(Error::Unknown)?;
        if r.receipt.transport != transport {
            return Err(Error::Stale);
        }
        r.active = r.active.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(Some((input, r.receipt)))
    }
    /// Remove the old address key at native destruction entry. A new object
    /// reusing this address gets a new Input and cannot be settled by old exit.
    pub fn retire(&mut self, record: u64) -> Result<Option<Input>, Error> {
        let Some(input) = self.locations.remove(&record) else {
            return Ok(None);
        };
        let r = self.records.get_mut(&input.0).ok_or(Error::Unknown)?;
        r.locations = r.locations.checked_sub(1).ok_or(Error::Stale)?;
        Ok(Some(input))
    }
    pub fn returned(&mut self, input: Input) -> Result<(), Error> {
        let r = self.records.get_mut(&input.0).ok_or(Error::Unknown)?;
        r.active = r.active.checked_sub(1).ok_or(Error::Stale)?;
        Ok(())
    }
    pub fn collected(&mut self, input: Input) -> Result<Option<Receipt<M>>, Error> {
        let r = self.records.get(&input.0).ok_or(Error::Unknown)?;
        if r.locations != 0 || r.active != 0 {
            return Ok(None);
        }
        Ok(Some(self.records.remove(&input.0).unwrap().receipt))
    }
    pub fn unresolved(&mut self) {
        self.unresolved = true;
    }
    pub fn pending(&self) -> Result<usize, Error> {
        if self.unresolved {
            Err(Error::Unresolved)
        } else {
            Ok(self.records.len())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deep_copy_and_vector_growth_preserve_original_receipt() {
        let mut l = Ledger::new();
        let r = Receipt {
            message: (1, 8),
            transport: 3,
        };
        let id = l.issued(r, 10).unwrap();
        l.copy(10, 20).unwrap();
        l.retire(10).unwrap();
        assert_eq!(l.collected(id), Ok(None));
        l.copy(20, 30).unwrap();
        l.retire(20).unwrap();
        assert_eq!(l.enter(30, 3), Ok(Some((id, r))));
        l.retire(30).unwrap();
        assert_eq!(l.collected(id), Ok(None));
        l.returned(id).unwrap();
        assert_eq!(l.collected(id), Ok(Some(r)));
        assert_eq!(l.pending(), Ok(0));
    }
    #[test]
    fn free_reuse_and_wrong_transport_cannot_transfer_world_receipt() {
        let mut l = Ledger::new();
        let old = l
            .issued(
                Receipt {
                    message: (1, 8),
                    transport: 3,
                },
                10,
            )
            .unwrap();
        assert!(l.enter(10, 3).unwrap().is_some());
        l.retire(10).unwrap();
        let new = l
            .issued(
                Receipt {
                    message: (2, 9),
                    transport: 4,
                },
                10,
            )
            .unwrap();
        assert_ne!(old, new);
        assert_eq!(l.enter(10, 3), Err(Error::Stale));
        l.returned(old).unwrap();
        assert_eq!(l.collected(old).unwrap().unwrap().message, (1, 8));
        assert_eq!(l.enter(10, 4).unwrap().unwrap().1.message, (2, 9));
    }
    #[test]
    fn unknown_record_never_adopts_current_world_and_unwind_keeps_unresolved() {
        let mut l: Ledger<u64> = Ledger::new();
        assert_eq!(l.copy(10, 20), Ok(None));
        assert_eq!(l.enter(20, 3), Ok(None));
        l.issued(
            Receipt {
                message: 1,
                transport: 3,
            },
            10,
        )
        .unwrap();
        l.unresolved();
        assert_eq!(l.pending(), Err(Error::Unresolved));
        assert_eq!(
            l.issued(
                Receipt {
                    message: 2,
                    transport: 3
                },
                20
            ),
            Err(Error::Unresolved)
        );
    }
}
