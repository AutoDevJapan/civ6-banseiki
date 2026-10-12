//! Finite follow-up to a *proven* native group return. The host installs the
//! event gate before the old neutralization handler and owns all saves/turns.
//! These Lua 5.1 scripts neither create spies nor edit native mission history.

/// Initial observations have no saved operation intent. Keep their failures
/// separate so a diagnostic failure can never manufacture a travel permission.
pub const UNKNOWN_PROPERTY: &str = "JAC_BOUNDARY_RETURNED_PREPARATION_UNKNOWN";

pub const UI_SETUP: &str = r#"
assert(JAC_ReturnedPreparation548==nil,'returned preparation already installed');
local KEY='JAC_BOUNDARY_RETURNED_PREPARATION';
local FKEY='JAC_BOUNDARY_RETURNED_PREPARATION_UNKNOWN';
local EXCHANGE='JAC_BOUNDARY_GROUP_EXCHANGE_62';
local WKEY='JAC_ReturnedPreparation548Witness';
local B=assert(ExposedMembers.JAC_PersonaBoundary,'boundary UI reader missing');
assert(type(B.ReadEnemy)=='function','boundary UI reader missing');
local P={}; JAC_ReturnedPreparation548=P;
local x=nil;
local nativeRead=nil;
local failed=false;
local function integer(v,min)
 return type(v)=='number' and v==v and v>=min and v<9007199254740992 and v==math.floor(v)
end
local function scalar(v,depth)
 depth=depth or 0;
 if v==nil or type(v)=='string' or type(v)=='boolean' then return v end
 if type(v)=='number' then
  if v==v and math.abs(v)<math.huge then return v end
  return {nativeType='number',nativeText=tostring(v)}
 end
 if type(v)~='table' then return {nativeType=type(v),nativeText=tostring(v)} end
 if depth>=12 then return {nativeType='table',nativeText='depth-limit'} end
 local out={}; local count=0;
 for k,a in pairs(v) do
  count=count+1; assert(count<=256,'native observation exceeds finite bound');
  if type(k)=='string' or (type(k)=='number' and integer(k,0)) then
   out[k]=scalar(a,depth+1)
  else out['unknownKey'..count]={nativeType=type(k),nativeText=tostring(k),value=scalar(a,depth+1)} end
 end
 return out
end
local function sameHistory(a,b)
 return a and b and a.id==b.id and a.turn==b.turn and a.operation==b.operation and
  a.plot==b.plot and a.initial==b.initial and a.escape==b.escape
end
local function readSourceMission()
 local getterType='unobserved';
 local ok,value=pcall(function()
  local d=assert(Players[0]:GetDiplomacy(),'native diplomacy missing');
  getterType=type(d.GetMission);
  assert(type(d.GetMission)=='function','native mission getter missing');
  return d:GetMission(0,3)
 end);
 nativeRead={id=3,owner=0,getterType=getterType,ok=ok,rawType=type(value),raw=scalar(value)};
 assert(ok,'native Sources history read failed: '..tostring(value));
 return value
end
local function lineage(e)
 local a=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'),'old archive missing');
 local s=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE'),'old state missing');
 local ack=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK'),'old ACK missing');
 local event=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'),'old event missing');
 assert(s.missionPost and (s.missionPost.histories=='UI' or s.missionPost.histories=='CORE') and
  event.attacker==0 and event.id==2 and event.turn==62 and
  ack.observed and ack.observed.preserved and ack.observed.mission,
  'old history provenance differs');
 local h={a.histories[1],a.histories[2],ack.observed.mission};
 local d=Players[0]:GetDiplomacy();
 assert(type(d.GetMission)=='function','UI native mission getter missing');
 for i=0,2 do
  local expected=e.before.histories[i+1]; local now=h[i+1];
  assert(now and expected and now.id==i and now.turn==({23,45,62})[i+1] and
   now.initial==1 and now.escape==-1 and
   sameHistory(now,expected) and sameHistory(now,e.returnEvidence.after.histories[i+1]) and
   (i==2 or sameHistory(now,ack.observed.preserved[i+1])),
   'saved old failure history changed');
  local n=assert(d:GetMission(0,i),'native old mission missing');
  assert(n.Operation==now.operation and n.PlotIndex==now.plot and
   n.CompletionTurn==now.turn and n.InitialResult==now.initial and
   n.EscapeResult==now.escape,'native old mission changed')
 end
 return scalar(h)
end
local function exchange()
 local e=assert(Game:GetProperty(EXCHANGE),'official group exchange missing');
 local r=assert(e.returnEvidence,'native return receipt missing');
 local a=assert(r.after,'native after roster missing');
 assert(e.stage=='RETURNED' and e.turn==62 and e.finalIntent and
  r.stage=='RETURNED_MAP' and r.returned and r.before and
  r.before.turn==62 and r.before.physical==3 and #r.before.captured==3 and
  a.physical==3 and a.capacity==3 and #a.map==1 and #a.activeOffMap==0 and
  #a.captured==2 and r.returned.id==a.map[1].id and
  r.returned.owner==0 and r.returned.unitType==a.map[1].unitType and
  r.returned.x==a.map[1].x and r.returned.y==a.map[1].y and
  r.returned.turn==a.turn and r.sessionID==e.finalIntent.sessionID,
  'formal map return is unproven');
 lineage(e); return e
end
local function role()
 assert(Game.GetLocalPlayer()==0 and Players[0]:IsHuman() and
  not Players[1]:IsHuman() and Players[0]:IsTurnActive(),
  'native attacker is not the active human');
