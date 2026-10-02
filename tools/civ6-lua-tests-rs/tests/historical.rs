use civ6_lua_tests_rs::{exec, exec_args, json_to_lua, new_lua, query_json, read_repo};
use mlua::{FromLuaMulti, Lua, Table};
use regex::Regex;
use rusqlite::Connection;
use serde_json::{json, Map, Value as Json};
use std::collections::BTreeMap;

const MOD: &str = "poc/JAC_PoC_HistoricalEvents";
const START_TURN: i64 = 16603;
const CIVS: [&str; 4] = [
    "CIVILIZATION_JAC_ODA",
    "CIVILIZATION_JAC_NAGAO",
    "CIVILIZATION_JAC_DATE",
    "CIVILIZATION_JAC_SHIMAZU",
];
const TABLES: [&str; 6] = [
    "JAC_HistoricalEvents",
    "JAC_HistoricalEventPrereqTechs",
    "JAC_HistoricalEventPrereqCivics",
    "JAC_HistoricalEventTargets",
    "JAC_HistoricalEventModifiers",
    "JAC_HistoricalEventRenames",
];
type Boost = (i64, String, i64, i64, i64);

fn child_text<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> &'a str {
    node.children()
        .find(|n| n.has_tag_name(name))
        .and_then(|n| n.text())
        .unwrap()
}

fn calendar() -> (i64, Vec<Json>) {
    let source = read_repo("poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Calendar.xml");
    let doc = roxmltree::Document::parse(&source).unwrap();
    let start = doc
        .descendants()
        .find(|n| n.has_tag_name("StartEras"))
        .unwrap();
    let year = start
        .descendants()
        .find(|n| n.has_tag_name("Year"))
        .unwrap()
        .text()
        .unwrap()
        .parse()
        .unwrap();
    let rows=doc.descendants().find(|n| n.has_tag_name("GameSpeed_Turns")).unwrap().children().filter(|n| n.has_tag_name("Row")).map(|n| json!({"GameSpeedType":child_text(n,"GameSpeedType"),"TurnsPerIncrement":child_text(n,"TurnsPerIncrement").parse::<i64>().unwrap(),"MonthIncrement":child_text(n,"MonthIncrement").parse::<i64>().unwrap()})).collect();
    (year, rows)
}

fn turn_for_year(year: i64, month: i64) -> i64 {
    let (start, rows) = calendar();
    let target = (year - start) * 12 + month - 1;
    let (mut months, mut turn) = (0, 1);
    for row in rows
        .iter()
        .filter(|row| row["GameSpeedType"] == "GAMESPEED_STANDARD")
    {
        let turns = row["TurnsPerIncrement"].as_i64().unwrap();
        let increment = row["MonthIncrement"].as_i64().unwrap();
        if months + turns * increment >= target {
            return turn + ((target - months).max(0) + increment - 1) / increment;
        }
        months += turns * increment;
        turn += turns;
    }
    turn
}

