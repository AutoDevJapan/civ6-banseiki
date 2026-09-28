-- 16.1 束1：生存中の主要文明の ID と、ゲーム中の表示名変更の可否を記録する。

local RENAME_PROPERTY = "JAC_POC_CUSTOM_CIVS_RENAME_ATTEMPTED"
local RENAME_NAME_PROPERTY = "JAC_POC_CUSTOM_CIVS_RENAMED_NAME"
local TAKEDA_CIV = "CIVILIZATION_JAC_TAKEDA"
local NEW_NAME = "LOC_JAC_TAKEDA_RENAMED_NAME"

local function Log(message)
	print("[JAC_POC] " .. message)
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
		Log(string.format("player=%d civ=%s leader=%s alive=%s",
			playerID, tostring(civType), tostring(leaderType), tostring(player:IsAlive())))
		if civType == "CIVILIZATION_JAC_ODA" then odaCount = odaCount + 1 end
		if civType == TAKEDA_CIV then takedaCount = takedaCount + 1 end
	end
	Log(string.format("customCivs oda=%d takeda=%d", odaCount, takedaCount))
end

local function ReadConfig(config, stage, field, getter, key)
	local ok, value = pcall(function()
		if key then return config[getter](config, key) end
		return config[getter](config)
	end)
	if not ok or value == nil then
		Log(string.format("renameRead stage=%s field=%s api=%s result=%s error=%s",
			stage, field, getter, ok and "nil" or "api_error", ok and "none" or tostring(value)))
	end
	return ok and value or nil
end

local function LogTakedaName(stage)
	local playerID, _, config = GetTakeda()
	if playerID == nil or config == nil then
		Log(string.format("renameState stage=%s result=%s", stage,
			playerID == nil and "takeda_absent" or "config_unavailable"))
		return
	end
	local civ = ReadConfig(config, stage, "CivilizationTypeName", "GetCivilizationTypeName")
	local name = ReadConfig(config, stage, "CivilizationName", "GetValue", "CivilizationName")
	local short = ReadConfig(config, stage, "CivilizationShortDescription", "GetCivilizationShortDescription")
	Log(string.format("renameState stage=%s player=%d civ=%s name=%s short=%s target=%s attempted=%s",
		stage, playerID, tostring(civ), tostring(name), tostring(short),
		tostring(Game:GetProperty(RENAME_NAME_PROPERTY)), tostring(Game:GetProperty(RENAME_PROPERTY))))
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
			Log(string.format("turn=%d player=%d civ=%s cities=%d alive=%s",
				turn, playerID, civType, cities and cities:GetCount() or 0, tostring(player:IsAlive())))
		end
	end
	if turn ~= 3 or Game:GetProperty(RENAME_PROPERTY) then return end
	local playerID, _, config = GetTakeda()
	if playerID == nil then
		Log("rename turn=3 result=takeda_absent")
		return
	end
	Game:SetProperty(RENAME_NAME_PROPERTY, NEW_NAME)
	if config == nil then
		Log(string.format("rename turn=3 player=%d result=config_unavailable", playerID))
		Game:SetProperty(RENAME_PROPERTY, true)
		return
	end

	local civBefore = ReadConfig(config, "before", "CivilizationTypeName", "GetCivilizationTypeName")
	local nameBefore = ReadConfig(config, "before", "CivilizationName", "GetValue", "CivilizationName")
	local shortBefore = ReadConfig(config, "before", "CivilizationShortDescription", "GetCivilizationShortDescription")
	for _, field in ipairs({
		{ "CivilizationName", "SetCivilizationName" },
		{ "CivilizationShortDescription", "SetCivilizationShortDescription" },
		{ "CivilizationDescription", "SetCivilizationDescription" },
	}) do
		local key, setterName = field[1], field[2]
		local found, setter = pcall(function() return config[setterName] end)
		local accepted = false
		if found and type(setter) == "function" then
			local ok, result = pcall(setter, config, NEW_NAME)
			accepted = ok and result ~= false
			Log(string.format("renameField field=%s api=%s result=%s error=%s",
				key, setterName, accepted and "call_ok" or (ok and "rejected" or "api_error"),
				ok and "none" or tostring(result)))
		else
			Log(string.format("renameField field=%s api=%s result=%s error=%s",
				key, setterName, found and "unavailable" or "lookup_error",
				found and "none" or tostring(setter)))
		end
		if not accepted then
			local ok, result = pcall(function() return config:SetValue(key, NEW_NAME) end)
			Log(string.format("renameField field=%s api=SetValue result=%s error=%s",
				key, ok and (result == false and "rejected" or "call_ok") or "api_error",
				ok and "none" or tostring(result)))
		end
	end
	Game:SetProperty(RENAME_PROPERTY, true)
	Log(string.format("rename turn=3 player=%d result=attempted civBefore=%s civAfter=%s nameBefore=%s nameAfter=%s shortBefore=%s shortAfter=%s",
		playerID, tostring(civBefore),
		tostring(ReadConfig(config, "after", "CivilizationTypeName", "GetCivilizationTypeName")),
		tostring(nameBefore), tostring(ReadConfig(config, "after", "CivilizationName", "GetValue", "CivilizationName")),
		tostring(shortBefore), tostring(ReadConfig(config, "after", "CivilizationShortDescription", "GetCivilizationShortDescription"))))
end

Events.LoadScreenClose.Add(OnLoadScreenClose)
Events.TurnBegin.Add(OnTurnBegin)
