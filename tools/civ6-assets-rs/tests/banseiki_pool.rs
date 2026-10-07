//! Banseiki pool (poc/JAC_PoC_BanseikiPool): ledger-driven generation, the official
//! gameplay and configuration schemas, the human-only setup filter and the save ledger.
//! Lua runs in mlua's Lua 5.1, including the unmodified stock SetupParameters module.
//! This does not prove runtime FrontEnd VFS loading; the live scenario asserts its marker.
use civ6_assets_rs::{banseiki_pool as pool, elements, generate::game_dir, read_json, root};
use mlua::{Function, Lua, Table, Value as LuaValue, Variadic};
use rusqlite::{params_from_iter, types::Value as Sql, types::ValueRef, Connection};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

type Row = BTreeMap<String, String>;

const DOMAIN: &str = pool::DOMAIN;
const BANSEIKI: &str = "RULESET_JAC_BANSEIKI";
const GS: &str = "RULESET_EXPANSION_2";
const CONTINENTS: &str = "Continents.lua";
const JAPAN_MAP: &str = "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_20km.Civ6Map";

fn mod_dir() -> PathBuf {
    root().join(pool::MOD_DIR)
}

fn official(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../civ6-data-rs/fixtures/game")
        .join(path)
}

fn read(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.trim_start_matches('\u{feff}').to_owned()
}

fn parse(path: &Path) -> xmltree::Element {
    xmltree::Element::parse(read(path).as_bytes()).unwrap()
}

/// Rows of every `<table>` section (attributes and child elements alike).
fn xml_rows(doc: &xmltree::Element, table: &str) -> Vec<Row> {
    let mut out = Vec::new();
    for section in elements(doc).filter(|e| e.name == table) {
        for e in elements(section).filter(|e| e.name == "Row" || e.name == "InsertOrIgnore") {
            let mut row: Row = e
                .attributes
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            for child in elements(e) {
                let text = child.get_text().map(|t| t.into_owned()).unwrap_or_default();
                row.insert(child.name.clone(), text);
            }
            out.push(row);
        }
    }
    out
}

fn columns(db: &Connection, table: &str) -> BTreeSet<String> {
    let mut statement = db
        .prepare(&format!("PRAGMA table_info(\"{table}\")"))
        .unwrap();
    let names = statement.query_map([], |r| r.get::<_, String>(1)).unwrap();
    names.map(Result::unwrap).collect()
}

fn insert(db: &Connection, table: &str, rows: &[Row]) {
    let known = columns(db, table);
    for row in rows {
        let fields: Vec<_> = row.iter().filter(|(k, _)| known.contains(*k)).collect();
        if fields.is_empty() {
            continue;
        }
        let names: Vec<String> = fields.iter().map(|(k, _)| format!("\"{k}\"")).collect();
        let marks = vec!["?"; fields.len()].join(", ");
        let sql = format!(
            "INSERT OR IGNORE INTO \"{table}\" ({}) VALUES ({marks})",
            names.join(", ")
        );
        let values = fields.iter().map(|(_, v)| match v.as_str() {
            "true" => "1".to_owned(),
            "false" => "0".to_owned(),
            v => v.to_owned(),
        });
        db.execute(&sql, params_from_iter(values)).unwrap();
    }
}

fn load_xml(db: &Connection, path: &Path) {
    let doc = parse(path);
    let tables: BTreeSet<String> = elements(&doc).map(|e| e.name.clone()).collect();
    for table in tables {
        insert(db, &table, &xml_rows(&doc, &table));
    }
}

fn text(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => "NULL".into(),
        ValueRef::Integer(n) => n.to_string(),
        ValueRef::Real(n) => n.to_string(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        ValueRef::Blob(b) => format!("{b:?}"),
    }
}

fn dump(db: &Connection, sql: &str, args: &[Sql]) -> Vec<Vec<String>> {
    let mut statement = db.prepare(sql).unwrap_or_else(|e| panic!("{sql}: {e}"));
    let width = statement.column_count();
    let rows = statement
        .query_map(params_from_iter(args.iter()), |r| {
            (0..width)
                .map(|i| Ok(text(r.get_ref(i)?)))
                .collect::<rusqlite::Result<Vec<String>>>()
        })
        .unwrap();
    rows.map(Result::unwrap).collect()
}

fn strings(db: &Connection, sql: &str, args: &[Sql]) -> BTreeSet<String> {
    dump(db, sql, args)
        .into_iter()
        .map(|r| r[0].clone())
        .collect()
}

fn people() -> Vec<pool::Person> {
    pool::people().unwrap()
}

#[test]
fn generated_files_match_the_ledger() {
    for (path, generated) in pool::outputs().unwrap() {
        let checked_in = fs::read_to_string(root().join(&path)).unwrap_or_default();
        assert!(
            checked_in == generated,
            "{} is missing or stale; run banseiki-pool-generate",
            path.display()
        );
    }
}

#[test]
fn every_candidate_is_registered_once_and_existing_leaders_are_reused() {
    let data = read_json(root().join(civ6_data_rs::pool::DATA_PATH)).unwrap();
    let rows = data["characters"].as_array().unwrap();
    let candidates: Vec<_> = rows
        .iter()
        .filter(|r| r["leader_candidate"] == true)
        .collect();
    let people = people();
    assert_eq!(people.len(), candidates.len());
    if rows.len() as u64 == data["target_size"].as_u64().unwrap() {
        // The full ledger: every one of the 300 people is a leader.
        assert_eq!(people.len(), 300);
    }
    let ids: BTreeSet<_> = people.iter().map(|p| &p.id).collect();
    let leaders: BTreeSet<_> = people.iter().map(|p| &p.leader).collect();
    assert_eq!(ids.len(), people.len());
    assert_eq!(leaders.len(), people.len());
    let base_civs: BTreeSet<_> = pool::REGIONS.iter().map(|r| r.3).collect();
    for (p, row) in people.iter().zip(candidates) {
        assert_eq!(p.id, row["id"].as_str().unwrap());
        if let Some(existing) = row["existing_leader_id"].as_str() {
            assert!(p.existing && p.leader == existing, "{}", p.id);
            continue;
        }
        assert!(!p.existing);
        assert_eq!(p.leader, format!("LEADER_{}", p.id));
        if p.faction.ends_with("（架空家臣）") {
            assert!(p.civ.starts_with("CIVILIZATION_JAC_"), "{}", p.id);
            assert!(p.source.starts_with("LEADER_JAC_"), "{}", p.id);
        } else {
            let region = pool::REGIONS.iter().find(|r| r.0 == p.region).unwrap();
            assert_eq!((p.civ.as_str(), p.source.as_str()), (region.3, region.4));
            assert!(base_civs.contains(p.civ.as_str()));
        }
    }
}

