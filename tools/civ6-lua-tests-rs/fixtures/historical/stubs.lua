
local tables, civs, startTurn = ...
local function gameInfoTable(rows, key)
  local t = {}
  for i, row in ipairs(rows) do row.Index = i - 1; if key then t[row[key]] = row end end
  return setmetatable(t, { __call = function()
    local i = 0
    return function() i = i + 1; return rows[i] end
  end })
end
local speedTurns = {}  -- the navigation property GameSpeedTurnCollection: the speed's rows in row order
for _, row in ipairs(tables.GameSpeed_Turns) do
  if row.GameSpeedType == (tables.Speed or "GAMESPEED_STANDARD") then table.insert(speedTurns, row) end
end
GameInfo = {
  StartEras = { ERA_ANCIENT = { Year = tables.StartYear } },
  GameSpeeds = { [0] = { GameSpeedType = tables.Speed or "GAMESPEED_STANDARD", GameSpeedTurnCollection = speedTurns } },
  GameSpeed_Turns = gameInfoTable(tables.GameInfoSpeedTurns or tables.GameSpeed_Turns),
  Technologies = gameInfoTable(tables.Technologies, "TechnologyType"),
  Civics = gameInfoTable(tables.Civics, "CivicType"),
  Boosts = gameInfoTable(tables.Boosts),
}
for _, name in ipairs(tables.Names) do GameInfo[name] = gameInfoTable(tables[name]) end
CURRENT_TURN = startTurn
GameConfiguration = { GetGameSpeedType = function() return 0 end, GetStartTurn = function() return startTurn end }
local function properties()
  local values = {}
  return function(_, k) return values[k] end, function(_, k, v) values[k] = v end
end
Game = { GetCurrentGameTurn = function() return CURRENT_TURN end, GetLocalPlayer = function() return LOCAL end }
Game.GetProperty, Game.SetProperty = properties()
LOCAL = 0
BOOSTS, FAIL_BOOST, NOTES, RAISED = {}, {}, {}, {}
Players, PlayerConfigurations, MAJORS = {}, {}, {}
for i, civ in ipairs(civs) do
  local id = i - 1
  local techs, civics, triggered = {}, {}, {}
  local p = {
    techs = techs, civics = civics,
    GetID = function() return id end,
    AttachModifierByID = function() end,
    GetTechs = function() return {
      HasTech = function(_, t) return techs[t] == true end,
      SetTech = function(_, t, v) techs[t] = v end,
      HasBoostBeenTriggered = function(_, t) return triggered[t] == true end,
      TriggerBoost = function(_, t, source)
        if FAIL_BOOST[id] then error("engine refused") end
        triggered[t] = true; table.insert(BOOSTS, { id, "tech", t, source, CURRENT_TURN - startTurn + 1 })
      end,
    } end,
    GetCulture = function() return {
      HasCivic = function(_, c) return civics[c] == true end,
      SetCivic = function(_, c, v) civics[c] = v end,
      HasBoostBeenTriggered = function(_, c) return triggered["c" .. c] == true end,
      TriggerBoost = function(_, c, source)
        if FAIL_BOOST[id] then error("engine refused") end
        triggered["c" .. c] = true; table.insert(BOOSTS, { id, "civic", c, source, CURRENT_TURN - startTurn + 1 })
      end,
    } end,
  }
  p.GetProperty, p.SetProperty = properties()
  Players[id] = p
  PlayerConfigurations[id] = { GetCivilizationTypeName = function() return civ end }
  table.insert(MAJORS, p)
end
PlayerManager = { GetAliveMajors = function() return MAJORS end }
ParameterTypes = { MESSAGE = "MESSAGE", SUMMARY = "SUMMARY" }
NotificationTypes = { USER_DEFINED_1 = 1 }
NotificationManager = { SendNotification = function(id, _, data)
  table.insert(NOTES, { id, data.MESSAGE, data.SUMMARY, CURRENT_TURN - startTurn + 1 })
end }
ExposedMembers = {}
LOG = {}
function print(line) LOG[#LOG + 1] = line end
-- UI side
Locale = { Lookup = function(k) return k end }
local newGame = {}
LuaEvents = { TutorialUIRoot_AdvisorRaise = function(item) table.insert(RAISED, item) end,
  NewGameInitialized = { Add = function(f) table.insert(newGame, f) end } }
function NEW_GAME_INITIALIZED() for _, f in ipairs(newGame) do f() end end
local uiHandlers = {}
Events = {
  LocalPlayerTurnBegin = { Add = function(f) table.insert(uiHandlers, f) end },
  LocalPlayerChanged = { Add = function(f) table.insert(uiHandlers, f) end },
}
function UI_EVENT() for _, f in ipairs(uiHandlers) do f() end end
local handlers = {}
GameEvents = { OnGameTurnStarted = { Add = function(f) table.insert(handlers, f) end } }
-- A load re-runs the scripts; the properties stay (they are saved with the game).
function RELOAD() handlers = {}; uiHandlers = {} end
function START_TURN(turn)
  CURRENT_TURN = startTurn + turn - 1
  for _, f in ipairs(handlers) do f(CURRENT_TURN) end
end
