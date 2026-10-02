use civ6_autotest_rs::capture;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Capture the verified Civ VI window as PNG")]
struct Args {
    path: PathBuf,
    /// Crop fractions left top right bottom.
    #[arg(long, num_args = 4)]
    crop: Option<Vec<f64>>,
    /// Absolute monotonic deadline.
    #[arg(long)]
    deadline: Option<f64>,
}

fn main() {
    let args = Args::parse();
    let crop = args.crop.map(|v| [v[0], v[1], v[2], v[3]]);
    if let Err(error) = capture::capture_civ_window(&args.path, crop, args.deadline) {
        eprintln!("{error}");
        std::process::exit(1);
    }
    println!("{}", args.path.display());
}
