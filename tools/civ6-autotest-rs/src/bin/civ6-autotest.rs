use civ6_autotest_rs::{judge, runner::Runner, scenario, Error, Result};
use clap::Parser;
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(about = "Run a Civ VI PoC scenario with automatic mod freshness checks")]
struct Args {
    scenario: PathBuf,
    #[arg(long)]
    check: bool,
    #[arg(long)]
    test_mode: bool,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    logs: Option<PathBuf>,
    #[arg(long)]
    saves: Option<PathBuf>,
    #[arg(long, default_value_t = 4318)]
    port: u16,
    #[arg(long)]
    mods: Option<PathBuf>,
    #[arg(long)]
    no_auto_restart: bool,
    #[arg(long)]
    owner_start: bool,
}
fn run(args: Args) -> Result<i32> {
    if args.port == 0 {
        return Err(Error::scenario("Invalid port or timeout"));
    }
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
        args.port,
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
