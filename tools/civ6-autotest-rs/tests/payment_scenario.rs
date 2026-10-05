mod support_fixture;
use civ6_autotest_rs::payment_scenario::{self, Person};
use mlua::Lua;
use serde_json::Value;
use support_fixture::{compile_lua, scenario};

fn closed_catalog() -> Lua {
    let lua = Lua::new();
    lua.load(
        r#"
FAULT=''; READS=0; PERMITTED={}; PROPS={JAC_PERSONA_ADMISSION_SCHEMA_V1=3}
Game={GetProperty=function(_,key) return PROPS[key] end}
local rows,defs,ids={},{},{}
for n=1,22 do
 rows[n]={LeaderType='L'..n,GovernorType='G'..n}; defs['G'..n]={Index=n+9}; ids[n]=n-1
end
ROWS=rows; DEFS=defs; IDS=ids
GameInfo={Governors=defs,JAC_LeaderPersonas=function()
 local n=0; return function() n=n+1; return rows[n] end
end}
PlayerManager={GetAliveMajorIDs=function() return ids end}
Players={}
for id=0,21 do
 local owner=id; local reset=false; local closed,acked={},{}
 Players[id]={GetProperty=function(_,key)
  READS=READS+1
  if key==902000+owner then
   assert(not reset); reset=true
   if FAULT=='reset' then return nil end
   return true
  end
  assert(reset,'natural Core receiver was not bound')
  if key>=903010 and key<=903031 then closed[key-903000]=true; return true end
  if key>=901010 and key<=901031 then
   assert(closed[key-901000],'close was not read before ACK'); acked[key-901000]=true
   return FAULT=='allow' and true or false
  end
  if key>=900010 and key<=900031 then
   assert(acked[key-900000],'typed false was not read before manifest')
   if FAULT=='manifest' then return nil end
   PERMITTED[owner..':'..(key-900000)]=false
   return true
  end
  error('unexpected property')
 end,SetProperty=function() error('diagnostic must not publish any property') end}
end
"#,
    )
    .exec()
    .unwrap();
    lua
}

#[test]
fn initial_core_catalog_is_observed_closed_before_the_first_native_query() {
    let lua = closed_catalog();
    let result: String = lua
        .load(payment_scenario::READ_CLOSED_ADMISSION)
        .eval()
        .unwrap();
    assert!(result.contains("484 existing closed Core qualifications"));
    assert_eq!(lua.globals().get::<usize>("READS").unwrap(), 1474);
    lua.load("local n=0; for _,allowed in pairs(PERMITTED) do assert(allowed==false); n=n+1 end; assert(n==484)").exec().unwrap();
    for cost in [2, 3] {
        let data = payment_scenario::build(&scenario("leader-persona-15km.json"), cost).unwrap();
        let steps = data["steps"].as_array().unwrap();
        let installed = steps
            .iter()
            .position(|s| s["action"] == "start_native_production_admission")
            .unwrap();
        assert_eq!(steps[installed + 1]["state_name"], "GameCore_Tuner");
        assert_eq!(
            steps[installed + 1]["code"],
            payment_scenario::READ_CLOSED_ADMISSION
        );
        assert!(steps[installed + 2]["code"]
            .as_str()
            .unwrap()
            .contains("CanEverAppointGovernor"));
    }
}

#[test]
fn catalog_bootstrap_rejects_stale_permissions_missing_owners_and_untyped_records() {
    for invalid in [
        "FAULT='reset'",
        "FAULT='allow'",
        "FAULT='manifest'",
        "Players[21]=nil",
        "IDS[22]=nil",
        "IDS[22]=IDS[21]",
        "DEFS.G22.Index=DEFS.G21.Index",
        "ROWS[22]=nil",
        "PROPS.JAC_PERSONA_ADMISSION_SCHEMA_V1=2",
        "PROPS.JAC_PERSONA_L1_OWNER=0",
    ] {
        let lua = closed_catalog();
        lua.load(invalid).exec().unwrap();
        assert!(
            lua.load(payment_scenario::READ_CLOSED_ADMISSION)
                .eval::<String>()
                .is_err(),
            "{invalid}"
        );
    }
}

