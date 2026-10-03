-- Isolated visible-frame entry. The normal GovernorPanel filters by CanEver,
-- so this test sends its single official operation directly and reads the engine.
ContextPtr:SetHide(false);
JAC_StaticGate = {lastAction='IDLE', aiChecked=false};
local PREFIX = 'JAC_STATIC_GATE_';
local TRAIT = 'TRAIT_JAC_TEST_ALL_PERSONAS_CLOSED';
local LEADER = 'LEADER_JAC_ODA_NOBUHIDE';
local pending, issued = nil, {};
local function Require(ok, message)
    if not ok then error('JAC_STATIC_GATE_FAIL ' .. message); end;
end;
local function Active()
    Require(not GameConfiguration.IsAnyMultiplayer(), 'single-player diagnostic required');
    local owner = Game.GetLocalPlayer();
    local player = Players[owner];
    Require(owner == 0 and player and player:IsHuman() and player:IsTurnActive()
        and not UI.IsGameCoreBusy(), 'visible human frame unavailable');
    return owner, player;
end;
local function Definition(row)
    local def = GameInfo.Governors[row.GovernorType];
    Require(def and def.TraitType == TRAIT, 'static persona trait missing: ' .. row.LeaderType);
    return def;
end;
function JAC_StaticGate.AssertClosed(allowJoined)
    Active();
    local people, houses, checked = 0, 0, 0;
    for row in GameInfo.JAC_LeaderPersonas() do people = people + 1; Definition(row); end;
    for _, player in ipairs(PlayerManager.GetAliveMajors()) do
        houses = houses + 1;
        local governors = player:GetGovernors();
        Require(type(governors.CanEverAppointGovernor) == 'function'
            and type(governors.HasGovernor) == 'function', 'native Governor getters unavailable');
        for row in GameInfo.JAC_LeaderPersonas() do
            checked = checked + 1;
            local def = Definition(row);
            local everHash = governors:CanEverAppointGovernor(def.Hash);
            local everIndex = governors:CanEverAppointGovernor(def.Index);
            Require(everHash == false and everIndex == false,
                'native eligibility open: ' .. player:GetID() .. '/' .. row.LeaderType);
            if not (allowJoined and player:GetID() == 0 and row.LeaderType == LEADER) then
                Require(not governors:HasGovernor(def.Hash),
                    'native governor already appointed: ' .. player:GetID() .. '/' .. row.LeaderType);
            end;
        end;
    end;
    Require(houses == 22 and people == 22 and checked == 484,
        'all 22 houses and 22 persona types required');
    return '484 native CanEver eligibility pairs are false';
end;
function JAC_StaticGate.AssertAI()
    Active();
    Require(Game:GetProperty(PREFIX .. 'AI_READY') == 1, 'AI title fixture missing');
    Require(Game.GetCurrentGameTurn() == Game:GetProperty(PREFIX .. 'AI_START_TURN') + 1,
        'first real AI turn was not exactly one turn');
    JAC_StaticGate.AssertClosed(false);
    local count = 0;
    for _, player in ipairs(PlayerManager.GetAliveMajors()) do
        if not player:IsHuman() then
            count = count + 1;
            local budget = Game:GetProperty(PREFIX .. 'AI_BUDGET_' .. player:GetID());
            local governors = player:GetGovernors();
            Require(budget and governors:GetGovernorPoints() >= budget.points
                and governors:GetGovernorPointsSpent() == budget.spent
                and governors:GetGovernorPoints() - governors:GetGovernorPointsSpent() >= 3,
                'AI title consumed: ' .. player:GetID());
            print('JAC_STATIC_GATE AI owner=' .. player:GetID() .. ' points='
                .. governors:GetGovernorPoints() .. ' spent=' .. governors:GetGovernorPointsSpent());
        end;
    end;
    Require(count == 21, '21 AI owners required');
    JAC_StaticGate.aiChecked = true;
    return '21 AI: first real turn, no persona appointment and no title spending';
