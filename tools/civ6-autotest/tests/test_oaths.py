from pathlib import Path
import json
import sys
import unittest
import lupa

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scenario import validate_scenario


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
