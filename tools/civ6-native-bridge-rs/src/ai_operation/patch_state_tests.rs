use super::*;
use std::sync::{atomic::AtomicUsize, Barrier};

struct Page(usize);
impl Page {
    fn new() -> Self {
        let p = unsafe { VirtualAlloc(ptr::null(), 4096, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
            as usize;
        assert_ne!(p, 0);
        // mov eax, 17; ret. Other cells on the SAME executable page.
        unsafe {
            ptr::copy_nonoverlapping([0xb8, 17, 0, 0, 0, 0xc3].as_ptr(), p as *mut u8, 6);
            (p as *mut u64).add(2).write(0x1234);
            (p as *mut u64).add(3).write(0x1234);
        }
        assert!(SystemApi.protect(p, 4096, PAGE_EXECUTE_READ));
        assert!(SystemApi.flush(p, 4096));
        Self(p)
    }
    fn patch(&self, offset: usize) -> Patch {
        Patch {
            address: self.0 + offset,
            old: 0x1234,
            new: 0x5678,
            wide: true,
        }
    }
    fn execute(&self) -> u32 {
        unsafe {
            let f: extern "system" fn() -> u32 = std::mem::transmute(self.0);
            f()
        }
    }
}
impl Drop for Page {
    fn drop(&mut self) {
        unsafe {
            VirtualFree(self.0 as *mut _, 0, MEM_RELEASE);
        }
    }
}

struct Faults {
    protect_fail: AtomicUsize,
    flush_fail: AtomicUsize,
    protect_calls: AtomicUsize,
    flush_calls: AtomicUsize,
}
impl Faults {
    fn new(protect: usize, flush: usize) -> Self {
        Self {
            protect_fail: AtomicUsize::new(protect),
            flush_fail: AtomicUsize::new(flush),
            protect_calls: AtomicUsize::new(0),
            flush_calls: AtomicUsize::new(0),
        }
    }
}
impl PatchApi for Faults {
    fn protect(&self, a: usize, n: usize, p: u32) -> bool {
        let call = self.protect_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.protect_fail.load(Ordering::SeqCst) {
            false
        } else {
            SystemApi.protect(a, n, p)
        }
    }
    fn flush(&self, a: usize, n: usize) -> bool {
        let call = self.flush_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.flush_fail.load(Ordering::SeqCst) {
            false
        } else {
            SystemApi.flush(a, n)
        }
    }
}
fn installed(page: &Page, guard: &MutexGuard<'_, ()>) -> Registration {
    let r = Registration::prepare_locked(vec![page.patch(16)], guard).unwrap();
    r.install_locked(guard, &SystemApi).unwrap();
    r
}

#[test]
fn original_bytes_with_failed_protection_are_not_complete_and_retry_preserves_rx() {
    let guard = INSTALL.lock().unwrap();
    let page = Page::new();
    let r = installed(&page, &guard);
    let failures = Faults::new(2, 0);
    assert_eq!(r.close_locked(&guard, &failures), Err(Failure::Patch));
    assert_eq!(u64_at(page.0 as u64 + 16), Some(0x1234));
    assert_eq!(protection(page.0, 8), Some(PAGE_EXECUTE_READWRITE));
    let state = r.states.lock().unwrap();
    assert!(state[0].protection_pending);
    assert_eq!(state[0].original_protection, PAGE_EXECUTE_READ);
    drop(state);
    let clone = r.clone();
    clone.close_locked(&guard, &failures).unwrap();
    assert_eq!(
        failures.protect_calls.load(Ordering::SeqCst),
        3,
        "retry only restores saved attributes"
    );
    assert_eq!(protection(page.0, 8), Some(PAGE_EXECUTE_READ));
    assert_eq!(page.execute(), 17);
}

#[test]
fn original_bytes_with_failed_cache_flush_retry_flush_before_success() {
    let guard = INSTALL.lock().unwrap();
    let page = Page::new();
    let r = installed(&page, &guard);
    let failures = Faults::new(0, 1);
    assert_eq!(r.close_locked(&guard, &failures), Err(Failure::Patch));
    assert_eq!(u64_at(page.0 as u64 + 16), Some(0x1234));
    assert_eq!(protection(page.0, 8), Some(PAGE_EXECUTE_READ));
    assert!(r.states.lock().unwrap()[0].cache_pending);
    r.clone().close_locked(&guard, &failures).unwrap();
    assert_eq!(failures.protect_calls.load(Ordering::SeqCst), 2);
    assert_eq!(failures.flush_calls.load(Ordering::SeqCst), 2);
    assert_eq!(page.execute(), 17);
}

#[test]
fn failed_make_writable_and_combined_cleanup_failure_remain_retryable() {
    for (protect, flush) in [(1, 0), (2, 1)] {
        let guard = INSTALL.lock().unwrap();
        let page = Page::new();
        let r = installed(&page, &guard);
        let failures = Faults::new(protect, flush);
        assert_eq!(r.close_locked(&guard, &failures), Err(Failure::Patch));
        assert!(r
            .states
            .lock()
            .unwrap()
            .iter()
            .any(|s| s.phase != Phase::Restored));
        r.close_locked(&guard, &SystemApi).unwrap();
        assert_eq!(protection(page.0, 8), Some(PAGE_EXECUTE_READ));
        assert_eq!(page.execute(), 17);
    }
}

#[test]
fn install_failure_cleans_all_sites_sharing_rx_page_without_recursive_lock() {
    for (protect, cache) in [(2, 0), (0, 1)] {
        let guard = INSTALL.lock().unwrap();
        let page = Page::new();
        let r = Registration::prepare_locked(vec![page.patch(16), page.patch(24)], &guard).unwrap();
        assert_eq!(
            r.install_locked(&guard, &Faults::new(protect, cache)),
            Err(Failure::Patch)
        );
        r.close_locked(&guard, &SystemApi).unwrap();
        for offset in [16, 24] {
            assert_eq!(u64_at((page.0 + offset) as u64), Some(0x1234));
        }
        assert!(r
            .states
            .lock()
            .unwrap()
            .iter()
            .all(|s| s.original_protection == PAGE_EXECUTE_READ && s.phase == Phase::Restored));
        assert_eq!(protection(page.0, 8), Some(PAGE_EXECUTE_READ));
    }
}

struct Pause {
    entered: Barrier,
    release: Barrier,
}
impl PatchApi for Pause {
    fn protect(&self, a: usize, n: usize, p: u32) -> bool {
        SystemApi.protect(a, n, p)
    }
    fn flush(&self, a: usize, n: usize) -> bool {
        SystemApi.flush(a, n)
    }
    fn before_cas(&self) {
        self.entered.wait();
        self.release.wait();
    }
}

#[test]
fn concurrent_close_and_recovery_cannot_restore_rx_before_other_cas() {
    if !super::super::tests::child("gateway::patch_state::tests::concurrent_close_and_recovery_cannot_restore_rx_before_other_cas","CIV6_CLOSE_RACE_CHILD"){return}
    let page = Page::new();
    let r = {
        let guard = INSTALL.lock().unwrap();
        installed(&page, &guard)
    };
    assert!(RECOVERY.set(r.clone()).is_ok());
    let pause = Arc::new(Pause {
        entered: Barrier::new(2),
        release: Barrier::new(2),
    });
    let (sent, recv) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let p = pause.clone();
        let a = r.clone();
        let first = scope.spawn(move || {
            let guard = INSTALL.lock().unwrap();
            a.close_locked(&guard, &*p)
        });
        pause.entered.wait();
        assert_eq!(protection(page.0, 8), Some(PAGE_EXECUTE_READWRITE));
        let barrier = Arc::new(Barrier::new(2));
        let other = barrier.clone();
        let second = scope.spawn(move || {
            other.wait();
            let result = recover_failed_registration();
            sent.send(()).unwrap();
            result
        });
        barrier.wait();
        assert!(recv
            .recv_timeout(std::time::Duration::from_millis(30))
            .is_err());
        assert_eq!(protection(page.0, 8), Some(PAGE_EXECUTE_READWRITE));
        pause.release.wait();
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
    });
    assert_eq!(protection(page.0, 8), Some(PAGE_EXECUTE_READ));
    assert_eq!(page.execute(), 17);
}
