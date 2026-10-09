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
          tostring(GameConfiguration.GetValue('RULESET')) .. '|' ..
          tostring(MapConfiguration.GetValue('MAP_SCRIPT')) .. '|' ..
          tostring(MapConfiguration.GetMaxMajorPlayers()) .. '|' .. tostring(limit))
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
  local configDirty = false
  local lastError = nil
  local stockOnShow = OnShow
  local stockOnReadyButton = OnReadyButton
  local stockCheckGameAutoStart = CheckGameAutoStart
  local stockSetLocalReady = SetLocalReady

  local function read(key) return GameConfiguration.GetValue(key) end
  local function write(key, value)
    if read(key) ~= value then
      GameConfiguration.SetValue(key, value)
      configDirty = true
    end
  end
  local function mode()
    return read(MODE) == JAPAN and JAPAN or DIFFERENT
  end
  local function assignmentKey(id) return "JAC_MP_AREA_" .. tostring(id) end
  local function priorTeamKey(id) return "JAC_MP_PRIOR_TEAM_" .. tostring(id) end
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
  local function valid(ids)
    if mode() ~= JAPAN then return true, nil end
    if #ids < 2 then return false, "LOC_JAC_MP_NEED_PLAYERS" end
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
    if not team or read("NO_TEAMS") == true or read("NO_TEAMS") == 1 then
      return false, "LOC_JAC_MP_TEAM_PENDING"
    end
    for _, id in ipairs(ids) do
      if PlayerConfigurations[id]:GetTeam() ~= team then
        return false, "LOC_JAC_MP_TEAM_PENDING"
      end
    end
    return true, nil
  end
  local function stopInvalidLaunch()
    if scoped() and mode() == JAPAN and not valid(humans()) then
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
  local function restoreTeams()
    for _, id in ipairs(rosterIds()) do
      local old = tonumber(read(priorTeamKey(id)))
      local player = PlayerConfigurations[id]
      if player and old and player:GetTeam() ~= old then
        player:SetTeam(old)
        Network.BroadcastPlayerInfo(id)
      end
      write(priorTeamKey(id), nil)
      write(assignmentKey(id), nil)
    end
    local prior = read(PRIOR_NO_TEAMS)
    if prior == "TRUE" then write("NO_TEAMS", true)
    elseif prior == "FALSE" then write("NO_TEAMS", false)
    elseif prior == "NIL" then write("NO_TEAMS", nil)
    end
    write(PRIOR_NO_TEAMS, nil)
    write(ROSTER, nil)
    write(TEAM, nil)
    write(SUBJECT, nil)
  end
  local function reconcileJapan(ids)
    local prior = rosterIds()
    local oldSet = humanSet(prior)
    local currentSet = humanSet(ids)
    for _, id in ipairs(prior) do
      if not currentSet[id] then
        local player = PlayerConfigurations[id]
        local oldTeam = tonumber(read(priorTeamKey(id)))
        if player and oldTeam and player:GetTeam() ~= oldTeam then
          player:SetTeam(oldTeam)
          Network.BroadcastPlayerInfo(id)
        end
        write(assignmentKey(id), nil)
        write(priorTeamKey(id), nil)
      end
    end
    for _, id in ipairs(ids) do
      if not oldSet[id] then
        write(assignmentKey(id), nil)
        write(priorTeamKey(id), PlayerConfigurations[id]:GetTeam())
      end
      if read(assignmentKey(id)) and not chosen(id) then
        write(assignmentKey(id), nil)
      end
    end
    write(ROSTER, rosterValue(ids))
    if not currentSet[tonumber(read(SUBJECT))] then write(SUBJECT, nil) end
    if read(PRIOR_NO_TEAMS) == nil then
      local current = read("NO_TEAMS")
      write(PRIOR_NO_TEAMS,
        current == nil and "NIL" or (current == true or current == 1) and "TRUE" or "FALSE")
    end
    write("NO_TEAMS", false)
    local team = chooseTeam(ids)
    write(TEAM, team)
    if team then
      for _, id in ipairs(ids) do
        local player = PlayerConfigurations[id]
        if player:GetTeam() ~= team then
          player:SetTeam(team)
          Network.BroadcastPlayerInfo(id)
        end
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
    if not active then return end
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
  end

  local function syncState()
    if syncing then return end
    syncing = true
    local active = scoped()
    local ids = active and humans() or nil
    if canEdit() then
      if active and mode() == JAPAN then
        reconcileJapan(ids)
      elseif read(ROSTER) ~= nil then
        restoreTeams()
        if not active then write(MODE, nil) end
      end
    end
    render(ids)
    local changed = configDirty
    configDirty = false
    if changed then Network.BroadcastGameConfig() end
    if active and mode() == JAPAN and canEdit() then
      local ok = valid(ids)
      if not ok and IsLaunchCountdownActive() then StopCountdown() end
      local localPlayer = PlayerConfigurations[Network.GetLocalPlayerID()]
      if (changed or not ok) and localPlayer and localPlayer:GetReady() then
        SetLocalReady(false)
      end
    end
    syncing = false
  end
  local function onMode(nextMode)
    if not scoped() or not canEdit() or mode() == nextMode then return end
    lastError = nil
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
    write(assignmentKey(id), code)
    syncState()
  end
  function JAC_SetSubject(id)
    if not scoped() or not canEdit() or mode() ~= JAPAN then return end
    if id and not humanSet(humans())[id] then return end
    lastError = nil
    write(SUBJECT, id)
    syncState()
  end
  local function onPreDisconnect(id)
    if scoped() and canEdit() and mode() == JAPAN then
      departing[id] = true
      local wasSyncing = syncing
      syncing = true
      local player = PlayerConfigurations[id]
      local oldTeam = tonumber(read(priorTeamKey(id)))
      if player and oldTeam and player:GetTeam() ~= oldTeam then
        player:SetTeam(oldTeam)
        Network.BroadcastPlayerInfo(id)
      end
      write(assignmentKey(id), nil)
      if tonumber(read(SUBJECT)) == id then write(SUBJECT, nil) end
      local current = {}
      for _, old in ipairs(rosterIds()) do
        if old ~= id then current[#current + 1] = old end
      end
      write(ROSTER, rosterValue(current))
      write(priorTeamKey(id), nil)
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
  Controls.ReadyButton:RegisterCallback(Mouse.eLClick, OnReadyButton)
  Controls.ReadyCheck:RegisterCallback(Mouse.eLClick, OnReadyButton)
  Controls.JAC_AllJapanButton:RegisterCallback(Mouse.eLClick, function() onMode(JAPAN) end)
  Controls.JAC_DifferentButton:RegisterCallback(Mouse.eLClick, function() onMode(DIFFERENT) end)

  Controls.JAC_AssignmentPanel:ChangeParent(Controls.GameSummaryStack)
  local order = {}
  for index, child in ipairs(Controls.GameSummaryStack:GetChildren()) do
    order[child] = child == Controls.JAC_AssignmentPanel and 0 or index
  end
  Controls.GameSummaryStack:SortChildren(function(a, b) return order[a] < order[b] end)
  ContextPtr:SetShowHandler(function()
    stockOnShow()
    syncState()
  end)
  Events.GameConfigChanged.Add(syncState)
  Events.PlayerInfoChanged.Add(syncState)
  Events.MultiplayerPlayerConnected.Add(onConnected)
  Events.MultiplayerPrePlayerDisconnected.Add(onPreDisconnect)
  Events.MultiplayerHostMigrated.Add(syncState)
  Events.SteamFriendsStatusUpdated.Add(syncState)
  Events.SteamFriendsPresenceUpdated.Add(syncState)
end
