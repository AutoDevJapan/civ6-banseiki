//! One explicit fresh production attempt after run04 lost its unsaved state.
//! No mission replay and no recovery of the lost engine state is claimed.
use serde_json::{json, Value};

fn ui(code: &str) -> Value {
    json!({"action":"ui_lua","state_name":"InGame","code":code})
}
fn wait(code: &str) -> Value {
    json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":code}})
}
fn save() -> Value {
    json!({"action":"save","name":"JAC-Enemy-Neutralization","exact_name":true,"checkpoint":true})
}
fn scenario() -> Value {
    json!({
        "name":"Run04 unknown production retained; one distinct attempt after one real forward turn",
        "judge_profile":"generic","strict_lua_errors":true,
        "steps":[
            {"action":"load","name":"JAC-Enemy-Neutralization-118003cc3d3442aba3a2fe9f93dbaf38-0000000000000001","exact_name":true,"timeout":300},
            wait("local p=Players[0]; local b=ExposedMembers.JAC_PersonaBoundary; return p and p:IsTurnActive() and b and type(b.ThirdSpyStatus)=='function'"),
            ui("local b=ExposedMembers.JAC_PersonaBoundary; local s=b.ThirdSpyStatus(); assert(s.turn==46 and not s.bound and not s.progressed and s.mapCount==0 and s.activeCount==0 and not s.queueSpy and s.capturedCount==2 and s.capacity==3); return b.RecordEnemy('RUN04_PREBUILD_PHYSICAL_SAVE_NOT_POSTMUTATION')"),
            {"action":"end_turn","turns":1,"timeout":600},
            save(),
            ui("ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('FRESH_PRODUCTION_04'); return 'separate production attempt2 queued; previous sent result remains unknown'"),
            wait("local a=ExposedMembers.JAC_PersonaBoundary.thirdLast; if a=='ERROR_FRESH_PRODUCTION_04' then error(a) end; return a=='ACCEPTED_FRESH_PRODUCTION_04'"),
            save(),
            ui("return ExposedMembers.JAC_PersonaBoundary.ThirdSpySend('BUILD_SPY')"),
            wait("local a=ExposedMembers.JAC_PersonaBoundary.lastAction; return a=='READY_BUILD_SPY' or a=='ERROR_BUILD_SPY' or a=='REJECTED_BUILD_SPY'"),
            save(),
            ui("assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='READY_BUILD_SPY'); ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_SPY'); return 'attempt2 progress queued exactly once'"),
            wait("local p=Game:GetProperty('JAC_BOUNDARY_SPY_PRODUCTION'); return p and p.attempt==2 and p.progressed==true"),
            save(),
            wait("local b=ExposedMembers.JAC_PersonaBoundary; if b.thirdLast=='ERROR_PROGRESS_SPY' then error(b.thirdLast) end; local p=Game:GetProperty('JAC_BOUNDARY_SPY_PROGRESS'); return p and p.generation==3 and b.thirdLast=='ACCEPTED_PROGRESS_SPY'"),
            save(),
            ui("local b=ExposedMembers.JAC_PersonaBoundary; local s=b.ThirdSpyStatus(); local u=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_PRODUCTION_UNCONFIRMED_04')); assert(u.status=='SENT_RESULT_UNCONFIRMED' and not u.actualSpyIDKnown and u.attempt==1 and s.progressed and not s.bound); return b.RecordEnemy('FRESH_ATTEMPT2_ENGINE_PRODUCTION_OBSERVED')"),
            {"action":"exit_to_menu"}
        ]
    })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os().nth(1).ok_or("output path required")?;
    let data = scenario();
    std::fs::write(output, serde_json::to_string_pretty(&data)?)?;
    Ok(())
}

#[test]
fn exact_reservation_and_mutation_saves_precede_assertions() {
    let s = scenario();
    let steps = s["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 18);
    assert!(steps[0]["name"].as_str().unwrap().contains("118003cc"));
    assert!(steps[0].get("checkpoint").is_none());
    assert_eq!(steps[3]["turns"], 1);
    for index in [4, 7, 10, 13, 15] {
        assert_eq!(steps[index]["action"], "save");
    }
    assert!(steps[11]["code"]
        .as_str()
        .unwrap()
        .contains("Queue('PROGRESS_SPY')"));
    assert!(steps[14]["condition"]["code"]
        .as_str()
        .unwrap()
        .contains("ACCEPTED_PROGRESS_SPY"));
    for step in steps {
        let text = step.to_string();
        assert!(!text.contains("SPY_MISSION") && !text.contains("RETRY_PREPARE"));
    }
}
