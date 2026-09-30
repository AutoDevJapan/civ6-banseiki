-- 朝廷・将軍の停戦裁定（SSOT 3.2）。日本マップ4種でだけ読み込まれる（modinfo）。
-- 1. 戦争中の当事者は相手との停戦裁定を請求できる。足利将軍家は、人間・AI を問わず、室町幕府の終わり（1573年）
--    までは他家どうしの停戦も発議でき、その裁定の力は3倍になる（停戦のターン数と拒否の不利益が3倍、費用は3分の1）。
-- 2. 請求者は金と正当性を払う（日本マップでは正当性が外交的支持の代わり：オーナー判断 R-1）。請求は UI から
--    EXECUTE_SCRIPT で届き、次のターンの初めに裁く。
-- 3. 答える側：人間は画面で応否を送る（届かなければ拒否）。AI は Rules の暫定規則で決める。
-- 4. 全員が応じればその場で講和し（最短の戦争期間を越える MakePeaceWith(相手, true)）、停戦中に戦争になって
--    いれば毎ターンの初めに講和し直す。応じた家は正当性が上がり、拒んだ家は正当性を失い、その家を対象に
--    討伐の勅命が出る（オーナー判断 R-2）。
-- 5. 正当性は毎ターンの初めに収入（Rules の LegitimacyIncome）で増え、高低で全都市の忠誠度が変わる
--    （都市のプロットの旗 → Data/JAC_CourtArbitration_Legitimacy.sql の修正値）。
-- 6. 討伐の勅命：期間中、どの家も参加できる（人間は画面のボタン、AI は対象と戦争中か隣接していれば自動）。
--    参加者は戦争疲弊が減り（代表の都市の旗 → 修正値）、期間中に対象の都市を攻め取れば金と正当性を得る。
--
-- マルチプレイ：状態はすべて Game / Player / プロットのプロパティ（セーブに残り、全員で共有）。変更は GameEvents
-- （EXECUTE_SCRIPT・OnGameTurnStarted・CityConquered）の中だけで、乱数もローカルプレイヤーの判定も使わない。
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

local function ChangeLegitimacy(id, delta)
	local value = R.ClampLegitimacy(R.Legitimacy(id) + delta)
	Players[id]:SetProperty(R.LEGITIMACY_KEY, value)
	return value
end

-- 本拠：各家の最初の首都（JAC_Sengoku_UnificationPact.lua の RecordHomes と同じ決め方。
-- GetCapitalCity は DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1103）
local function RecordHomes()
	for _, id in ipairs(SortedMajorIDs()) do
		if Players[id]:GetProperty(R.HOME_KEY) == nil then
			local capital = Players[id]:GetCities():GetCapitalCity()
			if capital then Players[id]:SetProperty(R.HOME_KEY, Map.GetPlot(capital:GetX(), capital:GetY()):GetIndex()) end
		end
	end
end

-- 都市の中心のプロットに旗を書く（冪等。JAC_Sengoku_Ages.lua の WriteProperties と同じ形。プロットの
-- SetProperty は DLC/CivRoyaleScenario/Scripts/CivRoyaleScenario_StartScript.lua:496-519）。値は 0/1 の数
-- （空文字はセーブで戻らない）。勅命の旗は、家ごとにプロット番号が最小の都市（代表）だけに立てる
local function WriteCityFlags(turn)
	local ids = SortedMajorIDs()
	for _, id in ipairs(ids) do
		local legitimacy = R.Legitimacy(id)
		local joined = false
		for _, target in ipairs(ids) do
			if R.HasJoined(id, target, turn) then joined = true end
		end
		local plots = {}
		for _, city in Players[id]:GetCities():Members() do -- BlackDeathScenario.lua:136
			table.insert(plots, Map.GetPlot(city:GetX(), city:GetY()))
		end
		table.sort(plots, function(p, q) return p:GetIndex() < q:GetIndex() end)
		for i, plot in ipairs(plots) do
			plot:SetProperty(R.FLAG_HIGH, legitimacy >= R.LEGITIMACY_HIGH and 1 or 0)
			plot:SetProperty(R.FLAG_LOW, legitimacy < R.LEGITIMACY_LOW and 1 or 0)
			plot:SetProperty(R.FLAG_SUBJUGATION, (joined and i == 1) and 1 or 0)
		end
	end
end

local function ApplyIncome(turn)
	for _, id in ipairs(SortedMajorIDs()) do
		local income = R.LegitimacyIncome(id, turn)
		Log(string.format("legitimacy player=%d +%d = %d turn=%d", id, income, ChangeLegitimacy(id, income), turn))
	end
end

