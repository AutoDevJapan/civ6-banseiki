//! Lua 5.1 checks for the test-only cross-player neutralization fixture.
//! Engine doubles validate guards and observations, not a live Civ6 outcome.
use civ6_lua_tests_rs::{exec, new_lua, read_repo};
use mlua::Lua;
use rusqlite::Connection;

const VIEW: &str = "poc/JAC_Test_PersonaBoundary/UI/JAC_PersonaBoundary_View.lua";

struct Enemy(Lua);

impl Enemy {
    fn new() -> Self {
        let lua = new_lua();
        exec(
            &lua,
            include_str!("../fixtures/persona/boundary_engine.lua"),
        );
        exec(
            &lua,
            &read_repo("poc/JAC_Test_PersonaBoundary/Shared/JAC_PersonaBoundaryRules.lua"),
        );
        exec(&lua, &read_repo(VIEW));
        exec(
            &lua,
            &read_repo("poc/JAC_Test_PersonaBoundary/Scripts/JAC_PersonaBoundary_Fixture.lua"),
        );
        exec(
            &lua,
            include_str!("../fixtures/persona/boundary_enemy_cases.lua"),
        );
        exec(&lua, "ENEMY_CASES.setup()");
        Self(lua)
    }

    fn run(&self, code: &str) {
        self.0.load(code).exec().unwrap();
    }

    fn reject(&self, code: &str, expected: &str) {
        let error = self.0.load(code).exec().expect_err("must reject");
        assert!(error.to_string().contains(expected), "{error}");
    }

    fn prepare(&self) {
        self.sync("PREPARE_ENEMY", "JAC_BoundaryPrepareEnemy");
        self.sync("EVOLVE_ENEMY", "JAC_BoundaryEvolveEnemy");
    }

    fn bind(&self) {
        self.run("ENEMY_CASES.appoint(true)");
        self.sync("CONFIRM_ENEMY", "JAC_BoundaryConfirmEnemy");
    }

    fn capture(&self) {
        self.sync("CAPTURE_ENEMY", "JAC_BoundaryCaptureEnemyRecovery");
    }

    fn sync(&self, action: &str, event: &str) {
        self.run(&format!(
            "ExposedMembers.JAC_PersonaBoundary.Queue('{action}'); FRAME(); \
             local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='{event}'); \
             ENEMY_CASES.fireCore('{event}',r.params); FRAME(); \
             local b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier'); \
             ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); \
             fire(Events.GameCoreEventPublishComplete); FRAME(); \
             assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='ACCEPTED_{action}')"
        ));
    }

    fn success(&self) {
        self.prepare();
        self.bind();
        self.run("ENEMY_CASES.settled(); ENEMY_CASES.mission(2,12,0,4); ENEMY_CASES.neutralize(3)");
    }

    fn failed(&self) {
        self.prepare();
        self.bind();
        self.run("ENEMY_CASES.settled(); ENEMY_CASES.mission(EspionageResultTypes.FAIL_UNDETECTED,12,0,4); CIVIC_COMPLETED=true; local spy=makeSpy(0,15); spy.x=FOREIGN.x; spy.y=FOREIGN.y; Game:SetProperty('JAC_BOUNDARY_SPY',{owner=0,id=15}); Game:SetProperty('JAC_BOUNDARY_SPY_TRAVEL',{oldID=14,name='Spy 15',level=1,targetOwner=1,targetID=20}); Game:SetProperty('JAC_BOUNDARY_SPY_BARRIER',{owner=0,spyID=15,action='SPY_MISSION',token=1}); assert(ExposedMembers.JAC_PersonaBoundary.EnemyProgress()=='FAILED')");
    }

    fn barrier(&self) {
        self.run("local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryEnemyBarrier'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyBarrier',r.params); fire(Events.GameCoreEventPublishComplete)");
    }
}

