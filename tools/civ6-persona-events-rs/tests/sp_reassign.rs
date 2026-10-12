use civ6_persona_events_rs::{sp_reassign as side, sp_runtime::SERIAL};
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
fn draft_cannot_run_and_physical_save_names_are_required() {
    let draft = side::build(&template(), None, 6).unwrap();
    assert_eq!(draft["input_status"], "placeholder_not_executable");
    assert!(draft["steps"][0]["code"]
        .as_str()
        .unwrap()
        .starts_with("error('DRAFT:"));
    for name in ["../save", "", side::PLACEHOLDER, side::OUTPUT_SAVE] {
        assert!(side::build(&template(), Some(name), 6).is_err());
    }
    for n in [0, 11] {
        assert!(side::build(&template(), Some("Successful-Physical-0001"), n).is_err());
    }
}
#[test]
fn scene_sends_one_void_assignment_no_payment_and_restores_without_gate_queries() {
    let scene = side::build(&template(), Some("Successful-Physical-0001"), 6).unwrap();
    civ6_autotest_rs::scenario::validate_scenario(&scene).unwrap();
    let steps = scene["steps"].as_array().unwrap();
    let lua = Lua::new();
    let mut code = String::new();
    for s in steps {
        let text = s["code"]
            .as_str()
            .or(s["condition"]["code"].as_str())
            .unwrap_or("");
        lua.load(text).into_function().unwrap();
        code.push_str(text);
        for forbidden in [
            "SetProperty",
            "JAC_RequestLeaderJoin",
            "JAC_EvolveLeader",
            "PROMOTE_GOVERNOR",
            "APPOINT_GOVERNOR",
            "CompleteEvolution",
            "JAC_PromoteLeader",
        ] {
            assert!(!text.contains(forbidden), "{forbidden}");
        }
    }
    assert_eq!(code.matches("JAC_AssignLeader(leader,target)").count(), 1);
    assert!(!code.contains("assert(JAC_AssignLeader"));
    assert_eq!(code.matches("UI.RequestPlayerOperation").count(), 1); // canonical republish only
    assert_eq!(
        steps.iter().filter(|s| s["action"] == "end_turn").count(),
        6
    );
    let save = steps.iter().position(|s| s["action"] == "save").unwrap();
    assert_eq!(steps[save + 1]["action"], "restart_game");
    assert_eq!(steps[save + 2]["action"], "load");
    let restored = steps
        .iter()
        .find(|s| s["equals_capture"] == "REASSIGN_SETTLED_SAVE")
        .unwrap()["code"]
        .as_str()
        .unwrap();
    assert!(
        !restored.contains("CanAssignGovernor") && !restored.contains("RequestPlayerOperation")
    );
    assert_eq!(steps[2]["name"], "Successful-Physical-0001");
    assert!(steps[2]["checkpoint"].is_null());
}