-- 自分の都市と対象の都市の最短の距離が SUBJUGATION_NEIGHBOR_DISTANCE 以下なら隣接とみなす（暫定）。
-- 公式：Map.GetPlotDistance（DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:132）
local function Neighbors(a, b)
	for _, mine in Players[a]:GetCities():Members() do
		for _, theirs in Players[b]:GetCities():Members() do
			if Map.GetPlotDistance(mine:GetX(), mine:GetY(), theirs:GetX(), theirs:GetY()) <= R.SUBJUGATION_NEIGHBOR_DISTANCE then
				return true
			end
		end
	end
	return false
end

local function Join(id, target, turn, how)
	Players[id]:SetProperty(R.JoinedKey(target), 1)
	Log(string.format("joined player=%d target=%d turn=%d until=%d by=%s", id, target, turn,
		Players[target]:GetProperty(R.OUTLAW_KEY), how))
	Notify({ id, target }, "LOC_JAC_ARB_NOTIFY_JOINED_NAME", "LOC_JAC_ARB_NOTIFY_JOINED_SUMMARY", R.NameKey(id), R.NameKey(target))
end

-- AI は、勅命の対象と戦争中か隣接していれば自分から参加する。人間は画面のボタンで参加する
local function AutoJoin(turn)
	local ids = SortedMajorIDs()
	for _, target in ipairs(ids) do
		if R.IsTarget(target, turn) then
			for _, id in ipairs(ids) do
				if not Players[id]:IsHuman() and R.CheckJoin(id, target, turn)
					and (Players[id]:GetDiplomacy():IsAtWarWith(target) or Neighbors(id, target)) then
					Join(id, target, turn, "ai")
				end
			end
		end
	end
end

-- 期限を過ぎた勅命を終える。参加の記録も消し、戦争疲弊の旗は続く WriteCityFlags で消える
local function ExpireOrders(turn)
	local ids = SortedMajorIDs()
	for _, target in ipairs(ids) do
		local untilTurn = Players[target]:GetProperty(R.OUTLAW_KEY)
		if untilTurn ~= nil and turn > untilTurn then
			Players[target]:SetProperty(R.OUTLAW_KEY, nil)
			local notified = { target }
			for _, id in ipairs(ids) do
				if Players[id]:GetProperty(R.JoinedKey(target)) ~= nil then
					Players[id]:SetProperty(R.JoinedKey(target), nil)
					table.insert(notified, id)
				end
			end
			Log(string.format("subjugation ended target=%d turn=%d", target, turn))
			Notify(notified, "LOC_JAC_ARB_NOTIFY_ORDER_ENDED_NAME", "LOC_JAC_ARB_NOTIFY_ORDER_ENDED_SUMMARY", R.NameKey(target))
		end
	end
end

-- 討伐の勅命への参加（UI の EXECUTE_SCRIPT。params.Target = 対象）
local function OnJoin(playerID, params)
	local turn = R.GameTurn()
	Game:SetProperty(R.LAST_EVENT_KEY, string.format("join player=%d turn=%d", playerID, turn))
	local target = tonumber(params.Target)
	local ok, reason = R.CheckJoin(playerID, target, turn)
	if not ok then
		Log(string.format("rejected join player=%d target=%s reason=%s", playerID, tostring(target), reason))
		return
	end
	Join(playerID, target, turn, "request")
	WriteCityFlags(turn)
end

-- 都市の攻略（公式：DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:174,248 の GameEvents.CityConquered。
-- 引数は 攻略者・元の所有者・都市 ID・座標）。勅命の期間中に参加者が対象の都市を攻め取れば褒賞を与える
local function OnCityConquered(capturerID, ownerID, cityID, x, y)
	local turn = R.GameTurn()
	if ownerID ~= nil and Players[ownerID] ~= nil and Players[capturerID] ~= nil and R.HasJoined(capturerID, ownerID, turn) then
		Players[capturerID]:GetTreasury():ChangeGoldBalance(R.SUBJUGATION_GOLD)
		local gained = ChangeLegitimacy(capturerID, R.SUBJUGATION_LEGITIMACY)
		local lost = ChangeLegitimacy(ownerID, -R.SUBJUGATION_LEGITIMACY)
		Log(string.format("subjugation capture player=%d target=%d city=%s turn=%d gold=+%d legitimacy=%d target_legitimacy=%d",
			capturerID, ownerID, tostring(cityID), turn, R.SUBJUGATION_GOLD, gained, lost))
		Notify(SortedMajorIDs(), "LOC_JAC_ARB_NOTIFY_CAPTURE_NAME", "LOC_JAC_ARB_NOTIFY_CAPTURE_SUMMARY",
			R.NameKey(capturerID), R.NameKey(ownerID), R.SUBJUGATION_GOLD, R.SUBJUGATION_LEGITIMACY)
	end
	WriteCityFlags(turn)
