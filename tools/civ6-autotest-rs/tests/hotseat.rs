mod support_driver;
use civ6_autotest_rs::{
    scenario::{self, SessionType},
    Error,
};
use mlua::Lua;
use serde_json::{json, Value};
use support_driver::{FakeBackend, Harness};

fn configuration() -> Value {
    json!({"action":"configure","ruleset":"RULESET_EXPANSION_2","map_size":"MAPSIZE_DUEL","speed":"GAMESPEED_STANDARD","seed":620905,"players":[
        {"leader":"LEADER_CLEOPATRA","civ":"CIVILIZATION_EGYPT","human":true},
        {"leader":"LEADER_GILGAMESH","civ":"CIVILIZATION_SUMERIA","human":true}
    ]})
}

#[test]
fn two_humans_require_hotseat_and_unknown_modes_are_rejected() {
    let mut data = json!({"name":"two humans","steps":[configuration()]});
    assert!(scenario::validate_scenario(&data).is_err());
    data["session_type"] = "hotseat".into();
    scenario::validate_scenario(&data).unwrap();
    for mode in [json!(null), json!(true), json!("lan"), json!("HOTSEAT")] {
        data["session_type"] = mode;
        assert!(scenario::validate_scenario(&data).is_err());
    }
    data["session_type"] = "hotseat".into();
    data["steps"][0]["players"][1]["human"] = false.into();
    assert!(scenario::validate_scenario(&data).is_err());
    data["steps"] = json!([{"action":"autoplay","turns":1}]);
    assert!(scenario::validate_scenario(&data)
        .unwrap_err()
        .to_string()
        .contains("autoplay"));
}

#[test]
fn load_uses_hotseat_mode_before_resolving_the_engine_save_type() {
    let lua = Lua::new();
    lua.load(
        r#"
        sequence = {}; mode = 'single'; loaded = nil
        UI = {IsInFrontEnd = function() return true end}
        GameModeTypes = {HOTSEAT = 7}
        GameConfiguration = {SetToDefaults = function(value)
            assert(value == 7); mode = 'hotseat'; table.insert(sequence, 'mode')
        end}
        Network = {
            GetGameConfigurationSaveType = function()
                assert(mode == 'hotseat'); table.insert(sequence, 'type'); return 23
            end,
            LoadGame = function(save, server)
                assert(save.Name == 'mid-turn' and save.Type == 23 and server == 19)
                loaded = save; table.insert(sequence, 'load'); return true
            end
        }
        SaveLocations = {LOCAL_STORAGE = 5}; SaveDirectories = {DEFAULT = 8}
        ServerType = {SERVER_TYPE_HOTSEAT = 19}
    "#,
    )
    .exec()
    .unwrap();
    assert!(lua
        .load(scenario::load_code("mid-turn", SessionType::Hotseat))
        .eval::<bool>()
        .unwrap());
    assert_eq!(
        lua.load("return table.concat(sequence, ',')")
            .eval::<String>()
            .unwrap(),
        "mode,type,load"
    );
    lua.load("Network.LoadGame = function() return false end")
        .exec()
        .unwrap();
    assert!(lua
        .load(scenario::load_code("mid-turn", SessionType::Hotseat))
        .eval::<bool>()
        .is_err());
    assert_eq!(SessionType::Hotseat.save_folder(), "Multi");
    assert_eq!(SessionType::SinglePlayer.save_folder(), "Single");
}

#[test]
fn hotseat_handoff_does_not_wait_for_a_whole_round_or_end_twice() {
    let mut ended = false;
    let fake = FakeBackend::new()
        .states(|| Ok(vec!["InGame".into(), "PlayerChange".into()]))
        .lua(move |_, code| {
            if code == "return Game.GetCurrentGameTurn()" {
                return Ok("7".into());
            }
            if code == "return Game.GetLocalPlayer()" {
                return Ok("0".into());
            }
            if code.contains("UI.RequestAction(ActionTypes.ACTION_ENDTURN)") {
                assert!(!ended);
                ended = true;
            }
            if code.contains("local p = Players[id]") && code.contains("id ~= 0") {
                assert!(ended);
                assert!(code.contains("Game.GetCurrentGameTurn() >= 7"));
                assert!(code.contains("p:IsTurnActive()"));
            }
            if code.contains("tostring(Game.GetCurrentGameTurn()) .. '|' ..") {
                return Ok("7|1".into());
            }
            Ok("true".into())
        });
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    assert_eq!(
        h.runner.end_turn(30.0, None).unwrap(),
        "hotseat turn/player 7|0->7|1"
    );
    assert_eq!(
        h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
        1
    );
    assert_eq!(h.count_code("OnKeyUp_Return()"), 2);
}

#[test]
fn handoff_timeout_never_reissues_endturn() {
    let fake = FakeBackend::new()
        .states(|| Ok(vec!["InGame".into(), "PlayerChange".into()]))
        .lua(|_, code| {
            if code == "return Game.GetCurrentGameTurn()" {
                return Ok("7".into());
            }
            if code == "return Game.GetLocalPlayer()" {
                return Ok("0".into());
            }
            if code.contains("id ~= 0") {
                return Ok("false".into());
            }
            Ok("true".into())
        });
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    assert!(h.runner.end_turn(3.0, None).is_err());
    assert_eq!(
        h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
        1
    );
}

#[test]
fn password_error_stops_before_sending_endturn() {
    let fake = FakeBackend::new()
        .states(|| Ok(vec!["InGame".into(), "PlayerChange".into()]))
        .lua(|state, code| {
            if state == "PlayerChange" && code.contains("OnKeyUp_Return()") {
                return Err(Error::lua(
                    "password-protected hotseat needs manual start-turn",
                ));
            }
            Ok("true".into())
        });
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    assert!(h
        .runner
        .end_turn(3.0, None)
        .unwrap_err()
        .to_string()
        .contains("password-protected"));
    assert_eq!(h.count_code("ACTION_ENDTURN"), 0);
}

#[test]
fn configure_routes_the_explicit_session_mode() {
    let mut h = Harness::new(json!({"session_type":"hotseat"}), FakeBackend::new());
    h.runner.run_step(&configuration()).unwrap();
    assert!(h.calls()[0]
        .1
        .contains("SetToDefaults(GameModeTypes.HOTSEAT)"));
    assert!(h.calls()[0].1.contains("GameConfiguration.IsHotseat()"));
    assert_eq!(
        h.calls()[0]
            .1
            .matches("p:SetSlotStatus(SlotStatus.SS_TAKEN)")
            .count(),
        2
    );
}