#[test]
fn kana_readings_become_latin_names_for_en_and_ru() {
    for (kana, latin) in [
        ("おだ のぶなが", "Oda Nobunaga"),
        ("もうり もとなり", "Mouri Motonari"),
        ("きっちょう", "Kitchou"),
        ("がっこう", "Gakkou"),
        ("マッチ", "Matchi"),
        ("ファーガス", "Faagasu"),
        ("ティム", "Timu"),
        ("ウィリアム", "Wiriamu"),
        ("ジェームズ", "Jeemuzu"),
        ("ジョン・スミス", "Jon Sumisu"),
        ("しゃ　ちゅう", "Sha Chuu"),
    ] {
        assert_eq!(pool::romanize(kana).unwrap(), latin);
    }
    assert!(pool::romanize("漢字").is_err());
    for p in people() {
        let latin = pool::romanize(&p.reading).unwrap();
        assert!(latin.is_ascii() && !latin.is_empty(), "{}", p.id);
    }
}

// 02_AddTriggers.sql fills the UNIQUE Hash columns with the engine's Make_Hash; a rowid
// stands in so the official UNIQUE constraint still holds.
const HASH_TRIGGERS: &str = "
CREATE TRIGGER OnKindInsert AFTER INSERT ON Kinds BEGIN
	UPDATE Kinds SET Hash = New.rowid WHERE Kind = New.Kind; END;
CREATE TRIGGER OnTypeInsert AFTER INSERT ON Types BEGIN
	UPDATE Types SET Hash = New.rowid WHERE Type = New.Type; END;
";
const GAMEPLAY_TABLES: [(&str, &str); 7] = [
    ("Types", "Type"),
    ("Traits", "TraitType"),
    ("Leaders", "LeaderType"),
    ("CivilizationLeaders", "LeaderType"),
    ("LeaderTraits", "LeaderType"),
    ("LoadingInfo", "LeaderType"),
    ("DiplomacyInfo", "Type"),
];

fn gameplay_db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
    for file in [
        "Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql",
        "DLC/Expansion2/Data/Expansion2_Schema.sql",
    ] {
        db.execute_batch(&read(&official(file))).unwrap();
    }
    db.execute_batch(HASH_TRIGGERS).unwrap();
    for file in ["Civilizations.xml", "Leaders.xml"] {
        load_xml(&db, &official(&format!("Base/Assets/Gameplay/Data/{file}")));
    }
    for file in ["JAC_CustomCivs_Gameplay.sql", "JAC_Japan1534_Gameplay.sql"] {
        let path = root().join("poc/JAC_PoC_CustomCivs/Data").join(file);
        db.execute_batch(&read(&path)).unwrap();
    }
    db
}

fn violations(db: &Connection) -> BTreeSet<Vec<String>> {
    let mut out = BTreeSet::new();
    for (table, _) in GAMEPLAY_TABLES {
        out.extend(dump(
            db,
            &format!("PRAGMA foreign_key_check(\"{table}\")"),
            &[],
        ));
    }
    out
}

fn existing_rows(db: &Connection) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for (table, key) in GAMEPLAY_TABLES {
        let sql = format!(
            "SELECT * FROM \"{table}\" WHERE {key} LIKE 'LEADER_JAC_%' \
             AND {key} NOT LIKE 'LEADER_JAC_POOL_%' ORDER BY rowid"
        );
        out.extend(dump(db, &sql, &[]));
    }
    out
}

fn gameplay_with_pool() -> Connection {
    let db = gameplay_db();
    db.execute_batch(&pool::gameplay_sql(&people())).unwrap();
    db
}

#[test]
fn gameplay_rows_satisfy_the_official_schema_and_keep_the_1534_leaders() {
    let people = people();
    let db = gameplay_db();
    let before = violations(&db);
    let existing = existing_rows(&db);
    db.execute_batch(&pool::gameplay_sql(&people)).unwrap();
    let added: Vec<_> = violations(&db).difference(&before).cloned().collect();
    assert!(added.is_empty(), "new foreign key violations: {added:?}");
    assert_eq!(existing_rows(&db), existing);
    let count = |sql: &str, key: &str| dump(&db, sql, &[Sql::Text(key.into())]).len();
    let mut civs = BTreeMap::<String, usize>::new();
    for p in people.iter().filter(|p| !p.existing) {
        for (table, key) in GAMEPLAY_TABLES.iter().filter(|t| t.0 != "Traits") {
            let sql = format!("SELECT * FROM \"{table}\" WHERE {key} = ?1");
            assert_eq!(count(&sql, &p.leader), 1, "{table} {}", p.leader);
        }
        let sql = "SELECT CivilizationType FROM CivilizationLeaders WHERE LeaderType = ?1";
        let civ = dump(&db, sql, &[Sql::Text(p.leader.clone())]);
        assert_eq!(civ[0][0], p.civ);
        *civs.entry(p.civ.clone()).or_default() += 1;
        let sql = "SELECT TraitType FROM LeaderTraits WHERE LeaderType = ?1";
        let traits = dump(&db, sql, &[Sql::Text(p.leader.clone())]);
        assert_eq!(traits[0][0], p.trait_type());
    }
    for p in people.iter().filter(|p| p.existing) {
        // The same person keeps the existing LeaderType; no second leader is generated.
        let sql = "SELECT * FROM Leaders WHERE LeaderType = ?1";
        assert_eq!(count(sql, &format!("LEADER_{}", p.id)), 0);
        assert_eq!(count(sql, &p.leader), 1);
    }
    let fresh = people.iter().filter(|p| !p.existing).count();
    if fresh >= 2 {
        // NO_DUPLICATE_CIVILIZATIONS = 0: different people can share a civilization.
        assert!(civs.values().any(|&n| n >= 2));
    }
    let ledger = dump(
        &db,
        "SELECT Version, Count FROM JAC_BanseikiPoolLedger",
        &[],
    );
    assert_eq!(
        ledger,
        [[pool::ledger_version(&people), people.len().to_string()]]
    );
    let registered = dump(
        &db,
        "SELECT PersonID FROM JAC_BanseikiPool ORDER BY SortIndex",
        &[],
    );
    let ids: Vec<_> = people.iter().map(|p| vec![p.id.clone()]).collect();
    assert_eq!(registered, ids);
    for p in &people {
        let row = dump(&db, "SELECT Name, Reading, Category, Region, Trait FROM JAC_BanseikiPool WHERE PersonID = ?1",
            &[Sql::Text(p.id.clone())]);
        assert_eq!(
            row,
            [vec![
                p.name.clone(),
                p.reading.clone(),
                p.category.clone(),
                p.region.clone(),
                p.trait_ja.clone()
            ]]
        );
    }
}

