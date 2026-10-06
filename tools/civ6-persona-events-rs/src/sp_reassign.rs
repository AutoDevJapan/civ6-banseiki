//! Resume a proven SP04 physical save. Reassignment only; never replay payment.
//! Native promotions and skills are preserved, not newly purchased here.
use crate::sp_runtime::{HEADER, LEADER, SERIAL};
use serde_json::{json, Value};

pub const PLACEHOLDER: &str = "SP04_SUCCESS_SAVE_NOT_YET_SUPPLIED";
pub const OUTPUT_SAVE: &str = "JAC-SP-Reassigned-Preserved";

pub const READER: &str = r#"
local function boxed(v) return {kind=type(v),value=v} end
local function snapshot()
 local leader='LEADER_JAC_DATE_TANEMUNE'; local prefix='JAC_PERSONA_'..leader..'_'
 local row=assert(GameInfo.JAC_LeaderPersonas[leader]); local d=assert(GameInfo.Governors[row.GovernorType])
 local p=assert(Players[0]); local gs=p:GetGovernors(); local g=assert(gs:GetGovernor(d.Hash),'saved native governor missing')
 local s=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,leader)
 assert(s.active and s.source=='civ6-persona-events-rs/v3/normal-Gameplay')
 assert(s.registrations.GovernorAppointed and s.registrations.GovernorPromoted)
 for _,key in ipairs({'JAC_TEST324_EVENTS_PROBE','JAC_TEST853_ADMISSION_ACTIVE','JAC_TEST853_ADMISSION_MANUAL_APPOINT','JAC_TEST853_PAID_ASSIGNMENT_V1'}) do
  assert(Game:GetProperty(key)==nil,'private flag in saved normal scenario')
 end
 local function prop(k) return Game:GetProperty(prefix..k) end
 assert(prop('OWNER')==0 and prop('SOURCE')==1 and prop('STATE')=='GOVERNOR' and prop('UNIT')==nil)
 assert(row.EvolutionCost==2 and prop('EVOLUTION_COST')==2 and prop('EVOLUTION_SURCHARGE')==1)
 assert(g:GetOwner()==0 and g:GetType()==d.Index and gs:GetGovernorPointsSpent()==2 and s.paid==2)
 local base=assert(GameInfo.GovernorPromotions[row.BasePromotionType])
 local title=assert(GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_1'])
 assert(g:HasPromotion(base.Hash)==true and g:HasPromotion(title.Hash)==true,'saved official promotion lost')
 local a=assert(s.acquired); local seen={}
 for _,key in ipairs({'appointed',1,2}) do
  local r=assert(a[key]); local promo=key==1 and base or key==2 and title or nil
  assert(r.source==s.source and r.context=='Gameplay' and r.owner==0 and r.leader==leader and r.accepted==true)
  assert(r.governor==d.Index and r.governorHash==d.Hash and not seen[r.sequence]); seen[r.sequence]=true
  assert(r.name==(promo and 'GovernorPromoted' or 'GovernorAppointed') and r.arity==(promo and 3 or 2))
  assert(r.raw[1].type=='number' and r.raw[1].value==0 and r.raw[2].type=='number' and r.raw[2].value==d.Index)
  if promo then
   assert(r.promotion==promo.Index and r.promotionHash==promo.Hash and r.raw[3].type=='number' and r.raw[3].value==promo.Index)
   local debit=assert(s.account.debits[r.spendingSerial]); local confirm=assert(debit.confirmed)
   assert(debit.owner==0 and debit.governor==d.Index and debit.promotion==promo.Index and debit.binding==r.binding and not debit.invalidated)
   assert(confirm.owner==0 and confirm.binding==r.binding and confirm.afterSpent==confirm.expected and confirm.afterSpent<=2)
  end
 end
 local unitType=assert(GameInfo.Units[GameInfo.GreatPersonClasses[GameInfo.GreatPersonIndividuals[row.PersonType].GreatPersonClassType].UnitType])
 for _,owner in ipairs(PlayerManager.GetAliveMajorIDs()) do for _,u in Players[owner]:GetUnits():Members() do
  if u:GetType()==unitType.Index then
   local id=u:GetGreatPerson():GetIndividual(); local actual=id~=nil and id~=-1 and GameInfo.GreatPersonIndividuals[id]
   assert(not actual or actual.GreatPersonIndividualType~=row.PersonType,'paid person GP reappeared')
  end
 end end
 local assigned=assert(g:GetAssignedCity(),'native assignment pending')
 assert(assigned:GetOwner()==0 and CityManager.GetCity(0,assigned:GetID()),'assigned city is no longer owned')
 local promotions,skills,branches={}, {}, {}
 for member in GameInfo.GovernorPromotionSets() do if member.GovernorType==row.GovernorType then
  local def=assert(GameInfo.GovernorPromotions[member.GovernorPromotion]); local owned=g:HasPromotion(def.Hash)
  assert(type(owned)=='boolean'); promotions[member.GovernorPromotion]={index=def.Index,hash=def.Hash,owned=owned}
 end end
 assert(promotions[base.GovernorPromotionType] and promotions[title.GovernorPromotionType])
 for node in GameInfo.JAC_PersonaTreeNodes() do if node.LeaderType==leader then
  skills[node.NodeType]=boxed(prop('NODE_'..node.NodeType))
  if node.ExclusiveGroup then local k='BRANCH_'..node.CategoryType..'_'..node.ExclusiveGroup; branches[k]=boxed(prop(k)) end
 end end
 assert(next(skills)~=nil,'person skill definitions missing')
 local merit=prop('MERIT'); assert(type(merit)=='number' and merit>=0 and merit%1==0,'saved merit missing')
 local stable={owner=0,source=1,index=d.Index,hash=d.Hash,state=prop('STATE'),cost=2,spent=2,paid=2,
  promotions=promotions,skills=skills,branches=branches,category=boxed(prop('CATEGORY')),
  acquired=a,account=s.account,reservation=s.reservation,sequence=s.sequence,receipts=s.receipts,commit=s.commit}
 local deeds={}
 for _,field in ipairs({'GOVERN_LAST_TURN','AI_LAST_TURN','ALLIANCES_BASELINED','GOVERN_TURN_COUNT','GOVERN_TURN_COUNT_TURN','BUILDING_COUNT','BUILDING_COUNT_TURN','VICTORY_COUNT','VICTORY_COUNT_TURN','ALLIANCE_COUNT','ALLIANCE_COUNT_TURN'}) do deeds[field]=boxed(prop(field)) end
 return {stable=stable,city={owner=assigned:GetOwner(),id=assigned:GetID()},turn=Game.GetCurrentGameTurn(),
  established=g:IsEstablished(),onSite=g:GetTurnsOnSite(),neutralized=g:GetNeutralizedTurns(),
  points=gs:GetGovernorPoints(),merit=merit,deeds=deeds,
  pending=boxed(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')),buckets=boxed(Game:GetProperty('JAC_PERSONA_MERIT_BUCKETS_V2')),
  pendingSequence=boxed(Game:GetProperty('JAC_PERSONA_PENDING_SEQUENCE'))}
end
local function destination(current)
 local ids={}; for _,city in Players[0]:GetCities():Members() do
  assert(city:GetOwner()==0); if city:GetID()~=current then ids[#ids+1]=city:GetID() end
 end
 table.sort(ids); return assert(ids[1],'a second currently owned city is required')
end
"#;

fn ui(code: impl Into<String>) -> Value {
    json!({"action":"ui_lua","state_name":"InGame","code":code.into()})
}
fn wait(code: impl Into<String>) -> Value {
    json!({"action":"wait_for","timeout":90,"condition":{"type":"lua","state_name":"InGame","code":code.into()}})
}
fn name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 80
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

/// `input` is a physical save basename, never the mutable SP04 checkpoint alias.
/// None emits an explicitly non-executable draft with a first-step failure.
pub fn build(template: &Value, input: Option<&str>, turns: u32) -> Result<Value, String> {
    if !(1..=10).contains(&turns) {
        return Err("settlement window must be 1..10 real turns".into());
    }
    let save = input.unwrap_or(PLACEHOLDER);
    if !name_ok(save) || input == Some(PLACEHOLDER) || save == OUTPUT_SAVE {
        return Err("invalid or unresolved physical input save".into());
    }
    if template["steps"][0]["action"] != "enable_mods" {
        return Err("normal mod setup required".into());
    }
    let mut steps = vec![];
    if input.is_none() {
        steps.push(json!({"action":"ui_lua","state_name":"MainMenu","code":"error('DRAFT: successful SP04 physical save has not been supplied; no operation may run')"}));
    }
    steps.extend([template["steps"][0].clone(),json!({"action":"restart_game"}),
        json!({"action":"load","name":save,"exact_name":true,"timeout":300}),
        wait("return Players[0] and Players[0]:IsTurnActive() and type(ExposedMembers.JAC_LeaderPersona)=='table'")]);
    let mut baseline = ui(format!(
        r#"{HEADER}
{SERIAL}
{READER}
assert(JAC_REASSIGN_AUDIT==nil,'already prepared; never replay this sidecar')
local before=snapshot(); local target=destination(before.city.id)
assert(target~=before.city.id and before.neutralized==0)
JAC_REASSIGN_AUDIT={{before=before,previous=before,target=target}}
return serial(before)
"#
    ));
    baseline["capture_as"] = "REASSIGN_INPUT".into();
    steps.push(baseline);
    steps.push(json!({"action":"start_native_paid_runtime","name":"normal-reassign-runtime","max_seconds":300,"timeout":30}));
    steps.push(ui("UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LeaderPersonaRepublish'}); return 'one canonical normal refresh; no payment request'"));
    steps.push(ui(format!(
        r#"{HEADER}
local q=assert(JAC_REASSIGN_AUDIT); local target=assert(CityManager.GetCity(0,q.target))
assert(target:GetOwner()==0 and target:GetID()~=q.before.city.id)
assert(gs:CanAssignGovernor(def.Hash,target)==true,'native destination permission not observed')
return 'explicit runtime permits the distinct owned target city'
"#
    )));
    steps.push(json!({"action":"ui_lua","state_name":"JAC_LeaderPersona_View","code":format!(r#"
assert(not JAC_REASSIGN_SENT,'reassignment must not be resent')
local leader='{LEADER}'; local row=GameInfo.JAC_LeaderPersonas[leader]
local g=assert(Players[0]:GetGovernors():GetGovernor(GameInfo.Governors[row.GovernorType].Hash))
local old=assert(g:GetAssignedCity()); assert(old:GetOwner()==0)
local ids={{}}; for _,city in Players[0]:GetCities():Members() do if city:GetOwner()==0 and city:GetID()~=old:GetID() then ids[#ids+1]=city:GetID() end end
table.sort(ids); local target=assert(ids[1],'second owned city missing')
JAC_REASSIGN_SENT=true
JAC_AssignLeader(leader,target)
return 'one void normal reassignment requested; actual city acceptance remains unverified'
"#)}));
    steps.push(wait(format!(r#"local q=assert(JAC_REASSIGN_AUDIT); local d=GameInfo.Governors[GameInfo.JAC_LeaderPersonas['{LEADER}'].GovernorType]
local g=Players[0]:GetGovernors():GetGovernor(d.Hash); local city=g and g:GetAssignedCity()
return city~=nil and city:GetOwner()==0 and city:GetID()==q.target
"#)));
    let check = format!(
        r#"{HEADER}
{SERIAL}
{READER}
local q=assert(JAC_REASSIGN_AUDIT); local current=snapshot()
assert(current.city.id==q.target and current.city.id~=q.before.city.id and current.city.owner==0)
assert(serial(current.stable)==serial(q.before.stable),'payment history, native promotions or skills changed')
assert(current.neutralized==0 and current.merit>=q.previous.merit,'neutralization or merit loss during reassignment')
"#
    );
    steps.push(ui(format!(
        "{check}\nassert(current.turn==q.before.turn); q.previous=current; return serial(current)"
    )));
    steps.push(json!({"action":"stop_native_paid_runtime_observation","timeout":30}));
    for offset in 1..=turns {
        steps.push(json!({"action":"end_turn","turns":1,"timeout":600}));
        steps.push(ui(format!("{check}\nassert(current.turn==q.before.turn+{offset} and Players[0]:IsTurnActive(),'real turn progress differs'); q.previous=current; return serial(current)")));
    }
    let mut settled=ui(format!("{check}\nassert(current.established==true,'native establishment did not finish in the bounded real-turn window'); return serial(current)"));
    settled["capture_as"] = "REASSIGN_SETTLED_SAVE".into();
    steps.push(settled);
    steps.extend([json!({"action":"save","name":OUTPUT_SAVE,"exact_name":true,"checkpoint":true}),
        json!({"action":"restart_game"}),json!({"action":"load","name":OUTPUT_SAVE,"exact_name":true,"checkpoint":true,"timeout":300}),
        wait("return Players[0] and Players[0]:IsTurnActive() and type(ExposedMembers.JAC_LeaderPersona)=='table'")]);
    let mut restored=ui(format!("{HEADER}\n{SERIAL}\n{READER}\nlocal current=snapshot(); assert(current.established==true and current.neutralized==0); return serial(current)"));
    restored["equals_capture"] = "REASSIGN_SETTLED_SAVE".into();
    steps.push(restored);
    steps.push(json!({"action":"exit_to_menu"}));
    Ok(
        json!({"name":"SP04 saved normal governor: one reassignment to a different owned city, bounded real turns, preserved promotions/skills/payment and save reload; no new promotion purchase; reload is saved-state-only without runtime",
        "input_status":if input.is_some(){"physical_save_supplied_parent_must_verify_SP04_success"}else{"placeholder_not_executable"},
        "input_save":save,"settlement_real_turns":turns,"judge_profile":"generic","strict_lua_errors":true,"steps":steps}),
    )
}
