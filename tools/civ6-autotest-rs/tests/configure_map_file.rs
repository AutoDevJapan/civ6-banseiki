//! Official script identifiers are exact values, not arbitrary filesystem paths.
use civ6_autotest_rs::scenario::{self, SessionType};
use mlua::Lua;
use serde_json::{json, Value};
fn config() -> Value {
    json!({"action":"configure","ruleset":"RULESET_JAC_BANSEIKI","map_size":"MAPSIZE_HUGE","speed":"GAMESPEED_STANDARD","seed":621006,"city_states":0,"build_setup":true,"map_file":"Continents.lua","players":[{"leader":"LEADER_A","civ":"CIVILIZATION_A","human":true},{"leader":"LEADER_B","civ":"CIVILIZATION_B","human":true}]})
}
fn valid(c: Value) -> bool {
    scenario::validate_scenario(
        &json!({"name":"map configuration","session_type":"hotseat","steps":[c]}),
    )
    .is_ok()
}
fn world() -> Lua {
    let lua = Lua::new();
    lua.load(r#"
map=nil;count=0;ignoreMap=false;refreshMap=nil;log={};print=function(s)log[#log+1]=s end
UI={IsInFrontEnd=function()return true end};GameModeTypes={HOTSEAT=1};SlotStatus={SS_TAKEN=3,SS_COMPUTER=1}
GameConfiguration={SetToDefaults=function(mode)assert(mode==1);map='Pangaea.lua' end,
SetRuleSet=function()end,UpdateEnabledMods=function()end,SetGameSpeedType=function()end,
SetValue=function(_,v)citystates=v end,GetValue=function()return citystates end,
SetParticipatingPlayerCount=function(v)count=v end,GetParticipatingPlayerCount=function()return count end,
GetHiddenPlayerCount=function()return 2 end,IsHotseat=function()return true end}
MapConfiguration={SetValue=function(k,v)if k=='MAP_SCRIPT' and not ignoreMap then map=v end end,GetValue=function(k)assert(k=='MAP_SCRIPT');return map end,
SetMapSize=function()end,SetMaxMajorPlayers=function()end}
PlayerConfigurations={}
for id=0,1 do local p={};PlayerConfigurations[id]=p
p.SetSlotStatus=function(_,v)p.slot=v end;p.GetSlotStatus=function()return p.slot end
p.SetLeaderTypeName=function(_,v)p.leader=v end;p.GetLeaderTypeName=function()return p.leader end
p.SetCivilizationTypeName=function(_,v)p.civ=v end;p.GetCivilizationTypeName=function()return p.civ end
end
function BuildHeadlessGameSetup()end
g_GameParameters={FullRefresh=function()if refreshMap~=nil then map=refreshMap end end}
function HideGameSetup()end
"#).exec().unwrap();
    lua
}
#[test]
fn only_two_official_scripts_and_existing_fixed_map_ids_are_accepted() {
    for map in [
        "Continents.lua",
        "Pangaea.lua",
        "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map",
    ] {
        let mut c = config();
        c["map_file"] = map.into();
        assert!(valid(c), "{map}");
    }
    for map in [
        "Maps/Continents.lua",
        "../Continents.lua",
        "C:/Continents.lua",
        "Random.lua",
        "continents.lua",
        "Continents.lua';error('x')",
    ] {
        let mut c = config();
        c["map_file"] = map.into();
        assert!(!valid(c), "{map}");
    }
    let mut c = config();
    c["map_file"] = json!(2);
    assert!(!valid(c));
}
#[test]
fn hotseat_default_is_explicitly_replaced_and_refreshed_value_is_verified() {
    let c = config();
    let lua = world();
    lua.load(scenario::configure_code_for_session(
        &c,
        SessionType::Hotseat,
    ))
    .exec()
    .unwrap();
    lua.load("assert(map=='Continents.lua' and #log==1 and log[1]:find('type=string value=Continents.lua',1,true))").exec().unwrap();
    lua.load(scenario::build_setup_code(&c)).exec().unwrap();
    lua.load("assert(map=='Continents.lua' and #log==2 and log[2]:find('setup-refreshed',1,true))")
        .exec()
        .unwrap();
    let mut omitted = c.clone();
    omitted.as_object_mut().unwrap().remove("map_file");
    let lua = world();
    lua.load(scenario::configure_code_for_session(
        &omitted,
        SessionType::Hotseat,
    ))
    .exec()
    .unwrap();
    lua.load("assert(map=='Pangaea.lua')").exec().unwrap();
}
#[test]
fn ignored_setter_wrong_prefix_and_wrong_type_are_recorded_then_rejected() {
    let c = config();
    let lua = world();
    lua.load("ignoreMap=true").exec().unwrap();
    assert!(lua
        .load(scenario::configure_code_for_session(
            &c,
            SessionType::Hotseat
        ))
        .exec()
        .is_err());
    lua.load("assert(log[1]:find('value=Pangaea.lua',1,true))")
        .exec()
        .unwrap();
    for mutation in [
        "refreshMap='Pangaea.lua'",
        "refreshMap='Maps/Continents.lua'",
        "refreshMap=42",
    ] {
        let lua = world();
        lua.load(scenario::configure_code_for_session(
            &c,
            SessionType::Hotseat,
        ))
        .exec()
        .unwrap();
        lua.load(mutation).exec().unwrap();
        assert!(
            lua.load(scenario::build_setup_code(&c)).exec().is_err(),
            "{mutation}"
        );
        lua.load("assert(#log==2 and log[2]:find('setup-refreshed',1,true))")
            .exec()
            .unwrap();
    }
}
