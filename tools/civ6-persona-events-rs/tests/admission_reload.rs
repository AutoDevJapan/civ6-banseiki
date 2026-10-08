use civ6_persona_events_rs::admission_reload;
use mlua::Lua;
use serde_json::Value;

fn template() -> Value {
    serde_json::from_str(include_str!(
        "../../civ6-autotest/scenarios/leader-persona-15km.json"
    ))
    .unwrap()
}
fn build(input: &Value) -> Result<Value, String> {
    let p = std::env::temp_dir();
    admission_reload::build(
        input,
        p.join("runtime.dll").to_str().unwrap(),
        &"a".repeat(64),
        p.join("bootstrap.dll").to_str().unwrap(),
        &"b".repeat(64),
    )
}

#[test]
fn one_process_two_worlds_and_normal_ui_are_required() {
    let input = template();
    let scene = build(&input).unwrap();
    civ6_autotest_rs::scenario::validate_scenario(&scene).unwrap();
    let steps = scene["steps"].as_array().unwrap();
    assert_eq!(
        steps
            .iter()
            .filter(|s| s["action"] == "restart_game")
            .count(),
        1
    );
    assert_eq!(
        steps
            .iter()
            .filter(|s| s["action"] == "start_native_bootstrap")
            .count(),
        1
    );
    assert_eq!(steps[0], input["steps"][0]);
    assert_eq!(steps[2], input["steps"][1]);
    assert_eq!(steps[3]["action"], "start_native_bootstrap");
    assert_eq!(steps[4], input["steps"][2]);
    let names: Vec<_> = steps
        .iter()
        .filter(|s| s["action"] == "load")
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [admission_reload::CLOSED_SAVE, admission_reload::PAID_SAVE]
    );
    assert_eq!(
        steps
            .iter()
            .filter(|s| s.get("equals_capture").is_some())
            .count(),
        4
    );
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
            "APPOINT_GOVERNOR",
            "PROMOTE_GOVERNOR",
            "OnStart=",
            "JAC_JoinedAdmission",
        ] {
            assert!(!code.contains(forbidden), "{forbidden}");
        }
    }
    assert_eq!(body.matches("JAC_RequestLeaderJoin(2)").count(), 1);
    assert_eq!(body.matches("JAC_EvolveLeader('").count(), 1);
    let last = steps.iter().rposition(|s| s["action"] == "load").unwrap();
    assert!(steps[last..]
        .iter()
        .all(|s| s["action"] != "restart_game" && s["action"] != "save"));
}

const FIXTURE: &str = r#"
paid=false; stale=false; wrongOwner=false; spentOverride=nil; missingReceipt=false; flag=false; merit=0.12345678901234567; initStage='committed'; initCandidate=false; forceAcquired=false; reservedOverride=0
local leader='LEADER_JAC_ASHINA_MORIKIYO'; local rows={}; local byName={}; local defs={}
for n=1,22 do
 local l=n==1 and leader or 'LEADER_TEST_'..n; local key='GOV_'..n
 local r={LeaderType=l,GovernorType=key,EvolutionCost=1,BasePromotionType='BASE',PersonType='PERSON'}
 rows[n]=r; byName[l]=r; defs[key]={Index=n,Hash=100+n}
end
setmetatable(byName,{__call=function() local n=0; return function() n=n+1; return rows[n] end end})
GameInfo={JAC_LeaderPersonas=byName,Governors=defs,GovernorPromotions={BASE={Index=7,Hash=107}},JAC_PersonaTreeNodes=function() return function() return nil end end}
local physical={GetOwner=function() return 0 end,GetType=function() return 1 end,HasPromotion=function() return true end}
Players={}; PlayerConfigurations={[2]={GetLeaderTypeName=function() return leader end}}
for owner=0,21 do
 local gs={GetGovernorPoints=function() return 1 end,GetGovernorPointsSpent=function() return spentOverride or (paid and 1 or 0) end,
 CanEverAppointGovernor=function(_,hash) return (paid and hash==101 and owner==0) or (stale and hash==101 and owner==0) or (wrongOwner and hash==101 and owner==1) end,
 HasGovernor=function(_,hash) return paid and hash==101 and owner==0 end,GetGovernor=function() return paid and physical or nil end}
 Players[owner]={GetGovernors=function() return gs end,GetUnits=function() return {Members=function() return function() return nil end end} end}
