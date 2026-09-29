-- 武功の台帳 (SSOT 3.2, documents/research/sengoku-ages.md §5-§7): each house scores points during a period
-- (改元 to 改元) and is judged golden / normal / dark (heroic after dark) at every 改元. The official era score
-- and ages are switched off by Data/JAC_Sengoku_Ages.sql. All numbers are provisional.
--
-- Save/load and multiplayer (§6): the whole ledger is one Game property (saved with the game, as
-- poc/JAC_PoC_Bundle0/Scripts/JAC_PoC_Bundle0.lua:11-14); nothing is cached in Lua between events, so a load
-- continues from the saved record and never re-initializes. Writes happen only in gameplay script from
-- GameEvents.OnGameTurnStarted (DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246), GameEvents.CityConquered
-- (DLC/NubiaScenario/Scripts/NubiaScenario.lua:937,969; the city already belongs to the capturer there) and
-- GameEvents.CityBuilt (DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:56,255), in a fixed order (players
-- by ID, cities by plot key), without randomness or local-player checks. Table keys are strings (§6.1).
--
-- Turns are game-relative: current - GameConfiguration.GetStartTurn() + 1 (the 1534 calendar starts the engine
-- turn near 16603; same as poc/JAC_PoC_JapanMapScale/Scripts/JAC_Resources.lua:149).

local LEDGER_KEY = "JAC_AGES_LEDGER"
local FIRED_PREFIX = "JAC_EVENT_FIRED_"            -- player property of JAC_PoC_HistoricalEvents (§5.1)
local ALLIANCE_PREFIX = "JAC_1534_ALLIANCE_"       -- Game property of JAC_Japan_1534Relations.lua:37,59 (§5.4)
local GOLDEN_FLAG = "JAC_AGE_GOLDEN"               -- plot properties read by Data/JAC_Sengoku_Ages.sql

-- §5.1 points and §5.3 threshold constants (official GS: Expansion2_GlobalParameters.xml:66,126,250-264;
-- standard era length 50 = middle of 40/60, Expansion1_Eras.xml:4). Kept here because the SQL overwrites
-- the official GlobalParameters.
local POINTS = { home = 6, ko = 8, regain = 3, castle = 2, ally = 2, research = 1 }
local GOLDEN_BASE, DARK_BASE, PAST_AGE_SHIFT, STANDARD_LENGTH = 28, 14, 5, 50
-- Official era points per ledger point (§5.3). The ledger scores far less than the official era score: live
-- 2026-09-29 (15 km map, 22 AI houses, turns 1-65) the houses had 3-12 points (research ~0.08 a turn, a few
-- event points, no castles or alliances), median 5.5 with median 4 cities. The official middle of a normal age
-- for that house is (21 + 4) x 65 / 50 = 32.5, and 32.5 / 5.5 ~ 6. Provisional.
local OFFICIAL_PER_POINT = 6

local function Log(message)
	print("[JAC_AGES] " .. message)
end

-- Calendar: the stages of GameSpeed_Turns for the game speed, longest turn first (same deterministic order as
-- JAC_HistoricalEvents.lua:36-47). A 改元 is judged at the start of the turn containing its month (§4.1).
local START_YEAR = GameInfo.StartEras["ERA_ANCIENT"].Year
local SPEED = GameInfo.GameSpeeds[GameConfiguration.GetGameSpeedType()].GameSpeedType
local STAGES = {}
for row in GameInfo.GameSpeed_Turns() do
	if row.GameSpeedType == SPEED then table.insert(STAGES, row) end
end
table.sort(STAGES, function(a, b)
	if a.MonthIncrement ~= b.MonthIncrement then return a.MonthIncrement > b.MonthIncrement end
	return a.TurnsPerIncrement < b.TurnsPerIncrement
end)

local function TurnContaining(year, month)
	local target, months, turn = (year - START_YEAR) * 12 + month - 1, 0, 1
	for _, row in ipairs(STAGES) do
		local span = row.TurnsPerIncrement * row.MonthIncrement
		if target < months + span then return turn + math.floor((target - months) / row.MonthIncrement) end
		months, turn = months + span, turn + row.TurnsPerIncrement
	end
	return turn
