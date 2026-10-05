use civ6_autotest_rs::{payment_scenario, repo_root, Result};
use clap::Parser;
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(about = "Generate a finite real cost-two or cost-three person payment diagnostic")]
struct Args {
    #[arg(long)]
    cost: u8,
    #[arg(long)]
    output: PathBuf,
}
fn run(args: Args) -> Result<()> {
    let template: serde_json::Value = serde_json::from_slice(&fs::read(
        repo_root().join("tools/civ6-autotest/scenarios/leader-persona-15km.json"),
    )?)?;
    let result = payment_scenario::build(&template, args.cost)?;
    // A previously reviewed/run scenario must not be silently overwritten.
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(args.output)?;
    use std::io::Write;
    out.write_all(serde_json::to_string_pretty(&result)?.as_bytes())?;
    out.write_all(b"\n")?;
    Ok(())
}
fn main() {
    if let Err(e) = run(Args::parse()) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