#[test]
fn enemy_fixture_requires_real_actor_roles_join_and_one_official_title() {
    let h = Enemy::new();
    h.reject(
        "fire(GameEvents.JAC_BoundaryPrepareEnemy,1)",
        "controller 0",
    );
    h.prepare();
    h.run("local c=Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG'); assert(c.attacker==0 and c.owner==1 and c.source==2 and c.spent==0 and c.total==1); assert(Game:GetProperty('JAC_PERSONA_'..c.leader..'_STATE')=='EVOLVING'); assert(ENEMY_POINTS==1 and ENEMY_SPENT==0)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_ENEMY'); FRAME()",
        "fresh independent game",
    );
}

#[test]
fn enemy_preflight_rejects_missing_ui_bridge_or_unjoined_candidate_before_core_write() {
    let missing = Enemy::new();
    missing.run("ExposedMembers.JAC_LeaderPersonaNativeRead=nil");
    missing.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_ENEMY'); FRAME()",
        "product UI native Governor reader unavailable",
    );
    missing.run("assert(#REQUESTS==0 and ENEMY_POINTS==0 and Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG')==nil)");

    let other = Enemy::new();
    other.run("local get=Players[1].GetGovernors; Players[1].GetGovernors=function() local g=get(); g.CanEverAppointGovernor=function() return true end; return g end");
    other.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_ENEMY'); FRAME()",
        "another unjoined persona",
    );
    other.run("assert(#REQUESTS==0 and ENEMY_POINTS==0 and Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG')==nil)");
}

#[test]
fn stale_scalar_receipt_cannot_join_or_grant_a_title_in_gameplay() {
    let h = Enemy::new();
    h.run("ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_ENEMY'); FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryPrepareEnemy'); r.params.Turn=TURN-1");
    h.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryPrepareEnemy',REQUESTS[#REQUESTS].params)",
        "stale or belongs to another actor",
    );
    h.run("assert(ENEMY_POINTS==0 and Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG')==nil)");
}

#[test]
fn rejected_core_prepare_is_reported_after_the_barrier_instead_of_timing_out() {
    let h = Enemy::new();
    h.run("ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_ENEMY'); FRAME(); local r=REQUESTS[#REQUESTS]; local reader=ExposedMembers.JAC_LeaderPersonaNativeRead; ExposedMembers.JAC_LeaderPersonaNativeRead=function() return nil end; local ok=pcall(ENEMY_CASES.fireCore,'JAC_BoundaryPrepareEnemy',r.params); ExposedMembers.JAC_LeaderPersonaNativeRead=reader; assert(not ok and ENEMY_POINTS==0); FRAME(); local b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='REJECTED_PREPARE_ENEMY' and Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG')==nil)");
}

#[test]
fn evolution_before_the_join_publish_event_cannot_advance_persona_state() {
    let h = Enemy::new();
    h.run("ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_ENEMY'); FRAME(); local r=REQUESTS[#REQUESTS]; ENEMY_CASES.fireCore('JAC_BoundaryPrepareEnemy',r.params); EARLY={Action='EVOLVE_ENEMY',Attacker=0,Defender=1,Source=2,Leader=ENEMY_CASES.leader,Token=r.params.Token+1,Turn=TURN,Frame=r.params.Frame+1,PublishEpoch=r.params.PublishEpoch,Total=ENEMY_POINTS,Spent=ENEMY_SPENT}");
    h.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryEvolveEnemy',EARLY)",
        "enemy Join must complete before evolution preparation",
    );
    h.run("assert(Game:GetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_STATE')=='GREAT_PERSON' and not Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG').evolvedToken)");
}

#[test]
fn cross_player_appointment_is_judged_only_after_the_synchronized_barrier() {
    let h = Enemy::new();
    h.prepare();
    h.run("BASE_REQUESTS=#REQUESTS; ExposedMembers.JAC_PersonaBoundary.QueueEnemy('APPOINT'); FRAME(); assert(#REQUESTS==BASE_REQUESTS+1 and REQUESTS[#REQUESTS].owner==1 and REQUESTS[#REQUESTS].op==PlayerOperations.APPOINT_GOVERNOR); assert(ExposedMembers.JAC_PersonaBoundary.enemyLast=='DISPATCHED_APPOINT'); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.enemyLast=='BARRIER_SENT_APPOINT')");
    h.barrier();
    h.run("FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.enemyLast=='REJECTED_APPOINT'); assert(ENEMY_SPENT==0 and ENEMY_GOV==nil); assert(#REQUESTS==BASE_REQUESTS+2)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.QueueEnemy('APPOINT')",
        "already attempted before load",
    );

    let accepted = Enemy::new();
    accepted.prepare();
    accepted.run("ExposedMembers.JAC_PersonaBoundary.QueueEnemy('APPOINT'); FRAME(); ENEMY_CASES.appoint(false); FRAME()");
    accepted.barrier();
    accepted.run("FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.enemyLast=='ACCEPTED_APPOINT'); assert(ENEMY_SPENT==1)");
}

