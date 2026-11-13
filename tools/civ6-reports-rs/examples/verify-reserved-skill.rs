//! Offline verification of the frozen T9 skill run, including the full snapshot.
use civ6_reports_rs::{persona_full::lua_return, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn require(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(why.into())
    }
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn complete_snapshot(before: &str, after: &str) -> Result<()> {
    require(
        before == after,
        "complete saved snapshot differs after restart/load",
    )?;
    require(
        before.len() == 166725
            && sha(before.as_bytes())
                == "6bb37f91139299f9591e9c531301a5162e789164cdbb39bcecb2120fe0c24bcc",
        "not the complete observed skill snapshot",
    )
}

#[derive(Debug)]
enum Node<'a> {
    Scalar(&'a str, &'a str),
    Table(Vec<(Node<'a>, Node<'a>)>),
}
impl<'a> Node<'a> {
    fn field(&self, key: &str) -> Result<&Self> {
        if let Self::Table(rows) = self {
            let mut found = rows
                .iter()
                .filter(|(k, _)| matches!(k,Self::Scalar("string",v) if *v==key));
            let result = &found.next().ok_or("missing snapshot field")?.1;
            require(found.next().is_none(), "duplicate snapshot field")?;
            Ok(result)
        } else {
            Err("snapshot field on scalar".into())
        }
    }
    fn number(&self) -> Result<i64> {
        match self {
            Self::Scalar("number", v) => Ok(v.parse()?),
            _ => Err("number required".into()),
        }
    }
}
// Lua #value is a UTF-8 byte length. Delimiters inside the scalar are data.
fn parse<'a>(input: &mut &'a str, depth: usize) -> Result<Node<'a>> {
    require(depth < 64, "snapshot nesting limit")?;
    if let Some(rest) = input.strip_prefix('{') {
        *input = rest;
        let mut rows = Vec::new();
        if let Some(rest) = input.strip_prefix('}') {
            *input = rest;
            return Ok(Node::Table(rows));
        }
        loop {
            let key = parse(input, depth + 1)?;
            *input = input
                .strip_prefix('=')
                .ok_or("snapshot key separator missing")?;
            let value = parse(input, depth + 1)?;
            rows.push((key, value));
            if let Some(rest) = input.strip_prefix('}') {
                *input = rest;
                break;
            }
            *input = input
                .strip_prefix(';')
                .ok_or("snapshot row separator missing")?;
        }
        Ok(Node::Table(rows))
    } else {
        let (kind, rest) = input.split_once(':').ok_or("scalar kind missing")?;
        require(
            matches!(kind, "string" | "number" | "boolean" | "nil"),
            "invalid scalar kind",
        )?;
        let (length, rest) = rest.split_once(':').ok_or("scalar length missing")?;
        let n: usize = length.parse()?;
        let value = rest.get(..n).ok_or("truncated scalar or split UTF-8")?;
        *input = &rest[n..];
        Ok(Node::Scalar(kind, value))
    }
}
fn snapshot(value: &str) -> Result<Node<'_>> {
    let mut remaining = value;
    let node = parse(&mut remaining, 0)?;
    require(remaining.is_empty(), "trailing snapshot bytes")?;
    Ok(node)
}
fn outcome<'a>(rows: &'a [Value], step: usize, action: &str) -> Result<&'a str> {
    let label = format!("{step}/42 {action}");
    let found: Vec<_> = rows
        .iter()
        .filter(|r| r["action"] == label && r["status"] == "OK")
        .collect();
    require(found.len() == 1, "missing or repeated successful step")?;
    Ok(found[0]["detail"].as_str().ok_or("step detail missing")?)
}
fn value(rows: &[Value], step: usize) -> Result<&str> {
    lua_return(outcome(rows, step, "ui_lua")?, "InGame")
}
fn sends(scene: &Value, rows: &[Value]) -> Result<()> {
    let mut expected = vec![json!({"action":"restart_game","timeout":420})];
    expected.extend(
        scene["steps"]
            .as_array()
            .ok_or("scenario steps missing")?
            .iter()
            .cloned(),
    );
    require(
        expected.len() == 42,
        "not the original 41 operations plus preparation",
    )?;
    require(rows.len() == 87, "unexpected raw event count")?;
    for name in ["start", "judge", "ingame_unblock"] {
        require(
            rows.iter()
                .filter(|r| r["action"] == name && r["status"] == "OK")
                .count()
                == 1,
            "missing or duplicate host event",
        )?;
    }
    let starts: Vec<_> = rows.iter().filter(|r| r["status"] == "START").collect();
    require(starts.len() == 42, "missing or additional START")?;
    for (i, (step, row)) in expected.iter().zip(starts).enumerate() {
        let action = step["action"].as_str().ok_or("action missing")?;
        require(
            row["action"] == format!("{}/42 {action}", i + 1),
            "START order differs",
        )?;
        let sent: Value = serde_json::from_str(row["detail"].as_str().ok_or("raw send missing")?)?;
        require(sent == *step, "raw operation differs from frozen scenario")?;
        outcome(rows, i + 1, action)?;
    }
    require(
        rows.iter()
            .all(|r| matches!(r["status"].as_str(), Some("START" | "OK"))),
        "failed or unknown raw row",
    )
}
fn verify(scene: &Value, rows: &[Value], result: &Value, save: &Path) -> Result<Value> {
    sends(scene, rows)?;
    require(
        result["passed"] == true
            && result["completed_steps"] == 42
            && result["failure"].is_null()
            && result["test_mode"].is_null(),
        "run not a complete normal-mode success",
    )?;
    let checks = result["checks"].as_array().ok_or("checks missing")?;
    let names = [
        "Generic: JAC Modding.log errors",
        "Generic: JAC Database.log errors",
        "Generic: [JAC_POC] errors",
        "Generic: JAC Runtime Error",
        "Generic: JAC missing texture",
        "Scenario: all Lua errors",
    ];
    require(
        checks.len() == 6
            && checks
                .iter()
                .zip(names)
                .all(|(c, n)| c["passed"] == true && c["name"] == n),
        "six original judges not passed",
    )?;
    let initial = snapshot(value(rows, 6)?)?;
    require(
        initial.field("cost")?.number()? == 8
            && initial.field("reward")?.number()? == 2
            && initial.field("requiredRealTurns")?.number()? == 2,
        "earning price/reward differs",
    )?;
    require(
        initial.field("state")?.field("turn")?.number()? == 9
            && initial.field("state")?.field("merit")?.number()? == 4,
        "original turn9 state differs",
    )?;
    for (step, turn, merit) in [
        (12, 10, 6),
        (15, 11, 8),
        (18, 11, 8),
        (21, 11, 8),
        (22, 11, 8),
        (26, 11, 0),
        (35, 11, 0),
        (40, 11, 0),
    ] {
        let state = snapshot(value(rows, step)?)?;
        require(
            state.field("turn")?.number()? == turn && state.field("merit")?.number()? == merit,
            "ordinary earning/purchase state differs",
        )?;
    }
    for step in [10, 13] {
        require(
            value(rows, step)? == "one ordinary UI end-turn requested; never resent by wait",
            "ordinary turn not acknowledged",
        )?;
    }
    for step in [16, 19] {
        require(
            value(rows, step)? == "enough earned merit; no turn requested",
            "extra earning turn requested",
        )?;
    }
    let before = value(rows, 35)?;
    let after = value(rows, 40)?;
    complete_snapshot(before, after)?;
    let final_state = snapshot(after)?;
    let stable = final_state.field("stable")?;
    for (field, expected) in [
        ("owner", 0),
        ("source", 1),
        ("index", 6),
        ("hash", 912436062),
        ("paid", 2),
        ("spent", 2),
    ] {
        require(
            stable.field(field)?.number()? == expected,
            "saved person/account binding differs",
        )?;
    }
    require(
        final_state.field("city")?.field("id")?.number()? == 131073
            && final_state.field("city")?.field("owner")?.number()? == 0,
        "assigned city changed",
    )?;
    require(
        final_state
            .field("stable")?
            .field("skills")?
            .field("LEADER_JAC_DATE_TANEMUNE_ENVOYS_1")?
            .field("value")?
            .number()?
            == 1,
        "purchased skill not saved",
    )?;
    require(
        value(rows, 26)? == before,
        "purchase/rejection changed complete saved state",
    )?;
    for step in [30, 33] {
        require(
            value(rows, step)? == "rejection kept complete state unchanged",
            "rejection state differs",
        )?;
    }
    require(
        lua_return(outcome(rows, 34, "ui_lua")?, "GameCore_Tuner")?
            == "actual active city effects and gameplay settlement marker verified",
        "Core effects not confirmed",
    )?;
    let saved = outcome(rows, 36, "save")?;
    let name = Path::new(saved).file_name().ok_or("save name missing")?;
    require(
        save.file_name() == Some(name),
        "physical save does not match raw SaveGame result",
    )?;
    require(
        outcome(rows, 38, "load")?
            == format!(
                "loaded {}",
                Path::new(saved).file_stem().unwrap().to_string_lossy()
            ),
        "reload is not the actual checkpoint",
    )?;
    let save_bytes = fs::read(save)?;
    require(
        sha(&save_bytes) == "cb6f4004136f0e4b41b9a47d90e8f1472e9c17d8ad5d2f28b413b6cdf06da33d",
        "physical checkpoint changed",
    )?;
    Ok(
        json!({"skill_verified":true,"returned_spy":false,"product_acceptance":false,"steps":42,"judges":6,
        "turns":[9,10,11],"merit":[4,6,8,0],"skill_price":8,"ordinary_reward":2,
        "node":"LEADER_JAC_DATE_TANEMUNE_ENVOYS_1","persona":{"owner":0,"source":1,"index":6,"hash":912436062,"paid":2,"spent":2},
        "city":{"id":131073,"owner":0},"core_effects_verified_at_step":34,
        "complete_snapshot_steps":[26,35,40],"snapshot_bytes":before.len(),"snapshot_sha256":sha(before.as_bytes()),
        "save_bytes":save_bytes.len(),"save_sha256":sha(&save_bytes)}),
    )
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    require(
        args.len() == 4,
        "usage: verify-reserved-skill SCENE ARTIFACTS PHYSICAL_SAVE OUTPUT",
    )?;
    let scene_bytes = fs::read(&args[0])?;
    require(
        sha(&scene_bytes) == "36a7b7afabd0be3b5b371373aae3dffb1a20ef870c6edb8b555a04a5601fd617",
        "not the frozen T9 scenario",
    )?;
    let dir = Path::new(&args[1]);
    let raw = fs::read_to_string(dir.join("steps.jsonl"))?;
    let rows: Vec<Value> = raw
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let result_bytes = fs::read(dir.join("result.json"))?;
    let mut report = verify(
        &serde_json::from_slice(&scene_bytes)?,
        &rows,
        &serde_json::from_slice(&result_bytes)?,
        Path::new(&args[2]),
    )?;
    report["steps_sha256"] = json!(sha(raw.as_bytes()));
    report["result_sha256"] = json!(sha(&result_bytes));
    fs::write(&args[3], serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}
#[test]
fn typed_lengths_do_not_confuse_embedded_delimiters_or_types() {
    let n = snapshot("{string:1:x=string:5:a;=}{;string:1:n=number:1:4}").unwrap();
    assert!(n.field("x").unwrap().number().is_err());
    assert_eq!(n.field("n").unwrap().number().unwrap(), 4);
    assert!(snapshot("{string:1:n=number:1:4}garbage").is_err());
    assert!(snapshot("{string:1:n=number:3:4}").is_err());
    assert!(snapshot("{string:1:n=number:1:4;string:1:n=number:1:5}")
        .unwrap()
        .field("n")
        .is_err());
}

#[test]
fn equal_syntactically_valid_shortened_snapshots_cannot_prove_completeness() {
    let shortened = "{string:5:merit=number:1:0;string:4:turn=number:2:11}";
    let parsed = snapshot(shortened).unwrap();
    assert_eq!(parsed.field("merit").unwrap().number().unwrap(), 0);
    assert_eq!(parsed.field("turn").unwrap().number().unwrap(), 11);
    assert!(complete_snapshot(shortened, shortened).is_err());
    assert!(complete_snapshot(shortened, "different").is_err());
}
#[test]
fn raw_send_modification_extra_send_and_missing_completion_are_rejected() {
    let scene = json!({"steps":(0..41).map(|_|json!({"action":"ui_lua","code":"return true","state_name":"InGame"})).collect::<Vec<_>>()});
    let mut ops = vec![json!({"action":"restart_game","timeout":420})];
    ops.extend(scene["steps"].as_array().unwrap().iter().cloned());
    let mut rows = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        let label = format!("{}/42 {}", i + 1, op["action"].as_str().unwrap());
        rows.push(json!({"action":label,"status":"START","detail":op.to_string()}));
        rows.push(json!({"action":label,"status":"OK","detail":"ok"}));
    }
    for action in ["start", "judge", "ingame_unblock"] {
        rows.push(json!({"action":action,"status":"OK","detail":"ok"}));
    }
    sends(&scene, &rows).unwrap();
    let mut bad = rows.clone();
    bad[2]["detail"] = json!(r#"{"action":"ui_lua","code":"mutate()"}"#);
    assert!(sends(&scene, &bad).is_err());
    let mut bad = rows.clone();
    bad.push(rows[0].clone());
    assert!(sends(&scene, &bad).is_err());
    rows.remove(83);
    assert!(sends(&scene, &rows).is_err());
}
