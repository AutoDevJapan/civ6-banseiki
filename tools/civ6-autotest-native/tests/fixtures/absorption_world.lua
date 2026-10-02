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
  GreatPersonIndividuals = {[42]={Index=42,GreatPersonIndividualType='GREAT_PERSON_TEST_GENERAL'}},
  Improvements = {IMPROVEMENT_BARBARIAN_CAMP={Index=1},IMPROVEMENT_GOODY_HUT={Index=2}},
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
RANDOM_CALLS, RANDOM_QUEUE = {}, {}
Game.GetRandNum=function(maximum,label)
  assert(type(maximum)=='number' and maximum>0 and type(label)=='string' and label~='', 'labelled engine roll required')
  RANDOM_CALLS[#RANDOM_CALLS+1]={Maximum=maximum,Label=label}
  local value=table.remove(RANDOM_QUEUE,1) or 0
  return value % maximum
end
Game.GetLocalPlayer=function() return 2 end
GameConfiguration={GetRuleSet=function() return 'RULESET_JAC_BANSEIKI' end,
  IsAnyMultiplayer=function() return false end,
  GetValue=function(key) return (key=='JAC_AUTOTEST' or key=='GAMEMODE_HEROES') and 1 or nil end}
ExposedMembers={}
local function event()
  local handlers={}
  return {Add=function(fn) handlers[#handlers+1]=fn end,
    Emit=function(...) for _,fn in ipairs(handlers) do fn(...) end end}
end
GameEvents={OnGameTurnStarted=event(),JAC_AbsorptionFlow_Operation=event()}
Players, PlayerConfigurations, CITIES, PLOTS = {}, {}, {}, {}
for owner=0,63 do
  PLAYER_ERAS[owner]=0;FAITH[owner]=0
  local p=prop({owner=owner,units={},cities={},techs={},civics={}})
  p.GetID=function() return owner end
  p.GetEras=function() return {GetEra=function() return PLAYER_ERAS[owner] end,
    SetEra=function(_,id) PLAYER_ERAS[owner]=id end} end
  p.IsAlive=function() return next(p.cities) ~= nil end
  p.IsHuman=function() return owner==2 end
  p.IsMajor=function() return owner<63 end
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
PlayerManager={GetAliveMajors=function()
  local result={};for owner=0,3 do if Players[owner]:IsAlive() then result[#result+1]=Players[owner] end end
  return result
end,GetAliveIDs=function()
  local result={};for owner=0,3 do if Players[owner]:IsAlive() then result[#result+1]=owner end end
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
    GetNumGreatWorkSlots=function() return 1 end,GetGreatWorkInSlot=function(_,id) return id==0 and (city.palaceWork or -1) or 1007 end
  } end
  city.ChangeLoyalty=function(self,n) self.loyalty=self.loyalty+n end
  city.GetCulturalIdentity=function(self) return {
    GetLoyalty=function() return self.loyalty end
  } end
  city.GetReligion=function(self) return {GetReligionsInCity=function()
    local result={};for id,n in pairs(self.faith) do result[#result+1]={Religion=tonumber(id),Followers=n} end;return result
  end} end
  Players[owner].cities[x]=city;CITIES[x]=city
  for index=x,x+1 do PLOTS[index]=PLOT(owner,index,x) end
  return city
end
function PLOT(owner,index,city)
  local plot={owner=owner,index=index,city=city,improvement=-1}
  plot.GetOwner=function(self) return self.owner end
  plot.GetIndex=function(self) return self.index end
  plot.GetX=function(self) return self.index end
  plot.GetY=function() return 0 end
  plot.GetImprovementType=function(self) return self.improvement end
  plot.IsWater=function() return false end
  plot.IsMountain=function() return false end
  plot.IsCity=function(self) return self.city==self.index end
  plot.GetDistrictType=function() return -1 end
  return plot
end
for i=0,19 do PLOTS[i]=PLOT(-1,i,nil) end
Map={GetPlotCount=function() return 20 end,GetPlotByIndex=function(i) return PLOTS[i] end,
  GetPlot=function(x,y) assert(y==0);return PLOTS[x] end}
ImprovementBuilder={SetImprovementType=function(plot,kind)
  assert(plot.improvement==-1,'occupied improvement');plot.improvement=kind
end}
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