end

local REIGNS = {}
for row in GameInfo.JAC_SengokuReigns() do
	table.insert(REIGNS, { type = row.ReignType, due = TurnContaining(row.Year, row.Month), announcement = row.Announcement })
end
table.sort(REIGNS, function(a, b)
	if a.due ~= b.due then return a.due < b.due end
	return a.type < b.type
end)

local POLICIES = {}
for row in GameInfo.JAC_SengokuDarkAgePolicies() do table.insert(POLICIES, row) end
table.sort(POLICIES, function(a, b) return a.Priority < b.Priority end)
local function AnchorFlag(policyType) return (string.gsub(policyType, "^JAC_AGE_POLICY_", "JAC_AGE_ANCHOR_")) end

local EVENT_SCORES = {}
for row in GameInfo.JAC_SengokuAgeEventScores() do table.insert(EVENT_SCORES, row) end
table.sort(EVENT_SCORES, function(a, b) return a.EventType < b.EventType end)

local AGE_ANNOUNCEMENTS = {
	GOLDEN = "LOC_JAC_AGE_GOLDEN_ANNOUNCEMENT",
	HEROIC = "LOC_JAC_AGE_HEROIC_ANNOUNCEMENT",
	NORMAL = "LOC_JAC_AGE_NORMAL_ANNOUNCEMENT",
}
for _, policy in ipairs(POLICIES) do AGE_ANNOUNCEMENTS[policy.PolicyType] = policy.Announcement end

local function ElapsedTurn()
	return Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn() + 1
end

local function SortedKeys(t)
	local keys = {}
	for k in pairs(t) do table.insert(keys, k) end
	table.sort(keys)
	return keys
end

local function AliveMajorIDs()
	local ids = {}
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do table.insert(ids, player:GetID()) end
	table.sort(ids)
	return ids
end

local function PlotIndex(x, y) return Map.GetPlot(x, y):GetIndex() end

-- Cities of every alive major: plot key -> {owner, index}, and the city count per owner.
-- ponytail: majors only (the Japan maps have no city-states and no loyalty flips, SSOT 3.1); a city held by a
-- non-major looks razed and is re-registered without points when a house takes it.
local function CurrentCities()
	local cities, counts = {}, {}
	for _, id in ipairs(AliveMajorIDs()) do
		counts[id] = 0
		for _, city in Players[id]:GetCities():Members() do  -- BlackDeathScenario.lua:136
			local index = PlotIndex(city:GetX(), city:GetY())
			cities["plot" .. index] = { owner = id, index = index, plot = Map.GetPlot(city:GetX(), city:GetY()) }
			counts[id] = counts[id] + 1
		end
	end
	return cities, counts
end

local function House(l, id) return l.houses["p" .. tostring(id)] end

-- Points once per key (§5.1); research passes no key (it is a count difference).
local function AddPoints(l, id, part, points, key)
	local house = House(l, id)
	if not house or (key and l.keys[key]) then return end
	if key then l.keys[key] = true end
	house.score = house.score + points
	house.parts[part] = (house.parts[part] or 0) + points
	Log(string.format("points player=%d part=%s +%d score=%d key=%s turn=%d", id, part, points, house.score,
		tostring(key), ElapsedTurn()))
end

-- A new castle: its first owner is the owner now (no points, §5.2). A house without a home takes the first
-- castle it founds as its home (settler starts; the fallback of §5.2, since GetOriginalOwner/IsOriginalCapital
-- have only UI-side uses, CityBannerManager.lua:1765).
local function RegisterCity(l, key, owner)
	l.cities[key] = { owner = owner, original = owner, n = 0 }
	local house = House(l, owner)
	if house and house.home == nil then house.home = key end
end

