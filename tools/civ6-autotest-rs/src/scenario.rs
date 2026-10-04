use crate::{Error, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::PathBuf};

pub const BANSEIKI_RULESET: &str = "RULESET_JAC_BANSEIKI";
pub const ACTIONS: [&str; 20] = [
    "restart_game",
    "enable_mods",
    "configure",
    "start_game",
    "autoplay",
    "end_turn",
    "ui_lua",
    "screenshot",
    "save",
    "exit_to_menu",
    "load",
    "wait_for",
    "pause",
    "manual",
    "start_native_governor_trace",
    "stop_native_governor_trace",
    "start_native_can_ever_trace",
    "stop_native_can_ever_trace",
    "start_native_property_trace",
    "stop_native_property_trace",
];
pub fn lua_string(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}
pub fn matches(pattern: &str, value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| Regex::new(pattern).unwrap().is_match(s))
}
pub fn type_id(value: &Value) -> bool {
    matches(r"^[A-Z][A-Z0-9_]+$", value)
}
pub fn validate_test_mode(mode: &Value) -> Result<()> {
    if !mode.is_object()
        || !mode["skipped_turns"].as_i64().is_some_and(|n| n >= 0)
        || mode["continuous_performance_valid"] != false
    {
        return Err(Error::scenario("test_mode needs a nonnegative integer skipped_turns and continuous_performance_valid: false"));
    }
    Ok(())
}
pub fn safe_relative_path(value: &str) -> Result<PathBuf> {
    let normalized = value.replace('\\', "/");
    if normalized
        .split('/')
        .all(|part| part.is_empty() || part == ".")
        || normalized.starts_with('/')
        || normalized.as_bytes().get(1) == Some(&b':')
        || normalized.split('/').any(|s| s == "..")
    {
        return Err(Error::scenario(format!(
            "Path must be relative to output directory: {value:?}"
        )));
    }
    Ok(PathBuf::from(normalized))
}
pub fn validate_scenario(data: &Value) -> Result<()> {
    if !data.is_object() || !data["name"].is_string() {
        return Err(Error::scenario("Scenario needs a name"));
    }
    if let Some(mode) = data.get("test_mode") {
        validate_test_mode(mode)?;
    }
    let profile = data
        .get("judge_profile")
        .and_then(Value::as_str)
        .unwrap_or("generic");
    if data.get("judge_profile").is_some_and(|v| !v.is_string())
        || !["generic", "bundle0", "growth", "japan_map"].contains(&profile)
    {
        return Err(Error::scenario(
            "judge_profile must be bundle0, generic, growth or japan_map",
        ));
    }
    if profile == "growth" && !data["expected_units"].as_i64().is_some_and(|n| n > 0) {
        return Err(Error::scenario(
            "growth scenario needs a positive expected_units count",
        ));
    }
    if data
        .get("strict_lua_errors")
        .is_some_and(|v| !v.is_boolean())
    {
        return Err(Error::scenario("strict_lua_errors must be boolean"));
    }
    let steps = data["steps"]
        .as_array()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::scenario("Scenario needs a nonempty steps array"))?;
    let mut trace_open = false;
    let mut bridge_open = false;
    let mut property_open = false;
    let mut trace_names = BTreeSet::new();
    for (i, step) in steps.iter().enumerate() {
        let fail = |s: String| Error::scenario(format!("Step {}: {s}", i + 1));
        let action = step["action"]
            .as_str()
            .filter(|s| ACTIONS.contains(s))
            .ok_or_else(|| fail("unknown action".into()))?;
        let bool_flags = |names: &[&str]| -> Result<()> {
            for name in names {
                if step.get(*name).is_some_and(|v| !v.is_boolean()) {
                    return Err(fail(format!("{name} must be boolean")));
                }
            }
            Ok(())
        };
        if bridge_open
            && matches!(
                action,
                "restart_game" | "start_game" | "exit_to_menu" | "load"
            )
        {
            return Err(fail(
                "stop native CanEver bridge before changing game lifecycle".into(),
            ));
        }
        if property_open && matches!(action, "save" | "end_turn" | "autoplay") {
            return Err(fail(
                "stop native property observation before a save or turn change".into(),
            ));
        }
        match action {
            "start_native_can_ever_trace" | "start_native_property_trace" => {
                if bridge_open || trace_open {
                    return Err(fail("native observation already active".into()));
                }
                if !matches(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,79}$", &step["name"])
                    || !trace_names.insert(step["name"].as_str().unwrap().to_owned())
                    || !step["max_seconds"].as_u64().is_some_and(|n| {
                        (1..=u64::from(civ6_native_bridge_rs::MAX_ACTIVE_SECONDS)).contains(&n)
                    })
                    || step.as_object().unwrap().keys().any(|k| {
                        !["action", "name", "max_seconds", "timeout"].contains(&k.as_str())
                    })
                {
                    return Err(fail("invalid bounded native CanEver bridge start".into()));
                }
                bridge_open = true;
                property_open = action == "start_native_property_trace";
            }
            "stop_native_can_ever_trace" | "stop_native_property_trace" => {
                if !bridge_open
                    || property_open != (action == "stop_native_property_trace")
                    || step
                        .as_object()
                        .unwrap()
                        .keys()
                        .any(|k| !["action", "timeout"].contains(&k.as_str()))
                {
                    return Err(fail("invalid native CanEver bridge stop".into()));
                }
                bridge_open = false;
                property_open = false;
            }
            "start_native_governor_trace" => {
                if trace_open || bridge_open {
                    return Err(fail("native governor trace is already active".into()));
                }
                if !matches(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,79}$", &step["name"]) {
                    return Err(fail("trace name must be a safe artifact basename".into()));
                }
                if !trace_names.insert(step["name"].as_str().unwrap().to_owned()) {
                    return Err(fail("trace name must be unique within the scenario".into()));
                }
                if !step["max_seconds"]
                    .as_u64()
                    .is_some_and(|n| (1..=300).contains(&n))
                {
                    return Err(fail("max_seconds must be 1..300".into()));
                }
                if step.get("mode").is_some()
                    && !matches!(
                        step["mode"].as_str(),
                        Some("governor" | "property" | "lifecycle")
                    )
                {
                    return Err(fail(
                        "trace mode must be governor, property or lifecycle".into(),
                    ));
                }
                if step["mode"] == "lifecycle"
                    && step["max_seconds"]
                        .as_u64()
                        .is_some_and(|n| n > civ6_native_probe_rs::MAX_LIFECYCLE_SECONDS)
                {
                    return Err(fail(
                        "lifecycle observation is limited to 20 seconds".into(),
                    ));
                }
                if step.as_object().unwrap().keys().any(|k| {
                    !["action", "name", "max_seconds", "mode", "timeout"].contains(&k.as_str())
                }) {
                    return Err(fail("unexpected trace start field".into()));
                }
                trace_open = true;
            }
            "stop_native_governor_trace" => {
                if !trace_open {
                    return Err(fail("native governor trace is not active".into()));
                }
                if step
                    .as_object()
                    .unwrap()
                    .keys()
                    .any(|k| !["action", "timeout"].contains(&k.as_str()))
                {
                    return Err(fail("unexpected trace stop field".into()));
                }
                trace_open = false;
            }
            "restart_game" => {
                if trace_open || bridge_open {
                    return Err(fail(
                        "stop native governor trace before restarting Civ6".into(),
                    ));
                }
                if step
                    .as_object()
                    .unwrap()
                    .keys()
                    .any(|k| k != "action" && k != "timeout")
                {
                    return Err(fail("restart_game only accepts timeout".into()));
                }
            }
            "enable_mods" => {
                let valid = |v: &Value| {
                    matches(r"^[0-9a-fA-F]{8}-(?:[0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12}$", v)
                };
                let ids = step["ids"]
                    .as_array()
                    .filter(|a| !a.is_empty() && a.iter().all(valid))
                    .ok_or_else(|| fail("ids must be mod UUIDs".into()))?;
                if let Some(disabled) = step.get("disable_ids") {
                    let set: BTreeSet<_> = ids
                        .iter()
                        .map(|v| v.as_str().unwrap().to_lowercase())
                        .collect();
                    if !disabled.as_array().is_some_and(|a| {
                        a.iter().all(valid)
                            && a.iter()
                                .all(|v| !set.contains(&v.as_str().unwrap().to_lowercase()))
                    }) {
                        return Err(fail("invalid disable_ids".into()));
                    }
                }
            }
            "configure" => {
                bool_flags(&["build_setup"])?;
                for key in ["ruleset", "map_size", "speed"] {
                    if !type_id(&step[key]) {
                        return Err(fail(format!("invalid {key}")));
                    }
                }
                if !step["seed"]
                    .as_i64()
                    .is_some_and(|v| i32::try_from(v).is_ok())
                {
                    return Err(fail("seed must be a signed 32-bit integer".into()));
                }
                if step.get("map_file").is_some_and(|v| {
                    !matches(r"^\{[0-9a-fA-F-]{36}\}Maps/[A-Za-z0-9_-]+\.Civ6Map$", v)
                }) {
                    return Err(fail("invalid map_file".into()));
                }
                if step
                    .get("city_states")
                    .is_some_and(|v| !v.as_i64().is_some_and(|n| (0..=24).contains(&n)))
                {
                    return Err(fail("city_states must be an integer from 0 to 24".into()));
                }
                let players = step["players"]
                    .as_array()
                    .filter(|a| {
                        a.len() >= 2 && a.iter().filter(|p| p["human"] == true).count() == 1
                    })
                    .ok_or_else(|| {
                        fail("specify at least two players and exactly one human".into())
                    })?;
                if players.iter().any(|p| {
                    !p.is_object()
                        || !p["human"].is_boolean()
                        || !type_id(&p["leader"])
                        || !type_id(&p["civ"])
                }) {
                    return Err(fail("invalid player".into()));
                }
            }
            "autoplay" => {
                if !step["turns"].as_i64().is_some_and(|n| n > 0) {
                    return Err(fail("turns must be positive".into()));
                }
                bool_flags(&[
                    "metrics",
                    "research_metrics",
                    "allow_human_elimination",
                    "fast_forward",
                ])?;
                if step
                    .get("metrics_group")
                    .is_some_and(|v| !type_id(v) || step["metrics"] != true)
                {
                    return Err(fail(
                        "metrics_group needs a type ID and metrics: true".into(),
                    ));
                }
                if step["fast_forward"] == true && !data["test_mode"].is_object() {
                    return Err(fail("fast_forward requires test mode".into()));
                }
            }
            "end_turn" => {
                for key in ["turns", "test_turns"] {
                    if step
                        .get(key)
                        .is_some_and(|v| !v.as_i64().is_some_and(|n| n > 0))
                    {
                        return Err(fail(format!("{key} must be positive")));
                    }
                }
            }
            "ui_lua" => {
                if step.get("lua_timeout").is_some_and(|v| {
                    !v.as_f64()
                        .is_some_and(|n| n.is_finite() && n > 0.0 && n <= 600.0)
                }) {
                    return Err(fail(
                        "lua_timeout must be finite and between 0 and 600 seconds".into(),
                    ));
                }
                if !step["state_name"].is_string()
                    || !step["code"].as_str().is_some_and(|s| !s.trim().is_empty())
                {
                    return Err(fail("ui_lua needs state_name and code".into()));
                }
                for key in ["capture_as", "equals_capture"] {
                    if step.get(key).is_some_and(|v| !type_id(v)) {
                        return Err(fail(format!("invalid {key}")));
                    }
                }
                crate::tuner::validate_lua_code(step["code"].as_str().unwrap())
                    .map_err(|e| fail(e.to_string()))?;
            }
            "screenshot" => {
                safe_relative_path(
                    step["path"]
                        .as_str()
                        .ok_or_else(|| fail("Screenshot path must be a string".into()))?,
                )?;
                if let Some(crop) = step.get("crop").filter(|v| !v.is_null()) {
                    if !crop.as_array().is_some_and(|a| {
                        a.len() == 4
                            && a.iter()
                                .all(|v| v.as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)))
                            && a[0].as_f64() < a[2].as_f64()
                            && a[1].as_f64() < a[3].as_f64()
                    }) {
                        return Err(fail(
                            "crop must be [left, top, right, bottom] fractions".into(),
                        ));
                    }
                }
            }
            "save" | "load" => {
                if !matches(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,79}$", &step["name"]) {
                    return Err(fail("invalid save name".into()));
                }
                bool_flags(&["exact_name", "metrics", "checkpoint"])?;
                if step["checkpoint"] == true && step["exact_name"] != true {
                    return Err(fail("checkpoint requires exact_name: true".into()));
                }
            }
            "wait_for" => {
                let c = &step["condition"];
                match c["type"].as_str() {
                    Some("lua") => {
                        if !c["state_name"].is_string()
                            || !c["code"].as_str().is_some_and(|s| !s.trim().is_empty())
                        {
                            return Err(fail("Lua wait needs state_name and code".into()));
                        }
                        crate::tuner::validate_lua_code(c["code"].as_str().unwrap())
                            .map_err(|e| fail(e.to_string()))?;
                    }
                    Some("state") => {
                        if !c["name"].is_string() {
                            return Err(fail("state wait needs name".into()));
                        }
                    }
                    Some("log_line") => {
                        if !c["log"].as_str().is_some_and(|s| {
                            ["Lua.log", "Database.log", "Modding.log"].contains(&s)
                        }) || !c["contains"].as_str().is_some_and(|s| !s.is_empty())
                        {
                            return Err(fail("log wait needs log and contains".into()));
                        }
                    }
                    _ => {
                        return Err(fail(
                            "wait_for needs a lua, log_line or state condition".into(),
                        ))
                    }
                }
            }
            "pause" => {
                if !step["seconds"]
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && n > 0.0 && n <= 10.0)
                {
                    return Err(fail("pause seconds must be in (0, 10]".into()));
                }
            }
            "manual" => {
                if !step["prompt"]
                    .as_str()
                    .is_some_and(|s| !s.trim().is_empty())
                {
                    return Err(fail("manual step needs prompt".into()));
                }
                if step.get("timeout").is_some() {
                    return Err(fail(
                        "manual waits for Enter and does not support timeout".into(),
                    ));
                }
            }
            _ => {}
        }
        if step
            .get("timeout")
            .is_some_and(|v| !v.as_f64().is_some_and(|n| n.is_finite() && n > 0.0))
        {
            return Err(fail("timeout must be a finite positive number".into()));
        }
    }
    if trace_open || bridge_open {
        return Err(Error::scenario("native governor trace has no stop step"));
    }
    Ok(())
}
pub fn fast_forward_scenario(data: &Value) -> Value {
    let mut data = data.clone();
    if let Some(steps) = data["steps"].as_array_mut() {
        for step in steps {
            if step["action"] == "autoplay" {
                if step["turns"].as_i64().unwrap_or(0) > 2 {
                    step["fast_forward"] = true.into();
                }
                if step["metrics"] == true {
                    step["metrics_group"] = format!(
                        "TEST_MODE_{}",
                        step["metrics_group"].as_str().unwrap_or("AUTOPLAY")
                    )
                    .into();
                }
            }
        }
    }
    data["test_mode"] = json!({"skipped_turns":0,"continuous_performance_valid":false});
    data
}
pub fn builds_setup(step: &Value) -> bool {
    step["build_setup"]
        .as_bool()
        .unwrap_or(step["ruleset"] == BANSEIKI_RULESET)
}
pub fn configure_code(step: &Value) -> String {
    let players = step["players"].as_array().unwrap();
    let mut lines = vec![
        "if not UI.IsInFrontEnd() then error('main menu required') end".into(),
        "GameConfiguration.SetToDefaults()".into(),
        format!(
            "GameConfiguration.SetRuleSet({})",
            lua_string(step["ruleset"].as_str().unwrap())
        ),
        "GameConfiguration.UpdateEnabledMods()".into(),
    ];
    if let Some(file) = step["map_file"].as_str() {
        lines.push(format!(
            "MapConfiguration.SetValue('MAP_SCRIPT', {})",
            lua_string(file)
        ));
    }
    lines.extend([format!("MapConfiguration.SetMapSize({})",lua_string(step["map_size"].as_str().unwrap())),format!("GameConfiguration.SetGameSpeedType({})",lua_string(step["speed"].as_str().unwrap())),format!("MapConfiguration.SetValue('RANDOM_SEED', {})",step["seed"]),format!("GameConfiguration.SetValue('GAME_SYNC_RANDOM_SEED', {})",step["seed"]),format!("MapConfiguration.SetMaxMajorPlayers({})",players.len()),format!("GameConfiguration.SetParticipatingPlayerCount({} + GameConfiguration.GetHiddenPlayerCount())",players.len())]);
    if let Some(count) = step.get("city_states") {
        lines.extend([format!("GameConfiguration.SetValue('CITY_STATE_COUNT', {count})"),format!("if GameConfiguration.GetValue('CITY_STATE_COUNT') ~= {count} then error('city-state count not applied') end")]);
    }
    for (i, p) in players.iter().enumerate() {
        let leader = lua_string(p["leader"].as_str().unwrap());
        let civ = lua_string(p["civ"].as_str().unwrap());
        let slot = if p["human"] == true {
            "SS_TAKEN"
        } else {
            "SS_COMPUTER"
        };
        lines.extend([format!("local p = PlayerConfigurations[{i}]"),format!("if not p then error('player slot {i} unavailable') end"),format!("p:SetSlotStatus(SlotStatus.{slot})"),format!("p:SetLeaderTypeName({leader})"),format!("p:SetCivilizationTypeName({civ})"),format!("if p:GetLeaderTypeName() ~= {leader} then error('leader mismatch in slot {i}') end"),format!("if p:GetCivilizationTypeName() ~= {civ} then error('civilization mismatch in slot {i}') end")]);
    }
    lines.push("return true".into());
    lines.join("\n")
}
pub fn build_setup_code(players: &[Value]) -> String {
    let mut lines = vec![
        "BuildHeadlessGameSetup()".to_string(),
        "g_GameParameters:FullRefresh()".into(),
    ];
    for (i, p) in players.iter().enumerate() {
        let slot = if p["human"] == true {
            "SS_TAKEN"
        } else {
            "SS_COMPUTER"
        };
        lines.extend([format!("local p = PlayerConfigurations[{i}]"),format!("if not p or p:GetLeaderTypeName() ~= {} or p:GetCivilizationTypeName() ~= {} or p:GetSlotStatus() ~= SlotStatus.{slot} then error('setup changed player {i}') end",lua_string(p["leader"].as_str().unwrap()),lua_string(p["civ"].as_str().unwrap()))]);
    }
    lines.extend([
        "HideGameSetup()".into(),
        "return 'setup built; configured players survived'".into(),
    ]);
    lines.join("\n")
}
pub fn enable_mods_code(ids: &[String], disabled: &[String]) -> String {
    let disabled = isolated_disabled_mods(ids, disabled);
    format!("if not UI.IsInFrontEnd() then error('main menu required') end\nlocal enableIDs = {{{}}}\nlocal disableIDs = {{{}}}\n{}",ids.iter().map(|s|lua_string(s)).collect::<Vec<_>>().join(", "),disabled.iter().map(|s|lua_string(s)).collect::<Vec<_>>().join(", "),ENABLE_MODS_BODY)
}

