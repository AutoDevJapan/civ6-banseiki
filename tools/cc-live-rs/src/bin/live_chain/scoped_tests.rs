use super::*;
use serde_json::json;

const ORIGINAL: &[u8] = b"Version 15\r\nDeviceID 5056\r\nAllowSWRenderer 0\r\nPlayIntroVideo 0\r\n";
const MODIFIED: &[u8] = b"Version 15\r\nDeviceID 140\r\nAllowSWRenderer 1\r\nPlayIntroVideo 0\r\n";
struct Fixture {
    root: PathBuf,
    paths: Paths,
    options: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("fixture-{}", unique_suffix()));
        fs::create_dir(&root).unwrap();
        let paths = Paths {
            here: root.join("here"),
            wt: root.join("wt"),
            jobs: root.join("jobs"),
            temp: root.join("temp"),
            profile: root.join("profile"),
            python: "never-executed".into(),
        };
        for dir in [
            &paths.here,
            &paths.wt,
            &paths.jobs,
            &paths.temp,
            &paths.profile,
        ] {
            fs::create_dir_all(dir).unwrap();
        }
        let options = root.join("AppOptions.txt");
        fs::write(&options, ORIGINAL).unwrap();
        fs::write(
            paths.temp.join("scenario.json"),
            r#"{"steps":[{"action":"restart_game","timeout":1}]}"#,
        )
        .unwrap();
        Self {
            root,
            paths,
            options,
        }
    }
    fn owner(&self) -> serde_json::Value {
        json!({"live_chain":true,"pid":std::process::id(),"chain_pid":std::process::id(),
            "pid_created_filetime":self_creation().unwrap(),"lock_token":"fixture-owner"})
    }
    fn lock(&self, owner: &serde_json::Value) {
        cc_live::write_json(self.paths.here.join("civ6.lock"), owner).unwrap();
    }
    fn backups(&self) -> Vec<PathBuf> {
        fs::read_dir(&self.root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "backup"))
            .collect()
    }
    fn runner(&self, mode: Mode) -> Scoped<Model> {
        Scoped {
            inner: Model {
                mode,
                paths: self.paths.clone(),
                options: self.options.clone(),
                printed: vec![],
                releases: 0,
                prepared: false,
                running: false,
                absence_error: false,
                absence_queries: 0,
                time: 1000.0,
                driver_calls: 0,
            },
            profile: None,
            paths: self.paths.clone(),
            options: self.options.clone(),
            backup_dir: self.root.clone(),
            restore_deadline: None,
            start_token: None,
        }
    }
    fn driver(&self) {
        let manifest = self.paths.live().join(cc_live::driver::MANIFEST);
        fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        fs::write(manifest, "fixture, never built").unwrap();
        let driver = self
            .paths
            .live()
            .join("tools/target/release/civ6-autotest.exe");
        fs::create_dir_all(driver.parent().unwrap()).unwrap();
        fs::write(driver, "fixture, never executed").unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Success,
    PrepareFail,
    DriverError,
}
struct Model {
    mode: Mode,
    paths: Paths,
    options: PathBuf,
    printed: Vec<String>,
    releases: usize,
    prepared: bool,
    running: bool,
    absence_error: bool,
    absence_queries: usize,
    time: f64,
    driver_calls: usize,
}
impl Runner for Model {
    fn run(&mut self, call: Call) -> Result<Output, String> {
        // Every command is injected. Never spawn Steam, a game, Python or a tool.
        assert!(call.args.iter().all(|s| !s.contains("python")));
        if call.args[0] == "tasklist" {
            return Ok(Output::default());
        }
        if call
            .args
            .last()
            .is_some_and(|arg| arg == cc_live::chain::REPORTER_COMMAND)
        {
            return Ok(Output::default());
        }
        if call.args.get(1).is_some_and(|s| s == "live") {
            assert_eq!(fs::read(&self.options).unwrap(), ORIGINAL);
            assert!(self.paths.here.join("civ6.lock").exists());
            return Ok(match self.mode {
                Mode::PrepareFail => Output {
                    code: 42,
                    stderr: "source prepare failed".into(),
                    ..Output::default()
                },
                _ => Output {
                    stdout: format!(
                        "{}\n{}\n",
                        self.paths.temp.join("scenario.json").display(),
                        self.paths.temp.join("artifacts").display()
                    ),
                    ..Output::default()
                },
            });
        }
        if call.args[0].ends_with("civ6-autotest.exe") {
            self.driver_calls += 1;
            assert_eq!(fs::read(&self.options).unwrap(), MODIFIED);
            assert!(self.paths.here.join("civ6.lock").exists());
            return match self.mode {
                Mode::DriverError => Err("injected driver failure after START".into()),
                _ => Ok(Output::default()),
            };
        }
        panic!("unexpected external command: {:?}", call.args);
    }
    fn prepare_process_queries(&mut self) -> Result<(), String> {
        self.prepared = true;
        Ok(())
    }
    fn process_identity(
        &mut self,
        _: u32,
        _: f64,
    ) -> Result<Option<cc_live::process::ProcessIdentity>, String> {
        assert!(self.prepared);
        Ok(Some(cc_live::process::ProcessIdentity {
            image: "live_chain.exe".into(),
            created_filetime: Some(self_creation().unwrap()),
        }))
    }
    fn before_lease_release(&mut self, _: f64) -> Result<(), String> {
        assert_eq!(fs::read(&self.options).unwrap(), ORIGINAL);
        assert!(self.paths.here.join("civ6.lock").exists());
        self.releases += 1;
        Ok(())
    }
    fn now(&mut self) -> f64 {
        self.time
    }
    fn monotonic(&mut self) -> f64 {
        self.time
    }
    fn sleep(&mut self, seconds: f64) -> Result<(), String> {
        self.time += seconds;
        Ok(())
    }
    fn print(&mut self, text: &str) -> Result<(), String> {
        self.printed.push(text.into());
        Ok(())
    }
}
impl ProfileRunner for Model {
    fn game_running(&mut self, deadline: Instant) -> Result<bool, String> {
        self.absence_queries += 1;
        if Instant::now() >= deadline {
            return Err("absence deadline expired".into());
        }
        if self.absence_error {
            return Err("injected uncertain game absence".into());
        }
        Ok(self.running)
    }
}

