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
        "local nativeModding = Modding\nlocal Modding = setmetatable({{}}, {{__index=nativeModding}})\nlocal MAP, MIDDLE, LEAF, TREES, GROWTH = {MAP:?}, {MIDDLE:?}, {LEAF:?}, {TREES:?}, {GROWTH:?}\n"
    );
    code.push_str(&transaction("disableCase", &[GROWTH], &[MAP]));
    code.push_str(&transaction("mixedCase", &[LEAF], &[GROWTH]));
    code.push_str(&transaction("enableCase", &[LEAF], &[]));
    code.push_str(HARNESS);
    code
}

fn scenario() -> serde_json::Value {
    let mut steps = vec![
        json!({"action": "restart_game", "timeout": 420}),
        json!({"action": "ui_lua", "state_name": "FrontEnd", "code": code(), "timeout": 90}),
        json!({"action": "ui_lua", "state_name": "FrontEnd", "code": "if not JAC_MOD_ROLLBACK_241 or not JAC_MOD_ROLLBACK_241.passed then error('rollback diagnostic incomplete') end\nreturn #JAC_MOD_ROLLBACK_241.rows", "capture_as": "MOD_COUNT", "timeout": 10}),
    ];
    for batch in 0..16 {
        let start = batch * 32 + 1;
        let end = (batch + 1) * 32;
        steps.push(json!({"action":"ui_lua", "state_name":"FrontEnd", "timeout":10,
            "capture_as":format!("MOD_SELECTION_{batch:02}"),
            "code":format!("local rows = JAC_MOD_ROLLBACK_241.rows\nlocal out = {{}}\nfor i = {start}, math.min({end}, #rows) do table.insert(out, rows[i]) end\nreturn table.concat(out, ',')")}));
    }
    steps.push(json!({"action":"restart_game", "timeout":420}));
    let read_rows = "local rows = {}\nfor _, mod in ipairs(Modding.GetInstalledMods()) do local info = Modding.GetModInfo(mod.Handle); table.insert(rows, string.lower(info.Id) .. '=' .. tostring(info.Enabled)) end\ntable.sort(rows)\n";
    steps.push(
        json!({"action":"ui_lua", "state_name":"FrontEnd", "timeout":10,
        "equals_capture":"MOD_COUNT", "code":format!("{read_rows}return #rows")}),
    );
    for batch in 0..16 {
        let start = batch * 32 + 1;
        let end = (batch + 1) * 32;
        steps.push(json!({"action":"ui_lua", "state_name":"FrontEnd", "timeout":10,
            "equals_capture":format!("MOD_SELECTION_{batch:02}"),
            "code":format!("{read_rows}local out = {{}}\nfor i = {start}, math.min({end}, #rows) do table.insert(out, rows[i]) end\nreturn table.concat(out, ',')")}));
    }
    let scenario = json!({
        "name": "MOD切替の途中失敗と全選択の復元（ToDo241）",
        "judge_profile": "generic",
        "strict_lua_errors": true,
        "steps": steps
    });
    validate_scenario(&scenario).expect("valid diagnostic");
    scenario
}

fn parse_original(text: &str) -> Result<std::collections::BTreeMap<String, bool>, String> {
    let mut original = std::collections::BTreeMap::new();
    let mut seen_guids = std::collections::BTreeSet::new();
    let mut counted = false;
    let mut rows_finished = false;
    for line in text.lines() {
        if let Some(row) = line.strip_prefix("FrontEnd: TODO241 ORIGINAL ") {
            if counted || rows_finished {
                return Err("original rows from multiple runs".into());
            }
            let (id, flag) = row.split_once('=').ok_or("incomplete original row")?;
            let guid = id
                .strip_prefix('{')
                .and_then(|s| s.strip_suffix('}'))
                .unwrap_or(id);
            if guid.len() != 36
                || !guid.bytes().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        b == b'-'
                    } else {
                        b.is_ascii_hexdigit()
                    }
                })
            {
                return Err("invalid original MOD id".into());
            }
            let enabled = match flag {
                "true" => true,
                "false" => false,
                _ => return Err("invalid original flag".into()),
            };
            if !seen_guids.insert(guid.to_ascii_lowercase()) {
                return Err("duplicate original row".into());
            }
            original.insert(id.to_ascii_lowercase(), enabled);
        } else {
            // A native audit emits the whole snapshot without yielding. A new
            // ORIGINAL block after other output belongs to a different run,
            // even if the first run never reached its PASS marker.
            rows_finished |= !original.is_empty();
            let Some(pass) = line.strip_prefix("FrontEnd: TODO241 PASS ") else {
                continue;
            };
            let (label, count) = pass
                .rsplit_once(" installed=")
                .ok_or("missing diagnostic count")?;
            if !counted && label != "native-false-preserves-disabled-intermediate" {
                return Err("missing first diagnostic count".into());
            }
            let count: usize = count.parse().map_err(|_| "invalid diagnostic count")?;
            if original.is_empty() || original.len() != count || count > 512 {
                return Err("every original MOD row is required".into());
            }
            counted = true;
        }
    }
    if !counted {
        return Err("original diagnostic count is required".into());
    }
    Ok(original)
}

