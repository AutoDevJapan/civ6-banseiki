//! Bounded third-generation cases execute the Rust-generated connection in
//! Lua 5.1. They do not attest that production or capacity 3 works in Civ6.
#[path = "../src/spy_third.rs"]
mod spy_third;

use mlua::Lua;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct Third(Lua);
impl Third {
    fn run(&self, code: &str) {
        self.0.load(code).exec().unwrap();
    }

    fn reject(&self, code: &str, message: &str) {
        let error = self.0.load(code).exec().expect_err("must reject");
        assert!(error.to_string().contains(message), "{error}");
    }

    fn new() -> Self {
        let lua = Lua::new();
        assert_eq!(
            lua.load("return _VERSION").eval::<String>().unwrap(),
            "Lua 5.1"
        );
        lua.load(include_str!(
            "../../civ6-lua-tests-rs/fixtures/persona/boundary_engine.lua"
        ))
        .exec()
        .unwrap();
        lua.load("GameEvents.JAC_BoundaryThirdSpyStep=event()")
            .exec()
            .unwrap();
        lua.globals()
            .set(
                "include",
                lua.create_function(|lua, name: String| {
                    if name == "JAC_PersonaThirdSpy" {
                        lua.load(spy_third::connection()).exec()?;
                    }
                    Ok(())
                })
                .unwrap(),
            )
            .unwrap();
        for path in [
            "Shared/JAC_PersonaBoundaryRules.lua",
            "UI/JAC_PersonaBoundary_View.lua",
            "Scripts/JAC_PersonaBoundary_Fixture.lua",
        ] {
            lua.load(
                std::fs::read_to_string(root().join("poc/JAC_Test_PersonaBoundary").join(path))
                    .unwrap(),
            )
            .exec()
            .unwrap();
        }
        lua.load(include_str!(
            "../../civ6-lua-tests-rs/fixtures/persona/boundary_enemy_cases.lua"
        ))
        .exec()
        .unwrap();
        let t = Self(lua);
        t.run("ENEMY_CASES.setup()");
        for (action, event) in [
            ("PREPARE_ENEMY", "JAC_BoundaryPrepareEnemy"),
            ("EVOLVE_ENEMY", "JAC_BoundaryEvolveEnemy"),
        ] {
            t.legacy_sync(action, event);
        }
        t.run("ENEMY_CASES.appoint(true)");
        t.legacy_sync("CONFIRM_ENEMY", "JAC_BoundaryConfirmEnemy");
        t.run(
            r#"
            ENEMY_CASES.settled(); TURN=45; UNITS[0]={}; SPY_CAPACITY=3; CIVIC_COMPLETED=true;
            GameInfo.UnitOperations[59]=GameInfo.UnitOperations[0];
            GameInfo.Civics.CIVIC_JAC_SENGOKU_NANBAN_ENVOYS={Index=20,Hash=2020};
            local original=Players[0].GetCulture;
            Players[0].GetCulture=function() local c=original(); local has=c.HasCivic;
             c.HasCivic=function(self,id) if id==20 then return true end; return has(self,id) end;
             return c end;
            local plot=Map.GetPlot(FOREIGN.x,FOREIGN.y):GetIndex();
            HISTORY[0][0]={Operation=59,PlotIndex=plot,CompletionTurn=23,InitialResult=3,EscapeResult=0};
            HISTORY[0][1]={Operation=59,PlotIndex=plot,CompletionTurn=45,InitialResult=3,EscapeResult=0};
            Game:SetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY',{current=2,mode='CAPACITY_MAP',spyID=851976,
             missionRequested=true,priorUnknownField={nested={true,73,'retain all previous retry fields'}},
             archive={spyID=655368,eventID=0,eventTurn=23,targetOwner=1,targetID=20,governorNeutralized=0,
              operation=59,plot=plot,completionTurn=23,initial=3,escape=0}});
            Game:SetProperty('JAC_BOUNDARY_ENEMY_MISSION',{attacker=0,id=1,turn=45});
            Game:SetProperty('JAC_BOUNDARY_SPY',{owner=0,id=851976});
            Game:SetProperty('JAC_BOUNDARY_SPY_TRAVEL',{oldID=786440,nameText='JAC_TEXT_V1:LOC_UNIT_SPY_NAME',
             level=1,targetOwner=1,targetID=20,x=FOREIGN.x,y=FOREIGN.y});
            Game:SetProperty('JAC_BOUNDARY_SPY_BARRIER',{owner=0,spyID=851976,action='SPY_MISSION',token=12});
            Game:SetProperty('JAC_BOUNDARY_SPY_PRODUCTION',{owner=0,progressed=true,beforeCount=0});
            Game:SetProperty('JAC_BOUNDARY_THIRD_CIVIC_INTENT',{owner=0,status='RESERVED',token=20,turn=45,
             spyID=851976,eventID=1,eventTurn=45,civicIndex=20,civicHash=2020,beforeCapacity=2});
            CAPTURED[1]={{OwningPlayer=0,Name='',Level=1,NameIndex=-1},
                         {OwningPlayer=0,Name='',Level=1,NameIndex=-1}};
            OFF_MAP={{Name='',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999},
                     {Name='',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}};
            BEFORE_RETRY=JAC_PersonaBoundaryRules.Serialize(Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY'));
            BEFORE_HISTORY=JAC_PersonaBoundaryRules.Serialize(HISTORY);
            BEFORE_TRAVEL=JAC_PersonaBoundaryRules.Serialize(Game:GetProperty('JAC_BOUNDARY_SPY_TRAVEL'));
            -- Two independent VMs are represented by a UI-owned scalar read.
            -- Remove UI Governor getters from Core using the existing engine double.
            local raw=ExposedMembers.JAC_PersonaThirdSpyRead;
            local uiGovernors=Players[1].GetGovernors;
            ExposedMembers.JAC_PersonaThirdSpyRead=function()
             local saved=Players[1].GetGovernors; local wasCore=CORE_READ;
             Players[1].GetGovernors=uiGovernors; CORE_READ=false;
             local ok,value=pcall(raw); Players[1].GetGovernors=saved; CORE_READ=wasCore;
             if not ok then error(value) end; return value
            end;
            assert(ExposedMembers.JAC_PersonaBoundary.ThirdSpyStatus().physicalCount==2);
            "#,
        );
        t
    }

    fn legacy_sync(&self, action: &str, event: &str) {
        self.run(&format!(
            "local b=ExposedMembers.JAC_PersonaBoundary; b.Queue('{action}'); FRAME(); \
             local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='{event}'); \
             ENEMY_CASES.fireCore('{event}',r.params); FRAME(); \
             r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier'); \
             ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',r.params); \
             fire(Events.GameCoreEventPublishComplete); FRAME(); assert(b.lastAction=='ACCEPTED_{action}')"
        ));
    }

    fn complete_step(&self, action: &str) {
        self.run(&format!(
            "FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryThirdSpyStep' \
             and r.params.Action=='{action}' and r.params.Phase==nil); \
             ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',r.params); FRAME(); \
             r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryThirdSpyStep' and r.params.Phase=='BARRIER'); \
             ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',r.params); \
             fire(Events.GameCoreEventPublishComplete); FRAME()"
        ));
    }

    fn begin(&self) {
        self.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('BEGIN')");
        self.complete_step("BEGIN");
        self.run("assert(ExposedMembers.JAC_PersonaBoundary.thirdLast=='ACCEPTED_BEGIN')");
    }

    fn produce(&self) {
        self.begin();
        self.run(
            "fire(Events.SaveComplete); local b=ExposedMembers.JAC_PersonaBoundary; \
             b.ThirdSpySend('BUILD_SPY'); FRAME(); FRAME(); assert(b.lastAction=='READY_BUILD_SPY'); \
             b.Queue('PROGRESS_SPY'); FRAME(); local r=REQUESTS[#REQUESTS]; \
             assert(r.params.OnStart=='JAC_BoundaryProgressSpy'); \
             ENEMY_CASES.fireCore('JAC_BoundaryProgressSpy',r.params); \
             assert(PROGRESS_CALLS==1); PRODUCED_SPY_ID=983048; ADVANCE_TURN(); \
             assert(TURN==46 and #UNITS[0]==1)"
        );
    }

    fn bind(&self) {
        self.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyBind()");
        self.complete_step("ATTEST");
        self.complete_step("ADOPT");
        self.run("assert(ExposedMembers.JAC_PersonaBoundary.thirdLast=='ACCEPTED_ADOPT')");
    }

    fn unchanged(&self) {
        self.run(
            "local r=JAC_PersonaBoundaryRules; \
             assert(r.Serialize(Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY'))==BEFORE_RETRY); \
             assert(r.Serialize(HISTORY)==BEFORE_HISTORY); \
             assert(Game:GetProperty('JAC_BOUNDARY_SPY').id==851976); \
             assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION').id==1)",
        );
    }
}