// Verbatim from Base/Assets/Configuration/Data/Schema/SetupParameters.sql and
// AdditionalTables.sql; compared with the installed game when it is present.
const CONFIG_SCHEMA: [&str; 6] = [
    "CREATE TABLE 'Queries' (
	'QueryId' TEXT NOT NULL,
	'SQL' TEXT NOT NULL,
 	PRIMARY KEY('QueryId')
);",
    "CREATE TABLE 'QueryParameters'(
	'QueryId' TEXT NOT NULL,
	'Index' INTEGER NOT NULL,
	'ConfigurationGroup' TEXT NOT NULL,
	'ConfigurationId' TEXT NOT NULL,
	PRIMARY KEY('QueryId', 'Index'),
	FOREIGN KEY('QueryId') REFERENCES 'Queries'('QueryId')
);",
    "CREATE TABLE 'DomainValueFilterQueries'(
	'QueryId' TEXT NOT NULL,
	'DomainField' TEXT NOT NULL DEFAULT 'Domain',
	'ValueField' TEXT NOT NULL DEFAULT 'Value',
	'FilterField' TEXT NOT NULL DEFAULT 'Filter',
	FOREIGN KEY('QueryId') REFERENCES 'Queries'('QueryId')
);",
    "CREATE TABLE 'MapLeaders' (
	'Map' TEXT NOT NULL,		-- A reference to Maps::File
	'LeaderType' TEXT NOT NULL,	-- A leader type (ignoring domain)
	PRIMARY KEY ('Map', 'LeaderType')
);",
    "CREATE TABLE 'Players' (
	'Domain' TEXT DEFAULT 'Players:StandardPlayers',
	'CivilizationType' TEXT NOT NULL,
	'LeaderType' TEXT NOT NULL,
	'LeaderName' TEXT NOT NULL,
	'LeaderIcon' TEXT NOT NULL,
	'CivilizationName' TEXT NOT NULL,
	'CivilizationIcon' TEXT NOT NULL,
	'LeaderAbilityName' TEXT NOT NULL,
	'LeaderAbilityDescription' TEXT NOT NULL,
	'LeaderAbilityIcon' TEXT NOT NULL,
	'CivilizationAbilityName' TEXT NOT NULL,
	'CivilizationAbilityDescription' TEXT NOT NULL,
	'CivilizationAbilityIcon' TEXT NOT NULL,
	'Portrait' TEXT,
	'PortraitBackground' TEXT,
	'PlayerColor' TEXT,
	'HumanPlayable' BOOLEAN NOT NULL DEFAULT 1,
	'SortIndex' INTEGER,
	PRIMARY KEY('Domain', 'CivilizationType', 'LeaderType')
);",
    "CREATE TABLE 'DuplicateLeaders' (
	'Domain' TEXT DEFAULT 'Players:StandardPlayers',
	'LeaderType' TEXT NOT NULL,
	'OtherLeaderType' TEXT NOT NULL
);",
];
// Base/Assets/Configuration/Data/SetupParameters.xml:161,248,281.
const MAP_LEADERS_QUERY: &str = "SELECT 'Players:*' AS Domain, LeaderType as Value, \
    'intersect' as Filter FROM MapLeaders WHERE Map = ?1";
const AI_ONLY: [(&str, &str); 4] = [
    ("CIVILIZATION_EGYPT", "LEADER_CLEOPATRA"),
    ("CIVILIZATION_GREECE", "LEADER_GORGO"),
    ("CIVILIZATION_KOREA", "LEADER_SEJONG"),
    ("CIVILIZATION_BABYLON_STK", "LEADER_HAMMURABI"),
];

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn embedded_configuration_schema_matches_the_installed_game() {
    let dir = game_dir().join("Base/Assets/Configuration/Data/Schema");
    if !dir.is_dir() {
        return; // CI has no game install; the copies above are then the reference.
    }
    let installed = squash(
        &(read(&dir.join("SetupParameters.sql")) + &read(&dir.join("AdditionalTables.sql"))),
    );
    for statement in CONFIG_SCHEMA {
        assert!(installed.contains(&squash(statement)), "{statement}");
    }
    let setup = read(&game_dir().join("Base/Assets/Configuration/Data/SetupParameters.xml"));
    assert!(setup.contains(MAP_LEADERS_QUERY));
}

fn official_players() -> Vec<Row> {
    let installed = game_dir().join("DLC/Expansion2/Config/Expansion2_Players.xml");
    if installed.is_file() {
        return xml_rows(&parse(&installed), "Players");
    }
    // Without the game: one row per source leader and the AI-only examples, shaped like
    // the official Players rows (the civilization fields are what the pool copies).
    let sources = pool::REGIONS.iter().map(|r| (r.3, r.4)).chain(AI_ONLY);
    let mut rows = Vec::new();
    for (civ, leader) in sources.collect::<BTreeSet<_>>() {
        let mut row = Row::new();
        row.insert("Domain".into(), DOMAIN.into());
        row.insert("CivilizationType".into(), civ.into());
        row.insert("LeaderType".into(), leader.into());
        for field in [
            "LeaderName",
            "LeaderAbilityName",
            "LeaderAbilityDescription",
        ] {
            row.insert(field.into(), format!("LOC_{leader}_{field}"));
        }
        for field in ["CivilizationName", "CivilizationAbilityName"] {
            row.insert(field.into(), format!("LOC_{civ}_{field}"));
        }
        row.insert(
            "CivilizationAbilityDescription".into(),
            format!("LOC_{civ}_TRAIT"),
        );
        for field in ["LeaderIcon", "LeaderAbilityIcon"] {
            row.insert(field.into(), format!("ICON_{leader}"));
        }
        for field in ["CivilizationIcon", "CivilizationAbilityIcon"] {
            row.insert(field.into(), format!("ICON_{civ}"));
        }
        rows.push(row);
    }
    rows
}

/// The configuration database before this MOD: official rows, CustomCivs and Banseiki.
fn config_db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
    for statement in CONFIG_SCHEMA {
        db.execute_batch(statement).unwrap();
    }
    db.execute(
        "INSERT INTO Queries (QueryId, SQL) VALUES ('MapLeaders', ?1)",
        [MAP_LEADERS_QUERY],
    )
    .unwrap();
    db.execute_batch(
        "INSERT INTO QueryParameters VALUES ('MapLeaders', 1, 'Map', 'MAP_SCRIPT');
         INSERT INTO DomainValueFilterQueries (QueryId) VALUES ('MapLeaders');",
    )
    .unwrap();
    insert(&db, "Players", &official_players());
    let custom = root().join("poc/JAC_PoC_CustomCivs/Config");
    for file in ["JAC_CustomCivs_Players.xml", "JAC_Japan1534_Players.xml"] {
        insert(
            &db,
            "Players",
            &xml_rows(&parse(&custom.join(file)), "Players"),
        );
    }
    let japan =
        parse(&root().join("poc/JAC_PoC_JapanMapScale/Config/JAC_JapanMapScale_Config.xml"));
    insert(&db, "MapLeaders", &xml_rows(&japan, "MapLeaders"));
    let banseiki = parse(&banseiki_config());
    for table in ["Queries", "QueryParameters", "DomainValueFilterQueries"] {
        insert(&db, table, &xml_rows(&banseiki, table));
    }
    db
}

