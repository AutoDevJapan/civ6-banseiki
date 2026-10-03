-- Native appointment/assignment; custom merit purchases are reviewed in the tree screen.
include("InstanceManager");
include("JAC_PersonaTreeRules");
-- AddUserInterfaces starts this context hidden. Keep the toggle and UI-frame
-- callbacks active; only PersonaPanel is initially collapsed (leaderperson-15).
ContextPtr:SetHide(false);
-- Read native UI objects here; never pass userdata or client-supplied state to gameplay.
-- leaderperson-04 verified that governor lookup/CanAppoint exist only in UI.
-- Every invocation reads the current engine state for the requested owner (including AI).
ExposedMembers.JAC_LeaderPersonaNativeRead = function(owner, leader)
    local row = GameInfo.JAC_LeaderPersonas[leader];
    local p = Players[owner];
    local def = row and GameInfo.Governors[row.GovernorType];
    if not p or not def then return nil; end
    local governors = p:GetGovernors();
    if not governors or type(governors.CanAppointGovernor) ~= "function" then return nil; end
    -- CanAppoint() checks the shared title pool, not this person's eligibility.
    -- Both native per-type getters accept the same governor Hash/Index conversion
    -- in the official XP2 UI cache. Evidence: documents/poc/leader-persona.md.
    local result = { appointed=governors:HasGovernor(def.Hash),
                     canAppoint=governors:CanAppointGovernor(def.Hash) };
    if result.appointed then
        local governor = governors:GetGovernor(def.Hash);
        if not governor then return nil; end
        local city = governor:GetAssignedCity();
        result.established = governor:IsEstablished();
        result.neutralized = governor:GetNeutralizedTurns();
        result.base = governor:HasPromotion(GameInfo.GovernorPromotions[row.BasePromotionType].Hash);
        local cost = Game:GetProperty("JAC_PERSONA_" .. leader .. "_EVOLUTION_COST") or row.EvolutionCost;
        result.surchargePaid = 0;
        for step = 1, cost - 1 do
            local payment = GameInfo.GovernorPromotions[row.BasePromotionType .. "_TITLE_" .. step];
            if not payment then return nil; end
            if governor:HasPromotion(payment.Hash) then
                result.surchargePaid = result.surchargePaid + 1;
            else
                break;
            end
        end
        result.canPromote = governors:CanPromoteGovernor(def.Hash);
        if city then result.cityOwner, result.cityID = city:GetOwner(), city:GetID(); end
    end
    return result;
end;

local m_Rows = InstanceManager:new("PersonaRow", "Root", Controls.Rows);
local m_Attempts = {};
local function Get(leader, field) return Game:GetProperty("JAC_PERSONA_" .. leader .. "_" .. field); end
local function Active()
    local p = Players[Game.GetLocalPlayer()];
    return p and p:IsTurnActive();
end

function JAC_RequestLeaderJoin(source)
    if not Active() then error("leader joining requires the local active turn"); end
    UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.EXECUTE_SCRIPT,
                              { OnStart="JAC_LeaderPersonaJoin", Source=source });
end

function JAC_EvolveLeader(leader)
    if not Active() then error("leader evolution requires the local active turn"); end
    local row = GameInfo.JAC_LeaderPersonas[leader];
    if not row or Get(leader, "OWNER") ~= Game.GetLocalPlayer() then error("leader not owned"); end
    if Get(leader, "STATE") ~= "EVOLVING" then m_Attempts[leader] = nil; end
    UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.EXECUTE_SCRIPT,
                              { OnStart="JAC_LeaderPersonaEvolve", Leader=leader });
end

-- A native appointment costs one title; each effect-free payment promotion costs
-- one more. Read the engine after every operation before requesting the next.
function JAC_AppointLeader(leader)
    if not Active() then return false; end
    local row = GameInfo.JAC_LeaderPersonas[leader];
    local owner = Game.GetLocalPlayer();
    if not row or Get(leader, "OWNER") ~= owner or Get(leader, "STATE") ~= "EVOLVING" then return false; end
    local governor = GameInfo.Governors[row.GovernorType];
    local governors = Players[owner]:GetGovernors();
    local native = JAC_PersonaTreeRules.NativeGovernor(owner, leader);
    if not native then return false; end
    local surcharge = Get(leader, "EVOLUTION_SURCHARGE");
    if type(surcharge) ~= "number" or surcharge < 0 then return false; end
    local stage = not native.appointed and "APPOINT" or
        (native.surchargePaid == surcharge and "FINALIZE" or
         "PROMOTE_" .. tostring(native.surchargePaid));
    local attempt = tostring(Game.GetCurrentGameTurn()) .. ":" .. stage;
    if m_Attempts[leader] == attempt then return false; end
    local remaining = surcharge + (native.appointed and 0 or 1) - (native.surchargePaid or 0);
    if native.appointed and native.surchargePaid == surcharge then
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT,
                                  { OnStart="JAC_LeaderPersonaFinalize", Leader=leader });
        m_Attempts[leader] = attempt;
        return true;
    end
    -- Other evolving people retain their unpaid reservations. A missing title
    -- delays this step without recording payment or destroying the Great Person.
    local available = governors:GetGovernorPoints() - governors:GetGovernorPointsSpent();
    local reserved, full = JAC_PersonaTreeRules.ReservedAppointments(owner);
    local ownReserved = full and (surcharge + 1) or remaining;
    if available - (reserved - ownReserved) < 1 then return false; end
    if native.appointed then
        if not native.canPromote or native.surchargePaid >= surcharge then return false; end
        local payment = GameInfo.GovernorPromotions[row.BasePromotionType .. "_TITLE_" .. (native.surchargePaid + 1)];
        if not payment then return false; end
        UI.RequestPlayerOperation(owner, PlayerOperations.PROMOTE_GOVERNOR,
            { [PlayerOperations.PARAM_GOVERNOR_TYPE]=governor.Index,
              [PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE]=payment.Index });
    else
        if not native.canAppoint then return false; end
        UI.RequestPlayerOperation(owner, PlayerOperations.APPOINT_GOVERNOR,
                                  { [PlayerOperations.PARAM_GOVERNOR_TYPE]=governor.Index });
    end
    m_Attempts[leader] = attempt;
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
            local native = state == "GREAT_PERSON" and JAC_PersonaTreeRules.NativeGovernor(owner, leader);
            local noTitle = state == "GREAT_PERSON" and
                (available < cost or not native or native.appointed);
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
        for row in GameInfo.JAC_LeaderPersonas() do
            local leader = row.LeaderType;
            if Get(leader, "OWNER") == owner and Get(leader, "STATE") == "EVOLVING" then
                JAC_AppointLeader(leader);
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
