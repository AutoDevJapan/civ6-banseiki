-- SSOT 3.2: resources that were not developed in 1534 appear later, on a historical year or when
-- the first faction finishes the matching tech. The site list is generated into JAC_ResourceSites.lua
-- (ExposedMembers.JAC_ResourceSiteData, keyed by grid size "WxH").
--
-- API for the events framework (callable from any gameplay script or from UI Lua):
--   ExposedMembers.JAC_Resources.RevealSite(siteId)   place the site's resource; true when it was placed now
--   ExposedMembers.JAC_Resources.RevealDue(year)      place every site whose year <= year
--   ExposedMembers.JAC_Resources.RevealForTech(tech)  place every site tied to that tech type
--   ExposedMembers.JAC_Resources.GetSites()           the sites of this map (array of {id, resource, plot, year, tech})
--   ExposedMembers.JAC_Resources.IsRevealed(siteId)
--   ExposedMembers.JAC_Resources.CurrentYear()        calendar year of the current turn
-- Until the events framework calls these, OnTurnBegin reveals sites by calendar year and by tech.

local REVEALED_KEY = "JAC_RESOURCE_SITES_REVEALED"
local BASELINE_KEY = "JAC_RESOURCE_TECH_BASELINE"

local function Log(message)
	print("[JAC_RESOURCES] " .. message)
end

local function GetSites()
	local data = ExposedMembers.JAC_ResourceSiteData
	local width, height = Map.GetGridSize()
	return data and data[width .. "x" .. height] or {}
end

local function FindSite(siteId)
	for _, site in ipairs(GetSites()) do
		if site.id == siteId then
			return site
		end
	end
	return nil
end

local function IsRevealed(siteId)
	local revealed = Game:GetProperty(REVEALED_KEY)
	return revealed ~= nil and revealed[siteId] == true
end

local function RevealSite(siteId)
	local site = FindSite(siteId)
	if site == nil then
		Log("unknown site " .. tostring(siteId))
		return false
	end
	if IsRevealed(siteId) then
		return false
	end
	local resource = GameInfo.Resources[site.resource]
	local plot = Map.GetPlotByIndex(site.plot)
	if resource == nil or plot == nil then
		Log("cannot place " .. siteId .. ": resource or plot missing")
		return false
	end
	if plot:GetResourceType() ~= -1 then
		Log("cannot place " .. siteId .. ": plot " .. site.plot .. " already has a resource")
		return false
	end
	local ok, err = pcall(ResourceBuilder.SetResourceType, plot, resource.Index, 1)
	if not ok or plot:GetResourceType() ~= resource.Index then
		Log("SetResourceType failed for " .. siteId .. ": " .. tostring(err))
		return false
	end
	local revealed = Game:GetProperty(REVEALED_KEY) or {}
	revealed[siteId] = true
	Game:SetProperty(REVEALED_KEY, revealed)
	Log(string.format("revealed %s: %s at plot %d", siteId, site.resource, site.plot))
	return true
end

local function RevealDue(year)
	local count = 0
	for _, site in ipairs(GetSites()) do
		if site.year ~= nil and site.year <= year and RevealSite(site.id) then
			count = count + 1
		end
	end
	return count
end

local function RevealForTech(techType)
	local count = 0
	for _, site in ipairs(GetSites()) do
		if site.tech == techType and RevealSite(site.id) then
			count = count + 1
		end
	end
	return count
end

-- Techs a faction already knew when the game began (starting knowledge) do not reveal anything.
local function TechKey(player, techType)
	return player:GetID() .. ":" .. techType
end

local function ForEachTechHolder(callback)
	local seen = {}
	for _, site in ipairs(GetSites()) do
		local tech = site.tech and GameInfo.Technologies[site.tech]
		if tech ~= nil and not seen[site.tech] then
			seen[site.tech] = true
			for _, player in ipairs(PlayerManager.GetAliveMajors()) do
				if player:GetTechs():HasTech(tech.Index) then
					callback(player, site.tech)
				end
			end
		end
	end
end

local function RevealByResearch()
	local baseline = Game:GetProperty(BASELINE_KEY)
	if baseline == nil then
		baseline = {}
		ForEachTechHolder(function(player, techType) baseline[TechKey(player, techType)] = true end)
		Game:SetProperty(BASELINE_KEY, baseline)
		return
	end
	ForEachTechHolder(function(player, techType)
		if not baseline[TechKey(player, techType)] then
			RevealForTech(techType)
		end
	end)
end

-- Calendar year of the current turn, from the same data the engine dates turns with:
-- the start era's StartEras.Year (01_GameplaySchema.sql:2545-2560; 1534 under the Sengoku pack,
-- JAC_Sengoku_Calendar.xml) plus the months of the elapsed turns taken from GameSpeed_Turns in row order
-- (01_GameplaySchema.sql:1403-1408, :3337). Elapsed turns are counted from GameConfiguration.GetStartTurn()
-- because a 1534 start puts the engine turn near 16603 (TopPanel.lua:387-389 normalizes the same way).
-- No official Lua calls a turn-to-year function; Calendar.MakeYearStr only returns display text.
local function StartYear()
	local era = GameInfo.Eras[GameConfiguration.GetStartEra()]
	for row in GameInfo.StartEras() do
		if era ~= nil and row.EraType == era.EraType then
			return row.Year
		end
	end
	return nil
end

local function CurrentYear()
	local startYear = StartYear()
	local speed = GameInfo.GameSpeeds[GameConfiguration.GetGameSpeedType()]
	if startYear == nil or speed == nil then
		return nil
	end
	local left = Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn()
	local months = 0
	for row in GameInfo.GameSpeed_Turns() do
		if left <= 0 then
			break
		end
		if row.GameSpeedType == speed.GameSpeedType then
			local turns = math.min(left, row.TurnsPerIncrement)
			months = months + turns * row.MonthIncrement
			left = left - turns
		end
	end
	return startYear + math.floor(months / 12)
end

local function OnTurnBegin()
	RevealByResearch()
	local year = CurrentYear()
	if year ~= nil then
		RevealDue(year)
	end
end

ExposedMembers.JAC_Resources = {
	RevealSite = RevealSite,
	RevealDue = RevealDue,
	RevealForTech = RevealForTech,
	GetSites = GetSites,
	IsRevealed = IsRevealed,
	CurrentYear = CurrentYear,
}

Events.TurnBegin.Add(OnTurnBegin)
Log("loaded")
