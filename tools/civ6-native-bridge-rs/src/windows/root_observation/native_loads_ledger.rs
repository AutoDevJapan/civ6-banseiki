//! A native load request survives command-bank and reused-view transitions.
//! Only an issued command can select a view; looking up the current view or
//! the global stream cannot create a request incarnation.
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Request(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Invocation(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Unknown,
    Stale,
    Busy,
    Rejected,
    Unresolved,
    Exhausted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Entered,
    Accepted,
    Rejected,
}
/// Values copied by the pinned request body into the native pathname load
/// configuration. This carries an intent, never a Session or World authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PathInput {
    pub options: [u8; 28],
    pub name: Vec<u16>,
}
struct Load {
    path: Option<PathInput>,
    stream: Option<u64>,
    stream_birth: Option<u64>,
    outcome: Outcome,
    commands: usize,
    view: Option<Invocation>,
}
struct Command {
    request: Request,
    descriptor: u64,
    running: bool,
}
struct View {
    request: Request,
    engine: u64,
    pointer: u64,
    workers: usize,
    reservations: usize,
    started: bool,
    joined: bool,
    initializer_returned: bool,
    world: Option<(u64, u64)>,
    stream_read: bool,
}
#[derive(Default)]
pub(super) struct Ledger {
    next: u64,
    loads: HashMap<u64, Load>,
    commands: HashMap<u64, Command>,
    pending: HashMap<u64, (u64, Request)>,
    views: HashMap<u64, View>,
    current: HashMap<u64, Invocation>,
    unresolved: bool,
    stream_pointer: u64,
    stream_birth: u64,
    copy_active: Option<u64>,
}
impl Ledger {
    fn id(&mut self) -> Result<u64, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        self.next = self.next.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(self.next)
    }
    pub fn enter(&mut self) -> Result<Request, Error> {
        let id = self.id()?;
        self.loads.insert(
            id,
            Load {
                path: None,
                stream: None,
                stream_birth: None,
                outcome: Outcome::Entered,
                commands: 0,
                view: None,
            },
        );
        Ok(Request(id))
    }
    pub fn enter_path(&mut self, path: PathInput) -> Result<Request, Error> {
        if path.name.len() > 259 || u32::from_le_bytes(path.options[8..12].try_into().unwrap()) == 4
        {
            return Err(Error::Stale);
        }
        let request = self.enter()?;
        self.loads.get_mut(&request.0).unwrap().path = Some(path);
        Ok(request)
    }
    fn copied(&mut self, request: Request, stream: u64) -> Result<(), Error> {
        let load = self.loads.get_mut(&request.0).ok_or(Error::Unknown)?;
        if stream == 0
            || load.path.is_some()
            || load.stream.is_some()
            || load.outcome != Outcome::Entered
        {
            return Err(Error::Stale);
        }
        load.stream = Some(stream);
        load.stream_birth = Some(self.stream_birth);
        Ok(())
    }
    pub fn begin_copy(&mut self) -> Result<u64, Error> {
        let id = self.id()?;
        self.copy_active = Some(id);
        self.stream_pointer = 0;
        self.stream_birth = 0;
        Ok(id)
    }
    pub fn stream_constructed(&mut self, copy: u64, pointer: u64) -> Result<(), Error> {
        if self.copy_active != Some(copy) || pointer == 0 || self.stream_birth != 0 {
            return Err(Error::Stale);
        }
        self.stream_pointer = pointer;
        self.stream_birth = copy;
        Ok(())
    }
    pub fn end_copy(
        &mut self,
        copy: u64,
        request: Option<Request>,
        actual: u64,
    ) -> Result<(), Error> {
        if request.is_none() && self.copy_active != Some(copy) {
            return Ok(());
        }
        if self.copy_active != Some(copy) {
            return Err(Error::Stale);
        }
        if request.is_some()
            && (self.stream_birth != copy || self.stream_pointer != actual || actual == 0)
        {
            return Err(Error::Stale);
        }
        if let Some(request) = request {
            self.copied(request, actual)?;
        }
        self.copy_active = None;
        Ok(())
    }
    pub fn abandon_copy(&mut self, copy: u64) {
        if self.copy_active == Some(copy) {
            self.copy_active = None;
            self.stream_birth = 0;
            self.stream_pointer = 0;
        }
    }
    #[cfg(test)]
    pub fn fixture_copied(&mut self, request: Request, pointer: u64) -> Result<(), Error> {
        let copy = self.begin_copy()?;
        self.stream_constructed(copy, pointer)?;
        self.end_copy(copy, Some(request), pointer)
    }
    fn same_stream(&self, request: Request, pointer: u64) -> bool {
        self.loads.get(&request.0).is_some_and(|load| {
            load.stream == Some(pointer)
                && load.stream_birth == Some(self.stream_birth)
                && self.stream_pointer == pointer
                && self.stream_birth != 0
                && self.copy_active.is_none()
        })
    }
    pub fn issued(&mut self, request: Request, payload: u64, descriptor: u64) -> Result<(), Error> {
        let load = self.loads.get_mut(&request.0).ok_or(Error::Unknown)?;
        if payload == 0
            || descriptor == 0
            || (load.stream.is_none() && load.path.is_none())
            || load.outcome != Outcome::Entered
        {
            return Err(Error::Stale);
        }
        if self.commands.contains_key(&payload) || load.commands != 0 {
            return Err(Error::Busy);
        }
        load.commands = 1;
        self.commands.insert(
            payload,
            Command {
                request,
                descriptor,
                running: false,
            },
        );
        Ok(())
    }
    pub fn returned(&mut self, request: Request, accepted: bool) -> Result<(), Error> {
        let load = self.loads.get_mut(&request.0).ok_or(Error::Unknown)?;
        if load.outcome != Outcome::Entered {
            return Err(Error::Stale);
        }
        if !accepted && (load.stream.is_some() || load.commands != 0) {
            self.unresolved = true;
            return Err(Error::Unresolved);
        }
        load.outcome = if accepted {
            Outcome::Accepted
        } else {
            Outcome::Rejected
        };
        Ok(())
    }
    pub fn consuming(&mut self, payload: u64, descriptor: u64) -> Result<Option<Request>, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let Some(command) = self.commands.get_mut(&payload) else {
            return Ok(None);
        };
        if command.descriptor != descriptor || command.running {
            return Err(Error::Stale);
        }
        let load = self.loads.get(&command.request.0).ok_or(Error::Unknown)?;
        match load.outcome {
            // Native publication can race the producer's AL1 return. Keep
            // the provisional carrier, but grant no World relation yet.
            Outcome::Accepted | Outcome::Entered => {}
            Outcome::Rejected => return Err(Error::Rejected),
        }
        command.running = true;
        Ok(Some(command.request))
    }
    pub fn selected(
        &mut self,
        request: Request,
        payload: u64,
        engine: u64,
        view: u64,
    ) -> Result<(), Error> {
        let command = self.commands.get(&payload).ok_or(Error::Unknown)?;
        if command.request != request || !command.running || engine == 0 || view == 0 {
            return Err(Error::Stale);
        }
        if self.pending.contains_key(&engine) {
            return Err(Error::Busy);
        }
        self.pending.insert(engine, (view, request));
        Ok(())
    }
    pub fn consumed(&mut self, request: Request, payload: u64) -> Result<(), Error> {
        let command = self.commands.get(&payload).ok_or(Error::Unknown)?;
        if command.request != request || !command.running {
            return Err(Error::Stale);
        }
        // The old command's native bank storage may be reused only after its
        // complete thunk returns. Later callbacks keep the request ID.
        self.commands.remove(&payload);
        Ok(())
    }
    pub fn initialize(
        &mut self,
        engine: u64,
        pointer: u64,
        stream: u64,
    ) -> Result<Option<Invocation>, Error> {
        let Some((view, request)) = self.pending.get(&engine).copied() else {
            return Ok(None);
        };
        let matches = self.same_stream(request, stream);
        self.initialize_matching(engine, pointer, view, request, matches)
    }
    pub fn path_pending(&self, engine: u64) -> bool {
        self.pending.get(&engine).is_some_and(|(_, request)| {
            self.loads
                .get(&request.0)
                .is_some_and(|load| load.path.is_some())
        })
    }
    pub fn initialize_path(
        &mut self,
        engine: u64,
        pointer: u64,
        input: &PathInput,
    ) -> Result<Option<Invocation>, Error> {
        let Some((view, request)) = self.pending.get(&engine).copied() else {
            return Ok(None);
        };
        let matches = self
            .loads
            .get(&request.0)
            .is_some_and(|load| load.path.as_ref() == Some(input));
        self.initialize_matching(engine, pointer, view, request, matches)
    }
    fn initialize_matching(
        &mut self,
        engine: u64,
        pointer: u64,
        view: u64,
        request: Request,
        matches: bool,
    ) -> Result<Option<Invocation>, Error> {
        let load = self.loads.get(&request.0).ok_or(Error::Unknown)?;
        if view != pointer || !matches || load.outcome == Outcome::Rejected || load.view.is_some() {
            return Err(Error::Stale);
        }
        if self
            .current
            .get(&pointer)
            .and_then(|id| self.views.get(&id.0))
            .is_some_and(|v| {
                v.workers != 0
                    || v.reservations != 0
                    || !v.initializer_returned
                    || (v.started && !v.joined)
            })
        {
            return Err(Error::Busy);
        }
        let invocation = Invocation(self.id()?);
        self.views.insert(
            invocation.0,
            View {
                request,
                engine,
                pointer,
                workers: 0,
                reservations: 0,
                started: false,
                joined: false,
                initializer_returned: false,
                world: None,
                stream_read: false,
            },
        );
        self.current.insert(pointer, invocation);
        self.loads.get_mut(&request.0).unwrap().view = Some(invocation);
        self.pending.remove(&engine);
        Ok(Some(invocation))
    }
    pub fn initialized(&mut self, invocation: Invocation) -> Result<(), Error> {
        let v = self.views.get_mut(&invocation.0).ok_or(Error::Unknown)?;
        if v.initializer_returned {
            return Err(Error::Stale);
        }
        v.initializer_returned = true;
        Ok(())
    }
    pub fn worker(&mut self, pointer: u64) -> Result<Option<Invocation>, Error> {
        let Some(id) = self.current.get(&pointer).copied() else {
            return Ok(None);
        };
        let v = self.views.get_mut(&id.0).ok_or(Error::Unknown)?;
        if v.workers != 0 {
            return Err(Error::Busy);
        }
        v.workers = v.workers.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(Some(id))
    }
    pub fn reserve_worker(&mut self, invocation: Invocation) -> Result<(), Error> {
        let v = self.views.get_mut(&invocation.0).ok_or(Error::Unknown)?;
        if self.current.get(&v.pointer) != Some(&invocation) || v.reservations != 0 {
            return Err(Error::Busy);
        }
        v.reservations = 1;
        v.started = true;
        v.joined = false;
        Ok(())
    }
    pub fn current(&self, pointer: u64) -> Option<Invocation> {
        self.current.get(&pointer).copied()
    }
    pub fn joined(&mut self, invocation: Invocation) -> Result<(), Error> {
        let v = self.views.get_mut(&invocation.0).ok_or(Error::Unknown)?;
        if !v.started
            || v.joined
            || v.reservations != 0
            || v.workers != 0
            || !v.initializer_returned
        {
            return Err(Error::Busy);
        }
        v.joined = true;
        Ok(())
    }
    pub fn release_worker(&mut self, invocation: Invocation) -> Result<(), Error> {
        let v = self.views.get_mut(&invocation.0).ok_or(Error::Unknown)?;
        if v.reservations != 1 || v.workers != 0 {
            return Err(Error::Busy);
        }
        v.reservations = 0;
        Ok(())
    }
    pub fn allocation_failed(&mut self, invocation: Invocation) -> Result<(), Error> {
        let v = self.views.get_mut(&invocation.0).ok_or(Error::Unknown)?;
        if self.current.get(&v.pointer) != Some(&invocation)
            || v.reservations != 1
            || v.workers != 0
            || v.world.is_some()
        {
            return Err(Error::Busy);
        }
        v.reservations = 0;
        v.started = false;
        v.joined = false;
        Ok(())
    }
    pub fn worker_returned(&mut self, invocation: Invocation) -> Result<(), Error> {
        let v = self.views.get_mut(&invocation.0).ok_or(Error::Unknown)?;
        if v.workers != 1 {
            return Err(Error::Stale);
        }
        v.workers = 0;
        Ok(())
    }
    pub fn request(&self, invocation: Invocation) -> Result<Request, Error> {
        if self.unresolved {
            return Err(Error::Unresolved);
        }
        let v = self.views.get(&invocation.0).ok_or(Error::Unknown)?;
        if self.current.get(&v.pointer) != Some(&invocation) || v.workers != 1 || v.engine == 0 {
            return Err(Error::Stale);
        }
        Ok(v.request)
    }
    pub fn world(
        &mut self,
        invocation: Invocation,
        serial: u64,
        epoch: u64,
    ) -> Result<Request, Error> {
        let request = self.request(invocation)?;
        let v = self.views.get_mut(&invocation.0).unwrap();
        if serial == 0 || epoch == 0 || v.world.is_some() {
            return Err(Error::Stale);
        }
        v.world = Some((serial, epoch));
        Ok(request)
    }
    pub fn stream_read(&mut self, invocation: Invocation, stream: u64) -> Result<(), Error> {
        let request = self.request(invocation)?;
        if !self.same_stream(request, stream) {
            return Err(Error::Stale);
        }
        self.views.get_mut(&invocation.0).unwrap().stream_read = true;
        Ok(())
    }
    pub fn unresolved(&mut self) {
        self.unresolved = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn accepted(l: &mut Ledger, stream: u64, payload: u64) -> Request {
        let r = l.enter().unwrap();
        l.fixture_copied(r, stream).unwrap();
        l.issued(r, payload, 36).unwrap();
        l.returned(r, true).unwrap();
        r
    }
    #[test]
    fn publication_can_race_acceptance_but_cannot_issue_world_authority() {
        let mut l = Ledger::default();
        let r = l.enter().unwrap();
        l.fixture_copied(r, 10).unwrap();
        l.issued(r, 20, 36).unwrap();
        assert_eq!(l.consuming(20, 36), Ok(Some(r)));
        l.selected(r, 20, 30, 40).unwrap();
        l.consumed(r, 20).unwrap();
        let v = l.initialize(30, 40, 10).unwrap().unwrap();
        l.worker(40).unwrap();
        // This records a pending birth. The runtime binds it to a Session
        // only after the original request's independent AL1 outcome.
        assert_eq!(l.world(v, 2, 1), Ok(r));
        l.returned(r, true).unwrap();
        assert_eq!(l.consuming(20, 36), Ok(None));
    }
    #[test]
    fn busy_rejection_does_not_publish_a_request_or_adopt_a_view() {
        let mut l = Ledger::default();
        let r = l.enter().unwrap();
        l.returned(r, false).unwrap();
        assert_eq!(l.consuming(20, 36), Ok(None));
        assert_eq!(l.initialize(1, 2, 10), Ok(None));
        assert_eq!(l.fixture_copied(r, 10), Err(Error::Stale));
    }
    #[test]
    fn a_failed_request_after_publication_is_sticky_unresolved() {
        let mut l = Ledger::default();
        let r = l.enter().unwrap();
        l.fixture_copied(r, 10).unwrap();
        assert_eq!(l.returned(r, false), Err(Error::Unresolved));
        assert_eq!(l.enter(), Err(Error::Unresolved));
    }
    #[test]
    fn pathname_intent_requires_its_issued_command_and_matching_native_configuration() {
        let mut l = Ledger::default();
        let input = PathInput {
            options: [0; 28],
            name: "ordinary.Civ6Save".encode_utf16().collect(),
        };
        assert_eq!(l.initialize_path(30, 40, &input), Ok(None));
        let r = l.enter_path(input.clone()).unwrap();
        assert_eq!(l.initialize_path(30, 40, &input), Ok(None));
        l.issued(r, 20, 36).unwrap();
        l.returned(r, true).unwrap();
        l.consuming(20, 36).unwrap();
        l.selected(r, 20, 30, 40).unwrap();
        l.consumed(r, 20).unwrap();
        assert!(l.path_pending(30));
        assert_eq!(l.initialize(30, 40, 0), Err(Error::Stale));
        let mut changed = input.clone();
        changed.name[0] ^= 1;
        assert_eq!(l.initialize_path(30, 40, &changed), Err(Error::Stale));
        changed = input.clone();
        changed.options[4] ^= 1;
        assert_eq!(l.initialize_path(30, 40, &changed), Err(Error::Stale));
        let v = l.initialize_path(30, 40, &input).unwrap().unwrap();
        assert_eq!(l.world(v, 2, 1), Err(Error::Stale));
        l.worker(40).unwrap();
        l.initialized(v).unwrap();
        assert_eq!(l.world(v, 2, 1), Ok(r));
        assert_eq!(l.stream_read(v, 10), Err(Error::Stale));
        l.worker_returned(v).unwrap();
    }
    #[test]
    fn native_command_view_and_stream_keep_one_request_incarnation() {
        let mut l = Ledger::default();
        let r = accepted(&mut l, 10, 20);
        assert_eq!(l.consuming(20, 37), Err(Error::Stale));
        l.consuming(20, 36).unwrap();
        l.selected(r, 20, 30, 40).unwrap();
        assert_eq!(l.initialize(30, 40, 11), Err(Error::Stale));
        l.consumed(r, 20).unwrap();
        let v = l.initialize(30, 40, 10).unwrap().unwrap();
        l.worker(40).unwrap();
        l.initialized(v).unwrap();
        assert_eq!(l.world(v, 2, 1), Ok(r));
        l.stream_read(v, 10).unwrap();
        assert_eq!(l.world(v, 3, 2), Err(Error::Stale));
        l.worker_returned(v).unwrap();
    }
    #[test]
    fn bank_payload_and_load_view_reuse_never_rebind_an_old_worker() {
        let mut l = Ledger::default();
        let old = accepted(&mut l, 10, 20);
        l.consuming(20, 36).unwrap();
        l.selected(old, 20, 30, 40).unwrap();
        l.consumed(old, 20).unwrap();
        let ov = l.initialize(30, 40, 10).unwrap().unwrap();
        l.worker(40).unwrap();
        l.initialized(ov).unwrap();
        let new = accepted(&mut l, 11, 20);
        assert_ne!(old, new);
        l.consuming(20, 36).unwrap();
        l.selected(new, 20, 30, 40).unwrap();
        l.consumed(new, 20).unwrap();
        assert_eq!(l.initialize(30, 40, 11), Err(Error::Busy));
        l.worker_returned(ov).unwrap();
        let nv = l.initialize(30, 40, 11).unwrap().unwrap();
        assert_ne!(ov, nv);
        l.worker(40).unwrap();
        assert_eq!(l.request(ov), Err(Error::Stale));
        assert_eq!(l.request(nv), Ok(new));
        assert_eq!(l.stream_read(nv, 10), Err(Error::Stale));
    }
    #[test]
    fn destroying_task_does_not_replace_same_group_native_wait_receipt() {
        let mut l = Ledger::default();
        let r = accepted(&mut l, 10, 20);
        l.consuming(20, 36).unwrap();
        l.selected(r, 20, 30, 40).unwrap();
        l.consumed(r, 20).unwrap();
        let old = l.initialize(30, 40, 10).unwrap().unwrap();
        l.reserve_worker(old).unwrap();
        l.initialized(old).unwrap();
        let new = accepted(&mut l, 11, 20);
        l.consuming(20, 36).unwrap();
        l.selected(new, 20, 30, 40).unwrap();
        l.consumed(new, 20).unwrap();
        assert_eq!(l.joined(old), Err(Error::Busy));
        l.release_worker(old).unwrap();
        assert_eq!(l.initialize(30, 40, 11), Err(Error::Busy));
        l.joined(old).unwrap();
        let next = l.initialize(30, 40, 11).unwrap().unwrap();
        assert_ne!(old, next);
        assert_eq!(l.joined(old), Err(Error::Busy));
    }
    #[test]
    fn terminal_unwind_cannot_create_acceptance_or_a_new_binding() {
        let mut l = Ledger::default();
        let r = accepted(&mut l, 10, 20);
        l.consuming(20, 36).unwrap();
        l.unresolved();
        assert_eq!(l.enter(), Err(Error::Unresolved));
        // A native failure is never treated as successful cancellation.
        assert_eq!(l.consuming(20, 36), Err(Error::Unresolved));
        assert_ne!(r.0, 0);
    }
    #[test]
    fn reused_stream_address_cannot_satisfy_an_old_request_incarnation() {
        let mut l = Ledger::default();
        let old = accepted(&mut l, 10, 20);
        l.consuming(20, 36).unwrap();
        l.selected(old, 20, 30, 40).unwrap();
        l.consumed(old, 20).unwrap();
        let newer = l.enter().unwrap();
        l.fixture_copied(newer, 10).unwrap();
        l.returned(newer, true).unwrap();
        assert_eq!(l.initialize(30, 40, 10), Err(Error::Stale));
        let mut other = Ledger::default();
        let r = other.enter().unwrap();
        let first = other.begin_copy().unwrap();
        other.stream_constructed(first, 10).unwrap();
        let second = other.begin_copy().unwrap();
        other.stream_constructed(second, 10).unwrap();
        other.end_copy(second, None, 10).unwrap();
        assert_eq!(other.end_copy(first, Some(r), 10), Err(Error::Stale));
    }
}