fn recovery_scenario(text: &str) -> serde_json::Value {
    let mut data = scenario();
    {
        let original = parse_original(text).expect("complete original snapshot");
        let mut recovery = format!(
            "if not UI.IsInFrontEnd() then error('main menu required') end\nlocal saved = {{{}}}\n",
            original
                .iter()
                .map(|(id, flag)| format!(
                    "[{}]={flag}",
                    civ6_autotest_rs::scenario::lua_string(id)
                ))
                .collect::<Vec<_>>()
                .join(",")
        );
        recovery.push_str("local nativeModding = Modding\nlocal Modding = setmetatable({}, {__index=nativeModding})\nlocal expected, handles, enableIDs, disableIDs = {}, {}, {}, {}\nfor _, mod in ipairs(Modding.GetInstalledMods()) do local info = Modding.GetModInfo(mod.Handle); local id = string.lower(info.Id); handles[id]=mod.Handle; expected[id]=info.Enabled end\nModding.GetModHandle = function(id) return handles[string.lower(id)] end\nfor id, flag in pairs(saved) do if not handles[id] then error('original MOD no longer installed: ' .. id) end; expected[id]=flag end\nfor id, flag in pairs(expected) do if flag then table.insert(enableIDs,id) else table.insert(disableIDs,id) end end\ntable.sort(enableIDs); table.sort(disableIDs)\nlocal function apply()\n");
        recovery.push_str(
            &enable_mods_code(&[], &[])
                .lines()
                .skip(3)
                .collect::<Vec<_>>()
                .join("\n"),
        );
        recovery.push_str("\nend\napply()\nlocal count=0\nfor id, flag in pairs(expected) do if Modding.GetModInfo(handles[id]).Enabled ~= flag then error('original recovery mismatch: ' .. id) end; print('TODO241 RECOVER ' .. id .. '=' .. tostring(flag)); count=count+1 end\nreturn 'TODO241 RECOVERED original selection; installed=' .. count");
        data["steps"].as_array_mut().unwrap().insert(
            1,
            json!({"action":"ui_lua","state_name":"FrontEnd","code":recovery,"timeout":120}),
        );
    }
    validate_scenario(&data).expect("valid recovery diagnostic");
    data
}

fn main() {
    let data = match std::env::args().nth(1) {
        Some(log) => {
            recovery_scenario(&std::fs::read_to_string(log).expect("read original selection log"))
        }
        None => scenario(),
    };
    println!("{}", serde_json::to_string_pretty(&data).unwrap());
}

const HARNESS: &str = r#"
if not UI.IsInFrontEnd() then error('main menu required') end
JAC_MOD_ROLLBACK_241 = nil
local nativeEnable, nativeDisable = Modding.EnableMod, Modding.DisableMod
if type(nativeEnable) ~= 'function' or type(nativeDisable) ~= 'function'
  or type(GameConfiguration.UpdateEnabledMods) ~= 'function' then
  error('required native MOD API missing before selection changes')
