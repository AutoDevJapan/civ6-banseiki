"""Static gates for the 1534 Japan-map diplomatic relations (data, Lua copy, mod wiring)."""

from __future__ import annotations

import json
import re
import sys
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "documents/spec/data/japan-1534-relations.json"
MOD = ROOT / "poc/JAC_PoC_JapanMapScale"
LUA = MOD / "Scripts/JAC_Japan_1534Relations.lua"
SCRIPT = "Scripts/JAC_Japan_1534Relations.lua"
sys.path.insert(0, str(ROOT / "tools/japan_map_scale"))
import write_feuds  # noqa: E402

FEUDS = MOD / "Data/JAC_Japan_Feuds.xml"
DATE = re.compile(r"^\d{4}(-\d{2}(-\d{2})?)?$")


def date_key(value: str | None, missing: int) -> tuple[int, int, int]:
    """Year-only or year-month dates pad with `missing` (0 = earliest, 99 = latest)."""
    if value is None:
        return (missing * 100,) * 3
    parts = [int(part) for part in value.split("-")]
    return tuple(parts + [missing] * (3 - len(parts)))


def applies(relation: dict, start: str) -> bool:
    if relation["confidence"] == "low":
        return False
    return date_key(relation["from"], 0) <= date_key(start, 0) < date_key(relation["to"], 99)


