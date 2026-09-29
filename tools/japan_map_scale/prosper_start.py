"""Put the 1534 prosperous start (SSOT 3.1, Q28 option 1) into the historical Japan maps.

The rules (tiers, population, districts, units, gold) live in prosperity.py.
This module turns them into Civ6Map rows for one map scale: city population,
city-centre and district buildings, districts, tile improvements, units and
the starting treasury.  Settler-start maps are never touched.

generate.py calls apply() for each historical map.  apply() first removes
everything it owns, so running it again on the same map gives the same rows.

    python tools/japan_map_scale/prosper_start.py     # re-apply to the checked-in maps
"""

from __future__ import annotations

import json
import shutil
import sqlite3
import sys
from collections import Counter, defaultdict
from contextlib import closing
from pathlib import Path
import tempfile

TOOL = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOL))
import generate as g  # noqa: E402
import prosperity as p  # noqa: E402

SCENARIO_DIR = g.ROOT / "tools/civ6-autotest/scenarios"
BASELINE_SCENARIO = SCENARIO_DIR / "japan-scale-baseline-15km.json"
SCENARIO = SCENARIO_DIR / "japan-prosperous-15km.json"

# Official placement rules (Base/Assets/Gameplay/Data/).
DISTRICT_RANGE = 3          # a city places districts within 3 tiles
RESERVE_RANGE = 2           # reserves stand within 2 tiles of a city (research doc 5.2)
NO_ADJACENT_CITY = {"DISTRICT_ENCAMPMENT"}  # Districts.xml:46 NoAdjacentCity="true"
# Owner decision E1=b: no feature (woods, marsh) is ever cleared. When the plots run out,
# fewer districts/improvements are placed instead.
# Harbor: Coast="true" AdjacentToLand="true" (Districts.xml:47).
FARM_TERRAINS = {"TERRAIN_GRASS", "TERRAIN_PLAINS"}  # Improvements.xml:101-102; hill farms need
#                                                    # CIVIC_CIVIL_ENGINEERING (:103-104), so none
MINE_TERRAINS = {"TERRAIN_GRASS_HILLS", "TERRAIN_PLAINS_HILLS"}  # Improvements.xml:105-106
IMPROVEMENT_RESOURCES = {  # Improvement_ValidResources, Improvements.xml:232-249
    "IMPROVEMENT_MINE": {"RESOURCE_COPPER", "RESOURCE_SILVER", "RESOURCE_SALT", "RESOURCE_IRON",
                         "RESOURCE_JADE", "RESOURCE_MERCURY"},
    "IMPROVEMENT_QUARRY": {"RESOURCE_STONE", "RESOURCE_MARBLE", "RESOURCE_GYPSUM"},
    "IMPROVEMENT_FARM": {"RESOURCE_RICE", "RESOURCE_WHEAT"},
}
LAND_UNITS = {"UNIT_WARRIOR", "UNIT_SPEARMAN", "UNIT_ARCHER"}


def improvement_for(terrain: str, feature: str | None, resource: str | None) -> str | None:
    """Improvement buildable under the Sengoku trees (JAC_Sengoku_Unlocks.sql:80) on this plot."""
    if feature:  # a feature that cannot be removed (volcano) takes no improvement
        return None
    if resource:  # a resource plot only takes its own improvement (pasture/plantation are not buildable)
        return next((imp for imp, kinds in IMPROVEMENT_RESOURCES.items() if resource in kinds), None)
    if terrain in FARM_TERRAINS:
        return "IMPROVEMENT_FARM"
    if terrain in MINE_TERRAINS:
        return "IMPROVEMENT_MINE"
    return None


def reset(db: sqlite3.Connection, features: list[tuple] | None) -> None:
    """Remove every row apply() writes, back to the bare historical state.

    features: the generator's PlotFeatures rows (the settler map keeps them), to put
    back features cleared by an older apply(); None when the map is fresh.
    """
    if features is not None:
        db.execute("DELETE FROM PlotFeatures")
        db.executemany("INSERT INTO PlotFeatures VALUES (?, ?)", features)
    db.execute("DELETE FROM Units")
    db.execute("DELETE FROM PlotImprovements")
    db.execute("DELETE FROM Districts WHERE DistrictType<>'DISTRICT_CITY_CENTER'")
    db.execute("DELETE FROM Buildings WHERE BuildingType<>'BUILDING_PALACE'")
    db.execute("DELETE FROM PlayerAttributes WHERE Type='Treasury'")
    db.execute("DELETE FROM PlotOwners WHERE ID IN (SELECT ID FROM Plots WHERE TerrainType IN (?, ?))",
               tuple(p.WATER))
    db.execute("UPDATE CityAttributes SET Value='2' WHERE Type='Population' AND Name='Initial'")


