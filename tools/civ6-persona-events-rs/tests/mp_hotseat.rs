use civ6_persona_events_rs::{mp_hotseat as mp, sp_runtime::SERIAL};
use mlua::Lua;
use serde_json::Value;
fn template(name: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../civ6-autotest/scenarios")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}
fn scene() -> Value {
    mp::build(
        &template("leader-persona-15km.json"),
        &template("multiplayer-banseiki-hotseat-growth.json"),
    )
    .unwrap()
}
#[test]
fn scenario_keeps_hotseat22_and_explicit_restart_reconnect_with_no_state_forgery() {
    let s = scene();
    civ6_autotest_rs::scenario::validate_scenario(&s).unwrap();
    assert_eq!(s["session_type"], "hotseat");
    assert_eq!(s["steps"][0]["exclusive_user_mods"], true);
    assert_eq!(
        s["steps"][0]["ids"],
        template("leader-persona-15km.json")["steps"][0]["ids"]
    );
    let steps = s["steps"].as_array().unwrap();
    let players = steps[2]["players"].as_array().unwrap();
    assert_eq!(players.len(), 22);
    assert_eq!(players.iter().filter(|p| p["human"] == true).count(), 2);
    assert_eq!(players[0]["leader"], "LEADER_JAC_ODA_NOBUHIDE");
    assert_eq!(players[1]["leader"], "LEADER_JAC_TAKEDA_NOBUTORA");
    let lua = Lua::new();
    let mut all = String::new();
    let mut resident = false;
    let mut installs = 0;
    for (i, step) in steps.iter().enumerate() {
        let code = step["code"]
            .as_str()
            .or(step["condition"]["code"].as_str())
            .unwrap_or("");
        lua.load(code).into_function().unwrap();
        all.push_str(code);
        for forbidden in [
            "SetProperty",
            "ChangeGovernorPoints",
            "SetGovernorPoints",
            "persona_runtime_restart",
            "PROMOTE_GOVERNOR",
            "APPOINT_GOVERNOR",
            "CompleteEvolution(",
        ] {
            assert!(!code.contains(forbidden), "{forbidden}");
        }
        match step["action"].as_str().unwrap() {
            "start_native_paid_runtime" => {
                assert!(!resident);
                resident = true;
                installs += 1;
            }
            "restart_game" => resident = false,
            "load" => {
                assert!(!resident);
                assert_eq!(steps[i - 1]["action"], "restart_game");
            }
            _ => {}
        }
    }
    assert_eq!(installs, 3);
    assert!(!resident);
    assert_eq!(all.matches("JAC_RequestLeaderJoin(").count(), 2);
    assert_eq!(all.matches("JAC_EvolveLeader(").count(), 2);
    assert_eq!(all.matches("JAC_AssignLeader(").count(), 2);
    assert_eq!(
        steps.iter().filter(|s| s["action"] == "end_turn").count(),
        2
    );
    assert_eq!(steps.iter().filter(|s| s["action"] == "save").count(), 2);
    let mid = steps
        .iter()
        .find(|s| s["capture_as"] == "HOTSEAT_MID_JOINED_UNPAID")
        .unwrap()["code"]
        .as_str()
        .unwrap();
    assert!(
        mid.contains("v.people[2].state=='GREAT_PERSON'")
            && mid.contains("v.people[2].remaining==2")
    );
    for s in steps.iter().filter(|s| s.get("equals_capture").is_some()) {
        assert!(!s["code"].as_str().unwrap().contains("person(0,true)"));
    }
    assert!(s["name"]
        .as_str()
        .unwrap()
        .contains("not network multi-device or saved Spent1 proof"));
}
#[test]
fn rejects_missing_22_roster_foreign_human_or_duplicate_identity() {
    let n = template("leader-persona-15km.json");
    let h = template("multiplayer-banseiki-hotseat-growth.json");
    let mut bad = n.clone();
    bad["steps"][1]["players"].as_array_mut().unwrap().pop();
    assert!(mp::build(&bad, &h).is_err());
    let mut bad = h.clone();
    bad["steps"][1]["players"][1] = bad["steps"][1]["players"][0].clone();
    assert!(mp::build(&n, &bad).is_err());
    let mut bad = h.clone();
    bad["session_type"] = "single_player".into();
    assert!(mp::build(&n, &bad).is_err());
}

fn world() -> Lua {
    let lua = Lua::new();
    lua.load(r#"
local leaders={'LEADER_JAC_DATE_TANEMUNE','LEADER_JAC_HOJO_UJITSUNA'}
props={};localId=0;turn=1;records={};phases={};members=function(t)return pairs(t)end
Game={GetLocalPlayer=function()return localId end,GetCurrentGameTurn=function()return turn end,GetProperty=function(_,k)return props[k]end,SetProperty=function()error('state write forbidden')end}
GameConfiguration={IsHotseat=function()return true end};PlayerConfigurations={[3]={GetLeaderTypeName=function()return leaders[1]end},[2]={GetLeaderTypeNameName=function()end,GetLeaderTypeName=function()return leaders[2]end}}
GameInfo={JAC_LeaderPersonas={},Governors={},GovernorPromotions={},GreatPersonIndividuals={},GreatPersonClasses={C={UnitType='U'}},Units={U={Index=99}}}
Players={};cities={};govs={};units={};spent={};gateOverride={};calls=0
PlayerManager={GetAliveMajorIDs=function() local ids={};for i=0,21 do ids[#ids+1]=i end;return ids end}
for owner=0,21 do local id=owner;units[id]={};cities[id]={};spent[id]=0
 for c=1,2 do local cityid=owner*100+c;cities[id][c]={GetID=function()return cityid end,GetOwner=function()return id end}end
 Players[id]={IsHuman=function()return id<2 end,IsTurnActive=function()return localId==id end,
 GetCities=function()return{Members=function()return members(cities[id])end}end,GetUnits=function()return{Members=function()return members(units[id])end,FindID=function(_,i)return units[id][i]end}end,
 GetGovernors=function()return{GetGovernor=function(_,hash)return govs[id] and govs[id].hash==hash and govs[id] or nil end,
 GetGovernorPointsSpent=function()return spent[id]end,GetGovernorPoints=function()return 2 end,
 CanAssignGovernor=function(_,hash,city)calls=calls+1;if gateOverride[id]~=nil then return gateOverride[id] end;return phases[id]=='full' and govs[id].hash==hash and city:GetOwner()==id end}end}
end
for owner=0,1 do local l=leaders[owner+1];local row={GovernorType='G'..owner,BasePromotionType='B'..owner,PersonType='P'..owner,EvolutionCost=2};GameInfo.JAC_LeaderPersonas[l]=row
 GameInfo.Governors[row.GovernorType]={Index=6+owner,Hash=106+owner}
 GameInfo.GovernorPromotions[row.BasePromotionType]={Index=54+owner,Hash=154+owner};GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_1']={Index=74+owner,Hash=174+owner}
 local ind={GreatPersonClassType='C',GreatPersonIndividualType=row.PersonType};GameInfo.GreatPersonIndividuals[row.PersonType]=ind;GameInfo.GreatPersonIndividuals[20+owner]=ind
end
function phase(owner,kind)
 phases[owner]=kind;local l=leaders[owner+1];local row=GameInfo.JAC_LeaderPersonas[l];local index=6+owner;local prefix='JAC_PERSONA_'..l..'_';local source=owner==0 and 3 or 2
 props[prefix..'OWNER']=owner;props[prefix..'SOURCE']=source;props[prefix..'EVOLUTION_COST']=2;props[prefix..'EVOLUTION_SURCHARGE']=1
 props[prefix..'STATE']=kind=='gp' and 'GREAT_PERSON' or kind=='partial' and 'EVOLVING' or 'GOVERNOR'
 local unitid=500+owner;props[prefix..'UNIT']=kind~='full' and unitid or nil;units[owner]={}
 if kind~='full'then units[owner][unitid]={GetID=function()return unitid end,GetType=function()return 99 end,GetOwner=function()return owner end,GetGreatPerson=function()return{GetIndividual=function()return 20+owner end}end}end
 spent[owner]=kind=='gp' and 0 or kind=='partial' and 1 or 2
 govs[owner]=nil;if kind~='gp'then govs[owner]={hash=106+owner,GetOwner=function()return owner end,GetType=function()return index end,GetAssignedCity=function()return nil end,HasPromotion=function(_,h)return h==154+owner or phases[owner]=='full' end}end
 local a={};local account={debits={}}
 local function receipt(name,promo,seq,serial)
 local r={owner=owner,leader=l,sourcePlayer=source,accepted=true,source='civ6-persona-events-rs/v3/normal-Gameplay',context='Gameplay',governor=index,governorHash=106+owner,sequence=seq,name=name,arity=promo and 3 or 2,raw={{type='number',value=owner},{type='number',value=index}},binding='B',spendingSerial=serial}
 if promo then r.promotion=promo;r.promotionHash=promo+100;r.raw[3]={type='number',value=promo};account.debits[serial]={owner=owner,governor=index,promotion=promo,binding='B',confirmed={owner=owner,binding='B',afterSpent=serial,expected=serial}}end;return r end
 if kind~='gp' then a[1]=receipt('GovernorPromoted',54+owner,owner*10+1,1);a.appointed=receipt('GovernorAppointed',nil,owner*10+2)end
 if kind=='full'then a[2]=receipt('GovernorPromoted',74+owner,owner*10+3,2)end
 records[owner]={active=true,source='civ6-persona-events-rs/v3/normal-Gameplay',registrations={GovernorAppointed=true,GovernorPromoted=true},paid=spent[owner],acquired=a,account=account}
end
ExposedMembers={JAC_LeaderPersona={EventsGameplayStatus=function(owner,l)assert(l==leaders[owner+1]);return records[owner]end}}
Events=setmetatable({},{__index=function(t,k)local e={Add=function(f)end,Remove=function(f)end};rawset(t,k,e);return e end})
CityManager={GetCity=function(owner,id)for _,c in pairs(cities[owner])do if c:GetID()==id then return c end end end}
phase(0,'gp');phase(1,'gp')
"#).exec().unwrap();
    lua
}
fn check(lua: &Lua, code: &str) -> mlua::Result<()> {
    lua.load(format!(
        "{}\n{}\n{SERIAL}\n{code}",
        mp::header([3, 2]),
        mp::READ
    ))
    .exec()
}
#[test]
fn both_humans_observe_unpaid_denial_then_full_permit_without_payment_sender() {
    for owner in 0..2 {
        let lua = world();
        lua.load(format!("localId={owner}")).exec().unwrap();
        lua.load(mp::observer(&mp::header([3, 2]), owner))
            .exec()
            .unwrap();
        lua.load(format!("phase({owner},'partial');JAC_MP_OBSERVER.observe();assert(JAC_MP_OBSERVER.partial.allowed==false and JAC_MP_OBSERVER.partial.remaining==1);phase({owner},'full');JAC_MP_OBSERVER.observe();assert(not JAC_MP_OBSERVER.error and JAC_MP_OBSERVER.full.paid==2 and JAC_MP_OBSERVER.full.remaining==0)")).exec().unwrap();
    }
}
#[test]
fn owner_swaps_raw_callback_spending_and_unpaid_permission_cannot_pass() {
    for fault in [
        "gateOverride[1]=true",
        "records[1].acquired[1].owner=0",
        "records[1].acquired.appointed.raw[1].value=0",
        "spent[1]=2",
        "records[1].paid=2",
        "props.JAC_TEST853_PAID_ASSIGNMENT_V1=true",
    ] {
        let lua = world();
        lua.load("phase(1,'partial')").exec().unwrap();
        lua.load(fault).exec().unwrap();
        assert!(check(&lua, "assert(person(1,true))").is_err(), "{fault}");
    }
    let lua = world();
    lua.load(mp::observer(&mp::header([3, 2]), 1))
        .exec()
        .unwrap();
    lua.load("phase(1,'full');JAC_MP_OBSERVER.observe();assert(JAC_MP_OBSERVER.full and not JAC_MP_OBSERVER.partial)").exec().unwrap();
}
#[test]
fn mid_round_save_read_preserves_full0_unpaid1_without_runtime_queries() {
    let lua = world();
    lua.load("phase(0,'full');localId=1;props.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1={version=3,sequence=3};calls=0").exec().unwrap();
    check(&lua,"local v=ledger();assert(v.active==1 and v.people[1].paid==2 and v.people[2].paid==0 and v.people[2].remaining==2);saved=serial(v);assert(calls==0)").unwrap();
    check(&lua, "assert(serial(ledger())==saved and calls==0)").unwrap();
    lua.load("records[1].paid=1").exec().unwrap();
    assert!(check(&lua, "assert(serial(ledger())==saved)").is_err());
}
#[test]
fn other_human_payment_does_not_change_first_humans_personal_receipts() {
    let lua = world();
    lua.load("phase(0,'full')").exec().unwrap();
    check(&lua, "saved0=serial(person(0,false))").unwrap();
    lua.load("phase(1,'full')").exec().unwrap();
    check(
        &lua,
        "assert(serial(person(0,false))==saved0);assert(person(1,true).paid==2)",
    )
    .unwrap();
    lua.load("records[0].account.debits[1].confirmed.afterSpent=2")
        .exec()
        .unwrap();
    assert!(check(&lua, "assert(serial(person(0,false))==saved0)").is_err());
}

#[test]
fn active_owner_receipt_mismatch_partial_missing_and_unknown_gate_fail_closed() {
    for owner in 0..2 {
        for fault in [
            "gateOverride[OWNER]=1",
            "records[OWNER].acquired[1].sourcePlayer=0",
        ] {
            let lua = world();
            lua.load(format!("localId={owner};phase({owner},'partial')"))
                .exec()
                .unwrap();
            lua.load(fault.replace("OWNER", &owner.to_string()))
                .exec()
                .unwrap();
            lua.load(mp::observer(&mp::header([3, 2]), owner))
                .exec()
                .unwrap();
            lua.load("JAC_MP_OBSERVER.observe();assert(JAC_MP_OBSERVER.error and not JAC_MP_OBSERVER.partial)").exec().unwrap();
        }
        let lua = world();
        lua.load(mp::observer(&mp::header([3, 2]), owner))
            .exec()
            .unwrap();
        lua.load(format!("phase({owner},'partial');records[{owner}].acquired.appointed=nil;JAC_MP_OBSERVER.observe();assert(not JAC_MP_OBSERVER.partial);phase({owner},'full');JAC_MP_OBSERVER.observe();assert(JAC_MP_OBSERVER.full and not JAC_MP_OBSERVER.partial)")).exec().unwrap();
    }
}

#[test]
fn cli_generates_exact_hotseat_scene_in_hidden_child() {
    for (mode, expected) in [
        ("generate-mp-hotseat", scene()),
        ("generate-mp-setup", setup_probe()),
    ] {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../civ6-autotest/scenarios");
        let path = std::env::temp_dir().join(format!(
            "todo324-mp-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_civ6-persona-events-rs"));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let result = command
            .arg(mode)
            .arg(root.join("leader-persona-15km.json"))
            .arg(root.join("multiplayer-banseiki-hotseat-growth.json"))
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let generated = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(generated, serde_json::to_string_pretty(&expected).unwrap());
    }
}

fn setup_probe() -> Value {
    mp::build_setup_probe(
        &template("leader-persona-15km.json"),
        &template("multiplayer-banseiki-hotseat-growth.json"),
    )
    .unwrap()
}
#[test]
fn setup_probe_is_one_bounded_start_without_persona_runtime_or_payment() {
    let probe = setup_probe();
    civ6_autotest_rs::scenario::validate_scenario(&probe).unwrap();
    let steps = probe["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 8);
    assert_eq!(steps[2]["players"].as_array().unwrap().len(), 12);
    assert_eq!(
        steps[2]["players"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["human"] == true)
            .count(),
        2
    );
    assert_eq!(steps[2]["map_file"], "Pangaea.lua");
    assert_eq!(steps[0], scene()["steps"][0]);
    assert_eq!(
        steps.iter().filter(|s| s["action"] == "start_game").count(),
        1
    );
    let lua = Lua::new();
    for s in steps {
        assert!(!matches!(
            s["action"].as_str(),
            Some("save" | "load" | "end_turn" | "start_native_paid_runtime")
        ));
        let code = s["code"]
            .as_str()
            .or(s["condition"]["code"].as_str())
            .unwrap_or("");
        lua.load(code).into_function().unwrap();
        for forbidden in [
            "SetProperty",
            "JAC_RequestLeaderJoin",
            "JAC_EvolveLeader",
            "RequestPlayerOperation",
            "HostGame(",
            "DoLeftClick(",
        ] {
            assert!(!code.contains(forbidden), "{forbidden}");
        }
    }
}
fn setup_world() -> Lua {
    let lua = Lua::new();
    let probe = setup_probe();
    lua.load(r#"
map='Pangaea.lua';maximum=20;minimum=2;count=12;humans=2;active=0;turn=1;accounting=false
UI={IsInFrontEnd=function()return true end}
GameConfiguration={IsHotseat=function()return true end,GetRuleSet=function()return 'RULESET_JAC_BANSEIKI' end,
GetParticipatingPlayerCount=function()return count+2 end,GetHiddenPlayerCount=function()return 2 end,GetValue=function()return 0 end,GetStartTurn=function()return 1 end,IsPaused=function()return false end}
MapConfiguration={GetValue=function()return map end,GetMaxMajorPlayers=function()return maximum end,GetMinMajorPlayers=function()return minimum end}
Game={GetLocalPlayer=function()return active end,GetCurrentGameTurn=function()return turn end,GetProperty=function()return accounting and {} or nil end}
SlotStatus={SS_TAKEN=3,SS_COMPUTER=1};CivilizationLevelTypes={CIVILIZATION_LEVEL_FULL_CIV=624367792};PlayerConfigurations={};Players={}
PlayerManager={GetAliveMajorIDs=function()local t={};for i=0,count-1 do t[#t+1]=i end;return t end}
ExposedMembers={} -- Pangaea does not satisfy LeaderPersona's map criteria.
"#).exec().unwrap();
    for (id, p) in probe["steps"][2]["players"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        lua.load(format!("do local id={id};PlayerConfigurations[id]={{GetLeaderTypeName=function()return '{}' end,GetCivilizationTypeName=function()return '{}' end,GetSlotStatus=function()return id<humans and 3 or 1 end,GetCivilizationLevelTypeID=function()return 624367792 end}};Players[id]={{IsAlive=function()return true end,IsMajor=function()return true end,IsHuman=function()return id<humans end,IsTurnActive=function()return id==active end}} end",p["leader"].as_str().unwrap(),p["civ"].as_str().unwrap())).exec().unwrap();
    }
    lua
}
#[test]
fn setup_proofs_reject_wrong_map_roster_and_activeplayer_without_persona_dependency() {
    let probe = setup_probe();
    let pre = probe["steps"][3]["code"].as_str().unwrap();
    let post = probe["steps"][6]["code"].as_str().unwrap();
    let lua = setup_world();
    lua.load(pre).exec().unwrap();
    lua.load(post).exec().unwrap();
    for fault in [
        "map='JAC_Japan_15km.Civ6Map'",
        "maximum=8",
        "count=22",
        "humans=3",
    ] {
        let lua = setup_world();
        lua.load(fault).exec().unwrap();
        assert!(lua.load(pre).exec().is_err(), "{fault}");
    }
    for fault in [
        "count=2",
        "humans=3",
        "active=1",
        "turn=2",
        "map='JAC_Japan_15km.Civ6Map'",
    ] {
        let lua = setup_world();
        lua.load(fault).exec().unwrap();
        assert!(lua.load(post).exec().is_err(), "{fault}");
    }
}

#[test]
fn measured_map_maximum_is_distinct_from_stock_staging_slot_cap() {
    let probe = setup_probe();
    let pre = probe["steps"][3]["code"].as_str().unwrap();
    for maximum in [12, 20, 22] {
        let lua = setup_world();
        lua.load(format!("maximum={maximum}")).exec().unwrap();
        let result = lua.load(pre).eval::<String>().unwrap();
        assert!(result.contains(&format!("mapMax={maximum} stagingMax=12")));
    }
    for fault in [
        "maximum=nil",
        "maximum='20'",
        "maximum=11",
        "minimum=13",
        "PlayerConfigurations[12]=PlayerConfigurations[11]",
        "PlayerConfigurations[5].GetCivilizationLevelTypeID=function()return 0 end",
    ] {
        let lua = setup_world();
        lua.load(fault).exec().unwrap();
        assert!(lua.load(pre).exec().is_err(), "{fault}");
    }
    // Non-full-civ slots are not counted by the official full-civ predicate.
    let lua = setup_world();
    lua.load("PlayerConfigurations[12]={GetSlotStatus=function()return 1 end,GetCivilizationLevelTypeID=function()return 0 end}").exec().unwrap();
    lua.load(pre).exec().unwrap();
}

#[test]
fn basic_startup_acceptance_is_independent_of_inapplicable_persona_api() {
    let probe = setup_probe();
    let post = probe["steps"][6]["code"].as_str().unwrap();
    assert!(!post.contains("EventsGameplayStatus") && !post.contains("GAMEPLAY_ACQUISITIONS"));
    for (setup,observed) in [
        ("ExposedMembers=nil","nil"),
        ("ExposedMembers={}","nil"),
        ("ExposedMembers={JAC_LeaderPersona={EventsGameplayStatus=function() error('must not call map-inapplicable API') end}}","table"),
    ] {
        let lua=setup_world();lua.load(setup).exec().unwrap();
        let result=lua.load(post).eval::<String>().unwrap();
        assert!(result.starts_with("MP_SETUP12 STARTED actual12majors"));
        assert!(result.contains("persona_actions=not_applicable") && result.contains("persona_acceptance=unmeasured"));
        assert!(result.contains(&format!("observed_api={observed}")));
        lua.load("count=11").exec().unwrap();assert!(lua.load(post).exec().is_err());
    }
}
