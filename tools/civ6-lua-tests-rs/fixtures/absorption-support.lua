-- Lua only supplies scaffolding. EQ/TRUTH/ERROR call the Rust strict boundary.
function FORMAT(source, values)
  return (source:gsub('{(%w+)}',function(key) return assert(values[key]) end))
end
function CONTAINS(needle, source, expected)
  assert(type(needle)=='string' and type(source)=='string')
  assert((source:find(needle,1,true)~=nil)==expected)
end
function RELOAD()
  RUN(READ('poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Rules.lua'))
  include=function() end
  RUN(READ('poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua'))
  -- Python's Lupa exposes zero/one/multiple results as None/value/tuple.
  -- Wrap public calls used by the transposed scaffolding, keeping the actual
  -- ExposedMembers API unchanged for the verbatim MOD/Lua fragments.
  local function proxy(source)
    return setmetatable({}, {__index=function(_,key)
      local value=source[key]
      if type(value)=='function' then return function(...) return RAW_CALL(value,...) end end
      if key=='Rules' and type(value)=='table' then return proxy(value) end
      return value
    end})
  end
  A=proxy(ExposedMembers.JAC_Absorption)
end
function APPLY() return A.Apply(PLAN) end
function VERIFY(id,exact)
  if BOOL(A.RequestVerification(id)) then RUN(STRINGS['AbsorptionTests.verify'][1]) end
  return A.Verify(id,exact)
end
function SNAPSHOT() return RUN(STRINGS['AbsorptionTests.snapshot'][1]) end
function REJECT(code,message)
  RUN(code)
  local before=SNAPSHOT()
  ERROR(function() APPLY() end,message)
  EQ(function() return SNAPSHOT() end,function() return before end)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
end
function SUICIDE(cause,requested)
  local L=STRINGS['AbsorptionTests.assert_native_suicide']
  requested=requested or L[1]
  local tx=APPLY()
  EQ(function() return tx.Status end,function() return L[2] end)
  RUN(FORMAT(L[3],{cause=cause,requested=requested}))
  local before=SNAPSHOT()
  RELOAD()
  APPLY()
  EQ(function() return SNAPSHOT() end,function() return before end)
end
function FIND_SPREAD(units,id)
  for _,s in pairs(units) do if s.ID==id then return s.SpreadCharges end end
  error('missing spread-charge snapshot')
end
function SELECT_CODES(steps,needle)
  local result,n={},0
  for i=1,COUNT(steps) do
    local code=steps[i].code or ''
    if code:find(needle,1,true) then n=n+1;result[n]=code end
  end
  return result
end
function FIRST_CODE(steps,needle)
  for i=1,COUNT(steps) do
    local code=steps[i].code or ''
    if code:find(needle,1,true) then return code end
  end
  error('missing scenario code: '..needle)
end
function FIRST_CAPTURE_STEP(steps,name)
  for i=1,COUNT(steps) do if steps[i].capture_as==name then return steps[i] end end
  error('missing scenario capture: '..name)
end
function FIRST_CONDITION_STEP(steps,needle)
  for i=1,COUNT(steps) do
    local code=(steps[i].condition or {}).code or ''
    if code:find(needle,1,true) then return steps[i] end
  end
  error('missing scenario condition: '..needle)
end
function LIVE()
  local L=STRINGS['AbsorptionTests.live_scenario_fixture']
  local data=JSON(READ(L[1]))
  local result,n={},0
  for i=1,COUNT(data.steps) do
    local step=data.steps[i]
    if step.action=='ui_lua' and step.state_name=='GameCore_Tuner'
      and (step.capture_as or ''):sub(1,20)~='JAC_ABSORPTION_SYNC_' then
      n=n+1;result[n]=step
    end
  end
  RUN(L[11])
  return result
end
function HUMAN_FILTER(steps)
  local result,n={},0
  for i=1,COUNT(steps) do
    local step=steps[i]
    local code=step.code or ''
    local capture=step.capture_as or ''
    local condition=(step.condition or {}).code or ''
    local excluded=false
    for _,key in pairs({'JAC_ABSORPTION_FIXTURE_UI_WAIT','GetUIPhase','GetFaithBalance()>=1000000','JAC_ABSORPTION_RECALL_COST_V1'}) do
      if condition:find(key,1,true) then excluded=true end
    end
    if not code:find('recall spawn blockers relocated',1,true) and step.action~='autoplay'
      and capture:sub(1,20)~='JAC_ABSORPTION_SYNC_' and not excluded then
      n=n+1;result[n]=step
    end
  end
  return result
end
