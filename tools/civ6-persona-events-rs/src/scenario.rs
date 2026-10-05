//! One fresh cost-two person, no existing partial/spy checkpoint is opened.
use serde_json::{json, Value};
pub const LEADER: &str = "LEADER_JAC_DATE_TANEMUNE";
const SAVE: &str = "JAC-Todo324-GameplayEvents-Cost2-Proved";

fn ui(code: impl Into<String>) -> Value {
    json!({"action":"ui_lua","state_name":"InGame","code":code.into()})
}
fn wait(code: impl Into<String>) -> Value {
    json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":code.into()}})
}
fn head() -> String {
    format!(
        r#"
local leader='{LEADER}'; local row=assert(GameInfo.JAC_LeaderPersonas[leader])
local def=assert(GameInfo.Governors[row.GovernorType]); local gs=Players[0]:GetGovernors()
local api=assert(ExposedMembers.JAC_LeaderPersona)
local s=api.EventsProbeStatus(0,leader)
assert(s.source=='civ6-persona-events-rs/v1/Gameplay' and s.context=='Gameplay','wrong lexical subscription source')
assert(s.registrations.GovernorAppointed.callOK and s.registrations.GovernorPromoted.callOK,'Gameplay subscriptions failed')
assert(PlayerConfigurations[1]:GetLeaderTypeName()==leader and row.EvolutionCost==2,'wrong fresh cost-two source')
"#
    )
}
const SERIAL: &str = r#"
local function serial(v)
 if type(v)~='table' then return type(v)..':'..tostring(v) end
 local keys={}; for k in pairs(v) do keys[#keys+1]=k end
 table.sort(keys,function(a,b) return tostring(a)<tostring(b) end)
 local out={}; for _,k in ipairs(keys) do out[#out+1]=serial(k)..'='..serial(v[k]) end
 return '{'..table.concat(out,';')..'}'
end
"#;
pub fn snapshot(paid: u8) -> String {
    format!(
        r#"{}
assert(s.active and s.paid=={paid} and s.readable==true,'Gameplay positive set unavailable')
local gov=assert(gs:GetGovernor(def.Hash)); local base=assert(GameInfo.GovernorPromotions[row.BasePromotionType])
local title=assert(GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_1'])
assert(s.definition.governor==def.Index and s.definition.base==base.Index and s.definition.title==title.Index,'live Index mapping differs')
assert(gs:GetGovernorPointsSpent()=={paid} and s.spent=={paid},'official Spent delta differs')
assert(gov:HasPromotion(base.Hash) and gov:HasPromotion(title.Hash)==({paid}==2),'independent official comparison differs')
assert(s.acquired.appointed.name=='GovernorAppointed' and s.acquired.appointed.arity==2)
assert(s.acquired.base.name=='GovernorPromoted' and s.acquired.base.arity==3 and s.acquired.base.promotion==base.Index)
assert(s.settlement.afterSpent=={paid} and not s.settlement.pending,'synchronized Gameplay Spent not settled')
assert(s.settlement.appointedSequence==s.acquired.appointed.sequence and s.settlement.baseSequence==s.acquired.base.sequence,'settlement lacks actual callbacks')
assert(s.acquired.appointed.owner==0 and s.acquired.base.owner==0 and s.acquired.base.leader==leader)
if {paid}==2 then
 assert(s.acquired.title.name=='GovernorPromoted' and s.acquired.title.promotion==title.Index,'TITLE callback Index mismatch')
 assert(s.settlement.titleSequence==s.acquired.title.sequence,'settlement lacks TITLE callback')
end
{}
return serial({{acquired=s.acquired,definition=s.definition,paid=s.paid,spent=s.spent,afterSpent=s.settlement.afterSpent}})
"#,
        head(),
        SERIAL
    )
}

fn raw_snapshot(label: &str) -> Value {
    let mut step = ui(format!("{}\n{}\nreturn serial(s)", head(), SERIAL));
    step["capture_as"] = label.into();
    step
}

pub fn build(template: &Value) -> Result<Value, String> {
    let prefix = template["steps"]
        .as_array()
        .ok_or("template steps missing")?;
    if prefix.len() < 4
        || prefix[..4]
            .iter()
            .map(|s| s["action"].as_str().unwrap_or(""))
            .ne(["enable_mods", "configure", "start_game", "wait_for"])
    {
        return Err(
            "use the existing normal cost3/payment template (first four setup steps)".into(),
        );
    }
    let players = prefix[1]["players"].as_array().ok_or("players missing")?;
    if players.len() != 22 || players[1]["leader"] != LEADER || players[0]["human"] != true {
        return Err(
            "requires the current normal 22-player source-one cost-two configuration".into(),
        );
    }
    let mut steps = vec![prefix[0].clone(), json!({"action":"restart_game"})];
    steps.extend_from_slice(&prefix[1..4]);
    steps.push(ui(format!(r#"{}
assert(not s.active and s.sequence==nil,'probe was already active')
assert(Game:GetProperty('JAC_PERSONA_'..leader..'_OWNER')==nil and not gs:HasGovernor(def.Hash),'fresh unappointed person required')
return 'EVENTS324 Gameplay source flag and subscriptions present before opt-in'
"#,head())));
    // The Tuner completes real civics only. Its Events table is never inspected.
    steps.push(json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":r#"
local gs=Players[0]:GetGovernors(); assert(gs:GetGovernorPointsSpent()==0)
local civics={}; for row in GameInfo.JAC_PersonaTitleCivics() do civics[#civics+1]=row.CivicType end
table.sort(civics); local culture=Players[0]:GetCulture()
for _,kind in ipairs(civics) do
 if gs:GetGovernorPoints()>=2 then break end
 local civic=assert(GameInfo.Civics[kind]); if not culture:HasCivic(civic.Index) then culture:SetCulturalProgress(civic.Index,culture:GetCultureCost(civic.Index)) end
end
assert(gs:GetGovernorPoints()>=2 and gs:GetGovernorPointsSpent()==0)
return 'EVENTS324 official civic titles ready, zero spending'
"#}));
    steps.push(ui("UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LeaderPersonaEventsProbe',Source=1}); return 'one synchronized fresh probe opt-in requested'"));
    steps.push(wait(
        "return Players[0]:GetProperty('JAC_PERSONA_RESULT')=='EVENT_PROBE_READY'",
    ));
    steps.push(ui(format!(r#"{}
assert(s.active and s.sequence==0 and next(s.acquired)==nil and s.paid==nil)
assert(Game:GetProperty('JAC_TEST853_ADMISSION_ACTIVE')==1 and Game:GetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT')==1)
return 'EVENTS324 Gameplay subscriptions remain registered after opt-in; payment sender is manual only'
"#,head())));
    steps.push(json!({"action":"start_native_production_admission","name":"todo324-events-cost2","max_seconds":300,"timeout":30}));
    // The synchronized operation resets only the fresh closed comparison cache
    // and reuses the lexical Gameplay publisher. UI property reads cannot bind Core.
    steps.push(ui("UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LeaderPersonaEventsRepublish',Source=1}); return 'fresh synchronized Gameplay closed republish requested after mode6 attachment'"));
    steps.push(wait(format!("local r=ExposedMembers.JAC_LeaderPersona.EventsProbeStatus(0,'{LEADER}').admission; return r and r.ok==true")));
    let mut admission = ui(format!(
        r#"{}
assert(s.admission and s.admission.ok and s.admission.synchronized and s.admission.context=='Gameplay' and s.admission.schema==3)
assert(#s.admission.reads==1474 and s.admission.cells==484,'closed natural Core read order incomplete')
{}
return serial(s.admission)
"#,
        head(),
        SERIAL
    ));
    admission["capture_as"] = "TODO324_ADMISSION_CORE_READS".into();
    steps.push(admission);
    steps.push(ui("local n=0; for _,id in ipairs(PlayerManager.GetAliveMajorIDs()) do local gs=Players[id]:GetGovernors(); for row in GameInfo.JAC_LeaderPersonas() do local def=assert(GameInfo.Governors[row.GovernorType]); assert(gs:CanEverAppointGovernor(def.Hash)==false and not gs:HasGovernor(def.Hash)); n=n+1 end end; assert(n==484); return 'all 484 unjoined owner/person pairs closed'"));
    steps.push(ui("UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LeaderPersonaJoin',Source=1}); return 'one normal Join requested'"));
    steps.push(wait(format!("return Game:GetProperty('JAC_PERSONA_{LEADER}_OWNER')==0 and Game:GetProperty('JAC_PERSONA_{LEADER}_STATE')=='GREAT_PERSON'")));
    steps.push(ui(format!("UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{{OnStart='JAC_LeaderPersonaEvolve',Leader='{LEADER}'}}); return 'one normal Evolve requested'")));
    steps.push(wait(format!(
        "return Game:GetProperty('JAC_PERSONA_{LEADER}_STATE')=='EVOLVING'"
    )));
    steps.push(ui(format!(r#"{}
local n=0
for _,id in ipairs(PlayerManager.GetAliveMajorIDs()) do
 for person in GameInfo.JAC_LeaderPersonas() do
  local d=assert(GameInfo.Governors[person.GovernorType]); assert(Players[id]:GetGovernors():CanEverAppointGovernor(d.Hash)==(id==0 and person.LeaderType==leader)); n=n+1
 end
end
assert(n==484 and gs:GetGovernorPointsSpent()==0 and not gs:HasGovernor(def.Hash))
UI.RequestPlayerOperation(0,PlayerOperations.APPOINT_GOVERNOR,{{[PlayerOperations.PARAM_GOVERNOR_TYPE]=def.Index}})
return 'one official APPOINT sent; 483 other owner/person pairs closed'
"#,head())));
    steps.push(wait(format!("local row=GameInfo.JAC_LeaderPersonas['{LEADER}']; local gs=Players[0]:GetGovernors(); return gs:HasGovernor(GameInfo.Governors[row.GovernorType].Hash) and gs:GetGovernorPointsSpent()==1")));
    steps.push(raw_snapshot("TODO324_EVENTS_APPOINT_RAW"));
    steps.push(ui(snapshot(1)));
    steps.push(ui(format!(r#"{}
local title=assert(GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_1'])
assert(s.paid==1 and gs:GetGovernorPointsSpent()==1 and not gs:GetGovernor(def.Hash):HasPromotion(title.Hash))
UI.RequestPlayerOperation(0,PlayerOperations.PROMOTE_GOVERNOR,{{[PlayerOperations.PARAM_GOVERNOR_TYPE]=def.Index,[PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE]=title.Index}})
return 'one official TITLE_1 PROMOTE sent'
"#,head())));
    steps.push(wait(format!("local row=GameInfo.JAC_LeaderPersonas['{LEADER}']; local gs=Players[0]:GetGovernors(); local gov=gs:GetGovernor(GameInfo.Governors[row.GovernorType].Hash); return gov and gov:HasPromotion(GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_1'].Hash) and gs:GetGovernorPointsSpent()==2")));
    steps.push(raw_snapshot("TODO324_EVENTS_TITLE_RAW"));
    let mut capture = ui(snapshot(2));
    capture["capture_as"] = "TODO324_EVENTS_PAID".into();
    steps.push(capture);
    steps.push(ui(format!("UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{{OnStart='JAC_LeaderPersonaFinalize',Leader='{LEADER}'}}); return 'normal Finalize via Gameplay event set requested'")));
    steps.push(wait(format!("return Game:GetProperty('JAC_PERSONA_{LEADER}_STATE')=='GOVERNOR' and Game:GetProperty('JAC_PERSONA_{LEADER}_UNIT')==nil")));
    steps.push(json!({"action":"stop_native_production_admission","timeout":30}));
    steps.push(json!({"action":"save","name":SAVE,"exact_name":true,"checkpoint":true}));
    steps.push(json!({"action":"exit_to_menu"}));
    steps.push(
        json!({"action":"load","name":SAVE,"exact_name":true,"checkpoint":true,"timeout":300}),
    );
    steps.push(wait("return Players[0] and Players[0]:IsTurnActive() and not UI.IsGameCoreBusy() and not UI.IsProcessingMessages() and type(ExposedMembers.JAC_LeaderPersona)=='table'"));
    let mut restored = ui(format!(
        "{}\nassert(Game:GetProperty('JAC_PERSONA_'..leader..'_STATE')=='GOVERNOR')\n{}",
        head(),
        snapshot(2)
    ));
    restored["equals_capture"] = "TODO324_EVENTS_PAID".into();
    steps.push(restored);
    // Repeat only read-only Status/snapshot queries after load. No projection,
    // synchronized operation or payment is requested for saved-proof inspection.
    let mut stable = ui(format!("{}\nlocal first=s\n{}\nassert(s.paid==first.paid and s.sequence==first.sequence, 'repeated Status read changed saved proof')\n{}",head(),head(),snapshot(2)));
    stable["equals_capture"] = "TODO324_EVENTS_PAID".into();
    steps.push(stable);
    steps.push(ui("return 'EVENTS324 finite opt-in Gameplay callback/payment/save diagnostic only; normal rollout, AI, native placement and old partial-save recovery remain unproved'"));
    steps.push(json!({"action":"exit_to_menu"}));
    Ok(
        json!({"name":"ToDo324 fresh cost-two Gameplay receipts, official Index/Spent comparison and reload without payment replay","judge_profile":"generic","strict_lua_errors":true,"steps":steps}),
    )
}
