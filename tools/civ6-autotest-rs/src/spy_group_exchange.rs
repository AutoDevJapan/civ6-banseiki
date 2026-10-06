//! Finite, diagnostic native group-captive exchange for the saved turn-62 game.
//! Each constant is a separate Tuner script. Only the host saves between calls.
//! No script grants a unit, edits a mission, or equates ForType with a spy ID.

pub const UI_SETUP: &str = r#"
assert(_G.JAC_GroupExchange548==nil,'group exchange UI instance already installed');
local B=assert(ExposedMembers.JAC_PersonaBoundary,'Boundary UI reader missing');
assert(type(B.ReadEnemy)=='function','Boundary UI reader missing');
local KEY='JAC_BOUNDARY_GROUP_EXCHANGE_62';
local WKEY='JAC_GroupExchange548Witness';
local previousTooltip=ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey;
local x=nil;
local G={}; _G.JAC_GroupExchange548=G;
local function integer(v,min)
 return type(v)=='number' and v==v and v>=min and v<9007199254740992 and v==math.floor(v)
end
local function copy(v)
 if type(v)~='table' then
  assert(v==nil or type(v)=='string' or type(v)=='boolean' or
   (type(v)=='number' and v==v and math.abs(v)<math.huge),'non-scalar witness');
  return v
 end
 local r={}; for k,a in pairs(v) do
  assert(type(k)=='string' or type(k)=='number','non-scalar witness key'); r[k]=copy(a)
 end; return r
end
local function history(h,id,turn)
 assert(type(h)=='table' and h.id==id and h.turn==turn and
  integer(h.operation,0) and integer(h.plot,0) and h.initial==1 and h.escape==-1,
  'preserved native failure differs');
 return {id=h.id,turn=h.turn,operation=h.operation,plot=h.plot,
  initial=h.initial,escape=h.escape}
end
local function sameHistory(a,b)
 return a and b and a.id==b.id and a.turn==b.turn and
  a.operation==b.operation and a.plot==b.plot and
  a.initial==b.initial and a.escape==b.escape
end
local function histories()
 local a=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'),'third archive missing');
 local s=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE'),'third state missing');
 local post=assert(s.missionPost,'third mission post missing');
 local ack=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK'),'third native mission acknowledgement missing');
 local event=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'),'third event missing');
 local h={history(a.histories[1],0,23),history(a.histories[2],1,45),
  history(post.histories[3],2,62)};
 assert(event.attacker==0 and event.id==2 and event.turn==62 and
  sameHistory(h[1],post.histories[1]) and sameHistory(h[2],post.histories[2]) and
  ack.observed and sameHistory(h[3],ack.observed.mission),
  'third mission lineage differs');
 local d=Players[0]:GetDiplomacy();
 for i=0,2 do
  local n=assert(d:GetMission(0,i),'native mission history missing');
  assert(sameHistory(h[i+1],{id=i,turn=n.CompletionTurn,operation=n.Operation,
   plot=n.PlotIndex,initial=n.InitialResult,escape=n.EscapeResult}),
   'native mission history changed')
 end
 return h
