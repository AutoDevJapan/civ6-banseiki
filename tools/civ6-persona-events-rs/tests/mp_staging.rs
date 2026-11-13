use civ6_persona_events_rs::mp_staging as staging;
use mlua::Lua;
#[test]
fn cap_is_limited_to_exact_four_maps_hotseat_and_ruleset() {
    for map in staging::MAPS {
        let lua = Lua::new();
        lua.load(format!("map='{map}';hotseat=true;ruleset='RULESET_JAC_BANSEIKI';GameConfiguration={{IsHotseat=function()return hotseat end,GetRuleSet=function()return ruleset end}};MapConfiguration={{GetValue=function()return map end}};{}\ncap=JAC_StagingCap",staging::policy())).exec().unwrap();
        lua.load("assert(cap()==22)").exec().unwrap();
        for fault in [
            "hotseat=false",
            "hotseat=1",
            "ruleset='RULESET_EXPANSION_2'",
            "map='Pangaea.lua'",
            "map=22",
            "map=nil",
        ] {
            lua.load("hotseat=true;ruleset='RULESET_JAC_BANSEIKI'")
                .exec()
                .unwrap();
            lua.load(format!("map='{map}';{fault};assert(cap()==12)"))
                .exec()
                .unwrap();
        }
    }
}
#[test]
fn registration_is_frontend_import_without_map_activation_or_new_controls() {
    let text = staging::manifest();
    let xml = roxmltree::Document::parse(&text).unwrap();
    assert_eq!(xml.root_element().attribute("id"), Some(staging::MOD_ID));
    assert_eq!(
        xml.descendants()
            .filter(|n| n.has_tag_name("ImportFiles"))
            .count(),
        1
    );
    assert!(
        !text.contains("ReplaceUIScript")
            && !text.contains("ActionCriteria")
            && !text.contains("AddUserInterfaces")
    );
    assert!(text.contains("<File>UI/StagingRoom.lua</File>"));
    assert!(!staging::STATUS.contains("LaunchGame") && !staging::STATUS.contains("SetDisabled"));
}
