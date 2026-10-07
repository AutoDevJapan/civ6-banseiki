//! A distinct stock-UI quote and sale to peer21, never the old captive equalization.
#[allow(dead_code)]
#[path = "../src/spy_group_exchange.rs"]
mod legacy;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
const SAVE: &str = "JAC-Barter-Captive-Offer-d7c7af495d2d4a43a6b2a4f366421215-0000000000000005";
const SHA: &str = "6529c3edefbd5db0a36c5bfb6c23ae066c78fee52448c7d3954c1d6dfbd10487";
const KEY: &str = "JAC_BOUNDARY_BORDER_SALE_62_21_V1";
fn setup() -> String {
    let a = legacy::UI_SETUP.find("local function integer").unwrap();
    let b = legacy::UI_SETUP
        .find("local function sameBaseline")
        .unwrap();
    format!(
        "local B=assert(ExposedMembers.JAC_PersonaBoundary);\n{}\n{SALE}",
        &legacy::UI_SETUP[a..b]
    )
}
const SALE: &str = r#"
assert(JAC_BorderSale1665==nil,'sale instance exists')
local F={};JAC_BorderSale1665=F
local KEY='JAC_BOUNDARY_BORDER_SALE_62_21_V1';local WKEY='JAC_BorderSale1665Witness'
local KEYS={'JAC_BOUNDARY_GROUP_EXCHANGE_62','JAC_BOUNDARY_FIXED_OFFER_62_V1','JAC_BOUNDARY_BARTER_OFFER_62_V1'}
local x={version=1,peer=21,phase='INIT',statements={}}
local function quote(s)return '"'..tostring(s):gsub('[%z\1-\31\\"]',function(c)
 if c=='"'then return '\\"'end;if c=='\\'then return '\\\\'end;return string.format('\\u%04x',string.byte(c))end)..'"'end
