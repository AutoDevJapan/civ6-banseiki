use civ6_autotest_rs::judge::{default_log_dir, judge, print_table, read_since, snapshot_logs};
use clap::Parser;
use std::{fs, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(about = "Judge Civ VI PoC logs from a byte-offset baseline")]
struct Args {
    #[arg(long)]
    logs: Option<PathBuf>,
    #[arg(long)]
    baseline: Option<PathBuf>,
    #[arg(long, help = "Write baseline before a manual run, then exit")]
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
