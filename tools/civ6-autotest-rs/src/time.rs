use std::{sync::OnceLock, time::{Duration, Instant}};
pub fn monotonic() -> f64 { static START: OnceLock<Instant> = OnceLock::new(); START.get_or_init(Instant::now).elapsed().as_secs_f64() }
pub fn utc_now() -> String { chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, false) }
pub trait Clock: Send + Sync { fn now(&self) -> f64; fn sleep(&self, seconds: f64); }
#[derive(Default)]
pub struct SystemClock;
impl Clock for SystemClock { fn now(&self) -> f64 { monotonic() } fn sleep(&self, seconds: f64) { if seconds > 0.0 { std::thread::sleep(Duration::from_secs_f64(seconds)); } } }
pub fn remaining(deadline: f64) -> crate::Result<f64> { let value = deadline - monotonic(); if value <= 0.0 { Err(crate::Error::scenario("Step deadline expired")) } else { Ok(value) } }
