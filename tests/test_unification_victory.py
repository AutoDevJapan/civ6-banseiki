"""Static gates for the native castle-unification victory and map registry."""

from __future__ import annotations

from contextlib import closing
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
import xml.etree.ElementTree as ET

from tools.generate_unification_victory import registry_scenario, scenario, sql
from tools.japan_map_scale.registered_castles import MARKER, verify_or_stamp
import unification_test_map as test_map  # on sys.path via generate_unification_victory


ROOT = Path(__file__).resolve().parents[1]
MAP_MOD = ROOT / "poc/JAC_PoC_JapanMapScale"
TREE_MOD = ROOT / "poc/JAC_PoC_SengokuTrees"
TEST_MOD = ROOT / "poc/JAC_Test_UnificationMap"
VICTORY = "VICTORY_JAC_SENGOKU_UNIFICATION"
SCHEMA = """
    CREATE TABLE Types (Type TEXT PRIMARY KEY, Kind TEXT);
    CREATE TABLE Victories (VictoryType TEXT PRIMARY KEY, Name TEXT, Description TEXT,
        Blurb TEXT, RequirementSetId TEXT, EnabledByDefault INT, OneMoreTurn INT, Icon TEXT);
    CREATE TABLE RequirementSets (RequirementSetId TEXT PRIMARY KEY, RequirementSetType TEXT);
    CREATE TABLE RequirementSetRequirements (RequirementSetId TEXT, RequirementId TEXT,
        PRIMARY KEY (RequirementSetId, RequirementId));
    CREATE TABLE Requirements (RequirementId TEXT PRIMARY KEY, RequirementType TEXT, Inverse INT);
    CREATE TABLE RequirementArguments (RequirementId TEXT, Name TEXT, Value TEXT,
        PRIMARY KEY (RequirementId, Name));
    CREATE TABLE RequirementStrings (RequirementId TEXT, Context TEXT, Text TEXT);
"""


class RequirementModel:
    """Evaluate the generated rows with the official requirement semantics we rely on."""

    def __init__(self, db: sqlite3.Connection):
        self.sets = dict(db.execute("SELECT RequirementSetId,RequirementSetType FROM RequirementSets"))
        self.members: dict[str, list[str]] = {}
        for set_id, req in db.execute("SELECT RequirementSetId,RequirementId FROM RequirementSetRequirements"):
            self.members.setdefault(set_id, []).append(req)
        self.reqs = {req: (kind, bool(inverse)) for req, kind, inverse in
                     db.execute("SELECT RequirementId,RequirementType,Inverse FROM Requirements")}
        self.args: dict[str, dict[str, str]] = {}
        for req, name, value in db.execute("SELECT RequirementId,Name,Value FROM RequirementArguments"):
            self.args.setdefault(req, {})[name] = value
        self.root = db.execute("SELECT RequirementSetId FROM Victories WHERE VictoryType=?",
                               (VICTORY,)).fetchone()[0]

    def wins(self, team: int, cities: list[tuple[int, bool]], domination: bool = False) -> bool:
        self.world = cities, domination
        return self.test_set(self.root, team, None)

    def test_set(self, set_id: str, team: int, city) -> bool:
        results = [self.test(req, team, city) for req in self.members[set_id]]
        return {"REQUIREMENTSET_TEST_ALL": all, "REQUIREMENTSET_TEST_ANY": any}[self.sets[set_id]](results)

    def test(self, req: str, team: int, city) -> bool:
        kind, inverse = self.reqs[req]
        args = self.args.get(req, {})
        cities, domination = self.world
        if kind == "REQUIREMENT_REQUIREMENTSET_IS_MET":
            result = self.test_set(args["RequirementSetId"], team, city)
        elif kind.startswith("REQUIREMENT_COLLECTION_"):
            self.assertion(args["CollectionType"] == "COLLECTION_ALL_CITIES", req)
            met = [self.test_set(args["RequirementSetId"], team, item) for item in cities]
            result = {"REQUIREMENT_COLLECTION_ALL_MET": lambda: all(met),
                      "REQUIREMENT_COLLECTION_COUNT_ATLEAST": lambda: sum(met) >= int(args["Count"]),
                      "REQUIREMENT_COLLECTION_COUNT_EQUALS": lambda: sum(met) == int(args["Count"])}[kind]()
        elif kind == "REQUIREMENT_CITY_HAS_BUILDING":
            self.assertion(args["BuildingType"] == MARKER, req)
            result = city[1]
        elif kind == "REQUIREMENT_PLAYER_OWNS_OBJECT":
            result = city[0] == team
        elif kind == "REQUIREMENT_TEAM_DOMINATION_VICTORY":
            result = domination
        else:
            raise AssertionError(f"Unmodelled requirement {req}: {kind}")
        return result != inverse

    @staticmethod
    def assertion(condition: bool, req: str) -> None:
        if not condition:
            raise AssertionError(f"Unexpected arguments on {req}")


