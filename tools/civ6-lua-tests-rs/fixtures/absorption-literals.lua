-- Verbatim string literals from tests/test_absorption_lua.py (8ff592e).
-- Lua fragments retain every original assert; f-string substitutions happen at execution.
return {
  ["module"]={
    -- source line 1, literal 1
    [==[SSOT 9.1 absorption, using independent engine doubles and a real Lua VM (never skipped).]==],
    -- source line 11, literal 2
    [==[poc/JAC_PoC_Absorption]==],
    -- source line 13, literal 3
    [==[
local outcomes = ...
local function rows(list) local i=0;return function() i=i+1;return list[i] end end
GameInfo = {
  Districts = {[0]={Index=0,DistrictType='DISTRICT_CITY_CENTER'},
    [1]={Index=1,DistrictType='DISTRICT_HOLY_SITE'}},
  JAC_AbsorptionOutcomes = function() return rows(outcomes) end,
  UnitPromotions = function() return rows({{Index=7},{Index=9}}) end,
  Buildings = setmetatable({BUILDING_PALACE={Index=0,BuildingType='BUILDING_PALACE'}},
    {__call=function() return rows({{Index=0,BuildingType='BUILDING_PALACE'},{Index=1,BuildingType='BUILDING_TEST'}}) end}),
  Units = {[0]={UnitType="UNIT_WARRIOR"},[1]={UnitType="UNIT_GREAT_GENERAL"},[2]={UnitType="UNIT_MISSIONARY"},
           [3]={UnitType="UNIT_BUILDER"},[4]={UnitType="UNIT_HERO_JAC_WARLORD_A"},[5]={UnitType="UNIT_HERO_JAC_WARLORD_B"}},
  HeroClasses = setmetatable({[0]={Index=0,HeroClassType='HEROCLASS_JAC_WARLORD_A',UnitType='UNIT_HERO_JAC_WARLORD_A'},
    [1]={Index=1,HeroClassType='HEROCLASS_JAC_WARLORD_B',UnitType='UNIT_HERO_JAC_WARLORD_B'}},
    {__call=function(t) return rows({t[0],t[1]}) end}),
  GreatPersonIndividuals = {[42]={Index=42,GreatPersonClassType='GREAT_PERSON_CLASS_GENERAL',ActionCharges=2}},
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
PlayerManager={GetAlive=function()
  local result={};for _,player in pairs(Players) do
    if player.IsAlive and player:IsAlive() then result[#result+1]=player end
  end
  table.sort(result,function(a,b) return a:GetID()<b:GetID() end)
  return result
end}
function UNIT(owner,id,kind,x)
  local u=prop({owner=owner,id=id,kind=TYPE[kind],x=x or 4,y=0,damage=0,moves=2,xp=0,individual=-1,
    promotions={},formation=0,gpcharges=2,build=kind=="UNIT_BUILDER" and 3 or 0,spread=kind=="UNIT_MISSIONARY" and 3 or 0,religion=-1,
    heroClass=kind=='UNIT_HERO_JAC_WARLORD_A' and 0 or kind=='UNIT_HERO_JAC_WARLORD_B' and 1 or -1,lifespan=30,
    charges=kind:match('^UNIT_HERO_') and 6 or 0,renown=0})
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
  city.districts={[0]={X=x,Y=0}}
  city.GetDistricts=function() return {Members=function()
    local list={};for id,d in pairs(city.districts) do
      local kind,position=id,d
      list[#list+1]={GetType=function() return position.Type or kind end,
        GetX=function() return position.X end,GetY=function() return position.Y end}
    end
    return ipairs(list)
  end,GetDistrictLocation=function(_,id)
    local d=city.districts[id];if d then return d.X,d.Y end
  end} end
  city.GetBuildings=function() return {
    HasBuilding=function(_,id) return (id==1 and city.monument) or (id==0 and city.palace==true) end,IsPillaged=function() return false end,
    GetNumGreatWorkSlots=function() return 1 end,
    -- Every city has its own native work index, as in a real save.
    GetGreatWorkInSlot=function(_,id) return id==0 and (city.palaceWork or -1) or 1007+city.x end
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
]==],
    -- source line 235, literal 4
    [==[:memory:]==],
    -- source line 237, literal 5
    [==[CREATE TABLE Units (UnitType TEXT PRIMARY KEY, CanTrain INTEGER)]==],
    -- source line 238, literal 6
    [==[Data/JAC_Absorption.sql]==],
    -- source line 238, literal 7
    [==[utf-8]==],
    -- source line 239, literal 8
    [==[Kind]==],
    -- source line 239, literal 9
    [==[Outcome]==],
    -- source line 239, literal 10
    [==[Destination]==],
    -- source line 240, literal 11
    [==[SELECT Kind,Outcome,Destination FROM JAC_AbsorptionOutcomes]==],
  },
  ["AbsorptionTests.load_scripts"]={
    -- source line 253, literal 1
    [==[Scripts/JAC_Absorption_Rules.lua]==],
    -- source line 253, literal 2
    [==[utf-8]==],
    -- source line 255, literal 3
    [==[Scripts/JAC_Absorption_Gameplay.lua]==],
    -- source line 255, literal 4
    [==[utf-8]==],
  },
  ["AbsorptionTests.verify"]={
    -- source line 266, literal 1
    [==[if UI_PUBLISH then UI_PUBLISH()
              elseif PUBLISH_HERO_UI then PUBLISH_HERO_UI()
              else error('UI publication fixture missing') end]==],
  },
  ["AbsorptionTests.snapshot"]={
    -- source line 272, literal 1
    [==[local a=ExposedMembers.JAC_Absorption;local result={State=a.GetState(),Owners={},Cities={},Units={},
          HeroClaims=HERO_CLAIMS,HeroOrigins=HERO_ORIGINS,HeroDiscoveries=HERO_DISCOVERED}
          for owner=0,3 do result.Owners[owner]=a.GetRoster(owner);result.Units[owner]={}
            for id,u in Players[owner]:GetUnits():Members() do
              result.Units[owner][id]={Owner=u.owner,Type=u.kind,Individual=u.individual,Damage=u.damage,
                Moves=u.moves,XP=u.xp,Promotions=u.promotions,Properties=u.props,
                HeroClass=u.heroClass,Lifespan=u.lifespan,ActionCharges=u.charges,Renown=u.renown} end end
          for x,c in pairs(CITIES) do result.Cities[x]={Owner=c:GetOwner(),Faith=a.CityReligion(c),Properties=c.props} end
          return a.Canonical(result)]==],
  },
  ["AbsorptionTests.test_all_assets_and_native_components_transfer_before_last_city_elimination"]={
    -- source line 292, literal 1
    [==[COMPLETE]==],
    -- source line 295, literal 2
    [==[local a=ExposedMembers.JAC_Absorption;local r=a.GetRoster(2)
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
          assert(a.FindUnit(2,'test-absorption:unit:10').religion==5)]==],
  },
  ["AbsorptionTests.test_save_reload_discards_lua_session_and_restores_properties_without_replaying"]={
    -- source line 315, literal 1
    [==[local copy=JAC_AbsorptionRules.Copy
          Game.props=copy(Game.props);for _,p in pairs(Players) do p.props=copy(p.props)
            for _,u in pairs(p.units) do u.props=copy(u.props) end
            for _,c in pairs(p.cities) do c.props=copy(c.props) end end
          ExposedMembers={};JAC_AbsorptionRules=nil]==],
    -- source line 322, literal 2
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_non_transfer_branches_keep_original_snapshots_and_detach_person_references"]={
    -- source line 327, literal 1
    [==[PLAN.Units['1:3']='RETIRE';PLAN.Units['1:8']='RETIRE';PLAN.Units['1:11']='HIDE';PLAN.Assets.hero='HIDE'
          PLAN.Units['1:12']='RETIRE';PLAN.Assets.person='RETIRE';PLAN.Assets.artifact='SCATTER';PLAN.Assets.unlock='PRESERVE';PLAN.Leader='EXILE' ]==],
    -- source line 330, literal 2
    [==[test-absorption]==],
    -- source line 331, literal 3
    [==[local s=ExposedMembers.JAC_Absorption.GetState();local ar=s.Archive
          assert(ar.hero.Owner==-1 and ar.hero.Lifespan==19 and ar.hero.Unit==nil and ar.hero.FormerUnit.ID==11)
          assert(ar.person.Outcome=='RETIRE' and ar.artifact.Outcome=='SCATTER' and ar['leader:1'].LeaderType=='LEADER_TEST_1')
          local retired=ar['native-person:1:8']
          assert(retired.Owner==-1 and retired.Outcome=='RETIRE' and retired.NativeIndividual==42 and retired.Unit==nil)
          assert(s.Transactions['test-absorption'].Units[1].Properties.JAC_POC_GROWTHUI_WARLORD.Level==4)
          assert(not Players[2]:GetTechs():HasTech(11) and not Players[2]:GetCulture():HasCivic(12))]==],
  },
  ["AbsorptionTests.test_returned_belonging_goes_to_explicit_surviving_third_party"]={
    -- source line 340, literal 1
    [==[PLAN.Assets.artifact={Outcome='RETURN',ReturnOwner=3}]==],
    -- source line 343, literal 2
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_every_spec_disposition_is_routable_and_rejects_cross_kind_outcomes"]={
    -- source line 347, literal 1
    [==[UNIT]==],
    -- source line 347, literal 2
    [==[SURRENDER]==],
    -- source line 347, literal 3
    [==[INCORPORATE]==],
    -- source line 347, literal 4
    [==[DISSOLVE]==],
    -- source line 347, literal 5
    [==[ESCAPE]==],
    -- source line 347, literal 6
    [==[RESIST]==],
    -- source line 348, literal 7
    [==[PERSON]==],
    -- source line 348, literal 8
    [==[CAPTURE]==],
    -- source line 348, literal 9
    [==[RECRUIT]==],
    -- source line 348, literal 10
    [==[SUBMIT]==],
    -- source line 348, literal 11
    [==[EXILE]==],
    -- source line 348, literal 12
    [==[EXECUTE]==],
    -- source line 348, literal 13
    [==[RETIRE]==],
    -- source line 348, literal 14
    [==[DEFECT]==],
    -- source line 349, literal 15
    [==[HERO]==],
    -- source line 349, literal 16
    [==[RECRUIT]==],
    -- source line 349, literal 17
    [==[PROTECT]==],
    -- source line 349, literal 18
    [==[RECONTRACT]==],
    -- source line 349, literal 19
    [==[DEFECT]==],
    -- source line 349, literal 20
    [==[HIDE]==],
    -- source line 349, literal 21
    [==[SUICIDE]==],
    -- source line 350, literal 22
    [==[GREAT_PERSON]==],
    -- source line 350, literal 23
    [==[RECRUIT]==],
    -- source line 350, literal 24
    [==[EXILE]==],
    -- source line 350, literal 25
    [==[RETIRE]==],
    -- source line 351, literal 26
    [==[ITEM]==],
    -- source line 351, literal 27
    [==[TRANSFER]==],
    -- source line 351, literal 28
    [==[LOOT]==],
    -- source line 351, literal 29
    [==[SEIZE]==],
    -- source line 351, literal 30
    [==[RETURN]==],
    -- source line 351, literal 31
    [==[SCATTER]==],
    -- source line 352, literal 32
    [==[UNLOCK]==],
    -- source line 352, literal 33
    [==[INHERIT]==],
    -- source line 352, literal 34
    [==[PRESERVE]==],
    -- source line 352, literal 35
    [==[RELIGION]==],
    -- source line 352, literal 36
    [==[PRESERVE]==],
    -- source line 356, literal 37
    [==[Kind]==],
    -- source line 356, literal 38
    [==[Outcome]==],
    -- source line 357, literal 39
    [==[Kind]==],
    -- source line 357, literal 40
    [==[Outcome]==],
    -- source line 357, literal 41
    [==[Destination]==],
    -- source line 359, literal 42
    [==[PLAN.Assets.hero='EXECUTE']==],
    -- source line 359, literal 43
    [==[invalid disposition]==],
  },
  ["AbsorptionTests.test_great_people_cannot_be_converted_to_yields"]={
    -- source line 362, literal 1
    [==[RESEARCH]==],
    -- source line 362, literal 2
    [==[FAITH]==],
    -- source line 362, literal 3
    [==[CULTURE]==],
    -- source line 364, literal 4
    [==[PLAN.Units['1:8']='{outcome}']==],
    -- source line 364, literal 5
    [==[invalid disposition]==],
  },
  ["AbsorptionTests.test_missing_unit_decision_is_rejected_before_any_mutation"]={
    -- source line 367, literal 1
    [==[PLAN.Units['1:3']=nil]==],
    -- source line 367, literal 2
    [==[explicit disposition]==],
  },
  ["AbsorptionTests.test_missing_asset_decision_is_rejected_before_any_mutation"]={
    -- source line 370, literal 1
    [==[PLAN.Assets.artifact=nil]==],
    -- source line 370, literal 2
    [==[explicit disposition]==],
  },
  ["AbsorptionTests.test_inconsistent_linked_person_and_unit_decisions_are_rejected"]={
    -- source line 373, literal 1
    [==[PLAN.Assets.hero='HIDE']==],
    -- source line 373, literal 2
    [==[dispositions disagree]==],
  },
  ["AbsorptionTests.test_coexistence_is_not_absorption"]={
    -- source line 376, literal 1
    [==[MUTUAL_PROTECTION]==],
    -- source line 376, literal 2
    [==[VASSAL]==],
    -- source line 376, literal 3
    [==[COLONY]==],
    -- source line 376, literal 4
    [==[FEDERATION]==],
    -- source line 376, literal 5
    [==[DIRECT_RULE]==],
    -- source line 376, literal 6
    [==[AUTONOMY]==],
    -- source line 378, literal 7
    [==[PLAN.Mode='{mode}']==],
    -- source line 378, literal 8
    [==[coexistence]==],
  },
  ["AbsorptionTests.test_entry_and_external_relations_require_explicit_resolution"]={
    -- source line 381, literal 1
    [==[PLAN.RelationsResolved=false]==],
    -- source line 381, literal 2
    [==[third-party]==],
  },
  ["AbsorptionTests.test_asset_collision_is_rejected_before_engine_changes"]={
    -- source line 384, literal 1
    [==[Players[2].props.JAC_ABSORPTION_ASSETS_V1.hero={ID='hero',Kind='HERO',Owner=2}]==],
    -- source line 384, literal 2
    [==[collision]==],
  },
  ["AbsorptionTests.test_invalid_return_is_rejected"]={
    -- source line 387, literal 1
    [==[PLAN.Assets.artifact={Outcome='RETURN',ReturnOwner=1}]==],
    -- source line 387, literal 2
    [==[third-party]==],
  },
  ["AbsorptionTests.add_native_hero"]={
    -- source line 390, literal 1
    [==[
          local u=UNIT(1,13,'UNIT_HERO_JAC_WARLORD_A');u.lifespan=17;u.charges=2;u.renown=13;u.damage=7;u.moves=1
          HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=CITIES[4].id};HERO_DISCOVERED['1:0']=true
          local person={ID='native',Kind='HERO',Owner=1,Unit={Owner=1,ID=13},Contract='RECONTRACT',
            Loyalty=71,Lifespan=17,Relations={friend=29},Equipment={'SPEAR'}}
          u:SetProperty('JAC_ABSORPTION_PERSON_V1',person)
          Players[1].props.JAC_ABSORPTION_ASSETS_V1.native=person
          PLAN.Units['1:13']='RECONTRACT';PLAN.Assets.native='RECONTRACT'
        ]==],
    -- source line 400, literal 2
    [==[
              local u=UNIT(1,14,'UNIT_HERO_JAC_WARLORD_B');u.lifespan=21;u.charges=0;u.renown=9
              HERO_CLAIMS[1]=1;HERO_ORIGINS[1]={player=1,id=CITIES[8].id};HERO_DISCOVERED['1:1']=true
              PLAN.Units['1:14']='PROTECT'
            ]==],
  },
  ["AbsorptionTests.assert_native_suicide"]={
    -- source line 406, literal 1
    [==[RECONTRACT]==],
    -- source line 408, literal 2
    [==[COMPLETE]==],
    -- source line 409, literal 3
    [==[
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
        ]==],
  },
  ["AbsorptionTests.split_hero_contexts"]={
    -- source line 436, literal 1
    [==[local full=Game.GetHeroesManager()
          UI_HERO_MANAGER={GetHeroClaimPlayer=full.GetHeroClaimPlayer,
            GetHeroOriginCityID=full.GetHeroOriginCityID,
            GetPlayerHeroOriginBuildingType=full.GetPlayerHeroOriginBuildingType}
          CORE_HERO_MANAGER={CreateHero=full.CreateHero,SetHeroDiscovered=full.SetHeroDiscovered,
            IsHeroDiscovered=full.IsHeroDiscovered}
          Game.GetHeroesManager=function() return UI_HERO_MANAGER end
          Events={LoadScreenClose={Add=function() end},GameCoreEventPublishComplete={Add=function(f) PUBLISH_HERO_UI=f end}}]==],
    -- source line 444, literal 2
    [==[UI/JAC_Absorption_HeroKeys.lua]==],
    -- source line 444, literal 3
    [==[utf-8]==],
    -- source line 445, literal 4
    [==[Game.GetHeroesManager=function() return CORE_HERO_MANAGER end
          assert(ExposedMembers.JAC_HeroReadAPI.CreateHero==nil)
          assert(ExposedMembers.JAC_HeroReadAPI.SetHeroDiscovered==nil)]==],
  },
  ["AbsorptionTests.split_unit_contexts"]={
    -- source line 452, literal 1
    [==[CORE_PLAYERS,CORE_GAME=Players,Game
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
        ]==],
    -- source line 480, literal 2
    [==[UI/JAC_Absorption_HeroKeys.lua]==],
    -- source line 480, literal 3
    [==[utf-8]==],
    -- source line 481, literal 4
    [==[Game,Players=CORE_GAME,CORE_PLAYERS
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
        ]==],
  },
  ["AbsorptionTests.finish_spread_publications"]={
    -- source line 496, literal 1
    [==[UI_PUBLISH()]==],
    -- source line 497, literal 2
    [==[test-absorption]==],
    -- source line 498, literal 3
    [==[COMPLETE]==],
    -- source line 500, literal 4
    [==[spread-charge UI publications did not finish]==],
  },
  ["AbsorptionTests.test_native_clones_stage_at_recipient_city_then_return_to_original_positions"]={
    -- source line 503, literal 1
    [==[local spawn=UnitManager.InitUnitValidAdjacentHex
          UnitManager.InitUnitValidAdjacentHex=function(owner,kind,x,y,radius)
            assert(owner==2 and x==12 and y==0 and KILLS==0 and TRANSFERS==0)
            return spawn(owner,kind,x,y,radius)
          end
          local manager=Game.GetGreatPeople();local create=manager.CreatePerson
          manager.CreatePerson=function(self,owner,individual,x,y)
            assert(owner==2 and x==12 and y==0 and KILLS==0 and TRANSFERS==0)
            return create(self,owner,individual,x,y)
          end
          Game.GetGreatPeople=function() return manager end]==],
    -- source line 515, literal 2
    [==[COMPLETE]==],
    -- source line 516, literal 3
    [==[for _,s in ipairs(ExposedMembers.JAC_Absorption.GetState().Transactions['test-absorption'].Units) do
          if s.KeepUnit then local u=ExposedMembers.JAC_Absorption.FindUnit(s.Destination,s.Token)
            assert(u:GetX()==s.X and u:GetY()==s.Y) end
        end]==],
  },
  ["AbsorptionTests.test_large_army_batches_publication_requests_instead_of_waiting_per_unit"]={
    -- source line 522, literal 1
    [==[for id=1000,1019 do UNIT(1,id,'UNIT_WARRIOR',4,0);PLAN.Units['1:'..id]='INCORPORATE' end]==],
    -- source line 526, literal 2
    [==[COMPLETE]==],
    -- source line 527, literal 3
    [==[UI_PUBLISH()]==],
    -- source line 528, literal 4
    [==[test-absorption]==],
    -- source line 530, literal 5
    [==[COMPLETE]==],
    -- source line 532, literal 6
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_off_map_native_candidates_are_placed_before_transfer_and_verified"]={
    -- source line 535, literal 1
    [==[local create=UNIT
          UNIT=function(...) local u=create(...);u.x=-9999;u.y=-9999;return u end]==],
    -- source line 538, literal 2
    [==[COMPLETE]==],
    -- source line 541, literal 3
    [==[local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3')
          assert(u:GetX()==4 and u:GetY()==0);u.x=-9999;u.y=-9999]==],
    -- source line 543, literal 4
    [==[off-map]==],
  },
  ["AbsorptionTests.test_failed_off_map_placement_keeps_donors_and_reuses_saved_candidate_on_resume"]={
    -- source line 547, literal 1
    [==[local create=UNIT
          UNIT=function(...) local u=create(...);u.x=-9999;u.y=-9999;return u end
          ORIGINAL_PLACE=UnitManager.PlaceUnit;UnitManager.PlaceUnit=function() end]==],
    -- source line 550, literal 2
    [==[placement failed; saved candidate retained]==],
    -- source line 552, literal 3
    [==[test-absorption:unit:3]==],
    -- source line 552, literal 4
    [==[test-absorption:unit:3]==],
    -- source line 554, literal 5
    [==[UnitManager.PlaceUnit=ORIGINAL_PLACE]==],
    -- source line 555, literal 6
    [==[test-absorption]==],
    -- source line 556, literal 7
    [==[COMPLETE]==],
    -- source line 558, literal 8
    [==[test-absorption:unit:3]==],
  },
  ["AbsorptionTests.test_empty_army_still_waits_for_city_cache_publication_before_preparation"]={
    -- source line 564, literal 1
    [==[Players[1].units={};Players[1].props.JAC_ABSORPTION_ASSETS_V1={};PLAN.Units={};PLAN.Assets={}
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
          CITIES[4].faith['5']=5]==],
    -- source line 585, literal 2
    [==[WAITING_UI]==],
    -- source line 588, literal 3
    [==[test-absorption]==],
    -- source line 588, literal 4
    [==[WAITING_UI]==],
    -- source line 589, literal 5
    [==[CITY_PUBLISH()]==],
    -- source line 590, literal 6
    [==[test-absorption]==],
    -- source line 591, literal 7
    [==[5]==],
    -- source line 592, literal 8
    [==[WAITING_UI]==],
    -- source line 593, literal 9
    [==[CITY_PUBLISH()]==],
    -- source line 594, literal 10
    [==[test-absorption]==],
    -- source line 594, literal 11
    [==[COMPLETE]==],
  },
  ["AbsorptionTests.test_public_verification_requires_a_new_publication_and_consumes_it"]={
    -- source line 600, literal 1
    [==[verification requires a published UI request]==],
    -- source line 601, literal 2
    [==[test-absorption]==],
    -- source line 602, literal 3
    [==[test-absorption]==],
    -- source line 603, literal 4
    [==[verification requires a published UI request]==],
    -- source line 604, literal 5
    [==[test-absorption]==],
    -- source line 605, literal 6
    [==[UI_PUBLISH()]==],
    -- source line 606, literal 7
    [==[test-absorption]==],
    -- source line 607, literal 8
    [==[verification requires a published UI request]==],
    -- source line 608, literal 9
    [==[test-absorption]==],
    -- source line 609, literal 10
    [==[ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:10').spread=1]==],
    -- source line 610, literal 11
    [==[test-absorption]==],
    -- source line 611, literal 12
    [==[UI_PUBLISH()]==],
    -- source line 612, literal 13
    [==[spent spread charges]==],
    -- source line 613, literal 14
    [==[test-absorption]==],
    -- source line 614, literal 15
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_ui_only_purchase_cost_and_faith_balance_return_scalars_to_core"]={
    -- source line 618, literal 1
    [==[MilitaryFormationTypes={STANDARD_MILITARY_FORMATION=0}
          for owner=0,3 do
            local n=owner
            UI_PLAYERS[n].GetCities=function() return {FindID=function(_,id)
              return {GetGold=function() return {GetPurchaseCost=function(_,yield,hash,formation)
                assert(yield==6 and hash==104 and formation==0);return 1000 end} end}
            end} end
            UI_PLAYERS[n].GetReligion=function() return {GetFaithBalance=function() return FAITH[n] end} end
          end
          Game,Players=UI_GAME,UI_PLAYERS]==],
    -- source line 628, literal 2
    [==[UI/JAC_Absorption_HeroKeys.lua]==],
    -- source line 628, literal 3
    [==[utf-8]==],
    -- source line 629, literal 4
    [==[Game,Players=CORE_GAME,CORE_PLAYERS;FAITH[2]=777
          for _,c in pairs(CITIES) do c.GetGold=nil end
          for _,p in pairs(Players) do p.GetReligion=function() return {} end end]==],
  },
  ["AbsorptionTests.test_ui_only_spread_getter_waits_for_publication_and_reuses_candidates"]={
    -- source line 638, literal 1
    [==[WAITING_UI]==],
    -- source line 641, literal 2
    [==[UI_PUBLISH()]==],
    -- source line 642, literal 3
    [==[test-absorption]==],
    -- source line 643, literal 4
    [==[WAITING_UI]==],
    -- source line 645, literal 5
    [==[local tx=ExposedMembers.JAC_Absorption.GetState().Transactions['test-absorption']
          for _,snap in ipairs(tx.Units) do
            if snap.ID>=0 then
              assert(type(snap.SpreadCharges)=='number')
              if snap.ID==10 then assert(snap.SpreadCharges==3) else assert(snap.SpreadCharges==0) end
            end
          end
          assert(tx.Units[1].SpreadReads.final.AwaitingUI)
        ]==],
    -- source line 655, literal 6
    [==[test-absorption]==],
    -- source line 655, literal 7
    [==[WAITING_UI]==],
    -- source line 656, literal 8
    [==[PUBLISH_HERO_UI()]==],
    -- source line 657, literal 9
    [==[test-absorption]==],
    -- source line 657, literal 10
    [==[WAITING_UI]==],
    -- source line 658, literal 11
    [==[UI_SYNC();UI_TURN=CURRENT_TURN-1;PUBLISH_HERO_UI()]==],
    -- source line 659, literal 12
    [==[test-absorption]==],
    -- source line 659, literal 13
    [==[WAITING_UI]==],
    -- source line 662, literal 14
    [==[COMPLETE]==],
    -- source line 664, literal 15
    [==[test-absorption]==],
    -- source line 665, literal 16
    [==[local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:10')
          u.spread=1]==],
    -- source line 667, literal 17
    [==[spent spread charges]==],
    -- source line 668, literal 18
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_ui_only_spread_mismatch_blocks_and_preserves_donors_until_resume"]={
    -- source line 671, literal 1
    [==[Players[1].units[10].spread=1]==],
    -- source line 673, literal 2
    [==[WAITING_UI]==],
    -- source line 674, literal 3
    [==[UI_PUBLISH()]==],
    -- source line 675, literal 4
    [==[test-absorption]==],
    -- source line 675, literal 5
    [==[WAITING_UI]==],
    -- source line 676, literal 6
    [==[local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:10')
          MISSIONARY_CANDIDATE=u:GetID();assert(u.spread==3)]==],
    -- source line 679, literal 7
    [==[UI_PUBLISH()]==],
    -- source line 681, literal 8
    [==[test-absorption]==],
    -- source line 683, literal 9
    [==[spent spread charges]==],
    -- source line 685, literal 10
    [==[WAITING_UI]==],
    -- source line 687, literal 11
    [==[UI spread mismatch was not detected]==],
    -- source line 689, literal 12
    [==[test-absorption]==],
    -- source line 689, literal 13
    [==[BLOCKED]==],
    -- source line 690, literal 14
    [==[assert(Players[1].units[10] and Players[1].units[3])
          assert(ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:10'):GetID()==MISSIONARY_CANDIDATE)
          ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:10').spread=1]==],
    -- source line 694, literal 15
    [==[COMPLETE]==],
    -- source line 696, literal 16
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_old_turn_ui_spread_cache_waits_before_preparation_and_resumes"]={
    -- source line 700, literal 1
    [==[UI_TURN=CURRENT_TURN-1]==],
    -- source line 702, literal 2
    [==[WAITING_UI]==],
    -- source line 705, literal 3
    [==[local s=Game:GetProperty('JAC_ABSORPTION_STATE_V1')
          s.Transactions['test-absorption'].SourceSpreadReads=nil;Game:SetProperty('JAC_ABSORPTION_STATE_V1',s)]==],
    -- source line 708, literal 4
    [==[test-absorption]==],
    -- source line 708, literal 5
    [==[WAITING_UI]==],
    -- source line 709, literal 6
    [==[UI_SYNC()]==],
    -- source line 710, literal 7
    [==[test-absorption]==],
    -- source line 710, literal 8
    [==[WAITING_UI]==],
    -- source line 712, literal 9
    [==[PUBLISH_HERO_UI()]==],
    -- source line 713, literal 10
    [==[test-absorption]==],
    -- source line 713, literal 11
    [==[WAITING_UI]==],
    -- source line 715, literal 12
    [==[COMPLETE]==],
  },
  ["AbsorptionTests.test_same_turn_spent_spread_is_published_before_snapshot_and_never_restored_as_unused"]={
    -- source line 719, literal 1
    [==[Players[1].units[10].spread=1]==],
    -- source line 724, literal 2
    [==[test-absorption]==],
    -- source line 724, literal 3
    [==[WAITING_UI]==],
    -- source line 726, literal 4
    [==[UI_PUBLISH()]==],
    -- source line 727, literal 5
    [==[test-absorption]==],
    -- source line 728, literal 6
    [==[WAITING_UI]==],
    -- source line 731, literal 7
    [==[UI_PUBLISH()]==],
    -- source line 733, literal 8
    [==[test-absorption]==],
    -- source line 735, literal 9
    [==[spent spread charges]==],
    -- source line 738, literal 10
    [==[spent donor was incorrectly accepted as unused]==],
    -- source line 739, literal 11
    [==[test-absorption]==],
    -- source line 739, literal 12
    [==[BLOCKED]==],
  },
  ["AbsorptionTests.test_rejected_preparation_releases_factions_and_corrected_request_waits_for_new_publication"]={
    -- source line 745, literal 1
    [==[PLAN.Units['1:3']=nil]==],
    -- source line 749, literal 2
    [==[UI_PUBLISH()]==],
    -- source line 750, literal 3
    [==[explicit disposition required]==],
    -- source line 751, literal 4
    [==[test-absorption]==],
    -- source line 752, literal 5
    [==[test-absorption]==],
    -- source line 754, literal 6
    [==[PLAN.Units['1:3']='INCORPORATE';Players[1].units[10].spread=1]==],
    -- source line 758, literal 7
    [==[test-absorption]==],
    -- source line 758, literal 8
    [==[WAITING_UI]==],
    -- source line 760, literal 9
    [==[UI_PUBLISH()]==],
    -- source line 761, literal 10
    [==[test-absorption]==],
    -- source line 762, literal 11
    [==[WAITING_UI]==],
  },
  ["AbsorptionTests.test_native_hero_ui_only_spread_wait_never_becomes_suicide"]={
    -- source line 768, literal 1
    [==[WAITING_UI]==],
    -- source line 771, literal 2
    [==[UI_PUBLISH()]==],
    -- source line 772, literal 3
    [==[test-absorption]==],
    -- source line 773, literal 4
    [==[WAITING_UI]==],
    -- source line 776, literal 5
    [==[test-absorption]==],
    -- source line 776, literal 6
    [==[WAITING_UI]==],
    -- source line 778, literal 7
    [==[SUICIDE]==],
    -- source line 779, literal 8
    [==[assert(ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:13'))]==],
    -- source line 780, literal 9
    [==[COMPLETE]==],
    -- source line 783, literal 10
    [==[COMPLETE]==],
    -- source line 785, literal 11
    [==[RECONTRACT]==],
    -- source line 786, literal 12
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_published_hero_spread_mismatch_records_suicide_instead_of_blocking_other_assets"]={
    -- source line 795, literal 1
    [==[local u=assert(ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:13'))
                  u.spread=1]==],
    -- source line 798, literal 2
    [==[UI_PUBLISH()]==],
    -- source line 799, literal 3
    [==[test-absorption]==],
    -- source line 800, literal 4
    [==[COMPLETE]==],
    -- source line 803, literal 5
    [==[COMPLETE]==],
    -- source line 804, literal 6
    [==[SUICIDE]==],
    -- source line 805, literal 7
    [==[HERO_RESTORE_ERROR]==],
    -- source line 807, literal 8
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_split_ui_getters_and_gameplay_mutations_preserve_native_heroes"]={
    -- source line 812, literal 1
    [==[WAITING_UI]==],
    -- source line 814, literal 2
    [==[did not complete]==],
    -- source line 815, literal 3
    [==[test-absorption]==],
    -- source line 817, literal 4
    [==[test-absorption]==],
    -- source line 817, literal 5
    [==[WAITING_UI]==],
    -- source line 820, literal 6
    [==[PUBLISH_HERO_UI()]==],
    -- source line 821, literal 7
    [==[test-absorption]==],
    -- source line 822, literal 8
    [==[COMPLETE]==],
    -- source line 824, literal 9
    [==[test-absorption]==],
    -- source line 825, literal 10
    [==[local a=ExposedMembers.JAC_Absorption
          local u=assert(a.FindUnit(2,'test-absorption:unit:13'))
          local h=a.SnapshotUnit(u).Hero
          assert(h.ClaimOwner==2 and h.Origin.Owner==2 and h.Lifespan==17 and h.ActionCharges==2)
          assert(a.GetState().Transactions['test-absorption'].HeroResults.native.Outcome~='SUICIDE')]==],
  },
  ["AbsorptionTests.test_fresh_published_hero_mismatch_is_archived_without_accepting_stale_ui_as_success"]={
    -- source line 834, literal 1
    [==[WAITING_UI]==],
    -- source line 835, literal 2
    [==[HERO_CLAIMS[0]=1;PUBLISH_HERO_UI()]==],
    -- source line 836, literal 3
    [==[test-absorption]==],
    -- source line 837, literal 4
    [==[WAITING_UI]==],
    -- source line 838, literal 5
    [==[PUBLISH_HERO_UI()]==],
    -- source line 839, literal 6
    [==[test-absorption]==],
    -- source line 840, literal 7
    [==[COMPLETE]==],
    -- source line 841, literal 8
    [==[SUICIDE]==],
    -- source line 842, literal 9
    [==[HERO_MANAGER_MISMATCH]==],
    -- source line 843, literal 10
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_ui_only_city_citizens_and_work_slots_wait_for_final_publication"]={
    -- source line 847, literal 1
    [==[local full={}
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
          end]==],
    -- source line 863, literal 2
    [==[return ExposedMembers.JAC_Absorption.Canonical(ExposedMembers.JAC_Absorption.CityReligion(CITIES[4]))]==],
    -- source line 865, literal 3
    [==[WAITING_UI]==],
    -- source line 867, literal 4
    [==[PUBLISH_HERO_UI()]==],
    -- source line 868, literal 5
    [==[test-absorption]==],
    -- source line 869, literal 6
    [==[COMPLETE]==],
    -- source line 870, literal 7
    [==[return ExposedMembers.JAC_Absorption.Canonical(ExposedMembers.JAC_Absorption.CityReligion(CITIES[4]))]==],
    -- source line 872, literal 8
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_only_missing_work_slot_getter_still_requires_final_ui_publication"]={
    -- source line 875, literal 1
    [==[local full={}
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
        ]==],
    -- source line 890, literal 2
    [==[WAITING_UI]==],
    -- source line 892, literal 3
    [==[Published]==],
    -- source line 893, literal 4
    [==[test-absorption]==],
    -- source line 895, literal 5
    [==[test-absorption]==],
    -- source line 895, literal 6
    [==[WAITING_UI]==],
    -- source line 896, literal 7
    [==[UI_READY['test-absorption:final']={Published=true}]==],
    -- source line 897, literal 8
    [==[test-absorption]==],
    -- source line 897, literal 9
    [==[COMPLETE]==],
    -- source line 898, literal 10
    [==[test-absorption]==],
    -- source line 899, literal 11
    [==[Published]==],
    -- source line 900, literal 12
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_native_heroes_keep_identity_spent_charges_renown_origin_and_person_record"]={
    -- source line 906, literal 1
    [==[test-absorption]==],
    -- source line 907, literal 2
    [==[
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
        ]==],
    -- source line 923, literal 3
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_native_hero_absent_gameplay_manager_becomes_suicide_history"]={
    -- source line 930, literal 1
    [==[Game.GetHeroesManager=function() return nil end]==],
    -- source line 931, literal 2
    [==[NATIVE_MANAGER_UNAVAILABLE]==],
  },
  ["AbsorptionTests.test_dormant_native_claim_without_unit_is_archived_as_suicide"]={
    -- source line 934, literal 1
    [==[HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=CITIES[4].id}]==],
    -- source line 936, literal 2
    [==[COMPLETE]==],
    -- source line 937, literal 3
    [==[local a=ExposedMembers.JAC_Absorption
          local r=a.GetState().Archive['native-hero:1:HEROCLASS_JAC_WARLORD_A']
          assert(r and r.Outcome=='SUICIDE' and r.SuicideCause=='DORMANT_CLAIM_SETTER_UNAVAILABLE')
          assert(a.GetState().Transactions['test-absorption'].HeroResults[r.ID].Outcome=='SUICIDE')
          assert(r.NativeHero.ClaimOwner==1 and r.NativeHero.Origin.X==4 and r.NativeUnit==nil)
          assert(r.Unit==nil and a.GetRoster(2)[r.ID]==nil and a.Verify('test-absorption',true))
        ]==],
  },
  ["AbsorptionTests.test_dormant_claim_with_stale_person_reference_preserves_requested_outcome"]={
    -- source line 946, literal 1
    [==[
          HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=CITIES[4].id}
          Players[1].props.JAC_ABSORPTION_ASSETS_V1.dormant={ID='dormant',Kind='HERO',Owner=1,
            Unit={Owner=1,ID=777},NativeHero={Class=0,ClassType='HEROCLASS_JAC_WARLORD_A',Lifespan=9},
            Contract='PROTECT',Loyalty=74,Relations={friend=31}}
          PLAN.Assets.dormant='PROTECT'
        ]==],
    -- source line 954, literal 2
    [==[local a=ExposedMembers.JAC_Absorption
          local r=a.GetState().Archive.dormant
          assert(r.Outcome=='SUICIDE' and r.RequestedOutcome=='PROTECT' and r.SuicideCause=='DORMANT_CLAIM_SETTER_UNAVAILABLE')
          assert(r.FormerUnit.ID==777 and not r.Unit and r.Contract=='PROTECT' and r.Relations.friend==31)
          assert(r.NativeHero.ClaimOwner==1 and r.NativeHero.Origin.X==4)
          assert(r.HistoricalNativeHero.Lifespan==9)
        ]==],
  },
  ["AbsorptionTests.test_native_hero_missing_origin_city_becomes_suicide"]={
    -- source line 964, literal 1
    [==[HERO_ORIGINS[0].id=9999]==],
    -- source line 965, literal 2
    [==[ORIGIN_UNAVAILABLE]==],
  },
  ["AbsorptionTests.test_native_hero_missing_origin_building_becomes_suicide"]={
    -- source line 969, literal 1
    [==[CITIES[4].monument=false]==],
    -- source line 970, literal 2
    [==[ORIGIN_BUILDING_UNAVAILABLE]==],
  },
  ["AbsorptionTests.test_native_heroes_sharing_origin_archive_the_second_claim"]={
    -- source line 974, literal 1
    [==[HERO_ORIGINS[1].id=CITIES[4].id]==],
    -- source line 976, literal 2
    [==[local a=ExposedMembers.JAC_Absorption
          assert(a.GetRoster(2).native.Unit and a.GetState().Archive['native-person:1:14'].SuicideCause=='HERO_ORIGIN_COLLISION')
        ]==],
  },
  ["AbsorptionTests.test_native_hero_no_spawn_after_claim_change_finishes_as_suicide"]={
    -- source line 983, literal 1
    [==[HERO_CREATE_NO_UNIT]==],
  },
  ["AbsorptionTests.test_native_hero_init_throw_after_creation_adopts_candidate_without_recreating"]={
    -- source line 992, literal 1
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_native_hero_token_write_failure_kills_only_its_candidate"]={
    -- source line 997, literal 1
    [==[
          local create=HERO_MANAGER.CreateHero
          HERO_MANAGER.CreateHero=function(...)
            create(...)
            local u=Players[2].units[NEXT_ID-1];local set=u.SetProperty
            u.SetProperty=function(self,key,value)
              if key=='JAC_ABSORPTION_UNIT_TOKEN_V1' and not TOKEN_FAILED then TOKEN_FAILED=true;error('token interrupted') end
              set(self,key,value)
            end
          end
        ]==],
    -- source line 1008, literal 2
    [==[HERO_TOKEN_ERROR]==],
    -- source line 1010, literal 3
    [==[assert(#(function() local a={};for _,u in Players[2]:GetUnits():Members() do if u:IsHero() then a[#a+1]=u end end;return a end)()==0)]==],
  },
  ["AbsorptionTests.test_native_hero_wrong_origin_is_suicide_without_duplicate"]={
    -- source line 1015, literal 1
    [==[HERO_MANAGER_MISMATCH]==],
  },
  ["AbsorptionTests.test_native_hero_city_failure_can_resume_from_escrow_without_second_kill"]={
    -- source line 1021, literal 1
    [==[city failed]==],
    -- source line 1026, literal 2
    [==[test-absorption]==],
    -- source line 1027, literal 3
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_native_hero_manager_mismatch_is_not_reported_as_inheritance"]={
    -- source line 1033, literal 1
    [==[HERO_MANAGER_MISMATCH]==],
  },
  ["AbsorptionTests.test_hero_creation_error_does_not_kill_a_third_party_unit"]={
    -- source line 1038, literal 1
    [==[HERO_MANAGER.CreateHero=function(_,owner,class,x,y)
          HERO_CREATIONS=HERO_CREATIONS+1;HERO_CLAIMS[class]=3
          UNIT(3,777,'UNIT_HERO_JAC_WARLORD_A')
          error('third party candidate appeared')
        end]==],
    -- source line 1043, literal 2
    [==[HERO_CREATE_ERROR]==],
    -- source line 1044, literal 3
    [==[assert(Players[3].units[777] and Players[3].units[777]:IsHero())]==],
  },
  ["AbsorptionTests.test_reused_donor_id_blocks_without_killing_an_unrelated_unit"]={
    -- source line 1048, literal 1
    [==[local spawn=UnitManager.InitUnitValidAdjacentHex
          UnitManager.InitUnitValidAdjacentHex=function(...)
            if not DONOR_REPLACED then
              DONOR_REPLACED=true
              Players[1].units[13]=nil
              UNIT(1,13,'UNIT_BUILDER')
            end
            return spawn(...)
          end]==],
    -- source line 1057, literal 2
    [==[native Hero changed before escrow]==],
    -- source line 1059, literal 3
    [==[assert(Players[1].units[13] and not Players[1].units[13]:IsHero())]==],
  },
  ["AbsorptionTests.test_native_hero_restore_error_becomes_suicide_without_losing_other_assets"]={
    -- source line 1063, literal 1
    [==[
          local create=HERO_MANAGER.CreateHero
          HERO_MANAGER.CreateHero=function(...)
            create(...)
            local u=Players[2].units[NEXT_ID-1]
            u.ChangeActionCharges=function() error('hero charges unavailable') end
          end
        ]==],
    -- source line 1071, literal 2
    [==[HERO_RESTORE_ERROR]==],
    -- source line 1072, literal 3
    [==[local a=ExposedMembers.JAC_Absorption
          assert(a.GetRoster(2).equipment and a.GetRoster(2).artifact and a.GetRoster(2).person)
        ]==],
  },
  ["AbsorptionTests.test_native_hero_recall_binds_new_id_and_person_after_reload"]={
    -- source line 1079, literal 1
    [==[
          local a=ExposedMembers.JAC_Absorption
          local old=a.FindUnit(2,'test-absorption:unit:13')
          OLD_HERO_ID=old:GetID();UnitManager.Kill(old)
          assert(a.Verify('test-absorption',false))
          a.BeginHeroRecall('recall:1','test-absorption','native')
        ]==],
    -- source line 1087, literal 2
    [==[exactly one recalled]==],
    -- source line 1088, literal 3
    [==[recall:1]==],
    -- source line 1089, literal 4
    [==[HERO_MANAGER:CreateHero(2,0,4,0)]==],
    -- source line 1090, literal 5
    [==[recall:1]==],
    -- source line 1092, literal 6
    [==[local a=ExposedMembers.JAC_Absorption
          local r=a.GetRoster(2).native;local u=a.FindUnit(2,'recall:1:unit')
          assert(r.Unit.ID==u:GetID() and r.Unit.Token=='recall:1:unit' and r.FormerUnit.ID==OLD_HERO_ID)
          assert(r.Contract=='RECONTRACT' and r.Loyalty==71 and r.Relations.friend==29)
          assert(u:GetProperty('JAC_ABSORPTION_PERSON_V1').Unit.ID==u:GetID())
          assert(#r.RecallHistory==1 and a.Verify('test-absorption',false))
        ]==],
    -- source line 1100, literal 7
    [==[recall:1]==],
    -- source line 1101, literal 8
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_recall_after_second_absorption_preserves_original_owner_and_uses_current_mapping"]={
    -- source line 1107, literal 1
    [==[
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
        ]==],
    -- source line 1125, literal 2
    [==[second-recall]==],
    -- source line 1125, literal 3
    [==[second-absorption]==],
    -- source line 1125, literal 4
    [==[native]==],
    -- source line 1126, literal 5
    [==[HERO_MANAGER:CreateHero(3,0,4,0)]==],
    -- source line 1127, literal 6
    [==[second-recall]==],
    -- source line 1128, literal 7
    [==[
          local a=ExposedMembers.JAC_Absorption;local r=a.GetRoster(3).native
          local u=assert(a.FindUnit(3,r.Unit.Token))
          assert(r.Owner==3 and r.OriginalOwner==1 and r.Unit.ID==u:GetID())
          assert(r.Contract=='RECONTRACT' and r.Loyalty==71 and r.Relations.friend==29)
          assert(u:GetProperty(a.Rules.PERSON_KEY).OriginalOwner==1)
        ]==],
    -- source line 1136, literal 8
    [==[second-recall]==],
  },
  ["AbsorptionTests.test_recall_rejects_candidate_and_token_collisions_without_killing_them"]={
    -- source line 1142, literal 1
    [==[local a=ExposedMembers.JAC_Absorption
          UnitManager.Kill(a.FindUnit(2,'test-absorption:unit:13'))
          a.BeginHeroRecall('recall:collision','test-absorption','native')
          UNIT(3,777,'UNIT_HERO_JAC_WARLORD_A')
          UNIT(2,778,'UNIT_HERO_JAC_WARLORD_A')
          UNIT(2,779,'UNIT_HERO_JAC_WARLORD_A')
        ]==],
    -- source line 1149, literal 2
    [==[exactly one recalled]==],
    -- source line 1150, literal 3
    [==[recall:collision]==],
    -- source line 1151, literal 4
    [==[
          assert(Players[3].units[777] and Players[2].units[778] and Players[2].units[779])
          Players[2].units[779]=nil
          Players[2].units[778]:SetProperty('JAC_ABSORPTION_UNIT_TOKEN_V1','someone-else')
        ]==],
    -- source line 1156, literal 5
    [==[another transaction]==],
    -- source line 1157, literal 6
    [==[recall:collision]==],
    -- source line 1158, literal 7
    [==[assert(Players[3].units[777] and Players[2].units[778])]==],
  },
  ["AbsorptionTests.test_recall_resumes_after_token_write_and_does_not_duplicate_history"]={
    -- source line 1163, literal 1
    [==[local a=ExposedMembers.JAC_Absorption
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
        ]==],
    -- source line 1175, literal 2
    [==[recall token interrupted]==],
    -- source line 1176, literal 3
    [==[recall:retry]==],
    -- source line 1178, literal 4
    [==[recall:retry]==],
    -- source line 1180, literal 5
    [==[recall:retry:unit]==],
    -- source line 1181, literal 6
    [==[recall:retry]==],
  },
  ["AbsorptionTests.test_native_hero_checks_detect_corruption_instead_of_only_testing_unit_count"]={
    -- source line 1186, literal 1
    [==[u.lifespan=18]==],
    -- source line 1186, literal 2
    [==[lifespan changed]==],
    -- source line 1186, literal 3
    [==[u.charges=6]==],
    -- source line 1186, literal 4
    [==[action charges changed]==],
    -- source line 1187, literal 5
    [==[u.renown=0]==],
    -- source line 1187, literal 6
    [==[renown changed]==],
    -- source line 1187, literal 7
    [==[HERO_CLAIMS[0]=1]==],
    -- source line 1187, literal 8
    [==[manager owner changed]==],
    -- source line 1188, literal 9
    [==[HERO_ORIGINS[0]={player=2,id=CITIES[12].id}]==],
    -- source line 1188, literal 10
    [==[recall origin changed]==],
    -- source line 1189, literal 11
    [==[UNIT(2,999,'UNIT_HERO_JAC_WARLORD_A')]==],
    -- source line 1189, literal 12
    [==[duplicate native Hero]==],
    -- source line 1195, literal 13
    [==[local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:13');]==],
    -- source line 1197, literal 14
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_native_lifespan_changes_after_turn_but_owner_recall_identity_must_remain"]={
    -- source line 1203, literal 1
    [==[ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:13').lifespan=16]==],
    -- source line 1205, literal 2
    [==[test-absorption]==],
    -- source line 1206, literal 3
    [==[lifespan changed]==],
    -- source line 1207, literal 4
    [==[test-absorption]==],
    -- source line 1208, literal 5
    [==[HERO_CLAIMS[0]=1]==],
    -- source line 1209, literal 6
    [==[manager owner changed]==],
    -- source line 1210, literal 7
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_gameplay_scenario_checks_use_the_context_aware_spread_snapshot"]={
    -- source line 1213, literal 1
    [==[absorption-japan.json]==],
    -- source line 1213, literal 2
    [==[absorption-hero-recall.json]==],
    -- source line 1214, literal 3
    [==[tools/civ6-autotest/scenarios]==],
    -- source line 1214, literal 4
    [==[utf-8]==],
    -- source line 1215, literal 5
    [==[code]==],
    -- source line 1215, literal 6
    [==[steps]==],
    -- source line 1215, literal 7
    [==[state_name]==],
    -- source line 1215, literal 8
    [==[GameCore_Tuner]==],
    -- source line 1218, literal 9
    [==[:GetSpreadCharges(]==],
  },
  ["AbsorptionTests.test_all_gameplay_scenario_chunks_use_shared_lifespan_key"]={
    -- source line 1221, literal 1
    [==[absorption-japan.json]==],
    -- source line 1221, literal 2
    [==[absorption-hero-recall.json]==],
    -- source line 1222, literal 3
    [==[tools/civ6-autotest/scenarios]==],
    -- source line 1222, literal 4
    [==[utf-8]==],
    -- source line 1223, literal 5
    [==[steps]==],
    -- source line 1224, literal 6
    [==[state_name]==],
    -- source line 1224, literal 7
    [==[GameCore_Tuner]==],
    -- source line 1225, literal 8
    [==[code]==],
    -- source line 1225, literal 9
    [==[]==],
    -- source line 1226, literal 10
    [==[UnitPropertyKeys.LifespanRemaining]==],
    -- source line 1226, literal 11
    [==[(UnitPropertyKeys and UnitPropertyKeys.LifespanRemaining)]==],
    -- source line 1226, literal 12
    [==[]==],
    -- source line 1227, literal 13
    [==[ExposedMembers.JAC_HeroLifespanKey]==],
    -- source line 1227, literal 14
    [==[]==],
    -- source line 1227, literal 15
    [==[code]==],
    -- source line 1227, literal 16
    [==[]==],
    -- source line 1227, literal 17
    [==[steps]==],
  },
  ["AbsorptionTests.test_ui_publishes_engine_enum_when_gameplay_has_no_unit_property_keys"]={
    -- source line 1231, literal 1
    [==[poc/JAC_PoC_Absorption/UI/JAC_Absorption_HeroKeys.lua]==],
    -- source line 1231, literal 2
    [==[utf-8]==],
    -- source line 1232, literal 3
    [==[
          NATIVE_TEST_LIFE_KEY=UnitPropertyKeys.LifespanRemaining
          UnitPropertyKeys={LifespanRemaining=-1318296071}
          Events={LoadScreenClose={Add=function(f) HERO_KEYS_ON_LOAD=f end},GameCoreEventPublishComplete={Add=function() end}}
        ]==],
    -- source line 1239, literal 4
    [==[
          UnitPropertyKeys=nil
          local u=Players[1].units[13]
          u.GetProperty=function(self,key) if key==-1318296071 then return self.lifespan end;return self.props[key] end
          local snap=ExposedMembers.JAC_Absorption.SnapshotUnit(u)
          assert(snap.Hero.LifespanKey==-1318296071 and snap.Hero.Lifespan==17)
        ]==],
  },
  ["AbsorptionTests.test_shipped_lifespan_property_is_used_without_legacy_lifespan_or_renown_methods"]={
    -- source line 1249, literal 1
    [==[
          Players[1].units[13].GetRenown=nil;Players[1].units[13].SetRenown=nil
          Players[1].props.JAC_ABSORPTION_ASSETS_V1.native.Renown=13
          local create=HERO_MANAGER.CreateHero
          HERO_MANAGER.CreateHero=function(...)
            create(...);local u=Players[2].units[NEXT_ID-1];u.GetRenown=nil;u.SetRenown=nil
          end
        ]==],
    -- source line 1258, literal 2
    [==[
          local a=ExposedMembers.JAC_Absorption;local u=a.FindUnit(2,'test-absorption:unit:13')
          assert(u:GetProperty(UnitPropertyKeys.LifespanRemaining)==17 and u:GetActionCharges()==2)
          local r=a.GetRoster(2).native;assert(r.Renown==13 and r.NativeHero.RenownUnavailable and r.NativeHero.Renown==nil)
          assert(u.GetLifespan==nil and u.SetLifespan==nil and u.GetRenown==nil and u.SetRenown==nil)
        ]==],
    -- source line 1264, literal 3
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_native_hero_archival_and_explicit_suicide_preserve_requested_outcome"]={
    -- source line 1267, literal 1
    [==[HIDE]==],
    -- source line 1267, literal 2
    [==[DEFECT]==],
    -- source line 1267, literal 3
    [==[SUICIDE]==],
    -- source line 1272, literal 4
    [==[PLAN.Units['1:13']=NATIVE_OUTCOME;PLAN.Assets.native=NATIVE_OUTCOME]==],
    -- source line 1273, literal 5
    [==[EXPLICIT_SUICIDE]==],
    -- source line 1273, literal 6
    [==[SUICIDE]==],
    -- source line 1274, literal 7
    [==[REQUESTED_OUTCOME_UNTRANSFERABLE]==],
  },
  ["AbsorptionTests.test_native_hero_position_is_restored_separately_from_its_recall_origin"]={
    -- source line 1278, literal 1
    [==[Players[1].units[13].x=10]==],
    -- source line 1280, literal 2
    [==[local a=ExposedMembers.JAC_Absorption;local u=a.FindUnit(2,'test-absorption:unit:13')
          assert(u:GetX()==10 and u:GetY()==0 and u:GetDamage()==7 and u:GetMovesRemaining()==1)
          assert(HERO_ORIGINS[0].id==CITIES[4].id and HERO_ORIGINS[0].player==2)
        ]==],
    -- source line 1284, literal 3
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_native_hero_unrestorable_exposed_renown_becomes_suicide"]={
    -- source line 1288, literal 1
    [==[Players[1].units[13].SetRenown=nil]==],
    -- source line 1289, literal 2
    [==[RENOWN_UNRESTORABLE]==],
  },
  ["AbsorptionTests.test_native_hero_is_not_escrowed_when_an_ordinary_unit_cannot_preserve_spent_charges"]={
    -- source line 1293, literal 1
    [==[local u=UNIT(1,15,'UNIT_BUILDER');u.build=1;PLAN.Units['1:15']='INCORPORATE']==],
    -- source line 1294, literal 2
    [==[spent build charges]==],
  },
  ["AbsorptionTests.test_native_hero_duplicate_at_absorber_survives_suicide"]={
    -- source line 1301, literal 1
    [==[UNIT(2,999,'UNIT_HERO_JAC_WARLORD_A')]==],
    -- source line 1302, literal 2
    [==[HERO_CLASS_COLLISION]==],
    -- source line 1303, literal 3
    [==[assert(Players[2].units[999] and Players[2].units[999]:IsHero())]==],
  },
  ["AbsorptionTests.test_native_hero_city_transfer_throw_after_mutation_resumes_with_the_new_city_id"]={
    -- source line 1307, literal 1
    [==[
          local transfer=CityManager.TransferCity
          CityManager.TransferCity=function(city,owner)
            transfer(city,owner)
            if not CITY_THROWN then CITY_THROWN=true;error('city transfer interrupted after mutation') end
          end
        ]==],
    -- source line 1314, literal 2
    [==[interrupted after mutation]==],
    -- source line 1317, literal 3
    [==[test-absorption]==],
    -- source line 1318, literal 4
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_native_hero_token_set_then_throw_becomes_suicide"]={
    -- source line 1323, literal 1
    [==[
          local create=HERO_MANAGER.CreateHero
          HERO_MANAGER.CreateHero=function(...)
            create(...);local u=Players[2].units[NEXT_ID-1];local set=u.SetProperty
            u.SetProperty=function(self,key,value)
              set(self,key,value)
              if key=='JAC_ABSORPTION_UNIT_TOKEN_V1' and not TOKEN_THROWN then TOKEN_THROWN=true;error('after token write') end
            end
          end
        ]==],
    -- source line 1333, literal 2
    [==[HERO_TOKEN_ERROR]==],
  },
  ["AbsorptionTests.test_engine_spawn_failure_preserves_every_donor_unit_and_city_then_resume_is_idempotent"]={
    -- source line 1338, literal 1
    [==[did not create]==],
    -- source line 1341, literal 2
    [==[test-absorption]==],
    -- source line 1341, literal 3
    [==[BLOCKED]==],
    -- source line 1344, literal 4
    [==[test-absorption]==],
    -- source line 1345, literal 5
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_property_copy_failure_is_blocked_before_removing_any_donor"]={
    -- source line 1350, literal 1
    [==[copy failed]==],
    -- source line 1354, literal 2
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_spent_build_charges_need_an_adapter_and_never_destroy_the_donor"]={
    -- source line 1358, literal 1
    [==[local u=UNIT(1,13,'UNIT_BUILDER');u.build=1;PLAN.Units['1:13']='INCORPORATE']==],
    -- source line 1359, literal 2
    [==[spent build charges]==],
  },
  ["AbsorptionTests.test_city_failure_retains_snapshots_and_resume_does_not_recreate_units"]={
    -- source line 1366, literal 1
    [==[city failed]==],
    -- source line 1369, literal 2
    [==[test-absorption]==],
    -- source line 1371, literal 3
    [==[test-absorption]==],
    -- source line 1373, literal 4
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_later_turn_resume_is_rejected_without_reapplying_old_damage_or_movement"]={
    -- source line 1381, literal 1
    [==[original turn]==],
    -- source line 1382, literal 2
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_same_transaction_id_with_changed_decisions_is_rejected"]={
    -- source line 1387, literal 1
    [==[PLAN.Assets.hero='HIDE']==],
    -- source line 1388, literal 2
    [==[different plan]==],
  },
  ["AbsorptionTests.test_verification_catches_missing_or_corrupted_transfers_instead_of_only_counting"]={
    -- source line 1393, literal 1
    [==[ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:8').individual=43]==],
    -- source line 1394, literal 2
    [==[individual changed]==],
    -- source line 1395, literal 3
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_completed_transaction_is_not_replayed_after_later_turns"]={
    -- source line 1400, literal 1
    [==[ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3').damage=0]==],
    -- source line 1402, literal 2
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_registered_unit_and_city_properties_are_preserved_and_unsupported_values_rejected"]={
    -- source line 1406, literal 1
    [==[FOREIGN_UNIT]==],
    -- source line 1407, literal 2
    [==[FOREIGN_CITY]==],
    -- source line 1408, literal 3
    [==[Players[1].units[3]:SetProperty('FOREIGN_UNIT',{mission='HOLD',endTurn=20});CITIES[4]:SetProperty('FOREIGN_CITY',{castle='KEEP'})]==],
    -- source line 1410, literal 4
    [==[assert(ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3'):GetProperty('FOREIGN_UNIT').endTurn==20);assert(CITIES[4]:GetProperty('FOREIGN_CITY').castle=='KEEP')]==],
  },
  ["AbsorptionTests.test_adapter_preflight_apply_verify_and_reload_contract"]={
    -- source line 1413, literal 1
    [==[ADAPTER_APPLIED=0
          ExposedMembers.JAC_Absorption.RegisterAdapter('religion-interface',{
            Prepare=function(tx) return {Founder=tx.From,Followers=11} end,
            Apply=function(tx,data) Game:SetProperty('ADAPTER_FAITH',data);ADAPTER_APPLIED=ADAPTER_APPLIED+1 end,
            Verify=function(tx,data) assert(JAC_AbsorptionRules.Equal(Game:GetProperty('ADAPTER_FAITH'),data)) end})]==],
    -- source line 1421, literal 2
    [==[re-register]==],
    -- source line 1422, literal 3
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_bad_adapter_and_empty_string_fail_before_native_mutation"]={
    -- source line 1425, literal 1
    [==[ExposedMembers.JAC_Absorption.RegisterAdapter('bad',{Prepare=function() return {} end,Apply=function() end,Verify=function() end});Players[1].props.JAC_ABSORPTION_ASSETS_V1.hero.Name='']==],
    -- source line 1426, literal 2
    [==[empty strings]==],
  },
  ["AbsorptionTests.register_mission_rebind"]={
    -- source line 1431, literal 1
    [==[
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
        ]==],
  },
  ["AbsorptionTests.test_foreign_reference_rebind_keeps_target_deadline_and_fixed_roll_and_survives_reload"]={
    -- source line 1449, literal 1
    [==[
          local a=ExposedMembers.JAC_Absorption;local u=a.FindUnit(2,'test-absorption:unit:3')
          local mission=u:GetProperty('FOREIGN_MISSION')
          assert(mission.Actor.Owner==2 and mission.Actor.ID==u:GetID())
          assert(mission.Target.Owner==3 and mission.Target.ID==77 and mission.ReadyTurn==20 and mission.FixedRoll==17)
          assert(ORIGINAL_MISSION.Actor.Owner==1 and ORIGINAL_MISSION.Actor.ID==3)
          local tx=a.GetState().Transactions['test-absorption']
          assert(tx.Units[1].Properties.FOREIGN_MISSION.Actor.Owner==1)
          assert(tx.Units[1].TransferredProperties.FOREIGN_MISSION.Actor.Owner==2)
        ]==],
    -- source line 1460, literal 2
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_saved_rebound_expectation_is_used_on_resume_without_reloading_callback"]={
    -- source line 1467, literal 1
    [==[copy failed]==],
    -- source line 1472, literal 2
    [==[test-absorption]==],
    -- source line 1473, literal 3
    [==[test-absorption]==],
    -- source line 1474, literal 4
    [==[local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3');assert(u:GetProperty('FOREIGN_MISSION').Actor.ID==u:GetID())]==],
  },
  ["AbsorptionTests.test_corrupted_rebound_reference_is_detected_by_generic_verification"]={
    -- source line 1480, literal 1
    [==[ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3'):GetProperty('FOREIGN_MISSION').Actor.Owner=1]==],
    -- source line 1481, literal 2
    [==[unit properties changed]==],
    -- source line 1482, literal 3
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_city_property_rebind_uses_saved_expectations_on_reload"]={
    -- source line 1485, literal 1
    [==[
          CITIES[4]:SetProperty('FOREIGN_CITY_OWNER',{Custodian=1,Text='scripture'})
          ExposedMembers.JAC_Absorption.RegisterCityProperty('FOREIGN_CITY_OWNER',function(value,tx)
            if value then value.Custodian=tx.To end;return value
          end)
        ]==],
    -- source line 1494, literal 2
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_nonserializable_property_rebind_blocks_before_destroying_donors"]={
    -- source line 1497, literal 1
    [==[ExposedMembers.JAC_Absorption.RegisterUnitProperty('BAD_REBIND',function() return function() end end)]==],
    -- source line 1498, literal 2
    [==[serializable]==],
  },
  ["AbsorptionTests.test_native_hero_detected_by_engine_flag_with_a_nonstandard_type_is_rejected"]={
    -- source line 1503, literal 1
    [==[local u=UNIT(1,13,'UNIT_WARRIOR');u.IsHero=function() return true end;PLAN.Units['1:13']='INCORPORATE']==],
    -- source line 1503, literal 2
    [==[native Hero]==],
  },
  ["AbsorptionTests.test_missing_native_great_person_charge_api_blocks_before_any_mutation"]={
    -- source line 1507, literal 1
    [==[Players[1].units[8].GetGreatPerson=function() return {GetIndividual=function() return 42 end} end]==],
    -- source line 1508, literal 2
    [==[native great-person charges unavailable]==],
  },
  ["AbsorptionTests.test_spent_native_great_person_charges_do_not_reset_on_transfer"]={
    -- source line 1511, literal 1
    [==[Players[1].units[8].gpcharges=1]==],
    -- source line 1512, literal 2
    [==[spent great-person charges]==],
  },
  ["AbsorptionTests.test_missing_staged_replacement_blocks_instead_of_breaking_saved_references"]={
    -- source line 1520, literal 1
    [==[copy failed]==],
    -- source line 1522, literal 2
    [==[local u=ExposedMembers.JAC_Absorption.FindUnit(2,'test-absorption:unit:3');Players[2].units[u:GetID()]=nil]==],
    -- source line 1525, literal 3
    [==[staged replacement was lost]==],
    -- source line 1526, literal 4
    [==[test-absorption]==],
  },
  ["AbsorptionTests.live_scenario_fixture"]={
    -- source line 1530, literal 1
    [==[tools/civ6-autotest/scenarios/absorption-japan.json]==],
    -- source line 1530, literal 2
    [==[utf-8]==],
    -- source line 1531, literal 3
    [==[steps]==],
    -- source line 1531, literal 4
    [==[action]==],
    -- source line 1531, literal 5
    [==[ui_lua]==],
    -- source line 1531, literal 6
    [==[state_name]==],
    -- source line 1531, literal 7
    [==[GameCore_Tuner]==],
    -- source line 1531, literal 8
    [==[capture_as]==],
    -- source line 1531, literal 9
    [==[]==],
    -- source line 1531, literal 10
    [==[JAC_ABSORPTION_SYNC_]==],
    -- source line 1532, literal 11
    [==[
          ExposedMembers.JAC_HeroLifespanKey=UnitPropertyKeys.LifespanRemaining
          local majors={};for id=0,21 do
            local n=id;majors[#majors+1]={GetID=function() return n end}
          end
          PlayerManager.GetAliveMajors=function() return majors end
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
            ChangeFaithBalance=function(_,v) FAITH[n]=FAITH[n]+v end,
            GetFaithBalance=function() return FAITH[n] end} end end
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
        ]==],
  },
  ["AbsorptionTests.test_live_scenario_builds_missing_hero_origins_on_their_actual_city_plots"]={
    -- source line 1571, literal 1
    [==[CITIES[4].monument=false;CITIES[8].monument=false;BUILDING_CREATIONS=0]==],
    -- source line 1572, literal 2
    [==[code]==],
  },
  ["AbsorptionTests.test_live_scenario_checks_execute_in_lua_and_detect_asset_corruption_after_reload"]={
    -- source line 1579, literal 1
    [==[code]==],
    -- source line 1580, literal 2
    [==[code]==],
    -- source line 1581, literal 3
    [==[code]==],
    -- source line 1583, literal 4
    [==[code]==],
    -- source line 1585, literal 5
    [==[for _,u in Players[2]:GetUnits():Members() do
          if u:IsHero() then u:SetProperty(UnitPropertyKeys.LifespanRemaining,u:GetProperty(UnitPropertyKeys.LifespanRemaining)-1) end
        end]==],
    -- source line 1588, literal 6
    [==[code]==],
    -- source line 1590, literal 7
    [==[code]==],
    -- source line 1591, literal 8
    [==[Players[2].props.JAC_ABSORPTION_ASSETS_V1['live:hero'].Lifespan=18]==],
    -- source line 1592, literal 9
    [==[persistent asset changed]==],
    -- source line 1593, literal 10
    [==[code]==],
    -- source line 1594, literal 11
    [==[Players[2].props.JAC_ABSORPTION_ASSETS_V1['live:hero'].Lifespan=19]==],
    -- source line 1595, literal 12
    [==[live:person]==],
    -- source line 1595, literal 13
    [==[live:equipment]==],
    -- source line 1598, literal 14
    [==[local r=Players[2].props.JAC_ABSORPTION_ASSETS_V1;LIVE_RESTORE_RECORD=r[MISSING_LIVE_ASSET];r[MISSING_LIVE_ASSET]=nil]==],
    -- source line 1599, literal 15
    [==[persistent asset lost]==],
    -- source line 1600, literal 16
    [==[code]==],
    -- source line 1601, literal 17
    [==[Players[2].props.JAC_ABSORPTION_ASSETS_V1[MISSING_LIVE_ASSET]=LIVE_RESTORE_RECORD]==],
    -- source line 1602, literal 18
    [==[local r=Players[2].props.JAC_ABSORPTION_ASSETS_V1['live:hero'];local u=ExposedMembers.JAC_Absorption.FindUnit(2,r.Unit.Token);u:GetProperty('JAC_ABSORPTION_PERSON_V1').Owner=1]==],
    -- source line 1603, literal 19
    [==[transferred unit properties changed]==],
    -- source line 1604, literal 20
    [==[code]==],
  },
  ["AbsorptionTests.test_live_transfer_rejects_missing_cleric_before_ai_turn"]={
    -- source line 1608, literal 1
    [==[code]==],
    -- source line 1609, literal 2
    [==[code]==],
    -- source line 1610, literal 3
    [==[for id,u in pairs(Players[2].units) do
          if GameInfo.Units[u:GetType()].UnitType=='UNIT_MISSIONARY' then
            Players[2].units[id]=nil
          end
        end]==],
    -- source line 1615, literal 4
    [==[transferred unit missing/type/owner mismatch]==],
    -- source line 1616, literal 5
    [==[code]==],
  },
  ["AbsorptionTests.test_live_ai_turn_allows_cleric_consumption_but_checks_current_save_snapshot"]={
    -- source line 1620, literal 1
    [==[code]==],
    -- source line 1621, literal 2
    [==[code]==],
    -- source line 1622, literal 3
    [==[code]==],
    -- source line 1624, literal 4
    [==[for id,u in pairs(Players[2].units) do
          if u:IsHero() then u:SetProperty(UnitPropertyKeys.LifespanRemaining,
            u:GetProperty(UnitPropertyKeys.LifespanRemaining)-1) end
          if GameInfo.Units[u:GetType()].UnitType=='UNIT_MISSIONARY' then
            Players[2].units[id]=nil
          end
        end
        local current=UNIT(2,777,'UNIT_MISSIONARY');current.religion=5;current.spread=2
        ]==],
    -- source line 1633, literal 5
    [==[code]==],
    -- source line 1635, literal 6
    [==[code]==],
    -- source line 1636, literal 7
    [==[Players[2].units[777].spread=1]==],
    -- source line 1637, literal 8
    [==[code]==],
    -- source line 1638, literal 9
    [==[for id,u in pairs(Players[2].units) do
          if GameInfo.Units[u:GetType()].UnitType=='UNIT_GREAT_GENERAL' then
            Players[2].units[id]=nil
          end
        end]==],
    -- source line 1643, literal 10
    [==[transferred unit lost after AI turn]==],
    -- source line 1644, literal 11
    [==[code]==],
  },
  ["AbsorptionTests.test_live_recall_probes_execute_native_eligibility_checks_and_detect_wrong_owner"]={
    -- source line 1648, literal 1
    [==[code]==],
    -- source line 1649, literal 2
    [==[code]==],
    -- source line 1650, literal 3
    [==[
          CityCommandTypes={PURCHASE=1,PARAM_UNIT_TYPE=2,PARAM_YIELD_TYPE=3}
          CityCommandResults={FAILURE_REASONS=4};MilitaryFormationTypes={STANDARD_MILITARY_FORMATION=0}
          Locale={Lookup=function(key) return key=='LOC_CAN_TRAIN_FAILURE_SAME_ERA' and 'APPEARANCE_ERA' or key end}
          GameInfo.Yields={YIELD_FAITH={Index=6}}
          GameInfo.Eras=setmetatable({[0]={Index=0,ChronologyIndex=0},[1]={Index=1,ChronologyIndex=1}},
            {__call=function(t) local n=-1;return function() n=n+1;return t[n] end end})
          GLOBAL_ERA=0;AI_ACTIVE=false;LOCAL_PLAYER=1;FAITH[2]=1000000
          Game.GetEras=function() return {GetCurrentEra=function() return GLOBAL_ERA end} end
          Game.GetLocalPlayer=function() return LOCAL_PLAYER end
          Players[1].IsTurnActive=function() return not AI_ACTIVE end
          Players[2].IsTurnActive=function() return AI_ACTIVE end
          Players[1].IsHuman=function() return true end;Players[2].IsHuman=function() return false end
          Events={GameCoreEventPublishComplete={Add=function() end,Remove=function() end}}
          for _,c in pairs(CITIES) do c.GetGold=function() return {GetPurchaseCost=function(_,yield,hash,formation)
            assert(yield==6 and (hash==104 or hash==105) and formation==0);return 1000 end} end end
          RECALL_QUERIES=0
          CityManager.CanStartCommand=function(city,command,visible,parameters,results)
            assert(command==1 and parameters[3]==6 and results==not visible)
            RECALL_QUERIES=RECALL_QUERIES+1
            local class=parameters[2]==104 and 0 or parameters[2]==105 and 1 or error('not a native hero')
            local origin=HERO_ORIGINS[class]
            if HERO_CLAIMS[class]~=city.owner or not origin or origin.player~=city.owner or origin.id~=city.id then
              return false,{[4]={'WRONG_OWNER_OR_ORIGIN'}} end
            for _,u in pairs(Players[city.owner].units) do
              if u:GetHeroClassType()==class then return false,{[4]={'HERO_ALIVE'}} end
            end
            -- The official display query can stay true in the appearance era.
            if visible then return true end
            if not AI_ACTIVE or LOCAL_PLAYER>=0 and LOCAL_PLAYER~=city.owner then return false end
            if HERO_CLAIM_ERAS[class]>=GLOBAL_ERA then return false,{[4]={'APPEARANCE_ERA'}} end
            if FAITH[city.owner]<1000 then return false,{[4]={'INSUFFICIENT_FAITH'}} end
            return true,{}
          end
        ]==],
    -- source line 1685, literal 4
    [==[tools/civ6-autotest/scenarios/absorption-japan.json]==],
    -- source line 1685, literal 5
    [==[utf-8]==],
    -- source line 1686, literal 6
    [==[code]==],
    -- source line 1686, literal 7
    [==[steps]==],
    -- source line 1686, literal 8
    [==[JAC_ABSORPTION_AI_PROBE={Phase=]==],
    -- source line 1686, literal 9
    [==[code]==],
    -- source line 1686, literal 10
    [==[]==],
    -- source line 1687, literal 11
    [==[code]==],
    -- source line 1687, literal 12
    [==[steps]==],
    -- source line 1687, literal 13
    [==[AI active-turn probe never ran]==],
    -- source line 1687, literal 14
    [==[code]==],
    -- source line 1687, literal 15
    [==[]==],
    -- source line 1691, literal 16
    [==[JAC_ABSORPTION_AI_PROBE_HANDLER()]==],
    -- source line 1693, literal 17
    [==[AI active-turn probe never ran]==],
    -- source line 1695, literal 18
    [==[AI_ACTIVE=true;LOCAL_PLAYER=-1;JAC_ABSORPTION_AI_PROBE_HANDLER()]==],
    -- source line 1698, literal 19
    [==[HERO_CLAIMS[0]=1]==],
    -- source line 1700, literal 20
    [==[JAC_ABSORPTION_AI_PROBE_HANDLER()]==],
    -- source line 1701, literal 21
    [==[recall claim/owner changed]==],
    -- source line 1703, literal 22
    [==[HERO_CLAIMS[0]=2;AI_ACTIVE=false;LOCAL_PLAYER=1]==],
    -- source line 1704, literal 23
    [==[code]==],
    -- source line 1704, literal 24
    [==[steps]==],
    -- source line 1704, literal 25
    [==[retired for engine recall eligibility probe]==],
    -- source line 1704, literal 26
    [==[code]==],
    -- source line 1704, literal 27
    [==[]==],
    -- source line 1707, literal 28
    [==[AI_ACTIVE=true;LOCAL_PLAYER=-1;JAC_ABSORPTION_AI_PROBE_HANDLER()]==],
    -- source line 1713, literal 29
    [==[JAC_ABSORPTION_AI_PROBE.Results[1].Results[CityCommandResults.FAILURE_REASONS]={'SPAWN_BLOCKED'}]==],
    -- source line 1714, literal 30
    [==[did not report the native era restriction]==],
    -- source line 1718, literal 31
    [==[JAC_ABSORPTION_AI_PROBE_HANDLER()]==],
    -- source line 1720, literal 32
    [==[native recall ignored same-era restriction]==],
    -- source line 1722, literal 33
    [==[GLOBAL_ERA=0;LOCAL_PLAYER=1]==],
    -- source line 1724, literal 34
    [==[JAC_ABSORPTION_AI_PROBE_HANDLER()]==],
    -- source line 1725, literal 35
    [==[another human command context]==],
    -- source line 1728, literal 36
    [==[code]==],
    -- source line 1728, literal 37
    [==[steps]==],
    -- source line 1728, literal 38
    [==[actual AI recall intent saved]==],
    -- source line 1728, literal 39
    [==[code]==],
    -- source line 1728, literal 40
    [==[]==],
    -- source line 1729, literal 41
    [==[code]==],
    -- source line 1729, literal 42
    [==[steps]==],
    -- source line 1729, literal 43
    [==[real purchase handler prepared]==],
    -- source line 1729, literal 44
    [==[code]==],
    -- source line 1729, literal 45
    [==[]==],
    -- source line 1730, literal 46
    [==[code]==],
    -- source line 1730, literal 47
    [==[steps]==],
    -- source line 1730, literal 48
    [==[native AI recall payment changed]==],
    -- source line 1730, literal 49
    [==[code]==],
    -- source line 1730, literal 50
    [==[]==],
    -- source line 1731, literal 51
    [==[code]==],
    -- source line 1731, literal 52
    [==[steps]==],
    -- source line 1731, literal 53
    [==[person rebound to new native Unit]==],
    -- source line 1731, literal 54
    [==[code]==],
    -- source line 1731, literal 55
    [==[]==],
    -- source line 1732, literal 56
    [==[GLOBAL_ERA=1;AI_ACTIVE=false;LOCAL_PLAYER=1]==],
    -- source line 1734, literal 57
    [==[
          CityCommandTypes.PARAM_MILITARY_FORMATION_TYPE=5
          PURCHASE_REQUESTS=0;PURCHASE_COST=1000
          Events.FaithChanged={Add=function(f) FAITH_EVENT=f end,Remove=function() end}
          Events.CityCommandStarted={Add=function(f) COMMAND_EVENT=f end,Remove=function() end}
          Events.UnitAddedToMap={Add=function(f) UNIT_EVENT=f end,Remove=function() end}
          for _,c in pairs(CITIES) do c.GetGold=function() return {
            GetPurchaseCost=function() return PURCHASE_COST end} end end
          CityManager.RequestCommand=function(city,command,parameters)
            assert(CityManager.CanStartCommand(city,command,false,parameters,true))
            assert(parameters[5]==0)
            PURCHASE_REQUESTS=PURCHASE_REQUESTS+1
            FAITH[city.owner]=FAITH[city.owner]-PURCHASE_COST
            FAITH_EVENT(city.owner,0,FAITH[city.owner])
            local id=NEXT_ID
            HERO_MANAGER:CreateHero(city.owner,0,city.x,city.y)
            UNIT_EVENT(city.owner,id,city.x,city.y)
            PURCHASE_COST=1600
          end
        ]==],
    -- source line 1755, literal 58
    [==[JAC_ABSORPTION_AI_PURCHASE_HANDLER()]==],
    -- source line 1757, literal 59
    [==[AI purchase did not complete]==],
    -- source line 1759, literal 60
    [==[AI_ACTIVE=true;JAC_ABSORPTION_AI_PURCHASE_HANDLER()]==],
    -- source line 1761, literal 61
    [==[another human command context]==],
    -- source line 1763, literal 62
    [==[LOCAL_PLAYER=-1]==],
    -- source line 1765, literal 63
    [==[JAC_ABSORPTION_AI_PURCHASE_HANDLER()]==],
    -- source line 1768, literal 64
    [==[JAC_ABSORPTION_AI_PURCHASE_HANDLER();JAC_ABSORPTION_AI_PURCHASE_HANDLER()]==],
    -- source line 1776, literal 65
    [==[live:hero]==],
    -- source line 1777, literal 66
    [==[code]==],
    -- source line 1777, literal 67
    [==[steps]==],
    -- source line 1777, literal 68
    [==[capture_as]==],
    -- source line 1777, literal 69
    [==[ABSORPTION_AI_RECALL]==],
    -- source line 1779, literal 70
    [==[CAPTURE_STATE=ExposedMembers.JAC_Absorption.GetState()]==],
    -- source line 1780, literal 71
    [==[local s=ExposedMembers.JAC_Absorption.GetState();s.Transactions['japan-live-absorption'].HeroRecallUnits=nil;Game:SetProperty('JAC_ABSORPTION_STATE_V1',s)]==],
    -- source line 1781, literal 72
    [==[saved Hero recall mapping missing]==],
    -- source line 1783, literal 73
    [==[Game:SetProperty('JAC_ABSORPTION_STATE_V1',CAPTURE_STATE)]==],
    -- source line 1784, literal 74
    [==[local s=ExposedMembers.JAC_Absorption.GetState();s.Transactions['japan-live-absorption'].RecallRecords['live:hero'].Unit.Token='wrong';Game:SetProperty('JAC_ABSORPTION_STATE_V1',s)]==],
    -- source line 1785, literal 75
    [==[saved recalled person mapping changed]==],
    -- source line 1787, literal 76
    [==[Game:SetProperty('JAC_ABSORPTION_STATE_V1',CAPTURE_STATE)]==],
    -- source line 1789, literal 77
    [==[FAITH[2]=FAITH[2]-1]==],
    -- source line 1791, literal 78
    [==[FAITH[2]=FAITH[2]+1]==],
    -- source line 1792, literal 79
    [==[JAC_ABSORPTION_AI_PURCHASE.FaithAfter=JAC_ABSORPTION_AI_PURCHASE.FaithAfter-1]==],
    -- source line 1793, literal 80
    [==[native AI recall payment changed]==],
  },
  ["AbsorptionTests.test_human_receiver_scenario_executes_real_recall_and_rebinds_person_in_lua_vm"]={
    -- source line 1798, literal 1
    [==[tools/civ6-autotest/scenarios/absorption-hero-recall.json]==],
    -- source line 1799, literal 2
    [==[utf-8]==],
    -- source line 1799, literal 3
    [==[steps]==],
    -- source line 1800, literal 4
    [==[recall spawn blockers relocated]==],
    -- source line 1800, literal 5
    [==[code]==],
    -- source line 1800, literal 6
    [==[]==],
    -- source line 1800, literal 7
    [==[action]==],
    -- source line 1800, literal 8
    [==[autoplay]==],
    -- source line 1800, literal 9
    [==[capture_as]==],
    -- source line 1800, literal 10
    [==[]==],
    -- source line 1800, literal 11
    [==[JAC_ABSORPTION_SYNC_]==],
    -- source line 1801, literal 12
    [==[condition]==],
    -- source line 1801, literal 13
    [==[code]==],
    -- source line 1801, literal 14
    [==[]==],
    -- source line 1801, literal 15
    [==[JAC_ABSORPTION_FIXTURE_UI_WAIT]==],
    -- source line 1801, literal 16
    [==[GetUIPhase]==],
    -- source line 1801, literal 17
    [==[GetFaithBalance()>=1000000]==],
    -- source line 1801, literal 18
    [==[JAC_ABSORPTION_RECALL_COST_V1]==],
    -- source line 1802, literal 19
    [==[
          Game.GetLocalPlayer=function() return 0 end
          for id=4,21 do
            local n=id
            PlayerConfigurations[id].GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_OTHER_'..n end
          end
          PlayerConfigurations[3].GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_OTHER_3' end
          GameInfo.Yields={YIELD_FAITH={Index=6}}
          GameInfo.GreatWorks={[0]={GreatWorkObjectType='GREATWORKOBJECT_HERO'},
            [1]={GreatWorkObjectType='GREATWORKOBJECT_WRITING'}}
          Game.GetGreatWorkType=function(id)
            assert(id==1007 or id==1011 or id==1015 or id==1019 or id==1023)
            return id==1011 and 0 or 1
          end
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
        ]==],
    -- source line 1841, literal 20
    [==[code]==],
    -- source line 1842, literal 21
    [==[code]==],
    -- source line 1844, literal 22
    [==[code]==],
    -- source line 1846, literal 23
    [==[local e=Game:GetProperty('JAC_ABSORPTION_RECALL_EXPECT_V1')
          local a=ExposedMembers.JAC_Absorption;local r=a.GetRoster(e.To)[e.RecordID]
          a.FindUnit(e.To,r.Unit.Token).lifespan=18]==],
    -- source line 1849, literal 24
    [==[code]==],
    -- source line 1851, literal 25
    [==[global era boundary not processed]==],
    -- source line 1852, literal 26
    [==[code]==],
    -- source line 1854, literal 27
    [==[code]==],
    -- source line 1855, literal 28
    [==[tools/civ6-autotest/scenarios/absorption-hero-recall.json]==],
    -- source line 1856, literal 29
    [==[utf-8]==],
    -- source line 1856, literal 30
    [==[steps]==],
    -- source line 1857, literal 31
    [==[GetFaithBalance()>=1000000]==],
    -- source line 1857, literal 32
    [==[condition]==],
    -- source line 1857, literal 33
    [==[code]==],
    -- source line 1857, literal 34
    [==[]==],
    -- source line 1858, literal 35
    [==[capture_as]==],
    -- source line 1858, literal 36
    [==[JAC_ABSORPTION_SYNC_RECALL_COST]==],
    -- source line 1859, literal 37
    [==[JAC_ABSORPTION_RECALL_COST_V1]==],
    -- source line 1859, literal 38
    [==[condition]==],
    -- source line 1859, literal 39
    [==[code]==],
    -- source line 1859, literal 40
    [==[]==],
    -- source line 1860, literal 41
    [==[condition]==],
    -- source line 1860, literal 42
    [==[code]==],
    -- source line 1861, literal 43
    [==[code]==],
    -- source line 1862, literal 44
    [==[condition]==],
    -- source line 1862, literal 45
    [==[code]==],
    -- source line 1863, literal 46
    [==[Game:GetProperty('JAC_ABSORPTION_RECALL_COST_V1').CostBefore=999]==],
    -- source line 1864, literal 47
    [==[condition]==],
    -- source line 1864, literal 48
    [==[code]==],
    -- source line 1865, literal 49
    [==[recall cost snapshot changed]==],
    -- source line 1866, literal 50
    [==[code]==],
    -- source line 1867, literal 51
    [==[code]==],
    -- source line 1868, literal 52
    [==[GAME_SET_PROPERTY=Game.SetProperty;Game.SetProperty=nil]==],
    -- source line 1869, literal 53
    [==[code]==],
    -- source line 1870, literal 54
    [==[Game.SetProperty=GAME_SET_PROPERTY]==],
    -- source line 1871, literal 55
    [==[tools/civ6-autotest/scenarios/absorption-hero-recall.json]==],
    -- source line 1872, literal 56
    [==[utf-8]==],
    -- source line 1872, literal 57
    [==[steps]==],
    -- source line 1873, literal 58
    [==[capture_as]==],
    -- source line 1873, literal 59
    [==[JAC_ABSORPTION_SYNC_VERIFY_2]==],
    -- source line 1874, literal 60
    [==[local a=ExposedMembers.JAC_Absorption;local request=a.RequestVerification
          a.RequestVerification=function(id)
            assert(a.GetState().Recalls['japan-human-recall:1'].Status=='COMPLETE','verification requested before binding')
            return request(id)
          end]==],
    -- source line 1879, literal 61
    [==[code]==],
    -- source line 1880, literal 62
    [==[local e=Game:GetProperty('JAC_ABSORPTION_RECALL_EXPECT_V1')
          local a=ExposedMembers.JAC_Absorption;local r=a.GetRoster(e.To)[e.RecordID]
          assert(a.GetState().Recalls['japan-human-recall:1'].Status=='COMPLETE')
          assert(r.Unit.ID~=e.TransferID and #r.RecallHistory==1)]==],
    -- source line 1884, literal 63
    [==[ExposedMembers.JAC_Absorption.CompleteHeroRecall=function() error('binding after verification request') end]==],
    -- source line 1885, literal 64
    [==[code]==],
    -- source line 1887, literal 65
    [==[code]==],
    -- source line 1888, literal 66
    [==[local e=Game:GetProperty('JAC_ABSORPTION_RECALL_EXPECT_V1')
          assert(e.RecallID~=e.TransferID and e.FaithAfter==999000 and e.CostAfter==1000)
          assert(e.Before.HeroCountAvailable and e.Before.Hero==1 and e.AfterRecall.Hero==1)
          assert(#ExposedMembers.JAC_Absorption.GetRoster(e.To)[e.RecordID].RecallHistory==1)
        ]==],
    -- source line 1893, literal 67
    [==[return Game:GetProperty('JAC_ABSORPTION_RECALL_EXPECT_V1').To]==],
    -- source line 1896, literal 68
    [==[native recall faith payment changed]==],
    -- source line 1897, literal 69
    [==[code]==],
  },
  ["AbsorptionTests.test_partial_expected_properties_freeze_references_to_every_staged_unit"]={
    -- source line 1901, literal 1
    [==[
          ExposedMembers.JAC_Absorption.RegisterUnitProperty('PARTIAL_REF',function(value,tx,snap)
            if snap.ID==3 then return {Target=tx.UnitMap['1:12'].ID} end
            if snap.ID==12 then error('late rebind failed') end
            return value
          end)
        ]==],
    -- source line 1908, literal 2
    [==[late rebind failed]==],
    -- source line 1910, literal 3
    [==[local a=ExposedMembers.JAC_Absorption;assert(a.GetState().Transactions['test-absorption'].UnitMapFrozen);local u=a.FindUnit(2,'test-absorption:unit:12');Players[2].units[u:GetID()]=nil]==],
    -- source line 1912, literal 4
    [==[staged replacement was lost]==],
    -- source line 1913, literal 5
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_receiver_assets_added_or_updated_while_blocked_survive_resume"]={
    -- source line 1918, literal 1
    [==[city failed]==],
    -- source line 1920, literal 2
    [==[local r=Players[2].props.JAC_ABSORPTION_ASSETS_V1;r.existing.Level=9;r.new={ID='new',Kind='ITEM',Owner=2,Type='ARTIFACT'}]==],
    -- source line 1922, literal 3
    [==[test-absorption]==],
    -- source line 1926, literal 4
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_receiver_assets_added_by_city_transfer_hooks_are_not_overwritten"]={
    -- source line 1929, literal 1
    [==[
          local transfer=CityManager.TransferCity
          CityManager.TransferCity=function(city,owner)
            transfer(city,owner)
            local r=Players[owner].props.JAC_ABSORPTION_ASSETS_V1
            r['hook:'..city.x]={ID='hook:'..city.x,Kind='ITEM',Owner=owner,Type='COLLECTION'}
          end
        ]==],
    -- source line 1938, literal 2
    [==[hook:4]==],
    -- source line 1939, literal 3
    [==[hook:8]==],
  },
  ["AbsorptionTests.test_new_destination_id_collision_is_stopped_before_removing_donors"]={
    -- source line 1943, literal 1
    [==[copy failed]==],
    -- source line 1945, literal 2
    [==[Players[2].props.JAC_ABSORPTION_ASSETS_V1.hero={ID='hero',Kind='HERO',Owner=2,Lifespan=1}]==],
    -- source line 1947, literal 3
    [==[destination asset collision]==],
    -- source line 1948, literal 4
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_source_assets_created_while_blocked_are_preserved_instead_of_cleared"]={
    -- source line 1953, literal 1
    [==[copy failed]==],
    -- source line 1955, literal 2
    [==[Players[1].props.JAC_ABSORPTION_ASSETS_V1.new={ID='new',Kind='ITEM',Owner=1,Type='ARTIFACT'}]==],
    -- source line 1957, literal 3
    [==[source roster changed]==],
    -- source line 1958, literal 4
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_unsupported_charge_types_stop_before_source_assets_are_changed"]={
    -- source line 1963, literal 1
    [==[ReligiousHealCharges]==],
    -- source line 1963, literal 2
    [==[DisasterCharges]==],
    -- source line 1963, literal 3
    [==[ParkCharges]==],
    -- source line 1963, literal 4
    [==[ActionCharges]==],
    -- source line 1966, literal 5
    [==[GameInfo.Units.UNIT_MISSIONARY[UNSUPPORTED_COUNTER]=3]==],
    -- source line 1967, literal 6
    [==[unsupported unit charge counter: ]==],
    -- source line 1971, literal 7
    [==[GameInfo.Units.UNIT_MISSIONARY[UNSUPPORTED_COUNTER]=nil]==],
  },
  ["AbsorptionTests.test_runtime_special_charge_without_definition_is_also_rejected"]={
    -- source line 1974, literal 1
    [==[Players[1].units[10].GetDisasterCharges=function() return 1 end]==],
    -- source line 1975, literal 2
    [==[unsupported remaining unit charges: DisasterCharges]==],
  },
  ["AbsorptionTests.test_spent_park_charge_cannot_be_reset_by_absorption"]={
    -- source line 1981, literal 1
    [==[GameInfo.Units.UNIT_WARRIOR.ParkCharges=2
          Players[1].units[3].GetParkCharges=function() return 1 end
          local create=UNIT
          UNIT=function(...)
            local u=create(...)
            if u:GetType()==0 then u.GetParkCharges=function() return 2 end end
            return u
          end]==],
    -- source line 1989, literal 2
    [==[unsupported unit charge counter: ParkCharges]==],
  },
  ["AbsorptionTests.test_runtime_park_and_nonhero_action_charges_are_rejected_before_changes"]={
    -- source line 1996, literal 1
    [==[ParkCharges]==],
    -- source line 1996, literal 2
    [==[ActionCharges]==],
    -- source line 2000, literal 3
    [==[Players[1].units[3]['Get'..UNSUPPORTED_COUNTER]=function() return 1 end]==],
    -- source line 2001, literal 4
    [==[unsupported remaining unit charges: ]==],
  },
  ["AbsorptionTests.test_nonhero_action_charges_declared_in_mode_table_are_rejected"]={
    -- source line 2007, literal 1
    [==[GameInfo.Units_MODE={UNIT_WARRIOR={ActionCharges=3}}]==],
    -- source line 2008, literal 2
    [==[unsupported unit charge counter: ActionCharges]==],
  },
  ["AbsorptionTests.test_rock_band_state_is_rejected_by_type_class_or_tag_before_changes"]={
    -- source line 2014, literal 1
    [==[GameInfo.Units.UNIT_WARRIOR.UnitType='UNIT_ROCK_BAND']==],
    -- source line 2015, literal 2
    [==[GameInfo.Units.UNIT_WARRIOR.PromotionClass='PROMOTION_CLASS_ROCK_BAND']==],
    -- source line 2016, literal 3
    [==[GameInfo.TypeTags=function() local sent=false;return function() if not sent then sent=true;]==],
    -- source line 2017, literal 4
    [==[return {Type='UNIT_WARRIOR',Tag='CLASS_ROCK_BAND'} end end end]==],
    -- source line 2022, literal 5
    [==[Players[1].units[3].GetRockBand=function() return {
                  GetAlbumSales=function() return 150 end,GetRockBandLevel=function() return 3 end} end]==],
    -- source line 2024, literal 6
    [==[unsupported unit state: RockBand]==],
  },
  ["AbsorptionTests.test_candidate_special_charge_blocks_and_reuses_the_candidate_on_resume"]={
    -- source line 2030, literal 1
    [==[DisasterCharges]==],
    -- source line 2030, literal 2
    [==[ParkCharges]==],
    -- source line 2030, literal 3
    [==[ActionCharges]==],
  },
  ["AbsorptionTests.assert_candidate_counter_blocks_and_resumes"]={
    -- source line 2037, literal 1
    [==[local create=UNIT
          UNIT=function(...)
            local u=create(...)
            if u:GetOwner()==2 and not u:IsHero() then
              u['Get'..CANDIDATE_COUNTER]=function() return CANDIDATE_CHARGES end
            end
            return u
          end
          CANDIDATE_CHARGES=1]==],
    -- source line 2046, literal 2
    [==[unsupported remaining unit charges: ]==],
    -- source line 2050, literal 3
    [==[test-absorption:unit:3]==],
    -- source line 2053, literal 4
    [==[unsupported remaining unit charges: ]==],
    -- source line 2054, literal 5
    [==[test-absorption]==],
    -- source line 2057, literal 6
    [==[test-absorption]==],
    -- source line 2058, literal 7
    [==[test-absorption:unit:3]==],
    -- source line 2061, literal 8
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_city_district_loss_is_detected_with_buildings_and_territory_intact"]={
    -- source line 2064, literal 1
    [==[CITIES[4].districts[1]={X=5,Y=0}]==],
    -- source line 2066, literal 2
    [==[test-absorption]==],
    -- source line 2067, literal 3
    [==[CITIES[4].districts[1]=nil]==],
    -- source line 2068, literal 4
    [==[transferred city district lost/moved: DISTRICT_HOLY_SITE]==],
    -- source line 2069, literal 5
    [==[test-absorption]==],
    -- source line 2070, literal 6
    [==[transferred city district lost/moved: DISTRICT_HOLY_SITE]==],
    -- source line 2071, literal 7
    [==[test-absorption]==],
    -- source line 2072, literal 8
    [==[CITIES[4].districts[1]={X=6,Y=0}]==],
    -- source line 2073, literal 9
    [==[transferred city district lost/moved: DISTRICT_HOLY_SITE]==],
    -- source line 2074, literal 10
    [==[test-absorption]==],
    -- source line 2075, literal 11
    [==[CITIES[4].districts[1]={X=5,Y=0}]==],
    -- source line 2077, literal 12
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_legacy_city_assets_keep_recorded_checks_without_inventing_district_coverage"]={
    -- source line 2081, literal 1
    [==[local tx=Game:GetProperty('JAC_ABSORPTION_STATE_V1').Transactions['test-absorption']
          assert(tx.Cities[1].X==4)
          tx.Cities[1].AssetCoverageVersion=nil;tx.Cities[1].Assets.Districts=nil]==],
    -- source line 2085, literal 2
    [==[test-absorption]==],
    -- source line 2086, literal 3
    [==[CITIES[4].monument=false]==],
    -- source line 2087, literal 4
    [==[transferred native Great Work index absent from absorber: native-work:1011]==],
    -- source line 2088, literal 5
    [==[test-absorption]==],
    -- source line 2089, literal 6
    [==[CITIES[4].monument=true;CITIES[4].districts[1]={X=5,Y=0}
          local tx=Game:GetProperty('JAC_ABSORPTION_STATE_V1').Transactions['test-absorption']
          tx.Cities[1].Assets.Districts={DISTRICT_HOLY_SITE={X=5,Y=0}}]==],
    -- source line 2092, literal 7
    [==[test-absorption]==],
    -- source line 2093, literal 8
    [==[CITIES[4].districts[1]=nil]==],
    -- source line 2094, literal 9
    [==[transferred city district lost/moved: DISTRICT_HOLY_SITE:5:0]==],
    -- source line 2095, literal 10
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_new_city_asset_coverage_rejects_missing_district_snapshot"]={
    -- source line 2099, literal 1
    [==[local tx=Game:GetProperty('JAC_ABSORPTION_STATE_V1').Transactions['test-absorption']
          assert(tx.Cities[1].AssetCoverageVersion==2);tx.Cities[1].Assets.Districts=nil]==],
    -- source line 2101, literal 2
    [==[district snapshot missing from current asset coverage]==],
    -- source line 2102, literal 3
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_multiple_same_type_districts_and_ui_only_enumeration_are_preserved"]={
    -- source line 2105, literal 1
    [==[CITIES[4].districts[1]={X=5,Y=0};CITIES[4].districts[2]={Type=1,X=6,Y=0}]==],
    -- source line 2107, literal 2
    [==[
          UI_DISTRICT_HANDLES={}
          for x,c in pairs(CITIES) do UI_DISTRICT_HANDLES[x]=c.GetDistricts end
          for _,p in pairs(UI_PLAYERS) do
            local get=p.GetCities
            p.GetCities=function()
              local cities=get()
              return {FindID=function(_,id)
                local c=cities:FindID(id)
                return c and setmetatable({GetDistricts=UI_DISTRICT_HANDLES[c:GetX()]},{__index=c})
              end}
            end
          end
          for _,c in pairs(CITIES) do c.GetDistricts=function() return {} end end
        ]==],
    -- source line 2123, literal 3
    [==[WAITING_UI]==],
    -- source line 2126, literal 4
    [==[test-absorption]==],
    -- source line 2127, literal 5
    [==[CITIES[4].districts[2]=nil]==],
    -- source line 2128, literal 6
    [==[transferred city district lost/moved: DISTRICT_HOLY_SITE:6:0]==],
    -- source line 2129, literal 7
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_district_only_ui_cache_waits_after_transfer_and_across_resume"]={
    -- source line 2132, literal 1
    [==[CORE_PLAYERS=Players;UI_PLAYERS={};PUBLISHED_TRANSFERS=0
          UI_DISTRICT_HANDLES={}
          for x,c in pairs(CITIES) do UI_DISTRICT_HANDLES[x]=c.GetDistricts end
          for owner,p in pairs(Players) do
            local source,playerID=p,owner
            UI_PLAYERS[owner]={GetCities=function() return {FindID=function(_,id)
              if playerID==2 and PUBLISHED_TRANSFERS<TRANSFERS then return nil end
              local c=source:GetCities():FindID(id)
              return c and setmetatable({GetDistricts=UI_DISTRICT_HANDLES[c:GetX()]},{__index=c})
            end} end,GetUnits=function() return source:GetUnits() end}
          end
          Players=UI_PLAYERS
          Events={LoadScreenClose={Add=function() end},
            GameCoreEventPublishComplete={Add=function(f) PUBLISH_CITY_UI=f end}}
        ]==],
    -- source line 2147, literal 2
    [==[UI/JAC_Absorption_HeroKeys.lua]==],
    -- source line 2147, literal 3
    [==[utf-8]==],
    -- source line 2148, literal 4
    [==[Players=CORE_PLAYERS
          for _,c in pairs(CITIES) do c.GetDistricts=function() return {} end end
          UI_PUBLISH=function() PUBLISHED_TRANSFERS=TRANSFERS;PUBLISH_CITY_UI() end]==],
    -- source line 2152, literal 5
    [==[WAITING_UI]==],
    -- source line 2154, literal 6
    [==[UI_PUBLISH()]==],
    -- source line 2155, literal 7
    [==[test-absorption]==],
    -- source line 2156, literal 8
    [==[WAITING_UI]==],
    -- source line 2161, literal 9
    [==[test-absorption]==],
    -- source line 2161, literal 10
    [==[WAITING_UI]==],
    -- source line 2163, literal 11
    [==[UI_PUBLISH()]==],
    -- source line 2164, literal 12
    [==[test-absorption]==],
    -- source line 2164, literal 13
    [==[COMPLETE]==],
    -- source line 2165, literal 14
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_palace_work_loss_is_detected_even_when_the_palace_building_is_exempt"]={
    -- source line 2168, literal 1
    [==[
          CITIES[4].palace=true;CITIES[4].palaceWork=9001
          local transfer=CityManager.TransferCity
          CityManager.TransferCity=function(city,owner) transfer(city,owner);city.palace=false end
        ]==],
    -- source line 2173, literal 2
    [==[transferred native Great Work index absent from absorber: native-work:9001]==],
    -- source line 2175, literal 3
    [==[test-absorption]==],
    -- source line 2176, literal 4
    [==[test-absorption]==],
    -- source line 2176, literal 5
    [==[BLOCKED]==],
  },
  ["AbsorptionTests.test_palace_work_moved_to_receiver_capital_is_preserved_and_verified"]={
    -- source line 2179, literal 1
    [==[
          CITIES[4].palace=true;CITIES[4].palaceWork=9001
          local transfer=CityManager.TransferCity
          CityManager.TransferCity=function(city,owner)
            local work=city.palaceWork;transfer(city,owner);city.palace=false
            if work then CITIES[12].palace=true;CITIES[12].palaceWork=work end
          end
        ]==],
    -- source line 2189, literal 2
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_previous_governance_is_saved_before_engine_clears_city_properties"]={
    -- source line 2192, literal 1
    [==[OLD_GOV={ID='prior',From=3,To=1,Turn=2,Previous={ID='older',From=0,To=3,Turn=1}};CITIES[4]:SetProperty('JAC_ABSORPTION_GOVERNANCE_V1',OLD_GOV)]==],
    -- source line 2194, literal 2
    [==[assert(JAC_AbsorptionRules.Equal(CITIES[4]:GetProperty('JAC_ABSORPTION_GOVERNANCE_V1').Previous,OLD_GOV))]==],
    -- source line 2196, literal 3
    [==[test-absorption]==],
  },
  ["AbsorptionTests.test_exact_verification_detects_city_rename_but_later_turns_allow_it"]={
    -- source line 2201, literal 1
    [==[CITIES[4].GetName=function() return 'RENAMED_CITY' end]==],
    -- source line 2202, literal 2
    [==[city name changed]==],
    -- source line 2203, literal 3
    [==[test-absorption]==],
    -- source line 2204, literal 4
    [==[test-absorption]==],
  },
  ["AbsorptionPackageTests.test_ai_purchase_receipt_rejects_interleaved_faith_or_unit_events"]={
    -- source line 2209, literal 1
    [==[tools/civ6-autotest/scenarios/absorption-japan.json]==],
    -- source line 2209, literal 2
    [==[utf-8]==],
    -- source line 2209, literal 3
    [==[steps]==],
    -- source line 2210, literal 4
    [==[code]==],
    -- source line 2210, literal 5
    [==[real purchase handler prepared]==],
    -- source line 2210, literal 6
    [==[code]==],
    -- source line 2210, literal 7
    [==[]==],
    -- source line 2211, literal 8
    [==[code]==],
    -- source line 2211, literal 9
    [==[native AI recall payment changed]==],
    -- source line 2211, literal 10
    [==[code]==],
    -- source line 2211, literal 11
    [==[]==],
    -- source line 2212, literal 12
    [==[none]==],
    -- source line 2212, literal 13
    [==[faith]==],
    -- source line 2212, literal 14
    [==[unit]==],
    -- source line 2212, literal 15
    [==[order]==],
    -- source line 2215, literal 16
    [==[
                  BALANCE=5000;COST=1000;UNITS={};REQUESTS=0
                  local api={Canonical=function() return 'receipt verified' end,Rules={Copy=function(v) return v end}}
                  ExposedMembers={JAC_Absorption=api}
                  GameInfo={HeroClasses={[0]={UnitType='HERO'}},Units={HERO={Hash=104}},Yields={YIELD_FAITH={Index=6}}}
                  CityCommandTypes={PURCHASE=1,PARAM_UNIT_TYPE=2,PARAM_YIELD_TYPE=3,PARAM_MILITARY_FORMATION_TYPE=4}
                  MilitaryFormationTypes={STANDARD_MILITARY_FORMATION=0}
                  local city={GetX=function() return 4 end,GetY=function() return 0 end,
                    GetGold=function() return {GetPurchaseCost=function() return COST end} end}
                  Players={[1]={IsTurnActive=function() return false end},[2]={IsTurnActive=function() return true end,
                    IsHuman=function() return false end,GetCities=function() return {FindID=function() return city end} end,
                    GetUnits=function() return {Members=function() return ipairs(UNITS) end} end,
                    GetReligion=function() return {GetFaithBalance=function() return BALANCE end} end}}
                  local manager={GetHeroClaimPlayer=function() return 2 end,
                    GetHeroOriginCityID=function() return {player=2,id=7} end}
                  Game={GetProperty=function() return {Owner=2,Class=0,CityID=7,EraBefore=0} end,
                    GetHeroesManager=function() return manager end,GetLocalPlayer=function() return -1 end,
                    GetCurrentGameTurn=function() return 10 end,GetEras=function() return {GetCurrentEra=function() return 1 end} end}
                  Events={GameCoreEventPublishComplete={Add=function() end,Remove=function() end},
                    FaithChanged={Add=function(f) FAITH_EVENT=f end,Remove=function() end},
                    CityCommandStarted={Add=function(f) COMMAND_EVENT=f end,Remove=function() end},
                    UnitAddedToMap={Add=function(f) UNIT_EVENT=f end,Remove=function() end}}
                  CityManager={CanStartCommand=function() return true,{} end,RequestCommand=function()
                    REQUESTS=REQUESTS+1;BALANCE=BALANCE-1000;FAITH_EVENT(2,0,BALANCE) end}
                  function Spawn(class,id)
                    UNITS[#UNITS+1]={GetHeroClassType=function() return class end,GetID=function() return id end}
                    UNIT_EVENT(2,id,4,0);if class==0 then COST=1600 end
                  end
                ]==],
    -- source line 2245, literal 17
    [==[JAC_ABSORPTION_AI_PURCHASE_HANDLER();JAC_ABSORPTION_AI_PURCHASE_HANDLER()]==],
    -- source line 2248, literal 18
    [==[faith]==],
    -- source line 2249, literal 19
    [==[BALANCE=BALANCE-100;FAITH_EVENT(2,0,BALANCE);BALANCE=BALANCE+100;FAITH_EVENT(2,0,BALANCE)]==],
    -- source line 2250, literal 20
    [==[unit]==],
    -- source line 2251, literal 21
    [==[Spawn(1,20)]==],
    -- source line 2252, literal 22
    [==[Spawn(0,21);JAC_ABSORPTION_AI_PURCHASE_HANDLER()]==],
    -- source line 2255, literal 23
    [==[order]==],
    -- source line 2256, literal 24
    [==[local t=JAC_ABSORPTION_AI_PURCHASE.Trace;t[2],t[3]=t[3],t[2]]==],
    -- source line 2257, literal 25
    [==[none]==],
    -- source line 2258, literal 26
    [==[receipt verified]==],
    -- source line 2260, literal 27
    [==[payment interval includes unrelated or unordered events]==],
  },
  ["AbsorptionPackageTests.test_cleric_fixture_can_be_purchased_without_enabling_other_disabled_units"]={
    -- source line 2264, literal 1
    [==[:memory:]==],
    -- source line 2265, literal 2
    [==[CREATE TABLE Units (UnitType TEXT PRIMARY KEY, CanTrain INTEGER)]==],
    -- source line 2266, literal 3
    [==[INSERT INTO Units VALUES (?,?)]==],
    -- source line 2267, literal 4
    [==[UNIT_MISSIONARY]==],
    -- source line 2267, literal 5
    [==[UNIT_APOSTLE]==],
    -- source line 2267, literal 6
    [==[UNIT_HERO_HERCULES]==],
    -- source line 2267, literal 7
    [==[UNIT_WARRIOR]==],
    -- source line 2268, literal 8
    [==[Data/JAC_Absorption.sql]==],
    -- source line 2268, literal 9
    [==[utf-8]==],
    -- source line 2269, literal 10
    [==[SELECT UnitType,CanTrain FROM Units]==],
    -- source line 2270, literal 11
    [==[UNIT_MISSIONARY]==],
    -- source line 2270, literal 12
    [==[UNIT_APOSTLE]==],
    -- source line 2270, literal 13
    [==[UNIT_HERO_HERCULES]==],
    -- source line 2270, literal 14
    [==[UNIT_WARRIOR]==],
    -- source line 2271, literal 15
    [==[JAC_PoC_Absorption.modinfo]==],
    -- source line 2272, literal 16
    [==[./InGameActions/UpdateDatabase]==],
    -- source line 2273, literal 17
    [==[criteria]==],
    -- source line 2273, literal 18
    [==[JAC_Absorption_Banseiki]==],
    -- source line 2274, literal 19
    [==[./Properties/LoadOrder]==],
  },
  ["AbsorptionPackageTests.test_scenario_lua_compiles_and_both_snapshots_have_round_trip_comparisons"]={
    -- source line 2277, literal 1
    [==[tools/civ6-autotest/scenarios/absorption-japan.json]==],
    -- source line 2277, literal 2
    [==[utf-8]==],
    -- source line 2278, literal 3
    [==[steps]==],
    -- source line 2280, literal 4
    [==[function(code) local f,e=load(code);return e end]==],
    -- source line 2282, literal 5
    [==[action]==],
    -- source line 2282, literal 6
    [==[ui_lua]==],
    -- source line 2283, literal 7
    [==[code]==],
    -- source line 2284, literal 8
    [==[action]==],
    -- source line 2284, literal 9
    [==[wait_for]==],
    -- source line 2285, literal 10
    [==[condition]==],
    -- source line 2285, literal 11
    [==[code]==],
    -- source line 2286, literal 12
    [==[capture_as]==],
    -- source line 2286, literal 13
    [==[capture_as]==],
    -- source line 2286, literal 14
    [==[capture_as]==],
    -- source line 2286, literal 15
    [==[JAC_ABSORPTION_SYNC_]==],
    -- source line 2287, literal 16
    [==[ABSORPTION_IMMEDIATE]==],
    -- source line 2287, literal 17
    [==[ABSORPTION_AFTER_AI_TURN]==],
    -- source line 2287, literal 18
    [==[ABSORPTION_AI_RECALL]==],
    -- source line 2288, literal 19
    [==[equals_capture]==],
    -- source line 2288, literal 20
    [==[equals_capture]==],
    -- source line 2289, literal 21
    [==[action]==],
    -- source line 2289, literal 22
    [==[save]==],
    -- source line 2290, literal 23
    [==[action]==],
    -- source line 2290, literal 24
    [==[load]==],
    -- source line 2291, literal 25
    [==[action]==],
    -- source line 2291, literal 26
    [==[configure]==],
    -- source line 2292, literal 27
    [==[action]==],
    -- source line 2292, literal 28
    [==[enable_mods]==],
    -- source line 2293, literal 29
    [==[8424840C-92EF-4426-A9B4-B4E0CB818049]==],
    -- source line 2293, literal 30
    [==[ids]==],
    -- source line 2294, literal 31
    [==[action]==],
    -- source line 2294, literal 32
    [==[ui_lua]==],
    -- source line 2294, literal 33
    [==[state_name]==],
    -- source line 2294, literal 34
    [==[FrontEnd]==],
    -- source line 2296, literal 35
    [==[GameConfiguration.SetValue('GAMEMODE_HEROES',true)]==],
    -- source line 2296, literal 36
    [==[code]==],
    -- source line 2297, literal 37
    [==[map_file]==],
    -- source line 2297, literal 38
    [==[{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map]==],
    -- source line 2298, literal 39
    [==[civ]==],
    -- source line 2298, literal 40
    [==[players]==],
    -- source line 2298, literal 41
    [==[human]==],
    -- source line 2299, literal 42
    [==[CIVILIZATION_JAC_ASHINA]==],
    -- source line 2299, literal 43
    [==[CIVILIZATION_JAC_DATE]==],
    -- source line 2300, literal 44
    [==[action]==],
    -- source line 2300, literal 45
    [==[end_turn]==],
    -- source line 2301, literal 46
    [==[turns]==],
  },
  ["AbsorptionPackageTests.test_control_missionary_publication_checks_charges_then_core_religion"]={
    -- source line 2304, literal 1
    [==[tools/civ6-autotest/scenarios/absorption-japan.json]==],
    -- source line 2305, literal 2
    [==[utf-8]==],
    -- source line 2306, literal 3
    [==[condition]==],
    -- source line 2306, literal 4
    [==[code]==],
    -- source line 2306, literal 5
    [==[steps]==],
    -- source line 2307, literal 6
    [==[action]==],
    -- source line 2307, literal 7
    [==[wait_for]==],
    -- source line 2307, literal 8
    [==[JAC_ABSORPTION_CONTROL_PRIEST_V1]==],
    -- source line 2307, literal 9
    [==[condition]==],
    -- source line 2307, literal 10
    [==[code]==],
    -- source line 2308, literal 11
    [==[code]==],
    -- source line 2308, literal 12
    [==[steps]==],
    -- source line 2309, literal 13
    [==[action]==],
    -- source line 2309, literal 14
    [==[ui_lua]==],
    -- source line 2309, literal 15
    [==[missionary core religion mismatch]==],
    -- source line 2309, literal 16
    [==[code]==],
    -- source line 2309, literal 17
    [==[]==],
    -- source line 2311, literal 18
    [==[
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
        ]==],
    -- source line 2327, literal 19
    [==[core religion and published charges match]==],
    -- source line 2328, literal 20
    [==[UI_VISIBLE=false]==],
    -- source line 2330, literal 21
    [==[missionary absent in core]==],
    -- source line 2332, literal 22
    [==[UI_VISIBLE=true;UI_CHARGES=2]==],
    -- source line 2334, literal 23
    [==[published charges mismatch]==],
    -- source line 2336, literal 24
    [==[UI_CHARGES=3;CORE_RELIGION=4]==],
    -- source line 2337, literal 25
    [==[core religion mismatch]==],
  },
  ["AbsorptionPackageTests.test_recall_publication_uses_ui_class_getter_and_waits_for_native_claim"]={
    -- source line 2341, literal 1
    [==[tools/civ6-autotest/scenarios/absorption-hero-recall.json]==],
    -- source line 2342, literal 2
    [==[utf-8]==],
    -- source line 2343, literal 3
    [==[condition]==],
    -- source line 2343, literal 4
    [==[code]==],
    -- source line 2343, literal 5
    [==[steps]==],
    -- source line 2344, literal 6
    [==[action]==],
    -- source line 2344, literal 7
    [==[wait_for]==],
    -- source line 2344, literal 8
    [==[e.TransferID]==],
    -- source line 2344, literal 9
    [==[condition]==],
    -- source line 2344, literal 10
    [==[code]==],
    -- source line 2346, literal 11
    [==[
          CLASS=0;ID=10;CLAIM=2;ORIGIN={player=2,id=7};VISIBLE=true
          Game={GetProperty=function() return {To=2,Class=0,TransferID=10} end,
            GetHeroesManager=function() return {
              GetHeroClaimPlayer=function() return CLAIM end,
              GetHeroOriginCityID=function() return ORIGIN end} end}
          local u={GetHeroClassType=function() return CLASS end,GetID=function() return ID end}
          -- Installed UI Units have GetHeroClassType, but no IsHero method.
          Players={[2]={GetUnits=function() return {Members=function()
            return ipairs(VISIBLE and {u} or {}) end} end}}
        ]==],
    -- source line 2358, literal 12
    [==[ID=11]==],
    -- source line 2360, literal 13
    [==[CLASS=1]==],
    -- source line 2360, literal 14
    [==[CLAIM=1]==],
    -- source line 2360, literal 15
    [==[ORIGIN=nil]==],
    -- source line 2360, literal 16
    [==[ORIGIN={player=1,id=7}]==],
    -- source line 2360, literal 17
    [==[VISIBLE=false]==],
    -- source line 2361, literal 18
    [==[CLASS=0;CLAIM=2;ORIGIN={player=2,id=7};VISIBLE=true]==],
  },
  ["AbsorptionPackageTests.test_recall_spawn_preparation_moves_only_blockers_and_preserves_them"]={
    -- source line 2366, literal 1
    [==[tools/civ6-autotest/scenarios/absorption-hero-recall.json]==],
    -- source line 2367, literal 2
    [==[utf-8]==],
    -- source line 2368, literal 3
    [==[code]==],
    -- source line 2368, literal 4
    [==[steps]==],
    -- source line 2369, literal 5
    [==[recall spawn blockers relocated]==],
    -- source line 2369, literal 6
    [==[code]==],
    -- source line 2369, literal 7
    [==[]==],
    -- source line 2370, literal 8
    [==[tools/civ6-autotest/scenarios/absorption-japan.json]==],
    -- source line 2371, literal 9
    [==[utf-8]==],
    -- source line 2372, literal 10
    [==[code]==],
    -- source line 2372, literal 11
    [==[steps]==],
    -- source line 2373, literal 12
    [==[recall spawn blockers relocated]==],
    -- source line 2373, literal 13
    [==[code]==],
    -- source line 2373, literal 14
    [==[]==],
    -- source line 2375, literal 15
    [==[
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
          Game={GetProperty=function() return {To=2,Owner=2,Class=0} end}
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
        ]==],
    -- source line 2406, literal 16
    [==[without deletion: 2]==],
    -- source line 2407, literal 17
    [==[assert(UNITS[1].x==14 and UNITS[1].y==16)
          assert(UNITS[3].x==14 and UNITS[3].y==17)
          assert(UNITS[2].x==20 and UNITS[2].y==20)
          for _,u in ipairs(UNITS) do assert(u.damage==5 and u.moves==2) end
          FREE=false;UNITS[1].x=20;UNITS[1].y=20]==],
    -- source line 2412, literal 18
    [==[no empty owned plot]==],
    -- source line 2414, literal 19
    [==[assert(#UNITS==3 and UNITS[1].x==20 and UNITS[1].y==20)]==],
    -- source line 2415, literal 20
    [==[FREE=true;for _,u in ipairs(UNITS) do u.x=20;u.y=20 end]==],
    -- source line 2416, literal 21
    [==[without deletion: 2]==],
    -- source line 2417, literal 22
    [==[assert(UNITS[1].x==14 and UNITS[1].y==16)
          assert(UNITS[3].x==14 and UNITS[3].y==17)
          assert(UNITS[2].x==20 and UNITS[2].y==20)
          for _,u in ipairs(UNITS) do assert(u.damage==5 and u.moves==2) end]==],
  },
  ["AbsorptionPackageTests.test_human_recall_scenario_uses_custom_native_classes_and_real_purchase"]={
    -- source line 2423, literal 1
    [==[tools/civ6-autotest/scenarios/absorption-hero-recall.json]==],
    -- source line 2424, literal 2
    [==[utf-8]==],
    -- source line 2425, literal 3
    [==[steps]==],
    -- source line 2426, literal 4
    [==[function(code) local f,e=load(code);return e end]==],
    -- source line 2428, literal 5
    [==[action]==],
    -- source line 2428, literal 6
    [==[ui_lua]==],
    -- source line 2429, literal 7
    [==[code]==],
    -- source line 2430, literal 8
    [==[action]==],
    -- source line 2430, literal 9
    [==[wait_for]==],
    -- source line 2431, literal 10
    [==[condition]==],
    -- source line 2431, literal 11
    [==[code]==],
    -- source line 2432, literal 12
    [==[action]==],
    -- source line 2432, literal 13
    [==[configure]==],
    -- source line 2433, literal 14
    [==[players]==],
    -- source line 2434, literal 15
    [==[civ]==],
    -- source line 2434, literal 16
    [==[players]==],
    -- source line 2434, literal 17
    [==[human]==],
    -- source line 2435, literal 18
    [==[CIVILIZATION_JAC_ODA]==],
    -- source line 2436, literal 19
    [==[action]==],
    -- source line 2436, literal 20
    [==[end_turn]==],
    -- source line 2437, literal 21
    [==[action]==],
    -- source line 2437, literal 22
    [==[save]==],
    -- source line 2438, literal 23
    [==[action]==],
    -- source line 2438, literal 24
    [==[load]==],
    -- source line 2439, literal 25
    [==[
]==],
    -- source line 2439, literal 26
    [==[code]==],
    -- source line 2439, literal 27
    [==[action]==],
    -- source line 2439, literal 28
    [==[ui_lua]==],
    -- source line 2440, literal 29
    [==[CityManager.RequestCommand(city,CityCommandTypes.PURCHASE,parameters)]==],
    -- source line 2441, literal 30
    [==[A.BeginHeroRecall]==],
    -- source line 2442, literal 31
    [==[A.CompleteHeroRecall]==],
    -- source line 2443, literal 32
    [==[non-Banseiki Hero class enabled]==],
  },
  ["AbsorptionPackageTests.test_modinfo_references_exist_and_have_no_open_pr_dependencies"]={
    -- source line 2446, literal 1
    [==[JAC_PoC_Absorption.modinfo]==],
    -- source line 2447, literal 2
    [==[./Files/File]==],
    -- source line 2448, literal 3
    [==[Data/JAC_Absorption.sql]==],
    -- source line 2448, literal 4
    [==[Text/JAC_Absorption_Text.xml]==],
    -- source line 2449, literal 5
    [==[Scripts/JAC_Absorption_Rules.lua]==],
    -- source line 2449, literal 6
    [==[Scripts/JAC_Absorption_Gameplay.lua]==],
    -- source line 2450, literal 7
    [==[UI/JAC_Absorption_HeroKeys.lua]==],
    -- source line 2450, literal 8
    [==[UI/JAC_Absorption_HeroKeys.xml]==],
    -- source line 2453, literal 9
    [==[id]==],
    -- source line 2453, literal 10
    [==[./Dependencies/Mod]==],
    -- source line 2454, literal 11
    [==[4873eb62-8ccc-4574-b784-dda455e74e68]==],
    -- source line 2454, literal 12
    [==[bd7a82c5-6b38-4b69-a7f5-8dc9af534249]==],
    -- source line 2455, literal 13
    [==[8424840C-92EF-4426-A9B4-B4E0CB818049]==],
    -- source line 2456, literal 14
    [==[./ActionCriteria/Criteria/RuleSetInUse]==],
    -- source line 2456, literal 15
    [==[RULESET_JAC_BANSEIKI]==],
  },
  ["AbsorptionPackageTests.test_all_text_has_exactly_japanese_english_and_russian"]={
    -- source line 2460, literal 1
    [==[Text/JAC_Absorption_Text.xml]==],
    -- source line 2460, literal 2
    [==[./LocalizedText/Row]==],
    -- source line 2461, literal 3
    [==[Tag]==],
    -- source line 2461, literal 4
    [==[Language]==],
    -- source line 2463, literal 5
    [==[Text]==],
    -- source line 2466, literal 6
    [==[ja_JP]==],
    -- source line 2466, literal 7
    [==[en_US]==],
    -- source line 2466, literal 8
    [==[ru_RU]==],
    -- source line 2470, literal 9
    [==[__main__]==],
  },
}
