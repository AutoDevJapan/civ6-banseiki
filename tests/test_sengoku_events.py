"""Static gates for documents/spec/data/sengoku-events.json."""

from __future__ import annotations

import json
import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EVENTS = json.loads((ROOT / "documents/spec/data/sengoku-events.json").read_text(encoding="utf-8"))["events"]
START = json.loads((ROOT / "documents/spec/data/japan-1534-start.json").read_text(encoding="utf-8"))
TREES_SQL = (ROOT / "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Trees.sql").read_text(encoding="utf-8")
FACTIONS = {f["id"] for f in START["factions"]}
RESOURCES = json.loads((ROOT / "documents/spec/data/japan-map-resources.json").read_text(encoding="utf-8"))
sys.path.insert(0, str(ROOT / "tools/japan_map_scale"))
import generate  # noqa: E402  REVEAL_TECH / START_YEAR of the merged resource placement


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

    def test_month_set_when_date_text_names_a_month(self):
        # SSOT 3.2: events follow the historical month as closely as possible.
        for e in EVENTS:
            if re.search(r"\d+月", e["date_text"]):
                self.assertIsNotNone(e["month"], e["id"])

    def test_boost_and_advisor_popup(self):
        for e in EVENTS:
            b = e["boost"]
            # SSOT 3.2: factions without the related research get a boost toward it.
            self.assertEqual(b is None, not e["earlier_by"], e["id"])
            if b:
                self.assertIn(b, e["earlier_by"], e["id"])
                prefix = {"tech": ("TECH_JAC_SENGOKU_", "T"), "civic": ("CIVIC_JAC_SENGOKU_", "C")}[b["type"]]
                self.assertTrue(b["id"].startswith(prefix[0]) and b["short"].startswith(prefix[1]), e["id"])
            a = e["advisor_popup"]
            if a is not None:
                self.assertTrue(all(a[k] for k in ("ja", "en", "ru")), e["id"])
        self.assertTrue(any(e["advisor_popup"] and "和睦" in e["advisor_popup"]["ja"] for e in EVENTS))

    def test_ssot_decisions(self):
        # SSOT 3.2: no Sekigahara; Toyotomi is Oda after the rename; Taikō kenchi is not house-specific.
        ids = {e["id"] for e in EVENTS}
        self.assertNotIn("EV_SEKIGAHARA", ids)
        for e in EVENTS:
            self.assertTrue(e.get("house") is None or e["house"] in FACTIONS, e["id"])
        self.assertFalse(next(e for e in EVENTS if e["id"] == "EV_YAMAZAKI_KENCHI")["house_specific"])

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

    def test_scope_is_machine_readable(self):
        schema = json.loads((ROOT / "documents/spec/data/sengoku-events.json").read_text(encoding="utf-8"))["_schema"]
        scope = schema["scope"]
        self.assertEqual(scope["ruleset"], "RULESET_JAC_BANSEIKI")
        self.assertEqual(scope["pack"], "sengoku")
        self.assertEqual(scope["maps"], ["JAC_Japan_15km"])
        self.assertTrue((ROOT / "poc/JAC_PoC_JapanMapScale/Maps/JAC_Japan_15km.Civ6Map").exists())

    def test_resources_match_the_resource_placement(self):
        # Events must agree with japan-map-resources.json on site, start year, and start visibility.
        points = {}
        for g in RESOURCES["resource_groups"]:
            for p in g["points"]:
                points[(g["civ6_id"], tuple(p["coordinates"]))] = p
        trades = {(t["civ6_id"], tuple(t["coordinates"])): t for t in RESOURCES["trade_access"]}
        seen = 0
        for e in EVENTS:
            r = e.get("resource")
            if not r:
                continue
            seen += 1
            m = re.search(r"\((-?[\d.]+), (-?[\d.]+)\)$", r["location"])
            self.assertIsNotNone(m, e["id"])
            key = (r["resource_id"], (float(m[2]), float(m[1])))  # location is (lat, lon); data is [lon, lat]
            if r["action"] == "trade_access":
                self.assertTrue(key in trades, (e["id"], key))
                self.assertEqual(r["start_year"], trades[key]["available_from_year"], e["id"])
                continue
            self.assertTrue(key in points, (e["id"], key))
            p = points[key]
            deferred = generate._resource_is_deferred(p)
            if r["action"] == "reveal":
                self.assertTrue(deferred, e["id"])
                self.assertEqual(r["start_year"], generate.reveal_year(p), e["id"])
                tech = generate.REVEAL_TECH.get(p["id"])
                if tech:
                    self.assertIn(tech, {x["id"] for x in e["earlier_by"]}, e["id"])
            else:
                self.assertEqual(r["action"], "ownership", e["id"])
                self.assertFalse(deferred, e["id"])  # ownership means the resource is on the map from 1534
                if p.get("start_year") is not None:
                    self.assertEqual(r["start_year"], p["start_year"], e["id"])
        self.assertGreaterEqual(seen, 6)


if __name__ == "__main__":
    unittest.main()
