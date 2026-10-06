//! Lua 5.1 contract tests. Mocked native rows are never real Civ6 receipts.
#[path = "../src/spy_prepared_mission.rs"]
mod mission;

use mlua::Lua;

const FIXTURE: &str = r#"
_G=nil;
PROPERTIES={}; CONTEXT='UI'; TURN=70; TIME=12; REQUESTS={}; WRITES={};
UNIT_ID=700; BAD_FIND=false; UNIT_OP=-1; UNIT_END=-1;
GOV_REMAIN=0; GOV_CITY=20; GOV_EST=true; EFFECT_ACTIVE=true; MERIT=10;
LAST_GOVERN_TURN=nil; DEED=nil; NODES={BASE=1};
MISSION4=0; CORE_GETTER_ERROR=false; UI_GETTER_ERROR=false; ASSIGN_REQUESTS=0;
local function h(id,turn)
 return {id=id,turn=turn,operation=59,plot=8511,initial=1,escape=-1}
end
H={h(0,23),h(1,45),h(2,62)};
PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE={histories={H[1],H[2]}};
PROPERTIES.JAC_BOUNDARY_THIRD_SPY_STATE={missionPost={histories='UI'}};
PROPERTIES.JAC_BOUNDARY_THIRD_MISSION_ACK={observed={preserved={H[1],H[2]},mission=H[3]}};
PROPERTIES.JAC_BOUNDARY_ENEMY_MISSION={attacker=0,id=2,turn=62};
local before={turn=62,physical=3,captured={{},{},{}},histories=H};
local after={turn=62,physical=3,capacity=3,captured={{},{}},activeOffMap={},
 map={{id=700,owner=0,unitType=18,x=140,y=40}},histories=H};
PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62={stage='RETURNED',turn=62,
 nonce='exchange:62',before=before,returnEvidence={stage='RETURNED_MAP',
 returned={id=700,owner=0,unitType=18,x=140,y=40,turn=62},after=after}};
local sourceEvent={attacker=0,id=3,spyID=700,targetOwner=1,targetCity=20,
 operation=60,plot=8511,turn=69,initial=0,escape=-1,
 raw={Operation=60,PlotIndex=8511,CompletionTurn=69,InitialResult=0,EscapeResult=-1},
 successEnums={undetected=0,mustEscape=2,failUndetected=1,
 failEscape=3,killed=4,captured=5}};
local sourceSpy={id=700,owner=0,unitType=18,x=145,y=40,operation=-1,endTurn=-1};
local sourceTarget={owner=1,city=20,x=145,y=40,plot=8511};
local sources={turn=62,spy=sourceSpy,target=sourceTarget,sourceTurns=0,
 gainOperation={index=60,hash=600},comparisonOperation={index=59,hash=590},
 comparisonProbability={available=true,success=0.4}};
PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION={stage='SOURCES_RECEIPT',
 outcome='SOURCES_COMPLETE',nonce='return:1',exchangeNonce='exchange:62',
 returnedID=700,oldHistories=H,arrival={unit=sourceSpy},sources=sources,
 sourcesReceipt={stage='SOURCES_COMPLETE',event=sourceEvent,
  after={spy=sourceSpy,spyInTarget=true,sourceTurns=6,
   comparisonProbability={available=true,success=0.6}},
  effectState='POSITIVE',sourceTurnsDelta=6,
  comparison={before=0.4,after=0.6,delta=0.6-0.4}}};