#[test]
fn rejected_foreign_assignment_can_fall_back_to_the_same_native_ai_governor() {
    let h = Enemy::new();
    h.prepare();
    h.run("ENEMY_CASES.appoint(false); assert(ExposedMembers.JAC_PersonaBoundary.TryEnemyAssignment()=='QUEUED_ASSIGN'); FRAME(); assert(REQUESTS[#REQUESTS].owner==1 and REQUESTS[#REQUESTS].op==PlayerOperations.ASSIGN_GOVERNOR and REQUESTS[#REQUESTS].params.city==20); FRAME()");
    h.barrier();
    h.run("FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.enemyLast=='REJECTED_ASSIGN'); ENEMY_GOV.assigned=FOREIGN");
    h.sync("CONFIRM_ENEMY", "JAC_BoundaryConfirmEnemy");
    h.run("local c=Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG'); assert(c.cityID==20 and c.x==2 and c.y==1); assert(Game:GetProperty('JAC_PERSONA_'..c.leader..'_STATE')=='GOVERNOR')");
    h.run("ENEMY_CASES.settled(); ExposedMembers.JAC_PersonaBoundary.CheckEnemy('SETTLED')");
}

#[test]
fn missing_native_governor_or_unprepared_person_cannot_become_a_target() {
    let h = Enemy::new();
    h.prepare();
    h.run("local c=Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG'); c.cityID=20; c.x=2; c.y=1; Game:SetProperty('JAC_BOUNDARY_ENEMY_CONFIG',c)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.ReadEnemy()",
        "native governor not appointed",
    );
    h.run("local c=Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG'); c.cityID=nil; c.x=nil; c.y=nil; Game:SetProperty('JAC_BOUNDARY_ENEMY_CONFIG',c)");
    h.bind();
    h.run("Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_STATE','GREAT_PERSON')");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.CheckEnemy('SETTLED')",
        "persona identity",
    );
}

#[test]
fn restored_native_binding_is_rechecked_without_waiting_for_an_old_reply() {
    let h = Enemy::new();
    h.prepare();
    h.bind();
    exec(&h.0, &read_repo(VIEW));
    h.run("local b=ExposedMembers.JAC_PersonaBoundary; assert(b.lastAction=='IDLE'); local before=#REQUESTS; assert(b.TryConfirmEnemy()=='NATIVE_BOUND'); assert(b.lastAction=='ALREADY_CONFIRMED_ENEMY' and #REQUESTS==before)");
    h.run("ENEMY_SPENT=2");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.TryConfirmEnemy()",
        "saved enemy appointment no longer matches",
    );
    h.run("ENEMY_SPENT=1; ENEMY_GOV.assigned=nil");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.TryConfirmEnemy()",
        "saved enemy appointment no longer matches",
    );
}

#[test]
fn missing_arrival_roster_is_unobserved_without_adopting_a_spy() {
    let h = Enemy::new();
    h.prepare();
    h.bind();
    h.run("Game:SetProperty('JAC_BOUNDARY_SPY_TRAVEL',{targetOwner=1,targetID=20,name='Spy',level=1,oldID=15}); local prior=#REQUESTS; assert(ExposedMembers.JAC_PersonaBoundary.TryAdoptEnemySpy()=='ARRIVAL_UNOBSERVED'); assert(#REQUESTS==prior and Game:GetProperty('JAC_BOUNDARY_SPY')==nil)");
}