def apply(db: sqlite3.Connection, width: int, height: int, km: int, cities: list[dict],
          features: list[tuple] | None = None) -> dict:
    """Write the prosperous start into an open historical map. Returns a per-faction report."""
    reset(db, features)
    info = p.read_db(db, width, height, km)
    terrain = dict(db.execute("SELECT ID, TerrainType FROM Plots"))
    impassable = {pid for (pid,) in db.execute("SELECT ID FROM Plots WHERE IsImpassable<>0")}
    feature = dict(db.execute("SELECT ID, FeatureType FROM PlotFeatures"))
    resource = dict(db.execute("SELECT ID, ResourceType FROM PlotResources"))
    owner_rows = db.execute("SELECT ID, Owner, CityOwner FROM PlotOwners ORDER BY ID").fetchall()
    plot_owner = {pid: owner for pid, owner, _ in owner_rows}
    city_tiles: dict[int, list[int]] = defaultdict(list)
    for pid, _owner, city in owner_rows:
        city_tiles[city].append(pid)
    centres = {city["plot"] for city in cities}

    def xy(pid: int) -> tuple[int, int]:
        return pid % width, pid // width

    def dist(a: int, b: int) -> int:
        return g.hex_distance(xy(a), xy(b))

    grid = g.HexGrid(km, width, height, 0.0, 0.0)  # only neighbours()/index() are used

    def neighbours(pid: int) -> list[int]:
        return [grid.index(c, r) for c, r in grid.neighbours(*xy(pid))]

    used: set[int] = set(centres)  # plots holding a district or improvement
    occupied: set[int] = set()     # plots holding a military unit
    districts_rows, building_rows, improvement_rows, unit_rows, owner_adds = [], [], [], [], []
    report: dict[str, dict] = {}
    by_owner: dict[str, list[dict]] = defaultdict(list)
    for city in cities:
        by_owner[city["owner"]].append(city)

    for seat in g.SEATS:
        slug, index = seat.slug, g.SEAT_INDEX[seat.slug]
        own = sorted(by_owner[slug], key=lambda c: (not c["capital"], c["research_priority"]))
        plans = []
        for city in own:
            centre = city["plot"]
            plan = p.city_plan(slug, city, info[centre]["rec"], info[centre]["coastal"])
            land = [pid for pid in city_tiles[centre] if pid != centre and terrain[pid] not in p.WATER
                    and pid not in impassable and not terrain[pid].endswith("MOUNTAIN")]
            placed = []
            for district in plan["district_options"]:
                if len(placed) >= plan["n_districts"]:
                    break
                dtype = district["district"]
                if dtype == "DISTRICT_HARBOR":
                    candidates = [pid for n in city_tiles[centre] for pid in neighbours(n)
                                  if terrain[pid] == "TERRAIN_COAST" and pid not in plot_owner
                                  and pid not in resource and pid not in used
                                  and 1 <= dist(pid, centre) <= DISTRICT_RANGE]
                else:
                    candidates = [pid for pid in land if pid not in used and pid not in resource
                                  and pid not in feature and dist(pid, centre) <= DISTRICT_RANGE
                                  and not (dtype in NO_ADJACENT_CITY and dist(pid, centre) < 2)]
                if not candidates:
                    continue  # try the next district the house's traits allow
                pid = min(set(candidates), key=lambda c: (dist(c, centre), c))
                used.add(pid)
                if dtype == "DISTRICT_HARBOR":
                    plot_owner[pid] = index
                    owner_adds.append((pid, index, centre, centre))
                districts_rows.append((dtype, centre, pid))
                building_rows.extend((b, pid) for b in district["buildings"])
                placed.append(district)
            building_rows.extend((b, centre) for b in plan["city_buildings"] if b != "BUILDING_PALACE")
            db.execute("UPDATE CityAttributes SET Value=? WHERE ID=? AND Type='Population' AND Name='Initial'",
                       (str(plan["pop"]), centre))
            # Improve as many tiles as there are working citizens, resources first.
            worked = max(0, plan["pop"] - len(placed))
            options = sorted(((pid not in resource, imp != "IMPROVEMENT_FARM",
                               dist(pid, centre), pid, imp)
                              for pid in land if pid not in used and dist(pid, centre) <= DISTRICT_RANGE
                              and (imp := improvement_for(terrain[pid], feature.get(pid), resource.get(pid)))))
            improvements = Counter()
            for *_key, pid, imp in options[:worked]:
                used.add(pid)
                improvement_rows.append((pid, imp, index))
                improvements[imp] += 1
            occupied.add(centre)
            unit_rows.append((plan["garrison"][0], index, centre))
            plans.append({"city": city, "plan": plan, "districts": placed, "improvements": improvements})
        # Reserves on free owned land within RESERVE_RANGE, capital first, then key castles.
        reserves = p.faction_reserves(slug, [x["plan"] for x in plans])
        unplaced = []
        order = sorted(plans, key=lambda x: (not x["city"]["capital"], -x["plan"]["importance"],
                                             x["city"]["research_priority"]))
        # Units may stand on land districts; small island/enclave houses fall back to
        # any owned land (e.g. Honganji's Kaga, Ryukyu's outer islands).
        slots = [pid for reach in (RESERVE_RANGE, DISTRICT_RANGE, width + height) for x in order for pid in sorted(
            (pid for pid in city_tiles[x["city"]["plot"]]
             if pid not in centres and pid not in impassable
             and terrain[pid] not in p.WATER and dist(pid, x["city"]["plot"]) <= reach),
            key=lambda pid, c=x["city"]["plot"]: (dist(pid, c), pid))]
        for unit in reserves:
            pid = next((s for s in slots if s not in occupied), None)
            if pid is None:
                unplaced.append(unit)
                continue
            occupied.add(pid)
            unit_rows.append((unit, index, pid))
        gold = p.starting_gold(slug)
        db.execute("INSERT INTO PlayerAttributes VALUES (?, 'Treasury', 'Balance', ?)", (index, f"{gold:.6f}"))
        units = [u for u, owner, _ in unit_rows if owner == index]
        report[slug] = {
            "tier": p.tier_of(slug), "cities": len(plans),
            "pop": sum(x["plan"]["pop"] for x in plans),
            "units": len(units),
            "districts": sum(len(x["districts"]) for x in plans),
            "walls": sum("BUILDING_WALLS" in x["plan"]["city_buildings"] for x in plans),
            "improvements": sum(sum(x["improvements"].values()) for x in plans),
            "gold": gold,
            "upkeep": sum(p.upkeep_of([], x["districts"]) for x in plans) + sum(p.UNIT_UPKEEP[u] for u in units),
            "unplaced": unplaced,
            "missing_districts": sum(x["plan"]["n_districts"] - len(x["districts"]) for x in plans),
        }
    db.executemany("INSERT INTO PlotOwners VALUES (?, ?, ?, ?)", owner_adds)
    db.executemany("INSERT INTO Districts VALUES (?, ?, ?)", districts_rows)
    db.executemany("INSERT INTO Buildings VALUES (?, ?)", building_rows)
    db.executemany("INSERT INTO PlotImprovements VALUES (?, ?, ?)", improvement_rows)
    db.executemany("INSERT INTO Units VALUES (?, ?, ?, ?, NULL)",
                   ((i, unit, owner, pid) for i, (unit, owner, pid) in enumerate(unit_rows)))
    return report


