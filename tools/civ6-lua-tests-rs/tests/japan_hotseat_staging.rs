use civ6_lua_tests_rs::{exec, new_lua, read_repo};
use mlua::Lua;
use regex::Regex;

const LIMITS: &str = "poc/JAC_PoC_JapanMapScale/UI/JAC_JapanHotseatLimits.lua";
const STAGING: &str = "poc/JAC_PoC_JapanMapScale/UI/JAC_JapanStagingRoom.lua";

fn fixture() -> Lua {
    let lua = new_lua();
    exec(
        &lua,
        r#"
      hotseat=true; ruleset='RULESET_JAC_BANSEIKI'; maxPlayers=22
      map='{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km_Settlers.Civ6Map'
      GameConfiguration={IsHotseat=function() return hotseat end,
        GetValue=function(key) assert(key=='RULESET'); return ruleset end}
      MapConfiguration={GetValue=function(key) assert(key=='MAP_SCRIPT'); return map end,
        GetMaxMajorPlayers=function() return maxPlayers end}
    "#,
    );
    exec(&lua, &read_repo(LIMITS));
    lua
}

fn stock_lua(source: &str) -> String {
    // Preserve quoted strings, long strings and comments byte for byte.
    // Only a type token in executable code is removed for the test VM.
    let tokens = Regex::new(
        r#"(?s)--\[\[.*?\]\]|--[^\r\n]*|\[\[.*?\]\]|'(?:\\.|[^'\\])*'|"(?:\\.|[^"\\])*"|:\s*(?:number|string|table|boolean|object)\b"#,
    ).unwrap();
    tokens
        .replace_all(source, |capture: &regex::Captures<'_>| {
            let matched = &capture[0];
            if matched.starts_with(':') {
                String::new()
            } else {
                matched.to_owned()
            }
        })
        .into_owned()
}

#[test]
fn test_vm_type_translation_preserves_strings_and_comments() {
    let source = "local n : number = 1; local text=':number'; -- :table\nlocal s:string=\" :boolean\"; --[[:number]]\nlocal long=[[:table]]";
    assert_eq!(stock_lua(source), "local n  = 1; local text=':number'; -- :table\nlocal s=\" :boolean\"; --[[:number]]\nlocal long=[[:table]]");
    new_lua().load(stock_lua(source)).exec().unwrap();
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
            "assert(JAC_JapanHotseatMaxPlayers(12)==12); assert(JAC_JapanHotseatMaxPlayers(8)==8)",
        );
    }
}

#[test]
fn official_staging_script_compiles_on_lua51_and_both_caps_use_scoped_limit() {
    let source = read_repo(STAGING);
    // The installed game accepts type annotations; strip only those for stock Lua 5.1.
    new_lua().load(stock_lua(&source)).into_function().unwrap();
    assert!(
        source.contains("math.min(newMaxPlayers, JAC_JapanHotseatMaxPlayers(MAX_EVER_PLAYERS))")
    );
    assert!(source.contains(
        "math.min(MapConfiguration.GetMaxMajorPlayers(), JAC_JapanHotseatMaxPlayers(12))"
    ));
    assert!(!source.contains("math.min(newMaxPlayers, MAX_EVER_PLAYERS)"));
    assert!(!source.contains("math.min(MapConfiguration.GetMaxMajorPlayers(), 12)"));
}

#[test]
fn actual_staging_setter_uses_twenty_two_then_restores_twelve_on_context_change() {
    let source = read_repo(STAGING);
    let setter = source
        .split_once("function SetCurrentMaxPlayers")
        .unwrap()
        .1
        .split_once("function SetCurrentMinPlayers")
        .unwrap()
        .0;
    let lua = fixture();
    exec(
        &lua,
        &format!(
            "MAX_EVER_PLAYERS=12; g_currentMaxPlayers=0; function SetCurrentMaxPlayers{}",
            stock_lua(setter)
        ),
    );
    exec(&lua, "SetCurrentMaxPlayers(22); assert(g_currentMaxPlayers==22); SetCurrentMaxPlayers(23); assert(g_currentMaxPlayers==22); SetCurrentMaxPlayers(10); assert(g_currentMaxPlayers==10)");
    exec(
        &lua,
        "hotseat=false; SetCurrentMaxPlayers(22); assert(g_currentMaxPlayers==12)",
    );
    exec(&lua, "hotseat=true; map='Continents.lua'; SetCurrentMaxPlayers(22); assert(g_currentMaxPlayers==12)");
}

#[test]
fn frontend_imports_the_real_staging_context_and_scoped_helper() {
    let manifest = read_repo("poc/JAC_PoC_JapanMapScale/JAC_PoC_JapanMapScale.modinfo");
    let doc = roxmltree::Document::parse(&manifest).unwrap();
    let action = doc
        .descendants()
        .find(|node| {
            node.has_tag_name("ImportFiles")
                && node.attribute("id") == Some("JAC_JapanMapScale_HotseatStaging")
        })
        .unwrap();
    assert!(action.parent().unwrap().has_tag_name("FrontEndActions"));
    let files: Vec<_> = action
        .children()
        .filter(|node| node.has_tag_name("File"))
        .map(|node| node.text().unwrap())
        .collect();
    assert_eq!(
        files,
        [
            "UI/JAC_JapanStagingRoom.lua",
            "UI/JAC_JapanHotseatLimits.lua"
        ]
    );
    let frontend = doc
        .descendants()
        .find(|node| node.has_tag_name("FrontEndActions"))
        .unwrap();
    let replacements: Vec<_> = frontend
        .children()
        .filter(|node| node.has_tag_name("ReplaceUIScript"))
        .collect();
    assert_eq!(replacements.len(), 4);
    let mut maps = std::collections::BTreeSet::new();
    for replacement in replacements {
        assert_eq!(
            replacement
                .descendants()
                .find(|node| node.has_tag_name("LuaContext"))
                .unwrap()
                .text(),
            Some("StagingRoom")
        );
        assert_eq!(
            replacement
                .descendants()
                .find(|node| node.has_tag_name("LuaReplace"))
                .unwrap()
                .text(),
            Some("UI/JAC_JapanStagingRoom.lua")
        );
        let criteria_id = replacement.attribute("criteria").unwrap();
        let criteria = doc
            .descendants()
            .find(|node| node.has_tag_name("Criteria") && node.attribute("id") == Some(criteria_id))
            .unwrap();
        assert_eq!(
            criteria
                .descendants()
                .find(|node| node.has_tag_name("RuleSetInUse"))
                .unwrap()
                .text(),
            Some("RULESET_JAC_BANSEIKI")
        );
        let matches = criteria
            .descendants()
            .find(|node| node.has_tag_name("ConfigurationValueMatches"))
            .unwrap();
        let field = |name| {
            matches
                .children()
                .find(|node| node.has_tag_name(name))
                .unwrap()
                .text()
                .unwrap()
        };
        assert_eq!(field("Group"), "Map");
        assert_eq!(field("ConfigurationId"), "MAP_SCRIPT");
        maps.insert(field("Value").to_owned());
    }
    let expected: std::collections::BTreeSet<_> =
        ["20km", "15km", "20km_Settlers", "15km_Settlers"]
            .map(|name| {
                format!("{{403be889-1455-4de7-967b-ad54c0ff3431}}Maps/JAC_Japan_{name}.Civ6Map")
            })
            .into();
    assert_eq!(maps, expected);
    assert!(!manifest.contains("UI/StagingRoom.lua"));
}
