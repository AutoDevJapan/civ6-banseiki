-- Gameplay only. Engine evidence and PoC limits: documents/poc/leader-persona.md.
-- Joining is an explicit integration seam; diplomacy/alliance/territory alone never calls it.
include("JAC_PersonaTreeRules");
local Rules = JAC_PersonaTreeRules;
local SnapshotAlliances;
local OnPlayerTurnStarted;
local PENDING = "JAC_PERSONA_PENDING_DEEDS";
local PREFIX = "JAC_PERSONA_";
local RESULT = "JAC_PERSONA_RESULT";
local PERSONALITIES = { BRAVE=true, CALM=true, CAREFUL=true, JOLLY=true, GENTLE=true, BOLD=true, FICKLE=true, LOYAL=true };
local TRAITS = { VALIANT=true, FIERCE=true, ASTUTE=true, SHREWD=true, DEVOUT=true, ASCETIC=true, MUNDANE=true, ADEPT=true };

local function Key(leader, field) return PREFIX .. leader .. "_" .. field; end
local function Get(leader, field) return Game:GetProperty(Key(leader, field)); end
local function Set(leader, field, value) Game:SetProperty(Key(leader, field), value); end
local function Definition(leader)
    if type(leader) ~= "string" then return nil; end
    return GameInfo.JAC_LeaderPersonas[leader];
end
local function Player(id, alive)
    if type(id) ~= "number" or id < 0 or id % 1 ~= 0 then return nil; end
    local p = Players[id];
    if not p or not p:IsMajor() or (alive and not p:IsAlive()) then return nil; end
    return p;
end
local function Finish(owner, result)
    local p = Player(owner, false);
    if p then p:SetProperty(RESULT, result); end
    return result;
end

local function FindPerson(owner, row)
    local p = Player(owner, false);
    local individual = GameInfo.GreatPersonIndividuals[row.PersonType];
    local class = individual and GameInfo.GreatPersonClasses[individual.GreatPersonClassType];
    local unitType = class and GameInfo.Units[class.UnitType];
    if not p or not unitType then return nil, false; end
    -- leaderperson-06 verified GetIndividual in GameCore, but not IsGreatPerson.
    -- Filter by the class's native unit type before reading the individual identity.
    for _,unit in p:GetUnits():Members() do
        if unit:GetType() == unitType.Index then
            local gp = unit:GetGreatPerson();
            local id = gp and gp:GetIndividual();
            local actual = id ~= nil and id ~= -1 and GameInfo.GreatPersonIndividuals[id];
            if actual and actual.GreatPersonIndividualType == row.PersonType then return unit, true; end
        end
    end
    return nil, true;
end

local function NativeGovernor(owner, row)
    return Rules.NativeGovernor(owner, row.LeaderType);
end

