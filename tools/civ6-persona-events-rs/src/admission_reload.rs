//! Normal cost-one UI acquisition and two saved worlds in one resident process.
//! Bootstrap owns the lifecycle. This is an explicit diagnostic, not autostart.
use serde_json::{json, Value};

pub const LEADER: &str = "LEADER_JAC_ASHINA_MORIKIYO";
pub const CLOSED_SAVE: &str = "JAC-Normal-Admission-Closed";
pub const PAID_SAVE: &str = "JAC-Normal-Admission-Cost1";
pub const HEADER: &str = r#"
local leader='LEADER_JAC_ASHINA_MORIKIYO'
local row=assert(GameInfo.JAC_LeaderPersonas[leader]); assert(row.EvolutionCost==1)
local def=assert(GameInfo.Governors[row.GovernorType]); local p=assert(Players[0]); local gs=p:GetGovernors()
assert(Game.GetLocalPlayer()==0 and PlayerConfigurations[2]:GetLeaderTypeName()==leader)
local api=assert(ExposedMembers.JAC_LeaderPersona)
local status=api.EventsGameplayStatus(0,leader)
assert(status.source=='civ6-persona-events-rs/v3/normal-Gameplay' and status.active,'normal Gameplay unavailable')
assert(status.registrations.GovernorAppointed and status.registrations.GovernorPromoted)
assert(status.commit and status.commit.nativeClosedAll22==true and status.commit.schema==3 and status.commit.context=='Gameplay')
assert(Game:GetProperty('JAC_PERSONA_PENDING_NATIVE_HISTORY')==nil,'unknown native payment history')
for _,key in ipairs({'JAC_TEST324_EVENTS_PROBE','JAC_TEST853_ADMISSION_ACTIVE','JAC_TEST853_ADMISSION_MANUAL_APPOINT','JAC_TEST853_PAID_ASSIGNMENT_V1'}) do
 assert(Game:GetProperty(key)==nil,'private diagnostic flag present: '..key)
