//! Identity accounting. Native addresses and IDs never create World authority.
use super::super::worlds::{BirthWindow, WorldLease};
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
pub(in super::super) struct BootstrapTarget {
    pub lease: Lease,
    pub window: BirthWindow,
    pub transport: Option<u64>,
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
    UnprovenRetired,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeReset {
    Closing(Transport),
    Returned(Transport),
}
struct Record {
    registry: u64,
    registry_lifetime: Option<u64>,
    owner: u64,
    kind: u32,
    world: Option<WorldReceipt>,
    birth_window: Option<BirthWindow>,
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
    unproven_transport: Option<Transport>,
    native_reset: Option<NativeReset>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transport {
    pub incarnation: u64,
    pub pointer: u64,
    pub network: u64,
    pub thread: u64,
}
pub type TerminalTarget = (Lease, u32, u64, Option<Transport>);
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
    input_worlds: HashMap<u64, Option<WorldReceipt>>,
    unresolved: bool,
    transport_proof_gaps: HashSet<(u64, u64)>,
    reset_inputs: HashSet<(u64, u64)>,
    retired_reset_packets: HashSet<u64>,
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
                birth_window: None,
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
                unproven_transport: None,
                native_reset: None,
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
                .is_none_or(|r| !matches!(r.phase, Phase::Closed | Phase::UnprovenRetired))
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
    pub fn observed_birth_return(&self, lease: Lease) -> bool {
        self.records.get(&lease.0).is_some_and(|record| {
            record.registry_lifetime.is_some()
                && record.native_id.is_some()
                && matches!(record.phase, Phase::Open | Phase::Null)
        })
    }
    /// The caller owns the exact World/session event proof. Neither lookup
    /// nor the current epoch substitutes for this single-use relation.
    pub fn bind_world(&mut self, lease: Lease, world: WorldReceipt) -> Result<(), Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if r.unproven_transport.is_some() {
            return Err(Error::Unresolved);
        }
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
        r.birth_window = None;
        Ok(())
    }
    pub fn record_birth_window(&mut self, lease: Lease, window: BirthWindow) {
        if let Some(record) = self.records.get_mut(&lease.0) {
            if record.phase == Phase::Birth
                && record.registry_lifetime.is_some()
                && record.kind != 4
            {
                record.birth_window = Some(window);
            }
        }
    }
    pub fn bootstrap_target(&self, owner: u64, session: u64) -> Option<BootstrapTarget> {
        let lease = self.snapshot_owner(owner, session)?;
        let record = self.records.get(&lease.0)?;
        if self.unresolved || record.world.is_some() {
            return None;
        }
        Some(BootstrapTarget {
            lease,
            window: record.birth_window?,
            transport: None,
        })
    }
    pub fn bootstrap_packet_matches(&self, lease: Lease, incarnation: u64) -> bool {
        self.records.get(&lease.0).is_some_and(|record| {
            record.phase == Phase::Open
                && !record.transport_closed
                && !record.transport_closing
                && record
                    .transport
                    .is_some_and(|transport| transport.incarnation == incarnation)
        })
    }
    pub fn bind_bootstrap_world(
        &mut self,
        target: BootstrapTarget,
        world: WorldLease,
    ) -> Result<(), Error> {
        let record = self.records.get(&target.lease.0).ok_or(Error::Unknown)?;
        if record.unproven_transport.is_some() {
            return Err(Error::Unresolved);
        }
        if target
            .transport
            .is_some_and(|transport| !self.bootstrap_packet_matches(target.lease, transport))
        {
            return Err(Error::Stale);
        }
        if self.unresolved
            || record.birth_window != Some(target.window)
            || !target.window.matches(world)
            || self.retired_worlds.contains(&(world.serial, world.epoch))
        {
            return Err(Error::Stale);
        }
        self.bind_world(
            target.lease,
            WorldReceipt {
                binding_id: world.serial,
                epoch: world.epoch,
            },
        )
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
    pub fn terminal_remove_begin(
        &mut self,
        registry: u64,
        lifetime: u64,
        id: u32,
        session: u64,
    ) -> Result<Option<(Lease, Option<Transport>)>, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let Some(lease) = self.keys.get(&(registry, id)).copied() else {
            return Ok(None);
        };
        let r = self.records.get(&lease.0).ok_or(Error::Unknown)?;
        if r.registry_lifetime != Some(lifetime) || r.session != session || r.native_id != Some(id)
        {
            return Ok(None);
        }
        if r.phase != Phase::Open
            || r.active != 0
            || self
                .packets
                .values()
                .any(|p| p.lease == lease && !self.reset_inputs.contains(&(lease.0, p.transport)))
        {
            return Err(Error::Busy);
        }
        let r = self.records.get_mut(&lease.0).unwrap();
        r.phase = Phase::Closing;
        let transport = if r.transport_closed {
            None
        } else {
            r.transport
        };
        r.transport_closing = transport.is_some();
        r.transport_joined = false;
        Ok(Some((lease, transport)))
    }
    pub fn terminal_registry_begin(
        &mut self,
        registry: u64,
        lifetime: u64,
    ) -> Result<Vec<TerminalTarget>, Error> {
        let candidates = self
            .records
            .iter()
            .filter(|(_, r)| {
                r.registry == registry
                    && r.registry_lifetime == Some(lifetime)
                    && r.phase == Phase::Open
            })
            .map(|(&id, r)| (Lease(id), r.native_id.unwrap_or(0), r.session))
            .collect::<Vec<_>>();
        if self.unresolved
            || candidates.iter().any(|(l, _, _)| {
                self.records[&l.0].active != 0
                    || self
                        .packets
                        .values()
                        .any(|p| p.lease == *l && !self.reset_inputs.contains(&(l.0, p.transport)))
            })
        {
            return Err(Error::Busy);
        }
        let mut output = Vec::new();
        for (lease, id, session) in candidates {
            let (confirmed, transport) = self
                .terminal_remove_begin(registry, lifetime, id, session)?
                .ok_or(Error::Stale)?;
            if confirmed != lease {
                return Err(Error::Stale);
            }
            output.push((lease, id, session, transport));
        }
        Ok(output)
    }
    pub fn terminal_remove_end(&mut self, lease: Lease) -> Result<(), Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if r.unproven_transport.is_some() {
            return Err(Error::Unresolved);
        }
        if r.phase != Phase::Closing
            || (!r.transport_closed && r.unproven_transport.is_none())
            || r.transport_closing
            || r.active != 0
            || self
                .packets
                .values()
                .any(|p| p.lease == lease && !self.reset_inputs.contains(&(lease.0, p.transport)))
        {
            return Err(Error::Busy);
        }
        r.phase = Phase::Closed;
        self.retire_reset_packets(lease);
        Ok(())
    }
    pub fn terminal_registry_end(&self, registry: u64, lifetime: u64) -> Result<(), Error> {
        if self.records.values().any(|r| {
            r.registry == registry
                && r.registry_lifetime == Some(lifetime)
                && (r.unproven_transport.is_some() || r.phase == Phase::UnprovenRetired)
        }) {
            return Err(Error::Unresolved);
        }
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        if self.records.values().any(|r| {
            r.registry == registry
                && r.registry_lifetime == Some(lifetime)
                && !matches!(r.phase, Phase::Closed | Phase::Null)
        }) {
            return Err(Error::Busy);
        }
        Ok(())
    }
    /// Native object removal is a lookup-retirement fact, not a join proof.
    pub fn retire_unproven_native(&mut self, lease: Lease) -> Result<(), Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Closing
            || r.unproven_transport.is_none()
            || r.transport_closing
            || r.active != 0
            || self.packets.values().any(|packet| {
                packet.lease == lease && !self.reset_inputs.contains(&(lease.0, packet.transport))
            })
        {
            return Err(Error::Busy);
        }
        r.phase = Phase::UnprovenRetired;
        self.retire_reset_packets(lease);
        Ok(())
    }
    pub fn unproven_native_unwind(&mut self, lease: Lease) -> Result<(), Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if r.unproven_transport.is_none() || r.active != 0 {
            return Err(Error::Stale);
        }
        r.phase = Phase::Open;
        r.transport_closing = false;
        r.transport_closed = false;
        Ok(())
    }
    pub fn retired_registry_metadata(&self, registry: u64, lifetime: u64) -> Result<(), Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        if self.records.values().any(|r| {
            r.registry == registry
                && r.registry_lifetime == Some(lifetime)
                && !matches!(
                    r.phase,
                    Phase::Closed | Phase::Null | Phase::UnprovenRetired
                )
        }) {
            return Err(Error::Busy);
        }
        Ok(())
    }
    pub fn unproven_transport_marker(&self, lease: Lease) -> Option<Transport> {
        let record = self.records.get(&lease.0)?;
        record
            .unproven_transport
            .or(match record.native_reset {
                Some(NativeReset::Closing(old) | NativeReset::Returned(old)) => Some(old),
                None => None,
            })
            .or_else(|| {
                record
                    .world
                    .filter(|world| {
                        self.transport_proof_gaps
                            .contains(&(world.binding_id, world.epoch))
                    })
                    .and(record.transport)
            })
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
        let reset = matches!(r.native_reset,Some(NativeReset::Returned(old)) if old.pointer==pointer && old.incarnation!=incarnation);
        if r.phase != Phase::Open
            || r.kind == 4
            || r.transport_closing
            || (!r.transport_closed && !reset)
            || incarnation == 0
            || pointer == 0
            || network == 0
            || thread == 0
            || self.transport_ids.contains(&incarnation)
            || self
                .packets
                .values()
                .any(|p| p.lease == lease && !self.reset_inputs.contains(&(lease.0, p.transport)))
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
        self.input_worlds.insert(incarnation, r.world);
        let r = self.records.get_mut(&lease.0).unwrap();
        r.transport = Some(transport);
        r.unproven_transport = None;
        r.native_reset = None;
        r.transport_closed = false;
        r.transport_joined = false;
        Ok(transport)
    }
    pub fn native_reset_begin(&mut self, lease: Lease, old: Transport) -> Result<(), Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let record = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        // After a published binding was reset, a new attempt may allocate a
        // network but fail before publication. Its next native reset is legal
        // only under a newer, never-published identity at the same pointer.
        // Earlier reset packet groups remain in reset_inputs independently of
        // this current reset marker; replacing it must not adopt their work.
        let unpublished_retry = matches!(record.native_reset, Some(NativeReset::Returned(previous))
            if record.transport.is_none()
                && !record.transport_closed
                && old.pointer == previous.pointer
                && old.incarnation > previous.incarnation
                && old.network != 0
                && !self.transport_ids.contains(&old.incarnation));
        if record.phase != Phase::Open
            || record.transport_closing
            || (record.native_reset.is_some() && !unpublished_retry)
        {
            return Err(Error::Stale);
        }
        if record.transport.is_none() && record.transport_closed {
            return Ok(());
        }
        if (!unpublished_retry && record.transport != Some(old))
            || record.unproven_transport.is_some()
        {
            return Err(Error::Stale);
        }
        record.native_reset = Some(NativeReset::Closing(old));
        record.transport = None;
        record.transport_closing = true;
        record.transport_joined = false;
        self.reset_inputs.insert((lease.0, old.incarnation));
        if let Some(world) = record.world {
            self.transport_proof_gaps
                .insert((world.binding_id, world.epoch));
        }
        Ok(())
    }
    pub fn native_reset_returned(&mut self, lease: Lease, old: Transport) -> Result<(), Error> {
        let record = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if record.native_reset.is_none() && record.transport.is_none() && record.transport_closed {
            return Ok(());
        }
        if record.native_reset != Some(NativeReset::Closing(old)) || record.transport.is_some() {
            return Err(Error::Stale);
        }
        record.native_reset = Some(NativeReset::Returned(old));
        record.transport_closing = false;
        // The prior binding is not asserted closed or joined. Only a newly
        // observed publication may replace it, under a different incarnation.
        Ok(())
    }
    pub fn take_reset_input(
        &mut self,
        message: Message,
        callback: u64,
        id: u32,
    ) -> Result<Option<Lease>, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let packet = self.packets.get(&message.0).ok_or(Error::Unknown)?;
        if !self
            .reset_inputs
            .contains(&(packet.lease.0, packet.transport))
        {
            return Ok(None);
        }
        let record = self.records.get(&packet.lease.0).ok_or(Error::Unknown)?;
        if packet.active
            || record.phase != Phase::Open
            || record.callback != callback
            || record.native_id != Some(id)
            || self.callbacks.get(&callback) != Some(&packet.lease)
            || self.sessions.get(&record.session) != Some(&packet.lease)
            || self
                .retired_worlds
                .contains(&(packet.world.binding_id, packet.world.epoch))
        {
            return Err(Error::Stale);
        }
        let lease = packet.lease;
        let world = packet.world;
        self.packets.remove(&message.0);
        self.transport_proof_gaps
            .insert((world.binding_id, world.epoch));
        Ok(Some(lease))
    }
    pub fn reserve_input(
        &mut self,
        lease: Lease,
        transport: Transport,
    ) -> Result<Option<Message>, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let r = self.records.get(&lease.0).ok_or(Error::Unknown)?;
        if r.transport != Some(transport)
            || r.phase != Phase::Open
            || r.transport_closed
            || r.transport_closing
        {
            return Err(Error::Stale);
        }
        let Some(world) = self
            .input_worlds
            .get(&transport.incarnation)
            .copied()
            .ok_or(Error::Unbound)?
        else {
            return Ok(None);
        };
        if self
            .retired_worlds
            .contains(&(world.binding_id, world.epoch))
        {
            return Err(Error::Closed);
        }
        let id = self.id()?;
        self.packets.insert(
            id,
            Packet {
                lease,
                transport: transport.incarnation,
                world,
                active: false,
            },
        );
        Ok(Some(Message(id)))
    }
    pub fn fork_input(&mut self, parent: Message) -> Result<Message, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        if self.retired_reset_packets.contains(&parent.0) {
            // Byte storage can outlive its native Session owner. Copies keep
            // an explicitly retired identity and never adopt a current World.
            let copy = self.id()?;
            self.retired_reset_packets.insert(copy);
            return Ok(Message(copy));
        }
        let p = self.packets.get(&parent.0).ok_or(Error::Unknown)?;
        let reset = self.reset_inputs.contains(&(p.lease.0, p.transport));
        if p.active
            || (!reset
                && self
                    .retired_worlds
                    .contains(&(p.world.binding_id, p.world.epoch)))
        {
            return Err(Error::Stale);
        }
        let r = self.records.get(&p.lease.0).ok_or(Error::Unknown)?;
        if !reset && (r.transport.map(|t| t.incarnation) != Some(p.transport) || r.transport_closed)
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
    pub fn unproven_transport_close_begin(
        &mut self,
        lease: Lease,
        t: Transport,
    ) -> Result<(), Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if !matches!(r.phase, Phase::Open | Phase::Closing)
            || r.transport.is_some_and(|current| current != t)
            || (r.transport_closing && r.unproven_transport.is_none() && r.phase != Phase::Closing)
            || r.active != 0
            || self.packets.values().any(|packet| {
                packet.lease == lease && !self.reset_inputs.contains(&(lease.0, packet.transport))
            })
        {
            return Err(Error::Busy);
        }
        // Revoke lookup/dispatch authority before the original closes it.
        // Neither the phase nor the later return is a join certificate.
        r.unproven_transport = Some(t);
        r.native_reset = None;
        r.transport = None;
        r.transport_closing = true;
        r.transport_closed = false;
        r.transport_joined = false;
        if let Some(world) = r.world {
            self.transport_proof_gaps
                .insert((world.binding_id, world.epoch));
        }
        Ok(())
    }
    pub fn unproven_transport_close_end(
        &mut self,
        lease: Lease,
        t: Transport,
        exceptional: bool,
    ) -> Result<(), Error> {
        let r = self.records.get_mut(&lease.0).ok_or(Error::Unknown)?;
        if r.unproven_transport != Some(t)
            || r.active != 0
            || self.packets.values().any(|packet| {
                packet.lease == lease && !self.reset_inputs.contains(&(lease.0, packet.transport))
            })
        {
            return Err(Error::Stale);
        }
        r.transport_closing = false;
        r.transport_closed = !exceptional;
        r.transport_joined = false;
        Ok(())
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
        if r.active != 0
            || self
                .packets
                .values()
                .any(|p| p.lease == lease && !self.reset_inputs.contains(&(lease.0, p.transport)))
        {
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
                    && r.unproven_transport.is_none()
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
        if r.unproven_transport.is_some() {
            return Err(Error::Unresolved);
        }
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
            .is_some_and(|r| {
                r.session == session && matches!(r.phase, Phase::Closed | Phase::UnprovenRetired)
            })
    }
    pub fn registry(&self, lease: Lease) -> Result<u64, Error> {
        Ok(self.records.get(&lease.0).ok_or(Error::Unknown)?.registry)
    }
    pub fn pending(&self, message: Message) -> Result<(), Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        if self.retired_reset_packets.contains(&message.0) {
            return Err(Error::Closed);
        }
        let p = self.packets.get(&message.0).ok_or(Error::Unknown)?;
        if p.active {
            Err(Error::Busy)
        } else {
            Ok(())
        }
    }
    pub fn world(&self, lease: Lease) -> Result<WorldReceipt, Error> {
        let r = self.records.get(&lease.0).ok_or(Error::Unknown)?;
        if r.unproven_transport.is_some() {
            return Err(Error::Unresolved);
        }
        r.world.ok_or(Error::Unbound)
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
    /// The exact callback is still native-valid, but its producer's active
    /// observation was lost. Settle only this ticket; preserve the missing
    /// qualification as a World-scoped gap, never a completed drain.
    pub fn dispatched_unproven(&mut self, message: Message) -> Result<(), Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let world = self.packets.get(&message.0).ok_or(Error::Unknown)?.world;
        self.dispatched(message)?;
        self.transport_proof_gaps
            .insert((world.binding_id, world.epoch));
        Ok(())
    }
    /// Only an actual pre-dispatch native cancellation owner may use this.
    pub fn cancel_before_dispatch(&mut self, message: Message) -> Result<(), Error> {
        if self.retired_reset_packets.remove(&message.0) {
            return Ok(());
        }
        if self.packets.get(&message.0).is_none_or(|p| p.active) {
            return Err(Error::Busy);
        }
        self.packets.remove(&message.0);
        Ok(())
    }
    fn retire_reset_packets(&mut self, lease: Lease) {
        let ids: Vec<_> = self
            .packets
            .iter()
            .filter(|(_, packet)| {
                packet.lease == lease && self.reset_inputs.contains(&(lease.0, packet.transport))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.packets.remove(&id);
            self.retired_reset_packets.insert(id);
        }
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
        if self.unresolved
            || self
                .transport_proof_gaps
                .contains(&(world.binding_id, world.epoch))
        {
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

#[cfg(test)]
#[path = "native_sessions_bootstrap_tests.rs"]
mod bootstrap_tests;
