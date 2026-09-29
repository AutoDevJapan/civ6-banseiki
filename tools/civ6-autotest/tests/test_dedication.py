import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scenario import AutoplayStatus, Runner, ScenarioError  # noqa: E402


def make_runner(statuses, pending):
    """Fake game: `statuses` are returned in order (last repeats); `pending` = dedication results."""
    runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
    clock = [0.0]
    commands = []
    it = iter(statuses)
    last = [None]

    def lua(_state, code, _timeout=5.0):
        commands.append(code)
        if "return tostring(startTurn)" in code:
            return "30|14"
        if "GetPlayerCommemorateChoices" in code:
            return pending.pop(0) if pending else "none"
        return "true"

    def status(_deadline):
        last[0] = next(it, last[0])
        return last[0]

    def sleep(seconds):
        clock[0] += max(seconds, 3)

    patches = [
        patch.object(runner, "_lua", side_effect=lua),
        patch.object(runner, "_autoplay_status", side_effect=status),
        patch.object(runner, "_close_options_menu", return_value=False),
        patch.object(runner, "_close_diplomacy", return_value=[]), patch.object(runner, "_dismiss_advisor", return_value=False),
        patch.object(runner, "_log"),
        patch("scenario.time.monotonic", side_effect=lambda: clock[0]),
        patch("scenario.time.sleep", side_effect=sleep),
    ]
    return runner, commands, patches


def S(turn, active, event=False, remaining=0):
    return AutoplayStatus(turn, active, event, remaining, 14)


class DedicationTest(unittest.TestCase):
    def run_case(self, statuses, pending, escape_ok=True):
        runner, commands, patches = make_runner(statuses, pending)
        for p in patches:
            p.start()
        self.addCleanup(lambda: [p.stop() for p in patches])
        with patch("scenario.press_key") as press:
            try:
                result = runner._autoplay(10, 1000)
            except ScenarioError as exc:
                result = exc
        return result, commands, press

    def test_stall_with_pending_dedication_chooses_closes_and_resumes(self):
        statuses = [S(31, True), S(31, False, remaining=10)] * 1 + [S(31, False, remaining=10)] * 3 + [S(40, False, True)]
        result, commands, press = self.run_case(statuses, ["7,9"])
        self.assertIsInstance(result, str)
        self.assertEqual(press.call_count, 1)
        self.assertEqual(sum("COMMEMORATE" in c and "GetPlayerCommemorateChoices" in c for c in commands), 1)
        self.assertEqual(sum("SetActive(true)" in c for c in commands), 2)  # start + resume

    def test_repeated_dedication_is_bounded_then_fails(self):
        statuses = [S(31, False, remaining=10)]
        result, commands, press = self.run_case(statuses, ["1"] * 50)
        self.assertIsInstance(result, ScenarioError)
        self.assertLessEqual(press.call_count, 3)
        self.assertEqual(sum("GetPlayerCommemorateChoices" in c for c in commands), 3)
        self.assertEqual(sum("SetActive(true)" in c for c in commands), 1 + 3 + 2)  # start, 3 dedications, 2 plain restarts

    def test_no_pending_dedication_keeps_old_restart_behaviour(self):
        result, commands, press = self.run_case([S(31, False, remaining=10)], [])
        self.assertIsInstance(result, ScenarioError)
        press.assert_not_called()


if __name__ == "__main__":
    unittest.main()
