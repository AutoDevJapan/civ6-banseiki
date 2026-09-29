-- SSOT 3.2: historical events fire in their historical year; a target faction that has already
-- researched the related tech/civic fires them earlier. Data: Data/JAC_HistoricalEvents_*.sql.
--
-- Save/load and multiplayer: all state lives in Player/Game properties (saved with the game and
-- shared by every machine); nothing is cached in Lua between turns. Evaluation runs only from
-- GameEvents.OnGameTurnStarted, in a fixed order (events by due turn then type, players by ID),
-- with no randomness and no local-player checks.

local FIRED_PREFIX = "JAC_EVENT_FIRED_"          -- Player property: turn the event fired for that player (the flag)
local RESOURCES_PREFIX = "JAC_EVENT_RESOURCES_"  -- Game property: plot indices where the event placed resources
local CIV_NAME_PREFIX = "JAC_CIV_NAME_"          -- Game property read by CustomCivs UI/JAC_CivNameOverride.lua
local FIRE_COUNT = "JAC_EVENTS_FIRE_COUNT"       -- Game property: total firings (for verification)

local function Log(message)
	print("[JAC_EVENTS] " .. message)
end

local function Param(name, default)
	local row = GameInfo.GlobalParameters[name]
	return (row and tonumber(row.Value)) or default
end

-- The official calendar exposes only a display string (Calendar.MakeYearStr), so years map to turns
-- here, like the official Pirates scenario's TopPanel. ponytail: one rate for all eras and speeds;
-- replace with the calendar's own table when the 1534 calendar lands.
local START_YEAR = Param("JAC_EVENTS_START_YEAR", 1534)
local TURNS_PER_YEAR = Param("JAC_EVENTS_TURNS_PER_YEAR", 3)

local function DueTurn(row)
	if row.Turn then return row.Turn end
	return (row.Year - START_YEAR) * TURNS_PER_YEAR + 1
end

local function GroupBy(tableName)
	local grouped = {}
	for row in GameInfo[tableName]() do
		local list = grouped[row.EventType]
		if not list then list = {}; grouped[row.EventType] = list end
		table.insert(list, row)
	end
	return grouped
end

local function LoadEvents()
	local targets, modifiers = GroupBy("JAC_HistoricalEventTargets"), GroupBy("JAC_HistoricalEventModifiers")
	local renames, resources = GroupBy("JAC_HistoricalEventRenames"), GroupBy("JAC_HistoricalEventResources")
	local events = {}
	for row in GameInfo.JAC_HistoricalEvents() do
		local event = { row = row, due = DueTurn(row), targets = nil,
			modifiers = modifiers[row.EventType] or {}, renames = renames[row.EventType] or {},
			resources = resources[row.EventType] or {} }
		if targets[row.EventType] then
			event.targets = {}
			for _, target in ipairs(targets[row.EventType]) do event.targets[target.CivilizationType] = true end
		end
		if row.PrereqTech then event.tech = GameInfo.Technologies[row.PrereqTech].Index end
		if row.PrereqCivic then event.civic = GameInfo.Civics[row.PrereqCivic].Index end
		table.insert(events, event)
	end
	table.sort(events, function(a, b)
		if a.due ~= b.due then return a.due < b.due end
		return a.row.EventType < b.row.EventType
	end)
	return events
end

local EVENTS = LoadEvents()

local function CivType(playerID)
	return PlayerConfigurations[playerID]:GetCivilizationTypeName()
end

local function SortedMajorIDs()
	local ids = {}
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do table.insert(ids, player:GetID()) end
	table.sort(ids)
	return ids
end

local function HasPrereq(player, event)
	if event.tech and player:GetTechs():HasTech(event.tech) then return true end
	if event.civic and player:GetCulture():HasCivic(event.civic) then return true end
	return false
end

