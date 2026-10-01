"""SSOT 9.1 absorption, using independent engine doubles and a real Lua VM (never skipped)."""
import json
from pathlib import Path
import sqlite3
import unittest
import xml.etree.ElementTree as ET

from lupa import LuaRuntime

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_Absorption"

ENGINE = r'''
local outcomes = ...
local function rows(list) local i=0;return function() i=i+1;return list[i] end end
GameInfo = {
  JAC_AbsorptionOutcomes = function() return rows(outcomes) end,
  UnitPromotions = function() return rows({{Index=7},{Index=9}}) end,
  Buildings = setmetatable({BUILDING_PALACE={Index=0,BuildingType='BUILDING_PALACE'}},
    {__call=function() return rows({{Index=0,BuildingType='BUILDING_PALACE'},{Index=1,BuildingType='BUILDING_TEST'}}) end}),
  Units = {[0]={UnitType="UNIT_WARRIOR"},[1]={UnitType="UNIT_GREAT_GENERAL"},[2]={UnitType="UNIT_MISSIONARY"},
           [3]={UnitType="UNIT_BUILDER"},[4]={UnitType="UNIT_HERO_JAC_WARLORD_A"},[5]={UnitType="UNIT_HERO_JAC_WARLORD_B"}},
  HeroClasses = setmetatable({[0]={Index=0,HeroClassType='HEROCLASS_JAC_WARLORD_A',UnitType='UNIT_HERO_JAC_WARLORD_A'},
    [1]={Index=1,HeroClassType='HEROCLASS_JAC_WARLORD_B',UnitType='UNIT_HERO_JAC_WARLORD_B'}},
    {__call=function(t) return rows({t[0],t[1]}) end}),
  Religions = {[5]={ReligionType="RELIGION_TEST"}},
  Technologies = {TECH_TEST={Index=11}}, Civics = {CIVIC_TEST={Index=12}}
}
UnitPropertyKeys={LifespanRemaining=4242}
TYPE = {UNIT_WARRIOR=0,UNIT_GREAT_GENERAL=1,UNIT_MISSIONARY=2,UNIT_BUILDER=3,UNIT_HERO_JAC_WARLORD_A=4,UNIT_HERO_JAC_WARLORD_B=5}
GameInfo.HeroClasses.HEROCLASS_JAC_WARLORD_A=GameInfo.HeroClasses[0]
GameInfo.HeroClasses.HEROCLASS_JAC_WARLORD_B=GameInfo.HeroClasses[1]
for name,id in pairs(TYPE) do GameInfo.Units[name]=GameInfo.Units[id];GameInfo.Units[id].Hash=100+id end
CURRENT_TURN, NEXT_ID, SPAWNS, KILLS, TRANSFERS = 5, 100, 0, 0, 0
FAIL_SPAWN, FAIL_CITY, FAIL_COPY, WIPE_ON_LAST_CITY = nil, false, false, true
HERO_CLAIMS,HERO_ORIGINS,HERO_DISCOVERED,HERO_CREATIONS={},{},{},0
HERO_CLAIM_ERAS,PLAYER_ERAS,FAITH={},{},{},{}
HERO_NO_SPAWN,HERO_THROW_AFTER_SPAWN,HERO_WRONG_OWNER,HERO_WRONG_ORIGIN=false,false,false,false
local function prop(obj)
  obj.props={}
  obj.GetProperty=function(self,key) return self.props[key] end
  obj.SetProperty=function(self,key,value) self.props[key]=value end
  return obj
end
Game = prop({GetCurrentGameTurn=function() return CURRENT_TURN end})
ExposedMembers={}
Players, PlayerConfigurations, CITIES, PLOTS = {}, {}, {}, {}
for owner=0,3 do
  PLAYER_ERAS[owner]=0;FAITH[owner]=0
  local p=prop({owner=owner,units={},cities={},techs={},civics={}})
  p.GetID=function() return owner end
  p.GetEras=function() return {GetEra=function() return PLAYER_ERAS[owner] end,
    SetEra=function(_,id) PLAYER_ERAS[owner]=id end} end
  p.IsAlive=function() return next(p.cities) ~= nil end
  p.IsHuman=function() return owner==0 end
  p.GetUnits=function() return {
    Members=function() return pairs(p.units) end,
    GetCount=function() local n=0;for _ in pairs(p.units) do n=n+1 end;return n end
  } end
  p.GetCities=function() return {
    Members=function() return pairs(p.cities) end,
    GetCount=function() local n=0;for _ in pairs(p.cities) do n=n+1 end;return n end,
    GetCapitalCity=function() local _,c=next(p.cities);return c end,
    FindID=function(_,id) for _,c in pairs(p.cities) do if c.id==id then return c end end end
  } end
  p.GetTechs=function() return {
    SetTech=function(_,id,v) p.techs[id]=v end,HasTech=function(_,id) return p.techs[id]==true end
  } end
  p.GetCulture=function() return {
    SetCivic=function(_,id,v) p.civics[id]=v end,HasCivic=function(_,id) return p.civics[id]==true end
  } end
  Players[owner]=p
  PlayerConfigurations[owner]={GetLeaderTypeName=function() return "LEADER_TEST_"..owner end}
end
function UNIT(owner,id,kind,x)
  local u=prop({owner=owner,id=id,kind=TYPE[kind],x=x or 4,y=0,damage=0,moves=2,xp=0,individual=-1,
    promotions={},formation=0,gpcharges=2,build=kind=="UNIT_BUILDER" and 3 or 0,spread=kind=="UNIT_MISSIONARY" and 3 or 0,religion=-1,
    heroClass=kind=='UNIT_HERO_JAC_WARLORD_A' and 0 or kind=='UNIT_HERO_JAC_WARLORD_B' and 1 or -1,lifespan=30,charges=6,renown=0})
  u.GetOwner=function(self) return self.owner end
  u.GetID=function(self) return self.id end
  u.GetType=function(self) return self.kind end
  local get,set=u.GetProperty,u.SetProperty
  u.GetProperty=function(self,key) if key==UnitPropertyKeys.LifespanRemaining then return self.lifespan end;return get(self,key) end
  u.SetProperty=function(self,key,value) if key==UnitPropertyKeys.LifespanRemaining then self.lifespan=value else set(self,key,value) end end
  u.IsHero=function(self) return self.heroClass>=0 end
  u.GetHeroClassType=function(self) return self.heroClass end
  -- Renown methods are optional legacy Debug-panel capabilities, not assumed present in the shipped build.
  u.GetRenown=function(self) return self.renown end;u.SetRenown=function(self,n) self.renown=n end
  u.GetActionCharges=function(self) return self.charges end;u.ChangeActionCharges=function(self,n) self.charges=self.charges+n end
  u.GetX=function(self) return self.x end;u.GetY=function(self) return self.y end
  u.GetDamage=function(self) return self.damage end
  u.SetDamage=function(self,n) if FAIL_COPY then error("copy failed") end;self.damage=n end
  u.GetMovesRemaining=function(self) return self.moves end
  u.ChangeMovesRemaining=function(self,n) self.moves=self.moves+n end
  u.GetMilitaryFormation=function(self) return self.formation end
  u.SetMilitaryFormation=function(self,n) self.formation=n end
  u.GetBuildCharges=function(self) return self.build end;u.GetSpreadCharges=function(self) return self.spread end
  u.GetName=function() return kind end
  u.GetGreatPerson=function(self) if self.kind~=1 then return nil end;return {
    GetIndividual=function() return self.individual end,GetActionCharges=function() return self.gpcharges end} end
  u.GetReligion=function(self)
    if self.kind~=2 then return nil end
    return {GetReligionType=function() return self.religion end,
      SetReligionType=function(_,name) assert(name=="RELIGION_TEST");self.religion=5 end}
  end
  u.GetExperience=function(self) return {
    HasPromotion=function(_,id) return self.promotions[id]==true end,
    SetPromotion=function(_,id) self.promotions[id]=true;self.damage=0;self.moves=2 end,
    GetExperiencePoints=function() return self.xp end,
    ChangeExperience=function(_,n) self.xp=self.xp+n end
  } end
  Players[owner].units[id]=u
  return u
end
function CITY(owner,x,pop,faith)
  local city=prop({owner=owner,id=x,x=x,y=0,pop=pop or 6,faith=faith or {['5']=4},loyalty=87,monument=true})
  city.GetID=function(self) return self.id end
  city.GetOwner=function(self) return self.owner end
  city.GetX=function(self) return self.x end;city.GetY=function(self) return self.y end
  city.GetPopulation=function(self) return self.pop end
  city.GetName=function(self) return "CITY_"..self.x end
  city.GetBuildQueue=function(self) return {
    CreateIncompleteBuilding=function(_,building,plot,progress)
      assert(building==1 and plot==self.x and progress==100,'building needs its city plot and construction progress')
      BUILDING_CREATIONS=(BUILDING_CREATIONS or 0)+1;self.monument=true
    end} end
  city.GetBuildings=function() return {
    HasBuilding=function(_,id) return (id==1 and city.monument) or (id==0 and city.palace==true) end,IsPillaged=function() return false end,
    GetNumGreatWorkSlots=function() return 1 end,GetGreatWorkInSlot=function(_,id) return id==0 and (city.palaceWork or -1) or 1007 end
  } end
  city.ChangeLoyalty=function(self,n) self.loyalty=self.loyalty+n end
  city.GetReligion=function(self) return {GetReligionsInCity=function()
    local result={};for id,n in pairs(self.faith) do result[#result+1]={Religion=tonumber(id),Followers=n} end;return result
  end} end
  Players[owner].cities[x]=city;CITIES[x]=city
  for index=x,x+1 do
    local plot={owner=owner,city=x,index=index}
    plot.GetOwner=function(self) return self.owner end
    plot.GetIndex=function(self) return self.index end
    PLOTS[index]=plot
  end
  return city
end
for i=0,19 do PLOTS[i]={GetOwner=function() return -1 end} end
Map={GetPlotCount=function() return 20 end,GetPlotByIndex=function(i) return PLOTS[i] end,
  GetPlot=function(x,y) assert(y==0);return PLOTS[x] end}
CityManager={GetCityAt=function(x,y) return CITIES[x] end,
  TransferCity=function(city,owner)
    if FAIL_CITY then error("city failed") end
    TRANSFERS=TRANSFERS+1
    local donor=city.owner
    Players[donor].cities[city.x]=nil;Players[owner].cities[city.x]=city;city.owner=owner;city.id=city.id+1000
    -- Deliberately replace properties and change engine IDs during transfer.
    city.props={}
    for _,plot in pairs(PLOTS) do if plot.city==city.x then plot.owner=owner end end
    if WIPE_ON_LAST_CITY and next(Players[donor].cities)==nil then Players[donor].units={} end
  end}
UnitManager={GetUnit=function(owner,id) return Players[owner].units[id] end,
  PlaceUnit=function(unit,x,y) unit.x=x;unit.y=y;unit.moves=0;unit.damage=0 end,
  Kill=function(unit) assert(Players[unit.owner].units[unit.id]);Players[unit.owner].units[unit.id]=nil;KILLS=KILLS+1 end,
  InitUnitValidAdjacentHex=function(owner,kind,x,y,radius)
    SPAWNS=SPAWNS+1
    if FAIL_SPAWN==SPAWNS then return end
    UNIT(owner,NEXT_ID,kind,x+1);NEXT_ID=NEXT_ID+1
    -- NO RETURN VALUE: verified live Civ6 behaviour.
  end}
Game.GetGreatPeople=function() return {CreatePerson=function(_,owner,individual,x,y)
  SPAWNS=SPAWNS+1;if FAIL_SPAWN==SPAWNS then return end
  local unit=UNIT(owner,NEXT_ID,"UNIT_GREAT_GENERAL",x+2);unit.individual=individual;NEXT_ID=NEXT_ID+1
end} end
HERO_MANAGER={
  GetHeroClaimPlayer=function(_,class) return HERO_CLAIMS[class] or -1 end,
  GetHeroOriginCityID=function(_,class) return HERO_ORIGINS[class] end,
  GetPlayerHeroOriginBuildingType=function(_,owner) return 1 end,
  IsHeroDiscovered=function(_,owner,class) return HERO_DISCOVERED[owner..':'..class]==true end,
  SetHeroDiscovered=function(_,owner,class,v) HERO_DISCOVERED[owner..':'..class]=v end,
  CreateHero=function(_,owner,class,x,y)
    assert(type(owner)=='number' and GameInfo.HeroClasses[class] and y==0,'CreateHero(player,class,x,y) signature')
    local city=assert(CITIES[x]);assert(city.owner==owner and city.monument,'native Hero needs an owned origin building')
    -- Native binding changes its claim before initialization, including failed initialization.
    HERO_CREATIONS=HERO_CREATIONS+1;HERO_CLAIMS[class]=HERO_WRONG_OWNER and 3 or owner;HERO_CLAIM_ERAS[class]=PLAYER_ERAS[owner]
    if HERO_NO_SPAWN then return end
    local origin=HERO_WRONG_ORIGIN and CITIES[12] or city
    HERO_ORIGINS[class]={player=origin.owner,id=origin.id}
    UNIT(owner,NEXT_ID,GameInfo.HeroClasses[class].UnitType,x);NEXT_ID=NEXT_ID+1
    if HERO_THROW_AFTER_SPAWN then error('hero init interrupted') end
    -- NO RETURN VALUE; ownership/origin are verified independently of the unit list.
  end}
Game.GetHeroesManager=function() return HERO_MANAGER end
CITY(0,0);CITY(1,4);CITY(1,8);CITY(2,12);CITY(3,16)
local warrior=UNIT(1,3,"UNIT_WARRIOR");warrior.damage=23;warrior.moves=1;warrior.xp=31;warrior.promotions[7]=true;warrior.formation=2
warrior:SetProperty("JAC_POC_GROWTHUI_WARLORD",{Level=4,XP=13,Personality="CALM",Equipment={"SWORD"},Artifact="RELIC"})
local general=UNIT(1,8,"UNIT_GREAT_GENERAL");general.individual=42
local priest=UNIT(1,10,"UNIT_MISSIONARY");priest.religion=5
local hero=UNIT(1,11,"UNIT_WARRIOR");hero:SetProperty("JAC_ABSORPTION_PERSON_V1",{
  ID="hero",Kind="HERO",Owner=1,Unit={Owner=1,ID=11},Contract="PROTECT",Loyalty=73,Lifespan=19,Relations={friend=25}})
local person=UNIT(1,12,"UNIT_WARRIOR");person:SetProperty("JAC_ABSORPTION_PERSON_V1",{
  ID="person",Kind="PERSON",Owner=1,Unit={Owner=1,ID=12},Level=6,Personality="LOYAL",Equipment={"ARMOR"}})
Players[1]:SetProperty("JAC_ABSORPTION_ASSETS_V1",{
  hero=hero:GetProperty("JAC_ABSORPTION_PERSON_V1"),person=person:GetProperty("JAC_ABSORPTION_PERSON_V1"),
  equipment={ID="equipment",Kind="ITEM",Owner=1,Type="EQUIPMENT",Anchor={Kind="PERSON",ID="person"}},
  artifact={ID="artifact",Kind="ITEM",Owner=1,Type="ARTIFACT",Anchor={Kind="VAULT",ID="vault:1"}},
  collection={ID="collection",Kind="ITEM",Owner=1,Type="COLLECTION",City={Owner=1,X=4,Y=0}},
  religion={ID="religion",Kind="RELIGION",Owner=1,ReligionID="SECT_1",Founder=1,State="PUBLIC",Followers=9,
    HolySites={{X=4,Y=0}},Clergy={"priest"},Scriptures={"book"},Relics={"artifact"},Sects={"SECT_1"}},
  unlock={ID="unlock",Kind="UNLOCK",Owner=1,Technology="TECH_TEST",Civic="CIVIC_TEST",Jobs={"MONK"}}
})
Players[2]:SetProperty("JAC_ABSORPTION_ASSETS_V1",{
  existing={ID="existing",Kind="ITEM",Owner=2,Type="EQUIPMENT"},
  samefaith={ID="samefaith",Kind="RELIGION",Owner=2,ReligionID="SECT_1",Founder=1,Followers=2,State="UNDERGROUND"}
})
CITIES[4]:SetProperty("JAC_ABSORPTION_CITY_FAITH_V1",{Followers=4,Scriptures={"book"},HolySite=true})
PLAN={ID="test-absorption",From=1,To=2,Mode="MERGER",EntryConditionsMet=true,RelationsResolved=true,
  Leader="RECRUIT",Units={['1:3']="INCORPORATE",['1:8']="RECRUIT",['1:10']="INCORPORATE",['1:11']="PROTECT",['1:12']="RECRUIT"},
  Assets={hero="PROTECT",person="RECRUIT",equipment="TRANSFER",artifact="SEIZE",collection="TRANSFER",religion="PRESERVE",unlock="INHERIT"}}
function print() end
'''