local function serial(v,d)
 assert(d<40,'depth');local t=type(v);if t=='nil'then return 'null'elseif t=='string'then return quote(v)
 elseif t=='boolean'or t=='number'then return tostring(v)end;assert(t=='table','scalar/table required')
 local k={};for a in pairs(v)do k[#k+1]=a end;table.sort(k,function(a,b)return tostring(a)<tostring(b)end)
 local r={};for _,a in ipairs(k)do r[#r+1]=quote(a)..':'..serial(v[a],d+1)end;return '{'..table.concat(r,',')..'}'
end
local function equal(a,b)return serial(a,0)==serial(b,0)end
local function read(fn)local ok,v=pcall(fn);if ok then return {status='OK',kind=type(v),value=v}end;return {status='ERROR',error=tostring(v)}end
local function witness()ExposedMembers[WKEY]=copy(x)end
local function gold(id)return Players[id]:GetTreasury():GetGoldBalance()end
local function deal(dir)return assert(DealManager.GetWorkingDeal(dir,0,21))end
local function items(dir)
 local d=deal(dir);local r={};for i in d:Items()do assert(#r<8,'item limit');r[#r+1]={id=i:GetID(),kind=i:GetType(),
  from=i:GetFromPlayerID(),to=i:GetToPlayerID(),sub=i:GetSubType(),duration=i:GetDuration(),amount=i:GetAmount(),
  valid=i:IsValid(),unacceptable=i:IsUnacceptable(),locked=i:IsLocked()}end
 assert(#r==d:GetItemCount(),'item enumeration');return r
end
local function intact()
 if Game.GetCurrentGameTurn()~=62 then return false end
 for _,k in ipairs(KEYS)do if not equal(Game:GetProperty(k),x.past[k])then return false end end
 return equal(rosterSnapshot(),x.roster)
end
local function guard()
 assert(Game.GetLocalPlayer()==0 and Players[0]:IsHuman() and not Players[21]:IsHuman() and
 Players[0]:IsTurnActive() and Players[21]:IsAlive() and Players[0]:GetDiplomacy():HasMet(21) and
 not Players[0]:GetDiplomacy():IsAtWarWith(21) and intact(),'roles/lineage changed')
end
local function session()
 local id=DiplomacyManager.FindOpenSessionID(0,21)
 assert(id~=nil and id==x.sessionID and DiplomacyManager.GetSessionInfo(id).FromPlayer==0,'owned session differs');return id
end
local function replies(action)
 local r={};for _,v in ipairs(x.statements)do local ok,k=pcall(DiplomacyManager.GetKeyName,v.StatementType)
  if ok and k=='MAKE_DEAL' and v.from==21 and v.to==0 and v.Initiator==0 and v.SessionID==x.sessionID and
   v.RespondingToDealAction==action then r[#r+1]=v end end;return r
end
local function ui(button)
 session();local c=assert(ContextPtr:LookUpControl('/InGame/DiplomacyDealView'))
 local b=assert(ContextPtr:LookUpControl('/InGame/DiplomacyDealView/'..button))
 return {contextHidden=c:IsHidden(),hidden=b:IsHidden(),disabled=b:IsDisabled(),pending=DealManager.HasPendingDeal(0,21),
  valid=deal(DealDirection.OUTGOING):Validate(),unacceptable=deal(DealDirection.OUTGOING):HasUnacceptableItems(),
  equal=DealManager.AreWorkingDealsEqual(0,21)}
end
local function permitted(v)return v.contextHidden==false and v.hidden==false and v.disabled==false and v.pending==false and
 v.valid==DealValidationResult.VALID and v.unacceptable==false end
local function terms(dir,requireGold)
 local rows=items(dir);local border,n,amount=0,0,nil
 for _,r in ipairs(rows)do assert(r.valid==true and r.unacceptable==false,'term invalid/unacceptable')
  if r.kind==DealItemTypes.AGREEMENTS then assert(r.from==0 and r.to==21 and r.sub==-242352742 and r.duration==30 and r.locked==true,'locked border changed');border=border+1
  elseif r.kind==DealItemTypes.GOLD then assert(r.from==21 and r.to==0 and r.duration==0 and integer(r.amount,1) and r.amount<=math.floor(x.peerGold),'unsafe money');n=n+1;amount=r.amount
  else error('unexpected sale term')end end
 assert(border==1 and ((requireGold and n==1 and #rows==2)or(not requireGold and n==0 and #rows==1)),'sale shape changed')
 return {rows=rows,amount=amount}
end
Events.DiplomacyStatement.Add(function(from,to,k)
 if x.closed or not x.openIntent or type(k)~='table' or not ((from==0 and to==21)or(from==21 and to==0))then return end
 if #x.statements>=64 then x.overflow=true;return end
 local r={from=from,to=to};for _,a in ipairs({'SessionID','Initiator','StatementType','StatementSubType','DealAction','RespondingToDealAction'})do r[a]=k[a]end;x.statements[#x.statements+1]=r
end)
function F.Prepare()
 assert(x.phase=='INIT' and Game:GetProperty(KEY)==nil,'sale already exists')
 local p=assert(Game:GetProperty(KEYS[3]));assert(p.phase=='FINAL_CHECK' and p.closed==true and p.skipped==true and
 p.result.stage=='NOT_SUBMITTED' and p.result.returned==false and p.acceptAttempted==nil,'latest unsent barter outcome required')
 x.past={};for _,k in ipairs(KEYS)do x.past[k]=copy(assert(Game:GetProperty(k)))end
 x.roster=rosterSnapshot();assert(x.roster.capacity==3 and #x.roster.captured==3 and #x.roster.map==0 and #x.roster.activeOffMap==0,'roster changed')
 guard();assert(not DealManager.HasPendingDeal(0,21) and DiplomacyManager.FindOpenSessionID(0,21)==nil and
 #items(DealDirection.OUTGOING)==0 and #items(DealDirection.INCOMING)==0,'existing deal/session')
 x.gold=gold(0);x.peerGold=gold(21);assert(x.gold==61.6953125 and x.peerGold>=1,'funding baseline changed')
 x.borderBefore=Players[21]:GetDiplomacy():HasOpenBordersFrom(0);assert(x.borderBefore==false,'border already active')
 local n=0;for _,c in ipairs(assert(DealManager.GetPossibleDealItems(0,21,DealItemTypes.AGREEMENTS,deal(DealDirection.OUTGOING))))do
  if c.SubType==-242352742 then assert(c.Duration==30 and(c.IsValid==true or c.IsValid==1)and c.ValidationResult==DealValidationResult.VALID,'candidate invalid');n=n+1 end end
 assert(n==1,'measured border candidate changed')
 x.nonce='border-sale21:'..tostring(UI.GetElapsedTime())..':62';x.phase='PREPARED';witness();return x.phase
end
function F.Open()
 guard();assert(x.phase=='PREPARED' and not x.openIntent and Game:GetProperty(KEY).nonce==x.nonce,'open intent differs')
 assert(not DealManager.HasPendingDeal(0,21) and DiplomacyManager.FindOpenSessionID(0,21)==nil and
  #items(DealDirection.OUTGOING)==0 and #items(DealDirection.INCOMING)==0 and gold(0)==x.gold and gold(21)==x.peerGold,'state changed before open')
 x.openIntent=true;local a=assert(deal(DealDirection.OUTGOING):AddItemOfType(DealItemTypes.AGREEMENTS,0));a:SetSubType(-242352742);a:SetDuration(30);a:SetLocked(true)
 x.original=terms(DealDirection.OUTGOING,false);x.openAt=UI.GetElapsedTime();DiplomacyManager.RequestSession(0,21,'MAKE_DEAL');return 'BORDER_OPENED'
end
function F.InspectReady()
 local id=DiplomacyManager.FindOpenSessionID(0,21);if id~=nil then assert(DiplomacyManager.GetSessionInfo(id).FromPlayer==0);x.sessionID=id end
 if UI.GetElapsedTime()-x.openAt>=30 or x.overflow then return true end
 if id==nil or DealManager.HasPendingDeal(0,21)or #replies(DealProposalAction.INSPECT)==0 then return false end
 local v=read(function()return ui('EqualizeDeal')end);return v.status=='OK' and permitted(v.value)
end
function F.ArmQuote()
 guard();assert(F.InspectReady() and x.phase=='PREPARED')
 x.inspection={responses=replies(DealProposalAction.INSPECT),ui=read(function()return ui('EqualizeDeal')end),terms=read(function()return terms(DealDirection.OUTGOING,false)end)}
 x.quoteAllowed=not x.overflow and #x.inspection.responses==1 and x.inspection.ui.status=='OK' and permitted(x.inspection.ui.value) and
 x.inspection.terms.status=='OK' and gold(0)==x.gold and gold(21)==x.peerGold
 x.phase='QUOTE_INTENT';witness();return x.quoteAllowed and 'NEW_QUOTE_PERMITTED' or 'NOT_PERMITTED'
end
function F.QuoteOnce()
 guard();assert(x.phase=='QUOTE_INTENT' and not x.quoteAttempted and not x.quoteSkipped)
 local p=assert(Game:GetProperty(KEY));assert(p.phase==x.phase and p.nonce==x.nonce and p.quoteAllowed==x.quoteAllowed and equal(p.inspection,x.inspection))
 if not x.quoteAllowed then x.quoteSkipped=true;witness();return 'QUOTE_NOT_SENT'end
 local observed=replies(DealProposalAction.INSPECT)
 if x.overflow or #observed~=1 or not equal(observed,x.inspection.responses)then
  x.quoteSkipped=true;x.quoteSkipReason='INSPECTION_CHANGED_AFTER_SAVE';x.quoteSkipResponses=copy(observed);witness();return 'QUOTE_NOT_SENT_RESPONSE_CHANGED'
 end
 session();assert(permitted(ui('EqualizeDeal')) and gold(0)==x.gold and gold(21)==x.peerGold)
 -- Native IN->OUT copying may renumber IDs; terms() validates all economic
 -- fields. Preserve both ID sets without treating an ID as the price.
 x.beforeQuoteTerms=terms(DealDirection.OUTGOING,false)
 x.quoteAttempted=true;x.quoteAt=UI.GetElapsedTime();witness()
 local ok,e=pcall(DealManager.SendWorkingDeal,DealProposalAction.EQUALIZE,0,21);if not ok then x.quoteError=tostring(e)end;return 'NEW_PEER_QUOTE_ONCE_OR_UNKNOWN'
end
function F.QuoteReady()
 if x.quoteSkipped then return true end;assert(x.quoteAttempted)
 if x.quoteError or x.overflow or UI.GetElapsedTime()-x.quoteAt>=30 then return true end
 local r=replies(DealProposalAction.EQUALIZE);if #r==0 or DealManager.HasPendingDeal(0,21)then return false end
 if #r~=1 then return true end
 local a=r[1].DealAction;if a~=DealProposalAction.ACCEPTED and a~=DealProposalAction.ADJUSTED and a~=DealProposalAction.PROPOSED then return true end
 local v=read(function()return ui('AcceptDeal')end);return v.status=='OK' and permitted(v.value) and v.value.equal==true
end
function F.RecordQuote()
 guard();assert(x.phase=='QUOTE_INTENT' and F.QuoteReady())
 x.quote={responses=replies(DealProposalAction.EQUALIZE),ui=read(function()return ui('AcceptDeal')end),
 outgoingRaw=read(function()return items(DealDirection.OUTGOING)end),incomingRaw=read(function()return items(DealDirection.INCOMING)end),
 outgoing=read(function()return terms(DealDirection.OUTGOING,true)end),incoming=read(function()return terms(DealDirection.INCOMING,true)end)}
 local q=x.quote;local a=#q.responses==1 and q.responses[1].DealAction or nil
 x.acceptAllowed=x.quoteAttempted==true and not x.quoteError and not x.overflow and
 (a==DealProposalAction.ACCEPTED or a==DealProposalAction.ADJUSTED or a==DealProposalAction.PROPOSED)and
 q.ui.status=='OK' and permitted(q.ui.value)and q.ui.value.equal==true and q.outgoing.status=='OK' and q.incoming.status=='OK' and
 q.outgoing.value.amount==q.incoming.value.amount and gold(0)==x.gold and gold(21)==x.peerGold
 if x.acceptAllowed then x.amount=q.outgoing.value.amount end;x.phase='QUOTED';witness();return x.acceptAllowed and 'LEGAL_FINANCE_QUOTE' or 'NO_SAFE_QUOTE'
end
function F.AcceptOnce()
 guard();assert(x.phase=='QUOTED' and not x.acceptAttempted and not x.acceptSkipped)
 local p=assert(Game:GetProperty(KEY));assert(p.phase==x.phase and p.nonce==x.nonce and equal(p.quote,x.quote) and p.acceptAllowed==x.acceptAllowed and p.amount==x.amount)
 if not x.acceptAllowed then x.acceptSkipped=true;witness();return 'ACCEPT_NOT_SENT'end
 local observed=replies(DealProposalAction.EQUALIZE)
 if x.overflow or #observed~=1 or not equal(observed,x.quote.responses)then
  x.acceptSkipped=true;x.acceptSkipReason='QUOTE_CHANGED_AFTER_SAVE';x.acceptSkipResponses=copy(observed);witness();return 'ACCEPT_NOT_SENT_RESPONSE_CHANGED'
 end
 session();local u=ui('AcceptDeal');assert(permitted(u)and u.equal==true and gold(0)==x.gold and gold(21)==x.peerGold)
 assert(equal(terms(DealDirection.OUTGOING,true),x.quote.outgoing.value)and equal(terms(DealDirection.INCOMING,true),x.quote.incoming.value),'quoted terms changed')
 x.acceptAttempted=true;x.acceptAt=UI.GetElapsedTime();witness()
 local ok,e=pcall(DealManager.SendWorkingDeal,DealProposalAction.ACCEPTED,0,21);if not ok then x.acceptError=tostring(e)end;return 'SALE_ACCEPTED_ONCE_OR_UNKNOWN'
end
local function settlement()
 return {funded=false,gold=read(function()return gold(0)end),peerGold=read(function()return gold(21)end),
 border=read(function()return Players[21]:GetDiplomacy():HasOpenBordersFrom(0)end),pending=read(function()return DealManager.HasPendingDeal(0,21)end),
 lineage=read(intact),roster=read(rosterSnapshot)}
end
local function funded(r)
 return x.acceptAttempted==true and not x.acceptError and not x.overflow and r.lineage.status=='OK' and r.lineage.value==true and
 r.gold.status=='OK' and r.gold.value==x.gold+x.amount and r.peerGold.status=='OK' and r.peerGold.value==x.peerGold-x.amount and
 r.border.status=='OK' and r.border.value==true and r.pending.status=='OK' and r.pending.value==false
end
function F.Complete()
 if x.acceptSkipped then return true end;assert(x.acceptAttempted)
 return x.acceptError~=nil or UI.GetElapsedTime()-x.acceptAt>=30 or funded(settlement())
end
function F.Publish()
 assert(x.phase=='QUOTED' and F.Complete());x.result=settlement();x.result.funded=funded(x.result)
 x.result.stage=x.result.funded and 'FUNDED_NATIVE' or(x.acceptSkipped and 'NOT_SUBMITTED' or 'UNKNOWN');x.phase='OUTCOME';witness();return x.result.stage
end
function F.Close()
 assert(x.phase=='OUTCOME' and not x.closed);x.cleanup=read(function()local id=DiplomacyManager.FindOpenSessionID(0,21)
 if id~=nil then session();DiplomacyManager.CloseSession(id)end;return true end);x.closed=true;witness();return 'CLOSED_OR_UNKNOWN'
end
function F.Evidence()local e=copy(x);e.saved=read(function()return Game:GetProperty(KEY)end);e.current=settlement();e.current.funded=funded(e.current);return serial(e,0)end
function F.Final()
 local p=assert(Game:GetProperty(KEY));local r=settlement();r.funded=funded(r)
 assert(x.closed and p.phase=='OUTCOME' and p.recordAnomaly==nil and p.result.funded==true and equal(p.result,x.result) and r.funded,
 'normal border sale not proven: '..serial(r,0));return serial(r,0)
end
return F.Prepare()
"#;
fn record(phase: &str, prior: &str) -> String {
    format!(
        r#"
local w=assert(ExposedMembers.JAC_BorderSale1665Witness);assert(w.phase=='{phase}' and w.peer==21 and w.version==1)
local function same(a,b,d)if d>40 or type(a)~=type(b)then return false end;if type(a)~='table'then return a==b end
 for k,v in pairs(a)do if not same(v,b[k],d+1)then return false end end;for k in pairs(b)do if a[k]==nil then return false end end;return true end
local p=Game:GetProperty('{KEY}');if '{prior}'=='ABSENT'then assert(p==nil)else assert(p and p.nonce==w.nonce and p.phase=='{prior}')end
local intact=Game.GetCurrentGameTurn()==62;for k,v in pairs(w.past)do if not same(Game:GetProperty(k),v,0)then intact=false end end
if '{phase}'=='OUTCOME'then if not intact then w.recordAnomaly='PAST_OR_TURN_CHANGED';w.result.funded=false;w.result.stage='UNKNOWN_RECORD_ANOMALY'end
else assert(intact,'past changed')end;Game:SetProperty('{KEY}',w);return w.phase
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
    json!({"action":"save","name":"JAC-Border-Sale-21","exact_name":true,"checkpoint":true,"preserve_diplomacy_session":true})
}
fn wait(code: &str) -> Value {
    json!({"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"InGame","code":code,"preserve_diplomacy_session":true}})
}
fn scene() -> Value {
    json!({"name":"New peer21 border sale financing; no captive equalization replay","judge_profile":"generic","strict_lua_errors":true,"steps":[
    {"action":"load","name":SAVE,"exact_name":true,"timeout":300,"save_sha256":SHA,"resume_once":true},
    wait("return Players[0] and Players[0]:IsTurnActive() and ExposedMembers.JAC_PersonaBoundary~=nil"),ui(setup()),core(record("PREPARED","ABSENT")),save(),ui("return JAC_BorderSale1665.Open()"),
    wait("return JAC_BorderSale1665.InspectReady()"),ui("return JAC_BorderSale1665.ArmQuote()"),core(record("QUOTE_INTENT","PREPARED")),save(),ui("return JAC_BorderSale1665.QuoteOnce()"),save(),
    wait("return JAC_BorderSale1665.QuoteReady()"),ui("return JAC_BorderSale1665.RecordQuote()"),core(record("QUOTED","QUOTE_INTENT")),save(),ui("return JAC_BorderSale1665.AcceptOnce()"),save(),
    wait("return JAC_BorderSale1665.Complete()"),ui("return JAC_BorderSale1665.Publish()"),core(record("OUTCOME","QUOTED")),save(),ui("return JAC_BorderSale1665.Close()"),ui("return JAC_BorderSale1665.Evidence()"),ui("return JAC_BorderSale1665.Final()"),{"action":"exit_to_menu"}
    ]})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().skip(1).collect();
    if a.len() != 2 {
        return Err("usage: sell-borders-62 LATEST_SAVE OUTPUT".into());
    }
    if format!("{:x}", Sha256::digest(std::fs::read(&a[0])?)) != SHA {
        return Err("latest save differs".into());
    }
    let s = scene();
    civ6_autotest_rs::scenario::validate_scenario(&s)?;
    std::fs::write(&a[1], serde_json::to_vec_pretty(&s)?)?;
    Ok(())
}
#[test]
fn sequence_claims_only_latest_save_and_keeps_outcome_before_funding_guard() {
    let s = scene();
    civ6_autotest_rs::scenario::validate_scenario(&s).unwrap();
    let steps = s["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 26);
    assert_eq!(steps[0]["save_sha256"], SHA);
    assert_eq!(steps[0]["resume_once"], true);
    for st in steps {
        if let Some(c) = st["code"].as_str() {
            mlua::Lua::new().load(c).into_function().unwrap();
        }
    }
    assert!(steps[22]["code"].as_str().unwrap().contains("Close"));
    assert!(steps[24]["code"].as_str().unwrap().contains("Final"));
    assert_eq!(
        setup()
            .matches("pcall(DealManager.SendWorkingDeal,DealProposalAction.EQUALIZE,0,21)")
            .count(),
        1
    );
    for bad in [
        "SetNameIndex(",
        "UnitManager.Init",
        "SendWorkingDeal,DealProposalAction.EQUALIZE,0,1)",
    ] {
        assert!(!setup().contains(bad));
    }
}
#[cfg(test)]
#[path = "../tests/fixtures/spy_exchange_native.rs"]
mod native_fixture;
#[cfg(test)]
fn prepared() -> mlua::Lua {
    let l = mlua::Lua::new();
    l.load(native_fixture::NATIVE).exec().unwrap();
    l.load(r#"
GOLD_BALANCE=61.6953125;PEER_GOLD=672.8125;BORDER=false;EQ_HIDDEN=false;ACCEPT_HIDDEN=false;MODE='GOOD';QUOTE_AMOUNT=20
DealItemTypes.AGREEMENTS=8;DealProposalAction.INSPECT=99
PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62={marker='old'}
PROPERTIES.JAC_BOUNDARY_FIXED_OFFER_62_V1={marker='fixed'}
PROPERTIES.JAC_BOUNDARY_BARTER_OFFER_62_V1={phase='FINAL_CHECK',closed=true,skipped=true,result={stage='NOT_SUBMITTED',returned=false}}
local d=Players[0]:GetDiplomacy();function d:HasMet()return true end
Players[21]={IsAlive=function()return true end,IsHuman=function()return false end,
 GetTreasury=function()return {GetGoldBalance=function()return PEER_GOLD end}end,
 GetDiplomacy=function()return {HasOpenBordersFrom=function()return BORDER end}end}
ContextPtr={LookUpControl=function(_,path)
 if path=='/InGame/DiplomacyDealView'then return {IsHidden=function()return false end}end
 return {IsHidden=function()return path:find('EqualizeDeal') and EQ_HIDDEN or ACCEPT_HIDDEN end,IsDisabled=function()return false end}
end}
function OUT:HasUnacceptableItems()return false end
local add=OUT.AddItemOfType
function OUT:AddItemOfType(kind,from)
 local i=add(self,kind,from);i.to=from==0 and 21 or 0
 function i:GetSubType()return self.sub end;function i:SetSubType(v)self.sub=v end
 function i:IsUnacceptable()return false end
 function i:SetDuration(v)self.duration=v end;function i:SetAmount(v)self.amount=v end
 return i
end
function DealManager.GetPossibleDealItems()return {{SubType=-242352742,Duration=30,IsValid=true,ValidationResult=1}}end
function DiplomacyManager.RequestSession(from,to,kind)
 assert(from==0 and to==21 and kind=='MAKE_DEAL');SESSION=7;IN.items=OUT.items
 Events.DiplomacyStatement.fire(21,0,{SessionID=7,Initiator=0,StatementType=5,
 DealAction=DealProposalAction.ACCEPTED,RespondingToDealAction=99})
end
function DealManager.SendWorkingDeal(action,from,to)
 assert(from==0 and to==21);SENDS[#SENDS+1]={action=action,from=from,to=to}
 if MODE=='THROW'then error('unknown after native send')end
 if MODE=='NONE'then return end
 if action==DealProposalAction.EQUALIZE then
  if MODE=='GOOD'or MODE=='NO_PAYMENT'then
   local g=OUT:AddItemOfType(DealItemTypes.GOLD,21);g:SetDuration(0);g:SetAmount(QUOTE_AMOUNT);IN.items=OUT.items
  end
  Events.DiplomacyStatement.fire(21,0,{SessionID=7,Initiator=0,StatementType=5,
   DealAction=MODE=='REJECT'and DealProposalAction.REJECTED or DealProposalAction.ADJUSTED,
   RespondingToDealAction=DealProposalAction.EQUALIZE})
 elseif action==DealProposalAction.ACCEPTED and MODE=='GOOD'then
  GOLD_BALANCE=GOLD_BALANCE+QUOTE_AMOUNT;PEER_GOLD=PEER_GOLD-QUOTE_AMOUNT;BORDER=true
 end
end
"#).exec().unwrap();
    assert_eq!(l.load(setup()).eval::<String>().unwrap(), "PREPARED");
    l.load(record("PREPARED", "ABSENT")).exec().unwrap();
    l
}
#[cfg(test)]
fn quoted(l: &mlua::Lua) {
    l.load("JAC_BorderSale1665.Open();if not JAC_BorderSale1665.InspectReady()then TIME=TIME+31 end;assert(JAC_BorderSale1665.InspectReady());JAC_BorderSale1665.ArmQuote()").exec().unwrap();
    l.load(record("QUOTE_INTENT", "PREPARED")).exec().unwrap();
    l.load("JAC_BorderSale1665.QuoteOnce();TIME=TIME+31;assert(JAC_BorderSale1665.QuoteReady());JAC_BorderSale1665.RecordQuote()").exec().unwrap();
    l.load(record("QUOTED", "QUOTE_INTENT")).exec().unwrap();
}
#[test]
fn actual_two_party_payment_and_border_are_required_and_sends_never_repeat() {
    let l = prepared();
    quoted(&l);
    l.load("JAC_BorderSale1665.AcceptOnce();assert(JAC_BorderSale1665.Complete());assert(JAC_BorderSale1665.Publish()=='FUNDED_NATIVE')").exec().unwrap();
    l.load(record("OUTCOME", "QUOTED")).exec().unwrap();
    l.load("JAC_BorderSale1665.Close();JAC_BorderSale1665.Final();assert(#SENDS==2 and GOLD_BALANCE==81.6953125 and PEER_GOLD==652.8125 and CAPTIVE_COUNT==3 and MAP_COUNT==0)").exec().unwrap();
    assert!(l.load("JAC_BorderSale1665.AcceptOnce()").exec().is_err());
    assert!(l.load("JAC_BorderSale1665.QuoteOnce()").exec().is_err());
}
#[test]
fn rejected_or_unknown_quotes_and_hidden_equalize_never_trigger_payment() {
    for change in [
        "MODE='REJECT'",
        "MODE='THROW'",
        "MODE='NONE'",
        "EQ_HIDDEN=true",
        "ContextPtr.LookUpControl=function()return {IsHidden=function()return nil end,IsDisabled=function()return nil end}end",
        "QUOTE_AMOUNT=700",
    ] {
        let l = prepared();
        l.load(change).exec().unwrap();
        quoted(&l);
        l.load("JAC_BorderSale1665.AcceptOnce();assert(JAC_BorderSale1665.Complete());assert(JAC_BorderSale1665.Publish()=='NOT_SUBMITTED');assert(GOLD_BALANCE==61.6953125 and PEER_GOLD==672.8125)").exec().unwrap();
        assert!(l.load("JAC_BorderSale1665.QuoteOnce()").exec().is_err());
    }
}
#[test]
fn mutated_terms_or_saved_intent_cannot_send_accept() {
    for change in [
        "OUT.items[2].amount=21",
        "OUT.items[1].duration=29",
        "PEER_GOLD=671",
        "ACCEPT_HIDDEN=true",
        "PROPERTIES.JAC_BOUNDARY_BORDER_SALE_62_21_V1.nonce='other'",
        "PROPERTIES.JAC_BOUNDARY_BORDER_SALE_62_21_V1.acceptAllowed=false",
        "PROPERTIES.JAC_BOUNDARY_BORDER_SALE_62_21_V1.amount=21",
        "PROPERTIES.JAC_BOUNDARY_BARTER_OFFER_62_V1.closed=false",
    ] {
        let l = prepared();
        quoted(&l);
        l.load(change).exec().unwrap();
        assert!(
            l.load("JAC_BorderSale1665.AcceptOnce()").exec().is_err(),
            "{change}"
        );
        l.load("assert(#SENDS==1)").exec().unwrap();
    }
}
#[test]
fn missing_payment_or_recovered_core_anomaly_cannot_pass_final() {
    let l = prepared();
    l.load("MODE='NO_PAYMENT'").exec().unwrap();
    quoted(&l);
    l.load("JAC_BorderSale1665.AcceptOnce();TIME=TIME+31;assert(JAC_BorderSale1665.Publish()=='UNKNOWN')").exec().unwrap();
    l.load(record("OUTCOME", "QUOTED")).exec().unwrap();
    l.load("JAC_BorderSale1665.Close()").exec().unwrap();
    assert!(l.load("JAC_BorderSale1665.Final()").exec().is_err());
    let l = prepared();
    quoted(&l);
    l.load("JAC_BorderSale1665.AcceptOnce();JAC_BorderSale1665.Publish();PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.marker='temporary'").exec().unwrap();
    l.load(record("OUTCOME", "QUOTED")).exec().unwrap();
    l.load("PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.marker='old';JAC_BorderSale1665.Close()")
        .exec()
        .unwrap();
    assert!(l.load("JAC_BorderSale1665.Final()").exec().is_err());
}
#[test]
fn invalid_quote_raw_values_and_late_replies_are_not_lost_or_accepted() {
    let l = prepared();
    l.load("QUOTE_AMOUNT=700").exec().unwrap();
    quoted(&l);
    let e: String = l
        .load("return JAC_BorderSale1665.Evidence()")
        .eval()
        .unwrap();
    let e: Value = serde_json::from_str(&e).unwrap();
    assert_eq!(e["quote"]["outgoingRaw"]["value"]["2"]["amount"], 700);
    assert_eq!(e["quote"]["outgoing"]["status"], "ERROR");
    let l = prepared();
    quoted(&l);
    l.load("Events.DiplomacyStatement.fire(21,0,{SessionID=7,Initiator=0,StatementType=5,DealAction=DealProposalAction.REJECTED,RespondingToDealAction=DealProposalAction.EQUALIZE})").exec().unwrap();
    l.load("assert(JAC_BorderSale1665.AcceptOnce()=='ACCEPT_NOT_SENT_RESPONSE_CHANGED');assert(#SENDS==1);JAC_BorderSale1665.Publish()").exec().unwrap();
    l.load(record("OUTCOME", "QUOTED")).exec().unwrap();
    l.load("JAC_BorderSale1665.Close();assert(CLOSED==1 and PROPERTIES.JAC_BOUNDARY_BORDER_SALE_62_21_V1.acceptSkipReason=='QUOTE_CHANGED_AFTER_SAVE')").exec().unwrap();
    let l = prepared();
    l.load(
        "JAC_BorderSale1665.Open();JAC_BorderSale1665.InspectReady();JAC_BorderSale1665.ArmQuote()",
    )
    .exec()
    .unwrap();
    l.load(record("QUOTE_INTENT", "PREPARED")).exec().unwrap();
    l.load("Events.DiplomacyStatement.fire(21,0,{SessionID=7,Initiator=0,StatementType=5,DealAction=DealProposalAction.REJECTED,RespondingToDealAction=99})").exec().unwrap();
    l.load("assert(JAC_BorderSale1665.QuoteOnce()=='QUOTE_NOT_SENT_RESPONSE_CHANGED');assert(#SENDS==0);JAC_BorderSale1665.RecordQuote()").exec().unwrap();
    l.load(record("QUOTED", "QUOTE_INTENT")).exec().unwrap();
    l.load("JAC_BorderSale1665.AcceptOnce();JAC_BorderSale1665.Publish()")
        .exec()
        .unwrap();
    l.load(record("OUTCOME", "QUOTED")).exec().unwrap();
    l.load("JAC_BorderSale1665.Close();assert(CLOSED==1 and #SENDS==0 and PROPERTIES.JAC_BOUNDARY_BORDER_SALE_62_21_V1.quoteSkipReason=='INSPECTION_CHANGED_AFTER_SAVE')").exec().unwrap();
}
