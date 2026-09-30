from pathlib import Path
import re
import sys
import tempfile
import unittest
from unittest.mock import patch


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scenario import Runner
from tuner import LuaResponseUnknown


class FakeGame:
    """Autoplay that an AI leader interrupts with a statement (or deal) to the human at given turns."""

    def __init__(self, interrupt_turns, *, stop_early=False, deal=False, starve=False, reopen=False):
        self.turn, self.active, self.done, self.left = 1, False, False, 0
        self.pending = set(interrupt_turns)
        self.stop_early, self.deal, self.starve, self.reopen = stop_early, deal, starve, reopen
        self.visible = {"DiplomacyActionView": False, "DiplomacyDealView": False}
        self.calls, self.closed = [], []

    def states(self, _timeout=5.0):
        return ["InGame", *self.visible]

    def lua(self, state, code, _timeout=5.0):
        self.calls.append((state, code))
        if state != "InGame":
            assert code.startswith("if ContextPtr:IsHidden() then return false end")
            if not self.visible[state]:
                return "false"
            # DEAL_MODE: the action view ignores OnClose while the deal screen is up.
            if state == "DiplomacyDealView" or not self.visible["DiplomacyDealView"]:
                self.visible[state] = False
                self.closed.append(state)
            return "true"
        if "SetTurns(" in code:
            self.left = int(re.search(r"SetTurns\((\d+)\)", code).group(1))
            self.active, self.done = True, False
            return f"{self.turn}|0" if "return tostring(startTurn)" in code else "true"
        if "tostring(AutoplayManager.IsActive())" in code:
            if self.starve and any(self.visible.values()):
                raise LuaResponseUnknown("FireTuner response timed out")
            self._advance()
            return f"{self.turn}|{str(self.active).lower()}|{str(self.done).lower()}|{self.left}|0"
        if "COMMEMORATE" in code:
            return "none"
        return "true"

    def _advance(self):
        if not self.active or self.visible["DiplomacyActionView"] or self.visible["DiplomacyDealView"]:
            return  # the AI turn waits for the human's answer
        if self.turn in self.pending or self.reopen:
            self.pending.discard(self.turn)
            self.visible["DiplomacyActionView"] = True
            self.visible["DiplomacyDealView"] = self.deal
            if self.stop_early:
                self.active = False
            return
        self.turn += 1
        self.left -= 1
        if self.left == 0:
            self.active, self.done = False, True


class DiplomacyDuringAutoplayTests(unittest.TestCase):
    def run_autoplay(self, game, turns):
        clock = [0.0]

        def sleep(seconds):
            clock[0] += seconds + 0.01

        with tempfile.TemporaryDirectory() as folder:
            runner = Runner({}, Path(folder), Path(folder), Path(folder), 4318)
            with patch.object(runner, "_states", side_effect=game.states), patch.object(
                runner, "_lua", side_effect=game.lua
            ), patch("scenario.time.monotonic", side_effect=lambda: clock[0]), patch(
                "scenario.time.sleep", side_effect=sleep
            ), patch("builtins.print"):
                detail = runner._autoplay(turns, 600)
            steps = (Path(folder) / "steps.jsonl").read_text(encoding="utf-8")
        return detail, steps

    def sent(self, game, state, call):
        return sum(s == state and call in code for s, code in game.calls)

    def assertClosed(self, game, expected):
        self.assertEqual(game.closed, expected)
        # Every close went through the official handler behind the visibility guard.
        self.assertTrue(all(s == "InGame" or code.startswith("if ContextPtr:IsHidden() then return false end")
                            for s, code in game.calls))

    def test_statement_mid_autoplay_is_closed_and_autoplay_completes(self):
        game = FakeGame({3})
        detail, steps = self.run_autoplay(game, 5)
        self.assertIn("autoplay completed 5 turns (1->6", detail)
        self.assertIn("closed AI diplomacy statement", steps)
        self.assertClosed(game, ["DiplomacyActionView"])
        self.assertEqual(self.sent(game, "InGame", "SetActive(true)"), 1)  # no resume needed

    def test_repeated_early_stops_by_statements_resume_without_using_restart_budget(self):
        game = FakeGame({2, 3, 4}, stop_early=True)
        detail, steps = self.run_autoplay(game, 5)
        self.assertIn("(1->6", detail)
        self.assertClosed(game, ["DiplomacyActionView"] * 3)
        self.assertEqual(self.sent(game, "InGame", "SetActive(true)"), 1 + 3)
        self.assertIn("resumed autoplay for 4 turns after closing diplomacy", steps)

    def test_deal_proposal_is_refused_before_the_statement_view_closes(self):
        game = FakeGame({2}, deal=True)
        detail, steps = self.run_autoplay(game, 3)
        self.assertIn("(1->4", detail)
        self.assertClosed(game, ["DiplomacyDealView", "DiplomacyActionView"])
        self.assertGreater(self.sent(game, "DiplomacyDealView", "OnLocalPlayerTurnEnd()"), 0)
        self.assertGreater(self.sent(game, "DiplomacyActionView", "OnClose()"), 0)
        self.assertIn("closed AI deal proposal, AI diplomacy statement", steps)

    def test_status_query_starved_by_statement_closes_it_and_asks_again(self):
        game = FakeGame({2}, starve=True)
        detail, _ = self.run_autoplay(game, 3)
        self.assertIn("(1->4", detail)
        self.assertClosed(game, ["DiplomacyActionView"])

    def test_dark_age_policy_choice_is_closed_without_holding_autoplay(self):
        game = FakeGame(set())
        game.visible["JAC_SengokuDarkPolicy"] = True  # shown at the human's turn start; it holds no AI turn
        detail, steps = self.run_autoplay(game, 2)
        self.assertIn("(1->3", detail)
        self.assertClosed(game, ["JAC_SengokuDarkPolicy"])
        self.assertGreater(self.sent(game, "JAC_SengokuDarkPolicy", "OnClose()"), 0)
        self.assertIn("closed dark-age policy choice", steps)

    def test_era_name_change_screen_is_closed_then_the_dark_policy_choice_it_opens(self):
        class ReignGame(FakeGame):
            def lua(self, state, code, _timeout=5.0):
                result = super().lua(state, code, _timeout)
                if state == "JAC_SengokuReignPopup" and result == "true":
                    self.visible["JAC_SengokuDarkPolicy"] = True  # UI/JAC_SengokuDarkPolicy.lua on ..._Closed
                return result

        game = ReignGame(set())
        game.visible["JAC_SengokuReignPopup"] = True  # shown at the 改元 turn; it holds no AI turn
        game.visible["JAC_SengokuDarkPolicy"] = False
        detail, steps = self.run_autoplay(game, 2)
        self.assertIn("(1->3", detail)
        self.assertClosed(game, ["JAC_SengokuReignPopup", "JAC_SengokuDarkPolicy"])
        self.assertIn("closed era-name change screen, dark-age policy choice", steps)

    def test_screen_that_keeps_reopening_is_bounded(self):
        game = FakeGame(set(), reopen=True)
        with self.assertRaisesRegex(Exception, "keeps reopening"):
            self.run_autoplay(game, 1)
        self.assertEqual(len(game.closed), 26)  # cap 20 + 5 * turns, then the step stops


if __name__ == "__main__":
    unittest.main()
