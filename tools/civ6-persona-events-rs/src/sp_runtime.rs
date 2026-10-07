//! Explicit SP runtime experiment. Product auto-bootstrap is not asserted.
use serde_json::{json, Value};
pub const LEADER: &str = "LEADER_JAC_DATE_TANEMUNE";
pub const SAVE: &str = "JAC-Normal-Paid-SP-Cost2";

pub const HEADER: &str = r#"
local leader='LEADER_JAC_DATE_TANEMUNE'; local row=assert(GameInfo.JAC_LeaderPersonas[leader])
local def=assert(GameInfo.Governors[row.GovernorType]); local p=assert(Players[0]); local gs=p:GetGovernors()
local api=assert(ExposedMembers.JAC_LeaderPersona)
local s=api.EventsGameplayStatus(0,leader)
assert(s.source=='civ6-persona-events-rs/v3/normal-Gameplay' and s.active,'normal Gameplay accounting unavailable')
assert(s.registrations.GovernorAppointed and s.registrations.GovernorPromoted,'normal subscriptions unavailable')
for _,key in ipairs({'JAC_TEST324_EVENTS_PROBE','JAC_TEST853_ADMISSION_ACTIVE','JAC_TEST853_ADMISSION_MANUAL_APPOINT','JAC_TEST853_PAID_ASSIGNMENT_V1'}) do
 assert(Game:GetProperty(key)==nil,'private flag present: '..key)
end
assert(Game.GetLocalPlayer()==0 and PlayerConfigurations[1]:GetLeaderTypeName()==leader and row.EvolutionCost==2)
"#;
pub const UNJOINED: &str = r#"
local owners,people=0,0
for _,owner in ipairs(PlayerManager.GetAliveMajorIDs()) do
 owners=owners+1
 for person in GameInfo.JAC_LeaderPersonas() do
  local d=assert(GameInfo.Governors[person.GovernorType])
  assert(not Players[owner]:GetGovernors():HasGovernor(d.Hash),'pre-existing unjoined governor: '..owner..'/'..person.LeaderType)
  assert(Game:GetProperty('JAC_PERSONA_'..person.LeaderType..'_OWNER')==nil,'pre-existing joined ledger')
  people=people+1
 end