PROPERTIES.JAC_BOUNDARY_ENEMY_CONFIG={attacker=0,owner=1,leader='LEADER_TEST',cityID=20};
local function serialize(v)
 if type(v)~='table' then return type(v)..':'..tostring(v) end
 local keys={}; for k in pairs(v) do keys[#keys+1]=k end
 table.sort(keys,function(a,b) return tostring(a)<tostring(b) end)
 local parts={}; for _,k in ipairs(keys) do
  parts[#parts+1]=serialize(k)..'='..serialize(v[k]) end
 return '{'..table.concat(parts,',')..'}'
end
function PROTECTED_SNAPSHOT()
 return serialize({exchange=PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62,
  preparation=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION,
  archive=PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE,
  state=PROPERTIES.JAC_BOUNDARY_THIRD_SPY_STATE,
  ack=PROPERTIES.JAC_BOUNDARY_THIRD_MISSION_ACK,
  event=PROPERTIES.JAC_BOUNDARY_ENEMY_MISSION})
end
BASELINE_PROTECTED=PROTECTED_SNAPSHOT();
Game={};
function Game:GetProperty(k)
 if k=='JAC_PERSONA_LEADER_TEST_MERIT' then return MERIT end
 local node=k:match('^JAC_PERSONA_LEADER_TEST_NODE_(.+)$');
 if node then return NODES[node] end
 if k=='JAC_PERSONA_LEADER_TEST_GOVERN_LAST_TURN' then return LAST_GOVERN_TURN end
 if string.find(k,'JAC_PERSONA_LEADER_TEST_DEED_GOVERN_TURN_',1,true)==1 then
  return DEED end
 return PROPERTIES[k]
end
function Game:SetProperty(k,v)
 assert(CONTEXT=='CORE' and k=='JAC_BOUNDARY_PREPARED_MISSION',
  'foreign or UI property write');
 WRITES[#WRITES+1]=k; PROPERTIES[k]=v
end
function Game.GetCurrentGameTurn() return TURN end
function Game.GetLocalPlayer() return 0 end
local city={};
function city:GetOwner() return 1 end
function city:GetID() return 20 end
function city:GetX() return 145 end
function city:GetY() return 40 end
function city:GetName() return 'Target' end
CityManager={GetCity=function(owner,id)
 if owner==1 and id==20 then return city end
end};
Map={GetPlot=function(x,y)
 if x==145 and y==40 then return {GetIndex=function() return 8511 end} end
end};
Cities={GetPlotPurchaseCity=function(plot)
 if plot and plot:GetIndex()==8511 then return city end
end};
local unit={};
function unit:GetID() return BAD_FIND and UNIT_ID+1 or UNIT_ID end
function unit:GetOwner() return 0 end
function unit:GetType() return 18 end
function unit:GetX() return 145 end
function unit:GetY() return 40 end
function unit:GetSpyOperation() return UNIT_OP end
function unit:GetSpyOperationEndTurn() return UNIT_END end
local units={};
function units:FindID(id) if id==UNIT_ID or BAD_FIND then return unit end end
local dip={};
function dip:GetMission(owner,id)
 if CORE_GETTER_ERROR and CONTEXT=='CORE' then error('Core getter ABI failure') end
 if UI_GETTER_ERROR and CONTEXT=='UI' and id==4 then error('UI getter ABI failure') end
 if id<3 then local v=H[id+1]; return {Operation=v.operation,PlotIndex=v.plot,
  CompletionTurn=v.turn,InitialResult=v.initial,EscapeResult=v.escape} end
 if id==3 then return sourceEvent.raw end
 if id==4 then return MISSION4 end
 return 0
end
local player0={};
function player0:IsHuman() return true end
function player0:IsTurnActive() return true end
function player0:GetDiplomacy() return dip end
function player0:GetUnits() return units end
local governor={};
function governor:GetType() return 4 end
function governor:GetAssignedCity() if GOV_CITY==20 then return city end end
function governor:GetNeutralizedTurns() return GOV_REMAIN end
function governor:IsEstablished() return GOV_EST end
local governors={};
function governors:HasGovernor(hash) return hash==-23281316 end
function governors:GetGovernor(hash)
 if hash==-23281316 then return governor end
end
function governors:CanAssignGovernor(hash,c)
 return hash==-23281316 and c:GetID()==20 and GOV_REMAIN==0 and GOV_CITY==-1
end
local player1={};
function player1:IsHuman() return false end
function player1:GetGovernors() return governors end
Players={[0]=player0,[1]=player1};
GameInfo={Units={UNIT_SPY={Index=18}},
 UnitOperations={UNITOPERATION_SPY_GAIN_SOURCES={Index=60,Hash=600},
  UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR={Index=59,Hash=590}},
 JAC_LeaderPersonas={LEADER_TEST={GovernorType='GOV_TEST'}},
 Governors={GOV_TEST={Index=4,Hash=-23281316}},
 JAC_PersonaTreePeople={LEADER_TEST={MeritProfile='ADMIN'}},
 JAC_PersonaMeritRules={ADMIN={GOVERN_TURN=3}},
 JAC_PersonaTreeNodes={BASE={LeaderType='LEADER_TEST',Cost=0},
  EXTRA={LeaderType='LEADER_TEST',Cost=2}}};
UnitManager={};
function UnitManager.CanStartOperation(u,op,plot)
 return op==590 and plot:GetIndex()==8511 and UNIT_OP==-1 and GOV_REMAIN==0,
  {native='eligible'}
end
function UnitManager.GetTimeToComplete(op,u) assert(op==59); return 7 end
function UnitManager.GetResultProbability(op,u,plot)
 assert(op==59 and plot:GetIndex()==8511);
 return {ESPIONAGE_SUCCESS_UNDETECTED=0.5,ESPIONAGE_SUCCESS_MUST_ESCAPE=0.1}
end
function UnitManager.RequestOperation(u,op)
 REQUESTS[#REQUESTS+1]={id=u:GetID(),operation=op};
 assert(op==590); UNIT_OP=op; UNIT_END=TURN+7
end
UI={};
function UI.GetElapsedTime() return TIME end
function UI.DeselectAllUnits() SELECTED=nil end
function UI.DeselectAllCities() end
function UI.SelectUnit(u) SELECTED=u end
function UI.GetHeadSelectedUnit() return SELECTED end
function UI.SetInterfaceMode(mode) MODE=mode end
function UI.GetInterfaceMode() return MODE end
function UI.RequestPlayerOperation(owner,op,args)
 assert(owner==1 and op==77 and args[1]==4 and args[2]==1 and args[3]==20);
 ASSIGN_REQUESTS=ASSIGN_REQUESTS+1; GOV_CITY=20; GOV_EST=false
end
InterfaceModeTypes={SPY_CHOOSE_MISSION=5};
PlayerOperations={ASSIGN_GOVERNOR=77,PARAM_GOVERNOR_TYPE=1,
 PARAM_PLAYER_ONE=2,PARAM_CITY_DEST=3};
EspionageResultTypes={SUCCESS_UNDETECTED=0,SUCCESS_MUST_ESCAPE=2,
 FAIL_UNDETECTED=1,FAIL_MUST_ESCAPE=3,KILLED=4,CAPTURED=5};
ExposedMembers={JAC_PersonaBoundary={}};
function ExposedMembers.JAC_PersonaBoundary.ReadEnemy()
 local settled=EFFECT_ACTIVE and 1 or 0;
 local target={id=20,owner=1,settled=settled,
  effects={JAC_PERSONA_SETTLED_LEADER_TEST=settled}};
 local control={id=21,owner=1,settled=0,
  effects={JAC_PERSONA_SETTLED_LEADER_TEST=0}};
 return {owner=1,attacker=0,leader='LEADER_TEST',turn=TURN,
  personOwner=1,state='GOVERNOR',merit=MERIT,
  target={id=20,owner=1,plot=8511},targetKey='145/40',
  native={type=4,hash=-23281316,
   cityOwner=GOV_CITY==20 and 1 or -1,cityID=GOV_CITY,neutralized=GOV_REMAIN},
  gameplay={merit=MERIT,cities={['145/40']={id=20,owner=1,settled=settled},
   ['150/40']={id=21,owner=1,settled=0}}},
  cities={['145/40']=target,['150/40']=control},
  nodesOwned=NODES,unmatchedEffects={}}
end
function COMPLETE(initial,remaining)
 TURN=TURN+7; UNIT_OP=-1; UNIT_END=-1;
 GOV_REMAIN=remaining; EFFECT_ACTIVE=remaining==0;
 if remaining>0 then GOV_EST=false end
 MISSION4={Operation=59,PlotIndex=8511,CompletionTurn=TURN,
  InitialResult=initial,EscapeResult=-1}
end
"#;

struct Case(Lua);

impl Case {
    fn new() -> Self {
        let lua = Lua::new();
        lua.load(FIXTURE).exec().unwrap();
        lua.load(mission::UI_SETUP).exec().unwrap();
        Self(lua)
    }

    fn run(&self, code: &str) {
        self.0.load(code).exec().unwrap();
    }

    fn fails(&self, code: &str) {
        assert!(
            self.0.load(code).exec().is_err(),
            "unexpected success: {code}"
        );
    }

    fn core(&self, code: &str) {
        self.run("CONTEXT='CORE'");
        self.0.load(code).exec().unwrap();
        self.run("CONTEXT='UI'");
    }

    fn intent(&self) {
        self.run("JAC_PreparedMission548.Prepare()");
        self.core(mission::CORE_COMMIT);
        self.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='MISSION_INTENT')");
    }

    fn success(&self) {
        self.intent();
        self.run("JAC_PreparedMission548.ArmAfterSave('MISSION'); JAC_PreparedMission548.SendOnce(); COMPLETE(0,6); assert(JAC_PreparedMission548.RouteEvent(0,4)=='OWNED'); assert(JAC_PreparedMission548.Result()=='SUCCEEDED')");
        self.core(mission::CORE_COMMIT);
        self.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='SUCCEEDED')");
    }
}

#[test]
fn sources_and_formal_return_gate_all_requests() {
    for change in [
        "PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION=nil",
        "PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.outcome='SOURCES_FAILED'",
        "PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.sourcesReceipt.stage='SOURCES_UNKNOWN'",
        "PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.sourcesReceipt.comparison.delta=-0.1",
        "PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage='UNKNOWN'",
    ] {
        let c = Case::new();
        c.run(change);
        c.fails("JAC_PreparedMission548.Prepare()");
        c.run("assert(#REQUESTS==0 and ASSIGN_REQUESTS==0 and #WRITES==0)");
    }
    let c = Case::new();
    c.run(
        "MISSION4={Operation=59,PlotIndex=8511,CompletionTurn=69,InitialResult=1,EscapeResult=-1}",
    );
    c.fails("JAC_PreparedMission548.Prepare()");
    c.run("assert(#REQUESTS==0)");
}

#[test]
fn exact_returned_id_and_all_old_history_are_required() {
    let c = Case::new();
    c.run("BAD_FIND=true");
    c.fails("JAC_PreparedMission548.Prepare()");
    c.run("assert(#REQUESTS==0)");

    let c = Case::new();
    c.run("PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.sources.spy.id=701");
    c.fails("JAC_PreparedMission548.Prepare()");

    let c = Case::new();
    c.run("H[2].escape=0");
    c.fails("JAC_PreparedMission548.Prepare()");

    let c = Case::new();
    c.intent();
    c.run("PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.returnEvidence.after.captured[1].name='tampered'");
    c.fails("JAC_PreparedMission548.SendOnce()");
    c.run("assert(#REQUESTS==0)");
}

#[test]
fn native_success_failure_and_unknown_are_separate_receipts() {
    let c = Case::new();
    c.success();
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.receipt.event.id==4 and PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.receipt.governorRead.raw.neutralized==6 and BASELINE_PROTECTED==PROTECTED_SNAPSHOT() and #REQUESTS==1 and #WRITES>0)");

    let c = Case::new();
    c.intent();
    c.run("JAC_PreparedMission548.ArmAfterSave('MISSION'); JAC_PreparedMission548.SendOnce(); COMPLETE(5,0); assert(JAC_PreparedMission548.RouteEvent(0,4)=='OWNED'); assert(JAC_PreparedMission548.Result()=='FAILED')");
    c.core(mission::CORE_COMMIT);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='FAILED' and PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.receipt.failureResult==5)");
    c.fails("JAC_PreparedMission548.ObserveRecovery()");

    let c = Case::new();
    c.intent();
    c.run("JAC_PreparedMission548.ArmAfterSave('MISSION'); JAC_PreparedMission548.SendOnce(); COMPLETE(0,0); assert(JAC_PreparedMission548.RouteEvent(0,4)=='OWNED'); assert(JAC_PreparedMission548.Result()=='UNKNOWN_NATIVE')");
    c.core(mission::CORE_COMMIT);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='UNKNOWN_NATIVE')");
    c.fails("JAC_PreparedMission548.ObserveRecovery()");
}

