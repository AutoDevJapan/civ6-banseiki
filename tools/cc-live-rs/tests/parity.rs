use cc_live::{chain::*, process::CREATE_NO_WINDOW, *};
use serde_json::json;
use std::{
    collections::VecDeque,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static SERIAL: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    p: Paths,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "cc-live-test-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::SeqCst)
        ));
        let p = Paths {
            here: root.join("here"),
            wt: root.join("wt"),
            jobs: root.join("jobs"),
            temp: root.join("temp"),
            profile: root.join("user"),
            python: "fixture-python".into(),
        };
        for dir in [&p.here, &p.wt, &p.jobs, &p.temp, &p.profile] {
            fs::create_dir_all(dir).unwrap();
        }
        Self { root, p }
    }
    fn json(&self, relative: &str, v: serde_json::Value) -> PathBuf {
        let p = self.root.join(relative);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        write_json(&p, &v).unwrap();
        p
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[derive(Default)]
struct Fake {
    calls: Vec<Call>,
    replies: VecDeque<Result<Output, String>>,
    times: VecDeque<f64>,
    time: f64,
    sleeps: Vec<f64>,
    printed: Vec<String>,
    errors: Vec<String>,
}
impl Fake {
    fn outputs(values: &[(&str, &str, i32)]) -> Self {
        Self {
            replies: values
                .iter()
                .map(|(s, e, c)| {
                    Ok(Output {
                        stdout: (*s).into(),
                        stderr: (*e).into(),
                        code: *c,
                    })
                })
                .collect(),
            ..Self::default()
        }
    }
    fn success() -> Self {
        Self::default()
    }
}
impl Runner for Fake {
    fn run(&mut self, call: Call) -> Result<Output, String> {
        assert_eq!(call.creationflags, CREATE_NO_WINDOW);
        self.calls.push(call);
        self.replies
            .pop_front()
            .unwrap_or_else(|| Ok(Output::default()))
    }
    fn now(&mut self) -> f64 {
        self.times.pop_front().unwrap_or(self.time)
    }
    fn sleep(&mut self, s: f64) -> Result<(), String> {
        self.sleeps.push(s);
        self.time += s;
        Ok(())
    }
    fn print(&mut self, s: &str) -> Result<(), String> {
        self.printed.push(s.into());
        Ok(())
    }
    fn error(&mut self, s: &str) -> Result<(), String> {
        self.errors.push(s.into());
        Ok(())
    }
}

// The six original Python unittest methods retain their names and all subcases.
#[test]
fn test_detection_accepts_either_renderer_and_not_similar_names() {
    for (dx11, dx12, expected) in [
        ("\"CivilizationVI.exe\",\"11\"", "", true),
        ("", "\"CivilizationVI_DX12.exe\",\"12\"", true),
        ("\"CIVILIZATIONVI.EXE\",\"11\"", "", true),
        (
            "\"CivilizationVIHelper.exe\",\"13\"",
            "INFO: no tasks",
            false,
        ),
        ("INFO: no tasks", "INFO: no tasks", false),
    ] {
        let mut r = Fake::outputs(&[(dx11, "", 0), (dx12, "", 0)]);
        assert_eq!(civ6_running(&mut r).unwrap(), expected);
        assert_eq!(r.calls[0].args[2], "IMAGENAME eq CivilizationVI.exe");
        if !dx11.starts_with("\"CivilizationVI.exe\"")
            && !dx11.starts_with("\"CIVILIZATIONVI.EXE\"")
        {
            assert_eq!(r.calls[1].args[2], "IMAGENAME eq CivilizationVI_DX12.exe");
        }
        assert!(r.calls.iter().all(|c| c.timeout == Some(30.0)));
    }
}
#[test]
fn test_enumeration_failure_is_not_reported_as_absent() {
    let mut r = Fake::outputs(&[("", "failed", 1)]);
    assert!(civ6_running(&mut r)
        .unwrap_err()
        .contains("Could not inspect Civ6"));
}
#[test]
fn test_cleanup_kills_both_renderers_before_scoped_reporter_cleanup() {
    let mut r = Fake::success();
    close_civ6(&mut r).unwrap();
    assert_eq!(
        r.calls[0].args,
        args(&["taskkill", "/IM", "CivilizationVI.exe", "/F"])
    );
    assert_eq!(
        r.calls[1].args,
        args(&["taskkill", "/IM", "CivilizationVI_DX12.exe", "/F"])
    );
    assert_eq!(
        r.calls.last().unwrap().args,
        args(&["powershell", "-NoProfile", "-Command", REPORTER_COMMAND])
    );
    assert_eq!(r.calls.len(), 5);
}
#[test]
fn test_failed_termination_stops_before_reporter_cleanup() {
    let mut r = Fake::outputs(&[
        ("", "access denied", 1),
        ("", "access denied", 1),
        ("\"CivilizationVI.exe\",\"11\"", "", 0),
    ]);
    r.times = VecDeque::from([0.0, 15.0]);
    assert!(close_civ6(&mut r)
        .unwrap_err()
        .contains("refusing to continue"));
    assert_eq!(r.calls.len(), 3);
    assert!(r.sleeps.is_empty());
}
#[test]
fn test_termination_waits_for_a_process_to_disappear() {
    let mut r = Fake::outputs(&[
        ("", "", 0),
        ("", "", 0),
        ("\"CivilizationVI.exe\",\"11\"", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
    ]);
    r.times = VecDeque::from([0.0, 1.0]);
    close_civ6(&mut r).unwrap();
    assert_eq!(r.sleeps, vec![0.5]);
    assert_eq!(r.calls.last().unwrap().args[0], "powershell");
}
#[test]
fn test_absent_game_avoids_tuner_and_kill() {
    let f = Fixture::new();
    let mut r = Fake::success();
    assert_eq!(ensure_responsive_civ6(&mut r, &f.p).unwrap(), "absent");
    assert_eq!(r.calls.len(), 3);
    assert_eq!(r.calls[2].args[0], "powershell");
    assert!(r
        .calls
        .iter()
        .all(|c| c.args[0] != "taskkill" && c.args[0] != "fixture-python"));
}

#[test]
fn scoped_reporter_cleanup_keeps_exact_path_and_checks_parent() {
    assert!(REPORTER_COMMAND.contains("$_.ExecutablePath -eq $expected"));
    assert!(REPORTER_COMMAND.contains("$_.ParentProcessId"));
    assert!(REPORTER_COMMAND.contains("if ($civ6) { exit 0 }"));
    assert!(REPORTER_COMMAND.contains("@('CivilizationVI','CivilizationVI_DX12')"));
}
#[test]
fn reporter_failure_aborts() {
    let mut r = Fake::outputs(&[("", "denied", 1)]);
    assert!(close_stale_civ6_reporters(&mut r)
        .unwrap_err()
        .contains("denied"));
}
#[test]
fn answering_game_is_still_closed_before_relaunch() {
    let f = Fixture::new();
    let mut r = Fake::outputs(&[("\"CivilizationVI.exe\",\"11\"", "", 0)]);
    assert_eq!(ensure_responsive_civ6(&mut r, &f.p).unwrap(), "killed");
    assert_eq!(r.calls[1].args[0], "fixture-python");
    assert_eq!(r.calls[2].args[0], "taskkill");
    assert_eq!(r.sleeps, vec![45.0]);
}
#[test]
fn unresponsive_game_retries_tuner_three_times() {
    let f = Fixture::new();
    let mut r = Fake::outputs(&[
        ("\"CivilizationVI.exe\",\"11\"", "", 0),
        ("", "", 1),
        ("", "", 1),
        ("", "", 1),
    ]);
    ensure_responsive_civ6(&mut r, &f.p).unwrap();
    assert_eq!(
        r.calls
            .iter()
            .filter(|c| c.args[0] == "fixture-python")
            .count(),
        3
    );
    assert_eq!(r.sleeps, vec![10.0, 10.0, 10.0, 45.0]);
}
#[test]
fn long_run_boundaries() {
    for (item, long) in [
        ("x:149turn.json", false),
        ("x:150turn.json", true),
        ("x:199turn.json", true),
        ("x:200turn.json", true),
        ("x:999turn.json", true),
        ("x:1000turn.json", false),
        ("x:1200turn.json", false),
        ("x:50turn.json", false),
    ] {
        assert_eq!(is_long(&[item.into()]), long, "{item}");
    }
}
#[test]
fn holder_alive_reads_existing_job_layout() {
    let f = Fixture::new();
    let mut r = Fake::success();
    for (status, expected) in [
        ("queued", true),
        ("starting", true),
        ("running", true),
        ("succeeded", false),
        ("failed", false),
        ("lost", false),
        ("cancelled", false),
        ("completed", false),
        ("unknown", false),
    ] {
        f.json(
            "jobs/job-a/job.json",
            json!({"status":status,"other":"preserved"}),
        );
        for owner in [
            json!({"job":"job-a"}),
            json!({"owner_job":"job-a"}),
            json!({"job":"job-a","pid":123}),
            json!({"owner_job":"job-a","pid":123}),
        ] {
            assert_eq!(
                holder_alive(&mut r, &f.p, &owner).unwrap(),
                expected,
                "{status}: {owner}"
            );
        }
    }
    assert!(!holder_alive(&mut r, &f.p, &json!({"job":"missing"})).unwrap());
    assert!(r.calls.is_empty());
}
#[test]
fn holder_alive_requires_exact_pid_and_excludes_game_reporter_and_steam() {
    let f = Fixture::new();
    for (stdout, expected) in [
        ("\"codex.exe\",\"123\",\"Console\",\"1\",\"100 K\"", true),
        ("\"worker.exe\",\"12345\"", false),
        ("\"worker123.exe\",\"45\"", false),
        ("INFO: no tasks matching PID 123", false),
        ("", false),
        ("\"CivilizationVI.exe\",\"123\"", false),
        ("\"CivilizationVI_DX12.exe\",\"123\"", false),
        ("\"FiraxisBugReporter.exe\",\"123\"", false),
        ("\"steam.exe\",\"123\"", false),
        ("\"CIVILIZATIONVI.EXE\",\"123\"", false),
        ("\"STEAM.EXE\",\"123\"", false),
        ("\"CivilizationVIHelper.exe\",\"123\"", true),
    ] {
        for pid in [json!(123), json!("123")] {
            let mut r = Fake::outputs(&[(stdout, "", 0)]);
            assert_eq!(
                holder_alive(&mut r, &f.p, &json!({"pid":pid})).unwrap(),
                expected,
                "{stdout}"
            );
            assert_eq!(
                r.calls[0].args,
                args(&["tasklist", "/FI", "PID eq 123", "/FO", "CSV", "/NH"])
            );
        }
    }
}

#[test]
fn live_chain_waiter_rejects_a_reused_pid_owned_by_another_program() {
    let f = Fixture::new();
    f.json("jobs/active/job.json", json!({"status":"running"}));
    for (image, expected) in [
        ("bash.exe", false),
        ("codex.exe", false),
        ("CivilizationVI.exe", false),
        ("live_chain.exe", true),
        ("live_chain_pr86_amd.exe", true),
    ] {
        let output = format!("\"{image}\",\"123\"");
        let mut r = Fake::outputs(&[(&output, "", 0)]);
        let owner = json!({"job":"active","live_chain":true,"pid":123});
        assert_eq!(
            holder_alive(&mut r, &f.p, &owner).unwrap(),
            expected,
            "{image}"
        );
    }
}

#[cfg(windows)]
#[test]
fn live_chain_waiter_rejects_mismatched_process_creation_time() {
    let f = Fixture::new();
    let pid = std::process::id();
    let actual = process_created_filetime(pid).unwrap().unwrap();
    assert!(actual > 0);
    let output = format!("\"live_chain.exe\",\"{pid}\"");
    let mut matching = Fake::outputs(&[(&output, "", 0)]);
    let owner = json!({"live_chain":true,"pid":pid,"pid_created_filetime":actual});
    assert!(holder_alive(&mut matching, &f.p, &owner).unwrap());
    let mut r = Fake::outputs(&[(&output, "", 0)]);
    let owner = json!({"live_chain":true,"pid":pid,"pid_created_filetime":0});
    assert!(!holder_alive(&mut r, &f.p, &owner).unwrap());
}

#[test]
fn holder_alive_invalid_pid_is_dead_without_a_process_query() {
    let f = Fixture::new();
    let mut r = Fake::success();
    for pid in [
        json!(null),
        json!(0),
        json!(-1),
        json!("abc"),
        json!(true),
        json!(1.5),
        json!(4294967296u64),
    ] {
        assert!(!holder_alive(&mut r, &f.p, &json!({"pid":pid})).unwrap());
    }
    assert!(r.calls.is_empty());
}

#[test]
fn finished_owner_job_overrides_another_active_job_and_a_live_pid() {
    let f = Fixture::new();
    f.json("jobs/finished/job.json", json!({"status":"succeeded"}));
    f.json("jobs/active/job.json", json!({"status":"running"}));
    let mut r = Fake::outputs(&[("\"codex.exe\",\"123\"", "", 0)]);
    for owner in [
        json!({"owner_job":"finished","job":"active","pid":123}),
        json!({"owner_job":"active","job":"finished","pid":123}),
        json!({"owner_job":"missing","pid":123}),
        json!({"job":"missing","pid":123}),
    ] {
        assert!(!holder_alive(&mut r, &f.p, &owner).unwrap());
    }
    assert!(r.calls.is_empty());
}

#[test]
fn holder_inspection_failure_is_not_treated_as_a_dead_owner() {
    let f = Fixture::new();
    let mut r = Fake::outputs(&[("", "tasklist failed", 1)]);
    assert!(holder_alive(&mut r, &f.p, &json!({"pid":123})).is_err());
    assert_eq!(r.calls[0].timeout, Some(30.0));
    f.json("jobs/partial/job.json", json!({"status":"running"}));
    fs::write(f.p.jobs.join("partial/job.json"), "{").unwrap();
    assert!(holder_alive(&mut r, &f.p, &json!({"job":"partial"})).is_err());
}
#[test]
fn short_waiter_precedes_older_long_waiter() {
    let f = Fixture::new();
    let me = f.json(
        "here/queue/me.json",
        json!({"job":"mine","long":1,"since":1}),
    );
    f.json(
        "here/queue/other.json",
        json!({"job":"other","long":0,"since":2}),
    );
    f.json("jobs/other/job.json", json!({"status":"running"}));
    assert!(!my_turn(&mut Fake::success(), &f.p, &me).unwrap());
}
#[test]
fn long_waiter_precedes_short_arriving_after_one_lease() {
    let f = Fixture::new();
    let me = f.json(
        "here/queue/me.json",
        json!({"job":"mine","long":1,"since":1}),
    );
    f.json(
        "here/queue/other.json",
        json!({"job":"other","long":0,"since":1802}),
    );
    f.json("jobs/other/job.json", json!({"status":"running"}));
    assert!(my_turn(&mut Fake::success(), &f.p, &me).unwrap());
}
#[test]
fn short_aged_long_and_fifo_requeue_have_one_shared_order() {
    let f = Fixture::new();
    let long = f.json(
        "here/queue/long.json",
        json!({"job":"long","long":1,"since":1}),
    );
    let fifo = f.json(
        "here/queue/fifo.json",
        json!({"job":"fifo","long":1,"fifo":true,"since":3}),
    );
    let short = f.json(
        "here/queue/short.json",
        json!({"job":"short","long":0,"since":4}),
    );
    for job in ["long", "fifo", "short"] {
        f.json(&format!("jobs/{job}/job.json"), json!({"status":"running"}));
    }
    assert!(my_turn(&mut Fake::success(), &f.p, &fifo).unwrap());
    assert!(!my_turn(&mut Fake::success(), &f.p, &short).unwrap());
    assert!(!my_turn(&mut Fake::success(), &f.p, &long).unwrap());
}
#[test]
fn same_priority_is_first_come_first_served() {
    let f = Fixture::new();
    let me = f.json("here/queue/me.json", json!({"long":0,"since":2}));
    f.json(
        "here/queue/other.json",
        json!({"job":"other","long":0,"since":1}),
    );
    f.json("jobs/other/job.json", json!({"status":"running"}));
    assert!(!my_turn(&mut Fake::success(), &f.p, &me).unwrap());
}
#[test]
fn stale_waiter_removed_and_broken_waiter_ignored() {
    let f = Fixture::new();
    let me = f.json("here/queue/me.json", json!({"long":1,"since":2}));
    let stale = f.json(
        "here/queue/other.json",
        json!({"job":"missing","long":0,"since":1}),
    );
    fs::write(f.p.here.join("queue/broken.json"), "{").unwrap();
    assert!(my_turn(&mut Fake::success(), &f.p, &me).unwrap());
    assert!(!stale.exists());
    assert!(f.p.here.join("queue/broken.json").exists());
}
#[test]
fn stale_pid_cleanup_keeps_a_new_registration_with_its_own_name() {
    let f = Fixture::new();
    let me = f.json("here/queue/me.json", json!({"long":0,"since":1}));
    let old = f.json(
        "here/queue/123-old.json",
        json!({"job":"old","pid":123,"long":0,"since":2}),
    );
    let new = f.json(
        "here/queue/123-new.json",
        json!({"job":"new","pid":123,"long":0,"since":3}),
    );
    f.json("jobs/old/job.json", json!({"status":"failed"}));
    f.json("jobs/new/job.json", json!({"status":"running"}));
    assert!(my_turn(&mut Fake::success(), &f.p, &me).unwrap());
    assert!(!old.exists());
    assert!(new.exists());
}
#[test]
fn acquisition_reclaims_stale_lock_and_retains_format() {
    let f = Fixture::new();
    f.json("here/civ6.lock", json!({"job":"missing"}));
    acquire(&mut Fake::success(), &f.p, &json!({"pid":123}), 20.0, false).unwrap();
    assert_eq!(
        fs::read_to_string(f.p.here.join("civ6.lock")).unwrap(),
        "{\"pid\": 123}"
    );
    assert_eq!(fs::read_dir(f.p.here.join("queue")).unwrap().count(), 0);
}
#[test]
fn acquisition_reclaims_game_lock_and_finished_job_even_with_live_pid() {
    for owner in [
        json!({"pid":42,"purpose":"Civ6の停止は禁止。"}),
        json!({"owner_job":"finished","pid":42}),
        json!({"job":"finished","pid":42}),
    ] {
        let f = Fixture::new();
        f.json("jobs/finished/job.json", json!({"status":"succeeded"}));
        f.json("here/civ6.lock", owner);
        let mut r = Fake::outputs(&[("\"CivilizationVI.exe\",\"42\"", "", 0)]);
        acquire(&mut r, &f.p, &json!({"pid":123}), 20.0, false).unwrap();
        assert_eq!(
            read_json(f.p.here.join("civ6.lock")).unwrap(),
            json!({"pid":123})
        );
        assert!(r.sleeps.is_empty());
        assert!(r.printed.is_empty());
        assert_eq!(fs::read_dir(f.p.here.join("queue")).unwrap().count(), 0);
    }
}
#[test]
fn lock_timeout_keeps_live_holder_and_removes_own_waiter() {
    let f = Fixture::new();
    f.json("here/civ6.lock", json!({"job":"alive"}));
    f.json("jobs/alive/job.json", json!({"status":"running"}));
    let mut r = Fake::success();
    assert!(acquire(&mut r, &f.p, &json!({"pid":123}), 1.0, false)
        .unwrap_err()
        .contains("{'job': 'alive'}"));
    assert_eq!(r.sleeps, vec![60.0]);
    assert_eq!(
        r.printed,
        vec!["waiting for civ6.lock held by {'job': 'alive'}"]
    );
    assert_eq!(fs::read_dir(f.p.here.join("queue")).unwrap().count(), 0);
}

#[cfg(windows)]
#[test]
fn acquisition_protects_dead_owner_until_it_has_written_its_own_lock() {
    struct Contender {
        fake: Fake,
        lock: PathBuf,
        inspected: bool,
    }
    impl Runner for Contender {
        fn run(&mut self, call: Call) -> Result<Output, String> {
            if call.args[0] == "tasklist" && call.args[2] == "PID eq 42" {
                self.inspected = true;
                assert!(fs::remove_file(&self.lock).is_err());
                assert!(fs::write(&self.lock, "another owner").is_err());
                assert!(fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&self.lock)
                    .is_err());
            }
            self.fake.run(call)
        }
        fn now(&mut self) -> f64 {
            self.fake.now()
        }
        fn sleep(&mut self, seconds: f64) -> Result<(), String> {
            self.fake.sleep(seconds)
        }
    }
    let f = Fixture::new();
    let lock = f.json("here/civ6.lock", json!({"pid":42}));
    let mut r = Contender {
        fake: Fake::outputs(&[("\"CivilizationVI.exe\",\"42\"", "", 0)]),
        lock: lock.clone(),
        inspected: false,
    };
    acquire(&mut r, &f.p, &json!({"pid":123}), 20.0, false).unwrap();
    assert!(r.inspected);
    assert_eq!(read_json(&lock).unwrap(), json!({"pid":123}));
    assert!(r.fake.sleeps.is_empty());
}
#[test]
fn queue_timeout_yields_to_short_run() {
    let f = Fixture::new();
    f.json(
        "here/queue/other.json",
        json!({"job":"other","long":0,"since":1}),
    );
    f.json("jobs/other/job.json", json!({"status":"running"}));
    let mut r = Fake {
        time: 2.0,
        ..Fake::success()
    };
    assert!(acquire(&mut r, &f.p, &json!({"pid":123}), 1.0, true)
        .unwrap_err()
        .contains("higher-priority"));
    assert_eq!(r.sleeps, vec![30.0]);
    assert!(!f.p.here.join("civ6.lock").exists());
}

#[cfg(windows)]
#[test]
fn acquisition_retries_a_protected_claim_and_respects_the_deadline() {
    use std::os::windows::fs::OpenOptionsExt;
    struct Protected {
        fake: Fake,
        handle: Option<fs::File>,
        release: bool,
    }
    impl Runner for Protected {
        fn run(&mut self, call: Call) -> Result<Output, String> {
            self.fake.run(call)
        }
        fn now(&mut self) -> f64 {
            self.fake.now()
        }
        fn sleep(&mut self, seconds: f64) -> Result<(), String> {
            if self.release {
                drop(self.handle.take());
            }
            self.fake.sleep(seconds)
        }
    }
    for release in [true, false] {
        let f = Fixture::new();
        let lock = f.json("here/civ6.lock", json!({"pid":42}));
        let handle = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(1)
            .open(&lock)
            .unwrap();
        let mut r = Protected {
            fake: Fake::success(),
            handle: Some(handle),
            release,
        };
        let result = acquire(&mut r, &f.p, &json!({"pid":123}), 1.0, false);
        if release {
            result.unwrap();
            assert_eq!(read_json(&lock).unwrap(), json!({"pid":123}));
            assert_eq!(r.fake.sleeps, vec![0.5]);
        } else {
            assert!(result.unwrap_err().contains("being claimed"));
            assert_eq!(read_json(&lock).unwrap(), json!({"pid":42}));
            assert_eq!(r.fake.sleeps, vec![0.5, 0.5, 0.5]);
        }
        assert_eq!(fs::read_dir(f.p.here.join("queue")).unwrap().count(), 0);
    }
}
#[test]
fn final_cleanup_happens_after_invalid_input() {
    let f = Fixture::new();
    let mut r = Fake::success();
    assert!(cli(&mut r, &f.p, &["invalid".into()]).is_err());
    assert!(!f.p.here.join("civ6.lock").exists());
    assert_eq!(r.calls[0].args[0], "taskkill");
}
#[test]
fn cleanup_failure_still_releases_lock() {
    let f = Fixture::new();
    let mut r = Fake::outputs(&[
        ("", "", 0),
        ("", "", 0),
        ("\"CivilizationVI.exe\",\"11\"", "", 0),
    ]);
    r.times = VecDeque::from([0.0, 0.0, 0.0, 0.0, 15.0]);
    assert!(cli(&mut r, &f.p, &[]).unwrap_err().contains("refusing"));
    assert!(!f.p.here.join("civ6.lock").exists());
}

#[cfg(windows)]
#[test]
#[ignore]
fn release_reader_child() {
    use std::{
        os::windows::fs::OpenOptionsExt,
        thread,
        time::{Duration, Instant},
    };
    let lock = PathBuf::from(std::env::var_os("CC_LIVE_TEST_LOCK").unwrap());
    let ready = PathBuf::from(std::env::var_os("CC_LIVE_TEST_READY").unwrap());
    let release = PathBuf::from(std::env::var_os("CC_LIVE_TEST_RELEASE").unwrap());
    let share = std::env::var("CC_LIVE_TEST_SHARE")
        .unwrap_or_else(|_| "1".into())
        .parse::<u32>()
        .unwrap();
    let handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(share)
        .open(lock)
        .unwrap();
    fs::write(ready, "ready").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !release.exists() {
        assert!(
            Instant::now() < deadline,
            "parent did not release test reader"
        );
        thread::sleep(Duration::from_millis(10));
    }
    drop(handle);
}

#[cfg(windows)]
struct OwnedReader {
    child: Option<std::process::Child>,
    release: PathBuf,
}

#[cfg(windows)]
impl OwnedReader {
    fn spawn(
        lock: &std::path::Path,
        ready: &std::path::Path,
        release: &std::path::Path,
        share: u32,
    ) -> Self {
        use std::{
            os::windows::process::CommandExt,
            process::{Command, Stdio},
            thread,
            time::Duration,
        };
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "release_reader_child"])
            .env("CC_LIVE_TEST_LOCK", lock)
            .env("CC_LIVE_TEST_READY", ready)
            .env("CC_LIVE_TEST_RELEASE", release)
            .env("CC_LIVE_TEST_SHARE", share.to_string())
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let reader = Self {
            child: Some(child),
            release: release.into(),
        };
        for _ in 0..200 {
            if ready.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(ready.exists(), "test reader did not open the lock");
        reader
    }

    fn finish(&mut self) {
        if let Some(mut child) = self.child.take() {
            fs::write(&self.release, "release").unwrap();
            assert!(child.wait().unwrap().success());
        }
    }
}

