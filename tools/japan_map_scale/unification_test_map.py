"""Build the small test-only map for the live unification-victory scenario.

The castle registry is written by the same generate.insert_historical_state()
as the 1534 Japan maps, so the marker rows come from the map data exactly
like the real maps. The map lives in its own test mod that players do not enable.
"""

from __future__ import annotations

from contextlib import closing
from pathlib import Path
import sqlite3
import uuid

from generate import OFFICIAL, ROOT, SEATS, SEAT_INDEX, create_schema, insert_historical_state


TEST_MOD = ROOT / "poc" / "JAC_Test_UnificationMap"
TEST_MOD_ID = "5c1e7a3d-9b2f-4e61-8d47-2a6f0c9e1b85"
MAP_NAME = "JAC_Unification_Test"
MAP_FILE = f"{{{TEST_MOD_ID}}}Maps/{MAP_NAME}.Civ6Map"
MAP_SIZE = "MAPSIZE_DUEL"
WIDTH, HEIGHT = 32, 20
LAND = {row * WIDTH + col for row in range(2, HEIGHT - 2) for col in range(2, WIDTH - 2)}
# Oda (human) holds two castles, Date holds its capital and one castle (the
# scenario razes that one), Ashina starts with only a settler and no castle.
# Castles are >= 9 tiles apart, beyond the official CITY_MIN_RANGE of 3.
CASTLES = (
    {"owner": "ODA", "col": 6, "row": 5, "capital": True, "name": "", "research_priority": 0},
    {"owner": "DATE", "col": 25, "row": 5, "capital": True, "name": "", "research_priority": 0},
    {"owner": "ODA", "col": 6, "row": 14, "capital": False, "name": "検証城（織田）", "research_priority": 1},
    {"owner": "DATE", "col": 25, "row": 14, "capital": False, "name": "検証城（伊達）", "research_priority": 2},
)
PLAYERS = ("ODA", "DATE", "ASHINA")
STARTS = {"ODA": (6, 5), "DATE": (25, 5), "ASHINA": (16, 10)}


def plot(col: int, row: int) -> int:
    return row * WIDTH + col


def neighbours(index: int) -> set[int]:
    col, row = index % WIDTH, index // WIDTH
    odd = row & 1
    deltas = ((-1, 0), (1, 0), (odd, -1), (odd - 1, -1), (odd, 1), (odd - 1, 1))
    return {plot(col + dx, row + dy) for dx, dy in deltas
            if 0 <= col + dx < WIDTH and 0 <= row + dy < HEIGHT}


def build(destination: Path = TEST_MOD / "Maps" / f"{MAP_NAME}.Civ6Map",
          official: Path = OFFICIAL) -> None:
    if [SEAT_INDEX[slug] for slug in PLAYERS] != list(range(len(PLAYERS))):
        raise ValueError("Test players must be the first seats so map owner IDs match slots")
    cities = [{**city, "plot": plot(city["col"], city["row"])} for city in CASTLES]
    coast = {n for index in LAND for n in neighbours(index)} - LAND
    allocations = []
    for city in cities:
        for index in sorted({city["plot"]} | neighbours(city["plot"])):
            allocations.append((index, SEAT_INDEX[city["owner"]], city["plot"], city["plot"]))
    db = create_schema(official, destination)
    try:
        db.execute("INSERT INTO Map VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                   ("Default", WIDTH, HEIGHT, 47, 23, 0, 0, MAP_SIZE))
        db.executemany("INSERT INTO MapAttributes VALUES (?, ?)",
                       (("MapScript", "Continents.lua"), ("Ruleset", "RULESET_EXPANSION_2")))
        db.executemany("INSERT INTO MetaData VALUES (?, ?)",
                       (("MetaDataVersion", "1"), ("AppVersion", "1.0.12.68"),
                        ("DisplayName", destination.name),
                        ("ID", str(uuid.uuid5(uuid.NAMESPACE_URL, "JAC/UnificationTestMap")).upper()),
                        ("IsMod", "1"), ("IsBasic", "1"), ("LastUpdated", "0")))
        db.executemany("INSERT INTO Plots VALUES (?, ?, ?, ?)",
                       ((index, "TERRAIN_GRASS" if index in LAND else
                         "TERRAIN_COAST" if index in coast else "TERRAIN_OCEAN",
                         "CONTINENT_ASIA" if index in LAND else "", 0)
                        for index in range(WIDTH * HEIGHT)))
        db.executemany("INSERT INTO StartPositions VALUES (?, 'LEADER', ?)",
                       ((plot(*STARTS[slug]), SEATS[SEAT_INDEX[slug]].leader) for slug in PLAYERS))
        db.executemany("INSERT INTO Players VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                       ((SEAT_INDEX[slug], SEATS[SEAT_INDEX[slug]].civ, SEATS[SEAT_INDEX[slug]].leader,
                         "CIVILIZATION_LEVEL_FULL_CIV", "", "Human", "DIFFICULTY_PRINCE", "INVALID",
                         "RANDOM", 1) for slug in PLAYERS))
        insert_historical_state(db, cities, allocations)
        if db.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
            raise ValueError(f"Invalid SQLite map: {destination}")
        db.commit()
        db.execute("VACUUM")
    finally:
        db.close()
    destination.with_suffix(".lua").write_text(
        "-- Static map: starting state is stored in the Civ6Map file.\n"
        "function InitializeNewGame() end\n"
        "LuaEvents.NewGameInitialized.Add(InitializeNewGame);\n", encoding="utf-8")


def castle_rows(path: Path = TEST_MOD / "Maps" / f"{MAP_NAME}.Civ6Map") -> dict[int, int]:
    """Registered castle plot -> map owner ID, read back from the map file."""
    with closing(sqlite3.connect(path)) as db:
        return dict(db.execute(
            "SELECT Cities.Plot, Cities.Owner FROM Cities JOIN Buildings ON Buildings.Plot=Cities.Plot "
            "WHERE Buildings.BuildingType='BUILDING_JAC_REGISTERED_CASTLE'"))


if __name__ == "__main__":
    build()
    print(f"{MAP_NAME}: castles {castle_rows()}")