def map_path(km: int) -> Path:
    return p.MAPS / f"JAC_Japan_{km}km.Civ6Map"


def settler_features(km: int) -> list[tuple]:
    """The generator's features; the settler variant is never modified by apply()."""
    with closing(sqlite3.connect(p.MAPS / f"JAC_Japan_{km}km_Settlers.Civ6Map")) as db:
        return db.execute("SELECT ID, FeatureType FROM PlotFeatures ORDER BY ID").fetchall()


def apply_file(path: Path, km: int) -> dict:
    item = p.manifest_item(km)
    features = settler_features(km)
    db = sqlite3.connect(path)
    try:
        report = apply(db, item["width"], item["height"], km, item["cities"], features)
        db.commit()
        db.execute("VACUUM")
    finally:
        db.close()
    return report


def report_for(km: int) -> dict:
    """Report of the checked-in map, computed on a copy so the map file is not rewritten."""
    with tempfile.TemporaryDirectory() as tmp:
        copy = Path(tmp) / "map.Civ6Map"
        shutil.copy(map_path(km), copy)
        return apply_file(copy, km)


START_STATE_KEY = "JAC_JAPAN_START_STATE"  # written by poc/JAC_PoC_JapanMapScale/Scripts/JAC_Japan_StartState.lua


def lua_start_check(report: dict) -> str:
    """Lua for the live scenario: the state recorded at load (per leader) matches the map rows.

    AI players move garrisons and buy units on turn 1 before any UI step runs (live run
    2026-09-29), so the check reads the record the gameplay script made before turn 1.
    """
    expected = ",".join(
        "{l}={{{cities},{pop},{units},{districts},{walls},{improvements},{gold}}}".format(
            l=g.SEATS[g.SEAT_INDEX[slug]].leader, **row) for slug, row in report.items())
    return (
        "local E={" + expected + "}; "
        f"local s=Game:GetProperty('{START_STATE_KEY}'); "
        "if s==nil then error('no start state recorded (JAC_Japan_StartState.lua)') end; "
        "local S={}; for id,vals in string.gmatch(s,'(%d+)=([%d,%-]+)') do local t={}; "
        "for v in string.gmatch(vals,'%-?%d+') do t[#t+1]=tonumber(v) end; S[tonumber(id)]=t end; "
        "local out={'START'}; local bad={}; local names={'cities','pop','units','districts','walls','improvements'}; "
        "for i=0,21 do local leader=PlayerConfigurations[i]:GetLeaderTypeName(); local e=E[leader]; local got=S[i]; "
        "if not got or not e then error('missing player '..i..' '..tostring(leader)) end; "
        "if got[8]~=got[1] then bad[#bad+1]=leader..' ungarrisoned cities '..(got[1]-got[8]) end; "
        "if got[9]>0 then bad[#bad+1]=leader..' stacked plots '..got[9] end; "
        "for k=1,6 do if got[k]~=e[k] then bad[#bad+1]=leader..' '..names[k]..' '..got[k]..' expected '..e[k] end end; "
        "if got[7]<e[7] then bad[#bad+1]=leader..' gold '..got[7]..' expected >= '..e[7] end; "
        "out[#out+1]=leader:gsub('LEADER_JAC_','')..' '..table.concat(got,'/',1,7) end; "
        "if #bad>0 then error(table.concat(bad,'; ')) end; return table.concat(out,'; ')"
    )


