-- 標準 UI と同じ PlayerConfigurations のゲッターを、UI 側から観測する。
local RENAME_PROPERTY = "JAC_POC_CUSTOM_CIVS_RENAME_ATTEMPTED"
local RENAME_NAME_PROPERTY = "JAC_POC_CUSTOM_CIVS_RENAMED_NAME"
local TAKEDA_CIV = "CIVILIZATION_JAC_TAKEDA"
local observedAttempt = false

local function Log(message)
	print("[JAC_POC] " .. tostring(message))
end

-- 未設定のキーでは GetProperty が値を1つも返さないため、ローカルへ代入して必ず1値にする。
local function GetProp(key)
	local value = Game:GetProperty(key)
	return value
end

local function ReadConfig(config, field, getter, key)
	local ok, value = pcall(function()
		if key then return config[getter](config, key) end
		return config[getter](config)
	end)
	if not ok or value == nil then
		Log(string.format("uiRenameRead field=%s api=%s result=%s error=%s",
			tostring(field), tostring(getter), ok and "nil" or "api_error", ok and "none" or tostring(value)))
	end
	return ok and value or nil
end

local function Observe(stage)
	local attempted = GetProp(RENAME_PROPERTY)
	if stage ~= "load" and not attempted then return end
	if stage == "update" and observedAttempt then return end

	for _, player in ipairs(PlayerManager.GetAliveMajors()) do
		local playerID = player:GetID()
		local config = PlayerConfigurations and PlayerConfigurations[playerID]
		if config then
			local civ = ReadConfig(config, "CivilizationTypeName", "GetCivilizationTypeName")
			if civ == TAKEDA_CIV then
				local name = ReadConfig(config, "CivilizationName", "GetValue", "CivilizationName")
				local short = ReadConfig(config, "CivilizationShortDescription", "GetCivilizationShortDescription")
				local localized = nil
				if short then
					local ok, value = pcall(function() return Locale.Lookup(short) end)
					localized = ok and value or nil
					if not ok or value == nil then
						Log(string.format("uiRenameRead field=localized result=%s error=%s",
							ok and "nil" or "api_error", ok and "none" or tostring(value)))
					end
				end
				Log(string.format("uiRenameState stage=%s player=%s civ=%s name=%s short=%s localized=%s target=%s attempted=%s",
					tostring(stage), tostring(playerID), tostring(civ), tostring(name), tostring(short), tostring(localized),
					tostring(GetProp(RENAME_NAME_PROPERTY)), tostring(attempted)))
				observedAttempt = attempted == true
				return
			end
		end
	end
	Log(string.format("uiRenameState stage=%s result=takeda_unavailable attempted=%s", tostring(stage), tostring(attempted)))
end

Events.LoadScreenClose.Add(function() Observe("load") end)
Events.GameCoreEventPublishComplete.Add(function() Observe("update") end)
Events.LocalPlayerTurnBegin.Add(function() Observe("turn") end)
