"""Guards for the Q28 prosperity plan: the JSON is current and internally consistent."""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

TOOL = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOL))
import prosperity as p  # noqa: E402


class ProsperityPlanTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.plan = p.build()

    def test_json_is_current(self):
        self.assertEqual(json.loads(p.OUT.read_text(encoding="utf-8")), json.loads(json.dumps(self.plan)))

    def test_covers_22_factions_and_56_cities(self):
        self.assertEqual(len(self.plan["factions"]), 22)
        cities = [c for f in self.plan["factions"] for c in f["plan"]]
        self.assertEqual(len(cities), 56)
        self.assertEqual(len({c["id"] for c in cities}), 56)

    def test_every_city_is_garrisoned_and_capped_by_land(self):
        for f in self.plan["factions"]:
            for c in f["plan"]:
                with self.subTest(city=c["id"]):
                    self.assertEqual([u["where"] for u in c["units"]].count("center"), 1)
                    self.assertGreaterEqual(c["pop_15km"], 2)
                    # owned land tiles minus the centre and mountains
                    self.assertLessEqual(c["pop_15km"], max(2, c["land_15km"]))
                    self.assertEqual(c["capital"], "BUILDING_PALACE" in c["city_buildings"])
                    # a citizen or a district occupies a tile
                    self.assertLessEqual(c["pop_15km"] + len(c["districts"]), max(2, c["land_15km"]))
                    # one improvement per worked tile (population minus districts)
                    self.assertLessEqual(sum(c["improvements"].values()), c["pop_15km"] - len(c["districts"]))

    def test_only_buildings_and_units_kept_by_the_sengoku_trees(self):
        buildings = {"BUILDING_PALACE", "BUILDING_WALLS", "BUILDING_GRANARY", "BUILDING_MARKET",
                     "BUILDING_LIGHTHOUSE", "BUILDING_BARRACKS", "BUILDING_SHRINE", "BUILDING_TEMPLE",
                     "BUILDING_WORKSHOP", "BUILDING_LIBRARY"}
        units = {"UNIT_WARRIOR", "UNIT_SPEARMAN", "UNIT_ARCHER"}  # ungated in JAC_Sengoku_Unlocks.sql
        for f in self.plan["factions"]:
            for c in f["plan"]:
                used = set(c["city_buildings"]) | {b for d in c["districts"] for b in d["buildings"]}
                self.assertLessEqual(used, buildings)
                self.assertLessEqual({u["type"] for u in c["units"]}, units)

    def test_koku_shares_and_tiers(self):
        for province, shares in p.SPLIT_SHARES.items():
            self.assertAlmostEqual(sum(shares.values()), 1.0)
        tiers = {f["id"]: f["tier"] for f in self.plan["factions"]}
        self.assertEqual(tiers["OUCHI"], "A")
        self.assertEqual(tiers["ODA"], "D")
        # bigger houses never get fewer field units than smaller ones
        self.assertGreater(p.field_units("OUCHI"), p.field_units("ODA"))
        # Ryukyu and the Ainu are sized from sources too (owner decision 8b)
        self.assertEqual((tiers["RYUKYU"], tiers["AINU"]), ("C", "D"))
        shares = {f["id"]: f["direct_koku_share_pct"] for f in self.plan["factions"]}
        self.assertIsNone(shares["AINU"])  # population-derived, not koku
        self.assertAlmostEqual(sum(v for v in shares.values() if v is not None), 100, delta=0.5)
        self.assertTrue(all(len(spec["k"]) == 3 and spec["k"][0] <= spec["k"][1] <= spec["k"][2]
                            for spec in p.FACTIONS.values()))


if __name__ == "__main__":
    unittest.main()
