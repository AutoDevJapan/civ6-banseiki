local api=assert(ExposedMembers.JAC_Absorption,'product Gameplay must initialize before native witness UI')
local bridge=assert(ExposedMembers.JAC_NativeCarrierWitness,'fixed Gameplay witness entry missing')
local loaded=bridge.Get();assert(loaded.Nonce==nonce)
local generation=loaded.Generation
local step=loaded.ResumeStep
local booted=false
local failed=false
local pending
local spent={}
local captures={}
local saving
local lastDialog
local lastReady
local closeSeen=false
local earlyArmed=false
local loadClosed=false
local timedStep=step
local stageElapsed=0
local function canonical(value) return api.Canonical(value) end
local function failure(code,err)
 if failed then return end;failed=true
 emit('FAIL',generation,step,canonical({Code=code,Error=tostring(err),Fixture=Game:GetProperty(fixtureKey)}),{code=code,error=tostring(err)})
end
-- This file is included immediately after JAC_CarrierMP_UI, before inventory
-- publication and the HeroKeys observer subscriptions. No runtime Tuner hook.
if generation==1 then
 assert(JAC996BorrowObserver==nil,'initial-scene borrow wrapper survived load')
 assert(armLoadedMove()==true,'real MOVE listener not armed before publication')
 earlyArmed=true
end
Events.LoadScreenClose.Add(function() loadClosed=true end)
local function dialog()
 local control=ContextPtr:LookUpControl('/InGame/PlayerChange')
 if not control then return false,false end
 local shown=not control:IsHidden() and UIManager:IsInPopupQueue(control)
 if not shown then return false,false end
 local panel=assert(ContextPtr:LookUpControl('/InGame/PlayerChange/PopupAlphaIn'))
 local button=assert(ContextPtr:LookUpControl('/InGame/PlayerChange/OkButton'))
 local id=Game.GetLocalPlayer();local player=Players[id];local config=PlayerConfigurations[id]
 local ready=panel and button and not panel:IsHidden() and not button:IsHidden() and not button:IsDisabled()
  and config and config:GetHotseatPassword()=='' and player and player:IsHuman() and player:IsTurnActive() and GameConfiguration.IsPaused()
 return true,ready==true
end
local function result(n,value,record)
 local a=actions[n]
 if a.capture~='' and a.capture then captures[a.capture]=value end
 if n==37 then ExposedMembers.JAC_NC_SaveCanonical=value end
 if a.compare~='' and a.compare then
  local expected=captures[a.compare] or loaded.SavedCanonical
  assert(type(expected)=='string' and value==expected,'complete saved ledger/current world differ')
 end
 local observed=record
 if not observed then
  if n==37 or n==43 then observed=value
  else observed=canonical({Result=value,Fixture=Game:GetProperty(fixtureKey),State=api.GetState(),OriginalObservation=ExposedMembers.JAC996Original}) end
 end
 emit(n==43 and 'DONE' or 'RESULT',generation,n,observed)
 step=n+1
end
local function send(n)
 assert(not spent[n],'same scene fixed operation already attempted');spent[n]=true
 assert(not bridge.Get().Receipts[tostring(generation)..':'..n],'saved operation is already spent; do not resend even after UI reset')
 pending={Step=n,Generation=generation}
 -- Gameplay validates the actual actor, fixed order and save generation.
 UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_NativeCarrierWitness_Operation',
  Version=1,Command='FIXED',Nonce=nonce,Actor=0,Generation=generation,Step=n,Turn=Game.GetCurrentGameTurn()})