fn banseiki_config() -> PathBuf {
    root().join("poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml")
}

/// Leader values of a player's PlayerLeader parameter after the domain filter queries,
/// as SetupParameters.lua:1099-1224 applies them (intersect, then difference).
fn choices(db: &Connection, ruleset: &str, map: &str, player: i64, pool: &str) -> BTreeSet<String> {
    let mut leaders = strings(
        db,
        "SELECT LeaderType FROM Players WHERE Domain = ?1",
        &[Sql::Text(DOMAIN.into())],
    );
    let mut intersect: Option<BTreeSet<String>> = None;
    let mut difference = BTreeSet::new();
    let filters = "SELECT q.QueryId, q.SQL FROM DomainValueFilterQueries f \
                   JOIN Queries q ON q.QueryId = f.QueryId";
    for query in dump(db, filters, &[]) {
        let sql = "SELECT ConfigurationGroup, ConfigurationId FROM QueryParameters \
                   WHERE QueryId = ?1 ORDER BY \"Index\"";
        let args: Vec<Sql> = dump(db, sql, &[Sql::Text(query[0].clone())])
            .into_iter()
            .map(|p| match (p[0].as_str(), p[1].as_str()) {
                ("Game", "RULESET") => Sql::Text(ruleset.into()),
                ("Player", "PLAYER_ID") => Sql::Integer(player),
                ("Game", "JAC_BANSEIKI_HUMAN_POOL") => Sql::Text(pool.into()),
                ("Map", "MAP_SCRIPT") => Sql::Text(map.into()),
                other => panic!("unexpected query parameter {other:?}"),
            })
            .collect();
        for row in dump(db, &query[1], &args) {
            if row[0] != DOMAIN && row[0] != "Players:*" {
                continue;
            }
            match row[2].as_str() {
                "intersect" => {
                    intersect.get_or_insert_default().insert(row[1].clone());
                }
                "difference" => {
                    difference.insert(row[1].clone());
                }
                other => panic!("unexpected filter {other}"),
            }
        }
    }
    if let Some(keep) = intersect {
        leaders.retain(|l| keep.contains(l));
    }
    leaders.retain(|l| !difference.contains(l));
    leaders
}

fn pool_leaders() -> BTreeSet<String> {
    people().into_iter().map(|p| p.leader).collect()
}

fn config_with_pool() -> Connection {
    let db = config_db();
    db.execute_batch(&pool::config_sql(&people())).unwrap();
    db
}

#[test]
fn setup_rows_extend_only_banseiki_and_keep_gs_and_the_fixed_japan_maps() {
    let people = people();
    let pool_set = pool_leaders();
    let fresh: BTreeSet<_> = people
        .iter()
        .filter(|p| !p.existing)
        .map(|p| p.leader.clone())
        .collect();
    let db = config_db();
    let gs_before = choices(&db, GS, CONTINENTS, 0, "JAC_BANSEIKI_ONLY");
    let japan_before = choices(&db, BANSEIKI, JAPAN_MAP, 0, "JAC_BANSEIKI_ONLY");
    // Before this MOD the slot-0 SQL filter left two leaders, whatever the player was.
    let slot0 = choices(&db, BANSEIKI, CONTINENTS, 0, "JAC_BANSEIKI_ONLY");
    assert_eq!(slot0.len(), 2);
    db.execute_batch(&pool::config_sql(&people)).unwrap();
    let filters = strings(&db, "SELECT QueryId FROM DomainValueFilterQueries", &[]);
    assert!(!filters.contains("JAC_Banseiki_HumanLeaderFilter"));
    // Standard Gathering Storm and the fixed Japan maps are unchanged.
    for player in [0, 1, 5] {
        assert_eq!(
            choices(&db, GS, CONTINENTS, player, "JAC_BANSEIKI_ONLY"),
            gs_before
        );
        assert_eq!(
            choices(&db, GS, CONTINENTS, player, "JAC_ALL_POOLS"),
            gs_before
        );
        let japan = choices(&db, BANSEIKI, JAPAN_MAP, player, "JAC_BANSEIKI_ONLY");
        assert_eq!(japan, japan_before);
        assert_eq!(japan.len(), 22);
        assert!(japan.is_disjoint(&fresh));
    }
    assert!(gs_before.is_disjoint(&fresh));
    // Banseiki: every slot sees the whole pool plus every supported AI leader; the human
    // restriction is applied by UI/JAC_PoolSetup.lua, not by slot number.
    let all = choices(&db, BANSEIKI, CONTINENTS, 0, "JAC_BANSEIKI_ONLY");
    assert!(all.is_superset(&pool_set));
    assert!(all.contains("LEADER_CLEOPATRA") && !all.contains("LEADER_SEJONG"));
    for player in [1, 2, 7] {
        for setting in ["JAC_BANSEIKI_ONLY", "JAC_ALL_POOLS"] {
            assert_eq!(choices(&db, BANSEIKI, CONTINENTS, player, setting), all);
        }
    }
    // One Players row per new leader, with the source leader's civilization fields.
    let sql = "SELECT p.CivilizationType, p.LeaderName, p.CivilizationName, b.CivilizationName \
               FROM Players p JOIN JAC_BanseikiPool j ON j.LeaderType = p.LeaderType \
               JOIN Players b ON b.Domain = p.Domain AND b.LeaderType = j.SourceLeaderType \
               WHERE p.Domain = ?1 AND p.LeaderType = ?2";
    for p in people.iter().filter(|p| !p.existing) {
        let args = [Sql::Text(DOMAIN.into()), Sql::Text(p.leader.clone())];
        let rows = dump(&db, sql, &args);
        assert_eq!(rows.len(), 1, "{}", p.leader);
        assert_eq!(rows[0][0], p.civ);
        assert_eq!(rows[0][1], format!("LOC_{}_NAME", p.leader));
        assert_eq!(rows[0][2], rows[0][3]);
    }
    for p in people.iter().filter(|p| p.existing) {
        let sql = "SELECT * FROM Players WHERE LeaderType = ?1";
        assert!(dump(&db, sql, &[Sql::Text(format!("LEADER_{}", p.id))]).is_empty());
        assert_eq!(dump(&db, sql, &[Sql::Text(p.leader.clone())]).len(), 1);
    }
    let aliases = dump(&db, "SELECT * FROM DuplicateLeaders", &[]);
    assert!(
        aliases.is_empty(),
        "one LeaderType per person needs no alias rows"
    );
    // SSOT 2.2 duplicate policy, kept from the Banseiki ruleset.
    let updates = xml_rows(&parse(&banseiki_config()), "ConfigurationUpdates");
    let flag = |id: &str| {
        let row = updates.iter().find(|r| r["TargetId"] == id).unwrap();
        row["TargetValue"].clone()
    };
    assert_eq!(flag("NO_DUPLICATE_LEADERS"), "1");
    assert_eq!(flag("NO_DUPLICATE_CIVILIZATIONS"), "0");
}

