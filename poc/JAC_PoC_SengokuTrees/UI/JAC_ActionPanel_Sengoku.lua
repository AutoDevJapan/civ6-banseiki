-- 右下の時代の目盛り（ActionPanel の EraIndicator）の説明を、公式の時代の名前（戦国パックではツリーの段階の名前）から
-- 今の元号の時代の名前と次の改元までのターンに替える（オーナー判断 L-1、2026-09-30：「XX era」と出る所を元号にする）。
-- Gathering Storm は ActionPanel を ReplaceUIScript で ActionPanel_Expansion2 に差し替えている（Expansion2.modinfo:366-371）ので、
-- それを読んでから RealizeEraIndicator（ActionPanel_Expansion1.lua:187-216。OnRefresh :181 がグローバル名で呼ぶ）を包む。
-- 目盛りの向き（公式の時代の段階）は変えず、説明の文だけを差し替える。台帳を読むだけで書かない。
include("ActionPanel_Expansion2")
include("JAC_SengokuAgesView")

local BASE_RealizeEraIndicator = RealizeEraIndicator
local BASE_DoEndTurn = DoEndTurn
local BASE_OnInputHandler = OnInputHandler
local BASE_AllowAutoEndTurn = AllowAutoEndTurn_Expansion
local BASE_OnRefresh = OnRefresh

-- 通知の有無ではなく、保存された判定ターンと選択済みの期で手番の終了を止める。
-- ボタン・Enter・通知の追加ボタンは公式 ActionPanel.lua:250,297,605,1298 の DoEndTurn を通る。
function JAC_IsDarkPolicyEndTurnBlocked()
	return JAC_AgesView_PendingDarkPolicy(JAC_AgesView_Ledger(), Game.GetLocalPlayer()) ~= nil
end

-- 公式の Shift+Enter は DoEndTurn を通さず直接終了する（ActionPanel.lua:1093-1106）。
function OnInputHandler(input)
	if JAC_IsDarkPolicyEndTurnBlocked() and input:GetMessageType() == KeyEvents.KeyUp
		and input:GetKey() == Keys.VK_RETURN then
		DoEndTurn()
		return true
	end
	return BASE_OnInputHandler(input)
end

function DoEndTurn(optionalNewBlocker)
	if JAC_IsDarkPolicyEndTurnBlocked() then
		local player = Players[Game.GetLocalPlayer()]
		if player and player:IsTurnActive() then
			LuaEvents.JAC_SengokuReignPopup_CloseForPolicy()
			LuaEvents.JAC_SengokuDarkPolicy_Open(JAC_AgesView_Ledger().reign)
		end
		return
	end
	return BASE_DoEndTurn(optionalNewBlocker)
end

-- 自動終了も止める。公式 ActionPanel.lua:497-520 の拡張用フック。
function AllowAutoEndTurn_Expansion()
	return not JAC_IsDarkPolicyEndTurnBlocked() and BASE_AllowAutoEndTurn()
end

function OnRefresh()
	BASE_OnRefresh()
	local player = Players[Game.GetLocalPlayer()]
	if not JAC_IsDarkPolicyEndTurnBlocked() or not player or not player:IsTurnActive() then return end
	Controls.EndTurnText:SetText(Locale.Lookup("LOC_JAC_AGE_DARK_CHOICE_TITLE"))
	Controls.EndTurnButton:SetToolTipString(Locale.Lookup("LOC_JAC_AGE_DARK_CHOICE_REQUIRED"))
	Controls.CurrentTurnBlockerIcon:SetHide(false)
	Controls.CurrentTurnBlockerIcon:SetIcon("ICON_NOTIFICATION_USER_DEFINED_3")
	Controls.CountImage:SetHide(true)
end

-- 公式 Initialize は include 中に元の関数を登録する（ActionPanel.lua:1368）。包んだ関数を登録し直す。
ContextPtr:SetRefreshHandler(OnRefresh)
ContextPtr:SetInputHandler(OnInputHandler, true)
Events.GameCoreEventPublishComplete.Add(function() ContextPtr:RequestRefresh() end)
Events.LoadScreenClose.Add(function() ContextPtr:RequestRefresh() end)

-- 自動テストが確かめる、最後に出した説明。
JAC_ERA_INDICATOR_TEXT = nil

function RealizeEraIndicator()
	BASE_RealizeEraIndicator()
	local text = JAC_AgesView_EraIndicatorText(JAC_AgesView_Ledger(), JAC_AgesView_ElapsedTurn())
	if text == nil then return end
	JAC_ERA_INDICATOR_TEXT = text
	Controls.EraToolTipArea1:SetToolTipString(text)
	Controls.EraToolTipArea2:SetToolTipString(text)
end

-- 改元のターンに、手番の始まりで説明を新しくする（OnRefresh は別の出来事で呼ばれるため）。
Events.LocalPlayerTurnBegin.Add(function() RealizeEraIndicator() end)
