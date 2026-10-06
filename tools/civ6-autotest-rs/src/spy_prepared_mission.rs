//! A finite diagnostic for one *new* official neutralization by the returned,
//! Gain Sources proven spy. The host owns physical saves, loads and turns.
//! This module does not install a View hook or modify the old mission handler.
//! Host order: UI_SETUP; P.Prepare; CORE_COMMIT; physical save; P.ArmAfterSave
//! (MISSION); P.SendOnce. If the native active operation is visible, call
//! P.ObserveSent; CORE_COMMIT; physical save. Route the event through
//! UI_EARLY_EVENT_GATE; P.Result; CORE_COMMIT; physical save. Only SUCCEEDED
//! permits P.ObserveRecovery / P.PositiveChallenge, each followed by
//! CORE_COMMIT and a physical save. After natural recovery, P.PrepareAssignment;
//! CORE_COMMIT; physical save; P.ArmAfterSave(ASSIGN); P.AssignOnce;
//! P.ObserveAssignment; CORE_COMMIT; physical save. On any caught exception,
//! call P.CaptureUnknown, UNKNOWN_CORE, then physical save; never resend.
//! Resume after the same UI LoadScreenClose observes saved state without arms.

pub const PROPERTY: &str = "JAC_BOUNDARY_PREPARED_MISSION";
pub const WITNESS: &str = "JAC_PreparedMission548Witness";

pub const UI_SETUP: &str = r#"
assert(JAC_PreparedMission548==nil,'prepared mission already installed');
local KEY='JAC_BOUNDARY_PREPARED_MISSION';
local PKEY='JAC_BOUNDARY_RETURNED_PREPARATION';
local EKEY='JAC_BOUNDARY_GROUP_EXCHANGE_62';
local WKEY='JAC_PreparedMission548Witness';
local B=assert(ExposedMembers.JAC_PersonaBoundary,'boundary reader missing');
assert(type(B.ReadEnemy)=='function','boundary reader missing');
local P={}; JAC_PreparedMission548=P;
local x=nil;
local function integer(v,min)
 return type(v)=='number' and v==v and v>=min and
  v<9007199254740992 and v==math.floor(v)
end
local function copy(v,depth)
 depth=depth or 0;
 if type(v)~='table' then
  if v==nil or type(v)=='string' or type(v)=='boolean' or
   (type(v)=='number' and v==v and math.abs(v)<math.huge) then return v end
  return {nativeType=type(v),nativeText=tostring(v)}
 end
 assert(depth<14,'observation nesting exceeds bound');
 local out={}; local n=0;
 for k,a in pairs(v) do
  n=n+1; assert(n<=512,'observation size exceeds bound');
  assert(type(k)=='string' or integer(k,0),'non-scalar observation key');
  out[k]=copy(a,depth+1)
 end
 return out
end
local function equal(a,b,depth)
 if type(a)~=type(b) then return false end
 if type(a)~='table' then return a==b end
 if (depth or 0)>14 then return false end
 for k,v in pairs(a) do if not equal(v,b[k],(depth or 0)+1) then return false end end
 for k in pairs(b) do if a[k]==nil then return false end end
 return true
end
local function history(n,id)
 assert(type(n)=='table','native mission history absent');
 return {id=id,operation=n.Operation,plot=n.PlotIndex,turn=n.CompletionTurn,
  initial=n.InitialResult,escape=n.EscapeResult,raw=copy(n)}
end
local function sameHistory(a,b)
 return a and b and a.id==b.id and a.operation==b.operation and
  a.plot==b.plot and a.turn==b.turn and a.initial==b.initial and
  a.escape==b.escape