def outcome_rows():
    db = sqlite3.connect(":memory:")
    try:
        db.execute("CREATE TABLE Units (UnitType TEXT PRIMARY KEY, CanTrain INTEGER)")
        db.executescript((MOD / "Data/JAC_Absorption.sql").read_text(encoding="utf-8"))
        return [dict(zip(("Kind", "Outcome", "Destination"), row))
                for row in db.execute("SELECT Kind,Outcome,Destination FROM JAC_AbsorptionOutcomes")]
    finally:
        db.close()


class AbsorptionTests(unittest.TestCase):
    def setUp(self):
        self.lua = LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(ENGINE, self.lua.table_from(outcome_rows(), recursive=True))
        self.g = self.lua.globals()
        self.load_scripts()

    def load_scripts(self):
        self.lua.execute((MOD / "Scripts/JAC_Absorption_Rules.lua").read_text(encoding="utf-8"))
        self.g.include = lambda _: None
        self.lua.execute((MOD / "Scripts/JAC_Absorption_Gameplay.lua").read_text(encoding="utf-8"))
        self.a = self.g.ExposedMembers.JAC_Absorption

    def run_lua(self, code):
        return self.lua.execute(code)

    def apply(self):
        return self.a.Apply(self.g.PLAN)

    def verify(self, transaction_id, exact):
        if self.a.RequestVerification(transaction_id):
            self.run_lua("""if UI_PUBLISH then UI_PUBLISH()
              elseif PUBLISH_HERO_UI then PUBLISH_HERO_UI()
              else error('UI publication fixture missing') end""")
        return self.a.Verify(transaction_id, exact)

    def snapshot(self):
        return self.run_lua('''local a=ExposedMembers.JAC_Absorption;local result={State=a.GetState(),Owners={},Cities={},Units={},
          HeroClaims=HERO_CLAIMS,HeroOrigins=HERO_ORIGINS,HeroDiscoveries=HERO_DISCOVERED}
          for owner=0,3 do result.Owners[owner]=a.GetRoster(owner);result.Units[owner]={}
            for id,u in Players[owner]:GetUnits():Members() do
              result.Units[owner][id]={Owner=u.owner,Type=u.kind,Individual=u.individual,Damage=u.damage,
                Moves=u.moves,XP=u.xp,Promotions=u.promotions,Properties=u.props,
                HeroClass=u.heroClass,Lifespan=u.lifespan,ActionCharges=u.charges,Renown=u.renown} end end
          for x,c in pairs(CITIES) do result.Cities[x]={Owner=c:GetOwner(),Faith=a.CityReligion(c),Properties=c.props} end
          return a.Canonical(result)''')

    def assert_rejected_unchanged(self, code, message):
        self.run_lua(code)
        before = self.snapshot()
        with self.assertRaisesRegex(Exception, message):
            self.apply()
        self.assertEqual(self.snapshot(), before)
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (0, 0, 0))

    def test_all_assets_and_native_components_transfer_before_last_city_elimination(self):
        tx = self.apply()
        self.assertEqual(tx.Status, "COMPLETE")
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (6, 5, 2))
        self.assertTrue(self.verify(tx.ID, True))
        self.run_lua('''local a=ExposedMembers.JAC_Absorption;local r=a.GetRoster(2)
          assert(r.person.Owner==2 and r.person.Level==6 and r.person.Equipment[1]=='ARMOR')
          assert(r.hero.Owner==2 and r.hero.Contract=='PROTECT' and r.hero.Loyalty==73 and r.hero.Lifespan==19 and r.hero.Relations.friend==25)
          assert(r.religion.Founder==1 and r.religion.Followers==9 and r.religion.State=='PUBLIC' and r.religion.Clergy[1]=='priest')
          assert(r.samefaith.Followers==2 and r.samefaith.State=='UNDERGROUND' and r.existing.Owner==2)
          assert(r.collection.City.Owner==2 and r.equipment.Anchor.ID=='person')
          assert(r['leader:1'].Persistent==true and r['leader:1'].Unit.Owner==2)
          assert(Players[2]:GetTechs():HasTech(11) and Players[2]:GetCulture():HasCivic(12))
          assert(CITIES[4]:GetProperty('JAC_ABSORPTION_GOVERNANCE_V1').BalancePending)
          local u=a.FindUnit(2,'test-absorption:unit:3');assert(u.damage==23 and u.moves==1 and u.xp==31 and u.promotions[7] and u.formation==2)
          assert(u:GetProperty('JAC_POC_GROWTHUI_WARLORD').Artifact=='RELIC')
          local h=a.FindUnit(2,'test-absorption:unit:11'):GetProperty('JAC_ABSORPTION_PERSON_V1')
          assert(h.Owner==2 and h.Unit.Owner==2 and h.Unit.ID~=11 and h.OriginalOwner==1)
          assert(a.FindUnit(2,'test-absorption:unit:8').individual==42)
          assert(a.FindUnit(2,'test-absorption:unit:10').religion==5)''')

    def test_save_reload_discards_lua_session_and_restores_properties_without_replaying(self):
        self.apply()
        before = self.snapshot()
        # Deep-copy all saved properties, discard exported functions/local closures, then reload both scripts.
        self.run_lua('''local copy=JAC_AbsorptionRules.Copy
          Game.props=copy(Game.props);for _,p in pairs(Players) do p.props=copy(p.props)
            for _,u in pairs(p.units) do u.props=copy(u.props) end
            for _,c in pairs(p.cities) do c.props=copy(c.props) end end
          ExposedMembers={};JAC_AbsorptionRules=nil''')
        self.load_scripts()
        self.assertEqual(self.snapshot(), before)
        self.assertTrue(self.verify("test-absorption", True))
        self.apply()
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (6, 5, 2))

    def test_non_transfer_branches_keep_original_snapshots_and_detach_person_references(self):
        self.run_lua('''PLAN.Units['1:3']='ESCAPE';PLAN.Units['1:8']='RETIRE';PLAN.Units['1:11']='HIDE';PLAN.Assets.hero='HIDE'
          PLAN.Units['1:12']='RETIRE';PLAN.Assets.person='RETIRE';PLAN.Assets.artifact='SCATTER';PLAN.Assets.unlock='PRESERVE';PLAN.Leader='EXILE' ''')
        self.apply()
        self.assertTrue(self.verify("test-absorption", True))
        self.run_lua('''local s=ExposedMembers.JAC_Absorption.GetState();local ar=s.Archive
          assert(ar.hero.Owner==-1 and ar.hero.Lifespan==19 and ar.hero.Unit==nil and ar.hero.FormerUnit.ID==11)
          assert(ar.person.Outcome=='RETIRE' and ar.artifact.Outcome=='SCATTER' and ar['leader:1'].LeaderType=='LEADER_TEST_1')
          local retired=ar['native-person:1:8']
          assert(retired.Owner==-1 and retired.Outcome=='RETIRE' and retired.NativeIndividual==42 and retired.Unit==nil)
          assert(s.Transactions['test-absorption'].Units[1].Properties.JAC_POC_GROWTHUI_WARLORD.Level==4)
          assert(not Players[2]:GetTechs():HasTech(11) and not Players[2]:GetCulture():HasCivic(12))''')

    def test_returned_belonging_goes_to_explicit_surviving_third_party(self):
        self.run_lua("PLAN.Assets.artifact={Outcome='RETURN',ReturnOwner=3}")
        self.apply()
        self.assertEqual(self.a.GetRoster(3).artifact.Owner, 3)
        self.assertTrue(self.verify("test-absorption", True))

    def test_every_spec_disposition_is_routable_and_rejects_cross_kind_outcomes(self):
        expected = {
            "UNIT": {"SURRENDER", "INCORPORATE", "DISSOLVE", "ESCAPE", "RESIST"},
            "PERSON": {"CAPTURE", "RECRUIT", "SUBMIT", "EXILE", "EXECUTE", "RETIRE", "DEFECT"},
            "HERO": {"RECRUIT", "PROTECT", "RECONTRACT", "DEFECT", "HIDE", "SUICIDE"},
            "GREAT_PERSON": {"RECRUIT", "EXILE", "RETIRE"},
            "ITEM": {"TRANSFER", "LOOT", "SEIZE", "RETURN", "SCATTER"},
            "UNLOCK": {"INHERIT", "PRESERVE"}, "RELIGION": {"PRESERVE"},
        }
        actual = {}
        for row in outcome_rows():
            actual.setdefault(row["Kind"], set()).add(row["Outcome"])
            self.assertEqual(self.a.Rules.Destination(row["Kind"], row["Outcome"]), row["Destination"])
        self.assertEqual(actual, expected)
        self.assert_rejected_unchanged("PLAN.Assets.hero='EXECUTE'", "invalid disposition")

    def test_great_people_cannot_be_converted_to_yields(self):
        for outcome in ('RESEARCH', 'FAITH', 'CULTURE'):
            with self.subTest(outcome=outcome):
                self.assert_rejected_unchanged(f"PLAN.Units['1:8']='{outcome}'", "invalid disposition")

    def test_missing_unit_decision_is_rejected_before_any_mutation(self):
        self.assert_rejected_unchanged("PLAN.Units['1:3']=nil", "explicit disposition")

    def test_missing_asset_decision_is_rejected_before_any_mutation(self):
        self.assert_rejected_unchanged("PLAN.Assets.artifact=nil", "explicit disposition")

    def test_inconsistent_linked_person_and_unit_decisions_are_rejected(self):
        self.assert_rejected_unchanged("PLAN.Assets.hero='HIDE'", "dispositions disagree")

    def test_coexistence_is_not_absorption(self):
        for mode in ("MUTUAL_PROTECTION", "VASSAL", "COLONY", "FEDERATION", "DIRECT_RULE", "AUTONOMY"):
            with self.subTest(mode=mode):
                self.assert_rejected_unchanged(f"PLAN.Mode='{mode}'", "coexistence")

    def test_entry_and_external_relations_require_explicit_resolution(self):
        self.assert_rejected_unchanged("PLAN.RelationsResolved=false", "third-party")

    def test_asset_collision_is_rejected_before_engine_changes(self):
        self.assert_rejected_unchanged("Players[2].props.JAC_ABSORPTION_ASSETS_V1.hero={ID='hero',Kind='HERO',Owner=2}", "collision")

    def test_invalid_return_is_rejected(self):
        self.assert_rejected_unchanged("PLAN.Assets.artifact={Outcome='RETURN',ReturnOwner=1}", "third-party")

    def add_native_hero(self, *, second=False):
        self.run_lua('''
          local u=UNIT(1,13,'UNIT_HERO_JAC_WARLORD_A');u.lifespan=17;u.charges=2;u.renown=13;u.damage=7;u.moves=1
          HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=CITIES[4].id};HERO_DISCOVERED['1:0']=true
          local person={ID='native',Kind='HERO',Owner=1,Unit={Owner=1,ID=13},Contract='RECONTRACT',
            Loyalty=71,Lifespan=17,Relations={friend=29},Equipment={'SPEAR'}}
          u:SetProperty('JAC_ABSORPTION_PERSON_V1',person)
          Players[1].props.JAC_ABSORPTION_ASSETS_V1.native=person
          PLAN.Units['1:13']='RECONTRACT';PLAN.Assets.native='RECONTRACT'
        ''')
        if second:
            self.run_lua('''
              local u=UNIT(1,14,'UNIT_HERO_JAC_WARLORD_B');u.lifespan=21;u.charges=0;u.renown=9
              HERO_CLAIMS[1]=1;HERO_ORIGINS[1]={player=1,id=CITIES[8].id};HERO_DISCOVERED['1:1']=true
              PLAN.Units['1:14']='PROTECT'
            ''')

    def assert_native_suicide(self, cause, requested="RECONTRACT"):
        tx = self.apply()
        self.assertEqual(tx.Status, "COMPLETE")
        self.run_lua(f'''
          local a=ExposedMembers.JAC_Absorption
          local tx=a.GetState().Transactions['test-absorption']
          assert(tx.HeroResults.native.Outcome=='SUICIDE' and tx.HeroResults.native.Cause=='{cause}')
          assert(tx.HeroResults.native.RequestedOutcome=='{requested}')
          assert(tx.HeroResults.native.NativeClaimAfter==HERO_CLAIMS[0] or tx.HeroResults.native.NativeClaimAfter==nil)
          local snap
          for _,value in ipairs(tx.Units) do if value.ID==13 then snap=value end end
          assert(snap and snap.Outcome=='SUICIDE' and snap.RequestedOutcome=='{requested}')
          assert(snap.SuicideCause=='{cause}' and not snap.KeepUnit)
          local record=a.GetState().Archive.native
          assert(record and record.Outcome=='SUICIDE' and record.RequestedOutcome=='{requested}')
          assert(record.SuicideCause=='{cause}' and record.Unit==nil and record.FormerUnit.ID==13)
          assert(record.NativeUnit.Hero.ClassType=='HEROCLASS_JAC_WARLORD_A')
          assert(record.NativeUnit.Hero.Lifespan==17 and record.NativeUnit.Hero.ActionCharges==2)
          assert(record.Contract=='RECONTRACT' and record.Loyalty==71 and record.Relations.friend==29)
          assert(a.FindUnit(2,'test-absorption:unit:13')==nil)
          assert(a.GetRoster(2).native==nil and next(a.GetRoster(1))==nil)
          assert(a.Verify('test-absorption',true))
        ''')
        before = self.snapshot()
        self.load_scripts()
        self.apply()  # The original request still identifies the completed operation.
        self.assertEqual(self.snapshot(), before)

    def split_hero_contexts(self):
        self.add_native_hero(second=True)
        self.run_lua("""local full=Game.GetHeroesManager()
          UI_HERO_MANAGER={GetHeroClaimPlayer=full.GetHeroClaimPlayer,
            GetHeroOriginCityID=full.GetHeroOriginCityID,
            GetPlayerHeroOriginBuildingType=full.GetPlayerHeroOriginBuildingType}
          CORE_HERO_MANAGER={CreateHero=full.CreateHero,SetHeroDiscovered=full.SetHeroDiscovered,
            IsHeroDiscovered=full.IsHeroDiscovered}
          Game.GetHeroesManager=function() return UI_HERO_MANAGER end
          Events={LoadScreenClose={Add=function() end},GameCoreEventPublishComplete={Add=function(f) PUBLISH_HERO_UI=f end}}""")
        self.run_lua((MOD / "UI/JAC_Absorption_HeroKeys.lua").read_text(encoding="utf-8"))
        self.run_lua("""Game.GetHeroesManager=function() return CORE_HERO_MANAGER end
          assert(ExposedMembers.JAC_HeroReadAPI.CreateHero==nil)
          assert(ExposedMembers.JAC_HeroReadAPI.SetHeroDiscovered==nil)""")

    def split_unit_contexts(self):
        # Distinct UI handles keep GetSpreadCharges while every Core handle loses it.
        # The UI cache only gains newly created units when the test publishes Core changes.
        self.run_lua("""CORE_PLAYERS,CORE_GAME=Players,Game
          UI_SPREAD_CACHE={};UI_TURN=CURRENT_TURN;UI_PLAYERS={}
          UI_SYNC=function()
            UI_TURN=CURRENT_TURN
            for owner=0,3 do
              UI_SPREAD_CACHE[owner]={}
              for id,u in pairs(CORE_PLAYERS[owner].units) do UI_SPREAD_CACHE[owner][id]=u.spread end
            end
          end
          UI_SYNC()
          for owner=0,3 do
            local source=CORE_PLAYERS[owner]
            UI_PLAYERS[owner]={
              GetUnits=function() return {Members=function()
                local visible={}
                for id,charges in pairs(UI_SPREAD_CACHE[owner]) do
                  visible[id]={id=id,charges=charges,GetID=function(self) return self.id end,
                    GetSpreadCharges=function(self) return self.charges end}
                end
                return pairs(visible)
              end} end,
              GetCities=function() return source:GetCities() end}
          end
          UI_GAME=setmetatable({GetCurrentGameTurn=function() return UI_TURN end}, {__index=CORE_GAME})
          Game,Players=UI_GAME,UI_PLAYERS
          Events={LoadScreenClose={Add=function() end},
            GameCoreEventPublishComplete={Add=function(f) PUBLISH_HERO_UI=f end}}
        """)
        self.run_lua((MOD / "UI/JAC_Absorption_HeroKeys.lua").read_text(encoding="utf-8"))
        self.run_lua("""Game,Players=CORE_GAME,CORE_PLAYERS
          for _,p in pairs(Players) do
            for _,u in pairs(p.units) do u.GetSpreadCharges=nil end
          end
          local create=UNIT
          UNIT=function(...)
            local u=create(...);u.GetSpreadCharges=nil;return u
          end
          UI_PUBLISH=function() UI_SYNC();PUBLISH_HERO_UI() end
          assert(Players[1].units[3].GetSpreadCharges==nil)
          assert(type(ExposedMembers.JAC_HeroReadAPI.ReadUnit(1,3,'SPREAD_CHARGES',CURRENT_TURN))=='number')
        """)

    def finish_spread_publications(self):
        for _ in range(16):
            self.run_lua("UI_PUBLISH()")
            tx = self.a.Resume("test-absorption")
            if tx.Status == "COMPLETE":
                return tx
        self.fail("spread-charge UI publications did not finish")

    def test_native_clones_stage_at_recipient_city_then_return_to_original_positions(self):
        self.run_lua("""local spawn=UnitManager.InitUnitValidAdjacentHex
          UnitManager.InitUnitValidAdjacentHex=function(owner,kind,x,y,radius)
            assert(owner==2 and x==12 and y==0 and KILLS==0 and TRANSFERS==0)
            return spawn(owner,kind,x,y,radius)
          end
          local manager=Game.GetGreatPeople();local create=manager.CreatePerson
          manager.CreatePerson=function(self,owner,individual,x,y)
            assert(owner==2 and x==12 and y==0 and KILLS==0 and TRANSFERS==0)
            return create(self,owner,individual,x,y)
          end
          Game.GetGreatPeople=function() return manager end""")
        tx = self.apply()
        self.assertEqual(tx.Status, 'COMPLETE')
        self.run_lua("""for _,s in ipairs(ExposedMembers.JAC_Absorption.GetState().Transactions['test-absorption'].Units) do
          if s.KeepUnit then local u=ExposedMembers.JAC_Absorption.FindUnit(s.Destination,s.Token)
            assert(u:GetX()==s.X and u:GetY()==s.Y) end
        end""")

    def test_large_army_batches_publication_requests_instead_of_waiting_per_unit(self):
        self.run_lua("""for id=1000,1019 do UNIT(1,id,'UNIT_WARRIOR',4,0);PLAN.Units['1:'..id]='INCORPORATE' end""")
        self.split_unit_contexts()
        tx = self.apply()
        publications = 0
        while tx.Status != 'COMPLETE' and publications < 4:
            self.run_lua('UI_PUBLISH()')
            tx = self.a.Resume('test-absorption')
            publications += 1
        self.assertEqual(tx.Status, 'COMPLETE')
        self.assertEqual(self.g.SPAWNS, 26)
        self.assertTrue(self.verify('test-absorption', True))

    def test_off_map_native_candidates_are_placed_before_transfer_and_verified(self):
        self.run_lua("""local create=UNIT
          UNIT=function(...) local u=create(...);u.x=-9999;u.y=-9999;return u end""")
        tx = self.apply()
        self.assertEqual(tx.Status, "COMPLETE")
        self.assertEqual(self.g.SPAWNS, 6)
        self.assertTrue(self.verify(tx.ID, True))
        self.run_lua("""local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3')
          assert(u:GetX()==4 and u:GetY()==0);u.x=-9999;u.y=-9999""")
        with self.assertRaisesRegex(Exception, "off-map"):
            self.verify(tx.ID, True)

    def test_failed_off_map_placement_keeps_donors_and_reuses_saved_candidate_on_resume(self):
        self.run_lua("""local create=UNIT
          UNIT=function(...) local u=create(...);u.x=-9999;u.y=-9999;return u end
          ORIGINAL_PLACE=UnitManager.PlaceUnit;UnitManager.PlaceUnit=function() end""")
        with self.assertRaisesRegex(Exception, "placement failed; saved candidate retained"):
            self.apply()
        candidate_id = self.a.FindUnit(2, 'test-absorption:unit:3').GetID(self.a.FindUnit(2, 'test-absorption:unit:3'))
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (1, 0, 0))
        self.run_lua("UnitManager.PlaceUnit=ORIGINAL_PLACE")
        tx = self.a.Resume('test-absorption')
        self.assertEqual(tx.Status, "COMPLETE")
        self.assertEqual(self.g.SPAWNS, 6)
        unit = self.a.FindUnit(2, 'test-absorption:unit:3')
        self.assertEqual(unit.GetID(unit), candidate_id)
        self.assertTrue(self.verify(tx.ID, True))

    def test_empty_army_still_waits_for_city_cache_publication_before_preparation(self):
        self.split_unit_contexts()
        self.run_lua("""Players[1].units={};Players[1].props.JAC_ABSORPTION_ASSETS_V1={};PLAN.Units={};PLAN.Assets={}
          UI_CITY_FAITH={}
          for x,c in pairs(CITIES) do
            UI_CITY_FAITH[x]=ExposedMembers.JAC_Absorption.Rules.Copy(c.faith)
            local get=c.GetReligion
            c.GetReligion=function(self) local r=get(self);r.GetReligionsInCity=nil;return r end
          end
          local original=ExposedMembers.JAC_HeroReadAPI.ReadCity
          ExposedMembers.JAC_HeroReadAPI.ReadCity=function(owner,id,method,...)
            if method~='RELIGIONS' then return original(owner,id,method,...) end
            local city=assert(Players[owner]:GetCities():FindID(id))
            local rows={};for religion,count in pairs(UI_CITY_FAITH[city:GetX()]) do
              rows[#rows+1]={Religion=tonumber(religion),Followers=count}
            end;return rows
          end
          CITY_PUBLISH=function()
            for x,c in pairs(CITIES) do UI_CITY_FAITH[x]=ExposedMembers.JAC_Absorption.Rules.Copy(c.faith) end
            UI_PUBLISH()
          end
          CITIES[4].faith['5']=5""")
        tx = self.apply()
        self.assertEqual(tx.Status, 'WAITING_UI')
        self.assertTrue(tx.AwaitingPreparation)
        self.assertEqual((self.g.SPAWNS, self.g.TRANSFERS), (0, 0))
        self.assertEqual(self.a.Resume('test-absorption').Status, 'WAITING_UI')
        self.run_lua('CITY_PUBLISH()')
        tx = self.a.Resume('test-absorption')
        self.assertEqual(tx.Cities[1].Faith['5'], 5)
        self.assertEqual(tx.Status, 'WAITING_UI')
        self.run_lua('CITY_PUBLISH()')
        self.assertEqual(self.a.Resume('test-absorption').Status, 'COMPLETE')

    def test_public_verification_requires_a_new_publication_and_consumes_it(self):
        self.split_unit_contexts()
        self.apply()
        self.finish_spread_publications()
        with self.assertRaisesRegex(Exception, 'verification requires a published UI request'):
            self.a.Verify('test-absorption', True)
        self.a.RequestVerification('test-absorption')
        with self.assertRaisesRegex(Exception, 'verification requires a published UI request'):
            self.a.Verify('test-absorption', True)
        self.run_lua('UI_PUBLISH()')
        self.assertTrue(self.a.Verify('test-absorption', True))
        with self.assertRaisesRegex(Exception, 'verification requires a published UI request'):
            self.a.Verify('test-absorption', True)
        self.run_lua("ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:10').spread=1")
        self.a.RequestVerification('test-absorption')
        self.run_lua('UI_PUBLISH()')
        with self.assertRaisesRegex(Exception, 'spent spread charges'):
            self.a.Verify('test-absorption', True)
        self.assertIsNone(self.a.GetState().Transactions['test-absorption'].VerificationUI)

    def test_ui_only_purchase_cost_and_faith_balance_return_scalars_to_core(self):
        self.split_unit_contexts()
        self.run_lua("""MilitaryFormationTypes={STANDARD_MILITARY_FORMATION=0}
          for owner=0,3 do
            local n=owner
            UI_PLAYERS[n].GetCities=function() return {FindID=function(_,id)
              return {GetGold=function() return {GetPurchaseCost=function(_,yield,hash,formation)
                assert(yield==6 and hash==104 and formation==0);return 1000 end} end}
            end} end
            UI_PLAYERS[n].GetReligion=function() return {GetFaithBalance=function() return FAITH[n] end} end
          end
          Game,Players=UI_GAME,UI_PLAYERS""")
        self.run_lua((MOD / "UI/JAC_Absorption_HeroKeys.lua").read_text(encoding="utf-8"))
        self.run_lua("""Game,Players=CORE_GAME,CORE_PLAYERS;FAITH[2]=777
          for _,c in pairs(CITIES) do c.GetGold=nil end
          for _,p in pairs(Players) do p.GetReligion=function() return {} end end""")
        self.assertEqual(self.a.CityPurchaseCost(self.g.CITIES[4], 6, 104), 1000)
        self.assertEqual(self.a.PlayerFaithBalance(2), 777)

    def test_ui_only_spread_getter_waits_for_publication_and_reuses_candidates(self):
        self.split_unit_contexts()
        tx = self.apply()
        self.assertEqual(tx.Status, "WAITING_UI")
        self.assertTrue(tx.AwaitingPreparation)
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (0, 0, 0))
        self.run_lua("UI_PUBLISH()")
        tx = self.a.Resume("test-absorption")
        self.assertEqual(tx.Status, "WAITING_UI")
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (6, 0, 0))
        self.run_lua("""local tx=ExposedMembers.JAC_Absorption.GetState().Transactions['test-absorption']
          for _,snap in ipairs(tx.Units) do
            if snap.ID>=0 then
              assert(type(snap.SpreadCharges)=='number')
              if snap.ID==10 then assert(snap.SpreadCharges==3) else assert(snap.SpreadCharges==0) end
            end
          end
          assert(tx.Units[1].SpreadReads.final.AwaitingUI)
        """)
        self.load_scripts()
        self.assertEqual(self.a.Resume("test-absorption").Status, "WAITING_UI")
        self.run_lua("PUBLISH_HERO_UI()")  # The candidate is still absent from the UI cache.
        self.assertEqual(self.a.Resume("test-absorption").Status, "WAITING_UI")
        self.run_lua("UI_SYNC();UI_TURN=CURRENT_TURN-1;PUBLISH_HERO_UI()")
        self.assertEqual(self.a.Resume("test-absorption").Status, "WAITING_UI")
        self.assertEqual((self.g.SPAWNS, self.g.KILLS), (6, 0))
        tx = self.finish_spread_publications()
        self.assertEqual(tx.Status, "COMPLETE")
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (6, 5, 2))
        self.assertTrue(self.verify("test-absorption", True))
        self.run_lua("""local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:10')
          u.spread=1""")
        with self.assertRaisesRegex(Exception, "spent spread charges"):
            self.verify("test-absorption", True)

    def test_ui_only_spread_mismatch_blocks_and_preserves_donors_until_resume(self):
        self.run_lua("Players[1].units[10].spread=1")  # A spent missionary charge has no setter.
        self.split_unit_contexts()
        self.assertEqual(self.apply().Status, "WAITING_UI")
        self.run_lua("UI_PUBLISH()")
        self.assertEqual(self.a.Resume("test-absorption").Status, "WAITING_UI")
        self.run_lua("""local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:10')
          MISSIONARY_CANDIDATE=u:GetID();assert(u.spread==3)""")
        for _ in range(16):
            self.run_lua("UI_PUBLISH()")
            try:
                tx = self.a.Resume("test-absorption")
            except Exception as exc:
                self.assertIn("spent spread charges", str(exc))
                break
            self.assertEqual(tx.Status, "WAITING_UI")
        else:
            self.fail("UI spread mismatch was not detected")
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (6, 0, 0))
        self.assertEqual(self.a.GetState().Transactions["test-absorption"].Status, "BLOCKED")
        self.run_lua("""assert(Players[1].units[10] and Players[1].units[3])
          assert(ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:10'):GetID()==MISSIONARY_CANDIDATE)
          ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:10').spread=1""")
        tx = self.finish_spread_publications()
        self.assertEqual(tx.Status, "COMPLETE")
        self.assertEqual(self.g.SPAWNS, 6)
        self.assertTrue(self.verify("test-absorption", True))

    def test_old_turn_ui_spread_cache_waits_before_preparation_and_resumes(self):
        self.split_unit_contexts()
        self.run_lua("UI_TURN=CURRENT_TURN-1")
        tx = self.apply()
        self.assertEqual(tx.Status, "WAITING_UI")
        self.assertTrue(tx.AwaitingPreparation)
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (0, 0, 0))
        self.run_lua("""local s=Game:GetProperty('JAC_ABSORPTION_STATE_V1')
          s.Transactions['test-absorption'].SourceSpreadReads=nil;Game:SetProperty('JAC_ABSORPTION_STATE_V1',s)""")
        self.load_scripts()
        self.assertEqual(self.a.Resume("test-absorption").Status, "WAITING_UI")
        self.run_lua("UI_SYNC()")
        self.assertEqual(self.a.Resume("test-absorption").Status, "WAITING_UI")
        self.assertEqual(self.g.SPAWNS, 0)
        self.run_lua("PUBLISH_HERO_UI()")
        self.assertEqual(self.a.Resume("test-absorption").Status, "WAITING_UI")
        self.assertEqual(self.g.SPAWNS, 6)
        self.assertEqual(self.finish_spread_publications().Status, "COMPLETE")

    def test_same_turn_spent_spread_is_published_before_snapshot_and_never_restored_as_unused(self):
        self.split_unit_contexts()
        self.run_lua("Players[1].units[10].spread=1")  # UI still shows three in the same turn.
        tx = self.apply()
        self.assertTrue(tx.AwaitingPreparation)
        self.assertEqual(self.g.SPAWNS, 0)
        self.load_scripts()
        self.assertEqual(self.a.Resume("test-absorption").Status, "WAITING_UI")
        self.assertEqual(self.g.SPAWNS, 0)
        self.run_lua("UI_PUBLISH()")
        tx = self.a.Resume("test-absorption")
        self.assertEqual(tx.Status, "WAITING_UI")
        self.assertEqual(next(s.SpreadCharges for _, s in tx.Units.items() if s.ID == 10), 1)
        for _ in range(20):
            self.run_lua("UI_PUBLISH()")
            try:
                tx = self.a.Resume("test-absorption")
            except Exception as exc:
                self.assertIn("spent spread charges", str(exc))
                break
        else:
            self.fail("spent donor was incorrectly accepted as unused")
        self.assertEqual(self.a.GetState().Transactions["test-absorption"].Status, "BLOCKED")
        self.assertEqual((self.g.KILLS, self.g.TRANSFERS), (0, 0))
        self.assertEqual(self.g.Players[1].units[10].spread, 1)

    def test_rejected_preparation_releases_factions_and_corrected_request_waits_for_new_publication(self):
        self.split_unit_contexts()
        self.run_lua("PLAN.Units['1:3']=nil")
        tx = self.apply()
        previous_token = tx.SourceSpreadReads[10].Token
        self.assertTrue(tx.AwaitingPreparation)
        self.run_lua("UI_PUBLISH()")
        with self.assertRaisesRegex(Exception, "explicit disposition required"):
            self.a.Resume("test-absorption")
        self.assertIsNone(self.a.GetState().Transactions["test-absorption"])
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (0, 0, 0))
        self.run_lua("PLAN.Units['1:3']='INCORPORATE';Players[1].units[10].spread=1")
        tx = self.apply()
        self.assertTrue(tx.AwaitingPreparation)
        self.assertNotEqual(tx.SourceSpreadReads[10].Token, previous_token)
        self.assertEqual(self.a.Resume("test-absorption").Status, "WAITING_UI")
        self.assertEqual(self.g.SPAWNS, 0)
        self.run_lua("UI_PUBLISH()")
        tx = self.a.Resume("test-absorption")
        self.assertEqual(tx.Status, "WAITING_UI")
        self.assertEqual(next(s.SpreadCharges for _, s in tx.Units.items() if s.ID == 10), 1)

    def test_native_hero_ui_only_spread_wait_never_becomes_suicide(self):
        self.add_native_hero()
        self.split_unit_contexts()
        self.assertEqual(self.apply().Status, "WAITING_UI")
        saw_hero_wait = False
        for _ in range(20):
            self.run_lua("UI_PUBLISH()")
            tx = self.a.Resume("test-absorption")
            if tx.Status == "WAITING_UI" and self.g.HERO_CREATIONS == 1:
                saw_hero_wait = True
                creations = self.g.HERO_CREATIONS
                self.assertEqual(self.a.Resume("test-absorption").Status, "WAITING_UI")
                self.assertEqual(self.g.HERO_CREATIONS, creations)
                self.assertNotEqual(tx.HeroResults.native.Outcome, "SUICIDE")
                self.run_lua("assert(ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:13'))")
            if tx.Status == "COMPLETE":
                break
        self.assertTrue(saw_hero_wait)
        self.assertEqual(tx.Status, "COMPLETE")
        self.assertEqual(self.g.HERO_CREATIONS, 1)
        self.assertEqual(tx.HeroResults.native.Outcome, "RECONTRACT")
        self.assertTrue(self.verify("test-absorption", True))

    def test_published_hero_spread_mismatch_records_suicide_instead_of_blocking_other_assets(self):
        self.add_native_hero()
        self.split_unit_contexts()
        tx = self.apply()
        corrupted = False
        for _ in range(24):
            if self.g.HERO_CREATIONS and not corrupted:
                self.run_lua("""local u=assert(ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:13'))
                  u.spread=1""")
                corrupted = True
            self.run_lua("UI_PUBLISH()")
            tx = self.a.Resume("test-absorption")
            if tx.Status == "COMPLETE":
                break
        self.assertTrue(corrupted)
        self.assertEqual(tx.Status, "COMPLETE")
        self.assertEqual(tx.HeroResults.native.Outcome, "SUICIDE")
        self.assertEqual(tx.HeroResults.native.Cause, "HERO_RESTORE_ERROR")
        self.assertEqual(self.g.HERO_CREATIONS, 1)
        self.assertTrue(self.verify("test-absorption", True))

    def test_split_ui_getters_and_gameplay_mutations_preserve_native_heroes(self):
        self.split_hero_contexts()
        tx = self.apply()
        self.assertEqual(tx.Status, "WAITING_UI")
        spawns = self.g.SPAWNS
        with self.assertRaisesRegex(Exception, "did not complete"):
            self.verify("test-absorption", True)
        self.load_scripts()
        self.assertEqual(self.a.Resume("test-absorption").Status, "WAITING_UI")
        self.assertEqual(self.g.SPAWNS, spawns)
        for _ in range(3):
            self.run_lua("PUBLISH_HERO_UI()")
            tx = self.a.Resume("test-absorption")
        self.assertEqual(tx.Status, "COMPLETE")
        self.load_scripts()
        self.assertTrue(self.verify("test-absorption", True))
        self.run_lua("""local a=ExposedMembers.JAC_Absorption
          local u=assert(a.FindUnit(2,'test-absorption:unit:13'))
          local h=a.SnapshotUnit(u).Hero
          assert(h.ClaimOwner==2 and h.Origin.Owner==2 and h.Lifespan==17 and h.ActionCharges==2)
          assert(a.GetState().Transactions['test-absorption'].HeroResults.native.Outcome~='SUICIDE')""")

    def test_fresh_published_hero_mismatch_is_archived_without_accepting_stale_ui_as_success(self):
        self.split_hero_contexts()
        tx = self.apply()
        self.assertEqual(tx.Status, "WAITING_UI")
        self.run_lua("HERO_CLAIMS[0]=1;PUBLISH_HERO_UI()")
        tx = self.a.Resume("test-absorption")
        self.assertEqual(tx.Status, "WAITING_UI")
        self.run_lua("PUBLISH_HERO_UI()")
        tx = self.a.Resume("test-absorption")
        self.assertEqual(tx.Status, "COMPLETE")
        self.assertEqual(tx.HeroResults.native.Outcome, "SUICIDE")
        self.assertEqual(tx.HeroResults.native.Cause, "HERO_MANAGER_MISMATCH")
        self.assertTrue(self.verify("test-absorption", True))

    def test_ui_only_city_citizens_and_work_slots_wait_for_final_publication(self):
        self.split_hero_contexts()
        self.run_lua("""local full={}
          for x,c in pairs(CITIES) do
            full[x]={Religion=c.GetReligion,Buildings=c.GetBuildings}
            local r,b=c.GetReligion,c.GetBuildings
            c.GetReligion=function(self) local v=r(self);v.GetReligionsInCity=nil;return v end
            c.GetBuildings=function(self) local v=b(self);v.GetNumGreatWorkSlots=nil;v.GetGreatWorkInSlot=nil;return v end
          end
          UI_CITY_READS=0
          ExposedMembers.JAC_HeroReadAPI.ReadCity=function(owner,id,method,building,slot)
            UI_CITY_READS=UI_CITY_READS+1
            local c=assert(Players[owner]:GetCities():FindID(id));local f=full[c.x]
            if method=='RELIGIONS' then return f.Religion(c):GetReligionsInCity()
            elseif method=='SLOTS' then return f.Buildings(c):GetNumGreatWorkSlots(building)
            elseif method=='WORK' then return f.Buildings(c):GetGreatWorkInSlot(building,slot) end
            error('invalid test city query')
          end""")
        before = self.run_lua("return ExposedMembers.JAC_Absorption.Canonical(ExposedMembers.JAC_Absorption.CityReligion(CITIES[4]))")
        tx = self.apply()
        self.assertEqual(tx.Status, "WAITING_UI")
        for _ in range(4):
            self.run_lua("PUBLISH_HERO_UI()")
            tx = self.a.Resume("test-absorption")
        self.assertEqual(tx.Status, "COMPLETE")
        self.assertEqual(self.run_lua("return ExposedMembers.JAC_Absorption.Canonical(ExposedMembers.JAC_Absorption.CityReligion(CITIES[4]))"), before)
        self.assertGreater(self.g.UI_CITY_READS, 0)
        self.assertTrue(self.verify("test-absorption", True))

    def test_only_missing_work_slot_getter_still_requires_final_ui_publication(self):
        self.run_lua("""local full={}
          for x,c in pairs(CITIES) do
            local b=c.GetBuildings;full[x]=b
            c.GetBuildings=function(self) local v=b(self);v.GetGreatWorkInSlot=nil;return v end
          end
          UI_READY={}
          ExposedMembers.JAC_HeroReadAPI={
            GetObservation=function(token) return UI_READY[token] end,
            ReadCity=function(owner,id,method,building,slot)
              assert(method=='WORK')
              local c=assert(Players[owner]:GetCities():FindID(id))
              return full[c.x](c):GetGreatWorkInSlot(building,slot)
            end}
        """)
        tx = self.apply()
        self.assertEqual(tx.Status, "WAITING_UI")
        self.assertTrue(tx.AwaitingPreparation)
        self.g.UI_READY[tx.SourceReadyToken] = self.lua.table_from({"Published": True})
        tx = self.a.Resume("test-absorption")
        self.assertTrue(tx.AwaitingFinalUI)
        self.assertEqual(self.a.Resume("test-absorption").Status, "WAITING_UI")
        self.run_lua("UI_READY['test-absorption:final']={Published=true}")
        self.assertEqual(self.a.Resume("test-absorption").Status, "COMPLETE")
        request = self.a.RequestVerification('test-absorption')
        self.g.UI_READY[request.Token] = self.lua.table_from({"Published": True})
        self.assertTrue(self.a.Verify("test-absorption", True))

    def test_native_heroes_keep_identity_spent_charges_renown_origin_and_person_record(self):
        self.add_native_hero(second=True)
        self.apply()
        self.assertEqual(self.g.HERO_CREATIONS, 2)
        self.assertTrue(self.verify('test-absorption', True))
        self.run_lua('''
          local a=ExposedMembers.JAC_Absorption;local u=a.FindUnit(2,'test-absorption:unit:13')
          assert(u:IsHero() and u:GetHeroClassType()==0 and u:GetProperty(UnitPropertyKeys.LifespanRemaining)==17 and u:GetActionCharges()==2 and u:GetRenown()==13)
          assert(u:GetDamage()==7 and u:GetMovesRemaining()==1)
          assert(a.FindUnit(2,'test-absorption:unit:14'):GetActionCharges()==0)
          assert(HERO_CLAIMS[0]==2 and HERO_CLAIMS[1]==2)
          assert(HERO_ORIGINS[0].player==2 and HERO_ORIGINS[0].id==CITIES[4]:GetID())
          assert(HERO_ORIGINS[1].player==2 and HERO_ORIGINS[1].id==CITIES[8]:GetID())
          local r=a.GetRoster(2).native
          assert(r.Unit.ID==u:GetID() and r.Contract=='RECONTRACT' and r.Loyalty==71 and r.Relations.friend==29)
          assert(r.NativeHero.ClassType=='HEROCLASS_JAC_WARLORD_A' and r.NativeHero.ActionCharges==2 and r.NativeHero.Origin.Owner==1)
          assert(a.GetRoster(2)['native-person:1:14'].Kind=='HERO')
          assert(u:GetProperty('JAC_ABSORPTION_PERSON_V1').Owner==2)
        ''')
        before = self.snapshot()
        self.load_scripts()
        self.assertTrue(self.verify('test-absorption', True))
        self.apply()
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(self.g.HERO_CREATIONS, 2)

    def test_native_hero_absent_gameplay_manager_becomes_suicide_history(self):
        self.add_native_hero()
        self.run_lua('Game.GetHeroesManager=function() return nil end')
        self.assert_native_suicide('NATIVE_MANAGER_UNAVAILABLE')

    def test_dormant_native_claim_without_unit_is_archived_as_suicide(self):
        self.run_lua("HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=CITIES[4].id}")
        tx = self.apply()
        self.assertEqual(tx.Status, 'COMPLETE')
        self.run_lua('''local a=ExposedMembers.JAC_Absorption
          local r=a.GetState().Archive['native-hero:1:HEROCLASS_JAC_WARLORD_A']
          assert(r and r.Outcome=='SUICIDE' and r.SuicideCause=='DORMANT_CLAIM')
          assert(a.GetState().Transactions['test-absorption'].HeroResults[r.ID].Outcome=='SUICIDE')
          assert(r.NativeHero.ClaimOwner==1 and r.NativeHero.Origin.X==4 and r.NativeUnit==nil)
          assert(r.Unit==nil and a.GetRoster(2)[r.ID]==nil and a.Verify('test-absorption',true))
        ''')

    def test_dormant_claim_with_stale_person_reference_preserves_requested_outcome(self):
        self.run_lua('''
          HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=CITIES[4].id}
          Players[1].props.JAC_ABSORPTION_ASSETS_V1.dormant={ID='dormant',Kind='HERO',Owner=1,
            Unit={Owner=1,ID=777},NativeHero={Class=0,ClassType='HEROCLASS_JAC_WARLORD_A',Lifespan=9},
            Contract='PROTECT',Loyalty=74,Relations={friend=31}}
          PLAN.Assets.dormant='PROTECT'
        ''')
        self.apply()
        self.run_lua('''local a=ExposedMembers.JAC_Absorption
          local r=a.GetState().Archive.dormant
          assert(r.Outcome=='SUICIDE' and r.RequestedOutcome=='PROTECT' and r.SuicideCause=='DORMANT_CLAIM')
          assert(r.FormerUnit.ID==777 and not r.Unit and r.Contract=='PROTECT' and r.Relations.friend==31)
          assert(r.NativeHero.ClaimOwner==1 and r.NativeHero.Origin.X==4)
          assert(r.HistoricalNativeHero.Lifespan==9)
        ''')

    def test_native_hero_missing_origin_city_becomes_suicide(self):
        self.add_native_hero()
        self.run_lua('HERO_ORIGINS[0].id=9999')
        self.assert_native_suicide('ORIGIN_UNAVAILABLE')

    def test_native_hero_missing_origin_building_becomes_suicide(self):
        self.add_native_hero()
        self.run_lua('CITIES[4].monument=false')
        self.assert_native_suicide('ORIGIN_BUILDING_UNAVAILABLE')

    def test_native_heroes_sharing_origin_archive_the_second_claim(self):
        self.add_native_hero(second=True)
        self.run_lua('HERO_ORIGINS[1].id=CITIES[4].id')
        self.apply()
        self.run_lua('''local a=ExposedMembers.JAC_Absorption
          assert(a.GetRoster(2).native.Unit and a.GetState().Archive['native-person:1:14'].SuicideCause=='HERO_ORIGIN_COLLISION')
        ''')

    def test_native_hero_no_spawn_after_claim_change_finishes_as_suicide(self):
        self.add_native_hero()
        self.g.HERO_NO_SPAWN = True
        self.assert_native_suicide('HERO_CREATE_NO_UNIT')
        self.assertEqual(self.g.HERO_CREATIONS, 1)
        self.assertEqual(self.g.HERO_CLAIMS[0], 2)
        self.assertEqual(self.a.GetState().Archive.native.NativeClaimAfter, 2)

    def test_native_hero_init_throw_after_creation_adopts_candidate_without_recreating(self):
        self.add_native_hero()
        self.g.HERO_THROW_AFTER_SPAWN = True
        self.apply()
        self.assertTrue(self.verify('test-absorption', True))
        self.assertEqual(self.g.HERO_CREATIONS, 1)

    def test_native_hero_token_write_failure_kills_only_its_candidate(self):
        self.add_native_hero()
        self.run_lua('''
          local create=HERO_MANAGER.CreateHero
          HERO_MANAGER.CreateHero=function(...)
            create(...)
            local u=Players[2].units[NEXT_ID-1];local set=u.SetProperty
            u.SetProperty=function(self,key,value)
              if key=='JAC_ABSORPTION_UNIT_TOKEN_V1' and not TOKEN_FAILED then TOKEN_FAILED=true;error('token interrupted') end
              set(self,key,value)
            end
          end
        ''')
        self.assert_native_suicide('HERO_TOKEN_ERROR')
        self.assertEqual(self.g.HERO_CREATIONS, 1)
        self.run_lua("assert(#(function() local a={};for _,u in Players[2]:GetUnits():Members() do if u:IsHero() then a[#a+1]=u end end;return a end)()==0)")

    def test_native_hero_wrong_origin_is_suicide_without_duplicate(self):
        self.add_native_hero()
        self.g.HERO_WRONG_ORIGIN = True
        self.assert_native_suicide('HERO_MANAGER_MISMATCH')
        self.assertEqual(self.g.HERO_CREATIONS, 1)

    def test_native_hero_city_failure_can_resume_from_escrow_without_second_kill(self):
        self.add_native_hero()
        self.g.FAIL_CITY = True
        with self.assertRaisesRegex(Exception, 'city failed'):
            self.apply()
        self.assertEqual((self.g.KILLS, self.g.HERO_CREATIONS), (1, 0))
        self.g.FAIL_CITY = False
        self.load_scripts()
        self.a.Resume('test-absorption')
        self.assertTrue(self.verify('test-absorption', True))
        self.assertEqual((self.g.KILLS, self.g.HERO_CREATIONS), (6, 1))

    def test_native_hero_manager_mismatch_is_not_reported_as_inheritance(self):
        self.add_native_hero()
        self.g.HERO_WRONG_OWNER = True
        self.assert_native_suicide('HERO_MANAGER_MISMATCH')
        self.assertEqual(self.g.HERO_CREATIONS, 1)

    def test_hero_creation_error_does_not_kill_a_third_party_unit(self):
        self.add_native_hero()
        self.run_lua('''HERO_MANAGER.CreateHero=function(_,owner,class,x,y)
          HERO_CREATIONS=HERO_CREATIONS+1;HERO_CLAIMS[class]=3
          UNIT(3,777,'UNIT_HERO_JAC_WARLORD_A')
          error('third party candidate appeared')
        end''')
        self.assert_native_suicide('HERO_CREATE_ERROR')
        self.run_lua("assert(Players[3].units[777] and Players[3].units[777]:IsHero())")

    def test_reused_donor_id_blocks_without_killing_an_unrelated_unit(self):
        self.add_native_hero()
        self.run_lua('''local spawn=UnitManager.InitUnitValidAdjacentHex
          UnitManager.InitUnitValidAdjacentHex=function(...)
            if not DONOR_REPLACED then
              DONOR_REPLACED=true
              Players[1].units[13]=nil
              UNIT(1,13,'UNIT_BUILDER')
            end
            return spawn(...)
          end''')
        with self.assertRaisesRegex(Exception, 'native Hero changed before escrow'):
            self.apply()
        self.run_lua("assert(Players[1].units[13] and not Players[1].units[13]:IsHero())")

    def test_native_hero_restore_error_becomes_suicide_without_losing_other_assets(self):
        self.add_native_hero()
        self.run_lua('''
          local create=HERO_MANAGER.CreateHero
          HERO_MANAGER.CreateHero=function(...)
            create(...)
            local u=Players[2].units[NEXT_ID-1]
            u.ChangeActionCharges=function() error('hero charges unavailable') end
          end
        ''')
        self.assert_native_suicide('HERO_RESTORE_ERROR')
        self.run_lua('''local a=ExposedMembers.JAC_Absorption
          assert(a.GetRoster(2).equipment and a.GetRoster(2).artifact and a.GetRoster(2).person)
        ''')

    def test_native_hero_recall_binds_new_id_and_person_after_reload(self):
        self.add_native_hero()
        self.apply()
        self.run_lua('''
          local a=ExposedMembers.JAC_Absorption
          local old=a.FindUnit(2,'test-absorption:unit:13')
          OLD_HERO_ID=old:GetID();UnitManager.Kill(old)
          assert(a.Verify('test-absorption',false))
          a.BeginHeroRecall('recall:1','test-absorption','native')
        ''')
        self.load_scripts()
        with self.assertRaisesRegex(Exception, 'exactly one recalled'):
            self.a.CompleteHeroRecall('recall:1')
        self.run_lua('HERO_MANAGER:CreateHero(2,0,4,0)')
        record = self.a.CompleteHeroRecall('recall:1')
        self.assertNotEqual(record.Unit.ID, self.g.OLD_HERO_ID)
        self.run_lua('''local a=ExposedMembers.JAC_Absorption
          local r=a.GetRoster(2).native;local u=a.FindUnit(2,'recall:1:unit')
          assert(r.Unit.ID==u:GetID() and r.Unit.Token=='recall:1:unit' and r.FormerUnit.ID==OLD_HERO_ID)
          assert(r.Contract=='RECONTRACT' and r.Loyalty==71 and r.Relations.friend==29)
          assert(u:GetProperty('JAC_ABSORPTION_PERSON_V1').Unit.ID==u:GetID())
          assert(#r.RecallHistory==1 and a.Verify('test-absorption',false))
        ''')
        self.load_scripts()
        self.a.CompleteHeroRecall('recall:1')
        self.assertTrue(self.verify('test-absorption', False))
        self.assertEqual(len(self.a.GetRoster(2).native.RecallHistory), 1)

    def test_recall_after_second_absorption_preserves_original_owner_and_uses_current_mapping(self):
        self.add_native_hero()
        self.apply()
        self.run_lua('''
          local a=ExposedMembers.JAC_Absorption
          local p={ID='second-absorption',From=2,To=3,Mode='MERGER',EntryConditionsMet=true,
            RelationsResolved=true,Leader='RECRUIT',Units={},Assets={}}
          for _,u in Players[2]:GetUnits():Members() do
            local s=a.SnapshotUnit(u)
            p.Units[a.Rules.UnitKey(2,u:GetID())]=s.Kind=='UNIT' and 'INCORPORATE' or 'RECRUIT'
          end
          for id,r in pairs(a.GetRoster(2)) do
            p.Assets[id]=r.Kind=='ITEM' and 'TRANSFER' or r.Kind=='UNLOCK' and 'INHERIT'
              or r.Kind=='RELIGION' and 'PRESERVE' or 'RECRUIT'
          end
          a.Apply(p)
          local r=a.GetRoster(3).native
          assert(r.OriginalOwner==1 and r.Owner==3)
          UnitManager.Kill(a.FindUnit(3,r.Unit.Token))
        ''')
        self.load_scripts()
        self.a.BeginHeroRecall('second-recall', 'second-absorption', 'native')
        self.run_lua("HERO_MANAGER:CreateHero(3,0,4,0)")
        self.a.CompleteHeroRecall('second-recall')
        self.run_lua('''
          local a=ExposedMembers.JAC_Absorption;local r=a.GetRoster(3).native
          local u=assert(a.FindUnit(3,r.Unit.Token))
          assert(r.Owner==3 and r.OriginalOwner==1 and r.Unit.ID==u:GetID())
          assert(r.Contract=='RECONTRACT' and r.Loyalty==71 and r.Relations.friend==29)
          assert(u:GetProperty(a.Rules.PERSON_KEY).OriginalOwner==1)
        ''')
        self.load_scripts()
        self.a.CompleteHeroRecall('second-recall')
        self.assertEqual(len(self.a.GetRoster(3).native.RecallHistory), 1)

    def test_recall_rejects_candidate_and_token_collisions_without_killing_them(self):
        self.add_native_hero()
        self.apply()
        self.run_lua('''local a=ExposedMembers.JAC_Absorption
          UnitManager.Kill(a.FindUnit(2,'test-absorption:unit:13'))
          a.BeginHeroRecall('recall:collision','test-absorption','native')
          UNIT(3,777,'UNIT_HERO_JAC_WARLORD_A')
          UNIT(2,778,'UNIT_HERO_JAC_WARLORD_A')
          UNIT(2,779,'UNIT_HERO_JAC_WARLORD_A')
        ''')
        with self.assertRaisesRegex(Exception, 'exactly one recalled'):
            self.a.CompleteHeroRecall('recall:collision')
        self.run_lua('''
          assert(Players[3].units[777] and Players[2].units[778] and Players[2].units[779])
          Players[2].units[779]=nil
          Players[2].units[778]:SetProperty('JAC_ABSORPTION_UNIT_TOKEN_V1','someone-else')
        ''')
        with self.assertRaisesRegex(Exception, 'another transaction'):
            self.a.CompleteHeroRecall('recall:collision')
        self.run_lua('''assert(Players[3].units[777] and Players[2].units[778])''')

    def test_recall_resumes_after_token_write_and_does_not_duplicate_history(self):
        self.add_native_hero()
        self.apply()
        self.run_lua('''local a=ExposedMembers.JAC_Absorption
          UnitManager.Kill(a.FindUnit(2,'test-absorption:unit:13'))
          a.BeginHeroRecall('recall:retry','test-absorption','native')
          HERO_MANAGER:CreateHero(2,0,4,0)
          local u=Players[2].units[NEXT_ID-1];local set=u.SetProperty
          u.SetProperty=function(self,key,value)
            set(self,key,value)
            if key=='JAC_ABSORPTION_UNIT_TOKEN_V1' and not RECALL_TOKEN_THROWN then
              RECALL_TOKEN_THROWN=true;error('recall token interrupted')
            end
          end
        ''')
        with self.assertRaisesRegex(Exception, 'recall token interrupted'):
            self.a.CompleteHeroRecall('recall:retry')
        self.load_scripts()
        record = self.a.CompleteHeroRecall('recall:retry')
        self.assertEqual(len(record.RecallHistory), 1)
        self.assertEqual(record.Unit.Token, 'recall:retry:unit')
        self.assertEqual(len(self.a.CompleteHeroRecall('recall:retry').RecallHistory), 1)
        self.assertEqual(self.g.HERO_CREATIONS, 2)

    def test_native_hero_checks_detect_corruption_instead_of_only_testing_unit_count(self):
        for corruption, message in (
            ('u.lifespan=18', 'lifespan changed'), ('u.charges=6', 'action charges changed'),
            ('u.renown=0', 'renown changed'), ('HERO_CLAIMS[0]=1', 'manager owner changed'),
            ('HERO_ORIGINS[0]={player=2,id=CITIES[12].id}', 'recall origin changed'),
            ("UNIT(2,999,'UNIT_HERO_JAC_WARLORD_A')", 'duplicate native Hero'),
        ):
            with self.subTest(corruption=corruption):
                self.setUp()
                self.add_native_hero()
                self.apply()
                self.run_lua("local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:13');" + corruption)
                with self.assertRaisesRegex(Exception, message):
                    self.verify('test-absorption', True)

    def test_native_lifespan_changes_after_turn_but_owner_recall_identity_must_remain(self):
        self.add_native_hero()
        self.apply()
        self.g.CURRENT_TURN = 6
        self.run_lua("ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:13').lifespan=16")
        self.load_scripts()
        self.assertTrue(self.verify('test-absorption', False))
        with self.assertRaisesRegex(Exception, 'lifespan changed'):
            self.verify('test-absorption', True)
        self.run_lua('HERO_CLAIMS[0]=1')
        with self.assertRaisesRegex(Exception, 'manager owner changed'):
            self.verify('test-absorption', False)

    def test_gameplay_scenario_checks_use_the_context_aware_spread_snapshot(self):
        for name in ('absorption-japan.json', 'absorption-hero-recall.json'):
            data = json.loads((ROOT / 'tools/civ6-autotest/scenarios' / name).read_text(encoding='utf-8'))
            chunks = [s['code'] for s in data['steps'] if s.get('state_name') == 'GameCore_Tuner']
            self.assertGreater(len(chunks), 0)
            for code in chunks:
                self.assertNotIn(':GetSpreadCharges(', code)

    def test_all_gameplay_scenario_chunks_use_shared_lifespan_key(self):
        for name in ('absorption-japan.json', 'absorption-hero-recall.json'):
            data = json.loads((ROOT / 'tools/civ6-autotest/scenarios' / name).read_text(encoding='utf-8'))
            for step in data['steps']:
                if step.get('state_name') == 'GameCore_Tuner':
                    code = step.get('code', '')
                    self.assertNotIn('UnitPropertyKeys.LifespanRemaining', code.replace('(UnitPropertyKeys and UnitPropertyKeys.LifespanRemaining)', ''), name)
            self.assertIn('ExposedMembers.JAC_HeroLifespanKey', ''.join(step.get('code', '') for step in data['steps']))

    def test_ui_publishes_engine_enum_when_gameplay_has_no_unit_property_keys(self):
        self.add_native_hero()
        ui = (ROOT / "poc/JAC_PoC_Absorption/UI/JAC_Absorption_HeroKeys.lua").read_text(encoding="utf-8")
        self.run_lua('''
          NATIVE_TEST_LIFE_KEY=UnitPropertyKeys.LifespanRemaining
          UnitPropertyKeys={LifespanRemaining=-1318296071}
          Events={LoadScreenClose={Add=function(f) HERO_KEYS_ON_LOAD=f end},GameCoreEventPublishComplete={Add=function() end}}
        ''')
        self.run_lua(ui)
        self.assertEqual(self.g.ExposedMembers.JAC_HeroLifespanKey, -1318296071)
        self.run_lua('''
          UnitPropertyKeys=nil
          local u=Players[1].units[13]
          u.GetProperty=function(self,key) if key==-1318296071 then return self.lifespan end;return self.props[key] end
          local snap=ExposedMembers.JAC_Absorption.SnapshotUnit(u)
          assert(snap.Hero.LifespanKey==-1318296071 and snap.Hero.Lifespan==17)
        ''')

    def test_shipped_lifespan_property_is_used_without_legacy_lifespan_or_renown_methods(self):
        self.add_native_hero()
        self.run_lua('''
          Players[1].units[13].GetRenown=nil;Players[1].units[13].SetRenown=nil
          Players[1].props.JAC_ABSORPTION_ASSETS_V1.native.Renown=13
          local create=HERO_MANAGER.CreateHero
          HERO_MANAGER.CreateHero=function(...)
            create(...);local u=Players[2].units[NEXT_ID-1];u.GetRenown=nil;u.SetRenown=nil
          end
        ''')
        self.apply()
        self.run_lua('''
          local a=ExposedMembers.JAC_Absorption;local u=a.FindUnit(2,'test-absorption:unit:13')
          assert(u:GetProperty(UnitPropertyKeys.LifespanRemaining)==17 and u:GetActionCharges()==2)
          local r=a.GetRoster(2).native;assert(r.Renown==13 and r.NativeHero.RenownUnavailable and r.NativeHero.Renown==nil)
          assert(u.GetLifespan==nil and u.SetLifespan==nil and u.GetRenown==nil and u.SetRenown==nil)
        ''')
        self.assertTrue(self.verify('test-absorption', True))

    def test_native_hero_archival_and_explicit_suicide_preserve_requested_outcome(self):
        for outcome in ('HIDE', 'DEFECT', 'SUICIDE'):
            with self.subTest(outcome=outcome):
                self.setUp()
                self.add_native_hero()
                self.g.NATIVE_OUTCOME = outcome
                self.run_lua("PLAN.Units['1:13']=NATIVE_OUTCOME;PLAN.Assets.native=NATIVE_OUTCOME")
                self.assert_native_suicide('EXPLICIT_SUICIDE' if outcome == 'SUICIDE'
                                           else 'REQUESTED_OUTCOME_UNTRANSFERABLE', outcome)

    def test_native_hero_position_is_restored_separately_from_its_recall_origin(self):
        self.add_native_hero()
        self.run_lua('Players[1].units[13].x=10')
        self.apply()
        self.run_lua('''local a=ExposedMembers.JAC_Absorption;local u=a.FindUnit(2,'test-absorption:unit:13')
          assert(u:GetX()==10 and u:GetY()==0 and u:GetDamage()==7 and u:GetMovesRemaining()==1)
          assert(HERO_ORIGINS[0].id==CITIES[4].id and HERO_ORIGINS[0].player==2)
        ''')
        self.assertTrue(self.verify('test-absorption', True))

    def test_native_hero_unrestorable_exposed_renown_becomes_suicide(self):
        self.add_native_hero()
        self.run_lua('Players[1].units[13].SetRenown=nil')
        self.assert_native_suicide('RENOWN_UNRESTORABLE')

    def test_native_hero_is_not_escrowed_when_an_ordinary_unit_cannot_preserve_spent_charges(self):
        self.add_native_hero()
        self.run_lua("local u=UNIT(1,15,'UNIT_BUILDER');u.build=1;PLAN.Units['1:15']='INCORPORATE'")
        with self.assertRaisesRegex(Exception, 'spent build charges'):
            self.apply()
        self.assertEqual((self.g.HERO_CREATIONS, self.g.KILLS, self.g.TRANSFERS), (0, 0, 0))
        self.assertIsNotNone(self.g.Players[1].units[13])

    def test_native_hero_duplicate_at_absorber_survives_suicide(self):
        self.add_native_hero()
        self.run_lua("UNIT(2,999,'UNIT_HERO_JAC_WARLORD_A')")
        self.assert_native_suicide('HERO_CLASS_COLLISION')
        self.run_lua("assert(Players[2].units[999] and Players[2].units[999]:IsHero())")

    def test_native_hero_city_transfer_throw_after_mutation_resumes_with_the_new_city_id(self):
        self.add_native_hero()
        self.run_lua('''
          local transfer=CityManager.TransferCity
          CityManager.TransferCity=function(city,owner)
            transfer(city,owner)
            if not CITY_THROWN then CITY_THROWN=true;error('city transfer interrupted after mutation') end
          end
        ''')
        with self.assertRaisesRegex(Exception, 'interrupted after mutation'):
            self.apply()
        self.load_scripts()
        self.a.Resume('test-absorption')
        self.assertTrue(self.verify('test-absorption', True))
        self.assertEqual((self.g.TRANSFERS, self.g.HERO_CREATIONS), (2, 1))

    def test_native_hero_token_set_then_throw_becomes_suicide(self):
        self.add_native_hero()
        self.run_lua('''
          local create=HERO_MANAGER.CreateHero
          HERO_MANAGER.CreateHero=function(...)
            create(...);local u=Players[2].units[NEXT_ID-1];local set=u.SetProperty
            u.SetProperty=function(self,key,value)
              set(self,key,value)
              if key=='JAC_ABSORPTION_UNIT_TOKEN_V1' and not TOKEN_THROWN then TOKEN_THROWN=true;error('after token write') end
            end
          end
        ''')
        self.assert_native_suicide('HERO_TOKEN_ERROR')
        self.assertEqual(self.g.HERO_CREATIONS, 1)

    def test_engine_spawn_failure_preserves_every_donor_unit_and_city_then_resume_is_idempotent(self):
        self.g.FAIL_SPAWN = 2
        with self.assertRaisesRegex(Exception, "did not create"):
            self.apply()
        self.assertEqual((self.g.KILLS, self.g.TRANSFERS), (0, 0))
        self.assertEqual(self.a.GetState().Transactions["test-absorption"].Status, "BLOCKED")
        self.g.FAIL_SPAWN = None
        self.load_scripts()  # no session-local transaction cache can help recovery
        self.a.Resume("test-absorption")
        self.assertTrue(self.verify("test-absorption", True))
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (7, 5, 2))

    def test_property_copy_failure_is_blocked_before_removing_any_donor(self):
        self.g.FAIL_COPY = True
        with self.assertRaisesRegex(Exception, "copy failed"):
            self.apply()
        self.assertEqual((self.g.KILLS, self.g.TRANSFERS), (0, 0))
        self.g.FAIL_COPY = False
        self.a.Resume("test-absorption")
        self.assertEqual(self.g.SPAWNS, 6)

    def test_spent_build_charges_need_an_adapter_and_never_destroy_the_donor(self):
        self.run_lua("local u=UNIT(1,13,'UNIT_BUILDER');u.build=1;PLAN.Units['1:13']='INCORPORATE'")
        with self.assertRaisesRegex(Exception, "spent build charges"):
            self.apply()
        self.assertEqual((self.g.KILLS, self.g.TRANSFERS), (0, 0))
        self.assertIsNotNone(self.g.Players[1].units[13])

    def test_city_failure_retains_snapshots_and_resume_does_not_recreate_units(self):
        self.g.FAIL_CITY = True
        with self.assertRaisesRegex(Exception, "city failed"):
            self.apply()
        self.assertEqual(self.g.SPAWNS, 6)
        self.assertEqual(len(self.a.GetState().Transactions["test-absorption"].Cities), 2)
        self.g.FAIL_CITY = False
        self.a.Resume("test-absorption")
        self.assertEqual(self.g.SPAWNS, 6)
        self.assertTrue(self.verify("test-absorption", True))

    def test_later_turn_resume_is_rejected_without_reapplying_old_damage_or_movement(self):
        self.g.FAIL_CITY = True
        with self.assertRaises(Exception):
            self.apply()
        self.g.CURRENT_TURN = 6
        before = self.snapshot()
        with self.assertRaisesRegex(Exception, "original turn"):
            self.a.Resume("test-absorption")
        self.assertEqual(self.snapshot(), before)

    def test_same_transaction_id_with_changed_decisions_is_rejected(self):
        self.apply()
        self.run_lua("PLAN.Assets.hero='HIDE'")
        with self.assertRaisesRegex(Exception, "different plan"):
            self.apply()

    def test_verification_catches_missing_or_corrupted_transfers_instead_of_only_counting(self):
        self.apply()
        self.run_lua("ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:8').individual=43")
        with self.assertRaisesRegex(Exception, "individual changed"):
            self.verify("test-absorption", True)

    def test_completed_transaction_is_not_replayed_after_later_turns(self):
        self.apply()
        self.g.CURRENT_TURN = 6
        self.run_lua("ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3').damage=0")
        self.apply()
        self.assertTrue(self.verify("test-absorption", False))
        self.assertEqual(self.g.SPAWNS, 6)

    def test_registered_unit_and_city_properties_are_preserved_and_unsupported_values_rejected(self):
        self.a.RegisterUnitProperty("FOREIGN_UNIT")
        self.a.RegisterCityProperty("FOREIGN_CITY")
        self.run_lua("Players[1].units[3]:SetProperty('FOREIGN_UNIT',{mission='HOLD',endTurn=20});CITIES[4]:SetProperty('FOREIGN_CITY',{castle='KEEP'})")
        self.apply()
        self.run_lua("assert(ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3'):GetProperty('FOREIGN_UNIT').endTurn==20);assert(CITIES[4]:GetProperty('FOREIGN_CITY').castle=='KEEP')")

    def test_adapter_preflight_apply_verify_and_reload_contract(self):
        self.run_lua('''ADAPTER_APPLIED=0
          ExposedMembers.JAC_Absorption.RegisterAdapter('religion-interface',{
            Prepare=function(tx) return {Founder=tx.From,Followers=11} end,
            Apply=function(tx,data) Game:SetProperty('ADAPTER_FAITH',data);ADAPTER_APPLIED=ADAPTER_APPLIED+1 end,
            Verify=function(tx,data) assert(JAC_AbsorptionRules.Equal(Game:GetProperty('ADAPTER_FAITH'),data)) end})''')
        self.apply()
        self.assertEqual(self.g.ADAPTER_APPLIED, 1)
        self.load_scripts()
        with self.assertRaisesRegex(Exception, "re-register"):
            self.verify("test-absorption", True)

    def test_bad_adapter_and_empty_string_fail_before_native_mutation(self):
        self.run_lua("ExposedMembers.JAC_Absorption.RegisterAdapter('bad',{Prepare=function() return {} end,Apply=function() end,Verify=function() end});Players[1].props.JAC_ABSORPTION_ASSETS_V1.hero.Name=''")
        with self.assertRaisesRegex(Exception, "empty strings"):
            self.apply()
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (0, 0, 0))

    def register_mission_rebind(self):
        self.run_lua('''
          ORIGINAL_MISSION={Actor={Owner=1,ID=3},Target={Owner=3,ID=77},ReadyTurn=20,FixedRoll=17}
          Players[1].units[3]:SetProperty('FOREIGN_MISSION',ORIGINAL_MISSION)
          REBIND_CALLS=0
          ExposedMembers.JAC_Absorption.RegisterUnitProperty('FOREIGN_MISSION',function(value,tx,snapshot)
            if not value then return nil end
            REBIND_CALLS=REBIND_CALLS+1
            if value.Actor.Owner==tx.From then
              local mapped=assert(tx.UnitMap[JAC_AbsorptionRules.UnitKey(value.Actor.Owner,value.Actor.ID)])
              value.Actor={Owner=tx.To,ID=mapped.ID}
            end
            return value
          end)
        ''')

    def test_foreign_reference_rebind_keeps_target_deadline_and_fixed_roll_and_survives_reload(self):
        self.register_mission_rebind()
        self.apply()
        self.run_lua('''
          local a=ExposedMembers.JAC_Absorption;local u=a.FindUnit(2,'test-absorption:unit:3')
          local mission=u:GetProperty('FOREIGN_MISSION')
          assert(mission.Actor.Owner==2 and mission.Actor.ID==u:GetID())
          assert(mission.Target.Owner==3 and mission.Target.ID==77 and mission.ReadyTurn==20 and mission.FixedRoll==17)
          assert(ORIGINAL_MISSION.Actor.Owner==1 and ORIGINAL_MISSION.Actor.ID==3)
          local tx=a.GetState().Transactions['test-absorption']
          assert(tx.Units[1].Properties.FOREIGN_MISSION.Actor.Owner==1)
          assert(tx.Units[1].TransferredProperties.FOREIGN_MISSION.Actor.Owner==2)
        ''')
        self.load_scripts()
        self.assertTrue(self.verify("test-absorption", True))
        self.apply()
        self.assertEqual(self.g.REBIND_CALLS, 1)

    def test_saved_rebound_expectation_is_used_on_resume_without_reloading_callback(self):
        self.register_mission_rebind()
        self.g.FAIL_COPY = True
        with self.assertRaisesRegex(Exception, "copy failed"):
            self.apply()
        self.assertEqual((self.g.KILLS, self.g.TRANSFERS), (0, 0))
        self.load_scripts()
        self.g.FAIL_COPY = False
        self.a.Resume("test-absorption")
        self.assertTrue(self.verify("test-absorption", True))
        self.run_lua("local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3');assert(u:GetProperty('FOREIGN_MISSION').Actor.ID==u:GetID())")
        self.assertEqual(self.g.REBIND_CALLS, 1)

    def test_corrupted_rebound_reference_is_detected_by_generic_verification(self):
        self.register_mission_rebind()
        self.apply()
        self.run_lua("ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3'):GetProperty('FOREIGN_MISSION').Actor.Owner=1")
        with self.assertRaisesRegex(Exception, "unit properties changed"):
            self.verify("test-absorption", True)

    def test_city_property_rebind_uses_saved_expectations_on_reload(self):
        self.run_lua('''
          CITIES[4]:SetProperty('FOREIGN_CITY_OWNER',{Custodian=1,Text='scripture'})
          ExposedMembers.JAC_Absorption.RegisterCityProperty('FOREIGN_CITY_OWNER',function(value,tx)
            if value then value.Custodian=tx.To end;return value
          end)
        ''')
        self.apply()
        self.assertEqual(self.g.CITIES[4].props.FOREIGN_CITY_OWNER.Custodian, 2)
        self.load_scripts()
        self.assertTrue(self.verify("test-absorption", True))

    def test_nonserializable_property_rebind_blocks_before_destroying_donors(self):
        self.run_lua("ExposedMembers.JAC_Absorption.RegisterUnitProperty('BAD_REBIND',function() return function() end end)")
        with self.assertRaisesRegex(Exception, "serializable"):
            self.apply()
        self.assertEqual((self.g.KILLS, self.g.TRANSFERS), (0, 0))

    def test_native_hero_detected_by_engine_flag_with_a_nonstandard_type_is_rejected(self):
        self.assert_rejected_unchanged("local u=UNIT(1,13,'UNIT_WARRIOR');u.IsHero=function() return true end;PLAN.Units['1:13']='INCORPORATE'", "native Hero")

    def test_missing_native_great_person_charge_api_blocks_before_any_mutation(self):
        self.assert_rejected_unchanged(
            "Players[1].units[8].GetGreatPerson=function() return {GetIndividual=function() return 42 end} end",
            "native great-person charges unavailable")

    def test_spent_native_great_person_charges_do_not_reset_on_transfer(self):
        self.run_lua("Players[1].units[8].gpcharges=1")
        with self.assertRaisesRegex(Exception, "spent great-person charges"):
            self.apply()
        self.assertEqual((self.g.KILLS, self.g.TRANSFERS), (0, 0))
        self.assertEqual(self.g.Players[1].units[8].gpcharges, 1)

    def test_missing_staged_replacement_blocks_instead_of_breaking_saved_references(self):
        self.register_mission_rebind()
        self.g.FAIL_COPY = True
        with self.assertRaisesRegex(Exception, "copy failed"):
            self.apply()
        self.run_lua("local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3');Players[2].units[u:GetID()]=nil")
        self.g.FAIL_COPY = False
        before = (self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS)
        with self.assertRaisesRegex(Exception, "staged replacement was lost"):
            self.a.Resume("test-absorption")
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), before)

    def live_scenario_fixture(self):
        data = json.loads((ROOT / "tools/civ6-autotest/scenarios/absorption-japan.json").read_text(encoding="utf-8"))
        steps = [step for step in data["steps"] if step["action"] == "ui_lua" and step["state_name"] == "GameCore_Tuner" and not step.get("capture_as", "").startswith("JAC_ABSORPTION_SYNC_")]
        self.run_lua('''
          ExposedMembers.JAC_HeroLifespanKey=UnitPropertyKeys.LifespanRemaining
          local majors={};for id=0,21 do
            local n=id;majors[#majors+1]={GetID=function() return n end}
          end
          PlayerManager={GetAliveMajors=function() return majors end}
          local makeUnit=UNIT
          UNIT=function(...)
            local u=makeUnit(...)
            if u:IsHero() then u.GetRenown=nil;u.SetRenown=nil end
            return u
          end
          for id=0,21 do
            local n=id
            PlayerConfigurations[id]={GetLeaderTypeName=function() return 'LEADER_TEST_'..n end,
              GetCivilizationTypeName=function() return n==1 and 'CIVILIZATION_JAC_ASHINA' or n==2 and 'CIVILIZATION_JAC_DATE' or 'CIVILIZATION_JAC_ODA' end}
          end
          local founder,beliefs,holyCities={},{},{}
          GameInfo.Beliefs={BELIEF_DIVINE_INSPIRATION={Index=3},BELIEF_CATHEDRAL={Index=7}}
          for id,p in pairs(Players) do local n=id;p.GetReligion=function() return {
            GetReligionTypeCreated=function() return founder[n] or -1 end,
            SetHolyCity=function(_,city) holyCities[n]=city:GetID() end,
            ChangeFaithBalance=function(_,v) FAITH[n]=FAITH[n]+v end} end end
          Game.GetReligion=function() return {FoundReligion=function(_,owner,id) assert(holyCities[owner]);founder[owner]=id;beliefs[owner]={} end,
            AddBelief=function(_,owner,id) table.insert(beliefs[owner],id) end,
            GetReligions=function() local list={};for owner,id in pairs(founder) do list[#list+1]={Religion=id,Founder=owner,Beliefs=beliefs[owner]} end;return list end} end
          GameInfo.Religions.RELIGION_CUSTOM_1={Index=5,ReligionType='RELIGION_TEST'}
          local individual={Index=42,GreatPersonClassType='GREAT_PERSON_CLASS_GENERAL'}
          GameInfo.GreatPersonIndividuals=setmetatable({[42]=individual},{__call=function()
            local done=false;return function() if not done then done=true;return individual end end end})
          for _,city in pairs(CITIES) do city.GetReligion=function(self) return {
            GetReligionsInCity=function() local list={};for id,count in pairs(self.faith) do list[#list+1]={Religion=tonumber(id),Followers=count} end;return list end,
            AddReligiousPressure=function(_,_,id,pressure,owner) self.faith[tostring(id)]=self.pop end} end end
        ''')
        return steps

    def test_live_scenario_builds_missing_hero_origins_on_their_actual_city_plots(self):
        steps = self.live_scenario_fixture()
        self.run_lua("CITIES[4].monument=false;CITIES[8].monument=false;BUILDING_CREATIONS=0")
        self.run_lua(steps[0]["code"])
        self.assertEqual(self.g.BUILDING_CREATIONS, 2)
        self.assertTrue(self.g.CITIES[4].monument)
        self.assertTrue(self.g.CITIES[8].monument)

    def test_live_scenario_checks_execute_in_lua_and_detect_asset_corruption_after_reload(self):
        steps = self.live_scenario_fixture()
        self.run_lua(steps[0]["code"])
        self.run_lua(steps[1]["code"])
        before = self.run_lua(steps[2]["code"])
        self.load_scripts()
        self.assertEqual(self.run_lua(steps[3]["code"]), before)
        self.g.CURRENT_TURN = 6
        self.run_lua('''for _,u in Players[2]:GetUnits():Members() do
          if u:IsHero() then u:SetProperty(UnitPropertyKeys.LifespanRemaining,u:GetProperty(UnitPropertyKeys.LifespanRemaining)-1) end
        end''')
        after = self.run_lua(steps[4]["code"])
        self.load_scripts()
        self.assertEqual(self.run_lua(steps[5]["code"]), after)
        self.run_lua("Players[2].props.JAC_ABSORPTION_ASSETS_V1['live:hero'].Lifespan=18")
        with self.assertRaisesRegex(Exception, "persistent asset changed"):
            self.run_lua(steps[5]["code"])
        self.run_lua("Players[2].props.JAC_ABSORPTION_ASSETS_V1['live:hero'].Lifespan=19")
        for asset in ("live:person", "live:equipment"):
            with self.subTest(missing_asset=asset):
                self.g.MISSING_LIVE_ASSET = asset
                self.run_lua("local r=Players[2].props.JAC_ABSORPTION_ASSETS_V1;LIVE_RESTORE_RECORD=r[MISSING_LIVE_ASSET];r[MISSING_LIVE_ASSET]=nil")
                with self.assertRaisesRegex(Exception, "persistent asset lost"):
                    self.run_lua(steps[5]["code"])
                self.run_lua("Players[2].props.JAC_ABSORPTION_ASSETS_V1[MISSING_LIVE_ASSET]=LIVE_RESTORE_RECORD")
        self.run_lua("local r=Players[2].props.JAC_ABSORPTION_ASSETS_V1['live:hero'];local u=ExposedMembers.JAC_Absorption.FindUnit(2,r.Unit.Token);u:GetProperty('JAC_ABSORPTION_PERSON_V1').Owner=1")
        with self.assertRaisesRegex(Exception, "transferred unit properties changed"):
            self.run_lua(steps[5]["code"])

    def test_live_transfer_rejects_missing_cleric_before_ai_turn(self):
        steps = self.live_scenario_fixture()
        self.run_lua(steps[0]['code'])
        self.run_lua(steps[1]['code'])
        self.run_lua('''for id,u in pairs(Players[2].units) do
          if GameInfo.Units[u:GetType()].UnitType=='UNIT_MISSIONARY' then
            Players[2].units[id]=nil
          end
        end''')
        with self.assertRaisesRegex(Exception, 'transferred unit missing/type/owner mismatch'):
            self.run_lua(steps[2]['code'])

    def test_live_ai_turn_allows_cleric_consumption_but_checks_current_save_snapshot(self):
        steps = self.live_scenario_fixture()
        self.run_lua(steps[0]['code'])
        self.run_lua(steps[1]['code'])
        self.run_lua(steps[2]['code'])
        self.g.CURRENT_TURN = 6
        self.run_lua('''for id,u in pairs(Players[2].units) do
          if u:IsHero() then u:SetProperty(UnitPropertyKeys.LifespanRemaining,
            u:GetProperty(UnitPropertyKeys.LifespanRemaining)-1) end
          if GameInfo.Units[u:GetType()].UnitType=='UNIT_MISSIONARY' then
            Players[2].units[id]=nil
          end
        end
        local current=UNIT(2,777,'UNIT_MISSIONARY');current.religion=5;current.spread=2
        ''')
        after = self.run_lua(steps[4]['code'])
        self.load_scripts()
        self.assertEqual(self.run_lua(steps[5]['code']), after)
        self.run_lua('Players[2].units[777].spread=1')
        self.assertNotEqual(self.run_lua(steps[5]['code']), after)
        self.run_lua('''for id,u in pairs(Players[2].units) do
          if GameInfo.Units[u:GetType()].UnitType=='UNIT_GREAT_GENERAL' then
            Players[2].units[id]=nil
          end
        end''')
        with self.assertRaisesRegex(Exception, 'transferred unit lost after AI turn'):
            self.run_lua(steps[5]['code'])

    def test_live_recall_probes_execute_native_eligibility_checks_and_detect_wrong_owner(self):
        steps = self.live_scenario_fixture()
        self.run_lua(steps[0]['code'])
        self.run_lua(steps[1]['code'])
        self.run_lua('''
          CityCommandTypes={PURCHASE=1,PARAM_UNIT_TYPE=2,PARAM_YIELD_TYPE=3}
          CityCommandResults={FAILURE_REASONS=4};MilitaryFormationTypes={STANDARD_MILITARY_FORMATION=0}
          GameInfo.Yields={YIELD_FAITH={Index=6}}
          GameInfo.Eras=setmetatable({[0]={Index=0,ChronologyIndex=0},[1]={Index=1,ChronologyIndex=1}},
            {__call=function(t) local n=-1;return function() n=n+1;return t[n] end end})
          GLOBAL_ERA=0
          Game.GetEras=function() return {GetCurrentEra=function() return GLOBAL_ERA end} end
          for _,c in pairs(CITIES) do c.GetGold=function() return {GetPurchaseCost=function(_,yield,hash,formation)
            assert(yield==6 and (hash==104 or hash==105) and formation==0);return 1000 end} end end
          RECALL_QUERIES=0
          CityManager.CanStartCommand=function(city,command,visible,parameters,results)
            assert(command==1 and visible==false and parameters[3]==6 and results==true)
            RECALL_QUERIES=RECALL_QUERIES+1
            local class=parameters[2]==104 and 0 or parameters[2]==105 and 1 or error('not a native hero')
            local origin=HERO_ORIGINS[class]
            if HERO_CLAIMS[class]~=city.owner or not origin or origin.player~=city.owner or origin.id~=city.id then
              return false,{[4]={'WRONG_OWNER_OR_ORIGIN'}} end
            for _,u in pairs(Players[city.owner].units) do
              if u:GetHeroClassType()==class then return false,{[4]={'HERO_ALIVE'}} end
            end
            if HERO_CLAIM_ERAS[class]>=GLOBAL_ERA then return false,{[4]={'APPEARANCE_ERA'}} end
            if FAITH[city.owner]<1000 then return false,{[4]={'INSUFFICIENT_FAITH'}} end
            return true,{}
          end
        ''')
        data = json.loads((ROOT / 'tools/civ6-autotest/scenarios/absorption-japan.json').read_text(encoding='utf-8'))
        recall = data['steps'][-5:]
        self.assertIn('duplicate recall denied', self.run_lua(recall[0]['code']))
        self.run_lua('HERO_CLAIMS[0]=1')
        with self.assertRaisesRegex(Exception, 'recall rights retained by donor'):
            self.run_lua(recall[0]['code'])
        self.run_lua('HERO_CLAIMS[0]=2')
        queries = self.g.RECALL_QUERIES
        for step in recall[1:]:
            if step['action']=='autoplay':
                with self.assertRaisesRegex(Exception, 'global era boundary not processed'):
                    self.run_lua(recall[-1]['code'])
                self.g.GLOBAL_ERA = 1
                continue
            if step.get('state_name') == 'InGame':
                self.run_lua('for _,u in Players[2]:GetUnits():Members() do u.IsHero=nil end')
            result = self.run_lua(step['code'])
        self.assertEqual(self.g.RECALL_QUERIES - queries, 2)
        self.assertIn('faith cost=1000', result)
        self.run_lua('HERO_CLAIMS[0]=1')
        with self.assertRaisesRegex(Exception, 'recall owner reverted'):
            self.run_lua(recall[-1]['code'])
        self.run_lua('''HERO_CLAIMS[0]=2;for _,u in Players[2]:GetUnits():Members() do
          u.GetHeroClassType=function() return 0 end;break end''')
        probes = [step for step in recall if step.get('state_name') == 'InGame'
                  and 'death fixture still alive' in step['code']]
        self.assertEqual(len(probes), 2)
        for step in probes:
            with self.assertRaisesRegex(Exception, 'death fixture still alive'):
                self.run_lua(step['code'])

    def test_human_receiver_scenario_executes_real_recall_and_rebinds_person_in_lua_vm(self):
        self.live_scenario_fixture()
        steps = json.loads((ROOT / 'tools/civ6-autotest/scenarios/absorption-hero-recall.json')
                           .read_text(encoding='utf-8'))['steps']
        steps = [step for step in steps if 'recall spawn blockers relocated' not in step.get('code', '') and step['action'] != 'autoplay' and not step.get('capture_as', '').startswith('JAC_ABSORPTION_SYNC_')
                 and not any(key in step.get('condition', {}).get('code', '') for key in ('JAC_ABSORPTION_FIXTURE_UI_WAIT', 'GetUIPhase', 'GetFaithBalance()>=1000000', 'JAC_ABSORPTION_RECALL_COST_V1'))]
        self.run_lua('''
          Game.GetLocalPlayer=function() return 0 end
          for id=4,21 do
            local n=id
            PlayerConfigurations[id].GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_OTHER_'..n end
          end
          PlayerConfigurations[3].GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_OTHER_3' end
          GameInfo.Yields={YIELD_FAITH={Index=6}}
          GameInfo.GreatWorks={[0]={GreatWorkObjectType='GREATWORKOBJECT_HERO'}}
          Game.GetGreatWorkType=function(id) assert(id==1007);return 0 end
          GameInfo.Eras=setmetatable({[0]={Index=0,ChronologyIndex=0},[1]={Index=1,ChronologyIndex=1}},
            {__call=function(t) local n=-1;return function() n=n+1;return t[n] end end})
          GLOBAL_ERA=0
          Game.GetEras=function() return {GetCurrentEra=function() return GLOBAL_ERA end} end
          CityCommandTypes={PURCHASE=1,PARAM_UNIT_TYPE=2,PARAM_MILITARY_FORMATION_TYPE=3,PARAM_YIELD_TYPE=4}
          CityCommandResults={FAILURE_REASONS=5}
          MilitaryFormationTypes={STANDARD_MILITARY_FORMATION=0}
          for owner,p in pairs(Players) do
            local original=p.GetReligion
            p.GetReligion=function()
              local religion=original()
              religion.GetFaithBalance=function() return FAITH[owner] end
              return religion
            end
          end
          for _,c in pairs(CITIES) do
            c.GetGold=function() return {GetPurchaseCost=function() return 1000 end} end
          end
          CityManager.CanStartCommand=function(city,command,visible,parameters,results)
            assert(command==1 and parameters[2]==104 and parameters[3]==0 and parameters[4]==6)
            return HERO_CLAIMS[0]==city.owner and GLOBAL_ERA>HERO_CLAIM_ERAS[0]
              and FAITH[city.owner]>=1000,{}
          end
          CityManager.RequestCommand=function(city,command,parameters)
            assert(CityManager.CanStartCommand(city,command,false,parameters,true))
            FAITH[city.owner]=FAITH[city.owner]-1000
            HERO_MANAGER:CreateHero(city.owner,0,city.x,city.y)
          end
        ''')
        self.run_lua(steps[5]['code'])
        self.run_lua(steps[6]['code'])
        self.load_scripts()
        self.run_lua(steps[11]['code'])
        self.g.CURRENT_TURN = 6
        self.run_lua('''local e=Game:GetProperty('JAC_ABSORPTION_RECALL_EXPECT_V1')
          local a=ExposedMembers.JAC_Absorption;local r=a.GetRoster(e.To)[e.RecordID]
          a.FindUnit(e.To,r.Unit.Token).lifespan=18''')
        self.run_lua(steps[14]['code'])
        self.load_scripts()
        with self.assertRaisesRegex(Exception, 'global era boundary not processed'):
            self.run_lua(steps[15]['code'])
        self.g.GLOBAL_ERA = 1
        self.run_lua(steps[15]['code'])
        current_steps = json.loads((ROOT / 'tools/civ6-autotest/scenarios/absorption-hero-recall.json')
                                   .read_text(encoding='utf-8'))['steps']
        funding = next(s for s in current_steps if 'GetFaithBalance()>=1000000' in s.get('condition', {}).get('code', ''))
        preparation = next(s for s in current_steps if s.get('capture_as') == 'JAC_ABSORPTION_SYNC_RECALL_COST')
        published = next(s for s in current_steps if 'JAC_ABSORPTION_RECALL_COST_V1' in s.get('condition', {}).get('code', ''))
        self.assertTrue(self.run_lua(funding['condition']['code']))
        self.run_lua(preparation['code'])
        self.assertTrue(self.run_lua(published['condition']['code']))
        self.run_lua("Game:GetProperty('JAC_ABSORPTION_RECALL_COST_V1').CostBefore=999")
        self.assertFalse(self.run_lua(published['condition']['code']))
        with self.assertRaisesRegex(Exception, 'recall cost snapshot changed'):
            self.run_lua(steps[16]['code'])
        self.run_lua(preparation['code'])
        self.run_lua('GAME_SET_PROPERTY=Game.SetProperty;Game.SetProperty=nil')
        self.run_lua(steps[16]['code'])
        self.run_lua('Game.SetProperty=GAME_SET_PROPERTY')
        all_steps = json.loads((ROOT / 'tools/civ6-autotest/scenarios/absorption-hero-recall.json')
                               .read_text(encoding='utf-8'))['steps']
        binding = next(s for s in all_steps if s.get('capture_as') == 'JAC_ABSORPTION_SYNC_VERIFY_2')
        self.run_lua("""local a=ExposedMembers.JAC_Absorption;local request=a.RequestVerification
          a.RequestVerification=function(id)
            assert(a.GetState().Recalls['japan-human-recall:1'].Status=='COMPLETE','verification requested before binding')
            return request(id)
          end""")
        self.run_lua(binding['code'])
        self.run_lua("""local e=Game:GetProperty('JAC_ABSORPTION_RECALL_EXPECT_V1')
          local a=ExposedMembers.JAC_Absorption;local r=a.GetRoster(e.To)[e.RecordID]
          assert(a.GetState().Recalls['japan-human-recall:1'].Status=='COMPLETE')
          assert(r.Unit.ID~=e.TransferID and #r.RecallHistory==1)""")
        self.run_lua("ExposedMembers.JAC_Absorption.CompleteHeroRecall=function() error('binding after verification request') end")
        self.run_lua(steps[18]['code'])
        self.load_scripts()
        self.run_lua(steps[23]['code'])
        self.run_lua('''local e=Game:GetProperty('JAC_ABSORPTION_RECALL_EXPECT_V1')
          assert(e.RecallID~=e.TransferID and e.FaithAfter==999000 and e.CostAfter==1000)
          assert(e.Before.HeroCountAvailable and e.Before.Hero==1 and e.AfterRecall.Hero==1)
          assert(#ExposedMembers.JAC_Absorption.GetRoster(e.To)[e.RecordID].RecallHistory==1)
        ''')
        recall_owner = self.run_lua("return Game:GetProperty('JAC_ABSORPTION_RECALL_EXPECT_V1').To")
        for invalid_balance in (999500, 998500):
            self.g.FAITH[recall_owner] = invalid_balance
            with self.assertRaisesRegex(Exception, "native recall faith payment changed"):
                self.run_lua(steps[18]['code'])
        self.g.FAITH[recall_owner] = 999000

    def test_partial_expected_properties_freeze_references_to_every_staged_unit(self):
        self.run_lua('''
          ExposedMembers.JAC_Absorption.RegisterUnitProperty('PARTIAL_REF',function(value,tx,snap)
            if snap.ID==3 then return {Target=tx.UnitMap['1:12'].ID} end
            if snap.ID==12 then error('late rebind failed') end
            return value
          end)
        ''')
        with self.assertRaisesRegex(Exception, "late rebind failed"):
            self.apply()
        self.run_lua("local a=ExposedMembers.JAC_Absorption;assert(a.GetState().Transactions['test-absorption'].UnitMapFrozen);local u=a.FindUnit(2,'test-absorption:unit:12');Players[2].units[u:GetID()]=nil")
        before = (self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS)
        with self.assertRaisesRegex(Exception, "staged replacement was lost"):
            self.a.Resume("test-absorption")
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), before)

    def test_receiver_assets_added_or_updated_while_blocked_survive_resume(self):
        self.g.FAIL_CITY = True
        with self.assertRaisesRegex(Exception, "city failed"):
            self.apply()
        self.run_lua("local r=Players[2].props.JAC_ABSORPTION_ASSETS_V1;r.existing.Level=9;r.new={ID='new',Kind='ITEM',Owner=2,Type='ARTIFACT'}")
        self.g.FAIL_CITY = False
        self.a.Resume("test-absorption")
        roster = self.a.GetRoster(2)
        self.assertEqual(roster.existing.Level, 9)
        self.assertEqual(roster.new.Owner, 2)
        self.assertTrue(self.verify("test-absorption", True))

    def test_receiver_assets_added_by_city_transfer_hooks_are_not_overwritten(self):
        self.run_lua('''
          local transfer=CityManager.TransferCity
          CityManager.TransferCity=function(city,owner)
            transfer(city,owner)
            local r=Players[owner].props.JAC_ABSORPTION_ASSETS_V1
            r['hook:'..city.x]={ID='hook:'..city.x,Kind='ITEM',Owner=owner,Type='COLLECTION'}
          end
        ''')
        self.apply()
        self.assertEqual(self.a.GetRoster(2)["hook:4"].Owner, 2)
        self.assertEqual(self.a.GetRoster(2)["hook:8"].Owner, 2)

    def test_new_destination_id_collision_is_stopped_before_removing_donors(self):
        self.g.FAIL_COPY = True
        with self.assertRaisesRegex(Exception, "copy failed"):
            self.apply()
        self.run_lua("Players[2].props.JAC_ABSORPTION_ASSETS_V1.hero={ID='hero',Kind='HERO',Owner=2,Lifespan=1}")
        self.g.FAIL_COPY = False
        with self.assertRaisesRegex(Exception, "destination asset collision"):
            self.a.Resume("test-absorption")
        self.assertEqual((self.g.KILLS, self.g.TRANSFERS), (0, 0))

    def test_source_assets_created_while_blocked_are_preserved_instead_of_cleared(self):
        self.g.FAIL_COPY = True
        with self.assertRaisesRegex(Exception, "copy failed"):
            self.apply()
        self.run_lua("Players[1].props.JAC_ABSORPTION_ASSETS_V1.new={ID='new',Kind='ITEM',Owner=1,Type='ARTIFACT'}")
        self.g.FAIL_COPY = False
        with self.assertRaisesRegex(Exception, "source roster changed"):
            self.a.Resume("test-absorption")
        self.assertEqual(self.a.GetRoster(1).new.Owner, 1)
        self.assertEqual((self.g.KILLS, self.g.TRANSFERS), (0, 0))

    def test_palace_work_loss_is_detected_even_when_the_palace_building_is_exempt(self):
        self.run_lua('''
          CITIES[4].palace=true;CITIES[4].palaceWork=9001
          local transfer=CityManager.TransferCity
          CityManager.TransferCity=function(city,owner) transfer(city,owner);city.palace=false end
        ''')
        with self.assertRaisesRegex(Exception, "Palace Great Work lost"):
            self.apply()
        self.assertEqual(self.a.GetState().Transactions["test-absorption"].Cities[1].PalaceWorks[1], 9001)
        self.assertEqual(self.a.GetState().Transactions["test-absorption"].Status, "BLOCKED")

    def test_palace_work_moved_to_receiver_capital_is_preserved_and_verified(self):
        self.run_lua('''
          CITIES[4].palace=true;CITIES[4].palaceWork=9001
          local transfer=CityManager.TransferCity
          CityManager.TransferCity=function(city,owner)
            local work=city.palaceWork;transfer(city,owner);city.palace=false
            if work then CITIES[12].palace=true;CITIES[12].palaceWork=work end
          end
        ''')
        self.apply()
        self.assertTrue(self.a.OwnedGreatWorks(2)[9001])
        self.assertTrue(self.verify("test-absorption", True))

    def test_previous_governance_is_saved_before_engine_clears_city_properties(self):
        self.run_lua("OLD_GOV={ID='prior',From=3,To=1,Turn=2,Previous={ID='older',From=0,To=3,Turn=1}};CITIES[4]:SetProperty('JAC_ABSORPTION_GOVERNANCE_V1',OLD_GOV)")
        self.apply()
        self.run_lua("assert(JAC_AbsorptionRules.Equal(CITIES[4]:GetProperty('JAC_ABSORPTION_GOVERNANCE_V1').Previous,OLD_GOV))")
        self.load_scripts()
        self.assertTrue(self.verify("test-absorption", True))


    def test_exact_verification_detects_city_rename_but_later_turns_allow_it(self):
        self.apply()
        self.run_lua("CITIES[4].GetName=function() return 'RENAMED_CITY' end")
        with self.assertRaisesRegex(Exception, 'city name changed'):
            self.a.Verify('test-absorption', True)
        self.a.Verify('test-absorption', False)