end;
function JAC_StaticGate.Ready()
    local owner, player = Active();
    local saved = Game:GetProperty(PREFIX .. 'HUMAN');
    Require(saved and saved.leader == LEADER, 'regular human join missing');
    Require(Game:GetProperty('JAC_PERSONA_' .. LEADER .. '_OWNER') == owner
        and Game:GetProperty('JAC_PERSONA_' .. LEADER .. '_SOURCE') == saved.source
        and Game:GetProperty('JAC_PERSONA_' .. LEADER .. '_STATE') == 'GREAT_PERSON'
        and Game:GetProperty('JAC_PERSONA_' .. LEADER .. '_EVOLUTION_COST') == 1,
        'regular join ledger or cost differs');
    local row = GameInfo.JAC_LeaderPersonas[LEADER];
    local def = Definition(row);
    local governors = player:GetGovernors();
    Require(not governors:HasGovernor(def.Hash), 'native person already appointed');
    Require(governors:GetGovernorPoints() == saved.points
        and governors:GetGovernorPointsSpent() == saved.spent
        and saved.points - saved.spent >= 1, 'human official title budget differs');
    local class = GameInfo.GreatPersonClasses['GREAT_PERSON_CLASS_JAC_PERSONA'];
    local unitType = class and GameInfo.Units[class.UnitType];
    Require(unitType ~= nil, 'native Great Person class missing');
    local found = 0;
    for _, unit in player:GetUnits():Members() do
        if unit:GetType() == unitType.Index then
            local gp = unit:GetGreatPerson();
            local id = gp and gp:GetIndividual();
            local person = id ~= nil and id ~= -1 and GameInfo.GreatPersonIndividuals[id];
            if person and person.GreatPersonIndividualType == row.PersonType then found = found + 1; end;
        end;
    end;
    Require(found == 1, 'joined native Great Person absent or duplicated');
    JAC_StaticGate.AssertClosed(false);
    return owner, def, governors;
end;
function JAC_StaticGate.Queue(action)
    Active();
    Require(action == 'AI_TITLES' or action == 'PREPARE_HUMAN'
        or action == 'APPOINT' or action == 'BARRIER', 'unknown gate action');
    Require(not pending and not issued[action], 'gate action already queued: ' .. action);
    pending = action;
    issued[action] = true;
    JAC_StaticGate.lastAction = 'QUEUED_' .. action;
    return JAC_StaticGate.lastAction;
end;
local function Submit(action)
    local owner = Active();
    if action == 'AI_TITLES' then
        JAC_StaticGate.AssertClosed(false);
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT,
            {OnStart='JAC_StaticGateGiveAITitles'});
    elseif action == 'PREPARE_HUMAN' then
        Require(JAC_StaticGate.aiChecked, 'AI native boundary not checked');
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT,
            {OnStart='JAC_StaticGatePrepareHuman'});
    elseif action == 'APPOINT' then
        local _, def, governors = JAC_StaticGate.Ready();
        Require(issued.PREPARE_HUMAN, 'human preparation was not queued');
        print('JAC_STATIC_GATE REQUEST leader=' .. LEADER .. ' canEver='
            .. tostring(governors:CanEverAppointGovernor(def.Hash)) .. ' canAppoint='
            .. tostring(governors:CanAppointGovernor(def.Hash)));
        -- Same official operation/Index as GovernorPanel. Deliberately bypass
        -- only the panel's candidate filter; no eligibility getter is replaced.
        UI.RequestPlayerOperation(owner, PlayerOperations.APPOINT_GOVERNOR,
            {[PlayerOperations.PARAM_GOVERNOR_TYPE]=def.Index});
    else
        Require(issued.APPOINT, 'official appointment was not queued');
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT,
            {OnStart='JAC_StaticGateBarrier'});
    end;
end;
ContextPtr:SetUpdate(function()
    if not pending or UI.IsGameCoreBusy() then return; end;
    local action = pending;
    pending = nil; -- An error or missing response must never replay APPOINT.
    local ok, result = pcall(Submit, action);
    JAC_StaticGate.lastAction = (ok and 'SUBMITTED_' or 'ERROR_') .. action;
    if not ok then error(result); end;
end);
function JAC_StaticGate.Outcome()
    Active();
    Require(issued.APPOINT, 'official appointment was not requested');
    Require(Game:GetProperty(PREFIX .. 'BARRIER') ~= nil,
        'RESPONSE_UNKNOWN synchronized barrier missing');
    JAC_StaticGate.AssertClosed(true);
    local saved = Game:GetProperty(PREFIX .. 'HUMAN');
    local row = GameInfo.JAC_LeaderPersonas[LEADER];
    local def = Definition(row);
    local governors = Players[0]:GetGovernors();
    local has = governors:HasGovernor(def.Hash);
    local native = has and governors:GetGovernor(def.Hash);
    local spent = governors:GetGovernorPointsSpent();
    local result = has and native and native:GetType() == def.Index
        and spent == saved.spent + 1 and governors:GetGovernorPoints() == saved.points;
    print('JAC_STATIC_GATE OUTCOME has=' .. tostring(has) .. ' type='
        .. tostring(native and native:GetType()) .. ' spent=' .. spent
        .. ' before=' .. saved.spent .. ' canEver='
        .. tostring(governors:CanEverAppointGovernor(def.Hash)));
    if not has and spent == saved.spent then
        error('JAC_STATIC_GATE_FAIL REQUEST_REJECTED_OR_UNOBSERVED; no retry');
    end;
    Require(result, 'native appointment or one-title accounting inconsistent');
    return 'DIAGNOSTIC_ACCEPTED: native Governor and one official spent title; production qualification remains unresolved';
end;
print('JAC_StaticPersonaGate_View: visible-frame diagnostic ready');