// A diagnostic that replaces production eligibility must never leak from the
// preceding game. Enforce this at the shared selection boundary, so generated
// and handwritten scenarios have the same isolation without snapshot edits.
pub const STATIC_PERSONA_GATE_ID: &str = "c678a216-58ab-4d40-a614-8f309d36ac86";
pub const ENEMY_PERSONA_GATE_ID: &str = "bba3a0d2-8f08-4715-9263-00a773e6ae86";
pub const TRAIT_GRANT_PROBE_ID: &str = "2b20f0d9-64cd-42b8-9841-74067419b81e";
pub fn isolated_disabled_mods(ids: &[String], disabled: &[String]) -> Vec<String> {
    let mut result = disabled.to_vec();
    for gate in [
        STATIC_PERSONA_GATE_ID,
        ENEMY_PERSONA_GATE_ID,
        TRAIT_GRANT_PROBE_ID,
    ] {
        if !ids.iter().any(|id| id.eq_ignore_ascii_case(gate))
            && !result.iter().any(|id| id.eq_ignore_ascii_case(gate))
        {
            result.push(gate.to_owned());
        }
    }
    result
}
const ENABLE_MODS_BODY: &str = r###"local keep, disabled = {}, {}
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
return table.concat(disabled, ', ')"###;

#[cfg(test)]
mod native_trace_tests {
    use super::*;
    #[test]
    fn native_trace_steps_are_bounded_and_paired() {
        let start = json!({"action":"start_native_governor_trace","name":"governor-before-join","max_seconds":300});
        let stop = json!({"action":"stop_native_governor_trace"});
        let scenario = |steps: Vec<Value>| json!({"name":"trace contract","steps":steps});
        assert!(validate_scenario(&scenario(vec![start.clone(), stop.clone()])).is_ok());
        assert!(validate_scenario(&scenario(vec![start.clone()])).is_err());
        assert!(validate_scenario(&scenario(vec![stop.clone()])).is_err());
        assert!(
            validate_scenario(&scenario(vec![start.clone(), start.clone(), stop.clone()])).is_err()
        );
        assert!(validate_scenario(&scenario(vec![
            start.clone(),
            json!({"action":"restart_game"}),
            stop.clone()
        ]))
        .is_err());
        for bad in [0, 301] {
            let mut step = start.clone();
            step["max_seconds"] = json!(bad);
            assert!(validate_scenario(&scenario(vec![step, stop.clone()])).is_err());
        }
        let mut property = start.clone();
        property["mode"] = json!("property");
        assert!(validate_scenario(&scenario(vec![property.clone(), stop.clone()])).is_ok());
        let mut lifecycle = start.clone();
        lifecycle["mode"] = json!("lifecycle");
        lifecycle["max_seconds"] = json!(20);
        assert!(validate_scenario(&scenario(vec![lifecycle.clone(), stop.clone()])).is_ok());
        lifecycle["max_seconds"] = json!(21);
        assert!(validate_scenario(&scenario(vec![lifecycle, stop.clone()])).is_err());
        property["mode"] = json!("unknown");
        assert!(validate_scenario(&scenario(vec![property, stop.clone()])).is_err());
        let mut bad = start;
        bad["name"] = json!("../outside");
        assert!(validate_scenario(&scenario(vec![bad, stop])).is_err());
        let again = json!({"action":"start_native_governor_trace","name":"governor-before-join","max_seconds":1});
        assert!(validate_scenario(&scenario(vec![
            json!({"action":"start_native_governor_trace","name":"governor-before-join","max_seconds":1}),
            json!({"action":"stop_native_governor_trace"}), again,
            json!({"action":"stop_native_governor_trace"})
        ])).is_err());
    }

