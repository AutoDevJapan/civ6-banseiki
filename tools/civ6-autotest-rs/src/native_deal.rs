//! One-shot original-work Core deal, using authenticated shared inventory.
pub const GAMEPLAY: &str = r#"
local A=assert(JAC_AbsorptionGameplay)
local R=assert(JAC_AbsorptionRules)
local NI=assert(JAC_NativeInventory)
local ND={};JAC_NativeDeal=ND
local function save(state) Game:SetProperty(R.STATE_KEY,R.Copy(state)) end
local function sameSlot(a,b)
  return a.Owner==b.Owner and a.CityID==b.CityID and a.X==b.X and a.Y==b.Y
    and a.Building==b.Building and a.Slot==b.Slot
end
local function identity(w)
  return {Index=w.Index,GreatWorkType=w.GreatWorkType,ObjectType=w.ObjectType,Name=w.Name,
    CreatorName=w.CreatorName,TurnCreated=w.TurnCreated,CreatedByPlayer=w.CreatedByPlayer,Effects=w.Effects}
end
local function slotRecord(w)
  return {Owner=w.Owner,City=w.CityID,CityID=w.CityID,X=w.X,Y=w.Y,Building=w.Building,Slot=w.Slot}
end
function ND.Original(source,tracked,donor,destination,custodian,currentID)
  if not (source.Owner==donor and source.CreatedByPlayer==donor
    and (tracked and tracked.NativeOwnerMoves or 0)==0 and destination>=0
    and custodian==destination) then return false end
  -- Preserve known global history even when a physical work comes back to its
  -- creator without bringing the destination roster record back with it.
  for id,old in pairs(A.GetState().Transactions or {}) do if id~=currentID then
    for _,w in ipairs(old.NativeWorks or {}) do
      if w.Source.Index==source.Index and (w.DealSubmitted or w.PhysicalMoved
        or old.SourceRosterMoved and w.Destination~=old.From) then return false end
    end
  end end
  return true
end
function ND.Eligible(tx,snap)
  return not snap.Carrier and ND.Original(snap.Source,(tx.SourceRoster or {})[snap.ID],
    tx.From,snap.Destination,snap.Custodian,tx.ID)
end
function ND.Preflight(tx,snap)
  assert(ND.Eligible(tx,snap),'MP Core deal requires original ownership and zero recorded owner moves')
  assert(DealManager and DealManager.GetWorkingDeal and DealManager.GetPossibleDealItems
    and DealManager.EnactWorkingDeal,'native Core deal API unavailable')
  if snap.DealSubmitted then assert(snap.MPDeal and snap.MPDeal.Issued,'legacy submitted deal lacks original-work journal') end
