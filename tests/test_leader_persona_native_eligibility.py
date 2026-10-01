"""Independent engine-boundary diagnostics; these never simulate a successful Civ6 run.

The scenario reads the native UI cache directly. The fixture deliberately does
not load any persona Lua or its native-reader bridge, so hiding a UI candidate
cannot make an eligible engine candidate pass this check.
"""
import json
from pathlib import Path
import unittest

from lupa import LuaError, LuaRuntime

ROOT = Path(__file__).resolve().parents[1]
SCENARIOS = ROOT / "tools/civ6-autotest/scenarios"


class NativeEligibilityDiagnosticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.scenario = json.loads((SCENARIOS / "leader-persona-native-eligibility-15km.json")
                                  .read_text(encoding="utf-8"))
        cls.gate = next(s["code"] for s in cls.scenario["steps"]
                        if s.get("code", "").startswith("-- UI cache: read the ENGINE directly"))

    def setUp(self):
        self.lua = LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(r'''
            PROPERTIES, EVER, NOW, APPOINTED = {}, {}, {}, {}
            Game={GetProperty=function(_,key) return PROPERTIES[key] end}
            ExposedMembers=setmetatable({}, {__index=function() error('persona bridge must not be consulted') end})
            Players, ROWS, DEFS = {}, {}, {}
            for i=0,21 do
                local id=i
                local def={Index=i,Hash=9000+i,GovernorType='GOVERNOR_TEST_'..i}
                DEFS[def.GovernorType]=def
                ROWS[#ROWS+1]={LeaderType='LEADER_TEST_'..i,GovernorType=def.GovernorType}
                EVER[id],NOW[id],APPOINTED[id]={},{},{}
                local function hash(value) return value<22 and value+9000 or value end
                local govs={
                    CanEverAppointGovernor=function(_,value) return EVER[id][hash(value)]==true end,
                    CanAppointGovernor=function(_,value) return NOW[id][hash(value)]==true end,
                    HasGovernor=function(_,value) return APPOINTED[id][value]==true end,
                    GetGovernor=function(_,value) return {GetType=function() return value-9000 end} end,
                    GetGovernorPoints=function() return id==0 and 0 or 1 end,
                    GetGovernorPointsSpent=function() return 0 end
                }
                Players[id]={GetID=function() return id end,
                    IsHuman=function() return id==0 end,GetGovernors=function() return govs end}
            end
            GameInfo={Governors=DEFS,JAC_LeaderPersonas=function()
                local i=0; return function() i=i+1; return ROWS[i] end
            end}
            PlayerManager={GetAliveMajors=function()
                local result={}; for i=0,21 do result[#result+1]=Players[i] end; return result
            end}
        ''')
        self.g = self.lua.globals()

    def test_closed_unjoined_engine_candidates_pass_with_21_positive_ai_budgets(self):
        self.assertEqual(self.lua.execute(self.gate), "484 engine eligibility checks passed")
        self.assertEqual(self.g.Players[1].GetGovernors().GetGovernorPoints(), 1)

    def test_hidden_ui_candidate_cannot_hide_native_can_ever_eligibility(self):
        # There is no persona screen at all in this fixture. The engine alone
        # reports a candidate available to an AI, exactly the root failure.
        self.g.EVER[1][9005] = True
        with self.assertRaisesRegex(LuaError, "unjoined native eligibility: 1/LEADER_TEST_5/ever=true"):
            self.lua.execute(self.gate)

    def test_no_titles_cannot_hide_native_can_ever_eligibility(self):
        self.assertEqual(self.g.Players[0].GetGovernors().GetGovernorPoints(), 0)
        self.g.EVER[0][9005] = True
        with self.assertRaisesRegex(LuaError, "0/LEADER_TEST_5/ever=true"):
            self.lua.execute(self.gate)

    def test_native_can_appoint_is_checked_even_if_can_ever_reports_false(self):
        self.g.NOW[1][9005] = True
        with self.assertRaisesRegex(LuaError, "1/LEADER_TEST_5/ever=false/now=true"):
            self.lua.execute(self.gate)

    def test_wrong_owner_and_already_appointed_unjoined_governor_are_rejected(self):
        self.g.PROPERTIES["JAC_PERSONA_LEADER_TEST_5_OWNER"] = 0
        self.g.EVER[0][9005] = True
        self.g.NOW[0][9005] = True
        self.assertEqual(self.lua.execute(self.gate), "484 engine eligibility checks passed")
        self.g.APPOINTED[1][9005] = True
        with self.assertRaisesRegex(LuaError, "1/LEADER_TEST_5/ever=false/now=false/appointed=true/nativeType=5"):
            self.lua.execute(self.gate)

    def test_all_22_players_and_types_are_required(self):
        self.lua.execute("table.remove(ROWS)")
        with self.assertRaisesRegex(LuaError, "all 22 houses and 22 persona types required"):
            self.lua.execute(self.gate)

    def test_hash_index_disagreement_is_not_treated_as_a_closed_gate(self):
        self.lua.execute('''
            Players[0]:GetGovernors().CanAppointGovernor=function(_,value) return value<22 end
        ''')
        with self.assertRaisesRegex(LuaError, "native Hash/Index eligibility disagreement"):
            self.lua.execute(self.gate)

    def test_native_methods_must_exist_in_the_ui_context(self):
        self.g.Players[0].GetGovernors().CanEverAppointGovernor = None
        with self.assertRaisesRegex(LuaError, "native eligibility getters unavailable in UI"):
            self.lua.execute(self.gate)

    def test_diagnostic_keeps_real_turn_payment_reloads_and_existing_full_assertions(self):
        steps = self.scenario["steps"]
        full = json.loads((SCENARIOS / "leader-persona-15km.json").read_text(encoding="utf-8"))
        self.assertEqual(len(full["steps"]), 113)
        self.assertEqual(steps[:5], full["steps"][:5])
        self.assertEqual([s["turns"] for s in steps if s["action"] == "end_turn"], [1])
        self.assertFalse(any(s["action"] == "autoplay" for s in steps))
        self.assertIn(full["steps"][35], steps)  # Real civic-funded AI budgets.
        self.assertIn(full["steps"][38], steps)  # Existing early native appointment/type failure.
        self.assertIn(full["steps"][25], steps)  # Full cost/base/native payment/GP removal.
        saves = [s["name"] for s in steps if s["action"] == "save"]
        self.assertEqual(saves, ["JAC-Native-Joined", "JAC-Native-Evolved"])
        self.assertEqual([s["name"] for s in steps if s["action"] == "load"], saves)
        self.assertEqual([s["capture_as"] for s in steps if "capture_as" in s],
                         ["JAC_NATIVE_UNJOINED", "JAC_NATIVE_JOINED_PARTIAL", "JAC_NATIVE_EVOLVED"])
        self.assertEqual([s["equals_capture"] for s in steps if "equals_capture" in s],
                         ["JAC_NATIVE_JOINED_PARTIAL", "JAC_NATIVE_EVOLVED"])
        for step in steps:
            if step.get("code") == self.gate:
                self.assertEqual(step["state_name"], "InGame")
        self.assertIn("native appointment exposed before full cost authorization",
                      "\n".join(s.get("code", "") for s in steps))

    def test_all_diagnostic_chunks_compile(self):
        compile_chunk = self.lua.eval("function(code) local f,err=load(code); assert(f,err); return true end")
        for step in self.scenario["steps"]:
            code = step.get("code") or step.get("condition", {}).get("code")
            if code:
                with self.subTest(action=step["action"]):
                    self.assertTrue(compile_chunk(code))


if __name__ == "__main__":
    unittest.main()
