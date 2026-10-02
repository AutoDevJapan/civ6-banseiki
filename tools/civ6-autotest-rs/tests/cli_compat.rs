use civ6_autotest_rs::repo_root;
use clap::Parser;
use std::{ffi::OsString, fs, path::PathBuf, process::Command};

#[path = "../src/bin/support/mod.rs"]
mod support;

#[derive(Parser)]
struct PathArgs {
    #[arg(value_parser = support::path_value_parser())]
    path: PathBuf,
}

#[test]
fn compat_path_helper_changes_only_empty_paths() {
    for (input, expected) in [
        ("", "."),
        (".", "."),
        (" ", " "),
        ("../logs", "../logs"),
        ("日本語の保存先", "日本語の保存先"),
    ] {
        assert_eq!(
            PathArgs::try_parse_from(["path", input]).unwrap().path,
            PathBuf::from(expected)
        );
    }
}

#[cfg(windows)]
#[test]
fn compat_path_helper_preserves_native_non_unicode_paths() {
    use std::os::windows::ffi::OsStringExt;
    let path = OsString::from_wide(&[b'x' as u16, 0xd800]);
    assert_eq!(
        PathArgs::try_parse_from([OsString::from("path"), path.clone()])
            .unwrap()
            .path
            .into_os_string(),
        path
    );
}

#[cfg(unix)]
#[test]
fn compat_path_helper_preserves_native_non_unicode_paths() {
    use std::os::unix::ffi::OsStringExt;
    let path = OsString::from_vec(vec![b'x', 0xff]);
    assert_eq!(
        PathArgs::try_parse_from([OsString::from("path"), path.clone()])
            .unwrap()
            .path
            .into_os_string(),
        path
    );
}

#[test]
fn compat_check_cli_accepts_repeats_prefixes_empty_paths_and_large_ports() {
    let temp = tempfile::tempdir().unwrap();
    let scenario = repo_root().join("tools/civ6-autotest/scenarios/bundle0-load-save.json");
    for (first, last) in [
        ("4318", "18446744073709551616"),
        ("-18446744073709551616", "4_318"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
            .arg(&scenario)
            .args([
                "--check",
                "--ch",
                "--test-mode",
                "--t",
                "--no-auto-restart",
                "--no",
                "--owner-start",
                "--ow",
                "--port",
                first,
                "--p",
                last,
                "--output",
                "unused-output",
                "--out=",
                "--logs",
                "unused-logs",
                "--l",
                "",
                "--saves",
                "unused-saves",
                "--s",
                "",
                "--mods",
                "unused-mods",
                "--m",
                "",
            ])
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{first}/{last}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            "VALID: JAC_PoC_Bundle0 load/save (10 steps)"
        );
        assert!(output.stderr.is_empty());
    }
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn compat_check_cli_rejects_ambiguous_unknown_and_invalid_occurrences() {
    let temp = tempfile::tempdir().unwrap();
    let scenario = repo_root().join("tools/civ6-autotest/scenarios/bundle0-load-save.json");
    for tail in [
        vec!["--o", "out"],
        vec!["--port", "", "--p", "4318"],
        vec!["--port", "bad", "--p", "4318"],
        vec!["--check=true"],
        vec!["--test_mode"],
        vec!["--output-extra", "out"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
            .arg(&scenario)
            .arg("--check")
            .args(&tail)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{tail:?}");
        assert!(output.stdout.is_empty());
    }
    let output = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
        .args(["", "--ch"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("Scenario error:"));
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn compat_tuner_missing_final_code_returns_before_connecting() {
    let output = Command::new(env!("CARGO_BIN_EXE_civ6-tuner"))
        .args([
            "lua", "--code", "return 1", "--c=", "--port", "4318", "--p", "65536",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap().trim(),
        "error: lua requires --code"
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn compat_click_conflicting_operations_still_fail_before_input() {
    for args in [
        vec!["--key", "1", "--k", "27", "--close", "--cl"],
        vec!["0.5", "0.5", "--key", "1", "--k", "27"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_civ6-click"))
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap().trim(),
            "Supply fx fy, --key VK, or --close"
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn compat_restart_last_seconds_still_enforces_guard_before_window_queries() {
    let output = Command::new(env!("CARGO_BIN_EXE_civ6-restart"))
        .args(["wait", "--seconds", "120", "--s", "0"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap().trim(),
        "seconds must be finite and positive"
    );
    assert!(output.stdout.is_empty());
}
