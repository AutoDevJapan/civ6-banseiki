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


class ScenarioSafetyTests(unittest.TestCase):
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

    def test_nonfinite_timeouts_are_rejected(self):
        for value in (json.loads("1e999"), float("-inf"), float("nan"), True, 10**400):
            data = {"name": "timeout", "steps": [{"action": "autoplay", "turns": 1,
                                                    "timeout": value}]}
            with self.subTest(value=value), self.assertRaisesRegex(ScenarioError, "finite positive"):
                validate_scenario(data)
        validate_scenario({"name": "timeout", "steps": [{"action": "autoplay", "turns": 1,
                                                          "timeout": 1.5}]})

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


class InputSafetyTests(unittest.TestCase):
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
        user32.keybd_event.assert_not_called()
        user32.mouse_event.assert_not_called()
        user32.SetCursorPos.assert_not_called()

    def test_focus_loss_after_keydown_still_releases_key(self):
        user32 = Mock()
        user32.SetForegroundWindow.return_value = 1
        user32.GetForegroundWindow.side_effect = (100, 100, 999)
        with patch.object(civ_click, "find_civ_window", return_value=100), patch.object(
            civ_click, "user32", user32
        ), patch.object(civ_click.time, "sleep"):
            with self.assertRaisesRegex(civ_click.InputError, "foreground"):
                civ_click.press_key(civ_click.VK_ESCAPE)
        self.assertEqual(user32.keybd_event.call_args_list, [
            unittest.mock.call(civ_click.VK_ESCAPE, 0, 0, 0),
            unittest.mock.call(civ_click.VK_ESCAPE, 0, civ_click.KEYEVENTF_KEYUP, 0),
        ])

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
        user32.keybd_event.assert_not_called()

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
            user32.keybd_event.assert_not_called()
        with patch.object(civ_click, "user32", user32), patch.object(
            civ_click, "_process_name", return_value="civilizationvi.exe"
        ):
            with self.assertRaisesRegex(RuntimeError, "found 2"):
                civ_click.find_civ_window()


if __name__ == "__main__":
    unittest.main()
