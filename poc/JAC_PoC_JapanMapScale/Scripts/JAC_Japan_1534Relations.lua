-- 日本マップ：1534年の同盟・婚姻・主従と交戦を、新規ゲームの開始時に1回だけ外交状態へ反映する。
-- 元データ：documents/spec/data/japan-1534-relations.json（適用分だけをここに写す。テストで一致を確かめる）
-- 同盟・婚姻・主従と、友好の島津・琉球王国（味方の8組）は開始時に同盟にし、同盟国として視界を共有する。
-- 視界の共有は Data/JAC_Japan_Feuds.xml の特性 TRAIT_JAC_1534_ALLY_VISION（プンドメーカーと同じ「全同盟で視界を共有」）が担う。Rise and Fall 以降、種類の無い同盟だけでは視界を共有しない。
-- オーナー判断I-1（軍事同盟から始める）の近似：スクリプトから同盟の種類とレベルは設定できない（documents/poc/japan-1534-relations.md）。
-- 味方の同盟は期限なしで続き、イベントが EndStartAlliance を呼んだとき（または両家が交戦したとき）だけ終わる（オーナー判断 2026-09-30・Discord H「bで島津と琉球も。」）。
-- 22勢力は開始時にすべて互いに出会い済みにする（固定キャラだけが出る日本マップなので、全員が互いを知っている）。
-- 敵対・不仲（hostility）は、ここでは何もしない。「1534年の因縁」の好感度減点と、全勢力の外交アクセスレベル +1 は Data/JAC_Japan_Feuds.xml が担う。
-- 史実の領土の日本マップだけで読み込まれる（modinfo）。開拓者からの2種では何も適用しない。

local PROPERTY_KEY = "JAC_JAPAN_1534_RELATIONS"

-- {勢力A, 勢力B, 種類, 関係ID}。勢力は CIVILIZATION_JAC_<名前> の <名前>
local RELATIONS = {
	{"OUCHI", "OTOMO", "war", "R01"},
	{"HOJO", "IMAGAWA", "alliance", "R02"},
	{"OUCHI", "MORI", "vassal", "R03"},
	{"DATE", "ASHINA", "marriage", "R04"},
	{"ASHIKAGA", "ROKKAKU", "alliance", "R05"},
	{"ASAKURA", "ROKKAKU", "alliance", "R06"},
	{"ASHIKAGA", "ASAKURA", "vassal", "R07"},
	{"AMAGO", "MORI", "alliance", "R08"},
	{"HOJO", "TAKEDA", "hostility", "R09"},
	{"IMAGAWA", "TAKEDA", "hostility", "R10"},
	{"HOJO", "YAMANOUCHI_UESUGI", "hostility", "R11"},
	{"NAGAO", "YAMANOUCHI_UESUGI", "hostility", "R12"},
	{"NAGAO", "HONGANJI", "hostility", "R13"},
	{"HONGANJI", "ROKKAKU", "hostility", "R14"},
	{"ODA", "MATSUDAIRA", "hostility", "R15"},
	{"SHIMAZU", "RYUKYU", "friendship", "R19"},
}

-- 味方（開始時に同盟にし、期限なしで保つ組）。友好（島津と琉球王国）も含む（オーナー判断 2026-09-30・Discord H）
local ALLIED_KINDS = {alliance = true, marriage = true, vassal = true, friendship = true}

-- 味方の組ごとの Game プロパティ。数値 = 開始時の同盟が続いていて、その値のターンに結んだ（結び直した）、false = イベントなどで終わった。セーブに残る
local ALLIANCE_KEY_PREFIX = "JAC_1534_ALLIANCE_"

-- エンジンの期限切れで解けた同盟を、同じターンの初めに結び直した回数（Game プロパティ。一度も切れていなければ nil）
local LAPSES_KEY = "JAC_1534_ALLIANCE_LAPSES"

-- 調査用シナリオ（tools/civ6-autotest/scenarios/probe-alliance-expiry.json）だけが true にする Game プロパティ。true の間は KeepStartAlliances が何もしない（エンジン本来の期限切れを測るため）。通常のゲームでは誰も書かないので nil のまま
local KEEP_DISABLED_KEY = "JAC_1534_KEEP_ALLIANCES_DISABLED"

local function Log(message)
	print("[JAC_1534_RELATIONS] " .. message)
end