end
local function ids(deal)
 assert(deal and type(deal.GetItemCount)=='function','native working deal unavailable');
 local out={}; for item in deal:Items() do out[#out+1]=item:GetID() end
 assert(#out==deal:GetItemCount() and #out<=8,'native deal enumeration differs');
 return out
end
local function outgoing()
 return assert(DealManager.GetWorkingDeal(DealDirection.OUTGOING,0,1),
  'native outgoing working deal missing')
end
local function rosterSnapshot()
 local s=B.ReadEnemy(); local r=assert(s.spyRoster,'UI native roster missing');
 assert(s.turn==Game.GetCurrentGameTurn() and s.attacker==0 and s.owner==1 and
  s.retry and s.retry.current==3 and s.retry.spyID==1048586 and
  s.spy and s.spy.id==1048586 and s.spy.owner==0 and
  s.neutralizationMission==false,'third mission state changed');
 local h=histories();
 assert(s.missions['0/2'] and sameHistory(h[3],s.missions['0/2']),
  'UI third mission differs');
 local out={turn=s.turn,capacity=Players[0]:GetDiplomacy():GetSpyCapacity(),
  referencePresent=s.spy.present,
  map={},activeOffMap={},captured={},offMap={},physical=r.physicalCount,
  histories=h};
 for _,v in ipairs(r.map) do out.map[#out.map+1]={id=v.id,owner=v.owner,
  unitType=v.unitType,x=v.x,y=v.y,level=v.level,name=v.name} end
 for _,v in ipairs(r.activeOffMap) do out.activeOffMap[#out.activeOffMap+1]=
  {name=v.name,level=v.level,returnTurn=v.returnTurn,x=v.x,y=v.y} end
 for _,v in ipairs(r.offMap) do out.offMap[#out.offMap+1]=
  {name=v.name,level=v.level,returnTurn=v.returnTurn,x=v.x,y=v.y} end
 for _,v in ipairs(r.captured) do out.captured[#out.captured+1]=
  {captor=v.captor,name=v.name,level=v.level,nameIndex=v.nameIndex} end
 return out
end
local function sameBaseline(now)
 if not x or now.turn~=x.before.turn or now.capacity~=3 or now.physical~=3 or
  now.referencePresent~=false or
  #now.map~=0 or #now.activeOffMap~=0 or #now.captured~=3 or #now.offMap~=3 then return false end
 for i=1,3 do
  local c=now.captured[i]; local o=now.offMap[i];
  if c.captor~=1 or c.name~='' or c.level~=1 or c.nameIndex~=-1 or
   o.name~='' or o.level~=1 or o.returnTurn~=-1 or o.x~=-9999 or o.y~=-9999 or
   not sameHistory(now.histories[i],x.before.histories[i]) then return false end
 end
 return true
end
local function candidate(deal)
 local list=assert(DealManager.GetPossibleDealItems(1,0,DealItemTypes.CAPTIVE,deal),
  'native captive candidates unavailable');
 assert(#list==3,'native captive group candidate count changed');
 local selected=nil;
 for _,entry in ipairs(list) do
  assert(integer(entry.ForType,-1) and
   (entry.IsValid==true or entry.IsValid==1) and
   entry.ValidationResult==DealValidationResult.VALID,
   'native captive candidate invalid');
  if selected==nil then selected=entry.ForType end
  assert(selected==entry.ForType,'native captive candidates are not one group')
 end
 assert(selected~=nil,'native captive group absent');
 return selected,#list
end
local function peace()
 assert(not Players[0]:GetDiplomacy():IsAtWarWith(1) and
  not DealManager.HasPendingDeal(0,1),'war or pending native deal');
end
local function session()
 assert(x and x.sessionRequested and integer(x.sessionID,0),
  'owned native session not bound');
 local id=DiplomacyManager.FindOpenSessionID(0,1);
 assert(id==x.sessionID,'native session identity changed');
 local info=assert(DiplomacyManager.GetSessionInfo(id),'native session info missing');
 assert(info.FromPlayer==0,'native session initiator changed');
 return id
end
local function ownItem()
 local d=outgoing(); local found=nil;
 for item in d:Items() do if item:GetID()==x.itemID then found=item end end
 assert(found and found:GetID()==x.itemID and found:GetType()==DealItemTypes.CAPTIVE and
  found:GetFromPlayerID()==1 and found:GetToPlayerID()==0 and
  found:GetValueType()==x.forType and found:IsLocked() and found:IsValid(),
  'owned captive item changed');
 return d,found
end
local function publish(phase,observation)
 ExposedMembers[WKEY]=copy({phase=phase,nonce=x.nonce,turn=x.before.turn,
  forType=x.forType,candidateCount=x.candidateCount,before=x.before,
  observation=observation});
end
local function statement(from,to,k)
 local r={from=from,to=to};
 for _,field in ipairs({'SessionID','Initiator','StatementType','StatementSubType',
  'DealAction','RespondingToDealAction'}) do r[field]=k[field] end
 return copy(r)
end
Events.DiplomacyStatement.Add(function(from,to,k)
 if not x or x.closed or type(k)~='table' then return end
 local r=statement(from,to,k);
 if x.sessionRequested and ((from==0 and to==1) or (from==1 and to==0)) and
  (x.sessionID==nil or r.SessionID==x.sessionID) then
  x.statements[#x.statements+1]=r;
  if #x.statements>32 then x.statementOverflow=true end
  if not x.ownedStatement and r.Initiator==0 and
   DiplomacyManager.GetKeyName(r.StatementType)=='MAKE_DEAL' then x.ownedStatement=r end
 end
 if x.equalizeSent and from==1 and to==0 and
  r.RespondingToDealAction==DealProposalAction.EQUALIZE then
  x.responses[#x.responses+1]=r
 end
end);
Events.DiplomacyIncomingDeal.Add(function(from,to,action)
 if x and x.equalizeSent and from==1 and to==0 then
  x.incomingEvents[#x.incomingEvents+1]={from=from,to=to,action=action}
 end
end);
local tooltip;
tooltip=function(...)
 if x and not x.closed and x.sessionRequested and x.itemID and x.forType==-1 then
  local ok,key=pcall(function()
   local d,item=ownItem(); local now=rosterSnapshot();
   local selected,count=candidate(d);
   assert(selected==x.forType and count==3 and sameBaseline(now),
    'owned tooltip group changed');
   assert(type(item.GetValueTypeNameID)=='function' and
    item:GetValueTypeNameID()==nil,'native captive already has tooltip name');
   local spy=assert(GameInfo.Units['UNIT_SPY'],'spy definition missing'); return spy.Name
  end);
  if ok then return key end
 end
 if previousTooltip then return previousTooltip(...) end
 return nil
end;
ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey=tooltip;

function G.Prepare()
 assert(not x and not Game:GetProperty(KEY),'group exchange already reserved');
 assert(Game.GetLocalPlayer()==0 and Players[0]:IsHuman() and
  not Players[1]:IsHuman() and Players[0]:IsTurnActive(),
  'native human/AI roles or active turn changed');
 peace(); assert(DiplomacyManager.FindOpenSessionID(0,1)==nil,
  'native diplomacy session already open');
 local before=rosterSnapshot();
 assert(before.turn==62 and before.capacity==3 and before.physical==3 and
  before.referencePresent==false and
  #before.captured==3 and #before.offMap==3 and
  #before.map==0 and #before.activeOffMap==0,'three-captive baseline changed');
 for i=1,3 do
  local c=before.captured[i]; local o=before.offMap[i];
  assert(c.captor==1 and c.name=='' and c.nameIndex==-1 and c.level==1 and
   o.name=='' and o.level==1 and o.returnTurn==-1 and o.x==-9999 and o.y==-9999,
   'native captive group or sentinel changed')
 end
 local deal=outgoing(); assert(#ids(deal)==0,'existing native offer must be preserved');
 local selected,count=candidate(deal);
 x={before=before,forType=selected,candidateCount=count,
  nonce=tostring(UI.GetElapsedTime())..':62',statements={},responses={},incomingEvents={}};
 publish('PREPARED',{stage='PREPARED'});
 return 'PREPARED'
end
function G.ArmAfterSave()
 assert(x and not x.armed and not x.sessionRequested,'UI instance or arm changed');
 local p=assert(Game:GetProperty(KEY),'saved Core reservation missing');
 assert(p.stage=='RESERVED' and p.nonce==x.nonce and p.forType==x.forType and
  p.turn==62 and sameBaseline(rosterSnapshot()),'Core reservation or UI instance changed');
 peace(); x.armed=true; return 'ARMED'
end
function G.OpenSession()
 assert(x and x.armed and not x.sessionRequested and
  Game:GetProperty(KEY).stage=='RESERVED','saved group reservation missing');
 assert(sameBaseline(rosterSnapshot()),'native roster changed before session');
 peace(); assert(DiplomacyManager.FindOpenSessionID(0,1)==nil,
  'another native session opened');
 local deal=outgoing(); assert(#ids(deal)==0,'native offer changed before session');
 assert(candidate(deal)==x.forType,'native group candidate changed');
 -- Reserve the only allowed item. A failed/unknown native call cannot be retried.
 x.sessionRequested=true;
 local item=assert(deal:AddItemOfType(DealItemTypes.CAPTIVE,1),
  'native captive item unavailable');
 x.itemID=item:GetID(); assert(integer(x.itemID,0),'native item ID unavailable');
 item:SetValueType(x.forType); assert(item:IsValid(),'native captive item invalid');
 item:SetLocked(true); ownItem();
 DiplomacyManager.RequestSession(0,1,'MAKE_DEAL');
 return 'SESSION_REQUESTED'
end
function G.SessionReady()
 if not x or not x.sessionRequested then return false end
 if not x.sessionID then
  local id=DiplomacyManager.FindOpenSessionID(0,1);
  if id==nil then return false end
  assert(integer(id,0) and
   DiplomacyManager.GetSessionInfo(id).FromPlayer==0,'native session not owned');
  x.sessionID=id
 end
 session();
 -- A statement from the engine, not just a visible screen, binds ownership.
 for _,r in ipairs(x.statements) do
  if r.SessionID==x.sessionID and r.Initiator==0 and
   DiplomacyManager.GetKeyName(r.StatementType)=='MAKE_DEAL' then
   x.ownedStatement=r; break
  end
 end
 return x.ownedStatement~=nil and not x.statementOverflow
end
function G.EqualizeOnce()
 assert(x and x.armed and not x.equalizeSent and G.SessionReady(),
  'owned native session not ready or EQUALIZE already sent');
 assert(Game:GetProperty(KEY).stage=='RESERVED' and sameBaseline(rosterSnapshot()),
  'saved reservation or roster changed');
 peace(); local deal,item=ownItem();
 assert(#ids(deal)==1 and item:GetID()==x.itemID and
  deal:Validate()==DealValidationResult.VALID,'native captive offer invalid');
 x.equalizeSent=true; x.equalizeAt=UI.GetElapsedTime();
 DealManager.SendWorkingDeal(DealProposalAction.EQUALIZE,0,1);
 return 'EQUALIZE_SENT'
end
local function dealRows(direction)
 local deal=DealManager.GetWorkingDeal(direction,0,1);
 if not deal then return {available=false,items={}} end
 local rows={}; for item in deal:Items() do
  rows[#rows+1]={id=item:GetID(),kind=item:GetType(),from=item:GetFromPlayerID(),
   to=item:GetToPlayerID(),value=item:GetValueType(),
   amount=type(item.GetAmount)=='function' and item:GetAmount() or nil,
   duration=type(item.GetDuration)=='function' and item:GetDuration() or nil,
   valid=item:IsValid()}
 end
 assert(#rows==deal:GetItemCount() and #rows<=8,'native deal rows changed during read');
 return {available=true,items=rows,validation=deal:Validate()}
end
local function safeTerms(d,balance)
 if not d.available or d.validation~=DealValidationResult.VALID or
  #d.items<1 or #d.items>2 then return false end
 local captive,gold=0,0;
 for _,r in ipairs(d.items) do
  if r.valid~=true then return false end
  if r.kind==DealItemTypes.CAPTIVE and r.from==1 and r.to==0 and
   r.value==x.forType then captive=captive+1
  elseif r.kind==DealItemTypes.GOLD and r.from==0 and r.to==1 and
   r.duration==0 and integer(r.amount,1) and r.amount<=balance then gold=gold+1
  else return false end
 end
 return captive==1 and gold<=1 and captive+gold==#d.items
end
local function sameDealRows(a,b)
 if not a or not b or a.available~=b.available or
  a.validation~=b.validation or #a.items~=#b.items then return false end
 for i=1,#a.items do
  local v,u=a.items[i],b.items[i];
  for _,field in ipairs({'id','kind','from','to','value','amount','duration','valid'}) do
   if v[field]~=u[field] then return false end
  end
 end
 return true
end
local function response()
 assert(x and x.equalizeSent,'EQUALIZE has not been sent');
 local result={stage='WAIT_AI',responses=copy(x.responses),
  incomingEvents=copy(x.incomingEvents),statements=copy(x.statements),
  sessionID=x.sessionID,elapsed=UI.GetElapsedTime()-x.equalizeAt};
 local now=rosterSnapshot(); result.roster=now;
 if not sameBaseline(now) then result.stage='UNKNOWN_ROSTER_CHANGED'; return result end
 if x.statementOverflow or #x.responses>1 then result.stage='UNKNOWN_RESPONSE'; return result end
 if #x.responses==0 then
  if result.elapsed>=30 then result.stage='UNKNOWN_TIMEOUT' end
  return result
 end
 local r=x.responses[1]; result.reply=copy(r);
 if r.SessionID~=x.sessionID or r.Initiator~=0 or r.from~=1 or r.to~=0 or
  DiplomacyManager.GetKeyName(r.StatementType)~='MAKE_DEAL' then
  result.stage='UNKNOWN_RESPONSE'; return result
 end
 if (type(DealProposalAction.REJECTED)=='number' and
   r.DealAction==DealProposalAction.REJECTED) or
  (type(DealProposalAction.REJECTED_PERMANENT)=='number' and
   r.DealAction==DealProposalAction.REJECTED_PERMANENT) or
  (type(DealProposalAction.EQUALIZE_FAILED)=='number' and
   r.DealAction==DealProposalAction.EQUALIZE_FAILED) then
  result.stage='AI_REFUSED'; return result
 end
 if r.DealAction~=DealProposalAction.ACCEPTED and
  r.DealAction~=DealProposalAction.ADJUSTED and
  r.DealAction~=DealProposalAction.PROPOSED then
  result.stage='UNKNOWN_RESPONSE'; return result
 end
 local ok=pcall(session); if not ok then result.stage='UNKNOWN_SESSION'; return result end
 if Players[0]:GetDiplomacy():IsAtWarWith(1) or
  DealManager.HasPendingDeal(0,1) then result.stage='UNKNOWN_RELATION'; return result end
 local balance=math.floor(Players[0]:GetTreasury():GetGoldBalance());
 result.goldBalance=balance;
 result.incoming=dealRows(DealDirection.INCOMING);
 result.outgoing=dealRows(DealDirection.OUTGOING);
 if not safeTerms(result.incoming,balance) then result.stage='UNSAFE_TERMS'; return result end
 -- The stock deal view may already have copied the incoming reply. Otherwise
 -- only our original locked captive item may be replaced by the official copy.
 if not DealManager.AreWorkingDealsEqual(0,1) then
  local okOwned,deal=pcall(ownItem);
  if not okOwned or #ids(deal)~=1 then result.stage='UNKNOWN_OUTGOING'; return result end
 end
 result.stage='AI_QUOTE'; result.needsCopy=not DealManager.AreWorkingDealsEqual(0,1);
 return result
end
function G.ResponseState() return copy(response()) end
function G.PublishQuote()
 assert(x and not x.quotePublished,'quote already published');
 local raw=response(); assert(raw.stage~='WAIT_AI','native AI response still pending');
 local q={stage=raw.stage,raw=raw,acceptable=false,sessionID=x.sessionID,
  forType=x.forType,turn=x.before.turn,
  uiEnums={equalize=DealProposalAction.EQUALIZE,
   accepted=DealProposalAction.ACCEPTED,adjusted=DealProposalAction.ADJUSTED,
   proposed=DealProposalAction.PROPOSED,captive=DealItemTypes.CAPTIVE,
   gold=DealItemTypes.GOLD,valid=DealValidationResult.VALID}};
 -- The raw native reply is retained even if the stock copy changes the deal
 -- and then fails. Never retry that ambiguous copy.
 x.quotePublished=true; x.quote=copy(q);
 if raw.stage=='AI_QUOTE' then
  local ok,err=pcall(function()
   if raw.needsCopy then DealManager.CopyIncomingToOutgoingWorkingDeal(0,1) end
   local balance=math.floor(Players[0]:GetTreasury():GetGoldBalance());
   q.incoming=dealRows(DealDirection.INCOMING);
   q.outgoing=dealRows(DealDirection.OUTGOING);
   q.goldBalance=balance;
   q.acceptable=safeTerms(q.incoming,balance) and safeTerms(q.outgoing,balance) and
    DealManager.AreWorkingDealsEqual(0,1) and sameBaseline(rosterSnapshot()) and
    not DealManager.HasPendingDeal(0,1)
  end);
  if not ok then q.stage='UNKNOWN_COPY'; q.copyError=tostring(err);
  elseif not q.acceptable then q.stage='UNKNOWN_AFTER_COPY' end
 end
 x.quote=copy(q); publish('QUOTE',q);
 return q.stage
end
function G.AcceptOnce()
 assert(x and x.quotePublished and not x.finalSent and x.quote.acceptable,
  'accepted native quote missing or final already sent');
 local p=assert(Game:GetProperty(KEY),'saved final intent missing');
 assert(p.stage=='FINAL_INTENT' and p.nonce==x.nonce and
  p.finalIntent.sessionID==x.sessionID and p.finalIntent.forType==x.forType and
  ExposedMembers[WKEY].phase=='QUOTE','saved final intent differs');
 assert(session()==x.sessionID and sameBaseline(rosterSnapshot()) and
  #x.responses==1,'native quote changed before final acceptance');
 peace(); local balance=math.floor(Players[0]:GetTreasury():GetGoldBalance());
 local incoming=dealRows(DealDirection.INCOMING);
 local outgoingRows=dealRows(DealDirection.OUTGOING);
 assert(safeTerms(incoming,balance) and safeTerms(outgoingRows,balance) and
  sameDealRows(incoming,x.quote.incoming) and
  sameDealRows(outgoingRows,x.quote.outgoing) and
  DealManager.AreWorkingDealsEqual(0,1),'native terms changed before final acceptance');
 x.finalSent=true; x.finalAt=UI.GetElapsedTime();
 DealManager.SendWorkingDeal(DealProposalAction.ACCEPTED,0,1);
 return 'FINAL_SENT'
end
function G.ReturnState()
 assert(x and x.finalSent,'native final acceptance not sent');
 local now=rosterSnapshot();
 local r={stage='WAIT_RETURN',before=x.before,after=now,
  sessionID=x.sessionID,quote=x.quote,elapsed=UI.GetElapsedTime()-x.finalAt};
 if now.turn~=62 or now.capacity~=3 or now.physical~=3 then
  r.stage='UNKNOWN_TOTAL'; return copy(r)
 end
 for i=1,3 do if not sameHistory(now.histories[i],x.before.histories[i]) then
  r.stage='UNKNOWN_HISTORY'; return copy(r) end end
 if #now.captured==3 and #now.map==0 and #now.activeOffMap==0 then
  if r.elapsed>=30 then r.stage='RETURN_UNOBSERVED' end
  return copy(r)
 end
 if #now.captured~=2 or #now.map+#now.activeOffMap~=1 then
  r.stage='UNKNOWN_ROSTER'; return copy(r)
 end
 for _,c in ipairs(now.captured) do if c.captor~=1 or c.name~='' or
  c.nameIndex~=-1 or c.level~=1 then r.stage='UNKNOWN_ROSTER'; return copy(r) end end
 if #now.map==0 then r.stage='RETURNED_OFF_MAP_ID_UNKNOWN'; return copy(r) end
 local u=now.map[1]; local spy=assert(GameInfo.Units['UNIT_SPY'],'native spy definition missing');
 if u.owner~=0 or u.unitType~=spy.Index or not integer(u.id,0) or
  not integer(u.x,0) or not integer(u.y,0) or
  not Map.GetPlot(u.x,u.y) then r.stage='UNKNOWN_MAP_IDENTITY'; return copy(r) end
 r.stage='RETURNED_MAP'; r.returned={id=u.id,owner=u.owner,unitType=u.unitType,
  x=u.x,y=u.y,turn=now.turn};
 return copy(r)
end
function G.PublishReturn()
 assert(x and x.finalSent and not x.returnPublished,'return already published');
 local r=G.ReturnState(); assert(r.stage~='WAIT_RETURN','native return still pending');
 x.returnPublished=true; x.returnEvidence=copy(r); publish('RETURN',r);
 return r.stage
end
function G.Evidence()
 return copy({ui=x and {nonce=x.nonce,armed=x.armed,sessionID=x.sessionID,
  sessionRequested=x.sessionRequested,equalizeSent=x.equalizeSent,
  quotePublished=x.quotePublished,finalSent=x.finalSent,
  returnPublished=x.returnPublished,closed=x.closed,
  statements=x.statements,responses=x.responses,incomingEvents=x.incomingEvents,
  quote=x.quote,returnEvidence=x.returnEvidence} or nil,
  core=Game:GetProperty(KEY),witness=ExposedMembers[WKEY]})
end
function G.Close()
 assert(x and not x.closed,'group exchange already closed');
 local p=Game:GetProperty(KEY);
 assert(not x.equalizeSent or
  (x.quotePublished and p and
   (p.stage=='QUOTED_UNKNOWN' or p.stage=='QUOTED' or
    p.stage=='FINAL_INTENT' or p.stage=='RETURNED' or p.stage=='RETURN_UNRESOLVED')),
  'native reply evidence must be recorded before close');
 local d=DealManager.GetWorkingDeal(DealDirection.OUTGOING,0,1);
 if d and x.itemID then
  local item=d:FindItemByID(x.itemID);
  if item then
   assert(item:GetType()==DealItemTypes.CAPTIVE and item:GetFromPlayerID()==1 and
    item:GetToPlayerID()==0 and item:GetValueType()==x.forType,
    'owned native item changed before cleanup');
   if item:IsLocked() then item:SetLocked(false) end
   assert(d:RemoveItemByID(x.itemID),'owned native item removal failed')
  end
 end
 if x.sessionID then
  local id=DiplomacyManager.FindOpenSessionID(0,1);
  if id~=nil then assert(session()==id,'native session ownership changed');
   DiplomacyManager.CloseSession(id) end
 end
 x.closed=true;
 if ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey==tooltip then
  ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey=previousTooltip
 end
 return 'CLOSED'
end
return 'GROUP_EXCHANGE_UI_READY'
"#;

pub const RESERVE_CORE: &str = r#"
local KEY='JAC_BOUNDARY_GROUP_EXCHANGE_62';
local w=assert(ExposedMembers.JAC_GroupExchange548Witness,'UI scalar witness missing');
assert(not Game:GetProperty(KEY) and w.phase=='PREPARED' and
 type(w.nonce)=='string' and w.turn==62 and w.forType==-1 and
 w.candidateCount==3 and w.observation.stage=='PREPARED',
 'new group reservation differs');
local b=w.before;
assert(b.turn==62 and b.capacity==3 and b.physical==3 and
 #b.map==0 and #b.activeOffMap==0 and #b.captured==3 and #b.offMap==3,
 'three-captive UI roster differs');
for i=1,3 do
 local c=b.captured[i]; local o=b.offMap[i]; local h=b.histories[i];
 assert(c.captor==1 and c.name=='' and c.nameIndex==-1 and c.level==1 and
  o.name=='' and o.level==1 and o.returnTurn==-1 and o.x==-9999 and o.y==-9999 and
  h.id==i-1 and h.turn==({23,45,62})[i] and h.initial==1 and h.escape==-1,
  'UI captive group or history differs')
end
local a=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'));
local s=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE'));
local ack=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK'));
local e=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'));
assert(e.attacker==0 and e.id==2 and e.turn==62 and s.missionPost and
 s.missionPost.histories and ack.observed and ack.observed.mission,
 'saved third failure lineage missing');
local source={roster='UI',histories='UI',map='UI',capacity='UI'};
local function same(h,k)
 return h.id==k.id and h.turn==k.turn and h.operation==k.operation and
  h.plot==k.plot and h.initial==k.initial and h.escape==k.escape
end
for i=1,3 do
 local p=i<=2 and a.histories[i] or s.missionPost.histories[3];
 assert(same(b.histories[i],p),'saved failure history changed');
end
assert(same(b.histories[3],ack.observed.mission),'third native acknowledgement changed');
local d=Players[0]:GetDiplomacy();
if type(d.GetMission)=='function' then
 for i=0,2 do
  local h=d:GetMission(0,i); local expected=b.histories[i+1];
  assert(type(h)=='table' and h.Operation==expected.operation and
   h.PlotIndex==expected.plot and h.CompletionTurn==expected.turn and
   h.InitialResult==expected.initial and h.EscapeResult==expected.escape,
   'Core native mission history differs')
 end; source.histories='CORE'
end
if type(d.GetSpyCapacity)=='function' then
 assert(d:GetSpyCapacity()==3,'Core spy capacity differs'); source.capacity='CORE'
end
local units=Players[0]:GetUnits();
if units and type(units.Members)=='function' then
 local n=0; for _,u in units:Members() do
  local def=GameInfo.Units[u:GetType()]; if def and def.Spy then n=n+1 end
 end; assert(n==0,'Core active map spies changed'); source.map='CORE'
end
Game:SetProperty(KEY,{stage='RESERVED',nonce=w.nonce,turn=62,forType=w.forType,
 candidateCount=w.candidateCount,before=w.before,sources=source});
return 'RESERVED'
"#;

pub const QUOTE_CORE: &str = r#"
local KEY='JAC_BOUNDARY_GROUP_EXCHANGE_62';
local p=assert(Game:GetProperty(KEY),'Core group reservation missing');
local w=assert(ExposedMembers.JAC_GroupExchange548Witness,'UI quote witness missing');
assert(p.stage=='RESERVED' and w.phase=='QUOTE' and w.nonce==p.nonce and
 w.turn==p.turn and w.forType==p.forType and w.candidateCount==p.candidateCount,
 'quote is not for the saved reservation');
local a=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'));
local s=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE'));
local ack=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK'));
for i=1,3 do
 local h=p.before.histories[i]; local n=i<=2 and a.histories[i] or s.missionPost.histories[3];
 assert(n and h.id==n.id and h.turn==n.turn and h.operation==n.operation and
  h.plot==n.plot and h.initial==n.initial and h.escape==n.escape,
  'saved native failure history changed before quote')
end
assert(ack.observed and ack.observed.mission and
 ack.observed.mission.id==p.before.histories[3].id and
 ack.observed.mission.turn==p.before.histories[3].turn,
 'third native acknowledgement changed before quote');
local q=assert(w.observation,'native quote missing');
assert(q.sessionID and q.forType==p.forType and q.turn==p.turn and
 q.raw and q.raw.stage~='WAIT_AI','unclassified native quote');
if q.acceptable then
 local e=assert(q.uiEnums,'UI native enum values missing');
 for _,name in ipairs({'equalize','accepted','adjusted','proposed',
  'captive','gold','valid'}) do
  assert(type(e[name])=='number','UI native enum value unavailable')
 end
 local r=assert(q.raw.reply,'explicit AI reply missing');
 assert(q.stage=='AI_QUOTE' and r.SessionID==q.sessionID and
  r.Initiator==0 and r.from==1 and r.to==0 and
  r.RespondingToDealAction==e.equalize and
  (r.DealAction==e.accepted or
   r.DealAction==e.adjusted or
   r.DealAction==e.proposed),
  'AI quote is not an explicit EQUALIZE reply');
 assert(q.incoming and q.outgoing and q.incoming.validation==e.valid and
  q.outgoing.validation==e.valid and
  #q.incoming.items==#q.outgoing.items and #q.incoming.items>=1 and
  #q.incoming.items<=2,'native quote deal invalid');
 local function terms(rows)
  local captive,gold=0,0;
  for _,item in ipairs(rows.items) do
   assert(item.valid==true,'invalid native quote item');
   if item.kind==e.captive then
    assert(item.from==1 and item.to==0 and item.value==p.forType,
     'native captive term changed'); captive=captive+1
   elseif item.kind==e.gold then
    assert(item.from==0 and item.to==1 and item.duration==0 and
     type(item.amount)=='number' and item.amount>0 and
     item.amount==math.floor(item.amount) and item.amount<=q.goldBalance,
     'native one-time gold term unsafe'); gold=gold+1
   else error('unsupported native quote item') end
  end
  assert(captive==1 and gold<=1 and captive+gold==#rows.items,
   'native quote cardinality differs')
 end
 terms(q.incoming); terms(q.outgoing)
 for i=1,#q.incoming.items do
  local a,b=q.incoming.items[i],q.outgoing.items[i];
  assert(a.kind==b.kind and a.from==b.from and a.to==b.to and
   a.value==b.value and a.amount==b.amount and a.duration==b.duration,
   'native incoming/outgoing terms differ')
 end
end
p.stage=q.acceptable and 'QUOTED' or 'QUOTED_UNKNOWN';
p.quote=q; p.quoteSource='UI_NATIVE_STATEMENT_AND_DEAL';
Game:SetProperty(KEY,p);
return p.stage
"#;

pub const FINAL_INTENT_CORE: &str = r#"
local KEY='JAC_BOUNDARY_GROUP_EXCHANGE_62';
local p=assert(Game:GetProperty(KEY),'saved native quote missing');
local w=assert(ExposedMembers.JAC_GroupExchange548Witness,'UI quote witness missing');
assert(p.stage=='QUOTED' and p.quote and p.quote.acceptable==true and
 w.phase=='QUOTE' and w.nonce==p.nonce and w.observation and
 w.observation.sessionID==p.quote.sessionID and
 w.observation.forType==p.quote.forType and
 w.observation.raw.reply.SessionID==p.quote.raw.reply.SessionID and
 w.observation.raw.reply.DealAction==p.quote.raw.reply.DealAction,
 'saved explicit native quote differs');
local a=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'));
local s=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE'));
for i=1,3 do
 local h=p.before.histories[i]; local n=i<=2 and a.histories[i] or s.missionPost.histories[3];
 assert(n and h.id==n.id and h.turn==n.turn and h.operation==n.operation and
  h.plot==n.plot and h.initial==n.initial and h.escape==n.escape,
  'saved native failure history changed before final intent')
end
assert(not p.finalIntent and not p.returnEvidence,'final intent already exists');
p.stage='FINAL_INTENT';
p.finalIntent={sessionID=p.quote.sessionID,forType=p.forType,
 turn=p.turn,replyAction=p.quote.raw.reply.DealAction,source='SAVED_QUOTE'};
Game:SetProperty(KEY,p);
return 'FINAL_INTENT'
"#;

pub const RETURN_CORE: &str = r#"
local KEY='JAC_BOUNDARY_GROUP_EXCHANGE_62';
local p=assert(Game:GetProperty(KEY),'saved final intent missing');
local w=assert(ExposedMembers.JAC_GroupExchange548Witness,'UI return witness missing');
assert(p.stage=='FINAL_INTENT' and p.finalIntent and w.phase=='RETURN' and
 w.nonce==p.nonce and w.observation and
 w.observation.sessionID==p.finalIntent.sessionID,
 'native return does not match final intent');
local a=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'));
local s=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE'));
for i=1,3 do
 local h=p.before.histories[i]; local n=i<=2 and a.histories[i] or s.missionPost.histories[3];
 assert(n and h.id==n.id and h.turn==n.turn and h.operation==n.operation and
  h.plot==n.plot and h.initial==n.initial and h.escape==n.escape,
  'saved native failure history changed before return')
end
local r=w.observation; local before=assert(r.before); local after=assert(r.after);
assert(before.turn==62 and #before.captured==3 and before.physical==3 and
 after.turn==62 and after.capacity==3 and after.physical==3,
 'native total was not conserved');
for i=1,3 do
 local a=before.histories[i]; local b=after.histories[i];
 assert(a.id==b.id and a.turn==b.turn and a.operation==b.operation and
  a.plot==b.plot and a.initial==b.initial and a.escape==b.escape,
  'preserved failure history changed during exchange')
end
if r.stage=='RETURNED_MAP' then
 assert(#after.captured==2 and #after.map==1 and #after.activeOffMap==0 and
  r.returned and r.returned.id==after.map[1].id and
  r.returned.owner==0 and r.returned.unitType==after.map[1].unitType and
  r.returned.x==after.map[1].x and r.returned.y==after.map[1].y and
  r.returned.turn==after.turn,'returned native spy identity differs');
 local units=Players[0]:GetUnits();
 if units and type(units.FindID)=='function' then
  local u=assert(units:FindID(r.returned.id),'Core returned native spy missing');
  assert(u:GetOwner()==0 and u:GetType()==r.returned.unitType,
   'Core returned native spy identity differs');
  if type(u.GetX)=='function' and type(u.GetY)=='function' then
   assert(u:GetX()==r.returned.x and u:GetY()==r.returned.y,
    'Core returned native spy position differs')
  end
 end
 p.stage='RETURNED'
else
 assert(r.stage~='WAIT_RETURN','native return still pending');
 p.stage='RETURN_UNRESOLVED'
end
p.returnEvidence=r;
Game:SetProperty(KEY,p);
return p.stage
"#;
