//! Generate a finite FrontEnd diagnostic using the production transaction.
use civ6_autotest_rs::scenario::{enable_mods_code, validate_scenario};
use serde_json::json;

const MAP: &str = "403be889-1455-4de7-967b-ad54c0ff3431";
const MIDDLE: &str = "5c1e7a3d-9b2f-4e61-8d47-2a6f0c9e1b85";
const LEAF: &str = "57a3af09-fb38-41da-b367-818de11a995c";
const TREES: &str = "26c967d0-c10d-4eeb-8aaf-c83dc7e621e7";
const GROWTH: &str = "7696e937-2044-4f3c-8e13-4e27632e2710";

fn transaction(name: &str, enabled: &[&str], disabled: &[&str]) -> String {
    format!(
        "local function {name}()\n{}\nend\n",
        enable_mods_code(
            &enabled.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
            &disabled.iter().map(|s| (*s).into()).collect::<Vec<_>>()
        )
    )
}

fn code() -> String {
    let mut code = format!(
        "local MAP, MIDDLE, LEAF, TREES, GROWTH = {MAP:?}, {MIDDLE:?}, {LEAF:?}, {TREES:?}, {GROWTH:?}\n"
    );
    code.push_str(&transaction("disableCase", &[GROWTH], &[MAP]));
    code.push_str(&transaction("mixedCase", &[LEAF], &[GROWTH]));
    code.push_str(&transaction("enableCase", &[LEAF], &[]));
    code.push_str(HARNESS);
    code
}

fn scenario() -> serde_json::Value {
    let scenario = json!({
        "name": "MOD切替の途中失敗と全選択の復元（ToDo241）",
        "judge_profile": "generic",
        "strict_lua_errors": true,
        "steps": [
            {"action": "restart_game", "timeout": 420},
            {"action": "ui_lua", "state_name": "FrontEnd", "code": code(), "timeout": 90},
            {"action": "ui_lua", "state_name": "FrontEnd", "code": "if not JAC_MOD_ROLLBACK_241 or not JAC_MOD_ROLLBACK_241.passed then error('rollback diagnostic incomplete') end\nreturn JAC_MOD_ROLLBACK_241.selection", "capture_as": "RESTORED_SELECTION", "timeout": 10},
            {"action": "restart_game", "timeout": 420},
            {"action": "ui_lua", "state_name": "FrontEnd", "code": "local rows = {}\nfor _, mod in ipairs(Modding.GetInstalledMods()) do local info = Modding.GetModInfo(mod.Handle); table.insert(rows, string.lower(info.Id) .. '=' .. tostring(info.Enabled)) end\ntable.sort(rows)\nreturn table.concat(rows, ',')", "equals_capture": "RESTORED_SELECTION", "timeout": 10}
        ]
    });
    validate_scenario(&scenario).expect("valid diagnostic");
    scenario
}

fn main() {
    println!("{}", serde_json::to_string_pretty(&scenario()).unwrap());
}

const HARNESS: &str = r#"
if not UI.IsInFrontEnd() then error('main menu required') end
JAC_MOD_ROLLBACK_241 = nil
local nativeEnable, nativeDisable = Modding.EnableMod, Modding.DisableMod
local handles, ids, children, ordered = {}, {}, {}, {}
for _, mod in ipairs(Modding.GetInstalledMods()) do
  local id = string.lower(Modding.GetModInfo(mod.Handle).Id)
  handles[id] = mod.Handle
  table.insert(ids, id)
  for _, dep in ipairs(Modding.GetModAssociations(mod.Handle) or {}) do
    local target = string.lower(dep.Id)
    children[target] = children[target] or {}
    table.insert(children[target], id)
  end
end
table.sort(ids)
for _, id in ipairs({MAP, MIDDLE, LEAF, TREES, GROWTH}) do
  if not handles[id] then error('diagnostic MOD missing: ' .. id) end
end
local seen = {}
local function visit(id)
  if seen[id] then return end
  seen[id] = true
  local deps = children[id] or {}
  table.sort(deps)
  for _, child in ipairs(deps) do visit(child) end
  table.insert(ordered, id)
end
for _, id in ipairs(ids) do visit(id) end
local function enabled(id) return Modding.GetModInfo(handles[id]).Enabled end
local function snapshot()
  local state = {}
  for _, id in ipairs(ids) do state[id] = enabled(id) end
  return state
