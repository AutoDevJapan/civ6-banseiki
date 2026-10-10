-- Frontend does not accept the gameplay map criteria. Keep the installed UI,
-- and change only its two twelve-slot clamps in this Lua context.
-- A reload can clear helper exports while include still considers them loaded.
-- Build the private policy afresh each time this context is initialized.
local japanMaps = {
  ['{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_20km.Civ6Map'] = true,
  ['{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_20km_Settlers.Civ6Map'] = true,
  ['{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map'] = true,
  ['{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km_Settlers.Civ6Map'] = true,
}
local function hotseatMaxPlayers(originalLimit)
  if GameConfiguration.IsHotseat()
    and GameConfiguration.GetValue('RULESET') == 'RULESET_JAC_BANSEIKI'
    and japanMaps[MapConfiguration.GetValue('MAP_SCRIPT')]
    and MapConfiguration.GetMaxMajorPlayers() == 22 then
    return 22
  end
  return originalLimit
end
local originalMath = math
local contextMath = {}
for key, value in pairs(originalMath) do contextMath[key] = value end
math = setmetatable(contextMath, { __index = originalMath })
print('JAC_JP_HS|HOOK_LOADED')
local scopeReported = false
math.min = function(...)
  if select('#', ...) == 2 then
    local first, second = ...
    if first == 22 and second == 12 then
      local limit = hotseatMaxPlayers(second)
      if not scopeReported then
        print('JAC_JP_HS|SCOPE|' .. tostring(GameConfiguration.IsHotseat()) .. '|' ..
          tostring((GameConfiguration.GetValue('RULESET'))) .. '|' ..
          tostring((MapConfiguration.GetValue('MAP_SCRIPT'))) .. '|' ..
          tostring((MapConfiguration.GetMaxMajorPlayers())) .. '|' .. tostring(limit))
        scopeReported = true
      end
      if limit == 22 then return 22 end
    end
  end
  return originalMath.min(...)
end
include("StagingRoom")

