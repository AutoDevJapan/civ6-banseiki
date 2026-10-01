"""AI arbitration telemetry must distinguish cases, respondents, and missing or truncated records."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

FOLDER = Path(__file__).resolve().parents[1]


def module(name):
    spec = importlib.util.spec_from_file_location(name, FOLDER / (name + ".py"))
    obj = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(obj)
    return obj


report = module("arbitration_report")
builder = module("build_arbitration_ai_scenario")

LOG = """[JAC_ARB] loaded shogunate_end_turn=120
[JAC_ARB] request player=0 pair=0-1 turn=2 power=1
[JAC_ARB] cost player=0 pair=0-1 turn=2 gold=100 legitimacy=5 gold_after=400.00 legitimacy_after=35 by=ai_party
[JAC_ARB] ai request player=0 pair=0-1 turn=2 by=ai_party score=6 duration=5 weariness=0 weariness_cities=0 army_loss=3 city_loss=0 low_legitimacy=2
[JAC_ARB] decision player=1 accept=true by=ai
[JAC_ARB] accepted pair=0-1 turn=3 truce_turns=10 power=1 peace=true
[JAC_ARB] request player=2 pair=1-3 turn=14 power=3
[JAC_ARB] cost player=2 pair=1-3 turn=14 gold=34 legitimacy=2 gold_after=466.00 legitimacy_after=75 by=ai_shogun
[JAC_ARB] ai request player=2 pair=1-3 turn=14 by=ai_shogun score=3 duration=12 ally=false capital=false long_war=true
[JAC_ARB] decision player=1 accept=false by=ai
[JAC_ARB] decision player=3 accept=false by=ai
[JAC_ARB] refused player=1 pair=1-3 turn=15 legitimacy=0 outlaw_until=45 power=3
[JAC_ARB] refusal cost player=1 pair=1-3 turn=15 intended=45 applied=30 legitimacy_before=30 legitimacy_after=0
[JAC_ARB] refused player=3 pair=1-3 turn=15 legitimacy=15 outlaw_until=45 power=3
[JAC_ARB] refusal cost player=3 pair=1-3 turn=15 intended=45 applied=45 legitimacy_before=60 legitimacy_after=15
[JAC_ARB] ai blocked turn=16 gold=1 legitimacy=2 interval=3
"""


class ArbitrationReportTests(unittest.TestCase):
    def summary(self, log=LOG, test_mode=None, result=True):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "Lua.log").write_text(log, encoding="utf-8")
            steps = [dict(status="OK", action=f"{i}/18 autoplay", detail="autoplay completed 50 turns") for i in range(4)]
            steps += [dict(status="OK", action="18/18 ui_lua", detail="return=T+201 surviving majors=22"),
                      dict(status="OK", action="judge", detail="8/8 PASS")]
            (root / "steps.jsonl").write_text("\n".join(json.dumps(s) for s in steps), encoding="utf-8")
            if result is not None:
                (root / "result.json").write_text(json.dumps(dict(passed=result, test_mode=test_mode)), encoding="utf-8")
            return report.summarize(root)

    def test_outcomes_costs_clamping_and_sources(self):
        data = self.summary()
        self.assertTrue(data["measurement_complete"], data["issues"])
        self.assertEqual(data["requests"], 2)
        self.assertEqual(data["sources"], {"ai_party": 1, "ai_shogun": 1})
        self.assertEqual(data["outcomes"], {"accepted": 1, "refused": 1})
        self.assertEqual(data["refused_respondents"], 2)
        self.assertEqual(data["decisions"], {"ai:true": 1, "ai:false": 2})
        self.assertEqual(data["spending"]["ai_shogun"], {"gold": 34, "legitimacy": 2})
        self.assertEqual((data["refusal_loss_intended"], data["refusal_loss_applied"]), (90, 75))
        self.assertEqual(data["blocked_candidate_visits"], {"gold": 1, "legitimacy": 2, "interval": 3})
        self.assertEqual(data["shogun_after_cutoff"], [])

    def test_missing_or_duplicate_cost_prevents_complete_measurement(self):
        for log in (LOG.replace(next(line for line in LOG.splitlines(True) if line.startswith("[JAC_ARB] cost ")), ""),
                    LOG + "[JAC_ARB] cost player=2 pair=1-3 turn=14 gold=34 legitimacy=2 gold_after=466.00 legitimacy_after=75 by=ai_shogun\n"):
            with self.subTest(log=log[-100:]):
                self.assertFalse(self.summary(log)["measurement_complete"])

    def test_missing_or_early_ruling_and_missing_refusal_loss_are_detected(self):
        for log in (LOG.replace("accepted pair=0-1 turn=3", "accepted pair=0-1 turn=2"),
                    LOG.replace(next(line for line in LOG.splitlines(True) if line.startswith("[JAC_ARB] refusal cost ")), "")):
            self.assertFalse(self.summary(log)["measurement_complete"])

    def test_empowered_third_party_request_at_120_is_detected(self):
        changed = LOG.replace("turn=14", "turn=120").replace("turn=15", "turn=121")
        data = self.summary(changed)
        self.assertFalse(data["measurement_complete"])
        self.assertEqual(data["shogun_after_cutoff"], [120])

    def test_pending_final_turn_is_counted_without_claiming_a_ruling(self):
        tail = """[JAC_ARB] request player=0 pair=0-3 turn=201 power=1
