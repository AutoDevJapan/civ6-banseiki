use cc_live::{deployment, path, Call, Output, Paths, Runner};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static SERIAL: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
    live: PathBuf,
    source: PathBuf,
    cache: PathBuf,
    link: PathBuf,
    p: Paths,
}

fn junction(link: &std::path::Path, target: &std::path::Path) {
    #[cfg(windows)]
    {
        use std::{
            os::windows::process::CommandExt,
            process::{Command, Stdio},
        };
        assert!(Command::new("cmd")
            .args(["/c", "mklink", "/J", &path(link), &path(target)])
            .creation_flags(cc_live::process::CREATE_NO_WINDOW)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
}

fn unlink(link: &std::path::Path) {
    #[cfg(windows)]
    fs::remove_dir(link).unwrap();
    #[cfg(unix)]
    fs::remove_file(link).unwrap();
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "cc-dlc-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::SeqCst)
        ));
        let p = Paths {
            here: root.join("here"),
            wt: root.join("wt"),
            jobs: root.join("jobs"),
            temp: root.join("temp"),
            profile: root.join("user"),
            python: String::new(),
        };
        let live = p.live();
        let source = live.join("poc").join(deployment::MOD);
        let cache = live.join("tools/target/local-mods").join(deployment::MOD);
        let link = p
            .profile
            .join("Documents/My Games/Sid Meier's Civilization VI/Mods")
            .join(deployment::MOD);
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        fs::create_dir_all(live.join("tools/civ6-data-rs/src/bin")).unwrap();
        fs::create_dir_all(live.join("tools/target/release")).unwrap();
        fs::write(
            source.join(format!("{}.modinfo", deployment::MOD)),
            "source one",
        )
        .unwrap();
        fs::write(live.join("tools/civ6-data-rs/Cargo.toml"), "[package]").unwrap();
        fs::write(
            live.join(format!(
                "tools/civ6-data-rs/src/bin/{}.rs",
                deployment::TOOL
            )),
            "fn main() {}\n",
        )
        .unwrap();
        junction(&link, &source);
        Self {
            root,
            live,
            source,
            cache,
            link,
            p,
        }
    }
    fn fake(&self) -> Fake {
        Fake {
            live: self.live.clone(),
            source: self.source.clone(),
            cache: self.cache.clone(),
            link: self.link.clone(),
            ..Fake::default()
        }
    }
    fn target_is(&self, expected: &std::path::Path) -> bool {
        self.link.canonicalize().unwrap() == expected.canonicalize().unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Default)]
struct Fake {
    live: PathBuf,
    source: PathBuf,
    cache: PathBuf,
    link: PathBuf,
    calls: Vec<Call>,
    wrong_artifact: bool,
    prepare_failure: bool,
    missing_output: bool,
    unconfirmed_link: bool,
}

impl Runner for Fake {
    fn run(&mut self, call: Call) -> Result<Output, String> {
        assert_eq!(call.creationflags, cc_live::process::CREATE_NO_WINDOW);
        assert!(call.kill_tree);
        self.calls.push(call.clone());
        if call.args.contains(&"build".into()) {
            let exe = self
                .live
                .join("tools/target/release")
                .join(if cfg!(windows) {
                    format!("{}.exe", deployment::TOOL)
                } else {
                    deployment::TOOL.into()
                });
            fs::write(&exe, "branch helper").unwrap();
            let manifest = if self.wrong_artifact {
                self.live
                    .join("tools/civ6-data-rs/src/bin")
                    .join(format!("{}.rs", deployment::TOOL))
            } else {
                self.live.join("tools/civ6-data-rs/Cargo.toml")
            };
            return Ok(Output { stdout: json!({"reason":"compiler-artifact", "manifest_path":path(manifest), "target":{"name":deployment::TOOL,"kind":["bin"]}, "executable":path(exe)}).to_string(), ..Output::default() });
        }
        if call.args.iter().any(|a| a == "--refresh") {
            if self.prepare_failure {
                return Ok(Output {
                    code: 1,
                    stderr: "DLC XML invalid".into(),
                    ..Output::default()
                });
            }
            if !self.missing_output {
                fs::create_dir_all(&self.cache).unwrap();
                fs::copy(
                    self.source.join(format!("{}.modinfo", deployment::MOD)),
                    self.cache.join(format!("{}.modinfo", deployment::MOD)),
                )
                .unwrap();
            }
            return Ok(Output::default());
        }
        assert_eq!(call.args[0], "pwsh");
        let script = call.args.last().unwrap();
        assert!(script.contains("LinkType -ne 'Junction'"));
        assert!(script.contains("junction changed during preparation"));
        assert!(!script.contains("-Recurse") && !script.contains("-Force"));
        if !self.unconfirmed_link {
            let desired = if script.contains(&format!(
                "-Target '{}'",
                path(&self.cache).replace('\'', "''")
            )) {
                &self.cache
            } else {
                &self.source
            };
            unlink(&self.link);
            junction(&self.link, desired);
        }
        Ok(Output::default())
    }
    fn now(&mut self) -> f64 {
        0.0
    }
    fn sleep(&mut self, _: f64) -> Result<(), String> {
        Err("unexpected wait".into())
    }
}

