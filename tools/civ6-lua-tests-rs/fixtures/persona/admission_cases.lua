ADMISSION_CASES = {}

local function expect(expected, actual, label)
    assert(expected == actual,
        label .. ': expected ' .. tostring(expected) .. ', got ' .. tostring(actual))
end

local function field(name)
    return Game:GetProperty('JAC_PERSONA_' .. ASHINA .. '_' .. name)
end

local function permission()
    return Players[0]:GetProperty(853304)
end

local function active()
    Game:SetProperty('JAC_TEST853_ADMISSION_ACTIVE', 1)
end

local function join(owner)
    FIRE('JAC_LeaderPersonaJoin', owner, {Source=2})
    expect('JOINED', Players[owner]:GetProperty('JAC_PERSONA_RESULT'), 'synchronized Join')
    expect('GREAT_PERSON', field('STATE'), 'joined state')
    expect(owner, field('OWNER'), 'joined owner')
    local unit = assert(Players[owner]:GetUnits():FindID(field('UNIT')), 'real Great Person missing')
    expect(99, unit:GetType(), 'Great Person unit type')
    expect(INDIVIDUALS[3].Index, unit:GetGreatPerson():GetIndividual(), 'Great Person identity')
    return unit
end

local function prepare(owner)
    FIRE('JAC_LeaderPersonaEvolve', owner, {Leader=ASHINA})
    return Players[owner]:GetProperty('JAC_PERSONA_RESULT')
end

function ADMISSION_CASES.join_prepare_native_appoint_finalize()
    active()
    expect(nil, permission(), 'unjoined admission')
    local person = join(0)
    expect(false, permission(), 'Join must keep admission closed')
    expect(0, Players[0].spent, 'titles before preparation')

    Players[0].points = 1
    expect('EVOLVING', prepare(0), 'real PrepareEvolution')
    expect('EVOLVING', field('STATE'), 'prepared state')
    expect(1, field('EVOLUTION_COST'), 'recorded Ashina cost')
    expect(0, field('EVOLUTION_SURCHARGE'), 'cost-one surcharge')
    assert(permission() == true, 'preparation did not publish Ashina admission')
    expect(person, Players[0]:GetUnits():FindID(field('UNIT')), 'person before appointment')
    expect(0, Players[0].spent, 'preparation did not spend a title')

    -- Model the official APPOINT operation only after the production gate opens.
    UI.RequestPlayerOperation(0, PlayerOperations.APPOINT_GOVERNOR,
        {[PlayerOperations.PARAM_GOVERNOR_TYPE] = 4})
    DRAIN()
    local definition = GameInfo.Governors[GameInfo.JAC_LeaderPersonas[ASHINA].GovernorType]
    local governor = assert(Players[0]:GetGovernors():GetGovernor(definition.Hash), 'native Governor absent')
    expect(4, governor:GetType(), 'native Governor Index')
    expect(true, governor:HasPromotion(GameInfo.GovernorPromotions[
        GameInfo.JAC_LeaderPersonas[ASHINA].BasePromotionType].Hash), 'native base promotion')
    expect(1, Players[0].spent, 'official title spent by APPOINT')
    expect(person, Players[0]:GetUnits():FindID(field('UNIT')), 'person retained before Finalize')

    FIRE('JAC_LeaderPersonaFinalize', 0, {Leader=ASHINA})
    expect('EVOLVED', Players[0]:GetProperty('JAC_PERSONA_RESULT'), 'real Finalize')
    expect('GOVERNOR', field('STATE'), 'finished state')
    expect(nil, field('UNIT'), 'person record removed')
    expect(nil, Players[0]:GetUnits():FindID(person:GetID()), 'Great Person removed')
    expect(1, DESTROYS, 'one native Great Person removal')
    expect(1, Players[0].spent, 'one official title spent')
    expect(true, permission(), 'committed Governor admission')
end

