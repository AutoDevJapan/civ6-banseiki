//! Host sequence for one native captive-group negotiation from the T62 save.
//! The module supplies context-specific diagnostic glue, not game outcomes.
#[path = "../src/spy_group_exchange.rs"]
mod exchange;
use serde_json::{Value, json};
fn ui(code: &str) -> Value {
    json!({"action":"ui_lua","state_name":"InGame","code":code})
}
fn core(code: &str) -> Value {
    json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":code})
}
fn save() -> Value {
    json!({"action":"save","name":"JAC-Enemy-Neutralization","exact_name":true,"checkpoint":true})
}
fn wait(code: &str) -> Value {
    json!({"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"InGame","code":code}})
}
const HAS_QUOTE: &str = "local p=assert(Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62')); if not (p.quote and p.quote.acceptable==true) then return 'NO_ACCEPTABLE_QUOTE' end; ";
const EVIDENCE: &str = r#"
local function text(v,depth)
 assert(depth<20,'evidence nesting exceeds limit');
 if type(v)~='table' then return string.format('%q',tostring(v)) end
 local keys={}; for k in pairs(v) do keys[#keys+1]=k end
 table.sort(keys,function(a,b) return tostring(a)<tostring(b) end);
 local rows={}; for _,k in ipairs(keys) do rows[#rows+1]='['..text(k,depth+1)..']='..text(v[k],depth+1) end
 return '{'..table.concat(rows,',')..'}'
end
return text(JAC_GroupExchange548.Evidence(),0)
"#;
pub(crate) fn scenario() -> Value {
    json!({"name":"One native group quote, matched terms and observed return; preserve all failed missions","judge_profile":"generic","strict_lua_errors":true,"steps":[
     {"action":"load","name":"JAC-Enemy-Neutralization-ad65ffec7ebf491c8c2057b227df2304-0000000000000010","exact_name":true,"timeout":300},
     wait("return Players[0] and Players[0]:IsTurnActive() and ExposedMembers.JAC_PersonaBoundary~=nil"),
     ui(exchange::UI_SETUP),
     ui("return JAC_GroupExchange548.Prepare()"),
     core(exchange::RESERVE_CORE),
     save(),
     ui("return JAC_GroupExchange548.ArmAfterSave()"),
     ui("return JAC_GroupExchange548.OpenSession()"),
     wait("return JAC_GroupExchange548.SessionReady()"),
     ui("return JAC_GroupExchange548.EqualizeOnce()"),
     save(),
     wait("return JAC_GroupExchange548.ResponseState().stage~='WAIT_AI'"),
     ui("return JAC_GroupExchange548.PublishQuote()"),
     core(exchange::QUOTE_CORE),
     save(),
     core(&format!("{HAS_QUOTE}{}",exchange::FINAL_INTENT_CORE)),
     save(),
     ui(&format!("{HAS_QUOTE}return JAC_GroupExchange548.AcceptOnce()")),
     save(),
     wait("local p=assert(Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62')); return not (p.quote and p.quote.acceptable==true) or JAC_GroupExchange548.ReturnState().stage~='WAIT_RETURN'"),
     ui(&format!("{HAS_QUOTE}return JAC_GroupExchange548.PublishReturn()")),
     core(&format!("{HAS_QUOTE}{}",exchange::RETURN_CORE)),
     save(),
     ui(EVIDENCE),
     ui("return JAC_GroupExchange548.Close()"),
     {"action":"exit_to_menu"}
    ]})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os().nth(1).ok_or("output required")?;
    std::fs::write(output, serde_json::to_vec_pretty(&scenario())?)?;
    Ok(())
}
#[test]
fn native_sends_follow_durable_intents_and_every_transition_has_a_checkpoint() {
    let data = scenario();
    let steps = data["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 26);
    assert!(steps[0].get("checkpoint").is_none());
    assert!(steps[0]["name"].as_str().unwrap().contains("ad65ffec"));
    for index in [5, 10, 14, 16, 18, 22] {
        assert_eq!(steps[index]["action"], "save");
    }
    assert!(steps[9]["code"].as_str().unwrap().contains("EqualizeOnce"));
    assert!(steps[17]["code"].as_str().unwrap().contains("AcceptOnce"));
    assert!(steps.iter().all(|s| s["action"] != "end_turn"));
}
#[test]
fn terminal_unknowns_are_saved_without_final_submission_or_false_return_claim() {
    let data = scenario();
    let steps = data["steps"].as_array().unwrap();
    assert!(
        steps[11]["condition"]["code"]
            .as_str()
            .unwrap()
            .contains("stage~='WAIT_AI'")
    );
    for index in [15, 17, 20, 21] {
        assert!(
            steps[index]["code"]
                .as_str()
                .unwrap()
                .starts_with(HAS_QUOTE)
        );
    }
    assert!(
        steps[19]["condition"]["code"]
            .as_str()
            .unwrap()
            .contains("stage~='WAIT_RETURN'")
    );
    assert_eq!(steps[23]["code"], EVIDENCE);
    assert!(steps[24]["code"].as_str().unwrap().contains("Close()"));
}
