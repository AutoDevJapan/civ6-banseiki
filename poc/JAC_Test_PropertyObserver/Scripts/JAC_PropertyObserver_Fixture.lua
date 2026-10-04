-- TEST ONLY. The UI sends each request through synchronized EXECUTE_SCRIPT.
-- These fixed keys are separate from persona ownership, state and title data.
local KEY = "JAC_TEST853_VALUE";
local PHASE = "JAC_TEST853_PHASE";
-- Numeric keys reach the pinned native getter as EDX directly in this DLL.
-- The fixture checks absence before writing, so an occupied ID fails closed.
local NUMERIC_KEY = 853271;
local NUMERIC_PHASE = 853272;
local VALUES = {true, false, 853271, -853271};

local function Step(owner, params, key, phase)
    assert(owner == 0 and Players[owner] and Players[owner]:IsHuman(),
        "property fixture requires the human player 0");
    assert(type(params) == "table" and type(params.Phase) == "number" and
        params.Phase == math.floor(params.Phase) and params.Phase >= 1 and params.Phase <= 5,
        "property fixture phase must be 1..5");
    local before = Game:GetProperty(phase) or 0;
    assert(params.Phase == before + 1, "property fixture phase must advance once");
    if before == 0 then
        assert(Game:GetProperty(key) == nil and Players[owner]:GetProperty(key) == nil,
            "property fixture keys must be absent before the first write");
    end
    local value = VALUES[params.Phase]; -- phase 5 removes both keys
    Game:SetProperty(key, value);
    Players[owner]:SetProperty(key, value);
    assert(Game:GetProperty(key) == value and Players[owner]:GetProperty(key) == value,
        "official Gameplay property read disagrees with the write");
    Game:SetProperty(phase, params.Phase);
    -- The native getter can return zero values for a removed numeric key.
    -- Assigning first normalizes that result to nil before passing it onward.
    local gameValue = Game:GetProperty(key);
    local playerValue = Players[owner]:GetProperty(key);
    print("JAC_PROPERTY_OBSERVER key=" .. tostring(key) .. " phase=" .. params.Phase ..
        " game=" .. tostring(gameValue) .. " player=" .. tostring(playerValue));
end

GameEvents.JAC_PropertyObserverStep.Add(function(owner, params)
    Step(owner, params, KEY, PHASE);
end);

GameEvents.JAC_PropertyObserverNumericStep.Add(function(owner, params)
    Step(owner, params, NUMERIC_KEY, NUMERIC_PHASE);
end);

-- Test-only derived permit publication for a fresh, cost-one Ashina join.
-- The Game property ledger and official native units/title balance remain the
-- sources of truth. This value does not alter CanEver in calibration mode.
local ASHINA = "LEADER_JAC_ASHINA_MORIKIYO";
local ADMISSION_BASE = 853300;
local ADMISSION_RECEIPT = "JAC_TEST853_ADMISSION_RECEIPT";
local function AdmissionKey(index)
    assert(type(index) == "number" and index >= 0 and index < 10000 and
        index == math.floor(index), "invalid runtime Governor Index");
    local key = ADMISSION_BASE + index;
    assert(key ~= NUMERIC_KEY and key ~= NUMERIC_PHASE, "test property ID collision");
    return key;
end

