import json
from contextlib import redirect_stdout
from io import StringIO
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import click as civ_click
from judge import snapshot_logs
from scenario import Runner, ScenarioError, _safe_relative_path, validate_scenario
from tuner import MAX_LUA_CODE_BYTES, LuaExecutionError, LuaResponseUnknown, LuaResult, TunerError


class ScenarioSafetyTests(unittest.TestCase):
    def test_load_wait_sends_one_escape_even_if_ingame_state_exists(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        deadline = civ_click.time.monotonic() + 1
        with patch.object(runner, "_states", return_value=["InGame"]), patch.object(
            runner, "_lua", side_effect=["false", "true"]
        ) as run_lua, patch.object(runner, "_loadscreen_count", return_value=1), patch.object(
            runner, "_close_options_menu"
        ), patch.object(runner, "_dismiss_advisor"), patch(
            "scenario.press_key"
        ) as press, patch("scenario.time.sleep"):
            runner._enter_game_after_loadscreen(0, 1, deadline=deadline)
        self.assertEqual(run_lua.call_count, 2)
        press.assert_called_once_with(civ_click.VK_ESCAPE, deadline=deadline)

    def test_load_wait_never_sends_escape_before_load_complete_log(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        with patch.object(runner, "_loadscreen_count", return_value=0), patch.object(
            runner, "_states"
        ) as states, patch("scenario.press_key") as press, patch("scenario.time.sleep"):
            with self.assertRaisesRegex(ScenarioError, "entering the game"):
                runner._enter_game_after_loadscreen(0, 0.02)
        states.assert_not_called()
        press.assert_not_called()

    def test_load_wait_sends_begin_game_escape_only_once(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        with patch.object(runner, "_states", side_effect=[[], [], ["InGame"]]), patch.object(
            runner, "_lua", return_value="true"
        ), patch.object(runner, "_loadscreen_count", return_value=1), patch.object(
            runner, "_close_options_menu"
        ), patch.object(runner, "_dismiss_advisor"), patch(
            "scenario.press_key"
        ) as press, patch("scenario.time.sleep"):
            runner._enter_game_after_loadscreen(0, 1)
        press.assert_called_once_with(civ_click.VK_ESCAPE, deadline=unittest.mock.ANY)

    def test_load_wait_reconnects_when_frontend_state_list_is_stale(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        client = Mock()
        runner.client = client
        with patch.object(runner, "_states", side_effect=[["FrontEnd"], ["InGame"]]), patch.object(
            runner, "_lua", return_value="true"
        ), patch.object(runner, "_loadscreen_count", return_value=1), patch.object(
            runner, "_close_options_menu"
        ), patch.object(runner, "_dismiss_advisor"), patch(
            "scenario.press_key"
        ) as press, patch("scenario.time.sleep"):
            runner._enter_game_after_loadscreen(0, 1)
        press.assert_called_once_with(civ_click.VK_ESCAPE, deadline=unittest.mock.ANY)
        client.close.assert_called_once()
        self.assertIsNone(runner.client)

    def test_load_wait_timeout_never_repeats_escape(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        with patch.object(runner, "_states", return_value=[]), patch.object(
            runner, "_loadscreen_count", return_value=1
        ), patch("scenario.press_key") as press, patch("scenario.time.sleep"):
            with self.assertRaisesRegex(ScenarioError, "entering the game"):
                runner._enter_game_after_loadscreen(0, 0.02)
        press.assert_called_once_with(civ_click.VK_ESCAPE, deadline=unittest.mock.ANY)

    def test_run_creates_missing_output_parents_but_rejects_existing_run(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            logs = base / "logs"
            logs.mkdir()
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (logs / name).write_text("", encoding="utf-8")
            output = base / "missing" / "nested" / "run"
            runner = Runner({"name": "empty logs", "judge_profile": "generic", "steps": [
                {"action": "wait_for", "condition": {"type": "state", "name": "FrontEnd"}}
            ]}, output, logs, base, 4318)
            with redirect_stdout(StringIO()), patch.object(runner, "_wait"):
                self.assertEqual(runner.run(), 0)
                with self.assertRaises(FileExistsError):
                    runner.run()
            self.assertTrue((output / "steps.jsonl").is_file())

    def test_screenshot_paths_reject_drives_absolute_and_parent_escape(self):
        rejected = (
            "D:shot.png", "D:\\shot.png", "\\\\server\\share\\shot.png",
            "/absolute/shot.png", "\\rooted.png", "../shot.png", "folder/../shot.png",
            "folder\\..\\shot.png",
        )
        for value in rejected:
            with self.subTest(value=value), self.assertRaises(ScenarioError):
                _safe_relative_path(value)
        self.assertEqual(_safe_relative_path("screens/shot.png"), Path("screens/shot.png"))

    def test_screenshot_rejects_resolved_path_outside_output(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            output = base / "output"
            output.mkdir()
            outside = base / "outside"
            outside.mkdir()
            try:
                (output / "link").symlink_to(outside, target_is_directory=True)
            except (OSError, NotImplementedError) as exc:
                self.skipTest(f"directory symlinks unavailable: {exc}")
            runner = Runner({}, output, base, base, 4318)
            with patch.object(runner, "_close_options_menu"), patch.object(
                runner, "_dismiss_advisor"
            ), patch("scenario.capture_civ_window") as capture:
                with self.assertRaisesRegex(ScenarioError, "escapes output"):
                    runner._run_step({"action": "screenshot", "path": "link/shot.png"})
                capture.assert_not_called()

    def test_screenshot_fails_if_capture_finishes_after_deadline(self):
        with tempfile.TemporaryDirectory() as root:
            output = Path(root) / "output"
            output.mkdir()
            runner = Runner({}, output, Path(root), Path(root), 4318)
            clock = [10.0]

            def slow_capture(_path, _crop, *, deadline):
                self.assertAlmostEqual(deadline, 10.01)
                clock[0] = deadline + 0.01

            with patch.object(runner, "_close_options_menu"), patch.object(
                runner, "_dismiss_advisor"
            ), patch("scenario.capture_civ_window", side_effect=slow_capture) as capture, patch(
                "scenario.time.monotonic", side_effect=lambda: clock[0]
            ):
                with self.assertRaisesRegex(ScenarioError, "deadline expired"):
                    runner._run_step({"action": "screenshot", "path": "shot.png", "timeout": 0.01})
            capture.assert_called_once()

    def test_nonfinite_timeouts_are_rejected(self):
        for value in (json.loads("1e999"), float("-inf"), float("nan"), True, 10**400):
            data = {"name": "timeout", "steps": [{"action": "autoplay", "turns": 1,
                                                    "timeout": value}]}
            with self.subTest(value=value), self.assertRaisesRegex(ScenarioError, "finite positive"):
                validate_scenario(data)
        validate_scenario({"name": "timeout", "steps": [{"action": "autoplay", "turns": 1,
                                                          "timeout": 1.5}]})

    def test_scenario_validates_lua_code_before_running(self):
        for step in (
            {"action": "ui_lua", "state_name": "InGame", "code": "return true"},
            {"action": "wait_for", "condition": {"type": "lua", "state_name": "InGame",
                                                 "code": "return true"}},
        ):
            with self.subTest(action=step["action"]):
                code_holder = step if step["action"] == "ui_lua" else step["condition"]
                for code, message in (("return\x00true", "NUL"),
                                      ("界" * (MAX_LUA_CODE_BYTES // 3 + 1), "too large")):
                    with self.subTest(message=message):
                        code_holder["code"] = code
                        with self.assertRaisesRegex(ScenarioError, "Step 1: .*" + message):
                            validate_scenario({"name": "Lua validation", "steps": [step]})
        validate_scenario({"name": "Lua validation", "steps": [
            {"action": "ui_lua", "state_name": "InGame", "code": "a" * MAX_LUA_CODE_BYTES}
        ]})

    def test_manual_wait_has_no_step_deadline_or_timeout_option(self):
        step = {"action": "manual", "prompt": "Inspect the game"}
        validate_scenario({"name": "manual", "steps": [step]})
        with self.assertRaisesRegex(ScenarioError, "manual waits for Enter"):
            validate_scenario({"name": "manual", "steps": [{**step, "timeout": 1}]})
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        with redirect_stdout(StringIO()), patch("scenario.input", return_value=""), patch(
            "scenario.time.monotonic", side_effect=AssertionError("manual has no deadline")
        ):
            self.assertEqual(runner._run_step(step), step["prompt"])

    def test_wait_caps_state_and_lua_queries_to_remaining_time(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        client = Mock()
        client.list_states.return_value = ["InGame"]
        client.run_lua.return_value = LuaResult("true", "")
        with patch.object(runner, "_connect", return_value=client):
            runner._wait({"type": "state", "name": "InGame"}, 0.1)
            runner._wait({"type": "lua", "state_name": "InGame", "code": "return true"}, 0.1)
        self.assertEqual(client.list_states.call_count, 2)
        for call in client.list_states.call_args_list:
            self.assertGreater(call.kwargs["timeout"], 0)
            self.assertLessEqual(call.kwargs["timeout"], 0.100001)
        self.assertGreater(client.run_lua.call_args.args[2], 0)
        self.assertLessEqual(client.run_lua.call_args.args[2], 0.100001)

    def test_lua_wait_unknown_reply_fails_without_reexecution(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        condition = {"type": "lua", "state_name": "InGame", "code": "return true"}
        with patch.object(runner, "_lua", side_effect=LuaResponseUnknown("command may have run")) as run_lua, patch(
            "scenario.time.sleep"
        ) as sleep:
            with self.assertRaisesRegex(LuaResponseUnknown, "may have run"):
                runner._wait(condition, 30)
        run_lua.assert_called_once()
        sleep.assert_not_called()

    def test_lua_wait_confirmed_error_fails_without_reexecution(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        condition = {"type": "lua", "state_name": "InGame", "code": "error('boom')"}
        with patch.object(runner, "_lua", side_effect=LuaExecutionError("Lua error in InGame: boom")) as run_lua, patch(
            "scenario.time.sleep"
        ) as sleep:
            with self.assertRaisesRegex(LuaExecutionError, "boom"):
                runner._wait(condition, 30)
        run_lua.assert_called_once()
        sleep.assert_not_called()

    def test_pre_send_framing_error_is_logged_as_step_failure(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            logs = base / "logs"
            logs.mkdir()
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (logs / name).write_text("", encoding="utf-8")
            steps = (
                {"action": "ui_lua", "state_name": "InGame", "code": "return true"},
                {"action": "wait_for", "condition": {"type": "lua", "state_name": "InGame",
                                                     "code": "return true"}},
            )
            for step in steps:
                with self.subTest(action=step["action"]):
                    runner = Runner({"name": "framing", "steps": [step]}, base / step["action"],
                                    logs, base, 4318)
                    original_wait = runner._wait

                    def wait_after_start(condition, timeout):
                        if condition["type"] == "lua" and condition["state_name"] == "FrontEnd":
                            return
                        return original_wait(condition, timeout)

                    with redirect_stdout(StringIO()), patch.object(runner, "_wait", side_effect=wait_after_start), patch.object(
                        runner, "_lua", side_effect=ValueError("FireTuner command is too large")
                    ) as run_lua:
                        self.assertEqual(runner.run(), 1)
                    run_lua.assert_called_once()
                    records = [json.loads(line) for line in (runner.output / "steps.jsonl").read_text(
                        encoding="utf-8").splitlines()]
                    self.assertEqual((records[-1]["action"], records[-1]["status"], records[-1]["detail"]),
                                     ("1/1 " + step["action"], "FAIL", "FireTuner command is too large"))

    def test_transition_pre_send_failure_is_logged_without_waiting(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            logs = base / "logs"
            logs.mkdir()
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (logs / name).write_text("", encoding="utf-8")
            (base / "existing.Civ6Save").write_bytes(b"save")
            for action in ("start_game", "load"):
                with self.subTest(action=action):
                    step = {"action": action}
                    if action == "load":
                        step["name"] = "existing"
                    runner = Runner({"name": action, "steps": [step]}, base / action, logs, base, 4318)
                    with redirect_stdout(StringIO()), patch.object(runner, "_wait"), patch.object(
                        runner, "_loadscreen_count", return_value=0
                    ), patch.object(runner, "_lua", side_effect=TunerError("pre-send connection failed")) as run_lua, patch.object(
                        runner, "_enter_game_after_loadscreen"
                    ) as enter_game:
                        self.assertEqual(runner.run(), 1)
                    run_lua.assert_called_once()
                    enter_game.assert_not_called()
                    records = [json.loads(line) for line in (runner.output / "steps.jsonl").read_text(
                        encoding="utf-8").splitlines()]
                    self.assertEqual((records[-1]["status"], records[-1]["detail"]),
                                     ("FAIL", "pre-send connection failed"))

    def test_transition_waits_after_unknown_reply_without_resending(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            (base / "existing.Civ6Save").write_bytes(b"save")
            for action in ("start_game", "load"):
                with self.subTest(action=action):
                    runner = Runner({}, base, base, base, 4318)
                    step = {"action": action}
                    if action == "load":
                        step["name"] = "existing"
                    with patch.object(runner, "_loadscreen_count", return_value=0), patch.object(
                        runner, "_lua", side_effect=LuaResponseUnknown("command may have run")
                    ) as run_lua, patch.object(runner, "_enter_game_after_loadscreen") as enter_game:
                        runner._run_step(step)
                    run_lua.assert_called_once()
                    enter_game.assert_called_once()

    def test_exit_to_menu_waits_for_state_before_lua_predicate(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        conditions = []

        def record_wait(condition, _timeout):
            conditions.append(condition["type"])

        with patch.object(runner, "_lua", return_value="true") as run_lua, patch.object(
            runner, "_disconnect"
        ) as disconnect, patch.object(runner, "_wait", side_effect=record_wait):
            self.assertEqual(runner._run_step({"action": "exit_to_menu"}), "main menu reached")
        run_lua.assert_called_once()
        disconnect.assert_called_once()
        self.assertEqual(conditions, ["state", "lua"])

    def test_log_wait_requires_new_line_and_does_not_reuse_a_match(self):
        with tempfile.TemporaryDirectory() as root:
            logs = Path(root)
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (logs / name).write_text("", encoding="utf-8")
            lua = logs / "Lua.log"
            lua.write_text("[JAC_POC] notebooks opened\n", encoding="utf-8")
            runner = Runner({}, logs / "output", logs, logs, 4318)
            runner.log_step_baseline = snapshot_logs(logs)
            condition = {"type": "log_line", "log": "Lua.log", "contains": "notebooks opened"}
            with self.assertRaisesRegex(ScenarioError, "Timed out"):
                runner._wait(condition, 0.02)
            with lua.open("a", encoding="utf-8") as handle:
                handle.write("[JAC_POC] notebooks opened\n")
            runner._wait(condition, 0.1)
            with self.assertRaisesRegex(ScenarioError, "Timed out"):
                runner._wait(condition, 0.02)

    def test_later_step_cannot_reuse_previous_steps_log_line(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            logs = base / "logs"
            logs.mkdir()
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (logs / name).write_text("", encoding="utf-8")
            condition = {"type": "log_line", "log": "Lua.log", "contains": "ready once"}
            trigger = {"action": "ui_lua", "state_name": "InGame", "code": "return true"}
            scenario = {"name": "log age", "steps": [
                trigger, {"action": "wait_for", "condition": condition, "timeout": 0.02},
                trigger, {"action": "wait_for", "condition": condition, "timeout": 0.02},
            ]}
            runner = Runner(scenario, base / "output", logs, base, 4318)
            original_wait = runner._wait
            original_run_step = runner._run_step
            emitted = False

            def fake_wait(current, timeout):
                if current["type"] == "lua":
                    return None  # Initial FrontEnd readiness check.
                return original_wait(current, timeout)

            def fake_step(step):
                nonlocal emitted
                if step["action"] == "ui_lua":
                    if not emitted:
                        with (logs / "Lua.log").open("a", encoding="utf-8") as handle:
                            handle.write("ready once\n")
                        emitted = True
                    return "triggered"
                return original_run_step(step)

            with redirect_stdout(StringIO()), patch.object(runner, "_wait", side_effect=fake_wait), patch.object(
                runner, "_run_step", side_effect=fake_step
            ):
                self.assertEqual(runner.run(), 1)
            records = [json.loads(line) for line in (runner.output / "steps.jsonl").read_text(
                encoding="utf-8").splitlines()]
            statuses = [(record["action"], record["status"]) for record in records]
            self.assertIn(("2/4 wait_for", "OK"), statuses)
            self.assertEqual(statuses[-1], ("4/4 wait_for", "FAIL"))

    def test_oserror_after_autoplay_start_attempts_cleanup_without_replay(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        commands = []

        def fake_lua(_state, code, _timeout=5.0):
            commands.append(code)
            if "AutoplayManager.SetActive(true)" in code:
                raise OSError("connection reset")
            return "true"

        with patch.object(runner, "_lua", side_effect=fake_lua), patch.object(
            runner, "_close_options_menu", return_value=False
        ), patch.object(runner, "_dismiss_advisor", return_value=False):
            with self.assertRaisesRegex(OSError, "connection reset"):
                runner._autoplay(3, 10)
        self.assertEqual(sum("AutoplayManager.SetActive(true)" in code for code in commands), 1)
        self.assertEqual(sum("AutoplayManager.SetActive(false)" in code for code in commands), 1)

    def test_step_deadline_prevents_late_autoplay_and_end_turn_side_effects(self):
        runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        commands = []

        def slow_first_lua(_state, code, _timeout=5.0):
            commands.append(code)
            if "JAC_Notebooks_Close" in code:
                import time
                time.sleep(0.025)
            return "true"

        with patch.object(runner, "_lua", side_effect=slow_first_lua), patch.object(
            runner, "_close_options_menu", return_value=False
        ), patch.object(runner, "_dismiss_advisor", return_value=False):
            with self.assertRaisesRegex(ScenarioError, "deadline"):
                runner._autoplay(3, 0.01)
            with self.assertRaisesRegex(ScenarioError, "deadline"):
                runner._end_turn(0.01)
        self.assertFalse(any("AutoplayManager.SetActive(true)" in code for code in commands))
        self.assertFalse(any("UI.RequestAction" in code for code in commands))


class InputSafetyTests(unittest.TestCase):
    def test_deadline_expiring_during_focus_sends_no_key(self):
        for delivery in ("window", "sendinput"):
            with self.subTest(delivery=delivery):
                user32 = Mock()
                user32.SetForegroundWindow.return_value = 1
                user32.GetForegroundWindow.return_value = 100
                clock = [0.0]

                def sleep(seconds):
                    clock[0] += seconds

                with patch.object(civ_click, "KEY_DELIVERY", delivery), patch.object(
                    civ_click, "find_civ_window", return_value=100
                ), patch.object(civ_click, "user32", user32), patch.object(
                    civ_click.time, "monotonic", side_effect=lambda: clock[0]
                ), patch.object(civ_click.time, "sleep", side_effect=sleep):
                    with self.assertRaisesRegex(civ_click.InputError, "deadline"):
                        civ_click.press_key(civ_click.VK_ESCAPE, deadline=0.3)
                user32.SetForegroundWindow.assert_called_once_with(100)
                user32.PostMessageW.assert_not_called()
                user32.SendInput.assert_not_called()

    def test_deadline_expiring_after_key_mapping_sends_no_window_key(self):
        user32 = Mock()
        user32.SetForegroundWindow.return_value = 1
        user32.GetForegroundWindow.return_value = 100
        clock = [0.0]

        def map_key(_vk, _map_type):
            clock[0] = 1.0
            return 1

        user32.MapVirtualKeyW.side_effect = map_key
        with patch.object(civ_click, "find_civ_window", return_value=100), patch.object(
            civ_click, "user32", user32
        ), patch.object(civ_click.time, "sleep"), patch.object(
            civ_click.time, "monotonic", side_effect=lambda: clock[0]
        ):
            with self.assertRaisesRegex(civ_click.InputError, "deadline"):
                civ_click.press_key(civ_click.VK_ESCAPE, deadline=1.0)
        user32.PostMessageW.assert_not_called()

    def test_deadline_expiring_after_foreground_check_sends_no_sendinput_key(self):
        user32 = Mock()
        user32.SetForegroundWindow.return_value = 1
        clock = [0.0]
        foreground_checks = [0]

        def foreground():
            foreground_checks[0] += 1
            if foreground_checks[0] == 2:
                clock[0] = 1.0
            return 100

        user32.GetForegroundWindow.side_effect = foreground
        with patch.object(civ_click, "KEY_DELIVERY", "sendinput"), patch.object(
            civ_click, "find_civ_window", return_value=100
        ), patch.object(civ_click, "user32", user32), patch.object(
            civ_click.time, "sleep"
        ), patch.object(civ_click.time, "monotonic", side_effect=lambda: clock[0]):
            with self.assertRaisesRegex(civ_click.InputError, "deadline"):
                civ_click.press_key(civ_click.VK_ESCAPE, deadline=1.0)
        self.assertEqual(foreground_checks[0], 2)
        user32.SendInput.assert_not_called()

    def test_key_before_deadline_is_sent(self):
        for delivery in ("window", "sendinput"):
            with self.subTest(delivery=delivery):
                user32 = Mock()
                user32.SetForegroundWindow.return_value = 1
                user32.GetForegroundWindow.return_value = 100
                user32.MapVirtualKeyW.return_value = 1
                user32.PostMessageW.return_value = 1
                user32.SendInput.return_value = 1
                with patch.object(civ_click, "KEY_DELIVERY", delivery), patch.object(
                    civ_click, "find_civ_window", return_value=100
                ), patch.object(civ_click, "user32", user32), patch.object(
                    civ_click.time, "sleep"
                ), patch.object(civ_click.time, "monotonic", return_value=0.0):
                    civ_click.press_key(civ_click.VK_ESCAPE, deadline=1.0)
                if delivery == "window":
                    self.assertEqual(user32.PostMessageW.call_count, 2)
                    user32.SendInput.assert_not_called()
                else:
                    self.assertEqual(user32.SendInput.call_count, 2)
                    user32.PostMessageW.assert_not_called()

    def test_foreground_mismatch_sends_no_keys_or_clicks(self):
        user32 = Mock()
        user32.SetForegroundWindow.return_value = 1
        user32.GetForegroundWindow.return_value = 999
        with patch.object(civ_click, "find_civ_window", return_value=100), patch.object(
            civ_click, "user32", user32
        ), patch.object(civ_click.time, "sleep"):
            with self.assertRaisesRegex(RuntimeError, "foreground"):
                civ_click.press_key(civ_click.VK_ESCAPE)
            with self.assertRaisesRegex(RuntimeError, "foreground"):
                civ_click.click_fraction(0.5, 0.5)
        user32.PostMessageW.assert_not_called()
        user32.SendInput.assert_not_called()
        user32.mouse_event.assert_not_called()
        user32.SetCursorPos.assert_not_called()

    def test_focus_loss_after_keydown_still_releases_key(self):
        user32 = Mock()
        user32.SetForegroundWindow.return_value = 1
        user32.GetForegroundWindow.side_effect = (100, 100, 999)
        user32.MapVirtualKeyW.return_value = 1
        user32.PostMessageW.return_value = 1
        with patch.object(civ_click, "find_civ_window", return_value=100), patch.object(
            civ_click, "user32", user32
        ), patch.object(civ_click.time, "sleep"):
            with self.assertRaisesRegex(civ_click.InputError, "foreground"):
                civ_click.press_key(civ_click.VK_ESCAPE)
        self.assertEqual(user32.PostMessageW.call_args_list, [
            unittest.mock.call(100, civ_click.WM_KEYDOWN, civ_click.VK_ESCAPE, 1 | (1 << 16)),
            unittest.mock.call(100, civ_click.WM_KEYUP, civ_click.VK_ESCAPE,
                               1 | (1 << 16) | 0xC0000000),
        ])

    def test_sendinput_fallback_checks_foreground_and_releases_key(self):
        user32 = Mock()
        user32.SetForegroundWindow.return_value = 1
        user32.GetForegroundWindow.side_effect = (100, 100, 100, 999)
        user32.SendInput.return_value = 1
        with patch.object(civ_click, "KEY_DELIVERY", "sendinput"), patch.object(
            civ_click, "find_civ_window", return_value=100
        ), patch.object(civ_click, "user32", user32), patch.object(civ_click.time, "sleep"):
            with self.assertRaisesRegex(civ_click.InputError, "foreground"):
                civ_click.press_key(civ_click.VK_ESCAPE)
        self.assertEqual(user32.SendInput.call_count, 2)
        user32.PostMessageW.assert_not_called()

    def test_input_error_is_logged_as_step_failure(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            logs = base / "logs"
            logs.mkdir()
            for name in ("Lua.log", "Database.log", "Modding.log"):
                (logs / name).write_text("", encoding="utf-8")
            runner = Runner({"name": "input failure", "steps": [{"action": "start_game"}]},
                            base / "output", logs, base, 4318)
            with redirect_stdout(StringIO()), patch.object(runner, "_wait"), patch.object(
                runner, "_run_step", side_effect=civ_click.InputError("foreground lost")
            ):
                self.assertEqual(runner.run(), 1)
            records = [json.loads(line) for line in (runner.output / "steps.jsonl").read_text(
                encoding="utf-8").splitlines()]
            self.assertEqual((records[-1]["action"], records[-1]["status"], records[-1]["detail"]),
                             ("1/1 start_game", "FAIL", "foreground lost"))

    def test_failed_foreground_activation_sends_no_input(self):
        user32 = Mock()
        user32.SetForegroundWindow.return_value = 0
        with patch.object(civ_click, "find_civ_window", return_value=100), patch.object(
            civ_click, "user32", user32
        ), patch.object(civ_click.time, "sleep"):
            with self.assertRaisesRegex(RuntimeError, "Could not bring"):
                civ_click.press_key(civ_click.VK_ESCAPE)
        user32.PostMessageW.assert_not_called()

    def test_window_must_have_unique_civ_executable(self):
        user32 = Mock()
        user32.IsWindowVisible.return_value = True
        user32.GetWindowTextLengthW.return_value = len(civ_click.TITLE_PREFIX)

        def title(_hwnd, buffer, _length):
            buffer.value = civ_click.TITLE_PREFIX
            return len(buffer.value)

        def enumerate_windows(callback, _param):
            return all(callback(hwnd, 0) for hwnd in (100, 200))

        user32.GetWindowTextW.side_effect = title
        user32.EnumWindows.side_effect = enumerate_windows
        with patch.object(civ_click, "user32", user32), patch.object(
            civ_click, "_process_name", side_effect=lambda hwnd: {
                100: "civilizationvi_dx12.exe", 200: "civilizationvihelper.exe"}[hwnd]
        ):
            self.assertEqual(civ_click.find_civ_window(), 100)
        with patch.object(civ_click, "user32", user32), patch.object(
            civ_click, "_process_name", return_value="civilizationvihelper.exe"
        ):
            with self.assertRaisesRegex(civ_click.InputError, "found 0"):
                civ_click.find_civ_window()
            with self.assertRaisesRegex(civ_click.InputError, "found 0"):
                civ_click.press_key(civ_click.VK_ESCAPE)
            user32.PostMessageW.assert_not_called()
        with patch.object(civ_click, "user32", user32), patch.object(
            civ_click, "_process_name", return_value="civilizationvi.exe"
        ):
            with self.assertRaisesRegex(RuntimeError, "found 2"):
                civ_click.find_civ_window()


if __name__ == "__main__":
    unittest.main()
