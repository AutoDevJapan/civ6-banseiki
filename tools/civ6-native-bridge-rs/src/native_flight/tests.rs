use super::*;
use std::{sync::mpsc, time::Duration};

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

fn pending_now(closing: Closing<'_>) {
    // Test outstanding accounting, not the independent expired-deadline path.
    // The first observation is always before the deadline, even if the OS
    // pauses this test before it acquires the mutex. A real Condvar timeout
    // (or the next observation after a spurious wake) bounds this assertion.
    let start = Instant::now();
    let limit = start + Duration::from_millis(20);
    let mut first = true;
    let result = closing.wait_with_clock(limit, || {
        if std::mem::replace(&mut first, false) {
            start
        } else {
            limit
        }
    });
    assert!(matches!(result, Err(Error::Timeout)));
}

#[test]
fn idle_generation_can_close_and_advance_without_a_scalar_entry_permit() {
    let world = WorldFlights::new(17).unwrap();
    let drained = world.close().unwrap().wait(deadline()).unwrap();
    assert_eq!(world.rebuild(drained), Ok(18));
    let current = world.dispatch().unwrap().enter().unwrap();
    assert_eq!(current.generation(), 18);
}

#[test]
fn dispatched_frame_delayed_across_close_cannot_become_a_new_world_entry() {
    let world = WorldFlights::new(17).unwrap();
    let dispatch = world.dispatch().unwrap();
    assert_eq!(dispatch.generation(), 17);
    pending_now(world.close().unwrap());
    assert!(matches!(world.dispatch(), Err(Error::Closed)));
    let caller = dispatch.enter().unwrap();
    assert_eq!(caller.generation(), 17);
    let job = caller.issue().unwrap().publish().unwrap();
    drop(caller);
    pending_now(world.close().unwrap());
    job.complete_after_native_join().unwrap();
    assert_eq!(
        world.rebuild(world.close().unwrap().wait(deadline()).unwrap()),
        Ok(18)
    );
    let fresh = world.dispatch().unwrap().enter().unwrap();
    assert_eq!(fresh.generation(), 18);
}

#[test]
fn losing_dispatched_frame_is_not_a_native_cancellation() {
    let world = WorldFlights::new(1).unwrap();
    let dispatch = world.dispatch().unwrap();
    let closing = world.close().unwrap();
    drop(dispatch);
    assert!(matches!(closing.wait(deadline()), Err(Error::Unresolved)));
}

#[test]
fn unsubmitted_dispatch_can_cancel_without_opening_admission() {
    let world = WorldFlights::new(1).unwrap();
    let dispatch = world.dispatch().unwrap();
    let closing = world.close().unwrap();
    dispatch.cancel_before_dispatch().unwrap();
    closing.wait(deadline()).unwrap();
    assert!(matches!(world.dispatch(), Err(Error::Closed)));
}

#[test]
fn inner_job_completion_does_not_drain_its_outer_native_caller() {
    let world = WorldFlights::new(1).unwrap();
    thread::scope(|scope| {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (leave_tx, leave_rx) = mpsc::channel();
        let world_ref = &world;
        let worker = scope.spawn(move || {
            let caller = world_ref.dispatch().unwrap().enter().unwrap();
            caller
                .issue()
                .unwrap()
                .publish()
                .unwrap()
                .complete_after_native_join()
                .unwrap();
            ready_tx.send(()).unwrap();
            leave_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            // This represents the original outer epilogue, after its final
            // receiver read, not the inner Wait/callback return.
            drop(caller);
        });
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let closing = world.close().unwrap();
        let late_rejected = matches!(world.dispatch(), Err(Error::Closed));
        let start = Instant::now();
        let limit = start + Duration::from_millis(20);
        let mut first = true;
        let result = closing.wait_with_clock(limit, || {
            if std::mem::replace(&mut first, false) {
                start
            } else {
                limit
            }
        });
        leave_tx.send(()).unwrap();
        worker.join().unwrap();
        assert!(late_rejected);
        assert!(matches!(result, Err(Error::Timeout)));
        let drained = world.close().unwrap().wait(deadline()).unwrap();
        assert_eq!(world.rebuild(drained), Ok(2));
    });
}

