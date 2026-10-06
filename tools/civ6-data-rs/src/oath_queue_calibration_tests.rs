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

#[test]
fn core_publishes_identity_and_ui_requests_without_a_property_setter() -> Result<()> {
    for mutation in [
        "",
        "owner=1",
        "cost=110000",
        "can_start=false",
        "props.JAC_TODO134_QCAL_ID.city=99",
    ] {
        let vm = Lua::new();
        vm.load(r#"
props={}; flags={}; requests=0; owner=0; cost=131072; can_start=true
Game={GetLocalPlayer=function()return 0 end, GetProperty=function(_,key)return props[key] end,
 SetProperty=function(_,key,value)props[key]=value end}
GameConfiguration={GetValue=function(key)return key=='JAC_TODO134_QUEUE_CALIBRATION' and 1 or 0 end}
GameInfo={Units={UNIT_SPEARMAN={Cost=131072,Index=1,Hash=123}},Yields={YIELD_PRODUCTION={Index=0}}}
local plot={GetOwner=function()return owner end,GetIndex=function()return 22 end,SetProperty=function(_,key,value)flags[key]=value end}
Map={GetPlot=function()return plot end}
local queue={GetUnitProgress=function()return 0 end,GetUnitCost=function()return cost end}
local city={GetID=function()return 3 end,GetOwner=function()return owner end,GetX=function()return 1 end,
 GetY=function()return 1 end,GetYield=function()return 8 end,GetBuildQueue=function()return queue end}
local cities={Members=function()local once=false;return function()if not once then once=true;return 3,city end end end,
 FindID=function(_,id)if id==3 then return city end end}
Players={[0]={IsHuman=function()return true end,IsTurnActive=function()return true end,
 GetCities=function()return cities end,AttachModifierByID=function()end}}
CityOperationTypes={PARAM_UNIT_TYPE=1,PARAM_INSERT_MODE=2,VALUE_REPLACE_AT=3,PARAM_QUEUE_DESTINATION_LOCATION=4,BUILD=5}
CityManager={CanStartOperation=function()return can_start end,RequestOperation=function(c,op,params)
 assert(c==city and op==5 and params[1]==123);requests=requests+1 end}
"#).exec()?;
        vm.load(format!("{LUA_HELPERS}\n{SETUP}"))
            .eval::<String>()?;
        assert_eq!(
            vm.load("return props.JAC_TODO134_QCAL_ID.city")
                .eval::<i64>()?,
            3
        );
        // InGame exposes GetProperty but not Game.SetProperty.
        vm.load("Game.SetProperty=nil").exec()?;
        vm.load(mutation).exec()?;
        let result = vm
            .load(format!("{LUA_HELPERS}\n{REQUEST}"))
            .eval::<String>();
        if mutation.is_empty() {
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(vm.load("return requests").eval::<i64>()?, 1);
        } else {
            assert!(result.is_err(), "{mutation}");
            assert_eq!(vm.load("return requests").eval::<i64>()?, 0);
        }
    }
    Ok(())
}

fn verdict(mode: &str) -> Result<serde_json::Value> {
    let vm = Lua::new();
    let lua = format!(
        "{LUA_HELPERS}\n{}\n{}",
        OBSERVATIONS.replace("__MODE__", &serde_json::to_string(mode)?),
        compare_code(1)
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
fn quarter_control_keeps_its_phase_identity_and_cannot_accept_the_one_percent_data() -> Result<()> {
    let vm = Lua::new();
    vm.load(
        OBSERVATIONS
            .replace("__MODE__", "'ok'")
            .replace("{100,1,15}", "{100,25,15}")
            .replace("and 520 or 4103", "and 519 or 4103"),
    )
    .exec()?;
    let value = vm
        .load(format!("{LUA_HELPERS}\n{}", compare_code(25)))
        .eval::<String>()?;
    let value: serde_json::Value = serde_json::from_str(&value)?;
    assert_eq!(value["accepted"], false);
    assert_eq!(value["calibration_established"], true, "{value}");
    assert_eq!(value["inputs"][0]["proportional_25_and_15"], true);
    assert!(vm
        .load(format!("{LUA_HELPERS}\n{}", compare_code(1)))
        .exec()
        .is_err());
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
Game={GetLocalPlayer=function()return 0 end,GetCurrentGameTurn=function()return current_turn end,GetProperty=function(_,key)return props[key] end}
local plot={GetOwner=function()return current_owner end,GetIndex=function()return 22 end}
Map={GetPlotByIndex=function()return plot end,GetPlot=function()return plot end}
local queue={GetCurrentProductionTypeHash=function()return current_hash end,GetUnitProgress=function()return progress end,GetUnitCost=function()return cost end}
local growth={GetHappinessNonFoodYieldModifier=function()return 0 end}
local city={GetOwner=function()return current_owner end,GetX=function()return 1 end,GetY=function()return 1 end,GetBuildQueue=function()return queue end,GetYield=function()return 4103 end,GetGrowth=function()return growth end}
Players={[0]={GetCities=function()return {FindID=function()return city end} end,IsTurnActive=function()return true end,IsTurnActiveComplete=function()return true end}}
GameInfo={Units={UNIT_SPEARMAN={Index=1}},Yields={YIELD_PRODUCTION={Index=0}}}
ActionTypes={ACTION_ENDTURN=1}; UI={RequestAction=function()requests=requests+1 end,CanEndTurn=function()return true end,HasSentTurnComplete=function()return false end,IsProcessingMessages=function()return false end}
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
            .filter(|s| s["action"] == "wait_for"
                && s["close_diplomacy"] == true
                && s["condition"]["code"]
                    .as_str()
                    .is_some_and(|c| c.contains("JAC_QUEUE_CAL")))
            .count(),
        38
    );
    assert!(steps.iter().all(|s| s["action"] != "autoplay"));
    for step in steps.iter().filter_map(|s| s["code"].as_str()) {
        Lua::new().load(step).into_function()?;
    }
    Ok(())
}

#[test]
fn generated_quarter_control_retains_all_native_requests_and_has_no_placeholders() -> Result<()> {
    let repo = Repository::checked_in();
    let data = quarter_control_scenario(&repo.json(super::super::BASE)?)?;
    assert_eq!(data["diagnostic_only"], true);
    assert_eq!(data["control_percent"], 25);
    assert_eq!(data["finite_turns"], 38);
    let steps = data["steps"].as_array().unwrap();
    assert_eq!(steps.iter().filter(|s| s["action"] == "save").count(), 1);
    assert_eq!(
        steps
            .iter()
            .filter(|s| s["code"]
                .as_str()
                .is_some_and(|c| c.contains("UI.RequestAction(ActionTypes.ACTION_ENDTURN,")))
            .count(),
        38
    );
    let mut quarter_on = 0;
    for step in steps.iter().filter_map(|s| s["code"].as_str()) {
        assert!(!step.contains("__CONTROL_PERCENT__"));
        Lua::new().load(step).into_function()?;
        if step.contains("percent=25") && step.contains("slot=\"on\"") {
            quarter_on += 1;
        }
    }
    assert_eq!(quarter_on, 4);
    Ok(())
}

#[test]
fn activation_gates_reject_pending_messages_and_sent_or_incomplete_turns() -> Result<()> {
    let vm = Lua::new();
    let reset = r#"
active=true;complete=true;can_end=true;sent=false;processing=false;local_id=0;requests=0
Game={GetLocalPlayer=function()return local_id end}
Players={[0]={IsTurnActive=function()return active end,IsTurnActiveComplete=function()return complete end}}
UI={CanEndTurn=function()return can_end end,HasSentTurnComplete=function()return sent end,IsProcessingMessages=function()return processing end,RequestAction=function()requests=requests+1 end}
"#;
    vm.load(reset).exec()?;
    assert!(vm.load(READY_CORE).eval::<bool>()?);
    assert!(vm.load(READY_UI).eval::<bool>()?);
    for mutation in [
        "active=false",
        "complete=false",
        "can_end=false",
        "sent=true",
        "processing=true",
        "local_id=1",
    ] {
        vm.load(reset).exec()?;
        vm.load(mutation).exec()?;
        assert!(!vm.load(READY_UI).eval::<bool>()?, "{mutation}");
        assert_eq!(vm.load("return requests").eval::<i64>()?, 0);
    }
    Ok(())
}