#[test]
fn enemy_observation_is_serialized_inside_its_own_ui_context() {
    let h = Enemy::new();
    h.prepare();
    h.bind();
    h.run("JAC_PersonaBoundaryRules=nil; local b=ExposedMembers.JAC_PersonaBoundary; local before=#WRITES; local text=b.RecordEnemy('FAILED_MISSION_STATE'); assert(type(text)=='string' and string.find(text,'owner=1',1,true)); assert(#WRITES==before)");
}

#[test]
fn wrong_history_or_event_id_fails_but_success_with_capture_counts() {
    let h = Enemy::new();
    h.prepare();
    h.bind();
    h.run("ENEMY_CASES.settled(); ENEMY_CASES.neutralize(3); ENEMY_CASES.mission(2,99,0,4)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED')",
        "mission history was not observed",
    );
    h.run("ENEMY_CASES.mission(3,12,0,4)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED')",
        "mission history was not observed",
    );
    h.run("ENEMY_CASES.mission(2,12,0,4); Game:SetProperty('JAC_BOUNDARY_ENEMY_MISSION',{attacker=0,id=1,turn=TURN})");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED')",
        "mission history was not observed",
    );
    h.run("Game:SetProperty('JAC_BOUNDARY_ENEMY_MISSION',{attacker=0,id=0,turn=TURN}); ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED'); assert(ExposedMembers.JAC_PersonaBoundary.ReadEnemy().missions['0/0'].escape==EspionageResultTypes.CAPTURED)");
}

#[test]
fn neutralized_governor_establishment_flag_is_observed_without_inventing_its_value() {
    let h = Enemy::new();
    h.success();
    h.run(
        "ENEMY_GOV.established=true; ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED')",
    );
}

#[test]
fn enemy_native_mission_uses_one_human_spy_request_and_does_not_claim_success_early() {
    let h = Enemy::new();
    h.prepare();
    h.bind();
    h.run("ENEMY_CASES.settled(); local spy=makeSpy(0,15); spy.x=FOREIGN.x; spy.y=FOREIGN.y; Game:SetProperty('JAC_BOUNDARY_SPY',{owner=0,id=15}); REQUEST_EFFECT='mission'; ExposedMembers.JAC_PersonaBoundary.Queue('SPY_MISSION',{owner=1,city=20}); FRAME(); assert(#UNIT_REQUESTS==1 and UNIT_REQUESTS[1].spy:GetOwner()==0 and UNIT_REQUESTS[1].op==990); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='DISPATCHED_SPY_MISSION'); START_NATIVE(0,15,990); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='BARRIER_SENT_SPY_MISSION')");
    h.run("local r=REQUESTS[#REQUESTS]; fire(GameEvents.JAC_BoundarySpyBarrier,0,r.params); fire(Events.GameCoreEventPublishComplete); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='ACCEPTED_SPY_MISSION'); assert(ExposedMembers.JAC_PersonaBoundary.EnemyProgress()=='MISSION'); assert(#UNIT_REQUESTS==1)");
}

#[test]
fn attacker_completion_event_is_recorded_once_then_checked_against_native_history() {
    let h = Enemy::new();
    h.success();
    h.run("BASE_REQUESTS=#REQUESTS; Game:SetProperty('JAC_BOUNDARY_ENEMY_MISSION',nil); fire(Events.SpyMissionCompleted,1,0); FRAME(); assert(#REQUESTS==BASE_REQUESTS); fire(Events.SpyMissionCompleted,0,0); assert(ExposedMembers.JAC_PersonaBoundary.enemyMissionStatus=='OBSERVED'); FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.owner==0 and r.params.OnStart=='JAC_BoundaryRecordEnemyMission' and r.params.MissionID==0); fire(GameEvents.JAC_BoundaryRecordEnemyMission,0,r.params); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.enemyMissionStatus=='RECORDED'); ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED'); assert(#REQUESTS==BASE_REQUESTS+1)");
    h.run("fire(Events.SpyMissionCompleted,0,0); assert(ExposedMembers.JAC_PersonaBoundary.enemyMissionStatus=='RECORDED'); FRAME(); assert(#REQUESTS==BASE_REQUESTS+1)");
}

