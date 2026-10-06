//! A normal-start rejection probe before the unchanged 113-step suite.
//! No bridge install, diagnostic flag, join or appointment request is inserted.
use civ6_autotest_rs::scenario::validate_scenario;
use serde_json::{json, Value};
use std::{env, fs};

const ORIGINAL: &str = include_str!("../../civ6-autotest/scenarios/leader-persona-15km.json");
const READ: &str = r#"
local function integer(v) return type(v)=='number' and v>=0 and v%1==0 end
assert(Game.GetLocalPlayer()==0 and Players[0]:IsHuman()==true,'human slot differs')
local turn=Game.GetCurrentGameTurn(); assert(integer(turn),'unknown actual turn')
local people,leaders,hashes={},{},{}
for row in GameInfo.JAC_LeaderPersonas() do
 local def=assert(GameInfo.Governors[row.GovernorType],'Governor definition absent')
 assert(type(row.LeaderType)=='string' and not leaders[row.LeaderType],'duplicate persona')
 assert(type(def.Hash)=='number' and not hashes[def.Hash],'duplicate Governor hash')
 leaders[row.LeaderType]=true; hashes[def.Hash]=true
 people[#people+1]={row=row,def=def}
 -- No one joins in this scenario. Reject an unexpected joined ledger too.
 for _,field in ipairs({'OWNER','STATE','UNIT'}) do
  assert(Game:GetProperty('JAC_PERSONA_'..row.LeaderType..'_'..field)==nil,'unexpected joined ledger')
 end
end
assert(#people==22,'complete 22-person catalog required')
table.sort(people,function(a,b) return a.row.LeaderType<b.row.LeaderType end)
local alive,count={},0
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 local id=p:GetID(); assert(integer(id) and id<22 and not alive[id],'duplicate or invalid owner')
 alive[id]=true; count=count+1
end
assert(count==22,'all 22 owners must be alive')
local class=assert(GameInfo.GreatPersonClasses['GREAT_PERSON_CLASS_JAC_PERSONA'])
local unit=assert(GameInfo.Units[class.UnitType])
assert(integer(unit.Index),'unknown persona unit type')
local pairs,ai,balances=0,0,{}
for id=0,21 do
 local p=assert(Players[id]); local c=assert(PlayerConfigurations[id]); local e=expected[id+1]
 assert(alive[id] and p:IsAlive()==true and p:IsMajor()==true and p:IsHuman()==e.human,'owner identity differs')
 assert(c:GetLeaderTypeName()==e.leader and c:GetCivilizationTypeName()==e.civ,'configured faction differs')
 local gs=assert(p:GetGovernors())
 assert(type(gs.HasGovernor)=='function','UI appointment getter unavailable')
 for _,entry in ipairs(people) do
  assert(gs:HasGovernor(entry.def.Hash)==false,'native unjoined appointment exists')
 end
 for _,u in p:GetUnits():Members() do
  assert(u:GetType()~=unit.Index,'unjoined persona Great Person exists')
 end
 if id>0 then
  ai=ai+1
  assert(type(gs.CanEverAppointGovernor)=='function' and type(gs.CanAppointGovernor)=='function','UI qualification getter unavailable')
  local total,spent=gs:GetGovernorPoints(),gs:GetGovernorPointsSpent()
  assert(integer(total) and integer(spent) and total>=spent,'invalid native title balance')
  if requireBudget then assert(total-spent>=1,'AI has no usable native title') end
  balances[#balances+1]=id..':'..total..':'..spent
  for _,entry in ipairs(people) do
   local ever=gs:CanEverAppointGovernor(entry.def.Hash)
   local now=gs:CanAppointGovernor(entry.def.Hash)
   assert(ever==false and now==false,'native unjoined qualification open or unknown: '..id..'/'..entry.row.LeaderType)
   pairs=pairs+1
  end
 end
end
assert(ai==21 and pairs==462,'AI qualification coverage differs')
return 'turn='..turn..'|ai='..ai..'|persons='..#people..'|negative='..pairs..'|titles='..table.concat(balances,',')
"#;

fn original() -> Value {
    serde_json::from_str(ORIGINAL).expect("preserved original scenario")
}

fn read(require_budget: bool) -> String {
    let source = original();
    let mut expected = String::from("local expected={\n");
    for player in source["steps"][1]["players"].as_array().unwrap() {
        // These are fixed identifiers from the original checked-in contract.
        expected.push_str(&format!(
            "{{leader='{}',civ='{}',human={}}},\n",
            player["leader"].as_str().unwrap(),
            player["civ"].as_str().unwrap(),
            player["human"].as_bool().unwrap()
        ));
    }
    format!("{expected}}}\nlocal requireBudget={require_budget}\n{READ}")
}

fn audit(require_budget: bool) -> Value {
    json!({"action":"ui_lua","state_name":"InGame","code":read(require_budget)})
}

fn scenario() -> Value {
    let source = original();
    let original_steps = source["steps"].as_array().unwrap();
    let budget: Vec<_> = original_steps
        .iter()
        .filter(|s| {
            s["code"]
                .as_str()
                .is_some_and(|c| c.contains("21 AI budgets required"))
        })
        .collect();
    assert_eq!(budget.len(), 1, "original AI budget operation changed");
    // Copy only the original launch/configuration, not the three joins.
    let mut steps = original_steps[..4].to_vec();
    steps.push(audit(false));
    steps.push(budget[0].clone());
    steps.push(audit(true));
    steps.push(json!({"action":"ui_lua","state_name":"InGame","code":"JAC407_before=Game.GetCurrentGameTurn(); return 'actual turn baseline='..JAC407_before"}));
    steps.push(json!({"action":"end_turn","timeout":300}));
    steps.push(json!({"action":"ui_lua","state_name":"InGame","code":"assert(type(JAC407_before)=='number' and Game.GetCurrentGameTurn()==JAC407_before+1,'not exactly one actual game turn'); return 'one actual turn observed'"}));
    let mut before_save = audit(true);
    before_save["capture_as"] = json!("JAC407_UNJOINED_AI");
    steps.push(before_save);
    steps.push(json!({"action":"save","name":"JAC-ToDo407-Unjoined-AI","timeout":120}));
    steps.push(json!({"action":"exit_to_menu"}));
    steps.push(json!({"action":"load","name":"JAC-ToDo407-Unjoined-AI","timeout":300}));
    steps.push(original_steps[3].clone());
    let mut restored = audit(true);
    restored["equals_capture"] = json!("JAC407_UNJOINED_AI");
    steps.push(restored);
    steps.push(json!({"action":"ui_lua","state_name":"InGame","code":"JAC407_before=Game.GetCurrentGameTurn(); return 'restored actual turn baseline='..JAC407_before"}));
    steps.push(json!({"action":"end_turn","timeout":300}));
    steps.push(steps[9].clone());
    steps.push(audit(true));
    steps.push(json!({"action":"exit_to_menu"}));
    json!({"name":"ToDo407 normal-start unjoined rejection: 21 AI x 22 persons, actual turns before/after save-load; not joined admission, AI payment or original113 completion", "judge_profile":"generic","strict_lua_errors":true,"steps":steps})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args_os().nth(1).ok_or("pass output scenario path")?;
    let data = scenario();
    validate_scenario(&data)?;
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(&data)?))?;
    println!("21 steps; two actual turns; 462 native AI qualification pairs per UI audit; original113 unchanged");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    fn fixture(mutation: &str) -> Lua {
        let lua = Lua::new();
        // The 22 factions come from the fixed original configuration; native
        // getters have independent mutable answers for fault injection.
        let source = original();
        let config = source["steps"][1]["players"].as_array().unwrap();
        let identities = config
            .iter()
            .map(|p| {
                format!(
                    "{{leader='{}',civ='{}'}}",
                    p["leader"].as_str().unwrap(),
                    p["civ"].as_str().unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        lua.load(format!(r#"
local config={{{identities}}}; local rows={{}}; local defs={{}}
for id=0,21 do rows[#rows+1]={{LeaderType=config[id+1].leader,GovernorType='G'..id}}; defs['G'..id]={{Hash=id}} end
local function list(v) return function() local i=0; return function() i=i+1; return v[i] end end end
BAD_OWNER=21; BAD_PERSON=21; EVER=false; NOW=false; HAS=false; TOTAL=1; SPENT=0; HUMAN=false; UNIT_TYPE=7; PROP=nil
local alive={{}}; Players={{}}; PlayerConfigurations={{}}
for id=0,21 do
 local gs={{HasGovernor=function(_,hash) if id==BAD_OWNER and hash==BAD_PERSON then return HAS end; return false end,
 CanEverAppointGovernor=function(_,hash) if id==BAD_OWNER and hash==BAD_PERSON then return EVER end; return false end,
 CanAppointGovernor=function(_,hash) if id==BAD_OWNER and hash==BAD_PERSON then return NOW end; return false end,
 GetGovernorPoints=function() return TOTAL end,GetGovernorPointsSpent=function() return SPENT end}}
 local p={{GetID=function() return id end,IsHuman=function() return id==0 or (id==BAD_OWNER and HUMAN) end,
 IsAlive=function() return true end,IsMajor=function() return true end,GetGovernors=function() return gs end,
 GetUnits=function() return {{Members=function() local once=false; return function() if not once then once=true; return 1,{{GetType=function() return UNIT_TYPE end}} end end end}} end}}
 Players[id]=p; alive[#alive+1]=p
 PlayerConfigurations[id]={{GetLeaderTypeName=function() return config[id+1].leader end,GetCivilizationTypeName=function() return config[id+1].civ end}}
end
PlayerManager={{GetAliveMajors=function() return alive end}}
Game={{GetLocalPlayer=function() return 0 end,GetCurrentGameTurn=function() return 10 end,GetProperty=function() return PROP end}}
GameInfo={{JAC_LeaderPersonas=list(rows),Governors=defs,GreatPersonClasses={{GREAT_PERSON_CLASS_JAC_PERSONA={{UnitType='PERSON'}}}},Units={{PERSON={{Index=99}}}}}}
{mutation}
"#)).exec().unwrap();
        lua
    }

    #[test]
    fn native_denial_requires_all_462_answers_and_zero_unjoined_units() {
        let code = read(true);
        assert!(fixture("")
            .load(&code)
            .eval::<String>()
            .unwrap()
            .contains("negative=462"));
        for mutation in [
            "EVER=true",
            "NOW=true",
            "EVER=nil",
            "NOW=nil",
            "HAS=true",
            "HAS=nil",
            "UNIT_TYPE=99",
            "PROP=0",
            "TOTAL=0",
            "SPENT=2",
            "TOTAL=0/0",
            "HUMAN=true",
            "Players[21]=nil",
            "GameInfo.Governors.G21=nil",
        ] {
            assert!(
                fixture(mutation).load(&code).eval::<String>().is_err(),
                "accepted {mutation}"
            );
        }
    }

    #[test]
    fn initial_probe_does_not_confuse_missing_titles_with_closed_admission() {
        assert!(fixture("TOTAL=0")
            .load(read(false))
            .eval::<String>()
            .is_ok());
        assert!(fixture("TOTAL=0;EVER=true")
            .load(read(false))
            .eval::<String>()
            .is_err());
    }

    #[test]
    fn save_snapshot_preserves_actual_turn_and_each_native_balance() {
        let code = read(true);
        let before = fixture("").load(&code).eval::<String>().unwrap();
        for mutation in [
            "Game.GetCurrentGameTurn=function() return 11 end",
            "TOTAL=2",
            "TOTAL=2;SPENT=1",
        ] {
            let after = fixture(mutation).load(&code).eval::<String>().unwrap();
            assert_ne!(before, after, "snapshot hid {mutation}");
        }
    }

    #[test]
    fn normal_probe_has_no_native_install_join_test_flag_or_title_setter() {
        let data = scenario();
        validate_scenario(&data).unwrap();
        let steps = data["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 21);
        assert_eq!(
            steps.iter().filter(|s| s["action"] == "end_turn").count(),
            2
        );
        assert_eq!(steps.iter().filter(|s| s["action"] == "save").count(), 1);
        assert_eq!(steps[11]["name"], steps[13]["name"]);
        assert_eq!(steps[10]["capture_as"], steps[15]["equals_capture"]);
        assert_eq!(steps[..4], original()["steps"].as_array().unwrap()[..4]);
        let codes = steps.iter().filter_map(|s| s["code"].as_str());
        for code in codes {
            Lua::new().load(code).into_function().unwrap();
            for forbidden in [
                "JAC_TEST",
                "BridgeRuntimeInstall",
                "JAC_RequestLeaderJoin",
                "RequestPlayerOperation",
                "ChangeGovernorPoints",
                "PrepareEvolution",
                "AppointGovernor",
            ] {
                // Getter names contain AppointGovernor; only an invocation of
                // the mutator itself is prohibited.
                let forbidden = if forbidden == "AppointGovernor" {
                    ":AppointGovernor("
                } else {
                    forbidden
                };
                assert!(!code.contains(forbidden), "contains {forbidden}");
            }
        }
    }
}
