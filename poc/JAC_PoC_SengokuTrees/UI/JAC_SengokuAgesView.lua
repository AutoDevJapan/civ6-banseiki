-- 改元の画面（UI/JAC_SengokuReignPopup.lua）、時代スコアの画面（UI/JAC_EraProgressPanel_Sengoku.lua）、右下の時代の
-- 目盛りの説明（UI/JAC_ActionPanel_Sengoku.lua）が共有する、武功の台帳（Game プロパティ JAC_AGES_LEDGER。
-- Scripts/JAC_Sengoku_Ages.lua が書く）の読み方と文の組み立て。読むだけで書かない。
-- 時代の名前は今の元号（「弘治の世」「Kōji Era」。オーナー判断 L-1、2026-09-30）。
-- ゲームプレイ側と同じ名前のファイルにしない（tests/test_lua_file_names.py）。

JAC_AGES_VIEW_PARTS = { "home", "regain", "castle", "ko", "ally", "research", "event" }

-- 年齢ごとの見せ方。絵は公式の時代の画面と同じもの（DLC/Expansion2/UI/Additions/EraReviewPopup.lua:70-106 の
-- Ages_BannerLong*・Ages_Parchment*・Ages_Frame*、EraProgressPanel.lua:141-185 の Controls_AgesBanner_* とアイコン）。
-- NONE は最初の改元の前（天文の期）で、効果は無いので通常期の絵を使う。
JAC_AGES_VIEW_STYLE = {
	HEROIC = { name = "LOC_JAC_AGE_NAME_HEROIC", icon = "[ICON_GLORY_SUPER_GOLDEN_AGE]", art = "Heroic", banner = "Controls_AgesBanner_Heroic" },
	GOLDEN = { name = "LOC_JAC_AGE_NAME_GOLDEN", icon = "[ICON_GLORY_GOLDEN_AGE]", art = "Golden", banner = "Controls_AgesBanner_Golden" },
	NORMAL = { name = "LOC_JAC_AGE_NAME_NORMAL", icon = "[ICON_GLORY_NORMAL_AGE]", art = "Normal", banner = "Controls_AgesBanner_Normal" },
	DARK = { name = "LOC_JAC_AGE_NAME_DARK", icon = "[ICON_GLORY_DARK_AGE]", art = "Dark", banner = "Controls_AgesBanner_Dark" },
	NONE = { name = "LOC_JAC_AGE_NAME_NONE", icon = "", art = "Normal", banner = "Controls_AgesBanner_Normal" },
}

local AGE_EFFECTS = {
	HEROIC = "LOC_JAC_AGE_HEROIC_ANNOUNCEMENT",
	GOLDEN = "LOC_JAC_AGE_GOLDEN_ANNOUNCEMENT",
	NORMAL = "LOC_JAC_AGE_NORMAL_ANNOUNCEMENT",
	NONE = "LOC_JAC_AGE_NONE_EFFECT",
}

function JAC_AgesView_Style(age) return JAC_AGES_VIEW_STYLE[age] or JAC_AGES_VIEW_STYLE.NONE end

function JAC_AgesView_Ledger()
	local l = Game:GetProperty("JAC_AGES_LEDGER")
	return type(l) == "table" and l or nil
end

-- ゲームプレイ側と同じ数え方（Scripts/JAC_Sengoku_Ages.lua の ElapsedTurn）。
function JAC_AgesView_ElapsedTurn()
	return Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn() + 1
end

-- 期の番号 → 元号の行。[0] は天文（開始時の元号、年は開始年）、[i] は i 番目の改元。ゲームプレイ側は改元のターンの順に
-- 並べる（Scripts/JAC_Sengoku_Ages.lua の REIGNS）。ターンは日付について単調なので、日付の順と同じ。
function JAC_AgesView_Reigns()
	local list = {}
	for row in GameInfo.JAC_SengokuReigns() do table.insert(list, row) end
	table.sort(list, function(a, b)
		local x, y = a.Year * 12 + a.Month, b.Year * 12 + b.Month
		if x ~= y then return x < y end
		return a.ReignType < b.ReignType
	end)
	list[0] = { ReignType = "REIGN_TENBUN", Name = "LOC_JAC_REIGN_TENBUN_NAME", Year = GameInfo.StartEras["ERA_ANCIENT"].Year }
	return list
