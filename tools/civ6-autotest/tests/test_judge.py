import json
import hashlib
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from judge import LOG_NAMES, judge, read_since, slice_log_bytes, snapshot_logs
from scenario import Runner, validate_scenario


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
GROWTH_MODDING = """7696e937-2044-4f3c-8e13-4e27632e2710
Applying Component - JAC_GrowthUI_Text (UpdateText)
Applying Component - JAC_GrowthUI_Gameplay (AddGameplayScripts)
Applying Component - JAC_GrowthUI_InGameUI (AddUserInterfaces)
Applying Component - JAC_GrowthUI_PartialScreenHooks (ReplaceUIScript)
Applying Component - JAC_GrowthUI_UIFiles (ImportFiles)
"""
GROWTH_LUA = """PartialScreenHooks: [JAC_POC] notebooks button attached parent=ButtonStack
JAC_PoC_GrowthUI_Gameplay: [JAC_POC] no saved state (new game)
JAC_PoC_GrowthUI_Gameplay: [JAC_POC] assigned unit=1 level=1 xp=0 personality=JAC_PERSONALITY_BRAVE trait=JAC_TRAIT_VALIANT equipment=JAC_ITEM_POC_BLADE,JAC_ITEM_POC_CLOAK artifact=JAC_ARTIFACT_POC_RELIC player=0
JAC_PoC_GrowthUI_Gameplay: [JAC_POC] initialized count=1
JAC_PoC_GrowthUI_Gameplay: [JAC_POC] unit=1 level=1 xp=20 personality=JAC_PERSONALITY_BRAVE trait=JAC_TRAIT_VALIANT equipment=JAC_ITEM_POC_BLADE,JAC_ITEM_POC_CLOAK artifact=JAC_ARTIFACT_POC_RELIC player=0 turn=1
JAC_Notebooks: [JAC_POC] notebooks path lookup found=true
JAC_Notebooks: [JAC_POC] notebooks opened
JAC_Notebooks: [JAC_POC] character journal shown count=1
JAC_PoC_GrowthUI_Gameplay: [JAC_POC] unit=1 level=2 xp=10 personality=JAC_PERSONALITY_BRAVE trait=JAC_TRAIT_VALIANT equipment=JAC_ITEM_POC_BLADE,JAC_ITEM_POC_CLOAK artifact=JAC_ARTIFACT_POC_RELIC player=0 turn=2
JAC_PoC_GrowthUI_Gameplay: [JAC_POC] unit=1 level=2 xp=30 personality=JAC_PERSONALITY_BRAVE trait=JAC_TRAIT_VALIANT equipment=JAC_ITEM_POC_BLADE,JAC_ITEM_POC_CLOAK artifact=JAC_ARTIFACT_POC_RELIC player=0 turn=3
JAC_PoC_GrowthUI_Gameplay: [JAC_POC] unit=1 level=2 xp=50 personality=JAC_PERSONALITY_BRAVE trait=JAC_TRAIT_VALIANT equipment=JAC_ITEM_POC_BLADE,JAC_ITEM_POC_CLOAK artifact=JAC_ARTIFACT_POC_RELIC player=0 turn=4
JAC_PoC_GrowthUI_Gameplay: [JAC_POC] unit=1 level=2 xp=70 personality=JAC_PERSONALITY_BRAVE trait=JAC_TRAIT_VALIANT equipment=JAC_ITEM_POC_BLADE,JAC_ITEM_POC_CLOAK artifact=JAC_ARTIFACT_POC_RELIC player=0 turn=5
JAC_Notebooks: [JAC_POC] notebooks opened
JAC_Notebooks: [JAC_POC] character journal shown count=1
JAC_PoC_GrowthUI_Gameplay: [JAC_POC] restored unit=1 level=2 xp=70 personality=JAC_PERSONALITY_BRAVE trait=JAC_TRAIT_VALIANT equipment=JAC_ITEM_POC_BLADE,JAC_ITEM_POC_CLOAK artifact=JAC_ARTIFACT_POC_RELIC player=0
JAC_PoC_GrowthUI_Gameplay: [JAC_POC] restored count=1
JAC_Notebooks: [JAC_POC] notebooks opened
JAC_Notebooks: [JAC_POC] character journal shown count=1
JAC_PoC_GrowthUI_Gameplay: [JAC_POC] unit=1 level=3 xp=5 personality=JAC_PERSONALITY_BRAVE trait=JAC_TRAIT_VALIANT equipment=JAC_ITEM_POC_BLADE,JAC_ITEM_POC_CLOAK artifact=JAC_ARTIFACT_POC_RELIC player=0 turn=6
"""


