
local tables, civs, startTurn, rulesPath = ...
local speedTurns = {}  -- the navigation property: the speed's GameSpeed_Turns rows in row order
for _, row in ipairs(tables.GameSpeed_Turns) do
  if row.GameSpeedType == "GAMESPEED_STANDARD" then table.insert(speedTurns, row) end
end
GameInfo = {
  StartEras = { ERA_ANCIENT = { Year = tables.StartYear } },
  GameSpeeds = { [0] = { GameSpeedType = "GAMESPEED_STANDARD", GameSpeedTurnCollection = speedTurns } },
  Yields = { YIELD_GOLD = { Index = 2 }, YIELD_CULTURE = { Index = 4 } },
  Units = { [0] = { Combat = 10, RangedCombat = 0 }, [1] = { Combat = 0, RangedCombat = 0 } },
}
CURRENT_TURN = startTurn
GameConfiguration = { GetGameSpeedType = function() return 0 end, GetStartTurn = function() return startTurn end }
local function properties()
  local values = {}
  PROPERTY_TABLES = PROPERTY_TABLES or {}
  table.insert(PROPERTY_TABLES, values)
  return function(_, k) return values[k] end, function(_, k, v) values[k] = v end
end
Game = { GetCurrentGameTurn = function() return CURRENT_TURN end }
Game.GetProperty, Game.SetProperty = properties()
WAR, WAR_CHANGED, ALLIED = {}, {}, {}
-- The engine's minimum war length (DIPLOMACY_WAR_MIN_TURNS): MakePeaceWith(b) silently does nothing before it
-- (court-arbitration-05); MakePeaceWith(b, true) makes peace at once (probe-make-peace, court-arbitration-06).
-- 0 = peace is always possible. FORCE_IGNORED simulates an engine where the second argument does nothing,
-- to exercise the script's retry-each-turn safety net.
MIN_WAR, FORCE_IGNORED = 0, false
local function pair(a, b) return math.min(a, b) .. "_" .. math.max(a, b) end
function SET_WAR(a, b, v) WAR[pair(a, b)] = v; WAR_CHANGED[pair(a, b)] = CURRENT_TURN end
local function canPeace(a, b) return CURRENT_TURN - (WAR_CHANGED[pair(a, b)] or 0) >= MIN_WAR end
-- Plots are keyed by x (y is always 0). A house's k-th city stands at x = 100 * id + k, so the plot's default
-- owner is floor(x / 100); PLOT_OWNER overrides it (a lost home). Houses are 99 tiles apart unless NEAR["a_b"].
PLOTS, PLOT_OWNER, NEAR = {}, {}, {}
local function plot(x)
  local p = PLOTS[x]
  if p == nil then
    p = {}
    p.GetIndex = function() return x end
    p.GetX = function() return x end
    p.GetY = function() return 0 end
    p.GetOwner = function() return PLOT_OWNER[x] or math.floor(x / 100) end
    p.GetProperty, p.SetProperty = properties()
    PLOTS[x] = p
  end
  return p
