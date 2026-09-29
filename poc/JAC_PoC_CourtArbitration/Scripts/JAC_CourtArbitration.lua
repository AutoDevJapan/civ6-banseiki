-- 朝廷・将軍の停戦裁定（SSOT 3.2）。日本マップ4種でだけ読み込まれる（modinfo）。
-- 1. 戦争中の当事者は相手との停戦裁定を請求できる。足利将軍家は、人間・AI を問わず、室町幕府の終わり（1573年）
--    までは他家どうしの停戦も発議でき、その裁定の力（停戦の再宣戦制限のターン数）は3倍になる。
-- 2. 請求者は外交的支持・金・正当性を払う。請求は UI から EXECUTE_SCRIPT で届き、次のターンの初めに裁く。
-- 3. 答える側：人間は画面で応否を送る（届かなければ拒否）。AI は Rules の暫定規則で決める。
-- 4. 全員が応じれば講和し、停戦中に戦争になっていれば毎ターンの初めに講和し直す。拒んだ勢力は正当性を失い、
--    ほかの家に「討伐の大義」が立つ（記録と通知のみ。エンジンに特定の相手への開戦事由を足す API が無い：Q58）。
--
-- マルチプレイ：状態はすべて Game / Player プロパティ（セーブに残り、全員で共有）。変更は GameEvents
-- （EXECUTE_SCRIPT と OnGameTurnStarted）の中だけで、乱数もローカルプレイヤーの判定も使わない。
include "JAC_CourtArbitration_Rules"
local R = JAC_ARB

local function Log(message)
	print("[JAC_ARB] " .. message)
end

local function SortedMajorIDs()
	local ids = {}
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do table.insert(ids, player:GetID()) end
	table.sort(ids)
	return ids
end

-- 各マシンが自分の言語で表示する（ゲームプレイ側の Locale.Lookup：PiratesScenario_GameCore_Script.lua:136-137）
local function Notify(ids, nameKey, summaryKey, ...)
	local names = {}
	for i, value in ipairs({...}) do names[i] = type(value) == "number" and value or Locale.Lookup(value) end
	local data = {
		[ParameterTypes.MESSAGE] = Locale.Lookup(nameKey),
		[ParameterTypes.SUMMARY] = Locale.Lookup(summaryKey, unpack(names)) -- Civ6 は Lua 5.1,
	}
	for _, id in ipairs(ids) do
		NotificationManager.SendNotification(id, NotificationTypes.USER_DEFINED_1, data)
	end
end

-- 請求（UI/JAC_CourtArbitration.lua → UI.RequestPlayerOperation(EXECUTE_SCRIPT)、OnStart = この名前。
-- 公式：BlackDeathScenario_PopupHandler.lua:22-28 → BlackDeathScenario.lua:66 の GameEvents.EventPopupChoice）
local function OnRequest(requester, params)
	local a, b = tonumber(params.A), tonumber(params.B)
	local ok, reason = R.CheckRequest(requester, a, b)
	if not ok then
		Log(string.format("rejected request player=%d pair=%s-%s reason=%s", requester, tostring(a), tostring(b), reason))
		return
	end
	if requester == b then a, b = b, a end -- 当事者の請求では A を請求者にそろえる
	local player = Players[requester]
	-- ChangeFavor は公式 Lua に使用例が無い（GS の DLL に名前がある）。失敗したら何も払わせずに断る
	local paid, err = pcall(function() player:ChangeFavor(-R.COST_FAVOR) end)
	if not paid then
		Log(string.format("FAILED favor payment player=%d: %s", requester, tostring(err)))
		return
	end
	player:GetTreasury():ChangeGoldBalance(-R.COST_GOLD) -- AustraliaScenario.lua:847
	player:SetProperty(R.LEGITIMACY_KEY, R.Legitimacy(requester) - R.COST_LEGITIMACY)
	local turn = R.GameTurn()
	local power = R.IsEmpoweredShogun(requester, turn) and R.SHOGUN_POWER or 1
	local pending = { Requester = requester, A = a, B = b, Turn = turn, Power = power }
	Game:SetProperty(R.PENDING_KEY, pending)
	Game:SetProperty(R.LAST_REQUEST_KEY, turn)
	Log(string.format("request player=%d pair=%d-%d turn=%d power=%d", requester, a, b, turn, power))
	local respondents = {}
	for _, id in ipairs({ a, b }) do
		if R.AnswerKey(pending, id) then table.insert(respondents, id) end
	end
	Notify(respondents, "LOC_JAC_ARB_NOTIFY_REQUEST_NAME", "LOC_JAC_ARB_NOTIFY_REQUEST_SUMMARY", R.NameKey(requester), R.NameKey(a), R.NameKey(b))
end

