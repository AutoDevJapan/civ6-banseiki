//! Parent-invoked one-shot wrapper. No new game, identity argument or payment.
use clap::Parser;
use std::{io::Write, path::PathBuf, process::Command};
#[derive(Parser)]
struct Args {
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "4318")]
    port: u16,
    /// Offline only: write/check the common case, never open a process.
    #[arg(long)]
    prepare_only: bool,
}
fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let args = Args::parse();
    if !args.prepare_only {
        if std::env::var_os("CIV6_LIVE_CHAIN_PID").is_none()
            || std::env::var_os("CIV6_LIVE_LOCK_PATH").is_none()
        {
            return Err("run inside the parent's owned live_chain environment; this wrapper acquires no lease".into());
        }
        if civ6_autotest_rs::live_lease::environment_budget()?
            .is_some_and(|seconds| seconds < 360.0)
        {
            return Err("parent lease has less than the bounded reader and cleanup budget".into());
        }
    }
    let case = civ6_autotest_rs::ai_reader_case::scenario();
    civ6_autotest_rs::scenario::validate_scenario(&case)?;
    std::fs::create_dir(&args.output)?;
    let scenario = args.output.join("reader-one.json");
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&scenario)?;
    writeln!(file, "{}", serde_json::to_string_pretty(&case)?)?;
    if args.prepare_only {
        println!("{}; no game opened", scenario.display());
        return Ok(0);
    }
    let driver = std::env::current_exe()?.with_file_name("civ6-autotest.exe");
    let mut command = Command::new(driver);
    command
        .args([
            "--no-auto-restart",
            "--port",
            &args.port.to_string(),
            "--output",
        ])
        .arg(args.output.join("run"))
        .arg(&scenario)
        .env("CIV6_PERSONA_RUNTIME_RESTART", "0");
    // Keep the selected candidate trio adjacent; do not inherit another lane's
    // controller override. Parent/live-chain owns the actual game and lease.
    for key in [
        "CIV6_NATIVE_BRIDGE_DIRECTORY",
        "CIV6_NATIVE_BRIDGE_CONTROLLER_SHA256",
        "CIV6_NATIVE_BRIDGE_DLL_SHA256",
    ] {
        command.env_remove(key);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    Ok(command.status()?.code().unwrap_or(2))
}
fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("AI read-only case: {e}");
            std::process::exit(2)
        }
    }
}