#[test]
fn different_event_id_and_getter_exception_do_not_reach_old_handler() {
    let c = Case::new();
    c.intent();
    c.run(&format!(
        "OLD=0; function ON_EVENT(attacker,missionID) {} OLD=OLD+1 end",
        mission::UI_EARLY_EVENT_GATE
    ));
    c.run("ON_EVENT(0,3); ON_EVENT(1,4); assert(OLD==2)");
    c.run("ON_EVENT(0,5); assert(OLD==2 and ExposedMembers.JAC_PreparedMission548Witness.phase=='UNKNOWN')");
    c.core(mission::UNKNOWN_CORE);
    c.run(
        "assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='UNKNOWN_CORE' and #REQUESTS==0)",
    );

    let c = Case::new();
    c.intent();
    c.run(&format!(
        "OLD=0; function ON_EVENT(attacker,missionID) {} OLD=OLD+1 end",
        mission::UI_EARLY_EVENT_GATE
    ));
    c.run("JAC_PreparedMission548=nil");
    c.fails("ON_EVENT(0,4)");
    c.run("assert(OLD==0)");

    let c = Case::new();
    c.intent();
    c.run("JAC_PreparedMission548.ArmAfterSave('MISSION'); JAC_PreparedMission548.SendOnce(); COMPLETE(0,6); assert(JAC_PreparedMission548.RouteEvent(0,4)=='OWNED'); assert(JAC_PreparedMission548.Result()=='SUCCEEDED'); CORE_GETTER_ERROR=true");
    c.core(mission::CORE_COMMIT);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='UNKNOWN_CORE' and PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.lastRaw.observation.event.raw.InitialResult==0 and string.find(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.coreError,'Core native mission getter failed',1,true))");
}

