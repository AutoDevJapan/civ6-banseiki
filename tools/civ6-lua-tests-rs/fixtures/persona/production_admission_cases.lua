PRODUCTION_ADMISSION_CASES = {}
local function gate(owner, index)
    return Players[owner]:GetProperty(901000 + index)
end
local function manifest(owner, index)
    return Players[owner]:GetProperty(900000 + index)
end
local function expect(expected, actual, label)
    assert(expected == actual, label .. ': expected ' .. tostring(expected) .. ', got ' .. tostring(actual))
end
function PRODUCTION_ADMISSION_CASES.ai_owner_and_sparse_gameinfo_indices()
    Players[1].human = false
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    for owner=0,2 do
        for row in GameInfo.JAC_LeaderPersonas() do
            local index = GameInfo.Governors[row.GovernorType].Index
            expect(true, manifest(owner, index), 'GameInfo manifest')
            expect(false, gate(owner, index), 'all unjoined people closed')
        end
    end
    FIRE('JAC_LeaderPersonaJoin', 1, {Source=2})
    expect('JOINED', Players[1]:GetProperty('JAC_PERSONA_RESULT'), 'AI Join')
    Players[1].points = 1
    FIRE('JAC_LeaderPersonaEvolve', 1, {Leader=ASHINA})
    expect('EVOLVING', Players[1]:GetProperty('JAC_PERSONA_RESULT'), 'AI reservation')
    expect(true, gate(1, 4), 'AI joined Ashina opens')
    expect(false, gate(0, 4), 'other owner remains closed')
    expect(false, gate(1, 22), 'other person remains closed')
    local id = Game:GetProperty('JAC_PERSONA_' .. ASHINA .. '_UNIT')
    local person = assert(Players[1]:GetUnits():FindID(id))
    NATIVE_UNITS[1][id] = nil
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false, gate(1, 4), 'lost real person revokes')
    NATIVE_UNITS[1][id] = person
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(true, gate(1, 4), 'restored person requires fresh proof')
end
function PRODUCTION_ADMISSION_CASES.cost_two_reserves_all_titles_and_rejects_wrong_owner()
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    FIRE('JAC_LeaderPersonaJoin', 2, {Source=1})
    expect('JOINED', Players[2]:GetProperty('JAC_PERSONA_RESULT'), 'Date Join')
    Players[2].points = 1
    FIRE('JAC_LeaderPersonaEvolve', 2, {Leader='LEADER_JAC_DATE_TANEMUNE'})
    expect('NO_TITLE', Players[2]:GetProperty('JAC_PERSONA_RESULT'), 'one title insufficient')
    expect(false, gate(2, 22), 'unpaid Date remains closed')
    Players[2].points = 2
    FIRE('JAC_LeaderPersonaEvolve', 2, {Leader='LEADER_JAC_DATE_TANEMUNE'})
    expect('EVOLVING', Players[2]:GetProperty('JAC_PERSONA_RESULT'), 'two title reservation')
    expect(true, gate(2, 22), 'actual GameInfo Index opens')
    expect(false, gate(1, 22), 'wrong owner remains closed')
    Players[2].spent = 1
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false, gate(2, 22), 'official remaining titles revoke')
end

function PRODUCTION_ADMISSION_CASES.official_payments_and_core_only_final_state()
    local leader = 'LEADER_JAC_DATE_TANEMUNE'
    FIRE('JAC_LeaderPersonaJoin', 2, {Source=1})
    Players[2].points = 2
    FIRE('JAC_LeaderPersonaEvolve', 2, {Leader=leader})
    expect(true, gate(2, 22), 'full pre-appointment reservation')
    UI.RequestPlayerOperation(2, PlayerOperations.APPOINT_GOVERNOR,
        {[PlayerOperations.PARAM_GOVERNOR_TYPE]=22})
    DRAIN()
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false, gate(2, 22), 'one of two official titles is not full payment')
    local row = GameInfo.JAC_LeaderPersonas[leader]
    local payment = GameInfo.GovernorPromotions[row.BasePromotionType .. '_TITLE_1']
    UI.RequestPlayerOperation(2, PlayerOperations.PROMOTE_GOVERNOR,
        {[PlayerOperations.PARAM_GOVERNOR_TYPE]=22,
         [PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE]=payment.Index})
    DRAIN()
    FIRE('JAC_LeaderPersonaFinalize', 2, {Leader=leader})
    expect('EVOLVED', Players[2]:GetProperty('JAC_PERSONA_RESULT'), 'officially paid Finalize')
    expect(2, Players[2].spent, 'both official titles spent')
    expect(true, gate(2, 22), 'Core Governor and promotions verified')
    local core = Players[2].GetGovernors
    Players[2].GetGovernors = function()
        local governors = core()
        governors.GetGovernor = nil
        return governors
    end
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false, gate(2, 22), 'UI state cannot replace missing Core getter')
    Players[2].GetGovernors = core
end

function PRODUCTION_ADMISSION_CASES.cost_three_needs_two_ordered_official_promotions()
    local row = GameInfo.JAC_LeaderPersonas['LEADER_JAC_ODA_NOBUHIDE']
    row.EvolutionCost = 3
    FIRE('JAC_LeaderPersonaJoin', 1, {Source=0})
    Players[1].points = 3
    FIRE('JAC_LeaderPersonaEvolve', 1, {Leader=row.LeaderType})
    expect('EVOLVING', Players[1]:GetProperty('JAC_PERSONA_RESULT'), 'cost-three reservation')
    expect(true, gate(1, 21), 'cost-three opens after full reservation')
    UI.RequestPlayerOperation(1, PlayerOperations.APPOINT_GOVERNOR,
        {[PlayerOperations.PARAM_GOVERNOR_TYPE]=21})
    DRAIN()
    for step=1,2 do
        local payment = GameInfo.GovernorPromotions[row.BasePromotionType .. '_TITLE_' .. step]
        UI.RequestPlayerOperation(1, PlayerOperations.PROMOTE_GOVERNOR,
            {[PlayerOperations.PARAM_GOVERNOR_TYPE]=21,
             [PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE]=payment.Index})
        DRAIN()
        expect(step+1, Players[1].spent, 'official promotion spent')
    end
    FIRE('JAC_LeaderPersonaFinalize', 1, {Leader=row.LeaderType})
    expect('EVOLVED', Players[1]:GetProperty('JAC_PERSONA_RESULT'), 'cost-three Finalize')
    expect(true, gate(1, 21), 'Core confirms both ordered promotions')
end
