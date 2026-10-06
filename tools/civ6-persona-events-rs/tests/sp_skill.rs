use civ6_persona_events_rs::sp_skill;
use mlua::Lua;
use serde_json::Value;
fn read(path: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path),
    )
    .unwrap()
}
fn template() -> Value {
    serde_json::from_str(&read(
        "tools/civ6-autotest/scenarios/leader-persona-15km.json",
    ))
    .unwrap()
}
#[test]
fn funding_uses_only_required_bounded_normal_turns_and_draft_cannot_run() {
    for (cost, reward, merit, expected) in [(3, 2, 2, 1), (8, 2, 2, 3), (10, 3, 2, 3), (8, 2, 8, 0)]
    {
        let lua = Lua::new();
        lua.load(format!(r#"
local merit,turn={merit},8
Players={{[0]={{IsTurnActive=function() return true end}}}}
JAC_SKILL_AUDIT={{cost={cost},requiredTurns=math.ceil(math.max(0,{cost}-merit)/{reward}),realTurns=0}}
requests=0; ActionTypes={{ACTION_ENDTURN=9}}
UI={{RequestAction=function(action) assert(action==9); requests=requests+1 end}}
function snapshot() return {{merit=merit,turn=turn,established=true,neutralized=0}} end
function simulate_engine_completion() if JAC_SKILL_AUDIT.advanced then merit=merit+{reward};turn=turn+1 end end
"#)).exec().unwrap();
        for _ in 0..sp_skill::MAX_EARNING_TURNS {
            lua.load(sp_skill::EARN_REQUEST).exec().unwrap();
            lua.load("simulate_engine_completion()").exec().unwrap();
        }
        assert_eq!(lua.globals().get::<u32>("requests").unwrap(), expected);
    }
    let t = template();
    let d = sp_skill::build(&t, None, None).unwrap();
    assert!(d["steps"][0]["code"]
        .as_str()
        .unwrap()
        .starts_with("error('DRAFT:"));
    assert!(sp_skill::build(&t, Some("save"), None).is_err());
    assert!(sp_skill::build(&t, None, Some(8)).is_err());
}
#[test]
fn generated_scene_reuses_original_ui_and_keeps_payment_history_and_save_contract() {
    let original = template();
    for merit in [0, 2, 8] {
        let scene = sp_skill::build(
            &original,
            Some("Reassignment-Success-Physical"),
            Some(merit),
        )
        .unwrap();
        civ6_autotest_rs::scenario::validate_scenario(&scene).unwrap();
        let steps = scene["steps"].as_array().unwrap();
        let lua = Lua::new();
        let mut code = String::new();
        for step in steps {
            let s = step["code"]
                .as_str()
                .or(step["condition"]["code"].as_str())
                .unwrap_or("");
            lua.load(s).into_function().unwrap();
            code.push_str(s);
            for forbidden in [
                "SetProperty",
                "OnPlayerTurnStarted",
                "JAC_EvolveLeader",
                "JAC_RequestLeaderJoin",
                "APPOINT_GOVERNOR",
                "PROMOTE_GOVERNOR",
            ] {
                assert!(!s.contains(forbidden), "{forbidden}");
            }
        }
        assert_eq!(
            code.matches("UI.RequestAction(ActionTypes.ACTION_ENDTURN)")
                .count(),
            sp_skill::MAX_EARNING_TURNS as usize
        );
        for s in steps {
            assert!(!s["condition"]["code"]
                .as_str()
                .unwrap_or("")
                .contains("RequestAction"));
        }
        assert!(code.contains("node.Cost==minimum") && !code.contains("node.Cost==8"));
        for ending in [
            "return 'review shows permanent locks'",
            "return 'requested gameplay purchase'",
        ] {
            let step = original["steps"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["code"].as_str().is_some_and(|c| c.contains(ending)))
                .unwrap();
            assert_eq!(steps.iter().filter(|s| *s == step).count(), 1);
        }
        assert_eq!(code.matches("JAC_TreeConfirm()").count(), 1);
        assert_eq!(
            code.matches("OnStart='JAC_LeaderPersonaPurchase'").count(),
            2
        ); // deliberate OWNED/BRANCH_LOCKED only
        assert!(code.contains("current.merit==q.before.merit-q.cost"));
        let saved = steps.iter().position(|s| s["action"] == "save").unwrap();
        assert_eq!(steps[saved + 1]["action"], "restart_game");
        for s in &steps[saved + 2..] {
            let c = s["code"].as_str().unwrap_or("");
            assert!(!c.contains("RequestPlayerOperation") && !c.contains("CanAssignGovernor"));
        }
    }
}
fn purchase_model(mutation: Option<(&str, &str)>) -> Lua {
    let lua = Lua::new();
    lua.load(r#"
leader='LEADER_JAC_DATE_TANEMUNE'; key='JAC_PERSONA_'..leader..'_';props={};props[key..'OWNER']=0;props[key..'STATE']='GOVERNOR';props[key..'MERIT']=8
Game={GetProperty=function(_,k) return props[k] end};Players={[0]={}};GameInfo={JAC_LeaderPersonas={[leader]={}},JAC_PersonaTreeNodes={}}
for _,spec in ipairs({{'ENVOYS_1','CONTACT','ENVOYS'},{'EXCHANGE_1','CONTACT','EXCHANGE'},{'WORKS_1','ADMIN','WORKS'}})do
 local n=leader..'_'..spec[1];GameInfo.JAC_PersonaTreeNodes[n]={NodeType=n,LeaderType=leader,CategoryType=spec[2],RouteType=spec[3],ExclusiveGroup='RELATIONS',Cost=8}
end
function Player(id) return Players[id] end
function Get(l,f) return props['JAC_PERSONA_'..l..'_'..f] end
function Set(l,f,v) props['JAC_PERSONA_'..l..'_'..f]=v end
function Finish(_,result) return result end
function SyncEffects() effects=(effects or 0)+1 end
ExposedMembers={}
"#).exec().unwrap();
    lua.load(read(
        "poc/JAC_PoC_LeaderPersona/Shared/JAC_PersonaTreeRules.lua",
    ))
    .exec()
    .unwrap();
    lua.load("Rules=JAC_PersonaTreeRules").exec().unwrap();
    let body = read("poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua");
    let a = body.find("local function Purchase(").unwrap();
    let b = a + body[a..].find("local MeritQueue =").unwrap();
    let mut source = body[a..b].replacen("local function Purchase(", "function Purchase(", 1);
    if let Some((from, to)) = mutation {
        assert!(source.contains(from));
        source = source.replacen(from, to, 1);
    }
    lua.load(source).exec().unwrap();
    lua
}
const PURCHASE:&str="assert(Purchase(0,leader,leader..'_ENVOYS_1','CONTACT','ENVOYS')=='PURCHASED'); assert(props[key..'MERIT']==0 and props[key..'NODE_'..leader..'_ENVOYS_1']==1 and props[key..'CATEGORY']=='CONTACT' and props[key..'BRANCH_CONTACT_RELATIONS']=='ENVOYS')";
#[test]
fn shipped_purchase_debits_exact_price_and_preserves_owned_and_exclusive_rejection() {
    let lua = purchase_model(None);
    lua.load(PURCHASE).exec().unwrap();
    lua.load("assert(Purchase(0,leader,leader..'_ENVOYS_1','CONTACT','ENVOYS')=='OWNED');assert(Purchase(0,leader,leader..'_EXCHANGE_1','CONTACT','ENVOYS')=='BRANCH_LOCKED');assert(Purchase(0,leader,leader..'_WORKS_1','ADMIN','WORKS')=='CATEGORY_LOCKED');assert(props[key..'MERIT']==0 and effects==1)").exec().unwrap();
    let lua = purchase_model(None);
    lua.load("props[key..'MERIT']=7;assert(Purchase(0,leader,leader..'_ENVOYS_1','CONTACT','ENVOYS')=='NO_MERIT');assert(props[key..'MERIT']==7 and props[key..'CATEGORY']==nil and effects==nil)").exec().unwrap();
    let lua = purchase_model(None);
    lua.load("GameInfo.JAC_PersonaTreeNodes[leader..'_ENVOYS_1'].Cost=3;props[key..'MERIT']=4;assert(Purchase(0,leader,leader..'_ENVOYS_1','CONTACT','ENVOYS')=='PURCHASED');assert(props[key..'MERIT']==1)").exec().unwrap();
}
#[test]
fn price_check_rejects_no_debit_and_double_debit_mutants() {
    for replacement in [
        "Rules.Merit(leader) - 0",
        "Rules.Merit(leader) - 2*node.Cost",
    ] {
        let lua = purchase_model(Some(("Rules.Merit(leader) - node.Cost", replacement)));
        assert!(lua.load(PURCHASE).exec().is_err());
    }
}

#[test]
fn purchased_effects_use_original_positive_not_same_named_unpurchased_negative() {
    let scene = sp_skill::build(&template(), Some("Physical-Save"), Some(2)).unwrap();
    let code = scene["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|s| {
            s["code"].as_str().filter(|c| {
                c.contains("actual active city effects and gameplay settlement marker verified")
            })
        })
        .unwrap();
    let world = || {
        let lua = Lua::new();
        lua.load(r#"
cityMarker=1;subjectName='Actual Reassigned City'
local city={GetName=function() return 'Actual Reassigned City' end,GetProperty=function() return cityMarker end}
Players={[0]={GetCities=function() return {Members=function() local done=false;return function() if not done then done=true;return 1,city end end end} end}}
Locale={Lookup=function(v) return v end}; skillActive=true; subjectOwner=0
GameEffects={GetModifiers=function() return {1,2} end,
GetModifierDefinition=function(id) return {Id=id==1 and 'JAC_PERSONA_DATE_TANEMUNE_BASE' or 'JAC_PERSONA_TREE_LEADER_JAC_DATE_TANEMUNE_ENVOYS_1'} end,
GetModifierActive=function(id) return id==1 or skillActive end,
GetModifierSubjects=function(id) return {id} end,
GetObjectsPlayerId=function() return subjectOwner end,
GetObjectName=function() return subjectName end}
"#).exec().unwrap();
        lua
    };
    let lua = world();
    lua.load(code).exec().unwrap();
    for mutation in [
        "skillActive=false",
        "subjectOwner=1",
        "subjectName='Different City'",
        "cityMarker=0",
    ] {
        let lua = world();
        lua.load(mutation).exec().unwrap();
        assert!(lua.load(code).exec().is_err());
    }
    let old_negative = code
        .replace("BASE']=1", "BASE']=0")
        .replace("ENVOYS_1']=1", "ENVOYS_1']=0");
    assert_ne!(old_negative, code);
    assert!(world().load(old_negative).exec().is_err());
}