#[cfg(windows)]
impl Drop for OwnedReader {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(windows)]
struct Pending {
    fake: Fake,
    lock: PathBuf,
    ready: PathBuf,
    release: PathBuf,
    reader: Option<OwnedReader>,
    release_on_sleep: bool,
}

#[cfg(windows)]
impl Runner for Pending {
    fn run(&mut self, c: Call) -> Result<Output, String> {
        self.fake.run(c)
    }
    fn now(&mut self) -> f64 {
        self.fake.now()
    }
    fn sleep(&mut self, s: f64) -> Result<(), String> {
        assert_eq!(s, 0.05, "unexpected wait during owned reader test");
        if self.release_on_sleep {
            self.reader.as_mut().unwrap().finish();
        }
        self.fake.sleep(s)
    }
    fn print(&mut self, s: &str) -> Result<(), String> {
        self.fake.print(s)?;
        if s == "CHAIN DONE" {
            self.reader = Some(OwnedReader::spawn(
                &self.lock,
                &self.ready,
                &self.release,
                7,
            ));
        }
        Ok(())
    }
}

#[cfg(windows)]
#[test]
fn delete_pending_compatible_reader_allows_immediate_future_acquire() {
    let f = Fixture::new();
    let mut r = Pending {
        fake: Fake::success(),
        lock: f.p.here.join("civ6.lock"),
        ready: f.root.join("compatible-ready"),
        release: f.root.join("compatible-release"),
        reader: None,
        release_on_sleep: true,
    };
    assert_eq!(cli(&mut r, &f.p, &[]).unwrap(), 0);
    let pending = fs::OpenOptions::new().read(true).open(&r.lock).unwrap_err();
    assert_eq!(pending.raw_os_error(), Some(5), "{pending}");
    acquire(&mut r, &f.p, &json!({"pid":77}), 1.0, false).unwrap();
    assert_eq!(r.fake.sleeps, vec![0.05]);
    assert_eq!(read_json(&r.lock).unwrap(), json!({"pid":77}));
}

