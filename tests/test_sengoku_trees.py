"""Static gates for the generated Sengoku technology and civic PoC."""

from __future__ import annotations

import importlib.util
import sqlite3
import unittest
import xml.etree.ElementTree as ET
from collections import defaultdict
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_SengokuTrees"
SPEC = importlib.util.spec_from_file_location("generate_sengoku_trees", ROOT / "tools/generate_sengoku_trees.py")
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
import sys
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class SengokuTreeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.nodes = MODULE.read_design()
        cls.by_id = {node.id: node for node in cls.nodes}

    def test_ids_unique_and_complete(self):
        self.assertEqual(len(self.nodes), 62)
        self.assertEqual(len(self.by_id), 62)
        self.assertEqual(sum(n.short.startswith("T") for n in self.nodes), 32)
        self.assertEqual(sum(n.short.startswith("C") for n in self.nodes), 30)
        self.assertEqual({n.short for n in self.nodes},
                         {f"T{i:02}" for i in range(1, 33)} | {f"C{i:02}" for i in range(1, 31)})

    def test_prerequisites_acyclic_and_all_nodes_reachable(self):
        visiting, visited = set(), set()

        def walk(node):
            if node.id in visiting:
                self.fail(f"cycle at {node.short}")
            if node.id in visited:
                return
            visiting.add(node.id)
            for parent_id in node.prereqs:
                self.assertIn(parent_id, self.by_id)
                parent = self.by_id[parent_id]
                self.assertEqual(parent.short[0], node.short[0])
                self.assertLess(parent.column, node.column)
                walk(parent)
            visiting.remove(node.id)
            visited.add(node.id)

        for node in self.nodes:
            walk(node)
        self.assertEqual(len(visited), 62)
        roots = [n for n in self.nodes if not n.prereqs]
        self.assertEqual({n.short for n in roots}, {"T01", "T02", "T03", "T04", "C01", "C02", "C03"})

    def test_positions_and_costs(self):
        for prefix, row_min, row_max, costs in (("T", -3, 4, MODULE.TECH_COSTS),
                                               ("C", -3, 3, MODULE.CIVIC_COSTS)):
            group = [n for n in self.nodes if n.short.startswith(prefix)]
            positions = {(n.column, n.row) for n in group}
            self.assertEqual(len(positions), len(group), f"overlapping {prefix} positions")
            self.assertTrue(all(row_min <= n.row <= row_max and 1 <= n.column <= 12 for n in group))
            self.assertTrue(all(n.cost == costs[n.column - 1] for n in group))
            self.assertTrue(all(a < b for a, b in zip(costs, costs[1:])))
            self.assertTrue(all(n.era == MODULE.ERAS[(n.column - 1) // 3] for n in group))
            self.assertEqual({n.era for n in group}, set(MODULE.ERAS))

    def test_generated_files_current_and_all_text_keys_translated(self):
        for path, expected in MODULE.outputs().items():
            self.assertEqual(path.read_text(encoding="utf-8"), expected, str(path))
        root = ET.parse(MOD / "Text/JAC_Sengoku_Text.xml").getroot()
        translations = defaultdict(dict)
        for row in root.findall("./LocalizedText/Row"):
            key, language = row.attrib["Tag"], row.attrib["Language"]
            self.assertNotIn(language, translations[key], f"duplicate {key}/{language}")
            translations[key][language] = row.findtext("Text")
        required = {f"LOC_{n.id}_{suffix}" for n in self.nodes for suffix in ("NAME", "DESCRIPTION")}
        required |= {f"LOC_JAC_SENGOKU_ERA_{i}_{suffix}" for i in range(1, 5)
                     for suffix in ("NAME", "DESCRIPTION")}
        required |= {f"LOC_JAC_SENGOKU_BOOST_{short}" for short in MODULE.BOOSTS}
        self.assertEqual(set(translations), required)
        for key, text in translations.items():
            self.assertEqual(set(text), {"ja_JP", "en_US", "ru_RU"}, key)
            self.assertTrue(all(value and value.strip() for value in text.values()), key)
        meta = ET.parse(MOD / "JAC_PoC_SengokuTrees.modinfo").getroot()
        self.assertEqual(meta.findtext("./ActionCriteria/Criteria/RuleSetInUse"), "RULESET_JAC_BANSEIKI")
        self.assertEqual({action.attrib.get("criteria") for action in meta.findall("./InGameActions/*")},
                         {"JAC_Sengoku_BanseikiOnly"})
        for row in meta.findall("./LocalizedText/Text"):
            self.assertTrue(all(row.findtext(lang) for lang in ("en_US", "ja_JP", "ru_RU")))
        files = [row.text for row in meta.findall("./Files/File")]
        self.assertEqual(files, [
            "Data/JAC_Sengoku_Trees.sql", "Data/JAC_Sengoku_Unlocks.sql",
            "Data/JAC_Sengoku_Victories.sql", "Data/JAC_Sengoku_Icons.xml",
            "Config/JAC_Sengoku_Victories.xml", "Text/JAC_Sengoku_Text.xml",
        ])
        self.assertTrue(all((MOD / file).is_file() for file in files))
        self.assertEqual([row.text for row in meta.findall("./FrontEndActions/UpdateDatabase/File")],
                         ["Config/JAC_Sengoku_Victories.xml"])
        self.assertEqual([row.text for row in meta.findall("./InGameActions/UpdateDatabase/File")][-1],
                         "Data/JAC_Sengoku_Victories.sql")
        icons = ET.parse(MOD / "Data/JAC_Sengoku_Icons.xml").getroot()
        icon_names = [row.get("Name") for row in icons.findall("./IconDefinitions/Row")]
        self.assertEqual(len(icon_names), 124)
        self.assertEqual(len(set(icon_names)), 124)
        self.assertEqual(set(icon_names), {"ICON_" + n.id + suffix for n in self.nodes
                                                for suffix in ("", "_FOW")})

    def test_every_unlock_proposal_has_explicit_proxy_or_todo(self):
        self.assertTrue(all(n.unlock and n.proposed_boost for n in self.nodes))
        self.assertTrue(set(MODULE.OFFICIAL_UNLOCKS) <= {n.short for n in self.nodes})
        matrix = (ROOT / "documents/poc/sengoku-tree-unlocks.md").read_text(encoding="utf-8")
        for node in self.nodes:
            self.assertIn(f"| {node.short} {node.ja} |", matrix)
            self.assertIn(node.unlock, matrix)
        self.assertEqual(matrix.count("| TODO: 設計案の固有実装") + matrix.count("| TODO: 公式代理なし"), 62)
        self.assertEqual(matrix.count("| TODO: 公式BoostClassで完全には表せない"), 60)

    def test_sql_loads_and_rewires_without_erasing_gs_sentinels(self):
        db = sqlite3.connect(":memory:")
        db.executescript("""
            CREATE TABLE Types (Type TEXT PRIMARY KEY, Kind TEXT);
            CREATE TABLE Technologies (TechnologyType TEXT PRIMARY KEY, Name TEXT, Description TEXT, Cost INT, EraType TEXT, UITreeRow INT, AdvisorType TEXT);
            CREATE TABLE Civics (CivicType TEXT PRIMARY KEY, Name TEXT, Description TEXT, Cost INT, EraType TEXT, UITreeRow INT, AdvisorType TEXT);
            CREATE TABLE TechnologyPrereqs (Technology TEXT, PrereqTech TEXT);
            CREATE TABLE CivicPrereqs (Civic TEXT, PrereqCivic TEXT);
            CREATE TABLE Eras (EraType TEXT PRIMARY KEY, Name TEXT, Description TEXT);
            CREATE TABLE Eras_XP2 (EraType TEXT, EraScoreThresholdShift INT);
            CREATE TABLE ClimateParameters (Name TEXT);
            CREATE TABLE Resolutions (ResolutionType TEXT);
            CREATE TABLE Boosts (TechnologyType TEXT, Boost INT, TriggerDescription TEXT, TriggerLongDescription TEXT, BoostClass TEXT, ImprovementType TEXT, ResourceType TEXT, Unit1Type TEXT);
            CREATE TABLE Units (UnitType TEXT PRIMARY KEY, PrereqTech TEXT, PrereqCivic TEXT, CanTrain INT);
            CREATE TABLE UnitUpgrades (Unit TEXT, UpgradeUnit TEXT);
            CREATE TABLE Districts (DistrictType TEXT PRIMARY KEY, PrereqTech TEXT, PrereqCivic TEXT, InternalOnly INT);
            CREATE TABLE Buildings (BuildingType TEXT PRIMARY KEY, PrereqTech TEXT, PrereqCivic TEXT, InternalOnly INT);
            CREATE TABLE Improvements (ImprovementType TEXT PRIMARY KEY, PrereqTech TEXT, PrereqCivic TEXT, Buildable INT);
            CREATE TABLE Resources (ResourceType TEXT PRIMARY KEY, PrereqTech TEXT);
            CREATE TABLE Governments (GovernmentType TEXT PRIMARY KEY, PrereqCivic TEXT);
            CREATE TABLE StartingCivics (Era TEXT, Civic TEXT);
            CREATE TABLE Policies (PolicyType TEXT PRIMARY KEY, PrereqCivic TEXT, PrereqTech TEXT);
            CREATE TABLE Projects (ProjectType TEXT PRIMARY KEY, PrereqTech TEXT, PrereqCivic TEXT);
            INSERT INTO Technologies VALUES ('TECH_POTTERY','Pottery',NULL,25,'ERA_ANCIENT',0,NULL);
            INSERT INTO Civics VALUES ('CIVIC_CODE_OF_LAWS','Code of Laws',NULL,20,'ERA_ANCIENT',0,NULL);
            INSERT INTO Eras VALUES ('ERA_ANCIENT','Ancient',NULL),('ERA_CLASSICAL','Classical',NULL),('ERA_MEDIEVAL','Medieval',NULL),('ERA_RENAISSANCE','Renaissance',NULL);
            INSERT INTO Eras_XP2 VALUES ('ERA_ANCIENT',-3);
            INSERT INTO ClimateParameters VALUES ('TemperatureChange');
            INSERT INTO Resolutions VALUES ('WC_RES_LUXURY');
            INSERT INTO Units VALUES ('UNIT_GALLEY','TECH_SAILING',NULL,1),('UNIT_MODERN_ARMOR','TECH_COMPOSITES',NULL,1);
            INSERT INTO Districts VALUES ('DISTRICT_HARBOR','TECH_CELESTIAL_NAVIGATION',NULL,0),('DISTRICT_AERODROME','TECH_FLIGHT',NULL,0);
            INSERT INTO Buildings VALUES ('BUILDING_GRANARY','TECH_POTTERY',NULL,0);
            INSERT INTO Improvements VALUES ('IMPROVEMENT_MINE','TECH_MINING',NULL,1);
            INSERT INTO Resources VALUES ('RESOURCE_IRON','TECH_BRONZE_WORKING'),('RESOURCE_HORSES','TECH_ANIMAL_HUSBANDRY');
            INSERT INTO Governments VALUES ('GOVERNMENT_CHIEFDOM',NULL),('GOVERNMENT_OLIGARCHY','CIVIC_POLITICAL_PHILOSOPHY');
            INSERT INTO StartingCivics VALUES ('ERA_ANCIENT','CIVIC_CODE_OF_LAWS');
            INSERT INTO Policies VALUES ('POLICY_DISCIPLINE','CIVIC_CODE_OF_LAWS',NULL);
            INSERT INTO Projects VALUES ('PROJECT_REPAIR_OUTER_DEFENSES',NULL,NULL);
        """)
        db.executescript((MOD / "Data/JAC_Sengoku_Trees.sql").read_text(encoding="utf-8"))
        db.executescript((MOD / "Data/JAC_Sengoku_Unlocks.sql").read_text(encoding="utf-8"))
        for table, col, count in (("Technologies", "TechnologyType", 32), ("Civics", "CivicType", 30)):
            self.assertEqual(db.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0], count)
            self.assertEqual(db.execute(f"SELECT COUNT(*) FROM {table} WHERE {col} NOT LIKE '%JAC_SENGOKU%'").fetchone()[0], 0)
        self.assertEqual(db.execute("SELECT PrereqTech FROM Units WHERE UnitType='UNIT_GALLEY'").fetchone()[0],
                         "TECH_JAC_SENGOKU_COASTAL_PILOTAGE")
        self.assertEqual(db.execute("SELECT CanTrain FROM Units WHERE UnitType='UNIT_MODERN_ARMOR'").fetchone()[0], 0)
        self.assertEqual(db.execute("SELECT PrereqTech FROM Districts WHERE DistrictType='DISTRICT_HARBOR'").fetchone()[0],
                         "TECH_JAC_SENGOKU_HARBOR_PILOTS")
        self.assertEqual(db.execute("SELECT PrereqTech FROM Districts WHERE DistrictType='DISTRICT_AERODROME'").fetchone()[0],
                         "TECH_JAC_SENGOKU_OUTER_DEFENSES")
        self.assertIsNone(db.execute("SELECT PrereqTech FROM Resources WHERE ResourceType='RESOURCE_HORSES'").fetchone()[0])
        self.assertEqual(db.execute("SELECT Civic FROM StartingCivics WHERE Era='ERA_ANCIENT'").fetchone()[0],
                         "CIVIC_JAC_SENGOKU_HOUSE_RULES")
        self.assertEqual(db.execute("SELECT COUNT(*) FROM Eras_XP2").fetchone()[0], 1)
        self.assertEqual(db.execute("SELECT COUNT(*) FROM ClimateParameters").fetchone()[0], 1)
        self.assertEqual(db.execute("SELECT COUNT(*) FROM Resolutions").fetchone()[0], 1)
        db.close()

    def test_banseiki_sengoku_disables_official_victories_and_time_limit(self):
        base = ET.parse(ROOT / "poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml")
        self.assertEqual(base.find("./Rulesets/Row").get("VictoryDomain"), "Expansion2_Victories")
        config = ET.parse(MOD / "Config/JAC_Sengoku_Victories.xml")
        update = config.find("./Rulesets/Update")
        self.assertEqual(update.find("Where").attrib, {"RulesetType": "RULESET_JAC_BANSEIKI"})
        self.assertEqual(update.find("Set").attrib,
                         {"VictoryDomain": "JAC_Sengoku_Victories", "MaxTurns": "0"})
        self.assertFalse(config.findall("./Victories/Row"))
        self.assertEqual([row.attrib for row in config.findall("./RulesetSupportedValues/Row")], [{
            "Ruleset": "RULESET_JAC_BANSEIKI", "Domain": "StandardTurnLimits", "Value": "TURNLIMIT_NONE"
        }])

        official = {"VICTORY_DEFAULT", "VICTORY_CONQUEST", "VICTORY_TECHNOLOGY",
                    "VICTORY_CULTURE", "VICTORY_RELIGIOUS", "VICTORY_DIPLOMATIC", "VICTORY_SCORE"}
        updates = config.findall("./ConfigurationUpdates/Row")
        self.assertTrue(all(row.get("SourceGroup") == "Game" and row.get("SourceId") == "RULESET"
                            and row.get("SourceValue") == "RULESET_JAC_BANSEIKI"
                            and row.get("TargetGroup") == "Game" for row in updates))
        values = {row.get("TargetId"): row.get("TargetValue") for row in updates}
        self.assertEqual(len(values), len(updates))
        self.assertEqual(values, {**dict.fromkeys(official | {"DEFEAT_TIME"}, "0"),
                                  "GAME_TURN_LIMIT": "TURNLIMIT_NONE", "GAME_MAX_TURNS": "0"})

        db = sqlite3.connect(":memory:")
        db.executescript("""
            CREATE TABLE Victories (VictoryType TEXT PRIMARY KEY, EnabledByDefault INT);
            CREATE TABLE Defeats (DefeatType TEXT PRIMARY KEY, EnabledByDefault INT);
            INSERT INTO Victories VALUES ('VICTORY_CUSTOM', 1);
            INSERT INTO Defeats VALUES ('DEFEAT_DEFAULT', 1), ('DEFEAT_TIME', 1);
        """)
        db.executemany("INSERT INTO Victories VALUES (?, 1)", ((victory,) for victory in official))
        db.executescript((MOD / "Data/JAC_Sengoku_Victories.sql").read_text(encoding="utf-8"))
        self.assertEqual(dict(db.execute("SELECT VictoryType, EnabledByDefault FROM Victories")),
                         {**dict.fromkeys(official, 0), "VICTORY_CUSTOM": 1})
        self.assertEqual(dict(db.execute("SELECT DefeatType, EnabledByDefault FROM Defeats")),
                         {"DEFEAT_DEFAULT": 1, "DEFEAT_TIME": 0})
        db.close()


if __name__ == "__main__":
    unittest.main()