fn tables() -> Json {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys = ON;
        CREATE TABLE GlobalParameters (Name TEXT PRIMARY KEY, Value TEXT);
        CREATE TABLE Technologies (TechnologyType TEXT PRIMARY KEY);
        CREATE TABLE Civics (CivicType TEXT PRIMARY KEY);
        CREATE TABLE Boosts (BoostID INTEGER PRIMARY KEY, TechnologyType TEXT, CivicType TEXT, Boost INTEGER NOT NULL,
            TriggerDescription TEXT NOT NULL, TriggerLongDescription TEXT NOT NULL, BoostClass TEXT NOT NULL,
            Unit1Type TEXT, ImprovementType TEXT, ResourceType TEXT);
        CREATE TABLE Modifiers (ModifierId TEXT PRIMARY KEY, ModifierType TEXT NOT NULL);
        CREATE TABLE ModifierArguments (ModifierId TEXT, Name TEXT, Value TEXT, PRIMARY KEY (ModifierId, Name));").unwrap();
    let trees = read_repo("poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Trees.sql");
    for (kind, table) in [("KIND_TECH", "Technologies"), ("KIND_CIVIC", "Civics")] {
        for m in Regex::new(&format!(r"\('([A-Z_]+)', '{kind}'\)"))
            .unwrap()
            .captures_iter(&trees)
        {
            db.execute(&format!("INSERT INTO {table} VALUES (?)"), [&m[1]])
                .unwrap();
        }
    }
    for statement in Regex::new(r"(?m)^INSERT INTO Boosts .*?;$")
        .unwrap()
        .find_iter(&trees)
    {
        db.execute_batch(statement.as_str()).unwrap();
    }
    for name in [
        "JAC_HistoricalEvents_Schema.sql",
        "JAC_HistoricalEvents_Sengoku.sql",
    ] {
        db.execute_batch(&read_repo(format!("{MOD}/Data/{name}")))
            .unwrap();
    }
    let mut out = Map::new();
    for name in ["Technologies", "Civics", "Boosts"]
        .into_iter()
        .chain(TABLES)
    {
        let rows = query_json(&db, &format!("SELECT * FROM {name}"))
            .into_iter()
            .map(|mut row| {
                row.as_object_mut().unwrap().retain(|_, v| !v.is_null());
                row
            })
            .collect();
        out.insert(name.into(), Json::Array(rows));
    }
    out.insert("Names".into(), json!(TABLES));
    let (year, rows) = calendar();
    out.insert("StartYear".into(), json!(year));
    out.insert("GameSpeed_Turns".into(), Json::Array(rows));
    Json::Object(out)
}

fn ui_source() -> String {
    let source = read_repo(format!("{MOD}/UI/JAC_HistoricalEventsAdvisor.lua"));
    let declaration = Regex::new(r"(?ms)^hstructure AdvisorItem\r?\n.*?^end\r?\n").unwrap();
    assert_eq!(
        declaration.find_iter(&source).count(),
        1,
        "the original harness rewrites exactly one HavokScript declaration"
    );
    let source = declaration.replace(&source, "").into_owned();
    let construction = Regex::new("hmake AdvisorItem ").unwrap();
    assert_eq!(construction.find_iter(&source).count(), 1);
    construction.replace(&source, "").into_owned()
}

struct Events {
    lua: Lua,
    teppo: i64,
}
impl Events {
    fn new() -> Self {
        Self::with_data(tables())
    }
    fn with_data(data: Json) -> Self {
        let lua = new_lua();
        exec_args(
            &lua,
            include_str!("../fixtures/historical/stubs.lua"),
            (
                json_to_lua(&lua, &data),
                json_to_lua(&lua, &json!(CIVS)),
                START_TURN,
            ),
        );
        let mut s = Self { lua, teppo: 0 };
        s.load_scripts();
        s.teppo = s.tech("TECH_JAC_SENGOKU_FIREARMS_CONTACT");
        s.known(0, "techs", s.teppo, Some(true));
        s
    }
    fn run(&self, source: &str) {
        exec(&self.lua, source);
    }
    fn eval<T: FromLuaMulti>(&self, source: &str) -> T {
        self.lua.load(format!("return {source}")).eval().unwrap()
    }
    fn load_scripts(&self) {
        self.run(&read_repo(format!(
            "{MOD}/Scripts/JAC_HistoricalEvents.lua"
        )));
        self.run(&ui_source());
    }
    fn tech(&self, name: &str) -> i64 {
        self.lua
            .load("return GameInfo.Technologies[...].Index")
            .call(name)
            .unwrap()
    }
    fn civic(&self, name: &str) -> i64 {
        self.lua
            .load("return GameInfo.Civics[...].Index")
            .call(name)
            .unwrap()
    }
    fn known(&self, player: i64, kind: &str, index: i64, value: Option<bool>) {
        self.lua
            .load("local p,k,i,v=...; Players[p][k][i]=v")
            .call::<()>((player, kind, index, value))
            .unwrap();
    }
    fn turns(&self, first: i64, last: i64) {
        let start = self
            .lua
            .globals()
            .get::<mlua::Function>("START_TURN")
            .unwrap();
        for turn in first..=last {
            start.call::<()>(turn).unwrap();
        }
    }
    fn boosts(&self, kind: &str, index: i64) -> Vec<Boost> {
        self.eval::<Table>("BOOSTS")
            .sequence_values::<Table>()
            .map(|b| {
                let b = b.unwrap();
                (
                    b.get(1).unwrap(),
                    b.get::<String>(2).unwrap(),
                    b.get(3).unwrap(),
                    b.get(4).unwrap(),
                    b.get(5).unwrap(),
                )
            })
            .filter(|b| b.1 == kind && b.2 == index)
            .collect()
    }
    fn prop(&self, player: i64, key: &str) -> Option<i64> {
        self.lua
            .load("local p,k=...; return Players[p]:GetProperty(k)")
            .call((player, key))
            .unwrap()
    }
    fn game<T: FromLuaMulti>(&self, key: &str) -> T {
        self.lua
            .load("return Game:GetProperty(...)")
            .call(key)
            .unwrap()
    }
    fn log(&self) -> Vec<String> {
        self.eval::<Table>("LOG")
            .sequence_values()
            .collect::<mlua::Result<_>>()
            .unwrap()
    }
    fn raised(&self) -> Vec<String> {
        self.eval::<Table>("RAISED")
            .sequence_values::<Table>()
            .map(|t| t.unwrap().get("Message").unwrap())
            .collect()
    }
}