function ADMISSION_CASES.missing_person_stays_closed()
    active()
    local person = join(0)
    Players[0].points = 1
    NATIVE_UNITS[0][person:GetID()] = nil
    expect('PERSON_MISSING', prepare(0), 'missing Great Person')
    expect('GREAT_PERSON', field('STATE'), 'missing person state')
    expect(false, permission(), 'missing person admission')
    expect(0, Players[0].spent, 'missing person spent')
end

function ADMISSION_CASES.duplicate_person_stays_closed()
    active()
    join(0)
    Players[0].points = 1
    local individual = INDIVIDUALS[3]
    Game.GetGreatPeople():GrantPerson(individual.Hash, 4000, 5000, 0, 0, false)
    expect(2, SPAWNS, 'duplicate native person created')
    expect('EVOLVING', prepare(0), 'duplicate person reaches admission check')
    expect(false, permission(), 'duplicate person admission')
    expect(0, Players[0].spent, 'duplicate person spent')
end

function ADMISSION_CASES.other_reservation_stays_closed()
    active()
    expect('JOINED', ExposedMembers.JAC_LeaderPersona.Join(0, 1), 'other person Join')
    Players[0].points = 2
    expect('EVOLVING', ExposedMembers.JAC_LeaderPersona.PrepareEvolution(
        0, 'LEADER_JAC_DATE_TANEMUNE'), 'other person reserves two titles')
    join(0)
    expect('NO_TITLE', prepare(0), 'other reservation consumes available budget')
    expect(false, permission(), 'insufficient reservation admission')
    expect('GREAT_PERSON', field('STATE'), 'unprepared Ashina state')
    expect(0, Players[0].spent, 'reservation is not official payment')
    Players[0].points = 3
    expect('EVOLVING', prepare(0), 'new title covers both reservations')
    expect(true, permission(), 'admission after full reservation')
end

function ADMISSION_CASES.wrong_owner_never_opens_owner_zero()
    active()
    join(1)
    expect(false, permission(), 'other owner Join')
    Players[1].points = 1
    expect('EVOLVING', prepare(1), 'other owner preparation')
    expect(false, permission(), 'other owner must not open owner zero')
    expect(nil, Players[1]:GetProperty(853304), 'other owner has no gate value')
    expect(0, Players[1].spent, 'other owner preparation spent')
end

function ADMISSION_CASES.inactive_flag_never_publishes()
    join(0)
    expect(nil, permission(), 'no active flag at Join')
    Players[0].points = 1
    expect('EVOLVING', prepare(0), 'preparation without fixture flag')
    expect(nil, permission(), 'no active flag at Prepare')
    active()
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(true, permission(), 'later synchronized reconciliation')
end

function ADMISSION_CASES.wrong_governor_index_never_publishes()
    active()
    join(0)
    expect(false, permission(), 'closed after Join')
    Players[0].points = 1
    local definition = GameInfo.Governors[GameInfo.JAC_LeaderPersonas[ASHINA].GovernorType]
    definition.Index = 5
    expect('EVOLVING', prepare(0), 'preparation with wrong Index')
    expect(false, permission(), 'wrong Governor Index admission')
    expect(0, Players[0].spent, 'wrong Index spent')
end

function ADMISSION_CASES.spent_title_never_counts_as_available()
    active()
    join(0)
    Players[0].points = 1
    Players[0].spent = 1
    expect('NO_TITLE', prepare(0), 'official remaining balance')
    expect(false, permission(), 'spent title admission')
    expect('GREAT_PERSON', field('STATE'), 'spent title state')
end

function ADMISSION_CASES.reconcile_revokes_after_person_or_owner_loss()
    active()
    local person = join(0)
    Players[0].points = 1
    expect('EVOLVING', prepare(0), 'valid preparation')
    expect(true, permission(), 'valid admission')
    NATIVE_UNITS[0][person:GetID()] = nil
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false, permission(), 'missing person revokes admission')
    NATIVE_UNITS[0][person:GetID()] = person
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(true, permission(), 'restored person reopens admission')
    Game:SetProperty('JAC_PERSONA_' .. ASHINA .. '_OWNER', 1)
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false, permission(), 'changed owner revokes admission')
end
