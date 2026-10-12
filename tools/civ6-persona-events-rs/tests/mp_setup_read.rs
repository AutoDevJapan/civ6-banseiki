use civ6_persona_events_rs::mp_setup_read as read;
use mlua::{Lua, Table};
use serde_json::{json, Value};
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn scene() -> Value {
    let r = root();
    let load = |file| {
        serde_json::from_str(
            &std::fs::read_to_string(r.join("tools/civ6-autotest/scenarios").join(file)).unwrap(),
        )
        .unwrap()
    };
    read::build(
        &r,
        &load("leader-persona-15km.json"),
        &load("multiplayer-banseiki-hotseat-growth.json"),
    )
    .unwrap()
}
fn world() -> Lua {
    let lua = Lua::new();
    lua.load(r#"
print=function()end
GameConfiguration={IsHotseat=function()return true end,IsNetworkMultiplayer=function()return false end,GetRuleSet=function()return 'RULESET_JAC_BANSEIKI' end,
GetParticipatingPlayerCount=function()return 14 end,GetHiddenPlayerCount=function()return 2 end,GetValue=function(key)return key=='CITY_STATE_COUNT' and 0 or 1 end}
mapMax=22;MapConfiguration={GetValue=function(k)if k=='MAP_SCRIPT'then return 'Pangaea.lua' end;return nil end,GetMapSize=function()return 'MAPSIZE_HUGE' end,
GetMinMajorPlayers=function()return 2 end,GetMaxMajorPlayers=function()return mapMax end,GetMinMinorPlayers=function()error('one missing reader')end,GetMaxMinorPlayers=function()return end}
UI={IsInFrontEnd=function()return true end};Game={GetLocalPlayer=function()error('not in game')end};Network={GetLocalPlayerID=function()return -1 end}
SlotStatus={SS_TAKEN=3,SS_COMPUTER=1};CivilizationLevelTypes={CIVILIZATION_LEVEL_FULL_CIV=0};PlayerConfigurations={}
for i=0,11 do local id=i;PlayerConfigurations[id]={GetSlotStatus=function()return id<2 and 3 or 1 end,
GetLeaderTypeName=function()return 'LEADER_'..id end,GetCivilizationTypeName=function()return 'CIV_'..id end,
GetCivilizationLevelTypeID=function()return 0 end,IsHumanRequired=function()return id<2 end,GetReady=function()return false end,GetModReady=function()return true end}end
DB={ConfigurationQuery=function()return {{Domain='StandardMapSizes',MapSizeType='MAPSIZE_HUGE',MinPlayers=2,MaxPlayers=22,DefaultPlayers=12}}end}
Modding={GetInstalledMods=function()return {{Handle=1},{Handle=2},{Handle=3}}end,GetModInfo=function(h)if h==2 then error('bad second mod')end;return {Id='mod'..h,Enabled=true}end}
"#).exec().unwrap();
    lua
}
fn report(lua: &Lua, manifest: &Value) -> Table {
    let code = read::reader(manifest);
    assert!(code.ends_with("return serial(report)\n"));
    // Return the collected table in the fixture; execute the same collector.
    lua.load(
        code.strip_suffix("return serial(report)\n")
            .unwrap()
            .to_owned()
            + "return report",
    )
    .eval()
    .unwrap()
}
#[test]
fn one_failing_reader_does_not_erase_success_missing_arity_or_later_mod_rows() {
    let lua = world();
    let r = report(&lua, &json!([]));
    let m: Table = r.get("measurements").unwrap();
    assert_eq!(
        m.get::<Table>("map.max_major")
            .unwrap()
            .get::<String>("status")
            .unwrap(),
        "OK"
    );
    let err = m.get::<Table>("map.min_minor").unwrap();
    assert_eq!(err.get::<String>("status").unwrap(), "ERROR");
    assert!(err
        .get::<String>("error")
        .unwrap()
        .contains("one missing reader"));
    let missing = m.get::<Table>("map.max_minor").unwrap();
    assert_eq!(missing.get::<String>("status").unwrap(), "MISSING");
    assert_eq!(
        missing
            .get::<Table>("result")
            .unwrap()
            .get::<u32>("arity")
            .unwrap(),
        0
    );
    let mods: Table = r.get("mods").unwrap();
    assert_eq!(mods.raw_len(), 3);
    assert_eq!(
        mods.get::<Table>(2)
            .unwrap()
            .get::<String>("status")
            .unwrap(),
        "ERROR"
    );
    assert_eq!(
        mods.get::<Table>(3)
            .unwrap()
            .get::<String>("status")
            .unwrap(),
        "OK"
    );
    assert_eq!(r.get::<Table>("slots").unwrap().raw_len(), 64);
    let eval: Table = r.get("stock_staging_assessment").unwrap();
    assert_eq!(eval.get::<u32>("effectiveMax").unwrap(), 12);
    assert_eq!(eval.get::<u32>("configured_taken_full_civs").unwrap(), 2);
    assert_eq!(
        eval.get::<u32>("configured_computer_full_civs").unwrap(),
        10
    );
    assert!(r
        .get::<String>("acceptance")
        .unwrap()
        .starts_with("not_evaluated"));
}
#[test]
fn criterion_prediction_is_separate_from_enabled_and_execution_and_preserves_unknown() {
    let manifest = json!([{"id":"mod1","file":"own.modinfo","criteria":{"fixed":[{"kind":"config","group":"Map","key":"MAP_SCRIPT","expected":"FixedJapan.Civ6Map"}],"mode":[{"kind":"config","group":"Game","key":"GAMEMODE_HEROES","expected":"1"}],"rule":[{"kind":"ruleset","values":["OTHER","RULESET_JAC_BANSEIKI"]}]},"actions":[{"id":"a","kind":"AddGameplayScripts","phase":"InGameActions","criteria":"fixed"},{"id":"b","criteria":"mode"},{"id":"c","criteria":"rule"}]}]);
    let lua = world();
    let r = report(&lua, &manifest);
    let actions: Table = r.get("actions").unwrap();
    for (n, prediction) in [(1, "no_match"), (2, "unknown"), (3, "match")] {
        let a: Table = actions.get(n).unwrap();
        assert_eq!(a.get::<String>("predicate_prediction").unwrap(), prediction);
        assert_eq!(a.get::<String>("execution").unwrap(), "not_measured");
    }
}
#[test]
fn native_bound_is_not_inferred_from_map_recommended_counts() {
    let lua = world();
    lua.load("mapMax=8").exec().unwrap();
    let r = report(&lua, &json!([]));
    let e: Table = r.get("stock_staging_assessment").unwrap();
    assert_eq!(e.get::<u32>("effectiveMax").unwrap(), 8);
    assert_eq!(e.get::<Table>("invalid_indices").unwrap().raw_len(), 4);
    lua.load("mapMax=nil").exec().unwrap();
    let r = report(&lua, &json!([]));
    let e: Table = r.get("stock_staging_assessment").unwrap();
    assert!(e.get::<Option<u32>>("effectiveMax").unwrap().is_none());
    assert!(e
        .get::<String>("reason")
        .unwrap()
        .contains("no guessed bound"));
}
#[test]
fn source_root_manifest_and_no_host_scene_are_complete() {
    let s = scene();
    civ6_autotest_rs::scenario::validate_scenario(&s).unwrap();
    let steps = s["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 4);
    assert_eq!(steps[2]["players"].as_array().unwrap().len(), 12);
    for step in steps {
        assert!(!matches!(
            step["action"].as_str(),
            Some("start_game" | "end_turn" | "save" | "load" | "start_native_paid_runtime")
        ));
    }
    let code = steps[3]["code"].as_str().unwrap();
    for bad in [
        "HostGame(",
        "DoLeftClick(",
        "RequestPlayerOperation",
        "SetProperty",
        "SetValue(",
        "assert(",
    ] {
        assert!(!code.contains(bad), "{bad}");
    }
    let m = read::manifest(&root(), steps[0]["ids"].as_array().unwrap()).unwrap();
    assert_eq!(m.as_array().unwrap().len(), 6);
    let persona = m
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == "91cf7b65-54bb-4f69-a452-19a65527cde2")
        .unwrap();
    assert_eq!(persona["criteria"].as_object().unwrap().len(), 4);
    assert!(persona["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["kind"] == "AddGameplayScripts"));
    Lua::new().load(code).into_function().unwrap();
    let lua = world();
    let encoded = lua.load(code).eval::<String>().unwrap();
    assert!(encoded.contains("not_measured_before_host") && encoded.contains("ERROR"));
    assert!(encoded.contains("measurements only; excludes map_size_rows and mods"));
}
