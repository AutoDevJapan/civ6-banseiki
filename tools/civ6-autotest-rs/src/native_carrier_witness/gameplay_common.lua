-- Gameplay has no UI session getters. Require the actual native MP predicate
-- and the persisted Hotseat setting verified by the fixed frontend controller.
local nonce=GameConfiguration and GameConfiguration.GetValue and GameConfiguration.GetValue('JAC_NATIVE_CARRIER_WITNESS_NONCE')
if type(nonce)~='string' or #nonce~=32 or not nonce:match('^%x+$') then return end
assert(type(GameConfiguration.IsNetworkMultiplayer)=='function','native multiplayer predicate missing')
local function nativeHotseat()
 local confirmed=GameConfiguration.GetValue('JAC_NATIVE_CARRIER_WITNESS_HOTSEAT')
 return GameConfiguration.IsNetworkMultiplayer()==true and (confirmed==true or confirmed==1)
end
print('[JAC_NATIVE_CARRIER_CONTEXT] Network='..tostring(GameConfiguration.IsNetworkMultiplayer())..' VerifiedHotseat='..tostring(GameConfiguration.GetValue('JAC_NATIVE_CARRIER_WITNESS_HOTSEAT')))
if not nativeHotseat() then return end
local bridgeKey='JAC_NATIVE_CARRIER_WITNESS_V1'
local fixtureKey='JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'