end
Game={GetLocalPlayer=function() return 0 end,GetCurrentGameTurn=function() return 1 end,GetProperty=function(_,key)
 if flag and key=='JAC_TEST853_ADMISSION_ACTIVE' then return 1 end
 if paid then
  local prefix='JAC_PERSONA_'..leader..'_'
  if key==prefix..'OWNER' then return 0 end; if key==prefix..'STATE' then return 'GOVERNOR' end
  if key==prefix..'SOURCE' then return 2 end; if key==prefix..'EVOLUTION_COST' then return 1 end
  if key==prefix..'MERIT' then return merit end
 end
end}
PlayerManager={GetAliveMajorIDs=function() local ids={}; for n=0,21 do ids[#ids+1]=n end; return ids end}
local function receipt(name) return {source='civ6-persona-events-rs/v3/normal-Gameplay',context='Gameplay',owner=0,leader=leader,
 accepted=true,governor=1,governorHash=101,name=name,promotion=7,spendingSerial=1,sequence=19} end
ExposedMembers={JAC_LeaderPersona={EventsGameplayStatus=function()
 return {source='civ6-persona-events-rs/v3/normal-Gameplay',active=true,reserved=reservedOverride,initialization={candidateCallOK=true,stage=initStage,candidate=initCandidate},commit={nativeClosedAll22=true,schema=3,context='Gameplay'},registrations={GovernorAppointed=true,GovernorPromoted=true},
 paid=paid and 1 or 0,acquired=(paid or forceAcquired) and not missingReceipt and {appointed=receipt('GovernorAppointed'),[1]=receipt('GovernorPromoted')} or nil,
 account={debits={{invalidated=false,confirmed={owner=0,afterSpent=1,expected=1}}}},reservation={owner=0,serial=0}}
end}}
"#;
fn run(paid: bool, mutation: &str) -> mlua::Result<String> {
    let lua = Lua::new();
    lua.load(FIXTURE).exec()?;
    lua.load(format!("paid={paid}; {mutation}")).exec()?;
    lua.load(admission_reload::snapshot(paid)).eval()
}
#[test]
fn stale_permission_and_wrong_owner_are_rejected_by_native_matrix() {
    assert!(run(false, "").is_ok());
    assert!(run(true, "").is_ok());
    assert!(run(false, "stale=true").is_err());
    assert!(run(true, "wrongOwner=true").is_err());
}
#[test]
fn payment_receipts_and_private_publishers_cannot_replace_normal_evidence() {
    for mutation in ["spentOverride=2", "missingReceipt=true", "flag=true"] {
        assert!(run(true, mutation).is_err(), "{mutation}");
    }
    assert!(run(false, "forceAcquired=true").is_err());
    assert!(run(false, "reservedOverride=1").is_err());
    assert_ne!(
        run(true, "").unwrap(),
        run(true, "merit=0.12345678901234568").unwrap(),
        "saved fractional merit must not be rounded away"
    );
}
#[test]
fn wrong_world_or_source_setup_is_rejected() {
    let mut input = template();
    input["steps"][1]["players"][2]["leader"] = "WRONG".into();
    assert!(build(&input).is_err());
    let mut input = template();
    input["steps"][1]["players"][1]["human"] = true.into();
    assert!(build(&input).is_err());
    let mut input = template();
    input["steps"][1]["ruleset"] = "WRONG".into();
    assert!(build(&input).is_err());
}

#[test]
fn cli_checks_actual_dll_bytes_and_never_overwrites_an_existing_scene() {
    use sha2::{Digest, Sha256};
    let base = std::env::temp_dir().join(format!(
        "admission1262-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let runtime = base.with_extension("runtime.dll");
    let bootstrap = base.with_extension("bootstrap.dll");
    let output = base.with_extension("json");
    struct Files(Vec<std::path::PathBuf>);
    impl Drop for Files {
        fn drop(&mut self) {
            for file in &self.0 {
                let _ = std::fs::remove_file(file);
            }
        }
    }
    let _owned = Files(vec![runtime.clone(), bootstrap.clone(), output.clone()]);
    std::fs::write(&runtime, b"finite generator fixture; never loaded").unwrap();
    std::fs::write(&bootstrap, b"finite bootstrap fixture; never loaded").unwrap();
    let runtime_sha = format!("{:x}", Sha256::digest(std::fs::read(&runtime).unwrap()));
    let bootstrap_sha = format!("{:x}", Sha256::digest(std::fs::read(&bootstrap).unwrap()));
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../civ6-autotest/scenarios/leader-persona-15km.json");
    let command = |sha: &str| {
        let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_civ6-persona-events-rs"));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x08000000);
        }
        c.arg("generate-admission-reload")
            .arg(&input)
            .arg(&runtime)
            .arg(sha)
            .arg(&bootstrap)
            .arg(&bootstrap_sha)
            .arg(&output)
            .output()
            .unwrap()
    };
    assert!(!command(&"0".repeat(64)).status.success());
    assert!(!output.exists());
    assert!(command(&runtime_sha).status.success());
    let generated = std::fs::read(&output).unwrap();
    let parsed: Value = serde_json::from_slice(&generated).unwrap();
    assert_eq!(
        parsed,
        admission_reload::build(
            &template(),
            runtime.to_str().unwrap(),
            &runtime_sha,
            bootstrap.to_str().unwrap(),
            &bootstrap_sha
        )
        .unwrap()
    );
    civ6_autotest_rs::scenario::validate_scenario(&parsed).unwrap();
    assert!(!command(&runtime_sha).status.success());
    assert_eq!(std::fs::read(&output).unwrap(), generated);
}

#[test]
fn paid_to_closed_transition_rejects_a_retained_old_permit() {
    let lua = Lua::new();
    lua.load(FIXTURE).exec().unwrap();
    let closed: String = lua.load(admission_reload::snapshot(false)).eval().unwrap();
    lua.load("paid=true").exec().unwrap();
    let paid: String = lua.load(admission_reload::snapshot(true)).eval().unwrap();
    lua.load("paid=false; stale=true").exec().unwrap();
    assert!(lua
        .load(admission_reload::snapshot(false))
        .eval::<String>()
        .is_err());
    lua.load("stale=false").exec().unwrap();
    assert_eq!(
        closed,
        lua.load(admission_reload::snapshot(false))
            .eval::<String>()
            .unwrap()
    );
    lua.load("paid=true").exec().unwrap();
    assert_eq!(
        paid,
        lua.load(admission_reload::snapshot(true))
            .eval::<String>()
            .unwrap()
    );
}

#[test]
fn readiness_accepts_restored_commit_but_never_waits_away_a_stale_gate() {
    let lua = Lua::new();
    lua.load(FIXTURE).exec().unwrap();
    lua.load("Players[0].IsTurnActive=function() return true end; UI={IsGameCoreBusy=function() return false end,IsProcessingMessages=function() return false end}").exec().unwrap();
    assert!(lua.load(admission_reload::READY).eval::<bool>().unwrap());
    assert!(lua
        .load(admission_reload::fresh_ready())
        .eval::<bool>()
        .unwrap());
    lua.load("initStage='candidate_ready'; initCandidate=true")
        .exec()
        .unwrap();
    assert!(!lua.load(admission_reload::READY).eval::<bool>().unwrap());
    lua.load("initStage='fresh_preconditions'; initCandidate=false; stale=true")
        .exec()
        .unwrap();
    assert!(lua.load(admission_reload::READY).eval::<bool>().unwrap());
    assert!(!lua
        .load(admission_reload::fresh_ready())
        .eval::<bool>()
        .unwrap());
    assert!(
        lua.load(admission_reload::snapshot(false))
            .eval::<String>()
            .is_err(),
        "fresh UI with retained native true must fail the first read"
    );
    lua.load("stale=false; paid=true").exec().unwrap();
    assert!(lua
        .load(admission_reload::paid_ready())
        .eval::<bool>()
        .unwrap());
    lua.load("missingReceipt=true").exec().unwrap();
    assert!(!lua
        .load(admission_reload::paid_ready())
        .eval::<bool>()
        .unwrap());
}
