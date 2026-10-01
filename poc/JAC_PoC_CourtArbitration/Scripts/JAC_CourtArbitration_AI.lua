-- オーナー判断 B（SSOT 3.2）。Gameplay だけで読み、乱数・UI・ローカルプレイヤーを使わない。
-- 数値と保存キーは Rules。ここでは読み取りと候補選択を行い、受付・支出は本来の請求処理を使う。
local R = JAC_ARB

-- 戦闘可能な部隊の数。人数の純減は損失の代用で、戦死数や相手ごとの戦果ではない。
function R.AiArmyCount(id)
	local count = 0
	for _, unit in Players[id]:GetUnits():Members() do
		local row = GameInfo.Units[unit:GetType()]
		if row and ((row.Combat or 0) > 0 or (row.RangedCombat or 0) > 0) then count = count + 1 end
	end
	return count
end

-- CitySupport.lua:537 の読み取り。Gameplay で使えないエンジンでは交戦期間による疲弊だけを使う。
-- 失敗を0の観測とは扱わず、読み取れた都市数も判断ログへ残す。
function R.AiWeariness(id)
	local total, observed = 0, 0
	for _, city in Players[id]:GetCities():Members() do
		local ok, value = pcall(function() return city:GetGrowth():GetAmenitiesLostFromWarWeariness() end)
		if ok and type(value) == "number" then
			total, observed = total + math.max(0, value), observed + 1
		end
	end
	return total, observed
end

-- 各ターン初めに観測する。和平した組は外し、再開した戦争は新しく数える。
-- 軍・都市は交戦中の最大数からの純減。補充・再建で戻り、他戦線の損失や部隊の統合も含む。
-- 旧セーブは最初に観測できたターンから始め、過去の戦闘を推定して足さない。
function R.ObserveAiWars(turn, ids)
	local previous = Game:GetProperty(R.AI_WARS_KEY) or {}
	local current, armies, cities = {}, {}, {}
	for _, id in ipairs(ids) do
		armies[id], cities[id] = R.AiArmyCount(id), Players[id]:GetCities():GetCount()
	end
	for i = 1, #ids do
		for j = i + 1, #ids do
			local a, b = ids[i], ids[j]
			if Players[a]:GetDiplomacy():IsAtWarWith(b) then
				local key = R.AiWarKey(a, b)
				local old = previous[key] or { Since = turn }
				current[key] = { Since = old.Since,
					CitiesA = math.max(old.CitiesA or 0, cities[a]), CitiesB = math.max(old.CitiesB or 0, cities[b]),
					ArmyA = math.max(old.ArmyA or 0, armies[a]), ArmyB = math.max(old.ArmyB or 0, armies[b]) }
			end
		end
	end
	Game:SetProperty(R.AI_WARS_KEY, current)
end

function R.AiPartyScore(id, opponent, turn, war)
	local suffix = id < opponent and "A" or "B"
	local duration = math.max(0, turn - war.Since)
	local weariness, observed = R.AiWeariness(id)
	local armyLoss = math.max(0, war["Army" .. suffix] - R.AiArmyCount(id))
	local cityLoss = math.max(0, war["Cities" .. suffix] - Players[id]:GetCities():GetCount())
	local low = R.Legitimacy(id) < R.AI_LOW_LEGITIMACY_LIMIT and R.AI_LOW_LEGITIMACY_POINTS or 0
	local score = math.min(R.AI_WAR_POINTS_MAX, math.floor(duration / R.AI_WAR_TURNS_PER_POINT))
		+ math.min(R.AI_WEARINESS_POINTS_MAX, math.floor(weariness))
		+ math.min(R.AI_ARMY_LOSS_POINTS_MAX, armyLoss)
		+ math.min(R.AI_CITY_LOSS_POINTS_MAX, cityLoss * R.AI_CITY_LOSS_POINTS) + low
	return score, string.format("duration=%d weariness=%g weariness_cities=%d army_loss=%d city_loss=%d low_legitimacy=%d",
		duration, weariness, observed, armyLoss, cityLoss, low)
