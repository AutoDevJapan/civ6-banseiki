-- 万世紀プール（SSOT 4.7）：人間が選べる指導者を台帳（設定DBの JAC_BanseikiPool）の人物に絞る。
-- 同名の SetupParameters.lua を VFS へ取り込み、別名で保持した公式モジュールの後に読む。
-- 公式の Parameter_FilterValues（Base/Assets/UI/Scripts/SetupParameters.lua:1344。重複・所有の検査）を
-- 先に呼び、その結果から台帳に無い候補を外す。重複の印（Invalid）はそのまま残る。
-- 人間の判定は PlayerConfiguration:IsHuman()。1人用・ホットシート・LAN の人間全員が同じ条件になり、
-- AI の候補は絞らない。旧来の「0番スロット＝人間」の SQL 絞り込みは Config SQL で外している。
local POOL_QUERY = "SELECT LeaderType FROM JAC_BanseikiPool"
local JAPAN_MAPS = "Maps/JAC_Japan_"

local function PoolHuman(playerId)
	if playerId == nil then return false end
	if GameConfiguration.GetValue("RULESET") ~= "RULESET_JAC_BANSEIKI" then return false end
	-- 日本固定の1人用は22当主を維持する。マルチプレイの人間は台帳から選ぶ。
	local map = MapConfiguration.GetValue("MAP_SCRIPT")
	if not GameConfiguration.IsAnyMultiplayer() and type(map) == "string" and string.find(map, JAPAN_MAPS, 1, true) then return false end
	local config = PlayerConfigurations[playerId]
	return config ~= nil and config:IsHuman() == true
end

-- MapLeaders の22人との交差で、MPの台帳候補が失われないようにする。
-- AIと他の地図・ルールセットの公式の問い合わせはそのまま実行する。
assert(SetupParameters and SetupParameters.Data_Query and SetupParameters.Parameter_FilterValues,
	"JAC_PoolSetup must be included after SetupParameters.lua")
local BaseDataQuery = SetupParameters.Data_Query
function SetupParameters:Data_Query(query)
	local map = MapConfiguration.GetValue("MAP_SCRIPT")
	if query.QueryId == "MapLeaders" and PoolHuman(self.PlayerId)
		and GameConfiguration.IsAnyMultiplayer() and type(map) == "string"
		and string.find(map, JAPAN_MAPS, 1, true) then return {} end
	return BaseDataQuery(self, query)
end

local BaseFilterValues = assert(SetupParameters and SetupParameters.Parameter_FilterValues,
	"JAC_PoolSetup must be included after SetupParameters.lua")

function SetupParameters:Parameter_FilterValues(parameter, values)
	values = BaseFilterValues(self, parameter, values)
	if parameter.ParameterId ~= "PlayerLeader" or not PoolHuman(self.PlayerId)
		or GameConfiguration.GetValue("JAC_BANSEIKI_HUMAN_POOL") ~= "JAC_BANSEIKI_ONLY" then
		return values
	end
	local pool = {}
	for _, row in ipairs(CachedQuery(POOL_QUERY)) do
		pool[row.LeaderType] = true
	end
	-- RANDOM は残す。抽選は公式どおりホストが LEADER_VALUES（この絞り込み後の一覧）から行う。
	local kept = {}
	for _, value in ipairs(values) do
		if value.Value == "RANDOM" or pool[value.Value] then
			table.insert(kept, value)
		end
	end
	return kept
end

print("[JAC_POOL] setup filter installed")
JAC_POOL_FILTER_INSTALLED = true
