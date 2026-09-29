"""The Sengoku tree deletion must not silently cascade away official gameplay rows.

Builds the official Gathering Storm database from the local install (tools/sengoku_cascade_audit.py),
runs the Sengoku SQL in modinfo order with foreign keys on, and pins what the official-tree DELETEs
remove or null.  Skipped when Civ6 is not installed.
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def _load(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / f"tools/{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


AUDIT = _load("sengoku_cascade_audit")
GEN = sys.modules.get("generate_sengoku_trees") or _load("generate_sengoku_trees")
C11 = "CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES"
ALLIANCES = ("DIPLOACTION_ALLIANCE", "DIPLOACTION_ALLIANCE_MILITARY", "DIPLOACTION_ALLIANCE_RESEARCH",
             "DIPLOACTION_ALLIANCE_CULTURAL", "DIPLOACTION_ALLIANCE_ECONOMIC", "DIPLOACTION_ALLIANCE_RELIGIOUS",
             "DIPLOACTION_RENEW_ALLIANCE")
# Rows the tree deletion still removes (lost) or nulls (changed); documented in CASCADE_DROPS.
EXPECTED_RESIDUAL = {
    "Adjacency_YieldChanges": (0, 12), "Boosts": (126, 0), "Buildings": (0, 70), "CivicModifiers": (27, 0),
    "CivicPrereqs": (80, 0), "CivicQuotes": (113, 0), "CivicRandomCosts": (10, 0), "Civics_XP2": (6, 0),
    "Improvement_BonusYieldChanges": (25, 0), "Improvement_Tourism": (11, 0), "Improvement_ValidFeatures": (1, 0),
    "Improvements": (0, 26), "RandomAgendaCivicTags": (2, 0), "RandomAgendasForCivic": (1, 0),
    "Resource_Harvests": (0, 3), "Routes_XP2": (1, 0), "StartingBoostedCivics": (11, 0),
    "StartingBoostedTechnologies": (21, 0), "Technologies_XP2": (9, 0), "TechnologyModifiers": (14, 0),
    "TechnologyPrereqs": (95, 0), "TechnologyQuotes": (147, 0), "TechnologyRandomCosts": (12, 0), "Units": (0, 94),
}


@unittest.skipUnless(AUDIT.available(), "official Civ6 data not installed")
class SengokuCascadeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.official = AUDIT.official_db()
        cls.result = AUDIT.audit(cls.official)
        cls.final = AUDIT._run_sengoku(cls.official, True)

    def test_no_diplomatic_action_is_lost(self):
        for table in ("DiplomaticActions", "DiplomaticActions_XP1", "DiplomaticStateActions"):
            self.assertNotIn(table, self.result)
            before = self.official.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]
            self.assertEqual(self.final.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0], before, table)
        rows = dict(self.final.execute(
            "SELECT DiplomaticActionType, InitiatorPrereqCivic || '|' || TargetPrereqCivic FROM DiplomaticActions "
            f"WHERE DiplomaticActionType IN ({','.join('?' * len(ALLIANCES))})", ALLIANCES))
        self.assertEqual(rows, dict.fromkeys(ALLIANCES, f"{C11}|{C11}"))
        typed = {r[0] for r in self.final.execute("SELECT DiplomaticActionType FROM DiplomaticActions_XP1 "
                                                  "WHERE AllianceType IS NOT NULL")}
        self.assertEqual(typed, set(ALLIANCES[1:6]))

    def test_only_documented_rows_are_lost(self):
        self.assertEqual({t: (len(d["lost"]), len(d["changed"])) for t, d in self.result.items()}, EXPECTED_RESIDUAL)

    def test_no_governor_title_source_remains(self):
        # SSOT 3.2: governors are disabled in the Sengoku pack until redesigned.
        query = ("SELECT ModifierId FROM Modifiers WHERE ModifierType IN (SELECT ModifierType FROM DynamicModifiers "
                 "WHERE EffectType='EFFECT_ADJUST_PLAYER_GOVERNOR_POINTS')")
        sources = {r[0] for r in self.official.execute(query)}
        self.assertEqual(len(sources), 15)
        self.assertEqual(self.final.execute(query).fetchall(), [])
        for (table,) in self.final.execute("SELECT name FROM sqlite_master WHERE type='table'"):
            for column in (c[1] for c in self.final.execute(f'PRAGMA table_info("{table}")')):
                if column.lower() == "modifierid":
                    used = {r[0] for r in self.final.execute(f'SELECT "{column}" FROM "{table}"')}
                    self.assertEqual(used & sources, set(), table)

    def test_owner_decided_gates(self):
        # SSOT 3.2: hill farms from T01, corps and armies from T30.
        hills = self.final.execute("SELECT TerrainType, PrereqTech, PrereqCivic FROM Improvement_ValidTerrains "
                                   "WHERE ImprovementType='IMPROVEMENT_FARM' AND TerrainType LIKE '%_HILLS'").fetchall()
        self.assertEqual(sorted(hills), [("TERRAIN_GRASS_HILLS", "TECH_JAC_SENGOKU_IRRIGATION_WORKS", None),
                                         ("TERRAIN_PLAINS_HILLS", "TECH_JAC_SENGOKU_IRRIGATION_WORKS", None)])
        corps = self.final.execute("SELECT CommandType, PrereqTech, PrereqCivic FROM UnitCommands "
                                   "WHERE CommandType IN ('UNITCOMMAND_FORM_CORPS', 'UNITCOMMAND_FORM_ARMY')").fetchall()
        self.assertEqual(sorted(corps), [(c, "TECH_JAC_SENGOKU_REGIONAL_SUPPLY", None)
                                         for c in ("UNITCOMMAND_FORM_ARMY", "UNITCOMMAND_FORM_CORPS")])

    def test_nulled_items_are_not_available(self):
        hidden = {"Units": "CanTrain=0", "Buildings": "InternalOnly=1", "Improvements": "Buildable=0"}
        for table, flag in hidden.items():
            key = self.final.execute(f"PRAGMA table_info({table})").fetchone()[1]
            closed = {r[0] for r in self.final.execute(f"SELECT {key} FROM {table} WHERE {flag}")}
            for before, after in self.result[table]["changed"]:
                columns = {c for c in before if before[c] != after[c]}
                with self.subTest(item=before[key]):
                    # Kept units only lose obsolescence (they never become obsolete).
                    self.assertTrue(before[key] in closed or columns <= {"MandatoryObsoleteTech", "ObsoleteTech"},
                                    columns)

    def test_every_remap_matches_an_official_row_and_leaves_no_dangling_gate(self):
        for table, where, values, _, _ in GEN.CASCADE_REMAPS:
            keep = {k: v for k, v in where.items() if k not in values}
            conditions = [f"{k} IS NULL" if v is None else f"{k}=?" for k, v in values.items()]
            conditions += [f"{k} IN ({','.join('?' * len(v))})" if isinstance(v, tuple) else f"{k}=?"
                           for k, v in keep.items()]
            args = [v for v in values.values() if v is not None]
            args += [x for v in keep.values() for x in (v if isinstance(v, tuple) else (v,))]
            count = self.final.execute(f"SELECT COUNT(*) FROM {table} WHERE {' AND '.join(conditions)}",
                                       args).fetchone()[0]
            self.assertGreater(count, 0, f"{table} {where}")
        violations = [r for r in self.final.execute("PRAGMA foreign_key_check")
                      if r[2] in ("Technologies", "Civics")]
        self.assertEqual(violations, [])


class AllianceScenarioTests(unittest.TestCase):
    def test_live_scenario_checks_every_alliance_and_the_c11_grant(self):
        import json
        scenario = json.loads((ROOT / "tools/civ6-autotest/scenarios/sengoku-alliances.json").read_text(encoding="utf-8"))
        steps = scenario["steps"]
        configure = next(step for step in steps if step["action"] == "configure")
        self.assertEqual(configure["ruleset"], "RULESET_JAC_BANSEIKI")
        self.assertIn(len(configure["players"]), (2, 3))
        codes = [step.get("code", "") for step in steps]
        self.assertTrue(all(any(a in code for code in codes) for a in ALLIANCES))
        # C11 の有無だけを変えて比べる：友好宣言済みで、両者なし→開始側だけ→両者あり の順に有効性を確かめる
        friend = [i for i, s in enumerate(steps) if "SetHasDeclaredFriendship" in s.get("code", "")]
        grant = [i for i, s in enumerate(steps) if "SetCivic" in s.get("code", "")]
        checks = [i for i, s in enumerate(steps) if "IsDiplomaticActionValid" in s.get("code", "")]
        self.assertEqual(len(friend), 1)
        self.assertEqual(len(grant), 2)
        self.assertEqual(len(checks), 3)
        self.assertEqual(sorted(friend + grant + checks),
                         [friend[0], checks[0], grant[0], checks[1], grant[1], checks[2]])
        for i in friend + grant:
            self.assertEqual(steps[i]["state_name"], "GameCore_Tuner")
        for i in grant:
            self.assertIn(C11, steps[i]["code"])
        self.assertIn("ipairs({0})", steps[grant[0]]["code"])
        self.assertIn("ipairs({1})", steps[grant[1]]["code"])
        self.assertIn("valid==0", steps[checks[0]]["code"])
        self.assertIn("valid==0", steps[checks[1]]["code"])
        self.assertIn("valid>0", steps[checks[2]]["code"])
        decided = [s["code"] for s in steps if "EFFECT_ADJUST_PLAYER_GOVERNOR_POINTS" in s.get("code", "")]
        self.assertEqual(len(decided), 1)
        self.assertLess(codes.index(decided[0]), grant[0])
        for needle in ("GetGovernorPoints", "TECH_JAC_SENGOKU_IRRIGATION_WORKS", "TECH_JAC_SENGOKU_REGIONAL_SUPPLY",
                       "UNITCOMMAND_FORM_CORPS", "UNITCOMMAND_FORM_ARMY"):
            self.assertIn(needle, decided[0])


if __name__ == "__main__":
    unittest.main()
