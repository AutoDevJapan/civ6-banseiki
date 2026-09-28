import json
import hashlib
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from judge import LOG_NAMES, judge, slice_log_bytes
from scenario import validate_scenario


MODDING = """Target Mods (in no particular order):
dfa0717e-4d0b-4370-b693-27fc6758571b (JAC PoC Bundle 0)
Applying Component - JAC_PoC_Bundle0_Gameplay (UpdateDatabase)
Applying Component - JAC_PoC_Bundle0_Scripts (AddGameplayScripts)
"""
LUA = """JAC_PoC_Bundle0: [JAC_POC] loaded version=1
JAC_PoC_Bundle0: [JAC_POC] no saved state (new game)
JAC_PoC_Bundle0: [JAC_POC] turn=1 firstTurn=1 turnsSeen=1
JAC_PoC_Bundle0: [JAC_POC] turn=2 firstTurn=1 turnsSeen=2
JAC_PoC_Bundle0: [JAC_POC] turn=3 firstTurn=1 turnsSeen=3
JAC_PoC_Bundle0: [JAC_POC] turn=4 firstTurn=1 turnsSeen=4
JAC_PoC_Bundle0: [JAC_POC] loaded version=1
JAC_PoC_Bundle0: [JAC_POC] restored firstTurn=1 lastTurn=4 turnsSeen=4
JAC_PoC_Bundle0: [JAC_POC] turn=5 firstTurn=1 turnsSeen=5
"""
DATABASE = """[3897835.363] [Localization] ERROR: UNIQUE constraint failed: LocalizedText
[3897835.363] from file CurrentClickouts/Text/ClickoutText_en_US.xml
[3898056.334] [Configuration]: Passed Validation.
"""


class JudgeTests(unittest.TestCase):
    def sample(self):
        return {"Lua.log": LUA, "Database.log": DATABASE, "Modding.log": MODDING}

    def test_bundle0_success_and_unrelated_database_error(self):
        checks = judge(self.sample(), "bundle0")
        self.assertGreaterEqual(len(checks), 9)
        self.assertTrue(all(check.passed for check in checks), checks)

    def test_restored_state_mismatch_fails(self):
        logs = self.sample()
        logs["Lua.log"] = logs["Lua.log"].replace("lastTurn=4 turnsSeen=4", "lastTurn=3 turnsSeen=4")
        failed = {c.name for c in judge(logs) if not c.passed}
        self.assertIn("Bundle0: restored values", failed)

    def test_missing_action_fails(self):
        logs = self.sample()
        logs["Modding.log"] = logs["Modding.log"].replace("Applying Component - JAC_PoC_Bundle0_Scripts", "Skipped - JAC_PoC_Bundle0_Scripts")
        self.assertIn("Bundle0: script action", {c.name for c in judge(logs) if not c.passed})

    def test_generic_errors_include_stack_and_api_error(self):
        logs = self.sample()
        logs["Lua.log"] += "JAC_CustomCivs: [JAC_POC] result=api_error error=function expected\n"
        logs["Lua.log"] += "Runtime Error: bad argument\n  [C]: pcall\n  Mods/JAC_PoC_CustomCivs/Scripts.lua:4\n"
        failed = {c.name for c in judge(logs, "generic") if not c.passed}
        self.assertEqual(failed, {"Generic: [JAC_POC] errors", "Generic: JAC Runtime Error"})

    def test_error_none_is_not_a_failure(self):
        logs = self.sample()
        logs["Lua.log"] += "JAC: [JAC_POC] result=unavailable error=none\n"
        self.assertTrue(all(c.passed for c in judge(logs, "generic")))

    def test_database_error_with_mod_context_fails(self):
        logs = self.sample()
        logs["Database.log"] += "[100] ERROR: constraint failed\n[100] from file Mods/JAC_PoC_Bundle0/Data/a.sql\n"
        self.assertIn("Bundle0: database errors", {c.name for c in judge(logs) if not c.passed})

    def test_baseline_excludes_old_lines_and_handles_truncate(self):
        old = b"old error\n"
        mark = {"offset": len(old), "inode": 7,
                "prefix_sha256": hashlib.sha256(old[:256]).hexdigest()}
        self.assertEqual(slice_log_bytes(old + b"new line\n", mark, 7), "new line\n")
        self.assertEqual(slice_log_bytes(b"fresh\n", mark, 7), "fresh\n")
        self.assertEqual(slice_log_bytes(b"new file\n", mark, 8), "new file\n")

    def test_shipped_scenarios_validate_offline(self):
        folder = Path(__file__).resolve().parents[1] / "scenarios"
        names = {path.name for path in folder.glob("*.json")}
        self.assertEqual(names, {"bundle0-load-save.json", "custom-clans.json", "growth-notebooks.json"})
        for path in folder.glob("*.json"):
            with self.subTest(path=path.name):
                validate_scenario(json.loads(path.read_text(encoding="utf-8")))


if __name__ == "__main__":
    unittest.main()
