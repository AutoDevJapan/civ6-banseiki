//! Explicit runner-only resident runtime lifecycle. The original scenario
//! actions and assertions remain unchanged. Loads use a new owned process;
//! this deliberately supplies no claim about an in-process teardown hook.
use super::*;

const READY: &str = r#"
assert(Game.GetLocalPlayer()==0 and Players[0]:IsTurnActive(),'requires human slot zero at own turn')
local api=assert(ExposedMembers.JAC_LeaderPersona,'persona Gameplay unavailable')
local count=0
for _,key in ipairs({'JAC_TEST853_ADMISSION_ACTIVE','JAC_TEST853_ADMISSION_MANUAL_APPOINT','JAC_TEST853_PAID_ASSIGNMENT_V1','JAC_TEST324_EVENTS_PROBE'}) do
 assert(Game:GetProperty(key)==nil,'private flag forbidden: '..key)
end
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 count=count+1
 local s=api.EventsGameplayStatus(p:GetID(),PlayerConfigurations[p:GetID()]:GetLeaderTypeName())
 assert(s.active and s.source=='civ6-persona-events-rs/v3/normal-Gameplay','normal acquisition ledger unavailable')
 assert(s.registrations.GovernorAppointed and s.registrations.GovernorPromoted,'normal subscriptions unavailable')
end
assert(count==22,'requires 22 live majors')
local n=0; for _ in GameInfo.JAC_LeaderPersonas() do n=n+1 end
assert(n==22,'requires 22 persona definitions')
return 'normal-v3-ready'
"#;

const REFRESH: &str = "UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LeaderPersonaRepublish'}); return 'one normal refresh requested'";

