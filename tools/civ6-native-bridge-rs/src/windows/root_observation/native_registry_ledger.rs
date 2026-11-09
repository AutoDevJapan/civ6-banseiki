//! Tombstones outlive the typed destructor until the owning native root returns.
use super::super::{SessionError as Error, SessionLease as Lease, SessionTransport as Transport};
use std::collections::HashMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Root(pub u64);
#[derive(Clone, Copy)]
pub(super) struct Target {
    pub lease: Lease,
    pub id: u32,
    pub session: u64,
    pub transport: Option<Transport>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Pending,
    Active,
    Returned,
}
struct Item {
    target: Target,
    phase: Phase,
    joined: bool,
}
struct Record {
    registry: u64,
    lifetime: u64,
    items: HashMap<u64, Item>,
}
#[derive(Default)]
pub(super) struct Ledger {
    next: u64,
    roots: HashMap<u64, Record>,
    failed: bool,
}
impl Ledger {
    pub fn begin(
        &mut self,
        registry: u64,
        lifetime: u64,
        targets: Vec<Target>,
    ) -> Result<Root, Error> {
        if self.failed {
            return Err(Error::Unresolved);
        }
        if registry == 0 || lifetime == 0 {
            return Err(Error::Unbound);
        }
        if self.roots.values().any(|r| r.registry == registry) {
            return Err(Error::Busy);
        }
        let mut items = HashMap::new();
        for t in targets {
            if t.session == 0
                || items
                    .insert(
                        t.session,
                        Item {
                            target: t,
                            phase: Phase::Pending,
                            joined: false,
                        },
                    )
                    .is_some()
            {
                return Err(Error::Stale);
            }
        }
        self.next = self.next.checked_add(1).ok_or(Error::Exhausted)?;
        let id = Root(self.next);
        self.roots.insert(
            id.0,
            Record {
                registry,
                lifetime,
                items,
            },
        );
        Ok(id)
    }
    pub fn enter(&mut self, root: Root, session: u64) -> Result<Option<Target>, Error> {
        if self.failed {
            return Err(Error::Unresolved);
        }
        let r = self.roots.get_mut(&root.0).ok_or(Error::Unknown)?;
        let Some(i) = r.items.get_mut(&session) else {
            return Ok(None);
        };
        if i.phase != Phase::Pending {
            return Err(Error::Stale);
        }
        i.phase = Phase::Active;
        Ok(Some(i.target))
    }
    pub fn join(&mut self, root: Root, session: u64, thread: u64, al: u8) -> Result<Target, Error> {
        let i = self
            .roots
            .get_mut(&root.0)
            .and_then(|r| r.items.get_mut(&session))
            .ok_or(Error::Unknown)?;
        if i.phase != Phase::Active || i.target.transport.is_none_or(|t| t.thread != thread) {
            return Err(Error::Stale);
        }
        if al != 1 {
            return Err(Error::NotJoined);
        }
        i.joined = true;
        Ok(i.target)
    }
    pub fn returned(&mut self, root: Root, session: u64) -> Result<Target, Error> {
        if self.failed {
            return Err(Error::Unresolved);
        }
        let i = self
            .roots
            .get_mut(&root.0)
            .and_then(|r| r.items.get_mut(&session))
            .ok_or(Error::Unknown)?;
        if i.phase != Phase::Active || (i.target.transport.is_some() && !i.joined) {
            return Err(Error::NotJoined);
        }
        i.phase = Phase::Returned;
        Ok(i.target)
    }
    pub fn finish(
        &mut self,
        root: Root,
        registry: u64,
        lifetime: u64,
    ) -> Result<Vec<Target>, Error> {
        if self.failed {
            return Err(Error::Unresolved);
        }
        let r = self.roots.get(&root.0).ok_or(Error::Unknown)?;
        if r.registry != registry
            || r.lifetime != lifetime
            || r.items.values().any(|i| i.phase != Phase::Returned)
        {
            return Err(Error::Busy);
        }
        Ok(self
            .roots
            .remove(&root.0)
            .unwrap()
            .items
            .into_values()
            .map(|i| i.target)
            .collect())
    }
    pub fn unresolved(&mut self) {
        self.failed = true;
    }
}
