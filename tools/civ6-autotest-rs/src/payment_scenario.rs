//! Finite, human-owner payment probes. These do not certify AI or paid placement.
//! Lua adapters are generated here; the product's Join/Evolve/Finalize handlers
//! and the engine's APPOINT/PROMOTE/ASSIGN operations retain their normal paths.
use crate::{scenario::validate_scenario, Error, Result};
use serde_json::{json, Value};

pub const BASELINE: &str = "JAC_TEST1720_PAYMENT_BASELINE";

/// The diagnostic installs its observer after Gameplay's initialization. Replay
/// only existing closed properties through natural Core reads, before querying
/// CanEver. Unchanged values are not republished by the normal reconciliation.
/// This never writes a property or grants a permission.
pub const READ_CLOSED_ADMISSION: &str = r#"
assert(Game:GetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1')==3,'closed admission schema missing')
local rows,seen={},{}
for row in GameInfo.JAC_LeaderPersonas() do
 assert(type(row.LeaderType)=='string' and Game:GetProperty('JAC_PERSONA_'..row.LeaderType..'_OWNER')==nil,'bootstrap requires unjoined people')
 local def=assert(GameInfo.Governors[row.GovernorType]); local index=def.Index
 assert(type(index)=='number' and index%1==0 and index>=0 and index<256 and not seen[index],'invalid persona catalog')
 seen[index]=true; rows[#rows+1]=index
end
assert(#rows==22,'incomplete persona catalog'); table.sort(rows)
local ids=PlayerManager.GetAliveMajorIDs(); local owners,slots={},0
for position,owner in pairs(ids) do
 assert(type(position)=='number' and position%1==0 and position>=1 and position<=22,'invalid owner list')
 assert(type(owner)=='number' and owner%1==0 and owner>=0 and owner<64 and not owners[owner],'invalid owner identity')
 owners[owner]=true; slots=slots+1
end
assert(slots==22,'incomplete owner list')
local count=0
for position=1,22 do
 local owner=assert(ids[position]); local p=assert(Players[owner],'owner missing')
 assert(p:GetProperty(902000+owner)==true,'owner reset missing')
 for _,index in ipairs(rows) do
  assert(p:GetProperty(903000+index)==true,'closed admission marker missing')
  assert(p:GetProperty(901000+index)==false,'initial admission is not closed')
  assert(p:GetProperty(900000+index)==true,'persona manifest missing')
  count=count+1
 end
end
assert(count==484)
return 'PAYMENT880 observed 484 existing closed Core qualifications after observer installation'
"#;

#[derive(Clone, Copy, Debug)]
pub struct Person {
    pub leader: &'static str,
    pub source: u32,
    pub cost: u8,
}

pub fn person(cost: u8) -> Result<Person> {
    match cost {
        2 => Ok(Person {
            leader: "LEADER_JAC_DATE_TANEMUNE",
            source: 1,
            cost,
        }),
        3 => Ok(Person {
            leader: "LEADER_JAC_YAMANOUCHI_UESUGI_NORIMASA",
            source: 4,
            cost,
        }),
        _ => Err(Error::input(
            "payment probe requires the real cost-two or cost-three person",
        )),
    }
}

fn header(p: Person) -> String {
    format!(
        r#"
local leader='{leader}'
local row=assert(GameInfo.JAC_LeaderPersonas[leader])
local def=assert(GameInfo.Governors[row.GovernorType])
local player=assert(Players[0]); local gs=assert(player:GetGovernors())
local baseline=assert(Game:GetProperty('{BASELINE}'),'payment baseline missing')
assert(Game.GetLocalPlayer()==0 and player:IsHuman(),'requires human owner zero')
assert(row.EvolutionCost=={cost} and baseline.cost=={cost} and baseline.owner==0 and baseline.leader==leader and baseline.source=={source},'payment identity/cost differs')
assert(PlayerConfigurations[{source}]:GetLeaderTypeName()==leader,'source leader differs')
assert(Game:GetProperty('JAC_PERSONA_'..leader..'_OWNER')==0 and Game:GetProperty('JAC_PERSONA_'..leader..'_SOURCE')=={source},'joined owner/source differs')
assert(Game:GetProperty('JAC_PERSONA_'..leader..'_EVOLUTION_COST')=={cost} and Game:GetProperty('JAC_PERSONA_'..leader..'_EVOLUTION_SURCHARGE')=={surcharge},'reserved cost differs')
"#,
        leader = p.leader,
        cost = p.cost,
        source = p.source,
        surcharge = p.cost - 1
    )
}

/// Setup uses real civic completion; no title-balance setter or persona setter.
/// The two existing fixture flags suspend the screen's automatic operations.
pub fn prepare_code(p: Person) -> String {
    format!(
        r#"
assert(Game:GetProperty('{BASELINE}')==nil,'payment fixture already initialized')
assert(Players[0] and Players[0]:IsHuman())
local leader='{leader}'; local row=assert(GameInfo.JAC_LeaderPersonas[leader])
assert(row.EvolutionCost=={cost} and PlayerConfigurations[{source}]:GetLeaderTypeName()==leader)
assert(Game:GetProperty('JAC_PERSONA_'..leader..'_OWNER')==nil,'person already joined')
assert(Game:GetProperty('JAC_TEST853_ADMISSION_ACTIVE')==nil and Game:GetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT')==nil,'another appointment fixture is active')
Game:SetProperty('JAC_TEST853_ADMISSION_ACTIVE',1)
Game:SetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT',1)
local gs=Players[0]:GetGovernors(); local spent=gs:GetGovernorPointsSpent()
assert(spent==0,'fresh payment probe already spent titles')
local civics={{}}; for item in GameInfo.JAC_PersonaTitleCivics() do civics[#civics+1]=item.CivicType end
table.sort(civics); assert(#civics==15,'official title sources differ')
local culture=Players[0]:GetCulture()
for _,kind in ipairs(civics) do
 if gs:GetGovernorPoints()-spent>={cost} then break end
 local civic=assert(GameInfo.Civics[kind])
 if not culture:HasCivic(civic.Index) then culture:SetCulturalProgress(civic.Index,culture:GetCultureCost(civic.Index)) end
end
assert(gs:GetGovernorPointsSpent()==spent and gs:GetGovernorPoints()-spent>={cost},'real civic titles unavailable')
Game:SetProperty('{BASELINE}',{{owner=0,source={source},leader=leader,cost={cost},spent=spent,total=gs:GetGovernorPoints(),turn=Game.GetCurrentGameTurn()}})
return 'PAYMENT1720 real civic titles reserved for one person; automatic screen operations suspended'
"#,
        leader = p.leader,
        source = p.source,
        cost = p.cost
    )
}

/// Normal UI bindings measure every promotion of the selected real Governor.
/// The final marker also matches the cost-neutral verifier owned by ToDo1595.
pub fn snapshot_code(p: Person, paid: u8, final_receipt: bool) -> String {
    let mut code = header(p);
    code.push_str(&format!(r#"
assert(Game:GetProperty('JAC_PERSONA_'..leader..'_STATE')=='EVOLVING' or Game:GetProperty('JAC_PERSONA_'..leader..'_STATE')=='GOVERNOR','payment state differs')
assert(gs:GetGovernorPoints()==baseline.total and gs:GetGovernorPointsSpent()==baseline.spent+{paid},'official spending differs from this person')
local gov=assert(gs:GetGovernor(def.Hash),'appointed person missing')
assert(gov:GetType()==def.Index,'Governor identity differs')
local base=assert(GameInfo.GovernorPromotions[row.BasePromotionType])
assert(gov:HasPromotion(base.Hash)==true,'base appointment unpaid')
local titles={{}}; local actual=1
for n=1,{surcharge} do
 local promotion=assert(GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_'..n])
 titles[#titles+1]=promotion.Index
 local owned=gov:HasPromotion(promotion.Hash)
 assert(type(owned)=='boolean' and owned==(n<{paid}),'same-person title prefix differs')
 if owned then actual=actual+1 end
end
assert(actual=={paid},'same-person paid count differs')
local unitCount=0; local unitID=Game:GetProperty('JAC_PERSONA_'..leader..'_UNIT')
for _,unit in player:GetUnits():Members() do
 local gp=unit:GetGreatPerson(); local individual=gp and gp:IsGreatPerson() and GameInfo.GreatPersonIndividuals[gp:GetIndividual()]
 if individual and individual.GreatPersonIndividualType==row.PersonType then
  unitCount=unitCount+1; assert(unit:GetOwner()==0 and unit:GetID()==unitID,'Great Person identity differs')
 end
end
local stage=Game:GetProperty('JAC_PERSONA_'..leader..'_STATE')
assert(stage~='GOVERNOR' or actual==row.EvolutionCost,'person finalized before all payments')
assert((stage=='EVOLVING' and unitCount==1) or (stage=='GOVERNOR' and unitCount==0 and unitID==nil),'Great Person removed before complete payment or retained after Finalize')
"#, surcharge=p.cost-1));
    if final_receipt {
        code.push_str(r#"
assert(stage=='GOVERNOR','final receipt requires normal Finalize')
local count=0; local owned={}; local seen={}
for promotion in GameInfo.GovernorPromotions() do
 assert(type(promotion.Index)=='number' and promotion.Index%1==0 and promotion.Index>=0 and not seen[promotion.Index],'promotion definitions differ')
 seen[promotion.Index]=true; count=math.max(count,promotion.Index+1)
 if gov:HasPromotion(promotion.Hash) then owned[#owned+1]=promotion.Index end
end
table.sort(owned)
local city=assert(player:GetCities():GetCapitalCity()); local can=gs:CanAssignGovernor(def.Hash,city)
assert(type(can)=='boolean' and can,'fully paid person cannot be assigned')
return 'PERSONA_PAYMENT1595 {"owner":0,"governor":'..def.Index..',"cost":'..row.EvolutionCost..',"base":'..base.Index..
 ',"titles":['..table.concat(titles,',')..'],"count":'..count..',"owned":['..table.concat(owned,',')..
 '],"paid":'..actual..',"beforeSpent":'..baseline.spent..',"spent":'..gs:GetGovernorPointsSpent()..
 ',"city":'..city:GetID()..',"x":'..city:GetX()..',"y":'..city:GetY()..',"canAssign":true}'
"#);
    } else {
        code.push_str(&format!("return 'PAYMENT1720 cost={cost} paid={paid} same-person prefix and retained Great Person verified'", cost=p.cost));
    }
    code
}

fn ui(code: impl Into<String>) -> Value {
    json!({"action":"ui_lua","state_name":"InGame","code":code.into()})
}
fn wait(code: impl Into<String>) -> Value {
    json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":code.into()}})
}

pub fn build(template: &Value, cost: u8) -> Result<Value> {
    let p = person(cost)?;
    let prefix = template["steps"]
        .as_array()
        .filter(|s| s.len() >= 4)
        .ok_or_else(|| Error::input("normal persona setup is missing"))?;
    if prefix[..4]
        .iter()
        .map(|s| s["action"].as_str().unwrap_or(""))
        .ne(["enable_mods", "configure", "start_game", "wait_for"])
    {
        return Err(Error::input("normal persona setup prefix changed"));
    }
    let mut steps = vec![prefix[0].clone(), json!({"action":"restart_game"})];
    steps.extend_from_slice(&prefix[1..4]);
    steps.push(json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":prepare_code(p)}));
    steps.push(json!({"action":"start_native_production_admission","name":format!("normal-cost{cost}"),"max_seconds":180,"timeout":30}));
    steps.push(
        json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":READ_CLOSED_ADMISSION}),
    );
    steps.push(ui("local n=0; for _,id in ipairs(PlayerManager.GetAliveMajorIDs()) do local gs=Players[id]:GetGovernors(); for row in GameInfo.JAC_LeaderPersonas() do local def=assert(GameInfo.Governors[row.GovernorType]); assert(gs:CanEverAppointGovernor(def.Hash)==false and not gs:HasGovernor(def.Hash),'unjoined person admitted'); n=n+1 end end; assert(n==484); return 'all 484 unjoined owner/person pairs closed'"));
    steps.push(ui(format!("assert(PlayerConfigurations[{}]:GetLeaderTypeName()=='{}'); UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{{OnStart='JAC_LeaderPersonaJoin',Source={}}}); return 'one normal Join requested'",p.source,p.leader,p.source)));
    steps.push(wait(format!("return Game:GetProperty('JAC_PERSONA_{}_OWNER')==0 and Game:GetProperty('JAC_PERSONA_{}_STATE')=='GREAT_PERSON'",p.leader,p.leader)));
    steps.push(ui(format!("UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{{OnStart='JAC_LeaderPersonaEvolve',Leader='{}'}}); return 'one full reservation requested'",p.leader)));
    steps.push(wait(format!(
        "return Game:GetProperty('JAC_PERSONA_{}_STATE')=='EVOLVING'",
        p.leader
    )));
    steps.push(ui(format!(r#"{}
local n=0
for _,id in ipairs(PlayerManager.GetAliveMajorIDs()) do
 local g=Players[id]:GetGovernors()
 for row in GameInfo.JAC_LeaderPersonas() do
  local d=assert(GameInfo.Governors[row.GovernorType]); local expected=id==0 and row.LeaderType==leader
  assert(g:CanEverAppointGovernor(d.Hash)==expected,'wrong normal owner/person qualification'); n=n+1
 end
end
assert(n==484 and gs:CanAppointGovernor(def.Hash) and not gs:HasGovernor(def.Hash))
assert(gs:GetGovernorPointsSpent()==baseline.spent)
return 'normal owner-bound admission; 483 other pairs closed'
"#,header(p))));
    steps.push(ui(format!("local row=assert(GameInfo.JAC_LeaderPersonas['{}']); local def=assert(GameInfo.Governors[row.GovernorType]); assert(not Players[0]:GetGovernors():HasGovernor(def.Hash)); UI.RequestPlayerOperation(0,PlayerOperations.APPOINT_GOVERNOR,{{[PlayerOperations.PARAM_GOVERNOR_TYPE]=def.Index}}); return 'one official APPOINT requested'",p.leader)));
    steps.push(wait(format!("local row=GameInfo.JAC_LeaderPersonas['{}']; local gs=Players[0]:GetGovernors(); return gs:HasGovernor(GameInfo.Governors[row.GovernorType].Hash)",p.leader)));
    steps.push(ui(snapshot_code(p, 1, false)));
    steps.push(json!({"action":"stop_native_production_admission","timeout":30}));
    let save = format!("JAC-PR86-Payment1720-Cost{cost}-Partial");
    steps.push(json!({"action":"save","name":save,"exact_name":true,"checkpoint":true}));
    steps.push(json!({"action":"exit_to_menu"}));
    steps.push(json!({"action":"load","name":save,"exact_name":true,"timeout":300}));
    steps.push(wait("return Players[0] and Players[0]:IsTurnActive() and not UI.IsGameCoreBusy() and not UI.IsProcessingMessages()"));
    steps.push(ui(snapshot_code(p, 1, false)));
    for title in 1..cost {
        steps.push(ui(format!(r#"{}
local gov=assert(gs:GetGovernor(def.Hash)); local promotion=assert(GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_{title}'])
assert(gs:GetGovernorPointsSpent()==baseline.spent+{title} and gov:HasPromotion(promotion.Hash)==false,'promotion already paid or unexpected spending')
assert(gs:GetGovernorPoints()-gs:GetGovernorPointsSpent()>=1,'official title unavailable')
UI.RequestPlayerOperation(0,PlayerOperations.PROMOTE_GOVERNOR,{{[PlayerOperations.PARAM_GOVERNOR_TYPE]=def.Index,[PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE]=promotion.Index}})
return 'one official payment promotion {title} requested'
"#,header(p))));
        steps.push(wait(format!("local row=GameInfo.JAC_LeaderPersonas['{}']; local gs=Players[0]:GetGovernors(); local gov=gs:GetGovernor(GameInfo.Governors[row.GovernorType].Hash); return gov and gov:HasPromotion(GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_{}'].Hash)",p.leader,title)));
        steps.push(ui(snapshot_code(p, title + 1, false)));
    }
    steps.push(ui(format!("UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{{OnStart='JAC_LeaderPersonaFinalize',Leader='{}'}}); return 'one normal Finalize requested after all payments'",p.leader)));
    steps.push(wait(format!(
        "return Game:GetProperty('JAC_PERSONA_{}_STATE')=='GOVERNOR'",
        p.leader
    )));
    // A failed native observation must not discard the completed official
    // payments. This checkpoint is outside both native observation windows.
    steps.push(json!({"action":"save","name":format!("JAC-PR86-Payment1720-Cost{cost}-Paid"),"exact_name":true,"checkpoint":true}));
    steps.push(json!({"action":"start_native_assign_calibration","name":format!("payment-cost{cost}"),"max_seconds":90,"timeout":30,"owner":0,"leader":p.leader}));
    steps.push(ui(snapshot_code(p, cost, true)));
    steps.push(json!({"action":"ui_lua","state_name":"JAC_LeaderPersona_View","code":format!("local id=assert(Players[0]:GetCities():GetCapitalCity()):GetID(); JAC_AssignLeader('{}',id); return 'one fully paid official assignment requested'",p.leader)}));
    steps.push(wait(format!("local row=GameInfo.JAC_LeaderPersonas['{}']; local gov=Players[0]:GetGovernors():GetGovernor(GameInfo.Governors[row.GovernorType].Hash); local city=gov and gov:GetAssignedCity(); local capital=Players[0]:GetCities():GetCapitalCity(); return city and capital and city:GetOwner()==0 and city:GetID()==capital:GetID()",p.leader)));
    steps.push(json!({"action":"stop_native_assign_calibration","timeout":30}));
    steps.push(ui("return 'PAYMENT1720 UI payment/reload diagnostic with raw Core trace; independent payment verifier required. AI, unpaid placement refusal, MP and native neutralization remain separate conditions'"));
    steps.push(json!({"action":"exit_to_menu"}));
    let result = json!({"name":format!("Normal cost-{cost} person: official payment, partial save/reload, and independent Core observation"),"judge_profile":"generic","strict_lua_errors":true,"steps":steps});
    validate_scenario(&result)?;
    Ok(result)
}