#[test]
fn test_teppo_boosts_every_faction_without_the_tech_on_its_due_turn() {
    let s = Events::new();
    s.turns(1, 29);
    assert_eq!(s.boosts("tech", s.teppo), vec![]);
    assert_eq!(s.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO"), Some(1));
    s.turns(30, 31);
    assert_eq!(
        s.boosts("tech", s.teppo),
        (1..=3)
            .map(|p| (p, "tech".into(), s.teppo, 1, 30))
            .collect::<Vec<_>>()
    );
    for player in [1, 2, 3] {
        assert_eq!(
            s.prop(player, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO"),
            Some(30)
        );
        assert_eq!(s.prop(player, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO"), None);
    }
    assert_eq!(s.prop(0, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO"), None);
    assert!(s.log().contains(&"[JAC_EVENTS] boosted event=JAC_EVENT_TEPPO player=1 civ=CIVILIZATION_JAC_NAGAO reason=year turn=30 due=30 boost=tech=boosted".into()));
    s.known(1, "techs", s.teppo, Some(true));
    s.turns(32, 32);
    assert_eq!(s.prop(1, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO"), Some(32));
    assert_eq!(s.boosts("tech", s.teppo).len(), 3);
}
#[test]
fn test_any_one_of_the_prerequisites_fires_the_event_early() {
    let s = Events::new();
    let councils = s.civic("CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS");
    let envoys = s.civic("CIVIC_JAC_SENGOKU_NANBAN_ENVOYS");
    s.known(1, "civics", councils, Some(true));
    s.turns(1, 1);
    assert_eq!(s.prop(1, "JAC_EVENT_FIRED_JAC_EVENT_CHRISTIAN"), Some(1));
    assert_eq!(s.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_CHRISTIAN"), None);
    s.turns(2, turn_for_year(1549, 8));
    assert_eq!(
        s.boosts("civic", envoys)
            .iter()
            .map(|b| b.0)
            .collect::<Vec<_>>(),
        vec![0, 2, 3]
    );
    assert_eq!(s.boosts("civic", councils), vec![]);
}
#[test]
fn test_starting_knowledge_fires_no_event_on_turn_1() {
    let s = Events::new();
    s.known(0, "techs", s.teppo, None);
    s.run(&read_repo(
        "poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_StartingKnowledge.lua",
    ));
    s.run("NEW_GAME_INITIALIZED()");
    let granted = s
        .eval::<Table>("Players")
        .pairs::<i64, Table>()
        .map(|row| {
            let (id, p) = row.unwrap();
            let count = |key| {
                p.get::<Table>(key)
                    .unwrap()
                    .pairs::<i64, bool>()
                    .filter(|row| row.as_ref().unwrap().1)
                    .count()
            };
            (id, (count("techs"), count("civics")))
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        granted,
        (0..CIVS.len() as i64).map(|id| (id, (4, 3))).collect()
    );
    s.turns(1, 1);
    assert_eq!(s.game::<Option<i64>>("JAC_EVENTS_FIRE_COUNT"), None);
    assert_eq!(s.eval::<Table>("BOOSTS").raw_len(), 0);
    assert_eq!(
        s.log()
            .iter()
            .filter(|l| l.contains("reason="))
            .collect::<Vec<_>>(),
        Vec::<&String>::new()
    );
}
#[test]
fn test_events_without_prerequisites_happen_in_their_year_for_their_targets_only() {
    let s = Events::new();
    s.turns(1, 119);
    assert_eq!(s.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_ASAKURA_FALL"), None);
    s.turns(120, 120);
    assert_eq!(
        s.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_ASAKURA_FALL"),
        Some(120)
    );
    assert_eq!(s.prop(1, "JAC_EVENT_FIRED_JAC_EVENT_ASAKURA_FALL"), None);
    assert!(s.log().contains(&"[JAC_EVENTS] fired event=JAC_EVENT_ASAKURA_FALL player=0 civ=CIVILIZATION_JAC_ODA reason=due turn=120 due=120 boost=".into()));
}
#[test]
fn test_an_engine_error_in_one_boost_does_not_stop_the_other_factions() {
    let s = Events::new();
    s.run("FAIL_BOOST[1]=true");
    s.turns(1, 30);
    assert_eq!(
        s.boosts("tech", s.teppo)
            .iter()
            .map(|b| b.0)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert!(s
        .log()
        .iter()
        .any(|line| line.contains("event=JAC_EVENT_TEPPO player=1")
            && line.contains("boost=tech=error:")));
}
#[test]
fn test_a_failed_boost_is_left_unrecorded_and_retried_next_turn() {
    let s = Events::new();
    s.run("FAIL_BOOST[1]=true");
    s.turns(1, 30);
    assert_eq!(s.prop(1, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO"), None);
    assert_eq!(s.prop(2, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO"), Some(30));
    s.run("FAIL_BOOST[1]=false");
    s.turns(31, 32);
    assert_eq!(
        s.boosts("tech", s.teppo)
            .iter()
            .map(|b| (b.0, b.4))
            .collect::<Vec<_>>(),
        vec![(2, 30), (3, 30), (1, 31)]
    );
    assert_eq!(s.prop(1, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO"), Some(31));
}
#[test]
fn test_resource_events_boost_the_sites_research_and_reveal_nothing() {
    let s = Events::new();
    s.run("REVEALED={}; ExposedMembers.JAC_Resources={RevealSite=function(id) table.insert(REVEALED,id) end, IsRevealed=function() return true end, GetSites=function() return {} end}");
    s.known(
        2,
        "civics",
        s.civic("CIVIC_JAC_SENGOKU_CASTLE_TOWN_ORDINANCES"),
        Some(true),
    );
    let sado = turn_for_year(1589, 8);
    s.turns(1, sado);
    assert_eq!(s.prop(2, "JAC_EVENT_FIRED_JAC_EVENT_AIZU_LACQUER"), Some(1));
    assert_eq!(
        s.prop(1, "JAC_EVENT_BOOSTED_JAC_EVENT_SADO_SUBJUGATION"),
        Some(sado)
    );
    let mine = s.tech("TECH_JAC_SENGOKU_MINE_MANAGEMENT");
    assert!(s
        .boosts("tech", mine)
        .contains(&(1, "tech".into(), mine, 1, sado)));
    assert_eq!(s.eval::<Table>("REVEALED").raw_len(), 0);
    assert!(!s.log().iter().any(|l| l.contains("resource event=")));
}
#[test]
fn test_houses_rename_only_by_the_succession_civics() {
    let s = Events::new();
    let envoys = s.civic("CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS");
    let truce = s.civic("CIVIC_JAC_SENGOKU_TRUCE_ARBITRATION");
    s.turns(1, 160);
    assert_eq!(
        s.game::<Option<String>>("JAC_CIV_NAME_CIVILIZATION_JAC_ODA"),
        None
    );
    assert_eq!(s.prop(0, "JAC_EVENT_BOOSTED_JAC_EVENT_HONNOJI"), Some(147));
    assert_eq!(
        s.prop(0, "JAC_EVENT_BOOSTED_JAC_EVENT_TOYOTOMI_SURNAME"),
        Some(160)
    );
    assert!(s
        .boosts("civic", truce)
        .contains(&(0, "civic".into(), truce, 1, 160)));
    s.known(
        0,
        "civics",
        s.civic("CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES"),
        Some(true),
    );
    s.turns(161, 161);
    assert_eq!(s.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_HONNOJI"), Some(161));
    assert_eq!(
        s.game::<Option<String>>("JAC_CIV_NAME_CIVILIZATION_JAC_ODA"),
        None
    );
    s.known(0, "civics", envoys, Some(true));
    s.turns(162, 162);
    assert_eq!(
        s.game::<String>("JAC_CIV_NAME_CIVILIZATION_JAC_ODA"),
        "LOC_JAC_EVENT_HASHIBA_CIV_NAME"
    );
    s.known(0, "civics", truce, Some(true));
    s.turns(163, 164);
    assert_eq!(
        s.game::<String>("JAC_CIV_NAME_CIVILIZATION_JAC_ODA"),
        "LOC_JAC_EVENT_TOYOTOMI_CIV_NAME"
    );
    assert_eq!(
        s.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_TOYOTOMI_SURNAME"),
        Some(163)
    );
    assert_eq!(s.log().iter().filter(|l| l.contains("renamed")).cloned().collect::<Vec<_>>(),vec!["[JAC_EVENTS] renamed civ=CIVILIZATION_JAC_ODA name=LOC_JAC_EVENT_HASHIBA_CIV_NAME player=0 civic=CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS turn=162","[JAC_EVENTS] renamed civ=CIVILIZATION_JAC_ODA name=LOC_JAC_EVENT_TOYOTOMI_CIV_NAME player=0 civic=CIVIC_JAC_SENGOKU_TRUCE_ARBITRATION turn=163"]);
    assert_eq!(
        s.prop(1, "JAC_EVENT_BOOSTED_JAC_EVENT_NAGAO_TO_UESUGI"),
        Some(83)
    );
    assert_eq!(
        s.game::<Option<String>>("JAC_CIV_NAME_CIVILIZATION_JAC_NAGAO"),
        None
    );
    assert_eq!(
        s.game::<Option<String>>("JAC_CIV_NAME_CIVILIZATION_JAC_DATE"),
        None
    );
}
#[test]
fn test_oda_rename_stages_keep_the_latest() {
    let s = Events::new();
    s.known(
        0,
        "civics",
        s.civic("CIVIC_JAC_SENGOKU_TRUCE_ARBITRATION"),
        Some(true),
    );
    s.turns(1, 1);
    assert_eq!(
        s.game::<String>("JAC_CIV_NAME_CIVILIZATION_JAC_ODA"),
        "LOC_JAC_EVENT_TOYOTOMI_CIV_NAME"
    );
    s.known(
        0,
        "civics",
        s.civic("CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS"),
        Some(true),
    );
    s.turns(2, 3);
    assert_eq!(
        s.game::<String>("JAC_CIV_NAME_CIVILIZATION_JAC_ODA"),
        "LOC_JAC_EVENT_TOYOTOMI_CIV_NAME"
    );
}
#[test]
fn test_nagao_becomes_uesugi_only_with_the_succession_civic() {
    let s = Events::new();
    s.known(
        1,
        "civics",
        s.civic("CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES"),
        Some(true),
    );
    s.turns(1, 2);
    assert_eq!(
        s.prop(1, "JAC_EVENT_FIRED_JAC_EVENT_NAGAO_TO_UESUGI"),
        Some(1)
    );
    assert_eq!(
        s.game::<Option<String>>("JAC_CIV_NAME_CIVILIZATION_JAC_NAGAO"),
        None
    );
    s.known(
        1,
        "civics",
        s.civic("CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS"),
        Some(true),
    );
    s.turns(3, 90);
    assert_eq!(
        s.game::<String>("JAC_CIV_NAME_CIVILIZATION_JAC_NAGAO"),
        "LOC_JAC_EVENT_UESUGI_CIV_NAME"
    );
    assert_eq!(s.log().iter().filter(|l|l.contains("renamed") && l.contains("NAGAO")).cloned().collect::<Vec<_>>(),vec!["[JAC_EVENTS] renamed civ=CIVILIZATION_JAC_NAGAO name=LOC_JAC_EVENT_UESUGI_CIV_NAME player=1 civic=CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS turn=3"]);
}
#[test]
fn test_house_specific_name_only_for_its_house() {
    let s = Events::new();
    let tea = s.civic("CIVIC_JAC_SENGOKU_TEA_GATHERINGS");
    for p in 0..CIVS.len() as i64 {
        s.known(p, "civics", tea, Some(true));
    }
    s.turns(1, 1);
    let notes = s
        .eval::<Table>("NOTES")
        .sequence_values::<Table>()
        .filter_map(|t| {
            let t = t.unwrap();
            let message = t.get::<String>(2).unwrap();
            message
                .contains("KITANO")
                .then(|| (t.get::<i64>(1).unwrap(), message))
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        notes,
        [
            (0, "LOC_JAC_EVENT_KITANO_TEA_NAME"),
            (1, "LOC_JAC_EVENT_KITANO_TEA_GENERIC_NAME"),
            (2, "LOC_JAC_EVENT_KITANO_TEA_GENERIC_NAME"),
            (3, "LOC_JAC_EVENT_KITANO_TEA_GENERIC_NAME")
        ]
        .map(|(p, k)| (p, k.into()))
        .into_iter()
        .collect()
    );
}
#[test]
fn test_no_double_firing_after_a_reload() {
    let s = Events::new();
    s.known(
        1,
        "civics",
        s.civic("CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS"),
        Some(true),
    );
    s.turns(1, 2);
    let count = s.game::<Option<i64>>("JAC_EVENTS_FIRE_COUNT");
    let fired = s.log().iter().filter(|l| l.contains("] fired ")).count();
    let notes = s.eval::<Table>("NOTES").raw_len();
    s.run("RELOAD()");
    s.load_scripts();
    s.turns(2, 3);
    assert_eq!(s.game::<Option<i64>>("JAC_EVENTS_FIRE_COUNT"), count);
    assert_eq!(
        s.log().iter().filter(|l| l.contains("] fired ")).count(),
        fired
    );
    assert_eq!(s.eval::<Table>("NOTES").raw_len(), notes);
    assert_eq!(s.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO"), Some(1));
}
#[test]
fn test_calendar_stages_follow_the_speeds_row_order() {
    let mut data = tables();
    data["Speed"] = json!("GAMESPEED_EPIC");
    data["GameInfoSpeedTurns"] = Json::Array(
        data["GameSpeed_Turns"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .cloned()
            .collect(),
    );
    let s = Events::with_data(data);
    s.turns(1, 41);
    assert_eq!(s.boosts("tech", s.teppo), vec![]);
    s.turns(42, 42);
    assert_eq!(
        s.boosts("tech", s.teppo)
            .iter()
            .map(|b| b.0)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}
#[test]
fn test_advisor_popup_for_the_local_human_on_the_turn_the_event_happens() {
    let s = Events::new();
    s.known(
        0,
        "techs",
        s.tech("TECH_JAC_SENGOKU_GRAIN_STORES"),
        Some(true),
    );
    s.known(
        3,
        "civics",
        s.civic("CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER"),
        Some(true),
    );
    s.turns(1, 1);
    assert_eq!(
        s.prop(3, "JAC_EVENT_FIRED_JAC_EVENT_SHIMAZU_UNIFY_1"),
        Some(1)
    );
    s.run("UI_EVENT()");
    assert_eq!(
        s.raised(),
        vec![
            "LOC_JAC_EVENT_TENBUN_FAMINE_NAME[NEWLINE][NEWLINE]LOC_JAC_EVENT_TENBUN_FAMINE_ADVISOR",
            "LOC_JAC_EVENT_TENSHO_QUAKE_NAME[NEWLINE][NEWLINE]LOC_JAC_EVENT_TENSHO_QUAKE_ADVISOR"
        ]
    );
    let item = s.eval::<Table>("RAISED[1]");
    assert!(item.get::<bool>("ShowPortrait").unwrap());
    assert_eq!(
        (
            item.get::<String>("Button1Text").unwrap(),
            item.get::<i64>("OptionsNum").unwrap()
        ),
        ("LOC_JAC_EVENT_ADVISOR_OK".into(), 1)
    );
    assert!(item
        .get::<Option<mlua::Function>>("Button1Func")
        .unwrap()
        .is_some());
    assert_eq!(item.get::<Table>("UITriggers").unwrap().raw_len(), 0);
    assert!(s
        .log()
        .contains(&"[JAC_EVENTS_UI] advisor event=JAC_EVENT_TENBUN_FAMINE player=0 turn=1".into()));
    s.turns(2, 2);
    s.run("UI_EVENT()");
    assert_eq!(s.raised().len(), 2);
}
#[test]
fn test_advisor_popup_for_each_hotseat_human_on_the_same_turn() {
    let s = Events::new();
    for p in [0, 1] {
        s.known(
            p,
            "techs",
            s.tech("TECH_JAC_SENGOKU_GRAIN_STORES"),
            Some(true),
        );
    }
    s.turns(1, 1);
    s.run("UI_EVENT(); LOCAL=1; UI_EVENT()");
    assert_eq!(
        s.raised()
            .iter()
            .filter(|m| m.starts_with("LOC_JAC_EVENT_TENBUN_FAMINE_NAME"))
            .count(),
        2
    );
    let mut keys = s
        .eval::<Table>("JAC_EVENTS_ADVISOR_SHOWN")
        .pairs::<String, bool>()
        .map(|r| r.unwrap().0)
        .filter(|k| k.contains("TENBUN"))
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        vec!["JAC_EVENT_TENBUN_FAMINE@1#0", "JAC_EVENT_TENBUN_FAMINE@1#1"]
    );
}
#[test]
fn test_advisor_popup_on_the_year_for_the_boost_and_not_again_on_research() {
    let s = Events::new();
    s.run("LOCAL=1");
    s.turns(1, 19);
    s.run("UI_EVENT()");
    assert_eq!(s.raised().len(), 1);
    assert_eq!(
        s.prop(1, "JAC_EVENT_BOOSTED_JAC_EVENT_TENBUN_FAMINE"),
        Some(19)
    );
    s.known(
        1,
        "techs",
        s.tech("TECH_JAC_SENGOKU_GRAIN_STORES"),
        Some(true),
    );
    s.turns(20, 20);
    s.run("UI_EVENT()");
    assert_eq!(
        s.prop(1, "JAC_EVENT_FIRED_JAC_EVENT_TENBUN_FAMINE"),
        Some(20)
    );
    assert_eq!(s.raised().len(), 1);
}
#[test]
fn test_no_advisor_popup_without_a_local_player() {
    let s = Events::new();
    s.run("LOCAL=-1");
    s.known(
        0,
        "techs",
        s.tech("TECH_JAC_SENGOKU_GRAIN_STORES"),
        Some(true),
    );
    s.turns(1, 1);
    s.run("UI_EVENT()");
    assert_eq!(s.raised(), Vec::<String>::new());
}
#[test]
fn test_advisor_popup_uses_the_generic_name_for_other_houses() {
    let s = Events::new();
    s.run("for row in GameInfo.JAC_HistoricalEvents() do if row.EventType == 'JAC_EVENT_TENBUN_FAMINE' then row.House='CIVILIZATION_JAC_DATE'; row.GenericName='LOC_GENERIC' end end");
    s.known(
        0,
        "techs",
        s.tech("TECH_JAC_SENGOKU_GRAIN_STORES"),
        Some(true),
    );
    s.turns(1, 1);
    s.run("UI_EVENT()");
    assert!(s.raised()[0].starts_with("LOC_GENERIC[NEWLINE]"));
}
