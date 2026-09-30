-- 右下の時代の目盛り（ActionPanel の EraIndicator）の説明を、公式の時代の名前（戦国パックではツリーの段階の名前）から
-- 今の元号の時代の名前と次の改元までのターンに替える（オーナー判断 L-1、2026-09-30：「XX era」と出る所を元号にする）。
-- Gathering Storm は ActionPanel を ReplaceUIScript で ActionPanel_Expansion2 に差し替えている（Expansion2.modinfo:366-371）ので、
-- それを読んでから RealizeEraIndicator（ActionPanel_Expansion1.lua:187-216。OnRefresh :181 がグローバル名で呼ぶ）を包む。
-- 目盛りの向き（公式の時代の段階）は変えず、説明の文だけを差し替える。台帳を読むだけで書かない。
include("ActionPanel_Expansion2")
include("JAC_SengokuAgesView")

local BASE_RealizeEraIndicator = RealizeEraIndicator

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
