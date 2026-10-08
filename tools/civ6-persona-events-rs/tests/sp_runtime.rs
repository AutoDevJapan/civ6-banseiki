use civ6_persona_events_rs::sp_runtime;
use mlua::Lua;
use serde_json::Value;

fn template() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../civ6-autotest/scenarios/leader-persona-15km.json"),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn assignment_step_uses_shipped_void_api_once_then_waits_for_real_state() {
    let scene = sp_runtime::build(&template()).unwrap();
    let steps = scene["steps"].as_array().unwrap();
    let at = steps
        .iter()
        .position(|s| {
            s["code"]
                .as_str()
                .is_some_and(|c| c.contains("one normal assignment requested"))
        })
        .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let view = std::fs::read_to_string(
        root.join("poc/JAC_PoC_LeaderPersona/UI/JAC_LeaderPersona_View.lua"),
    )
    .unwrap();
    let begin = view.find("function JAC_AssignLeader(").unwrap();
    let end = begin + view[begin..].find("function JAC_PromoteLeader(").unwrap();
    let lua = Lua::new();
    lua.load(r#"
calls=0; allowed=true
Active=function() return true end
Get=function(_,key) if key=='OWNER' then return 0 else return 'GOVERNOR' end end
Game={GetLocalPlayer=function() return 0 end}
GameInfo={JAC_LeaderPersonas={LEADER_JAC_DATE_TANEMUNE={GovernorType='DATE'}},Governors={DATE={Hash=1006,Index=6}}}
city={GetID=function() return 65536 end}
CityManager={GetCity=function(owner,id) assert(owner==0 and id==65536); return city end}
Players={[0]={GetGovernors=function() return {CanAssignGovernor=function(_,hash,c) assert(hash==1006 and c==city);return allowed end} end,
GetCities=function() return {Members=function() local n=0;return function() n=n+1;if n==1 then return 1,city end end end} end}}
PlayerOperations={ASSIGN_GOVERNOR=3,PARAM_GOVERNOR_TYPE='governor',PARAM_PLAYER_ONE='owner',PARAM_CITY_DEST='city'}
UI={RequestPlayerOperation=function(owner,operation,args) assert(owner==0 and operation==3 and args.governor==6 and args.owner==0 and args.city==65536);calls=calls+1 end}
"#).exec().unwrap();
    lua.load(&view[begin..end]).exec().unwrap();
    let result: String = lua
        .load(steps[at]["code"].as_str().unwrap())
        .eval()
        .unwrap();
    assert_eq!(result, "one normal assignment requested");
    lua.load("assert(calls==1);allowed=false").exec().unwrap();
    assert!(lua
        .load(steps[at]["code"].as_str().unwrap())
        .exec()
        .unwrap_err()
        .to_string()
        .contains("city not assignable"));
    lua.load("assert(calls==1)").exec().unwrap();
    assert_eq!(steps[at + 1]["action"], "wait_for");
    assert!(steps[at + 1]["condition"]["code"]
        .as_str()
        .unwrap()
        .contains("GetAssignedCity"));
}

