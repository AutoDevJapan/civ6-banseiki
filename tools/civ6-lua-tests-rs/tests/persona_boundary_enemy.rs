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
        self.run("fire(GameEvents.JAC_BoundaryPrepareEnemy,0)");
    }

    fn bind(&self) {
        self.run("ENEMY_CASES.appoint(true); ENEMY_CASES.bind()");
    }

    fn success(&self) {
        self.prepare();
        self.bind();
        self.run("ENEMY_CASES.settled(); ENEMY_CASES.mission(2,12,0,4); ENEMY_CASES.neutralize(3)");
    }

    fn barrier(&self) {
        self.run("local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryEnemyBarrier'); fire(GameEvents.JAC_BoundaryEnemyBarrier,0,r.params); fire(Events.GameCoreEventPublishComplete)");
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
    h.reject("fire(GameEvents.JAC_BoundaryPrepareEnemy,0)", "fresh game");
}

#[test]
fn cross_player_appointment_is_judged_only_after_the_synchronized_barrier() {
    let h = Enemy::new();
    h.prepare();
    h.run("ExposedMembers.JAC_PersonaBoundary.QueueEnemy('APPOINT'); FRAME(); assert(#REQUESTS==1 and REQUESTS[1].owner==1 and REQUESTS[1].op==PlayerOperations.APPOINT_GOVERNOR); assert(ExposedMembers.JAC_PersonaBoundary.enemyLast=='DISPATCHED_APPOINT'); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.enemyLast=='BARRIER_SENT_APPOINT')");
    h.barrier();
    h.run("FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.enemyLast=='REJECTED_APPOINT'); assert(ENEMY_SPENT==0 and ENEMY_GOV==nil); assert(#REQUESTS==2)");
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
    h.run("ENEMY_CASES.appoint(false); assert(ExposedMembers.JAC_PersonaBoundary.TryEnemyAssignment()=='QUEUED_ASSIGN'); FRAME(); assert(REQUESTS[1].owner==1 and REQUESTS[1].op==PlayerOperations.ASSIGN_GOVERNOR and REQUESTS[1].params.city==20); FRAME()");
    h.barrier();
    h.run("FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.enemyLast=='REJECTED_ASSIGN'); ENEMY_GOV.assigned=FOREIGN; ENEMY_CASES.bind(); local c=Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG'); assert(c.cityID==20 and c.x==2 and c.y==1); assert(Game:GetProperty('JAC_PERSONA_'..c.leader..'_STATE')=='GOVERNOR')");
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
    h.run("ENEMY_CASES.appoint(true); ENEMY_CASES.bind()");
    h.run("Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_STATE','GREAT_PERSON')");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.CheckEnemy('SETTLED')",
        "persona identity",
    );
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
    h.run("Game:SetProperty('JAC_BOUNDARY_ENEMY_MISSION',nil); fire(Events.SpyMissionCompleted,1,0); FRAME(); assert(#REQUESTS==0); fire(Events.SpyMissionCompleted,0,0); assert(ExposedMembers.JAC_PersonaBoundary.enemyMissionStatus=='OBSERVED'); FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.owner==0 and r.params.OnStart=='JAC_BoundaryRecordEnemyMission' and r.params.MissionID==0); fire(GameEvents.JAC_BoundaryRecordEnemyMission,0,r.params); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.enemyMissionStatus=='RECORDED'); ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED'); assert(#REQUESTS==1)");
    h.run("fire(Events.SpyMissionCompleted,0,0); assert(ExposedMembers.JAC_PersonaBoundary.enemyMissionStatus=='RECORDED'); FRAME(); assert(#REQUESTS==1)");
}

#[test]
fn finished_mission_without_completion_event_is_never_reported_as_success() {
    let h = Enemy::new();
    h.prepare();
    h.bind();
    h.run("ENEMY_CASES.settled(); local spy=makeSpy(0,15); spy.x=FOREIGN.x; spy.y=FOREIGN.y; spy.op=0; spy.finish=TURN+8; Game:SetProperty('JAC_BOUNDARY_SPY',{owner=0,id=15}); Game:SetProperty('JAC_BOUNDARY_SPY_BARRIER',{owner=0,spyID=15,action='SPY_MISSION',token=1}); assert(ExposedMembers.JAC_PersonaBoundary.EnemyMissionEventReady()); spy.op=-1; spy.finish=TURN");
    for _ in 0..9 {
        h.run("assert(not ExposedMembers.JAC_PersonaBoundary.EnemyMissionEventReady())");
    }
    h.run("assert(ExposedMembers.JAC_PersonaBoundary.EnemyMissionEventReady())");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.EnemyProgress()",
        "without the attacker event",
    );
}

#[test]
fn merit_stoppage_accounts_for_legitimate_skill_spending_without_counting_it_as_earnings() {
    let h = Enemy::new();
    h.success();
    h.run("Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_MERIT',12); fire(GameEvents.JAC_BoundaryCaptureEnemyRecovery,0); Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_NODE_'..ENEMY_CASES.skill,1); Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_MERIT',4); assert(ExposedMembers.JAC_PersonaBoundary.EnemyProgress()=='NEUTRALIZED')");
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
    h.run("Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_NODE_'..ENEMY_CASES.skill,1); fire(GameEvents.JAC_BoundaryCaptureEnemyRecovery,0); Game:SetProperty('JAC_PERSONA_'..ENEMY_CASES.leader..'_NODE_'..ENEMY_CASES.skill,nil)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.EnemyProgress()",
        "purchased native persona skill was lost",
    );
}

#[test]
fn recovery_survives_view_reload_stays_unassigned_and_requires_five_native_turns() {
    let h = Enemy::new();
    h.success();
    h.run("fire(GameEvents.JAC_BoundaryCaptureEnemyRecovery,0); assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_RECOVERY').merit==6); BEFORE=ExposedMembers.JAC_PersonaBoundary.CheckEnemy('NEUTRALIZED')");
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
