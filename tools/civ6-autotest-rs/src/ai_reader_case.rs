//! One current-game SP case. The ordinary parent republish is initialization,
//! not a join, payment, forced balance, or inferred appointment.
use serde_json::{Value, json};
pub fn scenario() -> Value {
    json!({"name":"AI1466 actual native reader; normal ledger preserved; no payment permission",
    "session_type":"single_player","judge_profile":"generic","strict_lua_errors":true,
    "steps":[
        {"action":"ui_lua","state_name":"InGame","code":crate::runner::AI_READER_READ,"capture_as":"AI1466_READER_PREFLIGHT","timeout":15},
        {"action":"start_native_paid_runtime","name":"ai1466-reader","max_seconds":300,"timeout":30},
        {"action":"ui_lua","state_name":"InGame","timeout":15,
         "code":"assert(Game.GetLocalPlayer()==0 and Players[0]:IsTurnActive(),'human turn required'); UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LeaderPersonaRepublish'}); return 'one existing normal ledger refresh; no join or payment requested'"},
        {"action":"wait_for","condition":{"type":"lua","state_name":"InGame","code":"return not UI.IsGameCoreBusy() and Players[0]:IsTurnActive()"},"timeout":30},
        {"action":"start_native_ai_reader","timeout":60},
        {"action":"end_turn","turns":1,"timeout":120},
        {"action":"stop_native_ai_reader","timeout":30},
        {"action":"stop_native_paid_runtime_observation","timeout":30}
    ]})
}
