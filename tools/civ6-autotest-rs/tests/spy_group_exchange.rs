//! Modeled native APIs exercise the generated Lua in Lua 5.1. These cases
//! establish script behavior, not a live Civ6 exchange or a specific spy ID.
#[path = "../src/spy_group_exchange.rs"]
mod spy_group_exchange;

use mlua::Lua;

#[allow(dead_code)]
#[path = "../examples/group-exchange-62.rs"]
mod host;

const NATIVE: &str = r#"
-- The actual InGame Tuner environment in native08 has no _G alias.
_G=nil;
TIME=10; PROPERTIES={}; SESSION=nil; CLOSED=0; SENDS={}; NEXT_ID=2;
REPLY_KIND='GOOD'; AUTO_RETURN=true; REPLY_SESSION=7; REPLY_INITIATOR=0;
CAPTIVE_COUNT=3; MAP_COUNT=0; ACTIVE_COUNT=0; GOLD_BALANCE=50;
MAP_ID=77777;
function history(id,turn)
 return {id=id,turn=turn,operation=59,plot=8511,initial=1,escape=-1}
end
H={history(0,23),history(1,45),history(2,62)};
PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE={histories={H[1],H[2]}};
-- spy_third::coreHistories stores the provenance string, not the rows.
PROPERTIES.JAC_BOUNDARY_THIRD_SPY_STATE={missionPost={histories='UI'}};
PROPERTIES.JAC_BOUNDARY_THIRD_MISSION_ACK={histories='UI',observed={mission=H[3],preserved={H[1],H[2]}}};
PROPERTIES.JAC_BOUNDARY_ENEMY_MISSION={attacker=0,id=2,turn=62};
Game={}; function Game:GetProperty(k) return PROPERTIES[k] end
function Game:SetProperty(k,v) PROPERTIES[k]=v end
function Game.GetCurrentGameTurn() return 62 end
function Game.GetLocalPlayer() return 0 end
UI={}; function UI.GetElapsedTime() return TIME end
local function emitter()
 local e={listeners={}};
 function e.Add(fn) e.listeners[#e.listeners+1]=fn end
 function e.fire(...) for _,fn in ipairs(e.listeners) do fn(...) end end
 return e
end
Events={DiplomacyStatement=emitter(),DiplomacyIncomingDeal=emitter()};
ExposedMembers={};
ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey=function() return 'OLD_TOOLTIP' end;
ORIGINAL_TOOLTIP=ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey;
ORIGINAL_UI=UI;
local dip={};
function dip:GetSpyCapacity() return 3 end
function dip:IsAtWarWith() return false end
function dip:GetMission(_,i)
 local h=H[i+1]; return {Operation=h.operation,PlotIndex=h.plot,
  CompletionTurn=h.turn,InitialResult=h.initial,EscapeResult=h.escape}
end
local treasury={}; function treasury:GetGoldBalance() return GOLD_BALANCE end
local units={};
function units:Members()
 local i=0; return function()
  i=i+1; if i<=MAP_COUNT then return i,MAP_UNIT end
 end
end
function units:FindID(id)
 if MAP_COUNT==1 and MAP_UNIT:GetID()==id then return MAP_UNIT end
 return nil
end
MAP_UNIT={}; function MAP_UNIT:GetID() return MAP_ID end
function MAP_UNIT:GetOwner() return 0 end
function MAP_UNIT:GetType() return 18 end
function MAP_UNIT:GetX() return 140 end
function MAP_UNIT:GetY() return 40 end
Players={};
for id=0,1 do
 local playerID=id;
 Players[id]={};
 Players[id].GetDiplomacy=function() return dip end;
 Players[id].GetUnits=function() return units end;
 Players[id].GetTreasury=function() return treasury end;
 Players[id].IsHuman=function() return playerID==0 end;
 Players[id].IsTurnActive=function() return playerID==0 end;
end
GameInfo={Units={UNIT_SPY={Index=18,Name='LOC_UNIT_SPY_NAME'},[18]={Spy=true}}};
Map={}; function Map.GetPlot(x,y)
 if x>=0 and y>=0 then return {x=x,y=y} end
 return nil
end
ExposedMembers.JAC_PersonaBoundary={ReadEnemy=function()
 local captured,offMap,active,map={},{},{},{};
 for i=1,CAPTIVE_COUNT do
  captured[i]={captor=1,name='',level=1,nameIndex=-1};
  offMap[i]={name='',level=1,returnTurn=-1,x=-9999,y=-9999}
 end
 for i=1,ACTIVE_COUNT do
  local row={name='Returned',level=1,returnTurn=63,x=140,y=40};
  active[i]=row; offMap[#offMap+1]=row
 end
 for i=1,MAP_COUNT do
  map[i]={id=MAP_ID,owner=0,unitType=18,x=140,y=40,level=1,name='Returned'}
 end
 return {turn=62,attacker=0,owner=1,
  retry={current=3,spyID=1048586},neutralizationMission=false,
  spy={id=1048586,owner=0,present=MAP_COUNT==1 and MAP_ID==1048586},
  spyRoster={captured=captured,offMap=offMap,map=map,
   activeOffMap=active,physicalCount=CAPTIVE_COUNT+ACTIVE_COUNT+MAP_COUNT},
  missions={['0/2']={attacker=0,id=2,turn=62,operation=59,
   plot=8511,initial=1,escape=-1}}}
end};
DealDirection={OUTGOING=1,INCOMING=2};
DealItemTypes={CAPTIVE=1,GOLD=2,RESOURCES=3};
DealValidationResult={VALID=1};
DealProposalAction={EQUALIZE=1,ACCEPTED=2,ADJUSTED=3,PROPOSED=4,
 REJECTED=5,REJECTED_PERMANENT=6,EQUALIZE_FAILED=7};
local function item(kind,from,to,value,amount,duration)
 local i={id=NEXT_ID,kind=kind,from=from,to=to,value=value,
  amount=amount or 0,duration=duration or 0,locked=false};
 NEXT_ID=NEXT_ID+1;
 function i:GetID() return self.id end
 function i:GetType() return self.kind end
 function i:GetFromPlayerID() return self.from end
 function i:GetToPlayerID() return self.to end
 function i:GetValueType() return self.value end
 function i:SetValueType(v) self.value=v end
 function i:GetAmount() return self.amount end
 function i:GetDuration() return self.duration end
 function i:IsValid() return true end
 function i:IsLocked() return self.locked end
 function i:SetLocked(v) self.locked=v end
 function i:GetValueTypeNameID() return nil end
 return i
end
local function deal()
 local d={items={}};
 function d:GetItemCount() return #self.items end
 function d:Items()
  local j=0; return function()
   j=j+1; return self.items[j]
  end
 end
 function d:FindItemByID(id)
  for _,v in ipairs(self.items) do if v.id==id then return v end end
 end
 function d:AddItemOfType(kind,from)
  local v=item(kind,from,from==1 and 0 or 1,0); self.items[#self.items+1]=v; return v
 end
 function d:RemoveItemByID(id)
  for j,v in ipairs(self.items) do if v.id==id then table.remove(self.items,j); return true end end
  return false
 end
 function d:Validate() return DealValidationResult.VALID end
 return d
end
OUT=deal(); IN=deal();
local function replyItems()
 IN.items={item(DealItemTypes.CAPTIVE,1,0,-1)};
 if REPLY_KIND=='GOOD' or REPLY_KIND=='GOLD_TOO_MUCH' then
  IN.items[#IN.items+1]=item(DealItemTypes.GOLD,0,1,0,
   REPLY_KIND=='GOOD' and 10 or 100,0)
 elseif REPLY_KIND=='UNSAFE' then
  IN.items[#IN.items+1]=item(DealItemTypes.RESOURCES,0,1,4,1,30)
 end
end
DealManager={};
function DealManager.GetWorkingDeal(direction)
 return direction==DealDirection.OUTGOING and OUT or IN
end
function DealManager.HasPendingDeal() return false end
function DealManager.GetPossibleDealItems()
 if #OUT.items>0 then return {} end
 return {{ForType=-1,IsValid=true,ValidationResult=1},
  {ForType=-1,IsValid=true,ValidationResult=1},
  {ForType=-1,IsValid=true,ValidationResult=1}}
end
function DealManager.AreWorkingDealsEqual()
 if #OUT.items~=#IN.items then return false end
 for j,a in ipairs(OUT.items) do
  local b=IN.items[j];
  if a.kind~=b.kind or a.from~=b.from or a.to~=b.to or
   a.value~=b.value or a.amount~=b.amount or a.duration~=b.duration then return false end
 end
 return true
end
function DealManager.CopyIncomingToOutgoingWorkingDeal()
 OUT.items={}; for _,v in ipairs(IN.items) do
  OUT.items[#OUT.items+1]=item(v.kind,v.from,v.to,v.value,v.amount,v.duration)
 end
end
function DealManager.SendWorkingDeal(action,from,to)
 SENDS[#SENDS+1]={action=action,from=from,to=to};
 if action==DealProposalAction.EQUALIZE and REPLY_KIND~='NONE' then
  replyItems();
  if REPLY_KIND=='MISMATCH' then
   OUT.items[#OUT.items+1]=item(DealItemTypes.RESOURCES,0,1,3,1,30)
  end
  local reply=REPLY_KIND=='REFUSED' and DealProposalAction.REJECTED or
   DealProposalAction.ADJUSTED;
  Events.DiplomacyStatement.fire(1,0,{SessionID=REPLY_SESSION,
   Initiator=REPLY_INITIATOR,StatementType=5,StatementSubType=6,
   DealAction=reply,RespondingToDealAction=DealProposalAction.EQUALIZE});
  Events.DiplomacyIncomingDeal.fire(1,0,reply)
 end
 if action==DealProposalAction.ACCEPTED and AUTO_RETURN then
  CAPTIVE_COUNT=2; MAP_COUNT=1
 end
end
DiplomacyManager={};
function DiplomacyManager.FindOpenSessionID() return SESSION end
function DiplomacyManager.GetSessionInfo(id)
 if id==SESSION then return {FromPlayer=0} end
 return nil
end
function DiplomacyManager.GetKeyName(id)
 if id==5 then return 'MAKE_DEAL' end
 return 'OTHER'
end
function DiplomacyManager.RequestSession(from,to,kind)
 assert(from==0 and to==1 and kind=='MAKE_DEAL'); SESSION=7;
 Events.DiplomacyStatement.fire(0,1,{SessionID=SESSION,Initiator=0,
  StatementType=5,StatementSubType=0,DealAction=0,RespondingToDealAction=0})
end
function DiplomacyManager.CloseSession(id)
 assert(id==SESSION); SESSION=nil; CLOSED=CLOSED+1
end
"#;

struct Case(Lua);

#[test]
fn cleanup_keeps_a_stock_copied_item_that_changed_after_observation() {
    let c = Case::new();
    c.run("REPLY_KIND='UNSAFE'");
    c.equalize();
    c.run(
        "DealManager.CopyIncomingToOutgoingWorkingDeal(0,1); JAC_GroupExchange548.PublishQuote()",
    );
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.run("KEPT_ID=OUT.items[1]:GetID(); OUT.items[1].amount=999; JAC_GroupExchange548.Close(); assert(#OUT.items==1 and OUT.items[1]:GetID()==KEPT_ID); local c=JAC_GroupExchange548.Evidence().ui.cleanup; assert(#c.removed==1 and #c.retained==1 and c.retained[1]==KEPT_ID)");
}

#[test]
fn host_executes_success_refusal_timeout_and_unresolved_return_with_saved_intents() {
    for (setup, expected_sends, expected_stage) in [
        ("", 2, "RETURNED"),
        ("REPLY_KIND='NONE'", 1, "QUOTED_UNKNOWN"),
        ("REPLY_KIND='UNSAFE'", 1, "QUOTED_UNKNOWN"),
        ("AUTO_RETURN=false", 2, "RETURN_UNRESOLVED"),
        ("FAULT='READ_REPLY'", 1, "QUOTED_UNKNOWN"),
        ("FAULT='READ_RETURN'", 2, "RETURN_UNRESOLVED"),
        ("FAULT='CORE_ID'", 2, "RETURN_UNRESOLVED"),
        ("FAULT='CORE_API'", 2, "RETURN_UNRESOLVED"),
        ("FAULT='CORE_QUOTE'", 1, "QUOTED_UNKNOWN"),
        ("FAULT='TERMS_FINAL'", 1, "RETURN_UNRESOLVED"),
        ("FAULT='SEND_EQUALIZE'", 0, "QUOTED_UNKNOWN"),
        ("FAULT='SEND_FINAL'", 1, "RETURN_UNRESOLVED"),
        ("FAULT='NON_SCALAR'", 1, "QUOTED_UNKNOWN"),
        ("FAULT='CLOSE_READ'", 2, "RETURNED"),
        (
            "FAULT='STOCK_UNSAFE'; REPLY_KIND='UNSAFE'",
            1,
            "QUOTED_UNKNOWN",
        ),
    ] {
        let lua = Lua::new();
        lua.load(NATIVE).exec().unwrap();
        lua.load(setup).exec().unwrap();
        lua.load("local send=DealManager.SendWorkingDeal; DealManager.SendWorkingDeal=function(action,a,b) assert(SAVED_STAGE==(action==DealProposalAction.EQUALIZE and 'RESERVED' or 'FINAL_INTENT'),'send before host save'); if (FAULT=='SEND_EQUALIZE' and action==DealProposalAction.EQUALIZE) or (FAULT=='SEND_FINAL' and action==DealProposalAction.ACCEPTED) then error('native send failure') end; local result=send(action,a,b); if FAULT=='STOCK_UNSAFE' and action==DealProposalAction.EQUALIZE then DealManager.CopyIncomingToOutgoingWorkingDeal(0,1) end; return result end").exec().unwrap();
        let scenario = host::scenario();
        for (index, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
            lua.load(format!("STEP={index}; if (FAULT=='READ_REPLY' and STEP==11) or (FAULT=='READ_RETURN' and STEP==19) then ExposedMembers.JAC_PersonaBoundary.ReadEnemy=function() error('native roster read failed') end end; if FAULT=='CORE_ID' and STEP==21 then Players[0]:GetUnits().FindID=function() return MAP_UNIT end; MAP_UNIT.GetID=function() return 99999 end end; if FAULT=='CORE_API' and STEP==21 then Players[0]:GetUnits().FindID=nil end; if FAULT=='CORE_QUOTE' and STEP==13 then PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.before.histories[3].turn=61 end; if FAULT=='TERMS_FINAL' and STEP==17 then IN.items[2].amount=20; OUT.items[2].amount=20 end")).exec().unwrap();
            lua.load("if FAULT=='NON_SCALAR' and STEP==11 then OUT.items[1].GetAmount=function() return function() end end end; if FAULT=='CLOSE_READ' and STEP==23 then DealManager.GetWorkingDeal=function() error('native cleanup read failed') end end").exec().unwrap();
            let action = step["action"].as_str().unwrap();
            match action {
                "load" | "exit_to_menu" => {}
                "save" => lua
                    .load("SAVED_STAGE=PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage")
                    .exec()
                    .unwrap(),
                "wait_for" => {
                    let code = step["condition"]["code"].as_str().unwrap();
                    if !lua.load(code).eval::<bool>().unwrap() {
                        lua.load("TIME=TIME+31").exec().unwrap();
                        assert!(
                            lua.load(code).eval::<bool>().unwrap(),
                            "wait {index} did not terminate"
                        );
                    }
                }
                "ui_lua" => {
                    let core = step["state_name"] == "GameCore_Tuner";
                    if core {
                        lua.load("UI_SAVED={UI,DealManager,DiplomacyManager,DealItemTypes,DealProposalAction,DealValidationResult}; UI=nil; DealManager=nil; DiplomacyManager=nil; DealItemTypes=nil; DealProposalAction=nil; DealValidationResult=nil").exec().unwrap();
                    }
                    lua.load(step["code"].as_str().unwrap())
                        .exec()
                        .unwrap_or_else(|e| panic!("step {index}: {e}"));
                    if core {
                        lua.load("UI,DealManager,DiplomacyManager,DealItemTypes,DealProposalAction,DealValidationResult=unpack(UI_SAVED)").exec().unwrap();
                    }
                }
                other => panic!("uncovered host operation {other}"),
            }
        }
        assert_eq!(
            lua.load("return #SENDS").eval::<i64>().unwrap(),
            expected_sends
        );
        assert_eq!(
            lua.load("return PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage")
                .eval::<String>()
                .unwrap(),
            expected_stage
        );
        lua.load("assert(CLOSED==1 and ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey==ORIGINAL_TOOLTIP); assert(H[1].turn==23 and H[2].turn==45 and H[3].turn==62); if FAULT=='NON_SCALAR' then assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.quote.raw.error:find('non%-scalar')) end; if FAULT=='CLOSE_READ' then assert(#JAC_GroupExchange548.Evidence().ui.cleanup.errors==1) end").exec().unwrap();
        lua.load("local p=PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62; assert(SAVED_STAGE==p.stage); if FAULT=='CORE_ID' or FAULT=='CORE_API' or FAULT=='READ_RETURN' or FAULT=='TERMS_FINAL' then assert(p.errors and #p.errors>0 and p.returnEvidence) end; if FAULT=='STOCK_UNSAFE' then assert(#OUT.items==0 and #JAC_GroupExchange548.Evidence().ui.cleanup.removed==2) end").exec().unwrap();
    }
}

#[test]
fn core_rejects_a_recycled_find_id_result() {
    let c = Case::new();
    c.equalize();
    c.run("JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.0.load(spy_group_exchange::FINAL_INTENT_CORE)
        .exec()
        .unwrap();
    c.run("JAC_GroupExchange548.AcceptOnce(); JAC_GroupExchange548.PublishReturn(); local u=Players[0]:GetUnits(); u.FindID=function() return MAP_UNIT end; MAP_UNIT.GetID=function() return 99999 end");
    c.rejects(
        spy_group_exchange::RETURN_CORE,
        "Core returned native spy identity differs",
    );
}

impl Case {
    fn new() -> Self {
        let lua = Lua::new();
        assert_eq!(
            lua.load("return _VERSION").eval::<String>().unwrap(),
            "Lua 5.1"
        );
        lua.load(NATIVE).exec().unwrap();
        lua.load(spy_group_exchange::UI_SETUP).exec().unwrap();
        Self(lua)
    }

    fn run(&self, lua: &str) {
        self.0.load(lua).exec().unwrap();
    }

    fn rejects(&self, lua: &str, reason: &str) {
        let error = self.0.load(lua).exec().expect_err("must fail closed");
        assert!(error.to_string().contains(reason), "{error}");
    }

    fn reserve(&self) {
        self.run("assert(JAC_GroupExchange548.Prepare()=='PREPARED')");
        self.0
            .load(spy_group_exchange::RESERVE_CORE)
            .exec()
            .unwrap();
        self.run("assert(JAC_GroupExchange548.ArmAfterSave()=='ARMED')");
    }

    fn equalize(&self) {
        self.reserve();
        self.run("assert(JAC_GroupExchange548.OpenSession()=='SESSION_REQUESTED')");
        self.run("assert(JAC_GroupExchange548.SessionReady()==true)");
        self.run("assert(JAC_GroupExchange548.EqualizeOnce()=='EQUALIZE_SENT')");
    }
}

#[test]
fn captured_core_backing_unit_is_preserved_but_not_counted_as_an_active_map_spy() {
    let c = Case::new();
    c.run("local units=Players[0]:GetUnits(); units.Members=function() local done=false; return function() if not done then done=true; return 1,MAP_UNIT end end end; MAP_UNIT.GetID=function() return 1048586 end; MAP_UNIT.GetX=function() return -9999 end; MAP_UNIT.GetY=function() return -9999 end");
    c.reserve();
    c.run("local p=PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62; assert(p.stage=='RESERVED'); assert(#p.sources.offMapUnits==1); local u=p.sources.offMapUnits[1]; assert(u.id==1048586 and u.owner==0 and u.x==-9999 and u.y==-9999 and u.classification=='OFF_MAP_SENTINEL' and u.individualCaptiveIdentity=='UNKNOWN'); assert(#SENDS==0)");
}

#[test]
fn unknown_core_position_cannot_make_a_nonempty_spy_inventory_empty() {
    for fault in [
        "MAP_UNIT.GetX=function() return -9999 end",
        "MAP_UNIT.GetX=nil; MAP_UNIT.GetY=nil",
    ] {
        let c = Case::new();
        c.run("assert(JAC_GroupExchange548.Prepare()=='PREPARED'); local units=Players[0]:GetUnits(); units.Members=function() local done=false; return function() if not done then done=true; return 1,MAP_UNIT end end end");
        c.run(fault);
        c.rejects(
            spy_group_exchange::RESERVE_CORE,
            "Core active map spies changed",
        );
        c.run("assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62==nil and #SENDS==0)");
    }
}

#[test]
fn native_reply_quote_saved_intent_one_accept_and_returned_actual_id() {
    let c = Case::new();
    c.reserve();
    c.run(
        "assert(JAC_GroupExchange548.OpenSession()=='SESSION_REQUESTED'); \
         assert(JAC_GroupExchange548.SessionReady()); \
         assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()=='LOC_UNIT_SPY_NAME'); \
         assert(UI==ORIGINAL_UI)",
    );
    c.run("assert(JAC_GroupExchange548.EqualizeOnce()=='EQUALIZE_SENT')");
    c.run("local r=JAC_GroupExchange548.ResponseState(); assert(r.stage=='AI_QUOTE' and r.reply.SessionID==7 and r.reply.RespondingToDealAction==DealProposalAction.EQUALIZE)");
    c.run("assert(JAC_GroupExchange548.PublishQuote()=='AI_QUOTE')");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.0.load(spy_group_exchange::FINAL_INTENT_CORE)
        .exec()
        .unwrap();
    c.run("assert(JAC_GroupExchange548.AcceptOnce()=='FINAL_SENT')");
    c.run("local r=JAC_GroupExchange548.ReturnState(); assert(r.stage=='RETURNED_MAP' and r.returned.id==77777 and #r.after.captured==2 and r.after.physical==3)");
    c.run("assert(JAC_GroupExchange548.PublishReturn()=='RETURNED_MAP')");
    c.0.load(spy_group_exchange::RETURN_CORE).exec().unwrap();
    c.run("assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage=='RETURNED'); assert(#SENDS==2 and SENDS[1].action==DealProposalAction.EQUALIZE and SENDS[2].action==DealProposalAction.ACCEPTED); assert(PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE.histories[1]==H[1] and PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE.histories[2]==H[2] and PROPERTIES.JAC_BOUNDARY_THIRD_MISSION_ACK.observed.mission==H[3] and PROPERTIES.JAC_BOUNDARY_THIRD_SPY_STATE.missionPost.histories=='UI')");
    c.run("assert(JAC_GroupExchange548.Close()=='CLOSED' and CLOSED==1); assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey==ORIGINAL_TOOLTIP)");
}

#[test]
fn unresolved_or_stale_reply_never_retries_or_sends_final() {
    for setup in ["REPLY_KIND='NONE'", "REPLY_SESSION=99", "REPLY_INITIATOR=1"] {
        let c = Case::new();
        c.run(setup);
        c.equalize();
        if setup.contains("NONE") {
            c.run("TIME=TIME+31");
        }
        c.rejects("JAC_GroupExchange548.EqualizeOnce()", "already sent");
        c.run("local r=JAC_GroupExchange548.ResponseState(); assert(r.stage=='UNKNOWN_TIMEOUT' or r.stage=='UNKNOWN_RESPONSE')");
        c.run("JAC_GroupExchange548.PublishQuote()");
        c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
        c.rejects(
            "JAC_GroupExchange548.AcceptOnce()",
            "accepted native quote missing",
        );
        c.run("assert(#SENDS==1 and PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage=='QUOTED_UNKNOWN')");
    }
}

#[test]
fn unsafe_terms_changed_outgoing_and_roster_are_refused() {
    for setup in [
        "REPLY_KIND='UNSAFE'",
        "REPLY_KIND='GOLD_TOO_MUCH'",
        "REPLY_KIND='MISMATCH'",
    ] {
        let c = Case::new();
        c.run(setup);
        c.equalize();
        c.run("local r=JAC_GroupExchange548.ResponseState(); assert(r.stage=='UNSAFE_TERMS' or r.stage=='UNKNOWN_OUTGOING')");
        c.run("JAC_GroupExchange548.PublishQuote()");
        c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
        c.rejects(
            "JAC_GroupExchange548.AcceptOnce()",
            "accepted native quote missing",
        );
        c.run("assert(#SENDS==1)");
    }
    let c = Case::new();
    c.equalize();
    c.run("CAPTIVE_COUNT=2; MAP_COUNT=1");
    c.run("assert(JAC_GroupExchange548.ResponseState().stage=='UNKNOWN_ROSTER_CHANGED')");
    c.run("JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.rejects(
        "JAC_GroupExchange548.AcceptOnce()",
        "accepted native quote missing",
    );
}

#[test]
fn saved_intent_duplicate_calls_reload_and_owned_tooltip() {
    let c = Case::new();
    c.run("assert(UI==ORIGINAL_UI); JAC_GroupExchange548.Prepare()");
    c.rejects("JAC_GroupExchange548.Prepare()", "already reserved");
    c.rejects(
        "JAC_GroupExchange548.ArmAfterSave()",
        "saved Core reservation missing",
    );
    c.0.load(spy_group_exchange::RESERVE_CORE).exec().unwrap();
    c.run("JAC_GroupExchange548=nil");
    c.0.load(spy_group_exchange::UI_SETUP).exec().unwrap();
    c.rejects("JAC_GroupExchange548.Prepare()", "already reserved");

    let c = Case::new();
    c.reserve();
    c.run("JAC_GroupExchange548.OpenSession(); assert(JAC_GroupExchange548.SessionReady())");
    c.run("assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()=='LOC_UNIT_SPY_NAME')");
    c.run("CAPTIVE_COUNT=2; assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()=='OLD_TOOLTIP'); CAPTIVE_COUNT=3");
    c.run("JAC_GroupExchange548.EqualizeOnce(); JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.rejects(
        "JAC_GroupExchange548.AcceptOnce()",
        "saved final intent differs",
    );
    c.0.load(spy_group_exchange::FINAL_INTENT_CORE)
        .exec()
        .unwrap();
    c.run("JAC_GroupExchange548.AcceptOnce()");
    c.rejects("JAC_GroupExchange548.AcceptOnce()", "final already sent");
    c.run("assert(#SENDS==2)");
}

#[test]
fn duplicate_statement_and_changed_saved_terms_stop_final_acceptance() {
    let c = Case::new();
    c.equalize();
    c.run("Events.DiplomacyStatement.fire(1,0,{SessionID=7,Initiator=0,StatementType=5,StatementSubType=6,DealAction=DealProposalAction.ADJUSTED,RespondingToDealAction=DealProposalAction.EQUALIZE})");
    c.run("assert(JAC_GroupExchange548.ResponseState().stage=='UNKNOWN_RESPONSE')");
    c.run("JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.rejects(
        "JAC_GroupExchange548.AcceptOnce()",
        "accepted native quote missing",
    );

    let c = Case::new();
    c.equalize();
    c.run("JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.0.load(spy_group_exchange::FINAL_INTENT_CORE)
        .exec()
        .unwrap();
    c.run("IN.items[2].amount=20; OUT.items[2].amount=20; assert(DealManager.AreWorkingDealsEqual(0,1))");
    c.rejects("JAC_GroupExchange548.AcceptOnce()", "native terms changed");
    c.run("assert(#SENDS==1)");
}

#[test]
fn unresolved_return_is_recorded_without_success() {
    let c = Case::new();
    c.run("AUTO_RETURN=false");
    c.equalize();
    c.run("JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.0.load(spy_group_exchange::FINAL_INTENT_CORE)
        .exec()
        .unwrap();
    c.run("JAC_GroupExchange548.AcceptOnce(); assert(JAC_GroupExchange548.ReturnState().stage=='WAIT_RETURN'); TIME=TIME+31; assert(JAC_GroupExchange548.ReturnState().stage=='RETURN_UNOBSERVED')");
    c.run("JAC_GroupExchange548.PublishReturn()");
    c.0.load(spy_group_exchange::RETURN_CORE).exec().unwrap();
    c.run("assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage=='RETURN_UNRESOLVED')");
}

#[test]
fn returned_reference_id_is_observed_if_native_engine_actually_uses_it() {
    let c = Case::new();
    c.run("MAP_ID=1048586");
    c.equalize();
    c.run("JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.0.load(spy_group_exchange::FINAL_INTENT_CORE)
        .exec()
        .unwrap();
    c.run("JAC_GroupExchange548.AcceptOnce(); local r=JAC_GroupExchange548.ReturnState(); assert(r.stage=='RETURNED_MAP' and r.returned.id==MAP_ID and r.after.referencePresent==true)");
    c.run("JAC_GroupExchange548.PublishReturn()");
    c.0.load(spy_group_exchange::RETURN_CORE).exec().unwrap();
}

#[test]
fn absent_candidates_and_ambiguous_stock_copy_fail_closed() {
    let c = Case::new();
    c.run("DealManager.GetPossibleDealItems=function() return {} end");
    c.rejects("JAC_GroupExchange548.Prepare()", "candidate count changed");
    c.run("assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62==nil and #OUT.items==0)");

    let c = Case::new();
    c.equalize();
    c.run(
        "DealManager.CopyIncomingToOutgoingWorkingDeal=function() error('native copy failed') end",
    );
    c.run("assert(JAC_GroupExchange548.PublishQuote()=='UNKNOWN_COPY')");
    c.rejects(
        "JAC_GroupExchange548.PublishQuote()",
        "quote already published",
    );
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.run("assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.quote.raw.reply.SessionID==7 and PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage=='QUOTED_UNKNOWN' and #SENDS==1)");
}