#[cfg(windows)]
#[test]
fn lease_requeues_after_delete_pending_reader_closes() {
    struct LeasePending {
        fake: Fake,
        lock: PathBuf,
        waiter: PathBuf,
        reader: OwnedReader,
    }
    impl Runner for LeasePending {
        fn run(&mut self, c: Call) -> Result<Output, String> {
            self.fake.run(c)
        }
        fn now(&mut self) -> f64 {
            self.fake.now()
        }
        fn sleep(&mut self, s: f64) -> Result<(), String> {
            assert_eq!(s, 0.05);
            self.reader.finish();
            self.fake.sleep(s)
        }
        fn print(&mut self, s: &str) -> Result<(), String> {
            self.fake.print(s)?;
            if s.starts_with("Civ6 lease expired") {
                assert_eq!(
                    fs::OpenOptions::new()
                        .read(true)
                        .open(&self.lock)
                        .unwrap_err()
                        .raw_os_error(),
                    Some(5)
                );
                fs::remove_file(&self.waiter).unwrap();
            }
            Ok(())
        }
    }
    let f = Fixture::new();
    let token = "my-lease-token";
    let lock = f.json(
        "here/civ6.lock",
        json!({
            "pid":std::process::id(),
            "chain_pid":std::process::id(),
            "live_chain":true,
            "lock_token":token,
            "acquired_at":1000
        }),
    );
    f.json("jobs/waiter/job.json", json!({"status":"running"}));
    let waiter = f.json(
        "here/queue/waiter.json",
        json!({"job":"waiter","since":1000,"long":0}),
    );
    let reader = OwnedReader::spawn(
        &lock,
        &f.root.join("lease-ready"),
        &f.root.join("lease-release"),
        7,
    );
    let mut r = LeasePending {
        fake: Fake {
            time: 2800.0,
            ..Fake::success()
        },
        lock: lock.clone(),
        waiter,
        reader,
    };
    assert!(yield_if_expired(&mut r, &f.p, token).unwrap());
    assert_eq!(r.fake.sleeps, vec![0.05]);
    let restored = read_json(lock).unwrap();
    assert_eq!(restored["lock_token"], token);
    assert_eq!(restored["fifo"], true);
}