end
Map = {
  GetPlot = function(x, y) return plot(x) end,
  GetPlotByIndex = function(i) return plot(i) end,
  GetPlotDistance = function(x1, y1, x2, y2)
    local a, b = math.floor(x1 / 100), math.floor(x2 / 100)
    return NEAR[math.min(a, b) .. "_" .. math.max(a, b)] and 1 or 99
  end,
}
function FLAG(x, key) return plot(x):GetProperty(key) end
Players, PlayerConfigurations, MAJORS = {}, {}, {}
for i, civ in ipairs(civs) do
  local id = i - 1
  local p = { gold = 500, cities = 2, units = 4, weariness = 0, human = false }
  p.GetID = function() return id end
  p.IsMajor = function() return true end
  p.IsAlive = function() return true end
  p.IsHuman = function() return p.human end
  p.GetTreasury = function() return {
    GetGoldBalance = function() return p.gold end,
    ChangeGoldBalance = function(_, n) p.gold = p.gold + n end,
  } end
  local function cityList()
    local list = {}
    for k = 1, p.cities do
      local x = 100 * id + k
      -- Every yield is 10, +10% / -10% while the city's high / low flag is set (Data/JAC_CourtArbitration_Legitimacy.sql).
      local function yield()
        return 10 * (1 + 0.1 * ((plot(x):GetProperty("JAC_ARB_LEGITIMACY_HIGH") or 0) - (plot(x):GetProperty("JAC_ARB_LEGITIMACY_LOW") or 0)))
      end
      list[k] = { GetX = function() return x end, GetY = function() return 0 end, GetYield = yield,
        GetGrowth = function() return {GetAmenitiesLostFromWarWeariness = function() return p.weariness end} end }
    end
    return list
  end
  p.GetCities = function() return {
    GetCount = function() return p.cities end,
    Members = function() return ipairs(cityList()) end,
    GetCapitalCity = function() return cityList()[1] end,
  } end
  p.GetUnits = function() return { Members = function()
    local list = {}
    for k = 1, p.units do list[k] = {GetType = function() return 0 end, GetOwner = function() return id end} end
    -- Civilians must not contribute to the military count or its contraction.
    list[#list + 1] = {GetType = function() return 1 end}
    return ipairs(list)
  end } end
  p.GetDiplomacy = function() return {
    IsAtWarWith = function(_, o) return WAR[math.min(id, o) .. "_" .. math.max(id, o)] == true end,
    HasAllied = function(_, o) return ALLIED[pair(id, o)] == true end,
    DeclareWarOn = function(_, o) SET_WAR(id, o, true) end,
    MakePeaceWith = function(_, o, force)
      if (force == true and not FORCE_IGNORED) or canPeace(id, o) then SET_WAR(id, o, false) end
    end,
    CanMakePeaceWith = function(_, o) return canPeace(id, o) end,
  } end
  p.GetProperty, p.SetProperty = properties()
  Players[id] = p
  PlayerConfigurations[id] = {
    GetCivilizationTypeName = function() return civ end,
    GetCivilizationShortDescription = function() return "LOC_" .. civ .. "_NAME" end,
  }
  table.insert(MAJORS, p)
end
PlayerManager = { GetAliveMajors = function() return MAJORS end }
Locale = { Lookup = function(key, ...) return key end }
ParameterTypes = { MESSAGE = "MESSAGE", SUMMARY = "SUMMARY" }
WarTypes = { FORMAL_WAR = 0 }
NotificationTypes = { USER_DEFINED_1 = 1 }
NOTES = {}
NotificationManager = { SendNotification = function(id, _, data) table.insert(NOTES, { id, data.MESSAGE, data.SUMMARY }) end }
ExposedMembers = {}
unpack = unpack or table.unpack
LOG = {}
function print(line) LOG[#LOG + 1] = line end
function include(name)
  dofile(string.gsub(rulesPath, "JAC_CourtArbitration_Rules.lua$", name .. ".lua"))
end
local handlers = {}
GameEvents = setmetatable({}, { __index = function(t, name)
  local list = {}
  local ev = { Add = function(f) table.insert(list, f) end, Call = function(...) for _, f in ipairs(list) do f(...) end end }
  rawset(t, name, ev)
  return ev
end })
function START_TURN(turn)
  CURRENT_TURN = startTurn + turn - 1
  GameEvents.OnGameTurnStarted.Call(CURRENT_TURN)
end
function REQUEST(player, a, b) GameEvents.JAC_CourtArbitrationRequest.Call(player, { Target = a, Other = b, OnStart = "x" }) end
function RESPOND(player, accept) GameEvents.JAC_CourtArbitrationRespond.Call(player, { Accept = accept }) end
function JOIN(player, target) GameEvents.JAC_CourtArbitrationJoin.Call(player, { Target = target, OnStart = "x" }) end
-- The engine moves the city, then raises CityConquered(capturer, previous owner, city, x, y) (AlexanderScenario.lua:174).
function CONQUER(capturer, owner, x)
  PLOT_OWNER[x] = capturer
  Players[owner].cities = Players[owner].cities - 1
  Players[capturer].cities = Players[capturer].cities + 1
  GameEvents.CityConquered.Call(capturer, owner, 7, x, 0)
end