local function Join(owner, source, personality, trait)
    local p, other = Player(owner, true), Player(source, false);
    if not p or not other or owner == source then return Finish(owner, "INVALID"); end
    local config = PlayerConfigurations[source];
    local leader = config and config:GetLeaderTypeName();
    local row = Definition(leader);
    local ownConfig = PlayerConfigurations[owner];
    if not row or not ownConfig or not Definition(ownConfig:GetLeaderTypeName()) or
       config:GetCivilizationTypeName() ~= row.CivilizationType then return Finish(owner, "INVALID"); end
    if (personality ~= nil and not PERSONALITIES[personality]) or
       (trait ~= nil and not TRAITS[trait]) then return Finish(owner, "INVALID_METADATA"); end
    -- A world-unique person, even if callers try another owner or a different source slot.
    if Get(leader, "OWNER") ~= nil then
        return Finish(owner, Get(leader, "OWNER") == owner and "ALREADY_JOINED" or "OTHER_OWNER");
    end
    if trait ~= nil and trait ~= row.Trait then return Finish(owner, "INVALID_METADATA"); end
    print("JAC_PERSONA_STAGE " .. leader .. " native-governor");
    local native = NativeGovernor(owner, row);
    if not native then return Finish(owner, "NATIVE_UNAVAILABLE"); end
    if native.appointed then return Finish(owner, "GOVERNOR_EXISTS"); end
    local city = p:GetCities():GetCapitalCity();
    if not city then return Finish(owner, "NO_CITY"); end
    print("JAC_PERSONA_STAGE " .. leader .. " find-person-before");
    local unit, ready = FindPerson(owner, row);
    if not ready then return Finish(owner, "NATIVE_UNAVAILABLE"); end
    if not unit then
        -- Gameplay grant: DLC/NubiaScenario/Scripts/NubiaScenario.lua:106-111.
        -- CreatePerson is a Tuner placement API and does not return here.
        local individual = GameInfo.GreatPersonIndividuals[row.PersonType];
        local class = GameInfo.GreatPersonClasses[individual.GreatPersonClassType];
        local era = GameInfo.Eras[individual.EraType];
        print("JAC_PERSONA_STAGE " .. leader .. " grant-person-before");
        Game.GetGreatPeople():GrantPerson(individual.Hash, class.Hash, era.Hash, 0, owner, false);
        print("JAC_PERSONA_STAGE " .. leader .. " grant-person-after");
        unit = FindPerson(owner, row);
    end
    if not unit then return Finish(owner, "SPAWN_FAILED"); end
    print("JAC_PERSONA_STAGE " .. leader .. " person-identified");
    Set(leader, "SOURCE", source);
    Set(leader, "NAME", row.Name);
    Set(leader, "PORTRAIT", row.PortraitIcon);
    -- Designed defaults are data, not a claim that historical psychology is known.
    Set(leader, "PERSONALITY", personality or row.Personality);
    Set(leader, "TRAIT", row.Trait);
    Set(leader, "EVOLUTION_COST", row.EvolutionCost);
    Set(leader, "UNIT", unit:GetID());
    Set(leader, "OWNER", owner);
    Set(leader, "STATE", "GREAT_PERSON");
    Set(leader, "MERIT", 0);
    return Finish(owner, "JOINED");
end

local function EvolutionCost(leader)
    local row = Definition(leader);
    return row and (Get(leader, "EVOLUTION_COST") or row.EvolutionCost);
end

local function AvailableTitles(p)
    local governors = p:GetGovernors();
    return governors:GetGovernorPoints() - governors:GetGovernorPointsSpent();
end

local function PrepareEvolution(owner, leader)
    local p, row = Player(owner, true), Definition(leader);
    if not p or not row or Get(leader, "OWNER") ~= owner then return Finish(owner, "NOT_OWNED"); end
    local state = Get(leader, "STATE");
    if state == "GOVERNOR" then return Finish(owner, "ALREADY_EVOLVED"); end
    if state == "EVOLVING" then return Finish(owner, "EVOLVING"); end
    if state ~= "GREAT_PERSON" then return Finish(owner, "PERSON_MISSING"); end
    local person, ready = FindPerson(owner, row);
    if not ready then return Finish(owner, "NATIVE_UNAVAILABLE"); end
    if not person then return Finish(owner, "PERSON_MISSING"); end
    local native = NativeGovernor(owner, row);
    if not native then return Finish(owner, "NATIVE_UNAVAILABLE"); end
    if native.appointed then return Finish(owner, "GOVERNOR_EXISTS"); end
    local cost = EvolutionCost(leader);
    -- Each pending appointment still needs one native title. Its persisted state
    -- reserves that title until completion, so another person cannot count it again.
    if AvailableTitles(p) - Rules.ReservedAppointments(owner) < cost or not native.canAppoint then
        return Finish(owner, "NO_TITLE");
    end
    -- Reserve the surcharge. Native APPOINT_GOVERNOR spends the remaining ONE title.
    -- Debug/Player.ltp:295,654,676; Expansion2/UI/Additions/GovernorPanel.lua:547-562.
    local surcharge = cost - 1;
    if surcharge > 0 then
        local before = AvailableTitles(p);
        p:GetGovernors():ChangeGovernorPoints(-surcharge);
        -- Negative deltas have no official example: never appoint for a cheaper price if rejected.
        local after = AvailableTitles(p);
        if after ~= before - surcharge then
            -- A rejected/partial payment must not consume titles or reserve an appointment.
            if after ~= before then p:GetGovernors():ChangeGovernorPoints(before - after); end
            return Finish(owner, "TITLE_PAYMENT_FAILED");
        end
    end
    Set(leader, "EVOLUTION_SURCHARGE", surcharge);
    Set(leader, "STATE", "EVOLVING");
    return Finish(owner, "EVOLVING");