#[test]
fn native_production_increment_is_explicitly_adopted_as_generation_three() {
    let t = Third::new();
    t.produce();
    t.bind();
    t.run(
        r#"
        local r=JAC_PersonaBoundaryRules; local b=ExposedMembers.JAC_PersonaBoundary;
        local a=Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE');
        local current=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY');
        assert(r.Serialize(a.properties.JAC_BOUNDARY_ENEMY_SPY_RETRY)==BEFORE_RETRY);
        assert(r.Serialize(a.properties.JAC_BOUNDARY_SPY_TRAVEL)==BEFORE_TRAVEL);
        assert(a.properties.JAC_BOUNDARY_SPY.id==851976 and a.properties.JAC_BOUNDARY_ENEMY_MISSION.id==1);
        assert(a.properties.JAC_BOUNDARY_ENEMY_SPY_RETRY.missionRequested==true);
        assert(a.propertiesEncoded.JAC_BOUNDARY_ENEMY_SPY_RETRY.priorUnknownField.nested[3]==
         'JAC_TEXT_V1:retain all previous retry fields');
        assert(r.Serialize(HISTORY)==BEFORE_HISTORY and current.current==3 and current.spyID==983048);
        local s=b.ThirdSpyStatus(); assert(s.bound and s.map.owner==0 and s.map.unitType==55 and s.physicalCount==3);
        assert(b.RetryLivingSpy()=='HOME_READY' or b.RetryLivingSpy()=='WAIT_NATIVE_READY');
        local count=#REQUESTS; assert(b.ThirdSpyBind()=='BOUND' and #REQUESTS==count);
        fire(Events.LoadScreenClose); assert(b.ThirdSpyStatus().bound and b.ReadEnemy().spy.id==983048);
        "#,
    );
}