    #[test]
    fn native_bridge_is_separate_bounded_and_stopped_before_game_lifecycle_changes() {
        let start =
            json!({"action":"start_native_can_ever_trace","name":"can-ever","max_seconds":300});
        let stop = json!({"action":"stop_native_can_ever_trace"});
        let scenario = |steps: Vec<Value>| json!({"name":"bridge contract","steps":steps});
        assert!(validate_scenario(&scenario(vec![start.clone(), stop.clone()])).is_ok());
        assert!(validate_scenario(&scenario(vec![start.clone()])).is_err());
        assert!(validate_scenario(&scenario(vec![stop.clone()])).is_err());
        for bad in [0, 301] {
            let mut step = start.clone();
            step["max_seconds"] = json!(bad);
            assert!(validate_scenario(&scenario(vec![step, stop.clone()])).is_err());
        }
        let mut extra = start.clone();
        extra["mode"] = json!("property");
        assert!(validate_scenario(&scenario(vec![extra, stop.clone()])).is_err());
        assert!(validate_scenario(&scenario(vec![
            start.clone(),
            json!({"action":"end_turn","turns":1,"timeout":600}),
            stop.clone()
        ]))
        .is_ok());
        for lifecycle in ["restart_game", "exit_to_menu", "load"] {
            assert!(validate_scenario(&scenario(vec![
                start.clone(),
                json!({"action":lifecycle}),
                stop.clone()
            ]))
            .is_err());
        }
        assert!(validate_scenario(&scenario(vec![
            start,
            json!({"action":"start_native_governor_trace","name":"conflict","max_seconds":20,"mode":"lifecycle"}),
            stop
        ])).is_err());
    }

