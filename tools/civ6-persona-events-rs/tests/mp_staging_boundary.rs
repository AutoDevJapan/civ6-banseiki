use civ6_autotest_rs::hotseat_bridge;
use civ6_persona_events_rs::{mp_staging as cap, mp_staging_boundary as boundary};
use mlua::{Lua, Table};
use serde_json::Value;
const NONCE: &str = "1234567890abcdef1234567890abcdef";
fn templates() -> (Value, Value) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../civ6-autotest/scenarios");
    let load = |name| serde_json::from_slice(&std::fs::read(root.join(name)).unwrap()).unwrap();
    (
        load("leader-persona-15km.json"),
        load("multiplayer-banseiki-hotseat-growth.json"),
    )
}
fn world() -> Lua {
    let (normal, hotseat) = templates();
    let s = boundary::build(&normal, &hotseat, NONCE).unwrap();
    let lua = Lua::new();
    lua.load(format!(r#"
logs={{}};print=function(s)logs[#logs+1]=s end
nonce='{NONCE}';map='{}';hotseat=true;host=true;hidden=false;calls=0
GameConfiguration={{IsHotseat=function()return hotseat end,GetRuleSet=function()return 'RULESET_JAC_BANSEIKI' end,GetValue=function(k)if k=='JAC_HOTSEAT_BRIDGE_NONCE'then return nonce end end}}
MapConfiguration={{GetValue=function()return map end}};Network={{IsGameHost=function()return host end}};ContextPtr={{IsHidden=function()return hidden end}}
status={{version='staging22/v1',stock='{}',visible=true,scopeCap=22,effectiveMax=22,effectiveMin=22,humans=2,ai=20,badSlot=false,duplicate=false,teamsValid=true,readyDisabled=false}}
JAC_Staging22Status=function()local s={{}};for k,v in pairs(status)do s[k]=v end;return s end
SlotStatus={{SS_TAKEN=3,SS_COMPUTER=1}};CivilizationLevelTypes={{CIVILIZATION_LEVEL_FULL_CIV=624367792}};PlayerConfigurations={{}}
UpdateReadyButton=function()calls=calls+1 end
"#,s["steps"][2]["map_file"].as_str().unwrap(),cap::STOCK_SHA)).exec().unwrap();
    for (i, p) in s["steps"][2]["players"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        lua.load(format!("PlayerConfigurations[{i}]={{GetSlotStatus=function()return {} end,GetCivilizationLevelTypeID=function()return 624367792 end,GetLeaderTypeName=function()return '{}' end,GetCivilizationTypeName=function()return '{}' end}}",if i<2{3}else{1},p["leader"].as_str().unwrap(),p["civ"].as_str().unwrap())).exec().unwrap();
    }
    lua.load(boundary::observer(&normal, &hotseat, NONCE).unwrap())
        .exec()
        .unwrap();
    lua
}
fn logs(lua: &Lua) -> String {
    lua.globals()
        .get::<Table>("logs")
        .unwrap()
        .sequence_values::<String>()
        .map(Result::unwrap)
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn original_ready_callback_delegates_once_and_existing_frames_reassemble() {
    let lua = world();
    lua.load("UpdateReadyButton();UpdateReadyButton();assert(calls==2)")
        .exec()
        .unwrap();
    let text = logs(&lua);
    let frames = hotseat_bridge::parse_frames(&text, NONCE).unwrap();
    assert_eq!(
        frames.iter().filter(|f| f.kind == "STAGING_READY").count(),
        1
    );
    let facts = frames.iter().find(|f| f.kind == "STAGING_FACTS").unwrap();
    assert!(facts.body.contains("LEADER_JAC_ODA_NOBUHIDE") && facts.body.contains("effectiveMax"));
    assert!(text.contains(&boundary::ready_marker(NONCE)));
    assert!(hotseat_bridge::parse_frames(&text, "abcdef1234567890abcdef1234567890").is_err());
}
#[test]
fn wrong_scope_roster_and_original_rejections_cannot_emit_ready() {
    for fault in [
        "nonce='wrong'",
        "hidden=true",
        "hotseat=false",
        "host=false",
        "map='Pangaea.lua'",
        "status.scopeCap=12",
        "status.effectiveMax=12",
        "status.effectiveMin=2",
        "status.badSlot=true",
        "status.duplicate=true",
        "status.teamsValid=false",
        "status.readyDisabled=true",
        "status.humans=1",
        "PlayerConfigurations[0]=nil",
        "PlayerConfigurations[22]=PlayerConfigurations[21]",
        "PlayerConfigurations[4].GetLeaderTypeName=function()return 'unknown' end",
    ] {
        let lua = world();
        lua.load(fault).exec().unwrap();
        lua.load("UpdateReadyButton();assert(calls==1)")
            .exec()
            .unwrap();
        assert!(
            !logs(&lua).contains(&boundary::ready_marker(NONCE)),
            "{fault}"
        );
    }
}
#[test]
fn initial_disabled_then_enabled_is_observed_without_resending_any_operation() {
    let lua = world();
    lua.load("status.readyDisabled=true;UpdateReadyButton()")
        .exec()
        .unwrap();
    assert!(!logs(&lua).contains(&boundary::ready_marker(NONCE)));
    lua.load("status.readyDisabled=false;UpdateReadyButton();UpdateReadyButton();assert(calls==3)")
        .exec()
        .unwrap();
    let frames = hotseat_bridge::parse_frames(&logs(&lua), NONCE).unwrap();
    assert_eq!(
        frames.iter().filter(|f| f.kind == "STAGING_FACTS").count(),
        2
    );
    assert_eq!(
        frames.iter().filter(|f| f.kind == "STAGING_READY").count(),
        1
    );
}
#[test]
fn read_error_does_not_break_original_ui_or_succeed() {
    let lua = world();
    lua.load("JAC_Staging22Status=function()error('missing snapshot')end;UpdateReadyButton();UpdateReadyButton();assert(calls==2)").exec().unwrap();
    let frames = hotseat_bridge::parse_frames(&logs(&lua), NONCE).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].kind, "STAGING_ERROR");
}
#[test]
fn all_added_observation_failures_are_isolated_but_original_errors_propagate() {
    let lua = world();
    lua.load("GameConfiguration.GetValue=function()error('nonce reader failed')end;UpdateReadyButton();GameConfiguration.GetValue=function()return nonce end;UpdateReadyButton();assert(calls==2)").exec().unwrap();
    let frames = hotseat_bridge::parse_frames(&logs(&lua), NONCE).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].kind, "STAGING_ERROR");
    assert!(!logs(&lua).contains(&boundary::ready_marker(NONCE)));
    let lua = world();
    lua.load("print=function()error('logger failed')end;UpdateReadyButton();UpdateReadyButton();assert(calls==2)").exec().unwrap();
    assert!(logs(&lua).is_empty());
    let (normal, hotseat) = templates();
    let lua = world();
    lua.load("UpdateReadyButton=function()error('original UI failed')end")
        .exec()
        .unwrap();
    lua.load(boundary::observer(&normal, &hotseat, NONCE).unwrap())
        .exec()
        .unwrap();
    assert!(lua
        .load("UpdateReadyButton()")
        .exec()
        .unwrap_err()
        .to_string()
        .contains("original UI failed"));
    assert!(logs(&lua).is_empty());
}
#[test]
fn finite_scene_sends_one_host_then_only_log_read_and_preserves22() {
    let (normal, hotseat) = templates();
    let s = boundary::build(&normal, &hotseat, NONCE).unwrap();
    civ6_autotest_rs::scenario::validate_scenario(&s).unwrap();
    assert_eq!(s["steps"].as_array().unwrap().len(), 5);
    assert_eq!(s["steps"][2]["players"].as_array().unwrap().len(), 22);
    assert_eq!(s["steps"][3]["code"], boundary::HOST);
    assert_eq!(s["steps"][4]["condition"]["type"], "log_line");
    assert_eq!(
        s["steps"][4]["condition"]["contains"],
        boundary::ready_marker(NONCE)
    );
    let observer = boundary::observer(&normal, &hotseat, NONCE).unwrap();
    for bad in [
        "LaunchGame(",
        "HostGame(",
        "DoLeftClick(",
        "SetDisabled(",
        "SetProperty(",
        "RequestPlayerOperation(",
    ] {
        assert!(!observer.contains(bad), "{bad}");
    }
    assert!(boundary::build(&normal, &hotseat, "bad").is_err());
}