-- 人間の応否（同じく EXECUTE_SCRIPT。params.Accept = 1 / 0）。答える立場で、まだ答えていないときだけ記録する
local function OnRespond(playerID, params)
	local pending = Game:GetProperty(R.PENDING_KEY)
	local key = R.AnswerKey(pending, playerID)
	if key == nil or pending[key] ~= nil then
		Log(string.format("ignored answer player=%d", playerID))
		return
	end
	pending[key] = tonumber(params.Accept) == 1
	Game:SetProperty(R.PENDING_KEY, pending)
	Log(string.format("answer player=%d accept=%s", playerID, tostring(pending[key])))
end

local function MakePeace(a, b)
	-- 公式：PiratesScenario_StartScript.lua:767
	local ok, err = pcall(function() Players[a]:GetDiplomacy():MakePeaceWith(b) end)
	if not ok then Log(string.format("FAILED peace %d-%d: %s", a, b, tostring(err))) end
	return ok and not Players[a]:GetDiplomacy():IsAtWarWith(b)
end

-- 停戦中の組が戦争になっていたら講和し直す。宣戦した側はゲームプレイ側で分からない
-- （宣戦のイベント Events.DiplomacyDeclareWar は UI 側の使用例しか無い）ため、違反の不利益は付けない
local function EnforceTruces(turn)
	local ids = SortedMajorIDs()
	for i = 1, #ids do
		for j = i + 1, #ids do
			local a, b = ids[i], ids[j]
			local key = R.TruceKey(a, b)
			local untilTurn = Game:GetProperty(key)
			if untilTurn ~= nil then
				if turn > untilTurn then
					Game:SetProperty(key, nil)
					Log(string.format("truce ended pair=%d-%d turn=%d", a, b, turn))
				elseif Players[a]:GetDiplomacy():IsAtWarWith(b) then
					Log(string.format("truce reimposed pair=%d-%d turn=%d until=%d ok=%s", a, b, turn, untilTurn,
						tostring(MakePeace(a, b))))
				end
			end
		end
	end
end

local function Resolve(turn)
	local pending = Game:GetProperty(R.PENDING_KEY)
	if pending == nil then return end
	Game:SetProperty(R.PENDING_KEY, nil)
	local a, b = pending.A, pending.B
	if not (Players[a]:IsAlive() and Players[b]:IsAlive() and Players[a]:GetDiplomacy():IsAtWarWith(b)) then
		Log(string.format("void request pair=%d-%d turn=%d (no longer at war)", a, b, turn))
		return
	end
	local refusers = {}
	for _, r in ipairs({ { a, b }, { b, a } }) do
		local id, opponent = r[1], r[2]
		local key = R.AnswerKey(pending, id)
		if key then
			local accept = pending[key]
			local how = "answer"
			if accept == nil then
				if Players[id]:IsHuman() then
					accept, how = false, "no_answer"
				else
					accept, how = R.AiAccepts(id, opponent), "ai"
				end
			end
			Log(string.format("decision player=%d accept=%s by=%s", id, tostring(accept), how))
			if not accept then table.insert(refusers, id) end
		end
	end
	local everyone = SortedMajorIDs()
	if #refusers == 0 then
		local untilTurn = turn + R.TRUCE_TURNS * pending.Power
		local peace = MakePeace(a, b)
		if peace then Game:SetProperty(R.TruceKey(a, b), untilTurn) end
		Log(string.format("accepted pair=%d-%d turn=%d until=%d power=%d peace=%s", a, b, turn, untilTurn,
			pending.Power, tostring(peace)))
		Notify(everyone, "LOC_JAC_ARB_NOTIFY_ACCEPTED_NAME", "LOC_JAC_ARB_NOTIFY_ACCEPTED_SUMMARY", R.NameKey(a), R.NameKey(b), untilTurn - turn)
	else
		for _, id in ipairs(refusers) do
			local legitimacy = R.Legitimacy(id) - R.REFUSAL_LEGITIMACY
			Players[id]:SetProperty(R.LEGITIMACY_KEY, legitimacy)
			Players[id]:SetProperty(R.OUTLAW_KEY, turn + R.OUTLAW_TURNS)
			Log(string.format("refused player=%d pair=%d-%d turn=%d legitimacy=%d outlaw_until=%d", id, a, b, turn,
				legitimacy, turn + R.OUTLAW_TURNS))
			Notify(everyone, "LOC_JAC_ARB_NOTIFY_REFUSED_NAME", "LOC_JAC_ARB_NOTIFY_REFUSED_SUMMARY", R.NameKey(id), R.NameKey(a), R.NameKey(b))
		end
	end
end

-- 公式：DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246（GameEvents.OnGameTurnStarted）
GameEvents.OnGameTurnStarted.Add(function()
	local turn = R.GameTurn()
	EnforceTruces(turn)
	Resolve(turn)
end)
GameEvents.JAC_CourtArbitrationRequest.Add(OnRequest)
GameEvents.JAC_CourtArbitrationRespond.Add(OnRespond)

-- 検証用の読み取り口（シナリオの GameCore_Tuner から読む。状態の変更には使わない）
ExposedMembers.JAC_CourtArbitration = R

Log(string.format("loaded shogunate_end_turn=%d", R.ShogunateEndTurn()))