#[test]
fn generation_three_cannot_start_before_native_capacity_three() {
    let t = Third::new();
    t.run("SPY_CAPACITY=2");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('BEGIN')",
        "not ready",
    );
    t.unchanged();
    t.run("assert(not Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'))");
}

#[test]
fn live_failure_values_one_and_minus_one_are_kept_as_raw_history() {
    let t = Third::new();
    t.run("EspionageResultTypes.SUCCESS_UNDETECTED=0; EspionageResultTypes.FAIL_UNDETECTED=1; EspionageResultTypes.NO_RESULT=-1; for _,id in ipairs({0,1}) do HISTORY[0][id].InitialResult=1; HISTORY[0][id].EscapeResult=-1 end; local old=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').archive; old.initial=1; old.escape=-1");
    t.produce();
    t.bind();
    t.run("local a=Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'); assert(a.histories[1].initial==1 and a.histories[1].escape==-1 and a.histories[2].initial==1 and a.histories[2].escape==-1); assert(a.histories[1].turn==23 and a.histories[2].turn==45)");
}

#[test]
fn build_is_blocked_before_save_and_after_reloaded_or_unknown_reservation() {
    let t = Third::new();
    t.begin();
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.ThirdSpySend('BUILD_SPY')",
        "reloaded, unsaved or already sent",
    );
    t.run("fire(Events.SaveComplete); fire(Events.LoadScreenClose)");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.ThirdSpySend('BUILD_SPY')",
        "reloaded, unsaved or already sent",
    );
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_SPY')",
        "cannot replay",
    );
    t.run("assert(#BUILD_REQUESTS==0 and PROGRESS_CALLS==0)");
}