#[test]
fn recovery_merit_challenge_and_official_reassignment_follow_native_state() {
    let c = Case::new();
    c.success();
    c.run("assert(JAC_PreparedMission548.ObserveRecovery()=='NEUTRALIZED')");
    c.core(mission::CORE_COMMIT);
    c.run("TURN=TURN+1; GOV_REMAIN=5; LAST_GOVERN_TURN=TURN; MERIT=11");
    c.fails("JAC_PreparedMission548.PositiveChallenge()");
    c.run("MERIT=10; EFFECT_ACTIVE=true");
    c.fails("JAC_PreparedMission548.PositiveChallenge()");
    c.run("EFFECT_ACTIVE=false; assert(JAC_PreparedMission548.PositiveChallenge()=='CHALLENGE_PROVED')");
    c.core(mission::CORE_COMMIT);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.challenge.positiveRule==3)");
    c.run("TURN=TURN+5; GOV_REMAIN=0; GOV_CITY=-1; GOV_EST=false; assert(JAC_PreparedMission548.ObserveRecovery()=='RECOVERED')");
    c.core(mission::CORE_COMMIT);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='RECOVERED'); JAC_PreparedMission548.PrepareAssignment()");
    c.core(mission::CORE_COMMIT);
    c.run("JAC_PreparedMission548.ArmAfterSave('ASSIGN'); JAC_PreparedMission548.AssignOnce(); assert(ASSIGN_REQUESTS==1); assert(JAC_PreparedMission548.ObserveAssignment()=='ASSIGN_ESTABLISHING')");
    c.core(mission::CORE_COMMIT);
    c.run("TURN=TURN+5; GOV_EST=true; EFFECT_ACTIVE=true; assert(JAC_PreparedMission548.ObserveAssignment()=='ASSIGN_ESTABLISHED')");
    c.core(mission::CORE_COMMIT);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='ASSIGN_ESTABLISHED' and BASELINE_PROTECTED==PROTECTED_SNAPSHOT() and ASSIGN_REQUESTS==1)");
}