end

local function CompleteEvolution(owner, leader)
    local p, row = Player(owner, true), Definition(leader);
    if not p or not row or Get(leader, "OWNER") ~= owner then return Finish(owner, "NOT_OWNED"); end
    if Get(leader, "STATE") == "GOVERNOR" then return Finish(owner, "ALREADY_EVOLVED"); end
    if Get(leader, "STATE") ~= "EVOLVING" then return Finish(owner, "NOT_EVOLVING"); end
    local native = NativeGovernor(owner, row);
    if not native or not native.appointed or not native.base then return Finish(owner, "APPOINTMENT_PENDING"); end
    -- Retain the Great Person until the engine has really appointed its Governor.
    local unit, ready = FindPerson(owner, row);
    if not ready then return Finish(owner, "REMOVAL_PENDING"); end
    if unit then p:GetUnits():Destroy(unit); end -- TunerMapPanel.lua:199, also accepts no return value
    local remaining, observed = FindPerson(owner, row);
    if remaining or not observed then return Finish(owner, "REMOVAL_PENDING"); end
    Set(leader, "UNIT", nil);
    Set(leader, "STATE", "GOVERNOR");
    SnapshotAlliances(owner, leader);
    return Finish(owner, "EVOLVED");
end

local function Reconcile()
    for row in GameInfo.JAC_LeaderPersonas() do
        local owner = Get(row.LeaderType, "OWNER");
        if owner ~= nil and Get(row.LeaderType, "STATE") == "EVOLVING" then
            CompleteEvolution(owner, row.LeaderType);
        end
    end
end

local function Parameter(name) return GameInfo.JAC_PersonaTreeParameters[name].Value; end
local function SettledCity(owner, row)
    if Get(row.LeaderType, "STATE") ~= "GOVERNOR" then return nil, true; end
    local governor = NativeGovernor(owner, row);
    if not governor then return nil, false; end
    if not governor.appointed or not governor.base or
       not governor.established or governor.neutralized > 0 or governor.cityOwner ~= owner then return nil, true; end
    local city = governor.cityID and CityManager.GetCity(owner, governor.cityID);
    if city and city:GetOwner() == owner then return city, true; end
    return nil, true;
end

-- Flag all owned city centres, so native reassignment immediately carries purchased skills.
-- The native base's COLLECTION_OWNER selects ONLY its established city; Lua never adds yields.
-- Plot properties: CivRoyaleScenario_Civilizations.xml:392-405 / CivRoyaleScenario_StartScript.lua:1287.
local function SyncEffects()
    for node in GameInfo.JAC_PersonaTreeNodes() do
        if Rules.Owned(node) then
            local p = Player(Get(node.LeaderType, "OWNER"), true);
            if p then
                for _,city in p:GetCities():Members() do
                    local plot = Map.GetPlot(city:GetX(), city:GetY());
                    if plot and plot:GetProperty(node.ModifierId) ~= 1 then plot:SetProperty(node.ModifierId, 1); end
                end
            end
        end
    end
end

local function Purchase(owner, leader, nodeType, confirmCategory, confirmRoute)
    local p = Player(owner, true);
    if not p then return Finish(owner, "NOT_OWNED"); end
    local status = Rules.Status(owner, leader, nodeType);
    if status ~= "AVAILABLE" then return Finish(owner, status); end
    local node = GameInfo.JAC_PersonaTreeNodes[nodeType];
    if (not Get(leader, "CATEGORY") and confirmCategory ~= node.CategoryType) or
       (node.ExclusiveGroup and not Get(leader, "BRANCH_" .. node.CategoryType .. "_" .. node.ExclusiveGroup) and
        confirmRoute ~= node.RouteType) then return Finish(owner, "CONFIRM_REQUIRED"); end
    -- No refund/respec entry point. Serial gameplay request rechecks balance/locks before writing.
    Set(leader, "MERIT", Rules.Merit(leader) - node.Cost);
    Set(leader, "CATEGORY", node.CategoryType);
    if node.ExclusiveGroup then Set(leader, "BRANCH_" .. node.CategoryType .. "_" .. node.ExclusiveGroup, node.RouteType); end
    Set(leader, "NODE_" .. nodeType, 1);
    SyncEffects();
    return Finish(owner, "PURCHASED");
