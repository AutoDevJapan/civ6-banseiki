//! A new explicitly priced PROPOSED deal, never the old EQUALIZE.
#[allow(dead_code)]
#[path = "../src/spy_group_exchange.rs"]
mod legacy;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
const SAVE: &str = "JAC-Enemy-Neutralization-02e9748e7497429e82af5ca10b890da7-0000000000000005";
const SHA: &str = "2e7744102442b852c5851d79f0c86c73dbc52fd4865b646c20fe90cb0c9fdbe6";
const KEY: &str = "JAC_BOUNDARY_FIXED_OFFER_62_V1";
fn setup() -> String {
    let a = legacy::UI_SETUP.find("local function integer").unwrap();
    let b = legacy::UI_SETUP
        .find("local function sameBaseline")
        .unwrap();
    format!(
        "local B=assert(ExposedMembers.JAC_PersonaBoundary);\n{}\n{}",
        &legacy::UI_SETUP[a..b],
        FIXED
    )
}
const FIXED: &str = r#"
assert(JAC_FixedOffer1665==nil,'fixed offer UI already exists')
local OLD='JAC_BOUNDARY_GROUP_EXCHANGE_62'
local KEY='JAC_BOUNDARY_FIXED_OFFER_62_V1'
local WKEY='JAC_FixedOffer1665Witness'
local F={};JAC_FixedOffer1665=F
local x={version=1,price=61,statements={},incomingEvents={},phase='INIT'}
local previousTooltip=ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey
local function quote(s)
 return '"'..tostring(s):gsub('[%z\1-\31\\"]',function(c)
  if c=='"' then return '\\"' end
  if c=='\\' then return '\\\\' end
  return string.format('\\u%04x',string.byte(c))
 end)..'"'
