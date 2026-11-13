-- The old Python methods are ported as named Lua cases. The Rust integration
-- test demands the exact case name and executes it against current MOD Lua.
CASES = {}
ASSERTIONS = 0
DATE = 'LEADER_JAC_DATE_TANEMUNE'
HOJO = 'LEADER_JAC_HOJO_UJITSUNA'
ODA = 'LEADER_JAC_ODA_NOBUHIDE'

local function same(a, b, seen)
    if type(a) ~= type(b) then return false end
    if type(a) ~= 'table' then return a == b end
    seen = seen or {}
    if seen[a] == b then return true end
    seen[a] = b
    for k, v in pairs(a) do if not same(v, b[k], seen) then return false end end
    for k, v in pairs(b) do if a[k] == nil then return false end end
    return true
end
function eq(expected, actual)
    ASSERTIONS = ASSERTIONS + 1
    if not same(expected, actual) then
        error('expected '..tostring(expected)..' ('..type(expected)..'), got '..tostring(actual)..' ('..type(actual)..')', 2)
    end
end
function yes(value) eq(true, value) end
function no(value) eq(false, value) end
function has(text, needle)
    ASSERTIONS = ASSERTIONS + 1
    if type(text) ~= 'string' or not string.find(text, needle, 1, true) then
        error('missing '..needle..' in '..tostring(text), 2)
    end
end
function raises(needle, f)
    ASSERTIONS = ASSERTIONS + 1
    local success, message = pcall(f)
    if success or not string.find(tostring(message), needle, 1, true) then
        error('expected error containing '..needle..', got '..tostring(message), 2)
    end
end
function count(t)
    local n = 0
    for _ in pairs(t) do n = n + 1 end
    return n
end
function prop(field, leader)
    return Game:GetProperty('JAC_PERSONA_'..(leader or DATE)..'_'..field)
end
function reload()
    RESET_CONTEXTS()
    LOAD_CONTEXTS()
end
function join() eq('JOINED', ExposedMembers.JAC_LeaderPersona.Join(0, 1, 'CALM', 'ASTUTE')) end
function grant_titles(owner, leader)
    Players[owner]:GetGovernors():ChangeGovernorPoints(ExposedMembers.JAC_LeaderPersona.EvolutionCost(leader))
end
function evolve()
    join()
    grant_titles(0, DATE)
    JAC_EvolveLeader(DATE)
    DRAIN()
    eq('GOVERNOR', prop('STATE'))
end
function settled(leader, city)
    leader, city = leader or DATE, city or 1
    local api = ExposedMembers.JAC_LeaderPersona
    if leader == DATE then evolve() else
        local source = leader == ODA and 0 or 2
        eq('JOINED', api.Join(0, source))
        grant_titles(0, leader)
        JAC_EvolveLeader(leader); DRAIN()
    end
    local governor = NATIVE_GOVS[0][GameInfo.Governors[GameInfo.JAC_LeaderPersonas[leader].GovernorType].Hash]
    governor.city = CITIES[0][city]
    return governor
end
function set_merit(amount, leader)
    Game:SetProperty('JAC_PERSONA_'..(leader or DATE)..'_MERIT', amount)
end
function request_purchase(owner, leader, node, category, route)
    FIRE('JAC_LeaderPersonaPurchase', owner,
        {Leader=leader, Node=node, ConfirmCategory=category, ConfirmRoute=route})
    return Players[owner]:GetProperty('JAC_PERSONA_PURCHASE_RESULT')
end
function purchase(route, depth, leader)
    leader, depth = leader or DATE, depth or 1
    local node = leader..'_'..route..'_'..depth
    local definition = GameInfo.JAC_PersonaTreeNodes[node]
    return request_purchase(0, leader, node, definition.CategoryType, definition.RouteType)
end
function combat(loser_id, x, dead, owner, attacker)
    loser_id, x, owner, attacker = loser_id or 900, x or 3, owner or 1, attacker or 0
    if dead == nil then dead = true end
    local function warunit(unit_owner, id, px, killed)
        return {GetID=function() return id end, GetOwner=function() return unit_owner end,
            GetX=function() return px end, GetY=function() return 0 end,
            IsDead=function() return killed end, IsDelayedDeath=function() return false end}
    end
    NATIVE_UNITS[attacker][800] = warunit(attacker, 800, 1, false)
    NATIVE_UNITS[owner][loser_id] = warunit(owner, loser_id, x, dead)
    FIRE('OnCombatOccurred', attacker, 800, owner, loser_id)
end

