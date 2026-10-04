//! Lua 5.1 checks for the test-only cross-player neutralization fixture.
//! Engine doubles validate guards and observations, not a live Civ6 outcome.
use civ6_data_rs::{cascade, Repository};
use civ6_lua_tests_rs::strict::ChunkExt;
use civ6_lua_tests_rs::{exec, new_lua, read_repo};
use mlua::Lua;
use rusqlite::Connection;
use serde_json::Value;

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
        let read = if action == "PREPARE_DISTRICT_CIVIC" {
            "local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryReadCapacityCivic'); \
             ENEMY_CASES.fireCore('JAC_BoundaryReadCapacityCivic',r.params); FRAME(); \
             local b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier' and b.params.Phase=='READ'); \
             ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); \
             fire(Events.GameCoreEventPublishComplete); FRAME();"
        } else {
            ""
        };
        self.run(&format!(
            "ExposedMembers.JAC_PersonaBoundary.Queue('{action}'); FRAME(); \
             local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='{event}'); \
             ENEMY_CASES.fireCore('{event}',r.params); FRAME(); \
             local b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier'); \
             ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); \
             fire(Events.GameCoreEventPublishComplete); FRAME(); {read} \
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

    fn captured_district(&self) {
        self.failed();
        self.run("UNITS[0]={}; CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}; OFF_MAP={{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}; GOV_CIVIC_COMPLETED=true");
    }

    fn captured_deal(&self) {
        self.failed();
        self.run(r#"
            UNITS[0]={};
            local travel=Game:GetProperty('JAC_BOUNDARY_SPY_TRAVEL');
            travel.name=GameInfo.Units['UNIT_SPY'].Name;
            CAPTURED[1]={{OwningPlayer=0,Name='',Level=1,NameIndex=-1}};
            OFF_MAP={{Name='',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}};
            DealDirection={OUTGOING=2,INCOMING=3};
            DealItemTypes={CAPTIVE=4};
            DealProposalAction={PENDING=0,PROPOSED=1,ACCEPTED=2,REJECTED=3,ADJUSTED=4};
            DealValidationResult={VALID=0,INVALID=1};
            OUTGOING={}; INCOMING={}; NEXT_DEAL_ID=1; SENDS={};
            AI_REPLY='ACCEPT'; ITEM_VALID=true; DEAL_VALIDATION_RESULT=0; SESSION_ID=nil;
            function makeDealItem(kind,from,to,value,locked)
                local item={id=NEXT_DEAL_ID,kind=kind,from=from,to=to,value=value,locked=locked or false};
                NEXT_DEAL_ID=NEXT_DEAL_ID+1;
                item.GetID=function() return item.id end;
                item.GetType=function() return item.kind end;
                item.GetFromPlayerID=function() return item.from end;
                item.GetToPlayerID=function() return item.to end;
                item.GetValueType=function() return item.value end;
                item.GetValueTypeNameID=function()
                    if item.value==-1 then return nil end;
                    return 'LOC_TEST_CAPTIVE_NAME';
                end;
                item.SetValueType=function(_,v)
                    if FAIL_SET_VALUE then error('native captive value setter unavailable') end;
                    item.value=v;
                end;
                item.IsValid=function() return ITEM_VALID end;
                item.SetLocked=function(_,v) item.locked=v end;
                item.IsLocked=function() return item.locked end;
                return item;
            end
            function makeDeal(items)
                return {
                    GetItemCount=function() return #items end,
                    Items=function() local i=0; return function() i=i+1; return items[i] end end,
                    FindItemByID=function(_,id)
                        for _,item in ipairs(items) do if item.id==id then return item end end
                    end,
                    AddItemOfType=function(_,kind,from)
                        assert(kind==DealItemTypes.CAPTIVE and from==1);
                        local item=makeDealItem(kind,from,0,-999,false);
                        items[#items+1]=item; return item;
                    end,
                    RemoveItemByID=function(_,id)
                        for i,item in ipairs(items) do if item.id==id then
                            table.remove(items,i); return true;
                        end end;
                        return false;
                    end,
                    Validate=function() return DEAL_VALIDATION_RESULT end
                };
            end
            OUT_DEAL=makeDeal(OUTGOING); IN_DEAL=makeDeal(INCOMING);
            DealManager={
                HasPendingDeal=function(a,b) assert(a==0 and b==1); return PENDING_DEAL==true end,
                ClearWorkingDeal=function() error('must not clear unsent native deal') end,
                GetWorkingDeal=function(direction,a,b)
                    assert(a==0 and b==1);
                    return direction==2 and OUT_DEAL or IN_DEAL;
                end,
                AreWorkingDealsEqual=function(a,b)
                    assert(a==0 and b==1);
                    if #OUTGOING~=#INCOMING then return false end;
                    for i,item in ipairs(OUTGOING) do
                        local other=INCOMING[i];
                        if item.kind~=other.kind or item.from~=other.from or
                            item.to~=other.to or item.value~=other.value then return false end;
                    end;
                    return true;
                end,
                SendWorkingDeal=function(action,a,b)
                    assert(a==0 and b==1);
                    SENDS[#SENDS+1]=action;
                    if action==DealProposalAction.PROPOSED then
                        if AI_REPLY=='ACCEPT' then
                            INCOMING={makeDealItem(4,1,0,-1,true)};
                            IN_DEAL=makeDeal(INCOMING);
                            fire(Events.DiplomacyIncomingDeal,1,0,DealProposalAction.ACCEPTED);
                        elseif AI_REPLY=='REJECT' then
                            fire(Events.DiplomacyIncomingDeal,1,0,DealProposalAction.REJECTED);
                        elseif AI_REPLY=='COUNTER' then
                            INCOMING={makeDealItem(4,1,0,-1,true),makeDealItem(99,0,1,100,false)};
                            IN_DEAL=makeDeal(INCOMING);
                            fire(Events.DiplomacyIncomingDeal,1,0,DealProposalAction.ADJUSTED);
                        elseif AI_REPLY=='UNKNOWN' then
                            fire(Events.DiplomacyIncomingDeal,1,0,99);
                        end;
                    elseif action==DealProposalAction.ACCEPTED then
                        assert(AI_REPLY=='ACCEPT' and DealManager.AreWorkingDealsEqual(a,b));
                        CAPTURED[1]={}; OFF_MAP={};
                        if RETURN_OFFMAP then
                            OFF_MAP={{Name='',Level=1,ReturnTurn=TURN+2,
                                XLocation=CONTROL.x,YLocation=CONTROL.y}};
                        else
                            local returned=makeSpy(0,16);
                            returned.name=GameInfo.Units['UNIT_SPY'].Name;
                            returned.level=1; returned.x=CONTROL.x; returned.y=CONTROL.y;
                        end;
                    else error('unexpected native deal action') end;
                end
            };
            DiplomacyManager={
                RequestSession=function(a,b,kind)
                    assert(a==0 and b==1 and kind=='MAKE_DEAL'); SESSION_ID=44;
                end,
                FindOpenSessionID=function(a,b) assert(a==0 and b==1); return SESSION_ID end,
                GetSessionInfo=function(id) assert(id==44); return {FromPlayer=0} end,
                CloseSession=function(id) assert(id==44); SESSION_ID=nil end
            };
        "#);
    }

    fn captive_capacity(&self) {
        self.failed();
        self.run("UNITS[0]={}; TURN=23; Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION').turn=23; HISTORY[0][0].CompletionTurn=23; Game:GetProperty('JAC_BOUNDARY_SPY').id=655368; Game:GetProperty('JAC_BOUNDARY_SPY_TRAVEL').name=GameInfo.Units['UNIT_SPY'].Name; CAPTURED[1]={{OwningPlayer=0,Name='',Level=1,NameIndex=-1}}; OFF_MAP={{Name='',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}; assert(ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan().stage=='NEEDS_DOMAIN_LAW')");
    }

    fn civic_roundtrip(&self, deliver_command: bool) {
        self.run("ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_DISTRICT_CIVIC')");
        self.civic_complete_queued(deliver_command);
    }

    fn civic_complete_queued(&self, deliver_command: bool) {
        self.run("FRAME(); assert(REQUESTS[#REQUESTS].params.OnStart=='JAC_BoundaryPrepareCapacityCivic'); SETTER_PARAMS=REQUESTS[#REQUESTS].params");
        if deliver_command {
            self.run("ENEMY_CASES.fireCore('JAC_BoundaryPrepareCapacityCivic',REQUESTS[#REQUESTS].params)");
        }
        self.run("FRAME(); local b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier' and b.params.Phase=='COMMAND'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryReadCapacityCivic' and r.params.Phase=='READ'); FIRST_READ_PARAMS=r.params; ENEMY_CASES.fireCore('JAC_BoundaryReadCapacityCivic',r.params); FRAME(); b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier' and b.params.Phase=='READ'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME()");
    }

    fn civic_resume_complete_queued(&self) {
        self.run("FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryReadCapacityCivic' and r.params.Action=='RESUME_CAPACITY_CIVIC_READ' and r.params.Phase=='RESUME_READ'); RESUME_READ_PARAMS=r.params; ENEMY_CASES.fireCore('JAC_BoundaryReadCapacityCivic',r.params); FRAME(); local b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier' and b.params.Phase=='RESUME_READ'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME()");
    }

    fn pin_captive_session(&self) {
        self.run("local b=ExposedMembers.JAC_PersonaBoundary; assert(b.CaptiveExchangeState().stage=='SESSION_UNBOUND'); assert(b.PinObservedCaptiveExchangeSession(SESSION_ID)==SESSION_ID); assert(b.CaptiveExchangeState().stage=='SESSION_OPEN')");
    }

    fn capacity_government_ready(&self) {
        self.captive_capacity();
        self.run("GOV_CIVIC_COMPLETED=true; GOV_DISTRICT_BUILT=true; GOV_DISTRICT_PLACED=true; GOV_CURRENT=0; GOV_CIVIC_COMPLETED_THIS_TURN=true; local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='NEEDS_TIER1_GOVERNMENT' and p.government.target.governmentType=='GOVERNMENT_OLIGARCHY' and p.government.currentType=='GOVERNMENT_CHIEFDOM' and p.government.stage=='READY_CHANGE')");
    }

    fn capacity_government_complete_queued(&self) {
        self.run("GOV_CALLS_BEFORE=GOV_REQUEST_CALLS; FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryCapacityGovernmentIntent'); GOV_INTENT_PARAMS=r.params; ENEMY_CASES.fireCore('JAC_BoundaryCapacityGovernmentIntent',r.params); FRAME(); local b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier' and b.params.Phase=='GOV_INTENT'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='REQUESTED_CHANGE_CAPACITY_GOVERNMENT' and GOV_REQUEST_CALLS==GOV_CALLS_BEFORE+1)");
    }

    fn capacity_government_complete_next_turn(&self) {
        self.run("FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryCapacityGovernmentRead' and r.params.Phase=='GOV_READ' and r.params.Turn==TURN and r.params.Token>GOV_INTENT_PARAMS.Token); GOV_READ_PARAMS=r.params; ENEMY_CASES.fireCore('JAC_BoundaryCapacityGovernmentRead',r.params); FRAME(); local b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier' and b.params.Phase=='GOV_READ'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME()");
    }

    fn tier2_civic_complete_queued(&self) {
        self.run("FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryPrepareTier2CapacityCivic'); TIER2_SETTER_PARAMS=r.params; ENEMY_CASES.fireCore('JAC_BoundaryPrepareTier2CapacityCivic',r.params); FRAME(); local b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier' and b.params.Phase=='COMMAND'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME(); r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryReadTier2CapacityCivic' and r.params.Phase=='READ'); ENEMY_CASES.fireCore('JAC_BoundaryReadTier2CapacityCivic',r.params); FRAME(); b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier' and b.params.Phase=='READ'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME()");
    }

    fn tier2_civic_complete_next_turn(&self) {
        self.run("FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryReadTier2CapacityCivic' and r.params.Phase=='RESUME_READ' and r.params.Turn==TURN); TIER2_READ_PARAMS=r.params; ENEMY_CASES.fireCore('JAC_BoundaryReadTier2CapacityCivic',r.params); FRAME(); local b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier' and b.params.Phase=='RESUME_READ'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME()");
    }

    fn barrier(&self) {
        self.run("local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryEnemyBarrier'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyBarrier',r.params); fire(Events.GameCoreEventPublishComplete)");
    }
}

fn capacity_civic_scene() -> Value {
    serde_json::from_str(&read_repo(
        "tools/civ6-autotest/scenarios/leader-persona-spy-neutralization-retry-capacity-civic.json",
    ))
    .unwrap()
}

fn capacity_government_scene(suffix: &str) -> Value {
    serde_json::from_str(&read_repo(format!(
        "tools/civ6-autotest/scenarios/leader-persona-spy-neutralization-retry-capacity-government-{suffix}.json"
    )))
    .unwrap()
}

fn scene_lua(scene: &Value, index: usize) -> &str {
    scene["steps"][index]["code"]
        .as_str()
        .or_else(|| scene["steps"][index]["condition"]["code"].as_str())
        .unwrap()
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
fn cascaded_official_spy_capacity_has_one_civic_and_one_government_building_route() {
    let repo = Repository::checked_in();
    let official = cascade::official_db_for(&repo).unwrap();
    let db = cascade::run_sengoku(&repo, &official, true).unwrap();
    let mut civic = db.prepare("SELECT c.CivicType,c.ModifierId,m.ModifierType,d.EffectType,a.Name,a.Value FROM CivicModifiers c JOIN Modifiers m USING(ModifierId) JOIN DynamicModifiers d USING(ModifierType) JOIN ModifierArguments a USING(ModifierId) WHERE d.EffectType='EFFECT_GRANT_SPY' ORDER BY c.CivicType").unwrap();
    let civic_rows = civic
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        civic_rows,
        vec![(
            "CIVIC_JAC_SENGOKU_LOCAL_LEAGUES_AND_INTELLIGENCE".into(),
            "CIVIC_GRANT_SPY".into(),
            "MODIFIER_PLAYER_GRANT_SPY".into(),
            "EFFECT_GRANT_SPY".into(),
            "Amount".into(),
            "1".into()
        )]
    );
    let mut building = db.prepare("SELECT b.BuildingType,b.ModifierId,m.ModifierType,d.EffectType,a.Name,a.Value FROM BuildingModifiers b JOIN Modifiers m USING(ModifierId) JOIN DynamicModifiers d USING(ModifierType) JOIN ModifierArguments a USING(ModifierId) WHERE d.EffectType='EFFECT_GRANT_SPY' ORDER BY b.BuildingType").unwrap();
    let building_rows = building
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        building_rows,
        vec![(
            "BUILDING_GOV_SPIES".into(),
            "GOV_GRANT_SPY".into(),
            "MODIFIER_PLAYER_GRANT_SPY".into(),
            "EFFECT_GRANT_SPY".into(),
            "Amount".into(),
            "1".into()
        )]
    );
    let tech_count: i64 = db.query_row("SELECT count(*) FROM TechnologyModifiers t JOIN Modifiers m USING(ModifierId) JOIN DynamicModifiers d USING(ModifierType) WHERE d.EffectType='EFFECT_GRANT_SPY'",[],|r|r.get(0)).unwrap();
    assert_eq!(tech_count, 0);
    let district: String = db
        .query_row(
            "SELECT PrereqCivic FROM Districts WHERE DistrictType='DISTRICT_GOVERNMENT'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(district, "CIVIC_JAC_SENGOKU_DOMAIN_LAW");
    let building_district: String = db
        .query_row(
            "SELECT PrereqDistrict FROM Buildings WHERE BuildingType='BUILDING_GOV_SPIES'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(building_district, "DISTRICT_GOVERNMENT");
    let mut prereqs=db.prepare("SELECT PrereqBuilding FROM BuildingPrereqs WHERE Building='BUILDING_GOV_SPIES' ORDER BY PrereqBuilding").unwrap();
    let prereq_rows = prereqs
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        prereq_rows,
        [
            "BUILDING_GOV_CONQUEST",
            "BUILDING_GOV_TALL",
            "BUILDING_GOV_WIDE"
        ]
    );
    let mut tiers = db.prepare("SELECT BuildingType,GovernmentTierRequirement,PrereqCivic FROM Buildings WHERE BuildingType IN ('BUILDING_GOV_TALL','BUILDING_GOV_WIDE','BUILDING_GOV_CONQUEST','BUILDING_GOV_SPIES') ORDER BY BuildingType").unwrap();
    let tier_rows = tiers
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        tier_rows,
        [
            ("BUILDING_GOV_CONQUEST".into(), "Tier1".into(), None),
            ("BUILDING_GOV_SPIES".into(), "Tier2".into(), None),
            ("BUILDING_GOV_TALL".into(), "Tier1".into(), None),
            ("BUILDING_GOV_WIDE".into(), "Tier1".into(), None),
        ]
    );
    let mut governments = db
        .prepare("SELECT GovernmentType,Tier,PrereqCivic FROM Governments ORDER BY GovernmentType")
        .unwrap();
    let government_rows = governments
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        government_rows,
        [
            ("GOVERNMENT_CHIEFDOM".into(), None, None),
            (
                "GOVERNMENT_MERCHANT_REPUBLIC".into(),
                Some("Tier2".into()),
                Some("CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS".into()),
            ),
            (
                "GOVERNMENT_MONARCHY".into(),
                Some("Tier2".into()),
                Some("CIVIC_JAC_SENGOKU_WARRIOR_FARMER_SEPARATION".into()),
            ),
            (
                "GOVERNMENT_OLIGARCHY".into(),
                Some("Tier1".into()),
                Some("CIVIC_JAC_SENGOKU_DOMAIN_LAW".into()),
            ),
        ]
    );
    let dangling_unlocks: i64 = db
        .query_row(
            "SELECT count(*) FROM Governments WHERE GovernmentType IN ('GOVERNMENT_OLIGARCHY','GOVERNMENT_MERCHANT_REPUBLIC','GOVERNMENT_MONARCHY') AND PolicyToUnlock IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(dangling_unlocks, 0);
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
fn normal_turn_and_governor_progress_do_not_restart_an_empty_roster_window() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; ELAPSED=0; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); TURN=TURN+1; ENEMY_GOV.onSite=ENEMY_GOV.onSite+1; ENEMY_POINTS=ENEMY_POINTS+1; ELAPSED=59.999; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='OBSERVATION_PENDING'); ELAPSED=60; assert(ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='PRODUCE'); assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY')==nil)");
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
fn live_86_captive_and_off_map_sentinel_are_one_operative_with_raw_records_retained() {
    let h = Enemy::new();
    h.run("EspionageResultTypes.FAIL_UNDETECTED=1; EspionageResultTypes.SUCCESS_UNDETECTED=2; EspionageResultTypes.SUCCESS_MUST_ESCAPE=3");
    h.failed();
    h.run("UNITS[0]={}; TURN=23; local e=Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'); e.turn=23; local mission=HISTORY[0][0]; mission.CompletionTurn=23; mission.Operation=59; mission.PlotIndex=8511; mission.EscapeResult=-1; GameInfo.UnitOperations[59]=GameInfo.UnitOperations[0]; PLOTS[FOREIGN.x..'/'..FOREIGN.y].GetIndex=function() return 8511 end; local spy=Game:GetProperty('JAC_BOUNDARY_SPY'); spy.id=655368; local travel=Game:GetProperty('JAC_BOUNDARY_SPY_TRAVEL'); travel.name=GameInfo.Units['UNIT_SPY'].Name; travel.level=1; CAPTURED[1]={{OwningPlayer=0,Name='',Level=1,NameIndex=-1}}; OFF_MAP={{Name='',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}; local before=#WRITES; local d=ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis(); assert(d.mode=='UNAVAILABLE_CAPACITY' and d.outcome=='CAPTURED' and d.spyID==655368 and d.eventTurn==23 and d.occupied==1 and d.capacity==1); assert(d.mission.operation==59 and d.mission.plot==8511 and d.mission.initial==1 and d.mission.escape==-1); assert(d.roster.physicalCount==1 and #d.roster.map==0 and #d.roster.offMap==1 and #d.roster.captured==1 and #d.roster.activeOffMap==0 and d.roster.capturedPlaceholder.offMapIndex==1); assert(d.captive.nameIndex==-1 and d.roster.offMap[1].returnTurn==-1 and d.roster.offMap[1].x==-9999 and d.roster.offMap[1].y==-9999 and #WRITES==before)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('BEGIN_SPY_RETRY'); FRAME()",
        "native retry route unavailable",
    );
}

#[test]
fn captive_sentinel_never_hides_two_real_spies_or_an_unrelated_record() {
    let h = Enemy::new();
    h.failed();
    h.run("CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}; OFF_MAP={{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}; assert(ExposedMembers.JAC_PersonaBoundary.ReadEnemy().spyRoster.physicalCount==2)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis()",
        "unambiguous original operative roster",
    );
    let duplicate = Enemy::new();
    duplicate.failed();
    duplicate.run("UNITS[0]={}; CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}; OFF_MAP={{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999},{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}; local r=ExposedMembers.JAC_PersonaBoundary.ReadEnemy().spyRoster; assert(r.physicalCount==3 and not r.capturedPlaceholder)");
    duplicate.reject(
        "ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis()",
        "unambiguous original operative roster",
    );
    let travel = Enemy::new();
    travel.failed();
    travel.run("UNITS[0]={}; CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}; OFF_MAP={{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999},{Name='Other Spy',Level=1,ReturnTurn=TURN+2,XLocation=CONTROL.x,YLocation=CONTROL.y}}; local r=ExposedMembers.JAC_PersonaBoundary.ReadEnemy().spyRoster; assert(r.physicalCount==2 and r.capturedPlaceholder.offMapIndex==1 and #r.activeOffMap==1 and r.activeOffMap[1].returnTurn==TURN+2)");
    travel.reject(
        "ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis()",
        "unambiguous original operative roster",
    );
    for record in [
        "{Name='Other',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}",
        "{Name='Spy 15',Level=2,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}",
        "{Name='Spy 15',Level=1,ReturnTurn=TURN+2,XLocation=-9999,YLocation=-9999}",
        "{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9998,YLocation=-9999}",
        "{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9998}",
    ] {
        let h = Enemy::new();
        h.failed();
        h.run(&format!(
            "UNITS[0]={{}}; CAPTURED[1]={{{{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}}}; OFF_MAP={{{record}}}; local r=ExposedMembers.JAC_PersonaBoundary.ReadEnemy().spyRoster; assert(r.physicalCount==2 and not r.capturedPlaceholder and #r.activeOffMap==1)"
        ));
        h.reject(
            "ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis()",
            "unambiguous original operative roster",
        );
    }
}

#[test]
fn negative_native_name_index_is_probed_without_sending_or_accepting_a_deal() {
    let h = Enemy::new();
    h.failed();
    h.run(
        r#"
        UNITS[0]={};
        local travel=Game:GetProperty('JAC_BOUNDARY_SPY_TRAVEL');
        travel.name=GameInfo.Units['UNIT_SPY'].Name;
        CAPTURED[1]={{OwningPlayer=0,Name='',Level=1,NameIndex=-1}};
        OFF_MAP={{Name='',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}};
        DealDirection={OUTGOING=2}; DealItemTypes={CAPTIVE=4};
        DEAL_ITEMS={}; NEXT_ITEM_ID=1; ADD_CALLS=0; REMOVED_IDS={};
        ITEM_VALID=false; PENDING_DEAL=false; ITEM_AVAILABLE=true;
        function makeDealItem(id,kind,from,to)
            local item={id=id,kind=kind,from=from,to=to,value=-999};
            item.GetID=function() return item.id end;
            item.GetType=function() return item.kind end;
            item.GetFromPlayerID=function() return item.from end;
            item.GetToPlayerID=function() return item.to end;
            item.GetValueType=function() return item.value end;
            item.IsLocked=function() return false end;
            item.SetValueType=function(_,value)
                if FAIL_SET then error('native value set failed') end;
                assert(value==-1); item.value=value; PROBED_VALUE=value;
            end;
            item.IsValid=function()
                if RACE_DURING_VALID then
                    RACE_DURING_VALID=false;
                    DEAL_ITEMS[#DEAL_ITEMS+1]=makeDealItem(900,99,0,1);
                    DEAL_ITEMS[#DEAL_ITEMS+1]=makeDealItem(902,99,0,1);
                end;
                return ITEM_VALID;
            end;
            item.SetLocked=function() error('probe must not lock a working item') end;
            return item;
        end
        WORKING_DEAL={
            GetItemCount=function() return #DEAL_ITEMS end,
            Items=function()
                local index=0;
                return function() index=index+1; return DEAL_ITEMS[index] end;
            end,
            AddItemOfType=function(_,kind,captor)
                assert(kind==4 and captor==1); ADD_CALLS=ADD_CALLS+1;
                if not ITEM_AVAILABLE then return nil end;
                local item=makeDealItem(NEXT_ITEM_ID,kind,captor,0);
                NEXT_ITEM_ID=NEXT_ITEM_ID+1;
                DEAL_ITEMS[#DEAL_ITEMS+1]=item;
                return item;
            end,
            FindItemByID=function(_,id)
                for _,item in ipairs(DEAL_ITEMS) do if item.id==id then
                    return setmetatable({}, {__index=item});
                end end;
            end,
            RemoveItemByID=function(_,id)
                for index,item in ipairs(DEAL_ITEMS) do if item.id==id then
                    REMOVED_IDS[#REMOVED_IDS+1]=id;
                    table.remove(DEAL_ITEMS,index); return true;
                end end;
                return false;
            end
        };
        READS=0;
        DealManager={
            HasPendingDeal=function(a,b) assert(a==0 and b==1); return PENDING_DEAL end,
            ClearWorkingDeal=function() error('must preserve unsent working deals') end,
            GetWorkingDeal=function(direction,a,b)
                assert(direction==2 and a==0 and b==1);
                READS=READS+1;
                if NO_WORKING_DEAL then return nil end;
                if RACE_BEFORE_ADD and READS==2 then
                    DEAL_ITEMS[#DEAL_ITEMS+1]=makeDealItem(901,99,0,1);
                end;
                return WORKING_DEAL;
            end
        };
        local before=#WRITES;
        local user=makeDealItem(800,99,0,1);
        local second=makeDealItem(801,99,1,0);
        DEAL_ITEMS={user,second};
        local r=ExposedMembers.JAC_PersonaBoundary.ProbeCapturedExchange();
        assert(r.status=='WORKING_DEAL_OCCUPIED' and r.detail==2);
        assert(#DEAL_ITEMS==2 and DEAL_ITEMS[1]==user and DEAL_ITEMS[2]==second
            and ADD_CALLS==0 and #REMOVED_IDS==0);
        DEAL_ITEMS={}; NO_WORKING_DEAL=true;
        r=ExposedMembers.JAC_PersonaBoundary.ProbeCapturedExchange();
        assert(r.status=='WORKING_DEAL_UNAVAILABLE' and ADD_CALLS==0 and #REMOVED_IDS==0);
        NO_WORKING_DEAL=false;
        local items=WORKING_DEAL.Items; WORKING_DEAL.Items=nil;
        r=ExposedMembers.JAC_PersonaBoundary.ProbeCapturedExchange();
        assert(r.status=='WORKING_DEAL_UNREADABLE' and ADD_CALLS==0 and #DEAL_ITEMS==0);
        WORKING_DEAL.Items=items; READS=0; RACE_BEFORE_ADD=true;
        r=ExposedMembers.JAC_PersonaBoundary.ProbeCapturedExchange();
        assert(r.status=='WORKING_DEAL_CHANGED' and #DEAL_ITEMS==1 and DEAL_ITEMS[1].id==901);
        assert(ADD_CALLS==0 and #REMOVED_IDS==0);
        RACE_BEFORE_ADD=false; READS=0; DEAL_ITEMS={};
        local r=ExposedMembers.JAC_PersonaBoundary.ProbeCapturedExchange();
        assert(r.status=='INVALID_ITEM' and r.nameIndex==-1 and PROBED_VALUE==-1);
        assert(#DEAL_ITEMS==0 and #REMOVED_IDS==1 and REMOVED_IDS[1]==1);
        assert(#WRITES==before and #CAPTURED[1]==1 and #OFF_MAP==1);
        ITEM_VALID=true;
        r=ExposedMembers.JAC_PersonaBoundary.ProbeCapturedExchange();
        assert(r.status=='VALID_ITEM' and #DEAL_ITEMS==0 and #REMOVED_IDS==2 and #WRITES==before);
        ITEM_AVAILABLE=false;
        r=ExposedMembers.JAC_PersonaBoundary.ProbeCapturedExchange();
        assert(r.status=='NO_ITEM' and #DEAL_ITEMS==0 and #REMOVED_IDS==2 and #WRITES==before);
        ITEM_AVAILABLE=true; RACE_DURING_VALID=true;
        r=ExposedMembers.JAC_PersonaBoundary.ProbeCapturedExchange();
        assert(r.status=='WORKING_DEAL_CHANGED' and #DEAL_ITEMS==2 and
            DEAL_ITEMS[1].id==900 and DEAL_ITEMS[2].id==902);
        assert(#REMOVED_IDS==3 and REMOVED_IDS[3]==3 and #WRITES==before);
        DEAL_ITEMS={}; FAIL_SET=true;
        local ok,err=pcall(ExposedMembers.JAC_PersonaBoundary.ProbeCapturedExchange);
        assert(not ok and string.find(tostring(err),'native value set failed',1,true));
        assert(#DEAL_ITEMS==0 and #REMOVED_IDS==4 and #WRITES==before);
        FAIL_SET=false;
        PENDING_DEAL=true;
        r=ExposedMembers.JAC_PersonaBoundary.ProbeCapturedExchange();
        assert(r.status=='PENDING_DEAL' and #DEAL_ITEMS==0 and #WRITES==before);
        AT_WAR=true;
        r=ExposedMembers.JAC_PersonaBoundary.ProbeCapturedExchange();
        assert(r.status=='AT_WAR' and #DEAL_ITEMS==0 and #WRITES==before);
    "#,
    );
}

#[test]
fn captive_exchange_requires_native_ai_acceptance_and_actual_returned_spy() {
    let h = Enemy::new();
    h.captured_deal();
    h.run("assert(ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()=='SESSION_REQUESTED'); assert(ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState().stage=='SESSION_UNBOUND'); assert(#OUTGOING==1 and OUTGOING[1].kind==4 and OUTGOING[1].from==1 and OUTGOING[1].to==0 and OUTGOING[1].value==-1 and OUTGOING[1].locked and #SENDS==0 and #CAPTURED[1]==1)");
    h.pin_captive_session();
    h.sync("MARK_CAPTIVE_EXCHANGE", "JAC_BoundaryMarkCaptiveExchange");
    h.run("assert(Game:GetProperty('JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT').eventID==0); assert(ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange()=='PROPOSED_WAIT_AI'); assert(#SENDS==1 and SENDS[1]==DealProposalAction.PROPOSED and #CAPTURED[1]==1); PENDING_DEAL=true; assert(ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState().stage=='AI_ACCEPTED_PENDING'); PENDING_DEAL=false; assert(ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState().stage=='AI_ACCEPTED_EXACT'); CAN_START=false; assert(ExposedMembers.JAC_PersonaBoundary.AcceptCaptiveExchange()=='FINAL_SENT'); assert(#SENDS==2 and SENDS[2]==DealProposalAction.ACCEPTED); OFF_MAP={{Name='',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}; assert(ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState().stage=='RETURN_UNOBSERVED'); OFF_MAP={}; local s=ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState(); assert(s.stage=='RETURNED_MAP_WAIT_NATIVE' and not s.operationReady and s.spyID==15 and s.currentID==16 and s.occupied==1 and #CAPTURED[1]==0); CAN_START=true; s=ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState(); assert(s.stage=='RETURNED_MAP_READY' and s.operationReady); assert(ExposedMembers.JAC_PersonaBoundary.CloseCaptiveExchangeSession()=='RETURNED_MAP_READY' and SESSION_ID==nil); assert(HISTORY[0][0].InitialResult==EspionageResultTypes.FAIL_UNDETECTED and ENEMY_GOV.neutralized==0)");
    h.sync("BEGIN_SPY_RETRY", "JAC_BoundaryBeginEnemySpyRetry");
    h.run("local r=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY'); assert(r.archive.spyID==15 and r.archive.eventID==0 and r.spyID==16 and r.mode=='MAP'); CAN_START=false; assert(ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()=='WAIT_NATIVE_READY'); CAN_START=true; assert(ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()=='HOME_READY' and PROGRESS_CALLS==0)");
}

#[test]
fn unnamed_captive_tooltip_key_requires_the_exact_native_locked_item() {
    let h = Enemy::new();
    h.captured_deal();
    h.run("local request=DiplomacyManager.RequestSession; DiplomacyManager.RequestSession=function(a,b,kind) assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()==GameInfo.Units['UNIT_SPY'].Name); return request(a,b,kind) end");
    h.run("local b=ExposedMembers.JAC_PersonaBoundary; assert(b.CaptiveExchangeSessionReadOnly()=='NONE' and b.MissingCaptiveDealTooltipKey()==nil); assert(b.BeginCaptiveExchange()=='SESSION_REQUESTED'); assert(b.CaptiveExchangeSessionReadOnly()=='0|1|'..OUTGOING[1]:GetID()..'|true|false|nil' and ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()==GameInfo.Units['UNIT_SPY'].Name and OUTGOING[1].value==-1 and #SENDS==0 and #CAPTURED[1]==1)");
    h.run("local item=OUTGOING[1]; item.GetValueTypeNameID=function() return 'LOC_REAL_NAME' end; assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()==nil); item.GetValueTypeNameID=function() return nil end; item.locked=false; assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()==nil); item.locked=true; item.id=item.id+1; assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()==nil); item.id=item.id-1; CAPTURED[1][1].NameIndex=3; assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()==nil); CAPTURED[1][1].NameIndex=-1; assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()==GameInfo.Units['UNIT_SPY'].Name)");
    h.pin_captive_session();
    h.sync("MARK_CAPTIVE_EXCHANGE", "JAC_BoundaryMarkCaptiveExchange");
    h.run("AI_REPLY='REJECT'; assert(ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange()=='PROPOSED_WAIT_AI'); assert(ExposedMembers.JAC_PersonaBoundary.CloseCaptiveExchangeSession()=='AI_REFUSED'); assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()==nil and #CAPTURED[1]==1 and HISTORY[0][0].InitialResult==EspionageResultTypes.FAIL_UNDETECTED)");
}

#[test]
fn observed_captive_session_must_bind_the_prepared_item_before_any_proposal() {
    let h = Enemy::new();
    h.captured_deal();
    h.run("local b=ExposedMembers.JAC_PersonaBoundary; assert(b.BeginCaptiveExchange()=='SESSION_REQUESTED'); assert(b.CaptiveExchangeState().stage=='SESSION_UNBOUND' and b.CaptiveExchangeSessionReadOnly()=='0|1|'..OUTGOING[1]:GetID()..'|true|false|nil')");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange()",
        "no observed ownership binding",
    );
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('MARK_CAPTIVE_EXCHANGE')",
        "no observed ownership binding",
    );
    h.run("assert(#SENDS==0 and Game:GetProperty('JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT')==nil and #CAPTURED[1]==1)");
    for value in ["nil", "true", "'44'", "-1", "44.5"] {
        h.reject(
            &format!(
                "ExposedMembers.JAC_PersonaBoundary.PinObservedCaptiveExchangeSession({value})"
            ),
            "not an exact integer",
        );
    }
    h.run("SESSION_ID=45");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.PinObservedCaptiveExchangeSession(44)",
        "changed before binding",
    );
    h.run("assert(ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState().stage=='SESSION_UNBOUND' and #SENDS==0 and #CAPTURED[1]==1)");
}

#[test]
fn captive_session_binding_rechecks_native_item_captive_and_initiator() {
    for mutation in [
        "OUTGOING[1].locked=false",
        "OUTGOING[1].value=7",
        "OUTGOING[1].id=OUTGOING[1].id+1",
        "CAPTURED[1][1].NameIndex=7",
        "DiplomacyManager.GetSessionInfo=function() return {FromPlayer=1} end",
    ] {
        let h = Enemy::new();
        h.captured_deal();
        h.run("assert(ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()=='SESSION_REQUESTED')");
        h.run(mutation);
        h.reject(
            "ExposedMembers.JAC_PersonaBoundary.PinObservedCaptiveExchangeSession(44)",
            if mutation.contains("GetSessionInfo") {
                "initiator changed"
            } else if mutation.contains("CAPTURED") {
                "captured operative differs"
            } else {
                "prepared captive"
            },
        );
        h.run("assert(ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState().stage=='SESSION_UNBOUND' and #SENDS==0 and Game:GetProperty('JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT')==nil)");
    }
}

#[test]
fn replaced_native_session_after_explicit_binding_cannot_propose_or_close() {
    let h = Enemy::new();
    h.captured_deal();
    h.run("assert(ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()=='SESSION_REQUESTED')");
    h.pin_captive_session();
    h.run("CLOSED={}; DiplomacyManager.CloseSession=function(id) CLOSED[#CLOSED+1]=id end; SESSION_ID=45");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange()",
        "session identity changed",
    );
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.CloseCaptiveExchangeSession()",
        "session identity changed",
    );
    h.run("assert(#CLOSED==0 and #SENDS==0 and Game:GetProperty('JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT')==nil and #CAPTURED[1]==1)");

    let accepted = Enemy::new();
    accepted.captured_deal();
    accepted.run(
        "assert(ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()=='SESSION_REQUESTED')",
    );
    accepted.pin_captive_session();
    accepted.sync("MARK_CAPTIVE_EXCHANGE", "JAC_BoundaryMarkCaptiveExchange");
    accepted.run("assert(ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange()=='PROPOSED_WAIT_AI' and #SENDS==1); SESSION_ID=45");
    accepted.reject(
        "ExposedMembers.JAC_PersonaBoundary.AcceptCaptiveExchange()",
        "session identity changed",
    );
    accepted.run("assert(#SENDS==1 and #CAPTURED[1]==1 and HISTORY[0][0].InitialResult==EspionageResultTypes.FAIL_UNDETECTED)");
}

#[test]
fn captive_exchange_never_closes_a_replacement_or_foreign_initiated_session() {
    let h = Enemy::new();
    h.captured_deal();
    h.run("AI_REPLY='REJECT'; local b=ExposedMembers.JAC_PersonaBoundary; b.BeginCaptiveExchange(); assert(b.CaptiveExchangeState().stage=='SESSION_UNBOUND')");
    h.pin_captive_session();
    h.sync("MARK_CAPTIVE_EXCHANGE", "JAC_BoundaryMarkCaptiveExchange");
    h.run("ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange(); CLOSED={}; DiplomacyManager.CloseSession=function(id) CLOSED[#CLOSED+1]=id end; SESSION_ID=45; DiplomacyManager.GetSessionInfo=function() return {FromPlayer=0} end");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.CloseCaptiveExchangeSession()",
        "session identity changed",
    );
    h.run("assert(#CLOSED==0); SESSION_ID=44; DiplomacyManager.GetSessionInfo=function() return {FromPlayer=1} end");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.CloseCaptiveExchangeSession()",
        "not initiated by the attacker",
    );
    h.run("assert(#CLOSED==0)");
}

#[test]
fn returned_map_captive_requires_a_physical_plot_before_terminal_classification() {
    let h = Enemy::new();
    h.captured_deal();
    h.run("ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()");
    h.pin_captive_session();
    h.sync("MARK_CAPTIVE_EXCHANGE", "JAC_BoundaryMarkCaptiveExchange");
    h.run("local b=ExposedMembers.JAC_PersonaBoundary; b.ProposeCaptiveExchange(); b.AcceptCaptiveExchange(); UNITS[0][1].x=-9999");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState()",
        "no real plot",
    );
    h.run("UNITS[0][1].x=9998; UNITS[0][1].y=9998");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState()",
        "no real plot",
    );
    h.run("UNITS[0][1].x=CONTROL.x; UNITS[0][1].y=CONTROL.y; assert(ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState().stage=='RETURNED_MAP_READY')");
}

#[test]
fn accepted_captive_exchange_can_follow_a_real_off_map_return_without_new_production() {
    let h = Enemy::new();
    h.captured_deal();
    h.run("RETURN_OFFMAP=true; assert(ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()=='SESSION_REQUESTED')");
    h.pin_captive_session();
    h.sync("MARK_CAPTIVE_EXCHANGE", "JAC_BoundaryMarkCaptiveExchange");
    h.run("assert(ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange()=='PROPOSED_WAIT_AI'); assert(ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState().stage=='AI_ACCEPTED_EXACT'); assert(ExposedMembers.JAC_PersonaBoundary.AcceptCaptiveExchange()=='FINAL_SENT'); local s=ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState(); assert(s.stage=='RETURNED_OFF_MAP' and not s.operationReady and s.spyID==15 and s.occupied==1 and #CAPTURED[1]==0 and #OFF_MAP==1 and OFF_MAP[1].ReturnTurn==TURN+2)");
    h.sync("BEGIN_SPY_RETRY", "JAC_BoundaryBeginEnemySpyRetry");
    h.run("local r=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY'); assert(r.mode=='RETURN' and r.archive.eventID==0 and r.archive.spyID==15 and r.returnTurn==TURN+2); assert(ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()=='OFF_MAP_RETURN'); ARRIVE_OFFMAP(16); assert(ExposedMembers.JAC_PersonaBoundary.RetryLivingSpy()=='ADOPTION_REQUIRED' and PROGRESS_CALLS==0)");
}

#[test]
fn returned_captive_requires_the_native_spy_owner_type_future_turn_and_real_coordinates() {
    let map = Enemy::new();
    map.captured_deal();
    map.run(
        "assert(ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()=='SESSION_REQUESTED')",
    );
    map.pin_captive_session();
    map.sync("MARK_CAPTIVE_EXCHANGE", "JAC_BoundaryMarkCaptiveExchange");
    map.run("ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange(); ExposedMembers.JAC_PersonaBoundary.AcceptCaptiveExchange(); UNITS[0][1].owner=1");
    map.reject(
        "ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState()",
        "living map spy does not match",
    );
    map.run("UNITS[0][1].owner=0; GameInfo.Units[56]={Index=56,Spy=true}; UNITS[0][1].GetType=function() return 56 end");
    map.reject(
        "ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState()",
        "living map spy does not match",
    );

    let off_map = Enemy::new();
    off_map.captured_deal();
    off_map.run("RETURN_OFFMAP=true; ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()");
    off_map.pin_captive_session();
    off_map.sync("MARK_CAPTIVE_EXCHANGE", "JAC_BoundaryMarkCaptiveExchange");
    off_map.run("ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange(); ExposedMembers.JAC_PersonaBoundary.AcceptCaptiveExchange(); OFF_MAP[1].ReturnTurn=TURN");
    off_map.reject(
        "ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState()",
        "off-map native spy differs",
    );
    off_map.run("OFF_MAP[1].ReturnTurn=TURN+2; OFF_MAP[1].XLocation=-9999");
    off_map.reject(
        "ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState()",
        "off-map native spy differs",
    );
    off_map.run("OFF_MAP[1].XLocation=CONTROL.x; OFF_MAP[1].YLocation=-9999");
    off_map.reject(
        "ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState()",
        "off-map native spy differs",
    );
}

#[test]
fn captive_exchange_refusal_or_changed_terms_never_releases_or_accepts_a_spy() {
    for (reply, expected) in [
        ("REJECT", "AI_REFUSED"),
        ("COUNTER", "AI_CHANGED_TERMS"),
        ("UNKNOWN", "AI_RESPONSE_UNCLASSIFIED"),
    ] {
        let h = Enemy::new();
        h.captured_deal();
        h.run(&format!("AI_REPLY='{reply}'; assert(ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()=='SESSION_REQUESTED')"));
        h.pin_captive_session();
        h.sync("MARK_CAPTIVE_EXCHANGE", "JAC_BoundaryMarkCaptiveExchange");
        h.run(&format!("assert(ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange()=='PROPOSED_WAIT_AI'); assert(ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState().stage=='{expected}'); assert(#SENDS==1 and SENDS[1]==DealProposalAction.PROPOSED and #CAPTURED[1]==1 and ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis().mode=='UNAVAILABLE_CAPACITY'); assert(ExposedMembers.JAC_PersonaBoundary.CloseCaptiveExchangeSession()=='{expected}' and SESSION_ID==nil)"));
        h.reject(
            "ExposedMembers.JAC_PersonaBoundary.AcceptCaptiveExchange()",
            "native AI did not accept",
        );
        h.reject(
            "ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()",
            "already attempted",
        );
        exec(&h.0, &read_repo(VIEW));
        h.reject(
            "ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()",
            "already attempted",
        );
    }
}

#[test]
fn captive_exchange_preserves_an_existing_unsent_deal_and_native_invalid_terms() {
    let h = Enemy::new();
    h.captured_deal();
    h.run("local own=makeDealItem(99,0,1,50,false); OUTGOING[1]=own; EXISTING=own");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()",
        "existing unsent native deal must be preserved",
    );
    h.run("assert(#OUTGOING==1 and OUTGOING[1]==EXISTING and SESSION_ID==nil and #SENDS==0 and #CAPTURED[1]==1)");

    let invalid = Enemy::new();
    invalid.captured_deal();
    invalid.run(
        "assert(ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()=='SESSION_REQUESTED')",
    );
    invalid.pin_captive_session();
    invalid.sync("MARK_CAPTIVE_EXCHANGE", "JAC_BoundaryMarkCaptiveExchange");
    invalid.run("DEAL_VALIDATION_RESULT=DealValidationResult.INVALID; assert(DealValidationResult.INVALID~=0); assert(ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange()=='DEAL_INVALID'); assert(ExposedMembers.JAC_PersonaBoundary.CaptiveExchangeState().stage=='DEAL_INVALID' and #OUTGOING==0 and #SENDS==0 and #CAPTURED[1]==1)");

    for (value, expected) in [
        ("true", "DEAL_VALIDATION_UNKNOWN"),
        ("nil", "DEAL_VALIDATION_UNKNOWN"),
        ("'VALID'", "DEAL_VALIDATION_UNKNOWN"),
        ("99", "DEAL_INVALID"),
    ] {
        let unknown = Enemy::new();
        unknown.captured_deal();
        unknown.run("assert(ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()=='SESSION_REQUESTED')");
        unknown.pin_captive_session();
        unknown.sync("MARK_CAPTIVE_EXCHANGE", "JAC_BoundaryMarkCaptiveExchange");
        unknown.run(&format!("DEAL_VALIDATION_RESULT={value}; local stage=ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange(); assert(stage=='{expected}' and #SENDS==0 and #OUTGOING==0 and #CAPTURED[1]==1)"));
    }
    let no_enum = Enemy::new();
    no_enum.captured_deal();
    no_enum.run(
        "assert(ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()=='SESSION_REQUESTED')",
    );
    no_enum.pin_captive_session();
    no_enum.sync("MARK_CAPTIVE_EXCHANGE", "JAC_BoundaryMarkCaptiveExchange");
    no_enum.run("DealValidationResult=nil; assert(ExposedMembers.JAC_PersonaBoundary.ProposeCaptiveExchange()=='DEAL_VALIDATION_UNKNOWN' and #SENDS==0 and #OUTGOING==0)");

    let unavailable = Enemy::new();
    unavailable.captured_deal();
    unavailable
        .run("DiplomacyManager.RequestSession=function() error('native session unavailable') end");
    unavailable.reject(
        "ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()",
        "native session unavailable",
    );
    unavailable.run("assert(#OUTGOING==0 and #SENDS==0 and #CAPTURED[1]==1 and Game:GetProperty('JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT')==nil and ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()==nil)");

    let setter = Enemy::new();
    setter.captured_deal();
    setter.run("FAIL_SET_VALUE=true");
    setter.reject(
        "ExposedMembers.JAC_PersonaBoundary.BeginCaptiveExchange()",
        "native captive value setter unavailable",
    );
    setter.run("assert(#OUTGOING==0 and #SENDS==0 and #CAPTURED[1]==1 and SESSION_ID==nil)");
}

#[test]
fn a_real_second_capacity_can_produce_while_the_original_captive_remains() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; SPY_CAPACITY=2; CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}; OFF_MAP={{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}; local d=ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis(); assert(d.mode=='PRODUCE' and d.outcome=='CAPTURED' and d.occupied==1 and d.capacity==2)");
    h.sync("BEGIN_SPY_RETRY", "JAC_BoundaryBeginEnemySpyRetry");
    h.run("PRODUCED_SPY_ID=16; ExposedMembers.JAC_PersonaBoundary.Queue('SPAWN_SPY'); FRAME(); ENEMY_CASES.fireCore('JAC_BoundarySpawnSpy'); ExposedMembers.JAC_PersonaBoundary.SpyPrerequisitesReady(); ExposedMembers.JAC_PersonaBoundary.Queue('BUILD_SPY'); FRAME(); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='READY_BUILD_SPY'); ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_SPY'); FRAME(); ENEMY_CASES.fireCore('JAC_BoundaryProgressSpy'); ADVANCE_TURN(); assert(ExposedMembers.JAC_PersonaBoundary.RetryProducedSpyVisible()); assert(#CAPTURED[1]==1 and #OFF_MAP==1 and PROGRESS_CALLS==1)");
    h.run("assert(ExposedMembers.JAC_PersonaBoundary.TryBindRetrySpy()=='BIND_QUEUED'); FRAME(); ENEMY_CASES.fireCore('JAC_BoundaryBindSpy'); assert(Game:GetProperty('JAC_BOUNDARY_SPY').id==16); REQUEST_EFFECT='travel'; ExposedMembers.JAC_PersonaBoundary.Queue('SPY_TRAVEL',{owner=1,city=20}); FRAME(); START_NATIVE(0,16,991); FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundarySpyBarrier'); ENEMY_CASES.fireCore('JAC_BoundarySpyBarrier',r.params); fire(Events.GameCoreEventPublishComplete); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='ACCEPTED_SPY_TRAVEL'); local roster=ExposedMembers.JAC_PersonaBoundary.ReadEnemy().spyRoster; assert(roster.physicalCount==2 and #roster.offMap==2 and #roster.activeOffMap==1 and roster.capturedPlaceholder.offMapIndex==1 and roster.activeOffMap[1].returnTurn>TURN); ExposedMembers.JAC_PersonaBoundary.Queue('SAVE_TRAVEL'); FRAME(); ENEMY_CASES.fireCore('JAC_BoundarySaveSpyTravel',REQUESTS[#REQUESTS].params); assert(ExposedMembers.JAC_PersonaBoundary.EnemyProgress()=='TRAVEL'); ARRIVE_OFFMAP(17); assert(#OFF_MAP==1 and OFF_MAP[1].ReturnTurn==-1); roster=ExposedMembers.JAC_PersonaBoundary.ReadEnemy().spyRoster; assert(roster.physicalCount==2 and #roster.map==1 and #roster.activeOffMap==0 and #roster.captured==1); assert(ExposedMembers.JAC_PersonaBoundary.TryAdoptEnemySpy()=='ADOPT_QUEUED')");
}

#[test]
fn government_plaza_capacity_uses_native_build_then_core_progress_then_real_turn_observation() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}; OFF_MAP={{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}; local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='NEEDS_DOMAIN_LAW' and p.capacity==1 and p.occupied==1 and not p.districtCivicOwned); GOV_CIVIC_COMPLETED=true; local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='BUILD_DISTRICT' and p.cityID==10 and p.plot==11); GOV_DISTRICT_BUILT=true; GOV_DISTRICT_PLACED=true; local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='BUILD_TIER1' and p.buildingType=='BUILDING_GOV_TALL' and p.cityID==10)");
    h.run("BUILD_HEAD={BuildingType=76}; assert(ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan().stage=='BUILD_QUEUED')");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_CAPACITY')",
        "BUILD acceptance is missing",
    );
    h.run("BUILD_HEAD=nil");
    h.run("ExposedMembers.JAC_PersonaBoundary.Queue('BUILD_CAPACITY'); FRAME(); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='READY_BUILD_CAPACITY' and BUILD_REQUESTS[#BUILD_REQUESTS].params.building==760 and BUILD_REQUESTS[#BUILD_REQUESTS].selected==CONTROL); assert(ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan().stage=='BUILD_QUEUED' and CAPACITY_PROGRESS_CALLS==0)");
    h.sync("PROGRESS_CAPACITY", "JAC_BoundaryProgressCapacityBuilding");
    h.run("assert(CAPACITY_PROGRESS_CALLS==1 and SPY_CAPACITY==1 and not GOV_TIER1_BUILT and #CAPTURED[1]==1 and Game:GetProperty('JAC_BOUNDARY_CAPACITY_PROGRESS').steps.BUILDING_GOV_TALL.cityID==10); local o=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityOutcome(); assert(o.stage=='PROGRESS_PENDING' and not o.targetBuilt and o.buildingType=='BUILDING_GOV_TALL')");
    h.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryProgressCapacityBuilding',REQUESTS[#REQUESTS].params)",
        "already added once",
    );
    h.run("CAPACITY_COMPLETES=false; ADVANCE_TURN(); assert(ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityOutcome().stage=='PROGRESS_PENDING'); CAPACITY_COMPLETES=true; ADVANCE_TURN(); assert(GOV_TIER1_BUILT and not GOV_SPIES_BUILT and SPY_CAPACITY==1); local o=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityOutcome(); assert(o.stage=='TIER1_BUILT' and o.targetBuilt and o.capacity==1 and o.turn>o.progressTurn); local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='BUILD_SPIES' and p.buildingType=='BUILDING_GOV_SPIES')");
    h.run("ExposedMembers.JAC_PersonaBoundary.Queue('BUILD_CAPACITY'); FRAME(); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='READY_BUILD_CAPACITY' and BUILD_REQUESTS[#BUILD_REQUESTS].params.building==790); assert(ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan().stage=='BUILD_QUEUED')");
    h.sync("PROGRESS_CAPACITY", "JAC_BoundaryProgressCapacityBuilding");
    h.run("assert(CAPACITY_PROGRESS_CALLS==2 and SPY_CAPACITY==1 and not GOV_SPIES_BUILT and #CAPTURED[1]==1); assert(ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityOutcome().stage=='PROGRESS_PENDING'); ADVANCE_TURN(); assert(GOV_SPIES_BUILT and SPY_CAPACITY==2); SPY_CAPACITY=1; assert(ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityOutcome().stage=='EFFECT_PENDING'); SPY_CAPACITY=2; local o=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityOutcome(); assert(o.stage=='CAPACITY_READY' and o.targetBuilt and o.capacity==2 and o.captured==1 and o.turn>o.progressTurn); local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='CAPACITY_READY' and p.capacity==2 and p.occupied==1 and #CAPTURED[1]==1)");
    h.sync("BEGIN_SPY_RETRY", "JAC_BoundaryBeginEnemySpyRetry");
    h.run("local r=Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY'); assert(r.mode=='PRODUCE' and r.archive.capacity==2 and r.archive.occupied==1 and r.archive.captiveNameIndex==7 and r.archive.eventID==0 and r.archive.spyID==15); assert(HISTORY[0][0].InitialResult==EspionageResultTypes.FAIL_UNDETECTED and #CAPTURED[1]==1)");
}

#[test]
fn capacity_progress_rechecks_the_ui_native_queue_before_core_add_progress() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}; OFF_MAP={{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}; GOV_CIVIC_COMPLETED=true; GOV_DISTRICT_BUILT=true; ExposedMembers.JAC_PersonaBoundary.Queue('BUILD_CAPACITY'); FRAME(); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='READY_BUILD_CAPACITY'); ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_CAPACITY'); FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryProgressCapacityBuilding'); BUILD_HEAD={UnitType=55}; assert(ExposedMembers.JAC_PersonaBoundary.ReadCapacityQueue(0,10).buildingType=='')");
    h.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryProgressCapacityBuilding',REQUESTS[#REQUESTS].params)",
        "queue changed before progress",
    );
    h.run("assert(CAPACITY_PROGRESS_CALLS==0 and Game:GetProperty('JAC_BOUNDARY_CAPACITY_PROGRESS')==nil and #CAPTURED[1]==1)");
}

