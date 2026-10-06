//! Routing/schema fixtures, not simulated native payment acceptance.
use super::*;
use std::sync::Mutex;
struct Route(Arc<Mutex<Vec<Value>>>);
impl Backend for Route {
    fn native_ai_diagnostic(&mut self, request: &Value) -> Result<Value> {
        self.0.lock().unwrap().push(request.clone());
        Err(Error::input("transport fixture: no native response"))
    }
}
fn start() -> Value {
    json!({"action":"start_native_ai_observation","owner":2,"generation":4,
    "index":7,"cost":3,"paid":1,"spent":9,"title":8})
}

#[test]
fn native_ai_reader_case_preserves_parent_modes_and_requires_actual_read() {
    let case=crate::ai_reader_case::scenario();
    crate::scenario::validate_scenario(&case).unwrap();
    assert_eq!(case["steps"].as_array().unwrap().len(),8);
    let mut mixed=case.clone();mixed["steps"][4]["owner"]=2.into();
    assert!(crate::scenario::validate_scenario(&mixed).is_err());
    let code=super::native_ai_reader::READ;
    let lua=mlua::Lua::new();
    lua.load(r#"
Game={GetLocalPlayer=function() return 0 end,GetCurrentGameTurn=function() return 10 end,GetProperty=function() return nil end}
Players={[0]={IsTurnActive=function() return true end}}
local p={GetID=function() return 3 end,IsHuman=function() return false end,IsAlive=function() return true end,IsMajor=function() return true end,
GetGovernors=function() return {GetGovernorPointsSpent=function() return 4 end,GetGovernorPoints=function() return 6 end} end}
PlayerManager={GetAliveMajors=function() return {p} end}
local records={};for n=1,22 do records[n]={LeaderType=n==1 and 'LEADER_JAC_DATE_TANEMUNE' or 'L'..n,EvolutionCost=2,GovernorType='G',BasePromotionType='B'} end
GameInfo={Governors={G={Index=9}},GovernorPromotions={B={Index=70},B_TITLE_1={Index=71}},
JAC_LeaderPersonas=function() local i=0;return function() i=i+1;return records[i] end end}
ExposedMembers={JAC_LeaderPersona={EventsGameplayStatus=function() return {active=true,source='civ6-persona-events-rs/v3/normal-Gameplay',
registrations={GovernorAppointed=true,GovernorPromoted=true},spent=4,paid=nil} end}}
"#).exec().unwrap();
    let raw=lua.load(code).eval::<String>().unwrap();
    let value:Value=serde_json::from_str(raw.strip_prefix("AI1466_READER_CANDIDATES ").unwrap()).unwrap();
    let row=&civ6_native_bridge_rs::ai_reader::candidates(&value).unwrap()[0];
    assert_eq!(row["owner"],3);assert!(row["paid"].is_null());assert_eq!(row["spent"],4);
    lua.load("Game.GetProperty=function() return true end").exec().unwrap();
    assert!(lua.load(code).eval::<String>().is_err());
    lua.load("Game.GetProperty=function() return nil end").exec().unwrap();
    lua.load("ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus=function() return {active=false} end").exec().unwrap();
    assert!(lua.load(code).eval::<String>().is_err());
}
#[test]
fn ai_diagnostic_common_runner_routes_start_stop_and_propagates_unknown() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let dir = tempfile::tempdir().unwrap();
    let mut runner = Runner::with_backend(
        json!({"name":"route","steps":[]}),
        dir.path().into(),
        dir.path().into(),
        dir.path().into(),
        4318,
        Box::new(Route(calls.clone())),
        Arc::new(SystemClock),
    );
    assert!(runner.run_step(&start()).is_err());
    assert!(calls.lock().unwrap().is_empty());
    runner.resident_runtime_identity = Some((1, 2));
    assert!(
        runner
            .run_step(&start())
            .unwrap_err()
            .to_string()
            .contains("no native response")
    );
    let mut stop = start();
    stop["action"] = json!("stop_native_ai_observation");
    assert!(
        runner
            .run_step(&stop)
            .unwrap_err()
            .to_string()
            .contains("no native response")
    );
    let mut bad = start();
    bad["execution_mode"] = json!(1);
    assert!(runner.run_step(&bad).is_err());
    assert_eq!(*calls.lock().unwrap(), vec![start(), stop]);
}
#[test]
fn ai_diagnostic_scenario_requires_resident_pair_same_subject_and_no_repeat() {
    let open = json!({"action":"start_native_paid_runtime","name":"fixture","max_seconds":120});
    let close = json!({"action":"stop_native_paid_runtime_observation"});
    let mut stop = start();
    stop["action"] = json!("stop_native_ai_observation");
    let validate = |steps: Vec<Value>| {
        crate::scenario::validate_scenario(&json!({"name":"AI diagnostic","steps":steps}))
    };
    assert!(validate(vec![open.clone(), start(), stop.clone(), close.clone()]).is_ok());
    assert!(validate(vec![start(), stop.clone()]).is_err());
    assert!(validate(vec![open.clone(), start(), close.clone()]).is_err());
    assert!(
        validate(vec![
            open.clone(),
            start(),
            stop.clone(),
            start(),
            stop.clone(),
            close.clone()
        ])
        .is_err()
    );
    stop["generation"] = json!(5);
    assert!(validate(vec![open, start(), stop, close]).is_err());
}
