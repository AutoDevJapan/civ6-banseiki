local A=assert(JAC_AbsorptionGameplay);local R=A.Rules
local function fresh()
 return {Version=1,Nonce=nonce,Generation=0,ResumeStep=6,NextOperation=1,Receipts={}}
end
local function read()
 local b=Game:GetProperty(bridgeKey)
 if not b then return fresh() end
 assert(b.Version==1 and b.Nonce==nonce,'saved witness identity changed')
 return b
end
local function box() return R.Copy(read()) end
local function save(b) Game:SetProperty(bridgeKey,R.Copy(b)) end
-- Initialization only creates the diagnostic ledger, never native assets.
if not Game:GetProperty(bridgeKey) then save(fresh()) else box() end
prepareSave=function(generation)
 local b=box();local e=assert(Game:GetProperty(fixtureKey))
 assert(generation==b.Generation+1 and generation<=2,'save generation out of order')
 local r=assert(A.GetState().Recoveries[e.Token]);local c=assert(r.NativeCarrier)
 if generation==1 then
  assert(c.MP.Phase=='BORROWED' and not c.MP.Claimed,'mid-loan checkpoint already advanced')
  local city=assert(CityManager.GetCityAt(e.Target.X,e.Target.Y));local l=assert(c.MP.Loans[2])
  assert(c.MP.Actor==0 and c.MP.MoveOwner==0 and l.Owner==1 and l.Issued and city:GetOwner()==0 and city:GetID()==l.BorrowedID,'mid-loan actual human-source city changed')
 else
  assert(r.Status=='COMPLETE' and c.MP.Phase=='DONE','returned checkpoint incomplete')
  local city=assert(CityManager.GetCityAt(e.Target.X,e.Target.Y))
  assert(city:GetOwner()==1 and city:GetID()==c.MP.Loans[2].FinalID,'actual returned city changed')
 end
 b.Generation=generation;b.ResumeStep=generation==1 and 33 or 41
 if generation==2 then
  local observed=assert(ExposedMembers.JAC_NC_SaveCanonical,'complete UI save observation missing')
  assert(type(observed)=='string' and #observed>0,'invalid complete UI save observation')
  b.SavedCanonical=observed;b.CaptureSource='UI_REPORTED_COMPLETE_LEDGER_WORLD'
 end
 b.SaveName=(generation==1 and 'JAC-NativeCarrier-MidLoan-' or 'JAC-NativeCarrier-Returned-')..nonce
 b.Checkpoint=R.Copy({Fixture=e,Recovery=r})
 save(b)
 return A.Canonical(b.Checkpoint)
end
local function receive(actor,p)
 assert(type(p)=='table' and p.Version==1 and p.Command=='FIXED' and p.Nonce==nonce,'invalid fixed witness packet')
 assert(actor==0 and p.Actor==0 and Players[0]:IsAlive() and Players[0]:IsHuman()
  and Players[0]:IsTurnActive() and Players[1]:IsHuman() and not Players[2]:IsHuman(),'actual human0 fixture turn required')
 local humans=0
 for _,id in ipairs(PlayerManager.GetAliveIDs()) do if Players[id]:IsHuman() then
  assert(id==0 or id==1,'fixed witness has an unexpected human slot');humans=humans+1
 end end
 assert(humans==2,'fixed witness needs exactly two actual living humans')
 assert(p.Turn==Game.GetCurrentGameTurn(),'stale fixture operation turn')
 local b=box();local key=tostring(p.Generation)..':'..tostring(p.Step)
 -- A receipt, including STARTED or FAIL, never grants another engine attempt.
 if b.Receipts[key] then return end
 assert(p.Generation==b.Generation and p.Step==operation_order[b.NextOperation]
  and type(operations[p.Step])=='function','fixed fixture operation order changed')
 assert(p.Source==nil and p.Code==nil and p.Snapshot==nil,'runtime source/snapshot injection forbidden')
 b.Receipts[key]={Status='STARTED',Step=p.Step,Generation=p.Generation,Turn=p.Turn}
 save(b) -- durable before any native mutation; a torn/unknown call stays spent
 local ok,result=pcall(operations[p.Step])
 b=box();local receipt=assert(b.Receipts[key])
 if ok then
  receipt.Status='COMPLETE';receipt.Canonical=A.Canonical({Result=result,Fixture=Game:GetProperty(fixtureKey),State=A.GetState()})
  receipt.Value=R.Copy(result);b.NextOperation=b.NextOperation+1
 else receipt.Status='FAIL';receipt.Error=tostring(result) end
 save(b)
end
GameEvents.JAC_NativeCarrierWitness_Operation.Add(function(actor,p)
 local ok,err=pcall(receive,actor,p)
 if not ok then
  local b=box();b.Failure=tostring(err);save(b)
  print('[JAC_NATIVE_CARRIER_WITNESS_REJECTED] '..tostring(err))
 end
end)
-- Detached data only. Never export or copy the private Gameplay API to UI.
ExposedMembers.JAC_NativeCarrierWitness={Get=function() return R.Copy(box()) end}
ExposedMembers.JAC_NativeCarrierWitness.GetHeader=function()
 local b=read()
 return {Version=b.Version,Nonce=b.Nonce,Generation=b.Generation,ResumeStep=b.ResumeStep,
  NextOperation=b.NextOperation,SaveName=b.SaveName,Failure=b.Failure}
end
ExposedMembers.JAC_NativeCarrierWitness.GetReceipt=function(generation,step)
 return R.Copy(read().Receipts[tostring(generation)..':'..tostring(step)])
end