#[test]
fn same_ui_load_resumes_observation_without_replay() {
    let c = Case::new();
    c.intent();
    c.run("assert(JAC_PreparedMission548.InvalidateOnLoad()=='UI_STATE_INVALIDATED')");
    assert_eq!(
        c.0.load(mission::UI_RESUME_IF_SAVED)
            .eval::<String>()
            .unwrap(),
        "MISSION_UNCONFIRMED"
    );
    c.fails("JAC_PreparedMission548.ArmAfterSave('MISSION')");
    c.fails("JAC_PreparedMission548.SendOnce()");
    c.run(
        "assert(#REQUESTS==0 and PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='MISSION_INTENT')",
    );
    c.run("MISSION4={Operation=59,PlotIndex=8511,CompletionTurn=TURN,InitialResult=0,EscapeResult=-1}; JAC_PreparedMission548.InvalidateOnLoad(); assert(JAC_PreparedMission548.Resume()=='MISSION_UNCONFIRMED')");
    c.fails("JAC_PreparedMission548.SendOnce()");
    c.run("assert(JAC_PreparedMission548.RouteEvent(0,4)=='OWNED_UNKNOWN' and #REQUESTS==0)");
    c.core(mission::UNKNOWN_CORE);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='UNKNOWN_CORE')");
}

