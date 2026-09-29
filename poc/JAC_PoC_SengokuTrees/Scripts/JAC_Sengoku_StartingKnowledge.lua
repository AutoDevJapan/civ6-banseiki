-- Provisional 1534 knowledge common to every major in the Sengoku pack.
-- StartingCivics does not grant civics at an Ancient start (live 2026-09-29: 0 civics), so both are scripted
-- like the official IndonesiaKhmerScenario.lua:31 (pCulture:SetCivic).
local INITIALIZED_KEY = "JAC_SENGOKU_STARTING_KNOWLEDGE_GRANTED_V1";
local STARTING_TECHS = {
  "TECH_JAC_SENGOKU_IRRIGATION_WORKS",
  "TECH_JAC_SENGOKU_COASTAL_PILOTAGE",
  "TECH_JAC_SENGOKU_IRON_FORGING",
  "TECH_JAC_SENGOKU_EARTHWORK_FORTS"
};
local STARTING_CIVICS = {
  "CIVIC_JAC_SENGOKU_HOUSE_RULES",
  "CIVIC_JAC_SENGOKU_KOKUJIN_LEAGUES",
  "CIVIC_JAC_SENGOKU_TEMPLES_AND_BELIEVERS"
};

local function InitializeNewGame()
  if Game:GetProperty(INITIALIZED_KEY) then return; end
  if Game.GetCurrentGameTurn() > GameConfiguration.GetStartTurn() then return; end

  local techs = {};
  for _, techType in ipairs(STARTING_TECHS) do
    local tech = GameInfo.Technologies[techType];
    assert(tech ~= nil, "Sengoku starting tech missing: " .. techType);
    table.insert(techs, tech.Index);
  end

  local civics = {};
  for _, civicType in ipairs(STARTING_CIVICS) do
    local civic = GameInfo.Civics[civicType];
    assert(civic ~= nil, "Sengoku starting civic missing: " .. civicType);
    table.insert(civics, civic.Index);
  end

  local majors = PlayerManager.GetAliveMajors();
  for _, player in ipairs(majors) do
    local playerTechs = player:GetTechs();
    assert(playerTechs ~= nil, "Sengoku major has no techs: " .. tostring(player:GetID()));
    for _, techIndex in ipairs(techs) do
      if not playerTechs:HasTech(techIndex) then
        playerTechs:SetTech(techIndex, true);
      end
    end
    local pCulture = player:GetCulture();
    for _, civicIndex in ipairs(civics) do
      if not pCulture:HasCivic(civicIndex) then
        pCulture:SetCivic(civicIndex, true);
      end
    end
  end

  Game:SetProperty(INITIALIZED_KEY, true);
  print("JAC_Sengoku_StartingKnowledge: granted 4 techs and 3 civics to " .. tostring(#majors) .. " majors");
end

LuaEvents.NewGameInitialized.Add(InitializeNewGame);
