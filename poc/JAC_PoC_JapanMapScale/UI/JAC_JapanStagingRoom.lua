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
