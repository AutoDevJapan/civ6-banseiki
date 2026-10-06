//! Lua 5.1 model of the saved property shape and the official API boundary.
//! These fixtures cannot establish that the captive was returned in Civ6.
#[path = "../src/spy_returned_preparation.rs"]
mod preparation;

use mlua::Lua;
#[allow(dead_code)]
#[path = "../examples/returned-preparation-stage.rs"]
mod host;

const FIXTURE: &str = r#"
_G=nil;
PROPERTIES={}; CONTEXT='UI'; TURN=62; TIME=10; REQUESTS={}; WRITES={};
CAPTURED=2; MAP_COUNT=1; ACTIVE_COUNT=0; UNIT_ID=700; UNIT_X=140; UNIT_Y=40;
UNIT_OPERATION=-1; UNIT_END=-1; SOURCE_TURNS=0; NEW_MISSION=0;
BAD_FIND=false; CORE_SOURCE_ABI='missing'; CAN_TRAVEL=true; CAN_SOURCES=true;
FAIL_REQUEST=false; CORE_MISSION_ERROR=false;
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
PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62={stage='RETURNED',turn=62,nonce='exchange:62',
 finalIntent={sessionID=7},before=before,
 returnEvidence={stage='RETURNED_MAP',sessionID=7,before=before,after=after,
  returned={id=700,owner=0,unitType=18,x=140,y=40,turn=62}}};
