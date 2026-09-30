"""Check the PoC against the installed official GS schema, after Sengoku's cascading tree deletes."""
import importlib.util
import json
from pathlib import Path
import sqlite3
import unittest
import xml.etree.ElementTree as ET

from lupa import LuaRuntime

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_LeaderPersona"
CUSTOM = ROOT / "poc/JAC_PoC_CustomCivs"
spec = importlib.util.spec_from_file_location("persona_cascade_audit", ROOT / "tools/sengoku_cascade_audit.py")
AUDIT = importlib.util.module_from_spec(spec)
spec.loader.exec_module(AUDIT)


@unittest.skipUnless(AUDIT.available(), "official Civ6 data not installed")
class LeaderPersonaDatabaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        original = AUDIT.official_db()
        for file in ["JAC_CustomCivs_Gameplay.sql", "JAC_Japan1534_Gameplay.sql"]:
            original.executescript((CUSTOM / "Data" / file).read_text(encoding="utf-8"))
        cls.db = AUDIT._run_sengoku(original, True)
        # This existing loader intentionally omits other DLC tables/data. Record its dangling
        # official references, then reject every additional violation introduced by this PoC.
        cls.before_fk = set(cls.db.execute("PRAGMA foreign_key_check"))
        cls.db.executescript((MOD / "Data/JAC_LeaderPersona.sql").read_text(encoding="utf-8"))

    def test_all_22_factions_keep_their_leader_identity(self):
        rows = self.db.execute("SELECT LeaderType, CivilizationType, PersonType, GovernorType, Name, PortraitIcon "
                               "FROM JAC_LeaderPersonas").fetchall()
        expected = self.db.execute("SELECT l.LeaderType,c.CivilizationType,l.Name FROM Leaders l JOIN CivilizationLeaders c "
                                   "ON c.LeaderType=l.LeaderType WHERE c.CivilizationType LIKE 'CIVILIZATION_JAC_%'").fetchall()
        self.assertEqual(len(rows), 22)
        self.assertEqual({(r[0], r[1], r[4]) for r in rows}, set(expected))
        for leader, civ, person, governor, name, portrait in rows:
            with self.subTest(leader=leader):
                self.assertEqual(portrait, f"ICON_{leader}")
                self.assertEqual(self.db.execute("SELECT Name,ActionCharges FROM GreatPersonIndividuals WHERE "
                                                 "GreatPersonIndividualType=?", (person,)).fetchone(), (name, 0))
                self.assertEqual(self.db.execute("SELECT Name,TransitionStrength,AssignCityState FROM Governors "
                                                 "WHERE GovernorType=?", (governor,)).fetchone(), (name, 100, 0))
        self.assertEqual(set(self.db.execute("PRAGMA foreign_key_check")) - self.before_fk, set())
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO JAC_LeaderPersonas SELECT 'LEADER_MISSING','CIVILIZATION_MISSING',"
                            "'PERSON_MISSING','GOVERNOR_MISSING','LOC_MISSING','ICON_MISSING','BASE_MISSING','PROMOTION_MISSING'")

    def test_no_recruitment_or_retirement_effects(self):
        self.assertEqual(self.db.execute("SELECT UnitType,AvailableInTimeline,PseudoYieldType FROM GreatPersonClasses "
                                         "WHERE GreatPersonClassType='GREAT_PERSON_CLASS_JAC_PERSONA'").fetchone(),
                         ("UNIT_GREAT_GENERAL", 0, None))
        for table in ["GreatPersonIndividualActionModifiers", "GreatPersonIndividualBirthModifiers"]:
            self.assertEqual(self.db.execute(f"SELECT COUNT(*) FROM {table} WHERE GreatPersonIndividualType IN "
                                             "(SELECT PersonType FROM JAC_LeaderPersonas)").fetchone()[0], 0)

    def test_each_governor_has_its_own_promotable_test_tree(self):
        rows = self.db.execute("SELECT GovernorType,BasePromotionType,PromotionType FROM JAC_LeaderPersonas").fetchall()
        self.assertEqual(len(rows), 22)
        for gov, base, promoted in rows:
            with self.subTest(gov=gov):
                self.assertEqual(set(self.db.execute("SELECT GovernorPromotion FROM GovernorPromotionSets WHERE "
                                                     "GovernorType=?", (gov,))), {(base,), (promoted,)})
                self.assertEqual(self.db.execute("SELECT BaseAbility,Level FROM GovernorPromotions WHERE "
                                                 "GovernorPromotionType=?", (base,)).fetchone(), (1, 0))
                self.assertEqual(self.db.execute("SELECT BaseAbility,Level FROM GovernorPromotions WHERE "
                                                 "GovernorPromotionType=?", (promoted,)).fetchone(), (0, 1))
                self.assertEqual(self.db.execute("SELECT PrereqGovernorPromotion FROM GovernorPromotionPrereqs "
                                                 "WHERE GovernorPromotionType=?", (promoted,)).fetchall(), [(base,)])
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM GovernorPromotionModifiers WHERE GovernorPromotionType "
                                         "LIKE 'GOVERNOR_PROMOTION_JAC_PERSONA_%'").fetchone()[0], 0)

    def test_no_new_sources_of_titles_are_added(self):
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM Modifiers WHERE ModifierId LIKE 'JAC_PERSONA_%'").fetchone()[0], 0)
        for table in ["GameModifiers", "CivicModifiers", "TraitModifiers", "BuildingModifiers", "GovernorModifiers"]:
            self.assertEqual(self.db.execute(f"SELECT COUNT(*) FROM {table} WHERE ModifierId LIKE 'JAC_PERSONA_TEST_%'").fetchone()[0], 0)
        self.assertEqual(int(self.db.execute("SELECT Value FROM GlobalParameters WHERE Name='MAX_GOVERNOR_APPOINTMENTS'").fetchone()[0]), 30)