def generated_model() -> RequirementModel:
    db = sqlite3.connect(":memory:")
    db.executescript(SCHEMA)
    db.executescript(sql())
    return RequirementModel(db)


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
        self.assertEqual(counts, {56})
        self.assertEqual(verify_or_stamp(), 56)

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

    def test_checked_in_sql_is_generated_without_a_castle_count(self):
        script = (TREE_MOD / "Data/JAC_Sengoku_Unification.sql").read_text(encoding="utf-8")
        self.assertEqual(script, sql())
        model = generated_model()
        counts = {req: args["Count"] for req, args in model.args.items() if "Count" in args}
        # Only existence (>=1) and absence (==0) tests: nothing depends on 56 or the test map's 4.
        self.assertEqual(counts, {"JAC_UNIFICATION_CASTLES_EXIST": "1", "JAC_UNIFICATION_NO_CASTLES": "0"})
        self.assertEqual(model.sets[model.root], "REQUIREMENTSET_TEST_ANY")
        self.assertEqual(model.members[model.root], ["JAC_UNIFICATION_CASTLES", "JAC_UNIFICATION_CAPITALS"])
        self.assertEqual(model.reqs["JAC_UNIFICATION_ALL_CAPITALS"],
                         ("REQUIREMENT_TEAM_DOMINATION_VICTORY", False))
        self.assertEqual(model.reqs["JAC_UNIFICATION_CITY_UNMARKED"], ("REQUIREMENT_CITY_HAS_BUILDING", True))
        with sqlite3.connect(":memory:") as db:
            db.executescript(SCHEMA)
            db.executescript(script)
            self.assertEqual(db.execute("SELECT Type,Kind FROM Types").fetchall(), [(VICTORY, "KIND_VICTORY")])
            self.assertEqual(db.execute("SELECT RequirementSetId,EnabledByDefault,OneMoreTurn FROM Victories").fetchone(),
                             ("JAC_UNIFICATION_VICTORY_REQUIREMENTS", 1, 0))
            self.assertEqual(dict(db.execute("SELECT RequirementId,Text FROM RequirementStrings")), {
                "JAC_UNIFICATION_CASTLES": "LOC_VICTORY_JAC_UNIFICATION_PROGRESS",
                "JAC_UNIFICATION_CAPITALS": "LOC_VICTORY_JAC_UNIFICATION_CAPITALS_PROGRESS",
            })

    def test_victory_follows_owned_castles_razes_and_settler_capitals(self):
        model = generated_model()
        oda, date = 0, 1
        four = [(oda, True), (oda, True), (date, True), (date, True)]
        self.assertFalse(model.wins(oda, four))
        self.assertFalse(model.wins(oda, four[:2] + [(oda, True), (date, True)]), "one castle missing")
        self.assertTrue(model.wins(oda, [(oda, True)] * 4))
        # Razed castle: the city and its marker are gone, so 3 of the original 4 suffice.
        self.assertTrue(model.wins(oda, [(oda, True)] * 3))
        # Later-founded cities never carry the marker and do not block or count.
        self.assertTrue(model.wins(oda, [(oda, True)] * 3 + [(date, False), (2, False)]))
        self.assertFalse(model.wins(date, [(oda, True)] * 3 + [(date, False)]))
        # Historical maps: capitals are castles, so domination alone never decides there.
        self.assertFalse(model.wins(oda, four[:3] + [(date, True)], domination=True))
        # Settler starts / ordinary maps: no registered castle -> official domination rule.
        self.assertFalse(model.wins(oda, [(oda, False), (date, False)]))
        self.assertFalse(model.wins(oda, [(oda, False)]))
        self.assertTrue(model.wins(oda, [(oda, False), (date, False)], domination=True))

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
                                     ("NAME", "DESCRIPTION", "BLURB", "PROGRESS", "CAPITALS_PROGRESS")})
        self.assertTrue(all(langs == {"ja_JP", "en_US", "ru_RU"} for langs in tags.values()))
        modinfo = ET.parse(TREE_MOD / "JAC_PoC_SengokuTrees.modinfo").getroot()
        self.assertEqual([row.text for row in modinfo.findall("./FrontEndActions/UpdateText/File")],
                         ["Text/JAC_Sengoku_Unification_Text.xml"])

    def test_test_map_registry_comes_from_map_data_and_stays_in_a_test_only_mod(self):
        path = TEST_MOD / "Maps" / f"{test_map.MAP_NAME}.Civ6Map"
        castles = test_map.castle_rows(path)
        self.assertEqual(sorted(castles.values()), [0, 0, 1, 1])
        with closing(sqlite3.connect(path)) as db:
            self.assertEqual({row[0] for row in db.execute("SELECT Plot FROM Cities")}, set(castles))
            self.assertEqual(db.execute("SELECT ID,LeaderType FROM Players ORDER BY ID").fetchall(),
                             [(0, "LEADER_JAC_ODA_NOBUHIDE"), (1, "LEADER_JAC_DATE_TANEMUNE"),
                              (2, "LEADER_JAC_ASHINA_MORIKIYO")])
            self.assertEqual(db.execute("SELECT MapSizeType FROM Map").fetchone()[0], test_map.MAP_SIZE)
            self.assertEqual(db.execute("PRAGMA integrity_check").fetchone()[0], "ok")
            capitals = {row[0] for row in db.execute(
                "SELECT Plot FROM Buildings WHERE BuildingType='BUILDING_PALACE'")}
        self.assertEqual({castles[plot] for plot in capitals}, {0, 1})
        if test_map.OFFICIAL.is_file():  # regenerate with the same tooling and compare
            with tempfile.TemporaryDirectory(dir=ROOT) as folder:
                fresh = Path(folder) / path.name
                test_map.build(fresh)
                self.assertEqual(test_map.castle_rows(fresh), castles)
                with closing(sqlite3.connect(fresh)) as a, closing(sqlite3.connect(path)) as b:
                    for table in ("Plots", "PlotOwners", "Units", "StartPositions", "Buildings"):
                        query = f"SELECT * FROM {table} ORDER BY 1, 2"
                        self.assertEqual(a.execute(query).fetchall(), b.execute(query).fetchall(), table)

        modinfo = ET.parse(TEST_MOD / "JAC_Test_UnificationMap.modinfo").getroot()
        self.assertEqual(modinfo.get("id"), test_map.TEST_MOD_ID)
        self.assertIn("TEST", modinfo.findtext("./LocalizedText/Text/en_US"))
        self.assertEqual({row.text for row in modinfo.findall("./Files/File")}, {
            "Config/JAC_Test_UnificationMap_Config.xml", "Text/JAC_Test_UnificationMap_Text.xml",
            f"Maps/{test_map.MAP_NAME}.Civ6Map", f"Maps/{test_map.MAP_NAME}.lua"})
        config = ET.parse(TEST_MOD / "Config/JAC_Test_UnificationMap_Config.xml").getroot()
        self.assertEqual([row.get("File") for row in config.findall("./Maps/Row")], [test_map.MAP_FILE])
        self.assertEqual([row.get("MapSizeType") for row in config.findall("./MapSizes/Row")], [test_map.MAP_SIZE])
        self.assertEqual({row.get("LeaderType") for row in config.findall("./MapLeaders/Row")},
                         {"LEADER_JAC_ODA_NOBUHIDE", "LEADER_JAC_DATE_TANEMUNE", "LEADER_JAC_ASHINA_MORIKIYO"})
        # The player's map mod never lists the test map.
        self.assertNotIn(test_map.MAP_NAME, (MAP_MOD / "Config/JAC_JapanMapScale_Config.xml").read_text(encoding="utf-8"))
        text = ET.parse(TEST_MOD / "Text/JAC_Test_UnificationMap_Text.xml").getroot()
        names = {row.get("Language"): row.findtext("Text") for row in text.findall("./LocalizedText/Row")
                 if row.get("Tag") == "LOC_JAC_TEST_UNIFICATION_MAP_NAME"}
        self.assertEqual(set(names), {"ja_JP", "en_US", "ru_RU"})
        self.assertIn("テスト専用", names["ja_JP"])

    def test_live_scenarios_use_the_test_map_and_a_light_real_map_check(self):
        folder = ROOT / "tools/civ6-autotest/scenarios"
        data = json.loads((folder / "unification-victory.json").read_text(encoding="utf-8"))
        self.assertEqual(data, scenario())
        steps = data["steps"]
        actions = [step["action"] for step in steps]
        self.assertEqual(actions[:4], ["enable_mods", "configure", "ui_lua", "start_game"])
        self.assertIn(test_map.TEST_MOD_ID, steps[0]["ids"])
        self.assertEqual(steps[1]["map_file"], test_map.MAP_FILE)
        self.assertEqual(len(steps[1]["players"]), 3)
        # No mass transfer: at most the raze fallback and the last castle.
        self.assertEqual(sum("TransferCity(" in step.get("code", "") for step in steps), 2)
        self.assertEqual(actions.count("save"), 1)
        self.assertEqual(actions.count("load"), 1)
        negative = [index for index, step in enumerate(steps) if
                    "victory with one castle missing" in step.get("code", "")]
        self.assertEqual(len(negative), 3)
        save, load = actions.index("save"), actions.index("load")
        self.assertTrue(negative[0] < actions.index("autoplay") < negative[1] < save < load < negative[2])
        final = next(index for index, step in enumerate(steps)
                     if "transferred the last castle" in step.get("code", ""))
        # The game ends during that turn, so autoplay is started from Lua and the winner is awaited (live 2026-09-29).
        self.assertEqual(actions[final + 1:final + 3], ["ui_lua", "wait_for"])
        self.assertIn("AutoplayManager.SetActive(true)", steps[final + 1]["code"])
        self.assertIn(VICTORY, steps[final + 2]["condition"]["code"])
        self.assertIn("raze not verified", steps[-1]["code"])

        registry = json.loads((folder / "unification-registry-15km.json").read_text(encoding="utf-8"))
        self.assertEqual(registry, registry_scenario(verify_or_stamp()))
        self.assertEqual(registry["steps"][1]["map_file"],
                         "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map")
        self.assertIn("total~=56", registry["steps"][-1]["code"])
        self.assertNotIn("TransferCity", json.dumps(registry))
        self.assertNotIn("autoplay", [step["action"] for step in registry["steps"]])


if __name__ == "__main__":
    unittest.main()
