use super::*;
use crate::Repository;
use anyhow::Result;
use mlua::{Lua, Value as LuaValue};

const OBSERVATIONS: &str = r#"
local mode=__MODE__
JAC_QUEUE_CAL={city=3,plot=22,owner=0,cost=131072,samples={}}
local turn=0; local progress=0
local function push(input,percent,rep,slot)
 turn=turn+1
 local baseline=input==512 and 520 or 4103
 local reference=input==512 and 520 or 4103
 local effect=math.floor(reference*percent/100+0.5)
 if mode=='no_reference' and percent==100 then effect=0 end
 if mode=='negative_reference' and percent==100 then effect=-100 end
 if mode=='coarse' and input==512 and percent==1 then effect=10 end
 if mode=='coarse' and input==512 and percent==15 then effect=80 end
 local delta=baseline+(slot=='on' and effect or 0)
 if mode=='off16' and input==512 and percent==100 and rep==1 and slot=='off_after' then delta=delta+16 end
 if mode=='repeat_mismatch' and input==512 and percent==100 and rep==2 and slot=='on' then delta=delta+10 end
 if mode=='wrong_one' and input==512 and percent==1 and rep==1 and slot=='on' then delta=delta+10 end
 if mode=='live13' and input==4096 and percent==100 and rep==1 then
  if slot=='off_before' then delta=4071 elseif slot=='on' then delta=4680 else delta=4055 end
 end
 local before=progress; progress=progress+delta
 local index=#JAC_QUEUE_CAL.samples+1
 JAC_QUEUE_CAL.samples[index]={index=index,input=input,percent=percent,rep=rep,slot=slot,turn=turn,city=3,plot=22,owner=0,cost=131072,before=before,after=progress,delta=delta,requested=true,before_diagnostic={production=4103},after_diagnostic={production=4103}}
end
for _,input in ipairs({512,4096}) do
 push(input,0,0,'warmup')
 for _,percent in ipairs({100,1,15}) do
  for rep=1,2 do
   for _,slot in ipairs({'off_before','on','off_after'}) do push(input,percent,rep,slot) end
  end
 end
end
if mode=='missing' then JAC_QUEUE_CAL.samples[2].delta=nil end
if mode=='nan' then JAC_QUEUE_CAL.samples[2].after=0/0 end
if mode=='negative' then JAC_QUEUE_CAL.samples[2].delta=-1 end
if mode=='turn' then JAC_QUEUE_CAL.samples[2].turn=8 end
if mode=='city' then JAC_QUEUE_CAL.samples[2].city=5 end
if mode=='phase' then JAC_QUEUE_CAL.samples[2].percent=15 end
if mode=='cost' then JAC_QUEUE_CAL.samples[2].cost=32768 end
if mode=='queue' then JAC_QUEUE_CAL.samples[2].before=12 end
if mode=='completion' then JAC_QUEUE_CAL.samples[38].after=131072 end
"#;

fn verdict(mode: &str) -> Result<serde_json::Value> {
    let vm = Lua::new();
    let lua = format!(
        "{LUA_HELPERS}\n{}\n{COMPARE}",
        OBSERVATIONS.replace("__MODE__", &serde_json::to_string(mode)?)
    );
    let LuaValue::String(value) = vm.load(lua).eval::<LuaValue>()? else {
        anyhow::bail!("calibration verdict is not JSON");
    };
    Ok(serde_json::from_str(value.to_str()?.as_ref())?)
}

#[test]
fn real_progress_establishes_a_diagnostic_verdict_without_using_displayed_yield() -> Result<()> {
    let result = verdict("ok")?;
    assert_eq!(result["diagnostic_only"], true);
    assert_eq!(result["accepted"], false);
    assert_eq!(result["calibration_established"], true, "{result}");
    assert_eq!(result["samples"], 38);
    assert_eq!(result["inputs"][0]["input"], 512);
    assert_eq!(result["inputs"][1]["input"], 4096);
    assert!(result["inputs"][0]["two_step_width"].as_f64().unwrap() < 0.005);
    assert!(!COMPARE.contains("GetYield"));
    assert!(!COMPARE.contains("city_yield"));
    Ok(())
}

#[test]
fn off_drift_repeat_ratio_reference_and_precision_cannot_pass() -> Result<()> {
    for mode in [
        "off16",
        "live13",
        "repeat_mismatch",
        "wrong_one",
        "no_reference",
        "negative_reference",
        "coarse",
    ] {
        let result = verdict(mode)?;
        assert_eq!(result["calibration_established"], false, "{mode}");
        assert_eq!(result["accepted"], false, "{mode}");
        let affected = if mode == "live13" { 1 } else { 0 };
        assert_eq!(
            result["inputs"][affected]["calibration_established"], false,
            "{mode}"
        );
    }
    let drift = verdict("off16")?;
    assert_eq!(drift["inputs"][0]["off_restored"], false);
    let live = verdict("live13")?;
    assert_eq!(live["inputs"][1]["off_restored"], false);
    let no_reference = verdict("no_reference")?;
    assert!(no_reference["inputs"][0]["two_step_width"].is_null());
    let coarse = verdict("coarse")?;
    assert!(coarse["inputs"][0]["two_step_width"].as_f64().unwrap() >= 0.005);
    Ok(())
}

