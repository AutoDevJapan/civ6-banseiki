use civ6_reports_rs::cli::{self, TuningArgs};

fn main() {
    if let Err(error) = cli::tuning(TuningArgs::parse()) {
        eprintln!("Report error: {error}");
        std::process::exit(1);
    }
}
