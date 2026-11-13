-- Extend the existing engine fixture before loading shipped Gameplay. The
-- native Index values are deliberately sparse and unrelated to the row order.
for i = 4, 22 do
    local suffix = 'TEST_' .. tostring(i)
    local leader = 'LEADER_JAC_' .. suffix
    local governor = 'GOVERNOR_JAC_PERSONA_' .. suffix
    local person = 'GREAT_PERSON_INDIVIDUAL_JAC_PERSONA_' .. suffix
    local base = 'GOVERNOR_PROMOTION_JAC_PERSONA_' .. suffix .. '_BASE'
    ROWS[i] = {LeaderType=leader, CivilizationType='CIVILIZATION_JAC_TEST',
        PersonType=person, GovernorType=governor, BasePromotionType=base,
        EvolutionCost=1, Personality='CALM', Trait='ASTUTE', Name=leader,
        PortraitIcon='ICON_' .. leader}
    INDIVIDUALS[i] = {GreatPersonIndividualType=person,
        GreatPersonClassType='GREAT_PERSON_CLASS_JAC_PERSONA',
        Index=100+i, Hash=3000+i, EraType='ERA_ANCIENT', Name=leader}
    GOVDEFS[i] = {GovernorType=governor, Index=150+i, Hash=1000+i, Name=leader}
    PROMOS[#PROMOS+1] = {GovernorPromotionType=base, Hash=2000+i}
end
GameInfo.JAC_LeaderPersonas = collection(ROWS, 'LeaderType')
GameInfo.GreatPersonIndividuals = collection(INDIVIDUALS, 'GreatPersonIndividualType')
GameInfo.Governors = collection(GOVDEFS, 'GovernorType')
GameInfo.GovernorPromotions = collection(PROMOS, 'GovernorPromotionType')

-- A fresh UI read is used by the existing Finalize operation. The new
-- producer independently reads Core objects for its finalized permission.
ExposedMembers.JAC_LeaderPersonaNativeRead = function(owner, leader)
    local row = GameInfo.JAC_LeaderPersonas[leader]
    local p = Players[owner]
    local def = row and GameInfo.Governors[row.GovernorType]
    if not p or not def then return nil end
    local governors = p:GetGovernors()
    local native = governors:GetGovernor(def.Hash)
    local result = {appointed=governors:HasGovernor(def.Hash), base=false, surchargePaid=0}
    if native then
        result.base = native:HasPromotion(GameInfo.GovernorPromotions[row.BasePromotionType].Hash)
        for step=1,row.EvolutionCost-1 do
            local payment = GameInfo.GovernorPromotions[row.BasePromotionType .. '_TITLE_' .. step]
            if payment and native:HasPromotion(payment.Hash) then
                result.surchargePaid = result.surchargePaid + 1
            end
        end
    end
    return result
end