#[test]
fn missing_nonfinite_negative_or_discontinuous_queue_data_is_rejected() {
    for mode in [
        "missing",
        "nan",
        "negative",
        "turn",
        "city",
        "phase",
        "cost",
        "queue",
        "completion",
    ] {
        assert!(
            verdict(mode).is_err(),
            "{mode} should reject invalid native observations"
        );
    }
}

#[test]
fn generated_observation_rejects_changed_queue_cost_owner_and_skipped_or_duplicate_turns(
) -> Result<()> {
    let vm = Lua::new();
    vm.load(r#"
current_turn=7; progress=100; cost=131072; current_hash=123; current_owner=0; requests=0
props={JAC_TODO134_QCAL_STATE={index=1,input=512,percent=0,rep=0,slot='warmup'},JAC_TODO134_QCAL_CORE={index=1,stage='before',turn=7,city=3,plot=22,owner=0,production=520}}
Game={GetCurrentGameTurn=function()return current_turn end,GetProperty=function(_,key)return props[key] end}
local plot={GetOwner=function()return current_owner end,GetIndex=function()return 22 end}
Map={GetPlotByIndex=function()return plot end,GetPlot=function()return plot end}
local queue={GetCurrentProductionTypeHash=function()return current_hash end,GetUnitProgress=function()return progress end,GetUnitCost=function()return cost end}
local growth={GetHappinessNonFoodYieldModifier=function()return 0 end}
local city={GetOwner=function()return current_owner end,GetX=function()return 1 end,GetY=function()return 1 end,GetBuildQueue=function()return queue end,GetYield=function()return 4103 end,GetGrowth=function()return growth end}
Players={[0]={GetCities=function()return {FindID=function()return city end} end,IsTurnActive=function()return true end}}
GameInfo={Units={UNIT_SPEARMAN={Index=1}},Yields={YIELD_PRODUCTION={Index=0}}}
ActionTypes={ACTION_ENDTURN=1}; UI={RequestAction=function()requests=requests+1 end}
JAC_QUEUE_CAL={city=3,plot=22,owner=0,unit_hash=123,cost=nil,samples={}}
"#).exec()?;
    let before = format!(
        "{LUA_HELPERS}\n{}",
        OBSERVE
            .replace("__INDEX__", "1")
            .replace("__STAGE__", "'before'")
    );
    let after = format!(
        "{LUA_HELPERS}\n{}",
        OBSERVE
            .replace("__INDEX__", "1")
            .replace("__STAGE__", "'after'")
    );
    vm.load(&before).eval::<String>()?;
    vm.load(END_TURN.replace("__INDEX__", "1"))
        .eval::<String>()?;
    assert!(vm.load(END_TURN.replace("__INDEX__", "1")).exec().is_err());
    let requests: i64 = vm.load("return requests").eval()?;
    assert_eq!(requests, 1);
    for mutation in [
        "current_turn=9; props.JAC_TODO134_QCAL_CORE.turn=9",
        "current_turn=8; props.JAC_TODO134_QCAL_CORE.turn=8; current_hash=456",
        "current_hash=123; cost=32768",
        "cost=131072; current_owner=1",
    ] {
        vm.load(mutation).exec()?;
        vm.load("props.JAC_TODO134_QCAL_CORE.stage='after'")
            .exec()?;
        assert!(vm.load(&after).exec().is_err(), "{mutation}");
    }
    vm.load("current_owner=0; current_turn=8; progress=620; props.JAC_TODO134_QCAL_CORE.turn=8")
        .exec()?;
    vm.load(&after).eval::<String>()?;
    Ok(())
}

#[test]
fn generated_diagnostic_is_finite_and_keeps_every_native_turn_in_one_save() -> Result<()> {
    let repo = Repository::checked_in();
    let data = scenario(&repo.json(super::super::BASE)?)?;
    assert_eq!(data["diagnostic_only"], true);
    assert_eq!(data["finite_turns"], 38);
    let steps = data["steps"].as_array().unwrap();
    assert_eq!(steps.iter().filter(|s| s["action"] == "save").count(), 1);
    assert_eq!(steps.iter().filter(|s| s["action"] == "load").count(), 0);
    assert_eq!(
        steps
            .iter()
            .filter(|s| s["code"]
                .as_str()
                .is_some_and(|c| c.contains("UI.RequestAction(ActionTypes.ACTION_ENDTURN,")))
            .count(),
        38
    );
    assert_eq!(
        steps
            .iter()
            .filter(|s| s["action"] == "wait_for" && s["close_diplomacy"] == true)
            .count(),
        38
    );
    assert!(steps.iter().all(|s| s["action"] != "autoplay"));
    for step in steps.iter().filter_map(|s| s["code"].as_str()) {
        Lua::new().load(step).into_function()?;
    }
    Ok(())
}
