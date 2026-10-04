mod support_fixture;

use civ6_autotest_rs::scenario::validate_scenario;
use mlua::Lua;
use serde_json::Value;
use support_fixture::scenario;

fn calibration() -> Value {
    scenario("leader-persona-promotion-call-calibration-15km.json")
}

fn snippet<'a>(scene: &'a Value, marker: &str) -> &'a str {
    let found: Vec<_> = scene["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["code"].as_str())
        .filter(|s| s.contains(marker))
        .collect();
    assert_eq!(found.len(), 1, "nonunique phase {marker}");
    found[0]
}

fn engine() -> Lua {
    let lua = Lua::new();
    lua.load(r#"
        total=1; spent=1; LOCAL_PLAYER=0; requests=0; civicCalls=0;
        baseOwned=true; canEarn=false; active=true;
        ExposedMembers={};
        Game={GetLocalPlayer=function() return LOCAL_PLAYER end,
            GetProperty=function(_,key) assert(key=='JAC_TEST853_ADMISSION_RECEIPT'); return {spent=0} end};
        GameInfo={JAC_LeaderPersonas={LEADER_JAC_ASHINA_MORIKIYO={GovernorType='ASHINA',BasePromotionType='BASE'}},
            Governors={ASHINA={Index=4,Hash=84}},GovernorPromotions={BASE={Index=7,Hash=91}},
            Civics={TITLE={Index=12}},CivicPrereqs=function() return function() end end};
        GameInfo.JAC_PersonaTitleCivics=function() local done=false; return function()
            if not done then done=true; return {CivicType='TITLE'} end end end;
        local physical={HasPromotion=function(_,h) assert(h==91); return baseOwned end};
        local gov={CanEarnPromotion=function(_,governor,promotion) assert(governor==84 and promotion==91); return canEarn end,GetGovernorPoints=function() return total end,GetGovernorPointsSpent=function() return spent end,
            GetGovernor=function(_,h) assert(h==84); return physical end};
        local culture={HasCivic=function(_,i) assert(i==12); return civicCalls>0 end,
            GetCultureCost=function(_,i) assert(i==12); return 100 end,
            SetCivic=function(_,i,value) assert(i==12 and value==true); civicCalls=civicCalls+1; total=total+1 end};
        Players={[0]={IsHuman=function() return true end,IsTurnActive=function() return active end,
            GetGovernors=function() return gov end,GetCulture=function() return culture end}};
        PlayerOperations={PROMOTE_GOVERNOR=9,PARAM_GOVERNOR_TYPE='gov',PARAM_GOVERNOR_PROMOTION_TYPE='promotion'};
        UI={RequestPlayerOperation=function(owner,op,args)
            assert(owner==0 and op==9 and args.gov==4 and args.promotion==7);
            requests=requests+1; assert(baseOwned);
            -- The original native rejection is modeled without changing its account.
        end};
    "#).exec().unwrap();
    lua
}

#[test]
fn single_use_observers_require_a_real_process_restart_before_reuse() {
    for (first, first_stop) in [
        ("start_native_property_trace", "stop_native_property_trace"),
        (
            "start_native_joined_admission",
            "stop_native_joined_admission",
        ),
        (
            "start_native_promotion_call_trace",
            "stop_native_promotion_call_trace",
        ),
    ] {
        let second = if first == "start_native_joined_admission" {
            "start_native_property_trace"
        } else {
            first
        };
        let second_stop = if second == "start_native_property_trace" {
            "stop_native_property_trace"
        } else {
            first_stop
        };
        let steps = vec![
            serde_json::json!({"action":first,"name":"first","max_seconds":120}),
            serde_json::json!({"action":first_stop}),
            serde_json::json!({"action":second,"name":"second","max_seconds":120}),
            serde_json::json!({"action":second_stop}),
        ];
        let mut bad = serde_json::json!({"name":"single-use contract","steps":steps});
        assert!(validate_scenario(&bad).is_err(), "{first}->{second}");
        bad["steps"]
            .as_array_mut()
            .unwrap()
            .insert(2, serde_json::json!({"action":"restart_game"}));
        validate_scenario(&bad).unwrap();
    }
}

#[test]
fn calibration_keeps_private_appointment_checks_and_closes_the_previous_hook() {
    let scene = calibration();
    validate_scenario(&scene).unwrap();
    let original = scenario("leader-persona-joined-admission-cost1-15km.json");
    let prefix =
        &original["steps"].as_array().unwrap()[..original["steps"].as_array().unwrap().len() - 1];
    assert_eq!(&scene["steps"].as_array().unwrap()[..prefix.len()], prefix);
    let old_stop = scene["steps"]
        .as_array()
        .unwrap()
        .iter()
        .position(|s| s["action"] == "stop_native_joined_admission")
        .unwrap();
    let mut overlapping = scene.clone();
    overlapping["steps"][old_stop] = serde_json::json!({"action":"pause","seconds":1});
    assert!(validate_scenario(&overlapping).is_err());
}

#[test]
fn calibration_lua_uses_a_real_civic_spare_title_and_detects_an_extra_payment() {
    let scene = calibration();
    let lua = engine();
    lua.load(snippet(&scene, "one official spare title prepared"))
        .exec()
        .unwrap();
    lua.load(snippet(&scene, "one formal PROMOTE request"))
        .exec()
        .unwrap();
    lua.load(snippet(&scene, "Core official Spent remains"))
        .exec()
        .unwrap();
    lua.load(snippet(&scene, "owned-base request checked"))
        .exec()
        .unwrap();
    lua.load("assert(total==2 and spent==1 and requests==1 and civicCalls==1); spent=2")
        .exec()
        .unwrap();
    assert!(lua
        .load(snippet(&scene, "Core official Spent remains"))
        .exec()
        .is_err());
    assert!(lua
        .load(snippet(&scene, "owned-base request checked"))
        .exec()
        .is_err());
}

#[test]
fn calibration_lua_refuses_foreign_owner_missing_base_and_accepts_either_query_result() {
    let scene = calibration();
    for change in ["LOCAL_PLAYER=1", "active=false", "baseOwned=false"] {
        let lua = engine();
        lua.load(change).exec().unwrap();
        assert!(
            lua.load(snippet(&scene, "one formal PROMOTE request"))
                .exec()
                .is_err(),
            "{change}"
        );
        lua.load("assert(requests==0 and spent==1)").exec().unwrap();
    }
    for query in [false, true] {
        let lua = engine();
        lua.load(format!("canEarn={query}")).exec().unwrap();
        lua.load(snippet(&scene, "one official spare title prepared"))
            .exec()
            .unwrap();
        lua.load(snippet(&scene, "one formal PROMOTE request"))
            .exec()
            .unwrap();
        lua.load(snippet(&scene, "owned-base request checked"))
            .exec()
            .unwrap();
        lua.load("assert(requests==1 and spent==1)").exec().unwrap();
    }
}
