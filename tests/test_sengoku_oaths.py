"""Oaths on the real GS schema, the live ledger and rendered official-popup controls (lupa)."""
from __future__ import annotations
import importlib.util
import json
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path
import lupa
from tests import test_sengoku_ages as ages
AUDIT, MOD, UI_STUBS = ages.AUDIT, ages.MOD, ages.UI_STUBS
from tests.test_sengoku_ages_screens import SCREEN_STUBS, ACTION_PANEL_STUBS, NOTIFICATION_PANEL_STUBS, _files

ROOT = Path(__file__).resolve().parents[1]
CATALOGUE = json.loads((ROOT/'documents/poc/data/sengoku-oaths.json').read_text(encoding='utf-8'))['houses']
ODA = ['JAC_OATH_ODA_TENKA_FUBU', 'JAC_OATH_ODA_TSUSHIMA', 'JAC_OATH_ODA_AZUCHI']


class OathDataTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # No skip: this workspace has the official game and the requested Lua dependencies.
        cls.original = AUDIT.official_db()
        cls.db = AUDIT._run_sengoku(cls.original, True)

    def test_exactly_three_unique_oaths_for_each_of_the_22_registered_houses(self):
        rows = self.db.execute('SELECT CivilizationType, COUNT(*), MIN(Position), MAX(Position) FROM JAC_SengokuOaths GROUP BY CivilizationType').fetchall()
        civ_text = ''.join((ROOT/'poc/JAC_PoC_CustomCivs/Data'/f).read_text(encoding='utf-8') for f in ('JAC_CustomCivs_Gameplay.sql','JAC_Japan1534_Gameplay.sql'))
        import re
        expected = set(re.findall(r"\('(CIVILIZATION_JAC_[A-Z_]+)', 'KIND_CIVILIZATION'\)", civ_text))
        self.assertEqual(len(expected), 22)
        self.assertEqual({r[0] for r in rows}, expected)
        self.assertEqual({r[1:] for r in rows}, {(3,1,3)})
        self.assertEqual(len(self.db.execute('SELECT DISTINCT OathType FROM JAC_SengokuOaths').fetchall()), 66)

    def test_all_four_ages_have_the_same_three_house_choices(self):
        rows = self.db.execute('SELECT OathType, GROUP_CONCAT(AgeType) FROM JAC_SengokuOathAges GROUP BY OathType').fetchall()
        self.assertEqual(len(rows),66)
        for _, ages in rows:
            self.assertEqual(set(ages.split(',')), {'NORMAL','DARK','GOLDEN','HEROIC'})

    def test_official_commemorations_removed_without_deleting_history_or_touching_gs(self):
        moments = self.original.execute('SELECT COUNT(*) FROM Moments').fetchone()[0]
        self.assertGreater(moments,100)
        self.assertEqual(self.db.execute('SELECT COUNT(*) FROM Moments').fetchone()[0], moments)
        for table in ('CommemorationTypes','CommemorationModifiers','ComplimentModifiers'):
            if table != 'ComplimentModifiers':  # reserved schema table has no GS rows
                self.assertGreater(self.original.execute(f'SELECT COUNT(*) FROM {table}').fetchone()[0],0)
            self.assertEqual(self.db.execute(f'SELECT COUNT(*) FROM {table}').fetchone()[0],0)
        self.assertEqual(self.db.execute('SELECT DISTINCT CommemorationType FROM Moments').fetchall(),[(None,)])
        self.assertEqual(self.original.execute("SELECT Value FROM GlobalParameters WHERE Name='COMMEMORATE_BASE_CHOICES_ALLOWED'").fetchone()[0],'1')
        meta = ET.parse(MOD/'JAC_PoC_SengokuTrees.modinfo').getroot()
        for action in meta.find('InGameActions'):
            self.assertEqual(action.get('criteria'),'JAC_Sengoku_BanseikiOnly')
        replacement = meta.find("./InGameActions/ReplaceUIScript[@id='JAC_Sengoku_OathsPopup']")
        self.assertEqual(replacement.findtext('./Properties/LuaContext'),'DedicationPopup')
        self.assertEqual(replacement.findtext('./Properties/LuaReplace'),'UI/JAC_DedicationPopup_Sengoku.lua')

    def test_every_effect_has_an_official_type_reversible_gate_and_attachment(self):
        mappings = self.db.execute('SELECT OathType, ModifierId FROM JAC_SengokuOathEffects').fetchall()
        self.assertEqual({k for k,_ in mappings},{r[0] for r in self.db.execute('SELECT OathType FROM JAC_SengokuOaths')})
        for oath, mid in mappings:
            row = self.db.execute('SELECT ModifierType, OwnerRequirementSetId, SubjectRequirementSetId, Permanent FROM Modifiers WHERE ModifierId=?',(mid,)).fetchone()
            self.assertIsNotNone(row,mid)
            self.assertEqual(row[3],0)
            self.assertIsNotNone(self.original.execute('SELECT 1 FROM DynamicModifiers WHERE ModifierType=?',(row[0],)).fetchone(), row[0])
            if mid.endswith('_ATTACH'):
                child = self.db.execute("SELECT Value FROM ModifierArguments WHERE ModifierId=? AND Name='ModifierId'",(mid,)).fetchone()[0]
                self.assertEqual(self.db.execute('SELECT OwnerRequirementSetId FROM Modifiers WHERE ModifierId=?',(child,)).fetchone()[0], oath+'_CITY')
            else:
                self.assertIn(oath+'_CITY',row[1:3],mid)
            self.assertEqual(self.db.execute("SELECT Value FROM RequirementArguments WHERE RequirementId=? AND Name='PropertyName'",(oath+'_REQ',)).fetchone()[0],oath+'_ON')
        violations = self.db.execute('PRAGMA foreign_key_check').fetchall()
        self.assertEqual([v for v in violations if v[0].startswith('JAC_SengokuOath')], [])
        for table,rowid,parent,key in violations:
            if table == 'Modifiers':
                mid = self.db.execute('SELECT ModifierId FROM Modifiers WHERE rowid=?',(rowid,)).fetchone()[0]
                self.assertFalse(mid.startswith('JAC_OATH_'), (mid,parent,key))

    def test_balance_amounts_and_tenka_actual_modifiers(self):
        amounts = [int(r[0]) for r in self.db.execute("SELECT Value FROM ModifierArguments WHERE ModifierId LIKE 'JAC_OATH_%' AND Name='Amount'")]
        self.assertGreater(len(amounts),132)
        self.assertGreaterEqual(min(amounts),5)
        self.assertLessEqual(max(amounts),20)
        self.assertEqual(self.db.execute("SELECT ModifierType FROM Modifiers WHERE ModifierId='JAC_OATH_ODA_TENKA_FUBU_E1'").fetchone()[0], 'MODIFIER_PLAYER_CITIES_ADJUST_MILITARY_UNITS_PRODUCTION')
        self.assertEqual(self.db.execute("SELECT Value FROM ModifierArguments WHERE ModifierId='JAC_OATH_ODA_TENKA_FUBU_E1' AND Name='Amount'").fetchone()[0],'15')
        self.assertEqual(self.db.execute("SELECT Name,Value FROM ModifierArguments WHERE ModifierId='JAC_OATH_ODA_TENKA_FUBU_E2' ORDER BY Name").fetchall(),[('Amount','5'),('YieldType','YIELD_CULTURE')])

    def test_effect_arguments_resolve_to_actual_buildings_and_yields(self):
        rows = self.db.execute("SELECT ModifierId,Name,Value FROM ModifierArguments WHERE ModifierId LIKE 'JAC_OATH_%' AND Name IN ('BuildingType','YieldType')").fetchall()
        self.assertGreater(len(rows),100)
        for mid,name,value in rows:
            table,column = ('Buildings','BuildingType') if name=='BuildingType' else ('Yields','YieldType')
            self.assertIsNotNone(self.db.execute(f'SELECT 1 FROM {table} WHERE {column}=?',(value,)).fetchone(), (mid,name,value))
        walls={r[2] for r in rows if r[1]=='BuildingType'}
        self.assertEqual(walls,{'BUILDING_WALLS','BUILDING_CASTLE','BUILDING_STAR_FORT'})

    def test_every_name_and_effect_translated_and_history_sourced(self):
        texts = {}
        for row in ET.parse(MOD/'Text/JAC_Sengoku_Oaths_Text.xml').getroot().iter('Row'):
            texts.setdefault(row.get('Tag'),{})[row.get('Language')] = row.findtext('Text')
        for names in texts.values():
            self.assertEqual(set(names),{'ja_JP','en_US','ru_RU'})
            self.assertTrue(all(v.strip() for v in names.values()))
        for house in CATALOGUE:
            self.assertEqual(len(house['oaths']),3)
            for oath in house['oaths']:
                self.assertTrue(oath['basis'])
                self.assertTrue(oath['sources'])
                self.assertTrue(all(s.startswith('https://') for s in oath['sources']))
        for lang in ('ja_JP','en_US','ru_RU'):
            names = [v[lang] for k,v in texts.items() if k.endswith('_NAME')]
            self.assertEqual(len(names),66)
            self.assertEqual(len(set(names)),66)
        self.assertEqual([k for k,v in texts.items() if v['ja_JP']=='天下布武'],['LOC_JAC_OATH_ODA_TENKA_FUBU_NAME'])

    def test_generated_data_current(self):
        spec = importlib.util.spec_from_file_location('oath_generator',ROOT/'tools/generate_sengoku_oaths.py')
        gen=importlib.util.module_from_spec(spec)
        spec.loader.exec_module(gen)
        for path,expected in gen.outputs().items():
            self.assertEqual(path.read_text(encoding='utf-8'),expected)


