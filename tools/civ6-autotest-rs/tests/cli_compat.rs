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

#[test]
fn compat_check_cli_reads_negative_number_paths_without_game_actions() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = repo_root().join("tools/civ6-autotest/scenarios/bundle0-load-save.json");
    let bytes = fs::read(&fixture).unwrap();
    fs::write(temp.path().join("-1"), &bytes).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
        .args([
            "-1", "--check", "--ch", "--output", "-2.5", "--out=", "--logs", "-3", "--saves", "-4",
            "--mods", "-5",
        ])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "VALID: JAC_PoC_Bundle0 load/save (10 steps)"
    );
    assert!(output.stderr.is_empty());
    let output = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
        .args(["-1", "--check", "--logs", "--unknown", "--l", "logs"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    assert_eq!(fs::read(temp.path().join("-1")).unwrap(), bytes);
}

#[test]
fn compat_tuner_negative_number_strings_reach_invalid_port_guard_without_connecting() {
    for args in [
        vec![
            "lua", "--state", "-1", "--s", "InGame", "--code", "return 1", "--port", "0",
        ],
        vec![
            "lua", "--state", "-1", "--code", "-2.5", "--c", "return 1", "--port", "0",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_civ6-tuner"))
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap().trim(),
            "Invalid port or timeout"
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn compat_tuner_subprocess_matches_argparse_negative_lexing_without_connecting() {
    for (state, attached, expected) in [
        ("-.5", false, 1),
        ("-1e3", false, 2),
        ("-1.", false, 2),
        ("-1e3", true, 1),
        ("-1.", true, 1),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_civ6-tuner"));
        command.arg("lua");
        if attached {
            command.arg(format!("--state={state}"));
        } else {
            command.args(["--state", state]);
        }
        let output = command
            .args([
                "--s", "InGame", "--code", "-.5", "--code", "return 1", "--port", "0",
            ])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{state}/{attached}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if expected == 1 {
            assert_eq!(
                String::from_utf8(output.stderr).unwrap().trim(),
                "Invalid port or timeout"
            );
        }
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn compat_check_subprocess_matches_negative_positional_and_path_lexing() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = repo_root().join("tools/civ6-autotest/scenarios/bundle0-load-save.json");
    let bytes = fs::read(fixture).unwrap();
    for name in ["-.5", "-1e3", "-1."] {
        fs::write(temp.path().join(name), &bytes).unwrap();
    }
    for (name, separator, expected) in [
        ("-.5", false, 0),
        ("-1e3", false, 2),
        ("-1.", false, 2),
        ("-1e3", true, 0),
        ("-1.", true, 0),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"));
        command.args(["--check", "--output", "-.5", "--out=-1e3"]);
        if separator {
            command.arg("--");
        }
        let output = command.arg(name).current_dir(temp.path()).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{name}/{separator}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if expected == 0 {
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                "VALID: JAC_PoC_Bundle0 load/save (10 steps)"
            );
            assert!(output.stderr.is_empty());
        } else {
            assert!(output.stdout.is_empty());
        }
    }
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 3);
}

#[test]
fn compat_tuner_subprocess_accepts_bare_lua_comment_and_prioritizes_help() {
    let bin = env!("CARGO_BIN_EXE_civ6-tuner");
    for code in ["-- comment\nreturn 1", "- path with spaces"] {
        let output = Command::new(bin)
            .args(["lua", "--code", code, "--port", "0"])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stderr).unwrap().trim(),
            "Invalid port or timeout"
        );
        assert!(output.stdout.is_empty());
    }
    let help = Command::new(bin)
        .args(["lua", "--unknown", "--help"])
        .output()
        .unwrap();
    assert_eq!(help.status.code(), Some(0));
    assert!(help.stderr.is_empty());
    assert!(String::from_utf8(help.stdout).unwrap().contains("--code"));
    for args in [
        vec!["lua", "--unknown", "--state", "--help"],
        vec!["lua", "--state", "--code=two words", "--port", "0"],
        vec!["lua", "--code", "--unknown", "--port", "0"],
        vec!["lua", "--code", "--comment\nreturn1", "--port", "0"],
    ] {
        let output = Command::new(bin).args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn compat_check_subprocess_accepts_space_containing_option_like_paths() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = repo_root().join("tools/civ6-autotest/scenarios/bundle0-load-save.json");
    let path = "- path with spaces";
    let bytes = fs::read(fixture).unwrap();
    fs::write(temp.path().join(path), &bytes).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_civ6-autotest"))
        .args([
            path, "--check", "--output", path, "--logs", path, "--saves", path, "--mods", path,
        ])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "VALID: JAC_PoC_Bundle0 load/save (10 steps)"
    );
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    assert_eq!(fs::read(temp.path().join(path)).unwrap(), bytes);
}
