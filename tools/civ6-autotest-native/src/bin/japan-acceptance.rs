//! Retain all original acceptance checks and isolate the funded purchase window.
use serde_json::{Value, json};
use std::fs;

const CHECKPOINT_READ: &str = "local A=assert(ExposedMembers.JAC_Absorption);ExposedMembers.JAC_NATIVE_RECALL_CHECKPOINT_READ=function() return A.Rules.Copy(Game:GetProperty('JAC_ABSORPTION_LIVE_RECALL_V1')) end;return A.Canonical(ExposedMembers.JAC_NATIVE_RECALL_CHECKPOINT_READ())";
const CHECKPOINT_WAIT: &str = "local read=ExposedMembers.JAC_NATIVE_RECALL_CHECKPOINT_READ;if type(read)~='function' then return false end;local e=read();return type(e)=='table' and type(e.Owner)=='number' and type(e.Class)=='number' and type(e.EraBefore)=='number' and type(e.CityID)=='number'";
const RESERVE: &str = r#"
local e=assert(Game:GetProperty('JAC_ABSORPTION_LIVE_RECALL_V1'))
assert(not Players[e.Owner]:IsHuman(),'fixture requires the original AI owner')
local m=assert(ExposedMembers.JAC_Absorption.GetHeroesManager())
assert(m:GetHeroClaimPlayer(e.Class)==e.Owner,'recall rights changed before waiting')
for _,u in Players[e.Owner]:GetUnits():Members() do assert(not u:IsHero() or u:GetHeroClassType()~=e.Class,'hero alive before waiting') end
local r=Players[e.Owner]:GetReligion();local faith=r:GetFaithBalance()
assert(faith>0,'funded same-era checks did not precede waiting')
local whole=math.floor(faith)
Game:SetProperty('JAC_NATIVE_RECALL_FIXTURE_RESERVE',{Owner=e.Owner,Class=e.Class,Faith=whole,Fraction=faith-whole,Era=Game.GetEras():GetCurrentEra()})
r:ChangeFaithBalance(-whole)
local remaining=r:GetFaithBalance()
assert(remaining>=0 and remaining<1,'waiting fixture faith was not reserved; before='..faith..' amount='..whole..' after='..remaining)
return 'funded rejection checks complete; reserve fixture faith until the explicit purchase window'
"#;
const RESTORE: &str = r#"
local A=assert(ExposedMembers.JAC_Absorption)
local e=assert(Game:GetProperty('JAC_ABSORPTION_LIVE_RECALL_V1'))
local s=assert(Game:GetProperty('JAC_NATIVE_RECALL_FIXTURE_RESERVE'))
assert(s.Owner==e.Owner and s.Class==e.Class,'waiting fixture identity changed')
assert(Game.GetEras():GetCurrentEra()>s.Era,'global era boundary not processed')
for _,u in Players[e.Owner]:GetUnits():Members() do assert(not u:IsHero() or u:GetHeroClassType()~=e.Class,'AI recalled before the explicit purchase window') end
local r=Players[e.Owner]:GetReligion();local income=r:GetFaithBalance()
assert(income>=0,'waiting fixture balance became negative')
r:ChangeFaithBalance(s.Faith)
local expected=s.Faith+income
local power=1
while power*2<=expected do power=power*2 end
local quantum=power/8388608
local scaled=expected/quantum;local lower=math.floor(scaled);local fraction=scaled-lower
if fraction>0.5 or fraction==0.5 and lower%2==1 then lower=lower+1 end
local engineExpected=lower*quantum
assert(r:GetFaithBalance()==engineExpected,'fixture funding restore mismatch; arithmetic='..expected..' engine='..engineExpected..' actual='..r:GetFaithBalance())
Game:SetProperty('JAC_NATIVE_RECALL_FIXTURE_RESERVE',nil)
return 'global era advanced; original fixture funding plus earned income restored before recall intent'
"#;

