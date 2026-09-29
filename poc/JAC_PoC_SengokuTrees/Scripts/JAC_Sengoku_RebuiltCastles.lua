-- Razed registered castles stay out of the unification victory (SSOT 3.1 "勝利", owner decision 3a):
-- a city founded later on a razed castle's plot is not a registered castle.
-- The victory requirements skip cities holding BUILDING_JAC_REBUILT_ON_CASTLE
-- (tools/generate_unification_victory.py); this script only places that marker.
--
-- A razed plot is recorded when a city is founded on it (GameEvents.CityBuilt, official use
-- DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:56,255). Founding needs an empty plot, so after
-- the start turn a registered plot gets a new city only once its castle was razed. The only official
-- removal event, Events.CityRemovedFromMap(playerID, cityID), is UI-side
-- (Base/Assets/UI/WorldView/CityBannerManager.lua:1929,2900) and carries no plot.
-- The record is one saved Game property per plot index (as JAC_Sengoku_StartingKnowledge.lua).
--
-- Capture removes city-centre buildings (Base/Assets/Gameplay/Data/Districts.xml:43), so the marker is
-- re-added on GameEvents.CityConquered (DLC/NubiaScenario/Scripts/NubiaScenario.lua:937,969) and, for
-- transfers or a capture handled before the buildings are removed, on every GameEvents.OnGameTurnStarted
-- (DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246).
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
  end
end

local function OnCityBuilt(playerID, cityID, x, y)
  local plot = Map.GetPlot(x, y);
  if not plot or not Registered()[plot:GetIndex()] then return; end
  if Game.GetCurrentGameTurn() <= GameConfiguration.GetStartTurn() then return; end  -- map-placed castles
  Game:SetProperty(RAZED_PREFIX .. plot:GetIndex(), true);
  MarkRebuilt(plot:GetIndex());
  print("JAC_Sengoku_RebuiltCastles: city rebuilt on razed castle " .. x .. "," .. y);
end

local function OnCityConquered(capturerID, ownerID, cityID, x, y)
  local plot = Map.GetPlot(x, y);
  if plot then MarkRebuilt(plot:GetIndex()); end
end

local function OnGameTurnStarted()
  for plotIndex in pairs(Registered()) do MarkRebuilt(plotIndex); end
end

GameEvents.CityBuilt.Add(OnCityBuilt);
GameEvents.CityConquered.Add(OnCityConquered);
GameEvents.OnGameTurnStarted.Add(OnGameTurnStarted);
