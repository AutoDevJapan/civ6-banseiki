-- Test-only synchronized entry. The real native CanEver result is read in UI.
local LEADER = "LEADER_JAC_ASHINA_MORIKIYO";
local MODIFIER = "JAC_TEST_ASHINA_JOINED_GRANT";
local RECEIPT = "JAC_TEST_PERSONA_TRAIT_GRANT_RECEIPT";

GameEvents.JAC_TestPersonaTraitGrantPrepare.Add(function(owner)
    local player = assert(Players[owner], "trait grant owner missing");
    assert(player:IsHuman() and player:IsAlive(),
        "trait grant requires a living human owner");
    assert(Game:GetProperty("JAC_PERSONA_" .. LEADER .. "_OWNER") == nil,
        "trait grant fixture needs a fresh unjoined person");
    assert(Game:GetProperty(RECEIPT) == nil, "trait grant fixture already ran");
    local source;
    for _, id in ipairs(PlayerManager.GetAliveMajorIDs()) do
        if PlayerConfigurations[id]:GetLeaderTypeName() == LEADER then
            assert(source == nil, "duplicate Ashina source");
            source = id;
        end
    end
    assert(source ~= nil and source ~= owner, "Ashina source missing");
    local row = assert(GameInfo.JAC_LeaderPersonas[LEADER], "Ashina definition missing");
    assert(row.EvolutionCost == 1, "fixture requires a one-title person");
    local governors = player:GetGovernors();
    local before_total = governors:GetGovernorPoints();
    local before_spent = governors:GetGovernorPointsSpent();
    assert(ExposedMembers.JAC_LeaderPersona.Join(owner, source) == "JOINED",
        "real joining failed");
    assert(Game:GetProperty("JAC_PERSONA_" .. LEADER .. "_OWNER") == owner and
        Game:GetProperty("JAC_PERSONA_" .. LEADER .. "_STATE") == "GREAT_PERSON",
        "join did not save the owner and Great Person state");
    if governors:GetGovernorPoints() - governors:GetGovernorPointsSpent() < 1 then
        local culture = player:GetCulture();
        local civics = {};
        for title in GameInfo.JAC_PersonaTitleCivics() do
            civics[#civics + 1] = title.CivicType;
        end
        table.sort(civics);
        local granted = false;
        for _, civic_type in ipairs(civics) do
            local civic = GameInfo.Civics[civic_type];
            if not culture:HasCivic(civic.Index) then
                culture:SetCulturalProgress(civic.Index, culture:GetCultureCost(civic.Index));
                granted = true;
                break;
            end
        end
        assert(granted, "no unused official title civic");
    end
    assert(governors:GetGovernorPoints() - governors:GetGovernorPointsSpent() >= 1 and
        governors:GetGovernorPointsSpent() == before_spent,
        "official title reservation input missing");
    assert(ExposedMembers.JAC_LeaderPersona.PrepareEvolution(owner, LEADER) == "EVOLVING",
        "full official reservation failed");
    assert(Game:GetProperty("JAC_PERSONA_" .. LEADER .. "_STATE") == "EVOLVING",
        "evolution reservation was not saved");
    player:AttachModifierByID(MODIFIER);
    Game:SetProperty(RECEIPT, {owner=owner, source=source, leader=LEADER,
        preparedTurn=Game.GetCurrentGameTurn(),
        total=governors:GetGovernorPoints(), spent=governors:GetGovernorPointsSpent(),
        beforeTotal=before_total, beforeSpent=before_spent,
        detachKind=type(player.DetachModifierByID),
        effectsRemoveKind=type(GameEffects and GameEffects.RemoveModifier)});
    print("JAC_TEST_PERSONA_TRAIT_GRANT attached owner=" .. tostring(owner) ..
        " source=" .. tostring(source) .. " modifier=" .. MODIFIER);
end);

-- Read the official account after the native request and a complete real turn.
-- This receipt classifies a probe; it never grants a Governor or spends a title.
GameEvents.JAC_TestPersonaTraitGrantObserve.Add(function(owner)
    local receipt = assert(Game:GetProperty(RECEIPT), "prepared trait probe missing");
    local player = assert(Players[owner], "observation owner missing");
    assert(owner == receipt.owner and player:IsHuman() and player:IsAlive(),
        "observation owner differs from the real joining owner");
    local governors = player:GetGovernors();
    Game:SetProperty("JAC_TEST_PERSONA_TRAIT_GRANT_OBSERVATION", {
        owner=owner, turn=Game.GetCurrentGameTurn(),
        total=governors:GetGovernorPoints(), spent=governors:GetGovernorPointsSpent()
    });
end);
