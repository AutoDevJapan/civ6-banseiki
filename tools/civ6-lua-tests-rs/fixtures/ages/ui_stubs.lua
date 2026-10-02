
local viewSource = ...
-- UI side of the choice screen: controls record their state; UI.RequestPlayerOperation hands the operation to the
-- gameplay handler as the engine does for EXECUTE_SCRIPT (requesting player, parameter table).
LOCAL_PLAYER, TURN_ACTIVE, REQUESTS = 1, true, {}
local function control()
  -- Unlisted control methods (SetColor, SetIcon, ...) are no-ops.
  local c = setmetatable({ hidden = false, disabled = false }, { __index = function() return function() end end })
  function c:SetText(t) self.text = t end
  function c:GetText() return self.text end
  function c:SetHide(h) self.hidden = h end
  function c:IsHidden() return self.hidden end
  function c:SetDisabled(d) self.disabled = d end
  function c:RegisterCallback(_, f) self.click = f end
  function c:SetTexture(t) self.texture = t end
  function c:SetToolTipString(t) self.tooltip = t end
  function c:SetSelected(v) self.selected = v end
  function c:IsSelected() return self.selected == true end
  function c:SetRefreshHandler(f) self.refresh = f end
  function c:SetInputHandler(f) self.input = f end
  function c:RequestRefresh() if self.refresh then self.refresh() end end
  return c
end
CONTROL = control
-- LuaEvents: every name is an event; calling it runs its handlers (they are shared by all contexts, as live).
LuaEvents = setmetatable({}, { __index = function(t, name)
  local handlers = {}
  local event = setmetatable({ Add = function(f) table.insert(handlers, f) end },
    { __call = function(_, ...) for _, f in ipairs(handlers) do f(...) end end })
  rawset(t, name, event)
  return event
end })
Controls = setmetatable({}, { __index = function(t, k) local c = control(); rawset(t, k, c); return c end })
ContextPtr = control()
ROWS = {}
function include(name)
  if name == "JAC_SengokuAgesView" then assert(load(viewSource, name))()
  else assert(name == "InstanceManager") end
end
InstanceManager = { new = function() return {
  m_AllocatedInstances = {},
  ResetInstances = function(self) self.m_AllocatedInstances = {}; ROWS = self.m_AllocatedInstances end,
  GetInstance = function(self)
    local row = { Name = control(), Description = control(), Choose = control(), SelectCheck = control(),
      CommemorationIcon = control(), MomentCategory = control(), MomentBonuses = control() }
    table.insert(self.m_AllocatedInstances, row)
    ROWS = self.m_AllocatedInstances
    return row
  end,
} end }
Locale = { Lookup = function(key, ...) local a = { ... }; return key .. (a[1] and ("(" .. a[1] .. ")") or "") end }
Mouse = { eLClick = 1 }
PlayerOperations = { EXECUTE_SCRIPT = "EXECUTE_SCRIPT" }
Game.GetLocalPlayer = function() return LOCAL_PLAYER end
for _, p in pairs(Players) do p.IsTurnActive = function() return TURN_ACTIVE end end
local uiEvents = { turn = {}, turnEnd = {}, load = {}, publish = {} }
Events = {
  LocalPlayerTurnBegin = { Add = function(f) table.insert(uiEvents.turn, f) end },
  LocalPlayerTurnEnd = { Add = function(f) table.insert(uiEvents.turnEnd, f) end },
  LoadScreenClose = { Add = function(f) table.insert(uiEvents.load, f) end },
  GameCoreEventPublishComplete = { Add = function(f) table.insert(uiEvents.publish, f) end },
}
function LOCAL_TURN_BEGIN() for _, f in ipairs(uiEvents.turn) do f() end end
function LOCAL_TURN_END() for _, f in ipairs(uiEvents.turnEnd) do f() end end
function LOAD_SCREEN_CLOSE() for _, f in ipairs(uiEvents.load) do f() end end
function GAME_CORE_PUBLISH() for _, f in ipairs(uiEvents.publish) do f() end end
PUBLISH = GAME_CORE_PUBLISH
UI = { RequestPlayerOperation = function(id, op, params)
  table.insert(REQUESTS, { id = id, op = op, OnStart = params.OnStart })
  if op == "EXECUTE_SCRIPT" and params.OnStart == "JAC_SengokuDarkPolicyChoose" then CHOOSE(id, params) end
  if op == "EXECUTE_SCRIPT" and params.OnStart == "JAC_SengokuOathsChoose" then CHOOSE_OATHS(id, params) end
  GAME_CORE_PUBLISH()
end }