#[test]
fn finished_mission_without_completion_event_is_never_reported_as_success() {
    let h = Enemy::new();
    h.prepare();
    h.bind();
    h.run("ENEMY_CASES.settled(); local spy=makeSpy(0,15); spy.x=FOREIGN.x; spy.y=FOREIGN.y; spy.op=0; spy.finish=TURN+8; Game:SetProperty('JAC_BOUNDARY_SPY',{owner=0,id=15}); Game:SetProperty('JAC_BOUNDARY_SPY_BARRIER',{owner=0,spyID=15,action='SPY_MISSION',token=1}); assert(ExposedMembers.JAC_PersonaBoundary.EnemyMissionEventReady()); spy.op=-1; spy.finish=TURN");
    for _ in 0..12 {
        h.run("assert(not ExposedMembers.JAC_PersonaBoundary.EnemyMissionEventReady())");
    }
    h.run("assert(not ExposedMembers.JAC_PersonaBoundary.EnemyMissionEventReady())");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.EnemyProgress()",
        "without the attacker event",
    );
}

#[test]
fn merit_stoppage_accounts_for_legitimate_skill_spending_without_counting_it_as_earnings() {
    let h = Enemy::new();
    h.success();
    h.run("Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_MERIT',12)");
    h.capture();
    h.run("Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_NODE_'..ENEMY_CASES.skill,1); Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_MERIT',4); assert(ExposedMembers.JAC_PersonaBoundary.EnemyProgress()=='NEUTRALIZED')");
    h.run("Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_MERIT',5)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.EnemyProgress()",
        "earned merit",
    );
}

#[test]
fn acquired_skill_cannot_disappear_during_native_recovery() {
    let h = Enemy::new();
    h.success();
    h.run("Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_NODE_'..ENEMY_CASES.skill,1)");
    h.capture();
    h.run("Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_NODE_'..ENEMY_CASES.skill,nil)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.EnemyProgress()",
        "purchased native persona skill was lost",
    );
}

#[test]
fn recovery_survives_view_reload_stays_unassigned_and_requires_five_native_turns() {
    let h = Enemy::new();
    h.success();
    h.capture();
    h.run("assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_RECOVERY').merit==6); BEFORE=ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED')");
    exec(&h.0, &read_repo(VIEW));
    h.run("assert(ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED')==BEFORE); ENEMY_CASES.neutralize(0); ExposedMembers.JAC_PersonaBoundary.CheckEnemy('RECOVERED',6); Game:SetProperty('JAC_BOUNDARY_ENEMY_BARRIER',{attacker=0,defender=1,action='REASSIGN',token=1})");
    for turn in 0..5 {
        h.run(&format!("ENEMY_CASES.reassign({turn}); ExposedMembers.JAC_PersonaBoundary.CheckEnemy('ESTABLISHING',6)"));
    }
    h.run("ENEMY_CASES.reassign(5); ExposedMembers.JAC_PersonaBoundary.CheckEnemy('SETTLED'); assert(ExposedMembers.JAC_PersonaBoundary.EnemyProgress()=='SETTLED')");
    h.run(
        "ENEMY_CASES.reassign(3); Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_MERIT',7)",
    );
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.CheckEnemy('ESTABLISHING',6)",
        "merit changed",
    );
}

