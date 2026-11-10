//! Native birth stages without authority from numeric IDs or current World.
use super::super::{SessionError as Error, SessionLease as Lease, SessionTransport as Transport};
use std::collections::HashMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Birth(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Factory,
    Transport,
    Network,
    Thread,
    Published,
    Closing,
    Closed,
    Null,
    Failed,
    UnprovenClosing,
    Unproven,
    UnprovenClosed,
}
struct Record {
    lease: Lease,
    session: u64,
    callback: u64,
    pointer: u64,
    network: u64,
    thread: u64,
    phase: Phase,
    active: usize,
    joined: bool,
}
#[derive(Default)]
pub(super) struct Ledger {
    next: u64,
    records: HashMap<u64, Record>,
    pointers: HashMap<u64, Birth>,
    networks: HashMap<u64, Birth>,
    threads: HashMap<u64, Birth>,
    failed: bool,
    unproven_close: bool,
}
impl Ledger {
    pub fn factory(&mut self, lease: Lease, session: u64, callback: u64) -> Result<Birth, Error> {
        if self.failed {
            return Err(Error::Unresolved);
        }
        if session == 0 || callback == 0 {
            return Err(Error::Unbound);
        }
        if self.records.values().any(|r| {
            r.session == session
                && !matches!(r.phase, Phase::Closed | Phase::Null | Phase::UnprovenClosed)
        }) {
            return Err(Error::Busy);
        }
        self.next = self.next.checked_add(1).ok_or(Error::Exhausted)?;
        let id = Birth(self.next);
        self.records.insert(
            id.0,
            Record {
                lease,
                session,
                callback,
                pointer: 0,
                network: 0,
                thread: 0,
                phase: Phase::Factory,
                active: 0,
                joined: false,
            },
        );
        Ok(id)
    }
    fn available(&self, address: u64, index: &HashMap<u64, Birth>) -> bool {
        address != 0
            && index.get(&address).is_none_or(|id| {
                self.records
                    .get(&id.0)
                    .is_some_and(|r| matches!(r.phase, Phase::Closed | Phase::UnprovenClosed))
            })
    }
    pub fn transport(&mut self, id: Birth, pointer: u64, callback: u64) -> Result<(), Error> {
        let r = self.records.get(&id.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Factory || r.callback != callback {
            return Err(Error::Stale);
        }
        if pointer == 0 {
            self.records.get_mut(&id.0).unwrap().phase = Phase::Null;
            return Ok(());
        }
        if !self.available(pointer, &self.pointers) {
            return Err(Error::Busy);
        }
        let r = self.records.get_mut(&id.0).unwrap();
        r.pointer = pointer;
        r.phase = Phase::Transport;
        self.pointers.insert(pointer, id);
        Ok(())
    }
    pub fn pointer_birth(&self, pointer: u64) -> Option<Birth> {
        self.pointers.get(&pointer).copied().filter(|id| {
            self.records.get(&id.0).is_some_and(|r| {
                !matches!(
                    r.phase,
                    Phase::Closed
                        | Phase::Failed
                        | Phase::UnprovenClosing
                        | Phase::Unproven
                        | Phase::UnprovenClosed
                )
            })
        })
    }
    pub fn network_birth(&self, network: u64) -> Option<Birth> {
        self.networks.get(&network).copied().filter(|id| {
            self.records.get(&id.0).is_some_and(|r| {
                !matches!(
                    r.phase,
                    Phase::Closed
                        | Phase::Failed
                        | Phase::UnprovenClosing
                        | Phase::Unproven
                        | Phase::UnprovenClosed
                )
            })
        })
    }
    pub fn published_network(&self, network: u64) -> Result<Option<(Lease, Transport)>, Error> {
        if self.failed {
            return Err(Error::Unresolved);
        }
        let Some(id) = self.network_birth(network) else {
            return Ok(None);
        };
        let r = self.records.get(&id.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Published {
            return Ok(None);
        }
        self.snapshot(id).map(|(lease, _, _, t)| Some((lease, t)))
    }
    pub fn network(&mut self, id: Birth, network: u64) -> Result<(), Error> {
        let r = self.records.get(&id.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Transport || !self.available(network, &self.networks) {
            return Err(Error::Stale);
        }
        let r = self.records.get_mut(&id.0).unwrap();
        r.network = network;
        r.phase = Phase::Network;
        self.networks.insert(network, id);
        Ok(())
    }
    pub fn thread(&mut self, id: Birth, thread: u64) -> Result<(), Error> {
        let r = self.records.get(&id.0).ok_or(Error::Unknown)?;
        if r.phase != Phase::Network || !self.available(thread, &self.threads) {
            return Err(Error::Stale);
        }
        let r = self.records.get_mut(&id.0).unwrap();
        r.thread = thread;
        r.phase = Phase::Thread;
        self.threads.insert(thread, id);
        Ok(())
    }
    pub fn snapshot(&self, id: Birth) -> Result<(Lease, u64, u64, Transport), Error> {
        let r = self.records.get(&id.0).ok_or(Error::Unknown)?;
        if self.failed
            || matches!(
                r.phase,
                Phase::Closed | Phase::Failed | Phase::Null | Phase::UnprovenClosed
            )
        {
            return Err(Error::Stale);
        }
        Ok((
            r.lease,
            r.session,
            r.callback,
            Transport {
                incarnation: id.0,
                pointer: r.pointer,
                network: r.network,
                thread: r.thread,
            },
        ))
    }
    pub fn publish(&mut self, id: Birth, thread: u64) -> Result<Transport, Error> {
        let (_, _, _, t) = self.snapshot(id)?;
        let r = self.records.get_mut(&id.0).unwrap();
        if r.phase != Phase::Thread || thread == 0 || r.thread != thread {
            return Err(Error::Stale);
        }
        r.phase = Phase::Published;
        Ok(t)
    }
    pub fn start_failed(&mut self, id: Birth) -> Result<(), Error> {
        let r = self.records.get_mut(&id.0).ok_or(Error::Unknown)?;
        if !matches!(r.phase, Phase::Network | Phase::Thread) || r.active != 0 {
            return Err(Error::Busy);
        }
        self.threads.remove(&r.thread);
        r.thread = 0;
        r.phase = Phase::Network;
        Ok(())
    }
    pub fn enter(&mut self, network: u64) -> Result<Option<Birth>, Error> {
        let Some(id) = self.network_birth(network) else {
            return Ok(None);
        };
        if self.failed {
            return Err(Error::Unresolved);
        }
        let r = self.records.get_mut(&id.0).unwrap();
        if !matches!(r.phase, Phase::Thread | Phase::Published | Phase::Closing) {
            return Err(Error::Stale);
        }
        r.active = r.active.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(Some(id))
    }
    pub fn leave(&mut self, id: Birth) -> Result<(), Error> {
        let r = self.records.get_mut(&id.0).ok_or(Error::Unknown)?;
        r.active = r.active.checked_sub(1).ok_or(Error::Stale)?;
        Ok(())
    }
    pub fn close(&mut self, session: u64) -> Result<Option<(Lease, Transport)>, Error> {
        let Some((&id, r)) = self
            .records
            .iter()
            .find(|(_, r)| r.session == session && r.phase == Phase::Published)
        else {
            return Ok(None);
        };
        let lease = r.lease;
        let t = self.snapshot(Birth(id))?.3;
        self.records.get_mut(&id).unwrap().phase = Phase::Closing;
        Ok(Some((lease, t)))
    }
    pub fn published(&self, id: Birth) -> Result<bool, Error> {
        let r = self.records.get(&id.0).ok_or(Error::Unknown)?;
        match r.phase {
            Phase::Published => Ok(true),
            Phase::Transport | Phase::Network | Phase::Thread | Phase::Unproven => Ok(false),
            _ => Err(Error::Stale),
        }
    }
    pub fn capture_allowed(&self, session: u64) -> bool {
        !self.records.values().any(|r| {
            r.session == session && matches!(r.phase, Phase::UnprovenClosing | Phase::Unproven)
        })
    }
    pub fn unproven_begin(&mut self, id: Birth) -> Result<(), Error> {
        let r = self.records.get_mut(&id.0).ok_or(Error::Unknown)?;
        if r.active != 0
            || !matches!(
                r.phase,
                Phase::Transport
                    | Phase::Network
                    | Phase::Thread
                    | Phase::Published
                    | Phase::Unproven
            )
        {
            return Err(Error::Busy);
        }
        r.phase = Phase::UnprovenClosing;
        self.unproven_close = true;
        Ok(())
    }
    pub fn unproven_end(
        &mut self,
        id: Birth,
        lease: Lease,
        t: Transport,
        exceptional: bool,
    ) -> Result<(), Error> {
        let r = self.records.get_mut(&id.0).ok_or(Error::Unknown)?;
        if r.phase == Phase::UnprovenClosed
            && exceptional
            && r.lease == lease
            && r.pointer == t.pointer
            && r.network == t.network
            && r.thread == t.thread
        {
            return Ok(());
        }
        if r.phase != Phase::UnprovenClosing
            || r.lease != lease
            || r.pointer != t.pointer
            || r.network != t.network
            || r.thread != t.thread
            || r.active != 0
        {
            return Err(Error::Stale);
        }
        // Original close returned, but no join certificate was observed.
        // Retire lookup authority without claiming joined/native quiescence.
        r.phase = if exceptional {
            Phase::Unproven
        } else {
            Phase::UnprovenClosed
        };
        Ok(())
    }
    pub fn has_unproven_close(&self) -> bool {
        self.unproven_close
    }
    pub fn note_unproven(&mut self) {
        self.unproven_close = true;
    }
    pub fn unproven_session_end(&mut self, session: u64, exceptional: bool) -> Result<(), Error> {
        for r in self.records.values_mut().filter(|r| r.session == session) {
            if r.active != 0 {
                return Err(Error::Busy);
            }
            if matches!(
                r.phase,
                Phase::Unproven
                    | Phase::UnprovenClosing
                    | Phase::Transport
                    | Phase::Network
                    | Phase::Thread
            ) {
                r.phase = if exceptional {
                    Phase::Unproven
                } else {
                    Phase::UnprovenClosed
                };
            }
        }
        self.unproven_close = true;
        Ok(())
    }
    pub fn joined(&mut self, lease: Lease, t: Transport, thread: u64, al: u8) -> Result<(), Error> {
        let r = self.records.get_mut(&t.incarnation).ok_or(Error::Unknown)?;
        if r.lease != lease
            || r.phase != Phase::Closing
            || r.pointer != t.pointer
            || r.network != t.network
            || r.thread != t.thread
            || thread != r.thread
        {
            return Err(Error::Stale);
        }
        if al != 1 {
            return Err(Error::NotJoined);
        }
        if r.active != 0 {
            return Err(Error::Busy);
        }
        r.joined = true;
        Ok(())
    }
    pub fn closed(&mut self, lease: Lease, t: Transport) -> Result<(), Error> {
        let r = self.records.get_mut(&t.incarnation).ok_or(Error::Unknown)?;
        if r.lease != lease
            || r.phase != Phase::Closing
            || !r.joined
            || r.active != 0
            || r.pointer != t.pointer
            || r.network != t.network
            || r.thread != t.thread
        {
            return Err(Error::NotJoined);
        }
        r.phase = Phase::Closed;
        Ok(())
    }
    pub fn unresolved(&mut self, id: Option<Birth>) {
        self.failed = true;
        if let Some(r) = id.and_then(|id| self.records.get_mut(&id.0)) {
            r.phase = Phase::Failed
        }
    }
}