end
"#;
pub const MATRIX: &str = r#"
local ids=PlayerManager.GetAliveMajorIDs(); assert(type(ids)=='table' and #ids==22)
local owners={}; local out={}; local count=0
for _,id in ipairs(ids) do
 assert(not owners[id],'duplicate owner'); owners[id]=true
 local native=assert(Players[id]):GetGovernors()
 local persons={}; local personCount=0
 for person in GameInfo.JAC_LeaderPersonas() do
  local l=person.LeaderType; assert(not persons[l],'duplicate person'); persons[l]=true
  local d=assert(GameInfo.Governors[person.GovernorType])
  local expected=expectedPaid and id==0 and l==leader or false
  local allowed=native:CanEverAppointGovernor(d.Hash); local present=native:HasGovernor(d.Hash)
  assert(type(allowed)=='boolean' and allowed==expected,'native permission differs: '..id..'/'..l)
  assert(present==expected,'native object differs: '..id..'/'..l)
  local ledger={}
  for _,key in ipairs({'OWNER','SOURCE','STATE','UNIT','EVOLUTION_COST','EVOLUTION_SURCHARGE','PRICE_CONTRACT','MERIT','NAME','PORTRAIT','PERSONALITY','TRAIT','CATEGORY','GOVERN_LAST_TURN','AI_LAST_TURN','ALLIANCES_BASELINED','GOVERN_TURN_COUNT','GOVERN_TURN_COUNT_TURN','BUILDING_COUNT','BUILDING_COUNT_TURN','VICTORY_COUNT','VICTORY_COUNT_TURN','ALLIANCE_COUNT','ALLIANCE_COUNT_TURN'}) do
   ledger[key]=Game:GetProperty('JAC_PERSONA_'..l..'_'..key)
  end
  if expectedPaid and l==leader then assert(ledger.OWNER==0 and ledger.SOURCE==2 and ledger.EVOLUTION_COST==1 and ledger.STATE=='GOVERNOR' and ledger.UNIT==nil)
  else assert(ledger.OWNER==nil,'unexpected joined ledger: '..l) end
  local nodes,branches={},{}
  for node in GameInfo.JAC_PersonaTreeNodes() do if node.LeaderType==l then
   nodes[node.NodeType]=Game:GetProperty('JAC_PERSONA_'..l..'_NODE_'..node.NodeType)
   if node.ExclusiveGroup then local key='BRANCH_'..node.CategoryType..'_'..node.ExclusiveGroup
    branches[key]=Game:GetProperty('JAC_PERSONA_'..l..'_'..key)
   end
  end end
  out[id..'/'..l]={allowed=allowed,present=present,ledger=ledger,nodes=nodes,branches=branches}
  count=count+1; personCount=personCount+1
 end
 assert(personCount==22)
end
assert(count==484)
for _,key in ipairs({'JAC_PERSONA_PENDING_DEEDS','JAC_PERSONA_MERIT_BUCKETS_V2','JAC_PERSONA_PENDING_SEQUENCE'}) do
 out[key]=Game:GetProperty(key)
end
"#;
pub const PAID: &str = r#"
local g=assert(gs:GetGovernor(def.Hash)); local base=assert(GameInfo.GovernorPromotions[row.BasePromotionType])
assert(g:GetOwner()==0 and g:GetType()==def.Index and g:HasPromotion(base.Hash))
assert(gs:GetGovernorPointsSpent()==1 and status.paid==1,'official payment differs')
local acquired=assert(status.acquired); local app=assert(acquired.appointed); local first=assert(acquired[1])
for _,receipt in ipairs({app,first}) do
 assert(receipt.source==status.source and receipt.context=='Gameplay' and receipt.owner==0 and receipt.leader==leader)
 assert(receipt.accepted==true and receipt.governor==def.Index and receipt.governorHash==def.Hash)
end
assert(app.name=='GovernorAppointed' and first.name=='GovernorPromoted' and first.promotion==base.Index)
assert(acquired[2]==nil,'extra official payment')
local debit=assert(status.account.debits[first.spendingSerial]); local confirmation=assert(debit.confirmed)
assert(not debit.invalidated and confirmation.owner==0 and confirmation.afterSpent==1 and confirmation.expected==1)
local people=0
for _,unit in p:GetUnits():Members() do
 local gp=unit:GetGreatPerson(); local id=gp and gp:GetIndividual()
 local individual=id~=nil and id~=-1 and GameInfo.GreatPersonIndividuals[id]
 if individual and individual.GreatPersonIndividualType==row.PersonType then people=people+1 end
end
assert(people==0,'finalized Great Person still present')
out.paid={acquired=acquired,account=status.account,reservation=status.reservation,base=base.Index}
"#;
pub fn snapshot(paid: bool) -> String {
    format!("local expectedPaid={}\n{HEADER}\n{MATRIX}\n{}\n{}\nout.points=gs:GetGovernorPoints(); out.spent=gs:GetGovernorPointsSpent(); out.turn=Game.GetCurrentGameTurn()\nreturn serial(out)",
        if paid { "true" } else { "false" },
        if paid { PAID } else { "assert(gs:GetGovernorPointsSpent()==0 and status.acquired==nil and status.paid==0 and status.reserved==0,'closed save contains payment')" },
        crate::snapshot::SERIAL)
}
fn ui(state: &str, code: impl Into<String>) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code.into()})
}
fn wait(code: impl Into<String>) -> Value {
    json!({"action":"wait_for","timeout":90,"condition":{"type":"lua","state_name":"InGame","code":code.into()}})
}
/// A saved world retains its validated commit; it does not run fresh candidate
/// creation again. Do not poll native gates here: a stale true must fail the
/// first matrix sample rather than be waited away.
pub const READY: &str = r#"
if not Players[0] or not Players[0]:IsTurnActive() or UI.IsGameCoreBusy() or UI.IsProcessingMessages() then return false end
local api=ExposedMembers.JAC_LeaderPersona; if type(api)~='table' then return false end
local s=api.EventsGameplayStatus(0,'LEADER_JAC_ASHINA_MORIKIYO')
local init=s.initialization
return s.active==true and s.source=='civ6-persona-events-rs/v3/normal-Gameplay' and
 s.commit and s.commit.nativeClosedAll22==true and s.commit.schema==3 and s.commit.context=='Gameplay' and
 init and init.candidateCallOK==true and init.error==nil and
 (init.stage=='committed' or (init.stage=='fresh_preconditions' and init.candidate==false))
"#;
pub fn fresh_ready() -> String {
    format!("local function ready()\n{READY}\nend\n")
        + r#"
if not ready() then return false end
local s=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,'LEADER_JAC_ASHINA_MORIKIYO')
return s.initialization.stage=='committed' and Players[0]:GetGovernors():GetGovernorPoints()>=1
"#
}
pub fn paid_ready() -> String {
    format!("local function ready()\n{READY}\nend\n")
        + r#"
if not ready() then return false end
local s=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,'LEADER_JAC_ASHINA_MORIKIYO')
local l='LEADER_JAC_ASHINA_MORIKIYO'; local row=GameInfo.JAC_LeaderPersonas[l]
local d=row and GameInfo.Governors[row.GovernorType]; local p=Players[0]; local gs=p:GetGovernors()
if Game:GetProperty('JAC_PERSONA_'..l..'_STATE')~='GOVERNOR' or not d or not gs:HasGovernor(d.Hash) or gs:GetGovernorPointsSpent()~=1 then return false end
local a=s.acquired; if s.paid~=1 or not a or not a.appointed or not a[1] then return false end
for _,u in p:GetUnits():Members() do
 local gp=u:GetGreatPerson(); local id=gp and gp:GetIndividual()
 local person=id~=nil and id~=-1 and GameInfo.GreatPersonIndividuals[id]
 if person and person.GreatPersonIndividualType==row.PersonType then return false end