// Independent native UI getters, not the producer's 901xxx property cache.
// Joined positives are subsequently required by the unmodified scenario.
const AUDIT: &str = r#"
local rows={}; local count=0
for row in GameInfo.JAC_LeaderPersonas() do
 local def=assert(GameInfo.Governors[row.GovernorType]); local owner=Game:GetProperty('JAC_PERSONA_'..row.LeaderType..'_OWNER')
 for _,p in ipairs(PlayerManager.GetAliveMajors()) do
  local gs=p:GetGovernors(); local allowed=gs:CanEverAppointGovernor(def.Hash)
  assert(type(allowed)=='boolean','native qualification unavailable')
  if owner~=p:GetID() then assert(allowed==false,'foreign or unjoined native qualification allowed') end
  rows[#rows+1]=table.concat({p:GetID(),row.LeaderType,def.Index,tostring(allowed)},':'); count=count+1
 end
end
assert(count==484,'incomplete native qualification matrix'); table.sort(rows)
return 'turn='..Game.GetCurrentGameTurn()..'|rows=484|'..table.concat(rows,',')
"#;

impl Runner {
    pub(super) fn validate_persona_runtime_mode(&self) -> Result<()> {
        if !self.persona_runtime_restart {
            return Ok(());
        }
        if scenario::SessionType::from_scenario(&self.scenario)?
            != scenario::SessionType::SinglePlayer
        {
            return Err(Error::scenario(
                "resident restart diagnostic requires a single-player session",
            ));
        }
        if self.test_mode || self.owner_start || self.scenario.get("test_mode").is_some() {
            return Err(Error::scenario(
                "resident restart diagnostic requires automatic, non-test execution",
            ));
        }
        if self.scenario["steps"].as_array().is_some_and(|steps| {
            steps.iter().any(|step| {
                step["action"].as_str().is_some_and(|action| {
                    action.starts_with("start_native_") || action.starts_with("stop_native_")
                })
            })
        }) {
            return Err(Error::scenario(
                "resident restart diagnostic cannot overlap explicit native observations",
            ));
        }
        Ok(())
    }
    fn persona_transition(&self, row: Value) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.output.join("persona-runtime-transitions.jsonl"))?;
        writeln!(file, "{row}")?;
        file.flush()?;
        Ok(())
    }

    pub(super) fn activate_persona_runtime(&mut self, deadline: f64) -> Result<()> {
        if !self.persona_runtime_restart {
            return Ok(());
        }
        self.validate_persona_runtime_mode()?;
        if self.test_mode || self.owner_start {
            return Err(Error::scenario(
                "resident restart diagnostic requires automatic, non-test single-player execution",
            ));
        }
        if self.resident_runtime_identity.is_some() {
            return Err(Error::scenario(
                "resident process must exit before saved-game load",
            ));
        }
        let before = self
            .backend
            .single_process(self.budget(deadline, 5.0)?)?
            .ok_or_else(|| Error::input("persona runtime game absent"))?;
        let ready = self.lua("InGame", READY, self.budget(deadline, 30.0)?)?;
        if ready != "normal-v3-ready" {
            return Err(Error::scenario("normal acquisition preflight differs"));
        }
        if self.backend.single_process(self.budget(deadline, 5.0)?)? != Some(before) {
            return Err(Error::input(
                "persona preflight process changed; install refused",
            ));
        }
        let generation = self
            .persona_runtime_generation
            .checked_add(1)
            .ok_or_else(|| Error::scenario("resident generation exhausted"))?;
        let name = format!("persona-runtime-{generation:04}");
        self.backend
            .start_native_bridge(&self.output, &name, 1, "paid-assignment-runtime")?;
        let identity = self
            .backend
            .native_bridge_identity()
            .ok_or_else(|| Error::input("resident install did not pin game identity"))?;
        self.resident_runtime_identity = Some(identity);
        if identity.0 != before.0 || self.backend.validate_native_bridge_target()? != Some(identity)
        {
            return Err(Error::input(
                "persona runtime installed target differs from preflight",
            ));
        }
        let artifact = self
            .backend
            .stop_native_bridge()?
            .ok_or_else(|| Error::input("resident observation receipt missing"))?;
        self.persona_runtime_generation = generation;
        // One formal refresh; never repeat a payment, join or evolution.
        self.lua("InGame", REFRESH, self.budget(deadline, 10.0)?)?;
        self.wait(&json!({"type":"lua","state_name":"InGame","code":"return not UI.IsGameCoreBusy() and Players[0]:IsTurnActive()"}),self.remaining(deadline)?,false)?;
        let raw = self.lua("InGame", AUDIT, self.budget(deadline, 30.0)?)?;
        if self.backend.validate_native_bridge_target()? != Some(identity) {
            return Err(Error::input("persona native audit process changed"));
        }
        self.persona_transition(json!({"kind":"installed","generation":generation,
            "pid":identity.0,"created_filetime":identity.1,"artifact":artifact,
            "native_matrix":raw,"refresh_requests":1,"same_process_reload":false,
            "product_autostart":false}))?;
        Ok(())
    }

    pub(super) fn retire_persona_process(&mut self, deadline: f64) -> Result<()> {
        let identity = self
            .resident_runtime_identity
            .ok_or_else(|| Error::input("resident process identity missing"))?;
        let (pid, started) = self
            .backend
            .single_process(self.budget(deadline, 5.0)?)?
            .ok_or_else(|| Error::input("resident process disappeared before renewal"))?;
        if pid != identity.0 || self.backend.validate_native_bridge_target()? != Some(identity) {
            return Err(Error::input("resident process changed before renewal"));
        }
        self.disconnect();
        // Direct termination avoids invoking an unverified GameCore teardown
        // callback while the resident entries are live. Backend rechecks the
        // process creation identity, and only this live_chain owner's target.
        self.backend.terminate(pid, started)?;
        if !self
            .backend
            .wait_exit(pid, deadline.min(self.clock.now() + 10.0))?
        {
            return Err(Error::input(
                "resident process exit not confirmed; reload refused",
            ));
        }
        self.backend.finish_native_process_exit()?;
        self.resident_runtime_identity = None;
        self.persona_transition(
            json!({"kind":"process_exit","generation":self.persona_runtime_generation,
            "pid":identity.0,"created_filetime":identity.1,"confirmed":true,
            "same_process_reload":false}),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;
    use std::sync::Mutex;

    fn model() -> Lua {
        let lua = Lua::new();
        lua.load(r#"
properties={}; status={active=true,source='civ6-persona-events-rs/v3/normal-Gameplay',registrations={GovernorAppointed=true,GovernorPromoted=true}}
Game={GetLocalPlayer=function() return 0 end,GetCurrentGameTurn=function() return 2 end,GetProperty=function(_,key) return properties[key] end}
rows={}; people={}; Players={}; PlayerConfigurations={}; native={}
for id=0,21 do
 local row={LeaderType='LEADER_'..id,GovernorType='GOV_'..id}; rows[#rows+1]=row
 local player={GetID=function() return id end,IsTurnActive=function() return true end,GetGovernors=function() return {CanEverAppointGovernor=function(_,hash) return native[id..':'..hash] or false end} end}
 people[#people+1]=player; Players[id]=player; PlayerConfigurations[id]={GetLeaderTypeName=function() return row.LeaderType end}
end
GameInfo={Governors={}}
for index,row in ipairs(rows) do GameInfo.Governors[row.GovernorType]={Hash=1000+index,Index=index} end
GameInfo.JAC_LeaderPersonas=function() local n=0; return function() n=n+1; return rows[n] end end
PlayerManager={GetAliveMajors=function() return people end}
ExposedMembers={JAC_LeaderPersona={EventsGameplayStatus=function() return status end}}
"#).exec().unwrap();
        lua
    }

    #[test]
    fn native_matrix_checks_all_owners_and_rejects_foreign_permission_and_unknown_getter() {
        let lua = model();
        assert_eq!(lua.load(READY).eval::<String>().unwrap(), "normal-v3-ready");
        assert!(lua
            .load(AUDIT)
            .eval::<String>()
            .unwrap()
            .starts_with("turn=2|rows=484|"));
        lua.load("properties.JAC_PERSONA_LEADER_0_OWNER=0; native['0:1001']=true")
            .exec()
            .unwrap();
        assert!(lua
            .load(AUDIT)
            .eval::<String>()
            .unwrap()
            .contains("0:LEADER_0:1:true"));
        lua.load("native['1:1001']=true").exec().unwrap();
        assert!(lua
            .load(AUDIT)
            .eval::<String>()
            .unwrap_err()
            .to_string()
            .contains("foreign or unjoined"));
        lua.load("native['1:1001']=nil; native['2:1001']=1")
            .exec()
            .unwrap();
        assert!(lua
            .load(AUDIT)
            .eval::<String>()
            .unwrap_err()
            .to_string()
            .contains("native qualification unavailable"));
        let lua = model();
        lua.load("rows[22]=nil").exec().unwrap();
        assert!(lua.load(AUDIT).eval::<String>().is_err());
    }

    #[test]
    fn ledger_subscription_or_private_flag_failure_prevents_install_preflight() {
        for mutation in [
            "status.active=false",
            "status.source='UI'",
            "status.registrations.GovernorAppointed=false",
            "status.registrations.GovernorPromoted=false",
            "people[22]=nil",
            "rows[22]=nil",
        ] {
            let lua = model();
            lua.load(mutation).exec().unwrap();
            assert!(lua.load(READY).eval::<String>().is_err(), "{mutation}");
        }
        for key in [
            "JAC_TEST853_ADMISSION_ACTIVE",
            "JAC_TEST853_ADMISSION_MANUAL_APPOINT",
            "JAC_TEST853_PAID_ASSIGNMENT_V1",
            "JAC_TEST324_EVENTS_PROBE",
        ] {
            for value in ["false", "1"] {
                let lua = model();
                lua.load(format!("properties['{key}']={value}"))
                    .exec()
                    .unwrap();
                assert!(lua.load(READY).eval::<String>().is_err(), "{key}={value}");
            }
        }
    }

    struct LifecycleBackend {
        identity: Option<(u32, u64)>,
        exited: bool,
        ready: bool,
        calls: Arc<Mutex<Vec<String>>>,
    }
    impl Backend for LifecycleBackend {
        fn single_process(&mut self, _: f64) -> Result<Option<(u32, f64)>> {
            Ok(Some((7, 1.0)))
        }
        fn require_single_process(&mut self) -> Result<()> {
            Ok(())
        }
        fn list_states(&mut self, _: f64) -> Result<Vec<String>> {
            Ok(vec!["InGame".into()])
        }
        fn run_lua(&mut self, _: &str, code: &str, _: f64) -> Result<String> {
            if code == READY {
                Ok(if self.ready {
                    "normal-v3-ready"
                } else {
                    "inactive"
                }
                .into())
            } else {
                self.calls.lock().unwrap().push("lua".into());
                Ok("true".into())
            }
        }
        fn validate_native_bridge_target(&mut self) -> Result<Option<(u32, u64)>> {
            Ok(self.identity)
        }
        fn native_bridge_identity(&self) -> Option<(u32, u64)> {
            self.identity
        }
        fn start_native_bridge(&mut self, _: &Path, _: &str, _: u64, _: &str) -> Result<()> {
            self.calls.lock().unwrap().push("install".into());
            Ok(())
        }
        fn terminate(&mut self, pid: u32, started: f64) -> Result<()> {
            assert_eq!((pid, started), (7, 1.0));
            self.calls.lock().unwrap().push("terminate".into());
            Ok(())
        }
        fn wait_exit(&mut self, _: u32, _: f64) -> Result<bool> {
            self.calls.lock().unwrap().push("wait_exit".into());
            Ok(self.exited)
        }
        fn finish_native_process_exit(&mut self) -> Result<()> {
            self.calls.lock().unwrap().push("release".into());
            Ok(())
        }
    }

    fn runner(
        dir: &Path,
        identity: Option<(u32, u64)>,
        exited: bool,
        ready: bool,
        calls: Arc<Mutex<Vec<String>>>,
    ) -> Runner {
        Runner::with_backend(
            json!({"name":"fixture","steps":[]}),
            dir.into(),
            dir.into(),
            dir.into(),
            4318,
            Box::new(LifecycleBackend {
                identity,
                exited,
                ready,
                calls,
            }),
            Arc::new(SystemClock),
        )
    }

    #[test]
    fn disabled_mode_or_failed_normal_preflight_never_mutates_native_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let calls = Arc::new(Mutex::new(vec![]));
        let mut r = runner(dir.path(), Some((7, 123)), true, false, Arc::clone(&calls));
        r.activate_persona_runtime(r.clock.now() + 30.0).unwrap();
        r.persona_runtime_restart = true;
        assert!(r.activate_persona_runtime(r.clock.now() + 30.0).is_err());
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn restart_mode_rejects_fast_forward_owner_start_and_overlapping_observers_before_run() {
        let dir = tempfile::tempdir().unwrap();
        let calls = Arc::new(Mutex::new(vec![]));
        let mut r = runner(dir.path(), Some((7, 123)), true, true, Arc::clone(&calls));
        r.persona_runtime_restart = true;
        r.validate_persona_runtime_mode().unwrap();
        r.test_mode = true;
        assert!(r.validate_persona_runtime_mode().is_err());
        r.test_mode = false;
        r.owner_start = true;
        assert!(r.validate_persona_runtime_mode().is_err());
        r.owner_start = false;
        r.scenario["test_mode"] = json!({"skipped_turns":0});
        assert!(r.validate_persona_runtime_mode().is_err());
        r.scenario.as_object_mut().unwrap().remove("test_mode");
        r.scenario["session_type"] = "hotseat".into();
        assert!(r.validate_persona_runtime_mode().is_err());
        assert!(r.activate_persona_runtime(r.clock.now() + 30.0).is_err());
        r.scenario.as_object_mut().unwrap().remove("session_type");
        r.scenario["steps"] = json!([{"action":"start_native_paid_runtime"}]);
        assert!(r.validate_persona_runtime_mode().is_err());
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn changed_process_identity_or_unconfirmed_exit_cannot_release_resident_ownership() {
        for identity in [None, Some((8, 123)), Some((7, 124))] {
            let dir = tempfile::tempdir().unwrap();
            let calls = Arc::new(Mutex::new(vec![]));
            let mut r = runner(dir.path(), identity, true, true, Arc::clone(&calls));
            r.resident_runtime_identity = Some((7, 123));
            assert!(r.retire_persona_process(r.clock.now() + 30.0).is_err());
            assert!(calls.lock().unwrap().is_empty());
            assert_eq!(r.resident_runtime_identity, Some((7, 123)));
        }
        let dir = tempfile::tempdir().unwrap();
        let calls = Arc::new(Mutex::new(vec![]));
        let mut r = runner(dir.path(), Some((7, 123)), false, true, Arc::clone(&calls));
        r.resident_runtime_identity = Some((7, 123));
        assert!(r.retire_persona_process(r.clock.now() + 30.0).is_err());
        assert_eq!(*calls.lock().unwrap(), ["terminate", "wait_exit"]);
        assert_eq!(r.resident_runtime_identity, Some((7, 123)));
    }

    #[test]
    fn confirmed_process_exit_releases_runtime_and_records_original_creation_identity() {
        let dir = tempfile::tempdir().unwrap();
        let calls = Arc::new(Mutex::new(vec![]));
        let mut r = runner(dir.path(), Some((7, 123)), true, true, Arc::clone(&calls));
        r.resident_runtime_identity = Some((7, 123));
        r.retire_persona_process(r.clock.now() + 30.0).unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            ["terminate", "wait_exit", "release"]
        );
        assert_eq!(r.resident_runtime_identity, None);
        let v: Value = serde_json::from_str(
            &fs::read_to_string(dir.path().join("persona-runtime-transitions.jsonl")).unwrap(),
        )
        .unwrap();
        assert_eq!(v["created_filetime"], 123);
        assert_eq!(v["confirmed"], true);
        assert_eq!(v["same_process_reload"], false);
    }
}
