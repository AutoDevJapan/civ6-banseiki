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
local function restoreProperties(current,l)
  for k,v in pairs(l.Before.Properties) do
    local value=current:GetProperty(k)
    assert(value==nil or Equal(value,v),'MP city property replaced during loan')
    if value==nil then current:SetProperty(k,R.Copy(v)) end
  end
end
local function phase(state,c,p)
  c.MP.Phase=p;c.MP.Nonce=c.MP.Nonce+1;save(state)
end
function MP.AdvanceSnap(state,tx,snap)
  assert(active(tx.To),'MP absorber must be an active human')
  assert(snap.Destination>=0,'MP scattering/recovery route requires a separate reservation protocol')
  if not snap.Carrier then
    snap.Carrier={Route='TO_OWNER',Phase='BORROW',BorrowIssued=false,Index=snap.Source.Index,
      Source=R.Copy(snap.Source),Owner=snap.Custodian,Borrower=tx.To,
      Token=tx.ID..':carrier:'..snap.Source.Index}
    snap.Carrier.MP={Version=1,Actor=tx.To,Phase='SELECT',Nonce=1}
    save(state);return false
  end
  if not MP.Advance(state,snap.Carrier,tx.To) then return false end
  snap.TargetSlot=R.Copy(snap.Carrier.FinalSlot);snap.PhysicalMoved=true;save(state)
  return true
end
function MP.Advance(state,c,preferred)
  assert(GameConfiguration.IsAnyMultiplayer(),'MP required')
  if not c.MP then
    assert(c.Phase=='BORROW' and not c.BorrowIssued,'legacy MP loan needs reconciliation')
    assert(c.Route=='TO_OWNER','MP route currently requires a human recipient')
    assert(active(preferred),'MP absorber must be an active human')
    local source=c.Source
    assert(source.BuildingType~='BUILDING_PALACE','MP Palace source needs native preservation proof')
    assert(source.ObjectType~='GREATWORKOBJECT_ARTIFACT','MP artifact cannot move to an empty museum')
    local loans={
      {Owner=source.Owner,ID=source.CityID,X=source.X,Y=source.Y,Building=source.Building,Slot=source.Slot},
      {Owner=c.Owner,ID=c.OriginalCity,X=c.X,Y=c.Y,Building=c.Building,Slot=c.Slot}}
    assert(loans[1].X~=loans[2].X or loans[1].Y~=loans[2].Y,'MP requires distinct cities')
    for _,l in ipairs(loans) do
      local current=city(l)
      assert(current:GetOwner()==l.Owner and current:GetID()==l.ID,'MP original city identity')
      if l.Owner~=preferred then
        assert(Players[l.Owner]:GetCities():GetCount()>1,'MP cannot borrow an owner last city')
        local capital=Players[l.Owner]:GetCities():GetCapitalCity()
        assert(not capital or capital:GetX()~=l.X or capital:GetY()~=l.Y,'MP cannot borrow a capital')
      end
      assert(not A.NativeCarrierFriction(current),'MP city has active friction')
      for _,tx in pairs(state.Transactions) do for _,snap in ipairs(tx.NativeWorks or {}) do
        local other=snap.Carrier
        if other and other.Token~=c.Token and other.MP and other.MP.Phase~='DONE' then
          for _,busy in ipairs(other.MP.Loans or {other.Source}) do
            assert(busy.X~=l.X or busy.Y~=l.Y,'MP city already reserved by another carrier')
          end
        end
      end end
    end
    c.Borrower=preferred;c.MP={Version=1,Actor=preferred,Phase='PREPARE',Nonce=1,Loans=loans}
    save(state);return false
  end
  local m=c.MP
  assert(m.Version==1 and m.Actor==c.Borrower,'MP saved actor contract')
  if m.Phase=='DONE' and m.ProofTurn~=Game.GetCurrentGameTurn() then
    phase(state,c,'RESTORED');return false
  end
  if m.Phase=='BORROW' then
    for _,l in ipairs(m.Loans) do
      local current=city(l)
      if current:GetOwner()==l.Owner and l.Owner~=m.Actor then
        assert(current:GetID()==l.ID,'MP original loan city changed')
        l.Issued=true;save(state);CityManager.TransferCity(current,m.Actor)
      else
        assert(current:GetOwner()==m.Actor,'MP city belongs to an unrelated owner')
        assert(l.Owner==m.Actor or l.Issued,'MP unissued loan')
        assert(l.Owner~=m.Actor or current:GetID()==l.ID,'MP actor city identity changed')
        assert(l.BorrowedID==nil or current:GetID()==l.BorrowedID,'MP saved borrowed identity changed')
      end
      l.BorrowedID=city(l):GetID();save(state)
      restoreProperties(city(l),l)
    end
    phase(state,c,'BORROWED');return false
  end
  if m.Phase=='RETURN' then
    for _,l in ipairs(m.Loans) do
      local current=city(l)
      if current:GetOwner()==m.Actor and l.Owner~=m.Actor then
        assert(current:GetID()==l.BorrowedID,'MP return loan city changed')
        l.ReturnIssued=true;save(state);CityManager.TransferCity(current,l.Owner)
      end
      current=city(l)
      assert(current:GetOwner()==l.Owner and (l.Owner==m.Actor or l.ReturnIssued),'MP return owner changed')
      l.FinalID=current:GetID()
      restoreProperties(current,l)
      save(state)
    end
    phase(state,c,'RESTORED');return false
  end
  return m.Phase=='DONE'
