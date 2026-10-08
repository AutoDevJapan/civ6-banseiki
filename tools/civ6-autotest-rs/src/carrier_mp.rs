//! Generated paired-city MP binding. Only an active human absorber issues MOVE;
//! every Gameplay VM consumes the same scalar, framed publication packet.
pub const CODEC: &str = r#"
local function Encode(v)
  local t=type(v)
  if t=='boolean' then return v and 'b1' or 'b0' end
  if t=='string' or t=='number' then
    local s=v
    if t=='number' then
      assert(v==v and v~=math.huge and v~=-math.huge,'nonfinite observation number')
      -- Lua 5.1 tostring uses too few digits for an IEEE-754 double. Keep
      -- 17 significant digits and preserve the sign of a zero value.
      if v==0 then s=1/v<0 and '-0' or '0' else s=string.format('%.17g',v) end
    end
    return (t=='string' and 's' or 'n')..#s..':'..s
  end
  assert(t=='table','unsupported observation value')
  local keys=JAC_AbsorptionRules.Keys(v);local parts={'t'..#keys..':'}
  for _,k in ipairs(keys) do parts[#parts+1]=Encode(k);parts[#parts+1]=Encode(v[k]) end
  return table.concat(parts)
end
local function Decode(s)
  assert(type(s)=='string' and #s<=1048576,'observation size')
  local at,budget=1,8192
  local function read(depth)
    budget=budget-1;assert(budget>=0 and depth<=24,'observation complexity')
    local tag=s:sub(at,at);at=at+1
    if tag=='b' then local v=s:sub(at,at);at=at+1;assert(v=='0' or v=='1');return v=='1' end
    assert(tag=='t' or tag=='s' or tag=='n','observation tag')
    local colon=assert(s:find(':',at,true));local digits=s:sub(at,colon-1)
    assert(digits:match('^%d+$'),'observation length')
    local n=assert(tonumber(digits));assert(n<=1048576);at=colon+1
    if tag=='t' then
      local v={};assert(n<=4096)
      for i=1,n do local k=read(depth+1)
        assert(type(k)=='string' or type(k)=='number','observation key')
        assert(v[k]==nil,'duplicate observation key');v[k]=read(depth+1)
      end
      return v
    end
    assert(at+n-1<=#s,'truncated observation');local v=s:sub(at,at+n-1);at=at+n
    if tag=='n' then v=assert(tonumber(v));assert(v==v and v~=math.huge and v~=-math.huge) end
    return v
  end
  local v=read(0);assert(at==#s+1,'trailing observation');return v
end
local function Equal(a,b)
  if a==nil or b==nil then return a==b end
  return Encode(a)==Encode(b)
end
"#;

pub const GAMEPLAY: &str = r#"
local MP={Encode=Encode,Decode=Decode}
JAC_CarrierMP=MP
local A=assert(JAC_AbsorptionGameplay)
local R=JAC_AbsorptionRules
local function save(state) Game:SetProperty(R.STATE_KEY,R.Copy(state)) end
local function city(l) return assert(CityManager.GetCityAt(l.X,l.Y),'MP loan city missing') end
local function active(id)
  local p=Players[id]
  return p and p:IsAlive() and p:IsHuman() and p.IsTurnActive and p:IsTurnActive()
end
local function phase(state,c,p)
  c.MP.Phase=p;c.MP.Nonce=c.MP.Nonce+1;save(state)
end
local function key(l) return Encode({l.Owner,l.City or l.ID,l.X,l.Y,l.Building,l.Slot}) end
local function eligible(l,actor)
  local current=CityManager.GetCityAt(l.X,l.Y)
  if not current or current:GetOwner()~=l.Owner or current:GetID()~=(l.City or l.ID)
    or not current:GetBuildings():HasBuilding(l.Building) then return false end
  if l.Owner~=actor then
    if Players[l.Owner]:GetCities():GetCount()<=1 then return false end
    local capital=Players[l.Owner]:GetCities():GetCapitalCity()
    if capital and capital:GetX()==l.X and capital:GetY()==l.Y then return false end
  end
  return not A.NativeCarrierFriction(current)
    and #A.ReadNativeCarrierCore(current,l).UnitsOnCenter==0
end
function MP.Preflight(tx)
  -- Scan every item before the first city loan, not only the next item.
  for _,snap in ipairs(tx.NativeWorks or {}) do
    if snap.Custodian then
      assert(snap.Destination>=0,'MP scattering/recovery requires a separate reservation protocol')
      assert(snap.NativeRoute=='CARRIER','MP legacy deal needs reconciliation')
      assert(snap.Source.BuildingType~='BUILDING_PALACE','MP Palace source needs native preservation proof')
      assert(snap.Source.ObjectType~='GREATWORKOBJECT_ARTIFACT','MP artifact cannot move to an empty museum')
      if not snap.Carrier then
        local s=snap.Source
        assert(eligible({Owner=s.Owner,ID=s.CityID,X=s.X,Y=s.Y,Building=s.Building,Slot=s.Slot},tx.To),
          'MP source city cannot be safely borrowed')
      end
    end
  end
end
local function restoreProperties(current,l)
  -- CoreBefore is captured by Gameplay, never copied from the UI packet.
  local before=assert(l.CoreBefore,'MP loan lacks trusted Core properties')
  local catalog=A.NativeCarrierPropertyKeys()
  for k in pairs(before.PropertyKeys) do
    assert(catalog[k]==true,'MP saved property no longer registered')
    local v=before.Properties[k];local value=current:GetProperty(k)
    assert(value==nil or Equal(value,v),'MP city property replaced during loan')
    if value==nil and v~=nil then current:SetProperty(k,R.Copy(v)) end
  end
end
local function returnLoans(state,c)
  local m=c.MP
  for _,l in ipairs(m.Loans) do
    local current=city(l)
    if l.Owner~=m.Actor and current:GetOwner()==m.Actor then
      assert(l.Issued,'MP cannot return an unissued loan')
      assert(l.BorrowedID==nil or current:GetID()==l.BorrowedID,'MP borrowed return identity changed')
      l.BorrowedID=current:GetID()
      l.ReturnIssued=true;save(state);CityManager.TransferCity(current,l.Owner)
    end
    current=city(l)
    assert(current:GetOwner()==l.Owner,'MP return owner changed')
    assert(l.Owner==m.Actor or l.ReturnIssued or current:GetID()==l.ID,'MP original return identity changed')
    if l.FinalID then assert(current:GetID()==l.FinalID,'MP returned city identity changed') end
    l.FinalID=current:GetID();restoreProperties(current,l);save(state)
  end
end
local function attemptReturn(state,c)
  local ok,err=pcall(returnLoans,state,c)
  if not ok then c.MP.ReturnError=tostring(err);save(state);return false end
  c.MP.ReturnError=nil
  phase(state,c,c.MP.Phase=='ABORT_RETURN' and 'ABORT_RESTORED' or 'RESTORED')
  return false
end
local function abort(state,c,reason)
  local m=c.MP
  assert(not m.Claimed or m.DispatchStatus=='REJECTED','MP unknown MOVE must be preserved')
  m.AbortReason=reason;m.MoveCanceled=true
  phase(state,c,'ABORT_RETURN')
  return attemptReturn(state,c)
end
function MP.HasContinuation(tx)
  for _,snap in ipairs(tx.NativeWorks or {}) do
    local c=snap.Carrier;local m=c and c.MP
    if m and m.Version==2 and m.Transaction==tx.ID and m.Actor==tx.To
      and c.Token==tx.ID..':carrier:'..snap.Source.Index and c.Source.Index==snap.Source.Index then return true end
  end
  return false
end
function MP.ContinueOnly(state,tx)
  if not active(tx.To) then tx.WaitingFor='MP_ACTOR_TURN';save(state);return end
  for _,snap in ipairs(tx.NativeWorks or {}) do
    local c=snap.Carrier;local m=c and c.MP
    if m and m.Version==2 and m.Transaction==tx.ID then MP.Advance(state,c,tx.To) end
  end
end
function MP.AdvanceSnap(state,tx,snap)
  MP.Preflight(tx)
  local player=Players[tx.To]
  assert(player and player:IsAlive() and player:IsHuman(),'MP absorber must be a living human')
  if not snap.Carrier then
    snap.Carrier={Route='TO_OWNER',Phase='BORROW',BorrowIssued=false,Index=snap.Source.Index,
      Source=R.Copy(snap.Source),Owner=snap.Custodian,Borrower=tx.To,
      Token=tx.ID..':carrier:'..snap.Source.Index}
    snap.Carrier.MP={Version=2,Transaction=tx.ID,Actor=tx.To,Phase='SELECT',Nonce=1,RejectedCandidates={}}
    save(state);return false
  end
  if not active(tx.To) then tx.WaitingFor='MP_ACTOR_TURN';save(state);return false end
  if not MP.Advance(state,snap.Carrier,tx.To) then return false end
  snap.TargetSlot=R.Copy(snap.Carrier.FinalSlot);snap.PhysicalMoved=true
  snap.NativeMoveEvidenceKind='SYNCHRONIZED_ACTOR_OBSERVATION'
  save(state);return true
end
function MP.Advance(state,c,preferred)
  assert(GameConfiguration.IsAnyMultiplayer(),'MP required')
  local m=assert(c.MP,'MP reservation required')
  assert(m.Version==2 and m.Actor==c.Borrower,'MP legacy saved loan requires trusted reconciliation')
  if not active(m.Actor) then m.WaitingFor='MP_ACTOR_TURN';save(state);return false end
  m.WaitingFor=nil
  if m.Phase=='DONE' and m.ObservationTurn~=Game.GetCurrentGameTurn() then
    phase(state,c,'RECHECK');return false
  end
  if m.Phase=='ABORT_RETURN' or m.Phase=='RETURN' then return attemptReturn(state,c) end
  if m.Phase=='ABORT_DONE' then
    m.SourceCurrentID=city(m.Loans[1]):GetID()
    if m.FailedLoan==2 or m.DispatchStatus=='REJECTED' then
      m.RejectedCandidates[key(m.Loans[2])]=m.AbortReason
    end
    m.Loans=nil;m.Claimed=nil;m.DispatchStatus=nil;m.DispatchSession=nil;m.MoveCanceled=nil
    c.BorrowIssued=false;c.Phase='BORROW';phase(state,c,'SELECT');return false
  end
  if m.Phase=='BORROW' then
    -- Validate every remaining city before mutating any of them. Count loans
    -- together so borrowing two cities from one owner cannot remove its last.
    local ok,err=pcall(function()
      local counts={}
      for i,l in ipairs(m.Loans) do
        m.FailedLoan=i
        local current=city(l)
        local core=A.ReadNativeCarrierCore(current,l)
        assert(core.HasBuilding and core.Name==l.CoreBefore.Name
          and core.Population==l.CoreBefore.Population and #core.UnitsOnCenter==0,'MP Core city changed before borrowing')
        if current:GetOwner()==l.Owner and l.Owner~=m.Actor then
          assert(eligible(l,m.Actor),'MP loan eligibility changed')
          counts[l.Owner]=(counts[l.Owner] or 0)+1
        else
          assert(current:GetOwner()==m.Actor and (l.Owner==m.Actor or l.Issued),'MP unissued or unrelated loan')
          assert(l.BorrowedID==nil or l.BorrowedID==current:GetID(),'MP saved borrowed identity changed')
        end
      end
      for owner,n in pairs(counts) do assert(Players[owner]:GetCities():GetCount()>n,'MP loans would remove owner last city') end
      for i,l in ipairs(m.Loans) do
        m.FailedLoan=i
        local current=city(l)
        if current:GetOwner()==l.Owner and l.Owner~=m.Actor then
          assert(eligible(l,m.Actor),'MP loan changed between paired transfers')
          l.Issued=true;save(state);CityManager.TransferCity(current,m.Actor)
        end
        assert(city(l):GetOwner()==m.Actor,'MP borrow did not reach selected actor')
        l.BorrowedID=city(l):GetID();save(state);restoreProperties(city(l),l)
      end
    end)
    if not ok then return abort(state,c,'borrow failed: '..tostring(err)) end
    m.FailedLoan=nil
    phase(state,c,'BORROWED');return false
  end
  return m.Phase=='DONE'
end
local function validateObservation(c,o,placement)
  local m=c.MP
  assert(type(o)=='table' and type(o.Cities)=='table' and type(o.Locations)=='table','MP observation shape')
  assert(#o.Cities==2 and #o.Locations==1,'MP actor reports original missing or duplicated')
  local work=o.Locations[1]
  for _,k in ipairs({'Index','GreatWorkType','ObjectType','CreatorName','TurnCreated','CreatedByPlayer','Effects'}) do
    assert(c.Source[k]~=nil and Equal(work[k],c.Source[k]),'MP reported provenance changed: '..k)
  end
  for i,l in ipairs(m.Loans) do
    local current=city(l);local observed=o.Cities[i];local core=A.ReadNativeCarrierCore(current,l)
    assert(observed.Owner==current:GetOwner() and observed.ID==current:GetID()
      and observed.X==l.X and observed.Y==l.Y,'MP observation city identity')
    assert(type(observed.State)=='table' and type(observed.Properties)=='table','MP city snapshot shape')
    assert(Equal(observed.PropertyKeys,core.PropertyKeys) and Equal(observed.Properties,core.Properties),
      'MP reported property differs from registered Core snapshot')
    assert(core.HasBuilding and type(observed.Slots)=='number' and l.Slot>=0 and l.Slot<observed.Slots,
      'MP physical slot unavailable')
    assert(observed.State.Name==core.Name and observed.State.Population==core.Population
      and Equal(observed.State.UnitsOnCenter,core.UnitsOnCenter) and #core.UnitsOnCenter==0,
      'MP observation Core city changed')
    assert(Equal(observed.State.Faith,current:GetProperty(R.CITY_FAITH_KEY))
      and Equal(observed.State.Governance,current:GetProperty(R.GOVERNANCE_KEY)),'MP reported Core governance changed')
    assert(type(observed.State.Assets)=='table' and type(observed.State.Religion)=='table'
      and type(observed.State.PurchasedPlots)=='table' and type(observed.State.Loyalty)=='number','MP incomplete actor city observation')
    -- Work slots, religion, loyalty, plots and global locations may be UI-only.
    -- Compare synchronized actor publications; do not read each peer UI cache.
    if l.ActorBefore and m.Phase~='RECHECK' then
      assert(Equal(observed.State,l.ActorBefore.State),'MP actor reports borrowed city changed')
    end
    if m.Phase=='RESTORED' or m.Phase=='ABORT_RESTORED' then
      assert(Equal(core.PropertyKeys,l.CoreBefore.PropertyKeys) and Equal(core.Properties,l.CoreBefore.Properties),
        'MP trusted properties not restored')
    end
  end
  local compatible=false
  for row in GameInfo.GreatWork_ValidSubTypes() do
    if row.GreatWorkObjectType==c.Source.ObjectType and row.GreatWorkSlotType==o.Cities[2].SlotType then compatible=true end
  end
  assert(compatible,'MP destination slot incompatible')
  local i=placement=='TARGET' and 2 or 1;local l=m.Loans[i];local seen=o.Cities[i]
  assert(work.Owner==seen.Owner and work.CityID==seen.ID and work.X==l.X and work.Y==l.Y
    and work.Building==l.Building and work.Slot==l.Slot,'MP reported work slot changed')
  assert(o.Cities[i].Work==c.Index and o.Cities[3-i].Work==-1,'MP reported source/target mismatch')
end
local function reserved(state,c,l)
  for _,tx in pairs(state.Transactions) do for _,snap in ipairs(tx.NativeWorks or {}) do
    local other=snap.Carrier;local m=other and other.MP
    if m and other.Token~=c.Token and m.Phase~='DONE' and m.Phase~='ABORT_DONE' then
      for _,busy in ipairs(m.Loans or {other.Source}) do
        if busy.X==l.X and busy.Y==l.Y then return true end
      end
    end
  end end
  return false
end
local function receive(actor,p)
  assert(GameConfiguration.IsAnyMultiplayer() and type(p)=='table' and p.Version==2,'MP wire contract')
  local state=A.GetState();local found={};local selectedSnap
  for _,tx in pairs(state.Transactions) do for _,snap in ipairs(tx.NativeWorks or {}) do
    if snap.Carrier and snap.Carrier.Token==p.Token then found[#found+1]=snap.Carrier;selectedSnap=snap end
  end end
  assert(#found==1,'MP carrier token missing or duplicated');local c=found[1];local m=assert(c.MP)
  assert(m.Version==2 and actor==m.Actor and p.Actor==actor,'MP unselected actor')
  if not active(actor) then return false end
  assert(p.Turn==Game.GetCurrentGameTurn(),'MP stale turn')
  assert(type(p.Session)=='string' and #p.Session<=128,'MP session')
  local id=Encode({Command=p.Command,Actor=actor,Token=p.Token,Phase=p.Phase,Nonce=p.Nonce,
    Turn=p.Turn,Payload=p.Payload,Session=p.Session,DispatchStatus=p.DispatchStatus or ''})
  if m.Receipts and m.Receipts[id] then return true end
  assert(p.Phase==m.Phase and p.Nonce==m.Nonce,'MP stale phase/nonce')
  if p.Command=='RESUME' then
    assert(m.Phase=='BORROW' or m.Phase=='RETURN' or m.Phase=='ABORT_RETURN' or m.Phase=='ABORT_DONE','MP resume phase')
    MP.Advance(state,c);return true
  end
  if m.Phase=='SELECT' then
    assert(p.Command=='OBSERVE','MP select command')
    local candidates=assert(Decode(p.Payload).Candidates);assert(type(candidates)=='table' and #candidates<=4096,'MP candidates')
    local source=c.Source
    local origin={Owner=source.Owner,ID=m.SourceCurrentID or source.CityID,X=source.X,Y=source.Y,Building=source.Building,Slot=source.Slot}
    assert(eligible(origin,actor),'MP source eligibility changed')
    local chosen
    table.sort(candidates,function(a,b) return key(a)<key(b) end)
    for _,target in ipairs(candidates) do
      if target.Owner==c.Owner and type(target.Building)=='number' and type(target.Slot)=='number'
        and target.Slot>=0 and target.Slot==math.floor(target.Slot)
        and (target.X~=source.X or target.Y~=source.Y) and not m.RejectedCandidates[key(target)]
        and eligible(target,actor) and not reserved(state,c,target) then chosen=target;break end
    end
    if not chosen then m.WaitingFor='TARGET_SLOT';save(state);return false end
    assert(not reserved(state,c,origin),'MP source city already reserved')
    c.X,c.Y,c.Building,c.Slot,c.OriginalCity=chosen.X,chosen.Y,chosen.Building,chosen.Slot,chosen.City
    selectedSnap.TargetSlot=R.Copy(chosen)
    m.Loans={origin,{Owner=chosen.Owner,ID=chosen.City,X=chosen.X,Y=chosen.Y,Building=chosen.Building,Slot=chosen.Slot}}
    phase(state,c,'PREPARE');return true
  end
  local o=Decode(p.Payload)
  local placement=(m.Phase=='RESTORED' or m.Phase=='RECHECK') and 'TARGET' or 'SOURCE'
  if m.Phase=='DISPATCH' then
    placement=o.Cities and o.Cities[2] and o.Cities[2].Work==c.Index and 'TARGET' or 'SOURCE'
  end
  validateObservation(c,o,placement)
  m.Receipts=m.Receipts or {};m.Receipts[id]=true
  if m.Phase=='PREPARE' then
    assert(p.Command=='OBSERVE','MP prepare command')
    for i,l in ipairs(m.Loans) do
      l.CoreBefore=R.Copy(A.ReadNativeCarrierCore(city(l),l))
      l.ActorBefore={State=R.Copy(o.Cities[i].State)}
    end
    c.Before=R.Copy(o.Cities[2].State);c.BorrowIssued=true
    phase(state,c,'BORROW');MP.Advance(state,c)
  elseif m.Phase=='BORROWED' then
    assert(p.Command=='OBSERVE','MP borrowed command')
    c.BorrowedCity=m.Loans[2].BorrowedID
    local function slot(l) return {Owner=m.Actor,City=l.BorrowedID,X=l.X,Y=l.Y,Building=l.Building,Slot=l.Slot} end
    c.MoveSource=slot(m.Loans[1]);c.MoveTarget=slot(m.Loans[2])
    c.Phase='MOVE';c.MoveToken=c.Token..':mp:'..m.Nonce
    m.GrantSession=p.Session;phase(state,c,'READY')
  elseif m.Phase=='READY' then
    if p.Command=='ABORT' then abort(state,c,'session reset before dispatch claim');return true end
    assert(p.Command=='CLAIM' and p.Session==m.GrantSession,'MP dispatch claim changed')
    m.Claimed=true;m.ClaimNonce=p.Nonce;m.DispatchSession=p.Session
    phase(state,c,'DISPATCH')
  elseif m.Phase=='DISPATCH' then
    assert(p.Command=='OBSERVE','MP dispatch observation')
    if placement=='TARGET' then
      m.ReportedMovedObservation=R.Copy(o);c.Phase='RETURN';c.ReturnToken=c.Token..':mp:return'
      phase(state,c,'RETURN');MP.Advance(state,c)
    elseif p.Session==m.DispatchSession and p.DispatchStatus=='REJECTED' then
      m.DispatchStatus='REJECTED';abort(state,c,'native MOVE explicitly rejected');return true
    else
      -- An original-slot observation alone does not drain an engine queue.
      -- Session loss / unknown result preserves the loan; never retry MOVE or
      -- return a city until the original is observed moved or rejection is known.
      m.UncertainActorObservation=R.Copy(o);m.WaitingFor='NATIVE_QUEUE_RESOLUTION';save(state)
    end
  elseif m.Phase=='ABORT_RESTORED' then
    assert(p.Command=='OBSERVE','MP abort publication')
    m.RollbackActorObservation=R.Copy(o)
    phase(state,c,'ABORT_DONE')
  else
    assert((m.Phase=='RESTORED' or m.Phase=='RECHECK') and p.Command=='OBSERVE','MP publication phase')
    -- This is an actor-reported, synchronized location; not an independent
    -- Gameplay inventory proof. Core independently checked identity/properties.
    m.FinalActorObservation=R.Copy(o);m.ObservationTurn=Game.GetCurrentGameTurn()
    local l=m.Loans[2];c.FinalSlot={Owner=l.Owner,City=l.FinalID,X=l.X,Y=l.Y,Building=l.Building,Slot=l.Slot}
    c.Phase='PROVED';phase(state,c,'DONE')
  end
  return true
end
GameEvents.JAC_CarrierMP_Operation.Add(function(actor,p)
  local ok,result=pcall(receive,actor,p)
  if not ok then print('[JAC_CARRIER_MP_REJECTED] '..tostring(result)) end
  return ok and result==true
end)
ExposedMembers.JAC_CarrierMP={Encode=Encode,Decode=Decode}
"#;
pub const UI: &str = r#"
-- UI.GetElapsedTime is used by Base/Assets/UI/ToolTips/PlotToolTip.lua:879,886.
local identity={};local session=tostring(identity)..':'..tostring(UI.GetElapsedTime())
local claimed,sent,status={},{},{}
local function request(c,command,payload)
  local m=c.MP
  UI.RequestPlayerOperation(m.Actor,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_CarrierMP_Operation',
    Version=2,Command=command,Actor=m.Actor,Token=c.Token,Phase=m.Phase,Nonce=m.Nonce,
    Turn=Game.GetCurrentGameTurn(),Payload=Encode(payload or {}),Session=session,
    DispatchStatus=status[c.MoveToken] or 'NOT_ATTEMPTED'})
  return true
end
local function Observe(c)
  assert(identity)
  local m=c and c.MP
  if not m or m.Version~=2 or m.Phase=='DONE' or Game.GetLocalPlayer()~=m.Actor then return false end
  local player=Players[m.Actor]
  if not player or not player:IsHuman() or not player:IsTurnActive() then return false end
  if m.Phase=='BORROW' or m.Phase=='RETURN' or m.Phase=='ABORT_RETURN' or m.Phase=='ABORT_DONE' then
    return request(c,'RESUME')
  end
  if m.Phase=='SELECT' then
    local reads=assert(ExposedMembers.JAC_HeroReadAPI)
    return request(c,'OBSERVE',{Candidates=reads.FindCompatibleNativeSlots(c.Owner,c.Source.ObjectType)})
  end
  if m.Phase=='DISPATCH' and m.DispatchSession==session and claimed[c.MoveToken]==m.ClaimNonce
    and not sent[c.MoveToken] then
    sent[c.MoveToken]=true -- even for an unknown send result: never repeat MOVE
    local p={}
    p[PlayerOperations.PARAM_PLAYER_ONE]=m.Actor
    p[PlayerOperations.PARAM_CITY_SRC]=c.MoveSource.City
    p[PlayerOperations.PARAM_CITY_DEST]=c.MoveTarget.City
    p[PlayerOperations.PARAM_BUILDING_SRC]=c.MoveSource.Building
    p[PlayerOperations.PARAM_BUILDING_DEST]=c.MoveTarget.Building
    p[PlayerOperations.PARAM_GREAT_WORK_INDEX]=c.Index
    p[PlayerOperations.PARAM_SLOT]=c.MoveTarget.Slot
    local ok,result=pcall(UI.RequestPlayerOperation,m.Actor,PlayerOperations.MOVE_GREAT_WORK,p)
    status[c.MoveToken]=ok and result==false and 'REJECTED' or ok and result==true and 'ACCEPTED' or 'UNKNOWN'
    return false -- reconcile at a later native publication, not the send call
  end
  local api=assert(ExposedMembers.JAC_Absorption)
  local o={Cities={},Locations=api.ReadNativeLocations(c.Index)}
  for _,l in ipairs(m.Loans or {}) do
    local current=assert(CityManager.GetCityAt(l.X,l.Y))
    o.Cities[#o.Cities+1]=api.ReadNativeCarrier(current,l)
  end
  if m.Phase=='READY' then
    if m.GrantSession~=session then return request(c,'ABORT',o) end
    claimed[c.MoveToken]=m.Nonce
    return request(c,'CLAIM',o)
  end
  if m.Phase=='BORROWED' or m.Phase=='PREPARE' or m.Phase=='DISPATCH' or m.Phase=='RESTORED'
    or m.Phase=='ABORT_RESTORED' or m.Phase=='RECHECK' then return request(c,'OBSERVE',o) end
  return false
end
JAC_ObserveNativeCarrierMP=function(c) local ok,result=pcall(Observe,c);return ok and result==true end
"#;
