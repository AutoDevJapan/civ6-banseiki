-- 日本マップ：1534年の同盟・婚姻・主従と交戦を、新規ゲームの開始時に1回だけ外交状態へ反映する。
-- 元データ：documents/spec/data/japan-1534-relations.json（適用分だけをここに写す。テストで一致を確かめる）
-- 敵対・不仲（hostility）は、ここでは出会い済みにするだけ。「1534年の因縁」の好感度減点は Data/JAC_Japan_Feuds.xml（勢力別の特性の修正値）が担う。
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

local function Apply()
	if Game:GetProperty(PROPERTY_KEY) ~= nil then
		Log("skip: already applied")
		return
	end
	local players = MajorPlayersByFaction()
	local applied, missing, failed = 0, 0, 0
	for _, relation in ipairs(RELATIONS) do
		local a, b, kind, id = players[relation[1]], players[relation[2]], relation[3], relation[4]
		if a == nil or b == nil then
			missing = missing + 1
			Log("missing player " .. id)
		else
			local diploA, diploB = Players[a]:GetDiplomacy(), Players[b]:GetDiplomacy()
			diploA:SetHasMet(b)
			diploB:SetHasMet(a)
			local ok, err
			if kind == "war" then
				ok, err = pcall(function() diploA:DeclareWarOn(b, WarTypes.FORMAL_WAR, true) end)
			elseif kind == "hostility" then
				ok = true -- 出会い済みにするのみ。減点は修正値が担う
			else
				-- ponytail: 同盟・婚姻・主従・友好はすべて友好宣言で近似する。期限（30ターン）で切れる
				ok, err = pcall(function()
					diploA:SetHasDeclaredFriendship(b, true)
					diploB:SetHasDeclaredFriendship(a, true)
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
	Game:SetProperty(PROPERTY_KEY, {Turn = Game.GetCurrentGameTurn(), Applied = applied, Missing = missing, Failed = failed})
	Log(string.format("applied=%d missing=%d failed=%d", applied, missing, failed))
end

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
