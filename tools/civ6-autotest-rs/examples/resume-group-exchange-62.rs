//! Single historical checkpoint recovery, not a general retry facility.
//! Original live04/source 8ca69057: nine completed steps, then wait failure.
#[allow(dead_code)]
#[path = "group-exchange-62.rs"]
mod host;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::Path};

const SAVE_NAME: &str =
    "JAC-Enemy-Neutralization-b4daf733336c42d682acf021ce8b2dd0-0000000000000001";
const SAVE_SHA: &str = "58a796ad65643de332ac2df703fcff0d16b4adc2ed09040f6f31cc2d5ee40253";
const TRACE_SHA: &str = "969d4256f8ecd0c42b8b0b09ab8b7654c008f870e1f4f96a69d5156a94b22ac4";
const SCENE_SHA: &str = "c5688b2e329131e76f4f069eb9963649752c4ab3b614c2448fdc55bcd116217e";
const LUA_SHA: &str = "9719791178e66ca516014651cc1db6ef4179e08e7891d23f47fdf3dd334ad8bb";
const RESULT_SHA: &str = "9b3514b0526077a69eb46a1ace861423a513c85484fafa723290937dcd00b378";
const PREFIX: &str = include_str!("../tests/fixtures/group-exchange-live04/prefix.json");
const TOKEN: &str = "live04-969d4256-b4daf733-58a796ad";
const SKILL_NAME: &str = "JAC-Skill-Failure-Turn9-95b8be0e";
const SKILL_SAVE_SHA: &str = "95b8be0eee90eae2c58cedf7836c6089b83e9ca7059ee8758227003470b59090";
const SKILL_SCENE_SHA: &str = "92e546605630b0ef50f0537f4f66616d1f7001b3f71efc5dd97339ab5afde296";

fn pin(bytes: &[u8], expected: &str) -> Result<(), Box<dyn Error>> {
    if format!("{:x}", Sha256::digest(bytes)) != expected {
        return Err("historical evidence SHA256 mismatch".into());
    }
    Ok(())
}

// Inspect every row, not just successful rows or a substring search for send.
// The immutable prefix includes the complete installed Lua and Core reservation.
fn verify_trace(bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let rows: Vec<Value> = std::str::from_utf8(bytes)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let prefix: Vec<Value> = serde_json::from_str(PREFIX)?;
    if rows.len() != 22 || rows[0]["action"] != "start" || rows[0]["status"] != "OK" {
        return Err("incomplete or additional raw events".into());
    }
    for (i, expected) in prefix.iter().enumerate() {
        let start = &rows[1 + i * 2];
        let action = format!(
            "{}/69 {}",
            i + 1,
            expected["action"].as_str().ok_or("action missing")?
        );
        if start["action"] != action
            || start["status"] != "START"
            || serde_json::from_str::<Value>(start["detail"].as_str().ok_or("missing raw send")?)?
                != *expected
        {
            return Err(format!("original send {} differs", i + 1).into());
        }
        if i < 9 {
            let end = &rows[2 + i * 2];
            if end["action"] != action || end["status"] != "OK" {
                return Err("completed prefix differs".into());
            }
        }
    }
    let saved = rows[14]["detail"]
        .as_str()
        .ok_or("save completion missing")?;
    if !saved
        .replace('\\', "/")
        .ends_with(&format!("/Single/{SAVE_NAME}.Civ6Save"))
        || rows[16]["detail"] != "ran Lua in InGame; return=ARMED"
        || rows[18]["detail"] != "ran Lua in InGame; return=SESSION_REQUESTED"
        || rows[20]["action"] != "ingame_unblock"
        || rows[20]["status"] != "OK"
        || rows[21]["action"] != "10/69 wait_for"
        || rows[21]["status"] != "FAIL"
    {
        return Err("save/session/failure boundary differs".into());
    }
    Ok(())
}

fn scenario() -> Value {
    let mut scene = host::scenario();
    scene["name"] =
        json!("Resume audited live04 reservation; first native quote and observed return");
    let steps = scene["steps"].as_array_mut().unwrap();
    steps[0] = json!({"action":"load","name":SAVE_NAME,"exact_name":true,"timeout":300,"save_sha256":SAVE_SHA,"resume_once":true});
    // Replace Prepare, Core reserve and save with UI-only restoration.
    steps.splice(
        3..6,
        [json!({"action":"ui_lua","state_name":"InGame",
        "preserve_diplomacy_session":true,
        "code":format!("return JAC_GroupExchange548.ResumeReserved('{TOKEN}')")})],
    );
    let last = steps.len() - 1;
    steps.insert(last,json!({"action":"ui_lua","state_name":"GameCore_Tuner",
        "code":"local p=assert(Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62')); assert(p.stage=='RETURNED','native return unresolved; preserve checkpoint'); return 'RETURNED'"}));
    scene
}