#[test]
fn official_domain_law_and_owned_plot_build_the_government_plaza_before_tier_one() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}; OFF_MAP={{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}; assert(ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan().stage=='NEEDS_DOMAIN_LAW')");
    h.sync("PREPARE_DISTRICT_CIVIC", "JAC_BoundaryPrepareCapacityCivic");
    h.run("local civic=Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC'); assert(GOV_CIVIC_COMPLETED and civic.index==13 and civic.beforeSpent==1 and civic.afterSpent==1); local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='BUILD_DISTRICT' and p.cityID==10 and p.plot==11 and p.x==1 and p.y==1)");
    h.run("ExposedMembers.JAC_PersonaBoundary.Queue('BUILD_CAPACITY_DISTRICT'); FRAME(); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='READY_BUILD_CAPACITY_DISTRICT',ExposedMembers.JAC_PersonaBoundary.lastAction)");
    h.run("assert(BUILD_REQUESTS[#BUILD_REQUESTS].params.district==700 and BUILD_REQUESTS[#BUILD_REQUESTS].params.x==1 and BUILD_REQUESTS[#BUILD_REQUESTS].params.y==1 and BUILD_HEAD.DistrictType==70,'official district request differs')");
    h.run("local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='DISTRICT_QUEUED',p.stage)");
    h.sync(
        "PROGRESS_CAPACITY_DISTRICT",
        "JAC_BoundaryProgressCapacityDistrict",
    );
    h.run("local r=Game:GetProperty('JAC_BOUNDARY_CAPACITY_DISTRICT_PROGRESS'); assert(r.plot==11 and r.cityID==10 and r.cityOwner==0 and r.districtIndex==70 and r.districtID==991 and r.memberKey==1 and r.memberKey~=r.districtID and r.acceptedNonce>0 and CAPACITY_PROGRESS_CALLS==1 and not GOV_DISTRICT_BUILT); assert(ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityDistrictOutcome().stage=='PROGRESS_PENDING'); ADVANCE_TURN(); local o=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityDistrictOutcome(); assert(o.stage=='DISTRICT_BUILT' and o.targetBuilt and o.owner==0 and o.cityOwner==0 and o.cityID==10 and o.districtID==r.districtID and o.plot==11 and o.memberKey==r.memberKey and o.acceptedNonce==r.acceptedNonce and o.turn>o.progressTurn and o.captured==1 and o.capacity==1); assert(ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan().stage=='BUILD_TIER1')");
    h.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryProgressCapacityDistrict',REQUESTS[#REQUESTS].params)",
        "stale",
    );
}

