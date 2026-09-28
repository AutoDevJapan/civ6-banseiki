-- 16.1 束1：生存中の主要文明の ID と、ゲーム中の表示名変更の可否を記録する。

local RENAME_PROPERTY = "JAC_POC_CUSTOM_CIVS_RENAME_ATTEMPTED"
local RENAME_NAME_PROPERTY = "JAC_POC_CUSTOM_CIVS_RENAMED_NAME"
local TAKEDA_CIV = "CIVILIZATION_JAC_TAKEDA"
local NEW_NAME = "LOC_JAC_TAKEDA_RENAMED_NAME"

local function Log(message)
	print("[JAC_POC] " .. tostring(message))
end

-- 未設定のキーでは GetProperty が値を1つも返さず、tostring(Game:GetProperty(...)) が
-- "value expected" で落ちる。ローカルへ代入して必ず1値（nil を含む）にする。
local function GetProp(key)
	local value = Game:GetProperty(key)
	return value
end

local function GetConfig(playerID)
	if PlayerConfigurations then return PlayerConfigurations[playerID] end
	return nil
end

local function GetTypes(playerID, player)
	local config = GetConfig(playerID)
	local civType, leaderType
	if config then
		local civOk, civName = pcall(function() return config:GetCivilizationTypeName() end)
		local leaderOk, leaderName = pcall(function() return config:GetLeaderTypeName() end)
		if civOk then civType = civName end
		if leaderOk then leaderType = leaderName end
	end
	-- Config が使えない実行環境でも、ゲーム内の型 ID を記録する。
	if not civType then
		local ok, civIndex = pcall(function() return player:GetCivilizationType() end)
		local row = ok and civIndex ~= nil and GameInfo.Civilizations[civIndex]
		civType = row and row.CivilizationType
	end
	if not leaderType then
		local ok, leaderIndex = pcall(function() return player:GetLeaderType() end)
		local row = ok and leaderIndex ~= nil and GameInfo.Leaders[leaderIndex]
		leaderType = row and row.LeaderType
	end
	return civType, leaderType, config
end

local function GetTakeda()
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do
		local playerID = player:GetID()
		local civType, _, config = GetTypes(playerID, player)
		if civType == TAKEDA_CIV then
			return playerID, player, config
		end
	end
	return nil, nil, nil
end

local function LogPlayers()
	local odaCount, takedaCount = 0, 0
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do
		local playerID = player:GetID()
		local civType, leaderType = GetTypes(playerID, player)
		Log(string.format("player=%s civ=%s leader=%s alive=%s",
			tostring(playerID), tostring(civType), tostring(leaderType), tostring(player:IsAlive())))
		if civType == "CIVILIZATION_JAC_ODA" then odaCount = odaCount + 1 end
		if civType == TAKEDA_CIV then takedaCount = takedaCount + 1 end
	end
	Log(string.format("customCivs oda=%d takeda=%d", odaCount, takedaCount))
end

local function ReadConfig(config, stage, field, getter)
	local ok, value = pcall(function() return config[getter](config) end)
	if not ok or value == nil then
		Log(string.format("renameRead stage=%s field=%s api=%s result=%s error=%s",
			tostring(stage), tostring(field), tostring(getter), ok and "nil" or "api_error", ok and "none" or tostring(value)))
	end
	return ok and value or nil
end

local function LogTakedaName(stage)
	local playerID, _, config = GetTakeda()
	if playerID == nil or config == nil then
		Log(string.format("renameState stage=%s result=%s", tostring(stage),
			playerID == nil and "takeda_absent" or "config_unavailable"))
		return
	end
	local civ = ReadConfig(config, stage, "CivilizationTypeName", "GetCivilizationTypeName")
	local short = ReadConfig(config, stage, "CivilizationShortDescription", "GetCivilizationShortDescription")
	Log(string.format("renameState stage=%s player=%s civ=%s short=%s target=%s attempted=%s",
		tostring(stage), tostring(playerID), tostring(civ), tostring(short),
		tostring(GetProp(RENAME_NAME_PROPERTY)), tostring(GetProp(RENAME_PROPERTY))))
end

local function OnLoadScreenClose()
	Log("loaded custom civs")
	LogPlayers()
	LogTakedaName("load")
end

local function OnTurnBegin(turn)
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do
		local playerID = player:GetID()
		local civType = GetTypes(playerID, player)
		if civType == "CIVILIZATION_JAC_ODA" or civType == TAKEDA_CIV then
			local cities = player:GetCities()
			Log(string.format("turn=%s player=%s civ=%s cities=%d alive=%s",
				tostring(turn), tostring(playerID), tostring(civType), cities and cities:GetCount() or 0, tostring(player:IsAlive())))
		end
	end
	if turn ~= 3 or GetProp(RENAME_PROPERTY) then return end
	local playerID = GetTakeda()
	if playerID == nil then
		Log("rename turn=3 result=takeda_absent")
		return
	end
	-- 実機確認済み（2026-09-28）：ゲームプレイ側の PlayerConfigurations には
	-- SetCivilizationName 等の専用セッターも GetValue / SetValue も無い。
	-- 毎回呼び直さず、項目ごとに使えないことだけを記録する。表示名の変更は UI 側で上書きする。
	for _, field in ipairs({ "CivilizationName", "CivilizationShortDescription", "CivilizationDescription" }) do
		Log(string.format("renameField field=%s context=gameplay result=unavailable", field))
	end
	-- 目標名は UI 側の表示上書きで読むため、共有状態として保存しておく。
	Game:SetProperty(RENAME_NAME_PROPERTY, NEW_NAME)
	Game:SetProperty(RENAME_PROPERTY, true)
	Log(string.format("rename turn=3 player=%s result=unavailable target=%s", tostring(playerID), NEW_NAME))
end

Events.LoadScreenClose.Add(OnLoadScreenClose)
Events.TurnBegin.Add(OnTurnBegin)
