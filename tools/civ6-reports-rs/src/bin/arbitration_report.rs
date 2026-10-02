use civ6_reports_rs::cli::{self, ArbitrationArgs};

fn main() {
    if let Err(error) = cli::arbitration(ArbitrationArgs::parse()) {
        eprintln!("Report error: {error}");
        std::process::exit(1);
    }
}
