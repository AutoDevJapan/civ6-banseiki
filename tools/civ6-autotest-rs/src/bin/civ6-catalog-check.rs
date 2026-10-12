use civ6_autotest_rs::catalog_check;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Offline normal 22-person raw capture verifier (no live operations)")]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Check { trace: PathBuf, steps: PathBuf },
}

fn main() {
    let Command::Check { trace, steps } = Args::parse().command;
    match catalog_check::check_files(&trace, &steps) {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("serializable report")
            );
            if !report.qualification_positive22 {
                std::process::exit(1);
            }
        }
        Err(error) => {
            println!(
                "{}",
                serde_json::json!({"capture_valid":false,"qualification_positive22":false,
                "product_acceptance":false,"payment_authority":false,"error":error.to_string()})
            );
            std::process::exit(1);
        }
    }
}
