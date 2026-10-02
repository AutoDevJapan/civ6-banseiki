use serde_json::Value;

pub fn quote(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization")
}

pub fn enable_mods(ids: &[String], disable: &[String]) -> String {
    format!(
        "if not UI.IsInFrontEnd() then error('main menu required') end\nlocal enableIDs = {{{}}}\nlocal disableIDs = {{{}}}\n{}",
        ids.iter().map(|s| quote(s)).collect::<Vec<_>>().join(", "),
        disable
            .iter()
            .map(|s| quote(s))
            .collect::<Vec<_>>()
            .join(", "),
        ENABLE_MODS
    )
}

// The Lua command body is unchanged from the existing verified driver.
const ENABLE_MODS: &str = r###"
local keep, disabled = {}, {}
for _, id in ipairs(enableIDs) do keep[string.lower(id)] = true end
local handles, dependents, original = {}, {}, {}
for _, installed in ipairs(Modding.GetInstalledMods()) do
    local info = Modding.GetModInfo(installed.Handle)
    local id = string.lower(info.Id)
    handles[id] = installed.Handle
    original[id] = info.Enabled
    local dependencies = Modding.GetModAssociations(installed.Handle)
    for _, dependency in ipairs(dependencies or {}) do
      local target = string.lower(dependency.Id)
      dependents[target] = dependents[target] or {}
      table.insert(dependents[target], id)
    end
end
local order = {}
if #disableIDs > 0 then
  local visiting, visited = {}, {}
  local function visit(id)
    if visiting[id] then error('mod dependency cycle: ' .. id) end
    if visited[id] then return end
    if keep[id] then error('scenario enables a dependent mod of a disable target: ' .. id) end
    visiting[id] = true
    local children = dependents[id] or {}
    table.sort(children)
    for _, child in ipairs(children) do visit(child) end
    local handle = handles[id]
    if handle and Modding.GetModInfo(handle).Enabled then table.insert(order, id) end
    visiting[id], visited[id] = nil, true
  end
  -- Build the complete order before changing anything, including paths through
  -- disabled intermediates. An enable-list conflict must not disable other mods.
  for _, id in ipairs(disableIDs) do visit(string.lower(id)) end
  local positions = {}
  for i, id in ipairs(order) do positions[id] = i end
  for i, id in ipairs(order) do
    local reason, _, sources = Modding.CanDisableMod(handles[id])
    if reason == 'MissingDependencies' and sources and #sources > 0 then
      -- The engine still sees dependents as enabled during preflight. Only
      -- dependencies already scheduled BEFORE this target can be resolved.
      for _, source in ipairs(sources) do
        local position = positions[string.lower(source.Id)]
        if not position or position >= i then
          error('cannot disable mod: ' .. id .. ' (unscheduled dependent ' .. source.Id .. ')')
        end
      end
    elseif reason ~= 'OK' then
      error('cannot disable mod: ' .. id .. ' (' .. tostring(reason) .. ')')
    end
  end
end
for _, id in ipairs(enableIDs) do
  if not Modding.GetModHandle(id) then error('mod is not installed: ' .. id) end
