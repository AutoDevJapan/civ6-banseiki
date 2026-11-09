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
        let inherited = s.frames.values().any(|(_, thread)| *thread == tid);
        if s.sealed && !inherited {
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
            if s.frames.is_empty() && s.jobs.is_empty() {
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
        if payload == 0 || set == 0 || s.sealed || s.jobs.values().any(|j| j.payload == payload) {
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
        if j.execution == 2 {
            s.jobs.remove(&id);
        }
        self.changed.notify_all();
        Ok(())
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
        Ok(id)
    }
    pub fn completed(&self, id: u64) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Poisoned)?;
        if s.jobs
            .get(&id)
            .is_none_or(|j| j.execution != 1 || j.generation != s.generation)
        {
            s.unresolved = true;
            return Err(Error::Unresolved);
        }
        let j = s.jobs.get_mut(&id).unwrap();
        j.execution = 2;
        if j.push_returned {
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
            .find_map(|(id, j)| (j.task == task && task != 0).then_some(*id))
        {
            let j = &s.jobs[&id];
            if j.execution != 0 || !j.push_returned {
                s.unresolved = true;
                return Err(Error::Unresolved);
            }
            s.jobs.remove(&id);
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
