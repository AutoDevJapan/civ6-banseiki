use civ6_autotest_rs::{judge, runner::Runner, scenario, tuner::PortArgument, Result};
use clap::Parser;
use std::{fs, path::PathBuf};

mod support;

#[derive(Parser)]
#[command(
    about = "Run a Civ VI PoC scenario with automatic mod freshness checks",
    infer_long_args = true,
    args_override_self = true
)]
struct Args {
    #[arg(value_parser = support::path_value_parser())]
    scenario: PathBuf,
    #[arg(long)]
    check: bool,
    #[arg(long)]
    test_mode: bool,
    #[arg(long, value_parser = support::path_value_parser())]
    output: Option<PathBuf>,
    #[arg(long, value_parser = support::path_value_parser())]
    logs: Option<PathBuf>,
    #[arg(long, value_parser = support::path_value_parser())]
    saves: Option<PathBuf>,
    #[arg(long, default_value = "4318")]
    port: PortArgument,
    #[arg(long, value_parser = support::path_value_parser())]
    mods: Option<PathBuf>,
    #[arg(long)]
    no_auto_restart: bool,
    #[arg(long)]
    owner_start: bool,
}
fn run(args: Args) -> Result<i32> {
    let bytes = fs::read(&args.scenario)?;
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
    let mut data = serde_json::from_slice(bytes)?;
    scenario::validate_scenario(&data)?;
    if args.test_mode {
        data = scenario::fast_forward_scenario(&data);
        scenario::validate_scenario(&data)?;
    }
    if args.check {
        println!(
            "VALID: {} ({} steps)",
            data["name"].as_str().unwrap(),
            data["steps"].as_array().unwrap().len()
        );
        return Ok(0);
    }
    let port = args.port.socket_port()?;
    civ6_autotest_rs::time::install_interrupt_handler()?;
    let output = args.output.unwrap_or_else(|| {
        civ6_autotest_rs::repo_root()
            .join("tools/civ6-autotest/artifacts")
            .join(format!(
                "{}-{}",
                chrono::Local::now().format("%Y%m%d-%H%M%S-%6f"),
                args.scenario
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
            ))
    });
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();
    let saves = args.saves.unwrap_or_else(|| {
        home.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single")
    });
    let mut runner = Runner::new(
        data,
        output,
        args.logs.map(Ok).unwrap_or_else(judge::default_log_dir)?,
        saves,
        port,
    );
    runner.auto_restart = !args.no_auto_restart;
    runner.owner_start = args.owner_start;
    runner.test_mode = args.test_mode;
    if let Some(mods) = args.mods {
        runner.mods_dir = mods;
    }
    runner.run()
}
fn main() {
    let code = match run(Args::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Scenario error: {e}");
            if e.kind == civ6_autotest_rs::ErrorKind::Interrupted {
                130
            } else {
                2
            }
        }
    };
    std::process::exit(code);
}

#[cfg(test)]
#[path = "support/parser_tests.rs"]
mod parser_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compat_scenario_negative_number_paths_preserve_values_and_last_occurrences() {
        let args = Args::try_parse_from([
            "civ6-autotest",
            "-1",
            "--output",
            "-2.5",
            "--logs",
            "-3",
            "--saves",
            "-4",
            "--mods",
            "-5",
            "--out",
            "out",
        ])
        .unwrap();
        assert_eq!(args.scenario, PathBuf::from("-1"));
        assert_eq!(args.output, Some(PathBuf::from("out")));
        assert_eq!(args.logs, Some(PathBuf::from("-3")));
        assert_eq!(args.saves, Some(PathBuf::from("-4")));
        assert_eq!(args.mods, Some(PathBuf::from("-5")));
        for value in ["--unknown", "-not-a-number", "-1file"] {
            parser_tests::assert_parse_error::<Args>(&[
                "civ6-autotest",
                "scenario.json",
                "--logs",
                value,
                "--l",
                "logs",
            ]);
        }
    }

    #[test]
    fn compat_scenario_prefixes_match_argparse() {
        parser_tests::assert_long_prefixes::<Args>(
            &["civ6-autotest", "scenario.json"],
            &[
                ("--check", &[]),
                ("--test-mode", &[]),
                ("--output", &["out"]),
                ("--logs", &["logs"]),
                ("--saves", &["saves"]),
                ("--port", &["4318"]),
                ("--mods", &["mods"]),
                ("--no-auto-restart", &[]),
                ("--owner-start", &[]),
            ],
        );
    }

    #[test]
    fn compat_scenario_repeats_keep_last_paths_ports_and_true_flags() {
        let args = Args::try_parse_from([
            "civ6-autotest",
            "",
            "--check",
            "--ch",
            "--test-mode",
            "--t",
            "--output",
            "old",
            "--out=",
            "--logs",
            "old",
            "--l",
            "",
            "--saves",
            "old",
            "--s",
            "",
            "--mods",
            "old",
            "--m",
            "",
            "--no-auto-restart",
            "--no",
            "--owner-start",
            "--ow",
            "--port",
            "65536",
            "--p",
            "4_318",
        ])
        .unwrap();
        assert_eq!(args.scenario, PathBuf::from("."));
        for path in [args.output, args.logs, args.saves, args.mods] {
            assert_eq!(path, Some(PathBuf::from(".")));
        }
        assert!(args.check && args.test_mode && args.no_auto_restart && args.owner_start);
        assert_eq!(args.port.socket_port().unwrap(), 4318);
        let args = Args::try_parse_from([
            "civ6-autotest",
            "scenario.json",
            "--port",
            "4318",
            "--p",
            "18446744073709551616",
        ])
        .unwrap();
        assert!(args.port.socket_port().is_err());
    }

    #[test]
    fn compat_scenario_invalid_occurrences_are_not_hidden_by_repeats() {
        for tail in [
            vec!["--port", "", "--port", "4318"],
            vec!["--port", "1.5", "--port", "4318"],
            vec!["--port", "4__318", "--port", "4318"],
            vec!["--check=true"],
            vec!["--test_mode"],
            vec!["--logs-extra", "logs"],
            vec!["another.json"],
        ] {
            let mut args = vec!["civ6-autotest", "scenario.json"];
            args.extend(tail);
            parser_tests::assert_parse_error::<Args>(&args);
        }
    }
}

civ6_cli_compat_rs::parser!(Args);
