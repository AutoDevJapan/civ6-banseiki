//! Read-only observer of actual AI turns in the preserved cost-one Governor
//! fixture. This does not demonstrate ordinary admission or AI title payment.
use serde_json::{json, Value};
use std::{env, fs, path::PathBuf};

const SOURCE_SAVE: &str =
    "JAC-Enemy-Neutralization-91f02912dd354a18821959c99aa89868-0000000000000008";
const READ: &str = r#"
function JAC158_Read()
 local leader='LEADER_JAC_ASHINA_MORIKIYO'; local owner=1; local key='JAC_PERSONA_'..leader..'_';
 local function get(field) return Game:GetProperty(key..field) end;
 local p=assert(Players[owner],'AI owner absent'); assert(not p:IsHuman(),'owner became human');
 assert(get('OWNER')==owner and get('STATE')=='GOVERNOR','committed AI Governor absent');
 local row=assert(GameInfo.JAC_LeaderPersonas[leader]); local gs=p:GetGovernors();
 local g=assert(gs:GetGovernor(GameInfo.Governors[row.GovernorType].Hash),'native Governor absent');
 assert(gs:HasGovernor(GameInfo.Governors[row.GovernorType].Hash) and
  g:HasPromotion(GameInfo.GovernorPromotions[row.BasePromotionType].Hash),'native Governor/base absent');
 local city=assert(g:GetAssignedCity(),'native assigned city absent');
 assert(city:GetOwner()==owner and g:IsEstablished() and g:GetNeutralizedTurns()==0,'AI Governor not active and established');
 assert(city:GetProperty('JAC_PERSONA_SETTLED_'..leader)==1,'native settlement marker absent');
 local s={turn=Game.GetCurrentGameTurn(),owner=owner,leader=leader,city=city:GetID(),
  merit=get('MERIT') or 0,category=get('CATEGORY'),branches={},nodes={},spent=gs:GetGovernorPointsSpent(),
  total=gs:GetGovernorPoints(),ai=get('AI_LAST_TURN'),govern=get('GOVERN_LAST_TURN'),
  deed=get('DEED_GOVERN_TURN_'..tostring(get('AI_LAST_TURN'))),count=get('GOVERN_TURN_COUNT'),
  countTurn=get('GOVERN_TURN_COUNT_TURN')};
 assert(s.category=='ADMIN','expected existing ADMIN commitment');
 for route in GameInfo.JAC_PersonaRouteTemplates() do
  local field='BRANCH_'..route.CategoryType..'_'..route.ExclusiveGroup;
  if get(field) then s.branches[field]=get(field) end;
 end;
 assert(s.branches.BRANCH_ADMIN_FOUNDATION=='STORES','existing STORES commitment absent');
 local count=0;
 for n in GameInfo.JAC_PersonaTreeNodes() do if n.LeaderType==leader then
  count=count+1; local value=get('NODE_'..n.NodeType);
  assert(value==nil or value==1,'invalid owned node value');
  if value==1 then
   assert(n.CategoryType==s.category,'owned node violates category');
   if n.ExclusiveGroup then assert(s.branches['BRANCH_'..n.CategoryType..'_'..n.ExclusiveGroup]==n.RouteType,'owned node violates branch') end;
   if n.PrereqNode then assert(get('NODE_'..n.PrereqNode)==1,'owned node lacks prerequisite') end;
   assert(JAC_PersonaTreeRules.Status(owner,leader,n.NodeType)=='OWNED','owned status mismatch'); s.nodes[n.NodeType]=n.Cost;
  elseif n.CategoryType~=s.category then
   assert(JAC_PersonaTreeRules.Status(owner,leader,n.NodeType)=='CATEGORY_LOCKED','other category opened');
  elseif n.ExclusiveGroup and s.branches['BRANCH_'..n.CategoryType..'_'..n.ExclusiveGroup] and
   s.branches['BRANCH_'..n.CategoryType..'_'..n.ExclusiveGroup]~=n.RouteType then
   assert(JAC_PersonaTreeRules.Status(owner,leader,n.NodeType)=='BRANCH_LOCKED','sibling branch opened');
  end;
 end end;
 assert(count>0 and next(s.nodes),'non-vacuous person tree required');
 return s;