#[test]
fn diagnostic_sql_limits_date_to_ashina_without_changing_the_product_tables() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE Types(Type TEXT,Kind TEXT); CREATE TABLE Traits(TraitType TEXT,Name TEXT,Description TEXT); \
         CREATE TABLE Governors(GovernorType TEXT,TraitType TEXT); \
         CREATE TABLE JAC_LeaderPersonas(LeaderType TEXT,GovernorType TEXT); \
         CREATE TABLE CivilizationTraits(CivilizationType TEXT,TraitType TEXT); \
         INSERT INTO JAC_LeaderPersonas VALUES('LEADER_JAC_ASHINA_MORIKIYO','GOV_ASHINA'),('OTHER','GOV_OTHER'); \
         INSERT INTO Governors VALUES('GOV_ASHINA',NULL),('GOV_OTHER',NULL);",
    )
    .unwrap();
    db.execute_batch(&read_repo(
        "poc/JAC_Test_EnemyGovernorGate/Data/JAC_EnemyGovernorFixture.sql",
    ))
    .unwrap();
    let ashina: String = db
        .query_row(
            "SELECT TraitType FROM Governors WHERE GovernorType='GOV_ASHINA'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let other: String = db
        .query_row(
            "SELECT TraitType FROM Governors WHERE GovernorType='GOV_OTHER'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let date: String = db
        .query_row(
            "SELECT CivilizationType FROM CivilizationTraits WHERE TraitType=?1",
            [&ashina],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(date, "CIVILIZATION_JAC_DATE");
    assert_ne!(ashina, other);
}

#[test]
fn failed_native_history_and_empty_roster_require_finite_observation_before_retry() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; local d=ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis(); assert(d.mode=='OBSERVATION_PENDING' and d.eventID==0 and d.spyID==15 and d.occupied==0 and d.capacity==1); assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY')==nil)");
    h.run("ELAPSED=61; local d=ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis(); assert(d.mode=='PRODUCE' and d.outcome=='GONE_NO_CAPTURE_RECORD')");
    h.sync("BEGIN_SPY_RETRY", "JAC_BoundaryBeginEnemySpyRetry");
    h.run("local r=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY'); assert(r.current==2 and r.mode=='PRODUCE' and r.archive.eventID==0 and r.archive.spyID==15 and r.archive.initial==EspionageResultTypes.FAIL_UNDETECTED and r.archive.governorNeutralized==0 and r.archive.travelOldID==14 and r.archive.total==1 and r.archive.spent==1); assert(HISTORY[0][0].InitialResult==EspionageResultTypes.FAIL_UNDETECTED and Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION')==nil and Game:GetProperty('JAC_BOUNDARY_SPY')==nil); assert(ExposedMembers.JAC_PersonaBoundary.EnemyProgress()=='SETTLED_INITIAL')");
    exec(&h.0, &read_repo(VIEW));
    h.run("assert(ExposedMembers.JAC_PersonaBoundary.ReadEnemy().retry.archive.eventID==0)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis()",
        "already archived",
    );
}

#[test]
fn empty_roster_window_restarts_after_any_nonempty_native_snapshot() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; ELAPSED=0; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=39; CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='UNAVAILABLE_CAPACITY'); CAPTURED[1]={}; ELAPSED=40; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=99.999; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=100; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='PRODUCE'); assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION').id==0 and Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY')==nil)");
}

#[test]
fn ui_load_resets_empty_window_even_if_the_lua_context_survives() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; ELAPSED=0; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=55; fire(Events.LoadScreenClose); ELAPSED=60; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=119.999; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=120; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='PRODUCE')");
}

#[test]
fn changed_failed_receipt_or_rewound_ui_clock_starts_a_new_empty_window() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; ELAPSED=0; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=45; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ENEMY_CASES.mission(EspionageResultTypes.FAIL_UNDETECTED,12,1,4); ELAPSED=61; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=120.999; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=121; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='PRODUCE'); ELAPSED=0; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=59.999; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=60; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='PRODUCE')");
}

#[test]
fn preserved_failure_diagnostic_logs_native_history_without_fixture_writes() {
    let h = Enemy::new();
    h.failed();
    h.run("local before=#WRITES; assert(ExposedMembers.JAC_PersonaBoundary.ReadEnemyResultDiagnostic()=='FAILED'); assert(#WRITES==before); local found=false; for _,line in ipairs(LOGS) do if string.find(line,'ENEMY_FAILED_MISSION',1,true) and string.find(line,'initial=3.000000',1,true) then found=true end end; assert(found)");
}

#[test]
fn native_captive_occupies_capacity_and_prevents_new_production() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}; local d=ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis(); assert(d.mode=='UNAVAILABLE_CAPACITY' and d.outcome=='CAPTURED' and d.occupied==1 and d.capacity==1); assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY')==nil)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('BEGIN_SPY_RETRY'); FRAME()",
        "native retry route unavailable",
    );
    h.run("assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION').id==0 and HISTORY[0][0].InitialResult==EspionageResultTypes.FAIL_UNDETECTED)");
}

