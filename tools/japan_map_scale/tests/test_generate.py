from __future__ import annotations

import json
import math
from collections import Counter
from pathlib import Path
import re
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
                self.assertEqual(db.execute("SELECT COUNT(*) FROM PlotResources").fetchone()[0],
                                 item["resource_placement"]["placed_count"])
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

    def test_oda_has_a_passable_neighbouring_domain_for_initial_route_check(self):
        for item in self.manifest:
            with self.subTest(km=item["km_per_tile"]), sqlite3.connect(g.MOD / "Maps" / item["file"]) as db:
                grid = g.HexGrid(item["km_per_tile"], item["width"], item["height"], 0, 0)
                start = next(row for row in item["starts"] if row["seat"] == "ODA")
                candidates = []
                for col, row in grid.neighbours(start["col"], start["row"]):
                    plot = grid.index(col, row)
                    owner = db.execute("SELECT Owner FROM PlotOwners WHERE ID=?", (plot,)).fetchone()
                    terrain, impassable = db.execute(
                        "SELECT TerrainType,IsImpassable FROM Plots WHERE ID=?", (plot,)).fetchone()
                    city = db.execute("SELECT 1 FROM Cities WHERE Plot=?", (plot,)).fetchone()
                    if owner and owner[0] != 0 and terrain.startswith("TERRAIN_GRASS") and not impassable and not city:
                        candidates.append(plot)
                self.assertTrue(candidates)

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
                self.assertEqual([city["owner"] for city in item["cities"][:22]],
                                 [seat.slug for seat in g.SEATS])
                self.assertTrue(all(city["capital"] for city in item["cities"][:22]))
                self.assertFalse(any(city["capital"] for city in item["cities"][22:]))
                self.assertEqual([city["research_priority"] for city in item["cities"][22:]],
                                 [i for i, castle in enumerate(draft["castles"], 1)
                                  if castle["id"] not in g.SEAT_CASTLES.values()])
                for castle_id, castle in expected_castles.items():
                    city = cities[castle_id]
                    self.assertEqual(city["owner"], castle["owner_faction_1534_draft"])
                    self.assertLessEqual(city["displacement_km"],
                                         20 + g.MAX_NUDGE_TILES * item["km_per_tile"])
                for slug, castle_id in expected_capitals.items():
                    self.assertEqual(cities[castle_id]["owner"], slug)
                    self.assertTrue(cities[castle_id]["capital"])
                    capital = db.execute("SELECT Plot FROM Cities WHERE Owner=? AND Name=?",
                                         (g.SEAT_INDEX[slug], f"LOC_JAC_1534_CAPITAL_{slug}")).fetchone()
                    self.assertEqual(capital, (cities[castle_id]["plot"],))
                players = db.execute("SELECT ID,CivilizationType,LeaderType FROM Players ORDER BY ID").fetchall()
                self.assertEqual(players, [(i, seat.civ, seat.leader) for i, seat in enumerate(g.SEATS)])
                expected_rows = sorted((g.SEAT_INDEX[city["owner"]], city["plot"],
                                        f"LOC_JAC_1534_CAPITAL_{city['owner']}" if city["capital"]
                                        else city["name"]) for city in cities.values())
                self.assertEqual(sorted(db.execute("SELECT Owner,Plot,Name FROM Cities")), expected_rows)
                self.assertEqual(sorted(db.execute(
                    "SELECT DistrictType,CityID,Plot FROM Districts")),
                    sorted(("DISTRICT_CITY_CENTER", city["plot"], city["plot"])
                           for city in cities.values()))
                self.assertEqual(sorted(db.execute(
                    "SELECT ID,Type,Name,Value FROM CityAttributes")),
                    sorted((city["plot"], "Population", "Initial", "2")
                           for city in cities.values()))
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
                with sqlite3.connect(g.MOD / "Maps" / item["file"]) as historical:
                    for table in ("PlotResources", "PlotFeatures", "PlotRivers"):
                        self.assertEqual(db.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0],
                                         historical.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0])

    def test_resource_counts_deferred_candidates_and_city_exclusion(self):
        draft = g.resource_draft()
        land_candidates = sum(group.get("placement_domain") != "marine"
                              for group in draft["resource_groups"] for _ in group["points"])
        marine_candidates = sum(group.get("placement_domain") == "marine"
                                for group in draft["resource_groups"] for _ in group["points"])
        self.assertEqual((land_candidates, marine_candidates), (50, 17))
        self.assertEqual(sum(len(group["points"]) for group in draft["resource_groups"]), 67)
        self.assertEqual({item["km_per_tile"]: item["resource_placement"]["placed_count"]
                          for item in self.manifest}, {20: 51, 15: 54})  # after nudged cities (a194589) take or free candidate plots
        for item in self.manifest:
            with self.subTest(km=item["km_per_tile"]), sqlite3.connect(g.MOD / "Maps" / item["file"]) as db:
                rows = db.execute("SELECT ID,ResourceType FROM PlotResources ORDER BY ID").fetchall()
                self.assertEqual(len(item["resource_candidates"]), 67)
                self.assertEqual(len(rows), item["resource_placement"]["placed_count"])
                self.assertEqual(dict(sorted(Counter(resource for _, resource in rows).items())),
                    item["resource_placement"]["count_by_type"])
                city_plots = {row[0] for row in db.execute("SELECT Plot FROM Cities")}
                self.assertTrue(city_plots.isdisjoint(plot for plot, _ in rows))
                placed = {record["id"] for record in item["resource_candidates"]
                          if record["placement_status"] == "placed"}
                self.assertEqual(len(placed), len(rows))
                self.assertEqual({record["id"] for record in item["deferred_resources"]} & placed, set())
                resource_types = {resource for _, resource in rows}
                self.assertNotIn("RESOURCE_COPPER", resource_types)
                self.assertNotIn("RESOURCE_NITER", resource_types)
                for requirement in item["custom_content_required"]:
                    if requirement["kind"] == "resource":
                        self.assertNotIn(requirement["type"], resource_types)
                collisions = [record for record in item["resource_candidates"]
                              if record["placement_status"] == "same_tile_conflict"]
                for record in collisions:
                    winner = next(candidate for candidate in item["resource_candidates"]
                                  if candidate["id"] == record["conflict_with"])
                    self.assertEqual(record["plot"], winner["plot"])

    def test_rivers_are_connected_and_only_use_shared_land_edges(self):
        self.assertEqual({item["km_per_tile"]: len(item["rivers"]) for item in self.manifest},
                         {20: 10, 15: 10})
        for item in self.manifest:
            grid = g.HexGrid(item["km_per_tile"], item["width"], item["height"],
                             *item["grid_origin_projected_km"])
            with self.subTest(km=item["km_per_tile"]), sqlite3.connect(g.MOD / "Maps" / item["file"]) as db:
                land = {row[0] for row in db.execute(
                    "SELECT ID FROM Plots WHERE ContinentType='CONTINENT_ASIA'")}
                actual_edges = set()
                for plot, ne, west, nw in db.execute(
                        "SELECT ID,IsNEOfRiver,IsWOfRiver,IsNWOfRiver FROM PlotRivers"):
                    for side, present in (("NE", ne), ("W", west), ("NW", nw)):
                        if not present:
                            continue
                        neighbor = g.side_neighbour(grid, plot, side)
                        self.assertIsNotNone(neighbor)
                        self.assertIn(plot, land)
                        self.assertIn(neighbor, land)
                        actual_edges.add((plot, side))
                expected_edges = set()
                for river in item["rivers"]:
                    path = river["edge_path"]
                    self.assertEqual(len(path), river["edge_count"])
                    self.assertGreater(river["edge_count"], 0)
                    for edge in path:
                        key = (edge["plot"], edge["side"])
                        expected_edges.add(key)
                        self.assertIn(key, actual_edges)
                        self.assertEqual(edge["neighbor"],
                                         g.side_neighbour(grid, edge["plot"], edge["side"]))
                    for before, after in zip(path, path[1:]):
                        before_vertices = set(g.river_edge_geometry(
                            grid, (before["plot"], before["side"]), before["neighbor"]))
                        after_vertices = set(g.river_edge_geometry(
                            grid, (after["plot"], after["side"]), after["neighbor"]))
                        self.assertEqual(len(before_vertices & after_vertices), 1)
                self.assertEqual(actual_edges, expected_edges)

    def test_lakes_features_and_volcanoes_follow_draft_counts(self):
        expected = {
            20: {"land": 1082, "lake_tiles": 7, "forest": 401, "marsh": 27},
            15: {"land": 1906, "lake_tiles": 8, "forest": 747, "marsh": 33},
        }
        for item in self.manifest:
            km = item["km_per_tile"]
            features = item["terrain_features"]
            with self.subTest(km=km), sqlite3.connect(g.MOD / "Maps" / item["file"]) as db:
                self.assertEqual(item["land_tiles"], expected[km]["land"])
                self.assertEqual(len(features["lakes"]), 6)
                lake_tiles = [plot for lake in features["lakes"] for plot in lake["tile_ids"]]
                self.assertEqual(len(lake_tiles), expected[km]["lake_tiles"])
                self.assertEqual(len(set(lake_tiles)), len(lake_tiles))
                for plot in lake_tiles:
                    self.assertEqual(db.execute("SELECT TerrainType,ContinentType FROM Plots WHERE ID=?",
                                                 (plot,)).fetchone(), ("TERRAIN_COAST", ""))
                    self.assertIsNone(db.execute("SELECT ID FROM PlotOwners WHERE ID=?", (plot,)).fetchone())
                feature_rows = db.execute("SELECT ID,FeatureType FROM PlotFeatures ORDER BY ID").fetchall()
                feature_plots = {plot for plot, _ in feature_rows}
                resource_plots = {row[0] for row in db.execute("SELECT ID FROM PlotResources")}
                city_plots = {row[0] for row in db.execute("SELECT Plot FROM Cities")}
                self.assertTrue(feature_plots.isdisjoint(resource_plots))
                self.assertTrue(feature_plots.isdisjoint(city_plots))
                self.assertTrue(feature_plots.isdisjoint(lake_tiles))
                actual_counts = dict(sorted(Counter(feature for _, feature in feature_rows).items()))
                self.assertEqual(actual_counts, features["feature_counts"])
                self.assertEqual(actual_counts["FEATURE_FOREST"], expected[km]["forest"])
                self.assertEqual(actual_counts["FEATURE_MARSH"], expected[km]["marsh"])
                self.assertEqual(actual_counts["FEATURE_VOLCANO"], 5)
                self.assertAlmostEqual(features["forest"]["placed_percent"], 45.0, delta=0.05)
                self.assertGreaterEqual(features["marsh"]["placed_count"], 22)
                self.assertLessEqual(features["marsh"]["placed_count"], 33)
                for volcano in features["volcanoes"]:
                    terrain, feature = db.execute(
                        "SELECT p.TerrainType,f.FeatureType FROM Plots p JOIN PlotFeatures f ON f.ID=p.ID WHERE p.ID=?",
                        (volcano["plot"],)).fetchone()
                    self.assertTrue(terrain.endswith("_MOUNTAIN"))
                    self.assertEqual(feature, "FEATURE_VOLCANO")

    def test_close_castle_pairs_report(self):
        self.assertEqual({item["km_per_tile"]: len(item["close_city_pairs"]) for item in self.manifest},
                         {20: 0, 15: 0})
        for item in self.manifest:
            self.assertEqual(item["close_city_pairs"], g.close_city_pairs(item["cities"]))
            self.assertTrue(all(g.hex_distance((a["col"], a["row"]), (b["col"], b["row"]))
                                >= g.MIN_CITY_DISTANCE for i, a in enumerate(item["cities"])
                                for b in item["cities"][i + 1:]))

    def test_nudges_use_nearest_free_plot_of_the_original_owner(self):
        if not (g.CACHE / "ne_10m_admin_0_countries.zip").is_file():
            self.skipTest("Natural Earth cache absent")
        japan = g.read_shapes(g.CACHE / "ne_10m_admin_0_countries.zip",
                              "ne_10m_admin_0_countries", country="JPN")
        for item in self.manifest:
            with self.subTest(km=item["km_per_tile"]), sqlite3.connect(g.MOD / "Maps" / item["file"]) as db:
                grid = g.HexGrid.for_scale(item["km_per_tile"], japan)
                land = {row[0] for row in db.execute(
                    "SELECT ID FROM Plots WHERE ContinentType='CONTINENT_ASIA'")}
                passable = {row[0] for row in db.execute(
                    "SELECT ID FROM Plots WHERE ContinentType='CONTINENT_ASIA' AND IsImpassable=0")}
                original = g.choose_cities(grid, passable, item["starts"], data()["castles"])
                original_owners = {plot: g.SEATS[owner].slug for plot, owner, _, _ in
                                   g.assign_land(grid, land, original)}
                occupied = {city["plot"] for city in original}
                placed = []
                self.assertEqual([city["id"] for city in item["cities"]],
                                 [city["id"] for city in original])
                self.assertTrue(any("nudge" in city for city in item["cities"]))
                for city, before in zip(item["cities"], original):
                    self.assertEqual(city["displacement_km"], round(math.dist(
                        g.project(city["lon"], city["lat"]),
                        grid.centre(city["col"], city["row"])), 1))
                    if "nudge" in city:
                        self.assertFalse(city["capital"])
                        self.assertEqual(city["nudge"]["from_plot"], before["plot"])
                        self.assertEqual(city["nudge"]["to_plot"], city["plot"])
                        move = math.dist(grid.centre(before["col"], before["row"]),
                                         grid.centre(city["col"], city["row"]))
                        self.assertEqual(city["nudge"]["distance_km"], round(move, 1))
                        self.assertLessEqual(move, g.MAX_NUDGE_TILES * grid.km + 1e-8)
                        self.assertEqual(original_owners[city["plot"]], city["owner"])
                        self.assertIn(city["plot"], passable)
                        candidates = []
                        for plot in passable - occupied:
                            if original_owners[plot] != city["owner"]:
                                continue
                            col, row = plot % grid.width, plot // grid.width
                            if any(g.hex_distance((col, row), (earlier["col"], earlier["row"])) < 2
                                   for earlier in placed):
                                continue
                            xy = grid.centre(col, row)
                            distance = math.dist(grid.centre(before["col"], before["row"]), xy)
                            if distance <= g.MAX_NUDGE_TILES * grid.km + 1e-8:
                                candidates.append((distance, math.dist(
                                    g.project(city["lon"], city["lat"]), xy), plot))
                        self.assertEqual(city["plot"], min(candidates)[2])
                        occupied.remove(before["plot"])
                        occupied.add(city["plot"])
                    else:
                        self.assertEqual(city["plot"], before["plot"])
                    placed.append(city)

    def test_city_related_inserts_put_capitals_first(self):
        class Recorder:
            def __init__(self):
                self.calls = []

            def executemany(self, sql, rows):
                self.calls.append((sql, list(rows)))

        for item in self.manifest:
            with self.subTest(km=item["km_per_tile"]), sqlite3.connect(g.MOD / "Maps" / item["file"]) as db:
                allocations = db.execute("SELECT ID,Owner,CityOwner,CityWorking FROM PlotOwners").fetchall()
                recorder = Recorder()
                g.insert_historical_state(recorder, list(reversed(item["cities"])), allocations)
                capital_plots = [city["plot"] for city in item["cities"][:22]]
                self.assertEqual([row[1] for row in recorder.calls[0][1][:22]], capital_plots)
                self.assertEqual([row[0] for row in recorder.calls[1][1][:22]], capital_plots)
                self.assertEqual([row[0] for row in recorder.calls[2][1][:22]], capital_plots)
                self.assertIn("BUILDING_PALACE", recorder.calls[3][0])
                self.assertEqual([row[0] for row in recorder.calls[3][1]], capital_plots)
                self.assertEqual([row[0] for row in recorder.calls[4][1][:22]], capital_plots)

    def test_every_capital_and_only_capitals_have_a_palace(self):
        for item in self.manifest:
            with self.subTest(km=item["km_per_tile"]), sqlite3.connect(g.MOD / "Maps" / item["file"]) as db:
                palaces = {plot for (plot,) in db.execute(
                    "SELECT Plot FROM Buildings WHERE BuildingType='BUILDING_PALACE'")}
                self.assertEqual(palaces, {city["plot"] for city in item["cities"] if city["capital"]})
                self.assertEqual(len(palaces), 22)

    def test_hex_distance_matches_neighbours(self):
        grid = g.HexGrid(40, 20, 20, 0.0, 0.0)
        for col, row in ((5, 4), (5, 5)):
            for other in grid.neighbours(col, row):
                self.assertEqual(g.hex_distance((col, row), other), 1)
        self.assertEqual(g.hex_distance((0, 0), (3, 0)), 3)
        self.assertEqual(g.hex_distance((0, 0), (0, 2)), 2)

    def test_config_and_scenarios_cover_all_scales(self):
        config = ET.parse(g.MOD / "Config" / "JAC_JapanMapScale_Config.xml").getroot()
        maps = config.findall("./Maps/Row")
        self.assertEqual(len(maps), 4)
        self.assertEqual({row.attrib["Domain"] for row in maps}, {"Maps:JAC_Banseiki"})
        self.assertEqual(config.findall("./DomainValueUnions/Row"), [])
        ruleset = ET.parse(g.MOD.parent / "JAC_PoC_BanseikiRuleset" / "Config/JAC_Banseiki_Config.xml").getroot()
        self.assertEqual([row.attrib for row in ruleset.findall("./DomainValueUnions/Row")],
                         [{"Domain": "Maps:JAC_Banseiki", "OtherDomain": "Maps:Expansion2Maps"}])
        self.assertEqual([row.attrib["Domain"] for row in ruleset.findall("./RulesetDomainOverrides/Row")
                          if row.attrib["ParameterId"] == "Map"], ["Maps:JAC_Banseiki"])
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
        self.assertEqual({row.attrib["id"] for row in mod.findall("./Dependencies/Mod")},
                         {"4873eb62-8ccc-4574-b784-dda455e74e68",
                          "ba3a1c58-9287-4654-a22f-85f9e6841e9b",
                          "bd7a82c5-6b38-4b69-a7f5-8dc9af534249"})
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
            expected = re.findall(r"\{(\d+),(\d+),(\d+)\}",
                                  scenario["steps"][3]["code"].split("local expected=", 1)[1]
                                  .split("; local leaders=", 1)[0])
            self.assertEqual([int(count) for _, _, count in expected],
                             [sum(city["owner"] == seat.slug for city in item["cities"])
                              for seat in g.SEATS])
            self.assertIn("if totalCities~=56", scenario["steps"][3]["code"])
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
        no_settlers = [action for action in actions
                       if action.findtext("File") == "Data/JAC_Japan_NoSettlers.xml"]
        self.assertEqual(len(no_settlers), 2)
        for action in no_settlers:
            selected = criteria[action.attrib["criteria"]]
            self.assertIn(selected, {f"{{403be889-1455-4de7-967b-ad54c0ff3431}}Maps/JAC_Japan_{km}km.Civ6Map"
                                     for km in g.SCALES})
        removal = ET.parse(g.MOD / "Data/JAC_Japan_NoSettlers.xml").getroot()
        self.assertEqual(removal.find("./MajorStartingUnits/Delete").attrib, {"Unit": "UNIT_SETTLER"})

    def test_banseiki_city_range_is_gameplay_only_and_ruleset_scoped(self):
        ruleset_mod = g.MOD.parent / "JAC_PoC_BanseikiRuleset"
        mod = ET.parse(ruleset_mod / "JAC_PoC_BanseikiRuleset.modinfo").getroot()
        criterion = mod.find("./ActionCriteria/Criteria[@id='JAC_Banseiki_Ruleset']")
        self.assertEqual(criterion.findtext("RuleSetInUse"), "RULESET_JAC_BANSEIKI")
        action = mod.find("./InGameActions/UpdateDatabase[@id='JAC_Banseiki_CityRange']")
        self.assertIsNotNone(action)
        self.assertEqual(action.attrib["criteria"], criterion.attrib["id"])
        self.assertEqual(action.findtext("File"), "Data/JAC_Banseiki_CityRange.sql")
        self.assertIn(action.findtext("File"),
                      [file.text for file in mod.findall("./Files/File")])
        self.assertIsNone(mod.find("./FrontEndActions/UpdateDatabase[@id='JAC_Banseiki_CityRange']"))
        sql = (ruleset_mod / action.findtext("File")).read_text(encoding="utf-8")
        with sqlite3.connect(":memory:") as db:
            db.execute("CREATE TABLE GlobalParameters (Name TEXT PRIMARY KEY, Value TEXT)")
            db.executemany("INSERT INTO GlobalParameters VALUES (?, ?)",
                           (("CITY_MIN_RANGE", "3"), ("CITY_MIN_DISTANCE_FOR_INCURSION", "8")))
            db.executescript(sql)
            self.assertEqual(db.execute("SELECT Value FROM GlobalParameters WHERE Name='CITY_MIN_RANGE'")
                             .fetchone(), ("1",))
            self.assertEqual(db.execute("SELECT Value FROM GlobalParameters WHERE Name='CITY_MIN_DISTANCE_FOR_INCURSION'")
                             .fetchone(), ("8",))
        official = (g.OFFICIAL.parents[2] / "Gameplay/Data/GlobalParameters.xml")
        if official.exists():
            rows = ET.parse(official).getroot().findall(".//*[@Name='CITY_MIN_RANGE']")
            self.assertEqual([row.attrib["Value"] for row in rows], ["3"])

    def test_domain_passage_and_loyalty_are_limited_to_all_four_japan_maps(self):
        config = ET.parse(g.MOD / "Config/JAC_JapanMapScale_Config.xml").getroot()
        map_files = {row.attrib["File"] for row in config.findall("./Maps/Row")}
        mod = ET.parse(g.MOD / "JAC_PoC_JapanMapScale.modinfo").getroot()
        criteria = {row.attrib["id"]: row for row in mod.findall("./ActionCriteria/Criteria")}
        rules_actions = [row for row in mod.findall("./InGameActions/UpdateDatabase")
                         if row.findtext("File") == "Data/JAC_Japan_MapRules.xml"]
        self.assertEqual(len(rules_actions), 4)
        self.assertEqual({criteria[row.attrib["criteria"]].findtext("./ConfigurationValueMatches/Value")
                          for row in rules_actions}, map_files)
        for row in rules_actions:
            criterion = criteria[row.attrib["criteria"]]
            self.assertEqual(criterion.findtext("RuleSetInUse"), "RULESET_JAC_BANSEIKI")
            self.assertEqual(criterion.findtext("./ConfigurationValueMatches/Group"), "Map")
            self.assertEqual(criterion.findtext("./ConfigurationValueMatches/ConfigurationId"), "MAP_SCRIPT")
        self.assertIn("Data/JAC_Japan_MapRules.xml",
                      {row.text for row in mod.findall("./Files/File")})

        rules = ET.parse(g.MOD / "Data/JAC_Japan_MapRules.xml").getroot()
        self.assertEqual(rules.find("./TypeTags/Row").attrib,
                         {"Type": "ABILITY_JAC_DOMAIN_CROSSING", "Tag": "CLASS_ALL_UNITS"})
        self.assertEqual(rules.find("./UnitAbilityModifiers/Row").attrib["ModifierId"],
                         "MOD_ENTER_FOREIGN_LANDS")
        self.assertEqual(rules.find("./Modifiers/Row").attrib["ModifierType"],
                         "MODIFIER_PLAYER_UNITS_GRANT_ABILITY")
        self.assertEqual(rules.find("./ModifierArguments/Row").attrib["Value"],
                         "ABILITY_JAC_DOMAIN_CROSSING")
        self.assertEqual({(row.tag, row.attrib["ModifierId"])
                          for row in rules.findall("./TraitModifiers/*")},
                         {("Row", "JAC_DOMAIN_CROSSING_ALL_UNITS"),
                          ("Delete", "STANDARD_DIPLOMATIC_INCURSION")})
        loyalty = rules.find("./GlobalParameters/Update")
        self.assertEqual(loyalty.find("Where").attrib["Name"],
                         "LOYALTY_PER_TURN_FROM_NEARBY_CITIZEN_PRESSURE_MAX_LOYALTY")
        self.assertEqual(loyalty.findtext("Set/Value"), "5")
        source = g.OFFICIAL.parents[4] / "DLC/Expansion2/Data/Expansion2_GlobalParameters.xml"
        if source.is_file():
            baseline = ET.parse(source).getroot()
            matches = [row.attrib["Value"] for row in baseline.findall("./GlobalParameters/Replace")
                       if row.attrib["Name"] == loyalty.find("Where").attrib["Name"]]
            self.assertEqual(matches, ["20"])


if __name__ == "__main__":
    unittest.main()
