-- 公式の通知パネルを読んだ後、このMODの印のある改元の通知だけを政策選択へつなぐ。
-- Base/Assets/UI/Panels/NotificationPanel.lua:1896 の wildcard で公式の拡張も読み込まれる。
-- RegisterHandlers は LateInitialize (:1854) で呼ばれるので、公式のハンドラー登録の後で包める。
include("NotificationPanel")
include("JAC_SengokuAgesView")

local BASE_RegisterHandlers = RegisterHandlers

function RegisterHandlers()
	BASE_RegisterHandlers()
	local handler = g_notificationHandlers[NotificationTypes.COMMEMORATION_AVAILABLE]
	local activate, dismiss = handler.Activate, handler.TryDismiss
	handler.Activate = function(entry, notificationID, activatedByUser)
		local notification = GetActiveNotificationFromEntry(entry, notificationID)
		if not JAC_AgesView_DarkPolicyNotification(notification) then
			return activate(entry, notificationID, activatedByUser)
		end
		if notification:GetPlayerID() ~= Game.GetLocalPlayer() or activatedByUser == false then return end
		local l = JAC_AgesView_Ledger()
		local reign = notification:GetValue("JAC_DARK_POLICY_REIGN")
		if l and reign == l.reign and JAC_AgesView_PendingDarkPolicy(l, Game.GetLocalPlayer()) then
			LuaEvents.JAC_SengokuReignPopup_CloseForPolicy()
			LuaEvents.JAC_SengokuDarkPolicy_Open(reign)
		end
	end
	-- 公式の CanUserDismiss/GetEndTurnBlocking の判定に加え、MODの未選択通知を右クリックで消させない。
	-- Base/Assets/UI/Panels/NotificationPanel.lua:1082-1094。通常の公式通知は元の処理へ渡す。
	handler.TryDismiss = function(entry)
		local notification = GetActiveNotificationFromEntry(entry)
		if JAC_AgesView_DarkPolicyNotification(notification)
			and JAC_AgesView_PendingDarkPolicy(JAC_AgesView_Ledger(), notification:GetPlayerID()) then return end
		return dismiss(entry)
	end
end