end
local ok, failure = pcall(function()
    for _, id in ipairs(order) do
      local handle = handles[id]
      local reason = Modding.CanDisableMod(handle)
      if reason ~= 'OK' then error('cannot disable mod: ' .. id .. ' (' .. tostring(reason) .. ')') end
      Modding.DisableMod(handle)
      if Modding.GetModInfo(handle).Enabled then error('mod stayed enabled: ' .. id) end
      table.insert(disabled, id)
    end
for _, id in ipairs(enableIDs) do
  local handle = Modding.GetModHandle(id)
  if not handle then error('mod is not installed: ' .. id) end
  if not Modding.GetModInfo(handle).Enabled then
    local reason = Modding.CanEnableMod(handle, true)
    if reason ~= 'OK' then error('cannot enable mod: ' .. id .. ' (' .. tostring(reason) .. ')') end
    Modding.EnableMod(handle, true)
  end
end
for _, id in ipairs(enableIDs) do
  if not Modding.GetModInfo(Modding.GetModHandle(id)).Enabled then error('mod did not become enabled: ' .. id) end
end
for _, id in ipairs(disableIDs) do
  local handle = Modding.GetModHandle(id)
  if handle and Modding.GetModInfo(handle).Enabled then error('mod stayed enabled: ' .. id) end
end
GameConfiguration.UpdateEnabledMods()
end)
if not ok then
  -- Restore the complete original selection, including prerequisites enabled
  -- implicitly by EnableMod(..., true), on failure in EITHER direction.
  local all, visited, restoreOrder = {}, {}, {}
  for id in pairs(handles) do table.insert(all, id) end
  table.sort(all)
  local function visit(id)
    if visited[id] then return end
    visited[id] = true
    local children = dependents[id] or {}
    table.sort(children)
    for _, child in ipairs(children) do visit(child) end
    if handles[id] then table.insert(restoreOrder, id) end
  end
  for _, id in ipairs(all) do visit(id) end
  local turnOff, changed = {}, false
  local function mark(id)
    if turnOff[id] then return end
    turnOff[id] = true
    for _, child in ipairs(dependents[id] or {}) do mark(child) end
  end
  for _, id in ipairs(all) do
    if Modding.GetModInfo(handles[id]).Enabled ~= original[id] then changed = true end
    if not original[id] and Modding.GetModInfo(handles[id]).Enabled then mark(id) end
  end
  local failures = {}
  for _, id in ipairs(restoreOrder) do
    if turnOff[id] and Modding.GetModInfo(handles[id]).Enabled then
      local restored, err = pcall(Modding.DisableMod, handles[id])
      if not restored or Modding.GetModInfo(handles[id]).Enabled then
        table.insert(failures, id .. ': ' .. tostring(err))
      end
    end
  end
  for i = #restoreOrder, 1, -1 do
    local id = restoreOrder[i]
    if original[id] and not Modding.GetModInfo(handles[id]).Enabled then
      -- False preserves an initially disabled intermediate prerequisite.
      local restored, err = pcall(Modding.EnableMod, handles[id], false)
      if not restored or not Modding.GetModInfo(handles[id]).Enabled then
        table.insert(failures, id .. ': ' .. tostring(err))
      end
    end
  end
  for _, id in ipairs(all) do
    if Modding.GetModInfo(handles[id]).Enabled ~= original[id] then
      table.insert(failures, id .. ': selection differs from original')
    end
  end
  if changed then
    local updated, err = pcall(GameConfiguration.UpdateEnabledMods)
    if not updated then table.insert(failures, 'configuration: ' .. tostring(err)) end
  end
  if #failures > 0 then error(tostring(failure) .. '; restoration failed: ' .. table.concat(failures, ', ')) end
  error(failure)
end
return table.concat(disabled, ', ')
"###;

pub fn configure(step: &Value) -> String {
    let players = step["players"].as_array().expect("validated players");
    let mut lines = vec![
        "if not UI.IsInFrontEnd() then error('main menu required') end".to_string(),
        "GameConfiguration.SetToDefaults()".to_string(),
        format!(
            "GameConfiguration.SetRuleSet({})",
            quote(step["ruleset"].as_str().unwrap())
        ),
        "GameConfiguration.UpdateEnabledMods()".to_string(),
    ];
    if let Some(map) = step["map_file"].as_str() {
        lines.push(format!(
            "MapConfiguration.SetValue('MAP_SCRIPT', {})",
            quote(map)
        ));
    }
    lines.extend([format!("MapConfiguration.SetMapSize({})",quote(step["map_size"].as_str().unwrap())),format!("GameConfiguration.SetGameSpeedType({})",quote(step["speed"].as_str().unwrap())),format!("MapConfiguration.SetValue('RANDOM_SEED', {})",step["seed"]),format!("GameConfiguration.SetValue('GAME_SYNC_RANDOM_SEED', {})",step["seed"]),format!("MapConfiguration.SetMaxMajorPlayers({})",players.len()),format!("GameConfiguration.SetParticipatingPlayerCount({} + GameConfiguration.GetHiddenPlayerCount())",players.len())]);
    if let Some(count) = step["city_states"].as_u64() {
        lines.extend([format!("GameConfiguration.SetValue('CITY_STATE_COUNT', {count})"),format!("if GameConfiguration.GetValue('CITY_STATE_COUNT') ~= {count} then error('city-state count not applied') end")]);
    }
    for (i, p) in players.iter().enumerate() {
        let leader = quote(p["leader"].as_str().unwrap());
        let civ = quote(p["civ"].as_str().unwrap());
        let slot = if p["human"].as_bool().unwrap() {
            "SS_TAKEN"
        } else {
            "SS_COMPUTER"
        };
        lines.extend([format!("local p = PlayerConfigurations[{i}]"),format!("if not p then error('player slot {i} unavailable') end"),format!("p:SetSlotStatus(SlotStatus.{slot})"),format!("p:SetLeaderTypeName({leader})"),format!("p:SetCivilizationTypeName({civ})"),format!("if p:GetLeaderTypeName() ~= {leader} then error('leader mismatch in slot {i}') end"),format!("if p:GetCivilizationTypeName() ~= {civ} then error('civilization mismatch in slot {i}') end")]);
    }
    lines.push("return true".to_string());
    lines.join("\n")
}

