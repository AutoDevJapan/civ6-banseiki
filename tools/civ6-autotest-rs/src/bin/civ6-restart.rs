use civ6_autotest_rs::{click, restart, time::monotonic};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Inspect or control the verified Civ VI process")]
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
    Wait {
        #[arg(long, default_value_t = 120.0)]
        seconds: f64,
    },
    /// Ask the verified game window to close.
    Close,
    /// Force terminate only the matching process and creation time.
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
