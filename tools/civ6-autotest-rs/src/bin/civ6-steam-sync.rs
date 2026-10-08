//! Fresh exit-sync barrier used by the chain while it still owns the game slot.
use civ6_autotest_rs::{
    steam_sync::{self, ExitEvidence, ExitMark, Monitor},
    time, Error, Result,
};
use clap::Parser;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    mark: bool,
    #[arg(long)]
    cursor: Option<String>,
    #[arg(long)]
    journal: Option<PathBuf>,
    #[arg(long, default_value_t = 45.0)]
    timeout: f64,
}
fn main() -> std::process::ExitCode {
    match run(Args::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}
fn run(args: Args) -> Result<()> {
    time::install_interrupt_handler()?;
    let path = steam_sync::cloud_log();
    if args.mark {
        println!("{}", serde_json::to_string(&ExitMark::capture()?)?);
        return Ok(());
    }
    if !args.timeout.is_finite() || args.timeout < 3.0 || args.timeout > 120.0 {
        return Err(Error::input(
            "sync timeout must be between 3 and 120 seconds",
        ));
    }
    let mut lease = civ6_autotest_rs::startup::RecoveryLease::acquire()?;
    let mark: ExitMark = serde_json::from_str(
        &args
            .cursor
            .ok_or_else(|| Error::input("fresh generation cursor required"))?,
    )?;
    let mut m = Monitor::from_cursor(path, mark.cloud);
    let journal = args
        .journal
        .ok_or_else(|| Error::input("sync journal required"))?;
    steam_sync::journal(&journal, "exit_wait_begin", &m)?;
    let start = Instant::now();
    loop {
        lease.check()?;
        if let Err(e) = m.poll() {
            steam_sync::journal(&journal, "exit_observation_invalid", &m)?;
            return Err(e);
        }
        let released = match steam_sync::fresh_release(&mark.process) {
            Ok(value) => value,
            Err(e) => {
                steam_sync::journal(&journal, "exit_process_observation_invalid", &m)?;
                return Err(e);
            }
        };
        let absent = civ6_autotest_rs::restart::single_civ_process(0.0)?.is_none();
        match m.exit_evidence(start.elapsed().as_secs_f64(), released, absent) {
            ExitEvidence::Failed => {
                steam_sync::journal(&journal, "exit_sync_failed", &m)?;
                return Err(Error::input(
                    "Steam exit sync failed; local saves retained; see steam-sync journal",
                ));
            }
            ExitEvidence::SyncComplete if absent => {
                steam_sync::journal(&journal, "exit_sync_complete", &m)?;
                return Ok(());
            }
            ExitEvidence::ReleasedWithoutNewSyncRequest => {
                // Play anyway can skip Cloud for the entire session. This
                // is explicit Steam idle evidence, not a claimed sync.
                steam_sync::journal(&journal, "steam_released_without_new_sync_request", &m)?;
                return Ok(());
            }
            _ => {}
        }
        if start.elapsed().as_secs_f64() >= args.timeout {
            steam_sync::journal(
                &journal,
                if m.states.is_empty() {
                    "exit_sync_no_events"
                } else {
                    "exit_sync_timeout"
                },
                &m,
            )?;
            return Err(Error::input(
                "Steam exit sync did not finish within the deadline",
            ));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}
