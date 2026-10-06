-- Test-only InGame UI addin. JAC_HotseatPlan.lua is generated per run by Rust.
include("JAC_HotseatPlan")
local plan = JAC_HOTSEAT_PLAN
if not plan or type(plan.nonce) ~= "string" or #plan.nonce ~= 32 then return end
if not GameConfiguration.IsHotseat() then return end
if GameConfiguration.GetValue("JAC_HOTSEAT_BRIDGE_NONCE") ~= plan.nonce then return end
-- AddUserInterfaces starts hidden. This empty context receives keys but draws nothing.
ContextPtr:SetHide(false)

local nonce = plan.nonce
local scene = string.gsub(tostring(ContextPtr), "[^%w]", "") .. "." ..
    tostring(Game.GetCurrentGameTurn()) .. "." .. tostring(Game.GetLocalPlayer())
local used = {}
local booted = false
local lastShown = nil
local lastReady = nil
local lastError = nil
local awaitingTurnReady = nil
local NIL = {}
local serial = 0
local inputSamples = 0

local function value(v)
  if v == nil or v == NIL then return "nil" end
  return type(v) .. ":" .. tostring(v)
end

local function append(items, v)
  items[#items + 1] = v == nil and NIL or v
end

local function fields(items)
  local out = {}
  for i = 1, #items do
    local s = value(items[i])
    out[i] = tostring(#s) .. ":" .. s
  end
  return table.concat(out)
end

local function emit(kind, step, body)
  serial = serial + 1
  local hex = {}
  for i = 1, #body do hex[i] = string.format("%02x", string.byte(body, i)) end
  local encoded = table.concat(hex)
  local count = math.max(1, math.ceil(#encoded / 192))
  for i = 1, count do
    print("JAC_HS|1|" .. nonce .. "|" .. scene .. "|" .. tostring(serial) .. "|" .. tostring(step) .. "|" .. kind ..
      "|" .. tostring(i) .. "/" .. tostring(count) .. "|" ..
      string.sub(encoded, (i - 1) * 192 + 1, i * 192))
  end
end

local function dialog()
  local control = ContextPtr:LookUpControl("/InGame/PlayerChange")
  if not control then error("PlayerChange context missing") end
  local shown = not control:IsHidden() and UIManager:IsInPopupQueue(control)
  if not shown then return false, false end
  local panel = ContextPtr:LookUpControl("/InGame/PlayerChange/PopupAlphaIn")
  local button = ContextPtr:LookUpControl("/InGame/PlayerChange/OkButton")
  if not panel or not button then error("PlayerChange turn controls missing") end
  local id = Game.GetLocalPlayer()
  local config = PlayerConfigurations[id]
  local player = Players[id]
  if not config or not player then error("PlayerChange player missing") end
  local ready = not panel:IsHidden() and not button:IsHidden() and not button:IsDisabled()
      and config:GetHotseatPassword() == "" and player:IsHuman()
      and player:IsTurnActive() and GameConfiguration.IsPaused()
  return true, ready == true
end

local function snapshot()
  local shown, ready = dialog()
  local localID = Game.GetLocalPlayer()
  local current = Players[localID]
  if not current then error("local player missing") end
  local result = {Game.GetCurrentGameTurn(), GameConfiguration.GetStartTurn(), localID,
    current:IsTurnActive(), current:IsHuman(), GameConfiguration.IsHotseat(),
    GameConfiguration.IsPaused(), shown, ready}
  for pid = 0, 1 do
    local player = Players[pid]
    if not player then error("human player " .. pid .. " missing") end
    append(result, player:IsHuman())
    append(result, player:IsTurnActive())
    local units = {}
    for _, unit in player:GetUnits():Members() do
      if unit:GetProperty("JAC_POC_GROWTHUI_WARLORD") then units[#units + 1] = unit end
    end
    table.sort(units, function(a, b) return a:GetID() < b:GetID() end)
    if #units ~= 2 then error("expected two growth units for player " .. pid .. ", got " .. #units) end
    for _, unit in ipairs(units) do
      local record = unit:GetProperty("JAC_POC_GROWTHUI_WARLORD")
      if type(record) ~= "table" or type(record.Equipment) ~= "table" then
        error("incomplete growth record for " .. pid .. ":" .. unit:GetID())
      end
      append(result, pid)
      append(result, unit:GetID())
      append(result, record.Level)
      append(result, record.XP)
      append(result, record.Personality)
      append(result, record.Trait)
      append(result, fields(record.Equipment))
      append(result, record.Artifact)
      append(result, record.LastXPTurn)
    end
  end
  return fields(result), shown, ready, result[4], result[5], result[7], result[1], result[3]
end

local function observe()
  local ok, observed, shown, ready, active, human, paused, turn, localID = pcall(snapshot)
  if not ok then
    if observed ~= lastError then emit("WITNESS_ERROR", 0, tostring(observed)); lastError = observed end
    return
  end
  lastError = nil
  if not booted then
    booted = true
    emit("BOOT", 0, observed)
    -- Diagnostics never acknowledge a step or include text typed by a player.
    print("JAC_HS_KEYS|1|" .. nonce .. "|" .. scene .. "|" ..
      tostring(KeyEvents.KeyDown) .. "|" .. tostring(KeyEvents.KeyUp) .. "|" ..
      tostring(Keys and Keys.F13) .. "|" .. tostring(Keys and Keys.VK_F13))
  end
  if shown and not ready and (lastShown ~= true or lastReady == true) then
    emit("DIALOG_WAIT", 0, observed)
  end
  if ready and lastReady ~= true then emit("DIALOG_OPEN", 0, observed) end
  if shown then awaitingTurnReady = nil end
  if not shown and lastShown == true then
    emit("DIALOG_CLOSED", 0, observed)
    awaitingTurnReady = {turn, localID}
  end
  if awaitingTurnReady and not shown then
    if turn ~= awaitingTurnReady[1] or localID ~= awaitingTurnReady[2] then
      emit("WITNESS_ERROR", 0, "turn/player changed before unpaused turn became ready")
      awaitingTurnReady = nil
    elseif active and human and not paused then
      emit("TURN_READY", 0, observed)
      awaitingTurnReady = nil
    end
  end
  lastShown = shown
  lastReady = ready
end

local function execute(step, action)
  if used[step] then emit("DUPLICATE", step, "same step received twice"); return end
  used[step] = true
  local beforeOK, before = pcall(snapshot)
  if not beforeOK then emit("ERROR", step, tostring(before)); return end
  if action == "end_turn" or action == "save" or action == "exit_to_menu" then
    local shown = dialog()
    local id = Game.GetLocalPlayer()
    if shown or GameConfiguration.IsPaused() or not Players[id]:IsHuman() or not Players[id]:IsTurnActive() then
      emit("ERROR", step, "active unpaused human turn required"); return
    end
  end
  emit("BEFORE", step, before)
  if action == "ui_lua" then
    local ok, result = pcall(plan.queries[step])
    if ok then emit("RESULT", step, value(result))
    else emit("ERROR", step, tostring(result)) end
  elseif action == "end_turn" then
    local ok, result = pcall(function() return UI.RequestAction(ActionTypes.ACTION_ENDTURN) end)
    if ok and result ~= false then emit("REQUESTED", step, before)
    else emit("ERROR", step, tostring(result)) end
  elseif action == "save" then
    local name = plan.save_names[step]
    local saveType = Network.GetGameConfigurationSaveType()
    if not name or saveType == nil then emit("ERROR", step, "save name or engine save type missing"); return end
    local function completed()
      Events.SaveComplete.Remove(completed)
      emit("SAVE_COMPLETE", step, name)
    end
    Events.SaveComplete.Add(completed)
    emit("REQUESTED", step, name)
    local ok, result = pcall(function()
      return Network.SaveGame({Name=name, Location=SaveLocations.LOCAL_STORAGE,
        Type=saveType, IsAutosave=false, IsQuicksave=false})
    end)
    if not ok or result == false then
      Events.SaveComplete.Remove(completed)
      emit("ERROR", step, tostring(result))
    else
      emit("SAVE_RETURNED", step, name)
    end
  elseif action == "exit_to_menu" then
    emit("REQUESTED", step, before)
    Events.ExitToMainMenu()
  else
    emit("ERROR", step, "unsupported action")
  end
end

local function onInput(input)
  if not booted then return false end
  local message = input:GetMessageType()
  local key = input:GetKey()
  if (message == KeyEvents.KeyDown or message == KeyEvents.KeyUp) and
      (key == 0 or key == 13 or plan.query_keys[key] or plan.action_keys[key]) and
      inputSamples < 32 then
    inputSamples = inputSamples + 1
    print("JAC_HS_INPUT|1|" .. nonce .. "|" .. scene .. "|" ..
      tostring(message) .. "|" .. tostring(key))
  end
  if message ~= KeyEvents.KeyUp then return false end
  local step = plan.query_keys[key]
  if step then execute(step, "ui_lua"); return true end
  local action = plan.action_keys[key]
  if action then
    local turn = Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn()
    local id = Game.GetLocalPlayer()
    step = plan.action_steps[action][tostring(turn) .. ":" .. tostring(id)]
    if not step then emit("ERROR", 0, "unexpected turn/player for " .. action); return true end
    execute(step, action)
    return true
  end
  return false
end

ContextPtr:SetInputHandler(onInput, true)
ContextPtr:SetUpdate(observe)
