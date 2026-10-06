//! Finite stages: physical saves and native turns are owned by the host.
//! SAVE is the latest successful physical checkpoint from the previous stage.
//! Each checkpoint allocates a fresh run-ID/sequence name; never overwrite or
//! automatically reload the input save after a native operation was sent.
#[path = "../src/spy_returned_preparation.rs"]
#[allow(dead_code)]
mod preparation;
use serde_json::{json, Value};
const KEY: &str = "JAC_BOUNDARY_RETURNED_PREPARATION";
const UNKNOWN_KEY: &str = preparation::UNKNOWN_PROPERTY;
const P: &str = "local P=assert(ExposedMembers.JAC_PersonaBoundary.ReturnedPreparation,'owned View preparation absent'); ";
const RESUME: &str = "if P.HasState() then return 'ALREADY_OBSERVING' end; local r=P.Resume(); assert(not string.find(r,'UNCONFIRMED',1,true),'native operation remains unconfirmed; no turns or resend'); return r";
fn lua(state: &str, code: String) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code})
}
fn ui(code: &str) -> Value {
    lua("InGame", format!("{P}{code}"))
}
fn save() -> Value {
    json!({"action":"save","name":"JAC-Enemy-Neutralization","exact_name":true,"checkpoint":true})
}
fn ui_observe(code: &str, label: &str) -> Value {
    ui(&format!(
        "return P.ObserveStep('{label}',function() {code} end)"
    ))
}
fn core_observe(code: &str, label: &str) -> Value {
    lua(
        "GameCore_Tuner",
        format!(
            "ExposedMembers.JAC_ReturnedPreparationCoreFailure=nil; local previous=ExposedMembers.JAC_ReturnedPreparation548Witness; local ok,r=pcall(function() {code} end); if not ok then ExposedMembers.JAC_ReturnedPreparationCoreFailure={{step='{label}',error=tostring(r),previousWitness=previous}}; return 'CORE_UNKNOWN' end; return r"
        ),
    )
}
fn checkpoint(steps: &mut Vec<Value>) {
    steps.push(ui("local failure=ExposedMembers.JAC_ReturnedPreparationCoreFailure; if failure then P.CaptureUnknown(failure.step,{error=failure.error},failure.previousWitness); ExposedMembers.JAC_ReturnedPreparationCoreFailure=nil end; return 'OBSERVED'"));
    steps.push(lua("GameCore_Tuner",format!("local w=ExposedMembers.JAC_ReturnedPreparation548Witness; if not w or w.phase~='UNKNOWN' then return 'NO_UNKNOWN' end; {}",preparation::UNKNOWN_CORE)));
    steps.push(save());
    steps.push(ui(&format!("local p=Game:GetProperty('{KEY}'); assert(not Game:GetProperty('{UNKNOWN_KEY}') and not(p and p.unknowns and #p.unknowns>0),'saved unresolved preparation; no further operations'); return 'RECORDED'")));
}
fn observe_travel(steps: &mut Vec<Value>) {
    steps.push(ui_observe("return P.Observe().stage", "TRAVEL_READ"));
    checkpoint(steps);
    steps.push(core_observe(&format!("local p=assert(Game:GetProperty('{KEY}')); local w=assert(ExposedMembers.JAC_ReturnedPreparation548Witness); if w.observation.stage=='IN_TRANSIT' and not p.transit then {} elseif w.observation.stage=='ARRIVED' and p.stage=='TRAVEL_INTENT' then {} else return 'TRAVEL_NOT_FINISHED' end",preparation::TRAVEL_CHECKPOINT_CORE,preparation::TRAVEL_RECEIPT_CORE),"TRAVEL_CHECKPOINT"));
    checkpoint(steps);
}
pub(crate) fn scenario(physical: &str, phase: &str) -> Result<Value, String> {
    if !physical.starts_with("JAC-Enemy-Neutralization-")
        || !physical
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("exact physical save name required, without extension".into());
    }
    let mut steps = vec![
        json!({"action":"load","name":physical,"exact_name":true,"timeout":300}),
        json!({"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"InGame","code":"return Players[0] and Players[0]:IsTurnActive() and ExposedMembers.JAC_PersonaBoundary and ExposedMembers.JAC_PersonaBoundary.ReturnedPreparation~=nil"}}),
    ];
    match phase {
        "travel-start" => {
            steps.push(ui_observe("P.Setup(); local c=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG')); return P.PrepareTravel(c.owner,c.cityID)","TRAVEL_READY"));
            checkpoint(&mut steps);
            steps.push(core_observe(
                preparation::TRAVEL_INTENT_CORE,
                "TRAVEL_INTENT",
            ));
            checkpoint(&mut steps);
            steps.push(ui_observe("return P.ArmAfterSave('TRAVEL')", "TRAVEL_ARM"));
            checkpoint(&mut steps);
            steps.push(ui_observe("return P.TravelOnce()", "TRAVEL_SEND"));
            checkpoint(&mut steps);
            observe_travel(&mut steps);
        }
        "travel-observe" | "travel-tick" => {
            steps.push(ui_observe(RESUME, "RESUME_TRAVEL"));
            checkpoint(&mut steps);
            if phase == "travel-tick" {
                steps.push(ui_observe("ExposedMembers.JAC_ReturnedHostTurnReady=false; local o=P.Observe(); ExposedMembers.JAC_ReturnedHostTurnReady=o.stage=='IN_TRANSIT' or o.stage=='WAIT_TRAVEL'; return o.stage","TRAVEL_BEFORE_TURN"));
                checkpoint(&mut steps);
                steps.push(ui("assert(ExposedMembers.JAC_ReturnedHostTurnReady==true,'arrival already observed; use travel-observe'); return 'TURN_READY'"));
                steps.push(json!({"action":"end_turn","timeout":180}));
                checkpoint(&mut steps);
            }
            observe_travel(&mut steps);
        }
        "sources-start" => {
            steps.push(ui_observe(RESUME, "RESUME_ARRIVAL"));
            checkpoint(&mut steps);
            steps.push(ui_observe("return P.ReadySources()", "SOURCES_READY"));
            checkpoint(&mut steps);
            steps.push(core_observe(
                preparation::SOURCES_INTENT_CORE,
                "SOURCES_INTENT",
            ));
            checkpoint(&mut steps);
            steps.push(ui_observe(
                "return P.ArmAfterSave('SOURCES')",
                "SOURCES_ARM",
            ));
            checkpoint(&mut steps);
            steps.push(ui_observe("return P.SourcesOnce()", "SOURCES_SEND"));
            checkpoint(&mut steps);
        }
        "sources-observe" | "sources-tick" => {
            steps.push(ui_observe(RESUME, "RESUME_SOURCES"));
            checkpoint(&mut steps);
            if phase == "sources-tick" {
                steps.push(ui_observe("ExposedMembers.JAC_ReturnedHostTurnReady=false; local completed=P.OwnedAuxiliaryEvent(0,3); ExposedMembers.JAC_ReturnedHostTurnReady=not completed; return completed and 'COMPLETED_ALREADY' or 'WAIT_SOURCES'","SOURCES_BEFORE_TURN"));
                checkpoint(&mut steps);
                steps.push(ui("assert(ExposedMembers.JAC_ReturnedHostTurnReady==true,'completion already observed; use sources-observe'); return 'TURN_READY'"));
                steps.push(json!({"action":"end_turn","timeout":180}));
                checkpoint(&mut steps);
            }
            steps.push(ui_observe(&format!("if Game:GetProperty('{KEY}').stage=='SOURCES_RECEIPT' then return 'ALREADY_RECORDED' end; if P.OwnedAuxiliaryEvent(0,3) then return P.SourceResult().stage end; return 'WAIT_SOURCES'"),"SOURCES_RESULT"));
            checkpoint(&mut steps);
            steps.push(core_observe(&format!("local w=ExposedMembers.JAC_ReturnedPreparation548Witness; if not w or w.phase~='SOURCES_RESULT' then return 'WAIT_SOURCES' end; {}",preparation::SOURCES_RECEIPT_CORE),"SOURCES_RECEIPT"));
            checkpoint(&mut steps);
        }
        _ => return Err("unknown finite stage".into()),
    }
    steps.push(ui(r#"
local p=assert(Game:GetProperty('JAC_BOUNDARY_RETURNED_PREPARATION'));
print('JAC_RETURNED_PREPARATION_STAGE '..tostring(p.stage)..' outcome='..tostring(p.outcome));
local function dump(v,depth)
 assert(depth<20,'preparation evidence nesting limit');
 if type(v)~='table' then return string.format('%q',tostring(v)) end
 local keys={}; for k in pairs(v) do keys[#keys+1]=k end
 table.sort(keys,function(a,b) return tostring(a)<tostring(b) end);
 local out={}; for _,k in ipairs(keys) do out[#out+1]='['..dump(k,depth+1)..']='..dump(v[k],depth+1) end
 return '{'..table.concat(out,',')..'}'
end
return dump(P.Evidence(),0)
"#));
    steps.push(json!({"action":"exit_to_menu"}));
    Ok(
        json!({"name":format!("Returned native spy {phase}; preserve all failed histories"),"judge_profile":"generic","strict_lua_errors":true,"steps":steps}),
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: returned-preparation-stage SAVE PHASE OUTPUT".into());
    }
    std::fs::write(
        &args[2],
        serde_json::to_vec_pretty(&scenario(&args[0], &args[1])?)?,
    )?;
    Ok(())
}
