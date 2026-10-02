-- Lua 5.1 model of the API boundary only. It never publishes a native receipt.
print = function() end
GAME_PROPERTIES = {}
TURN = 0
Game = {
  GetLocalPlayer = function() return 0 end,
  GetCurrentGameTurn = function() return TURN end,
  GetProperty = function(_, key) return GAME_PROPERTIES[key] end,
  SetProperty = function(_, key, value) GAME_PROPERTIES[key] = value end,
}
GameConfiguration = {IsAnyMultiplayer = function() return false end}
ExposedMembers = {}
Events = {}
GameEvents = setmetatable({}, {__index = function(events, name)
  local event = {handlers = {}}
  event.Add = function(handler) table.insert(event.handlers, handler) end
  rawset(events, name, event)
  return event
end})
ContextPtr = {SetHide = function() end, SetUpdate = function() end, ClearUpdate = function() end}
function include(_) return true end
for _, name in ipairs({'AddGovernorCandidate', 'AddSecretGovernorCandidate',
  'AddGovernorAppointed', 'AddSecretGovernorAppointed', 'OnAppointGovernor',
  'OnPromoteGovernor', 'OnNameButton'}) do _G[name] = function() end end

local SLOT, PROXY = 'GOVERNOR_JAC_NATIVE_SLOT', 'GOVERNOR_JAC_TITLE_PROXY'
GameInfo = {Governors = {}, GovernorsCannotAssign = {}, Types = {}, Civics = {}}
GameInfo.Governors[SLOT] = {GovernorType=SLOT, Hash=301, Index=1,
  IdentityPressure=8, TransitionStrength=100, AssignCityState=0}
GameInfo.Governors[PROXY] = {GovernorType=PROXY, Hash=302, Index=2}
GameInfo.Governors[1] = GameInfo.Governors[SLOT]
GameInfo.Governors[2] = GameInfo.Governors[PROXY]
GameInfo.GovernorsCannotAssign[PROXY] = {CannotAssign=1}
local personas, civics = {}, {}
for id = 1, 22 do
  personas[id] = {LeaderType='LEADER_TEST_'..id, GovernorType='GOVERNOR_PERSONA_'..id}
end
for id = 1, 15 do
  local name = 'CIVIC_TEST_'..id
  civics[id] = {CivicType=name}
  GameInfo.Civics[name] = {Index=id}
end
local function iterator(rows)
  local i = 0
  return function() i=i+1; return rows[i] end
end
GameInfo.JAC_LeaderPersonas = setmetatable({}, {
  __call = function() return iterator(personas) end
})
for _, row in ipairs(personas) do GameInfo.JAC_LeaderPersonas[row.LeaderType] = row end
GameInfo.JAC_PersonaTitleCivics = function() return iterator(civics) end

local function governor(hash)
  local instance = {hash=hash, city=nil, site=0, established=false, neutralized=0}
  function instance:GetAssignedCity() return self.city end
  function instance:IsEstablished() return self.established end
  function instance:GetTurnsToEstablish() return 5 end
  function instance:GetTurnsOnSite() return self.site end
  function instance:GetNeutralizedTurns() return self.neutralized end
  return instance
end
local city = {id=1, owner=0, props={}}
function city:GetID() return self.id end
function city:GetOwner() return self.owner end
function city:GetProperty(key) return self.props[key] end
function city:GetCulturalIdentity()
  return {GetIdentitySourcesDetailedBreakdown=function() return {} end}
end
CAPITAL = city
Players = {}
local majors = {}
for id = 0, 21 do
  local gs = {points=0, spent=0, native={}}
  function gs:GetGovernorPoints() return self.points end
  function gs:GetGovernorPointsSpent() return self.spent end
  function gs:HasGovernor(hash) return self.native[hash] ~= nil end
  function gs:GetGovernor(hash) return self.native[hash] end
  function gs:CanAppointGovernor(hash)
    return self.native[hash] == nil and self.points > self.spent
  end
  function gs:CanAssignGovernor(hash, destination)
    return hash == 301 and self.native[hash] ~= nil and destination == city
  end
  local culture = {acquired={}}
  function culture:HasCivic(index) return self.acquired[index] == true end
  function culture:GetCultureCost(_) return 1 end
  function culture:SetCulturalProgress(index, _)
    if not self.acquired[index] then self.acquired[index]=true; gs.points=gs.points+1 end
  end
  local player = {id=id, props={}, gs=gs, culture=culture}
  function player:GetID() return self.id end
  function player:IsHuman() return self.id == 0 end
  function player:IsTurnActive() return self.id == 0 end
  function player:IsAlive() return true end
  function player:IsMajor() return true end
  function player:GetGovernors() return self.gs end
  function player:GetCulture() return self.culture end
  function player:GetProperty(key) return self.props[key] end
  function player:SetProperty(key, value) self.props[key]=value end
  function player:GetCities()
    local list = self.id == 0 and {city} or {}
    return {Members=function() return ipairs(list) end,
      GetCapitalCity=function() return list[1] end}
  end
  function player:GetUnits() return {Members=function() return ipairs({}) end} end
  Players[id] = player
  majors[#majors+1] = player
end
PlayerManager = {GetAliveMajors=function() return majors end}
PlayerOperations = {EXECUTE_SCRIPT='EXECUTE_SCRIPT', APPOINT_GOVERNOR='APPOINT_GOVERNOR',
  ASSIGN_GOVERNOR='ASSIGN_GOVERNOR', PROMOTE_GOVERNOR='PROMOTE_GOVERNOR',
  PARAM_GOVERNOR_TYPE='governor', PARAM_PLAYER_ONE='owner',
  PARAM_CITY_DEST='city', PARAM_GOVERNOR_PROMOTION_TYPE='promotion'}
OPERATIONS = {}
UI = {IsGameCoreBusy=function() return false end}
function UI.RequestPlayerOperation(owner, operation, params)
  OPERATIONS[#OPERATIONS+1] = {owner=owner, operation=operation, params=params}
  if operation == PlayerOperations.EXECUTE_SCRIPT then
    local handlers = GameEvents[params.OnStart].handlers
    if #handlers ~= 1 then error('synchronized GameCore handler missing') end
    return handlers[1](owner, params)
  end
  local row = GameInfo.Governors[params[PlayerOperations.PARAM_GOVERNOR_TYPE]]
  local gs = Players[owner]:GetGovernors()
  if operation == PlayerOperations.APPOINT_GOVERNOR then
    if not gs:CanAppointGovernor(row.Hash) then error('engine appointment denied') end
    gs.native[row.Hash] = governor(row.Hash)
    gs.spent = gs.spent+1
  elseif operation == PlayerOperations.ASSIGN_GOVERNOR then
    if row.Hash ~= 301 then error('proxy cannot be assigned') end
    gs.native[row.Hash].city = city
  elseif operation ~= PlayerOperations.PROMOTE_GOVERNOR then
    error('unexpected operation '..tostring(operation))
  end
  -- The engine fixture deliberately does not write Player/City receipts.
end
