use civ6_autotest_rs::click;
use clap::Parser;

#[derive(Parser)]
#[command(about = "Send verified input to the one Civ VI window")]
struct Args {
    /// Horizontal fraction of the window (0..1).
    fx: Option<f64>,
    /// Vertical fraction of the window (0..1).
    fy: Option<f64>,
    /// Post a virtual key instead of clicking.
    #[arg(long)]
    key: Option<u16>,
    /// Request the Civ VI window to close.
    #[arg(long)]
    close: bool,
    /// Absolute monotonic deadline for --key.
    #[arg(long)]
    deadline: Option<f64>,
}

fn main() {
    let args = Args::parse();
    let result = match (args.fx, args.fy, args.key, args.close) {
        (Some(x), Some(y), None, false) => {
            click::click_fraction(x, y).map(|(x, y)| format!("({x}, {y})"))
        }
        (None, None, Some(vk), false) => {
            click::press_key(vk, args.deadline).map(|_| "key sent".to_string())
        }
        (None, None, None, true) => {
            click::request_civ_window_close().map(|_| "close requested".to_string())
        }
        _ => Err(civ6_autotest_rs::Error::input(
            "Supply fx fy, --key VK, or --close",
        )),
    };
    match result {
        Ok(message) => println!("{message}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
