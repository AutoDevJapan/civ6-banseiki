-- Native appointment/assignment; custom merit purchases are reviewed in the tree screen.
include("InstanceManager");
include("JAC_PersonaTreeRules");
-- Read native UI objects here; never pass userdata or client-supplied state to gameplay.
-- leaderperson-04 verified that governor lookup/CanAppoint exist only in UI.
-- Every invocation reads the current engine state for the requested owner (including AI).
ExposedMembers.JAC_LeaderPersonaNativeRead = function(owner, leader)
    local row = GameInfo.JAC_LeaderPersonas[leader];
    local p = Players[owner];
    local def = row and GameInfo.Governors[row.GovernorType];
    if not p or not def then return nil; end
    local governors = p:GetGovernors();
    local result = { appointed=governors:HasGovernor(def.Hash), canAppoint=governors:CanAppoint() };
    if result.appointed then
        local governor = governors:GetGovernor(def.Hash);
        if not governor then return nil; end
        local city = governor:GetAssignedCity();
        result.established = governor:IsEstablished();
        result.neutralized = governor:GetNeutralizedTurns();
        result.base = governor:HasPromotion(GameInfo.GovernorPromotions[row.BasePromotionType].Hash);
        if city then result.cityOwner, result.cityID = city:GetOwner(), city:GetID(); end
    end
    return result;
end;

-- leaderperson-05: GreatPerson:IsGreatPerson/GetIndividual are UI reads too.
-- Return identity values only; creation and destruction remain in gameplay.
ExposedMembers.JAC_LeaderPersonaPersonRead = function(owner, leader)
    local row = GameInfo.JAC_LeaderPersonas[leader];
    local p = Players[owner];
    if not p or not row then return nil; end
    for _,unit in p:GetUnits():Members() do
        local gp = unit:GetGreatPerson();
        if gp and gp:IsGreatPerson() then
            local individual = GameInfo.GreatPersonIndividuals[gp:GetIndividual()];
            if individual and individual.GreatPersonIndividualType == row.PersonType then
                return {unitID=unit:GetID(), unitType=unit:GetType()};
            end
        end
    end
    return {};
end;

local m_Rows = InstanceManager:new("PersonaRow", "Root", Controls.Rows);
local m_Attempts, m_Finalize = {}, {};
local function Get(leader, field) return Game:GetProperty("JAC_PERSONA_" .. leader .. "_" .. field); end
local function Active()
    local p = Players[Game.GetLocalPlayer()];
    return p and p:IsTurnActive();
end

function JAC_EvolveLeader(leader)
    if not Active() then error("leader evolution requires the local active turn"); end
    local row = GameInfo.JAC_LeaderPersonas[leader];
    if not row or Get(leader, "OWNER") ~= Game.GetLocalPlayer() then error("leader not owned"); end
    m_Attempts[leader], m_Finalize[leader] = nil, nil;
    UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.EXECUTE_SCRIPT,
                              { OnStart="JAC_LeaderPersonaEvolve", Leader=leader });
end

-- Repeating after a rejected appointment is safe: native HasGovernor is checked first.
function JAC_AppointLeader(leader)
    if not Active() then return false; end
    local row = GameInfo.JAC_LeaderPersonas[leader];
    local owner = Game.GetLocalPlayer();
    if not row or Get(leader, "OWNER") ~= owner or Get(leader, "STATE") ~= "EVOLVING" then return false; end
    local governor = GameInfo.Governors[row.GovernorType];
    local governors = Players[owner]:GetGovernors();
    if governors:HasGovernor(governor.Hash) then
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT,
                                  { OnStart="JAC_LeaderPersonaFinalize", Leader=leader });
    else
        if not governors:CanAppoint() then return false; end
        UI.RequestPlayerOperation(owner, PlayerOperations.APPOINT_GOVERNOR,
                                  { [PlayerOperations.PARAM_GOVERNOR_TYPE]=governor.Index });
    end
    return true;
end

-- Automation entry points use the SAME operations as the existing Governor buttons.
function JAC_AssignLeader(leader, cityID)
    local owner = Game.GetLocalPlayer();
    local row = GameInfo.JAC_LeaderPersonas[leader];
    if not Active() or not row or Get(leader, "OWNER") ~= owner or Get(leader, "STATE") ~= "GOVERNOR" then
        error("governor not owned or turn inactive");
    end
    local city = CityManager.GetCity(owner, cityID);
    local def = GameInfo.Governors[row.GovernorType];
    if not city or not Players[owner]:GetGovernors():CanAssignGovernor(def.Hash, city) then error("city not assignable"); end
    UI.RequestPlayerOperation(owner, PlayerOperations.ASSIGN_GOVERNOR,
        { [PlayerOperations.PARAM_GOVERNOR_TYPE]=def.Index,
          [PlayerOperations.PARAM_PLAYER_ONE]=owner, [PlayerOperations.PARAM_CITY_DEST]=cityID });
end