SURVIVORS = (
    "local t=Game.GetCurrentGameTurn(); local alive,dead={},{}; "
    "for i=0,21 do local n=PlayerConfigurations[i]:GetLeaderTypeName():gsub('LEADER_JAC_',''); local p=Players[i]; "
    "if p and p:IsAlive() then local c=p:GetCities():GetCount(); local cap=p:GetCities():GetCapitalCity(); "
    "alive[#alive+1]=n..' c'..c..(cap and '' or ' NO_CAPITAL') else dead[#dead+1]=n end end; "
    "return 'SURVIVORS T'..t..' '..#alive..'/22: '..table.concat(alive,', ')..' | DEAD: '..table.concat(dead,', ')"
)


def write_scenario(report: dict) -> None:
    """The 50-turn AI autoplay survey of the baseline, plus a start check and a survivor line."""
    data = json.loads(BASELINE_SCENARIO.read_text(encoding="utf-8"))
    steps = data["steps"]
    start = next(i for i, step in enumerate(steps) if step["action"] == "start_game")
    # The driver on main has no allow_human_elimination option, so the human
    # seat is the largest house (Ouchi) as in the baseline; the rest is AI autoplay.
    steps.insert(start + 1, {"action": "ui_lua", "state_name": "InGame", "code": lua_start_check(report)})
    steps.append({"action": "ui_lua", "state_name": "InGame", "code": SURVIVORS})
    data["name"] = ("JAC Japan map 15 km prosperous 1534 start: start-state check, then 50 turns of AI "
                    "autoplay (OUCHI human) with a survey every 10 turns and the survivors")
    SCENARIO.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main() -> None:
    for km in g.SCALES:
        report = apply_file(map_path(km), km)
        if km == 15:
            write_scenario(report)
        print(f"{km} km:", json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
