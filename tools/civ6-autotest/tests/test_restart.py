import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from click import InputError
from restart import (enabled_mod_folders, missing_atlas_textures, newest_mod_file, restart_reason, single_civ_process,
                     wait_for_civ_window)
from scenario import Runner, ScenarioError, validate_scenario
from tuner import LuaResponseUnknown


MOD_ID = "ba3a1c58-9287-4654-a22f-85f9e6841e9b"


class FreshnessTests(unittest.TestCase):
    def test_freshness_decision_uses_strictly_newer_mod_file(self):
        newest = (Path("changed.dds"), 200.0)
        self.assertEqual(restart_reason(None, None), "Civ6 is not running")
        self.assertIn("changed.dds", restart_reason(newest, 199.0))
        self.assertIsNone(restart_reason(newest, 200.0))
        self.assertIsNone(restart_reason(newest, 201.0))
        self.assertIsNone(restart_reason(None, 100.0))

    def test_mod_scan_resolves_folder_and_finds_newest_file(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            mods = base / "Mods"
            folder = mods / "MyMod"
            folder.mkdir(parents=True)
            info = folder / "MyMod.modinfo"
            info.write_text(f'<Mod id="{MOD_ID}" version="1"/>', encoding="utf-8")
            newer = folder / "Art" / "icon.dds"
            newer.parent.mkdir()
            newer.write_bytes(b"test")
            os.utime(info, (100, 100))
            os.utime(newer, (200, 200))
            scenario = {"steps": [{"action": "enable_mods", "ids": [MOD_ID.upper()]}]}
            folders = enabled_mod_folders(scenario, mods)
            self.assertEqual(folders, [folder.resolve()])
            self.assertEqual(newest_mod_file(folders), (newer, 200.0))
            self.assertIn("icon.dds", restart_reason(newest_mod_file(folders), 150.0))

    def test_missing_or_duplicate_enabled_mod_is_rejected(self):
        with tempfile.TemporaryDirectory() as root:
            mods = Path(root)
            scenario = {"steps": [{"action": "enable_mods", "ids": [MOD_ID]}]}
            with self.assertRaisesRegex(InputError, "not installed"):
                enabled_mod_folders(scenario, mods)
            for name in ("a", "b"):
                folder = mods / name
                folder.mkdir()
                (folder / "mod.modinfo").write_text(f'<Mod id="{MOD_ID}"/>', encoding="utf-8")
            with self.assertRaisesRegex(InputError, "Duplicate"):
                enabled_mod_folders(scenario, mods)


    def test_atlas_images_missing_from_the_mod_packages_are_reported(self):
        # ages-screens-01: the crest atlas rows loaded but the installed BLP predated the crests, so every crest was "?".
        with tempfile.TemporaryDirectory() as root:
            folder = Path(root) / "MyMod"
            (folder / "Data").mkdir(parents=True)
            (folder / "Data" / "Icons.xml").write_text(
                '<GameInfo><IconTextureAtlases><Row Name="A" IconSize="36" Filename="JAC_Old36.dds"/>'
                '<Row Name="B" IconSize="36" Filename="JAC_Crests36.dds"/><Row Name="C" IconSize="36" '
                'Filename="JAC_Loose36.dds"/></IconTextureAtlases></GameInfo>', encoding="utf-8")
            (folder / "Data" / "Text.xml").write_text("<GameInfo><LocalizedText/></GameInfo>", encoding="utf-8")
            (folder / "Textures").mkdir()
            (folder / "Textures" / "JAC_Loose36.dds").write_bytes(b"dds")
            blps = folder / "Platforms" / "Windows" / "BLPs"
            blps.mkdir(parents=True)
            (blps / "Icons.blp").write_bytes(b"header JAC_Old36 pixels")
            self.assertEqual(missing_atlas_textures([folder]), ["MyMod: JAC_Crests36"])
            (blps / "Icons.blp").write_bytes(b"JAC_Old36 JAC_Crests36")
            self.assertEqual(missing_atlas_textures([folder]), [])

    def test_stale_art_stops_before_restart(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (base / name).write_text("", encoding="utf-8")
            runner = Runner({"name": "stale art", "steps": [{"action": "pause", "seconds": 0.1}]},
                            base / "output", base, base, 4318)
            with patch("scenario.enabled_mod_folders", return_value=[base]), patch(
                "scenario.missing_atlas_textures", return_value=["MyMod: JAC_Crests36"]
            ), patch.object(runner, "_restart_game") as restart, patch.object(runner, "_run_step") as step:
                self.assertEqual(runner.run(), 1)
            restart.assert_not_called()
            step.assert_not_called()
            records = [json.loads(line) for line in (runner.output / "steps.jsonl").read_text(
                encoding="utf-8").splitlines()]
            self.assertIn("build_art.py", records[-1]["detail"])
            self.assertIn("JAC_Crests36", records[-1]["detail"])


class RestartActionTests(unittest.TestCase):
    def setUp(self):
        # Runner tests default to a simulated running game. Individual lifecycle
        # tests override this patch when testing absence or multiple processes.
        active_game = patch("scenario.single_civ_process", return_value=(12, 100.0))
        active_game.start()
        self.addCleanup(active_game.stop)

    def test_restart_action_validation(self):
        valid = {"name": "restart", "steps": [{"action": "restart_game", "timeout": 300}]}
        validate_scenario(valid)
        for step in ({"action": "restart_game", "timeout": 0},
                     {"action": "restart_game", "timeout": float("inf")},
                     {"action": "restart_game", "ids": [MOD_ID]}):
            with self.subTest(step=step), self.assertRaises(ScenarioError):
                validate_scenario({"name": "restart", "steps": [step]})

    def test_auto_restart_precedes_first_step_and_log_baseline(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            logs = base / "logs"
            logs.mkdir()
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (logs / name).write_text("", encoding="utf-8")
            scenario = {"name": "freshness", "steps": [{"action": "pause", "seconds": 0.1}]}
            runner = Runner(scenario, base / "output", logs, base, 4318)
            events = []

            def restart(_timeout):
                events.append("restart")
                (logs / "Lua.log").write_text("after restart\n", encoding="utf-8")
                return "ready"

            def step(_step):
                events.append("step")
                return "done"

            with patch("scenario.enabled_mod_folders", return_value=[base]), patch(
                "scenario.newest_mod_file", return_value=(base / "mod.dds", 200.0)
            ), patch("scenario.single_civ_process", side_effect=[(12, 100.0), (13, 300.0)]), patch.object(
                runner, "_restart_game", side_effect=restart
            ), patch.object(runner, "_run_step", side_effect=step), patch.object(
                runner, "_wait"
            ), patch("scenario.require_single_civ_process"):
                self.assertEqual(runner.run(), 0)
            self.assertEqual(events, ["restart", "step"])
            records = [json.loads(line) for line in (runner.output / "steps.jsonl").read_text(
                encoding="utf-8").splitlines()]
            self.assertEqual([(r["action"], r["status"]) for r in records[:4]], [
                ("start", "OK"), ("auto_restart", "START"), ("auto_restart", "OK"),
                ("1/1 pause", "START")])
            self.assertIn("mod.dds", records[1]["detail"])
            self.assertEqual(runner.baseline["files"]["Lua.log"]["offset"],
                             (logs / "Lua.log").stat().st_size)

    def test_no_auto_restart_skips_freshness_query(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (base / name).write_text("", encoding="utf-8")
            runner = Runner({"name": "debug", "steps": [{"action": "pause", "seconds": 0.1}]},
                            base / "output", base, base, 4318, auto_restart=False)
            with patch("scenario.enabled_mod_folders") as folders, patch.object(
                runner, "_restart_game"
            ) as restart, patch.object(runner, "_wait"), patch.object(
                runner, "_run_step", return_value="done"
            ), patch("scenario.require_single_civ_process"):
                self.assertEqual(runner.run(), 0)
            folders.assert_not_called()
            restart.assert_not_called()

    def test_still_stale_after_restart_stops_before_first_step(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (base / name).write_text("", encoding="utf-8")
            runner = Runner({"name": "future mtime", "steps": [{"action": "pause", "seconds": 0.1}]},
                            base / "output", base, base, 4318)
            with patch("scenario.enabled_mod_folders", return_value=[base]), patch(
                "scenario.newest_mod_file", return_value=(base / "future.dds", 500.0)
            ), patch("scenario.single_civ_process", side_effect=[(12, 100.0), (13, 300.0)]), patch.object(
                runner, "_restart_game", return_value="ready"
            ), patch.object(runner, "_run_step") as step:
                self.assertEqual(runner.run(), 1)
            step.assert_not_called()
            records = [json.loads(line) for line in (runner.output / "steps.jsonl").read_text(
                encoding="utf-8").splitlines()]
            self.assertEqual((records[-1]["action"], records[-1]["status"]), ("run", "FAIL"))
            self.assertIn("still stale", records[-1]["detail"])

    def test_restart_action_launches_and_clicks_verified_window(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        with patch("scenario.single_civ_process", return_value=None), patch(
            "scenario.launch_via_steam"
        ) as launch, patch("scenario.wait_for_civ_window") as window, patch(
            "scenario.click_fraction"
        ) as click, patch("scenario.time.sleep"), patch.object(runner, "_wait_frontend_ready") as wait:
            runner._run_step({"action": "restart_game"})
        launch.assert_called_once_with()
        window.assert_called_once()
        click.assert_called_once_with(0.5, 0.15)
        wait.assert_called_once()

    def test_frontend_probe_reconnects_after_unknown_read_only_reply(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        with patch.object(runner, "_lua", side_effect=[LuaResponseUnknown("reply lost"), "true"]) as lua, patch.object(
            runner, "_disconnect"
        ) as disconnect, patch("scenario.time.sleep"):
            runner._wait_frontend_ready(10)
        self.assertEqual(lua.call_count, 2)
        disconnect.assert_called_once_with()

    def test_frontend_probe_clicks_again_if_movie_still_plays(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        clock = [0.0]
        clicked = [False]

        def click(*_args):
            clicked[0] = True

        with patch("scenario.time.monotonic", side_effect=lambda: clock[0]), patch(
            "scenario.time.sleep", side_effect=lambda seconds: clock.__setitem__(0, clock[0] + seconds)
        ), patch.object(runner, "_lua", side_effect=lambda *_: "true" if clicked[0] else "false"), patch(
            "scenario.click_fraction", side_effect=click
        ) as left_click:
            runner._wait_frontend_ready(30, skip_opening=True)
        left_click.assert_called_once_with(0.5, 0.15)

    def test_steam_launch_retries_when_prior_session_blocks_first_uri(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        clock = [0.0]
        windows = [0]

        def wait(short_deadline):
            clock[0] = short_deadline
            windows[0] += 1
            if windows[0] <= 9:
                raise InputError("Steam has not started Civ VI")
            return 123

        with patch("scenario.time.monotonic", side_effect=lambda: clock[0]), patch(
            "scenario.single_civ_process", return_value=None
        ), patch("scenario.launch_via_steam") as launch, patch(
            "scenario.wait_for_civ_window", side_effect=wait
        ):
            runner._launch_steam_and_wait(120)
        self.assertEqual(launch.call_count, 2)

    def test_four_launch_requests_still_wait_for_delayed_window_until_deadline(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        clock = [0.0]

        def wait(short_deadline):
            clock[0] = short_deadline
            if clock[0] < 230:
                raise InputError("Steam launch is still pending")
            return 123

        with patch("scenario.time.monotonic", side_effect=lambda: clock[0]), patch(
            "scenario.single_civ_process", return_value=None
        ), patch("scenario.launch_via_steam") as launch, patch(
            "scenario.wait_for_civ_window", side_effect=wait
        ):
            runner._launch_steam_and_wait(420)
        self.assertEqual(launch.call_count, 4)
        self.assertEqual(clock[0], 230)

    def test_exhausted_launch_requests_fail_at_the_supplied_deadline(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        clock = [0.0]

        def wait(short_deadline):
            clock[0] = short_deadline
            raise InputError("no window")

        with patch("scenario.time.monotonic", side_effect=lambda: clock[0]), patch(
            "scenario.single_civ_process", return_value=None
        ), patch("scenario.launch_via_steam") as launch, patch(
            "scenario.wait_for_civ_window", side_effect=wait
        ):
            with self.assertRaisesRegex(ScenarioError, "within the deadline"):
                runner._launch_steam_and_wait(420)
        self.assertEqual(launch.call_count, 4)
        self.assertEqual(clock[0], 420)

    def test_single_process_waits_for_exiting_process_to_disappear(self):
        with patch("restart._civ_process_ids", side_effect=[{1, 2}, {1, 2}, {2}]), patch(
            "restart.process_start_time", return_value=100.0
        ), patch("restart.time.sleep"):
            self.assertEqual(single_civ_process(), (2, 100.0))

    def test_single_process_still_refuses_when_duplicates_persist(self):
        with patch("restart._civ_process_ids", return_value={1, 2}), patch("restart.time.sleep"):
            with self.assertRaisesRegex(InputError, "Multiple Civ VI processes"):
                single_civ_process(settle=0)

    def test_window_wait_ignores_transient_duplicate_processes(self):
        with patch("restart.single_civ_process", side_effect=[
            InputError("Multiple Civ VI processes found (2)"), (12, 100.0)
        ]), patch("restart.find_civ_window", return_value=123), patch("restart.time.sleep"):
            self.assertEqual(wait_for_civ_window(10**12), 123)

    def test_steam_wait_does_not_launch_again_during_process_overlap(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        clock = [0.0]

        def wait(short_deadline):
            clock[0] = short_deadline
            if clock[0] == 5:
                raise InputError("launcher still opening")
            return 123

        with patch("scenario.time.monotonic", side_effect=lambda: clock[0]), patch(
            "scenario.single_civ_process", side_effect=[
                None, InputError("Multiple Civ VI processes found (2)")
            ]
        ), patch("scenario.launch_via_steam") as launch, patch(
            "scenario.wait_for_civ_window", side_effect=wait
        ):
            runner._launch_steam_and_wait(120)
        launch.assert_called_once_with()

    def test_first_restart_action_can_start_stopped_game_with_auto_disabled(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (base / name).write_text("", encoding="utf-8")
            runner = Runner({"name": "manual restart", "steps": [{"action": "restart_game"}]},
                            base / "output", base, base, 4318, auto_restart=False)
            with patch.object(runner, "_restart_game", return_value="ready") as restart, patch.object(
                runner, "_wait"
            ) as wait, patch("scenario.require_single_civ_process") as require:
                self.assertEqual(runner.run(), 0)
            restart.assert_called_once()
            wait.assert_not_called()
            require.assert_not_called()

    def test_restart_requests_ui_quit_before_steam_launch(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        events = []
        with patch("scenario.single_civ_process", side_effect=[(12, 100.0), None, None]), patch.object(
            runner, "_states", return_value=["FrontEnd"]
        ), patch.object(runner, "_lua", side_effect=lambda *_: events.append("quit")) as lua, patch(
            "scenario.wait_for_exit", return_value=True
        ), patch("scenario.terminate_civ_process") as terminate, patch(
            "scenario.launch_via_steam", side_effect=lambda: events.append("launch")
        ), patch("scenario.wait_for_civ_window"), patch("scenario.click_fraction"), patch(
            "scenario.time.sleep"
        ), patch.object(runner, "_wait_frontend_ready"):
            runner._restart_game(360)
        self.assertEqual(events, ["quit", "launch"])
        self.assertIn("Events.UserConfirmedClose()", lua.call_args.args[1])
        terminate.assert_not_called()

    def test_restart_forces_only_same_process_after_grace(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        with patch("scenario.single_civ_process", side_effect=[(12, 100.0), (12, 100.0), None, None]), patch.object(
            runner, "_states", side_effect=InputError("no window")
        ), patch("scenario.request_civ_window_close") as close, patch(
            "scenario.wait_for_exit", side_effect=[False, True]
        ), patch("scenario.terminate_civ_process") as terminate, patch(
            "scenario.launch_via_steam"
        ), patch("scenario.wait_for_civ_window"), patch("scenario.click_fraction"), patch(
            "scenario.time.sleep"
        ), patch.object(runner, "_wait_frontend_ready"):
            runner._restart_game(360)
        close.assert_called_once_with()
        terminate.assert_called_once_with(12, 100.0)

    def test_restart_without_process_launches_then_waits_for_frontend(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        events = []
        with patch("scenario.single_civ_process", return_value=None), patch(
            "scenario.terminate_civ_process"
        ) as terminate, patch("scenario.request_civ_window_close") as close, patch.object(
            runner, "_states"
        ) as states, patch("scenario.launch_via_steam", side_effect=lambda: events.append("launch")), patch(
            "scenario.wait_for_civ_window", side_effect=lambda *_: events.append("window")
        ), patch("scenario.click_fraction", side_effect=lambda *_: events.append("skip")), patch(
            "scenario.time.sleep"
        ), patch.object(runner, "_wait_frontend_ready", side_effect=lambda *a, **k: events.append("frontend")):
            runner._restart_game(360)
        self.assertEqual(events, ["launch", "window", "skip", "frontend"])
        for unused in (terminate, close, states):
            unused.assert_not_called()

    def test_restart_still_refuses_when_several_processes_persist(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        with patch("scenario.single_civ_process", side_effect=InputError("Multiple Civ VI processes found (2)")), patch(
            "scenario.terminate_civ_process"
        ) as terminate, patch("scenario.launch_via_steam") as launch:
            with self.assertRaises(InputError):
                runner._restart_game(360)
        terminate.assert_not_called()
        launch.assert_not_called()

    def test_run_launches_civ_when_not_running_and_first_step_needs_game(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (base / name).write_text("", encoding="utf-8")
            runner = Runner({"name": "cold", "steps": [{"action": "pause", "seconds": 0.1}]},
                            base / "output", base, base, 4318, auto_restart=False)
            with patch("scenario.single_civ_process", return_value=None), patch(
                "scenario.require_single_civ_process"
            ), patch.object(runner, "_restart_game", return_value="ready") as restart, patch.object(
                runner, "_wait"
            ) as wait, patch.object(runner, "_run_step", return_value="ok"):
                runner.run()
            restart.assert_called_once_with(360)
            wait.assert_not_called()  # the restart already proved FrontEnd answers


if __name__ == "__main__":
    unittest.main()