local function MajorPlayersByFaction()
	local result = {}
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do
		local civ = PlayerConfigurations[player:GetID()]:GetCivilizationTypeName()
		local faction = civ and string.match(civ, "^CIVILIZATION_JAC_(.+)$")
		if faction then result[faction] = player:GetID() end
	end
	return result
end

local function AllianceKey(factionA, factionB)
	if factionA > factionB then factionA, factionB = factionB, factionA end
	return ALLIANCE_KEY_PREFIX .. factionA .. "-" .. factionB
end

local function IsStartAllianceActive(factionA, factionB)
	return type(Game:GetProperty(AllianceKey(factionA, factionB))) == "number"
end

-- 開始時の同盟を終わらせる。将来のイベント（裏切りなど）がゲームプレイのスクリプトから呼ぶ。
-- 以後は結び直さない。終わらせたら true、続いていなかったら false。
-- 同盟中の SetHasAllied(相手, false) はエンジンが受け付けず、同盟のまま残った（実機 diplo-17）。その場合、同盟はエンジンの期限切れ（最長で期限 + 1 ターン）で終わる
local function EndStartAlliance(factionA, factionB)
	if not IsStartAllianceActive(factionA, factionB) then return false end
	Game:SetProperty(AllianceKey(factionA, factionB), false)
	local players = MajorPlayersByFaction()
	local a, b = players[factionA], players[factionB]
	if a ~= nil and b ~= nil and Players[a]:GetDiplomacy():HasAllied(b) then
		Players[a]:GetDiplomacy():SetHasAllied(b, false)
		Players[a]:GetDiplomacy():RecheckVisibilityOnAll()
		Players[b]:GetDiplomacy():RecheckVisibilityOnAll()
	end
	Log(string.format("ended start alliance %s-%s", factionA, factionB))
	return true
end

-- 毎ターンの初めに、続いている開始時の同盟を保つ。RELATIONS の順に処理する（全員で同じ順になる）。
-- エンジンの同盟の期限は延ばせない（実機 diplo-17：同盟中の SetHasAllied(false) は何も起きず、false → true も true の再呼び出しも期限を数え直さない。
-- 公式の「同盟の更新」DIPLOACTION_RENEW_ALLIANCE は万世紀ルールセットの DB に無い）。
-- エンジンは結んだターン + 期限 + 1 ターン目（開始ターン1なら32ターン目）へ切り替わるときに、この処理より前に同盟を解く（実機 diplo-16・diplo-17）。
-- そこでこの処理で結び直す（"re-made after lapse"）。どの家の手番よりも前なので、手番の間に同盟と視界の共有が途切れることは無い
local function KeepStartAlliances()
	if Game:GetProperty(KEEP_DISABLED_KEY) == true then return end
	local players = MajorPlayersByFaction()
	local turn = Game.GetCurrentGameTurn()
	for _, relation in ipairs(RELATIONS) do
		local factionA, factionB = relation[1], relation[2]
		local a, b = players[factionA], players[factionB]
		if a ~= nil and b ~= nil and IsStartAllianceActive(factionA, factionB) then
			local diplo = Players[a]:GetDiplomacy()
			if diplo:IsAtWarWith(b) then
				-- 同盟国どうしは通常の外交では宣戦できない。スクリプトなどで交戦になったら、同盟も終わりとする
				EndStartAlliance(factionA, factionB)
			elseif not diplo:HasAllied(b) then
				local key = AllianceKey(factionA, factionB)
				local madeTurn = Game:GetProperty(key)
				local ok, err = pcall(function() diplo:SetHasAllied(b, true) end)
				if ok then
					Game:SetProperty(key, turn)
					Game:SetProperty(LAPSES_KEY, (Game:GetProperty(LAPSES_KEY) or 0) + 1)
					diplo:RecheckVisibilityOnAll()
					Players[b]:GetDiplomacy():RecheckVisibilityOnAll()
					Log(string.format("re-made after lapse %s %s-%s on turn %d (made on turn %d)",
						relation[4], factionA, factionB, turn, madeTurn))
				else
					Log(string.format("FAILED re-make %s %s-%s: %s", relation[4], factionA, factionB, tostring(err)))
				end
			end
		end
	end
end

