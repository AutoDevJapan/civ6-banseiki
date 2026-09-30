-- 武功の台帳 (SSOT 3.2, documents/research/sengoku-ages.md §5-§7): each house scores points during a period
-- (改元 to 改元) and is judged golden / normal / dark (heroic after dark) at every 改元. The official era score
-- and ages are switched off by Data/JAC_Sengoku_Ages.sql. All numbers are provisional.
--
-- Save/load and multiplayer (§6): the whole ledger is one Game property (saved with the game, as
-- poc/JAC_PoC_Bundle0/Scripts/JAC_PoC_Bundle0.lua:11-14); nothing is cached in Lua between events, so a load
-- continues from the saved record and never re-initializes. Writes happen only in gameplay script from
-- GameEvents.OnGameTurnStarted (DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246), GameEvents.CityConquered
-- (DLC/NubiaScenario/Scripts/NubiaScenario.lua:937,969; the city already belongs to the capturer there) and
-- GameEvents.CityBuilt (DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:56,255) and the dark-age policy
-- choice (below), in a fixed order (players by ID, cities by plot key), without randomness or local-player checks.
-- Table keys are strings (§6.1).
--
-- Dark-age policy choice (§6.2, §7.2): a dark house gets its default policy at the 改元 (its row of
-- JAC_SengokuDarkAgePolicyDefaults, else the lowest Priority), so AI houses and humans who never answer keep it.
-- A human chooses on UI/JAC_SengokuDarkPolicy.lua, which sends UI.RequestPlayerOperation(local,
-- PlayerOperations.EXECUTE_SCRIPT, {OnStart = "JAC_SengokuDarkPolicyChoose", PolicyType, Reign}); the engine raises
-- that GameEvent on every machine with the requesting player first (DLC/BlackDeathScenario/UI/
-- BlackDeathScenario_PopupHandler.lua:21-28, Scripts/BlackDeathScenario_StateUtils.lua:64-74; live in this repo for
-- the court arbitration, documents/poc/court-arbitration.md). The requester is that engine-supplied player, never a parameter.
--
-- Turns are game-relative: current - GameConfiguration.GetStartTurn() + 1 (the 1534 calendar starts the engine
-- turn near 16603; same as poc/JAC_PoC_JapanMapScale/Scripts/JAC_Resources.lua:149).

include("JAC_Sengoku_Oaths")

local LEDGER_KEY = "JAC_AGES_LEDGER"
local FIRED_PREFIX = "JAC_EVENT_FIRED_"            -- player property of JAC_PoC_HistoricalEvents (§5.1)
local ALLIANCE_PREFIX = "JAC_1534_ALLIANCE_"       -- Game property of JAC_Japan_1534Relations.lua:37,59 (§5.4)
local GOLDEN_FLAG = "JAC_AGE_GOLDEN"               -- plot properties read by Data/JAC_Sengoku_Ages.sql

-- §5.1 points and §5.3 threshold constants (official GS: Expansion2_GlobalParameters.xml:66,126,250-264;
-- standard era length 50 = middle of 40/60, Expansion1_Eras.xml:4). Kept here because the SQL overwrites
-- the official GlobalParameters.
-- friend: a declared friendship made during the game, in the alliance part (owner decision L-2, 2026-09-30).
local POINTS = { home = 6, ko = 8, regain = 3, castle = 2, ally = 2, friend = 1, research = 1 }
local GOLDEN_BASE, DARK_BASE, PAST_AGE_SHIFT, STANDARD_LENGTH = 28, 14, 5, 50
-- Official era points per ledger point (§5.3). The ledger scores far less than the official era score: live
-- 2026-09-29 (15 km map, 22 AI houses, turns 1-65) the houses had 3-12 points (research ~0.08 a turn, a few
-- event points, no castles or alliances), median 5.5 with median 4 cities. The official middle of a normal age
-- for that house is (21 + 4) x 65 / 50 = 32.5, and 32.5 / 5.5 ~ 6. Provisional.
local OFFICIAL_PER_POINT = 6

local function Log(message)
	print("[JAC_AGES] " .. message)
end

