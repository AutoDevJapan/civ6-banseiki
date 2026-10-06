use super::calibration::{Plan, DATE, UESUGI};
use mlua::Lua;
use serde_json::Value;

// Explicit Lua shape fixtures test diagnostics, never the native dispatcher or
// actual game acceptance. Independent flags/Spent can deliberately disagree.
fn fixture(cost: u8, paid: u8, unjoined: bool) -> Lua {
    let lua = Lua::new();
    lua.load(format!(r#"
TARGET={target}; COST={cost}; SPENT={paid}; TURN=10; HUMAN=false; FAIL=''; UNKNOWN=false; ASSIGN=false
FLAGS={{}}; ROWS={{}}; PROPS={{}}; TOTAL=20; STAGE='EVOLVING'; UNIT=55
local function iter(values) return function() local n=0; return function() n=n+1; return values[n] end end end
local function members(values) return function() local n=0; return function() n=n+1; if values[n] then return n,values[n] end end end end
local defs,ps,all={{}},{{}},{{}}
for n=0,21 do
 local name=n==0 and '{DATE}' or (n==1 and '{UESUGI}' or 'L'..n)
 local c=n==0 and 2 or (n==1 and 3 or 1)
 ROWS[n+1]={{LeaderType=name,GovernorType='G'..n,BasePromotionType='B'..n,PersonType='P'..n,EvolutionCost=c}}
 defs['G'..n]={{Index=n,Hash=100+n}}
 for t=0,c-1 do local key='B'..n..(t==0 and '' or '_TITLE_'..t); local p={{Index=n*3+t,Hash=500+n*3+t}}; ps[key]=p; all[#all+1]=p end
 FLAGS[n]={{}}
end
setmetatable(ps,{{__call=iter(all)}})
GameInfo={{JAC_LeaderPersonas=iter(ROWS),Governors=defs,GovernorPromotions=ps,GreatPersonIndividuals={{[99]={{GreatPersonIndividualType='P'..TARGET}}}}}}
local target=ROWS[TARGET+1]; local prefix='JAC_PERSONA_'..target.LeaderType..'_'
if not {unjoined} then
 PROPS[prefix..'OWNER']=2; PROPS[prefix..'SOURCE']=TARGET==0 and 1 or 4
 PROPS[prefix..'EVOLUTION_COST']=COST; PROPS[prefix..'EVOLUTION_SURCHARGE']=COST-1
 for t=0,{paid}-1 do FLAGS[TARGET][TARGET*3+t]=true end
end
local gs={{}}
function gs:HasGovernor(hash) if FAIL=='has' then return nil end; return next(FLAGS[hash-100])~=nil end
function gs:GetGovernor(hash)
 local n=hash-100
 if FAIL=='get' then return nil end
 if next(FLAGS[n])==nil then return nil end
 return {{GetOwner=function() return FAIL=='owner' and 3 or 2 end,GetType=function() return n end,
 HasPromotion=function(_,p) if UNKNOWN then return nil end; return FLAGS[n][p-500]==true end}}
end
function gs:CanEverAppointGovernor() if FAIL=='ever' then return nil end; return false end
function gs:CanAppointGovernor() return FAIL=='open' end
function gs:CanAssignGovernor() return ASSIGN end
function gs:GetGovernorPoints() return TOTAL end
function gs:GetGovernorPointsSpent() return SPENT end
local p={{IsHuman=function() return HUMAN end,IsAlive=function() return true end,IsMajor=function() return true end,GetGovernors=function() return gs end}}
function p:GetCities() return {{Members=members({{{{GetID=function() return 8 end}}}}),GetCapitalCity=function() return {{GetID=function() return 8 end}} end}} end
function p:GetUnits()
 local units={{}}
 if not {unjoined} and UNIT~=nil then
  units[1]={{GetID=function() return UNIT end,GetOwner=function() return 2 end,GetGreatPerson=function() return {{IsGreatPerson=function() return true end,GetIndividual=function() return 99 end}} end}}
 end
 return {{Members=members(units)}}
end
Players={{[0]={{IsHuman=function() return true end}},[2]=p}}
PlayerConfigurations={{[1]={{GetLeaderTypeName=function() return '{DATE}' end}},[4]={{GetLeaderTypeName=function() return '{UESUGI}' end}}}}
Game={{GetLocalPlayer=function() return 0 end,GetCurrentGameTurn=function() return TURN end,
 GetProperty=function(_,key)
  if not {unjoined} and key==prefix..'STATE' then return STAGE end
  if not {unjoined} and key==prefix..'UNIT' then return UNIT end
  return PROPS[key]
 end}}
UI={{RequestPlayerOperation=function() error('diagnostic must not send') end}}
"#,target=u8::from(cost==3))).exec().unwrap();
    lua
}

fn snapshot(lua: &Lua, code: &str) -> Value {
    let text: String = lua.load(code).eval().unwrap();
    serde_json::from_str(text.strip_prefix("AI1466_SNAPSHOT ").unwrap()).unwrap()
}

#[test]
fn generated_driver_uses_only_readers_and_one_actual_turn() {
    for cost in [2, 3] {
        for paid in 0..cost {
            let plan = Plan::new("step", 2, cost, paid).unwrap();
            let value = plan.scenario();
            civ6_autotest_rs::scenario::validate_scenario(&value).unwrap();
            let steps = value["steps"].as_array().unwrap();
            assert_eq!(steps.len(), 3);
            assert_eq!(steps[1]["action"], "end_turn");
            for step in steps.iter().filter(|v| v["action"] == "ui_lua") {
                let code = step["code"].as_str().unwrap();
                for forbidden in [
                    "SetProperty(",
                    "RequestPlayerOperation(",
                    "ChangeGovernorPoints(",
                    "ContextPtr:SetUpdate",
                ] {
                    assert!(!code.contains(forbidden));
                }
                assert_eq!(step["state_name"], "InGame");
            }
        }
    }
    for args in [
        ("step", 0, 2, 1),
        ("step", 2, 3, 3),
        ("unjoined", 2, 2, 1),
        ("hold", 2, 0, 0),
    ] {
        assert!(Plan::new(args.0, args.1, args.2, args.3).is_err());
    }
}

#[test]
fn date_and_uesugi_steps_require_exact_personal_bits_and_spend() {
    for (cost, paid) in [(2, 0), (2, 1), (3, 1), (3, 2)] {
        let plan = Plan::new("step", 2, cost, paid).unwrap();
        let lua = fixture(cost, paid, false);
        let before = snapshot(&lua, &plan.first());
        lua.load(format!(
            "FLAGS[TARGET][TARGET*3+{paid}]=true; SPENT=SPENT+1; TURN=TURN+1"
        ))
        .exec()
        .unwrap();
        let after = snapshot(&lua, &plan.second());
        assert_eq!(
            after["spent"].as_u64(),
            before["spent"].as_u64().map(|v| v + 1)
        );
        assert_eq!(after["people"].as_array().unwrap().len(), 22);
        assert_eq!(after["operation_proven"], false);
    }
}

#[test]
fn wrong_spend_other_person_nil_cost_and_false_completion_are_rejected() {
    let plan = Plan::new("step", 2, 3, 1).unwrap();
    for mutation in [
        "SPENT=3; FLAGS[1][4]=true; FLAGS[1][5]=true", // two steps
        "SPENT=2; FLAGS[0][0]=true",                   // different person
        "SPENT=2",                                     // no target bit
        "FLAGS[1][4]=true",                            // no spend
        "FLAGS[1][5]=true; SPENT=2",                   // hole
        "FAIL='owner'",
        "UNKNOWN=true",
        "FAIL='has'",
        "FAIL='get'",
        "UNIT=nil",
        "STAGE='GOVERNOR'",
        "ASSIGN=true",
        "HUMAN=true",
    ] {
        let lua = fixture(3, 1, false);
        snapshot(&lua, &plan.first());
        lua.load(format!("TURN=11;{mutation}")).exec().unwrap();
        assert!(
            lua.load(plan.second()).eval::<String>().is_err(),
            "accepted {mutation}"
        );
    }
    for field in ["EVOLUTION_COST", "EVOLUTION_SURCHARGE"] {
        let lua = fixture(3, 1, false);
        lua.load(format!("PROPS['JAC_PERSONA_{UESUGI}_{field}']=nil"))
            .exec()
            .unwrap();
        assert!(lua.load(plan.first()).eval::<String>().is_err());
    }
}

#[test]
fn negative_lane_and_reload_do_not_grant_payment_or_replay_it() {
    let plan = Plan::new("unjoined", 2, 2, 0).unwrap();
    let lua = fixture(2, 0, true);
    snapshot(&lua, &plan.first());
    lua.load("TURN=11").exec().unwrap();
    assert_eq!(snapshot(&lua, &plan.second())["operation_proven"], false);
    for failure in [
        "FAIL='ever'",
        "FAIL='open'",
        "FLAGS[0][0]=true;SPENT=1",
        "TOTAL=0",
    ] {
        let lua = fixture(2, 0, true);
        lua.load(failure).exec().unwrap();
        assert!(lua.load(plan.first()).eval::<String>().is_err());
    }
    let reload = Plan::new("reload", 2, 3, 2).unwrap();
    assert_eq!(reload.scenario()["steps"].as_array().unwrap().len(), 1);
    let lua = fixture(3, 2, false);
    snapshot(&lua, &reload.first());
    let hold = Plan::new("hold", 2, 3, 2).unwrap();
    lua.load("TURN=11").exec().unwrap();
    snapshot(&lua, &hold.second());
    assert!(
        lua.load(hold.second()).eval::<String>().is_err(),
        "baseline consumed once"
    );
}

#[test]
fn saved_partial_snapshot_cannot_be_replaced_by_another_person_or_missing_bits() {
    let lua = fixture(3, 2, false);
    let snapshot = snapshot(&lua, &Plan::new("reload", 2, 3, 2).unwrap().first());
    let result = super::calibration::compare_reload(&snapshot, &snapshot).unwrap();
    assert_eq!(result["ai_payment_acceptance"], false);
    for field in ["source", "owner", "spent", "people", "unit", "remaining"] {
        let mut bad = snapshot.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(super::calibration::compare_reload(&snapshot, &bad).is_err());
        assert!(super::calibration::compare_reload(&bad, &bad).is_err());
    }
    let mut bad = snapshot.clone();
    let person = bad["people"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["leader"] == UESUGI)
        .unwrap();
    person["owned"] = serde_json::json!([3, 3]);
    assert!(super::calibration::compare_reload(&bad, &bad).is_err());
    assert_eq!(super::calibration::bundle(2).unwrap().len(), 9);
    for (_, plan) in super::calibration::bundle(2).unwrap() {
        civ6_autotest_rs::scenario::validate_scenario(&plan.scenario()).unwrap();
    }
}

#[test]
fn completed_hold_requires_full_formal_payment_finalization_and_no_repayment() {
    for cost in [2, 3] {
        let plan = Plan::new("hold", 2, cost, cost).unwrap();
        let lua = fixture(cost, cost, false);
        assert!(
            lua.load(plan.first()).eval::<String>().is_err(),
            "EVOLVING is not final completion"
        );
        lua.load("STAGE='GOVERNOR';UNIT=nil;ASSIGN=true")
            .exec()
            .unwrap();
        snapshot(&lua, &plan.first());
        lua.load("TURN=11").exec().unwrap();
        snapshot(&lua, &plan.second());
    }
}
