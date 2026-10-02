use civ6_reports_rs::cli::{self, TuningArgs};
use clap::Parser;

fn main() {
    if let Err(error) = cli::tuning(TuningArgs::parse()) {
        eprintln!("Report error: {error}");
        std::process::exit(1);
    }
}
