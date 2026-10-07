-- Frontend does not accept the gameplay map criteria. Keep the installed UI,
-- and change only its two twelve-slot clamps in this Lua context.
include("JAC_JapanHotseatLimits")
local originalMath = math
math = setmetatable({}, { __index = originalMath })
math.min = function(...)
  if select('#', ...) == 2 then
    local first, second = ...
    if first == 22 and second == 12
      and JAC_JapanHotseatMaxPlayers(second) == 22 then
      return 22
    end
  end
  return originalMath.min(...)
end
include("StagingRoom")
