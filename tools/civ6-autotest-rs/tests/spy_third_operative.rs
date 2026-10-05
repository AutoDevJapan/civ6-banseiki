//! Bounded third-generation cases execute the Rust-generated connection in
//! Lua 5.1. They do not attest that production or capacity 3 works in Civ6.
#[path = "../src/spy_completed_failure.rs"]
#[allow(dead_code)]
mod spy_completed_failure;
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
        lua.load("ORIGINAL_NATIVE_UI=UI; ORIGINAL_NATIVE_REQUEST=UI.RequestPlayerOperation")
            .exec()
            .unwrap();
        lua.load(spy_completed_failure::LUA).exec().unwrap();
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
            -- A Core callback cannot regain UI getters by calling an Exposed
            -- function. No proxy swaps the Governor or CORE_READ back to UI.
            UI_CORE_CALLS=0; CORE_ROSTER_MODE='absent'; CORE_UNIT_MODE='missing';
            CORE_UNIT_METHODS={};
            local unitMethodNames={'GetID','GetOwner','GetType','GetX','GetY','GetName',
             'GetExperience','GetSpyOperation','GetSpyOperationEndTurn'};
            local function rememberUnit(u)
             if not CORE_UNIT_METHODS[u] then local methods={};
              for _,name in ipairs(unitMethodNames) do methods[name]=u[name] end;
              CORE_UNIT_METHODS[u]=methods;
             end
            end
            local function coreUnitSurface(u)
             rememberUnit(u);
             if CORE_UNIT_MODE=='missing' then
              u.GetName=nil; u.GetSpyOperation=nil; u.GetSpyOperationEndTurn=nil;
              u.GetExperience=function() return {GetExperiencePoints=function() return 0 end} end;
             end
             if CORE_UNIT_FAULT then CORE_UNIT_FAULT(u) end
            end
            local nativeMakeSpy=makeSpy;
            makeSpy=function(...)
             local u=nativeMakeSpy(...); rememberUnit(u);
             if CORE_READ then coreUnitSurface(u) end;
             return u
            end;
            local function uiOnly(fn)
             return function(...)
              if CORE_READ then UI_CORE_CALLS=UI_CORE_CALLS+1; error('UI function called from Gameplay') end
              return fn(...)
             end
            end
            JAC_PersonaBoundaryRules.Third.Read=uiOnly(JAC_PersonaBoundaryRules.Third.Read);
            JAC_PersonaBoundaryRules.Third.CheckHistory=uiOnly(JAC_PersonaBoundaryRules.Third.CheckHistory);
            ExposedMembers.JAC_PersonaThirdSpyRead=uiOnly(function() return JAC_PersonaBoundaryRules.Third.Read() end);
            ExposedMembers.JAC_PersonaThirdSpyMissionRead=uiOnly(function() error('obsolete UI history callback') end);
            Game.GetPlayers=uiOnly(Game.GetPlayers);
            for _,playerID in ipairs({0,1}) do
             local p=Players[playerID]; local original=p.GetDiplomacy;
             p.GetDiplomacy=function()
              local d=original();
              if CORE_READ and CORE_ROSTER_MODE~='available' then
               d.GetNumSpiesOffMap=nil; d.GetNthOffMapSpy=nil;
               d.GetNumSpiesCaptured=nil; d.GetNthCapturedSpy=nil; d.GetMission=nil;
              end
              return d
             end;
             p.GetID=uiOnly(p.GetID);
            end
            local native=ExposedMembers.JAC_LeaderPersonaNativeRead;
            ExposedMembers.JAC_LeaderPersonaNativeRead=function(...)
             if CORE_READ and THIRD_CORE_EVENT then UI_CORE_CALLS=UI_CORE_CALLS+1; error('UI Governor function called from Gameplay') end
             return native(...)
            end;
            local coreEvent=ENEMY_CASES.fireCore;
            ENEMY_CASES.fireCore=function(name,params,owner)
             THIRD_CORE_EVENT=name=='JAC_BoundaryThirdSpyStep' or name=='JAC_BoundaryRecordEnemyMission' or name=='JAC_BoundaryProgressSpy';
             for _,u in ipairs(UNITS[0]) do rememberUnit(u) end;
             for u in pairs(CORE_UNIT_METHODS) do coreUnitSurface(u) end;
             local ok,err=pcall(coreEvent,name,params,owner); THIRD_CORE_EVENT=false;
             for u,methods in pairs(CORE_UNIT_METHODS) do
              for _,method in ipairs(unitMethodNames) do u[method]=methods[method] end
             end
             if not ok then error(err) end
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
             ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',r.params); \
             fire(Events.GameCoreEventPublishComplete); FRAME(); \
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
             assert(r.params.OnStart=='JAC_BoundaryThirdSpyStep' and r.params.Action=='PROGRESS_SPY'); \
             ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',r.params); \
             assert(PROGRESS_CALLS==1 and not Game:GetProperty('JAC_BOUNDARY_SPY_PROGRESS')); \
             fire(Events.GameCoreEventPublishComplete); FRAME(); \
             r=REQUESTS[#REQUESTS]; assert(r.params.Phase=='BARRIER'); \
             ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',r.params); \
             fire(Events.GameCoreEventPublishComplete); FRAME(); \
             assert(Game:GetProperty('JAC_BOUNDARY_SPY_PROGRESS') and UI_CORE_CALLS==0); \
             PRODUCED_SPY_ID=983048; ADVANCE_TURN(); assert(TURN==46 and #UNITS[0]==1)"
        );
    }

    // The live failure was after AddProgress had already created a map unit.
    // Model that native side effect before the command acknowledgement.
    fn enable_immediate_production(&self) {
        self.run(
            r#"
            local queue=CONTROL:GetBuildQueue(); local add=queue.AddProgress;
            queue.AddProgress=function(self,amount)
             assert(CORE_READ,'native production mutation belongs in Core');
             add(self,amount);
             local spy=makeSpy(0,983048); spy.x=CONTROL.x; spy.y=CONTROL.y; spy.finish=0;
             BUILD_HEAD=nil;
            end;
            "#,
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

    // Exercise existing public routes. Only native engine observations are
    // doubled; no recovery baseline or successful fixture event is forged.
    fn third_mission_started(&self) {
        self.produce();
        self.bind();
        self.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('SPY_TRAVEL')");
        self.complete_step("SPY_TRAVEL");
        self.run(
            r#"
            fire(Events.SaveComplete); REQUEST_EFFECT='travel';
            local request=UnitManager.RequestOperation;
            UnitManager.RequestOperation=function(spy,operation,params)
             local first=OFF_MAP[1]; request(spy,operation,params);
             if operation==UnitOperationTypes.SPY_TRAVEL_NEW_CITY then OFF_MAP[#OFF_MAP+1]=first end
            end;
            local b=ExposedMembers.JAC_PersonaBoundary;
            b.ThirdSpySend('SPY_TRAVEL'); FRAME(); START_NATIVE(0,983048,991);
            FRAME(); ACK_BARRIER(); FRAME(); assert(b.lastAction=='ACCEPTED_SPY_TRAVEL');
            b.Queue('SAVE_TRAVEL'); FRAME();
            ENEMY_CASES.fireCore('JAC_BoundarySaveSpyTravel',REQUESTS[#REQUESTS].params);
            assert(b.RetryLivingSpy()=='OFF_MAP_TRAVEL');
            ARRIVE_OFFMAP(1114112); assert(b.TryAdoptEnemySpy()=='ADOPT_QUEUED');
            FRAME(); FRAME(); ACK_ADOPTION(); FRAME();
            assert(b.RetryLivingSpy()=='MISSION_READY');
            "#,
        );
        self.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('SPY_MISSION')");
        self.complete_step("SPY_MISSION");
        self.run(
            r#"
            fire(Events.SaveComplete); REQUEST_EFFECT='mission_hash';
            local b=ExposedMembers.JAC_PersonaBoundary;
            b.ThirdSpySend('SPY_MISSION'); FRAME(); START_NATIVE(0,1114112,990);
            FRAME(); ACK_BARRIER(); FRAME();
            assert(b.lastAction=='ACCEPTED_SPY_MISSION' and b.EnemyProgress()=='MISSION');
            assert(#UNIT_REQUESTS==2 and UNIT_REQUESTS[2].spy:GetID()==1114112);
            local r=JAC_PersonaBoundaryRules;
            PRESERVED_FAILURES=r.Serialize({HISTORY[0][0],HISTORY[0][1]});
            PRESERVED_LINEAGE=r.Serialize(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'));
            "#,
        );
    }

    fn complete_third_mission(&self, initial: &str, escape: &str, remaining: u32) {
        self.run(&format!(
            r#"
            TURN=54;
            HISTORY[0][2]={{Operation=59,PlotIndex=Map.GetPlot(FOREIGN.x,FOREIGN.y):GetIndex(),
             CompletionTurn=54,InitialResult={initial},EscapeResult={escape}}};
            UNITS[0][1].op=-1; UNITS[0][1].finish=TURN;
            if {escape}==EspionageResultTypes.CAPTURED then
             UNITS[0]={{}}; CAPTURED[1][3]={{OwningPlayer=0,Name='',Level=1,NameIndex=-1}};
             OFF_MAP[3]={{Name='',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}};
            end;
            if {remaining}>0 then ENEMY_CASES.neutralize({remaining}) end;
            local b=ExposedMembers.JAC_PersonaBoundary; local count=#REQUESTS;
            fire(Events.SpyMissionCompleted,0,0); fire(Events.SpyMissionCompleted,0,1);
            FRAME(); assert(#REQUESTS==count and not Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'));
            fire(Events.SpyMissionCompleted,0,2); FRAME();
            local r=REQUESTS[#REQUESTS];
            assert(r.params.OnStart=='JAC_BoundaryRecordEnemyMission' and r.params.MissionID==2);
            ENEMY_CASES.fireCore('JAC_BoundaryRecordEnemyMission',r.params);
            fire(Events.GameCoreEventPublishComplete); FRAME();
            r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryThirdSpyStep' and r.params.Action=='RECORD_POST');
            ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',r.params);
            fire(Events.GameCoreEventPublishComplete); FRAME(); FRAME();
            assert(b.enemyMissionStatus=='RECORDED' and Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION').id==2);
            assert(UI_CORE_CALLS==0 and Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE').missionPost.source=='UI');
            "#
        ));
        self.preserved_failures();
    }

    fn third_success(&self, captured: bool) {
        self.third_mission_started();
        self.complete_third_mission(
            if captured {
                "EspionageResultTypes.SUCCESS_MUST_ESCAPE"
            } else {
                "EspionageResultTypes.SUCCESS_UNDETECTED"
            },
            if captured {
                "EspionageResultTypes.CAPTURED"
            } else {
                "EspionageResultTypes.NO_RESULT"
            },
            3,
        );
        self.run("local b=ExposedMembers.JAC_PersonaBoundary; b.CheckEnemy('NEUTRALIZED',6); assert(b.EnemyProgress()=='NEUTRALIZED')");
    }

    fn capture_third_recovery(&self) {
        self.legacy_sync("CAPTURE_ENEMY", "JAC_BoundaryCaptureEnemyRecovery");
        self.run(
            r#"
            local r=JAC_PersonaBoundaryRules;
            local baseline=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_RECOVERY'));
            assert(baseline.merit==6 and baseline.spent==1 and baseline.remaining==3 and baseline.turn==54);
            PRESERVED_RECOVERY=r.Serialize(baseline);
            "#,
        );
        self.preserved_failures();
    }

    fn preserved_failures(&self) {
        self.run(
            r#"
            local r=JAC_PersonaBoundaryRules; local b=ExposedMembers.JAC_PersonaBoundary;
            assert(r.Serialize({HISTORY[0][0],HISTORY[0][1]})==PRESERVED_FAILURES);
            assert(r.Serialize(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'))==PRESERVED_LINEAGE);
            local s=b.ReadEnemy(); assert(s.retry.current==3 and s.retry.spyID==1114112);
            assert(s.retry.archive.eventID==0 and s.retry.archive.eventTurn==23);
            local a=Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE');
            assert(a.properties.JAC_BOUNDARY_ENEMY_SPY_RETRY.spyID==851976 and
             a.properties.JAC_BOUNDARY_ENEMY_MISSION.id==1 and a.properties.JAC_BOUNDARY_ENEMY_MISSION.turn==45);
            "#,
        );
    }

    // Recreate the actual View and its event subscriptions, retaining the
    // Gameplay fixture and in-memory native observations. This is UI reload
    // only: no save file/load, natural success, or native neutralization.
    fn recreate_view_only(&self) {
        self.run(
            r#"
            OLD_THIRD_VIEW=ExposedMembers.JAC_PersonaBoundary;
            local keys={'GameCoreEventPublishComplete','LocalPlayerTurnBegin','LoadScreenClose','SaveComplete',
             'SpyMissionCompleted','CityProductionQueueChanged','UnitOperationStarted','DiplomacyIncomingDeal'};
            for _,key in ipairs(keys) do Events[key]=event() end;
            LuaEvents.DiploPopup_DealUpdated=event();
            "#,
        );
        self.0
            .load(
                std::fs::read_to_string(
                    root().join("poc/JAC_Test_PersonaBoundary/UI/JAC_PersonaBoundary_View.lua"),
                )
                .unwrap(),
            )
            .exec()
            .unwrap();
        self.run(
            r#"
            assert(OLD_THIRD_VIEW~=ExposedMembers.JAC_PersonaBoundary);
            fire(Events.LoadScreenClose);
            assert(JAC_PersonaBoundaryRules.Serialize(Game:GetProperty('JAC_BOUNDARY_ENEMY_RECOVERY'))==PRESERVED_RECOVERY);
            assert(ExposedMembers.JAC_PersonaBoundary.ReadEnemy().missions['0/2']);
            "#,
        );
        self.preserved_failures();
    }
}

#[test]
fn third_generation_owned_sender_keeps_the_native_api_namespace_unchanged() {
    let t = Third::new();
    t.produce();
    t.bind();
    t.run(
        "assert(UI==ORIGINAL_NATIVE_UI and UI.RequestPlayerOperation==ORIGINAL_NATIVE_REQUEST); \
         assert(rawget(UI,'RequestPlayerOperation')==ORIGINAL_NATIVE_REQUEST); \
         assert(UI_CORE_CALLS==0)",
    );
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
    t.run("EspionageResultTypes.SUCCESS_UNDETECTED=5; EspionageResultTypes.SUCCESS_MUST_ESCAPE=4; EspionageResultTypes.FAIL_UNDETECTED=3; EspionageResultTypes.FAIL_MUST_ESCAPE=2; EspionageResultTypes.CAPTURED=1; EspionageResultTypes.KILLED=0; EspionageResultTypes.NO_RESULT=-1; for _,id in ipairs({0,1}) do HISTORY[0][id].InitialResult=1; HISTORY[0][id].EscapeResult=-1 end; local old=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').archive; old.initial=1; old.escape=-1");
    t.produce();
    t.bind();
    t.run("local a=Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'); assert(a.histories[1].initial==1 and a.histories[1].escape==-1 and a.histories[2].initial==1 and a.histories[2].escape==-1); assert(a.histories[1].turn==23 and a.histories[2].turn==45)");
}

#[test]
fn actual_captured_outcome_allows_third_generation_without_rewriting_either_failure() {
    let t = Third::new();
    t.run("EspionageResultTypes.SUCCESS_UNDETECTED=5; EspionageResultTypes.SUCCESS_MUST_ESCAPE=4; EspionageResultTypes.FAIL_UNDETECTED=3; EspionageResultTypes.FAIL_MUST_ESCAPE=2; EspionageResultTypes.CAPTURED=1; EspionageResultTypes.KILLED=0; EspionageResultTypes.NO_RESULT=-1; for _,id in ipairs({0,1}) do HISTORY[0][id].InitialResult=1; HISTORY[0][id].EscapeResult=-1 end; local a=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').archive; a.initial=1; a.escape=-1");
    t.produce();
    t.bind();
    t.run("assert(HISTORY[0][0].InitialResult==1 and HISTORY[0][1].InitialResult==1 and HISTORY[0][1].EscapeResult==-1 and #CAPTURED[1]==2)");
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
         makeSpy(0,983048)",
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
    t.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyBind(); FRAME(); local original=Players[0].GetUnits; local alias=makeSpy(0,983049); table.remove(UNITS[0],2); Players[0].GetUnits=function() local units=original(); units.FindID=function() return alias end; return units end");
    t.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)",
        "exact unit lookup differs",
    );
    t.run("assert(not Game:GetProperty('JAC_BOUNDARY_SPY'))");

    let t = Third::new();
    t.produce();
    t.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyBind(); FRAME(); GameInfo.Units[99]=GameInfo.Units.UNIT_PERSONA; UNITS[0][1].GetType=function() return 99 end");
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
         CORE_ROSTER_MODE='available'; \
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
        local observed=ExposedMembers.JAC_PersonaThirdSpyObservation.observed;
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
    t.reject("FRAME()", "changed before native frame dispatch");
    t.run("assert(#UNIT_REQUESTS==0)");
}

#[test]
fn a_new_event_id_with_old_native_completion_history_is_not_a_third_result() {
    let t = Third::new();
    t.third_mission_started();
    t.run(
        r#"
        HISTORY[0][2]={Operation=59,PlotIndex=Map.GetPlot(FOREIGN.x,FOREIGN.y):GetIndex(),
         CompletionTurn=44,InitialResult=1,EscapeResult=0};
        fire(Events.SpyMissionCompleted,0,2); FRAME();
        "#,
    );
    t.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryRecordEnemyMission',REQUESTS[#REQUESTS].params)",
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

#[test]
fn third_success_capture_baseline_survives_actual_view_recreation_only() {
    for captured in [false, true] {
        let t = Third::new();
        t.third_success(captured);
        t.capture_third_recovery();
        t.run("assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_RECOVERY').unassignedAtCapture==1); BEFORE_NEUTRALIZED_VIEW=ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED',6)");
        t.recreate_view_only();
        t.run(
            r#"
            local b=ExposedMembers.JAC_PersonaBoundary;
            assert(b.CheckEnemy('NEUTRALIZED',6)==BEFORE_NEUTRALIZED_VIEW);
            assert(b.EnemyProgress()=='NEUTRALIZED');
            local count=#REQUESTS;
            fire(Events.SpyMissionCompleted,0,0); fire(Events.SpyMissionCompleted,0,1);
            fire(Events.SpyMissionCompleted,0,2); FRAME();
            assert(#REQUESTS==count and Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION').id==2);
            "#,
        );
        t.preserved_failures();
    }
}

#[test]
fn third_success_merit_and_city_effects_stay_stopped_until_unassigned_recovery() {
    let t = Third::new();
    t.third_success(false);
    t.capture_third_recovery();
    t.recreate_view_only();
    for remaining in [2, 1] {
        t.run(&format!("ADVANCE_TURN(); ENEMY_CASES.neutralize({remaining}); local b=ExposedMembers.JAC_PersonaBoundary; b.CheckEnemy('NEUTRALIZED',6); assert(b.EnemyProgress()=='NEUTRALIZED')"));
        t.preserved_failures();
    }
    t.run("ENEMY_EFFECT=true");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED',6)",
        "native city effect remains/missing",
    );
    t.run("ENEMY_EFFECT=false; Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_MERIT',7)");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.EnemyProgress()",
        "earned merit while neutralized",
    );
    t.run("Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_MERIT',6); ADVANCE_TURN(); ENEMY_CASES.neutralize(0); local b=ExposedMembers.JAC_PersonaBoundary; b.CheckEnemy('RECOVERED',6); assert(b.EnemyProgress()=='RECOVERED' and b.ReadEnemy().native.cityOwner==-1)");
    t.preserved_failures();
}

#[test]
fn third_success_neutralization_rejects_merit_gain_with_established_flag_true() {
    let t = Third::new();
    t.third_success(false);
    t.run("ENEMY_GOV.established=true");
    t.capture_third_recovery();
    t.recreate_view_only();
    t.run("assert(ExposedMembers.JAC_PersonaBoundary.EnemyProgress()=='NEUTRALIZED'); Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_MERIT',7)");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.EnemyProgress()",
        "earned merit while neutralized",
    );
}

#[test]
fn third_success_ai_reassignment_requires_recorded_unassigned_baseline_and_five_turns() {
    let t = Third::new();
    t.third_success(false);
    t.capture_third_recovery();
    t.recreate_view_only();
    t.run("assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_RECOVERY').unassignedAtCapture==1); ENEMY_CASES.neutralize(0); REASSIGN_REQUESTS_BEFORE=#REQUESTS");
    for turn in 0..5 {
        t.run(&format!("ADVANCE_TURN(); ENEMY_CASES.reassign({turn}); local b=ExposedMembers.JAC_PersonaBoundary; b.CheckEnemy('ESTABLISHING',6); assert(b.EnemyProgress()=='ESTABLISHING' and #REQUESTS==REASSIGN_REQUESTS_BEFORE)"));
        t.reject(
            "ExposedMembers.JAC_PersonaBoundary.CheckEnemy('SETTLED',6)",
            "unexpected native establishment state",
        );
        t.preserved_failures();
    }
    t.run("ENEMY_EFFECT=true");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.CheckEnemy('ESTABLISHING',6)",
        "native city effect remains/missing",
    );
    t.run("ENEMY_EFFECT=false; Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_MERIT',7)");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.EnemyProgress()",
        "earned merit while neutralized",
    );
    t.run("Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_MERIT',6)");
    t.run("ADVANCE_TURN(); ENEMY_CASES.reassign(5); local b=ExposedMembers.JAC_PersonaBoundary; b.CheckEnemy('SETTLED'); assert(b.EnemyProgress()=='SETTLED' and #REQUESTS==REASSIGN_REQUESTS_BEFORE and #UNIT_REQUESTS==2)");
    t.preserved_failures();
}

#[test]
fn third_success_ai_reassignment_is_rejected_when_capture_observed_assignment() {
    let t = Third::new();
    t.third_success(false);
    t.run("ENEMY_GOV.assigned=FOREIGN");
    t.capture_third_recovery();
    t.recreate_view_only();
    t.run("assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_RECOVERY').unassignedAtCapture==0); ENEMY_CASES.neutralize(0)");
    for turn in [0, 5] {
        t.run(&format!("ENEMY_CASES.reassign({turn})"));
        t.reject(
            "ExposedMembers.JAC_PersonaBoundary.EnemyProgress()",
            "native assignment has no prior unassigned observation",
        );
        t.preserved_failures();
    }
}

#[test]
fn third_success_public_reassignment_uses_native_request_and_retains_five_turn_boundary() {
    let t = Third::new();
    t.third_success(false);
    t.capture_third_recovery();
    t.recreate_view_only();
    t.run(
        r#"
        ENEMY_CASES.neutralize(0);
        local b=ExposedMembers.JAC_PersonaBoundary;
        b.CheckEnemy('RECOVERED',6); assert(b.EnemyProgress()=='RECOVERED');
        local request=UI.RequestPlayerOperation; NATIVE_REASSIGN_REQUESTS=0;
        UI.RequestPlayerOperation=function(owner,operation,params)
         request(owner,operation,params);
         if operation==PlayerOperations.ASSIGN_GOVERNOR then
          assert(owner==1 and params[PlayerOperations.PARAM_GOVERNOR_TYPE]==1 and
           params[PlayerOperations.PARAM_PLAYER_ONE]==1 and params[PlayerOperations.PARAM_CITY_DEST]==20);
          NATIVE_REASSIGN_REQUESTS=NATIVE_REASSIGN_REQUESTS+1; ENEMY_CASES.reassign(0);
         end
        end;
        b.QueueEnemy('REASSIGN'); FRAME(); FRAME();
        local r=REQUESTS[#REQUESTS];
        assert(r.params.OnStart=='JAC_BoundaryEnemyBarrier' and r.params.Action=='REASSIGN');
        ENEMY_CASES.fireCore('JAC_BoundaryEnemyBarrier',r.params);
        fire(Events.GameCoreEventPublishComplete); FRAME();
        assert(b.enemyLast=='ACCEPTED_REASSIGN' and NATIVE_REASSIGN_REQUESTS==1);
        assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_ATTEMPTS').REASSIGN);
        "#,
    );
    for turn in 0..5 {
        t.run(&format!("ENEMY_CASES.reassign({turn}); local b=ExposedMembers.JAC_PersonaBoundary; b.CheckEnemy('ESTABLISHING',6); assert(b.EnemyProgress()=='ESTABLISHING')"));
    }
    t.run("ENEMY_CASES.reassign(5); local b=ExposedMembers.JAC_PersonaBoundary; b.CheckEnemy('SETTLED'); assert(b.EnemyProgress()=='SETTLED')");
    t.recreate_view_only();
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.QueueEnemy('REASSIGN')",
        "already attempted before load",
    );
    t.run("assert(NATIVE_REASSIGN_REQUESTS==1 and #UNIT_REQUESTS==2)");
    t.preserved_failures();
}

#[test]
fn third_success_recovery_capture_never_uses_either_archived_failed_event() {
    let t = Third::new();
    t.third_success(false);
    for id in [0, 1] {
        t.run(&format!(
            "Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION').id={id}"
        ));
        t.reject(
            "ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED')",
            "successful native mission history was not observed",
        );
        t.reject(
            "ExposedMembers.JAC_PersonaBoundary.Queue('CAPTURE_ENEMY'); FRAME()",
            "successful native mission history was not observed",
        );
        t.run("assert(not Game:GetProperty('JAC_BOUNDARY_ENEMY_RECOVERY'))");
        t.preserved_failures();
    }
    t.run("Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION').id=2");
    t.capture_third_recovery();
    t.run("assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_RECOVERY').turn==54)");
}

#[test]
fn third_success_recovery_requires_both_new_success_and_native_neutralization_observation() {
    for (initial, remaining, message) in [
        (
            "EspionageResultTypes.SUCCESS_UNDETECTED",
            0,
            "native neutralization was not observed",
        ),
        (
            "EspionageResultTypes.FAIL_UNDETECTED",
            3,
            "successful native mission history was not observed",
        ),
    ] {
        let t = Third::new();
        t.third_mission_started();
        t.complete_third_mission(initial, "EspionageResultTypes.NO_RESULT", remaining);
        t.reject(
            "ExposedMembers.JAC_PersonaBoundary.Queue('CAPTURE_ENEMY'); FRAME()",
            message,
        );
        t.run("assert(not Game:GetProperty('JAC_BOUNDARY_ENEMY_RECOVERY') and #UNIT_REQUESTS==2)");
        t.preserved_failures();
    }
}

#[test]
fn core_ui_function_reentry_is_a_fixture_error_not_an_independent_read() {
    let t = Third::new();
    t.run("GameEvents.JAC_TestThirdUIReentry=event(); GameEvents.JAC_TestThirdUIReentry.Add(function() ExposedMembers.JAC_PersonaThirdSpyRead() end)");
    t.reject(
        "ENEMY_CASES.fireCore('JAC_TestThirdUIReentry',{})",
        "UI function called from Gameplay",
    );
    t.run("assert(UI_CORE_CALLS==1 and not Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'))");
    t.unchanged();
}

#[test]
fn scalar_witness_begin_progress_and_mission_work_with_ui_only_core_apis_absent() {
    let t = Third::new();
    t.third_success(false);
    t.run(
        r#"
        assert(UI_CORE_CALLS==0 and CORE_ROSTER_MODE=='absent');
        local a=Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE');
        local p=Game:GetProperty('JAC_BOUNDARY_SPY_PRODUCTION');
        assert(a.sources.governor=='UI' and a.sources.assignment=='UI' and a.sources.queue=='UI');
        assert(a.sources.histories=='UI' and a.sources.captives=='UI' and a.sources.offMap=='UI');
        assert(a.sources.map=='CORE' and a.sources.person=='CORE' and a.sources.city=='CORE' and a.sources.titles=='CORE');
        assert(p.sources.governor=='UI' and p.sources.histories=='UI' and p.sources.map=='CORE');
        assert(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK').histories=='UI');
        assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE').missionPost.source=='UI');
        "#,
    );
}

#[test]
fn available_core_getters_are_compared_without_core_player_get_id_or_ui_callbacks() {
    let t = Third::new();
    t.run("CORE_ROSTER_MODE='available'");
    t.third_success(false);
    t.run(
        r#"
        local sources=Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE').sources;
        assert(sources.histories=='CORE' and sources.capacity=='CORE' and sources.captives=='CORE' and sources.offMap=='CORE');
        assert(sources.governor=='UI' and sources.assignment=='UI' and sources.queue=='UI');
        assert(UI_CORE_CALLS==0 and Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK').histories=='CORE');
        "#,
    );
}

#[test]
fn request_metadata_and_observation_must_match_the_published_scalar_witness() {
    for mutation in [
        "r.Frame=r.Frame+1",
        "r.PublishEpoch=r.PublishEpoch+1",
        "r.Token=r.Token+1",
        "r.Turn=r.Turn-1",
        "r.Observation=r.Observation..'changed'",
        "ExposedMembers.JAC_PersonaThirdSpyObservation.observed.capacity=4",
    ] {
        let t = Third::new();
        t.run(&format!("ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('BEGIN'); FRAME(); local r=REQUESTS[#REQUESTS].params; {mutation}"));
        let error =
            t.0.load("ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)")
                .exec()
                .expect_err("mismatched scalar witness must reject");
        assert!(
            error.to_string().contains("observation")
                || error.to_string().contains("witness")
                || error.to_string().contains("synchronized request differs"),
            "{error}"
        );
        t.unchanged();
        t.run("assert(UI_CORE_CALLS==0 and not Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE') and #BUILD_REQUESTS==0 and PROGRESS_CALLS==0)");
    }
}

#[test]
fn scalar_witness_cannot_hide_a_function_or_trigger_a_metatable_callback() {
    for mutation in [
        "w.observed.hidden=function() error('function ran in Core') end",
        "w.observed.hidden=setmetatable({}, {__index=function() error('metatable ran in Core') end})",
    ] {
        let t=Third::new();
        t.run(&format!("ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('BEGIN'); FRAME(); local w=ExposedMembers.JAC_PersonaThirdSpyObservation; {mutation}"));
        let error=t.0.load("ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)").exec().expect_err("non-scalar/mismatched data must reject");
        assert!(!error.to_string().contains("ran in Core"), "{error}");
        t.unchanged();
    }
}

#[test]
fn post_publish_begin_change_never_arms_build_even_after_save_complete() {
    let t = Third::new();
    t.run(
        r#"
        local b=ExposedMembers.JAC_PersonaBoundary; b.ThirdSpyReserve('BEGIN'); FRAME();
        ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params);
        fire(Events.GameCoreEventPublishComplete); FRAME();
        ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params);
        fire(Events.GameCoreEventPublishComplete);
        CAPTURED[1][1].NameIndex=17;
        "#,
    );
    t.reject("FRAME()", "preserved captive groups");
    t.run("fire(Events.SaveComplete); CAPTURED[1][1].NameIndex=-1");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.ThirdSpySend('BUILD_SPY')",
        "reloaded, unsaved or already sent",
    );
    t.run("assert(#BUILD_REQUESTS==0 and PROGRESS_CALLS==0 and UI_CORE_CALLS==0)");
}

#[test]
fn native_progress_waits_for_fresh_post_command_observation_before_its_receipt() {
    let t = Third::new();
    t.begin();
    t.run(
        r#"
        fire(Events.SaveComplete); local b=ExposedMembers.JAC_PersonaBoundary;
        b.ThirdSpySend('BUILD_SPY'); FRAME(); FRAME();
        b.Queue('PROGRESS_SPY'); FRAME();
        ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params);
        assert(PROGRESS_CALLS==1 and not Game:GetProperty('JAC_BOUNDARY_SPY_PROGRESS'));
        fire(Events.GameCoreEventPublishComplete); CAPTURED[1][1].NameIndex=17;
        "#,
    );
    t.reject("FRAME()", "changed after Core response");
    t.run("assert(PROGRESS_CALLS==1 and not Game:GetProperty('JAC_BOUNDARY_SPY_PROGRESS') and UI_CORE_CALLS==0); fire(Events.LoadScreenClose); CAPTURED[1][1].NameIndex=-1");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_SPY')",
        "cannot replay",
    );
}

#[test]
fn mission_record_requires_post_publish_history_comparison_before_result_ready() {
    let t = Third::new();
    t.third_mission_started();
    t.run(
        r#"
        TURN=54; UNITS[0][1].op=-1; UNITS[0][1].finish=TURN;
        HISTORY[0][2]={Operation=59,PlotIndex=Map.GetPlot(FOREIGN.x,FOREIGN.y):GetIndex(),
         CompletionTurn=54,InitialResult=EspionageResultTypes.SUCCESS_UNDETECTED,EscapeResult=EspionageResultTypes.NO_RESULT};
        fire(Events.SpyMissionCompleted,0,2); FRAME();
        ENEMY_CASES.fireCore('JAC_BoundaryRecordEnemyMission',REQUESTS[#REQUESTS].params);
        assert(not ExposedMembers.JAC_PersonaBoundary.EnemyMissionEventReady());
        fire(Events.GameCoreEventPublishComplete); HISTORY[0][2].EscapeResult=17;
        "#,
    );
    t.reject("FRAME()", "mission changed after Core response");
    t.run("assert(UI_CORE_CALLS==0 and not Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE').missionPost)");
}

#[test]
fn core_getter_errors_and_negative_counts_do_not_become_ui_fallback_success() {
    for fault in [
        "d.GetNumSpiesCaptured=function() error('unavailable Core getter') end",
        "d.GetNumSpiesCaptured=function() return -1 end",
        "d.GetMission=function() return 0 end",
        "d.GetMission=function() error('unavailable Core history') end",
        "d.GetMission=false",
        "d.GetNumSpiesCaptured=17; d.GetNthCapturedSpy=17",
        "d.GetSpyCapacity=false",
    ] {
        let t = Third::new();
        t.run(&format!("ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('BEGIN'); FRAME(); CORE_ROSTER_MODE='available'; local original=Players[0].GetDiplomacy; Players[0].GetDiplomacy=function() local d=original(); if CORE_READ then {fault} end; return d end"));
        t.0.load("ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)")
            .exec()
            .expect_err("unknown Core getter must reject");
        t.unchanged();
        t.run(
            "assert(not Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE') and UI_CORE_CALLS==0)",
        );
    }
}

#[test]
fn turn_46_capacity_three_begin_preserves_turn_23_and_45_native_failures() {
    let t = Third::new();
    t.run("TURN=46; EspionageResultTypes.SUCCESS_UNDETECTED=5; EspionageResultTypes.SUCCESS_MUST_ESCAPE=4; EspionageResultTypes.FAIL_UNDETECTED=3; EspionageResultTypes.FAIL_MUST_ESCAPE=2; EspionageResultTypes.CAPTURED=1; EspionageResultTypes.KILLED=0; EspionageResultTypes.NO_RESULT=-1; for _,id in ipairs({0,1}) do HISTORY[0][id].InitialResult=1; HISTORY[0][id].EscapeResult=-1 end; local a=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').archive; a.initial=1; a.escape=-1");
    t.begin();
    t.run(
        r#"
        local a=Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE');
        assert(a.turn==46 and a.histories[1].turn==23 and a.histories[2].turn==45);
        assert(a.histories[1].initial==1 and a.histories[2].escape==-1);
        assert(UI_CORE_CALLS==0 and #BUILD_REQUESTS==0 and PROGRESS_CALLS==0);
        fire(Events.SaveComplete);
        "#,
    );
}

#[test]
fn ui_reload_after_unknown_mission_record_keeps_history_but_does_not_replay() {
    let t = Third::new();
    t.third_mission_started();
    t.run(
        r#"
        TURN=54; UNITS[0][1].op=-1; UNITS[0][1].finish=TURN;
        HISTORY[0][2]={Operation=59,PlotIndex=Map.GetPlot(FOREIGN.x,FOREIGN.y):GetIndex(),
         CompletionTurn=54,InitialResult=EspionageResultTypes.SUCCESS_UNDETECTED,EscapeResult=EspionageResultTypes.NO_RESULT};
        fire(Events.SpyMissionCompleted,0,2); FRAME();
        ENEMY_CASES.fireCore('JAC_BoundaryRecordEnemyMission',REQUESTS[#REQUESTS].params);
        UNKNOWN_RECORD_REQUESTS=#REQUESTS;
        fire(Events.LoadScreenClose); FRAME();
        assert(#REQUESTS==UNKNOWN_RECORD_REQUESTS and not ExposedMembers.JAC_PersonaBoundary.EnemyMissionEventReady());
        assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION').id==2 and
         not Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE').missionPost and UI_CORE_CALLS==0);
        "#,
    );
    t.preserved_failures();
}

#[test]
fn immediate_production_core_optional_getters_absent_preserves_public_adoption_and_mission() {
    let t = Third::new();
    t.enable_immediate_production();
    t.third_success(false);
    t.run(
        r#"
        local receipt=Game:GetProperty('JAC_BOUNDARY_SPY_PROGRESS');
        local binding=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').binding;
        for _,sources in ipairs({receipt.sources,binding.sources}) do
         assert(sources.map=='CORE' and sources.unit.identity=='CORE' and sources.unit.position=='CORE');
         assert(sources.unit.name=='UI' and sources.unit.level=='UI' and
          sources.unit.operation=='UI' and sources.unit.endTurn=='UI');
        end
        assert(PROGRESS_CALLS==1 and UI_CORE_CALLS==0 and #UNIT_REQUESTS==2);
        assert(UI==ORIGINAL_NATIVE_UI and UI.RequestPlayerOperation==ORIGINAL_NATIVE_REQUEST);
        "#,
    );
    t.preserved_failures();
}

#[test]
fn immediate_production_available_core_unit_getters_are_independently_compared() {
    let t = Third::new();
    t.run("CORE_UNIT_MODE='available'");
    t.enable_immediate_production();
    t.produce();
    t.bind();
    t.run(
        r#"
        for _,sources in ipairs({Game:GetProperty('JAC_BOUNDARY_SPY_PROGRESS').sources,
         Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').binding.sources}) do
         for _,field in ipairs({'identity','position','name','level','operation','endTurn'}) do
          assert(sources.unit[field]=='CORE',field)
         end
        end
        assert(UI_CORE_CALLS==0 and PROGRESS_CALLS==1);
        "#,
    );
}

#[test]
fn optional_core_unit_getter_throw_invalid_and_partial_are_not_successful_fallbacks() {
    for (fault, message) in [
        (
            "u.GetName=function() error('Core name getter throws') end",
            "Core name getter throws",
        ),
        (
            "u.GetExperience=function() error('Core experience getter throws') end",
            "Core experience getter throws",
        ),
        (
            "u.GetExperience=function() return nil end",
            "experience object unavailable",
        ),
        (
            "u.GetExperience=function() return {GetLevel=false} end",
            "unknown third spy Core unit getter",
        ),
        (
            "u.GetExperience=function() return {GetLevel=function() return -1 end} end",
            "operative level differs",
        ),
        (
            "u.GetSpyOperation=function() error('Core operation getter throws') end",
            "Core operation getter throws",
        ),
        (
            "u.GetSpyOperation=function() return nil end",
            "operative operation differs",
        ),
        (
            "u.GetSpyOperation=false",
            "unknown third spy Core unit getter",
        ),
        (
            "u.GetSpyOperationEndTurn=function() return -17 end",
            "operative endTurn differs",
        ),
        ("u.GetX=nil", "partial third spy Core unit position API"),
        (
            "u.GetX=function() return -9999 end",
            "operative position differs",
        ),
    ] {
        let t = Third::new();
        t.produce();
        t.run(&format!("CORE_UNIT_MODE='available'; CORE_UNIT_FAULT=function(u) {fault} end; ExposedMembers.JAC_PersonaBoundary.ThirdSpyBind(); FRAME()"));
        t.reject(
            "ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)",
            message,
        );
        t.run("assert(not Game:GetProperty('JAC_BOUNDARY_SPY') and not Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE').bound and UI_CORE_CALLS==0 and PROGRESS_CALLS==1)");
    }
}

#[test]
fn native_identity_getters_remain_mandatory_for_explicit_adoption() {
    for fault in [
        "u.GetID=nil",
        "u.GetOwner=nil",
        "u.GetType=nil",
        "u.GetID=false",
        "u.GetOwner=function() return 1 end",
    ] {
        let t = Third::new();
        t.produce();
        t.run(&format!("CORE_UNIT_FAULT=function(u) {fault} end; ExposedMembers.JAC_PersonaBoundary.ThirdSpyBind(); FRAME()"));
        let error =
            t.0.load("ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)")
                .exec()
                .expect_err("native identity absence or disagreement must reject");
        assert!(
            error.to_string().contains("identity") || error.to_string().contains("unit getter"),
            "{error}"
        );
        t.run("assert(not Game:GetProperty('JAC_BOUNDARY_SPY') and UI_CORE_CALLS==0)");
    }
}

#[test]
fn missing_core_position_pair_is_ui_witness_only_in_generation_three_binding() {
    let t = Third::new();
    t.produce();
    t.run("CORE_UNIT_FAULT=function(u) u.GetX=nil; u.GetY=nil end");
    t.bind();
    t.run("local sources=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').binding.sources; assert(sources.unit.identity=='CORE' and sources.unit.position=='UI' and UI_CORE_CALLS==0)");
}

#[test]
fn core_optional_unit_getter_failure_after_immediate_progress_keeps_one_mutation_and_no_receipt() {
    let t = Third::new();
    t.enable_immediate_production();
    t.begin();
    t.run(
        r#"
        fire(Events.SaveComplete); local b=ExposedMembers.JAC_PersonaBoundary;
        b.ThirdSpySend('BUILD_SPY'); FRAME(); FRAME();
        b.Queue('PROGRESS_SPY'); FRAME();
        ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params);
        assert(PROGRESS_CALLS==1 and #UNITS[0]==1);
        fire(Events.GameCoreEventPublishComplete); FRAME();
        CORE_UNIT_FAULT=function(u) u.GetSpyOperation=function() error('Core operation getter throws') end end;
        "#,
    );
    t.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)",
        "Core operation getter throws",
    );
    t.run("assert(not Game:GetProperty('JAC_BOUNDARY_SPY_PROGRESS') and PROGRESS_CALLS==1 and UI_CORE_CALLS==0); fire(Events.LoadScreenClose)");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_SPY')",
        "cannot replay",
    );
}

#[test]
fn fresh_production_preserves_unknown_attempt_and_saves_before_barrier() {
    let t = Third::new();
    t.run("TURN=46");
    t.begin();
    t.run("OLD_ARCHIVE=JAC_PersonaBoundaryRules.Serialize(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE')); fire(Events.LoadScreenClose); TURN=47");
    t.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('FRESH_PRODUCTION_04')");
    t.complete_step("FRESH_PRODUCTION_04");
    t.run(
        r#"
        local b=ExposedMembers.JAC_PersonaBoundary;
        local u=Game:GetProperty('JAC_BOUNDARY_THIRD_PRODUCTION_UNCONFIRMED_04');
        assert(u.status=='SENT_RESULT_UNCONFIRMED' and u.source=='HOST_ARTIFACT');
        assert(u.buildSent and u.progressSent and not u.actualSpyIDKnown and not u.postMutationSave);
        assert(u.state.intents.BUILD_SPY.turn==46 and u.ack.action=='BEGIN');
        assert(JAC_PersonaBoundaryRules.Serialize(u.archive)==OLD_ARCHIVE);
        assert(JAC_PersonaBoundaryRules.Serialize(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'))==OLD_ARCHIVE);
        assert(#BUILD_REQUESTS==0 and PROGRESS_CALLS==0);
        fire(Events.SaveComplete); b.ThirdSpySend('BUILD_SPY'); FRAME(); FRAME();
        b.Queue('PROGRESS_SPY'); FRAME();
        ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params);
        fire(Events.GameCoreEventPublishComplete); FRAME();
        assert(b.thirdLast=='COMMAND_PROGRESS_AWAITING_SAVE');
        local requests=#REQUESTS; FRAME(); FRAME(); assert(#REQUESTS==requests);
        assert(PROGRESS_CALLS==1 and not Game:GetProperty('JAC_BOUNDARY_SPY_PROGRESS'));
        fire(Events.SaveComplete); FRAME();
        assert(REQUESTS[#REQUESTS].params.Phase=='BARRIER');
        ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params);
        fire(Events.GameCoreEventPublishComplete); FRAME();
        assert(b.thirdLast=='ACCEPTED_PROGRESS_SPY' and #BUILD_REQUESTS==1 and PROGRESS_CALLS==1);
        assert(JAC_PersonaBoundaryRules.Serialize(HISTORY)==BEFORE_HISTORY);
        assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE').properties.JAC_BOUNDARY_SPY.id==851976);
        fire(Events.LoadScreenClose);
        "#,
    );
    t.run("PRODUCED_SPY_ID=983048; ADVANCE_TURN(); assert(#UNITS[0]==1)");
    t.bind();
    t.run("local binding=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').binding; local production=Game:GetProperty('JAC_BOUNDARY_SPY_PRODUCTION'); assert(binding.source=='NATIVE_BUILD' and binding.spyID==983048 and binding.productionAttempt==2 and binding.productionReservation==production.reservationToken and binding.sources.unit.identity=='CORE')");
    t.run("fire(Events.LoadScreenClose)");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('FRESH_PRODUCTION_04')",
        "fresh production requires",
    );
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.ThirdSpySend('BUILD_SPY')",
        "reloaded, unsaved or already sent",
    );
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_SPY')",
        "cannot replay",
    );
}

#[test]
fn fresh_production_refuses_other_turns_and_any_existing_native_production() {
    for fault in [
        "TURN=46",
        "TURN=48",
        "Game:GetProperty('JAC_BOUNDARY_SPY_PRODUCTION').progressed=true",
        "Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE').bound=true",
        "Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ACK').action='PROGRESS_SPY'",
        "Game:SetProperty('JAC_BOUNDARY_THIRD_PRODUCTION_UNCONFIRMED_04',{})",
    ] {
        let t = Third::new();
        t.run("TURN=46");
        t.begin();
        t.run("fire(Events.LoadScreenClose); TURN=47");
        t.run(fault);
        t.reject(
            "ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('FRESH_PRODUCTION_04')",
            "fresh production requires",
        );
        t.run("assert(#BUILD_REQUESTS==0 and PROGRESS_CALLS==0 and #UNIT_REQUESTS==0)");
    }
}

#[test]
fn fresh_production_cannot_replace_the_preserved_native_failure() {
    let t = Third::new();
    t.run("TURN=46");
    t.begin();
    t.run("fire(Events.LoadScreenClose); TURN=47; Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE').histories[2].turn=44");
    t.reject(
        "ExposedMembers.JAC_PersonaBoundary.ThirdSpyReserve('FRESH_PRODUCTION_04')",
        "preserved native failure changed",
    );
    t.run("assert(#BUILD_REQUESTS==0 and PROGRESS_CALLS==0 and #UNIT_REQUESTS==0)");
}

#[test]
fn adoption_requires_attempt_and_reservation_to_match_the_build_intent() {
    for fault in [
        "production.attempt=nil",
        "production.reservationToken=nil",
        "production.reservationToken=s.intents.BUILD_SPY.token+1",
        "s.intents.BUILD_SPY.attempt=nil",
        "s.intents.BUILD_SPY.token=nil",
        "Game:SetProperty('JAC_BOUNDARY_THIRD_PRODUCTION_UNCONFIRMED_04',nil)",
    ] {
        let t = Third::new();
        t.produce();
        t.run(&format!(
            "local s=Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE'); \
             local production=Game:GetProperty('JAC_BOUNDARY_SPY_PRODUCTION'); \
             s.productionAttempt=2; s.intents.BUILD_SPY.attempt=2; production.attempt=2; \
             production.reservationToken=s.intents.BUILD_SPY.token; \
             Game:SetProperty('JAC_BOUNDARY_THIRD_PRODUCTION_UNCONFIRMED_04',{{status='SENT_RESULT_UNCONFIRMED'}}); \
             {fault}; ExposedMembers.JAC_PersonaBoundary.ThirdSpyBind(); FRAME()"
        ));
        t.reject(
            "ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)",
            "fresh production attempt or reservation differs",
        );
        t.run("assert(not Game:GetProperty('JAC_BOUNDARY_SPY') and not Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE').attestation and PROGRESS_CALLS==1)");
    }
}

#[test]
fn saved_command_can_be_attested_after_ui_reload_without_replaying_native_production() {
    let t = Third::new();
    t.enable_immediate_production();
    t.begin();
    t.run(
        r#"
        fire(Events.SaveComplete); local b=ExposedMembers.JAC_PersonaBoundary;
        b.ThirdSpySend('BUILD_SPY'); FRAME(); FRAME(); b.Queue('PROGRESS_SPY'); FRAME();
        ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params);
        assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ACK').phase=='COMMAND');
        assert(PROGRESS_CALLS==1 and #UNITS[0]==1 and not Game:GetProperty('JAC_BOUNDARY_SPY_PROGRESS'));
        -- Recreate only UI dispatch state, not a claim of physical save/load.
        fire(Events.LoadScreenClose);
        "#,
    );
    t.bind();
    t.run(
        r#"
        local s=Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE');
        assert(s.attestation.productionCommand.action=='PROGRESS_SPY' and
         s.attestation.productionCommand.phase=='COMMAND');
        local binding=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').binding;
        assert(binding.sources.unit.identity=='CORE' and binding.sources.unit.level=='UI' and
         binding.sources.unit.levelUnavailable=='GetLevel');
        assert(PROGRESS_CALLS==1 and #BUILD_REQUESTS==1 and #UNIT_REQUESTS==0 and UI_CORE_CALLS==0);
        "#,
    );
}

#[test]
fn reservation_without_completed_progress_ack_cannot_supply_an_adoption() {
    for fault in [
        "Game:SetProperty('JAC_BOUNDARY_THIRD_SPY_ACK',nil)",
        "Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ACK').action='BEGIN'",
        "Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ACK').phase='RESERVED'",
        "Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ACK').turn=123",
        "Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ACK').observed.capacity=99",
        "Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ACK').observed=nil",
    ] {
        let t = Third::new();
        t.produce();
        t.run("ExposedMembers.JAC_PersonaBoundary.ThirdSpyBind(); FRAME()");
        t.run(fault);
        t.reject(
            "ENEMY_CASES.fireCore('JAC_BoundaryThirdSpyStep',REQUESTS[#REQUESTS].params)",
            "adoption lacks the completed production command",
        );
        t.run("assert(not Game:GetProperty('JAC_BOUNDARY_SPY') and not Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE').attestation and PROGRESS_CALLS==1)");
    }
}

#[test]
fn absent_experience_object_getter_remains_distinct_from_absent_level_getter() {
    let t = Third::new();
    t.produce();
    t.run("CORE_UNIT_FAULT=function(u) u.GetExperience=nil end");
    t.bind();
    t.run("local sources=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').binding.sources; assert(sources.unit.level=='UI' and sources.unit.levelUnavailable=='GetExperience')");
}
