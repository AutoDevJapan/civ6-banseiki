use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
pub(crate) static INTERRUPT_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
thread_local! {
    static INTERRUPT_CLEANUP: Cell<bool> = const { Cell::new(false) };
}

/// Permit only the bounded cleanup call on this thread; keep Ctrl-C pending.
pub(crate) fn with_interrupt_cleanup<T>(action: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            INTERRUPT_CLEANUP.with(|state| state.set(self.0));
        }
    }
    let _restore = Restore(INTERRUPT_CLEANUP.with(|state| state.replace(true)));
    action()
}
pub fn install_interrupt_handler() -> crate::Result<()> {
    ctrlc::set_handler(|| {
        INTERRUPTED.store(true, Ordering::Relaxed);
    })
    .map_err(|e| crate::Error::scenario(format!("Cannot register Ctrl-C handler: {e}")))
}
pub fn check_interrupt() -> crate::Result<()> {
    if INTERRUPTED.load(Ordering::Relaxed) && !INTERRUPT_CLEANUP.with(Cell::get) {
        Err(crate::Error::interrupted("interrupted"))
    } else {
        Ok(())
    }
}
pub fn monotonic() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}
pub fn utc_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, false)
}
pub trait Clock: Send + Sync {
    fn now(&self) -> f64;
    fn sleep(&self, seconds: f64);
}
#[derive(Default)]
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> f64 {
        monotonic()
    }
    fn sleep(&self, seconds: f64) {
        let end = monotonic() + seconds;
        while !INTERRUPTED.load(Ordering::Relaxed) {
            let left = end - monotonic();
            if left <= 0.0 {
                break;
            }
            std::thread::sleep(Duration::from_secs_f64(left.min(0.2)));
        }
    }
}
pub fn remaining(deadline: f64) -> crate::Result<f64> {
    let value = deadline - monotonic();
    if value <= 0.0 {
        Err(crate::Error::scenario("Step deadline expired"))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{AutoplayOptions, Backend, Runner};
    use crate::{ErrorKind, Result};
    use std::sync::{atomic::AtomicUsize, Arc};

    #[test]
    fn pre_dispatch_interrupt_is_preserved_at_every_connection_boundary() {
        let _exclusive = INTERRUPT_TEST_LOCK.lock().unwrap();
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                INTERRUPTED.store(false, Ordering::Relaxed);
            }
        }
        let _reset = Reset;
        struct Probe {
            connection: usize,
            interrupt_at: usize,
        }
        impl Backend for Probe {
            fn require_single_process(&mut self) -> Result<()> {
                Ok(())
            }
            fn connect(&mut self, _: u16, _: f64) -> Result<()> {
                self.connection += 1;
                if self.connection == self.interrupt_at {
                    INTERRUPTED.store(true, Ordering::Relaxed);
                }
                Ok(())
            }
            fn list_states(&mut self, _: f64) -> Result<Vec<String>> {
                Ok(vec!["InGame".into()])
            }
            fn run_lua(&mut self, _: &str, _: &str, _: f64) -> Result<String> {
                panic!("Lua must not be dispatched after interruption")
            }
        }
        for interrupt_at in [0, 1, 2] {
            INTERRUPTED.store(interrupt_at == 0, Ordering::Relaxed);
            let root = tempfile::tempdir().unwrap();
            let mut runner = Runner::with_backend(
                serde_json::json!({}),
                root.path().into(),
                root.path().into(),
                root.path().into(),
                4318,
                Box::new(Probe {
                    connection: 0,
                    interrupt_at,
                }),
                Arc::new(SystemClock),
            );
            assert_eq!(
                runner
                    .lua("InGame", "mutating command", 5.0)
                    .unwrap_err()
                    .kind,
                ErrorKind::Interrupted
            );
            assert_eq!(check_interrupt().unwrap_err().kind, ErrorKind::Interrupted);
        }
    }

    #[test]
    fn pending_ctrl_c_allows_one_bounded_stop_and_stays_pending() {
        let _exclusive = INTERRUPT_TEST_LOCK.lock().unwrap();
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                INTERRUPTED.store(false, Ordering::Relaxed);
            }
        }
        let _reset = Reset;
        struct Game {
            starts: Arc<AtomicUsize>,
            stops: Arc<AtomicUsize>,
            interrupt_at_status: bool,
        }
        impl Backend for Game {
            fn require_single_process(&mut self) -> Result<()> {
                Ok(())
            }
            fn list_states(&mut self, _: f64) -> Result<Vec<String>> {
                Ok(vec!["InGame".into()])
            }
            fn run_lua(&mut self, _: &str, code: &str, timeout: f64) -> Result<String> {
                if code.contains("return tostring(startTurn)") {
                    self.starts.fetch_add(1, Ordering::Relaxed);
                    if !self.interrupt_at_status {
                        INTERRUPTED.store(true, Ordering::Relaxed);
                    }
                    return Ok("1|0".into());
                }
                if self.interrupt_at_status && code.contains("tostring(AutoplayManager.IsActive())")
                {
                    INTERRUPTED.store(true, Ordering::Relaxed);
                    return Err(crate::Error::tuner("status transport failed during Ctrl-C"));
                }
                if code.contains("SetActive(false)") {
                    assert!(timeout > 0.0 && timeout <= 5.0);
                    self.stops.fetch_add(1, Ordering::Relaxed);
                }
                Ok("true".into())
            }
        }
        for interrupt_at_status in [false, true] {
            INTERRUPTED.store(false, Ordering::Relaxed);
            let root = tempfile::tempdir().unwrap();
            let starts = Arc::new(AtomicUsize::new(0));
            let stops = Arc::new(AtomicUsize::new(0));
            let mut runner = Runner::with_backend(
                serde_json::json!({}),
                root.path().into(),
                root.path().into(),
                root.path().into(),
                4318,
                Box::new(Game {
                    starts: starts.clone(),
                    stops: stops.clone(),
                    interrupt_at_status,
                }),
                Arc::new(SystemClock),
            );
            assert_eq!(
                runner
                    .autoplay(3, 10.0, AutoplayOptions::default())
                    .unwrap_err()
                    .kind,
                ErrorKind::Interrupted
            );
            assert_eq!(starts.load(Ordering::Relaxed), 1);
            assert_eq!(stops.load(Ordering::Relaxed), 1);
            assert_eq!(check_interrupt().unwrap_err().kind, ErrorKind::Interrupted);
            // The next action still honors the original interrupt and never starts.
            assert_eq!(
                runner
                    .autoplay(3, 10.0, AutoplayOptions::default())
                    .unwrap_err()
                    .kind,
                ErrorKind::Interrupted
            );
            assert_eq!(starts.load(Ordering::Relaxed), 1);
            assert_eq!(stops.load(Ordering::Relaxed), 1);
            // Panic unwinding must also restore the thread's cleanup state.
            let _ = std::panic::catch_unwind(|| with_interrupt_cleanup(|| panic!("cleanup test")));
            assert_eq!(check_interrupt().unwrap_err().kind, ErrorKind::Interrupted);
        }
    }
}
