-- House oaths; included only by the gameplay ledger (JAC_Sengoku_Ages.lua).
-- State lives in its saved Game property; this file neither saves nor subscribes to UI events.
-- Data and rationale: documents/poc/sengoku-oaths.md. Strong effects read city plot flags,
-- the already live ages mechanism (Ages.sql; official property example: DLC/CivRoyaleScenario/
-- Data/CivRoyaleScenario_Civilizations.xml:388-407). All balance values are provisional.

local BY_CIV, BY_TYPE, AGES = {}, {}, {}
local ORDERED = {}
for row in GameInfo.JAC_SengokuOaths() do
	BY_TYPE[row.OathType] = row
	table.insert(ORDERED, row.OathType)
	BY_CIV[row.CivilizationType] = BY_CIV[row.CivilizationType] or {}
	table.insert(BY_CIV[row.CivilizationType], row)
end
table.sort(ORDERED)
for _, rows in pairs(BY_CIV) do table.sort(rows, function(a, b) return a.Position < b.Position end) end
for row in GameInfo.JAC_SengokuOathAges() do
	AGES[row.OathType] = AGES[row.OathType] or {}
	AGES[row.OathType][row.AgeType] = true
end

function JAC_Oaths_Choices(house)
	local rows = {}
	for _, row in ipairs(BY_CIV[house.civ] or {}) do
		if AGES[row.OathType][house.age] then table.insert(rows, row) end
	end
	return rows
end

local function Strong(house) return house.age == "GOLDEN" or house.age == "HEROIC" end
local function Majors()
	local players = {}
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do table.insert(players, player) end
	table.sort(players, function(a, b) return a:GetID() < b:GetID() end)
	return players
end
local function Knowledge(player)
	local tech, civic = 0, 0
	for row in GameInfo.Technologies() do if player:GetTechs():HasTech(row.Index) then tech = tech + 1 end end
	for row in GameInfo.Civics() do if player:GetCulture():HasCivic(row.Index) then civic = civic + 1 end end
	return { tech = tech, civic = civic }
end

-- Deterministic preferences, using gameplay APIs only. At war favour recruitment; a poor treasury
-- favours income; a landlocked house avoids naval production. Stable Position breaks ties.
-- IsAtWarWith: DLC/NubiaScenario/Scripts/NubiaScenario.lua:86;
-- GetGoldBalance / IsCoastalLand: DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:404,270.
local function DefaultChoice(id, house)
	local player, war, coastal = Players[id], false, false
	for _, other in ipairs(Majors()) do
		if other:GetID() ~= id and player:GetDiplomacy():IsAtWarWith(other:GetID()) then war = true end
	end
	for _, city in player:GetCities():Members() do
		if Map.GetPlot(city:GetX(), city:GetY()):IsCoastalLand() then coastal = true end
	end
	local poor = player:GetTreasury():GetGoldBalance() < 50
	local best, bestScore
	for _, row in ipairs(JAC_Oaths_Choices(house)) do
		local score = 100 - row.Position
		if row.AIFocus == "WAR" and war then score = score + 100 end
		if row.AIFocus == "ECONOMY" and poor then score = score + 80 end
		if row.AIFocus == "NAVAL" then score = score + (coastal and 60 or -100) end
		if row.AIFocus == "LEARNING" and not war then score = score + 20 end
		if row.AIFocus == "GROWTH" and player:GetCities():GetCount() <= 2 then score = score + 30 end
		if bestScore == nil or score > bestScore then best, bestScore = row.OathType, score end
	end
	return best
end

local function AssignDefault(l, id, house)
	house.oaths = {}
	if house.age == "HEROIC" then
		for _, row in ipairs(JAC_Oaths_Choices(house)) do house.oaths[row.OathType] = true end
	else
		local oath = DefaultChoice(id, house)
		if oath then house.oaths[oath] = true end
	end
	house.oathChosen = l.reign
	house.oathKnowledge = Knowledge(Players[id])
	print(string.format("[JAC_OATHS] default player=%d reign=%d age=%s", id, l.reign, house.age))
end