pub fn build_setup(players: &[Value]) -> String {
    let mut lines = vec![
        "BuildHeadlessGameSetup()".to_string(),
        "g_GameParameters:FullRefresh()".to_string(),
    ];
    for (i, p) in players.iter().enumerate() {
        let slot = if p["human"].as_bool().unwrap() {
            "SS_TAKEN"
        } else {
            "SS_COMPUTER"
        };
        lines.extend([format!("local p = PlayerConfigurations[{i}]"),format!("if not p or p:GetLeaderTypeName() ~= {} or p:GetCivilizationTypeName() ~= {} or p:GetSlotStatus() ~= SlotStatus.{slot} then error('setup changed player {i}') end",quote(p["leader"].as_str().unwrap()),quote(p["civ"].as_str().unwrap()))]);
    }
    lines.extend([
        "HideGameSetup()".into(),
        "return 'setup built; configured players survived'".into(),
    ]);
    lines.join("\n")
}

pub const POPUPS: [(&str, &str); 5] = [
    ("DiplomacyDealView", "OnLocalPlayerTurnEnd()"),
    ("DiplomacyActionView", "OnClose()"),
    ("JAC_SengokuReignPopup", "OnClose()"),
    ("DedicationPopup", "OnClose()"),
    ("JAC_SengokuDarkPolicy", "OnClose()"),
];
pub const AUTO_STATUS: &str = "return tostring(Game.GetCurrentGameTurn()) .. '|' .. tostring(AutoplayManager.IsActive()) .. '|' .. tostring(JAC_AUTOTEST_AUTO_DONE == true) .. '|' .. tostring(AutoplayManager.GetTurns()) .. '|' .. tostring(Game.GetLocalPlayer())";
pub const ERA_STATE: &str = "local eras = Game.GetEras()\nreturn tostring(Game.GetCurrentGameTurn()) .. '|' .. tostring(eras:GetCurrentEraMinimumEndTurn()) .. '|' .. tostring(eras:GetCurrentEraMaximumEndTurn()) .. '|' .. tostring(eras:GetNextEraCountdown())";
pub const AUTO_CLEANUP: &str = "if JAC_AUTOTEST_AUTO_HANDLER then LuaEvents.AutoPlayEnd.Remove(JAC_AUTOTEST_AUTO_HANDLER) end\nJAC_AUTOTEST_AUTO_HANDLER = nil\nreturn true";

pub fn autoplay_start(turns: i64, start: i64) -> String {
    format!(
        "if AutoplayManager.IsActive() then error('autoplay already active') end\nlocal startTurn = {start}\nlocal human = Game.GetLocalPlayer()\nif human < 0 then error('no local human player to return to') end\nif JAC_AUTOTEST_AUTO_HANDLER then LuaEvents.AutoPlayEnd.Remove(JAC_AUTOTEST_AUTO_HANDLER) end\nJAC_AUTOTEST_AUTO_DONE = false\nJAC_AUTOTEST_AUTO_HANDLER = function() JAC_AUTOTEST_AUTO_DONE = true end\nLuaEvents.AutoPlayEnd.Add(JAC_AUTOTEST_AUTO_HANDLER)\nAutoplayManager.SetTurns({turns})\nAutoplayManager.SetReturnAsPlayer(human)\nAutoplayManager.SetObserveAsPlayer(PlayerTypes.NONE)\nAutoplayManager.SetActive(true)\nreturn tostring(startTurn) .. '|' .. tostring(human)"
    )
}

pub fn dedication(human: i64) -> String {
    format!(
        r#"local h = {human}
if not (Game.GetEras and PlayerOperations and PlayerOperations.COMMEMORATE) then return 'none' end
local eras = Game.GetEras()
local n = eras:GetPlayerNumAllowedCommemorations(h)
local choices = eras:GetPlayerCommemorateChoices(h)
if n <= 0 or #choices == 0 then return 'none' end
local picked = {{}}
for i = 1, math.min(n, #choices) do
 local p = {{}}
 p[PlayerOperations.PARAM_COMMEMORATION_TYPE] = choices[i]
 UI.RequestPlayerOperation(h, PlayerOperations.COMMEMORATE, p)
 table.insert(picked, tostring(choices[i]))
end
return table.concat(picked, ',')"#
    )
}
