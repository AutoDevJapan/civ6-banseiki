-- Frontend does not accept the gameplay map criteria. Keep the installed UI,
-- and change only its two twelve-slot clamps in this Lua context.
include("JAC_JapanHotseatLimits")
local originalMath = math
math = setmetatable({}, { __index = originalMath })
math.min = function(first, second, ...)
  if first == 22 and second == 12 and select('#', ...) == 0
    and JAC_JapanHotseatMaxPlayers(second) == 22 then
    return 22
  end
  return originalMath.min(first, second, ...)
end
include("StagingRoom")