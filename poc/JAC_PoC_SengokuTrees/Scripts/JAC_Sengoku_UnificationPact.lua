-- 天下統一同盟 (unification pact, SSOT 3.1 "勝利"): the members pool their castles and only the lead wins.
-- The 1534 start alliances pool the same way while they last (owner decision O-5).
--
-- Requests (gameplay state changes only here; the UI sends requests, MP-safe):
--   UI/JAC_UnificationPact.lua -> UI.RequestPlayerOperation(local, PlayerOperations.EXECUTE_SCRIPT,
--   {OnStart = "JAC_UnificationPactPropose" | "JAC_UnificationPactAccept" | "JAC_UnificationPactLeave",
--   Target = id}), which raises the GameEvent of that name on every machine with the requesting player first
--   (DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:21-28,
--    DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:66, BlackDeathScenario_Support.lua:116).
--   The requester is that engine-supplied player, never a parameter.
-- Rules (SSOT 3.1; owner decisions O-5, O-7, O-8 of 2026-09-30):
--   the proposer is the lead; a human target must accept, an AI target joins at once when it is allied
--   with the proposer (the official alliance is the AI's consent); a pact has one lead and any number of
--   members, and a player is in at most one pact.
--   Leaving: a member leaves at once (Leave request). A member at war with another member of its pact
--   leaves at the next check (turn start or city change; the gameplay side has no declare-war event):
--   the lead stays, and two warring members both leave. A pact left with one house is dissolved.
--   The pact outlives its lead: when the lead is eliminated or leaves, the member holding the most counted
--   targets (tie: lowest player ID) leads it, on every machine alike.
--   AI proposals (provisional, O-7): at each turn start, an AI house in no pact proposes to the first
--   (lowest ID) house it is allied with that is in no pact and has no open proposal (one that its proposer
--   could still take up), when the two hold at
--   least AI_SHARE percent of the counted targets together. A human target gets the accept panel.
--   Start alliances (O-5): each pair whose JAC_1534_ALLIANCE_<A>-<B> Game property is a number
--   (JAC_Japan_1534Relations.lua:37,59; false once ended) pools as a pair, led by the one holding more
--   counted targets (tie: lowest ID). A pair inside the same pact is left to the pact.
--   Players on the same Civ6 team already pool through the team-evaluated victory, so a pact joins teams.
--   An eliminated member drops out (Members() lists alive majors; it owns no cities), so its lost castles
--   count for whoever took them.
-- Targets: the counted castles (ExposedMembers.JAC_UnificationCastles from JAC_Sengoku_RebuiltCastles.lua);
-- with none counted (settler starts, maps without castles), every major's home capital, as the victory's
-- capital rule (owner decision O-8). The home capital is the first capital a major holds, recorded here
-- (HOME_PREFIX) because City:IsOriginalCapital has UI-side use only (Base/Assets/UI/PartialScreens/WorldRankings.lua:1844).
-- Victory: when a group's teams together hold every target, the hidden BUILDING_JAC_PACT_UNIFIED goes into
-- the lead's capital; the victory's pact branch (tools/generate_unification_victory.py) is met only by the
-- team owning that city, on either kind of map.
-- ponytail: the marker is never removed (no documented removal API); the victory fired on the same
-- change live for TransferCity (unify-08), so a castle lost before the evaluation is not expected.

local LEAD_PREFIX = "JAC_UNIFICATION_PACT_LEAD_";         -- + player ID -> lead's player ID (Game property)
local PROPOSAL_PREFIX = "JAC_UNIFICATION_PACT_PROPOSAL_"; -- + target ID -> proposer ID (Game property)
local RESULT = "JAC_UNIFICATION_PACT_RESULT";             -- player property: outcome of the last change
local HOME_PREFIX = "JAC_UNIFICATION_HOME_CAPITAL_";      -- + player ID -> plot index of its first capital
local START_ALLIANCE_PREFIX = "JAC_1534_ALLIANCE_";       -- + <A>-<B> faction names, sorted
local AI_SHARE = 30;  -- provisional (O-7): percent of the targets an AI pair must hold to propose
local UNIFIED = GameInfo.Buildings["BUILDING_JAC_PACT_UNIFIED"];

local function Log(message) print("JAC_Sengoku_UnificationPact: " .. message); end

local function LeadOf(playerID) return Game:GetProperty(LEAD_PREFIX .. playerID); end

local function Alive(playerID)
  local player = type(playerID) == "number" and Players[playerID] or nil;
  return player ~= nil and player:IsMajor() and player:IsAlive();
end

local function AliveIDs()
  local ids = {};
  for _, player in ipairs(PlayerManager.GetAliveMajors()) do  -- DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:47
    table.insert(ids, player:GetID());
  end
  table.sort(ids);
  return ids;
end

-- The target may join the proposer's pact: both alive majors on different teams and not at war, the target
-- in no pact, the proposer in none or leading one. The war check is explicit: live (owner-o-02) an AI still
-- counted as allied with a house it was at war with and proposed to it the turn their pact dissolved.
local function CanJoin(lead, target)
  if lead == target or not Alive(lead) or not Alive(target) then return false; end
  if Players[lead]:GetTeam() == Players[target]:GetTeam() then return false; end
  if Players[lead]:GetDiplomacy():IsAtWarWith(target) then return false; end
  local lead_of = LeadOf(lead);
  return LeadOf(target) == nil and (lead_of == nil or lead_of == lead);
end

-- Players in the lead's pact, the lead included.
local function Members(lead)
  local members = {};
  for _, id in ipairs(AliveIDs()) do
    if LeadOf(id) == lead then table.insert(members, id); end
  end
  return members;
end

-- Each alive major's first capital (O-8). CityBuilt fires when a major founds its first city.
local function RecordHomes()
  for _, id in ipairs(AliveIDs()) do
    if Game:GetProperty(HOME_PREFIX .. id) == nil then
      local capital = Players[id]:GetCities():GetCapitalCity();  -- DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1103
      if capital then Game:SetProperty(HOME_PREFIX .. id, Map.GetPlot(capital:GetX(), capital:GetY()):GetIndex()); end
    end
  end
end

-- The cities a group must hold: counted castles, else every home capital (empty until every alive major has one).
local function Targets()
  local api = ExposedMembers.JAC_UnificationCastles;
  if not api then return {}; end
  local castles = api.CountedCities();
  if #castles > 0 then return castles; end
  local homes = {};
  for _, id in ipairs(AliveIDs()) do
    if Game:GetProperty(HOME_PREFIX .. id) == nil then return {}; end  -- not everyone has founded yet
  end
  for id = 0, PlayerManager.GetWasEverAliveCount() - 1 do  -- Base/Assets/Maps/Utility/MapUtilities.lua:705
    local index = Game:GetProperty(HOME_PREFIX .. id);
    local plot = index and Map.GetPlotByIndex(index);  -- DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:107
    local city = plot and CityManager.GetCityAt(plot:GetX(), plot:GetY());
    if city then table.insert(homes, city); end
  end
  return homes;
end

local function Held(targets)
  local held = {};
  for _, city in ipairs(targets) do held[city:GetOwner()] = (held[city:GetOwner()] or 0) + 1; end
  return held;
end

-- Marks the lead's capital when the group's teams hold every target.
local function CheckGroup(lead, members, targets, label)
  if #targets == 0 or not Alive(lead) then return; end
  local teams = {};
  for _, id in ipairs(members) do teams[Players[id]:GetTeam()] = true; end
  for _, city in ipairs(targets) do
    if not teams[Players[city:GetOwner()]:GetTeam()] then return; end
  end
  local capital = Players[lead]:GetCities():GetCapitalCity();
  if capital and not capital:GetBuildings():HasBuilding(UNIFIED.Index) then
    capital:GetBuildQueue():CreateIncompleteBuilding(UNIFIED.Index,
      Map.GetPlot(capital:GetX(), capital:GetY()):GetIndex(), 100);
    Log(label .. " of " .. lead .. " holds all " .. #targets .. " targets; marked its capital");
  end
end

-- Members of a pact whose lead is gone (eliminated or left) follow the heir described in the header.
local function PassLead(held)
  local heir = {};  -- old lead -> heir
  for _, id in ipairs(AliveIDs()) do
    local lead = LeadOf(id);
    if lead ~= nil and lead ~= id and (not Alive(lead) or LeadOf(lead) ~= lead) then
      local best = heir[lead];
      local mine, theirs = held[id] or 0, best and held[best] or 0;
      if best == nil or mine > theirs or (mine == theirs and id < best) then heir[lead] = id; end
    end
  end
  for _, id in ipairs(AliveIDs()) do
    local old = LeadOf(id);
    local new = old ~= nil and heir[old] or nil;
    if new ~= nil then
      Game:SetProperty(LEAD_PREFIX .. id, new);
      if id == new then Log("lead " .. old .. " gone; " .. new .. " leads the pact (" .. (held[new] or 0) .. " targets)"); end
    end
  end
end

-- A member at war with another member of its pact leaves it (the lead stays).
local function DropWarring()
  local ids, drop = AliveIDs(), {};
  for _, id in ipairs(ids) do
    local lead = LeadOf(id);
    if lead ~= nil and lead ~= id then
      for _, other in ipairs(ids) do
        if other ~= id and LeadOf(other) == lead and Players[id]:GetDiplomacy():IsAtWarWith(other) then
          table.insert(drop, { id, other });
          break;
        end
      end
    end
  end
  for _, pair in ipairs(drop) do
    Log(pair[1] .. " left the pact of " .. LeadOf(pair[1]) .. " (at war with " .. pair[2] .. ")");
    Game:SetProperty(LEAD_PREFIX .. pair[1], nil);
    Players[pair[1]]:SetProperty(RESULT, "AT_WAR");
  end
end

-- A pact needs two houses.
local function Prune()
  local count, last = {}, {};
  for _, id in ipairs(AliveIDs()) do
    local lead = LeadOf(id);
    if lead ~= nil then count[lead] = (count[lead] or 0) + 1; last[lead] = id; end
  end
  for lead, n in pairs(count) do
    if n == 1 then
      Game:SetProperty(LEAD_PREFIX .. last[lead], nil);
      Players[last[lead]]:SetProperty(RESULT, "DISSOLVED");
      Log("pact of " .. lead .. " dissolved");
    end
  end
end

local function Faction(id)
  local civ = PlayerConfigurations[id]:GetCivilizationTypeName();
  return civ and string.match(civ, "^CIVILIZATION_JAC_(.+)$");
end

local function CheckStartAlliances(targets, held)
  local ids = AliveIDs();
  for i = 1, #ids do
    for j = i + 1, #ids do
      local a, b = ids[i], ids[j];
      local fa, fb = Faction(a), Faction(b);
      if fa and fb then
        if fa > fb then fa, fb = fb, fa; end
        local inPact = LeadOf(a) ~= nil and LeadOf(a) == LeadOf(b);
        if not inPact and type(Game:GetProperty(START_ALLIANCE_PREFIX .. fa .. "-" .. fb)) == "number" then
          local lead = (held[b] or 0) > (held[a] or 0) and b or a;
          CheckGroup(lead, { a, b }, targets, "start alliance " .. fa .. "-" .. fb);
        end
      end
    end
  end
end

local function CheckAll()
  RecordHomes();
  local targets = Targets();
  local held = Held(targets);
  PassLead(held);
  DropWarring();
  Prune();
  for _, id in ipairs(AliveIDs()) do
    if LeadOf(id) == id then CheckGroup(id, Members(id), targets, "pact"); end
  end
  CheckStartAlliances(targets, held);
end

local function Join(lead, member)
  Game:SetProperty(LEAD_PREFIX .. lead, lead);
  Game:SetProperty(LEAD_PREFIX .. member, lead);
  Game:SetProperty(PROPOSAL_PREFIX .. member, nil);
  Players[lead]:SetProperty(RESULT, "JOINED");
  Players[member]:SetProperty(RESULT, "JOINED");
  Log(member .. " joined the pact of " .. lead);
  CheckAll();
end

local function Propose(playerID, target)
  if not CanJoin(playerID, target) then
    Players[playerID]:SetProperty(RESULT, "INVALID");
  elseif Players[target]:IsHuman() then
    Game:SetProperty(PROPOSAL_PREFIX .. target, playerID);
    Players[playerID]:SetProperty(RESULT, "PROPOSED");
    Log(playerID .. " proposed a pact to " .. target);
  elseif Players[playerID]:GetDiplomacy():HasAllied(target) then  -- DLC/VikingsScenario/Scripts/VikingScenario.lua:77
    Join(playerID, target);
  else
    Players[playerID]:SetProperty(RESULT, "NOT_ALLIED");
  end
end

-- AI proposals (header). CanJoin rules out a house at war with the proposer.
local function AiPropose()
  local targets = Targets();
  if #targets == 0 then return; end
  local held = Held(targets);
  local ids = AliveIDs();
  for _, p in ipairs(ids) do
    if not Players[p]:IsHuman() and LeadOf(p) == nil then
      for _, t in ipairs(ids) do
        local open = Game:GetProperty(PROPOSAL_PREFIX .. t);  -- a proposal that can still be accepted
        if (open == nil or not CanJoin(open, t)) and Players[p]:GetDiplomacy():HasAllied(t) and CanJoin(p, t)
            and ((held[p] or 0) + (held[t] or 0)) * 100 >= #targets * AI_SHARE then
          Propose(p, t);
          break;
        end
      end
    end
  end
end

local function OnPropose(playerID, params) Propose(playerID, params and params.Target); end

local function OnAccept(playerID, params)
  local lead = params and params.Target;
  if lead ~= nil and Game:GetProperty(PROPOSAL_PREFIX .. playerID) == lead and CanJoin(lead, playerID) then
    Join(lead, playerID);
  else
    Players[playerID]:SetProperty(RESULT, "INVALID");
  end
end

local function OnLeave(playerID)
  if LeadOf(playerID) == nil then
    Players[playerID]:SetProperty(RESULT, "INVALID");
    return;
  end
  Log(playerID .. " left the pact of " .. LeadOf(playerID));
  Game:SetProperty(LEAD_PREFIX .. playerID, nil);
  Players[playerID]:SetProperty(RESULT, "LEFT");
  CheckAll();
end

local function OnCityChanged() CheckAll(); end
local function OnTurnStarted() CheckAll(); AiPropose(); end

GameEvents.JAC_UnificationPactPropose.Add(OnPropose);
GameEvents.JAC_UnificationPactAccept.Add(OnAccept);
GameEvents.JAC_UnificationPactLeave.Add(OnLeave);
GameEvents.CityBuilt.Add(OnCityChanged);      -- also fires on TransferCity (live 2026-09-29, unify-13)
GameEvents.CityConquered.Add(OnCityChanged);  -- DLC/NubiaScenario/Scripts/NubiaScenario.lua:937,969
GameEvents.OnGameTurnStarted.Add(OnTurnStarted);  -- razes, wars; DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246