end

-- 元号の行 → 時代の名前（「弘治の世」「Kōji Era」「Эпоха Кодзи」）。
function JAC_AgesView_EraName(reign)
	return Locale.Lookup("LOC_JAC_AGE_ERA_NAME", reign.Name)
end

-- 右下の時代の目盛りの説明（公式 ActionPanel_Expansion1.lua:205-209 の「時代の名前」と「時代の終わりまでのターン」）を、
-- 今の元号と次の改元までのターンにする。台帳が無ければ nil（公式のまま）。
function JAC_AgesView_EraIndicatorText(l, elapsed)
	if l == nil then return nil end
	local reign = JAC_AgesView_Reigns()[type(l.reign) == "number" and l.reign or 0]
	if reign == nil then return nil end
	local text = JAC_AgesView_EraName(reign)
	if type(l.nextDue) == "number" then
		text = text .. "[NEWLINE][NEWLINE]" .. Locale.Lookup("LOC_JAC_AGE_ERA_ENDS_IN", math.max(0, l.nextDue - elapsed))
	end
	return text
end

local function PolicyRow(policyType)
	for row in GameInfo.JAC_SengokuDarkAgePolicies() do
		if row.PolicyType == policyType then return row end
	end
	return nil
end

-- Shared by the choice and notification UI. This is the saved judgement turn, not the era's predicted due turn.
function JAC_AgesView_PendingDarkPolicy(l, playerID)
	local player = Players[playerID]
	if player == nil or not player:IsHuman() then return nil end
	local house = l and l.houses and l.houses["p" .. tostring(playerID)]
	local judged = l and l.history and l.history["r" .. tostring(l.reign)]
	if house and house.age == "DARK" and house.chosen ~= l.reign
		and judged and judged.turn == JAC_AgesView_ElapsedTurn() then return house end
	return nil
end

-- Custom notification properties: DLC/PiratesScenario/Scripts/PiratesScenario_PropKeys.lua:85-89 and
-- UI/Replacements/NotificationPanel_PiratesScenario.lua:81,145. Never capture an official dedication.
function JAC_AgesView_DarkPolicyNotification(notification)
	return notification ~= nil and notification:GetType() == NotificationTypes.COMMEMORATION_AVAILABLE
		and type(notification:GetValue("JAC_DARK_POLICY_REIGN")) == "number"
end

-- 改元の画面に出す、自分の家の今の期の判定。elapsed を渡すと、そのターンに判定した改元のときだけ返す。
function JAC_AgesView_Reign(l, playerID, elapsed)
	if l == nil or type(l.reign) ~= "number" or l.reign < 1 or type(l.history) ~= "table" then return nil end
	local judged = l.history["r" .. tostring(l.reign)]
	if type(judged) ~= "table" or (elapsed ~= nil and judged.turn ~= elapsed) then return nil end
	local result = judged["p" .. tostring(playerID)]
	local house = l.houses and l.houses["p" .. tostring(playerID)]
	if type(result) ~= "table" or type(house) ~= "table" then return nil end
	local reigns = JAC_AgesView_Reigns()
	local dark = result.age == "DARK"
	-- 判定した期（from。同じターンの改元をまとめたときは直前の番号ではない。古い台帳には無い）。
	local from = type(judged.from) == "number" and judged.from or l.reign - 1
	return {
		index = l.reign, reign = reigns[l.reign], previous = reigns[from], turn = judged.turn,
		age = result.age, score = result.score, golden = result.golden, dark = result.dark,
		policy = dark and house.policy or nil,
		pendingChoice = dark and JAC_AgesView_PendingDarkPolicy(l, playerID) ~= nil,
	}
end

