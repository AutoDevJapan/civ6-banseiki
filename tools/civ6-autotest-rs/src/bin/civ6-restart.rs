use civ6_autotest_rs::{click, restart, time::monotonic};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    about = "Inspect or control the verified Civ VI process",
    infer_long_args = true,
    args_override_self = true,
    allow_negative_numbers = true
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Report the single game process if present.
    Status,
    /// Close orphan Civ VI crash reporters, then launch DX11 via Steam.
    Launch,
    /// Wait for a game window and verify it is DX11.
    #[command(allow_negative_numbers = true)]
    Wait {
        #[arg(long, default_value_t = 120.0)]
        seconds: f64,
    },
    /// Ask the verified game window to close.
    Close,
    /// Force terminate only the matching process and creation time.
    #[command(allow_negative_numbers = true)]
    Terminate { pid: u32, expected_start: f64 },
}

fn main() {
    let args = Args::parse();
    let result = match args.command {
        Command::Status => restart::single_civ_process(30.0).map(|p| format!("{p:?}")),
        Command::Launch => restart::launch_via_steam().map(|_| restart::STEAM_URI.to_string()),
        Command::Wait { seconds } if seconds.is_finite() && seconds > 0.0 => {
            restart::wait_for_civ_window(monotonic() + seconds).and_then(|hwnd| {
                restart::require_dx11_window(hwnd)?;
                Ok(format!("DX11 window {hwnd}"))
            })
        }
        Command::Wait { .. } => Err(civ6_autotest_rs::Error::input(
            "seconds must be finite and positive",
        )),
        Command::Close => click::request_civ_window_close().map(|_| "close requested".to_string()),
        Command::Terminate {
            pid,
            expected_start,
        } => {
            restart::terminate_civ_process(pid, expected_start).map(|_| format!("terminated {pid}"))
        }
    };
    match result {
        Ok(message) => println!("{message}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
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
    fn compat_restart_negative_numbers_apply_to_subcommands_without_accepting_flags() {
        let args = Args::try_parse_from(["civ6-restart", "wait", "--seconds", "-1.5", "--s", "0"])
            .unwrap();
        assert!(matches!(args.command, Command::Wait { seconds } if seconds == 0.0));
        let args = Args::try_parse_from(["civ6-restart", "terminate", "1", "-1.5"]).unwrap();
        assert!(
            matches!(args.command, Command::Terminate { pid: 1, expected_start } if expected_start == -1.5)
        );
        parser_tests::assert_parse_error::<Args>(&[
            "civ6-restart",
            "wait",
            "--seconds",
            "--unknown",
            "--s",
            "0",
        ]);
    }

    #[test]
    fn compat_restart_wait_inherits_long_prefixes_and_last_value() {
        parser_tests::assert_long_prefixes::<Args>(
            &["civ6-restart", "wait"],
            &[("--seconds", &["120"])],
        );
        let args =
            Args::try_parse_from(["civ6-restart", "wait", "--seconds", "120", "--s", "0"]).unwrap();
        assert!(matches!(args.command, Command::Wait { seconds } if seconds == 0.0));
    }

    #[test]
    fn compat_restart_commands_and_each_numeric_occurrence_stay_strict() {
        for args in [
            vec!["civ6-restart", "sta"],
            vec!["civ6-restart", "w"],
            vec!["civ6-restart", "WAIT"],
            vec!["civ6-restart", "wait", "--seconds", "", "--s", "120"],
            vec!["civ6-restart", "wait", "--seconds", "bad", "--s", "120"],
            vec!["civ6-restart", "wait", "--seconds-extra", "120"],
            vec!["civ6-restart", "terminate", "", "1"],
            vec!["civ6-restart", "terminate", "1", ""],
        ] {
            parser_tests::assert_parse_error::<Args>(&args);
        }
    }
}