[JAC_ARB] cost player=0 pair=0-3 turn=201 gold=100 legitimacy=5 gold_after=300.00 legitimacy_after=60 by=ai_party
[JAC_ARB] ai request player=0 pair=0-3 turn=201 by=ai_party score=6 duration=24
"""
        data = self.summary(LOG + tail)
        self.assertTrue(data["measurement_complete"], data["issues"])
        self.assertEqual(data["unfinished"], 1)
        self.assertEqual(data["player_intervals"]["0"], [199])

    def test_requester_and_pair_interval_violations_are_detected(self):
        # Change the second requester to 0, and make it only nine turns after the first.
        changed = LOG.replace("turn=14", "turn=11").replace("turn=15", "turn=12").replace("player=2", "player=0")
        data = self.summary(changed)
        self.assertFalse(data["measurement_complete"])
        self.assertIn("AI request interval shorter than ten turns", data["issues"])

    def test_test_clock_jump_never_counts_as_uninterrupted_measurement(self):
        self.assertFalse(self.summary(test_mode=dict(skipped_turns=113))["complete_200_turns"])

    def test_reloaded_log_is_not_an_uninterrupted_measurement(self):
        data = self.summary(LOG + "[JAC_ARB] loaded shogunate_end_turn=120\n")
        self.assertFalse(data["measurement_complete"])

    def test_missing_or_failed_result_is_not_complete(self):
        for result in (None, False):
            with self.subTest(result=result):
                self.assertFalse(self.summary(result=result)["measurement_complete"])

    def test_refusal_cost_must_match_each_respondent_and_pair(self):
        for changed in (
            LOG.replace("refusal cost player=1", "refusal cost player=8"),
            LOG.replace("refusal cost player=1 pair=1-3", "refusal cost player=1 pair=1-8"),
            LOG.replace("refusal cost player=3", "refusal cost player=1"),
            LOG.replace("refusal cost player=1 pair=1-3 turn=15", "refusal cost player=1 pair=1-3 turn=16"),
        ):
            with self.subTest(changed=changed):
                self.assertFalse(self.summary(changed)["measurement_complete"])

    def test_scenario_generator_matches_fixture_and_compiles_every_gameplay_step(self):
        from lupa import LuaRuntime
        generated = builder.scenario()
        actual = json.loads((FOLDER / "scenarios/court-arbitration-ai.json").read_text(encoding="utf-8"))
        self.assertEqual(generated, actual)
        self.assertEqual(len([s for s in actual["steps"] if s["action"] == "save"]), 5)
        self.assertEqual(len([s for s in actual["steps"] if s["action"] == "load"]), 5)
        in_menu = True
        for step in actual["steps"]:
            if step["action"] == "exit_to_menu":
                in_menu = True
            elif step["action"] in ("start_game", "load"):
                self.assertTrue(in_menu, "Network.LoadGame requires FrontEnd; gameplay alone cannot load a save")
                in_menu = False
        self.assertEqual(sum(s["turns"] for s in actual["steps"] if s["action"] == "autoplay"), 7)
        compile_lua = LuaRuntime().eval("function(s) local _,e=load(s); return e end")
        for i, step in enumerate(actual["steps"], 1):
            if "code" in step:
                with self.subTest(step=i):
                    self.assertIsNone(compile_lua(step["code"]))

    def test_saved_snapshot_preserves_nested_ai_state_and_missing_properties(self):
        from tests.test_court_arbitration import ArbitrationAiTests
        harness = ArbitrationAiTests()
        harness.setUp()
        harness.exhaust()
        harness.next_turn(2)
        harness.lua.execute("""Game:SetProperty('JAC_ARB_AI_TEST',{shogun=2,a=0,b=1})""")
        before = harness.lua.execute(builder.SNAPSHOT)
        self.assertIn("Source=ai_party", before)
        self.assertIn("p0_1={ArmyA=4,ArmyB=4,CitiesA=2,CitiesB=2,Since=1}", before)
        self.assertIn("0:2:51:{A=0,B=1,Gold=100,GoldAfter=400,GoldBefore=500,Legitimacy=5,LegitimacyAfter=51,LegitimacyBefore=56,Source=ai_party,Turn=2}", before)
        harness.lua.execute("Players[0]:SetProperty('JAC_ARB_AI_LAST_REQUEST',nil)")
        self.assertNotEqual(harness.lua.execute(builder.SNAPSHOT), before)

    def test_snapshot_checks_saved_costs_instead_of_native_ai_spending_during_load(self):
        from tests.test_court_arbitration import ArbitrationAiTests
        harness = ArbitrationAiTests()
        harness.setUp()
        harness.exhaust()
        harness.next_turn(2)
        harness.lua.execute("Game:SetProperty('JAC_ARB_AI_TEST',{shogun=2,a=0,b=1})")
        before = harness.lua.execute(builder.SNAPSHOT)
        harness.g.Players[3].gold += 10
        harness.g.Players[0].gold += 10
        self.assertEqual(harness.lua.execute(builder.SNAPSHOT), before)
        harness.lua.execute("Players[0]:SetProperty('JAC_ARB_LAST_COST',nil)")
        self.assertNotEqual(harness.lua.execute(builder.SNAPSHOT), before)

    def test_all_live_gameplay_steps_run_with_save_restore_on_lupa(self):
        from lupa import LuaRuntime
        from tests.test_court_arbitration import STUBS, START_TURN, calendar, MOD
        lua = LuaRuntime(unpack_returned_tuples=True)
        data = builder.scenario()
        civs = [p["civ"] for p in data["steps"][1]["players"]]
        lua.execute(STUBS, lua.table_from(calendar(), recursive=True), lua.table_from(civs),
                    START_TURN, (MOD / "Scripts/JAC_CourtArbitration_Rules.lua").as_posix())
        lua.execute((MOD / "Scripts/JAC_CourtArbitration.lua").read_text(encoding="utf-8"))
        lua.execute("""