end
local function resetApis()
  local errors = {}
  local a, ae = pcall(function()
    Modding.EnableMod = nativeEnable
    if Modding.EnableMod ~= nativeEnable then error('enable API restore did not take effect') end
  end)
  local b, be = pcall(function()
    Modding.DisableMod = nativeDisable
    if Modding.DisableMod ~= nativeDisable then error('disable API restore did not take effect') end
  end)
  if not a then table.insert(errors, tostring(ae)) end
  if not b then table.insert(errors, tostring(be)) end
  return a and b, table.concat(errors, ';')
end
-- Probe both assignments before the first selection mutation.
local canWrap, wrapError = pcall(function()
  local enable = function(...) return nativeEnable(...) end
  local disable = function(...) return nativeDisable(...) end
  Modding.EnableMod, Modding.DisableMod = enable, disable
  if Modding.EnableMod ~= enable or Modding.DisableMod ~= disable then
    error('API assignment did not take effect')
  end
end)
local reset, resetError = resetApis()
if not canWrap or not reset then
  error('API assignment preflight failed: ' .. tostring(wrapError) .. ';' .. tostring(resetError))
end
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
if #ids > 512 then error('diagnostic supports at most 512 installed MODs') end
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
local function rows(state)
  local values = {}
  for _, id in ipairs(ids) do
    table.insert(values, id .. '=' .. tostring(state[id]))
    if #values[#values] > 48 then error('MOD snapshot row exceeds transport budget') end
  end
  return values
end
local function audit(label, state)
  for _, value in ipairs(rows(state)) do print('TODO241 ' .. label .. ' ' .. value) end
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
      -- Keep cleanup effective even if the engine rejects false. Only use
      -- dependency expansion after the engine confirms every prerequisite is
      -- already enabled, so an originally disabled prerequisite stays off.
      if not enabled(id) then
        local checked, reason = pcall(nativeModding.CanEnableMod, handles[id], false)
        if checked and reason == 'OK' then
          ok, err = pcall(nativeEnable, handles[id], true)
        elseif not checked then
          table.insert(failures, id .. ': cleanup preflight: ' .. tostring(reason))
        end
      end
      if not ok then table.insert(failures, tostring(err)) end
    end
  end
  local ok, err = pcall(GameConfiguration.UpdateEnabledMods)
  if not ok then table.insert(failures, tostring(err)) end
  compare(expected, 'native cleanup selection mismatch')
  if #failures > 0 then error(table.concat(failures, ';')) end
end
local original = snapshot()
audit('ORIGINAL', original)
local passed = {}
local function mark(label)
  table.insert(passed, label)
  print('TODO241 PASS ' .. label .. ' installed=' .. #ids)
end
local function force(id, value, dependencies)
  if value then nativeEnable(handles[id], dependencies) else nativeDisable(handles[id]) end
  if enabled(id) ~= value then
    local reason = value and nativeModding.CanEnableMod(handles[id], dependencies) or nativeModding.CanDisableMod(handles[id])
    error('native preparation failed: ' .. id .. ' requested=' .. tostring(value) .. ' actual=' .. tostring(enabled(id)) .. ' reason=' .. tostring(reason))
  end
end
local function expectFailure(fn, marker, expected)
  local ok, err = pcall(fn)
  local reset, resetError = resetApis()
  if not reset then error('API restore failed: ' .. resetError) end
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
  local probe = snapshot()
  local reason = nativeModding.CanEnableMod(handles[LEAF], false)
  nativeEnable(handles[LEAF], false)
  if enabled(MIDDLE) then error('EnableMod(false) enabled disabled intermediate') end
  if enabled(LEAF) then
    probe[LEAF] = true
  elseif reason ~= 'MissingDependencies' then
    error('native false refusal has unexpected reason: ' .. tostring(reason))
  end
  compare(probe, 'native false probe changed unrelated selection')
  print('TODO241 FALSE_PROBE leaf=' .. tostring(enabled(LEAF)) .. ' middle=' .. tostring(enabled(MIDDLE)) .. ' reason=' .. tostring(reason))
  mark('native-false-preserves-disabled-intermediate')

  -- The engine may refuse a child with a disabled prerequisite. Prepare a
  -- valid selection, then prove false can re-enable a child when ready.
  force(LEAF, false)
  force(MIDDLE, true, true)
  print('TODO241 FALSE_READY_PRECHECK reason=' .. tostring(nativeModding.CanEnableMod(handles[LEAF], false)) .. ' middle=' .. tostring(enabled(MIDDLE)))
  force(LEAF, true, false)
  print('TODO241 FALSE_READY leaf=' .. tostring(enabled(LEAF)) .. ' middle=' .. tostring(enabled(MIDDLE)))

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
  mark('disable-failure-restores-all')

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
  force(LEAF, true, true)
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
  local reset, resetError = resetApis()
  local rejected = not accepted and injected
    and string.find(tostring(err), 'restoration failed:', 1, true)
    and string.find(tostring(err), 'selection differs from original', 1, true)
    and not enabled(LEAF)
  restore(baseline)
  if not reset then error('API restore failed: ' .. resetError) end
  if not rejected then error('incomplete rollback was not reported: ' .. tostring(err)) end
  mark('incomplete-restoration-is-error')
end)
local reset, resetError = resetApis()
local cleaned, cleanupError = pcall(restore, original)
audit('FINAL', snapshot())
if nativeModding.EnableMod ~= nativeEnable or nativeModding.DisableMod ~= nativeDisable then
  error('native MOD API changed during diagnostic')