end
local function serialize(state)
  local values = {}
  for _, id in ipairs(ids) do
    table.insert(values, id .. '=' .. tostring(state[id]))
  end
  return table.concat(values, ',')
end
local function compare(expected, label)
  local mismatch = {}
  for _, id in ipairs(ids) do
    if enabled(id) ~= expected[id] then table.insert(mismatch, id) end
  end
  if #mismatch > 0 then error(label .. ': ' .. table.concat(mismatch, ',')) end
end
-- Cleanup is independent of the production transaction and always uses native APIs.
local function restore(expected)
  local failures = {}
  local off = {}
  local function markOff(id)
    if off[id] then return end
    off[id] = true
    for _, child in ipairs(children[id] or {}) do markOff(child) end
  end
  for _, id in ipairs(ids) do
    if not expected[id] and enabled(id) then markOff(id) end
  end
  for _, id in ipairs(ordered) do
    if off[id] and enabled(id) then
      local ok, err = pcall(nativeDisable, handles[id])
      if not ok then table.insert(failures, tostring(err)) end
    end
  end
  for i = #ordered, 1, -1 do
    local id = ordered[i]
    if expected[id] and not enabled(id) then
      local ok, err = pcall(nativeEnable, handles[id], false)
      if not ok then table.insert(failures, tostring(err)) end
    end
  end
  local ok, err = pcall(GameConfiguration.UpdateEnabledMods)
  if not ok then table.insert(failures, tostring(err)) end
  compare(expected, 'native cleanup selection mismatch')
  if #failures > 0 then error(table.concat(failures, ';')) end