#[test]
fn persisted_production_binds_only_a_distinct_official_unit_after_one_real_turn() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; ELAPSED=0; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=61; PRODUCED_SPY_ID=16");
    h.sync("BEGIN_SPY_RETRY", "JAC_BoundaryBeginEnemySpyRetry");
    h.run("ExposedMembers.JAC_PersonaBoundary.Queue('SPAWN_SPY'); FRAME(); ENEMY_CASES.fireCore('JAC_BoundarySpawnSpy'); assert(Game:GetProperty('JAC_BOUNDARY_SPY_PRODUCTION').progressed==false); ExposedMembers.JAC_PersonaBoundary.SpyPrerequisitesReady(); ExposedMembers.JAC_PersonaBoundary.Queue('BUILD_SPY'); FRAME(); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='READY_BUILD_SPY'); ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_SPY'); FRAME(); ENEMY_CASES.fireCore('JAC_BoundaryProgressSpy'); assert(PROGRESS_CALLS==1 and Game:GetProperty('JAC_BOUNDARY_SPY_PRODUCTION').progressed)");
    h.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryProgressSpy')",
        "already added",
    );
    h.run("ADVANCE_TURN(); assert(ExposedMembers.JAC_PersonaBoundary.TryBindRetrySpy()=='BIND_QUEUED'); FRAME(); ENEMY_CASES.fireCore('JAC_BoundaryBindSpy'); local r=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY'); assert(r.spyID==16 and r.archive.spyID==15 and Game:GetProperty('JAC_BOUNDARY_SPY').id==16 and PROGRESS_CALLS==1)");
    h.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryBindSpy')",
        "not completed once",
    );
    h.run("UNITS[0]={}; local arrived=makeSpy(0,17); arrived.x=FOREIGN.x; arrived.y=FOREIGN.y; ENEMY_CASES.fireCore('JAC_BoundaryAdoptSpyIdentity',{OldID=16,NewID=17,TargetOwner=1,TargetID=20}); assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').spyID==17 and Game:GetProperty('JAC_BOUNDARY_SPY').id==17); ExposedMembers.JAC_PersonaBoundary.Queue('SPY_MISSION',{owner=1,city=20})");
}

#[test]
fn archived_failed_event_cannot_satisfy_new_mission_or_be_recorded_twice() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=61");
    h.sync("BEGIN_SPY_RETRY", "JAC_BoundaryBeginEnemySpyRetry");
    h.run("fire(Events.SpyMissionCompleted,0,0); assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION')==nil); assert(ExposedMembers.JAC_PersonaBoundary.EnemyProgress()=='SETTLED_INITIAL')");
    h.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryRecordEnemyMission',{Attacker=0,MissionID=0})",
        "archived failed native mission",
    );
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED')",
        "native neutralization was not observed",
    );
}

#[test]
fn living_off_map_spy_returns_with_a_new_native_id_without_creating_another_spy() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; OFF_MAP={{Name='Spy 15',Level=1,ReturnTurn=TURN+2,XLocation=CONTROL.x,YLocation=CONTROL.y}}; local d=ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis(); assert(d.mode=='RETURN' and d.outcome=='LIVING_OFF_MAP')");
    h.sync("BEGIN_SPY_RETRY", "JAC_BoundaryBeginEnemySpyRetry");
    h.run("assert(ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()=='OFF_MAP_RETURN'); local spy=ARRIVE_OFFMAP(16); assert(spy:GetID()==16 and ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()=='ADOPTION_REQUIRED'); assert(ExposedMembers.JAC_PersonaBoundary.TryAdoptReturnedSpy()=='ADOPT_QUEUED'); FRAME(); FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryAdoptSpyIdentity'); ENEMY_CASES.fireCore('JAC_BoundaryAdoptSpyIdentity',r.params); fire(Events.GameCoreEventPublishComplete); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='ACCEPTED_RETRY_ADOPT'); assert(ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()=='HOME_READY'); assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').spyID==16 and PROGRESS_CALLS==0)");
}

