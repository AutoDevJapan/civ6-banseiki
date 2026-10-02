
local FACTIONS = { "HOJO", "IMAGAWA", "SHIMAZU", "RYUKYU" }
ID = {}
TURN = 1
ALLIED, WAR, CALLS = {}, {}, {}  -- ALLIED/WAR keyed by the unordered pair "lo-hi"
local function pair(a, b) if a > b then a, b = b, a end return a .. "-" .. b end
local props = {}
Game = { GetCurrentGameTurn = function() return TURN end,
  GetProperty = function(_, k) return props[k] end, SetProperty = function(_, k, v) props[k] = v end }
GameConfiguration = { GetStartTurn = function() return 1 end }
WarTypes = { FORMAL_WAR = 7 }
PlayerConfigurations, Players, local_majors = {}, {}, {}
for i, name in ipairs(FACTIONS) do
  local id = i - 1
  ID[name] = id
  PlayerConfigurations[id] = { GetCivilizationTypeName = function() return "CIVILIZATION_JAC_" .. name end }
  local d = {}
  function d:SetHasMet() end
  function d:SetHasDeclaredFriendship() end
  function d:RecheckVisibilityOnAll() end
  function d:HasAllied(b) return ALLIED[pair(id, b)] == true end
  function d:IsAtWarWith(b) return WAR[pair(id, b)] == true end
  function d:SetHasAllied(b, value)
    table.insert(CALLS, "SetHasAllied " .. id .. ">" .. b .. " " .. tostring(value))
    if value then ALLIED[pair(id, b)] = true end  -- false is ignored while allied (live diplo-17)
  end
  function d:DeclareWarOn(b, warType, force)
    table.insert(CALLS, "DeclareWarOn " .. id .. ">" .. b .. " " .. tostring(warType) .. " " .. tostring(force))
    WAR[pair(id, b)] = true  -- the alliance stays (live end-alliance-01)
  end
  function d:MakePeaceWith(b, force)
    table.insert(CALLS, "MakePeaceWith " .. id .. ">" .. b .. " " .. tostring(force) .. " turn " .. TURN)
    WAR[pair(id, b)] = nil
  end
  Players[id] = { GetID = function() return id end, GetDiplomacy = function() return d end }
  local_majors[#local_majors + 1] = Players[id]
end
PlayerManager = { GetAliveMajors = function() return local_majors end }
ExposedMembers = {}
LOG = {}
function print(line) LOG[#LOG + 1] = line end
local handlers = {}
local function events()
  return setmetatable({}, { __index = function(t, name)
    local event = { Add = function(f) handlers[name] = handlers[name] or {}; table.insert(handlers[name], f) end }
    rawset(t, name, event); return event
  end })
end
GameEvents, LuaEvents, Events = events(), events(), events()
function FIRE(name, ...) for _, f in ipairs(handlers[name] or {}) do f(...) end end
function PAIR(a, b) return pair(ID[a], ID[b]) end
function COUNT(prefix) local n = 0; for _, c in ipairs(CALLS) do if c:sub(1, #prefix) == prefix then n = n + 1 end end; return n end
