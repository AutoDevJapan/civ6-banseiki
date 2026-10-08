//! Native inventory observations for MP preparation and CITY inheritance.
//! The generated Core binding validates engine-supplied actor identity and
//! authoritative city descriptors. UI-only inventory is explicitly reported.
pub const GAMEPLAY: &str = r#"
local A=assert(JAC_AbsorptionGameplay)
local R=assert(JAC_AbsorptionRules)
local KEY='JAC_NATIVE_INVENTORY_V1'
local NI={};JAC_NativeInventory=NI
JAC_NativeInventoryCodec={Encode=Encode,Decode=Decode}
local context
local function box() return R.Copy(Game:GetProperty(KEY) or {Version=1,Serial=0,Requests={}}) end
local function save(b) assert(b.Version==1);Game:SetProperty(KEY,R.Copy(b)) end
local function prune(b,turn)
  local changed=false
  local transactions,flows
  for key,q in pairs(b.Requests) do
    if q.Turn<turn then
      local terminal=q.Consumed
      if not terminal and (q.Phase=='FLOW_BEGIN' or q.Phase=='FLOW_CHECK') then
        if not flows then flows=(Game:GetProperty('JAC_ABSORPTION_FLOW_V1') or {}).Flows or {} end
        local flow=flows[q.ID];terminal=not flow or flow.Phase=='DONE' or flow.Phase=='RESULT'
      elseif not terminal and q.Phase:match('^CITY_LOYALTY:') then
        local x,y=q.Phase:match('^CITY_LOYALTY:(%-?%d+):(%-?%d+):')
        local city=x and CityManager.GetCityAt(tonumber(x),tonumber(y))
        local governance=city and city:GetProperty(R.GOVERNANCE_KEY)
        terminal=not governance or governance.ID~=q.ID
          or not (governance.ImmediatePending or governance.FrictionPending)
      elseif not terminal then
        if not transactions then transactions=(Game:GetProperty(R.STATE_KEY) or {}).Transactions or {} end
        local tx=transactions[q.ID]
        terminal=not tx or tx.Status=='COMPLETE'
      end
      if terminal then b.Requests[key]=nil;changed=true end
    end
  end
  return changed
