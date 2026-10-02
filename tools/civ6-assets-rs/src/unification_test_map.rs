use crate::*;
use generate::{HexGrid, Tiles};
use rusqlite::{params, Connection};
use serde_json::json;
use std::collections::BTreeMap;
pub const TEST_MOD_ID: &str = "5c1e7a3d-9b2f-4e61-8d47-2a6f0c9e1b85";
pub const MAP_NAME: &str = "JAC_Unification_Test";
pub const MAP_SIZE: &str = "MAPSIZE_DUEL";
pub const WIDTH: i32 = 32;
pub const HEIGHT: i32 = 20;
pub const PLAYERS: [&str; 3] = ["ODA", "DATE", "ASHINA"];
pub const STARTS: [(i32, i32); 3] = [(6, 5), (25, 5), (16, 10)];
pub fn plot(col: i32, row: i32) -> i32 {
    row * WIDTH + col
}
pub fn neighbours(p: i32) -> Tiles {
    grid().neighbor_ids(p).into_iter().collect()
}
pub fn grid() -> HexGrid {
    HexGrid {
        km: 20,
        width: WIDTH,
        height: HEIGHT,
        xmin: 0.,
        ymin: 0.,
    }
}
pub fn castles() -> Vec<Value> {
    vec![
        json!({"owner":"ODA","col":6,"row":5,"capital":true,"research_priority":0}),
        json!({"owner":"DATE","col":25,"row":5,"capital":true,"research_priority":0}),
        json!({"owner":"ODA","col":6,"row":14,"capital":false,"name_tag":"LOC_JAC_TEST_UNIFICATION_CASTLE_ODA","research_priority":1}),
        json!({"owner":"DATE","col":25,"row":14,"capital":false,"name_tag":"LOC_JAC_TEST_UNIFICATION_CASTLE_DATE","research_priority":2}),
    ]
}
pub fn build(destination: &Path, official: &Path) -> Result<()> {
    let db = generate::create_schema(official, destination)?;
    build_db(&db, destination)?;
    drop(db);
    write(destination.with_extension("lua"), generate::STATIC_LUA)
}
pub fn build_db(db: &Connection, destination: &Path) -> Result<()> {
    anyhow::ensure!(
        PLAYERS
            .iter()
            .enumerate()
            .all(|(i, p)| constants::seat_index(p) == i as i32),
        "Test players must be first seats"
    );
    let land: Tiles = (2..HEIGHT - 2)
        .flat_map(|r| (2..WIDTH - 2).map(move |c| plot(c, r)))
        .collect();
    let coast: Tiles = land
        .iter()
        .flat_map(|&p| neighbours(p))
        .filter(|p| !land.contains(p))
        .collect();
    let mut cities = castles();
    let mut allocations = vec![];
    for c in &mut cities {
        let p = plot(n(&c["col"]), n(&c["row"]));
        c["plot"] = json!(p);
        let mut tiles = neighbours(p);
        tiles.insert(p);
        allocations.extend(
            tiles
                .into_iter()
                .map(|i| (i, constants::seat_index(s(&c["owner"])), p, p)),
        );
    }
    db.execute_batch("BEGIN IMMEDIATE")?;
    db.execute(
        "INSERT INTO Map VALUES ('Default',?,?,47,23,0,0,?)",
        params![WIDTH, HEIGHT, MAP_SIZE],
    )?;
    generate::write_map_metadata(db, destination, "JAC/UnificationTestMap")?;
    for p in 0..WIDTH * HEIGHT {
        db.execute(
            "INSERT INTO Plots VALUES (?,?,?,0)",
            params![
                p,
                if land.contains(&p) {
                    "TERRAIN_GRASS"
                } else if coast.contains(&p) {
                    "TERRAIN_COAST"
                } else {
                    "TERRAIN_OCEAN"
                },
                if land.contains(&p) {
                    "CONTINENT_ASIA"
                } else {
                    ""
                }
            ],
        )?;
    }
    for (i, xy) in STARTS.iter().enumerate() {
        db.execute(
            "INSERT INTO StartPositions VALUES (?,'LEADER',?)",
            params![plot(xy.0, xy.1), constants::SEATS[i].leader],
        )?;
    }
    generate::insert_players(db, 3)?;
    generate::insert_historical_state(db, &cities, &allocations)?;
    anyhow::ensure!(
        db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))? == "ok",
        "invalid map"
    );
    db.execute_batch("COMMIT;VACUUM;")?;
    Ok(())
}
pub fn castle_rows(path: &Path) -> Result<BTreeMap<i32, i32>> {
    let db = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let rows = db
        .prepare("SELECT Plot,Owner FROM Cities")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}
