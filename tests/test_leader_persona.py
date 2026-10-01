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
        cls.before_persona = cls.db.serialize()
        for file in ["JAC_LeaderPersona.sql", "JAC_LeaderPersona_Trees.sql"]:
            cls.db.executescript((MOD / "Data" / file).read_text(encoding="utf-8"))

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
                            "'PERSON_MISSING','GOVERNOR_MISSING','LOC_MISSING','ICON_MISSING','BASE_MISSING',"
                            "'CALM','MUNDANE',NULL,0,'HEAD',1")

    def test_no_recruitment_or_retirement_effects(self):
        self.assertEqual(self.db.execute("SELECT UnitType,AvailableInTimeline,GenerateDuplicateIndividuals,PseudoYieldType FROM GreatPersonClasses "
                                         "WHERE GreatPersonClassType='GREAT_PERSON_CLASS_JAC_PERSONA'").fetchone(),
                         ("UNIT_JAC_PERSONA", 0, 1, "PSEUDOYIELD_GPP_GENERAL"))
        self.assertEqual(self.db.execute("SELECT GreatPersonClassType FROM GreatPersonClasses "
                                         "WHERE UnitType='UNIT_JAC_PERSONA'").fetchall(),
                         [("GREAT_PERSON_CLASS_JAC_PERSONA",)])
        self.assertEqual(self.db.execute("SELECT CanTrain,CanCapture,CanRetreatWhenCaptured FROM Units "
                                         "WHERE UnitType='UNIT_JAC_PERSONA'").fetchone(), (0, 0, 1))
        self.assertEqual(self.db.execute("SELECT GreatPersonClassType FROM GreatPersonClasses "
                                         "WHERE UnitType='UNIT_GREAT_GENERAL'").fetchall(),
                         [("GREAT_PERSON_CLASS_GENERAL",)])
        for table in ["GreatPersonIndividualActionModifiers", "GreatPersonIndividualBirthModifiers"]:
            self.assertEqual(self.db.execute(f"SELECT COUNT(*) FROM {table} WHERE GreatPersonIndividualType IN "
                                             "(SELECT PersonType FROM JAC_LeaderPersonas)").fetchone()[0], 0)

    def test_only_native_bases_are_registered_and_all_custom_nodes_have_gated_effects(self):
        people = self.db.execute("SELECT GovernorType,BasePromotionType FROM JAC_LeaderPersonas").fetchall()
        self.assertEqual(len(people), 22)
        for governor,base in people:
            self.assertEqual(self.db.execute("SELECT GovernorPromotion FROM GovernorPromotionSets WHERE GovernorType=?", (governor,)).fetchall(), [(base,)])
            self.assertEqual(self.db.execute("SELECT BaseAbility,Level FROM GovernorPromotions WHERE GovernorPromotionType=?", (base,)).fetchone(), (1,0))
        nodes = self.db.execute("SELECT LeaderType,ModifierId,EffectKind,Amount FROM JAC_PersonaTreeNodes").fetchall()
        self.assertEqual(len(nodes), 762)
        for leader,modifier,kind,amount in nodes:
            self.assertGreater(amount, 0)
            self.assertEqual(self.db.execute("SELECT GovernorPromotionType FROM GovernorPromotionModifiers WHERE ModifierId=?", (modifier,)).fetchone(), self.db.execute("SELECT BasePromotionType FROM JAC_LeaderPersonas WHERE LeaderType=?", (leader,)).fetchone())
            self.assertEqual(self.db.execute("SELECT SubjectRequirementSetId FROM Modifiers WHERE ModifierId=?", (modifier,)).fetchone(), (modifier+"_SET",))
            self.assertEqual(dict(self.db.execute("SELECT Name,Value FROM RequirementArguments WHERE RequirementId=?", (modifier+"_REQ",))), {"PropertyName":modifier,"PropertyMinimum":"1"})
            self.assertEqual(self.db.execute("SELECT RequirementType FROM Requirements WHERE RequirementId=?", (modifier+"_REQ",)).fetchone(), ("REQUIREMENT_PLOT_PROPERTY_MATCHES",))
            self.assertEqual(self.db.execute("SELECT RequirementId FROM RequirementSetRequirements WHERE RequirementSetId=?", (modifier+"_SET",)).fetchall(), [(modifier+"_REQ",)])
            self.assertEqual(int(self.db.execute("SELECT Value FROM ModifierArguments WHERE ModifierId=? AND Name='Amount'", (modifier,)).fetchone()[0]), amount)

    def test_all_persons_have_reusable_chains_choices_and_one_unique_signature(self):
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM JAC_PersonaRouteTemplates").fetchone()[0], 16)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM JAC_PersonaNodeTemplates").fetchone()[0], 48)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM JAC_PersonaSignatureTemplates").fetchone()[0], 66)
        people = self.db.execute("SELECT LeaderType,PreferredCategory,PreferredRoute FROM JAC_PersonaTreePeople").fetchall()
        self.assertEqual(len(people), 22)
        for leader,preferred,strong in people:
            access = [r[0] for r in self.db.execute("SELECT CategoryType FROM JAC_PersonaCategoryAccess WHERE LeaderType=?", (leader,))]
            self.assertIn(len(access), (2,3))
            self.assertIn(preferred, access)
            self.assertLess(len(access), 4)
            signatures = self.db.execute("SELECT CategoryType,RouteType,Depth,Cost FROM JAC_PersonaTreeNodes WHERE LeaderType=? AND Signature=1 ORDER BY Depth", (leader,)).fetchall()
            self.assertEqual([(r[0],r[1],r[2],r[3]) for r in signatures], [(preferred,"SIGNATURE_"+leader,d,c) for d,c in zip((1,2,3),(8,12,18))])
            for category in access:
                routes = self.db.execute("SELECT DISTINCT RouteType,ExclusiveGroup FROM JAC_PersonaTreeNodes WHERE LeaderType=? AND CategoryType=? AND Signature=0", (leader,category)).fetchall()
                self.assertEqual(len(routes),4)
                self.assertEqual(sorted(sum(1 for r in routes if r[1]==group) for group in {r[1] for r in routes}),[2,2])
                # One chain from each exclusive pair gives SIX promotions in any category.
                self.assertEqual(len({r[1] for r in routes})*3,6)
                for route,group in routes:
                    chain = self.db.execute("SELECT NodeType,PrereqNode,Depth,Cost FROM JAC_PersonaTreeNodes WHERE LeaderType=? AND RouteType=? ORDER BY Depth", (leader,route)).fetchall()
                    self.assertEqual(len(chain),3)
                    self.assertEqual([r[1] for r in chain],[None,chain[0][0],chain[1][0]])
                    self.assertEqual([r[3] for r in chain],[8,12,18] if route==strong else [10,15,22])

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

    def test_base_and_shared_effects_use_official_types_and_conservative_magnitudes(self):
        rows=self.db.execute("SELECT LeaderType,YieldType,Amount FROM JAC_PersonaSkills ORDER BY LeaderType").fetchall()
        self.assertEqual(len(rows),22)
        bases={r[0]:(r[1],r[2]) for r in rows}
        self.assertEqual(bases["LEADER_JAC_DATE_TANEMUNE"],("YIELD_CULTURE",10))
        self.assertEqual(bases["LEADER_JAC_ODA_NOBUHIDE"],("YIELD_GOLD",10))
        for modifier,amount in [("LIBRARIAN_CULTURE_YIELD_BONUS",15),("PASHA_BONUS_UNIT_PRODUCTION",20)]:
            self.assertEqual(int(self.db.execute("SELECT Value FROM ModifierArguments WHERE ModifierId=? AND Name='Amount'", (modifier,)).fetchone()[0]),amount)
        maxima=dict(self.db.execute("SELECT EffectKind,MAX(Amount) FROM JAC_PersonaTreeNodes GROUP BY EffectKind"))
        self.assertEqual(maxima,{"AMENITIES":1,"CULTURE":4,"FAITH":4,"FOOD":4,"GOLD":4,"MILITARY":10,"PRODUCTION":4,"SCIENCE":4})
        self.assertEqual(dict(self.db.execute("SELECT Name,Value FROM JAC_PersonaTreeParameters WHERE Name LIKE '%COST_%'")),
                         {"NODE_COST_1":10,"NODE_COST_2":15,"NODE_COST_3":22,"STRONG_COST_1":8,"STRONG_COST_2":12,"STRONG_COST_3":18})

    def test_titles_come_only_from_15_sengoku_civics_and_only_persona_governors_exist(self):
        modifiers=self.db.execute("SELECT ModifierId FROM Modifiers JOIN DynamicModifiers USING(ModifierType) WHERE EffectType='EFFECT_ADJUST_PLAYER_GOVERNOR_POINTS'").fetchall()
        self.assertEqual(modifiers,[("JAC_PERSONA_CIVIC_EVOLUTION_TITLE",)])
        grants=self.db.execute("SELECT CivicType FROM CivicModifiers WHERE ModifierId='JAC_PERSONA_CIVIC_EVOLUTION_TITLE'").fetchall()
        self.assertEqual(len(grants),15)
        self.assertTrue(all(r[0].startswith("CIVIC_JAC_SENGOKU_") for r in grants))
        self.assertEqual(set(grants),set(self.db.execute("SELECT CivicType FROM JAC_PersonaTitleCivics")))
        self.assertEqual(self.db.execute("SELECT Name,Value FROM ModifierArguments WHERE ModifierId='JAC_PERSONA_CIVIC_EVOLUTION_TITLE'").fetchall(),[("Delta","1")])
        for table in ["GameModifiers","TraitModifiers","BuildingModifiers","GovernorModifiers","GreatPersonIndividualActionModifiers"]:
            self.assertEqual(self.db.execute(f"SELECT COUNT(*) FROM {table} WHERE ModifierId='JAC_PERSONA_CIVIC_EVOLUTION_TITLE'").fetchone()[0],0)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM GoodyHutSubTypes WHERE ModifierID='JAC_PERSONA_CIVIC_EVOLUTION_TITLE'").fetchone()[0],0)
        disabled={r[0] for r in self.db.execute("SELECT GovernorType FROM JAC_PersonaDisabledGovernors")}
        self.assertEqual(disabled, {"GOVERNOR_THE_DEFENDER", "GOVERNOR_THE_CARDINAL", "GOVERNOR_THE_BUILDER",
                                   "GOVERNOR_THE_AMBASSADOR", "GOVERNOR_THE_RESOURCE_MANAGER", "GOVERNOR_THE_MERCHANT",
                                   "GOVERNOR_THE_EDUCATOR", "GOVERNOR_IBRAHIM"})
        self.assertEqual(set(self.db.execute("SELECT GovernorType FROM Governors")),
                         set(self.db.execute("SELECT GovernorType FROM JAC_LeaderPersonas")))
        for table in ["GovernorPromotionSets", "GovernorModifiers", "Governors_XP2", "GovernorsCannotAssign"]:
            self.assertEqual(self.db.execute(f"SELECT COUNT(*) FROM {table} WHERE GovernorType IN (SELECT GovernorType FROM JAC_PersonaDisabledGovernors)").fetchone()[0],0)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM Governors WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas) AND TraitType IS NOT NULL").fetchone()[0],0)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM Kinds WHERE Kind='KIND_TRAIT'").fetchone()[0],1)

    def test_official_governor_removal_preserves_another_mods_governor_and_children(self):
        db = sqlite3.connect(":memory:")
        try:
            db.create_function("Make_Hash", 1, lambda value: hash(value) & 0x7FFFFFFF)
            db.deserialize(self.before_persona)
            db.execute("PRAGMA foreign_keys=ON")
            foreign = "GOVERNOR_OTHER_MOD"
            db.execute("INSERT INTO Types (Type,Kind) VALUES (?, 'KIND_GOVERNOR')", (foreign,))
            db.execute("INSERT INTO Governors (GovernorType,Name,Description,Title,ShortTitle,PortraitImage,PortraitImageSelected) "
                       "SELECT ?,Name,Description,Title,ShortTitle,PortraitImage,PortraitImageSelected FROM Governors "
                       "WHERE GovernorType='GOVERNOR_THE_DEFENDER'", (foreign,))
            promotion = db.execute("SELECT GovernorPromotion FROM GovernorPromotionSets WHERE GovernorType='GOVERNOR_THE_DEFENDER' ORDER BY GovernorPromotion LIMIT 1").fetchone()[0]
            db.execute("INSERT INTO GovernorPromotionSets VALUES (?, ?)", (foreign, promotion))
            for file in ["JAC_LeaderPersona.sql", "JAC_LeaderPersona_Trees.sql"]:
                db.executescript((MOD / "Data" / file).read_text(encoding="utf-8"))
            self.assertEqual(db.execute("SELECT GovernorType FROM Governors WHERE GovernorType=?", (foreign,)).fetchall(), [(foreign,)])
            self.assertEqual(db.execute("SELECT GovernorPromotion FROM GovernorPromotionSets WHERE GovernorType=?", (foreign,)).fetchall(), [(promotion,)])
            self.assertEqual(db.execute("SELECT GovernorType FROM JAC_PersonaDisabledGovernors WHERE GovernorType=?", (foreign,)).fetchall(), [])
        finally:
            db.close()



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
        self.assertEqual(len(translations), 291)
        self.assertTrue(all(langs == {"ja_JP", "en_US", "ru_RU"} for langs in translations.values()))

    def test_governor_icons_reference_the_correct_existing_leader_and_unique_pk(self):
        leaders = {}
        for file in ["JAC_CustomCivs_Icons.xml", "JAC_Japan1534_Icons.xml"]:
            for row in ET.parse(CUSTOM / "Data" / file).findall("./IconDefinitions/Row"):
                if row.get("Name").startswith("ICON_LEADER_JAC_"):
                    leaders[row.get("Name")[16:]] = (row.get("Atlas"), row.get("Index"))
        icons = ET.parse(MOD / "Data/JAC_LeaderPersona_Icons.xml").findall("./IconDefinitions/Row")
        self.assertEqual(len(leaders), 22)
        self.assertEqual(len(icons), 111)
        self.assertEqual(len({(r.get("Name"), r.get("Atlas")) for r in icons}), 111)
        for suffix, (atlas, index) in leaders.items():
            for end in ["", "_FILL", "_SLOT"]:
                found = [r for r in icons if r.get("Name") == f"ICON_GOVERNOR_JAC_PERSONA_{suffix}{end}" and r.get("Atlas") == atlas]
                self.assertEqual([r.get("Index") for r in found], [index])
        # Icon XML is loaded in table order, with atlas references checked immediately.
        # Use the already loaded official 22px/32px city-banner art for both meters.
        official = ET.parse(AUDIT.GAME / "DLC/Expansion1/Data/Expansion1_Icons_Governors.xml")
        for end, filename in [("_FILL", "Fill"), ("_SLOT", "Slot")]:
            atlas = "ICON_ATLAS_EXPANSION_1_GOVERNORS" + end
            sizes = {int(r.get("IconSize")): r.get("Filename") for r in
                     official.findall("./IconTextureAtlases/Row") if r.get("Name") == atlas}
            self.assertEqual(sizes, {22: f"XP1_GovernorsCityBanner{filename}22.dds",
                                     32: f"XP1_GovernorsCityBanner{filename}32.dds"})
            meters = [r for r in icons if r.get("Atlas") == atlas]
            self.assertEqual({r.get("Name") for r in meters},
                             {f"ICON_GOVERNOR_JAC_PERSONA_{suffix}{end}" for suffix in leaders})
            self.assertEqual({r.get("Index") for r in meters}, {"6"})

    def test_gameplay_never_registers_ui_events_or_uses_a_void_unit_return(self):
        lua = (MOD / "Scripts/JAC_LeaderPersona_Gameplay.lua").read_text(encoding="utf-8")
        self.assertNotIn("Events.LoadScreenClose", lua)
        self.assertNotIn("UnitManager.InitUnitValidAdjacentHex", lua)
        self.assertNotIn('""', lua)
        self.assertNotIn("LuaEvents.", lua)
        self.assertNotIn("GrantTestTitle", lua)
        self.assertNotIn("TEST_TITLE_", lua)

    def test_dedicated_persona_unit_loads_the_official_general_model_on_all_target_maps(self):
        art = ET.parse(MOD / "ArtDefs/Units.artdef")
        units = art.findall("./m_RootCollections/Element/Element")
        self.assertEqual([u.find("m_Name").get("text") for u in units], ["UNIT_JAC_PERSONA"])
        members = units[0].findall(".//Element[@class='AssetObjects..ArtDefReferenceValue']")
        self.assertTrue(any(e.find("m_ElementName").get("text") == "GreatGeneral_Classical_Male"
                            and e.find("m_RootCollectionName").get("text") == "UnitMemberTypes"
                            for e in members))
        dep = ET.parse(MOD / "JAC_PoC_LeaderPersona.dep")
        self.assertEqual([e.get("text") for e in dep.findall("./ArtDefDependencies/Element/ArtDefPath")],
                         ["Units.artdef"])
        mod = ET.parse(MOD / "JAC_PoC_LeaderPersona.modinfo")
        actions = mod.findall("./InGameActions/UpdateArt")
        self.assertEqual({a.get("criteria") for a in actions}, {f"JAC_Persona_Map_{i}" for i in range(4)})
        self.assertEqual([a.find("File").text for a in actions], ["JAC_PoC_LeaderPersona.dep"] * 4)
        icons = ET.parse(MOD / "Data/JAC_LeaderPersona_Icons.xml")
        icon = icons.find("./IconDefinitions/Row[@Name='ICON_UNIT_JAC_PERSONA']")
        self.assertEqual((icon.get("Atlas"), icon.get("Index")), ("ICON_ATLAS_UNITS", "8"))

    def test_live_scenario_compiles_and_asserts_civic_titles_merit_and_permanent_locks(self):
        scenario=json.loads((ROOT / "tools/civ6-autotest/scenarios/leader-persona-15km.json").read_text(encoding="utf-8"))
        steps=scenario["steps"]
        saves=[s["name"] for s in steps if s["action"]=="save"]
        self.assertEqual([s["name"] for s in steps if s["action"]=="load"],saves)
        self.assertGreaterEqual(len(saves),4)
        self.assertTrue(any(s["action"]=="end_turn" and s.get("turns",1)>1 for s in steps))
        lua=LuaRuntime(unpack_returned_tuples=True)
        codes=[s["code"] for s in steps if s["action"]=="ui_lua"]
        codes += [s["condition"]["code"] for s in steps if s["action"]=="wait_for" and s["condition"]["type"]=="lua"]
        for i,code in enumerate(codes):
            with self.subTest(code=i):self.assertNotIsInstance(lua.eval("load")(code),tuple)
        assertions="\n".join(codes)
        for text in ["SetCulturalProgress","JAC_PersonaDisabledGovernors","GetGovernorPointsSpent","JAC_TreeConfirm", "JAC_TreeCancel",
                     "CATEGORY_LOCKED","BRANCH_LOCKED","PREREQ_LOCKED","MERIT","NODE_", "GOVERN_LAST_TURN",
                     "EVOLUTION_SURCHARGE","IsEstablished","GetAssignedCity","GameEffects.GetModifierSubjects","GameEffects.GetModifierActive"]:
            self.assertIn(text,assertions)
        self.assertNotIn("GrantTestTitle",assertions)



if __name__ == "__main__":
    unittest.main()