class Japan1534RelationsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = json.loads(DATA.read_text(encoding="utf-8"))
        cls.relations = cls.data["relations"]

    def test_factions_match_the_22_map_seats(self):
        seats = re.findall(r'^\s*Seat\("([A-Z_]+)"', (ROOT / "tools/japan_map_scale/generate.py")
                           .read_text(encoding="utf-8"), re.M)
        self.assertEqual(len(seats), 22)
        self.assertEqual(self.data["factions"], seats)

    def test_relations_are_valid_unordered_pairs(self):
        factions = set(self.data["factions"])
        pairs, ids = set(), set()
        for relation in self.relations:
            with self.subTest(relation=relation["id"]):
                a, b = relation["a"], relation["b"]
                self.assertIn(a, factions)
                self.assertIn(b, factions)
                self.assertNotEqual(a, b)
                pair = frozenset((a, b))
                self.assertNotIn(pair, pairs, "a pair has one relation; A-B and B-A are the same pair")
                pairs.add(pair)
                self.assertNotIn(relation["id"], ids)
                ids.add(relation["id"])
                self.assertIn(relation["type"], self.data["relation_types"])
                self.assertIn(relation["confidence"], {"high", "medium", "low"})
                if relation["type"] == "vassal":
                    self.assertIn(relation["lord"], (a, b))
                else:
                    self.assertNotIn("lord", relation)
                self.assertTrue(relation["summary"])
                self.assertTrue(relation["sources"])
                for source in relation["sources"]:
                    self.assertTrue(source["title"])
                    self.assertTrue(source["url"].startswith("https://"))

    def test_dates_are_well_formed_and_ordered(self):
        self.assertEqual(self.data["start_date"], "1534-05-12")
        for relation in self.relations:
            with self.subTest(relation=relation["id"]):
                for key in ("from", "to"):
                    value = relation[key]
                    self.assertTrue(value is None or DATE.fullmatch(value), value)
                    if value:
                        month_day = [int(part) for part in value.split("-")[1:]]
                        if month_day:
                            self.assertTrue(1 <= month_day[0] <= 12)
                        if len(month_day) == 2:
                            self.assertTrue(1 <= month_day[1] <= 30)  # lunar months have 29 or 30 days
                if relation["from"] and relation["to"]:
                    self.assertLessEqual(date_key(relation["from"], 0), date_key(relation["to"], 99))

    def test_apply_rule_uses_start_date_and_confidence(self):
        start = self.data["start_date"]
        by_id = {relation["id"]: relation for relation in self.relations}
        self.assertTrue(applies(by_id["R01"], start))  # war confirmed from 1534-04-06
        self.assertFalse(applies(by_id["R18"], start))  # begins 1534-08, after the start
        self.assertFalse(applies(by_id["R16"], start))  # low confidence stays a record
        self.assertFalse(applies({"confidence": "high", "from": "1534-05-12", "to": "1534-05-12"}, start))
        self.assertTrue(applies({"confidence": "medium", "from": "1534", "to": "1534-05"}, start))

    def test_lua_copies_exactly_the_applicable_rows(self):
        start = self.data["start_date"]
        expected = [(r["a"], r["b"], r["type"], r["id"]) for r in self.relations
                    if applies(r, start)]
        body = LUA.read_text(encoding="utf-8").split("local RELATIONS = {", 1)[1].split("\n}", 1)[0]
        actual = re.findall(r'\{"([A-Z_]+)", "([A-Z_]+)", "([a-z]+)", "(R\d+)"\}', body)
        self.assertEqual(actual, expected)
        self.assertEqual(len(actual), body.count("{"))  # no row escaped the pattern
        self.assertEqual({row[2] for row in actual}, {"war", "alliance", "marriage", "vassal", "hostility", "friendship"})
        self.assertIn(("SHIMAZU", "RYUKYU", "friendship", "R19"), actual)
        self.assertIn("LuaEvents.NewGameInitialized.Add(Apply)", LUA.read_text(encoding="utf-8"))

    def test_script_and_feuds_run_on_the_two_historical_maps_only(self):
        mod = ET.parse(MOD / "JAC_PoC_JapanMapScale.modinfo").getroot()
        criteria = {row.attrib["id"]: row for row in mod.findall("./ActionCriteria/Criteria")}
        config = ET.parse(MOD / "Config/JAC_JapanMapScale_Config.xml").getroot()
        map_files = {row.attrib["File"] for row in config.findall("./Maps/Row")}
        historical = {f for f in map_files if "Settlers" not in f}
        self.assertEqual(len(map_files), 4)
        self.assertEqual(len(historical), 2)
        for tag, path in (("AddGameplayScripts", SCRIPT), ("UpdateDatabase", "Data/JAC_Japan_Feuds.xml")):
            rows = [row for row in mod.findall(f"./InGameActions/{tag}") if row.findtext("File") == path]
            self.assertEqual(len(rows), 2, path)
            selected = set()
            for row in rows:
                criterion = criteria[row.attrib["criteria"]]
                self.assertEqual(criterion.findtext("RuleSetInUse"), "RULESET_JAC_BANSEIKI")
                self.assertEqual(criterion.findtext("./ConfigurationValueMatches/ConfigurationId"), "MAP_SCRIPT")
                selected.add(criterion.findtext("./ConfigurationValueMatches/Value"))
            self.assertEqual(selected, historical, path)
        files = {row.text for row in mod.findall("./Files/File")}
        self.assertLessEqual({SCRIPT, "Data/JAC_Japan_Feuds.xml"}, files)

    def test_feud_xml_is_generated_from_the_hostility_rows(self):
        expected = {frozenset((r["a"], r["b"])) for r in self.relations
                    if r["type"] == "hostility" and applies(r, self.data["start_date"])}
        self.assertEqual(len(expected), 7)
        self.assertEqual({frozenset(p) for p in write_feuds.feud_pairs()}, expected)
        root = ET.parse(FEUDS).getroot()
        mods = {row.attrib["ModifierId"]: row.attrib for row in root.findall("./Modifiers/Row")
                if row.attrib["ModifierId"] != write_feuds.ACCESS_MODIFIER}
        owners = {(row.attrib["TraitType"], row.attrib["ModifierId"]) for row in root.findall("./TraitModifiers/Row")}
        for pair in expected:
            for a, b in (tuple(pair), tuple(pair)[::-1]):  # both sides dislike each other
                with self.subTest(a=a, b=b):
                    mod = mods[f"JAC_1534_FEUD_{a}_{b}"]
                    self.assertEqual(mod["ModifierType"], "MODIFIER_PLAYER_DIPLOMACY_SIMPLE_MODIFIER")
                    self.assertEqual(mod["SubjectRequirementSetId"], f"JAC_1534_PLAYER_IS_{b}")
                    self.assertIn((f"TRAIT_JAC_1534_FEUD_{a}", mod["ModifierId"]), owners)
        args = {(row.attrib["ModifierId"], row.attrib["Name"]): row.attrib["Value"]
                for row in root.findall("./ModifierArguments/Row")}
        self.assertEqual(len(mods), 14)
        for mod_id in mods:
            self.assertLess(int(args[(mod_id, "InitialValue")]), 0)
            self.assertNotIn((mod_id, "ReductionTurns"), args)  # never decays while the pair stands
            self.assertEqual(args[(mod_id, "SimpleModifierDescription")], write_feuds.TEXT_TAG)
        strings = {row.attrib["ModifierId"]: row.attrib for row in root.findall("./ModifierStrings/Row")}
        self.assertEqual(set(strings), set(mods))  # no row -> "Unknown Reason" on the diplomacy screen
        for row in strings.values():
            self.assertEqual((row["Context"], row["Text"]), ("Sample", write_feuds.SAMPLE_TAG))
        self.assertNotIn("FEUD_SHIMAZU", FEUDS.read_text(encoding="utf-8"))  # friendship is not a feud
        # the checked-in file is what the generator writes
        fresh = write_feuds.build()
        ET.indent(fresh, space="  ")
        self.assertEqual(ET.tostring(root, encoding="unicode"), ET.tostring(fresh, encoding="unicode"))
        text = (MOD / "Text/JAC_JapanMapScale_Text.xml").read_text(encoding="utf-8")
        self.assertIn(write_feuds.TEXT_TAG, text)
        self.assertIn("1534年の因縁", text)

    def test_every_major_starts_with_limited_access_so_reasons_are_not_unknown(self):
        # At access level None the engine hides every reason, official ones too ("Unknown Reason",
        # Civilopedia_Concepts_Text.xml:771). All 22 houses get +1 the way Printing grants it
        # (Technologies.xml:546-616): MODIFIER_PLAYER_ADD_DIPLO_VISIBILITY on one trait + a source row.
        # Official rows, field by field (Base/Assets/Gameplay/Data/):
        #   Technologies.xml:546-549 Modifiers  PRINTING_ADD_DIPLO_VISIBILITY: ModifierId, ModifierType only
        #   Technologies.xml:603-617 Arguments  Amount=1, Source=SOURCE_TECH, SourceType=DIPLO_SOURCE_ALL_NAMES
        #   Leaders.xml:1698-1701 / :4011-4025  Catherine: same columns, Source=SOURCE_TRAIT
        #   Gossip.xml:78 SOURCE_TRAIT: Description, ActionDescription, GossipString, FromCitizen=true,
        #                 TraitType=FLYING_SQUADRON_TRAIT (the trait that carries the modifier, Leaders.xml:1088)
        official_modifier = {"ModifierId": "PRINTING_ADD_DIPLO_VISIBILITY",
                             "ModifierType": "MODIFIER_PLAYER_ADD_DIPLO_VISIBILITY"}
        official_args = {"Amount": "1", "Source": "SOURCE_TECH", "SourceType": "DIPLO_SOURCE_ALL_NAMES"}
        official_source = {"VisibilitySourceType": "SOURCE_TRAIT", "Description": "LOC_VIZSOURCE_TRAIT",
                           "ActionDescription": "LOC_VIZSOURCE_ACTION_TRAIT", "GossipString": "LOC_GOSSIP_SOURCE_TRAIT",
                           "FromCitizen": "true", "TraitType": "FLYING_SQUADRON_TRAIT"}
        root = ET.parse(FEUDS).getroot()
        mod = write_feuds.ACCESS_MODIFIER
        rows = [row.attrib for row in root.findall("./Modifiers/Row") if row.attrib["ModifierId"] == mod]
        self.assertEqual(rows, [{**official_modifier, "ModifierId": mod}])
        args = {row.attrib["Name"]: row.attrib["Value"] for row in root.findall("./ModifierArguments/Row")
                if row.attrib["ModifierId"] == mod}
        self.assertEqual(args, {**official_args, "Source": write_feuds.ACCESS_SOURCE})
        holders = {row.attrib["CivilizationType"] for row in root.findall("./CivilizationTraits/Row")
                   if row.attrib["TraitType"] == write_feuds.ACCESS_TRAIT}
        self.assertEqual(holders, {f"CIVILIZATION_JAC_{f}" for f in self.data["factions"]})
        self.assertEqual(len(holders), 22)
        self.assertEqual([(r.attrib["TraitType"], r.attrib["ModifierId"]) for r in root.findall("./TraitModifiers/Row")
                          if r.attrib["ModifierId"] == mod], [(write_feuds.ACCESS_TRAIT, mod)])
        sources = [row.attrib for row in root.findall("./DiplomaticVisibilitySources/Row")]
        self.assertEqual([s["VisibilitySourceType"] for s in sources], [write_feuds.ACCESS_SOURCE])
        # Every official source row names its grant (PrereqTech / TraitType / ...); the bare row gave access 0
        # live on 2026-09-29. Ours copies SOURCE_TRAIT column for column, gated on the trait that carries it.
        self.assertEqual(set(sources[0]), set(official_source))
        self.assertEqual(sources[0]["FromCitizen"], official_source["FromCitizen"])
        self.assertEqual(sources[0]["TraitType"], write_feuds.ACCESS_TRAIT)
        self.assertIn(write_feuds.ACCESS_TRAIT, {r.attrib["TraitType"] for r in root.findall("./Traits/Row")})
        text = (MOD / "Text/JAC_JapanMapScale_Text.xml").read_text(encoding="utf-8")
        for key in ("Description", "ActionDescription", "GossipString"):
            tag = sources[0][key]
            for lang in ("en_US", "ja_JP", "ru_RU"):
                self.assertIn(f'Tag="{tag}" Language="{lang}"', text)
        # The source must not borrow an official row that the Sengoku tree removes (SOURCE_TECH = Printing).
        self.assertNotIn("PrereqTech", sources[0])
        lua = LUA.read_text(encoding="utf-8")
        self.assertIn("RecheckVisibilityOnAll()", lua)
        # everyone meets everyone once at new-game start (not a per-relation side effect)
        self.assertRegex(lua, r"for _, a in pairs\(players\) do\s+for _, b in pairs\(players\) do\s+"
                              r"if a ~= b then Players\[a\]:GetDiplomacy\(\):SetHasMet\(b\) end")
        self.assertLess(lua.index("SetHasMet(b) end"), lua.index("for _, relation in ipairs(RELATIONS)"))

    def test_ruleset_shares_ally_vision_by_default(self):
        config = (ROOT / "poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml").read_text(encoding="utf-8")
        row = re.search(r'<Row [^>]*ConfigurationId="GAME_ALLIES_SHARE_VISIBILITY"[^>]*/>', config).group(0)
        self.assertIn('DefaultValue="1"', row)

    def test_sengoku_tree_keeps_the_printing_access_bonus(self):
        sql = (ROOT / "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Unlocks.sql").read_text(encoding="utf-8")
        tech = "TECH_JAC_SENGOKU_ROADS_AND_PACKTRAINS"
        move = f"UPDATE TechnologyModifiers SET TechnologyType='{tech}'"
        self.assertIn(move, sql)
        self.assertIn("PRINTING_ADD_DIPLO_VISIBILITY", sql)
        self.assertIn(f"UPDATE DiplomaticVisibilitySources SET PrereqTech='{tech}'", sql)
        self.assertLess(sql.index(move), sql.index("DELETE FROM Technologies WHERE"))

    def test_feud_ui_scenario_rejects_unknown_reason_rows(self):
        path = ROOT / "tools/civ6-autotest/scenarios/japan-1534-feud-ui.json"
        codes = [step.get("code", "") for step in json.loads(path.read_text(encoding="utf-8"))["steps"]]
        read = next(code for code in codes if "RelationshipReasonStack" in code)
        self.assertIn("LOC_TOOLTIP_DIPLOMACY_UNKNOWN_REASON", read)
        self.assertIn("reason row(s) read", read)
        opened = next(code for code in codes if "DiplomacyRibbon_OpenDiplomacyActionView" in code)
        self.assertIn("GetVisibilityOn(id.TAKEDA)", opened)


if __name__ == "__main__":
    unittest.main()