end

-- 請求（UI/JAC_CourtArbitrationPanel.lua → UI.RequestPlayerOperation(EXECUTE_SCRIPT)、OnStart = この名前。
-- 公式：BlackDeathScenario_PopupHandler.lua:22-28 → BlackDeathScenario.lua:66 の GameEvents.EventPopupChoice）
local function OnRequest(requester, params)
	Game:SetProperty(R.LAST_EVENT_KEY, string.format("request player=%d turn=%d", requester, R.GameTurn()))
	local a, b = tonumber(params.Target), tonumber(params.Other)
	local ok, reason = R.CheckRequest(requester, a, b)
	if not ok then
		Log(string.format("rejected request player=%d pair=%s-%s reason=%s", requester, tostring(a), tostring(b), reason))
		return
	end
	if requester == b then a, b = b, a end -- 当事者の請求では A を請求者にそろえる
	local player = Players[requester]
	local turn = R.GameTurn()
	local power = R.Power(requester, turn)
	local gold, legitimacy = R.Costs(power) -- 将軍家の請求は3分の1（オーナー判断 O-3・R-3）
	player:GetTreasury():ChangeGoldBalance(-gold) -- AustraliaScenario.lua:847
	ChangeLegitimacy(requester, -legitimacy)
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
	Game:SetProperty(R.LAST_EVENT_KEY, string.format("respond player=%d turn=%d", playerID, R.GameTurn()))
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

-- 講和させ、できたら true。公式：DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:767（MakePeaceWith）。
-- エンジンは開戦から DIPLOMACY_WAR_MIN_TURNS（10、Base/Assets/Gameplay/Data/GlobalParameters.xml:260）の間、
-- 引数1つの MakePeaceWith をエラーも出さずに無視する（court-arbitration-05）。第2引数 true はこれを越えて
-- その場で講和させる（実機 court-arbitration-06 の probe-make-peace：ターン1に宣戦した琉球・伊達と、開始時から
-- 交戦中の大内・大友が、true 付きでだけ同じターンに講和した）。念のため呼んだ後に IsAtWarWith で確かめ、
-- できなければ理由を返す（呼び出し側が次のターンに試し直す安全網。実機では起きていない）
local function MakePeace(a, b, turn)
	local diplo = Players[a]:GetDiplomacy()
	local ok, err = pcall(function() diplo:MakePeaceWith(b, true) end)
	if not ok then return false, "error: " .. tostring(err) end
	if diplo:IsAtWarWith(b) then
		local known, can = pcall(function() return diplo:CanMakePeaceWith(b) end)
		return false, (known and can == false) and "min_war" or "still_at_war"
	end
	Game:SetProperty(R.PeaceTurnKey(a, b), turn)
	return true
end

