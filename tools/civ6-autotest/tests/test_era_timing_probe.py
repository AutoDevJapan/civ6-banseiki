"""Static and Lua checks for scenarios/probe-era-timing-{standard,quick}.json (documents/poc/era-equals-reign.md)."""
from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SCENARIOS = ROOT / "tools" / "civ6-autotest" / "scenarios"
PROBE_MOD = "57a3af09-fb38-41da-b367-818de11a995c"
SQL = ROOT / "poc" / "JAC_Test_EraTiming" / "Data" / "JAC_Test_EraTiming.sql"


def steps(speed: str) -> list[dict]:
    return json.loads((SCENARIOS / f"probe-era-timing-{speed}.json").read_text(encoding="utf-8"))["steps"]


class EraTimingProbeTests(unittest.TestCase):
    def test_every_single_turn_is_logged_in_both_contexts(self):
        for speed in ("standard", "quick"):
            s = steps(speed)
            chunks = [i for i, step in enumerate(s) if step["action"] == "autoplay"]
            self.assertGreaterEqual(len(chunks), 15)
            for i in chunks:
                with self.subTest(speed=speed, step=i + 1):
                    self.assertEqual(s[i]["turns"], 1)
                    self.assertEqual([x["state_name"] for x in s[i + 1:i + 3]], ["GameCore_Tuner", "InGame"])
                    self.assertTrue(all("JAC_ERP.Log(" in x["code"] for x in s[i + 1:i + 3]))

    def test_speeds_differ_only_in_speed(self):
        std, quick = steps("standard"), steps("quick")
        self.assertIn(PROBE_MOD, std[0]["ids"])
        self.assertEqual(std[1]["speed"], "GAMESPEED_STANDARD")
        self.assertEqual(quick[1]["speed"], "GAMESPEED_QUICK")
        text = json.dumps(quick).replace("GAMESPEED_QUICK", "GAMESPEED_STANDARD")
        self.assertEqual(json.loads(text), std)

    def test_setup_guard_matches_the_test_mod_sql(self):
        sql = {era: f"{lo}/{hi}" for lo, hi, era in re.findall(
            r"GameEraMinimumTurns = (\d+), GameEraMaximumTurns = (\d+) WHERE EraType = '(\w+)'", SQL.read_text())}
        guard = next(x["code"] for x in steps("standard") if "local want=" in x.get("code", ""))
        want = dict(re.findall(r"(ERA_\w+)='(\d+/\d+)'", guard))
        self.assertEqual(want, sql)
        self.assertIn("('NEXT_ERA_TURN_COUNTDOWN', '1')", SQL.read_text())

    def test_other_scenarios_on_the_test_map_disable_the_probe_mod(self):
        # enable_mods never disables other mods, and the probe mod acts on the unification test map.
        for path in SCENARIOS.glob("*.json"):
            text = path.read_text(encoding="utf-8")
            if "JAC_Unification_Test.Civ6Map" in text and not path.name.startswith("probe-era-timing"):
                with self.subTest(path=path.name):
                    self.assertIn(PROBE_MOD, json.loads(text)["steps"][0]["disable_ids"])

    def test_gamecore_logger_never_raises_and_reports_changes(self):
        try:
            import lupa
        except ImportError:
            self.skipTest("lupa is not installed")
        lua = lupa.LuaRuntime()
        lua.execute("""
          printed={}; function print(s) printed[#printed+1]=s end
          state={turn=1, era=0}
          local erasRows={[0]={EraType='ERA_ANCIENT'},[1]={EraType='ERA_CLASSICAL'}}
          local function iter(t) local i=0; return function() i=i+1; return t[i] end end
          GameInfo={Eras=erasRows, Technologies=function() return iter({{Index=0},{Index=1}}) end,
                    Civics=function() return iter({{Index=0}}) end}
          local eras={GetCurrentEra=function() return state.era end, GetCurrentEraStartTurn=function() return 1 end,
                      GetNextEraCountdown=function() error('boom') end}
          local p={GetID=function() return 0 end, GetEra=function() return state.era end,
                   GetTechs=function() return {HasTech=function(_, i) return i==0 end} end,
                   GetCulture=function() return {} end}
          Game={GetCurrentGameTurn=function() return state.turn end, GetEras=function() return eras end}
          GameConfiguration={GetStartTurn=function() return 1 end}
          PlayerManager={GetAliveMajors=function() return {p} end}
        """)
        code = next(x["code"] for x in steps("standard") if x.get("state_name") == "GameCore_Tuner")
        first = lua.execute(code)
        self.assertIn("[JAC_ERA_PROBE] ctx=GameCore tag=setup turn=1 elapsed=1", first)
        self.assertIn("era=0:ERA_ANCIENT", first)
        self.assertIn("countdown=ERR(", first)       # a raising getter is reported, not raised
        self.assertIn("minEnd=absent", first)         # a missing getter is reported as absent
        self.assertIn("date=ERR(", first)             # no Calendar in this fake context
        self.assertIn("p0:era=0:ERA_ANCIENT/techs=1/civics=ERR(", first)
        self.assertNotIn("CHANGED", first)
        lua.execute("state.turn=4; state.era=1")
        after = lua.execute(next(x["code"] for x in steps("standard") if x.get("state_name") == "GameCore_Tuner"
                                 and "after3" in x["code"]))
        self.assertIn("elapsed=4", after)
        self.assertIn("CHANGED from=ERA_ANCIENT eraLenObserved=3", after)
        self.assertEqual(list(lua.globals().printed.values()), [first, after])


if __name__ == "__main__":
    unittest.main()