-- One capture of the castle at key by capturer (§5.1): home > former own castle > other castle, plus the
-- house's fall when the loser has no castle left. <n> counts the captures of the castle, so re-captures in
-- one turn each score, and the capture event and the turn-start difference share one record.
local function RecordCapture(l, key, capturer, loser, loserCities)
	local record = l.cities[key]
	record.n = record.n + 1
	record.owner = capturer
	local part = "castle"
	local homeOf = nil
	for _, houseKey in ipairs(SortedKeys(l.houses)) do
		if l.houses[houseKey].home == key then homeOf = houseKey end
	end
	if homeOf and homeOf ~= "p" .. tostring(capturer) then
		part = "home"
	elseif record.original == capturer then
		part = "regain"
	end
	local plotKey = string.sub(key, 5)
	AddPoints(l, capturer, part, POINTS[part], "cap:" .. plotKey .. ":" .. record.n)
	if House(l, loser) and loserCities == 0 then
		AddPoints(l, capturer, "ko", POINTS.ko, "ko:" .. tostring(loser))
	end
end

local function CountResearch(player)
	local n = 0
	local techs, culture = player:GetTechs(), player:GetCulture()
	for row in GameInfo.Technologies() do if techs:HasTech(row.Index) then n = n + 1 end end
	for row in GameInfo.Civics() do if culture:HasCivic(row.Index) then n = n + 1 end end
	return n
end

local function Faction(id)
	local civ = PlayerConfigurations[id]:GetCivilizationTypeName()
	return civ and string.match(civ, "^CIVILIZATION_JAC_(.+)$")
end

-- §5.4: a pair's state is "none", "ally" or "start". Start alliances (and their re-making) carry the pair's
-- Game property as a number and never score; after the property turns false the lingering alliance keeps its
-- state, so only an alliance made from "none" scores, once per pair and period. Same-team pairs never score.
-- HasAllied is the gameplay-side call (JAC_Japan_1534Relations.lua:76,102); GetAllianceType is UI-only (nil here).
local function IntakeAlliances(l)
	local ids = AliveMajorIDs()
	for i = 1, #ids do
		for j = i + 1, #ids do
			local a, b = ids[i], ids[j]
			local key = tostring(a) .. "-" .. tostring(b)
			local allied = Players[a]:GetDiplomacy():HasAllied(b)
			local fa, fb = Faction(a), Faction(b)
			local property = nil
			if fa and fb then
				if fa > fb then fa, fb = fb, fa end
				property = Game:GetProperty(ALLIANCE_PREFIX .. fa .. "-" .. fb)
			end
			local previous = l.allies[key]
			local state = allied and "ally" or "none"
			if type(property) == "number" then
				state = "start"
			elseif allied and previous == "none" and Players[a]:GetTeam() ~= Players[b]:GetTeam() then
				local pairKey = "ally:" .. tostring(a) .. ":" .. tostring(b) .. ":" .. tostring(l.reign)
				if not l.keys[pairKey] then
					l.keys[pairKey] = true
					AddPoints(l, a, "ally", POINTS.ally)
					AddPoints(l, b, "ally", POINTS.ally)
				end
			end
			l.allies[key] = state
		end
	end
end

local function IntakeCities(l)
	local cities, counts = CurrentCities()
	for _, key in ipairs(SortedKeys(l.cities)) do
		if cities[key] == nil then l.cities[key] = nil end  -- razed: no points (the capture event scored it)
	end
	for _, key in ipairs(SortedKeys(cities)) do
		local owner = cities[key].owner
		local record = l.cities[key]
		if record == nil then
			RegisterCity(l, key, owner)
		elseif record.owner ~= owner then
			local loser = record.owner
			RecordCapture(l, key, owner, loser, counts[loser] or 0)
		end
	end
end

local function IntakeResearch(l)
	for _, id in ipairs(AliveMajorIDs()) do
		local house = House(l, id)
		if house then
			local n = CountResearch(Players[id])
			if n > house.research then AddPoints(l, id, "research", (n - house.research) * POINTS.research) end
			house.research = n
		end
	end
end