#[test]
fn capacity_civic_old_saved_barrier_cannot_acknowledge_a_missing_core_callback() {
    let h = Enemy::new();
    h.captive_capacity();
    h.run("Game:SetProperty('JAC_BOUNDARY_ENEMY_FIXTURE_BARRIER',{action='PREPARE_DISTRICT_CIVIC',token=100,turn=TURN,attacker=0,phase='COMMAND',serial=7}); Game:SetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_READ',{owner=0,token=100,generation=1,serial=1,owned=true,status='OBSERVED'}); ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_DISTRICT_CIVIC'); FRAME(); assert(REQUESTS[#REQUESTS].params.Token==101,'new token must exceed saved markers'); fire(Events.GameCoreEventPublishComplete); FRAME(); assert(REQUESTS[#REQUESTS].params.Phase=='COMMAND','first new barrier must be sent'); local before=#REQUESTS; fire(Events.GameCoreEventPublishComplete); FRAME(); assert(#REQUESTS==before and ExposedMembers.JAC_PersonaBoundary.lastAction=='BARRIER_SENT_PREPARE_DISTRICT_CIVIC','old marker must not publish new barrier')");
    h.run("local b=REQUESTS[#REQUESTS]; ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME(); assert(REQUESTS[#REQUESTS].params.OnStart=='JAC_BoundaryReadCapacityCivic','new barrier must permit diagnostic read'); ENEMY_CASES.fireCore('JAC_BoundaryReadCapacityCivic',REQUESTS[#REQUESTS].params); FRAME(); b=REQUESTS[#REQUESTS]; ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='UNKNOWN_PREPARE_DISTRICT_CIVIC' and not GOV_CIVIC_COMPLETED and Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC')==nil,'missing callback must remain unknown')");
    h.reject(
        scene_lua(&capacity_civic_scene(), 3),
        "UNKNOWN_PREPARE_DISTRICT_CIVIC",
    );
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_DISTRICT_CIVIC')",
        "do not replay the setter",
    );
}

