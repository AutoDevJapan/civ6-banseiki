use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Verify original persona 113-step raw artifacts, without running Civ6")]
struct Args {
    artifact: PathBuf,
    /// Verify the existing five-operation startup comparison instead.
    #[arg(long)]
    baseline: bool,
}

fn main() {
    let args = Args::parse();
    let verified = if args.baseline {
        civ6_reports_rs::persona_full::verify_baseline_directory(&args.artifact)
    } else {
        civ6_reports_rs::persona_full::verify_directory(&args.artifact)
    };
    match verified {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => {
            eprintln!("Persona full scenario verification failed: {error}");
            std::process::exit(1);
        }
    }
}