class LeaderPersonaStaticTests(unittest.TestCase):
    def test_modinfo_files_exist_and_every_action_is_scoped_to_a_japan_map(self):
        meta = ET.parse(MOD / "JAC_PoC_LeaderPersona.modinfo").getroot()
        files = {f.text for f in meta.findall("./Files/File")}
        self.assertEqual(files, {p.relative_to(MOD).as_posix() for p in MOD.rglob("*") if p.is_file() and p.suffix != ".modinfo"})
        criteria = {c.get("id"): c for c in meta.findall("./ActionCriteria/Criteria")}
        self.assertEqual(len(criteria), 4)
        self.assertEqual({c.findtext("ConfigurationValueMatches/Value").split("Maps/")[1] for c in criteria.values()},
                         {"JAC_Japan_15km.Civ6Map", "JAC_Japan_20km.Civ6Map", "JAC_Japan_15km_Settlers.Civ6Map", "JAC_Japan_20km_Settlers.Civ6Map"})
        for action in meta.find("InGameActions"):
            self.assertEqual(criteria[action.get("criteria")].findtext("RuleSetInUse"), "RULESET_JAC_BANSEIKI")
            for path in action.findall("File"):
                self.assertIn(path.text, files)
            if action.tag == "UpdateDatabase":
                self.assertGreater(int(action.findtext("Properties/LoadOrder")), 200)

    def test_each_text_tag_has_exactly_the_three_required_languages(self):
        rows = ET.parse(MOD / "Text/JAC_LeaderPersona_Text.xml").findall("./LocalizedText/Row")
        translations = {}
        for row in rows:
            tag, lang = row.get("Tag"), row.get("Language")
            self.assertNotIn(lang, translations.setdefault(tag, set()))
            translations[tag].add(lang)
            self.assertTrue(row.findtext("Text"))
        self.assertEqual(len(translations), 14)
        self.assertTrue(all(langs == {"ja_JP", "en_US", "ru_RU"} for langs in translations.values()))

    def test_governor_icons_reference_the_correct_existing_leader_and_unique_pk(self):
        leaders = {}
        for file in ["JAC_CustomCivs_Icons.xml", "JAC_Japan1534_Icons.xml"]:
            for row in ET.parse(CUSTOM / "Data" / file).findall("./IconDefinitions/Row"):
                if row.get("Name").startswith("ICON_LEADER_JAC_"):
                    leaders[row.get("Name")[16:]] = (row.get("Atlas"), row.get("Index"))
        icons = ET.parse(MOD / "Data/JAC_LeaderPersona_Icons.xml").findall("./IconDefinitions/Row")
        self.assertEqual(len(leaders), 22)
        self.assertEqual(len(icons), 110)
        self.assertEqual(len({(r.get("Name"), r.get("Atlas")) for r in icons}), 110)
        for suffix, (atlas, index) in leaders.items():
            for end in ["", "_FILL", "_SLOT"]:
                found = [r for r in icons if r.get("Name") == f"ICON_GOVERNOR_JAC_PERSONA_{suffix}{end}" and r.get("Atlas") == atlas]
                self.assertEqual([r.get("Index") for r in found], [index])

    def test_gameplay_never_registers_ui_events_or_uses_a_void_unit_return(self):
        lua = (MOD / "Scripts/JAC_LeaderPersona_Gameplay.lua").read_text(encoding="utf-8")
        self.assertNotIn("Events.LoadScreenClose", lua)
        self.assertNotIn("UnitManager.InitUnitValidAdjacentHex", lua)
        self.assertNotIn('""', lua)
        self.assertNotIn("LuaEvents.", lua)

    def test_live_scenario_compiles_and_checks_each_saved_native_state(self):
        scenario = json.loads((ROOT / "tools/civ6-autotest/scenarios/leader-persona-15km.json").read_text(encoding="utf-8"))
        steps = scenario["steps"]
        saves = [s["name"] for s in steps if s["action"] == "save"]
        self.assertEqual(saves, ["JAC-Persona-GreatPerson", "JAC-Persona-Governor", "JAC-Persona-Establishing",
                                 "JAC-Persona-Reassigned", "JAC-Persona-Promoted"])
        self.assertEqual([s["name"] for s in steps if s["action"] == "load"], saves)
        self.assertEqual(sum(s.get("turns", 0) for s in steps if s["action"] == "end_turn"), 11)
        lua = LuaRuntime(unpack_returned_tuples=True)
        codes = [s["code"] for s in steps if s["action"] == "ui_lua"]
        codes += [s["condition"]["code"] for s in steps if s["action"] == "wait_for"]
        self.assertGreater(len(codes), 50)
        for i, code in enumerate(codes):
            with self.subTest(code=i):
                compiled = lua.eval("load")(code)
                self.assertNotIsInstance(compiled, tuple, "Lua compilation failed")
        assertions = "\n".join(codes)
        for api in ["GetIndividual", "GetName", "GetActionCharges", "GetUnitPortraitIconNames", "GetGovernorList",
                    "GetAssignedGovernor", "GetAssignedCity", "IsEstablished", "GetTurnsOnSite", "HasPromotion",
                    "NO_TITLE", "ALREADY_JOINED", "ALREADY_EVOLVED", "TITLE_ALREADY_GRANTED"]:
            self.assertIn(api, assertions)


if __name__ == "__main__":
    unittest.main()
