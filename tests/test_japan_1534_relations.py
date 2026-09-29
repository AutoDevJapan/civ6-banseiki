"""Static gates for the 1534 Japan-map diplomatic relations (data, Lua copy, mod wiring)."""

from __future__ import annotations

import json
import re
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "documents/spec/data/japan-1534-relations.json"
MOD = ROOT / "poc/JAC_PoC_JapanMapScale"
LUA = MOD / "Scripts/JAC_Japan_1534Relations.lua"
SCRIPT = "Scripts/JAC_Japan_1534Relations.lua"
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

    def test_lua_copies_exactly_the_applicable_war_and_friendship_rows(self):
        start = self.data["start_date"]
        expected = [(r["a"], r["b"], r["type"], r["id"]) for r in self.relations
                    if applies(r, start) and r["type"] != "hostility"]
        body = LUA.read_text(encoding="utf-8").split("local RELATIONS = {", 1)[1].split("\n}", 1)[0]
        actual = re.findall(r'\{"([A-Z_]+)", "([A-Z_]+)", "([a-z]+)", "(R\d+)"\}', body)
        self.assertEqual(actual, expected)
        self.assertEqual(len(actual), body.count("{"))  # no row escaped the pattern
        self.assertIn("war", {row[2] for row in actual})
        self.assertIn("LuaEvents.NewGameInitialized.Add(Apply)", LUA.read_text(encoding="utf-8"))

    def test_script_runs_on_all_four_japan_maps_only(self):
        mod = ET.parse(MOD / "JAC_PoC_JapanMapScale.modinfo").getroot()
        criteria = {row.attrib["id"]: row for row in mod.findall("./ActionCriteria/Criteria")}
        config = ET.parse(MOD / "Config/JAC_JapanMapScale_Config.xml").getroot()
        map_files = {row.attrib["File"] for row in config.findall("./Maps/Row")}
        scripts = [row for row in mod.findall("./InGameActions/AddGameplayScripts")
                   if row.findtext("File") == SCRIPT]
        self.assertEqual(len(scripts), 4)
        selected = set()
        for row in scripts:
            criterion = criteria[row.attrib["criteria"]]
            self.assertEqual(criterion.findtext("RuleSetInUse"), "RULESET_JAC_BANSEIKI")
            self.assertEqual(criterion.findtext("./ConfigurationValueMatches/ConfigurationId"), "MAP_SCRIPT")
            selected.add(criterion.findtext("./ConfigurationValueMatches/Value"))
        self.assertEqual(selected, map_files)
        self.assertEqual(len(map_files), 4)
        self.assertIn(SCRIPT, {row.text for row in mod.findall("./Files/File")})


if __name__ == "__main__":
    unittest.main()
