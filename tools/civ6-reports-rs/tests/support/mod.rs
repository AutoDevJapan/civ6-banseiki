use mlua::{Lua, LuaSerdeExt};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

pub fn root() -> PathBuf {
    // Default CLI output belongs to the executable's checkout. A shared Cargo
    // target can be inside a different checkout from the fixture source.
    for start in [
        std::env::current_exe().unwrap(),
        std::env::current_dir().unwrap(),
    ] {
        if let Some(root) = start.ancestors().find(|path| path.join(".git").exists()) {
            return root.to_path_buf();
        }
    }
    source_root()
}

pub fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn tempdir() -> TempDir {
    tempfile::tempdir().unwrap()
}

pub fn repo_text(path: &str) -> String {
    fs::read_to_string(source_root().join(path))
        .unwrap()
        .replace("\r\n", "\n")
}

pub fn repo_json(path: &str) -> Value {
    serde_json::from_str(&repo_text(path)).unwrap()
}

pub fn write_steps(root: &Path, steps: &[Value]) {
    fs::write(
        root.join("steps.jsonl"),
        steps
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
}

pub fn artifact(root: &Path, lua: &str, steps: &[Value]) {
    fs::write(root.join("Lua.log"), lua).unwrap();
    write_steps(root, steps);
    fs::write(
        root.join("result.json"),
        json!({"passed": true, "test_mode": null}).to_string(),
    )
    .unwrap();
}

pub fn completed_steps() -> Vec<Value> {
    let mut steps: Vec<_> = (0..4).map(|i| json!({"status": "OK", "action": format!("{i}/18 autoplay"), "detail": "autoplay completed 50 turns"})).collect();
    steps.extend([
        json!({"status": "OK", "action": "18/18 ui_lua", "detail": "return=T+201 surviving majors=22"}),
        json!({"status": "OK", "action": "judge", "detail": "8/8 PASS"}),
    ]);
    steps
}

pub fn golden_artifact(parent: &Path, name: &str) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/goldens")
        .join(name);
    let artifact = parent.join(name);
    fs::create_dir_all(&artifact).unwrap();
    for file in ["Lua.log", "steps.jsonl", "result.json"] {
        // The fixed expectations use LF bytes, independently of Git's checkout settings.
        fs::write(
            artifact.join(file),
            fs::read_to_string(source.join(file))
                .unwrap()
                .replace("\r\n", "\n"),
        )
        .unwrap();
    }
    artifact
}

pub fn golden_text(name: &str, file: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/goldens")
            .join(name)
            .join(file),
    )
    .unwrap()
    .replace("\r\n", "\n")
}

pub fn contains(issues: &Value, message: &str) {
    assert!(
        issues.as_array().unwrap().iter().any(|v| v == message),
        "missing {message:?} in {issues}"
    );
}

pub fn calendar() -> Value {
    use regex::Regex;
    let xml = repo_text("poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Calendar.xml");
    let year = Regex::new(r"<Year>(-?\d+)</Year>")
        .unwrap()
        .captures(&xml)
        .unwrap()[1]
        .parse::<i64>()
        .unwrap();
    let blocks = Regex::new(r"(?s)<GameSpeed_Turns>(.*?)</GameSpeed_Turns>").unwrap();
    let rows = Regex::new(r"(?s)<Row>(.*?)</Row>").unwrap();
    let speed = Regex::new(r"<GameSpeedType>(.*?)</GameSpeedType>").unwrap();
    let months = Regex::new(r"<MonthIncrement>(\d+)</MonthIncrement>").unwrap();
    let turns = Regex::new(r"<TurnsPerIncrement>(\d+)</TurnsPerIncrement>").unwrap();
    let block = blocks.captures(&xml).unwrap();
    let values: Vec<_> = rows
        .captures_iter(&block[1])
        .map(|r| {
            json!({
                "GameSpeedType": &speed.captures(&r[1]).unwrap()[1],
                "MonthIncrement": months.captures(&r[1]).unwrap()[1].parse::<i64>().unwrap(),
                "TurnsPerIncrement": turns.captures(&r[1]).unwrap()[1].parse::<i64>().unwrap()
            })
        })
        .collect();
    assert!(!values.is_empty());
    json!({"StartYear": year, "GameSpeed_Turns": values})
}

pub fn arbitration_lua(civs: &[String]) -> Lua {
    let lua = Lua::new();
    let rules = source_root()
        .join("poc/JAC_PoC_CourtArbitration/Scripts/JAC_CourtArbitration_Rules.lua")
        .to_string_lossy()
        .replace('\\', "/");
    let stubs = lua
        .load(include_str!("../fixtures/arbitration-stubs.lua"))
        .set_name("arbitration-stubs")
        .into_function()
        .unwrap();
    stubs
        .call::<()>((
            lua.to_value(&calendar()).unwrap(),
            lua.to_value(civs).unwrap(),
            16603,
            rules,
        ))
        .unwrap();
    lua.load(repo_text(
        "poc/JAC_PoC_CourtArbitration/Scripts/JAC_CourtArbitration.lua",
    ))
    .set_name("JAC_CourtArbitration")
    .exec()
    .unwrap();
    lua
}

pub fn exhausted_party_lua() -> Lua {
    let civs = [
        "CIVILIZATION_JAC_OUCHI",
        "CIVILIZATION_JAC_OTOMO",
        "CIVILIZATION_JAC_ASHIKAGA",
        "CIVILIZATION_JAC_MORI",
    ]
    .map(str::to_owned);
    let lua = arbitration_lua(&civs);
    lua.load("JAC_ARB.AI_EXHAUSTION_THRESHOLD=9999; JAC_ARB.AI_SHOGUN_THRESHOLD=9999; SET_WAR(0,1,true); START_TURN(1)").exec().unwrap();
    lua.load("JAC_ARB.AI_EXHAUSTION_THRESHOLD=6; JAC_ARB.AI_SHOGUN_THRESHOLD=3; Players[0].cities=1; Players[0].units=1; START_TURN(2); Game:SetProperty('JAC_ARB_AI_TEST',{shogun=2,a=0,b=1})").exec().unwrap();
    lua
}