local function Apply()
	if Game:GetProperty(PROPERTY_KEY) ~= nil then
		Log("skip: already applied")
		return
	end
	local players = MajorPlayersByFaction()
	-- 全員を互いに出会い済みにする（両方向。新規ゲームの開始時に1回だけ）
	for _, a in pairs(players) do
		for _, b in pairs(players) do
			if a ~= b then Players[a]:GetDiplomacy():SetHasMet(b) end
		end
	end
	local applied, missing, failed = 0, 0, 0
	for _, relation in ipairs(RELATIONS) do
		local a, b, kind, id = players[relation[1]], players[relation[2]], relation[3], relation[4]
		if a == nil or b == nil then
			missing = missing + 1
			Log("missing player " .. id)
		else
			local diploA, diploB = Players[a]:GetDiplomacy(), Players[b]:GetDiplomacy()
			local ok, err
			if kind == "war" then
				ok, err = pcall(function() diploA:DeclareWarOn(b, WarTypes.FORMAL_WAR, true) end)
			elseif kind == "hostility" then
				ok = true -- 何もしない。減点は修正値が担う
			else
				-- 味方は友好宣言のあと通常の同盟にする。視界の共有は、両家が持つ特性 TRAIT_JAC_1534_ALLY_VISION が同盟に効かせる（オーナー判断G）。
				-- 永久同盟（SetPermanentAlliance）は解く関数が無いので使わない。期限で解けたら KeepStartAlliances が同じターンの初めに結び直す。
				-- 公式：DLC/VikingsScenario/Scripts/VikingScenario.lua:18〜:20
				ok, err = pcall(function()
					diploA:SetHasDeclaredFriendship(b, true)
					diploB:SetHasDeclaredFriendship(a, true)
					if ALLIED_KINDS[kind] then
						diploA:SetHasAllied(b, true)
						Game:SetProperty(AllianceKey(relation[1], relation[2]), Game.GetCurrentGameTurn())
					end
				end)
			end
			if ok then
				applied = applied + 1
				Log(string.format("%s %s %s-%s", id, kind, relation[1], relation[2]))
			else
				failed = failed + 1
				Log(string.format("FAILED %s %s: %s", id, kind, tostring(err)))
			end
		end
	end
	-- 出会い済みにした相手へのアクセスレベル（全勢力の +1。Data/JAC_Japan_Feuds.xml）を開始ターンから反映する。
	-- 公式シナリオも出会い・代表団を設定した後に呼ぶ（DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1336〜:1339）
	for _, id in pairs(players) do
		Players[id]:GetDiplomacy():RecheckVisibilityOnAll()
	end
	Game:SetProperty(PROPERTY_KEY, {Turn = Game.GetCurrentGameTurn(), Applied = applied, Missing = missing, Failed = failed})
	Log(string.format("applied=%d missing=%d failed=%d", applied, missing, failed))
end

-- 同盟を保つ。公式：DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246（GameEvents.OnGameTurnStarted）
GameEvents.OnGameTurnStarted.Add(KeepStartAlliances)

-- 将来のイベントの仕組みから呼ぶ入口（ゲームプレイのスクリプトから呼ぶ。UI から呼ぶとマルチプレイで同期しない）。
-- 勢力は CIVILIZATION_JAC_<名前> の <名前>（例 "HOJO"）。組の順は問わない。
--   ExposedMembers.JAC_Japan1534Relations.EndStartAlliance("HOJO", "IMAGAWA")        開始時の同盟を終わらせる
--   ExposedMembers.JAC_Japan1534Relations.IsStartAllianceActive("HOJO", "IMAGAWA")   開始時の同盟が続いているか
ExposedMembers.JAC_Japan1534Relations = {
	EndStartAlliance = EndStartAlliance,
	IsStartAllianceActive = IsStartAllianceActive,
}

-- 新規ゲームだけで発火する（公式シナリオと同じ入口）。ロード時は発火しない
LuaEvents.NewGameInitialized.Add(Apply)

-- 念のため：開始ターンのまま未適用なら適用し、適用済みならログだけ出す（セーブに残るプロパティで二重適用を防ぐ）
Events.LoadScreenClose.Add(function()
	local state = Game:GetProperty(PROPERTY_KEY)
	if state == nil and Game.GetCurrentGameTurn() == GameConfiguration.GetStartTurn() then
		Apply()
	else
		Log("load: kept state from turn " .. tostring(state and state.Turn))
	end
end)
