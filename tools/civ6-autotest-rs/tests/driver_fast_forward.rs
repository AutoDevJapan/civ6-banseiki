mod support_driver;
use civ6_autotest_rs::{runner::AutoplayOptions, scenario, Error, ErrorKind};
use serde_json::{json, Value};
use std::{
    fs,
    sync::{Arc, Mutex},
};
use support_driver::{FakeBackend, Harness};

struct Engine {
    turn: i64,
    era: usize,
    boundaries: Vec<i64>,
    seen: Vec<i64>,
    skipped: i64,
    target: i64,
    active: bool,
    rule: String,
    multiplayer: bool,
    plays: usize,
}
fn engine_harness(source: Value, boundaries: Vec<i64>) -> (Harness, Arc<Mutex<Engine>>) {
    let state = Arc::new(Mutex::new(Engine {
        turn: 1,
        era: 0,
        boundaries,
        seen: vec![],
        skipped: 0,
        target: 1,
        active: false,
        rule: "RULESET_JAC_BANSEIKI".into(),
        multiplayer: false,
        plays: 0,
    }));
    let game = state.clone();
    let fake=FakeBackend::new().lua(move|context,code|{
        if code.contains("tostring(AutoplayManager.IsActive())") {let mut g=game.lock().unwrap();if g.active{g.turn+=1;if g.turn==g.boundaries[g.era]{let reached=g.turn;g.seen.push(reached);g.era+=1;}g.active=g.turn<g.target;}return Ok(format!("{}|{}|{}|{}|0",g.turn,g.active,!g.active,g.target-g.turn));}
        let lua=mlua::Lua::new();
        let current=game.clone();lua.globals().set("native_current",lua.create_function(move|_,()|Ok(current.lock().unwrap().turn)).unwrap()).unwrap();
        let boundary=game.clone();lua.globals().set("native_end",lua.create_function(move|_,()|{let g=boundary.lock().unwrap();Ok(g.boundaries[g.era])}).unwrap()).unwrap();
        let setter=game.clone();lua.globals().set("native_set",lua.create_function(move|_,target:i64|{let mut g=setter.lock().unwrap();assert!(target<g.boundaries[g.era],"jump crossed an era boundary");g.skipped+=target-g.turn;g.turn=target;Ok(())}).unwrap()).unwrap();
        let turns=game.clone();lua.globals().set("native_turns",lua.create_function(move|_,count:i64|{assert!(count<=2,"era-boundary autoplay must preserve at most two engine turns");let mut g=turns.lock().unwrap();g.target=g.turn+count;g.plays+=1;Ok(())}).unwrap()).unwrap();
        let active=game.clone();lua.globals().set("native_active",lua.create_function(move|_,value:bool|{active.lock().unwrap().active=value;Ok(())}).unwrap()).unwrap();
        let isactive=game.clone();lua.globals().set("native_is_active",lua.create_function(move|_,()|Ok(isactive.lock().unwrap().active)).unwrap()).unwrap();
        lua.load("Game={GetCurrentGameTurn=native_current,SetCurrentGameTurn=native_set,GetLocalPlayer=function() return 0 end,GetEras=function() return {GetCurrentEraMinimumEndTurn=native_end,GetCurrentEraMaximumEndTurn=native_end,GetNextEraCountdown=function() return -1 end} end};PlayerTypes={NONE=-1};Players={[0]={IsAlive=function() return true end}};LuaEvents={AutoPlayEnd={Add=function() end,Remove=function() end}};AutoplayManager={IsActive=native_is_active,SetTurns=native_turns,SetActive=native_active,SetReturnAsPlayer=function() end,SetObserveAsPlayer=function() end}").exec().unwrap();
        if context!="GameCore_Tuner"{let g=game.lock().unwrap();let rule=g.rule.clone();let multiplayer=g.multiplayer;drop(g);let config=lua.create_table().unwrap();config.set("GetRuleSet",lua.create_function(move|_,()|Ok(rule.clone())).unwrap()).unwrap();config.set("IsAnyMultiplayer",lua.create_function(move|_,()|Ok(multiplayer)).unwrap()).unwrap();lua.globals().set("GameConfiguration",config).unwrap();}
        let value=lua.load(code).eval::<mlua::Value>().map_err(|e|Error::lua(e.to_string()))?;
        Ok(match value{mlua::Value::Nil=>"nil".into(),mlua::Value::Boolean(v)=>v.to_string(),mlua::Value::Integer(v)=>v.to_string(),mlua::Value::Number(v)=>v.to_string(),mlua::Value::String(s)=>s.to_str().unwrap().to_string(),v=>panic!("unexpected Lua value {v:?}")})
    });
    (Harness::new(source, fake), state)
}

