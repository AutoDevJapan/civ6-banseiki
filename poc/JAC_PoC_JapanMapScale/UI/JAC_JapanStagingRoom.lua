-- Frontend does not accept the gameplay map criteria. Keep the installed UI,
-- and change only its two twelve-slot clamps in this Lua context.
include("JAC_JapanHotseatLimits")
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
      local limit = JAC_JapanHotseatMaxPlayers(second)
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