end

local function Award(row, deed, identity)
    local leader = row.LeaderType;
    local owner = Get(leader, "OWNER");
    if not Player(owner, true) or not SettledCity(owner, row) then return false; end
    local design = GameInfo.JAC_PersonaTreePeople[leader];
    local rule = GameInfo.JAC_PersonaMeritRules[design.MeritProfile];
    local turn = Game.GetCurrentGameTurn();
    local seen = "DEED_" .. deed .. "_" .. identity;
    if Get(leader, seen) ~= nil then return false; end
    local cap = deed == "GOVERN_TURN" and 1 or Parameter(deed .. "_DEEDS_PER_TURN");
    local count = Get(leader, deed .. "_COUNT_TURN") == turn and (Get(leader, deed .. "_COUNT") or 0) or 0;
    -- Remember identities even at the cap; queued duplicate notifications cannot pay next turn.
    Set(leader, seen, turn);
    if count >= cap then return false; end
    Set(leader, deed .. "_COUNT_TURN", turn);
    Set(leader, deed .. "_COUNT", count + 1);
    Set(leader, "MERIT", Rules.Merit(leader) + rule[deed]);
    return true;
end

-- Pending notifications are immutable gameplay identities, not UI-provided eligibility.
-- Keep them through save/load; never mark a deed/ally/AI turn done before a native read.
local function Defer(row, deed, identity, cityID, x, y)
    local pending = Game:GetProperty(PENDING) or {};
    local key = row.LeaderType .. "_" .. deed .. "_" .. identity;
    if not pending[key] then
        local sequence = (Game:GetProperty("JAC_PERSONA_PENDING_SEQUENCE") or 0) + 1;
        Game:SetProperty("JAC_PERSONA_PENDING_SEQUENCE", sequence);
        pending[key] = {sequence=sequence, leader=row.LeaderType, owner=Get(row.LeaderType, "OWNER"),
            deed=deed, identity=identity, turn=Game.GetCurrentGameTurn(), cityID=cityID, x=x, y=y};
        Game:SetProperty(PENDING, pending);
    end
end
local function HandleDeed(row, deed, identity, cityID, x, y)
    local owner = Get(row.LeaderType, "OWNER");
    if Get(row.LeaderType, "STATE") ~= "GOVERNOR" or not Player(owner, true) then return; end
    local city, ready = SettledCity(owner, row);
    if not ready then Defer(row, deed, identity, cityID, x, y); return; end
    if not city then return; end
    if deed == "BUILDING" and city:GetID() ~= cityID then return; end
    if deed == "VICTORY" and Map.GetPlotDistance(city:GetX(), city:GetY(), x, y) > Parameter("VICTORY_RADIUS") then return; end
    Award(row, deed, identity);