GameEvents.JAC_JoinedAdmissionInitialize.Add(function(owner)
    assert(owner == 0 and Players[owner] and Players[owner]:IsHuman(),
        "admission calibration requires human player zero");
    assert(Game:GetProperty(ADMISSION_RECEIPT) == nil, "admission already initialized");
    local definitions = {};
    local keys = {};
    for row in GameInfo.JAC_LeaderPersonas() do
        local governor = assert(GameInfo.Governors[row.GovernorType]);
        local key = AdmissionKey(governor.Index);
        assert(not keys[key] and Game:GetProperty(key) == nil,
            "admission numeric key collides with existing Game property");
        keys[key] = true;
        definitions[#definitions + 1] = {key=key, leader=row.LeaderType};
    end
    assert(#definitions == 22, "admission requires exactly 22 persona definitions");
    local players = PlayerManager.GetAliveMajorIDs();
    assert(#players == 22, "admission requires exactly 22 living owners");
    for _, id in ipairs(players) do
        local player = assert(Players[id]);
        for _, definition in ipairs(definitions) do
            assert(player:GetProperty(definition.key) == nil,
                "admission numeric key collides with existing Player property");
        end
    end
    for _, id in ipairs(players) do
        local player = Players[id];
        for _, definition in ipairs(definitions) do
            player:SetProperty(definition.key, false);
        end
    end
    local ashina = assert(GameInfo.JAC_LeaderPersonas[ASHINA]);
    local governor = assert(GameInfo.Governors[ashina.GovernorType]);
    assert(governor.Index == 4 and AdmissionKey(governor.Index) == 853304,
        "Ashina Index differs from pinned calibration scene");
    Game:SetProperty("JAC_TEST853_ADMISSION_ACTIVE", 1);
    Game:SetProperty(ADMISSION_RECEIPT, {
        phase="CLOSED", owner=owner, leader=ASHINA, governorIndex=governor.Index,
        key=AdmissionKey(governor.Index), pairs=22 * #players,
    });
end);

GameEvents.JAC_JoinedAdmissionPrepare.Add(function(owner)
    local receipt = assert(Game:GetProperty(ADMISSION_RECEIPT));
    assert(receipt.phase == "CLOSED" and owner == receipt.owner,
        "admission prepare requires the closed owner receipt");
    local player = assert(Players[owner]);
    local source;
    for _, id in ipairs(PlayerManager.GetAliveMajorIDs()) do
        if PlayerConfigurations[id]:GetLeaderTypeName() == ASHINA then
            assert(source == nil, "duplicate Ashina source");
            source = id;
        end
    end
    assert(source ~= nil and source ~= owner, "Ashina source missing");
    local row = assert(GameInfo.JAC_LeaderPersonas[ASHINA]);
    assert(row.EvolutionCost == 1 and
        Game:GetProperty("JAC_PERSONA_" .. ASHINA .. "_OWNER") == nil,
        "admission requires fresh cost-one Ashina");
    assert(ExposedMembers.JAC_LeaderPersona.Join(owner, source) == "JOINED",
        "real Ashina join failed");
    local class = assert(GameInfo.GreatPersonIndividuals[row.PersonType]);
    local unitType = assert(GameInfo.Units[GameInfo.GreatPersonClasses[class.GreatPersonClassType].UnitType]);
    local count = 0;
    for _, unit in player:GetUnits():Members() do
        if unit:GetType() == unitType.Index then
            local gp = unit:GetGreatPerson();
            local id = gp and gp:GetIndividual();
            local person = id ~= nil and id ~= -1 and GameInfo.GreatPersonIndividuals[id];
            if person and person.GreatPersonIndividualType == row.PersonType then count = count + 1; end
        end
    end
    assert(count == 1, "joined Great Person must have one native unit");
    local governors = player:GetGovernors();
    if governors:GetGovernorPoints() - governors:GetGovernorPointsSpent() < 1 then
        local culture = player:GetCulture();
        local civics = {};
        for title in GameInfo.JAC_PersonaTitleCivics() do civics[#civics + 1] = title.CivicType; end
        table.sort(civics);
        local granted = false;
        for _, civicType in ipairs(civics) do
            local civic = assert(GameInfo.Civics[civicType]);
            if not culture:HasCivic(civic.Index) then
                culture:SetCulturalProgress(civic.Index, culture:GetCultureCost(civic.Index));
                granted = true;
                break;
            end
        end
        assert(granted, "no unused official title civic");
    end
    assert(ExposedMembers.JAC_LeaderPersona.PrepareEvolution(owner, ASHINA) == "EVOLVING",
        "full official title reservation failed");
    assert(Game:GetProperty("JAC_PERSONA_" .. ASHINA .. "_OWNER") == owner and
        Game:GetProperty("JAC_PERSONA_" .. ASHINA .. "_STATE") == "EVOLVING" and
        Game:GetProperty("JAC_PERSONA_" .. ASHINA .. "_EVOLUTION_COST") == 1 and
        governors:GetGovernorPoints() - governors:GetGovernorPointsSpent() >= 1,
        "authoritative admission inputs disagree");
    -- PrepareEvolution published from its verified Gameplay transition.
    assert(player:GetProperty(receipt.key) == true,
        "Gameplay Player receiver did not see the derived permission");
    Game:SetProperty(ADMISSION_RECEIPT, {
        phase="EVOLVING", owner=owner, source=source, leader=ASHINA,
        governorIndex=receipt.governorIndex, key=receipt.key,
        personCount=count, total=governors:GetGovernorPoints(),
        spent=governors:GetGovernorPointsSpent(), pairs=receipt.pairs,
        turn=Game.GetCurrentGameTurn(),
    });
end);

print("JAC_PropertyObserver_Fixture: ready (test only; no automatic mutations)");
