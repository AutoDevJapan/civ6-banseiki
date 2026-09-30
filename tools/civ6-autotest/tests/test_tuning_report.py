"""The report must not turn incomplete runs or duplicated observations into evidence."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

path = Path(__file__).resolve().parents[1] / 'tuning_report.py'
spec = importlib.util.spec_from_file_location('tuning_report', path)
report = importlib.util.module_from_spec(spec)
spec.loader.exec_module(report)


class TuningReportTests(unittest.TestCase):
    def make_artifact(self, root, lua, steps):
        root = Path(root)
        (root / 'Lua.log').write_text(lua, encoding='utf-8')
        (root / 'steps.jsonl').write_text('\n'.join(json.dumps(s) for s in steps), encoding='utf-8')
        return root

    def test_counts_ages_policies_and_case_vs_respondent_outcomes(self):
        lua = '''[1] JAC_Ages: [JAC_AGES] judged reign=REIGN_KOJI turn=66 length=65 era=1 engineEra=1 player=0 age=NORMAL score=4 golden>=7 dark>=2 policy=
[1] JAC_Ages: [JAC_AGES] judged reign=REIGN_KOJI turn=66 length=65 era=1 engineEra=1 player=1 age=DARK score=0 golden>=7 dark>=2 policy=POLICY_JAC_TEST
[1] [JAC_ARB] request player=0 pair=0-1 turn=20 power=1
[1] [JAC_ARB] refused player=1 pair=0-1 turn=21 legitimacy=60
[1] [JAC_ARB] request player=2 pair=2-3 turn=40 power=1
[1] [JAC_ARB] accepted pair=2-3 turn=41 truce_turns=10
[1] [JAC_AGES] points player=3 part=ko +8 score=14 key=ko:1 turn=101
'''
        with tempfile.TemporaryDirectory() as tmp:
            data = report.summarize(self.make_artifact(tmp, lua, []))
        self.assertEqual(data['reigns']['REIGN_KOJI']['ages'], {'NORMAL': 1, 'DARK': 1})
        self.assertEqual(data['reigns']['REIGN_KOJI']['policies'], {'POLICY_JAC_TEST': 1})
        self.assertEqual(data['judgments'][1]['score'], 0)
        self.assertEqual(data['judgments'][1]['golden'], 7)
        self.assertEqual(data['arbitration']['request'], 2)
        self.assertEqual(data['arbitration']['accepted'], 1)
        self.assertEqual(data['arbitration']['refused'], 1)
        self.assertEqual(data['arbitration']['refused_cases'], 1)
        self.assertEqual(data['arbitration']['request_intervals'], [20])
        self.assertEqual(data['elimination_point_events'], [{'capturer':3,'player':1,'turn':101}])
        self.assertFalse(data['complete_200_turns'])

    def test_duplicate_judgment_is_rejected(self):
        row = '[JAC_AGES] judged reign=REIGN_KOJI turn=66 length=65 player=0 age=NORMAL score=3 golden>=6 dark>=1 policy=\n'
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(ValueError, 'duplicate age judgment'):
                report.summarize(self.make_artifact(tmp, row * 2, []))

    def test_completion_requires_target_and_passed_judge(self):
        steps = [{'action':f'{i}/18 autoplay','status':'OK','detail':'autoplay completed 50 turns'} for i in range(4)]
        steps.append({'action':'18/18 ui_lua','status':'OK','detail':'ran Lua in InGame; return=T+201 surviving majors=21'})
        with tempfile.TemporaryDirectory() as tmp:
            root = self.make_artifact(tmp, '', steps)
            self.assertFalse(report.summarize(root)['complete_200_turns'])
            steps.append({'action':'judge','status':'OK','detail':'all passed'})
            self.make_artifact(tmp, '', steps)
            self.assertTrue(report.summarize(root)['complete_200_turns'])
            steps.append({'action':'run','status':'FAIL','detail':'failed'})
            self.make_artifact(tmp, '', steps)
            self.assertFalse(report.summarize(root)['complete_200_turns'])

    def test_observer_handles_engine_functions_returning_no_values(self):
        from lupa import LuaRuntime
        lua = LuaRuntime(unpack_returned_tuples=True)
        lua.execute('''
Game={GetCurrentGameTurn=function() return 16603 end,
 GetProperty=function() end, GetEras=function() return {GetCurrentEra=function() return 0 end} end}
GameConfiguration={GetStartTurn=function() return 16603 end}
local city={GetOwner=function() return 0 end}
ExposedMembers={JAC_UnificationCastles={CountedCities=function() return {city} end}}
GameInfo={Eras={[0]={EraType='ERA_ANCIENT'}}, Eras_XP2=function()
 local done=false; return function() if not done then done=true; return {EraType='ERA_ANCIENT',GrievanceDecayRate=10,TensionDecayRate=10,TradeRouteMinimumEndTurnChange=0} end end end}
Players={}; PlayerConfigurations={}
for id=0,21 do
 Players[id]={IsAlive=function() return true end,GetCities=function() return {GetCount=function() return 2 end} end,GetProperty=function() end}
 PlayerConfigurations[id]={GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_TEST' end}
end
''')
        scenario = json.loads((path.parent / 'scenarios/japan-15km-200turn-tuning.json').read_text(encoding='utf-8'))
        code = next(s['code'] for s in scenario['steps'] if s['action']=='ui_lua' and 'checkpoint turn=' in s['code'])
        value = lua.execute(code)
        self.assertEqual(value.count(' ; house player='), 22)
        self.assertTrue(value.startswith('checkpoint turn=1 era=ERA_ANCIENT targets=1 ; era-values grievances=10 tensions=10 trade-minimum-change=0 ; '))
        self.assertIn('house player=21 civ=CIVILIZATION_JAC_TEST alive=true cities=2 castles=0 legitimacy=nil age=nil policy=nil pact-lead=nil', value)

    def test_variant_preserves_original_settings_and_progression(self):
        folder = path.parent / 'scenarios'
        base = json.loads((folder / 'japan-15km-200turn-full.json').read_text(encoding='utf-8'))
        variant = json.loads((folder / 'japan-15km-200turn-tuning.json').read_text(encoding='utf-8'))
        readers = [s for s in variant['steps'] if s['action']=='ui_lua' and 'checkpoint turn=' in s['code']]
        self.assertEqual(len(readers), 5)
        restored = []
        for step in variant['steps']:
            if step in readers:
                continue
            step = dict(step)
            if step['action']=='autoplay':
                self.assertIs(step['metrics'], True)
                if 'metrics' not in base['steps'][len(restored)]:
                    step.pop('metrics')
            restored.append(step)
        self.assertEqual(restored, base['steps'])


if __name__ == '__main__':
    unittest.main()
