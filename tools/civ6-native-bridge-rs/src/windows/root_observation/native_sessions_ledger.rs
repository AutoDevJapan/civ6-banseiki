//! Identity accounting. Native addresses and IDs never create World authority.
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lease(pub(super) u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message(pub(super) u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldReceipt {
    pub binding_id: u64,
    pub epoch: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Unknown,
    Stale,
    Closed,
    Busy,
    Unresolved,
    Exhausted,
    Unbound,
    NotJoined,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Birth,
    Open,
    Closing,
    Closed,
    Failed,
    Null,
}
struct Record {
    registry: u64,
    registry_lifetime: Option<u64>,
    owner: u64,
    kind: u32,
    world: Option<WorldReceipt>,
    native_id: Option<u32>,
    session: u64,
    callback: u64,
    transport: Option<Transport>,
    phase: Phase,
    active: usize,
    joined: bool,
    transport_closing: bool,
    transport_closed: bool,
    transport_joined: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transport {
    pub incarnation: u64,
    pub pointer: u64,
    pub network: u64,
    pub thread: u64,
}
struct Packet {
    lease: Lease,
    transport: u64,
    world: WorldReceipt,
    active: bool,
}
#[derive(Default)]
pub(super) struct Ledger {
    next: u64,
    records: HashMap<u64, Record>,
    sessions: HashMap<u64, Lease>,
    callbacks: HashMap<u64, Lease>,
    keys: HashMap<(u64, u32), Lease>,
    packets: HashMap<u64, Packet>,
    retired_worlds: HashSet<(u64, u64)>,
    transport_ids: HashSet<u64>,
    unresolved: bool,
}
impl Ledger {
    fn id(&mut self) -> Result<u64, Error> {
        self.next = self.next.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(self.next)
    }
    pub fn birth(
        &mut self,
        registry: u64,
        lifetime: Option<u64>,
        owner: u64,
        kind: u32,
    ) -> Result<Lease, Error> {
        if registry == 0 || lifetime == Some(0) {
            return Err(Error::Unbound);
        }
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let id = self.id()?;
        self.records.insert(
            id,
            Record {
                registry,
                registry_lifetime: lifetime,
                owner,
                kind,
                world: None,
                native_id: None,
                session: 0,
                callback: 0,
                transport: None,
                phase: Phase::Birth,
                active: 0,
                joined: false,
                transport_closing: false,
                transport_closed: true,
                transport_joined: false,
            },
        );
        Ok(Lease(id))
    }
    pub fn factory(
        &mut self,
        lease: Lease,
        id: u32,
        owner: u64,
        session: u64,
        callback: u64,
    ) -> Result<(), Error> {
        let r = self.records.get(&lease.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Birth || r.owner != owner {
            return Err(Error::Stale);
        }
        if session == 0 {
            let r = self.records.get_mut(&lease.0).unwrap();
            r.phase = Phase::Null;
            r.native_id = Some(id);
            return Ok(());
        }
        // Claims precede both original shared stores. A live old key or
        // address is never silently transferred to a new incarnation.
        for old in [
            self.sessions.get(&session),
            self.callbacks.get(&callback),
            self.keys.get(&(r.registry, id)),
        ]
        .into_iter()
        .flatten()
        {
            if self
                .records
                .get(&old.0)
                .is_none_or(|r| r.phase != Phase::Closed)
            {
                return Err(Error::Busy);
            }
        }
        let r = self.records.get_mut(&lease.0).unwrap();
        r.native_id = Some(id);
        r.session = session;
        r.callback = callback;
        self.sessions.insert(session, lease);
        if callback != 0 {
            self.callbacks.insert(callback, lease);
        }
        self.keys.insert((r.registry, id), lease);
        Ok(())
    }
    pub fn published(&mut self, lease: Lease, id: u32) -> Result<(), Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if r.native_id != Some(id) {
            return Err(Error::Stale);
        }
        match r.phase {
            Phase::Null => Ok(()),
            Phase::Birth if r.session != 0 => {
                r.phase = Phase::Open;
                Ok(())
            }
            _ => Err(Error::Stale),
        }
    }
    /// The caller owns the exact World/session event proof. Neither lookup
    /// nor the current epoch substitutes for this single-use relation.
    pub fn bind_world(&mut self, lease: Lease, world: WorldReceipt) -> Result<(), Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if world.binding_id == 0
            || world.epoch == 0
            || r.world.is_some()
            || r.registry_lifetime.is_none()
            || r.phase != Phase::Open
            || r.callback == 0
            || r.kind == 4
        {
            return Err(Error::Unbound);
        }
        r.world = Some(world);
        Ok(())
    }
    pub fn session(&self, session: u64) -> Option<Lease> {
        self.sessions.get(&session).copied()
    }
    pub fn existing_transport_owner(
        &self,
        session: u64,
        callback: u64,
    ) -> Result<Option<Lease>, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let Some(lease) = self.session(session) else {
            return Ok(None);
        };
        let r = self.records.get(&lease.0).ok_or(Error::Unknown)?;
        if r.session != session
            || r.callback != callback
            || r.registry_lifetime.is_none()
            || r.phase != Phase::Open
            || self.callbacks.get(&callback) != Some(&lease)
        {
            return Ok(None);
        }
        Ok(Some(lease))
    }
    pub fn publish_transport(
        &mut self,
        lease: Lease,
        incarnation: u64,
        pointer: u64,
        network: u64,
        thread: u64,
    ) -> Result<Transport, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let r = self.records.get(&lease.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Open
            || r.kind == 4
            || r.transport_closing
            || !r.transport_closed
            || incarnation == 0
            || pointer == 0
            || network == 0
            || thread == 0
            || self.transport_ids.contains(&incarnation)
            || self.packets.values().any(|p| p.lease == lease)
        {
            return Err(Error::Stale);
        }
        let transport = Transport {
            incarnation,
            pointer,
            network,
            thread,
        };
        self.transport_ids.insert(incarnation);
        let r = self.records.get_mut(&lease.0).unwrap();
        r.transport = Some(transport);
        r.transport_closed = false;
        r.transport_joined = false;
        Ok(transport)
    }
    pub fn transport_disconnect_begin(&mut self, lease: Lease) -> Result<Option<Transport>, Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Open || r.transport_closing {
            return Err(Error::Stale);
        }
        if r.transport_closed {
            return Ok(None);
        }
        r.transport_closing = true;
        r.transport_joined = false;
        Ok(r.transport)
    }
    pub fn transport_join_result(
        &mut self,
        lease: Lease,
        transport: Transport,
        thread: u64,
        al: u8,
    ) -> Result<(), Error> {
        let r = self.records.get(&lease.0).ok_or(Error::Unknown)?;
        if !r.transport_closing || r.transport != Some(transport) || transport.thread != thread {
            return Err(Error::Stale);
        }
        if al != 1 {
            return Err(Error::NotJoined);
        }
        if r.active != 0 || self.packets.values().any(|p| p.lease == lease) {
            return Err(Error::Busy);
        }
        self.records.get_mut(&lease.0).unwrap().transport_joined = true;
        Ok(())
    }
    pub fn transport_disconnect_end(&mut self, lease: Lease) -> Result<(), Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if !r.transport_closing || !r.transport_joined || r.active != 0 {
            return Err(Error::NotJoined);
        }
        r.transport_closed = true;
        r.transport_closing = false;
        r.transport_joined = false;
        Ok(())
    }
    pub fn snapshot_owner(&self, owner: u64, session: u64) -> Option<Lease> {
        let lease = self.session(session)?;
        self.records
            .get(&lease.0)
            .filter(|r| {
                r.owner == owner
                    && r.registry_lifetime.is_some()
                    && r.phase == Phase::Open
                    && r.kind != 4
                    && r.callback != 0
            })
            .map(|_| lease)
    }
    /// The caller supplies the previous binding captured by the accepted
    /// native load request and the new World's before-publication birth.
    /// Existing packets retain their issued World even after this transition.
    pub fn loaded_world(
        &mut self,
        lease: Lease,
        previous: Option<WorldReceipt>,
        new: WorldReceipt,
    ) -> Result<(), Error> {
        if previous.is_some_and(|old| !self.retired_worlds.contains(&(old.binding_id, old.epoch))) {
            return Err(Error::Busy);
        }
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if self.unresolved
            || r.phase != Phase::Open
            || r.registry_lifetime.is_none()
            || r.kind == 4
            || r.world != previous
            || new.binding_id == 0
            || new.epoch == 0
            || previous
                .is_some_and(|old| old.binding_id == new.binding_id || old.epoch >= new.epoch)
        {
            return Err(Error::Stale);
        }
        r.world = Some(new);
        Ok(())
    }
    /// Logical fence after the verified old World's native cleanup. It
    /// accounts for tracked packets only and is not a transport/OS queue seal.
    pub fn retire_world(&mut self, world: WorldReceipt) -> Result<(), Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        if world.binding_id == 0 || world.epoch == 0 {
            return Err(Error::Unbound);
        }
        if self.packets.values().any(|p| p.world == world)
            || self
                .records
                .values()
                .any(|r| r.world == Some(world) && r.active != 0)
        {
            return Err(Error::Busy);
        }
        self.retired_worlds.insert((world.binding_id, world.epoch));
        Ok(())
    }
    pub fn born(&self, registry: u64, lifetime: u64, id: u32) -> Option<Lease> {
        let l = *self.keys.get(&(registry, id))?;
        self.records
            .get(&l.0)
            .filter(|r| r.registry_lifetime == Some(lifetime) && r.phase == Phase::Open)
            .map(|_| l)
    }
    pub fn retired_slot(&self, registry: u64, id: u32, session: u64) -> bool {
        self.keys
            .get(&(registry, id))
            .and_then(|l| self.records.get(&l.0))
            .is_some_and(|r| r.session == session && r.phase == Phase::Closed)
    }
    pub fn registry(&self, lease: Lease) -> Result<u64, Error> {
        Ok(self.records.get(&lease.0).ok_or(Error::Unknown)?.registry)
    }
    pub fn pending(&self, message: Message) -> Result<(), Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let p = self.packets.get(&message.0).ok_or(Error::Unknown)?;
        if p.active {
            Err(Error::Busy)
        } else {
            Ok(())
        }
    }
    pub fn world(&self, lease: Lease) -> Result<WorldReceipt, Error> {
        self.records
            .get(&lease.0)
            .ok_or(Error::Unknown)?
            .world
            .ok_or(Error::Unbound)
    }
    pub fn snapshot(&self, lease: Lease) -> Result<(u64, u32, u64), Error> {
        let r = self.records.get(&lease.0).ok_or(Error::Unknown)?;
        Ok((r.session, r.native_id.ok_or(Error::Unbound)?, r.callback))
    }
    pub fn bind_transport(
        &mut self,
        lease: Lease,
        pointer: u64,
        network: u64,
        thread: u64,
    ) -> Result<Transport, Error> {
        let r = self.records.get(&lease.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Open
            || pointer == 0
            || network == 0
            || thread == 0
            || r.transport.is_some()
            || r.kind == 4
        {
            return Err(Error::Stale);
        }
        let transport = Transport {
            incarnation: self.id()?,
            pointer,
            network,
            thread,
        };
        self.records.get_mut(&lease.0).unwrap().transport = Some(transport);
        self.records.get_mut(&lease.0).unwrap().transport_closed = false;
        Ok(transport)
    }
    pub fn transport(&self, lease: Lease) -> Result<Transport, Error> {
        self.records
            .get(&lease.0)
            .ok_or(Error::Unknown)?
            .transport
            .ok_or(Error::Unbound)
    }
    pub fn reserve(&mut self, lease: Lease) -> Result<Message, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let r = self.records.get(&lease.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Open {
            return Err(Error::Closed);
        }
        let world = self.world(lease)?;
        if self
            .retired_worlds
            .contains(&(world.binding_id, world.epoch))
        {
            return Err(Error::Closed);
        }
        let transport = r.transport.ok_or(Error::Unbound)?.incarnation;
        if r.transport_closing || r.transport_closed {
            return Err(Error::Closed);
        }
        let id = self.id()?;
        self.packets.insert(
            id,
            Packet {
                lease,
                transport,
                world,
                active: false,
            },
        );
        Ok(Message(id))
    }
    pub fn message_world(&self, message: Message) -> Result<WorldReceipt, Error> {
        self.packets
            .get(&message.0)
            .map(|p| p.world)
            .ok_or(Error::Unknown)
    }
    pub fn message_lease(&self, message: Message) -> Result<Lease, Error> {
        self.packets
            .get(&message.0)
            .map(|p| p.lease)
            .ok_or(Error::Unknown)
    }
    pub fn message_session(&self, message: Message) -> Result<u64, Error> {
        let lease = self.message_lease(message)?;
        self.records
            .get(&lease.0)
            .map(|r| r.session)
            .ok_or(Error::Unknown)
    }
    /// A pending native serialized record copies an already executing packet;
    /// it never reserves from the Session's mutable World binding.
    pub fn fork_message(&mut self, parent: Message) -> Result<Message, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let p = self.packets.get(&parent.0).ok_or(Error::Unknown)?;
        let r = self.records.get(&p.lease.0).ok_or(Error::Unknown)?;
        if !p.active
            || !matches!(r.phase, Phase::Open | Phase::Closing)
            || self
                .retired_worlds
                .contains(&(p.world.binding_id, p.world.epoch))
        {
            return Err(Error::Stale);
        }
        let packet = Packet {
            lease: p.lease,
            transport: p.transport,
            world: p.world,
            active: false,
        };
        let id = self.id()?;
        self.packets.insert(id, packet);
        Ok(Message(id))
    }
    pub fn dispatch_deferred(&mut self, message: Message, session: u64) -> Result<Lease, Error> {
        let lease = self.message_lease(message)?;
        let r = self.records.get(&lease.0).ok_or(Error::Unknown)?;
        if r.session != session || self.sessions.get(&session) != Some(&lease) {
            return Err(Error::Stale);
        }
        let (callback, id) = (r.callback, r.native_id.ok_or(Error::Unbound)?);
        self.dispatch(message, callback, id)
    }
    pub fn dispatch(&mut self, message: Message, callback: u64, id: u32) -> Result<Lease, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let p = self.packets.get_mut(&message.0).ok_or(Error::Unknown)?;
        let r = self.records.get(&p.lease.0).ok_or(Error::Unknown)?;
        if self
            .retired_worlds
            .contains(&(p.world.binding_id, p.world.epoch))
            || p.active
            || !matches!(r.phase, Phase::Open | Phase::Closing)
            || r.callback != callback
            || r.native_id != Some(id)
            || r.transport.map(|t| t.incarnation) != Some(p.transport)
            || r.transport_closed
            || self.callbacks.get(&callback) != Some(&p.lease)
        {
            return Err(Error::Stale);
        }
        p.active = true;
        let lease = p.lease;
        self.records.get_mut(&lease.0).unwrap().active += 1;
        Ok(lease)
    }
    pub fn admitted(&mut self, lease: Lease, session: u64) -> Result<(), Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if r.session != session
            || !matches!(r.phase, Phase::Open | Phase::Closing)
            || self.sessions.get(&session) != Some(&lease)
        {
            return Err(Error::Stale);
        }
        r.active = r.active.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(())
    }
    pub fn admission_returned(&mut self, lease: Lease) -> Result<(), Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        r.active = r.active.checked_sub(1).ok_or(Error::Stale)?;
        Ok(())
    }
    pub fn dispatched(&mut self, message: Message) -> Result<(), Error> {
        let p = self.packets.get(&message.0).ok_or(Error::Unknown)?;
        if !p.active {
            return Err(Error::Stale);
        }
        let lease = p.lease;
        self.admission_returned(lease)?;
        self.packets.remove(&message.0);
        Ok(())
    }
    /// Only an actual pre-dispatch native cancellation owner may use this.
    pub fn cancel_before_dispatch(&mut self, message: Message) -> Result<(), Error> {
        if self.packets.get(&message.0).is_none_or(|p| p.active) {
            return Err(Error::Busy);
        }
        self.packets.remove(&message.0);
        Ok(())
    }
    #[cfg(test)]
    pub fn close(&mut self, lease: Lease) -> Result<Option<Transport>, Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Open {
            return Err(Error::Closed);
        }
        r.phase = Phase::Closing;
        Ok(r.transport)
    }
    pub fn join(
        &mut self,
        lease: Lease,
        transport: Transport,
        thread: u64,
        al: u8,
    ) -> Result<(), Error> {
        let r = self.records.get(&lease.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Closing || r.transport != Some(transport) || transport.thread != thread
        {
            return Err(Error::Stale);
        }
        if al != 1 {
            return Err(Error::NotJoined);
        }
        if r.active != 0 || self.packets.values().any(|p| p.lease == lease) {
            return Err(Error::Busy);
        }
        self.records.get_mut(&lease.0).unwrap().joined = true;
        Ok(())
    }
    pub fn closed(&mut self, lease: Lease) -> Result<(), Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Closing || !r.joined || r.active != 0 {
            return Err(Error::NotJoined);
        }
        r.phase = Phase::Closed;
        Ok(())
    }
    pub fn unresolved(&mut self, lease: Option<Lease>) {
        self.unresolved = true;
        if let Some(l) = lease {
            if let Some(r) = self.records.get_mut(&l.0) {
                r.phase = Phase::Failed;
            }
        }
    }
    pub fn drained(&self, world: WorldReceipt) -> Result<(), Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let mut found = false;
        for r in self.records.values().filter(|r| r.world == Some(world)) {
            found = true;
            if r.phase != Phase::Closed || r.active != 0 {
                return Err(Error::Busy);
            }
        }
        if found {
            Ok(())
        } else {
            Err(Error::Unknown)
        }
    }
}
