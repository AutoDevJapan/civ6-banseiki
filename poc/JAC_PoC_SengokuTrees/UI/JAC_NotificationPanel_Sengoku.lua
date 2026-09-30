-- 公式の NotificationPanel.lua:1896 の wildcard から読み込まれる追加スクリプト。
-- ここで公式パネルを include すると自分自身も再読込される（実機 dark-policy-at-reign-01/02）。
-- 公式の NotificationPanel_Expansion1.lua:3-6 と同じく、登録関数だけを包む。
include("JAC_SengokuAgesView")

local BASE_RegisterHandlers = RegisterHandlers

function RegisterHandlers()
	BASE_RegisterHandlers()
	local handler = g_notificationHandlers[NotificationTypes.USER_DEFINED_2]
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