end
return true
"#
}
pub fn build(
    template: &Value,
    runtime: &str,
    runtime_sha: &str,
    bootstrap: &str,
    bootstrap_sha: &str,
) -> Result<Value, String> {
    let steps = template["steps"]
        .as_array()
        .ok_or("template steps missing")?;
    if steps.len() < 4
        || steps[..4]
            .iter()
            .map(|s| s["action"].as_str().unwrap_or(""))
            .ne(["enable_mods", "configure", "start_game", "wait_for"])
    {
        return Err("normal setup prefix changed".into());
    }
    let players = steps[1]["players"].as_array().ok_or("players missing")?;
    if steps[1]["ruleset"] != "RULESET_JAC_BANSEIKI"
        || players.len() != 22
        || players[0]["human"] != true
        || players[2]["leader"] != LEADER
        || players.iter().skip(1).any(|p| p["human"] != false)
    {
        return Err("requires human0 / source2 Ashina / 21 AI".into());
    }
    let mut result = vec![
        steps[0].clone(),
        json!({"action":"restart_game"}),
        steps[1].clone(),
        json!({"action":"start_native_bootstrap","name":"admission-reload-native","max_seconds":300,"controller_seconds":900,
        "runtime_dll":runtime,"runtime_sha256":runtime_sha,"bootstrap_dll":bootstrap,"bootstrap_sha256":bootstrap_sha,"timeout":40}),
        steps[2].clone(),
        steps[3].clone(),
    ];
    result.push(ui("GameCore_Tuner",r#"
local gs=Players[0]:GetGovernors(); assert(gs:GetGovernorPointsSpent()==0)
local civics={}; for row in GameInfo.JAC_PersonaTitleCivics() do civics[#civics+1]=row.CivicType end
table.sort(civics); local culture=Players[0]:GetCulture()
for _,kind in ipairs(civics) do
 if gs:GetGovernorPoints()>=1 then break end
 local civic=assert(GameInfo.Civics[kind])
 if not culture:HasCivic(civic.Index) then culture:SetCulturalProgress(civic.Index,culture:GetCultureCost(civic.Index)) end
end
assert(gs:GetGovernorPoints()>=1 and gs:GetGovernorPointsSpent()==0)
return 'official civic supplies one title; no title balance setter'
"#));
    result.push(wait(fresh_ready()));
    let mut closed = ui("InGame", snapshot(false));
    closed["capture_as"] = json!("ADMISSION_CLOSED");
    result.push(closed);
    result.push(json!({"action":"save","name":CLOSED_SAVE,"checkpoint":true,"exact_name":true}));
    result.push(ui(
        "JAC_LeaderPersona_View",
        "JAC_RequestLeaderJoin(2); return 'one normal Ashina Join requested'",
    ));
    result.push(wait(format!("local unit=Game:GetProperty('JAC_PERSONA_{LEADER}_UNIT'); return Game:GetProperty('JAC_PERSONA_{LEADER}_OWNER')==0 and Game:GetProperty('JAC_PERSONA_{LEADER}_STATE')=='GREAT_PERSON' and type(unit)=='number' and Players[0]:GetUnits():FindID(unit)~=nil")));
    result.push(ui(
        "JAC_LeaderPersona_View",
        format!("JAC_EvolveLeader('{LEADER}'); return 'one normal cost-one evolution requested'"),
    ));
    result.push(wait(paid_ready()));
    let mut paid = ui("InGame", snapshot(true));
    paid["capture_as"] = json!("ADMISSION_PAID");
    result.push(paid);
    result.push(json!({"action":"save","name":PAID_SAVE,"checkpoint":true,"exact_name":true}));
    for (name, paid, capture) in [
        (CLOSED_SAVE, false, "ADMISSION_CLOSED"),
        (PAID_SAVE, true, "ADMISSION_PAID"),
    ] {
        result.push(json!({"action":"exit_to_menu","timeout":60}));
        result.push(
            json!({"action":"load","name":name,"checkpoint":true,"exact_name":true,"timeout":300}),
        );
        result.push(wait(READY));
        let mut read = ui("InGame", snapshot(paid));
        read["equals_capture"] = json!(capture);
        result.push(read);
        // A second strict read never masks a stale first sample.
        result.push(wait(READY));
        let mut again = ui("InGame", snapshot(paid));
        again["equals_capture"] = json!(capture);
        result.push(again);
    }
    result.push(json!({"action":"stop_native_bootstrap","timeout":45}));
    result.push(json!({"action":"exit_to_menu","timeout":60}));
    Ok(
        json!({"name":"Normal cost-one admission: paid native permit revoked by closed-save reload, then restored by paid-save reload in one owned process",
        "product_acceptance":false,"native_boundary_audit_required":true,
        "judge_profile":"generic","strict_lua_errors":true,"steps":result}),
    )
}