#[test]
fn generated_cli_file_exactly_matches_current_scenario_source() {
    for cost in [2, 3] {
        let output = std::env::temp_dir().join(format!(
            "todo1433-sp-runtime-cost{cost}-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../civ6-autotest/scenarios/leader-persona-15km.json");
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_civ6-persona-events-rs"));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        command.arg("generate-sp-runtime").arg(input).arg(&output);
        if cost == 3 {
            command.arg("3");
        }
        let status = command.status().unwrap();
        assert!(status.success());
        let actual = std::fs::read_to_string(&output).unwrap();
        std::fs::remove_file(&output).unwrap();
        let expected =
            serde_json::to_string_pretty(&sp_runtime::build_with_cost(&template(), cost).unwrap())
                .unwrap();
        assert_eq!(
            actual, expected,
            "CLI scenario must match the current generator exactly"
        );
    }
}

#[test]
fn scenario_keeps_normal_ui_sender_and_explicit_runtime_restart_boundaries() {
    let scene = sp_runtime::build(&template()).unwrap();
    civ6_autotest_rs::scenario::validate_scenario(&scene).unwrap();
    let steps = scene["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 32);
    let lua = Lua::new();
    let mut body = String::new();
    for s in steps {
        let code = s["code"]
            .as_str()
            .or(s["condition"]["code"].as_str())
            .unwrap_or("");
        lua.load(code).into_function().unwrap();
        body.push_str(code);
        for forbidden in [
            "SetProperty",
            "ChangeGovernorPoints",
            "JAC_LeaderPersonaProject",
            "EventsProbeStatus",
            "APPOINT_GOVERNOR",
            "PROMOTE_GOVERNOR",
        ] {
            assert!(!code.contains(forbidden), "forbidden {forbidden}");
        }
    }
    assert_eq!(body.matches("JAC_EvolveLeader('").count(), 1);
    assert_eq!(
        body.matches("OnStart='JAC_LeaderPersonaRepublish'").count(),
        1
    );
    assert_eq!(body.matches("JAC_RequestLeaderJoin(1)").count(), 1);
    assert_eq!(body.matches("JAC_AssignLeader('").count(), 1);
    assert_eq!(steps[5]["action"], "ui_lua");
    assert!(steps[5]["code"]
        .as_str()
        .unwrap()
        .contains("pre-existing unjoined governor"));
    let attach = steps
        .iter()
        .position(|s| s["action"] == "start_native_paid_runtime")
        .unwrap();
    assert!(attach > 5);
    let saved = steps.iter().position(|s| s["action"] == "save").unwrap();
    let turn = steps
        .iter()
        .position(|s| s["action"] == "end_turn")
        .unwrap();
    assert_eq!(
        steps.iter().filter(|s| s["action"] == "end_turn").count(),
        1
    );
    assert!(attach < turn && turn < saved);
    assert_eq!(steps[turn]["turns"], 1);
    // The pre-turn audit must follow the official AI title grant, not merely
    // repeat the fresh-start check before any title can be spent.
    assert_eq!(steps[turn - 2]["state_name"], "GameCore_Tuner");
    assert!(steps[turn - 2]["code"]
        .as_str()
        .unwrap()
        .contains("AI has no usable official title"));
    for position in [turn - 1, turn + 1] {
        assert_eq!(steps[position]["state_name"], "InGame");
        assert!(steps[position]["code"]
            .as_str()
            .unwrap()
            .contains(sp_runtime::AI_ADMISSION));
    }
    assert_eq!(body.matches(sp_runtime::AI_ADMISSION).count(), 2);
    assert_eq!(steps[turn + 1]["state_name"], "InGame");
    assert!(steps[turn + 1]["code"]
        .as_str()
        .unwrap()
        .contains(sp_runtime::AI_GOVERNORS));
    assert_eq!(steps[turn + 2]["state_name"], "GameCore_Tuner");
    assert_eq!(steps[turn + 2]["code"], sp_runtime::AI_GREAT_PEOPLE);
    assert!(steps[saved - 1]["code"]
        .as_str()
        .unwrap()
        .contains("sample(false)"));
    assert_eq!(steps[saved + 1]["action"], "restart_game");
    assert_eq!(steps[saved + 2]["action"], "load");
    for s in &steps[saved + 2..] {
        assert!(!s["code"]
            .as_str()
            .unwrap_or("")
            .contains("RequestPlayerOperation"));
    }
    assert!(body.contains("a.allowed==false and b.allowed==true"));
    assert!(body.contains("checked==483"));
    let observer = sp_runtime::observer();
    assert!(!observer.contains("RequestPlayerOperation") && !observer.contains(".Remove("));
}

#[test]
fn restored_snapshot_reads_saved_state_without_querying_absent_runtime_gate() {
    let lua = world();
    lua.load("full(); assigned=true; Players[0]:GetGovernors().CanAssignGovernor=function() error('no runtime after restart') end").exec().unwrap();
    lua.load(format!("{}\nlocal v=assert(sample(false)); assert(v.phase=='full' and v.allowed==nil and v.assigned==v.city)", sp_runtime::SNAPSHOT)).exec().unwrap();
    assert!(lua
        .load(format!("{}\nreturn sample()", sp_runtime::SNAPSHOT))
        .exec()
        .is_err());
}

fn ai_world() -> Lua {
    let lua = Lua::new();
    lua.load(r#"
badGovernor=false; badPerson=false; joined=false; aiCount=21
admissionResult=false; admissionCalls={}; badOwner=7; badHash=109
Game={GetProperty=function() return joined and 7 or nil end}
local rows={}; for i=1,22 do rows[i]={LeaderType='L'..i,GovernorType='G'..i,PersonType='P'..i} end
GameInfo={Governors={},GreatPersonIndividuals={},GreatPersonClasses={C={UnitType='U'}},Units={U={Index=99}},
 JAC_LeaderPersonas=function() local i=0; return function() i=i+1; return rows[i] end end}
for i=1,22 do GameInfo.Governors['G'..i]={Hash=100+i}; local p={GreatPersonIndividualType='P'..i,GreatPersonClassType='C'}
 GameInfo.GreatPersonIndividuals['P'..i]=p; GameInfo.GreatPersonIndividuals[i]=p end
PlayerManager={GetAliveMajors=function()
 local all={}; for i=1,aiCount do local owner=i
  all[#all+1]={IsHuman=function() return false end,GetID=function() return owner end,
   GetGovernors=function() return {
    HasGovernor=function(_,hash) return badGovernor and owner==7 and hash==109 end,
    CanEverAppointGovernor=function(_,hash)
     local key=owner..'/'..hash; admissionCalls[key]=(admissionCalls[key] or 0)+1
     if owner==badOwner and hash==badHash then return admissionResult end; return false
    end} end,
   GetUnits=function() return {Members=function()
    local n=0; return function() n=n+1; if n==1 then return 1,{GetType=function() return 1 end,GetGreatPerson=function() error('ordinary unit GP read') end}
     elseif n==2 and badPerson and owner==7 then return 2,{GetType=function() return 99 end,GetGreatPerson=function() return {GetIndividual=function() return 9 end} end} end end
   end} end}
 end; return all
end}
"#).exec().unwrap();
    lua
}

#[test]
fn ai_admission_checks_all_462_pairs_before_and_after_turn_and_rejects_permission_only_faults() {
    let lua = ai_world();
    for phase in 1..=2 {
        lua.load(sp_runtime::AI_ADMISSION).exec().unwrap();
        lua.load(format!("local n=0; for owner=1,21 do for hash=101,122 do assert(admissionCalls[owner..'/'..hash]=={phase}); n=n+1 end end; assert(n==462)")).exec().unwrap();
    }
    // Native objects remain absent: these faults would pass both old audits.
    for result in ["true", "nil", "0", "'false'"] {
        for (owner, hash) in [(1, 101), (7, 109), (21, 122)] {
            let lua = ai_world();
            lua.load(sp_runtime::AI_ADMISSION).exec().unwrap();
            lua.load(format!(
                "badOwner={owner}; badHash={hash}; admissionResult={result}"
            ))
            .exec()
            .unwrap();
            lua.load(sp_runtime::AI_GOVERNORS).exec().unwrap();
            lua.load(sp_runtime::AI_GREAT_PEOPLE).exec().unwrap();
            let err = lua
                .load(sp_runtime::AI_ADMISSION)
                .exec()
                .unwrap_err()
                .to_string();
            assert!(
                err.contains("unjoined AI appointment permission opened"),
                "{err}"
            );
        }
    }
    for fault in ["aiCount=20", "joined=true"] {
        let lua = ai_world();
        lua.load(fault).exec().unwrap();
        assert!(
            lua.load(sp_runtime::AI_ADMISSION).exec().is_err(),
            "{fault}"
        );
    }
}

#[test]
fn native_ai_governor_and_gp_audits_independently_reject_each_violation() {
    for (change, governor_ok, gp_ok) in [
        ("", true, true),
        ("badGovernor=true", false, true),
        ("badPerson=true", true, false),
        ("joined=true", false, false),
        ("aiCount=20", false, false),
    ] {
        let lua = ai_world();
        lua.load(change).exec().unwrap();
        assert_eq!(
            lua.load(sp_runtime::AI_GOVERNORS).exec().is_ok(),
            governor_ok,
            "{change}"
        );
        assert_eq!(
            lua.load(sp_runtime::AI_GREAT_PEOPLE).exec().is_ok(),
            gp_ok,
            "{change}"
        );
    }
}

fn world() -> Lua {
    let lua = Lua::new();
    lua.load(r#"
L='LEADER_JAC_DATE_TANEMUNE'; SOURCE='civ6-persona-events-rs/v3/normal-Gameplay'
state='EVOLVING'; unit=500; paid=1; spent=1; title=false; allowed=false; assigned=false
Game={GetLocalPlayer=function() return 0 end,GetProperty=function(_,k)
 if k=='JAC_PERSONA_'..L..'_STATE' then return state end
 if k=='JAC_PERSONA_'..L..'_UNIT' then return unit end
end,SetProperty=function() error('write forbidden') end}
PlayerConfigurations={[1]={GetLeaderTypeName=function() return L end}}
GameInfo={JAC_LeaderPersonas={[L]={GovernorType='G',BasePromotionType='B',EvolutionCost=2}},
 Governors={G={Index=6,Hash=106}},GovernorPromotions={B={Index=54,Hash=154},B_TITLE_1={Index=74,Hash=174}}}
local city={GetID=function() return 65536 end,GetOwner=function() return 0 end}
nativeOwner=0
local g={GetOwner=function() return nativeOwner end,GetType=function() return 6 end,
 HasPromotion=function(_,h) return h==154 or title end,GetAssignedCity=function() return assigned and city or nil end}
local gs={GetGovernor=function(_,h) assert(h==106); return g end,GetGovernorPointsSpent=function() return spent end,
 GetGovernorPoints=function() return 2 end,CanAssignGovernor=function(_,h,c) assert(h==106 and c==city); return allowed end}
Players={[0]={GetGovernors=function() return gs end,GetUnits=function() return {FindID=function(_,id) return id==unit and {} or nil end} end,
 GetCities=function() return {Members=function() local seen=false; return function() if not seen then seen=true; return 1,city end end end} end}}
function record(name,promotion,hash,seq,serial)
 local r={source=SOURCE,context='Gameplay',owner=0,leader=L,governor=6,governorHash=106,accepted=true,
  name=name,sequence=seq,promotion=promotion,promotionHash=hash,spendingSerial=serial,binding='bound',arity=promotion and 3 or 2,
  raw={{type='number',value=0},{type='number',value=6}}}
 if promotion then r.raw[3]={type='number',value=promotion} end; return r
end
a={appointed=record('GovernorAppointed',nil,nil,2),[1]=record('GovernorPromoted',54,154,1,1)}
debits={{owner=0,governor=6,promotion=54,binding='bound',confirmed={owner=0,binding='bound',afterSpent=1,expected=1}},
 {owner=0,governor=6,promotion=74,binding='bound',confirmed={owner=0,binding='bound',afterSpent=2,expected=2}}}
ExposedMembers={JAC_LeaderPersona={EventsGameplayStatus=function()
 return {source=SOURCE,active=true,registrations={GovernorAppointed=true,GovernorPromoted=true},
  acquired=a,paid=paid,sequence=title and 3 or 2,account={debits=debits}}
end}}
UI={RequestPlayerOperation=function() error('observer cannot send') end}
handlers={}
Events=setmetatable({},{__index=function(t,k) local e={Add=function(f) handlers[#handlers+1]=f end,
 Remove=function() error('observer cannot remove another callback') end}; rawset(t,k,e); return e end})
function full()
 title=true; spent=2; paid=2; state='GOVERNOR'; unit=nil; allowed=true
 a[2]=record('GovernorPromoted',74,174,3,2)
end
    "#).exec().unwrap();
    lua
}

#[test]
fn observer_captures_real_partial_then_confirmed_full_without_sending_or_writing() {
    let lua = world();
    lua.load(sp_runtime::observer()).exec().unwrap();
    lua.load("assert(#handlers==2); handlers[1](); assert(JAC_NORMAL_SP_AUDIT.partial.allowed==false); full(); handlers[2](); assert(JAC_NORMAL_SP_AUDIT.full.allowed and JAC_NORMAL_SP_AUDIT.error==nil)").exec().unwrap();
}

#[test]
fn observer_rejects_unpaid_true_full_false_double_charge_and_wrong_owner() {
    for change in [
        "allowed=true",
        "full(); allowed=false",
        "full(); spent=3",
        "nativeOwner=1",
        "full(); debits[2].confirmed=nil",
        "local original=Game.GetProperty; Game.GetProperty=function(self,key) if key=='JAC_TEST853_PAID_ASSIGNMENT_V1' then return true end; return original(self,key) end",
    ] {
        let lua = world();
        lua.load(sp_runtime::observer()).exec().unwrap();
        lua.load(change).exec().unwrap();
        lua.load("handlers[1](); assert(JAC_NORMAL_SP_AUDIT.error~=nil)")
            .exec()
            .unwrap();
    }
}

#[test]
fn missing_partial_window_cannot_be_manufactured_from_a_full_result() {
    let lua = world();
    lua.load(sp_runtime::observer()).exec().unwrap();
    lua.load("full(); handlers[1](); assert(JAC_NORMAL_SP_AUDIT.full and JAC_NORMAL_SP_AUDIT.partial==nil)").exec().unwrap();
}

#[test]
fn generator_rejects_mp_and_wrong_source_configuration() {
    let mut t = template();
    t["steps"][1]["players"][2]["human"] = true.into();
    assert!(sp_runtime::build(&t).is_err());
    let mut t = template();
    t["steps"][1]["players"][1]["leader"] = "LEADER_JAC_OTHER".into();
    assert!(sp_runtime::build(&t).is_err());
    let mut t = template();
    t["steps"][1]["ruleset"] = "RULESET_STANDARD".into();
    assert!(sp_runtime::build(&t).is_err());
}

fn three_observer() -> String {
    let scene = sp_runtime::build_with_cost(&template(), 3).unwrap();
    scene["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|s| {
            let code = s["code"].as_str()?;
            code.contains("observer already exists")
                .then(|| code.to_owned())
        })
        .unwrap()
}

fn three_world() -> Lua {
    let lua = world();
    lua.load(r#"
L='LEADER_JAC_YAMANOUCHI_UESUGI_NORIMASA'; third=false
PlayerConfigurations[4]=PlayerConfigurations[1]
GameInfo.JAC_LeaderPersonas={[L]={GovernorType='G',BasePromotionType='B',EvolutionCost=3}}
GameInfo.GovernorPromotions.B_TITLE_2={Index=94,Hash=194}
local gs=Players[0]:GetGovernors(); local gov=gs:GetGovernor(106)
gov.HasPromotion=function(_,h) if h==154 then return true elseif h==174 then return title elseif h==194 then return third end; error('unknown promotion') end
gs.GetGovernorPoints=function() return 3 end
a={appointed=record('GovernorAppointed',nil,nil,2),[1]=record('GovernorPromoted',54,154,1,1)}
function middle()
 title=true; spent=2; paid=2; a[2]=record('GovernorPromoted',74,174,3,2)
end
function full()
 middle(); third=true; spent=3; paid=3; state='GOVERNOR'; unit=nil; allowed=true
 a[3]=record('GovernorPromoted',94,194,4,3)
 debits[3]={owner=0,governor=6,promotion=94,binding='bound',confirmed={owner=0,binding='bound',afterSpent=3,expected=3}}
end
"#).exec().unwrap();
    lua
}

#[test]
fn cost_three_observes_both_unpaid_prefixes_and_only_all_paid_permit() {
    let lua = three_world();
    lua.load(three_observer()).exec().unwrap();
    lua.load(
        r#"
handlers[1](); assert(JAC_NORMAL_SP_AUDIT.partials[1].allowed==false)
middle(); handlers[2](); assert(JAC_NORMAL_SP_AUDIT.partials[2].allowed==false)
full(); handlers[1](); local q=JAC_NORMAL_SP_AUDIT
assert(q.error==nil and q.full.allowed and #q.full.promotions==3 and q.full.spent==3)
"#,
    )
    .exec()
    .unwrap();
}

#[test]
fn cost_three_observer_rejects_early_permit_gaps_receipt_mixup_and_double_charge() {
    for change in [
        "allowed=true",
        "middle(); allowed=true",
        "a[1]=nil; allowed=true",
        "third=true",
        "full(); allowed=false",
        "full(); spent=4",
        "middle(); debits[2].confirmed=nil",
        "full(); a[3].leader='other'",
        "full(); a[3].promotion=74",
        "full(); a[3].sequence=a[2].sequence",
        "full(); debits[3].confirmed.afterSpent=2; debits[3].confirmed.expected=2",
        "nativeOwner=1",
    ] {
        let lua = three_world();
        lua.load(three_observer()).exec().unwrap();
        lua.load(change).exec().unwrap();
        lua.load("handlers[1](); assert(JAC_NORMAL_SP_AUDIT.error~=nil)")
            .exec()
            .unwrap_or_else(|e| panic!("mutation {change}: {e}"));
    }
}

#[test]
fn cost_three_generated_pair_requires_real_second_partial_and_preserves_normal_senders() {
    let scene = sp_runtime::build_with_cost(&template(), 3).unwrap();
    civ6_autotest_rs::scenario::validate_scenario(&scene).unwrap();
    let lua = three_world();
    lua.load(three_observer()).exec().unwrap();
    lua.load("handlers[1](); full(); handlers[2]()")
        .exec()
        .unwrap();
    let pair = scene["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["capture_as"] == "NORMAL_SP_PAIR")
        .unwrap()["code"]
        .as_str()
        .unwrap();
    // Model only the non-person audit matrix here; payment identity and account
    // are exercised through the real generated sample and pair code.
    lua.load(r#"
PlayerManager={GetAliveMajorIDs=function() return {0} end}
GameInfo.JAC_LeaderPersonas=setmetatable(GameInfo.JAC_LeaderPersonas,{__call=function(t) local n=0; return function()
 n=n+1; if n<=483 then return {LeaderType='OTHER',GovernorType='OTHER'} end
end end})
GameInfo.Governors.OTHER={Hash=999}
local gs=Players[0]:GetGovernors(); gs.HasGovernor=function() return false end; gs.CanEverAppointGovernor=function() return false end
"#).exec().unwrap();
    assert!(lua
        .load(pair)
        .exec()
        .unwrap_err()
        .to_string()
        .contains("second unpaid prefix"));
    // A result cannot manufacture the missing earlier event after finalization.
    lua.load("assert(JAC_NORMAL_SP_AUDIT.partials[2]==nil)")
        .exec()
        .unwrap();
    let mut body = String::new();
    for s in scene["steps"].as_array().unwrap() {
        let code = s["code"]
            .as_str()
            .or(s["condition"]["code"].as_str())
            .unwrap_or("");
        Lua::new().load(code).into_function().unwrap();
        body.push_str(code);
        for forbidden in [
            "SetProperty",
            "ChangeGovernorPoints",
            "APPOINT_GOVERNOR",
            "PROMOTE_GOVERNOR",
        ] {
            assert!(!code.contains(forbidden));
        }
    }
    assert_eq!(body.matches("JAC_RequestLeaderJoin(4)").count(), 1);
    assert_eq!(body.matches("JAC_EvolveLeader('").count(), 1);
    assert_eq!(body.matches("JAC_AssignLeader('").count(), 1);
    assert!(!body.contains(sp_runtime::LEADER));
    assert!(body.contains("GetGovernorPoints()>=3"));
    assert!(body.contains("v.promotions[3].sequence==b.promotions[3].sequence"));
    assert_eq!(
        sp_runtime::build_with_cost(&template(), 2).unwrap(),
        sp_runtime::build(&template()).unwrap()
    );
    assert!(sp_runtime::build_with_cost(&template(), 1).is_err());
    let mut wrong = template();
    wrong["steps"][1]["players"][4]["leader"] = "wrong".into();
    assert!(sp_runtime::build_with_cost(&wrong, 3).is_err());
}
