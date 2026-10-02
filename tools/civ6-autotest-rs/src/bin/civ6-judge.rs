use civ6_autotest_rs::judge::{default_log_dir, judge, print_table, read_since, snapshot_logs};
use clap::Parser;
use std::{fs, path::PathBuf, process::ExitCode};

mod support;

#[derive(Parser)]
#[command(
    about = "Judge Civ VI PoC logs from a byte-offset baseline",
    infer_long_args = true,
    args_override_self = true,
    allow_negative_numbers = true
)]
struct Args {
    #[arg(long, value_parser = support::path_value_parser())]
    logs: Option<PathBuf>,
    #[arg(long, value_parser = support::path_value_parser())]
    baseline: Option<PathBuf>,
    #[arg(long, value_parser = support::path_value_parser(), help = "Write baseline before a manual run, then exit")]
    snapshot: Option<PathBuf>,
    #[arg(long, default_value="bundle0", value_parser=["bundle0","generic","growth","japan_map"])]
    profile: String,
    #[arg(long)]
    expected_units: Option<usize>,
}
fn run(args: Args) -> civ6_autotest_rs::Result<ExitCode> {
    let log_dir = match args.logs {
        Some(p) => p,
        None => default_log_dir()?,
    };
    if let Some(path) = args.snapshot {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &path,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&snapshot_logs(&log_dir)?)?
            ),
        )?;
        println!("{}", path.display());
        return Ok(ExitCode::SUCCESS);
    }
    let path = args
        .baseline
        .ok_or_else(|| civ6_autotest_rs::Error::input("--baseline is required when judging"))?;
    let baseline = serde_json::from_slice(&fs::read(path)?)?;
    let checks = judge(
        &read_since(&log_dir, &baseline)?,
        &args.profile,
        args.expected_units,
        false,
    );
    print_table(&checks);
    Ok(if checks.iter().all(|c| c.passed) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}
fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Cannot judge logs: {e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
#[path = "support/parser_tests.rs"]
mod parser_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compat_judge_negative_paths_do_not_loosen_profile_choices() {
        let args = Args::try_parse_from([
            "civ6-judge",
            "--logs",
            "-1",
            "--baseline",
            "-2.5",
            "--snapshot",
            "-3",
            "--l",
            "logs",
        ])
        .unwrap();
        assert_eq!(args.logs, Some(PathBuf::from("logs")));
        assert_eq!(args.baseline, Some(PathBuf::from("-2.5")));
        assert_eq!(args.snapshot, Some(PathBuf::from("-3")));
        let error = Args::try_parse_from(["civ6-judge", "--profile", "-1", "--p", "generic"])
            .err()
            .unwrap();
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
        assert_eq!(error.exit_code(), 2);
        parser_tests::assert_parse_error::<Args>(&[
            "civ6-judge",
            "--logs",
            "--unknown",
            "--l",
            "logs",
        ]);
    }

    #[test]
    fn compat_judge_prefixes_match_argparse() {
        parser_tests::assert_long_prefixes::<Args>(
            &["civ6-judge"],
            &[
                ("--logs", &["logs"]),
                ("--baseline", &["baseline.json"]),
                ("--snapshot", &["snapshot.json"]),
                ("--profile", &["generic"]),
                ("--expected-units", &["2"]),
            ],
        );
    }

    #[test]
    fn compat_judge_last_values_preserve_paths_choices_and_counts() {
        let args = Args::try_parse_from([
            "civ6-judge",
            "--logs",
            "old",
            "--l=",
            "--baseline",
            "old",
            "--b",
            "",
            "--snapshot",
            "old",
            "--s",
            "",
            "--profile",
            "bundle0",
            "--p",
            "growth",
            "--expected-units",
            "1",
            "--e",
            "2",
        ])
        .unwrap();
        for path in [args.logs, args.baseline, args.snapshot] {
            assert_eq!(path, Some(PathBuf::from(".")));
        }
        assert_eq!(args.profile, "growth");
        assert_eq!(args.expected_units, Some(2));
    }

    #[test]
    fn compat_judge_each_choice_and_numeric_occurrence_is_validated() {
        for args in [
            vec!["civ6-judge", "--expected-units", "", "--e", "2"],
            vec!["civ6-judge", "--expected-units", "1.5", "--e", "2"],
            vec!["civ6-judge", "--profile", "gen", "--p", "generic"],
            vec!["civ6-judge", "--profile", "", "--p", "generic"],
            vec!["civ6-judge", "--profile", "other", "--p", "generic"],
            vec!["civ6-judge", "--logs-extra", "logs"],
        ] {
            parser_tests::assert_parse_error::<Args>(&args);
        }
    }
}
