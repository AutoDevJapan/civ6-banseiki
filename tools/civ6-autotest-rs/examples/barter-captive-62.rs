//! Inspect a distinct, measured barter offer through the stock UI before acceptance.
#[allow(dead_code)]
#[path = "../src/spy_group_exchange.rs"]
mod legacy;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
const SAVE: &str = "JAC-Fixed-Captive-Offer-2f83cc9655344adf9d671d24ddacf5be-0000000000000004";
const SHA: &str = "c25efd2a205d3727d6b515f10c09f06a835e8de12887bfdfe808dff831b47b1d";
const KEY: &str = "JAC_BOUNDARY_BARTER_OFFER_62_V1";
fn setup() -> String {
    let a = legacy::UI_SETUP.find("local function integer").unwrap();
    let b = legacy::UI_SETUP
        .find("local function sameBaseline")
        .unwrap();
    format!(
        "local B=assert(ExposedMembers.JAC_PersonaBoundary);\n{}\n{BARTER}",
        &legacy::UI_SETUP[a..b]
    )
}
const BARTER: &str = r#"
assert(JAC_Barter1665==nil,'barter instance exists')
local KEY='JAC_BOUNDARY_BARTER_OFFER_62_V1'
local OLD='JAC_BOUNDARY_GROUP_EXCHANGE_62'
local FIXED='JAC_BOUNDARY_FIXED_OFFER_62_V1'
local WKEY='JAC_Barter1665Witness'
local F={};JAC_Barter1665=F
local x={version=1,price=61,borderDuration=30,phase='INIT',statements={}}
local priorTooltip=ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey
local function quote(s)
 return '"'..tostring(s):gsub('[%z\1-\31\\"]',function(c)
  if c=='"'then return '\\"'end;if c=='\\'then return '\\\\'end
  return string.format('\\u%04x',string.byte(c))
 end)..'"'
