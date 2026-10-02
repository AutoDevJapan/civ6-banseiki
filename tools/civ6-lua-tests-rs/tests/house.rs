use civ6_lua_tests_rs::{exec, exec_args, json_to_lua, new_lua, read_repo};
use mlua::{FromLuaMulti, IntoLuaMulti, Lua, Table};
use serde_json::{Map, Value as Json};
use std::collections::BTreeMap;

const UI: &str = "poc/JAC_PoC_BanseikiRuleset/UI";
const DESC: &str = "LOC_JAC_TEST_PERIOD_DESCRIPTION";
const ERA: &str = "LOC_JAC_TEST_PERIOD_NAME";

fn load_texts(language: &str) -> BTreeMap<String, String> {
    let (generic, house, notification, moment) = match language {
        "en_US" => (
            "Campaigns for Unification",
            "Tenka Fubu and Wide-Area Campaigns",
            "You have discovered {1_Name}!",
            "First civic of the {1_Era} era.",
        ),
        "ja_JP" => (
            "統一への広域戦",
            "天下布武と広域戦",
            "{1_Name}を習得しました！",
            "{1_Era}の最初の社会制度。",
        ),
        _ => panic!("unknown test language {language}"),
    };
    let mut texts = BTreeMap::new();
    for tag in [ERA, DESC] {
        texts.insert(tag.into(), generic.into());
        texts.insert(format!("{tag}_HOUSE"), house.into());
    }
    texts.insert(
        "LOC_NOTIFICATION_CIVIC_DISCOVERED_SUMMARY".into(),
        notification.into(),
    );
    texts.insert("LOC_MOMENT_TEST".into(), moment.into());
    let xml = read_repo("poc/JAC_PoC_SengokuTrees/Text/JAC_Sengoku_Text.xml");
    let document = roxmltree::Document::parse(&xml).unwrap();
    for row in document
        .descendants()
        .filter(|n| n.has_tag_name("Row") && n.attribute("Language") == Some(language))
    {
        let text = row
            .children()
            .find(|n| n.has_tag_name("Text"))
            .and_then(|n| n.text())
            .unwrap_or("");
        texts.insert(row.attribute("Tag").unwrap().into(), text.into());
    }
    texts
}

fn sources() -> Map<String, Json> {
    let mut sources = Map::new();
    sources.insert(
        "JAC_HouseNames".into(),
        Json::from(read_repo(format!("{UI}/JAC_HouseNames.lua"))),
    );
    sources.insert(
        "JAC_HouseNamesData_Sengoku".into(),
        Json::from(read_repo(
            "poc/JAC_PoC_SengokuTrees/UI/JAC_HouseNamesData_Sengoku.lua",
        )),
    );
    sources.insert("JAC_HouseNamesData_Test".into(),Json::from([ERA,DESC].map(|tag| format!("JAC_RegisterHouseName(\"{tag}\", \"{tag}_HOUSE\", \"CIVILIZATION_JAC_ODA\")")).join("\n")));
    sources
}

struct House {
    lua: Lua,
    texts: BTreeMap<String, String>,
}
impl House {
    fn new(civilization: Option<&str>, language: &str, extra: &str) -> Self {
        let lua = new_lua();
        let texts = load_texts(language);
        Self::init(&lua, &texts, sources(), civilization, language);
        if !extra.is_empty() {
            exec(&lua, extra);
        }
        exec(
            &lua,
            "include('JAC_HouseNames'); JAC_InstallHouseNames('Test')",
        );
        Self { lua, texts }
    }
    fn init(
        lua: &Lua,
        texts: &BTreeMap<String, String>,
        sources: Map<String, Json>,
        civilization: Option<&str>,
        language: &str,
    ) {
        exec_args(
            lua,
            include_str!("../fixtures/house/stubs.lua"),
            (
                json_to_lua(lua, &serde_json::to_value(texts).unwrap()),
                language,
                json_to_lua(lua, &Json::Object(sources)),
                civilization,
            ),
        );
    }
    fn lookup(&self, args: impl IntoLuaMulti) -> String {
        self.lua
            .load("return Locale.Lookup(...)")
            .call(args)
            .unwrap()
    }
    fn upper(&self, value: &str) -> String {
        self.lua
            .load("return Locale.ToUpper(...)")
            .call(value)
            .unwrap()
    }
    fn base_upper(&self, value: &str) -> String {
        self.lua
            .load("return JAC_HouseNamesBaseLocale.ToUpper(...)")
            .call(value)
            .unwrap()
    }
    fn eval<T: FromLuaMulti>(&self, expr: &str) -> T {
        self.lua.load(format!("return {expr}")).eval().unwrap()
    }
    fn strings(&self, expr: &str) -> Vec<String> {
        self.eval::<Table>(expr)
            .sequence_values()
            .collect::<mlua::Result<_>>()
            .unwrap()
    }
}