end;
function JAC158_Encode(s)
 local a={}; for _,field in ipairs({'turn','owner','leader','city','merit','category','spent','total','ai','govern','deed','count','countTurn'}) do a[#a+1]=field..'='..tostring(s[field]) end;
 local b={}; for field,value in pairs(s.branches) do b[#b+1]=field..'='..value end; table.sort(b); for _,v in ipairs(b) do a[#a+1]=v end;
 local n={}; for node,cost in pairs(s.nodes) do n[#n+1]=node..'='..cost end; table.sort(n); for _,v in ipairs(n) do a[#a+1]=v end;
 return table.concat(a,'|');
end;
"#;

const CHECK: &str = r#"
local old=assert(JAC158_previous,'AI turn observation not initialized'); local s=JAC158_Read();
assert(s.turn==old.turn+1,'not exactly one actual game turn');
assert(s.city==old.city and s.spent==old.spent,'fixture assignment or title spending changed');
assert(type(old.ai)=='number' and s.ai==old.ai+1 and (s.ai==s.turn or s.ai==s.turn-1) and s.govern==s.ai,'native AI turn did not run persona callback');
assert(s.deed==s.ai and s.countTurn==s.ai and s.count==1,'governance deed not credited exactly once');
assert(s.category==old.category,'AI changed category');
for field,value in pairs(old.branches) do assert(s.branches[field]==value,'AI changed irreversible route') end;
for node,cost in pairs(old.nodes) do assert(s.nodes[node]==cost,'AI lost an owned node') end;
local purchases=0; local cost=0;
for node,price in pairs(s.nodes) do if not old.nodes[node] then
 purchases=purchases+1; cost=cost+price;
 local n=assert(GameInfo.JAC_PersonaTreeNodes[node]);
 assert(not n.PrereqNode or old.nodes[n.PrereqNode],'AI bought a node before its prerequisite');
end end;
assert(purchases<=GameInfo.JAC_PersonaTreeParameters['AI_PURCHASES_PER_TURN'].Value,'AI exceeded purchase budget');
local earned=s.merit+cost-old.merit;
local profile=GameInfo.JAC_PersonaTreePeople[s.leader].MeritProfile;
assert(earned>=GameInfo.JAC_PersonaMeritRules[profile].GOVERN_TURN,'merit conservation/earning failed');
JAC158_purchases=(JAC158_purchases or 0)+purchases; JAC158_previous=s;
return 'AI_REAL_TURN purchases='..purchases..' cost='..cost..' earned='..earned..'|'..JAC158_Encode(s);
"#;

fn lua(code: impl Into<String>) -> Value {
    json!({"action":"ui_lua","state_name":"JAC_LeaderPersona_Tree","code":code.into()})
}

fn scenario() -> Value {
    let mut steps = vec![
        json!({"action":"load","name":SOURCE_SAVE,"exact_name":true,"timeout":300}),
        json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"JAC_LeaderPersona_Tree","code":"local p=Players[Game.GetLocalPlayer()]; return p and p:IsHuman() and p:IsTurnActive() and type(ExposedMembers.JAC_LeaderPersonaNativeRead)=='function' and type(JAC_PersonaTreeRules)=='table'"}}),
        lua(format!("{READ}\nJAC158_previous=JAC158_Read(); JAC158_purchases=0; assert(JAC158_previous.turn==45,'wrong preserved input turn'); return 'AI_BASELINE|'..JAC158_Encode(JAC158_previous)")),
    ];
    // No autoplay, direct callback, purchase, merit or lock setters.
    for _ in 0..12 {
        steps.push(json!({"action":"end_turn","timeout":300}));
        steps.push(lua(CHECK));
    }
    let mut capture = lua("assert(JAC158_purchases>0,'no newly observed AI skill purchase'); return JAC158_Encode(JAC158_Read())");
    capture["capture_as"] = json!("AI_PERSONA_BEFORE_SAVE");
    steps.push(capture);
    steps.push(
        json!({"action":"save","name":"JAC-ToDo158-AI-Skills","checkpoint":true,"timeout":120}),
    );
    steps.push(json!({"action":"exit_to_menu"}));
    steps.push(
        json!({"action":"load","name":"JAC-ToDo158-AI-Skills","checkpoint":true,"timeout":300}),
    );
    steps.push(json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"JAC_LeaderPersona_Tree","code":"local p=Players[Game.GetLocalPlayer()]; return p and p:IsTurnActive() and type(ExposedMembers.JAC_LeaderPersonaNativeRead)=='function' and type(JAC_PersonaTreeRules)=='table'"}}));
    let mut restored = lua(format!("{READ}\nreturn JAC158_Encode(JAC158_Read())"));
    restored["equals_capture"] = json!("AI_PERSONA_BEFORE_SAVE");
    steps.push(restored);
    steps.push(lua("JAC158_previous=JAC158_Read(); JAC158_purchases=0; return 'post-load actual AI turn initialized'"));
    steps.push(json!({"action":"end_turn","timeout":300}));
    steps.push(lua(CHECK));
    steps.push(json!({"action":"exit_to_menu"}));
    json!({"name":"ToDo158: actual AI merit, skill purchases and immutable choices after save/load (cost-one fixture only)","judge_profile":"generic","strict_lua_errors":true,"steps":steps})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("pass output scenario path")?;
    let mut data = serde_json::to_string_pretty(&scenario())?;
    data.push('\n');
    fs::write(output, data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    fn check_with(old: &str, current: &str) -> mlua::Result<String> {
        let vm = Lua::new();
        vm.load(format!(
            "GameInfo={{JAC_PersonaTreeParameters={{AI_PURCHASES_PER_TURN={{Value=1}}}}, JAC_PersonaTreePeople={{L={{MeritProfile='ADMIN'}}}},JAC_PersonaMeritRules={{ADMIN={{GOVERN_TURN=2}}}}, JAC_PersonaTreeNodes={{A={{}},B={{PrereqNode='A'}},C={{}}}}}}; JAC158_previous={old}; function JAC158_Read() return {current} end; function JAC158_Encode() return 'checked' end;"
        )).exec()?;
        vm.load(CHECK).eval()
    }

    const OLD: &str =
        "{turn=45,ai=45,city=7,spent=1,merit=8,category='ADMIN',branches={F='STORES'},nodes={A=8}}";
    const NEXT: &str = "{turn=46,city=7,spent=1,merit=0,category='ADMIN',branches={F='STORES'},nodes={A=8,B=10},ai=46,govern=46,deed=46,countTurn=46,count=1,leader='L'}";

    #[test]
    fn actual_turn_checks_purchase_cost_and_governance_receipt() {
        assert!(check_with(OLD, NEXT).unwrap().contains("cost=10 earned=2"));
        for bad in [
            NEXT.replace("merit=0", "merit=-1"),
            NEXT.replace("ai=46", "ai=45"),
            NEXT.replace("count=1", "count=2"),
            NEXT.replace("F='STORES'", "F='WORKS'"),
            NEXT.replace("category='ADMIN'", "category='MILITARY'"),
            NEXT.replace("B=10", "B=10,C=8"),
            NEXT.replace("A=8,B=10", "B=10"),
        ] {
            assert!(check_with(OLD, &bad).is_err(), "accepted {bad}");
        }
        assert!(check_with(&OLD.replace("nodes={A=8}", "nodes={}"), NEXT).is_err());
    }

    #[test]
    fn scenario_does_not_mutate_product_state_or_replay_a_mission() {
        let data = scenario();
        let steps = data["steps"].as_array().unwrap();
        assert_eq!(
            steps.iter().filter(|s| s["action"] == "end_turn").count(),
            13
        );
        assert_eq!(steps.iter().filter(|s| s["action"] == "save").count(), 1);
        for s in steps.iter().filter(|s| s["action"] == "ui_lua") {
            let code = s["code"].as_str().unwrap();
            Lua::new().load(code).into_function().unwrap();
            for forbidden in [
                "SetProperty",
                "OnPlayerTurnStarted(",
                ".Queue(",
                ".Purchase(",
                "RequestPlayerOperation",
                "ChangeGovernorPoints",
            ] {
                assert!(!code.contains(forbidden), "scenario contains {forbidden}");
            }
        }
        let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../civ6-autotest/scenarios/leader-persona-ai-skills-save-load.json");
        let recorded: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(recorded, data, "regenerate the checked-in scenario");
    }
}