#[cfg(windows)]
#[test]
fn delete_pending_access_denied_is_bounded_and_preserves_the_reader() {
    let f = Fixture::new();
    let mut r = Pending {
        fake: Fake::success(),
        lock: f.p.here.join("civ6.lock"),
        ready: f.root.join("persistent-ready"),
        release: f.root.join("persistent-release"),
        reader: None,
        release_on_sleep: false,
    };
    assert_eq!(cli(&mut r, &f.p, &[]).unwrap(), 0);
    let error = acquire(&mut r, &f.p, &json!({"pid":77}), 3600.0, false).unwrap_err();
    assert!(error.contains("civ6.lock"), "{error}");
    assert!(error.contains(&r.lock.display().to_string()), "{error}");
    assert!(error.contains("os error 5"), "{error}");
    assert!(r.fake.time >= 5.0 && r.fake.time < 5.1);
    assert!(r
        .reader
        .as_mut()
        .unwrap()
        .child
        .as_mut()
        .unwrap()
        .try_wait()
        .unwrap()
        .is_none());
    assert_eq!(fs::read_dir(f.p.here.join("queue")).unwrap().count(), 0);
    r.reader.as_mut().unwrap().finish();
    acquire(&mut r, &f.p, &json!({"pid":77}), 1.0, false).unwrap();
    assert_eq!(read_json(&r.lock).unwrap(), json!({"pid":77}));
}

#[cfg(windows)]
#[test]
fn release_waits_for_real_reader_then_deletes_only_its_own_lock() {
    use std::{
        os::windows::process::CommandExt,
        process::{Child, Command, Stdio},
        thread,
        time::Duration,
    };
    struct Blocked {
        fake: Fake,
        lock: PathBuf,
        ready: PathBuf,
        release: PathBuf,
        child: Option<Child>,
    }
    impl Runner for Blocked {
        fn run(&mut self, c: Call) -> Result<Output, String> {
            if c.args[0] == "taskkill" {
                // The verified DELETE handle stays open while cleanup runs.
                assert_eq!(
                    fs::remove_file(&self.lock).unwrap_err().raw_os_error(),
                    Some(32)
                );
                assert_eq!(
                    fs::write(&self.lock, "replacement")
                        .unwrap_err()
                        .raw_os_error(),
                    Some(32)
                );
            }
            self.fake.run(c)
        }
        fn now(&mut self) -> f64 {
            self.fake.now()
        }
        fn sleep(&mut self, s: f64) -> Result<(), String> {
            assert_eq!(s, 0.05);
            fs::write(&self.release, "release").unwrap();
            assert!(self.child.take().unwrap().wait().unwrap().success());
            self.fake.sleep(s)
        }
        fn print(&mut self, s: &str) -> Result<(), String> {
            self.fake.print(s)?;
            if s == "CHAIN DONE" {
                let child = Command::new(std::env::current_exe().unwrap())
                    .args(["--ignored", "--exact", "release_reader_child"])
                    .env("CC_LIVE_TEST_LOCK", &self.lock)
                    .env("CC_LIVE_TEST_READY", &self.ready)
                    .env("CC_LIVE_TEST_RELEASE", &self.release)
                    .creation_flags(CREATE_NO_WINDOW)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap();
                self.child = Some(child);
                for _ in 0..200 {
                    if self.ready.exists() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                assert!(self.ready.exists(), "test reader did not open the lock");
                assert_eq!(
                    fs::remove_file(&self.lock).unwrap_err().raw_os_error(),
                    Some(32)
                );
            }
            Ok(())
        }
    }
    let f = Fixture::new();
    let mut r = Blocked {
        fake: Fake::success(),
        lock: f.p.here.join("civ6.lock"),
        ready: f.root.join("reader-ready"),
        release: f.root.join("reader-release"),
        child: None,
    };
    assert_eq!(cli(&mut r, &f.p, &[]).unwrap(), 0);
    assert_eq!(r.fake.sleeps, vec![0.05]);
    assert_eq!(
        r.fake
            .calls
            .iter()
            .filter(|c| c.args[0] == "taskkill")
            .count(),
        2
    );
    assert!(!r.lock.exists());
}

#[cfg(windows)]
#[test]
fn release_skips_replaced_owner_after_a_sharing_violation() {
    use std::os::windows::fs::OpenOptionsExt;
    struct Swapped {
        fake: Fake,
        lock: PathBuf,
        blocker: Option<fs::File>,
        peer: serde_json::Value,
    }
    impl Runner for Swapped {
        fn run(&mut self, c: Call) -> Result<Output, String> {
            self.fake.run(c)
        }
        fn now(&mut self) -> f64 {
            self.fake.now()
        }
        fn sleep(&mut self, s: f64) -> Result<(), String> {
            drop(self.blocker.take());
            fs::remove_file(&self.lock).unwrap();
            write_json(&self.lock, &self.peer).unwrap();
            self.fake.sleep(s)
        }
        fn print(&mut self, s: &str) -> Result<(), String> {
            self.fake.print(s)?;
            if s == "CHAIN DONE" {
                self.blocker = Some(
                    fs::OpenOptions::new()
                        .read(true)
                        .share_mode(1)
                        .open(&self.lock)
                        .unwrap(),
                );
            }
            Ok(())
        }
    }
    let f = Fixture::new();
    let peer = json!({"live_chain":true,"chain_pid":std::process::id(),"lock_token":"peer-token"});
    let mut r = Swapped {
        fake: Fake::success(),
        lock: f.p.here.join("civ6.lock"),
        blocker: None,
        peer: peer.clone(),
    };
    let error = cli(&mut r, &f.p, &[]).unwrap_err();
    assert!(
        error.contains("ownership changed before cleanup"),
        "{error}"
    );
    assert_eq!(read_json(&r.lock).unwrap(), peer);
    assert!(r.fake.calls.is_empty(), "peer's game must not be closed");
    assert_eq!(r.fake.sleeps, vec![0.05]);
}

#[cfg(windows)]
#[test]
fn release_reports_persistent_sharing_violation_and_keeps_the_lock() {
    use std::os::windows::fs::OpenOptionsExt;
    struct Stuck {
        fake: Fake,
        lock: PathBuf,
        blocker: Option<fs::File>,
    }
    impl Runner for Stuck {
        fn run(&mut self, c: Call) -> Result<Output, String> {
            self.fake.run(c)
        }
        fn now(&mut self) -> f64 {
            self.fake.now()
        }
        fn sleep(&mut self, s: f64) -> Result<(), String> {
            self.fake.sleep(s)
        }
        fn print(&mut self, s: &str) -> Result<(), String> {
            self.fake.print(s)?;
            if s == "CHAIN DONE" {
                self.blocker = Some(
                    fs::OpenOptions::new()
                        .read(true)
                        .share_mode(1)
                        .open(&self.lock)
                        .unwrap(),
                );
            }
            Ok(())
        }
    }
    let f = Fixture::new();
    let mut r = Stuck {
        fake: Fake::success(),
        lock: f.p.here.join("civ6.lock"),
        blocker: None,
    };
    let error = cli(&mut r, &f.p, &[]).unwrap_err();
    assert!(error.contains("open owned lock"), "{error}");
    assert!(error.contains(&r.lock.display().to_string()), "{error}");
    assert!(error.contains("os error 32"), "{error}");
    assert!(r.fake.time >= 5.0 && r.fake.time < 5.1);
    assert_eq!(
        r.fake
            .calls
            .iter()
            .filter(|c| c.args[0] == "taskkill")
            .count(),
        2
    );
    drop(r.blocker.take());
    assert!(r.lock.exists());
}
fn ready_chain(f: &Fixture, run_code: i32) -> Fake {
    let prepare = format!(
        "{}\n{}\n",
        path(f.root.join("scenario.json")),
        path(f.root.join("artifacts"))
    );
    Fake::outputs(&[
        (&prepare, "", 0),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("[OK] one\n[FAIL] two\nnoise\n", "", run_code),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
    ])
}
#[test]
fn chain_closes_game_after_each_run_and_on_exit() {
    let f = Fixture::new();
    let mut r = ready_chain(&f, 0);
    assert_eq!(
        cli(&mut r, &f.p, &["name:s.json".into(), "--test-mode".into()]).unwrap(),
        0
    );
    let scenario = r
        .calls
        .iter()
        .find(|c| c.args[0] == "fixture-python")
        .unwrap();
    assert!(scenario.merge_stderr);
    assert_eq!(scenario.args.last().unwrap(), "--test-mode");
    assert_eq!(
        r.calls.iter().filter(|c| c.args[0] == "taskkill").count(),
        4
    );
    assert!(r
        .printed
        .iter()
        .any(|s| s == &format!("RESULT name s.json 0 {}", path(f.root.join("artifacts")))));
    assert_eq!(r.printed.last().unwrap(), "CHAIN DONE");
    assert_eq!(
        read_text(f.root.join("artifacts.log")).unwrap(),
        "[OK] one\n[FAIL] two\nnoise\n"
    );
}
#[test]
fn chain_stop_on_failure_retains_exit_code_and_cleans() {
    let f = Fixture::new();
    let mut r = ready_chain(&f, 7);
    assert_eq!(
        cli(
            &mut r,
            &f.p,
            &[
                "name:s.json".into(),
                "--stop-on-failure".into(),
                "never:next.json".into()
            ]
        )
        .unwrap(),
        7
    );
    assert_eq!(r.printed.last().unwrap(), "CHAIN STOPPED ON FAILURE");
    assert_eq!(
        r.calls
            .iter()
            .filter(|c| c.args[0].ends_with("ship.exe"))
            .count(),
        1
    );
    assert!(!f.p.here.join("civ6.lock").exists());
}

fn rust_driver_fixture(f: &Fixture) {
    let live = f.p.live();
    let manifest = live.join(driver::MANIFEST);
    fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    fs::write(
        manifest,
        "[package]\nname='civ6-autotest-rs'\nversion='0.1.0'\n",
    )
    .unwrap();
    for name in ["civ6-autotest", "civ6-tuner"] {
        let name = if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        };
        let executable = live.join("tools/target/release").join(name);
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(executable, "fixture executable").unwrap();
    }
}

