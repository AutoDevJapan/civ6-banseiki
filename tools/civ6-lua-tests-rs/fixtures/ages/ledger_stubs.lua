
local tables, civs, startTurn, speed = ...
function include(name)
  assert(name == "JAC_Sengoku_Oaths", name)
  assert(load(tables.OathLua, name))()
end
local function gameInfoTable(rows, key)
  local t = {}
  for i, row in ipairs(rows) do row.Index = i - 1; if key then t[row[key]] = row end end
  return setmetatable(t, { __call = function()
    local i = 0
    return function() i = i + 1; return rows[i] end
  end })
end
local techs, civics = {}, {}
for i = 1, 10 do techs[i] = { TechnologyType = "T" .. i } end
for i = 1, 10 do civics[i] = { CivicType = "C" .. i } end
speed = speed or "GAMESPEED_STANDARD"
GameInfo = {
  StartEras = { ERA_ANCIENT = { Year = tables.StartYear } },
  GameSpeeds = { [0] = { GameSpeedType = speed, CostMultiplier = tables.Multiplier[speed] } },
  Eras = gameInfoTable(tables.Eras, "EraType"),
  Eras_XP1 = gameInfoTable(tables.Eras_XP1, "EraType"),
  Technologies = gameInfoTable(techs), Civics = gameInfoTable(civics),
  JAC_SengokuReigns = gameInfoTable(tables.JAC_SengokuReigns),
  JAC_SengokuDarkAgePolicies = gameInfoTable(tables.JAC_SengokuDarkAgePolicies),
  JAC_SengokuAgeEventScores = gameInfoTable(tables.JAC_SengokuAgeEventScores),
  JAC_SengokuDarkAgePolicyDefaults = gameInfoTable(tables.JAC_SengokuDarkAgePolicyDefaults),
  JAC_SengokuOaths = gameInfoTable(tables.JAC_SengokuOaths, "OathType"),
  JAC_SengokuOathAges = gameInfoTable(tables.JAC_SengokuOathAges),
}
CURRENT_TURN = startTurn
GameConfiguration = { GetGameSpeedType = function() return 0 end, GetStartTurn = function() return startTurn end }
local function properties()
  local values = {}
  return function(_, k) return values[k] end, function(_, k, v) values[k] = v end
end
-- ENGINE_ERA: what Game.GetEras():GetCurrentEra() returns; nil = no Game.GetEras (the call fails).
Game = { GetCurrentGameTurn = function() return CURRENT_TURN end,
  GetEras = function() return { GetCurrentEra = function() return ENGINE_ERA end } end }
Game.GetProperty, Game.SetProperty = properties()
PLOTS = {}
Map = { GetPlot = function(x, y)
  local index = y * 100 + x
  if not PLOTS[index] then
    local p = { GetIndex = function() return index end, GetX = function() return x end, GetY = function() return y end,
      IsCoastalLand = function() return COASTAL and COASTAL[index] == true end }
    p.GetProperty, p.SetProperty = properties()
    PLOTS[index] = p
  end
  return PLOTS[index]
end }
CITIES = {}          -- list of {owner, x, y, capital}
ALLIED, FRIENDS, TEAM, ALIVE = {}, {}, {}, {}
HUMAN, WAR, GOLD, COASTAL = {[1]=true,[3]=true}, {}, {}, {}
NO_FRIENDSHIP_API = false  -- true: the gameplay diplomacy object has no HasDeclaredFriendship
Players, PlayerConfigurations = {}, {}
local function citiesOf(id)
  local list = {}
  for _, c in ipairs(CITIES) do if c.owner == id then table.insert(list, c) end end
  return list
end
local function cityObject(c) return { GetX = function() return c.x end, GetY = function() return c.y end } end
FREE_CITIES = 62  -- a non-major player (the free cities; a city-state behaves the same here)
local function makePlayer(id, civ)
  ALIVE[id], TEAM[id] = true, id
  local p = { research = { techs = {}, civics = {} } }
  p.GetID = function() return id end
  p.IsHuman = function() return HUMAN[id] == true end
  p.GetTreasury = function() return {GetGoldBalance = function() return GOLD[id] or 100 end} end
  p.GetTeam = function() return TEAM[id] end
  p.GetCities = function() return {
    Members = function()
      local list, n = citiesOf(id), 0
      return function() n = n + 1; if list[n] then return n, cityObject(list[n]) end end
    end,
    GetCount = function() return #citiesOf(id) end,
    GetCapitalCity = function()
      for _, c in ipairs(citiesOf(id)) do if c.capital then return cityObject(c) end end
    end,
  } end
  p.GetTechs = function() return { HasTech = function(_, t) return p.research.techs[t] == true end } end
  p.GetCulture = function() return { HasCivic = function(_, c) return p.research.civics[c] == true end } end
  p.GetDiplomacy = function()
    local d = { HasAllied = function(_, b)
      local a, bb = math.min(id, b), math.max(id, b); return ALLIED[a .. "-" .. bb] == true end,
      IsAtWarWith = function(_, b) return WAR[id .. "-" .. b] == true end }
    if not NO_FRIENDSHIP_API then d.HasDeclaredFriendship = function(_, b)
      local a, bb = math.min(id, b), math.max(id, b); return FRIENDS[a .. "-" .. bb] == true end end
    return d
  end
  p.GetProperty, p.SetProperty = properties()
  Players[id] = p
  PlayerConfigurations[id] = { GetCivilizationTypeName = function() return civ end }
