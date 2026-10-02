use civ6_reports_rs::cli::{self, ScenarioArgs};

fn main() {
    if let Err(error) = cli::scenario(ScenarioArgs::parse()) {
        eprintln!("Scenario error: {error}");
        std::process::exit(1);
    }
}
