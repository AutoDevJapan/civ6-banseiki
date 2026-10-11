-- BEGIN GENERATED NATIVE CARRIER FRONTEND
local requiredRuleset="RULESET_JAC_BANSEIKI"
local requiredMap="{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map"
-- Configuration may be discarded before this frontend VM is constructed.
-- Fixed keyboard transport accepts only a nonce, generation and checksum.
include('MainMenu')
local stockShow=assert(OnShow)
local stockUpdate=assert(OnUpdate)
local armed,packet,scene
local attempted={}
-- Structured input reports game Keys values, not Windows virtual-key codes.
local digitKeys={}
for n=1,16 do digitKeys[assert(Keys['VK_F'..n])]=string.format('%x',n-1) end
local serial,epoch=0,0
local announced,failed=false,false
local function quoted(s)
 assert(type(s)=='string')
 return '"'..s:gsub('[%z\1-\31\\"]',function(c)
  if c=='\\' then return '\\\\' elseif c=='"' then return '\\"' end
  return string.format('\\u%04x',string.byte(c))
 end)..'"'
end
local function visible() return not ContextPtr:IsHidden() and UI.IsInFrontEnd() end
local function saveName(r)
 return 'JAC-NativeCarrier-'..(r.Generation==1 and 'MidLoan' or 'Returned')..'-'..r.Nonce
end
local function emit(kind,r,code,err)
 local body='{"generation":'..r.Generation..',"canonical":'..quoted(kind..':'..r.ID)..',"name":'..quoted(saveName(r))
 if code then body=body..',"code":'..quoted(code) end
 if err then body=body..',"error":'..quoted(tostring(err)) end
 body=body..'}';serial=serial+1
 local parts=math.max(1,math.ceil(#body/96))
 for part=1,parts do
  local bytes=body:sub((part-1)*96+1,part*96);local hex={}
  for i=1,#bytes do hex[i]=string.format('%02x',bytes:byte(i)) end
  print('JAC_NC|1|'..r.Nonce..'|'..scene..'.g'..r.Generation..'|'..serial..'|'..r.Generation..'|0|'..kind..'|'..part..'/'..parts..'|'..table.concat(hex))
 end
end
local function apply(r,spent)
 GameConfiguration.SetToDefaults(GameModeTypes.HOTSEAT)
 GameConfiguration.SetRuleSet(requiredRuleset)
 MapConfiguration.SetValue('MAP_SCRIPT',requiredMap)
 GameConfiguration.SetValue('JAC_NATIVE_CARRIER_WITNESS_NONCE',r.Nonce)
 GameConfiguration.SetValue('JAC_NATIVE_CARRIER_LOAD_GENERATION',r.Generation)
 GameConfiguration.SetValue('JAC_NATIVE_CARRIER_WITNESS_HOTSEAT',true)
 GameConfiguration.SetValue('JAC_NATIVE_CARRIER_LOAD_REQUEST',r.ID)
 if spent then GameConfiguration.SetValue('JAC_NATIVE_CARRIER_FRONTEND_SPENT',r.ID) end
 assert(GameConfiguration.IsHotseat() and GameConfiguration.IsAnyMultiplayer()
  and GameConfiguration.GetValue('JAC_NATIVE_CARRIER_WITNESS_NONCE')==r.Nonce
  and tonumber((GameConfiguration.GetValue('JAC_NATIVE_CARRIER_LOAD_GENERATION')))==r.Generation,
  'frontend configuration not published')
end
local function announce()
 if announced or failed or not visible() then return end
 epoch=epoch+1;serial=0
 scene=string.gsub(tostring(ContextPtr),'[^%w]','')..'.fe'..epoch
 announced=true;print('JAC_NC_FRONTEND_AVAILABLE|'..scene)
end
OnShow=function(...)
 stockShow(...);armed=nil;packet=nil;announced=false;announce()
end
ContextPtr:SetShowHandler(OnShow)
OnUpdate=function(delta,...)
 stockUpdate(delta,...);announce()
 if packet then
  packet.Elapsed=packet.Elapsed+(tonumber(delta) or 0)
  if packet.Elapsed>15 then packet=nil;failed=true end
 end
end
ContextPtr:SetInputHandler(function(input)
 local message,key=input:GetMessageType(),input:GetKey()
 if not visible() or not announced or failed then return false end
 if message~=KeyEvents.KeyUp then return packet~=nil end
 if packet then
  if key==Keys.VK_F21 then
   local text=packet.Text;packet=nil
   if #text~=37 then failed=true;return true end
   local payload=text:sub(1,33);local checksum=0
   for i=1,#payload do checksum=(checksum*17+assert(tonumber(payload:sub(i,i),16)))%65536 end
   local g=tonumber(payload:sub(33),16)
   if checksum~=tonumber(text:sub(34),16) or (g~=1 and g~=2) then failed=true;return true end
   local n=payload:sub(1,32);local id=n..':'..g
   if attempted[id] or armed then failed=true;return true end
   local r={Nonce=n,Generation=g,ID=id}
   local ok,err=pcall(function() apply(r);armed=r;emit('FRONTEND_READY',r) end)
   if not ok then failed=true;armed=nil;emit('FAIL',r,'FRONTEND_PREPARE',err) end
  elseif digitKeys[key] and #packet.Text<37 then
   packet.Text=packet.Text..digitKeys[key]
  else packet=nil;failed=true end
  return true
 end
 if key==Keys.VK_F20 then
  if armed then failed=true;return true end
  packet={Text='',Elapsed=0};return true
 end
 if key~=Keys.VK_F15 or not armed then return false end
 local r=armed
 if attempted[r.ID] then return false end
 attempted[r.ID]=true;armed=nil
 local ok,err=pcall(function()
  GameConfiguration.SetValue('JAC_NATIVE_CARRIER_FRONTEND_SPENT',r.ID)
  assert(GameConfiguration.GetValue('JAC_NATIVE_CARRIER_FRONTEND_SPENT')==r.ID,'frontend sent marker not published')
  emit('LOAD_REQUESTED',r)
  Network.LeaveGame();apply(r,true)
  local ret=Network.LoadGame({Name=saveName(r),Location=SaveLocations.LOCAL_STORAGE,
   Type=Network.GetGameConfigurationSaveType(),Directory=SaveDirectories.DEFAULT,
   IsAutosave=false,IsQuicksave=false},ServerType.SERVER_TYPE_HOTSEAT)
  assert(ret~=false,'Network.LoadGame rejected fixed save')
 end)
 if not ok then failed=true;emit('FAIL',r,'FRONTEND_LOAD',err) end
 return true
end,true)
-- Stock Show can run before the engine's frontend flag becomes true.
-- Preserve its active update even then, so the later transition is observed.
if not ContextPtr:IsHidden() then announce();ContextPtr:SetUpdate(OnUpdate) end
-- END GENERATED NATIVE CARRIER FRONTEND
