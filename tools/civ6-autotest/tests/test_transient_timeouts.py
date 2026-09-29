from pathlib import Path
import sys
import time
import unittest
from unittest.mock import patch


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scenario import Runner
from tuner import LuaExecutionError, LuaResponseUnknown, LuaResult, TunerError


class FakeTuner:
    """Times out `failures` times on run_lua, then answers from `replies`."""

    instances: list = []
    failures = 1
    calls: list = []
    replies = {"InGame": "42|true|false|10|0", "AdvisorPopup": "true"}
    error: Exception = LuaResponseUnknown("No Lua acknowledgment; FireTuner response timed out after 0/8 bytes")

    def __init__(self, *_args, **_kwargs):
        self.closed = False
        FakeTuner.instances.append(self)

    def __enter__(self):
        return self

    def close(self):
        self.closed = True

    def list_states(self, timeout=None):
        return ["InGame", "AdvisorPopup"]

    def run_lua(self, state, code, timeout=None):
        FakeTuner.calls.append(state)
        if FakeTuner.failures > 0:
            FakeTuner.failures -= 1
            raise FakeTuner.error
        return LuaResult(FakeTuner.replies[state], "")


class TransientTimeoutTests(unittest.TestCase):
    def setUp(self):
        FakeTuner.instances, FakeTuner.calls, FakeTuner.failures = [], [], 1
        FakeTuner.error = LuaResponseUnknown("timed out after 0/8 bytes")
        self.runner = Runner({}, Path("unused"), Path("unused"), Path("unused"), 4318)
        self.patches = [patch("scenario.TunerClient", FakeTuner), patch("scenario.require_single_civ_process"),
                        patch("scenario.time.sleep")]
        for p in self.patches:
            p.start()
            self.addCleanup(p.stop)

    def test_status_query_retries_after_timeout(self):
        status = self.runner._autoplay_status(time.monotonic() + 60)
        self.assertEqual(status.turn, 42)
        self.assertEqual(FakeTuner.calls, ["InGame", "InGame"])
        self.assertTrue(FakeTuner.instances[0].closed)  # stale socket dropped before retry

    def test_advisor_dismissal_retries_after_timeout(self):
        self.assertTrue(self.runner._dismiss_advisor(time.monotonic() + 60))
        self.assertEqual(FakeTuner.calls, ["AdvisorPopup", "AdvisorPopup"])

    def test_research_query_retries_after_timeout(self):
        FakeTuner.replies = {**FakeTuner.replies, "InGame": "3|0~A~true~0~0~~~~"}
        self.addCleanup(FakeTuner.replies.update, {"InGame": "42|true|false|10|0"})
        self.assertEqual(self.runner._research_query("return 1", time.monotonic() + 60), "3|0~A~true~0~0~~~~")
        self.assertEqual(FakeTuner.calls, ["InGame", "InGame"])

    def test_gives_up_only_after_deadline(self):
        FakeTuner.failures = 10 ** 6
        with self.assertRaises(LuaResponseUnknown):
            self.runner._autoplay_status(time.monotonic() + 3)
        self.assertGreater(len(FakeTuner.calls), 1)

    def test_lua_error_is_not_retried(self):
        FakeTuner.error = LuaExecutionError("boom")
        with self.assertRaises(LuaExecutionError):
            self.runner._autoplay_status(time.monotonic() + 60)
        self.assertEqual(FakeTuner.calls, ["InGame"])

    def test_state_changing_lua_is_not_replayed(self):
        with self.assertRaises(LuaResponseUnknown):
            self.runner._lua("InGame", "AutoplayManager.SetActive(true)")
        self.assertEqual(FakeTuner.calls, ["InGame"])


if __name__ == "__main__":
    unittest.main()