-- Flags whose turn is before `beforeTurn` (all when nil). At a 改元 the flags of earlier turns go to the old
-- period and the 改元 turn's own flags to the new one, whichever script runs first that turn (§5.3).
local function IntakeEvents(l, beforeTurn)
	for _, row in ipairs(EVENT_SCORES) do
		for _, id in ipairs(AliveMajorIDs()) do
			local fired = Players[id]:GetProperty(FIRED_PREFIX .. row.EventType)
			if type(fired) == "number" and (beforeTurn == nil or fired < beforeTurn) then
				AddPoints(l, id, "event", row.Score, "evt:" .. row.EventType .. ":" .. tostring(id))
			end
		end
	end
end

-- §7.1: rewrite the plot properties of every city centre from the ledger. Idempotent; the anchor is the
-- house's city with the smallest plot index. City properties per SetProperty on plots as
-- DLC/CivRoyaleScenario/Scripts/CivRoyaleScenario_StartScript.lua:496-519.
local function WriteProperties(l)
	local cities = CurrentCities()
	local anchors = {}
	for key, city in pairs(cities) do
		local current = anchors[city.owner]
		if current == nil or city.index < cities[current].index then anchors[city.owner] = key end
	end
	for _, key in ipairs(SortedKeys(cities)) do
		local city = cities[key]
		local house = House(l, city.owner)
		local age = house and house.age or "NONE"
		local policy = house and house.policy or ""
		city.plot:SetProperty(GOLDEN_FLAG, (age == "GOLDEN" or age == "HEROIC") and 1 or 0)
		for _, row in ipairs(POLICIES) do
			local on = policy == row.PolicyType
			city.plot:SetProperty(row.PolicyType, on and 1 or 0)
			city.plot:SetProperty(AnchorFlag(row.PolicyType), (on and anchors[city.owner] == key) and 1 or 0)
		end
	end
end

-- LOC keys, not looked-up text: each machine shows its own language (JAC_HistoricalEvents.lua:158-167).
-- USER_DEFINED_2 has its own icon (Data/JAC_Sengoku_Ages_Icons.xml).
local function Notify(id, reign, age, policy)
	local summary = AGE_ANNOUNCEMENTS[age == "DARK" and policy or age]
	NotificationManager.SendNotification(id, NotificationTypes.USER_DEFINED_2,
		{ [ParameterTypes.MESSAGE] = reign.announcement, [ParameterTypes.SUMMARY] = summary })
end

-- §5.3: thresholds scale with the period's length L; shift = cities + 5 x past golden - 5 x past dark.
-- Golden stays above dark, so a short period keeps a normal age between them.
local function Thresholds(house, cities, length)
	local shift = cities + PAST_AGE_SHIFT * house.golden - PAST_AGE_SHIFT * house.dark
	local per = STANDARD_LENGTH * OFFICIAL_PER_POINT
	local dark = math.max(1, math.ceil((DARK_BASE + shift) * length / per))
	return math.max(dark + 1, math.ceil((GOLDEN_BASE + shift) * length / per)), dark
end

local function Judge(l, index)
	local reign = REIGNS[index]
	local length = reign.due - l.reignStart
	local _, counts = CurrentCities()
	local results = { turn = ElapsedTurn(), due = reign.due, length = length }
	for _, id in ipairs(AliveMajorIDs()) do
		local house = House(l, id)
		if house then
			local golden, dark = Thresholds(house, counts[id] or 0, length)
			local age = "DARK"
			if house.score >= golden then
				age = house.age == "DARK" and "HEROIC" or "GOLDEN"
				house.golden = house.golden + 1
			elseif house.score >= dark then
				age = "NORMAL"
			else
				house.dark = house.dark + 1
			end
			-- The dark-age policy choice screen is a follow-up; every dark house takes the first policy (§6.2).
			house.policy = age == "DARK" and POLICIES[1].PolicyType or ""
			house.age = age
			results["p" .. tostring(id)] = { age = age, score = house.score, golden = golden, dark = dark }
			Log(string.format("judged reign=%s turn=%d length=%d player=%d age=%s score=%d golden>=%d dark>=%d policy=%s",
				reign.type, results.turn, length, id, age, house.score, golden, dark, house.policy))
			house.score, house.parts = 0, {}
			Notify(id, reign, age, house.policy)
		end
	end
	l.history["r" .. tostring(index)] = results
	l.reign, l.reignStart = index, reign.due
