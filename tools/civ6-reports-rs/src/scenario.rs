use crate::{common, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[path = "scenario_lua.rs"]
mod script;
pub use script::PREAMBLE;

pub fn scenario_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../civ6-autotest/scenarios")
}

pub fn default_output() -> PathBuf {
    scenario_folder().join("court-arbitration-ai.json")
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
    scenario_from_base(&serde_json::from_str(&common::read_text(
        &scenario_folder().join("japan-15km-200turn-full.json"),
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
