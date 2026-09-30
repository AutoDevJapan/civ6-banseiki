import json
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from metrics import RESEARCH_SAMPLE_CODE, ResearchMetrics, parse_research_sample, research_sample_code
from research_report import summarize
from scenario import AutoplayStatus, Runner, ScenarioError, _build_setup_code, validate_scenario


SCENARIOS = Path(__file__).resolve().parents[1] / "scenarios"


class DriverFeatureTests(unittest.TestCase):
    def test_end_turn_honours_count_default_and_shared_deadline(self):
        runner = Runner({}, Path("unused-output"), Path("unused-game"), Path("unused-user"), 4318)
        with patch.object(runner, "_end_turn", return_value="advanced") as advance:
            self.assertEqual(runner._run_step({"action": "end_turn", "turns": 2}), "advanced; advanced")
            self.assertEqual(advance.call_count, 2)
            self.assertEqual(advance.call_args_list[0].kwargs["deadline"], advance.call_args_list[1].kwargs["deadline"])
            advance.reset_mock()
            self.assertEqual(runner._run_step({"action": "end_turn"}), "advanced")
            advance.assert_called_once()
        with patch.object(runner, "_end_turn", side_effect=ScenarioError("deadline")) as advance:
            with self.assertRaisesRegex(ScenarioError, "deadline"):
                runner._run_step({"action": "end_turn", "turns": 2})
            advance.assert_called_once()

    def test_end_turn_rejects_invalid_counts(self):
        for turns in (0, -1, True, 1.5, "2"):
            with self.subTest(turns=turns), self.assertRaisesRegex(ScenarioError, "turns"):
                validate_scenario({"name": "turn count", "steps": [{"action": "end_turn", "turns": turns}]})

    def test_build_setup_checks_every_player_after_refresh(self):
        players = [{"leader": "LEADER_JAC_ODA_NOBUHIDE", "civ": "CIVILIZATION_JAC_ODA", "human": True},
                   {"leader": "LEADER_JAC_TAKEDA_NOBUTORA", "civ": "CIVILIZATION_JAC_TAKEDA", "human": False}]
        code = _build_setup_code(players)
        self.assertLess(code.index("BuildHeadlessGameSetup()"), code.index("g_GameParameters:FullRefresh()"))
        self.assertLess(code.index("setup changed player 0"), code.index("setup changed player 1"))
        self.assertLess(code.index("setup changed player 1"), code.index("HideGameSetup()"))
        self.assertIn("GetSlotStatus() ~= SlotStatus.SS_TAKEN", code)
        self.assertIn("GetSlotStatus() ~= SlotStatus.SS_COMPUTER", code)
        with tempfile.TemporaryDirectory() as folder:
            runner = Runner({}, Path(folder), Path(folder), Path(folder), 4318)
            step = {"action": "configure", "ruleset": "RULESET_JAC_BANSEIKI", "map_size": "MAPSIZE_SMALL",
                    "speed": "GAMESPEED_STANDARD", "seed": 1, "players": players, "build_setup": True}
            with patch.object(runner, "_lua", return_value="true") as lua:
                runner._run_step(step)
            self.assertEqual([call.args[0] for call in lua.call_args_list], ["FrontEnd", "HostGame"])
            self.assertEqual(lua.call_args_list[1].args[1], code)

    def test_build_setup_defaults_on_for_banseiki_only(self):
        players = [{"leader": "L", "civ": "C", "human": True}]
        base = {"action": "configure", "map_size": "M", "speed": "S", "seed": 1, "players": players}
        cases = [("RULESET_JAC_BANSEIKI", {}, ["FrontEnd", "HostGame"]),
                 ("RULESET_JAC_BANSEIKI", {"build_setup": False}, ["FrontEnd"]),
                 ("RULESET_STANDARD", {}, ["FrontEnd"]),
                 ("RULESET_STANDARD", {"build_setup": True}, ["FrontEnd", "HostGame"])]
        for ruleset, extra, states in cases:
            with self.subTest(ruleset=ruleset, extra=extra), tempfile.TemporaryDirectory() as folder:
                runner = Runner({}, Path(folder), Path(folder), Path(folder), 4318)
                with patch.object(runner, "_lua", return_value="true") as lua:
                    runner._run_step({**base, "ruleset": ruleset, **extra})
                self.assertEqual([call.args[0] for call in lua.call_args_list], states)

    def test_start_turn_guard(self):
        with tempfile.TemporaryDirectory() as folder:
            runner = Runner({}, Path(folder), Path(folder), Path(folder), 4318)
            with patch.object(runner, "_lua", return_value="16603") as lua:
                runner._check_start_turn(time.monotonic() + 60)  # other rulesets are not checked
                lua.assert_not_called()
                runner.ruleset = "RULESET_JAC_BANSEIKI"
                with self.assertRaisesRegex(ScenarioError, "start turn is 16603, expected 1"):
                    runner._check_start_turn(time.monotonic() + 60)
            with patch.object(runner, "_lua", return_value="1"):
                runner._check_start_turn(time.monotonic() + 60)

    def test_output_scoped_save_name_and_exact_name(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            a = Runner({}, base / "first", base, base, 4318)
            b = Runner({}, base / "second", base, base, 4318)
            step = {"name": "JAC-Test"}
            self.assertNotEqual(a._save_name(step), b._save_name(step))
            self.assertEqual(a._save_name(step), a._save_name({"action": "load", "name": "JAC-Test"}))
            self.assertLessEqual(len(a._save_name({"name": "A" * 80})), 80)
            self.assertEqual(a._save_name({"name": "JAC-Test", "exact_name": True}), "JAC-Test")
            a.output.mkdir()
            target = a._save_path(a._save_name(step))
            target.write_bytes(b"existing")
            with patch.object(a, "_lua") as lua:
                with self.assertRaisesRegex(ScenarioError, "will not overwrite"):
                    a._run_step({"action": "save", **step})
            lua.assert_not_called()

    def test_save_dismisses_blocking_advisor_before_saving(self):
        # Live sengoku-events-list-06: an advisor left open after autoplay kept SaveComplete from firing.
        with tempfile.TemporaryDirectory() as folder:
            runner = Runner({}, Path(folder), Path(folder), Path(folder), 4318)
            order = []
            name = runner._save_name({"name": "JAC-Test"})

            def lua(state, code, timeout):
                order.append("save" if "Network.SaveGame" in code else state)
                if "Network.SaveGame" in code:
                    runner._save_path(name).write_bytes(b"save")
                return "true"

            with patch.object(runner, "_lua", side_effect=lua), patch.object(runner, "_wait"), \
                    patch.object(runner, "_close_options_menu", side_effect=lambda d: order.append("menu")), \
                    patch.object(runner, "_dismiss_advisor", side_effect=lambda d: order.append("advisor")):
                runner._run_step({"action": "save", "name": "JAC-Test"})
            self.assertEqual(order[:3], ["menu", "advisor", "save"])

    def test_flag_validation(self):
        config = json.loads((SCENARIOS / "sengoku-trees.json").read_text(encoding="utf-8"))
        config["steps"][1]["build_setup"] = "yes"
        with self.assertRaisesRegex(ScenarioError, "build_setup"):
            validate_scenario(config)
        for key in ("research_metrics", "allow_human_elimination"):
            with self.subTest(key=key):
                data = {"name": "flag", "steps": [{"action": "autoplay", "turns": 1, key: "yes"}]}
                with self.assertRaisesRegex(ScenarioError, key):
                    validate_scenario(data)
        with self.assertRaisesRegex(ScenarioError, "exact_name"):
            validate_scenario({"name": "flag", "steps": [{"action": "save", "name": "x", "exact_name": 1}]})

    def test_allow_elimination_observes_target_and_records_turn(self):
        with tempfile.TemporaryDirectory() as folder:
            runner = Runner({}, Path(folder), Path(folder), Path(folder), 4318)
            statuses = iter((AutoplayStatus(2, True, False, 1, -1),
                             AutoplayStatus(3, False, True, 0, -1)))

            def lua(_state, code, _timeout=5.0):
                if "return tostring(startTurn)" in code:
                    return "1|0"
                return "false" if "IsAlive()" in code else "true"

            with patch.object(runner, "_lua", side_effect=lua), patch.object(
                runner, "_autoplay_status", side_effect=lambda _deadline: next(statuses)
            ), patch.object(runner, "_close_options_menu"), patch.object(runner, "_close_diplomacy", return_value=[]), patch.object(
                runner, "_dismiss_advisor"
            ), patch("scenario.time.sleep"):
                detail = runner._autoplay(2, 10, allow_human_elimination=True)
            self.assertIn("1->3", detail)
            self.assertIn("human_elimination_first_observed_turn=2", detail)
            events = [json.loads(line) for line in (Path(folder) / "steps.jsonl").read_text(
                encoding="utf-8").splitlines()]
            self.assertEqual(len(events), 1)
            self.assertEqual(events[0]["action"], "autoplay_elimination")

    def test_research_samples_and_report_use_first_observed_completion(self):
        self.assertIn("research:GetResearchingTech()", RESEARCH_SAMPLE_CODE)
        self.assertIn("culture:GetProgressingCivic()", RESEARCH_SAMPLE_CODE)
        self.assertIn("local configured = {0,1}", research_sample_code({1, 0}))
        first = "1|0~LEADER_A~true~0~0~~~~;1~LEADER_B~true~0~0~~~~"
        second = "3|0~LEADER_A~true~1~1~TECH_B~CIVIC_B~TECH_A~CIVIC_A;1~LEADER_B~false~0~0~~~~"
        third = "5|0~LEADER_A~true~1~1~TECH_B~CIVIC_B~TECH_A~CIVIC_A;1~LEADER_B~false~1~0~~~TECH_A~"
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "research.jsonl"
            metrics = ResearchMetrics(path)
            self.assertEqual(metrics.sample(first), 1)
            self.assertEqual(metrics.sample(second), 3)
            self.assertEqual(metrics.sample(third), 5)
            rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
            self.assertEqual(len(rows), 6)
            self.assertEqual(sorted({row["turn"] for row in rows}), [1, 3, 5])
            self.assertEqual(rows[2]["current_tech"], "TECH_B")
            self.assertEqual(rows[3]["alive"], False)
            self.assertEqual(summarize(path), [{"id": "TECH_A", "factions": 2,
                                                "median_turn": 4, "min_turn": 3, "max_turn": 5,
                                                "spread_turns": 2, "baseline_factions": 0}])
            self.assertEqual(summarize(path, "civic")[0]["id"], "CIVIC_A")
            with self.assertRaisesRegex(ValueError, "Major slots changed"):
                metrics.sample("6|0~LEADER_A~true~1~1~~~TECH_A~CIVIC_A")
        with self.assertRaisesRegex(ValueError, "Invalid research sample"):
            parse_research_sample("2|0~LEADER_A~true~2~0~~~TECH_A~")

    def test_autoplay_emits_research_jsonl_for_each_observed_turn(self):
        with tempfile.TemporaryDirectory() as folder:
            runner = Runner({}, Path(folder), Path(folder), Path(folder), 4318)
            statuses = iter((AutoplayStatus(2, True, False, 1, 0),
                             AutoplayStatus(3, False, True, 0, 0)))
            samples = iter(("1|0~LEADER_A~true~0~0~~~~",
                            "2|0~LEADER_A~true~1~0~~~TECH_A~",
                            "3|0~LEADER_A~true~1~0~~~TECH_A~"))

            def lua(_state, code, _timeout=5.0):
                if "return tostring(startTurn)" in code:
                    return "1|0"
                if "for _, id in ipairs(configured" in code:
                    return next(samples)
                return "true"

            with patch.object(runner, "_lua", side_effect=lua), patch.object(
                runner, "_autoplay_status", side_effect=lambda _deadline: next(statuses)
            ), patch.object(runner, "_close_options_menu"), patch.object(runner, "_close_diplomacy", return_value=[]), patch.object(
                runner, "_dismiss_advisor"
            ), patch("scenario.time.sleep"):
                detail = runner._autoplay(2, 10, research_metrics=True)
            path = Path(folder) / "autoplay-research-1.jsonl"
            rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
            self.assertEqual([row["turn"] for row in rows], [1, 2, 3])
            self.assertEqual(rows[1]["completed_techs"], ["TECH_A"])
            self.assertIn(path.name, detail)

    def test_new_scenarios_check_requested_states(self):
        screen = json.loads((SCENARIOS / "japan-map-setup-screen.json").read_text(encoding="utf-8"))
        self.assertIn("OnSetMapByValue(file)", screen["steps"][4]["code"])
        self.assertIn("#values~=22", screen["steps"][4]["code"])
        self.assertIn("#values~=2", screen["steps"][2]["code"])
        settler = json.loads((SCENARIOS / "japan-map-15km-settlers.json").read_text(encoding="utf-8"))
        self.assertIn("_Settlers.Civ6Map", settler["steps"][1]["map_file"])
        self.assertIn("p:GetCities():GetCount()~=0", settler["steps"][3]["code"])
        self.assertIn("count~=1", settler["steps"][3]["code"])
        long = json.loads((SCENARIOS / "japan-map-15km-200turn.json").read_text(encoding="utf-8"))
        steps = long["steps"]
        auto = next(i for i, step in enumerate(steps) if step["action"] == "autoplay")
        self.assertEqual(steps[auto]["turns"], 200)
        self.assertTrue(steps[auto]["allow_human_elimination"])
        self.assertTrue(steps[auto]["research_metrics"])
        # Research metrics must never be recorded on the official trees.
        self.assertIn("26c967d0-c10d-4eeb-8aaf-c83dc7e621e7", steps[0]["ids"])
        self.assertTrue(steps[1]["build_setup"])
        self.assertEqual(steps[1]["ruleset"], "RULESET_JAC_BANSEIKI")
        start = next(i for i, step in enumerate(steps) if step["action"] == "start_game")
        check = next(step["code"] for step in steps[start + 1:auto] if "RULESET_JAC_BANSEIKI" in step.get("code", ""))
        for text in ("GetRuleSet()~='RULESET_JAC_BANSEIKI'", "TECH_JAC_SENGOKU_", "CIVIC_JAC_SENGOKU_", "t~=32 or c~=30"):
            self.assertIn(text, check)

    def test_early_stop_resume_clears_done_flag_and_waits_for_new_end_event(self):
        with tempfile.TemporaryDirectory() as folder:
            runner = Runner({}, Path(folder), Path(folder), Path(folder), 4318)
            # Early stop at turn 2 with a stale end event, then the real end at turn 4.
            statuses = iter((AutoplayStatus(2, False, True, 0, 0), AutoplayStatus(2, False, True, 0, 0),
                             AutoplayStatus(3, True, False, 1, 0), AutoplayStatus(4, False, True, 0, 0)))
            calls = []

            def lua(_state, code, _timeout=5.0):
                calls.append(code)
                return "1|0" if "return tostring(startTurn)" in code else "true"

            clock = iter(range(0, 1000, 2))
            with patch.object(runner, "_lua", side_effect=lua), patch.object(
                runner, "_autoplay_status", side_effect=lambda _deadline: next(statuses)
            ), patch.object(runner, "_close_options_menu"), patch.object(runner, "_close_diplomacy", return_value=[]), patch.object(
                runner, "_dismiss_advisor"
            ), patch.object(runner, "_choose_dedication", return_value=None), patch(
                "scenario.time.sleep"
            ), patch("scenario.time.monotonic", side_effect=lambda: next(clock)):
                detail = runner._autoplay(3, 1000)
            resumes = [c for c in calls if "SetTurns(2)" in c]
            self.assertEqual(len(resumes), 1)
            self.assertLess(resumes[0].index("JAC_AUTOTEST_AUTO_DONE = false"), resumes[0].index("SetActive(true)"))
            self.assertIn("1->4", detail)


if __name__ == "__main__":
    unittest.main()