fn pin_skill_input(scene: &mut Value) -> Result<(), Box<dyn Error>> {
    let steps = scene["steps"].as_array_mut().ok_or("skill steps missing")?;
    let load = steps
        .iter_mut()
        .find(|s| s["action"] == "load")
        .ok_or("skill input load missing")?;
    if load["name"] != SKILL_NAME || load["exact_name"] != true || load.get("checkpoint").is_some()
    {
        return Err("skill input is not the preserved turn9 save".into());
    }
    load["save_sha256"] = json!(SKILL_SAVE_SHA);
    load["resume_once"] = json!(true);
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 {
        return Err(
            "usage: resume-group-exchange-62 ORIGINAL_ARTIFACTS ORIGINAL_SCENE ACTUAL_SAVE OUTPUT"
                .into(),
        );
    }
    if args[0] == "--skill-turn9" {
        let raw = fs::read(&args[1])?;
        pin(&raw, SKILL_SCENE_SHA)?;
        pin(&fs::read(&args[2])?, SKILL_SAVE_SHA)?;
        let mut scene: Value = serde_json::from_slice(&raw)?;
        pin_skill_input(&mut scene)?;
        civ6_autotest_rs::scenario::validate_scenario(&scene)?;
        fs::write(&args[3], serde_json::to_vec_pretty(&scene)?)?;
        println!("VERIFIED untouched T9 skill scene and input; original 41 operations retained");
        return Ok(());
    }
    let artifacts = Path::new(&args[0]);
    let raw = fs::read(artifacts.join("steps.jsonl"))?;
    pin(&raw, TRACE_SHA)?;
    verify_trace(&raw)?;
    pin(&fs::read(artifacts.join("Lua.log"))?, LUA_SHA)?;
    pin(&fs::read(artifacts.join("result.json"))?, RESULT_SHA)?;
    pin(&fs::read(&args[1])?, SCENE_SHA)?;
    pin(&fs::read(&args[2])?, SAVE_SHA)?;
    let scene = scenario();
    civ6_autotest_rs::scenario::validate_scenario(&scene)?;
    fs::write(&args[3], serde_json::to_vec_pretty(&scene)?)?;
    println!("VERIFIED live04: all 10 START records, 9 OK, terminal wait FAIL; EQUALIZE/ACCEPT not started; save {SAVE_SHA}");
    Ok(())
}

#[test]
fn skill_pin_changes_only_first_load_and_keeps_post_purchase_checkpoint_load() {
    let original = json!({"steps":[{"action":"load","name":SKILL_NAME,"exact_name":true},
      {"action":"ui_lua","code":"original skill purchase"},
      {"action":"load","name":"JAC-SP-Normal-Merit-Skill","exact_name":true,"checkpoint":true}]});
    let mut scene = original.clone();
    pin_skill_input(&mut scene).unwrap();
    assert_eq!(scene["steps"][0]["save_sha256"], SKILL_SAVE_SHA);
    assert_eq!(scene["steps"][0]["resume_once"], true);
    scene["steps"][0]
        .as_object_mut()
        .unwrap()
        .remove("save_sha256");
    scene["steps"][0]
        .as_object_mut()
        .unwrap()
        .remove("resume_once");
    assert_eq!(scene, original);
    scene["steps"][0]["name"] = json!("older-turn8");
    assert!(pin_skill_input(&mut scene).is_err());
}

#[test]
fn historical_trace_is_pinned_and_all_sends_are_checked() {
    let raw = include_bytes!("../tests/fixtures/group-exchange-live04/steps.jsonl");
    pin(raw, TRACE_SHA).unwrap();
    verify_trace(raw).unwrap();
    let rows: Vec<Value> = std::str::from_utf8(raw)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    for index in 0..rows.len() {
        let mut mutated = rows.clone();
        mutated.remove(index);
        let text = mutated
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            verify_trace(text.as_bytes()).is_err(),
            "missing row {index}"
        );
    }
    for index in (1..20).step_by(2) {
        let mut mutated = rows.clone();
        mutated[index]["detail"] = json!(
            r#"{"action":"ui_lua","state_name":"InGame","code":"return JAC_GroupExchange548.EqualizeOnce()"}"#
        );
        let text = mutated
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            verify_trace(text.as_bytes()).is_err(),
            "substituted send {index}"
        );
    }
    let mut extra = raw.to_vec();
    extra.extend_from_slice(b"\n{\"action\":\"11/69 ui_lua\",\"status\":\"START\"}\n");
    assert!(verify_trace(&extra).is_err());
    assert!(pin(&extra, TRACE_SHA).is_err());
    for expected in [SAVE_SHA, SCENE_SHA, LUA_SHA, RESULT_SHA] {
        assert!(pin(b"changed", expected).is_err());
    }
}

#[test]
fn resume_has_no_prepare_or_reservation_write_and_stops_on_unknown_return() {
    let s = scenario();
    let steps = s["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 25);
    assert_eq!(steps[0]["save_sha256"], SAVE_SHA);
    assert_eq!(
        steps[3]["code"],
        format!("return JAC_GroupExchange548.ResumeReserved('{TOKEN}')")
    );
    assert!(steps[4]["code"]
        .as_str()
        .unwrap()
        .contains("ArmAfterSave()"));
    assert!(steps[5]["code"].as_str().unwrap().contains("OpenSession()"));
    assert!(steps[23]["code"]
        .as_str()
        .unwrap()
        .contains("p.stage=='RETURNED'"));
    civ6_autotest_rs::scenario::validate_scenario(&s).unwrap();
}