#[test]
fn core_map_change_between_ui_read_and_archive_preserves_old_generation() {
    let t = Third::new();
    t.run(
        "ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('BEGIN'); FRAME(); \
         local raw=ExposedMembers.JAC_PersonaThirdSpyRead; local saved=raw(); \
         ExposedMembers.JAC_PersonaThirdSpyRead=function() return saved end; makeSpy(0,983048)",
    );
    t.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)",
        "Core map inventory differs",
    );
    t.unchanged();
}

#[test]
fn native_adoption_rejects_old_ids_and_two_new_spies() {
    for id in [spy_third::OLD_IDS[0], spy_third::OLD_IDS[1], 786440] {
        let t = Third::new();
        t.produce();
        t.run(&format!("UNITS[0][1].id={id}"));
        t.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyBind(); FRAME()");
        t.reject(
            "ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)",
            "operative identity differs",
        );
        t.run("assert(not Game:GetProperty('JAC_BOUNDARY_SPY'))");
    }
    let t = Third::new();
    t.produce();
    t.run("makeSpy(0,983049)");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.ThirdSpyBind()",
        "not ready",
    );
}

#[test]
fn adoption_does_not_accept_a_find_id_alias_or_changed_native_unit_type() {
    let t = Third::new();
    t.produce();
    t.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyBind(); FRAME(); local raw=ExposedMembers.JAC_PersonaThirdSpyRead; local saved=raw(); ExposedMembers.JAC_PersonaThirdSpyRead=function() return saved end; local original=Players[0].GetUnits; local alias=makeSpy(0,983049); table.remove(UNITS[0],2); Players[0].GetUnits=function() local units=original(); units.FindID=function() return alias end; return units end");
    t.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)",
        "exact unit lookup differs",
    );
    t.run("assert(not Game:GetProperty('JAC_BOUNDARY_SPY'))");

    let t = Third::new();
    t.produce();
    t.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyBind(); FRAME(); local raw=ExposedMembers.JAC_PersonaThirdSpyRead; local saved=raw(); ExposedMembers.JAC_PersonaThirdSpyRead=function() return saved end; GameInfo.Units[99]=GameInfo.Units.UNIT_PERSONA; UNITS[0][1].GetType=function() return 99 end");
    t.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)",
        "Core map inventory differs",
    );
    t.run("assert(not Game:GetProperty('JAC_BOUNDARY_SPY'))");
}

#[test]
fn changed_captive_group_prevents_final_adoption_without_pairing_anonymous_people() {
    let t = Third::new();
    t.produce();
    t.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyBind()");
    t.complete_step("ATTEST");
    t.run("CAPTURED[1][1].NameIndex=2; FRAME()");
    t.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)",
        "immutable archive differs",
    );
    t.run("assert(not Game:GetProperty('JAC_BOUNDARY_SPY') and not Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE').bound)");
}

#[test]
fn archived_events_cannot_be_used_as_a_third_mission_result() {
    let t = Third::new();
    t.produce();
    t.bind();
    t.run(
        "fire(Events.SpyMissionCompleted,0,0); fire(Events.SpyMissionCompleted,0,1); \
         assert(not Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'))",
    );
    for id in 0..=1 {
        t.reject(
            &format!("ENEMY_CASES.fireCore('JAC_BoundaryRecordEnemyMission',{{Attacker=0,MissionID={id}}})"),
            "archived failed native mission",
        );
    }
    t.run("HISTORY[0][1].CompletionTurn=44");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.ReadEnemy()",
        "preserved native failure changed",
    );
}

#[test]
fn partial_core_roster_api_is_rejected_before_archiving_anything() {
    let t = Third::new();
    t.run(
        "ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('BEGIN'); FRAME(); \
         local raw=ExposedMembers.JAC_PersonaThirdSpyRead; local saved=raw(); \
         ExposedMembers.JAC_PersonaThirdSpyRead=function() return saved end; \
         local original=Players[0].GetDiplomacy; Players[0].GetDiplomacy=function() \
          local d=original(); d.GetNthOffMapSpy=nil; return d end",
    );
    t.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)",
        "partial native Core roster API",
    );
    t.unchanged();
}