fn world() -> Lua {
    let lua = Lua::new();
    lua.load(r#"
L='LEADER_JAC_DATE_TANEMUNE'; SOURCE='civ6-persona-events-rs/v3/normal-Gameplay'; turn=3; assigned=42; spent=2; merit=5; title=true
props={}; local prefix='JAC_PERSONA_'..L..'_'
for k,v in pairs({OWNER=0,SOURCE=1,STATE='GOVERNOR',EVOLUTION_COST=2,EVOLUTION_SURCHARGE=1,MERIT=5,NODE_SKILL=1,CATEGORY='C',BRANCH_C_G='R'}) do props[prefix..k]=v end
Game={GetCurrentGameTurn=function() return turn end,GetProperty=function(_,k) if k==prefix..'MERIT' then return merit end; return props[k] end,SetProperty=function() error('write forbidden') end}
function collection(rows,key) local t={};for _,r in ipairs(rows) do t[r[key]]=r;if r.Index then t[r.Index]=r end end;return setmetatable(t,{__call=function() local i=0;return function() i=i+1;return rows[i] end end}) end
GameInfo={JAC_LeaderPersonas={[L]={GovernorType='G',BasePromotionType='B',PersonType='P',EvolutionCost=2}},Governors={G={Index=6,Hash=106}},
GovernorPromotions={B={GovernorPromotionType='B',Index=54,Hash=154},B_TITLE_1={GovernorPromotionType='B_TITLE_1',Index=74,Hash=174}},
GreatPersonIndividuals={P={GreatPersonClassType='C'}},GreatPersonClasses={C={UnitType='U'}},Units={U={Index=99}},
GovernorPromotionSets=collection({{GovernorType='G',GovernorPromotion='B'},{GovernorType='G',GovernorPromotion='B_TITLE_1'}},'GovernorPromotion'),
JAC_PersonaTreeNodes=collection({{LeaderType=L,NodeType='SKILL',CategoryType='C',ExclusiveGroup='G'}},'NodeType')}
cities={};for _,id in ipairs({9,42,77}) do local n=id;cities[n]={GetID=function() return n end,GetOwner=function() return 0 end} end
CityManager={GetCity=function(owner,id) return owner==0 and cities[id] or nil end}
function members(t) return pairs(t) end
g={GetOwner=function() return 0 end,GetType=function() return 6 end,GetAssignedCity=function() return cities[assigned] end,
 HasPromotion=function(_,h) return h==154 or title end,IsEstablished=function() return true end,GetTurnsOnSite=function() return 5 end,GetNeutralizedTurns=function() return 0 end}
gs={GetGovernor=function() return g end,GetGovernorPointsSpent=function() return spent end,GetGovernorPoints=function() return 2 end,
 CanAssignGovernor=function() error('snapshot must not query runtime') end}
Players={[0]={GetGovernors=function() return gs end,GetCities=function() return {Members=function() return members(cities) end} end,GetUnits=function() return {Members=function() return members({}) end} end}}
PlayerManager={GetAliveMajorIDs=function() return {0} end}
function record(name,promotion,hash,seq,serial)
 local r={name=name,source=SOURCE,context='Gameplay',owner=0,leader=L,accepted=true,governor=6,governorHash=106,
 sequence=seq,spendingSerial=serial,binding='bound',promotion=promotion,promotionHash=hash,arity=promotion and 3 or 2,
 raw={{type='number',value=0},{type='number',value=6}}};if promotion then r.raw[3]={type='number',value=promotion} end;return r
end
acquired={appointed=record('GovernorAppointed',nil,nil,2),[1]=record('GovernorPromoted',54,154,1,1),[2]=record('GovernorPromoted',74,174,3,2)}
account={debits={{owner=0,governor=6,promotion=54,binding='bound',confirmed={owner=0,binding='bound',afterSpent=1,expected=1}},
 {owner=0,governor=6,promotion=74,binding='bound',confirmed={owner=0,binding='bound',afterSpent=2,expected=2}}}}
ExposedMembers={JAC_LeaderPersona={EventsGameplayStatus=function() return {source=SOURCE,active=true,registrations={GovernorAppointed=true,GovernorPromoted=true},
 paid=2,sequence=3,acquired=acquired,account=account,reservation={baseline=0},receipts={acquired[1],acquired.appointed,acquired[2]},commit={turn=1}} end}}
"#).exec().unwrap();
    lua
}
#[test]
fn reader_uses_actual_nonfirst_assignment_and_distinct_owned_destination() {
    let lua = world();
    lua.load(format!(
        "{}\nlocal s=snapshot(); assert(s.city.id==42 and destination(s.city.id)==9)",
        side::READER
    ))
    .exec()
    .unwrap();
    lua.load("assigned=9").exec().unwrap();
    lua.load(format!(
        "{}\nlocal s=snapshot(); assert(s.city.id==9 and destination(s.city.id)==42)",
        side::READER
    ))
    .exec()
    .unwrap();
    lua.load("cities[42]=nil; cities[77]=nil").exec().unwrap();
    assert!(lua
        .load(format!(
            "{}\nreturn destination(snapshot().city.id)",
            side::READER
        ))
        .exec()
        .is_err());
}

#[test]
fn delayed_republish_waits_read_only_without_resending_and_rejects_unknown_gate() {
    let scene = side::build(&template(), Some("Successful-Physical-0001"), 1).unwrap();
    let step = scene["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| {
            s["condition"]["code"]
                .as_str()
                .is_some_and(|c| c.contains("unknown native destination permission"))
        })
        .unwrap();
    assert_eq!(step["action"], "wait_for");
    assert_eq!(step["timeout"], 90);
    let code = step["condition"]["code"].as_str().unwrap();
    assert!(!code.contains("RequestPlayerOperation") && !code.contains("SetProperty"));
    let lua = world();
    lua.load(
        r#"
Game.GetLocalPlayer=function() return 0 end
PlayerConfigurations={[1]={GetLeaderTypeName=function() return L end}}
JAC_REASSIGN_AUDIT={target=9,before={city={id=42}}}
gate=false; reads=0; gs.CanAssignGovernor=function(_,hash,city)
 assert(hash==106 and city:GetID()==9); reads=reads+1; return gate
end
UI={RequestPlayerOperation=function() error('read-only wait resent request') end}
"#,
    )
    .exec()
    .unwrap();
    assert!(!lua.load(code).eval::<bool>().unwrap());
    assert!(!lua.load(code).eval::<bool>().unwrap());
    lua.load("gate=true").exec().unwrap();
    assert!(lua.load(code).eval::<bool>().unwrap());
    lua.load("assert(reads==3); gate=nil").exec().unwrap();
    assert!(lua.load(code).exec().is_err());
    lua.load("gate=true; assigned=77").exec().unwrap();
    assert!(lua.load(code).exec().is_err());
}

