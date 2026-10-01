-- A read-only GameCore accounting observation, not a synthetic balance.
include('JAC_TitlePaymentRules');
local Payments = JAC_TitlePaymentRules;
local function FindPerson(owner, row)
    local individual = GameInfo.GreatPersonIndividuals[row.PersonType];
    local class = individual and GameInfo.GreatPersonClasses[individual.GreatPersonClassType];
    local definition = class and GameInfo.Units[class.UnitType];
    if not definition or not Players[owner] then return nil, 0; end;
    local found, count;
    count = 0;
    for _, unit in Players[owner]:GetUnits():Members() do
        if unit:GetType() == definition.Index then
            local gp = unit:GetGreatPerson();
            local index = gp and gp:GetIndividual();
            local person = index ~= nil and index ~= -1 and GameInfo.GreatPersonIndividuals[index];
            if person and person.GreatPersonIndividualType == row.PersonType then found, count = unit, count + 1; end;
        end;
    end;
    return found, count;
end;
-- An explicit diagnostic fixture uses the already verified native grant API.
-- It creates real people before testing the candidate payment queue; it is not
-- a claim that the final virtual Governor integration has been completed.
GameEvents.JAC_TitleProxyFixture.Add(function(owner)
    local player = Players[owner];
    if not player or not player:IsAlive() or not player:IsHuman() then return; end;
    for _, leader in ipairs({'LEADER_JAC_YAMANOUCHI_UESUGI_NORIMASA', 'LEADER_JAC_ODA_NOBUHIDE', 'LEADER_JAC_DATE_TANEMUNE'}) do
        local row = GameInfo.JAC_LeaderPersonas[leader];
        if Game:GetProperty('JAC_PERSONA_' .. leader .. '_OWNER') ~= nil then error('fixture already joined'); end;
        local source;
        for id = 0, PlayerManager.GetWasEverAliveCount() - 1 do
            if PlayerConfigurations[id]:GetLeaderTypeName() == leader then source = id; break; end;
        end;
        if source == nil or source == owner then error('fixture source missing'); end;
        local individual = GameInfo.GreatPersonIndividuals[row.PersonType];
        local class = GameInfo.GreatPersonClasses[individual.GreatPersonClassType];
        local era = GameInfo.Eras[individual.EraType];
        Game.GetGreatPeople():GrantPerson(individual.Hash, class.Hash, era.Hash, 0, owner, false);
        local person, count = FindPerson(owner, row);
        if not person or count ~= 1 then error('fixture needs exactly one real native person'); end;
        local prefix = 'JAC_PERSONA_' .. leader .. '_';
        for key, value in pairs({OWNER=owner, SOURCE=source, STATE='GREAT_PERSON', UNIT=person:GetID(),
            EVOLUTION_COST=row.EvolutionCost, NAME=row.Name, PORTRAIT=row.PortraitIcon,
            PERSONALITY=row.Personality, TRAIT=row.Trait}) do Game:SetProperty(prefix .. key, value); end;
    end;
    player:SetProperty('JAC_PROXY_FIXTURE_READY', 1);
end);
GameEvents.JAC_TitleProxyRequest.Add(function(owner, params)
    local leader = type(params) == 'table' and params.Leader;
    local row = type(leader) == 'string' and GameInfo.JAC_LeaderPersonas[leader];
    local count = 0;
    if row then local person; person, count = FindPerson(owner, row); end;
    local result = count == 1 and Payments.Request(owner, leader) or 'PERSON_MISSING';
    if Players[owner] then
        Players[owner]:SetProperty('JAC_PROXY_REQUEST_RESULT', result);
        local serial = type(params) == 'table' and params.RequestSerial;
        if type(serial) == 'number' and serial > 0 and serial % 1 == 0 then
            Players[owner]:SetProperty('JAC_PROXY_REQUEST_SERIAL', serial);
        end;
    end;
    print('JAC_PROXY_REQUEST ' .. tostring(owner) .. ':' .. result);
end);
GameEvents.JAC_TitleProxyAdvance.Add(function(owner)
    local result = Payments.Advance(owner);
    if Players[owner] then Players[owner]:SetProperty('JAC_PROXY_ADVANCE_RESULT', result); end;
    print('JAC_PROXY_ADVANCE ' .. tostring(owner) .. ':' .. result);
end);
ExposedMembers.JAC_TitleProxyAccounting = function(owner)
    local player = Players[owner];
    if not player then return nil; end
    local governors = player:GetGovernors();
    return governors:GetGovernorPoints(), governors:GetGovernorPointsSpent();
end;
ExposedMembers.JAC_TitleProxyReceipt = function(owner, step)
    local player = Players[owner];
    if not player then return nil; end
    return player:GetProperty('JAC_PROXY_NATIVE_PAID_' .. step);
end;
ExposedMembers.JAC_TitleProxyPaid = function(owner, leader)
    return Payments.Paid(owner, leader);
end;
-- Observe native event availability rather than assume a receipt API exists.
-- These optional diagnostics are not used to accept or finalize any payment.
for _, name in ipairs({'GovernorChanged', 'GovernorAppointed', 'GovernorPromoted', 'GovernorPointsChanged'}) do
    local event = Events[name];
    print('JAC_PROXY_EVENT_AVAILABLE ' .. name .. '=' .. tostring(event ~= nil));
    if event then
        event.Add(function(...)
            local values = {};
            for index, value in ipairs({...}) do values[index] = tostring(value); end
            print('JAC_PROXY_NATIVE_EVENT ' .. name .. ':' .. table.concat(values, ','));
        end);
    end;
end;
print('JAC_TitleProxy_Probe: ready');
