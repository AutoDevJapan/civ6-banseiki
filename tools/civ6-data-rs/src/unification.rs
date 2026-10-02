#[cfg(test)]
use anyhow::anyhow;
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OpenFlags};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::{apply_outputs, Outputs, Repository};

pub const VICTORY: &str = "VICTORY_JAC_SENGOKU_UNIFICATION";
pub const CASTLE_SET: &str = "JAC_UNIFICATION_CITY_IS_CASTLE";
pub const COUNTED_SET: &str = "JAC_UNIFICATION_CITY_COUNTS";
pub const REBUILT: &str = "BUILDING_JAC_REBUILT_ON_CASTLE";
pub const UNIFIED: &str = "BUILDING_JAC_PACT_UNIFIED";
pub const PACT_LEAD_PREFIX: &str = "JAC_UNIFICATION_PACT_LEAD_";
pub const PACT_PROPOSAL_PREFIX: &str = "JAC_UNIFICATION_PACT_PROPOSAL_";
pub const PACT_RESULT: &str = "JAC_UNIFICATION_PACT_RESULT";
pub const RAZED_PREFIX: &str = "JAC_UNIFICATION_RAZED_CASTLE_PLOT_";
pub const HUMAN: i64 = 0;

pub const MAP_MOD: &str = "poc/JAC_PoC_JapanMapScale";
pub const TREE_MOD: &str = "poc/JAC_PoC_SengokuTrees";
pub const TEST_MOD: &str = "poc/JAC_Test_UnificationMap";
pub const TEST_MOD_ID: &str = "5c1e7a3d-9b2f-4e61-8d47-2a6f0c9e1b85";
pub const MAP_NAME: &str = "JAC_Unification_Test";
pub const MAP_FILE: &str =
    "{5c1e7a3d-9b2f-4e61-8d47-2a6f0c9e1b85}Maps/JAC_Unification_Test.Civ6Map";
pub const MAP_SIZE: &str = "MAPSIZE_DUEL";
pub const WIDTH: i64 = 32;
pub const HEIGHT: i64 = 20;

pub const TEST_MAP: &str = "poc/JAC_Test_UnificationMap/Maps/JAC_Unification_Test.Civ6Map";
pub const REAL_MAP: &str = "poc/JAC_PoC_JapanMapScale/Maps/JAC_Japan_15km.Civ6Map";
pub const OUTPUT: &str = "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Unification.sql";
pub const SCENARIOS: &str = "tools/civ6-autotest/scenarios";
pub const SCENARIO: &str = "tools/civ6-autotest/scenarios/unification-victory.json";
pub const REGISTRY_SCENARIO: &str = "tools/civ6-autotest/scenarios/unification-registry-15km.json";
pub const PACT_SCENARIO: &str = "tools/civ6-autotest/scenarios/unification-pact.json";

pub const CASTLE_FILES: [(&str, &str, &str); 3] = [
    (
        "poc/JAC_PoC_JapanMapScale/Maps/JAC_Japan_20km.Civ6Map",
        "20KM",
        "poc/JAC_PoC_JapanMapScale/Data/JAC_Japan_20km_Castles.sql",
    ),
    (
        REAL_MAP,
        "15KM",
        "poc/JAC_PoC_JapanMapScale/Data/JAC_Japan_15km_Castles.sql",
    ),
    (
        TEST_MAP,
        "TEST",
        "poc/JAC_Test_UnificationMap/Data/JAC_Unification_Test_Castles.sql",
    ),
];

pub type Castle = (i64, i64, i64);

fn open_readonly(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("open map {}", path.display()))
}

