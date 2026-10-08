//! Fresh exit-sync barrier used by the chain while it still owns the game slot.
use civ6_autotest_rs::{
    steam_sync::{self, ExitEvidence, ExitMark, Monitor},
    time, Error, Result,
};
use clap::Parser;
use std::{
    path::{Path, PathBuf},
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
fn sharing_retry_delay(
    error: &Error,
    elapsed: f64,
    timeout: f64,
    windows: bool,
) -> Option<Duration> {
    let left = timeout - elapsed;
    (windows && error.is_windows_sharing_violation() && left.is_finite() && left > 0.0)
        .then(|| Duration::from_secs_f64(left.min(0.25)))
}
fn require_time_left(elapsed: f64, timeout: f64) -> Result<()> {
    if elapsed >= timeout {
        return Err(Error::input(
            "Steam exit sync did not finish within the deadline",
        ));
    }
    Ok(())
}
fn deadline_phase(failed: bool, empty: bool, blocked: bool) -> &'static str {
    if failed {
        "exit_sync_failed"
    } else if blocked {
        "exit_sync_observation_blocked"
    } else if empty {
        "exit_sync_no_events"
    } else {
        "exit_sync_timeout"
    }
}
fn enforce_deadline(
    m: &Monitor,
    journal: &Path,
    elapsed: f64,
    timeout: f64,
    blocked: bool,
) -> Result<()> {
    if let Err(error) = require_time_left(elapsed, timeout) {
        steam_sync::journal(
            journal,
            deadline_phase(m.failed(), m.states.is_empty(), blocked),
            m,
        )?;
        return Err(if m.failed() {
            Error::input("Steam exit sync failed; local saves retained; see steam-sync journal")
        } else {
            error
        });
    }
    Ok(())
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
    let mut sharing_phase = None;
    loop {
        lease.check()?;
        enforce_deadline(
            &m,
            &journal,
            start.elapsed().as_secs_f64(),
            args.timeout,
            sharing_phase.is_some(),
        )?;
        if let Err(e) = m.poll() {
            if let Some(delay) = sharing_retry_delay(
                &e,
                start.elapsed().as_secs_f64(),
                args.timeout,
                cfg!(windows),
            ) {
                if sharing_phase != Some("cloud") {
                    steam_sync::journal(&journal, "exit_cloud_log_sharing_wait", &m)?;
                    sharing_phase = Some("cloud");
                }
                std::thread::sleep(delay);
                continue;
            }
            steam_sync::journal(&journal, "exit_observation_invalid", &m)?;
            return Err(e);
        }
        let released = match steam_sync::fresh_release(&mark.process) {
            Ok(value) => value,
            Err(e) => {
                if let Some(delay) = sharing_retry_delay(
                    &e,
                    start.elapsed().as_secs_f64(),
                    args.timeout,
                    cfg!(windows),
                ) {
                    if sharing_phase != Some("process") {
                        steam_sync::journal(&journal, "exit_process_log_sharing_wait", &m)?;
                        sharing_phase = Some("process");
                    }
                    std::thread::sleep(delay);
                    continue;
                }
                steam_sync::journal(&journal, "exit_process_observation_invalid", &m)?;
                return Err(e);
            }
        };
        sharing_phase = None;
        let absent = civ6_autotest_rs::restart::single_civ_process(0.0)?.is_none();
        let elapsed = start.elapsed().as_secs_f64();
        enforce_deadline(&m, &journal, elapsed, args.timeout, false)?;
        match m.exit_evidence(elapsed, released, absent) {
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
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use civ6_autotest_rs::ErrorKind;
    #[test]
    fn a_shared_log_observation_keeps_one_deadline_and_never_authorizes_success() {
        let shared = Error::new(ErrorKind::Io, "locked (os error 32)");
        assert_eq!(
            sharing_retry_delay(&shared, 0.0, 45.0, true),
            Some(Duration::from_millis(250))
        );
        assert!(
            sharing_retry_delay(&shared, 44.9, 45.0, true).unwrap() <= Duration::from_millis(101)
        );
        for elapsed in [45.0, 46.0] {
            assert_eq!(sharing_retry_delay(&shared, elapsed, 45.0, true), None);
            assert!(require_time_left(elapsed, 45.0).is_err());
        }
        assert!(require_time_left(44.9, 45.0).is_ok());
        assert_eq!(sharing_retry_delay(&shared, 0.0, 45.0, false), None);
        for error in [
            Error::new(ErrorKind::Io, "denied (os error 5)"),
            Error::interrupted("cancelled (os error 32)"),
            Error::input("Steam sync failed"),
        ] {
            assert_eq!(sharing_retry_delay(&error, 0.0, 45.0, true), None);
        }
    }
    #[test]
    fn a_late_completion_is_rejected_with_the_observed_failure_or_empty_journal_phase() {
        let root = std::env::temp_dir().join(format!(
            "exit-deadline-{}-{}.jsonl",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let mut m = Monitor::from_cursor(PathBuf::new(), Default::default());
        assert!(enforce_deadline(&m, &root, 45.0, 45.0, false).is_err());
        assert!(enforce_deadline(&m, &root, 45.0, 45.0, true).is_err());
        m.states.insert(289070, steam_sync::SyncState::Complete);
        assert!(enforce_deadline(&m, &root, 46.0, 45.0, false).is_err());
        m.states.insert(289070, steam_sync::SyncState::Failed);
        let error = enforce_deadline(&m, &root, 46.0, 45.0, true).unwrap_err();
        assert!(error.message.contains("sync failed"));
        let phases: Vec<String> = std::fs::read_to_string(&root)
            .unwrap()
            .lines()
            .map(|l| {
                serde_json::from_str::<serde_json::Value>(l).unwrap()["phase"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(
            phases,
            [
                "exit_sync_no_events",
                "exit_sync_observation_blocked",
                "exit_sync_timeout",
                "exit_sync_failed"
            ]
        );
        std::fs::remove_file(root).unwrap();
    }
}
