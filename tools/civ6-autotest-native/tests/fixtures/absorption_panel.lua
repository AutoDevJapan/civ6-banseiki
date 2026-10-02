-- Civ6 panel controls and operation bus for executing the actual UI script.
local function control(id)
  local c={id=id,hidden=false,disabled=false,text='',children={}}
  function c:SetHide(value) self.hidden=value end
  function c:IsHidden() return self.hidden end
  function c:SetDisabled(value) self.disabled=value end
  function c:IsDisabled() return self.disabled end
  function c:SetText(value) self.text=tostring(value) end
  function c:GetText() return self.text end
  function c:SetToolTipString(value) self.tooltip=value end
  function c:SetIcon(value) self.icon=value;return true end
  function c:RegisterCallback(_,fn) self.callback=fn end
  function c:DoLeftClick() assert(not self.disabled and self.callback,'control cannot be clicked: '..id);return self.callback() end
  function c:SetScrollValue(value) self.scroll=value end
  function c:SetSizeY(value) self.sizeY=value end
  function c:CalculateSize() end
  function c:CalculateInternalSize() end
  function c:GetChildren() return self.children end
  function c:GetID() return self.id end
  return c
end

Controls=setmetatable({}, {__index=function(t,key)
  local value=control(key);rawset(t,key,value);return value
end})
Controls.Panel:SetHide(true)
Controls.OpenButton:SetHide(true)

InstanceManager={}
function InstanceManager:new(kind)
  local manager={instances={}}
  if kind=='AbsorptionEntry' then TEST_ENTRY_MANAGER=manager
  else TEST_ACTION_MANAGER=manager end
  function manager:ResetInstances() self.instances={} end
  function manager:GetInstance()
    local row={Root=control('Root'),Focus=control('Focus'),Name=control('Name'),Choose=control('Choose')}
    if kind=='AbsorptionEntry' then
      row.Identity=control('Identity');row.Abilities=control('Abilities')
      row.Outcome=control('Outcome');row.Portrait=control('Portrait')
    end
    self.instances[#self.instances+1]=row
    return row
  end
  return manager
end

Locale={Lookup=function(tag,...)
  local args={...};for i,value in ipairs(args) do args[i]=tostring(value) end
  return tostring(tag)..(#args>0 and '('..table.concat(args,',')..')' or '')
end}
for id,config in pairs(PlayerConfigurations) do
  config.GetCivilizationDescription=function() return 'LOC_TEST_CIV_'..id end
end
for id,player in pairs(Players) do player.IsTurnActive=function() return id==2 end end
PlayerManager.GetAlive=function()
  local result={};for id=0,3 do if Players[id]:IsAlive() then result[#result+1]=Players[id] end end
  return result
end

Mouse={eLClick=1}
KeyEvents={KeyUp=1}
Keys={VK_ESCAPE=27,VK_TAB=9,VK_DOWN=40,VK_RIGHT=39,VK_UP=38,VK_LEFT=37,VK_RETURN=13}
ContextPtr={hidden=false,SetInputHandler=function(self,fn) self.input=fn end,
  IsHidden=function(self) return self.hidden end}
PlayerOperations={EXECUTE_SCRIPT=1}
OPERATION_CALLS={}
UI={RequestPlayerOperation=function(actor,operation,params)
  assert(operation==PlayerOperations.EXECUTE_SCRIPT)
  OPERATION_CALLS[#OPERATION_CALLS+1]={Actor=actor,Params=params}
  if OPERATION_HANDLER then return OPERATION_HANDLER(actor,params) end
  return true
end}
local function event()
  local handlers={}
  return {Add=function(fn) handlers[#handlers+1]=fn end,
    Emit=function(...) for _,fn in ipairs(handlers) do fn(...) end end}
end
Events={GameCoreEventPublishComplete=event(),LocalPlayerTurnBegin=event(),
  LocalPlayerTurnEnd=event(),LoadScreenClose=event()}