local function serialize(v)
 if type(v)~='table' then return type(v)..':'..tostring(v) end
 local keys={}; for k in pairs(v) do keys[#keys+1]=k end
 table.sort(keys,function(a,b) return tostring(a)<tostring(b) end);
 local out={}; for _,k in ipairs(keys) do
  out[#out+1]=serialize(k)..'='..serialize(v[k])
 end
 return '{'..table.concat(out,',')..'}'
end
function PROTECTED_SNAPSHOT()
 return serialize({exchange=PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62,
  archive=PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE,
  state=PROPERTIES.JAC_BOUNDARY_THIRD_SPY_STATE,
  ack=PROPERTIES.JAC_BOUNDARY_THIRD_MISSION_ACK,
  event=PROPERTIES.JAC_BOUNDARY_ENEMY_MISSION})
end
function SAVE_FAILURE_SNAPSHOT()
 return serialize({intent=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION,
  initial=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION_UNKNOWN})
end
BASELINE_PROTECTED=PROTECTED_SNAPSHOT();
Game={}; function Game:GetProperty(k) return PROPERTIES[k] end
function Game:SetProperty(k,v)
 assert(CONTEXT=='CORE','UI attempted Game:SetProperty');
 assert(k=='JAC_BOUNDARY_RETURNED_PREPARATION' or
  k=='JAC_BOUNDARY_RETURNED_PREPARATION_UNKNOWN','foreign property write');
 WRITES[#WRITES+1]=k; PROPERTIES[k]=v
end
function Game.GetCurrentGameTurn() return TURN end
function Game.GetLocalPlayer() return 0 end
UI={};
function UI.GetElapsedTime() return TIME end
function UI.DeselectAllUnits() SELECTED=nil end
function UI.DeselectAllCities() end
function UI.SelectUnit(u) SELECTED=u end
function UI.GetHeadSelectedUnit() return SELECTED end
function UI.SetInterfaceMode(mode) MODE=mode end
function UI.GetInterfaceMode() return MODE end
InterfaceModeTypes={SPY_TRAVEL_TO_CITY=1,SPY_CHOOSE_MISSION=2,SELECTION=3};
ActivityTypes={ACTIVITY_AWAKE=4};
EspionageResultTypes={SUCCESS_UNDETECTED=0,SUCCESS_MUST_ESCAPE=2,
 FAIL_UNDETECTED=1,FAIL_MUST_ESCAPE=3,KILLED=4,CAPTURED=5};
UnitOperationTypes={SPY_TRAVEL_NEW_CITY=100,PARAM_X=101,PARAM_Y=102};
local unit={};
function unit:GetID() return BAD_FIND and UNIT_ID+1 or UNIT_ID end
function unit:GetOwner() return 0 end
function unit:GetType() return 18 end
function unit:GetX() return UNIT_X end
function unit:GetY() return UNIT_Y end
function unit:GetSpyOperation() return UNIT_OPERATION end
function unit:GetSpyOperationEndTurn() return UNIT_END end
function unit:GetExperience() return {} end
function unit:IsReadyToMove() return true end
local units={};
function units:FindID(id)
 if MAP_COUNT==1 and (id==UNIT_ID or BAD_FIND) then return unit end
 return nil
end
function units:Members()
 local i=0; return function()
  i=i+1; if i<=MAP_COUNT then return i,unit end
 end
end
local d={};
function d:GetMission(_,id)
 if id<=2 then return {Operation=H[id+1].operation,PlotIndex=H[id+1].plot,
  CompletionTurn=H[id+1].turn,InitialResult=H[id+1].initial,
  EscapeResult=H[id+1].escape} end
 if id==3 then return NEW_MISSION end
 return 0
end
function d:GetSourceTurnsRemaining(city)
 assert(city:GetID()==20); return SOURCE_TURNS
end
local coreDip={};
function coreDip:GetMission(owner,id)
 if CORE_MISSION_ERROR then error('Core mission getter ABI failure') end
 return d:GetMission(owner,id)
end
function coreDip:GetSourceTurnsRemaining(city)
 if CORE_SOURCE_ABI=='error' then error('unknown core ABI') end
 return d:GetSourceTurnsRemaining(city)
end
local player0={};
function player0:IsHuman() return true end
function player0:IsTurnActive() return true end
function player0:GetUnits() return units end
function player0:GetDiplomacy()
 if CONTEXT=='CORE' then
  if CORE_SOURCE_ABI=='missing' then coreDip.GetSourceTurnsRemaining=nil
  else coreDip.GetSourceTurnsRemaining=function(self,city)
   if CORE_SOURCE_ABI=='error' then error('unknown core ABI') end
   return d:GetSourceTurnsRemaining(city)
  end end
  return coreDip
 end
 return d
end
Players={[0]=player0,[1]={IsHuman=function() return false end}};
local home={};
function home:GetOwner() return 0 end
function home:GetID() return 10 end
local enemy={};
function enemy:GetOwner() return 1 end
function enemy:GetID() return 20 end
function enemy:GetX() return 145 end
function enemy:GetY() return 40 end
CityManager={};
function CityManager.GetCity(owner,id)
 if owner==1 and id==20 then return enemy end
 return nil
end
Map={};
function Map.GetPlot(x,y)
 if y~=40 or (x~=140 and x~=145) then return nil end
 return {GetIndex=function() return x==145 and 8511 or 8000 end,
  GetX=function() return x end,GetY=function() return y end}
end
Cities={};
function Cities.GetPlotPurchaseCity(plot)
 if not plot then return nil end
 return plot:GetIndex()==8511 and enemy or home
end
GameInfo={Units={UNIT_SPY={Index=18},[18]={Spy=true}},
 UnitOperations={UNITOPERATION_SPY_GAIN_SOURCES={Index=60,Hash=600},
  UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR={Index=59,Hash=590}},
 GlobalParameters={ESPIONAGE_BONUS_GAIN_SOURCES={Value='2'}}};
UnitManager={};
function UnitManager.GetActivityType() return ActivityTypes.ACTIVITY_AWAKE end
function UnitManager.CanStartOperation(u,op,plot,args)
 if op==UnitOperationTypes.SPY_TRAVEL_NEW_CITY then
  return CAN_TRAVEL and plot:GetIndex()==8511 and
   args[UnitOperationTypes.PARAM_X]==145 and args[UnitOperationTypes.PARAM_Y]==40,
   {native='travel'}
 end
 if op==600 then return CAN_SOURCES and SOURCE_TURNS==0 and UNIT_OPERATION==-1,
  {native='sources'} end
 if op==590 then return true,{native='comparison'} end
 return false,{native='unknown'}
end
function UnitManager.GetTimeToComplete(op)
 assert(op==60); return 7
end
function UnitManager.GetResultProbability(op)
 if op==60 then return {ESPIONAGE_SUCCESS_UNDETECTED=1} end
 assert(op==59);
 return {ESPIONAGE_SUCCESS_UNDETECTED=SOURCE_TURNS==0 and 0.3 or 0.5,
  ESPIONAGE_SUCCESS_MUST_ESCAPE=0.1}
end
function UnitManager.RequestOperation(u,op,args)
 REQUESTS[#REQUESTS+1]={id=u:GetID(),operation=op,args=args};
 if op==UnitOperationTypes.SPY_TRAVEL_NEW_CITY then
  MAP_COUNT=0; ACTIVE_COUNT=1; UNIT_OPERATION=op; UNIT_END=TURN+2
 elseif op==600 then UNIT_OPERATION=op; UNIT_END=TURN+7
 else error('unowned native request') end
 if FAIL_REQUEST then error('native request result unknown after dispatch') end
end
ExposedMembers={JAC_PersonaBoundary={ReadEnemy=function()
 local map,active,captured={},{},{};
 for i=1,MAP_COUNT do
  map[i]={id=UNIT_ID,owner=0,unitType=18,x=UNIT_X,y=UNIT_Y,level=1,
   operation=UNIT_OPERATION,endTurn=UNIT_END}
 end
 for i=1,ACTIVE_COUNT do
  active[i]={name='Returned',level=1,returnTurn=UNIT_END,x=145,y=40}
 end
 for i=1,CAPTURED do captured[i]={captor=1,name='',nameIndex=-1,level=1} end
 return {attacker=0,owner=1,turn=TURN,
  spyRoster={map=map,activeOffMap=active,captured=captured,
   physicalCount=MAP_COUNT+ACTIVE_COUNT+CAPTURED}}
end}};
function ARRIVE(newID)
 TURN=TURN+2; MAP_COUNT=1; ACTIVE_COUNT=0;
 UNIT_ID=newID; UNIT_X=145; UNIT_Y=40; UNIT_OPERATION=-1; UNIT_END=-1
end
function COMPLETE_SOURCES(operation,plot,initial,sourceTurns)
 TURN=TURN+7; UNIT_OPERATION=-1; UNIT_END=-1;
 SOURCE_TURNS=sourceTurns==nil and 6 or sourceTurns;
 NEW_MISSION={Operation=operation or 60,PlotIndex=plot or 8511,
  CompletionTurn=TURN,InitialResult=initial or 0,EscapeResult=-1}
end
"#;

struct Case {
    lua: Lua,
}

#[test]
fn host_preturn_read_errors_are_saved_before_any_turn_or_retry() {
    for phase in ["travel-tick", "sources-tick"] {
        let c = Case::new();
        c.travel_intent();
        if phase == "travel-tick" {
            c.run("JAC_ReturnedPreparation548.TravelOnce(); JAC_ReturnedPreparation548.Observe()");
        } else {
            c.arrive(700);
            c.sources_intent();
            c.run("JAC_ReturnedPreparation548.SourcesOnce()");
        }
        c.run("ExposedMembers.JAC_PersonaBoundary.ReturnedPreparation=JAC_ReturnedPreparation548; SAVES=0; local d=Players[0]:GetDiplomacy(); d.GetMission=function() error('preturn native history read failure') end");
        let scenario =
            host::scenario("JAC-Enemy-Neutralization-newest-physical-checkpoint", phase).unwrap();
        let steps = scenario["steps"].as_array().unwrap();
        let start = steps
            .iter()
            .position(|s| {
                s["code"]
                    .as_str()
                    .is_some_and(|s| s.contains("ExposedMembers.JAC_ReturnedHostTurnReady=false"))
            })
            .unwrap();
        for step in &steps[start..start + 4] {
            match step["action"].as_str().unwrap() {
    "ui_lua"=>{c.run(if step["state_name"]=="GameCore_Tuner" {"CONTEXT='CORE'"}else{"CONTEXT='UI'"}); c.run(step["code"].as_str().unwrap());},
    "save"=>c.run("SAVES=SAVES+1; SAVED_UNKNOWN=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.unknowns[1]"),
    other=>panic!("operation before persisted unknown: {other}"),
   }
        }
        c.fail(steps[start + 4]["code"].as_str().unwrap());
        c.run("assert(SAVES==1 and SAVED_UNKNOWN and SAVED_UNKNOWN.raw.error:find('preturn native history'))");
    }
}

#[test]
fn host_stages_resume_each_turn_without_resending_and_record_native_success_or_failure() {
    for failed in [false, true] {
        let lua = Lua::new();
        lua.load(FIXTURE).exec().unwrap();
        lua.load("PROPERTIES.JAC_BOUNDARY_ENEMY_CONFIG={owner=1,cityID=20}; SAVE_STAGE=nil; local send=UnitManager.RequestOperation; UnitManager.RequestOperation=function(u,op,args) local p=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION; assert(p and SAVE_STAGE==p.stage,'native send before physical host save'); return send(u,op,args) end").exec().unwrap();
        let mut phases = vec![
            "travel-start",
            "travel-tick",
            "travel-tick",
            "sources-start",
        ];
        phases.extend(std::iter::repeat_n("sources-tick", 7));
        phases.push("sources-observe");
        let mut turns = 0;
        for phase in phases {
            let scenario = host::scenario("JAC-Enemy-Neutralization-modeled-save", phase).unwrap();
            for (index, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
                match step["action"].as_str().unwrap() {
                    "load" => {
                        lua.load("CONTEXT='UI'; JAC_ReturnedPreparation548=nil; ExposedMembers.JAC_ReturnedPreparation548Witness=nil; ExposedMembers.JAC_ReturnedPreparationCoreFailure=nil").exec().unwrap();
                        lua.load(preparation::UI_SETUP).exec().unwrap();
                        lua.load("ExposedMembers.JAC_PersonaBoundary.ReturnedPreparation=JAC_ReturnedPreparation548").exec().unwrap();
                    }
                    "save" => lua
                        .load("local p=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION; SAVE_STAGE=p and p.stage")
                        .exec()
                        .unwrap(),
                    "ui_lua" => {
                        lua.globals()
                            .set(
                                "CONTEXT",
                                if step["state_name"] == "GameCore_Tuner" {
                                    "CORE"
                                } else {
                                    "UI"
                                },
                            )
                            .unwrap();
                        lua.load(step["code"].as_str().unwrap())
                            .exec()
                            .unwrap_or_else(|e| panic!("{phase} step{index}: {e}"));
                    }
                    "wait_for" => assert!(
                        lua.load(step["condition"]["code"].as_str().unwrap())
                            .eval::<bool>()
                            .unwrap()
                    ),
                    "end_turn" => {
                        turns += 1;
                        lua.load(format!("TURN=TURN+1; CONTEXT='UI'; if TURN>=UNIT_END and UNIT_END>=0 then if ACTIVE_COUNT==1 then ACTIVE_COUNT=0; MAP_COUNT=1; UNIT_ID=701; UNIT_X=145; UNIT_Y=40; else SOURCE_TURNS={}; NEW_MISSION={{Operation=60,PlotIndex=8511,CompletionTurn=TURN,InitialResult={},EscapeResult=-1}} end; UNIT_OPERATION=-1; UNIT_END=-1 end",if failed {0}else{6},if failed {3}else{0})).exec().unwrap();
                    }
                    "exit_to_menu" => {}
                    other => panic!("unmodeled action {other}"),
                }
            }
        }
        assert_eq!(turns, 9);
        lua.load(format!("assert(#REQUESTS==2); local p=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION; assert(p.stage=='SOURCES_RECEIPT' and p.outcome=='{}'); assert(H[1].turn==23 and H[2].turn==45 and H[3].turn==62)",if failed {"SOURCES_FAILED"} else {"SOURCES_COMPLETE"})).exec().unwrap();
    }
}

impl Case {
    fn new() -> Self {
        let lua = Lua::new();
        lua.load(FIXTURE).exec().unwrap();
        lua.load(preparation::UI_SETUP).exec().unwrap();
        Self { lua }
    }

    fn run(&self, source: &str) {
        self.lua
            .load(source)
            .exec()
            .unwrap_or_else(|e| panic!("{source}: {e}"));
    }

    fn fail(&self, source: &str) {
        assert!(self.lua.load(source).exec().is_err(), "accepted {source}");
    }

    fn core(&self, source: &str) {
        self.run("CONTEXT='CORE'");
        self.lua
            .load(source)
            .exec()
            .unwrap_or_else(|e| panic!("Core script: {e}"));
        self.run("CONTEXT='UI'");
    }

    fn reload(&self) -> String {
        self.run(
            "JAC_ReturnedPreparation548=nil; ExposedMembers.JAC_ReturnedPreparation548Witness=nil",
        );
        self.lua.load(preparation::UI_SETUP).exec().unwrap();
        self.lua
            .load(preparation::UI_RESUME_IF_SAVED)
            .eval::<String>()
            .unwrap()
    }

    fn travel_intent(&self) {
        self.run("assert(JAC_ReturnedPreparation548.Setup()=='RETURNED_READY')");
        self.run("assert(JAC_ReturnedPreparation548.PrepareTravel(1,20)=='TRAVEL_READY')");
        self.core(preparation::TRAVEL_INTENT_CORE);
        self.run("assert(JAC_ReturnedPreparation548.ArmAfterSave('TRAVEL')=='TRAVEL_ARMED')");
    }

    fn arrive(&self, new_id: i64) {
        self.run("assert(JAC_ReturnedPreparation548.TravelOnce()=='TRAVEL_SENT')");
        self.run("assert(JAC_ReturnedPreparation548.Observe().stage=='IN_TRANSIT')");
        self.run(&format!("ARRIVE({new_id})"));
        self.run("assert(JAC_ReturnedPreparation548.Observe().stage=='ARRIVED')");
        self.core(preparation::TRAVEL_RECEIPT_CORE);
    }

    fn sources_intent(&self) {
        self.run("local q=JAC_ReturnedPreparation548.ReadySources(); assert(q.sourceTurns==0 and q.duration==7 and q.bonusParameter=='2' and q.comparisonProbability.success==0.4)");
        self.core(preparation::SOURCES_INTENT_CORE);
        self.run("assert(JAC_ReturnedPreparation548.ArmAfterSave('SOURCES')=='SOURCES_ARMED')");
    }
}

#[test]
fn returned_travel_sources_and_native_receipt() {
    let c = Case::new();
    c.travel_intent();
    c.run("assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.coreAvailability.experience=='function' and PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.coreAvailability.level=='nil')");
    c.arrive(701);
    c.sources_intent();
    c.run("assert(JAC_ReturnedPreparation548.SourcesOnce()=='SOURCES_SENT' and #REQUESTS==2 and REQUESTS[1].operation==100 and REQUESTS[2].operation==600)");
    c.run("assert(JAC_ReturnedPreparation548.OwnedAuxiliaryEvent(0,2)==false)");
    c.run("COMPLETE_SOURCES(); assert(JAC_ReturnedPreparation548.OwnedAuxiliaryEvent(0,3)==true)");
    c.run("assert(JAC_ReturnedPreparation548.OwnedAuxiliaryEvent(0,3)==true)");
    c.run("local r=JAC_ReturnedPreparation548.SourceResult(); assert(r.event.id==3 and r.event.spyID==701 and r.after.sourceTurns==6 and r.comparison.before==0.4 and r.comparison.after==0.6 and r.comparison.delta>0)");
    c.core(preparation::SOURCES_RECEIPT_CORE);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.stage=='SOURCES_RECEIPT' and #WRITES==4 and PROTECTED_SNAPSHOT()==BASELINE_PROTECTED)");
}

#[test]
fn unproven_return_and_wrong_native_id_refuse_all_sends() {
    for change in [
        "PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage='RETURN_UNRESOLVED'",
        "PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.returnEvidence.stage='RETURNED_OFF_MAP_ID_UNKNOWN'",
        "PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.returnEvidence.after.map={}",
        "PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.returnEvidence.after.captured={{},{},{}}",
        "BAD_FIND=true",
    ] {
        let c = Case::new();
        c.run(change);
        c.fail("JAC_ReturnedPreparation548.Setup()");
        c.run("assert(#REQUESTS==0 and #WRITES==0)");
    }
}

#[test]
fn travel_destination_history_and_reload_are_fenced() {
    let c = Case::new();
    c.run("JAC_ReturnedPreparation548.Setup()");
    c.fail("JAC_ReturnedPreparation548.PrepareTravel(0,20)");
    c.fail("JAC_ReturnedPreparation548.PrepareTravel(1,21)");
    c.run("CAN_TRAVEL=false");
    c.fail("JAC_ReturnedPreparation548.PrepareTravel(1,20)");
    c.run("CAN_TRAVEL=true; JAC_ReturnedPreparation548.PrepareTravel(1,20)");
    c.core(preparation::TRAVEL_INTENT_CORE);
    c.fail("JAC_ReturnedPreparation548.TravelOnce()");
    c.run("JAC_ReturnedPreparation548.ArmAfterSave('TRAVEL'); JAC_ReturnedPreparation548.TravelOnce()");
    c.fail("JAC_ReturnedPreparation548.TravelOnce()");
    c.run("JAC_ReturnedPreparation548=nil");
    c.lua.load(preparation::UI_SETUP).exec().unwrap();
    c.fail("JAC_ReturnedPreparation548.Setup()");
    c.run("assert(#REQUESTS==1)");

    let c = Case::new();
    c.run("H[2].turn=44");
    c.fail("JAC_ReturnedPreparation548.Setup()");
    let c = Case::new();
    c.run("NEW_MISSION={Operation=59,PlotIndex=8511,CompletionTurn=62,InitialResult=1,EscapeResult=-1}");
    c.fail("JAC_ReturnedPreparation548.PrepareTravel(1,20)");
}

#[test]
fn changed_id_requires_observed_transit_and_exact_findid() {
    let c = Case::new();
    c.travel_intent();
    c.run("JAC_ReturnedPreparation548.TravelOnce(); ARRIVE(701)");
    c.fail("JAC_ReturnedPreparation548.Observe()");
    c.run("assert(#REQUESTS==1)");

    let c = Case::new();
    c.travel_intent();
    c.run("JAC_ReturnedPreparation548.TravelOnce(); JAC_ReturnedPreparation548.Observe(); ARRIVE(701); BAD_FIND=true");
    c.fail("JAC_ReturnedPreparation548.Observe()");
    c.run("assert(#REQUESTS==1)");
}

#[test]
fn sources_event_never_claims_neutralization_or_wrong_target() {
    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    c.run("JAC_ReturnedPreparation548.SourcesOnce(); COMPLETE_SOURCES(59,8511)");
    c.run("assert(JAC_ReturnedPreparation548.OwnedAuxiliaryEvent(0,3)==false and JAC_ReturnedPreparation548.OwnedAuxiliaryEvent(0,2)==false)");
    c.fail("JAC_ReturnedPreparation548.SourceResult()");

    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    c.run("JAC_ReturnedPreparation548.SourcesOnce(); COMPLETE_SOURCES(60,8000)");
    c.run("assert(JAC_ReturnedPreparation548.OwnedAuxiliaryEvent(0,3)==false)");

    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    c.run("JAC_ReturnedPreparation548.SourcesOnce(); COMPLETE_SOURCES(60,8511,1,0)");
    c.run("assert(JAC_ReturnedPreparation548.OwnedAuxiliaryEvent(0,3)==true and JAC_ReturnedPreparation548.SourceResult().stage=='SOURCES_FAILED')");
    c.core(preparation::SOURCES_RECEIPT_CORE);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.outcome=='SOURCES_FAILED')");

    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    c.run("JAC_ReturnedPreparation548.SourcesOnce(); COMPLETE_SOURCES(); SOURCE_TURNS=0");
    c.run("assert(JAC_ReturnedPreparation548.OwnedAuxiliaryEvent(0,3)==true)");
    c.run("assert(JAC_ReturnedPreparation548.SourceResult().stage=='SOURCES_UNKNOWN')");
    c.core(preparation::SOURCES_RECEIPT_CORE);
}

#[test]
fn unknown_native_result_is_owned_but_never_counted_as_success() {
    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    c.run("JAC_ReturnedPreparation548.SourcesOnce(); COMPLETE_SOURCES(60,8511,99,0)");
    c.run("assert(JAC_ReturnedPreparation548.OwnedAuxiliaryEvent(0,3)==true)");
    c.run("local result=JAC_ReturnedPreparation548.SourceResult(); assert(result.stage=='SOURCES_UNKNOWN' and result.event.initial==99 and result.event.raw.InitialResult==99 and result.effectState=='ZERO' and result.comparison==nil)");
    c.core(preparation::SOURCES_RECEIPT_CORE);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.outcome=='SOURCES_UNKNOWN' and PROTECTED_SNAPSHOT()==BASELINE_PROTECTED)");
}

#[test]
fn destination_drift_and_unsaved_sources_send_refuse() {
    let c = Case::new();
    c.travel_intent();
    c.run("JAC_ReturnedPreparation548.TravelOnce(); ACTIVE_COUNT=1; MAP_COUNT=0; local old=ExposedMembers.JAC_PersonaBoundary.ReadEnemy; ExposedMembers.JAC_PersonaBoundary.ReadEnemy=function() local s=old(); s.spyRoster.activeOffMap[1].x=140; return s end");
    c.fail("JAC_ReturnedPreparation548.Observe()");
    c.run("assert(#REQUESTS==1)");

    let c = Case::new();
    c.travel_intent();
    c.run("JAC_ReturnedPreparation548.TravelOnce(); JAC_ReturnedPreparation548.Observe(); MAP_COUNT=1; ACTIVE_COUNT=0; UNIT_X=145; UNIT_Y=40; UNIT_OPERATION=-1; UNIT_END=-1");
    c.fail("JAC_ReturnedPreparation548.Observe()");

    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.run("JAC_ReturnedPreparation548.ReadySources()");
    c.fail("JAC_ReturnedPreparation548.SourcesOnce()");
    c.core(preparation::SOURCES_INTENT_CORE);
    c.run("JAC_ReturnedPreparation548.ArmAfterSave('SOURCES'); JAC_ReturnedPreparation548.SourcesOnce(); JAC_ReturnedPreparation548=nil");
    c.lua.load(preparation::UI_SETUP).exec().unwrap();
    c.fail("JAC_ReturnedPreparation548.Setup()");
    c.run("assert(#REQUESTS==2 and PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.stage=='SOURCES_INTENT')");
}

#[test]
fn old_history_change_after_owned_sources_event_blocks_receipt() {
    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    c.run("JAC_ReturnedPreparation548.SourcesOnce(); COMPLETE_SOURCES(); assert(JAC_ReturnedPreparation548.OwnedAuxiliaryEvent(0,3)); JAC_ReturnedPreparation548.SourceResult(); H[1].turn=24; CONTEXT='CORE'");
    assert!(c
        .lua
        .load(preparation::SOURCES_RECEIPT_CORE)
        .exec()
        .is_err());
    c.run("CONTEXT='UI'; assert(#WRITES==3 and PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.stage=='SOURCES_INTENT')");
}

#[test]
fn core_unknown_getter_is_recorded_without_mocking_it() {
    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    c.run("assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.sourcesCore.sourceTurns=='nil')");

    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.run("CORE_SOURCE_ABI='error'");
    c.run("JAC_ReturnedPreparation548.ReadySources(); CONTEXT='CORE'");
    c.lua.load(preparation::SOURCES_INTENT_CORE).exec().unwrap();
    c.run("CONTEXT='UI'; assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.sourcesCore.sourceTurnsCall.ok==false and string.find(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.sourcesCore.sourceTurnsCall.value,'unknown core ABI',1,true))");
}

#[test]
fn transit_checkpoint_and_resumed_first_sources_send() {
    let c = Case::new();
    c.travel_intent();
    c.run("JAC_ReturnedPreparation548.TravelOnce(); assert(JAC_ReturnedPreparation548.Observe().stage=='IN_TRANSIT')");
    c.core(preparation::TRAVEL_CHECKPOINT_CORE);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.transit.x==145 and #REQUESTS==1)");
    assert_eq!(c.reload(), "IN_TRANSIT");
    c.fail("JAC_ReturnedPreparation548.ArmAfterSave('TRAVEL')");
    c.fail("JAC_ReturnedPreparation548.TravelOnce()");
    c.run("ARRIVE(701); assert(JAC_ReturnedPreparation548.Observe().stage=='ARRIVED')");
    c.core(preparation::TRAVEL_RECEIPT_CORE);
    assert_eq!(c.reload(), "TRAVEL_RECEIPT");
    c.fail("JAC_ReturnedPreparation548.TravelOnce()");
    c.fail("JAC_ReturnedPreparation548.SourcesOnce()");
    c.run("assert(JAC_ReturnedPreparation548.ReadySources().spy.id==701)");
    c.fail("JAC_ReturnedPreparation548.SourcesOnce()");
    c.core(preparation::SOURCES_INTENT_CORE);
    c.run("JAC_ReturnedPreparation548.ArmAfterSave('SOURCES'); assert(JAC_ReturnedPreparation548.SourcesOnce()=='SOURCES_SENT')");
    c.fail("JAC_ReturnedPreparation548.SourcesOnce()");
    c.run("assert(#REQUESTS==2 and REQUESTS[2].id==701)");
    assert_eq!(c.reload(), "SOURCES_OBSERVED");
    c.fail("JAC_ReturnedPreparation548.ArmAfterSave('SOURCES')");
    c.fail("JAC_ReturnedPreparation548.SourcesOnce()");
    c.run("COMPLETE_SOURCES(60,8511,1,0); CAPTURED=3; MAP_COUNT=0");
    c.run("assert(JAC_ReturnedPreparation548.OwnedAuxiliaryEvent(0,3)==true)");
    c.run("local result=JAC_ReturnedPreparation548.SourceResult(); assert(result.stage=='SOURCES_FAILED' and result.after.spy==nil and result.event.spyID==701)");
    c.core(preparation::SOURCES_RECEIPT_CORE);
    assert_eq!(c.reload(), "SOURCES_RECEIPT");
    c.run("assert(#REQUESTS==2 and PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.outcome=='SOURCES_FAILED' and PROTECTED_SNAPSHOT()==BASELINE_PROTECTED)");
}

#[test]
fn intents_alone_never_rearm_after_reload() {
    let c = Case::new();
    c.travel_intent();
    assert_eq!(c.reload(), "TRAVEL_UNCONFIRMED");
    c.fail("JAC_ReturnedPreparation548.ArmAfterSave('TRAVEL')");
    c.fail("JAC_ReturnedPreparation548.TravelOnce()");
    c.run("assert(#REQUESTS==0)");

    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    assert_eq!(c.reload(), "SOURCES_UNCONFIRMED");
    c.fail("JAC_ReturnedPreparation548.ArmAfterSave('SOURCES')");
    c.fail("JAC_ReturnedPreparation548.SourcesOnce()");
    c.run("assert(JAC_ReturnedPreparation548.OwnedAuxiliaryEvent(0,3)==false and #REQUESTS==1)");
}

#[test]
fn early_gate_owns_failed_sources_only_and_preserves_unknown_raw() {
    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    c.run("JAC_ReturnedPreparation548.SourcesOnce(); COMPLETE_SOURCES(60,8511,1,0)");
    c.run("OLD_HANDLER={}");
    c.run(&format!(
        "function ON_MISSION(attacker,missionID) {} OLD_HANDLER[#OLD_HANDLER+1]={{attacker=attacker,id=missionID}} end",
        preparation::UI_EARLY_EVENT_GATE
    ));
    c.run("ON_MISSION(0,0); ON_MISSION(0,1); ON_MISSION(1,3); ON_MISSION(0,2); assert(#OLD_HANDLER==4)");
    c.run("ON_MISSION(0,3); assert(#OLD_HANDLER==4 and JAC_ReturnedPreparation548.SourceResult().stage=='SOURCES_FAILED')");
    c.core(preparation::SOURCES_RECEIPT_CORE);
    c.run("JAC_ReturnedPreparation548.CaptureUnknown('POST_SAVE',{message='native ABI unclear',rawCode=17})");
    c.core(preparation::UNKNOWN_CORE);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.unknowns[1].raw.rawCode==17 and PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.outcome=='SOURCES_FAILED' and #OLD_HANDLER==4)");
}

#[test]
fn same_context_load_invalidates_arms_and_unreadable_owned_event_never_reaches_old_handler() {
    let c = Case::new();
    c.travel_intent();
    c.run("JAC_ReturnedPreparation548.TravelOnce(); JAC_ReturnedPreparation548.Observe()");
    c.core(preparation::TRAVEL_CHECKPOINT_CORE);
    c.run("local p=JAC_ReturnedPreparation548; p.InvalidateOnLoad(); assert(not p.HasState()); assert(p.Resume()=='IN_TRANSIT')");
    c.fail("JAC_ReturnedPreparation548.ArmAfterSave('TRAVEL')");
    c.run("ARRIVE(701); JAC_ReturnedPreparation548.Observe()");
    c.core(preparation::TRAVEL_RECEIPT_CORE);
    c.sources_intent();
    c.run("JAC_ReturnedPreparation548.SourcesOnce(); JAC_ReturnedPreparation548.InvalidateOnLoad(); assert(JAC_ReturnedPreparation548.Resume()=='SOURCES_OBSERVED')");
    c.fail("JAC_ReturnedPreparation548.ArmAfterSave('SOURCES')");
    c.run("COMPLETE_SOURCES(); local d=Players[0]:GetDiplomacy(); d.GetMission=function() error('native history unreadable') end; OLD_HANDLER=0");
    c.run(&format!(
        "function ON_MISSION(attacker,missionID) {} OLD_HANDLER=OLD_HANDLER+1 end",
        preparation::UI_EARLY_EVENT_GATE
    ));
    c.run("ON_MISSION(0,3); assert(OLD_HANDLER==0 and ExposedMembers.JAC_ReturnedPreparation548Witness.phase=='UNKNOWN')");
    c.core(preparation::UNKNOWN_CORE);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.unknowns[1].step=='EVENT_GATE'); assert(#REQUESTS==2 and PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.outcome==nil)");
}

#[test]
fn resumed_event_gate_checks_native_history_and_actual_id() {
    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    c.run("JAC_ReturnedPreparation548.SourcesOnce()");
    c.run("JAC_ReturnedPreparation548=nil");
    c.lua.load(preparation::UI_SETUP).exec().unwrap();
    c.run("COMPLETE_SOURCES(60,8511,1,0); CAPTURED=3; MAP_COUNT=0; OLD_HANDLER={}");
    c.run(&format!(
        "function ON_MISSION(attacker,missionID) {} OLD_HANDLER[#OLD_HANDLER+1]=missionID end",
        preparation::UI_EARLY_EVENT_GATE
    ));
    c.run("ON_MISSION(0,3); assert(#OLD_HANDLER==0 and JAC_ReturnedPreparation548.HasState())");

    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.run("BAD_FIND=true");
    c.run("JAC_ReturnedPreparation548=nil");
    c.lua.load(preparation::UI_SETUP).exec().unwrap();
    c.fail("JAC_ReturnedPreparation548.Resume()");

    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    c.run("JAC_ReturnedPreparation548.SourcesOnce(); COMPLETE_SOURCES(); BAD_FIND=true; JAC_ReturnedPreparation548=nil");
    c.lua.load(preparation::UI_SETUP).exec().unwrap();
    c.fail("JAC_ReturnedPreparation548.Resume()");
    c.run("assert(#REQUESTS==2)");
}

#[test]
fn ambiguous_native_dispatch_is_saved_unknown_and_never_retried() {
    let c = Case::new();
    c.travel_intent();
    c.run("FAIL_REQUEST=true");
    c.fail("JAC_ReturnedPreparation548.TravelOnce()");
    c.run("JAC_ReturnedPreparation548.CaptureUnknown('TRAVEL_SEND',{error='native request result unknown after dispatch',operation=100})");
    c.core(preparation::UNKNOWN_CORE);
    c.run("assert(#REQUESTS==1 and PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.unknowns[1].raw.operation==100)");
    assert_eq!(c.reload(), "IN_TRANSIT");
    c.fail("JAC_ReturnedPreparation548.TravelOnce()");
    c.run("assert(#REQUESTS==1)");

    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    c.run("FAIL_REQUEST=true");
    c.fail("JAC_ReturnedPreparation548.SourcesOnce()");
    c.run("JAC_ReturnedPreparation548.CaptureUnknown('SOURCES_SEND',{error='native request result unknown after dispatch',operation=600})");
    c.core(preparation::UNKNOWN_CORE);
    assert_eq!(c.reload(), "SOURCES_OBSERVED");
    c.fail("JAC_ReturnedPreparation548.SourcesOnce()");
    c.run("assert(#REQUESTS==2 and PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.unknowns[1].raw.operation==600)");
}

#[test]
fn core_observation_error_keeps_raw_and_saved_intent() {
    let c = Case::new();
    c.travel_intent();
    c.run("JAC_ReturnedPreparation548.TravelOnce(); JAC_ReturnedPreparation548.Observe(); ARRIVE(701); JAC_ReturnedPreparation548.Observe(); CORE_MISSION_ERROR=true; CONTEXT='CORE'");
    assert!(c.lua.load(preparation::TRAVEL_RECEIPT_CORE).exec().is_err());
    c.run("CONTEXT='UI'; JAC_ReturnedPreparation548.CaptureUnknown('TRAVEL_CORE_RECEIPT',{error='Core mission getter ABI failure',spyID=701})");
    c.core(preparation::UNKNOWN_CORE);
    c.run("assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.stage=='TRAVEL_INTENT' and PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.unknowns[1].raw.spyID==701 and PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE.histories[1]==H[1] and PROPERTIES.JAC_BOUNDARY_THIRD_MISSION_ACK.observed.mission==H[3])");
}

// Execute the generated host through its terminal guard. Saving copies the
// evidence here; no tests manufacture an intent or native result to recover.
fn host_failure(c: &Case, phase: &str) -> usize {
    c.run("ExposedMembers.JAC_PersonaBoundary.ReturnedPreparation=JAC_ReturnedPreparation548; PROPERTIES.JAC_BOUNDARY_ENEMY_CONFIG={owner=1,cityID=20}");
    let scenario = host::scenario("JAC-Enemy-Neutralization-failure-checkpoint", phase).unwrap();
    let mut saves = 0;
    for step in scenario["steps"].as_array().unwrap() {
        match step["action"].as_str().unwrap() {
            "load" | "wait_for" => {}
            "ui_lua" => {
                c.run(if step["state_name"] == "GameCore_Tuner" {
                    "CONTEXT='CORE'"
                } else {
                    "CONTEXT='UI'"
                });
                if let Err(error) = c.lua.load(step["code"].as_str().unwrap()).exec() {
                    assert!(
                        error.to_string().contains("saved unresolved preparation"),
                        "{error}"
                    );
                    assert!(saves > 0, "failure escaped before physical checkpoint");
                    c.run("assert(PROTECTED_SNAPSHOT()==BASELINE_PROTECTED)");
                    return saves;
                }
            }
            "save" => {
                assert_eq!(step["checkpoint"], true);
                assert_eq!(step["name"], "JAC-Enemy-Neutralization");
                saves += 1;
                c.run("SAVED_EVIDENCE=SAVE_FAILURE_SNAPSHOT()");
            }
            other => panic!("unexpected action after unknown: {other}"),
        }
    }
    panic!("host did not stop after unknown")
}

#[test]
fn owned_notifications_with_missing_nil_or_scalar_history_are_saved_unknown() {
    for change in [
        "NEW_MISSION=nil",
        "NEW_MISSION=0",
        "NEW_MISSION='unreadable-native-row'",
        "NEW_MISSION=false",
        "Players[0]:GetDiplomacy().GetMission=nil",
        "Players[0]:GetDiplomacy().GetMission=17",
    ] {
        let c = Case::new();
        c.travel_intent();
        c.arrive(700);
        c.sources_intent();
        c.run("JAC_ReturnedPreparation548.SourcesOnce(); COMPLETE_SOURCES()");
        c.run(change);
        c.run("assert(JAC_ReturnedPreparation548.RouteEvent(1,3)=='UNOWNED' and JAC_ReturnedPreparation548.RouteEvent(0,2)=='UNOWNED'); assert(JAC_ReturnedPreparation548.RouteEvent(0,3)=='OWNED_UNKNOWN')");
        c.core(preparation::UNKNOWN_CORE);
        c.run("local p=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION; local u=p.unknowns[1]; assert(p.stage=='SOURCES_INTENT' and p.outcome==nil and p.observerOutcome=='UNKNOWN' and u.nativeRead and u.nativeRead.id==3 and u.nativeRead.owner==0); assert(#REQUESTS==2 and PROTECTED_SNAPSHOT()==BASELINE_PROTECTED)");
        c.fail("JAC_ReturnedPreparation548.ArmAfterSave('SOURCES')");
        c.fail("JAC_ReturnedPreparation548.SourcesOnce()");
        if change.contains("GetMission") {
            c.run("assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.unknowns[1].nativeRead.ok==false)");
        } else {
            c.run("local r=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.unknowns[1].nativeRead; assert(r.ok and r.rawType==type(NEW_MISSION) and r.raw==NEW_MISSION)");
        }
    }
}

#[test]
fn notification_ownership_uses_saved_actor_and_not_native_outcome() {
    for change in ["p.sources.spy.owner=1", "p.sources.target.owner=0"] {
        let c = Case::new();
        c.travel_intent();
        c.arrive(700);
        c.sources_intent();
        c.run(&format!("local p=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION; {change}; NEW_MISSION=nil; assert(JAC_ReturnedPreparation548.RouteEvent(0,3)=='UNOWNED' and ExposedMembers.JAC_ReturnedPreparation548Witness.phase~='UNKNOWN')"));
    }
    let c = Case::new();
    c.travel_intent();
    c.arrive(700);
    c.sources_intent();
    c.run("JAC_ReturnedPreparation548.SourcesOnce(); COMPLETE_SOURCES(59,8000); OLD_HANDLER=0");
    c.run(&format!(
        "function EVENT(attacker,missionID) {} OLD_HANDLER=OLD_HANDLER+1 end",
        preparation::UI_EARLY_EVENT_GATE
    ));
    c.run("EVENT(1,3); EVENT(0,2); assert(OLD_HANDLER==2); EVENT(0,3); assert(OLD_HANDLER==2)");
    c.core(preparation::UNKNOWN_CORE);
    c.run("local u=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION.unknowns[1]; assert(u.nativeRead.raw.Operation==59 and u.nativeRead.raw.PlotIndex==8000 and #REQUESTS==2)");
}

#[test]
fn initial_ui_failure_saves_provenance_without_an_intent_and_stays_terminal() {
    assert_eq!(
        preparation::UNKNOWN_PROPERTY,
        "JAC_BOUNDARY_RETURNED_PREPARATION_UNKNOWN"
    );
    for change in [
        "Players[0]:GetUnits().FindID=nil",
        "BAD_FIND=true",
        "UnitManager.CanStartOperation=nil",
        "Players[0]:GetDiplomacy().GetMission=nil",
    ] {
        let c = Case::new();
        c.run(change);
        assert_eq!(host_failure(&c, "travel-start"), 1);
        c.run("local u=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION_UNKNOWN; assert(u.stage=='INITIAL_UNKNOWN' and u.observerOutcome=='UNKNOWN' and u.observation.step=='TRAVEL_READY' and u.observation.observer.localPlayer==0 and u.observation.initialProvenance.exchange.returnEvidence.returned.id==700 and u.observation.initialProvenance.ack.observed.mission.turn==62); assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION==nil and #REQUESTS==0 and SAVED_EVIDENCE:find('INITIAL_UNKNOWN'))");
        c.fail("JAC_ReturnedPreparation548.Setup()");
        c.fail("JAC_ReturnedPreparation548.ArmAfterSave('TRAVEL')");
        assert_eq!(c.reload(), "INITIAL_UNKNOWN");
        c.fail("JAC_ReturnedPreparation548.Setup()");
        c.fail("JAC_ReturnedPreparation548.ArmAfterSave('SOURCES')");
        c.run("assert(JAC_ReturnedPreparation548.RouteEvent(0,3)=='UNOWNED' and #REQUESTS==0)");
    }
}

#[test]
fn initial_core_failure_preserves_ready_witness_without_inventing_travel() {
    let c = Case::new();
    c.run("CORE_MISSION_ERROR=true");
    assert_eq!(host_failure(&c, "travel-start"), 2);
    c.run("local u=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION_UNKNOWN; local w=u.observation.previousWitness; assert(u.observation.step=='TRAVEL_INTENT' and u.observation.raw.error:find('Core mission getter ABI failure') and w.phase=='TRAVEL_READY' and w.returnedID==700 and w.observation.before.unit.id==700 and w.observation.target.city==20); assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION==nil and #REQUESTS==0 and SAVED_EVIDENCE:find('TRAVEL_READY'))");
    c.fail("JAC_ReturnedPreparation548.TravelOnce()");
    assert_eq!(c.reload(), "INITIAL_UNKNOWN");
}

#[test]
fn missing_or_malformed_return_receipt_remains_unknown_raw() {
    for change in [
        "PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62=nil",
        "PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62=17",
        "PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.returnEvidence=nil",
    ] {
        let c = Case::new();
        c.run(change);
        c.run("BASELINE_PROTECTED=PROTECTED_SNAPSHOT()");
        assert_eq!(host_failure(&c, "travel-start"), 1);
        c.run("local u=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION_UNKNOWN; local original=u.observation.initialProvenance.exchange; if type(original)=='table' then assert(original.returnEvidence==nil) else assert(original==PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62) end; assert(u.stage=='INITIAL_UNKNOWN' and u.observation.raw.error and PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION==nil and H[1].turn==23 and H[2].turn==45 and H[3].turn==62 and #REQUESTS==0)");
    }
}

#[test]
fn repeated_unknown_copies_are_flat_and_have_a_finite_persistence_bound() {
    let c = Case::new();
    c.travel_intent();
    c.run("local p=JAC_ReturnedPreparation548; for i=1,32 do p.CaptureUnknown('REPEAT',{marker=17}) end; local w=ExposedMembers.JAC_ReturnedPreparation548Witness; assert(w.observation.previousWitness.phase=='TRAVEL_READY' and w.observation.previousWitness.observation.previousWitness==nil); UNKNOWN_SIZE=#SAVE_FAILURE_SNAPSHOT()");
    for _ in 0..16 {
        c.core(preparation::UNKNOWN_CORE);
        c.run("local p=JAC_ReturnedPreparation548; p.CaptureUnknown('REPEAT',{marker=17}); local w=ExposedMembers.JAC_ReturnedPreparation548Witness; assert(w.observation.previousWitness.phase=='TRAVEL_READY' and w.observation.previousWitness.observation.previousWitness==nil)");
    }
    c.run("local p=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION; assert(#p.unknowns==16); for _,u in ipairs(p.unknowns) do assert(u.previousWitness.phase=='TRAVEL_READY' and u.previousWitness.observation.previousWitness==nil and u.raw.marker==17) end; assert(#REQUESTS==0 and PROTECTED_SNAPSHOT()==BASELINE_PROTECTED)");
    c.run("CONTEXT='CORE'");
    c.fail(preparation::UNKNOWN_CORE);
    c.run("CONTEXT='UI'");
    c.fail("JAC_ReturnedPreparation548.ArmAfterSave('TRAVEL')");
}

#[test]
fn initial_unknown_never_turns_an_old_witness_into_send_permission() {
    let c = Case::new();
    c.run("JAC_ReturnedPreparation548.Setup(); JAC_ReturnedPreparation548.PrepareTravel(1,20); ORIGINAL_READY=ExposedMembers.JAC_ReturnedPreparation548Witness");
    for _ in 0..20 {
        c.run("JAC_ReturnedPreparation548.CaptureUnknown('INITIAL_READ',{marker=19})");
        c.core(preparation::UNKNOWN_CORE);
        c.run("local u=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION_UNKNOWN; assert(u.observation.previousWitness.phase=='TRAVEL_READY' and u.observation.previousWitness.observation.previousWitness==nil and u.observation.raw.marker==19); local size=#SAVE_FAILURE_SNAPSHOT(); if INITIAL_SIZE then assert(size==INITIAL_SIZE) else INITIAL_SIZE=size end; assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION==nil)");
    }
    c.run("ExposedMembers.JAC_ReturnedPreparation548Witness=ORIGINAL_READY; CONTEXT='CORE'");
    c.fail(preparation::TRAVEL_INTENT_CORE);
    c.run("CONTEXT='UI'");
    c.fail("JAC_ReturnedPreparation548.ArmAfterSave('TRAVEL')");
    c.fail("JAC_ReturnedPreparation548.TravelOnce()");
    assert_eq!(c.reload(), "INITIAL_UNKNOWN");
    c.fail("JAC_ReturnedPreparation548.Setup()");
    c.run("assert(PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION==nil and #REQUESTS==0 and PROTECTED_SNAPSHOT()==BASELINE_PROTECTED)");
}

#[test]
fn host_core_failure_keeps_full_arrival_and_sources_result_witnesses() {
    for phase in ["travel-observe", "sources-observe"] {
        let c = Case::new();
        c.travel_intent();
        if phase == "travel-observe" {
            c.run("JAC_ReturnedPreparation548.TravelOnce(); JAC_ReturnedPreparation548.Observe(); ARRIVE(701)");
        } else {
            c.arrive(700);
            c.sources_intent();
            c.run("JAC_ReturnedPreparation548.SourcesOnce(); COMPLETE_SOURCES()");
        }
        c.run("CORE_MISSION_ERROR=true");
        host_failure(&c, phase);
        if phase == "travel-observe" {
            c.run("local p=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION; local w=p.unknowns[1].previousWitness; assert(p.stage=='TRAVEL_INTENT' and not p.arrival and w.phase=='TRAVEL_OBSERVATION' and w.observation.stage=='ARRIVED' and w.observation.unit.id==701 and SAVED_EVIDENCE:find('ARRIVED') and #REQUESTS==1)");
        } else {
            c.run("local p=PROPERTIES.JAC_BOUNDARY_RETURNED_PREPARATION; local w=p.unknowns[1].previousWitness; assert(p.stage=='SOURCES_INTENT' and not p.sourcesReceipt and p.outcome==nil and w.phase=='SOURCES_RESULT' and w.observation.event.raw.InitialResult==0 and w.observation.after.sourceTurns==6 and w.observation.comparison.delta>0 and SAVED_EVIDENCE:find('SOURCES_RESULT') and #REQUESTS==2)");
        }
        c.fail("JAC_ReturnedPreparation548.ArmAfterSave('SOURCES')");
    }
}
