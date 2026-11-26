use civ6_autotest_rs::{persona_payment, repo_root, scenario, Result};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

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
fn generate(input: &Path, output: &Path) -> Result<()> {
    let mut data: serde_json::Value = serde_json::from_slice(&fs::read(input)?)?;
    data["name"] = json!("PR86 cost-one current Core promotion flags independent UI comparison");
    let steps = data["steps"].as_array_mut().unwrap();
    let start = steps
        .iter()
        .position(|s| s["action"] == "start_native_assign_calibration")
        .ok_or_else(|| civ6_autotest_rs::Error::input("base assignment observation missing"))?;
    steps.insert(
        start + 1,
        json!({"action":"ui_lua","state_name":"InGame","code":persona_payment::READ_COST_ONE}),
    );
    scenario::validate_scenario(&data)?;
    let bytes = serde_json::to_vec_pretty(&data)?;
    // Match the payment-scenario generator: reviewed inputs and prior output
    // are not overwritten, including hard links and symbolic-link aliases.
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    out.write_all(&bytes)?;
    Ok(())
}

fn run(args: Args) -> Result<()> {
    match args.command {
        Command::Generate { output } => {
            let path = repo_root()
                .join("tools/civ6-autotest/scenarios/leader-persona-assign-calibration-15km.json");
            generate(&path, &output)?;
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
            if evidence.artifact_path.canonicalize()? != trace {
                return Err(civ6_autotest_rs::Error::input(
                    "trace path differs from the observed restoration path",
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn original(directory: &Path) -> PathBuf {
        let input = directory.join("original.json");
        fs::copy(
            repo_root()
                .join("tools/civ6-autotest/scenarios/leader-persona-assign-calibration-15km.json"),
            &input,
        )
        .unwrap();
        input
    }

    #[test]
    fn generation_preserves_its_input_and_retains_every_original_operation() {
        let directory = tempfile::tempdir().unwrap();
        let input = original(directory.path());
        let before = fs::read(&input).unwrap();
        assert!(generate(&input, &input).is_err());
        assert_eq!(fs::read(&input).unwrap(), before);

        let output = directory.path().join("generated.json");
        generate(&input, &output).unwrap();
        let mut result: serde_json::Value =
            serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        scenario::validate_scenario(&result).unwrap();
        let source: serde_json::Value = serde_json::from_slice(&before).unwrap();
        let steps = result["steps"].as_array_mut().unwrap();
        let start = steps
            .iter()
            .position(|step| step["action"] == "start_native_assign_calibration")
            .unwrap();
        assert_eq!(steps[start + 1]["code"], persona_payment::READ_COST_ONE);
        steps.remove(start + 1);
        assert_eq!(*steps, source["steps"].as_array().unwrap().clone());
        assert_eq!(fs::read(&input).unwrap(), before);
    }

    #[test]
    fn generation_preserves_hardlinked_input_and_prior_diagnostics() {
        let directory = tempfile::tempdir().unwrap();
        let input = original(directory.path());
        let before = fs::read(&input).unwrap();
        let alias = directory.path().join("alias.json");
        fs::hard_link(&input, &alias).unwrap();
        assert!(generate(&input, &alias).is_err());
        assert_eq!(fs::read(&input).unwrap(), before);
        assert_eq!(fs::read(&alias).unwrap(), before);

        let prior = directory.path().join("reviewed.json");
        fs::write(&prior, b"previous reviewed result").unwrap();
        assert!(generate(&input, &prior).is_err());
        assert_eq!(fs::read(&prior).unwrap(), b"previous reviewed result");
    }

    #[test]
    fn invalid_input_creates_no_output_and_keeps_prior_output_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("invalid.json");
        fs::write(&input, b"invalid json").unwrap();
        let output = directory.path().join("not-created.json");
        assert!(generate(&input, &output).is_err());
        assert!(!output.exists());
        fs::write(&output, b"previous result").unwrap();
        assert!(generate(&input, &output).is_err());
        assert_eq!(fs::read(&input).unwrap(), b"invalid json");
        assert_eq!(fs::read(&output).unwrap(), b"previous result");
    }

    #[cfg(unix)]
    #[test]
    fn generation_does_not_follow_output_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        let input = original(directory.path());
        let before = fs::read(&input).unwrap();
        let alias = directory.path().join("alias.json");
        std::os::unix::fs::symlink(&input, &alias).unwrap();
        assert!(generate(&input, &alias).is_err());
        assert_eq!(fs::read(&input).unwrap(), before);
        let missing = directory.path().join("missing-original");
        let dangling = directory.path().join("dangling.json");
        std::os::unix::fs::symlink(&missing, &dangling).unwrap();
        assert!(generate(&input, &dangling).is_err());
        assert!(!missing.exists());
    }
}