end

function R.AiShogunScore(id, a, b, turn, war)
	if not R.IsEmpoweredShogun(id, turn) or id == a or id == b then return 0, "" end
	local diplomacy = Players[id]:GetDiplomacy()
	local ally = diplomacy:HasAllied(a) or diplomacy:HasAllied(b)
	local home = Players[id]:GetProperty(R.HOME_KEY)
	local plot = home ~= nil and Map.GetPlotByIndex(home) or nil
	local capitalRegion = false
	if plot ~= nil then
		for _, party in ipairs({ a, b }) do
			for _, city in Players[party]:GetCities():Members() do
				if Map.GetPlotDistance(plot:GetX(), plot:GetY(), city:GetX(), city:GetY()) <= R.AI_SHOGUN_CAPITAL_RADIUS then
					capitalRegion = true
				end
			end
		end
	end
	local longWar = turn - war.Since >= R.AI_SHOGUN_LONG_WAR_TURNS
	local score = (ally and R.AI_SHOGUN_ALLY_POINTS or 0) + (capitalRegion and R.AI_SHOGUN_CAPITAL_POINTS or 0)
		+ (longWar and R.AI_SHOGUN_LONG_WAR_POINTS or 0)
	return score, string.format("duration=%d ally=%s capital=%s long_war=%s", turn - war.Since,
		tostring(ally), tostring(capitalRegion), tostring(longWar))
end

-- 当事者の請求を先にする。同じ経路では点数の高い順、同点は請求者・小さい当事者ID・大きい当事者IDの順。
-- 1つが費用不足でも次の候補を調べる。AI用の間隔は受付に成功したときだけ更新する。
function R.AiCandidate(turn, ids)
	local wars = Game:GetProperty(R.AI_WARS_KEY) or {}
	local parties, shoguns = {}, {}
	for i = 1, #ids do
		for j = i + 1, #ids do
			local a, b = ids[i], ids[j]
			local war = wars[R.AiWarKey(a, b)]
			if war and Players[a]:GetDiplomacy():IsAtWarWith(b) and Game:GetProperty(R.TruceKey(a, b)) == nil then
				for _, pair in ipairs({ { a, b }, { b, a } }) do
					local id, opponent = pair[1], pair[2]
					if not Players[id]:IsHuman() then
						local score, detail = R.AiPartyScore(id, opponent, turn, war)
						if score >= R.AI_EXHAUSTION_THRESHOLD then
							table.insert(parties, { Requester = id, A = a, B = b, Score = score, Detail = detail, Source = "ai_party" })
						end
					end
				end
				for _, id in ipairs(ids) do
					if not Players[id]:IsHuman() and R.IsEmpoweredShogun(id, turn) and id ~= a and id ~= b then
						local score, detail = R.AiShogunScore(id, a, b, turn, war)
						if score >= R.AI_SHOGUN_THRESHOLD then
							table.insert(shoguns, { Requester = id, A = a, B = b, Score = score, Detail = detail, Source = "ai_shogun" })
						end
					end
				end
			end
		end
	end
	local blocked = { GOLD = 0, LEGITIMACY = 0, INTERVAL = 0 }
	for _, candidates in ipairs({ parties, shoguns }) do
		table.sort(candidates, function(x, y)
			if x.Score ~= y.Score then return x.Score > y.Score end
			if x.Requester ~= y.Requester then return x.Requester < y.Requester end
			if x.A ~= y.A then return x.A < y.A end
			return x.B < y.B
		end)
		for _, candidate in ipairs(candidates) do
			local id, a, b = candidate.Requester, candidate.A, candidate.B
			if not R.AiIntervalReady(id, a, b, turn) then
				blocked.INTERVAL = blocked.INTERVAL + 1
			else
				local ok, reason = R.CheckRequest(id, a, b)
				if ok then return candidate, blocked end
				if blocked[reason] ~= nil then blocked[reason] = blocked[reason] + 1 end
			end
		end
	end
	return nil, blocked
end