#[test]
fn capacity_civic_saved_native_marker_prevents_another_setter_request() {
    let h = Enemy::new();
    h.captive_capacity();
    h.run("Game:SetProperty('JAC_BOUNDARY_CAPACITY_CIVIC',{owner=0,index=13,token=1}); BEFORE_REQUESTS=#REQUESTS");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_DISTRICT_CIVIC')",
        "do not replay the setter",
    );
    h.run("assert(#REQUESTS==BEFORE_REQUESTS and not GOV_CIVIC_COMPLETED)");
}

#[test]
fn capacity_civic_setter_return_without_native_ownership_stays_pending() {
    let h = Enemy::new();
    h.captive_capacity();
    h.run("local original=Players[0].GetCulture; Players[0].GetCulture=function() local culture=original(); culture.SetCivic=function(_,index,grant) assert(index==13 and grant); SETTER_CALLS=(SETTER_CALLS or 0)+1 end; return culture end");
    h.civic_roundtrip(true);
    h.run("local c=assert(Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_RECEIPT')); local r=assert(Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_READ')); assert(c.phase=='SETTER_RETURNED' and c.status=='PENDING_NATIVE' and r.status=='PENDING_NATIVE' and not r.owned and SETTER_CALLS==1); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='PENDING_PREPARE_DISTRICT_CIVIC' and Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC')==nil and not GOV_CIVIC_COMPLETED)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_DISTRICT_CIVIC')",
        "do not replay the setter",
    );
}

