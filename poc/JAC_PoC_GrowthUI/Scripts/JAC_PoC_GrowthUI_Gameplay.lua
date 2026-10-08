-- 束0・束1 PoC：Unit プロパティに武将の成長状態を保存する。
local INITIALIZED_KEY = "JAC_POC_GROWTHUI_INITIALIZED"
local WARLORD_KEY = "JAC_POC_GROWTHUI_WARLORD"
local XP_PER_TURN = 20 -- 成長式の検証用固定値。報酬量の仕様ではない。
local NORMAL_LEVEL_CAP = 30

local PERSONALITIES = {
	"JAC_PERSONALITY_BRAVE", "JAC_PERSONALITY_CALM", "JAC_PERSONALITY_CAREFUL",
	"JAC_PERSONALITY_JOLLY", "JAC_PERSONALITY_GENTLE", "JAC_PERSONALITY_BOLD",
	"JAC_PERSONALITY_FICKLE", "JAC_PERSONALITY_LOYAL"
}
local TRAITS = {
	"JAC_TRAIT_VALIANT", "JAC_TRAIT_FIERCE", "JAC_TRAIT_ASTUTE",
	"JAC_TRAIT_SHREWD", "JAC_TRAIT_DEVOUT", "JAC_TRAIT_ASCETIC",
	"JAC_TRAIT_MUNDANE", "JAC_TRAIT_ADEPT"
}

local initializedThisSession = false
local newGameReported = false
local waitingReason = nil

local function IsNetworkGame()
	return GameConfiguration and GameConfiguration.IsNetworkMultiplayer
		and GameConfiguration.IsNetworkMultiplayer() == true
end

local function ReportWaiting(reason)
	reason = reason or "waiting for human starting units and roster"
	if waitingReason ~= reason then
		print("[JAC_POC] " .. reason)
		waitingReason = reason
	end
end

local function Log(message)
	print("[JAC_POC] " .. message)
end

local function SortedUnits(player)
	local units = {}
	for _, unit in player:GetUnits():Members() do
		table.insert(units, unit)
	end
	table.sort(units, function(a, b) return a:GetID() < b:GetID() end)
	return units
end

local function HumanPlayers()
	local humans = {}
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do
		if player:IsHuman() then
			table.insert(humans, player)
		end
	end
	table.sort(humans, function(a, b) return a:GetID() < b:GetID() end)
	return humans
end

local function EquipmentText(record)
	if #record.Equipment == 0 then return "none" end
	return table.concat(record.Equipment, ",")
end

local function LogRecord(prefix, playerID, unit, record, turn)
	local suffix = turn and (" turn=" .. tostring(turn)) or ""
	Log(string.format("%sunit=%d level=%d xp=%d personality=%s trait=%s equipment=%s artifact=%s player=%d%s",
		prefix, unit:GetID(), record.Level, record.XP, record.Personality, record.Trait,
		EquipmentText(record), record.Artifact or "none", playerID, suffix))
end

local function InitializeNewGame()
	if Game:GetProperty(INITIALIZED_KEY) then return end
	if not newGameReported then
		Log("no saved state (new game)")
		newGameReported = true
	end

	local humans = HumanPlayers()
	if IsNetworkGame() then
		-- Official StagingRoom.lua uses GetParticipatingPlayerIDs and config:IsHuman.
		-- Do not initialize only the peers whose gameplay objects already arrived.
		if not GameConfiguration.GetParticipatingPlayerIDs or not PlayerConfigurations then
			ReportWaiting("network roster API unavailable; growth initialization is pending")
			return
		end
		local found, expected = {}, 0
		for _, player in ipairs(humans) do found[player:GetID()] = true end
		for _, id in ipairs(GameConfiguration.GetParticipatingPlayerIDs()) do
			local config = PlayerConfigurations[id]
			if not config then ReportWaiting(); return end
			if not config.IsHuman then
				ReportWaiting("configured human getter unavailable; growth initialization is pending")
				return
			end
			if config:IsHuman() then
				expected = expected + 1
				if not found[id] then ReportWaiting(); return end
			end
		end
		if expected == 0 or expected ~= #humans then ReportWaiting(); return end
	end
	-- Do not finalize a partial roster while a peer's starting units are pending.
	-- Preflight before writing any unit records so a later retry cannot reset XP.
	for _, player in ipairs(humans) do
		if #SortedUnits(player) == 0 then
			ReportWaiting()
			return
		end
	end
	local assigned = 0
	for _, player in ipairs(humans) do
		local units = SortedUnits(player)
		for index = 1, math.min(3, #units) do
			local record = {
				Level = 1,
				XP = 0,
				Personality = PERSONALITIES[index],
				Trait = TRAITS[index],
				Equipment = index == 1 and { "JAC_ITEM_POC_BLADE", "JAC_ITEM_POC_CLOAK" }
					or (index == 2 and { "JAC_ITEM_POC_BLADE" } or {}),
				Artifact = index == 1 and "JAC_ARTIFACT_POC_RELIC" or nil
			}
			units[index]:SetProperty(WARLORD_KEY, record)
			LogRecord("assigned ", player:GetID(), units[index], record)
			assigned = assigned + 1
		end
	end

	if assigned == 0 then
		ReportWaiting()
		return
	end

	Game:SetProperty(INITIALIZED_KEY, true)
	initializedThisSession = true
	Log("initialized count=" .. tostring(assigned))
end

local function OnLoadScreenClose()
	if not Game:GetProperty(INITIALIZED_KEY) then
		-- Each network client closes its load screen at a different time. Saved
		-- state is initialized only by the synchronized Gameplay turn event.
		if IsNetworkGame() then return end
		InitializeNewGame()
		return
	end
	if initializedThisSession then return end

	local restored = 0
	for _, player in ipairs(HumanPlayers()) do
		for _, unit in ipairs(SortedUnits(player)) do
			local record = unit:GetProperty(WARLORD_KEY)
			if record then
				LogRecord("restored ", player:GetID(), unit, record)
				restored = restored + 1
			end
		end
	end
	Log("restored count=" .. tostring(restored))
end

local function OnPlayerTurnStartComplete(playerID)
	if not Game:GetProperty(INITIALIZED_KEY) then InitializeNewGame() end
	if not Game:GetProperty(INITIALIZED_KEY) then return end

	local player = Players[playerID]
	if not player or not player:IsHuman() then return end
	local turn = Game.GetCurrentGameTurn()
	for _, unit in ipairs(SortedUnits(player)) do
		local record = unit:GetProperty(WARLORD_KEY)
		if record then
			if record.LastXPTurn ~= turn then
				record.XP = record.XP + XP_PER_TURN
				while record.Level < NORMAL_LEVEL_CAP do
					local needed = math.floor(30 * record.Level ^ 1.5 + 0.5)
					if record.XP < needed then break end
					record.XP = record.XP - needed
					record.Level = record.Level + 1
				end
				record.LastXPTurn = turn
				unit:SetProperty(WARLORD_KEY, record)
			end
			LogRecord("", playerID, unit, record, turn)
		end
	end
end

if Events and Events.LoadScreenClose and Events.LoadScreenClose.Add then
	Events.LoadScreenClose.Add(OnLoadScreenClose)
end
GameEvents.PlayerTurnStartComplete.Add(OnPlayerTurnStartComplete)
