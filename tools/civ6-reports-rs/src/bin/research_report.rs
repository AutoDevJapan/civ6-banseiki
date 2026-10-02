use civ6_reports_rs::cli::{self, ResearchArgs};
use clap::Parser;

fn main() {
    if let Err(error) = cli::research(ResearchArgs::parse()) {
        eprintln!("Report error: {error}");
        std::process::exit(2);
    }
}
