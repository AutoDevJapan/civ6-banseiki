use crate::{common, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[path = "scenario_lua.rs"]
mod script;
pub use script::PREAMBLE;

const SCENARIO_FOLDER: &str = "tools/civ6-autotest/scenarios";
const BASE_SCENARIO: &str = "japan-15km-200turn-full.json";

fn has_base_scenario(repository: &Path) -> bool {
    repository
        .join(SCENARIO_FOLDER)
        .join(BASE_SCENARIO)
        .is_file()
}

fn find_repository(start: &Path) -> Option<&Path> {
    start.ancestors().find(|path| has_base_scenario(path))
}

/// Prefer the checkout containing this executable to an unrelated working directory.
/// Installed binaries can use the working directory or an explicit repository.
pub fn repository_root(repository: Option<&Path>) -> Result<PathBuf> {
    if let Some(repository) = repository {
        let repository = repository.canonicalize()?;
        if !has_base_scenario(&repository) {
            return Err(format!(
                "repository {} has no {SCENARIO_FOLDER}/{BASE_SCENARIO}",
                repository.display()
            )
            .into());
        }
        return Ok(repository);
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(repository) = executable.parent().and_then(find_repository) {
            return Ok(repository.canonicalize()?);
        }
    }
    let working_directory = std::env::current_dir()?;
    if let Some(repository) = find_repository(&working_directory) {
        return Ok(repository.canonicalize()?);
    }
    Err(format!(
        "cannot locate {SCENARIO_FOLDER}/{BASE_SCENARIO}; run from the checkout or use --repo"
    )
    .into())
}

pub fn scenario_folder() -> Result<PathBuf> {
    Ok(repository_root(None)?.join(SCENARIO_FOLDER))
}

pub fn default_output() -> Result<PathBuf> {
    Ok(output_in_repository(&repository_root(None)?))
}

pub fn output_in_repository(repository: &Path) -> PathBuf {
    repository
        .join(SCENARIO_FOLDER)
        .join("court-arbitration-ai.json")
}

pub fn snapshot() -> String {
    [script::PREAMBLE, script::SNAPSHOT_TAIL].concat()
}

fn lua(steps: &mut Vec<Value>, code: String) {
    steps.push(json!({"action": "ui_lua", "state_name": "GameCore_Tuner", "code": code}));
}

fn turn(steps: &mut Vec<Value>) {
    steps.push(json!({"action": "autoplay", "turns": 1, "timeout": 600}));
}

fn round_trip(steps: &mut Vec<Value>, label: &str) {
    let capture = label.to_uppercase().replace('-', "_");
    lua(steps, snapshot());
    steps.last_mut().unwrap()["capture_as"] = json!(capture);
    steps.push(json!({"action": "save", "name": format!("JAC-AI-{label}"), "timeout": 90}));
    lua(steps, [script::PREAMBLE, script::MUTATE_STATE].concat());
    steps.push(json!({"action": "exit_to_menu", "timeout": 600}));
    steps.push(json!({"action": "load", "name": format!("JAC-AI-{label}"), "timeout": 600}));
    steps.push(json!({"action": "wait_for", "timeout": 600,
        "condition": {"type": "lua", "state_name": "InGame", "code": "local p=Players[Game.GetLocalPlayer()]; return p~=nil and p:IsTurnActive()"}}));
    lua(steps, snapshot());
    steps.last_mut().unwrap()["equals_capture"] = json!(capture);
}

pub fn scenario() -> Result<Value> {
    scenario_in_repository(&repository_root(None)?)
}

pub fn scenario_in_repository(repository: &Path) -> Result<Value> {
    scenario_from_base(&serde_json::from_str(&common::read_text(
        &repository.join(SCENARIO_FOLDER).join(BASE_SCENARIO),
    )?)?)
}

/// Take setup from the same existing artifact as the original generator.
pub fn scenario_from_base(base: &Value) -> Result<Value> {
    let original = base
        .get("steps")
        .and_then(Value::as_array)
        .ok_or("base scenario has no steps")?;
    let mut steps: Vec<_> = original.iter().take(3).cloned().collect();
    if steps.len() < 2 {
        return Err("base scenario has no configure step".into());
    }
    steps[1]["seed"] = json!(620981);
    lua(
        &mut steps,
        [
            script::PREPARE_PARTY_PREFIX,
            script::PICK_PAIR,
            script::PREPARE_PARTY_SUFFIX,
        ]
        .concat(),
    );
    turn(&mut steps);
    lua(
        &mut steps,
        [script::PREAMBLE, script::PARTY_PENDING].concat(),
    );
    round_trip(&mut steps, "party-pending");
    turn(&mut steps);
    lua(
        &mut steps,
        [script::PREAMBLE, script::PARTY_RESOLVED].concat(),
    );
    round_trip(&mut steps, "party-resolved");
    lua(
        &mut steps,
        [script::PREAMBLE, script::PICK_PAIR, script::PREPARE_SHOGUN].concat(),
    );
    turn(&mut steps);
    lua(
        &mut steps,
        [script::PREAMBLE, script::SHOGUN_PENDING].concat(),
    );
    round_trip(&mut steps, "shogun-pending");
    turn(&mut steps);
    lua(
        &mut steps,
        [script::PREAMBLE, script::SHOGUN_RESOLVED].concat(),
    );
    round_trip(&mut steps, "shogun-resolved");
    lua(
        &mut steps,
        [
            script::PREAMBLE,
            script::PREPARE_CUTOFF_PREFIX,
            script::PICK_PAIR,
            script::PREPARE_CUTOFF_SUFFIX,
        ]
        .concat(),
    );
    turn(&mut steps);
    lua(
        &mut steps,
        [script::PREAMBLE, script::LAST_EMPOWERED].concat(),
    );
    turn(&mut steps);
    lua(&mut steps, [script::PREAMBLE, script::AT_CUTOFF].concat());
    round_trip(&mut steps, "after-cutoff");
    turn(&mut steps);
    lua(
        &mut steps,
        [script::PREAMBLE, script::AFTER_CUTOFF].concat(),
    );
    Ok(
        json!({"name": "AI arbitration: exhausted AI vs AI party, shogun third-party proposal, exact 1573 cutoff, five save/load round trips. Army losses are real; prior fatigue and boundary clock are controlled fixtures.",
        "judge_profile": "japan_map", "test_mode": {"skipped_turns": 113, "continuous_performance_valid": false}, "steps": steps}),
    )
}
