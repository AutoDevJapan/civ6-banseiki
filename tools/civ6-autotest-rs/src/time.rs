use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
pub fn install_interrupt_handler() -> crate::Result<()> {
    ctrlc::set_handler(|| {
        INTERRUPTED.store(true, Ordering::Relaxed);
    })
    .map_err(|e| crate::Error::scenario(format!("Cannot register Ctrl-C handler: {e}")))
}
pub fn check_interrupt() -> crate::Result<()> {
    if INTERRUPTED.load(Ordering::Relaxed) {
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