pub fn castles_from_connection(db: &Connection) -> Result<Vec<Castle>> {
    let width: i64 = db.query_row("SELECT Width FROM Map", [], |row| row.get(0))?;
    ensure!(width > 0, "Map width must be positive: {width}");
    let mut statement = db.prepare("SELECT Plot, Owner FROM Cities ORDER BY Plot")?;
    let rows = statement
        .query_map([], |row| {
            let plot: i64 = row.get(0)?;
            let owner: i64 = row.get(1)?;
            Ok((plot % width, plot / width, owner))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(!rows.is_empty(), "No registered castles");
    Ok(rows)
}

pub fn castles(path: impl AsRef<Path>) -> Result<Vec<Castle>> {
    let path = path.as_ref();
    castles_from_connection(&open_readonly(path)?)
        .with_context(|| format!("read registered castles: {}", path.display()))
}

pub fn castle_rows(db: &Connection) -> Result<BTreeMap<i64, i64>> {
    let mut statement = db.prepare("SELECT Plot, Owner FROM Cities")?;
    let result = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
    Ok(result)
}

pub fn verify_registry(repo: &Repository) -> Result<usize> {
    let manifest = repo.json(format!("{MAP_MOD}/Maps/manifest.json"))?;
    let items = manifest
        .as_array()
        .context("Map manifest must be an array")?;
    let mut counts = BTreeSet::new();
    for item in items {
        let file = field(item, "file")?;
        counts.insert(castles(repo.path(format!("{MAP_MOD}/Maps/{file}")))?.len());
    }
    for item in items {
        let file = field(item, "settler_file")?;
        let db = open_readonly(&repo.path(format!("{MAP_MOD}/Maps/{file}")))?;
        let count: i64 = db.query_row("SELECT COUNT(*) FROM Cities", [], |r| r.get(0))?;
        ensure!(count == 0, "Settler map has registered castles: {file}");
    }
    ensure!(
        counts.len() == 1,
        "Historical maps have different castle counts: {counts:?}"
    );
    counts.into_iter().next().context("Empty map manifest")
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value
        .get(name)
        .and_then(Value::as_str)
        .with_context(|| format!("Missing string field {name}"))
}

pub fn castle_sql(tag: &str, rows: &[Castle]) -> String {
    let ids: Vec<_> = rows
        .iter()
        .map(|&(x, y, _)| format!("JAC_UNIFICATION_CASTLE_{tag}_{x}_{y}"))
        .collect();
    let requirements = ids
        .iter()
        .map(|id| format!("('{id}', 'REQUIREMENT_CITY_LOCATION_MATCHES')"))
        .collect::<Vec<_>>()
        .join(",\n    ");
    let arguments = ids
        .iter()
        .zip(rows)
        .flat_map(|(id, &(x, y, _))| {
            [
                format!("('{id}', 'X', '{x}')"),
                format!("('{id}', 'Y', '{y}')"),
            ]
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    let members = ids
        .iter()
        .map(|id| format!("('{CASTLE_SET}', '{id}')"))
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!(
        "-- Generated by tools/generate_unification_victory.py from this map's Cities rows.\n\
-- A city standing on one of these plots is a registered castle, whoever owns it.\n\
-- Loaded after the victory SQL (LoadOrder 300); OR IGNORE keeps the shared set either way.\n\
INSERT OR IGNORE INTO RequirementSets (RequirementSetId, RequirementSetType) VALUES\n    \
('{CASTLE_SET}', 'REQUIREMENTSET_TEST_ANY');\n\
INSERT INTO Requirements (RequirementId, RequirementType) VALUES\n    \
{requirements};\n\
INSERT INTO RequirementArguments (RequirementId, Name, Value) VALUES\n    \
{arguments};\n\
INSERT INTO RequirementSetRequirements (RequirementSetId, RequirementId) VALUES\n    \
{members};\n"
    )
}

pub fn sql() -> String {
    format!(
        r#"-- Generated by tools/generate_unification_victory.py.
-- Castle branch: at least one city stands on a registered castle plot and the team
-- owns every such city (official patterns: DLC/AlexanderScenario/Data/
-- AlexanderScenario_Victories.xml, DLC/WarMachineScenario/Data/WarMachineScenario_GameplayData.xml).
-- Capital branch: no city stands on a registered plot, so the official domination
-- requirement decides (Base/Assets/Gameplay/Data/Victories.xml).
-- The plots come from the map mods' *_Castles.sql, loaded only with a historical map;
-- the never-matching (-1,-1) entry keeps the set defined when no castle file is loaded.
-- A city rebuilt on a razed castle's plot holds {REBUILT} (placed by
-- Scripts/JAC_Sengoku_RebuiltCastles.lua) and is not counted (owner decision 3a).
-- Pact branch (天下統一同盟 and 1534 start alliances, SSOT 3.1): the team owns a city holding {UNIFIED},
-- which Scripts/JAC_Sengoku_UnificationPact.lua puts in the lead's capital once the group's teams
-- together hold every counted castle, or with none counted every home capital (owner decision O-8).
-- Partners do not own that city, so only the lead wins, on either kind of map.
INSERT INTO Types (Type, Kind) VALUES ('{VICTORY}', 'KIND_VICTORY'), ('{REBUILT}', 'KIND_BUILDING'),
    ('{UNIFIED}', 'KIND_BUILDING');
INSERT INTO Buildings (BuildingType, Name, PrereqDistrict, Cost, InternalOnly) VALUES
    ('{REBUILT}', 'LOC_{REBUILT}_NAME', 'DISTRICT_CITY_CENTER', 1, 1),
    ('{UNIFIED}', 'LOC_{UNIFIED}_NAME', 'DISTRICT_CITY_CENTER', 1, 1);
INSERT INTO Victories
    (VictoryType, Name, Description, Blurb, RequirementSetId, EnabledByDefault, OneMoreTurn, Icon)
VALUES
    ('{VICTORY}', 'LOC_VICTORY_JAC_UNIFICATION_NAME',
     'LOC_VICTORY_JAC_UNIFICATION_DESCRIPTION', 'LOC_VICTORY_JAC_UNIFICATION_BLURB',
     'JAC_UNIFICATION_VICTORY_REQUIREMENTS', 1, 0, 'ICON_VICTORY_DOMINATION');

INSERT INTO RequirementSets (RequirementSetId, RequirementSetType) VALUES
    ('JAC_UNIFICATION_VICTORY_REQUIREMENTS', 'REQUIREMENTSET_TEST_ANY'),
    ('JAC_UNIFICATION_CASTLE_REQUIREMENTS', 'REQUIREMENTSET_TEST_ALL'),
    ('JAC_UNIFICATION_CAPITAL_REQUIREMENTS', 'REQUIREMENTSET_TEST_ALL'),
    ('JAC_UNIFICATION_CITY_HELD', 'REQUIREMENTSET_TEST_ANY'),
    ('{COUNTED_SET}', 'REQUIREMENTSET_TEST_ALL'),
    ('JAC_UNIFICATION_PACT_CITY', 'REQUIREMENTSET_TEST_ALL');
INSERT OR IGNORE INTO RequirementSets (RequirementSetId, RequirementSetType) VALUES
    ('{CASTLE_SET}', 'REQUIREMENTSET_TEST_ANY');
INSERT INTO RequirementSetRequirements (RequirementSetId, RequirementId) VALUES
    ('JAC_UNIFICATION_VICTORY_REQUIREMENTS', 'JAC_UNIFICATION_CASTLES'),
    ('JAC_UNIFICATION_VICTORY_REQUIREMENTS', 'JAC_UNIFICATION_CAPITALS'),
    ('JAC_UNIFICATION_VICTORY_REQUIREMENTS', 'JAC_UNIFICATION_PACT'),
    ('JAC_UNIFICATION_CASTLE_REQUIREMENTS', 'JAC_UNIFICATION_CASTLES_EXIST'),
    ('JAC_UNIFICATION_CASTLE_REQUIREMENTS', 'JAC_UNIFICATION_CASTLES_OWNED'),
    ('JAC_UNIFICATION_CAPITAL_REQUIREMENTS', 'JAC_UNIFICATION_NO_CASTLES'),
    ('JAC_UNIFICATION_CAPITAL_REQUIREMENTS', 'JAC_UNIFICATION_ALL_CAPITALS'),
    ('JAC_UNIFICATION_CITY_HELD', 'JAC_UNIFICATION_CITY_NOT_CASTLE'),
    ('JAC_UNIFICATION_CITY_HELD', 'JAC_UNIFICATION_CITY_OWNED'),
    ('{COUNTED_SET}', 'JAC_UNIFICATION_CITY_ON_CASTLE_PLOT'),
    ('{COUNTED_SET}', 'JAC_UNIFICATION_CITY_NOT_REBUILT'),
    ('JAC_UNIFICATION_PACT_CITY', 'JAC_UNIFICATION_CITY_OWNED'),
    ('JAC_UNIFICATION_PACT_CITY', 'JAC_UNIFICATION_CITY_PACT_UNIFIED'),
    ('{CASTLE_SET}', 'JAC_UNIFICATION_CASTLE_NONE');
INSERT INTO Requirements (RequirementId, RequirementType, Inverse) VALUES
    ('JAC_UNIFICATION_CASTLES', 'REQUIREMENT_REQUIREMENTSET_IS_MET', 0),
    ('JAC_UNIFICATION_CAPITALS', 'REQUIREMENT_REQUIREMENTSET_IS_MET', 0),
    ('JAC_UNIFICATION_PACT', 'REQUIREMENT_COLLECTION_COUNT_ATLEAST', 0),
    ('JAC_UNIFICATION_CITY_PACT_UNIFIED', 'REQUIREMENT_CITY_HAS_BUILDING', 0),
    ('JAC_UNIFICATION_CASTLES_EXIST', 'REQUIREMENT_COLLECTION_COUNT_ATLEAST', 0),
    ('JAC_UNIFICATION_CASTLES_OWNED', 'REQUIREMENT_COLLECTION_ALL_MET', 0),
    ('JAC_UNIFICATION_NO_CASTLES', 'REQUIREMENT_COLLECTION_COUNT_EQUALS', 0),
    ('JAC_UNIFICATION_ALL_CAPITALS', 'REQUIREMENT_TEAM_DOMINATION_VICTORY', 0),
    ('JAC_UNIFICATION_CITY_NOT_CASTLE', 'REQUIREMENT_REQUIREMENTSET_IS_MET', 1),
    ('JAC_UNIFICATION_CITY_OWNED', 'REQUIREMENT_PLAYER_OWNS_OBJECT', 0),
    ('JAC_UNIFICATION_CITY_ON_CASTLE_PLOT', 'REQUIREMENT_REQUIREMENTSET_IS_MET', 0),
    ('JAC_UNIFICATION_CITY_NOT_REBUILT', 'REQUIREMENT_CITY_HAS_BUILDING', 1),
    ('JAC_UNIFICATION_CASTLE_NONE', 'REQUIREMENT_CITY_LOCATION_MATCHES', 0);
INSERT INTO RequirementArguments (RequirementId, Name, Value) VALUES
    ('JAC_UNIFICATION_CASTLES', 'RequirementSetId', 'JAC_UNIFICATION_CASTLE_REQUIREMENTS'),
    ('JAC_UNIFICATION_CAPITALS', 'RequirementSetId', 'JAC_UNIFICATION_CAPITAL_REQUIREMENTS'),
    ('JAC_UNIFICATION_PACT', 'CollectionType', 'COLLECTION_ALL_CITIES'),
    ('JAC_UNIFICATION_PACT', 'RequirementSetId', 'JAC_UNIFICATION_PACT_CITY'),
    ('JAC_UNIFICATION_PACT', 'Count', '1'),
    ('JAC_UNIFICATION_CITY_PACT_UNIFIED', 'BuildingType', '{UNIFIED}'),
    ('JAC_UNIFICATION_CASTLES_EXIST', 'CollectionType', 'COLLECTION_ALL_CITIES'),
    ('JAC_UNIFICATION_CASTLES_EXIST', 'RequirementSetId', '{COUNTED_SET}'),
    ('JAC_UNIFICATION_CASTLES_EXIST', 'Count', '1'),
    ('JAC_UNIFICATION_CASTLES_OWNED', 'CollectionType', 'COLLECTION_ALL_CITIES'),
    ('JAC_UNIFICATION_CASTLES_OWNED', 'RequirementSetId', 'JAC_UNIFICATION_CITY_HELD'),
    ('JAC_UNIFICATION_NO_CASTLES', 'CollectionType', 'COLLECTION_ALL_CITIES'),
    ('JAC_UNIFICATION_NO_CASTLES', 'RequirementSetId', '{COUNTED_SET}'),
    ('JAC_UNIFICATION_NO_CASTLES', 'Count', '0'),
    ('JAC_UNIFICATION_CITY_NOT_CASTLE', 'RequirementSetId', '{COUNTED_SET}'),
    ('JAC_UNIFICATION_CITY_ON_CASTLE_PLOT', 'RequirementSetId', '{CASTLE_SET}'),
    ('JAC_UNIFICATION_CITY_NOT_REBUILT', 'BuildingType', '{REBUILT}'),
    ('JAC_UNIFICATION_CASTLE_NONE', 'X', '-1'),
    ('JAC_UNIFICATION_CASTLE_NONE', 'Y', '-1');
INSERT INTO RequirementStrings (RequirementId, Context, Text) VALUES
    ('JAC_UNIFICATION_CASTLES', 'VictoryProgress', 'LOC_VICTORY_JAC_UNIFICATION_PROGRESS'),
    ('JAC_UNIFICATION_CAPITALS', 'VictoryProgress', 'LOC_VICTORY_JAC_UNIFICATION_CAPITALS_PROGRESS'),
    ('JAC_UNIFICATION_PACT', 'VictoryProgress', 'LOC_VICTORY_JAC_UNIFICATION_PACT_PROGRESS');
"#
    )
}

#[derive(Clone, Copy)]
pub struct TestSeat {
    pub slug: &'static str,
    pub leader: &'static str,
    pub civ: &'static str,
    pub start: (i64, i64),
}

pub const TEST_SEATS: [TestSeat; 3] = [
    TestSeat {
        slug: "ODA",
        leader: "LEADER_JAC_ODA_NOBUHIDE",
        civ: "CIVILIZATION_JAC_ODA",
        start: (6, 5),
    },
    TestSeat {
        slug: "DATE",
        leader: "LEADER_JAC_DATE_TANEMUNE",
        civ: "CIVILIZATION_JAC_DATE",
        start: (25, 5),
    },
    TestSeat {
        slug: "ASHINA",
        leader: "LEADER_JAC_ASHINA_MORIKIYO",
        civ: "CIVILIZATION_JAC_ASHINA",
        start: (16, 10),
    },
];

#[derive(Clone, Copy)]
pub struct TestCastle {
    pub owner: usize,
    pub x: i64,
    pub y: i64,
    pub capital: bool,
    pub research_priority: i64,
    pub name_tag: &'static str,
}

// Source fixtures from unification_test_map.py, not generated map contents.
pub const TEST_CASTLES: [TestCastle; 4] = [
    TestCastle {
        owner: 0,
        x: 6,
        y: 5,
        capital: true,
        research_priority: 0,
        name_tag: "LOC_JAC_1534_CAPITAL_ODA",
    },
    TestCastle {
        owner: 1,
        x: 25,
        y: 5,
        capital: true,
        research_priority: 0,
        name_tag: "LOC_JAC_1534_CAPITAL_DATE",
    },
    TestCastle {
        owner: 0,
        x: 6,
        y: 14,
        capital: false,
        research_priority: 1,
        name_tag: "LOC_JAC_TEST_UNIFICATION_CASTLE_ODA",
    },
    TestCastle {
        owner: 1,
        x: 25,
        y: 14,
        capital: false,
        research_priority: 2,
        name_tag: "LOC_JAC_TEST_UNIFICATION_CASTLE_DATE",
    },
];

pub const TEST_MAP_LUA: &str = "-- Static map: starting state is stored in the Civ6Map file.\n\
function InitializeNewGame() end\n\
LuaEvents.NewGameInitialized.Add(InitializeNewGame);\n";

// Official WorldMapHuge.Civ6Map schema source fixture. No official map rows
// or checked-in generated map rows are included.
const MAP_SCHEMA: &str = r#"
CREATE TABLE Map (ID TEXT NOT NULL PRIMARY KEY, Width INTEGER, Height INTEGER, TopLatitude INTEGER, BottomLatitude INTEGER, WrapX BOOLEAN, WrapY BOOLEAN, MapSizeType TEXT);
CREATE TABLE MapAttributes (Name TEXT NOT NULL PRIMARY KEY, Value TEXT);
CREATE TABLE MetaData (Name TEXT NOT NULL PRIMARY KEY, Value TEXT);
CREATE TABLE Plots (ID INTEGER NOT NULL PRIMARY KEY, TerrainType TEXT NOT NULL, ContinentType TEXT, IsImpassable BOOLEAN);
CREATE TABLE PlotOwners (ID INTEGER NOT NULL PRIMARY KEY, Owner INTEGER, CityOwner INTEGER, CityWorking INTEGER);
CREATE TABLE Cities (Owner INTEGER NOT NULL, Plot INTEGER NOT NULL PRIMARY KEY, Name TEXT);
CREATE TABLE Districts (DistrictType TEXT NOT NULL, CityID INTEGER NOT NULL, Plot INTEGER NOT NULL PRIMARY KEY);
CREATE TABLE Buildings (BuildingType TEXT NOT NULL, Plot INTEGER NOT NULL, PRIMARY KEY(BuildingType, Plot));
CREATE TABLE Players (ID INTEGER NOT NULL PRIMARY KEY, CivilizationType TEXT, LeaderType TEXT, CivilizationLevelType TEXT, AgendaType TEXT, Status TEXT, Handicap TEXT, StartingPosition TEXT, Color TEXT, Initialized BOOLEAN);
CREATE TABLE StartPositions (Plot INTEGER NOT NULL PRIMARY KEY, Type STRING NOT NULL, Value STRING NOT NULL);
CREATE TABLE Units (ID INTEGER NOT NULL PRIMARY KEY, UnitType TEXT NOT NULL, Owner INTEGER NOT NULL, Plot INTEGER NOT NULL, Name TEXT);
CREATE TABLE RevealedPlots (ID INTEGER NOT NULL, Player INTEGER, PRIMARY KEY(ID, Player));
CREATE TABLE PlotRoutes (ID INTEGER NOT NULL PRIMARY KEY, RouteType TEXT, RouteEra TEXT, IsRoutePillaged BOOLEAN);
CREATE TABLE PlotImprovements (ID INTEGER NOT NULL PRIMARY KEY, ImprovementType TEXT, ImprovementOwner INTEGER);
CREATE TABLE PlotFeatures (ID INTEGER NOT NULL PRIMARY KEY, FeatureType TEXT);
CREATE TABLE PlotResources (ID INTEGER NOT NULL PRIMARY KEY, ResourceType TEXT, ResourceCount INTEGER);
CREATE TABLE PlotCliffs (ID INTEGER NOT NULL PRIMARY KEY, IsNEOfCliff BOOLEAN, IsWOfCliff BOOLEAN, IsNWOfCliff BOOLEAN);
CREATE TABLE PlotRivers (ID INTEGER NOT NULL PRIMARY KEY, IsNEOfRiver BOOLEAN, IsWOfRiver BOOLEAN, IsNWOfRiver BOOLEAN, EFlowDirection INTEGER, SEFlowDirection INTEGER, SWFlowDirection INTEGER);
CREATE TABLE NamedRiver (ID INTEGER NOT NULL PRIMARY KEY, Type TEXT, DiscoveringCiv TEXT, CustomName TEXT);
CREATE TABLE NamedRiverPlot (ID INTEGER NOT NULL, PlotIndex INTEGER NOT NULL, Edge INTEGER NOT NULL, PRIMARY KEY(ID, PlotIndex, Edge));
CREATE TABLE GameConfig (ID TEXT NOT NULL PRIMARY KEY, Value TEXT);
CREATE TABLE ModDependencies (ID TEXT NOT NULL PRIMARY KEY, Title TEXT NOT NULL);
CREATE TABLE ModText (Language TEXT NOT NULL, ID TEXT NOT NULL, Value TEXT, PRIMARY KEY(Language, ID));
CREATE TABLE ModSettings (Type TEXT NOT NULL, ID TEXT NOT NULL, PRIMARY KEY(Type, ID));
CREATE TABLE ModComponents (Type TEXT NOT NULL, ID TEXT NOT NULL, PRIMARY KEY(Type, ID));
CREATE TABLE ModProperties (Name TEXT NOT NULL, Value TEXT, PRIMARY KEY(Name, Value));
"#;

fn fixture_schema() -> Result<Connection> {
    let db = Connection::open_in_memory()?;
    db.execute_batch(MAP_SCHEMA)?;
    for table in [
        "CityAttributes",
        "UnitAttributes",
        "PlotAttributes",
        "PlayerAttributes",
        "DistrictAttributes",
    ] {
        db.execute_batch(&format!(
            "CREATE TABLE {table} (ID INTEGER NOT NULL, Type TEXT NOT NULL, Name TEXT NOT NULL, Value TEXT, PRIMARY KEY(ID, Type, Name));"
        ))?;
    }
    for table in [
        "ModSettings_Items",
        "ModSettings_Properties",
        "ModComponent_Items",
        "ModComponent_Properties",
    ] {
        db.execute_batch(&format!(
            "CREATE TABLE {table} (Type TEXT NOT NULL, ID TEXT NOT NULL, Name TEXT NOT NULL, Value TEXT, PRIMARY KEY(Type, ID, Name, Value));"
        ))?;
    }
    Ok(db)
}

fn plot(x: i64, y: i64) -> i64 {
    y * WIDTH + x
}

pub fn neighbours(index: i64) -> BTreeSet<i64> {
    let x = index % WIDTH;
    let y = index / WIDTH;
    let odd = y & 1;
    [
        (-1, 0),
        (1, 0),
        (odd, -1),
        (odd - 1, -1),
        (odd, 1),
        (odd - 1, 1),
    ]
    .into_iter()
    .filter_map(|(dx, dy)| {
        let (nx, ny) = (x + dx, y + dy);
        ((0..WIDTH).contains(&nx) && (0..HEIGHT).contains(&ny)).then_some(plot(nx, ny))
    })
    .collect()
}

pub struct TestMapAssets {
    pub database: Connection,
    pub lua: String,
}

// Accepts an official map/schema connection as an explicit future-assets
// interface; copies only schema and deterministically regenerates every row.
pub fn test_map_assets_from_schema(source: &Connection) -> Result<TestMapAssets> {
    let db = Connection::open_in_memory()?;
    let mut statement = source.prepare(
        "SELECT sql FROM sqlite_master WHERE type IN ('table','index') AND sql IS NOT NULL ORDER BY type DESC, name",
    )?;
    let statements = statement
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(!statements.is_empty(), "Official map schema is empty");
    for statement in statements {
        db.execute_batch(&statement)?;
    }
    populate_test_map(&db)?;
    Ok(TestMapAssets {
        database: db,
        lua: TEST_MAP_LUA.to_owned(),
    })
}

pub fn test_map_assets(_repo: &Repository) -> Result<TestMapAssets> {
    test_map_assets_from_schema(&fixture_schema()?)
}

fn populate_test_map(db: &Connection) -> Result<()> {
    db.execute(
        "INSERT INTO Map VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params!["Default", WIDTH, HEIGHT, 47, 23, 0, 0, MAP_SIZE],
    )?;
    for (name, value) in [
        ("MapScript", "Continents.lua"),
        ("Ruleset", "RULESET_EXPANSION_2"),
    ] {
        db.execute(
            "INSERT INTO MapAttributes VALUES (?, ?)",
            params![name, value],
        )?;
    }
    for (name, value) in [
        ("MetaDataVersion", "1"),
        ("AppVersion", "1.0.12.68"),
        ("DisplayName", "JAC_Unification_Test.Civ6Map"),
        // UUIDv5(NAMESPACE_URL, "JAC/UnificationTestMap"), as in the source.
        ("ID", "4962F284-8BFB-569F-9328-464DB5901F75"),
        ("IsMod", "1"),
        ("IsBasic", "1"),
        ("LastUpdated", "0"),
    ] {
        db.execute("INSERT INTO MetaData VALUES (?, ?)", params![name, value])?;
    }
    let land: BTreeSet<_> = (2..HEIGHT - 2)
        .flat_map(|y| (2..WIDTH - 2).map(move |x| plot(x, y)))
        .collect();
    let coast: BTreeSet<_> = land
        .iter()
        .flat_map(|&index| neighbours(index))
        .filter(|index| !land.contains(index))
        .collect();
    for index in 0..WIDTH * HEIGHT {
        let terrain = if land.contains(&index) {
            "TERRAIN_GRASS"
        } else if coast.contains(&index) {
            "TERRAIN_COAST"
        } else {
            "TERRAIN_OCEAN"
        };
        let continent = if land.contains(&index) {
            "CONTINENT_ASIA"
        } else {
            ""
        };
        db.execute(
            "INSERT INTO Plots VALUES (?, ?, ?, ?)",
            params![index, terrain, continent, 0],
        )?;
    }
    for (id, seat) in TEST_SEATS.iter().enumerate() {
        db.execute(
            "INSERT INTO StartPositions VALUES (?, 'LEADER', ?)",
            params![plot(seat.start.0, seat.start.1), seat.leader],
        )?;
        db.execute(
            "INSERT INTO Players VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id as i64,
                seat.civ,
                seat.leader,
                "CIVILIZATION_LEVEL_FULL_CIV",
                "",
                "Human",
                "DIFFICULTY_PRINCE",
                "INVALID",
                "RANDOM",
                1
            ],
        )?;
    }
    let mut ordered = TEST_CASTLES.to_vec();
    ordered.sort_by_key(|city| {
        (
            !city.capital,
            if city.capital {
                city.owner as i64
            } else {
                city.research_priority
            },
        )
    });
    let mut city_order = BTreeMap::new();
    for (order, city) in ordered.iter().enumerate() {
        let centre = plot(city.x, city.y);
        city_order.insert(centre, order);
        db.execute(
            "INSERT INTO Cities VALUES (?, ?, ?)",
            params![city.owner as i64, centre, city.name_tag],
        )?;
        db.execute(
            "INSERT INTO Districts VALUES ('DISTRICT_CITY_CENTER', ?, ?)",
            params![centre, centre],
        )?;
        db.execute(
            "INSERT INTO CityAttributes VALUES (?, 'Population', 'Initial', '2')",
            [centre],
        )?;
        if city.capital {
            db.execute(
                "INSERT INTO Buildings VALUES ('BUILDING_PALACE', ?)",
                [centre],
            )?;
        }
    }
    let mut allocations = Vec::new();
    for city in TEST_CASTLES {
        let centre = plot(city.x, city.y);
        let mut owned = neighbours(centre);
        owned.insert(centre);
        for index in owned {
            allocations.push((index, city.owner as i64, centre, centre));
        }
    }
    allocations.sort_by_key(|row| match city_order.get(&row.0) {
        Some(&order) => (0_i64, order as i64),
        None => (1, row.0),
    });
    for (index, owner, city_owner, city_working) in allocations {
        db.execute(
            "INSERT INTO PlotOwners VALUES (?, ?, ?, ?)",
            params![index, owner, city_owner, city_working],
        )?;
    }
    let integrity: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    ensure!(integrity == "ok", "Invalid SQLite test map: {integrity}");
    db.execute_batch("VACUUM")?;
    Ok(())
}

fn base(repo: &Repository) -> Result<(Value, Value)> {
    Ok((
        repo.json(format!("{SCENARIOS}/japan-map-15km.json"))?,
        repo.json(format!("{SCENARIOS}/sengoku-trees.json"))?,
    ))
}

fn setup_step(setup: &Value, second_leader: &str) -> Result<Value> {
    let mut step = setup
        .get("steps")
        .and_then(Value::as_array)
        .and_then(|steps| steps.get(2))
        .cloned()
        .context("Missing sengoku-trees setup step 2")?;
    step["code"] =
        Value::String(field(&step, "code")?.replace("LEADER_JAC_TAKEDA_NOBUTORA", second_leader));
    Ok(step)
}

fn base_step(base: &Value, index: usize) -> Result<Value> {
    base.get("steps")
        .and_then(Value::as_array)
        .and_then(|steps| steps.get(index))
        .cloned()
        .with_context(|| format!("Missing base scenario step {index}"))
}

pub fn castle_lua(rows: &[Castle]) -> String {
    let keys = rows
        .iter()
        .map(|&(x, y, _)| format!("['{x},{y}']=true"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "local plots={{{keys}}}; local function castle(city) return plots[city:GetX()..','..city:GetY()]==true end; "
    )
}

pub fn razed_lua(rows: &[Castle]) -> String {
    let coords = rows
        .iter()
        .map(|&(x, y, _)| format!("{{{x},{y}}}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "local coords={{{coords}}}; local marker=GameInfo.Buildings['{REBUILT}'].Index; \
local function razed(x,y) return Game:GetProperty('{RAZED_PREFIX}'..Map.GetPlot(x,y):GetIndex())==true end; "
    )
}

pub fn player_lua(var: &str, civ: &str, label: &str) -> String {
    format!(
        "local {var}=nil; for id=0,PlayerManager.GetWasEverAliveCount()-1 do \
if PlayerConfigurations[id]:GetCivilizationTypeName()=='{civ}' then {var}=id end end; \
if not {var} then error('{label} missing') end; "
    )
}

fn tally() -> String {
    format!(
        "local total=0; local owned=0; \
for _,owner in ipairs(PlayerManager.GetAliveIDs()) do \
for _,city in Players[owner]:GetCities():Members() do \
if castle(city) then total=total+1; \
if owner=={HUMAN} then owned=owned+1 end end end end; "
    )
}

pub fn count_code(
    label: &str,
    rows: &[Castle],
    total: Option<usize>,
    missing: Option<usize>,
) -> Result<String> {
    let mut checks = Vec::new();
    if let Some(total) = total {
        checks.push(format!("total~={total}"));
    }
    if let Some(missing) = missing {
        checks.push(format!("total-owned~={missing}"));
    }
    ensure!(!checks.is_empty(), "A castle count check needs a condition");
    Ok(format!(
        "{}{}if {} then error('{label} castles '..owned..'/'..total) end; ",
        castle_lua(rows),
        tally(),
        checks.join(" or ")
    ))
}

fn ui(state: &str, code: impl Into<String>) -> Value {
    json!({"action": "ui_lua", "state_name": state, "code": code.into()})
}

fn wait(state: &str, code: impl Into<String>, timeout: i64) -> Value {
    json!({
        "action": "wait_for",
        "condition": {"type": "lua", "state_name": state, "code": code.into()},
        "timeout": timeout
    })
}

fn autoplay() -> Value {
    json!({"action": "autoplay", "turns": 1, "timeout": 300})
}

fn start_autoplay() -> Value {
    ui(
        "InGame",
        "local human=Game.GetLocalPlayer(); AutoplayManager.SetTurns(1); AutoplayManager.SetReturnAsPlayer(human); AutoplayManager.SetObserveAsPlayer(PlayerTypes.NONE); AutoplayManager.SetActive(true); return true",
    )
}

pub fn scenario(repo: &Repository) -> Result<Value> {
    scenario_from_registry(repo, &castles(repo.path(TEST_MAP))?)
}

pub fn scenario_from_registry(repo: &Repository, rows: &[Castle]) -> Result<Value> {
    let (base, setup) = base(repo)?;
    let total = rows.len();
    ensure!(
        total >= 3 && rows.iter().filter(|row| row.2 == HUMAN).count() == total - 2,
        "Test map must leave exactly two castles to Date: {rows:?}"
    );
    let players: Vec<_> = TEST_SEATS
        .iter()
        .map(|seat| json!({"leader": seat.leader, "civ": seat.civ, "human": seat.slug == "ODA"}))
        .collect();
    let date = player_lua("date", TEST_SEATS[1].civ, "Date");
    let builder = player_lua("builder", TEST_SEATS[2].civ, "Ashina");

    let initial = count_code("initial", rows, Some(total), Some(2))?
        + &format!(
            "if not Game.IsVictoryEnabled('{VICTORY}') then error('unification victory disabled') end; \
if Game.GetWinningTeam()~=nil then error('winner at start') end; \
return 'registered={total}, two held by Date'"
        );
    let raze = castle_lua(rows)
        + &date
        + "local target=nil; \
local capital=Players[date]:GetCities():GetCapitalCity(); \
for _,city in Players[date]:GetCities():Members() do \
if castle(city) and (not capital or city:GetID()~=capital:GetID()) \
then target=city end end; \
if not target then error('Date castle to raze missing') end; \
local owner,id=target:GetOwner(),target:GetID(); local how='DestroyCity'; \
if CityManager.DestroyCity then pcall(CityManager.DestroyCity,target) end; \
if CityManager.GetCity(owner,id) then how='fallback transfer (DestroyCity did not raze)'; "
        + &format!("CityManager.TransferCity(CityManager.GetCity(owner,id),{HUMAN}) end; ")
        + "return 'raze: '..how";
    let negative = count_code("negative", rows, None, Some(1))?
        + "if Game.GetWinningTeam()~=nil then error('victory with one castle missing') end; \
return 'negative: one castle missing ('..owned..'/'..total..'); no winner'";
    let rebuild = razed_lua(rows)
        + "local target=nil; \
for _,xy in ipairs(coords) do local city=CityManager.GetCityAt(xy[1],xy[2]); \
if razed(xy[1],xy[2]) then if city then error('razed plot has a city') end; \
if target then error('more than one razed plot') end; target=xy \
elseif not city then error('empty plot not recorded as razed: '..xy[1]..','..xy[2]) end end; \
if not target then error('no razed plot recorded (raze or turn-start scan failed)') end; "
        + &builder
        + "Players[builder]:GetCities():Create(target[1],target[2]); \
if not CityManager.GetCityAt(target[1],target[2]) then error('rebuild failed') end; \
return 'rebuilt '..target[1]..','..target[2]..' for Ashina'";
    let rebuilt = razed_lua(rows)
        + "local n=0; \
for _,xy in ipairs(coords) do local city=CityManager.GetCityAt(xy[1],xy[2]); \
if city and razed(xy[1],xy[2]) then n=n+1; \
if not city:GetBuildings():HasBuilding(marker) then error('rebuilt city unmarked') end \
elseif city and city:GetBuildings():HasBuilding(marker) then error('original castle marked') end end; \
if n~=1 then error('rebuilt cities '..n) end; \
if Game.GetWinningTeam()~=nil then error('winner before the last castle') end; \
return 'rebuilt city marked; no winner'";
    let final_transfer = castle_lua(rows)
        + &date
        + "local last=nil; \
for _,city in Players[date]:GetCities():Members() do \
if castle(city) then \
if last then error('more than one castle held out') end; last=city end end; \
if not last then error('held out castle missing') end; "
        + &format!("CityManager.TransferCity(last,{HUMAN}); return 'transferred the last castle'");
    let won = format!(
        "local winner,v=Game.GetWinningTeam(); \
return winner==Players[{HUMAN}]:GetTeam() and v~=nil and GameInfo.Victories[v]~=nil \
and GameInfo.Victories[v].VictoryType=='{VICTORY}'"
    );
    let transferred = razed_lua(rows)
        + "local owned=0; local rebuilt=0; \
for _,xy in ipairs(coords) do local city=CityManager.GetCityAt(xy[1],xy[2]); \
if razed(xy[1],xy[2]) then rebuilt=rebuilt+1; \
if not city or not city:GetBuildings():HasBuilding(marker) then error('rebuilt city lost') end; "
        + &format!("if city:GetOwner()=={HUMAN} then error('rebuilt city owned by Oda') end ")
        + "else if not city then error('castle missing at '..xy[1]..','..xy[2]) end; \
if city:GetBuildings():HasBuilding(marker) then error('castle marked as rebuilt: '..xy[1]..','..xy[2]) end; "
        + &format!("if city:GetOwner()=={HUMAN} then owned=owned+1 end end end; ")
        + &format!(
            "if rebuilt~=1 or owned~={} then error('after transfer owned '..owned..', rebuilt '..rebuilt) end; \
return 'all {} castles held by Oda, none marked; rebuilt city marked'",
            total - 1,
            total - 1
        );
    let mut configure = base_step(&base, 1)?;
    configure["map_file"] = json!(MAP_FILE);
    configure["map_size"] = json!(MAP_SIZE);
    configure["seed"] = json!(620951);
    configure["players"] = json!(players);

    let mut enable = base_step(&base, 0)?;
    enable["ids"]
        .as_array_mut()
        .context("Base enable_mods ids must be an array")?
        .push(json!(TEST_MOD_ID));
    if enable.get("disable_ids").is_none() {
        enable["disable_ids"] = json!([]);
    }
    enable["disable_ids"]
        .as_array_mut()
        .context("Base disable_ids must be an array")?
        .push(json!("57a3af09-fb38-41da-b367-818de11a995c"));

    Ok(json!({
        "name": "Sengoku unification victory on the test map: raze, one castle missing, save/load, rebuild, then all",
        "judge_profile": "generic",
        "steps": [
            enable,
            configure,
            setup_step(&setup, TEST_SEATS[1].leader)?,
            {"action": "start_game", "timeout": 240},
            ui("InGame", initial),
            ui("GameCore_Tuner", raze),
            ui("InGame", negative.clone()),
            ui("InGame", "LuaEvents.PartialScreenHooks_OpenWorldRankings(); return true"),
            wait("WorldRankings", "return not ContextPtr:IsHidden()", 30),
            ui("WorldRankings", format!(
                "ViewGeneric('{VICTORY}'); if Controls.GenericView:IsHidden() then error('custom victory tab hidden') end; return 'World Rankings shows 天下統一 condition'"
            )),
            {"action": "screenshot", "path": "unification-progress-negative.png"},
            ui("InGame", "LuaEvents.PartialScreenHooks_CloseAllExcept('_AllTheThings'); return true"),
            autoplay(),
            ui("InGame", negative.clone()),
            {"action": "save", "name": "jac-unification-negative"},
            {"action": "exit_to_menu"},
            {"action": "load", "name": "jac-unification-negative", "timeout": 300},
            ui("InGame", negative),
            ui("GameCore_Tuner", rebuild),
            autoplay(),
            ui("GameCore_Tuner", rebuilt),
            ui("GameCore_Tuner", final_transfer),
            ui("GameCore_Tuner", transferred),
            start_autoplay(),
            wait("InGame", won, 300),
            {"action": "screenshot", "path": "unification-victory.png"}
        ]
    }))
}

pub fn registry_scenario(repo: &Repository) -> Result<Value> {
    let (base, setup) = base(repo)?;
    let rows = castles(repo.path(REAL_MAP))?;
    let count = rows.len();
    let check = count_code("registry", &rows, Some(count), None)?
        + &format!(
            "if not Game.IsVictoryEnabled('{VICTORY}') then error('unification victory disabled') end; \
if Game.GetWinningTeam()~=nil then error('winner at start') end; return 'registered={count}'"
        );
    Ok(json!({
        "name": format!("Sengoku unification: the 15 km historical map starts with {count} registered castles"),
        "judge_profile": "generic",
        "steps": [
            base_step(&base, 0)?,
            base_step(&base, 1)?,
            setup_step(&setup, "LEADER_JAC_DATE_TANEMUNE")?,
            {"action": "start_game", "timeout": 600},
            ui("GameCore_Tuner", check)
        ]
    }))
}

pub fn pact_scenario(repo: &Repository) -> Result<Value> {
    pact_scenario_from_registry(repo, &castles(repo.path(TEST_MAP))?)
}

pub fn pact_scenario_from_registry(repo: &Repository, rows: &[Castle]) -> Result<Value> {
    let built = scenario_from_registry(repo, rows)?;
    let mut steps = built["steps"]
        .as_array()
        .context("Victory scenario steps must be an array")?[..5]
        .to_vec();
    let date_civ = field(&steps[1]["players"][1], "civ")?;
    let ashina_civ = field(&steps[1]["players"][2], "civ")?;
    let ids = player_lua("date", date_civ, "Date") + &player_lua("ashina", ashina_civ, "Ashina");
    let propose = ids.clone() + "JAC_RequestPact('Propose',date); return 'Oda proposes to Date'";
    let result =
        |value: &str| format!("return Players[{HUMAN}]:GetProperty('{PACT_RESULT}')=='{value}'");
    let not_allied = ids.clone()
        + &result("NOT_ALLIED")
        + &format!(" and Game:GetProperty('{PACT_LEAD_PREFIX}'..date)==nil");
    let joined = ids.clone()
        + &result("JOINED")
        + &format!(
            " and Game:GetProperty('{PACT_LEAD_PREFIX}'..date)=={HUMAN} \
and Game:GetProperty('{PACT_LEAD_PREFIX}{HUMAN}')=={HUMAN}"
        );
    let ally_ashina = ids.clone()
        + &format!(
            "local a,b=Players[{HUMAN}]:GetDiplomacy(),Players[ashina]:GetDiplomacy(); \
a:SetHasMet(ashina); b:SetHasMet({HUMAN}); a:SetHasDeclaredFriendship(ashina,true); \
b:SetHasDeclaredFriendship({HUMAN},true); a:SetHasAllied(ashina,true); \
if not a:HasAllied(ashina) then error('Oda and Ashina not allied') end; return 'Oda and Ashina allied'"
        );
    let propose_ashina =
        ids.clone() + "JAC_RequestPact('Propose',ashina); return 'Oda proposes to Ashina'";
    let joined_ashina = ids.clone()
        + &result("JOINED")
        + &format!(
            " and Game:GetProperty('{PACT_LEAD_PREFIX}'..ashina)=={HUMAN} \
and Game:GetProperty('{PACT_LEAD_PREFIX}{HUMAN}')=={HUMAN}"
        );
    let leave = format!("JAC_RequestPact('Leave',{HUMAN}); return 'Oda leaves the pact'");
    let dissolved = format!(
        "Game:GetProperty('{PACT_LEAD_PREFIX}'..ashina)==nil and Game:GetProperty('{PACT_LEAD_PREFIX}{HUMAN}')==nil \
 and Players[ashina]:GetProperty('{PACT_RESULT}')=='DISSOLVED'"
    );
    let left = ids.clone() + &result("LEFT") + " and " + &dissolved;
    let war = ids.clone()
        + &format!(
            "Players[{HUMAN}]:GetDiplomacy():DeclareWarOn(ashina,WarTypes.FORMAL_WAR,true); \
if not Players[{HUMAN}]:GetDiplomacy():IsAtWarWith(ashina) then \
error('DeclareWarOn(true) did not start a war between the allies Oda and Ashina (engine refuses war on an ally?)') end; \
return 'Oda at war with its pact partner Ashina'"
        );
    let war_dissolved = ids.clone()
        + &format!(
            "if not (Game:GetProperty('{PACT_LEAD_PREFIX}'..ashina)==nil and Game:GetProperty('{PACT_LEAD_PREFIX}{HUMAN}')==nil) then \
error('pact not dissolved by the war: ashina='..tostring(Players[ashina]:GetProperty('{PACT_RESULT}'))..' oda='..tostring(Players[{HUMAN}]:GetProperty('{PACT_RESULT}'))) end; \
if Game:GetProperty('{PACT_PROPOSAL_PREFIX}{HUMAN}')==ashina then error('Ashina proposed a pact to Oda while at war') end; \
Players[{HUMAN}]:GetDiplomacy():MakePeaceWith(ashina,true); \
if Players[{HUMAN}]:GetDiplomacy():IsAtWarWith(ashina) then error('peace with Ashina failed') end; \
return 'war dissolved the pact; peace restored'"
        );
    let prepare = ids.clone()
        + &castle_lua(rows)
        + "local capital=Players[date]:GetCities():GetCapitalCity(); local moved=nil; \
for _,city in Players[date]:GetCities():Members() do \
if castle(city) and city:GetID()~=capital:GetID() then moved=city end end; \
if not moved then error('Date castle to move missing') end; \
CityManager.TransferCity(moved,ashina); "
        + &format!(
            "local a,b=Players[{HUMAN}]:GetDiplomacy(),Players[date]:GetDiplomacy(); \
a:SetHasMet(date); b:SetHasMet({HUMAN}); a:SetHasDeclaredFriendship(date,true); \
b:SetHasDeclaredFriendship({HUMAN},true); a:SetHasAllied(date,true); \
if not a:HasAllied(date) then error('Oda and Date not allied') end; \
return 'Ashina holds one castle; Oda and Date allied'"
        );
    let unified = format!("GameInfo.Buildings['{UNIFIED}'].Index");
    let held_out = format!(
        "local capital=Players[{HUMAN}]:GetCities():GetCapitalCity(); \
if capital:GetBuildings():HasBuilding({unified}) then error('pact marked with 3 of 4 castles') end; \
if Game.GetWinningTeam()~=nil then error('winner while Ashina holds a castle') end; \
return 'pact holds 3 of 4 castles; no winner'"
    );
    let handover = ids.clone()
        + &castle_lua(rows)
        + "local last=nil; \
for _,city in Players[ashina]:GetCities():Members() do if castle(city) then last=city end end; \
if not last then error('Ashina castle missing') end; \
CityManager.TransferCity(last,date); return 'Ashina castle handed to Date (pact partner)'";
    let marked = format!(
        "return Players[{HUMAN}]:GetCities():GetCapitalCity():GetBuildings():HasBuilding({unified})"
    );
    let won = ids
        + "local winner,v=Game.GetWinningTeam(); "
        + &format!(
            "return winner==Players[{HUMAN}]:GetTeam() and winner~=Players[date]:GetTeam() and v~=nil \
and GameInfo.Victories[v]~=nil and GameInfo.Victories[v].VictoryType=='{VICTORY}'"
        );
    let panel = "JAC_UnificationPact";
    steps.extend([
        wait(panel, "return JAC_RequestPact~=nil", 30),
        ui("GameCore_Tuner", ally_ashina),
        ui(panel, propose_ashina.clone()),
        wait("InGame", joined_ashina.clone(), 30),
        ui(panel, leave),
        wait("InGame", left, 30),
        ui(panel, propose_ashina),
        wait("InGame", joined_ashina, 30),
        ui("GameCore_Tuner", war),
        autoplay(),
        ui("GameCore_Tuner", war_dissolved),
        wait(
            "InGame",
            "local p=Players[Game.GetLocalPlayer()]; return p~=nil and p:IsTurnActive()",
            600,
        ),
        ui(panel, propose.clone()),
        wait("InGame", not_allied, 30),
        ui("GameCore_Tuner", prepare),
        ui(panel, propose),
        wait("InGame", joined.clone(), 30),
        ui("GameCore_Tuner", held_out.clone()),
        json!({"action": "save", "name": "jac-unification-pact"}),
        json!({"action": "exit_to_menu"}),
        json!({"action": "load", "name": "jac-unification-pact", "timeout": 300}),
        wait("InGame", joined, 30),
        ui("GameCore_Tuner", held_out),
        wait(panel, "return JAC_RequestPact~=nil", 30),
        ui(panel, "JAC_TogglePactPanel(); return true"),
        json!({"action": "screenshot", "path": "unification-pact-panel.png"}),
        ui(panel, "JAC_TogglePactPanel(); return true"),
        ui("GameCore_Tuner", handover),
        wait("GameCore_Tuner", marked, 30),
        start_autoplay(),
        wait("InGame", won, 300),
        json!({"action": "screenshot", "path": "unification-pact-victory.png"}),
    ]);
    Ok(json!({
        "name": "Sengoku unification pact on the test map: leave and war dissolve a pact, the AI joins only when allied, pooled castles, only the lead wins",
        "judge_profile": "generic",
        "steps": steps
    }))
}

fn json_text(value: &Value) -> Result<String> {
    Ok(serde_json::to_string_pretty(value)? + "\n")
}

fn outputs_from_test_map(repo: &Repository, assets: &TestMapAssets) -> Result<Outputs> {
    verify_registry(repo)?;
    let test_rows = castles_from_connection(&assets.database)?;
    let mut result = Outputs::new();
    result.insert(PathBuf::from(OUTPUT), sql());
    result.insert(
        PathBuf::from(format!("{TEST_MOD}/Maps/{MAP_NAME}.lua")),
        assets.lua.clone(),
    );
    for (path, tag, output) in CASTLE_FILES {
        let rows = if path == TEST_MAP {
            test_rows.clone()
        } else {
            castles(repo.path(path))?
        };
        result.insert(PathBuf::from(output), castle_sql(tag, &rows));
    }
    result.insert(
        PathBuf::from(SCENARIO),
        json_text(&scenario_from_registry(repo, &test_rows)?)?,
    );
    result.insert(
        PathBuf::from(REGISTRY_SCENARIO),
        json_text(&registry_scenario(repo)?)?,
    );
    result.insert(
        PathBuf::from(PACT_SCENARIO),
        json_text(&pact_scenario_from_registry(repo, &test_rows)?)?,
    );
    Ok(result)
}

pub fn outputs(repo: &Repository) -> Result<Outputs> {
    outputs_from_test_map(repo, &test_map_assets(repo)?)
}

pub fn cli(repo: &Repository, args: &[String]) -> Result<()> {
    // The original main reads no arguments and prints nothing.
    let _ = args;
    verify_registry(repo)?;
    let assets = test_map_assets(repo)?;
    let generated = outputs_from_test_map(repo, &assets)?;
    let path = repo.path(TEST_MAP);
    let parent = path.parent().context("Test map has no parent directory")?;
    std::fs::create_dir_all(parent)?;
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("replace map {}", path.display()))?;
    }
    let mut destination = Connection::open(&path)?;
    {
        let backup = rusqlite::backup::Backup::new(&assets.database, &mut destination)?;
        backup.run_to_completion(128, std::time::Duration::from_millis(1), None)?;
    }
    destination.execute_batch("VACUUM")?;
    drop(destination);
    apply_outputs(repo, generated, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use regex::Regex;
    use roxmltree::{Document, Node};
    use rusqlite::types::Value as SqlValue;

    const SCHEMA: &str = r#"
CREATE TABLE Types (Type TEXT PRIMARY KEY, Kind TEXT);
CREATE TABLE Buildings (BuildingType TEXT PRIMARY KEY, Name TEXT NOT NULL, PrereqDistrict TEXT,
    Cost INT NOT NULL, InternalOnly INT NOT NULL DEFAULT 0);
CREATE TABLE Victories (VictoryType TEXT PRIMARY KEY, Name TEXT, Description TEXT,
    Blurb TEXT, RequirementSetId TEXT, EnabledByDefault INT, OneMoreTurn INT, Icon TEXT);
CREATE TABLE RequirementSets (RequirementSetId TEXT PRIMARY KEY, RequirementSetType TEXT);
CREATE TABLE RequirementSetRequirements (RequirementSetId TEXT, RequirementId TEXT,
    PRIMARY KEY (RequirementSetId, RequirementId));
CREATE TABLE Requirements (RequirementId TEXT PRIMARY KEY, RequirementType TEXT, Inverse INT);
CREATE TABLE RequirementArguments (RequirementId TEXT, Name TEXT, Value TEXT,
    PRIMARY KEY (RequirementId, Name));
CREATE TABLE RequirementStrings (RequirementId TEXT, Context TEXT, Text TEXT);
"#;

    const FOUR: [(i64, i64); 4] = [(6, 5), (6, 14), (25, 5), (25, 14)];

    #[derive(Clone, Copy, Debug)]
    struct City {
        owner: i64,
        x: i64,
        y: i64,
        rebuilt: bool,
        unified: bool,
    }

    fn city(owner: i64, at: (i64, i64)) -> City {
        City {
            owner,
            x: at.0,
            y: at.1,
            rebuilt: false,
            unified: false,
        }
    }

    fn held(owner: i64, coordinates: &[(i64, i64)]) -> Vec<City> {
        coordinates.iter().map(|&at| city(owner, at)).collect()
    }

    fn four_sql() -> String {
        castle_sql(
            "T",
            &FOUR.iter().map(|&(x, y)| (x, y, 0)).collect::<Vec<_>>(),
        )
    }

    struct RequirementModel {
        sets: BTreeMap<String, String>,
        members: BTreeMap<String, Vec<String>>,
        reqs: IndexMap<String, (String, bool)>,
        args: BTreeMap<String, BTreeMap<String, String>>,
        root: String,
    }

    impl RequirementModel {
        fn new(db: &Connection) -> Result<Self> {
            let sets = db
                .prepare("SELECT RequirementSetId,RequirementSetType FROM RequirementSets")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<BTreeMap<String, String>>>()?;
            let mut members: BTreeMap<String, Vec<String>> = BTreeMap::new();
            let mut statement = db.prepare(
                "SELECT RequirementSetId,RequirementId FROM RequirementSetRequirements ORDER BY rowid",
            )?;
            for row in
                statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            {
                let (set, req) = row?;
                members.entry(set).or_default().push(req);
            }
            let reqs = db
                .prepare(
                    "SELECT RequirementId,RequirementType,Inverse FROM Requirements ORDER BY rowid",
                )?
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        (
                            r.get::<_, String>(1)?,
                            r.get::<_, Option<i64>>(2)?.unwrap_or(0) != 0,
                        ),
                    ))
                })?
                .collect::<rusqlite::Result<IndexMap<_, _>>>()?;
            let mut args: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
            let mut statement =
                db.prepare("SELECT RequirementId,Name,Value FROM RequirementArguments")?;
            for row in statement.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })? {
                let (req, name, value) = row?;
                args.entry(req).or_default().insert(name, value);
            }
            let root = db.query_row(
                "SELECT RequirementSetId FROM Victories WHERE VictoryType=?",
                [VICTORY],
                |r| r.get(0),
            )?;
            Ok(Self {
                sets,
                members,
                reqs,
                args,
                root,
            })
        }

        fn wins(&self, team: i64, cities: &[City], domination: bool) -> bool {
            self.test_set(&self.root, team, None, cities, domination)
        }

        fn test_set(
            &self,
            set: &str,
            team: i64,
            city: Option<&City>,
            cities: &[City],
            domination: bool,
        ) -> bool {
            let results: Vec<_> = self.members[set]
                .iter()
                .map(|req| self.test(req, team, city, cities, domination))
                .collect();
            match self.sets[set].as_str() {
                "REQUIREMENTSET_TEST_ALL" => results.into_iter().all(|x| x),
                "REQUIREMENTSET_TEST_ANY" => results.into_iter().any(|x| x),
                other => panic!("Unmodelled requirement set {set}: {other}"),
            }
        }

        fn test(
            &self,
            req: &str,
            team: i64,
            city: Option<&City>,
            cities: &[City],
            domination: bool,
        ) -> bool {
            let (kind, inverse) = &self.reqs[req];
            let empty = BTreeMap::new();
            let args = self.args.get(req).unwrap_or(&empty);
            let result = match kind.as_str() {
                "REQUIREMENT_REQUIREMENTSET_IS_MET" => {
                    self.test_set(&args["RequirementSetId"], team, city, cities, domination)
                }
                kind if kind.starts_with("REQUIREMENT_COLLECTION_") => {
                    assert_eq!(
                        args["CollectionType"], "COLLECTION_ALL_CITIES",
                        "Unexpected arguments on {req}"
                    );
                    let met: Vec<_> = cities
                        .iter()
                        .map(|city| {
                            self.test_set(
                                &args["RequirementSetId"],
                                team,
                                Some(city),
                                cities,
                                domination,
                            )
                        })
                        .collect();
                    match kind {
                        "REQUIREMENT_COLLECTION_ALL_MET" => met.iter().all(|&x| x),
                        "REQUIREMENT_COLLECTION_COUNT_ATLEAST" => {
                            met.iter().filter(|&&x| x).count()
                                >= args["Count"].parse::<usize>().unwrap()
                        }
                        "REQUIREMENT_COLLECTION_COUNT_EQUALS" => {
                            met.iter().filter(|&&x| x).count()
                                == args["Count"].parse::<usize>().unwrap()
                        }
                        other => panic!("Unmodelled requirement {req}: {other}"),
                    }
                }
                "REQUIREMENT_CITY_LOCATION_MATCHES" => {
                    let city = city.expect("Location requirement requires a city");
                    // Coordinates remain coordinates when marker fields are present.
                    (city.x, city.y)
                        == (
                            args["X"].parse::<i64>().unwrap(),
                            args["Y"].parse::<i64>().unwrap(),
                        )
                }
                "REQUIREMENT_CITY_HAS_BUILDING" => {
                    let city = city.expect("Building requirement requires a city");
                    match args["BuildingType"].as_str() {
                        REBUILT => city.rebuilt,
                        UNIFIED => city.unified,
                        other => panic!("Unmodelled building {other}"),
                    }
                }
                "REQUIREMENT_PLAYER_OWNS_OBJECT" => {
                    city.expect("Ownership requirement requires a city").owner == team
                }
                "REQUIREMENT_TEAM_DOMINATION_VICTORY" => domination,
                other => panic!("Unmodelled requirement {req}: {other}"),
            };
            result != *inverse
        }
    }

    fn generated_model(scripts: &[String]) -> Result<RequirementModel> {
        let db = Connection::open_in_memory()?;
        db.execute_batch(SCHEMA)?;
        db.execute_batch(&sql())?;
        for script in scripts {
            db.execute_batch(script)?;
        }
        RequirementModel::new(&db)
    }

    // The migration inventory includes this evaluator method as a baseline ID.
    // Pin its ALL/ANY semantics separately from the victory scenario assertions.
    #[test]
    fn test_set() -> Result<()> {
        let mut model = generated_model(&[])?;
        model.reqs.insert(
            "OWNER".into(),
            ("REQUIREMENT_PLAYER_OWNS_OBJECT".into(), false),
        );
        model.reqs.insert(
            "DOMINATION".into(),
            ("REQUIREMENT_TEAM_DOMINATION_VICTORY".into(), false),
        );
        for (set, kind) in [
            ("ALL", "REQUIREMENTSET_TEST_ALL"),
            ("ANY", "REQUIREMENTSET_TEST_ANY"),
        ] {
            model.sets.insert(set.into(), kind.into());
            model
                .members
                .insert(set.into(), vec!["OWNER".into(), "DOMINATION".into()]);
        }
        for (owner, domination, all, any) in [
            (0, false, false, true),
            (1, false, false, false),
            (0, true, true, true),
            (1, true, false, true),
        ] {
            let city = City {
                owner,
                x: 6,
                y: 5,
                rebuilt: false,
                unified: false,
            };
            assert_eq!(
                model.test_set("ALL", 0, Some(&city), &[city], domination),
                all
            );
            assert_eq!(
                model.test_set("ANY", 0, Some(&city), &[city], domination),
                any
            );
        }
        model.members.insert("ALL".into(), vec![]);
        model.members.insert("ANY".into(), vec![]);
        assert!(model.test_set("ALL", 0, None, &[], false));
        assert!(!model.test_set("ANY", 0, None, &[], false));
        Ok(())
    }

    fn nodes<'a, 'input>(root: Node<'a, 'input>, path: &str) -> Vec<Node<'a, 'input>> {
        let mut result = vec![root];
        for part in path.split('/').filter(|s| !s.is_empty() && *s != ".") {
            result = result
                .into_iter()
                .flat_map(|node| {
                    node.children()
                        .filter(move |child| child.is_element() && child.tag_name().name() == part)
                })
                .collect();
        }
        result
    }

    fn node_text(node: Node<'_, '_>) -> String {
        node.text().unwrap_or("").to_owned()
    }

    fn text_at(root: Node<'_, '_>, path: &str) -> String {
        node_text(*nodes(root, path).first().expect("Missing XML path"))
    }

    fn attr(node: Node<'_, '_>, name: &str) -> String {
        node.attribute(name)
            .expect("Missing XML attribute")
            .to_owned()
    }

    fn files(root: Node<'_, '_>, path: &str) -> Vec<String> {
        nodes(root, path).into_iter().map(node_text).collect()
    }

    fn string_set(items: impl IntoIterator<Item = impl Into<String>>) -> BTreeSet<String> {
        items.into_iter().map(Into::into).collect()
    }

    fn database_rows(db: &Connection, query: &str) -> Result<Vec<Vec<SqlValue>>> {
        let mut statement = db.prepare(query)?;
        let columns = statement.column_count();
        let result = statement
            .query_map([], |row| {
                (0..columns)
                    .map(|column| row.get(column))
                    .collect::<rusqlite::Result<Vec<SqlValue>>>()
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(result)
    }

    fn sv(value: &str) -> SqlValue {
        SqlValue::Text(value.to_owned())
    }

    fn code(step: &Value) -> &str {
        step.get("code")
            .and_then(Value::as_str)
            .or_else(|| step.get("condition")?.get("code")?.as_str())
            .unwrap_or("")
    }

    fn steps(data: &Value) -> &[Value] {
        data["steps"].as_array().expect("Missing steps")
    }

    fn actions(data: &[Value]) -> Vec<&str> {
        data.iter()
            .map(|step| step["action"].as_str().unwrap())
            .collect()
    }

    fn first(data: &[Value], needle: &str) -> usize {
        data.iter()
            .position(|step| code(step).contains(needle))
            .unwrap_or_else(|| panic!("Missing scenario code {needle}"))
    }

    fn indices(data: &[Value], needle: &str) -> Vec<usize> {
        data.iter()
            .enumerate()
            .filter_map(|(i, step)| code(step).contains(needle).then_some(i))
            .collect()
    }

    fn compile_lua(data: &Value) {
        let lua = mlua::Lua::new();
        for step in steps(data) {
            let code = code(step);
            if !code.is_empty() {
                lua.load(code)
                    .into_function()
                    .unwrap_or_else(|e| panic!("Invalid Lua scenario step: {e}\n{code}"));
            }
        }
    }

    #[test]
    fn test_map_cities_are_the_registry_and_settler_maps_have_none() -> Result<()> {
        let repo = Repository::checked_in();
        let manifest = repo.json(format!("{MAP_MOD}/Maps/manifest.json"))?;
        for item in manifest.as_array().context("Missing manifest array")? {
            let path = repo.path(format!("{MAP_MOD}/Maps/{}", field(item, "file")?));
            let db = open_readonly(&path)?;
            let width: i64 = db.query_row("SELECT Width FROM Map", [], |r| r.get(0))?;
            let count: i64 = db.query_row(
                "SELECT COUNT(*) FROM Buildings WHERE BuildingType IN (?, ?)",
                params![REBUILT, "BUILDING_JAC_REGISTERED_CASTLE"],
                |r| r.get(0),
            )?;
            assert_eq!(count, 0, "scale {}", item["km_per_tile"]);
            let cities = item["cities"]
                .as_array()
                .context("Missing manifest cities")?;
            let plots: BTreeSet<_> = cities
                .iter()
                .map(|city| city["plot"].as_i64().unwrap())
                .collect();
            assert_eq!(
                castles(&path)?
                    .iter()
                    .map(|&(x, y, _)| y * width + x)
                    .collect::<BTreeSet<_>>(),
                plots
            );
            let owners: BTreeSet<_> = cities
                .iter()
                .map(|city| city["owner"].as_str().unwrap())
                .collect();
            assert!(BTreeSet::from(["RYUKYU", "AINU"]).is_subset(&owners));
            assert_eq!(
                item["bytes"].as_u64().context("Missing manifest bytes")?,
                std::fs::metadata(&path)?.len()
            );
            let settler = open_readonly(
                &repo.path(format!("{MAP_MOD}/Maps/{}", field(item, "settler_file")?)),
            )?;
            assert_eq!(
                settler.query_row("SELECT COUNT(*) FROM Cities", [], |r| r.get::<_, i64>(0))?,
                0
            );
        }
        assert_eq!(verify_registry(&repo)?, 56);

        let map_text = repo.read(format!("{MAP_MOD}/JAC_PoC_JapanMapScale.modinfo"))?;
        let test_text = repo.read(format!("{TEST_MOD}/JAC_Test_UnificationMap.modinfo"))?;
        let map_doc = Document::parse(&map_text)?;
        let test_doc = Document::parse(&test_text)?;
        let map_root = map_doc.root_element();
        let test_root = test_doc.root_element();
        let mut criteria = BTreeMap::new();
        for root in [map_root, test_root] {
            for criterion in nodes(root, "ActionCriteria/Criteria") {
                criteria.insert(
                    attr(criterion, "id"),
                    nodes(criterion, "ConfigurationValueMatches/Value")
                        .first()
                        .copied()
                        .map(node_text),
                );
            }
        }
        let mut registered = BTreeMap::new();
        for (root, folder) in [(map_root, MAP_MOD), (test_root, TEST_MOD)] {
            let listed: BTreeSet<_> = files(root, "Files/File").into_iter().collect();
            for action in nodes(root, "InGameActions/UpdateDatabase") {
                for file in files(action, "File") {
                    if file.ends_with("_Castles.sql") {
                        assert!(listed.contains(&file));
                        assert_eq!(text_at(action, "Properties/LoadOrder"), "300");
                        registered.insert(
                            format!("{folder}/{file}"),
                            criteria[&attr(action, "criteria")].clone(),
                        );
                    }
                }
            }
        }
        for (path, tag, output) in CASTLE_FILES {
            assert_eq!(
                repo.read(output)?,
                castle_sql(tag, &castles(repo.path(path))?)
            );
            let root = if path.starts_with(MAP_MOD) {
                map_root
            } else {
                test_root
            };
            let filename = Path::new(path).file_name().unwrap().to_str().unwrap();
            assert_eq!(
                registered
                    .remove(output)
                    .context("Unregistered castle file")?,
                Some(format!("{{{}}}Maps/{filename}", attr(root, "id")))
            );
        }
        assert!(registered.is_empty());
        assert!(!repo
            .path(format!("{MAP_MOD}/Data/JAC_RegisteredCastle.xml"))
            .exists());
        let tree_text = repo.read(format!("{TREE_MOD}/JAC_PoC_SengokuTrees.modinfo"))?;
        let tree_doc = Document::parse(&tree_text)?;
        let action = tree_doc
            .descendants()
            .find(|node| {
                node.has_tag_name("UpdateDatabase")
                    && files(*node, "File").contains(&"Data/JAC_Sengoku_Unification.sql".to_owned())
            })
            .context("Victory SQL is not registered")?;
        assert!(text_at(action, "Properties/LoadOrder").parse::<i64>()? < 300);
        Ok(())
    }

    #[test]
    fn test_checked_in_sql_is_generated_without_a_castle_count() -> Result<()> {
        let repo = Repository::checked_in();
        let script = repo.read(OUTPUT)?;
        assert_eq!(script, sql());
        let model = generated_model(&[])?;
        let counts: BTreeMap<_, _> = model
            .args
            .iter()
            .filter_map(|(req, args)| {
                args.get("Count")
                    .map(|count| (req.as_str(), count.as_str()))
            })
            .collect();
        assert_eq!(
            counts,
            BTreeMap::from([
                ("JAC_UNIFICATION_CASTLES_EXIST", "1"),
                ("JAC_UNIFICATION_NO_CASTLES", "0"),
                ("JAC_UNIFICATION_PACT", "1"),
            ])
        );
        assert_eq!(
            model
                .reqs
                .iter()
                .filter(|(_, (kind, _))| kind == "REQUIREMENT_CITY_HAS_BUILDING")
                .map(|(req, (_, inverse))| (req.as_str(), *inverse))
                .collect::<Vec<_>>(),
            [
                ("JAC_UNIFICATION_CITY_PACT_UNIFIED", false),
                ("JAC_UNIFICATION_CITY_NOT_REBUILT", true),
            ]
        );
        assert_eq!(model.sets[COUNTED_SET], "REQUIREMENTSET_TEST_ALL");
        assert_eq!(
            model.members[COUNTED_SET],
            [
                "JAC_UNIFICATION_CITY_ON_CASTLE_PLOT",
                "JAC_UNIFICATION_CITY_NOT_REBUILT",
            ]
        );
        let references: BTreeMap<_, _> = model
            .args
            .iter()
            .filter_map(|(req, args)| {
                args.get("RequirementSetId")
                    .filter(|set| set.as_str() == CASTLE_SET || set.as_str() == COUNTED_SET)
                    .map(|set| (req.as_str(), set.as_str()))
            })
            .collect();
        assert_eq!(
            references,
            BTreeMap::from([
                ("JAC_UNIFICATION_CASTLES_EXIST", COUNTED_SET),
                ("JAC_UNIFICATION_NO_CASTLES", COUNTED_SET),
                ("JAC_UNIFICATION_CITY_NOT_CASTLE", COUNTED_SET),
                ("JAC_UNIFICATION_CITY_ON_CASTLE_PLOT", CASTLE_SET),
            ])
        );
        assert_eq!(model.members[CASTLE_SET], ["JAC_UNIFICATION_CASTLE_NONE"]);
        assert_eq!(
            model.args["JAC_UNIFICATION_CASTLE_NONE"],
            BTreeMap::from([
                ("X".to_owned(), "-1".to_owned()),
                ("Y".to_owned(), "-1".to_owned()),
            ])
        );
        assert_eq!(model.sets[&model.root], "REQUIREMENTSET_TEST_ANY");
        assert_eq!(
            model.members[&model.root],
            [
                "JAC_UNIFICATION_CASTLES",
                "JAC_UNIFICATION_CAPITALS",
                "JAC_UNIFICATION_PACT",
            ]
        );
        assert_eq!(
            model.members["JAC_UNIFICATION_PACT_CITY"],
            [
                "JAC_UNIFICATION_CITY_OWNED",
                "JAC_UNIFICATION_CITY_PACT_UNIFIED",
            ]
        );
        assert_eq!(
            model.reqs["JAC_UNIFICATION_ALL_CAPITALS"],
            ("REQUIREMENT_TEAM_DOMINATION_VICTORY".to_owned(), false)
        );
        assert_eq!(
            model.reqs["JAC_UNIFICATION_CITY_NOT_CASTLE"],
            ("REQUIREMENT_REQUIREMENTSET_IS_MET".to_owned(), true)
        );
        let db = Connection::open_in_memory()?;
        db.execute_batch(SCHEMA)?;
        db.execute_batch(&script)?;
        assert_eq!(
            database_rows(&db, "SELECT Type,Kind FROM Types ORDER BY rowid")?,
            vec![
                vec![sv(VICTORY), sv("KIND_VICTORY")],
                vec![sv(REBUILT), sv("KIND_BUILDING")],
                vec![sv(UNIFIED), sv("KIND_BUILDING")],
            ]
        );
        assert_eq!(
            database_rows(&db, "SELECT * FROM Buildings ORDER BY rowid")?,
            [REBUILT, UNIFIED]
                .iter()
                .map(|building| vec![
                    sv(building),
                    sv(&format!("LOC_{building}_NAME")),
                    sv("DISTRICT_CITY_CENTER"),
                    SqlValue::Integer(1),
                    SqlValue::Integer(1),
                ])
                .collect::<Vec<_>>()
        );
        assert_eq!(
            database_rows(
                &db,
                "SELECT RequirementSetId,EnabledByDefault,OneMoreTurn FROM Victories"
            )?,
            vec![vec![
                sv("JAC_UNIFICATION_VICTORY_REQUIREMENTS"),
                SqlValue::Integer(1),
                SqlValue::Integer(0),
            ]]
        );
        let strings = db
            .prepare("SELECT RequirementId,Text FROM RequirementStrings")?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
        assert_eq!(
            strings,
            [
                (
                    "JAC_UNIFICATION_CASTLES",
                    "LOC_VICTORY_JAC_UNIFICATION_PROGRESS"
                ),
                (
                    "JAC_UNIFICATION_CAPITALS",
                    "LOC_VICTORY_JAC_UNIFICATION_CAPITALS_PROGRESS"
                ),
                (
                    "JAC_UNIFICATION_PACT",
                    "LOC_VICTORY_JAC_UNIFICATION_PACT_PROGRESS"
                ),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect()
        );
        Ok(())
    }

    #[test]
    fn test_victory_follows_owned_castles_razes_and_settler_capitals() -> Result<()> {
        let model = generated_model(&[four_sql()])?;
        let four = [
            city(0, FOUR[0]),
            city(0, FOUR[1]),
            city(1, FOUR[2]),
            city(1, FOUR[3]),
        ];
        assert!(!model.wins(0, &four, false));
        let mut missing = four;
        missing[2].owner = 0;
        assert!(!model.wins(0, &missing, false), "one castle missing");
        assert!(model.wins(0, &held(0, &FOUR), false));
        assert!(model.wins(0, &held(0, &FOUR[..3]), false));
        let mut extra = held(0, &FOUR[..3]);
        extra.extend([city(1, (16, 10)), city(2, (12, 8))]);
        assert!(model.wins(0, &extra, false));
        let mut other = held(0, &FOUR[..3]);
        other.push(city(1, (16, 10)));
        assert!(!model.wins(1, &other, false));
        assert!(!model.wins(0, &four, true));
        let bare = generated_model(&[])?;
        let ordinary = [city(0, FOUR[0]), city(1, FOUR[2])];
        assert!(!bare.wins(0, &ordinary, false));
        assert!(!bare.wins(0, &[city(0, FOUR[0])], false));
        assert!(bare.wins(0, &ordinary, true));
        Ok(())
    }

    #[test]
    fn test_captured_or_transferred_castle_still_counts() -> Result<()> {
        let model = generated_model(&[four_sql()])?;
        let before = [city(0, FOUR[0]), city(0, FOUR[1]), city(1, FOUR[2])];
        assert!(!model.wins(0, &before, false));
        assert!(!model.wins(1, &before, false));
        let mut captured = before;
        captured[2].owner = 0;
        assert!(model.wins(0, &captured, false));
        assert!(!model.wins(1, &captured, false));
        assert!(model.wins(1, &held(1, &FOUR), false));
        let mut missing = held(0, &FOUR);
        missing[3].owner = 1;
        assert!(!model.wins(0, &missing, false));
        Ok(())
    }

    #[test]
    fn test_city_rebuilt_on_a_razed_castle_does_not_count() -> Result<()> {
        let model = generated_model(&[four_sql()])?;
        let mut world = held(0, &FOUR[..3]);
        let mut rebuilt = city(1, FOUR[3]);
        rebuilt.rebuilt = true;
        world.push(rebuilt);
        assert!(
            model.wins(0, &world, false),
            "Date's rebuilt city blocks nothing"
        );
        assert!(!model.wins(1, &world, false));
        world[3].owner = 0;
        assert!(model.wins(0, &world, false));
        world[3] = city(1, FOUR[3]);
        assert!(!model.wins(0, &world, false));
        let mut rebuilt_only = held(0, &FOUR[..2]);
        for city in &mut rebuilt_only {
            city.rebuilt = true;
        }
        assert!(!model.wins(0, &rebuilt_only, false));
        assert!(model.wins(0, &rebuilt_only, true));
        Ok(())
    }

    #[test]
    fn test_pact_marker_lets_only_the_lead_team_win() -> Result<()> {
        let model = generated_model(&[four_sql()])?;
        let unmarked = [
            city(0, FOUR[0]),
            city(0, FOUR[1]),
            city(1, FOUR[2]),
            city(1, FOUR[3]),
        ];
        let mut pooled = unmarked;
        pooled[0].unified = true;
        assert!(
            model.wins(0, &pooled, false),
            "the lead's team owns the marked capital"
        );
        assert!(!model.wins(1, &pooled, false), "the partner does not win");
        assert!(!model.wins(2, &pooled, false));
        assert!(!(0..3).any(|team| model.wins(team, &unmarked, false)));
        pooled[0].owner = 1;
        assert!(model.wins(1, &pooled, false));
        assert!(!model.wins(0, &pooled, false));
        Ok(())
    }

    #[test]
    fn test_pact_script_and_panel_are_registered() -> Result<()> {
        let repo = Repository::checked_in();
        let source = repo.read(format!("{TREE_MOD}/JAC_PoC_SengokuTrees.modinfo"))?;
        let doc = Document::parse(&source)?;
        let root = doc.root_element();
        let scripts: BTreeMap<_, _> = nodes(root, "InGameActions/AddGameplayScripts")
            .into_iter()
            .map(|action| (attr(action, "id"), files(action, "File")))
            .collect();
        assert_eq!(
            scripts["JAC_Sengoku_UnificationPact"],
            ["Scripts/JAC_Sengoku_UnificationPact.lua"]
        );
        let ui = nodes(root, "InGameActions/AddUserInterfaces")
            .into_iter()
            .find(|node| node.attribute("id") == Some("JAC_Sengoku_UnificationPactUI"))
            .context("Missing pact UI action")?;
        assert_eq!(text_at(ui, "Properties/Context"), "InGame");
        assert_eq!(files(ui, "File"), ["UI/JAC_UnificationPact.xml"]);
        let imported = files(root, "InGameActions/ImportFiles/File");
        let listed = files(root, "Files/File");
        for name in ["UI/JAC_UnificationPact.xml", "UI/JAC_UnificationPact.lua"] {
            assert!(imported.iter().any(|file| file == name));
            assert!(listed.iter().any(|file| file == name));
        }
        assert!(listed.contains(&"Scripts/JAC_Sengoku_UnificationPact.lua".to_owned()));
        let pact = repo.read(format!(
            "{TREE_MOD}/Scripts/JAC_Sengoku_UnificationPact.lua"
        ))?;
        let panel = repo.read(format!("{TREE_MOD}/UI/JAC_UnificationPact.lua"))?;
        for key in [PACT_LEAD_PREFIX, PACT_PROPOSAL_PREFIX, PACT_RESULT] {
            let needle = format!("\"{key}\"");
            assert!(pact.contains(&needle));
            assert!(panel.contains(&needle));
        }
        assert!(pact.contains(&format!("GameInfo.Buildings[\"{UNIFIED}\"]")));
        assert!(panel.contains("PlayerOperations.EXECUTE_SCRIPT"));
        for call in ["SetProperty", "CreateIncompleteBuilding", "TransferCity"] {
            assert!(!panel.contains(call));
        }
        for event in [
            "JAC_UnificationPactPropose",
            "JAC_UnificationPactAccept",
            "JAC_UnificationPactLeave",
        ] {
            assert!(pact.contains(&format!("GameEvents.{event}.Add(")));
        }
        let localization =
            repo.read(format!("{TREE_MOD}/Text/JAC_Sengoku_Unification_Text.xml"))?;
        let text_doc = Document::parse(&localization)?;
        let texts: BTreeSet<_> = nodes(text_doc.root_element(), "LocalizedText/Row")
            .into_iter()
            .map(|row| attr(row, "Tag"))
            .collect();
        let xml = repo.read(format!("{TREE_MOD}/UI/JAC_UnificationPact.xml"))?;
        let tags = Regex::new(r#""(LOC_JAC_UNIFICATION_PACT_[A-Z_]+)""#)?;
        let mut used: BTreeSet<_> = tags
            .captures_iter(&(panel.clone() + &xml))
            .map(|capture| capture[1].to_owned())
            .collect();
        used.remove("LOC_JAC_UNIFICATION_PACT_RESULT_");
        let outcomes = Regex::new(r#"SetProperty\(RESULT, "([A-Z_]+)"\)"#)?;
        used.extend(
            outcomes
                .captures_iter(&pact)
                .map(|capture| format!("LOC_JAC_UNIFICATION_PACT_RESULT_{}", &capture[1])),
        );
        assert_eq!(used.len(), 15);
        assert!(used.is_subset(&texts));
        let lua = mlua::Lua::new();
        lua.load(&pact)
            .into_function()
            .expect("Pact script must compile");
        lua.load(&panel)
            .into_function()
            .expect("Pact panel must compile");
        Ok(())
    }

    #[test]
    fn test_rebuilt_castle_script_marks_razed_plots_and_survives_capture() -> Result<()> {
        let repo = Repository::checked_in();
        let script = repo.read(format!("{TREE_MOD}/Scripts/JAC_Sengoku_RebuiltCastles.lua"))?;
        for needle in [
            format!("\"{CASTLE_SET}\""),
            format!("\"{REBUILT}\""),
            format!("\"{RAZED_PREFIX}\""),
            "GameEvents.CityBuilt.Add(OnCityBuilt)".to_owned(),
            "GameEvents.CityConquered.Add(OnCityConquered)".to_owned(),
            "GameEvents.OnGameTurnStarted.Add(OnGameTurnStarted)".to_owned(),
            "Game:GetProperty(RAZED_PREFIX".to_owned(),
            "CreateIncompleteBuilding(MARKER.Index".to_owned(),
            "local function OnCityConquered(capturerID, ownerID, cityID, x, y)".to_owned(),
            "local function OnCityBuilt(playerID, cityID, x, y)".to_owned(),
        ] {
            assert!(script.contains(&needle), "Missing Lua hook: {needle}");
        }
        assert!(!script.contains("CityRemovedFromMap.Add"));
        let start = script.find("local function OnGameTurnStarted").unwrap();
        let end = script.find("GameEvents.CityBuilt.Add").unwrap();
        let scan = &script[start..end];
        assert_eq!(script.matches("Game:SetProperty(RAZED_PREFIX").count(), 1);
        assert!(scan.contains("Game:SetProperty(RAZED_PREFIX"));
        assert!(scan.contains("not CityManager.GetCityAt(plot:GetX(), plot:GetY())"));
        assert!(scan.contains("Game.GetCurrentGameTurn() > GameConfiguration.GetStartTurn()"));
        let source = repo.read(format!("{TREE_MOD}/JAC_PoC_SengokuTrees.modinfo"))?;
        let doc = Document::parse(&source)?;
        let root = doc.root_element();
        let scripts: BTreeMap<_, _> = nodes(root, "InGameActions/AddGameplayScripts")
            .into_iter()
            .map(|action| (attr(action, "id"), files(action, "File")))
            .collect();
        assert_eq!(
            scripts["JAC_Sengoku_RebuiltCastles"],
            ["Scripts/JAC_Sengoku_RebuiltCastles.lua"]
        );
        assert!(files(root, "Files/File")
            .contains(&"Scripts/JAC_Sengoku_RebuiltCastles.lua".to_owned()));
        mlua::Lua::new()
            .load(&script)
            .into_function()
            .expect("Rebuilt-castle script must compile");
        Ok(())
    }

    #[test]
    fn test_castle_files_do_not_depend_on_load_order() -> Result<()> {
        for scripts in [[sql(), four_sql()], [four_sql(), sql()]] {
            let db = Connection::open_in_memory()?;
            db.execute_batch(SCHEMA)?;
            for script in scripts {
                db.execute_batch(&script)?;
            }
            let count: i64 = db.query_row(
                "SELECT COUNT(*) FROM RequirementSetRequirements WHERE RequirementSetId=?",
                [CASTLE_SET],
                |r| r.get(0),
            )?;
            assert_eq!(count, 5);
        }
        Ok(())
    }

    #[test]
    fn test_victory_is_in_sengoku_domain_and_visible_in_official_rankings() -> Result<()> {
        let repo = Repository::checked_in();
        let source = repo.read(format!("{TREE_MOD}/Config/JAC_Sengoku_Victories.xml"))?;
        let config = Document::parse(&source)?;
        assert_eq!(
            nodes(config.root_element(), "Victories/Row")
                .into_iter()
                .map(|row| (
                    attr(row, "Domain"),
                    attr(row, "VictoryType"),
                    attr(row, "EnabledByDefault")
                ))
                .collect::<Vec<_>>(),
            vec![(
                "JAC_Sengoku_Victories".to_owned(),
                VICTORY.to_owned(),
                "true".to_owned()
            )]
        );
        let source = repo.read(format!("{TREE_MOD}/Text/JAC_Sengoku_Unification_Text.xml"))?;
        let text = Document::parse(&source)?;
        let mut tags: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for row in nodes(text.root_element(), "LocalizedText/Row") {
            tags.entry(attr(row, "Tag"))
                .or_default()
                .insert(attr(row, "Language"));
            assert!(!text_at(row, "Text").is_empty());
        }
        let mut expected = string_set(
            [
                "NAME",
                "DESCRIPTION",
                "BLURB",
                "PROGRESS",
                "CAPITALS_PROGRESS",
            ]
            .map(|part| format!("LOC_VICTORY_JAC_UNIFICATION_{part}")),
        );
        expected.extend([
            format!("LOC_{REBUILT}_NAME"),
            format!("LOC_{UNIFIED}_NAME"),
            "LOC_VICTORY_JAC_UNIFICATION_PACT_PROGRESS".to_owned(),
        ]);
        expected.extend(
            tags.keys()
                .filter(|tag| tag.starts_with("LOC_JAC_UNIFICATION_PACT_"))
                .cloned(),
        );
        assert_eq!(tags.keys().cloned().collect::<BTreeSet<_>>(), expected);
        let languages = string_set(["ja_JP", "en_US", "ru_RU"]);
        assert!(tags.values().all(|actual| actual == &languages));
        let source = repo.read(format!("{TREE_MOD}/JAC_PoC_SengokuTrees.modinfo"))?;
        let info = Document::parse(&source)?;
        assert_eq!(
            files(info.root_element(), "FrontEndActions/UpdateText/File"),
            ["Text/JAC_Sengoku_Unification_Text.xml"]
        );
        Ok(())
    }

    #[test]
    fn test_test_map_registry_comes_from_map_data_and_stays_in_a_test_only_mod() -> Result<()> {
        let repo = Repository::checked_in();
        let db = open_readonly(&repo.path(TEST_MAP))?;
        let registry = castle_rows(&db)?;
        let mut owners: Vec<_> = registry.values().copied().collect();
        owners.sort();
        assert_eq!(owners, [0, 0, 1, 1]);
        assert_eq!(
            castles_from_connection(&db)?
                .into_iter()
                .map(|(x, y, _)| (x, y))
                .collect::<BTreeSet<_>>(),
            FOUR.into_iter().collect()
        );
        assert_eq!(
            database_rows(&db, "SELECT ID,LeaderType FROM Players ORDER BY ID")?,
            vec![
                vec![SqlValue::Integer(0), sv("LEADER_JAC_ODA_NOBUHIDE")],
                vec![SqlValue::Integer(1), sv("LEADER_JAC_DATE_TANEMUNE")],
                vec![SqlValue::Integer(2), sv("LEADER_JAC_ASHINA_MORIKIYO")],
            ]
        );
        assert_eq!(
            db.query_row("SELECT MapSizeType FROM Map", [], |r| r.get::<_, String>(0))?,
            MAP_SIZE
        );
        assert_eq!(
            db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))?,
            "ok"
        );
        let capitals = db
            .prepare("SELECT Plot FROM Buildings WHERE BuildingType='BUILDING_PALACE'")?
            .query_map([], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<BTreeSet<_>>>()?;
        assert_eq!(
            capitals
                .into_iter()
                .map(|plot| registry[&plot])
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([0, 1])
        );

        // The extracted official schema fixture makes regeneration mandatory
        // on every platform instead of skipping it when Civ6 is not installed.
        let fresh = test_map_assets(&repo)?;
        assert_eq!(castle_rows(&fresh.database)?, registry);
        for table in [
            "Plots",
            "PlotOwners",
            "Units",
            "StartPositions",
            "Buildings",
        ] {
            let query = format!("SELECT * FROM {table} ORDER BY 1, 2");
            assert_eq!(
                database_rows(&fresh.database, &query)?,
                database_rows(&db, &query)?,
                "{table}"
            );
        }
        assert_eq!(fresh.lua, TEST_MAP_LUA);
        assert_eq!(
            repo.read(format!("{TEST_MOD}/Maps/{MAP_NAME}.lua"))?,
            fresh.lua
        );

        let source = repo.read(format!("{TEST_MOD}/JAC_Test_UnificationMap.modinfo"))?;
        let info = Document::parse(&source)?;
        let root = info.root_element();
        assert_eq!(attr(root, "id"), TEST_MOD_ID);
        assert!(text_at(root, "LocalizedText/Text/en_US").contains("TEST"));
        assert_eq!(
            files(root, "Files/File")
                .into_iter()
                .collect::<BTreeSet<_>>(),
            string_set([
                "Config/JAC_Test_UnificationMap_Config.xml".to_owned(),
                "Text/JAC_Test_UnificationMap_Text.xml".to_owned(),
                "Data/JAC_Unification_Test_Castles.sql".to_owned(),
                format!("Maps/{MAP_NAME}.Civ6Map"),
                format!("Maps/{MAP_NAME}.lua"),
            ])
        );
        let source = repo.read(format!(
            "{TEST_MOD}/Config/JAC_Test_UnificationMap_Config.xml"
        ))?;
        let config = Document::parse(&source)?;
        let root = config.root_element();
        assert_eq!(
            nodes(root, "Maps/Row")
                .into_iter()
                .map(|row| attr(row, "File"))
                .collect::<Vec<_>>(),
            [MAP_FILE]
        );
        assert_eq!(
            nodes(root, "MapSizes/Row")
                .into_iter()
                .map(|row| attr(row, "MapSizeType"))
                .collect::<Vec<_>>(),
            [MAP_SIZE]
        );
        assert_eq!(
            nodes(root, "MapLeaders/Row")
                .into_iter()
                .map(|row| attr(row, "LeaderType"))
                .collect::<BTreeSet<_>>(),
            string_set([
                "LEADER_JAC_ODA_NOBUHIDE",
                "LEADER_JAC_DATE_TANEMUNE",
                "LEADER_JAC_ASHINA_MORIKIYO",
            ])
        );
        assert!(!repo
            .read(format!("{MAP_MOD}/Config/JAC_JapanMapScale_Config.xml"))?
            .contains(MAP_NAME));
        let source = repo.read(format!("{TEST_MOD}/Text/JAC_Test_UnificationMap_Text.xml"))?;
        let text = Document::parse(&source)?;
        let names: BTreeMap<_, _> = nodes(text.root_element(), "LocalizedText/Row")
            .into_iter()
            .filter(|row| row.attribute("Tag") == Some("LOC_JAC_TEST_UNIFICATION_MAP_NAME"))
            .map(|row| (attr(row, "Language"), text_at(row, "Text")))
            .collect();
        assert_eq!(
            names.keys().cloned().collect::<BTreeSet<_>>(),
            string_set(["ja_JP", "en_US", "ru_RU"])
        );
        assert!(names["ja_JP"].contains("テスト専用"));
        Ok(())
    }

    #[test]
    fn test_live_scenarios_use_the_test_map_and_a_light_real_map_check() -> Result<()> {
        let repo = Repository::checked_in();
        let data = repo.json(SCENARIO)?;
        assert_eq!(data, scenario(&repo)?);
        let steps = steps(&data);
        let actions = actions(steps);
        assert_eq!(
            &actions[..4],
            ["enable_mods", "configure", "ui_lua", "start_game"]
        );
        assert!(steps[0]["ids"]
            .as_array()
            .unwrap()
            .contains(&json!(TEST_MOD_ID)));
        assert_eq!(steps[1]["map_file"], MAP_FILE);
        assert_eq!(steps[1]["players"].as_array().unwrap().len(), 3);
        assert_eq!(
            steps
                .iter()
                .filter(|step| step
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .contains("TransferCity("))
                .count(),
            2
        );
        assert_eq!(
            actions.iter().filter(|&&action| action == "save").count(),
            1
        );
        assert_eq!(
            actions.iter().filter(|&&action| action == "load").count(),
            1
        );
        let negative = indices(steps, "victory with one castle missing");
        assert_eq!(negative.len(), 3);
        let save = actions.iter().position(|&action| action == "save").unwrap();
        let load = actions.iter().position(|&action| action == "load").unwrap();
        let autoplay = actions
            .iter()
            .position(|&action| action == "autoplay")
            .unwrap();
        assert!(
            negative[0] < autoplay
                && autoplay < negative[1]
                && negative[1] < save
                && save < load
                && load < negative[2]
        );
        let final_transfer = first(steps, "transferred the last castle");
        let rebuild = first(steps, "GetCities():Create(");
        assert!(load < rebuild && rebuild < final_transfer);
        assert_eq!(
            &actions[rebuild..final_transfer],
            ["ui_lua", "autoplay", "ui_lua"]
        );
        assert!(code(&steps[rebuild]).contains(RAZED_PREFIX));
        assert!(code(&steps[rebuild + 2]).contains("rebuilt city unmarked"));
        assert_eq!(
            &actions[final_transfer + 1..final_transfer + 4],
            ["ui_lua", "ui_lua", "wait_for"]
        );
        let own = &steps[final_transfer + 1];
        assert_eq!(own["state_name"], "GameCore_Tuner");
        assert!(code(own).contains("{25,5}"));
        assert!(code(own).contains("castle marked as rebuilt"));
        assert!(code(own).contains("owned~=3"));
        for index in [rebuild, rebuild + 2, final_transfer, final_transfer + 1] {
            assert_eq!(steps[index]["state_name"], "GameCore_Tuner");
        }
        assert!(!serde_json::to_string(&data)?.contains("BUILDING_JAC_REGISTERED_CASTLE"));
        assert!(code(&steps[final_transfer + 2]).contains("AutoplayManager.SetActive(true)"));
        assert!(code(&steps[final_transfer + 3]).contains(VICTORY));
        assert!(!code(steps.last().unwrap()).contains("castles"));

        let registry = repo.json(REGISTRY_SCENARIO)?;
        assert_eq!(registry, registry_scenario(&repo)?);
        assert_eq!(
            registry["steps"][1]["map_file"],
            "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map"
        );
        let registry_steps = super::tests::steps(&registry);
        assert!(code(registry_steps.last().unwrap()).contains("total~=56"));
        assert!(!serde_json::to_string(&registry)?.contains("TransferCity"));
        assert!(!super::tests::actions(registry_steps).contains(&"autoplay"));
        compile_lua(&data);
        compile_lua(&registry);

        let generated = outputs(&repo)?;
        assert_eq!(generated[Path::new(SCENARIO)], json_text(&data)?);
        assert_eq!(
            generated[Path::new(REGISTRY_SCENARIO)],
            json_text(&registry)?
        );
        Ok(())
    }

    #[test]
    fn test_pact_live_scenario_forms_the_pact_through_the_panel() -> Result<()> {
        let repo = Repository::checked_in();
        let data = repo.json(PACT_SCENARIO)?;
        let generated = pact_scenario(&repo)?;
        let differences: Vec<_> = data["steps"]
            .as_array()
            .unwrap()
            .iter()
            .zip(generated["steps"].as_array().unwrap())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| (i, a, b))
            .collect();
        assert!(
            data == generated,
            "scenario differs at steps {differences:?}"
        );
        let built = scenario(&repo)?;
        let steps = steps(&data);
        assert_eq!(&steps[..5], &self::steps(&built)[..5]);
        let codes = steps
            .iter()
            .map(serde_json::to_string)
            .collect::<serde_json::Result<Vec<_>>>()?;
        let propose = indices(steps, "JAC_RequestPact('Propose',date)");
        assert_eq!(propose.len(), 2);
        assert!(propose
            .iter()
            .all(|&i| steps[i]["state_name"] == "JAC_UnificationPact"));
        let not_allied = first(steps, "NOT_ALLIED");
        let allied = first(steps, "SetHasAllied(date,true)");
        let joined = first(
            steps,
            "=='JOINED' and Game:GetProperty('JAC_UNIFICATION_PACT_LEAD_'..date)",
        );
        let held_out = first(steps, "pact marked with 3 of 4 castles");
        let handover = first(steps, "TransferCity(last,date)");
        assert!(
            propose[0] < not_allied
                && not_allied < allied
                && allied < propose[1]
                && propose[1] < joined
                && joined < held_out
                && held_out < handover
        );
        let with_ashina = indices(steps, "JAC_RequestPact('Propose',ashina)");
        let leave = first(steps, "JAC_RequestPact('Leave',");
        let left = first(steps, "=='LEFT'");
        let war = first(steps, "DeclareWarOn(ashina");
        let dissolved = first(steps, "pact not dissolved by the war");
        assert_eq!(with_ashina.len(), 2);
        assert!(
            first(steps, "SetHasAllied(ashina,true)") < with_ashina[0]
                && with_ashina[0] < leave
                && leave < left
                && left < with_ashina[1]
                && with_ashina[1] < war
                && war < dissolved
                && dissolved < propose[0]
        );
        assert_eq!(actions(&steps[war + 1..dissolved]), ["autoplay"]);
        assert!(codes[dissolved].contains("MakePeaceWith(ashina,true)"));
        assert!(codes[dissolved].contains("proposed a pact to Oda while at war"));
        for i in with_ashina.into_iter().chain(std::iter::once(leave)) {
            assert_eq!(steps[i]["state_name"], "JAC_UnificationPact");
        }
        let actions = actions(steps);
        let save = actions.iter().position(|&action| action == "save").unwrap();
        assert_eq!(&actions[save..save + 3], ["save", "exit_to_menu", "load"]);
        assert!(held_out < save && save < handover);
        assert_eq!(
            steps[save + 3]["condition"]["code"],
            steps[joined]["condition"]["code"]
        );
        assert_eq!(
            codes
                .iter()
                .filter(|code| code.contains("TransferCity("))
                .count(),
            2
        );
        assert!(codes[handover + 1].contains(UNIFIED));
        assert!(codes[codes.len() - 3].contains("AutoplayManager.SetActive(true)"));
        assert!(codes[codes.len() - 2].contains("winner~=Players[date]:GetTeam()"));
        assert_eq!(steps.last().unwrap()["action"], "screenshot");
        compile_lua(&data);
        assert_eq!(outputs(&repo)?[Path::new(PACT_SCENARIO)], json_text(&data)?);
        Ok(())
    }

    #[test]
    fn test_live_scenarios_find_players_dead_or_alive() -> Result<()> {
        let repo = Repository::checked_in();
        for built in [scenario(&repo)?, pact_scenario(&repo)?] {
            for step in steps(&built) {
                let code = code(step);
                if code.contains("GetCivilizationTypeName") {
                    assert!(code.contains("PlayerManager.GetWasEverAliveCount()"));
                    assert!(!code.contains("GetAliveIDs()) do if PlayerConfigurations"));
                    assert!(!code.contains("local civ=PlayerConfigurations"));
                }
            }
        }

        let lua = mlua::Lua::new();
        lua.load(
            r#"
PlayerManager = {
    GetWasEverAliveCount = function() return 3 end,
    GetAliveIDs = function() return {0} end
}
PlayerConfigurations = {
    [0] = {GetCivilizationTypeName = function() return 'CIVILIZATION_JAC_ODA' end},
    [1] = {GetCivilizationTypeName = function() return 'CIVILIZATION_JAC_DATE' end},
    [2] = {GetCivilizationTypeName = function() return 'CIVILIZATION_JAC_ASHINA' end}
}
"#,
        )
        .exec()
        .map_err(|e| anyhow!("initialize player lookup fixture: {e}"))?;
        let lookup = player_lua("oda", TEST_SEATS[0].civ, "Oda")
            + &player_lua("date", TEST_SEATS[1].civ, "Date")
            + &player_lua("ashina", TEST_SEATS[2].civ, "Ashina")
            + "return oda,date,ashina";
        let ids: (i64, i64, i64) = lua
            .load(&lookup)
            .eval()
            .map_err(|e| anyhow!("evaluate dead-or-alive lookup: {e}"))?;
        assert_eq!(ids, (0, 1, 2));
        let missing = player_lua("absent", "CIVILIZATION_JAC_ABSENT", "Absent");
        assert!(lua.load(&missing).exec().is_err());
        Ok(())
    }
}
