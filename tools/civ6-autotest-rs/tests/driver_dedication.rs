mod support_driver;

use civ6_autotest_rs::{runner::AutoplayOptions, ErrorKind};
use serde_json::json;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use support_driver::{FakeBackend, Harness};

fn run_case(statuses: &[&str], pending: &[&str]) -> (Harness, civ6_autotest_rs::Result<String>) {
    let statuses = Arc::new(Mutex::new(VecDeque::from(
        statuses.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )));
    let last = statuses.lock().unwrap().back().cloned().unwrap();
    let pending = Arc::new(Mutex::new(VecDeque::from(
        pending.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )));
    let fake = FakeBackend::new().lua(move |_, code| {
        if code.contains("return tostring(startTurn)") {
            return Ok("30|14".into());
        }
        if code.contains("tostring(AutoplayManager.IsActive())") {
            return Ok(statuses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| last.clone()));
        }
        if code.contains("GetPlayerCommemorateChoices") {
            return Ok(pending
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| "none".into()));
        }
        if code.contains("IsAlive()") {
            return Ok("true".into());
        }
        Ok("true".into())
    });
    let mut h = Harness::new(json!({}), fake);
    h.clock.set_min_sleep(3.0);
    let result = h.runner.autoplay(10, 1000.0, AutoplayOptions::default());
    (h, result)
}
fn count(h: &Harness, code: &str) -> usize {
    h.count_code(code)
}
fn keys(h: &Harness) -> usize {
    h.events().iter().filter(|s| s.starts_with("key:")).count()
}

#[test]
fn test_stall_with_pending_dedication_chooses_closes_and_resumes() {
    let (h, result) = run_case(
        &[
            "31|true|false|0|14",
            "31|false|false|10|14",
            "31|false|false|10|14",
            "31|false|false|10|14",
            "31|false|false|10|14",
            "40|false|true|0|14",
        ],
        &["7,9"],
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(keys(&h), 1);
    assert_eq!(count(&h, "GetPlayerCommemorateChoices"), 1);
    assert_eq!(count(&h, "SetActive(true)"), 2);
}

#[test]
fn test_repeated_dedication_is_bounded_then_fails() {
    let (h, result) = run_case(&["31|false|false|10|14"], &["1"; 50]);
    assert_eq!(result.unwrap_err().kind, ErrorKind::Scenario);
    assert!(keys(&h) <= 3);
    assert_eq!(count(&h, "GetPlayerCommemorateChoices"), 3);
    assert_eq!(count(&h, "SetActive(true)"), 1 + 3 + 2);
}

#[test]
fn test_no_pending_dedication_keeps_old_restart_behaviour() {
    let (h, result) = run_case(&["31|false|false|10|14"], &[]);
    assert_eq!(result.unwrap_err().kind, ErrorKind::Scenario);
    assert_eq!(keys(&h), 0);
}
