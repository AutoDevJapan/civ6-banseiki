-- Offline Civ API fixture. State is populated from the generated SQLite map, not scenario code.
assert(_VERSION == 'Lua 5.1')
State.turn, State.year, State.allowPassage = 1, 1534, true
State.researched, State.minors = {}, {}

function FixtureEvent()
  local event = {handlers={}, added=0, removed=0}
  event.Add = function(handler)
    assert(type(handler) == 'function')
    event.added = event.added + 1
    table.insert(event.handlers, handler)
  end
  event.Remove = function(handler)
    for i,value in ipairs(event.handlers) do
      if value == handler then table.remove(event.handlers, i); event.removed=event.removed+1; return end
    end
    error('unregistered event handler')
  end
  event.Fire = function(...) for _,handler in ipairs(event.handlers) do handler(...) end end
  return event
end
LuaEvents = {NewGameInitialized=FixtureEvent()}
Events = {CulturalIdentityCityConverted=FixtureEvent()}
ExposedMembers = {}

local function collection(items)
  return {
    GetCount=function() return #items end,
    Members=function() return ipairs(items) end,
    GetCapitalCity=function() for _,city in ipairs(items) do if city.capital then return city end end end
  }
end
local function info_rows(rows, key)
  local result={}
  for i,row in ipairs(rows) do row.Index=i; if key then result[row[key]]=row end end
  return setmetatable(result, {__call=function()
    local i=0
    return function() i=i+1; return rows[i] end
  end})
end
local resources={}
for _,name in ipairs(State.resources) do
  table.insert(resources, {ResourceType=name, PrereqTech=name=='RESOURCE_IRON' and 'TECH_JAC_SENGOKU_IRON_FORGING' or nil})
end
GameInfo = {
  Resources=info_rows(resources, 'ResourceType'),
  Resource_Harvests=info_rows({{ResourceType='RESOURCE_RICE', PrereqTech='TECH_JAC_SENGOKU_IRRIGATION_WORKS'}}, 'ResourceType'),
  Resource_ValidTerrains=info_rows({{ResourceType='RESOURCE_SILVER', TerrainType='TERRAIN_GRASS_HILLS'}}),
  Improvement_ValidResources=info_rows({
    {ResourceType='RESOURCE_JAC_GOLD', ImprovementType='IMPROVEMENT_MINE'},
    {ResourceType='RESOURCE_IRON', ImprovementType='IMPROVEMENT_MINE'},
    {ResourceType='RESOURCE_JAC_LACQUER', ImprovementType='IMPROVEMENT_PLANTATION'},
    {ResourceType='RESOURCE_TEA', ImprovementType='IMPROVEMENT_PLANTATION'},
    {ResourceType='RESOURCE_SILK', ImprovementType='IMPROVEMENT_PLANTATION'}
  }),
  Improvements={IMPROVEMENT_PLANTATION={Buildable=true, PrereqTech='TECH_JAC_SENGOKU_IRRIGATION_WORKS'}},
  Buildings={BUILDING_PALACE={Index=1}},
  Units={}, Technologies={}
}
for _,name in ipairs(State.unitTypes) do
  GameInfo.Units[name]={UnitType=name, Combat=(name=='UNIT_SETTLER' or name=='UNIT_BUILDER') and 0 or 10}
end
GameInfo.Units.UNIT_SETTLER={UnitType='UNIT_SETTLER', Combat=0}
for i,name in ipairs({'TECH_JAC_SENGOKU_MINE_MANAGEMENT','TECH_JAC_SENGOKU_CUPELLATION','TECH_JAC_SENGOKU_ORE_PROSPECTING'}) do
  GameInfo.Technologies[name]={Index=i}
end