Players[14].human=true
SET_WAR(14,18,true)
UnitManager={InitUnitValidAdjacentHex=function(id) Players[id].units=Players[id].units+1 end,
 Kill=function(u) local p=Players[u:GetOwner()]; p.units=p.units-1 end}
Game.SetCurrentGameTurn=function(value) CURRENT_TURN=value end
local function copy(v)
 if type(v)~='table' then return v end
 local t={}; for k,x in pairs(v) do t[k]=copy(x) end; return t
end
local saved={}
function STUB_SAVE(name)
 local state={turn=CURRENT_TURN,props=copy(PROPERTY_TABLES),war=copy(WAR),changed=copy(WAR_CHANGED),gold={}}
 for id,p in pairs(Players) do state.gold[id]=p.gold end
 saved[name]=state
end
function STUB_LOAD(name)
 local state=assert(saved[name]); CURRENT_TURN=state.turn; WAR=copy(state.war); WAR_CHANGED=copy(state.changed)
 for i,props in ipairs(PROPERTY_TABLES) do
  for k in pairs(props) do props[k]=nil end
  for k,v in pairs(state.props[i] or {}) do props[k]=copy(v) end
 end
 for id,value in pairs(state.gold) do Players[id].gold=value end
end
START_TURN(1)
""")
        captures, executed = {}, 0
        for index, step in enumerate(data["steps"], 1):
            with self.subTest(step=index, action=step["action"]):
                if step["action"] == "ui_lua":
                    value = lua.execute(step["code"])
                    executed += 1
                    if "capture_as" in step:
                        captures[step["capture_as"]] = value
                    if "equals_capture" in step:
                        self.assertEqual(value, captures[step["equals_capture"]])
                elif step["action"] == "autoplay":
                    lua.globals().START_TURN(lua.globals().JAC_ARB.GameTurn() + step["turns"])
                elif step["action"] == "save":
                    lua.globals().STUB_SAVE(step["name"])
                elif step["action"] == "load":
                    lua.globals().STUB_LOAD(step["name"])
        self.assertEqual((executed, len(captures), lua.globals().JAC_ARB.GameTurn()), (25, 5, 121))
        logs = list(lua.globals().LOG.values())
        self.assertEqual(sum(line.startswith("[JAC_ARB] ai request ") for line in logs), 3)
        self.assertEqual(sum(" by=ai_shogun " in line and line.startswith("[JAC_ARB] ai request ") for line in logs), 2)


if __name__ == "__main__":
    unittest.main()
