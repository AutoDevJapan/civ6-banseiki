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
        self.assertTrue(30 <= len(EVENTS) <= 70, len(EVENTS))
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
                # year_end is the historical span for display; the event still fires once at year/month.
                self.assertGreater(e["year_end"], e["year"], e["id"])
                ye = str(e["year_end"])
                self.assertTrue(ye in e["date_text"] or f"〜{ye[2:]}" in e["date_text"], e["id"])
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

    def test_renames_chain_in_date_order(self):
        # SSOT 3.2: Nagao->Uesugi, Matsudaira->Tokugawa, Oda->Hashiba->Toyotomi, each stage at its own event date.
        stages = {}
        for e in EVENTS:
            if r := e.get("rename"):
                self.assertIn(r["faction"], e["primary_factions"], e["id"])
                stages.setdefault(r["faction"], []).append(((e["year"], e["month"]), r["from"], r["to"], e["id"]))
        chains = {}
        for f, s in stages.items():
            s.sort()
            for a, b in zip(s, s[1:]):
                self.assertLess(a[0], b[0], (a[3], b[3]))
                self.assertEqual(a[2], b[1], (a[3], b[3]))
            chains[f] = [s[0][1]] + [x[2] for x in s]
        self.assertEqual(chains, {"NAGAO": ["長尾", "上杉"], "MATSUDAIRA": ["松平", "徳川"], "ODA": ["織田", "羽柴", "豊臣"]})

    def test_every_house_renames_by_research_only(self):
        # Owner decisions 2026-09-30 (SSOT 3.2; Q 1a supersedes 2a): every stage switches when the house researches
        # its succession civic; the year only boosts it.
        by = {e["id"]: e["rename"].get("by") for e in EVENTS if e.get("rename")}
        self.assertEqual({k: v and v["short"] for k, v in by.items()}, {
            "EV_NAGAO_TO_UESUGI": "C16", "EV_MATSUDAIRA_TOKUGAWA": "C16", "EV_HONNOJI": "C16", "EV_TOYOTOMI_SURNAME": "C24"})
        for e in EVENTS:
            if e.get("rename"):
                self.assertEqual(e["rename"]["by"], e["boost"], e["id"])
                self.assertIn(f"'{e['boost']['id']}'", TREES_SQL, e["id"])
        # C24 (Toyotomi) needs C16 (Hashiba), so research cannot skip the Hashiba stage.
        self.assertRegex(TREES_SQL, r"\('CIVIC_JAC_SENGOKU_TRUCE_ARBITRATION', 'CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS'\)")

    def test_rakuichi_is_two_events(self):
        # Owner decision 2026-09-30: Rokkaku's Ishidera new market (1549) and Oda's Kanō (1567) are separate events,
        # each for its own house; the name stays generic (SSOT 3.2).
        got = {e["id"]: (e["year"], e["month"], e["affected_factions"], e["house_specific"], e["boost"]["short"])
               for e in EVENTS if e["id"] in ("EV_RAKUICHI", "EV_ROKKAKU_RAKUICHI")}
        self.assertEqual(got, {"EV_RAKUICHI": (1567, 11, ["ODA"], False, "C17"),
                               "EV_ROKKAKU_RAKUICHI": (1549, 12, ["ROKKAKU"], False, "C15")})

    def test_catholicism_spreads_by_research_not_a_fixed_order(self):
        # Owner decision 2026-09-30: from the houses that researched C19/C13; no fixed geographic order.
        e = next(e for e in EVENTS if e["id"] == "EV_CHRISTIAN")
        self.assertEqual({r["short"] for r in e["earlier_by"]}, {"C19", "C13"})
        self.assertEqual(e["affected_factions"], ["ALL"])
        self.assertNotIn("→", e["gameplay_effect"])

    def test_former_domains_are_ai_targets_only(self):
        # Owner decision 2026-09-30: former domains are AI conquest targets; ownership never moves by itself.
        targeted = {}
        for e in EVENTS:
            for f, provinces in e.get("ai_targets", {}).items():
                self.assertIn(f, e["affected_factions"], e["id"])
                self.assertTrue(provinces and set(provinces) <= set(e["regions"]), e["id"])
                targeted.setdefault(e["id"], set()).add(f)
            r = e.get("resource") or {}
            if r.get("action") == "ownership":
                self.assertIn(r["owner_to"], targeted.get(e["id"], set()), e["id"])
        self.assertTrue({"EV_TAKEDA_FALL", "EV_MATSUDAIRA_KANTO", "EV_KANTO_KANREI"} <= set(targeted))
        self.assertNotIn("所有移転の候補", json.dumps(EVENTS, ensure_ascii=False))

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
        expected = {
            "EV_IKUNO_SILVER": ("RESOURCE_SILVER", "生野・但馬 (35.167, 134.790)", "reveal", 1542, None),
            "EV_HIRADO_PORTUGUESE": ("RESOURCE_NITER", "平戸・肥前 (33.368, 129.553)", "trade_access", 1586, None),
            "EV_MORI_IWAMI": ("RESOURCE_SILVER", "石見銀山・石見 (35.103, 132.437)", "ownership", 1526, "MORI"),
            "EV_AMAGO_FALL": ("RESOURCE_IRON", "奥出雲・出雲 (35.199, 133.092)", "ownership", 1534, "MORI"),
            "EV_SADO_SUBJUGATION": ("RESOURCE_JAC_GOLD", "西三川・佐渡 (37.819, 138.273)", "reveal", 1593, None),
            "EV_AIZU_LACQUER": ("RESOURCE_JAC_LACQUER", "会津 (37.490, 139.930)", "reveal", 1590, None),
            "EV_AIKAWA": ("RESOURCE_JAC_GOLD", "相川・佐渡 (38.032, 138.235)", "reveal", 1596, None),
        }
        actual = {
            e["id"]: (r["resource_id"], r["location"], r["action"], r["start_year"], r.get("owner_to"))
            for e in EVENTS
            if (r := e.get("resource"))
        }
        self.assertEqual(actual, expected)
        for e in EVENTS:
            r = e.get("resource")
            if not r:
                continue
            if r["action"] == "ownership":
                # The transfer target must be machine-readable, one of the event's primary factions.
                self.assertIn(r["owner_to"], FACTIONS & set(e["primary_factions"]), e["id"])
            m = re.search(r"\((-?[\d.]+), (-?[\d.]+)\)$", r["location"])
            self.assertIsNotNone(m, e["id"])
            key = (r["resource_id"], (float(m[2]), float(m[1])))  # location is (lat, lon); data is [lon, lat]
            if r["action"] == "trade_access":
                self.assertTrue(key in trades, (e["id"], key))
                self.assertEqual(r["start_year"], trades[key]["available_from_year"], e["id"])
                # Owner decision 2026-09-30: trade access opens by research; the event boosts that research.
                self.assertEqual(e["boost"]["id"], trades[key]["unlock_civic"], e["id"])
                self.assertEqual(generate.trade_access_status(trades[key]), "unlocked_by_research", e["id"])
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
                    # SSOT 3.2: the event does not reveal the site; its boost is the research that does.
                    self.assertEqual((e["boost"] or {}).get("id"), tech, e["id"])
            else:
                self.assertEqual(r["action"], "ownership", e["id"])
                self.assertFalse(deferred, e["id"])  # ownership means the resource is on the map from 1534
                if p.get("start_year") is not None:
                    self.assertEqual(r["start_year"], p["start_year"], e["id"])


if __name__ == "__main__":
    unittest.main()