#[test]
fn deployment_refreshes_source_changes_even_when_junction_already_uses_the_cache() {
    let f = Fixture::new();
    let mut r = f.fake();
    deployment::prepare(&mut r, &f.p, &f.live).unwrap();
    assert!(f.target_is(&f.cache));
    fs::write(
        f.source.join(format!("{}.modinfo", deployment::MOD)),
        "source two",
    )
    .unwrap();
    deployment::prepare(&mut r, &f.p, &f.live).unwrap();
    assert_eq!(
        fs::read_to_string(f.cache.join(format!("{}.modinfo", deployment::MOD))).unwrap(),
        "source two"
    );
    assert_eq!(r.calls.iter().filter(|c| c.args[0] == "pwsh").count(), 1);
    assert_eq!(
        r.calls
            .iter()
            .filter(|c| c.args.iter().any(|a| a == "--refresh"))
            .count(),
        2
    );
}

#[test]
fn older_branch_without_generator_restores_its_source_mod() {
    let f = Fixture::new();
    let mut r = f.fake();
    deployment::prepare(&mut r, &f.p, &f.live).unwrap();
    fs::remove_file(f.live.join(format!(
        "tools/civ6-data-rs/src/bin/{}.rs",
        deployment::TOOL
    )))
    .unwrap();
    r.calls.clear();
    deployment::prepare(&mut r, &f.p, &f.live).unwrap();
    assert!(f.target_is(&f.source));
    assert_eq!(r.calls.len(), 1);
    assert_eq!(r.calls[0].args[0], "pwsh");
}

#[test]
fn failed_preparation_or_missing_generated_manifest_never_changes_the_original_junction() {
    for missing in [false, true] {
        let f = Fixture::new();
        let mut r = f.fake();
        r.prepare_failure = !missing;
        r.missing_output = missing;
        assert!(deployment::prepare(&mut r, &f.p, &f.live).is_err());
        assert!(f.target_is(&f.source));
        assert!(!r.calls.iter().any(|c| c.args[0] == "pwsh"));
    }
}

#[test]
fn wrong_package_artifact_is_rejected_before_preparation_or_junction_changes() {
    let f = Fixture::new();
    let mut r = f.fake();
    r.wrong_artifact = true;
    assert!(deployment::prepare(&mut r, &f.p, &f.live)
        .unwrap_err()
        .contains("Cargo did not report"));
    assert_eq!(r.calls.len(), 1);
    assert!(f.target_is(&f.source));
}

#[test]
fn a_junction_owned_by_another_worktree_is_left_alone() {
    let f = Fixture::new();
    let foreign = f.root.join("other-worktree");
    fs::create_dir_all(&foreign).unwrap();
    fs::write(foreign.join("owner.txt"), "owner data").unwrap();
    unlink(&f.link);
    junction(&f.link, &foreign);
    let mut r = f.fake();
    assert!(deployment::prepare(&mut r, &f.p, &f.live)
        .unwrap_err()
        .contains("outside the shared"));
    assert!(r.calls.is_empty());
    assert!(f.target_is(&foreign));
    assert_eq!(
        fs::read_to_string(foreign.join("owner.txt")).unwrap(),
        "owner data"
    );
}

#[test]
fn successful_shell_exit_is_not_accepted_without_the_actual_junction_change() {
    let f = Fixture::new();
    let mut r = f.fake();
    r.unconfirmed_link = true;
    assert!(deployment::prepare(&mut r, &f.p, &f.live)
        .unwrap_err()
        .contains("was not confirmed"));
    assert!(f.target_is(&f.source));
}

#[cfg(windows)]
#[test]
fn actual_pwsh_switches_and_restores_the_fixture_junction() {
    struct Shell(Fake);
    impl Runner for Shell {
        fn run(&mut self, call: Call) -> Result<Output, String> {
            if call.args[0] == "pwsh" {
                cc_live::process::RealRunner.run(call)
            } else {
                self.0.run(call)
            }
        }
        fn now(&mut self) -> f64 {
            0.0
        }
        fn sleep(&mut self, _: f64) -> Result<(), String> {
            Err("unexpected wait".into())
        }
    }
    let f = Fixture::new();
    let mut r = Shell(f.fake());
    deployment::prepare(&mut r, &f.p, &f.live).unwrap();
    assert!(f.target_is(&f.cache));
    deployment::restore(&mut r, &f.p, &f.live).unwrap();
    assert!(f.target_is(&f.source));
}
