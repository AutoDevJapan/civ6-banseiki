
BASE_ERA_CALLS = 0
function RealizeEraIndicator()
  BASE_ERA_CALLS = BASE_ERA_CALLS + 1
  Controls.EraToolTipArea1:SetToolTipString("LOC_GAME_ERA_DESC")
  Controls.EraToolTipArea2:SetToolTipString("LOC_GAME_ERA_DESC")
end
END_TURN_REQUESTS, AUTO_END_TURN_REQUESTS, BASE_REFRESHES = 0, 0, 0
EXPANSION_ALLOWS_AUTO_END = true
function DoEndTurn(optionalNewBlocker) END_TURN_REQUESTS = END_TURN_REQUESTS + 1; LAST_BLOCKER = optionalNewBlocker end
function AllowAutoEndTurn_Expansion() return EXPANSION_ALLOWS_AUTO_END end
function CheckAutoEndTurn()
  if AllowAutoEndTurn_Expansion() then AUTO_END_TURN_REQUESTS = AUTO_END_TURN_REQUESTS + 1 end
end
function OnRefresh()
  BASE_REFRESHES = BASE_REFRESHES + 1
  Controls.EndTurnText:SetText("NEXT TURN")
  Controls.EndTurnButton:SetToolTipString("NEXT TURN")
end
function OnEndTurnClicked() DoEndTurn() end
function OnInputActionTriggered(action) if action == "END_TURN" then DoEndTurn() end end
BASE_INPUTS, FORCED_END_TURN_REQUESTS = 0, 0
function OnInputHandler(input)
  BASE_INPUTS = BASE_INPUTS + 1
  if input:GetMessageType() == KeyEvents.KeyUp and input:GetKey() == Keys.VK_RETURN then
    if input:IsShiftDown() then FORCED_END_TURN_REQUESTS = FORCED_END_TURN_REQUESTS + 1
    else DoEndTurn() end
    return true
  end
  return false
end
function KEY_INPUT(key, shift, message)
  return {GetMessageType = function() return message or KeyEvents.KeyUp end,
          GetKey = function() return key end, IsShiftDown = function() return shift end}
end
ContextPtr:SetRefreshHandler(OnRefresh)
ContextPtr:SetInputHandler(OnInputHandler, true)