#[test]
fn off_map_return_matches_archived_name_level_turn_and_both_coordinates() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; OFF_MAP={{Name='Spy 15',Level=1,ReturnTurn=TURN+2,XLocation=CONTROL.x,YLocation=CONTROL.y}}; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='RETURN')");
    h.sync("BEGIN_SPY_RETRY", "JAC_BoundaryBeginEnemySpyRetry");
    h.run("local r=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY'); assert(r.returnName=='Spy 15' and r.returnX==CONTROL.x and r.returnY==CONTROL.y and r.returnLevel==1 and r.returnTurn==TURN+2); assert(ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()=='OFF_MAP_RETURN'); OFF_MAP[1].Name='Other Spy'");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()",
        "native return record changed",
    );
    h.run("OFF_MAP[1].Name='Spy 15'; OFF_MAP[1].XLocation=TARGET.x");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()",
        "native return record changed",
    );
    h.run("OFF_MAP[1].XLocation=CONTROL.x; OFF_MAP[1].YLocation=FOREIGN.y");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()",
        "native return record changed",
    );
    h.run("OFF_MAP[1].YLocation=CONTROL.y; OFF_MAP[2]={Name='Spy 15',Level=1,ReturnTurn=TURN+2,XLocation=CONTROL.x,YLocation=CONTROL.y}");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()",
        "living retry operative roster is ambiguous",
    );
    h.run("OFF_MAP={}; CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()",
        "became captive",
    );
}

#[test]
fn unnamed_default_spy_uses_the_proven_empty_off_map_name_only_at_intake() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; local travel=Game:GetProperty('JAC_BOUNDARY_SPY_TRAVEL'); travel.name=GameInfo.Units['UNIT_SPY'].Name; Game:SetProperty('JAC_BOUNDARY_SPY_TRAVEL',travel); OFF_MAP={{Name='',Level=1,ReturnTurn=TURN+2,XLocation=CONTROL.x,YLocation=CONTROL.y}}; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='RETURN')");
    h.sync("BEGIN_SPY_RETRY", "JAC_BoundaryBeginEnemySpyRetry");
    h.run("assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').returnName==''); assert(ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()=='OFF_MAP_RETURN'); OFF_MAP[1].Name=GameInfo.Units['UNIT_SPY'].Name");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()",
        "native return record changed",
    );
}

#[test]
fn already_returned_map_spy_with_new_id_is_bound_without_production() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; local spy=makeSpy(0,16); spy.name='Spy 15'; spy.x=CONTROL.x; spy.y=CONTROL.y; local d=ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis(); assert(d.mode=='MAP' and d.currentID==16 and d.spyID==15)");
    h.sync("BEGIN_SPY_RETRY", "JAC_BoundaryBeginEnemySpyRetry");
    h.run("local r=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY'); assert(r.archive.spyID==15 and r.spyID==16 and Game:GetProperty('JAC_BOUNDARY_SPY').id==16); assert(ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()=='HOME_READY'); assert(PROGRESS_CALLS==0)");
}

#[test]
fn retry_refuses_a_second_official_mission_request_after_its_native_barrier() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=61");
    h.sync("BEGIN_SPY_RETRY", "JAC_BoundaryBeginEnemySpyRetry");
    h.run("local spy=makeSpy(0,16); Game:SetProperty('JAC_BOUNDARY_SPY',{owner=0,id=16}); local r=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY'); r.spyID=16; Game:SetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY',r); ENEMY_CASES.fireCore('JAC_BoundarySpyBarrier',{Token=4,SpyID=16,Action='SPY_MISSION'}); assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY').missionRequested)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('SPY_MISSION',{owner=1,city=20})",
        "already been requested",
    );
    h.reject(
        "ENEMY_CASES.fireCore('JAC_BoundarySpyBarrier',{Token=5,SpyID=16,Action='SPY_MISSION'})",
        "second official mission request",
    );
    h.run("assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION')==nil and HISTORY[0][0].InitialResult==EspionageResultTypes.FAIL_UNDETECTED)");
}