fn model(p: Person, paid: u8, finalized: bool) -> Lua {
    let lua = Lua::new();
    lua.load(format!(r#"
LEADER='{leader}'; COST={cost}; PAID={paid}; FINALIZED={finalized}
local prefix='JAC_PERSONA_'..LEADER..'_'
PROPS={{
 JAC_TEST1720_PAYMENT_BASELINE={{owner=0,source={source},leader=LEADER,cost=COST,spent=0,total=3,turn=1}},
 [prefix..'OWNER']=0,[prefix..'SOURCE']={source},[prefix..'EVOLUTION_COST']=COST,
 [prefix..'EVOLUTION_SURCHARGE']=COST-1,[prefix..'STATE']=FINALIZED and 'GOVERNOR' or 'EVOLVING',
 [prefix..'UNIT']=not FINALIZED and 42 or nil
}}
Game={{GetProperty=function(_,k) return PROPS[k] end,GetLocalPlayer=function() return 0 end}}
PlayerConfigurations={{[{source}]={{GetLeaderTypeName=function() return LEADER end}}}}
local row={{GovernorType='PERSON',PersonType='INDIVIDUAL',BasePromotionType='BASE',EvolutionCost=COST}}
local promotions={{{{Index=3,Hash=1003}},{{Index=7,Hash=1007}},{{Index=8,Hash=1008}},{{Index=71,Hash=1071}}}}
local all={{BASE=promotions[1],BASE_TITLE_1=promotions[2],EXTRA=promotions[4]}}
if COST==3 then all.BASE_TITLE_2=promotions[3] end
GameInfo={{JAC_LeaderPersonas={{[LEADER]=row}},Governors={{PERSON={{Index=9,Hash=109}}}},
 GreatPersonIndividuals={{[20]={{GreatPersonIndividualType='INDIVIDUAL'}}}},
 GovernorPromotions=setmetatable(all,{{__call=function()
  local list=COST==3 and promotions or {{promotions[1],promotions[2],promotions[4]}}
  local n=0; return function() n=n+1; return list[n] end
 end}})
}}
OWNED={{[1003]=true,[1007]=PAID>=2,[1008]=PAID>=3,[1071]=true}}
SPENT=PAID; TOTAL=3; GOVTYPE=9; UNITOWNER=0; UNITID=42; MISSING=false; DUPLICATE=false
local gov={{GetType=function() return GOVTYPE end,HasPromotion=function(_,hash) return OWNED[hash]==true end}}
local gs={{GetGovernorPoints=function() return TOTAL end,GetGovernorPointsSpent=function() return SPENT end,
 GetGovernor=function(_,hash) assert(hash==109); return gov end,CanAssignGovernor=function() return true end}}
local gp={{IsGreatPerson=function() return true end,GetIndividual=function() return 20 end}}
local unit={{GetGreatPerson=function() return gp end,GetOwner=function() return UNITOWNER end,GetID=function() return UNITID end}}
local units={{Members=function()
 local n=0; return function() n=n+1; if not FINALIZED and not MISSING and (n==1 or (n==2 and DUPLICATE)) then return n,unit end end
end}}
local city={{GetID=function() return 5 end,GetX=function() return 80 end,GetY=function() return 21 end}}
Players={{[0]={{IsHuman=function() return true end,GetGovernors=function() return gs end,
 GetUnits=function() return units end,GetCities=function() return {{GetCapitalCity=function() return city end}} end}}}}
"#,leader=p.leader,source=p.source,cost=p.cost)).exec().unwrap();
    lua
}

#[test]
fn two_real_people_compile_in_lua51_and_use_distinct_normal_handlers_once() {
    let template = scenario("leader-persona-15km.json");
    for cost in [2, 3] {
        let data = payment_scenario::build(&template, cost).unwrap();
        let steps = data["steps"].as_array().unwrap();
        let lua = Lua::new();
        let mut appoint = 0;
        let mut promote = 0;
        let mut join = 0;
        let mut evolve = 0;
        let mut finalize = 0;
        for step in steps {
            if let Some(code) = step["code"]
                .as_str()
                .or_else(|| step["condition"]["code"].as_str())
            {
                compile_lua(&lua, code);
                if code.contains("PlayerOperations.APPOINT_GOVERNOR,") {
                    appoint += 1;
                }
                if code.contains("PlayerOperations.PROMOTE_GOVERNOR,") {
                    promote += 1;
                }
                if code.contains("OnStart='JAC_LeaderPersonaJoin'") {
                    join += 1;
                }
                if code.contains("OnStart='JAC_LeaderPersonaEvolve'") {
                    evolve += 1;
                }
                if code.contains("OnStart='JAC_LeaderPersonaFinalize'") {
                    finalize += 1;
                }
                assert!(
                    !code.contains("ChangeGovernorPoints") && !code.contains("SetGovernorPoints")
                );
            }
        }
        assert_eq!(
            (appoint, promote, join, evolve, finalize),
            (1, cost - 1, 1, 1, 1)
        );
        assert!(steps
            .iter()
            .all(|s| !matches!(s["action"].as_str(), Some("end_turn" | "autoplay"))));
    }
}

#[test]
fn partial_save_keeps_the_appointed_person_and_does_not_replay_appointment() {
    for cost in [2, 3] {
        let data = payment_scenario::build(&scenario("leader-persona-15km.json"), cost).unwrap();
        let steps = data["steps"].as_array().unwrap();
        let save = steps.iter().position(|s| s["action"] == "save").unwrap();
        let load = steps.iter().position(|s| s["action"] == "load").unwrap();
        let stop = steps
            .iter()
            .position(|s| s["action"] == "stop_native_production_admission")
            .unwrap();
        assert!(stop < save && save < load);
        assert_eq!(steps[save]["checkpoint"], true);
        assert_eq!(steps[save]["name"], steps[load]["name"]);
        assert!(steps[load..].iter().all(|s| !s["code"]
            .as_str()
            .is_some_and(|c| c.contains("APPOINT_GOVERNOR")
                || c.contains("JAC_LeaderPersonaJoin'")
                || c.contains("JAC_LeaderPersonaEvolve'"))));
        let read =
            payment_scenario::snapshot_code(payment_scenario::person(cost).unwrap(), 1, false);
        assert_eq!(steps[save - 2]["code"], read);
        assert_eq!(steps[load + 2]["code"], read);
    }
}

#[test]
fn native_observation_contains_the_same_capital_read_and_fully_paid_assignment() {
    for cost in [2, 3] {
        let data = payment_scenario::build(&scenario("leader-persona-15km.json"), cost).unwrap();
        let steps = data["steps"].as_array().unwrap();
        let start = steps
            .iter()
            .position(|s| s["action"] == "start_native_assign_calibration")
            .unwrap();
        let stop = steps
            .iter()
            .position(|s| s["action"] == "stop_native_assign_calibration")
            .unwrap();
        assert_eq!(stop - start, 4);
        assert!(steps[start + 1]["code"]
            .as_str()
            .unwrap()
            .contains("GetCapitalCity()"));
        assert_eq!(steps[start + 2]["state_name"], "JAC_LeaderPersona_View");
        assert!(steps[start + 2]["code"]
            .as_str()
            .unwrap()
            .contains("GetCapitalCity()"));
        assert!(steps[start + 1..stop].iter().all(|s| !matches!(
            s["action"].as_str(),
            Some("save" | "load" | "restart_game" | "end_turn")
        )));
    }
}

#[test]
fn every_partial_payment_requires_the_same_persons_promotion_prefix() {
    for cost in [2, 3] {
        let p = payment_scenario::person(cost).unwrap();
        for paid in 1..=cost {
            let lua = model(p, paid, false);
            let code = payment_scenario::snapshot_code(p, paid, false);
            let actual: String = lua.load(&code).eval().unwrap();
            assert!(actual.contains(&format!("cost={cost} paid={paid}")));
            lua.load("OWNED[1003]=false").exec().unwrap();
            assert!(lua.load(&code).eval::<String>().is_err());
        }
    }
}

#[test]
fn total_spending_without_this_persons_payment_or_with_wrong_identity_is_rejected() {
    let p = payment_scenario::person(3).unwrap();
    let code = payment_scenario::snapshot_code(p, 3, false);
    for mutation in [
        "OWNED[1007]=false",
        "OWNED[1008]=false",
        "GOVTYPE=10",
        "UNITOWNER=1",
        "UNITID=43",
        "SPENT=4",
        "TOTAL=4",
        "MISSING=true",
        "DUPLICATE=true",
        "PROPS['JAC_PERSONA_'..LEADER..'_OWNER']=1",
        "PROPS.JAC_TEST1720_PAYMENT_BASELINE.source=1",
    ] {
        let lua = model(p, 3, false);
        lua.load(mutation).exec().unwrap();
        assert!(lua.load(&code).eval::<String>().is_err(), "{mutation}");
    }
    let lua = model(p, 2, false);
    lua.load("OWNED[1007]=false; OWNED[1008]=true")
        .exec()
        .unwrap();
    assert!(lua
        .load(payment_scenario::snapshot_code(p, 2, false))
        .eval::<String>()
        .is_err());
}

#[test]
fn a_finalized_unpaid_person_cannot_supply_a_partial_payment_receipt() {
    for cost in [2, 3] {
        let p = payment_scenario::person(cost).unwrap();
        let lua = model(p, 1, true);
        let err = lua
            .load(payment_scenario::snapshot_code(p, 1, false))
            .eval::<String>()
            .unwrap_err()
            .to_string();
        assert!(err.contains("finalized before all payments"));
    }
}

#[test]
fn final_receipt_keeps_unrelated_native_promotions_and_runtime_indices() {
    for cost in [2, 3] {
        let p = payment_scenario::person(cost).unwrap();
        let lua = model(p, cost, true);
        let result: String = lua
            .load(payment_scenario::snapshot_code(p, cost, true))
            .eval()
            .unwrap();
        let receipt: Value =
            serde_json::from_str(result.strip_prefix("PERSONA_PAYMENT1595 ").unwrap()).unwrap();
        let titles: Vec<u32> = if cost == 2 { vec![7] } else { vec![7, 8] };
        let owned: Vec<u32> = if cost == 2 {
            vec![3, 7, 71]
        } else {
            vec![3, 7, 8, 71]
        };
        assert_eq!(receipt["titles"], serde_json::json!(titles));
        assert_eq!(receipt["owned"], serde_json::json!(owned));
        assert_eq!(
            (receipt["cost"].as_u64(), receipt["paid"].as_u64()),
            (Some(cost.into()), Some(cost.into()))
        );
        assert_eq!(
            (
                receipt["governor"].as_u64(),
                receipt["count"].as_u64(),
                receipt["city"].as_u64()
            ),
            (Some(9), Some(72), Some(5))
        );
    }
}

#[test]
fn changed_setup_and_unrepresented_costs_do_not_generate_a_probe() {
    let mut data = scenario("leader-persona-15km.json");
    assert!(payment_scenario::build(&data, 1).is_err());
    assert!(payment_scenario::build(&data, 4).is_err());
    data["steps"][0]["action"] = serde_json::json!("load");
    assert!(payment_scenario::build(&data, 2).is_err());
}

#[test]
fn assignment_subject_uses_the_actual_person_and_rejects_bad_owner_or_index() {
    use civ6_autotest_rs::{
        native_bridge::{assignment_subject_code, AssignmentSubject},
        scenario::validate_scenario,
    };
    let p = payment_scenario::person(2).unwrap();
    let code = assignment_subject_code(0, p.leader);
    let lua = model(p, 2, true);
    let raw: String = lua.load(&code).eval().unwrap();
    assert_eq!(
        AssignmentSubject::parse(&raw).unwrap(),
        AssignmentSubject {
            owner: 0,
            governor: 9
        }
    );
    lua.load("GOVTYPE=10").exec().unwrap();
    assert!(lua.load(&code).eval::<String>().is_err());
    for bad in ["0|256", "64|9", "-1|9", "0|1.5", "0|9|1", "9", "0|"] {
        assert!(AssignmentSubject::parse(bad).is_err(), "{bad}");
    }
    let make = |fields: Value| {
        let mut start = serde_json::json!({"action":"start_native_assign_calibration","name":"normal","max_seconds":30});
        start
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        serde_json::json!({"name":"subject contract","steps":[start,{"action":"stop_native_assign_calibration"}]})
    };
    validate_scenario(&make(serde_json::json!({"owner":0,"leader":p.leader}))).unwrap();
    for fields in [
        serde_json::json!({"owner":0}),
        serde_json::json!({"leader":p.leader}),
        serde_json::json!({"owner":64,"leader":p.leader}),
        serde_json::json!({"owner":0,"leader":"bad|code"}),
    ] {
        assert!(validate_scenario(&make(fields)).is_err());
    }
}
