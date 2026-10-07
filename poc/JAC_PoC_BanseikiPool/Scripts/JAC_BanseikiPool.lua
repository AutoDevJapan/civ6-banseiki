-- 万世紀プール（SSOT 4.7）：各プレイヤーの人物ID・特性・役割を最初の1回だけ記録する。
-- 保存・再開では記録を読み直すだけで、台帳（JAC_BanseikiPool）が後で変わっても作り直さない。
-- 台帳の版（JAC_BanseikiPoolLedger）も最初の1回だけ Game に記録し、食い違いはログに出す。
-- 読み込み時に1回、以後は各ターンの開始時に、記録の無いプレイヤーだけを埋める（全端末で同じ順に動く）。
local LEDGER = "JAC_BANSEIKI_POOL_LEDGER"
local PERSON = "JAC_POOL_PERSON_ID"
local TRAIT = "JAC_POOL_TRAIT"
local ROLE = "JAC_POOL_ROLE"

local reported = {}

local function Log(message)
	print("[JAC_POOL] " .. message)
end

-- 未設定のキーでは GetProperty が値を1つも返さないため、ローカルへ代入して nil にそろえる。
local function Get(object, key)
	local value = object:GetProperty(key)
	return value
end

local function Record()
	local version
	for row in GameInfo.JAC_BanseikiPoolLedger() do
		version = row.Version
	end
	local saved = Get(Game, LEDGER)
	if saved == nil then
		Game:SetProperty(LEDGER, version)
	elseif saved ~= version and not reported.ledger then
		reported.ledger = true
		Log("ledger saved=" .. tostring(saved) .. " current=" .. tostring(version) .. " result=kept_saved_ids")
	end
	local pool = {}
	for row in GameInfo.JAC_BanseikiPool() do
		pool[row.LeaderType] = row
	end
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do
		local id = player:GetID()
		local leader = PlayerConfigurations[id]:GetLeaderTypeName()
		local person = Get(player, PERSON)
		local row = pool[leader]
		if person ~= nil then
			if not reported[id] then
				reported[id] = true
				Log(string.format("player=%d leader=%s person=%s result=restored", id, tostring(leader), tostring(person)))
			end
		elseif row ~= nil then
			player:SetProperty(PERSON, row.PersonID)
			player:SetProperty(TRAIT, row.TraitType)
			player:SetProperty(ROLE, "LEADER")
			reported[id] = true
			Log(string.format("player=%d leader=%s person=%s result=assigned", id, leader, row.PersonID))
		end
	end
end

Record()
Events.TurnBegin.Add(Record)
