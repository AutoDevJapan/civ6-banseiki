
OFFICIAL_ACTIVATIONS = 0
g_notificationHandlers = {}
function GetActiveNotificationFromEntry(entry, notificationID)
  return NotificationManager.Find(entry.m_PlayerID, notificationID or entry.id)
end
function RegisterHandlers()
  g_notificationHandlers[NotificationTypes.COMMEMORATION_AVAILABLE] = {
    Activate = function() OFFICIAL_ACTIVATIONS = OFFICIAL_ACTIVATIONS + 1 end,
    TryDismiss = function(entry) NotificationManager.Dismiss(entry.m_PlayerID, entry.id) end,
  }
  g_notificationHandlers[NotificationTypes.USER_DEFINED_3] = {
    Activate = function() OFFICIAL_ACTIVATIONS = OFFICIAL_ACTIVATIONS + 1 end,
    TryDismiss = function(entry) NotificationManager.Dismiss(entry.m_PlayerID, entry.id) end,
  }
end
function Entry(n) return { m_PlayerID = n:GetPlayerID(), id = n:GetID() } end
function OnNotificationActivated(playerID, notificationID, activatedByUser)
  local n = NotificationManager.Find(playerID, notificationID)
  g_notificationHandlers[n:GetType()].Activate(Entry(n), notificationID, activatedByUser)
end
LuaEvents.ActionPanel_ActivateNotification.Add(function(n)
  OnNotificationActivated(n:GetPlayerID(), n:GetID(), true)
end)
