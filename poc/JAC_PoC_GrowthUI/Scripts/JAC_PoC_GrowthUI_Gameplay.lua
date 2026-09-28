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
local waitingReported = false

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
		if not waitingReported then
			Log("waiting for human starting units")
			waitingReported = true
		end
		return
	end

	Game:SetProperty(INITIALIZED_KEY, true)
	initializedThisSession = true
	Log("initialized count=" .. tostring(assigned))
end

local function OnLoadScreenClose()
	if not Game:GetProperty(INITIALIZED_KEY) then
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

Events.LoadScreenClose.Add(OnLoadScreenClose)
GameEvents.PlayerTurnStartComplete.Add(OnPlayerTurnStartComplete)