#[test]
fn template_identifiers_cannot_escape_lua_or_panic_any_roster_generator() {
    for field in ["leader", "civ"] {
        let prefix = if field == "leader" {
            "LEADER_"
        } else {
            "CIVILIZATION_"
        };
        for bad in [
            serde_json::json!(format!("{prefix}X'}}; injected=true; --")),
            serde_json::json!(format!("{prefix}X\nerror('injected')")),
            serde_json::json!(format!("{prefix}X\0")),
            serde_json::json!(format!("{prefix}X\\")),
            serde_json::json!(format!("{prefix}X\"")),
            serde_json::json!(format!("{prefix}x")),
            serde_json::json!(format!("{prefix}人")),
            serde_json::json!(prefix),
            serde_json::json!(false),
            serde_json::json!(17),
            serde_json::json!([]),
            serde_json::json!({}),
            Value::Null,
        ] {
            for human_input in [false, true] {
                let (mut normal, mut hotseat) = templates();
                if human_input {
                    hotseat["steps"][1]["players"][0][field] = bad.clone();
                } else {
                    normal["steps"][1]["players"][5][field] = bad.clone();
                }
                assert!(civ6_persona_events_rs::mp_hotseat::build(&normal, &hotseat).is_err());
                assert!(
                    civ6_persona_events_rs::mp_hotseat::build_setup_probe(&normal, &hotseat)
                        .is_err()
                );
                assert!(boundary::observer(&normal, &hotseat, NONCE).is_err());
                assert!(boundary::build(&normal, &hotseat, NONCE).is_err());
            }
        }
    }
}
