use civ6_autotest_rs::{
    capture,
    judge::LOG_NAMES,
    repo_root,
    runner::{Backend, Runner},
    time::Clock,
    Result,
};
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct FakeClock(Mutex<f64>);
impl Clock for FakeClock {
    fn now(&self) -> f64 {
        *self.0.lock().unwrap()
    }
    fn sleep(&self, s: f64) {
        *self.0.lock().unwrap() += s;
    }
}
struct BundleGame {
    logs: PathBuf,
    saves: PathBuf,
    turn: i64,
    target: i64,
    active: bool,
    commands: Arc<Mutex<Vec<String>>>,
}
impl BundleGame {
    fn append(&self, file: &str, line: &str) {
        let mut f = OpenOptions::new()
            .append(true)
            .open(self.logs.join(file))
            .unwrap();
        writeln!(f, "{line}").unwrap();
    }
}
impl Backend for BundleGame {
    fn require_single_process(&mut self) -> Result<()> {
        Ok(())
    }
    fn single_process(&mut self, _settle: f64) -> Result<Option<(u32, f64)>> {
        Ok(Some((7, 0.0)))
    }
    fn list_states(&mut self, _timeout: f64) -> Result<Vec<String>> {
        Ok(vec![
            "FrontEnd".into(),
            "InGame".into(),
            "LoadScreen".into(),
        ])
    }
    fn run_lua(&mut self, _state: &str, code: &str, _timeout: f64) -> Result<String> {
        self.commands.lock().unwrap().push(code.into());
        if code.contains("local enableIDs") {
            self.append("Modding.log","dfa0717e-4d0b-4370-b693-27fc6758571b\nApplying Component - JAC_PoC_Bundle0_Gameplay (UpdateDatabase)\nApplying Component - JAC_PoC_Bundle0_Scripts (AddGameplayScripts)");
            return Ok(String::new());
        }
        if code.contains("Network.HostGame") {
            self.append("Lua.log","JAC_PoC_Bundle0: [JAC_POC] loaded version=1\nJAC_PoC_Bundle0: [JAC_POC] no saved state (new game)\nJAC_PoC_Bundle0: [JAC_POC] turn=1 firstTurn=1 turnsSeen=1\nLoadScreen: OnLoadGameViewStateDone");
        }
        if code.contains("Network.LoadGame") {
            self.append("Lua.log",&format!("JAC_PoC_Bundle0: [JAC_POC] loaded version=1\nJAC_PoC_Bundle0: [JAC_POC] restored firstTurn=1 lastTurn={} turnsSeen={}\nLoadScreen: OnLoadGameViewStateDone",self.turn,self.turn));
        }
        if code.contains("AutoplayManager.SetTurns(") {
            let turns = regex::Regex::new(r"SetTurns\((\d+)\)")
                .unwrap()
                .captures(code)
                .unwrap()[1]
                .parse::<i64>()
                .unwrap();
            self.target = self.turn + turns;
            self.active = true;
            return Ok(format!("{}|0", self.turn));
        }
        if code.contains("tostring(AutoplayManager.IsActive())") {
            if self.active {
                self.turn += 1;
                self.active = self.turn < self.target;
                self.append(
                    "Lua.log",
                    &format!(
                        "JAC_PoC_Bundle0: [JAC_POC] turn={} firstTurn=1 turnsSeen={}",
                        self.turn, self.turn
                    ),
                );
            }
            return Ok(format!(
                "{}|{}|{}|{}|0",
                self.turn,
                self.active,
                !self.active,
                self.target - self.turn
            ));
        }
        if code.contains("IsAlive()") {
            return Ok(format!("{}|true", self.turn));
        }
        if code.contains("Network.SaveGame") {
            let re = regex::Regex::new(r#"Name=("[^"]+")"#).unwrap();
            let name: String = serde_json::from_str(&re.captures(code).unwrap()[1]).unwrap();
            fs::write(
                self.saves.join(format!("{name}.Civ6Save")),
                b"Civ VI save fixture",
            )?;
        }
        Ok("true".into())
    }
    fn capture(&mut self, path: &Path, _crop: Option<[f64; 4]>, _deadline: f64) -> Result<()> {
        capture::save_png(path, 1, 1, &[8, 16, 32, 255])
    }
}

#[test]
fn bundle0_runs_all_ten_actions_and_preserves_artifact_contract() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let logs = root.join("logs");
    fs::create_dir(&logs).unwrap();
    for name in LOG_NAMES {
        fs::write(logs.join(name), b"old unrelated log\n").unwrap();
    }
    let commands = Arc::new(Mutex::new(vec![]));
    let backend = BundleGame {
        logs: logs.clone(),
        saves: root.to_path_buf(),
        turn: 1,
        target: 1,
        active: false,
        commands: commands.clone(),
    };
    let scenario: Value = serde_json::from_slice(
        &fs::read(repo_root().join("tools/civ6-autotest/scenarios/bundle0-load-save.json"))
            .unwrap(),
    )
    .unwrap();
    let output = root.join("nested/run");
    let mut runner = Runner::with_backend(
        scenario,
        output.clone(),
        logs,
        root.to_path_buf(),
        4318,
        Box::new(backend),
        Arc::new(FakeClock::default()),
    );
    runner.auto_restart = false;
    assert_eq!(runner.run().unwrap(), 0);
    let result: Value =
        serde_json::from_slice(&fs::read(output.join("result.json")).unwrap()).unwrap();
    assert_eq!(result["scenario"], "JAC_PoC_Bundle0 load/save");
    assert_eq!(result["passed"], true);
    assert_eq!(result["completed_steps"], 10);
    assert!(result["failure"].is_null());
    assert!(result["test_mode"].is_null());
    let keys: std::collections::BTreeSet<_> = result
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "scenario",
            "passed",
            "test_mode",
            "completed_steps",
            "failure",
            "checks",
            "metrics"
        ]
        .into_iter()
        .collect()
    );
    assert!(!result["checks"].as_array().unwrap().is_empty());
    assert!(result["checks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["passed"] == true));
    assert_eq!(result["metrics"]["autoplay"], json!([]));
    assert_eq!(result["metrics"]["groups"], json!({}));
    assert_eq!(result["metrics"]["save_load"]["samples"], json!([]));
    for file in [
        "baseline.json",
        "steps.jsonl",
        "Lua.log",
        "Database.log",
        "Modding.log",
        "bundle0-before-save.png",
        "bundle0-after-load.png",
    ] {
        assert!(output.join(file).is_file(), "missing {file}");
    }
    let steps: Vec<Value> = fs::read_to_string(output.join("steps.jsonl"))
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(steps.iter().filter(|s| s["status"] == "START").count(), 10);
    assert!(steps.iter().all(|s| s["status"] != "FAIL"));
    assert!(!fs::read_to_string(output.join("Lua.log"))
        .unwrap()
        .contains("old unrelated"));
    let commands = commands.lock().unwrap();
    assert_eq!(
        commands
            .iter()
            .filter(|c| c.contains("Network.SaveGame"))
            .count(),
        1
    );
    assert_eq!(
        commands
            .iter()
            .filter(|c| c.contains("Network.LoadGame"))
            .count(),
        1
    );
    assert_eq!(
        commands
            .iter()
            .filter(|c| c.contains("Network.HostGame"))
            .count(),
        1
    );
    assert_eq!(
        commands
            .iter()
            .filter(|c| c.contains("SetActive(true)"))
            .count(),
        2
    );
}

#[test]
fn check_cli_resolves_fixtures_from_unrelated_working_directory() {
    let temp = tempfile::tempdir().unwrap();
    let scenario = repo_root().join("tools/civ6-autotest/scenarios/bundle0-load-save.json");
    for args in [vec!["--check"], vec!["--check", "--test-mode"]] {
        let result = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
            .arg(&scenario)
            .args(args)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap().trim(),
            "VALID: JAC_PoC_Bundle0 load/save (10 steps)"
        );
    }
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    for port in [
        "0",
        "65536",
        "-1",
        "18446744073709551616",
        "-18446744073709551616",
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
            .arg(&scenario)
            .args(["--check", "--port", port])
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "port={port}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).starts_with("VALID:"));
        let output = temp.path().join(format!("port-{port}"));
        let live = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
            .arg(&scenario)
            .args(["--port", port, "--output"])
            .arg(&output)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert_eq!(live.status.code(), Some(2));
        assert!(!output.exists());
    }
    for port in ["+4318", "0004318", "4_318"] {
        let result = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
            .arg(&scenario)
            .args(["--check", "--port", port])
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(result.status.success(), "{port}");
    }
    // argparse accepts the signed underscore form only as an attached value.
    let attached = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
        .arg(&scenario)
        .args(["--check", "--port=-4_318"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(attached.status.success());
    for port in ["-4_318", "-1e3", "-1."] {
        let result = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
            .arg(&scenario)
            .args(["--check", "--port", port])
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2), "{port}");
        assert!(result.stdout.is_empty());
    }
    for port in ["1.5", "unused", "_4318", "4__318"] {
        let result = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
            .arg(&scenario)
            .args(["--check", "--port", port])
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&result.stderr).contains("invalid integer value"));
    }
}