-- 元号 = 時代 (owner decision T, documents/poc/era-equals-reign.md): each reign is one engine era
-- (Data/JAC_Sengoku_Eras.sql) and a 改元 is judged when its era starts. ERA_START: era Index -> the game-relative turn
-- the engine starts it, from the same Eras_XP1 lengths the engine uses: scaled by the speed's CostMultiplier and
-- rounded down, at least 2 turns (probe 2026-09-30, standard and quick). At standard speed this is the turn
-- containing the 改元's month.
local MULTIPLIER = GameInfo.GameSpeeds[GameConfiguration.GetGameSpeedType()].CostMultiplier
local ERA_START = {}
do
	local eras = {}
	for row in GameInfo.Eras() do table.insert(eras, row) end
	table.sort(eras, function(a, b) return a.ChronologyIndex < b.ChronologyIndex end)
	local turn = 1
	for _, era in ipairs(eras) do
		ERA_START[era.Index] = turn
		local xp1 = GameInfo.Eras_XP1[era.EraType]
		if xp1 == nil or xp1.GameEraMaximumTurns == nil then break end  -- the last era never ends
		turn = turn + math.max(2, math.floor(xp1.GameEraMaximumTurns * MULTIPLIER / 100))
	end
end

-- Date order (the UI's order, UI/JAC_SengokuAgesView.lua), which is the era order. era: the engine era's Index.
-- forced: the age of every house at that 改元 (遅代: DARK, SSOT 3.2).
local REIGNS = {}
for row in GameInfo.JAC_SengokuReigns() do
	local era = GameInfo.Eras[row.EraType].Index
	table.insert(REIGNS, { type = row.ReignType, date = row.Year * 12 + row.Month, era = era, due = ERA_START[era],
		notification = row.Notification, forced = row.ForcedAge })
end
table.sort(REIGNS, function(a, b)
	if a.date ~= b.date then return a.date < b.date end
	return a.type < b.type
end)

-- The next 改元 after the period `reign`, or nil after the last.
local function NextJudged(reign)
	return REIGNS[reign + 1] and reign + 1 or nil
end

-- The engine's current era (GameCore has Game.GetEras():GetCurrentEra(); probe 2026-09-30), nil without it.
local function EngineEra()
	local ok, era = pcall(function() return Game.GetEras():GetCurrentEra() end)
	return ok and type(era) == "number" and era or nil
end

-- A 改元 is due when its era's start turn has come or the engine is already in its era (either way deterministic;
-- the turn covers a turn-start event that runs before the engine changes the era, the era a length the formula
-- does not predict).
local function IsDue(reign, turn, engineEra)
	return reign.due <= turn or (engineEra ~= nil and engineEra >= reign.era)
end

-- The turn the period before `reign` ends: its era's start, or now if the engine changed the era earlier.
local function EndTurn(reign, turn)
	return math.min(reign.due, turn)
end

local POLICIES = {}
for row in GameInfo.JAC_SengokuDarkAgePolicies() do table.insert(POLICIES, row) end
table.sort(POLICIES, function(a, b) return a.Priority < b.Priority end)
local POLICY_BY_TYPE = {}
for _, row in ipairs(POLICIES) do POLICY_BY_TYPE[row.PolicyType] = row end
local DEFAULT_POLICY = {}  -- CivilizationType -> PolicyType
for row in GameInfo.JAC_SengokuDarkAgePolicyDefaults() do DEFAULT_POLICY[row.CivilizationType] = row.PolicyType end
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

-- Every alive player's ID, sorted. PlayerManager.GetAlive has the city-states and the free cities too
-- (gameplay use: DLC/NubiaScenario/Scripts/NubiaScenario.lua:83).
local function AlivePlayerIDs()
	local ids = {}
	for _, player in ipairs(PlayerManager.GetAlive()) do table.insert(ids, player:GetID()) end
	table.sort(ids)
	return ids
end