class AbsorptionPackageTests(unittest.TestCase):
    def test_cleric_fixture_can_be_purchased_without_enabling_other_disabled_units(self):
        with sqlite3.connect(":memory:") as db:
            db.execute("CREATE TABLE Units (UnitType TEXT PRIMARY KEY, CanTrain INTEGER)")
            db.executemany("INSERT INTO Units VALUES (?,?)", [
                ("UNIT_MISSIONARY", 0), ("UNIT_APOSTLE", 0), ("UNIT_HERO_HERCULES", 0), ("UNIT_WARRIOR", 1)])
            db.executescript((MOD / "Data/JAC_Absorption.sql").read_text(encoding="utf-8"))
            self.assertEqual(dict(db.execute("SELECT UnitType,CanTrain FROM Units")), {
                "UNIT_MISSIONARY": 1, "UNIT_APOSTLE": 0, "UNIT_HERO_HERCULES": 0, "UNIT_WARRIOR": 1})
        root = ET.parse(MOD / "JAC_PoC_Absorption.modinfo").getroot()
        action = root.find("./InGameActions/UpdateDatabase")
        self.assertEqual(action.get("criteria"), "JAC_Absorption_Banseiki")
        self.assertGreater(int(action.findtext("./Properties/LoadOrder")), 200)

    def test_scenario_lua_compiles_and_both_snapshots_have_round_trip_comparisons(self):
        data = json.loads((ROOT / "tools/civ6-autotest/scenarios/absorption-japan.json").read_text(encoding="utf-8"))
        steps = data["steps"]
        vm = LuaRuntime()
        compile_error = vm.eval("function(code) local f,e=load(code);return e end")
        for step in steps:
            if step["action"] == "ui_lua":
                self.assertIsNone(compile_error(step["code"]))
            if step["action"] == "wait_for":
                self.assertIsNone(compile_error(step["condition"]["code"]))
        captures = {step["capture_as"] for step in steps if "capture_as" in step and not step["capture_as"].startswith("JAC_ABSORPTION_SYNC_")}
        self.assertEqual(captures, {"ABSORPTION_IMMEDIATE", "ABSORPTION_AFTER_AI_TURN"})
        self.assertEqual({step["equals_capture"] for step in steps if "equals_capture" in step}, captures)
        self.assertEqual(sum(step["action"] == "save" for step in steps), 2)
        self.assertEqual(sum(step["action"] == "load" for step in steps), 2)
        configure = next(step for step in steps if step["action"] == "configure")
        enabled = next(step for step in steps if step['action'] == 'enable_mods')
        self.assertIn('8424840C-92EF-4426-A9B4-B4E0CB818049', enabled['ids'])
        modes = [step for step in steps if step['action'] == 'ui_lua' and step['state_name'] == 'FrontEnd']
        self.assertEqual(len(modes), 1)
        self.assertIn("GameConfiguration.SetValue('GAMEMODE_HEROES',true)", modes[0]['code'])
        self.assertEqual(configure["map_file"], "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map")
        ai = {p["civ"] for p in configure["players"] if not p["human"]}
        self.assertTrue({"CIVILIZATION_JAC_ASHINA", "CIVILIZATION_JAC_DATE"} <= ai)
        end = next(step for step in steps if step["action"] == "end_turn")
        self.assertEqual(end["turns"], 1)

    def test_control_missionary_publication_checks_charges_then_core_religion(self):
        data = json.loads((ROOT / 'tools/civ6-autotest/scenarios/absorption-japan.json')
                          .read_text(encoding='utf-8'))
        condition = next(s['condition']['code'] for s in data['steps']
                         if s['action']=='wait_for' and 'JAC_ABSORPTION_CONTROL_PRIEST_V1' in s['condition']['code'])
        check = next(s['code'] for s in data['steps']
                     if s['action']=='ui_lua' and 'missionary core religion mismatch' in s.get('code',''))
        vm = LuaRuntime()
        vm.execute('''
          CONTROL={Owner=2,ID=19,Religion=5,SpreadCharges=3,Baseline={Transferred={ID=20}}}
          CORE_RELIGION=5;UI_CHARGES=3;UI_VISIBLE=true
          Game={GetProperty=function() return CONTROL end}
          local units={}
          for id=19,20 do local n=id;units[#units+1]={GetID=function() return n end,
            GetOwner=function() return 2 end,GetType=function() return 0 end,
            GetReligionType=function() return -1 end,
            GetSpreadCharges=function() return UI_CHARGES end,
            GetReligion=function() return {GetReligionType=function() return CORE_RELIGION end} end} end
          Players={[2]={GetUnits=function() return {Members=function()
            return ipairs(UI_VISIBLE and units or {}) end} end}}
          GameInfo={Units={[0]={UnitType='UNIT_MISSIONARY'}}}
          ExposedMembers={JAC_Absorption={SnapshotUnit=function() return {SpreadCharges=UI_CHARGES} end}}
        ''')
        self.assertTrue(vm.execute(condition))
        self.assertIn('core religion and published charges match', vm.execute(check))
        vm.execute('UI_VISIBLE=false')
        self.assertFalse(vm.execute(condition))
        with self.assertRaisesRegex(Exception, 'missionary absent in core'):
            vm.execute(check)
        vm.execute('UI_VISIBLE=true;UI_CHARGES=2')
        self.assertFalse(vm.execute(condition))
        with self.assertRaisesRegex(Exception, 'published charges mismatch'):
            vm.execute(check)
        vm.execute('UI_CHARGES=3;CORE_RELIGION=4')
        with self.assertRaisesRegex(Exception, 'core religion mismatch'):
            vm.execute(check)

    def test_recall_publication_uses_ui_class_getter_and_waits_for_native_claim(self):
        data = json.loads((ROOT / 'tools/civ6-autotest/scenarios/absorption-hero-recall.json')
                          .read_text(encoding='utf-8'))
        code = next(s['condition']['code'] for s in data['steps']
                    if s['action'] == 'wait_for' and 'e.TransferID' in s['condition']['code'])
        vm = LuaRuntime()
        vm.execute('''
          CLASS=0;ID=10;CLAIM=2;ORIGIN={player=2,id=7};VISIBLE=true
          Game={GetProperty=function() return {To=2,Class=0,TransferID=10} end,
            GetHeroesManager=function() return {
              GetHeroClaimPlayer=function() return CLAIM end,
              GetHeroOriginCityID=function() return ORIGIN end} end}
          local u={GetHeroClassType=function() return CLASS end,GetID=function() return ID end}
          -- Installed UI Units have GetHeroClassType, but no IsHero method.
          Players={[2]={GetUnits=function() return {Members=function()
            return ipairs(VISIBLE and {u} or {}) end} end}}
        ''')
        self.assertFalse(vm.execute(code))
        vm.execute('ID=11')
        self.assertTrue(vm.execute(code))
        for state in ('CLASS=1', 'CLAIM=1', 'ORIGIN=nil', 'ORIGIN={player=1,id=7}', 'VISIBLE=false'):
            vm.execute('CLASS=0;CLAIM=2;ORIGIN={player=2,id=7};VISIBLE=true')
            vm.execute(state)
            self.assertFalse(vm.execute(code), state)

    def test_recall_spawn_preparation_moves_only_blockers_and_preserves_them(self):
        data = json.loads((ROOT / 'tools/civ6-autotest/scenarios/absorption-hero-recall.json')
                          .read_text(encoding='utf-8'))
        code = next(s['code'] for s in data['steps']
                    if 'recall spawn blockers relocated' in s.get('code', ''))
        vm = LuaRuntime()
        vm.execute('''
          FREE=true;UNITS={}
          for id=1,3 do
            local u={id=id,x=20,y=20,moves=2,damage=5,type=id==2 and 'Civilian' or 'Warrior'}
            u.GetID=function(self) return self.id end
            u.GetType=function(self) return self.type end
            u.GetX=function(self) return self.x end;u.GetY=function(self) return self.y end
            u.GetDamage=function(self) return self.damage end
            u.SetDamage=function(self,n) self.damage=n end
            u.GetMovesRemaining=function(self) return self.moves end
            u.ChangeMovesRemaining=function(self,n) self.moves=self.moves+n end
            UNITS[id]=u
          end
          GameInfo={HeroClasses={[0]={UnitType='Hero'}},Units={Hero={FormationClass='LAND'},
            Warrior={FormationClass='LAND'},Civilian={FormationClass='CIVILIAN'}}}
          Game={GetProperty=function() return {To=2,Class=0} end}
          ExposedMembers={JAC_Absorption={GetHeroesManager=function() return {
            GetHeroOriginCityID=function() return {player=2,id=7} end} end}}
          Players={[2]={GetCities=function() return {FindID=function(_,id)
            assert(id==7);return {GetX=function() return 20 end,GetY=function() return 20 end} end} end,
            GetUnits=function() return {Members=function() return ipairs(UNITS) end} end}}
          Map={GetPlot=function(x,y) return {x=x,y=y,GetOwner=function()
            return FREE and x==14 and y>=14 and y<=17 and 2 or -1 end,
            IsWater=function() return y==14 end,IsImpassable=function() return false end} end,
            GetUnitsAt=function(plot) return {GetCount=function()
              local n=plot.x==14 and plot.y==15 and 1 or 0
              for _,u in ipairs(UNITS) do if u.x==plot.x and u.y==plot.y then n=n+1 end end
              return n end} end}
          UnitManager={PlaceUnit=function(u,x,y) u.x=x;u.y=y;u.moves=0;u.damage=0 end,
            Kill=function() error('spawn preparation must never delete units') end}
        ''')
        self.assertIn('without deletion: 2', vm.execute(code))
        vm.execute('''assert(UNITS[1].x==14 and UNITS[1].y==16)
          assert(UNITS[3].x==14 and UNITS[3].y==17)
          assert(UNITS[2].x==20 and UNITS[2].y==20)
          for _,u in ipairs(UNITS) do assert(u.damage==5 and u.moves==2) end
          FREE=false;UNITS[1].x=20;UNITS[1].y=20''')
        with self.assertRaisesRegex(Exception, 'no empty owned plot'):
            vm.execute(code)
        vm.execute('assert(#UNITS==3 and UNITS[1].x==20 and UNITS[1].y==20)')

    def test_human_recall_scenario_uses_custom_native_classes_and_real_purchase(self):
        data = json.loads((ROOT / "tools/civ6-autotest/scenarios/absorption-hero-recall.json")
                          .read_text(encoding="utf-8"))
        steps = data["steps"]
        compile_error = LuaRuntime().eval("function(code) local f,e=load(code);return e end")
        for step in steps:
            if step["action"] == "ui_lua":
                self.assertIsNone(compile_error(step["code"]))
            if step["action"] == "wait_for":
                self.assertIsNone(compile_error(step["condition"]["code"]))
        configure = next(step for step in steps if step["action"] == "configure")
        self.assertEqual(len(configure["players"]), 22)
        self.assertEqual([player["civ"] for player in configure["players"] if player["human"]],
                         ["CIVILIZATION_JAC_ODA"])
        self.assertEqual(sum(step["action"] == "end_turn" for step in steps), 1)
        self.assertEqual(sum(step["action"] == "save" for step in steps), 2)
        self.assertEqual(sum(step["action"] == "load" for step in steps), 2)
        code = "\n".join(step["code"] for step in steps if step["action"] == "ui_lua")
        self.assertIn("CityManager.RequestCommand(city,CityCommandTypes.PURCHASE,parameters)", code)
        self.assertIn("A.BeginHeroRecall", code)
        self.assertIn("A.CompleteHeroRecall", code)
        self.assertIn("non-Banseiki Hero class enabled", code)

    def test_modinfo_references_exist_and_have_no_open_pr_dependencies(self):
        root = ET.parse(MOD / "JAC_PoC_Absorption.modinfo").getroot()
        files = {n.text for n in root.findall("./Files/File")}
        self.assertEqual(files, {"Data/JAC_Absorption.sql", "Text/JAC_Absorption_Text.xml",
                                 "Scripts/JAC_Absorption_Rules.lua", "Scripts/JAC_Absorption_Gameplay.lua",
                                 "UI/JAC_Absorption_HeroKeys.lua", "UI/JAC_Absorption_HeroKeys.xml"})
        for name in files:
            self.assertTrue((MOD / name).is_file(), name)
        self.assertEqual({n.get("id") for n in root.findall("./Dependencies/Mod")},
                         {"4873eb62-8ccc-4574-b784-dda455e74e68", "bd7a82c5-6b38-4b69-a7f5-8dc9af534249",
                          "8424840C-92EF-4426-A9B4-B4E0CB818049"})
        self.assertEqual(root.findtext("./ActionCriteria/Criteria/RuleSetInUse"), "RULESET_JAC_BANSEIKI")

    def test_all_text_has_exactly_japanese_english_and_russian(self):
        texts = {}
        for row in ET.parse(MOD / "Text/JAC_Absorption_Text.xml").findall("./LocalizedText/Row"):
            key, lang = row.get("Tag"), row.get("Language")
            self.assertNotIn(lang, texts.setdefault(key, {}))
            texts[key][lang] = row.findtext("Text")
        self.assertEqual(len(texts), 4)
        for langs in texts.values():
            self.assertEqual(set(langs), {"ja_JP", "en_US", "ru_RU"})
            self.assertTrue(all(langs.values()))


if __name__ == "__main__":
    unittest.main()