end
local function roster(e)
 local s=B.ReadEnemy(); local r=assert(s.spyRoster,'native UI spy roster missing');
 assert(s.attacker==0 and s.owner==1 and s.turn==Game.GetCurrentGameTurn() and
  r.physicalCount==3 and #r.captured==2 and
  #r.map+#r.activeOffMap==1,'returned roster changed');
 for _,c in ipairs(r.captured) do
  assert(c.captor==1 and c.name=='' and c.nameIndex==-1 and c.level==1,
   'anonymous captive group changed')
 end
 lineage(e);
 return r,s.turn
end
local function unit(id)
 local units=assert(Players[0]:GetUnits(),'UI spy units missing');
 assert(type(units.FindID)=='function','UI spy FindID missing');
 local u=assert(units:FindID(id),'returned spy ID missing');
 local def=assert(GameInfo.Units['UNIT_SPY'],'spy definition missing');
 assert(u:GetID()==id and u:GetOwner()==0 and u:GetType()==def.Index and
  GameInfo.Units[u:GetType()].Spy,'native returned spy identity differs');
 return u
end
local function target(owner,cityID)
 assert(owner==1 and integer(cityID,0),'foreign target owner/city differs');
 local c=assert(CityManager.GetCity(owner,cityID),'native target city missing');
 assert(c:GetOwner()==owner and c:GetID()==cityID,'native target city identity differs');
 local plot=assert(Map.GetPlot(c:GetX(),c:GetY()),'native target plot missing');
 assert(integer(plot:GetIndex(),0),'native target plot ID missing');
 return c,plot
end
local function atTarget(u)
 local c,plot=target(x.targetOwner,x.targetCity);
 local actual=assert(Map.GetPlot(u:GetX(),u:GetY()),'native spy plot missing');
 local purchase=Cities.GetPlotPurchaseCity(actual);
 return u:GetX()==c:GetX() and u:GetY()==c:GetY() and
  purchase and purchase:GetOwner()==x.targetOwner and purchase:GetID()==x.targetCity and
  plot:GetIndex()==x.targetPlot
end
local function nativeUnit(u)
 return {id=u:GetID(),owner=u:GetOwner(),unitType=u:GetType(),x=u:GetX(),y=u:GetY(),
  operation=u:GetSpyOperation(),endTurn=u:GetSpyOperationEndTurn()}
end
local function publish(phase,observation)
 ExposedMembers[WKEY]=scalar({phase=phase,nonce=x.nonce,exchangeNonce=x.exchangeNonce,
  returnedID=x.returnedID,targetOwner=x.targetOwner,targetCity=x.targetCity,
  targetPlot=x.targetPlot,observation=observation});
end
local function stillOwned()
 local e=exchange(); assert(x and e.nonce==x.exchangeNonce,'return UI instance changed');
 local p=Game:GetProperty(KEY);
 if p then assert(p.nonce==x.nonce and p.exchangeNonce==x.exchangeNonce,
  'saved return preparation belongs to another instance') end
 return e,p
end
function P.Setup()
 assert(not failed and not Game:GetProperty(FKEY),'initial observation failed; no new intent');
 assert(not x and not Game:GetProperty(KEY),'returned preparation already reserved');
 role(); local e=exchange(); local r=roster(e);
 assert(#r.map==1 and #r.activeOffMap==0 and r.map[1].id==e.returnEvidence.returned.id and
  r.map[1].level==1,
  'native map return differs from saved exchange');
 local u=unit(r.map[1].id); local n=nativeUnit(u);
 assert(n.x==r.map[1].x and n.y==r.map[1].y and
  n.x==e.returnEvidence.returned.x and n.y==e.returnEvidence.returned.y and
  n.operation==-1,'returned native spy unavailable for official travel');
 x={nonce=e.nonce..':'..tostring(UI.GetElapsedTime()),exchangeNonce=e.nonce,
  returnedID=n.id,returned=n};
 return 'RETURNED_READY'
end
function P.HasState() return x~=nil end
-- Observation only. A saved intent is never permission to send again. The
-- returned stages describe native evidence, not a reconstructed dispatch.
function P.Resume()
 assert(not x,'returned preparation UI state already installed');
 if Game:GetProperty(FKEY) then failed=true; return 'INITIAL_UNKNOWN' end
 local p=assert(Game:GetProperty(KEY),'saved preparation missing');
 local e=exchange(); local r=e.returnEvidence;
 assert(type(p.nonce)=='string' and p.nonce~='' and
  p.exchangeNonce==e.nonce and p.returnedID==r.returned.id and
  p.returnEvidence and p.returnEvidence.stage=='RETURNED_MAP' and
  p.returnEvidence.returned and
  p.returnEvidence.returned.id==r.returned.id and
  p.returnEvidence.returned.owner==r.returned.owner and
  p.returnEvidence.returned.unitType==r.returned.unitType and
  p.returnEvidence.returned.x==r.returned.x and
  p.returnEvidence.returned.y==r.returned.y and
  p.oldHistories and #p.oldHistories==3,
  'saved preparation is not for the proven return');
 local h=lineage(e);
 for i=1,3 do assert(sameHistory(h[i],p.oldHistories[i]),
  'saved original mission history changed') end
 local t=assert(p.travel and p.travel.target,'saved destination missing');
 local before=assert(p.travel.before,'saved returned snapshot missing');
 local u=assert(before.unit,'saved returned unit missing');
 assert(t.owner==1 and integer(t.city,0) and integer(t.plot,0) and
  u.id==p.returnedID and u.owner==0 and
  u.unitType==r.returned.unitType and u.x==r.returned.x and
  u.y==r.returned.y and u.operation==-1,
  'saved travel identity changed');
 local city,plot=target(t.owner,t.city);
 assert(city:GetX()==t.x and city:GetY()==t.y and plot:GetIndex()==t.plot,
  'saved travel destination changed');
 local candidate={resumed=true,nonce=p.nonce,exchangeNonce=e.nonce,
  returnedID=p.returnedID,returned=scalar(u),travelBefore=scalar(u),
  targetOwner=t.owner,targetCity=t.city,targetPlot=t.plot,
  targetX=t.x,targetY=t.y,
  travelArguments={[UnitOperationTypes.PARAM_X]=t.x,[UnitOperationTypes.PARAM_Y]=t.y},
  unknown=p.unknowns and #p.unknowns>0 or false};
 if p.transit then
  assert(p.transit.returnTurn and p.transit.turn and
   p.transit.returnTurn>=p.transit.turn and
   ((p.transit.x==t.x and p.transit.y==t.y) or
    (p.transit.x==-9999 and p.transit.y==-9999)),
   'saved off-map transit differs');
  candidate.sawTransit=true; candidate.transit=scalar(p.transit)
 end
 if p.stage=='TRAVEL_INTENT' then
  x=candidate;
  local s=B.ReadEnemy(); local rosterNow=assert(s.spyRoster);
  if #rosterNow.activeOffMap==1 and #rosterNow.map==0 then
   x.travelSent=true;
   local observation=P.Observe();
   assert(observation.stage=='IN_TRANSIT','native off-map travel not observed');
   publish('RESUMED',{stage='IN_TRANSIT',native=observation});
   return 'IN_TRANSIT'
  end
  if #rosterNow.map==1 and #rosterNow.activeOffMap==0 and
   candidate.sawTransit then
   x.travelSent=true;
   local observation=P.Observe();
   if observation.stage=='ARRIVED' then
    publish('RESUMED',{stage='ARRIVED',native=observation});
    return 'ARRIVED'
   end
  end
  publish('RESUMED',{stage='TRAVEL_UNCONFIRMED',native=scalar(rosterNow)});
  return 'TRAVEL_UNCONFIRMED'
 end
 local arrival=assert(p.arrival,'saved native arrival missing');
 assert(arrival.unit,'saved native arrival unit missing');
 assert(arrival.stage=='ARRIVED' and arrival.roster and
  arrival.roster.physical==3 and #arrival.roster.captured==2 and
  #arrival.roster.map==1 and #arrival.roster.activeOffMap==0 and
  arrival.roster.map[1].id==arrival.unit.id and
  arrival.unit.owner==0 and arrival.unit.unitType==u.unitType and
  arrival.unit.x==t.x and arrival.unit.y==t.y,
  'saved arrival receipt differs');
 candidate.travelSent=true; candidate.arrivedID=arrival.unit.id;
 candidate.receiptedArrival=true;
 if arrival.transit then
  candidate.sawTransit=true; candidate.transit=scalar(arrival.transit)
 end
 x=candidate;
 if p.stage=='TRAVEL_RECEIPT' then
  local observation=P.Observe();
  assert(observation.stage=='ARRIVED' and observation.unit.id==arrival.unit.id,
   'native arrived spy changed on resume');
  publish('RESUMED',{stage='TRAVEL_RECEIPT',native=observation});
  return 'TRAVEL_RECEIPT'
 end
 assert(p.stage=='SOURCES_INTENT' or p.stage=='SOURCES_RECEIPT',
  'unknown saved preparation stage');
 local q=assert(p.sources,'saved sources preparation missing');
 assert(q.spy and q.target and q.gainOperation,
  'saved sources identity missing');
 local op=assert(GameInfo.UnitOperations['UNITOPERATION_SPY_GAIN_SOURCES']);
 assert(q.spy.id==arrival.unit.id and q.spy.owner==0 and
  q.spy.unitType==arrival.unit.unitType and q.target.owner==1 and
  q.target.city==t.city and q.target.plot==t.plot and
  q.gainOperation.index==op.Index and q.gainOperation.hash==op.Hash and
  q.sourceTurns==0 and q.canStart==true,
  'saved sources identity or native operation changed');
 x.ready=scalar(q); x.sourcesSentTurn=q.turn;
 local native=readSourceMission();
 local found=Players[0]:GetUnits():FindID(arrival.unit.id);
 if found then
  assert(found:GetID()==arrival.unit.id and found:GetOwner()==0 and
   found:GetType()==arrival.unit.unitType,
   'native FindID reused a different spy');
 end
 if type(native)=='table' and
  (native.Operation==op.Index or native.Operation==op.Hash) and
  native.PlotIndex==t.plot and integer(native.CompletionTurn,q.turn) and
  native.CompletionTurn<=Game.GetCurrentGameTurn() then
  x.sourcesSent=true; x.sourcesNative='HISTORY'
 else
  if found and
   found:GetX()==t.x and found:GetY()==t.y then
   local current=found:GetSpyOperation();
   local endTurn=found:GetSpyOperationEndTurn();
   if (current==op.Index or current==op.Hash) and
    integer(endTurn,Game.GetCurrentGameTurn()) then
    x.sourcesSent=true; x.sourcesNative='ACTIVE_OPERATION';
    x.sourcesEndTurn=endTurn
   end
  end
 end
 if p.stage=='SOURCES_RECEIPT' then
  assert(p.sourcesReceipt and p.sourcesReceipt.event and
   p.sourcesReceipt.event.id==3 and x.sourcesSent,
   'saved sources receipt has no native completion');
  assert(type(native)=='table' and
   native.Operation==p.sourcesReceipt.event.operation and
   native.PlotIndex==p.sourcesReceipt.event.plot and
   native.CompletionTurn==p.sourcesReceipt.event.turn and
   native.InitialResult==p.sourcesReceipt.event.initial and
   native.EscapeResult==p.sourcesReceipt.event.escape,
   'saved sources result differs from native mission history');
  x.ownedEvent=scalar(p.sourcesReceipt.event);
  publish('RESUMED',{stage='SOURCES_RECEIPT',native=scalar(native)});
  return 'SOURCES_RECEIPT'
 end
 local stage=x.sourcesSent and 'SOURCES_OBSERVED' or 'SOURCES_UNCONFIRMED';
 publish('RESUMED',{stage=stage,native=scalar(native),
  operation=x.sourcesNative,endTurn=x.sourcesEndTurn});
 return stage
end
function P.Observe()
 local e,p=stillOwned(); local r,turn=roster(e);
 local out={turn=turn,roster={map=scalar(r.map),activeOffMap=scalar(r.activeOffMap),
  captured=scalar(r.captured),physical=r.physicalCount},stage='RETURNED'};
 if not x.travelSent then
  assert(#r.map==1 and r.map[1].id==x.returnedID and #r.activeOffMap==0,
   'returned spy changed before travel');
  local u=unit(x.returnedID); out.unit=nativeUnit(u);
  assert(out.unit.x==x.returned.x and out.unit.y==x.returned.y and
   out.unit.operation==-1,'returned spy moved before owned travel');
 elseif #r.activeOffMap==1 and #r.map==0 then
  local a=r.activeOffMap[1];
  assert(integer(a.returnTurn,turn) and
   ((a.x==x.targetX and a.y==x.targetY) or
    (a.x==-9999 and a.y==-9999)),
   'native travel off-map destination differs');
  x.sawTransit=true; x.transit=scalar(a);
  out.stage='IN_TRANSIT'; out.offMap=scalar(a)
 elseif #r.map==1 and #r.activeOffMap==0 then
  local id=r.map[1].id; local u=unit(id); local n=nativeUnit(u);
  assert(n.x==r.map[1].x and n.y==r.map[1].y and
   n.owner==r.map[1].owner and n.unitType==r.map[1].unitType,
   'native returned spy/map row differs');
  out.unit=n;
  if atTarget(u) and n.operation==-1 and
   (x.sawTransit or x.receiptedArrival) then
   -- A changed ID is adopted only after one observed off-map trip and a
   -- single native map candidate. No anonymous captive is assigned an ID.
   if x.arrivedID then assert(x.arrivedID==id,'arrival spy ID changed') end
   if x.transit then
    assert(turn>=x.transit.returnTurn,
     'native spy arrived before observed off-map return')
   else
    assert(id==x.returnedID,
     'changed ID has no observed off-map return')
   end
   x.arrivedID=id; out.stage='ARRIVED'; out.adopted=id~=x.returnedID;
   out.transit=scalar(x.transit);
  else
   assert(id==x.returnedID,'unknown map spy before observed arrival');
   out.stage='WAIT_TRAVEL'
  end
 else error('native return/travel roster is ambiguous') end
 if x.travelSent then
  assert(p and (p.stage=='TRAVEL_INTENT' or p.stage=='TRAVEL_RECEIPT' or
   p.stage=='SOURCES_INTENT' or p.stage=='SOURCES_RECEIPT'),
   'saved travel intent missing');
  publish('TRAVEL_OBSERVATION',out)
 end
 return scalar(out)
end
function P.PrepareTravel(owner,cityID)
 assert(not failed and not Game:GetProperty(FKEY),'initial observation failed; no travel intent');
 role(); local e,p=stillOwned(); assert(not x.unknown and not p and not x.targetCity,
  'travel already prepared');
 local before=P.Observe(); local u=unit(x.returnedID);
 assert(before.stage=='RETURNED' and u:IsReadyToMove() and
  UnitManager.GetActivityType(u)==ActivityTypes.ACTIVITY_AWAKE,
  'returned spy is not ready for the chooser');
 local home=Cities.GetPlotPurchaseCity(Map.GetPlot(u:GetX(),u:GetY()));
 assert(home and home:GetOwner()==0,'returned spy is not in a home city');
 local c,plot=target(owner,cityID);
 local args={[UnitOperationTypes.PARAM_X]=c:GetX(),[UnitOperationTypes.PARAM_Y]=c:GetY()};
 local can,reason=UnitManager.CanStartOperation(u,UnitOperationTypes.SPY_TRAVEL_NEW_CITY,plot,args);
 assert(can==true,'official native travel refused');
 local nextMission=Players[0]:GetDiplomacy():GetMission(0,3);
 assert(nextMission==nil or nextMission==0,
  'unused native mission ID 3 already occupied');
 x.targetOwner=owner; x.targetCity=cityID; x.targetPlot=plot:GetIndex();
 x.targetX=c:GetX(); x.targetY=c:GetY(); x.travelArguments=args;
 x.travelBefore=before.unit;
 publish('TRAVEL_READY',{before=before,canStart=can,reason=scalar(reason),
  target={owner=owner,city=cityID,x=x.targetX,y=x.targetY,plot=x.targetPlot},
  unusedMission3=true});
 return 'TRAVEL_READY'
end
function P.ArmAfterSave(action)
 assert(not failed and not Game:GetProperty(FKEY),'initial observation failed; no send arm');
 role(); local _,p=stillOwned();
 if action=='TRAVEL' then
 assert(not x.resumed and not x.unknown and x.targetCity and
  not x.travelArmed and not x.travelSent and
   p and p.stage=='TRAVEL_INTENT' and p.travel and
   p.travel.target.plot==x.targetPlot and p.travel.before.unit.id==x.returnedID,
   'saved travel intent missing');
  x.travelArmed=true; return 'TRAVEL_ARMED'
 end
 assert(action=='SOURCES' and not x.unknown and
  x.ready and x.freshSourcesPrepared and
  not x.sourcesArmed and
  not x.sourcesSent and p and p.stage=='SOURCES_INTENT' and p.sources and
  p.sources.spy.id==x.arrivedID and p.sources.target.plot==x.targetPlot,
  'saved sources intent missing');
 x.sourcesArmed=true; return 'SOURCES_ARMED'
end
function P.TravelOnce()
 role(); local _,p=stillOwned();
 assert(not x.resumed and not x.unknown and
  x.travelArmed and not x.travelSent and
  p.stage=='TRAVEL_INTENT',
  'travel is not saved or already sent');
 local u=unit(x.returnedID); local r=roster(exchange());
 assert(#r.map==1 and r.map[1].id==u:GetID() and #r.activeOffMap==0 and
  u:GetSpyOperation()==-1 and u:GetX()==x.travelBefore.x and
  u:GetY()==x.travelBefore.y,'native spy changed before travel');
 local c,plot=target(x.targetOwner,x.targetCity);
 local can=UnitManager.CanStartOperation(u,UnitOperationTypes.SPY_TRAVEL_NEW_CITY,
  plot,x.travelArguments);
 assert(can==true and c:GetX()==x.targetX and c:GetY()==x.targetY,
  'official travel eligibility or destination changed');
 UI.DeselectAllUnits(); UI.DeselectAllCities(); UI.SelectUnit(u);
 assert(UI.GetHeadSelectedUnit() and UI.GetHeadSelectedUnit():GetID()==x.returnedID and
  UI.GetHeadSelectedUnit():GetOwner()==0,
  'official UI selection differs');
 UI.SetInterfaceMode(InterfaceModeTypes.SPY_TRAVEL_TO_CITY);
 assert(UI.GetInterfaceMode()==InterfaceModeTypes.SPY_TRAVEL_TO_CITY and
  UI.GetHeadSelectedUnit() and UI.GetHeadSelectedUnit():GetID()==x.returnedID and
  UI.GetHeadSelectedUnit():GetOwner()==0,
  'official travel mode lost spy');
 x.travelSent=true; x.travelSentTurn=Game.GetCurrentGameTurn();
 UnitManager.RequestOperation(u,UnitOperationTypes.SPY_TRAVEL_NEW_CITY,x.travelArguments);
 UI.SetInterfaceMode(InterfaceModeTypes.SELECTION);
 return 'TRAVEL_SENT'
end
local function probability(operation,u,plot,required)
 local raw=UnitManager.GetResultProbability(operation.Index,u,plot);
 if type(raw)~='table' then
  assert(not required,'native comparison probability unavailable');
  return {available=false,raw=scalar(raw)}
 end
 local a=raw.ESPIONAGE_SUCCESS_UNDETECTED;
 local b=raw.ESPIONAGE_SUCCESS_MUST_ESCAPE or 0;
 if type(a)~='number' then
  assert(not required,'native comparison probability shape changed');
  return {available=false,raw=scalar(raw)}
 end
 assert(type(b)=='number' and a>=0 and b>=0 and a+b<=1.000001,
  'native probability shape changed');
 return {available=true,raw=scalar(raw),success=a+b}
end
local function sourceProbe()
 local u=unit(x.arrivedID); assert(atTarget(u),'native spy is outside target city');
 local n=nativeUnit(u); assert(n.operation==-1,'native spy already assigned');
 local c,plot=target(x.targetOwner,x.targetCity);
 local d=Players[0]:GetDiplomacy();
 assert(type(d.GetSourceTurnsRemaining)=='function','native source turn getter missing');
 local remaining=d:GetSourceTurnsRemaining(c);
 assert(integer(remaining,0),'native source remaining unknown');
 local gain=assert(GameInfo.UnitOperations['UNITOPERATION_SPY_GAIN_SOURCES'],
  'native gain sources operation missing');
 local neutral=assert(GameInfo.UnitOperations['UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR'],
  'native comparison operation missing');
 local can,result=UnitManager.CanStartOperation(u,gain.Hash,plot,false,true);
 local neutralCan,neutralResult=UnitManager.CanStartOperation(u,neutral.Hash,plot,false,true);
 local duration=UnitManager.GetTimeToComplete(gain.Index,u);
 assert(integer(duration,1),'native gain sources duration unknown');
 local bonus=GameInfo.GlobalParameters and
  GameInfo.GlobalParameters['ESPIONAGE_BONUS_GAIN_SOURCES'];
 return {turn=Game.GetCurrentGameTurn(),spy=n,target={owner=x.targetOwner,
  city=x.targetCity,x=c:GetX(),y=c:GetY(),plot=plot:GetIndex()},
  sourceTurns=remaining,canStart=can,canResult=scalar(result),
  duration=duration,gainProbability=probability(gain,u,plot,false),
  comparisonOperation={index=neutral.Index,hash=neutral.Hash},
  comparisonCanStart=neutralCan,comparisonResult=scalar(neutralResult),
  comparisonProbability=probability(neutral,u,plot,true),
  bonusParameter=bonus and scalar(bonus.Value) or {nativeType='nil'},
  gainOperation={index=gain.Index,hash=gain.Hash}}
end
function P.ReadySources()
 role(); local _,p=stillOwned();
 assert(p and p.stage=='TRAVEL_RECEIPT' and not x.unknown and
  not x.ready and not x.sourcesSent,
  'saved native arrival missing');
 local travel=P.Observe(); assert(travel.stage=='ARRIVED' and
  travel.unit.id==p.arrival.unit.id,'native arrival changed');
 local q=sourceProbe();
 assert(q.sourceTurns==0 and q.canStart==true and
  q.spy.id==x.arrivedID and q.target.plot==x.targetPlot,
  'gain sources is not ready with zero prior boost');
 local nextMission=Players[0]:GetDiplomacy():GetMission(0,3);
 assert(nextMission==nil or nextMission==0,
  'native mission ID 3 used before sources intent');
 x.ready=scalar(q); x.freshSourcesPrepared=true;
 publish('SOURCES_READY',q);
 return scalar(q)
end
function P.SourcesOnce()
 role(); local _,p=stillOwned();
 assert(not x.unknown and x.sourcesArmed and x.freshSourcesPrepared and
  not x.sourcesSent and p.stage=='SOURCES_INTENT',
  'sources intent is not saved or was sent already');
 local q=sourceProbe();
 local nextMission=Players[0]:GetDiplomacy():GetMission(0,3);
 assert(q.canStart==true and q.sourceTurns==0 and
  q.spy.id==x.ready.spy.id and q.spy.operation==-1 and
  q.target.plot==x.ready.target.plot and
  q.gainOperation.hash==x.ready.gainOperation.hash and
  (nextMission==nil or nextMission==0),
  'native sources preparation changed');
 local u=unit(x.arrivedID);
 UI.DeselectAllUnits(); UI.DeselectAllCities(); UI.SelectUnit(u);
 assert(UI.GetHeadSelectedUnit() and UI.GetHeadSelectedUnit():GetID()==x.arrivedID and
  UI.GetHeadSelectedUnit():GetOwner()==0,
  'official UI selection differs');
 UI.SetInterfaceMode(InterfaceModeTypes.SPY_CHOOSE_MISSION);
 assert(UI.GetInterfaceMode()==InterfaceModeTypes.SPY_CHOOSE_MISSION and
  UI.GetHeadSelectedUnit() and UI.GetHeadSelectedUnit():GetID()==x.arrivedID and
  UI.GetHeadSelectedUnit():GetOwner()==0,
  'official mission mode lost spy');
 x.sourcesSent=true; x.sourcesSentTurn=Game.GetCurrentGameTurn();
 UnitManager.RequestOperation(u,q.gainOperation.hash);
 return 'SOURCES_SENT'
end
-- Poll for matching native completion. False may mean pending or unreadable;
-- only RouteEvent decides whether a notification may reach the old handler.
function P.OwnedAuxiliaryEvent(attacker,missionID)
 if attacker~=0 or missionID~=3 then return false end
 local p=Game:GetProperty(KEY);
 if not p or (p.stage~='SOURCES_INTENT' and
  p.stage~='SOURCES_RECEIPT') then return false end
 if not x then P.Resume() end
 if p.nonce~=x.nonce then return false end
 if p.stage=='SOURCES_RECEIPT' and not x.ownedEvent then return false end
 exchange();
 local n=readSourceMission();
 if type(n)~='table' then return false end
 local op=x.ready.gainOperation;
 if (n.Operation~=op.index and n.Operation~=op.hash) or
  n.PlotIndex~=x.targetPlot or not integer(n.CompletionTurn,x.ready.turn) or
  n.CompletionTurn~=Game.GetCurrentGameTurn() then return false end
 local c,plot=target(x.targetOwner,x.targetCity);
 if c:GetOwner()~=1 or plot:GetIndex()~=n.PlotIndex then return false end
 if x.resumed and not x.sourcesSent then
  x.sourcesSent=true; x.sourcesNative='HISTORY';
  x.sourcesSentTurn=x.ready.turn
 end
 if not x.sourcesSent then return false end
 if x.ownedEvent then
  return n.Operation==x.ownedEvent.operation and
   n.PlotIndex==x.ownedEvent.plot and
   n.CompletionTurn==x.ownedEvent.turn and
   n.InitialResult==x.ownedEvent.initial and
   n.EscapeResult==x.ownedEvent.escape
 end
 x.ownedEvent={attacker=attacker,id=missionID,operation=n.Operation,
  plot=n.PlotIndex,turn=n.CompletionTurn,initial=n.InitialResult,
  escape=n.EscapeResult,spyID=x.arrivedID,targetOwner=1,targetCity=x.targetCity,
  raw=scalar(n),successEnums={
   undetected=EspionageResultTypes and EspionageResultTypes.SUCCESS_UNDETECTED or nil,
   mustEscape=EspionageResultTypes and EspionageResultTypes.SUCCESS_MUST_ESCAPE or nil,
   failUndetected=EspionageResultTypes and EspionageResultTypes.FAIL_UNDETECTED or nil,
   failEscape=EspionageResultTypes and EspionageResultTypes.FAIL_MUST_ESCAPE or nil,
   killed=EspionageResultTypes and EspionageResultTypes.KILLED or nil,
   captured=EspionageResultTypes and EspionageResultTypes.CAPTURED or nil}};
 publish('SOURCES_EVENT',x.ownedEvent);
 return true
end
function P.SourceResult()
 local e,p=stillOwned();
 assert(x.sourcesSent and x.ownedEvent and p and p.stage=='SOURCES_INTENT',
  'owned native sources event missing');
 lineage(e);
 local n=readSourceMission();
 assert(type(n)=='table' and n.Operation==x.ownedEvent.operation and
  n.PlotIndex==x.ownedEvent.plot and n.CompletionTurn==x.ownedEvent.turn and
  n.InitialResult==x.ownedEvent.initial and n.EscapeResult==x.ownedEvent.escape,
  'native Gain Sources mission changed after event');
 local c,plot=target(x.targetOwner,x.targetCity);
 local after={turn=Game.GetCurrentGameTurn(),target={owner=1,
  city=x.targetCity,x=c:GetX(),y=c:GetY(),plot=plot:GetIndex()}};
 local d=Players[0]:GetDiplomacy();
 local ok,value=pcall(d.GetSourceTurnsRemaining,d,c);
 after.sourceTurnsRead={ok=ok,raw=scalar(value)};
 if ok and integer(value,0) then after.sourceTurns=value end
 local rosterOK,read=pcall(B.ReadEnemy);
 after.rosterRead={ok=rosterOK,raw=scalar(read)};
 if rosterOK and type(read)=='table' and read.spyRoster then
  after.roster=scalar(read.spyRoster)
 end
 local uOK,u=pcall(unit,x.arrivedID);
 after.spyRead={ok=uOK,raw=uOK and 'native-unit' or scalar(u)};
 if uOK then
  after.spy=nativeUnit(u);
  local inCity=atTarget(u);
  after.spyInTarget=inCity==true;
  if inCity then
   local neutral=assert(GameInfo.UnitOperations['UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR']);
   local probabilityOK,prob=pcall(probability,neutral,u,plot,true);
   after.comparisonRead={ok=probabilityOK,raw=scalar(prob)};
   if probabilityOK then after.comparisonProbability=prob end
  end
 end
 local enums=x.ownedEvent.successEnums;
 local success=type(enums.undetected)=='number' and
  type(enums.mustEscape)=='number' and
  (n.InitialResult==enums.undetected or n.InitialResult==enums.mustEscape);
 local failure=(type(enums.failUndetected)=='number' and
   n.InitialResult==enums.failUndetected) or
  (type(enums.failEscape)=='number' and n.InitialResult==enums.failEscape) or
  (type(enums.killed)=='number' and n.InitialResult==enums.killed) or
  (type(enums.captured)=='number' and n.InitialResult==enums.captured);
 local stage='SOURCES_UNKNOWN'; local comparison=nil;
 if success and after.sourceTurns and after.sourceTurns>0 and
  after.comparisonProbability and after.comparisonProbability.available and
  after.spy and after.spyInTarget then
  comparison={before=x.ready.comparisonProbability.success,
   after=after.comparisonProbability.success};
  comparison.delta=comparison.after-comparison.before;
  stage='SOURCES_COMPLETE'
 elseif failure and (not after.sourceTurns or after.sourceTurns==0) then
  stage='SOURCES_FAILED'
 end
 local effectState=after.sourceTurns==nil and 'UNKNOWN' or
  (after.sourceTurns==0 and 'ZERO' or 'POSITIVE');
 local result={stage=stage,event=scalar(x.ownedEvent),before=scalar(x.ready),
  after=after,effectState=effectState,comparison=comparison,
  sourceTurnsDelta=after.sourceTurns and
   after.sourceTurns-x.ready.sourceTurns or nil};
 publish('SOURCES_RESULT',result); return scalar(result)
end
-- All observer failures use one envelope. Ownership is the saved intent, not
-- whether the native observation succeeded. Initial failure has no intent.
function P.CaptureUnknown(step,raw,previous)
 local p=Game:GetProperty(KEY); local e=Game:GetProperty(EXCHANGE);
 local observedExchangeNonce=type(e)=='table' and e.nonce or nil;
 assert(type(step)=='string' and step~='','unknown observation phase missing');
 if p and x then assert(x.nonce==p.nonce,'unknown observation nonce differs') end
 previous=previous or ExposedMembers[WKEY];
 if previous and previous.phase=='UNKNOWN' then
  previous=previous.observation and previous.observation.previousWitness
 end
 failed=true; if x then x.unknown=true end
 local nonce=p and p.nonce or 'initial:'..tostring(observedExchangeNonce)..':'..tostring(Game.GetCurrentGameTurn());
 ExposedMembers[WKEY]=scalar({phase='UNKNOWN',nonce=nonce,
  exchangeNonce=p and p.exchangeNonce or observedExchangeNonce,intentSaved=p~=nil,
  observation={step=step,raw=scalar(raw),previousWitness=scalar(previous),
   nativeRead=scalar(nativeRead),turn=Game.GetCurrentGameTurn(),
   observer={localPlayer=Game.GetLocalPlayer(),requestedAttacker=0,requestedTargetOwner=1},
   initialProvenance=not p and {exchange=scalar(e),
    archive=scalar(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE')),
    state=scalar(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE')),
    ack=scalar(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK')),
    event=scalar(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'))} or nil}});
 return 'UNKNOWN_RECORDED_UI'
end
function P.ObserveStep(step,observe)
 assert(type(observe)=='function','observation callback missing');
 nativeRead=nil;
 local ok,result=pcall(observe);
 if not ok then return P.CaptureUnknown(step,{error=tostring(result)}) end
 return result
end
function P.InvalidateOnLoad()
 x=nil;
 ExposedMembers[WKEY]=nil;
 ExposedMembers.JAC_ReturnedPreparationCoreFailure=nil;
 nativeRead=nil; failed=false;
 return 'UI_STATE_INVALIDATED'
end
-- Routing ownership is separate from native outcome verification. An
-- expected event with an unreadable outcome belongs to this diagnostic's
-- unknown record, never to the old neutralization receiver.
function P.RouteEvent(attacker,missionID)
 local p=Game:GetProperty(KEY);
 if attacker~=0 or missionID~=3 or not p or not p.sources or
   not p.sources.spy or p.sources.spy.owner~=0 or not p.sources.target or
   p.sources.target.owner~=1 or
   (p.stage~='SOURCES_INTENT' and p.stage~='SOURCES_RECEIPT') then return 'UNOWNED' end
 nativeRead=nil;
 local ok,owned=pcall(function()
  readSourceMission();
  return P.OwnedAuxiliaryEvent(attacker,missionID)
 end);
 if ok and owned then return 'OWNED' end
 -- A real notification for this saved ID belongs here even if its getter
 -- returns nil/0/a scalar or its history fails validation. Polling may still
 -- return false while the operation is running; notification routing may not.
 P.CaptureUnknown('EVENT_GATE',{attacker=attacker,missionID=missionID,
  error=ok and 'owned Sources notification lacks matching native history' or tostring(owned)});
 return 'OWNED_UNKNOWN'
end
function P.Evidence()
 return scalar({ui=x,core=Game:GetProperty(KEY),initialUnknown=Game:GetProperty(FKEY),witness=ExposedMembers[WKEY]})
end
return 'RETURNED_PREPARATION_UI_READY'
"#;

/// Insert as the FIRST statements of View's existing SpyMissionCompleted
/// callback, before the neutralization retry/mission ownership assertions.
/// UI_SETUP must already have installed JAC_ReturnedPreparation548 in that
/// View instance. Only a known unrelated event falls through to the old handler.
pub const UI_EARLY_EVENT_GATE: &str = r#"
local preparation=JAC_ReturnedPreparation548;
if preparation and preparation.RouteEvent(attacker,missionID)~='UNOWNED' then return end
"#;

/// Call on LoadScreenClose after UI_SETUP, or let UI_EARLY_EVENT_GATE call
/// Resume through OwnedAuxiliaryEvent when the first post-load event arrives.
pub const UI_RESUME_IF_SAVED: &str = r#"
local p=Game:GetProperty('JAC_BOUNDARY_RETURNED_PREPARATION');
local preparation=assert(JAC_ReturnedPreparation548,
 'returned preparation UI module missing');
if (p or Game:GetProperty('JAC_BOUNDARY_RETURNED_PREPARATION_UNKNOWN')) and
 not preparation.HasState() then return preparation.Resume() end
return p and 'ALREADY_OBSERVING' or 'NO_SAVED_PREPARATION'
"#;

/// Save one independently observed active off-map trip for later ID adoption.
pub const TRAVEL_CHECKPOINT_CORE: &str = r#"
local KEY='JAC_BOUNDARY_RETURNED_PREPARATION';
local p=assert(Game:GetProperty(KEY),'saved travel intent missing');
local e=assert(Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62'));
local w=assert(ExposedMembers.JAC_ReturnedPreparation548Witness,
 'UI transit witness missing');
local q=assert(w.observation); local o=assert(q.offMap);
assert(p.stage=='TRAVEL_INTENT' and not p.transit and
 e.stage=='RETURNED' and e.nonce==p.exchangeNonce and
 w.phase=='TRAVEL_OBSERVATION' and w.nonce==p.nonce and
 w.targetOwner==1 and w.targetCity==p.travel.target.city and
 w.targetPlot==p.travel.target.plot and q.stage=='IN_TRANSIT' and
 q.roster.physical==3 and #q.roster.captured==2 and
 #q.roster.map==0 and #q.roster.activeOffMap==1 and
 q.roster.activeOffMap[1].returnTurn==o.returnTurn and
 q.roster.activeOffMap[1].x==o.x and
 q.roster.activeOffMap[1].y==o.y and
 type(q.turn)=='number' and type(o.returnTurn)=='number' and
 o.returnTurn>=q.turn and
 ((o.x==p.travel.target.x and o.y==p.travel.target.y) or
  (o.x==-9999 and o.y==-9999)),
 'native off-map travel checkpoint differs');
local archive=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'));
local ack=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK'));
for i=1,3 do
 local h=i<=2 and archive.histories[i] or ack.observed.mission;
 local old=p.oldHistories[i];
 assert(h and h.id==old.id and h.turn==old.turn and
  h.operation==old.operation and h.plot==old.plot and
  h.initial==old.initial and h.escape==old.escape,
  'old mission history changed during transit')
end
local next={}; for k,v in pairs(p) do next[k]=v end
next.transit={turn=q.turn,returnTurn=o.returnTurn,x=o.x,y=o.y,
 name=o.name,level=o.level};
Game:SetProperty(KEY,next); return 'TRAVEL_CHECKPOINT'
"#;

/// Host calls only after a caught UI/native/Core read or send exception.
/// Unknown raw data remains under this diagnostic key; it never authorizes a
/// retry or converts a missing receipt into success.
pub const UNKNOWN_CORE: &str = r#"
local KEY='JAC_BOUNDARY_RETURNED_PREPARATION';
local FKEY='JAC_BOUNDARY_RETURNED_PREPARATION_UNKNOWN';
local p=Game:GetProperty(KEY);
local w=assert(ExposedMembers.JAC_ReturnedPreparation548Witness,
 'UI unknown witness missing');
assert(w.phase=='UNKNOWN' and type(w.nonce)=='string' and w.observation and
 type(w.observation.step)=='string' and
 w.observation.turn==Game.GetCurrentGameTurn(),
 'unknown observer envelope missing');
if not p then
 assert(w.intentSaved==false and w.observation.initialProvenance and
  w.observation.observer,'initial failure cannot create an operation intent');
 local previous=Game:GetProperty(FKEY);
 assert(not previous or previous.nonce==w.nonce,'initial failure nonce changed');
 Game:SetProperty(FKEY,{stage='INITIAL_UNKNOWN',observerOutcome='UNKNOWN',
  nonce=w.nonce,exchangeNonce=w.exchangeNonce,observation=w.observation});
 return 'INITIAL_UNKNOWN_CORE'
end
assert(w.intentSaved==true and w.nonce==p.nonce and
 w.exchangeNonce==p.exchangeNonce,'unknown observation belongs to another intent');
local previous=p.unknowns or {};
assert(#previous<16,'finite unknown observation bound exceeded');
local next={}; for k,v in pairs(p) do next[k]=v end
next.observerOutcome='UNKNOWN';
next.unknowns={}; for i,v in ipairs(previous) do next.unknowns[i]=v end
next.unknowns[#next.unknowns+1]=w.observation;
Game:SetProperty(KEY,next); return 'UNKNOWN_RECORDED_CORE'
"#;

pub const TRAVEL_INTENT_CORE: &str = r#"
local KEY='JAC_BOUNDARY_RETURNED_PREPARATION';
assert(not Game:GetProperty('JAC_BOUNDARY_RETURNED_PREPARATION_UNKNOWN'),
 'initial observation failed; stale witness cannot create travel intent');
local e=assert(Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62'),'exchange missing');
local w=assert(ExposedMembers.JAC_ReturnedPreparation548Witness,'UI travel witness missing');
local r=assert(e.returnEvidence,'native return missing');
assert(not Game:GetProperty(KEY) and e.stage=='RETURNED' and e.turn==62 and
 r.stage=='RETURNED_MAP' and r.returned and r.after and
 r.after.physical==3 and #r.after.captured==2 and #r.after.map==1 and
 #r.after.activeOffMap==0 and r.returned.id==r.after.map[1].id and
 w.phase=='TRAVEL_READY' and w.exchangeNonce==e.nonce and
 w.returnedID==r.returned.id and type(w.nonce)=='string',
 'formal returned spy or owned travel witness missing');
local q=assert(w.observation); local t=assert(q.target); local b=assert(q.before);
assert(q.canStart==true and q.unusedMission3==true and
 t.owner==1 and t.city==w.targetCity and t.plot==w.targetPlot and
 b.stage=='RETURNED' and b.unit and b.unit.id==r.returned.id and
 b.unit.owner==0 and b.unit.unitType==r.returned.unitType and
 b.unit.x==r.returned.x and b.unit.y==r.returned.y and
 b.unit.operation==-1 and b.roster.physical==3 and
 #b.roster.captured==2 and #b.roster.map==1 and #b.roster.activeOffMap==0,
 'native travel identity/destination differs');
local city=assert(CityManager.GetCity(1,t.city),'target city missing');
local plot=assert(Map.GetPlot(t.x,t.y),'target plot missing');
assert(city:GetOwner()==1 and city:GetID()==t.city and
 city:GetX()==t.x and city:GetY()==t.y and plot:GetIndex()==t.plot,
 'native target city changed');
local archive=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'));
local state=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_STATE'));
local ack=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK'));
local event=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'));
assert(state.missionPost and
 (state.missionPost.histories=='UI' or state.missionPost.histories=='CORE') and
 event.attacker==0 and event.id==2 and event.turn==62 and
 ack.observed and ack.observed.preserved,'old mission lineage missing');
local d=Players[0]:GetDiplomacy(); local availability={mission=type(d.GetMission),
 sourceTurns=type(d.GetSourceTurnsRemaining)};
for i=1,3 do
 local h=i<=2 and archive.histories[i] or ack.observed.mission;
 local old=e.before.histories[i]; local after=r.after.histories[i];
 assert(h and old and after and h.id==i-1 and h.turn==({23,45,62})[i] and
  h.initial==1 and h.escape==-1 and
  h.operation==old.operation and h.operation==after.operation and
  h.plot==old.plot and h.plot==after.plot and
  h.initial==old.initial and h.initial==after.initial and
  h.escape==old.escape and h.escape==after.escape and
  (i==3 or (h.operation==ack.observed.preserved[i].operation and
    h.turn==ack.observed.preserved[i].turn)),
  'old failure history changed');
 if type(d.GetMission)=='function' then
  local n=assert(d:GetMission(0,i-1),'Core old mission missing');
  assert(n.Operation==h.operation and n.PlotIndex==h.plot and
   n.CompletionTurn==h.turn and n.InitialResult==h.initial and
   n.EscapeResult==h.escape,'Core old mission changed')
 end
end
if type(d.GetMission)=='function' then
 local nextMission=d:GetMission(0,3);
 assert(nextMission==nil or nextMission==0,'native mission ID 3 already used')
end
local units=Players[0]:GetUnits(); availability.findID=units and type(units.FindID) or 'nil';
if units and type(units.FindID)=='function' then
 local u=assert(units:FindID(b.unit.id),'Core returned spy missing');
 assert(u:GetID()==b.unit.id and u:GetOwner()==b.unit.owner and
  u:GetType()==b.unit.unitType,'Core returned spy identity differs');
 availability.getX=type(u.GetX); availability.getY=type(u.GetY);
 if type(u.GetX)=='function' and type(u.GetY)=='function' then
  assert(u:GetX()==b.unit.x and u:GetY()==b.unit.y,
   'Core returned spy coordinates differ')
 end
 availability.operation=type(u.GetSpyOperation);
 availability.endTurn=type(u.GetSpyOperationEndTurn);
 availability.experience=type(u.GetExperience);
 if type(u.GetExperience)=='function' then
  local experience=u:GetExperience();
  availability.level=experience and type(experience.GetLevel) or 'nil'
 end
 if type(u.GetSpyOperation)=='function' then
  assert(u:GetSpyOperation()==b.unit.operation,'Core returned operation differs')
 end
 if type(u.GetSpyOperationEndTurn)=='function' then
  assert(u:GetSpyOperationEndTurn()==b.unit.endTurn,'Core returned end turn differs')
 end
end
local function historyRow(h)
 return {id=h.id,turn=h.turn,operation=h.operation,plot=h.plot,
  initial=h.initial,escape=h.escape}
end
Game:SetProperty(KEY,{stage='TRAVEL_INTENT',nonce=w.nonce,
 exchangeNonce=e.nonce,returnedID=r.returned.id,returnEvidence={
  stage=r.stage,after=r.after,returned=r.returned},
 oldHistories={historyRow(archive.histories[1]),historyRow(archive.histories[2]),
  historyRow(ack.observed.mission)},
 travel=q,coreAvailability=availability});
return 'TRAVEL_INTENT'
"#;

pub const TRAVEL_RECEIPT_CORE: &str = r#"
local KEY='JAC_BOUNDARY_RETURNED_PREPARATION';
local p=assert(Game:GetProperty(KEY),'travel intent missing');
local e=assert(Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62'));
local w=assert(ExposedMembers.JAC_ReturnedPreparation548Witness,'arrival witness missing');
local a=assert(w.observation); local n=assert(a.unit);
assert(p.stage=='TRAVEL_INTENT' and e.stage=='RETURNED' and
 e.nonce==p.exchangeNonce and w.nonce==p.nonce and
 w.phase=='TRAVEL_OBSERVATION' and a.stage=='ARRIVED' and
 w.targetOwner==1 and w.targetCity==p.travel.target.city and
 w.targetPlot==p.travel.target.plot and a.roster.physical==3 and
 #a.roster.captured==2 and #a.roster.map==1 and #a.roster.activeOffMap==0 and
 a.roster.map[1].id==n.id and n.owner==0 and
 n.unitType==p.travel.before.unit.unitType and
 n.x==p.travel.target.x and n.y==p.travel.target.y and n.operation==-1,
 'native arrival does not match saved travel');
if n.id~=p.returnedID then
 assert(a.adopted==true and a.transit and
  ((a.transit.x==p.travel.target.x and a.transit.y==p.travel.target.y) or
   (a.transit.x==-9999 and a.transit.y==-9999)) and
  type(a.transit.returnTurn)=='number' and a.turn>=a.transit.returnTurn,
  'changed native ID lacks observed off-map travel')
end
if p.transit then
 assert(a.transit and a.transit.returnTurn==p.transit.returnTurn and
  a.transit.x==p.transit.x and a.transit.y==p.transit.y and
  p.transit.turn<=a.turn,'saved transit checkpoint changed before arrival')
end
local city=assert(CityManager.GetCity(1,w.targetCity));
local plot=assert(Map.GetPlot(n.x,n.y));
assert(city:GetOwner()==1 and city:GetID()==w.targetCity and
 city:GetX()==n.x and city:GetY()==n.y and plot:GetIndex()==w.targetPlot,
 'native arrival city/plot changed');
local old=assert(p.oldHistories); local archive=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'));
local ack=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK'));
local d=Players[0]:GetDiplomacy();
for i=1,3 do
 local h=i<=2 and archive.histories[i] or ack.observed.mission;
 assert(h and h.id==old[i].id and h.turn==old[i].turn and
  h.operation==old[i].operation and h.plot==old[i].plot and
  h.initial==old[i].initial and h.escape==old[i].escape,
  'old mission history changed during travel');
 if type(d.GetMission)=='function' then
  local m=assert(d:GetMission(0,i-1));
  assert(m.Operation==h.operation and m.PlotIndex==h.plot and
   m.CompletionTurn==h.turn and m.InitialResult==h.initial and
   m.EscapeResult==h.escape,'Core native history changed during travel')
 end
end
local units=Players[0]:GetUnits(); local source={findID=units and type(units.FindID) or 'nil',
 mission=type(d.GetMission)};
if units and type(units.FindID)=='function' then
 local u=assert(units:FindID(n.id),'Core arrived spy missing');
 assert(u:GetID()==n.id and u:GetOwner()==n.owner and
  u:GetType()==n.unitType,'Core arrived spy ID differs');
 source.getX=type(u.GetX); source.getY=type(u.GetY);
 if type(u.GetX)=='function' and type(u.GetY)=='function' then
  assert(u:GetX()==n.x and u:GetY()==n.y,'Core arrived coordinates differ')
 end
 source.operation=type(u.GetSpyOperation); source.endTurn=type(u.GetSpyOperationEndTurn);
 if type(u.GetSpyOperation)=='function' then
  assert(u:GetSpyOperation()==n.operation,'Core arrived operation differs')
 end
 if type(u.GetSpyOperationEndTurn)=='function' then
  assert(u:GetSpyOperationEndTurn()==n.endTurn,'Core arrived end turn differs')
 end
end
local next={}; for k,v in pairs(p) do next[k]=v end
next.stage='TRAVEL_RECEIPT'; next.arrival=a; next.arrivalSources=source;
Game:SetProperty(KEY,next); return 'TRAVEL_RECEIPT'
"#;

pub const SOURCES_INTENT_CORE: &str = r#"
local KEY='JAC_BOUNDARY_RETURNED_PREPARATION';
local p=assert(Game:GetProperty(KEY),'arrival receipt missing');
local e=assert(Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62'));
local w=assert(ExposedMembers.JAC_ReturnedPreparation548Witness,'sources witness missing');
local q=assert(w.observation); local t=assert(q.target); local u=assert(q.spy);
assert(p.stage=='TRAVEL_RECEIPT' and e.stage=='RETURNED' and
 e.nonce==p.exchangeNonce and w.nonce==p.nonce and
 w.phase=='SOURCES_READY' and w.returnedID==p.returnedID and
 q.canStart==true and q.sourceTurns==0 and
 u.id==p.arrival.unit.id and u.owner==0 and
 u.unitType==p.arrival.unit.unitType and u.operation==-1 and
 t.owner==1 and t.city==p.travel.target.city and
 t.plot==p.travel.target.plot and u.x==t.x and u.y==t.y and
 type(q.duration)=='number' and q.duration>0 and
 q.gainOperation and q.comparisonProbability and
 q.comparisonProbability.success>=0 and q.comparisonProbability.success<=1,
 'official sources readiness differs from arrived spy');
local city=assert(CityManager.GetCity(1,t.city));
local plot=assert(Map.GetPlot(t.x,t.y));
assert(city:GetOwner()==1 and city:GetID()==t.city and
 city:GetX()==t.x and city:GetY()==t.y and plot:GetIndex()==t.plot,
 'native sources target city changed');
local gain=assert(GameInfo.UnitOperations['UNITOPERATION_SPY_GAIN_SOURCES']);
local neutral=assert(GameInfo.UnitOperations['UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR']);
assert(q.gainOperation.index==gain.Index and q.gainOperation.hash==gain.Hash and
 q.comparisonOperation.index==neutral.Index and
 q.comparisonOperation.hash==neutral.Hash,
 'native source/comparison operation differs');
local archive=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'));
local ack=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK'));
local d=Players[0]:GetDiplomacy();
for i=1,3 do
 local h=i<=2 and archive.histories[i] or ack.observed.mission;
 local old=p.oldHistories[i];
 assert(h and h.id==old.id and h.turn==old.turn and
  h.operation==old.operation and h.plot==old.plot and
  h.initial==old.initial and h.escape==old.escape,
  'old failure history changed before sources');
 if type(d.GetMission)=='function' then
  local n=assert(d:GetMission(0,i-1));
  assert(n.Operation==h.operation and n.PlotIndex==h.plot and
   n.CompletionTurn==h.turn and n.InitialResult==h.initial and
   n.EscapeResult==h.escape,'Core old mission changed before sources')
 end
end
local source={mission=type(d.GetMission),sourceTurns=type(d.GetSourceTurnsRemaining)};
local units=Players[0]:GetUnits(); source.findID=units and type(units.FindID) or 'nil';
if units and type(units.FindID)=='function' then
 local core=assert(units:FindID(u.id),'Core sources spy missing');
 assert(core:GetID()==u.id and core:GetOwner()==0 and core:GetType()==u.unitType,
  'Core sources spy identity differs');
 source.getX=type(core.GetX); source.getY=type(core.GetY);
 if type(core.GetX)=='function' and type(core.GetY)=='function' then
  assert(core:GetX()==u.x and core:GetY()==u.y,'Core sources spy position differs')
 end
 source.operation=type(core.GetSpyOperation);
 source.endTurn=type(core.GetSpyOperationEndTurn);
 if type(core.GetSpyOperation)=='function' then
  assert(core:GetSpyOperation()==u.operation,'Core sources operation differs')
 end
 if type(core.GetSpyOperationEndTurn)=='function' then
  assert(core:GetSpyOperationEndTurn()==u.endTurn,'Core sources end turn differs')
 end
end
if type(d.GetMission)=='function' then
 local nextMission=d:GetMission(0,3);
 assert(nextMission==nil or nextMission==0,'native mission ID 3 used before sources')
end
if type(d.GetSourceTurnsRemaining)=='function' then
 local ok,value=pcall(d.GetSourceTurnsRemaining,d,city);
 source.sourceTurnsCall={ok=ok,value=ok and value or tostring(value)};
 if ok then assert(value==q.sourceTurns,'Core source turns differ') end
end
local next={}; for k,v in pairs(p) do next[k]=v end
next.stage='SOURCES_INTENT'; next.sources=q; next.sourcesCore=source;
Game:SetProperty(KEY,next); return 'SOURCES_INTENT'
"#;

pub const SOURCES_RECEIPT_CORE: &str = r#"
local KEY='JAC_BOUNDARY_RETURNED_PREPARATION';
local p=assert(Game:GetProperty(KEY),'sources intent missing');
local e=assert(Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62'));
local w=assert(ExposedMembers.JAC_ReturnedPreparation548Witness,'sources result missing');
local q=assert(w.observation); local event=assert(q.event); local after=assert(q.after);
assert(p.stage=='SOURCES_INTENT' and e.stage=='RETURNED' and
 e.nonce==p.exchangeNonce and w.nonce==p.nonce and
 w.phase=='SOURCES_RESULT' and
 (q.stage=='SOURCES_COMPLETE' or q.stage=='SOURCES_FAILED' or
  q.stage=='SOURCES_UNKNOWN') and
 event.attacker==0 and event.id==3 and event.spyID==p.sources.spy.id and
 event.targetOwner==1 and event.targetCity==p.sources.target.city and
 event.plot==p.sources.target.plot and
 (event.operation==p.sources.gainOperation.index or
  event.operation==p.sources.gainOperation.hash) and
 type(event.turn)=='number' and event.turn>=p.sources.turn and
 after.target.plot==event.plot and after.target.owner==1 and
 after.target.city==event.targetCity and
 p.sources.sourceTurns==0 and
 q.before and q.before.spy and q.before.spy.id==p.sources.spy.id and
 q.before.target and q.before.target.plot==p.sources.target.plot and
 event.successEnums and event.raw and
 event.raw.Operation==event.operation and
 event.raw.PlotIndex==event.plot and
 event.raw.CompletionTurn==event.turn and
 event.raw.InitialResult==event.initial and
 event.raw.EscapeResult==event.escape and
 q.effectState==(after.sourceTurns==nil and 'UNKNOWN' or
  (after.sourceTurns==0 and 'ZERO' or 'POSITIVE')),
 'native sources receipt ownership differs');
local city=assert(CityManager.GetCity(1,event.targetCity));
local plot=assert(Map.GetPlot(city:GetX(),city:GetY()));
assert(city:GetOwner()==1 and city:GetID()==event.targetCity and
 plot:GetIndex()==event.plot,'native sources target changed');
local enums=event.successEnums;
local success=type(enums.undetected)=='number' and
 type(enums.mustEscape)=='number' and
 (event.initial==enums.undetected or event.initial==enums.mustEscape);
local failed=(type(enums.failUndetected)=='number' and
 event.initial==enums.failUndetected) or
 (type(enums.failEscape)=='number' and event.initial==enums.failEscape) or
 (type(enums.killed)=='number' and event.initial==enums.killed) or
 (type(enums.captured)=='number' and event.initial==enums.captured);
if q.stage=='SOURCES_COMPLETE' then
 assert(success and type(after.sourceTurns)=='number' and
  after.sourceTurns>0 and after.spy and after.spy.id==event.spyID and
  after.spy.owner==0 and after.spy.unitType==p.sources.spy.unitType and
  after.spy.x==city:GetX() and after.spy.y==city:GetY() and
  after.spyInTarget==true and after.comparisonProbability and
  after.comparisonProbability.available==true and
  q.comparison and
  q.comparison.before==p.sources.comparisonProbability.success and
  q.comparison.after==after.comparisonProbability.success and
  q.comparison.delta==q.comparison.after-q.comparison.before and
  q.sourceTurnsDelta==after.sourceTurns,
  'native sources success lacks effect/comparison evidence')
elseif q.stage=='SOURCES_FAILED' then
 assert(failed and (after.sourceTurns==nil or after.sourceTurns==0) and
  q.comparison==nil,'native sources failure outcome differs')
else
 assert(q.comparison==nil,'unknown sources outcome claims comparison success')
end
local archive=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_SPY_ARCHIVE'));
local ack=assert(Game:GetProperty('JAC_BOUNDARY_THIRD_MISSION_ACK'));
local d=Players[0]:GetDiplomacy();
for i=1,3 do
 local h=i<=2 and archive.histories[i] or ack.observed.mission;
 local old=p.oldHistories[i];
 assert(h and h.id==old.id and h.turn==old.turn and
  h.operation==old.operation and h.plot==old.plot and
  h.initial==old.initial and h.escape==old.escape,
  'old failure history changed after sources');
 if type(d.GetMission)=='function' then
  local n=assert(d:GetMission(0,i-1));
  assert(n.Operation==h.operation and n.PlotIndex==h.plot and
   n.CompletionTurn==h.turn and n.InitialResult==h.initial and
   n.EscapeResult==h.escape,'Core old native history changed after sources')
 end
end
local source={mission=type(d.GetMission),sourceTurns=type(d.GetSourceTurnsRemaining)};
if type(d.GetMission)=='function' then
 local n=assert(d:GetMission(0,event.id),'Core native sources mission absent');
 assert(n.Operation==event.operation and n.PlotIndex==event.plot and
  n.CompletionTurn==event.turn and n.InitialResult==event.initial and
  n.EscapeResult==event.escape,'Core native sources mission differs')
end
if type(d.GetSourceTurnsRemaining)=='function' then
 local ok,value=pcall(d.GetSourceTurnsRemaining,d,city);
 source.sourceTurnsCall={ok=ok,value=ok and value or tostring(value)};
 if ok and after.sourceTurns~=nil then
  assert(value==after.sourceTurns,'Core source turns differ')
 end
end
local units=Players[0]:GetUnits(); source.findID=units and type(units.FindID) or 'nil';
if after.spy and units and type(units.FindID)=='function' then
 local u=assert(units:FindID(event.spyID),'Core sources spy absent');
 assert(u:GetID()==event.spyID and u:GetOwner()==0 and
  u:GetType()==after.spy.unitType,'Core sources spy ID differs');
 source.getX=type(u.GetX); source.getY=type(u.GetY);
 if type(u.GetX)=='function' and type(u.GetY)=='function' then
  assert(u:GetX()==after.spy.x and u:GetY()==after.spy.y,
   'Core sources spy coordinates differ')
 end
 source.operation=type(u.GetSpyOperation);
 source.endTurn=type(u.GetSpyOperationEndTurn);
 if type(u.GetSpyOperation)=='function' then
  assert(u:GetSpyOperation()==after.spy.operation,'Core sources operation differs')
 end
 if type(u.GetSpyOperationEndTurn)=='function' then
  assert(u:GetSpyOperationEndTurn()==after.spy.endTurn,'Core sources end turn differs')
 end
end
local next={}; for k,v in pairs(p) do next[k]=v end
next.stage='SOURCES_RECEIPT'; next.outcome=q.stage;
next.sourcesReceipt=q; next.receiptCore=source;
Game:SetProperty(KEY,next); return 'SOURCES_RECEIPT'
"#;