-- Cities of every alive player: plot key -> {owner, index}, and the city count per owner. The pack runs on the
-- Gathering Storm maps too (SSOT 3.1), where a castle can pass to a city-state or the free cities; its record
-- keeps its history there, and only a house (a major with a ledger entry) scores (AddPoints).
local function CurrentCities()
	local cities, counts = {}, {}
	for _, id in ipairs(AlivePlayerIDs()) do
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
	if part == "home" or part == "regain" or part == "castle" then
		JAC_Oaths_Record(house, "CAPTURE", 1)
	elseif part == "ally" then
		JAC_Oaths_Record(house, "RELATION", 1)
	end
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

-- Gameplay-side friendship: the DLL exports HasDeclaredFriendship (GameCore_XP2_FinalRelease.dll strings) but no
-- official script calls it, and GetDeclaredFriendshipTurn was nil in gameplay live (documents/poc/
-- japan-1534-relations.md). Without the call, friendships score nothing and the log says so once (要実機確認).
local m_NoFriendshipLogged = false
local function Friends(diplomacy, other)
	if type(diplomacy.HasDeclaredFriendship) ~= "function" then
		if not m_NoFriendshipLogged then Log("no gameplay HasDeclaredFriendship: friendships are not scored") end
		m_NoFriendshipLogged = true
		return nil
	end
	return diplomacy:HasDeclaredFriendship(other) == true
end

-- One relation of a pair (§5.4): its state is "none", "on" or "start". A start relation carries the pair's Game
-- property as a number while the relation holds and never scores; the property turns false when the start
-- alliance lapses (not re-made, owner decision K) or an event ends it. A lingering relation after false keeps its
-- state, so only a relation made from "none" scores, once per pair, kind and period; a pair that re-forms after
-- the lapse scores. Checking the relation with the number makes the lapse turn read "none" whether the ledger runs
-- before or after JAC_Japan_1534Relations.lua. Same-team pairs never score. Returns the new state.
local function TrackRelation(l, kind, a, b, on, start, previous, sameTeam)
	if start and on then return "start" end
	if on and previous == "none" and not sameTeam then
		local pairKey = kind .. ":" .. tostring(a) .. ":" .. tostring(b) .. ":" .. tostring(l.reign)
		if not l.keys[pairKey] then
			l.keys[pairKey] = true
			AddPoints(l, a, "ally", POINTS[kind])
			AddPoints(l, b, "ally", POINTS[kind])
		end
	end
	return on and "on" or "none"
end

-- §5.4: alliances (2 points each) and declared friendships (1 point each, owner decision L-2), both in the
-- alliance part. The start relations of JAC_Japan_1534Relations.lua (friendship, then alliance, and the pair's
-- property) never score. HasAllied is the gameplay-side call (JAC_Japan_1534Relations.lua TrackStartAlliances);
-- GetAllianceType is UI-only (nil here).
local function IntakeAlliances(l)
	l.friends = l.friends or {}  -- ledgers saved before the friendship points
	local ids = AliveMajorIDs()
	for i = 1, #ids do
		for j = i + 1, #ids do
			local a, b = ids[i], ids[j]
			local key = tostring(a) .. "-" .. tostring(b)
			local diplomacy = Players[a]:GetDiplomacy()
			local fa, fb = Faction(a), Faction(b)
			local start = false
			if fa and fb then
				if fa > fb then fa, fb = fb, fa end
				start = type(Game:GetProperty(ALLIANCE_PREFIX .. fa .. "-" .. fb)) == "number"
			end
			local sameTeam = Players[a]:GetTeam() == Players[b]:GetTeam()
			l.allies[key] = TrackRelation(l, "ally", a, b, diplomacy:HasAllied(b), start, l.allies[key], sameTeam)
			local friends = Friends(diplomacy, b)
			if friends ~= nil then
				l.friends[key] = TrackRelation(l, "friend", a, b, friends, start, l.friends[key], sameTeam)
			end
		end
	end
end

local function IntakeCities(l)
	local cities, counts = CurrentCities()
	for _, key in ipairs(SortedKeys(l.cities)) do
		-- Gone from every player's list: razed, no points (the capture event scored it).
		if cities[key] == nil then l.cities[key] = nil end
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
		local policy = house and house.policy
		JAC_Oaths_WritePlot(house, city.plot)
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
-- The heading names the new era ("改元：弘治の世"; owner decision L-1: the 元号 is the era wherever one is named).
local function Notify(id, reign, age, policy)
	local summary = AGE_ANNOUNCEMENTS[age == "DARK" and policy or age]
	NotificationManager.SendNotification(id, NotificationTypes.USER_DEFINED_2,
		{ [ParameterTypes.MESSAGE] = reign.notification, [ParameterTypes.SUMMARY] = summary })