class OathLedgerTests(unittest.TestCase):
    load_script, turns, house, snapshot, plot = (ages.LedgerTests.load_script,ages.LedgerTests.turns,ages.LedgerTests.house,ages.LedgerTests.snapshot,ages.LedgerTests.plot)

    def setUp(self):
        ages.LedgerTests.setUp(self)
        self.g.HUMAN[0] = True

    def judge(self, age='GOLDEN'):
        self.turns(1,65)
        self.house(0).age = 'DARK' if age=='HEROIC' else 'NONE'
        self.house(0).score = {'GOLDEN':1000,'HEROIC':1000,'NORMAL':4,'DARK':0}[age]
        self.turns(66)
        self.assertEqual(self.house(0).age,age)

    def choose(self, oaths=ODA[0], player=0, reign=1, **extra):
        values = [oaths] if isinstance(oaths,str) else oaths
        params={'Reign':reign,**{f'Oath{i}':v for i,v in enumerate(values,1)},**extra}
        self.g.CHOOSE_OATHS(player,self.lua.table_from(params,recursive=True))

    def selected(self, player=0):
        return {k for k,v in (self.house(player).oaths or {}).items() if v}

    def test_no_oath_or_effect_before_first_reign(self):
        self.turns(1,65)
        self.choose(reign=0)
        self.assertIsNone(self.house(0).oathChosen)
        self.assertEqual(self.selected(),set())
        self.assertEqual(self.plot(1,1).GetProperty(None,ODA[0]+'_ON'),0)

    def test_each_age_offers_exactly_odas_three_and_no_other_house(self):
        for age in ('NORMAL','DARK','GOLDEN','HEROIC'):
            h = self.lua.table_from({'civ':'CIVILIZATION_JAC_ODA','age':age})
            self.assertEqual([r.OathType for r in self.g.JAC_Oaths_Choices(h).values()],ODA)
        self.assertEqual(len(self.g.JAC_Oaths_Choices(self.lua.table_from({'civ':'CIVILIZATION_AMERICA','age':'GOLDEN'}))),0)

    def test_golden_single_choice_activates_only_tenka_in_all_owned_cities(self):
        self.judge()
        self.choose()
        self.assertEqual(self.selected(),{ODA[0]})
        self.assertEqual(self.house(0).oathChosen,1)
        for x,y in ((1,1),(2,1)):
            self.assertEqual([self.plot(x,y).GetProperty(None,k+'_ON') for k in ODA],[1,0,0])
        self.assertEqual(self.plot(5,5).GetProperty(None,ODA[0]+'_ON'),0)
        before=self.snapshot()
        self.choose(ODA[1])
        self.assertEqual(self.snapshot(),before)

    def test_heroic_requires_atomic_three_distinct_choices(self):
        self.judge('HEROIC')
        self.choose()
        self.choose([ODA[0],ODA[0],ODA[2]])
        self.assertEqual(self.selected(),set())
        self.choose(ODA)
        self.assertEqual(self.selected(),set(ODA))
        self.assertEqual([self.plot(1,1).GetProperty(None,k+'_ON') for k in ODA],[1,1,1])

    def test_foreign_unknown_stale_wrong_count_and_forged_requester_rejected(self):
        self.judge()
        before=self.snapshot()
        for choices in ('JAC_OATH_TAKEDA_RYUO','UNKNOWN',[],ODA,False):
            self.choose([choices] if choices is False else choices)
        self.choose(reign=0)
        self.choose(player=1,PlayerID=0)
        self.choose(player=62)
        self.g.CHOOSE_OATHS(0,None)
        self.g.CHOOSE_OATHS(0,self.lua.table_from({'Reign':1,1:ODA[0]}))
        self.assertEqual(self.snapshot(),before)

    def test_dark_and_normal_capture_quest_once_per_actual_capture(self):
        for age in ('NORMAL','DARK'):
            with self.subTest(age=age):
                self.setUp()
                self.judge(age)
                self.choose()
                self.assertEqual(self.plot(1,1).GetProperty(None,ODA[0]+'_ON'),0)
                self.g.CAPTURE(0,5,5) # home: base 6, quest 1
                self.assertEqual(self.house(0).score,7)
                self.assertEqual(self.house(0).parts.oath,1)
                self.turns(67,68)
                self.assertEqual(self.house(0).score,7) # conquest + later diff never counted twice
                self.g.CAPTURE(1,5,5)
                self.g.CAPTURE(0,5,5)
                self.assertEqual(self.house(0).parts.oath,2)

    def test_golden_capture_does_not_add_quest_score(self):
        self.judge()
        self.choose()
        self.g.CAPTURE(0,5,5)
        self.assertEqual(self.house(0).score,6)
        self.assertIsNone(self.house(0).parts.oath)

    def test_research_quests_only_for_matching_completions_after_choice(self):
        for oath,kind,other in ((ODA[2],'techs','civics'),(ODA[1],'civics','techs')):
            with self.subTest(oath=oath):
                self.setUp()
                self.judge('NORMAL')
                self.g.Players[0].research[kind][1]=True # before choice, no oath credit
                self.choose(oath)
                self.turns(67)
                self.assertIsNone(self.house(0).parts.oath)
                self.g.Players[0].research[kind][2]=True
                self.g.Players[0].research[other][1]=True
                self.turns(68)
                self.assertEqual(self.house(0).parts.oath,1)
                self.turns(69)
                self.assertEqual(self.house(0).parts.oath,1)

    def test_relation_quest_uses_base_ledger_dedup_and_friendship_one_not_two(self):
        self.g.PlayerConfigurations[2].GetCivilizationTypeName=lambda *args:'CIVILIZATION_JAC_ASHINA'
        self.g.HUMAN[2]=True
        self.judge()
        self.choose('JAC_OATH_ASHINA_AIZU_ALLIANCES',player=2)
        self.g.FRIENDS['2-3']=True
        self.turns(67)
        self.assertEqual((self.house(2).parts.ally,self.house(2).parts.oath),(1,1))
        self.g.ALLIED['2-3']=True
        self.turns(68,69)
        self.assertEqual((self.house(2).parts.ally,self.house(2).parts.oath),(3,2))

    def test_next_reign_expires_choice_and_flags_and_rejects_old_popup(self):
        self.judge()
        self.choose()
        self.turns(67,73)
        self.assertEqual(self.house(0).age,'NORMAL')
        self.assertEqual(self.selected(),set())
        self.assertEqual(self.plot(1,1).GetProperty(None,ODA[0]+'_ON'),0)
        self.choose(ODA[1],reign=1)
        self.assertEqual(self.selected(),set())
        self.choose(ODA[2],reign=2)
        self.assertEqual(self.selected(),{ODA[2]})

    def test_new_and_transferred_cities_reconcile_both_owners_flags(self):
        self.judge()
        self.choose()
        self.g.FOUND(0,3,1,False,False)
        self.assertEqual(self.plot(3,1).GetProperty(None,ODA[0]+'_ON'),1)
        self.g.TRANSFER(62,2,1)
        self.assertEqual(self.plot(2,1).GetProperty(None,ODA[0]+'_ON'),0)
        self.g.CAPTURE(0,5,5)
        self.assertEqual(self.plot(5,5).GetProperty(None,ODA[0]+'_ON'),1)
        self.assertEqual(self.plot(5,5).GetProperty(None,'JAC_OATH_TAKEDA_RYUO_ON'),0)

    def test_selection_flags_and_quest_counts_survive_script_reload_without_rescoring(self):
        self.judge('NORMAL')
        self.choose(ODA[2])
        self.g.Players[0].research.techs[1]=True
        self.turns(67)
        before=self.snapshot()
        self.g.RESET_HANDLERS()
        self.load_script()
        self.assertEqual(self.snapshot(),before)
        self.assertEqual(self.selected(),{ODA[2]})
        self.turns(68)
        self.assertEqual(self.house(0).parts.oath,1)

    def test_golden_flags_persist_before_first_turn_after_reload(self):
        self.judge()
        self.choose()
        self.g.RESET_HANDLERS()
        self.load_script()
        self.assertEqual(self.plot(1,1).GetProperty(None,ODA[0]+'_ON'),1)
        self.assertEqual(self.selected(),{ODA[0]})

    def test_unanswered_human_defaults_next_turn_and_locks_choice(self):
        self.judge()
        self.assertEqual(self.selected(),set())
        self.g.GOLD[0]=0
        self.turns(67)
        self.assertEqual(self.selected(),{ODA[1]})
        self.choose(ODA[0])
        self.assertEqual(self.selected(),{ODA[1]})

    def test_ai_favours_war_or_income_and_is_deterministic(self):
        for war,gold,expected in ((True,100,ODA[0]),(False,0,ODA[1])):
            self.setUp()
            self.g.HUMAN[0]=False
            self.g.WAR['0-1']=war
            self.g.GOLD[0]=gold
            self.judge()
            self.assertEqual(self.selected(),{expected})
            self.assertEqual(self.house(0).oathChosen,1)

    def test_ai_naval_oath_requires_a_coastal_city(self):
        for coastal,expected in ((True,'JAC_OATH_MORI_SETO_FLEETS'),(False,'JAC_OATH_MORI_THREE_SONS')):
            self.setUp()
            self.g.HUMAN[0]=False
            self.g.PlayerConfigurations[0].GetCivilizationTypeName=lambda *args:'CIVILIZATION_JAC_MORI'
            self.g.COASTAL[101]=coastal
            self.judge()
            self.assertEqual(self.selected(),{expected})

    def test_ai_heroic_gets_all_three_without_randomness(self):
        self.g.HUMAN[0]=False
        self.judge('HEROIC')
        self.assertEqual(self.selected(),set(ODA))

    def test_property_writes_use_fixed_order_for_all_66_flags(self):
        self.judge()
        expected=sorted('JAC_OATH_'+h['id']+'_'+o['id']+'_ON' for h in CATALOGUE for o in h['oaths'])
        self.lua.execute('WRITTEN={}; RECORD_PLOT={SetProperty=function(_,key,value) WRITTEN[#WRITTEN+1]=key end}')
        self.g.JAC_Oaths_WritePlot(self.house(0),self.g.RECORD_PLOT)
        self.assertEqual(list(self.g.WRITTEN.values()),expected)