#[test]
fn third_generation_uses_existing_travel_arrival_mission_and_completion_routes() {
    let t = Third::new();
    t.produce();
    t.bind();
    t.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('SPY_TRAVEL')");
    t.complete_step("SPY_TRAVEL");
    t.run(
        r#"
        fire(Events.SaveComplete); REQUEST_EFFECT='travel';
        -- The legacy double handles one placeholder. Retain both sentinels
        -- around its existing native travel simulation for this test only.
        local request=UnitManager.RequestOperation;
        UnitManager.RequestOperation=function(spy,operation,params)
         local first=OFF_MAP[1]; request(spy,operation,params);
         if operation==UnitOperationTypes.SPY_TRAVEL_NEW_CITY then OFF_MAP[#OFF_MAP+1]=first end
        end;
        local b=ExposedMembers.JAC_PersonaBoundary; b.ThirdSpySend('SPY_TRAVEL'); FRAME();
        START_NATIVE(0,983048,991); FRAME(); ACK_BARRIER(); FRAME();
        assert(b.lastAction=='ACCEPTED_SPY_TRAVEL' and #UNIT_REQUESTS==1);
        b.Queue('SAVE_TRAVEL'); FRAME();
        local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundarySaveSpyTravel');
        ENEMY_CASES.fireCore('JAC_BoundarySaveSpyTravel',r.params);
        assert(b.RetryLivingSpy()=='OFF_MAP_TRAVEL');
        fire(Events.LoadScreenClose); assert(b.RetryLivingSpy()=='OFF_MAP_TRAVEL');
        ARRIVE_OFFMAP(1114112); assert(b.TryAdoptEnemySpy()=='ADOPT_QUEUED');
        FRAME(); FRAME(); ACK_ADOPTION(); FRAME();
        assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').spyID==1114112);
        assert(b.RetryLivingSpy()=='MISSION_READY');
        "#,
    );
    t.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('SPY_MISSION')");
    t.complete_step("SPY_MISSION");
    t.run(
        r#"
        fire(Events.SaveComplete); REQUEST_EFFECT='mission_hash';
        local b=ExposedMembers.JAC_PersonaBoundary; b.ThirdSpySend('SPY_MISSION'); FRAME();
        START_NATIVE(0,1114112,990); FRAME(); ACK_BARRIER(); FRAME();
        assert(b.lastAction=='ACCEPTED_SPY_MISSION' and #UNIT_REQUESTS==2);
        assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').missionRequested);
        assert(b.EnemyProgress()=='MISSION');
        fire(Events.SpyMissionCompleted,0,0); fire(Events.SpyMissionCompleted,0,1);
        assert(not Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'));
        TURN=54; HISTORY[0][2]={Operation=59,PlotIndex=Map.GetPlot(FOREIGN.x,FOREIGN.y):GetIndex(),
         CompletionTurn=54,InitialResult=3,EscapeResult=0};
        UNITS[0]={}; CAPTURED[1][3]={OwningPlayer=0,Name='',Level=1,NameIndex=-1};
        OFF_MAP[3]={Name='',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999};
        fire(Events.SpyMissionCompleted,0,2); FRAME();
        local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryRecordEnemyMission' and r.params.MissionID==2);
        ENEMY_CASES.fireCore('JAC_BoundaryRecordEnemyMission',r.params); FRAME();
        assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION').id==2 and b.EnemyProgress()=='FAILED');
        assert(HISTORY[0][0].CompletionTurn==23 and HISTORY[0][1].CompletionTurn==45 and #UNIT_REQUESTS==2);
        "#,
    );
}

#[test]
fn unavailable_core_roster_reads_remain_explicit_ui_evidence() {
    let t = Third::new();
    t.run(
        r#"
        ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('BEGIN'); FRAME();
        local raw=ExposedMembers.JAC_PersonaThirdSpyRead; local observed=raw();
        ExposedMembers.JAC_PersonaThirdSpyRead=function() return observed end;
        for _,id in ipairs({0,1}) do local original=Players[id].GetDiplomacy;
         Players[id].GetDiplomacy=function() local d=original();
          d.GetSpyCapacity=nil; d.GetNumSpiesOffMap=nil; d.GetNthOffMapSpy=nil;
          d.GetNumSpiesCaptured=nil; d.GetNthCapturedSpy=nil; d.GetMission=nil; return d end
        end;
        ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params);
        local a=Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE');
        assert(a.sources.map=='CORE' and a.sources.civic=='CORE');
        assert(a.sources.histories=='UI' and a.sources.capacity=='UI' and
         a.sources.captives=='UI' and a.sources.offMap=='UI');
        assert(a.histories[1].id==0 and a.histories[2].id==1);
        "#,
    );
}

#[test]
fn reserved_operative_change_is_rejected_before_a_native_operation_is_sent() {
    let t = Third::new();
    t.produce();
    t.bind();
    t.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('SPY_TRAVEL')");
    t.complete_step("SPY_TRAVEL");
    t.run("fire(Events.SaveComplete); ExposedMembers.JAC_PersonaBoundary.ThirdSpySend('SPY_TRAVEL'); UNITS[0][1].id=983049; Game:GetProperty('JAC_BOUNDARY_SPY').id=983049; Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').spyID=983049");
    t.reject(
        "FRAME()",
        "reserved operative changed before native dispatch",
    );
    t.run("assert(#UNIT_REQUESTS==0)");
}

#[test]
fn a_new_event_id_with_old_native_completion_history_is_not_a_third_result() {
    let t = Third::new();
    t.produce();
    t.bind();
    t.run(
        r#"
        Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE').intents.SPY_MISSION={token=30,turn=46,spyID=983048};
        Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').missionRequested=true;
        Game:SetProperty('JAC_BOUNDARY_SPY_BARRIER',{owner=0,spyID=983048,action='SPY_MISSION',token=30});
        HISTORY[0][2]={Operation=59,PlotIndex=Map.GetPlot(FOREIGN.x,FOREIGN.y):GetIndex(),
         CompletionTurn=44,InitialResult=1,EscapeResult=0};
        "#,
    );
    t.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryRecordEnemyMission',{Attacker=0,MissionID=2})",
        "differs from new native history",
    );
    t.run("assert(not Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'))");
}

#[test]
fn generated_connection_matches_the_rust_source_and_registered_mod_file() {
    spy_third::write_connection(&root(), true).unwrap();
    let meta = std::fs::read_to_string(
        root().join("poc/JAC_Test_PersonaBoundary/JAC_Test_PersonaBoundary.modinfo"),
    )
    .unwrap();
    assert_eq!(
        meta.matches("<File>Shared/JAC_PersonaThirdSpy.lua</File>")
            .count(),
        2
    );
}

#[test]
fn third_scenarios_parse_and_save_reservations_before_native_send() {
    for suffix in [
        "produce",
        "production-bind",
        "production-tick",
        "travel",
        "mission",
    ] {
        let path = root().join("tools/civ6-autotest/scenarios").join(format!(
            "leader-persona-spy-neutralization-third-{suffix}.json"
        ));
        let data: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let steps = data["steps"].as_array().unwrap();
        assert_eq!(data["strict_lua_errors"], true);
        assert_eq!(steps[0]["action"], "load");
        assert_eq!(steps[0]["checkpoint"], true);
        assert_eq!(steps[0]["exact_name"], true);
        let lua = Lua::new();
        let mut reservation = None;
        for (index, step) in steps.iter().enumerate() {
            if step["action"] == "save" {
                assert_eq!(step["checkpoint"], true);
                assert_eq!(step["exact_name"], true);
            }
            if let Some(code) = step["code"]
                .as_str()
                .or_else(|| step["condition"]["code"].as_str())
            {
                lua.load(code).into_function().unwrap();
                assert!(!code.contains("BEGIN_SPY_RETRY") && !code.contains("SPAWN_SPY"));
                if code.contains("ThirdSpyReserve(") {
                    reservation = Some(index);
                }
                if code.contains("ThirdSpySend(") {
                    let reserve = reservation.expect("native send needs a generation reservation");
                    assert!(steps[reserve + 1..index]
                        .iter()
                        .any(|v| v["action"] == "save"));
                }
            }
        }
    }
}