end

-- §5.3: thresholds scale with the period's length L; shift = cities + 5 x past golden - 5 x past dark.
-- Dark is rounded down and may be 0: a house is dark only when it misses a whole expected point. In a short
-- period the expectation is under one point (live 2026-09-30, health-200-01: 7-11 turns scored 0-1 points),
-- and the old minimum of 1 made 10-14 of 22 houses dark, nearly all on a score of 0. Golden needs at least
-- MIN_GOLDEN points, so one tech in a short period stays normal. Golden stays above dark
-- (ceil(golden expectation) > floor(dark expectation)), so a normal age is always between them.
local MIN_GOLDEN = 2
local function Thresholds(house, cities, length)
	local shift = cities + PAST_AGE_SHIFT * house.golden - PAST_AGE_SHIFT * house.dark
	local per = STANDARD_LENGTH * OFFICIAL_PER_POINT
	local dark = math.max(0, math.floor((DARK_BASE + shift) * length / per))
	return math.max(MIN_GOLDEN, math.ceil((GOLDEN_BASE + shift) * length / per)), dark
end

local function Judge(l, index, engineEra)
	local reign = REIGNS[index]
	local endTurn = EndTurn(reign, ElapsedTurn())
	local length = endTurn - l.reignStart
	local _, counts = CurrentCities()
	-- from: the period judged (its 改元; 0 is 天文). engineEra: the engine's era then (nil without Game.GetEras).
	local results = { turn = ElapsedTurn(), due = reign.due, length = length, from = l.reign, forced = reign.forced,
		engineEra = engineEra }
	for _, id in ipairs(AliveMajorIDs()) do
		local house = House(l, id)
		if house then
			local golden, dark = Thresholds(house, counts[id] or 0, length)
			local age = "DARK"
			if reign.forced == "DARK" then
				house.dark = house.dark + 1  -- 遅代: dark whatever the score, so never heroic (SSOT 3.2)
			elseif house.score >= golden then
				age = house.age == "DARK" and "HEROIC" or "GOLDEN"
				house.golden = house.golden + 1
			elseif house.score >= dark then
				age = "NORMAL"
			else
				house.dark = house.dark + 1
			end
			-- The house's default until it chooses (§6.2). No policy is nil, not "": live 2026-09-29 (ages-impl-02)
			-- a NORMAL house logged policy= here, yet the policy read back from the Game property on the next turn
			-- was not "" (a "" field does not survive).
			house.policy = age == "DARK" and (DEFAULT_POLICY[house.civ] or POLICIES[1].PolicyType) or nil
			house.age = age
			results["p" .. tostring(id)] = { age = age, score = house.score, golden = golden, dark = dark }
			Log(string.format("judged reign=%s turn=%d length=%d era=%d engineEra=%s player=%d age=%s score=%d golden>=%d dark>=%d policy=%s%s",
				reign.type, results.turn, length, reign.era, tostring(engineEra), id, age, house.score, golden, dark,
				house.policy or "", reign.forced and " forced" or ""))
			house.score, house.parts = 0, {}
			Notify(id, reign, age, house.policy)
		end
	end
	l.history["r" .. tostring(index)] = results
	l.reign, l.reignStart = index, endTurn
	for _, id in ipairs(AliveMajorIDs()) do
		local house = House(l, id)
		if house then JAC_Oaths_Reset(l, id, house, ElapsedTurn()) end
	end
end

