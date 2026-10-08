//! Execute the generated assertions against independent numeric observations.
use super::*;
use mlua::{Lua, Value as LuaValue};

#[test]
fn modifier_inventory_preserves_false_and_distinguishes_unavailable_lists() -> Result<()> {
    use crate::oath_expiry_probe::Frame;
    let lua = Lua::new();
    lua.load(r#"
Game={GetCurrentGameTurn=function()return 1 end}
PlayerManager={GetAliveMajors=function()return {} end}
GameEffects={GetModifiers=function()return {17,18,19} end,
 GetModifierDefinition=function(id)if id==19 then error('definition read failed') end;return {Id=id==17 and 'JAC_OATH_ODA_TENKA_FUBU_E1' or 'UNRELATED_OFFICIAL_MODIFIER'} end,
 GetModifierActive=function()return false end,GetModifierOwner=function()return 0 end,
 GetModifierSubjectCount=function()return 0 end,GetModifierTrackedObjectCount=function()return 0 end,
 GetModifierSubjects=function()return {} end,GetModifierTrackedObjects=function()return {} end}
"#).exec()?;
    let code = MODIFIER_INVENTORY
        .replace("__OATH__", "'JAC_OATH_ODA_TENKA_FUBU'")
        .replace("__CONTEXT__", "'GameCore_Tuner'")
        .replace("__PHASE__", "'baseline'");
    let capture = || -> Result<Frame> {
        let value: String = lua
            .load(format!("{}{code}", crate::oath_save_probe::HELPERS))
            .eval()?;
        Frame::parse(
            value
                .strip_prefix("JAC_OATH_MODIFIER_INVENTORY|")
                .context("inventory marker")?,
        )
    };
    let observed = capture()?;
    assert_eq!(observed.get("inventory_available")?, &Frame::Bool(false));
    assert_eq!(observed.get("total_instance_count")?.int()?, 3);
    assert_eq!(observed.get("other_instance_count")?.int()?, 1);
    assert_eq!(
        observed.get("other_definitions")?.entries()?[0]
            .1
            .get("definition")?
            .text()?,
        "UNRELATED_OFFICIAL_MODIFIER"
    );
    assert!(observed.get("unresolved_definitions")?.entries()?[0]
        .1
        .get("call")?
        .get("error")?
        .text()?
        .contains("definition read failed"));
    let instance = &observed.get("instances")?.entries()?[0].1;
    assert_eq!(
        instance.get("GetModifierActive")?.get("value")?,
        &Frame::Bool(false)
    );
    assert_eq!(instance.get("GetModifierOwner")?.get("value")?.int()?, 0);
    lua.load("GameEffects.GetModifiers=function()return {17,18} end")
        .exec()?;
    assert_eq!(capture()?.get("inventory_available")?, &Frame::Bool(true));
    lua.load("GameEffects.GetModifierSubjects=function()return nil end")
        .exec()?;
    let observed = capture()?;
    assert_eq!(observed.get("inventory_available")?, &Frame::Bool(false));
    let instance = &observed.get("instances")?.entries()?[0].1;
    assert_eq!(
        instance
            .get("GetModifierSubjects")?
            .get("call")?
            .get("value_type")?
            .text()?,
        "nil"
    );
    lua.load(r#"
GameEffects.GetModifierSubjects=function()return {42} end
GameEffects.GetObjectName=function()return 'city name' end
GameEffects.GetObjectType=function()return 'City' end
GameEffects.GetObjectString=function()error('debug description failed')end
GameEffects.GetObjectsPlayerId=function()return 3 end
local city={GetID=function()return 65536 end,GetOwner=function()return 3 end,GetX=function()return 8 end,GetY=function()return 9 end}
PlayerManager.GetAliveMajors=function()return {{GetID=function()return 3 end,GetCities=function()return {Members=function()local used=false;return function()if not used then used=true;return 1,city end end end}end}}end
Map={GetPlot=function()return {GetOwner=function()return 3 end,GetIndex=function()return 789 end,GetProperty=function()return 0 end}end}
"#).exec()?;
    let observed = capture()?;
    assert_eq!(
        observed.get("cities")?.entries()?[0].1.get("plot")?.int()?,
        789
    );
    assert_eq!(
        observed.get("cities")?.entries()?[0].1.get("flag")?.int()?,
        0
    );
    assert_eq!(observed.get("inventory_available")?, &Frame::Bool(false));
    let instance = &observed.get("instances")?.entries()?[0].1;
    let object = &instance
        .get("GetModifierSubjects")?
        .get("objects")?
        .entries()?[0]
        .1;
    assert_eq!(object.get("id")?.int()?, 42);
    assert!(object
        .get("GetObjectString")?
        .get("error")?
        .text()?
        .contains("debug description failed"));
    lua.load("GameEffects.GetModifiers=nil").exec()?;
    let observed = capture()?;
    assert_eq!(observed.get("inventory_available")?, &Frame::Bool(false));
    assert_eq!(
        observed.get("calls")?.get("modifiers")?.get("available")?,
        &Frame::Bool(false)
    );
    lua.load(
        "GameEffects=nil;PlayerManager.GetAliveMajors=function()error('city inventory failed')end",
    )
    .exec()?;
    let observed = capture()?;
    assert_eq!(observed.get("inventory_available")?, &Frame::Bool(false));
    assert!(observed
        .get("city_call")?
        .get("error")?
        .text()?
        .contains("city inventory failed"));
    Ok(())
}

#[test]
fn modifier_inventory_scenario_is_observation_only_after_formal_choice() -> Result<()> {
    let repo = Repository::checked_in();
    let case = cases(&repo)?
        .into_iter()
        .find(|case| case.oath == "JAC_OATH_ODA_TENKA_FUBU")
        .unwrap();
    let scenario = modifier_inventory_scenario(&repo.json(BASE)?, &case)?;
    let steps = scenario["steps"].as_array().unwrap();
    assert_eq!(scenario["diagnostic_only"], true);
    assert_eq!(scenario["modifier_inventory"]["quantity_accepted"], false);
    assert_eq!(scenario["modifier_inventory"]["product_accepted"], false);
    assert_eq!(
        scenario["modifier_inventory"]["native_ordering_proved"],
        false
    );
    assert!(steps.iter().all(|step| ![
        "oath_exact_removal",
        "autoplay",
        "save",
        "load",
        "exit_to_menu"
    ]
    .contains(&step["action"].as_str().unwrap())));
    assert_eq!(
        steps
            .iter()
            .filter(|step| step["code"]
                .as_str()
                .is_some_and(|code| code.contains("real UI EXECUTE_SCRIPT")))
            .count(),
        1
    );
    let vm = Lua::new();
    for step in steps {
        if let Some(code) = step["code"]
            .as_str()
            .or_else(|| step["condition"]["code"].as_str())
        {
            vm.load(code).into_function()?;
        }
    }
    Ok(())
}

#[test]
fn growth_wait_rejects_previous_values_and_requires_every_owned_city_to_update() -> Result<()> {
    let lua = Lua::new();
    lua.load(r#"
values={0,0}; flag=1; yield_reads=0
GameInfo={Yields={}}; for i,name in ipairs({'PRODUCTION','GOLD','SCIENCE','CULTURE','FAITH','FOOD'}) do GameInfo.Yields['YIELD_'..name]={Index=i} end
JAC_OATH_AUDIT={baseline={own={{id=1,plot=11,owner=0,other=0},{id=2,plot=12,owner=0,other=0}}}}
Players={[0]={GetCities=function() return {FindID=function(_,id) return {
 GetOwner=function() return 0 end,GetX=function() return id end,GetY=function() return 0 end,
 GetYield=function() yield_reads=yield_reads+1; return 2 end,
 GetGrowth=function() return {GetOtherGrowthModifier=function() return values[id] end} end
} end} end}}
Map={GetPlot=function(x) return {GetIndex=function() return x+10 end,GetProperty=function() return flag end} end}
"#).exec()?;
    let code = GROWTH_UPDATED
        .replace("__PREVIOUS__", "'baseline'")
        .replace("__OATH__", "'JAC_OATH_ODA_TSUSHIMA'")
        .replace("__FLAG__", "1");
    assert!(!lua.load(&code).eval::<bool>()?);
    assert_eq!(lua.load("return yield_reads").eval::<i64>()?, 6);
    lua.load("values[1]=0.046875").exec()?;
    assert!(!lua.load(&code).eval::<bool>()?);
    lua.load("values[2]=0.046875").exec()?;
    assert!(lua.load(&code).eval::<bool>()?);
    lua.load("flag=0").exec()?;
    assert!(!lua.load(&code).eval::<bool>()?);
    lua.load("flag=1; values[2]=0/0").exec()?;
    assert!(lua.load(&code).eval::<bool>().is_err());
    // Readiness does not bless the rounded amount or hide a residue.
    lua.load("values[2]=-0.00390625").exec()?;
    assert!(lua.load(&code).eval::<bool>()?);
    Ok(())
}

#[test]
fn growth_core_ui_wait_rejects_stale_phase_and_foreign_metric_mismatches() -> Result<()> {
    let lua = Lua::new();
    lua.load(r#"
local names={'PRODUCTION','GOLD','SCIENCE','CULTURE','FAITH','FOOD'}
GameInfo={Yields={}}; for i,name in ipairs(names) do GameInfo.Yields['YIELD_'..name]={Index=i} end
local function row(id,owner)
 return {id=id,owner=owner,plot=100*owner+id,flag=owner==0 and 1 or 0,growth=1,other=0.046875,housing=1,happiness=0,surplus=2,loyalty=1,occupied=false,y={production=2,gold=2,science=2,culture=2,faith=2,food=2}}
end
actual={[0]={row(1,0),row(2,0)},[1]={}}
core={oath='JAC_OATH_ODA_TSUSHIMA',phase='selected',turn=4,snapshot={own={row(1,0),row(2,0)},foreign={}}}
for i=1,54 do actual[1][i]=row(i,1); core.snapshot.foreign[i]=row(i,1) end
Game={GetCurrentGameTurn=function() return 4 end,GetProperty=function() return core end}
Players={}
for owner=0,1 do local o=owner; Players[o]={GetCities=function() return {FindID=function(_,id)
 local r=actual[o][id]; if not r then return nil end
 return {GetID=function() return id end,GetOwner=function() return o end,GetX=function() return id end,GetY=function() return o end,
 IsOccupied=function() return r.occupied end,GetYield=function() return r.y.production end,
 GetGrowth=function() assert(not forbid_growth,'growth API is UI-only'); return {GetOverallGrowthModifier=function() return r.growth end,
 GetOtherGrowthModifier=function() return r.other end,GetHousingGrowthModifier=function() return r.housing end,
 GetHappinessGrowthModifier=function() return r.happiness end,GetFoodSurplus=function() return r.surplus end,
 GetLoyaltyGrowthModifier=function() return r.loyalty end} end}
end} end} end
Map={GetPlot=function(x,y) return {GetIndex=function() return 100*y+x end,GetOwner=function() return y end,
 GetProperty=function(_,name) return name=='JAC_OATH_ODA_TSUSHIMA_ON' and actual[y][x].flag or 0 end} end}
"#).exec()?;
    let code = GROWTH_CORE_MATCH
        .replace("__PHASE__", "'selected'")
        .replace("__OATH__", "'JAC_OATH_ODA_TSUSHIMA'");
    assert!(lua.load(&code).eval::<bool>()?);
    for change in [
        "core.phase='off'",
        "actual[0][2].flag=0",
        "actual[1][54].flag=1",
        "actual[1][54].y.production=2.00390625",
    ] {
        lua.load(change).exec()?;
        assert!(!lua.load(&code).eval::<bool>()?, "accepted {change}");
        lua.load("core.phase='selected'; actual[0][2].flag=1; actual[1][54].flag=0; actual[1][54].y.production=2").exec()?;
    }
    lua.load("core.turn=5").exec()?;
    assert!(lua.load(&code).eval::<bool>().is_err());
    // Execute the real Core snapshot with growth explicitly unavailable, as
    // in live33. It may use only identity, flags and verified yield getters.
    lua.load(r#"
forbid_growth=true
Game.SetProperty=function(_,name,value) assert(name=='JAC_TODO134_GROWTH_CORE'); core=value end
GameInfo.JAC_SengokuOaths=function() local i=0; return function() i=i+1; if i<=66 then return {OathType=i==1 and 'JAC_OATH_ODA_TSUSHIMA' or ('OTHER_'..i)} end end end
for owner=0,1 do local o=owner; Players[o].GetID=function() return o end
 local original=Players[o].GetCities
 Players[o].GetCities=function() local cities=original()
  cities.Members=function() local i=0; return function() i=i+1; if actual[o][i] then return i,cities:FindID(i) end end end
  return cities
 end
end
PlayerManager={GetAliveMajors=function() return {Players[0],Players[1]} end}
"#).exec()?;
    let snapshot = GROWTH_CORE_SNAPSHOT
        .replace("__PHASE__", "'selected'")
        .replace("__OATH__", "'JAC_OATH_ODA_TSUSHIMA'")
        .replace("__FLAG__", "1");
    let report = lua
        .load(format!("{LUA_HELPERS}\n{snapshot}"))
        .eval::<String>()?;
    assert!(report.contains("\"growth_api_used\":false"));
    assert!(lua.load(&code).eval::<bool>()?);
    Ok(())
}

#[test]
fn all_growth_measurements_use_update_and_core_ui_gates_before_acceptance() -> Result<()> {
    let repo = Repository::checked_in();
    let base = repo.json(BASE)?;
    let mut checked = 0;
    for case in cases(&repo)? {
        if !case.effects.iter().any(|effect| effect.kind == "growth") {
            continue;
        }
        let data = selected_scenario(&base, &case)?;
        let steps = data["steps"].as_array().unwrap();
        for phase in ["selected", "off", "restored"] {
            let pattern = format!("local phase={}", lua_string(phase));
            let index = steps
                .iter()
                .position(|step| {
                    step["state_name"] == "InGame"
                        && step["code"].as_str().is_some_and(|code| {
                            code.contains(&pattern) && code.contains("local function sample(c)")
                        })
                })
                .unwrap();
            assert_eq!(steps[index - 3]["state_name"], "GameCore_Tuner");
            assert!(steps[index - 3]["code"]
                .as_str()
                .unwrap()
                .contains("Game:SetProperty('JAC_TODO134_GROWTH_CORE'"));
            assert!(!steps[index - 3]["code"]
                .as_str()
                .unwrap()
                .contains("GetGrowth"));
            assert_eq!(steps[index - 2]["action"], "wait_for");
            assert!(steps[index - 2]["condition"]["code"]
                .as_str()
                .unwrap()
                .contains("core.snapshot.foreign"));
            assert_eq!(steps[index - 1]["action"], "wait_for");
            assert!(steps[index - 1]["condition"]["code"]
                .as_str()
                .unwrap()
                .contains("if value==row.other then return false end"));
            checked += 1;
        }
    }
    assert!(checked > 0);
    Ok(())
}

#[test]
fn every_native_production_snapshot_waits_for_the_shared_core_and_ui_contract() -> Result<()> {
    let repo = Repository::checked_in();
    let base = repo.json(BASE)?;
    let mut checked = 0;
    for case in cases(&repo)? {
        let data = selected_scenario(&base, &case)?;
        let steps = data["steps"].as_array().unwrap();
        for (index, step) in steps.iter().enumerate() {
            if step["state_name"] == "GameCore_Tuner"
                && step["code"]
                    .as_str()
                    .is_some_and(|c| c.contains("Game:SetProperty('JAC_TODO134_QUEUE_CORE'"))
            {
                assert!(index >= 2);
                for (gate, context, code) in [
                    (
                        &steps[index - 2],
                        "GameCore_Tuner",
                        civ6_autotest_rs::runner::CORE_PLAYER0_READY,
                    ),
                    (
                        &steps[index - 1],
                        "InGame",
                        civ6_autotest_rs::runner::HUMAN_TURN_READY_UI,
                    ),
                ] {
                    assert_eq!(gate["action"], "wait_for");
                    assert_eq!(gate["condition"]["state_name"], context);
                    assert_eq!(gate["condition"]["code"], code);
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 0);
    Ok(())
}

#[test]
fn reporting_step_supports_live_binary_yields_without_inventing_subinteger_progress() -> Result<()>
{
    let vm = Lua::new();
    let observed: f64 = vm
        .load(format!(
            "{LUA_HELPERS}\nreturn reported_step({{7.8984375,15.796875,7.9765625,8.984375}})"
        ))
        .eval()?;
    assert!((observed - (1.0 / 128.0 + 0.00002)).abs() < 1e-10);
    let integral: f64 = vm
        .load(format!(
            "{LUA_HELPERS}\nreturn reported_step({{0,7,17,26}})"
        ))
        .eval()?;
    assert!(integral >= 1.0, "integer progress must remain coarse");
    let noise: f64 = vm
        .load(format!(
            "{LUA_HELPERS}\nreturn reported_step({{100,110.0000000001,120.0000000002}})"
        ))
        .eval()?;
    assert!(
        noise >= 10.0,
        "floating noise must not imply a fine reporting step"
    );
    let same: LuaValue = vm
        .load(format!("{LUA_HELPERS}\nreturn reported_step({{7,7,7}})"))
        .eval()?;
    assert!(matches!(same, LuaValue::Nil));
    Ok(())
}

fn decode(lua: &Lua, code: &str) -> Result<Value> {
    let LuaValue::String(result) = lua.load(code).eval::<LuaValue>()? else {
        bail!("generated observation must return a JSON string");
    };
    Ok(serde_json::from_str(result.to_str()?.as_ref())?)
}

fn generated_code(case: &Case, needle: &str) -> Result<String> {
    let repo = Repository::checked_in();
    let scenario = selected_scenario(&repo.json(BASE)?, case)?;
    scenario["steps"]
        .as_array()
        .context("steps")?
        .iter()
        .filter_map(|step| step["code"].as_str())
        .find(|code| code.contains(needle))
        .map(str::to_owned)
        .context("generated assertion missing")
}

fn oda() -> Result<Case> {
    cases(&Repository::checked_in())?
        .into_iter()
        .find(|c| c.oath == "JAC_OATH_ODA_TENKA_FUBU")
        .context("catalogue lacks Tenka Fubu")
}

const PRODUCTION_OBSERVATIONS: &str = r#"
JAC_OATH_AUDIT={current={Target='UNIT_SPEARMAN',City=3},production={}}
function audit_rows(off,on,last,first_yield,on_yield,last_yield)
 local turn=0; local progress=10
 local function phase(delta,yield)
  local samples={}
  for i=1,1 do
   turn=turn+1; local before=progress; progress=progress+delta
   samples[i]={city=3,plot=22,turn=turn,before=before,after=progress,delta=delta,city_yield=yield,city_yield_after=yield,ui_city_yield=yield,ui_city_yield_after=yield,cost=32768}
  end
  return {samples=samples}
 end
 local first=phase(off,first_yield); local active=phase(on,on_yield); local final=phase(last,last_yield)
 return {warmup={samples={{city=3,plot=22,turn=0,before=0,after=10,cost=32768,end_requested=true}}},off_first=first,on=active,off_last=final}
end
-- Integer queue progress at sufficient input, near the nominal 15% target.
-- Distinct city yields ensure progress, not changing displayed yield, is tested.
JAC_OATH_AUDIT.production.UNIT_SPEARMAN=audit_rows(4099,4718,4101,4099,4103,4101)
"#;

#[test]
fn actual_progress_verifies_fifteen_percent_separately_from_city_yield() -> Result<()> {
    let code = generated_code(&oda()?, "local target=\"UNIT_SPEARMAN\"; local amount=15")?;
    let lua = Lua::new();
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    let result = decode(&lua, &code)?;
    assert_eq!(result["quantity_verified"], true);
    assert_eq!(result["expected_percent"], 15);
    assert_eq!(result["expected_on"], 1.15);
    assert!((result["on"].as_f64().unwrap() - 4718.0 / 4103.0).abs() < 1e-9);
    assert_eq!(result["restored"], true);
    for delta in [4103.0, 4513.0, 4651.0, 4756.0, 5000.0] {
        lua.load(PRODUCTION_OBSERVATIONS).exec()?;
        lua.load(format!(
            "JAC_OATH_AUDIT.production.UNIT_SPEARMAN=audit_rows(4099,{delta},4101,4099,4103,4101)"
        ))
        .exec()?;
        assert!(
            decode(&lua, &code).is_err(),
            "accepted wrong on-progress {delta}"
        );
    }
    Ok(())
}

#[test]
fn queue_inputs_stay_fixed_and_conditioning_precedes_every_measured_target() -> Result<()> {
    let repo = Repository::checked_in();
    let files = outputs(&repo)?;
    for case in cases(&repo)? {
        let scene: Value = serde_json::from_str(
            &files[&PathBuf::from(format!("{DIR}/{}.json", case.oath.to_lowercase()))],
        )?;
        let vm = Lua::new();
        vm.load(format!(r#"
properties={{}}; current_oath={}; ledger={{reign=1,houses={{p0={{oathChosen=1,civ={},oaths={{}}}}}}}}
Game={{GetProperty=function(_,key) if key=='JAC_AGES_LEDGER' then return ledger else return {{Target=current_target,Oath=current_oath}} end end,SetProperty=function()end}}
local city={{GetOwner=function()return 0 end,GetX=function()return 1 end,GetY=function()return 1 end}}
Players={{[0]={{GetCities=function()return {{Members=function() local done=false; return function() if not done then done=true; return 1,city end end end}} end}}}}
local plot={{GetOwner=function()return 0 end,SetProperty=function(_,key,value)properties[key]=value end}}
Map={{GetPlot=function()return plot end}}
"#,lua_string(&case.oath),lua_string(&case.civ))).exec()?;
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for step in scene["steps"].as_array().context("steps")? {
            let Some(code) = step["code"].as_str() else {
                continue;
            };
            if !code.contains("return 'production turn '") {
                continue;
            }
            let target = code
                .split("local target=")
                .nth(1)
                .context("target")?
                .split(';')
                .next()
                .context("target delimiter")?;
            vm.load(format!("current_target={target}")).exec()?;
            vm.load(code).exec()?;
            let _: bool=vm.load("assert(properties.JAC_TODO134_FINE_ON==0 and properties.JAC_TODO134_SHIFT_ON==0 and properties.JAC_TODO134_SHIFT_TWO_ON==0); assert(properties.JAC_TODO134_HEADROOM_ON==1); return true").eval()?;
            let n = counts.entry(target.to_owned()).or_default();
            if *n == 0 {
                assert!(
                    code.contains("'production turn '..\"warmup\""),
                    "first phase must condition queue: {}",
                    case.oath
                )
            }
            *n += 1;
        }
        assert!(
            counts.values().all(|n| *n == 5),
            "conditioning, off/on/off and restore required"
        );
        assert_eq!(counts.len(), production_expectations(&case).len());
        if case.effects.iter().any(|e| e.kind == "walls") {
            assert!(production_expectations(&case).contains(&("BUILDING_MONUMENT".into(), 0)));
            assert!(!production_expectations(&case)
                .iter()
                .any(|(t, _)| t == "UNIT_BUILDER"));
        }
    }
    Ok(())
}

#[test]
fn generated_core_snapshot_rejects_missing_stale_foreign_and_nonfinite_observations() -> Result<()>
{
    let code = generated_code(&oda()?, "return 'core production snapshot '")?;
    let vm = Lua::new();
    vm.load(r#"
props={JAC_OATH_PRODUCTION_TEST={Target='UNIT_SPEARMAN'}}; ui_turn=7
Game={GetCurrentGameTurn=function()return ui_turn end,GetProperty=function(_,key)return props[key] end,SetProperty=function(_,key,value)props[key]=value end}
GameInfo={Yields={YIELD_PRODUCTION={Index=0}}}
local cities={}; for i=1,2 do local id=i==1 and 3 or 5; local x=i; cities[i]={GetID=function()return id end,GetOwner=function()return 0 end,GetX=function()return x end,GetY=function()return 1 end,GetYield=function()return 4096.125+x-1 end} end
Players={[0]={GetCities=function()return {Members=function() local i=0; return function() i=i+1; if cities[i] then return i,cities[i] end end end} end}}
Map={GetPlot=function(x)return {GetOwner=function()return 0 end,GetIndex=function()return x==1 and 22 or 33 end} end}
"#).exec()?;
    let reader=vm.load(format!("{LUA_HELPERS}\n{PRODUCTION_CORE_READER}\nreturn function() return core_production('UNIT_SPEARMAN','warmup','before',1,{{City=3,Plot=22}}) end")).eval::<mlua::Function>()?;
    vm.load(&code).exec()?;
    assert_eq!(reader.call::<f64>(())?, 4096.125);
    for mutation in [
        "props.JAC_TODO134_QUEUE_CORE=nil",
        "props.JAC_TODO134_QUEUE_CORE.turn=6",
        "props.JAC_TODO134_QUEUE_CORE.stage='after'",
        "props.JAC_TODO134_QUEUE_CORE.target='UNIT_BUILDER'",
        "props.JAC_TODO134_QUEUE_CORE.cities[1].owner=1",
        "props.JAC_TODO134_QUEUE_CORE.cities[1].plot=33",
        "props.JAC_TODO134_QUEUE_CORE.cities[1].production='4096.125'",
        "props.JAC_TODO134_QUEUE_CORE.cities[1].production=0/0",
        "props.JAC_TODO134_QUEUE_CORE.cities[2]=props.JAC_TODO134_QUEUE_CORE.cities[1]",
    ] {
        vm.load(&code).exec()?;
        vm.load(mutation).exec()?;
        assert!(reader.call::<f64>(()).is_err(), "accepted {mutation}");
    }
    Ok(())
}

#[test]
fn absent_conditioning_and_core_ui_disagreement_never_prove_production() -> Result<()> {
    let code = generated_code(&oda()?, "local target=\"UNIT_SPEARMAN\"; local amount=15")?;
    let vm = Lua::new();
    vm.load(PRODUCTION_OBSERVATIONS).exec()?;
    vm.load("JAC_OATH_AUDIT.production.UNIT_SPEARMAN.warmup=nil")
        .exec()?;
    assert!(decode(&vm, &code).is_err());
    vm.load(PRODUCTION_OBSERVATIONS).exec()?;
    vm.load("JAC_OATH_AUDIT.production.UNIT_SPEARMAN.off_first.samples[1].ui_city_yield=999")
        .exec()?;
    assert_eq!(decode(&vm, &code)?["quantity_verified"], false);
    vm.load(PRODUCTION_OBSERVATIONS).exec()?;
    vm.load("JAC_OATH_AUDIT.production.UNIT_SPEARMAN.warmup.samples[1].turn='0'")
        .exec()?;
    assert!(decode(&vm, &code).is_err());
    Ok(())
}

#[test]
fn other_production_modifiers_and_insufficient_resolution_do_not_prove_percentage() -> Result<()> {
    let code = generated_code(&oda()?, "local target=\"UNIT_SPEARMAN\"; local amount=15")?;
    let lua = Lua::new();
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    lua.load("JAC_OATH_AUDIT.production.UNIT_SPEARMAN=audit_rows(15,19.8,15,10,12,10)")
        .exec()?;
    let result = decode(&lua, &code)?;
    assert_eq!(result["quantity_verified"], false);
    assert_eq!(result["unmodified_off_basis_observed"], false);
    assert_eq!(result["expected_on"], Value::Null);
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    lua.load("JAC_OATH_AUDIT.production.UNIT_SPEARMAN=audit_rows(0.1,0.115,0.1,0.1,0.1,0.1)")
        .exec()?;
    let result = decode(&lua, &code)?;
    assert_eq!(result["quantity_verified"], false);
    assert_eq!(result["integer_percentage_resolved"], false);
    Ok(())
}

#[test]
fn non_target_builds_wrong_city_and_nonconsecutive_turns_are_rejected() -> Result<()> {
    let case = oda()?;
    let subject = generated_code(&case, "local target=\"UNIT_SPEARMAN\"; local amount=15")?;
    let control = generated_code(&case, "local target=\"UNIT_BUILDER\"; local amount=0")?;
    let lua = Lua::new();
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    lua.load("JAC_OATH_AUDIT.current.Target='UNIT_BUILDER'; JAC_OATH_AUDIT.production.UNIT_BUILDER=audit_rows(10.01,12.09,11.02,10.01,12.09,11.02)").exec()?;
    assert_eq!(decode(&lua, &control)?["quantity_verified"], true);
    lua.load(
        "JAC_OATH_AUDIT.production.UNIT_BUILDER=audit_rows(10.01,13.9035,11.02,10.01,12.09,11.02)",
    )
    .exec()?;
    assert!(
        decode(&lua, &control).is_err(),
        "specialty leaked to a builder"
    );
    for mutation in [
        "r.on.samples[1].city=4",
        "r.off_last.samples[1].plot=99",
        "r.on.samples[1].turn=3",
        "r.off_last.samples[1].delta=9",
    ] {
        lua.load(PRODUCTION_OBSERVATIONS).exec()?;
        lua.load(format!(
            "local r=JAC_OATH_AUDIT.production.UNIT_SPEARMAN; {mutation}"
        ))
        .exec()?;
        assert!(decode(&lua, &subject).is_err(), "accepted {mutation}");
    }
    Ok(())
}

#[test]
fn generated_native_end_turn_is_forced_and_is_never_requested_twice() -> Result<()> {
    let code = generated_code(&oda()?, "single native human end-turn requested")?;
    let lua = Lua::new();
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    lua.load(r#"
local calls=0
JAC_OATH_AUDIT.production.UNIT_SPEARMAN.warmup.samples[1].end_requested=nil
Game={GetCurrentGameTurn=function() return 0 end}
Players={[0]={IsTurnActive=function() return true end}}
ActionTypes={ACTION_ENDTURN=9}
UI={RequestAction=function(action,params) assert(action==9 and params.REASON=='UserForced'); calls=calls+1 end}
native_request_count=function() return calls end
"#).exec()?;
    lua.load(&code).exec()?;
    assert!(lua.load(&code).exec().is_err());
    let value = lua
        .load("return native_request_count()")
        .eval::<LuaValue>()?;
    match value {
        LuaValue::Number(calls) => assert_eq!(calls, 1.0),
        LuaValue::Integer(calls) => assert_eq!(calls, 1),
        _ => bail!("numeric native request count required"),
    }
    Ok(())
}

#[test]
fn single_turn_samples_distinguish_adjacent_percentages_and_reject_low_resolution() -> Result<()> {
    let military = generated_code(&oda()?, "local target=\"UNIT_SPEARMAN\"; local amount=15")?;
    let mut wall_case = growth_case();
    wall_case.effects = vec![
        Effect {
            kind: "walls".into(),
            amount: 20,
        },
        Effect {
            kind: "growth".into(),
            amount: 10,
        },
    ];
    let wall = generated_code(
        &wall_case,
        "local target=\"BUILDING_WALLS\"; local amount=20",
    )?;
    let lua = Lua::new();
    for (target, code, correct, adjacent) in [
        ("UNIT_SPEARMAN", military, 4714, 4673),
        ("BUILDING_WALLS", wall, 4919, 4878),
    ] {
        lua.load(PRODUCTION_OBSERVATIONS).exec()?;
        lua.load(format!("JAC_OATH_AUDIT.current.Target='{target}'; JAC_OATH_AUDIT.production['{target}']=audit_rows(4099,{correct},4099,4099,4099,4099)")).exec()?;
        let result = decode(&lua, &code)?;
        assert_eq!(result["quantity_verified"], true);
        assert_eq!(result["integer_percentage_resolved"], true);
        lua.load(format!(
            "JAC_OATH_AUDIT.production['{target}']=audit_rows(4099,{adjacent},4099,4099,4099,4099)"
        ))
        .exec()?;
        assert!(
            decode(&lua, &code).is_err(),
            "accepted adjacent amount for {target}"
        );
        lua.load(format!(
            "JAC_OATH_AUDIT.production['{target}']=audit_rows(20,23,20,20,20,20)"
        ))
        .exec()?;
        let unresolved = decode(&lua, &code)?;
        assert_eq!(unresolved["quantity_verified"], false);
        assert_eq!(unresolved["integer_percentage_resolved"], false);
    }
    Ok(())
}

#[test]
fn production_changes_inside_a_turn_never_prove_a_specialty_percentage() -> Result<()> {
    let code = generated_code(&oda()?, "local target=\"UNIT_SPEARMAN\"; local amount=15")?;
    let lua = Lua::new();
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    lua.load("JAC_OATH_AUDIT.production.UNIT_SPEARMAN.on.samples[1].city_yield_after=13")
        .exec()?;
    let result = decode(&lua, &code)?;
    assert_eq!(result["quantity_verified"], false);
    assert_eq!(result["city_production_stable_inside_turns"], false);
    assert_eq!(result["expected_on"], Value::Null);
    Ok(())
}

const CITY_OBSERVATIONS: &str = r#"
local oath='JAC_OATH_ODA_TENKA_FUBU'
GameInfo={JAC_SengokuOaths=function()
 local i=0; return function() i=i+1; if i<=66 then return {OathType=i==1 and oath or ('OTHER_'..i)} end end
end}
Map={GetPlotByIndex=function() return {GetOwner=function() return 0 end,GetProperty=function(_,name) return name==oath..'_ON' and 1 or 0 end} end}
local function city(owner,on)
 return {id=owner+1,plot=owner+10,owner=owner,y={production=10,gold=3,science=on and 2.1 or 2,culture=3,faith=1,food=10},
 growth=on and 0.55 or 0.5,other=on and 0.1 or 0,housing=0.5,happiness=0,surplus=2,occupied=false,loyalty=1}
end
JAC_OATH_AUDIT={}
for _,phase in ipairs({'baseline','selected','off','restored'}) do
 JAC_OATH_AUDIT[phase]={own={city(0,phase=='selected' or phase=='restored')},foreign={city(1,false)}}
end
"#;

fn growth_case() -> Case {
    Case {
        house: "ODA".into(),
        civ: "CIVILIZATION_JAC_ODA".into(),
        oath: "JAC_OATH_ODA_TENKA_FUBU".into(),
        effects: vec![
            Effect {
                kind: "growth".into(),
                amount: 10,
            },
            Effect {
                kind: "science".into(),
                amount: 5,
            },
        ],
    }
}

#[test]
fn growth_uses_housing_formula_and_raw_yield_quantity_remains_unaccepted() -> Result<()> {
    let code = generated_code(&growth_case(), "local spec=")?;
    let lua = Lua::new();
    lua.load(CITY_OBSERVATIONS).exec()?;
    let result = decode(&lua, &code)?;
    assert_eq!(result["results"][0]["expected_delta"], 0.05);
    assert_eq!(result["results"][0]["observed_delta"], 0.05);
    assert_eq!(result["results"][0]["quantity_verified"], true);
    assert_eq!(result["results"][1]["expected_delta"], Value::Null);
    assert_eq!(result["results"][1]["expected_percent"], 5);
    assert_eq!(result["quantity_verified"], false);
    for mutation in [
        "a.selected.own[1].other=0.01",
        "a.selected.own[1].growth=0.6",
        "a.off.own[1].growth=0.55",
        "a.selected.foreign[1].y.science=3",
        "a.restored.foreign[1].plot=99",
        "a.selected.own[1].y.gold=4",
    ] {
        lua.load(CITY_OBSERVATIONS).exec()?;
        lua.load(format!("local a=JAC_OATH_AUDIT; {mutation}"))
            .exec()?;
        assert!(decode(&lua, &code).is_err(), "accepted {mutation}");
    }
    Ok(())
}

#[test]
fn all_production_kinds_keep_native_q8_and_nominal_acceptance_separate() -> Result<()> {
    let repo = Repository::checked_in();
    let mut seen = std::collections::BTreeSet::new();
    for case in cases(&repo)? {
        let Some(effect) = case
            .effects
            .iter()
            .find(|e| ["military", "naval", "walls"].contains(&e.kind.as_str()))
        else {
            continue;
        };
        seen.insert(effect.kind.clone());
        for (target, amount) in production_expectations(&case) {
            let normal = generated_code(
                &case,
                &format!(
                    "local target={}; local amount={amount}",
                    lua_string(&target)
                ),
            )?;
            let entry = crate::oath_save_probe::catalogue(&repo)?
                .into_iter()
                .find(|entry| entry["oath"] == case.oath)
                .context("catalogue entry")?;
            if case.effects.iter().any(|e| e.kind == "growth") {
                continue; // Native production probes explicitly reject mixed growth.
            }
            let native = crate::oath_save_probe::make_native_probe(&repo, &entry)?;
            let diagnostic = native["steps"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|step| step["code"].as_str())
                .find(|code| {
                    code.contains(&format!(
                        "local target={}; local amount={amount}",
                        lua_string(&target)
                    ))
                })
                .context("native production comparison")?;
            let vm = Lua::new();
            vm.load(PRODUCTION_OBSERVATIONS).exec()?;
            let observe = |delta: f64| -> Result<()> {
                vm.load(format!(
                    "JAC_OATH_AUDIT.current.Target={0}; JAC_OATH_AUDIT.production[{0}]=audit_rows(4099,{delta},4101,4099,4103,4101)",
                    lua_string(&target),
                )).exec()?;
                Ok(())
            };
            let raw = native_percentage_raw(amount);
            observe((4103.0 * (1.0 + raw as f64 / 256.0)).round())?;
            let result = decode(&vm, diagnostic)?;
            assert_eq!(result["quantity_verified"], true, "{} {target}", case.oath);
            assert_eq!(result["expected_on"], 1.0 + raw as f64 / 256.0);
            if amount != 0 {
                assert!(
                    decode(&vm, &normal).is_err(),
                    "native result blessed as nominal: {} {target}",
                    case.oath
                );
                observe((4103.0 * (1.0 + amount as f64 / 100.0)).round())?;
                assert_eq!(decode(&vm, &normal)?["quantity_verified"], true);
                assert!(
                    decode(&vm, diagnostic).is_err(),
                    "nominal result blessed as Q8: {} {target}",
                    case.oath
                );
            }
            // Both paths retain the exact reversible OFF requirement.
            vm.load(format!(
                "JAC_OATH_AUDIT.production[{}]=audit_rows(4099,4921,4201,4099,4103,4101)",
                lua_string(&target),
            ))
            .exec()?;
            assert!(decode(&vm, &normal).is_err());
            assert!(decode(&vm, diagnostic).is_err());
        }
    }
    assert_eq!(
        seen,
        std::collections::BTreeSet::from(["military".into(), "naval".into(), "walls".into()])
    );
    Ok(())
}

#[test]
fn native_q8_observation_does_not_replace_nominal_quantity_acceptance() -> Result<()> {
    let lua = Lua::new();
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    lua.load("JAC_OATH_AUDIT.production.UNIT_SPEARMAN=audit_rows(4099,4712,4101,4099,4103,4101)")
        .exec()?;
    let normal = generated_code(&oda()?, "local target=\"UNIT_SPEARMAN\"; local amount=15")?;
    assert!(decode(&lua, &normal).is_err());
    let diagnostic = normal.replace(
        "local native_diagnostic=false",
        "local native_diagnostic=true",
    );
    assert_eq!(decode(&lua, &diagnostic)?["expected_on"], 1.1484375);
    lua.load(CITY_OBSERVATIONS).exec()?;
    lua.load("for _,phase in ipairs({'selected','restored'}) do local r=JAC_OATH_AUDIT[phase].own[1]; r.other=25/256; r.growth=140/256 end").exec()?;
    let normal = generated_code(&growth_case(), "local spec=")?;
    assert!(decode(&lua, &normal).is_err());
    assert!(decode(
        &lua,
        &normal.replace(
            "local native_diagnostic=false",
            "local native_diagnostic=true"
        )
    )
    .is_ok());
    Ok(())
}

#[test]
fn growth_quantizes_signed_happiness_before_the_housing_product() -> Result<()> {
    let code = generated_code(&growth_case(), "local spec=")?.replace(
        "local native_diagnostic=false",
        "local native_diagnostic=true",
    );
    let lua = Lua::new();
    // Audited producer vectors: happiness 20 -> raw 51; -15 -> raw -38.
    // With housing 128/256 and a +25 raw oath, the two products round
    // independently. Decimal happiness arithmetic predicts different samples.
    for (happiness, baseline, selected) in [(20, 153, 166), (-15, 109, 121)] {
        lua.load(CITY_OBSERVATIONS).exec()?;
        lua.load("for _,phase in ipairs({'selected','restored'}) do JAC_OATH_AUDIT[phase].own[1].other=25/256 end").exec()?;
        lua.load(format!(
            "for _,phase in ipairs({{'baseline','selected','off','restored'}}) do local r=JAC_OATH_AUDIT[phase].own[1]; r.happiness={happiness}; r.growth=((phase=='selected' or phase=='restored') and {selected} or {baseline})/256 end"
        )).exec()?;
        assert!(decode(&lua, &code).is_ok(), "happiness {happiness}");
        lua.load("JAC_OATH_AUDIT.baseline.own[1].growth=JAC_OATH_AUDIT.baseline.own[1].growth+1/256; JAC_OATH_AUDIT.off.own[1].growth=JAC_OATH_AUDIT.baseline.own[1].growth").exec()?;
        assert!(
            decode(&lua, &code).is_err(),
            "accepted wrong producer vector"
        );
    }
    Ok(())
}

#[test]
fn growth_uses_native_literal_vectors_and_still_rejects_signed_removal_residue() -> Result<()> {
    // Golden vectors from the pinned DLL's positive fixed-point converter;
    // the decimal 10% value is not a representable UI Q8 sample.
    for (amount, raw) in [
        (1, 2),
        (5, 12),
        (10, 25),
        (15, 38),
        (20, 51),
        (25, 64),
        (100, 256),
    ] {
        assert_eq!(native_percentage_raw(amount), raw);
    }
    let code = generated_code(&growth_case(), "local spec=")?.replace(
        "local native_diagnostic=false",
        "local native_diagnostic=true",
    );
    let lua = Lua::new();
    for mutation in [
        "a.selected.own[1].other=0.1; a.selected.own[1].growth=0.55",
        "a.off.own[1].other=-0.00390625",
        "a.restored.own[1].other=0.09375",
        "a.off.own[1].growth=0.49609375",
        "a.selected.own[1].other=0.09375",
    ] {
        lua.load(CITY_OBSERVATIONS).exec()?;
        lua.load("for _,phase in ipairs({'selected','restored'}) do local r=JAC_OATH_AUDIT[phase].own[1]; r.other=25/256; r.growth=140/256 end").exec()?;
        lua.load(format!("local a=JAC_OATH_AUDIT; {mutation}"))
            .exec()?;
        assert!(decode(&lua, &code).is_err(), "accepted {mutation}");
    }
    Ok(())
}

#[test]
fn growth_overall_matches_pinned_signed_division_and_each_q8_product() -> Result<()> {
    let lua = Lua::new();
    lua.load(format!(
        "{GROWTH_NATIVE_FORMULA}\ncheck_overall=native_overall"
    ))
    .exec()?;
    let check: mlua::Function = lua.globals().get("check_overall")?;
    // Literal results from RVA1247C0, not from the implementation under test.
    for (happy, other, housing, loyalty, expected) in [
        (20.0, 0.0, 1.0, 1.0, 307),
        (20.0, 0.0, 0.5, 1.0, 153),
        (-15.0, 0.0, 1.0, 1.0, 218),
        (20.0, 0.0, 0.5, 0.75, 114),
        (-100.0, -1.0, 0.5, 0.75, 0),
    ] {
        let row = lua.create_table()?;
        row.set("happiness", happy)?;
        row.set("other", other)?;
        row.set("housing", housing)?;
        row.set("loyalty", loyalty)?;
        assert_eq!(check.call::<f64>((row, 0))?, f64::from(expected) / 256.0);
    }
    for field in ["happiness", "other", "housing", "loyalty"] {
        for invalid in [0.1, f64::NAN, f64::INFINITY, 8388608.0] {
            let row = lua.create_table()?;
            row.set("happiness", 0)?;
            row.set("other", 0)?;
            row.set("housing", 1)?;
            row.set("loyalty", 1)?;
            row.set(field, invalid)?;
            assert!(
                check.call::<f64>((row, 0)).is_err(),
                "accepted {field}={invalid}"
            );
        }
    }
    for (other, loyalty) in [
        (2147483647.0 / 256.0, 1.0),
        (2000000000.0 / 256.0, 2000000000.0 / 256.0),
    ] {
        let row = lua.create_table()?;
        row.set("happiness", 0)?;
        row.set("other", other)?;
        row.set("housing", 1)?;
        row.set("loyalty", loyalty)?;
        assert!(
            check.call::<f64>((row, 0)).is_err(),
            "accepted overflowing arithmetic"
        );
    }
    Ok(())
}

#[test]
fn growth_observation_rejects_decimal_formula_and_non_q8_snapshots() -> Result<()> {
    let code = generated_code(&growth_case(), "local spec=")?.replace(
        "local native_diagnostic=false",
        "local native_diagnostic=true",
    );
    let lua = Lua::new();
    // Happiness -15: old decimal formula gives 217, actual signed quotient 218.
    for mutation in [
        "a.baseline.own[1].growth=217/256; a.off.own[1].growth=217/256",
        "a.selected.own[1].happiness=-15+0.000001",
        "a.selected.own[1].other=25/256+0.000001",
        "a.restored.own[1].growth=243/256+0.000001",
        "a.off.own[1].growth=217/256",
    ] {
        lua.load(CITY_OBSERVATIONS).exec()?;
        lua.load("for _,p in ipairs({'baseline','selected','off','restored'}) do local r=JAC_OATH_AUDIT[p].own[1]; local on=p=='selected' or p=='restored'; r.happiness=-15; r.housing=1; r.other=on and 25/256 or 0; r.growth=on and 243/256 or 218/256 end").exec()?;
        assert_eq!(
            decode(&lua, &code)?["results"][0]["expected_delta"],
            25.0 / 256.0
        );
        lua.load(format!("local a=JAC_OATH_AUDIT; {mutation}"))
            .exec()?;
        assert!(decode(&lua, &code).is_err(), "accepted {mutation}");
    }
    Ok(())
}

#[test]
fn native_reference_proves_yield_percentage_and_rejects_wrong_reference_or_leakage() -> Result<()> {
    let code = generated_code(&growth_case(), "local spec=")?;
    let lua = Lua::new();
    let reference = r#"
local function copy(value)
 if type(value)~='table' then return value end
 local result={}; for k,v in pairs(value) do result[k]=copy(v) end; return result
end
local a=JAC_OATH_AUDIT
a.selected.own[1].y.science=2.4; a.restored.own[1].y.science=2.4
a.reference=copy(a.baseline); a.reference_off=copy(a.baseline)
a.reference.own[1].y.science=a.baseline.own[1].y.science+8
a.precision_one=copy(a.baseline); a.precision_two=copy(a.baseline); a.precision_off=copy(a.baseline)
a.precision_one.own[1].y.science=a.baseline.own[1].y.science+0.08; a.precision_two.own[1].y.science=a.baseline.own[1].y.science+1.09
"#;
    lua.load(CITY_OBSERVATIONS).exec()?;
    lua.load(reference).exec()?;
    let result = decode(&lua, &code)?;
    assert_eq!(result["quantity_verified"], true);
    assert_eq!(result["results"][1]["expected_delta"], 0.4);
    assert_eq!(
        result["results"][1]["reason"],
        "native_100_percent_reference_delta"
    );
    for mutation in [
        "a.reference.own[1].y.science=a.baseline.own[1].y.science",
        "a.reference.own[1].y.science=a.baseline.own[1].y.science+9",
        "a.reference_off.own[1].y.science=99",
        "a.reference.own[1].id=99",
        "a.reference.own[1].owner=99",
        "a.reference.foreign={}",
        "a.reference_off=nil",
        "a.reference.foreign[1].y.gold=99",
        "a.reference_off.foreign[1].owner=0",
    ] {
        lua.load(CITY_OBSERVATIONS).exec()?;
        lua.load(reference).exec()?;
        lua.load(format!("local a=JAC_OATH_AUDIT; {mutation}"))
            .exec()?;
        assert!(decode(&lua, &code).is_err(), "accepted {mutation}");
    }
    lua.load(CITY_OBSERVATIONS).exec()?;
    lua.load(reference).exec()?;
    lua.load("local a=JAC_OATH_AUDIT; a.reference.own[1].y.science=a.baseline.own[1].y.science+2; a.selected.own[1].y.science=2.1; a.restored.own[1].y.science=2.1").exec()?;
    assert_eq!(decode(&lua, &code)?["quantity_verified"], false);
    Ok(())
}

#[test]
fn reference_mod_has_only_explicit_scoped_inputs_and_all_valid_modifier_definitions() -> Result<()>
{
    let files = meter_outputs();
    let sql = &files[&PathBuf::from(format!("{METER_DIR}/Data/Meter.sql"))];
    let db = rusqlite::Connection::open_in_memory()?;
    db.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE Requirements(RequirementId TEXT PRIMARY KEY,RequirementType TEXT NOT NULL); CREATE TABLE RequirementArguments(RequirementId TEXT REFERENCES Requirements,Name TEXT,Value TEXT,PRIMARY KEY(RequirementId,Name)); CREATE TABLE RequirementSets(RequirementSetId TEXT PRIMARY KEY,RequirementSetType TEXT); CREATE TABLE RequirementSetRequirements(RequirementSetId TEXT REFERENCES RequirementSets,RequirementId TEXT REFERENCES Requirements,PRIMARY KEY(RequirementSetId,RequirementId)); CREATE TABLE Modifiers(ModifierId TEXT PRIMARY KEY,ModifierType TEXT NOT NULL,SubjectRequirementSetId TEXT REFERENCES RequirementSets); CREATE TABLE ModifierArguments(ModifierId TEXT REFERENCES Modifiers,Name TEXT,Value TEXT,PRIMARY KEY(ModifierId,Name));")?;
    db.execute_batch(sql)?;
    let count: i64 = db.query_row("SELECT count(*) FROM Modifiers", [], |r| r.get(0))?;
    assert_eq!(count, 45);
    for percent in [100, 1, 15] {
        let id = format!("JAC_TODO134_QCAL_TAG_P{percent}");
        let row: (String, String, String, String, String) = db.query_row(
            "SELECT m.ModifierType, a.Value, u.Value, e.Value, p.Value FROM Modifiers m JOIN ModifierArguments a ON a.ModifierId=m.ModifierId AND a.Name='Amount' JOIN ModifierArguments u ON u.ModifierId=m.ModifierId AND u.Name='UnitPromotionClass' JOIN ModifierArguments e ON e.ModifierId=m.ModifierId AND e.Name='EraType' JOIN RequirementSetRequirements s ON s.RequirementSetId=m.SubjectRequirementSetId JOIN RequirementArguments p ON p.RequirementId=s.RequirementId AND p.Name='PropertyName' WHERE m.ModifierId=?",
            [&id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
        )?;
        assert_eq!(
            row,
            (
                "MODIFIER_PLAYER_CITIES_ADJUST_UNIT_TAG_ERA_PRODUCTION".to_owned(),
                percent.to_string(),
                "PROMOTION_CLASS_ANTI_CAVALRY".to_owned(),
                "ERA_ANCIENT".to_owned(),
                format!("JAC_TODO134_QCAL_P{percent}_ON"),
            )
        );
    }
    for percent in [100, 1, 15] {
        let id = format!("JAC_TODO134_QCAL_UNIT_P{percent}");
        let row: (String, String, String, String) = db.query_row(
            "SELECT m.ModifierType, a.Value, u.Value, p.Value FROM Modifiers m JOIN ModifierArguments a ON a.ModifierId=m.ModifierId AND a.Name='Amount' JOIN ModifierArguments u ON u.ModifierId=m.ModifierId AND u.Name='UnitType' JOIN RequirementSetRequirements s ON s.RequirementSetId=m.SubjectRequirementSetId JOIN RequirementArguments p ON p.RequirementId=s.RequirementId AND p.Name='PropertyName' WHERE m.ModifierId=?",
            [&id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))
        )?;
        assert_eq!(
            row,
            (
                "MODIFIER_PLAYER_CITIES_ADJUST_UNIT_PRODUCTION".to_owned(),
                percent.to_string(),
                "UNIT_SPEARMAN".to_owned(),
                format!("JAC_TODO134_QCAL_P{percent}_ON"),
            )
        );
    }
    for (id, amount, kind, property) in [
        (
            "JAC_TODO134_QCAL_INPUT_512",
            "512",
            "MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_CHANGE",
            "JAC_TODO134_QCAL_INPUT_512_ON",
        ),
        (
            "JAC_TODO134_QCAL_GROWTH",
            "-1000",
            "MODIFIER_PLAYER_CITIES_ADJUST_CITY_GROWTH",
            "JAC_TODO134_QCAL_GROWTH_ON",
        ),
        (
            "JAC_TODO134_QCAL_P100",
            "100",
            "MODIFIER_PLAYER_CITIES_ADJUST_MILITARY_UNITS_PRODUCTION",
            "JAC_TODO134_QCAL_P100_ON",
        ),
        (
            "JAC_TODO134_QCAL_P1",
            "1",
            "MODIFIER_PLAYER_CITIES_ADJUST_MILITARY_UNITS_PRODUCTION",
            "JAC_TODO134_QCAL_P1_ON",
        ),
        (
            "JAC_TODO134_QCAL_P15",
            "15",
            "MODIFIER_PLAYER_CITIES_ADJUST_MILITARY_UNITS_PRODUCTION",
            "JAC_TODO134_QCAL_P15_ON",
        ),
        (
            "JAC_TODO134_QCAL_P25",
            "25",
            "MODIFIER_PLAYER_CITIES_ADJUST_MILITARY_UNITS_PRODUCTION",
            "JAC_TODO134_QCAL_P25_ON",
        ),
    ] {
        let (actual_kind, actual_amount, actual_property): (String, String, String) = db.query_row(
            "SELECT m.ModifierType, a.Value, p.Value FROM Modifiers m JOIN ModifierArguments a ON a.ModifierId=m.ModifierId AND a.Name='Amount' JOIN RequirementArguments p ON p.RequirementId=m.ModifierId||'_REQ' AND p.Name='PropertyName' WHERE m.ModifierId=?",
            [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        )?;
        assert_eq!(
            (
                actual_kind.as_str(),
                actual_amount.as_str(),
                actual_property.as_str()
            ),
            (kind, amount, property)
        );
    }
    for (id, kind, amount) in [
        (
            "JAC_TODO134_HEADROOM_PRODUCTION",
            "MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_CHANGE",
            "4096",
        ),
        (
            "JAC_TODO134_HEADROOM_GROWTH",
            "MODIFIER_PLAYER_CITIES_ADJUST_CITY_GROWTH",
            "-1000",
        ),
    ] {
        assert_eq!(
            db.query_row(
                "SELECT ModifierType FROM Modifiers WHERE ModifierId=?",
                [id],
                |r| r.get::<_, String>(0)
            )?,
            kind
        );
        assert_eq!(
            db.query_row(
                "SELECT Value FROM ModifierArguments WHERE ModifierId=? AND Name='Amount'",
                [id],
                |r| r.get::<_, String>(0)
            )?,
            amount
        );
        assert_eq!(db.query_row("SELECT Value FROM RequirementArguments WHERE RequirementId='JAC_TODO134_HEADROOM_REQ' AND Name='PropertyName'",[],|r|r.get::<_,String>(0))?,"JAC_TODO134_HEADROOM_ON");
        assert_eq!(
            db.query_row(
                "SELECT SubjectRequirementSetId FROM Modifiers WHERE ModifierId=?",
                [id],
                |r| r.get::<_, String>(0)
            )?,
            "JAC_TODO134_HEADROOM_SET"
        );
    }
    for kind in ["PRODUCTION", "GOLD", "SCIENCE", "CULTURE", "FAITH", "FOOD"] {
        for (mode, amount, suffix) in [
            ("BASE", "6", "CHANGE"),
            ("CAL", "100", "MODIFIER"),
            ("FINE", "1", "MODIFIER"),
            ("FINE_TWO", "2", "MODIFIER"),
            ("SHIFT", "1", "CHANGE"),
        ] {
            let id = format!("JAC_TODO134_{mode}_{kind}");
            let modifier: String = db.query_row(
                "SELECT ModifierType FROM Modifiers WHERE ModifierId=?",
                [&id],
                |r| r.get(0),
            )?;
            assert_eq!(
                modifier,
                format!("MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_{suffix}")
            );
            let value: String = db.query_row(
                "SELECT Value FROM ModifierArguments WHERE ModifierId=? AND Name='Amount'",
                [&id],
                |r| r.get(0),
            )?;
            assert_eq!(value, amount);
            let yield_type: String = db.query_row(
                "SELECT Value FROM ModifierArguments WHERE ModifierId=? AND Name='YieldType'",
                [&id],
                |r| r.get(0),
            )?;
            assert_eq!(yield_type, format!("YIELD_{kind}"));
            if mode == "FINE_TWO" {
                let property: String = db.query_row(
                    "SELECT Value FROM RequirementArguments WHERE RequirementId=? AND Name='PropertyName'",
                    [format!("{id}_REQ")],
                    |r| r.get(0),
                )?;
                assert_eq!(property, "JAC_TODO134_FINE_TWO_ON");
            }
        }
    }
    assert!(
        !sql.contains("TraitModifiers"),
        "diagnostic must not attach automatically"
    );
    let meta = &files[&PathBuf::from(format!("{METER_DIR}/JAC_PoC_OathMeter.modinfo"))];
    let document = roxmltree::Document::parse(meta)?;
    assert_eq!(document.root_element().attribute("id"), Some(METER_ID));
    assert!(document
        .descendants()
        .any(|n| n.has_tag_name("RuleSetInUse") && n.text() == Some("RULESET_JAC_BANSEIKI")));
    assert!(document
        .descendants()
        .any(|n| n.has_tag_name("UpdateDatabase")
            && n.attribute("criteria") == Some("JAC_OathMeter_Banseiki")));
    assert!(!document
        .descendants()
        .any(|n| n.has_tag_name("AddGameplayScripts")));
    let criteria = document
        .descendants()
        .find(|n| n.has_tag_name("Criteria") && n.attribute("id") == Some("JAC_OathMeter_Headroom"))
        .context("headroom criteria")?;
    assert!(criteria
        .descendants()
        .any(|n| n.has_tag_name("RuleSetInUse") && n.text() == Some("RULESET_JAC_BANSEIKI")));
    assert!(criteria.descendants().any(
        |n| n.has_tag_name("ConfigurationId") && n.text() == Some("JAC_TODO134_QUEUE_HEADROOM")
    ));
    let headroom = &files[&PathBuf::from(format!("{METER_DIR}/Data/Headroom.sql"))];
    let config = &files[&PathBuf::from(format!("{METER_DIR}/Config/Headroom.xml"))];
    let config = roxmltree::Document::parse(config)?;
    let row = config
        .descendants()
        .find(|n| n.attribute("ConfigurationId") == Some("JAC_TODO134_QUEUE_HEADROOM"))
        .context("headroom config row")?;
    assert_eq!(row.attribute("DefaultValue"), Some("0"));
    assert_eq!(row.attribute("Key2"), Some("RULESET_JAC_BANSEIKI"));
    assert_eq!(row.attribute("Visible"), Some("0"));
    let calibration = config
        .descendants()
        .find(|n| n.attribute("ConfigurationId") == Some("JAC_TODO134_QUEUE_CALIBRATION"))
        .context("calibration config row")?;
    assert_eq!(calibration.attribute("DefaultValue"), Some("0"));
    assert_eq!(calibration.attribute("Visible"), Some("0"));
    assert_eq!(calibration.attribute("Key2"), Some("RULESET_JAC_BANSEIKI"));
    let tag_prerequisite = config
        .descendants()
        .find(|n| n.attribute("ConfigurationId") == Some("JAC_TODO134_TAG_ERA_PREREQ"))
        .context("tag prerequisite config")?;
    assert_eq!(tag_prerequisite.attribute("DefaultValue"), Some("0"));
    assert_eq!(tag_prerequisite.attribute("Visible"), Some("0"));
    assert_eq!(
        tag_prerequisite.attribute("Key2"),
        Some("RULESET_JAC_BANSEIKI")
    );
    let tag_criteria = document
        .descendants()
        .find(|n| {
            n.has_tag_name("Criteria") && n.attribute("id") == Some("JAC_OathMeter_TagPrerequisite")
        })
        .context("tag prerequisite criteria")?;
    assert!(tag_criteria
        .descendants()
        .any(|n| n.has_tag_name("RuleSetInUse") && n.text() == Some("RULESET_JAC_BANSEIKI")));
    assert!(tag_criteria.descendants().any(
        |n| n.has_tag_name("ConfigurationId") && n.text() == Some("JAC_TODO134_TAG_ERA_PREREQ")
    ));
    assert!(document
        .descendants()
        .any(|n| n.has_tag_name("UpdateDatabase")
            && n.attribute("id") == Some("JAC_OathMeter_TagPrerequisite_Data")
            && n.attribute("criteria") == Some("JAC_OathMeter_TagPrerequisite")));
    let calibration_criteria = document
        .descendants()
        .find(|n| {
            n.has_tag_name("Criteria") && n.attribute("id") == Some("JAC_OathMeter_Calibration")
        })
        .context("calibration criteria")?;
    assert!(calibration_criteria
        .descendants()
        .any(|n| n.has_tag_name("ConfigurationId")
            && n.text() == Some("JAC_TODO134_QUEUE_CALIBRATION")));
    assert!(document
        .descendants()
        .any(|n| n.has_tag_name("UpdateDatabase")
            && n.attribute("id") == Some("JAC_OathMeter_Calibration_Data")
            && n.attribute("criteria") == Some("JAC_OathMeter_Calibration")));
    assert!(document
        .descendants()
        .any(|n| n.has_tag_name("File") && n.text() == Some("Data/QueueCalibration.sql")));
    for mode in [
        "SupportsLANMultiplayer",
        "SupportsInternetMultiplayer",
        "SupportsHotSeat",
        "SupportsPlayByCloud",
    ] {
        assert_eq!(row.attribute(mode), Some("0"));
        assert_eq!(calibration.attribute(mode), Some("0"));
        assert_eq!(tag_prerequisite.attribute(mode), Some("0"));
    }
    db.execute_batch("CREATE TABLE Units(UnitType TEXT PRIMARY KEY,Cost INTEGER); CREATE TABLE Buildings(BuildingType TEXT PRIMARY KEY,Cost INTEGER); INSERT INTO Units VALUES('UNIT_SPEARMAN',65),('UNIT_BUILDER',50),('UNIT_GALLEY',65),('UNIT_WARRIOR',40); INSERT INTO Buildings VALUES('BUILDING_WALLS',80),('BUILDING_CASTLE',100),('BUILDING_STAR_FORT',300),('BUILDING_MONUMENT',60),('BUILDING_GRANARY',65);")?;
    db.execute_batch(headroom)?;
    for unit in ["UNIT_SPEARMAN", "UNIT_BUILDER", "UNIT_GALLEY"] {
        assert_eq!(
            db.query_row("SELECT Cost FROM Units WHERE UnitType=?", [unit], |r| r
                .get::<_, i64>(0))?,
            32768
        );
    }
    for building in [
        "BUILDING_WALLS",
        "BUILDING_CASTLE",
        "BUILDING_STAR_FORT",
        "BUILDING_MONUMENT",
    ] {
        assert_eq!(
            db.query_row(
                "SELECT Cost FROM Buildings WHERE BuildingType=?",
                [building],
                |r| r.get::<_, i64>(0)
            )?,
            32768
        );
    }
    assert_eq!(
        db.query_row(
            "SELECT Cost FROM Units WHERE UnitType='UNIT_SPEARMAN'",
            [],
            |r| r.get::<_, i64>(0)
        )?,
        32768
    );
    assert_eq!(
        db.query_row(
            "SELECT Cost FROM Units WHERE UnitType='UNIT_WARRIOR'",
            [],
            |r| r.get::<_, i64>(0)
        )?,
        40
    );
    assert_eq!(
        db.query_row(
            "SELECT Cost FROM Buildings WHERE BuildingType='BUILDING_GRANARY'",
            [],
            |r| r.get::<_, i64>(0)
        )?,
        65
    );
    let calibration_sql = &files[&PathBuf::from(format!("{METER_DIR}/Data/QueueCalibration.sql"))];
    db.execute_batch(calibration_sql)?;
    assert_eq!(
        db.query_row(
            "SELECT Cost FROM Units WHERE UnitType='UNIT_SPEARMAN'",
            [],
            |r| r.get::<_, i64>(0)
        )?,
        131072
    );
    assert_eq!(
        db.query_row(
            "SELECT Cost FROM Units WHERE UnitType='UNIT_BUILDER'",
            [],
            |r| r.get::<_, i64>(0)
        )?,
        32768
    );
    db.execute_batch("ALTER TABLE Units ADD COLUMN PrereqTech TEXT; ALTER TABLE Units ADD COLUMN PrereqCivic TEXT; UPDATE Units SET PrereqTech='TECH_OTHER',PrereqCivic='CIVIC_OTHER' WHERE UnitType='UNIT_WARRIOR';")?;
    let gated = &files[&PathBuf::from(format!("{METER_DIR}/Data/TagEraPrerequisite.sql"))];
    db.execute_batch(gated)?;
    assert_eq!(
        db.query_row(
            "SELECT PrereqTech,PrereqCivic,Cost FROM Units WHERE UnitType='UNIT_SPEARMAN'",
            [],
            |r| Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, i64>(2)?
            ))
        )?,
        ("TECH_JAC_SENGOKU_IRON_FORGING".to_owned(), None, 131072)
    );
    assert_eq!(
        db.query_row(
            "SELECT PrereqTech,PrereqCivic,Cost FROM Units WHERE UnitType='UNIT_WARRIOR'",
            [],
            |r| Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?
            ))
        )?,
        ("TECH_OTHER".to_owned(), "CIVIC_OTHER".to_owned(), 40)
    );
    Ok(())
}

const FINAL_NATIVE_FIXTURE: &str = r#"
local oath='JAC_OATH_ODA_TENKA_FUBU'
local ledger={reign=1,houses={p0={civ='CIVILIZATION_JAC_ODA',oathChosen=1,oaths={[oath]=true}}}}
Game={GetProperty=function(_,key) assert(key=='JAC_AGES_LEDGER'); return ledger end}
GameInfo={JAC_SengokuOaths=function()
 local i=0; return function() i=i+1; if i<=66 then return {OathType=i==1 and oath or ('OTHER_'..i)} end end
end}
local plots={}; local majors={}; local own={}; local foreign={}
Players={}
for owner=0,21 do
 local playerID=owner; local cities={}
 local n=playerID>=1 and playerID<=12 and 3 or 2
 for index=1,n do
  local cityID=index; local plotID=playerID*100+cityID
  local city={GetID=function() return cityID end,GetOwner=function() return playerID end,
   GetX=function() return plotID end,GetY=function() return 0 end}
  plots[plotID]={GetIndex=function() return plotID end,GetOwner=function() return playerID end,
   GetProperty=function(_,name) return playerID==0 and name==oath..'_ON' and 1 or 0 end}
  cities[#cities+1]=city
  local records=playerID==0 and own or foreign
  records[#records+1]={id=cityID,plot=plotID,owner=playerID}
 end
 local collection={Members=function() return ipairs(cities) end}
 local player={GetID=function() return playerID end,GetCities=function() return collection end}
 Players[playerID]=player; majors[#majors+1]=player
end
PlayerManager={GetAliveMajors=function() return majors end}
Map={GetPlot=function(x,y) assert(y==0); return plots[x] end}
JAC_OATH_AUDIT=JAC_OATH_AUDIT or {}
JAC_OATH_AUDIT.baseline={own=own,foreign=foreign}
JAC_OATH_AUDIT.yield_growth_observed=true
"#;

#[test]
fn final_generated_report_and_gate_require_true_quantity_and_every_production_result() -> Result<()>
{
    let final_code = generated_code(&oda()?, "saved UI choice not restored after diagnostics")?;
    let lua = Lua::new();
    for quantity in ["true", "false", "nil"] {
        lua.load("JAC_OATH_AUDIT=nil").exec()?;
        lua.load(FINAL_NATIVE_FIXTURE).exec()?;
        lua.load(format!(r#"
JAC_OATH_AUDIT.quantities_verified={quantity}
local function proof(amount) return {{quantity_verified=true,expected_percent=amount,sample_count=3,off_first={{}},on={{}},off_last={{}}}} end
JAC_OATH_AUDIT.production={{UNIT_SPEARMAN=proof(15),UNIT_BUILDER=proof(0)}}
"#)).exec()?;
        let result = decode(&lua, &final_code)?;
        assert_eq!(result["accepted"], quantity == "true");
        assert_eq!(result["production_results_complete"], true);
        if quantity == "true" {
            lua.load(ACCEPTANCE_GATE).exec()?;
        } else {
            assert!(lua.load(ACCEPTANCE_GATE).exec().is_err());
        }
    }
    for mutation in [
        "a.production.UNIT_BUILDER=nil",
        "a.production.UNIT_BUILDER.quantity_verified=false",
        "a.production.UNIT_BUILDER.quantity_verified=nil",
        "a.production.UNIT_SPEARMAN.expected_percent=1",
        "a.production.UNIT_SPEARMAN.off_last=nil",
        "a.production.UNIT_SPEARMAN.sample_count=nil",
        "a.production.UNIT_SPEARMAN.sample_count=9",
        "a.production.UNIT_SPEARMAN.sample_count=2",
        "a.production={}; a.production_quantities_verified=true",
    ] {
        lua.load("JAC_OATH_AUDIT=nil").exec()?;
        lua.load(FINAL_NATIVE_FIXTURE).exec()?;
        lua.load("JAC_OATH_AUDIT.quantities_verified=true; local function p(amount) return {quantity_verified=true,expected_percent=amount,sample_count=3,off_first={},on={},off_last={}} end; JAC_OATH_AUDIT.production={UNIT_SPEARMAN=p(15),UNIT_BUILDER=p(0)}").exec()?;
        lua.load(format!("local a=JAC_OATH_AUDIT; {mutation}"))
            .exec()?;
        let result = decode(&lua, &final_code)?;
        assert_eq!(result["accepted"], false, "accepted {mutation}");
        assert_eq!(result["production_results_complete"], false);
        assert!(lua.load(ACCEPTANCE_GATE).exec().is_err());
    }
    Ok(())
}

#[test]
fn low_effective_yield_is_not_mistaken_for_zero_raw_yield() -> Result<()> {
    let case = growth_case();
    let compare = generated_code(&case, "local spec=")?;
    let final_code = generated_code(&case, "saved UI choice not restored after diagnostics")?;
    let lua = Lua::new();
    lua.load(CITY_OBSERVATIONS).exec()?;
    lua.load("local a=JAC_OATH_AUDIT; a.baseline.own[1].y.science=0.02; a.off.own[1].y.science=0.02; a.selected.own[1].y.science=0.12; a.restored.own[1].y.science=0.12").exec()?;
    let observed = decode(&lua, &compare)?;
    assert_eq!(observed["results"][1]["observed_delta"], 0.1);
    assert_eq!(observed["quantity_verified"], false);
    // Keep the actual comparison result while adding the native final-state fixture.
    lua.load(FINAL_NATIVE_FIXTURE).exec()?;
    let result = decode(&lua, &final_code)?;
    assert_eq!(result["accepted"], false);
    assert!(lua.load(ACCEPTANCE_GATE).exec().is_err());
    Ok(())
}

#[test]
fn zero_yield_no_growth_or_wrong_oath_flag_never_proves_effect() -> Result<()> {
    let code = generated_code(&growth_case(), "local spec=")?;
    let lua = Lua::new();
    for mutation in [
        "for _,p in ipairs({'baseline','selected','off','restored'}) do a[p].own[1].y.science=0 end",
        "for _,p in ipairs({'baseline','selected','off','restored'}) do a[p].own[1].housing=0; a[p].own[1].growth=0 end",
        "Map.GetPlotByIndex=function() return {GetOwner=function() return 0 end,GetProperty=function() return 1 end} end",
    ] {
        lua.load(CITY_OBSERVATIONS).exec()?;
        lua.load(format!("local a=JAC_OATH_AUDIT; {mutation}")).exec()?;
        assert!(decode(&lua,&code).is_err(),"accepted {mutation}");
    }
    Ok(())
}

#[test]
fn capability_diagnosis_calls_apis_and_preserves_json_without_certifying_candidates() -> Result<()>
{
    let base = Repository::checked_in().json(BASE)?;
    let scenario = capability_scenario(&base)?;
    let code = scenario["steps"]
        .as_array()
        .context("steps")?
        .last()
        .context("UI diagnosis")?["code"]
        .as_str()
        .context("code")?;
    let lua = Lua::new();
    lua.load(r#"
local c={GetID=function() return 1 end,GetYield=function() return 10 end,GetBaseYield=function() return 8 end,
 GetYieldModifier=function() error('unknown "modifier"\nAPI') end,
 GetGrowth=function() return {GetOverallGrowthModifier=function() return 1 end} end,
 GetBuildQueue=function() return setmetatable({GetUnitProgress=function() return '0' end,GetUnitCost=function() return 0/0 end},{__index=function(_,key) error('missing member '..key) end}) end}
Players={[0]={GetCities=function() return {GetCapitalCity=function() return c end} end}}
GameInfo={Yields=setmetatable({},{__index=function() return {Index=0} end}),Units={UNIT_SPEARMAN={Index=1}},Buildings={BUILDING_WALLS={Index=2}}}
"#).exec()?;
    let result = decode(&lua, code)?;
    assert_eq!(result["accepted"], false);
    let probes = result["probes"].as_array().context("probes")?;
    assert_eq!(probes.len(), 31);
    let find = |name: &str| {
        probes
            .iter()
            .find(|p| p["method"] == name)
            .expect("named probe")
    };
    assert_eq!(find("GetBaseYield")["finite_numeric"], true);
    assert_eq!(
        find("GetBaseYield")["quantity_contract"],
        "unknown_do_not_accept"
    );
    assert_eq!(find("GetYieldModifier")["call_ok"], false);
    assert!(find("GetYieldModifier")["error"]
        .as_str()
        .context("escaped error")?
        .contains("\nAPI"));
    assert_eq!(find("GetUnitProgress")["return_type"], "string");
    assert_eq!(find("GetUnitProgress")["finite_numeric"], false);
    assert_eq!(find("GetUnitCost")["value"], Value::Null);
    assert_eq!(find("GetBuildingCost")["exists"], false);
    assert!(find("GetBuildingCost")["error"]
        .as_str()
        .context("lookup diagnostic")?
        .contains("missing member"));
    Ok(())
}

#[test]
fn checked_in_scenarios_and_inventory_match_the_generator() -> Result<()> {
    let repo = Repository::checked_in();
    for (path, expected) in outputs(&repo)? {
        assert_eq!(repo.read(&path)?, expected, "stale {}", path.display());
    }
    Ok(())
}

fn copied_source() -> Result<(tempfile::TempDir, Repository)> {
    let directory = tempfile::tempdir()?;
    let copied = Repository::new(directory.path());
    let source = Repository::checked_in();
    for relative in [BASE, "documents/poc/data/sengoku-oaths.json"] {
        let path = copied.path(relative);
        std::fs::create_dir_all(path.parent().context("fixture parent")?)?;
        std::fs::write(path, source.read(relative)?)?;
    }
    Ok((directory, copied))
}

#[test]
fn incomplete_catalogues_duplicate_metrics_and_invalid_map_rosters_are_rejected() -> Result<()> {
    for mutation in 0..4 {
        let (_directory, repo) = copied_source()?;
        let mut catalogue = repo.json("documents/poc/data/sengoku-oaths.json")?;
        match mutation {
            0 => {
                catalogue["houses"][0]["oaths"]
                    .as_array_mut()
                    .context("oaths")?
                    .pop();
            }
            1 => {
                catalogue["houses"][0]["oaths"][0]["effects"][1][0] = json!("military");
            }
            2 => {
                catalogue["houses"][0]["oaths"][0]["effects"][0][0] = json!("unknown");
            }
            _ => {
                catalogue["houses"][0]["oaths"][0]["effects"][0][1] = json!(0);
            }
        }
        std::fs::write(
            repo.path("documents/poc/data/sengoku-oaths.json"),
            serde_json::to_vec(&catalogue)?,
        )?;
        assert!(
            outputs(&repo).is_err(),
            "accepted malformed catalogue case {mutation}"
        );
    }
    let (_directory, repo) = copied_source()?;
    let mut base = repo.json(BASE)?;
    base["steps"][1]["players"][1] = base["steps"][1]["players"][0].clone();
    std::fs::write(repo.path(BASE), serde_json::to_vec(&base)?)?;
    assert!(
        outputs(&repo).is_err(),
        "accepted a duplicated and missing real house"
    );
    for field in ["ruleset", "map_file", "map_size"] {
        let (_directory, repo) = copied_source()?;
        let mut base = repo.json(BASE)?;
        base["steps"][1][field] = json!("UNRELATED");
        std::fs::write(repo.path(BASE), serde_json::to_vec(&base)?)?;
        assert!(outputs(&repo).is_err(), "accepted unrelated {field}");
    }
    Ok(())
}

#[test]
fn check_rejects_stale_missing_and_extra_scenarios_without_writing_them() -> Result<()> {
    let (_directory, repo) = copied_source()?;
    cli(&repo, &[])?;
    cli(&repo, &["--check".into()])?;
    let path = repo.path(format!("{DIR}/jac_oath_oda_tenka_fubu.json"));
    std::fs::write(&path, "{\"stale\":true}\n")?;
    assert!(cli(&repo, &["--check".into()]).is_err());
    assert_eq!(std::fs::read_to_string(&path)?, "{\"stale\":true}\n");
    std::fs::remove_file(&path)?;
    assert!(cli(&repo, &["--check".into()]).is_err());
    assert!(!path.exists());
    cli(&repo, &[])?;
    let extra = repo.path(format!("{DIR}/retired.json"));
    std::fs::write(&extra, "{}")?;
    assert!(cli(&repo, &["--check".into()]).is_err());
    assert_eq!(std::fs::read_to_string(extra)?, "{}");
    Ok(())
}

#[test]
fn every_product_lua_file_compiles_with_lua51() -> Result<()> {
    let repo = Repository::checked_in();
    let lua = Lua::new();
    let mut files = BTreeSet::new();
    for entry in walkdir::WalkDir::new(repo.path("poc")) {
        let entry = entry?;
        if entry.file_type().is_file()
            && entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("lua"))
        {
            let path = entry.path().strip_prefix(&repo.root)?;
            lua.load(product_lua_for_vm(path, repo.read(path)?)?)
                .set_name(path.to_string_lossy().as_ref())
                .into_function()
                .with_context(|| format!("Lua 5.1 compile {}", path.display()))?;
            files.insert(path.to_path_buf());
        }
    }
    assert!(files.contains(&PathBuf::from(
        "poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_Oaths.lua"
    )));
    assert!(
        files.len() > 50,
        "product Lua inventory unexpectedly shrank"
    );
    eprintln!("compiled {} product Lua files with Lua 5.1; the existing AdvisorItem Havok declaration was validated and adapted", files.len());
    Ok(())
}

#[derive(Debug)]
struct CodeToken {
    text: String,
    start: usize,
    end: usize,
    literal: bool,
}

fn long_lua_end(source: &str, start: usize) -> Result<Option<usize>> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&b'[') {
        return Ok(None);
    }
    let mut cursor = start + 1;
    while bytes.get(cursor) == Some(&b'=') {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'[') {
        return Ok(None);
    }
    let close = format!("]{}]", "=".repeat(cursor - start - 1));
    let suffix = &source[cursor + 1..];
    let end = suffix
        .find(&close)
        .context("unterminated Lua long string/comment")?;
    Ok(Some(cursor + 1 + end + close.len()))
}

// Only tokens in actual code can introduce Havok declarations/constructions.
// Keep comments and strings verbatim; both Lua long-bracket levels and escaped
// quoted strings must be skipped before looking for the two native keywords.
fn lua_code_tokens(source: &str) -> Result<Vec<CodeToken>> {
    let bytes = source.as_bytes();
    let mut cursor = 0;
    let mut tokens = Vec::new();
    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if bytes.get(cursor..cursor + 2) == Some(b"--") {
            if let Some(end) = long_lua_end(source, cursor + 2)? {
                cursor = end;
            } else {
                cursor += 2;
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
            }
            continue;
        }
        if bytes[cursor] == b'\'' || bytes[cursor] == b'"' {
            let start = cursor;
            let quote = bytes[cursor];
            cursor += 1;
            let mut closed = false;
            while cursor < bytes.len() {
                if bytes[cursor] == b'\\' {
                    cursor += 2;
                } else if bytes[cursor] == quote {
                    cursor += 1;
                    closed = true;
                    break;
                } else {
                    cursor += 1;
                }
            }
            if !closed {
                bail!("unterminated quoted Lua string");
            }
            tokens.push(CodeToken {
                text: source[start..cursor].to_owned(),
                start,
                end: cursor,
                literal: true,
            });
            continue;
        }
        if let Some(end) = long_lua_end(source, cursor)? {
            tokens.push(CodeToken {
                text: source[cursor..end].to_owned(),
                start: cursor,
                end,
                literal: true,
            });
            cursor = end;
            continue;
        }
        let start = cursor;
        if bytes[cursor].is_ascii_alphabetic() || bytes[cursor] == b'_' {
            cursor += 1;
            while cursor < bytes.len()
                && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
            {
                cursor += 1;
            }
        } else {
            cursor += source[cursor..]
                .chars()
                .next()
                .context("code character")?
                .len_utf8();
        }
        tokens.push(CodeToken {
            text: source[start..cursor].to_owned(),
            start,
            end: cursor,
            literal: false,
        });
    }
    Ok(tokens)
}

// Adapt only the existing native AdvisorItem. A token parser validates the
// complete field grammar, so commented declarations and extra terminators can
// never be discarded as harmless type annotations.
fn product_lua_for_vm(path: &std::path::Path, source: String) -> Result<String> {
    if path
        != std::path::Path::new("poc/JAC_PoC_HistoricalEvents/UI/JAC_HistoricalEventsAdvisor.lua")
    {
        return Ok(source);
    }
    let tokens = lua_code_tokens(&source)?;
    let definitions = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.literal && t.text == "hstructure")
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if definitions.len() != 1
        || definitions[0] != 0
        || tokens.get(1).map(|t| t.text.as_str()) != Some("AdvisorItem")
    {
        bail!("expected the one official AdvisorItem declaration");
    }
    let mut actual = BTreeSet::new();
    let mut cursor = 2;
    while tokens.get(cursor).map(|t| t.text.as_str()) != Some("end") {
        let field = tokens
            .get(cursor..cursor + 4)
            .context("incomplete AdvisorItem field")?;
        if field.iter().any(|t| t.literal)
            || field[1].text != ":"
            || field[3].text != ";"
            || !["string", "number", "ifunction", "boolean", "table"]
                .contains(&field[2].text.as_str())
        {
            bail!("invalid Havok AdvisorItem field grammar");
        }
        if !actual.insert((field[0].text.clone(), field[2].text.clone())) {
            bail!("duplicate AdvisorItem field");
        }
        cursor += 4;
    }
    let declaration_end = tokens[cursor].end;
    if tokens.get(cursor + 1).map(|t| t.text.as_str()) == Some("end") {
        bail!("extra AdvisorItem terminator");
    }
    let expected: BTreeSet<_> = [
        ("Message", "string"),
        ("MessageAudio", "string"),
        ("Image", "string"),
        ("OptionsNum", "number"),
        ("Button1Text", "string"),
        ("Button2Text", "string"),
        ("Button1Func", "ifunction"),
        ("Button2Func", "ifunction"),
        ("CalloutHeader", "string"),
        ("CalloutBody", "string"),
        ("PlotCallback", "ifunction"),
        ("ShowPortrait", "boolean"),
        ("UITriggers", "table"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_owned(), b.to_owned()))
    .collect();
    if actual != expected {
        bail!("AdvisorItem fields do not match the official constructor contract");
    }
    let constructions = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.literal && t.text == "hmake")
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if constructions.len() != 1 {
        bail!("expected one AdvisorItem construction");
    }
    let index = constructions[0];
    if tokens.get(index + 1).map(|t| t.text.as_str()) != Some("AdvisorItem")
        || tokens.get(index + 2).map(|t| t.text.as_str()) != Some("{")
    {
        bail!("invalid AdvisorItem construction");
    }
    let mut edits = vec![
        (tokens[0].start, declaration_end),
        (tokens[index].start, tokens[index + 1].end),
    ];
    edits.sort_by_key(|(start, _)| std::cmp::Reverse(*start));
    let mut adapted = source;
    for (start, end) in edits {
        let blanks = adapted.as_bytes()[start..end]
            .iter()
            .map(|b| if *b == b'\n' || *b == b'\r' { *b } else { b' ' })
            .collect::<Vec<_>>();
        adapted.replace_range(start..end, std::str::from_utf8(&blanks)?);
    }
    Ok(adapted)
}

#[test]
fn havok_adapter_validates_fields_and_does_not_hide_unknown_syntax() -> Result<()> {
    let repo = Repository::checked_in();
    let path =
        std::path::Path::new("poc/JAC_PoC_HistoricalEvents/UI/JAC_HistoricalEventsAdvisor.lua");
    let source = repo.read(path)?;
    assert!(product_lua_for_vm(
        path,
        source.replace("hstructure AdvisorItem", "hstructure OtherItem")
    )
    .is_err());
    for literal in ["\"junk\"", "'junk'", "[=[junk]=]"] {
        assert!(product_lua_for_vm(
            path,
            source.replace("hmake AdvisorItem", &format!("hmake {literal} AdvisorItem"))
        )
        .is_err());
        assert!(product_lua_for_vm(
            path,
            source.replace(
                "hstructure AdvisorItem",
                &format!("hstructure {literal} AdvisorItem")
            )
        )
        .is_err());
    }
    assert!(product_lua_for_vm(path, source.replace(": ifunction;", ": unknown;")).is_err());
    assert!(product_lua_for_vm(
        path,
        source.replace(
            "hstructure AdvisorItem\n",
            "hstructure AdvisorItem\n error('hidden executable')\n"
        )
    )
    .is_err());
    let declaration_end = source.find("\nend\n").context("declaration end")? + 5;
    let commented = format!(
        "--[=[{}]=]\n{}",
        &source[..declaration_end],
        &source[declaration_end..]
    );
    assert!(product_lua_for_vm(path, commented).is_err());
    assert!(product_lua_for_vm(path, source.replacen("\nend\n", "\n  end\nend\n", 1)).is_err());
    let prefix = "--[==[ hstructure AdvisorItem end hmake AdvisorItem {} ]==]\n";
    let suffix="\nlocal quoted = \"hmake AdvisorItem {}\"\nlocal long = [=[hstructure AdvisorItem end]=]\n";
    let preserved = product_lua_for_vm(path, format!("{prefix}{source}{suffix}"))?;
    assert!(preserved.starts_with(prefix) && preserved.ends_with(suffix));
    assert!(Lua::new().load(preserved).into_function().is_ok());
    let adapter = product_lua_for_vm(path, source)?;
    assert!(Lua::new().load(adapter.clone()).into_function().is_ok());
    assert!(Lua::new()
        .load(format!("{adapter}\nlocal broken ="))
        .into_function()
        .is_err());
    Ok(())
}
