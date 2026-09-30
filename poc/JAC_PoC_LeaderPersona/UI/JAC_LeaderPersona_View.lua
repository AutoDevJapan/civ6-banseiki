-- This UI shows persistent identities. Appointment, assignment and promotion use native operations.
include("InstanceManager");
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

function JAC_PromoteLeader(leader)
    local owner = Game.GetLocalPlayer();
    local row = GameInfo.JAC_LeaderPersonas[leader];
    if not Active() or not row or Get(leader, "OWNER") ~= owner or Get(leader, "STATE") ~= "GOVERNOR" then
        error("governor not owned or turn inactive");
    end
    local def = GameInfo.Governors[row.GovernorType];
    UI.RequestPlayerOperation(owner, PlayerOperations.PROMOTE_GOVERNOR,
        { [PlayerOperations.PARAM_GOVERNOR_TYPE]=def.Index,
          [PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE]=GameInfo.GovernorPromotions[row.PromotionType].Index });
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
            local noTitle = state == "GREAT_PERSON" and not Players[owner]:GetGovernors():CanAppoint();
            view.Evolve:SetDisabled(not Active() or noTitle);
            view.Evolve:SetToolTipString(noTitle and Locale.Lookup("LOC_NO_GOVERNORS_TITLE_AVAILABLE") or "");
            view.Evolve:RegisterCallback(Mouse.eLClick, function() JAC_EvolveLeader(leader); end);
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
Events.GameCoreEventPublishComplete.Add(Update);
Events.GovernorChanged.Add(Update);
Events.LocalPlayerTurnBegin.Add(Update);
Events.LoadScreenClose.Add(Update);