-- For the era score screen (UI only reads the ledger): the next 改元's turn and each house's thresholds for the
-- current period with its cities at this turn start, by the same Thresholds as the judgement, and the forced age of
-- that 改元 (遅代). None after the last.
local function Project(l)
	local nextIndex = NextJudged(l.reign)
	local nextReign = nextIndex and REIGNS[nextIndex]
	l.nextDue = nextReign and nextReign.due or nil
	local _, counts = CurrentCities()
	for _, key in ipairs(SortedKeys(l.houses)) do
		local house = l.houses[key]
		house.next = nil
		if nextReign then
			local golden, dark = Thresholds(house, counts[tonumber(string.sub(key, 2))] or 0, nextReign.due - l.reignStart)
			house.next = { golden = golden, dark = dark, forced = nextReign.forced }
		end
	end
end

local function Initialize()
	local turn = ElapsedTurn()
	local l = { version = 1, reign = 0, reignStart = 1, houses = {}, cities = {}, allies = {}, friends = {}, keys = {},
		history = {} }
	-- A ledger started mid-game (the pack added to a save) skips the 改元 already passed.
	local engineEra = EngineEra()
	local index = NextJudged(l.reign)
	while index and IsDue(REIGNS[index], turn, engineEra) do
		l.reign, l.reignStart = index, EndTurn(REIGNS[index], turn)
		index = NextJudged(l.reign)
	end
	for _, id in ipairs(AliveMajorIDs()) do
		local player = Players[id]
		local house = { civ = PlayerConfigurations[id]:GetCivilizationTypeName(), score = 0, parts = {},
			golden = 0, dark = 0, age = "NONE", research = CountResearch(player) }
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
	IntakeAlliances(l)  -- records the alliances and friendships of the start without points
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
		JAC_Oaths_Update(l, turn)
		IntakeCities(l)
		IntakeAlliances(l)
		IntakeResearch(l)
		JAC_Oaths_IntakeResearch(l)
		local engineEra = EngineEra()
		local index = NextJudged(l.reign)
		while index and IsDue(REIGNS[index], turn, engineEra) do
			IntakeEvents(l, EndTurn(REIGNS[index], turn))
			Judge(l, index, engineEra)
			index = NextJudged(l.reign)
		end
		IntakeEvents(l, nil)
	end
	Project(l)
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

-- A human's choice (header). Taken only from a house judged dark in the current period, once per 改元
-- (<chosen> holds the period's number), for the period the screen showed, and for a policy of the table;
-- anything else is dropped and logged.
local function OnChoosePolicy(playerID, params)
	local l = Game:GetProperty(LEDGER_KEY)
	local house = l and House(l, playerID)
	params = params or {}
	local policy = params.PolicyType
	local reason = nil
	if house == nil then
		reason = "no house"
	elseif house.age ~= "DARK" then
		reason = "not dark"
	elseif params.Reign ~= l.reign then
		reason = "stale period " .. tostring(params.Reign)
	elseif house.chosen == l.reign then
		reason = "already chosen"
	elseif POLICY_BY_TYPE[policy] == nil then
		reason = "unknown policy"
	end
	if reason then
		Log(string.format("choice dropped player=%d policy=%s: %s", playerID, tostring(policy), reason))
		return
	end
	house.policy, house.chosen = policy, l.reign
	Log(string.format("chosen player=%d reign=%d policy=%s turn=%d", playerID, l.reign, policy, ElapsedTurn()))
	WriteProperties(l)
	Game:SetProperty(LEDGER_KEY, l)
end

local function OnChooseOaths(playerID, params)
	local l = Game:GetProperty(LEDGER_KEY)
	if JAC_Oaths_Choose(l, playerID, params) then
		WriteProperties(l)
		Game:SetProperty(LEDGER_KEY, l)
	end
end

Log(string.format("loaded reigns=%d first=%s@%d multiplier=%s", #REIGNS, REIGNS[1] and REIGNS[1].type or "none",
	REIGNS[1] and REIGNS[1].due or -1, tostring(MULTIPLIER)))
GameEvents.OnGameTurnStarted.Add(OnGameTurnStarted)
GameEvents.CityConquered.Add(OnCityConquered)
GameEvents.CityBuilt.Add(OnCityBuilt)
GameEvents.JAC_SengokuDarkPolicyChoose.Add(OnChoosePolicy)
GameEvents.JAC_SengokuOathsChoose.Add(OnChooseOaths)