#[test]
fn capacity_civic_scene_uses_one_real_turn_and_read_only_resume_before_save() {
    let scene = capacity_civic_scene();
    let h = Enemy::new();
    h.captive_capacity();
    h.run("local original=Players[0].GetCulture; Players[0].GetCulture=function() local culture=original(); culture.SetCivic=function(_,index,grant) assert(index==13 and grant); SETTER_CALLS=(SETTER_CALLS or 0)+1 end; return culture end");
    h.run(scene_lua(&scene, 2));
    h.civic_complete_queued(true);
    assert!(h
        .0
        .load(scene_lua(&scene, 3))
        .strict_eval::<bool>()
        .unwrap());
    h.run("local a=Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_ATTEMPT'); assert(a and a.owner==0 and a.index==13 and a.token==SETTER_PARAMS.Token and a.turn==TURN and a.beforeSpent==1 and a.sequence==a.token and Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC')==nil); ADVANCE_TURN(); GOV_CIVIC_COMPLETED=true");
    assert_eq!(scene["steps"][4]["action"], "end_turn");
    assert_eq!(scene["steps"][4]["turns"], 1);
    assert!(h
        .0
        .load(scene_lua(&scene, 5))
        .strict_eval::<bool>()
        .unwrap());
    h.run(scene_lua(&scene, 6));
    h.civic_resume_complete_queued();
    assert!(h
        .0
        .load(scene_lua(&scene, 7))
        .strict_eval::<bool>()
        .unwrap());
    h.run(scene_lua(&scene, 8));
    h.run(scene_lua(&scene, 9));
    assert_eq!(scene["steps"][10]["action"], "save");
    h.run("local a=Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_ATTEMPT'); local r=Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_READ'); assert(SETTER_CALLS==1 and a.turn==TURN-1 and r.turn==TURN and r.action=='RESUME_CAPACITY_CIVIC_READ' and r.frame==RESUME_READ_PARAMS.Frame); ENEMY_CASES.fireCore('JAC_BoundaryPrepareCapacityCivic',SETTER_PARAMS); assert(SETTER_CALLS==1)");
}

