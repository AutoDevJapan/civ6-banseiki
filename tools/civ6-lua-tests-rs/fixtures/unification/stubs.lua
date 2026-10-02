
local CASTLE_SET, REBUILT, UNIFIED = ...
local WIDTH = 32
local castles = { {6, 5}, {6, 14}, {25, 5}, {25, 14} }
local reqs, args = {}, {}
for i, xy in ipairs(castles) do
  reqs[#reqs + 1] = { RequirementSetId = CASTLE_SET, RequirementId = "C" .. i }
  args[#args + 1] = { RequirementId = "C" .. i, Name = "X", Value = tostring(xy[1]) }
  args[#args + 1] = { RequirementId = "C" .. i, Name = "Y", Value = tostring(xy[2]) }
end
reqs[#reqs + 1] = { RequirementSetId = CASTLE_SET, RequirementId = "NONE" }
args[#args + 1] = { RequirementId = "NONE", Name = "X", Value = "-1" }
args[#args + 1] = { RequirementId = "NONE", Name = "Y", Value = "-1" }
local function iter(rows) return function() local i = 0; return function() i = i + 1; return rows[i] end end end
GameInfo = {
  Buildings = { [REBUILT] = { Index = 7 }, [UNIFIED] = { Index = 8 } },
  RequirementSetRequirements = iter(reqs),
  RequirementArguments = iter(args),
}
function SET_NO_CASTLES() reqs = { reqs[#reqs] }; GameInfo.RequirementSetRequirements = iter(reqs) end
local function properties()
  local values = {}
  return function(_, k) return values[k] end, function(_, k, v) values[k] = v end
end
START, TURN = 16603, 16603
Game = { GetCurrentGameTurn = function() return TURN end }
Game.GetProperty, Game.SetProperty = properties()
GameConfiguration = { GetStartTurn = function() return START end }
local plots = {}
Map = { GetPlot = function(x, y)
  local i = y * WIDTH + x
  plots[i] = plots[i] or { GetX = function() return x end, GetY = function() return y end, GetIndex = function() return i end }
  return plots[i]
end }
Map.GetPlotByIndex = function(i) return Map.GetPlot(i % WIDTH, math.floor(i / WIDTH)) end
CITIES, BUILT = {}, {}  -- plot index -> city; {building, plot} placed
function CITY(owner, x, y, capital)
  local buildings = {}
  local city = { owner = owner, capital = capital, buildings = buildings }
  city.GetOwner = function() return city.owner end
  city.GetX = function() return x end
  city.GetY = function() return y end
  city.GetID = function() return y * WIDTH + x end
  city.GetBuildings = function() return { HasBuilding = function(_, b) return buildings[b] == true end } end
  city.GetBuildQueue = function() return { CreateIncompleteBuilding = function(_, b, plot, pct)
    assert(plot == y * WIDTH + x and pct == 100); buildings[b] = true; table.insert(BUILT, { b, plot })
  end } end
  CITIES[y * WIDTH + x] = city
  return city
end
CityManager = { GetCityAt = function(x, y) return CITIES[y * WIDTH + x] end }
Players, PlayerConfigurations, MAJORS, ALLIED, WAR = {}, {}, {}, {}, {}
local CIVS = { "ODA", "DATE", "ASHINA", "SHIMAZU" }
for id, spec in ipairs({ { human = true }, {}, {}, { human = true } }) do
  local pid = id - 1
  local p = { team = pid }
  p.GetID = function() return pid end
  p.IsMajor = function() return true end
  p.IsAlive = function() return not p.dead end
  p.IsHuman = function() return spec.human == true end
  p.GetTeam = function() return p.team end
  p.GetDiplomacy = function() return {
    HasAllied = function(_, other) return ALLIED[pid .. "-" .. other] == true end,
    IsAtWarWith = function(_, other) return WAR[math.min(pid, other) .. "-" .. math.max(pid, other)] == true end,
  } end
  PlayerConfigurations[pid] = { GetCivilizationTypeName = function() return "CIVILIZATION_JAC_" .. CIVS[id] end }
  p.GetCities = function() return { GetCapitalCity = function()
    -- Lua 5.1 and newer Lua versions traverse this sparse integer hash in
    -- different orders. Preserve the original tests' capital return values:
    -- a house's own founding capital precedes conquered capital-tagged cities.
    local home = ({ [0] = 5 * WIDTH + 6, [1] = 5 * WIDTH + 25,
                    [2] = 10 * WIDTH + 16, [3] = 3 * WIDTH + 3 })[pid]
    local own = CITIES[home]
    if own and own.owner == pid and own.capital then return own end
    for _, c in pairs(CITIES) do if c.owner == pid and c.capital then return c end end
  end } end
  p.GetProperty, p.SetProperty = properties()
  Players[pid] = p
  MAJORS[#MAJORS + 1] = p
end
PlayerManager = { GetAliveMajors = function()
  local alive = {}
  for _, p in ipairs(MAJORS) do if not p.dead then alive[#alive + 1] = p end end
  return alive
end, GetWasEverAliveCount = function() return #MAJORS end }
ExposedMembers = {}
LOG = {}
function print(line) LOG[#LOG + 1] = line end
local handlers = {}
GameEvents = setmetatable({}, { __index = function(t, name)
  local event = { Add = function(f) handlers[name] = handlers[name] or {}; table.insert(handlers[name], f) end }
  rawset(t, name, event); return event
end })
function FIRE(name, ...) for _, f in ipairs(handlers[name] or {}) do f(...) end end