-- The stock context owns the player list, game summary and chat. Attach the
-- extra settings to its summary instead of replacing the installed UI.
if Controls then
  assert(Controls.JAC_AssignmentPanel and Controls.GameSummaryStack,
    "Japan staging controls are missing")

  local MODE = "JAC_MP_MODE"
  local ROSTER = "JAC_MP_ROSTER"
  local SUBJECT = "JAC_MP_SUBJECT"
  local TEAM = "JAC_MP_TEAM"
  local PRIOR_NO_TEAMS = "JAC_MP_PRIOR_NO_TEAMS"
  local RESTORE_DONE = "JAC_MP_RESTORE_DONE"
  -- A reused slot must not receive the previous occupant's saved team.
  local REJOINING = "REJOINING"
  local VERIFY = "VERIFY"
  local JAPAN = "JAPAN"
  local DIFFERENT = "DIFFERENT"
  local choices = {
    { code = "EAST", first = 1, last = 23, tag = "LOC_JAC_MP_AREA_EAST" },
    { code = "WEST", first = 24, last = 47, tag = "LOC_JAC_MP_AREA_WEST" },
    { code = "KYUSHU", first = 40, last = 47, tag = "LOC_JAC_MP_AREA_KYUSHU" },
    { code = "HOKKAIDO", first = 1, last = 1, tag = "LOC_JAC_MP_AREA_HOKKAIDO" },
  }
  local choiceByCode = {}
  for _, choice in ipairs(choices) do choiceByCode[choice.code] = choice end
  for i = 1, 47 do
    local code = string.format("P%02d", i)
    local choice = {
      code = code, first = i, last = i,
      tag = string.format("LOC_JAC_MP_PREF_%02d", i),
    }
    choices[#choices + 1] = choice
    choiceByCode[code] = choice
  end

  local rows = InstanceManager:new("JAC_AssignmentRow", "Root", Controls.JAC_AssignmentRows)
  local pulldownManagers = {}
  local syncing = false
  local departing = {}
  local pendingRejoin = {}
  local configDirty = false
  local configAttempts = {}
  local teamAttempts = {}
  local function resetAttempts()
    local pendingRoster = configAttempts[ROSTER]
    configAttempts = {}
    if pendingRoster and type(pendingRoster.value) == "string" then
      local saved = GameConfiguration.GetValue(ROSTER)
      if saved ~= pendingRoster.value then
        configAttempts[ROSTER] = { value = pendingRoster.value }
      end
    end
    teamAttempts = {}
  end
  local lastError = nil
  local guardedReadyCheck = false
  local stockOnShow = OnShow
  local stockOnReadyButton = OnReadyButton
  local stockCheckGameAutoStart = CheckGameAutoStart
  local stockSetLocalReady = SetLocalReady
  local stockUpdateReadyButton = UpdateReadyButton

  local function read(key)
    -- The engine returns no Lua values for an unset configuration key.
    -- Normalize that to one nil so callers such as tonumber receive a value.
    local value = GameConfiguration.GetValue(key)
    return value
  end
  local function sameValue(actual, wanted)
    if actual == wanted then return true end
    if type(wanted) == "number" and type(actual) == "string" then
      return tonumber(actual) == wanted
    end
    if type(wanted) == "string" and type(actual) == "number" then
      return wanted == tostring(actual)
    end
    if type(wanted) == "boolean" then
      local encoded = wanted and "1" or "0"
      return actual == tonumber(encoded) or actual == encoded
        or actual == tostring(wanted)
    end
    return false
  end
  local function fingerprint(value)
    return type(value) .. ":" .. tostring(value)
  end
  local function write(key, value)
    local current = read(key)
    if sameValue(current, value) then
      configAttempts[key] = nil
      return
    end
    local target = fingerprint(value)
    local observed = fingerprint(current)
    local previous = configAttempts[key]
    if previous and previous.target == target
      and previous.observed == observed then return end
    configAttempts[key] = { target = target, observed = observed, value = value }
    GameConfiguration.SetValue(key, value)
    configDirty = true
  end
  local function mode()
    return read(MODE) == JAPAN and JAPAN or DIFFERENT
  end
  local function assignmentKey(id) return "JAC_MP_AREA_" .. tostring(id) end
  local function priorTeamKey(id) return "JAC_MP_PRIOR_TEAM_" .. tostring(id) end
  local function identityKey(id) return "JAC_MP_ID_" .. tostring(id) end
  local function retiringKey(id) return "JAC_MP_RETIRING_" .. tostring(id) end
  local function networkIdentity(id)
    if GameConfiguration.IsHotseat() then return nil end
    local player = PlayerConfigurations[id]
    if not player or not player.GetNetworkIdentifer then return nil end
    local identifier = player:GetNetworkIdentifer()
    if identifier == nil or identifier == "" then return nil end
    return tostring(identifier)
  end
  local function scoped()
    return GameConfiguration.GetValue("RULESET") == "RULESET_JAC_BANSEIKI"
      and japanMaps[MapConfiguration.GetValue("MAP_SCRIPT")] == true
      and MapConfiguration.GetMaxMajorPlayers() == 22
      and (GameConfiguration.IsHotseat() or Network.IsInSession())
  end
  local function canEdit()
    return GameConfiguration.IsHotseat() or Network.IsGameHost()
  end
  local function humans()
    local result = {}
    for _, id in ipairs(GameConfiguration.GetParticipatingPlayerIDs()) do
      local player = PlayerConfigurations[id]
      if player and player:GetSlotStatus() == SlotStatus.SS_TAKEN
        and not departing[id] then
        result[#result + 1] = id
      end
    end
    table.sort(result)
    return result
  end
  local function humanSet(ids)
    local set = {}
    for _, id in ipairs(ids) do set[id] = true end
    return set
  end
  local function rosterIds()
    local ids = {}
    local saved = read(ROSTER)
    if type(saved) ~= "string" and configAttempts[ROSTER] then
      saved = configAttempts[ROSTER].value
    end
    if type(saved) == "string" then
      for part in string.gmatch(saved, "%d+") do
        local id = tonumber(part)
        if id then ids[#ids + 1] = id end
      end
    end
    return ids
  end
  local function rosterValue(ids)
    local parts = {}
    for _, id in ipairs(ids) do parts[#parts + 1] = tostring(id) end
    return table.concat(parts, ",")
  end
  local function nameFor(id)
    local player = PlayerConfigurations[id]
    return player and Locale.Lookup(player:GetPlayerName()) or tostring(id)
  end
  local function overlaps(a, b)
    return a.first <= b.last and b.first <= a.last
  end
  local function chosen(id)
    return choiceByCode[read(assignmentKey(id))]
  end
  local function retirementPending()
    if canEdit() and next(pendingRejoin) then return true end
    for id = 0, 21 do
      if read(retiringKey(id)) ~= nil then return true end
    end
    return false
  end
  local function identityPending(ids)
    for _, id in ipairs(ids) do
      local saved = read(identityKey(id))
      local current = networkIdentity(id)
      if (saved == nil and current ~= nil)
        or (saved ~= nil and current ~= tostring(saved)) then
        return true
      end
    end
    return false
  end
  local function valid(ids)
    if mode() ~= JAPAN then return true, nil end
    if #ids < 2 then return false, "LOC_JAC_MP_NEED_PLAYERS" end
    if read(ROSTER) ~= rosterValue(ids) then
      return false, "LOC_JAC_MP_TEAM_PENDING"
    end
    if read(PRIOR_NO_TEAMS) == nil then
      return false, "LOC_JAC_MP_TEAM_PENDING"
    end
    if read(RESTORE_DONE) ~= nil then
      return false, "LOC_JAC_MP_TEAM_PENDING"
    end
    if retirementPending() then
      return false, "LOC_JAC_MP_TEAM_PENDING"
    end
    if identityPending(ids) then return false, "LOC_JAC_MP_TEAM_PENDING" end
    local set = humanSet(ids)
    if not set[tonumber(read(SUBJECT))] then
      return false, "LOC_JAC_MP_NEED_SUBJECT"
    end
    for i, id in ipairs(ids) do
      local choice = chosen(id)
      if not choice then return false, "LOC_JAC_MP_NEED_AREA" end
      for j = 1, i - 1 do
        if overlaps(choice, chosen(ids[j])) then
          return false, "LOC_JAC_MP_OVERLAP"
        end
      end
    end
    local team = tonumber(read(TEAM))
    if not team or not sameValue(read("NO_TEAMS"), false) then
      return false, "LOC_JAC_MP_TEAM_PENDING"
    end
    for _, id in ipairs(ids) do
      if read(priorTeamKey(id)) == nil then
        return false, "LOC_JAC_MP_TEAM_PENDING"
      end
      if PlayerConfigurations[id]:GetTeam() ~= team then
        return false, "LOC_JAC_MP_TEAM_PENDING"
      end
    end
    return true, nil
  end
  local function assignmentBlocked(ids)
    if read(ROSTER) ~= nil and (not scoped() or mode() ~= JAPAN) then
      return true
    end
    return scoped() and mode() == JAPAN and not valid(ids or humans())
  end
  local function stopInvalidLaunch()
    if assignmentBlocked(humans()) then
      if IsLaunchCountdownActive() then StopCountdown() end
      return true
    end
    return false
  end
  local function teamAvailable(team, humanIds)
    if team == nil or team < 0 then return false end
    local set = humanSet(humanIds)
    for _, id in ipairs(GameConfiguration.GetParticipatingPlayerIDs()) do
      local player = PlayerConfigurations[id]
      if player and not set[id] and player:GetTeam() == team then
        return false
      end
    end
    return true
  end
  local function chooseTeam(ids)
    local saved = tonumber(read(TEAM))
    if teamAvailable(saved, ids) then return saved end
    for _, id in ipairs(ids) do
      local current = PlayerConfigurations[id]:GetTeam()
      if teamAvailable(current, ids) then return current end
    end
    for candidate = 0, 21 do
      if teamAvailable(candidate, ids) then return candidate end
    end
    return nil
  end
  local function applyTeam(id, wanted)
    local player = PlayerConfigurations[id]
    if not player or wanted == nil then return false end
    local current = player:GetTeam()
    if current == wanted then
      teamAttempts[id] = nil
      return true
    end
    local previous = teamAttempts[id]
    if previous and previous.target == wanted
      and previous.observed == current then return false end
    teamAttempts[id] = { target = wanted, observed = current }
    player:SetTeam(wanted)
    if player:GetTeam() == wanted then
      Network.BroadcastPlayerInfo(id)
      return true
    end
    return false
  end
  local function restoreTeam(id)
    local player = PlayerConfigurations[id]
    if not player then return true end
    return applyTeam(id, tonumber(read(priorTeamKey(id))))
  end
  local function retirePlayer(id)
    local key = retiringKey(id)
    local marker = read(key)
    if not sameValue(marker, true) and marker ~= REJOINING then return false end
    local current = rosterIds()
    local kept = {}
    local wasTracked = false
    for _, old in ipairs(current) do
      if old == id then wasTracked = true
      else kept[#kept + 1] = old end
    end
    if wasTracked then
      if marker ~= REJOINING and not restoreTeam(id) then return false end
      write(ROSTER, rosterValue(kept))
      if read(ROSTER) ~= rosterValue(kept) then return false end
    end
    if tonumber(read(SUBJECT)) == id then
      write(SUBJECT, nil)
      if read(SUBJECT) ~= nil then return false end
    end
    write(assignmentKey(id), nil)
    write(priorTeamKey(id), nil)
    write(identityKey(id), nil)
    if read(assignmentKey(id)) ~= nil or read(priorTeamKey(id)) ~= nil
      or read(identityKey(id)) ~= nil then
      return false
    end
    write(key, nil)
    return read(key) == nil
  end
  local function restoreTeams()
    local completed = sameValue(read(RESTORE_DONE), true)
    if not completed and retirementPending() then return false end
    local ids = rosterIds()
    if not completed and identityPending(ids) then return false end
    if not completed then
      for _, id in ipairs(ids) do
        if not restoreTeam(id) then return false end
      end
      local prior = read(PRIOR_NO_TEAMS)
      if prior ~= "TRUE" and prior ~= "FALSE" and prior ~= "NIL" then
        return false
      end
      local wanted = nil
      if prior == "TRUE" then wanted = true
      elseif prior == "FALSE" then wanted = false end
      write("NO_TEAMS", wanted)
      if not sameValue(read("NO_TEAMS"), wanted) then return false end
      write(RESTORE_DONE, true)
      if not sameValue(read(RESTORE_DONE), true) then return false end
    end
    for _, id in ipairs(ids) do
      write(priorTeamKey(id), nil)
      write(assignmentKey(id), nil)
      write(identityKey(id), nil)
      if read(priorTeamKey(id)) ~= nil or read(assignmentKey(id)) ~= nil
        or read(identityKey(id)) ~= nil then
        return false
      end
    end
    write(PRIOR_NO_TEAMS, nil)
    write(TEAM, nil)
    write(SUBJECT, nil)
    if read(PRIOR_NO_TEAMS) ~= nil or read(TEAM) ~= nil
      or read(SUBJECT) ~= nil then return false end
    for id = 0, 21 do
      write(retiringKey(id), nil)
      if read(retiringKey(id)) ~= nil then return false end
    end
    write(ROSTER, nil)
    if read(ROSTER) ~= nil then return false end
    write(RESTORE_DONE, nil)
    return true
  end
  local function finishRetirements(ids)
    local currentSet = humanSet(ids)
    for _, id in ipairs(rosterIds()) do
      if currentSet[id] then
        local saved = read(identityKey(id))
        local current = networkIdentity(id)
        local marker = read(retiringKey(id))
        if marker == VERIFY then
          if current == nil then return false end
          if saved ~= nil and sameValue(saved, current) then
            write(retiringKey(id), nil)
            if read(retiringKey(id)) ~= nil then return false end
          else
            pendingRejoin[id] = true
          end
        elseif marker == nil and saved ~= nil and current == nil then
          write(retiringKey(id), VERIFY)
          if read(retiringKey(id)) ~= VERIFY then return false end
          return false
        elseif current ~= nil then
          if saved == nil then
            write(identityKey(id), current)
            if not sameValue(read(identityKey(id)), current) then return false end
          elseif not sameValue(saved, current) then
            pendingRejoin[id] = true
          end
        end
      end
    end
    for id in pairs(pendingRejoin) do
      write(retiringKey(id), REJOINING)
      if read(retiringKey(id)) ~= REJOINING then
        -- If this key is rejected, remove the slot from the saved roster
        -- before the context can reload and forget the connection event.
        local kept = {}
        for _, old in ipairs(rosterIds()) do
          if old ~= id then kept[#kept + 1] = old end
        end
        write(ROSTER, rosterValue(kept))
        if read(ROSTER) ~= rosterValue(kept) then return false end
      end
      pendingRejoin[id] = nil
    end
    for _, id in ipairs(rosterIds()) do
      if not currentSet[id] then
        write(retiringKey(id), true)
        if not sameValue(read(retiringKey(id)), true) then return false end
      end
    end
    for id = 0, 21 do
      if read(retiringKey(id)) ~= nil and not retirePlayer(id) then return false end
    end
    return true
  end
  local function reconcileJapan(ids)
    if read(RESTORE_DONE) ~= nil then
      if read(ROSTER) ~= nil then restoreTeams()
      else write(RESTORE_DONE, nil) end
      if read(RESTORE_DONE) ~= nil then return end
    end
    if not finishRetirements(ids) then return end
    local currentSet = humanSet(ids)
    local oldSet = humanSet(rosterIds())
    for _, id in ipairs(ids) do
      if not oldSet[id] then
        if tonumber(read(SUBJECT)) == id then
          write(SUBJECT, nil)
          if read(SUBJECT) ~= nil then return end
        end
        write(assignmentKey(id), nil)
        if read(assignmentKey(id)) ~= nil then return end
        write(priorTeamKey(id), PlayerConfigurations[id]:GetTeam())
        if not sameValue(read(priorTeamKey(id)),
          PlayerConfigurations[id]:GetTeam()) then return end
        local identity = networkIdentity(id)
        write(identityKey(id), identity)
        if not sameValue(read(identityKey(id)), identity) then return end
      end
      if read(assignmentKey(id)) and not chosen(id) then
        write(assignmentKey(id), nil)
      end
    end
    if not currentSet[tonumber(read(SUBJECT))] then write(SUBJECT, nil) end
    if read(PRIOR_NO_TEAMS) == nil then
      local current = read("NO_TEAMS")
      write(PRIOR_NO_TEAMS,
        current == nil and "NIL" or sameValue(current, true) and "TRUE" or "FALSE")
    end
    if read(PRIOR_NO_TEAMS) == nil then return end
    for _, id in ipairs(ids) do
      if read(priorTeamKey(id)) == nil then return end
    end
    write(ROSTER, rosterValue(ids))
    if read(ROSTER) ~= rosterValue(ids) then return end
    write("NO_TEAMS", false)
    local team = chooseTeam(ids)
    write(TEAM, team)
    if team and sameValue(read("NO_TEAMS"), false)
      and sameValue(read(TEAM), team) then
      for _, id in ipairs(ids) do
        applyTeam(id, team)
      end
    end
  end

  local function pulldown(control, options, selected, callback)
    local manager = pulldownManagers[control]
    if not manager then
      manager = PullDownInstanceManager:new("InstanceOne", "Button", control)
      pulldownManagers[control] = manager
    end
    manager:ResetInstances()
    for index, option in ipairs(options) do
      local item = manager:GetInstance()
      item.Button:SetText(option.label)
      item.Button:SetVoids(index, 0)
    end
    control:GetButton():SetText(selected)
    control:CalculateInternals()
    control:RegisterSelectionCallback(function(index)
      if options[index] then callback(options[index].value) end
    end)
  end

  local function render(ids)
    local active = scoped()
    Controls.JAC_AssignmentPanel:SetHide(not active)
    if GameConfiguration.IsHotseat() then
      Controls.InfoContainer:SetHide(not active)
    end
    if not active then
      UpdateReadyButton()
      return
    end
    ids = ids or humans()
    local isJapan = mode() == JAPAN
    local edit = canEdit()
    Controls.JAC_AllJapanButton:SetDisabled(not edit or isJapan)
    Controls.JAC_DifferentButton:SetDisabled(not edit or not isJapan)
    Controls.JAC_ModeDescription:SetText(Locale.Lookup(
      isJapan and "LOC_JAC_MP_JAPAN_HELP" or "LOC_JAC_MP_DIFFERENT_HELP"))
    Controls.JAC_JapanOptions:SetHide(not isJapan)
    rows:ResetInstances()
    if isJapan then
      local areaOptions = {{ value = nil, label = Locale.Lookup("LOC_JAC_MP_UNSET") }}
      for _, choice in ipairs(choices) do
        areaOptions[#areaOptions + 1] = {
          value = choice.code, label = Locale.Lookup(choice.tag)
        }
      end
      for _, id in ipairs(ids) do
        local row = rows:GetInstance()
        row.PlayerName:SetText(nameFor(id))
        local choice = chosen(id)
        pulldown(row.AreaPullDown, areaOptions,
          choice and Locale.Lookup(choice.tag) or Locale.Lookup("LOC_JAC_MP_UNSET"),
          function(code) JAC_SetPlayerArea(id, code) end)
        row.AreaPullDown:SetDisabled(not edit)
      end
      local subjectOptions = {{ value = nil, label = Locale.Lookup("LOC_JAC_MP_UNSET") }}
      local subject = tonumber(read(SUBJECT))
      for _, id in ipairs(ids) do
        subjectOptions[#subjectOptions + 1] = { value = id, label = nameFor(id) }
      end
      pulldown(Controls.JAC_SubjectPullDown, subjectOptions,
        humanSet(ids)[subject] and nameFor(subject) or Locale.Lookup("LOC_JAC_MP_UNSET"),
        function(id) JAC_SetSubject(id) end)
      Controls.JAC_SubjectPullDown:SetDisabled(not edit)
      Controls.JAC_SubjectIs:SetText(
        Locale.Lookup("LOC_JAC_MP_SUBJECT_IS") .. " " ..
        (humanSet(ids)[subject] and nameFor(subject) or Locale.Lookup("LOC_JAC_MP_UNSET")))
      local ok, reason = valid(ids)
      Controls.JAC_AssignmentStatus:SetText(Locale.Lookup(
        lastError or (ok and "LOC_JAC_MP_READY" or reason)))
    else
      Controls.JAC_AssignmentStatus:SetText("")
    end
    Controls.JAC_AssignmentRows:CalculateSize()
    Controls.JAC_AssignmentContent:CalculateSize()
    Controls.GameSummaryStack:CalculateSize()
    Controls.ParametersScrollPanel:CalculateSize()
    UpdateReadyButton()
  end

  local function syncState()
    if syncing or ContextPtr:IsHidden() then return end
    syncing = true
    lastError = nil
    local active = scoped()
    local ids = active and humans() or nil
    if canEdit() then
      if active and mode() == JAPAN then
        reconcileJapan(ids)
      elseif read(ROSTER) ~= nil then
        if (sameValue(read(RESTORE_DONE), true)
          or finishRetirements(humans())) and restoreTeams() and not active then
          write(MODE, nil)
        end
      end
    end
    render(ids)
    local changed = configDirty
    configDirty = false
    if changed then Network.BroadcastGameConfig() end
    if canEdit() and assignmentBlocked(ids) then
      if IsLaunchCountdownActive() then StopCountdown() end
      local localPlayer = PlayerConfigurations[Network.GetLocalPlayerID()]
      if localPlayer and localPlayer:GetReady() then
        SetLocalReady(false)
      end
    end
    syncing = false
  end
  local function onMode(nextMode)
    if not scoped() or not canEdit() or mode() == nextMode then return end
    if nextMode == DIFFERENT then
      syncState()
      local ids = humans()
      if retirementPending() or identityPending(ids)
        or read(ROSTER) ~= rosterValue(ids) then return end
    end
    lastError = nil
    resetAttempts()
    write(MODE, nextMode)
    syncState()
  end
  function JAC_SetPlayerArea(id, code)
    if not scoped() or not canEdit() or mode() ~= JAPAN then return end
    local ids = humans()
    if not humanSet(ids)[id] or (code and not choiceByCode[code]) then return end
    local nextChoice = code and choiceByCode[code]
    if nextChoice then
      for _, other in ipairs(ids) do
        if other ~= id then
          local otherChoice = chosen(other)
          if otherChoice and overlaps(nextChoice, otherChoice) then
            lastError = "LOC_JAC_MP_OVERLAP"
            render(ids)
            return
          end
        end
      end
    end
    lastError = nil
    resetAttempts()
    write(assignmentKey(id), code)
    syncState()
  end
  function JAC_SetSubject(id)
    if not scoped() or not canEdit() or mode() ~= JAPAN then return end
    if id and not humanSet(humans())[id] then return end
    lastError = nil
    resetAttempts()
    write(SUBJECT, id)
    syncState()
  end
  local function onPreDisconnect(id)
    if not ContextPtr:IsHidden() and scoped() and canEdit()
      and mode() == JAPAN then
      lastError = nil
      departing[id] = true
      resetAttempts()
      local wasSyncing = syncing
      syncing = true
      write(retiringKey(id), true)
      if sameValue(read(retiringKey(id)), true) then retirePlayer(id) end
      if tonumber(read(SUBJECT)) == id then write(SUBJECT, nil) end
      local changed = configDirty
      configDirty = false
      if changed then Network.BroadcastGameConfig() end
      if IsLaunchCountdownActive() then StopCountdown() end
      syncing = wasSyncing
      if not syncing then render(humans()) end
    end
  end
  local function onConnected(id)
    departing[id] = nil
    resetAttempts()
    if canEdit() and mode() == JAPAN then
      for _, old in ipairs(rosterIds()) do
        if old == id and read(retiringKey(id)) == nil then
          local saved = read(identityKey(id))
          local current = networkIdentity(id)
          if saved ~= nil and current == nil then
            write(retiringKey(id), VERIFY)
          elseif saved == nil or not sameValue(saved, current) then
            pendingRejoin[id] = true
          end
        end
      end
    end
    syncState()
  end

  -- Stock handlers were registered by include(). Replace the ready-button
  -- callbacks as well as the global used by player-card clicks.
  function OnReadyButton()
    if stopInvalidLaunch() then
      render()
      return
    end
    stockOnReadyButton()
  end
  function SetLocalReady(ready)
    if ready and stopInvalidLaunch() then
      render()
      return
    end
    stockSetLocalReady(ready)
  end
  function CheckGameAutoStart()
    if stopInvalidLaunch() then return end
    stockCheckGameAutoStart()
  end
  function UpdateReadyButton()
    stockUpdateReadyButton()
    if assignmentBlocked(humans()) then
      Controls.ReadyButton:SetDisabled(true)
      Controls.ReadyCheck:SetDisabled(true)
      guardedReadyCheck = true
    elseif guardedReadyCheck then
      -- Stock hotseat updates only ReadyButton; restore our own ReadyCheck lock.
      if GameConfiguration.IsHotseat() then
        Controls.ReadyCheck:SetDisabled(false)
      end
      guardedReadyCheck = false
    end
  end
  Controls.ReadyButton:RegisterCallback(Mouse.eLClick, OnReadyButton)
  Controls.ReadyCheck:RegisterCallback(Mouse.eLClick, OnReadyButton)
  local nativeWitnessInstalled=false
  local nativeWitnessDiagnostic
  local function installNativeWitness()
    local nativeWitnessNonce=GameConfiguration.GetValue('JAC_NATIVE_CARRIER_WITNESS_NONCE')
    -- Saved metadata can replace settings before the lobby is shown. Record
    -- the actual values even when a missing nonce prevents witness inclusion.
    -- Keep the exact scope checks and avoid repeating unchanged observations.
    if scoped() and GameConfiguration.IsHotseat() then
      local generation=GameConfiguration.GetValue('JAC_NATIVE_CARRIER_LOAD_GENERATION')
      local ruleset=GameConfiguration.GetValue('RULESET')
      local map=MapConfiguration.GetValue('MAP_SCRIPT')
      local multiplayer
      if type(GameConfiguration.IsAnyMultiplayer)=='function' then
        local ok,value=pcall(GameConfiguration.IsAnyMultiplayer)
        if ok then multiplayer=value end
      end
      local observed='Nonce='..tostring(nativeWitnessNonce)..' Generation='..tostring(generation)
        ..' Ruleset='..tostring(ruleset)..' Map='..tostring(map)
        ..' Hotseat=true AnyMultiplayer='..tostring(multiplayer)
      if observed~=nativeWitnessDiagnostic then
        nativeWitnessDiagnostic=observed
        print('[JAC_NATIVE_CARRIER_STAGING_SETTINGS] '..observed)
      end
    end
    if not nativeWitnessInstalled and scoped() and GameConfiguration.IsHotseat()
      and type(nativeWitnessNonce)=='string' and #nativeWitnessNonce==32 and nativeWitnessNonce:match('^%x+$') then
      nativeWitnessInstalled=true -- one fixed constructor per Lua context
-- BEGIN GENERATED NATIVE CARRIER STAGING
-- Generated by native_carrier_witness.rs; existing Japanese staging context.
local stockUpdate=UpdateReadyButton
if type(stockUpdate)~='function' then return end
local installed
local function initialize()
-- No test mode or product guard is weakened. Invalid/unset nonce is a no-op.
local nonce=GameConfiguration and GameConfiguration.GetValue and GameConfiguration.GetValue('JAC_NATIVE_CARRIER_WITNESS_NONCE')
if type(nonce)~='string' or #nonce~=32 or not nonce:match('^%x+$') then return end
if not GameConfiguration.IsHotseat() or not GameConfiguration.IsAnyMultiplayer() then return end
local bridgeKey='JAC_NATIVE_CARRIER_WITNESS_V1'
local fixtureKey='JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'
local function jsonString(s)
 assert(type(s)=='string')
 return '"'..s:gsub('[%z\1-\31\\"]',function(c)
  if c=='\\' then return '\\\\' elseif c=='"' then return '\\"' end
  return string.format('\\u%04x',string.byte(c))
 end)..'"'
end
local serial=0
local scene
local function initializeScene(generation)
 -- Freeze the actual construction generation for this entire UI scene,
 -- including SAVE_RETURNED carrying its next header generation. Both
 -- reloads differ even if the engine reuses a ContextPtr address.
 scene=string.gsub(tostring(ContextPtr),'[^%w]','')..'.g'..generation
 assert(#scene<=128 and scene:match('^[%w%.]+$'),'invalid native scene')
end
local function emit(kind,generation,step,canonical,extra)
 assert(type(canonical)=='string' and #canonical>0,'complete nonempty canonical record required')
 assert(generation>=0 and generation<=2 and generation==math.floor(generation),'invalid save generation')
 assert(step>=0 and step<=43 and step==math.floor(step),'invalid fixed stage')
 if not scene then
  initializeScene(generation)
 end
 local body='{"generation":'..generation..',"canonical":'..jsonString(canonical)
 for _,key in ipairs({'name','code','error'}) do if extra and extra[key]~=nil then
  body=body..','..jsonString(key)..':'..jsonString(extra[key])
 end end
 body=body..'}'
 assert(#body<=1048576,'native complete record exceeds one MiB; never truncate evidence')
 serial=serial+1
 local parts=math.max(1,math.ceil(#body/96))
 assert(parts<=10923)
 for part=1,parts do
  local bytes=body:sub((part-1)*96+1,part*96);local hex={}
  for i=1,#bytes do hex[i]=string.format('%02x',bytes:byte(i)) end
  print('JAC_NC|1|'..nonce..'|'..scene..'|'..serial..'|'..generation..'|'..step..'|'..kind..'|'..part..'/'..parts..'|'..table.concat(hex))
 end
end

-- SSOT 9.1. These are PoC policies, not balance decisions or the old nine states.
JAC_AbsorptionRules = {}
local R = JAC_AbsorptionRules
R.STATE_KEY = "JAC_ABSORPTION_STATE_V1"
R.ROSTER_KEY = "JAC_ABSORPTION_ASSETS_V1"
R.PERSON_KEY = "JAC_ABSORPTION_PERSON_V1"
R.CITY_FAITH_KEY = "JAC_ABSORPTION_CITY_FAITH_V1"
R.GOVERNANCE_KEY = "JAC_ABSORPTION_GOVERNANCE_V1"
R.TOKEN_KEY = "JAC_ABSORPTION_UNIT_TOKEN_V1"
R.REQUIRE_EXPLICIT_DISPOSITIONS_POC = true
-- Provisional magnitudes. The official Expansion1_GlobalParameters.xml uses
-- 50 after combat transfer and 100 after cultural-identity transfer; espionage
-- foment unrest is -15 (-5 per promotion level). Friction is a separate,
-- once-per-turn loyalty change, never a culture/science yield from a person.
R.CITY_POLICY = {
  CONQUEST = { Immediate = -15, Friction = -2, Turns = 3 },
  SMALL_STATE = { Immediate = -5, Friction = -1, Turns = 2 },
  MERGER = { Immediate = 0, Friction = 0, Turns = 0 },
}
R.UNIT_SEARCH_RADIUS_POC = 8
R.LEADER_UNIT_TYPE_POC = "UNIT_GREAT_GENERAL"
R.LEADER_REPRESENTATION_POC = "PERSISTENT_PERSON_WITH_GENERAL_PROXY"
R.NATIVE_HERO_POLICY_POC = "TRANSFER_OR_ARCHIVE_SUICIDE_WITH_CAUSE"
R.ENTRY_MODES = { CONQUEST = true, SMALL_STATE = true, MERGER = true }

-- SSOT 9.1 provisional flow policy; all rates are basis points (0..10000).
R.FLOW_FIRST_ESCAPE = { CONQUEST = 2400, SMALL_STATE = 1400, MERGER = 700 }
R.FLOW_METHOD_ACTION = { CONQUEST = -700, SMALL_STATE = 0, MERGER = 600 }
R.FLOW_ACTION_SUCCESS = {
  CAPTURE = 8800, RECRUIT = 6800, SUBMIT = 7800, EXECUTE = 9500,
  PROTECT = 8200, RECONTRACT = 7000, SUICIDE = 9500,
  TRANSFER = 9400, LOOT = 8800, SEIZE = 8600, RETURN = 9000,
  SURRENDER = 9100, INCORPORATE = 8500, DISSOLVE = 9500,
  INHERIT = 10000, PRESERVE = 10000,
}
R.FLOW_RATE = {
  RollRange = 10000, AttitudeMinimum = 0, AttitudeMaximum = 100,
  NeutralAttitude = 50, LoyaltyEscapeWeight = 22, RelationEscapeWeight = 12,
  ProtectedEscapeDelta = -500, BrokenEscapeDelta = 500,
  ItemEscapeNumerator = 3, ItemEscapeDenominator = 4,
  RetryEscapeDelta = 500, EscapeMaximum = 9500,
  LoyaltyActionWeight = 15, RelationActionWeight = 10,
  ActionMinimum = 500, ActionMaximum = 9900,
  WorldDelayTurns = 2,
}

function R.UnitKind(unit)
  local person = unit:GetProperty(R.PERSON_KEY)
  if unit.IsHero and unit:IsHero() then return "HERO" end
  local info = assert(GameInfo.Units[unit:GetType()], "unknown unit definition")
  local gp = unit:GetGreatPerson()
  local individual = gp and gp:GetIndividual() or -1
  if person and person.Kind == "GREAT_PERSON" then
    assert((individual >= 0 and GameInfo.GreatPersonIndividuals[individual]
      and GameInfo.GreatPersonIndividuals[individual].GreatPersonClassType)
      or (person.Persistent and person.Representation == R.LEADER_REPRESENTATION_POC),
      "Great Person record has no native individual or explicit leader provenance")
    return "GREAT_PERSON"
  end
  if person and person.Kind and person.Kind ~= "UNIT" then return person.Kind end
  if info.UnitType == "UNIT_GREAT_GENERAL" or unit:GetProperty("JAC_POC_GROWTHUI_WARLORD") then
    return "PERSON"
  end
  if person and person.Kind then return person.Kind end
  if individual >= 0 then
    assert(GameInfo.GreatPersonIndividuals[individual]
      and GameInfo.GreatPersonIndividuals[individual].GreatPersonClassType,
      "native Great Person individual definition missing")
    return "GREAT_PERSON"
  end
  return "UNIT"
end

-- Database rows supply the dispositions, so a missing/invalid decision fails before mutation.
function R.Destination(kind, outcome)
  assert(type(outcome) == "string", "explicit disposition required for " .. kind)
  for row in GameInfo.JAC_AbsorptionOutcomes() do
    if row.Kind == kind and row.Outcome == outcome then return row.Destination end
  end
  error("invalid disposition " .. kind .. "/" .. outcome)
end

function R.Keys(t)
  local keys = {}
  for k in pairs(t) do
    assert(type(k) == "string" or type(k) == "number", "unsupported property key")
    keys[#keys + 1] = k
  end
  table.sort(keys, function(a, b)
    if type(a) ~= type(b) then return type(a) < type(b) end
    return a < b
  end)
  return keys
end

function R.Copy(value, seen)
  local t = type(value)
  if t == "nil" or t == "boolean" then return value end
  if t == "number" then
    assert(value == value and value ~= math.huge and value ~= -math.huge, "non-finite property")
    return value
  end
  if t == "string" then
    assert(value ~= "", "empty strings do not survive Civ6 saves; use nil")
    return value
  end
  assert(t == "table", "property must be serializable: " .. t)
  seen = seen or {}
  assert(not seen[value], "cyclic property")
  seen[value] = true
  local result = {}
  for _, k in ipairs(R.Keys(value)) do result[k] = R.Copy(value[k], seen) end
  seen[value] = nil
  return result
end

function R.Canonical(value)
  local t = type(value)
  if t ~= "table" then return t .. ":" .. (t == "string" and string.format("%q", value) or tostring(value)) end
  local out = { "{" }
  for _, k in ipairs(R.Keys(value)) do
    out[#out + 1] = R.Canonical(k) .. "=" .. R.Canonical(value[k]) .. ";"
  end
  out[#out + 1] = "}"
  return table.concat(out)
end

function R.Equal(a, b) return R.Canonical(a) == R.Canonical(b) end
function R.UnitKey(owner, id) return tostring(owner) .. ":" .. tostring(id) end
function R.KeepUnit(kind, outcome)
  return (kind == "UNIT" and (outcome == "SURRENDER" or outcome == "INCORPORATE"))
    or ((kind == "PERSON" or kind == "GOVERNOR")
      and (outcome == "CAPTURE" or outcome == "RECRUIT" or outcome == "SUBMIT"))
    or (kind == "HERO" and (outcome == "RECRUIT" or outcome == "PROTECT" or outcome == "RECONTRACT"))
    or (kind == "GREAT_PERSON" and outcome == "RECRUIT")
end

-- Only explicit component references are rewritten; Founder, OriginalOwner, historical IDs stay intact.
function R.Rebind(record, tx)
  local result = R.Copy(record)
  if result.Unit and result.Unit.Owner == tx.From then
    local old = result.Unit
    local new = tx.UnitMap[R.UnitKey(old.Owner, old.ID)]
    result.FormerUnit = R.Copy(old)
    result.Unit = new and { Owner = tx.To, ID = new.ID, Token = new.Token } or nil
  end
  if result.City and result.City.Owner == tx.From then result.City.Owner = tx.To end
  return result
end

local requiredRuleset="RULESET_JAC_BANSEIKI"
local requiredMap="{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map"
local UpdateReadyButton=stockUpdate
local canonical=assert(JAC_AbsorptionRules and JAC_AbsorptionRules.Canonical,'staging witness requires canonical codec')
local bindingGeneration=GameConfiguration.GetValue('JAC_NATIVE_CARRIER_LOAD_GENERATION')
print('[JAC_NATIVE_CARRIER_STAGING_BIND] Nonce='..nonce..' Generation='..tostring(bindingGeneration))
local attempted={}
local failed=false
local observedGeneration=0
local function ready()
 if failed then return end
 if not GameConfiguration.IsHotseat() or not GameConfiguration.IsAnyMultiplayer()
  or GameConfiguration.GetValue('JAC_NATIVE_CARRIER_WITNESS_NONCE')~=nonce then return end
 if GameConfiguration.GetValue('RULESET')~=requiredRuleset or not MapConfiguration
  or type(MapConfiguration.GetValue)~='function' or MapConfiguration.GetValue('MAP_SCRIPT')~=requiredMap then return end
 local savedGeneration=GameConfiguration.GetValue('JAC_NATIVE_CARRIER_LOAD_GENERATION')
 local generation=savedGeneration~=nil and assert(tonumber(savedGeneration)) or 0
 assert(generation==0 or generation==1 or generation==2)
 observedGeneration=generation
 if attempted[generation] or ContextPtr:IsHidden() or not Network.IsGameHost() or Controls.ReadyButton:IsDisabled() then return end
 local humans=0
 for _,id in ipairs(GameConfiguration.GetParticipatingPlayerIDs()) do
  local p=PlayerConfigurations[id]
  if p:GetSlotStatus()==SlotStatus.SS_TAKEN then humans=humans+1 end
 end
 assert(humans==2,'native staging must retain two actual human slots')
 emit('STAGING_READY',generation,0,canonical({Nonce=nonce,Humans=humans,Hotseat=true,Host=true,ReadyEnabled=true}))
 attempted[generation]=true -- durable in this context before the official handler
 Controls.ReadyButton:DoLeftClick()
end
-- The existing stock update callback is preserved; no Ready flags are forged.
local update=UpdateReadyButton
UpdateReadyButton=function(...) update(...);local ok,err=pcall(ready)
 if not ok then
  failed=true
  -- Do not call the failing API again while recording its original error.
  emit('FAIL',observedGeneration,0,canonical({Error=tostring(err)}),{code='STAGING_READY',error=tostring(err)})
 end
end

installed=UpdateReadyButton
end
UpdateReadyButton=function(...)
 if not installed then initialize() end
 if installed then return installed(...) end
 return stockUpdate(...)
end
-- END GENERATED NATIVE CARRIER STAGING
    end
  end
  installNativeWitness()
  Controls.JAC_AllJapanButton:RegisterCallback(Mouse.eLClick, function() onMode(JAPAN) end)
  Controls.JAC_DifferentButton:RegisterCallback(Mouse.eLClick, function() onMode(DIFFERENT) end)

  Controls.JAC_AssignmentPanel:ChangeParent(Controls.GameSummaryStack)
  local order = {}
  local panelKey = tostring(Controls.JAC_AssignmentPanel)
  for index, child in ipairs(Controls.GameSummaryStack:GetChildren()) do
    local key = tostring(child)
    order[key] = key == panelKey and 0 or index
  end
  Controls.GameSummaryStack:SortChildren(function(a, b)
    return (order[tostring(a)] or math.huge) < (order[tostring(b)] or math.huge)
  end)
  ContextPtr:SetShowHandler(function()
    installNativeWitness()
    stockOnShow()
    resetAttempts()
    syncState()
  end)
  Events.GameConfigChanged.Add(function()
    installNativeWitness()
    syncState()
  end)
  Events.PlayerInfoChanged.Add(syncState)
  Events.MultiplayerPlayerConnected.Add(onConnected)
  Events.MultiplayerPrePlayerDisconnected.Add(onPreDisconnect)
  Events.MultiplayerHostMigrated.Add(function()
    pendingRejoin = {}
    resetAttempts()
    syncState()
  end)
  Events.SteamFriendsStatusUpdated.Add(syncState)
  Events.SteamFriendsPresenceUpdated.Add(syncState)
end
