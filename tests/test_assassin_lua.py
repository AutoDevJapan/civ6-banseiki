"""Execute the real Assassin gameplay and shared rules with mandatory lupa.

These stubs model persistence and engine calls, not native diplomacy, unit art or MP.
The live scenario is prepared to test the real operation path and two save/load cycles.
"""
from __future__ import annotations

import json
from pathlib import Path
import re
import sqlite3
import unittest
import xml.etree.ElementTree as ET

from lupa import LuaRuntime

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_Assassin"
RULES = (MOD / "Scripts/JAC_Assassin_Rules.lua").read_text(encoding="utf-8")
GAMEPLAY = (MOD / "Scripts/JAC_Assassin_Gameplay.lua").read_text(encoding="utf-8")

STUBS = r"""
unpack=table.unpack or unpack
function clone(v)
  if type(v)~='table' then return v end
  local t={}; for k,x in pairs(v) do t[k]=clone(x) end; return t
end
NOW, SPEED, RAND_CALLS, SPAWN_BLOCKED = 1000, 'GAMESPEED_STANDARD', 0, false
GAME_PROPS, DATA, NOTES, KILLS, EVENTS = {}, {}, {}, {}, {}
Game={GetCurrentGameTurn=function() return NOW end, GetLocalPlayer=function() error('gameplay must not read local player') end,
  GetRandNum=function(max,label) RAND_CALLS=RAND_CALLS+1; return ROLL or 0 end,
  GetProperty=function(_,k) return clone(GAME_PROPS[k]) end,
  SetProperty=function(_,k,v) GAME_PROPS[k]=clone(v) end}
GameConfiguration={GetGameSpeedType=function() return SPEED end}
local unitRows={
 {UnitType='UNIT_JAC_JAPAN_SENGOKU_ASSASSIN',Index=0,Combat=10,BuildCharges=0},
 {UnitType='UNIT_BUILDER',Index=1,Combat=0,BuildCharges=3},
 {UnitType='UNIT_GREAT_GENERAL',Index=2,Combat=0,BuildCharges=0},
 {UnitType='UNIT_HERO_HERCULES',Index=3,Combat=30,BuildCharges=0},
 {UnitType='UNIT_WARRIOR',Index=4,Combat=20,BuildCharges=0},
 {UnitType='UNIT_JAC_ASSASSIN',Index=5,Combat=10,BuildCharges=0}}
GameInfo={GameSpeeds={},Units={}}
for _,speed in ipairs({'ONLINE','QUICK','STANDARD','EPIC','MARATHON'}) do
  local key='GAMESPEED_'..speed; GameInfo.GameSpeeds[key]={GameSpeedType=key}
end
for _,row in ipairs(unitRows) do GameInfo.Units[row.Index]=row; GameInfo.Units[row.UnitType]=row end
Map={GetPlotDistance=function(ax,ay,bx,by) return math.max(math.abs(ax-bx),math.abs(ay-by)) end}
PlayersVisibility={}
for id=0,2 do PlayersVisibility[id]={IsRevealed=function() return not HIDDEN_CITY end,
  IsVisible=function() return not HIDDEN_UNIT end} end
Players, PlayerConfigurations = {}, {}
function props(p,data)
  p.GetProperty=function(_,k) return clone(data.Properties[k]) end
  p.SetProperty=function(_,k,v) data.Properties[k]=clone(v) end
end
function memberCollection(items,make)
  return {FindID=function(_,id) if items[id] then return make(items[id]) end end,
    GetCount=function() local n=0; for _ in pairs(items) do n=n+1 end; return n end,
    Members=function()
      local keys={}; for k in pairs(items) do keys[#keys+1]=k end; table.sort(keys)
      local i=0; return function() i=i+1; local k=keys[i]; if k then return k,make(items[k]) end end
    end}
end
function makeUnit(d)
  local u={GetID=function() return d.ID end, GetType=function() return d.Type end,
    GetOwner=function() return d.Owner end, GetX=function() return d.X end,GetY=function() return d.Y end}
  props(u,d); return u
end
function makeCity(d)
  local c={GetID=function() return d.ID end,GetOwner=function() return d.Owner end,
    GetX=function() return d.X end,GetY=function() return d.Y end}
  props(c,d); return c
end
function rebuild()
  for id=0,2 do
    local d=DATA[id]
    local cities=memberCollection(d.Cities,makeCity)
    cities.GetCapitalCity=function() return cities:FindID(d.Capital) end
    local p={GetID=function() return id end,IsAlive=function() return d.Alive end,
      GetCities=function() return cities end,GetUnits=function() return memberCollection(d.Units,makeUnit) end,
      GetDiplomacy=function() return {IsAtWarWith=function(_,other) return d.Wars[other]==true end} end}
    props(p,d); Players[id]=p
    PlayerConfigurations[id]={GetLeaderTypeName=function() return 'LEADER_'..id end,
      GetCivilizationTypeName=function() return ({[0]='CIVILIZATION_JAC_ODA',[1]='CIVILIZATION_JAC_MATSUDAIRA',[2]='CIVILIZATION_JAC_AINU'})[id] end}
  end
end
for id=0,2 do
  DATA[id]={Properties={},Units={},Cities={},Capital=10,Alive=true,Wars={}}
  for _,cid in ipairs({10,11}) do DATA[id].Cities[cid]={ID=cid,Owner=id,X=id*2,Y=cid-10,Properties={}} end
end
rebuild()
PlayerManager={GetAliveMajors=function()
  local t={}; for id=2,0,-1 do if DATA[id].Alive then t[#t+1]=Players[id] end end; return t
end}
NEXT_UNIT=1
function GRANT(owner,unitType,x,y)
  local id=NEXT_UNIT; NEXT_UNIT=NEXT_UNIT+1
  local d={ID=id,Type=unitType,Owner=owner,X=x,Y=y,Properties={}}
  DATA[owner].Units[id]=d; return makeUnit(d)
end
UnitManager={InitUnitValidAdjacentHex=function(id,kind,x,y)
  if SPAWN_BLOCKED then return nil end
  GRANT(id,GameInfo.Units[kind].Index,x,y)  -- like the engine: creates the unit, returns no Unit object (live assassin-01)
end, Kill=function(u)
  KILLS[#KILLS+1]={Owner=u:GetOwner(),ID=u:GetID()}; DATA[u:GetOwner()].Units[u:GetID()]=nil
end}
GameEvents=setmetatable({}, {__index=function(t,k)
  local event={Add=function(f) EVENTS[k]=f end}; rawset(t,k,event); return event
end})
ExposedMembers={}
NotificationTypes={USER_DEFINED_1=1}; ParameterTypes={MESSAGE='MESSAGE',SUMMARY='SUMMARY'}
Locale={Lookup=function(k,...) return k end}
NotificationManager={SendNotification=function(id,kind,t) NOTES[#NOTES+1]={id,kind,t} end}
function print() end
function ADVANCE(turns)
  for _=1,turns do NOW=NOW+1; EVENTS.OnGameTurnStarted() end
end
function FORCE(success,detected,branch)
  Game:SetProperty('JAC_ASN_TEST_MODE',1)
  Players[0]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=success and 1 or 0,Detected=detected and 1 or 0,Branch=branch})
end
"""