#[test]
fn capacity_civic_scene_immediate_ownership_still_uses_the_fresh_turn_read() {
    let scene = capacity_civic_scene();
    let h = Enemy::new();
    h.captive_capacity();
    h.run(scene_lua(&scene, 2));
    h.civic_complete_queued(true);
    h.run(
        "assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='ACCEPTED_PREPARE_DISTRICT_CIVIC')",
    );
    assert!(h
        .0
        .load(scene_lua(&scene, 3))
        .strict_eval::<bool>()
        .unwrap());
    h.run("ADVANCE_TURN()");
    assert!(h
        .0
        .load(scene_lua(&scene, 5))
        .strict_eval::<bool>()
        .unwrap());
    h.run(scene_lua(&scene, 6));
    h.civic_resume_complete_queued();
    assert!(h
        .0
        .load(scene_lua(&scene, 7))
        .strict_eval::<bool>()
        .unwrap());
    h.run(scene_lua(&scene, 8));
    h.run(scene_lua(&scene, 9));
    h.run("local a=Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_ATTEMPT'); local r=Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_READ'); assert(a.turn==TURN-1 and r.action=='RESUME_CAPACITY_CIVIC_READ' and r.turn==TURN and r.token~=a.token)");
}

#[test]
fn capacity_civic_scene_fails_before_save_if_still_pending_after_the_real_turn() {
    let scene = capacity_civic_scene();
    let h = Enemy::new();
    h.captive_capacity();
    h.run("local original=Players[0].GetCulture; Players[0].GetCulture=function() local culture=original(); culture.SetCivic=function(_,index,grant) assert(index==13 and grant); SETTER_CALLS=(SETTER_CALLS or 0)+1 end; return culture end");
    h.run(scene_lua(&scene, 2));
    h.civic_complete_queued(true);
    assert!(h
        .0
        .load(scene_lua(&scene, 3))
        .strict_eval::<bool>()
        .unwrap());
    h.run("ADVANCE_TURN()");
    assert!(h
        .0
        .load(scene_lua(&scene, 5))
        .strict_eval::<bool>()
        .unwrap());
    h.run(scene_lua(&scene, 6));
    h.civic_resume_complete_queued();
    h.reject(scene_lua(&scene, 7), "PENDING_RESUME_CAPACITY_CIVIC_READ");
    h.run("assert(SETTER_CALLS==1 and Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC')==nil and not GOV_CIVIC_COMPLETED)");
    assert!(h.0.load(scene_lua(&scene, 8)).exec().is_err());
}

#[test]
fn capacity_civic_old_turn_read_and_changed_original_reference_are_rejected() {
    let old = Enemy::new();
    old.captive_capacity();
    old.civic_roundtrip(true);
    old.run("ADVANCE_TURN(); ENEMY_CASES.fireCore('JAC_BoundaryReadCapacityCivic',FIRST_READ_PARAMS); local r=Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_READ'); assert(r.status=='ERROR' and r.error=='READ_FAILED' and r.turn==TURN and r.originalTurn==TURN-1)");

    let changed = Enemy::new();
    changed.captive_capacity();
    changed.run("local original=Players[0].GetCulture; Players[0].GetCulture=function() local culture=original(); culture.SetCivic=function(_,index,grant) SETTER_CALLS=(SETTER_CALLS or 0)+1 end; return culture end");
    changed.civic_roundtrip(true);
    changed.run("ADVANCE_TURN(); GOV_CIVIC_COMPLETED=true; ExposedMembers.JAC_PersonaBoundary.Queue('RESUME_CAPACITY_CIVIC_READ'); FRAME(); local original=REQUESTS[#REQUESTS].params; assert(original.OnStart=='JAC_BoundaryReadCapacityCivic'); local wrong={}; for key,value in pairs(original) do wrong[key]=value end; wrong.OriginalToken=wrong.OriginalToken+1; ENEMY_CASES.fireCore('JAC_BoundaryReadCapacityCivic',wrong); local r=Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_READ'); assert(r.status=='ERROR' and Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC')==nil); FRAME(); local b=REQUESTS[#REQUESTS]; ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='ERROR_RESUME_CAPACITY_CIVIC_READ' and SETTER_CALLS==1)");
}

#[test]
fn capacity_civic_real_turn_read_rejects_changed_official_title_spend() {
    let scene = capacity_civic_scene();
    let h = Enemy::new();
    h.captive_capacity();
    h.run("local original=Players[0].GetCulture; Players[0].GetCulture=function() local culture=original(); culture.SetCivic=function(_,index,grant) SETTER_CALLS=(SETTER_CALLS or 0)+1 end; return culture end");
    h.civic_roundtrip(true);
    h.run("ADVANCE_TURN(); GOV_CIVIC_COMPLETED=true; local original=Players[0].GetGovernors; Players[0].GetGovernors=function() local governors=original(); governors.GetGovernorPointsSpent=function() return 2 end; return governors end; ExposedMembers.JAC_PersonaBoundary.Queue('RESUME_CAPACITY_CIVIC_READ')");
    h.civic_resume_complete_queued();
    h.reject(scene_lua(&scene, 7), "REJECTED_RESUME_CAPACITY_CIVIC_READ");
    h.run("assert(SETTER_CALLS==1 and Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC')==nil)");
}

#[test]
fn capacity_civic_core_ownership_with_stale_ui_cache_cannot_be_accepted() {
    let h = Enemy::new();
    h.captive_capacity();
    h.run("ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_DISTRICT_CIVIC'); FRAME(); ENEMY_CASES.fireCore('JAC_BoundaryPrepareCapacityCivic',REQUESTS[#REQUESTS].params); FRAME(); local b=REQUESTS[#REQUESTS]; ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME(); local r=REQUESTS[#REQUESTS]; ENEMY_CASES.fireCore('JAC_BoundaryReadCapacityCivic',r.params); assert(Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_READ').owned and Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC')); FRAME(); b=REQUESTS[#REQUESTS]; ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); local original=Players[0].GetCulture; Players[0].GetCulture=function() local culture=original(); culture.HasCivic=function(_,index) if index==13 then return false end; return true end; return culture end; fire(Events.GameCoreEventPublishComplete); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='PENDING_PREPARE_DISTRICT_CIVIC' and GOV_CIVIC_COMPLETED)");
}

#[test]
fn capacity_civic_stale_ui_cache_can_be_rechecked_without_setter_after_a_turn() {
    let h = Enemy::new();
    h.captive_capacity();
    h.run("ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_DISTRICT_CIVIC'); FRAME(); SETTER_PARAMS=REQUESTS[#REQUESTS].params; ENEMY_CASES.fireCore('JAC_BoundaryPrepareCapacityCivic',SETTER_PARAMS); FRAME(); local b=REQUESTS[#REQUESTS]; ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME(); ENEMY_CASES.fireCore('JAC_BoundaryReadCapacityCivic',REQUESTS[#REQUESTS].params); FRAME(); b=REQUESTS[#REQUESTS]; ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); ORIGINAL_HUMAN_CULTURE=Players[0].GetCulture; Players[0].GetCulture=function() local culture=ORIGINAL_HUMAN_CULTURE(); culture.HasCivic=function(_,index) if index==13 then return false end; return true end; return culture end; fire(Events.GameCoreEventPublishComplete); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='PENDING_PREPARE_DISTRICT_CIVIC' and Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_READ').owned)");
    h.run("ADVANCE_TURN(); Players[0].GetCulture=ORIGINAL_HUMAN_CULTURE; ExposedMembers.JAC_PersonaBoundary.Queue('RESUME_CAPACITY_CIVIC_READ')");
    h.civic_resume_complete_queued();
    h.run("assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='ACCEPTED_RESUME_CAPACITY_CIVIC_READ' and GOV_CIVIC_COMPLETED); local r=Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC'); assert(r.commandTurn==TURN-1 and r.turn==TURN and r.afterSpent==r.beforeSpent)");
}

#[test]
fn capacity_civic_unknown_setter_result_records_phase_and_does_not_replay() {
    let h = Enemy::new();
    h.captive_capacity();
    h.run("local original=Players[0].GetCulture; Players[0].GetCulture=function() local culture=original(); culture.SetCivic=function(_,index,grant) assert(index==13 and grant); SETTER_CALLS=(SETTER_CALLS or 0)+1; error('simulated native setter failure') end; return culture end");
    h.civic_roundtrip(true);
    h.run("local c=assert(Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_RECEIPT')); local logged=false; for _,line in ipairs(LOGS) do if string.find(line,'simulated native setter failure',1,true) then logged=true end end; assert(c.phase=='SETTER_ENTERED' and c.status=='ERROR' and c.error=='CALLBACK_FAILED' and logged); assert(SETTER_CALLS==1 and ExposedMembers.JAC_PersonaBoundary.lastAction=='ERROR_PREPARE_DISTRICT_CIVIC' and Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC')==nil)");
    h.reject(
        scene_lua(&capacity_civic_scene(), 3),
        "ERROR_PREPARE_DISTRICT_CIVIC",
    );
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PREPARE_DISTRICT_CIVIC')",
        "do not replay the setter",
    );
}

#[test]
fn capacity_civic_malformed_core_parameters_are_recorded_before_setter() {
    let h = Enemy::new();
    h.captive_capacity();
    h.run("ENEMY_CASES.fireCore('JAC_BoundaryPrepareCapacityCivic',{Action='PREPARE_DISTRICT_CIVIC',Token=1,Turn=TURN,Frame=1,PublishEpoch=0,Attacker=0,Defender=1,Source=2,Leader=ENEMY_CASES.leader,CivicIndex=999,Total=1,Spent=1}); local c=assert(Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_RECEIPT')); assert(c.phase=='ENTERED' and c.status=='ERROR' and c.error and not GOV_CIVIC_COMPLETED and Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC')==nil)");
}