#[test]
fn exported_names_match_the_single_new_property() {
    assert_eq!(mission::PROPERTY, "JAC_BOUNDARY_PREPARED_MISSION");
    assert_eq!(mission::WITNESS, "JAC_PreparedMission548Witness");
}

#[test]
fn saved_active_operation_can_route_later_event_without_resending() {
    let c = Case::new();
    c.intent();
    c.run("JAC_PreparedMission548.ArmAfterSave('MISSION'); JAC_PreparedMission548.SendOnce(); JAC_PreparedMission548.ObserveSent()");
    c.core(mission::CORE_COMMIT);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.sent.spy.id==700); JAC_PreparedMission548.InvalidateOnLoad(); assert(JAC_PreparedMission548.Resume()=='SENT_OBSERVED')");
    c.fails("JAC_PreparedMission548.SendOnce()");
    c.run("COMPLETE(0,6); assert(JAC_PreparedMission548.RouteEvent(0,4)=='OWNED'); assert(JAC_PreparedMission548.Result()=='SUCCEEDED')");
    c.core(mission::CORE_COMMIT);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='SUCCEEDED' and #REQUESTS==1)");
}

#[test]
fn ui_id_mixup_and_ui_getter_error_become_finite_unknowns() {
    let c = Case::new();
    c.intent();
    c.run("BAD_FIND=true; JAC_PreparedMission548.ArmAfterSave('MISSION')");
    c.fails("JAC_PreparedMission548.SendOnce()");
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.coreAvailability.spyFindID=='KNOWN_UNRELIABLE_NO_CALL' and #REQUESTS==0)");

    let c = Case::new();
    c.intent();
    c.run("JAC_PreparedMission548.ArmAfterSave('MISSION'); JAC_PreparedMission548.SendOnce(); COMPLETE(0,6); UI_GETTER_ERROR=true; assert(JAC_PreparedMission548.RouteEvent(0,4)=='OWNED_UNKNOWN')");
    c.core(mission::UNKNOWN_CORE);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='UNKNOWN_CORE' and PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.unknown.mission4Read.ok==false and #REQUESTS==1)");
}

#[test]
fn regular_merit_before_completion_becomes_the_success_stop_baseline() {
    let c = Case::new();
    c.intent();
    c.run("JAC_PreparedMission548.ArmAfterSave('MISSION'); JAC_PreparedMission548.SendOnce(); MERIT=31; COMPLETE(0,6); assert(JAC_PreparedMission548.RouteEvent(0,4)=='OWNED'); assert(JAC_PreparedMission548.Result()=='SUCCEEDED')");
    c.core(mission::CORE_COMMIT);
    c.run("local p=PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION; assert(p.before.merit==10 and p.receipt.stopBaseline.merit==31 and p.receipt.stopBaseline.turn==TURN); assert(JAC_PreparedMission548.ObserveRecovery()=='NEUTRALIZED')");
    c.core(mission::CORE_COMMIT);
    c.run("TURN=TURN+1; GOV_REMAIN=5; LAST_GOVERN_TURN=TURN; assert(JAC_PreparedMission548.PositiveChallenge()=='CHALLENGE_PROVED')");
    c.core(mission::CORE_COMMIT);
    c.run("MERIT=32");
    c.fails("JAC_PreparedMission548.ObserveRecovery()");
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.receipt.stopBaseline.merit==31 and BASELINE_PROTECTED==PROTECTED_SNAPSHOT())");
}

