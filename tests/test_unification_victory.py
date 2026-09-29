"""Static gates for the native castle-unification victory and map registry."""

from __future__ import annotations

import json
from pathlib import Path
import sqlite3
import unittest
import xml.etree.ElementTree as ET

from tools.generate_unification_victory import scenario, sql
from tools.japan_map_scale.registered_castles import MARKER, verify_or_stamp


ROOT = Path(__file__).resolve().parents[1]
MAP_MOD = ROOT / "poc/JAC_PoC_JapanMapScale"
TREE_MOD = ROOT / "poc/JAC_PoC_SengokuTrees"
VICTORY = "VICTORY_JAC_SENGOKU_UNIFICATION"


class UnificationVictoryTests(unittest.TestCase):
    def test_map_cities_are_the_registry_and_settler_maps_have_none(self):
        manifest = json.loads((MAP_MOD / "Maps/manifest.json").read_text(encoding="utf-8"))
        counts = set()
        for item in manifest:
            with self.subTest(scale=item["km_per_tile"]):
                with sqlite3.connect(MAP_MOD / "Maps" / item["file"]) as db:
                    city_plots = {row[0] for row in db.execute("SELECT Plot FROM Cities")}
                    marker_plots = {row[0] for row in db.execute(
                        "SELECT Plot FROM Buildings WHERE BuildingType=?", (MARKER,))}
                    self.assertEqual(marker_plots, city_plots)
                    self.assertEqual(city_plots, {city["plot"] for city in item["cities"]})
                    self.assertTrue({"RYUKYU", "AINU"} <= {city["owner"] for city in item["cities"]})
                    self.assertEqual(item["bytes"], (MAP_MOD / "Maps" / item["file"]).stat().st_size)
                    self.assertTrue(city_plots)
                    counts.add(len(city_plots))
                with sqlite3.connect(MAP_MOD / "Maps" / item["settler_file"]) as db:
                    self.assertEqual(db.execute("SELECT COUNT(*) FROM Cities").fetchone()[0], 0)
                    self.assertEqual(db.execute(
                        "SELECT COUNT(*) FROM Buildings WHERE BuildingType=?", (MARKER,)).fetchone()[0], 0)
        self.assertEqual(len(counts), 1)
        self.assertEqual(verify_or_stamp(), counts.pop())

        data = ET.parse(MAP_MOD / "Data/JAC_RegisteredCastle.xml").getroot()
        self.assertEqual(data.find("./Types/Row").attrib,
                         {"Type": MARKER, "Kind": "KIND_BUILDING"})
        self.assertEqual(data.find("./Buildings/Row").get("InternalOnly"), "1")
        self.assertEqual(data.find("./Buildings/Row").get("PrereqDistrict"), "DISTRICT_CITY_CENTER")
        self.assertEqual(data.find("./Buildings/Row").get("Name"),
                         "LOC_BUILDING_JAC_REGISTERED_CASTLE_NAME")
        modinfo = ET.parse(MAP_MOD / "JAC_PoC_JapanMapScale.modinfo").getroot()
        self.assertIn("Data/JAC_RegisteredCastle.xml", [row.text for row in modinfo.findall("./Files/File")])
        self.assertEqual([row.get("criteria") for row in modinfo.findall("./InGameActions/UpdateDatabase")
                          if row.findtext("File") == "Data/JAC_RegisteredCastle.xml"],
                         ["JAC_JapanMapScale_Banseiki"])
        self.assertIn("Text/JAC_RegisteredCastle_Text.xml", [row.text for row in modinfo.findall("./InGameActions/UpdateText/File")])
        marker_text = ET.parse(MAP_MOD / "Text/JAC_RegisteredCastle_Text.xml").getroot()
        rows = marker_text.findall("./LocalizedText/Row")
        self.assertEqual({row.get("Language") for row in rows},
                         {"ja_JP", "en_US", "ru_RU"})
        self.assertTrue(all(row.get("Tag") == "LOC_BUILDING_JAC_REGISTERED_CASTLE_NAME" and
                            row.findtext("Text") for row in rows))

    def test_generated_native_requirement_chain_and_threshold(self):
        count = verify_or_stamp()
        script = (TREE_MOD / "Data/JAC_Sengoku_Unification.sql").read_text(encoding="utf-8")
        self.assertEqual(script, sql(count))
        with sqlite3.connect(":memory:") as db:
            db.executescript("""
                CREATE TABLE Types (Type TEXT PRIMARY KEY, Kind TEXT);
                CREATE TABLE Victories (VictoryType TEXT PRIMARY KEY, Name TEXT, Description TEXT,
                    Blurb TEXT, RequirementSetId TEXT, EnabledByDefault INT, OneMoreTurn INT, Icon TEXT);
                CREATE TABLE RequirementSets (RequirementSetId TEXT PRIMARY KEY, RequirementSetType TEXT);
                CREATE TABLE RequirementSetRequirements (RequirementSetId TEXT, RequirementId TEXT);
                CREATE TABLE Requirements (RequirementId TEXT PRIMARY KEY, RequirementType TEXT);
                CREATE TABLE RequirementArguments (RequirementId TEXT, Name TEXT, Value TEXT);
                CREATE TABLE RequirementStrings (RequirementId TEXT, Context TEXT, Text TEXT);
            """)
            db.executescript(script)
            self.assertEqual(db.execute("SELECT Type,Kind FROM Types").fetchall(),
                             [(VICTORY, "KIND_VICTORY")])
            self.assertEqual(db.execute("SELECT RequirementSetId,EnabledByDefault,OneMoreTurn FROM Victories").fetchone(),
                             ("JAC_UNIFICATION_VICTORY_REQUIREMENTS", 1, 0))
            requirements = dict(db.execute("SELECT RequirementId,RequirementType FROM Requirements"))
            self.assertEqual(requirements, {
                "JAC_UNIFICATION_TEAM_MEMBER": "REQUIREMENT_COLLECTION_ANY_MET",
                "JAC_UNIFICATION_CASTLE_COUNT": "REQUIREMENT_COLLECTION_COUNT_EQUALS",
                "JAC_UNIFICATION_CITY_MARKER": "REQUIREMENT_CITY_HAS_BUILDING",
            })
            args = {(req, name): value for req, name, value in db.execute(
                "SELECT RequirementId,Name,Value FROM RequirementArguments")}
            self.assertEqual(args["JAC_UNIFICATION_TEAM_MEMBER", "CollectionType"], "COLLECTION_TEAM_PLAYERS")
            self.assertEqual(args["JAC_UNIFICATION_TEAM_MEMBER", "RequirementSetId"],
                             "JAC_UNIFICATION_PLAYER_REQUIREMENTS")
            self.assertEqual(args["JAC_UNIFICATION_CASTLE_COUNT", "CollectionType"], "COLLECTION_PLAYER_CITIES")
            self.assertEqual(args["JAC_UNIFICATION_CASTLE_COUNT", "RequirementSetId"],
                             "JAC_UNIFICATION_CITY_REQUIREMENTS")
            self.assertEqual(int(args["JAC_UNIFICATION_CASTLE_COUNT", "Count"]), count)
            self.assertEqual(args["JAC_UNIFICATION_CITY_MARKER", "BuildingType"], MARKER)
            self.assertEqual(set(db.execute("SELECT RequirementSetId,RequirementId FROM RequirementSetRequirements")), {
                ("JAC_UNIFICATION_VICTORY_REQUIREMENTS", "JAC_UNIFICATION_TEAM_MEMBER"),
                ("JAC_UNIFICATION_PLAYER_REQUIREMENTS", "JAC_UNIFICATION_CASTLE_COUNT"),
                ("JAC_UNIFICATION_CITY_REQUIREMENTS", "JAC_UNIFICATION_CITY_MARKER"),
            })

    def test_victory_is_in_sengoku_domain_and_visible_in_official_rankings(self):
        config = ET.parse(TREE_MOD / "Config/JAC_Sengoku_Victories.xml").getroot()
        self.assertEqual([(row.get("Domain"), row.get("VictoryType"), row.get("EnabledByDefault"))
                          for row in config.findall("./Victories/Row")],
                         [("JAC_Sengoku_Victories", VICTORY, "true")])
        text = ET.parse(TREE_MOD / "Text/JAC_Sengoku_Unification_Text.xml").getroot()
        tags = {}
        for row in text.findall("./LocalizedText/Row"):
            tags.setdefault(row.get("Tag"), set()).add(row.get("Language"))
            self.assertTrue(row.findtext("Text"))
        self.assertEqual(set(tags), {f"LOC_VICTORY_JAC_UNIFICATION_{part}" for part in
                                     ("NAME", "DESCRIPTION", "BLURB", "PROGRESS")})
        self.assertTrue(all(langs == {"ja_JP", "en_US", "ru_RU"} for langs in tags.values()))
        modinfo = ET.parse(TREE_MOD / "JAC_PoC_SengokuTrees.modinfo").getroot()
        self.assertEqual([row.text for row in modinfo.findall("./FrontEndActions/UpdateText/File")],
                         ["Text/JAC_Sengoku_Unification_Text.xml"])

    def test_live_scenario_checks_negative_reload_and_positive_result(self):
        path = ROOT / "tools/civ6-autotest/scenarios/unification-victory.json"
        data = json.loads(path.read_text(encoding="utf-8"))
        self.assertEqual(data, scenario(verify_or_stamp()))
        actions = [step["action"] for step in data["steps"]]
        self.assertEqual(actions[:4], ["enable_mods", "configure", "ui_lua", "start_game"])
        self.assertEqual(actions.count("save"), 1)
        self.assertEqual(actions.count("load"), 1)
        self.assertEqual(actions[-1], "ui_lua")
        negative = [step for step in data["steps"] if step["action"] == "ui_lua" and
                    "victory with one castle missing" in step["code"]]
        self.assertEqual(len(negative), 3)
        self.assertTrue(any(step["action"] == "wait_for" and
                            VICTORY in step["condition"].get("code", "") for step in data["steps"]))


if __name__ == "__main__":
    unittest.main()
