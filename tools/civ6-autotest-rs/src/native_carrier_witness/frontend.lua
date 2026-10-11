-- Fixed, invisible transport on the stock MainMenu. No arbitrary Lua or path.
local nonceKey='JAC_NATIVE_CARRIER_WITNESS_NONCE'
local generationKey='JAC_NATIVE_CARRIER_LOAD_GENERATION'
local requestKey='JAC_NATIVE_CARRIER_LOAD_REQUEST'
local spentKey='JAC_NATIVE_CARRIER_FRONTEND_SPENT'
local hotseatKey='JAC_NATIVE_CARRIER_WITNESS_HOTSEAT'
local function capture()
 if not GameConfiguration or type(GameConfiguration.GetValue)~='function' then return nil end
 local n=GameConfiguration.GetValue(nonceKey)
 local g=tonumber((GameConfiguration.GetValue(generationKey)))
 if type(n)~='string' or #n~=32 or not n:match('^%x+$') or (g~=1 and g~=2) then return nil end
 local id=n..':'..g
 if GameConfiguration.GetValue(requestKey)~=id
  or GameConfiguration.GetValue(hotseatKey)~=true
  or not GameConfiguration.IsHotseat() or not GameConfiguration.IsAnyMultiplayer()
  or GameConfiguration.GetValue('RULESET')~=requiredRuleset
  or not MapConfiguration or MapConfiguration.GetValue('MAP_SCRIPT')~=requiredMap then return nil end
 return {Nonce=n,Generation=g,ID=id,Spent=GameConfiguration.GetValue(spentKey)}
end
-- Capture before the stock include/OnShow can reset the game configuration.
local constructed=capture()
include('MainMenu')
local stockShow=assert(OnShow)
local stockUpdate=assert(OnUpdate)
local armed
local attempted={}
local serial=0
local failed=false
local function quoted(s)
 assert(type(s)=='string')
 return '"'..s:gsub('[%z\1-\31\\"]',function(c)
  if c=='\\' then return '\\\\' elseif c=='"' then return '\\"' end
  return string.format('\\u%04x',string.byte(c))
 end)..'"'
end
local function saveName(r)
 return 'JAC-NativeCarrier-'..(r.Generation==1 and 'MidLoan' or 'Returned')..'-'..r.Nonce
end
local function emit(kind,r,code,err)
 local name=saveName(r)
 local body='{"generation":'..r.Generation..',"canonical":'..quoted(kind..':'..r.ID)..',"name":'..quoted(name)
 if code then body=body..',"code":'..quoted(code) end
 if err then body=body..',"error":'..quoted(tostring(err)) end
 body=body..'}'
 serial=serial+1
 local scene=string.gsub(tostring(ContextPtr),'[^%w]','')..'.fe.g'..r.Generation
 local parts=math.max(1,math.ceil(#body/96))
 for part=1,parts do
  local bytes=body:sub((part-1)*96+1,part*96);local hex={}
  for i=1,#bytes do hex[i]=string.format('%02x',bytes:byte(i)) end
  print('JAC_NC|1|'..r.Nonce..'|'..scene..'|'..serial..'|'..r.Generation..'|0|'..kind..'|'..part..'/'..parts..'|'..table.concat(hex))
 end
end
local function apply(r,spent)
 spent=spent or ''
 GameConfiguration.SetToDefaults(GameModeTypes.HOTSEAT)
 GameConfiguration.SetRuleSet(requiredRuleset)
 MapConfiguration.SetValue('MAP_SCRIPT',requiredMap)
 GameConfiguration.SetValue(nonceKey,r.Nonce)
 GameConfiguration.SetValue(generationKey,r.Generation)
 GameConfiguration.SetValue(hotseatKey,true)
 GameConfiguration.SetValue(requestKey,r.ID)
 GameConfiguration.SetValue(spentKey,spent)
 assert(GameConfiguration.IsHotseat() and GameConfiguration.IsAnyMultiplayer()
  and GameConfiguration.GetValue(nonceKey)==r.Nonce
  and tonumber((GameConfiguration.GetValue(generationKey)))==r.Generation
  and GameConfiguration.GetValue(requestKey)==r.ID
  and GameConfiguration.GetValue(spentKey)==spent,'frontend configuration not published')
end
local function arm(r)
 if not r or r.Spent==r.ID or attempted[r.ID] or failed then armed=nil;return end
 local ok,err=pcall(function()
  assert(UI.IsInFrontEnd(),'frontend required')
  apply(r,r.Spent)
  armed=r
 end)
 if not ok then armed=nil;failed=true;emit('FAIL',r,'FRONTEND_PREPARE',err) end
end
local function publishReady()
 if not armed or armed.Emitted or failed or ContextPtr:IsHidden() or not UI.IsInFrontEnd() then return end
 local current=capture()
 if not current or current.ID~=armed.ID or current.Spent==armed.ID then armed=nil;return end
 armed.Emitted=true;emit('FRONTEND_READY',armed)
end
OnShow=function(...)
 local r=capture()
 stockShow(...)
 arm(r);publishReady()
end
ContextPtr:SetShowHandler(OnShow)
-- Preserve the stock updater. Only this exact new request consumes F15.
OnUpdate=function(...) stockUpdate(...);publishReady() end
ContextPtr:SetInputHandler(function(message,key)
 if message~=KeyEvents.KeyUp or key~=0x7E or not armed then return false end
 local r=armed
 if failed or attempted[r.ID] or ContextPtr:IsHidden() or not UI.IsInFrontEnd() then return false end
 local current=capture()
 if not current or current.ID~=r.ID or current.Spent==r.ID then return false end
 attempted[r.ID]=true;armed=nil
 local ok,err=pcall(function()
  -- Persist before leaving/loading, including response loss or VM recreation.
  GameConfiguration.SetValue(spentKey,r.ID)
  assert(GameConfiguration.GetValue(spentKey)==r.ID,'frontend sent marker not published')
  emit('LOAD_REQUESTED',r)
  Network.LeaveGame()
  apply(r,r.ID)
  local ret=Network.LoadGame({Name=saveName(r),Location=SaveLocations.LOCAL_STORAGE,
   Type=Network.GetGameConfigurationSaveType(),Directory=SaveDirectories.DEFAULT,
   IsAutosave=false,IsQuicksave=false},ServerType.SERVER_TYPE_HOTSEAT)
  assert(ret~=false,'Network.LoadGame rejected fixed save')
 end)
 if not ok then failed=true;emit('FAIL',r,'FRONTEND_LOAD',err) end
 return true
end,true)
-- If include triggered the original Show synchronously, adopt only the
-- construction request from this same call stack, never a later stale cache.
if constructed and not ContextPtr:IsHidden() and UI.IsInFrontEnd() then
 arm(constructed);publishReady();ContextPtr:SetUpdate(OnUpdate)
end
constructed=nil
