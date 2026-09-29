-- 天下統一同盟 (unification pact, SSOT 3.1 "勝利"): the members pool their castles and only the lead wins.
--
-- Forming (gameplay state changes only here; the UI sends requests, MP-safe):
--   UI/JAC_UnificationPact.lua -> UI.RequestPlayerOperation(local, PlayerOperations.EXECUTE_SCRIPT,
--   {OnStart = "JAC_UnificationPactPropose" | "JAC_UnificationPactAccept", Target = id}), which raises the
--   GameEvent of that name on every machine with the requesting player first
--   (DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:21-28,
--    DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:66, BlackDeathScenario_Support.lua:116).
--   The requester is that engine-supplied player, never a parameter.
-- Rules (the simplest reading; owner questions Q61-Q64 in documents/questions.md):
--   the proposer is the lead; a human target must accept, an AI target joins at once when it is allied
--   with the proposer (the official alliance is the AI's consent); a pact has one lead and any number of
--   members, a player is in at most one pact, and a pact lasts until victory (no leaving, Q63).
--   The pact outlives its lead: when the lead is eliminated, the surviving member holding the most counted
--   castles (tie: lowest player ID) leads it, on every machine alike. With no survivor the pact is gone
--   (no live player refers to the dead lead, and a dead lead cannot take members).
--   Players on the same Civ6 team already pool through the team-evaluated victory, so a pact joins teams.
--   An eliminated member drops out (Members() lists alive majors; it owns no cities), so its lost castles
--   count for whoever took them and the rest of the pact still wins by holding every counted castle.
-- Victory: when the pact's teams together hold every counted castle
-- (ExposedMembers.JAC_UnificationCastles from JAC_Sengoku_RebuiltCastles.lua), the hidden
-- BUILDING_JAC_PACT_UNIFIED goes into the lead's capital; the victory's pact branch
-- (tools/generate_unification_victory.py) is met only by the team owning that city.
-- ponytail: the marker is never removed (no documented removal API); the victory fired on the same
-- change live for TransferCity (unify-08), so a castle lost before the evaluation is not expected.
-- The pact is only for maps with registered castles (NO_CASTLES otherwise; the panel says so too);
-- settler starts keep the per-team capital rule (Q64).

local LEAD_PREFIX = "JAC_UNIFICATION_PACT_LEAD_";         -- + player ID -> lead's player ID (Game property)
local PROPOSAL_PREFIX = "JAC_UNIFICATION_PACT_PROPOSAL_"; -- + target ID -> proposer ID (Game property)
local RESULT = "JAC_UNIFICATION_PACT_RESULT";             -- player property: outcome of the last request
local UNIFIED = GameInfo.Buildings["BUILDING_JAC_PACT_UNIFIED"];

local function Log(message) print("JAC_Sengoku_UnificationPact: " .. message); end

local function LeadOf(playerID) return Game:GetProperty(LEAD_PREFIX .. playerID); end

local function Alive(playerID)
  local player = type(playerID) == "number" and Players[playerID] or nil;
  return player ~= nil and player:IsMajor() and player:IsAlive();
end

-- The target may join the proposer's pact: both alive majors on different teams, the target in no pact,
-- the proposer in none or leading one.
local function CanJoin(lead, target)
  if lead == target or not Alive(lead) or not Alive(target) then return false; end
  if Players[lead]:GetTeam() == Players[target]:GetTeam() then return false; end
  local lead_of = LeadOf(lead);
  return LeadOf(target) == nil and (lead_of == nil or lead_of == lead);
end

-- Players in the lead's pact, the lead included.
local function Members(lead)
  local members = {};
  for _, player in ipairs(PlayerManager.GetAliveMajors()) do  -- DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:47
    if LeadOf(player:GetID()) == lead then table.insert(members, player:GetID()); end
  end
  return members;
end

local function CheckPact(lead)
  local api = ExposedMembers.JAC_UnificationCastles;
  if not api or not Alive(lead) then return; end
  local teams = {};
  for _, id in ipairs(Members(lead)) do teams[Players[id]:GetTeam()] = true; end
  local castles = api.CountedCities();
  if #castles == 0 then return; end  -- every castle razed: nothing to pool
  for _, city in ipairs(castles) do
    if not teams[Players[city:GetOwner()]:GetTeam()] then return; end
  end
  local capital = Players[lead]:GetCities():GetCapitalCity();
  if capital and not capital:GetBuildings():HasBuilding(UNIFIED.Index) then
    capital:GetBuildQueue():CreateIncompleteBuilding(UNIFIED.Index,
      Map.GetPlot(capital:GetX(), capital:GetY()):GetIndex(), 100);
    Log("pact of " .. lead .. " holds all " .. #castles .. " castles; marked its capital");
  end
end

-- Members of an eliminated lead's pact follow the heir described in the header.
local function PassLead(castles)
  local held, heir = {}, {};  -- player -> counted castles; dead lead -> heir
  for _, city in ipairs(castles) do held[city:GetOwner()] = (held[city:GetOwner()] or 0) + 1; end
  local alive = PlayerManager.GetAliveMajors();
  for _, player in ipairs(alive) do
    local id, lead = player:GetID(), LeadOf(player:GetID());
    if lead ~= nil and not Alive(lead) then
      local best = heir[lead];
      local mine, theirs = held[id] or 0, best and held[best] or 0;
      if best == nil or mine > theirs or (mine == theirs and id < best) then heir[lead] = id; end
    end
  end
  for _, player in ipairs(alive) do
    local id = player:GetID();
    local dead = LeadOf(id);
    local new = dead ~= nil and heir[dead] or nil;
    if new ~= nil then
      Game:SetProperty(LEAD_PREFIX .. id, new);
      if id == new then Log("lead " .. dead .. " eliminated; " .. new .. " leads the pact (" .. (held[new] or 0) .. " castles)"); end
    end
  end
end

local function CheckAll()
  local api = ExposedMembers.JAC_UnificationCastles;
  PassLead(api and api.CountedCities() or {});
  for _, player in ipairs(PlayerManager.GetAliveMajors()) do
    if LeadOf(player:GetID()) == player:GetID() then CheckPact(player:GetID()); end
  end
end

local function Join(lead, member)
  Game:SetProperty(LEAD_PREFIX .. lead, lead);
  Game:SetProperty(LEAD_PREFIX .. member, lead);
  Players[lead]:SetProperty(RESULT, "JOINED");
  Players[member]:SetProperty(RESULT, "JOINED");
  Log(member .. " joined the pact of " .. lead);
  CheckPact(lead);
end

local function OnPropose(playerID, params)
  local target = params and params.Target;
  local api = ExposedMembers.JAC_UnificationCastles;
  if not (api and api.HasCastles()) then
    Players[playerID]:SetProperty(RESULT, "NO_CASTLES");
  elseif not CanJoin(playerID, target) then
    Players[playerID]:SetProperty(RESULT, "INVALID");
  elseif Players[target]:IsHuman() then
    Game:SetProperty(PROPOSAL_PREFIX .. target, playerID);
    Players[playerID]:SetProperty(RESULT, "PROPOSED");
  elseif Players[playerID]:GetDiplomacy():HasAllied(target) then
    Join(playerID, target);
  else
    Players[playerID]:SetProperty(RESULT, "NOT_ALLIED");
  end
end

local function OnAccept(playerID, params)
  local lead = params and params.Target;
  if lead ~= nil and Game:GetProperty(PROPOSAL_PREFIX .. playerID) == lead and CanJoin(lead, playerID) then
    Join(lead, playerID);
  else
    Players[playerID]:SetProperty(RESULT, "INVALID");
  end
end

local function OnCityChanged() CheckAll(); end

GameEvents.JAC_UnificationPactPropose.Add(OnPropose);
GameEvents.JAC_UnificationPactAccept.Add(OnAccept);
GameEvents.CityBuilt.Add(OnCityChanged);      -- also fires on TransferCity (live 2026-09-29, unify-13)
GameEvents.CityConquered.Add(OnCityChanged);  -- DLC/NubiaScenario/Scripts/NubiaScenario.lua:937,969
GameEvents.OnGameTurnStarted.Add(CheckAll);   -- razes; DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246