end
local function serial(v,depth)
 assert(depth<40,'evidence depth limit')
 local t=type(v);if t=='nil'then return 'null'elseif t=='string'then return quote(v)
 elseif t=='number'or t=='boolean'then return tostring(v)end
 assert(t=='table','non scalar evidence')
 local keys={};for k in pairs(v)do keys[#keys+1]=k end
 table.sort(keys,function(a,b)return tostring(a)<tostring(b)end)
 local out={};for _,k in ipairs(keys)do out[#out+1]=quote(k)..':'..serial(v[k],depth+1)end
 return '{'..table.concat(out,',')..'}'
end
local function equal(a,b)return serial(a,0)==serial(b,0)end
local function read(fn)
 local ok,v=pcall(fn);if ok then return {status='OK',kind=type(v),value=v}end
 return {status='ERROR',error=tostring(v)}
end
local function witness()ExposedMembers[WKEY]=copy(x)end
local function guard()
 assert(Game.GetCurrentGameTurn()==62 and Game.GetLocalPlayer()==0 and Players[0]:IsHuman()
  and not Players[1]:IsHuman() and Players[0]:IsTurnActive(),'turn/roles changed')
 assert(equal(Game:GetProperty(OLD),x.old) and equal(Game:GetProperty(FIXED),x.fixed),'past evidence changed')
 assert(not Players[0]:GetDiplomacy():IsAtWarWith(1),'war changed')
end
local function items(direction)
 local d=assert(DealManager.GetWorkingDeal(direction,0,1));local rows={}
 for i in d:Items()do
  assert(#rows<16,'item limit')
  rows[#rows+1]={id=i:GetID(),kind=i:GetType(),subType=i:GetSubType(),from=i:GetFromPlayerID(),
   to=i:GetToPlayerID(),value=i:GetValueType(),amount=i:GetAmount(),duration=i:GetDuration(),
   valid=i:IsValid(),unacceptable=i:IsUnacceptable(),locked=i:IsLocked()}
 end
 assert(#rows==d:GetItemCount(),'items changed during read');return rows
end
local function terms(direction)
 local rows=items(direction);assert(#rows==3,'barter requires exactly three terms')
 local cap,gold,border=0,0,0
 for _,r in ipairs(rows)do
  assert(r.valid==true and r.unacceptable==false,'invalid/unacceptable term')
  if r.kind==DealItemTypes.CAPTIVE then
   assert(r.from==1 and r.to==0 and r.value==-1,'anonymous captive term changed');cap=cap+1
  elseif r.kind==DealItemTypes.GOLD then
   assert(r.from==0 and r.to==1 and r.duration==0 and r.amount==61,'gold term changed');gold=gold+1
  elseif r.kind==DealItemTypes.AGREEMENTS then
   assert(r.from==0 and r.to==1 and r.subType==x.borderType and r.duration==30,'border term changed');border=border+1
  else error('unexpected barter term')end
 end
 assert(cap==1 and gold==1 and border==1,'barter kinds differ');return rows
end
local function session()
 local id=DiplomacyManager.FindOpenSessionID(0,1)
 assert(id~=nil and id==x.sessionID and DiplomacyManager.GetSessionInfo(id).FromPlayer==0,'owned session differs')
 return id
end
local function inspections()
 local rows={};for _,r in ipairs(x.statements)do
  local ok,key=pcall(DiplomacyManager.GetKeyName,r.StatementType)
  if ok and key=='MAKE_DEAL' and r.from==1 and r.to==0 and r.SessionID==x.sessionID and
   r.Initiator==0 and r.RespondingToDealAction==DealProposalAction.INSPECT then rows[#rows+1]=r end
 end;return rows
end
Events.DiplomacyStatement.Add(function(from,to,k)
 if x.closed or not x.openIntent or type(k)~='table' or not ((from==0 and to==1)or(from==1 and to==0))then return end
 if #x.statements>=64 then x.overflow=true;return end
 local r={from=from,to=to}
 for _,key in ipairs({'SessionID','Initiator','StatementType','StatementSubType','DealAction','RespondingToDealAction'})do r[key]=k[key]end
 x.statements[#x.statements+1]=r
end)
local function tooltip(...)
 if x.openIntent and not x.closed and x.captiveItemID then
  local ok,key=pcall(function()
   local i=assert(outgoing():FindItemByID(x.captiveItemID))
   assert(i:GetType()==DealItemTypes.CAPTIVE and i:GetFromPlayerID()==1 and
    i:GetToPlayerID()==0 and i:GetValueType()==-1 and i:GetValueTypeNameID()==nil)
   return GameInfo.Units['UNIT_SPY'].Name
  end);if ok then return key end
 end
 if priorTooltip then return priorTooltip(...)end
end
ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey=tooltip
function F.Prepare()
 assert(x.phase=='INIT' and Game:GetProperty(KEY)==nil,'new barter exists')
 local p=assert(Game:GetProperty(FIXED))
 assert(p.phase=='OUTCOME' and p.sent==true and p.price==61 and p.result.stage=='REJECTED' and
  p.result.returned==false and p.result.responses[1].DealAction==2 and
  p.result.responses[1].RespondingToDealAction==4,'latest priced rejection required')
 x.old=copy(assert(Game:GetProperty(OLD)));x.fixed=copy(p);guard()
 assert(not DealManager.HasPendingDeal(0,1) and DiplomacyManager.FindOpenSessionID(0,1)==nil and
  #items(DealDirection.OUTGOING)==0 and #items(DealDirection.INCOMING)==0,'pending/session/items exist')
 x.before=rosterSnapshot()
 assert(x.before.capacity==3 and x.before.physical==3 and #x.before.captured==3 and
  #x.before.offMap==3 and #x.before.map==0 and #x.before.activeOffMap==0,'baseline differs')
 for _,c in ipairs(x.before.captured)do assert(c.captor==1 and c.nameIndex==-1 and c.name=='' and c.level==1,'captive changed')end
 local caps=assert(DealManager.GetPossibleDealItems(1,0,DealItemTypes.CAPTIVE,outgoing()))
 assert(#caps==3,'captives differ');for _,c in ipairs(caps)do
  assert(c.ForType==-1 and (c.IsValid==true or c.IsValid==1) and c.ValidationResult==DealValidationResult.VALID,'captive invalid')
 end
 local def=assert(GameInfo.DiplomaticActions['DIPLOACTION_OPEN_BORDERS'])
 assert(def.Hash==-242352742,'measured border definition changed');x.borderType=def.Hash
 local found=0
 for _,c in ipairs(assert(DealManager.GetPossibleDealItems(0,1,DealItemTypes.AGREEMENTS,outgoing())))do
  if c.SubType==x.borderType then
   assert(c.Duration==30 and (c.IsValid==true or c.IsValid==1) and c.ValidationResult==DealValidationResult.VALID,'border unavailable')
   found=found+1
  end
 end;assert(found==1,'measured border candidate missing/duplicate')
 x.goldBefore=Players[0]:GetTreasury():GetGoldBalance();assert(x.goldBefore==61.6953125,'actual budget changed')
 x.borderBefore=Players[1]:GetDiplomacy():HasOpenBordersFrom(0);assert(x.borderBefore==false,'border already active')
 x.nonce='barter-v1:'..tostring(UI.GetElapsedTime())..':62';x.phase='PREPARED';witness();return x.phase
end
function F.Open()
 guard();assert(x.phase=='PREPARED' and not x.openIntent,'open already attempted')
 local p=assert(Game:GetProperty(KEY));assert(p.phase==x.phase and p.nonce==x.nonce,'saved intent differs')
 assert(equal(x.before,rosterSnapshot()) and not DealManager.HasPendingDeal(0,1) and
  DiplomacyManager.FindOpenSessionID(0,1)==nil and #items(DealDirection.OUTGOING)==0,'state changed')
 x.openIntent=true
 local d=outgoing();local c=assert(d:AddItemOfType(DealItemTypes.CAPTIVE,1))
 x.captiveItemID=c:GetID();c:SetValueType(-1);assert(c:IsValid());c:SetLocked(true)
 local g=assert(d:AddItemOfType(DealItemTypes.GOLD,0));g:SetDuration(0)
 assert(g:GetMaxAmount()>=61,'native budget insufficient');g:SetAmount(61)
 local a=assert(d:AddItemOfType(DealItemTypes.AGREEMENTS,0));a:SetSubType(x.borderType);a:SetDuration(30)
 x.originalTerms=terms(DealDirection.OUTGOING);DiplomacyManager.RequestSession(0,1,'MAKE_DEAL')
 x.openAt=UI.GetElapsedTime();return 'NEW_BARTER_OPENED'
end
local availability,permitted
local function inspectionTerms()
 terms(DealDirection.INCOMING);terms(DealDirection.OUTGOING)
 assert(equal(x.before,rosterSnapshot()),'inspection roster changed')
 assert(Players[0]:GetTreasury():GetGoldBalance()==x.goldBefore,'inspection budget changed')
 return true
end
function F.InspectionComplete()
 if not x.openIntent or x.closed then return false end
 local id=DiplomacyManager.FindOpenSessionID(0,1)
 if id~=nil then
  assert(DiplomacyManager.GetSessionInfo(id).FromPlayer==0,'foreign session');x.sessionID=id
 end
 if x.overflow or UI.GetElapsedTime()-x.openAt>=30 then return true end
 if id==nil or DealManager.HasPendingDeal(0,1)then return false end
 local r=inspections();if #r==0 then return false end
 if #r~=1 or r[1].DealAction~=DealProposalAction.ACCEPTED then return true end
 x.lastInspectionProbe={ui=read(availability),terms=read(inspectionTerms)}
 return x.lastInspectionProbe.ui.status=='OK' and permitted(x.lastInspectionProbe.ui.value) and
  x.lastInspectionProbe.terms.status=='OK' and x.lastInspectionProbe.terms.value==true
end
availability=function()
 session();local ctx=assert(ContextPtr:LookUpControl('/InGame/DiplomacyDealView'),'stock deal context missing')
 local button=assert(ContextPtr:LookUpControl('/InGame/DiplomacyDealView/AcceptDeal'),'stock accept button missing')
 local d=outgoing()
 return {contextHidden=ctx:IsHidden(),buttonHidden=button:IsHidden(),buttonDisabled=button:IsDisabled(),
  unacceptable=d:HasUnacceptableItems(),validation=d:Validate(),equal=DealManager.AreWorkingDealsEqual(0,1),
  pending=DealManager.HasPendingDeal(0,1)}
end
permitted=function(v)
 return v.contextHidden==false and v.buttonHidden==false and v.buttonDisabled==false and
  v.unacceptable==false and v.validation==DealValidationResult.VALID and v.equal==true and v.pending==false
end
function F.Inspect()
 guard();assert(x.phase=='PREPARED' and F.InspectionComplete(),'inspection not ready')
 x.inspection={responses=copy(inspections()),outgoing=read(function()return items(DealDirection.OUTGOING)end),
  incoming=read(function()return items(DealDirection.INCOMING)end),ui=read(availability),terms=read(inspectionTerms)}
 x.allowed=false
 local r=x.inspection.responses
 if #r==1 and not x.overflow and r[1].DealAction==DealProposalAction.ACCEPTED and
  x.inspection.ui.status=='OK' and permitted(x.inspection.ui.value) and
  x.inspection.terms.status=='OK' and x.inspection.terms.value==true then
  x.allowed=true
 end
 x.phase='INSPECTED';witness();return x.allowed and 'STOCK_UI_PERMITTED' or 'NOT_PERMITTED'
end
function F.AcceptIfPermitted()
 guard();assert(x.phase=='INSPECTED' and not x.acceptAttempted and not x.skipped,'accept already handled')
 local p=assert(Game:GetProperty(KEY));assert(p.phase=='INSPECTED' and p.nonce==x.nonce and equal(p.inspection,x.inspection),'saved inspection differs')
 if not x.allowed then x.skipped=true;witness();return 'NOT_SENT_UI_NOT_PERMITTED'end
 session();assert(permitted(availability()) and #inspections()==1 and
  inspections()[1].DealAction==DealProposalAction.ACCEPTED,'UI approval changed')
 assert(equal(items(DealDirection.INCOMING),x.inspection.incoming.value) and
  equal(items(DealDirection.OUTGOING),x.inspection.outgoing.value),'inspected terms changed')
 terms(DealDirection.OUTGOING);terms(DealDirection.INCOMING)
 assert(equal(x.before,rosterSnapshot()) and Players[0]:GetTreasury():GetGoldBalance()==x.goldBefore,'baseline changed')
 x.acceptAttempted=true;x.acceptAt=UI.GetElapsedTime();witness()
 local ok,err=pcall(DealManager.SendWorkingDeal,DealProposalAction.ACCEPTED,0,1)
 if not ok then x.sendError=tostring(err)end
 return 'ACCEPTED_ONCE_OR_UNKNOWN'
end
function F.Complete()
 if x.skipped then return true end
 assert(x.acceptAttempted,'accept not handled')
 if x.sendError or UI.GetElapsedTime()-x.acceptAt>=30 then return true end
 local ok,v=pcall(function()
  local n=rosterSnapshot();return #n.captured==2 and #n.map==1 and #n.activeOffMap==0 and
   Players[0]:GetTreasury():GetGoldBalance()==x.goldBefore-61 and not DealManager.HasPendingDeal(0,1) and
   Players[1]:GetDiplomacy():HasOpenBordersFrom(0)==true
 end);return ok and v==true
end
function F.Publish()
 assert(x.phase=='INSPECTED' and F.Complete(),'outcome not ready')
 local r={returned=false,stage=x.skipped and 'NOT_SUBMITTED' or 'UNKNOWN',source='STOCK_UI_AND_NATIVE',
  roster=read(rosterSnapshot),gold=read(function()return Players[0]:GetTreasury():GetGoldBalance()end),
  pending=read(function()return DealManager.HasPendingDeal(0,1)end),
  border=read(function()return Players[1]:GetDiplomacy():HasOpenBordersFrom(0)end),
  outgoing=read(function()return items(DealDirection.OUTGOING)end),incoming=read(function()return items(DealDirection.INCOMING)end)}
 if x.acceptAttempted and not x.sendError and not x.overflow and r.roster.status=='OK' and
  r.gold.status=='OK' and r.gold.value==x.goldBefore-61 and r.pending.status=='OK' and r.pending.value==false and
  r.border.status=='OK' and r.border.value==true and equal(Game:GetProperty(OLD),x.old) and equal(Game:GetProperty(FIXED),x.fixed)then
  local n=r.roster.value
  if n.turn==62 and n.capacity==3 and n.physical==3 and equal(n.histories,x.before.histories) and
   #n.captured==2 and #n.map==1 and #n.activeOffMap==0 then
   local same=true;for _,c in ipairs(n.captured)do if c.captor~=1 or c.nameIndex~=-1 or c.name~='' or c.level~=1 then same=false end end
   local u=n.map[1]
   if same and u.owner==0 and u.unitType==GameInfo.Units['UNIT_SPY'].Index and integer(u.id,0) and
    integer(u.x,0) and integer(u.y,0) and Map.GetPlot(u.x,u.y)then r.returned=true;r.stage='RETURNED_MAP';r.unit=copy(u)end
  end
 end
 x.result=r;x.phase='OUTCOME';witness();return r.stage
end
function F.Close()
 assert(x.phase=='OUTCOME' and not x.closed,'outcome required')
 x.cleanup=read(function()
  local id=DiplomacyManager.FindOpenSessionID(0,1);if id~=nil then session();DiplomacyManager.CloseSession(id)end
  return true
 end);x.closed=true
 if ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey==tooltip then ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey=priorTooltip end
 return 'CLOSED_OR_UNKNOWN'
end
function F.Evidence()
 local e=copy(x);e.saved=read(function()return Game:GetProperty(KEY)end);return serial(e,0)
end
local function finalState()
 local e={returned=false,source='POST_CLOSE_UI_NATIVE',
  roster=read(rosterSnapshot),gold=read(function()return Players[0]:GetTreasury():GetGoldBalance()end),
  pending=read(function()return DealManager.HasPendingDeal(0,1)end),
  border=read(function()return Players[1]:GetDiplomacy():HasOpenBordersFrom(0)end),
  oldIntact=read(function()return equal(Game:GetProperty(OLD),x.old)end),
  fixedIntact=read(function()return equal(Game:GetProperty(FIXED),x.fixed)end)}
 e.returned=x.closed==true and x.acceptAttempted==true and x.result.returned==true and
  Game.GetCurrentGameTurn()==62 and e.roster.status=='OK' and equal(e.roster.value,x.result.roster.value) and
  e.gold.status=='OK' and e.gold.value==x.goldBefore-61 and e.pending.status=='OK' and e.pending.value==false and
  e.border.status=='OK' and e.border.value==true and e.oldIntact.status=='OK' and e.oldIntact.value==true and
  e.fixedIntact.status=='OK' and e.fixedIntact.value==true
 return e
end
function F.FinalCheck()
 assert(x.phase=='OUTCOME' and x.closed==true,'close after outcome first')
 x.finalCheck=finalState();x.phase='FINAL_CHECK';witness();return serial(x.finalCheck,0)
end
function F.AssertFinal()
 assert(x.phase=='FINAL_CHECK','final check missing')
 local now=finalState();local p=assert(Game:GetProperty(KEY))
 local diagnostic={current=now,savedOutcomeReturned=p.result.returned,savedRecordAnomaly=p.recordAnomaly,
  savedFinalReturned=p.finalCheck.returned,persistedOutcomeMismatch=p.finalCheck.persistedOutcomeMismatch}
 assert(now.returned==true and x.finalCheck.returned==true and p.phase=='FINAL_CHECK' and p.nonce==x.nonce and
  p.recordAnomaly==nil and p.result.returned==true and equal(p.result,x.result) and
  equal(p.finalCheck,x.finalCheck) and equal(now,x.finalCheck),'post-close return not proven: '..serial(diagnostic,0))
 return serial(now,0)
end
return F.Prepare()
"#;
fn record(phase: &str, prior: &str) -> String {
    format!(
        r#"
local w=assert(ExposedMembers.JAC_Barter1665Witness)
assert(w.version==1 and w.phase=='{phase}' and w.price==61 and w.borderDuration==30 and type(w.nonce)=='string')
local function same(a,b,d)
 if d>40 or type(a)~=type(b)then return false end;if type(a)~='table'then return a==b end
 for k,v in pairs(a)do if not same(v,b[k],d+1)then return false end end
 for k in pairs(b)do if a[k]==nil then return false end end;return true
end
local p=Game:GetProperty('{KEY}')
if '{prior}'=='ABSENT'then assert(p==nil and w.acceptAttempted==nil)
else assert(p and p.nonce==w.nonce and p.phase=='{prior}')end
local intact=Game.GetCurrentGameTurn()==62 and same(Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62'),w.old,0) and
 same(Game:GetProperty('JAC_BOUNDARY_FIXED_OFFER_62_V1'),w.fixed,0)
if '{phase}'=='OUTCOME'then
 assert(type(w.result)=='table')
 if not intact then w.recordAnomaly='TURN_OR_PAST_EVIDENCE_CHANGED';w.result.returned=false;w.result.stage='UNKNOWN_RECORD_ANOMALY'end
elseif '{phase}'=='FINAL_CHECK'then
 assert(w.closed==true and type(w.finalCheck)=='table')
 -- Core's saved OUTCOME is authoritative and sticky. A fresh UI witness
 -- must not erase an anomaly after the external state happens to recover.
 local final=w.finalCheck
 if not intact then final.returned=false;final.recordAnomaly='TURN_OR_PAST_EVIDENCE_CHANGED'end
 if p.recordAnomaly~=nil or not same(p.result,w.result,0) or p.result.returned~=true then
  final.returned=false;final.persistedOutcomeMismatch=true
 end
 p.finalCheck=final;p.closed=true;p.finalCleanup=w.cleanup;p.phase='FINAL_CHECK'
 Game:SetProperty('{KEY}',p);return p.phase
else assert(intact,'past evidence changed')end
Game:SetProperty('{KEY}',w);return w.phase
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
    json!({"action":"save","name":"JAC-Barter-Captive-Offer","exact_name":true,"checkpoint":true,"preserve_diplomacy_session":true})
}
fn wait(code: &str) -> Value {
    json!({"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"InGame","code":code,"preserve_diplomacy_session":true}})
}
fn scene() -> Value {
    json!({"name":"Distinct 30 turn open borders plus 61 gold barter; stock UI gated acceptance","judge_profile":"generic","strict_lua_errors":true,"steps":[
      {"action":"load","name":SAVE,"exact_name":true,"timeout":300,"save_sha256":SHA,"resume_once":true},
      wait("return Players[0] and Players[0]:IsTurnActive() and ExposedMembers.JAC_PersonaBoundary~=nil"),
      ui(setup()),core(record("PREPARED","ABSENT")),save(),ui("return JAC_Barter1665.Open()"),
      wait("return JAC_Barter1665.InspectionComplete()"),ui("return JAC_Barter1665.Inspect()"),
      core(record("INSPECTED","PREPARED")),save(),ui("return JAC_Barter1665.AcceptIfPermitted()"),save(),
      wait("return JAC_Barter1665.Complete()"),ui("return JAC_Barter1665.Publish()"),
      core(record("OUTCOME","INSPECTED")),save(),ui("return JAC_Barter1665.Close()"),
      ui("return JAC_Barter1665.FinalCheck()"),core(record("FINAL_CHECK","OUTCOME")),save(),
      ui("return JAC_Barter1665.Evidence()"),ui("return JAC_Barter1665.AssertFinal()"),
      {"action":"exit_to_menu"}
    ]})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().skip(1).collect();
    if a.len() != 2 {
        return Err("usage: barter-captive-62 LATEST_SAVE OUTPUT".into());
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
    lua.load(r#"
GOLD_BALANCE=61.6953125;PENDING=false;BORDER=false;UI_HIDDEN=false;BUTTON_HIDDEN=false;BUTTON_DISABLED=false
UNACCEPTABLE=false;MODE='ACCEPT';INSPECT_ACTION=2;DELAY=false;INSPECT_INITIATOR=0
DealItemTypes.AGREEMENTS=8;DealProposalAction.INSPECT=99
GameInfo.DiplomaticActions={DIPLOACTION_OPEN_BORDERS={Hash=-242352742}}
PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62={stage='QUOTED_UNKNOWN',nonce='53.430954889496:62',
 quote={raw={reply={DealAction=10,RespondingToDealAction=6}}}}
PROPERTIES.JAC_BOUNDARY_FIXED_OFFER_62_V1={phase='OUTCOME',sent=true,price=61,
 result={stage='REJECTED',returned=false,responses={{DealAction=2,RespondingToDealAction=4}}}}
Players[1].GetDiplomacy=function()return {
 IsAtWarWith=function()return false end,
 HasOpenBordersFrom=function(_,from)assert(from==0);return BORDER end}end
ContextPtr={LookUpControl=function(_,path)
 if path=='/InGame/DiplomacyDealView'then return {IsHidden=function()return UI_HIDDEN end}end
 if path=='/InGame/DiplomacyDealView/AcceptDeal'then return {
  IsHidden=function()return BUTTON_HIDDEN end,IsDisabled=function()return BUTTON_DISABLED end}end
end}
local add=OUT.AddItemOfType
function OUT:AddItemOfType(kind,from)
 local i=add(self,kind,from)
 function i:GetSubType()return self.subType end
 function i:SetSubType(v)self.subType=v end
 function i:SetDuration(v)self.duration=v end
 function i:SetAmount(v)self.amount=v end
 function i:IsUnacceptable()return UNACCEPTABLE end
 function i:GetMaxAmount()return math.floor(GOLD_BALANCE)end
 return i
end
function OUT:HasUnacceptableItems()return UNACCEPTABLE end
function DealManager.HasPendingDeal()return PENDING end
function DealManager.GetPossibleDealItems(from,to,kind)
 if kind==DealItemTypes.AGREEMENTS then return {{SubType=-242352742,Duration=30,IsValid=true,ValidationResult=1}}end
 return {{ForType=-1,IsValid=true,ValidationResult=1},{ForType=-1,IsValid=true,ValidationResult=1},{ForType=-1,IsValid=true,ValidationResult=1}}
end
function emitInspection()
 IN.items=OUT.items
 Events.DiplomacyStatement.fire(1,0,{SessionID=7,Initiator=INSPECT_INITIATOR,StatementType=5,
  DealAction=INSPECT_ACTION,RespondingToDealAction=99})
end
local request=DiplomacyManager.RequestSession
function DiplomacyManager.RequestSession(...)
 request(...);if not DELAY then emitInspection()end
end
function DealManager.SendWorkingDeal(action,from,to)
 assert(action==DealProposalAction.ACCEPTED and from==0 and to==1)
 SENDS[#SENDS+1]={action=action,from=from,to=to}
 if MODE=='THROW'then error('native send unknown')end
 if MODE=='ACCEPT'then CAPTIVE_COUNT=2;MAP_COUNT=1;GOLD_BALANCE=GOLD_BALANCE-61;BORDER=true end
end
"#).exec().unwrap();
    assert_eq!(lua.load(setup()).eval::<String>().unwrap(), "PREPARED");
    lua.load(record("PREPARED", "ABSENT")).exec().unwrap();
    lua
}
#[cfg(test)]
fn inspected(lua: &mlua::Lua) {
    lua.load("JAC_Barter1665.Open();if not JAC_Barter1665.InspectionComplete()then TIME=TIME+31 end;assert(JAC_Barter1665.InspectionComplete());JAC_Barter1665.Inspect()").exec().unwrap();
    lua.load(record("INSPECTED", "PREPARED")).exec().unwrap();
}
#[test]
fn ui_permitted_barter_pays_and_returns_measured_native_unit_once() {
    let lua = prepared();
    inspected(&lua);
    lua.load("JAC_Barter1665.AcceptIfPermitted();assert(JAC_Barter1665.Complete());assert(JAC_Barter1665.Publish()=='RETURNED_MAP')").exec().unwrap();
    lua.load(record("OUTCOME", "INSPECTED")).exec().unwrap();
    let raw:String=lua.load("JAC_Barter1665.Close();assert(#SENDS==1 and CLOSED==1 and GOLD_BALANCE==0.6953125 and BORDER);assert(H[1].turn==23 and H[2].turn==45 and H[3].turn==62);assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey==ORIGINAL_TOOLTIP);return JAC_Barter1665.Evidence()").eval().unwrap();
    let e: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(e["result"]["unit"]["id"], 77777);
    assert!(lua
        .load("JAC_Barter1665.AcceptIfPermitted()")
        .exec()
        .is_err());
}
#[test]
fn refused_unacceptable_hidden_or_disabled_ui_never_sends() {
    for change in [
        "INSPECT_ACTION=5",
        "BUTTON_HIDDEN=true",
        "BUTTON_DISABLED=true",
        "UI_HIDDEN=true",
        "ContextPtr.LookUpControl=function()return nil end",
        "INSPECT_INITIATOR=1;DELAY=true",
    ] {
        let lua = prepared();
        lua.load(change).exec().unwrap();
        if change.contains("DELAY") {
            lua.load("JAC_Barter1665.Open();emitInspection();assert(not JAC_Barter1665.InspectionComplete());TIME=TIME+31;assert(JAC_Barter1665.InspectionComplete());JAC_Barter1665.Inspect()").exec().unwrap();
            lua.load(record("INSPECTED", "PREPARED")).exec().unwrap();
        } else {
            inspected(&lua)
        }
        lua.load("assert(JAC_Barter1665.AcceptIfPermitted()=='NOT_SENT_UI_NOT_PERMITTED');assert(JAC_Barter1665.Complete());assert(JAC_Barter1665.Publish()=='NOT_SUBMITTED');assert(#SENDS==0 and GOLD_BALANCE==61.6953125 and CAPTIVE_COUNT==3)").exec().unwrap();
        lua.load(record("OUTCOME", "INSPECTED")).exec().unwrap();
        lua.load("JAC_Barter1665.Close();assert(CLOSED==1)")
            .exec()
            .unwrap();
    }
    let lua = prepared();
    lua.load("JAC_Barter1665.Open();UNACCEPTABLE=true;assert(not JAC_Barter1665.InspectionComplete());TIME=TIME+31;JAC_Barter1665.Inspect()").exec().unwrap();
    lua.load(record("INSPECTED", "PREPARED")).exec().unwrap();
    lua.load("JAC_Barter1665.AcceptIfPermitted();assert(#SENDS==0)")
        .exec()
        .unwrap();
}
#[test]
fn changed_saved_intent_ui_or_terms_prevent_acceptance() {
    for change in [
        "BUTTON_HIDDEN=true",
        "UNACCEPTABLE=true",
        "GOLD_BALANCE=60",
        "OUT.items[3].duration=29",
        "OUT.items[2].amount=60",
        "PROPERTIES.JAC_BOUNDARY_BARTER_OFFER_62_V1.nonce='other'",
        "PROPERTIES.JAC_BOUNDARY_FIXED_OFFER_62_V1.result.stage='changed'",
    ] {
        let lua = prepared();
        inspected(&lua);
        lua.load(change).exec().unwrap();
        assert!(
            lua.load("JAC_Barter1665.AcceptIfPermitted()")
                .exec()
                .is_err(),
            "{change}"
        );
        lua.load("assert(#SENDS==0)").exec().unwrap();
    }
}
#[test]
fn unknown_send_missing_payment_or_border_are_never_return_success() {
    for change in ["MODE='THROW'", "MODE='NONE'", "MODE='ACCEPT';BORDER=false"] {
        let lua = prepared();
        inspected(&lua);
        lua.load(change).exec().unwrap();
        lua.load("JAC_Barter1665.AcceptIfPermitted()")
            .exec()
            .unwrap();
        if change.contains("BORDER") {
            lua.load("BORDER=false;assert(not JAC_Barter1665.Complete())")
                .exec()
                .unwrap();
        }
        lua.load("TIME=TIME+31;assert(JAC_Barter1665.Complete());assert(JAC_Barter1665.Publish()=='UNKNOWN');assert(#SENDS==1)").exec().unwrap();
        assert!(lua
            .load("JAC_Barter1665.AcceptIfPermitted()")
            .exec()
            .is_err());
    }
}
#[test]
fn pending_inspect_then_late_settlement_are_waited_without_resend() {
    let lua = prepared();
    lua.load("DELAY=true;JAC_Barter1665.Open();PENDING=true;emitInspection();assert(not JAC_Barter1665.InspectionComplete());PENDING=false;assert(JAC_Barter1665.InspectionComplete());JAC_Barter1665.Inspect()").exec().unwrap();
    lua.load(record("INSPECTED", "PREPARED")).exec().unwrap();
    lua.load("MODE='NONE';JAC_Barter1665.AcceptIfPermitted();assert(not JAC_Barter1665.Complete());CAPTIVE_COUNT=2;MAP_COUNT=1;GOLD_BALANCE=GOLD_BALANCE-61;BORDER=true;assert(JAC_Barter1665.Complete());assert(JAC_Barter1665.Publish()=='RETURNED_MAP');assert(#SENDS==1)").exec().unwrap();
}
#[test]
fn generated_scene_pins_latest_save_and_retains_evidence_before_return_guard() {
    let s = scene();
    civ6_autotest_rs::scenario::validate_scenario(&s).unwrap();
    let steps = s["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 23);
    assert_eq!(steps[0]["resume_once"], true);
    assert_eq!(steps[0]["save_sha256"], SHA);
    for i in [4, 9, 11, 15] {
        assert_eq!(steps[i]["action"], "save")
    }
    assert!(steps[16]["code"].as_str().unwrap().contains(".Close()"));
    assert!(steps[17]["code"]
        .as_str()
        .unwrap()
        .contains(".FinalCheck()"));
    assert_eq!(steps[19]["action"], "save");
    assert!(steps[21]["code"]
        .as_str()
        .unwrap()
        .contains(".AssertFinal()"));
    let code = setup();
    assert_eq!(
        code.matches("pcall(DealManager.SendWorkingDeal,DealProposalAction.ACCEPTED")
            .count(),
        1
    );
    for forbidden in [
        "DealProposalAction.PROPOSED",
        "DealProposalAction.EQUALIZE",
        "SetNameIndex(",
        "SetName(",
        "UnitManager.Init",
    ] {
        assert!(!code.contains(forbidden))
    }
    for step in steps {
        if let Some(c) = step["code"].as_str() {
            mlua::Lua::new().load(c).into_function().unwrap();
        }
    }
}

#[test]
fn accepted_inspection_waits_for_ui_and_preserves_term_error() {
    let lua = prepared();
    lua.load("BUTTON_HIDDEN=true;JAC_Barter1665.Open();assert(not JAC_Barter1665.InspectionComplete());TIME=TIME+1;BUTTON_HIDDEN=false;assert(JAC_Barter1665.InspectionComplete());JAC_Barter1665.Inspect()").exec().unwrap();
    lua.load(record("INSPECTED", "PREPARED")).exec().unwrap();
    lua.load("JAC_Barter1665.AcceptIfPermitted();assert(#SENDS==1)")
        .exec()
        .unwrap();
    let lua = prepared();
    lua.load("JAC_Barter1665.Open();GOLD_BALANCE=60;assert(not JAC_Barter1665.InspectionComplete());TIME=TIME+31;JAC_Barter1665.Inspect()").exec().unwrap();
    lua.load(record("INSPECTED", "PREPARED")).exec().unwrap();
    let e: String = lua
        .load(
            "JAC_Barter1665.AcceptIfPermitted();assert(#SENDS==0);return JAC_Barter1665.Evidence()",
        )
        .eval()
        .unwrap();
    let e: Value = serde_json::from_str(&e).unwrap();
    assert_eq!(e["inspection"]["terms"]["status"], "ERROR");
    assert!(e["inspection"]["terms"]["error"]
        .as_str()
        .unwrap()
        .contains("inspection budget changed"));
}
#[test]
fn late_border_reflection_and_api_errors_wait_until_ready_or_timeout() {
    for mode in ["delay", "error"] {
        let lua = prepared();
        inspected(&lua);
        lua.load(
            "JAC_Barter1665.AcceptIfPermitted();BORDER=false;assert(not JAC_Barter1665.Complete())",
        )
        .exec()
        .unwrap();
        if mode == "delay" {
            lua.load("TIME=TIME+1;assert(not JAC_Barter1665.Complete());BORDER=true;assert(JAC_Barter1665.Complete());assert(JAC_Barter1665.Publish()=='RETURNED_MAP')").exec().unwrap();
        } else {
            lua.load("Players[1].GetDiplomacy=function()error('border API unavailable')end;assert(not JAC_Barter1665.Complete());TIME=TIME+31;assert(JAC_Barter1665.Complete());assert(JAC_Barter1665.Publish()=='UNKNOWN')").exec().unwrap();
            let e: String = lua.load("return JAC_Barter1665.Evidence()").eval().unwrap();
            let e: Value = serde_json::from_str(&e).unwrap();
            assert_eq!(e["result"]["border"]["status"], "ERROR");
        }
    }
}
#[test]
fn post_outcome_changes_preserve_original_and_fail_current_final_verification() {
    for changed in [
        "H[3].turn=63",
        "MAP_ID=88888",
        "GOLD_BALANCE=1",
        "BORDER=false",
        "PROPERTIES.JAC_BOUNDARY_FIXED_OFFER_62_V1.result.stage='changed'",
        "PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.nonce='changed'",
    ] {
        let lua = prepared();
        inspected(&lua);
        lua.load("JAC_Barter1665.AcceptIfPermitted();JAC_Barter1665.Publish()")
            .exec()
            .unwrap();
        lua.load(record("OUTCOME", "INSPECTED")).exec().unwrap();
        lua.load(changed).exec().unwrap();
        lua.load("JAC_Barter1665.Close();JAC_Barter1665.FinalCheck()")
            .exec()
            .unwrap();
        lua.load(record("FINAL_CHECK", "OUTCOME")).exec().unwrap();
        lua.load("assert(PROPERTIES.JAC_BOUNDARY_BARTER_OFFER_62_V1.result.returned==true and PROPERTIES.JAC_BOUNDARY_BARTER_OFFER_62_V1.finalCheck.returned==false)").exec().unwrap();
        assert!(
            lua.load("JAC_Barter1665.AssertFinal()").exec().is_err(),
            "{changed}"
        );
    }
    let lua = prepared();
    inspected(&lua);
    lua.load("JAC_Barter1665.AcceptIfPermitted();JAC_Barter1665.Publish()")
        .exec()
        .unwrap();
    lua.load(record("OUTCOME", "INSPECTED")).exec().unwrap();
    lua.load("JAC_Barter1665.Close();JAC_Barter1665.FinalCheck()")
        .exec()
        .unwrap();
    lua.load(record("FINAL_CHECK", "OUTCOME")).exec().unwrap();
    lua.load("JAC_Barter1665.AssertFinal();MAP_ID=99999")
        .exec()
        .unwrap();
    assert!(lua.load("JAC_Barter1665.AssertFinal()").exec().is_err());
}

#[test]
fn core_outcome_anomaly_cannot_be_erased_by_a_recovered_ui_witness() {
    let lua = prepared();
    inspected(&lua);
    lua.load("JAC_Barter1665.AcceptIfPermitted();JAC_Barter1665.Publish();ORIGINAL_NONCE=PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.nonce;PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.nonce='temporary-anomaly'").exec().unwrap();
    lua.load(record("OUTCOME", "INSPECTED")).exec().unwrap();
    lua.load("local p=PROPERTIES.JAC_BOUNDARY_BARTER_OFFER_62_V1;assert(p.recordAnomaly and p.result.returned==false);SAVED_ANOMALOUS_RESULT=p.result;PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.nonce=ORIGINAL_NONCE;JAC_Barter1665.Close();JAC_Barter1665.FinalCheck()").exec().unwrap();
    lua.load(record("FINAL_CHECK", "OUTCOME")).exec().unwrap();
    lua.load("local p=PROPERTIES.JAC_BOUNDARY_BARTER_OFFER_62_V1;assert(p.result==SAVED_ANOMALOUS_RESULT and p.recordAnomaly=='TURN_OR_PAST_EVIDENCE_CHANGED' and p.result.returned==false and p.finalCheck.returned==false and p.finalCheck.persistedOutcomeMismatch)").exec().unwrap();
    assert!(lua.load("JAC_Barter1665.AssertFinal()").exec().is_err());
}