fn cargo_artifacts(f: &Fixture) -> String {
    ["civ6-autotest", "civ6-tuner"].iter().map(|name| {
        let filename = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
        json!({"reason":"compiler-artifact", "manifest_path":path(f.p.live().join(driver::MANIFEST)), "target":{"name":name,"kind":["bin"]}, "executable":path(f.p.live().join("tools/target/release").join(filename))}).to_string()
    }).collect::<Vec<_>>().join("\n")
}

#[test]
fn rust_driver_is_rebuilt_from_checked_out_branch_and_never_uses_python() {
    let f = Fixture::new();
    rust_driver_fixture(&f);
    let artifacts = cargo_artifacts(&f);
    let mut r = Fake::outputs(&[(&artifacts, "", 0), (&artifacts, "", 0)]);
    driver::prepare(&mut r, &f.p, &f.p.live()).unwrap();
    driver::prepare(&mut r, &f.p, &f.p.live()).unwrap();
    assert_eq!(r.calls.len(), 2);
    for call in &r.calls {
        assert!(call
            .args
            .windows(2)
            .any(|a| a == ["--manifest-path", &path(f.p.live().join(driver::MANIFEST))]));
        assert!(call
            .args
            .windows(2)
            .any(|a| a == ["--target-dir", &path(f.p.live().join("tools/target"))]));
        assert!(call
            .args
            .windows(2)
            .any(|a| a == ["--bin", "civ6-autotest"]));
        assert!(call.args.windows(2).any(|a| a == ["--bin", "civ6-tuner"]));
        assert!(call.args.contains(&"--locked".into()));
        assert!(call.kill_tree);
        assert!(call.args.contains(&"--message-format=json".into()));
    }
    let scenario = driver::scenario(&f.p, &f.p.live()).unwrap();
    assert_eq!(scenario.len(), 1);
    assert!(scenario[0].contains("tools"));
    assert!(!scenario[0].contains("fixture-python"));
    let tuner = driver::tuner(&f.p, &f.p.live()).unwrap();
    assert_eq!(tuner.len(), 1);
    assert!(tuner[0].contains("civ6-tuner"));
}

#[test]
fn rust_driver_build_failure_keeps_legacy_driver_unused_even_with_stale_binary() {
    let f = Fixture::new();
    rust_driver_fixture(&f);
    let mut r = Fake::outputs(&[("compiler output", "compile error", 42)]);
    let error = driver::prepare(&mut r, &f.p, &f.p.live()).unwrap_err();
    assert!(error.contains("42"));
    assert!(error.contains("compile error"));
    assert_eq!(r.calls.len(), 1);
    assert_ne!(r.calls[0].args[0], "fixture-python");
}

#[test]
fn rust_driver_missing_binary_is_an_error_instead_of_a_legacy_fallback() {
    let f = Fixture::new();
    rust_driver_fixture(&f);
    let executable = driver::scenario(&f.p, &f.p.live()).unwrap().remove(0);
    fs::remove_file(executable).unwrap();
    assert!(driver::scenario(&f.p, &f.p.live())
        .unwrap_err()
        .contains("not been built"));
    assert!(driver::prepare(&mut Fake::success(), &f.p, &f.p.live())
        .unwrap_err()
        .contains("did not produce"));
}

#[test]
fn branches_without_rust_driver_keep_their_own_legacy_commands() {
    let f = Fixture::new();
    let mut r = Fake::success();
    driver::prepare(&mut r, &f.p, &f.p.live()).unwrap();
    assert!(r.calls.is_empty());
    assert_eq!(
        driver::scenario(&f.p, &f.p.live()).unwrap(),
        args(&[
            "fixture-python",
            &path(f.p.live().join("tools/civ6-autotest/scenario.py"))
        ])
    );
    assert_eq!(
        driver::tuner(&f.p, &f.p.live()).unwrap(),
        args(&[
            "fixture-python",
            &path(f.p.live().join("tools/civ6-autotest/tuner.py"))
        ])
    );
}

#[test]
fn chain_passes_legacy_flags_and_artifact_paths_to_rust_driver() {
    let f = Fixture::new();
    rust_driver_fixture(&f);
    let mut r = ready_chain(&f, 0);
    assert_eq!(
        cli(&mut r, &f.p, &["name:s.json".into(), "--test-mode".into()]).unwrap(),
        0
    );
    let expected = driver::scenario(&f.p, &f.p.live()).unwrap().remove(0);
    let scenario = r.calls.iter().find(|c| c.args[0] == expected).unwrap();
    assert_eq!(
        scenario.args,
        vec![
            expected,
            path(f.root.join("scenario.json")),
            "--no-auto-restart".into(),
            "--output".into(),
            path(f.root.join("artifacts")),
            "--test-mode".into()
        ]
    );
    assert!(scenario.merge_stderr);
    assert!(!r.calls.iter().any(|c| c.args[0] == "fixture-python"));
    assert_eq!(
        r.calls.iter().filter(|c| c.args[0] == "taskkill").count(),
        4
    );
    assert!(!f.p.here.join("civ6.lock").exists());
}