    #[test]
    fn property_trace_requires_its_own_stop_and_excludes_save_turn_and_load() {
        let start =
            json!({"action":"start_native_property_trace","name":"values","max_seconds":120});
        let stop = json!({"action":"stop_native_property_trace"});
        let scenario = |steps: Vec<Value>| json!({"name":"property contract","steps":steps});
        assert!(validate_scenario(&scenario(vec![start.clone(),
            json!({"action":"ui_lua","state_name":"InGame","code":"return Game:GetProperty('JAC_TEST853_VALUE')"}),
            stop.clone()])).is_ok());
        assert!(validate_scenario(&scenario(vec![start.clone()])).is_err());
        assert!(validate_scenario(&scenario(vec![
            start.clone(),
            json!({"action":"stop_native_can_ever_trace"})
        ]))
        .is_err());
        for action in [
            "save",
            "load",
            "end_turn",
            "autoplay",
            "restart_game",
            "exit_to_menu",
        ] {
            assert!(
                validate_scenario(&scenario(vec![
                    start.clone(),
                    json!({"action":action}),
                    stop.clone()
                ]))
                .is_err(),
                "{action}"
            );
        }
        for duration in [0, 301] {
            let mut bad = start.clone();
            bad["max_seconds"] = json!(duration);
            assert!(validate_scenario(&scenario(vec![bad, stop.clone()])).is_err());
        }
    }
}
