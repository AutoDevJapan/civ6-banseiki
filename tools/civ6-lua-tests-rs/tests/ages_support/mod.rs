#![allow(dead_code)] // Each integration-test executable uses a different subset of these helpers.

use civ6_lua_tests_rs::{json_to_lua, new_lua, query_json, read_repo, repo_root};
use mlua::{Lua, Table, Value};
use rusqlite::Connection;
use serde_json::{json, Map, Value as Json};
use std::collections::BTreeMap;
pub mod official;

pub const MOD: &str = "poc/JAC_PoC_SengokuTrees/";
pub const START_TURN: i64 = 16603;
pub const SPEEDS: [&str; 5] = [
    "GAMESPEED_ONLINE",
    "GAMESPEED_QUICK",
    "GAMESPEED_STANDARD",
    "GAMESPEED_EPIC",
    "GAMESPEED_MARATHON",
];

pub fn mod_text(path: &str) -> String {
    read_repo(format!("{MOD}{path}"))
}
pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        repo_root()
            .join("tools/civ6-lua-tests-rs/fixtures/ages")
            .join(name),
    )
    .unwrap()
}
pub fn reigns() -> Json {
    serde_json::from_str(&read_repo("documents/spec/data/sengoku-reigns.json")).unwrap()
}

pub fn calendar() -> BTreeMap<String, Vec<(i64, i64)>> {
    let text = mod_text("Data/JAC_Sengoku_Calendar.xml");
    let doc = roxmltree::Document::parse(&text).unwrap();
    let mut out: BTreeMap<String, Vec<(i64, i64)>> = BTreeMap::new();
    for row in doc.descendants().filter(|n| n.has_tag_name("Row")) {
        let field = |name| {
            row.children()
                .find(|n| n.has_tag_name(name))
                .and_then(|n| n.text())
        };
        if let (Some(speed), Some(inc), Some(turns)) = (
            field("GameSpeedType"),
            field("MonthIncrement"),
            field("TurnsPerIncrement"),
        ) {
            out.entry(speed.to_string())
                .or_default()
                .push((inc.parse().unwrap(), turns.parse().unwrap()));
        }
    }
    out
}
pub fn month_offsets() -> Vec<i64> {
    reigns()["reigns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["year"].as_i64().unwrap() - 1534) * 12 + r["month"].as_i64().unwrap() - 1)
        .collect()
}
pub fn calendar_turn(month: i64, rows: &[(i64, i64)]) -> i64 {
    let mut current_month = 0;
    let mut turn = 1;
    for &(increment, count) in rows {
        if month < current_month + increment * count {
            return turn + (month - current_month) / increment;
        }
        current_month += increment * count;
        turn += count;
    }
    turn
}
pub fn standard_turns() -> Vec<i64> {
    month_offsets().iter().map(|&m| 1 + m / 4).collect()
}
pub fn era_rows() -> Vec<Json> {
    let source = reigns();
    let mut all = vec![source["start"].clone()];
    all.extend(source["reigns"].as_array().unwrap().iter().cloned());
    let turns = standard_turns();
    let official = [
        "ERA_ANCIENT",
        "ERA_CLASSICAL",
        "ERA_MEDIEVAL",
        "ERA_RENAISSANCE",
        "ERA_INDUSTRIAL",
        "ERA_MODERN",
        "ERA_ATOMIC",
        "ERA_INFORMATION",
        "ERA_FUTURE",
    ];
    all.iter().enumerate().map(|(i,r)| json!({
        "EraType": if i < official.len() { official[i].to_string() } else { format!("ERA_JAC_{}", r["id"].as_str().unwrap()) },
        "ChronologyIndex": i+1,
        "length": if i < turns.len() { Some(turns[i] - if i == 0 {1} else {turns[i-1]}) } else { None },
    })).collect()
}
pub fn engine_turns(speed: &str) -> Vec<i64> {
    let multiplier = multiplier(speed);
    let mut turn = 1;
    era_rows()
        .into_iter()
        .filter_map(|e| e["length"].as_i64())
        .map(|length| {
            turn += (length * multiplier / 100).max(2);
            turn
        })
        .collect()
}
pub fn multiplier(speed: &str) -> i64 {
    match speed {
        "GAMESPEED_ONLINE" => 50,
        "GAMESPEED_QUICK" => 67,
        "GAMESPEED_STANDARD" => 100,
        "GAMESPEED_EPIC" => 150,
        "GAMESPEED_MARATHON" => 300,
        _ => panic!("{speed}"),
    }
}

pub fn ages_tables() -> Json {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
    db.execute_batch("CREATE TABLE Eras (EraType TEXT PRIMARY KEY);")
        .unwrap();
    for era in era_rows() {
        db.execute(
            "INSERT INTO Eras VALUES (?1)",
            [era["EraType"].as_str().unwrap()],
        )
        .unwrap();
    }
    for name in [
        "Data/JAC_Sengoku_Ages.sql",
        "Data/JAC_Sengoku_Reigns.sql",
        "Data/JAC_Sengoku_Oaths.sql",
    ] {
        let clean = mod_text(name)
            .lines()
            .map(|line| line.split("--").next().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        for statement in clean.split(';') {
            let s = statement.trim();
            if s.starts_with("CREATE TABLE JAC_") || s.starts_with("INSERT INTO JAC_") {
                db.execute_batch(s)
                    .unwrap_or_else(|error| panic!("{name}: {s}: {error}"));
            }
        }
    }
    let mut out = Map::new();
    for name in [
        "JAC_SengokuReigns",
        "JAC_SengokuDarkAgePolicies",
        "JAC_SengokuAgeEventScores",
        "JAC_SengokuDarkAgePolicyDefaults",
        "JAC_SengokuOaths",
        "JAC_SengokuOathAges",
    ] {
        out.insert(
            name.to_string(),
            Json::Array(query_json(&db, &format!("SELECT * FROM {name}"))),
        );
    }
    let eras = era_rows();
    out.insert(
        "Eras".into(),
        Json::Array(
            eras.iter()
                .map(|e| json!({"EraType":e["EraType"],"ChronologyIndex":e["ChronologyIndex"]}))
                .collect(),
        ),
    );
    out.insert(
        "Eras_XP1".into(),
        Json::Array(
            eras.iter()
                .map(|e| json!({"EraType":e["EraType"],"GameEraMaximumTurns":e["length"]}))
                .collect(),
        ),
    );
    out.insert(
        "Multiplier".into(),
        json!(SPEEDS
            .iter()
            .map(|s| (s.to_string(), multiplier(s)))
            .collect::<BTreeMap<_, _>>()),
    );
    out.insert("StartYear".into(), json!(1534));
    out.insert(
        "OathLua".into(),
        json!(mod_text("Scripts/JAC_Sengoku_Oaths.lua")),
    );
    Json::Object(out)
}

pub fn value_json(value: Value) -> Json {
    match value {
        Value::Nil => Json::Null,
        Value::Boolean(v) => json!(v),
        Value::Integer(v) => json!(v),
        Value::Number(v) => json!(v),
        Value::String(v) => json!(v.to_str().unwrap().to_string()),
        Value::Table(t) => {
            let mut map = Map::new();
            for pair in t.pairs::<Value, Value>() {
                let (k, v) = pair.unwrap();
                let key = match k {
                    Value::String(s) => s.to_str().unwrap().to_string(),
                    Value::Integer(i) => i.to_string(),
                    Value::Number(n) => n.to_string(),
                    _ => panic!("unsupported key {k:?}"),
                };
                map.insert(key, value_json(v));
            }
            Json::Object(map)
        }
        other => panic!("unsupported Lua value {other:?}"),
    }
}

pub struct World {
    pub lua: Lua,
}
impl World {
    pub fn new() -> Self {
        Self::with_civs(
            &[
                "CIVILIZATION_JAC_ODA",
                "CIVILIZATION_JAC_TAKEDA",
                "CIVILIZATION_JAC_HOJO",
                "CIVILIZATION_JAC_IMAGAWA",
            ],
            "GAMESPEED_STANDARD",
            true,
        )
    }
    pub fn with_civs(civs: &[&str], speed: &str, cities: bool) -> Self {
        Self::with_civs_and_script(civs, speed, cities, true)
    }
    pub fn with_civs_and_script(
        civs: &[&str],
        speed: &str,
        cities: bool,
        load_script: bool,
    ) -> Self {
        let lua = new_lua();
        lua.load("local old=load; load=function(source,name,mode,env) local f=old(source,name); if f and env then setfenv(f,env) end; return f end").exec().unwrap();
        let tables = json_to_lua(&lua, &ages_tables());
        let civs = json_to_lua(&lua, &json!(civs));
        lua.load(fixture("ledger_stubs.lua"))
            .call::<()>((tables, civs, START_TURN, speed))
            .unwrap();
        let world = Self { lua };
        if cities {
            for (owner, x, y, capital) in [
                (0, 1, 1, true),
                (0, 2, 1, false),
                (1, 5, 5, true),
                (1, 6, 5, false),
                (2, 9, 9, true),
                (3, 12, 12, true),
                (3, 13, 12, false),
                (3, 14, 12, false),
            ] {
                world.run(&format!("FOUND({owner},{x},{y},{capital},true)"));
            }
        }
        if load_script {
            world.load_script();
        }
        world
    }
    pub fn run(&self, source: &str) {
        self.lua
            .load(source)
            .exec()
            .unwrap_or_else(|e| panic!("{source}: {e}"));
    }
    pub fn j(&self, expr: &str) -> Json {
        value_json(
            self.lua
                .load(format!("return {expr}"))
                .eval()
                .unwrap_or_else(|e| panic!("{expr}: {e}")),
        )
    }
    pub fn array(&self, expr: &str) -> Vec<Json> {
        let value = self.j(expr);
        let object = value.as_object().unwrap();
        (1..=object.len())
            .map(|i| object[&i.to_string()].clone())
            .collect()
    }
    pub fn turns(&self, first: i64, last: i64) {
        for turn in first..=last {
            self.run(&format!("START_TURN({turn})"));
        }
    }
    pub fn turn(&self, turn: i64) {
        self.turns(turn, turn);
    }
    pub fn load_script(&self) {
        self.run(&format!(
            "assert(load({:?}, 'JAC_Sengoku_Ages'))()",
            mod_text("Scripts/JAC_Sengoku_Ages.lua")
        ));
    }
    pub fn reload(&self) {
        self.run("RESET_HANDLERS()");
        self.load_script();
    }
    pub fn ui_base(&self) {
        let source = mod_text("UI/JAC_SengokuAgesView.lua");
        self.lua
            .load(fixture("ui_stubs.lua"))
            .call::<()>(source)
            .unwrap();
    }
    pub fn screen_base(&self) {
        self.ui_base();
        let files = screen_files();
        self.lua
            .load(fixture("screen_stubs.lua"))
            .call::<()>(json_to_lua(&self.lua, &files))
            .unwrap();
    }
    pub fn context(&self, name: &str, sources: &[String]) -> Table {
        let f: mlua::Function = self.lua.globals().get("NEW_CONTEXT").unwrap();
        let args: Vec<Value> =
            std::iter::once(Value::String(self.lua.create_string(name).unwrap()))
                .chain(
                    sources
                        .iter()
                        .map(|s| Value::String(self.lua.create_string(s).unwrap())),
                )
                .collect();
        f.call(mlua::MultiValue::from_vec(args)).unwrap()
    }
    pub fn snapshot(&self) -> Json {
        self.j("LEDGER()")
    }
    pub fn bind_context(&self, name: &str, context: Table) {
        self.lua.globals().set(name, context).unwrap();
    }
    pub fn turns_with_oath(&self, first: i64, last: i64) {
        self.turns(first, last);
        self.run("local l=LEDGER(); local h=l.houses.p1; if l.reign>0 and h.oathChosen~=l.reign then local picked={Reign=l.reign}; for i,r in pairs(JAC_Oaths_Choices(h)) do if i==1 or h.age=='HEROIC' then picked['Oath'..i]=r.OathType end end; CHOOSE_OATHS(1,picked) end");
    }
    pub fn house(&self, pid: i64, field: &str) -> Json {
        self.j(&format!("LEDGER().houses.p{pid}.{field}"))
    }
    pub fn parts(&self, pid: i64) -> Json {
        self.house(pid, "parts")
    }
    pub fn choose(&self, pid: i64, policy: Option<&str>, reign: i64) {
        let policy = policy.map(|s| format!("{:?}", s)).unwrap_or("nil".into());
        self.run(&format!(
            "CHOOSE({pid},{{PolicyType={policy},Reign={reign}}})"
        ));
    }
    pub fn plot_property(&self, x: i64, y: i64, name: &str) -> Json {
        self.j(&format!("Map.GetPlot({x},{y}):GetProperty({:?})", name))
    }
    pub fn plot_properties(&self) -> Json {
        let names = [
            "JAC_AGE_GOLDEN",
            "JAC_AGE_POLICY_VALOR",
            "JAC_AGE_POLICY_ELITE",
            "JAC_AGE_POLICY_PIRATE",
            "JAC_AGE_POLICY_TEMPLE",
            "JAC_AGE_ANCHOR_VALOR",
            "JAC_AGE_ANCHOR_ELITE",
            "JAC_AGE_ANCHOR_PIRATE",
            "JAC_AGE_ANCHOR_TEMPLE",
        ];
        let mut map = Map::new();
        let plots = self.j(
            "(function() local ids={} for id in pairs(PLOTS) do ids[id]=true end return ids end)()",
        );
        for index in plots.as_object().unwrap().keys() {
            let x: i64 = index.parse::<i64>().unwrap() % 100;
            let y: i64 = index.parse::<i64>().unwrap() / 100;
            map.insert(
                index.clone(),
                Json::Array(
                    names
                        .iter()
                        .map(|name| self.plot_property(x, y, name))
                        .collect(),
                ),
            );
        }
        Json::Object(map)
    }
    pub fn policy_flags(&self, x: i64, y: i64) -> Json {
        let mut map = Map::new();
        for kind in ["VALOR", "ELITE", "PIRATE", "TEMPLE"] {
            map.insert(
                kind.into(),
                json!([
                    self.plot_property(x, y, &format!("JAC_AGE_POLICY_{kind}")),
                    self.plot_property(x, y, &format!("JAC_AGE_ANCHOR_{kind}"))
                ]),
            );
        }
        Json::Object(map)
    }
    pub fn relations(&self, before_ledger: bool) {
        self.run("ALLIED['2-3']=true; ALLIED['0-1']=true; FRIENDS['2-3']=true; FRIENDS['0-1']=true; Game:SetProperty('JAC_1534_ALLIANCE_HOJO-IMAGAWA',16603); Game:SetProperty('JAC_1534_ALLIANCE_ODA-TAKEDA',16603)");
        if before_ledger {
            self.run("RESET_HANDLERS()");
        }
        self.run("GameEvents.OnGameTurnStarted.Add(function() for _,pair in ipairs({{'2-3','HOJO-IMAGAWA'},{'0-1','ODA-TAKEDA'}}) do local key='JAC_1534_ALLIANCE_'..pair[2]; if type(Game:GetProperty(key))=='number' and not ALLIED[pair[1]] then Game:SetProperty(key,false) end end end)");
        if before_ledger {
            self.load_script();
        }
    }
    pub fn lapse_at_32(&self) {
        self.turns(1, 31);
        self.run("ALLIED['2-3']=false; ALLIED['0-1']=false; FRIENDS['2-3']=false");
        self.turns(32, 34);
    }
}
pub fn screen_files() -> Json {
    json!({
        "JAC_HouseNames":read_repo("poc/JAC_PoC_BanseikiRuleset/UI/JAC_HouseNames.lua"),
        "JAC_HouseNamesData_Sengoku":mod_text("UI/JAC_HouseNamesData_Sengoku.lua"),
        "JAC_SengokuAgesView":mod_text("UI/JAC_SengokuAgesView.lua"),
        "JAC_EraProgressPanel_Sengoku":mod_text("UI/JAC_EraProgressPanel_Sengoku.lua"),
    })
}
