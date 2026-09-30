-- 朝廷・将軍の停戦裁定の画面（SSOT 3.2）。表示と請求の送信だけを行い、ゲームの状態は変えない。
-- 請求・応否は UI.RequestPlayerOperation(EXECUTE_SCRIPT) でゲームプレイ側の GameEvents に送る
-- （公式：BlackDeathScenario_PopupHandler.lua:22-28。UI のイベントはマルチプレイで同期しないため）。
include("InstanceManager")
include("JAC_CourtArbitration_Rules")
local R = JAC_ARB

local m_rows = InstanceManager:new("RowInstance", "Root", Controls.Rows)

local function Name(id)
	return Locale.Lookup(R.NameKey(id))
end

-- 自分の手番が始まっているか（公式：ActionPanel.lua:688）。ターン処理中（「お待ちください」）に送った
-- 要求はゲームプレイ側に届かなかった（実機 court-arbitration-03）ので、その間は送らない
local function TurnActive()
	local player = Players[Game.GetLocalPlayer()]
	return player ~= nil and player:IsTurnActive()
end

-- 送れたら true
local function Send(onStart, params)
	if not TurnActive() then
		print("[JAC_ARB] not sent (local turn not active): " .. onStart)
		return false
	end
	params.OnStart = onStart
	UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.EXECUTE_SCRIPT, params)
	return true
end

-- シナリオからも呼ぶ（tools/civ6-autotest/scenarios/court-arbitration.json）。引数名は動作済みの
-- 天下統一同盟（JAC_UnificationPact.lua の Target）にそろえる
function JAC_RequestArbitration(a, b)
	return Send("JAC_CourtArbitrationRequest", { Target = a, Other = b })
end

function JAC_RespondArbitration(accept)
	return Send("JAC_CourtArbitrationRespond", { Accept = accept and 1 or 0 })
end

function JAC_JoinSubjugation(target)
	return Send("JAC_CourtArbitrationJoin", { Target = target })
end

-- 請求できる組。自分が戦っている相手と、裁定の力を持つ将軍家なら他家どうしの戦争も
local function SortedIDs()
	local ids = {}
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do table.insert(ids, player:GetID()) end
	table.sort(ids)
	return ids
end

local function Candidates(localID, turn)
	local ids = SortedIDs()
	local shogun = R.IsEmpoweredShogun(localID, turn)
	local list = {}
	for i = 1, #ids do
		for j = i + 1, #ids do
			local a, b = ids[i], ids[j]
			local mine = a == localID or b == localID
			if (mine or shogun) and Players[a]:GetDiplomacy():IsAtWarWith(b) then
				if b == localID then a, b = b, a end
				table.insert(list, { a, b })
			end
		end
	end
	return list
end

local function AddRow(text)
	local row = m_rows:GetInstance()
	row.Text:SetText(text)
	row.Action2:SetHide(true)
	row.Action:SetHide(false)
	row.Action:SetDisabled(false)
	row.Action:SetToolTipString("")
	return row
end