#[test]
fn shipped_void_assignment_is_sent_once_and_completion_wait_reads_actual_target() {
    let scene = side::build(&template(), Some("Successful-Physical-0001"), 1).unwrap();
    let steps = scene["steps"].as_array().unwrap();
    let at = steps
        .iter()
        .position(|s| s["state_name"] == "JAC_LeaderPersona_View")
        .unwrap();
    let view = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../poc/JAC_PoC_LeaderPersona/UI/JAC_LeaderPersona_View.lua"),
    )
    .unwrap();
    let begin = view.find("function JAC_AssignLeader(").unwrap();
    let end = view[begin..].find("function JAC_PromoteLeader(").unwrap() + begin;
    let lua = world();
    lua.load(r#"
function Active() return true end
function Get(leader,field) return Game:GetProperty('JAC_PERSONA_'..leader..'_'..field) end
Game.GetLocalPlayer=function() return 0 end
PlayerOperations={ASSIGN_GOVERNOR=5,PARAM_GOVERNOR_TYPE='governor',PARAM_PLAYER_ONE='owner',PARAM_CITY_DEST='city'}
gs.CanAssignGovernor=function(_,hash,city) return hash==106 and city:GetID()==9 end
requests=0; UI={RequestPlayerOperation=function(owner,op,p)
 requests=requests+1;assert(owner==0 and op==5 and p.governor==6 and p.owner==0 and p.city==9)
end}
JAC_REASSIGN_AUDIT={target=9}
"#).exec().unwrap();
    lua.load(&view[begin..end]).exec().unwrap();
    let sender = steps[at]["code"].as_str().unwrap();
    lua.load(sender).exec().unwrap();
    let condition = steps[at + 1]["condition"]["code"].as_str().unwrap();
    assert!(!lua.load(condition).eval::<bool>().unwrap());
    assert!(lua.load(sender).exec().is_err());
    lua.load("assert(requests==1); assigned=77").exec().unwrap();
    assert!(!lua.load(condition).eval::<bool>().unwrap());
    lua.load("assigned=9").exec().unwrap();
    assert!(lua.load(condition).eval::<bool>().unwrap());
}
#[test]
fn reader_rejects_missing_title_double_spend_invalid_receipt_and_wrong_city() {
    for fault in [
        "title=false",
        "spent=3",
        "account.debits[2].confirmed=nil",
        "acquired[2].raw[3].value=99",
        "assigned=999",
        "props['JAC_TEST853_PAID_ASSIGNMENT_V1']=true",
    ] {
        let lua = world();
        lua.load(fault).exec().unwrap();
        assert!(
            lua.load(format!("{}\nreturn snapshot()", side::READER))
                .exec()
                .is_err(),
            "{fault}"
        );
    }
}
#[test]
fn complete_history_and_skills_change_is_detected_even_if_paid_count_stays_two() {
    for fault in [
        "acquired[2].sequence=4",
        "props['JAC_PERSONA_'..L..'_NODE_SKILL']=nil",
        "props['JAC_PERSONA_'..L..'_BRANCH_C_G']='other'",
        "account.debits[1].confirmed.afterSpent=2;account.debits[1].confirmed.expected=2",
    ] {
        let lua = world();
        lua.load(format!(
            "{SERIAL}\n{}\nbeforeStable=serial(snapshot().stable)",
            side::READER
        ))
        .exec()
        .unwrap();
        lua.load(fault).exec().unwrap();
        assert!(
            lua.load(format!(
                "{SERIAL}\n{}\nassert(serial(snapshot().stable)==beforeStable)",
                side::READER
            ))
            .exec()
            .is_err(),
            "{fault}"
        );
    }
}
#[test]
fn save_comparison_includes_actual_city_merit_and_typed_nil_state() {
    for fault in [
        "assigned=77",
        "merit=6",
        "props.JAC_PERSONA_PENDING_SEQUENCE=0",
    ] {
        let lua = world();
        lua.load(format!(
            "{SERIAL}\n{}\nsaved=serial(snapshot())",
            side::READER
        ))
        .exec()
        .unwrap();
        lua.load(fault).exec().unwrap();
        assert!(
            lua.load(format!(
                "{SERIAL}\n{}\nassert(serial(snapshot())==saved)",
                side::READER
            ))
            .exec()
            .is_err(),
            "{fault}"
        );
    }
}