#[test]
fn pending_publication_survives_caller_exit_and_close() {
    let world = WorldFlights::new(5).unwrap();
    let caller = world.dispatch().unwrap().enter().unwrap();
    let pending = caller.issue().unwrap();
    drop(caller);
    pending_now(world.close().unwrap());
    assert_eq!(pending.generation(), 5);
    let job = pending.publish().unwrap();
    assert_eq!(job.generation(), 5);
    pending_now(world.close().unwrap());
    job.complete_after_native_join().unwrap();
    let drained = world.close().unwrap().wait(deadline()).unwrap();
    assert_eq!(world.rebuild(drained), Ok(6));
}

#[test]
fn admitted_outer_caller_can_reenter_after_seal_without_changing_generation() {
    let world = WorldFlights::new(9).unwrap();
    let caller = world.dispatch().unwrap().enter().unwrap();
    assert!(matches!(world.close(), Err(Error::Reentrant)));
    assert!(matches!(world.dispatch(), Err(Error::Closed)));
    // Native continuation is preserved: admitted caller finishes its own
    // second callback. Closure does not skip a native call or adopt epoch10.
    let repeated = caller.issue().unwrap();
    assert_eq!(repeated.generation(), 9);
    let job = repeated.publish().unwrap();
    job.complete_after_native_join().unwrap();
    drop(caller);
    let drained = world.close().unwrap().wait(deadline()).unwrap();
    assert_eq!(world.rebuild(drained), Ok(10));
}

#[test]
fn unpublished_and_published_jobs_are_both_required_to_settle() {
    // Keep the original settlement order and exercise its reverse too: either
    // category must independently keep the same old generation sealed.
    for cancel_pending_first in [false, true] {
        let world = WorldFlights::new(1).unwrap();
        let caller = world.dispatch().unwrap().enter().unwrap();
        let pending = caller.issue().unwrap();
        let published = caller.issue().unwrap().publish().unwrap();
        drop(caller);
        pending_now(world.close().unwrap());
        if cancel_pending_first {
            pending.cancel_before_submission().unwrap();
            pending_now(world.close().unwrap());
            published.complete_after_native_join().unwrap();
        } else {
            published.complete_after_native_join().unwrap();
            pending_now(world.close().unwrap());
            pending.cancel_before_submission().unwrap();
        }
        let drained = world.close().unwrap().wait(deadline()).unwrap();
        assert_eq!(world.rebuild(drained), Ok(2));
    }
}

#[test]
fn losing_unpublished_work_is_unresolved_instead_of_false_zero() {
    let world = WorldFlights::new(1).unwrap();
    let caller = world.dispatch().unwrap().enter().unwrap();
    let pending = caller.issue().unwrap();
    drop(caller);
    let closing = world.close().unwrap();
    drop(pending);
    assert!(matches!(closing.wait(deadline()), Err(Error::Unresolved)));
    assert!(matches!(world.dispatch(), Err(Error::Unresolved)));
}

#[test]
fn losing_published_work_does_not_substitute_rust_drop_for_native_wait() {
    let world = WorldFlights::new(1).unwrap();
    let caller = world.dispatch().unwrap().enter().unwrap();
    let published = caller.issue().unwrap().publish().unwrap();
    drop(caller);
    let closing = world.close().unwrap();
    drop(published);
    assert!(matches!(closing.wait(deadline()), Err(Error::Unresolved)));
}

#[test]
fn all_outer_frames_on_one_thread_must_exit() {
    let world = WorldFlights::new(1).unwrap();
    let outer = world.dispatch().unwrap().enter().unwrap();
    let nested = world.dispatch().unwrap().enter().unwrap();
    assert!(matches!(world.close(), Err(Error::Reentrant)));
    drop(nested);
    assert!(matches!(world.close(), Err(Error::Reentrant)));
    drop(outer);
    world.close().unwrap().wait(deadline()).unwrap();
}

#[test]
fn blocking_drain_wakes_when_old_world_work_finishes() {
    let world = WorldFlights::new(1).unwrap();
    let caller = world.dispatch().unwrap().enter().unwrap();
    let job = caller.issue().unwrap().publish().unwrap();
    drop(caller);
    let closing = world.close().unwrap();
    thread::scope(|scope| {
        let worker = scope.spawn(|| {
            job.complete_after_native_join().unwrap();
        });
        let drained = closing.wait(deadline()).unwrap();
        worker.join().unwrap();
        assert_eq!(world.rebuild(drained), Ok(2));
    });
}

