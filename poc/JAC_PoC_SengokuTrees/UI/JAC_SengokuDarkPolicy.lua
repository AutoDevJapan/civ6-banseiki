-- 暗黒期の政策の選択画面（documents/research/sengoku-ages.md §6.2・§7.2・§8）。台帳（Game プロパティ
-- JAC_AGES_LEDGER）を読むだけで、選択は UI.RequestPlayerOperation(EXECUTE_SCRIPT) でゲームプレイ側の
-- Scripts/JAC_Sengoku_Ages.lua に送る（公式：DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:21-28。
-- UI のイベントはマルチプレイで同期しない）。受け側が「暗黒期の家か・今の期か・未選択か・表にある政策か」を確かめる。
-- 画面の出し方・閉じ方は公式の DLC/Expansion2/UI/Additions/DisloyalCityChooser.lua（ContextPtr:SetHide、OnClose）。
-- 入力は奪わない（自動テストの autoplay を止めない）。自動テストは OnClose() で閉じる（scenario.py の POPUP_CLOSERS）。
include("InstanceManager")

local LEDGER_KEY = "JAC_AGES_LEDGER"
local m_Rows = InstanceManager:new("PolicyRow", "Root", Controls.Rows)

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

-- 自分の家が今の期の選択を待っているなら、その家
local function PendingHouse()
	local id = Game.GetLocalPlayer()
	local l = id ~= -1 and Ledger() or nil
	local house = l and l.houses["p" .. tostring(id)]
	if house and house.age == "DARK" and house.chosen ~= l.reign then return house end
	return nil
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
	if l == nil or not TurnActive() then return false end
	UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.EXECUTE_SCRIPT,
		{ OnStart = "JAC_SengokuDarkPolicyChoose", PolicyType = policyType, Reign = l.reign })
	OnClose()
	return true
end

-- 改元の画面（UI/JAC_SengokuReignPopup.lua）が出ている間は出さず、閉じたら出す（下の LuaEvents）。
local m_ReignPopupOpen = false

-- 選択待ちの間、自分の手番の始まり（と読み込み）ごとに出す。「あとで」と手番の終わりは次の手番まで閉じるだけ（Q72）
local function Refresh()
	if m_ReignPopupOpen then return end
	local house = PendingHouse()
	if house == nil then
		OnClose()
		return
	end
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
	ContextPtr:SetHide(false)
end

ContextPtr:SetHide(true)
Controls.LaterButton:RegisterCallback(Mouse.eLClick, OnClose)
Events.LocalPlayerTurnBegin.Add(Refresh) -- Base/Assets/UI/ActionPanel.lua:1334
Events.LocalPlayerTurnEnd.Add(OnClose)  -- Base/Assets/UI/ActionPanel.lua:1335。手番外では送れないので閉じる
Events.LoadScreenClose.Add(Refresh)      -- Base/Assets/UI/Popups/PlayerChange.lua:298
LuaEvents.JAC_SengokuReignPopup_Shown.Add(function()
	m_ReignPopupOpen = true
	OnClose()
end)
LuaEvents.JAC_SengokuReignPopup_Closed.Add(function(showChoice)
	m_ReignPopupOpen = false
	if showChoice then Refresh() end
end)