function CASES.test_all_live_save_snapshots_accept_properties_with_no_return_value()
    join()
    local read = Game.GetProperty
    Game.GetProperty = function(self, key)
        local value = read(self, key)
        if value ~= nil then return value end
    end
    GameInfo.JAC_LeaderPersonas.LEADER_JAC_YAMANOUCHI_UESUGI_NORIMASA = {GovernorType='GOVERNOR_TEST_NORIMASA'}
    GameInfo.Governors.GOVERNOR_TEST_NORIMASA = {Hash=9000}
    eq(12, #SCENARIO_SNAPSHOTS)
    for _, code in ipairs(SCENARIO_SNAPSHOTS) do
        local result = assert(loadstring(code))()
        has(result, 'EVOLUTION_SURCHARGE=nil')
        has(result, 'CATEGORY=nil')
        if string.find(code, "local leader='LEADER_JAC_DATE_TANEMUNE'", 1, true) then
            has(result, 'STATE=GREAT_PERSON')
            has(result, 'MERIT=0')
        end
    end
end

function CASES.test_gameplay_without_ui_governor_methods_uses_fresh_native_reads()
    local original = Players[0].GetGovernors
    local read = ExposedMembers.JAC_LeaderPersonaNativeRead
    UI_NATIVE_READ = false; GAMECORE_UNIT_READS = true
    Players[0].GetGovernors = function()
        local all = original()
        if UI_NATIVE_READ then return all end
        return {GetGovernorPoints=all.GetGovernorPoints,
            GetGovernorPointsSpent=all.GetGovernorPointsSpent,
            ChangeGovernorPoints=all.ChangeGovernorPoints}
    end
    ExposedMembers.JAC_LeaderPersonaNativeRead = function(owner, leader)
        UI_NATIVE_READ = true
        local success, result = pcall(read, owner, leader)
        UI_NATIVE_READ = false
        if not success then error(result) end
        return result
    end
    local api = ExposedMembers.JAC_LeaderPersona
    join(); grant_titles(0, DATE)
    eq('EVOLVING', api.PrepareEvolution(0, DATE))
    eq('APPOINTMENT_PENDING', api.CompleteEvolution(0, DATE))
    local governor = NEWGOV(0, DATE); governor.city = CITIES[0][1]
    eq('EVOLVED', api.CompleteEvolution(0, DATE))
    api.OnPlayerTurnStarted(0)
    eq(2, prop('MERIT'))
    set_merit(8); eq('PURCHASED', purchase('ENVOYS'))
    governor.established = false; TURN = 2; api.OnPlayerTurnStarted(0)
    eq(0, prop('MERIT'))
    governor.established = true; governor.city = CITIES[0][2]
    TURN = 3; api.OnPlayerTurnStarted(0)
    eq(2, prop('MERIT'))
end

function CASES.test_missing_native_reader_never_creates_or_removes_a_person()
    local api = ExposedMembers.JAC_LeaderPersona
    local read = ExposedMembers.JAC_LeaderPersonaNativeRead
    ExposedMembers.JAC_LeaderPersonaNativeRead = nil
    eq('NATIVE_UNAVAILABLE', api.Join(0, 1)); eq(0, SPAWNS)
    ExposedMembers.JAC_LeaderPersonaNativeRead = read
    join(); grant_titles(0, DATE); api.PrepareEvolution(0, DATE); NEWGOV(0, DATE)
    ExposedMembers.JAC_LeaderPersonaNativeRead = nil
    eq('APPOINTMENT_PENDING', api.CompleteEvolution(0, DATE))
    eq('EVOLVING', prop('STATE')); eq(0, DESTROYS)
    ExposedMembers.JAC_LeaderPersonaNativeRead = read
    eq('EVOLVED', api.CompleteEvolution(0, DATE))
end

function CASES.test_non_great_person_unit_type_never_reads_an_individual_identity()
    NATIVE_UNITS[0][77] = {GetID=function() return 77 end,
        GetOwner=function() return 0 end, GetType=function() return 0 end,
        GetGreatPerson=function() error('ordinary unit has no person identity') end}
    join(); eq(1, SPAWNS); eq(1, prop('UNIT'))
end

function CASES.test_purchase_is_only_available_through_the_gameplay_event()
    eq(nil, ExposedMembers.JAC_LeaderPersona.Purchase)
end

function CASES.test_governor_panel_preserves_foreign_governors_but_blocks_official_and_unjoined_personas()
    GOVDEFS[#GOVDEFS+1] = {GovernorType='GOVERNOR_OTHER_MOD',Index=90,Hash=9090}
    GOVDEFS[#GOVDEFS+1] = {GovernorType='GOVERNOR_THE_DEFENDER',Index=91,Hash=9191}
    GameInfo.Governors = collection(GOVDEFS,'GovernorType')
    GameInfo.JAC_PersonaDisabledGovernors = collection({{GovernorType='GOVERNOR_THE_DEFENDER'}},'GovernorType')
    PANEL_CALLS = {}
    AddGovernorCandidate = function(def,can) PANEL_CALLS[#PANEL_CALLS+1]='candidate:'..def.GovernorType..':'..tostring(can) end
    AddGovernorShared = function() return {GovernorPortrait=control(),GovernorTitle=control()} end
    AddGovernorAppointed = function(_,_,def) PANEL_CALLS[#PANEL_CALLS+1]='appointed:'..def.GovernorType end
    OnAppointGovernor = function(id) PANEL_CALLS[#PANEL_CALLS+1]='appoint:'..id end
    OnNameButton = function(id) PANEL_CALLS[#PANEL_CALLS+1]='name:'..id end
    OnPromoteGovernor = function(id,promotion) PANEL_CALLS[#PANEL_CALLS+1]='promote:'..id..':'..promotion end
    LOAD_MOD('governor_panel')
    AddGovernorCandidate(GameInfo.Governors[90],true)
    OnAppointGovernor(90); OnPromoteGovernor(90,77); OnNameButton(90)
    AddGovernorCandidate(GameInfo.Governors[91],true)
    OnAppointGovernor(91); OnPromoteGovernor(91,77)
    AddGovernorCandidate(GameInfo.Governors[22],true)
    OnAppointGovernor(22)
    AddGovernorAppointed(nil,nil,GameInfo.Governors[22],true)
    eq({'candidate:GOVERNOR_OTHER_MOD:true','appoint:90','promote:90:77','name:90'}, PANEL_CALLS)
    join(); grant_titles(0,DATE); ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,DATE)
    AddGovernorAppointed(nil,nil,GameInfo.Governors[22],true)
    eq(4,#PANEL_CALLS)
    Game:SetProperty('JAC_PERSONA_'..DATE..'_STATE','GOVERNOR')
    AddGovernorAppointed(nil,nil,GameInfo.Governors[22],false)
    eq('appointed:GOVERNOR_JAC_PERSONA_DATE_TANEMUNE',PANEL_CALLS[5])
end

function CASES.test_only_established_unneutralized_city_governance_earns_once_across_reload()
    local governor = settled()
    governor.established = false; FIRE('PlayerTurnStarted',0); eq(0,prop('MERIT'))
    governor.established = true; governor.neutralized = 2
    FIRE('PlayerTurnStarted',0); eq(0,prop('MERIT'))
    governor.neutralized = 0; FIRE('PlayerTurnStarted',0)
    eq(0,prop('MERIT')); eq(1,prop('GOVERN_LAST_TURN'))
    reload(); FIRE('PlayerTurnStarted',0); eq(0,prop('MERIT'))
    TURN=2; FIRE('PlayerTurnStarted',0); eq(2,prop('MERIT'))
    governor.city=nil; TURN=3; FIRE('PlayerTurnStarted',0); eq(2,prop('MERIT'))
end

function CASES.test_personal_profiles_pay_different_amounts_and_never_share_balances()
    settled(DATE,1); settled(HOJO,2)
    FIRE('PlayerTurnStarted',0); eq(2,prop('MERIT')); eq(3,prop('MERIT',HOJO))
    FIRE('BuildingConstructed',0,2,0,100,true)
    eq(2,prop('MERIT')); eq(9,prop('MERIT',HOJO))
    set_merit(10,HOJO); eq('PURCHASED',purchase('TAX',1,HOJO))
    eq(2,prop('MERIT')); eq(2,prop('MERIT',HOJO)); eq(nil,prop('CATEGORY'))
end

function CASES.test_building_deeds_reject_repairs_wrong_city_duplicates_and_enforce_cap()
    settled()
    FIRE('BuildingConstructed',0,1,0,100,false)
    FIRE('BuildingConstructed',0,2,0,101,true)
    FIRE('BuildingConstructed',0,1,99999,100,true)
    eq(0,prop('MERIT'))
    for building=0,2 do FIRE('BuildingConstructed',0,1,building,100,true) end
    eq(6,prop('MERIT'))
    reload(); TURN=2
    for building=0,2 do FIRE('BuildingConstructed',0,1,building,100,true) end
    eq(6,prop('MERIT'))
    FIRE('BuildingConstructed',0,1,3,100,true)
    eq(9,prop('MERIT'))
end

function CASES.test_nearby_victories_pay_personal_merit_and_ignore_nonlethal_or_distant_battles()
    settled(); combat(nil,nil,false); combat(nil,10)
    eq(0,prop('MERIT'))
    combat(901,3); eq(2,prop('MERIT'))
    reload(); combat(901,3); eq(2,prop('MERIT'))
    combat(902); combat(903); eq(4,prop('MERIT'))
end

function CASES.test_unproven_building_and_victory_notifications_survive_reload_without_late_rewards()
    settled()
    local city=CITIES[0][1]; local getter=city.GetProperty
    city.GetProperty=nil
    FIRE('BuildingConstructed',0,1,0,100,true); combat(nil,3)
    eq(0,prop('MERIT')); eq(nil,prop('DEED_BUILDING_100_0'))
    eq(2,count(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')))
    eq(1,Game:GetProperty('JAC_PERSONA_PENDING_NATIVE_HISTORY'))
    city.GetProperty=getter; reload(); FIREUI('LoadScreenClose')
    eq(0,#REQUESTS); eq(0,prop('MERIT'))
    eq(2,count(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')))
    eq(1,Game:GetProperty('JAC_PERSONA_PENDING_NATIVE_HISTORY'))
    FIRE('BuildingConstructed',0,1,0,100,true); combat(nil,3)
    eq(0,prop('MERIT'))
    FIRE('BuildingConstructed',0,1,1,100,true); combat(901,3)
    eq(5,prop('MERIT'))
end

function CASES.test_same_turn_assignment_establishment_or_recovery_cannot_make_old_construction_eligible()
    for _,change in ipairs({'assignment','establishment','neutralization'}) do
        RESET_TEST()
        local governor=settled()
        if change=='assignment' then governor.city=CITIES[0][2] end
        if change=='establishment' then governor.established=false end
        if change=='neutralization' then governor.neutralized=2 end
        FIRE('BuildingConstructed',0,1,0,100,true)
        governor.city=CITIES[0][1]; governor.established=true; governor.neutralized=0
        reload(); FIRE('BuildingConstructed',0,1,0,100,true)
        eq(0,prop('MERIT')); eq(nil,Game:GetProperty('JAC_PERSONA_PENDING_DEEDS'))
        FIRE('BuildingConstructed',0,1,1,100,true); eq(3,prop('MERIT'))
    end
end

function CASES.test_ai_turn_dispatch_uses_committed_state_when_ui_reader_is_missing()
    local outcomes={}
    for _,reader_present in ipairs({false,true}) do
        RESET_TEST(); settled(); set_merit(100)
        Players[0].human=false; Players[0].allies[2]=true
        if not reader_present then ExposedMembers.JAC_LeaderPersonaNativeRead=nil end
        ExposedMembers.JAC_LeaderPersona.OnPlayerTurnStarted(0)
        outcomes[#outcomes+1]={prop('CATEGORY'),prop('AI_LAST_TURN'),prop('NODE_'..DATE..'_ENVOYS_1'),prop('ALLY_SEEN_2')}
        eq(94,prop('MERIT')); eq(1,Game:GetProperty('JAC_PERSONA_PENDING_NATIVE_HISTORY'))
        reload(); ExposedMembers.JAC_LeaderPersona.OnPlayerTurnStarted(0)
        eq(1,prop('AI_LAST_TURN'))
    end
    eq(outcomes[1],outcomes[2]); eq({'CONTACT',1,1,1},outcomes[1])
end

function CASES.test_unproven_deeds_cannot_pay_again_on_later_turns()
    settled(); local city=CITIES[0][1]; local getter=city.GetProperty
    city.GetProperty=nil
    for building=0,2 do FIRE('BuildingConstructed',0,1,building,100,true) end
    city.GetProperty=getter; reload(); eq(0,prop('MERIT'))
    TURN=2
    for building=0,2 do FIRE('BuildingConstructed',0,1,building,100,true) end
    eq(0,prop('MERIT'))
    FIRE('BuildingConstructed',0,1,3,100,true); eq(3,prop('MERIT'))
end

function CASES.test_deferred_events_preserve_ai_turn_before_later_construction()
    settled(); set_merit(5); Players[0].human=false
    local city=CITIES[0][1]; local getter=city.GetProperty
    city.GetProperty=nil
    ExposedMembers.JAC_LeaderPersona.OnPlayerTurnStarted(0)
    FIRE('BuildingConstructed',0,1,0,100,true)
    city.GetProperty=getter; reload()
    eq(5,prop('MERIT')); eq(nil,prop('CATEGORY')); eq(1,prop('AI_LAST_TURN'))
    eq(2,count(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')))
end

function CASES.test_historical_pending_deed_is_kept_without_inventing_eligibility()
    settled(); local city=CITIES[0][1]; local getter=city.GetProperty
    city.GetProperty=nil; FIRE('BuildingConstructed',0,1,0,100,true)
    city.GetProperty=getter; Game:SetProperty('JAC_PERSONA_PENDING_NATIVE_HISTORY',nil)
    TURN=2; reload()
    eq(0,prop('MERIT'))
    local pending=Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')
    eq(1,count(pending))
    local event=pending[DATE..'_BUILDING_100_0']
    eq(1,event.turn); eq(1,event.cityID); eq('100_0',event.identity)
    eq(1,Game:GetProperty('JAC_PERSONA_PENDING_NATIVE_HISTORY'))
end

function CASES.test_turn_scan_records_alliances_without_inventing_formation_state()
    Players[0].allies[1]=true; settled()
    FIRE('PlayerTurnStarted',0); eq(2,prop('MERIT'))
    Players[0].allies[2]=true; TURN=2; FIRE('PlayerTurnStarted',0)
    eq(4,prop('MERIT'))
    local pending=Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')
    eq(2,pending[DATE..'_ALLIANCE_2'].turn)
    eq(true,pending[DATE..'_ALLIANCE_2'].eligibilityUnknown)
    Players[0].allies[2]=false; TURN=3; FIRE('PlayerTurnStarted',0)
    reload(); Players[0].allies[2]=true; TURN=4; FIRE('PlayerTurnStarted',0)
    eq(8,prop('MERIT'))
    eq(1,count(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')))
end

function CASES.test_eligible_alliance_formation_pays_eight_once_across_update_reload_and_renewal()
    settled(); FIREUI('DiplomacyRelationshipChanged',0,2)
    eq(nil,prop('ALLY_SEEN_2'))
    Players[0].allies[2]=true; FIRE('OnDiplomacyRelationshipChanged',0,2)
    eq(0,prop('MERIT'))
    FIREUI('DiplomacyRelationshipChanged',0,2)
    eq(8,prop('MERIT')); eq(1,prop('ALLY_SEEN_2'))
    FIREUI('DiplomacyRelationshipChanged',2,0); eq(8,prop('MERIT'))
    reload(); FIREUI('DiplomacyRelationshipChanged',0,2); FIRE('PlayerTurnStarted',0)
    eq(10,prop('MERIT'))
    Players[0].allies[2]=false; FIREUI('DiplomacyRelationshipChanged',0,2)
    Players[0].allies[2]=true; FIREUI('DiplomacyRelationshipChanged',0,2)
    eq(10,prop('MERIT')); eq(nil,Game:GetProperty('JAC_PERSONA_PENDING_DEEDS'))
end

function CASES.test_preexisting_alliance_does_not_pay_on_relationship_updates()
    Players[0].allies[1]=true; settled()
    FIREUI('DiplomacyRelationshipChanged',0,1); eq(0,prop('MERIT'))
    Players[0].allies[2]=true; FIREUI('DiplomacyRelationshipChanged',0,2)
    eq(8,prop('MERIT')); eq(1,prop('ALLY_SEEN_2'))
    TURN=2; FIREUI('DiplomacyRelationshipChanged',0,1)
    FIREUI('DiplomacyRelationshipChanged',0,2); eq(8,prop('MERIT'))
end

function CASES.test_alliance_cap_records_two_new_partners_without_next_turn_payment()
    settled(); Players[0].allies[1]=true
    FIREUI('DiplomacyRelationshipChanged',0,1)
    Players[0].allies[2]=true; FIREUI('DiplomacyRelationshipChanged',0,2)
    eq(8,prop('MERIT')); eq(1,prop('ALLY_SEEN_1')); eq(1,prop('ALLY_SEEN_2'))
    TURN=2; FIREUI('DiplomacyRelationshipChanged',0,2); eq(8,prop('MERIT'))
end

function CASES.test_absent_gamecore_alliance_event_keeps_turn_scan_unproven()
    settled(); RESET_CONTEXTS()
    Events.DiplomacyRelationshipChanged=false
    LOAD_CONTEXTS()
    Players[0].allies[2]=true; FIRE('PlayerTurnStarted',0)
    eq(2,prop('MERIT'))
    local pending=Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')
    eq(true,pending[DATE..'_ALLIANCE_2'].eligibilityUnknown)
end

function CASES.test_transfer_does_not_revalue_old_city_deed_and_new_city_can_earn()
    local governor=settled()
    governor.established=false; governor.city=CITIES[0][2]
    FIRE('BuildingConstructed',0,2,0,200,true)
    governor.established=true; reload()
    FIRE('BuildingConstructed',0,2,0,200,true); eq(0,prop('MERIT'))
    FIRE('BuildingConstructed',0,1,1,100,true); eq(0,prop('MERIT'))
    FIRE('BuildingConstructed',0,2,1,200,true); eq(3,prop('MERIT'))
end

function CASES.test_ai_six_point_boundary_is_independent_of_ui_reader()
    local outcomes={}
    for _,reader_present in ipairs({false,true}) do
        RESET_TEST(); settled(); set_merit(6); Players[0].human=false
        if not reader_present then ExposedMembers.JAC_LeaderPersonaNativeRead=nil end
        FIRE('PlayerTurnStarted',0)
        outcomes[#outcomes+1]={prop('MERIT'),prop('CATEGORY'),
            prop('NODE_'..DATE..'_ENVOYS_1'),prop('AI_LAST_TURN')}
        reload(); FIRE('PlayerTurnStarted',0); eq(0,prop('MERIT'))
    end
    eq({0,'CONTACT',1,1},outcomes[1]); eq(outcomes[1],outcomes[2])
end

function CASES.test_zero_markers_are_ineligible_without_history_but_duplicate_markers_are_unknown()
    local governor=settled()
    local marker='JAC_PERSONA_SETTLED_'..DATE
    governor.city=nil
    FIRE('PlayerTurnStarted',0); FIRE('BuildingConstructed',0,1,0,100,true)
    eq(0,prop('MERIT'))
    eq(nil,Game:GetProperty('JAC_PERSONA_PENDING_DEEDS'))
    eq(nil,Game:GetProperty('JAC_PERSONA_PENDING_NATIVE_HISTORY'))
    governor.city=CITIES[0][1]; CITIES[0][2].markerOverrides[marker]=1
    TURN=2; FIRE('PlayerTurnStarted',0)
    local pending=Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')
    eq(true,pending[DATE..'_TURN_2'].eligibilityUnknown)
    eq(1,Game:GetProperty('JAC_PERSONA_PENDING_NATIVE_HISTORY'))
    CITIES[0][2].markerOverrides[marker]=nil; reload()
    FIRE('PlayerTurnStarted',0); eq(0,prop('MERIT'))
    TURN=3; FIRE('PlayerTurnStarted',0); eq(2,prop('MERIT'))
end

function CASES.test_alliance_formed_before_assignment_establishment_or_recovery_never_pays_late()
    for _,change in ipairs({'assignment','establishment','neutralization'}) do
        RESET_TEST()
        local governor=settled()
        if change=='assignment' then governor.city=nil
        elseif change=='establishment' then governor.established=false
        else governor.neutralized=2 end
        ExposedMembers.JAC_LeaderPersona.OnPlayerTurnStarted(0)
        Players[0].allies[2]=true
        governor.city=CITIES[0][1]; governor.established=true; governor.neutralized=0
        TURN=2; ExposedMembers.JAC_LeaderPersona.OnPlayerTurnStarted(0)
        eq(2,prop('MERIT')); eq(1,prop('ALLY_SEEN_2'))
        local pending=Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')
        eq(true,pending[DATE..'_ALLIANCE_2'].eligibilityUnknown)
        reload(); ExposedMembers.JAC_LeaderPersona.OnPlayerTurnStarted(0)
        eq(2,prop('MERIT'))
    end
end

function CASES.test_same_turn_assignment_establishment_and_recovery_never_revalue_unknown_alliance()
    for _,change in ipairs({'assignment','establishment','neutralization'}) do
        RESET_TEST()
        local governor=settled()
        if change=='assignment' then governor.city=nil
        elseif change=='establishment' then governor.established=false
        else governor.neutralized=2 end
        Players[0].allies[2]=true; FIREUI('DiplomacyRelationshipChanged',0,2)
        governor.city=CITIES[0][1]; governor.established=true; governor.neutralized=0
        reload(); ExposedMembers.JAC_LeaderPersona.OnPlayerTurnStarted(0)
        eq(2,prop('MERIT')); eq(1,prop('ALLY_SEEN_2'))
        eq(nil,Game:GetProperty('JAC_PERSONA_PENDING_DEEDS'))
    end
end

function CASES.test_skill_purchase_requires_own_person_prerequisite_merit_and_confirmation()
    settled()
    local first=DATE..'_ENVOYS_1'
    eq('NOT_OWNED',request_purchase(2,DATE,first,'CONTACT','ENVOYS'))
    eq('INVALID_NODE',request_purchase(0,DATE,ODA..'_EXCHANGE_1','CONTACT','EXCHANGE'))
    eq('PREREQ_LOCKED',purchase('ENVOYS',2)); eq('NO_MERIT',purchase('ENVOYS'))
    set_merit(8)
    eq('CONFIRM_REQUIRED',request_purchase(0,DATE,first))
    eq('CONFIRM_REQUIRED',request_purchase(0,DATE,first,'ADMIN','ENVOYS'))
    eq(nil,prop('CATEGORY')); eq(8,prop('MERIT'))
    local points,spent,grants=Players[0].points,Players[0].spent,#GRANTS[0]
    eq('PURCHASED',purchase('ENVOYS')); eq(0,prop('MERIT'))
    eq(points,Players[0].points); eq(spent,Players[0].spent); eq(grants,#GRANTS[0])
    eq('CONTACT',prop('CATEGORY')); eq('ENVOYS',prop('BRANCH_CONTACT_RELATIONS'))
    eq(1,prop('NODE_'..first)); eq('OWNED',purchase('ENVOYS')); eq(0,prop('MERIT'))
end

function CASES.test_malformed_gameplay_requests_do_not_reach_native_lookups_or_change_state()
    settled(); set_merit(8)
    for _,leader in ipairs({0,{}}) do
        eq('NOT_OWNED',request_purchase(0,leader,DATE..'_ENVOYS_1','CONTACT','ENVOYS'))
        no(JAC_TreeOpen(leader))
    end
    eq('NOT_OWNED',request_purchase(0,nil,DATE..'_ENVOYS_1','CONTACT','ENVOYS'))
    no(JAC_TreeOpen(nil))
    for _,node in ipairs({0,{},'unknown'}) do
        eq('INVALID_NODE',request_purchase(0,DATE,node,'CONTACT','ENVOYS'))
    end
    eq('INVALID_NODE',request_purchase(0,DATE,nil,'CONTACT','ENVOYS'))
    eq(8,prop('MERIT')); eq(nil,prop('CATEGORY'))
end

function CASES.test_irreversible_category_and_exclusive_sibling_locks_survive_load()
    settled(); set_merit(100)
    eq('PURCHASED',purchase('ENVOYS'))
    eq('CATEGORY_LOCKED',purchase('WORKS'))
    eq('BRANCH_LOCKED',purchase('EXCHANGE'))
    eq('PREREQ_LOCKED',purchase('ENVOYS',3))
    eq('PURCHASED',purchase('ENVOYS',2)); eq('PURCHASED',purchase('ENVOYS',3))
    eq('PURCHASED',purchase('HOSTING')); eq('BRANCH_LOCKED',purchase('MARKET'))
    eq('PURCHASED',purchase('SIGNATURE'))
    local balance=prop('MERIT'); reload()
    eq(balance,prop('MERIT'))
    eq('OWNED',purchase('ENVOYS')); eq('BRANCH_LOCKED',purchase('EXCHANGE'))
    eq('CATEGORY_LOCKED',purchase('WORKS')); eq('BRANCH_LOCKED',purchase('MARKET'))
    eq(balance,prop('MERIT'))
end

function CASES.test_all_six_shared_promotions_and_three_signature_nodes_can_be_acquired()
    settled(); set_merit(200)
    for _,route in ipairs({'ENVOYS','HOSTING','SIGNATURE'}) do
        for depth=1,3 do eq('PURCHASED',purchase(route,depth)) end
    end
    eq(200-38-47-38,prop('MERIT'))
    eq(nil,ExposedMembers.JAC_LeaderPersona.Refund)
    eq(nil,ExposedMembers.JAC_LeaderPersona.Respec)
end

function CASES.test_owned_node_flags_follow_native_assignment_and_new_city_centres()
    local governor=settled(); set_merit(8); eq('PURCHASED',purchase('ENVOYS'))
    local modifier=GameInfo.JAC_PersonaTreeNodes[DATE..'_ENVOYS_1'].ModifierId
    for x=1,2 do eq(1,Map.GetPlot(x,0):GetProperty(modifier)) end
    JAC_AssignLeader(DATE,2); DRAIN()
    eq(2,governor.city:GetID())
    local promotions=0
    for _,owned in pairs(governor.promotions) do if owned then promotions=promotions+1 end end
    eq(2,promotions); eq(2,Players[0].points); eq(2,Players[0].spent)
    CITIES[0][3]={GetID=function() return 3 end,GetOwner=function() return 0 end,
        GetX=function() return 3 end,GetY=function() return 0 end}
    FIRE('CityBuilt',0,3,3,0)
    eq(1,Map.GetPlot(3,0):GetProperty(modifier))
end

function CASES.test_ai_uses_personality_role_preference_and_only_one_purchase_each_turn()
    settled(); set_merit(100); Players[0].human=false
    FIRE('PlayerTurnStarted',0)
    eq('CONTACT',prop('CATEGORY')); eq('ENVOYS',prop('BRANCH_CONTACT_RELATIONS'))
    eq(94,prop('MERIT'))
    reload(); FIRE('PlayerTurnStarted',0)
    eq(94,prop('MERIT')); eq(nil,prop('NODE_'..DATE..'_ENVOYS_2'))
    TURN=2; FIRE('PlayerTurnStarted',0)
    eq(1,prop('NODE_'..DATE..'_ENVOYS_2')); eq(84,prop('MERIT'))
end

function CASES.test_tree_confirmation_lists_losses_and_cancel_changes_nothing()
    settled(); set_merit(8)
    yes(JAC_TreeOpen(DATE)); yes(JAC_TreeReview(DATE..'_ENVOYS_1'))
    no(Controls.Confirmation.hidden)
    eq(nil,prop('CATEGORY')); eq(8,prop('MERIT'))
    local categories,routes=JAC_PersonaTreeRules.Locks(DATE,GameInfo.JAC_PersonaTreeNodes[DATE..'_ENVOYS_1'])
    local category_set={}
    for _,row in pairs(categories) do category_set[row.CategoryType]=true end
    eq({MILITARY=true,ADMIN=true},category_set)
    local route_types={}
    for _,row in pairs(routes) do route_types[#route_types+1]=row.RouteType end
    eq({'EXCHANGE'},route_types)
    JAC_TreeCancel(); yes(Controls.Confirmation.hidden); no(JAC_TreeConfirm())
    eq(8,prop('MERIT')); eq(0,#REQUESTS)
end

function CASES.test_tree_spends_only_via_gameplay_request_and_rechecks_queued_duplicates()
    settled(); set_merit(30); JAC_TreeOpen(DATE)
    local node=DATE..'_ENVOYS_1'
    for _=1,2 do yes(JAC_TreeReview(node)); yes(JAC_TreeConfirm()) end
    eq(2,#REQUESTS)
    for _,request in ipairs(REQUESTS) do
        eq(1,request.op); eq('JAC_LeaderPersonaPurchase',request.params.OnStart)
    end
    eq(30,prop('MERIT')); DRAIN()
    eq(22,prop('MERIT')); eq(1,prop('NODE_'..node))
    eq(2,Players[0].points); eq(2,Players[0].spent)
end

function CASES.test_queued_conflicting_confirmations_cannot_buy_sibling_or_other_category()
    settled(); set_merit(40)
    for _,route in ipairs({'ENVOYS','EXCHANGE','WORKS'}) do
        local node=DATE..'_'..route..'_1'
        yes(JAC_TreeOpen(DATE,node)); yes(JAC_TreeConfirm())
    end
    DRAIN(); eq(32,prop('MERIT'))
    eq(1,prop('NODE_'..DATE..'_ENVOYS_1'))
    eq(nil,prop('NODE_'..DATE..'_EXCHANGE_1'))
    eq(nil,prop('NODE_'..DATE..'_WORKS_1'))
    eq('CONTACT',prop('CATEGORY')); eq('ENVOYS',prop('BRANCH_CONTACT_RELATIONS'))
end

function CASES.test_tree_rechecks_stale_balance_at_confirmation_and_respects_inactive_turn()
    settled(); set_merit(8); JAC_TreeOpen(DATE)
    JAC_TreeReview(DATE..'_ENVOYS_1'); set_merit(0)
    no(JAC_TreeConfirm()); eq(0,#REQUESTS)
    set_merit(8); Players[0].active=false
    no(JAC_TreeReview(DATE..'_ENVOYS_1')); no(JAC_TreeConfirm())
    eq(nil,prop('CATEGORY'))
end

function CASES.test_tree_only_shows_matching_new_purchase_receipts_and_never_replays_old_results()
    settled(); set_merit(30)
    local p=Players[0]
    p:SetProperty('JAC_PERSONA_RESULT','PURCHASED')
    p:SetProperty('JAC_PERSONA_PURCHASE_SERIAL',5)
    p:SetProperty('JAC_PERSONA_PURCHASE_LEADER',DATE)
    p:SetProperty('JAC_PERSONA_PURCHASE_NODE',DATE..'_ENVOYS_1')
    p:SetProperty('JAC_PERSONA_PURCHASE_RESULT','PURCHASED')
    JAC_TreeOpen(DATE); FIREUI('GameCoreEventPublishComplete')
    eq('',Controls.Result.text)
    JAC_TreeReview(DATE..'_ENVOYS_1'); JAC_TreeConfirm()
    eq(6,REQUESTS[1].params.RequestSerial)
    FIREUI('GameCoreEventPublishComplete'); eq('',Controls.Result.text)
    DRAIN(); eq('LOC_JAC_PERSONA_NODE_STATUS_PURCHASED',Controls.Result.text)
    JAC_TreeOpen(DATE); FIREUI('GameCoreEventPublishComplete'); eq('',Controls.Result.text)
    reload(); JAC_TreeOpen(DATE)
    JAC_TreeReview(DATE..'_ENVOYS_2'); JAC_TreeConfirm()
    eq(7,REQUESTS[1].params.RequestSerial)
    p:SetProperty('JAC_PERSONA_PURCHASE_SERIAL',7)
    p:SetProperty('JAC_PERSONA_PURCHASE_NODE',DATE..'_HOSTING_1')
    FIREUI('GameCoreEventPublishComplete'); eq('',Controls.Result.text)
    DRAIN(); eq('LOC_JAC_PERSONA_NODE_STATUS_PURCHASED',Controls.Result.text)
end

function CASES.test_live_reopen_assertion_accepts_empty_native_label_and_rejects_old_receipt()
    settled()
    for _,getter in ipairs({'return ""','return nil','return'}) do
        Controls.Result.GetText=assert(loadstring('return function() '..getter..' end'))()
        has(assert(loadstring(SCENARIO_REOPEN_CODE))(),'tree reopened after load')
    end
    JAC_TreeOpen=function() return true end
    Controls.Result.GetText=function() return 'previous purchase result' end
    raises('old purchase result replayed on reopen: previous purchase result',
        function() assert(loadstring(SCENARIO_REOPEN_CODE))() end)
end

function CASES.test_keyboard_can_review_cancel_commit_switch_categories_and_cycle_governors()
    settled(); set_merit(20); JAC_TreeOpen(DATE)
    yes(JAC_TreeInput(INPUT_KEY(13))); no(Controls.Confirmation.hidden)
    JAC_TreeInput(INPUT_KEY(27)); yes(Controls.Confirmation.hidden)
    JAC_TreeInput(INPUT_KEY(13)); JAC_TreeInput(INPUT_KEY(13)); DRAIN()
    eq(1,prop('NODE_'..DATE..'_ENVOYS_1'))
    JAC_TreeInput(INPUT_KEY(40)); JAC_TreeInput(INPUT_KEY(39))
    yes(Controls.NodesScroll.scroll>=0 and Controls.NodesScroll.scroll<=1)
    JAC_TreeInput(INPUT_KEY(9)); JAC_TreeInput(INPUT_KEY(27))
    yes(ContextPtr.hidden)
end

function CASES.test_promotion_titles_fixture_is_removed_and_official_candidates_are_hidden()
    join(); eq(nil,ExposedMembers.JAC_LeaderPersona.GrantTestTitle); eq(0,Players[0].points)
    CANDIDATES,APPOINTED={},{}
    AddGovernorCandidate=function(def) CANDIDATES[#CANDIDATES+1]=def.GovernorType end
    AddGovernorShared=function() return {GovernorPortrait=control(),GovernorTitle=control()} end
    OnAppointGovernor=function(id) APPOINTED[#APPOINTED+1]=id end
    GameInfo.Governors[99]={GovernorType='GOVERNOR_THE_EDUCATOR',Index=99,Hash=999}
    LOAD_MOD('governor_panel')
    AddGovernorCandidate(GameInfo.Governors[99],true); OnAppointGovernor(99)
    eq(0,#CANDIDATES); eq(0,#APPOINTED)
end

function CASES.test_void_creation_keeps_named_individual_and_metadata()
    join()
    eq('GREAT_PERSON',prop('STATE'))
    eq(1,prop('SOURCE')); eq(0,prop('OWNER')); eq(1,SPAWNS)
    eq('LOC_'..DATE..'_NAME',prop('NAME')); eq('ICON_'..DATE,prop('PORTRAIT'))
    eq('CALM',prop('PERSONALITY')); eq('ASTUTE',prop('TRAIT'))
    eq(12,NATIVE_UNITS[0][1].individual)
    eq(true,Players[1].alive)
end

function CASES.test_synchronized_join_validates_source_and_replays_only_once()
    JAC_RequestLeaderJoin(1)
    eq(0,SPAWNS); eq(nil,prop('OWNER'))
    DRAIN(); eq(0,prop('OWNER')); eq(1,prop('SOURCE')); eq(1,SPAWNS)
    FIRE('JAC_LeaderPersonaJoin',0,{Source=1})
    eq(1,SPAWNS); eq('ALREADY_JOINED',Players[0]:GetProperty('JAC_PERSONA_RESULT'))
    for _,params in ipairs({{}, {Source='1'}, {Source=0}}) do
        FIRE('JAC_LeaderPersonaJoin',0,params)
        eq(1,SPAWNS); eq('INVALID',Players[0]:GetProperty('JAC_PERSONA_RESULT'))
    end
    FIRE('JAC_LeaderPersonaJoin',0,nil)
    eq(1,SPAWNS); eq('INVALID',Players[0]:GetProperty('JAC_PERSONA_RESULT'))
end

function CASES.test_designed_personality_trait_and_cost_are_join_defaults()
    local api=ExposedMembers.JAC_LeaderPersona
    eq('JOINED',api.Join(0,1))
    eq('CALM',prop('PERSONALITY')); eq('ASTUTE',prop('TRAIT')); eq(2,prop('EVOLUTION_COST'))
    eq('JOINED',api.Join(1,0))
    eq('BOLD',prop('PERSONALITY',ODA)); eq('ASTUTE',prop('TRAIT',ODA))
    eq(1,prop('EVOLUTION_COST',ODA))
end

function CASES.test_fixed_trait_cannot_be_replaced_at_join()
    local api=ExposedMembers.JAC_LeaderPersona
    eq('INVALID_METADATA',api.Join(0,1,'CALM','VALIANT')); eq(0,SPAWNS)
    eq('JOINED',api.Join(0,1,'CAREFUL'))
    eq('CAREFUL',prop('PERSONALITY')); eq('ASTUTE',prop('TRAIT'))
end

function CASES.test_partial_titles_refuse_costlier_date_but_suffice_for_oda()
    local api=ExposedMembers.JAC_LeaderPersona
    join(); Players[0].points=1
    eq('NO_TITLE',api.PrepareEvolution(0,DATE))
    eq('GREAT_PERSON',prop('STATE')); eq(1,Players[0].points); eq(0,DESTROYS)
    JAC_RefreshPersonas(); yes(VIEWS[1].disabled)
    eq('JOINED',api.Join(1,0)); Players[1].points=1
    eq('EVOLVING',api.PrepareEvolution(1,ODA))
    eq(1,Players[1].points); eq(0,prop('EVOLUTION_SURCHARGE',ODA))
end

function CASES.test_pending_surcharge_survives_reload_and_spending_without_double_charge()
    local api=ExposedMembers.JAC_LeaderPersona
    join(); Players[0].points=2
    eq('EVOLVING',api.PrepareEvolution(0,DATE))
    eq(2,Players[0].points); eq(1,prop('EVOLUTION_SURCHARGE'))
    yes(JAC_AppointLeader(DATE)); STEP()
    eq(2,Players[0].points); eq(1,Players[0].spent)
    Players[0].spent=2
    no(JAC_AppointLeader(DATE))
    eq('TITLE_PAYMENT_PENDING',api.CompleteEvolution(0,DATE))
    reload(); eq('EVOLVING',ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,DATE))
    eq(2,Players[0].points); eq(0,DESTROYS)
    Players[0].points=3; JAC_AppointLeader(DATE); DRAIN()
    eq('GOVERNOR',prop('STATE')); eq(3,Players[0].points); eq(3,Players[0].spent)
    eq({},GRANTS[0])
end

function CASES.test_two_pending_evolutions_cannot_share_the_native_appointment_title()
    local api=ExposedMembers.JAC_LeaderPersona
    join(); eq('JOINED',api.Join(0,2)); Players[0].points=3
    eq('EVOLVING',api.PrepareEvolution(0,DATE))
    eq('NO_TITLE',api.PrepareEvolution(0,HOJO))
    eq('GREAT_PERSON',prop('STATE',HOJO)); eq(3,Players[0].points)
    eq(nil,prop('EVOLUTION_SURCHARGE',HOJO))
    JAC_RefreshPersonas(); yes(VIEWS[1].disabled); yes(VIEWS[2].disabled)
    reload(); eq('NO_TITLE',ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,HOJO))
    JAC_AppointLeader(DATE); DRAIN(); eq('GOVERNOR',prop('STATE'))
    eq('NO_TITLE',ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,HOJO))
    Players[0].points=Players[0].points+1
    eq('EVOLVING',ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,HOJO))
    JAC_AppointLeader(HOJO); DRAIN()
    eq('GOVERNOR',prop('STATE',HOJO))
    eq(4,Players[0].points); eq(4,Players[0].spent); eq({},GRANTS[0])
end

function CASES.test_negative_point_change_is_not_required_for_native_payment()
    join(); Players[0].points=2
    local original=Players[0].GetGovernors
    Players[0].GetGovernors=function()
        local g=original(); g.ChangeGovernorPoints=function() end; return g
    end
    JAC_EvolveLeader(DATE); DRAIN()
    eq('EVOLVED',Players[0]:GetProperty('JAC_PERSONA_RESULT'))
    eq('GOVERNOR',prop('STATE')); eq(1,prop('EVOLUTION_SURCHARGE')); eq(1,DESTROYS)
    eq(2,Players[0].points); eq(2,Players[0].spent)
end

function CASES.test_rejected_second_native_payment_waits_and_resumes_without_double_charge()
    GameInfo.JAC_LeaderPersonas[DATE].EvolutionCost=3
    local api=ExposedMembers.JAC_LeaderPersona
    join(); Players[0].points=3; REJECT_PROMOTION_STEP=2
    JAC_EvolveLeader(DATE); DRAIN()
    eq('EVOLVING',prop('STATE')); eq(2,prop('EVOLUTION_SURCHARGE')); eq(0,DESTROYS)
    eq(3,Players[0].points); eq(2,Players[0].spent)
    eq('TITLE_PAYMENT_PENDING',api.CompleteEvolution(0,DATE))
    JAC_EvolveLeader(DATE); DRAIN()
    eq(2,Players[0].spent); eq(0,DESTROYS)
    reload(); REJECT_PROMOTION_STEP=nil
    yes(JAC_AppointLeader(DATE)); DRAIN()
    eq('GOVERNOR',prop('STATE')); eq(3,Players[0].points)
    eq(3,Players[0].spent); eq(1,DESTROYS)
end

function CASES.test_upper_cost_consumes_two_extra_titles_and_one_native_once()
    GameInfo.JAC_LeaderPersonas[DATE].EvolutionCost=3
    local api=ExposedMembers.JAC_LeaderPersona
    join(); grant_titles(0,DATE)
    JAC_EvolveLeader(DATE); DRAIN()
    eq('GOVERNOR',prop('STATE')); eq(3,prop('EVOLUTION_COST'))
    eq(2,prop('EVOLUTION_SURCHARGE'))
    eq(3,Players[0].points); eq(3,Players[0].spent)
    reload(); eq('ALREADY_EVOLVED',ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,DATE))
    eq(nil,ExposedMembers.JAC_LeaderPersona.GrantTestTitle)
    eq({3},GRANTS[0]); eq(1,DESTROYS); eq(3,Players[0].points); eq(3,Players[0].spent)
end

function CASES.test_joined_cost_is_fixed_even_if_balance_data_changes()
    join(); GameInfo.JAC_LeaderPersonas[DATE].EvolutionCost=3
    eq(2,ExposedMembers.JAC_LeaderPersona.EvolutionCost(DATE))
end

function CASES.test_native_individual_identity_is_resolved_through_gameinfo()
    local api=ExposedMembers.JAC_LeaderPersona
    join()
    GameInfo.GreatPersonIndividuals[9002]=GameInfo.GreatPersonIndividuals[12]
    NATIVE_UNITS[0][1].individual=9002
    grant_titles(0,DATE)
    eq('EVOLVING',api.PrepareEvolution(0,DATE))
    NEWGOV(0,DATE)
    eq('EVOLVED',api.CompleteEvolution(0,DATE))
    eq(1,DESTROYS)
end

function CASES.test_repeated_join_and_other_owner_do_not_spawn_or_replace_metadata()
    local api=ExposedMembers.JAC_LeaderPersona
    join()
    eq('ALREADY_JOINED',api.Join(0,1,'BRAVE','VALIANT'))
    eq('OTHER_OWNER',api.Join(2,1))
    eq(1,SPAWNS); eq('CALM',prop('PERSONALITY')); eq('ASTUTE',prop('TRAIT'))
end

function CASES.test_invalid_requests_never_create_a_person()
    local api=ExposedMembers.JAC_LeaderPersona
    for _,result in ipairs({api.Join(0,0),api.Join(0,99),api.Join(99,1),
        api.Join(-1,1),api.Join(0.5,1),api.Join(0,1,'UNKNOWN'),api.Join(0,1,nil,'UNKNOWN')}) do
        yes(result=='INVALID' or result=='INVALID_METADATA')
    end
    PlayerConfigurations[1].civ='CIVILIZATION_EGYPT'
    eq('INVALID',api.Join(0,1)); eq(0,SPAWNS); eq(nil,prop('OWNER'))
end

function CASES.test_no_city_failed_spawn_and_retry()
    local api=ExposedMembers.JAC_LeaderPersona
    local city=CITIES[0][1]; CITIES[0][1]=nil
    eq('NO_CITY',api.Join(0,1))
    CITIES[0][1]=city; FAIL_SPAWN=true
    eq('SPAWN_FAILED',api.Join(0,1)); eq(nil,prop('STATE'))
    FAIL_SPAWN=false; join(); eq(2,SPAWNS)

    -- Execute the product Join for an AI owner; the enemy fixture's stub
    -- alone cannot prove that a matching Great Person is actually created.
    RESET_TEST(); LOAD_CONTEXTS()
    api=ExposedMembers.JAC_LeaderPersona
    Players[1].human=false
    FAIL_SPAWN=true
    eq('SPAWN_FAILED',api.Join(1,0)); eq(nil,prop('OWNER',ODA))
    eq(0,count(NATIVE_UNITS[1])); eq(0,count(NATIVE_UNITS[0]))
    FAIL_SPAWN=false
    eq('JOINED',api.Join(1,0)); eq(2,SPAWNS)
    eq(1,prop('OWNER',ODA)); eq(0,prop('SOURCE',ODA))
    eq('GREAT_PERSON',prop('STATE',ODA))
    local person=assert(Players[1]:GetUnits():FindID(prop('UNIT',ODA)))
    local individual=GameInfo.GreatPersonIndividuals[GameInfo.JAC_LeaderPersonas[ODA].PersonType]
    eq(1,person:GetOwner()); eq(individual.Index,person:GetGreatPerson():GetIndividual())
    eq(1,count(NATIVE_UNITS[1])); eq(0,count(NATIVE_UNITS[0]))
    eq('ALREADY_JOINED',api.Join(1,0)); eq(2,SPAWNS)
    reload()
    eq(person:GetID(),prop('UNIT',ODA))
    eq(individual.Index,Players[1]:GetUnits():FindID(prop('UNIT',ODA)):GetGreatPerson():GetIndividual())
end

function CASES.test_evolution_requires_ownership_person_and_native_title()
    local api=ExposedMembers.JAC_LeaderPersona
    eq('NOT_OWNED',api.PrepareEvolution(0,DATE))
    join(); eq('NOT_OWNED',api.PrepareEvolution(2,DATE))
    eq('NO_TITLE',api.PrepareEvolution(0,DATE)); eq('GREAT_PERSON',prop('STATE'))
    NATIVE_UNITS[0][1]=nil; grant_titles(0,DATE)
    eq('PERSON_MISSING',api.PrepareEvolution(0,DATE)); eq(0,DESTROYS)
end

function CASES.test_native_type_rejection_keeps_person_and_titles_without_reserving_evolution()
    local api=ExposedMembers.JAC_LeaderPersona
    join(); grant_titles(0,DATE)
    local governor=GameInfo.Governors[GameInfo.JAC_LeaderPersonas[DATE].GovernorType]
    NATIVE_APPOINTABLE[0][governor.Hash]=false
    yes(Players[0]:GetGovernors():CanAppoint())
    -- The previously expected precheck was circular: admission is opened only
    -- after the full official balance is reserved. A closed native gate cannot
    -- issue APPOINT or spend even if the state is pending.
    eq('EVOLVING',api.PrepareEvolution(0,DATE))
    eq('EVOLVING',prop('STATE')); eq(1,prop('EVOLUTION_SURCHARGE'))
    eq(2,Players[0].points); eq(0,Players[0].spent); eq(0,DESTROYS)
    JAC_RefreshPersonas(); yes(VIEWS[1].disabled)
    no(JAC_AppointLeader(DATE)); eq(0,#REQUESTS)
    reload(); eq('EVOLVING',ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,DATE))
    no(JAC_AppointLeader(DATE)); eq(0,#REQUESTS)
    NATIVE_APPOINTABLE[0][governor.Hash]=true
    yes(JAC_AppointLeader(DATE)); DRAIN()
    eq('GOVERNOR',prop('STATE')); eq(2,Players[0].points)
    eq(2,Players[0].spent); eq(1,DESTROYS)
end

function CASES.test_ai_admission_reserves_full_official_cost_before_any_native_spend()
    join(); Players[0].human=false
    local governor=GameInfo.Governors[GameInfo.JAC_LeaderPersonas[DATE].GovernorType]
    NATIVE_APPOINTABLE[0][governor.Hash]=false
    Players[0].points=1
    ExposedMembers.JAC_LeaderPersona.OnPlayerTurnStarted(0)
    eq('GREAT_PERSON',prop('STATE')); eq(0,Players[0].spent)
    Players[0].points=2
    ExposedMembers.JAC_LeaderPersona.OnPlayerTurnStarted(0)
    eq('EVOLVING',prop('STATE')); eq(1,prop('EVOLUTION_SURCHARGE'))
    no(JAC_AppointLeader(DATE)); eq(0,#REQUESTS)
    reload(); ExposedMembers.JAC_LeaderPersona.OnPlayerTurnStarted(0)
    eq('EVOLVING',prop('STATE')); eq(0,Players[0].spent)
end

function CASES.test_native_receipt_ahead_of_official_spent_keeps_full_reservation()
    join(); Players[0].points=3
    eq('EVOLVING',ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,DATE))
    eq(2,JAC_PersonaTreeRules.ReservedAppointments(0))
    NEWGOV(0,DATE,false)
    eq(2,JAC_PersonaTreeRules.ReservedAppointments(0))
    Players[0].spent=1
    eq(1,JAC_PersonaTreeRules.ReservedAppointments(0))
end

function CASES.test_prior_persona_payment_cannot_mask_unpaid_current_evolution()
    local api=ExposedMembers.JAC_LeaderPersona
    join(); Players[0].points=4
    eq('EVOLVING',api.PrepareEvolution(0,DATE))
    NEWGOV(0,DATE); Players[0].spent=2
    eq('EVOLVED',api.CompleteEvolution(0,DATE))
    eq('JOINED',api.Join(0,2))
    eq('EVOLVING',api.PrepareEvolution(0,HOJO))
    eq(2,JAC_PersonaTreeRules.ReservedAppointments(0))
    NEWGOV(0,HOJO,false)
    eq(2,JAC_PersonaTreeRules.ReservedAppointments(0))
    Players[0].spent=3
    eq(1,JAC_PersonaTreeRules.ReservedAppointments(0))
end

function CASES.test_lost_unappointed_person_releases_only_its_unpaid_reservation_after_next_turn()
    local api=ExposedMembers.JAC_LeaderPersona
    join(); eq('JOINED',api.Join(0,2)); Players[0].points=4
    eq('EVOLVING',api.PrepareEvolution(0,DATE))
    eq('EVOLVING',api.PrepareEvolution(0,HOJO))
    eq(4,JAC_PersonaTreeRules.ReservedAppointments(0))
    NATIVE_UNITS[0][1]=nil
    api.Reconcile(); eq('EVOLVING',prop('STATE'))
    eq(1,prop('MISSING_SINCE_TURN'))
    reload(); TURN=2; ExposedMembers.JAC_LeaderPersona.Reconcile()
    eq('GREAT_PERSON',prop('STATE')); eq(nil,prop('EVOLUTION_SURCHARGE'))
    eq(nil,prop('MISSING_SINCE_TURN'))
    eq('EVOLVING',prop('STATE',HOJO))
    eq(2,JAC_PersonaTreeRules.ReservedAppointments(0))
    yes(JAC_AppointLeader(HOJO)); DRAIN()
    eq('GOVERNOR',prop('STATE',HOJO))
    eq(2,Players[0].spent); eq(4,Players[0].points)
end

function CASES.test_pending_evolution_rechecks_native_type_before_requesting_appointment()
    local api=ExposedMembers.JAC_LeaderPersona
    join(); grant_titles(0,DATE)
    eq('EVOLVING',api.PrepareEvolution(0,DATE))
    local governor=GameInfo.Governors[GameInfo.JAC_LeaderPersonas[DATE].GovernorType]
    NATIVE_APPOINTABLE[0][governor.Hash]=false
    no(JAC_AppointLeader(DATE)); eq(0,#REQUESTS)
    eq('EVOLVING',prop('STATE')); eq(0,Players[0].spent); eq(0,DESTROYS)
    NATIVE_APPOINTABLE[0][governor.Hash]=true
    yes(JAC_AppointLeader(DATE)); DRAIN()
    eq('GOVERNOR',prop('STATE')); eq(2,Players[0].spent)
end

function CASES.test_native_reader_uses_requested_owner_and_target_type_independently_of_pool()
    local definition=GameInfo.Governors[GameInfo.JAC_LeaderPersonas[DATE].GovernorType]
    Players[0].points=3; Players[2].points=3
    NATIVE_APPOINTABLE[0][definition.Hash]=false
    local read=ExposedMembers.JAC_LeaderPersonaNativeRead
    no(read(0,DATE).canAppoint)
    yes(read(2,DATE).canAppoint)
    yes(read(0,HOJO).canAppoint)
end

function CASES.test_missing_type_getter_is_unavailable_and_never_uses_pool_as_eligibility()
    local api=ExposedMembers.JAC_LeaderPersona
    join(); grant_titles(0,DATE)
    local original=Players[0].GetGovernors
    Players[0].GetGovernors=function()
        local g=original(); g.CanAppointGovernor=nil; return g
    end
    eq('NATIVE_UNAVAILABLE',api.PrepareEvolution(0,DATE))
    eq('GREAT_PERSON',prop('STATE')); eq(0,Players[0].spent); eq(0,DESTROYS)
    JAC_RefreshPersonas(); yes(VIEWS[1].disabled)
end

function CASES.test_pending_appointment_keeps_unit_until_native_success()
    local api=ExposedMembers.JAC_LeaderPersona
    join(); grant_titles(0,DATE)
    eq('EVOLVING',api.PrepareEvolution(0,DATE))
    eq('APPOINTMENT_PENDING',api.CompleteEvolution(0,DATE)); eq(0,DESTROYS)
    eq('EVOLVING',api.PrepareEvolution(0,DATE))
    NEWGOV(0,DATE); eq('EVOLVED',api.CompleteEvolution(0,DATE))
    eq(nil,prop('UNIT'))
    eq('ALREADY_EVOLVED',api.CompleteEvolution(0,DATE)); eq(1,DESTROYS)
end

function CASES.test_delayed_removal_does_not_mark_evolution_complete()
    local api=ExposedMembers.JAC_LeaderPersona
    join(); grant_titles(0,DATE); api.PrepareEvolution(0,DATE); NEWGOV(0,DATE)
    FAIL_DESTROY=true
    eq('REMOVAL_PENDING',api.CompleteEvolution(0,DATE))
    eq('EVOLVING',prop('STATE'))
    FAIL_DESTROY=false; FIRE('OnGameTurnStarted')
    eq('GOVERNOR',prop('STATE'))
end

function CASES.test_native_candidate_object_without_appointment_does_not_remove_the_person()
    local api=ExposedMembers.JAC_LeaderPersona
    join(); grant_titles(0,DATE); api.PrepareEvolution(0,DATE); NEWGOV(0,DATE)
    local original=Players[0].GetGovernors
    Players[0].GetGovernors=function()
        local g=original(); g.HasGovernor=function() return false end; return g
    end
    eq('APPOINTMENT_PENDING',api.CompleteEvolution(0,DATE))
    eq('EVOLVING',prop('STATE')); eq(0,DESTROYS)
end

function CASES.test_reload_contexts_preserves_unit_and_reconciles_only_pending()
    join(); reload(); FIRE('OnGameTurnStarted')
    eq('GREAT_PERSON',prop('STATE')); eq(1,SPAWNS)
    grant_titles(0,DATE)
    ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,DATE)
    NEWGOV(0,DATE); reload(); FIRE('OnGameTurnStarted')
    eq('GOVERNOR',prop('STATE')); eq(1,DESTROYS)
    reload(); FIRE('OnGameTurnStarted')
    eq('CALM',prop('PERSONALITY')); eq('ASTUTE',prop('TRAIT')); eq(1,DESTROYS)
end

function CASES.test_ui_uses_native_operation_indices_and_engine_requester()
    join(); grant_titles(0,DATE)
    JAC_EvolveLeader(DATE)
    eq('JAC_LeaderPersonaEvolve',REQUESTS[1].params.OnStart)
    STEP(); eq(2,REQUESTS[1].op); eq(22,REQUESTS[1].params.governor)
    eq(0,DESTROYS)
    STEP(); eq(4,REQUESTS[1].op); eq(22,REQUESTS[1].params.governor)
    eq(45,REQUESTS[1].params.promotion)
    DRAIN(); eq('GOVERNOR',prop('STATE')); eq(2,Players[0].spent)
    eq(2,Players[0].points); eq(1,prop('EVOLUTION_SURCHARGE'))
    JAC_EvolveLeader(DATE); DRAIN()
    eq(1,DESTROYS); eq(2,Players[0].spent)
end

function CASES.test_ui_rejects_inactive_turn_and_unowned_governor()
    join(); Players[0].active=false
    raises('active turn',function() JAC_EvolveLeader(DATE) end)
    eq(0,#REQUESTS)
    Players[0].active=true
    raises('not owned',function() JAC_EvolveLeader(HOJO) end)
    raises('governor not owned',function() JAC_AssignLeader(DATE,1) end)
end

function CASES.test_official_governor_panel_hides_unjoined_candidates_and_keeps_native_buttons()
    CANDIDATES,APPOINTED={},{}
    AddGovernorCandidate=function(def,available) CANDIDATES[#CANDIDATES+1]=def.GovernorType end
    AddGovernorShared=function(def)
        return {GovernorPortrait={SetIcon=function(_,icon,size) ICON,ICON_SIZE=icon,size end},GovernorTitle=control()}
    end
    OnAppointGovernor=function(id) APPOINTED[#APPOINTED+1]=id end
    LOAD_MOD('governor_panel')
    local gov=GameInfo.Governors[22]
    AddGovernorCandidate(gov,true); OnAppointGovernor(22)
    eq(0,#CANDIDATES); eq(0,#APPOINTED)
    join(); AddGovernorCandidate(gov,true); eq(0,#CANDIDATES)
    grant_titles(0,DATE); ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,DATE)
    AddGovernorCandidate(gov,true); OnAppointGovernor(22); AddGovernorShared(gov)
    eq({'GOVERNOR_JAC_PERSONA_DATE_TANEMUNE'},CANDIDATES)
    eq({22},APPOINTED)
    eq('ICON_'..DATE,ICON); eq(256,ICON_SIZE)
end

function CASES.test_unit_panel_uses_the_leader_portrait_and_preserves_other_units()
    join()
    GetUnitPortraitIconNames=function(unit) return 'original','prefix','era','fallback' end
    LOAD_MOD('unit_panel')
    local a,b,c,d=GetUnitPortraitIconNames(NATIVE_UNITS[0][1])
    eq({'ICON_'..DATE,'ICON_'..DATE,'ICON_'..DATE,'ICON_'..DATE},{a,b,c,d})
    a,b,c,d=GetUnitPortraitIconNames(nil)
    eq({'original','prefix','era','fallback'},{a,b,c,d})
end

function CASES.test_governor_details_preserve_native_refresh_then_set_the_leader_portrait()
    REFRESHES=0; Refresh=function() REFRESHES=REFRESHES+1 end
    BASE_PROMOTES=0; ApplyPromotion=function() BASE_PROMOTES=BASE_PROMOTES+1 end
    Controls.GovernorNameLabel=control()
    Controls.GovernorNameLabel.GetText=function() return 'LOC_'..DATE..'_NAME' end
    Controls.GovernorPortrait={SetIcon=function(_,icon,size) ICON,ICON_SIZE=icon,size end}
    TREE_OPENS={}
    LuaEvents.JAC_PersonaTree_Open.Add(function(leader) TREE_OPENS[#TREE_OPENS+1]=leader end)
    LOAD_MOD('governor_details')
    Refresh()
    eq(1,REFRESHES); eq('ICON_'..DATE,ICON); eq(256,ICON_SIZE)
    ApplyPromotion()
    eq({DATE},TREE_OPENS); eq(0,BASE_PROMOTES)
    Controls.GovernorNameLabel.GetText=function() return 'OTHER GOVERNOR' end
    ApplyPromotion(); eq(1,BASE_PROMOTES)
end
