import csv
from contextlib import redirect_stdout
from io import StringIO
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from metrics import TurnMetrics, process_memory
from scenario import AutoplayStatus, Runner, ScenarioError, validate_scenario


class MeasurementTests(unittest.TestCase):
    def test_fast_forward_csv_files_warn_against_continuous_performance_comparison(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            runner = Runner({"name": "short", "test_mode": {"skipped_turns": 48}, "steps": []},
                            base, base, base, 4318)
            runner.save_load_metrics.record("save", "synthetic-turn-51", 51, 400, 2.0)
            recorder = TurnMetrics(base / "turns.jsonl", 49, test_mode=True)
            recorder.start({"working_set_bytes": 80, "private_bytes": 100})
            recorder.sample(50, {"working_set_bytes": 85, "private_bytes": 110})
            for name in ("save-load-metrics.csv", "turns.csv"):
                with (base / name).open(encoding="utf-8", newline="") as handle:
                    rows = list(csv.DictReader(handle))
                self.assertGreater(len(rows), 0)
                for row in rows:
                    self.assertEqual(row["test_mode"], "True")
                    self.assertEqual(row["continuous_performance_valid"], "False")

    def test_csv_and_summary_keep_skipped_turns_and_untimed_intervals_explicit(self):
        clock = [10.0]
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "turns.jsonl"
            recorder = TurnMetrics(path, 1, clock=lambda: clock[0])
            recorder.start({"working_set_bytes": 80, "private_bytes": 100})
            clock[0] = 11.5
            recorder.sample(2, {"working_set_bytes": 85, "private_bytes": 110})
            clock[0] = 13.0
            recorder.sample(4, {"working_set_bytes": 90, "private_bytes": 120})
            clock[0] = 16.0
            recorder.sample(5, {"working_set_bytes": 95, "private_bytes": 140})
            recorder.sample(5, {"working_set_bytes": 999, "private_bytes": 999})
            with recorder.csv_path.open(encoding="utf-8", newline="") as handle:
                rows = list(csv.DictReader(handle))
            self.assertEqual([row["turn"] for row in rows], ["1", "2", "3", "4", "5"])
            self.assertEqual([row["duration_seconds"] for row in rows], ["", "1.5", "", "", "3.0"])
            self.assertEqual([row["private_bytes"] for row in rows], ["100", "110", "", "120", "140"])
            self.assertEqual(rows[2]["observed"], "False")
            summary = recorder.summary()
            self.assertEqual((summary["observed_turns"], summary["missed_turns"]), (3, 1))
            self.assertEqual(summary["duration_seconds"],
                             {"count": 2, "min": 1.5, "median": 2.25, "p95": 3.0, "max": 3.0})
            self.assertEqual(summary["private_bytes"],
                             {"count": 4, "min": 100, "median": 115.0, "p95": 140, "max": 140,
                              "first": 100, "last": 140, "growth": 40})
            with self.assertRaisesRegex(ValueError, "backwards"):
                recorder.sample(4, {"working_set_bytes": 0, "private_bytes": 0})

    def test_first_interval_starts_before_autoplay_response(self):
        with tempfile.TemporaryDirectory() as folder:
            recorder = TurnMetrics(Path(folder) / "turns.jsonl", 1, clock=lambda: 12.0, started_at=8.0)
            recorder.start({"working_set_bytes": 10, "private_bytes": 20})
            recorder.sample(2, {"working_set_bytes": 11, "private_bytes": 21})
            self.assertEqual(recorder.rows[1]["duration_seconds"], 4.0)

    @unittest.skipUnless(os.name == "nt", "Windows process counters")
    def test_stdlib_memory_api_samples_a_real_process(self):
        sample = process_memory(os.getpid())
        self.assertEqual(set(sample), {"working_set_bytes", "private_bytes"})
        self.assertGreater(sample["working_set_bytes"], 0)
        self.assertGreater(sample["private_bytes"], 0)

    def test_save_load_measure_actual_file_and_wait_until_ready(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            runner = Runner({}, base, base, base, 4318)
            clock = [10.0]

            def lua(state, code, timeout=5.0):
                if "return tostring(Game.GetCurrentGameTurn())" in code:
                    return "21"
                if "Network.SaveGame" in code:
                    runner._save_path(runner._save_name({"name": "checkpoint"})).write_bytes(b"save" * 100)
                if "Network.LoadGame" in code:
                    clock[0] += 0.5
                return "true"

            def wait_ready(*args, **kwargs):
                clock[0] += 2.0

            with patch.object(runner, "_lua", side_effect=lua), patch.object(runner, "_wait", side_effect=wait_ready), \
                    patch.object(runner, "_close_options_menu"), patch.object(runner, "_dismiss_advisor"), \
                    patch.object(runner, "_loadscreen_count", return_value=0), \
                    patch.object(runner, "_enter_game_after_loadscreen", side_effect=wait_ready), \
                    patch("scenario.time.monotonic", side_effect=lambda: clock[0]):
                save_detail = runner._run_step({"action": "save", "name": "checkpoint", "metrics": True})
                load_detail = runner._run_step({"action": "load", "name": "checkpoint", "metrics": True})
            with (base / "save-load-metrics.csv").open(encoding="utf-8", newline="") as handle:
                rows = list(csv.DictReader(handle))
            self.assertEqual([row["action"] for row in rows], ["save", "load"])
            self.assertEqual([row["turn"] for row in rows], ["21", "21"])
            self.assertEqual([row["save_bytes"] for row in rows], ["400", "400"])
            self.assertEqual([row["duration_seconds"] for row in rows], ["2.0", "2.5"])
            self.assertIn("save_bytes=400", save_detail)
            self.assertIn("load_seconds=2.5", load_detail)
            self.assertEqual(runner.save_load_metrics.summary()["load_seconds"]["median"], 2.5)

    def test_result_keeps_partial_measurements_on_failed_run(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            output = base / "run"
            runner = Runner({"name": "partial", "judge_profile": "generic", "steps": [
                {"action": "autoplay", "turns": 3, "metrics": True}
            ]}, output, base, base, 4318, auto_restart=False)
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (base / name).write_text("", encoding="utf-8")
            statuses = iter((AutoplayStatus(2, True, False, 2, 0),))

            def status(deadline):
                try:
                    return next(statuses)
                except StopIteration:
                    raise ScenarioError("fixture stopped after one turn")

            def lua(state, code, timeout=5.0):
                return "1|0" if "return tostring(startTurn)" in code else "true"

            with redirect_stdout(StringIO()), patch("scenario.require_single_civ_process"), \
                    patch("scenario.single_civ_process", return_value=(123, 0)), \
                    patch("scenario.process_memory", return_value={"working_set_bytes": 80, "private_bytes": 100}), \
                    patch.object(runner, "_lua", side_effect=lua), patch.object(runner, "_wait"), \
                    patch.object(runner, "_autoplay_status", side_effect=status), \
                    patch.object(runner, "_close_diplomacy", return_value=[]), \
                    patch.object(runner, "_close_options_menu"), patch.object(runner, "_dismiss_advisor"), \
                    patch("scenario.time.sleep"):
                self.assertEqual(runner.run(), 1)
            result = json.loads((output / "result.json").read_text(encoding="utf-8"))
            self.assertIs(result["passed"], False)
            self.assertEqual(result["completed_steps"], 0)
            self.assertIn("fixture stopped after one turn", result["failure"])
            run = result["metrics"]["autoplay"][0]
            self.assertEqual((run["pid"], run["requested_turns"], run["last_turn"]), (123, 3, 2))
            self.assertIs(run["completed"], False)
            self.assertEqual(result["metrics"]["turn_summary"]["observed_turns"], 1)
            self.assertTrue((output / run["csv"]).is_file())

    def test_save_load_flags_require_boolean(self):
        for action in ("save", "load"):
            with self.subTest(action=action), self.assertRaisesRegex(ScenarioError, "metrics must be boolean"):
                validate_scenario({"name": "bad", "steps": [{"action": action, "name": "test", "metrics": "true"}]})

    def test_result_separates_continuous_memory_from_post_load_memory(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            runner = Runner({}, base, base, base, 4318)
            clock = [10.0]
            continuous = TurnMetrics(base / "continuous.jsonl", 1, clock=lambda: clock[0])
            continuous.start({"working_set_bytes": 80, "private_bytes": 100})
            clock[0] = 12.0
            continuous.sample(2, {"working_set_bytes": 400, "private_bytes": 500})
            post_load = TurnMetrics(base / "post-load.jsonl", 301, clock=lambda: clock[0])
            post_load.start({"working_set_bytes": 40, "private_bytes": 50})
            clock[0] = 13.0
            post_load.sample(302, {"working_set_bytes": 45, "private_bytes": 60})
            runner.turn_metrics = [
                {"recorder": continuous, "pid": 123, "requested_turns": 1, "group": "CONTINUOUS"},
                {"recorder": post_load, "pid": 123, "requested_turns": 1, "group": "POST_LOAD"},
            ]
            runner._write_result()
            result = json.loads((base / "result.json").read_text(encoding="utf-8"))
            groups = result["metrics"]["groups"]
            self.assertEqual(groups["CONTINUOUS"]["private_bytes"]["growth"], 400)
            self.assertEqual(groups["POST_LOAD"]["private_bytes"]["growth"], 10)
            self.assertEqual(result["metrics"]["turn_summary"]["private_bytes"]["growth"], -40)

    def test_interruption_is_recorded_and_summary_write_error_does_not_replace_it(self):
        for fail_write in (False, True):
            with self.subTest(fail_write=fail_write), tempfile.TemporaryDirectory() as folder:
                base = Path(folder)
                runner = Runner({"name": "interrupted", "judge_profile": "generic", "steps": [
                    {"action": "autoplay", "turns": 1}
                ]}, base / "run", base, base, 4318, auto_restart=False)
                for name in ("Lua.log", "Database.log", "Modding.log"):
                    (base / name).write_text("", encoding="utf-8")
                with redirect_stdout(StringIO()), patch("scenario.require_single_civ_process"), \
                        patch("scenario.single_civ_process", return_value=(123, 0)), \
                        patch.object(runner, "_wait"), \
                        patch.object(runner, "_run_step", side_effect=KeyboardInterrupt("fixture interruption")):
                    if fail_write:
                        with patch.object(runner, "_write_result", side_effect=OSError("fixture disk error")):
                            with self.assertRaisesRegex(KeyboardInterrupt, "fixture interruption"):
                                runner.run()
                    else:
                        with self.assertRaisesRegex(KeyboardInterrupt, "fixture interruption"):
                            runner.run()
                self.assertIn("KeyboardInterrupt", runner.failure)
                if not fail_write:
                    result = json.loads((runner.output / "result.json").read_text(encoding="utf-8"))
                    self.assertIs(result["passed"], False)
                    self.assertIn("fixture interruption", result["failure"])

    def test_metric_groups_are_validated(self):
        for value, enabled in (("../bad", True), (1, True), ("CONTINUOUS", False)):
            with self.subTest(value=value, enabled=enabled), self.assertRaisesRegex(ScenarioError, "metrics_group"):
                validate_scenario({"name": "bad group", "steps": [
                    {"action": "autoplay", "turns": 1, "metrics": enabled, "metrics_group": value}
                ]})