#[test]
fn capacity_government_plan_exposes_official_tier_and_change_gates() {
    let h = Enemy::new();
    h.captive_capacity();
    h.run("GOV_CIVIC_COMPLETED=true; GOV_DISTRICT_BUILT=true; GOV_DISTRICT_PLACED=true; GOV_CURRENT=0; local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='TIER1_GOVERNMENT_BLOCKED' and p.government.stage=='NO_CIVIC_COMPLETED_THIS_TURN' and p.government.currentType=='GOVERNMENT_CHIEFDOM' and p.government.target.civicType=='CIVIC_JAC_SENGOKU_DOMAIN_LAW' and p.cities[1].tier1Production.BUILDING_GOV_TALL.reasons=='GOVERNMENT_TIER_REQUIREMENT_Tier1')");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('CHANGE_CAPACITY_GOVERNMENT')",
        "official capacity Government change is unavailable",
    );
    h.run("assert(GOV_REQUEST_CALLS==0 and Game:GetProperty('JAC_BOUNDARY_CAPACITY_GOVERNMENT_INTENTS')==nil)");
    let scene = capacity_government_scene("plan");
    h.run("CONTROL.id=65536");
    let observation: String = h.0.load(scene_lua(&scene, 2)).strict_eval().unwrap();
    assert!(observation.contains("TIER1_GOVERNMENT_BLOCKED"));
    assert!(observation.contains("NO_CIVIC_COMPLETED_THIS_TURN"));
    h.run(scene_lua(&scene, 3));
    h.run("FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='JAC_BoundaryCapacityGovernmentProbe'); ENEMY_CASES.fireCore('JAC_BoundaryCapacityGovernmentProbe',r.params); FRAME(); local b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME()");
    assert!(h
        .0
        .load(scene_lua(&scene, 4))
        .strict_eval::<bool>()
        .unwrap());
    let core: String = h.0.load(scene_lua(&scene, 5)).strict_eval().unwrap();
    assert!(core.contains("gameplayStatus=OBSERVED getter=function index=0"));
}

#[test]
fn capacity_tier_one_government_change_is_one_official_request_with_core_and_ui_proof() {
    let scene = capacity_government_scene("tier1");
    let h = Enemy::new();
    h.capacity_government_ready();
    h.run(scene_lua(&scene, 2));
    h.capacity_government_complete_queued();
    assert!(h
        .0
        .load(scene_lua(&scene, 3))
        .strict_eval::<bool>()
        .unwrap());
    assert_eq!(scene["steps"][4]["action"], "end_turn");
    h.run("ADVANCE_TURN()");
    assert!(h
        .0
        .load(scene_lua(&scene, 5))
        .strict_eval::<bool>()
        .unwrap());
    h.capacity_government_complete_next_turn();
    assert!(h
        .0
        .load(scene_lua(&scene, 6))
        .strict_eval::<bool>()
        .unwrap());
    h.run(scene_lua(&scene, 7));
    assert_eq!(scene["steps"][8]["action"], "save");
    h.run("local intents=assert(Game:GetProperty('JAC_BOUNDARY_CAPACITY_GOVERNMENT_INTENTS')); local intent=assert(intents.Tier1); local read=assert(Game:GetProperty('JAC_BOUNDARY_CAPACITY_GOVERNMENT_READ')); local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); local d=ExposedMembers.JAC_PersonaBoundary.EnemyRetryDiagnosis(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='ACCEPTED_CHANGE_CAPACITY_GOVERNMENT' and intent.status=='PREPARED' and intent.targetGovernment=='GOVERNMENT_OLIGARCHY' and intent.beforeGovernment==0 and intent.beforeSpent==1 and read.status=='OBSERVED' and read.governmentType=='GOVERNMENT_OLIGARCHY' and read.governmentTier=='Tier1' and read.spent==intent.beforeSpent and read.turn==intent.turn+1 and read.originalToken==intent.token and p.stage=='BUILD_TIER1' and d.spyID==655368 and d.captive.captor==1 and d.captive.nameIndex==-1 and GOV_REQUEST_CALLS==1)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('CHANGE_CAPACITY_GOVERNMENT')",
        "already attempted",
    );
}

#[test]
fn capacity_tier_two_government_requires_its_real_civic_and_tier_one_building() {
    let scene = capacity_government_scene("tier2");
    let h = Enemy::new();
    h.captive_capacity();
    h.run("GOV_CIVIC_COMPLETED=true; GOV_DISTRICT_BUILT=true; GOV_DISTRICT_PLACED=true; GOV_TIER1_BUILT=true; GOV_CURRENT=1; GOV_TIER2_CIVIC_COMPLETED=false; GOV_CIVIC_COMPLETED_THIS_TURN=false; local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='NEEDS_TIER2_CIVIC' and p.government.target.governmentType=='GOVERNMENT_MERCHANT_REPUBLIC' and p.government.target.civicType=='CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS' and p.governmentRequirement=='Tier2'); GOV_TIER2_CIVIC_COMPLETED=true; GOV_CIVIC_COMPLETED_THIS_TURN=true; p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='NEEDS_TIER2_GOVERNMENT' and p.government.stage=='READY_CHANGE')");
    h.run(scene_lua(&scene, 2));
    h.capacity_government_complete_queued();
    assert!(h
        .0
        .load(scene_lua(&scene, 3))
        .strict_eval::<bool>()
        .unwrap());
    h.run("ADVANCE_TURN()");
    assert!(h
        .0
        .load(scene_lua(&scene, 5))
        .strict_eval::<bool>()
        .unwrap());
    h.capacity_government_complete_next_turn();
    assert!(h
        .0
        .load(scene_lua(&scene, 6))
        .strict_eval::<bool>()
        .unwrap());
    h.run(scene_lua(&scene, 7));
    h.run("local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='ACCEPTED_CHANGE_CAPACITY_GOVERNMENT' and p.stage=='BUILD_SPIES' and p.buildingType=='BUILDING_GOV_SPIES' and p.capacity==1 and p.occupied==1 and GOV_REQUEST_CALLS==1)");
}

#[test]
fn capacity_government_tier_two_intent_preserves_the_saved_tier_one_receipt() {
    let h = Enemy::new();
    h.capacity_government_ready();
    h.run("ExposedMembers.JAC_PersonaBoundary.Queue('CHANGE_CAPACITY_GOVERNMENT')");
    h.capacity_government_complete_queued();
    h.run("ADVANCE_TURN()");
    h.capacity_government_complete_next_turn();
    h.run("local i=Game:GetProperty('JAC_BOUNDARY_CAPACITY_GOVERNMENT_INTENTS'); TIER1_INTENT_TOKEN=i.Tier1.token; assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='ACCEPTED_CHANGE_CAPACITY_GOVERNMENT'); GOV_TIER1_BUILT=true; GOV_TIER2_CIVIC_COMPLETED=true; GOV_CIVIC_COMPLETED_THIS_TURN=true; local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='NEEDS_TIER2_GOVERNMENT'); ExposedMembers.JAC_PersonaBoundary.Queue('CHANGE_CAPACITY_GOVERNMENT')");
    h.capacity_government_complete_queued();
    h.run("ADVANCE_TURN()");
    h.capacity_government_complete_next_turn();
    h.run("local i=Game:GetProperty('JAC_BOUNDARY_CAPACITY_GOVERNMENT_INTENTS'); local r=Game:GetProperty('JAC_BOUNDARY_CAPACITY_GOVERNMENT_READ'); assert(i.Tier1.status=='PREPARED' and i.Tier1.token==TIER1_INTENT_TOKEN and i.Tier2.status=='PREPARED' and i.Tier2.token>i.Tier1.token and r.tier=='Tier2' and r.governmentType=='GOVERNMENT_MERCHANT_REPUBLIC' and GOV_REQUEST_CALLS==2 and ExposedMembers.JAC_PersonaBoundary.lastAction=='ACCEPTED_CHANGE_CAPACITY_GOVERNMENT')");
}

#[test]
fn capacity_tier_two_civic_scene_preserves_domain_law_and_never_replays_the_setter() {
    let scene: Value = serde_json::from_str(&read_repo("tools/civ6-autotest/scenarios/leader-persona-spy-neutralization-retry-capacity-tier2-civic.json")).unwrap();
    let h = Enemy::new();
    h.captive_capacity();
    h.civic_roundtrip(true);
    h.run("ORIGINAL_DOMAIN_TOKEN=Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_ATTEMPT').token; GOV_CURRENT=1; GOV_DISTRICT_BUILT=true; GOV_DISTRICT_PLACED=true; GOV_TIER1_BUILT=true; GOV_TIER2_CIVIC_COMPLETED=false; GOV_CIVIC_COMPLETED_THIS_TURN=true; local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='NEEDS_TIER2_CIVIC' and p.government.target.governmentType=='GOVERNMENT_MERCHANT_REPUBLIC')");
    h.run("local original=Players[0].GetCulture; Players[0].GetCulture=function() local culture=original(); culture.SetCivic=function(_,index,grant) assert(index==14 and grant); TIER2_SETTER_CALLS=(TIER2_SETTER_CALLS or 0)+1 end; return culture end");
    h.run(scene_lua(&scene, 2));
    h.tier2_civic_complete_queued();
    assert!(h
        .0
        .load(scene_lua(&scene, 3))
        .strict_eval::<bool>()
        .unwrap());
    h.run("local a=assert(Game:GetProperty('JAC_BOUNDARY_TIER2_CIVIC_ATTEMPT')); assert(a.owner==0 and a.index==14 and a.civicType=='CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS' and a.targetGovernment=='GOVERNMENT_MERCHANT_REPUBLIC' and a.beforeSpent==1 and TIER2_SETTER_CALLS==1 and Game:GetProperty('JAC_BOUNDARY_TIER2_CIVIC')==nil); ADVANCE_TURN(); GOV_TIER2_CIVIC_COMPLETED=true");
    assert_eq!(scene["steps"][4]["action"], "end_turn");
    assert!(h
        .0
        .load(scene_lua(&scene, 5))
        .strict_eval::<bool>()
        .unwrap());
    h.run(scene_lua(&scene, 6));
    h.tier2_civic_complete_next_turn();
    assert!(h
        .0
        .load(scene_lua(&scene, 7))
        .strict_eval::<bool>()
        .unwrap());
    h.run(scene_lua(&scene, 8));
    assert_eq!(scene["steps"][9]["action"], "save");
    h.run("local domain=Game:GetProperty('JAC_BOUNDARY_CAPACITY_CIVIC_ATTEMPT'); local tier2=Game:GetProperty('JAC_BOUNDARY_TIER2_CIVIC_ATTEMPT'); assert(domain.index==13 and domain.token==ORIGINAL_DOMAIN_TOKEN and tier2.index==14 and TIER2_SETTER_CALLS==1); ENEMY_CASES.fireCore('JAC_BoundaryPrepareTier2CapacityCivic',TIER2_SETTER_PARAMS); assert(TIER2_SETTER_CALLS==1)");
}

#[test]
fn capacity_tier_two_civic_scene_stops_before_save_when_native_read_is_pending() {
    let scene: Value = serde_json::from_str(&read_repo("tools/civ6-autotest/scenarios/leader-persona-spy-neutralization-retry-capacity-tier2-civic.json")).unwrap();
    let h = Enemy::new();
    h.captive_capacity();
    h.run("GOV_CIVIC_COMPLETED=true; GOV_CURRENT=1; GOV_DISTRICT_BUILT=true; GOV_DISTRICT_PLACED=true; GOV_TIER1_BUILT=true; GOV_TIER2_CIVIC_COMPLETED=false");
    h.run("local original=Players[0].GetCulture; Players[0].GetCulture=function() local culture=original(); culture.SetCivic=function(_,index,grant) assert(index==14 and grant); TIER2_SETTER_CALLS=(TIER2_SETTER_CALLS or 0)+1 end; return culture end");
    h.run(scene_lua(&scene, 2));
    h.tier2_civic_complete_queued();
    assert!(h
        .0
        .load(scene_lua(&scene, 3))
        .strict_eval::<bool>()
        .unwrap());
    h.run("ADVANCE_TURN()");
    h.run(scene_lua(&scene, 6));
    h.tier2_civic_complete_next_turn();
    h.reject(
        scene_lua(&scene, 7),
        "PENDING_RESUME_TIER2_CAPACITY_CIVIC_READ",
    );
    h.run("assert(TIER2_SETTER_CALLS==1 and Game:GetProperty('JAC_BOUNDARY_TIER2_CIVIC')==nil)");
    assert!(h.0.load(scene_lua(&scene, 8)).exec().is_err());
}