end
local function saveGame(n)
 local b=bridge.Get();generation=b.Generation
 assert(b.SaveName and b.Generation==actions[n].generation,'save checkpoint generation differs')
 -- Complete actual world observation, retained in the file's diagnostic ledger
 -- by a separate fixed receipt before the native save, never a Core snapshot.
 local value=actions[37].run()
 assert(type(value)=='string' and #value>0)
 GameConfiguration.SetValue('JAC_NATIVE_CARRIER_LOAD_GENERATION',generation)
 assert(tonumber(GameConfiguration.GetValue('JAC_NATIVE_CARRIER_LOAD_GENERATION'))==generation,'save configuration generation was not applied')
 saving={Step=n,Name=b.SaveName,Canonical=value,Returned=false,Complete=false,Emitted=false}
 local requested=saving
 local function completed()
  Events.SaveComplete.Remove(completed);requested.Complete=true
 end
 Events.SaveComplete.Add(completed)
 local ok,ret=pcall(function() return Network.SaveGame({Name=b.SaveName,Location=SaveLocations.LOCAL_STORAGE,
  Type=assert(Network.GetGameConfigurationSaveType()),IsAutosave=false,IsQuicksave=false}) end)
 if not ok or ret==false then Events.SaveComplete.Remove(completed);error('native save failed: '..tostring(ret)) end
 saving.Returned=true
 emit('SAVE_RETURNED',generation,n,value,{name=b.SaveName,code=ret==nil and 'API_RETURNED_VOID' or 'API_RETURNED_'..type(ret)..':'..tostring(ret)})
end
local function tick(delta)
 if failed then return end
 assert(type(delta)=='number' and delta>=0 and delta==delta,'invalid update elapsed time')
 if Game.GetLocalPlayer()<0 then return end
 if timedStep~=step then timedStep=step;stageElapsed=0 end
 stageElapsed=stageElapsed+delta
 if step<=43 then assert(stageElapsed<assert(actions[step].timeout),'fixed stage timeout: '..step) end
 local b=bridge.GetHeader();if b.Failure then error(b.Failure) end
 local shown,ready=dialog()
 if ready and lastReady~=true then emit('DIALOG_OPEN',generation,0,canonical({Player=Game.GetLocalPlayer(),Turn=Game.GetCurrentGameTurn(),Paused=GameConfiguration.IsPaused(),Shown=shown,Ready=ready})) end
 if not shown and lastDialog==true then
  emit('DIALOG_CLOSED',generation,0,canonical({Player=Game.GetLocalPlayer(),Turn=Game.GetCurrentGameTurn(),Paused=GameConfiguration.IsPaused(),Shown=false}));closeSeen=true
 end
 lastDialog=shown;lastReady=ready
 if shown or GameConfiguration.IsPaused() or not closeSeen or Game.GetLocalPlayer()~=0 or not Players[0]:IsTurnActive() then return end
 if step>43 then return end
 if pending then
  local receipt=bridge.GetReceipt(pending.Generation,pending.Step)
  if not receipt or receipt.Status=='STARTED' then return end
  if receipt.Status~='COMPLETE' then error(receipt.Error or 'fixed operation rejected') end
  assert(type(receipt.Fixture)=='table','completed operation lacks its fixture publication')
  -- A direct Gameplay receipt can arrive before the UI's native property view.
  -- Keep this operation spent and its deadline running until the actual UI
  -- value matches. Never substitute the receipt for the UI's world observation.
  if canonical(Game:GetProperty(fixtureKey))~=canonical(receipt.Fixture) then return end
  local n=pending.Step;pending=nil
  if actions[n].kind=='save' then saveGame(n) else result(n,receipt.Value,receipt.Canonical) end
  return
 end
 if saving then
  if saving.Returned and saving.Complete then
   assert(not saving.Emitted);saving.Emitted=true
   emit('SAVE_COMPLETE',generation,saving.Step,saving.Canonical,{name=saving.Name})
   step=saving.Step+1;saving=nil
  end
  return
 end
 local a=assert(actions[step],'unregistered native stage')
 if a.kind=='GP' or a.kind=='save' then send(step)
 elseif a.kind=='exit' then
  assert(not spent[step]);spent[step]=true
  emit('EXIT_REQUESTED',generation,step,canonical({Bridge=b,Fixture=Game:GetProperty(fixtureKey)}))
  step=step+1;Events.ExitToMainMenu()
 elseif a.kind=='external_load' then return -- parent checks real file and sends menu load once
 elseif a.kind=='wait_for' then
  local ready=a.run();if ready==true then result(step,true) end
 else
  assert(not spent[step]);spent[step]=true
  local value=a.run();result(step,value)
 end
end
-- Synchronous construction receipts do not depend on hidden context updates
-- before Begin. The importing script has not yet subscribed normal HeroKeys
-- or inventory observers. Actual listener registration precedes these frames.
local construction=bridge.GetHeader()
emit('BOOT',generation,0,canonical({Bridge=construction,Fixture=Game:GetProperty(fixtureKey),State=api.GetState()}))
assert(generation~=1 or earlyArmed)
emit('LOAD_READY',generation,0,canonical({EarlyMoveListenerRegistered=earlyArmed,LoadScreenCloseReceived=loadClosed,Bridge=construction}))
booted=true
ContextPtr:SetHide(false) -- the existing HeroKeys context has no visual controls
ContextPtr:SetUpdate(function(delta)
 local ok,err=pcall(tick,delta)
 if not ok then failure('FIXTURE_STAGE',err) end
end)
