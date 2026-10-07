use mlua::Lua;
use serde_json::Value;

fn cycle() -> Value {
    serde_json::from_str(include_str!(
        "../../civ6-autotest/scenarios/reliability-cycle.json"
    ))
    .unwrap()
}

#[test]
fn stopped_mod_turn_handler_cannot_pass_after_the_base_game_advances() {
    for active_handler in [false, true] {
        let lua = Lua::new();
        lua.load("turn=10; saved={FirstTurn=2,LastTurn=10,TurnsSeen=4}; Game={GetCurrentGameTurn=function() return turn end,GetProperty=function() return saved end}; GameInfo={GlobalParameters={JAC_POC_BUNDLE0_VERSION={Value=1}}}").exec().unwrap();
        let data = cycle();
        lua.load(data["steps"][3]["code"].as_str().unwrap())
            .eval::<String>()
            .unwrap();
        lua.load(if active_handler {
            "turn=11; saved.LastTurn=11; saved.TurnsSeen=5"
        } else {
            "turn=11"
        })
        .exec()
        .unwrap();
        let result = lua
            .load(data["steps"][5]["code"].as_str().unwrap())
            .eval::<String>();
        if active_handler {
            assert_eq!(result.unwrap(), "11|2|11|5");
        } else {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("turn handler did not advance"));
        }
    }
}

#[test]
fn malformed_restored_mod_state_cannot_match_the_saved_snapshot() {
    let lua = Lua::new();
    lua.load("Game={GetCurrentGameTurn=function() return 11 end,GetProperty=function() return {FirstTurn=2,LastTurn=11,TurnsSeen='5'} end}").exec().unwrap();
    let data = cycle();
    let error = lua
        .load(data["steps"][9]["code"].as_str().unwrap())
        .eval::<String>()
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("invalid minimal mod state after reload"));
}
