//! Observe an already-paid saved Governor without replaying any operation.
use clap::Parser;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::PathBuf};

const ORIGINAL: &str = include_str!("../../civ6-autotest/scenarios/leader-persona-15km.json");
// Telemetry may stop after 300s, but the controller owns both bounded reads.
const CONTROLLER_SECONDS: u32 = 2 * 300 + 2 * 60 + 2 * 30 + 60 + 45 + 60;
const READY: &str = r#"
local p=Players[0]; local api=ExposedMembers.JAC_LeaderPersona
if not p or not p:IsTurnActive() or UI.IsGameCoreBusy() or UI.IsProcessingMessages()
 or type(api)~='table' or type(api.EventsGameplayStatus)~='function' then return false end
local s=api.EventsGameplayStatus(0,'LEADER_JAC_DATE_TANEMUNE')
return type(s)=='table' and s.active==true
 and s.source=='civ6-persona-events-rs/v3/normal-Gameplay'
 and type(s.registrations)=='table'
 and s.registrations.GovernorAppointed==true and s.registrations.GovernorPromoted==true
"#;
const READ: &str = r#"
local leader='LEADER_JAC_DATE_TANEMUNE'
local row=assert(GameInfo.JAC_LeaderPersonas[leader])
local def=assert(GameInfo.Governors[row.GovernorType])
local p=assert(Players[0]); local gs=p:GetGovernors(); local g=assert(gs:GetGovernor(def.Hash))
assert(Game.GetLocalPlayer()==0 and g:GetType()==def.Index and g:GetOwner()==0,'saved person differs')
assert(Game:GetProperty('JAC_PERSONA_'..leader..'_STATE')=='GOVERNOR','saved state differs')
local cost=Game:GetProperty('JAC_PERSONA_'..leader..'_EVOLUTION_COST')
assert(cost==2 and gs:GetGovernorPointsSpent()==2,'saved payment differs')
local out={leader=leader,index=def.Index,owner=g:GetOwner(),cost=cost,total=gs:GetGovernorPoints(),spent=gs:GetGovernorPointsSpent(),promotions={},cities={}}
for n=0,1 do
 local key=row.BasePromotionType; if n>0 then key=key..'_TITLE_'..n end
 local d=assert(GameInfo.GovernorPromotions[key]); local owned=g:HasPromotion(d.Hash)
 assert(owned==true,'saved payment promotion absent')
 out.promotions[#out.promotions+1]={index=d.Index,hash=d.Hash,owned=owned}
end
local a=assert(Game:GetProperty('JAC_PERSONA_SCENARIO_CITY_A'))
local city=assert(CityManager.GetCity(0,a),'saved city A absent')
out.cityA=a; out.canAssign=gs:CanAssignGovernor(def.Hash,city)
assert(type(out.canAssign)=='boolean','assignment query missing')
if expected_can_assign~=nil then assert(out.canAssign==expected_can_assign,'paid placement permission was not restored') end
local assigned=g:GetAssignedCity(); out.assignedCity=assigned and assigned:GetID() or 'none'
out.established=g:IsEstablished(); out.turnsOnSite=g:GetTurnsOnSite()
for _,c in p:GetCities():Members() do out.cities[#out.cities+1]={id=c:GetID(),owner=c:GetOwner()} end
table.sort(out.cities,function(x,y) return x.id<y.id end)
local api=assert(ExposedMembers.JAC_LeaderPersona)
out.gameplay=assert(api.EventsGameplayStatus(0,leader))
local function dump(v,seen)
 if type(v)~='table' then return type(v)=='string' and string.format('%q',v) or tostring(v) end
 assert(not seen[v],'cyclic status'); seen[v]=true
 local keys={}; for k in pairs(v) do keys[#keys+1]=k end
 table.sort(keys,function(x,y) return tostring(x)<tostring(y) end)
 local parts={}; for _,k in ipairs(keys) do parts[#parts+1]='['..dump(k,seen)..']='..dump(v[k],seen) end
 seen[v]=nil; return '{'..table.concat(parts,',')..'}'
end
return dump(out,{})
"#;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    save: PathBuf,
    #[arg(long)]
    runtime: PathBuf,
    #[arg(long)]
    bootstrap: PathBuf,
    #[arg(long)]
    output: PathBuf,
    /// Require the paid Governor to be assignable on both loads, without assigning it.
    #[arg(long)]
    expect_assignable: bool,
}

fn read_code(expect_assignable: bool) -> String {
    format!(
        "local expected_can_assign={}\n{READ}",
        if expect_assignable { "true" } else { "nil" }
    )
}

fn digest(path: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

fn scene(
    name: &str,
    save_sha: &str,
    runtime: &str,
    runtime_sha: &str,
    bootstrap: &str,
    bootstrap_sha: &str,
    expect_assignable: bool,
) -> Value {
    let original: Value = serde_json::from_str(ORIGINAL).unwrap();
    let load =
        json!({"action":"load","name":name,"exact_name":true,"save_sha256":save_sha,"timeout":300});
    let wait = json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":READY}});
    let read = json!({"action":"ui_lua","state_name":"InGame","code":read_code(expect_assignable),"timeout":30});
    json!({"name":"Read-only paid Governor: cold saved-world load and same-process reload; not assignment acceptance","judge_profile":"generic","strict_lua_errors":true,"steps":[
        original["steps"][0],{"action":"restart_game"},
        {"action":"start_native_bootstrap","name":"paid-reload-native","max_seconds":300,"controller_seconds":CONTROLLER_SECONDS,"runtime_dll":runtime,"runtime_sha256":runtime_sha,"bootstrap_dll":bootstrap,"bootstrap_sha256":bootstrap_sha,"timeout":40},
        load,wait,read,{"action":"exit_to_menu","timeout":60},load,wait,read,
        {"action":"stop_native_bootstrap","timeout":45}
    ]})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let save = fs::canonicalize(&args.save)?;
    let name = save
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("save name is not UTF-8")?;
    let runtime = fs::canonicalize(&args.runtime)?;
    let bootstrap = fs::canonicalize(&args.bootstrap)?;
    let data = scene(
        name,
        &digest(&save)?,
        runtime.to_str().ok_or("runtime path is not UTF-8")?,
        &digest(&runtime)?,
        bootstrap.to_str().ok_or("bootstrap path is not UTF-8")?,
        &digest(&bootstrap)?,
        args.expect_assignable,
    );
    civ6_autotest_rs::scenario::validate_scenario(&data)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.output)?;
    output.write_all(format!("{}\n", serde_json::to_string_pretty(&data)?).as_bytes())?;
    println!("VALID: 11 steps; two hash-pinned reads of the same save; no payment, assignment, turn or save operation");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_requires_restored_normal_gameplay_and_both_official_subscriptions() {
        let lua = mlua::Lua::new();
        lua.load(
            r#"
            local function unexpected() error('readiness sent an operation') end
            PlayerOperations={EXECUTE_SCRIPT=1}; UnitManager={RequestOperation=unexpected}
            UI={IsGameCoreBusy=function() return false end,
                IsProcessingMessages=function() return false end, RequestPlayerOperation=unexpected}
            Players={[0]={IsTurnActive=function() return true end}}
            ExposedMembers={JAC_LeaderPersona={EventsGameplayStatus=function(owner,leader)
              assert(owner==0 and leader=='LEADER_JAC_DATE_TANEMUNE'); return status
            end}}
            "#,
        )
        .exec()
        .unwrap();
        for (status, ready) in [
            ("nil", false),
            ("{active=false,source='civ6-persona-events-rs/v3/normal-Gameplay'}", false),
            ("{active=true,source='diagnostic',registrations={GovernorAppointed=true,GovernorPromoted=true}}", false),
            ("{active=true,source='civ6-persona-events-rs/v3/normal-Gameplay'}", false),
            ("{active=true,source='civ6-persona-events-rs/v3/normal-Gameplay',registrations={GovernorAppointed=true,GovernorPromoted=false}}", false),
            ("{active=true,source='civ6-persona-events-rs/v3/normal-Gameplay',registrations={GovernorAppointed=false,GovernorPromoted=true}}", false),
            ("{active=true,source='civ6-persona-events-rs/v3/normal-Gameplay',registrations={GovernorAppointed=true,GovernorPromoted=true}}", true),
        ] {
            lua.load(format!("status={status}")).exec().unwrap();
            assert_eq!(lua.load(READY).eval::<bool>().unwrap(), ready, "{status}");
        }
        lua.load("UI.IsGameCoreBusy=function() return true end")
            .exec()
            .unwrap();
        assert!(!lua.load(READY).eval::<bool>().unwrap());
    }

    #[test]
    fn saved_input_is_pinned_twice_and_no_operation_is_replayed() {
        let root = std::env::temp_dir();
        let r = root.join("r.dll");
        let b = root.join("b.dll");
        let s = scene(
            "paid-save",
            &"a".repeat(64),
            r.to_str().unwrap(),
            &"b".repeat(64),
            b.to_str().unwrap(),
            &"c".repeat(64),
            false,
        );
        civ6_autotest_rs::scenario::validate_scenario(&s).unwrap();
        assert_eq!(s["steps"].as_array().unwrap().len(), 11);
        let (telemetry, controller) =
            civ6_bootstrap_rs::protocol::Request::durations(&s["steps"][2]).unwrap();
        let post_prepare_budget: u32 = s["steps"].as_array().unwrap()[3..]
            .iter()
            .map(|step| step["timeout"].as_u64().unwrap() as u32)
            .sum();
        assert_eq!(telemetry, 300);
        assert_eq!(controller, post_prepare_budget + 60);
        assert_eq!(s["steps"][3], s["steps"][7]);
        assert_eq!(s["steps"][3]["save_sha256"], "a".repeat(64));
        assert_eq!(s["steps"][3]["exact_name"], true);
        for load in [3, 7] {
            assert_eq!(s["steps"][load]["action"], "load");
            assert_eq!(s["steps"][load + 1]["action"], "wait_for");
            assert_eq!(s["steps"][load + 1]["condition"]["code"], READY);
            assert_eq!(s["steps"][load + 2]["action"], "ui_lua");
        }
        for step in s["steps"].as_array().unwrap() {
            assert!(!matches!(
                step["action"].as_str(),
                Some("save" | "end_turn" | "start_game")
            ));
        }
        for denied in [
            "RequestPlayerOperation",
            "SetProperty",
            "PrepareEvolution",
            "JAC_EvolveLeader",
            "JAC_AssignLeader",
            "ChangeGovernorPoints",
            "SyncProductionAdmission",
        ] {
            assert!(
                !READ.contains(denied) && !READY.contains(denied),
                "{denied}"
            );
        }
    }

    #[test]
    fn real_lua_observes_both_native_decisions_without_accepting_bad_payment() {
        let lua = mlua::Lua::new();
        lua.load(r#"
local g={GetType=function() return 6 end,GetOwner=function() return 0 end,HasPromotion=function() return PAID end,GetAssignedCity=function() return nil end,IsEstablished=function() return false end,GetTurnsOnSite=function() return 0 end}
local city={GetID=function() return 7 end,GetOwner=function() return 0 end}
local gs={GetGovernor=function() return g end,GetGovernorPointsSpent=function() return 2 end,GetGovernorPoints=function() return 2 end,CanAssignGovernor=function() QUERIES=QUERIES+1;return ALLOWED end}
local cities={Members=function() return ipairs({city}) end}
Players={[0]={GetGovernors=function() return gs end,GetCities=function() return cities end}}
GameInfo={JAC_LeaderPersonas={LEADER_JAC_DATE_TANEMUNE={GovernorType='DATE',BasePromotionType='BASE'}},Governors={DATE={Index=6,Hash=106}},GovernorPromotions={BASE={Index=10,Hash=110},BASE_TITLE_1={Index=11,Hash=111}}}
Game={GetLocalPlayer=function() return 0 end,GetProperty=function(_,k) if k=='JAC_PERSONA_SCENARIO_CITY_A' then return 7 elseif k:match('_STATE$') then return 'GOVERNOR' elseif k:match('_EVOLUTION_COST$') then return 2 else error(k) end end}
CityManager={GetCity=function(o,id) assert(o==0 and id==7);return city end}
ExposedMembers={JAC_LeaderPersona={EventsGameplayStatus=function(o,l) assert(o==0 and l=='LEADER_JAC_DATE_TANEMUNE');return {active=true,paid=2,account={spent=2}} end}}
PAID=true;ALLOWED=false;QUERIES=0
"#).exec().unwrap();
        let denied = lua.load(READ).eval::<String>().unwrap();
        assert!(denied.contains("[\"canAssign\"]=false"));
        lua.load("ALLOWED=true").exec().unwrap();
        let allowed = lua.load(READ).eval::<String>().unwrap();
        assert!(allowed.contains("[\"canAssign\"]=true"));
        assert_eq!(lua.globals().get::<u32>("QUERIES").unwrap(), 2);
        lua.load("ALLOWED=false").exec().unwrap();
        assert!(lua
            .load(read_code(true))
            .eval::<String>()
            .unwrap_err()
            .to_string()
            .contains("paid placement permission was not restored"));
        lua.load("ALLOWED=true").exec().unwrap();
        assert!(lua
            .load(read_code(true))
            .eval::<String>()
            .unwrap()
            .contains("[\"canAssign\"]=true"));
        assert_eq!(lua.globals().get::<u32>("QUERIES").unwrap(), 4);
        lua.load("PAID=false").exec().unwrap();
        assert!(lua
            .load(READ)
            .eval::<String>()
            .unwrap_err()
            .to_string()
            .contains("saved payment promotion absent"));
        assert_eq!(lua.globals().get::<u32>("QUERIES").unwrap(), 4);
    }
}
