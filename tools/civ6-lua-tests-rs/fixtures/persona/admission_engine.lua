-- Adapt the existing three-player engine fixture to the private Ashina gate.
-- Only engine data and the native read seam live here; admission remains in
-- the shipped Gameplay Lua loaded by the Rust test.
ASHINA = 'LEADER_JAC_ASHINA_MORIKIYO'
local row = ROWS[3]
row.LeaderType = ASHINA
row.CivilizationType = 'CIVILIZATION_JAC_ASHINA'
row.PersonType = 'GREAT_PERSON_INDIVIDUAL_JAC_PERSONA_ASHINA_MORIKIYO'
row.GovernorType = 'GOVERNOR_JAC_PERSONA_ASHINA_MORIKIYO'
row.BasePromotionType = 'GOVERNOR_PROMOTION_JAC_PERSONA_ASHINA_MORIKIYO_BASE'
row.EvolutionCost = 1

INDIVIDUALS[3].GreatPersonIndividualType = row.PersonType
GOVDEFS[3].GovernorType = row.GovernorType
GOVDEFS[3].Index = 4
GOVDEFS[3].Hash = 1004
for _,promotion in ipairs(PROMOS) do
    promotion.GovernorPromotionType = string.gsub(
        promotion.GovernorPromotionType, 'HOJO_UJITSUNA', 'ASHINA_MORIKIYO')
end
PlayerConfigurations[2].leader = ASHINA
PlayerConfigurations[2].civ = row.CivilizationType
GameInfo.JAC_LeaderPersonas = collection(ROWS, 'LeaderType')
GameInfo.GreatPersonIndividuals = collection(INDIVIDUALS, 'GreatPersonIndividualType')
GameInfo.Governors = collection(GOVDEFS, 'GovernorType')
GameInfo.GovernorPromotions = collection(PROMOS, 'GovernorPromotionType')

ExposedMembers.JAC_LeaderPersonaNativeRead = function(owner, leader)
    local person = GameInfo.JAC_LeaderPersonas[leader]
    local player = Players[owner]
    local definition = person and GameInfo.Governors[person.GovernorType]
    if not player or not definition then return nil end
    local governors = player:GetGovernors()
    local native = governors:GetGovernor(definition.Hash)
    local base = GameInfo.GovernorPromotions[person.BasePromotionType]
    return {
        appointed = governors:HasGovernor(definition.Hash),
        base = native and native:HasPromotion(base.Hash) or false,
        surchargePaid = 0,
    }
end