function JAC_PromoteLeader(leader, promotionType)
    local owner = Game.GetLocalPlayer();
    local row = GameInfo.JAC_LeaderPersonas[leader];
    if not Active() or not row or Get(leader, "OWNER") ~= owner or Get(leader, "STATE") ~= "GOVERNOR" then
        error("governor not owned or turn inactive");
    end
    if promotionType then
        local node = GameInfo.JAC_PersonaTreeNodes[promotionType];
        if not node or node.LeaderType ~= leader then error("promotion not in person's tree"); end
    end
    LuaEvents.JAC_PersonaTree_Open(leader, promotionType);
end

function JAC_RefreshPersonas()
    local owner = Game.GetLocalPlayer();
    m_Rows:ResetInstances();
    if owner == -1 or not Players[owner] then return; end
    for row in GameInfo.JAC_LeaderPersonas() do
        local leader = row.LeaderType;
        if Get(leader, "OWNER") == owner then
            local view = m_Rows:GetInstance();
            local state = Get(leader, "STATE");
            view.Portrait:SetIcon(row.PortraitIcon);
            view.Name:SetText(Locale.Lookup(Get(leader, "NAME")));
            view.Status:SetText(Locale.Lookup("LOC_JAC_PERSONA_STATE_" .. state));
            view.Evolve:SetHide(state == "GOVERNOR");
            local cost = Get(leader, "EVOLUTION_COST") or row.EvolutionCost;
            local governors = Players[owner]:GetGovernors();
            local available = math.max(0, governors:GetGovernorPoints() - governors:GetGovernorPointsSpent() -
                JAC_PersonaTreeRules.ReservedAppointments(owner));
            local noTitle = state == "GREAT_PERSON" and (available < cost or not governors:CanAppoint());
            view.Evolve:SetText(Locale.Lookup("LOC_JAC_PERSONA_EVOLVE_COST", cost));
            view.Evolve:SetDisabled(not Active() or noTitle or state == "EVOLVING");
            view.Evolve:SetToolTipString(Locale.Lookup("LOC_JAC_PERSONA_COST_HELP", cost, available));
            view.Name:SetToolTipString(Locale.Lookup("LOC_JAC_PERSONA_PERSONALITY_" .. Get(leader, "PERSONALITY")) ..
                " / " .. Locale.Lookup("LOC_JAC_PERSONA_TRAIT_" .. Get(leader, "TRAIT")));
            view.Evolve:RegisterCallback(Mouse.eLClick, function() JAC_EvolveLeader(leader); end);
            view.Merit:SetText(Locale.Lookup("LOC_JAC_PERSONA_MERIT", JAC_PersonaTreeRules.Merit(leader)));
            view.Merit:SetToolTipString(Locale.Lookup("LOC_JAC_PERSONA_MERIT_HELP"));
            view.Tree:SetHide(state ~= "GOVERNOR");
            view.Tree:SetText(Locale.Lookup("LOC_JAC_PERSONA_TREE"));
            view.Tree:RegisterCallback(Mouse.eLClick, function() JAC_PromoteLeader(leader); end);
        end
    end
    Controls.Rows:CalculateSize();
    Controls.PersonaPanel:DoAutoSize();
end

local function Update()
    if Active() then
        local owner = Game.GetLocalPlayer();
        local turn = Game.GetCurrentGameTurn();
        for row in GameInfo.JAC_LeaderPersonas() do
            local leader = row.LeaderType;
            if Get(leader, "OWNER") == owner and Get(leader, "STATE") == "EVOLVING" then
                local hasGovernor = Players[owner]:GetGovernors():HasGovernor(GameInfo.Governors[row.GovernorType].Hash);
                if hasGovernor and m_Finalize[leader] ~= turn then
                    m_Finalize[leader] = turn; JAC_AppointLeader(leader);
                elseif not hasGovernor and m_Attempts[leader] ~= turn then
                    m_Attempts[leader] = turn; JAC_AppointLeader(leader);
                end
            end
        end
    end
    if not Controls.PersonaPanel:IsHidden() then JAC_RefreshPersonas(); end
end

Controls.PersonaToggle:RegisterCallback(Mouse.eLClick, function()
    Controls.PersonaPanel:SetHide(not Controls.PersonaPanel:IsHidden()); JAC_RefreshPersonas();
end);
Controls.Governors:RegisterCallback(Mouse.eLClick, function() LuaEvents.GovernorPanel_Open(); end);
Controls.PersonaToggle:SetToolTipString(Locale.Lookup("LOC_JAC_PERSONA_KEYBOARD_OPEN"));
Events.GameCoreEventPublishComplete.Add(Update);
Events.GovernorChanged.Add(Update);
Events.LocalPlayerTurnBegin.Add(Update);
Events.LoadScreenClose.Add(Update);
-- F8 opens the first owned Governor; Tab in that screen cycles owned Governors.
-- Input contract: Expansion2/UI/Additions/GovernorPanel.lua:661-671,739.
ContextPtr:SetInputHandler(function(input)
    if input:GetMessageType() == KeyEvents.KeyUp and input:GetKey() == Keys.VK_F8 then
        for row in GameInfo.JAC_LeaderPersonas() do
            if Get(row.LeaderType, "OWNER") == Game.GetLocalPlayer() and Get(row.LeaderType, "STATE") == "GOVERNOR" then
                JAC_PromoteLeader(row.LeaderType); return true;
            end
        end
    end
    return false;
end, true);
