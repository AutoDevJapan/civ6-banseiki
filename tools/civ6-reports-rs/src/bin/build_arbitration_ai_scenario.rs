use civ6_reports_rs::cli::{self, ScenarioArgs};
use clap::Parser;

fn main() {
    if let Err(error) = cli::scenario(ScenarioArgs::parse()) {
        eprintln!("Scenario error: {error}");
        std::process::exit(1);
    }
}
