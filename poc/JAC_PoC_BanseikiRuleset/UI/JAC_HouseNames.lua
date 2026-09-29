-- SSOT 3.2「特定の人物・家に結びつく名前は、その家以外では一般的な名前で表示する」の表示切替。
-- データベースの Name は常に一般名のテキストキーを指す（他の家の画面へ家固有名が漏れない）。
-- ここでは各 UI コンテキストの Locale.Lookup / Locale.ToUpper / Locale.ConvertTextKey を包み、操作中のプレイヤーの文明が登録された家なら、
--   1. 一般名のキー（本体と書式文への引数。例：Locale.Lookup("LOC_UI_CIVIC_PROGRESS_COMPLETE_BLURB", civic.Name)）を家固有名のキーへ置き換え、
--   2. 結果の文に残った一般名の文字列を家固有名の文字列へ置き換える。
-- 2 は、エンジンが組み立て済みの文（通知の本文 pNotification:GetMessage()/GetSummary()、歴史的瞬間の説明など）を UI が Locale.Lookup に通すときに効く。
-- エンジンの Locale 表は読み取り専用（書き込むと "Attempt to modify read-only table"、2026-09-29 実機）なので書き換えない。
-- 代わりに、コンテキストのグローバル Locale を「包んだ関数を持ち、残りは本物へ委ねる表」に差し替える（JAC_HistoricMoments.lua の Game と同じ方式）。
-- グローバルはコンテキストごとなので、差し替えは取り付けたコンテキストの中だけに効き、同じコンテキストで後から読まれるファイルも差し替え後の Locale を見る。
-- 登録は UI/JAC_HouseNamesData_*.lua の JAC_RegisterHouseName(一般名キー, 家固有名キー, 文明ID) で行う（他 MOD が追加してよい）。
-- 各コンテキストへの取り付けは documents/poc/house-names.md を参照。

-- 一般名キー -> { 文明ID -> 家固有名キー }
JAC_HOUSE_NAMES = JAC_HOUSE_NAMES or {}

function JAC_RegisterHouseName(genericTag, houseTag, civilizationType)
	local byCivilization = JAC_HOUSE_NAMES[genericTag]
	if byCivilization == nil then
		byCivilization = {}
		JAC_HOUSE_NAMES[genericTag] = byCivilization
	end
	byCivilization[civilizationType] = houseTag
end

include("JAC_HouseNamesData_", true)

-- 操作中プレイヤーの文明。観戦・ロード画面など操作中のプレイヤーがいないときは nil（一般名を出す）。
local function GetLocalCivilization()
	local playerID = Game.GetLocalPlayer()
	if playerID == nil or playerID < 0 then return nil end
	local config = PlayerConfigurations[playerID]
	return config and config:GetCivilizationTypeName() or nil
end

function JAC_ResolveHouseTag(value)
	if type(value) ~= "string" then return value end
	local byCivilization = JAC_HOUSE_NAMES[value]
	if byCivilization == nil then return value end
	local civilization = GetLocalCivilization()
	return civilization and byCivilization[civilization] or value
end

-- 文字列の置き換え（パターンではなく文字どおり）。
local function ReplacePlain(text, from, to)
	local start = string.find(text, from, 1, true)
	if start == nil then return text end
	local parts, position = {}, 1
	while start do
		parts[#parts + 1] = string.sub(text, position, start - 1)
		parts[#parts + 1] = to
		position = start + #from
		start = string.find(text, from, position, true)
	end
	parts[#parts + 1] = string.sub(text, position)
	return table.concat(parts)
end

-- 文明ごとの { 一般名の文字列, 家固有名の文字列 } の組（表示言語で翻訳済み、大文字形も含む）。長い一般名から置き換える。
local textPairsByCivilization = {}

local function GetTextPairs(civilization)
	local cached = textPairsByCivilization[civilization]
	if cached then return cached end
	local originals = JAC_HouseNamesBaseLocale
	local found = {}
	for genericTag, byCivilization in pairs(JAC_HOUSE_NAMES) do
		local houseTag = byCivilization[civilization]
		if houseTag then
			local generic, house = originals.Lookup(genericTag), originals.Lookup(houseTag)
			-- 家固有名が一般名を含むと、置き換え済みの文をもう一度通したときに二重になるので使わない。
			if generic ~= "" and generic ~= genericTag and house ~= "" and house ~= houseTag
					and string.find(house, generic, 1, true) == nil then
				found[#found + 1] = { generic, house }
				local upperGeneric = originals.ToUpper(generic)
				if upperGeneric ~= generic then found[#found + 1] = { upperGeneric, originals.ToUpper(house) } end
			end
		end
	end
	table.sort(found, function(a, b) return #a[1] > #b[1] end)
	textPairsByCivilization[civilization] = found
	return found
end

-- 操作中の家に家固有名が1つでもあるか。無い家（一般名を見る家）の表示には何もしない。
function JAC_LocalHouseHasNames()
	if JAC_HouseNamesBaseLocale == nil then return false end
	local civilization = GetLocalCivilization()
	return civilization ~= nil and #GetTextPairs(civilization) > 0
end

-- 翻訳済みの文に含まれる一般名を、操作中の家の家固有名へ置き換える。
function JAC_ApplyHouseNames(text)
	if type(text) ~= "string" or text == "" or JAC_HouseNamesBaseLocale == nil then return text end
	local civilization = GetLocalCivilization()
	if civilization == nil then return text end
	for _, pair in ipairs(GetTextPairs(civilization)) do text = ReplacePlain(text, pair[1], pair[2]) end
	return text
end

local unpackArguments = table.unpack or unpack

local function WrapLookup(original)
	return function(tag, ...)
		local count = select("#", ...)
		if count == 0 then return JAC_ApplyHouseNames(original(JAC_ResolveHouseTag(tag))) end
		local arguments = { ... }
		for i = 1, count do arguments[i] = JAC_ResolveHouseTag(arguments[i]) end
		return JAC_ApplyHouseNames(original(JAC_ResolveHouseTag(tag), unpackArguments(arguments, 1, count)))
	end
end

-- 取り付けると、グローバル JAC_HouseNamesBaseLocale に取り付け前の本物の Locale が入る（取り付け済みの印を兼ねる）。
function JAC_InstallHouseNames(contextName)
	local prefix = "[JAC_POC] houseNames context=" .. tostring(contextName) .. " result="
	if Locale == nil then
		print(prefix .. "no_locale")
		return
	end
	-- 同じコンテキストで何度読まれても（ToolTipHelper は1つのコンテキストに複数の経路で読まれる）二重に包まない。
	if JAC_HouseNamesBaseLocale ~= nil then
		print(prefix .. "already_installed")
		return
	end
	local ok, message = pcall(function()
		local base = Locale
		local shadow = setmetatable({}, { __index = base })
		for _, name in ipairs({ "Lookup", "ToUpper", "ConvertTextKey" }) do
			if type(base[name]) == "function" then shadow[name] = WrapLookup(base[name]) end
		end
		Locale = shadow
		if Locale ~= shadow then error("global Locale was not replaced") end
		JAC_HouseNamesBaseLocale = base
	end)
	print(prefix .. (ok and "installed" or ("error " .. tostring(message))))
end
