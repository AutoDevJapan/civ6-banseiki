-- Diagnostic mutations only: official civic completion and the regular Join seam.
-- Native Governor eligibility and appointment are read in UI, not fabricated here.
local PREFIX = 'JAC_STATIC_GATE_';
local LEADER = 'LEADER_JAC_ODA_NOBUHIDE';
local function Require(ok, message)
    if not ok then error('JAC_STATIC_GATE_FAIL ' .. message); end;
end;
local function Human(owner)
    Require(owner == 0 and Players[owner] and Players[owner]:IsHuman()
        and Players[owner]:IsAlive(), 'human owner differs');
end;
local function Civics()
    local rows = {};
    for row in GameInfo.JAC_PersonaTitleCivics() do rows[#rows+1] = row.CivicType; end;
    table.sort(rows);
    Require(#rows >= 3, 'official title civics unavailable');
    return rows;
end;
local function GrantTitles(player, count, civics)
    local governors, culture = player:GetGovernors(), player:GetCulture();
    local before, spent = governors:GetGovernorPoints(), governors:GetGovernorPointsSpent();
    local earned = 0;
    for _, kind in ipairs(civics) do
        if earned < count then
            local civic = GameInfo.Civics[kind];
            Require(civic ~= nil, 'title civic definition missing: ' .. kind);
            if not culture:HasCivic(civic.Index) then
                local total = governors:GetGovernorPoints();
                culture:SetCulturalProgress(civic.Index, culture:GetCultureCost(civic.Index));
                Require(culture:HasCivic(civic.Index) and governors:GetGovernorPoints() == total + 1
                    and governors:GetGovernorPointsSpent() == spent,
                    'official civic did not grant exactly one unspent title');
                earned = earned + 1;
            end;
        end;
    end;
    Require(earned == count and governors:GetGovernorPoints() == before + count,
        'positive official title fixture missing: ' .. player:GetID());
    return {points=governors:GetGovernorPoints(), spent=spent};
end;
GameEvents.JAC_StaticGateGiveAITitles.Add(function(owner)
    Human(owner);
    Require(Game:GetProperty(PREFIX .. 'AI_START_TURN') == nil, 'AI titles already granted');
    local count, humans = 0, 0;
    for _, player in ipairs(PlayerManager.GetAliveMajors()) do
        if player:IsHuman() then humans = humans + 1;
        else
            local budget = GrantTitles(player, 3, Civics());
            Game:SetProperty(PREFIX .. 'AI_BUDGET_' .. player:GetID(), budget);
            count = count + 1;
        end;
    end;
    Require(count == 21 and humans == 1, '22-house fixture changed');
    Game:SetProperty(PREFIX .. 'AI_START_TURN', Game.GetCurrentGameTurn());
    Game:SetProperty(PREFIX .. 'AI_READY', 1);
    print('JAC_STATIC_GATE AI_TITLES 21 native civic budgets prepared');
end);
GameEvents.JAC_StaticGatePrepareHuman.Add(function(owner)
    Human(owner);
    Require(Game:GetProperty(PREFIX .. 'AI_READY') == 1
        and Game.GetCurrentGameTurn() == Game:GetProperty(PREFIX .. 'AI_START_TURN') + 1,
        'first AI turn must finish before human preparation');
    Require(Game:GetProperty(PREFIX .. 'HUMAN') == nil, 'human fixture already prepared');
    local source;
    for _, id in ipairs(PlayerManager.GetAliveMajorIDs()) do
        if PlayerConfigurations[id]:GetLeaderTypeName() == LEADER then
            Require(source == nil, 'duplicate source leader');
            source = id;
        end;
    end;
    Require(source and source ~= owner, 'living Oda source missing');
    local row = GameInfo.JAC_LeaderPersonas[LEADER];
    Require(row and row.EvolutionCost == 1, 'one-title joined person missing');
    Require(Players[owner]:GetCities():GetCapitalCity() ~= nil, 'human capital missing');
    local budget = GrantTitles(Players[owner], 1, Civics());
    Require(type(ExposedMembers.JAC_LeaderPersona) == 'table'
        and type(ExposedMembers.JAC_LeaderPersona.Join) == 'function', 'regular join seam unavailable');
    Require(ExposedMembers.JAC_LeaderPersona.Join(owner, source) == 'JOINED', 'regular join refused');
    Require(Game:GetProperty('JAC_PERSONA_' .. LEADER .. '_OWNER') == owner
        and Game:GetProperty('JAC_PERSONA_' .. LEADER .. '_SOURCE') == source
        and Game:GetProperty('JAC_PERSONA_' .. LEADER .. '_STATE') == 'GREAT_PERSON',
        'regular joined-person ledger differs');
    Game:SetProperty(PREFIX .. 'HUMAN', {leader=LEADER, source=source,
        points=budget.points, spent=budget.spent});
    print('JAC_STATIC_GATE JOINED source=' .. source .. ' points=' .. budget.points
        .. ' spent=' .. budget.spent);
end);
GameEvents.JAC_StaticGateBarrier.Add(function(owner)
    Human(owner);
    Require(Game:GetProperty(PREFIX .. 'HUMAN') ~= nil, 'joined person missing');
    Require(Game:GetProperty(PREFIX .. 'BARRIER') == nil, 'barrier already processed');
    -- A synchronized operation after the one APPOINT request. It is a clock,
    -- never a claim that APPOINT was accepted by the native engine.
    Game:SetProperty(PREFIX .. 'BARRIER', Game.GetCurrentGameTurn());
    print('JAC_STATIC_GATE BARRIER processed');
end);