end
local function serial(v,depth)
 assert(depth<40,'evidence depth limit')
 if type(v)=='nil' then return 'null' elseif type(v)=='string' then return quote(v)
 elseif type(v)=='number' or type(v)=='boolean' then return tostring(v) end
 assert(type(v)=='table','non scalar evidence')
 local keys={};for k in pairs(v) do keys[#keys+1]=k end
 table.sort(keys,function(a,b)return tostring(a)<tostring(b)end)
 local out={};for _,k in ipairs(keys) do out[#out+1]=quote(k)..':'..serial(v[k],depth+1)end
 return '{'..table.concat(out,',')..'}'
end
local function equal(a,b)return serial(a,0)==serial(b,0)end
local function oldGuard()
 assert(Game.GetCurrentGameTurn()==62 and Game.GetLocalPlayer()==0 and Players[0]:IsHuman()
  and not Players[1]:IsHuman() and Players[0]:IsTurnActive(),'roles/turn changed')
 assert(equal(Game:GetProperty(OLD),x.oldQuote),'old quote was altered')
 assert(not Players[0]:GetDiplomacy():IsAtWarWith(1),'war changed')
end
local function witness()ExposedMembers[WKEY]=copy(x)end
local function items(direction)
 local d=assert(DealManager.GetWorkingDeal(direction,0,1))
 local r={};for i in d:Items() do
  assert(#r<16,'deal item limit')
  r[#r+1]={id=i:GetID(),kind=i:GetType(),from=i:GetFromPlayerID(),to=i:GetToPlayerID(),
   value=i:GetValueType(),amount=i:GetAmount(),duration=i:GetDuration(),valid=i:IsValid(),locked=i:IsLocked()}
 end
 assert(#r==d:GetItemCount(),'deal changed while reading');return r
end
local function read(fn)local ok,v=pcall(fn);return ok and {status='OK',value=v} or {status='ERROR',error=tostring(v)}end
local function ownedSession()
 local id=DiplomacyManager.FindOpenSessionID(0,1)
 assert(id~=nil and id==x.sessionID and DiplomacyManager.GetSessionInfo(id).FromPlayer==0,'owned session differs')
 return id
end
local function ownedStatement(r)
 if r.SessionID~=x.sessionID or r.Initiator~=0 then return false end
 local ok,key=pcall(DiplomacyManager.GetKeyName,r.StatementType)
 return ok and key=='MAKE_DEAL'
end
local function responses(action)
 local out={}
 for _,r in ipairs(x.statements)do
  if r.from==1 and r.to==0 and r.RespondingToDealAction==action and ownedStatement(r) then out[#out+1]=r end
 end
 return out
end
local function terms()
 local rows=items(DealDirection.OUTGOING);assert(#rows==2,'explicit offer must have two items')
 local cap,gold=0,0
 for _,r in ipairs(rows) do
  assert(r.valid==true,'native offer item invalid')
  if r.kind==DealItemTypes.CAPTIVE then
   assert(r.from==1 and r.to==0 and r.value==-1 and r.locked==true,'captive term changed');cap=cap+1
  elseif r.kind==DealItemTypes.GOLD then
   assert(r.from==0 and r.to==1 and r.duration==0 and r.amount==x.price,'gold term changed');gold=gold+1
  else error('unexpected offer term') end
 end
 assert(cap==1 and gold==1,'offer kinds differ')
 assert(Players[0]:GetTreasury():GetGoldBalance()>=x.price,'insufficient actual gold')
 return rows
end
Events.DiplomacyStatement.Add(function(from,to,k)
 if x.closed or not x.openIntent or type(k)~='table' then return end
 if not ((from==0 and to==1) or (from==1 and to==0)) then return end
 if #x.statements>=64 then x.overflow=true;return end
 local r={from=from,to=to}
 for _,key in ipairs({'SessionID','Initiator','StatementType','StatementSubType','DealAction','RespondingToDealAction'})do r[key]=k[key]end
 x.statements[#x.statements+1]=r
end)
Events.DiplomacyIncomingDeal.Add(function(from,to,action)
 if x.closed or not x.openIntent or from~=1 or to~=0 then return end
 if #x.incomingEvents>=64 then x.overflow=true;return end
 x.incomingEvents[#x.incomingEvents+1]={from=from,to=to,action=action}
end)
local function tooltip(...)
 if x.openIntent and not x.closed and x.captiveItemID then
  local ok,key=pcall(function()
   local item=assert(outgoing():FindItemByID(x.captiveItemID))
   assert(item:GetType()==DealItemTypes.CAPTIVE and item:GetFromPlayerID()==1 and
    item:GetToPlayerID()==0 and item:GetValueType()==-1 and item:GetValueTypeNameID()==nil)
   return GameInfo.Units['UNIT_SPY'].Name
  end)
  if ok then return key end
 end
 if previousTooltip then return previousTooltip(...) end
end
ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey=tooltip
function F.Prepare()
 assert(x.phase=='INIT' and not Game:GetProperty(KEY),'new fixed offer already exists')
 local p=assert(Game:GetProperty(OLD))
 assert(p.stage=='QUOTED_UNKNOWN' and p.nonce=='53.430954889496:62' and p.quote and
  p.quote.raw and p.quote.raw.reply and p.quote.raw.reply.DealAction==10 and
  p.quote.raw.reply.RespondingToDealAction==6 and p.quote.acceptable==false,'not latest failed equalization')
 x.oldQuote=copy(p);oldGuard()
 assert(not DealManager.HasPendingDeal(0,1) and DiplomacyManager.FindOpenSessionID(0,1)==nil,'pending/session exists')
 assert(#items(DealDirection.OUTGOING)==0 and #items(DealDirection.INCOMING)==0,'working deal not empty')
 x.before=rosterSnapshot()
 assert(x.before.capacity==3 and x.before.physical==3 and #x.before.captured==3 and
  #x.before.offMap==3 and #x.before.map==0 and #x.before.activeOffMap==0,'three-captive baseline differs')
 for _,c in ipairs(x.before.captured)do assert(c.captor==1 and c.nameIndex==-1 and c.name=='' and c.level==1,'captive changed')end
 local candidates=assert(DealManager.GetPossibleDealItems(1,0,DealItemTypes.CAPTIVE,outgoing()))
 assert(#candidates==3,'candidate count differs')
 for _,c in ipairs(candidates)do assert(c.ForType==-1 and (c.IsValid==true or c.IsValid==1) and c.ValidationResult==DealValidationResult.VALID,'candidate invalid')end
 x.goldBefore=Players[0]:GetTreasury():GetGoldBalance();assert(math.floor(x.goldBefore)==61,'budget baseline changed')
 x.nonce='fixed-price-v1:'..tostring(UI.GetElapsedTime())..':62';x.phase='PREPARED';witness();return x.phase
end
function F.Open()
 oldGuard();assert(x.phase=='PREPARED' and not x.openIntent,'open already attempted')
 local p=assert(Game:GetProperty(KEY));assert(p.phase=='PREPARED' and p.nonce==x.nonce,'saved new intent missing')
 assert(equal(x.before,rosterSnapshot()) and not DealManager.HasPendingDeal(0,1) and
  DiplomacyManager.FindOpenSessionID(0,1)==nil and #items(DealDirection.OUTGOING)==0,'state changed before open')
 x.openIntent=true
 local d=outgoing();local c=assert(d:AddItemOfType(DealItemTypes.CAPTIVE,1))
 x.captiveItemID=c:GetID();c:SetValueType(-1);assert(c:IsValid());c:SetLocked(true)
 local g=assert(d:AddItemOfType(DealItemTypes.GOLD,0));g:SetDuration(0)
 assert(g:GetMaxAmount()>=x.price,'native gold limit is below explicit price');g:SetAmount(x.price);assert(g:IsValid())
 x.originalTerms=terms();DiplomacyManager.RequestSession(0,1,'MAKE_DEAL');return 'OPEN_REQUESTED'
end
function F.SessionReady()
 if not x.openIntent or x.closed then return false end
 local id=DiplomacyManager.FindOpenSessionID(0,1);if id==nil then return false end
 assert(DiplomacyManager.GetSessionInfo(id).FromPlayer==0,'session initiator differs');x.sessionID=id
 -- The stock deal view inspects the populated deal on show. A session alone
 -- does not mean that asynchronous inspection is finished.
 return not x.overflow and not DealManager.HasPendingDeal(0,1) and
  type(DealProposalAction.INSPECT)=='number' and #responses(DealProposalAction.INSPECT)>0
end
function F.Arm()
 oldGuard();assert(F.SessionReady() and not x.sent and x.phase=='PREPARED')
 assert(equal(x.before,rosterSnapshot()) and not DealManager.HasPendingDeal(0,1))
 x.proposedTerms=terms();assert(outgoing():Validate()==DealValidationResult.VALID)
 x.phase='READY_TO_PROPOSE';witness();return x.phase
end
function F.Propose()
 oldGuard();assert(x.phase=='READY_TO_PROPOSE' and not x.sent,'proposal already attempted')
 local p=assert(Game:GetProperty(KEY));assert(p.phase==x.phase and p.nonce==x.nonce,'saved proposal intent differs')
 ownedSession();assert(not DealManager.HasPendingDeal(0,1) and equal(x.before,rosterSnapshot()))
 assert(equal(terms(),x.proposedTerms) and outgoing():Validate()==DealValidationResult.VALID,'terms changed after save')
 x.sent=true;x.sentAt=UI.GetElapsedTime()
 local ok,err=pcall(DealManager.SendWorkingDeal,DealProposalAction.PROPOSED,0,1)
 if not ok then x.sendError=tostring(err) end
 return 'PROPOSED_ONCE_OR_UNKNOWN'
end
function F.Complete()
 assert(x.sent,'proposal not attempted')
 if x.sendError~=nil or UI.GetElapsedTime()-x.sentAt>=30 then return true end
 local replies=responses(DealProposalAction.PROPOSED)
 if #replies==0 then return false end
 if #replies~=1 or replies[1].DealAction~=DealProposalAction.ACCEPTED then return true end
 -- ACCEPTED can arrive before the native payment/roster becomes visible.
 local ok,settled=pcall(function()
  local now=rosterSnapshot()
  return #now.captured==2 and #now.map==1 and #now.activeOffMap==0 and
   Players[0]:GetTreasury():GetGoldBalance()==x.goldBefore-x.price and not DealManager.HasPendingDeal(0,1)
 end)
 return ok and settled==true
end
function F.Publish()
 assert(x.sent and F.Complete() and x.phase=='READY_TO_PROPOSE','outcome not ready')
 local r={source='UI_NATIVE',returned=false,stage='UNKNOWN',
  outgoing=read(function()return items(DealDirection.OUTGOING)end),incoming=read(function()return items(DealDirection.INCOMING)end),
  roster=read(rosterSnapshot),gold=read(function()return Players[0]:GetTreasury():GetGoldBalance()end),
  pending=read(function()return DealManager.HasPendingDeal(0,1)end)}
 local matches=responses(DealProposalAction.PROPOSED)
 r.responses=copy(matches)
 if #matches==1 and not x.overflow and not x.sendError then
  local reply=matches[1].DealAction
  if reply==DealProposalAction.REJECTED or reply==DealProposalAction.REJECTED_PERMANENT then r.stage='REJECTED' end
  if reply==DealProposalAction.ACCEPTED and r.roster.status=='OK' then
   local now=r.roster.value
   if now.capacity==3 and now.physical==3 and now.turn==62 and equal(now.histories,x.before.histories) and
    #now.captured==2 and #now.map==1 and #now.activeOffMap==0 and r.gold.status=='OK' and
    r.gold.value==x.goldBefore-x.price and r.pending.status=='OK' and r.pending.value==false and
    equal(Game:GetProperty(OLD),x.oldQuote) then
    local remaining=true
    for _,c in ipairs(now.captured)do
     if c.captor~=1 or c.nameIndex~=-1 or c.name~='' or c.level~=1 then remaining=false end
    end
    local u=now.map[1]
    if remaining and u.owner==0 and u.unitType==GameInfo.Units['UNIT_SPY'].Index and integer(u.id,0) and
     integer(u.x,0) and integer(u.y,0) and Map.GetPlot(u.x,u.y) then
     r.stage='RETURNED_MAP';r.returned=true;r.unit=copy(u)
    end
   end
  end
 end
 x.result=r;x.phase='OUTCOME';witness();return r.stage
end
function F.Close()
 assert(x.phase=='OUTCOME' and not x.closed,'record outcome before close')
 x.cleanup=read(function()
  local id=DiplomacyManager.FindOpenSessionID(0,1)
  if id~=nil then assert(id==x.sessionID and DiplomacyManager.GetSessionInfo(id).FromPlayer==0,'foreign session');DiplomacyManager.CloseSession(id) end
  return true
 end)
 x.closed=true
 if ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey==tooltip then ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey=previousTooltip end
 return 'CLOSED_OR_UNKNOWN'
end
function F.Evidence()return serial(x,0)end
return F.Prepare()
"#;
fn record(phase: &str, prior: &str) -> String {
    format!(
        r#"
local key='{KEY}';local w=assert(ExposedMembers.JAC_FixedOffer1665Witness)
assert(w.version==1 and w.phase=='{phase}' and w.price==61 and type(w.nonce)=='string')
local old=Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62')
local function same(a,b,depth)
 if depth>40 or type(a)~=type(b) then return false end
 if type(a)~='table' then return a==b end
 for k,v in pairs(a)do if not same(v,b[k],depth+1)then return false end end
 for k in pairs(b)do if a[k]==nil then return false end end
 return true
end
local p=Game:GetProperty(key)
if '{prior}'=='ABSENT' then assert(p==nil and w.sent==nil) else assert(p and p.nonce==w.nonce and p.phase=='{prior}') end
if '{phase}'=='OUTCOME' then
 assert(w.sent==true and type(w.result)=='table' and w.result.source=='UI_NATIVE')
 w.recordTurn=Game.GetCurrentGameTurn()
 if w.recordTurn~=62 or not same(old,w.oldQuote,0) then
  w.recordAnomaly={{reason='OLD_QUOTE_OR_TURN_CHANGED',oldQuotePresent=old~=nil,oldQuote=old,turn=w.recordTurn}}
  w.result.returned=false;w.result.stage='UNKNOWN_RECORD_ANOMALY'
 end
else
 assert(Game.GetCurrentGameTurn()==62 and type(old)=='table' and
  old.nonce=='53.430954889496:62' and old.stage=='QUOTED_UNKNOWN' and
  old.quote.raw.reply.DealAction==10 and same(old,w.oldQuote,0))
end
Game:SetProperty(key,w);return w.phase
"#
    )
}
fn ui(code: impl Into<String>) -> Value {
    json!({"action":"ui_lua","state_name":"InGame","code":code.into(),"preserve_diplomacy_session":true})
}
fn core(code: String) -> Value {
    json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":code})
}
fn save() -> Value {
    json!({"action":"save","name":"JAC-Fixed-Captive-Offer","exact_name":true,"checkpoint":true,"preserve_diplomacy_session":true})
}
fn wait(code: &str) -> Value {
    json!({"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"InGame","code":code,"preserve_diplomacy_session":true}})
}
fn scene() -> Value {
    json!({"name":"New fixed 61 gold captive proposal; no old equalization replay","judge_profile":"generic","strict_lua_errors":true,"steps":[
      {"action":"load","name":SAVE,"exact_name":true,"timeout":300,"save_sha256":SHA,"resume_once":true},
      wait("return Players[0] and Players[0]:IsTurnActive() and ExposedMembers.JAC_PersonaBoundary~=nil"),
      ui(setup()),core(record("PREPARED","ABSENT")),save(),ui("return JAC_FixedOffer1665.Open()"),
      wait("return JAC_FixedOffer1665.SessionReady()"),ui("return JAC_FixedOffer1665.Arm()"),
      core(record("READY_TO_PROPOSE","PREPARED")),save(),ui("return JAC_FixedOffer1665.Propose()"),save(),
      wait("return JAC_FixedOffer1665.Complete()"),ui("return JAC_FixedOffer1665.Publish()"),
      core(record("OUTCOME","READY_TO_PROPOSE")),save(),ui("return JAC_FixedOffer1665.Close()"),
      ui("return JAC_FixedOffer1665.Evidence()"),
      ui(format!("local p=assert(Game:GetProperty('{KEY}'));assert(p.phase=='OUTCOME' and p.result.returned==true,'new offer did not prove native return');return p.result.stage")),
      {"action":"exit_to_menu"}
    ]})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().skip(1).collect();
    if a.len() != 2 {
        return Err("usage: propose-fixed-captive-62 LATEST_SAVE OUTPUT".into());
    }
    if format!("{:x}", Sha256::digest(std::fs::read(&a[0])?)) != SHA {
        return Err("latest save differs".into());
    }
    let s = scene();
    civ6_autotest_rs::scenario::validate_scenario(&s)?;
    std::fs::write(&a[1], serde_json::to_vec_pretty(&s)?)?;
    Ok(())
}

#[cfg(test)]
#[path = "../tests/fixtures/spy_exchange_native.rs"]
mod native_fixture;
#[cfg(test)]
fn prepared() -> mlua::Lua {
    let lua = mlua::Lua::new();
    lua.load(native_fixture::NATIVE).exec().unwrap();
    lua.load(
        r#"
GOLD_BALANCE=61.6953125;MODE='ACCEPT';PENDING=false;DELAY_INSPECT=false
DealProposalAction.INSPECT=99
function DealManager.HasPendingDeal()return PENDING end
function inspectReply(initiator)
 Events.DiplomacyStatement.fire(1,0,{SessionID=7,Initiator=initiator or 0,StatementType=5,
  DealAction=DealProposalAction.REJECTED,RespondingToDealAction=DealProposalAction.INSPECT})
end
local request=DiplomacyManager.RequestSession
function DiplomacyManager.RequestSession(...)
 request(...);if not DELAY_INSPECT then inspectReply()end
end
PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62={stage='QUOTED_UNKNOWN',nonce='53.430954889496:62',
 quote={acceptable=false,raw={reply={DealAction=10,RespondingToDealAction=6}}}}
OLD_REFERENCE=PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62
local add=OUT.AddItemOfType
function OUT:AddItemOfType(kind,from)
 local i=add(self,kind,from)
 function i:SetDuration(v)self.duration=v end
 function i:SetAmount(v)self.amount=v end
 function i:GetMaxAmount()return math.floor(GOLD_BALANCE)end
 return i
end
function DealManager.SendWorkingDeal(action,from,to)
 assert(action==DealProposalAction.PROPOSED and from==0 and to==1,'not new priced proposal')
 SENDS[#SENDS+1]={action=action,from=from,to=to}
 if MODE=='THROW' then error('unknown after native send')end
 if MODE=='NONE' then return end
 local reply=(MODE=='ACCEPT' or MODE=='ACCEPT_NO_RETURN') and DealProposalAction.ACCEPTED or DealProposalAction.REJECTED
 if MODE=='ACCEPT' then CAPTIVE_COUNT=2;MAP_COUNT=1;GOLD_BALANCE=GOLD_BALANCE-61 end
 Events.DiplomacyStatement.fire(1,0,{SessionID=7,Initiator=0,StatementType=5,
  DealAction=reply,RespondingToDealAction=DealProposalAction.PROPOSED})
end
"#,
    )
    .exec()
    .unwrap();
    assert_eq!(lua.load(setup()).eval::<String>().unwrap(), "PREPARED");
    lua.load(record("PREPARED", "ABSENT")).exec().unwrap();
    lua
}
#[cfg(test)]
fn armed() -> mlua::Lua {
    let lua = prepared();
    lua.load("JAC_FixedOffer1665.Open();assert(JAC_FixedOffer1665.SessionReady());JAC_FixedOffer1665.Arm()").exec().unwrap();
    lua.load(record("READY_TO_PROPOSE", "PREPARED"))
        .exec()
        .unwrap();
    lua
}
#[test]
fn exact_priced_proposal_returns_native_unit_and_keeps_old_quote_and_histories() {
    let lua = armed();
    lua.load("JAC_FixedOffer1665.Propose();assert(JAC_FixedOffer1665.Complete());assert(JAC_FixedOffer1665.Publish()=='RETURNED_MAP')").exec().unwrap();
    lua.load(record("OUTCOME", "READY_TO_PROPOSE"))
        .exec()
        .unwrap();
    lua.load("JAC_FixedOffer1665.Close();assert(#SENDS==1 and CLOSED==1);assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62==OLD_REFERENCE);assert(H[1].turn==23 and H[2].turn==45 and H[3].turn==62);assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey==ORIGINAL_TOOLTIP)").exec().unwrap();
    let e: String = lua
        .load("return JAC_FixedOffer1665.Evidence()")
        .eval()
        .unwrap();
    let e: Value = serde_json::from_str(&e).unwrap();
    assert_eq!(e["result"]["unit"]["id"], 77777);
    assert_eq!(e["result"]["gold"]["value"], 0.6953125);
    assert_eq!(e["result"]["returned"], true);
    assert!(lua.load("JAC_FixedOffer1665.Propose()").exec().is_err());
}
#[test]
fn rejected_or_unknown_send_never_retries_and_keeps_terms_in_evidence() {
    for mode in ["REJECT", "THROW", "NONE"] {
        let lua = armed();
        lua.load(format!(
            "MODE='{mode}';JAC_FixedOffer1665.Propose();TIME=TIME+31;JAC_FixedOffer1665.Publish()"
        ))
        .exec()
        .unwrap();
        assert!(lua.load("JAC_FixedOffer1665.Propose()").exec().is_err());
        let e:String=lua.load("assert(#SENDS==1 and CAPTIVE_COUNT==3 and GOLD_BALANCE==61.6953125);return JAC_FixedOffer1665.Evidence()").eval().unwrap();
        let e: Value = serde_json::from_str(&e).unwrap();
        assert_eq!(e["result"]["returned"], false);
        assert_eq!(e["result"]["outgoing"]["value"]["2"]["amount"], 61);
        assert_eq!(
            e["result"]["stage"],
            if mode == "REJECT" {
                "REJECTED"
            } else {
                "UNKNOWN"
            }
        );
    }
}
#[test]
fn changed_budget_terms_or_saved_intent_prevent_any_proposal() {
    for changed in [
        "GOLD_BALANCE=60",
        "OUT.items[2].amount=60",
        "OUT.items[1].value=0",
        "PROPERTIES.JAC_BOUNDARY_FIXED_OFFER_62_V1.nonce='other'",
        "PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage='RESERVED'",
    ] {
        let lua = armed();
        lua.load(changed).exec().unwrap();
        assert!(
            lua.load("JAC_FixedOffer1665.Propose()").exec().is_err(),
            "{changed}"
        );
        lua.load("assert(#SENDS==0)").exec().unwrap();
    }
}
#[test]
fn accepted_reply_without_payment_or_return_is_not_success() {
    for command in [
        "MODE='ACCEPT_NO_RETURN';JAC_FixedOffer1665.Propose()",
        "JAC_FixedOffer1665.Propose();GOLD_BALANCE=61.6953125",
        "JAC_FixedOffer1665.Propose();CAPTIVE_COUNT=3;MAP_COUNT=0",
        "JAC_FixedOffer1665.Propose();PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.nonce='changed'",
    ] {
        let lua = armed();
        lua.load(command).exec().unwrap();
        lua.load("TIME=TIME+31;JAC_FixedOffer1665.Publish()")
            .exec()
            .unwrap();
        let e: String = lua
            .load("return JAC_FixedOffer1665.Evidence()")
            .eval()
            .unwrap();
        let e: Value = serde_json::from_str(&e).unwrap();
        assert_eq!(e["result"]["responses"]["1"]["DealAction"], 2);
        assert_eq!(e["result"]["returned"], false, "{command}");
    }
}

#[test]
fn delayed_inspection_and_pending_reply_are_waited_out_before_arming() {
    let lua = prepared();
    lua.load("DELAY_INSPECT=true;JAC_FixedOffer1665.Open();PENDING=true;assert(not JAC_FixedOffer1665.SessionReady());PENDING=false;assert(not JAC_FixedOffer1665.SessionReady());inspectReply(1);assert(not JAC_FixedOffer1665.SessionReady());PENDING=true;inspectReply(0);assert(not JAC_FixedOffer1665.SessionReady());PENDING=false;assert(JAC_FixedOffer1665.SessionReady());JAC_FixedOffer1665.Arm();assert(#SENDS==0)").exec().unwrap();
}

#[test]
fn malformed_proposal_reply_and_late_native_payment_do_not_end_wait_early() {
    let lua = armed();
    lua.load("MODE='NONE';JAC_FixedOffer1665.Propose();Events.DiplomacyStatement.fire(1,0,{SessionID=7,Initiator=1,StatementType=5,DealAction=2,RespondingToDealAction=4});assert(not JAC_FixedOffer1665.Complete());Events.DiplomacyStatement.fire(1,0,{SessionID=7,Initiator=0,StatementType=5,DealAction=2,RespondingToDealAction=4});assert(not JAC_FixedOffer1665.Complete());CAPTIVE_COUNT=2;MAP_COUNT=1;GOLD_BALANCE=GOLD_BALANCE-61;assert(JAC_FixedOffer1665.Complete());assert(JAC_FixedOffer1665.Publish()=='RETURNED_MAP');assert(#SENDS==1)").exec().unwrap();
}

#[test]
fn post_send_old_quote_change_records_anomaly_then_reaches_cleanup_and_failure_guard() {
    let lua = armed();
    lua.load("JAC_FixedOffer1665.Propose();PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.nonce='changed';TIME=TIME+31").exec().unwrap();
    let s = scene();
    for index in 13..18 {
        let step = &s["steps"][index];
        if step["action"] == "save" {
            lua.load("SAVED_OUTCOME=PROPERTIES.JAC_BOUNDARY_FIXED_OFFER_62_V1;assert(SAVED_OUTCOME.phase=='OUTCOME' and SAVED_OUTCOME.result.returned==false and SAVED_OUTCOME.recordAnomaly.reason=='OLD_QUOTE_OR_TURN_CHANGED')").exec().unwrap();
        } else {
            lua.load(step["code"].as_str().unwrap()).exec().unwrap();
        }
    }
    assert!(lua
        .load(s["steps"][18]["code"].as_str().unwrap())
        .exec()
        .is_err());
    lua.load("assert(SAVED_OUTCOME and CLOSED==1 and #SENDS==1);assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62==OLD_REFERENCE and OLD_REFERENCE.nonce=='changed')").exec().unwrap();
}
#[test]
fn generated_sequence_pins_latest_save_and_preserves_checkpoints_before_final_guard() {
    let s = scene();
    civ6_autotest_rs::scenario::validate_scenario(&s).unwrap();
    let steps = s["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 20);
    assert_eq!(steps[0]["resume_once"], true);
    assert_eq!(steps[0]["save_sha256"], SHA);
    assert_eq!(steps[9]["action"], "save");
    assert_eq!(steps[11]["action"], "save");
    assert_eq!(steps[15]["action"], "save");
    assert!(steps[16]["code"].as_str().unwrap().contains(".Close()"));
    assert!(steps[18]["code"]
        .as_str()
        .unwrap()
        .contains("result.returned==true"));
    let code = setup();
    assert_eq!(
        code.matches("DealManager.SendWorkingDeal,DealProposalAction.PROPOSED")
            .count(),
        1
    );
    for forbidden in [
        "SendWorkingDeal(DealProposalAction.EQUALIZE",
        "ResumeReserved(",
        "UnitManager.Init",
        "SetName(",
        "SetNameIndex(",
    ] {
        assert!(!code.contains(forbidden));
    }
    for step in steps {
        if let Some(code) = step["code"].as_str() {
            mlua::Lua::new().load(code).into_function().unwrap();
        }
    }
}