class OathPopupTests(unittest.TestCase):
    load_script, turns, house, snapshot, plot, judge, selected = (ages.LedgerTests.load_script,ages.LedgerTests.turns,ages.LedgerTests.house,ages.LedgerTests.snapshot,ages.LedgerTests.plot,OathLedgerTests.judge,OathLedgerTests.selected)

    def setUp(self):
        ages.LedgerTests.setUp(self)
        self.g.HUMAN[0]=True
        self.lua.execute(UI_STUBS)
        self.g.LOCAL_PLAYER=0
        self.lua.execute(SCREEN_STUBS,self.lua.table_from(_files()))
        self.ui=self.g.NEW_CONTEXT('DedicationPopup',(MOD/'UI/JAC_DedicationPopup_Sengoku.lua').read_text(encoding='utf-8'))

    def test_actual_rendered_rows_are_exactly_odas_three_in_each_age(self):
        for age in ('NORMAL','DARK','GOLDEN','HEROIC'):
            self.setUp()
            self.judge(age)
            self.assertTrue(self.ui.JAC_ShowOaths(True))
            rows=list(self.ui.JAC_GetOfferedOaths().values())
            self.assertEqual([r.type for r in rows],ODA)
            self.assertEqual([r.name for r in rows],['LOC_'+k+'_NAME' for k in ODA])
            suffix='_GOLDEN' if age in ('GOLDEN','HEROIC') else '_QUEST'
            self.assertEqual([r.description for r in rows],['LOC_'+k+suffix for k in ODA])
            self.assertEqual(self.ui.JAC_OATH_POPUP_LAST.allowed,3 if age=='HEROIC' else 1)

    def test_click_and_confirm_uses_engine_operation_and_activates_modifier(self):
        self.judge()
        self.ui.JAC_ShowOaths(True)
        self.assertTrue(self.ui.Controls.Confirm.disabled)
        self.ui.JAC_SelectOath(ODA[0])
        self.assertFalse(self.ui.Controls.Confirm.disabled)
        self.ui.Controls.Confirm.click()
        self.assertEqual(self.selected(),{ODA[0]})
        self.assertEqual(self.g.REQUESTS[1].OnStart,'JAC_SengokuOathsChoose')
        self.assertEqual(self.g.REQUESTS[1].id,0)
        self.assertTrue(self.ui.ContextPtr.hidden)
        self.assertEqual(self.plot(1,1).GetProperty(None,ODA[0]+'_ON'),1)

    def test_heroic_confirms_three_in_one_operation(self):
        self.judge('HEROIC')
        self.ui.JAC_ShowOaths(True)
        self.assertFalse(self.ui.Controls.Confirm.disabled)
        self.ui.Controls.Confirm.click()
        self.assertEqual(len(self.g.REQUESTS),1)
        self.assertEqual(self.selected(),set(ODA))

    def test_inactive_turn_and_foreign_row_do_not_send(self):
        self.judge()
        self.ui.JAC_ShowOaths(True)
        self.g.TURN_ACTIVE=False
        self.assertFalse(self.ui.JAC_ChooseOath(ODA[0]))
        self.assertFalse(self.ui.JAC_SelectOath('JAC_OATH_TAKEDA_RYUO'))
        self.assertEqual(len(self.g.REQUESTS),0)

    def test_reign_popup_defers_choice_until_closed(self):
        self.judge()
        self.g.LuaEvents.JAC_SengokuReignPopup_Shown()
        self.assertFalse(self.ui.JAC_ShowOaths(True))
        self.g.LuaEvents.JAC_SengokuReignPopup_Closed(True)
        self.assertFalse(self.ui.ContextPtr.hidden)

    def test_pending_choice_can_be_reopened_after_load(self):
        self.judge()
        source=(MOD/'UI/JAC_DedicationPopup_Sengoku.lua').read_text(encoding='utf-8')
        self.ui=self.g.NEW_CONTEXT('DedicationPopup',source)
        self.assertTrue(self.ui.JAC_ShowOaths(True))
        self.assertEqual([r.type for r in self.ui.JAC_GetOfferedOaths().values()],ODA)

    def test_pending_load_can_show_before_activation_but_confirms_only_when_active(self):
        self.judge()
        self.g.TURN_ACTIVE=False
        self.g.LOAD_SCREEN_CLOSE()
        self.assertFalse(self.ui.ContextPtr.hidden)
        self.assertFalse(self.ui.JAC_ChooseOath(ODA[0]))
        self.assertFalse(self.ui.ContextPtr.hidden)
        self.assertEqual(len(self.g.REQUESTS),0)
        self.g.TURN_ACTIVE=True
        self.g.LOCAL_TURN_BEGIN()
        self.assertFalse(self.ui.Controls.Confirm.disabled)
        self.ui.Controls.Confirm.click()
        self.assertEqual(self.selected(),{ODA[0]})

    def test_reopening_same_view_keeps_selection_but_player_change_discards_it(self):
        self.judge()
        self.ui.JAC_ShowOaths(True)
        self.ui.JAC_SelectOath(ODA[1])
        self.ui.JAC_ShowOaths(True)
        self.assertFalse(self.ui.Controls.Confirm.disabled)
        self.g.LOCAL_PLAYER=1
        self.ui.JAC_ShowOaths(True)
        self.assertTrue(self.ui.Controls.Confirm.disabled)
        self.assertEqual([r.type for r in self.ui.JAC_GetOfferedOaths().values()],
                         ['JAC_OATH_TAKEDA_TSUTSUJIGASAKI','JAC_OATH_TAKEDA_KOSHU_LAW','JAC_OATH_TAKEDA_RYUO'])

    def test_no_shared_oaths_for_unregistered_civilization(self):
        self.judge()
        self.house(0).civ='CIVILIZATION_AMERICA'
        self.assertFalse(self.ui.JAC_ShowOaths(True))

    def test_turn_end_closes_and_late_popup_cannot_rechoose(self):
        self.judge()
        self.ui.JAC_ShowOaths(True)
        self.g.LOCAL_TURN_END()
        self.assertTrue(self.ui.ContextPtr.hidden)
        self.turns(67)
        self.assertFalse(self.ui.JAC_ShowOaths(True))
        self.assertFalse(self.ui.JAC_ChooseOath(ODA[0]))

    def test_delayed_engine_reply_opens_dark_policy_only_after_oath_is_applied(self):
        self.judge('DARK')
        dark=self.g.NEW_CONTEXT('JAC_SengokuDarkPolicy',(MOD/'UI/JAC_SengokuDarkPolicy.lua').read_text(encoding='utf-8'))
        self.lua.execute('UI.RequestPlayerOperation=function(id,op,params) DELAYED={id=id,params=params} end; function APPLY_DELAYED() CHOOSE_OATHS(DELAYED.id,DELAYED.params); GAME_CORE_PUBLISH() end')
        self.ui.JAC_ShowOaths(True)
        self.assertTrue(self.ui.JAC_ChooseOath(ODA[0]))
        self.assertTrue(self.ui.ContextPtr.hidden)
        self.assertTrue(dark.ContextPtr.hidden)
        self.assertIsNone(self.house(0).oathChosen)
        self.g.APPLY_DELAYED()
        self.assertEqual(self.house(0).oathChosen,1)
        self.assertFalse(dark.ContextPtr.hidden)

    def test_notification_and_all_end_turn_paths_require_oath_before_dark_policy(self):
        read = lambda name: (MOD / f'UI/{name}.lua').read_text(encoding='utf-8')
        dark = self.g.NEW_CONTEXT('JAC_SengokuDarkPolicy', read('JAC_SengokuDarkPolicy'))
        reign = self.g.NEW_CONTEXT('JAC_SengokuReignPopup', read('JAC_SengokuReignPopup'))
        self.lua.execute(SCREEN_STUBS, self.lua.table_from(dict(_files(), ActionPanel_Expansion2=ACTION_PANEL_STUBS)))
        action = self.g.NEW_CONTEXT('ActionPanel', read('JAC_ActionPanel_Sengoku'))
        panel = self.g.NEW_CONTEXT('NotificationPanel', NOTIFICATION_PANEL_STUBS, read('JAC_NotificationPanel_Sengoku'))
        panel.RegisterHandlers()
        self.judge('DARK')
        self.g.LOCAL_TURN_BEGIN()
        self.assertFalse(reign.ContextPtr.hidden)
        note = self.g.NotificationManager.FindType(3, 0)
        panel.OnNotificationActivated(0, note.id, True)
        self.assertTrue(reign.ContextPtr.hidden)
        self.assertFalse(self.ui.ContextPtr.hidden)
        self.assertTrue(dark.ContextPtr.hidden)
        self.assertFalse(dark.JAC_ChooseDarkPolicy('JAC_AGE_POLICY_TEMPLE'))
        self.assertEqual(len(self.g.REQUESTS), 0)
        for invoke in (action.DoEndTurn, action.OnEndTurnClicked,
                       lambda: action.OnInputActionTriggered('END_TURN'),
                       lambda: action.ContextPtr.input(action.KEY_INPUT(13, False)),
                       lambda: action.ContextPtr.input(action.KEY_INPUT(13, True))):
            self.ui.OnClose()
            invoke()
            self.assertFalse(self.ui.ContextPtr.hidden)
            self.assertTrue(dark.ContextPtr.hidden)
        action.CheckAutoEndTurn()
        self.assertEqual((action.END_TURN_REQUESTS, action.AUTO_END_TURN_REQUESTS, action.FORCED_END_TURN_REQUESTS), (0, 0, 0))
        self.lua.execute('UI.RequestPlayerOperation=function(id,op,params) DELAYED={id=id,params=params} end')
        self.assertTrue(self.ui.JAC_ChooseOath(ODA[0]))
        self.g.GAME_CORE_PUBLISH()
        self.assertTrue(dark.ContextPtr.hidden)
        self.g.CHOOSE_OATHS(0, self.g.DELAYED.params)
        self.g.GAME_CORE_PUBLISH()
        self.assertFalse(dark.ContextPtr.hidden)
        self.assertTrue(action.JAC_IsDarkPolicyEndTurnBlocked())
        self.g.CHOOSE(0, self.lua.table_from({'Reign': 1, 'PolicyType': 'JAC_AGE_POLICY_TEMPLE'}))
        self.g.GAME_CORE_PUBLISH()
        self.assertFalse(action.JAC_IsDarkPolicyEndTurnBlocked())
        action.DoEndTurn()
        self.assertEqual(action.END_TURN_REQUESTS, 1)

    def test_normal_and_golden_oaths_block_until_chosen_even_without_policy_notification(self):
        for age in ('NORMAL', 'GOLDEN'):
            self.setUp()
            self.lua.execute(SCREEN_STUBS, self.lua.table_from(dict(_files(), ActionPanel_Expansion2=ACTION_PANEL_STUBS)))
            action = self.g.NEW_CONTEXT('ActionPanel', (MOD / 'UI/JAC_ActionPanel_Sengoku.lua').read_text(encoding='utf-8'))
            self.judge(age)
            action.DoEndTurn()
            self.assertFalse(self.ui.ContextPtr.hidden)
            self.assertEqual(action.END_TURN_REQUESTS, 0)
            self.assertTrue(self.ui.JAC_ChooseOath(ODA[1]))
            self.assertFalse(action.JAC_IsDarkPolicyEndTurnBlocked())
            action.DoEndTurn()
            self.assertEqual(action.END_TURN_REQUESTS, 1)


if __name__=='__main__':
    unittest.main()