#[test]
fn responsive_game_probe_uses_branch_rust_tuner() {
    let f = Fixture::new();
    rust_driver_fixture(&f);
    let mut r = Fake::outputs(&[("\"CivilizationVI.exe\",\"11\"", "", 0)]);
    assert_eq!(ensure_responsive_civ6(&mut r, &f.p).unwrap(), "killed");
    let tuner = driver::tuner(&f.p, &f.p.live()).unwrap().remove(0);
    assert_eq!(r.calls[1].args, vec![tuner, "states".into()]);
    assert!(!r.calls.iter().any(|c| c.args[0] == "fixture-python"));
    assert_eq!(r.sleeps, vec![45.0]);
}

#[test]
fn build_in_another_target_directory_rejects_stale_driver_files() {
    let f = Fixture::new();
    rust_driver_fixture(&f);
    let stale_message = cargo_artifacts(&f).replace("release", "configured-target/release");
    let mut r = Fake::outputs(&[(&stale_message, "", 0)]);
    assert!(driver::prepare(&mut r, &f.p, &f.p.live())
        .unwrap_err()
        .contains("refusing a stale executable"));
}

#[test]
fn startup_diagnostic_worker_is_built_and_its_provenance_is_required() {
    let f = Fixture::new();
    rust_driver_fixture(&f);
    let source =
        f.p.live()
            .join("tools/civ6-autotest-rs/src/bin/civ6-startup-diagnostics.rs");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, "fn main() {}").unwrap();
    let filename = if cfg!(windows) {
        "civ6-startup-diagnostics.exe"
    } else {
        "civ6-startup-diagnostics"
    };
    let executable = f.p.live().join("tools/target/release").join(filename);
    fs::write(&executable, "old diagnostic worker").unwrap();
    let old_artifacts = cargo_artifacts(&f);
    let mut r = Fake::outputs(&[(&old_artifacts, "", 0)]);
    assert!(driver::prepare(&mut r, &f.p, &f.p.live())
        .unwrap_err()
        .contains("refusing a stale executable"));
    assert!(r.calls[0]
        .args
        .windows(2)
        .any(|a| a == ["--bin", "civ6-startup-diagnostics"]));
    let artifacts = format!(
        "{}\n{}",
        old_artifacts,
        json!({"reason":"compiler-artifact", "manifest_path":path(f.p.live().join(driver::MANIFEST)), "target":{"name":"civ6-startup-diagnostics","kind":["bin"]}, "executable":path(&executable)})
    );
    driver::prepare(
        &mut Fake::outputs(&[(&artifacts, "", 0)]),
        &f.p,
        &f.p.live(),
    )
    .unwrap();
}

#[test]
fn missing_startup_helper_source_refuses_existing_stale_binary_before_build() {
    let f = Fixture::new();
    rust_driver_fixture(&f);
    let startup = f.p.live().join("tools/civ6-autotest-rs/src/startup.rs");
    fs::create_dir_all(startup.parent().unwrap()).unwrap();
    fs::write(startup, "pub fn recovery() {}").unwrap();
    fs::write(
        f.p.live()
            .join("tools/target/release/civ6-startup-diagnostics.exe"),
        "old helper",
    )
    .unwrap();
    let mut runner = Fake::success();
    assert!(driver::prepare(&mut runner, &f.p, &f.p.live())
        .unwrap_err()
        .contains("source is missing"));
    assert!(runner.calls.is_empty());
}

#[cfg(windows)]
#[test]
fn cancelled_build_waits_for_real_grandchildren_to_exit() {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_INVALID_PARAMETER},
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    let f = Fixture::new();
    let marker = f.root.join("grandchild.pid");
    let escaped = path(&marker).replace('\'', "''");
    let script = format!("$info = New-Object System.Diagnostics.ProcessStartInfo; $info.FileName = 'cmd.exe'; $info.Arguments = '/d /c ping 127.0.0.1 -n 31 > nul'; $info.UseShellExecute = $false; $info.CreateNoWindow = $true; $child = [System.Diagnostics.Process]::Start($info); [System.IO.File]::WriteAllText('{escaped}', [string]$child.Id); Start-Sleep -Seconds 30");
    let result = process::RealRunner.run(
        Call::new(args(&["powershell", "-NoProfile", "-Command", &script]))
            .managed()
            .timeout(5.0),
    );
    assert!(result.unwrap_err().contains("timed out"));
    let pid: u32 = read_text(marker).unwrap().parse().unwrap();
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            assert_eq!(GetLastError(), ERROR_INVALID_PARAMETER);
        } else {
            let mut code = 259;
            let success = GetExitCodeProcess(handle, &mut code);
            CloseHandle(handle);
            assert_ne!(success, 0);
            assert_ne!(code, 259, "Grandchild survived the cancelled build");
        }
    }
}