local function Refresh()
	m_rows:ResetInstances()
	local localID = Game.GetLocalPlayer()
	if localID == -1 or Players[localID] == nil then
		Controls.OpenButton:SetHide(true)
		return
	end
	local player, turn = Players[localID], R.GameTurn()
	local gold, legitimacy = R.Costs(R.Power(localID, turn))
	local status = Locale.Lookup("LOC_JAC_ARB_STATUS", R.Legitimacy(localID), R.LegitimacyIncome(localID, turn),
		player:GetTreasury():GetGoldBalance(), gold, legitimacy)
		.. "[NEWLINE]" .. Locale.Lookup("LOC_JAC_ARB_LEGITIMACY_EFFECTS", R.LEGITIMACY_HIGH, R.LEGITIMACY_LOW)
	if R.IsEmpoweredShogun(localID, turn) then
		status = status .. "[NEWLINE]" .. Locale.Lookup("LOC_JAC_ARB_SHOGUN_POWER", R.MUROMACHI_END_YEAR)
	end
	Controls.Status:SetText(status)

	local count = 0
	local pending = Game:GetProperty(R.PENDING_KEY)
	local key = R.AnswerKey(pending, localID)
	if key and pending[key] == nil then
		local row = AddRow(Locale.Lookup("LOC_JAC_ARB_INCOMING", Name(pending.Requester), Name(pending.A), Name(pending.B)))
		row.Action:SetText(Locale.Lookup("LOC_JAC_ARB_ACCEPT"))
		row.Action:SetDisabled(not TurnActive())
		row.Action:RegisterCallback(Mouse.eLClick, function() JAC_RespondArbitration(true); Refresh() end)
		row.Action2:SetHide(false)
		row.Action2:SetText(Locale.Lookup("LOC_JAC_ARB_REFUSE"))
		row.Action2:SetDisabled(not TurnActive())
		row.Action2:RegisterCallback(Mouse.eLClick, function() JAC_RespondArbitration(false); Refresh() end)
		count = count + 1
	end
	for _, pair in ipairs(Candidates(localID, turn)) do
		local a, b = pair[1], pair[2]
		local row = AddRow(Locale.Lookup("LOC_JAC_ARB_PAIR", Name(a), Name(b)))
		row.Action:SetText(Locale.Lookup("LOC_JAC_ARB_REQUEST"))
		local ok, reason = R.CheckRequest(localID, a, b)
		row.Action:SetDisabled(not ok or not TurnActive())
		if not ok then row.Action:SetToolTipString(Locale.Lookup("LOC_JAC_ARB_REASON_" .. reason)) end
		row.Action:RegisterCallback(Mouse.eLClick, function() JAC_RequestArbitration(a, b); Refresh() end)
		count = count + 1
	end
	-- 討伐の勅命の一覧（対象・期限・参加者）と参加のボタン
	for _, target in ipairs(SortedIDs()) do
		if R.IsTarget(target, turn) then
			local names = {}
			for _, id in ipairs(SortedIDs()) do
				if R.HasJoined(id, target, turn) then table.insert(names, Name(id)) end
			end
			local row = AddRow(Locale.Lookup("LOC_JAC_ARB_ORDER_ROW", Name(target), Players[target]:GetProperty(R.OUTLAW_KEY),
				#names > 0 and table.concat(names, Locale.Lookup("LOC_JAC_ARB_LIST_SEPARATOR")) or Locale.Lookup("LOC_JAC_ARB_NONE")))
			local ok, reason = R.CheckJoin(localID, target, turn)
			row.Action:SetText(Locale.Lookup(R.HasJoined(localID, target, turn) and "LOC_JAC_ARB_JOINED" or "LOC_JAC_ARB_JOIN"))
			row.Action:SetHide(target == localID)
			row.Action:SetDisabled(not ok or not TurnActive())
			if not ok then row.Action:SetToolTipString(Locale.Lookup("LOC_JAC_ARB_REASON_" .. reason)) end
			row.Action:RegisterCallback(Mouse.eLClick, function() JAC_JoinSubjugation(target); Refresh() end)
			count = count + 1
		end
	end
	Controls.Rows:CalculateSize()
	Controls.Scroll:CalculateInternalSize()
	Controls.OpenButton:SetHide(count == 0)
	if count == 0 then Controls.Panel:SetHide(true) end
end

Controls.OpenButton:RegisterCallback(Mouse.eLClick, function()
	Controls.Panel:SetHide(not Controls.Panel:IsHidden())
	Refresh()
end)
Controls.CloseButton:RegisterCallback(Mouse.eLClick, function() Controls.Panel:SetHide(true) end)

Events.LocalPlayerTurnBegin.Add(Refresh)
Events.DiplomacyDeclareWar.Add(Refresh)
Events.DiplomacyMakePeace.Add(Refresh)
Events.LoadScreenClose.Add(Refresh)
Events.NotificationAdded.Add(Refresh) -- 請求の通知が届いたら応否の行を出す