end
local function emptySlots(world,owner,objectType)
  local valid={};for row in GameInfo.GreatWork_ValidSubTypes() do
    if row.GreatWorkObjectType==objectType then valid[row.GreatWorkSlotType]=true end
  end
  local result={}
  for _,row in ipairs(world.Cities) do if row.Owner==owner then
    local city=assert(CityManager.GetCityAt(row.X,row.Y),'deal recipient city missing')
    assert(city:GetOwner()==row.Owner and city:GetID()==row.ID,'deal recipient identity changed')
    for _,building in ipairs(R.Keys(row.Buildings)) do
      if row.Pillaged[building]==false then
        for slot=0,row.Reads.SLOTS[building]-1 do
          local kind=row.Reads.SLOT_TYPES and row.Reads.SLOT_TYPES[building] and row.Reads.SLOT_TYPES[building][slot]
          if valid[kind] and row.Reads.WORK[building][slot]==-1 then
            result[#result+1]={Owner=owner,CityID=row.ID,City=row.ID,X=row.X,Y=row.Y,Building=building,Slot=slot,SlotType=kind}
          end
        end
      end
    end
  end end
  return result
end
local function workMap(world)
  local map={};for _,w in ipairs(world.Works) do assert(not map[w.Index],'deal duplicate physical index');map[w.Index]=w end
  return map
end
local function sourceEmpty(source)
  local city=assert(CityManager.GetCityAt(source.X,source.Y),'deal source city missing')
  assert(city:GetOwner()==source.Owner and city:GetID()==source.CityID
    and city:GetBuildings():HasBuilding(source.Building),'deal source identity changed')
  return NI.ReadCity(city,'WORK',source.Building,source.Slot)==-1
end
local function reconcile(state,tx,snap)
  local d=assert(snap.MPDeal);assert(d.Version==1 and d.Issued and d.From==tx.From and d.To==snap.Custodian
    and Equal(d.Source,snap.Source),'deal saved intent changed')
  local now=NI.Snapshot();local previous=workMap(d.Before);local current=workMap(now)
  assert(#now.Works==#d.Before.Works,'deal physical work cardinality changed')
  for index,w in pairs(previous) do
    local actual=assert(current[index],'deal another original work missing')
    assert(Equal(identity(w),identity(actual)),'deal work provenance or effects changed')
    if index~=d.Source.Index then assert(sameSlot(w,actual),'deal another work displaced or replaced') end
  end
  local moved=assert(current[d.Source.Index])
  if sameSlot(moved,d.Source) then
    d.WaitingFor='ISSUED_UNCONFIRMED';save(state);return false -- never enact again, including a false/nil reply
  end
  assert(moved.Owner==d.To,'deal original moved to an unexpected owner')
  local final
  for _,candidate in ipairs(d.EmptySlots) do if sameSlot(moved,candidate) then final=candidate end end
  assert(final and sourceEmpty(d.Source),'deal target was not a compatible pre-issue empty slot or source is occupied')
  snap.TargetSlot=slotRecord(moved);snap.TargetSlot.SlotType=final.SlotType
  snap.PhysicalMoved=true;d.FinalSlot=R.Copy(snap.TargetSlot);d.Complete=true;d.WaitingFor=nil
  d.After=now;d.AfterEvidenceKind=NI.EvidenceKind();d.VerifiedTurn=Game.GetCurrentGameTurn()
  save(state);return true
end
function ND.Advance(state,tx,snap)
  ND.Preflight(tx,snap)
  if snap.MPDeal then
    assert(Equal(snap.MPDeal.Source,snap.Source) and snap.MPDeal.From==tx.From
      and snap.MPDeal.To==snap.Custodian,'deal saved source or recipient changed')
    if snap.PhysicalMoved then assert(snap.MPDeal.Complete);return true end
  else
    NI.With(tx,'DEAL_BEFORE:'..snap.Source.Index,function()
      local before=NI.Snapshot();local works=workMap(before);local source=assert(works[snap.Source.Index],'deal source absent')
      assert(sameSlot(source,snap.Source) and Equal(identity(source),identity(snap.Source)),'deal source snapshot changed')
      local city=assert(CityManager.GetCityAt(source.X,source.Y))
      assert(city:GetOwner()==tx.From and city:GetID()==source.CityID,'deal source Core owner or ID changed')
      local slots=emptySlots(before,snap.Custodian,source.ObjectType)
      assert(#slots>0,'deal has no compatible empty native slot')
      local definition=assert(GameInfo.GreatWorks[source.GreatWorkType])
      A.BuildNativeWorkDealCore(tx.From,snap.Custodian,source.Index,definition.Index,tx.NativeWorks)
      snap.MPDeal={Version=1,From=tx.From,To=snap.Custodian,Source=R.Copy(snap.Source),
        Before=before,EmptySlots=slots,BeforeEvidenceKind=NI.EvidenceKind(),Issued=true,
        IssuedTurn=Game.GetCurrentGameTurn(),WaitingFor='ISSUED_UNCONFIRMED'}
      snap.DealSubmitted=true;save(state) -- durable intent immediately before the only native call
      local ok,value=pcall(DealManager.EnactWorkingDeal,tx.From,snap.Custodian)
      snap.MPDeal.CallReturned=ok;snap.MPDeal.CallResult=ok and tostring(value) or nil
      snap.MPDeal.CallError=not ok and tostring(value) or nil
      save(state) -- return values and exceptions are diagnostic, never physical proof
    end)
  end
  return NI.With(tx,'DEAL_AFTER:'..snap.Source.Index,function() return reconcile(state,tx,snap) end)
end
function ND.HasContinuation(tx)
  for _,snap in ipairs(tx.NativeWorks or {}) do if snap.MPDeal and snap.MPDeal.Issued then return true end end
  return false
end
function ND.ContinueOnly(state,tx)
  for _,snap in ipairs(tx.NativeWorks or {}) do
    if snap.MPDeal and snap.MPDeal.Issued and not snap.PhysicalMoved then ND.Advance(state,tx,snap) end
  end
end
"#;