#[test]
fn ship_builds_checked_out_rust_driver_before_publishing_scenario_and_artifacts() {
    let f = Fixture::new();
    rust_driver_fixture(&f);
    f.json(
        "wt/live/tools/civ6-autotest/scenarios/s.json",
        json!({"name":"s","steps":[{"action":"pause","seconds":1}]}),
    );
    let artifacts = cargo_artifacts(&f);
    let mut r = Fake::outputs(&[("", "", 0), ("", "", 0), ("", "", 0), (&artifacts, "", 0)]);
    let (scenario, _) = ship::live(&mut r, &f.p, "name", "s.json", true).unwrap();
    assert_eq!(&r.calls[0].args[3..5], &["fetch", "-q"]);
    assert_eq!(&r.calls[1].args[3..5], &["checkout", "-q"]);
    assert_eq!(&r.calls[2].args[3..5], &["merge", "-q"]);
    assert!(r.calls[3].args.contains(&"build".into()));
    assert!(r.calls[3].kill_tree);
    assert_eq!(r.printed.len(), 2);
    assert!(std::path::Path::new(&scenario).is_file());

    let f = Fixture::new();
    rust_driver_fixture(&f);
    f.json(
        "wt/live/tools/civ6-autotest/scenarios/s.json",
        json!({"name":"s","steps":[{"action":"pause","seconds":1}]}),
    );
    let mut r = Fake::outputs(&[
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("", "build failed", 1),
    ]);
    assert!(ship::live(&mut r, &f.p, "name", "s.json", true)
        .unwrap_err()
        .contains("build failed"));
    assert!(r.printed.is_empty());
    assert!(!r.calls.iter().any(|c| c.args[0] == "fixture-python"));
    assert!(!f.p.temp.join("live_name_01.json").exists());
}
#[test]
fn preparation_failure_retries_once_and_stops() {
    let f = Fixture::new();
    let mut r = Fake::outputs(&[
        ("", "first", 1),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("", "checkout failed", 1),
    ]);
    assert_eq!(
        main(
            &mut r,
            &f.p,
            &["a:s.json".into(), "--stop-on-failure".into()]
        )
        .unwrap(),
        1
    );
    assert_eq!(r.sleeps, vec![15.0]);
    assert_eq!(
        r.printed,
        vec!["RESULT a s.json prepare-failed checkout failed"]
    );
}
#[test]
fn scenario_error_still_closes_and_releases() {
    let f = Fixture::new();
    let mut r = ready_chain(&f, 0);
    r.replies[4] = Err("scenario spawn failed".into());
    assert!(cli(&mut r, &f.p, &["a:s.json".into()])
        .unwrap_err()
        .contains("spawn failed"));
    assert!(r.calls.iter().any(|c| c.args[0] == "taskkill"));
    assert!(!f.p.here.join("civ6.lock").exists());
}
#[test]
fn ship_merges_main_and_suffixes_save_names_without_changing_source() {
    let f = Fixture::new();
    let source=f.json("wt/live/tools/civ6-autotest/scenarios/s.json",json!({"steps":[{"action":"save","name":"元"},{"action":"load","name":"fixed","exact_name":true}]}));
    fs::create_dir_all(f.p.live().join("tools/civ6-autotest/artifacts/name-01")).unwrap();
    fs::create_dir_all(
        f.p.profile
            .join("Documents/My Games/Sid Meier's Civilization VI/Mods/JAC_Missing"),
    )
    .unwrap();
    let mut r = Fake::success();
    let (target, artifact) = ship::live(&mut r, &f.p, "feat/name", "s.json", true).unwrap();
    let data = read_json(target).unwrap();
    assert_eq!(
        data["steps"][0],
        json!({"action":"restart_game","timeout":420})
    );
    assert_eq!(data["steps"][1]["name"], "元-name-02");
    assert_eq!(data["steps"][2]["name"], "fixed");
    assert!(artifact.ends_with("name-02"));
    assert_eq!(
        r.calls[2].args[3..],
        args(&["merge", "-q", "--no-edit", "origin/main"])
    );
    assert!(f.p.live().join("poc/JAC_Missing").exists());
    assert_eq!(
        read_json(source).unwrap()["steps"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
#[test]
fn merge_conflict_is_aborted_and_warned() {
    let f = Fixture::new();
    f.json(
        "wt/live/tools/civ6-autotest/scenarios/s.json",
        json!({"steps":[{"action":"restart_game"}]}),
    );
    let mut r = Fake::outputs(&[("", "", 0), ("", "", 0), ("", "conflict", 1), ("", "", 0)]);
    ship::live(&mut r, &f.p, "name", "s.json", true).unwrap();
    assert_eq!(r.calls[3].args[3..], args(&["merge", "--abort"]));
    assert_eq!(
        r.errors,
        vec![
            "warning: origin/main does not merge cleanly into feat/name; testing the branch alone"
        ]
    );
}
#[test]
fn main_branch_skips_merge_and_no_restart_keeps_first_action() {
    let f = Fixture::new();
    f.json(
        "wt/live/tools/civ6-autotest/scenarios/s.json",
        json!({"steps":[{"action":"save","name":"x"}]}),
    );
    let mut r = Fake::success();
    let (target, _) = ship::live(&mut r, &f.p, "main", "s.json", false).unwrap();
    assert_eq!(r.calls.len(), 2);
    assert_eq!(r.calls[1].args.last().unwrap(), "origin/main");
    assert_eq!(read_json(target).unwrap()["steps"][0]["action"], "save");
}
#[test]
fn failed_checkout_does_not_prepare_scenario() {
    let f = Fixture::new();
    let mut r = Fake::outputs(&[("", "", 0), ("", "locked", 1)]);
    assert!(ship::live(&mut r, &f.p, "name", "s.json", true)
        .unwrap_err()
        .contains("locked"));
    assert!(r.printed.is_empty());
}
#[test]
fn last_message_uses_latest_run_and_strips_absolute_links() {
    let f = Fixture::new();
    for (run, text) in [
        ("001-name", "old"),
        (
            "002-name",
            "  [file](D:/src/a.md) [unix](/tmp/a.md) [web](https://example.com)  ",
        ),
    ] {
        let dir = f.p.jobs.join(run);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("last_message.md"), text).unwrap();
    }
    assert_eq!(
        ship::last_message(&f.p, "name").unwrap(),
        "`file` `unix` [web](https://example.com)"
    );
    assert_eq!(ship::last_message(&f.p, "missing").unwrap(), "");
}
#[test]
fn pr_uses_utf8_draft_payload_and_prints_api_url() {
    let f = Fixture::new();
    let mut r = Fake::outputs(&[
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("https://example.com/pr/1\n", "", 0),
    ]);
    ship::pr(&mut r, &f.p, "name", "日本語の題名", "commit").unwrap();
    let payload = read_json(f.p.temp.join("pr_name.json")).unwrap();
    assert_eq!(payload["title"], "日本語の題名");
    assert_eq!(payload["draft"], true);
    assert_eq!(r.printed, vec!["https://example.com/pr/1"]);
    assert_eq!(r.calls[3].args[0], "gh");
}
#[test]
fn review_preserves_facts_and_uses_jobs_executable() {
    let f = Fixture::new();
    let facts = f.root.join("facts.txt");
    fs::write(&facts, "  結果 {name}\n").unwrap();
    let mut r = Fake::outputs(&[("notice\njob-id\n", "", 0)]);
    review::cli(&mut r, &f.p, &["name".into(), "123".into(), path(facts)]).unwrap();
    let prompt = read_text(f.p.here.join("rev_name_in.txt")).unwrap();
    assert!(prompt.starts_with("Review PR #123"));
    assert!(prompt.contains("結果 {name}\n"));
    assert_eq!(
        r.calls[0].args[0],
        path(f.p.jobs.parent().unwrap().join("codex_jobs.exe"))
    );
    assert!(r.calls[0]
        .args
        .windows(2)
        .any(|s| s == ["--sandbox", "read-only"]));
    assert_eq!(r.printed, vec!["job-id"]);
}
#[test]
fn merge_pr_posts_marks_ready_and_squashes_in_order() {
    let f = Fixture::new();
    let md = f.root.join("comment.md");
    fs::write(&md, "検証済み\n").unwrap();
    let mut r = Fake::outputs(&[
        ("url\n", "", 0),
        ("", "ignored", 1),
        ("", "merge failed", 1),
    ]);
    assert_eq!(
        review::merge_cli(&mut r, &f.p, &["3".into(), path(md)]).unwrap(),
        0
    );
    assert_eq!(r.printed, vec!["url", "merge failed"]);
    assert_eq!(r.calls[1].args[2], "ready");
    assert_eq!(r.calls[2].args.last().unwrap(), "--squash");
    assert_eq!(
        read_json(f.p.temp.join("c3.json")).unwrap(),
        json!({"body":"検証済み\n"})
    );
}

#[test]
fn panic_still_closes_game_and_releases_lock() {
    struct PanicRunner(Fake);
    impl Runner for PanicRunner {
        fn run(&mut self, c: Call) -> Result<Output, String> {
            self.0.run(c)
        }
        fn now(&mut self) -> f64 {
            self.0.now()
        }
        fn sleep(&mut self, s: f64) -> Result<(), String> {
            self.0.sleep(s)
        }
        fn print(&mut self, _: &str) -> Result<(), String> {
            panic!("broken stdout");
        }
    }
    let f = Fixture::new();
    let mut r = PanicRunner(Fake::success());
    assert!(cli(&mut r, &f.p, &[]).unwrap_err().contains("panicked"));
    assert_eq!(
        r.0.calls.iter().filter(|c| c.args[0] == "taskkill").count(),
        2
    );
    assert!(!f.p.here.join("civ6.lock").exists());
}

#[test]
fn windows_fault_code_is_unsigned_in_result() {
    let f = Fixture::new();
    let mut r = ready_chain(&f, 0xc0000005u32 as i32);
    assert_eq!(
        main(
            &mut r,
            &f.p,
            &["name:s.json".into(), "--stop-on-failure".into()]
        )
        .unwrap(),
        0xc0000005u32 as i32
    );
    assert!(r.printed.contains(&format!(
        "RESULT name s.json 3221225477 {}",
        path(f.root.join("artifacts"))
    )));
    assert_eq!(python_cli_exit(0xc0000005u32 as i32), -1);
    assert_eq!(python_cli_exit(7), 7);
}

#[test]
fn restart_retry_preserves_first_artifacts_and_closes_each_run() {
    let f = Fixture::new();
    let artifacts = f.root.join("artifacts");
    fs::create_dir(&artifacts).unwrap();
    fs::write(artifacts.join("first.txt"), "first attempt").unwrap();
    fs::create_dir(f.root.join("artifacts-try1")).unwrap();
    let prepare = format!("scenario.json\n{}\n", path(&artifacts));
    let mut r = Fake::outputs(&[
        (&prepare, "", 0),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("[FAIL] 1/ restart_game", "", 1),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("PASS second attempt", "", 0),
    ]);
    assert_eq!(main(&mut r, &f.p, &["a:s.json".into()]).unwrap(), 0);
    assert_eq!(r.sleeps, vec![60.0]);
    assert_eq!(
        read_text(f.root.join("artifacts-try1/artifacts/first.txt")).unwrap(),
        "first attempt"
    );
    assert_eq!(
        r.calls
            .iter()
            .filter(|c| c.args[0] == "fixture-python")
            .count(),
        2
    );
    assert_eq!(
        r.calls.iter().filter(|c| c.args[0] == "taskkill").count(),
        4
    );
}

#[test]
fn recovered_startup_failure_does_not_launch_another_outer_retry() {
    let f = Fixture::new();
    let artifacts = f.root.join("artifacts");
    fs::create_dir(&artifacts).unwrap();
    fs::write(artifacts.join("diagnostic.json"), "saved before shutdown").unwrap();
    let prepare = format!("scenario.json\n{}\n", path(&artifacts));
    let mut r = Fake::outputs(&[
        (&prepare, "", 0),
        ("", "", 0),
        ("", "", 0),
        ("", "", 0),
        ("STARTUP_RECOVERY_ATTEMPTED\n[FAIL] 1/ restart_game", "", 1),
    ]);
    assert_eq!(
        main(
            &mut r,
            &f.p,
            &["a:s.json".into(), "--stop-on-failure".into()]
        )
        .unwrap(),
        1
    );
    let drivers: Vec<_> = r
        .calls
        .iter()
        .filter(|c| c.args[0] == "fixture-python")
        .collect();
    assert_eq!(drivers.len(), 1);
    assert_eq!(
        drivers[0].env["CIV6_LIVE_CHAIN_PID"],
        std::process::id().to_string()
    );
    assert_eq!(
        drivers[0].env["CIV6_LIVE_LOCK_PATH"],
        path(f.p.here.join("civ6.lock"))
    );
    assert!(r.sleeps.is_empty());
    assert!(!f.root.join("artifacts-try1").exists());
    assert_eq!(
        read_text(artifacts.join("diagnostic.json")).unwrap(),
        "saved before shutdown"
    );
    assert_eq!(
        r.calls.iter().filter(|c| c.args[0] == "taskkill").count(),
        2
    );
}

#[test]
fn child_specific_environment_is_applied_without_changing_the_parent() {
    let key = "CIV6_LIVE_TEST_CHILD_ENV";
    let previous = std::env::var_os(key);
    let mut r = process::RealRunner;
    let out = r
        .run(
            Call::new(args(&[
                "cmd.exe",
                "/d",
                "/c",
                "set CIV6_LIVE_TEST_CHILD_ENV",
            ]))
            .env(key, "chain-owned")
            .timeout(5.0),
        )
        .unwrap();
    assert_eq!(out.code, 0);
    assert_eq!(out.stdout, "CIV6_LIVE_TEST_CHILD_ENV=chain-owned\n");
    assert_eq!(std::env::var_os(key), previous);
}

#[test]
fn real_child_output_pipe_is_merged_in_order_without_game_or_network() {
    let mut r = process::RealRunner;
    let out = r
        .run(
            Call::new(args(&[
                "cmd.exe",
                "/d",
                "/c",
                "echo first & echo second 1>&2 & echo third",
            ]))
            .merged()
            .timeout(5.0),
        )
        .unwrap();
    assert_eq!(out.code, 0);
    assert_eq!(out.stdout, "first \nsecond  \nthird\n");
    assert_eq!(out.stderr, "");
}

#[test]
fn broken_stdout_stops_before_scenario_and_still_cleans() {
    struct BrokenWriter(Fake);
    impl Runner for BrokenWriter {
        fn run(&mut self, c: Call) -> Result<Output, String> {
            self.0.run(c)
        }
        fn now(&mut self) -> f64 {
            self.0.now()
        }
        fn sleep(&mut self, s: f64) -> Result<(), String> {
            self.0.sleep(s)
        }
        fn print(&mut self, _: &str) -> Result<(), String> {
            Err("broken pipe".into())
        }
    }
    let f = Fixture::new();
    let mut r = BrokenWriter(ready_chain(&f, 0));
    assert!(
        cli(&mut r, &f.p, &["a:s.json".into(), "never:s.json".into()])
            .unwrap_err()
            .contains("broken pipe")
    );
    assert!(!r.0.calls.iter().any(|c| c.args[0] == "fixture-python"));
    assert_eq!(
        r.0.calls
            .iter()
            .filter(|c| c.args[0].ends_with("ship.exe"))
            .count(),
        1
    );
    assert_eq!(
        r.0.calls.iter().filter(|c| c.args[0] == "taskkill").count(),
        2
    );
    assert!(!f.p.here.join("civ6.lock").exists());
}

#[test]
fn contention_lease_keeps_uncontended_and_not_yet_expired_holder() {
    let f = Fixture::new();
    f.json(
        "here/civ6.lock",
        json!({"job":"holder", "acquired_at":1000}),
    );
    let mut r = Fake {
        time: 10000.,
        ..Fake::default()
    };
    let todo = f.root.join("todo.json");
    let mut lock = fairness::Lock::open(&f.p).unwrap().unwrap();
    assert!(!lock.expired(&mut r, &f.p, &todo).unwrap());
    assert!(lock.info["competitor_since"].is_null());
    f.json("jobs/waiter/job.json", json!({"status":"running"}));
    f.json(
        "here/queue/waiter.json",
        json!({"job":"waiter","since":10000}),
    );
    assert!(!lock.expired(&mut r, &f.p, &todo).unwrap());
    r.time = 11799.;
    assert!(!lock.expired(&mut r, &f.p, &todo).unwrap());
    r.time = 11800.;
    assert!(lock.expired(&mut r, &f.p, &todo).unwrap());
    fs::remove_file(f.p.here.join("queue/waiter.json")).unwrap();
    assert!(!lock.expired(&mut r, &f.p, &todo).unwrap());
    assert!(lock.info["competitor_since"].is_null());
}

#[test]
fn lease_yield_does_not_release_another_token_with_the_same_pid() {
    let f = Fixture::new();
    let peer = json!({
        "live_chain": true,
        "chain_pid": std::process::id(),
        "lock_token": "peer-token",
        "acquired_at": 1000
    });
    let lock = f.json("here/civ6.lock", peer.clone());
    assert!(yield_if_expired(&mut Fake::success(), &f.p, "my-token")
        .unwrap_err()
        .contains("ownership changed before scenario"));
    assert_eq!(read_json(lock).unwrap(), peer);
}

#[test]
fn chain_stops_before_a_second_scenario_if_its_lock_was_replaced() {
    struct Replaced {
        fake: Fake,
        lock: PathBuf,
        peer: serde_json::Value,
    }
    impl Runner for Replaced {
        fn run(&mut self, c: Call) -> Result<Output, String> {
            self.fake.run(c)
        }
        fn now(&mut self) -> f64 {
            self.fake.now()
        }
        fn sleep(&mut self, s: f64) -> Result<(), String> {
            self.fake.sleep(s)
        }
        fn print(&mut self, s: &str) -> Result<(), String> {
            self.fake.print(s)?;
            if s.starts_with("RESULT a s.json") {
                fs::remove_file(&self.lock).unwrap();
                write_json(&self.lock, &self.peer).unwrap();
            }
            Ok(())
        }
    }
    let f = Fixture::new();
    let peer = json!({"live_chain":true,"chain_pid":std::process::id(),"lock_token":"peer-token"});
    let mut r = Replaced {
        fake: ready_chain(&f, 0),
        lock: f.p.here.join("civ6.lock"),
        peer: peer.clone(),
    };
    let error = cli(&mut r, &f.p, &["a:s.json".into(), "b:s.json".into()]).unwrap_err();
    assert!(error.contains("ownership changed"), "{error}");
    assert_eq!(read_json(&r.lock).unwrap(), peer);
    assert_eq!(
        r.fake
            .calls
            .iter()
            .filter(|c| c.args[0].ends_with("ship.exe"))
            .count(),
        1
    );
    assert_eq!(
        r.fake
            .calls
            .iter()
            .filter(|c| c.args[0] == "taskkill")
            .count(),
        2
    );
}

#[test]
fn live_chain_finishes_scenario_then_requeues_fifo_before_next() {
    struct Contended {
        p: Paths,
        time: f64,
        scenarios: usize,
        yielded: bool,
    }
    impl Runner for Contended {
        fn now(&mut self) -> f64 {
            self.time
        }
        fn run(&mut self, c: Call) -> Result<Output, String> {
            if c.args[0].ends_with("ship.exe") {
                if self.scenarios == 1 {
                    assert!(self.yielded);
                }
                return Ok(Output {
                    stdout: format!(
                        "scenario.json\n{}\n",
                        path(self.p.temp.join(format!("run{}", self.scenarios)))
                    ),
                    ..Output::default()
                });
            }
            if c.args[0] == "fixture-python" {
                assert!(self.p.here.join("civ6.lock").exists());
                self.scenarios += 1;
                if self.scenarios == 1 {
                    write_json(
                        self.p.here.join("queue/waiter.json"),
                        &json!({"job":"waiter","since":1000,"long":1}),
                    )?;
                    self.time = 2800.;
                    // Tick may observe expiration, but cannot remove an active chain.
                    let mut lock = fairness::Lock::open(&self.p)?.unwrap();
                    assert!(lock.expired(self, &self.p.clone(), &self.p.temp.join("todo.json"))?);
                    assert!(self.p.here.join("civ6.lock").exists());
                }
            }
            Ok(Output::default())
        }
        fn sleep(&mut self, seconds: f64) -> Result<(), String> {
            // Acquisition must yield even to an older long waiter when we're short.
            assert_eq!(self.scenarios, 1);
            assert!(!self.p.here.join("civ6.lock").exists());
            let mut mine = Vec::new();
            for entry in fs::read_dir(self.p.here.join("queue")).map_err(|e| e.to_string())? {
                let path = entry.map_err(|e| e.to_string())?.path();
                if !path.ends_with("waiter.json") {
                    mine.push(read_json(path)?);
                }
            }
            assert_eq!(mine.len(), 1);
            assert_eq!(mine[0]["fifo"], true);
            assert!(mine[0]["since"].as_f64().unwrap() > 1000.);
            fs::remove_file(self.p.here.join("queue/waiter.json")).unwrap();
            self.yielded = true;
            self.time += seconds;
            Ok(())
        }
        fn print(&mut self, _: &str) -> Result<(), String> {
            Ok(())
        }
    }
    let f = Fixture::new();
    f.json("jobs/waiter/job.json", json!({"status":"running"}));
    let mut r = Contended {
        p: f.p.clone(),
        time: 1000.,
        scenarios: 0,
        yielded: false,
    };
    assert_eq!(
        cli(&mut r, &f.p, &["a:s.json".into(), "b:s.json".into()]).unwrap(),
        0
    );
    assert_eq!(r.scenarios, 2);
    assert!(r.yielded);
    assert!(!f.p.here.join("civ6.lock").exists());
}

#[test]
fn ready_scheduler_item_counts_as_competitor_and_blocks_requeued_chain() {
    let f = Fixture::new();
    f.json("todo/todo.json", json!({"items":[{"id":437,"created_s":1000,"status":"todo","dispatchable":true,"spec":"s.md","needs":["civ6"]}]}));
    f.json("here/civ6.lock", json!({"job":"holder"}));
    let mut r = Fake {
        time: 2800.,
        ..Fake::default()
    };
    let mut lock = fairness::Lock::open(&f.p).unwrap().unwrap();
    assert!(lock
        .expired(&mut r, &f.p, &fairness::todo_path(&f.p))
        .unwrap());
    assert_eq!(lock.info["competitor_since"], 1000.);
    let me = f.json(
        "here/queue/me.json",
        json!({"pid":123,"since":2800,"long":0,"fifo":true}),
    );
    assert!(!my_turn(&mut r, &f.p, &me).unwrap());
}
