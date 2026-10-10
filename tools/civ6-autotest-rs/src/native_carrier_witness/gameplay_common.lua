-- Gameplay has no UI Hotseat getter. Qualify the verified frontend setting
-- with actual living human players; Hotseat's network predicate is false.
local nonce=GameConfiguration and GameConfiguration.GetValue and GameConfiguration.GetValue('JAC_NATIVE_CARRIER_WITNESS_NONCE')
if type(nonce)~='string' or #nonce~=32 or not nonce:match('^%x+$') then return end
assert(type(GameConfiguration.IsNetworkMultiplayer)=='function','native multiplayer predicate missing')
local function nativeHotseat()
 local confirmed=GameConfiguration.GetValue('JAC_NATIVE_CARRIER_WITNESS_HOTSEAT')
 if confirmed~=true and confirmed~=1 then return false end
 local humans=0
 for _,id in ipairs(PlayerManager.GetAliveIDs()) do
  local player=Players[id]
  if player and player:IsHuman() then
   if (id~=0 and id~=1) or not player:IsAlive() then return false end
   humans=humans+1
  end
 end
 return humans==2 and Players[0]:IsAlive() and Players[1]:IsAlive()
end
print('[JAC_NATIVE_CARRIER_CONTEXT] Network='..tostring(GameConfiguration.IsNetworkMultiplayer())..' VerifiedHotseat='..tostring(GameConfiguration.GetValue('JAC_NATIVE_CARRIER_WITNESS_HOTSEAT')))
if not nativeHotseat() then return end
local bridgeKey='JAC_NATIVE_CARRIER_WITNESS_V1'
local fixtureKey='JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'