#[test]
fn skill_purchases_before_and_after_success_use_separate_baselines() {
    let c = Case::new();
    c.intent();
    c.run("JAC_PreparedMission548.ArmAfterSave('MISSION'); JAC_PreparedMission548.SendOnce(); MERIT=29; NODES.EXTRA=1; COMPLETE(0,6); JAC_PreparedMission548.RouteEvent(0,4); assert(JAC_PreparedMission548.Result()=='SUCCEEDED')");
    c.core(mission::CORE_COMMIT);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.receipt.stopBaseline.nodesOwned.EXTRA==1); GameInfo.JAC_PersonaTreeNodes.LATE={LeaderType='LEADER_TEST',Cost=4}; NODES.LATE=1; MERIT=25; assert(JAC_PreparedMission548.ObserveRecovery()=='NEUTRALIZED')");
    c.core(mission::CORE_COMMIT);
    c.run("local p=PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION; local r=p.recovery.observations[1]; assert(r.meritWithPurchases==29 and r.purchased==4 and p.receipt.stopBaseline.nodesOwned.LATE==nil); NODES.EXTRA=nil");
    c.fails("JAC_PreparedMission548.ObserveRecovery()");
}

#[test]
fn delayed_success_read_is_unknown_instead_of_inventing_completion_merit() {
    let c = Case::new();
    c.intent();
    c.run("JAC_PreparedMission548.ArmAfterSave('MISSION'); JAC_PreparedMission548.SendOnce(); COMPLETE(0,6); JAC_PreparedMission548.RouteEvent(0,4); TURN=TURN+1; assert(JAC_PreparedMission548.Result()=='UNKNOWN_NATIVE')");
    c.core(mission::CORE_COMMIT);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.stage=='UNKNOWN_NATIVE' and PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.receipt.stopBaseline==nil)");
    c.fails("JAC_PreparedMission548.ObserveRecovery()");
}

#[test]
fn initial_observation_failure_can_be_saved_without_creating_send_permission() {
    let c = Case::new();
    c.run("BAD_FIND=true");
    c.fails("JAC_PreparedMission548.Prepare()");
    c.run("assert(JAC_PreparedMission548.CaptureUnknown('PREFLIGHT',{error='FindID alias'})=='UNKNOWN_UI')");
    c.core(mission::UNKNOWN_CORE);
    c.run("local p=PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION; assert(p.stage=='UNKNOWN_CORE' and p.before==nil and p.unknown.raw.initialRead.ok==true and p.unknown.raw.initialRead.raw.archive.histories[1].turn==23 and #REQUESTS==0 and BASELINE_PROTECTED==PROTECTED_SNAPSHOT()); JAC_PreparedMission548.InvalidateOnLoad(); assert(JAC_PreparedMission548.Resume()=='UNKNOWN_CORE')");
    c.fails("JAC_PreparedMission548.ArmAfterSave('MISSION')");
    c.fails("JAC_PreparedMission548.SendOnce()");
}

#[test]
fn unknown_preserves_the_last_raw_success_witness_without_rewriting_it() {
    let c = Case::new();
    c.intent();
    c.run("JAC_PreparedMission548.ArmAfterSave('MISSION'); JAC_PreparedMission548.SendOnce(); COMPLETE(0,6); JAC_PreparedMission548.RouteEvent(0,4); assert(JAC_PreparedMission548.Result()=='SUCCEEDED'); JAC_PreparedMission548.CaptureUnknown('CORE_RECEIPT',{error='mismatch'})");
    c.core(mission::UNKNOWN_CORE);
    c.run("local raw=PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION.unknown.previousWitness; assert(raw.phase=='RESULT' and raw.observation.event.id==4 and raw.observation.stopBaseline.merit==10 and BASELINE_PROTECTED==PROTECTED_SNAPSHOT())");
    c.run("JAC_PreparedMission548.CaptureUnknown('SECOND_READ',{error='later failure'})");
    c.core(mission::UNKNOWN_CORE);
    c.run("local p=PROPERTIES.JAC_BOUNDARY_PREPARED_MISSION; assert(p.firstUnknown.step=='CORE_RECEIPT' and p.unknown.step=='SECOND_READ' and p.unknown.previousWitness.phase=='RESULT' and p.unknown.previousWitness.observation.stopBaseline.merit==10)");
}
