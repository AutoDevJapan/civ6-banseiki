//! One executable common-driver case from an explicit real-subject request.
//! This generator performs no process calls; native identity is checked live.
use civ6_native_bridge_rs::ai_diagnostic::Command;
use serde_json::{Value, json};

pub fn scenario(mut start: Value) -> Result<Value, Box<dyn std::error::Error>> {
    // Only schema checking here; neither synthetic identity is serialized.
    let command = Command::parse(&start, 1, 1)?;
    if command.stop {
        return Err("provide a start request, not a stop receipt".into());
    }
    let s = command.subject;
    let plan = super::calibration::Plan::new("step", s.owner, s.cost as u8, s.paid as u8)?;
    let first = plan.first();
    let preflight = format!(
        r#"local text=(function()
{first}
end)()
local snapshot=assert(JAC1466_previous,'actual snapshot missing')
assert(snapshot.spent=={spent},'requested SPENT differs from actual snapshot')
local found=0
for _,row in ipairs(snapshot.records) do
 if row.leader==snapshot.leader then
  found=found+1
  assert(row.index=={index} and row.cost=={cost} and row.paid=={paid},'requested person/cost/paid differs')
  assert(row.expected[{next}]=={title},'requested next TITLE differs from actual definition')
 end
end
assert(found==1,'actual personal row missing or duplicated')
return text"#,
        spent = s.spent,
        index = s.index,
        cost = s.cost,
        paid = s.paid,
        next = s.paid + 1,
        title = s.title
    );
    start["timeout"] = 30.into();
    let mut stop = start.clone();
    stop["action"] = "stop_native_ai_observation".into();
    let scenario = json!({"name":"AI1466 one resident read-only registration; one natural turn; no payment authority",
    "judge_profile":"generic","strict_lua_errors":true,"steps":[
        {"action":"start_native_paid_runtime","name":"ai1466-readonly-one","max_seconds":300,"timeout":30},
        {"action":"ui_lua","state_name":"InGame","code":preflight,"timeout":30},
        start,
        {"action":"end_turn","turns":1,"timeout":120},
        stop,
        {"action":"stop_native_paid_runtime_observation","timeout":30}
    ]});
    civ6_autotest_rs::scenario::validate_scenario(&scenario)?;
    Ok(scenario)
}
