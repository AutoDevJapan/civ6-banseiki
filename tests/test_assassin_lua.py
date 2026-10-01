"""Execute the real Assassin gameplay and shared rules with mandatory lupa.

These stubs model persistence and engine calls, not native diplomacy, unit art or MP.
The live scenario tests the operation path, four outcomes and three save/load cycles.
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
NOW, SPEED, RAND_CALLS, SPAWN_BLOCKED, MAX_MAJOR = 1000, 'GAMESPEED_STANDARD', 0, false, 2
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
GameInfo={GameSpeeds={},Units={},Religions={
  [0]={ReligionType='RELIGION_PANTHEON',Pantheon=true},
  [1]={ReligionType='RELIGION_BUDDHISM',Pantheon=false},
  [2]={ReligionType='RELIGION_CATHOLICISM',Pantheon=false}}}
for _,speed in ipairs({'ONLINE','QUICK','STANDARD','EPIC','MARATHON'}) do
  local key='GAMESPEED_'..speed; GameInfo.GameSpeeds[key]={GameSpeedType=key}
end
for _,row in ipairs(unitRows) do GameInfo.Units[row.Index]=row; GameInfo.Units[row.UnitType]=row end
PLOTS={}
Map={GetPlotDistance=function(ax,ay,bx,by) return math.max(math.abs(ax-bx),math.abs(ay-by)) end,
  GetPlotCount=function() return #PLOTS end,
  GetPlotByIndex=function(i) local d=PLOTS[i+1]; return {
    GetOwner=function() return d.Owner end,GetX=function() return i end,GetY=function() return 0 end} end,
  GetAdjacentPlots=function(x,y) local d=PLOTS[x+1]; local result={}
    for dir=0,5 do local owner=(d.Adjacent or {})[dir]
      if owner then result[#result+1]={GetOwner=function() return owner end} end
    end; return result end}
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
    GetOwner=function() return d.Owner end, GetX=function() return d.X end,GetY=function() return d.Y end,
    GetDamage=function() return d.Damage or 0 end,GetMaxDamage=function() return 100 end,
    ChangeDamage=function(_,delta)
      d.Damage=(d.Damage or 0)+delta
      if UI_HANDLERS and UI_HANDLERS.UnitDamageChanged then UI_HANDLERS.UnitDamageChanged(d.Owner,d.ID,d.Damage) end
    end}
  props(u,d); return u
end
function makeCity(d)
  local c={GetID=function() return d.ID end,GetOwner=function() return d.Owner end,
    GetX=function() return d.X end,GetY=function() return d.Y end}
  props(c,d); return c
end
function rebuild()
  for id=0,MAX_MAJOR+1 do
    local d=DATA[id]
    local cities=memberCollection(d.Cities,makeCity)
    cities.GetCapitalCity=function() return cities:FindID(d.Capital) end
    local p={GetID=function() return id end,IsAlive=function() return d.Alive end,
      IsHuman=function() return id==0 end,IsTurnActive=function() return d.Alive end,
      GetCities=function() return cities end,GetUnits=function() return memberCollection(d.Units,makeUnit) end,
      GetStats=function() return {GetMilitaryStrength=function() return d.Military end} end,
      GetReligion=function() return {GetReligionInMajorityOfCities=function() return d.Religion end} end,
      GetDiplomacy=function() return {IsAtWarWith=function(_,other) return d.Wars[other]==true end,
        HasAllied=function(_,other) return d.Allies[other]==true end,
        GetFavor=function() return d.Favor end,ChangeFavor=function(_,n) d.Favor=d.Favor+n end} end}
    props(p,d); Players[id]=p
    PlayerConfigurations[id]={GetLeaderTypeName=function() return 'LEADER_'..id end,
      GetCivilizationTypeName=function() return d.Civilization or ({[0]='CIVILIZATION_JAC_ODA',[1]='CIVILIZATION_JAC_MATSUDAIRA',[2]='CIVILIZATION_JAC_AINU'})[id] end}
  end
end
for id=0,3 do
  DATA[id]={Properties={},Units={},Cities={},Capital=10,Alive=id~=3,Wars={},Allies={},Favor=0,Military=0,Religion=-1}
  if id~=3 then
    for _,cid in ipairs({10,11}) do DATA[id].Cities[cid]={ID=cid,Owner=id,X=id*2,Y=cid-10,Properties={}} end
  end
end
rebuild()
PlayerManager={GetFreeCitiesPlayerID=function() return MAX_MAJOR+1 end,GetAliveMajors=function()
  local t={}; for id=MAX_MAJOR,0,-1 do if DATA[id].Alive then t[#t+1]=Players[id] end end; return t
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
  if UI_HANDLERS and UI_HANDLERS.UnitRemovedFromMap then UI_HANDLERS.UnitRemovedFromMap(u:GetOwner(),u:GetID()) end
end}
TRANSFERS={}
CityManager={TransferCity=function(city,owner)
  local from,old=city:GetOwner(),city:GetID()
  local d=DATA[from].Cities[old]; DATA[from].Cities[old]=nil
  local newid=100+#TRANSFERS; d.Owner=owner; d.ID=newid; DATA[owner].Cities[newid]=d
  TRANSFERS[#TRANSFERS+1]={From=from,To=owner,X=d.X,Y=d.Y}
  DATA[from].Alive=Players[from]:GetCities():GetCount()>0 or Players[from]:GetUnits():GetCount()>0
end}
FREE_CITY_CALLS=0
CityManager.TransferCityToFreeCities=function(city)
  local free=PlayerManager.GetFreeCitiesPlayerID()
  FREE_CITY_CALLS=FREE_CITY_CALLS+1; CityManager.TransferCity(city,free); DATA[free].Alive=true
end
CityManager.GetCityAt=function(x,y)
  for id=0,MAX_MAJOR+1 do for _,city in pairs(DATA[id].Cities) do
    if city.X==x and city.Y==y then return makeCity(city) end
  end end
end
GameEvents=setmetatable({}, {__index=function(t,k)
  local event={Add=function(f) EVENTS[k]=f end}; rawset(t,k,event); return event
end})
UI_HANDLERS={}
Events=setmetatable({}, {__index=function(t,k)
  local event={Add=function(f) UI_HANDLERS[k]=f end,
    Remove=function(f) if UI_HANDLERS[k]==f then UI_HANDLERS[k]=nil end end}
  rawset(t,k,event); return event
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

    def test_unplaced_spawn_keeps_training_pending_without_starting_nationwide_lock(self):
        self.run_lua("""
          assert(A.StartProduction(0,10))
          local spawn=UnitManager.InitUnitValidAdjacentHex
          UnitManager.InitUnitValidAdjacentHex=function(id,kind,x,y)
            BAD_UNIT=GRANT(id,GameInfo.Units[kind].Index,-9999,-9999):GetID()
          end
          ADVANCE(100)
          local s=A.State(0)
          assert(s.Production and s.Production.Due==1100)
          assert(s.LastCompletion==nil and s.LastProducedUnit==nil and (s.ProductionUnlock or 0)<=NOW)
          assert(Players[0]:GetUnits():FindID(BAD_UNIT)==nil and #KILLS==1)
          UnitManager.InitUnitValidAdjacentHex=spawn; ADVANCE(1)
          s=A.State(0)
          assert(s.Production==nil and s.LastCompletion==1101 and s.ProductionUnlock==1111)
          local unit=Players[0]:GetUnits():FindID(s.LastProducedUnit)
          assert(unit and unit:GetX()>=0 and unit:GetY()>=0)
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

    def test_training_recalculates_current_factors_and_never_rerolls(self):
        self.run_lua("""
          assert(A.StartProduction(0,10)); ADVANCE(10)
          Players[0]:SetProperty(A.INPUT,{Security=100}); ADVANCE(1)
          assert(A.State(0).Production.Due==1120 and A.State(0).Production.Start==1000)
          Players[0]:SetProperty(A.INPUT,{Security=0,Policy=1,Relations={['1']=1}}); ADVANCE(1)
          assert(A.State(0).Production.Due==1060)
          Players[0]:GetCities():FindID(10):SetProperty(A.INPUT,{Building=1000})
          ADVANCE(7); assert(NOW==1019 and A.State(0).Production.Due==1020)
          assert(A.State(0).LastCompletion==nil); ADVANCE(1)
          assert(A.State(0).LastCompletion==1020 and A.State(0).ProductionUnlock==1030 and RAND_CALLS==0)
        """)

    def test_training_can_extend_again_and_finish_after_elapsed_reduction(self):
        self.run_lua("""
          local city=Players[0]:GetCities():FindID(10)
          city:SetProperty(A.INPUT,{Building=1000}); assert(A.StartProduction(0,10)); ADVANCE(19)
          city:SetProperty(A.INPUT,nil); ADVANCE(1)
          assert(A.State(0).Production.Due==1100 and A.State(0).LastCompletion==nil)
          ADVANCE(10); city:SetProperty(A.INPUT,{Building=1000}); ADVANCE(1)
          assert(A.State(0).LastCompletion==1031 and A.State(0).ProductionUnlock==1041)
        """)

    def test_dynamic_training_round_trip_keeps_start_speed_and_current_inputs(self):
        self.run_lua("""
          assert(A.StartProduction(0,10)); ADVANCE(10)
          Players[0]:SetProperty(A.INPUT,{Security=100}); ADVANCE(1)
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          ADVANCE(7); GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now
          rebuild(); EVENTS={}
        """)
        self.lua.execute(RULES); self.lua.execute(GAMEPLAY)
        self.run_lua("""
          A=JAC_ASN; A.Tick(); assert(A.State(0).Production.Due==1120)
          Players[0]:SetProperty(A.INPUT,{Security=0}); ADVANCE(1)
          local p=A.State(0).Production
          assert(p.Start==1000 and p.Speed==1 and p.Due==1080 and p.EvaluatedTurn==1012 and RAND_CALLS==0)
          ADVANCE(68); assert(A.State(0).LastCompletion==1080)
        """)

    def test_test_deadline_hook_is_inert_when_test_mode_is_off(self):
        self.run_lua("""
          assert(A.StartProduction(0,10)); assert(A.StartAttempt(0,U,1,'LEADER'))
          local original=A.State(0); original.ProductionUnlock=1099; Players[0]:SetProperty(A.STATE,original)
          local ok,reason=ExposedMembers.JAC_Assassin.TestAdvanceDeadlines(0)
          assert(not ok and reason=='TEST_MODE_OFF')
          local s=A.State(0)
          assert(s.Production.Due==1100 and s.Production.TestDue==nil and s.Production.Start==1000)
          assert(s.Mission.Due==1005 and s.Mission.SuccessRoll==original.Mission.SuccessRoll)
          assert(s.CommandUnlock==1050 and s.ProductionUnlock==1099 and RAND_CALLS==3)
        """)

    def test_test_deadlines_survive_recomputation_reload_and_resolve_on_real_turn(self):
        self.run_lua("""
          FORCE(true,false,'FREE_STATE'); assert(A.StartProduction(0,10)); assert(A.StartAttempt(0,U,1,'LEADER'))
          local s=A.State(0); s.ProductionUnlock=1099; Players[0]:SetProperty(A.STATE,s)
          assert(A.TestAdvanceDeadlines(0))
          s=A.State(0)
          assert(s.Production.Due==1001 and s.Production.TestDue==1001 and s.Production.Turns==100)
          assert(s.Mission.Due==1001 and s.CommandUnlock==1001 and s.ProductionUnlock==1001)
          assert(s.LastMission==nil and s.LastCompletion==nil and FREE_CITY_CALLS==0 and RAND_CALLS==3)
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now; rebuild(); EVENTS={}
        """)
        self.lua.execute(RULES); self.lua.execute(GAMEPLAY)
        self.run_lua("""
          A=JAC_ASN; ADVANCE(1); local s=A.State(0)
          assert(s.LastCompletion==1001 and s.Production==nil and s.ProductionUnlock==1011)
          assert(s.Mission==nil and s.LastMission.Result=='LEADER_FREE_STATE' and FREE_CITY_CALLS==2)
          assert(RAND_CALLS==3); ADVANCE(1); assert(FREE_CITY_CALLS==2)
        """)

    def test_test_deadline_hook_selects_clocks_and_never_extends_expired_locks(self):
        self.run_lua("""
          Game:SetProperty(A.TEST,1); assert(A.StartProduction(0,10)); assert(A.StartAttempt(0,U,1,'LEADER'))
          local s=A.State(0); s.ProductionUnlock=1000; Players[0]:SetProperty(A.STATE,s)
          assert(A.TestAdvanceDeadlines(0,{ProductionLock=true,CommandLock=true}))
          s=A.State(0); assert(s.ProductionUnlock==1000 and s.CommandUnlock==1001)
          assert(s.Production.Due==1100 and s.Mission.Due==1005)
          assert(A.TestAdvanceDeadlines(0,{Training=true}))
          Game:SetProperty(A.TEST,nil); ADVANCE(1)
          assert(A.State(0).Production.Due==1100 and A.State(0).Production.Turns==100)
          assert(A.State(0).LastCompletion==nil)
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
          assert(A.State(0).LastMission.Result=='LEADER_FREE_STATE')
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

    def test_leader_death_splits_all_cities_disbands_units_and_archives_people(self):
        self.run_lua("""
          local gp=GRANT(1,2,2,0); GRANT(1,4,2,0)
          Players[1]:SetProperty(A.INPUT,{Successors={{Identity='PERSON_GP',Kind='GREAT_PERSON',Unit=gp:GetID()}},
            Governors={['7']={City=10,Identity='GOV'}}})
          Players[1]:SetProperty(A.PEOPLE,{OLD={Identity='OLD',Dead=1,Unit=999,Governor=8,City=10}})
          FORCE(true,false,'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)
          local n=A.Nation(1)
          assert(n.LeaderDead==1 and n.State=='FREE_STATE' and n.DeathTurn==1005)
          assert(n.DecisionRequired==nil and n.Government=='FREE_CITIES' and n.Slot=='RETIRED' and not DATA[1].Alive)
          assert(n.FreeCitiesPlayer==3 and n.AI=='FREE_CITIES_ENGINE' and FREE_CITY_CALLS==2)
          assert(Players[1]:GetCities():GetCount()==0 and Players[1]:GetUnits():GetCount()==0)
          assert(DATA[0].Cities[10].Owner==0 and Players[3]:GetCities():GetCount()==2)
          assert(n.People.PERSON_GP.Role=='UNAFFILIATED' and n.People.PERSON_GP.SourceUnit==gp:GetID())
          assert(n.People.PERSON_GP.Unit==nil and n.People.GOV.Governor==nil and n.People.LEADER_1.Dead==1)
          assert(n.People.GOV.Role=='UNAFFILIATED' and n.People.GOV.SourceGovernor==7)
          assert(#n.Cities==2 and #n.DisbandedUnits==2)
          assert(n.People.OLD.Dead==1 and n.People.OLD.Role=='DEAD' and n.People.OLD.SourceUnit==999)
          assert(n.People.OLD.Unit==nil and n.People.OLD.Governor==nil and n.People.OLD.City==nil)
          assert(A.State(0).CommandUnlock==1050)
          Players[1]:SetProperty(A.NATION,nil); Players[1]:SetProperty(A.PEOPLE,nil)
          assert(A.Nation(1).People.PERSON_GP.Role=='UNAFFILIATED')
          local count=FREE_CITY_CALLS; A.Tick(); ADVANCE(1); assert(FREE_CITY_CALLS==count)
        """)

    def test_eliminated_human_slot_cancels_saved_training_after_reload_without_retransferring(self):
        self.run_lua("""
          assert(A.StartProduction(0,10)); local enemy=GRANT(2,0,1,0)
          Game:SetProperty(A.TEST,1); Players[2]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=1,Detected=0,Branch='FREE_STATE'})
          assert(A.StartAttempt(2,enemy:GetID(),0,'LEADER')); ADVANCE(5)
          assert(not Players[0]:IsAlive() and Players[0]:IsHuman() and A.Nation(0).Slot=='RETIRED')
          assert(Players[0]:GetUnits():GetCount()==0 and Players[0]:GetCities():GetCount()==0 and FREE_CITY_CALLS==2)
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now; rebuild(); EVENTS={}
        """)
        self.lua.execute(RULES); self.lua.execute(GAMEPLAY)
        self.run_lua("""
          A=JAC_ASN; ADVANCE(1)
          assert(A.State(0).Production==nil and A.State(0).LastCompletion==nil and not Players[0]:IsAlive())
          assert(A.Nation(0).State=='FREE_STATE' and FREE_CITY_CALLS==2 and RAND_CALLS==3)
        """)

    def test_post_death_snapshot_and_branch_are_frozen(self):
        self.run_lua("""
          FORCE(true,false,'SUCCESSION'); Players[1]:SetProperty(A.INPUT,{Security=10,Successor='PERSON_1'})
          assert(A.StartAttempt(0,U,1,'LEADER'))
          Players[1]:SetProperty(A.INPUT,{Security=90,Successor='PERSON_2'})
          ADVANCE(5); local n=Players[1]:GetProperty(A.NATION)
          assert(n.Branch=='SUCCESSION' and n.Snapshot.Security==10 and n.Snapshot.Successor.Identity=='PERSON_1')
          assert(n.CurrentLeader=='PERSON_1' and n.State=='SUCCESSION')
          assert(A.Input(Players[1]).Successor=='PERSON_2') -- a new, different candidate is retained
        """)

    def test_equivalent_favor_vote_is_available_without_test_mode_on_japan_map(self):
        self.run_lua("""
          assert(Game:GetProperty(A.TEST)==nil); DATA[2].Favor=60
          assert(A.StartAttempt(0,U,1,'LEADER'))
          local snapshot=A.State(0).Mission.Snapshot
          assert(snapshot.Congress and snapshot.Ballots['2'].Favor==60 and snapshot.Ballots['2'].Weight==4)
          Players[0]:SetProperty(A.STATE,nil); Players[1]:SetProperty(A.INPUT,{Congress={Enabled=false}})
          FORCE(true,false,'CONGRESS') -- an explicit disabled vote provider still excludes this branch
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

    def test_diplomacy_changes_recalculate_training_with_worst_living_major(self):
        self.run_lua("""
          assert(A.StartProduction(0,10))
          Players[0]:SetProperty(A.INPUT,{Relations={['0']=1,['1']=0.5,['2']=-1,['3']=1}})
          ADVANCE(1); assert(A.State(0).Production.Due==1095)
          DATA[0].Wars[2]=true
          ADVANCE(1); assert(A.State(0).Production.Due==1090)
          DATA[0].Wars[2]=nil; DATA[2].Alive=false
          Players[1]:SetProperty('JAC_ASN_DIPLOMACY_V1',{Friendship={['0']=-10}})
          ADVANCE(1); assert(A.State(0).Production.Due==1094)
          Players[0]:SetProperty(A.INPUT,{Relations={['1']=-1,['2']=1}})
          ADVANCE(1); assert(A.State(0).Production.Due==1100 and RAND_CALLS==0)
          assert(A.ProductionTurns({Relation=1})==90)
          assert(A.ProductionTurns({Relation=2})==90 and A.ProductionTurns({Relation=-1})==100)
          assert(A.ProductionTurns({Relation=1,Building=1000})==20)
          for _,v in ipairs({{0.5,45},{0.67,61},{1,90},{1.5,135},{3,270}}) do
            assert(A.ProductionTurns({Relation=1},v[1])==v[2])
          end
        """)

    def test_training_relation_survives_reload_and_completes_at_reduced_boundary(self):
        self.run_lua("""
          Players[0]:SetProperty(A.INPUT,{Relations={['1']=1}})
          assert(A.StartProduction(0,10)); ADVANCE(89)
          assert(A.State(0).LastCompletion==nil and A.State(0).Production.Due==1090)
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          ADVANCE(1); GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now
          rebuild(); EVENTS={}
        """)
        self.lua.execute(RULES); self.lua.execute(GAMEPLAY)
        self.run_lua("""
          A=JAC_ASN; A.Tick(); local p=A.State(0).Production
          assert(p.Start==1000 and p.Speed==1 and p.Turns==90 and p.Due==1090)
          ADVANCE(1); assert(A.State(0).LastCompletion==1090 and A.State(0).ProductionUnlock==1100)
          assert(RAND_CALLS==0)
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
          assert(A.State(0).LastCompletion==1099 and A.State(0).Production==nil)
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

    def test_actor_loss_cancels_all_four_outcomes_but_always_applies_fixed_penalty_once(self):
        for success,detected in ((False,False),(False,True),(True,False),(True,True)):
            with self.subTest(success=success,detected=detected):
                self.setUp()
                self.run_lua(f"""
                  local victim=GRANT(1,4,2,0):GetID(); FORCE({str(success).lower()},{str(detected).lower()})
                  assert(A.StartAttempt(0,U,1,'UNIT',victim)); DATA[0].Units[U]=nil
                  DATA[0].Units[U]={{ID=U,Owner=0,Type=0,X=1,Y=0,Properties={{}}}} -- replacement must survive
                  ADVANCE(2); local s=A.State(0); local p=Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1')
                  assert(s.LastMission.Result=='ACTOR_GONE' and s.LastMission.Cancelled)
                  assert(DATA[1].Units[victim] and DATA[0].Units[U] and #KILLS==0)
                  assert(p.Grievances['0']==25 and p.Friendship['0']==-10 and p.WarReason['0']==nil)
                  assert(Players[2]:GetProperty('JAC_ASN_DENOUNCE_0')==nil and s.CommandUnlock==1050)
                  A.Tick(); ADVANCE(2); assert(Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances['0']==25)
                """)

    def test_unregistered_great_person_keeps_original_unit_id_when_archived(self):
        self.run_lua("""
          local gp=GRANT(1,2,2,0); gp:SetProperty(A.PERSON,'UNREGISTERED_GP')
          FORCE(true,false,'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)
          local person=A.Nation(1).People.UNREGISTERED_GP
          assert(person.Role=='UNAFFILIATED' and person.SourceUnit==gp:GetID() and person.Unit==nil)
        """)

    def test_absorption_freezes_recipient_transfers_cities_disbands_and_preserves_people(self):
        self.run_lua("""
          DATA[2].Cities[12]={ID=12,Owner=2,X=4,Y=2,Properties={}}
          local gp=GRANT(1,2,2,0); local token=gp:GetID()
          GRANT(1,4,2,0); Players[1]:SetProperty(A.INPUT,{Governors={['7']={City=10,Identity='GOV'}}})
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          local m=A.State(0).Mission; assert(m.PostDeathPlan.Recipient==2)
          DATA[0].Cities[12]={ID=12,Owner=0,X=0,Y=2,Properties={}} -- later strength cannot change recipient
          ADVANCE(5); local n=A.Nation(1)
          assert(n.State=='ABSORPTION' and n.Recipient==2 and n.Slot=='RETIRED' and n.AI=='RECIPIENT_SLOT')
          assert(#TRANSFERS==2 and TRANSFERS[1].To==2 and TRANSFERS[2].To==2)
          assert(Players[1]:GetCities():GetCount()==0 and Players[1]:GetUnits():GetCount()==0 and not DATA[1].Alive)
          assert(Players[2]:GetCities():GetCount()==5 and Players[0]:GetCities():GetCount()==3)
          assert(Players[2]:GetProperty(A.PEOPLE).GOV.Role=='MEMBER')
          assert(Players[2]:GetProperty(A.PEOPLE).GOV.SourceGovernor==7)
          assert(Players[2]:GetProperty(A.PEOPLE)['LEADER_1']==nil)
          assert(Game:GetProperty(A.NATIONS)['1'].State=='ABSORPTION' and RAND_CALLS==3)
          local count=#TRANSFERS; A.Tick(); ADVANCE(1); assert(#TRANSFERS==count)
        """)

    def test_recipient_formula_combines_strength_friendship_border_distance_religion_culture(self):
        self.run_lua("""
          DATA[2].Military=200; DATA[0].Religion=1; DATA[1].Religion=1; DATA[2].Religion=2
          Players[0]:SetProperty(A.INPUT,{CultureGroup='A'})
          Players[2]:SetProperty(A.INPUT,{CultureGroup='B'})
          Players[1]:SetProperty(A.INPUT,{CultureGroup='A',Relations={['0']=-1,['2']=1}})
          PLOTS={{Owner=1,Adjacent={[0]=0,[1]=0,[2]=2}}}
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          local snapshot=A.State(0).Mission.Snapshot; local a,b=snapshot.Neighbors[1],snapshot.Neighbors[2]
          assert(a.Player==0 and b.Player==2 and snapshot.Recipient==0)
          assert(a.Military==0 and b.Military==200 and a.Border==2 and b.Border==1 and a.Distance==2)
          assert(math.abs(a.Score-0.775)<0.000001 and math.abs(b.Score-0.525)<0.000001)
          assert(a.AvailableWeight==100 and a.Components.Religion==1 and b.Components.Culture==0)
          DATA[2].Military=10000; DATA[1].Allies[2]=true; Players[1]:SetProperty(A.INPUT,{Relations={['2']=-1}})
          ADVANCE(5); assert(A.Nation(1).Recipient==0 and A.Nation(1).Snapshot.Neighbors[1].Score==a.Score)
        """)

    def test_recipient_alliance_is_gameplay_and_optional_closeness_does_not_reward_missing_data(self):
        self.run_lua("""
          DATA[1].Allies[2]=true
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          local snapshot=A.State(0).Mission.Snapshot; local n=snapshot.Neighbors[1]
          assert(n.Player==2 and n.Friendship==1 and n.Allied and n.AvailableWeight==90)
          assert(n.Components.Religion==nil and n.Components.Culture==nil)
          assert(snapshot.Recipient==2)
        """)

    def test_recipient_omits_pantheon_and_missing_apis_and_prefers_supplied_culture_over_proxy(self):
        self.run_lua("""
          DATA[0].Religion=0; DATA[1].Religion=0; DATA[2].Religion=0
          GameInfo.Civilizations={CIVILIZATION_JAC_ODA={Ethnicity='ASIAN'},
            CIVILIZATION_JAC_MATSUDAIRA={Ethnicity='ASIAN'},CIVILIZATION_JAC_AINU={Ethnicity='OTHER'}}
          Players[0]:SetProperty(A.INPUT,{CultureGroup='SUPPLIED'})
          Players[0].GetStats=nil; Map.GetAdjacentPlots=nil
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          for _,n in ipairs(A.State(0).Mission.Snapshot.Neighbors) do
            assert(n.Components.Religion==nil and n.Components.Border==nil and n.AvailableWeight==80)
            assert(n.Culture==0 and n.Military==0 and n.Strength==n.Cities)
          end
        """)

    def test_recipient_distance_and_border_can_change_choice_and_ties_use_player_id(self):
        self.run_lua("""
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          assert(A.State(0).Mission.PostDeathPlan.Recipient==0)
          Players[0]:SetProperty(A.STATE,nil)
          DATA[2].Cities[10].X=3; DATA[2].Cities[11].X=3
          PLOTS={{Owner=1,Adjacent={[0]=2,[1]=2}}}
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          assert(A.State(0).Mission.PostDeathPlan.Recipient==2)
        """)

    def test_favor_vote_weight_outvotes_more_nations_and_is_frozen_at_command(self):
        self.run_lua("""
          assert(A.FavorVotes(-10)==1 and A.FavorVotes(0)==1 and A.FavorVotes(9)==1)
          assert(A.FavorVotes(10)==2 and A.FavorVotes(29)==2 and A.FavorVotes(30)==3 and A.FavorVotes(60)==4)
          DATA[0].Favor=60
          Players[1]:SetProperty(A.INPUT,{Successor='HEIR',Congress={Votes={['0']='SUCCESSION',
            ['1']='FREE_STATE',['2']='FREE_STATE',['999']='FREE_STATE'}}})
          FORCE(true,false,'CONGRESS'); assert(A.StartAttempt(0,U,1,'LEADER'))
          local plan=A.State(0).Mission.PostDeathPlan
          assert(plan.Tally.SUCCESSION==4 and plan.Tally.FREE_STATE==2 and plan.Resolution=='SUCCESSION')
          assert(plan.Ballots['0'].Favor==60 and plan.Voting=='FAVOR_WEIGHTED_EQUIVALENT')
          DATA[0].Favor=0; DATA[1].Favor=1000; Players[1]:SetProperty(A.INPUT,{Congress={Votes={['0']='FREE_STATE'}}})
          ADVANCE(5); local n=A.Nation(1)
          assert(n.State=='SUCCESSION' and n.CurrentLeader=='HEIR' and n.Plan.Tally.SUCCESSION==4 and RAND_CALLS==3)
        """)

    def test_lost_absorption_recipient_falls_back_without_new_draw(self):
        self.run_lua("""
          DATA[2].Cities[12]={ID=12,Owner=2,X=4,Y=2,Properties={}}
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER')); DATA[2].Alive=false
          ADVANCE(5); assert(A.Nation(1).State=='FREE_STATE' and FREE_CITY_CALLS==2 and RAND_CALLS==3)
        """)

    def test_successor_kin_first_and_single_role_for_gp_and_governor(self):
        for kind in ("GREAT_PERSON","GOVERNOR"):
            with self.subTest(kind=kind):
                self.setUp()
                self.run_lua("""
                  GP=GRANT(1,2,2,0):GetID()
                  Players[1]:SetProperty(A.INPUT,{Governors={['7']={Identity='PERSON_KIN',City=10}},
                    Successors={{Identity='AAA',Kind='PERSON',Kin=0},
                    {Identity='PERSON_KIN',Name='Heir',Kin=1,CountryName='IGNORED',Kind='"""+kind+"""',Unit=GP,Governor=7}}})
                  FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)
                  local n=A.Nation(1); assert(n.CurrentLeader=='PERSON_KIN' and n.CountryName==nil)
                  assert(n.State=='SUCCESSION' and n.Slot=='RETAINED' and DATA[1].Alive and #TRANSFERS==0)
                  assert(A.ResolveTarget(1,'LEADER').Identity=='PERSON_KIN')
                """)
                if kind=="GREAT_PERSON":
                    self.run_lua("assert(DATA[1].Units[GP]==nil)")
                else:
                    self.run_lua("assert(A.Input(Players[1]).Governors['7'].Retired==1); assert(A.ResolveTarget(1,'GOVERNOR',7)==nil)")

    def test_lost_successor_falls_back_without_selecting_next_candidate(self):
        self.run_lua("""
          local gp=GRANT(1,2,2,0):GetID()
          Players[1]:SetProperty(A.INPUT,{Successors={{Identity='A',Kind='GREAT_PERSON',Unit=gp},{Identity='B'}}})
          FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER')); DATA[1].Units[gp]=nil
          ADVANCE(5); assert(A.Nation(1).State=='FREE_STATE' and A.Nation(1).CurrentLeader==nil and RAND_CALLS==3)
        """)

    def test_registered_death_overrides_present_gp_governor_or_person(self):
        for kind in ("PERSON","GREAT_PERSON","GOVERNOR"):
            for source in ("people","single","list"):
                with self.subTest(kind=kind,source=source):
                    self.setUp()
                    self.run_lua("""
                      GP=GRANT(1,2,2,0):GetID()
                      Players[1]:SetProperty(A.INPUT,{Governors={['7']={Identity='HEIR',City=10}},
                        Successor={Identity='HEIR',Kind='"""+kind+"""',Unit=GP,Governor=7}})
                      FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER'))
                    """)

                    if source=="people":
                        self.run_lua("Players[1]:SetProperty(A.PEOPLE,{HEIR={Identity='HEIR',Dead=1}})")
                    elif source=="single":
                        self.run_lua("local i=A.Input(Players[1]); i.Successor.Dead=1; Players[1]:SetProperty(A.INPUT,i)")
                    else:
                        self.run_lua("local i=A.Input(Players[1]); i.Successors={{Identity='HEIR',Dead=1}}; Players[1]:SetProperty(A.INPUT,i)")
                    self.run_lua("""
                      ADVANCE(5); assert(A.Nation(1).State=='FREE_STATE' and A.Nation(1).CurrentLeader==nil)
                      assert(DATA[1].Units[GP]==nil and A.Input(Players[1]).Governors==nil)
                      assert(RAND_CALLS==3)
                    """)


    def test_absorption_never_recreates_dead_registered_gp_under_a_unit_token(self):
        for source in ("people","single","list"):
            with self.subTest(source=source):
                self.setUp()
                self.run_lua("""
                  GP=GRANT(1,2,2,0):GetID()
                  Players[1]:SetProperty(A.INPUT,{Successor={Identity='DEAD_GP',Kind='GREAT_PERSON',Unit=GP}})
                  FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
                  assert(DATA[1].Units[GP].Properties[A.PERSON]=='DEAD_GP')
                """)
                if source=="people":
                    self.run_lua("Players[1]:SetProperty(A.PEOPLE,{DEAD_GP={Identity='DEAD_GP',Dead=1}})")
                elif source=="single":
                    self.run_lua("local i=A.Input(Players[1]); i.Successor.Dead=1; Players[1]:SetProperty(A.INPUT,i)")
                else:
                    self.run_lua("local i=A.Input(Players[1]); i.Successors={{Identity='DEAD_GP',Dead=1}}; Players[1]:SetProperty(A.INPUT,i)")
                self.run_lua("""
                  ADVANCE(5); assert(A.Nation(1).State=='ABSORPTION')
                  local roster=Players[0]:GetProperty(A.PEOPLE)
                  assert(next(roster)==nil and DATA[1].Units[GP]==nil)
                """)

    def test_persistent_person_binding_does_not_claim_same_id_replacement_gp(self):
        self.run_lua("""
          GP=GRANT(1,2,2,0):GetID()
          Players[1]:SetProperty(A.INPUT,{Successor={Identity='OLD_PERSON',Kind='GREAT_PERSON',Unit=GP}})
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          BOUND=Players[1]:GetProperty(A.BINDINGS).OLD_PERSON.Token
          assert(BOUND==DATA[1].Units[GP].Properties[A.TOKEN])
          DATA[1].Units[GP]={ID=GP,Owner=1,Type=2,X=2,Y=0,Properties={}}
          Players[1]:SetProperty(A.PEOPLE,{OLD_PERSON={Identity='OLD_PERSON',Dead=1}})
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now; rebuild(); EVENTS={}
        """)
        self.lua.execute(RULES); self.lua.execute(GAMEPLAY)
        self.run_lua("""
          A=JAC_ASN; ADVANCE(5)
          local people=Players[0]:GetProperty(A.PEOPLE); local count=0
          for _,p in pairs(people) do count=count+1; assert(p.Identity~='OLD_PERSON' and p.Dead~=1) end
          assert(count==1 and Players[1]:GetProperty(A.BINDINGS).OLD_PERSON.Token==BOUND)
          assert(RAND_CALLS==3 and A.Nation(1).State=='ABSORPTION')
        """)

    def test_absorption_people_have_one_identity_and_dead_leaders_never_rejoin(self):
        self.run_lua("""
          local gp=GRANT(1,2,2,0):GetID()
          Players[1]:SetProperty(A.INPUT,{Successors={{Identity='PERSON',Kind='GREAT_PERSON',Unit=gp}}})
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          local input=A.Input(Players[1]); input.Successors[#input.Successors+1]={Identity='NEW_PERSON'}
          Players[1]:SetProperty(A.INPUT,input); ADVANCE(5)
          local roster=Players[0]:GetProperty(A.PEOPLE); local count=0
          for _ in pairs(roster) do count=count+1 end
          assert(count==2 and roster.PERSON and roster.NEW_PERSON and roster.LEADER_1==nil)
          assert(roster.PERSON.Unit==nil and roster.PERSON.SourceUnit==gp and roster.PERSON.Role=='MEMBER')
        """)

    def test_later_leader_death_updates_people_registry_and_cannot_revive_predecessor(self):
        self.run_lua("""
          Players[1]:SetProperty(A.INPUT,{Successor='HEIR'})
          FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(50)
          assert(Players[1]:GetProperty(A.PEOPLE).LEADER_1.Dead==1)
          Players[1]:SetProperty(A.INPUT,{Successor='LEADER_1'})
          FORCE(true,false,'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER'))
          assert(A.State(0).Mission.Snapshot.Successor==nil); ADVANCE(5)
          local people=Players[1]:GetProperty(A.PEOPLE)
          assert(people.HEIR.Dead==1 and people.HEIR.Role=='DEAD_LEADER' and people.LEADER_1.Dead==1)
          assert(A.Nation(1).State=='FREE_STATE' and A.Nation(1).Previous.CurrentLeader=='HEIR')
        """)

    def test_registered_country_name_survives_kin_succession_and_free_city_archive(self):
        self.run_lua("""
          Players[1]:SetProperty(A.INPUT,{Successor={Identity='FOUNDER',CountryName='LOC_NEW_COUNTRY'}})
          FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(50)
          assert(A.Nation(1).CountryName=='LOC_NEW_COUNTRY')
          Players[1]:SetProperty(A.INPUT,{Successor={Identity='KIN',Kin=1,CountryName='IGNORE'}})
          FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(50)
          assert(A.Nation(1).CountryName=='LOC_NEW_COUNTRY')
          FORCE(true,false,'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)
          assert(A.Nation(1).CountryName=='LOC_NEW_COUNTRY')
        """)

    def test_congress_ballots_freeze_favor_weight_and_tie_goes_to_free_cities(self):
        for votes,outcome in (("{['0']='SUCCESSION',['1']='SUCCESSION',['999']='ABSORPTION'}","SUCCESSION"),
                              ("{['0']='FREE_STATE',['1']='SUCCESSION'}","FREE_STATE"),
                              ("{['0']='ABSORPTION',['1']='ABSORPTION'}","ABSORPTION"),
                              ("{}","FREE_STATE")):
            with self.subTest(votes=votes):
                self.setUp()
                self.run_lua("""
                  Players[1]:SetProperty(A.INPUT,{Successor='HEIR',Congress={Enabled=true,Votes="""+votes+"""}})
                  FORCE(true,false,'CONGRESS'); assert(A.StartAttempt(0,U,1,'LEADER'))
                  local plan=A.State(0).Mission.PostDeathPlan
                  Players[1]:SetProperty(A.INPUT,{Congress={Enabled=true,Votes={['0']='FREE_STATE'}}})
                  ADVANCE(5); local n=A.Nation(1)
                  assert(n.Branch=='CONGRESS' and n.State=='"""+outcome+"""' and n.Plan.Resolution==plan.Resolution)
                  assert(RAND_CALLS==3 and n.DecisionRequired==nil)
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
          ADVANCE(5); assert(RAND_CALLS==3 and A.State(0).LastMission.Result=='LEADER_FREE_STATE')
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

    def test_all_post_death_outcomes_round_trip_without_reapplying_assets(self):
        for branch in ("FREE_STATE","ABSORPTION","SUCCESSION","CONGRESS"):
            with self.subTest(branch=branch):
                self.setUp()
                self.run_lua("""
                  Players[1]:SetProperty(A.INPUT,{Successor='HEIR',Congress={Enabled=true,Votes={['0']='SUCCESSION'}}})
                  FORCE(true,false,'"""+branch+"""'); assert(A.StartAttempt(0,U,1,'LEADER'))
                  ADVANCE(1); SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
                  ADVANCE(4); EXPECTED=clone(A.Nation(1))
                  GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now; rebuild(); EVENTS={}
                """)
                self.lua.execute(RULES); self.lua.execute(GAMEPLAY)
                self.run_lua("""
                  A=JAC_ASN; ADVANCE(4); local n=A.Nation(1)
                  assert(n.State==EXPECTED.State and n.CurrentLeader==EXPECTED.CurrentLeader and n.Recipient==EXPECTED.Recipient)
                  assert(n.Plan.Branch==EXPECTED.Plan.Branch and n.Plan.Resolution==EXPECTED.Plan.Resolution and RAND_CALLS==3)
                  SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}; COUNT=#TRANSFERS
                  GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now; rebuild(); EVENTS={}
                """)
                self.lua.execute(RULES); self.lua.execute(GAMEPLAY)
                self.run_lua("A=JAC_ASN; A.Tick(); ADVANCE(1); assert(#TRANSFERS==COUNT and RAND_CALLS==3)")

    def test_earlier_pending_save_migrates_plan_without_new_rng(self):
        self.run_lua("""
          Players[1]:SetProperty(A.INPUT,{Successor='OLD_PERSON'})
          FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          local s=A.State(0); s.Mission.PostDeathPlan=nil; s.Mission.Snapshot.Successor='OLD_PERSON'
          s.Mission.Snapshot.Recipient=nil; Players[0]:SetProperty(A.STATE,s)
          ADVANCE(5); assert(A.Nation(1).CurrentLeader=='OLD_PERSON' and RAND_CALLS==3)
          Players[1]:SetProperty(A.NATION,nil)
          assert(A.Nation(1).CurrentLeader=='OLD_PERSON') -- use Game archive when native slot data is unavailable
        """)

    def test_live_fixture_reveals_hidden_target_without_disabling_gameplay_visibility(self):
        from tools.generate_assassin_poc import build_scenario
        code=next(s['code'] for s in build_scenario()['steps'] if s['action']=='ui_lua')
        helper=code[code.index('local function FixtureReveal'):code.index('local function Same')]
        self.run_lua("""
          REVEALS=0; HIDDEN_CITY=true; HIDDEN_UNIT=true
          Map.GetPlot=function(x,y) return {GetIndex=function() return x*1000+y end} end
          PlayersVisibility[0].ChangeVisibilityCount=function(_,index,count)
            assert(index==2000 and count==1); REVEALS=REVEALS+1
            HIDDEN_CITY=false; HIDDEN_UNIT=false
          end
        """)
        self.run_lua("local Map={}; local PlayersVisibility={}; local R=A; "+helper+"""
          assert(not pcall(function() FixtureReveal(0,2,0) end) and REVEALS==0)
          local ok,why=R.CheckAttempt(0,U,1,'LEADER'); assert(not ok and why=='TARGET_UNKNOWN')
          Game:SetProperty(R.TEST,1); FixtureReveal(0,2,0)
          assert(REVEALS==1 and R.KnownTarget(0,R.ResolveTarget(1,'LEADER')))
          assert(R.PROVISIONAL.Visibility=='REVEALED_CITY_VISIBLE_UNIT')
          assert(R.StartAttempt(0,U,1,'LEADER'))
        """)

    def test_live_fixture_reveals_unit_tile_separately_from_known_capital(self):
        from tools.generate_assassin_poc import build_scenario
        code=next(s['code'] for s in build_scenario()['steps'] if s['action']=='ui_lua')
        helper=code[code.index('local function FixtureReveal'):code.index('local function Same')]
        self.run_lua("""
          VISIBLE_TILES={[2000]=true}; REVEALED_TILES={[2000]=true,[3001]=true}; REVEALS=0
          Map.GetPlot=function(x,y) return {GetIndex=function() return x*1000+y end} end
          PlayersVisibility[0].IsRevealed=function(_,x,y) return REVEALED_TILES[x*1000+y]==true end
          PlayersVisibility[0].IsVisible=function(_,x,y) return VISIBLE_TILES[x*1000+y]==true end
          PlayersVisibility[0].ChangeVisibilityCount=function(_,index,count)
            assert(index==3001 and count==1); REVEALS=REVEALS+1
            VISIBLE_TILES[index]=true; REVEALED_TILES[index]=true
          end
          VICTIM=GRANT(1,4,3,1):GetID()
        """)
        self.run_lua("local Map={}; local PlayersVisibility={}; local R=A; "+helper+"""
          assert(R.KnownTarget(0,R.ResolveTarget(1,'LEADER')))
          local ok,why=R.CheckAttempt(0,U,1,'UNIT',VICTIM)
          assert(not ok and why=='TARGET_UNKNOWN' and R.State(0).Mission==nil)
          assert(not pcall(function() FixtureReveal(0,3,1) end) and REVEALS==0)
          Game:SetProperty(R.TEST,1); FixtureReveal(0,2,0)
          assert(REVEALS==0 and not R.KnownTarget(0,R.ResolveTarget(1,'UNIT',VICTIM)))
          ok,why=R.CheckAttempt(0,U,1,'UNIT',VICTIM)
          assert(not ok and why=='TARGET_UNKNOWN' and R.State(0).Mission==nil)
          FixtureReveal(0,3,1); FixtureReveal(0,3,1)
          assert(REVEALS==1 and R.PROVISIONAL.Visibility=='REVEALED_CITY_VISIBLE_UNIT')
          assert(R.StartAttempt(0,U,1,'UNIT',VICTIM))
          local target=R.State(0).Mission.Target
          assert(target.Kind=='UNIT' and target.Owner==1 and target.ID==VICTIM and target.X==3 and target.Y==1)
        """)

    def test_live_fixture_retries_unplaced_unit_without_killing_existing_units(self):
        from tools.generate_assassin_poc import build_scenario
        code=next(s['code'] for s in build_scenario()['steps'] if s['action']=='ui_lua')
        helper=code[code.index('local function FixtureReveal'):code.index('local function Same')]
        self.run_lua("""
          Game:SetProperty(A.TEST,1); GRANT_CALLS=0
          UnitManager.InitUnitValidAdjacentHex=function(owner,kind,x,y,radius)
            GRANT_CALLS=GRANT_CALLS+1; assert(radius==GRANT_CALLS)
            local u=GRANT(owner,GameInfo.Units[kind].Index,radius==1 and -9999 or x,y)
            if radius==1 then BAD_GRANT=u:GetID() end
          end
        """)
        self.run_lua("local R=A; "+helper+"""
          local u=Grant(0,R.UNIT,Players[1]:GetCities():GetCapitalCity())
          assert(GRANT_CALLS==2 and u:GetX()==2 and u:GetY()==0)
          assert(Players[0]:GetUnits():FindID(BAD_GRANT)==nil and #KILLS==1)
          assert(Players[0]:GetUnits():FindID(U) and Players[0]:GetUnits():FindID(U2))
        """)

    def test_live_trained_unit_check_rejects_unplaced_missing_or_wrong_type_unit(self):
        from tools.generate_assassin_poc import build_scenario
        code=next(s['code'] for s in build_scenario()['steps'] if s['action']=='ui_lua')
        helper=code[code.index('local function FixtureReveal'):code.index('local function Same')]
        self.run_lua("BAD_TRAINED=GRANT(0,0,-9999,-9999):GetID(); WRONG_TRAINED=GRANT(0,1,2,0):GetID()")
        self.run_lua("local R=A; "+helper+"""
          assert(TrainedUnit(0,U):GetID()==U)
          assert(not pcall(function() TrainedUnit(0,BAD_TRAINED) end))
          assert(not pcall(function() TrainedUnit(0,WRONG_TRAINED) end))
          assert(not pcall(function() TrainedUnit(0,-1) end))
        """)

    def test_live_training_check_requires_one_new_unit_and_rejects_reused_id(self):
        from tools.generate_assassin_poc import build_scenario
        code=next(s['code'] for s in build_scenario()['steps'] if s['action']=='ui_lua')
        helper=code[code.index('local function FixtureReveal'):code.index('local function Same')]
        self.run_lua("local R=A; "+helper+"""
          local before=TrainingUnits(0)
          assert(not pcall(function() NewTrainedUnit(0,before,U) end))
          local fresh=GRANT(0,0,2,0):GetID()
          assert(NewTrainedUnit(0,before,fresh+0.0):GetID()==fresh)
          assert(not pcall(function() NewTrainedUnit(0,before,U) end))
          GRANT(0,0,2,1)
          assert(not pcall(function() NewTrainedUnit(0,before,fresh) end))
        """)

    def _start_live_trained_unit_watch(self):
        from tools.generate_assassin_poc import build_scenario
        steps=build_scenario()['steps']
        setup=next(s['code'] for s in steps if 'JAC_ASN_LIVE_WATCH=watch' in s.get('code',''))
        check=next(s['code'] for s in steps if 'trained unit vanished without lethal native damage' in s.get('code',''))
        self.run_lua("""
          Game.GetLocalPlayer=function() return 0 end; Game:SetProperty(A.TEST,1)
          Players[0]:SetProperty(A.STATE,{LastProducedUnit=U2})
          Game:SetProperty('JAC_ASN_LIVE_TEST',{Actor=0,LockState={LastProducedUnit=U}})
        """)
        self.run_lua(setup)
        return check

    def test_live_unit_watch_accepts_witnessed_native_death_and_removes_observers(self):
        check=self._start_live_trained_unit_watch()
        self.run_lua("""
          UI_HANDLERS.UnitDamageChanged(0,U,100,83)
          UI_HANDLERS.UnitRemovedFromMap(0,U); DATA[0].Units[U]=nil
        """)
        self.run_lua(check)
        self.run_lua("""
          assert(Players[0]:GetUnits():FindID(U2))
          assert(JAC_ASN_LIVE_WATCH==nil and UI_HANDLERS.UnitDamageChanged==nil and UI_HANDLERS.UnitRemovedFromMap==nil)
        """)

    def test_live_unit_watch_rejects_unexplained_loss_or_replacement(self):
        for change in (
            "DATA[0].Units[U]=nil",
            "UI_HANDLERS.UnitDamageChanged(1,U,100,83); UI_HANDLERS.UnitRemovedFromMap(1,U); DATA[0].Units[U]=nil",
            "UI_HANDLERS.UnitRemovedFromMap(0,U); DATA[0].Units[U]=nil",
            "UI_HANDLERS.UnitDamageChanged(0,U,100,83); DATA[0].Units[U]=nil",
            "UI_HANDLERS.UnitDamageChanged(0,U,99,83); UI_HANDLERS.UnitRemovedFromMap(0,U); DATA[0].Units[U]=nil",
            "DATA[0].Units[U].X=-9999",
            "DATA[0].Units[U].Type=1",
            "UI_HANDLERS.UnitDamageChanged(0,U,100,83); UI_HANDLERS.UnitRemovedFromMap(0,U)",
        ):
            with self.subTest(change=change):
                self.setUp()
                check=self._start_live_trained_unit_watch()
                self.run_lua(change)
                with self.assertRaisesRegex(Exception,'trained'):
                    self.run_lua(check)
                self.run_lua("assert(JAC_ASN_LIVE_WATCH==nil and UI_HANDLERS.UnitDamageChanged==nil and UI_HANDLERS.UnitRemovedFromMap==nil)")

    def test_full_generated_live_sequence_resolves_in_18_real_ticks_and_round_trips(self):
        from tools.generate_assassin_poc import build_scenario
        self.run_lua("""
          MAX_MAJOR=4
          local civs={'ODA','MATSUDAIRA','IMAGAWA','ASAKURA','ASHIKAGA'}
          DATA={}
          for id=0,MAX_MAJOR+1 do
            DATA[id]={Properties={},Units={},Cities={},Capital=10,Alive=id<=MAX_MAJOR,
              Wars={},Allies={},Favor=0,Military=0,Religion=-1,
              Civilization=civs[id+1] and 'CIVILIZATION_JAC_'..civs[id+1] or 'CIVILIZATION_FREE_CITIES'}
            if id<=MAX_MAJOR then for _,cid in ipairs({10,11}) do
              DATA[id].Cities[cid]={ID=cid,Owner=id,X=id*2,Y=cid-10,Properties={}}
            end end
          end
          rebuild()
          function JAC_AssassinProduce(city)
            EVENTS.JAC_AssassinOperation(0,{Action='PRODUCE',City=city}); return true
          end
          function JAC_AssassinAttempt(unit,target,kind)
            EVENTS.JAC_AssassinOperation(0,{Action='ATTEMPT',Unit=unit,Target=target,Kind=kind}); return true
          end
          LIVE_SAVES={}
        """)
        turns=0
        for index,step in enumerate(build_scenario()["steps"]):
            with self.subTest(step=index,action=step["action"]):
                # UI alone may read the human player; core scripts keep the context guard.
                self.run_lua("Game.GetLocalPlayer=function() error('gameplay must not read local player') end")
                if step.get("state_name")=="JAC_AssassinPanel":
                    self.run_lua("Game.GetLocalPlayer=function() return 0 end")
                if step["action"]=="ui_lua":
                    self.run_lua(step["code"])
                elif step["action"]=="end_turn":
                    turns+=step.get("turns",1)
                    self.run_lua(f"ADVANCE({step.get('turns',1)})")
                elif step["action"]=="wait_for":
                    code=step["condition"]["code"]
                    if step["condition"]["state_name"]=="InGame":
                        self.run_lua("Game.GetLocalPlayer=function() return 0 end")
                    self.assertTrue(self.run_lua(code))
                elif step["action"]=="save":
                    self.run_lua("LIVE_SAVES['"+step["name"]+"']={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}")
                elif step["action"]=="load":
                    self.run_lua("local s=LIVE_SAVES['"+step["name"]+"']; GAME_PROPS,DATA,NOW=clone(s.Game),clone(s.Players),s.Now; rebuild(); EVENTS={}")
                    self.lua.execute(RULES); self.lua.execute(GAMEPLAY)
        self.assertEqual(turns,18)
        self.run_lua("assert(NOW==1018 and RAND_CALLS==20 and FREE_CITY_CALLS==2 and Game:GetProperty('JAC_ASN_TEST_MODE')==nil)")

    def test_live_final_check_rejects_penalty_added_during_load(self):
        scenario=json.loads((ROOT/"tools/civ6-autotest/scenarios/assassin-leader-save-load.json").read_text(encoding="utf-8"))
        checks=[s["code"] for s in scenario["steps"] if "load changed penalties" in s.get("code","")]
        self.assertEqual(len(checks),1)
        self.run_lua("""
          Game:SetProperty(A.NATIONS,{['1']={State='FREE_STATE'},['2']={CurrentLeader='JAC_LIVE_HEIR'},
            ['3']={State='ABSORPTION'},['4']={Branch='CONGRESS'}})
          Game:SetProperty('JAC_ASN_LIVE_TEST',{Actor=0,Target=1,LossTarget=1,Houses={CIVILIZATION_JAC_IMAGAWA=2,
            CIVILIZATION_JAC_ASAKURA=3,CIVILIZATION_JAC_ASHIKAGA=4},
            LockState={LastProducedUnit=U},
            FinalGrievances=100,FinalFriendship=-40,FinalApplied={m=1},FinalMissionID='m',FinalWait=1100})
          Players[1]:SetProperty('JAC_ASN_DIPLOMACY_V1',{Grievances={['0']=100},Friendship={['0']=-40},Applied={m=1}})
          Players[0]:SetProperty(A.STATE,{LastMission={ID='m',Result='ACTOR_GONE'},CommandUnlock=1100,LastProducedUnit=U2})
          local t=Game:GetProperty('JAC_ASN_LIVE_TEST'); t.FinalNations=Game:GetProperty(A.NATIONS)
          t.FinalState=A.State(0); Game:SetProperty('JAC_ASN_LIVE_TEST',t)
        """)
        self.run_lua(checks[0].split('local function Same')[0]+"""
          t.FinalUnits={}
          for _,id in ipairs({U,U2}) do t.FinalUnits[tostring(id)]=TrainedSnapshot(0,id) end
          Game:SetProperty('JAC_ASN_LIVE_TEST',t)
        """)
        self.run_lua(checks[0])
        for change in (
            "DATA[0].Units[U].X=DATA[0].Units[U].X+1",
            "DATA[0].Units[U2].Y=DATA[0].Units[U2].Y+1",
            "DATA[0].Units[U].Damage=83",
            "DATA[0].Units[U2]=nil",
        ):
            with self.subTest(change=change):
                self.run_lua("local saved=clone(DATA); BAD_LOAD_BACKUP=saved; "+change)
                with self.assertRaisesRegex(Exception,"load changed trained individual|trained unit is not placed"):
                    self.run_lua(checks[0])
                self.run_lua("DATA=BAD_LOAD_BACKUP; rebuild()")
        self.run_lua("""
          local ledger=Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1')
          ledger.Grievances['0']=125; Players[1]:SetProperty('JAC_ASN_DIPLOMACY_V1',ledger)
        """)
        with self.assertRaisesRegex(Exception,"load changed penalties"):
            self.run_lua(checks[0])


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

    def test_training_terms_in_all_three_languages(self):
        rows=ET.parse(MOD/"Text/JAC_Assassin_Text.xml").getroot().find("LocalizedText")
        texts={(r.get("Tag"),r.get("Language")):r.findtext("Text") for r in rows}
        for lang,word in (("ja_JP","育成"),("en_US","training"),("ru_RU","подготов")):
            for tag in ("PRODUCE","CANCEL","TRAINING","PRODUCED"):
                self.assertIn(word,texts[("LOC_JAC_ASN_"+tag,lang)].lower())
        self.assertNotRegex(str(texts),r"募集|[Rr]ecruit|[Нн]абор|подготовкаа")

    def test_live_scenario_has_three_actual_round_trips_and_boundary_assertions(self):
        scenario=json.loads((ROOT/"tools/civ6-autotest/scenarios/assassin-leader-save-load.json").read_text(encoding="utf-8"))
        actions=[s["action"] for s in scenario["steps"]]
        self.assertEqual(actions.count("save"),3); self.assertEqual(actions.count("load"),3)
        turns=sum(s.get("turns",1) for s in scenario["steps"] if s["action"]=="end_turn")
        self.assertEqual(turns,18)
        self.assertTrue(all(s["timeout"]>=600 for s in scenario["steps"] if s["action"]=="end_turn"))
        code="\n".join(s.get("code","") for s in scenario["steps"])
        for needle in ("IsHuman()","SuccessRoll","FREE_STATE","ABSORPTION","SUCCESSION","CONGRESS",
                       "PRODUCTION_LOCK","MISSION_ACTIVE","COMMAND_LOCK","t.Lock-1","s.LastCompletion+10",
                       "t.Start+120","t.Start+110","t.Start+70","t.MinStart+20","ACTOR_GONE","LossGrievances+25","WarReason","PostDeathPlan",
                       "TestAdvanceDeadlines","TEST_MODE_OFF","GetFreeCitiesPlayerID","receiver formula",
                       "GetDiplomacy()","ChangeFavor","favor did not outweigh majority","Same(s.Mission,t.Frozen)"):
            self.assertIn(needle,code)
        self.assertNotIn("PENDING_",code)
        self.assertNotRegex(code,r"local u=UnitManager.InitUnitValidAdjacentHex")
        lua=LuaRuntime()
        for step in scenario["steps"]:
            if step["action"]=="ui_lua":
                self.assertIn("assert(",step["code"])
                lua.compile(step["code"])

    def test_live_scenario_matches_generator_exactly(self):
        from tools.generate_assassin_poc import build_scenario
        expected=json.dumps(build_scenario(),ensure_ascii=False,indent=2)+"\n"
        actual=(ROOT/"tools/civ6-autotest/scenarios/assassin-leader-save-load.json").read_text(encoding="utf-8")
        self.assertEqual(actual,expected,"Regenerate with python tools/generate_assassin_poc.py")


if __name__ == "__main__":
    unittest.main()