#[test]
fn capacity_government_unknown_native_result_does_not_replay_the_change() {
    let h = Enemy::new();
    h.capacity_government_ready();
    h.run("local original=Players[0].GetCulture; Players[0].GetCulture=function() local culture=original(); culture.RequestChangeGovernment=function(_,hash) assert(hash==GameInfo.Types.GOVERNMENT_OLIGARCHY.Hash); GOV_REQUEST_CALLS=GOV_REQUEST_CALLS+1; GOV_CURRENT=1; return nil end; return culture end");
    h.run("ExposedMembers.JAC_PersonaBoundary.Queue('CHANGE_CAPACITY_GOVERNMENT'); FRAME(); ENEMY_CASES.fireCore('JAC_BoundaryCapacityGovernmentIntent',REQUESTS[#REQUESTS].params); FRAME(); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',REQUESTS[#REQUESTS].params); fire(Events.GameCoreEventPublishComplete); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='REJECTED_CHANGE_CAPACITY_GOVERNMENT' and GOV_REQUEST_CALLS==1 and GOV_CURRENT==1); local i=Game:GetProperty('JAC_BOUNDARY_CAPACITY_GOVERNMENT_INTENTS'); assert(i.Tier1.status=='PREPARED')");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('CHANGE_CAPACITY_GOVERNMENT')",
        "already attempted",
    );
    h.run("assert(GOV_REQUEST_CALLS==1)");
}

#[test]
fn capacity_government_missing_gameplay_reader_records_failure_before_native_change() {
    let h = Enemy::new();
    h.capacity_government_ready();
    h.run("local original=Players[0].GetCulture; Players[0].GetCulture=function() local culture=original(); if GAMEPLAY_ONLY then culture.GetCurrentGovernment=nil end; return culture end; ExposedMembers.JAC_PersonaBoundary.Queue('CHANGE_CAPACITY_GOVERNMENT'); FRAME(); GAMEPLAY_ONLY=true; ENEMY_CASES.fireCore('JAC_BoundaryCapacityGovernmentIntent',REQUESTS[#REQUESTS].params); GAMEPLAY_ONLY=false; FRAME(); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',REQUESTS[#REQUESTS].params); fire(Events.GameCoreEventPublishComplete); FRAME(); local i=Game:GetProperty('JAC_BOUNDARY_CAPACITY_GOVERNMENT_INTENTS'); assert(i.Tier1.status=='ERROR' and ExposedMembers.JAC_PersonaBoundary.lastAction=='ERROR_CHANGE_CAPACITY_GOVERNMENT' and GOV_REQUEST_CALLS==0)");
    h.run("ExposedMembers.JAC_PersonaBoundary.Queue('PROBE_CAPACITY_GOVERNMENT_API'); FRAME(); GAMEPLAY_ONLY=true; ENEMY_CASES.fireCore('JAC_BoundaryCapacityGovernmentProbe',REQUESTS[#REQUESTS].params); GAMEPLAY_ONLY=false; FRAME(); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',REQUESTS[#REQUESTS].params); fire(Events.GameCoreEventPublishComplete); FRAME(); local r=Game:GetProperty('JAC_BOUNDARY_CAPACITY_GOVERNMENT_API_PROBE'); assert(r.status=='API_UNAVAILABLE' and r.getter=='nil' and GOV_REQUEST_CALLS==0)");
}

#[test]
fn capacity_government_read_rejects_changed_original_intent_after_one_request() {
    let scene = capacity_government_scene("tier1");
    let h = Enemy::new();
    h.capacity_government_ready();
    h.run(scene_lua(&scene, 2));
    h.capacity_government_complete_queued();
    h.run("ADVANCE_TURN(); FRAME(); local original=REQUESTS[#REQUESTS].params; assert(original.OnStart=='JAC_BoundaryCapacityGovernmentRead' and original.Turn==TURN); local wrong={}; for key,value in pairs(original) do wrong[key]=value end; wrong.OriginalToken=wrong.OriginalToken+1; ENEMY_CASES.fireCore('JAC_BoundaryCapacityGovernmentRead',wrong); local r=Game:GetProperty('JAC_BOUNDARY_CAPACITY_GOVERNMENT_READ'); assert(r.status=='ERROR' and GOV_REQUEST_CALLS==1); FRAME(); local b=REQUESTS[#REQUESTS]; ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME()");
    h.reject(scene_lua(&scene, 6), "ERROR_CHANGE_CAPACITY_GOVERNMENT");
    h.run("assert(GOV_REQUEST_CALLS==1)");
}

#[test]
fn capacity_government_change_without_real_tier_one_production_cannot_save() {
    let scene = capacity_government_scene("tier1");
    let h = Enemy::new();
    h.capacity_government_ready();
    h.run(scene_lua(&scene, 2));
    h.capacity_government_complete_queued();
    h.run("ADVANCE_TURN(); local original=CONTROL.GetBuildQueue; CONTROL.GetBuildQueue=function() local queue=original(); local can=queue.CanProduce; queue.CanProduce=function(_,hash,...) if hash==760 or hash==770 or hash==780 then return false,'POLICY_SLOTS_OPEN' end; return can(queue,hash,...) end; return queue end");
    h.capacity_government_complete_next_turn();
    h.reject(scene_lua(&scene, 6), "PENDING_CHANGE_CAPACITY_GOVERNMENT");
    h.run("local p=ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan(); assert(p.stage=='TIER1_PRODUCTION_UNAVAILABLE' and p.cities[1].tier1Production.BUILDING_GOV_TALL.reasons=='POLICY_SLOTS_OPEN' and GOV_REQUEST_CALLS==1)");
    assert!(h.0.load(scene_lua(&scene, 7)).exec().is_err());
}

#[test]
fn native_district_instance_owner_city_and_id_are_required_at_build_acceptance() {
    for mutation in [
        "GOV_DISTRICT_TYPE=71",
        "GOV_DISTRICT_OWNER=1",
        "GOV_DISTRICT_CITY=TARGET",
        "GOV_DISTRICT_CITY=FOREIGN",
        "GOV_DISTRICT_ID=nil",
        "GOV_DISTRICT_MISSING_GETTER='GetOwner'",
        "GOV_DISTRICT_MISSING_GETTER='GetCity'",
        "GOV_DISTRICT_MISSING_GETTER='GetID'",
    ] {
        let h = Enemy::new();
        h.captured_district();
        h.run(&format!("ExposedMembers.JAC_PersonaBoundary.Queue('BUILD_CAPACITY_DISTRICT'); FRAME(); {mutation}; FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='REJECTED_BUILD_CAPACITY_DISTRICT' and #BUILD_REQUESTS==1 and CAPACITY_PROGRESS_CALLS==0)"));
        h.reject(
            "ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_CAPACITY_DISTRICT')",
            "BUILD acceptance is missing",
        );
    }
}

#[test]
fn native_district_instance_change_before_core_progress_denies_add_progress() {
    for mutation in [
        "GOV_DISTRICT_TYPE=71",
        "GOV_DISTRICT_OWNER=1",
        "GOV_DISTRICT_CITY=TARGET",
        "GOV_DISTRICT_CITY=FOREIGN",
        "GOV_DISTRICT_ID=992",
        "GOV_DISTRICT_MEMBER_KEY=2",
        "GOV_DISTRICT_MISSING_GETTER='GetCity'",
    ] {
        let h = Enemy::new();
        h.captured_district();
        h.run("ExposedMembers.JAC_PersonaBoundary.Queue('BUILD_CAPACITY_DISTRICT'); FRAME(); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='READY_BUILD_CAPACITY_DISTRICT'); local q=ExposedMembers.JAC_PersonaBoundary.ReadCapacityQueue(0,10); assert(q.districtOwner==0 and q.districtCityOwner==0 and q.districtCityID==10 and q.districtID==991 and q.memberKey==1 and q.acceptedNonce>0); ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_CAPACITY_DISTRICT'); FRAME()");
        h.run(mutation);
        h.reject(
            "ENEMY_CASES.fireCore('JAC_BoundaryProgressCapacityDistrict',REQUESTS[#REQUESTS].params)",
            "queue changed before progress",
        );
        h.run("assert(CAPACITY_PROGRESS_CALLS==0 and Game:GetProperty('JAC_BOUNDARY_CAPACITY_DISTRICT_PROGRESS')==nil)");
    }
}

#[test]
fn native_district_instance_change_after_turn_denies_completion() {
    for mutation in [
        "GOV_DISTRICT_TYPE=71",
        "GOV_DISTRICT_OWNER=1",
        "GOV_DISTRICT_CITY=TARGET",
        "GOV_DISTRICT_CITY=FOREIGN",
        "GOV_DISTRICT_ID=992",
        "GOV_DISTRICT_MEMBER_KEY=2",
        "GOV_DISTRICT_MISSING_GETTER='GetID'",
    ] {
        let h = Enemy::new();
        h.captured_district();
        h.run("ExposedMembers.JAC_PersonaBoundary.Queue('BUILD_CAPACITY_DISTRICT'); FRAME(); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='READY_BUILD_CAPACITY_DISTRICT')");
        h.sync(
            "PROGRESS_CAPACITY_DISTRICT",
            "JAC_BoundaryProgressCapacityDistrict",
        );
        h.run(&format!("ADVANCE_TURN(); {mutation}"));
        h.reject(
            "ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityDistrictOutcome()",
            "district identity changed after progress",
        );
        h.run("assert(CAPACITY_PROGRESS_CALLS==1 and Game:GetProperty('JAC_BOUNDARY_CAPACITY_DISTRICT_PROGRESS').districtID==991)");
    }
}

#[test]
fn government_plaza_rejects_foreign_placement_and_stale_native_queue() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}; OFF_MAP={{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}; GOV_CIVIC_COMPLETED=true; REAL_TARGETS=CityManager.GetOperationTargets; CityManager.GetOperationTargets=function() return {[CityOperationResults.PLOTS]={FOREIGN.x+FOREIGN.y*10}} end; assert(ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan().stage=='DISTRICT_UNAVAILABLE')");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('BUILD_CAPACITY_DISTRICT')",
        "no proven owned placement",
    );
    h.run("assert(#BUILD_REQUESTS==0 and CAPACITY_PROGRESS_CALLS==0); CityManager.GetOperationTargets=REAL_TARGETS; ExposedMembers.JAC_PersonaBoundary.Queue('BUILD_CAPACITY_DISTRICT'); FRAME(); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='READY_BUILD_CAPACITY_DISTRICT'); ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_CAPACITY_DISTRICT'); FRAME(); GOV_DISTRICT_X=1; GOV_DISTRICT_Y=2; local q=ExposedMembers.JAC_PersonaBoundary.ReadCapacityQueue(0,10); assert(q.districtType=='DISTRICT_GOVERNMENT' and q.districtPlot==21 and q.acceptedNonce==nil)");
    h.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryProgressCapacityDistrict',REQUESTS[#REQUESTS].params)",
        "queue changed before progress",
    );
    h.run("GOV_DISTRICT_X=1; GOV_DISTRICT_Y=1; local r=REQUESTS[#REQUESTS].params; r.AcceptedNonce=r.AcceptedNonce+1");
    h.reject(
        "ENEMY_CASES.fireCore('JAC_BoundaryProgressCapacityDistrict',REQUESTS[#REQUESTS].params)",
        "queue changed before progress",
    );
    h.run("assert(CAPACITY_PROGRESS_CALLS==0 and Game:GetProperty('JAC_BOUNDARY_CAPACITY_DISTRICT_PROGRESS')==nil and #CAPTURED[1]==1)");
}

#[test]
fn same_district_type_on_a_different_owned_plot_is_not_accepted_as_the_build_receipt() {
    let h = Enemy::new();
    h.failed();
    h.run("UNITS[0]={}; CAPTURED[1]={{OwningPlayer=0,Name='Spy 15',Level=1,NameIndex=7}}; OFF_MAP={{Name='Spy 15',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}; GOV_CIVIC_COMPLETED=true; GOV_DISTRICT_X=1; GOV_DISTRICT_Y=2; assert(ExposedMembers.JAC_PersonaBoundary.CaptiveCapacityPlan().plot==11); ExposedMembers.JAC_PersonaBoundary.Queue('BUILD_CAPACITY_DISTRICT'); FRAME(); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='REJECTED_BUILD_CAPACITY_DISTRICT' and BUILD_HEAD.DistrictType==70 and #BUILD_REQUESTS==1)");
    h.reject(
        "ExposedMembers.JAC_PersonaBoundary.Queue('PROGRESS_CAPACITY_DISTRICT')",
        "BUILD acceptance is missing",
    );
    h.run("assert(CAPACITY_PROGRESS_CALLS==0 and Game:GetProperty('JAC_BOUNDARY_CAPACITY_DISTRICT_PROGRESS')==nil)");
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
