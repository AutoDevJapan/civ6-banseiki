use crate::common::{count, rows, rows_params};
use civ6_assets_rs::{arr, n, read_json, s};
use mlua::{Lua, Table};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, path::Path};

fn into_lua(lua: &Lua, value: &Value) -> mlua::Value {
    match value {
        Value::Null => mlua::Value::Nil,
        Value::Bool(value) => mlua::Value::Boolean(*value),
        Value::Number(value) => mlua::Value::Number(value.as_f64().unwrap()),
        Value::String(value) => mlua::Value::String(lua.create_string(value).unwrap()),
        Value::Array(values) => {
            let table = lua.create_table().unwrap();
            for (index, value) in values.iter().enumerate() {
                table.set(index + 1, into_lua(lua, value)).unwrap();
            }
            mlua::Value::Table(table)
        }
        Value::Object(values) => {
            let table = lua.create_table().unwrap();
            for (key, value) in values {
                table.set(key.as_str(), into_lua(lua, value)).unwrap();
            }
            mlua::Value::Table(table)
        }
    }
}

// Read the generated map independently of the scenario generator and its templates.
fn state(maps: &Path, item: &Value, scenario: &Value) -> Value {
    let db = Connection::open_with_flags(
        maps.join(s(&item["file"])),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Players"), 22);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Cities"), 56);
    let configuration = &scenario["steps"][1];
    assert_eq!(arr(&configuration["players"]).len(), 22);
    let source_players = rows(
        &db,
        "SELECT ID,LeaderType,CivilizationType FROM Players ORDER BY ID",
    );
    let source_ids = arr(&configuration["players"])
        .iter()
        .map(|p| {
            let source = source_players.iter().find(|r| r[1] == p["leader"]).unwrap();
            assert_eq!(source[2], p["civ"]);
            n(&source[0])
        })
        .collect::<Vec<_>>();
    assert_eq!(source_ids.iter().collect::<BTreeSet<_>>().len(), 22);
    let local_player = arr(&configuration["players"])
        .iter()
        .position(|p| p["human"] == true)
        .unwrap();
    let mut properties = Vec::new();
    let mut building_properties = Vec::new();
    let players = arr(&configuration["players"]).iter().enumerate().map(|(slot,p)| {
        let source_id = source_ids[slot];
        let capital = rows_params(&db, "SELECT Plot FROM StartPositions WHERE Value=?", [s(&p["leader"])])[0][0].clone();
        let cities = rows_params(&db, "SELECT c.Plot,CAST(a.Value AS INTEGER) FROM Cities c JOIN CityAttributes a ON a.ID=c.Plot AND a.Type='Population' AND a.Name='Initial' WHERE c.Owner=? ORDER BY c.Plot", [source_id]).into_iter().map(|r| {
            let buildings = rows_params(&db, "SELECT BuildingType FROM Buildings WHERE Plot=?", [n(&r[0])]).into_iter().map(|b| (s(&b[0]).to_owned(), json!(true))).collect::<serde_json::Map<_,_>>();
            json!({"plot":r[0],"pop":r[1],"capital":r[0]==capital,"buildings":buildings})
        }).collect::<Vec<_>>();
        let units = rows_params(&db, "SELECT UnitType,Plot FROM Units WHERE Owner=? ORDER BY ID", [source_id]).into_iter().map(|r| json!({"kind":r[0],"plot":r[1]})).collect::<Vec<_>>();
        let gold = rows_params(&db, "SELECT CAST(Value AS INTEGER) FROM PlayerAttributes WHERE ID=? AND Type='Treasury' AND Name='Balance'", [source_id])[0][0].clone();
        let districts = n(&rows_params(&db, "SELECT COUNT(*) FROM Districts d JOIN Cities c ON c.Plot=d.CityID WHERE c.Owner=? AND d.DistrictType<>'DISTRICT_CITY_CENTER'", [source_id])[0][0]);
        let improvements = n(&rows_params(&db, "SELECT COUNT(*) FROM PlotImprovements WHERE ImprovementOwner=?", [source_id])[0][0]);
        let buildings = rows_params(&db, "SELECT b.BuildingType,COUNT(*) FROM Buildings b JOIN Districts d ON d.Plot=b.Plot JOIN Cities c ON c.Plot=d.CityID WHERE c.Owner=? GROUP BY b.BuildingType ORDER BY b.BuildingType", [source_id]);
        let walls = buildings.iter().find(|r| r[0] == "BUILDING_WALLS").map_or(0, |r| n(&r[1]));
        let values = [cities.len() as i32,cities.iter().map(|c| n(&c["pop"])).sum(),units.len() as i32,districts,walls,improvements,n(&gold),cities.len() as i32,0];
        properties.push(format!("{slot}={}", values.iter().map(i32::to_string).collect::<Vec<_>>().join(",")));
        building_properties.push(format!("{slot}:{}", buildings.iter().map(|r| format!("{}={}",s(&r[0]),n(&r[1]))).collect::<Vec<_>>().join(",")));
        json!({"leader":p["leader"],"civ":p["civ"],"cities":cities,"units":units,"gold":gold})
    }).collect::<Vec<_>>();
    let plots = rows(&db, "SELECT p.ID,p.TerrainType,COALESCE(o.Owner,-1),COALESCE(r.ResourceType,'') FROM Plots p LEFT JOIN PlotOwners o ON o.ID=p.ID LEFT JOIN PlotResources r ON r.ID=p.ID ORDER BY p.ID").into_iter().map(|r| {
        let owner = source_ids.iter().position(|id| *id == n(&r[2])).map_or(-1, |slot| slot as i32);
        json!({"id":r[0],"water":matches!(s(&r[1]),"TERRAIN_COAST"|"TERRAIN_OCEAN"),"owner":owner,"resource":r[3]})
    }).collect::<Vec<_>>();
    let foreign = plots
        .iter()
        .find(|p| {
            p["water"] == false && n(&p["owner"]) >= 0 && n(&p["owner"]) != local_player as i32
        })
        .unwrap();
    json!({"width":item["width"],"height":item["height"],"players":players,"plots":plots,"localPlayer":local_player,"foreignPlot":foreign["id"],
        "unitTypes":rows(&db,"SELECT DISTINCT UnitType FROM Units ORDER BY UnitType").iter().map(|r|s(&r[0])).collect::<Vec<_>>(),
        "resources":arr(&item["resource_candidates"]).iter().map(|r|s(&r["resource_id"])).collect::<BTreeSet<_>>(),
        "properties":{"JAC_JAPAN_START_STATE":properties.join(";"),"JAC_JAPAN_START_BUILDINGS":building_properties.join(";")}})
}

