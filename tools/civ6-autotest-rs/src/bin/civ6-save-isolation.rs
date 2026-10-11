use civ6_autotest_rs::{Error, Result};
use clap::Parser;
#[derive(Parser)]
struct Args {
    #[arg(long)]
    begin: bool,
    #[arg(long)]
    restore: bool,
    #[arg(long)]
    state: std::path::PathBuf,
    #[arg(long)]
    archive: Option<std::path::PathBuf>,
    #[arg(long, value_parser = ["single_player", "hotseat"], default_value = "single_player")]
    session_type: String,
}
fn run(a: Args) -> Result<()> {
    let mut lease = civ6_autotest_rs::startup::RecoveryLease::acquire()?;
    lease.check()?;
    if civ6_autotest_rs::restart::single_civ_process(0.0)?.is_some() {
        return Err(Error::input("autosave isolation requires a closed Civ6"));
    }
    let profile = std::path::PathBuf::from(
        std::env::var_os("USERPROFILE").ok_or_else(|| Error::input("USERPROFILE unavailable"))?,
    );
    match (a.begin, a.restore) {
        (true, false) => civ6_autotest_rs::save_isolation::begin_for_session(
            &profile,
            &a.archive.ok_or_else(|| Error::input("archive required"))?,
            &a.state,
            civ6_autotest_rs::session::SessionType::from_scenario(
                &serde_json::json!({"session_type":a.session_type}),
            )?,
        ),
        (false, true) => civ6_autotest_rs::save_isolation::retry_recovery_with(
            &civ6_autotest_rs::time::SystemClock,
            25.0,
            || {
                lease.check()?;
                if civ6_autotest_rs::restart::single_civ_process(0.0)?.is_some() {
                    return Err(Error::input("Civ6 appeared before autosave recovery"));
                }
                Ok(())
            },
            || civ6_autotest_rs::save_isolation::restore(&profile, &a.state),
        ),
        _ => Err(Error::input("choose begin or restore")),
    }
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