end
for i, civ in ipairs(civs) do makePlayer(i - 1, civ) end
makePlayer(FREE_CITIES, "CIVILIZATION_FREE_CITIES")
local function alive(ids)
  local list = {}
  for _, id in ipairs(ids) do if ALIVE[id] then table.insert(list, Players[id]) end end
  return list
end
local majors = {}
for i = 1, #civs do majors[i] = i - 1 end
PlayerManager = {
  GetAliveMajors = function() return alive(majors) end,
  GetAlive = function()  -- the free cities come first, as the order is not the ID order live
    local ids = { FREE_CITIES }
    for _, id in ipairs(majors) do table.insert(ids, id) end
    return alive(ids)
  end,
}
ParameterTypes = { MESSAGE = "MESSAGE", SUMMARY = "SUMMARY" }
NotificationParameterTypes = { CanUserDismiss = "CanUserDismiss" }
NotificationTypes = { USER_DEFINED_2 = 2, USER_DEFINED_3 = 3, COMMEMORATION_AVAILABLE = 71 }
EndTurnBlockingTypes = { NO_ENDTURN_BLOCKING = -1, ENDTURN_BLOCKING_COMMEMORATION_AVAILABLE = 71 }
NOTES, NOTIFICATIONS = {}, {}
NotificationManager = { SendNotification = function(id, kind, data)
  table.insert(NOTES, { id, kind, data.MESSAGE, data.SUMMARY, CURRENT_TURN - startTurn + 1 })
  local n = { player = id, kind = kind, data = data, id = #NOTIFICATIONS + 1, dismissed = false }
  function n:GetPlayerID() return self.player end
  function n:GetID() return self.id end
  function n:GetType() return self.kind end
  function n:GetValue(key) return self.data[key] end
  function n:IsDismissed() return self.dismissed end
  -- Model official notifications separately; USER_DEFINED_2 has no native blocker.
  -- The policy tests execute the real MOD ActionPanel guard rather than assuming DLL behavior.
  function n:GetEndTurnBlocking() return self.kind == 71 and 71 or -1 end
  function n:CanUserDismiss() return self.data.CanUserDismiss ~= false end
  table.insert(NOTIFICATIONS, n)
end }
NotificationManager.FindType = function(kind, id)
  for _, n in ipairs(NOTIFICATIONS) do if n.kind == kind and n.player == id and not n.dismissed then return n end end
end
NotificationManager.Find = function(id, index)
  local n = NOTIFICATIONS[index]; if n and n.player == id then return n end
end
NotificationManager.GetList = function(id)
  local ids = {}
  for _, n in ipairs(NOTIFICATIONS) do if n.player == id then table.insert(ids, n.id) end end
  return ids
end
NotificationManager.Dismiss = function(id, index) NotificationManager.Find(id, index).dismissed = true end
NotificationManager.GetAllEndTurnBlocking = function(id)
  local kinds = {}
  for _, n in ipairs(NOTIFICATIONS) do
    if n.player == id and not n.dismissed and n:GetEndTurnBlocking() ~= -1 then table.insert(kinds, n:GetEndTurnBlocking()) end
  end
  return kinds
end
LOG = {}
function print(line) LOG[#LOG + 1] = line end
local handlers = { turn = {}, conquered = {}, built = {}, choose = {}, oaths = {} }
function RESET_HANDLERS() handlers = { turn = {}, conquered = {}, built = {}, choose = {}, oaths = {} } end
GameEvents = {
  OnGameTurnStarted = { Add = function(f) table.insert(handlers.turn, f) end },
  CityConquered = { Add = function(f) table.insert(handlers.conquered, f) end },
  CityBuilt = { Add = function(f) table.insert(handlers.built, f) end },
  JAC_SengokuDarkPolicyChoose = { Add = function(f) table.insert(handlers.choose, f) end },
  JAC_SengokuOathsChoose = { Add = function(f) table.insert(handlers.oaths, f) end },
}
function CHOOSE_OATHS(playerID, params)
  for _, f in ipairs(handlers.oaths) do f(playerID, params) end
end
-- EXECUTE_SCRIPT from the choice screen: the engine passes the requesting player and the parameter table.
function CHOOSE(playerID, params)
  for _, f in ipairs(handlers.choose) do f(playerID, params) end
end
function START_TURN(turn)
  CURRENT_TURN = startTurn + turn - 1
  for _, f in ipairs(handlers.turn) do f(CURRENT_TURN) end
end
local function find(x, y) for i, c in ipairs(CITIES) do if c.x == x and c.y == y then return c, i end end end
function FOUND(owner, x, y, capital, silent)
  table.insert(CITIES, { owner = owner, x = x, y = y, capital = capital })
  if not silent then for _, f in ipairs(handlers.built) do f(owner, #CITIES, x, y) end end
end
function CAPTURE(capturer, x, y, raze)
  local c = find(x, y); local loser = c.owner; c.owner = capturer; c.capital = false
  for _, f in ipairs(handlers.conquered) do f(capturer, loser, 1, x, y) end
  if raze then local _, i = find(x, y); table.remove(CITIES, i) end
end
function TRANSFER(to, x, y)  -- CityManager.TransferCity: CityBuilt fires (live 2026-09-29)
  local c = find(x, y); c.owner = to; c.capital = false
  for _, f in ipairs(handlers.built) do f(to, 1, x, y) end
end
function LEDGER() return Game:GetProperty("JAC_AGES_LEDGER") end