end
local function fingerprint(payload)
  local first,second=0,1
  for at=1,#payload do local byte=payload:byte(at)
    -- Products remain below 2^53, so these integer operations are exact in
    -- Lua 5.1 doubles. Receipts acknowledge completed work, never authorize
    -- another mutation or replace its saved observation.
    first=(first*257+byte)%2147483647;second=(second*263+byte)%2147483629
  end
  return {Bytes=#payload,First=first,Second=second}
end
local function receiptPayload(q)
  local receipt=q.Receipt
  if receipt and not receipt.PartFingerprints and not receipt.Fingerprint then
    -- Older V1 saves retain the accepted observation/chunks with the receipt.
    local parts=receipt.Parts or q.Chunks
    if parts then
      receipt.PartFingerprints={};for i,part in ipairs(parts) do receipt.PartFingerprints[i]=fingerprint(part) end
    elseif receipt.Payload or q.Observation then receipt.Fingerprint=fingerprint(receipt.Payload or Encode(q.Observation)) end
  end
  if receipt then receipt.Parts=nil;receipt.Payload=nil end
  return receipt
end
local function active(id)
  local p=Players[id];return p and p:IsAlive() and p:IsHuman() and p.IsTurnActive and p:IsTurnActive()
end
local function selected()
  local best
  -- Observer identity comes from shared life/human state, never turn activity.
  -- A delivery-time IsTurnActive difference can delay acceptance, not change
  -- the saved actor/nonce or authorize a different participant.
  for _,id in ipairs(PlayerManager.GetAliveIDs()) do
    local p=Players[id]
    if p and p:IsAlive() and p:IsHuman() and (not best or id<best) then best=id end
  end
  return best
end
local function citykey(owner,id) return tostring(owner)..':'..tostring(id) end
local function orderedReads(reads)
  table.sort(reads.RELIGIONS,function(a,b) return Encode(a)<Encode(b) end)
  table.sort(reads.DISTRICTS,function(a,b) return Encode(a)<Encode(b) end)
end
local function effectsFor(info)
  local effects={}
  for r in GameInfo.GreatWork_YieldChanges() do if r.GreatWorkType==info.GreatWorkType then effects[#effects+1]=r.YieldType..' '..r.YieldChange end end
  if info.Tourism and info.Tourism~=0 then effects[#effects+1]='TOURISM '..info.Tourism end
  table.sort(effects);return effects
end
local function dense(list)
  assert(type(list)=='table','inventory array required');local n=#list;local count=0
  for k in pairs(list) do assert(type(k)=='number' and k==math.floor(k) and k>=1 and k<=n,'inventory sparse array');count=count+1 end
  assert(count==n,'inventory sparse array');return n
end
local function descriptors()
  local rows={};local ids={};local seen={}
  for _,id in ipairs(PlayerManager.GetAliveIDs()) do ids[#ids+1]=id;seen[id]=true end
  local free=PlayerManager.GetFreeCitiesPlayerID and PlayerManager.GetFreeCitiesPlayerID()
  if type(free)=='number' and free>=0 and Players[free] and not seen[free] then ids[#ids+1]=free end
  table.sort(ids)
  for _,owner in ipairs(ids) do for _,c in Players[owner]:GetCities():Members() do
    local buildings={};for info in GameInfo.Buildings() do
      if c:GetBuildings():HasBuilding(info.Index) then buildings[info.Index]=true end
    end
    local props={};local keys=A.NativeCarrierPropertyKeys()
    for k in pairs(keys) do props[k]=R.Copy(c:GetProperty(k)) end
    rows[#rows+1]={Owner=owner,ID=c:GetID(),X=c:GetX(),Y=c:GetY(),Name=c:GetName(),Population=c:GetPopulation(),
      Buildings=buildings,PropertyKeys=keys,Properties=props}
  end end
  table.sort(rows,function(a,b) return a.Owner==b.Owner and a.ID<b.ID or a.Owner<b.Owner end)
  return rows
end
local function captureCore(core)
  -- This reader calls only direct native methods. Never consult ExposedMembers
  -- or the legacy helpers whose missing-getter branch reads the local UI cache.
  local result={Cities={},Works={}}
  for _,d in ipairs(core) do
    local c=assert(CityManager.GetCityAt(d.X,d.Y));local b=c:GetBuildings()
    if not b.GetNumGreatWorkSlots or not b.GetGreatWorkInSlot or not b.GetGreatWorkTypeFromIndex
      or not b.GetCreatorNameFromIndex or not b.GetTurnFromIndex or not Game.GetGreatWorkPlayer then return nil end
    local identity=c.GetCulturalIdentity and c:GetCulturalIdentity()
    local religion=c:GetReligion();local districts=c:GetDistricts()
    if not identity or not identity.GetLoyalty or not religion.GetReligionsInCity or not districts.Members then return nil end
    local row=R.Copy(d);row.Reads={LOYALTY=identity:GetLoyalty(),RELIGIONS={},DISTRICTS={},SLOTS={},WORK={}}
    for _,r in pairs(religion:GetReligionsInCity()) do row.Reads.RELIGIONS[#row.Reads.RELIGIONS+1]=R.Copy(r) end
    for _,r in districts:Members() do row.Reads.DISTRICTS[#row.Reads.DISTRICTS+1]={Type=GameInfo.Districts[r:GetType()].DistrictType,X=r:GetX(),Y=r:GetY()} end
    for building in pairs(d.Buildings) do
      local count=b:GetNumGreatWorkSlots(building);row.Reads.SLOTS[building]=count;row.Reads.WORK[building]={}
      for slot=0,count-1 do
        local index=b:GetGreatWorkInSlot(building,slot);row.Reads.WORK[building][slot]=index
        if index>=0 then
          local kind=b:GetGreatWorkTypeFromIndex(index);local info=assert(GameInfo.GreatWorks[kind]);local effects={}
          for r in GameInfo.GreatWork_YieldChanges() do if r.GreatWorkType==info.GreatWorkType then effects[#effects+1]=r.YieldType..' '..r.YieldChange end end
          if info.Tourism and info.Tourism~=0 then effects[#effects+1]='TOURISM '..info.Tourism end;table.sort(effects)
          local buildingType;for def in GameInfo.Buildings() do if def.Index==building then buildingType=def.BuildingType end end
          result.Works[#result.Works+1]={Index=index,Owner=d.Owner,CityID=d.ID,X=d.X,Y=d.Y,Building=building,
            BuildingType=buildingType,Slot=slot,GreatWorkType=info.GreatWorkType,ObjectType=info.GreatWorkObjectType,
            Name=info.Name,CreatorName=b:GetCreatorNameFromIndex(index),TurnCreated=b:GetTurnFromIndex(index),
            CreatedByPlayer=Game.GetGreatWorkPlayer(index),Effects=effects}
        end
      end
    end
    orderedReads(row.Reads);result.Cities[#result.Cities+1]=row
  end
  table.sort(result.Works,function(a,b) return a.Index<b.Index end)
  return result
end
local function validate(o,core)
  assert(type(o)=='table' and dense(o.Cities)==#core,'inventory observation shape');dense(o.Works)
  local cities={};local expected={}
  for i,d in ipairs(core) do
    local row=assert(o.Cities[i]);local fields={'Owner','ID','X','Y','Name','Population','Buildings','PropertyKeys','Properties'}
    for _,field in ipairs(fields) do assert(Equal(row[field],d[field]),'inventory Core descriptor changed: '..field) end
    local reads=assert(row.Reads);assert(type(reads.SLOTS)=='table' and type(reads.WORK)=='table'
      and type(reads.RELIGIONS)=='table' and type(reads.DISTRICTS)=='table'
      and type(reads.LOYALTY)=='number' and reads.LOYALTY>=0 and reads.LOYALTY<=100,'inventory city reads')
    dense(reads.RELIGIONS);dense(reads.DISTRICTS)
    local city=assert(CityManager.GetCityAt(d.X,d.Y))
    local identity=city.GetCulturalIdentity and city:GetCulturalIdentity()
    if identity and identity.GetLoyalty then assert(reads.LOYALTY==identity:GetLoyalty(),'inventory Core loyalty changed') end
    local religion=city:GetReligion()
    if religion.GetReligionsInCity then
      local direct={};for _,r in pairs(religion:GetReligionsInCity()) do direct[#direct+1]=R.Copy(r) end
      table.sort(direct,function(a,b) return Encode(a)<Encode(b) end)
      assert(Equal(reads.RELIGIONS,direct),'inventory Core religion changed')
    end
    local districts=city:GetDistricts()
    if districts.Members then
      local direct={};for _,r in districts:Members() do direct[#direct+1]={Type=GameInfo.Districts[r:GetType()].DistrictType,X=r:GetX(),Y=r:GetY()} end
      table.sort(direct,function(a,b) return Encode(a)<Encode(b) end)
      assert(Equal(reads.DISTRICTS,direct),'inventory Core districts changed')
    end
    for building in pairs(reads.SLOTS) do assert(d.Buildings[building],'inventory extra building') end
    for building in pairs(reads.WORK) do assert(d.Buildings[building],'inventory extra work building') end
    for building in pairs(d.Buildings) do
      local count=reads.SLOTS[building];assert(type(count)=='number' and count>=0 and count<=128 and count==math.floor(count),'inventory slot count')
      local slots=assert(reads.WORK[building])
      for slot in pairs(slots) do assert(type(slot)=='number' and slot==math.floor(slot) and slot>=0 and slot<count,'inventory extra slot') end
      local native=assert(CityManager.GetCityAt(d.X,d.Y)):GetBuildings()
      if native.GetNumGreatWorkSlots then assert(count==native:GetNumGreatWorkSlots(building),'inventory Core slot count changed') end
      for slot=0,count-1 do
        local index=slots[slot];assert(type(index)=='number' and index>=-1 and index==math.floor(index),'inventory slot value')
        if native.GetGreatWorkInSlot then assert(index==native:GetGreatWorkInSlot(building,slot),'inventory Core work changed') end
        if index>=0 then assert(not expected[index],'inventory duplicate index');expected[index]={d=d,Building=building,Slot=slot} end
      end
    end
    cities[citykey(d.Owner,d.ID)]=row
  end
  local found={}
  for _,w in ipairs(o.Works) do
    local e=assert(expected[w.Index],'inventory undeclared work');assert(not found[w.Index],'inventory duplicate work');found[w.Index]=true
    assert(w.Owner==e.d.Owner and w.CityID==e.d.ID and w.X==e.d.X and w.Y==e.d.Y
      and w.Building==e.Building and w.Slot==e.Slot,'inventory work location')
    assert(type(w.GreatWorkType)=='string' and type(w.ObjectType)=='string' and type(w.Effects)=='table'
      and type(w.TurnCreated)=='number' and type(w.CreatedByPlayer)=='number','inventory provenance shape')
    local definition=assert(GameInfo.GreatWorks[w.GreatWorkType],'inventory work definition')
    assert(definition.GreatWorkObjectType==w.ObjectType,'inventory object type')
    local buildingType;for def in GameInfo.Buildings() do if def.Index==w.Building then buildingType=def.BuildingType end end
    assert(w.BuildingType==buildingType and w.Name==definition.Name and Equal(w.Effects,effectsFor(definition)),'inventory work definition data')
    assert(type(w.CreatorName)=='string' and w.TurnCreated>=0 and w.TurnCreated==math.floor(w.TurnCreated)
      and w.CreatedByPlayer>=0 and w.CreatedByPlayer==math.floor(w.CreatedByPlayer),'inventory provenance values')
    local native=assert(CityManager.GetCityAt(w.X,w.Y)):GetBuildings()
    if native.GetGreatWorkTypeFromIndex then
      local actual=assert(GameInfo.GreatWorks[native:GetGreatWorkTypeFromIndex(w.Index)])
      assert(actual.GreatWorkType==w.GreatWorkType,'inventory Core type changed')
    end
    if native.GetCreatorNameFromIndex then assert(w.CreatorName==native:GetCreatorNameFromIndex(w.Index),'inventory Core creator changed') end
    if native.GetTurnFromIndex then assert(w.TurnCreated==native:GetTurnFromIndex(w.Index),'inventory Core creation turn changed') end
    if Game.GetGreatWorkPlayer then assert(w.CreatedByPlayer==Game.GetGreatWorkPlayer(w.Index),'inventory Core original player changed') end
  end
  for index in pairs(expected) do assert(found[index],'inventory work missing') end
  return cities
end
function NI.HasContext() return context~=nil end
function NI.With(plan,phase,fn)
  local id=assert(plan.ID)
  assert(type(id)=='string' and #id>0 and #id<=256 and type(plan.From)=='number'
    and type(plan.To)=='number' and plan.From~=plan.To and Players[plan.From] and Players[plan.To],'inventory intent contract')
  if context and context.Request.ID==id and context.Request.Phase==phase then return fn() end
  local b=box();assert(b.Version==1,'inventory save version')
  if prune(b,Game.GetCurrentGameTurn()) then save(b) end
  local core=descriptors();local signature=Encode(core)
  local key=id..':'..phase;local q=b.Requests[key]
  if q then assert(q.From==plan.From and q.To==plan.To,'inventory intent identity changed') end
  if not q or q.Consumed or q.Turn~=Game.GetCurrentGameTurn() or q.CoreSignature~=signature then
    b.Serial=b.Serial+1
    q={ID=id,From=plan.From,To=plan.To,Phase=phase,Nonce=b.Serial,Turn=Game.GetCurrentGameTurn(),Core=core,CoreSignature=signature,
      Plan=phase=='FLOW_BEGIN' and R.Copy(plan) or {ID=id,From=plan.From,To=plan.To}}
    local direct=captureCore(core)
    if direct then q.Observation=direct;q.EvidenceKind='DIRECT_CORE';validate(direct,core)
    else q.Actor=selected() end
    b.Requests[key]=q;save(b)
  end
  if not q.Observation then
    local actor=selected()
    if actor~=q.Actor then
      b.Serial=b.Serial+1;q.Actor=actor;q.Nonce=b.Serial;q.Chunks=nil;q.ChunkCount=nil;save(b)
    end
    error('JAC_ABSORPTION_WAITING_UI',0)
  end
  local current=captureCore(core)
  if current then q.Observation=current;q.EvidenceKind='DIRECT_CORE';validate(current,core) end
  local previous=context;context={Request=q,Cities=validate(q.Observation,core)}
  local values={pcall(fn)};context=previous
  if not values[1] then error(values[2],0) end
  -- A later logical read gets another publication, even if only UI-only work
  -- positions changed and the Core descriptor signature remained identical.
  b=box();local finished=assert(b.Requests[key]);finished.Consumed=true
  receiptPayload(finished)
  finished.Core=nil;finished.CoreSignature=nil;finished.Observation=nil;finished.Plan=nil
  finished.Chunks=nil;finished.ChunkCount=nil;save(b)
  return unpack(values,2)
end
function NI.Works(owner)
  assert(context,'shared inventory context required');local list={}
  for _,w in ipairs(context.Request.Observation.Works) do if w.Owner==owner then list[#list+1]=R.Copy(w) end end
  table.sort(list,function(a,b) return a.Index<b.Index end);return list
end
function NI.ReadCity(city,method,building,slot)
  assert(context,'shared inventory context required')
  local row=assert(context.Cities[citykey(city:GetOwner(),city:GetID())],'shared inventory city missing')
  if method=='SLOTS' then return assert(row.Reads.SLOTS[building]) end
  if method=='WORK' then return assert(row.Reads.WORK[building][slot]) end
  local value=assert(row.Reads[method],'unsupported shared city read')
  return R.Copy(value)
end
function NI.Locations(index)
  assert(context,'shared inventory context required');local found={}
  for _,w in ipairs(context.Request.Observation.Works) do if w.Index==index then found[#found+1]=R.Copy(w) end end
  return found
end
function NI.Tick()
  local pending=box().Requests
  for _,key in ipairs(R.Keys(pending)) do
    local current=Game:GetProperty(KEY)
    local q=current and current.Requests[key] and R.Copy(current.Requests[key])
    if q and q.Observation and not q.Consumed then
      local ok,err=pcall(function()
        if q.Phase=='FLOW_BEGIN' then return JAC_AbsorptionFlowGameplay.Begin(q.Plan) end
        if q.Phase=='FLOW_CHECK' then return JAC_AbsorptionFlowGameplay.Resolve(q.To,q.ID) end
        if q.Phase=='SOURCE_PREPARE' or q.Phase=='CITY_COMMIT' or q.Phase=='CITY_VERIFY'
          or q.Phase:match('^CITY_LOYALTY:') then
          local tx=A.GetState().Transactions[q.ID]
          if tx and tx.Status~='COMPLETE' then return A.Resume(q.ID) end
          if tx and q.Phase:match('^CITY_LOYALTY:') then return A.ApplyCityFriction(q.To) end
        end
      end)
      if not ok and tostring(err)~='JAC_ABSORPTION_WAITING_UI' then print('[JAC_NATIVE_INVENTORY_RESUME_REJECTED] '..tostring(err)) end
    end
  end
end
-- Gameplay turn receipts refresh the observer assignment after load/rejoin.
-- UI GetPending is detached and never writes shared state or selects an actor.
function NI.Refresh()
  if not GameConfiguration or not GameConfiguration.IsAnyMultiplayer
    or not GameConfiguration.IsAnyMultiplayer() then return end
  local b=box();local turn=Game.GetCurrentGameTurn();local changed=prune(b,turn)
  local needsDescriptor=false
  for _,q in pairs(b.Requests) do if not q.Consumed and not q.Observation then needsDescriptor=true;break end end
  if not needsDescriptor then if changed then save(b) end;NI.Tick();return end
  local actor=selected()
  local core=descriptors();local signature=Encode(core)
  for _,key in ipairs(R.Keys(b.Requests)) do local q=b.Requests[key]
    if not q.Consumed and not q.Observation and (q.Actor~=actor or q.Turn~=turn or q.CoreSignature~=signature) then
      b.Serial=b.Serial+1;q.Actor=actor;q.Turn=turn;q.Nonce=b.Serial
      q.Core=R.Copy(core);q.CoreSignature=signature
      q.Chunks=nil;q.ChunkCount=nil;changed=true
    end
  end
  if changed then save(b) end
  NI.Tick()
end
local function receive(actor,p)
  assert(GameConfiguration.IsAnyMultiplayer() and type(p)=='table' and p.Version==1,'inventory wire contract')
  local b=box();local key=p.ID..':'..p.Phase;local q=assert(b.Requests[key],'inventory intent missing')
  assert(q.Actor==actor and p.Actor==actor and actor==selected() and active(actor),'inventory unauthorized actor')
  assert(p.From==q.From and p.To==q.To and p.Nonce==q.Nonce and p.Turn==q.Turn
    and p.Turn==Game.GetCurrentGameTurn(),'inventory stale intent')
  local chunked=p.Part~=nil or p.Parts~=nil
  if chunked then
    assert(type(p.Part)=='number' and type(p.Parts)=='number' and p.Part==math.floor(p.Part)
      and p.Parts==math.floor(p.Parts) and p.Parts>=1 and p.Parts<=64 and p.Part>=1 and p.Part<=p.Parts
      and type(p.Payload)=='string' and #p.Payload>0 and #p.Payload<=16384,'inventory chunk contract')
  end
  if q.Receipt then
    local receipt=receiptPayload(q)
    if chunked then
      assert(receipt.PartFingerprints and #receipt.PartFingerprints==p.Parts
        and Equal(receipt.PartFingerprints[p.Part],fingerprint(p.Payload)),'inventory duplicate chunk changed')
    else assert(Equal(receipt.Fingerprint,fingerprint(p.Payload)),'inventory duplicate payload changed') end
    if q.Consumed and (q.Core or q.Observation or q.Plan or q.Chunks) then
      q.Core=nil;q.CoreSignature=nil;q.Observation=nil;q.Plan=nil;q.Chunks=nil;q.ChunkCount=nil;save(b)
    end
    return true
  end
  if chunked then
    assert(not q.ChunkCount or q.ChunkCount==p.Parts,'inventory chunk count changed')
    if q.Chunks and q.Chunks[p.Part] then
      assert(q.Chunks[p.Part]==p.Payload,'inventory duplicate chunk changed');return true
    end
  end
  local fresh=descriptors();local signature=Encode(fresh)
  if signature~=q.CoreSignature then
    b.Serial=b.Serial+1;q.Core=fresh;q.CoreSignature=signature;q.Nonce=b.Serial
    q.Actor=selected();q.Chunks=nil;q.ChunkCount=nil;save(b);return false
  end
  local payload=p.Payload
  if chunked then
    q.ChunkCount=p.Parts;q.Chunks=q.Chunks or {};q.Chunks[p.Part]=p.Payload
    for part=1,q.ChunkCount do if not q.Chunks[part] then save(b);return true end end
    payload=table.concat(q.Chunks)
  else assert(not q.ChunkCount,'inventory mixed wire format') end
  local o=Decode(payload);validate(o,q.Core)
  local direct=captureCore(q.Core)
  if direct then assert(Equal(direct,o),'inventory differs from available Core getters') end
  if q.Observation then assert(Equal(q.Observation,o),'inventory duplicate payload changed');return true end
  q.Observation=R.Copy(o);q.EvidenceKind=direct and 'DIRECT_CORE' or 'SYNCHRONIZED_ACTOR_OBSERVATION'
  local fingerprints
  if chunked then fingerprints={};for i,part in ipairs(q.Chunks) do fingerprints[i]=fingerprint(part) end end
  q.Receipt={Actor=actor,Nonce=q.Nonce,Turn=q.Turn,
    PartFingerprints=fingerprints,Fingerprint=not chunked and fingerprint(payload) or nil}
  q.Chunks=nil;q.ChunkCount=nil;save(b);return true
end
GameEvents.JAC_NativeInventory_Operation.Add(function(actor,p)
  local ok,result=pcall(receive,actor,p)
  if not ok then print('[JAC_NATIVE_INVENTORY_REJECTED] '..tostring(result)) end
  if ok and result==true then NI.Tick() end
  return ok and result==true
end)
for _,name in ipairs({'PlayerTurnStartComplete','OnPlayerTurnEnded'}) do
  if GameEvents[name] then GameEvents[name].Add(NI.Refresh) end
end
ExposedMembers.JAC_NativeInventory={GetPending=function()
  local b=box();local list={};for _,q in pairs(b.Requests) do if not q.Consumed and not q.Observation then list[#list+1]=R.Copy(q) end end
  return list
end}
"#;

pub const UI: &str = r#"
local function observe(q)
  if q.Actor~=Game.GetLocalPlayer() or q.Turn~=Game.GetCurrentGameTurn() then return end
  local player=Players[q.Actor];if not player or not player:IsHuman() or not player:IsTurnActive() then return end
  local o={Cities={},Works={}}
  for _,d in ipairs(q.Core) do
    local c=assert(Players[d.Owner]:GetCities():FindID(d.ID),'inventory UI city not published')
    assert(c:GetX()==d.X and c:GetY()==d.Y and c:GetOwner()==d.Owner,'inventory UI identity changed')
    local b=c:GetBuildings();local row={Owner=d.Owner,ID=d.ID,X=d.X,Y=d.Y,Name=c:GetName(),Population=c:GetPopulation(),
      Buildings={},PropertyKeys=R.Copy(d.PropertyKeys),Properties={},Reads={RELIGIONS={},DISTRICTS={},SLOTS={},WORK={},LOYALTY=c:GetCulturalIdentity():GetLoyalty()}}
    for k in pairs(d.PropertyKeys) do row.Properties[k]=R.Copy(c:GetProperty(k)) end
    for _,r in pairs(c:GetReligion():GetReligionsInCity()) do row.Reads.RELIGIONS[#row.Reads.RELIGIONS+1]=R.Copy(r) end
    for _,r in c:GetDistricts():Members() do row.Reads.DISTRICTS[#row.Reads.DISTRICTS+1]={Type=GameInfo.Districts[r:GetType()].DistrictType,X=r:GetX(),Y=r:GetY()} end
    for info in GameInfo.Buildings() do if b:HasBuilding(info.Index) then
      local building=info.Index;row.Buildings[building]=true;local count=b:GetNumGreatWorkSlots(building)
      row.Reads.SLOTS[building]=count;row.Reads.WORK[building]={}
      for slot=0,count-1 do
        local index=b:GetGreatWorkInSlot(building,slot);row.Reads.WORK[building][slot]=index
        if index>=0 then
          local kind=b:GetGreatWorkTypeFromIndex(index);local definition=assert(GameInfo.GreatWorks[kind]);local effects={}
          for r in GameInfo.GreatWork_YieldChanges() do if r.GreatWorkType==definition.GreatWorkType then effects[#effects+1]=r.YieldType..' '..r.YieldChange end end
          if definition.Tourism and definition.Tourism~=0 then effects[#effects+1]='TOURISM '..definition.Tourism end;table.sort(effects)
          o.Works[#o.Works+1]={Index=index,Owner=d.Owner,CityID=d.ID,X=d.X,Y=d.Y,Building=building,BuildingType=info.BuildingType,
            Slot=slot,GreatWorkType=definition.GreatWorkType,ObjectType=definition.GreatWorkObjectType,Name=definition.Name,
            CreatorName=b:GetCreatorNameFromIndex(index),TurnCreated=b:GetTurnFromIndex(index),CreatedByPlayer=Game.GetGreatWorkPlayer(index),Effects=effects}
        end
      end
    end end
    table.sort(row.Reads.RELIGIONS,function(a,b) return Encode(a)<Encode(b) end)
    table.sort(row.Reads.DISTRICTS,function(a,b) return Encode(a)<Encode(b) end)
    o.Cities[#o.Cities+1]=row
  end
  table.sort(o.Works,function(a,b) return a.Index<b.Index end)
  local payload=Encode(o);local chunks={};local start=1
  while start<=#payload do
    local last=math.min(start+16383,#payload)
    -- Native string parameters must not split a UTF-8 codepoint.
    while last<#payload do local byte=payload:byte(last+1)
      if byte<128 or byte>191 then break end;last=last-1
    end
    chunks[#chunks+1]=payload:sub(start,last);start=last+1
  end
  local count=#chunks;assert(count>=1 and count<=64,'inventory publication size')
  for part=1,count do
    UI.RequestPlayerOperation(q.Actor,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_NativeInventory_Operation',Version=1,
      Actor=q.Actor,ID=q.ID,From=q.From,To=q.To,Phase=q.Phase,Nonce=q.Nonce,Turn=q.Turn,
      Part=part,Parts=count,Payload=chunks[part]})
  end
end
local function publish()
  if not GameConfiguration.IsAnyMultiplayer() then return end
  local api=ExposedMembers.JAC_NativeInventory;if not api then return end
  for _,q in ipairs(api.GetPending()) do local ok,err=pcall(observe,q)
    if not ok then print('[JAC_NATIVE_INVENTORY_WAITING] '..tostring(err)) end
  end
end
Events.GameCoreEventPublishComplete.Add(publish)
Events.LoadScreenClose.Add(publish)
Events.LocalPlayerTurnBegin.Add(publish)
"#;