class AssassinLuaTests(unittest.TestCase):
    def setUp(self):
        self.lua=LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(STUBS)
        self.lua.execute(RULES)
        self.lua.globals().include=lambda _: None
        self.lua.execute(GAMEPLAY)
        self.run_lua("A=JAC_ASN; U=GRANT(0,0,1,0):GetID(); U2=GRANT(0,0,1,0):GetID()")

    def run_lua(self, source):
        return self.lua.execute(source)

    def test_base_100_and_minimum_20_for_all_speeds(self):
        self.run_lua("""
          for _,v in ipairs({{'ONLINE',50,10,5},{'QUICK',67,14,7},{'STANDARD',100,20,10},
                             {'EPIC',150,30,15},{'MARATHON',300,60,30}}) do
            SPEED='GAMESPEED_'..v[1]
            assert(A.ProductionTurns({})==v[2]); assert(A.ProductionTurns({Building=1000})==v[3])
            assert(A.Scale(A.PRODUCTION_LOCK)==v[4])
          end
        """)

    def test_every_factor_and_security_direction(self):
        self.run_lua("""
          for _,k in ipairs({'Building','District','Policy','Government','Trait','Job'}) do
            assert(A.ProductionTurns({[k]=1})<100)
          end
          assert(A.ProductionTurns({Security=100})==120)
          assert(A.ProductionTurns({Security=0})==80)
          for security=0,100 do
            assert(A.ProductionTurns({Security=security,Building=10000})==20)
          end
        """)

    def test_multiple_cities_reject_parallel_production(self):
        self.assertTrue(self.run_lua("return A.StartProduction(0,10)"))
        self.assertEqual(self.run_lua("return A.StartProduction(0,11)"),(False,"PRODUCING"))
        self.assertTrue(self.run_lua("return A.StartProduction(1,10)"))

    def test_actual_completion_and_exact_lock_boundary(self):
        self.run_lua("""
          assert(A.StartProduction(0,10)); ADVANCE(99)
          local s=A.State(0); assert(s.Production and s.LastCompletion==nil)
          ADVANCE(1); s=A.State(0)
          assert(s.Production==nil and s.LastCompletion==1100 and s.ProductionUnlock==1110)
          assert(Players[0]:GetUnits():FindID(s.LastProducedUnit))
          ADVANCE(9)
          for _,id in ipairs({10,11}) do local ok,r=A.StartProduction(0,id); assert(not ok and r=='PRODUCTION_LOCK') end
          ADVANCE(1); assert(A.StartProduction(0,11))
        """)

    def test_minimum_is_elapsed_time_even_with_extreme_bonuses(self):
        self.run_lua("""
          Players[0]:GetCities():FindID(10):SetProperty(A.INPUT,{Building=10000})
          assert(A.StartProduction(0,10)); ADVANCE(19); assert(A.State(0).LastCompletion==nil)
          ADVANCE(1); assert(A.State(0).LastCompletion==1020)
        """)

    def test_cancellation_cannot_transfer_elapsed_progress(self):
        self.run_lua("""
          assert(A.StartProduction(0,10)); ADVANCE(99); A.CancelProduction(0)
          assert(A.StartProduction(0,11)); ADVANCE(1)
          assert(A.State(0).LastCompletion==nil and A.State(0).Production.Due==1199)
        """)

    def test_cancel_does_not_clear_existing_completion_lock(self):
        self.run_lua("""
          assert(A.StartProduction(0,10)); ADVANCE(100); A.CancelProduction(0)
          local ok,r=A.StartProduction(0,11); assert(not ok and r=='PRODUCTION_LOCK')
          assert(A.State(0).ProductionUnlock==1110)
        """)

    def test_lost_city_releases_only_the_production_reservation(self):
        self.run_lua("""
          assert(A.StartProduction(0,10)); DATA[0].Cities[10]=nil; ADVANCE(1)
          assert(A.State(0).Production==nil and A.State(0).LastCompletion==nil)
          assert(A.StartProduction(0,11))
        """)

    def test_blocked_spawn_lock_begins_at_actual_completion(self):
        self.run_lua("""
          assert(A.StartProduction(0,10)); SPAWN_BLOCKED=true; ADVANCE(105)
          assert(A.State(0).Production and A.State(0).ProductionUnlock==nil)
          SPAWN_BLOCKED=false; ADVANCE(1)
          assert(A.State(0).LastCompletion==1106 and A.State(0).ProductionUnlock==1116)
        """)

    def test_production_and_command_locks_are_independent(self):
        self.run_lua("""
          assert(A.StartProduction(0,10)); ADVANCE(100)
          U=GRANT(0,0,1,0):GetID()
          assert(A.StartAttempt(0,U,1,'LEADER')); assert(A.State(0).Mission)
          ADVANCE(10); assert(A.StartProduction(0,11))
          local ok,r=A.CanCommand(A.State(0),NOW); assert(not ok and r=='COMMAND_LOCK')
        """)

    def test_only_full_stack_reaches_wait_one(self):
        self.run_lua("""
          assert(A.WaitTurns({})==50)
          local f={Security=0,Relation=1,School=1,Governor=1,Hero=1,GreatPerson=1}
          assert(A.WaitTurns(f)==1)
          for _,k in ipairs({'Security','Relation','School','Governor','Hero','GreatPerson'}) do
            local g=clone(f); g[k]=(k=='Security') and 50 or 0; assert(A.WaitTurns(g)>1)
          end
          assert(A.WaitTurns(f,3)==3); assert(A.WaitTurns({},0.67)==34)
        """)

    def test_distances_target_periods_and_rounding(self):
        self.run_lua("""
          for _,x in ipairs({{'LEADER',5,8,10},{'GOVERNOR',3,5,6},{'GREAT_PERSON',2,3,4}}) do
            assert(A.MissionTurns(x[1],1)==x[2]); assert(A.MissionTurns(x[1],2)==x[3])
            assert(A.MissionTurns(x[1],4)==x[4])
          end
          assert(A.MissionTurns('GOVERNOR',2,0.67)==4)
          DATA[0].Units[U].X=99
          local ok,r=A.StartAttempt(0,U,1,'LEADER'); assert(not ok and r=='TOO_FAR')
          DATA[0].Units[U].X=1; assert(A.StartAttempt(0,U,1,'LEADER'))
        """)

    def test_all_six_success_rates_and_corrections(self):
        self.run_lua("""
          for _,r in ipairs({{'LEADER',1},{'GOVERNOR',15},{'GREAT_PERSON',30},{'HERO',50},{'UNIT',50},{'BUILDER',90}}) do
            assert(A.SuccessChance(r[1],{})==r[2])
            assert(A.SuccessChance(r[1],{Security=100,Guard=1})<r[2])
          end
        """)

    def test_builder_hero_gp_and_assassin_are_classified_by_real_unit(self):
        self.run_lua("""
          assert(A.Classify(GRANT(1,0,2,0))=='UNIT')
          assert(A.Classify(GRANT(1,1,2,0))=='BUILDER')
          assert(A.Classify(GRANT(1,2,2,0))=='GREAT_PERSON')
          assert(A.Classify(GRANT(1,3,2,0))=='HERO')
          assert(A.Classify(GRANT(1,4,2,0))=='UNIT')
          local u=GRANT(1,1,2,0); local ok,r=A.StartAttempt(0,U,1,'UNIT',u:GetID())
          assert(not ok and r=='TARGET_KIND') -- no claiming a builder as a generic unit
        """)

    def test_names_are_assigned_by_confirmed_country_not_whole_map(self):
        self.run_lua("""
          assert(A.UnitForPlayer(0)==A.UNIT)
          assert(A.UnitForPlayer(2)==A.COMMON_UNIT)
          PlayerConfigurations[2].GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_RYUKYU' end
          assert(A.UnitForPlayer(2)==A.COMMON_UNIT)
          PlayerConfigurations[2].GetCivilizationTypeName=function() return 'CIVILIZATION_MONGOLIA' end
          assert(A.UnitForPlayer(2)==A.COMMON_UNIT)
          assert(A.IsAssassin(GRANT(2,5,0,0)))
          assert(A.StartProduction(2,10)); ADVANCE(100)
          assert(DATA[2].Units[A.State(2).LastProducedUnit].Type==5)
        """)

    def test_one_active_mission_even_after_wait_expires(self):
        self.run_lua("""
          Players[0]:SetProperty(A.INPUT,{Security=0,Relations={['1']=1},School=1,Governor=1,Hero=1,GreatPerson=1})
          assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(1)
          local ok,r=A.StartAttempt(0,U2,2,'LEADER'); assert(not ok and r=='MISSION_ACTIVE')
          assert(A.State(0).LastCommand==1000)
        """)

    def test_command_wait_cannot_be_bypassed_by_unit_or_country(self):
        self.run_lua("""
          FORCE(false,false); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(49)
          local ok,r=A.StartAttempt(0,U2,2,'LEADER'); assert(not ok and r=='COMMAND_LOCK')
          ADVANCE(1); assert(A.StartAttempt(0,U2,2,'LEADER'))
        """)

    def test_outcome_roll_is_at_start_and_not_at_resolution(self):
        self.run_lua("""
          ROLL=99; assert(A.StartAttempt(0,U,1,'LEADER'))
          assert(A.State(0).Mission.Success); assert(RAND_CALLS==3)
          ROLL=9999; ADVANCE(5); assert(RAND_CALLS==3)
          assert(A.State(0).LastMission.Result=='LEADER_DEAD_PENDING')
        """)

    def test_percent_threshold_is_not_inclusive(self):
        self.run_lua("""
          ROLL=100; assert(A.StartAttempt(0,U,1,'LEADER')); assert(not A.State(0).Mission.Success)
        """)

    def test_forced_outcome_only_works_with_explicit_test_flag(self):
        self.run_lua("""
          Players[0]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=1,Detected=0}); ROLL=9999
          assert(A.StartAttempt(0,U,1,'LEADER')); assert(not A.State(0).Mission.Success)
        """)

    def test_ui_operation_cannot_supply_forced_outcome(self):
        self.run_lua("""
          ROLL=9999; EVENTS.JAC_AssassinOperation(0,{Action='ATTEMPT',Unit=U,Target=1,Kind='LEADER',Success=1})
          assert(not A.State(0).Mission.Success)
        """)

    def test_leader_death_does_not_eliminate_player_or_transfer_territory(self):
        self.run_lua("""
          FORCE(true,false,'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)
          local n=Players[1]:GetProperty(A.NATION)
          assert(n.LeaderDead==1 and n.State=='PENDING_FREE_STATE' and n.DeathTurn==1005)
          assert(n.DecisionRequired==1 and DATA[1].Alive)
          assert(DATA[0].Cities[10].Owner==0 and DATA[1].Cities[10].Owner==1)
          assert(A.State(0).CommandUnlock==1050)
        """)

    def test_post_death_snapshot_and_branch_are_frozen(self):
        self.run_lua("""
          FORCE(true,false,'SUCCESSION'); Players[1]:SetProperty(A.INPUT,{Security=10,Successor='PERSON_1'})
          assert(A.StartAttempt(0,U,1,'LEADER'))
          Players[1]:SetProperty(A.INPUT,{Security=90,Successor='PERSON_2'})
          ADVANCE(5); local n=Players[1]:GetProperty(A.NATION)
          assert(n.Branch=='SUCCESSION' and n.Snapshot.Security==10 and n.Snapshot.Successor=='PERSON_1')
        """)

    def test_no_congress_branch_on_japan_map(self):
        self.run_lua("""
          for roll=0,100 do assert(A.Branch({Neighbor=true,Successor='P',Congress=false},roll)~='CONGRESS') end
          FORCE(true,false,'CONGRESS')
        """)
        with self.assertRaisesRegex(Exception,"Congress absent"):
            self.run_lua("A.StartAttempt(0,U,1,'LEADER')")

    def test_unit_success_and_failure_dispositions(self):
        self.run_lua("""
          local target=GRANT(1,0,2,0):GetID(); FORCE(true,false)
          assert(A.StartAttempt(0,U,1,'UNIT',target)); ADVANCE(2)
          assert(DATA[1].Units[target]==nil and DATA[0].Units[U]~=nil)
          ADVANCE(48); FORCE(false,false); assert(A.StartAttempt(0,U,2,'LEADER')); ADVANCE(8)
          assert(DATA[0].Units[U]==nil)
        """)

    def test_failure_and_detection_independently_share_penalty_once(self):
        for success,detected in ((False,False),(False,True),(True,True),(True,False)):
            with self.subTest(success=success,detected=detected):
                self.setUp()
                self.run_lua(f"FORCE({str(success).lower()},{str(detected).lower()},'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)")
                self.run_lua("""
                  local m=A.State(0).LastMission; local p=Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1')
                  if not m.Success or m.Detected then
                    assert(p.Grievances['0']==25 and p.Friendship['0']==-10)
                    assert((p.WarReason['0']~=nil)==m.Detected)
                    assert((Players[2]:GetProperty('JAC_ASN_DENOUNCE_0')~=nil)==m.Detected)
                  else assert(p==nil) end
                  assert((DATA[0].Units[U]==nil)==(not m.Success))
                  A.Tick(); ADVANCE(1)
                  if p then assert(Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances['0']==25) end
                """)

    def test_missing_target_does_not_redirect_or_reroll(self):
        self.run_lua("""
          local target=GRANT(1,4,2,0):GetID(); FORCE(true,false)
          assert(A.StartAttempt(0,U,1,'UNIT',target)); DATA[1].Units[target]=nil
          ADVANCE(2); assert(A.State(0).LastMission.Result=='TARGET_GONE')
          assert(A.State(0).CommandUnlock==1050 and RAND_CALLS==2)
        """)

    def test_fixed_failure_detection_penalties_survive_target_disappearance(self):
        self.run_lua("""
          local target=GRANT(1,4,2,0):GetID(); FORCE(false,true)
          assert(A.StartAttempt(0,U,1,'UNIT',target)); DATA[1].Units[target]=nil
          ADVANCE(2)
          local p=Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1')
          assert(p.Grievances['0']==25 and p.Friendship['0']==-10 and p.WarReason['0']==1002)
          assert(DATA[0].Units[U]==nil and A.State(0).LastMission.Result=='TARGET_GONE')
        """)

    def test_actor_disappearance_prevents_ghost_success(self):
        self.run_lua("""
          local target=GRANT(1,4,2,0):GetID(); FORCE(true,true)
          assert(A.StartAttempt(0,U,1,'UNIT',target)); DATA[0].Units[U]=nil
          ADVANCE(2)
          assert(DATA[1].Units[target] and A.State(0).LastMission.Result=='ACTOR_GONE')
          assert(Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances['0']==25)
          assert(A.State(0).CommandUnlock==1050)
        """)

    def test_eliminated_country_finishes_saved_jobs_and_fixed_penalties(self):
        self.run_lua("""
          assert(A.StartProduction(0,10)); FORCE(false,true)
          assert(A.StartAttempt(0,U,1,'LEADER'))
          DATA[0].Alive=false; DATA[0].Units={}; DATA[0].Cities={}
          ADVANCE(5)
          local s=A.State(0); assert(s.Mission==nil and s.Production==nil)
          assert(s.LastMission.Result=='ACTOR_GONE' and s.CommandUnlock==1050)
          assert(Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances['0']==25)
          ADVANCE(1); assert(Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances['0']==25)
        """)

    def test_replacement_unit_with_same_id_and_type_is_not_the_target(self):
        self.run_lua("""
          local target=GRANT(1,4,2,0):GetID(); FORCE(true,false)
          assert(A.StartAttempt(0,U,1,'UNIT',target))
          DATA[1].Units[target]={ID=target,Owner=1,Type=4,X=2,Y=0,Properties={}}
          ADVANCE(2); assert(DATA[1].Units[target] and A.State(0).LastMission.Result=='TARGET_GONE')
        """)

    def test_relation_penalty_increases_hostility_and_shortens_wait(self):
        self.run_lua("""
          local before=A.WaitTurns(A.CommandInput(0,1))
          FORCE(false,false); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)
          assert(A.Relation(0,1)>0); assert(A.WaitTurns(A.CommandInput(0,1))<before)
          assert(A.State(0).CommandUnlock==1050) -- accepted command clock remains frozen
        """)

    def test_notification_exception_cannot_duplicate_production_or_penalties(self):
        self.run_lua("""
          NotificationManager.SendNotification=function() error('injected notification failure') end
          assert(A.StartProduction(0,10)); FORCE(false,true)
          assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(101)
          assert(Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances['0']==25)
          assert(A.State(0).LastCompletion==1100 and A.State(0).Production==nil)
          local n=Players[0]:GetUnits():GetCount(); ADVANCE(2); assert(Players[0]:GetUnits():GetCount()==n)
        """)

    def test_visibility_checks_do_not_disclose_or_accept_hidden_targets(self):
        self.run_lua("""
          HIDDEN_CITY=true; local ok,r=A.StartAttempt(0,U,1,'LEADER'); assert(not ok and r=='TARGET_UNKNOWN')
          HIDDEN_CITY=false; HIDDEN_UNIT=true; local target=GRANT(1,4,2,0):GetID()
          local ok,r=A.StartAttempt(0,U,1,'UNIT',target); assert(not ok and r=='TARGET_UNKNOWN')
          assert(RAND_CALLS==0)
        """)

    def test_branch_weights_use_security_unrest_strength_relations_and_candidates(self):
        self.run_lua("""
          local a=A.BranchWeights({Security=100,Neighbor=true,Successor='P',Cities=2,
            Neighbors={{Cities=2,Relation=0}}})
          local b=A.BranchWeights({Security=0,Neighbor=true,Successor='P',Cities=2,
            Regions={Unrest=50},Neighbors={{Cities=4,Relation=1}}})
          assert(b[1][2]>a[1][2] and b[2][2]>a[2][2] and b[4][2]<a[4][2])
          local c=A.BranchWeights({Congress=false}); assert(c[2][2]==0 and c[3][2]==0 and c[4][2]==0)
        """)

    def test_registered_governor_uses_stable_identity(self):
        self.run_lua("""
          Players[1]:SetProperty(A.INPUT,{Governors={['7']={City=10,Identity='GOV_TEST'}}})
          FORCE(true,false); assert(A.StartAttempt(0,U,1,'GOVERNOR',7)); ADVANCE(3)
          assert(A.State(0).LastMission.Result=='GOVERNOR_DEAD_PENDING')
          assert(A.Input(Players[1]).Governors['7'].Dead==1)
        """)

    def test_round_trip_reload_reinitializes_script_without_reroll(self):
        self.run_lua("""
          assert(A.StartProduction(0,10)); FORCE(true,false,'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER'))
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          FROZEN=clone(A.State(0)); ADVANCE(2)
          GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now
          rebuild(); EVENTS={}
        """)
        self.lua.execute(RULES)
        self.lua.execute(GAMEPLAY)
        self.run_lua("""
          A=JAC_ASN; local s=A.State(0)
          assert(s.Production.Due==FROZEN.Production.Due and s.Mission.Due==FROZEN.Mission.Due)
          assert(s.Mission.SuccessRoll==FROZEN.Mission.SuccessRoll and s.Mission.Branch==FROZEN.Mission.Branch)
          ADVANCE(5); assert(RAND_CALLS==3 and A.State(0).LastMission.Result=='LEADER_DEAD_PENDING')
          ADVANCE(95); assert(A.State(0).ProductionUnlock==1110)
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          local notes=#NOTES; A.Tick(); assert(#NOTES==notes)
          GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now; rebuild(); EVENTS={}
        """)
        self.lua.execute(RULES)
        self.lua.execute(GAMEPLAY)
        self.run_lua("""
          A=JAC_ASN; A.Tick(); assert(A.State(0).ProductionUnlock==1110)
          assert(Players[1]:GetProperty(A.NATION).LeaderDead==1)
          local ok,r=A.StartProduction(0,11); assert(not ok and r=='PRODUCTION_LOCK')
        """)