end

local function Initialize()
	local turn = ElapsedTurn()
	local l = { version = 1, reign = 0, reignStart = 1, houses = {}, cities = {}, allies = {}, keys = {}, history = {} }
	-- A ledger started mid-game (the pack added to a save) skips the 改元 already passed.
	while REIGNS[l.reign + 1] and REIGNS[l.reign + 1].due <= turn do
		l.reign, l.reignStart = l.reign + 1, REIGNS[l.reign + 1].due
	end
	for _, id in ipairs(AliveMajorIDs()) do
		local player = Players[id]
		local house = { civ = PlayerConfigurations[id]:GetCivilizationTypeName(), score = 0, parts = {},
			golden = 0, dark = 0, age = "NONE", policy = "", research = CountResearch(player) }
		l.houses["p" .. tostring(id)] = house
		local capital = player:GetCities():GetCapitalCity()  -- BlackDeathScenario.lua:1103
		if capital then
			local key = "plot" .. PlotIndex(capital:GetX(), capital:GetY())
			RegisterCity(l, key, id)
		end
	end
	local cities = CurrentCities()
	for _, key in ipairs(SortedKeys(cities)) do
		if l.cities[key] == nil then RegisterCity(l, key, cities[key].owner) end
	end
	IntakeAlliances(l)  -- records the alliances of the start without points
	Log(string.format("initialized turn=%d houses=%d cities=%d reign=%d", turn, #SortedKeys(l.houses),
		#SortedKeys(l.cities), l.reign))
	return l
end

-- Initialized at the first turn start, after LuaEvents.NewGameInitialized has granted the starting knowledge
-- (JAC_Sengoku_StartingKnowledge.lua) and made the start alliances, so neither scores.
local function OnGameTurnStarted()
	local l = Game:GetProperty(LEDGER_KEY)
	if l == nil then
		l = Initialize()
	else
		local turn = ElapsedTurn()
		IntakeCities(l)
		IntakeAlliances(l)
		IntakeResearch(l)
		while REIGNS[l.reign + 1] and REIGNS[l.reign + 1].due <= turn do
			IntakeEvents(l, REIGNS[l.reign + 1].due)
			Judge(l, l.reign + 1)
		end
		IntakeEvents(l, nil)
	end
	WriteProperties(l)
	Game:SetProperty(LEDGER_KEY, l)
end

local function OnCityConquered(capturerID, ownerID, cityID, x, y)
	local l = Game:GetProperty(LEDGER_KEY)
	if l == nil then return end  -- before the first turn start; the initialization records the state
	local key = "plot" .. PlotIndex(x, y)
	if l.cities[key] == nil then RegisterCity(l, key, ownerID) end
	if l.cities[key].owner ~= capturerID then
		RecordCapture(l, key, capturerID, ownerID, Players[ownerID]:GetCities():GetCount())
	end
	WriteProperties(l)
	Game:SetProperty(LEDGER_KEY, l)
end

-- Also fires on CityManager.TransferCity (live 2026-09-29, documents/poc/unification-victory.md), so it never
-- scores: it registers a new castle and rewrites the properties; a transfer is scored by the turn-start diff.
local function OnCityBuilt(playerID, cityID, x, y)
	local l = Game:GetProperty(LEDGER_KEY)
	if l == nil then return end  -- map-placed cities: the initialization records them
	local key = "plot" .. PlotIndex(x, y)
	if l.cities[key] == nil then RegisterCity(l, key, playerID) end
	WriteProperties(l)
	Game:SetProperty(LEDGER_KEY, l)
end

Log(string.format("loaded reigns=%d first=%s@%d speed=%s", #REIGNS, REIGNS[1] and REIGNS[1].type or "none",
	REIGNS[1] and REIGNS[1].due or -1, SPEED))
GameEvents.OnGameTurnStarted.Add(OnGameTurnStarted)
GameEvents.CityConquered.Add(OnCityConquered)
GameEvents.CityBuilt.Add(OnCityBuilt)
