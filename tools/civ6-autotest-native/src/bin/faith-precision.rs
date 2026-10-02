//! Prepare a short precision measurement; no full acceptance is claimed.
use serde_json::{Value, json};
use std::fs;
fn main() {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    let result = (|| -> Result<(), String> {
        if a.len() != 2 {
            return Err("usage: faith-precision ORIGINAL_JSON OUTPUT_JSON".into());
        }
        let source: Value = serde_json::from_slice(&fs::read(&a[0]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let output = json!({"name":"PR87 Rust faith reserve/restore precision measurement, not full acceptance","judge_profile":"generic","test_mode":{"skipped_turns":0,"continuous_performance_valid":false},"steps":[source["steps"][0],{"action":"load","name":"jac-absorption-after-turn-absorb-34-0675c70c359e","exact_name":true,"timeout":600},source["steps"][100],{"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":"local e=assert(Game:GetProperty('JAC_ABSORPTION_LIVE_EXPECT_V1'));local r=Players[e.To]:GetReligion();r:ChangeFaithBalance(1000000);local before=r:GetFaithBalance();local whole=math.floor(before);r:ChangeFaithBalance(-whole);local remainder=r:GetFaithBalance();Game:SetProperty('JAC_NATIVE_FAITH_PRECISION',{Owner=e.To,Before=before,Whole=whole,Remainder=remainder});return JAC_AbsorptionGameplay.Canonical(Game:GetProperty('JAC_NATIVE_FAITH_PRECISION'))"},{"action":"autoplay","turns":1,"timeout":600},{"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":"local p=assert(Game:GetProperty('JAC_NATIVE_FAITH_PRECISION'));local r=Players[p.Owner]:GetReligion();p.Income=r:GetFaithBalance();p.Expected=p.Whole+p.Income;r:ChangeFaithBalance(p.Whole);p.Actual=r:GetFaithBalance();p.Difference=p.Actual-p.Expected;print('[JAC_NATIVE_FAITH_PRECISION] '..JAC_AbsorptionGameplay.Canonical(p));return JAC_AbsorptionGameplay.Canonical(p)"}]});
        if std::path::Path::new(&a[1]).exists() {
            return Err("output exists".into());
        }
        fs::write(
            &a[1],
            serde_json::to_vec_pretty(&output).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
