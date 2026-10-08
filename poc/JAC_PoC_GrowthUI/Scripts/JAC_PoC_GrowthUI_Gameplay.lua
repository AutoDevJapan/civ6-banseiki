-- 束0・束1 PoC：Unit プロパティに武将の成長状態を保存する。
local INITIALIZED_KEY = "JAC_POC_GROWTHUI_INITIALIZED"
local WARLORD_KEY = "JAC_POC_GROWTHUI_WARLORD"
local XP_PER_TURN = 20 -- 成長式の検証用固定値。報酬量の仕様ではない。
local NORMAL_LEVEL_CAP = 30
local ROSTER_KEY = "JAC_POC_GROWTHUI_NETWORK_ROSTER"
local ACTIVE_KEY = "JAC_POC_GROWTHUI_ACTIVE_TURNS"
local PENDING_KEY = "JAC_POC_GROWTHUI_PENDING_STARTS"

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
		local expected = Game:GetProperty(ROSTER_KEY)
		if not expected then ReportWaiting("waiting for synchronized human roster"); return end
		local found = {}
		for _, player in ipairs(humans) do found[player:GetID()] = true end
		for _, id in ipairs(expected) do
			if not found[id] then ReportWaiting(); return end
		end
		if #expected == 0 or #expected ~= #humans then ReportWaiting(); return end
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

local function ApplyGrowth(playerID, turn, starts)
	local player = Players[playerID]
	if not player or not player:IsHuman() then return end
	for _, unit in ipairs(SortedUnits(player)) do
		local record = unit:GetProperty(WARLORD_KEY)
		if record then
			if record.LastXPTurn == nil or record.LastXPTurn < turn then
				record.XP = record.XP + XP_PER_TURN * (starts or 1)
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

local function CopyStarts(value)
	local result = {}
	-- Compact waiting receipts to one row per major, including old saved rows.
	-- Keep the actual start count; a long initialization wait must not lose XP.
	for _, row in pairs(value or {}) do
		local key = tostring(row.owner)
		local prior = result[key]
		if prior then
			prior.starts = prior.starts + (row.starts or 1)
			prior.turn = math.max(prior.turn, row.turn)
		else result[key] = {owner=row.owner, turn=row.turn, starts=row.starts or 1} end
	end
	return result
end

local function DrainPending()
	if not Game:GetProperty(INITIALIZED_KEY) then InitializeNewGame() end
	if not Game:GetProperty(INITIALIZED_KEY) then return end
	local rows = {}
	for _, row in pairs(CopyStarts(Game:GetProperty(PENDING_KEY))) do rows[#rows+1] = row end
	table.sort(rows, function(a,b) return a.turn < b.turn or a.turn == b.turn and a.owner < b.owner end)
	for _, row in ipairs(rows) do ApplyGrowth(row.owner, row.turn, row.starts) end
	Game:SetProperty(PENDING_KEY, nil)
end

local function OnPlayerTurnStartComplete(playerID)
	local p = type(playerID) == "number" and Players[playerID]
	if not p then return end
	local turn = Game.GetCurrentGameTurn()
	if IsNetworkGame() then
		local active = Game:GetProperty(ACTIVE_KEY) or {}; local nextActive = {}
		for key,value in pairs(active) do nextActive[key] = value end
		nextActive[tostring(playerID)] = turn
		Game:SetProperty(ACTIVE_KEY, nextActive)
		if not Game:GetProperty(INITIALIZED_KEY) then
			local pending = CopyStarts(Game:GetProperty(PENDING_KEY))
			-- Core's human flag may lag the configured roster. Keep starts for
			-- every alive major, then filter through IsHuman when applying XP.
			local major = false
			for _,alive in ipairs(PlayerManager.GetAliveMajors()) do
				if alive:GetID() == playerID then major = true; break end
			end
			if major then
				local key = tostring(playerID)
				local prior = pending[key]
				if not prior or turn > prior.turn then
					pending[key] = {owner=playerID, turn=turn, starts=(prior and prior.starts or 0)+1}
				end
			end
			Game:SetProperty(PENDING_KEY, pending)
			DrainPending()
			return
		end
	elseif not Game:GetProperty(INITIALIZED_KEY) then InitializeNewGame() end
	if Game:GetProperty(INITIALIZED_KEY) then
		-- A failed unit write may leave only part of a pending batch applied.
		-- Drain it before a newer turn can advance LastXPTurn past old receipts.
		if Game:GetProperty(PENDING_KEY) then DrainPending() end
		ApplyGrowth(playerID, turn)
	end
end

local function ReceiveRoster(actor, params)
	assert(IsNetworkGame() and type(params) == "table", "network roster operation required")
	assert(type(actor) == "number" and actor == math.floor(actor) and actor >= 0 and actor <= 63,
		"invalid roster engine actor")
	for key in pairs(params) do
		assert(key == "OnStart" or key == "Turn" or key == "Roster", "unexpected roster field")
	end
	assert((params.OnStart == nil or params.OnStart == "JAC_GrowthRoster")
		and params.Turn == Game.GetCurrentGameTurn(), "stale roster operation")
	assert(type(params.Roster) == "string" and #params.Roster <= 256, "invalid roster encoding")
	local ids, prior, majors = {}, -1, {}
	for _,p in ipairs(PlayerManager.GetAliveMajors()) do majors[p:GetID()] = p end
	for text in params.Roster:gmatch("[^,]+") do
		local id = tonumber(text)
		assert(id and id == math.floor(id) and id >= 0 and id <= 63 and tostring(id) == text
			and id > prior and majors[id], "invalid configured major player")
		ids[#ids+1], prior = id, id
	end
	assert(#ids > 0 and table.concat(ids, ",") == params.Roster, "invalid roster list")
	local listed = {}; for _,id in ipairs(ids) do listed[id] = true end
	local active, selected = Game:GetProperty(ACTIVE_KEY) or {}, nil
	for _,id in ipairs(ids) do
		if active[tostring(id)] == params.Turn and majors[id]:IsHuman()
			and majors[id]:IsTurnActive() then selected=id; break end
	end
	assert(selected ~= nil and actor == selected, "roster requires the active human engine actor")
	assert(majors[actor]:IsTurnActive(), "roster actor no longer has the active turn")
	for _,p in ipairs(HumanPlayers()) do assert(listed[p:GetID()], "human player omitted from roster") end
	if Game:GetProperty(INITIALIZED_KEY) then return true end
	local known = Game:GetProperty(ROSTER_KEY)
	-- The human coverage check above also protects updates to the saved roster.
	-- Newly configured seats may still lag Core's human flag, so continue waiting.
	if not known or table.concat(known, ",") ~= params.Roster then Game:SetProperty(ROSTER_KEY, ids) end
	DrainPending()
	return true
end

if Events and Events.LoadScreenClose and Events.LoadScreenClose.Add then
	Events.LoadScreenClose.Add(OnLoadScreenClose)
end
GameEvents.PlayerTurnStartComplete.Add(OnPlayerTurnStartComplete)
GameEvents.OnPlayerTurnEnded.Add(function(playerID)
	if not IsNetworkGame() then return end
	local nextActive = {}
	for id,turn in pairs(Game:GetProperty(ACTIVE_KEY) or {}) do
		if id ~= tostring(playerID) then nextActive[id] = turn end
	end
	Game:SetProperty(ACTIVE_KEY, nextActive)
end)
GameEvents.JAC_GrowthRoster.Add(function(actor, params)
	local ok,result = pcall(ReceiveRoster, actor, params)
	if not ok then print("[JAC_POC] network roster rejected: "..tostring(result)) end
	return ok and result == true
end)
