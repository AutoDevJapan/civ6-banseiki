-- Human-facing view of saved flow snapshots. Only the gameplay operation changes a flow.
include("InstanceManager")

local entryInstances = InstanceManager:new("AbsorptionEntry", "Root", Controls.Entries)
local actionInstances = InstanceManager:new("AbsorptionAction", "Root", Controls.Actions)
local flow, selectedID, ownerChoice, dismissed, requestPending
local targets, focusKey, rendered = {}, nil, {}

local function L(tag, ...)
  return Locale.Lookup("LOC_JAC_ABS_FLOW_" .. tag, ...)
end

local function Localized(value)
  if value == nil then return nil end
  if type(value) == "table" then
    if value.Name then return Localized(value.Name) end
    local parts = {}
    for _, part in ipairs(value) do parts[#parts + 1] = Localized(part) or "" end
    return table.concat(parts, ", ")
  end
  local s = tostring(value)
  local known = { ADMINISTRATION=true, DILIGENT=true, LOYAL=true, DEVOUT=true,
    ARMOR=true, GOVERNOR=true, EQUIPMENT=true, ARTIFACT=true, COLLECTION=true }
  if known[s] then return L("VALUE_" .. s) end
  return s:sub(1, 4) == "LOC_" and Locale.Lookup(s) or s
end

local function EntryName(entry)
  if entry.Leader and flow then
    local config = PlayerConfigurations[flow.From]
    if config then return Locale.Lookup(config:GetLeaderName()) end
  end
  return Localized(entry.Name) or L("UNNAMED", tostring(entry.ID))
end

local function EntryIdentity(entry)
  local source = entry.Source or {}
  local role = source.Role or source.Type or entry.Role or entry.Type
  if role then return L("ROLE_ID", Localized(role), tostring(source.ID or entry.ID)) end
  return tostring(source.ID or entry.ID)
end

local function EntryAbilities(entry)
  local source = entry.Source or {}
  local values = {}
  for _, field in ipairs({ "Abilities", "Ability", "Traits", "Trait", "Personality", "Equipment" }) do
    local value = entry[field] or source[field]
    if value then values[#values + 1] = Localized(value) end
  end
  if #values == 0 and entry.UnitType then
    local unit = GameInfo.Units[entry.UnitType]
    if unit and unit.Description then values[#values + 1] = Locale.Lookup(unit.Description) end
  end
  return #values > 0 and table.concat(values, ", ") or L("NOT_RECORDED")
end

local function PlayerName(id)
  local config = id ~= nil and PlayerConfigurations[id]
  if config then return Locale.Lookup(config:GetCivilizationDescription()) end
  return L("PLAYER", tostring(id))
end

local function ActionName(action)
  if type(action) ~= "string" then return L("UNKNOWN", tostring(action)) end
  local known = {
    SURRENDER=true, INCORPORATE=true, DISSOLVE=true, ESCAPE=true, RESIST=true,
    CAPTURE=true, RECRUIT=true, SUBMIT=true, EXILE=true, EXECUTE=true,
    RETIRE=true, DEFECT=true, PROTECT=true, RECONTRACT=true, HIDE=true,
    SUICIDE=true, TRANSFER=true, LOOT=true, SEIZE=true, RETURN=true,
    SCATTER=true, INHERIT=true, PRESERVE=true, REESCAPE=true,
    RE_ESCAPE=true, FAILED=true, BLOCKED=true,
  }
  return known[action] and L("ACTION_" .. action) or L("UNKNOWN", action)
end

local function OutcomeName(outcome)
  if type(outcome) == "table" then
    return ActionName(outcome.Outcome or outcome.Action or outcome.Result)
  end
  return ActionName(outcome)
end

local function KindName(kind)
  local known = { UNIT=true, PERSON=true, HERO=true, GREAT_PERSON=true,
    ITEM=true, UNLOCK=true, RELIGION=true, LEADER=true, GOVERNOR=true }
  return known[kind] and L("KIND_" .. kind) or L("UNKNOWN", tostring(kind))
end

local function PhaseName(phase)
  local known = { CHOICE=true, RESULT=true, DONE=true, APPLYING=true, BLOCKED=true }
  return known[phase] and L("PHASE_" .. phase) or L("UNKNOWN", tostring(phase))
end

local function ModeName(mode)
  local known = { CONQUEST=true, SMALL_STATE=true, MERGER=true }
  return known[mode] and L("MODE_" .. mode) or L("UNKNOWN", tostring(mode))
end

local function CanSend()
  local id = Game.GetLocalPlayer()
  local player = id and id >= 0 and Players[id]
  return flow ~= nil and player ~= nil and player:IsHuman() and player:IsTurnActive()
    and id == flow.To and not requestPending
end

local function CurrentEntry()
  if not flow then return nil end
  for _, entry in ipairs(flow.Entries or {}) do
    if entry.ID == selectedID then return entry end
  end
  return nil
end

local function EligibleOwners()
  local owners = {}
  if not flow then return owners end
  for _, player in ipairs(PlayerManager.GetAlive() or {}) do
    local id = player:GetID()
    if id ~= flow.From and id ~= flow.To and player:IsAlive()
      and player:GetCities():GetCount() > 0 then
      owners[#owners + 1] = id
    end
  end
  table.sort(owners)
  return owners
end

local function AllChosen()
  if not flow or flow.Phase ~= "CHOICE" then return false, 0, 0 end
  local selected, required = 0, 0
  for _, entry in ipairs(flow.Entries or {}) do
    if not entry.EscapeOutcome then
      required = required + 1
      if entry.SelectedAction then selected = selected + 1 end
    end
  end
  return selected == required, selected, required
end

local function AddTarget(key, activate, marker, scroll, index, count)
  targets[#targets + 1] = { Key=key, Activate=activate, Marker=marker,
    Scroll=scroll, Index=index, Count=count }
end

local function SetFocus(key)
  focusKey = key
  local focused
  for _, target in ipairs(targets) do
    if target.Marker then target.Marker:SetHide(target.Key ~= key) end
    if target.Key == key and target.Scroll then
      target.Scroll:SetScrollValue((target.Index - 1) / math.max(1, target.Count - 1))
    end
  end
  if key == "confirm" then focused = L("CONFIRM")
  elseif key == "retry" then focused = L("RETRY")
  elseif key == "ack" then focused = L("ACK")
  elseif key == "close" then focused = L("CLOSE") end
  Controls.Hint:SetText((rendered.hint or "") .. (focused and (" " .. L("FOCUSED", focused)) or ""))
  rendered.focus = focusKey
end

local function Send(command, assetID, action, returnOwner)
  if not CanSend() then return false end
  local params = { OnStart="JAC_AbsorptionFlow_Operation", Command=command, FlowID=flow.ID,
    AssetID=assetID, Action=action, ReturnOwner=returnOwner }
  requestPending = true
  local ok, err = pcall(UI.RequestPlayerOperation, Game.GetLocalPlayer(),
    PlayerOperations.EXECUTE_SCRIPT, params)
  if not ok then
    requestPending = false
    print("[JAC_AbsorptionFlow UI] operation request failed: " .. tostring(err))
  end
  return ok
end

local Draw
local function SelectEntry(id)
  if not flow then return false end
  for _, entry in ipairs(flow.Entries or {}) do
    if entry.ID == id then
      if selectedID ~= id then ownerChoice = nil end
      selectedID = id
      focusKey = "entry:" .. tostring(id)
      Draw()
      return true
    end
  end
  return false
end

local function ChooseAction(action)
  local entry = CurrentEntry()
  if not entry or flow.Phase ~= "CHOICE" or entry.EscapeOutcome or not CanSend() then return false end
  local allowed = false
  for _, offered in ipairs(entry.AllowedActions or {}) do
    if offered == action then allowed = true; break end
  end
  if not allowed then return false end
  if action == "RETURN" or action == "EXILE" then
    if #EligibleOwners() == 0 then return false end
    ownerChoice = { AssetID=entry.ID, Action=action }
    focusKey = "owner:" .. tostring(EligibleOwners()[1])
    Draw()
    return true
  end
  ownerChoice = nil
  return Send("SELECT", entry.ID, action)
end

local function ChooseOwner(id)
  if not ownerChoice or not CanSend() then return false end
  for _, owner in ipairs(EligibleOwners()) do
    if owner == id then
      local choice = ownerChoice
      ownerChoice = nil
      return Send("SELECT", choice.AssetID, choice.Action, id)
    end
  end
  return false
end

local function Resolve()
  if flow and (flow.Phase == "APPLYING"
    or flow.Phase == "BLOCKED" and flow.AppliedPlan) and CanSend() then
    return Send("RESOLVE")
  end
  if not AllChosen() or not CanSend() then return false end
  return Send("RESOLVE")
end

local function Acknowledge()
  if not flow or flow.Phase ~= "RESULT" or not CanSend() then return false end
  return Send("ACK")
end

local function Close()
  if flow then dismissed = tostring(flow.ID) .. ":" .. tostring(flow.Phase) end
  Controls.Panel:SetHide(true)
  rendered.visible = false
  return true
end

local function AddAction(key, label, callback, enabled, index, count)
  local row = actionInstances:GetInstance()
  row.Name:SetText(label)
  row.Choose:SetDisabled(not enabled)
  row.Choose:RegisterCallback(Mouse.eLClick, callback)
  AddTarget(key, callback, row.Focus, Controls.ActionScroll, index, count)
  rendered.actions[#rendered.actions + 1] = { key=key, text=row.Name:GetText(), enabled=enabled }
end

Draw = function()
  if not flow then return end
  local entries = flow.Entries or {}
  local complete, selected, required = AllChosen()
  rendered = { id=flow.ID, phase=flow.Phase, rows={}, actions={},
    visible=not Controls.Panel:IsHidden(), canResolve=complete and CanSend() }
  targets = {}
  entryInstances:ResetInstances()
  actionInstances:ResetInstances()
  local choosing = flow.Phase == "CHOICE"
  Controls.EntryScroll:SetSizeY(choosing and 266 or 430)
  Controls.ActionHeading:SetHide(not choosing)
  Controls.ActionScroll:SetHide(not choosing)

  Controls.Title:SetText(L(flow.Phase == "RESULT" and "RESULT_TITLE" or "TITLE"))
  Controls.Parties:SetText(L("PARTIES", PlayerName(flow.From), PlayerName(flow.To), ModeName(flow.Mode)))
  Controls.Status:SetText(flow.Phase == "CHOICE" and L("PROGRESS", selected, required)
    or PhaseName(flow.Phase))
  Controls.Instructions:SetText(L(flow.Phase == "CHOICE" and "KEYS_CHOICE" or "KEYS_RESULT"))

  for index, entry in ipairs(entries) do
    local row = entryInstances:GetInstance()
    local name = EntryName(entry)
    local identity = Localized(entry.Identity) or EntryIdentity(entry)
    local abilities = EntryAbilities(entry)
    local outcome
    if flow.Phase == "RESULT" or flow.Phase == "DONE" or flow.Phase == "BLOCKED" then
      local final = entry.FinalOutcome or entry.EscapeOutcome or entry.SelectedAction
      local result = entry.EscapeOutcome and L("ESCAPED_RESULT")
        or entry.Success == false and L("FAILURE")
        or entry.Success == true and L("SUCCESS") or L("RESULT_PENDING")
      outcome = L("FINAL", result, final and OutcomeName(final) or L("NONE"))
      if entry.RequestedOutcome or entry.Success == false and entry.SelectedAction then
        outcome = outcome .. " " .. L("REQUESTED",
          ActionName(entry.RequestedOutcome or entry.SelectedAction))
      end
      if entry.RetryEscapeOutcome then
        outcome = outcome .. " " .. L("REESCAPE", OutcomeName(entry.RetryEscapeOutcome))
      end
      local cause = entry.TransferCause or entry.FailureReason or entry.Reason or entry.Error
      if cause then
        local detail = type(cause) == "string" and cause:sub(1, 4) == "LOC_"
          and Locale.Lookup(cause) or L("TRANSFER_FAILURE")
        outcome = outcome .. " " .. L("REASON", detail)
      end
    elseif entry.EscapeOutcome then
      outcome = L("ESCAPED", OutcomeName(entry.EscapeOutcome))
    elseif entry.SelectedAction then
      outcome = L("SELECTED", ActionName(entry.SelectedAction))
      if entry.ReturnOwner ~= nil or entry.SelectedReturnOwner ~= nil then
        outcome = outcome .. " " .. L("TO_OWNER", PlayerName(entry.ReturnOwner or entry.SelectedReturnOwner))
      end
    else
      outcome = L("UNSELECTED")
    end
    row.Name:SetText(name)
    row.Identity:SetText(L("IDENTITY", KindName(entry.Kind), identity))
    row.Abilities:SetText(L("ABILITIES", abilities))
    row.Outcome:SetText(outcome)
    row.Portrait:SetHide(true)
    if type(entry.Portrait) == "string" and entry.Portrait ~= "" then
      local ok, found = pcall(row.Portrait.SetIcon, row.Portrait, entry.Portrait)
      row.Portrait:SetHide(not ok or found == false)
      row.Portrait:SetToolTipString(L("PORTRAIT", entry.Portrait))
    end
    local portrait = entry.Portrait and L("PORTRAIT", tostring(entry.Portrait)) or L("PORTRAIT_NONE")
    row.Choose:SetToolTipString(portrait .. "[NEWLINE]" .. row.Identity:GetText()
      .. "[NEWLINE]" .. row.Abilities:GetText() .. "[NEWLINE]" .. outcome)
    row.Choose:SetText(L("INSPECT"))
    row.Choose:RegisterCallback(Mouse.eLClick, function() SelectEntry(entry.ID) end)
    local key = "entry:" .. tostring(entry.ID)
    AddTarget(key, function() SelectEntry(entry.ID) end, row.Focus,
      Controls.EntryScroll, index, #entries)
    rendered.rows[#rendered.rows + 1] = { id=entry.ID, name=row.Name:GetText(),
      identity=row.Identity:GetText(), abilities=row.Abilities:GetText(),
      outcome=row.Outcome:GetText(), portrait=entry.Portrait, selected=entry.SelectedAction,
      escaped=entry.EscapeOutcome ~= nil, success=entry.Success }
  end
  Controls.Entries:CalculateSize()
  Controls.EntryScroll:CalculateInternalSize()

  local entry = CurrentEntry()
  if flow.Phase == "CHOICE" and entry and not entry.EscapeOutcome then
    if ownerChoice and ownerChoice.AssetID == entry.ID then
      local owners = EligibleOwners()
      Controls.ActionHeading:SetText(L("CHOOSE_OWNER", ActionName(ownerChoice.Action)))
      for index, owner in ipairs(owners) do
        AddAction("owner:" .. tostring(owner), PlayerName(owner),
          function() ChooseOwner(owner) end, CanSend(), index, #owners + 1)
      end
      AddAction("owner:cancel", L("BACK"), function()
        ownerChoice = nil; focusKey = "entry:" .. tostring(entry.ID); Draw(); return true
      end, true, #owners + 1, #owners + 1)
    else
      ownerChoice = nil
      local offered = entry.AllowedActions or {}
      Controls.ActionHeading:SetText(L("CHOOSE_ACTION", EntryName(entry)))
      for index, action in ipairs(offered) do
        local needsOwner = action == "RETURN" or action == "EXILE"
        local label = ActionName(action)
        if needsOwner then label = L("ACTION_WITH_OWNER", label) end
        AddAction("action:" .. tostring(action), label,
          function() return ChooseAction(action) end,
          CanSend() and (not needsOwner or #EligibleOwners() > 0), index, #offered)
      end
    end
  else
    Controls.ActionHeading:SetText(entry and L("DETAIL", EntryName(entry))
      or L("DETAIL_NONE"))
  end
  Controls.Actions:CalculateSize()
  Controls.ActionScroll:CalculateInternalSize()

  -- Place the selected entry's actions immediately after that entry in the keyboard order.
  -- Otherwise Tab would select another entry before reaching the current choices.
  local ordered, inserted = {}, false
  for i = 1, #entries do
    ordered[#ordered + 1] = targets[i]
    if entries[i].ID == selectedID then
      for j = #entries + 1, #targets do ordered[#ordered + 1] = targets[j] end
      inserted = true
    end
  end
  if not inserted then
    for j = #entries + 1, #targets do ordered[#ordered + 1] = targets[j] end
  end
  targets = ordered

  local retryable = flow.Phase == "BLOCKED" and flow.AppliedPlan ~= nil
  Controls.ConfirmButton:SetHide(flow.Phase ~= "CHOICE" and flow.Phase ~= "RESULT"
    and flow.Phase ~= "APPLYING" and not retryable)
  if flow.Phase == "CHOICE" then
    Controls.ConfirmButton:SetText(L("CONFIRM"))
    Controls.ConfirmButton:SetDisabled(not complete or not CanSend())
    Controls.Hint:SetText(complete and L("READY") or L("NEED_CHOICES"))
    AddTarget("confirm", Resolve)
  elseif flow.Phase == "RESULT" then
    Controls.ConfirmButton:SetText(L("ACK"))
    Controls.ConfirmButton:SetDisabled(not CanSend())
    Controls.Hint:SetText(L("ACK_HINT"))
    AddTarget("ack", Acknowledge)
  elseif flow.Phase == "APPLYING" then
    Controls.ConfirmButton:SetText(L("CONTINUE"))
    Controls.ConfirmButton:SetDisabled(not CanSend())
    Controls.Hint:SetText(L("WAIT_HINT"))
    AddTarget("continue", Resolve)
  elseif retryable then
    Controls.ConfirmButton:SetText(L("RETRY"))
    Controls.ConfirmButton:SetDisabled(not CanSend())
    Controls.Hint:SetText(L("RETRY_HINT"))
    AddTarget("retry", Resolve)
  else
    Controls.Hint:SetText(L(flow.Phase == "BLOCKED" and "BLOCKED_HINT" or "WAIT_HINT"))
  end
  rendered.hint = Controls.Hint:GetText()
  AddTarget("close", Close)

  local exists = false
  for _, target in ipairs(targets) do if target.Key == focusKey then exists = true; break end end
  if not exists then focusKey = selectedID and "entry:" .. tostring(selectedID) or "close" end
  SetFocus(focusKey)
end

local function Refresh(autoOpen)
  local id = Game.GetLocalPlayer()
  local api = ExposedMembers.JAC_AbsorptionFlow
  local ok, snapshot = false, nil
  if api and api.GetPending and id and id >= 0 then
    ok, snapshot = pcall(api.GetPending, id)
  end
  if not ok then snapshot = nil end
  local oldKey = flow and (tostring(flow.ID) .. ":" .. tostring(flow.Phase))
  local newKey = snapshot and (tostring(snapshot.ID) .. ":" .. tostring(snapshot.Phase))
  if newKey ~= oldKey then
    selectedID, ownerChoice, focusKey = nil, nil, nil
    dismissed = nil
  end
  flow = snapshot
  Controls.OpenButton:SetHide(flow == nil or flow.Phase == "DONE")
  if flow then
    Controls.OpenButton:SetText(L(flow.Phase == "CHOICE" and "OPEN"
      or flow.Phase == "RESULT" and "OPEN_RESULT" or "OPEN_STATUS"))
  end
  if not flow or flow.Phase == "DONE" then
    Controls.Panel:SetHide(true)
    rendered = { visible=false }
    return false
  end
  if not CurrentEntry() then
    for _, entry in ipairs(flow.Entries or {}) do
      if not entry.EscapeOutcome then selectedID = entry.ID; break end
    end
    if not selectedID and flow.Entries and flow.Entries[1] then selectedID = flow.Entries[1].ID end
  end
  if autoOpen and dismissed ~= newKey then Controls.Panel:SetHide(false) end
  Draw()
  return true
end

local function Open()
  ContextPtr:SetHide(false)
  requestPending = false
  if not Refresh(false) then return false end
  dismissed = nil
  Controls.Panel:SetHide(false)
  Draw()
  return true
end

local function Navigate(step)
  if Controls.Panel:IsHidden() or #targets == 0 then return false end
  local current = 0
  for i, target in ipairs(targets) do if target.Key == focusKey then current = i; break end end
  local nextIndex = ((current - 1 + step) % #targets) + 1
  local target = targets[nextIndex]
  focusKey = target.Key
  if target.Key:sub(1, 6) == "entry:" then
    for _, entry in ipairs(flow.Entries or {}) do
      if "entry:" .. tostring(entry.ID) == target.Key then
        selectedID = entry.ID
        ownerChoice = nil
        Draw()
        break
      end
    end
  else
    SetFocus(focusKey)
  end
  return true
end

local function Activate()
  if Controls.Panel:IsHidden() then return false end
  for _, target in ipairs(targets) do
    if target.Key == focusKey then return target.Activate() == true end
  end
  return false
end

-- The scenario driver uses these handlers and reads the actual rendered labels/buttons.
JAC_AbsorptionFlow_UI = {
  Open=Open, Close=Close, Refresh=function() return Refresh(false) end,
  Navigate=Navigate, Activate=Activate, SelectEntry=SelectEntry,
  ChooseAction=ChooseAction, ChooseOwner=ChooseOwner,
  Resolve=Resolve, Acknowledge=Acknowledge,
  GetView=function() return rendered end,
}

Controls.OpenButton:RegisterCallback(Mouse.eLClick, Open)
Controls.CloseButton:RegisterCallback(Mouse.eLClick, Close)
Controls.ConfirmButton:RegisterCallback(Mouse.eLClick, function()
  if flow and (flow.Phase == "CHOICE" or flow.Phase == "APPLYING"
    or flow.Phase == "BLOCKED" and flow.AppliedPlan) then Resolve()
  elseif flow and flow.Phase == "RESULT" then Acknowledge() end
end)
ContextPtr:SetInputHandler(function(input)
  if Controls.Panel:IsHidden() or input:GetMessageType() ~= KeyEvents.KeyUp then return false end
  local key = input:GetKey()
  if key == Keys.VK_ESCAPE then return Close() end
  if key == Keys.VK_TAB or key == Keys.VK_DOWN or key == Keys.VK_RIGHT then return Navigate(1) end
  if key == Keys.VK_UP or key == Keys.VK_LEFT then return Navigate(-1) end
  if key == Keys.VK_RETURN then return Activate() end
  return false
end, true)
Events.GameCoreEventPublishComplete.Add(function() requestPending = false; Refresh(true) end)
Events.LocalPlayerTurnBegin.Add(function() requestPending = false; Refresh(true) end)
Events.LocalPlayerTurnEnd.Add(function() Controls.Panel:SetHide(true); rendered.visible = false end)
Events.LoadScreenClose.Add(function()
  ContextPtr:SetHide(false)
  dismissed = nil; requestPending = false; Refresh(true)
end)
ContextPtr:SetHide(false)
Refresh(true)