const SETUP_PRELUDE: &str = r#"
CONFIG = { game = {}, map = {}, players = {}, ids = {}, hotseat = false, mp = false }
GameConfiguration = {
	GetValue = function(key) return CONFIG.game[key] end,
	IsHotseat = function() return CONFIG.hotseat end,
	IsAnyMultiplayer = function() return CONFIG.mp end,
	GetParticipatingPlayerIDs = function() return CONFIG.ids end,
}
MapConfiguration = { GetValue = function(key) return CONFIG.map[key] end }
PlayerConfigurations = setmetatable({}, { __index = function(_, id)
	local p = CONFIG.players[id]
	if p == nil then return nil end
	return {
		IsHuman = function() return p.human end,
		GetLeaderTypeName = function() return p.leader end,
        GetCivilizationTypeName = function() return "CIVILIZATION_JAPAN" end,
        GetSlotStatus = function() return p.human and SlotStatus.SS_TAKEN or SlotStatus.SS_COMPUTER end,
	}
end })
GameStateTypes = { GAMESTATE_PREGAME = 0 }
SlotStatus = { SS_COMPUTER = 1, SS_TAKEN = 2 }
GameConfiguration.GetGameState = function() return 0 end
Network = { IsGameHost = function() return true end, GetLocalPlayerID = function() return 0 end }
Modding = { IsLeaderAllowed = function() return true end }
print = function() end
function RUN(id, values)
	return SetupParameters.Parameter_FilterValues({ PlayerId = id }, { ParameterId = "PlayerLeader" }, values)
end
"#;

fn setup_lua(db: Connection) -> Lua {
    let lua = Lua::new();
    let query = lua
        .create_function(move |lua, (sql, args): (String, Variadic<LuaValue>)| {
            let out = lua.create_table()?;
            let mut statement = db.prepare(&sql).map_err(mlua::Error::external)?;
            let names: Vec<String> = statement
                .column_names()
                .iter()
                .map(|n| n.to_string())
                .collect();
            let bound: Vec<Sql> = args
                .iter()
                .take(statement.parameter_count())
                .map(|v| match v {
                    LuaValue::Nil => Sql::Null,
                    LuaValue::String(s) => Sql::Text(s.to_str().unwrap().to_string()),
                    LuaValue::Integer(n) => Sql::Integer(*n),
                    LuaValue::Boolean(b) => Sql::Integer(i64::from(*b)),
                    other => panic!("unexpected SQL argument {other:?}"),
                })
                .collect();
            let mut rows = statement
                .query(params_from_iter(bound))
                .map_err(mlua::Error::external)?;
            let mut index = 0;
            while let Some(row) = rows.next().map_err(mlua::Error::external)? {
                let entry = lua.create_table()?;
                for (i, name) in names.iter().enumerate() {
                    entry.set(
                        name.as_str(),
                        text(row.get_ref(i).map_err(mlua::Error::external)?),
                    )?;
                }
                index += 1;
                out.set(index, entry)?;
            }
            Ok(out)
        })
        .unwrap();
    lua.load(SETUP_PRELUDE)
        .set_name("setup prelude")
        .exec()
        .unwrap();
    let db_api = lua.create_table().unwrap();
    db_api.set("ConfigurationQuery", query).unwrap();
    db_api
        .set(
            "ConfigurationChanges",
            lua.create_function(|_, ()| Ok(0)).unwrap(),
        )
        .unwrap();
    lua.globals().set("DB", db_api).unwrap();
    let include = lua
        .create_function(|lua, name: String| {
            let source = read(&mod_dir().join(format!("UI/{name}.lua")));
            lua.load(&source).set_name(name).exec()
        })
        .unwrap();
    lua.globals().set("include", include).unwrap();
    lua.load(read(&mod_dir().join("UI/SetupParameters.lua")))
        .exec()
        .unwrap();
    lua
}

struct Setup<'a> {
    ruleset: &'a str,
    pool: &'a str,
    map: &'a str,
    hotseat: bool,
    mp: bool,
    players: &'a [(i64, bool, Option<&'a str>)],
}

fn configure(lua: &Lua, setup: &Setup<'_>) {
    let mut code = format!(
        "CONFIG.game = {{ RULESET = {:?}, JAC_BANSEIKI_HUMAN_POOL = {:?} }}\n\
         CONFIG.map = {{ MAP_SCRIPT = {:?} }}\n\
         CONFIG.game.NO_DUPLICATE_LEADERS = true\nCONFIG.game.NO_DUPLICATE_CIVILIZATIONS = false\nCONFIG.hotseat = {}\nCONFIG.mp = {}\nCONFIG.players = {{}}\nCONFIG.ids = {{}}\n",
        setup.ruleset, setup.pool, setup.map, setup.hotseat, setup.mp
    );
    for (id, human, leader) in setup.players {
        let leader = leader.map_or("nil".to_owned(), |l| format!("{l:?}"));
        code.push_str(&format!(
            "CONFIG.players[{id}] = {{ human = {human}, leader = {leader} }}\n\
             table.insert(CONFIG.ids, {id})\n"
        ));
    }
    lua.load(&code).exec().unwrap();
}

fn run(lua: &Lua, player: i64, values: &[(String, String)]) -> Vec<(String, bool)> {
    let input = lua.create_table().unwrap();
    for (i, (domain, value)) in values.iter().enumerate() {
        let entry = lua.create_table().unwrap();
        entry.set("Domain", domain.as_str()).unwrap();
        entry.set("Value", value.as_str()).unwrap();
        input.set(i + 1, entry).unwrap();
    }
    let f: Function = lua.globals().get("RUN").unwrap();
    let out: Table = f.call((player, input)).unwrap();
    out.sequence_values::<Table>()
        .map(|entry| {
            let entry = entry.unwrap();
            let invalid = entry.get::<Option<bool>>("Invalid").unwrap();
            (
                entry.get::<String>("Value").unwrap(),
                invalid.unwrap_or(false),
            )
        })
        .collect()
}

fn values_of(result: &[(String, bool)]) -> BTreeSet<String> {
    result.iter().map(|(v, _)| v.clone()).collect()
}

