-- 16.1 束1：各プレイヤーの固有 ID と、ゲーム中の表示名変更の可否を記録する。

local RENAME_PROPERTY = "JAC_POC_CUSTOM_CIVS_RENAME_ATTEMPTED"
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
	if config then
		return config:GetCivilizationTypeName(), config:GetLeaderTypeName(), config
	end
	-- Config が使えない実行環境でも、ゲーム内の型 ID を記録する。
	local civOk, civIndex = pcall(function() return player:GetCivilizationType() end)
	local leaderOk, leaderIndex = pcall(function() return player:GetLeaderType() end)
	local civRow = civOk and GameInfo.Civilizations[civIndex]
	local leaderRow = leaderOk and GameInfo.Leaders[leaderIndex]
	return civRow and civRow.CivilizationType, leaderRow and leaderRow.LeaderType, nil
end

local function GetTakeda()
	for playerID = 0, GameDefines.MAX_PLAYERS - 1 do
		local player = Players[playerID]
		if player then
			local civType, _, config = GetTypes(playerID, player)
			if civType == TAKEDA_CIV then
				return playerID, player, config
			end
		end
	end
	return nil, nil, nil
end

local function LogPlayers()
	local odaCount, takedaCount = 0, 0
	for playerID = 0, GameDefines.MAX_PLAYERS - 1 do
		local player = Players[playerID]
		if player then
			local civType, leaderType = GetTypes(playerID, player)
			Log(string.format("player=%d civ=%s leader=%s alive=%s",
				playerID, tostring(civType), tostring(leaderType), tostring(player:IsAlive())))
			if civType == "CIVILIZATION_JAC_ODA" then odaCount = odaCount + 1 end
			if civType == TAKEDA_CIV then takedaCount = takedaCount + 1 end
		end
	end
	Log(string.format("customCivs oda=%d takeda=%d", odaCount, takedaCount))
end

local function LogTakedaName(stage)
	local playerID, _, config = GetTakeda()
	if playerID and config then
		local name = config:GetValue("CivilizationName")
		local short = config:GetCivilizationShortDescription()
		Log(string.format("renameState stage=%s player=%d civ=%s name=%s short=%s attempted=%s",
			stage, playerID, tostring(config:GetCivilizationTypeName()),
			tostring(name), tostring(short), tostring(Game:GetProperty(RENAME_PROPERTY))))
	end
end

local function OnLoadScreenClose()
	Log("loaded custom civs")
	LogPlayers()
	LogTakedaName("load")
end

local function OnTurnBegin(turn)
	for playerID = 0, GameDefines.MAX_PLAYERS - 1 do
		local player = Players[playerID]
		if player then
			local civType = GetTypes(playerID, player)
			if civType == "CIVILIZATION_JAC_ODA" or civType == TAKEDA_CIV then
				local cities = player:GetCities()
				Log(string.format("turn=%d player=%d civ=%s cities=%d alive=%s",
					turn, playerID, civType, cities and cities:GetCount() or 0, tostring(player:IsAlive())))
			end
		end
	end
	if turn ~= 3 or Game:GetProperty(RENAME_PROPERTY) then return end
	local playerID, _, config = GetTakeda()
	if playerID == nil then
		Log("rename turn=3 result=takeda_absent")
		return
	end
	if config == nil then
		Log(string.format("rename turn=3 player=%d result=config_unavailable", playerID))
		Game:SetProperty(RENAME_PROPERTY, true)
		return
	end

	local civBefore = config:GetCivilizationTypeName()
	local nameBefore = config:GetValue("CivilizationName")
	local shortBefore = config:GetCivilizationShortDescription()
	local method = "PlayerConfig.SetValue"
	local ok, err = pcall(function()
		local setters = {}
		for _, setterName in ipairs({ "SetCivilizationName", "SetCivilizationShortDescription", "SetCivilizationDescription" }) do
			local found, setter = pcall(function() return config[setterName] end)
			if found and type(setter) == "function" then
				setter(config, NEW_NAME)
				table.insert(setters, setterName)
			end
		end
		if #setters > 0 then
			method = "PlayerConfig." .. table.concat(setters, ",")
		else
			-- 標準の SetValue は任意キーの保存。既定 UI の表示変更は別途確認する。
			config:SetValue("CivilizationName", NEW_NAME)
			config:SetValue("CivilizationShortDescription", NEW_NAME)
			config:SetValue("CivilizationDescription", NEW_NAME)
		end
	end)
	Game:SetProperty(RENAME_PROPERTY, true)
	Log(string.format("rename turn=3 player=%d method=%s result=%s civBefore=%s civAfter=%s nameBefore=%s nameAfter=%s shortBefore=%s shortAfter=%s error=%s",
		playerID, method, ok and "write_accepted" or "api_error", tostring(civBefore),
		tostring(config:GetCivilizationTypeName()), tostring(nameBefore),
		tostring(config:GetValue("CivilizationName")), tostring(shortBefore),
		tostring(config:GetCivilizationShortDescription()), ok and "none" or tostring(err)))
end

Events.LoadScreenClose.Add(OnLoadScreenClose)
Events.TurnBegin.Add(OnTurnBegin)