end
local function validateObservation(c,o)
  local m=c.MP
  assert(type(o)=='table' and type(o.Cities)=='table' and type(o.Locations)=='table','MP observation shape')
  assert(#o.Cities==2 and #o.Locations==1,'MP original missing or duplicated')
  local work=o.Locations[1]
  for _,k in ipairs({'Index','GreatWorkType','ObjectType','CreatorName','TurnCreated','CreatedByPlayer','Effects'}) do
    assert(c.Source[k]~=nil and Equal(work[k],c.Source[k]),'MP work provenance changed: '..k)
  end
  for i,l in ipairs(m.Loans) do
    local current=city(l);local observed=o.Cities[i]
    assert(observed.Owner==current:GetOwner() and observed.ID==current:GetID()
      and observed.X==l.X and observed.Y==l.Y,'MP observation city identity')
    assert(type(observed.State)=='table' and type(observed.Properties)=='table','MP city snapshot shape')
    assert(current:GetBuildings():HasBuilding(l.Building) and type(observed.Slots)=='number'
      and l.Slot>=0 and l.Slot<observed.Slots,'MP physical slot unavailable')
    assert(observed.State.Name==current:GetName() and observed.State.Population==current:GetPopulation(),
      'MP observation core city changed')
    assert(type(observed.State.Assets)=='table' and type(observed.State.Religion)=='table'
      and type(observed.State.PurchasedPlots)=='table' and type(observed.State.UnitsOnCenter)=='table'
      and type(observed.State.Loyalty)=='number','MP incomplete city snapshot')
    assert(#observed.State.UnitsOnCenter==0,'MP loan center occupied')
    if l.Before then
      assert(Equal(observed.State,l.Before.State),'MP borrowed city changed')
      if m.Phase=='RESTORED' then assert(Equal(observed.Properties,l.Before.Properties),'MP properties not restored') end
    end
  end
  local compatible=false
  for row in GameInfo.GreatWork_ValidSubTypes() do
    if row.GreatWorkObjectType==c.Source.ObjectType and row.GreatWorkSlotType==o.Cities[2].SlotType then compatible=true end
  end
  assert(compatible,'MP destination slot incompatible')
  local moved=m.Phase=='DISPATCH' or m.Phase=='RESTORED'
  local i=moved and 2 or 1;local l=m.Loans[i];local seen=o.Cities[i]
  assert(work.Owner==seen.Owner and work.CityID==seen.ID and work.X==l.X and work.Y==l.Y
    and work.Building==l.Building and work.Slot==l.Slot,'MP physical work slot changed')
  assert(o.Cities[i].Work==c.Index and o.Cities[3-i].Work==-1,'MP physical source/target mismatch')
end
local function receive(actor,p)
  assert(GameConfiguration.IsAnyMultiplayer() and type(p)=='table' and p.Version==1,'MP wire contract')
  local state=A.GetState();local found={};local selectedSnap
  for _,tx in pairs(state.Transactions) do for _,snap in ipairs(tx.NativeWorks or {}) do
    if snap.Carrier and snap.Carrier.Token==p.Token then found[#found+1]=snap.Carrier;selectedSnap=snap end
  end end
  assert(#found==1,'MP carrier token missing or duplicated');local c=found[1];local m=assert(c.MP)
  assert(actor==m.Actor and p.Actor==actor and active(actor),'MP unselected or inactive actor')
  assert(p.Turn==Game.GetCurrentGameTurn(),'MP stale turn')
  assert((p.Command=='OBSERVE' or p.Command=='RESUME') and type(p.Session)=='string' and #p.Session<=128,'MP command/session')
  local id=Encode({Command=p.Command,Actor=actor,Token=p.Token,Phase=p.Phase,Nonce=p.Nonce,Turn=p.Turn,Payload=p.Payload,Session=p.Session})
  if m.Receipts and m.Receipts[id] then return true end
  assert(p.Phase==m.Phase and p.Nonce==m.Nonce,'MP stale phase/nonce')
  if p.Command=='RESUME' then
    assert(m.Phase=='BORROW' or m.Phase=='RETURN','MP resume phase')
    MP.Advance(state,c);return true
  end
  if m.Phase=='SELECT' then
    local target=assert(Decode(p.Payload).Target)
    assert(target.Owner==c.Owner and type(target.Building)=='number' and type(target.Slot)=='number','MP target descriptor')
    local current=assert(CityManager.GetCityAt(target.X,target.Y))
    assert(current:GetOwner()==target.Owner and current:GetID()==target.City
      and current:GetBuildings():HasBuilding(target.Building),'MP selected target changed')
    c.X,c.Y,c.Building,c.Slot,c.OriginalCity=target.X,target.Y,target.Building,target.Slot,target.City
    selectedSnap.TargetSlot=R.Copy(target);c.MP=nil
    MP.Advance(state,c,actor)
    return true
  end
  assert(m.Phase=='PREPARE' or m.Phase=='BORROWED' or m.Phase=='DISPATCH' or m.Phase=='RESTORED','MP publication phase')
  local o=Decode(p.Payload);validateObservation(c,o)
  m.Receipts=m.Receipts or {};m.Receipts[id]=true
  if m.Phase=='PREPARE' then
    for i,l in ipairs(m.Loans) do l.Before=R.Copy(o.Cities[i]) end
    c.Before=R.Copy(o.Cities[2].State);c.BorrowIssued=true
    phase(state,c,'BORROW');MP.Advance(state,c)
  elseif m.Phase=='BORROWED' then
    c.BorrowedCity=m.Loans[2].BorrowedID
    local function slot(l) return {Owner=m.Actor,City=l.BorrowedID,X=l.X,Y=l.Y,Building=l.Building,Slot=l.Slot} end
    c.MoveSource=slot(m.Loans[1]);c.MoveTarget=slot(m.Loans[2])
    c.Phase='MOVE';c.MoveToken=c.Token..':mp:'..m.Nonce
    m.Session=p.Session;m.MoveIssued=true;phase(state,c,'DISPATCH')
  elseif m.Phase=='DISPATCH' then
    m.Moved=R.Copy(o);c.Phase='RETURN';c.ReturnToken=c.Token..':mp:return'
    phase(state,c,'RETURN');MP.Advance(state,c)
  else
    m.Final=R.Copy(o)
    m.ProofTurn=Game.GetCurrentGameTurn()
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
local identity={};local session=tostring(identity)..':'..tostring(UI.GetElapsedTime());local sent={}
local function Observe(c)
  assert(identity)
  local m=c and c.MP
  if not m or m.Phase=='DONE' or Game.GetLocalPlayer()~=m.Actor then return false end
  local player=Players[m.Actor]
  if not player or not player:IsHuman() or not player:IsTurnActive() then return false end
  if m.Phase=='BORROW' or m.Phase=='RETURN' then
    UI.RequestPlayerOperation(m.Actor,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_CarrierMP_Operation',
      Version=1,Command='RESUME',Actor=m.Actor,Token=c.Token,Phase=m.Phase,Nonce=m.Nonce,
      Turn=Game.GetCurrentGameTurn(),Payload='',Session=session})
    return true
  end
  if m.Phase=='SELECT' then
    local reads=assert(ExposedMembers.JAC_HeroReadAPI)
    local slots=reads.FindCompatibleNativeSlots(c.Owner,c.Source.ObjectType)
    for _,slot in ipairs(slots) do
      if slot.X~=c.Source.X or slot.Y~=c.Source.Y then
        UI.RequestPlayerOperation(m.Actor,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_CarrierMP_Operation',
          Version=1,Command='OBSERVE',Actor=m.Actor,Token=c.Token,Phase=m.Phase,Nonce=m.Nonce,
          Turn=Game.GetCurrentGameTurn(),Payload=Encode({Target=slot}),Session=session})
        return true
      end
    end
    return false
  end
  if m.Phase=='DISPATCH' and m.Session==session and not sent[c.MoveToken] then
    sent[c.MoveToken]=true -- also on an unknown send result: never repeat MOVE
    local p={}
    p[PlayerOperations.PARAM_PLAYER_ONE]=m.Actor
    p[PlayerOperations.PARAM_CITY_SRC]=c.MoveSource.City
    p[PlayerOperations.PARAM_CITY_DEST]=c.MoveTarget.City
    p[PlayerOperations.PARAM_BUILDING_SRC]=c.MoveSource.Building
    p[PlayerOperations.PARAM_BUILDING_DEST]=c.MoveTarget.Building
    p[PlayerOperations.PARAM_GREAT_WORK_INDEX]=c.Index
    p[PlayerOperations.PARAM_SLOT]=c.MoveTarget.Slot
    pcall(UI.RequestPlayerOperation,m.Actor,PlayerOperations.MOVE_GREAT_WORK,p)
    return false
  end
  if m.Phase~='PREPARE' and m.Phase~='BORROWED' and m.Phase~='DISPATCH' and m.Phase~='RESTORED' then return false end
  local api=assert(ExposedMembers.JAC_Absorption)
  local o={Cities={},Locations=api.ReadNativeLocations(c.Index)}
  for _,l in ipairs(m.Loans) do
    local current=assert(CityManager.GetCityAt(l.X,l.Y))
    o.Cities[#o.Cities+1]=api.ReadNativeCarrier(current,l)
  end
  -- A failed or unknown MOVE remains pending. Never infer success from a
  -- request return, event receipt, number of publications, or an empty cache.
  if m.Phase=='DISPATCH' and (o.Cities[1].Work~=-1 or o.Cities[2].Work~=c.Index) then return false end
  UI.RequestPlayerOperation(m.Actor,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_CarrierMP_Operation',
    Version=1,Command='OBSERVE',Actor=m.Actor,Token=c.Token,Phase=m.Phase,Nonce=m.Nonce,
    Turn=Game.GetCurrentGameTurn(),Payload=Encode(o),Session=session})
  return true
end
JAC_ObserveNativeCarrierMP=function(c) local ok,result=pcall(Observe,c);return ok and result==true end
"#;
