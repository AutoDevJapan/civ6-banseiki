use civ6_autotest_rs::{persona_payment, repo_root, scenario, Result};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(about = "Generate or check read-only native persona payment evidence")]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Extend the proven cost-one scene with an independent promotion read.
    Generate { output: PathBuf },
    /// Check one restored native trace against its successful UI step receipt.
    Check { trace: PathBuf, steps: PathBuf },
}
fn run(args: Args) -> Result<()> {
    match args.command {
        Command::Generate { output } => {
            let path = repo_root()
                .join("tools/civ6-autotest/scenarios/leader-persona-assign-calibration-15km.json");
            let mut data: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
            data["name"] =
                json!("PR86 cost-one current Core promotion flags independent UI comparison");
            let steps = data["steps"].as_array_mut().unwrap();
            let start = steps
                .iter()
                .position(|s| s["action"] == "start_native_assign_calibration")
                .ok_or_else(|| {
                    civ6_autotest_rs::Error::input("base assignment observation missing")
                })?;
            steps.insert(start+1,json!({"action":"ui_lua","state_name":"InGame","code":persona_payment::READ_COST_ONE}));
            scenario::validate_scenario(&data)?;
            fs::write(output, serde_json::to_vec_pretty(&data)?)?;
        }
        Command::Check { trace, steps } => {
            let trace = trace.canonicalize()?;
            let steps = steps.canonicalize()?;
            if trace.parent() != steps.parent()
                || steps.file_name().and_then(|s| s.to_str()) != Some("steps.jsonl")
            {
                return Err(civ6_autotest_rs::Error::input(
                    "trace and steps must belong to one artifact directory",
                ));
            }
            let evidence = persona_payment::evidence_from_steps(&fs::read_to_string(steps)?)?;
            if trace.file_name().and_then(|s| s.to_str()) != Some(evidence.artifact.as_str()) {
                return Err(civ6_autotest_rs::Error::input(
                    "trace filename differs from observed start and stop",
                ));
            }
            let report = persona_payment::verify(&fs::read_to_string(trace)?, &evidence)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