-- 時代スコアの画面に出す、自分の家の今の期と過去の期。
function JAC_AgesView_Panel(l, playerID, elapsed)
	if l == nil or type(l.houses) ~= "table" then return nil end
	local house = l.houses["p" .. tostring(playerID)]
	if type(house) ~= "table" then return nil end
	local reigns = JAC_AgesView_Reigns()
	local index = type(l.reign) == "number" and l.reign or 0
	local parts = type(house.parts) == "table" and house.parts or {}
	local view = {
		index = index, reign = reigns[index], nextReign = reigns[index + 1],
		score = house.score or 0, age = house.age or "NONE", policy = house.policy,
		turnsLeft = l.nextDue and math.max(0, l.nextDue - elapsed) or nil,
		parts = {}, history = {},
	}
	for _, part in ipairs(JAC_AGES_VIEW_PARTS) do table.insert(view.parts, { part = part, points = parts[part] or 0 }) end
	if type(house.next) == "table" then
		view.golden, view.dark = house.next.golden, house.next.dark
		-- ゲームプレイ側の Judge と同じ判定（英雄期は暗黒期の家が黄金期の閾値に届いたとき。遅代は点によらず暗黒期）。
		if house.next.forced then
			view.projected = house.next.forced
		elseif view.score >= view.golden then
			view.projected = view.age == "DARK" and "HEROIC" or "GOLDEN"
		elseif view.score >= view.dark then
			view.projected = "NORMAL"
		else
			view.projected = "DARK"
		end
	end
	for i = 1, index do
		local judged = type(l.history) == "table" and l.history["r" .. tostring(i)] or nil
		local result = type(judged) == "table" and judged["p" .. tostring(playerID)] or nil
		if type(result) == "table" then
			table.insert(view.history, { index = i, reign = reigns[i], age = result.age, score = result.score,
				golden = result.golden, dark = result.dark })
		end
	end
	return view
end

function JAC_AgesView_AgeLabel(age)
	local style = JAC_AgesView_Style(age)
	local label = Locale.Lookup(style.name)
	if style.icon ~= "" then label = label .. " " .. style.icon end
	return label
end

-- 年齢の効果の文。暗黒期は家の政策の文（公式の暗黒時代の政策の代わり。documents/research/sengoku-ages.md §7.2）。
function JAC_AgesView_EffectText(age, policy)
	if age == "DARK" then
		local row = PolicyRow(policy)
		return row and Locale.Lookup(row.Announcement) or ""
	end
	return Locale.Lookup(AGE_EFFECTS[age] or AGE_EFFECTS.NONE)
end

function JAC_AgesView_ThresholdText(golden, dark)
	return Locale.Lookup("LOC_JAC_AGE_THRESHOLDS", golden, dark)
end

-- 年は文字列で渡す（数のまま渡すと言語の数の書式になりうる）。
-- 時代スコアの画面の下段（公式の EraEffects の欄）：年齢の効果、今の期の内訳、過去の期。
function JAC_AgesView_LedgerText(view)
	local lines = { JAC_AgesView_EffectText(view.age, view.policy), "",
		Locale.Lookup("LOC_JAC_AGE_PANEL_PARTS", view.reign.Name) }
	for _, part in ipairs(view.parts) do
		table.insert(lines, "[ICON_BULLET]" .. Locale.Lookup("LOC_JAC_AGE_PART_" .. string.upper(part.part), part.points))
	end
	table.insert(lines, "")
	table.insert(lines, Locale.Lookup("LOC_JAC_AGE_PANEL_HISTORY"))
	if #view.history == 0 then
		table.insert(lines, "[ICON_BULLET]" .. Locale.Lookup("LOC_JAC_AGE_PANEL_HISTORY_NONE"))
	end
	for _, entry in ipairs(view.history) do
		table.insert(lines, "[ICON_BULLET]" .. Locale.Lookup("LOC_JAC_AGE_PANEL_HISTORY_ROW", entry.reign.Name, tostring(entry.reign.Year),
			JAC_AgesView_AgeLabel(entry.age), entry.score) .. " " .. JAC_AgesView_ThresholdText(entry.golden, entry.dark))
	end
	return table.concat(lines, "[NEWLINE]")
end

-- 公式の画面の「他の家の年齢」の一覧（EraProgressPanel.lua:273-321）を台帳の年齢で出すための、Game.GetEras() の代わり。
-- 公式の年齢は止めてある（Data/JAC_Sengoku_Ages.sql）ので、そのままでは全家が通常になる。
function JAC_AgesView_LedgerEras(l)
	local function age(id)
		local house = l.houses and l.houses["p" .. tostring(id)]
		return type(house) == "table" and house.age or "NONE"
	end
	return {
		HasHeroicGoldenAge = function(_, id) return age(id) == "HEROIC" end,
		HasGoldenAge = function(_, id) return age(id) == "GOLDEN" end,
		HasDarkAge = function(_, id) return age(id) == "DARK" end,
	}
end
