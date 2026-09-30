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
           [3]={UnitType="UNIT_BUILDER"},[4]={UnitType="UNIT_HERO_HERCULES"}},
  Religions = {[5]={ReligionType="RELIGION_TEST"}},
  Technologies = {TECH_TEST={Index=11}}, Civics = {CIVIC_TEST={Index=12}}
}
TYPE = {UNIT_WARRIOR=0,UNIT_GREAT_GENERAL=1,UNIT_MISSIONARY=2,UNIT_BUILDER=3,UNIT_HERO_HERCULES=4}
CURRENT_TURN, NEXT_ID, SPAWNS, KILLS, TRANSFERS = 5, 100, 0, 0, 0
FAIL_SPAWN, FAIL_CITY, FAIL_COPY, WIPE_ON_LAST_CITY = nil, false, false, true
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
  local p=prop({owner=owner,units={},cities={},techs={},civics={}})
  p.GetID=function() return owner end
  p.IsAlive=function() return next(p.cities) ~= nil end
  p.IsHuman=function() return owner==0 end
  p.GetUnits=function() return {
    Members=function() return pairs(p.units) end,
    GetCount=function() local n=0;for _ in pairs(p.units) do n=n+1 end;return n end
  } end
  p.GetCities=function() return {
    Members=function() return pairs(p.cities) end,
    GetCount=function() local n=0;for _ in pairs(p.cities) do n=n+1 end;return n end,
    GetCapitalCity=function() local _,c=next(p.cities);return c end
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
    promotions={},formation=0,gpcharges=2,build=kind=="UNIT_BUILDER" and 3 or 0,spread=kind=="UNIT_MISSIONARY" and 3 or 0,religion=-1})
  u.GetOwner=function(self) return self.owner end
  u.GetID=function(self) return self.id end
  u.GetType=function(self) return self.kind end
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
  local city=prop({owner=owner,x=x,y=0,pop=pop or 6,faith=faith or {['5']=4},loyalty=87})
  city.GetOwner=function(self) return self.owner end
  city.GetX=function(self) return self.x end;city.GetY=function(self) return self.y end
  city.GetPopulation=function(self) return self.pop end
  city.GetName=function(self) return "CITY_"..self.x end
  city.GetBuildings=function() return {
    HasBuilding=function(_,id) return id==1 or (id==0 and city.palace==true) end,IsPillaged=function() return false end,
    GetNumGreatWorkSlots=function() return 1 end,GetGreatWorkInSlot=function(_,id) return id==0 and (city.palaceWork or -1) or 1007 end
  } end
  city.ChangeLoyalty=function(self,n) self.loyalty=self.loyalty+n end
  city.GetReligion=function(self) return {GetReligionsInCity=function()
    local result={};for id,n in pairs(self.faith) do result[#result+1]={Religion=tonumber(id),Followers=n} end;return result
  end} end
  Players[owner].cities[x]=city;CITIES[x]=city
  for index=x,x+1 do
    local plot={owner=owner,city=x}
    plot.GetOwner=function(self) return self.owner end
    PLOTS[index]=plot
  end
  return city
end
for i=0,19 do PLOTS[i]={GetOwner=function() return -1 end} end
Map={GetPlotCount=function() return 20 end,GetPlotByIndex=function(i) return PLOTS[i] end}
CityManager={GetCityAt=function(x,y) return CITIES[x] end,
  TransferCity=function(city,owner)
    if FAIL_CITY then error("city failed") end
    TRANSFERS=TRANSFERS+1
    local donor=city.owner
    Players[donor].cities[city.x]=nil;Players[owner].cities[city.x]=city;city.owner=owner
    -- Deliberately replace properties and change engine IDs during transfer.
    city.props={}
    for _,plot in pairs(PLOTS) do if plot.city==city.x then plot.owner=owner end end
    if WIPE_ON_LAST_CITY and next(Players[donor].cities)==nil then Players[donor].units={} end
  end}
UnitManager={GetUnit=function(owner,id) return Players[owner].units[id] end,
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

    def snapshot(self):
        return self.run_lua('''local a=ExposedMembers.JAC_Absorption;local result={State=a.GetState(),Owners={},Cities={},Units={}}
          for owner=0,3 do result.Owners[owner]=a.GetRoster(owner);result.Units[owner]={}
            for id,u in Players[owner]:GetUnits():Members() do
              result.Units[owner][id]={Owner=u.owner,Type=u.kind,Individual=u.individual,Damage=u.damage,
                Moves=u.moves,XP=u.xp,Promotions=u.promotions,Properties=u.props} end end
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
        self.assertTrue(self.a.Verify(tx.ID, True))
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
        self.assertTrue(self.a.Verify("test-absorption", True))
        self.apply()
        self.assertEqual((self.g.SPAWNS, self.g.KILLS, self.g.TRANSFERS), (6, 5, 2))

    def test_non_transfer_branches_keep_original_snapshots_and_detach_person_references(self):
        self.run_lua('''PLAN.Units['1:3']='ESCAPE';PLAN.Units['1:8']='CULTURE';PLAN.Units['1:11']='HIDE';PLAN.Assets.hero='HIDE'
          PLAN.Units['1:12']='RETIRE';PLAN.Assets.person='RETIRE';PLAN.Assets.artifact='SCATTER';PLAN.Assets.unlock='PRESERVE';PLAN.Leader='EXILE' ''')
        self.apply()
        self.assertTrue(self.a.Verify("test-absorption", True))
        self.run_lua('''local s=ExposedMembers.JAC_Absorption.GetState();local ar=s.Archive
          assert(ar.hero.Owner==-1 and ar.hero.Lifespan==19 and ar.hero.Unit==nil and ar.hero.FormerUnit.ID==11)
          assert(ar.person.Outcome=='RETIRE' and ar.artifact.Outcome=='SCATTER' and ar['leader:1'].LeaderType=='LEADER_TEST_1')
          local converted=ExposedMembers.JAC_Absorption.GetRoster(2)['native-person:1:8']
          assert(converted.Owner==2 and converted.Outcome=='CULTURE' and converted.NativeIndividual==42 and converted.Unit==nil)
          assert(s.Transactions['test-absorption'].Units[1].Properties.JAC_POC_GROWTHUI_WARLORD.Level==4)
          assert(not Players[2]:GetTechs():HasTech(11) and not Players[2]:GetCulture():HasCivic(12))''')

    def test_returned_belonging_goes_to_explicit_surviving_third_party(self):
        self.run_lua("PLAN.Assets.artifact={Outcome='RETURN',ReturnOwner=3}")
        self.apply()
        self.assertEqual(self.a.GetRoster(3).artifact.Owner, 3)
        self.assertTrue(self.a.Verify("test-absorption", True))

    def test_every_spec_disposition_is_routable_and_rejects_cross_kind_outcomes(self):
        expected = {
            "UNIT": {"SURRENDER", "INCORPORATE", "DISSOLVE", "ESCAPE", "RESIST"},
            "PERSON": {"CAPTURE", "RECRUIT", "SUBMIT", "EXILE", "EXECUTE", "RETIRE", "DEFECT"},
            "HERO": {"RECRUIT", "PROTECT", "RECONTRACT", "DEFECT", "HIDE"},
            "GREAT_PERSON": {"RECRUIT", "RESEARCH", "FAITH", "CULTURE", "EXILE", "RETIRE"},
            "ITEM": {"TRANSFER", "LOOT", "SEIZE", "RETURN", "SCATTER"},
            "UNLOCK": {"INHERIT", "PRESERVE"}, "RELIGION": {"PRESERVE"},
        }
        actual = {}
        for row in outcome_rows():
            actual.setdefault(row["Kind"], set()).add(row["Outcome"])
            self.assertEqual(self.a.Rules.Destination(row["Kind"], row["Outcome"]), row["Destination"])
        self.assertEqual(actual, expected)
        self.assert_rejected_unchanged("PLAN.Assets.hero='EXECUTE'", "invalid disposition")

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

    def test_native_hero_is_rejected_without_loss(self):
        self.assert_rejected_unchanged("UNIT(1,13,'UNIT_HERO_HERCULES');PLAN.Units['1:13']='PROTECT'", "native Hero")

    def test_engine_spawn_failure_preserves_every_donor_unit_and_city_then_resume_is_idempotent(self):
        self.g.FAIL_SPAWN = 2
        with self.assertRaisesRegex(Exception, "did not create"):
            self.apply()
        self.assertEqual((self.g.KILLS, self.g.TRANSFERS), (0, 0))
        self.assertEqual(self.a.GetState().Transactions["test-absorption"].Status, "BLOCKED")
        self.g.FAIL_SPAWN = None
        self.load_scripts()  # no session-local transaction cache can help recovery
        self.a.Resume("test-absorption")
        self.assertTrue(self.a.Verify("test-absorption", True))
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
        self.assertTrue(self.a.Verify("test-absorption", True))

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
            self.a.Verify("test-absorption", True)

    def test_completed_transaction_is_not_replayed_after_later_turns(self):
        self.apply()
        self.g.CURRENT_TURN = 6
        self.run_lua("ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3').damage=0")
        self.apply()
        self.assertTrue(self.a.Verify("test-absorption", False))
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
            self.a.Verify("test-absorption", True)

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
        self.assertTrue(self.a.Verify("test-absorption", True))
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
        self.assertTrue(self.a.Verify("test-absorption", True))
        self.run_lua("local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3');assert(u:GetProperty('FOREIGN_MISSION').Actor.ID==u:GetID())")
        self.assertEqual(self.g.REBIND_CALLS, 1)

    def test_corrupted_rebound_reference_is_detected_by_generic_verification(self):
        self.register_mission_rebind()
        self.apply()
        self.run_lua("ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3'):GetProperty('FOREIGN_MISSION').Actor.Owner=1")
        with self.assertRaisesRegex(Exception, "unit properties changed"):
            self.a.Verify("test-absorption", True)

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
        self.assertTrue(self.a.Verify("test-absorption", True))

    def test_nonserializable_property_rebind_blocks_before_destroying_donors(self):
        self.run_lua("ExposedMembers.JAC_Absorption.RegisterUnitProperty('BAD_REBIND',function() return function() end end)")
        with self.assertRaisesRegex(Exception, "serializable"):
            self.apply()
        self.assertEqual((self.g.KILLS, self.g.TRANSFERS), (0, 0))

    def test_native_hero_detected_by_engine_flag_with_a_nonstandard_type_is_rejected(self):
        self.assert_rejected_unchanged("local u=UNIT(1,13,'UNIT_WARRIOR');u.IsHero=function() return true end;PLAN.Units['1:13']='INCORPORATE'", "native Hero")

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

    def test_live_scenario_checks_execute_in_lua_and_detect_asset_corruption_after_reload(self):
        data = json.loads((ROOT / "tools/civ6-autotest/scenarios/absorption-japan.json").read_text(encoding="utf-8"))
        steps = [step for step in data["steps"] if step["action"] == "ui_lua"]
        self.run_lua('''
          local majors={};for id=0,21 do
            local n=id;majors[#majors+1]={GetID=function() return n end}
          end
          PlayerManager={GetAliveMajors=function() return majors end}
          for id=0,21 do
            local n=id
            PlayerConfigurations[id]={GetLeaderTypeName=function() return 'LEADER_TEST_'..n end,
              GetCivilizationTypeName=function() return n==1 and 'CIVILIZATION_JAC_ASHINA' or n==2 and 'CIVILIZATION_JAC_DATE' or 'CIVILIZATION_JAC_ODA' end}
          end
          local founder={}
          for id,p in pairs(Players) do local n=id;p.GetReligion=function() return {GetReligionTypeCreated=function() return founder[n] or -1 end} end end
          Game.GetReligion=function() return {FoundReligion=function(_,owner,id) founder[owner]=id end,
            GetReligions=function() local list={};for owner,id in pairs(founder) do list[#list+1]={Religion=id,Founder=owner,Beliefs={3,7}} end;return list end} end
          GameInfo.Religions.RELIGION_CUSTOM_1={Index=5,ReligionType='RELIGION_TEST'}
          local individual={Index=42,GreatPersonClassType='GREAT_PERSON_CLASS_GENERAL'}
          GameInfo.GreatPersonIndividuals=setmetatable({[42]=individual},{__call=function()
            local done=false;return function() if not done then done=true;return individual end end end})
          for _,city in pairs(CITIES) do city.GetReligion=function(self) return {
            GetReligionsInCity=function() local list={};for id,count in pairs(self.faith) do list[#list+1]={Religion=tonumber(id),Followers=count} end;return list end,
            AddReligiousPressure=function(_,_,id,pressure,owner) self.faith[tostring(id)]=self.pop end} end end
        ''')
        self.run_lua(steps[0]["code"])
        self.run_lua(steps[1]["code"])
        before = self.run_lua(steps[2]["code"])
        self.load_scripts()
        self.assertEqual(self.run_lua(steps[3]["code"]), before)
        self.g.CURRENT_TURN = 6
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
        self.assertTrue(self.a.Verify("test-absorption", True))

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
        self.assertTrue(self.a.Verify("test-absorption", True))

    def test_previous_governance_is_saved_before_engine_clears_city_properties(self):
        self.run_lua("OLD_GOV={ID='prior',From=3,To=1,Turn=2,Previous={ID='older',From=0,To=3,Turn=1}};CITIES[4]:SetProperty('JAC_ABSORPTION_GOVERNANCE_V1',OLD_GOV)")
        self.apply()
        self.run_lua("assert(JAC_AbsorptionRules.Equal(CITIES[4]:GetProperty('JAC_ABSORPTION_GOVERNANCE_V1').Previous,OLD_GOV))")
        self.load_scripts()
        self.assertTrue(self.a.Verify("test-absorption", True))


class AbsorptionPackageTests(unittest.TestCase):
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
        captures = {step["capture_as"] for step in steps if "capture_as" in step}
        self.assertEqual(captures, {"ABSORPTION_IMMEDIATE", "ABSORPTION_AFTER_AI_TURN"})
        self.assertEqual({step["equals_capture"] for step in steps if "equals_capture" in step}, captures)
        self.assertEqual(sum(step["action"] == "save" for step in steps), 2)
        self.assertEqual(sum(step["action"] == "load" for step in steps), 2)
        configure = next(step for step in steps if step["action"] == "configure")
        self.assertEqual(configure["map_file"], "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map")
        ai = {p["civ"] for p in configure["players"] if not p["human"]}
        self.assertTrue({"CIVILIZATION_JAC_ASHINA", "CIVILIZATION_JAC_DATE"} <= ai)
        end = next(step for step in steps if step["action"] == "end_turn")
        self.assertEqual(end["turns"], 1)

    def test_modinfo_references_exist_and_have_no_open_pr_dependencies(self):
        root = ET.parse(MOD / "JAC_PoC_Absorption.modinfo").getroot()
        files = {n.text for n in root.findall("./Files/File")}
        self.assertEqual(files, {"Data/JAC_Absorption.sql", "Text/JAC_Absorption_Text.xml",
                                 "Scripts/JAC_Absorption_Rules.lua", "Scripts/JAC_Absorption_Gameplay.lua"})
        for name in files:
            self.assertTrue((MOD / name).is_file(), name)
        self.assertEqual({n.get("id") for n in root.findall("./Dependencies/Mod")},
                         {"4873eb62-8ccc-4574-b784-dda455e74e68", "bd7a82c5-6b38-4b69-a7f5-8dc9af534249"})
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
