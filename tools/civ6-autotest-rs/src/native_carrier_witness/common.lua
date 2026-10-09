-- No test mode or product guard is weakened. Invalid/unset nonce is a no-op.
local nonce=GameConfiguration and GameConfiguration.GetValue and GameConfiguration.GetValue('JAC_NATIVE_CARRIER_WITNESS_NONCE')
if type(nonce)~='string' or #nonce~=32 or not nonce:match('^%x+$') then return end
if not GameConfiguration.IsHotseat() or not GameConfiguration.IsAnyMultiplayer() then return end
local bridgeKey='JAC_NATIVE_CARRIER_WITNESS_V1'
local fixtureKey='JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996'
local function jsonString(s)
 assert(type(s)=='string')
 return '"'..s:gsub('[%z\1-\31\\"]',function(c)
  if c=='\\' then return '\\\\' elseif c=='"' then return '\\"' end
  return string.format('\\u%04x',string.byte(c))
 end)..'"'
end
local serial=0
local scene
local function initializeScene(generation)
 -- Freeze the actual construction generation for this entire UI scene,
 -- including SAVE_RETURNED carrying its next header generation. Both
 -- reloads differ even if the engine reuses a ContextPtr address.
 scene=string.gsub(tostring(ContextPtr),'[^%w]','')..'.g'..generation
 assert(#scene<=128 and scene:match('^[%w%.]+$'),'invalid native scene')
end
local function emit(kind,generation,step,canonical,extra)
 assert(type(canonical)=='string' and #canonical>0,'complete nonempty canonical record required')
 assert(generation>=0 and generation<=2 and generation==math.floor(generation),'invalid save generation')
 assert(step>=0 and step<=43 and step==math.floor(step),'invalid fixed stage')
 if not scene then
  initializeScene(generation)
 end
 local body='{"generation":'..generation..',"canonical":'..jsonString(canonical)
 for _,key in ipairs({'name','code','error'}) do if extra and extra[key]~=nil then
  body=body..','..jsonString(key)..':'..jsonString(extra[key])
 end end
 body=body..'}'
 assert(#body<=1048576,'native complete record exceeds one MiB; never truncate evidence')
 serial=serial+1
 local parts=math.max(1,math.ceil(#body/96))
 assert(parts<=10923)
 for part=1,parts do
  local bytes=body:sub((part-1)*96+1,part*96);local hex={}
  for i=1,#bytes do hex[i]=string.format('%02x',bytes:byte(i)) end
  print('JAC_NC|1|'..nonce..'|'..scene..'|'..serial..'|'..generation..'|'..step..'|'..kind..'|'..part..'/'..parts..'|'..table.concat(hex))
 end
end