#[test]
fn test_install_shadows_the_context_global_without_writing_the_engine_table() {
    let s = House::new(Some("CIVILIZATION_JAC_ODA"), "en_US", "");
    assert_eq!(
        s.strings("LOG"),
        vec!["[JAC_POC] houseNames context=Test result=installed"]
    );
    assert!(s.eval::<bool>("Locale ~= ENGINE_LOCALE and JAC_HouseNamesBaseLocale == ENGINE_LOCALE"));
    assert_eq!(s.lookup(DESC), s.texts[&format!("{DESC}_HOUSE")]);
    assert_eq!(
        s.lua
            .load("return ENGINE_LOCALE.Lookup(...)")
            .call::<String>(DESC)
            .unwrap(),
        s.texts[DESC]
    );
    assert!(s.eval::<bool>("Locale.Compare == ENGINE_LOCALE.Compare"));
    exec(
        &s.lua,
        "include('JAC_HouseNames'); JAC_InstallHouseNames('Test')",
    );
    assert_eq!(
        s.strings("LOG").last().unwrap(),
        "[JAC_POC] houseNames context=Test result=already_installed"
    );
    assert!(s.eval::<bool>("JAC_HouseNamesBaseLocale == ENGINE_LOCALE"));
    assert_eq!(
        s.lua
            .load("return Locale.Lookup(...)")
            .call::<String>(DESC)
            .unwrap(),
        s.texts[&format!("{DESC}_HOUSE")]
    );
}
#[test]
fn test_owner_sees_house_names_for_keys_arguments_and_upper_case() {
    let s = House::new(Some("CIVILIZATION_JAC_ODA"), "en_US", "");
    let house = &s.texts[&format!("{DESC}_HOUSE")];
    assert_eq!(&s.lookup(DESC), house);
    assert_eq!(
        s.lookup(("LOC_NOTIFICATION_CIVIC_DISCOVERED_SUMMARY", DESC)),
        format!("You have discovered {house}!")
    );
    assert_eq!(s.upper(DESC), s.base_upper(house));
}
#[test]
fn test_engine_built_text_is_switched_for_the_owner() {
    for language in ["en_US", "ja_JP"] {
        let s = House::new(Some("CIVILIZATION_JAC_ODA"), language, "");
        let generic = &s.texts[DESC];
        let house = &s.texts[&format!("{DESC}_HOUSE")];
        let engine =
            s.texts["LOC_NOTIFICATION_CIVIC_DISCOVERED_SUMMARY"].replace("{1_Name}", generic);
        let shown = s.lookup(engine.as_str());
        assert!(shown.contains(house));
        assert!(!shown.contains(generic));
        let moment = s.texts["LOC_MOMENT_TEST"].replace("{1_Era}", &s.texts[ERA]);
        assert_eq!(
            s.lookup(moment.as_str()),
            s.texts["LOC_MOMENT_TEST"].replace("{1_Era}", &s.texts[&format!("{ERA}_HOUSE")])
        );
        assert_eq!(
            s.upper(&s.base_upper(&engine)),
            s.base_upper(&engine.replace(generic, house))
        );
        assert_eq!(s.lookup(s.lookup(engine.as_str()).as_str()), shown);
    }
}
#[test]
fn test_other_houses_and_observers_keep_generic_names() {
    for civilization in [Some("CIVILIZATION_JAC_TAKEDA"), None] {
        let s = House::new(civilization, "en_US", "");
        let generic = &s.texts[DESC];
        let engine = format!("You have discovered {generic}!");
        assert_eq!(&s.lookup(DESC), generic);
        assert_eq!(s.lookup(engine.as_str()), engine);
        assert!(!s.eval::<bool>("JAC_LocalHouseHasNames()"));
    }
}
#[test]
fn test_load_screen_without_game_keeps_generic_names() {
    let s = House::new(
        Some("CIVILIZATION_JAC_ODA"),
        "en_US",
        "Game=nil; PlayerConfigurations=nil",
    );
    let generic = &s.texts[DESC];
    assert_eq!(&s.lookup(DESC), generic);
    assert_eq!(
        &s.lua
            .load("return JAC_ApplyHouseNames(...)")
            .call::<String>(generic.as_str())
            .unwrap(),
        generic
    );
    assert!(!s.eval::<bool>("JAC_LocalHouseHasNames()"));
}
#[test]
fn test_civilopedia_parts_receive_house_text_only_for_the_owner() {
    let pedia = read_repo(format!("{UI}/CivilopediaPage_JAC_HouseNames.lua"));
    let calls = format!(
        r#"
        Do_AddChapterWithParagraphs(nil, nil, "LOC_UI_PEDIA_DESCRIPTION", {{ "{ERA}" }})
        Do_AddHeader(nil, "{DESC}")
        HookupIcon({{ "ICON_X", "{DESC}", "SEARCH" }})
        AddQuote("{ERA}")
        AddRightColumnStatBox("LOC_UI_PEDIA_REQUIREMENTS", function(s)
          s:AddLabel("{ERA}")
          s:AddIconLabel({{ "ICON_X", "{DESC}" }}, "{DESC}")
        end)
    "#
    );
    for (civilization, expect_house) in [
        ("CIVILIZATION_JAC_ODA", true),
        ("CIVILIZATION_JAC_TAKEDA", false),
    ] {
        let s = House::new(
            Some(civilization),
            "en_US",
            include_str!("../fixtures/house/pedia_stubs.lua"),
        );
        exec(&s.lua, &pedia);
        exec(&s.lua, &calls);
        let captions = s.strings("CAPTIONS");
        if expect_house {
            assert!(captions.contains(&s.texts[&format!("{ERA}_HOUSE")]));
            assert!(captions.contains(&s.texts[&format!("{DESC}_HOUSE")]));
            assert!(!captions.contains(&ERA.to_string()));
            assert!(!captions.contains(&DESC.to_string()));
            assert!(captions.contains(&"ICON_X".into()));
            assert!(captions.contains(&"same".into()));
        } else {
            assert!(captions.contains(&ERA.to_string()));
            assert!(captions.contains(&DESC.to_string()));
            assert!(!captions.contains(&s.texts[&format!("{DESC}_HOUSE")]));
        }
    }
}
#[test]
fn test_historic_moment_descriptions_are_switched() {
    let lua = new_lua();
    let texts = load_texts("en_US");
    let mut source = sources();
    source.insert(
        "HistoricMoments".into(),
        Json::from("CHECK = Game.GetHistoryManager()"),
    );
    House::init(&lua, &texts, source, Some("CIVILIZATION_JAC_ODA"), "en_US");
    lua.load(r#"
        local generic = ...
        Game.GetHistoryManager = function()
          return { GetAllMomentsData = function(self, player, level)
                     return { { InstanceDescription = "First civic of the " .. generic .. " era.", Turn = 3 } } end,
                   GetMomentData = function(self, id) return { InstanceDescription = generic, ID = id }, "second" end }
        end
    "#).call::<()>(texts[ERA].as_str()).unwrap();
    exec(&lua, &read_repo(format!("{UI}/JAC_HistoricMoments.lua")));
    let moments = lua
        .load("return CHECK:GetAllMomentsData(0,1)")
        .eval::<Table>()
        .unwrap();
    let moment = moments.get::<Table>(1).unwrap();
    assert_eq!(
        moment.get::<String>("InstanceDescription").unwrap(),
        format!("First civic of the {} era.", texts[&format!("{ERA}_HOUSE")])
    );
    assert_eq!(moment.get::<i64>("Turn").unwrap(), 3);
    let (first, second) = lua
        .load("return CHECK:GetMomentData(7)")
        .eval::<(Table, String)>()
        .unwrap();
    assert_eq!(
        first.get::<String>("InstanceDescription").unwrap(),
        texts[&format!("{ERA}_HOUSE")]
    );
    assert_eq!(
        (first.get::<i64>("ID").unwrap(), second),
        (7, "second".into())
    );
    assert!(lua
        .globals()
        .get::<Option<Table>>("CHECK")
        .unwrap()
        .is_some());
}
