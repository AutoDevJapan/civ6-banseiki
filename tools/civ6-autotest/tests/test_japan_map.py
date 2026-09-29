import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from judge import judge
from metrics import TurnMetrics
from scenario import AutoplayStatus, Runner, ScenarioError, _configure_code, validate_scenario


SCENARIOS = Path(__file__).resolve().parents[1] / "scenarios"


class JapanMapScenarioTests(unittest.TestCase):
    def test_every_scale_scenario_is_valid(self):
        for km in (20, 15):
            with self.subTest(km=km):
                data = json.loads((SCENARIOS / f"japan-map-{km}km.json").read_text(encoding="utf-8"))
                validate_scenario(data)
                code = _configure_code(data["steps"][1])
                self.assertIn("MapConfiguration.SetValue('MAP_SCRIPT',", code)
                self.assertLess(code.index("MAP_SCRIPT"), code.index("SetMapSize"))
                self.assertIn("SetMaxMajorPlayers(22)", code)
                self.assertIn("SetValue('CITY_STATE_COUNT', 0)", code)
                self.assertEqual(data["steps"][5]["capture_as"], "JAPAN_STATE")
                self.assertEqual(data["steps"][9]["equals_capture"], "JAPAN_STATE")
                self.assertIn("city count", data["steps"][3]["code"])
                self.assertIn("if totalCities~=56 then", data["steps"][3]["code"])
                self.assertIn("unowned land plot", data["steps"][3]["code"])
                self.assertIn("settler in slot", data["steps"][3]["code"])

    def test_rejects_bad_map_file_and_metrics_flag(self):
        data = json.loads((SCENARIOS / "japan-map-20km.json").read_text(encoding="utf-8"))
        data["steps"][1]["map_file"] = "../outside.Civ6Map"
        with self.assertRaisesRegex(ScenarioError, "map_file"):
            validate_scenario(data)
        data["steps"][1]["map_file"] = "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_20km.Civ6Map"
        data["steps"][4]["metrics"] = "yes"
        with self.assertRaisesRegex(ScenarioError, "metrics"):
            validate_scenario(data)

    def test_capture_and_compare_lua_results(self):
        with tempfile.TemporaryDirectory() as folder:
            runner = Runner({}, Path(folder), Path(folder), Path(folder), 4318)
            with patch.object(runner, "_lua", side_effect=["22,56,7", "22,56,7", "21,55,8"]):
                runner._run_step({"action": "ui_lua", "state_name": "InGame", "code": "return 'state'",
                                  "capture_as": "JAPAN_STATE"})
                runner._run_step({"action": "ui_lua", "state_name": "InGame", "code": "return 'state'",
                                  "equals_capture": "JAPAN_STATE"})
                with self.assertRaisesRegex(ScenarioError, "snapshot mismatch"):
                    runner._run_step({"action": "ui_lua", "state_name": "InGame", "code": "return 'state'",
                                      "equals_capture": "JAPAN_STATE"})

    def test_turn_metrics_preserve_missing_samples(self):
        current = [10.0]
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "metrics.jsonl"
            metrics = TurnMetrics(path, 1, clock=lambda: current[0])
            metrics.start({"private_bytes": 100, "working_set_bytes": 80})
            current[0] = 11.5
            metrics.sample(2, {"private_bytes": 110, "working_set_bytes": 85})
            current[0] = 13.0
            metrics.sample(4, {"private_bytes": 120, "working_set_bytes": 90})
            metrics.sample(4, {"private_bytes": 120, "working_set_bytes": 90})
            rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
            self.assertEqual([row["turn"] for row in rows], [1, 2, 3, 4])
            self.assertEqual(rows[1]["duration_seconds"], 1.5)
            self.assertIsNone(rows[2]["duration_seconds"])
            self.assertIsNone(rows[3]["duration_seconds"])
            self.assertEqual((metrics.observed_turns, metrics.missed_turns), (2, 1))

    def test_autoplay_writes_turn_and_memory_samples(self):
        with tempfile.TemporaryDirectory() as folder:
            runner = Runner({}, Path(folder), Path(folder), Path(folder), 4318)
            statuses = iter((AutoplayStatus(2, True, False, 1, 0),
                             AutoplayStatus(3, False, True, 0, 0)))

            def lua(_state, code, _timeout=5.0):
                return "1|0" if "return tostring(startTurn)" in code else "true"

            with patch.object(runner, "_lua", side_effect=lua), patch.object(
                runner, "_autoplay_status", side_effect=lambda _deadline: next(statuses)
            ), patch.object(runner, "_close_options_menu"), patch.object(
                runner, "_dismiss_advisor"
            ), patch("scenario.single_civ_process", return_value=(123, 0.0)), patch(
                "scenario.process_memory", return_value={"private_bytes": 100, "working_set_bytes": 80}
            ), patch("scenario.time.sleep"):
                detail = runner._autoplay(2, 10, record_metrics=True)
            rows = [json.loads(line) for line in (Path(folder) / "autoplay-metrics-1.jsonl").read_text(
                encoding="utf-8").splitlines()]
            self.assertEqual([row["turn"] for row in rows], [1, 2, 3])
            self.assertEqual(rows[-1]["memory"]["private_bytes"], 100)
            self.assertIn("observed=2 missed=0", detail)

    def test_map_log_profile_detects_mod_errors(self):
        base = {"Lua.log": "", "Database.log": "",
                "Modding.log": ("403be889-1455-4de7-967b-ad54c0ff3431\n"
                                "Applying Component - JAC_JapanMapScale_Config (UpdateDatabase)\n"
                                "Applying Component - JAC_JapanMapScale_InGameText (UpdateText)\n")}
        self.assertTrue(all(check.passed for check in judge(base, "japan_map")))
        broken = dict(base, **{"Database.log": "ERROR JAC_JapanMapScale_InGameText invalid tag\n"})
        self.assertFalse(next(check.passed for check in judge(broken, "japan_map")
                              if check.name == "Japan map: Database.log"))
        def modding_passes(extra):
            logs = dict(base, **{"Modding.log": base["Modding.log"] + extra})
            return next(check.passed for check in judge(logs, "japan_map") if check.name == "Japan map: Modding.log")
        # Seen live: an official DLC warning followed by a JAC load line is not ours.
        self.assertTrue(modding_passes("LocalizedText - Loading Text/CivRoyaleScenario_Translations_ConfigText_China.xml\n"
                                       "Warning: LocalizedText - Failed loading XML.\n"
                                       "UISettingHandler - Loading JAC_PoC_CustomCivs.dep\n"))
        self.assertFalse(modding_passes("LocalizedText - Loading Text/JAC_JapanMapScale_Text.xml\n"
                                        "Warning: LocalizedText - Failed loading XML.\n"))
        broken = dict(base, **{"Modding.log": base["Modding.log"] + "ERROR JAC_JapanMapScale_Config failed\n"})
        self.assertFalse(next(check.passed for check in judge(broken, "japan_map")
                              if check.name == "Japan map: Modding.log"))


if __name__ == "__main__":
    unittest.main()
