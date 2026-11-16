//! Behavioral updates for the archived tests whose former authority was a UI
//! read. All new evidence here comes from the ordinary engine/event fixture.
pub const SOURCE: &str = r#"
if CASES then
function CASES.test_native_receipt_ahead_of_official_spent_keeps_full_reservation()
 local api=ExposedMembers.JAC_LeaderPersona
 local appoint=JAC_AppointLeader; JAC_AppointLeader=function() return false end
 join(); Players[0].points=3
 eq('EVOLVING',api.PrepareEvolution(0,DATE))
 eq(2,JAC_PersonaTreeRules.ReservedAppointments(0))
 NEWGOV(0,DATE,false)
 eq(2,JAC_PersonaTreeRules.ReservedAppointments(0))
 Players[0].spent=1
 -- A UI read cannot turn the newly visible pool balance into a receipt.
 eq(2,JAC_PersonaTreeRules.ReservedAppointments(0)); eq(0,DESTROYS)
 api.Reconcile()
 eq(1,JAC_PersonaTreeRules.ReservedAppointments(0)); eq(1,Players[0].spent)
 eq('EVOLVING',prop('STATE')); eq(0,DESTROYS); eq(0,#REQUESTS)
 yes(appoint(DATE)); yes(STEP()); api.Reconcile()
 eq('GOVERNOR',prop('STATE')); eq(2,Players[0].spent); eq(1,DESTROYS)
 eq(0,JAC_PersonaTreeRules.ReservedAppointments(0)); eq(0,#REQUESTS)
 JAC_AppointLeader=appoint
end
function CASES.test_prior_persona_payment_cannot_mask_unpaid_current_evolution()
 local api=ExposedMembers.JAC_LeaderPersona
 local appoint=JAC_AppointLeader; JAC_AppointLeader=function() return false end
 join(); Players[0].points=4
 eq('EVOLVING',api.PrepareEvolution(0,DATE))
 NEWGOV(0,DATE); eq(2,Players[0].spent)
 eq('EVOLVED',api.CompleteEvolution(0,DATE))
 eq('JOINED',api.Join(0,2)); eq('EVOLVING',api.PrepareEvolution(0,HOJO))
 eq(2,JAC_PersonaTreeRules.ReservedAppointments(0))
 NEWGOV(0,HOJO,false)
 eq(2,JAC_PersonaTreeRules.ReservedAppointments(0))
 Players[0].spent=3
 eq(2,JAC_PersonaTreeRules.ReservedAppointments(0)); eq(1,DESTROYS)
 api.Reconcile()
 eq(1,JAC_PersonaTreeRules.ReservedAppointments(0)); eq(3,Players[0].spent)
 eq('EVOLVING',prop('STATE',HOJO)); eq(1,DESTROYS); eq(0,#REQUESTS)
 yes(appoint(HOJO)); yes(STEP()); api.Reconcile()
 eq('GOVERNOR',prop('STATE',HOJO)); eq(4,Players[0].spent); eq(2,DESTROYS)
 eq(0,JAC_PersonaTreeRules.ReservedAppointments(0)); eq(0,#REQUESTS)
 JAC_AppointLeader=appoint
end
function CASES.test_two_partial_people_with_stock_spending_keep_only_unpaid_reservations()
 local api=ExposedMembers.JAC_LeaderPersona
 Players[0].points=6
 eq('JOINED',api.Join(0,1)); eq('JOINED',api.Join(0,2))
 eq(3,api.EvolutionCost(DATE)); eq(2,api.EvolutionCost(HOJO))
 eq('EVOLVING',api.PrepareEvolution(0,DATE)); eq('EVOLVING',api.PrepareEvolution(0,HOJO))
 -- Serialize this fixture's calls without changing the production function.
 local appoint=JAC_AppointLeader; JAC_AppointLeader=function() return false end
 ENGINE_STOCK_SPEND(0)
 for _,leader in ipairs({DATE,HOJO}) do
  local governor=GameInfo.Governors[GameInfo.JAC_LeaderPersonas[leader].GovernorType]
  UI.RequestPlayerOperation(0,PlayerOperations.APPOINT_GOVERNOR,{governor=governor.Index}); yes(STEP())
 end
 api.Reconcile(); eq(3,Players[0].spent)
 eq(1,api.EventsGameplayStatus(0,DATE).paid); eq(1,api.EventsGameplayStatus(0,HOJO).paid)
 local store='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1'
 local function image(v)
  if type(v)~='table' then return v end
  local out={}; for k,x in pairs(v) do out[k]=image(x) end; return out
 end
 local function readOnly(f)
  local before=image(Game:GetProperty(store))
  local gameSet=Game.SetProperty; local playerSets={}; local gameWrites,playerWrites=0,0
  Game.SetProperty=function(...) gameWrites=gameWrites+1; return gameSet(...) end
  for owner,p in pairs(Players) do
   local setter=p.SetProperty; playerSets[owner]=setter
   p.SetProperty=function(...) playerWrites=playerWrites+1; return setter(...) end
  end
  local ok,a,b=pcall(f)
  Game.SetProperty=gameSet
  for owner,setter in pairs(playerSets) do Players[owner].SetProperty=setter end
  yes(ok); eq(0,gameWrites); eq(0,playerWrites); eq(before,Game:GetProperty(store))
  return a,b
 end
 -- These controls must fail even when the store exposes mutable table aliases.
 for _,mutate in ipairs({
  function() Game:GetProperty(store).accounts[0].expected=99 end,
  function() Game:SetProperty(store,Game:GetProperty(store)) end,
  function() Players[0]:SetProperty('JAC_PERSONA_TEST_SAME',Players[0]:GetProperty('JAC_PERSONA_TEST_SAME')) end
 }) do
  local before=image(Game:GetProperty(store))
  local accepted=pcall(function() readOnly(mutate) end)
  no(accepted); Game:SetProperty(store,before)
 end
 local third=ROWS[4].LeaderType
 eq('JOINED',api.Join(0,3))
 local function thirdDisabled(expected)
  JAC_RefreshPersonas()
  local target
  for _,view in ipairs(VIEWS) do if view.name==Game:GetProperty('JAC_PERSONA_'..third..'_NAME') then target=view end end
  yes(target~=nil); eq(expected,target.disabled)
 end
 Players[0].points=7; thirdDisabled(false)
 local total,none=readOnly(function() return api.ReservedEvolutionTitles(0) end)
 eq(3,total); eq(0,none)
 Players[0].points=6; thirdDisabled(true)
 local saved=image(Game:GetProperty(store))
 local base=saved.people[HOJO].acquired[1]
 local broken=image(Game:GetProperty(store)); broken.people[HOJO].acquired[1]=nil
 Game:SetProperty(store,broken); Players[0].points=5
 no(appoint(DATE)); eq(0,#REQUESTS); eq(3,Players[0].spent)
 thirdDisabled(true)
 Game:SetProperty(store,nil); no(appoint(DATE)); eq(0,#REQUESTS); thirdDisabled(true)
 saved.people[HOJO].acquired[1]=base; Game:SetProperty(store,saved); api.Reconcile()
 local reservations,own=readOnly(function() return api.ReservedEvolutionTitles(0,DATE) end)
 eq(3,reservations); eq(2,own)
 local foreign,foreignOwn=readOnly(function() return api.ReservedEvolutionTitles(1,DATE) end)
 eq(math.huge,foreign); eq(0,foreignOwn)
 -- One available title belongs to the other pending person; do not take it.
 Players[0].points=4; no(appoint(DATE)); eq(0,#REQUESTS); eq(3,Players[0].spent)
 Players[0].points=6
 yes(appoint(DATE)); yes(STEP()); api.Reconcile(); eq(4,Players[0].spent)
 reload(); api=ExposedMembers.JAC_LeaderPersona
 appoint=JAC_AppointLeader; JAC_AppointLeader=function() return false end
 eq(2,api.EventsGameplayStatus(0,DATE).paid); eq(1,api.EventsGameplayStatus(0,HOJO).paid)
 -- There are two unpaid titles and exactly two left, despite stock Spent.
 yes(appoint(DATE)); yes(STEP()); api.Reconcile(); eq(5,Players[0].spent)
 eq('GOVERNOR',prop('STATE'))
 yes(appoint(HOJO)); yes(STEP()); api.Reconcile(); eq(6,Players[0].spent)
 eq('GOVERNOR',prop('STATE',HOJO))
 no(appoint(DATE)); no(appoint(HOJO)); eq(0,#REQUESTS)
 eq(6,Players[0].spent); eq(2,DESTROYS); eq({},GRANTS[0])
 JAC_AppointLeader=appoint
end
function CASES.test_missing_native_reader_never_creates_or_removes_a_person()
 local api=ExposedMembers.JAC_LeaderPersona; local key='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1'
 local before=Game:GetProperty(key); Game:SetProperty(key,nil)
 eq('NATIVE_UNAVAILABLE',api.Join(0,1)); eq(0,SPAWNS)
 Game:SetProperty(key,before)
 join(); grant_titles(0,DATE); api.PrepareEvolution(0,DATE); NEWGOV(0,DATE)
 local paid=Game:GetProperty(key); Game:SetProperty(key,nil)
 -- Removing the whole store also removes the independent joining contract.
 -- The receipt guard must stop before any native read or Great Person removal.
 eq('TITLE_PAYMENT_PENDING',api.CompleteEvolution(0,DATE)); eq('EVOLVING',prop('STATE')); eq(0,DESTROYS)
 Game:SetProperty(key,paid)
 eq('EVOLVED',api.CompleteEvolution(0,DATE)); eq(2,Players[0].spent)
end
function CASES.test_missing_type_getter_is_unavailable_and_never_uses_pool_as_eligibility()
 local api=ExposedMembers.JAC_LeaderPersona; join(); grant_titles(0,DATE)
 local original=Players[0].GetGovernors
 Players[0].GetGovernors=function() local g=original(); g.CanAppointGovernor=nil; return g end
 -- Gameplay has its own successful startup and native event authority. The
 -- UI must still refuse sending when its per-type getter is unavailable.
 eq('EVOLVING',api.PrepareEvolution(0,DATE)); eq('EVOLVING',prop('STATE'))
 eq(0,Players[0].spent); eq(0,DESTROYS)
 no(JAC_AppointLeader(DATE)); eq(0,#REQUESTS)
 JAC_RefreshPersonas(); yes(VIEWS[1].disabled)
end
function CASES.test_native_candidate_object_without_appointment_does_not_remove_the_person()
 local api=ExposedMembers.JAC_LeaderPersona
 join(); grant_titles(0,DATE); api.PrepareEvolution(0,DATE)
 NATIVE_CANDIDATE(0,DATE) -- no official success notification and no debit
 eq('APPOINTMENT_PENDING',api.CompleteEvolution(0,DATE))
 eq('EVOLVING',prop('STATE')); eq(0,DESTROYS); eq(0,Players[0].spent)
end
function CASES.test_pending_surcharge_survives_reload_and_spending_without_double_charge()
 local api=ExposedMembers.JAC_LeaderPersona
 join(); Players[0].points=2; eq('EVOLVING',api.PrepareEvolution(0,DATE))
 eq(2,Players[0].points); eq(1,prop('EVOLUTION_SURCHARGE'))
 yes(JAC_AppointLeader(DATE)); STEP(); eq(1,Players[0].spent)
 ENGINE_STOCK_SPEND(0); eq(2,Players[0].spent)
 no(JAC_AppointLeader(DATE)); eq('TITLE_PAYMENT_PENDING',api.CompleteEvolution(0,DATE))
 reload(); eq('EVOLVING',ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,DATE))
 eq(2,Players[0].points); eq(0,DESTROYS)
 Players[0].points=3; JAC_AppointLeader(DATE); DRAIN()
 eq('GOVERNOR',prop('STATE')); eq(3,Players[0].points); eq(3,Players[0].spent); eq({},GRANTS[0])
end
end

if PRODUCTION_ADMISSION_CASES then
function PRODUCTION_ADMISSION_CASES.legacy_schema_claims_empty_close_range_before_republishing()
 for owner=0,21 do
  if owner>0 then Players[owner]:SetProperty(902000+owner,nil) end
  for row in GameInfo.JAC_LeaderPersonas() do
   Players[owner]:SetProperty(903000+GameInfo.Governors[row.GovernorType].Index,nil)
  end
 end
 Game:SetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1',1)
 assert(loadstring(GAMEPLAY_SOURCE))()
 assert(Game:GetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1')==3,'vacant legacy ranges must migrate')
 for owner=0,21 do
  assert(Players[owner]:GetProperty(901004)==false and Players[owner]:GetProperty(900004)==true)
 end
end
function PRODUCTION_ADMISSION_CASES.official_payments_and_core_only_final_state()
 local api=ExposedMembers.JAC_LeaderPersona; local leader='LEADER_JAC_DATE_TANEMUNE'
 FIRE('JAC_LeaderPersonaJoin',2,{Source=1}); Players[2].points=2
 FIRE('JAC_LeaderPersonaEvolve',2,{Leader=leader})
 assert(Players[2]:GetProperty(901022)==true)
 UI.RequestPlayerOperation(2,PlayerOperations.APPOINT_GOVERNOR,{governor=22}); DRAIN()
 local row=GameInfo.JAC_LeaderPersonas[leader]
 UI.RequestPlayerOperation(2,PlayerOperations.PROMOTE_GOVERNOR,
  {governor=22,promotion=GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_1'].Index}); DRAIN()
 FIRE('JAC_LeaderPersonaFinalize',2,{Leader=leader})
 assert(Players[2]:GetProperty('JAC_PERSONA_RESULT')=='EVOLVED' and Players[2].spent==2)
 assert(Players[2]:GetProperty(901022)==true)
 local key='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1'; local paid=Game:GetProperty(key)
 Game:SetProperty(key,nil); api.Reconcile()
 assert(Players[2]:GetProperty(901022)==false,'current UI/native object cannot replace missing Gameplay proof')
 Game:SetProperty(key,paid); api.Reconcile()
 assert(Players[2]:GetProperty(901022)==true and Players[2].spent==2)
end
function PRODUCTION_ADMISSION_CASES.partial_native_payment_reserves_only_each_persons_unpaid_titles()
 local api=ExposedMembers.JAC_LeaderPersona; local leader='LEADER_JAC_ODA_NOBUHIDE'
 assert(GameInfo.JAC_LeaderPersonas[leader].EvolutionCost==3)
 FIRE('JAC_LeaderPersonaJoin',1,{Source=0}); FIRE('JAC_LeaderPersonaJoin',1,{Source=2})
 Players[1].points=4
 FIRE('JAC_LeaderPersonaEvolve',1,{Leader=leader}); FIRE('JAC_LeaderPersonaEvolve',1,{Leader=ASHINA})
 assert(Players[1]:GetProperty(901021)==true and Players[1]:GetProperty(901004)==true)
 UI.RequestPlayerOperation(1,PlayerOperations.APPOINT_GOVERNOR,{governor=21}); DRAIN()
 local row=GameInfo.JAC_LeaderPersonas[leader]
 UI.RequestPlayerOperation(1,PlayerOperations.PROMOTE_GOVERNOR,
  {governor=21,promotion=GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_1'].Index}); DRAIN()
 api.Reconcile(); assert(Players[1].spent==2)
 assert(Players[1]:GetProperty(901021)==true and Players[1]:GetProperty(901004)==true)
 Players[1].spent=1; api.Reconcile(); assert(Players[1]:GetProperty(901004)==false)
 Players[1].spent=3; api.Reconcile(); assert(Players[1]:GetProperty(901004)==false)
 Players[1].spent=2; api.Reconcile(); assert(Players[1]:GetProperty(901004)==true)
 local key='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1'; local paid=Game:GetProperty(key)
 Game:SetProperty(key,nil); api.Reconcile()
 assert(Players[1]:GetProperty(901004)==false,'UI/native objects cannot supply missing Gameplay proof')
 Game:SetProperty(key,paid); api.Reconcile(); assert(Players[1]:GetProperty(901004)==true)
 -- A hole in the saved acquisition set must not reduce another reservation.
 local base=paid.people[leader].acquired[1]; paid.people[leader].acquired[1]=nil
 Game:SetProperty(key,paid); api.Reconcile(); assert(Players[1]:GetProperty(901004)==false)
 paid.people[leader].acquired[1]=base; Game:SetProperty(key,paid)
 assert(loadstring(GAMEPLAY_SOURCE))(); api.Reconcile()
 assert(Players[1]:GetProperty(901004)==true and Players[1].spent==2)
end
end
"#;
