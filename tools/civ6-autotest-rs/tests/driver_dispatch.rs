mod support_driver;
use civ6_autotest_rs::{judge, scenario, Error};
use mlua::{Lua, Table};
use serde_json::{json, Value};
use std::{
    fs,
    sync::{Arc, Mutex},
};
use support_driver::{FakeBackend, Harness};

fn id(n: u8) -> String {
    format!("abcdef{n:02x}-1234-1234-1234-123456789abc")
}
fn graph_backend(
    edges: Vec<(u8, Vec<u8>)>,
    disabled: Vec<u8>,
    uppercase: bool,
) -> (FakeBackend, Arc<Mutex<Value>>) {
    let state = Arc::new(Mutex::new(Value::Null));
    let result = state.clone();
    let fake=FakeBackend::new().lua(move|context,code|{
        assert_eq!(context,"FrontEnd");let lua=Lua::new();let definitions=lua.create_table().unwrap();
        let spell=|n|if uppercase{id(n).to_uppercase()}else{id(n)};
        for(i,(n,deps))in edges.iter().enumerate(){let definition=lua.create_table().unwrap();definition.set("Id",spell(*n)).unwrap();definition.set("Enabled",!disabled.contains(n)).unwrap();definition.set("Dependencies",lua.create_sequence_from(deps.iter().map(|n|spell(*n))).unwrap()).unwrap();definitions.set(i+1,definition).unwrap();}
        lua.load(include_str!("fixtures/modding_stub.lua")).call::<()>(definitions).unwrap();let detail=lua.load(code).eval::<String>().map_err(|e|Error::lua(e.to_string()))?;
        let globals=lua.globals();let events=|key|globals.get::<Table>(key).unwrap().sequence_values::<String>().collect::<mlua::Result<Vec<_>>>().unwrap();let mut enabled=serde_json::Map::new();let mods=globals.get::<Table>("MODS").unwrap();for(n,_)in &edges{enabled.insert(id(*n),mods.get::<Table>(id(*n)).unwrap().get::<bool>("Enabled").unwrap().into());}
        *result.lock().unwrap()=json!({"disabled":events("DISABLED"),"enabled":enabled,"updates":globals.get::<i64>("UPDATES").unwrap()});Ok(detail)
    });
    (fake, state)
}

#[test]
fn test_transitive_dependents_are_disabled_first_once_and_reported() {
    let (fake, state) = graph_backend(
        vec![
            (1, vec![]),
            (2, vec![1]),
            (3, vec![2, 4]),
            (4, vec![1]),
            (5, vec![]),
            (6, vec![1]),
        ],
        vec![5, 6],
        true,
    );
    let mut h = Harness::new(json!({}), fake);
    let step = json!({"action":"enable_mods","ids":[id(5)],"disable_ids":[id(1),id(4),id(7)]});
    let detail = h.runner.run_step(&step).unwrap();
    let expected = [id(3), id(2), id(4), id(1)];
    assert_eq!(state.lock().unwrap()["disabled"], json!(expected));
    assert_eq!(
        detail,
        format!(
            "enabled/verified {}; disabled {}",
            id(5),
            expected.join(", ")
        )
    );
    assert_eq!(state.lock().unwrap()["enabled"][id(5)], true);
    assert_eq!(state.lock().unwrap()["enabled"][id(6)], false);
    assert_eq!(state.lock().unwrap()["updates"], 1);
    assert_eq!(h.calls().len(), 1);
    assert_eq!(h.calls()[0].0, "FrontEnd");
    assert_eq!(
        h.calls()[0].1,
        scenario::enable_mods_code(&[id(5)], &[id(1), id(4), id(7)])
    );
}

#[test]
fn test_no_disable_targets_leaves_unrelated_mods_enabled() {
    let (fake, state) = graph_backend(vec![(1, vec![]), (5, vec![1])], vec![5], false);
    let mut h = Harness::new(json!({}), fake);
    let detail = h
        .runner
        .run_step(&json!({"action":"enable_mods","ids":[id(5)]}))
        .unwrap();
    assert_eq!(detail, format!("enabled/verified {}; disabled none", id(5)));
    assert_eq!(state.lock().unwrap()["disabled"], json!([]));
    assert_eq!(state.lock().unwrap()["enabled"][id(1)], true);
}

#[test]
fn test_input_error_is_logged_as_step_failure() {
    let fake = FakeBackend::new().lua(|_, code| {
        if code.contains("Network.HostGame") {
            Err(Error::input("foreground lost"))
        } else {
            Ok("true".into())
        }
    });
    let mut h = Harness::new(
        json!({"name":"input failure","steps":[{"action":"start_game"}]}),
        fake,
    );
    h.runner.auto_restart = false;
    fs::remove_dir(&h.runner.output).unwrap();
    for name in judge::LOG_NAMES {
        fs::write(h.runner.logs.join(name), "").unwrap();
    }
    assert_eq!(h.runner.run().unwrap(), 1);
    let rows = h.jsonl("steps.jsonl");
    let last = rows.last().unwrap();
    assert_eq!(last["action"], "1/1 start_game");
    assert_eq!(last["status"], "FAIL");
    assert_eq!(last["detail"], "foreground lost");
}

#[test]
fn test_stale_art_stops_before_restart() {
    let mut fake = FakeBackend::new();
    fake.on_stale = Box::new(|| Ok(vec!["JAC_CustomCivs: JAC_Crests".into()]));
    let mut h = Harness::new(
        json!({"name":"stale art","steps":[{"action":"pause","seconds":0.1}]}),
        fake,
    );
    fs::remove_dir(&h.runner.output).unwrap();
    assert_eq!(h.runner.run().unwrap(), 1);
    assert!(h.calls().is_empty());
    assert!(!h.events().iter().any(|e| e == "launch"));
    let records = h.jsonl("steps.jsonl");
    assert_eq!(records.last().unwrap()["status"], "FAIL");
    let detail = records.last().unwrap()["detail"].as_str().unwrap();
    assert!(detail.contains("JAC_Crests"));
    assert!(detail.contains("rebuild"));
    assert!(!detail.contains("python"));
    assert!(!records
        .iter()
        .any(|r| r["action"].as_str().is_some_and(|a| a.starts_with("1/1"))));
}
