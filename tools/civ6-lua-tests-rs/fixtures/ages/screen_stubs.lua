
local files = ...
-- A UI context: its own globals (ContextPtr, Controls, functions, the wrapped Locale) over the shared ones, as live.
-- include loads the named file from the mods into the context (a wildcard include loads every file with the prefix).
function NEW_CONTEXT(name, ...)
  local env = setmetatable({}, { __index = _G })
  env.ContextPtr = CONTROL()
  env.Controls = setmetatable({}, { __index = function(t, k) local c = CONTROL(); rawset(t, k, c); return c end })
  env.include = function(file, wildcard)
    if file == "InstanceManager" then return end
    local found = false
    for fileName, source in pairs(files) do
      if fileName == file or (wildcard and string.sub(fileName, 1, #file) == file) then
        found = true
        assert(load(source, fileName, "t", env))()
      end
    end
    assert(found or wildcard, "include " .. file)
  end
  for _, source in ipairs({ ... }) do assert(load(source, name, "t", env))() end
  return env
end
Locale = {
  Lookup = function(key, ...)
    local args = { ... }
    if #args == 0 then return key end
    for i, v in ipairs(args) do args[i] = tostring(v) end
    return key .. "(" .. table.concat(args, ",") .. ")"
  end,
  ToUpper = function(s) return "UP:" .. s end,
}
Keys = { VK_ESCAPE = 27, VK_RETURN = 13 }
KeyEvents = { KeyUp = 2 }
UI.GetPlayerColors = function() return 1, 2 end
UI.PlaySound = function() end
for _, config in pairs(PlayerConfigurations) do config.GetLeaderTypeName = function() return "LEADER_X" end end
function AUTOPLAY_END() LuaEvents.AutoPlayEnd() end