fn prepare(source: &Value) -> Result<Value, String> {
    let original = source["steps"].as_array().ok_or("steps missing")?;
    if original.len() != 127
        || original[111]["action"] != "autoplay"
        || original[111]["turns"] != 80
    {
        return Err("original acceptance layout changed".into());
    }
    for (index, state, marker) in [
        (106, "GameCore_Tuner", "native Warlord A retired"),
        (
            110,
            "InGame",
            "same-era rejection did not report the native era restriction",
        ),
        (113, "GameCore_Tuner", "A.BeginHeroRecall"),
        (
            124,
            "GameCore_Tuner",
            "RequestVerification('japan-live-absorption')",
        ),
    ] {
        if original[index]["action"] != "ui_lua"
            || original[index]["state_name"] != state
            || !original[index]["code"]
                .as_str()
                .is_some_and(|code| code.contains(marker))
        {
            return Err(format!("original acceptance phase {} changed", index + 1));
        }
    }
    let mut steps = Vec::new();
    for (i, s) in original.iter().enumerate() {
        if i == 111 {
            steps.push(json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":RESERVE}));
        }
        if i == 113 {
            steps.push(json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":RESTORE}));
        }
        if i == 124 {
            steps.push(json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":"local A=assert(ExposedMembers.JAC_Absorption);A.RegisterUnitProperty('JAC_ABSORPTION_LIVE_BELONGINGS');return 'restore the same registered unit-property inspection after load'"}));
        }
        let mut s = s.clone();
        if i >= 108
            && s["state_name"] == "InGame"
            && let Some(code) = s["code"].as_str()
        {
            s["code"] = json!(code.replace(
                "Game:GetProperty('JAC_ABSORPTION_LIVE_RECALL_V1')",
                "ExposedMembers.JAC_NATIVE_RECALL_CHECKPOINT_READ()"
            ));
        }
        steps.push(s);
        if i == 106 {
            steps.push(
                json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":CHECKPOINT_READ}),
            );
            steps.push(json!({"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"InGame","code":CHECKPOINT_WAIT}}));
        }
    }
    let mut result = source.clone();
    result["name"] = json!(
        "JAC Absorption full acceptance: original 127 checks; explicit AI purchase window isolated from era-wait native purchases"
    );
    result["steps"] = json!(steps);
    Ok(result)
}
fn main() {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    let result = (|| -> Result<(), String> {
        if a.len() != 2 {
            return Err("usage: japan-acceptance ORIGINAL_JSON OUTPUT_JSON".into());
        }
        let source = serde_json::from_slice(&fs::read(&a[0]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let result = prepare(&source)?;
        if std::path::Path::new(&a[1]).exists() {
            return Err("output exists; refusing overwrite".into());
        }
        fs::write(
            &a[1],
            serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        println!(
            "prepared {} steps including all original 127 checks",
            result["steps"].as_array().unwrap().len()
        );
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_original_checks_remain_in_order_with_only_read_adapter_changes() {
        let source: Value = serde_json::from_str(include_str!(
            "../../../civ6-autotest/scenarios/absorption-japan.json"
        ))
        .unwrap();
        let native = prepare(&source).unwrap();
        assert_eq!(native["steps"].as_array().unwrap().len(), 132);
        let mut offset = 0;
        for i in 0..127 {
            if i == 111 || i == 113 || i == 124 {
                offset += 1;
            }
            let mut expected = source["steps"][i].clone();
            if i >= 108 && expected["state_name"] == "InGame" {
                let code = expected["code"].as_str().unwrap().replace(
                    "Game:GetProperty('JAC_ABSORPTION_LIVE_RECALL_V1')",
                    "ExposedMembers.JAC_NATIVE_RECALL_CHECKPOINT_READ()",
                );
                expected["code"] = json!(code);
            }
            assert_eq!(
                native["steps"][i + offset],
                expected,
                "original check {} changed",
                i + 1
            );
            if i == 106 {
                offset += 2;
            }
        }
    }
    #[test]
    fn added_commands_compile_and_never_delete_or_purchase_heroes() {
        let lua = mlua::Lua::new();
        for code in [CHECKPOINT_READ, CHECKPOINT_WAIT, RESERVE, RESTORE] {
            lua.load(code).into_function().unwrap();
            assert!(!code.contains("UnitManager.Kill"));
            assert!(!code.contains("RequestCommand"));
            assert!(!code.contains("SetSlotStatus"));
        }
    }
    #[test]
    fn moved_acceptance_phases_are_not_accepted() {
        let source: Value = serde_json::from_str(include_str!(
            "../../../civ6-autotest/scenarios/absorption-japan.json"
        ))
        .unwrap();
        for index in [106, 110, 113, 124] {
            for (field, value) in [
                ("code", "return 'different phase'"),
                ("state_name", "FrontEnd"),
                ("action", "wait_for"),
            ] {
                let mut changed = source.clone();
                changed["steps"][index][field] = json!(value);
                assert!(prepare(&changed).is_err(), "phase {index} field {field}");
            }
        }
    }
    #[test]
    fn fixture_funding_is_restored_with_income_and_rejects_early_recall() {
        let lua = mlua::Lua::new();
        lua.load(r#"
        bank,era,alive=2000000.625,0,false
        local properties={JAC_ABSORPTION_LIVE_RECALL_V1={Owner=2,Class=0}}
        Game={GetProperty=function(_,k) return properties[k] end,SetProperty=function(_,k,v) properties[k]=v end,GetEras=function() return {GetCurrentEra=function() return era end} end}
        local religion={GetFaithBalance=function() return bank end,ChangeFaithBalance=function(_,v) bank=bank+v end}
        local unit={IsHero=function() return alive end,GetHeroClassType=function() return 0 end}
        Players={[2]={IsHuman=function() return false end,GetReligion=function() return religion end,GetUnits=function() return {Members=function() return ipairs({unit}) end} end}}
        ExposedMembers={JAC_Absorption={GetHeroesManager=function() return {GetHeroClaimPlayer=function() return 2 end} end}}
        "#).exec().unwrap();
        lua.load(RESERVE).exec().unwrap();
        lua.load("assert(bank==0.625);bank=34.625;era=2")
            .exec()
            .unwrap();
        lua.load(RESTORE).exec().unwrap();
        lua.load("assert(bank==2000034.625);assert(Game:GetProperty('JAC_NATIVE_RECALL_FIXTURE_RESERVE')==nil);era=0").exec().unwrap();
        lua.load(RESERVE).exec().unwrap();
        lua.load("alive=true;era=2").exec().unwrap();
        assert!(
            lua.load(RESTORE)
                .exec()
                .unwrap_err()
                .to_string()
                .contains("AI recalled before")
        );
        lua.load("assert(bank==0.625)").exec().unwrap();
    }
    #[test]
    fn restoration_matches_observed_single_precision_getter() {
        let lua = mlua::Lua::new();
        lua.load(r#"
        local property={Owner=2,Class=0,Faith=1000003,Era=0}
        local current=4.046875
        local r={GetFaithBalance=function() return current end,ChangeFaithBalance=function(_,amount) assert(amount==1000003);current=1000007.0625 end}
        Game={GetProperty=function(_,k) if k=='JAC_ABSORPTION_LIVE_RECALL_V1' then return {Owner=2,Class=0} end return property end,SetProperty=function(_,_,v) property=v end,GetEras=function() return {GetCurrentEra=function() return 2 end} end}
        Players={[2]={GetReligion=function() return r end,GetUnits=function() return {Members=function() return ipairs({}) end} end}}
        ExposedMembers={JAC_Absorption={}}
        "#).exec().unwrap();
        lua.load(RESTORE).exec().unwrap();
        assert_eq!((1000007.046875f64 as f32) as f64, 1000007.0625);
    }
}