end
local function protected()
 return {exchange=copy(Game:GetProperty(EKEY)),
  preparation=copy(Game:GetProperty(PKEY)),
  archive=copy(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE')),
  state=copy(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE')),
  ack=copy(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK')),
  oldEvent=copy(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'))}
end
local function proof(saved)
 local e=assert(Game:GetProperty(EKEY),'formal return missing');
 local p=assert(Game:GetProperty(PKEY),'Sources receipt missing');
 local q=assert(p.sourcesReceipt,'Sources result missing');
 local ev=assert(q.event,'Sources native event missing');
 local source=assert(p.sources,'Sources intent missing');
 local r=assert(e.returnEvidence,'formal return evidence missing');
 assert(e.stage=='RETURNED' and r.stage=='RETURNED_MAP' and
  r.returned and r.returned.owner==0 and r.returned.unitType==18 and
  r.after and r.after.physical==3 and
  #r.after.captured==2 and #r.after.map==1 and
  #r.after.activeOffMap==0 and r.after.map[1].id==r.returned.id and
  r.after.map[1].owner==0 and
  e.before and e.before.physical==3 and #e.before.captured==3 and
  e.nonce==p.exchangeNonce and p.returnedID==r.returned.id and
  p.stage=='SOURCES_RECEIPT' and p.outcome=='SOURCES_COMPLETE' and
  q.stage=='SOURCES_COMPLETE' and p.arrival and p.arrival.unit and
  source.spy and source.target and source.gainOperation and
  source.comparisonOperation and q.after and q.after.spy and
  q.after.spy.owner==0 and
  q.after.spy.unitType==source.spy.unitType and
  q.after.spy.x==source.target.x and q.after.spy.y==source.target.y and
  q.after.sourceTurns and q.after.sourceTurns>0 and
  q.effectState=='POSITIVE' and
  q.sourceTurnsDelta==q.after.sourceTurns and
  q.after.spyInTarget==true and q.comparison and
  q.comparison.delta>=0 and source.sourceTurns==0 and
  ev.attacker==0 and ev.id==3 and ev.spyID==source.spy.id and
  ev.spyID==p.arrival.unit.id and ev.spyID==q.after.spy.id and
  ev.targetOwner==1 and ev.targetCity==source.target.city and
  ev.plot==source.target.plot and
  (ev.operation==source.gainOperation.index or
   ev.operation==source.gainOperation.hash) and
  ev.initial~=nil and ev.escape~=nil and ev.turn>=source.turn and
  ev.raw and ev.raw.Operation==ev.operation and
  ev.raw.PlotIndex==ev.plot and ev.raw.CompletionTurn==ev.turn and
  ev.raw.InitialResult==ev.initial and ev.raw.EscapeResult==ev.escape,
  'real return and successful Sources receipt not proved');
 local enums=assert(ev.successEnums,'Sources result enums missing');
 assert((ev.initial==enums.undetected or ev.initial==enums.mustEscape) and
  type(enums.undetected)=='number' and type(enums.mustEscape)=='number' and
  source.comparisonProbability and source.comparisonProbability.available and
  q.after.comparisonProbability and q.after.comparisonProbability.available and
  q.comparison.before==source.comparisonProbability.success and
  q.comparison.after==q.after.comparisonProbability.success and
  q.comparison.delta==q.comparison.after-q.comparison.before,
  'Sources outcome or probability is not positive evidence');
 local d=Players[0]:GetDiplomacy();
 assert(type(d.GetMission)=='function','native mission getter unavailable');
 local n=history(d:GetMission(0,3),3);
 assert(sameHistory(n,ev),'Sources native history changed');
 local a=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'));
 local ack=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK'));
 local state=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE'));
 local oldEvent=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'));
 assert(state.missionPost and
  (state.missionPost.histories=='UI' or state.missionPost.histories=='CORE') and
  oldEvent.attacker==0 and oldEvent.id==2 and oldEvent.turn==62 and
  ack.observed and ack.observed.preserved and ack.observed.mission and
  p.oldHistories and #p.oldHistories==3 and
  e.before and e.before.histories and #e.before.histories==3 and
  r.after.histories and #r.after.histories==3,
  'old three mission lineage unavailable');
 for i=1,3 do
  local old=i<3 and a.histories[i] or ack.observed.mission;
  local native=history(d:GetMission(0,i-1),i-1);
  assert(old and old.id==i-1 and old.turn==({23,45,62})[i] and
   old.initial==1 and old.escape==-1 and
   sameHistory(old,p.oldHistories[i]) and
   sameHistory(old,e.before.histories[i]) and
   sameHistory(old,r.after.histories[i]) and
   sameHistory(old,native) and
   (i==3 or sameHistory(old,ack.observed.preserved[i])),
   'old native failure history changed')
 end
 if saved then assert(equal(saved,protected()),'protected historical content changed') end
 return p,e
end
local function role()
 assert(Game.GetLocalPlayer()==0 and Players[0]:IsHuman() and
  not Players[1]:IsHuman() and Players[0]:IsTurnActive(),
  'attacker turn or roles changed')
end
local function target(p)
 local t=assert(p.sources.target);
 assert(t.owner==1 and integer(t.city,0) and integer(t.plot,0),
  'Sources target identity missing');
 local c=assert(CityManager.GetCity(1,t.city),'target city missing');
 local plot=assert(Map.GetPlot(c:GetX(),c:GetY()),'target plot missing');
 assert(c:GetOwner()==1 and c:GetID()==t.city and
  c:GetX()==t.x and c:GetY()==t.y and plot:GetIndex()==t.plot,
  'native target city changed');
 return c,plot
end
local function spy(p,requireIdle)
 local id=p.sources.spy.id;
 local units=assert(Players[0]:GetUnits(),'native units missing');
 assert(type(units.FindID)=='function','native FindID missing');
 local u=assert(units:FindID(id),'prepared spy missing');
 local def=assert(GameInfo.Units['UNIT_SPY'],'spy definition missing');
 assert(u:GetID()==id and u:GetOwner()==0 and u:GetType()==def.Index and
  id==p.sourcesReceipt.after.spy.id and
  p.sources.spy.owner==0 and p.sources.spy.unitType==def.Index,
  'FindID resolved another or reused spy');
 local c,plot=target(p);
 local purchase=Cities.GetPlotPurchaseCity(Map.GetPlot(u:GetX(),u:GetY()));
 assert(u:GetX()==c:GetX() and u:GetY()==c:GetY() and
  purchase and purchase:GetOwner()==1 and purchase:GetID()==c:GetID(),
  'prepared spy left target city');
 if requireIdle then assert(u:GetSpyOperation()==-1,'spy has an operation') end
 return u,plot,{id=u:GetID(),owner=u:GetOwner(),unitType=u:GetType(),
  x=u:GetX(),y=u:GetY(),operation=u:GetSpyOperation(),
  endTurn=u:GetSpyOperationEndTurn()}
end
local function governor(p)
 local s=assert(B.ReadEnemy(),'enemy native snapshot missing');
 local config=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG'));
 local c,plot=target(p);
 assert(s.owner==1 and s.attacker==0 and s.leader==config.leader and
  s.turn==Game.GetCurrentGameTurn() and config.owner==1 and
  config.cityID==c:GetID() and s.target and s.target.id==c:GetID() and
  s.target.owner==1 and s.target.plot==plot:GetIndex() and
  s.personOwner==1 and s.state=='GOVERNOR' and
  s.merit==s.gameplay.merit,
  'joined defender persona changed');
 local row=assert(GameInfo.JAC_LeaderPersonas[config.leader]);
 local def=assert(GameInfo.Governors[row.GovernorType]);
 local gs=assert(Players[1]:GetGovernors());
 assert(gs:HasGovernor(def.Hash),'native defender Governor missing');
 local g=assert(gs:GetGovernor(def.Hash));
 local assigned=g:GetAssignedCity();
 local cityOwner=assigned and assigned:GetOwner() or -1;
 local cityID=assigned and assigned:GetID() or -1;
 local remaining=g:GetNeutralizedTurns();
 assert(g:GetType()==def.Index and s.native.type==def.Index and
  s.native.hash==def.Hash and s.native.cityOwner==cityOwner and
  s.native.cityID==cityID and s.native.neutralized==remaining and
  integer(remaining,0),'native Governor and public snapshot disagree');
 return s,gs,g,{owner=1,leader=config.leader,type=def.Index,hash=def.Hash,
  city=cityID,cityOwner=cityOwner,neutralized=remaining,
  established=g:IsEstablished(),turn=s.turn,targetCity=c:GetID(),
  targetPlot=plot:GetIndex()}
end
local function effects(s,active)
 local out={}; local count=0;
 assert(type(s.cities)=='table' and type(s.gameplay.cities)=='table');
 for key,city in pairs(s.cities) do
  local own=assert(s.gameplay.cities[key],'gameplay city ledger missing');
  assert(city.id==own.id and city.owner==own.owner and
   city.settled==own.settled,'city effect owner or marker differs');
  local expected=active and key==s.targetKey and 1 or 0;
  assert(city.settled==expected and type(city.effects)=='table' and
   next(city.effects),'native effect inventory/marker differs');
  out[key]={id=city.id,owner=city.owner,settled=city.settled,effects={}};
  for modifier,n in pairs(city.effects) do
   assert(n==expected,'native persona effect activity differs');
   out[key].effects[modifier]=n; count=count+1
  end
 end
 assert(count>0 and out[s.targetKey],'target effect inventory missing');
 assert(type(s.unmatchedEffects)=='table' and not next(s.unmatchedEffects),
  'unmatched native effect subject');
 return out
end
local function publish(phase,observation)
 ExposedMembers[WKEY]=copy({phase=phase,nonce=x and x.nonce,
  turn=Game.GetCurrentGameTurn(),observation=observation})
end
local function current()
 local p=assert(Game:GetProperty(KEY),'saved prepared mission missing');
 assert(x and p.nonce==x.nonce,'prepared mission nonce differs');
 proof(p.protected);
 return p
end
local function mission4()
 return Players[0]:GetDiplomacy():GetMission(0,4)
end
function P.Prepare()
 role(); assert(not x and not Game:GetProperty(KEY),'mission already reserved');
 local p,e=proof(); local u,plot,n=spy(p,true);
 local s,_,_,g=governor(p);
 assert(g.cityOwner==1 and g.city==p.sources.target.city and
  g.established==true and g.neutralized==0,
  'defender Governor is not active in target city');
 local op=assert(GameInfo.UnitOperations['UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR']);
 assert(op.Index==p.sources.comparisonOperation.index and
  op.Hash==p.sources.comparisonOperation.hash,
  'official neutralize operation differs from Sources comparison');
 local free=mission4(); assert(free==nil or free==0,'mission ID 4 is used');
 -- Installed Base/UI/Popups/EspionagePopup.lua:181,280,474.
 local can,reason=UnitManager.CanStartOperation(u,op.Hash,plot,false,true);
 local duration=UnitManager.GetTimeToComplete(op.Index,u);
 local probability=UnitManager.GetResultProbability(op.Index,u,plot);
 assert(can==true and integer(duration,1) and type(probability)=='table' and
  type(probability.ESPIONAGE_SUCCESS_UNDETECTED)=='number',
  'native neutralization is not eligible');
 local tree=assert(GameInfo.JAC_PersonaTreePeople[s.leader]);
 local rule=assert(GameInfo.JAC_PersonaMeritRules[tree.MeritProfile]);
 assert(type(rule.GOVERN_TURN)=='number' and rule.GOVERN_TURN>0,
  'positive regular merit challenge unavailable');
 local before={turn=Game.GetCurrentGameTurn(),spy=n,governor=g,
  merit=s.merit,nodesOwned=copy(s.nodesOwned),effects=effects(s,true),
  operation={index=op.Index,hash=op.Hash},target=copy(p.sources.target),
  duration=duration,probability=copy(probability),canStart=can,
  canReason=copy(reason),positiveGovernTurn=rule.GOVERN_TURN,
  sourcesEvent=copy(p.sourcesReceipt.event),exchangeNonce=e.nonce,
  preparationNonce=p.nonce,unusedMission4=true};
 x={nonce=p.nonce..':neutral:'..tostring(UI.GetElapsedTime()),
  protected=protected(),before=copy(before),fresh=true};
 publish('READY',{before=before,protected=x.protected});
 return copy(before)
end
function P.HasState() return x~=nil end
function P.Resume()
 assert(not x,'UI state already installed');
 local saved=assert(Game:GetProperty(KEY),'saved mission missing');
 if saved.stage=='UNKNOWN_CORE' then
  x={nonce=saved.nonce,resumed=true,unknown=true};
  publish('RESUMED',{stage='UNKNOWN_CORE',sendPermission=false});
  return 'UNKNOWN_CORE'
 end
 assert(type(saved.nonce)=='string' and saved.nonce~='' and
  saved.protected and saved.before and saved.before.sourcesEvent and
  saved.before.sourcesEvent.id==3 and
  saved.before.spy.id==saved.protected.preparation.sources.spy.id,
  'saved mission provenance missing');
 proof(saved.protected);
 x={nonce=saved.nonce,protected=copy(saved.protected),
  before=copy(saved.before),resumed=true};
 local ok,n=pcall(mission4);
 local stage=saved.stage;
 if not ok then stage='NATIVE_GETTER_UNKNOWN'
 elseif saved.stage=='MISSION_INTENT' then
  stage=saved.sent and 'SENT_OBSERVED' or 'MISSION_UNCONFIRMED'
 elseif saved.receipt and
  (type(n)~='table' or
   not sameHistory(history(n,4),saved.receipt.event)) then
  stage='HISTORY_UNCONFIRMED'
 end
 publish('RESUMED',{stage=stage,mission4Read={ok=ok,raw=copy(n)},
  sendPermission=false});
 return stage
end
function P.ArmAfterSave(action)
 role(); local saved=current();
 if action=='MISSION' then
  assert(x.fresh and not x.resumed and not x.sent and not x.armed and
   saved.stage=='MISSION_INTENT' and equal(saved.before,x.before),
   'fresh saved mission intent required');
  x.armed=true; return 'MISSION_ARMED'
 end
 assert(action=='ASSIGN' and x.freshAssignment and not x.assignSent and
  not x.assignArmed and saved.stage=='ASSIGN_INTENT',
  'fresh saved assignment intent required');
 x.assignArmed=true; return 'ASSIGN_ARMED'
end
function P.SendOnce()
 role(); local saved=current();
 assert(x.fresh and x.armed and not x.sent and
  saved.stage=='MISSION_INTENT','mission send is not armed');
 local p=proof(saved.protected); local u,plot,n=spy(p,true);
 local _,_,_,g=governor(p);
 assert(equal(n,x.before.spy) and g.type==x.before.governor.type and
  g.hash==x.before.governor.hash and g.city==x.before.governor.city and
  g.neutralized==0,'mission actors changed after physical save');
 local op=assert(GameInfo.UnitOperations['UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR']);
 local can=UnitManager.CanStartOperation(u,op.Hash,plot,false,true);
 local free=mission4();
 assert(can==true and (free==nil or free==0) and
  op.Hash==x.before.operation.hash,'native mission readiness changed');
 UI.DeselectAllUnits(); UI.DeselectAllCities(); UI.SelectUnit(u);
 assert(UI.GetHeadSelectedUnit() and
  UI.GetHeadSelectedUnit():GetID()==n.id and
  UI.GetHeadSelectedUnit():GetOwner()==0,'official spy selection changed');
 UI.SetInterfaceMode(InterfaceModeTypes.SPY_CHOOSE_MISSION);
 assert(UI.GetInterfaceMode()==InterfaceModeTypes.SPY_CHOOSE_MISSION and
  UI.GetHeadSelectedUnit():GetID()==n.id,'official mission mode changed');
 x.sent=true; x.sentTurn=Game.GetCurrentGameTurn();
 UnitManager.RequestOperation(u,op.Hash);
 return 'MISSION_SENT'
end
-- Optional checkpoint before a physical save. A reloaded UI may own a later
-- event only when Core saved this actual active operation before completion.
function P.ObserveSent()
 local saved=current();
 assert(saved.stage=='MISSION_INTENT' and x.sent and not x.resumed and
  not saved.sent,'fresh native send observation required');
 local p=proof(saved.protected); local u,_,n=spy(p,false);
 assert(n.id==saved.before.spy.id and
  (n.operation==saved.before.operation.index or
   n.operation==saved.before.operation.hash) and
  integer(n.endTurn,Game.GetCurrentGameTurn()),
  'native mission operation did not start');
 local out={spy=n,turn=Game.GetCurrentGameTurn(),
  operation=saved.before.operation,missionID=4};
 publish('SENT',out); return copy(out)
end
function P.CaptureUnknown(step,raw)
 local saved=Game:GetProperty(KEY);
 if not x and saved then
  x={nonce=saved.nonce,protected=saved.protected,
   before=saved.before,resumed=true}
 end
 assert(type(step)=='string' and step~='' and
  ((saved and x and saved.nonce==x.nonce) or
   (not saved and x and x.fresh)),
  'unknown record has no owned mission');
 x.unknown=true;
 publish('UNKNOWN',{step=step,raw=copy(raw),
  mission4Read=(function() local ok,n=pcall(mission4); return {ok=ok,raw=copy(n)} end)()});
 return 'UNKNOWN_UI'
end
function P.RouteEvent(attacker,missionID)
 local saved=Game:GetProperty(KEY);
 if not saved or attacker~=0 or type(missionID)~='number' or
  missionID<4 then return 'UNOWNED' end
 local ok,answer=pcall(function()
  if not x then P.Resume() end
  assert(saved.nonce==x.nonce and missionID==4 and
   (saved.stage=='MISSION_INTENT' or saved.stage=='SUCCEEDED' or
    saved.stage=='FAILED') and
   (saved.stage~='MISSION_INTENT' or x.sent or saved.sent),
   'new mission event send ownership unknown');
  local n=history(Players[0]:GetDiplomacy():GetMission(0,missionID),missionID);
  assert((n.operation==saved.before.operation.index or
   n.operation==saved.before.operation.hash) and
   n.plot==saved.before.target.plot and
   integer(n.turn,saved.before.turn) and
   n.turn==Game.GetCurrentGameTurn(),
   'new native mission history does not match owned intent');
  if saved.event then
   assert(sameHistory(n,saved.event),'duplicate native event changed');
  end
  x.event=copy(n);
  x.event.attacker=attacker;
  x.event.spyID=saved.before.spy.id;
  x.event.targetOwner=1;
  x.event.targetCity=saved.before.target.city;
  x.event.successEnums={undetected=EspionageResultTypes.SUCCESS_UNDETECTED,
   mustEscape=EspionageResultTypes.SUCCESS_MUST_ESCAPE,
   failUndetected=EspionageResultTypes.FAIL_UNDETECTED,
   failEscape=EspionageResultTypes.FAIL_MUST_ESCAPE,
   killed=EspionageResultTypes.KILLED,captured=EspionageResultTypes.CAPTURED};
  publish('EVENT',{event=x.event});
  return 'OWNED'
 end);
 if ok then return answer end
 P.CaptureUnknown('EVENT_GATE',{attacker=attacker,id=missionID,error=tostring(answer)});
 return 'OWNED_UNKNOWN'
end
function P.Result()
 local ok,result=pcall(function()
  local saved=current();
  assert(saved.stage=='MISSION_INTENT' and x.event and x.event.id==4,
   'owned new native event missing');
  local n=history(mission4(),4);
  assert(sameHistory(n,x.event),'new native result changed');
  local enums=x.event.successEnums;
  local success=(n.initial==enums.undetected or n.initial==enums.mustEscape) and
   type(enums.undetected)=='number' and type(enums.mustEscape)=='number';
  local failure=(n.initial==enums.failUndetected or
   n.initial==enums.failEscape or n.initial==enums.killed or
   n.initial==enums.captured) and
   type(n.initial)=='number';
  local govOK,s,gs,g,gov=pcall(governor,proof(saved.protected));
  local out={event=copy(x.event),native=copy(n),
   governorRead={ok=govOK,raw=govOK and copy(gov) or copy(s)},
   outcome='UNKNOWN_NATIVE',turn=Game.GetCurrentGameTurn()};
  if failure then out.outcome='FAILED'; out.failureResult=n.initial;
  elseif success and govOK and gov.neutralized>0 and
   gov.type==saved.before.governor.type and
   gov.hash==saved.before.governor.hash and
   gov.targetCity==saved.before.governor.targetCity and
   s.merit==s.gameplay.merit then
   out.effects=effects(s,false);
   out.merit=s.merit;
   out.outcome='SUCCEEDED'
  end
  publish('RESULT',out); return out.outcome
 end);
 if ok then return result end
 P.CaptureUnknown('RESULT',{error=tostring(result),event=x and x.event});
 return 'UNKNOWN_NATIVE'
end
local function confirmed()
 local saved=current();
 assert(saved.stage=='SUCCEEDED' or saved.stage=='NEUTRALIZED' or
  saved.stage=='RECOVERED' or saved.stage=='ASSIGN_INTENT' or
  saved.stage=='ASSIGN_ESTABLISHING' or
  saved.stage=='ASSIGN_ESTABLISHED','successful native receipt required');
 assert(saved.receipt and saved.receipt.outcome=='SUCCEEDED' and
  saved.receipt.event and saved.receipt.event.id==4 and
  sameHistory(history(mission4(),4),saved.receipt.event),
  'successful native mission history changed');
 return saved
end
local function adjustedMerit(s,before)
 local spent=0;
 for name in pairs(before.nodesOwned) do
  assert(s.nodesOwned[name]==1,'baseline persona skill was lost')
 end
 for name in pairs(s.nodesOwned) do
  if not before.nodesOwned[name] then
   local node=assert(GameInfo.JAC_PersonaTreeNodes[name]);
   assert(node.LeaderType==s.leader and integer(node.Cost,0),
    'new skill cost unavailable');
   spent=spent+node.Cost
  end
 end
 return s.merit+spent,spent
end
function P.ObserveRecovery()
 local saved=confirmed(); local p=proof(saved.protected);
 assert(saved.stage~='RECOVERED','recovery already recorded');
 local s,_,_,g=governor(p);
 assert(g.type==saved.before.governor.type and
  g.hash==saved.before.governor.hash and
  g.targetCity==saved.before.governor.targetCity and
  Game.GetCurrentGameTurn()>=saved.receipt.turn,
  'native recovered Governor identity changed');
 local stage=g.neutralized>0 and 'NEUTRALIZED' or 'RECOVERED';
 if stage=='RECOVERED' then
  assert(saved.recovery and saved.recovery.firstPositive and
   Game.GetCurrentGameTurn()>saved.recovery.firstPositive.turn and
   g.cityOwner==-1 and g.city==-1,
   'natural recovery or unassigned state not observed')
 end
 local effective,spent=adjustedMerit(s,saved.before);
 if g.neutralized>0 then
  assert(effective==saved.before.merit,'merit rose during neutralization');
  effects(s,false)
 end
 local out={stage=stage,turn=s.turn,governor=g,merit=s.merit,
  meritWithPurchases=effective,purchased=spent,effects=effects(s,false)};
 publish('RECOVERY',out); return stage
end
function P.PositiveChallenge()
 local saved=confirmed(); assert(saved.stage=='NEUTRALIZED',
  'neutralized interval required for positive challenge');
 assert(not saved.challenge,'positive challenge already recorded');
 local p=proof(saved.protected); local s,_,_,g=governor(p);
 local turn=Game.GetCurrentGameTurn();
 local prefix='JAC_PERSONA_'..s.leader..'_';
 local last=Game:GetProperty(prefix..'GOVERN_LAST_TURN');
 local deed=Game:GetProperty(prefix..'DEED_GOVERN_TURN_'..tostring(turn));
 local effective,spent=adjustedMerit(s,saved.before);
 assert(turn>saved.receipt.turn and g.neutralized>0 and
  saved.before.positiveGovernTurn>0 and last==turn and deed==nil and
  effective==saved.before.merit and s.merit==s.gameplay.merit,
  'positive regular turn challenge or merit stop unproved');
 local out={turn=turn,governor=g,governLastTurn=last,
  deed=deed,positiveRule=saved.before.positiveGovernTurn,
  merit=s.merit,meritWithPurchases=effective,purchased=spent,
  effects=effects(s,false)};
 publish('CHALLENGE',out); return 'CHALLENGE_PROVED'
end
function P.PrepareAssignment()
 role(); local saved=confirmed();
 assert(saved.stage=='RECOVERED' and not saved.assignment and
  not x.freshAssignment,'reassignment already reserved');
 local p=proof(saved.protected); local s,gs,g,actual=governor(p);
 local city=target(p);
 assert(actual.neutralized==0 and actual.cityOwner==-1 and
  actual.city==-1 and g:GetAssignedCity()==nil and
  gs:CanAssignGovernor(actual.hash,city)==true,
  'official reassignment is unavailable');
 local out={turn=Game.GetCurrentGameTurn(),governor=actual,
  targetCity=city:GetID(),merit=s.merit,effects=effects(s,false),
  canAssign=true};
 x.freshAssignment=true;
 publish('ASSIGN_READY',out); return copy(out)
end
function P.AssignOnce()
 role(); local saved=current();
 assert(saved.stage=='ASSIGN_INTENT' and x.freshAssignment and
  x.assignArmed and not x.assignSent,'saved assignment intent required');
 local p=proof(saved.protected); local _,gs,g,actual=governor(p);
 local city=target(p);
 assert(actual.neutralized==0 and actual.cityOwner==-1 and
  g:GetAssignedCity()==nil and
  gs:CanAssignGovernor(actual.hash,city)==true and
  saved.assignment and saved.assignment.targetCity==city:GetID(),
  'native assignment conditions changed');
 x.assignSent=true;
 -- Installed Expansion1/UI/Additions/GovernorPanel.lua:550-557 and the
 -- existing View's DispatchEnemy(REASSIGN) use this official operation.
 UI.RequestPlayerOperation(1,PlayerOperations.ASSIGN_GOVERNOR,
  {[PlayerOperations.PARAM_GOVERNOR_TYPE]=actual.type,
   [PlayerOperations.PARAM_PLAYER_ONE]=1,
   [PlayerOperations.PARAM_CITY_DEST]=city:GetID()});
 return 'ASSIGN_SENT'
end
function P.ObserveAssignment()
 local saved=confirmed();
 assert((saved.stage=='ASSIGN_INTENT' or
  saved.stage=='ASSIGN_ESTABLISHING') and saved.assignment,
  'saved assignment request missing');
 local p=proof(saved.protected); local s,_,_,g=governor(p);
 local stage='ASSIGN_UNKNOWN';
 if g.neutralized==0 and g.cityOwner==1 and
  g.city==saved.assignment.targetCity then
  if g.established then effects(s,true); stage='ASSIGN_ESTABLISHED'
  else effects(s,false); stage='ASSIGN_ESTABLISHING' end
 end
 local out={stage=stage,turn=s.turn,governor=g,merit=s.merit,
  effects=stage~='ASSIGN_UNKNOWN' and effects(s,g.established) or nil};
 publish('ASSIGN_RESULT',out); return stage
end
function P.InvalidateOnLoad()
 x=nil; ExposedMembers[WKEY]=nil;
 return 'UI_STATE_INVALIDATED'
end
function P.Evidence()
 return copy({ui=x,core=Game:GetProperty(KEY),witness=ExposedMembers[WKEY]})
end
return 'PREPARED_MISSION_UI_READY'
"#;

/// Father inserts this ahead of the existing Sources route in View's one
/// SpyMissionCompleted callback. An owned unknown never reaches legacy code.
pub const UI_EARLY_EVENT_GATE: &str = r#"
if Game:GetProperty('JAC_BOUNDARY_PREPARED_MISSION') then
 local prepared=assert(JAC_PreparedMission548,'prepared UI route missing');
 if prepared.RouteEvent(attacker,missionID)~='UNOWNED' then return end
end
"#;

/// Call from the same UI LoadScreenClose path as returned preparation.
pub const UI_RESUME_IF_SAVED: &str = r#"
local prepared=assert(JAC_PreparedMission548,'prepared UI module missing');
if Game:GetProperty('JAC_BOUNDARY_PREPARED_MISSION') and
 not prepared.HasState() then return prepared.Resume() end
return prepared.HasState() and 'ALREADY_OBSERVING' or 'NO_PREPARED_MISSION'
"#;

/// UI never writes Game properties. Host runs this only in Gameplay/Core after
/// a UI witness. It persists raw event/observation before any outcome assert.
pub const CORE_COMMIT: &str = r#"
local KEY='JAC_BOUNDARY_PREPARED_MISSION';
local w=assert(ExposedMembers.JAC_PreparedMission548Witness,'prepared UI witness missing');
local old=Game:GetProperty(KEY);
local phase=assert(w.phase);
local function copy(v)
 if type(v)~='table' then return v end
 local out={}; for k,a in pairs(v) do out[k]=copy(a) end; return out
end
local function equal(a,b)
 if type(a)~=type(b) then return false end
 if type(a)~='table' then return a==b end
 for k,v in pairs(a) do if not equal(v,b[k]) then return false end end
 for k in pairs(b) do if a[k]==nil then return false end end
 return true
end
local function mission(d,id)
 if type(d.GetMission)~='function' then return nil,'missing' end
 local ok,n=pcall(d.GetMission,d,0,id);
 return ok and n or nil,ok and 'ok' or tostring(n)
end
local function checkedMission(d,id)
 local n,status=mission(d,id);
 assert(status=='ok' or status=='missing',
  'Core native mission getter failed: '..status);
 return n,status
end
local function same(n,h)
 return type(n)=='table' and h and n.Operation==h.operation and
  n.PlotIndex==h.plot and n.CompletionTurn==h.turn and
  n.InitialResult==h.initial and n.EscapeResult==h.escape
end
local next=old and copy(old) or {};
next.lastRaw=copy(w);
next.lastRawTurn=Game.GetCurrentGameTurn();
if not old then
 assert(phase=='READY' and type(w.nonce)=='string' and w.nonce~='',
  'new mission requires ready witness');
 next.nonce=w.nonce; next.stage='RAW_READY'
else
 assert(w.nonce==old.nonce,'prepared witness nonce differs')
end
Game:SetProperty(KEY,next);
local ok,result=pcall(function()
 local prep=assert(Game:GetProperty('JAC_BOUNDARY_RETURNED_PREPARATION'));
 local e=assert(Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62'));
 local d=assert(Players[0]:GetDiplomacy());
 local protected=phase=='READY' and assert(w.observation.protected) or
  assert(old.protected);
 assert(equal(protected.preparation,prep) and
  equal(protected.exchange,e) and
  equal(protected.archive,Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE')) and
  equal(protected.state,Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE')) and
  equal(protected.ack,Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK')) and
  equal(protected.oldEvent,Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION')),
  'saved historical properties changed');
 assert(prep.stage=='SOURCES_RECEIPT' and prep.outcome=='SOURCES_COMPLETE' and
  prep.sourcesReceipt and prep.sourcesReceipt.stage=='SOURCES_COMPLETE' and
  prep.sourcesReceipt.event and prep.sourcesReceipt.event.id==3 and
  prep.sourcesReceipt.after.sourceTurns>0 and e.stage=='RETURNED',
  'actual return/Sources receipt unavailable');
 local archived=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'));
 local ack=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK'));
 for i=0,2 do
  local n,status=checkedMission(d,i);
  if status=='ok' then
   local h=i<2 and archived.histories[i+1] or ack.observed.mission;
   assert(same(n,h) and h.id==i and h.turn==({23,45,62})[i+1] and
    h.initial==1 and h.escape==-1,'Core old native history differs')
  end
 end
 local source,status=checkedMission(d,3);
 if status=='ok' then assert(same(source,prep.sourcesReceipt.event),
  'Core Sources native result differs') end
 local before=phase=='READY' and assert(w.observation.before) or old.before;
 assert(before and before.sourcesEvent and before.sourcesEvent.id==3 and
  before.spy.id==prep.sources.spy.id and before.spy.owner==0 and
  before.spy.unitType==prep.sources.spy.unitType and
  before.operation.index==prep.sources.comparisonOperation.index and
  before.operation.hash==prep.sources.comparisonOperation.hash and
  before.target.plot==prep.sources.target.plot and
  before.governor.owner==1 and before.governor.city==prep.sources.target.city,
  'prepared identity differs from proven Sources');
 local meritKey='JAC_PERSONA_'..before.governor.leader..'_MERIT';
 -- Live Core FindID has returned a different ID. UI checks ID/owner/type,
 -- coordinates and operation on the actual object; Core cross-checks the
 -- saved scalar identity against Sources and independent native histories.
 local op=assert(GameInfo.UnitOperations['UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR']);
 assert(op.Index==before.operation.index and op.Hash==before.operation.hash,
  'Core official operation differs');
 if phase=='READY' then
  assert(not old and before.unusedMission4==true and before.canStart==true and
   before.duration>0 and before.positiveGovernTurn>0 and
   (Game:GetProperty(meritKey) or 0)==before.merit,
   'new mission preflight incomplete');
  local n,availability=checkedMission(d,4);
  if availability=='ok' then assert(n==nil or n==0,'native mission ID 4 used') end
  next.stage='MISSION_INTENT'; next.protected=protected;
  next.before=before; next.coreAvailability={mission4=availability,
   spyFindID='KNOWN_UNRELIABLE_NO_CALL'};
 elseif phase=='SENT' then
  local q=assert(w.observation);
  assert(old.stage=='MISSION_INTENT' and not old.sent and
   q.missionID==4 and q.spy and q.spy.id==before.spy.id and
   q.spy.owner==0 and q.spy.unitType==before.spy.unitType and
   q.spy.x==before.spy.x and q.spy.y==before.spy.y and
   (q.spy.operation==op.Index or q.spy.operation==op.Hash) and
   type(q.spy.endTurn)=='number' and q.spy.endTurn>=q.turn and
   q.turn==Game.GetCurrentGameTurn(),
   'native mission start observation differs');
  next.sent=q;
 elseif phase=='RESULT' then
  local q=assert(w.observation); local ev=assert(q.event);
  assert(old.stage=='MISSION_INTENT' and ev.id==4 and
   ev.attacker==0 and ev.spyID==before.spy.id and
   ev.targetOwner==1 and ev.targetCity==before.target.city and
   ev.plot==before.target.plot and
   (ev.operation==op.Index or ev.operation==op.Hash) and
   q.native and same(q.native.raw,ev),
   'owned new native mission receipt differs');
  local n,status=checkedMission(d,4);
  if status=='ok' then assert(same(n,ev),'Core new native mission differs') end
  local enums=assert(ev.successEnums);
  local success=(type(enums.undetected)=='number' and
   ev.initial==enums.undetected) or
   (type(enums.mustEscape)=='number' and ev.initial==enums.mustEscape);
  local failure=(type(enums.failUndetected)=='number' and
   ev.initial==enums.failUndetected) or
   (type(enums.failEscape)=='number' and ev.initial==enums.failEscape) or
   (type(enums.killed)=='number' and ev.initial==enums.killed) or
   (type(enums.captured)=='number' and ev.initial==enums.captured);
  next.event=ev; next.receipt=q;
  if q.outcome=='SUCCEEDED' then
   assert(success and q.governorRead.ok and
    q.governorRead.raw.neutralized>0 and
    q.governorRead.raw.owner==1 and
    q.governorRead.raw.type==before.governor.type and
    q.governorRead.raw.hash==before.governor.hash and
    q.effects and q.merit==(Game:GetProperty(meritKey) or 0),
    'successful mission lacks native Governor/effect evidence');
   next.stage='SUCCEEDED';
   next.recovery={firstPositive=q.governorRead.raw,observations={}};
  elseif q.outcome=='FAILED' then
   assert(failure and q.failureResult==ev.initial,
    'native failure enum differs');
   next.stage='FAILED'
  else
   next.stage='UNKNOWN_NATIVE'
  end
 elseif phase=='RECOVERY' then
  local q=assert(w.observation);
  assert((old.stage=='SUCCEEDED' or old.stage=='NEUTRALIZED') and
   old.receipt.outcome=='SUCCEEDED' and
   q.turn==Game.GetCurrentGameTurn() and
   q.governor.type==before.governor.type and
   q.governor.hash==before.governor.hash and
   q.governor.owner==1 and q.governor.neutralized>=0 and
   q.meritWithPurchases==before.merit and
   q.merit==(Game:GetProperty(meritKey) or 0) and q.effects,
   'native recovery/merit observation differs');
  local n,status=checkedMission(d,4);
  if status=='ok' then assert(same(n,old.receipt.event),
   'Core mission changed during recovery') end
  if q.stage=='RECOVERED' then
   assert(q.governor.neutralized==0 and q.governor.cityOwner==-1 and
    q.turn>old.recovery.firstPositive.turn,'natural recovery unproved');
   next.stage='RECOVERED'
  else
   assert(q.stage=='NEUTRALIZED' and q.governor.neutralized>0,
    'neutralized interval unproved');
   next.stage='NEUTRALIZED'
  end
  assert(#old.recovery.observations<64,'finite recovery bound exceeded');
  next.recovery.observations[#next.recovery.observations+1]=q;
 elseif phase=='CHALLENGE' then
  local q=assert(w.observation);
  assert(old.stage=='NEUTRALIZED' and q.governor.neutralized>0 and
   q.positiveRule==before.positiveGovernTurn and q.positiveRule>0 and
   q.turn>old.receipt.turn and q.turn==Game.GetCurrentGameTurn() and
   q.governLastTurn==q.turn and q.deed==nil and
   q.meritWithPurchases==before.merit and
   q.merit==(Game:GetProperty(meritKey) or 0) and q.effects,
   'real positive merit challenge unproved');
  local prefix='JAC_PERSONA_'..before.governor.leader..'_';
  assert(Game:GetProperty(prefix..'GOVERN_LAST_TURN')==q.turn and
   Game:GetProperty(prefix..'DEED_GOVERN_TURN_'..tostring(q.turn))==nil,
   'Core product merit turn differs');
  next.challenge=q
 elseif phase=='ASSIGN_READY' then
  local q=assert(w.observation);
  assert(old.stage=='RECOVERED' and not old.assignment and
   q.canAssign==true and q.governor.neutralized==0 and
   q.governor.cityOwner==-1 and q.targetCity==before.target.city and
   q.merit==(Game:GetProperty(meritKey) or 0),
   'native assignment intent unproved');
  next.stage='ASSIGN_INTENT'; next.assignment=q
 elseif phase=='ASSIGN_RESULT' then
  local q=assert(w.observation);
  assert((old.stage=='ASSIGN_INTENT' or
   old.stage=='ASSIGN_ESTABLISHING') and
   q.stage~='ASSIGN_UNKNOWN' and q.governor.neutralized==0 and
   q.governor.cityOwner==1 and
   q.governor.city==old.assignment.targetCity and q.effects and
   q.merit==(Game:GetProperty(meritKey) or 0),
   'official native assignment result unproved');
  next.assignmentReceipt=q;
  next.stage=q.stage
 else error('unknown prepared mission witness phase') end
 return next.stage
end);
if not ok then
 next.stage='UNKNOWN_CORE'; next.coreError=tostring(result);
 Game:SetProperty(KEY,next); return 'UNKNOWN_CORE'
end
Game:SetProperty(KEY,next); return result
"#;

/// Call after P.CaptureUnknown for any UI/native/Core read or send exception.
/// The host then makes a physical save; this is terminal for sending.
pub const UNKNOWN_CORE: &str = r#"
local KEY='JAC_BOUNDARY_PREPARED_MISSION';
local w=assert(ExposedMembers.JAC_PreparedMission548Witness,
 'prepared unknown witness missing');
assert(w.phase=='UNKNOWN' and w.observation and
 type(w.observation.step)=='string','unknown witness missing');
local p=Game:GetProperty(KEY) or {};
assert(not p.nonce or p.nonce==w.nonce,'unknown mission nonce differs');
local next={}; for k,v in pairs(p) do next[k]=v end;
next.nonce=w.nonce; next.stage='UNKNOWN_CORE';
next.unknown=w.observation; next.unknownTurn=Game.GetCurrentGameTurn();
Game:SetProperty(KEY,next); return 'UNKNOWN_CORE'
"#;
