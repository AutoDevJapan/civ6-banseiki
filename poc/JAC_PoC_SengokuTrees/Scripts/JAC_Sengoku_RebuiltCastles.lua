-- Razed registered castles stay out of the unification victory (SSOT 3.1 "勝利", owner decision 3a):
-- a city founded later on a razed castle's plot is not a registered castle.
-- The victory requirements skip cities holding BUILDING_JAC_REBUILT_ON_CASTLE
-- (tools/generate_unification_victory.py); this script only places that marker.
--
-- A registered plot is razed once a turn starts with no city on it. Capture and transfer never leave
-- the plot empty, so those castles keep counting. GameEvents.CityBuilt is not a "founded" signal:
-- it also fires on CityManager.TransferCity (live 2026-09-29, unify-13), so it only re-marks.
-- Founding needs a unit on the empty plot after the city is gone, so the turn-start scan
-- (GameEvents.OnGameTurnStarted, DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246) sees the
-- empty plot first. Events.CityRemovedFromMap has no plot and only UI-side use
-- (Base/Assets/UI/WorldView/CityBannerManager.lua:1929,2900), so it is not used.
-- ponytail: a raze and a refound on the same plot within one turn is missed; add a per-player scan
-- (GameEvents.PlayerTurnStarted) if that shows up in play.
-- The record is one saved Game property per plot index (as JAC_Sengoku_StartingKnowledge.lua).
--
-- Capture removes city-centre buildings (Base/Assets/Gameplay/Data/Districts.xml:43), so the marker is
-- re-added on GameEvents.CityConquered (DLC/NubiaScenario/Scripts/NubiaScenario.lua:937,969),
-- GameEvents.CityBuilt (DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:56,255; also transfers)
-- and every turn start.
local CASTLE_SET = "JAC_UNIFICATION_CITY_IS_CASTLE";
local MARKER = GameInfo.Buildings["BUILDING_JAC_REBUILT_ON_CASTLE"];
local RAZED_PREFIX = "JAC_UNIFICATION_RAZED_CASTLE_PLOT_";

local m_Registered = nil;

-- Registered plot index -> plot, read from the castle SQL of the map in use.
local function Registered()
  if m_Registered then return m_Registered; end
  local coords = {};
  for row in GameInfo.RequirementSetRequirements() do
    if row.RequirementSetId == CASTLE_SET then coords[row.RequirementId] = {}; end
  end
  for row in GameInfo.RequirementArguments() do
    local xy = coords[row.RequirementId];
    if xy then xy[row.Name] = tonumber(row.Value); end
  end
  m_Registered = {};
  for _, xy in pairs(coords) do
    if xy.X >= 0 and xy.Y >= 0 then  -- skip the (-1, -1) sentinel
      local plot = Map.GetPlot(xy.X, xy.Y);
      if plot then m_Registered[plot:GetIndex()] = plot; end
    end
  end
  return m_Registered;
end

local function MarkRebuilt(plotIndex)
  local plot = Registered()[plotIndex];
  if not plot or not Game:GetProperty(RAZED_PREFIX .. plotIndex) then return; end
  local city = CityManager.GetCityAt(plot:GetX(), plot:GetY());
  if city and not city:GetBuildings():HasBuilding(MARKER.Index) then
    city:GetBuildQueue():CreateIncompleteBuilding(MARKER.Index, plotIndex, 100);
    print("JAC_Sengoku_RebuiltCastles: marked city rebuilt on razed castle " .. plot:GetX() .. "," .. plot:GetY());
  end
end

local function MarkAt(x, y)
  local plot = Map.GetPlot(x, y);
  if plot then MarkRebuilt(plot:GetIndex()); end
end

local function OnCityBuilt(playerID, cityID, x, y) MarkAt(x, y); end
local function OnCityConquered(capturerID, ownerID, cityID, x, y) MarkAt(x, y); end

local function OnGameTurnStarted()
  local scan = Game.GetCurrentGameTurn() > GameConfiguration.GetStartTurn();  -- map-placed castles
  for plotIndex, plot in pairs(Registered()) do
    if scan and not Game:GetProperty(RAZED_PREFIX .. plotIndex)
        and not CityManager.GetCityAt(plot:GetX(), plot:GetY()) then
      Game:SetProperty(RAZED_PREFIX .. plotIndex, true);
      print("JAC_Sengoku_RebuiltCastles: registered castle razed at " .. plot:GetX() .. "," .. plot:GetY());
    end
    MarkRebuilt(plotIndex);
  end
end

-- The cities the victory counts as castles (on a registered plot, not rebuilt), for the pact check in
-- JAC_Sengoku_UnificationPact.lua. Gameplay-to-gameplay through ExposedMembers, as JAC_Resources.lua.
ExposedMembers.JAC_UnificationCastles = {
  CountedCities = function()
    local cities = {};
    for _, plot in pairs(Registered()) do
      local city = CityManager.GetCityAt(plot:GetX(), plot:GetY());
      if city and not city:GetBuildings():HasBuilding(MARKER.Index) then table.insert(cities, city); end
    end
    return cities;
  end,
};

GameEvents.CityBuilt.Add(OnCityBuilt);
GameEvents.CityConquered.Add(OnCityConquered);
GameEvents.OnGameTurnStarted.Add(OnGameTurnStarted);
