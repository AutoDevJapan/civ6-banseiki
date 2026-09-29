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
        text = ET.parse(g.MOD / "Text" / "JAC_JapanMapScale_Text.xml").getroot()
        by_tag = {}
        for row in text.findall("./LocalizedText/Row"):
            by_tag.setdefault(row.attrib["Tag"], set()).add(row.attrib["Language"])
        for km in g.SCALES:
            self.assertEqual(by_tag[f"LOC_JAC_JAPAN_{km}_NAME"], {"ja_JP", "en_US", "ru_RU"})
        mod = ET.parse(g.MOD / "JAC_PoC_JapanMapScale.modinfo").getroot()
        packaged = {element.text for element in mod.findall("./Files/File")}
        for item in self.manifest:
            file = "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/" + item["file"]
            self.assertIn(file, {row.attrib["File"] for row in config.findall("./Maps/Row")})
            self.assertIn("Maps/" + item["file"], packaged)
            self.assertIn("Maps/" + item["file"].replace(".Civ6Map", ".lua"), packaged)
            scenario = json.loads((g.ROOT / "tools" / "civ6-autotest" / "scenarios" /
                                   f"japan-map-{item['km_per_tile']}km.json").read_text(encoding="utf-8"))
            self.assertEqual(scenario["steps"][1]["map_file"], file)
            self.assertEqual(scenario["steps"][1]["ruleset"], "RULESET_JAC_BANSEIKI")
            self.assertEqual(len(scenario["steps"][1]["players"]), 22)
            self.assertEqual(scenario["steps"][4]["turns"], 50)

    def test_leaders_unique_and_human_from_banseiki_pool(self):
        leaders = [seat.leader for seat in g.SEATS]
        self.assertEqual(len(set(leaders)), 22)  # SSOT 2.2: leaders never repeat; civilizations may
        self.assertEqual(g.SEATS[0].leader, "LEADER_JAC_ODA_NOBUHIDE")

    def test_official_leaders_are_installed(self):
        base = g.OFFICIAL.parents[4]
        files = (base / "Base/Assets/Configuration/Data/Players.xml",
                 base / "DLC/Expansion1/Config/Expansion1_Players.xml",
                 base / "DLC/Expansion2/Config/Expansion2_Players.xml")
        if not all(file.is_file() for file in files):
            self.skipTest("Civ6 and both expansions not installed at default path")
        rows = [row.attrib for file in files for row in ET.parse(file).getroot().findall("./Players/Row")]
        for seat in g.SEATS:
            if seat.leader.startswith("LEADER_JAC_"):
                continue  # defined by JAC_PoC_CustomCivs
            self.assertTrue(any(row.get("LeaderType") == seat.leader and
                                row.get("CivilizationType") == seat.civ
                                for row in rows), seat.leader)


if __name__ == "__main__":
    unittest.main()