#[test]
fn every_human_chooses_from_the_pool_and_ai_keeps_every_supported_leader() {
    let people = people();
    let db = config_with_pool();
    let leaders = choices(&db, BANSEIKI, CONTINENTS, 0, "JAC_BANSEIKI_ONLY");
    let mut values: Vec<(String, String)> = ["RANDOM", "RANDOM_POOL1", "RANDOM_POOL2"]
        .into_iter()
        .map(|v| ("Players:*".to_owned(), v.to_owned()))
        .collect();
    values.extend(leaders.iter().map(|l| (DOMAIN.to_owned(), l.clone())));
    let mut expected = pool_leaders();
    expected.insert("RANDOM".into());
    let lua = setup_lua(db);
    // An existing 1534 leader in the pool is the alias case: one LeaderType per person.
    let held = people
        .iter()
        .find(|p| p.existing)
        .unwrap_or(&people[0])
        .leader
        .clone();
    let players = [
        (0, true, None),
        (1, true, Some(held.as_str())),
        (2, false, Some("LEADER_CLEOPATRA")),
        (3, false, None),
    ];
    let everyone: BTreeSet<String> = values.iter().map(|(_, v)| v.clone()).collect();
    for (hotseat, mp) in [(true, true), (false, true)] {
        let setup = Setup {
            ruleset: BANSEIKI,
            pool: "JAC_BANSEIKI_ONLY",
            map: CONTINENTS,
            hotseat,
            mp,
            players: &players,
        };
        configure(&lua, &setup);
        let first = run(&lua, 0, &values);
        let second = run(&lua, 1, &values);
        // Two humans are restricted alike, whatever their slots.
        assert_eq!(values_of(&first), expected);
        assert_eq!(values_of(&second), expected);
        // The native duplicate check still applies inside the pool.
        assert!(first.contains(&(held.clone(), true)));
        assert!(second.contains(&(held.clone(), false)));
        // AI slots keep every supported leader, the pool included.
        for ai in [2, 3] {
            let result = run(&lua, ai, &values);
            assert_eq!(values_of(&result), everyone);
            assert!(result.contains(&(held.clone(), true)));
        }
    }
    // A single human in single player is restricted too.
    let single = [(0, true, None), (1, false, None)];
    let setup = Setup {
        ruleset: BANSEIKI,
        pool: "JAC_BANSEIKI_ONLY",
        map: CONTINENTS,
        hotseat: false,
        mp: false,
        players: &single,
    };
    configure(&lua, &setup);
    assert_eq!(values_of(&run(&lua, 0, &values)), expected);
    assert_eq!(values_of(&run(&lua, 1, &values)), everyone);
    // Japan SP preserves the map list, while both hotseat and LAN use the human pool.
    for mp in [false, true] {
        let setup = Setup {
            ruleset: BANSEIKI,
            pool: "JAC_BANSEIKI_ONLY",
            map: JAPAN_MAP,
            hotseat: mp,
            mp,
            players: &players,
        };
        configure(&lua, &setup);
        assert_eq!(
            values_of(&run(&lua, 0, &values)),
            if mp {
                expected.clone()
            } else {
                everyone.clone()
            }
        );
    }
    // All pools and standard Gathering Storm are not restricted.
    for (ruleset, pool_setting, map) in [
        (BANSEIKI, "JAC_ALL_POOLS", CONTINENTS),
        (GS, "JAC_BANSEIKI_ONLY", CONTINENTS),
    ] {
        let setup = Setup {
            ruleset,
            pool: pool_setting,
            map,
            hotseat: true,
            mp: true,
            players: &players,
        };
        configure(&lua, &setup);
        assert_eq!(
            values_of(&run(&lua, 0, &values)),
            everyone,
            "{ruleset} {pool_setting} {map}"
        );
    }
}

#[test]
fn fixed_japan_map_query_is_retained_for_sp_and_ai_but_bypassed_for_mp_humans() {
    let lua = setup_lua(config_with_pool());
    let players = [(0, true, None), (1, true, None), (2, false, None)];
    let query = lua.create_table().unwrap();
    query.set("QueryId", "MapLeaders").unwrap();
    query
        .set("SQL", "SELECT LeaderType FROM MapLeaders WHERE Map = ?1")
        .unwrap();
    let params = lua.create_table().unwrap();
    let map = lua.create_table().unwrap();
    map.set("ConfigurationGroup", "Map").unwrap();
    map.set("ConfigurationId", "MAP_SCRIPT").unwrap();
    params.set(1, map).unwrap();
    query.set("Parameters", params).unwrap();
    let module: Table = lua.globals().get("SetupParameters").unwrap();
    let data_query: Function = module.get("Data_Query").unwrap();
    for mp in [false, true] {
        let setup = Setup {
            ruleset: BANSEIKI,
            pool: "JAC_BANSEIKI_ONLY",
            map: JAPAN_MAP,
            hotseat: mp,
            mp,
            players: &players,
        };
        configure(&lua, &setup);
        for id in [0, 1, 2] {
            let instance = lua.create_table().unwrap();
            instance.set("PlayerId", id).unwrap();
            instance
                .set(
                    "Config_Read",
                    module.get::<Function>("Config_Read").unwrap(),
                )
                .unwrap();
            let rows: Table = data_query.call((instance, query.clone())).unwrap();
            assert_eq!(rows.raw_len(), if mp && id != 2 { 0 } else { 22 });
        }
    }
}

