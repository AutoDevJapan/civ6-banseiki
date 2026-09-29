from __future__ import annotations

import json
import math
from pathlib import Path
import sqlite3
import sys
import unittest
import xml.etree.ElementTree as ET


TOOL = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOL))
import generate as g
from start_1534 import HEADS, civ_id, data, leader_id


MANIFEST = g.MOD / "Maps" / "manifest.json"


class ProjectionTests(unittest.TestCase):
    def test_round_trip_across_japan(self):
        for lon, lat in ((127.719, 26.218), (135.76, 35.03), (142.149, 42.633),
                         (123.0, 24.5), (145.0, 45.0)):
            with self.subTest(lon=lon, lat=lat):
                actual = g.unproject(*g.project(lon, lat))
                self.assertAlmostEqual(actual[0], lon, places=8)
                self.assertAlmostEqual(actual[1], lat, places=8)

    def test_hex_neighbours_and_odd_row_offset(self):
        grid = g.HexGrid(20, 8, 8, 0, 0)
        self.assertAlmostEqual(grid.centre(2, 1)[0] - grid.centre(2, 0)[0], 10)
        self.assertEqual(len(grid.neighbours(3, 3)), 6)
        for row in (2, 3):
            for col, other_row in grid.neighbours(3, row):
                self.assertIn((3, row), grid.neighbours(col, other_row))
                x1, y1 = grid.centre(3, row)
                x2, y2 = grid.centre(col, other_row)
                self.assertAlmostEqual(math.hypot(x2 - x1, y2 - y1), 20, places=8)

    def test_closest_centre(self):
        grid = g.HexGrid(25, 10, 10, -100, -100)
        for col, row in ((1, 2), (4, 5), (8, 8)):
            self.assertEqual(grid.closest(*g.unproject(*grid.centre(col, row))), (col, row))