end
local original = snapshot()
print('TODO241 ORIGINAL ' .. serialize(original))
local passed = {}
local function mark(label)
  table.insert(passed, label)
  print('TODO241 PASS ' .. label .. ' installed=' .. #ids)
end
local function force(id, value, dependencies)
  if value then nativeEnable(handles[id], dependencies) else nativeDisable(handles[id]) end
  if enabled(id) ~= value then error('native preparation failed: ' .. id) end
end
local function expectFailure(fn, marker, expected)
  local ok, err = pcall(fn)
  Modding.EnableMod, Modding.DisableMod = nativeEnable, nativeDisable
  if ok or not string.find(tostring(err), marker, 1, true) then
    error('expected injected failure missing: ' .. tostring(err))
  end
  print('TODO241 EXPECTED ' .. tostring(err))
  compare(expected, 'production rollback selection mismatch')
end
local ok, failure = pcall(function()
  force(TREES, true, true)
  force(MAP, true, true)
  force(LEAF, false)
  force(MIDDLE, false)
  force(LEAF, true, false)
  if enabled(MIDDLE) then error('EnableMod(false) enabled disabled intermediate') end
  mark('native-false-preserves-disabled-intermediate')

  local baseline = snapshot()
  local injected = false
  Modding.DisableMod = function(handle)
    nativeDisable(handle)
    if handle == handles[MAP] and not injected then
      injected = true
      error('TODO241_DISABLE_AFTER_NATIVE')
    end
  end
  expectFailure(disableCase, 'TODO241_DISABLE_AFTER_NATIVE', baseline)
  if not injected then error('disable injection not reached') end
  if enabled(MIDDLE) then error('rollback enabled disabled intermediate') end
  mark('disable-failure-restores-all-with-disabled-intermediate')

  for _, mixed in ipairs({false, true}) do
    force(GROWTH, true, true)
    force(LEAF, false)
    force(MIDDLE, false)
    baseline = snapshot()
    local nativeSideEffect, count = false, 0
    injected = false
    Modding.EnableMod = function(handle, dependencies)
      nativeEnable(handle, dependencies)
      if handle == handles[LEAF] and dependencies == true and not injected then
        injected = true
        for _, id in ipairs(ids) do
          if id ~= LEAF and not baseline[id] and enabled(id) then count = count + 1 end
        end
        nativeSideEffect = enabled(MIDDLE) and count > 0
        error('TODO241_ENABLE_AFTER_NATIVE')
      end
    end
    expectFailure(mixed and mixedCase or enableCase, 'TODO241_ENABLE_AFTER_NATIVE', baseline)
    if not injected or not nativeSideEffect then error('native automatic prerequisite not observed') end
    mark((mixed and 'mixed' or 'enable-only') .. '-failure-restores-auto-dependencies=' .. count)
  end

  -- A refused rollback must be reported instead of returning success.
  force(LEAF, false)
  force(MIDDLE, false)
  force(LEAF, true, false)
  baseline = snapshot()
  injected = false
  Modding.DisableMod = function(handle)
    nativeDisable(handle)
    if handle == handles[MAP] and not injected then
      injected = true
      error('TODO241_RESTORE_REFUSAL')
    end
  end
  Modding.EnableMod = function(handle, dependencies)
    if handle == handles[LEAF] and dependencies == false then return end
    return nativeEnable(handle, dependencies)
  end
  local accepted, err = pcall(disableCase)
  Modding.EnableMod, Modding.DisableMod = nativeEnable, nativeDisable
  local rejected = not accepted and injected
    and string.find(tostring(err), 'restoration failed:', 1, true)
    and string.find(tostring(err), 'selection differs from original', 1, true)
    and not enabled(LEAF)
  restore(baseline)
  if not rejected then error('incomplete rollback was not reported: ' .. tostring(err)) end
  mark('incomplete-restoration-is-error')
end)
Modding.EnableMod, Modding.DisableMod = nativeEnable, nativeDisable
local cleaned, cleanupError = pcall(restore, original)
print('TODO241 FINAL ' .. serialize(snapshot()))
if not cleaned then error(tostring(failure) .. '; cleanup failed: ' .. tostring(cleanupError)) end
if not ok then error(failure) end
if #passed ~= 5 then error('incomplete diagnostic coverage') end
JAC_MOD_ROLLBACK_241 = {passed=true, selection=serialize(snapshot())}
return 'TODO241 PASS 5/5 all-selection-restored installed=' .. #ids
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::{Lua, Table};

    #[test]
    fn checked_in_scenario_uses_current_production_transaction() {
        let checked_in: serde_json::Value = serde_json::from_str(include_str!(
            "../../civ6-autotest/scenarios/mod-selection-rollback.json"
        ))
        .unwrap();
        assert_eq!(checked_in, scenario());
    }

    fn fixture(mask: u8) -> Lua {
        let lua = Lua::new();
        let definitions = lua.create_table().unwrap();
        for (index, (id, dependencies)) in [
            (MAP, vec![]),
            (MIDDLE, vec![MAP]),
            (LEAF, vec![MIDDLE, TREES]),
            (TREES, vec![]),
            (GROWTH, vec![]),
        ]
        .iter()
        .enumerate()
        {
            let row = lua.create_table().unwrap();
            row.set("Id", *id).unwrap();
            row.set("Enabled", mask & (1 << index) != 0).unwrap();
            row.set(
                "Dependencies",
                lua.create_sequence_from(dependencies.iter().copied())
                    .unwrap(),
            )
            .unwrap();
            definitions.set(index + 1, row).unwrap();
        }
        lua.load(include_str!("../tests/fixtures/modding_stub.lua"))
            .call::<()>(definitions)
            .unwrap();
        lua
    }

    #[test]
    fn generated_diagnostic_covers_five_cases_and_restores_every_original_selection() {
        for mask in 0..32 {
            let lua = fixture(mask);
            let result: String = lua.load(code()).eval().unwrap();
            assert!(result.starts_with("TODO241 PASS 5/5"));
            let mods: Table = lua.globals().get("MODS").unwrap();
            for (index, id) in [MAP, MIDDLE, LEAF, TREES, GROWTH].iter().enumerate() {
                let info: Table = mods.get(*id).unwrap();
                assert_eq!(
                    info.get::<bool>("Enabled").unwrap(),
                    mask & (1 << index) != 0
                );
            }
        }
    }

    #[test]
    fn native_false_enabling_dependencies_is_detected() {
        let lua = fixture(31);
        lua.load("local original = Modding.EnableMod; Modding.EnableMod = function(handle, dependencies) return original(handle, true) end")
            .exec().unwrap();
        let failure = lua.load(code()).eval::<String>().unwrap_err().to_string();
        assert!(failure.contains("EnableMod(false) enabled disabled intermediate"));
    }
}