#[test]
fn test_fast_forward_runs_engine_jump_with_scope_checks_and_keeps_source_intact() {
    let source = json!({"name":"test","steps":[{"action":"autoplay","turns":64,"timeout":14400}]});
    let result = scenario::fast_forward_scenario(&source);
    assert_eq!(source["steps"][0]["turns"], 64);
    assert_eq!(
        result["steps"],
        json!([{"action":"autoplay","turns":64,"timeout":14400,"fast_forward":true}])
    );
    assert_eq!(
        result["test_mode"],
        json!({"skipped_turns":0,"continuous_performance_valid":false})
    );
    let (mut h, game) = engine_harness(result, vec![66, 73, 110]);
    h.runner
        .fast_forward_autoplay(64, 10.0, AutoplayOptions::default())
        .unwrap();
    assert_eq!(game.lock().unwrap().turn, 65);
    assert_eq!(h.runner.scenario["test_mode"]["skipped_turns"], 62);
    assert_eq!(game.lock().unwrap().plays, 1);
    for (rule, multiplayer, message) in [
        ("RULESET_STANDARD", false, "test ruleset required"),
        ("RULESET_JAC_BANSEIKI", true, "single-player test required"),
    ] {
        let mut g = game.lock().unwrap();
        g.rule = rule.into();
        g.multiplayer = multiplayer;
        drop(g);
        let e = h
            .runner
            .fast_forward_autoplay(64, 10.0, AutoplayOptions::default())
            .unwrap_err();
        assert_eq!(e.kind, ErrorKind::LuaExecution);
        assert!(e.message.contains(message));
        assert_eq!(game.lock().unwrap().turn, 65);
    }
}

#[test]
fn test_fast_forward_processes_every_era_boundary_before_skipping_again() {
    for (turns, boundaries) in [
        (17, (3..24).step_by(2).collect::<Vec<_>>()),
        (300, vec![66, 73, 110, 119, 178, 189, 246, 271, 334]),
    ] {
        let source = scenario::fast_forward_scenario(
            &json!({"name":"boundaries","steps":[{"action":"autoplay","turns":turns}]}),
        );
        let (mut h, game) = engine_harness(source, boundaries.clone());
        h.runner
            .fast_forward_autoplay(turns, 1000.0, AutoplayOptions::default())
            .unwrap();
        let g = game.lock().unwrap();
        assert_eq!(g.turn, turns + 1);
        assert_eq!(
            g.seen,
            boundaries
                .into_iter()
                .filter(|b| *b <= turns + 1)
                .collect::<Vec<_>>()
        );
        assert_eq!(h.runner.scenario["test_mode"]["skipped_turns"], g.skipped);
        assert_eq!(g.skipped == 0, turns == 17);
        let old = g.turn;
        drop(g);
        game.lock().unwrap().multiplayer = true;
        assert!(h
            .runner
            .fast_forward_autoplay(10, 10.0, AutoplayOptions::default())
            .unwrap_err()
            .message
            .contains("single-player test required"));
        assert_eq!(game.lock().unwrap().turn, old);
    }
}

#[test]
fn test_lost_jump_acknowledgment_marks_count_unknown_and_never_replays() {
    let source = scenario::fast_forward_scenario(
        &json!({"name":"lost ack","steps":[{"action":"autoplay","turns":10}]}),
    );
    let applied = Arc::new(Mutex::new(1));
    let current = applied.clone();
    let fake = FakeBackend::new().lua(move |state, code| {
        if state == "GameCore_Tuner" {
            *current.lock().unwrap() = 9;
            return Err(Error::unknown("fixture applied, acknowledgment lost"));
        }
        Ok(if code.contains("local eras") {
            "1|100|100|-1"
        } else {
            "true"
        }
        .into())
    });
    let mut h = Harness::new(source, fake);
    assert!(h
        .runner
        .fast_forward_autoplay(10, 10.0, AutoplayOptions::default())
        .unwrap_err()
        .message
        .contains("acknowledgment lost"));
    assert_eq!(h.calls().len(), 3);
    assert_eq!(*applied.lock().unwrap(), 9);
    h.runner.write_result().unwrap();
    let result: Value =
        serde_json::from_slice(&fs::read(h.runner.output.join("result.json")).unwrap()).unwrap();
    assert!(result["test_mode"]["skipped_turns"].is_null());
    assert_eq!(result["test_mode"]["acknowledged_skipped_turns"], 0);
    assert_eq!(
        result["test_mode"]["unconfirmed_jump"],
        json!({"from":1,"to":9})
    );
}

#[test]
fn test_run_rejects_invalid_metadata_even_without_a_fast_forward_step() {
    for action in [
        json!({"action":"restart_game"}),
        json!({"action":"pause","seconds":0.1}),
    ] {
        let mut h = Harness::new(
            json!({"name":"invalid entry","test_mode":{},"steps":[action]}),
            FakeBackend::new(),
        );
        h.runner.output = h.root.path().join("output-not-created");
        assert!(h
            .runner
            .run()
            .unwrap_err()
            .message
            .contains("test_mode needs"));
        assert!(h.calls().is_empty());
        assert!(h.events().is_empty());
        assert!(!h.runner.output.exists());
    }
}

#[test]
fn test_invalid_test_mode_is_rejected_before_any_game_call() {
    for mode in [
        Value::Null,
        json!({}),
        json!({"skipped_turns":-1,"continuous_performance_valid":false}),
        json!({"skipped_turns":true,"continuous_performance_valid":false}),
        json!({"skipped_turns":"0","continuous_performance_valid":false}),
        json!({"skipped_turns":0,"continuous_performance_valid":true}),
    ] {
        let data = json!({"name":"invalid","test_mode":mode,"steps":[{"action":"autoplay","turns":10,"fast_forward":true}]});
        assert!(scenario::validate_scenario(&data)
            .unwrap_err()
            .message
            .contains("test_mode needs"));
        let mut h = Harness::new(data, FakeBackend::new());
        assert!(h
            .runner
            .fast_forward_autoplay(10, 10.0, AutoplayOptions::default())
            .unwrap_err()
            .message
            .contains("test_mode needs"));
        assert!(h.calls().is_empty());
    }
}
