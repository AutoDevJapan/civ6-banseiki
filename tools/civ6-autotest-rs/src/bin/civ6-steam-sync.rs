//! Fresh exit-sync barrier used by the chain while it still owns the game slot.
use civ6_autotest_rs::{
    steam_sync::{self, Monitor},
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
    offset: Option<u64>,
    #[arg(long)]
    process_offset: Option<u64>,
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
        let size = match std::fs::metadata(path) {
            Ok(m) => m.len(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
            Err(e) => return Err(e.into()),
        };
        println!("{size}");
        return Ok(());
    }
    if !args.timeout.is_finite() || args.timeout < 3.0 || args.timeout > 120.0 {
        return Err(Error::input(
            "sync timeout must be between 3 and 120 seconds",
        ));
    }
    let mut lease = civ6_autotest_rs::startup::RecoveryLease::acquire()?;
    let mut m = Monitor::from_offset(
        path,
        args.offset
            .ok_or_else(|| Error::input("fresh cloud log offset required"))?,
    );
    let journal = args
        .journal
        .ok_or_else(|| Error::input("sync journal required"))?;
    steam_sync::journal(&journal, "exit_wait_begin", &m)?;
    let start = Instant::now();
    loop {
        lease.check()?;
        m.poll()?;
        if m.failed() {
            steam_sync::journal(&journal, "exit_sync_failed", &m)?;
            return Err(Error::input(
                "Steam exit sync failed; local saves retained; see steam-sync journal",
            ));
        }
        if start.elapsed().as_secs_f64() >= 10.0 && !m.pending() && !m.states.is_empty() {
            steam_sync::journal(&journal, "exit_sync_complete", &m)?;
            return Ok(());
        }
        if start.elapsed().as_secs_f64() >= 10.0 && m.states.is_empty() {
            if let Some(offset) = args.process_offset {
                if steam_sync::fresh_release(offset)?
                    && civ6_autotest_rs::restart::single_civ_process(0.0)?.is_none()
                {
                    // Play anyway can skip Cloud for the entire session. This
                    // is explicit Steam idle evidence, not a claimed sync.
                    steam_sync::journal(&journal, "steam_released_without_new_sync_request", &m)?;
                    return Ok(());
                }
            }
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