#[test]
fn expired_idle_drain_does_not_authorize_rebuild() {
    let world = WorldFlights::new(1).unwrap();
    let closing = world.close().unwrap();
    let expired = Instant::now();
    assert!(matches!(closing.wait(expired), Err(Error::Timeout)));
    assert!(matches!(world.dispatch(), Err(Error::Closed)));
    assert_eq!(world.lock().unwrap().phase, Phase::Closing);
    let drained = world.close().unwrap().wait(deadline()).unwrap();
    assert_eq!(world.rebuild(drained), Ok(2));
}

#[test]
fn final_join_after_wait_deadline_cannot_turn_timeout_into_drain() {
    let world = WorldFlights::new(1).unwrap();
    let caller = world.dispatch().unwrap().enter().unwrap();
    let job = caller.issue().unwrap().publish().unwrap();
    drop(caller);
    let closing = world.close().unwrap();
    thread::scope(|scope| {
        let (finish_tx, finish_rx) = mpsc::channel();
        let worker = scope.spawn(move || {
            finish_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            // The waiter sends while holding the mutex. Completion cannot
            // acquire it until Condvar releases it, so this is a real wake.
            job.complete_after_native_join().unwrap();
        });
        let before = Instant::now();
        let limit = before + Duration::from_secs(5);
        let mut observations = 0;
        let result = closing.wait_with_clock(limit, || {
            observations += 1;
            if observations == 1 {
                finish_tx.send(()).unwrap();
                before
            } else {
                // Deterministically model the expired reacquisition instant;
                // do not depend on scheduler speed or a fixed sleep.
                limit
            }
        });
        worker.join().unwrap();
        assert_eq!(observations, 2);
        assert!(matches!(result, Err(Error::Timeout)));
        assert_eq!(world.lock().unwrap().phase, Phase::Closing);
        assert!(matches!(world.dispatch(), Err(Error::Closed)));
        let drained = world.close().unwrap().wait(deadline()).unwrap();
        assert_eq!(world.rebuild(drained), Ok(2));
    });
}

#[test]
fn a_drain_from_another_tracker_and_an_old_close_cannot_reopen() {
    let first = WorldFlights::new(1).unwrap();
    let second = WorldFlights::new(1).unwrap();
    let stale_close = first.close().unwrap();
    let drain = first.close().unwrap().wait(deadline()).unwrap();
    assert_eq!(second.rebuild(drain), Err(Error::Stale));
    assert_eq!(
        first.rebuild(first.close().unwrap().wait(deadline()).unwrap()),
        Ok(2)
    );
    assert!(matches!(stale_close.wait(deadline()), Err(Error::Stale)));
}

#[test]
fn duplicate_drain_cannot_advance_the_same_generation_twice() {
    let world = WorldFlights::new(1).unwrap();
    let first = world.close().unwrap().wait(deadline()).unwrap();
    let second = world.close().unwrap().wait(deadline()).unwrap();
    assert_eq!(world.rebuild(first), Ok(2));
    assert_eq!(world.rebuild(second), Err(Error::Stale));
}

#[test]
fn generation_exhaustion_leaves_admission_closed() {
    assert!(matches!(WorldFlights::new(0), Err(Error::Exhausted)));
    let world = WorldFlights::new(u64::MAX).unwrap();
    let drain = world.close().unwrap().wait(deadline()).unwrap();
    assert_eq!(world.rebuild(drain), Err(Error::Exhausted));
    assert!(matches!(world.dispatch(), Err(Error::Closed)));
}

#[test]
fn rust_unwind_settles_the_complete_caller_scope() {
    let world = WorldFlights::new(1).unwrap();
    assert!(std::panic::catch_unwind(|| {
        let _caller = world.dispatch().unwrap().enter().unwrap();
        panic!("finite fixture unwind; not a native SEH execution");
    })
    .is_err());
    world.close().unwrap().wait(deadline()).unwrap();
}
