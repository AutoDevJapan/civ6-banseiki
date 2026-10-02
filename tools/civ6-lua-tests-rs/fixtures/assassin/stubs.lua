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
  local u={GetID=function() return d.ID end, GetType=function() return d.Type end,GetName=function() return 'UNIT_'..d.Type end,
    GetOwner=function() return d.Owner end, GetX=function() return d.X end,GetY=function() return d.Y end,
    GetDamage=function() return d.Damage or 0 end,GetMaxDamage=function() return 100 end,
    ChangeDamage=function(_,delta)
      d.Damage=(d.Damage or 0)+delta
      if UI_HANDLERS and UI_HANDLERS.UnitDamageChanged then UI_HANDLERS.UnitDamageChanged(d.Owner,d.ID,d.Damage) end
    end}
  props(u,d); return u
end
function makeCity(d)
  local c={GetID=function() return d.ID end,GetOwner=function() return d.Owner end,GetName=function() return 'CITY_'..d.ID end,
    GetX=function() return d.X end,GetY=function() return d.Y end}
  props(c,d); return c
end
function rebuild()
  for id=0,MAX_MAJOR+1 do
    local d=DATA[id]
    local cities=memberCollection(d.Cities,makeCity)
    cities.GetCapitalCity=function() return cities:FindID(d.Capital) end
    local p={GetID=function() return id end,IsAlive=function() return d.Alive end,
      IsHuman=function() return id==0 end,IsTurnActive=function() return d.Alive end,IsMajor=function() return id<=MAX_MAJOR end,
      GetCities=function() return cities end,GetUnits=function() return memberCollection(d.Units,makeUnit) end,
      GetStats=function() return {GetMilitaryStrength=function() return d.Military end} end,
      GetReligion=function() return {GetReligionInMajorityOfCities=function() return d.Religion end} end,
      GetDiplomacy=function() return {IsAtWarWith=function(_,other) return d.Wars[other]==true end,
        HasAllied=function(_,other) return d.Allies[other]==true end,
        GetFavor=function() return d.Favor end,ChangeFavor=function(_,n) d.Favor=d.Favor+n end} end}
    props(p,d); Players[id]=p
    PlayerConfigurations[id]={GetLeaderTypeName=function() return 'LEADER_'..id end,
      GetLeaderName=function() assert(id<=MAX_MAJOR,'non-major has no leader'); return 'LEADER_'..id end,
      GetCivilizationDescription=function() return 'CIV_'..id end,
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
end,GetAlive=function()
  local t={}; for id=MAX_MAJOR+1,0,-1 do if DATA[id].Alive then t[#t+1]=Players[id] end end; return t
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
end,PlaceUnit=function(u,x,y)
  local d=DATA[u:GetOwner()].Units[u:GetID()]; d.X=x; d.Y=y
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
