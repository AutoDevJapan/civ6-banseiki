use civ6_lua_tests_rs::{exec, new_lua, read_repo};
use mlua::Lua;
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
    lua
}
#[test]
fn fixed_japan_hotseat_maps_allow_all_twenty_two_slots() {
    let lua = wrapped_fixture();
    for suffix in ["20km", "20km_Settlers", "15km", "15km_Settlers"] {
        lua.globals()
            .set(
                "map",
                format!("{{403be889-1455-4de7-967b-ad54c0ff3431}}Maps/JAC_Japan_{suffix}.Civ6Map"),
            )
            .unwrap();
        exec(&lua, "assert(math.min(22,12)==22)");
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
        let lua = wrapped_fixture();
        exec(&lua, change);
        exec(
            &lua,
            "assert(math.min(22,12)==12);assert(math.min(22,8)==8)",
        );
    }
}
// Installed 1.0.12.68 StagingRoom's two independent cap statements, from
// SetCurrentMaxPlayers and OnShow. The production wrapper loads the installed
// script instead of shipping a whole UI snapshot.
fn wrapped_fixture() -> Lua {
    let lua = fixture();
    exec(
        &lua,
        r#"
 original_math=math; included={}
 function include(name)
  included[#included+1]=name
  if name=='StagingRoom' then
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
    exec(&lua,"assert(#included==1 and included[1]=='StagingRoom');SetCurrentMaxPlayers(22);assert(ReadMax()==22);OnShow();assert(ReadMax()==22)");
    exec(&lua,"hotseat=false;SetCurrentMaxPlayers(22);assert(ReadMax()==12);OnShow();assert(ReadMax()==12)");
    exec(
        &lua,
        "hotseat=true;map='Continents.lua';OnShow();assert(ReadMax()==12)",
    );
}
#[test]
fn save_load_keeps_the_limits_callback_after_frontend_globals_are_cleared() {
    let lua = wrapped_fixture();
    exec(
        &lua,
        "JAC_JapanHotseatMaxPlayers=nil;OnShow();assert(ReadMax()==22);hotseat=false;OnShow();assert(ReadMax()==12)",
    );
}
#[test]
fn reloaded_frontend_defines_limits_without_cached_helper_exports() {
    let lua = wrapped_fixture();
    exec(&lua, "JAC_JapanHotseatMaxPlayers=nil");
    exec(&lua, &read_repo(STAGING));
    exec(
        &lua,
        "OnShow();assert(ReadMax()==22);hotseat=false;OnShow();assert(ReadMax()==12)",
    );
}
#[test]
fn wrapper_keeps_original_math_and_unrelated_minimums_unchanged() {
    let lua = wrapped_fixture();
    exec(
        &lua,
        r#"
      assert(math~=original_math and original_math.min(22,12)==12)
      for key,value in pairs(original_math) do
        if key~='min' then assert(rawget(math,key)==value) end
      end
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
fn frontend_xml_binds_stock_wrappers_without_unsupported_replacement_actions() {
    let manifest = read_repo("poc/JAC_PoC_JapanMapScale/JAC_PoC_JapanMapScale.modinfo");
    let doc = roxmltree::Document::parse(&manifest).unwrap();
    let frontend = doc
        .descendants()
        .find(|n| n.has_tag_name("FrontEndActions"))
        .unwrap();
    assert!(!frontend
        .children()
        .any(|n| n.has_tag_name("ReplaceUIScript")));
    let imports = frontend
        .children()
        .find(|n| n.has_tag_name("ImportFiles"))
        .unwrap();
    assert!(imports.attribute("criteria").is_none());
    let files: Vec<_> = imports
        .children()
        .filter(|n| n.has_tag_name("File"))
        .map(|n| n.text().unwrap())
        .collect();
    assert_eq!(
        files,
        [
            "UI/FrontEnd.xml",
            "UI/JAC_JapanMainMenu.xml",
            "UI/JAC_JapanMainMenu.lua",
            "UI/JAC_JapanStagingRoom.xml",
            "UI/JAC_JapanStagingRoom.lua",
        ]
    );
    let front = read_repo("poc/JAC_PoC_JapanMapScale/UI/FrontEnd.xml");
    let front = roxmltree::Document::parse(&front).unwrap();
    assert_eq!(
        front
            .descendants()
            .find(|n| n.has_tag_name("LuaContext") && n.attribute("ID") == Some("MainMenu"))
            .unwrap()
            .attribute("FileName"),
        Some("JAC_JapanMainMenu")
    );
    let main = read_repo("poc/JAC_PoC_JapanMapScale/UI/JAC_JapanMainMenu.xml");
    // Independent installed 1.0.12.68 XML, with line endings and trailing
    // whitespace normalized (SHA-256 of the original bytes:
    // d522f102c3b0bf9182ccc08ea60a58fde596b8d628fabd1f2baea9512357e667).
    // Including MainMenu and adding another StagingRoom duplicated its events
    // and let saved games open the original twelve-slot context in live05.
    let stock_main = read_repo("tools/civ6-lua-tests-rs/fixtures/frontend/MainMenu-1.0.12.68.xml");
    assert_eq!(
        main.replace(
            "FileName=\"JAC_JapanStagingRoom\"",
            "FileName=\"StagingRoom\""
        ),
        stock_main
    );
    let main = roxmltree::Document::parse(&main).unwrap();
    assert!(!main
        .descendants()
        .any(|n| n.has_tag_name("Include") && n.attribute("File") == Some("MainMenu")));
    assert_eq!(
        main.descendants()
            .filter(|n| n.has_tag_name("LuaContext") && n.attribute("ID") == Some("StagingRoom"))
            .count(),
        1
    );
    assert_eq!(
        main.descendants()
            .find(|n| n.has_tag_name("LuaContext") && n.attribute("ID") == Some("StagingRoom"))
            .unwrap()
            .attribute("FileName"),
        Some("JAC_JapanStagingRoom")
    );
    let stage = read_repo("poc/JAC_PoC_JapanMapScale/UI/JAC_JapanStagingRoom.xml");
    let stage = roxmltree::Document::parse(&stage).unwrap();
    assert_eq!(
        stage
            .descendants()
            .find(|n| n.has_tag_name("Include"))
            .unwrap()
            .attribute("File"),
        Some("StagingRoom")
    );
    assert_eq!(
        read_repo("poc/JAC_PoC_JapanMapScale/UI/JAC_JapanMainMenu.lua").trim(),
        "include(\"MainMenu\")"
    );
    let source = read_repo(STAGING);
    new_lua().load(&source).into_function().unwrap();
    assert!(source.contains("include(\"StagingRoom\")"));
    assert!(!manifest.contains("UI/StagingRoom.lua"));
}
