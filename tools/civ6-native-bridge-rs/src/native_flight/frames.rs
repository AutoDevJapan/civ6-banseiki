//! Full synchronous native frames, separate from dispatch/job ownership.
//!
//! Entry never authorizes skipping native code. A new root after sealing is
//! tracked and poisons the drain, even if it returns before the waiter runs.
use super::Error;
use std::{
    collections::HashMap,
    sync::{Condvar, Mutex},
    thread::{self, ThreadId},
    time::Instant,
};

struct State {
    generation: u64,
    sealed: bool,
    executions_drained: bool,
    unresolved: bool,
    next: u64,
    frames: HashMap<u64, (u64, ThreadId)>,
    jobs: HashMap<u64, Job>,
}
struct Job {
    generation: u64,
    payload: u64,
    task: u64,
    push_returned: bool,
    execution: u8,
    set: u64,
    queue_parent: Option<u64>,
    retiring: bool,
    executing_thread: Option<ThreadId>,
}

/// Accounts only for installed full-frame gateways. It is not a producer seal,
/// pending-list receipt, same-set Wait receipt, or all-domain certificate.
pub struct NativeFrames {
    state: Mutex<State>,
    changed: Condvar,
}

impl Default for NativeFrames {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeFrames {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                generation: 1,
                sealed: false,
                executions_drained: false,
                unresolved: false,
                next: 0,
                frames: HashMap::new(),
                jobs: HashMap::new(),
            }),
            changed: Condvar::new(),
        }
    }

    /// Called before the original's first instruction. Reentry on the same
    /// native stack inherits its root's epoch, including after closure.
    pub fn enter(&self) -> Result<u64, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        let tid = thread::current().id();
        let inherited = Self::inherited(&s, tid);
        if s.executions_drained || (s.sealed && !inherited) {
            s.unresolved = true;
        }
        let id = s.next.checked_add(1).ok_or(Error::Exhausted)?;
        s.next = id;
        let generation = s.generation;
        s.frames.insert(id, (generation, tid));
        self.changed.notify_all();
        Ok(id)
    }

    /// A gateway clears its stack cookie before calling this on normal return
    /// or terminal SEH. A stale/duplicate/wrong-thread cookie cannot settle it.
    pub fn leave(&self, id: u64) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.frames.get(&id) != Some(&(s.generation, thread::current().id())) {
            s.unresolved = true;
            self.changed.notify_all();
            return Err(Error::Stale);
        }
        s.frames.remove(&id);
        self.changed.notify_all();
        Ok(())
    }

    pub fn seal_and_wait(&self, deadline: Instant) -> Result<(), Error> {
        self.seal_and_wait_for(deadline, true)
    }

    /// Before destructive native cleanup, wait for every executing frame and
    /// Host job. Inert serialized nodes must survive until that original
    /// cleanup cancels/frees them. This is not a storage-drained receipt.
    pub fn seal_executions_and_wait(&self, deadline: Instant) -> Result<(), Error> {
        self.seal_and_wait_for(deadline, false)
    }

    fn seal_and_wait_for(&self, deadline: Instant, include_storage: bool) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        s.sealed = true;
        loop {
            if s.unresolved {
                return Err(Error::Unresolved);
            }
            if s.frames
                .values()
                .any(|(_, tid)| *tid == thread::current().id())
            {
                return Err(Error::Reentrant);
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|d| !d.is_zero())
                .ok_or(Error::Timeout)?;
            let jobs_drained = s.jobs.values().all(|j| {
                !include_storage
                    && j.set == u64::MAX
                    && j.execution != 1
                    && !j.retiring
                    && j.push_returned
            });
            if s.frames.is_empty() && jobs_drained {
                if !include_storage {
                    s.executions_drained = true;
                }
                return Ok(());
            }
            let (next, timeout) = self
                .changed
                .wait_timeout(s, remaining)
                .map_err(|_| Error::Poisoned)?;
            s = next;
            if timeout.timed_out() {
                return Err(Error::Timeout);
            }
        }
    }

    fn inherited(s: &State, tid: ThreadId) -> bool {
        s.frames.values().any(|(_, t)| *t == tid)
            || s.jobs.values().any(|j| j.executing_thread == Some(tid))
    }

    /// A Host consumer inherits only an already-issued T's generation. It
    /// retains a separate complete-frame hold after that T has completed.
    pub fn consumer_enter(&self, task: u64) -> Result<Option<u64>, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.unresolved {
            return Err(Error::Unresolved);
        }
        let job = s
            .jobs
            .values()
            .find(|j| j.task == task && task != 0 && j.set != u64::MAX);
        let Some(job) = job else {
            return Ok(None);
        };
        if job.generation != s.generation {
            s.unresolved = true;
            return Err(Error::Stale);
        }
        let id = s.next.checked_add(1).ok_or(Error::Exhausted)?;
        s.next = id;
        let generation = s.generation;
        s.frames.insert(id, (generation, thread::current().id()));
        Ok(Some(id))
    }

    pub fn consumer_attach(&self, hold: u64, task: u64) -> Result<bool, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.unresolved || s.frames.get(&hold) != Some(&(s.generation, thread::current().id())) {
            s.unresolved = true;
            return Err(Error::Stale);
        }
        let Some(job) = s
            .jobs
            .values()
            .find(|j| j.task == task && task != 0 && j.set != u64::MAX)
        else {
            return Ok(false);
        };
        if job.generation != s.generation {
            s.unresolved = true;
            return Err(Error::Stale);
        }
        Ok(true)
    }

    pub fn consumer_exit(&self, hold: u64) -> Result<(), Error> {
        self.leave(hold)
    }

    pub fn reopen(&self) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.unresolved {
            return Err(Error::Unresolved);
        }
        if !s.sealed || !s.frames.is_empty() || !s.jobs.is_empty() {
            return Err(Error::Busy);
        }
        s.generation = s.generation.checked_add(1).ok_or(Error::Exhausted)?;
        s.sealed = false;
        s.executions_drained = false;
        Ok(())
    }

    pub fn healthy(&self) -> bool {
        self.state.lock().is_ok_and(|s| !s.unresolved)
    }
    pub fn mark_unresolved(&self) {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        s.unresolved = true;
        self.changed.notify_all();
    }
    pub fn contains_payload(&self, payload: u64) -> bool {
        self.state
            .lock()
            .is_ok_and(|s| s.jobs.values().any(|j| j.payload == payload))
    }
    pub fn contains_task(&self, task: u64) -> bool {
        self.state
            .lock()
            .is_ok_and(|s| s.jobs.values().any(|j| j.task == task && task != 0))
    }

    /// Reserve before the original host Push can create or publish native T.
    /// Addresses are opaque keys, never retained references to native objects.
    pub fn issue(&self, payload: u64) -> Result<u64, Error> {
        self.issue_on_set(payload, 1)
    }
    pub fn issue_on_set(&self, payload: u64, set: u64) -> Result<u64, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if payload == 0
            || set == 0
            || s.unresolved
            || s.executions_drained
            || (s.sealed && !Self::inherited(&s, thread::current().id()))
            || s.jobs.values().any(|j| j.payload == payload)
        {
            s.unresolved = true;
            return Err(Error::Unresolved);
        }
        let id = s.next.checked_add(1).ok_or(Error::Exhausted)?;
        s.next = id;
        let generation = s.generation;
        s.jobs.insert(
            id,
            Job {
                generation,
                payload,
                task: 0,
                push_returned: false,
                execution: 0,
                set,
                queue_parent: None,
                retiring: false,
                executing_thread: None,
            },
        );
        Ok(id)
    }
    pub fn pending_on_set(&self, set: u64) -> Result<usize, Error> {
        let s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.unresolved {
            return Err(Error::Unresolved);
        }
        Ok(s.jobs.values().filter(|j| j.set == set).count())
    }

    /// R8 is the original wrapper's checked output T. It may already have
    /// started/finished on another thread before Push returns.
    pub fn pushed(&self, id: u64, task: u64) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        let generation = s.generation;
        let valid = s.jobs.get(&id).is_some_and(|j| {
            j.generation == generation
                && !j.push_returned
                && task != 0
                && (j.task == 0 || j.task == task)
        });
        if !valid {
            s.unresolved = true;
            self.changed.notify_all();
            return Err(Error::Unresolved);
        }
        let j = s.jobs.get_mut(&id).unwrap();
        j.task = task;
        j.push_returned = true;
        if (j.execution == 2 && j.set != u64::MAX) || (j.execution == 3 && !j.retiring) {
            s.jobs.remove(&id);
        }
        self.changed.notify_all();
        Ok(())
    }
    /// Bind the allocator's original result before caller initialization or
    /// publication. The allocator frame supplies only an opaque live address.
    pub fn bind_before_publication(&self, id: u64, task: u64) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        let valid = task != 0
            && s.jobs
                .get(&id)
                .is_some_and(|j| j.generation == s.generation && j.task == 0 && !j.push_returned)
            && !s.jobs.values().any(|j| j.task == task);
        if !valid {
            s.unresolved = true;
            return Err(Error::Unresolved);
        }
        s.jobs.get_mut(&id).unwrap().task = task;
        Ok(())
    }
    pub fn reserve_queue(&self) -> Result<u64, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.sealed || s.unresolved {
            s.unresolved = true;
            return Err(Error::Unresolved);
        }
        let id = s.next.checked_add(1).ok_or(Error::Exhausted)?;
        s.next = id;
        let generation = s.generation;
        s.jobs.insert(
            id,
            Job {
                generation,
                payload: 0,
                task: 0,
                push_returned: false,
                execution: 0,
                set: u64::MAX,
                queue_parent: None,
                retiring: false,
                executing_thread: None,
            },
        );
        Ok(id)
    }

    /// One complete native producer may allocate multiple nodes. Its frame
    /// holds every child until the producer's original normal return.
    pub fn begin_queue_producer(&self) -> Result<u64, Error> {
        let id = self.enter()?;
        if self.healthy() {
            Ok(id)
        } else {
            Err(Error::Unresolved)
        }
    }

    /// The verified shutdown control frame owns this original destructive
    /// cleanup. It may free inert storage after execution admission closes,
    /// while its own lifetime must still block the final drain.
    pub fn begin_native_cleanup(&self) -> Result<u64, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.unresolved || !s.executions_drained {
            return Err(Error::Unresolved);
        }
        let id = s.next.checked_add(1).ok_or(Error::Exhausted)?;
        s.next = id;
        let generation = s.generation;
        s.frames.insert(id, (generation, thread::current().id()));
        Ok(id)
    }

    /// Called at the allocator return, before initialization or shared links.
    /// Copies inherit the recorded source node; they never acquire a fresh
    /// epoch at the later dequeue. Both addresses remain opaque keys.
    pub fn queue_allocated(
        &self,
        producer: u64,
        task: u64,
        source: Option<u64>,
    ) -> Result<u64, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        let valid_source = source.is_none_or(|node| {
            s.jobs.values().any(|j| {
                j.task == node
                    && node != 0
                    && j.generation == s.generation
                    && j.set == u64::MAX
                    && !j.retiring
            })
        });
        if s.unresolved
            || s.executions_drained
            || s.frames.get(&producer) != Some(&(s.generation, thread::current().id()))
            || task == 0
            || !valid_source
            || s.jobs.values().any(|j| j.task == task)
        {
            s.unresolved = true;
            self.changed.notify_all();
            return Err(Error::Unresolved);
        }
        let id = s.next.checked_add(1).ok_or(Error::Exhausted)?;
        s.next = id;
        let generation = s.generation;
        s.jobs.insert(
            id,
            Job {
                generation,
                payload: 0,
                task,
                push_returned: false,
                execution: 0,
                set: u64::MAX,
                queue_parent: Some(producer),
                retiring: false,
                executing_thread: None,
            },
        );
        Ok(id)
    }

    pub fn queue_producer_returned(&self, producer: u64) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.frames.get(&producer) != Some(&(s.generation, thread::current().id())) {
            s.unresolved = true;
            return Err(Error::Stale);
        }
        s.jobs.retain(|_, j| {
            if j.queue_parent == Some(producer) {
                j.queue_parent = None;
                j.push_returned = true;
                j.execution != 3 || j.retiring
            } else {
                true
            }
        });
        s.frames.remove(&producer);
        self.changed.notify_all();
        Ok(())
    }

    /// The original node-free call owns the address until it frees it. Release
    /// its lookup key at entry, but retain the reservation through normal
    /// return. An allocator may reuse the address before this callback returns.
    pub fn retire_queue(&self, task: u64) -> Result<u64, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        let id = s.jobs.iter().find_map(|(id, j)| {
            (j.task == task
                && task != 0
                && j.set == u64::MAX
                && j.generation == s.generation
                && !j.retiring
                && j.execution != 1)
                .then_some(*id)
        });
        let Some(id) = id else {
            s.unresolved = true;
            return Err(Error::Unresolved);
        };
        let j = s.jobs.get_mut(&id).unwrap();
        j.task = 0;
        j.retiring = true;
        Ok(id)
    }

    pub fn queue_retired(&self, id: u64) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.jobs.get(&id).is_none_or(|j| {
            j.generation != s.generation || j.set != u64::MAX || !j.retiring || j.execution == 1
        }) {
            s.unresolved = true;
            return Err(Error::Unresolved);
        }
        let j = s.jobs.get_mut(&id).unwrap();
        j.retiring = false;
        j.execution = 3;
        if j.push_returned {
            s.jobs.remove(&id);
        }
        self.changed.notify_all();
        Ok(())
    }
    pub fn finish_unpublished_queue(&self, id: u64) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.jobs
            .get(&id)
            .is_none_or(|j| j.task != 0 || j.push_returned)
        {
            s.unresolved = true;
            return Err(Error::Unresolved);
        }
        s.jobs.remove(&id);
        self.changed.notify_all();
        Ok(())
    }
    pub fn queue_published(&self, id: u64) -> Result<(), Error> {
        let task = {
            let s = self.state.lock().map_err(|_| Error::Poisoned)?;
            s.jobs.get(&id).ok_or(Error::Unresolved)?.task
        };
        if task == 0 {
            self.finish_unpublished_queue(id)
        } else {
            self.pushed(id, task)
        }
    }
    pub fn queue_execute(&self, task: u64) -> Result<u64, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.executions_drained {
            s.unresolved = true;
            return Err(Error::Unresolved);
        }
        let id = s.jobs.iter().find_map(|(id, j)| {
            (j.task == task
                && task != 0
                && j.set == u64::MAX
                && j.execution == 0
                && j.generation == s.generation)
                .then_some(*id)
        });
        let Some(id) = id else {
            s.unresolved = true;
            return Err(Error::Unresolved);
        };
        s.jobs.get_mut(&id).unwrap().execution = 1;
        s.jobs.get_mut(&id).unwrap().executing_thread = Some(thread::current().id());
        Ok(id)
    }

    /// The current execution invocation supplies both T and its current P.
    /// Never adopt the newest generation for an unrecorded native task.
    pub fn execute(&self, task: u64, payload: u64) -> Result<u64, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        let id = s.jobs.iter().find_map(|(id, j)| {
            (j.payload == payload
                && j.generation == s.generation
                && j.execution == 0
                && (j.task == 0 || j.task == task)
                && task != 0)
                .then_some(*id)
        });
        let Some(id) = id else {
            s.unresolved = true;
            return Err(Error::Unresolved);
        };
        let j = s.jobs.get_mut(&id).unwrap();
        j.task = task;
        j.execution = 1;
        j.executing_thread = Some(thread::current().id());
        Ok(id)
    }
    pub fn completed(&self, id: u64) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.jobs.get(&id).is_none_or(|j| {
            j.execution != 1
                || j.generation != s.generation
                || j.executing_thread != Some(thread::current().id())
        }) {
            s.unresolved = true;
            return Err(Error::Unresolved);
        }
        let j = s.jobs.get_mut(&id).unwrap();
        j.execution = 2;
        j.executing_thread = None;
        if j.push_returned && j.set != u64::MAX {
            s.jobs.remove(&id);
        }
        self.changed.notify_all();
        Ok(())
    }
    /// A normal T destructor may settle an unexecuted, already bound T. A
    /// destructor before binding/while executing remains unresolved; timeout
    /// or dropped accounting never cancels a job.
    pub fn cancelled(&self, task: u64) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if let Some(id) = s
            .jobs
            .iter()
            .find_map(|(id, j)| (j.task == task && task != 0 && j.set != u64::MAX).then_some(*id))
        {
            let j = &s.jobs[&id];
            if j.execution != 0 {
                s.unresolved = true;
                return Err(Error::Unresolved);
            }
            if j.push_returned {
                s.jobs.remove(&id);
            } else {
                s.jobs.get_mut(&id).unwrap().execution = 3;
            }
            self.changed.notify_all();
        }
        Ok(())
    }

    /// Grant closure is not a native producer seal. Keep old executions and
    /// dispatches in the same epoch until the later native cleanup boundary.
    pub fn request_close(&self) -> Result<(), Error> {
        let s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.unresolved {
            Err(Error::Unresolved)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{mpsc, Arc},
        time::Duration,
    };
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(5)
    }

    #[test]
    fn inner_completion_cannot_drain_outer_frame_or_its_reentry() {
        let frames = Arc::new(NativeFrames::new());
        let (tx, rx) = mpsc::channel();
        let (resume, wait) = mpsc::channel();
        let native = frames.clone();
        let thread = thread::spawn(move || {
            let outer = native.enter().unwrap();
            let inner = native.enter().unwrap();
            native.leave(inner).unwrap();
            tx.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            let repeated = native.enter().unwrap();
            native.leave(repeated).unwrap();
            native.leave(outer).unwrap();
        });
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        // An expired deadline seals deterministically without sleeping.
        assert_eq!(frames.seal_and_wait(Instant::now()), Err(Error::Timeout));
        assert_eq!(frames.reopen(), Err(Error::Busy));
        resume.send(()).unwrap();
        thread.join().unwrap();
        frames.seal_and_wait(deadline()).unwrap();
        frames.reopen().unwrap();
    }

    #[test]
    fn late_native_root_runs_but_cannot_be_erased_by_return_or_rebuild() {
        let frames = NativeFrames::new();
        frames.seal_and_wait(deadline()).unwrap();
        let late = frames.enter().unwrap();
        frames.leave(late).unwrap();
        assert_eq!(frames.seal_and_wait(deadline()), Err(Error::Unresolved));
        assert_eq!(frames.reopen(), Err(Error::Unresolved));
    }

    #[test]
    fn reentrant_close_and_wrong_thread_release_never_drain() {
        let frames = Arc::new(NativeFrames::new());
        let id = frames.enter().unwrap();
        assert_eq!(frames.seal_and_wait(deadline()), Err(Error::Reentrant));
        let other = frames.clone();
        assert_eq!(
            thread::spawn(move || other.leave(id)).join().unwrap(),
            Err(Error::Stale)
        );
        frames.leave(id).unwrap();
        assert_eq!(frames.reopen(), Err(Error::Unresolved));
    }

    #[test]
    fn old_queued_job_blocks_reopen_and_keeps_its_original_reservation() {
        let f = NativeFrames::new();
        let job = f.issue(100).unwrap();
        f.pushed(job, 200).unwrap();
        f.request_close().unwrap();
        assert_eq!(f.seal_and_wait(Instant::now()), Err(Error::Timeout));
        assert_eq!(f.reopen(), Err(Error::Busy));
        assert_eq!(f.execute(200, 100), Ok(job));
        f.completed(job).unwrap();
        f.seal_and_wait(deadline()).unwrap();
        f.reopen().unwrap();
        assert_eq!(f.execute(200, 100), Err(Error::Unresolved));
    }

    #[test]
    fn fast_execution_before_push_return_cannot_open_an_intermediate_zero() {
        let f = NativeFrames::new();
        let job = f.issue(100).unwrap();
        assert_eq!(f.execute(200, 100), Ok(job));
        f.completed(job).unwrap();
        assert_eq!(f.seal_and_wait(Instant::now()), Err(Error::Timeout));
        assert_eq!(f.reopen(), Err(Error::Busy));
        f.pushed(job, 200).unwrap();
        f.seal_and_wait(deadline()).unwrap();
    }
    #[test]
    fn unexecuted_cancel_before_push_return_keeps_reservation_until_original_return() {
        let f = NativeFrames::new();
        let id = f.issue_on_set(100, 10).unwrap();
        f.bind_before_publication(id, 200).unwrap();
        f.cancelled(200).unwrap();
        assert_eq!(f.pending_on_set(10), Ok(1));
        assert_eq!(f.seal_and_wait(Instant::now()), Err(Error::Timeout));
        assert_eq!(f.reopen(), Err(Error::Busy));
        f.pushed(id, 200).unwrap();
        f.seal_and_wait(deadline()).unwrap();
        f.reopen().unwrap();
    }
    #[test]
    fn serialized_operation_is_reserved_before_its_node_is_published() {
        let f = NativeFrames::new();
        let id = f.reserve_queue().unwrap();
        f.bind_before_publication(id, 123).unwrap();
        f.queue_published(id).unwrap();
        assert_eq!(f.seal_and_wait(Instant::now()), Err(Error::Timeout));
        assert_eq!(f.reopen(), Err(Error::Busy));
        assert_eq!(f.queue_execute(123), Ok(id));
        f.completed(id).unwrap();
        let retiring = f.retire_queue(123).unwrap();
        assert_eq!(retiring, id);
        f.queue_retired(retiring).unwrap();
        f.seal_and_wait(deadline()).unwrap();
        f.reopen().unwrap();
        assert_eq!(f.queue_execute(123), Err(Error::Unresolved));
    }

    #[test]
    fn serialized_execution_return_alone_does_not_retire_native_queue_node() {
        let f = NativeFrames::new();
        let id = f.reserve_queue().unwrap();
        f.bind_before_publication(id, 123).unwrap();
        f.queue_published(id).unwrap();
        assert_eq!(f.queue_execute(123), Ok(id));
        f.completed(id).unwrap();
        assert!(f.contains_task(123));
        assert_eq!(f.seal_and_wait(Instant::now()), Err(Error::Timeout));
        assert_eq!(f.reopen(), Err(Error::Busy));
    }

    #[test]
    fn producer_retains_multiple_nodes_and_early_retirement_until_its_return() {
        let f = NativeFrames::new();
        let producer = f.begin_queue_producer().unwrap();
        let first = f.queue_allocated(producer, 123, None).unwrap();
        let second = f.queue_allocated(producer, 456, Some(123)).unwrap();
        assert_eq!(f.retire_queue(123), Ok(first));
        f.queue_retired(first).unwrap();
        assert_eq!(f.retire_queue(456), Ok(second));
        f.queue_retired(second).unwrap();
        assert_eq!(f.seal_and_wait(deadline()), Err(Error::Reentrant));
        f.queue_producer_returned(producer).unwrap();
        f.seal_and_wait(deadline()).unwrap();
        f.reopen().unwrap();
        assert_eq!(f.queue_execute(123), Err(Error::Unresolved));
    }

    #[test]
    fn allocator_reuse_during_old_free_keeps_two_distinct_reservations() {
        let f = NativeFrames::new();
        let first_producer = f.begin_queue_producer().unwrap();
        let old = f.queue_allocated(first_producer, 123, None).unwrap();
        f.queue_producer_returned(first_producer).unwrap();
        assert_eq!(f.retire_queue(123), Ok(old));
        let next_producer = f.begin_queue_producer().unwrap();
        let new = f.queue_allocated(next_producer, 123, None).unwrap();
        assert_ne!(old, new);
        f.queue_producer_returned(next_producer).unwrap();
        f.queue_retired(old).unwrap();
        assert!(f.contains_task(123));
        assert_eq!(f.queue_execute(123), Ok(new));
        f.completed(new).unwrap();
        assert_eq!(f.retire_queue(123), Ok(new));
        f.queue_retired(new).unwrap();
        f.seal_and_wait(deadline()).unwrap();
    }

    #[test]
    fn unknown_copy_source_never_receives_the_current_epoch() {
        let f = NativeFrames::new();
        let producer = f.begin_queue_producer().unwrap();
        assert_eq!(
            f.queue_allocated(producer, 456, Some(123)),
            Err(Error::Unresolved)
        );
        f.queue_producer_returned(producer).unwrap();
        assert_eq!(f.seal_and_wait(deadline()), Err(Error::Unresolved));
        assert_eq!(f.reopen(), Err(Error::Unresolved));
    }

    #[test]
    fn execution_drain_keeps_archive_for_original_cleanup_and_final_drain() {
        let f = NativeFrames::new();
        let p = f.begin_queue_producer().unwrap();
        f.queue_allocated(p, 123, None).unwrap();
        f.queue_producer_returned(p).unwrap();
        f.seal_executions_and_wait(deadline()).unwrap();
        assert_eq!(f.reopen(), Err(Error::Busy));
        let cleanup = f.begin_native_cleanup().unwrap();
        let r = f.retire_queue(123).unwrap();
        f.queue_retired(r).unwrap();
        assert_eq!(f.seal_and_wait(deadline()), Err(Error::Reentrant));
        f.leave(cleanup).unwrap();
        f.seal_and_wait(deadline()).unwrap();
        f.reopen().unwrap();
    }

    #[test]
    fn completed_execution_phase_never_allows_archived_work_to_start() {
        let f = NativeFrames::new();
        let p = f.begin_queue_producer().unwrap();
        f.queue_allocated(p, 123, None).unwrap();
        f.queue_producer_returned(p).unwrap();
        f.seal_executions_and_wait(deadline()).unwrap();
        assert_eq!(f.queue_execute(123), Err(Error::Unresolved));
        assert_eq!(f.reopen(), Err(Error::Unresolved));
    }
    #[test]
    fn original_cleanup_frame_never_authorizes_new_producer_reentry() {
        let f = NativeFrames::new();
        f.seal_executions_and_wait(deadline()).unwrap();
        let cleanup = f.begin_native_cleanup().unwrap();
        assert_eq!(f.issue_on_set(100, 10), Err(Error::Unresolved));
        f.leave(cleanup).unwrap();
        assert_eq!(f.reopen(), Err(Error::Unresolved));
        let g = NativeFrames::new();
        g.seal_executions_and_wait(deadline()).unwrap();
        let cleanup = g.begin_native_cleanup().unwrap();
        assert_eq!(g.begin_queue_producer(), Err(Error::Unresolved));
        g.leave(cleanup).unwrap();
        assert_eq!(g.reopen(), Err(Error::Unresolved));
    }

    #[test]
    fn known_t_consumer_frame_survives_execute_and_destructor_until_native_return() {
        let f = NativeFrames::new();
        assert_eq!(f.consumer_enter(123), Ok(None));
        let job = f.issue_on_set(100, 10).unwrap();
        f.bind_before_publication(job, 123).unwrap();
        let hold = f.consumer_enter(123).unwrap().unwrap();
        f.pushed(job, 123).unwrap();
        assert_eq!(f.execute(123, 100), Ok(job));
        f.completed(job).unwrap();
        assert_eq!(f.consumer_attach(hold, 999), Ok(false));
        f.cancelled(123).unwrap();
        assert_eq!(
            f.seal_executions_and_wait(deadline()),
            Err(Error::Reentrant)
        );
        assert_eq!(f.reopen(), Err(Error::Busy));
        f.consumer_exit(hold).unwrap();
        f.seal_and_wait(deadline()).unwrap();
        f.reopen().unwrap();
        assert_eq!(f.consumer_enter(123), Ok(None));
    }

    #[test]
    fn grant_close_does_not_prevent_native_shutdown_from_completing_work() {
        let f = NativeFrames::new();
        let job = f.issue(100).unwrap();
        f.pushed(job, 200).unwrap();
        f.request_close().unwrap();
        assert_eq!(f.execute(200, 100), Ok(job));
        f.completed(job).unwrap();
        let pending = f.issue(300).unwrap();
        f.pushed(pending, 400).unwrap();
        f.cancelled(400).unwrap();
        f.seal_and_wait(deadline()).unwrap();
        f.reopen().unwrap();
    }
}
