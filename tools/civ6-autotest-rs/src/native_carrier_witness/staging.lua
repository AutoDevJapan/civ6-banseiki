local canonical=assert(JAC_AbsorptionRules and JAC_AbsorptionRules.Canonical,'staging witness requires canonical codec')
local attempted=false
local function ready()
 if attempted or ContextPtr:IsHidden() or not Network.IsGameHost() or Controls.ReadyButton:IsDisabled() then return end
 local humans=0
 for _,id in ipairs(GameConfiguration.GetParticipatingPlayerIDs()) do
  local p=PlayerConfigurations[id]
  if p:GetSlotStatus()==SlotStatus.SS_TAKEN then humans=humans+1 end
 end
 assert(humans==2,'native staging must retain two actual human slots')
 local savedGeneration=GameConfiguration.GetValue('JAC_NATIVE_CARRIER_LOAD_GENERATION')
 local generation=savedGeneration~=nil and assert(tonumber(savedGeneration)) or 0
 assert(generation==0 or generation==1 or generation==2)
 emit('STAGING_READY',generation,0,canonical({Nonce=nonce,Humans=humans,Hotseat=true,Host=true,ReadyEnabled=true}))
 attempted=true -- set before invoking the official handler; never replay unknown
 Controls.ReadyButton:DoLeftClick()
end
-- The existing stock update callback is preserved; no Ready flags are forged.
local update=UpdateReadyButton
UpdateReadyButton=function(...) update(...);local ok,err=pcall(ready)
 if not ok then
  emit('FAIL',0,0,canonical({Error=tostring(err)}),{code='STAGING_READY',error=tostring(err)})
  attempted=true
 end
end