local function Notify(event, playerID)
	-- LOC keys, not looked-up strings: each machine localizes in its own language
	-- (PiratesScenario_Shared_Script.lua:318-338 passes the keys the same way).
	local data = { [ParameterTypes.MESSAGE] = event.row.Name, [ParameterTypes.SUMMARY] = event.row.Description }
	local notifyAll = event.row.NotifyAll == true or event.row.NotifyAll == 1
	local ids = notifyAll and SortedMajorIDs() or { playerID }
	for _, id in ipairs(ids) do
		NotificationManager.SendNotification(id, NotificationTypes.USER_DEFINED_1, data)
	end
end

-- Nearest legal empty non-city plot to the target, ties broken by plot index.
local function FindResourcePlot(target, resourceIndex, range)
	local best, bestDistance
	local x0, y0 = target:GetX(), target:GetY()
	for dy = -range, range do
		for dx = -range, range do
			local plot = Map.GetPlot(x0 + dx, y0 + dy)
			if plot then
				local distance = Map.GetPlotDistance(x0, y0, plot:GetX(), plot:GetY())
				if distance <= range and plot:GetResourceType() == -1 and not plot:IsCity()
						and ResourceBuilder.CanHaveResource(plot, resourceIndex)
						and (not best or distance < bestDistance
							or (distance == bestDistance and plot:GetIndex() < best:GetIndex())) then
					best, bestDistance = plot, distance
				end
			end
		end
	end
	return best
end

local function PlaceResources(event)
	local key = RESOURCES_PREFIX .. event.row.EventType
	if #event.resources == 0 or Game:GetProperty(key) ~= nil then return end
	local placed = {}
	for _, row in ipairs(event.resources) do
		local target = Map.GetPlotByIndex(row.PlotIndex)
		local resource = GameInfo.Resources[row.ResourceType].Index
		local plot = target and FindResourcePlot(target, resource, row.SearchRange)
		if plot then
			ResourceBuilder.SetResourceType(plot, resource, row.Amount)
			table.insert(placed, plot:GetIndex())
			Log(string.format("resource event=%s type=%s plot=%d target=%d", event.row.EventType,
				row.ResourceType, plot:GetIndex(), row.PlotIndex))
		else
			Log(string.format("resource event=%s type=%s target=%d result=no_legal_plot", event.row.EventType,
				row.ResourceType, row.PlotIndex))
		end
	end
	Game:SetProperty(key, placed)
end

local function Fire(event, player, playerID, civType, reason, turn)
	player:SetProperty(FIRED_PREFIX .. event.row.EventType, turn)
	for _, row in ipairs(event.modifiers) do player:AttachModifierByID(row.ModifierId) end
	for _, row in ipairs(event.renames) do
		if row.CivilizationType == civType then Game:SetProperty(CIV_NAME_PREFIX .. civType, row.Name) end
	end
	PlaceResources(event)
	Notify(event, playerID)
	Game:SetProperty(FIRE_COUNT, (Game:GetProperty(FIRE_COUNT) or 0) + 1)
	Log(string.format("fired event=%s player=%d civ=%s reason=%s turn=%d due=%d", event.row.EventType,
		playerID, civType, reason, turn, event.due))
end

-- Same hook and turn source as the official scenarios (AustraliaScenario.lua:1440-1455).
local function OnGameTurnStarted()
	local turn = Game.GetCurrentGameTurn()
	local ids = SortedMajorIDs()
	for _, event in ipairs(EVENTS) do
		for _, playerID in ipairs(ids) do
			local player = Players[playerID]
			local civType = CivType(playerID)
			if (not event.targets or event.targets[civType])
					and player:GetProperty(FIRED_PREFIX .. event.row.EventType) == nil then
				local reason = (turn >= event.due and "year") or (HasPrereq(player, event) and "prereq") or nil
				if reason then Fire(event, player, playerID, civType, reason, turn) end
			end
		end
	end
end

Log(string.format("loaded events=%d startYear=%d turnsPerYear=%d", #EVENTS, START_YEAR, TURNS_PER_YEAR))
GameEvents.OnGameTurnStarted.Add(OnGameTurnStarted)