-- 毎ターンの初めに、組ごとに講和を試す。
-- 1. 応諾済みで講和を待つ組（安全網：裁定の場で MakePeace が失敗したときだけできる）：講和できたターンから停戦を始める。
-- 2. 停戦中に戦争になっている組：講和し直す。講和できなければ（安全網）、停戦が切れた後も講和できるまで試す。
--    前のターンか同じターンにスクリプトが講和させた組は、1ターン待ってから講和し直す（往復の防止）。
-- 宣戦した側はゲームプレイ側で分からない（宣戦のイベント Events.DiplomacyDeclareWar は UI 側の使用例しか無い）ため、
-- 違反の不利益は付けない
local function EnforceTruces(turn)
	local ids = SortedMajorIDs()
	for i = 1, #ids do
		for j = i + 1, #ids do
			local a, b = ids[i], ids[j]
			local awaitKey = R.AwaitKey(a, b)
			local truceTurns = Game:GetProperty(awaitKey)
			if truceTurns ~= nil then
				local peace, reason = true, nil
				if Players[a]:GetDiplomacy():IsAtWarWith(b) then peace, reason = MakePeace(a, b, turn) end
				if peace then
					Game:SetProperty(awaitKey, nil)
					Game:SetProperty(R.TruceKey(a, b), turn + truceTurns)
					Log(string.format("truce started pair=%d-%d turn=%d until=%d", a, b, turn, turn + truceTurns))
				else
					Log(string.format("SAFETY NET peace deferred pair=%d-%d turn=%d reason=%s", a, b, turn, reason))
				end
			end
			local key = R.TruceKey(a, b)
			local untilTurn = Game:GetProperty(key)
			if untilTurn ~= nil then
				local peace, reason = true, nil
				local lastPeace = Game:GetProperty(R.PeaceTurnKey(a, b))
				if Players[a]:GetDiplomacy():IsAtWarWith(b) and lastPeace ~= nil and lastPeace >= turn - 1 then
					-- スクリプトが講和させたターンの中で戦争になった組（通常は講和後の最短期間 DIPLOMACY_PEACE_MIN_TURNS = 10
					-- のため起きず、強制の DeclareWarOn でだけ起きる）は、講和と宣戦を続けて往復させないよう1ターン待つ。
					-- court-arbitration-08 では、強制講和と同じターンに強制宣戦した後のターン処理でエンジンが止まった
					peace = false
					Log(string.format("truce breach deferred pair=%d-%d turn=%d peace_turn=%d", a, b, turn, lastPeace))
				elseif Players[a]:GetDiplomacy():IsAtWarWith(b) then
					peace, reason = MakePeace(a, b, turn)
					Log(string.format("%struce reimposed pair=%d-%d turn=%d until=%d ok=%s%s", peace and "" or "SAFETY NET ",
						a, b, turn, untilTurn, tostring(peace), peace and "" or " reason=" .. reason))
				end
				if peace and turn > untilTurn then
					Game:SetProperty(key, nil)
					Log(string.format("truce ended pair=%d-%d turn=%d", a, b, turn))
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
	local refusers, accepters = {}, {}
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
			table.insert(accept and accepters or refusers, id)
		end
	end
	-- 応じた家は正当性が上がる（R-1。力は掛けない）。相手が拒んで裁定が成らなくても上がる
	for _, id in ipairs(accepters) do
		Log(string.format("accept bonus player=%d legitimacy=%d", id, ChangeLegitimacy(id, R.ACCEPT_LEGITIMACY)))
	end
	local everyone = SortedMajorIDs()
	if #refusers == 0 then
		local truceTurns = R.TRUCE_TURNS * pending.Power
		local peace, reason = MakePeace(a, b, turn)
		local summary = "LOC_JAC_ARB_NOTIFY_ACCEPTED_SUMMARY"
		if peace then
			Game:SetProperty(R.TruceKey(a, b), turn + truceTurns)
		else
			-- 安全網（MakePeaceWith(相手, true) が効かなかったとき）：講和できるターンの初めに講和し、
			-- そこから停戦を数える（EnforceTruces）
			Game:SetProperty(R.AwaitKey(a, b), truceTurns)
			summary = "LOC_JAC_ARB_NOTIFY_ACCEPTED_DEFERRED_SUMMARY"
		end
		Log(string.format("%saccepted pair=%d-%d turn=%d truce_turns=%d power=%d peace=%s", peace and "" or "SAFETY NET ",
			a, b, turn, truceTurns, pending.Power, peace and "true" or "deferred reason=" .. reason))
		Notify(everyone, "LOC_JAC_ARB_NOTIFY_ACCEPTED_NAME", summary, R.NameKey(a), R.NameKey(b), truceTurns)
	else
		-- 将軍家の裁定を拒めば不利益も3倍（オーナー判断 O-3・R-3）。勅命が続いている家は、長い方の期限にする
		local loss, outlawTurns = R.RefusalPenalty(pending.Power)
		for _, id in ipairs(refusers) do
			local legitimacy = ChangeLegitimacy(id, -loss)
			local untilTurn = math.max(turn + outlawTurns, Players[id]:GetProperty(R.OUTLAW_KEY) or 0)
			Players[id]:SetProperty(R.OUTLAW_KEY, untilTurn)
			Log(string.format("refused player=%d pair=%d-%d turn=%d legitimacy=%d outlaw_until=%d power=%d", id, a, b, turn,
				legitimacy, untilTurn, pending.Power))
			Notify(everyone, "LOC_JAC_ARB_NOTIFY_REFUSED_NAME", "LOC_JAC_ARB_NOTIFY_REFUSED_SUMMARY", R.NameKey(id), R.NameKey(a),
				R.NameKey(b), untilTurn)
		end
	end
end

-- 公式：DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246（GameEvents.OnGameTurnStarted）
GameEvents.OnGameTurnStarted.Add(function()
	local turn = R.GameTurn()
	RecordHomes()
	ExpireOrders(turn)
	ApplyIncome(turn)
	EnforceTruces(turn)
	Resolve(turn)
	AutoJoin(turn)
	WriteCityFlags(turn)
end)
GameEvents.JAC_CourtArbitrationRequest.Add(OnRequest)
GameEvents.JAC_CourtArbitrationRespond.Add(OnRespond)
GameEvents.JAC_CourtArbitrationJoin.Add(OnJoin)
GameEvents.CityConquered.Add(OnCityConquered)

-- 検証用の読み取り口（シナリオの GameCore_Tuner から読む。状態の変更には使わない）
ExposedMembers.JAC_CourtArbitration = R

Log(string.format("loaded shogunate_end_turn=%d", R.ShogunateEndTurn()))
