"""Static gates for documents/spec/data/sengoku-events.json."""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EVENTS = json.loads((ROOT / "documents/spec/data/sengoku-events.json").read_text(encoding="utf-8"))["events"]
START = json.loads((ROOT / "documents/spec/data/japan-1534-start.json").read_text(encoding="utf-8"))
TREES_SQL = (ROOT / "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Trees.sql").read_text(encoding="utf-8")
FACTIONS = {f["id"] for f in START["factions"]}


class SengokuEventTests(unittest.TestCase):
    def test_count_and_unique_ids(self):
        self.assertTrue(30 <= len(EVENTS) <= 60, len(EVENTS))
        ids = [e["id"] for e in EVENTS]
        self.assertEqual(len(ids), len(set(ids)))

    def test_factions_are_the_22_seats(self):
        self.assertEqual(len(FACTIONS), 22)
        for e in EVENTS:
            for f in e["affected_factions"] + e["primary_factions"]:
                self.assertTrue(f == "ALL" or f in FACTIONS, (e["id"], f))
            self.assertTrue(e["primary_factions"], e["id"])

    def test_earlier_by_ids_exist_in_trees(self):
        for e in EVENTS:
            for r in e["earlier_by"]:
                self.assertIn(f"'{r['id']}'", TREES_SQL, (e["id"], r["id"]))

    def test_years_month_and_names(self):
        for e in EVENTS:
            self.assertTrue(1533 <= e["year"] <= 1600, e["id"])
            self.assertTrue(e["month"] is None or 1 <= e["month"] <= 12, e["id"])
            if e.get("year_end"):
                self.assertGreaterEqual(e["year_end"], e["year"], e["id"])
            self.assertTrue(all(e["name"][k] for k in ("ja", "en", "ru")), e["id"])
            self.assertTrue(e["sources"] and e["confidence"] in ("high", "medium", "low"), e["id"])

    def test_house_specific_has_generic_name(self):
        for e in EVENTS:
            if e["house_specific"]:
                self.assertTrue(e["house"] and all(e["generic_name"][k] for k in ("ja", "en", "ru")), e["id"])

    def test_required_events_present_and_every_faction_covered(self):
        ids = {e["id"] for e in EVENTS}
        self.assertTrue({"EV_TEPPO", "EV_CHRISTIAN", "EV_YAMAZAKI_KENCHI", "EV_NAGAO_TO_UESUGI", "EV_AIKAWA", "EV_IKUNO_SILVER"} <= ids)
        seen = {f for e in EVENTS for f in e["affected_factions"] + e["primary_factions"]}
        self.assertEqual(FACTIONS - seen, set())

    def test_no_dangling_event_references(self):
        ids = {e["id"] for e in EVENTS}
        text = json.dumps(EVENTS, ensure_ascii=False)
        self.assertEqual(set(re.findall(r"EV_[A-Z0-9_]+", text)) - ids, set())


if __name__ == "__main__":
    unittest.main()
