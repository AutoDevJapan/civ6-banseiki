//! Read-only payment-save replay. It neither sends nor replays payments.
use civ6_autotest_rs::scenario::validate_scenario;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};

const LEADER: &str = "LEADER_JAC_YAMANOUCHI_UESUGI_NORIMASA";

fn ui_check(paid: u8, allowed: bool) -> Value {
    json!({"action":"ui_lua","state_name":"InGame","code":format!(r#"
local owner=0; local leader='{LEADER}'; local row=assert(GameInfo.JAC_LeaderPersonas[leader]); local def=assert(GameInfo.Governors[row.GovernorType]); local p=assert(Players[owner]); local gs=p:GetGovernors(); local g=assert(gs:GetGovernor(def.Hash));
assert(row.EvolutionCost==3 and def.Index==21 and g:GetType()==def.Index and g:GetOwner()==owner,'saved native person differs');
local acquired=0; local missing=false; for step=0,2 do local key=row.BasePromotionType; if step>0 then key=key..'_TITLE_'..step end; local promotion=assert(GameInfo.GovernorPromotions[key]); local owned=g:HasPromotion(promotion.Hash); assert(type(owned)=='boolean'); if owned then assert(not missing,'non-prefix native payment'); acquired=acquired+1 else missing=true end end;
assert(acquired=={paid} and gs:GetGovernorPointsSpent()=={paid},'native payment changed during read-only replay');
local state=Game:GetProperty('JAC_PERSONA_'..leader..'_STATE'); assert(state==({paid}==3 and 'GOVERNOR' or 'EVOLVING'),'saved ledger state differs');
local cities={{}}; for _,city in p:GetCities():Members() do cities[#cities+1]=city end; table.sort(cities,function(a,b) return a:GetID()<b:GetID() end); local city=assert(cities[1]);
local result=gs:CanAssignGovernor(def.Hash,city); assert(type(result)=='boolean' and result=={allowed},'actual native assignment decision differs');
return 'owner0/Index21/paid'..acquired..'/assign='..tostring(result)..'/city='..city:GetID()
"#, allowed=if allowed {"true"} else {"false"})})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("expected partial|paid proof.json output.json".into());
    }
    let paid = match args[0].as_str() {
        "partial" => 1,
        "paid" => 3,
        _ => return Err("unknown replay kind".into()),
    };
    let proof: Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let saved = &proof["save_hashes"][if paid == 1 { 0 } else { 1 }];
    let path = Path::new(saved["path"].as_str().ok_or("save path missing")?);
    let expected = saved["sha256"].as_str().ok_or("save hash missing")?;
    let actual = format!("{:X}", Sha256::digest(fs::read(path)?));
    if actual != expected {
        return Err("physical preserved payment save differs".into());
    }
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("save name missing")?;
    let request = |serial| json!({"action":"ui_lua","state_name":"InGame","code":format!("UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{{OnStart='JAC_LeaderPersonaProject',ProjectionSerial={serial}}}); return 'projection requested'")});
    let receipt = |serial| json!({"action":"wait_for","timeout":30,"condition":{"type":"lua","state_name":"InGame","code":format!("return Players[0]:GetProperty('JAC_PERSONA_PROJECT_RECEIPT')=={serial}")}});
    let allowed = paid == 3;
    let data = json!({"name":format!("Resident runtime retains native cost3 {}/3 decision after observation stop",paid),
    "judge_profile":"generic","strict_lua_errors":true,"steps":[
        {"action":"load","name":name,"exact_name":true,"timeout":300},
        {"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"return Players[0] and Players[0]:IsTurnActive() and type(ExposedMembers.JAC_LeaderPersona)=='table'"}},
        ui_check(paid,true),
        {"action":"ui_lua","state_name":"GameCore_Tuner","code":"Game:SetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1',true); return 'diagnostic projection flag only'"},
        request(1),receipt(1),
        {"action":"start_native_paid_runtime","name":format!("runtime-cost3-{paid}"),"max_seconds":90},
        request(2),receipt(2),ui_check(paid,allowed),
        {"action":"stop_native_paid_runtime_observation"},
        ui_check(paid,allowed)
    ]});
    validate_scenario(&data)?;
    fs::write(
        &args[2],
        format!("{}\n", serde_json::to_string_pretty(&data)?),
    )?;
    println!("save SHA-256 {actual}; 12 steps; formal payment requests=0");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_read_rejects_nonprefix_flags_and_changed_owner_or_spending() {
        let lua = mlua::Lua::new();
        lua.load(format!(r#"
GameInfo={{JAC_LeaderPersonas={{['{LEADER}']={{GovernorType='G',BasePromotionType='B',EvolutionCost=3}}}},
Governors={{G={{Index=21,Hash=21}}}},GovernorPromotions={{B={{Hash=69}},B_TITLE_1={{Hash=84}},B_TITLE_2={{Hash=88}}}}}};
FLAGS={{[69]=true,[84]=false,[88]=false}}; SPENT=1; OWNER=0; ALLOWED=false;
local g={{GetType=function() return 21 end,GetOwner=function() return OWNER end,HasPromotion=function(_,id) return FLAGS[id] end}};
local city={{GetID=function() return 1 end}};
Players={{[0]={{GetGovernors=function() return {{GetGovernor=function() return g end,GetGovernorPointsSpent=function() return SPENT end,CanAssignGovernor=function() return ALLOWED end}} end,
GetCities=function() return {{Members=function() local once=false; return function() if not once then once=true; return 1,city end end end}} end}}}};
Game={{GetProperty=function() return SPENT==3 and 'GOVERNOR' or 'EVOLVING' end}};
"#)).exec().unwrap();
        let partial = ui_check(1, false);
        let code = partial["code"].as_str().unwrap();
        lua.load(code).exec().unwrap();
        for mutation in [
            "FLAGS[69]=false;FLAGS[84]=true",
            "OWNER=1",
            "SPENT=2",
            "FLAGS[69]=1",
        ] {
            lua.load("FLAGS={[69]=true,[84]=false,[88]=false};OWNER=0;SPENT=1")
                .exec()
                .unwrap();
            lua.load(mutation).exec().unwrap();
            assert!(lua.load(code).exec().is_err(), "{mutation}");
        }
        lua.load("FLAGS={[69]=true,[84]=true,[88]=true};OWNER=0;SPENT=3;ALLOWED=true")
            .exec()
            .unwrap();
        let paid = ui_check(3, true);
        lua.load(paid["code"].as_str().unwrap()).exec().unwrap();
    }
}
