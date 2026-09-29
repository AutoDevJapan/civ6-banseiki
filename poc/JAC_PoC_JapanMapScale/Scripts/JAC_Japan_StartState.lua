-- Historical Japan maps: record each major's state as loaded from the map, once, before turn 1 is played.
-- AI players act on turn 1 (move garrisons, buy units) before a UI-side check can run, so the autotest
-- compares the map rows with this record (tools/japan_map_scale/prosper_start.py lua_start_check).
-- Value: "player=cities,pop,units,districts,walls,improvements,gold,garrisoned,stacked;..." where
-- garrisoned = cities whose plot holds exactly one own military unit, stacked = plots with 2+ of them.
local KEY = "JAC_JAPAN_START_STATE"

local function Record()
	local saved = Game:GetProperty(KEY) -- one value even when unset (see JAC_CustomCivs.lua:12-17)
	if saved ~= nil or Game.GetCurrentGameTurn() ~= GameConfiguration.GetStartTurn() then return end
	local walls = GameInfo.Buildings["BUILDING_WALLS"].Index
	local centre = GameInfo.Districts["DISTRICT_CITY_CENTER"].Index
	local width = Map.GetGridSize()
	local improvements = {}
	for i = 0, Map.GetPlotCount() - 1 do
		local plot = Map.GetPlotByIndex(i)
		if plot:GetImprovementType() >= 0 then
			improvements[plot:GetOwner()] = (improvements[plot:GetOwner()] or 0) + 1
		end
	end
	local rows = {}
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do
		local id = player:GetID()
		local onPlot, units, stacked = {}, 0, 0
		for _, unit in player:GetUnits():Members() do
			local info = GameInfo.Units[unit:GetType()]
			if (info.Combat or 0) > 0 or (info.RangedCombat or 0) > 0 then
				units = units + 1
				local at = unit:GetY() * width + unit:GetX()
				onPlot[at] = (onPlot[at] or 0) + 1
				if onPlot[at] == 2 then stacked = stacked + 1 end
			end
		end
		local cities, pop, wl, garrisoned = 0, 0, 0, 0
		for _, city in player:GetCities():Members() do
			cities = cities + 1
			pop = pop + city:GetPopulation()
			if city:GetBuildings():HasBuilding(walls) then wl = wl + 1 end
			if onPlot[city:GetY() * width + city:GetX()] == 1 then garrisoned = garrisoned + 1 end
		end
		local districts = 0
		for _, district in player:GetDistricts():Members() do
			if district:IsComplete() and district:GetType() ~= centre then districts = districts + 1 end
		end
		rows[#rows + 1] = string.format("%d=%d,%d,%d,%d,%d,%d,%d,%d,%d", id, cities, pop, units, districts, wl,
			improvements[id] or 0, math.floor(player:GetTreasury():GetGoldBalance()), garrisoned, stacked)
	end
	Game:SetProperty(KEY, table.concat(rows, ";"))
	print("[JAC_JAPAN] start state recorded: " .. table.concat(rows, ";"))
end

Record()
