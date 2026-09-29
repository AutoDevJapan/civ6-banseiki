-- SSOT 3.2「特定の人物・家に結びつく名前は、その家以外では一般的な名前で表示する」の表示切替。
-- データベースの Name は常に一般名のテキストキーを指す（エンジンが組み立てる文は一般名になり、家名が漏れない）。
-- ここでは各 UI コンテキストの Locale.Lookup / Locale.ToUpper / Locale.ConvertTextKey を包み、
-- 操作中のプレイヤーの文明が登録された家なら、一般名のキーを家固有名のキーへ置き換える。
-- 置き換えるのはキー本体と、書式文への引数に渡されたキー（例：Locale.Lookup("LOC_UI_CIVIC_PROGRESS_COMPLETE_BLURB", civic.Name)）。
-- 登録は UI/JAC_HouseNamesData_*.lua の JAC_RegisterHouseName(一般名キー, 家固有名キー, 文明ID) で行う（他 MOD が追加してよい）。
-- 各コンテキストへの取り付けは documents/poc/house-names.md を参照。
local INSTALLED_FLAG = "JAC_HouseNamesInstalled"

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

local function ResolveTag(value)
	if type(value) ~= "string" then return value end
	local byCivilization = JAC_HOUSE_NAMES[value]
	if byCivilization == nil then return value end
	local civilization = GetLocalCivilization()
	return civilization and byCivilization[civilization] or value
end

local unpackArguments = table.unpack or unpack

local function WrapLookup(original)
	return function(tag, ...)
		local count = select("#", ...)
		if count == 0 then return original(ResolveTag(tag)) end
		local arguments = { ... }
		for i = 1, count do arguments[i] = ResolveTag(arguments[i]) end
		return original(ResolveTag(tag), unpackArguments(arguments, 1, count))
	end
end

function JAC_InstallHouseNames(contextName)
	if Locale == nil then
		print("[JAC_POC] houseNames context=" .. tostring(contextName) .. " result=no_locale")
		return
	end
	-- Locale の表がコンテキスト間で共有されていても二重に包まない。
	if Locale[INSTALLED_FLAG] then
		print("[JAC_POC] houseNames context=" .. tostring(contextName) .. " result=shared_with_" .. tostring(Locale[INSTALLED_FLAG]))
		return
	end
	local ok, message = pcall(function()
		for _, name in ipairs({ "Lookup", "ToUpper", "ConvertTextKey" }) do
			if type(Locale[name]) == "function" then Locale[name] = WrapLookup(Locale[name]) end
		end
		Locale[INSTALLED_FLAG] = tostring(contextName)
	end)
	print("[JAC_POC] houseNames context=" .. tostring(contextName) .. " result=" .. (ok and "installed" or ("error " .. tostring(message))))
end
