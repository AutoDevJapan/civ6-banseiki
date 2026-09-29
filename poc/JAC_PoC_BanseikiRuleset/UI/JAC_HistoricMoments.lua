-- 歴史的瞬間（タイムラインと瞬間のポップアップ）へ家固有名の表示切替を取り付ける。
-- 瞬間の説明 InstanceDescription はエンジンが組み立て済みの文で、UI は Locale.Lookup を通さず SetText する（HistoricMoments.lua:289）。
-- そこで、このコンテキストでだけ Game.GetHistoryManager() を包み、GetMomentData（:153）と GetAllMomentsData（:217）が返す説明の一般名を置き換える。
-- ReplaceUIScript で差し替え、元のファイルをそのまま読み込む。
include("JAC_HouseNames")
JAC_InstallHouseNames("HistoricMoments")

local function ApplyToMoments(value)
	if type(value) ~= "table" then return value end
	if type(value.InstanceDescription) == "string" then value.InstanceDescription = JAC_ApplyHouseNames(value.InstanceDescription) end
	for _, moment in ipairs(value) do ApplyToMoments(moment) end
	return value
end

local function ApplyToFirst(first, ...)
	return ApplyToMoments(first), ...
end

-- 履歴マネージャーの代理。メソッド呼び出しはそのまま本物へ渡し、戻り値の説明だけを置き換える。
function JAC_HouseNamesHistoryProxy(manager)
	return setmetatable({}, { __index = function(_, key)
		local member = manager[key]
		if type(member) ~= "function" then return member end
		return function(_, ...) return ApplyToFirst(member(manager, ...)) end
	end })
end

-- エンジンの Game 表は書き換えず、このコンテキストのグローバル Game だけを差し替える。
local engineGame = Game
Game = setmetatable({
	GetHistoryManager = function(...)
		local manager = engineGame.GetHistoryManager(...)
		if manager == nil then return nil end
		return JAC_HouseNamesHistoryProxy(manager)
	end,
}, { __index = engineGame })

include("HistoricMoments")
