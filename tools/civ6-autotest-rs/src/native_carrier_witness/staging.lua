local canonical=assert(JAC_AbsorptionRules and JAC_AbsorptionRules.Canonical,'staging witness requires canonical codec')
local bindingGeneration=GameConfiguration.GetValue('JAC_NATIVE_CARRIER_LOAD_GENERATION')
print('[JAC_NATIVE_CARRIER_STAGING_BIND] Nonce='..nonce..' Generation='..tostring(bindingGeneration))
local attempted={}
local failed=false
local function ready()
 if failed then return end
 local savedGeneration=GameConfiguration.GetValue('JAC_NATIVE_CARRIER_LOAD_GENERATION')
 local generation=savedGeneration~=nil and assert(tonumber(savedGeneration)) or 0
 assert(generation==0 or generation==1 or generation==2)
 if attempted[generation] or ContextPtr:IsHidden() or not Network.IsGameHost() or Controls.ReadyButton:IsDisabled() then return end
 local humans=0
 for _,id in ipairs(GameConfiguration.GetParticipatingPlayerIDs()) do
  local p=PlayerConfigurations[id]
  if p:GetSlotStatus()==SlotStatus.SS_TAKEN then humans=humans+1 end
 end
 assert(humans==2,'native staging must retain two actual human slots')
 emit('STAGING_READY',generation,0,canonical({Nonce=nonce,Humans=humans,Hotseat=true,Host=true,ReadyEnabled=true}))
 attempted[generation]=true -- durable in this context before the official handler
 Controls.ReadyButton:DoLeftClick()
end
-- The existing stock update callback is preserved; no Ready flags are forged.
local update=UpdateReadyButton
UpdateReadyButton=function(...) update(...);local ok,err=pcall(ready)
 if not ok then
  local g=tonumber(GameConfiguration.GetValue('JAC_NATIVE_CARRIER_LOAD_GENERATION')) or 0
  if g~=0 and g~=1 and g~=2 then g=0 end
  failed=true
  emit('FAIL',g,0,canonical({Error=tostring(err)}),{code='STAGING_READY',error=tostring(err)})
 end
end