#[test]
fn standard_chain_prepare_failure_applies_zero_and_releases() {
    let f = Fixture::new();
    let mut r = f.runner(Mode::PrepareFail);
    assert_eq!(
        cc_live::chain::cli(
            &mut r,
            &f.paths,
            &cc_live::args(&["fixture:s.json", "--stop-on-failure"])
        )
        .unwrap(),
        42
    );
    assert!(r.inner.prepared);
    assert!(r.profile.is_none());
    assert!(f.backups().is_empty());
    assert_eq!(r.inner.driver_calls, 0);
    assert!(!r
        .inner
        .printed
        .iter()
        .any(|line| line.starts_with("PROFILE APPLY")));
    assert_eq!(r.inner.releases, 1);
    assert_eq!(fs::read(&f.options).unwrap(), ORIGINAL);
    assert!(!f.paths.here.join("civ6.lock").exists());
}
#[test]
fn standard_chain_normal_and_driver_error_restore_before_release() {
    for mode in [Mode::Success, Mode::DriverError] {
        let f = Fixture::new();
        f.driver();
        let mut r = f.runner(mode);
        let result = cc_live::chain::cli(&mut r, &f.paths, &cc_live::args(&["fixture:s.json"]));
        match mode {
            Mode::Success => assert_eq!(result.unwrap(), 0),
            _ => assert!(result.unwrap_err().contains("injected driver failure")),
        }
        assert!(r.inner.prepared);
        assert_eq!(r.inner.driver_calls, 1);
        assert_eq!(r.inner.releases, 1);
        assert!(r.profile.is_none());
        assert_eq!(fs::read(&f.options).unwrap(), ORIGINAL);
        assert_eq!(fs::read(&f.backups()[0]).unwrap(), ORIGINAL);
        assert!(!f.paths.here.join("civ6.lock").exists());
    }
}
#[test]
fn standard_chain_setup_failure_without_close_notification_restores_at_release() {
    let f = Fixture::new();
    let mut r = f.runner(Mode::Success);
    let error =
        cc_live::chain::cli(&mut r, &f.paths, &cc_live::args(&["fixture:s.json"])).unwrap_err();
    assert!(error.contains("Rust driver manifest is missing"));
    assert_eq!(
        r.inner
            .printed
            .iter()
            .filter(|s| s.starts_with("CIV6_CLOSE"))
            .count(),
        1,
        "only the pre-prepare close occurs"
    );
    assert_eq!(r.inner.driver_calls, 0);
    assert!(r.profile.is_none());
    assert_eq!(r.inner.releases, 1);
    assert_eq!(fs::read(&f.options).unwrap(), ORIGINAL);
    assert!(!f.paths.here.join("civ6.lock").exists());
}
#[test]
fn close_outcomes_restore_without_taskkill_and_drop_pin() {
    for outcome in ["absent", "closed", "killed"] {
        let f = Fixture::new();
        f.lock(&f.owner());
        let mut r = f.runner(Mode::Success);
        r.start_scenario(
            "START fixture s.json civ6=closed-before-prepare",
            Some("fixture-owner"),
            90.0,
        )
        .unwrap();
        assert!(fs::remove_file(f.paths.here.join("civ6.lock")).is_err());
        r.print(&format!("CIV6_CLOSE outcome={outcome}")).unwrap();
        assert!(r.profile.is_none());
        assert_eq!(fs::read(&f.options).unwrap(), ORIGINAL);
        fs::remove_file(f.paths.here.join("civ6.lock")).unwrap();
    }
}
#[test]
fn wrong_or_missing_pid_creation_and_token_refuse_apply() {
    let f = Fixture::new();
    for key in [
        "pid",
        "chain_pid",
        "pid_created_filetime",
        "lock_token",
        "live_chain",
    ] {
        for remove in [false, true] {
            let mut owner = f.owner();
            if remove {
                owner.as_object_mut().unwrap().remove(key);
            } else {
                owner[key] = if key == "lock_token" {
                    json!("")
                } else {
                    json!(0)
                };
            }
            f.lock(&owner);
            let mut r = f.runner(Mode::Success);
            assert!(
                r.start_scenario(
                    "START fixture s.json civ6=closed-before-prepare",
                    Some("fixture-owner"),
                    90.0
                )
                .is_err(),
                "{key}"
            );
            assert!(r.profile.is_none());
            assert_eq!(fs::read(&f.options).unwrap(), ORIGINAL);
            assert!(f.backups().is_empty());
        }
    }
}
#[test]
fn old_start_running_game_and_uncertain_absence_refuse_apply() {
    let f = Fixture::new();
    f.lock(&f.owner());
    let mut r = f.runner(Mode::Success);
    assert!(r
        .print("START fixture s.json civ6=closed-before-prepare")
        .is_err());
    assert!(r
        .start_scenario(
            "START fixture s.json civ6=closed-before-prepare",
            None,
            90.0
        )
        .is_err());
    assert!(r
        .start_scenario(
            "START fixture s.json civ6=closed-before-prepare",
            Some("wrong-chain-token"),
            90.0
        )
        .is_err());
    assert!(r
        .start_scenario(
            "START fixture s.json civ6=absent",
            Some("fixture-owner"),
            90.0
        )
        .is_err());
    r.inner.running = true;
    assert!(r
        .start_scenario(
            "START fixture s.json civ6=closed-before-prepare",
            Some("fixture-owner"),
            90.0
        )
        .is_err());
    r.inner.running = false;
    r.inner.absence_error = true;
    assert!(r
        .start_scenario(
            "START fixture s.json civ6=closed-before-prepare",
            Some("fixture-owner"),
            90.0
        )
        .is_err());
    assert!(r.profile.is_none());
    assert!(f.backups().is_empty());
    assert_eq!(fs::read(&f.options).unwrap(), ORIGINAL);
}
#[test]
fn backup_create_new_never_overwrites_existing_data() {
    let f = Fixture::new();
    f.lock(&f.owner());
    let backup = f.root.join("existing.backup");
    fs::write(&backup, b"other acquisition backup").unwrap();
    assert!(Profile::prepare(&f.paths.here.join("civ6.lock"), &f.options, &backup, None).is_err());
    assert_eq!(fs::read(&backup).unwrap(), b"other acquisition backup");
    assert_eq!(fs::read(&f.options).unwrap(), ORIGINAL);
}
#[test]
fn changed_lease_and_unclosed_game_keep_original_backup_and_pending_pin() {
    for changed_owner in [false, true] {
        let f = Fixture::new();
        f.lock(&f.owner());
        let mut r = f.runner(Mode::Success);
        r.start_scenario(
            "START fixture s.json civ6=closed-before-prepare",
            Some("fixture-owner"),
            90.0,
        )
        .unwrap();
        if changed_owner {
            let mut owner = f.owner();
            owner["lock_token"] = json!("peer");
            f.lock(&owner);
        } else {
            r.inner.running = true;
        }
        let error = r.before_lease_release(0.01).unwrap_err();
        assert!(!error.is_empty());
        assert!(r.profile.is_some());
        assert_eq!(r.inner.releases, 0);
        assert_eq!(fs::read(&f.options).unwrap(), MODIFIED);
        assert_eq!(fs::read(&f.backups()[0]).unwrap(), ORIGINAL);
        assert!(fs::remove_file(f.paths.here.join("civ6.lock")).is_err());
        // Recover the injected fixture; never mutate a real lease or game.
        f.lock(&f.owner());
        r.inner.running = false;
        r.restore_deadline = None;
        r.before_lease_release(1.0).unwrap();
    }
}
#[test]
fn native_file_lock_failure_retries_at_common_release_boundary() {
    let f = Fixture::new();
    f.lock(&f.owner());
    let mut r = f.runner(Mode::Success);
    r.start_scenario(
        "START fixture s.json civ6=closed-before-prepare",
        Some("fixture-owner"),
        90.0,
    )
    .unwrap();
    let blocker = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&f.options)
        .unwrap();
    r.print("CIV6_CLOSE outcome=closed").unwrap();
    assert!(r.profile.is_some());
    assert_eq!(fs::read(&f.options).unwrap(), MODIFIED);
    drop(blocker);
    r.before_lease_release(1.0).unwrap();
    assert!(r.profile.is_none());
    assert_eq!(fs::read(&f.options).unwrap(), ORIGINAL);
    assert_eq!(r.inner.releases, 1);
}
#[test]
fn permanent_native_file_lock_failure_is_finite_and_keeps_original() {
    let f = Fixture::new();
    f.lock(&f.owner());
    let mut r = f.runner(Mode::Success);
    r.start_scenario(
        "START fixture s.json civ6=closed-before-prepare",
        Some("fixture-owner"),
        90.0,
    )
    .unwrap();
    let blocker = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&f.options)
        .unwrap();
    let begin = Instant::now();
    assert!(r.before_lease_release(0.02).is_err());
    assert!(begin.elapsed() < Duration::from_secs(2));
    assert!(r.profile.is_some());
    assert_eq!(r.inner.releases, 0);
    assert_eq!(r.profile.as_ref().unwrap().original, ORIGINAL);
    assert_eq!(fs::read(&f.backups()[0]).unwrap(), ORIGINAL);
    drop(blocker);
    r.restore_deadline = None;
    r.before_lease_release(1.0).unwrap();
}
#[test]
fn retry_budget_does_not_restart_and_queries_receive_original_deadline() {
    let f = Fixture::new();
    f.lock(&f.owner());
    let mut r = f.runner(Mode::Success);
    r.start_scenario(
        "START fixture s.json civ6=closed-before-prepare",
        Some("fixture-owner"),
        90.0,
    )
    .unwrap();
    r.inner.running = true;
    assert!(r.before_lease_release(0.001).is_err());
    let deadline = r.restore_deadline.unwrap();
    assert!(r.before_lease_release(90.0).is_err());
    assert_eq!(r.restore_deadline, Some(deadline));
    r.inner.running = false;
    r.restore_deadline = None;
    r.before_lease_release(1.0).unwrap();
}
#[test]
fn native_identity_service_is_forwarded_and_original_sha_is_stable() {
    let f = Fixture::new();
    let mut r = f.runner(Mode::Success);
    r.prepare_process_queries().unwrap();
    assert!(r.inner.prepared);
    let identity = r
        .process_identity(std::process::id(), 1.0)
        .unwrap()
        .unwrap();
    assert_eq!(identity.created_filetime, Some(self_creation().unwrap()));
    assert_eq!(identity.image, "live_chain.exe");
    assert_eq!(
        original_hash(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
