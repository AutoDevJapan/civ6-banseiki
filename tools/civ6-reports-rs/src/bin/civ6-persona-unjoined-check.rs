use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Verify fixed normal-start unjoined-AI raw artifacts without running Civ6")]
struct Args {
    artifact: PathBuf,
    /// Byte length recorded from the original host save, outside the artifact.
    #[arg(long)]
    source_save_bytes: u64,
    /// SHA-256 recorded from the original host save, outside the artifact.
    #[arg(long)]
    source_save_sha256: String,
}

fn main() {
    let args = Args::parse();
    match civ6_reports_rs::persona_unjoined::verify_directory(
        &args.artifact,
        args.source_save_bytes,
        &args.source_save_sha256,
    ) {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => {
            eprintln!("Unjoined AI verification failed: {error}");
            std::process::exit(1);
        }
    }
}
