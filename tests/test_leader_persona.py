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
                            "'PERSON_MISSING','GOVERNOR_MISSING','LOC_MISSING','ICON_MISSING','BASE_MISSING','PROMOTION_MISSING',"
                            "'PROMOTION2_MISSING','CALM','MUNDANE',NULL,0,'HEAD',1")

    def test_no_recruitment_or_retirement_effects(self):
        self.assertEqual(self.db.execute("SELECT UnitType,AvailableInTimeline,PseudoYieldType FROM GreatPersonClasses "
                                         "WHERE GreatPersonClassType='GREAT_PERSON_CLASS_JAC_PERSONA'").fetchone(),
                         ("UNIT_GREAT_GENERAL", 0, None))
        for table in ["GreatPersonIndividualActionModifiers", "GreatPersonIndividualBirthModifiers"]:
            self.assertEqual(self.db.execute(f"SELECT COUNT(*) FROM {table} WHERE GreatPersonIndividualType IN "
                                             "(SELECT PersonType FROM JAC_LeaderPersonas)").fetchone()[0], 0)

    def test_each_governor_has_three_effectful_nodes_with_two_branches(self):
        rows = self.db.execute("SELECT GovernorType,BasePromotionType,PromotionType,SecondPromotionType FROM JAC_LeaderPersonas").fetchall()
        self.assertEqual(len(rows), 22)
        for gov, base, first, second in rows:
            with self.subTest(gov=gov):
                self.assertEqual(set(self.db.execute("SELECT GovernorPromotion FROM GovernorPromotionSets WHERE "
                                                     "GovernorType=?", (gov,))), {(base,), (first,), (second,)})
                self.assertEqual(self.db.execute("SELECT BaseAbility,Level FROM GovernorPromotions WHERE "
                                                 "GovernorPromotionType=?", (base,)).fetchone(), (1, 0))
                for promoted in (first, second):
                    self.assertEqual(self.db.execute("SELECT BaseAbility,Level FROM GovernorPromotions WHERE "
                                                     "GovernorPromotionType=?", (promoted,)).fetchone(), (0, 1))
                    self.assertEqual(self.db.execute("SELECT PrereqGovernorPromotion FROM GovernorPromotionPrereqs "
                                                     "WHERE GovernorPromotionType=?", (promoted,)).fetchall(), [(base,)])
                for node in (base, first, second):
                    self.assertEqual(self.db.execute("SELECT COUNT(*) FROM GovernorPromotionModifiers WHERE GovernorPromotionType=?", (node,)).fetchone()[0], 1)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM GovernorPromotionModifiers WHERE GovernorPromotionType "
                                         "LIKE 'GOVERNOR_PROMOTION_JAC_PERSONA_%'").fetchone()[0], 66)

    def test_cost_defaults_and_all_22_designs_match_named_provisional_formula(self):
        expected = {"ODA_NOBUHIDE":1, "TAKEDA_NOBUTORA":1, "DATE_TANEMUNE":2, "ASHINA_MORIKIYO":1,
                    "HOJO_UJITSUNA":2, "YAMANOUCHI_UESUGI_NORIMASA":3, "NAGAO_TAMEKAGE":2,
                    "IMAGAWA_UJITERU":2, "MATSUDAIRA_KIYOYASU":1, "ASAKURA_TAKAKAGE":2,
                    "ASHIKAGA_YOSHIHARU":3, "MIYOSHI_NAGAYOSHI":2, "ROKKAKU_SADAYORI":2,
                    "HONGANJI_SHONYO":3, "OUCHI_YOSHITAKA":2, "AMAGO_TSUNEHISA":2,
                    "MORI_MOTONARI":1, "CHOSOKABE_KUNICHIKA":1, "OTOMO_YOSHIAKI":2,
                    "SHIMAZU_TAKAHISA":1, "RYUKYU_SHO_SEI":3, "AINU_TARIKONA":2}
        rows = self.db.execute("SELECT LeaderType,EvolutionCost,Personality,Trait,Age1534 FROM JAC_LeaderPersonas").fetchall()
        self.assertEqual({r[0][11:]:r[1] for r in rows}, expected)
        self.assertTrue(all(r[2] and r[3] for r in rows))
        self.assertEqual({r[0][11:] for r in rows if r[4] is None}, {"NAGAO_TAMEKAGE", "AINU_TARIKONA"})
        self.assertEqual(dict(self.db.execute("SELECT Name,Value FROM JAC_PersonaCostParameters")),
                         {"BASE_TITLES":1,"FAME_TITLE_WEIGHT":1,"YOUTH_AGE_LIMIT":16,"YOUTH_TITLE_SURCHARGE":1,
                          "REPRESENTATIVE_TITLE_SURCHARGE":1,"MIN_TITLES":1,"MAX_TITLES":3})

    def test_date_and_oda_modifiers_use_real_city_effects_and_conservative_magnitudes(self):
        expected = {
            "DATE_TANEMUNE":[("YIELD_CULTURE",10),(None,1),("YIELD_GOLD",10)],
            "ODA_NOBUHIDE":[("YIELD_GOLD",10),("YIELD_PRODUCTION",10),(None,20)]}
        for person, effects in expected.items():
            rows = self.db.execute("SELECT s.YieldType,s.Amount,m.ModifierType,d.CollectionType,d.EffectType "
                                   "FROM JAC_PersonaSkills s JOIN Modifiers m USING(ModifierId) JOIN DynamicModifiers d "
                                   "ON d.ModifierType=m.ModifierType WHERE s.LeaderType=? ORDER BY s.Slot", ("LEADER_JAC_"+person,)).fetchall()
            self.assertEqual([(r[0],r[1]) for r in rows], effects)
            self.assertEqual([r[3] for r in rows], ["COLLECTION_OWNER"]*3)
            self.assertEqual([r[4] for r in rows], ["EFFECT_ADJUST_CITY_YIELD_MODIFIER",
                             "EFFECT_ADJUST_CITY_AMENITIES_FROM_GOVERNORS" if person.startswith("DATE") else "EFFECT_ADJUST_CITY_YIELD_MODIFIER",
                             "EFFECT_ADJUST_CITY_YIELD_MODIFIER" if person.startswith("DATE") else "EFFECT_ADJUST_CITY_ALL_MILITARY_UNITS_PRODUCTION"])
        for modifier, amount in [("LIBRARIAN_CULTURE_YIELD_BONUS",15), ("PASHA_BONUS_UNIT_PRODUCTION",20)]:
            self.assertEqual(int(self.db.execute("SELECT Value FROM ModifierArguments WHERE ModifierId=? AND Name='Amount'", (modifier,)).fetchone()[0]), amount)
        # GS deletes Amani's old local amenity promotion; compare its XP1 source magnitude.
        args = ET.parse(AUDIT.GAME / "DLC/Expansion1/Data/Expansion1_Governors.xml").findall("./ModifierArguments/Row")
        amounts = [int(r.findtext("Value")) for r in args if r.findtext("ModifierId")=="PROMOTER_ADDITIONAL_CITY_AMENITIES" and r.findtext("Name")=="Amount"]
        self.assertEqual(amounts, [4])

    def test_no_new_sources_of_titles_are_added(self):
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM Modifiers m JOIN DynamicModifiers d USING(ModifierType) "
                                         "WHERE m.ModifierId LIKE 'JAC_PERSONA_%' AND d.EffectType='EFFECT_ADJUST_PLAYER_GOVERNOR_POINTS'").fetchone()[0], 0)
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
        self.assertEqual(len(translations), 183)
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
                                 "JAC-Persona-Reassigned", "JAC-Persona-Promoted", "JAC-Persona-Two-Skilled-Governors",
                                 "JAC-Persona-Three-Title-Cost"])
        self.assertEqual([s["name"] for s in steps if s["action"] == "load"], saves)
        self.assertEqual(sum(s.get("turns", 0) for s in steps if s["action"] == "end_turn"), 16)
        self.assertEqual(len(steps), 134)
        human = [p for p in steps[1]["players"] if p["human"]]
        self.assertEqual([p["leader"] for p in human], ["LEADER_JAC_HOJO_UJITSUNA"])
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
                    "NO_TITLE", "ALREADY_JOINED", "ALREADY_EVOLVED", "TITLE_ALREADY_GRANTED",
                    "EvolutionCost", "EVOLUTION_SURCHARGE", "LEADER_JAC_ODA_NOBUHIDE",
                    "SecondPromotionType", "GameEffects.GetModifierActive", "GameEffects.GetModifierSubjects",
                    "duplicate owned city display name", "upper-cost title accounting differs"]:
            self.assertIn(api, assertions)


if __name__ == "__main__":
    unittest.main()