@unittest.skipUnless(MANIFEST.is_file(), "generated map files absent")
class GeneratedMapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))

    def test_schema_matches_official_and_plot_inventory(self):
        if not g.OFFICIAL.is_file():
            self.skipTest("official Civ6 map not installed")
        with sqlite3.connect(g.OFFICIAL) as official:
            schema = dict(official.execute("SELECT name, sql FROM sqlite_master WHERE type='table'"))
        gs_map = g.OFFICIAL.parents[4] / "DLC/Expansion2/Maps/EarthMaps/WorldMapHuge_XP2.Civ6Map"
        if gs_map.is_file():
            with sqlite3.connect(gs_map) as gs:
                self.assertEqual(dict(gs.execute("SELECT name, sql FROM sqlite_master WHERE type='table'")), schema)
        for item in self.manifest:
            with self.subTest(km=item["km_per_tile"]), sqlite3.connect(g.MOD / "Maps" / item["file"]) as db:
                self.assertEqual(dict(db.execute("SELECT name, sql FROM sqlite_master WHERE type='table'")), schema)
                self.assertEqual(db.execute("PRAGMA integrity_check").fetchone()[0], "ok")
                width, height, wrapx, wrapy, size = db.execute(
                    "SELECT Width,Height,WrapX,WrapY,MapSizeType FROM Map").fetchone()
                self.assertEqual((width, height, wrapx, wrapy, size),
                                 (item["width"], item["height"], 0, 0, "MAPSIZE_HUGE"))
                self.assertEqual(db.execute("SELECT COUNT(*) FROM Plots").fetchone()[0], width * height)
                self.assertEqual(db.execute("SELECT COUNT(*) FROM Plots WHERE ContinentType='CONTINENT_ASIA'").fetchone()[0], item["land_tiles"])
                self.assertEqual(db.execute("SELECT COUNT(*) FROM PlotResources").fetchone()[0], 0)
                self.assertLessEqual((g.MOD / "Maps" / item["file"]).stat().st_size, 10_000_000)

    def test_known_land_and_removed_foreign_land(self):
        # Korea and Sakhalin may lie inside a map's rectangular board but must
        # always be sea.  Cities at the other points must remain Japanese land.
        land = ((139.69, 35.68), (141.35, 43.06), (127.719, 26.218), (130.56, 31.60),
                *((lon, lat) for _, lon, lat in g.ISLAND_ANCHORS))
        water = ((129.07, 35.18), (135.0, 39.0), (132.0, 38.5))
        for item in self.manifest:
            grid = g.HexGrid.for_scale(item["km_per_tile"],
                                       g.read_shapes(g.CACHE / "ne_10m_admin_0_countries.zip",
                                                     "ne_10m_admin_0_countries", country="JPN")) if (g.CACHE / "ne_10m_admin_0_countries.zip").exists() else None
            if grid is None:
                self.skipTest("Natural Earth cache absent")
            with self.subTest(km=item["km_per_tile"]), sqlite3.connect(g.MOD / "Maps" / item["file"]) as db:
                for lon, lat in land:
                    col, row = grid.closest(lon, lat)
                    self.assertIn(db.execute("SELECT TerrainType FROM Plots WHERE ID=?",
                                             (grid.index(col, row),)).fetchone()[0],
                                  ("TERRAIN_GRASS", "TERRAIN_GRASS_HILLS", "TERRAIN_GRASS_MOUNTAIN"))
                for lon, lat in water:
                    col, row = grid.closest(lon, lat)
                    terrain = db.execute("SELECT TerrainType FROM Plots WHERE ID=?",
                                         (grid.index(col, row),)).fetchone()[0]
                    self.assertIn(terrain, ("TERRAIN_COAST", "TERRAIN_OCEAN"))

    def test_22_starts_unique_and_traversable(self):
        for item in self.manifest:
            starts = item["starts"]
            self.assertEqual(len(starts), 22)
            self.assertEqual(len({start["plot"] for start in starts}), 22)
            self.assertEqual({start["seat"] for start in starts}, {seat.slug for seat in g.SEATS})
            with sqlite3.connect(g.MOD / "Maps" / item["file"]) as db:
                self.assertEqual(db.execute("SELECT COUNT(*) FROM StartPositions").fetchone()[0], 22)
                for start in starts:
                    terrain, impassable = db.execute("SELECT TerrainType,IsImpassable FROM Plots WHERE ID=?",
                                                    (start["plot"],)).fetchone()
                    self.assertTrue(terrain.startswith("TERRAIN_GRASS"))
                    self.assertEqual(impassable, 0)
                    self.assertEqual(db.execute("SELECT Type,Value FROM StartPositions WHERE Plot=?",
                                                (start["plot"],)).fetchone(),
                                     ("LEADER", start["leader"]))
                    self.assertLessEqual(start["displacement_km"], 30)
            self.assertEqual(item["close_start_pairs"], g.close_start_pairs(starts))

    def test_historical_players_castles_and_territory(self):
        draft = data()
        self.assertEqual(len(draft["castles"]), 53)
        expected_castles = {castle["id"]: castle for castle in draft["castles"]}
        expected_capitals = {seat.slug: g.SEAT_CASTLES.get(seat.slug, f"{seat.slug}_SEAT")
                             for seat in g.SEATS}
        for item in self.manifest:
            with self.subTest(km=item["km_per_tile"]), sqlite3.connect(g.MOD / "Maps" / item["file"]) as db:
                cities = {city["id"]: city for city in item["cities"]}
                self.assertEqual(len(cities), 56)
                self.assertEqual(set(cities), set(expected_castles) | {"ODA_SEAT", "DATE_SEAT", "ASHIKAGA_SEAT"})
                for castle_id, castle in expected_castles.items():
                    city = cities[castle_id]
                    self.assertEqual(city["owner"], castle["owner_faction_1534_draft"])
                    self.assertLessEqual(city["displacement_km"], 20)
                for slug, castle_id in expected_capitals.items():
                    self.assertEqual(cities[castle_id]["owner"], slug)
                    self.assertTrue(cities[castle_id]["capital"])
                    capital = db.execute("SELECT Plot FROM Cities WHERE Owner=? AND Name=?",
                                         (g.SEAT_INDEX[slug], f"LOC_JAC_1534_CAPITAL_{slug}")).fetchone()
                    self.assertEqual(capital, (cities[castle_id]["plot"],))
                players = db.execute("SELECT ID,CivilizationType,LeaderType FROM Players ORDER BY ID").fetchall()
                self.assertEqual(players, [(i, seat.civ, seat.leader) for i, seat in enumerate(g.SEATS)])
                self.assertEqual(db.execute("SELECT COUNT(*) FROM Cities").fetchone()[0], 56)
                self.assertEqual(db.execute("SELECT COUNT(*) FROM Districts WHERE DistrictType='DISTRICT_CITY_CENTER'").fetchone()[0], 56)
                self.assertEqual(db.execute("SELECT COUNT(*) FROM Units").fetchone()[0], 0)
                self.assertEqual(db.execute("SELECT COUNT(*) FROM PlotOwners").fetchone()[0], item["land_tiles"])
                self.assertEqual(db.execute("SELECT COUNT(*) FROM Plots p LEFT JOIN PlotOwners o ON p.ID=o.ID WHERE p.ContinentType='CONTINENT_ASIA' AND o.ID IS NULL").fetchone()[0], 0)
                self.assertEqual(db.execute("SELECT COUNT(*) FROM Cities c JOIN Plots p ON c.Plot=p.ID WHERE p.ContinentType!='CONTINENT_ASIA' OR p.IsImpassable!=0").fetchone()[0], 0)
                self.assertEqual(db.execute("SELECT COUNT(*) FROM PlotOwners o LEFT JOIN Cities c ON o.CityOwner=c.Plot WHERE c.Plot IS NULL OR c.Owner!=o.Owner OR o.CityWorking!=o.CityOwner").fetchone()[0], 0)
                self.assertEqual(db.execute("SELECT COUNT(*) FROM Cities c JOIN PlotOwners o ON c.Plot=o.ID WHERE c.Owner!=o.Owner OR o.CityOwner!=c.Plot").fetchone()[0], 0)

    def test_settler_variant_has_no_premade_state(self):
        for item in self.manifest:
            with sqlite3.connect(g.MOD / "Maps" / item["settler_file"]) as db:
                for table in ("Players", "Cities", "Districts", "PlotOwners", "Units"):
                    self.assertEqual(db.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0], 0)
                self.assertEqual(db.execute("SELECT COUNT(*) FROM StartPositions").fetchone()[0], 22)

    def test_close_castle_pairs_report(self):
        self.assertEqual({item["km_per_tile"]: len(item["close_city_pairs"]) for item in self.manifest},
                         {20: 55, 15: 32})
        for item in self.manifest:
            self.assertEqual(item["close_city_pairs"], g.close_city_pairs(item["cities"]))
            self.assertTrue(all(1 <= pair[2] <= g.CITY_MIN_RANGE for pair in item["close_city_pairs"]))

    def test_hex_distance_matches_neighbours(self):
        grid = g.HexGrid(40, 20, 20, 0.0, 0.0)
        for col, row in ((5, 4), (5, 5)):
            for other in grid.neighbours(col, row):
                self.assertEqual(g.hex_distance((col, row), other), 1)
        self.assertEqual(g.hex_distance((0, 0), (3, 0)), 3)
        self.assertEqual(g.hex_distance((0, 0), (0, 2)), 2)

    def test_config_and_scenarios_cover_all_scales(self):
        config = ET.parse(g.MOD / "Config" / "JAC_JapanMapScale_Config.xml").getroot()
        self.assertEqual(len(config.findall("./Maps/Row")), 4)
        self.assertEqual(len(config.findall("./MapSizes/Row")), 4)
        self.assertEqual(config.findall("./Players/Row"), [])
        self.assertEqual(len(config.findall("./MapLeaders/Row")), 88)
        map_leaders = {}
        for row in config.findall("./MapLeaders/Row"):
            map_leaders.setdefault(row.attrib["Map"], []).append(row.attrib["LeaderType"])
        self.assertEqual(len(map_leaders), 4)
        for leaders in map_leaders.values():
            self.assertEqual(len(leaders), 22)
            self.assertEqual(set(leaders), {seat.leader for seat in g.SEATS})
            self.assertEqual(len(set(leaders)), 22)
        for row in config.findall("./MapSizes/Row"):
            self.assertEqual((row.attrib["MinPlayers"], row.attrib["MaxPlayers"],
                              row.attrib["DefaultPlayers"], row.attrib["DefaultCityStates"]),
                             ("22", "22", "22", "0"))
        text = ET.parse(g.MOD / "Text" / "JAC_JapanMapScale_Text.xml").getroot()
        by_tag = {}
        for row in text.findall("./LocalizedText/Row"):
            by_tag.setdefault(row.attrib["Tag"], set()).add(row.attrib["Language"])
        for km in g.SCALES:
            for mode in ("HISTORICAL", "SETTLERS"):
                self.assertEqual(by_tag[f"LOC_JAC_JAPAN_{km}_{mode}_NAME"], {"ja_JP", "en_US", "ru_RU"})
        mod = ET.parse(g.MOD / "JAC_PoC_JapanMapScale.modinfo").getroot()
        packaged = {element.text for element in mod.findall("./Files/File")}
        for item in self.manifest:
            file = "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/" + item["file"]
            self.assertIn(file, {row.attrib["File"] for row in config.findall("./Maps/Row")})
            self.assertIn("Maps/" + item["file"], packaged)
            self.assertIn("Maps/" + item["file"].replace(".Civ6Map", ".lua"), packaged)
            self.assertIn("Maps/" + item["settler_file"], packaged)
            scenario = json.loads((g.ROOT / "tools" / "civ6-autotest" / "scenarios" /
                                   f"japan-map-{item['km_per_tile']}km.json").read_text(encoding="utf-8"))
            self.assertEqual(scenario["steps"][1]["map_file"], file)
            self.assertEqual(scenario["steps"][1]["ruleset"], "RULESET_JAC_BANSEIKI")
            self.assertEqual(len(scenario["steps"][1]["players"]), 22)
            self.assertEqual(scenario["steps"][4]["turns"], 50)
            self.assertEqual(scenario["steps"][5]["capture_as"], "JAPAN_STATE")
            self.assertEqual(scenario["steps"][9]["equals_capture"], "JAPAN_STATE")

    def test_leaders_unique_and_human_from_banseiki_pool(self):
        leaders = [seat.leader for seat in g.SEATS]
        self.assertEqual(len(set(leaders)), 22)  # SSOT 2.2: leaders never repeat; civilizations may
        self.assertEqual(g.SEATS[0].leader, "LEADER_JAC_ODA_NOBUHIDE")

    def test_all_leaders_registered_with_nonmissing_icons(self):
        mod = g.MOD.parent / "JAC_PoC_CustomCivs"
        players = [row.attrib for filename in ("JAC_CustomCivs_Players.xml", "JAC_Japan1534_Players.xml")
                   for row in ET.parse(mod / "Config" / filename).getroot().findall("./Players/Row")]
        icons = {row.attrib["Name"] for filename in ("JAC_CustomCivs_Icons.xml", "JAC_Japan1534_Icons.xml")
                 for row in ET.parse(mod / "Data" / filename).getroot().findall("./IconDefinitions/Row")}
        self.assertEqual(len(players), 22)
        for seat in g.SEATS:
            self.assertEqual(seat.civ, civ_id(seat.slug))
            self.assertEqual(seat.leader, leader_id(seat.slug))
            player = next(p for p in players if p["LeaderType"] == seat.leader)
            self.assertEqual(player["CivilizationType"], seat.civ)
            self.assertIn(player["CivilizationIcon"], icons)
            self.assertIn(player["LeaderIcon"], icons)
            self.assertTrue(player["Portrait"].startswith("LEADER_JAC_"))
            self.assertTrue(player["PortraitBackground"].startswith("LEADER_JAC_"))
        self.assertEqual(len(HEADS), 22)
        localized = ET.parse(mod / "Text/JAC_Japan1534_Text.xml").getroot()
        by_key = {(row.attrib["Tag"], row.attrib["Language"]): row.findtext("Text")
                  for row in localized.findall("./LocalizedText/Row")}
        self.assertEqual(by_key[("LOC_LEADER_JAC_SHIMAZU_TAKAHISA_NAME", "ja_JP")], "島津貴久")
        self.assertEqual(by_key[("LOC_LEADER_JAC_AINU_TARIKONA_NAME", "ja_JP")], "タリコナ")
        for seat in g.SEATS:
            for lang in ("ja_JP", "en_US", "ru_RU"):
                self.assertIn((f"LOC_JAC_1534_CAPITAL_{seat.slug}", lang), by_key)

    def test_banseiki_filter_skips_japan_maps_and_settler_removal_is_scoped(self):
        ruleset = ET.parse(g.MOD.parent / "JAC_PoC_BanseikiRuleset" / "Config/JAC_Banseiki_Config.xml").getroot()
        query = next(row.attrib["SQL"] for row in ruleset.findall("./Queries/Row")
                     if row.attrib["QueryId"] == "JAC_Banseiki_HumanLeaderFilter")
        with sqlite3.connect(":memory:") as db:
            db.execute("CREATE TABLE Players (Domain TEXT, LeaderType TEXT)")
            db.executemany("INSERT INTO Players VALUES ('Players:Expansion2_Players', ?)",
                           ((seat.leader,) for seat in g.SEATS))
            other = db.execute(query, ("RULESET_JAC_BANSEIKI", 0, "JAC_BANSEIKI_ONLY", "Continents.lua")).fetchall()
            self.assertEqual({row[1] for row in other},
                             {"LEADER_JAC_ODA_NOBUHIDE", "LEADER_JAC_TAKEDA_NOBUTORA"})
            for km in g.SCALES:
                for suffix in ("", "_Settlers"):
                    map_file = f"{{403be889-1455-4de7-967b-ad54c0ff3431}}Maps/JAC_Japan_{km}km{suffix}.Civ6Map"
                    self.assertEqual(db.execute(query, ("RULESET_JAC_BANSEIKI", 0,
                                                        "JAC_BANSEIKI_ONLY", map_file)).fetchall(), [])
        mod = ET.parse(g.MOD / "JAC_PoC_JapanMapScale.modinfo").getroot()
        criteria = {c.attrib["id"]: c.findtext("./ConfigurationValueMatches/Value")
                    for c in mod.findall("./ActionCriteria/Criteria")}
        actions = mod.findall("./InGameActions/UpdateDatabase")
        self.assertEqual(len(actions), 2)
        for action in actions:
            selected = criteria[action.attrib["criteria"]]
            self.assertIn(selected, {f"{{403be889-1455-4de7-967b-ad54c0ff3431}}Maps/JAC_Japan_{km}km.Civ6Map"
                                     for km in g.SCALES})
            self.assertEqual(action.findtext("File"), "Data/JAC_Japan_NoSettlers.xml")
        removal = ET.parse(g.MOD / "Data/JAC_Japan_NoSettlers.xml").getroot()
        self.assertEqual(removal.find("./MajorStartingUnits/Delete").attrib, {"Unit": "UNIT_SETTLER"})


if __name__ == "__main__":
    unittest.main()
