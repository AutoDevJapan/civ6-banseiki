"""Check the replacement against the installed game's schema and official row data."""
from pathlib import Path
import sqlite3
import unittest
import xml.etree.ElementTree as ET
import zlib

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_BanseikiRuleset"
GAME = Path("C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI")
SCHEMA = GAME / "Base/Assets/Gameplay/Data/Schema"
CUSTOM = {"HEROCLASS_JAC_WARLORD_A", "HEROCLASS_JAC_WARLORD_B"}


@unittest.skipUnless((SCHEMA / "01_GameplaySchema.sql").is_file(), "installed Civ6 schema required")
class NativeHeroDatabaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(":memory:")
        # SQLite needs the engine's hash trigger callback. Values here only provide unique
        # test keys; compatibility of the engine's hash algorithm is not being claimed.
        cls.db.create_function("Make_Hash", 1, lambda value: zlib.crc32(value.encode()))
        for name in ("01_GameplaySchema.sql", "02_AddTriggers.sql"):
            cls.db.executescript((SCHEMA / name).read_text(encoding="utf-8-sig"))
        columns = {name: {row[1] for row in cls.db.execute(f'PRAGMA table_info("{name}")')}
                   for (name,) in cls.db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        files = sorted((GAME / "Base/Assets/Gameplay/Data").glob("*.xml"))
        files.append(GAME / "DLC/Babylon/Data/Babylon_Heroes_MODE.xml")
        for path in files:
            for section in ET.parse(path).getroot():
                if section.tag not in columns:
                    continue
                canonical = {name.lower(): name for name in columns[section.tag]}
                for row in section.findall("Row"):
                    # Some shipped rows include expansion-only columns; the mod's SQL
                    # uses base-schema columns, so seed that schema's supported fields.
                    values = {canonical[key.lower()]: value for key, value in row.attrib.items() if key.lower() in canonical}
                    values.update({canonical[child.tag.lower()]: child.text for child in row if child.tag.lower() in canonical})
                    if not values:
                        continue
                    values = {key: 1 if str(value).lower() == "true" else 0 if str(value).lower() == "false" else value
                              for key, value in values.items()}
                    names = ",".join(f'"{key}"' for key in values)
                    marks = ",".join("?" for _ in values)
                    cls.db.execute(f'INSERT OR IGNORE INTO "{section.tag}" ({names}) VALUES ({marks})',
                                   tuple(values.values()))
        cls.original = list(cls.db.execute("SELECT HeroClassType,UnitType,CreationProjectType FROM HeroClasses"))
        if len(cls.original) != 12:
            raise AssertionError("official seed must include all twelve shipped Heroes")
        cls.db.execute("INSERT INTO Types(Type,Kind) VALUES ('ERA_JAC_HERO_TEST','KIND_ERA')")
        era_columns = [row[1] for row in cls.db.execute("PRAGMA table_info(Eras)")]
        fields = ",".join(f'"{name}"' for name in era_columns)
        expressions = ",".join("'ERA_JAC_HERO_TEST'" if name == "EraType" else "100" if name == "ChronologyIndex" else f'"{name}"'
                               for name in era_columns)
        cls.db.execute(f"INSERT INTO Eras({fields}) SELECT {expressions} FROM Eras WHERE EraType='ERA_RENAISSANCE'")
        cls.db.commit()
        # Only INSERT rows were loaded, not every official update/DLC. Record existing
        # seed gaps so new violations caused by this mod cannot hide in that baseline.
        cls.baseline = set(cls.db.execute("PRAGMA foreign_key_check"))
        cls.db.execute("PRAGMA foreign_keys=ON")
        cls.db.executescript((MOD / "Data/JAC_Banseiki_Heroes.sql").read_text(encoding="utf-8"))

    @classmethod
    def tearDownClass(cls):
        cls.db.close()

    def test_native_classes_reference_complete_projects_units_and_works(self):
        rows = list(self.db.execute("""SELECT h.HeroClassType FROM HeroClasses h
            JOIN Units u ON u.UnitType=h.UnitType JOIN Units_MODE m ON m.UnitType=u.UnitType
            JOIN Projects p ON p.ProjectType=h.CreationProjectType
            JOIN GreatWorks a ON a.GreatWorkType=h.ArtifactGreatWorkType
            JOIN GreatWorks e ON e.GreatWorkType=h.EpicGreatWorkType
            WHERE u.CanTrain=1 AND u.MustPurchase=1 AND p.MaxPlayerInstances=1
            AND m.ActionCharges=3 AND a.GreatWorkObjectType='GREATWORKOBJECT_HERO'
            AND e.GreatWorkObjectType='GREATWORKOBJECT_HERO'"""))
        self.assertEqual({row[0] for row in rows}, CUSTOM)
        self.assertEqual(set(self.db.execute("PRAGMA foreign_key_check")) - self.baseline, set())

    def test_shipped_heroes_have_no_discovery_project_or_purchase_path(self):
        for hero, unit, project in self.original:
            with self.subTest(hero=hero):
                self.assertIsNone(self.db.execute("SELECT 1 FROM HeroClasses WHERE HeroClassType=?", (hero,)).fetchone())
                self.assertEqual(self.db.execute("SELECT CanTrain FROM Units WHERE UnitType=?", (unit,)).fetchone(), (0,))
                self.assertEqual(self.db.execute("SELECT MaxPlayerInstances FROM Projects WHERE ProjectType=?", (project,)).fetchone(), (0,))
                self.assertEqual(self.db.execute("SELECT COUNT(*) FROM ProjectCompletionModifiers WHERE ProjectType=?", (project,)).fetchone(), (0,))

    def test_native_ids_are_dense_and_every_custom_era_has_lifespan_and_strength_data(self):
        self.assertEqual(list(self.db.execute("SELECT rowid FROM HeroClasses ORDER BY rowid")), [(1,), (2,)])
        eras = {row[0] for row in self.db.execute("SELECT EraType FROM Eras")}
        self.assertIn("ERA_JAC_HERO_TEST", eras)
        for hero in sorted(CUSTOM):
            self.assertEqual({row[0] for row in self.db.execute("SELECT EraType FROM HeroClassProgressions WHERE HeroClassType=?", (hero,))}, eras)
        units = {row[0] for row in self.db.execute("SELECT UnitType FROM HeroClasses")}
        self.assertEqual({row[0] for row in self.db.execute("SELECT Type FROM TypeProperties WHERE Name='LIFESPAN' AND Value='30'") if row[0] in units}, units)

    def test_framework_discounts_and_recruitment_bonus_target_new_definitions(self):
        for suffix in ("A", "B"):
            unit = "UNIT_HERO_JAC_WARLORD_" + suffix
            buildings = {row[0] for row in self.db.execute("""SELECT b.BuildingType FROM BuildingModifiers b
                JOIN ModifierArguments a ON a.ModifierId=b.ModifierId
                WHERE a.Name='UnitType' AND a.Value=?""", (unit,))}
            self.assertEqual(buildings, {"BUILDING_TEMPLE", "BUILDING_ORACLE", "BUILDING_STAVE_CHURCH"})
            project = "PROJECT_CREATE_HERO_JAC_WARLORD_" + suffix
            traits = {row[0] for row in self.db.execute("""SELECT t.TraitType FROM TraitModifiers t
                JOIN ModifierArguments a ON a.ModifierId=t.ModifierId
                WHERE a.Name='ProjectType' AND a.Value=?""", (project,))}
            self.assertEqual(traits, {"TRAIT_LEADER_GILGAMESH_HEROES"})


class NativeHeroPackageTests(unittest.TestCase):
    def test_native_dlc_uses_registered_id_and_is_enabled_before_dependents(self):
        import json
        canonical = "8424840C-92EF-4426-A9B4-B4E0CB818049"
        for folder in ("JAC_PoC_BanseikiRuleset", "JAC_PoC_Absorption"):
            mod = ET.parse(ROOT / "poc" / folder / (folder + ".modinfo")).getroot()
            self.assertIn(canonical, {node.get("id") for node in mod.findall("Dependencies/Mod")})
        for name in ("absorption-japan.json", "absorption-hero-recall.json"):
            data = json.loads((ROOT / "tools/civ6-autotest/scenarios" / name).read_text(encoding="utf-8"))
            ids = data["steps"][0]["ids"]
            self.assertEqual(ids[0], canonical)
            self.assertLess(ids.index("ba3a1c58-9287-4654-a22f-85f9e6841e9b"),
                            ids.index("bd7a82c5-6b38-4b69-a7f5-8dc9af534249"))

    def test_native_warlord_art_uses_generic_infantry_and_cavalry(self):
        root = ET.parse(MOD / "ArtDefs/Units.artdef").getroot()
        units = root.findall("m_RootCollections/Element/Element")
        self.assertEqual({unit.find("m_Name").get("text") for unit in units},
                         {"UNIT_HERO_JAC_WARLORD_A", "UNIT_HERO_JAC_WARLORD_B"})
        expected = {"UNIT_HERO_JAC_WARLORD_A": "Warrior", "UNIT_HERO_JAC_WARLORD_B": "Horsemen"}
        for unit in units:
            formation = next(value for value in unit.findall("m_Fields/m_Values/Element")
                             if value.find("m_ParamName").get("text") == "Formation")
            self.assertEqual(formation.find("m_ElementName").get("text"), expected[unit.find("m_Name").get("text")])

    def test_icons_preserve_every_size_of_generic_units(self):
        db = sqlite3.connect(":memory:")
        self.addCleanup(db.close)
        db.execute('CREATE TABLE IconDefinitions(Name TEXT PRIMARY KEY,"Index" INTEGER,Atlas TEXT)')
        db.execute('CREATE TABLE IconTextureAtlases(Name TEXT,IconSize INTEGER,PRIMARY KEY(Name,IconSize))')
        db.executemany('INSERT INTO IconTextureAtlases VALUES (?,?)',
                       [("BASE_UNITS", size) for size in (32, 38, 50, 80, 256)])
        for unit in ("WARRIOR", "HORSEMAN"):
            db.execute('INSERT INTO IconDefinitions VALUES (?,?,?)', ("ICON_UNIT_" + unit, 7, "BASE_UNITS"))
        db.executescript((MOD / "Data/JAC_Banseiki_Hero_Icons.sql").read_text(encoding="utf-8"))
        for suffix, source in (("A", "WARRIOR"), ("B", "HORSEMAN")):
            query = 'SELECT a.IconSize,d."Index",d.Atlas FROM IconDefinitions d JOIN IconTextureAtlases a ON a.Name=d.Atlas WHERE d.Name=? ORDER BY a.IconSize'
            original = list(db.execute(query, ("ICON_UNIT_" + source,)))
            self.assertEqual(len(original), 5)
            for prefix in ("ICON_UNIT_HERO_JAC_WARLORD_", "ICON_HEROCLASS_JAC_WARLORD_"):
                self.assertEqual(list(db.execute(query, (prefix + suffix,))), original)

    def test_all_custom_names_have_three_languages_and_actions_are_scoped(self):
        texts = ET.parse(MOD / "Text/JAC_Banseiki_Heroes_Text.xml").getroot().find("LocalizedText")
        by_tag = {}
        for row in texts:
            by_tag.setdefault(row.get("Tag"), set()).add(row.get("Language"))
            self.assertTrue(row.findtext("Text"))
        self.assertEqual(len(by_tag), 10)
        self.assertTrue(all(languages == {"ja_JP", "en_US", "ru_RU"} for languages in by_tag.values()))
        mod = ET.parse(MOD / "JAC_PoC_BanseikiRuleset.modinfo").getroot()
        hero_actions = [action for action in mod.find("InGameActions") if action.get("id", "").startswith("JAC_Banseiki_Heroes")]
        self.assertEqual(len(hero_actions), 4)
        self.assertTrue(all(action.get("criteria") == "JAC_Banseiki_Heroes" for action in hero_actions))
        criteria = mod.find("ActionCriteria/Criteria[@id='JAC_Banseiki_Heroes']")
        self.assertEqual(criteria.findtext("RuleSetInUse"), "RULESET_JAC_BANSEIKI")
        self.assertEqual(criteria.findtext("ConfigurationValueMatches/ConfigurationId"), "GAMEMODE_HEROES")
        self.assertEqual(criteria.findtext("ConfigurationValueMatches/Value"), "1")
