-- Lua 5.1 executable fixture for the test MOD. Native state is simulated;
-- production Lua is loaded unchanged by the Rust tests.
TURN, BUSY, ACCEPT = 0, false, true
REQUESTS, JOIN_CALLS, LOGS = {}, 0, {}
local properties = {}
Game = {
  GetCurrentGameTurn=function() return TURN end,
  GetLocalPlayer=function() return 0 end,
  GetProperty=function(_,key) return properties[key] end,
  SetProperty=function(_,key,value)
    assert(CONTEXT ~= 'UI', 'UI must not write Game properties')
    properties[key]=value
  end
}
GameConfiguration = {IsAnyMultiplayer=function() return false end}
function print(message) LOGS[#LOGS+1]=message end
local function collection(rows,key)
  local result={}
  for _,row in ipairs(rows) do
    result[row[key]]=row
    if row.Index then result[row.Index]=row end
  end
  return setmetatable(result,{__call=function()
    local index=0
    return function() index=index+1; return rows[index] end
  end})
end
GameInfo={}
local people,governors,civics,titles = {},{},{},{}
for i=1,22 do
  local leader=i==4 and 'LEADER_JAC_ODA_NOBUHIDE' or 'LEADER_JAC_TEST_'..i
  local kind='GOVERNOR_JAC_PERSONA_'..i
  people[i]={LeaderType=leader,GovernorType=kind,PersonType='PERSON_'..i,
    EvolutionCost=i==4 and 1 or 2}
  governors[i]={GovernorType=kind,Index=200+i,Hash=100+i,
    TraitType='TRAIT_JAC_TEST_ALL_PERSONAS_CLOSED'}
end
for i=1,3 do
  civics[i]={CivicType='CIVIC_TITLE_'..i}
  titles[i]={CivicType='CIVIC_TITLE_'..i,Index=i}
end
GameInfo.JAC_LeaderPersonas=collection(people,'LeaderType')
GameInfo.Governors=collection(governors,'GovernorType')
GameInfo.JAC_PersonaTitleCivics=collection(civics,'CivicType')
GameInfo.Civics=collection(titles,'CivicType')
GameInfo.GreatPersonClasses={GREAT_PERSON_CLASS_JAC_PERSONA={UnitType='UNIT_PERSON'}}
GameInfo.Units={UNIT_PERSON={Index=900}}
GameInfo.GreatPersonIndividuals={}
for i,row in ipairs(people) do GameInfo.GreatPersonIndividuals[i]={GreatPersonIndividualType=row.PersonType} end
Players,PlayerConfigurations,EVER = {},{},{}
for id=0,21 do
  local player={id=id,human=id==0,points=0,spent=0,appointed={},units={},civics={}}
  function player:GetID() return self.id end
  function player:IsHuman() return self.human end
  function player:IsAlive() return true end
  function player:IsTurnActive() return self.human end
  function player:GetGovernors() return self end
  function player:GetGovernorPoints() return self.points end
  function player:GetGovernorPointsSpent() return self.spent end
  function player:CanEverAppointGovernor(value) return EVER[self.id] and EVER[self.id][value] or false end
  function player:CanAppointGovernor(_) return self.points>self.spent end
  function player:HasGovernor(value) return self.appointed[value]~=nil end
  function player:GetGovernor(value) return self.appointed[value] end
  function player:GetCulture() return self end
  function player:HasCivic(value) return self.civics[value]==true end
  function player:GetCultureCost(_) return 100 end
  function player:SetCulturalProgress(value,_)
    if not self.civics[value] then self.civics[value]=true; self.points=self.points+1 end
  end
  function player:GetCities() return {GetCapitalCity=function() return {id=1} end} end
  function player:GetUnits() return {Members=function() return ipairs(self.units) end} end
  Players[id]=player
  local leader=people[id+1].LeaderType
  PlayerConfigurations[id]={GetLeaderTypeName=function() return leader end}
end
PlayerManager={
  GetAliveMajors=function()
    local rows={}; for id=0,21 do rows[#rows+1]=Players[id] end; return rows
  end,
  GetAliveMajorIDs=function()
    local rows={}; for id=0,21 do rows[#rows+1]=id end; return rows
  end
}
ExposedMembers={JAC_LeaderPersona={Join=function(owner,source)
  JOIN_CALLS=JOIN_CALLS+1
  assert(owner==0 and source==3)
  local prefix='JAC_PERSONA_LEADER_JAC_ODA_NOBUHIDE_'
  Game:SetProperty(prefix..'OWNER',owner)
  Game:SetProperty(prefix..'SOURCE',source)
  Game:SetProperty(prefix..'STATE','GREAT_PERSON')
  Game:SetProperty(prefix..'EVOLUTION_COST',1)
  local unit={}
  function unit:GetType() return 900 end
  function unit:GetGreatPerson() return {GetIndividual=function() return 4 end} end
  Players[owner].units[#Players[owner].units+1]=unit
  return 'JOINED'
end}}
PlayerOperations={EXECUTE_SCRIPT='EXECUTE_SCRIPT',APPOINT_GOVERNOR='APPOINT_GOVERNOR',
  PARAM_GOVERNOR_TYPE='PARAM_GOVERNOR_TYPE'}
UI={
  IsGameCoreBusy=function() return BUSY end,
  RequestPlayerOperation=function(owner,kind,params)
    REQUESTS[#REQUESTS+1]={owner=owner,kind=kind,params=params}
  end
}
ContextPtr={
  SetHide=function(_,hidden) assert(hidden==false) end,
  SetUpdate=function(_,callback) FRAME_CALLBACK=callback end
}
GameEvents={}
for _,name in ipairs({'JAC_StaticGateGiveAITitles','JAC_StaticGatePrepareHuman',
    'JAC_StaticGateBarrier'}) do
  local event={handlers={}}
  function event.Add(callback) event.handlers[#event.handlers+1]=callback end
  GameEvents[name]=event
end
function FRAME()
  CONTEXT='UI'; FRAME_CALLBACK(); CONTEXT=nil
end
function UI_CALL(callback)
  CONTEXT='UI'; local ok,value=pcall(callback); CONTEXT=nil
  if not ok then error(value) end
  return value
end
function FLUSH()
  while #REQUESTS>0 do
    local request=table.remove(REQUESTS,1)
    CONTEXT='GAME'
    if request.kind==PlayerOperations.EXECUTE_SCRIPT then
      local event=assert(GameEvents[request.params.OnStart])
      assert(#event.handlers==1)
      event.handlers[1](request.owner)
    elseif request.kind==PlayerOperations.APPOINT_GOVERNOR and ACCEPT then
      local def=GameInfo.Governors[request.params.PARAM_GOVERNOR_TYPE]
      local native={GetType=function() return def.Index end}
      Players[request.owner].appointed[def.Hash]=native
      Players[request.owner].spent=Players[request.owner].spent+1
    end
    CONTEXT=nil
  end
end