end
local function FlushPending()
    local pending = Game:GetProperty(PENDING) or {};
    local keys = {}; for key in pairs(pending) do keys[#keys+1] = key; end
    table.sort(keys, function(a,b)
        local first, second = pending[a].sequence or 0, pending[b].sequence or 0;
        if first ~= second then return first < second; end
        return a < b;
    end);
    for _,key in ipairs(keys) do
        local event = pending[key];
        local row = Definition(event.leader);
        if row and Get(event.leader, "OWNER") == event.owner and NativeGovernor(event.owner, row) then
            if event.turn == Game.GetCurrentGameTurn() then
                if event.deed == "TURN" then OnPlayerTurnStarted(event.owner);
                else HandleDeed(row, event.deed, event.identity, event.cityID, event.x, event.y); end
                -- Reload after processing, so another person's deferred event is not overwritten.
                local remaining = Game:GetProperty(PENDING) or {};
                remaining[key] = nil;
                Game:SetProperty(PENDING, next(remaining) and remaining or nil);
            else
                -- Current assignment cannot prove historical eligibility. Preserve evidence.
                Game:SetProperty("JAC_PERSONA_PENDING_NATIVE_HISTORY", 1);
            end
        end
    end
end

local function MajorIDs()
    local ids = PlayerManager.GetAliveMajorIDs(); -- BlackDeathScenario.lua:461.
    table.sort(ids);
    return ids;
end
SnapshotAlliances = function(owner, leader)
    if Get(leader, "ALLIANCES_BASELINED") then return; end
    local diplomacy = Players[owner]:GetDiplomacy();
    for _,other in ipairs(MajorIDs()) do
        if other ~= owner and diplomacy:HasAllied(other) then Set(leader, "ALLY_SEEN_" .. other, 1); end
    end
    Set(leader, "ALLIANCES_BASELINED", 1);
end

local function ChooseAI(owner, row)
    local leader = row.LeaderType;
    local preferred = GameInfo.JAC_PersonaTreePeople[leader];
    local category = Get(leader, "CATEGORY") or preferred.PreferredCategory;
    local candidates = {};
    for _,node in ipairs(Rules.Nodes(leader, category)) do
        if Rules.Status(owner, leader, node.NodeType) == "AVAILABLE" then table.insert(candidates, node); end
    end
    local function Rank(node)
        if node.RouteType == preferred.PreferredRoute then return 0; end
        if node.Signature == 1 then return 1; end
        return 2;
    end
    table.sort(candidates, function(a,b)
        if Rank(a) ~= Rank(b) then return Rank(a) < Rank(b); end
        if a.Depth ~= b.Depth then return a.Depth < b.Depth; end
        return a.NodeType < b.NodeType;
    end);
    local budget = Parameter("AI_PURCHASES_PER_TURN");
    local spent = 0;
    for _,node in ipairs(candidates) do
        if spent >= budget then break; end
        if Purchase(owner, leader, node.NodeType, node.CategoryType, node.RouteType) == "PURCHASED" then spent = spent + 1; end
    end
end

OnPlayerTurnStarted = function(owner)
    Reconcile(); SyncEffects();
    local p = Player(owner, true);
    if not p then return; end
    local turn = Game.GetCurrentGameTurn();
    for row in GameInfo.JAC_LeaderPersonas() do
        local leader = row.LeaderType;
        if Get(leader, "OWNER") == owner and Get(leader, "STATE") == "GOVERNOR" then
            if not NativeGovernor(owner, row) then
                Defer(row, "TURN", tostring(turn));
            else
                SnapshotAlliances(owner, leader);
                local settled = SettledCity(owner, row);
                if settled and Get(leader, "GOVERN_LAST_TURN") ~= turn then
                    Set(leader, "GOVERN_LAST_TURN", turn);
                    Award(row, "GOVERN_TURN", tostring(turn));
                end
                for _,other in ipairs(MajorIDs()) do
                    if other ~= owner and p:GetDiplomacy():HasAllied(other) and not Get(leader, "ALLY_SEEN_" .. other) then
                        -- Count each partner once ever; renewal and save/load cannot farm merit.
                        Set(leader, "ALLY_SEEN_" .. other, 1);
                        if settled then Award(row, "ALLIANCE", tostring(other)); end
                    end
                end
                if not p:IsHuman() and Get(leader, "AI_LAST_TURN") ~= turn then
                    Set(leader, "AI_LAST_TURN", turn);
                    ChooseAI(owner, row);
                end
            end
        end
    end
end

-- Original construction only. Repairs and duplicate building events earn nothing.
-- Signature: DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:362.
local function OnBuildingConstructed(owner, cityID, buildingIndex, plotIndex, original)
    if original ~= true or not GameInfo.Buildings[buildingIndex] then return; end
    for row in GameInfo.JAC_LeaderPersonas() do
        if Get(row.LeaderType, "OWNER") == owner then
            HandleDeed(row, "BUILDING", tostring(plotIndex) .. "_" .. tostring(buildingIndex), cityID);
        end
    end
end

-- Resolve the dying loser while the engine still exposes it, exactly as the official Pirates
-- scenario does (PiratesScenario_StartScript.lua:1240-1265,1617). District battles earn no kill.
local function OnCombatOccurred(attacker, attackerUnit, defender, defenderUnit)
    local a, d = Players[attacker], Players[defender];
    if not a or not d or attacker == defender then return; end
    local au, du = a:GetUnits():FindID(attackerUnit), d:GetUnits():FindID(defenderUnit);
    if not au or not du then return; end
    local function Dead(unit) return unit:IsDead() or unit:IsDelayedDeath(); end
    local winner, loser;
    if Dead(du) and not Dead(au) then winner, loser = attacker, du;
    elseif Dead(au) and not Dead(du) then winner, loser = defender, au;
    else return; end
    for row in GameInfo.JAC_LeaderPersonas() do
        if Get(row.LeaderType, "OWNER") == winner then
            HandleDeed(row, "VICTORY", tostring(loser:GetOwner()) .. "_" .. tostring(loser:GetID()) .. "_" .. Game.GetCurrentGameTurn(),
                nil, loser:GetX(), loser:GetY());
        end
    end
end

-- UI integrations submit this synchronized operation; native creation runs in
-- the gameplay script rather than a cross-context FireTuner function call.
GameEvents.JAC_LeaderPersonaJoin.Add(function(owner, params)
    print("JAC_PERSONA_STAGE join-operation owner=" .. tostring(owner));
    Join(owner, type(params) == "table" and params.Source or nil);
end);
GameEvents.JAC_LeaderPersonaEvolve.Add(function(owner, params)
    PrepareEvolution(owner, type(params) == "table" and params.Leader or nil);
end);
GameEvents.JAC_LeaderPersonaFinalize.Add(function(owner, params)
    CompleteEvolution(owner, type(params) == "table" and params.Leader or nil);
end);
GameEvents.JAC_LeaderPersonaNativeReady.Add(function(owner)
    if Player(owner, true) then Reconcile(); FlushPending(); end
end);
GameEvents.OnGameTurnStarted.Add(Reconcile);
GameEvents.OnGameTurnStarted.Add(SyncEffects);
GameEvents.PlayerTurnStarted.Add(OnPlayerTurnStarted);
GameEvents.BuildingConstructed.Add(OnBuildingConstructed);
GameEvents.OnCombatOccurred.Add(OnCombatOccurred);
GameEvents.CityBuilt.Add(SyncEffects); -- AustraliaScenario.lua:1465.
GameEvents.CityConquered.Add(SyncEffects); -- AlexanderScenario.lua:248.
GameEvents.JAC_LeaderPersonaPurchase.Add(function(owner, params)
    if type(params) ~= "table" then return; end
    local result = Purchase(owner, params.Leader, params.Node, params.ConfirmCategory, params.ConfirmRoute);
    local p = Player(owner, false);
    if p then
        -- A receipt only for this purchase, independent of evolution/assignment results.
        -- Never persist empty strings; malformed request metadata uses nil.
        p:SetProperty("JAC_PERSONA_PURCHASE_RESULT", result);
        p:SetProperty("JAC_PERSONA_PURCHASE_LEADER", type(params.Leader) == "string" and #params.Leader > 0 and params.Leader or nil);
        p:SetProperty("JAC_PERSONA_PURCHASE_NODE", type(params.Node) == "string" and #params.Node > 0 and params.Node or nil);
        local serial = params.RequestSerial;
        p:SetProperty("JAC_PERSONA_PURCHASE_SERIAL", type(serial) == "number" and serial > 0 and serial % 1 == 0 and serial or nil);
    end
end);
ExposedMembers.JAC_LeaderPersona = {
    Join = Join, PrepareEvolution = PrepareEvolution, CompleteEvolution = CompleteEvolution,
    Reconcile = Reconcile, EvolutionCost = EvolutionCost,
    OnPlayerTurnStarted = OnPlayerTurnStarted, SyncEffects = SyncEffects
};
print("JAC_LeaderPersona_Gameplay: ready");
