use civ6_data_rs::{absorption_admission, apply_outputs, parse_check_args, Repository};

fn main() -> anyhow::Result<()> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let envoy_probe = if let Some(index) = args.iter().position(|a| a == "--envoy-probe") {
        args.remove(index);
        true
    } else {
        false
    };
    let scenario_path = if let Some(index) = args.iter().position(|a| a == "--scenario-output") {
        anyhow::ensure!(index + 1 < args.len(), "--scenario-output requires a path");
        let path = args.remove(index + 1);
        args.remove(index);
        Some(std::path::PathBuf::from(path))
    } else {
        None
    };
    anyhow::ensure!(
        !envoy_probe || scenario_path.is_some(),
        "--envoy-probe requires --scenario-output"
    );
    let Some(check) = parse_check_args(
        "absorption-admission-generate",
        &args,
        "Generate the Gameplay connection from normal admission to three-stage absorption.",
    )?
    else {
        return Ok(());
    };
    let repo = Repository::checked_in();
    apply_outputs(&repo, absorption_admission::generate(&repo)?, check)?;
    if let Some(path) = scenario_path {
        let scenario = if envoy_probe {
            absorption_admission::envoy_probe_scenario(&repo)?
        } else {
            absorption_admission::live_scenario(&repo)?
        };
        if check {
            anyhow::ensure!(
                std::fs::read_to_string(&path)? == scenario,
                "stale generated scenario: {}",
                path.display()
            );
        } else {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, scenario)?;
        }
    }
    Ok(())
}
