use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Verify original persona 113-step raw artifacts, without running Civ6")]
struct Args {
    artifact: PathBuf,
}

fn main() {
    match civ6_reports_rs::persona_full::verify_directory(&Args::parse().artifact) {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => {
            eprintln!("Persona full scenario verification failed: {error}");
            std::process::exit(1);
        }
    }
}
