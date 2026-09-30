from pathlib import Path
import json
import sys
import unittest
import lupa

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scenario import validate_scenario, fast_forward_scenario


class OathScenarioTests(unittest.TestCase):
    def setUp(self):
        path = Path(__file__).resolve().parents[1] / 'scenarios/sengoku-oaths-oda.json'
        self.data = json.loads(path.read_text(encoding='utf-8'))

    def test_scenario_is_valid_starts_as_oda_and_checks_load_before_advancing(self):
        validate_scenario(self.data)
        steps = self.data['steps']
        config = next(s for s in steps if s['action'] == 'configure')
        self.assertEqual(config['ruleset'], 'RULESET_JAC_BANSEIKI')
        self.assertEqual(config['players'][0]['civ'], 'CIVILIZATION_JAC_ODA')
        self.assertTrue(config['players'][0]['human'])
        self.assertEqual(sum(p['human'] for p in config['players']), 1)
        self.assertEqual(len(config['players']), 22)
        load = next(i for i,s in enumerate(steps) if s['action'] == 'load')
        self.assertEqual(steps[load+1]['action'], 'ui_lua')
        self.assertEqual(steps[load+1]['state_name'], 'GameCore_Tuner')
        self.assertIn("snapshot~=Game:GetProperty('JAC_OATH_TEST_SNAPSHOT')", steps[load+1]['code'])
        self.assertIn('city:GetYield(culture)', steps[load+1]['code'])
        self.assertIn('on>off', steps[load+1]['code'])

    def test_every_embedded_lua_step_and_wait_compiles(self):
        compiler = lupa.LuaRuntime(unpack_returned_tuples=True).eval(
            'function(code) local fn,err=load(code); return fn~=nil,err end')
        checked = 0
        for step in self.data['steps']:
            code = step.get('code') or step.get('condition',{}).get('code')
            if code:
                ok,error = compiler(code)
                self.assertTrue(ok, error)
                checked += 1
        self.assertGreaterEqual(checked, 12)

    def test_automatic_popup_assertion_cannot_force_a_missing_screen_open(self):
        code = next(s['code'] for s in self.data['steps'] if s.get('state_name') == 'DedicationPopup')
        lua = lupa.LuaRuntime()
        lua.execute("ContextPtr={IsHidden=function() return true end}; JAC_ShowOaths=function() error('forced popup') end")
        with self.assertRaisesRegex(lupa.LuaError, 'did not automatically open'):
            lua.execute(code)

    def test_fast_forward_runs_engine_jump_with_scope_checks_and_keeps_source_intact(self):
        source = {'name': 'test', 'steps': [{'action': 'autoplay', 'turns': 64, 'timeout': 14400}]}
        result = fast_forward_scenario(source)
        self.assertEqual(source['steps'][0]['turns'], 64)
        self.assertEqual(result['steps'][2]['turns'], 2)
        self.assertEqual([s.get('state_name') for s in result['steps'][:2]], ['InGame', 'GameCore_Tuner'])
        self.assertEqual(result['test_mode'], {'skipped_turns': 62, 'continuous_performance_valid': False})
        lua = lupa.LuaRuntime()
        lua.execute("TURN=1; RULE='RULESET_JAC_BANSEIKI'; MULTI=false; Game={GetCurrentGameTurn=function() return TURN end,SetCurrentGameTurn=function(t) TURN=t end}; GameConfiguration={GetRuleSet=function() return RULE end,IsAnyMultiplayer=function() return MULTI end}")
        lua.execute(result['steps'][0]['code'])
        code = result['steps'][1]['code']
        lua.execute(code)
        self.assertEqual(lua.globals().TURN, 63)
        for rule, multi, message in (('RULESET_STANDARD', False, 'test ruleset required'),
                                     ('RULESET_JAC_BANSEIKI', True, 'single-player test required')):
            lua.globals().RULE, lua.globals().MULTI = rule, multi
            with self.assertRaisesRegex(lupa.LuaError, message):
                lua.execute(result['steps'][0]['code'])
            self.assertEqual(lua.globals().TURN, 63)
        # GameCore really lacks IsAnyMultiplayer: the jump must not call a UI-only API.
        lua.globals().RULE = 'RULESET_JAC_BANSEIKI'
        lua.execute('GameConfiguration.IsAnyMultiplayer=nil')
        lua.execute('GameConfiguration.GetRuleSet=nil')
        lua.execute(code)
        self.assertEqual(lua.globals().TURN, 125)