Players, PlayerConfigurations = {}, {}
for slot,raw in ipairs(State.players) do
  local id=slot-1
  raw.alive=true
  PlayerConfigurations[id]={
    GetLeaderTypeName=function() return raw.leader end,
    GetCivilizationTypeName=function() return raw.civ end
  }
  for _,city in ipairs(raw.cities) do
    city.loyalty=100
    city.GetX=function() return city.plot%State.width end
    city.GetY=function() return math.floor(city.plot/State.width) end
    city.GetID=function() return city.plot end
    city.GetOwner=function() return id end
    city.GetPopulation=function() return city.pop end
    city.GetBuildings=function() return {HasBuilding=function(_,index)
      assert(index==GameInfo.Buildings.BUILDING_PALACE.Index)
      return city.buildings.BUILDING_PALACE==true
    end} end
    city.GetCulturalIdentity=function() return {
      GetLoyalty=function() return 100 end,
      GetLoyaltyPerTurn=function() return city.loyalty end
    } end
  end
  for _,unit in ipairs(raw.units) do
    unit.GetX=function() return unit.plot%State.width end
    unit.GetY=function() return math.floor(unit.plot/State.width) end
    unit.GetUnitType=function() return unit.kind end
  end
  Players[id]={
    GetID=function() return id end,
    IsAlive=function() return raw.alive end,
    IsMajor=function() return true end,
    GetCities=function() return collection(raw.cities) end,
    GetUnits=function() return collection(raw.units) end,
    GetDiplomacy=function() return {IsAtWarWith=function() return false end, HasOpenBordersFrom=function() return false end} end,
    GetTreasury=function() return {GetGoldBalance=function() return raw.gold end, GetGoldYield=function() return 10 end, GetTotalMaintenance=function() return 2 end} end,
    GetTechs=function() return {HasTech=function(_,index) return State.researched[index]==true end} end
  }
end
for _,plot in ipairs(State.plots) do
  plot.GetX=function() return plot.id%State.width end
  plot.GetY=function() return math.floor(plot.id/State.width) end
  plot.GetOwner=function() return plot.owner end
  plot.IsWater=function() return plot.water end
  plot.GetResourceType=function() return GameInfo.Resources[plot.resource] and GameInfo.Resources[plot.resource].Index or -1 end
end
Map={
  GetGridSize=function() return State.width, State.height end,
  GetPlotByIndex=function(index) return State.plots[index+1] end
}
PlayerManager={
  GetAliveMajors=function()
    local result={}
    for i,raw in ipairs(State.players) do if raw.alive then table.insert(result, Players[i-1]) end end
    return result
  end,
  GetAliveMinors=function() return State.minors end
}
function AddMinor(civ, cities)
  local id=22+#State.minors
  PlayerConfigurations[id]={GetCivilizationTypeName=function() return civ end}
  table.insert(State.minors, {GetID=function() return id end, GetCities=function() return {GetCount=function() return cities end} end})
end
Game={
  GetLocalPlayer=function() return State.localPlayer end,
  GetCurrentGameTurn=function() return State.turn end,
  GetProperty=function(_,name) return State.properties[name] end
}
GameConfiguration={GetStartTurn=function() return 1 end}
Calendar={MakeYearStr=function() return tostring(State.engineYear or State.year)..' AD' end}
UnitOperationTypes={PARAM_X='x', PARAM_Y='y', MOVE_TO=1}
UnitManager={
  GetReachableMovement=function() return {State.foreignPlot} end,
  CanStartOperation=function() return State.allowPassage end
}
function AttachResourceAPI()
  local size=State.width..'x'..State.height
  ExposedMembers.JAC_Resources={
    GetSites=function() return ExposedMembers.JAC_ResourceSiteData[size] end,
    CurrentYear=function() return State.year end
  }
end
function ReleaseSites(year)
  State.year=year
  State.turn=1+(year-1534)*3
  for _,site in ipairs(ExposedMembers.JAC_Resources.GetSites()) do
    local tech=site.tech and GameInfo.Technologies[site.tech]
    State.plots[site.plot+1].resource=(site.year<=year or (tech and State.researched[tech.Index])) and site.resource or ''
  end
end
