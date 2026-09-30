-- 暗黒期の政策の選択画面（documents/research/sengoku-ages.md §6.2・§7.2・§8）。台帳（Game プロパティ
-- JAC_AGES_LEDGER）を読むだけで、選択は UI.RequestPlayerOperation(EXECUTE_SCRIPT) でゲームプレイ側の
-- Scripts/JAC_Sengoku_Ages.lua に送る（公式：DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:21-28。
-- UI のイベントはマルチプレイで同期しない）。受け側が「暗黒期の家か・今の期か・未選択か・表にある政策か」を確かめる。
-- 画面の出し方・閉じ方は公式の DLC/Expansion2/UI/Additions/DisloyalCityChooser.lua（ContextPtr:SetHide、OnClose）。
-- 改元の画面を閉じたときに1回だけ出し、そのターンの改元の通知から開き直す（SSOT 3.2）。
-- 終了ブロックは JAC_ActionPanel_Sengoku.lua が同期済み台帳から判定する。
include("InstanceManager")
include("JAC_SengokuAgesView")

local LEDGER_KEY = "JAC_AGES_LEDGER"
local m_Rows = InstanceManager:new("PolicyRow", "Root", Controls.Rows)
local m_ShownReign = -1
JAC_DARK_POLICY_SHOW_COUNT = 0 -- 実機シナリオ：自動表示と通知からの再表示の回数

local POLICIES = {}
local POLICY_BY_TYPE = {}
for row in GameInfo.JAC_SengokuDarkAgePolicies() do
	table.insert(POLICIES, row)
	POLICY_BY_TYPE[row.PolicyType] = row
end
table.sort(POLICIES, function(a, b) return a.Priority < b.Priority end)

local function Ledger()
	local l = Game:GetProperty(LEDGER_KEY)
	return type(l) == "table" and l or nil
end

-- 保存された改元の判定と同じターンだけ選べる。読み込みで受付期間を延ばさない。
local function PendingHouse()
	local id = Game.GetLocalPlayer()
	local player = Players[id]
	if player == nil or not player:IsHuman() then return nil end
	return JAC_AgesView_PendingDarkPolicy(Ledger(), id)
end

-- 自分の手番が始まっているか（公式：ActionPanel.lua:688）。ターン処理中に送った要求は届かない
-- （実機 court-arbitration-03）
local function TurnActive()
	local player = Players[Game.GetLocalPlayer()]
	return player ~= nil and player:IsTurnActive()
end

function OnClose()
	ContextPtr:SetHide(true)
end

-- 選ぶボタンとシナリオ（tools/civ6-autotest/scenarios/sengoku-dark-policy.json）から呼ぶ。送れたら true。
-- 手番の前（読み込み直後など）は送らず、画面を開いたままにする（ボタンは押し直せる）
function JAC_ChooseDarkPolicy(policyType)
	local l = Ledger()
	if l == nil or PendingHouse() == nil or POLICY_BY_TYPE[policyType] == nil or not TurnActive() then return false end
	UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.EXECUTE_SCRIPT,
		{ OnStart = "JAC_SengokuDarkPolicyChoose", PolicyType = policyType, Reign = l.reign })
	OnClose()
	return true
end

-- 改元の画面（UI/JAC_SengokuReignPopup.lua）が出ている間は出さず、閉じたら出す（下の LuaEvents）。
local m_ReignPopupOpen = false

-- automatic = true は改元の画面を閉じたときだけ。通知からは同じターン中に開き直せる。
local function ShowChoice(automatic)
	if m_ReignPopupOpen then return false end
	local house = PendingHouse()
	if house == nil then
		OnClose()
		return false
	end
	local l = Ledger()
	if automatic and m_ShownReign == l.reign then return false end
	if not ContextPtr:IsHidden() then return true end
	m_Rows:ResetInstances()
	local current = POLICY_BY_TYPE[house.policy]
	Controls.Body:SetText(Locale.Lookup("LOC_JAC_AGE_DARK_CHOICE_BODY", current and Locale.Lookup(current.Name) or ""))
	for _, row in ipairs(POLICIES) do
		local instance = m_Rows:GetInstance()
		instance.Name:SetText(Locale.Lookup(row.Name))
		instance.Description:SetText(Locale.Lookup(row.Description))
		instance.Choose:RegisterCallback(Mouse.eLClick, function() JAC_ChooseDarkPolicy(row.PolicyType) end)
	end
	Controls.Rows:CalculateSize()
	Controls.Panel:DoAutoSize()
	m_ShownReign = l.reign
	JAC_DARK_POLICY_SHOW_COUNT = JAC_DARK_POLICY_SHOW_COUNT + 1
	ContextPtr:SetHide(false)
	return true
end

-- 通知は閉じる操作では消えない。要求がゲームプレイで受理された後にだけ、同期済み台帳で解除する。
-- 公式の UI 消去：DLC/Expansion2/UI/Replacements/NotificationPanel_Expansion2.lua:43。
local function RefreshNotification()
	local id = Game.GetLocalPlayer()
	if id == nil or id < 0 then OnClose(); return end
	local l, pending = Ledger(), PendingHouse()
	-- Scan every ID so an older/official notification cannot hide our blocker from cleanup.
	-- Official: DLC/PiratesScenario/UI/Replacements/NotificationPanel_PiratesScenario.lua:74-84.
	for _, notificationID in pairs(NotificationManager.GetList(id) or {}) do
		local notification = NotificationManager.Find(id, notificationID)
		if JAC_AgesView_DarkPolicyNotification(notification) and not notification:IsDismissed()
			and (pending == nil or notification:GetValue("JAC_DARK_POLICY_REIGN") ~= l.reign) then
			NotificationManager.Dismiss(id, notificationID)
		end
	end
	if pending == nil then OnClose() end
end

ContextPtr:SetHide(true)
local savedLedger = Ledger()
if savedLedger then m_ShownReign = savedLedger.reign end -- 同じ改元のセーブを開いても自動表示を繰り返さない
Controls.LaterButton:RegisterCallback(Mouse.eLClick, OnClose)
Events.LocalPlayerTurnBegin.Add(RefreshNotification) -- 自動表示は改元の画面を閉じたときだけ
Events.LocalPlayerTurnEnd.Add(OnClose)  -- Base/Assets/UI/ActionPanel.lua:1335。手番外では送れないので閉じる
Events.LoadScreenClose.Add(RefreshNotification) -- 保存した通知から再開する。画面は自動で出さない
Events.GameCoreEventPublishComplete.Add(RefreshNotification) -- Base/Assets/UI/Choosers/ResearchChooser.lua:485
LuaEvents.JAC_SengokuDarkPolicy_Open.Add(function(reign)
	local l = Ledger()
	if l and reign == l.reign and TurnActive() then ShowChoice(false) end
end)
LuaEvents.JAC_SengokuReignPopup_Shown.Add(function()
	m_ReignPopupOpen = true
	OnClose()
end)
LuaEvents.JAC_SengokuReignPopup_Closed.Add(function(showChoice)
	m_ReignPopupOpen = false
	if showChoice then ShowChoice(true) end
end)
