"""The prosperous 1534 start in the historical maps (SSOT 3.1, Q28 option 1)."""

from __future__ import annotations

import json
from collections import Counter, defaultdict
from contextlib import closing
from pathlib import Path
import shutil
import sqlite3
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET

TOOL = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOL))
sys.path.insert(0, str(TOOL.parents[0] / "civ6-autotest"))
import generate as g  # noqa: E402
import prosper_start as ps  # noqa: E402
import prosperity as p  # noqa: E402
from scenario import validate_scenario  # noqa: E402

ALLOWED_BUILDINGS = {  # kept by poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Unlocks.sql:69-74
    "BUILDING_PALACE", "BUILDING_WALLS", "BUILDING_GRANARY", "BUILDING_MARKET", "BUILDING_LIGHTHOUSE",
    "BUILDING_BARRACKS", "BUILDING_SHRINE", "BUILDING_TEMPLE", "BUILDING_WORKSHOP", "BUILDING_LIBRARY"}
DISTRICT_OF = {b: d for d, b in p.DISTRICT_BUILDING.items()} | {"BUILDING_TEMPLE": "DISTRICT_HOLY_SITE"}
ALLOWED_IMPROVEMENTS = {"IMPROVEMENT_FARM", "IMPROVEMENT_MINE", "IMPROVEMENT_QUARRY"}  # Unlocks.sql:80-82

# Snapshot of the per-tier totals: (factions, pop, units, districts, walls, improvements, gold).
TIER_TOTALS = {
    15: {"A": (7, 80, 55, 16, 11, 60, 3850), "B": (5, 47, 34, 7, 7, 31, 1620),
         "C": (4, 47, 27, 9, 8, 29, 1070), "D": (6, 30, 21, 6, 9, 21, 780)},
    20: {"A": (7, 77, 55, 16, 11, 45, 3850), "B": (5, 46, 34, 5, 7, 35, 1620),
         "C": (4, 47, 27, 6, 8, 27, 1070), "D": (6, 29, 21, 5, 9, 24, 780)},
}
# Districts that do not fit without clearing woods are left out (owner decision E1=b).
MISSING_DISTRICTS = {15: {}, 20: {"MATSUDAIRA": 1, "ASHIKAGA": 1}}


def rows(db: sqlite3.Connection, sql: str, *args) -> list[tuple]:
    return db.execute(sql, args).fetchall()


class ProsperousMapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.maps = {}
        for km in g.SCALES:
            item = p.manifest_item(km)
            db = sqlite3.connect(f"file:{ps.map_path(km).as_posix()}?mode=ro", uri=True)
            cls.addClassCleanup(db.close)
            cls.maps[km] = (item, db)

    def test_totals_per_tier(self):
        for km, (item, db) in self.maps.items():
            with self.subTest(km=km):
                pop = dict(rows(db, "SELECT ID, CAST(Value AS INTEGER) FROM CityAttributes"))
                owner_of = {c["plot"]: c["owner"] for c in item["cities"]}
                units = Counter(g.SEATS[o].slug for (o,) in rows(db, "SELECT Owner FROM Units"))
                districts = Counter(owner_of[c] for (c,) in rows(
                    db, "SELECT CityID FROM Districts WHERE DistrictType<>'DISTRICT_CITY_CENTER'"))
                walls = Counter(owner_of[b] for (b,) in rows(
                    db, "SELECT Plot FROM Buildings WHERE BuildingType='BUILDING_WALLS'"))
                imps = Counter(g.SEATS[o].slug for (o,) in rows(db, "SELECT ImprovementOwner FROM PlotImprovements"))
                gold = {g.SEATS[i].slug: round(float(v)) for i, v in rows(
                    db, "SELECT ID, Value FROM PlayerAttributes WHERE Type='Treasury' AND Name='Balance'")}
                totals = defaultdict(lambda: [0] * 7)
                for seat in g.SEATS:
                    t = totals[p.tier_of(seat.slug)]
                    own = [c["plot"] for c in item["cities"] if c["owner"] == seat.slug]
                    for k, v in enumerate((1, sum(pop[c] for c in own), units[seat.slug], districts[seat.slug],
                                           walls[seat.slug], imps[seat.slug], gold[seat.slug])):
                        t[k] += v
                self.assertEqual({k: tuple(v) for k, v in totals.items()}, TIER_TOTALS[km])
                # A richer tier gets more per house: capital population, units and gold.
                capital_pop = defaultdict(list)
                for c in item["cities"]:
                    if c["capital"] and c["id"] != p.KYOTO:
                        capital_pop[p.tier_of(c["owner"])].append(pop[c["plot"]])
                for key in (lambda t: sum(capital_pop[t]) / len(capital_pop[t]),
                            lambda t: totals[t][2] / totals[t][0], lambda t: totals[t][6] / totals[t][0]):
                    self.assertGreater(key("A"), key("B"))
                    self.assertGreater(key("B"), key("C") - 0.1)
                    self.assertGreater(key("C"), key("D"))

    def test_every_city_has_one_garrison_of_its_owner(self):
        for km, (item, db) in self.maps.items():
            units = defaultdict(list)
            for unit, owner, plot in rows(db, "SELECT UnitType, Owner, Plot FROM Units"):
                units[plot].append((unit, owner))
            for city in item["cities"]:
                with self.subTest(km=km, city=city["id"]):
                    self.assertEqual(len(units[city["plot"]]), 1)
                    unit, owner = units[city["plot"]][0]
                    self.assertEqual(owner, g.SEAT_INDEX[city["owner"]])
                    strong = city["capital"] or p.IMPORTANCE[city["id"]] >= 2
                    self.assertEqual(unit, "UNIT_SPEARMAN" if strong else "UNIT_WARRIOR")

    def test_no_unit_on_invalid_terrain(self):
        for km, (item, db) in self.maps.items():
            with self.subTest(km=km):
                bad = rows(db, "SELECT u.ID FROM Units u JOIN Plots t ON t.ID=u.Plot LEFT JOIN PlotOwners o ON o.ID=u.Plot "
                               "WHERE t.TerrainType IN ('TERRAIN_COAST','TERRAIN_OCEAN') OR t.IsImpassable<>0 "
                               "OR t.TerrainType LIKE '%MOUNTAIN' OR o.Owner IS NOT u.Owner")
                self.assertEqual(bad, [])
                # One military unit per tile, as in the official scenario maps (research doc 5.4).
                self.assertEqual(rows(db, "SELECT Plot FROM Units GROUP BY Plot HAVING COUNT(*)>1"), [])
                self.assertLessEqual({u for (u,) in rows(db, "SELECT DISTINCT UnitType FROM Units")}, ps.LAND_UNITS)
                self.assertEqual([u for (u,) in rows(db, "SELECT ID FROM Units ORDER BY ID")],
                                 list(range(len(rows(db, "SELECT ID FROM Units")))))

    def test_districts_buildings_and_improvements_follow_the_rules(self):
        for km, (item, db) in self.maps.items():
            with self.subTest(km=km):
                width = item["width"]
                features = dict(ps.settler_features(km))
                resource = dict(rows(db, "SELECT ID, ResourceType FROM PlotResources"))
                terrain = dict(rows(db, "SELECT ID, TerrainType FROM Plots"))
                owner = {pid: (o, c) for pid, o, c in rows(db, "SELECT ID, Owner, CityOwner FROM PlotOwners")}
                district_at = {}
                for dtype, city, plot in rows(db, "SELECT DistrictType, CityID, Plot FROM Districts "
                                                  "WHERE DistrictType<>'DISTRICT_CITY_CENTER'"):
                    district_at[plot] = dtype
                    d = g.hex_distance((plot % width, plot // width), (city % width, city // width))
                    self.assertEqual(owner[plot][1], city)
                    self.assertTrue(1 <= d <= ps.DISTRICT_RANGE)
                    self.assertNotIn(plot, resource)
                    self.assertNotIn(plot, features)
                    if dtype == "DISTRICT_ENCAMPMENT":
                        self.assertGreaterEqual(d, 2)
                    if dtype == "DISTRICT_HARBOR":
                        self.assertEqual(terrain[plot], "TERRAIN_COAST")
                    else:
                        self.assertNotIn(terrain[plot], p.WATER)
                        self.assertFalse(terrain[plot].endswith("MOUNTAIN"))
                cities = {c["plot"]: c for c in item["cities"]}
                for building, plot in rows(db, "SELECT BuildingType, Plot FROM Buildings"):
                    self.assertIn(building, ALLOWED_BUILDINGS)
                    if building in DISTRICT_OF:
                        self.assertEqual(district_at[plot], DISTRICT_OF[building])
                    else:
                        self.assertIn(plot, cities)
                walls = {plot for (plot,) in rows(db, "SELECT Plot FROM Buildings WHERE BuildingType='BUILDING_WALLS'")}
                self.assertEqual(walls, {c["plot"] for c in item["cities"] if c["id"] not in p.NO_WALLS
                                         and (c["capital"] or p.IMPORTANCE[c["id"]] >= 2)})
                for plot, imp, imp_owner in rows(db, "SELECT ID, ImprovementType, ImprovementOwner FROM PlotImprovements"):
                    self.assertIn(imp, ALLOWED_IMPROVEMENTS)
                    self.assertEqual(owner[plot][0], imp_owner)
                    self.assertNotIn(plot, district_at)
                    self.assertNotIn(plot, cities)
                    self.assertNotIn(plot, features)
                    self.assertEqual(ps.improvement_for(terrain[plot], None, resource.get(plot)), imp)

    def test_no_forest_or_marsh_is_removed(self):
        for km, (_item, db) in self.maps.items():
            with self.subTest(km=km):
                base = set(ps.settler_features(km))
                self.assertGreater(len(base), 0)
                self.assertEqual(set(rows(db, "SELECT ID, FeatureType FROM PlotFeatures")), base)
                self.assertTrue({"FEATURE_FOREST", "FEATURE_MARSH"} <= {k for _, k in base})

    def test_kyoto_and_treasury(self):
        for km, (item, db) in self.maps.items():
            with self.subTest(km=km):
                kyoto = next(c["plot"] for c in item["cities"] if c["id"] == p.KYOTO)
                self.assertEqual(rows(db, "SELECT Value FROM CityAttributes WHERE ID=?", kyoto), [("6",)])
                built = {(d, b) for d, b in rows(db, "SELECT d.DistrictType, b.BuildingType FROM Districts d "
                                                     "JOIN Buildings b ON b.Plot=d.Plot WHERE d.CityID=?", kyoto)}
                # 20 km has no woodless plot left for Kyoto's holy site (E1=b), so only 15 km has it.
                want = {("DISTRICT_COMMERCIAL_HUB", "BUILDING_MARKET")}
                if km == 15:
                    want |= {("DISTRICT_HOLY_SITE", "BUILDING_SHRINE"), ("DISTRICT_HOLY_SITE", "BUILDING_TEMPLE")}
                self.assertLessEqual(want, built)
                gold = rows(db, "SELECT ID, Value FROM PlayerAttributes WHERE Type='Treasury' ORDER BY ID")
                self.assertEqual(gold, [(i, f"{p.starting_gold(s.slug)}.000000") for i, s in enumerate(g.SEATS)])
        self.assertEqual(p.starting_gold("ODA"), 200)       # 8 man koku, trade 3
        self.assertEqual(p.starting_gold("MORI"), p.GOLD_FLOOR)
        self.assertEqual(p.starting_gold("ASHIKAGA"), 370)  # 150 + 120 + Kyoto's 100

    def test_settler_maps_stay_bare(self):
        for km in g.SCALES:
            with closing(sqlite3.connect(p.MAPS / f"JAC_Japan_{km}km_Settlers.Civ6Map")) as db:
                for table in ("Units", "PlotImprovements", "PlayerAttributes", "Buildings", "CityAttributes"):
                    self.assertEqual(rows(db, f"SELECT COUNT(*) FROM {table}"), [(0,)])

    def test_apply_is_deterministic_and_matches_the_checked_in_maps(self):
        def dump(path: Path) -> dict:
            with closing(sqlite3.connect(path)) as db:
                return {t: sorted(rows(db, f'SELECT * FROM "{t}"'))
                        for (t,) in rows(db, "SELECT name FROM sqlite_master WHERE type='table'")}
        for km in g.SCALES:
            with self.subTest(km=km), tempfile.TemporaryDirectory() as tmp:
                copy = Path(tmp) / "map.Civ6Map"
                shutil.copy(ps.map_path(km), copy)
                first = ps.apply_file(copy, km)
                self.assertEqual(dump(copy), dump(ps.map_path(km)))
                self.assertEqual(ps.apply_file(copy, km), first)
                self.assertEqual(dump(copy), dump(ps.map_path(km)))
                self.assertTrue(all(not r["unplaced"] for r in first.values()))
                self.assertEqual({s: r["missing_districts"] for s, r in first.items() if r["missing_districts"]},
                                 MISSING_DISTRICTS[km])

    def test_improvement_rules(self):
        self.assertEqual(ps.improvement_for("TERRAIN_GRASS", None, None), "IMPROVEMENT_FARM")
        self.assertEqual(ps.improvement_for("TERRAIN_GRASS_HILLS", None, None), "IMPROVEMENT_MINE")
        self.assertEqual(ps.improvement_for("TERRAIN_GRASS_HILLS", None, "RESOURCE_IRON"), "IMPROVEMENT_MINE")
        self.assertEqual(ps.improvement_for("TERRAIN_PLAINS", None, "RESOURCE_SALT"), "IMPROVEMENT_MINE")
        self.assertEqual(ps.improvement_for("TERRAIN_GRASS", None, "RESOURCE_RICE"), "IMPROVEMENT_FARM")
        self.assertIsNone(ps.improvement_for("TERRAIN_GRASS", None, "RESOURCE_HORSES"))  # pasture not buildable
        self.assertIsNone(ps.improvement_for("TERRAIN_GRASS", "FEATURE_MARSH", None))
        self.assertIsNone(ps.improvement_for("TERRAIN_GRASS_MOUNTAIN", None, None))

    def test_live_scenario_is_current_and_valid(self):
        data = json.loads(ps.SCENARIO.read_text(encoding="utf-8"))
        validate_scenario(data)
        with tempfile.TemporaryDirectory() as tmp:
            original, ps.SCENARIO = ps.SCENARIO, Path(tmp) / "s.json"
            shutil.copy(original, ps.SCENARIO)
            try:
                ps.write_scenario(ps.report_for(15))
                self.assertEqual(json.loads(ps.SCENARIO.read_text(encoding="utf-8")), data)
            finally:
                ps.SCENARIO = original
        steps = data["steps"]
        self.assertIn("JAC_Japan_15km.Civ6Map", steps[1]["map_file"])
        self.assertEqual([pl["leader"] for pl in steps[1]["players"] if pl["human"]], ["LEADER_JAC_OUCHI_YOSHITAKA"])
        self.assertEqual(steps[3]["action"], "ui_lua")
        self.assertIn("ungarrisoned cities", steps[3]["code"])
        self.assertIn("stacked plots", steps[3]["code"])
        self.assertIn(f"Game:GetProperty('{ps.START_STATE_KEY}')", steps[3]["code"])
        self.assertIn("LEADER_JAC_ODA_NOBUHIDE={2,5,4,", steps[3]["code"])
        self.assertEqual(sum(s["turns"] for s in steps if s["action"] == "autoplay"), 50)
        self.assertEqual(steps[-1]["code"], ps.SURVIVORS)

    def test_survivor_step_fails_on_a_death_or_a_lost_capital(self):
        # Regression rule of the live run: all 22 houses alive with a capital after 50 turns.
        self.assertIn("if #alive<22 or nocap>0 then error('regression: '..line) end; return line", ps.SURVIVORS)
        self.assertIn("if not cap then nocap=nocap+1 end", ps.SURVIVORS)
        self.assertIn("for i=0,21 do", ps.SURVIVORS)

    def test_start_check_compares_every_planned_building(self):
        # The planned set in the start check equals the Buildings rows of the checked-in map per owner.
        item, db = self.maps[15]
        owner = dict(rows(db, "SELECT ID, Owner FROM PlotOwners"))
        in_map: dict[str, Counter] = defaultdict(Counter)
        for kind, plot in rows(db, "SELECT BuildingType, Plot FROM Buildings"):
            in_map[g.SEATS[owner[plot]].slug][kind] += 1
        report = ps.report_for(15)
        code = json.loads(ps.SCENARIO.read_text(encoding="utf-8"))["steps"][3]["code"]
        self.assertIn(f"Game:GetProperty('{ps.START_BUILDINGS_KEY}')", code)
        for slug, row in report.items():
            with self.subTest(slug=slug):
                self.assertEqual(row["buildings"], dict(in_map[slug]))
                self.assertIn(row["buildings"]["BUILDING_PALACE"], (1,))
                table = ",".join(f"{k}={v}" for k, v in row["buildings"].items())
                self.assertIn(f"{g.SEATS[g.SEAT_INDEX[slug]].leader}={{{table}}}", code)
        self.assertEqual(sum(sum(r["buildings"].values()) for r in report.values()),
                         rows(db, "SELECT COUNT(*) FROM Buildings")[0][0])

    def test_start_state_script_runs_only_on_the_historical_maps(self):
        # AI players act on turn 1 before a UI step runs, so the live check reads what this
        # gameplay script recorded at load (before turn 1) under the same property key.
        script = (g.MOD / "Scripts/JAC_Japan_StartState.lua").read_text(encoding="utf-8")
        self.assertIn(f'local KEY = "{ps.START_STATE_KEY}"', script)
        self.assertIn(f'local BUILDINGS_KEY = "{ps.START_BUILDINGS_KEY}"', script)
        self.assertIn("for row in GameInfo.Buildings() do", script)
        self.assertIn("GameConfiguration.GetStartTurn()", script)
        mod = ET.parse(g.MOD / "JAC_PoC_JapanMapScale.modinfo").getroot()
        criteria = {c.attrib["id"]: c.findtext("./ConfigurationValueMatches/Value")
                    for c in mod.findall("./ActionCriteria/Criteria")}
        actions = [a for a in mod.findall("./InGameActions/AddGameplayScripts") if a.attrib["id"].endswith("_StartState")]
        self.assertEqual([a.findtext("File") for a in actions], ["Scripts/JAC_Japan_StartState.lua"] * 2)
        self.assertEqual({criteria[a.attrib["criteria"]] for a in actions},
                         {f"{{403be889-1455-4de7-967b-ad54c0ff3431}}Maps/JAC_Japan_{km}km.Civ6Map" for km in g.SCALES})
        self.assertIn("Scripts/JAC_Japan_StartState.lua", [f.text for f in mod.findall("./Files/File")])


if __name__ == "__main__":
    unittest.main()
