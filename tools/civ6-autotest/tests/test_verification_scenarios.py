import importlib.util
import json
from pathlib import Path
import re
import sqlite3
import sys
import unittest
import xml.etree.ElementTree as ET

import lupa

ROOT = Path(__file__).resolve().parents[3]
SCENARIOS = ROOT / "tools/civ6-autotest/scenarios"
sys.path.insert(0, str(SCENARIOS.parent))
from judge import judge
from scenario import ScenarioError, fast_forward_scenario, validate_scenario

NEW_SCENARIOS = (
    "japan-15km-20turn-full-metrics.json", "japan-15km-300turn-full-metrics.json",
    "sengoku-era-reigns-epic.json", "sengoku-era-reigns-marathon.json",
    "sengoku-era-reigns-beyond-nine.json", "great-people-zero-musicians.json",
    "resources-calendar-epic.json",
)


def scenario(name):
    return json.loads((SCENARIOS / name).read_text(encoding="utf-8"))


class VerificationScenarioTests(unittest.TestCase):
    def test_scenario_localization_keys_exist_in_mod_text(self):
        defined = {row.attrib["Tag"] for path in (ROOT / "poc").glob("*/Text/*.xml")
                   for row in ET.parse(path).getroot().iter("Row") if "Tag" in row.attrib}
        self.assertGreater(len(defined), 0)
        for name in NEW_SCENARIOS + ("sengoku-era-reigns.json",):
            keys = set(re.findall(r"LOC_JAC_[A-Z0-9_]+", json.dumps(scenario(name))))
            self.assertEqual(keys - defined, set(), name)

    def test_fast_forward_keeps_checkpoints_without_claiming_continuous_metrics(self):
        original = scenario("japan-15km-300turn-full-metrics.json")
        short = fast_forward_scenario(original)
        validate_scenario(short)
        self.assertEqual(short["test_mode"], {"skipped_turns": 288, "continuous_performance_valid": False})
        self.assertEqual([s["turns"] for s in short["steps"] if s["action"] == "autoplay"], [2]*6+[1])
        self.assertEqual([s["turns"] for s in original["steps"] if s["action"] == "autoplay"], [50]*6+[1])
        self.assertTrue(all(s["metrics_group"].startswith("TEST_MODE_")
                            for s in short["steps"] if s["action"] == "autoplay"))
        self.assertEqual([s for s in short["steps"] if s["action"] in ("save", "load")],
                         [s for s in original["steps"] if s["action"] in ("save", "load")])
        jump = next(s["code"] for s in short["steps"] if "TEST MODE: skipped" in s.get("code", ""))
        lua = lupa.LuaRuntime()
        lua.execute("""
            turn=1
            GameConfiguration={GetRuleSet=function() return 'RULESET_JAC_BANSEIKI' end,
                               IsAnyMultiplayer=function() return false end}
            Game={GetCurrentGameTurn=function() return turn end,
                  SetCurrentGameTurn=function(t) turn=t end}
        """)
        self.assertEqual(lua.execute(jump), "TEST MODE: skipped 48 turns; current=49")
        self.assertEqual(lua.globals().turn, 49)
        lua.execute("GameConfiguration.IsAnyMultiplayer=function() return true end")
        with self.assertRaisesRegex(lupa.LuaError, "single-player test required"):
            lua.execute(jump)
        lua.execute("GameConfiguration.IsAnyMultiplayer=function() return false end")
        lua.execute("Game.SetCurrentGameTurn=function(t) end")
        with self.assertRaisesRegex(lupa.LuaError, "engine fast-forward failed"):
            lua.execute(jump)
        lua.execute("Game.SetCurrentGameTurn=nil")
        with self.assertRaisesRegex(lupa.LuaError, "engine fast-forward unavailable"):
            lua.execute(jump)
        lua.execute("GameConfiguration.GetRuleSet=function() return 'RULESET_STANDARD' end")
        with self.assertRaisesRegex(lupa.LuaError, "test ruleset required"):
            lua.execute(jump)

    def test_all_new_chunks_validate_and_compile_as_lua(self):
        lua = lupa.LuaRuntime()
        compile_chunk = lua.eval("function(code) local f,err=load(code); assert(f,err); return true end")
        for name in NEW_SCENARIOS:
            data = scenario(name)
            with self.subTest(scenario=name):
                validate_scenario(data)
                self.assertIs(data["strict_lua_errors"], True)
                for step in data["steps"]:
                    code = step.get("code") or step.get("condition", {}).get("code")
                    if code:
                        self.assertTrue(compile_chunk(code))

    def test_full_map_measurements_cover_continuous_play_and_both_reloads(self):
        base = scenario("japan-15km-200turn-full.json")
        for turns, check_turns in ((20, [1, 21]), (300, [1, 51, 101, 151, 201, 251, 301])):
            data = scenario(f"japan-15km-{turns}turn-full-metrics.json")
            steps = data["steps"]
            with self.subTest(turns=turns):
                self.assertEqual(steps[0]["ids"], base["steps"][0]["ids"])
                self.assertEqual(len(steps[1]["players"]), 22)
                self.assertEqual([i for i, p in enumerate(steps[1]["players"]) if p["human"]], [20])
                self.assertEqual(steps[1]["players"][20]["civ"], "CIVILIZATION_JAC_RYUKYU")
                self.assertIn("JAC_Japan_15km.Civ6Map", steps[1]["map_file"])
                auto = [s for s in steps if s["action"] == "autoplay"]
                self.assertEqual(sum(s["turns"] for s in auto[:-1]), turns)
                self.assertEqual(auto[-1]["turns"], 1)
                self.assertTrue(all(s["metrics"] for s in auto))
                self.assertEqual([s["metrics_group"] for s in auto], ["CONTINUOUS"]*(len(auto)-1)+["POST_LOAD"])
                self.assertTrue(all(not s.get("research_metrics") for s in auto))
                self.assertTrue(all(not s.get("allow_human_elimination") for s in auto))
                saves = [s for s in steps if s["action"] == "save"]
                self.assertEqual([s["name"] for s in saves], [f"JAC-Perf-T{t}" for t in check_turns])
                self.assertTrue(all(s["metrics"] for s in saves))
                loads = [i for i, s in enumerate(steps) if s["action"] == "load"]
                self.assertEqual([steps[i]["name"] for i in loads], ["JAC-Perf-T1", f"JAC-Perf-T{turns+1}"])
                self.assertTrue(all(steps[i]["metrics"] for i in loads))
                for i in loads:
                    self.assertIn("equals_capture", steps[i + 2])
                    self.assertIn("resourceBaseline", steps[i + 2]["code"])
                    self.assertIn("JAC_AGES_LEDGER", steps[i + 2]["code"])
                # There are no mid-run reloads to reset the continuous memory growth.
                first_auto = next(i for i, s in enumerate(steps) if s["action"] == "autoplay")
                self.assertLess(loads[0], first_auto)
                self.assertGreater(loads[1], next(i for i, s in reversed(list(enumerate(steps)))
                                                    if s["action"] == "save"))

    def test_saved_state_snapshot_uses_indexed_city_iterator_and_detects_changes(self):
        code = next(s["code"] for s in scenario("japan-15km-20turn-full-metrics.json")["steps"]
                    if s.get("capture_as") == "PERF_INITIAL_STATE")
        lua = lupa.LuaRuntime()
        lua.execute("""
            state={population=3,owner=0,unitX=2,damage=4,unitPresent=true,warlord={XP=20},ledger={reign=0,history={}}}
            local city={GetID=function() return 7 end,GetX=function() return 2 end,
                        GetY=function() return 3 end,GetPopulation=function() return state.population end}
            local cities={Members=function() local done=false; return function()
                if done then return nil end; done=true; return 0,city
            end end}
            local research={HasTech=function() return true end}
            local culture={HasCivic=function() return true end}
            local unit={GetID=function() return 8 end,GetOwner=function() return 0 end,
                        GetUnitType=function() return 1 end,GetX=function() return state.unitX end,
                        GetY=function() return 3 end,GetDamage=function() return state.damage end,
                        GetProperty=function(_,key) return state.warlord end}
            local units={Members=function() local done=false; return function()
                if done or not state.unitPresent then return nil end; done=true; return 0,unit
            end end}
            local player={IsAlive=function() return true end,GetCities=function() return cities end,
                          GetUnits=function() return units end,
                          GetTechs=function() return research end,GetCulture=function() return culture end}
            Players={};for i=0,21 do Players[i]=player end
            Game={GetCurrentGameTurn=function() return 1 end,
                  GetEras=function() return {GetCurrentEra=function() return 0 end} end,
                  GetProperty=function(_,key) if key=='JAC_AGES_LEDGER' then return state.ledger end end}
            local function rows(row) return function() local done=false; return function()
                if done then return nil end; done=true;return row
            end end end
            GameInfo={Technologies=rows({Index=0,TechnologyType='TECH_TEST'}),
                      Civics=rows({Index=0,CivicType='CIVIC_TEST'})}
            Map={GetPlotCount=function() return 2 end,
                 GetPlotByIndex=function(i) return {GetOwner=function() return i==0 and 0 or state.owner end} end}
        """)
        before = lua.execute(code)
        self.assertEqual(before, lua.execute(code))
        self.assertIn('population"=number:3', before)
        self.assertIn('"count"=number:2', before)
        lua.globals().state.population = 4
        self.assertNotEqual(before, lua.execute(code))
        lua.globals().state.population = 3
        self.assertEqual(before, lua.execute(code))
        for key, changed in (("unitX", 7), ("damage", 10)):
            old = lua.globals().state[key]
            lua.globals().state[key] = changed
            self.assertNotEqual(before, lua.execute(code))
            lua.globals().state[key] = old
            self.assertEqual(before, lua.execute(code))
        lua.globals().state.warlord.XP = 40
        self.assertNotEqual(before, lua.execute(code))
        lua.globals().state.warlord.XP = 20
        lua.globals().state.unitPresent = False
        self.assertNotEqual(before, lua.execute(code))
        lua.globals().state.unitPresent = True
        self.assertEqual(before, lua.execute(code))
        lua.globals().state.owner = 1
        self.assertNotEqual(before, lua.execute(code))

    def test_boundary_checks_accept_verified_state_and_reject_wrong_turn_era_and_ledger(self):
        kinds = ["ERA_ANCIENT", "ERA_CLASSICAL", "ERA_MEDIEVAL", "ERA_RENAISSANCE",
                 "ERA_INDUSTRIAL", "ERA_MODERN", "ERA_ATOMIC", "ERA_INFORMATION",
                 "ERA_FUTURE", "ERA_JAC_SHOHO", "ERA_JAC_KEIAN"]
        cases = [
            ("sengoku-era-reigns-epic.json", [1, 98, 108, 163], {98: 97, 108: 10, 163: 55}),
            ("sengoku-era-reigns-marathon.json", [1, 196, 217, 328], {196: 195, 217: 21, 328: 111}),
            ("sengoku-era-reigns-beyond-nine.json", {8: 17, 9: 19, 10: 21}, {19: 2, 21: 2}),
        ]
        checked = 0
        for name, starts, lengths in cases:
            for step in scenario(name)["steps"]:
                code = step.get("code", "")
                match = re.search(r"local T,n,kind=(\d+),(\d+),'([^']+)'", code)
                if not match:
                    continue
                turn, reign, kind = int(match[1]), int(match[2]), match[3]
                lua = lupa.LuaRuntime()
                lua.execute("""
                    function print() end
                    state={}; ledger={history={}}
                    local eras={GetCurrentEra=function() return state.era end,
                                GetCurrentEraStartTurn=function() return state.start end}
                    Game={GetCurrentGameTurn=function() return state.turn end,
                          GetEras=function() return eras end,
                          GetProperty=function() return ledger end}
                    GameConfiguration={GetStartTurn=function() return 1 end}
                    GameInfo={Eras={}}
                """)
                for index, era in enumerate(kinds):
                    lua.globals().GameInfo.Eras[era] = lua.table_from({"Index": index})
                lua.globals().state.turn = turn
                lua.globals().state.era = kinds.index(kind)
                lua.globals().state.start = starts[reign]
                lua.globals().ledger.reign = reign
                if turn in lengths:
                    lua.execute(f"ledger.history.r{reign}={{turn={turn},due={turn},length={lengths[turn]},"
                                f"from={reign-1},p0={{}},engineEra=-1}}")
                with self.subTest(scenario=name, turn=turn, context=step["state_name"]):
                    self.assertTrue(lua.execute(code))
                    lua.globals().state.turn = turn + 1
                    with self.assertRaises(lupa.LuaError):
                        lua.execute(code)
                    lua.globals().state.turn = turn
                    lua.globals().state.era = -1
                    with self.assertRaises(lupa.LuaError):
                        lua.execute(code)
                    lua.globals().state.era = kinds.index(kind)
                    if turn in lengths:
                        lua.execute(f"ledger.history.r{reign}.due={turn+1}")
                        with self.assertRaises(lupa.LuaError):
                            lua.execute(code)
                checked += 1
        self.assertEqual(checked, 34)

    def test_optional_tail_is_scoped_hidden_and_preserves_endless_last_era(self):
        mod = ROOT / "poc/JAC_Test_EraTiming"
        meta = ET.parse(mod / "JAC_Test_EraTiming.modinfo").getroot()
        criteria = meta.find("./ActionCriteria/Criteria[@id='JAC_Test_EraTiming_Tail']")
        self.assertEqual(criteria.findtext("RuleSetInUse"), "RULESET_JAC_BANSEIKI")
        matches = {(e.findtext("Group"), e.findtext("ConfigurationId")): e.findtext("Value")
                   for e in criteria.findall("ConfigurationValueMatches")}
        self.assertEqual(matches, {
            ("Map", "MAP_SCRIPT"): "{5c1e7a3d-9b2f-4e61-8d47-2a6f0c9e1b85}Maps/JAC_Unification_Test.Civ6Map",
            ("Game", "JAC_ERA_TIMING_TAIL"): "1",
        })
        config = ET.parse(mod / "Config/JAC_Test_EraTiming_Config.xml").getroot().find("./Parameters/Row")
        self.assertEqual((config.get("Visible"), config.get("DefaultValue"), config.get("Domain")), ("0", "0", "bool"))
        for speed in ("standard", "quick"):
            self.assertIn("SetValue('JAC_ERA_TIMING_TAIL', false)", scenario(f"probe-era-timing-{speed}.json")["steps"][2]["code"])
        spec = importlib.util.spec_from_file_location("verification_reigns", ROOT / "tools/generate_sengoku_reigns.py")
        generator = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(generator)
        db = sqlite3.connect(":memory:")
        self.addCleanup(db.close)
        db.executescript("CREATE TABLE Eras(EraType TEXT PRIMARY KEY,ChronologyIndex INTEGER);"
                         "CREATE TABLE Eras_XP1(EraType TEXT PRIMARY KEY,GameEraMinimumTurns INTEGER,GameEraMaximumTurns INTEGER);"
                         "CREATE TABLE GlobalParameters(Name TEXT PRIMARY KEY,Value TEXT);")
        eras = generator.eras()
        db.executemany("INSERT INTO Eras VALUES (?,?)", [(e["EraType"], e["ChronologyIndex"]) for e in eras])
        db.executemany("INSERT INTO Eras_XP1 VALUES (?,?,?)", [(e["EraType"], e["length"], e["length"]) for e in eras])
        db.executescript((mod / "Data/JAC_Test_EraTiming_Tail.sql").read_text(encoding="utf-8"))
        self.assertEqual(db.execute("SELECT COUNT(*) FROM Eras_XP1 WHERE GameEraMinimumTurns=2 AND GameEraMaximumTurns=2").fetchone(), (47,))
        self.assertEqual(db.execute("SELECT GameEraMinimumTurns,GameEraMaximumTurns FROM Eras_XP1 WHERE EraType='ERA_JAC_CHIDAI'").fetchone(), (None, None))
        self.assertEqual(db.execute("SELECT Value FROM GlobalParameters WHERE Name='JAC_TEST_ERA_TAIL_VERSION'").fetchone(), ("1",))

    def test_resource_calendar_calls_actual_getter_and_rejects_wrong_year(self):
        data = scenario("resources-calendar-epic.json")
        checks = [s for s in data["steps"] if s["action"] == "ui_lua" and "local actual=api.CurrentYear()" in s["code"]]
        expected = {1: 1534, 25: 1540, 41: 1543}
        self.assertEqual(len(checks), 6)
        for step in checks:
            code = step["code"]
            turn = int(re.search(r"if turn~=(\d+)", code)[1])
            lua = lupa.LuaRuntime()
            lua.execute("""
                function print() end
                state={}
                local rows={}; for i=1,99 do rows[i]={} end
                Game={GetCurrentGameTurn=function() return state.turn end}
                GameConfiguration={GetStartTurn=function() return 1 end,GetGameSpeedType=function() return 0 end}
                GameInfo={GameSpeeds={[0]={GameSpeedType='GAMESPEED_EPIC',GameSpeedTurnCollection=rows}}}
                ExposedMembers={JAC_Resources={CurrentYear=function() return state.year end}}
                Calendar={MakeYearStr=function() return tostring(state.year)..' AD' end}
            """)
            lua.globals().state.turn = turn
            lua.globals().state.year = expected[turn]
            with self.subTest(turn=turn, context=step["state_name"]):
                self.assertIn(f"turn={turn} year={expected[turn]} rows=99", lua.execute(code))
                lua.globals().state.year = expected[turn] + 1
                with self.assertRaises(lupa.LuaError):
                    lua.execute(code)
                if step["state_name"] == "GameCore_Tuner":
                    self.assertNotIn("Calendar.", code)

    def test_strict_judge_rejects_official_screen_errors(self):
        for message in ("Runtime Error: GreatPeoplePopup.lua:123: attempt to index nil",
                        "UI.DataError: EraComplete cannot show this era"):
            logs = {"Lua.log": message, "Database.log": "", "Modding.log": ""}
            self.assertTrue(all(c.passed for c in judge(logs, "generic")))
            strict = judge(logs, "generic", strict_lua_errors=True)
            self.assertFalse(strict[-1].passed)
        self.assertTrue(all(c.passed for c in judge({"Lua.log": "", "Database.log": "", "Modding.log": ""},
                                                   "generic", strict_lua_errors=True)))
        with self.assertRaisesRegex(ScenarioError, "strict_lua_errors"):
            validate_scenario({"name": "bad flag", "strict_lua_errors": 1,
                               "steps": [{"action": "autoplay", "turns": 1}]})