def two_unit_growth_logs():
    rows = []
    for line in GROWTH_LUA.splitlines():
        if "initialized count=1" in line or "restored count=1" in line or "character journal shown count=1" in line:
            line = line.replace("count=1", "count=2")
        rows.append(line)
        if "JAC_PoC_GrowthUI_Gameplay: [JAC_POC]" in line and "unit=1 " in line:
            rows.append(line.replace("unit=1 ", "unit=2 ")
                        .replace("JAC_PERSONALITY_BRAVE", "JAC_PERSONALITY_CALM")
                        .replace("JAC_TRAIT_VALIANT", "JAC_TRAIT_FIERCE")
                        .replace("JAC_ITEM_POC_BLADE,JAC_ITEM_POC_CLOAK", "JAC_ITEM_POC_BLADE")
                        .replace("JAC_ARTIFACT_POC_RELIC", "none"))
    return {"Lua.log": "\n".join(rows) + "\n", "Modding.log": GROWTH_MODDING, "Database.log": ""}


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

    def test_missing_jac_icon_or_texture_fails(self):
        logs = self.sample()
        logs["Lua.log"] += 'DiplomacyRibbon: Could not find icon in CivilizationIcon.UpdateIcon: icon="ICON_CIVILIZATION_CHINA", iconSize=22\n'
        self.assertTrue(all(c.passed for c in judge(logs, "generic")))
        logs["Lua.log"] += 'DiplomacyRibbon: Could not find icon in CivilizationIcon.UpdateIcon: icon="ICON_CIVILIZATION_JAC_AINU", iconSize=22\n'
        failed = {c.name for c in judge(logs, "generic") if not c.passed}
        self.assertEqual(failed, {"Generic: JAC missing texture"})
    def test_missing_jac_texture_fails(self):
        logs = self.sample()
        logs["Lua.log"] += "LoadScreen: Failed to load background image texture: LEADER_JAC_MORI_MOTONARI_BACKGROUND\n"
        logs["Lua.log"] += "LoadScreen: We are lacking a texture for LEADER_JAC_MORI_MOTONARI_NEUTRAL\n"
        logs["Lua.log"] += "LoadScreen: We are lacking a texture for LEADER_HOJO_NEUTRAL\n"
        checks = {c.name: c for c in judge(logs, "generic")}
        self.assertFalse(checks["Generic: JAC missing texture"].passed)
        self.assertEqual(checks["Generic: JAC missing texture"].detail, "2 matching lines")

    def test_error_none_is_not_a_failure(self):
        logs = self.sample()
        logs["Lua.log"] += "JAC: [JAC_POC] result=unavailable error=none\n"
        self.assertTrue(all(c.passed for c in judge(logs, "generic")))

    def test_database_error_with_mod_context_fails(self):
        logs = self.sample()
        logs["Database.log"] += "[100] ERROR: constraint failed\n[100] from file Mods/JAC_PoC_Bundle0/Data/a.sql\n"
        self.assertIn("Bundle0: database errors", {c.name for c in judge(logs) if not c.passed})

    def test_growth_success_and_restored_mismatch(self):
        logs = {"Lua.log": GROWTH_LUA, "Modding.log": GROWTH_MODDING, "Database.log": ""}
        checks = judge(logs, "growth", expected_units=1)
        self.assertGreaterEqual(len(checks), 12)
        self.assertTrue(all(c.passed for c in checks), checks)
        logs["Lua.log"] = GROWTH_LUA.replace("restored unit=1 level=2 xp=70", "restored unit=1 level=2 xp=50")
        failed = {c.name for c in judge(logs, "growth", expected_units=1) if not c.passed}
        self.assertIn("Growth: restored rows and journal", failed)

    def test_growth_expected_two_units_in_assignment_restore_and_journal(self):
        logs = two_unit_growth_logs()
        self.assertTrue(all(check.passed for check in judge(logs, "growth", expected_units=2)))
        missing_assignment = {**logs, "Lua.log": logs["Lua.log"].replace(
            next(line for line in logs["Lua.log"].splitlines() if "assigned unit=2 " in line) + "\n", "")}
        self.assertIn("Growth: new-game assignment", {c.name for c in judge(
            missing_assignment, "growth", expected_units=2) if not c.passed})
        missing_restore = {**logs, "Lua.log": logs["Lua.log"].replace(
            next(line for line in logs["Lua.log"].splitlines() if "restored unit=2 " in line) + "\n", "")}
        self.assertIn("Growth: restored rows and journal", {c.name for c in judge(
            missing_restore, "growth", expected_units=2) if not c.passed})
        wrong_journal = {**logs, "Lua.log": logs["Lua.log"].replace(
            "character journal shown count=2", "character journal shown count=1")}
        self.assertIn("Growth: restored rows and journal", {c.name for c in judge(
            wrong_journal, "growth", expected_units=2) if not c.passed})

    def test_growth_ignores_unrelated_game_error_and_catches_target_ui_error(self):
        logs = {"Lua.log": GROWTH_LUA +
                "BaseGamePanel: UI.DataError: unrelated game panel\n"
                "Runtime Error: base game failure\n"
                "  Assets/UI/BaseGamePanel.lua:21\n"
                "BaseGamePanel: Cannot determine if partial screen for BaseGamePanel\n"
                "JAC_Notebooks: [JAC_POC] notebooks opened\n",
                "Modding.log": GROWTH_MODDING, "Database.log": ""}
        self.assertTrue(all(check.passed for check in judge(logs, "growth", expected_units=1)),
                        judge(logs, "growth", expected_units=1))
        logs["Lua.log"] += "UI.DataError: Cannot determine if partial screen for JAC_Notebooks\n"
        failed = {check.name for check in judge(logs, "growth", expected_units=1) if not check.passed}
        self.assertEqual(failed, {"Growth: partial-screen errors"})
        logs["Lua.log"] += "Runtime Error: JAC UI failure\n  Mods/JAC_PoC_GrowthUI/JAC_Notebooks.lua:4\n"
        failed = {check.name for check in judge(logs, "growth", expected_units=1) if not check.passed}
        self.assertEqual(failed, {"Growth: partial-screen errors", "Generic: JAC Runtime Error"})

    def test_japan_map_fails_on_congress_dependent_ui_error(self):
        logs = {"Lua.log": "Runtime Error: unrelated\n  BaseGamePanel.lua:21\n",
                "Database.log": "",
                "Modding.log": "403be889-1455-4de7-967b-ad54c0ff3431\n"
                               "Applying Component - JAC_JapanMapScale_InGameText (UpdateText)\n"}
        self.assertTrue(all(check.passed for check in judge(logs, "japan_map")))
        logs["Lua.log"] += "Runtime Error: attempt to index a nil value\n  ActionPanel_Expansion2.lua:44\n"
        self.assertIn("Japan map: GS Congress paths",
                      {check.name for check in judge(logs, "japan_map") if not check.passed})
        logs["Lua.log"] = "UI.DataError: WorldCongressPopup invalid stage\n"
        self.assertIn("Japan map: GS Congress paths",
                      {check.name for check in judge(logs, "japan_map") if not check.passed})
        for name in ("GovernorPanel.lua:723", "CityPanel_Expansion2.lua:11"):
            logs["Lua.log"] = f"Runtime Error: attempt to index a nil value\n  {name}\n"
            self.assertIn("Japan map: GS Congress paths",
                          {check.name for check in judge(logs, "japan_map") if not check.passed})

    def test_baseline_excludes_old_lines_and_handles_truncate(self):
        old = b"old error\n"
        mark = {"offset": len(old), "inode": 7,
                "prefix_sha256": hashlib.sha256(old[:256]).hexdigest()}
        self.assertEqual(slice_log_bytes(old + b"new line\n", mark, 7), "new line\n")
        self.assertEqual(slice_log_bytes(b"fresh\n", mark, 7), "fresh\n")
        self.assertEqual(slice_log_bytes(b"new file\n", mark, 8), "new file\n")

    def test_baseline_detects_same_inode_rewrite_past_old_offset(self):
        with tempfile.TemporaryDirectory() as root:
            log_dir = Path(root)
            for name in LOG_NAMES:
                (log_dir / name).write_bytes(b"")
            lua = log_dir / "Lua.log"
            original = b"P" * 256 + b"M" * 256 + b"T" * 256
            lua.write_bytes(original)
            baseline = snapshot_logs(log_dir)
            with lua.open("ab") as handle:
                handle.write(b"appended\n")
            self.assertEqual(read_since(log_dir, baseline)["Lua.log"], "appended\n")

            replacement = b"P" * 256 + b"R" * 256 + b"U" * 256 + b"fresh\n"
            lua.write_bytes(replacement)
            self.assertEqual(lua.stat().st_ino, baseline["files"]["Lua.log"]["inode"])
            self.assertEqual(read_since(log_dir, baseline)["Lua.log"], replacement.decode("ascii"))

    def test_shipped_scenarios_validate_offline(self):
        folder = Path(__file__).resolve().parents[1] / "scenarios"
        names = {path.name for path in folder.glob("*.json")}
        self.assertEqual(names, {
            "banseiki-ruleset.json", "bundle0-load-save.json", "clans-notebooks.json",
            "clans-house-names.json", "clans-both-ai.json", "custom-clans.json",
            "leader-portraits.json",
            "growth-notebooks.json", "sengoku-trees.json", "sengoku-starting-knowledge.json", "sengoku-calendar.json",
            "unification-victory.json", "unification-registry-15km.json",
            "sengoku-house-names-oda.json", "sengoku-house-names-takeda.json", "sengoku-alliances.json",
            "house-crests.json",
            "japan-map-20km.json", "japan-map-15km.json",
            "japan-map-20km-takeda.json", "japan-map-15km-takeda.json",
            "japan-1534-relations.json", "probe-ally-vision.json", "probe-alliance-type.json", "probe-alliance-expiry.json",
            "japan-1534-relations-settlers.json",
            "japan-1534-feud-ui.json",
            "japan-resources.json",
            "japan-no-world-congress.json", "japan-city-names.json",
            "japan-map-setup-screen.json", "japan-map-15km-settlers.json",
            "japan-map-15km-200turn.json",
            "japan-scale-baseline-15km.json", "japan-prosperous-15km.json",
        })
        for path in folder.glob("*.json"):
            with self.subTest(path=path.name):
                validate_scenario(json.loads(path.read_text(encoding="utf-8")))
        growth = json.loads((folder / "growth-notebooks.json").read_text(encoding="utf-8"))
        self.assertEqual(growth["expected_units"], 2)
        del growth["expected_units"]
        with self.assertRaisesRegex(RuntimeError, "expected_units"):
            validate_scenario(growth)


class AutoplayTests(unittest.TestCase):
    def test_event_and_transient_inactive_state_do_not_complete_or_restart_early(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        statuses = iter(("1|true|true|3|0", "2|false|true|0|0",
                         "3|false|true|0|0", "4|false|false|0|0",
                         "4|false|true|0|0"))
        commands = []

        def fake_lua(state, code, timeout=5.0):
            commands.append((state, code))
            if "return tostring(startTurn)" in code:
                return "1|0"
            if "tostring(AutoplayManager.IsActive())" in code:
                return next(statuses)
            return "true"

        with patch.object(runner, "_lua", side_effect=fake_lua), patch.object(runner, "_close_diplomacy", return_value=[]), patch.object(
            runner, "_dismiss_advisor", return_value=False
        ), patch.object(runner, "_close_options_menu", return_value=False), patch("scenario.time.sleep"):
            detail = runner._autoplay(3, 10)

        self.assertIn("1->4", detail)
        self.assertEqual(sum("SetActive(true)" in code for _, code in commands), 1)
        self.assertEqual(sum("tostring(AutoplayManager.IsActive())" in code for _, code in commands), 5)


if __name__ == "__main__":
    unittest.main()
