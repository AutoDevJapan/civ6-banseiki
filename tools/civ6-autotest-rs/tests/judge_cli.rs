use civ6_autotest_rs::judge::LOG_NAMES;
use std::{fs, process::Command};

#[test]
fn snapshot_then_judge_new_log_bytes_and_exit_codes() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    fs::create_dir(&logs).unwrap();
    for name in LOG_NAMES {
        fs::write(logs.join(name), b"old errors\n").unwrap();
    }
    let baseline = dir.path().join("baseline.json");
    let bin = env!("CARGO_BIN_EXE_civ6-judge");
    let snap = Command::new(bin)
        .args([
            "--logs",
            logs.to_str().unwrap(),
            "--snapshot",
            baseline.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        snap.status.success(),
        "{}",
        String::from_utf8_lossy(&snap.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&baseline).unwrap()).unwrap();
    assert_eq!(value["files"]["Lua.log"]["offset"], 11);
    for name in LOG_NAMES {
        use std::io::Write;
        fs::OpenOptions::new()
            .append(true)
            .open(logs.join(name))
            .unwrap()
            .write_all(b"new line\n")
            .unwrap();
    }
    let ok = Command::new(bin)
        .args([
            "--logs",
            logs.to_str().unwrap(),
            "--baseline",
            baseline.to_str().unwrap(),
            "--profile",
            "generic",
        ])
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    assert!(String::from_utf8_lossy(&ok.stdout).contains("Generic: JAC Runtime Error"));
    use std::io::Write;
    fs::OpenOptions::new()
        .append(true)
        .open(logs.join("Lua.log"))
        .unwrap()
        .write_all(b"JAC: [JAC_POC] api_error\n")
        .unwrap();
    let failed = Command::new(bin)
        .args([
            "--logs",
            logs.to_str().unwrap(),
            "--baseline",
            baseline.to_str().unwrap(),
            "--profile",
            "generic",
        ])
        .output()
        .unwrap();
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stdout).contains("FAIL"));
    let invalid = Command::new(bin)
        .args(["--logs", logs.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
}