fn vm(state: &Value) -> Lua {
    let lua = Lua::new();
    lua.globals().set("State", into_lua(&lua, state)).unwrap();
    lua.load(include_str!("../fixtures/generated-civ-stubs.lua"))
        .set_name("generated Civ stubs")
        .exec()
        .unwrap();
    lua
}
fn execute(lua: &Lua, name: &str, code: &str) -> String {
    lua.load(code)
        .set_name(name)
        .eval()
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn rejects(state: &Value, name: &str, code: &str, mutation: &str, message: &str) {
    let lua = vm(state);
    lua.load(mutation).exec().unwrap();
    let error = lua
        .load(code)
        .set_name(name)
        .eval::<String>()
        .expect_err(message)
        .to_string();
    assert!(error.contains(message), "{name}, {mutation}: {error}");
}

fn resource_vm(state: &Value, resource_file: &Path) -> Lua {
    let lua = vm(state);
    lua.load(fs::read_to_string(resource_file).unwrap())
        .set_name(resource_file.to_string_lossy())
        .exec()
        .unwrap();
    lua.load("AttachResourceAPI(); ReleaseSites(1534)")
        .exec()
        .unwrap();
    lua
}
fn rejects_resource(state: &Value, file: &Path, code: &str, mutation: &str, message: &str) {
    let lua = resource_vm(state, file);
    lua.load(mutation).exec().unwrap();
    let error = lua
        .load(code)
        .eval::<String>()
        .expect_err(message)
        .to_string();
    assert!(error.contains(message), "{mutation}: {error}");
}

pub fn verify(maps: &Path, mod_dir: &Path, scenarios: &Path) {
    let manifest = read_json(maps.join("manifest.json")).unwrap();
    assert_eq!(arr(&manifest).len(), 2);
    let mut map_scripts = BTreeSet::new();
    let mut map_scenarios = BTreeSet::new();
    let mut scenario_chunks = 0;
    for (km, width, height, land) in [(20, 154, 67, 1082), (15, 204, 88, 1906)] {
        let item = arr(&manifest)
            .iter()
            .find(|r| n(&r["km_per_tile"]) == km)
            .unwrap();
        assert_eq!(
            (
                n(&item["width"]),
                n(&item["height"]),
                n(&item["land_tiles"])
            ),
            (width, height, land)
        );
        for suffix in ["", "_Settlers"] {
            let path = maps.join(format!("JAC_Japan_{km}km{suffix}.lua"));
            let lua = Lua::new();
            lua.load("LuaEvents={NewGameInitialized={handlers={},Add=function(f) assert(type(f)=='function'); table.insert(LuaEvents.NewGameInitialized.handlers,f) end}}; ExposedMembers={}").exec().unwrap();
            lua.load(fs::read_to_string(&path).unwrap())
                .set_name(path.to_string_lossy())
                .exec()
                .unwrap();
            lua.load("assert(_VERSION=='Lua 5.1'); assert(#LuaEvents.NewGameInitialized.handlers==1); assert(LuaEvents.NewGameInitialized.handlers[1]==InitializeNewGame); assert(InitializeNewGame()==nil); assert(LuaEvents.NewGameInitialized.handlers[1]()==nil); assert(next(ExposedMembers)==nil)").exec().unwrap();
            assert!(map_scripts.insert(path));
        }
        for human in ["oda", "takeda"] {
            let suffix = if human == "oda" {
                String::new()
            } else {
                format!("-{human}")
            };
            let name = format!("japan-map-{km}km{suffix}.json");
            let scenario = read_json(scenarios.join(&name)).unwrap();
            let lua_steps = arr(&scenario["steps"])
                .iter()
                .enumerate()
                .filter(|(_, step)| step["action"] == "ui_lua")
                .map(|(index, _)| index)
                .collect::<BTreeSet<_>>();
            let mut executed_steps = BTreeSet::new();
            assert_eq!(
                scenario["steps"][1]["map_file"],
                format!("{{403be889-1455-4de7-967b-ad54c0ff3431}}Maps/JAC_Japan_{km}km.Civ6Map")
            );
            let data = state(maps, item, &scenario);
            let lua = vm(&data);
            let start = s(&scenario["steps"][3]["code"]);
            let result = execute(&lua, &name, start);
            assert!(executed_steps.insert(3));
            assert!(result.starts_with("22 majors, 0 city-states, 56 cities/capitals, all land owned, no settlers; passage="), "{result}");
            // Loading twice must replace, rather than accumulate, the conversion watch.
            execute(&lua, &name, start);
            lua.load("assert(#Events.CulturalIdentityCityConverted.handlers==1); assert(Events.CulturalIdentityCityConverted.added==2); assert(Events.CulturalIdentityCityConverted.removed==1)").exec().unwrap();
            for (mutation,message) in [
                ("State.width=State.width+1","map grid mismatch"),
                ("AddMinor('CIVILIZATION_MINOR',0)","city-states present: 1"),
                ("AddMinor('CIVILIZATION_FREE_CITIES',1)","Free Cities own 1 cities"),
                ("State.localPlayer=-1","no local player"),
                ("State.players[1].alive=false","major count"),
                ("State.players[2].leader=State.players[1].leader","leader to map player mapping"),
                ("State.players[1].civ='WRONG'","faction mismatch"),
                ("table.remove(State.players[1].cities)","city count"),
                ("State.players[1].units={}","capital garrison missing"),
                ("for _,c in ipairs(State.players[1].cities) do c.buildings.BUILDING_PALACE=false end; Players[0].GetCities=function() return {GetCount=function() return #State.players[1].cities end, Members=function() return ipairs(State.players[1].cities) end, GetCapitalCity=function() return {GetX=function() return -1 end,GetY=function() return -1 end} end} end","capital mismatch"),
                ("for _,p in ipairs(State.plots) do if not p.water then p.owner=-1; break end end","unowned land plot"),
                ("for _,p in ipairs(State.plots) do if not p.water then p.owner=(p.owner+1)%22; break end end","initial territory owners mismatch"),
                ("local u=State.players[1].units[1]; table.insert(State.players[1].units,{GetX=u.GetX,GetY=u.GetY,GetUnitType=function() return 'UNIT_SETTLER' end})","settler in slot"),
                ("State.players[1].cities[1].loyalty=-1","negative initial city loyalty/turn"),
                ("for _,c in ipairs(State.players[1].cities) do if c.capital then c.buildings.BUILDING_PALACE=false end end","palace mismatch"),
            ] {
                rejects(&data,&name,start,mutation,message);
            }
            if human == "oda" {
                rejects(
                    &data,
                    &name,
                    start,
                    "State.allowPassage=false",
                    "no initial route into peaceful foreign territory",
                );
                lua.load("State.turn=51").exec().unwrap();
                let after = s(&scenario["steps"][5]["code"]);
                let snapshot = s(&scenario["steps"][9]["code"]);
                let capture = execute(&lua, &name, after);
                assert_eq!(execute(&lua, &name, snapshot), capture);
                assert!(executed_steps.insert(5));
                assert!(executed_steps.insert(9));
                assert!(capture.starts_with("city counts (start->now)=ODA=2->2|"));
                let state_text = capture.split("; state=").nth(1).unwrap();
                assert_eq!(state_text.split(',').count(), 48);
                assert_eq!(
                    state_text.split(',').nth(46).unwrap().split('|').count(),
                    56
                );
                lua.load("Events.CulturalIdentityCityConverted.Fire(22,1,nil); assert(#JAC_JAPAN_LOYALTY_WATCH.majorDefections==0); Events.CulturalIdentityCityConverted.Fire(22,1,0); assert(#JAC_JAPAN_LOYALTY_WATCH.majorDefections==1)").exec().unwrap();
                let error = lua.load(after).eval::<String>().unwrap_err().to_string();
                assert!(error.contains("major cities lost by loyalty during autoplay"));
                rejects(
                    &data,
                    &name,
                    after,
                    "State.turn=51",
                    "loyalty conversion watch lost",
                );
                rejects(
                    &data,
                    &name,
                    snapshot,
                    "State.turn=50",
                    "50-turn state lost",
                );
                rejects(
                    &data,
                    &name,
                    snapshot,
                    "State.turn=51; State.players[1].cities[1].loyalty=-1",
                    "negative city loyalty/turn",
                );
                lua.load("for _,p in ipairs(State.plots) do if not p.water then p.owner=(p.owner+1)%22; break end end").exec().unwrap();
                assert_ne!(
                    execute(&lua, &name, snapshot),
                    capture,
                    "territory must affect save/load capture"
                );
            }
            assert_eq!(
                executed_steps, lua_steps,
                "all generated Lua steps in {name}"
            );
            assert_eq!(executed_steps.len(), if human == "oda" { 3 } else { 1 });
            scenario_chunks += executed_steps.len();
            assert!(map_scenarios.insert(name));
        }
    }
    assert_eq!(map_scripts.len(), 4);
    assert_eq!(map_scenarios.len(), 4);
    assert_eq!(scenario_chunks, 8);
    verify_resources(maps, mod_dir, scenarios, &manifest);
    verify_prosperous(maps, scenarios, &manifest);
}

fn verify_resources(maps: &Path, mod_dir: &Path, scenarios: &Path, manifest: &Value) {
    let resource_file = mod_dir.join("Scripts/JAC_ResourceSites.lua");
    let scenario = read_json(scenarios.join("japan-resources.json")).unwrap();
    assert_eq!(
        scenario["steps"][1]["map_file"],
        "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map"
    );
    let item = arr(manifest)
        .iter()
        .find(|r| r["km_per_tile"] == 15)
        .unwrap();
    let data = state(maps, item, &scenario);
    let lua = resource_vm(&data, &resource_file);
    let sites: Table = lua
        .load("return ExposedMembers.JAC_ResourceSiteData")
        .eval()
        .unwrap();
    assert_eq!(sites.clone().pairs::<String, Table>().count(), 2);
    // Captured source inventory: exact map-specific coordinates, resource IDs, years and optional techs.
    let baseline = [
        (
            "154x67",
            vec![
                ("silver-ikuno", 6089, "RESOURCE_SILVER", 1542, "CUPELLATION"),
                (
                    "silver-sado-tsurushi",
                    6112,
                    "RESOURCE_SILVER",
                    1542,
                    "CUPELLATION",
                ),
                (
                    "copper-ashio",
                    4416,
                    "RESOURCE_COPPER",
                    1550,
                    "ORE_PROSPECTING",
                ),
                (
                    "gold-izu-toi",
                    3483,
                    "RESOURCE_JAC_GOLD",
                    1577,
                    "MINE_MANAGEMENT",
                ),
                (
                    "gold-noto-hodatsu",
                    5948,
                    "RESOURCE_JAC_GOLD",
                    1586,
                    "MINE_MANAGEMENT",
                ),
                ("lacquer-aizu", 4882, "RESOURCE_JAC_LACQUER", 1590, ""),
                (
                    "gold-sado-aikawa",
                    5958,
                    "RESOURCE_JAC_GOLD",
                    1596,
                    "MINE_MANAGEMENT",
                ),
                (
                    "silver-nobesawa",
                    5196,
                    "RESOURCE_SILVER",
                    1596,
                    "MINE_MANAGEMENT",
                ),
            ],
        ),
        (
            "204x88",
            vec![
                (
                    "silver-ikuno",
                    10719,
                    "RESOURCE_SILVER",
                    1542,
                    "CUPELLATION",
                ),
                (
                    "silver-sado-tsurushi",
                    10545,
                    "RESOURCE_SILVER",
                    1542,
                    "CUPELLATION",
                ),
                (
                    "copper-ashio",
                    7482,
                    "RESOURCE_COPPER",
                    1550,
                    "ORE_PROSPECTING",
                ),
                (
                    "gold-izu-toi",
                    6042,
                    "RESOURCE_JAC_GOLD",
                    1577,
                    "MINE_MANAGEMENT",
                ),
                (
                    "gold-noto-hodatsu",
                    10328,
                    "RESOURCE_JAC_GOLD",
                    1586,
                    "MINE_MANAGEMENT",
                ),
                ("lacquer-aizu", 8304, "RESOURCE_JAC_LACQUER", 1590, ""),
                (
                    "gold-sado-nishimikawa",
                    10340,
                    "RESOURCE_JAC_GOLD",
                    1593,
                    "MINE_MANAGEMENT",
                ),
                (
                    "gold-sado-aikawa",
                    10544,
                    "RESOURCE_JAC_GOLD",
                    1596,
                    "MINE_MANAGEMENT",
                ),
                (
                    "silver-nobesawa",
                    8924,
                    "RESOURCE_SILVER",
                    1596,
                    "MINE_MANAGEMENT",
                ),
            ],
        ),
    ];
    let mut checked = 0;
    for (size, expected) in baseline {
        let actual = sites
            .get::<Table>(size)
            .unwrap()
            .sequence_values::<Table>()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert_eq!(actual.len(), if size == "154x67" { 8 } else { 9 });
        for (actual, (id, plot, resource, year, tech)) in actual.iter().zip(expected) {
            assert_eq!(actual.get::<String>("id").unwrap(), id);
            assert_eq!(actual.get::<i32>("plot").unwrap(), plot);
            assert_eq!(actual.get::<String>("resource").unwrap(), resource);
            assert_eq!(actual.get::<i32>("year").unwrap(), year);
            assert_eq!(
                actual.get::<Option<String>>("tech").unwrap(),
                (!tech.is_empty()).then(|| format!("TECH_JAC_SENGOKU_{tech}"))
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 17);
    let static_code = s(&scenario["steps"][4]["code"]);
    assert_eq!(
        execute(&lua, "generated resource static check", static_code),
        "57 start resources and 9 deferred sites present; plantation and mine rows in place"
    );
    assert_eq!(
        execute(
            &lua,
            "generated 1534 release check",
            s(&scenario["steps"][5]["code"])
        ),
        "year 1534 (engine 1534 AD, turn 0): 0 of 9 deferred sites on the map"
    );
    lua.load("ReleaseSites(1542)").exec().unwrap();
    let release = s(&scenario["steps"][8]["code"]);
    assert_eq!(
        execute(&lua, "generated 1542 release check", release),
        "year 1542 (engine 1542 AD, turn 24): 2 of 9 deferred sites on the map"
    );
    for (mutation, message) in [
        (
            "GameInfo.Resources.RESOURCE_JAC_GOLD=nil",
            "missing resource RESOURCE_JAC_GOLD",
        ),
        (
            "GameInfo.Resources.RESOURCE_JAC_LACQUER.PrereqTech='TECH_MINING'",
            "must not be gated by a tech",
        ),
        (
            "GameInfo.Resources.RESOURCE_IRON.PrereqTech='TECH_JAC_SENGOKU_WRONG'",
            "iron gate",
        ),
        (
            "GameInfo.Resource_Harvests.RESOURCE_RICE.PrereqTech='TECH_MINING'",
            "official tech harvest gate",
        ),
        (
            "GameInfo.Resource_ValidTerrains=function() return function() end end",
            "silver is not valid on grass hills",
        ),
        (
            "GameInfo.Improvements.IMPROVEMENT_PLANTATION.Buildable=false",
            "plantation not buildable",
        ),
        (
            "GameInfo.Improvement_ValidResources=function() return function() end end",
            "missing improvement",
        ),
        (
            "for _,p in ipairs(State.plots) do p.resource='' end",
            "start resource missing",
        ),
        (
            "ExposedMembers.JAC_Resources=nil",
            "JAC_Resources API missing",
        ),
        (
            "ExposedMembers.JAC_Resources.GetSites()[1].plot=0",
            "deferred site moved",
        ),
        (
            "table.remove(ExposedMembers.JAC_Resources.GetSites())",
            "deferred site missing",
        ),
        (
            "table.insert(ExposedMembers.JAC_Resources.GetSites(),{id='extra'})",
            "unexpected deferred site",
        ),
    ] {
        rejects_resource(&data, &resource_file, static_code, mutation, message)
    }
    for (mutation, message) in [
        ("State.year=1541", "calendar year 1541, expected 1542"),
        (
            "ReleaseSites(1542); State.engineYear=1541",
            "engine date 1541 AD is not 1542",
        ),
        (
            "ReleaseSites(1542); State.plots[10720].resource=''",
            "site not released: silver-ikuno",
        ),
        (
            "ReleaseSites(1542); State.plots[6043].resource='RESOURCE_JAC_GOLD'",
            "site released too early: gold-izu-toi",
        ),
    ] {
        rejects_resource(&data, &resource_file, release, mutation, message)
    }
    let early = resource_vm(&data, &resource_file);
    early.load("State.researched[GameInfo.Technologies.TECH_JAC_SENGOKU_CUPELLATION.Index]=true; ReleaseSites(1534)").exec().unwrap();
    assert_eq!(
        execute(
            &early,
            "generated tech release check",
            s(&scenario["steps"][5]["code"])
        ),
        "year 1534 (engine 1534 AD, turn 0): 2 of 9 deferred sites on the map"
    );
    let lua_steps = arr(&scenario["steps"])
        .iter()
        .enumerate()
        .filter(|(_, step)| step["action"] == "ui_lua")
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    assert_eq!(lua_steps, BTreeSet::from([4, 5, 8]));
    assert_eq!(lua_steps.len(), 3);
}

fn verify_prosperous(maps: &Path, scenarios: &Path, manifest: &Value) {
    let scenario = read_json(scenarios.join("japan-prosperous-15km.json")).unwrap();
    let item = arr(manifest)
        .iter()
        .find(|r| r["km_per_tile"] == 15)
        .unwrap();
    let data = state(maps, item, &scenario);
    let lua = vm(&data);
    let mut checked = 0;
    for (index, step) in arr(&scenario["steps"]).iter().enumerate() {
        if step["action"] != "ui_lua" {
            continue;
        }
        let code = s(&step["code"]);
        let result = execute(&lua, "generated prosperous scenario", code);
        if index == 3 {
            assert!(result.starts_with("START; "));
            assert_eq!(result.split(';').count(), 23);
            for (mutation,message) in [
                ("State.properties.JAC_JAPAN_START_STATE=nil","no start state recorded"),
                ("State.properties.JAC_JAPAN_START_BUILDINGS=nil","no start buildings recorded"),
                ("State.properties.JAC_JAPAN_START_STATE=State.properties.JAC_JAPAN_START_STATE:gsub('0=2,','0=3,',1)","cities 3 expected 2"),
                ("State.properties.JAC_JAPAN_START_BUILDINGS=State.properties.JAC_JAPAN_START_BUILDINGS..',BUILDING_FAKE=1'","BUILDING_FAKE 1 expected 0"),
            ] {rejects(&data,"generated prosperous start",code,mutation,message)}
        } else if index == arr(&scenario["steps"]).len() - 1 {
            assert!(result.starts_with("SURVIVORS T1 22/22: "));
            rejects(
                &data,
                "generated survivors",
                code,
                "State.players[1].alive=false",
                "regression: SURVIVORS",
            );
            rejects(
                &data,
                "generated survivors",
                code,
                "for _,c in ipairs(State.players[1].cities) do c.capital=false end",
                "NO_CAPITAL",
            );
        } else {
            assert!(result.starts_with("T1; 0:"));
            assert_eq!(result.split(';').count(), 23);
        }
        checked += 1;
    }
    assert_eq!(checked, 8);
}
