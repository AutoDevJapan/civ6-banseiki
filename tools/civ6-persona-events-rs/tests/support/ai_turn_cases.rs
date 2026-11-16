// Included by gameplay.rs: normal non-human turn callbacks and saved receipts.
const AI_TURN_FIXTURE: &str = r#"
GameInfo.JAC_PersonaTreePeople={}
GameInfo.JAC_PersonaTreeParameters={AI_PURCHASES_PER_TURN={Value=1}}
for row in GameInfo.JAC_LeaderPersonas() do
 GameInfo.JAC_PersonaTreePeople[row.LeaderType]={PreferredCategory='CONTACT',PreferredRoute='ENVOYS'}
end
for owner=0,21 do
 local cities=Players[owner]:GetCities()
 cities.Members=function() return function() return nil end end
 Players[owner].GetCities=function() return cities end
end
"#;

const AI_TURN_PAYMENTS: &str = r#"
local api=ExposedMembers.JAC_LeaderPersona
for _,case in ipairs({{owner=3,source=1,cost=2},{owner=4,source=2,cost=3}}) do
 local owner,source,cost=case.owner,case.source,case.cost
 assert(not Players[owner]:IsHuman() and Game.GetLocalPlayer()~=owner)
 local leader=PlayerConfigurations[source]:GetLeaderTypeName()
 local row=GameInfo.JAC_LeaderPersonas[leader]
 assert(row.EvolutionCost==cost)
 emit(GameEvents,'JAC_LeaderPersonaJoin',owner,{Source=source})
 assert(playerProps[owner].JAC_PERSONA_RESULT=='JOINED' and properties['JAC_PERSONA_'..leader..'_STATE']=='GREAT_PERSON')
 emit(GameEvents,'PlayerTurnStarted',owner)
 assert(properties['JAC_PERSONA_'..leader..'_STATE']=='EVOLVING','turn callback did not reserve AI cost')
 assert(api.EventsGameplayStatus(owner,leader).reserved==cost and used[owner]==0)
 local governor=GameInfo.Governors[row.GovernorType].Index
 for paid=1,cost do
  used[owner]=paid
  if paid==1 and cost==2 then appointed(owner,row); success(owner,row,0)
  elseif paid==1 then success(owner,row,0); appointed(owner,row)
  else success(owner,row,paid-1) end
  emit(GameEvents,'PlayerTurnStarted',owner)
  local s=api.EventsGameplayStatus(owner,leader)
  assert(s.paid==paid and used[owner]==paid)
  assert(s.acquired.appointed.owner==owner and s.acquired.appointed.leader==leader)
  if paid<cost then
   assert(properties['JAC_PERSONA_'..leader..'_STATE']=='EVOLVING' and #unitLists[owner]==1)
  else
   assert(properties['JAC_PERSONA_'..leader..'_STATE']=='GOVERNOR' and #unitLists[owner]==0)
  end
  local sequence=s.acquired[paid].sequence
  success(owner,row,paid-1); appointed(owner,row)
  emit(GameEvents,'PlayerTurnStarted',owner)
  local duplicate=api.EventsGameplayStatus(owner,leader)
  assert(duplicate.paid==paid and duplicate.acquired[paid].sequence==sequence and used[owner]==paid)
 end
 assert(api.EventsGameplayStatus(owner,leader).reserved==0)
end
assert(uiReads==0 and operations==0)
"#;

#[test]
fn normal_ai_turn_callbacks_reserve_and_settle_cost_two_and_three_once() {
    let lua = normal_context(AI_TURN_FIXTURE);
    run(&lua, AI_TURN_PAYMENTS);
}

#[test]
fn normal_ai_turn_requires_own_notifications_and_restores_partial_receipts() {
    for (owner, source, cost) in [(3, 1, 2), (4, 2, 3)] {
        let a = normal_context(AI_TURN_FIXTURE);
        run(
            &a,
            &format!(
                r#"
local owner,source={owner},{source}
l=PlayerConfigurations[source]:GetLeaderTypeName(); row=GameInfo.JAC_LeaderPersonas[l]
emit(GameEvents,'JAC_LeaderPersonaJoin',owner,{{Source=source}})
emit(GameEvents,'PlayerTurnStarted',owner)
-- Another owner's real title consumption and an aggregate balance without
-- a matching success notification never pay this AI person's reservation.
used[0]=1; success(0,row,0); appointed(0,row)
used[owner]=1; emit(GameEvents,'PlayerTurnStarted',owner)
assert(properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING' and #unitLists[owner]==1)
assert(next(select(2,normalStatus(owner,l)))==nil)
success(owner,row,0); appointed(owner,row)
emit(GameEvents,'PlayerTurnStarted',owner)
local s=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(owner,l)
assert(s.paid==1 and s.reserved=={cost}-1 and #unitLists[owner]==1)
savedBase=s.acquired[1].sequence
"#
            ),
        );
        let sequence: i64 = a.globals().get("savedBase").unwrap();
        let b = restore_normal(
            &a,
            &format!(
                r#"
{AI_TURN_FIXTURE}
used[0]=1; used[{owner}]=1
Game.GetCurrentGameTurn=function() return 1 end
Game.GetGreatPeople():GrantPerson({hash},7,8,0,{owner})
"#,
                hash = 200 + source
            ),
        );
        b.globals().set("savedBase", sequence).unwrap();
        run(
            &b,
            &format!(
                r#"
local owner,cost={owner},{cost}
local l=PlayerConfigurations[{source}]:GetLeaderTypeName(); local row=GameInfo.JAC_LeaderPersonas[l]
emit(GameEvents,'PlayerTurnStarted',owner)
local s=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(owner,l)
assert(s.paid==1 and s.acquired[1].sequence==savedBase)
assert(properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING' and #unitLists[owner]==1)
for paid=2,cost do
 used[owner]=paid; success(owner,row,paid-1)
 emit(GameEvents,'PlayerTurnStarted',owner)
end
s=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(owner,l)
assert(s.paid==cost and s.reserved==0 and s.acquired[1].sequence==savedBase)
assert(properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR' and #unitLists[owner]==0)
assert(used[owner]==cost and uiReads==0 and operations==0)
"#
            ),
        );
    }
}

#[test]
fn disconnected_ai_turn_callback_breaks_the_normal_payment_positive_control() {
    let source = fs::read_to_string(root().join(GAMEPLAY)).unwrap();
    let registration = "GameEvents.PlayerTurnStarted.Add(OnPlayerTurnStarted);";
    assert_eq!(source.matches(registration).count(), 1);
    let disconnected = source.replace(
        registration,
        "GameEvents.PlayerTurnStarted.Add(function() end);",
    );
    let lua = Lua::new();
    run(&lua, FIXTURE);
    run(&lua, NORMAL_FIXTURE);
    run(&lua, AI_TURN_FIXTURE);
    load(&lua, &disconnected);
    let error = lua.load(AI_TURN_PAYMENTS).exec().unwrap_err().to_string();
    assert!(
        error.contains("turn callback did not reserve AI cost"),
        "{error}"
    );
}