-- Called after each judgement, before it is saved. There is no choice during the starting 天文 period.
function JAC_Oaths_Reset(l, id, house, turn)
	house.oaths, house.oathReign, house.oathChosen = {}, l.reign, nil
	house.oathDue = turn + 1
	house.oathKnowledge = Knowledge(Players[id])
	if #JAC_Oaths_Choices(house) == 0 then return end -- no fallback oath for other civilizations
	if not Players[id]:IsHuman() then AssignDefault(l, id, house) end
end

-- Humans can confirm until the next turn start. Then the same deterministic choice as the AI applies.
-- Migration of pre-oath saves: start a choice for the saved current age, without counting old research.
function JAC_Oaths_Update(l, turn)
	for _, player in ipairs(Majors()) do
		local id = player:GetID()
		local house = l.houses["p" .. tostring(id)]
		if house and l.reign > 0 and house.age ~= "NONE" then
			if house.oathReign ~= l.reign then JAC_Oaths_Reset(l, id, house, turn) end
			if house.oathChosen ~= l.reign and house.oathDue <= turn and #JAC_Oaths_Choices(house) > 0 then
				AssignDefault(l, id, house)
			end
		end
	end
end

-- Each occurrence earns one ledger point in NORMAL/DARK; no official ERA_SCORE modifier is re-enabled.
-- Called only after a base capture/relation was admitted (the ledger already deduplicates it).
function JAC_Oaths_Record(house, kind, count)
	if house == nil or Strong(house) or count <= 0 then return end
	local points = 0
	for oath in pairs(house.oaths or {}) do
		local row = BY_TYPE[oath]
		if row and row.CivilizationType == house.civ and AGES[oath][house.age] and row.QuestType == kind then points = points + count end
	end
	if points > 0 then
		house.score = house.score + points
		house.parts.oath = (house.parts.oath or 0) + points
		print(string.format("[JAC_OATHS] quest civ=%s kind=%s +%d", house.civ, kind, points))
	end
end

function JAC_Oaths_IntakeResearch(l)
	for _, player in ipairs(Majors()) do
		local house = l.houses["p" .. tostring(player:GetID())]
		if house then
			local current = Knowledge(player)
			local previous = house.oathKnowledge or current
			JAC_Oaths_Record(house, "TECH", current.tech - previous.tech)
			JAC_Oaths_Record(house, "CIVIC", current.civic - previous.civic)
			house.oathKnowledge = current
		end
	end
end

-- The engine supplies the requester to the GameEvent (EXECUTE_SCRIPT), never trust params.PlayerID.
-- A single atomic operation confirms all three in HEROIC; other ages must submit exactly one.
function JAC_Oaths_Choose(l, id, params)
	if type(params) ~= "table" then return false end
	local house = l and l.houses["p" .. tostring(id)]
	if house == nil or params.Reign ~= l.reign or house.oathReign ~= l.reign
		or house.oathChosen == l.reign or params.Oath4 ~= nil then return false end
	local picked, count = {}, 0
	for i = 1, 3 do
		-- Flat scalar parameters match the official EXECUTE_SCRIPT examples; no nested RPC table.
		local oath = params["Oath" .. i]
		if oath ~= nil then
			if type(oath) ~= "string" or picked[oath] then return false end
			local row = BY_TYPE[oath]
			if row == nil or row.CivilizationType ~= house.civ or not AGES[oath][house.age] then return false end
			picked[oath], count = true, count + 1
		end
	end
	if count ~= (house.age == "HEROIC" and 3 or 1) then return false end
	house.oaths, house.oathChosen = picked, l.reign
	house.oathKnowledge = Knowledge(Players[id]) -- no retroactive reward for research completed before choosing
	print(string.format("[JAC_OATHS] chosen player=%d reign=%d age=%s count=%d", id, l.reign, house.age, count))
	return true
end

-- All city flags are reconstructed by the ledger on turn start, conquest, founding and choice.
-- Every flag is a separate 0/1 property; an old owner's flags must be cleared on transfer.
function JAC_Oaths_WritePlot(house, plot)
	for _, oath in ipairs(ORDERED) do
		local row = BY_TYPE[oath]
		local on = house and row.CivilizationType == house.civ and Strong(house) and house.oaths and house.oaths[oath] == true
		plot:SetProperty(oath .. "_ON", on and 1 or 0)
	end
end
