"""Static checks for the Banseiki FrontEnd leader filters and scenario."""

from __future__ import annotations

import json
from pathlib import Path
import re
import sqlite3
import unittest
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[3]
CONFIG = ROOT / "poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml"
SCENARIO = ROOT / "tools/civ6-autotest/scenarios/banseiki-ruleset.json"
DOMAIN = "Players:Expansion2_Players"

# Audit of all 37 installed official DLC/*.modinfo on 2026-09-29. This is the
# one FrontEnd leader whose <Leaders> definition is confined to gameplay
# actions requiring an official ruleset, despite its Expansion2_Players row.
AUDITED_UNSUPPORTED = {
    "LEADER_SEJONG": {
        "civilization": "CIVILIZATION_KOREA",
        "modinfo": "GreatBuilders/GreatBuilders.modinfo:36,38,61",
        "frontend": "GreatBuilders/Data/GreatBuilders_ConfigData_Korea.xml:7",
    },
}


class BanseikiRulesetStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.config = ET.parse(CONFIG).getroot()
        cls.queries = {
            row.attrib["QueryId"]: row.attrib["SQL"]
            for row in cls.config.findall("./Queries/Row")
        }
        cls.registered = {
            row.attrib["QueryId"]
            for row in cls.config.findall("./DomainValueFilterQueries/Row")
        }
        cls.parameters = {}
        for row in cls.config.findall("./QueryParameters/Row"):
            cls.parameters.setdefault(row.attrib["QueryId"], {})[int(row.attrib["Index"])] = (
                row.attrib["ConfigurationGroup"], row.attrib["ConfigurationId"]
            )

    def _choices(self, ruleset: str, player_id: int, pool: str) -> set[str]:
        original = {
            "LEADER_JAC_ODA_NOBUHIDE", "LEADER_JAC_TAKEDA_NOBUTORA",
            "LEADER_CLEOPATRA", "LEADER_GORGO", "LEADER_LUDWIG",
            *AUDITED_UNSUPPORTED,
        }
        config_values = {
            ("Game", "RULESET"): ruleset,
            ("Player", "PLAYER_ID"): player_id,
            ("Game", "JAC_BANSEIKI_HUMAN_POOL"): pool,
            # Japan maps skip the human filter; that case is covered in japan_map_scale tests.
            ("Map", "MAP_SCRIPT"): "Continents.lua",
        }
        with sqlite3.connect(":memory:") as database:
            database.execute("CREATE TABLE Players (Domain TEXT, LeaderType TEXT)")
            database.executemany(
                "INSERT INTO Players VALUES (?, ?)", ((DOMAIN, leader) for leader in original)
            )
            excluded = set()
            intersection = None
            for query_id in self.registered:
                indices = self.parameters[query_id]
                arguments = tuple(config_values[indices[index]] for index in range(1, len(indices) + 1))
                rows = database.execute(self.queries[query_id], arguments).fetchall()
                for domain, value, operation in rows:
                    self.assertEqual(domain, DOMAIN)
                    self.assertIn(value, original)
                    if operation == "difference":
                        excluded.add(value)
                    elif operation == "intersect":
                        if intersection is None:
                            intersection = set()
                        intersection.add(value)
                    else:
                        self.fail(f"Unexpected domain filter {operation!r}")
        leaders = original - excluded
        return leaders if intersection is None else leaders & intersection

    def test_audited_exclusion_is_exact_and_applies_to_every_banseiki_slot(self):
        query_id = "JAC_Banseiki_UnsupportedLeaderFilter"
        self.assertIn(query_id, self.registered)
        self.assertEqual(
            set(re.findall(r"'LEADER_[A-Z0-9_]+'", self.queries[query_id])),
            {f"'{leader}'" for leader in AUDITED_UNSUPPORTED},
        )
        original = {
            "LEADER_JAC_ODA_NOBUHIDE", "LEADER_JAC_TAKEDA_NOBUTORA",
            "LEADER_CLEOPATRA", "LEADER_GORGO", "LEADER_LUDWIG",
            *AUDITED_UNSUPPORTED,
        }
        expected = original - AUDITED_UNSUPPORTED.keys()
        for player_id in (0, 1, 2):
            with self.subTest(player_id=player_id):
                self.assertEqual(
                    self._choices("RULESET_JAC_BANSEIKI", player_id, "JAC_ALL_POOLS"), expected
                )
        self.assertEqual(
            self._choices("RULESET_JAC_BANSEIKI", 0, "JAC_BANSEIKI_ONLY"),
            {"LEADER_JAC_ODA_NOBUHIDE", "LEADER_JAC_TAKEDA_NOBUTORA"},
        )
        for player_id in (1, 2):
            self.assertEqual(
                self._choices("RULESET_JAC_BANSEIKI", player_id, "JAC_BANSEIKI_ONLY"), expected
            )
        self.assertEqual(self._choices("RULESET_EXPANSION_2", 0, "JAC_ALL_POOLS"), original)

    def test_duplicate_policy_and_runtime_scenario_cover_same_civilization(self):
        updates = {
            row.attrib["TargetId"]: row.attrib["TargetValue"]
            for row in self.config.findall("./ConfigurationUpdates/Row")
            if row.attrib.get("SourceValue") == "RULESET_JAC_BANSEIKI"
        }
        self.assertEqual(updates["NO_DUPLICATE_LEADERS"], "1")
        self.assertEqual(updates["NO_DUPLICATE_CIVILIZATIONS"], "0")

        steps = json.loads(SCENARIO.read_text(encoding="utf-8"))["steps"]
        configure_index = next(i for i, step in enumerate(steps) if step["action"] == "configure")
        start_index = next(i for i, step in enumerate(steps) if step["action"] == "start_game")
        players = steps[configure_index]["players"]
        self.assertTrue(any(
            left["civ"] == right["civ"] and left["leader"] != right["leader"]
            for i, left in enumerate(players) for right in players[i + 1:]
        ))
        # The ruleset's ConfigurationUpdates apply once the setup parameters are built,
        # so the check may run in HostGame after BuildHeadlessGameSetup (live 2026-09-29).
        frontend_checks = [
            step["code"] for step in steps[configure_index + 1:start_index]
            if step["action"] == "ui_lua" and step["state_name"] in ("FrontEnd", "HostGame")
        ]
        self.assertTrue(any(
            "GameConfiguration.GetValue('NO_DUPLICATE_LEADERS')" in code
            and "GameConfiguration.GetValue('NO_DUPLICATE_CIVILIZATIONS')" in code
            for code in frontend_checks
        ))
        self.assertTrue(any(
            step["action"] == "ui_lua" and step["state_name"] == "InGame"
            and "LEADER_GORGO" in step["code"] and "LEADER_PERICLES" in step["code"]
            and "CIVILIZATION_GREECE" in step["code"]
            for step in steps[start_index + 1:]
        ))


if __name__ == "__main__":
    unittest.main()