#[test]
fn hotseat_scenario_uses_two_distinct_people_in_their_actual_shared_civilization() {
    let scenario =
        read_json(&root().join("tools/civ6-autotest/scenarios/banseiki-pool-hotseat.json"))
            .unwrap();
    let configure = scenario["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["action"] == "configure")
        .unwrap();
    let configured = configure["players"].as_array().unwrap();
    assert_eq!(configured.len(), 2);
    let people = people();
    let mut ids = BTreeSet::new();
    let mut civs = BTreeSet::new();
    for player in configured {
        let p = people
            .iter()
            .find(|p| player["leader"] == p.leader)
            .unwrap();
        assert_eq!(player["civ"], p.civ);
        assert_eq!(player["human"], true);
        ids.insert(&p.id);
        civs.insert(&p.civ);
    }
    assert_eq!(ids.len(), 2);
    assert_eq!(civs.len(), 1);
    let version = pool::ledger_version(&people);
    let mut changed = people.clone();
    changed[0].civ = "different".into();
    assert_ne!(pool::ledger_version(&changed), version);
    let mut changed = people.clone();
    changed[0].trait_key = "different".into();
    assert_ne!(pool::ledger_version(&changed), version);
}

const GAMEPLAY_PRELUDE: &str = r#"
SETS = 0
LOG = {}
print = function(message) table.insert(LOG, message) end
local function holder(store)
	return {
		GetProperty = function(_, key) return store[key] end,
		SetProperty = function(_, key, value) store[key] = value; SETS = SETS + 1 end,
	}
end
Game = holder(STATE.game)
local function rows(list)
	return function()
		local i = 0
		return function() i = i + 1; return list[i] end
	end
end
GameInfo = { JAC_BanseikiPool = rows(POOL), JAC_BanseikiPoolLedger = rows(LEDGER_ROWS) }
local majors = {}
PlayerConfigurations = {}
for _, id in ipairs(ORDER) do
	local leader = LEADERS[id]
	STATE.players[id] = STATE.players[id] or {}
	local player = holder(STATE.players[id])
	player.GetID = function() return id end
	table.insert(majors, player)
	PlayerConfigurations[id] = { GetLeaderTypeName = function() return leader end }
end
PlayerManager = { GetAliveMajors = function() return majors end }
Events = { TurnBegin = { Add = function(f) TURN_BEGIN = f end } }
"#;

#[derive(Clone, Default, Debug, PartialEq)]
struct Saved {
    game: BTreeMap<String, String>,
    players: BTreeMap<i64, BTreeMap<String, String>>,
}

fn session(saved: &Saved, rows: &[[String; 3]], version: &str, leaders: &[(i64, &str)]) -> Lua {
    let lua = Lua::new();
    let g = lua.globals();
    let state = lua.create_table().unwrap();
    let game = lua.create_table().unwrap();
    for (k, v) in &saved.game {
        game.set(k.as_str(), v.as_str()).unwrap();
    }
    let players = lua.create_table().unwrap();
    for (id, props) in &saved.players {
        let t = lua.create_table().unwrap();
        for (k, v) in props {
            t.set(k.as_str(), v.as_str()).unwrap();
        }
        players.set(*id, t).unwrap();
    }
    state.set("game", game).unwrap();
    state.set("players", players).unwrap();
    g.set("STATE", state).unwrap();
    let pool_rows = lua.create_table().unwrap();
    for (i, [leader, person, trait_type]) in rows.iter().enumerate() {
        let row = lua.create_table().unwrap();
        row.set("LeaderType", leader.as_str()).unwrap();
        row.set("PersonID", person.as_str()).unwrap();
        row.set("TraitType", trait_type.as_str()).unwrap();
        pool_rows.set(i + 1, row).unwrap();
    }
    g.set("POOL", pool_rows).unwrap();
    let ledger = lua.create_table().unwrap();
    let entry = lua.create_table().unwrap();
    entry.set("Version", version).unwrap();
    ledger.set(1, entry).unwrap();
    g.set("LEDGER_ROWS", ledger).unwrap();
    let by_id = lua.create_table().unwrap();
    let order = lua.create_table().unwrap();
    for (i, (id, leader)) in leaders.iter().enumerate() {
        by_id.set(*id, *leader).unwrap();
        order.set(i + 1, *id).unwrap();
    }
    g.set("LEADERS", by_id).unwrap();
    g.set("ORDER", order).unwrap();
    lua.load(GAMEPLAY_PRELUDE)
        .set_name("gameplay prelude")
        .exec()
        .unwrap();
    let source = read(&mod_dir().join("Scripts/JAC_BanseikiPool.lua"));
    lua.load(source.as_str())
        .set_name("JAC_BanseikiPool.lua")
        .exec()
        .unwrap();
    lua
}

fn saved(lua: &Lua) -> Saved {
    let state: Table = lua.globals().get("STATE").unwrap();
    let game: Table = state.get("game").unwrap();
    let players: Table = state.get("players").unwrap();
    let mut out = Saved {
        game: game.pairs::<String, String>().map(Result::unwrap).collect(),
        ..Saved::default()
    };
    for pair in players.pairs::<i64, Table>() {
        let (id, props) = pair.unwrap();
        let props = props
            .pairs::<String, String>()
            .map(Result::unwrap)
            .collect();
        out.players.insert(id, props);
    }
    out
}

fn sets(lua: &Lua) -> i64 {
    lua.globals().get("SETS").unwrap()
}

fn turn(lua: &Lua) {
    let f: Function = lua.globals().get("TURN_BEGIN").unwrap();
    f.call::<()>(()).unwrap();
}

fn log(lua: &Lua) -> Vec<String> {
    let log: Table = lua.globals().get("LOG").unwrap();
    log.sequence_values::<String>()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn gameplay_ledger_is_recorded_once_and_restored_after_load() {
    let people = people();
    let db = gameplay_with_pool();
    let sql = "SELECT LeaderType, PersonID, TraitType FROM JAC_BanseikiPool ORDER BY SortIndex";
    let rows: Vec<[String; 3]> = dump(&db, sql, &[])
        .into_iter()
        .map(|r| [r[0].clone(), r[1].clone(), r[2].clone()])
        .collect();
    let version = dump(&db, "SELECT Version FROM JAC_BanseikiPoolLedger", &[])[0][0].clone();
    assert_eq!(version, pool::ledger_version(&people));
    let first = &people[0];
    let alias = people
        .iter()
        .find(|p| p.existing)
        .unwrap_or(&people[people.len() - 1]);
    let leaders = [
        (0, first.leader.as_str()),
        (1, alias.leader.as_str()),
        (2, "LEADER_CLEOPATRA"),
    ];
    // New game: the ledger version and every pool player's person are recorded once.
    let lua = session(&Saved::default(), &rows, &version, &leaders);
    let state = saved(&lua);
    assert_eq!(state.game["JAC_BANSEIKI_POOL_LEDGER"], version);
    for (id, p) in [(0, first), (1, alias)] {
        let props = &state.players[&id];
        assert_eq!(props["JAC_POOL_PERSON_ID"], p.id);
        assert_eq!(props["JAC_POOL_TRAIT"], p.trait_type());
        assert_eq!(props["JAC_POOL_ROLE"], "LEADER");
    }
    assert!(state.players[&2].is_empty());
    assert_eq!(sets(&lua), 7);
    turn(&lua);
    assert_eq!(sets(&lua), 7);
    // Load: the same IDs are read back and nothing is written again.
    let lua = session(&state, &rows, &version, &leaders);
    turn(&lua);
    assert_eq!(sets(&lua), 0);
    assert_eq!(saved(&lua), state);
    assert!(log(&lua).iter().any(|l| l.contains("result=restored")));
    // Load with a changed ledger: the saved IDs win and the mismatch is logged.
    let mut changed = rows.clone();
    changed[0][1] = "JAC_POOL_O999".into();
    let lua = session(&state, &changed, "changed", &leaders);
    turn(&lua);
    assert_eq!(sets(&lua), 0);
    assert_eq!(saved(&lua), state);
    assert!(log(&lua)
        .iter()
        .any(|l| l.contains("result=kept_saved_ids")));
    // A player without a record (a later join) is filled in; the saved ones are untouched.
    if let Some(late) = people
        .iter()
        .find(|p| p.leader != first.leader && p.leader != alias.leader)
    {
        let joined = [
            leaders[0],
            leaders[1],
            leaders[2],
            (3, late.leader.as_str()),
        ];
        let lua = session(&state, &rows, &version, &joined);
        assert_eq!(sets(&lua), 3);
        turn(&lua);
        assert_eq!(sets(&lua), 3);
        let after = saved(&lua);
        assert_eq!(after.players[&3]["JAC_POOL_PERSON_ID"], late.id);
        assert_eq!(after.players[&0], state.players[&0]);
    }
}

#[test]
fn mod_lists_every_file_and_imports_the_shared_frontend_module() {
    let modinfo = parse(&mod_dir().join("JAC_PoC_BanseikiPool.modinfo"));
    assert_eq!(
        modinfo.attributes["id"],
        "c8fb8042-8e3e-4b35-b02a-781947194700"
    );
    let dependencies: BTreeSet<_> = elements(modinfo.get_child("Dependencies").unwrap())
        .map(|e| e.attributes["id"].clone())
        .collect();
    for id in [
        "ba3a1c58-9287-4654-a22f-85f9e6841e9b", // CustomCivs
        "bd7a82c5-6b38-4b69-a7f5-8dc9af534249", // BanseikiRuleset
        "4873eb62-8ccc-4574-b784-dda455e74e68", // Gathering Storm
    ] {
        assert!(dependencies.contains(id), "{id}");
    }
    let files: BTreeSet<String> = elements(modinfo.get_child("Files").unwrap())
        .map(|e| e.get_text().unwrap().into_owned())
        .collect();
    for file in &files {
        assert!(mod_dir().join(file).is_file(), "{file}");
    }
    let mut used = BTreeSet::new();
    let frontend = modinfo.get_child("FrontEndActions").unwrap();
    let mut imported = BTreeSet::new();
    for context in ["InGameActions", "FrontEndActions"] {
        for action in elements(modinfo.get_child(context).unwrap()) {
            for file in elements(action).filter(|e| e.name == "File") {
                used.insert(file.get_text().unwrap().into_owned());
            }
        }
    }
    for action in elements(frontend) {
        assert_ne!(
            action.name, "ReplaceUIScript",
            "frontend uses VFS imports, not an in-game hook"
        );
        if action.name == "ImportFiles" {
            for file in elements(action).filter(|e| e.name == "File") {
                imported.insert(file.get_text().unwrap().into_owned());
            }
        }
    }
    assert!(used.is_subset(&files));
    for file in [
        "UI/SetupParameters.lua",
        "UI/JAC_Stock_SetupParameters.lua",
        "UI/JAC_PoolSetup.lua",
    ] {
        assert!(imported.contains(file), "{file}");
    }
    let ingame = modinfo.get_child("InGameActions").unwrap();
    let ingame_imports: BTreeSet<String> = elements(ingame)
        .filter(|e| e.name == "ImportFiles")
        .flat_map(|e| elements(e).filter(|f| f.name == "File"))
        .map(|e| e.get_text().unwrap().into_owned())
        .collect();
    assert!(
        imported.is_subset(&ingame_imports),
        "Options includes the same module in-game; aliases must remain available"
    );
    let wrapper = read(&mod_dir().join("UI/SetupParameters.lua"));
    assert!(
        wrapper
            .find("include(\"JAC_Stock_SetupParameters\")")
            .unwrap()
            < wrapper.find("include(\"JAC_PoolSetup\")").unwrap()
    );
    let installed = game_dir().join("Base/Assets/UI/Scripts/SetupParameters.lua");
    if installed.is_file() {
        assert_eq!(
            read(&mod_dir().join("UI/JAC_Stock_SetupParameters.lua")),
            read(&installed)
        );
    }
    let dep = read(&mod_dir().join("JAC_PoC_BanseikiPool.dep"));
    for needle in [
        "Leaders.artdef",
        "FallbackLeaders.artdef",
        "LeaderFallbackImages.blp",
    ] {
        assert!(dep.contains(needle), "{needle}");
    }
}

#[test]
fn mod_lua_compiles_as_lua_5_1() {
    let mut count = 0;
    for dir in ["UI", "Scripts"] {
        for entry in fs::read_dir(mod_dir().join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("lua") {
                continue;
            }
            let lua = Lua::new();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let source = read(&path);
            lua.load(source.as_str())
                .set_name(name)
                .into_function()
                .unwrap();
            count += 1;
        }
    }
    assert_eq!(count, 4);
}

#[test]
fn images_and_colours_use_existing_official_keys() {
    let colours = read(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/PlayerStandardColors.xml"),
    );
    for (a, b) in pool::PALETTE {
        for colour in [a, b] {
            assert!(
                colours.contains(&format!("<Type>{colour}</Type>")),
                "{colour}"
            );
        }
    }
    let people = people();
    let icons = pool::icons_xml(&people);
    let fallback = pool::outputs().unwrap();
    let fallback = &fallback
        .iter()
        .find(|(p, _)| p.ends_with("FallbackLeaders.artdef"))
        .unwrap()
        .1;
    for p in people.iter().filter(|p| !p.existing) {
        let icon = format!(
            "<Row Name=\"ICON_{}\" Atlas=\"ICON_ATLAS_LEADERS\" Index=\"0\"/>",
            p.leader
        );
        assert!(icons.contains(&icon));
        assert!(fallback.contains(&format!("<m_Name text=\"{}\"/>", p.leader)));
        // Backgrounds are scenery: the base leader's `_BACKGROUND` or the house's own.
        assert!(p.background.ends_with("_BACKGROUND"), "{}", p.id);
    }
    assert!(
        !fallback.contains("FALLBACK_NEUTRAL"),
        "no person-specific image is reused"
    );
    let base = game_dir().join("Base");
    let leaders_icons = base.join("Assets/UI/Icons/Icons_Leaders.xml");
    if leaders_icons.is_file() {
        let text = read(&leaders_icons);
        assert!(text.contains("Name=\"ICON_LEADER_DEFAULT\""));
        assert!(text.contains("Name=\"ICON_ATLAS_LEADERS\""));
        let artdef = read(&base.join("ArtDefs/FallbackLeaders.artdef"));
        assert!(artdef.contains("<m_EntryName text=\"ART_LEADER_DEFAULT\"/>"));
        assert!(artdef.contains("<m_BLPPackage text=\"LeaderFallbackImages\"/>"));
        assert!(base
            .join("Platforms/Windows/BLPs/LeaderFallbackImages.blp")
            .is_file());
    }
}
