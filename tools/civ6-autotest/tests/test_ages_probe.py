"""Static checks for scenarios/ages-probe.json (documents/research/sengoku-ages.md §11).

The probe must record the previous-era score of all 22 players after every autoplay chunk, and a chunk
must never be longer than the shortest era it checks for, so no era change can slip between two records.
"""
from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

SCENARIO = Path(__file__).resolve().parents[1] / "scenarios" / "ages-probe.json"


class AgesProbeTests(unittest.TestCase):
    def setUp(self):
        self.steps = json.loads(SCENARIO.read_text(encoding="utf-8"))["steps"]

    def test_every_chunk_is_followed_by_a_full_previous_era_record(self):
        chunks = [i for i, step in enumerate(self.steps) if step["action"] == "autoplay"]
        self.assertGreaterEqual(len(chunks), 3)
        for i in chunks:
            with self.subTest(step=i + 1):
                self.assertTrue(self.steps[i]["allow_human_elimination"])
                record = self.steps[i + 1]
                self.assertEqual(record["action"], "ui_lua")
                code = record["code"]
                self.assertIn("for pid = 0, 21 do", code)
                self.assertIn("GetPlayerPreviousEraScoreBreakdown(pid)", code)
                self.assertIn(f"local chunk = {self.steps[i]['turns']}", code)
                # Lua errors fail the step; a pcall would turn them into text that nothing checks.
                self.assertNotIn("pcall", code)
                self.assertIn("error('listed '", code)

    def test_chunk_length_is_guarded_by_the_minimum_era_length(self):
        longest = max(step["turns"] for step in self.steps if step["action"] == "autoplay")
        guard = next(step["code"] for step in self.steps
                     if step["action"] == "ui_lua" and "GameEraMinimumTurns" in step["code"])
        self.assertEqual(int(re.search(r"GameEraMinimumTurns or 0\) < (\d+)", guard).group(1)), longest)
        self.assertIn("error('wrong ruleset", guard)
        self.assertIn("error('alive majors", guard)

    def test_api_listing_separates_not_enumerable_from_absent(self):
        # An engine object whose methods cannot be walked lists nothing; that must not read as "API absent" (§11 L2).
        listing = next(step["code"] for step in self.steps
                       if step["action"] == "ui_lua" and "local function list(label, obj)" in step["code"])
        self.assertIn("NOT_ENUMERABLE (0 keys)", listing)
        self.assertIn("NOT_ENUMERABLE ERR:", listing)
        self.assertIn("'enumerated ' .. total", listing)

    def test_moments_cover_all_players(self):
        self.assertEqual(self.steps[-1]["action"], "ui_lua")
        self.assertIn("for pid = 0, 21 do", self.steps[-1]["code"])
        self.assertIn("GetAllMomentsData(pid, 0)", self.steps[-1]["code"])


if __name__ == "__main__":
    unittest.main()