end
print('TODO241 ENGINE_API_UNCHANGED')
if not cleaned then error(tostring(failure) .. '; cleanup failed: ' .. tostring(cleanupError)) end
if not reset then error(tostring(failure) .. '; API restore failed: ' .. resetError) end
if not ok then error(failure) end
if #passed ~= 5 then error('incomplete diagnostic coverage') end
JAC_MOD_ROLLBACK_241 = {passed=true, rows=rows(snapshot())}
return 'TODO241 PASS 5/5 all-selection-restored installed=' .. #ids
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::{Lua, Table};

    fn snapshot_log(rows: &[(&str, bool)]) -> String {
        let mut text = rows
            .iter()
            .map(|(id, flag)| format!("FrontEnd: TODO241 ORIGINAL {id}={flag}\n"))
            .collect::<String>();
        text.push_str(&format!(
            "FrontEnd: TODO241 PASS native-false-preserves-disabled-intermediate installed={}\n",
            rows.len()
        ));
        text
    }

    #[test]
    fn recovery_parser_preserves_braced_ids_and_uses_only_diagnostic_counts() {
        let id = format!("{{{MAP}}}");
        let text = format!(
            "unrelated installed=99\n{}",
            snapshot_log(&[(&id, true), (GROWTH, false)])
        );
        let original = parse_original(&text).unwrap();
        assert_eq!(original.len(), 2);
        assert_eq!(original[&id], true);
        assert_eq!(original[GROWTH], false);
    }

    #[test]
    fn recovery_parser_rejects_partial_duplicate_invalid_and_mixed_runs() {
        let valid = snapshot_log(&[(MAP, true), (GROWTH, false)]);
        for bad in [
            valid.replace(&format!("FrontEnd: TODO241 ORIGINAL {GROWTH}=false\n"), ""),
            valid.replace(" installed=2", " installed=1"),
            valid.replace("=false", "=FALSE"),
            valid.replace(MAP, "gggggggg-0000-0000-0000-000000000000"),
            valid.replace(MAP, GROWTH),
            snapshot_log(&[(MAP, true), (&format!("{{{MAP}}}"), false)]),
            valid.replace("FrontEnd: TODO241 PASS ", "unrelated "),
            format!("{valid}{}", snapshot_log(&[(TREES, true)])),
            format!(
                "FrontEnd: TODO241 ORIGINAL {MAP}=true\nFrontEnd: diagnostic aborted\n{}",
                snapshot_log(&[(GROWTH, false)]).replace("installed=1", "installed=2")
            ),
            format!("{valid}FrontEnd: TODO241 PASS disable-failure-restores-all installed=3\n"),
        ] {
            assert!(
                parse_original(&bad).is_err(),
                "accepted incomplete or mixed snapshot: {bad}"
            );
        }
    }

    #[test]
    fn recovery_executes_native_installed_handles_and_preserves_unrecorded_mods() {
        for extra_enabled in [false, true] {
            let lua = fixture(31);
            lua.globals().set("EXTRA_ENABLED", extra_enabled).unwrap();
            let official = "{02a8bdde-67ea-4d38-9540-26e685e3156e}";
            lua.globals().set("OFFICIAL", official).unwrap();
            lua.load(r#"
                local info = {Id=OFFICIAL, Handle=OFFICIAL, Enabled=false, Dependencies={}, Official=true}
                MODS[OFFICIAL] = info; INSTALLED[#INSTALLED+1] = info
                local extra = {Id='deaddead-eeee-4ddd-aeee-000000000001', Handle='deaddead-eeee-4ddd-aeee-000000000001', Enabled=EXTRA_ENABLED, Dependencies={}}
                MODS[extra.Id] = extra; INSTALLED[#INSTALLED+1] = extra
                local lookup = Modding.GetModHandle
                Modding.GetModHandle = function(id) if id == OFFICIAL then return nil end; return lookup(id) end
                local native = Modding
                Modding = setmetatable({}, {__index=native, __newindex=function() error('native API is read only') end})
                ORIGINAL_LOOKUP = Modding.GetModHandle
            "#).exec().unwrap();
            let rows = [
                (MAP, true),
                (MIDDLE, false),
                (LEAF, false),
                (TREES, true),
                (GROWTH, false),
                (official, true),
            ];
            let data = recovery_scenario(&snapshot_log(&rows));
            assert_eq!(data["steps"].as_array().unwrap().len(), 38);
            let result = lua
                .load(data["steps"][1]["code"].as_str().unwrap())
                .eval::<String>()
                .unwrap();
            assert_eq!(result, "TODO241 RECOVERED original selection; installed=7");
            let mods: Table = lua.globals().get("MODS").unwrap();
            for (id, enabled) in rows {
                assert_eq!(
                    mods.get::<Table>(id)
                        .unwrap()
                        .get::<bool>("Enabled")
                        .unwrap(),
                    enabled
                );
            }
            assert_eq!(
                mods.get::<Table>("deaddead-eeee-4ddd-aeee-000000000001")
                    .unwrap()
                    .get::<bool>("Enabled")
                    .unwrap(),
                extra_enabled
            );
            assert!(lua
                .load("return Modding.GetModHandle == ORIGINAL_LOOKUP")
                .eval::<bool>()
                .unwrap());
        }
    }

    #[test]
    fn recovery_missing_original_mod_fails_before_any_selection_changes() {
        let lua = fixture(31);
        let data = recovery_scenario(&snapshot_log(&[(
            "02a8bdde-67ea-4d38-9540-26e685e3156e",
            true,
        )]));
        let err = lua
            .load(data["steps"][1]["code"].as_str().unwrap())
            .eval::<String>()
            .unwrap_err();
        assert!(err.to_string().contains("original MOD no longer installed"));
        assert_eq!(lua.globals().get::<Table>("ENABLED").unwrap().raw_len(), 0);
        assert_eq!(lua.globals().get::<Table>("DISABLED").unwrap().raw_len(), 0);
    }

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

    #[test]
    fn native_false_dependency_refusal_is_observed_and_valid_rollback_cases_continue() {
        let lua = fixture(0);
        lua.load(
            r#"
          local can, enable = Modding.CanEnableMod, Modding.EnableMod
          Modding.CanEnableMod = function(handle, dependencies)
            if dependencies == false then
              for _, id in ipairs(MODS[handle].Dependencies) do
                if not MODS[id].Enabled then return 'MissingDependencies' end
              end
            end
            return can(handle, dependencies)
          end
          Modding.EnableMod = function(handle, dependencies)
            if Modding.CanEnableMod(handle, dependencies) ~= 'OK' then return end
            return enable(handle, dependencies)
          end
        "#,
        )
        .exec()
        .unwrap();
        let result = lua.load(code()).eval::<String>().unwrap();
        assert!(result.starts_with("TODO241 PASS 5/5"));
        let mods: Table = lua.globals().get("MODS").unwrap();
        for id in [MAP, MIDDLE, LEAF, TREES, GROWTH] {
            let info: Table = mods.get(id).unwrap();
            assert!(!info.get::<bool>("Enabled").unwrap());
        }
    }

    #[test]
    fn restart_selection_is_compared_in_bounded_complete_chunks() {
        let data = scenario();
        let steps = data["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 37);
        assert_eq!(steps[2]["capture_as"], "MOD_COUNT");
        assert_eq!(steps[20]["equals_capture"], "MOD_COUNT");
        for batch in 0..16 {
            assert_eq!(
                steps[3 + batch]["capture_as"],
                steps[21 + batch]["equals_capture"]
            );
        }
        for mask in 0..32 {
            let lua = fixture(mask);
            lua.load(code()).eval::<String>().unwrap();
            let expected: String = lua.load(steps[3]["code"].as_str().unwrap()).eval().unwrap();
            let actual: String = lua
                .load(steps[21]["code"].as_str().unwrap())
                .eval()
                .unwrap();
            assert_eq!(actual, expected);
            assert!(actual.len() < 1600);
            for batch in 1..16 {
                let empty: String = lua
                    .load(steps[3 + batch]["code"].as_str().unwrap())
                    .eval()
                    .unwrap();
                assert_eq!(empty, "");
            }
        }
    }

    #[test]
    fn restart_chunks_preserve_every_id_and_flag_across_multiple_nonempty_batches() {
        let lua = fixture(31);
        lua.load(
            r#"
          for i = 1, 67 do
            local id = string.format('deaddead-eeee-4ddd-aeee-%012x', i)
            local row = {Id=id, Handle=id, Enabled=(i % 2 == 0), Dependencies={}}
            MODS[id] = row
            INSTALLED[#INSTALLED + 1] = row
          end
        "#,
        )
        .exec()
        .unwrap();
        lua.load(code()).eval::<String>().unwrap();
        let data = scenario();
        let steps = data["steps"].as_array().unwrap();
        let mut all = Vec::new();
        for batch in 0..16 {
            let before: String = lua
                .load(steps[3 + batch]["code"].as_str().unwrap())
                .eval()
                .unwrap();
            let after: String = lua
                .load(steps[21 + batch]["code"].as_str().unwrap())
                .eval()
                .unwrap();
            assert_eq!(before, after);
            assert!(before.len() < 1600);
            if !before.is_empty() {
                all.extend(before.split(',').map(str::to_owned));
            }
        }
        assert_eq!(all.len(), 72);
        let expected = lua.load("local out = {}; for _, row in ipairs(INSTALLED) do table.insert(out, row.Id .. '=' .. tostring(row.Enabled)) end; table.sort(out); return table.concat(out, ',')").eval::<String>().unwrap();
        assert_eq!(all.join(","), expected);
        lua.load("MODS['deaddead-eeee-4ddd-aeee-000000000043'].Enabled = true")
            .exec()
            .unwrap();
        let changed: String = lua
            .load(steps[23]["code"].as_str().unwrap())
            .eval()
            .unwrap();
        let saved: String = lua.load(steps[5]["code"].as_str().unwrap()).eval().unwrap();
        assert_ne!(changed, saved);
    }

    #[test]
    fn false_activation_refusal_stops_calibration_and_cleanup_preserves_the_original() {
        let lua = fixture(31);
        lua.load(
            r#"
          local can, enable = Modding.CanEnableMod, Modding.EnableMod
          Modding.CanEnableMod = function(handle, dependencies)
            if dependencies == false then
              for _, id in ipairs(MODS[handle].Dependencies) do
                if not MODS[id].Enabled then return 'MissingDependencies' end
              end
            end
            return can(handle, dependencies)
          end
          Modding.EnableMod = function(handle, dependencies)
            if dependencies == false then return end
            return enable(handle, dependencies)
          end
        "#,
        )
        .exec()
        .unwrap();
        let failure = lua.load(code()).eval::<String>().unwrap_err().to_string();
        assert!(failure.contains(&format!("native preparation failed: {LEAF}")));
        let mods: Table = lua.globals().get("MODS").unwrap();
        for id in [MAP, MIDDLE, LEAF, TREES, GROWTH] {
            let info: Table = mods.get(id).unwrap();
            assert!(info.get::<bool>("Enabled").unwrap());
        }
    }

    #[test]
    fn cleanup_preflight_errors_do_not_skip_remaining_restorations() {
        let lua = fixture(31);
        lua.globals().set("GROWTH_ID", GROWTH).unwrap();
        lua.globals().set("MIDDLE_ID", MIDDLE).unwrap();
        lua.load(
            r#"
          local can, enable = Modding.CanEnableMod, Modding.EnableMod
          local trap = false
          Modding.CanEnableMod = function(handle, dependencies)
            if dependencies == false then
              if trap and handle == GROWTH_ID then error('cleanup preflight refused') end
              for _, id in ipairs(MODS[handle].Dependencies) do
                if not MODS[id].Enabled then return 'MissingDependencies' end
              end
            end
            return can(handle, dependencies)
          end
          Modding.EnableMod = function(handle, dependencies)
            if dependencies == false then return end
            enable(handle, dependencies)
            if handle == MIDDLE_ID then
              MODS[GROWTH_ID].Enabled = false
              trap = true
            end
          end
        "#,
        )
        .exec()
        .unwrap();
        let failure = lua.load(code()).eval::<String>().unwrap_err().to_string();
        assert!(failure.contains("cleanup failed"));
        let mods: Table = lua.globals().get("MODS").unwrap();
        let leaf: Table = mods.get(LEAF).unwrap();
        assert!(leaf.get::<bool>("Enabled").unwrap());
        let growth: Table = mods.get(GROWTH).unwrap();
        assert!(!growth.get::<bool>("Enabled").unwrap());
    }

    #[test]
    fn read_only_native_api_is_preserved_while_real_operations_are_exercised() {
        let lua = fixture(0);
        lua.load("local api = Modding; Modding = setmetatable({}, {__index=api, __newindex=function() error('read only API') end}); ORIGINAL_ENABLE = Modding.EnableMod; ORIGINAL_DISABLE = Modding.DisableMod")
            .exec().unwrap();
        let result = lua.load(code()).eval::<String>().unwrap();
        assert!(result.starts_with("TODO241 PASS 5/5"));
        assert!(lua.load("return Modding.EnableMod == ORIGINAL_ENABLE and Modding.DisableMod == ORIGINAL_DISABLE").eval::<bool>().unwrap());
        let changes: Table = lua.globals().get("ENABLED").unwrap();
        assert!(changes.raw_len() > 0);
        let changes: Table = lua.globals().get("DISABLED").unwrap();
        assert!(changes.raw_len() > 0);
        let mods: Table = lua.globals().get("MODS").unwrap();
        for id in [MAP, MIDDLE, LEAF, TREES, GROWTH] {
            let info: Table = mods.get(id).unwrap();
            assert!(!info.get::<bool>("Enabled").unwrap());
        }
    }

    #[test]
    fn missing_native_api_is_rejected_before_mod_selection_changes() {
        for removed in [
            "Modding.EnableMod = nil",
            "Modding.DisableMod = nil",
            "GameConfiguration.UpdateEnabledMods = nil",
        ] {
            let lua = fixture(0);
            lua.load(removed).exec().unwrap();
            let failure = lua.load(code()).eval::<String>().unwrap_err().to_string();
            assert!(failure.contains("required native MOD API missing"));
            let changes: Table = lua.globals().get("ENABLED").unwrap();
            assert_eq!(changes.raw_len(), 0);
            let changes: Table = lua.globals().get("DISABLED").unwrap();
            assert_eq!(changes.raw_len(), 0);
        }
    }

    #[test]
    fn failed_api_restoration_still_attempts_native_selection_cleanup() {
        let lua = fixture(0);
        let fault = code().replace(
            "local Modding = setmetatable({}, {__index=nativeModding})",
            r#"local api, writes = {}, 0
          local Modding = setmetatable({}, {__index=function(_, key)
            return api[key] or nativeModding[key]
          end, __newindex=function(_, key, value)
            writes = writes + 1
            if writes >= 6 then error('restore assignment refused') end
            api[key] = value
          end})
        "#,
        );
        let failure = lua.load(fault).eval::<String>().unwrap_err().to_string();
        assert!(failure.contains("API restore failed"));
        let mods: Table = lua.globals().get("MODS").unwrap();
        for id in [MAP, MIDDLE, LEAF, TREES, GROWTH] {
            let info: Table = mods.get(id).unwrap();
            assert!(!info.get::<bool>("Enabled").unwrap());
        }
    }
}
