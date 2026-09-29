"""Static gates for the native castle-unification victory and map registry."""

from __future__ import annotations

from contextlib import closing
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
import xml.etree.ElementTree as ET

from tools.generate_unification_victory import (
    CASTLE_FILES, CASTLE_SET, COUNTED_SET, REBUILT, castle_sql, castles, registry_scenario, scenario, sql, verify_registry)
import unification_test_map as test_map  # on sys.path via generate_unification_victory


ROOT = Path(__file__).resolve().parents[1]
MAP_MOD = ROOT / "poc/JAC_PoC_JapanMapScale"
TREE_MOD = ROOT / "poc/JAC_PoC_SengokuTrees"
TEST_MOD = ROOT / "poc/JAC_Test_UnificationMap"
VICTORY = "VICTORY_JAC_SENGOKU_UNIFICATION"
SCHEMA = """
    CREATE TABLE Types (Type TEXT PRIMARY KEY, Kind TEXT);
    CREATE TABLE Buildings (BuildingType TEXT PRIMARY KEY, Name TEXT NOT NULL, PrereqDistrict TEXT,
        Cost INT NOT NULL, InternalOnly INT NOT NULL DEFAULT 0);
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

    def wins(self, team: int, cities: list[tuple], domination: bool = False) -> bool:
        """cities: (owner, x, y) or (owner, x, y, rebuilt) where rebuilt means the city holds the marker."""
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
        elif kind == "REQUIREMENT_CITY_LOCATION_MATCHES":
            result = city[1:] == (int(args["X"]), int(args["Y"]))
        elif kind == "REQUIREMENT_CITY_HAS_BUILDING":
            self.assertion(args == {"BuildingType": REBUILT}, req)
            result = len(city) > 3 and city[3]
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


def generated_model(*castle_scripts: str) -> RequirementModel:
    """Victory SQL, then the castle SQL of the map in use (modinfo LoadOrder 200 -> 300)."""
    db = sqlite3.connect(":memory:")
    db.executescript(SCHEMA)
    db.executescript(sql())
    for script in castle_scripts:
        db.executescript(script)
    return RequirementModel(db)


# Four castles as on the test map: Oda holds the first two, Date the rest.
FOUR = [(6, 5), (6, 14), (25, 5), (25, 14)]
FOUR_SQL = castle_sql("T", [(x, y, 0) for x, y in FOUR])


class UnificationVictoryTests(unittest.TestCase):
    def test_map_cities_are_the_registry_and_settler_maps_have_none(self):
        manifest = json.loads((MAP_MOD / "Maps/manifest.json").read_text(encoding="utf-8"))
        for item in manifest:
            with self.subTest(scale=item["km_per_tile"]):
                path = MAP_MOD / "Maps" / item["file"]
                with closing(sqlite3.connect(path)) as db:
                    width = db.execute("SELECT Width FROM Map").fetchone()[0]
                    self.assertEqual(db.execute("SELECT COUNT(*) FROM Buildings WHERE BuildingType "
                                                "NOT IN ('BUILDING_PALACE')").fetchone()[0], 0)
                plots = {city["plot"] for city in item["cities"]}
                self.assertEqual({y * width + x for x, y, _ in castles(path)}, plots)
                self.assertTrue({"RYUKYU", "AINU"} <= {city["owner"] for city in item["cities"]})
                self.assertEqual(item["bytes"], path.stat().st_size)
                with closing(sqlite3.connect(MAP_MOD / "Maps" / item["settler_file"])) as db:
                    self.assertEqual(db.execute("SELECT COUNT(*) FROM Cities").fetchone()[0], 0)
        self.assertEqual(verify_registry(), 56)

        # Each castle file is generated from its map and loads only with that historical map.
        modinfo = ET.parse(MAP_MOD / "JAC_PoC_JapanMapScale.modinfo").getroot()
        criteria = {c.get("id"): c.findtext("./ConfigurationValueMatches/Value")
                    for c in modinfo.findall("./ActionCriteria/Criteria")}
        test_modinfo = ET.parse(TEST_MOD / "JAC_Test_UnificationMap.modinfo").getroot()
        criteria.update({c.get("id"): c.findtext("./ConfigurationValueMatches/Value")
                         for c in test_modinfo.findall("./ActionCriteria/Criteria")})
        actions = {}
        for info, folder in ((modinfo, MAP_MOD), (test_modinfo, TEST_MOD)):
            files = {row.text for row in info.findall("./Files/File")}
            for action in info.findall("./InGameActions/UpdateDatabase"):
                for file in action.findall("File"):
                    if file.text.endswith("_Castles.sql"):
                        self.assertIn(file.text, files)
                        self.assertEqual(action.findtext("./Properties/LoadOrder"), "300")
                        actions[folder / file.text] = criteria[action.get("criteria")]
        for path, (tag, output) in CASTLE_FILES.items():
            with self.subTest(castle_file=output.name):
                self.assertEqual(output.read_text(encoding="utf-8"), castle_sql(tag, castles(path)))
                mod = MAP_MOD if path.parent.parent == MAP_MOD else TEST_MOD
                mod_id = (modinfo if mod == MAP_MOD else test_modinfo).get("id")
                self.assertEqual(actions.pop(output), f"{{{mod_id}}}Maps/{path.name}")
        self.assertEqual(actions, {})
        self.assertFalse((MAP_MOD / "Data/JAC_RegisteredCastle.xml").exists())
        # The castle files (LoadOrder 300) come after the victory SQL.
        tree = ET.parse(TREE_MOD / "JAC_PoC_SengokuTrees.modinfo").getroot()
        order = next(action.findtext("./Properties/LoadOrder") for action in tree.iter("UpdateDatabase")
                     if "Data/JAC_Sengoku_Unification.sql" in [f.text for f in action.findall("File")])
        self.assertLess(int(order), 300)

    def test_checked_in_sql_is_generated_without_a_castle_count(self):
        script = (TREE_MOD / "Data/JAC_Sengoku_Unification.sql").read_text(encoding="utf-8")
        self.assertEqual(script, sql())
        model = generated_model()
        counts = {req: args["Count"] for req, args in model.args.items() if "Count" in args}
        # Only existence (>=1) and absence (==0) tests: nothing depends on 56 or the test map's 4.
        self.assertEqual(counts, {"JAC_UNIFICATION_CASTLES_EXIST": "1", "JAC_UNIFICATION_NO_CASTLES": "0"})
        # Castles are plots; a building only ever excludes (the rebuilt marker, inverted), since
        # the city centre has CaptureRemovesBuildings and a registering marker would be lost on capture.
        self.assertEqual([(req, inv) for req, (kind, inv) in model.reqs.items()
                          if kind == "REQUIREMENT_CITY_HAS_BUILDING"], [("JAC_UNIFICATION_CITY_NOT_REBUILT", True)])
        self.assertEqual(model.sets[COUNTED_SET], "REQUIREMENTSET_TEST_ALL")
        self.assertEqual(model.members[COUNTED_SET],
                         ["JAC_UNIFICATION_CITY_ON_CASTLE_PLOT", "JAC_UNIFICATION_CITY_NOT_REBUILT"])
        self.assertEqual({req: args["RequirementSetId"] for req, args in model.args.items()
                          if args.get("RequirementSetId") in (CASTLE_SET, COUNTED_SET)}, {
            "JAC_UNIFICATION_CASTLES_EXIST": COUNTED_SET, "JAC_UNIFICATION_NO_CASTLES": COUNTED_SET,
            "JAC_UNIFICATION_CITY_NOT_CASTLE": COUNTED_SET, "JAC_UNIFICATION_CITY_ON_CASTLE_PLOT": CASTLE_SET})
        self.assertEqual(model.members[CASTLE_SET], ["JAC_UNIFICATION_CASTLE_NONE"])
        self.assertEqual(model.args["JAC_UNIFICATION_CASTLE_NONE"], {"X": "-1", "Y": "-1"})
        self.assertEqual(model.sets[model.root], "REQUIREMENTSET_TEST_ANY")
        self.assertEqual(model.members[model.root], ["JAC_UNIFICATION_CASTLES", "JAC_UNIFICATION_CAPITALS"])
        self.assertEqual(model.reqs["JAC_UNIFICATION_ALL_CAPITALS"],
                         ("REQUIREMENT_TEAM_DOMINATION_VICTORY", False))
        self.assertEqual(model.reqs["JAC_UNIFICATION_CITY_NOT_CASTLE"], ("REQUIREMENT_REQUIREMENTSET_IS_MET", True))
        with sqlite3.connect(":memory:") as db:
            db.executescript(SCHEMA)
            db.executescript(script)
            self.assertEqual(db.execute("SELECT Type,Kind FROM Types").fetchall(),
                             [(VICTORY, "KIND_VICTORY"), (REBUILT, "KIND_BUILDING")])
            self.assertEqual(db.execute("SELECT * FROM Buildings").fetchall(),
                             [(REBUILT, f"LOC_{REBUILT}_NAME", "DISTRICT_CITY_CENTER", 1, 1)])
            self.assertEqual(db.execute("SELECT RequirementSetId,EnabledByDefault,OneMoreTurn FROM Victories").fetchone(),
                             ("JAC_UNIFICATION_VICTORY_REQUIREMENTS", 1, 0))
            self.assertEqual(dict(db.execute("SELECT RequirementId,Text FROM RequirementStrings")), {
                "JAC_UNIFICATION_CASTLES": "LOC_VICTORY_JAC_UNIFICATION_PROGRESS",
                "JAC_UNIFICATION_CAPITALS": "LOC_VICTORY_JAC_UNIFICATION_CAPITALS_PROGRESS",
            })

    def test_victory_follows_owned_castles_razes_and_settler_capitals(self):
        model = generated_model(FOUR_SQL)
        oda, date = 0, 1
        four = [(oda, *FOUR[0]), (oda, *FOUR[1]), (date, *FOUR[2]), (date, *FOUR[3])]
        self.assertFalse(model.wins(oda, four))
        self.assertFalse(model.wins(oda, four[:2] + [(oda, *FOUR[2]), (date, *FOUR[3])]), "one castle missing")
        self.assertTrue(model.wins(oda, [(oda, x, y) for x, y in FOUR]))
        # Razed castle: no city stands on its plot, so 3 of the original 4 suffice.
        self.assertTrue(model.wins(oda, [(oda, x, y) for x, y in FOUR[:3]]))
        # Later-founded cities off the registered plots do not block or count.
        self.assertTrue(model.wins(oda, [(oda, x, y) for x, y in FOUR[:3]] + [(date, 16, 10), (2, 12, 8)]))
        self.assertFalse(model.wins(date, [(oda, x, y) for x, y in FOUR[:3]] + [(date, 16, 10)]))
        # Historical maps: capitals are castles, so domination alone never decides there.
        self.assertFalse(model.wins(oda, four, domination=True))
        # Settler starts / ordinary maps: no castle file loaded -> official domination rule.
        bare = generated_model()
        self.assertFalse(bare.wins(oda, [(oda, *FOUR[0]), (date, *FOUR[2])]))
        self.assertFalse(bare.wins(oda, [(oda, *FOUR[0])]))
        self.assertTrue(bare.wins(oda, [(oda, *FOUR[0]), (date, *FOUR[2])], domination=True))

    def test_captured_or_transferred_castle_still_counts(self):
        """Capture keeps the plot and changes only the owner; nothing in the check can be lost."""
        model = generated_model(FOUR_SQL)
        oda, date = 0, 1
        before = [(oda, *FOUR[0]), (oda, *FOUR[1]), (date, *FOUR[2])]  # 4th castle razed
        self.assertFalse(model.wins(oda, before))
        self.assertFalse(model.wins(date, before))
        captured = before[:2] + [(oda, *FOUR[2])]  # Oda takes Date's capital (capture or TransferCity)
        self.assertTrue(model.wins(oda, captured))
        self.assertFalse(model.wins(date, captured))
        # The capture also works the other way: Date taking Oda's castles.
        self.assertTrue(model.wins(date, [(date, x, y) for x, y in FOUR]))
        # Old design check: had the marker vanished on capture, a castle captured by Date would
        # have dropped out and Oda would win with the rest; now Oda must retake it.
        self.assertFalse(model.wins(oda, [(oda, *FOUR[0]), (oda, *FOUR[1]), (oda, *FOUR[2]), (date, *FOUR[3])]))

    def test_city_rebuilt_on_a_razed_castle_does_not_count(self):
        """Owner decision 3a (SSOT 3.1): a razed castle drops out, even if a city is rebuilt on its plot."""
        model = generated_model(FOUR_SQL)
        oda, date = 0, 1
        held = [(oda, x, y) for x, y in FOUR[:3]]  # 4th castle razed
        self.assertTrue(model.wins(oda, held + [(date, *FOUR[3], True)]), "Date's rebuilt city blocks nothing")
        self.assertFalse(model.wins(date, held + [(date, *FOUR[3], True)]))
        self.assertTrue(model.wins(oda, held + [(oda, *FOUR[3], True)]))
        # Without the marker (e.g. capture removed it before the script re-added it) it would block.
        self.assertFalse(model.wins(oda, held + [(date, *FOUR[3])]))
        # Only rebuilt cities left on registered plots: nothing counts, so the capital rule decides.
        rebuilt_only = [(oda, x, y, True) for x, y in FOUR[:2]]
        self.assertFalse(model.wins(oda, rebuilt_only))
        self.assertTrue(model.wins(oda, rebuilt_only, domination=True))

    def test_rebuilt_castle_script_marks_razed_plots_and_survives_capture(self):
        script = (TREE_MOD / "Scripts/JAC_Sengoku_RebuiltCastles.lua").read_text(encoding="utf-8")
        for needle in (f'"{CASTLE_SET}"', f'"{REBUILT}"', "GameEvents.CityBuilt.Add(OnCityBuilt)",
                       "GameEvents.CityConquered.Add(OnCityConquered)",
                       "GameEvents.OnGameTurnStarted.Add(OnGameTurnStarted)", "Game:SetProperty(RAZED_PREFIX",
                       "Game:GetProperty(RAZED_PREFIX", "CreateIncompleteBuilding(MARKER.Index",
                       "Game.GetCurrentGameTurn() <= GameConfiguration.GetStartTurn()"):
            self.assertIn(needle, script)
        self.assertNotIn("CityRemovedFromMap.Add", script)  # UI-side, no plot (see the script header)
        modinfo = ET.parse(TREE_MOD / "JAC_PoC_SengokuTrees.modinfo").getroot()
        scripts = {action.get("id"): [f.text for f in action.findall("File")]
                   for action in modinfo.findall("./InGameActions/AddGameplayScripts")}
        self.assertEqual(scripts["JAC_Sengoku_RebuiltCastles"], ["Scripts/JAC_Sengoku_RebuiltCastles.lua"])
        self.assertIn("Scripts/JAC_Sengoku_RebuiltCastles.lua", [f.text for f in modinfo.findall("./Files/File")])

    def test_castle_files_do_not_depend_on_load_order(self):
        for scripts in ((sql(), FOUR_SQL), (FOUR_SQL, sql())):
            db = sqlite3.connect(":memory:")
            db.executescript(SCHEMA)
            for script in scripts:
                db.executescript(script)
            self.assertEqual(db.execute("SELECT COUNT(*) FROM RequirementSetRequirements WHERE RequirementSetId=?",
                                        (CASTLE_SET,)).fetchone()[0], 5)

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
                                     ("NAME", "DESCRIPTION", "BLURB", "PROGRESS", "CAPITALS_PROGRESS")}
                         | {f"LOC_{REBUILT}_NAME"})
        self.assertTrue(all(langs == {"ja_JP", "en_US", "ru_RU"} for langs in tags.values()))
        modinfo = ET.parse(TREE_MOD / "JAC_PoC_SengokuTrees.modinfo").getroot()
        self.assertEqual([row.text for row in modinfo.findall("./FrontEndActions/UpdateText/File")],
                         ["Text/JAC_Sengoku_Unification_Text.xml"])

    def test_test_map_registry_comes_from_map_data_and_stays_in_a_test_only_mod(self):
        path = TEST_MOD / "Maps" / f"{test_map.MAP_NAME}.Civ6Map"
        registry = test_map.castle_rows(path)
        self.assertEqual(sorted(registry.values()), [0, 0, 1, 1])
        self.assertEqual({(x, y) for x, y, _ in castles(path)}, set(FOUR))
        with closing(sqlite3.connect(path)) as db:
            self.assertEqual(db.execute("SELECT ID,LeaderType FROM Players ORDER BY ID").fetchall(),
                             [(0, "LEADER_JAC_ODA_NOBUHIDE"), (1, "LEADER_JAC_DATE_TANEMUNE"),
                              (2, "LEADER_JAC_ASHINA_MORIKIYO")])
            self.assertEqual(db.execute("SELECT MapSizeType FROM Map").fetchone()[0], test_map.MAP_SIZE)
            self.assertEqual(db.execute("PRAGMA integrity_check").fetchone()[0], "ok")
            capitals = {row[0] for row in db.execute(
                "SELECT Plot FROM Buildings WHERE BuildingType='BUILDING_PALACE'")}
        self.assertEqual({registry[plot] for plot in capitals}, {0, 1})
        if test_map.OFFICIAL.is_file():  # regenerate with the same tooling and compare
            with tempfile.TemporaryDirectory(dir=ROOT) as folder:
                fresh = Path(folder) / path.name
                test_map.build(fresh)
                self.assertEqual(test_map.castle_rows(fresh), registry)
                with closing(sqlite3.connect(fresh)) as a, closing(sqlite3.connect(path)) as b:
                    for table in ("Plots", "PlotOwners", "Units", "StartPositions", "Buildings"):
                        query = f"SELECT * FROM {table} ORDER BY 1, 2"
                        self.assertEqual(a.execute(query).fetchall(), b.execute(query).fetchall(), table)

        modinfo = ET.parse(TEST_MOD / "JAC_Test_UnificationMap.modinfo").getroot()
        self.assertEqual(modinfo.get("id"), test_map.TEST_MOD_ID)
        self.assertIn("TEST", modinfo.findtext("./LocalizedText/Text/en_US"))
        self.assertEqual({row.text for row in modinfo.findall("./Files/File")}, {
            "Config/JAC_Test_UnificationMap_Config.xml", "Text/JAC_Test_UnificationMap_Text.xml",
            "Data/JAC_Unification_Test_Castles.sql",
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
        self.assertEqual(actions[final + 1:final + 4], ["wait_for", "ui_lua", "wait_for"])
        own = steps[final + 1]  # the transfer is not visible immediately: wait (live 2026-09-29 "positive castles 2/3")
        self.assertEqual((own["condition"]["type"], own["condition"]["state_name"], own["timeout"]), ("lua", "InGame", 30))
        self.assertIn("owned==total", own["condition"]["code"])
        # Castles are found by plot in every live check, never by a building.
        self.assertNotIn("HasBuilding", json.dumps(data))
        self.assertIn("['25,5']=true", own["condition"]["code"])
        self.assertIn("AutoplayManager.SetActive(true)", steps[final + 2]["code"])
        self.assertIn(VICTORY, steps[final + 3]["condition"]["code"])
        self.assertNotIn("castles", steps[-1].get("code", ""))  # nothing but the screenshot after victory

        registry = json.loads((folder / "unification-registry-15km.json").read_text(encoding="utf-8"))
        self.assertEqual(registry, registry_scenario())
        self.assertEqual(registry["steps"][1]["map_file"],
                         "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map")
        self.assertIn("total~=56", registry["steps"][-1]["code"])
        self.assertNotIn("TransferCity", json.dumps(registry))
        self.assertNotIn("autoplay", [step["action"] for step in registry["steps"]])


if __name__ == "__main__":
    unittest.main()
