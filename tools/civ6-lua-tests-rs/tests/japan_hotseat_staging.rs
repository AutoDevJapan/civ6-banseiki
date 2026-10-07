use civ6_lua_tests_rs::{exec, new_lua, read_repo};
use mlua::Lua;
const LIMITS: &str = "poc/JAC_PoC_JapanMapScale/UI/JAC_JapanHotseatLimits.lua";
const STAGING: &str = "poc/JAC_PoC_JapanMapScale/UI/JAC_JapanStagingRoom.lua";
fn fixture() -> Lua {
    let lua = new_lua();
    exec(
        &lua,
        r#"
 hotseat=true; ruleset='RULESET_JAC_BANSEIKI'; maxPlayers=22
 map='{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km_Settlers.Civ6Map'
 GameConfiguration={IsHotseat=function() return hotseat end,GetValue=function(key) assert(key=='RULESET');return ruleset end}
 MapConfiguration={GetValue=function(key) assert(key=='MAP_SCRIPT');return map end,GetMaxMajorPlayers=function() return maxPlayers end}
 "#,
    );
    exec(&lua, &read_repo(LIMITS));
    lua
}
#[test]
fn fixed_japan_hotseat_maps_allow_all_twenty_two_slots() {
    let lua = fixture();
    for suffix in ["20km", "20km_Settlers", "15km", "15km_Settlers"] {
        lua.globals()
            .set(
                "map",
                format!("{{403be889-1455-4de7-967b-ad54c0ff3431}}Maps/JAC_Japan_{suffix}.Civ6Map"),
            )
            .unwrap();
        exec(&lua, "assert(JAC_JapanHotseatMaxPlayers(12)==22)");
    }
}
#[test]
fn network_other_rulesets_maps_and_unexpected_rosters_keep_original_cap() {
    for change in [
        "hotseat=false",
        "ruleset='RULESET_EXPANSION_2'",
        "map='Continents.lua'",
        "map=map..'.other'",
        "maxPlayers=23",
        "maxPlayers=21",
        "maxPlayers=nil",
    ] {
        let lua = fixture();
        exec(&lua, change);
        exec(
            &lua,
            "assert(JAC_JapanHotseatMaxPlayers(12)==12);assert(JAC_JapanHotseatMaxPlayers(8)==8)",
        );
    }
}
// Installed 1.0.12.68 StagingRoom's two independent cap statements, from
// SetCurrentMaxPlayers and OnShow. The production wrapper loads the installed
// script instead of shipping a whole UI snapshot.
fn wrapped_fixture() -> Lua {
    let lua = fixture();
    lua.globals()
        .set("limits_source", read_repo(LIMITS))
        .unwrap();
    exec(
        &lua,
        r#"
 original_math=math; included={}
 function include(name)
  included[#included+1]=name
  if name=='JAC_JapanHotseatLimits' then assert(loadstring(limits_source))()
  elseif name=='StagingRoom' then
   local MAX_EVER_PLAYERS=12;local currentMax=MAX_EVER_PLAYERS
   function SetCurrentMaxPlayers(newMaxPlayers) currentMax=math.min(newMaxPlayers,MAX_EVER_PLAYERS) end
   function OnShow() currentMax=math.min(MapConfiguration.GetMaxMajorPlayers(),12) end
   function ReadMax() return currentMax end
  else error('unexpected include '..name) end
 end
 "#,
    );
    exec(&lua, &read_repo(STAGING));
    lua
}
#[test]
fn stock_callbacks_get_twenty_two_and_keep_dynamic_context_changes() {
    let lua = wrapped_fixture();
    exec(&lua,"assert(#included==2 and included[1]=='JAC_JapanHotseatLimits' and included[2]=='StagingRoom');SetCurrentMaxPlayers(22);assert(ReadMax()==22);OnShow();assert(ReadMax()==22)");
    exec(&lua,"hotseat=false;SetCurrentMaxPlayers(22);assert(ReadMax()==12);OnShow();assert(ReadMax()==12)");
    exec(
        &lua,
        "hotseat=true;map='Continents.lua';OnShow();assert(ReadMax()==12)",
    );
}
#[test]
fn wrapper_keeps_original_math_and_unrelated_minimums_unchanged() {
    let lua = wrapped_fixture();
    exec(
        &lua,
        r#"
 assert(math~=original_math and original_math.min(22,12)==12)
 assert(math.min(22,12)==22)
 assert(math.min(22,12,9)==9 and math.min(22,11)==11)
 assert(math.min(23,12)==12 and math.min(-5,12)==-5)
 assert(math.min(22)==22 and math.sqrt==original_math.sqrt)
 for _,args in ipairs({{22,12},{23,12},{22,11},{22,12,9}}) do
  hotseat=false;assert(math.min(unpack(args))==original_math.min(unpack(args)))
 end
 "#,
    );
    exec(&new_lua(), "assert(math.min(22,12)==12)");
}
#[test]
fn frontend_binds_thin_wrapper_without_gameplay_criteria_or_filename_shadow() {
    let manifest = read_repo("poc/JAC_PoC_JapanMapScale/JAC_PoC_JapanMapScale.modinfo");
    let doc = roxmltree::Document::parse(&manifest).unwrap();
    let frontend = doc
        .descendants()
        .find(|n| n.has_tag_name("FrontEndActions"))
        .unwrap();
    let replacements: Vec<_> = frontend
        .children()
        .filter(|n| n.has_tag_name("ReplaceUIScript"))
        .collect();
    assert_eq!(replacements.len(), 1);
    let replacement = replacements[0];
    assert!(replacement.attribute("criteria").is_none());
    assert_eq!(
        replacement
            .descendants()
            .find(|n| n.has_tag_name("LuaContext"))
            .unwrap()
            .text(),
        Some("StagingRoom")
    );
    assert_eq!(
        replacement
            .descendants()
            .find(|n| n.has_tag_name("LuaReplace"))
            .unwrap()
            .text(),
        Some("UI/JAC_JapanStagingRoom.lua")
    );
    let imports = frontend
        .children()
        .find(|n| n.has_tag_name("ImportFiles"))
        .unwrap();
    let files: Vec<_> = imports
        .children()
        .filter(|n| n.has_tag_name("File"))
        .map(|n| n.text().unwrap())
        .collect();
    assert_eq!(
        files,
        [
            "UI/JAC_JapanStagingRoom.lua",
            "UI/JAC_JapanHotseatLimits.lua"
        ]
    );
    assert!(!manifest.contains("UI/StagingRoom.lua"));
    let source = read_repo(STAGING);
    new_lua().load(&source).into_function().unwrap();
    assert!(source.lines().count() < 30);
    assert!(source.contains("include(\"StagingRoom\")"));
}