end
assert(owners==22 and people==484 and gs:GetGovernorPointsSpent()==0)
return 'fresh SP: 22 owners / 484 absent persons; no pre-existing AI appointments'
"#;
pub use crate::snapshot::SERIAL;
pub const SNAPSHOT: &str = r#"
local function sample(readGate)
 local leader='LEADER_JAC_DATE_TANEMUNE'; local row=assert(GameInfo.JAC_LeaderPersonas[leader])
 local d=assert(GameInfo.Governors[row.GovernorType]); local p=assert(Players[0]); local gs=p:GetGovernors()
 local s=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,leader)
 assert(s.source=='civ6-persona-events-rs/v3/normal-Gameplay' and s.active)
 for _,key in ipairs({'JAC_TEST324_EVENTS_PROBE','JAC_TEST853_ADMISSION_ACTIVE','JAC_TEST853_ADMISSION_MANUAL_APPOINT','JAC_TEST853_PAID_ASSIGNMENT_V1'}) do
  assert(Game:GetProperty(key)==nil,'private flag appeared during observation: '..key)
 end
 local g=gs:GetGovernor(d.Hash); if not g then return nil end
 assert(g:GetOwner()==0 and g:GetType()==d.Index,'native person differs')
 local base=assert(GameInfo.GovernorPromotions[row.BasePromotionType])
 local title=assert(GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_1'])
 local b,t=g:HasPromotion(base.Hash),g:HasPromotion(title.Hash)
 assert(type(b)=='boolean' and type(t)=='boolean' and (not t or b))
 local a=s.acquired or {}; local state=Game:GetProperty('JAC_PERSONA_'..leader..'_STATE')
 local unit=Game:GetProperty('JAC_PERSONA_'..leader..'_UNIT')
 local function receipt(r,name,promotion)
  assert(r and r.source==s.source and r.context=='Gameplay' and r.owner==0 and r.leader==leader)
  assert(r.name==name and r.governor==d.Index and r.governorHash==d.Hash and r.accepted==true)
  assert(r.raw[1].type=='number' and r.raw[1].value==0 and r.raw[2].type=='number' and r.raw[2].value==d.Index)
  if promotion then assert(r.promotion==promotion.Index and r.promotionHash==promotion.Hash and r.arity==3 and r.raw[3].type=='number' and r.raw[3].value==promotion.Index)
  else assert(r.arity==2) end
  return {sequence=r.sequence,promotion=r.promotion,spendingSerial=r.spendingSerial}
 end
 -- A native UI notification may precede publication of Gameplay callbacks.
 -- Such an interval is recorded as pending, never invented as an observation.
 if not b or not a.appointed or not a[1] then return nil end
 local app=receipt(a.appointed,'GovernorAppointed'); local first=receipt(a[1],'GovernorPromoted',base)
 local cities={}; for _,city in p:GetCities():Members() do cities[#cities+1]=city end
 table.sort(cities,function(x,y) return x:GetID()<y:GetID() end); local city=assert(cities[1])
 local allowed=nil
 if readGate~=false then allowed=gs:CanAssignGovernor(d.Hash,city); assert(type(allowed)=='boolean') end
 local spent=gs:GetGovernorPointsSpent(); local assigned=g:GetAssignedCity()
 local out={owner=0,index=d.Index,city=city:GetID(),assigned=assigned and assigned:GetID() or -1,
  spent=spent,points=gs:GetGovernorPoints(),base=base.Index,title=title.Index,hasBase=b,hasTitle=t,
  normalPaid=s.paid or -1,state=state,unit=unit or -1,allowed=allowed,sequence=s.sequence,app=app,first=first}
 if not t and state=='EVOLVING' then
  assert(spent==1 and (s.paid==nil or s.paid==1) and a[2]==nil and type(unit)=='number')
  assert(p:GetUnits():FindID(unit),'pending Great Person disappeared')
  assert(allowed==false,'unpaid native assignment was accepted')
  out.phase='partial'
 elseif t and state=='GOVERNOR' then
  assert(spent==2 and s.paid==2 and unit==nil,'normal full settlement missing')
  out.second=receipt(a[2],'GovernorPromoted',title)
  for step=1,2 do
   local r=a[step]; local debit=assert(s.account.debits[r.spendingSerial]); local confirmed=assert(debit.confirmed)
   assert(debit.owner==0 and debit.governor==d.Index and debit.promotion==r.promotion and debit.binding==r.binding)
   assert(confirmed.owner==0 and confirmed.binding==r.binding and confirmed.afterSpent==confirmed.expected and confirmed.afterSpent<=spent and not debit.invalidated)
  end
  out.phase='full'
 else return nil end
 return out
end
"#;
/// UI and Gameplay audits deliberately enumerate independent native objects.
pub const AI_GOVERNORS: &str = r#"
local owners,pairs=0,0
for _,p in ipairs(PlayerManager.GetAliveMajors()) do if not p:IsHuman() then
 owners=owners+1; local gs=p:GetGovernors()
 for row in GameInfo.JAC_LeaderPersonas() do
  assert(Game:GetProperty('JAC_PERSONA_'..row.LeaderType..'_OWNER')~=p:GetID(),'AI joined unexpectedly; unjoined regression precondition lost')
  local d=assert(GameInfo.Governors[row.GovernorType])
  assert(not gs:HasGovernor(d.Hash),'AI appointed unjoined person after actual turn: '..p:GetID()..'/'..row.LeaderType)
  pairs=pairs+1
 end
end end
assert(owners==21 and pairs==462,'AI governor matrix incomplete')
"#;
/// Check permission independently: absence of an appointed object is insufficient.
pub const AI_ADMISSION: &str = r#"
local owners,pairs=0,0
for _,p in ipairs(PlayerManager.GetAliveMajors()) do if not p:IsHuman() then
 owners=owners+1; local gs=p:GetGovernors()
 for row in GameInfo.JAC_LeaderPersonas() do
  assert(Game:GetProperty('JAC_PERSONA_'..row.LeaderType..'_OWNER')~=p:GetID(),'AI joined unexpectedly; admission precondition lost')
  local d=assert(GameInfo.Governors[row.GovernorType])
  assert(gs:CanEverAppointGovernor(d.Hash)==false,'unjoined AI appointment permission opened: '..p:GetID()..'/'..row.LeaderType)
  pairs=pairs+1
 end
end end
assert(owners==21 and pairs==462,'AI admission matrix incomplete')
"#;
pub const AI_GREAT_PEOPLE: &str = r#"
local owners,pairs=0,0
for _,p in ipairs(PlayerManager.GetAliveMajors()) do if not p:IsHuman() then
 owners=owners+1
 for row in GameInfo.JAC_LeaderPersonas() do
  assert(Game:GetProperty('JAC_PERSONA_'..row.LeaderType..'_OWNER')~=p:GetID(),'AI joined unexpectedly')
  local person=assert(GameInfo.GreatPersonIndividuals[row.PersonType])
  local class=assert(GameInfo.GreatPersonClasses[person.GreatPersonClassType])
  local kind=assert(GameInfo.Units[class.UnitType])
  for _,unit in p:GetUnits():Members() do if unit:GetType()==kind.Index then
   local gp=assert(unit:GetGreatPerson()); local id=gp:GetIndividual()
   local actual=id~=nil and id~=-1 and GameInfo.GreatPersonIndividuals[id]
   assert(not actual or actual.GreatPersonIndividualType~=row.PersonType,'AI owns unjoined corresponding GP after actual turn: '..p:GetID()..'/'..row.LeaderType)
  end end
  pairs=pairs+1
 end
end end
assert(owners==21 and pairs==462,'AI GP matrix incomplete')
return 'Gameplay: 21 AI / 462 unjoined corresponding GP identities absent after actual turn'
"#;
pub fn observer() -> String {
    format!(
        r#"{HEADER}
assert(JAC_NORMAL_SP_AUDIT==nil,'observer already exists; do not replay')
{SNAPSHOT}
local q={{seen=0}}; JAC_NORMAL_SP_AUDIT=q
q.observe=function()
 if q.error then return end
 local ok,value=pcall(sample)
 if not ok then q.error=tostring(value); print('JAC_NORMAL_SP_OBSERVER_ERROR '..q.error); return end
 q.seen=q.seen+1
 if value and value.phase=='partial' and not q.partial then q.partial=value end
 if value and value.phase=='full' and not q.full then
  if value.allowed~=true then q.error='full native assignment denied'; return end
  q.full=value
 end
end
Events.GovernorChanged.Add(q.observe); Events.GameCoreEventPublishComplete.Add(q.observe)
return 'read-only observer registered; shipped UI remains the sole payment sender'
"#
    )
}
fn ui(state: &str, code: impl Into<String>) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code.into()})
}
fn wait(code: impl Into<String>) -> Value {
    json!({"action":"wait_for","timeout":90,"condition":{"type":"lua","state_name":"InGame","code":code.into()}})
}
pub fn build(template: &Value) -> Result<Value, String> {
    let t = template["steps"]
        .as_array()
        .ok_or("template steps missing")?;
    if t.len() < 4
        || t[..4]
            .iter()
            .map(|s| s["action"].as_str().unwrap_or(""))
            .ne(["enable_mods", "configure", "start_game", "wait_for"])
    {
        return Err("normal setup prefix changed".into());
    }
    let players = t[1]["players"].as_array().ok_or("players missing")?;
    if t[1]["ruleset"] != "RULESET_JAC_BANSEIKI"
        || players.len() != 22
        || players[0]["human"] != true
        || players[1]["leader"] != LEADER
        || players.iter().skip(1).any(|p| p["human"] == true)
    {
        return Err("requires SP human0 / source1 Date / 22 players".into());
    }
    let mut steps = vec![t[0].clone(), json!({"action":"restart_game"})];
    steps.extend_from_slice(&t[1..4]);
    steps.push(ui("InGame", format!("{HEADER}\n{UNJOINED}")));
    steps.push(json!({"action":"start_native_paid_runtime","name":"normal-paid-sp-runtime","max_seconds":300,"timeout":30}));
    steps.push(ui("InGame","UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LeaderPersonaRepublish'}); return 'one canonical normal refresh requested after explicit runtime install'"));
    steps.push(ui("InGame",format!(r#"{HEADER}
local n=0; for _,owner in ipairs(PlayerManager.GetAliveMajorIDs()) do for r in GameInfo.JAC_LeaderPersonas() do
 local d=GameInfo.Governors[r.GovernorType]; assert(not Players[owner]:GetGovernors():HasGovernor(d.Hash))
 assert(Players[owner]:GetGovernors():CanEverAppointGovernor(d.Hash)==false); n=n+1
end end; assert(n==484); return '484 unjoined pairs closed; joined positive control is still required'
"#)));
    steps.push(json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":r#"
local gs=Players[0]:GetGovernors(); assert(gs:GetGovernorPointsSpent()==0)
local civics={}; for row in GameInfo.JAC_PersonaTitleCivics() do civics[#civics+1]=row.CivicType end
table.sort(civics); local culture=Players[0]:GetCulture()
for _,kind in ipairs(civics) do
 if gs:GetGovernorPoints()>=2 then break end
 local civic=assert(GameInfo.Civics[kind]); if not culture:HasCivic(civic.Index) then culture:SetCulturalProgress(civic.Index,culture:GetCultureCost(civic.Index)) end
end
assert(gs:GetGovernorPoints()>=2 and gs:GetGovernorPointsSpent()==0)
return 'fixture preparation: official civic completion awards titles; no point setter'
"#}));
    steps.push(ui("InGame", observer()));
    steps.push(ui(
        "JAC_LeaderPersona_View",
        "JAC_RequestLeaderJoin(1); return 'one normal Join requested'",
    ));
    steps.push(wait(format!("return Game:GetProperty('JAC_PERSONA_{LEADER}_OWNER')==0 and Game:GetProperty('JAC_PERSONA_{LEADER}_STATE')=='GREAT_PERSON'")));
    steps.push(ui("JAC_LeaderPersona_View",format!("JAC_EvolveLeader('{LEADER}'); return 'one normal UI evolution requested; UI alone sends APPOINT/PROMOTE/Finalize'")));
    steps.push(wait("local q=assert(JAC_NORMAL_SP_AUDIT); q.observe(); assert(q.error==nil,q.error); return q.partial~=nil and q.full~=nil"));
    let mut pair = ui(
        "InGame",
        format!(
            r#"{HEADER}
local q=assert(JAC_NORMAL_SP_AUDIT); assert(q.error==nil,q.error)
local a,b=assert(q.partial,'partial window was not observed'),assert(q.full)
assert(a.owner==b.owner and a.index==b.index and a.city==b.city and a.assigned==-1 and b.assigned==-1)
assert(a.allowed==false and b.allowed==true and a.spent==1 and b.spent==2 and a.points==b.points)
assert(a.first.sequence==b.first.sequence and a.app.sequence==b.app.sequence)
assert(b.normalPaid==2 and b.second.sequence>a.first.sequence)
local checked=0
for _,owner in ipairs(PlayerManager.GetAliveMajorIDs()) do for r in GameInfo.JAC_LeaderPersonas() do
 if owner~=0 or r.LeaderType~=leader then
  local d=GameInfo.Governors[r.GovernorType]; local other=Players[owner]:GetGovernors()
  assert(not other:HasGovernor(d.Hash) and other:CanEverAppointGovernor(d.Hash)==false,'unjoined person opened during payment')
  checked=checked+1
 end
end end
assert(checked==483)
{SERIAL}
return serial({{partial=a,full=b}})
"#
        ),
    );
    pair["capture_as"] = "NORMAL_SP_PAIR".into();
    steps.push(pair);
    steps.push(json!({"action":"stop_native_paid_runtime_observation","timeout":30}));
    steps.push(ui(
        "InGame",
        format!(
            r#"{HEADER}
{SNAPSHOT}
local v=assert(sample()); assert(v.phase=='full' and v.allowed==true)
local q=assert(JAC_NORMAL_SP_AUDIT); assert(q.error==nil,q.error)
assert(v.spent==q.full.spent and v.index==q.full.index and v.city==q.full.city)
Events.GovernorChanged.Remove(q.observe); Events.GameCoreEventPublishComplete.Remove(q.observe)
return 'full permit remains after observation stop; own observer detached'
"#
        ),
    ));
    steps.push(ui("JAC_LeaderPersona_View",format!(r#"local cities={{}}; for _,c in Players[0]:GetCities():Members() do cities[#cities+1]=c end
table.sort(cities,function(a,b) return a:GetID()<b:GetID() end)
JAC_AssignLeader('{LEADER}',assert(cities[1]):GetID()); return 'one normal assignment requested'
"#)));
    steps.push(wait(format!(r#"local r=GameInfo.JAC_LeaderPersonas['{LEADER}']; local g=Players[0]:GetGovernors():GetGovernor(GameInfo.Governors[r.GovernorType].Hash)
local city=g and g:GetAssignedCity(); local q=assert(JAC_NORMAL_SP_AUDIT)
return city and city:GetOwner()==0 and city:GetID()==q.full.city
"#)));
    steps.push(ui("GameCore_Tuner", r#"
local count=0
for _,p in ipairs(PlayerManager.GetAliveMajors()) do if not p:IsHuman() then
 local gs=p:GetGovernors()
 if gs:GetGovernorPoints()-gs:GetGovernorPointsSpent()<1 then
  local culture=p:GetCulture(); local added=false
  for row in GameInfo.JAC_PersonaTitleCivics() do
   local c=assert(GameInfo.Civics[row.CivicType])
   if not culture:HasCivic(c.Index) then culture:SetCulturalProgress(c.Index,culture:GetCultureCost(c.Index)); added=true; break end
  end
  assert(added,'AI title civic unavailable')
 end
 assert(gs:GetGovernorPoints()-gs:GetGovernorPointsSpent()>=1,'AI has no usable official title')
 count=count+1
end end
assert(count==21); return '21 AI have an official title for the actual opponent turn'
"#));
    steps.push(ui("InGame", format!(r#"{HEADER}
{AI_ADMISSION}
{SNAPSHOT}
local v=assert(sample()); assert(v.phase=='full' and v.assigned==v.city)
local q=assert(JAC_NORMAL_SP_AUDIT); q.beforeAITurn=Game.GetCurrentGameTurn(); q.beforeAI=v
assert(Players[0]:IsTurnActive()); return 'resident runtime retained; actual opponent turn starts next'
"#)));
    steps.push(json!({"action":"end_turn","turns":1,"timeout":600}));
    steps.push(ui("InGame", format!(r#"{HEADER}
local q=assert(JAC_NORMAL_SP_AUDIT)
assert(Game.GetCurrentGameTurn()>q.beforeAITurn and Players[0]:IsTurnActive(),'actual opponent turn did not finish')
{AI_ADMISSION}
{AI_GOVERNORS}
{SNAPSHOT}
local v=assert(sample()); local b=q.beforeAI
assert(v.phase=='full' and v.assigned==v.city and v.city==b.city and v.index==b.index and v.spent==b.spent and v.normalPaid==b.normalPaid)
assert(v.app.sequence==b.app.sequence and v.first.sequence==b.first.sequence and v.second.sequence==b.second.sequence,'payment was replayed during opponent turn')
return 'UI: 21 AI / 462 unjoined permissions closed and governors absent after actual turn; paid human state retained'
"#)));
    steps.push(ui("GameCore_Tuner", AI_GREAT_PEOPLE));
    let settled = format!(
        r#"{HEADER}
{SNAPSHOT}
local v=assert(sample(false)); assert(v.phase=='full' and v.assigned==v.city)
-- Saved values only. Restart removes the resident runtime; neither this
-- snapshot nor its restored comparison queries or proves the native gate.
{SERIAL}
return serial(v)
"#
    );
    let mut before = ui("InGame", &settled);
    before["capture_as"] = "NORMAL_SP_SAVED".into();
    steps.push(before);
    steps.push(json!({"action":"save","name":SAVE,"exact_name":true,"checkpoint":true}));
    // Existing runner forbids load/exit while the resident process survives.
    steps.push(json!({"action":"restart_game"}));
    steps.push(
        json!({"action":"load","name":SAVE,"exact_name":true,"checkpoint":true,"timeout":300}),
    );
    steps.push(wait("return Players[0] and Players[0]:IsTurnActive() and type(ExposedMembers.JAC_LeaderPersona)=='table'"));
    let mut after = ui("InGame", settled);
    after["equals_capture"] = "NORMAL_SP_SAVED".into();
    steps.push(after);
    steps.push(json!({"action":"exit_to_menu"}));
    Ok(
        json!({"name":"SP normal cost-two unpaid denial then paid permit: explicit resident runtime, normal UI and restart/reload",
        "judge_profile":"generic","strict_lua_errors":true,"steps":steps}),
    )
}