class AssassinPackagingTests(unittest.TestCase):
    def test_mod_scope_files_and_order(self):
        meta=ET.parse(MOD / "JAC_PoC_Assassin.modinfo").getroot()
        criteria=meta.findall("./ActionCriteria/Criteria")
        self.assertEqual(len(criteria),4)
        for c in criteria:
            self.assertEqual(c.findtext("RuleSetInUse"),"RULESET_JAC_BANSEIKI")
            self.assertRegex(c.findtext("ConfigurationValueMatches/Value"),r"JAC_Japan_(15|20)km(_Settlers)?\.Civ6Map$")
        for a in meta.find("InGameActions"):
            if a.tag!="ImportFiles": self.assertIn(a.get("criteria"),{c.get("id") for c in criteria})
            if a.tag=="UpdateDatabase": self.assertGreater(int(a.findtext("Properties/LoadOrder")),200)
            for f in a.findall("File"): self.assertTrue((MOD/f.text).is_file(),f.text)

    def test_data_loads_against_official_schema_after_sengoku(self):
        from tools.sengoku_cascade_audit import official_db, _run_sengoku
        db=_run_sengoku(official_db(),True)
        existing_fk_errors=set(db.execute("PRAGMA foreign_key_check"))
        db.executescript((MOD/"Data/JAC_Assassin.sql").read_text(encoding="utf-8"))
        row=db.execute("SELECT CanTrain,PurchaseYield,MustPurchase,PrereqTech,PrereqCivic FROM Units WHERE UnitType=?",
                       ("UNIT_JAC_JAPAN_SENGOKU_ASSASSIN",)).fetchone()
        self.assertEqual(row,(0,None,0,None,None))
        self.assertEqual(db.execute("SELECT * FROM UnitUpgrades WHERE Unit=? OR UpgradeUnit=?",
                         ("UNIT_JAC_JAPAN_SENGOKU_ASSASSIN",)*2).fetchall(),[])
        self.assertEqual(set(db.execute("PRAGMA foreign_key_check"))-existing_fk_errors,set())
        db.close()

    def test_japan_name_and_no_unapproved_cultural_names(self):
        rows=ET.parse(MOD/"Text/JAC_Assassin_Text.xml").getroot().find("LocalizedText")
        texts={(r.get("Tag"),r.get("Language")):r.findtext("Text") for r in rows}
        for lang,value in (("ja_JP","暗殺忍者"),("en_US","Assassin Ninja"),("ru_RU","Ниндзя-убийца")):
            self.assertEqual(texts[("LOC_JAC_ASN_NINJA_NAME",lang)],value)
        self.assertEqual(texts[("LOC_JAC_ASN_COMMON_NAME","ru_RU")],"Убийца")
        self.assertFalse(any(name in str(texts) for name in ("Hashashin","Cike","Jagaek")))

    def test_live_scenario_has_two_actual_round_trips_and_boundary_assertions(self):
        scenario=json.loads((ROOT/"tools/civ6-autotest/scenarios/assassin-leader-save-load.json").read_text(encoding="utf-8"))
        actions=[s["action"] for s in scenario["steps"]]
        self.assertEqual(actions.count("save"),2); self.assertEqual(actions.count("load"),2)
        code="\n".join(s.get("code","") for s in scenario["steps"])
        for needle in ("IsHuman()","SuccessRoll","PENDING_FREE_STATE","PRODUCTION_LOCK","MISSION_ACTIVE","COMMAND_LOCK","t.Lock-1","s.LastCompletion+10"):
            self.assertIn(needle,code)
        for step in scenario["steps"]:
            if step["action"]=="ui_lua": self.assertIn("assert(",step["code"])


if __name__ == "__main__":
    unittest.main()
