-- A separate engine double for the attacker 0 / defender AI1 / source AI2 path.
ENEMY_CASES={}
local E=ENEMY_CASES
E.leader='LEADER_JAC_ASHINA_MORIKIYO'
E.base='JAC_PERSONA_ASHINA_MORIKIYO_BASE'
E.skill=E.leader..'_SIGNATURE_1'

function E.setup()
 Game:SetProperty('JAC_BOUNDARY_CONFIG',nil)
 GameInfo.JAC_LeaderPersonas=collection({
  {LeaderType=LEADER,GovernorType='GOV_TEST',PersonType='GP_TEST',BasePromotionType='BASE_TEST',EvolutionCost=1},
  {LeaderType=E.leader,GovernorType='GOV_ENEMY',PersonType='GP_ENEMY',BasePromotionType='BASE_ENEMY',EvolutionCost=1}},'LeaderType')
 GameInfo.Governors=collection({
  {GovernorType='GOV_TEST',Hash=9000,Index=0},
  {GovernorType='GOV_ENEMY',Hash=9001,Index=1}},'GovernorType')
 GameInfo.GovernorPromotions.BASE_ENEMY={Hash=7001}
 GameInfo.JAC_PersonaSkills=collection({{LeaderType=E.leader,ModifierId=E.base}},'ModifierId')
 GameInfo.JAC_PersonaTreeNodes=collection({{LeaderType=E.leader,
  NodeType=E.skill,ModifierId='JAC_ENEMY_SIGNATURE',Cost=8,
  CategoryType='ADMIN',RouteType='SIGNATURE'}},'NodeType')
 GameInfo.JAC_PersonaTitleCivics=collection({{CivicType='CIVIC_FIXTURE'}},'CivicType')
 GameInfo.Civics.CIVIC_FIXTURE={Index=7}
 PlayerConfigurations={
  [0]={GetLeaderTypeName=function() return 'LEADER_JAC_HOJO_UJITSUNA' end},
  [1]={GetLeaderTypeName=function() return 'LEADER_JAC_DATE_TANEMUNE' end},
  [2]={GetLeaderTypeName=function() return E.leader end}}
 Players[2]={IsAlive=function() return true end,IsHuman=function() return false end}
 Players[2].GetCities=function() return {Members=function() return function() end end} end
 ENEMY_POINTS,ENEMY_SPENT,ENEMY_GOV,ENEMY_SETTLED,ENEMY_EFFECT=0,0,nil,0,false
 ENEMY_ON_SITE=0
 local governors={
  HasGovernor=function(_,hash) return hash==9001 and ENEMY_GOV~=nil end,
  GetGovernor=function(_,hash) assert(hash==9001); return ENEMY_GOV end,
  CanEverAppointGovernor=function(_,hash) return hash==9001 end,
  CanAppointGovernor=function(_,hash)
   return hash==9001 and ENEMY_GOV==nil and ENEMY_POINTS-ENEMY_SPENT>=1
  end,
  GetGovernorPoints=function() return ENEMY_POINTS end,
  GetGovernorPointsSpent=function() return ENEMY_SPENT end,
  CanAssignGovernor=function(_,hash,city) return hash==9001 and city==FOREIGN and ENEMY_GOV~=nil end}
 Players[1].GetGovernors=function() return governors end
 -- This doubles the existing product UI bridge. It reads UI-owned state even
 -- while fireCore temporarily removes every per-type getter from Gameplay.
 ExposedMembers.JAC_LeaderPersonaNativeRead=function(owner,leader)
  assert(owner==1 and leader==E.leader)
  local value={appointed=ENEMY_GOV~=nil,
   canAppoint=ENEMY_GOV==nil and ENEMY_POINTS-ENEMY_SPENT>=1}
  if ENEMY_GOV then
   value.base=ENEMY_GOV.base; value.neutralized=ENEMY_GOV.neutralized
   value.established=ENEMY_GOV.established; value.surchargePaid=0
   if ENEMY_GOV.assigned then
    value.cityOwner=ENEMY_GOV.assigned:GetOwner()
    value.cityID=ENEMY_GOV.assigned:GetID()
   end
  end
  return value
 end
 local civic=false
 Players[1].GetCulture=function() return {
  HasCivic=function(_,index) assert(index==7); return civic end,
  GetCultureCost=function(_,index) assert(index==7); return 100 end,
  SetCulturalProgress=function(_,index,amount)
   assert(index==7 and amount==100 and not civic); civic=true; ENEMY_POINTS=ENEMY_POINTS+1
  end}
 end
 ExposedMembers.JAC_LeaderPersona={
  Join=function(owner,source)
   assert(owner==1 and source==2)
   Game:SetProperty('JAC_PERSONA_'..E.leader..'_OWNER',1)
   Game:SetProperty('JAC_PERSONA_'..E.leader..'_SOURCE',2)
   Game:SetProperty('JAC_PERSONA_'..E.leader..'_STATE','GREAT_PERSON')
   Game:SetProperty('JAC_PERSONA_'..E.leader..'_MERIT',6)
   return 'JOINED'
  end,
  PrepareEvolution=function(owner,leader)
   assert(owner==1 and leader==E.leader and ENEMY_POINTS-ENEMY_SPENT>=1)
   Game:SetProperty('JAC_PERSONA_'..E.leader..'_STATE','EVOLVING')
   return 'EVOLVING'
  end,
  CompleteEvolution=function(owner,leader)
   assert(owner==1 and leader==E.leader)
   Game:SetProperty('JAC_PERSONA_'..E.leader..'_STATE','GOVERNOR')
   return 'EVOLVED'
  end}
 FOREIGN.GetProperty=function(_,key)
  if key=='JAC_PERSONA_SETTLED_'..E.leader then return ENEMY_SETTLED end
 end
 GameEffects.GetModifiers=function() return {300,301} end
 GameEffects.GetModifierDefinition=function(id)
  return {Id=id==300 and E.base or 'JAC_PERSONA_SETTLED_'..E.leader}
 end
 GameEffects.GetModifierOwner=function() return {owner=1,name='Ashina Governor'} end
 GameEffects.GetModifierActive=function() return ENEMY_EFFECT end
 GameEffects.GetModifierSubjects=function() return {FOREIGN} end
 PlayerManager.GetAliveMajorIDs=function() return {0,1,2} end
 Players[2].GetDiplomacy=function() return {GetMission=function() return 0 end} end
end

function E.fireCore(name,params,controller)
 local original=Players[1].GetGovernors
 Players[1].GetGovernors=function()
  return {GetGovernorPoints=function() return ENEMY_POINTS end,
   GetGovernorPointsSpent=function() return ENEMY_SPENT end}
 end
 local core=Players[1]:GetGovernors()
 assert(core.HasGovernor==nil and core.CanEverAppointGovernor==nil and
  core.CanAppointGovernor==nil and core.GetGovernor==nil and
  core.CanAssignGovernor==nil, 'test leaked UI-only Governor getter into Gameplay')
 local ok,err=pcall(fire,assert(GameEvents[name]),controller or 0,params)
 Players[1].GetGovernors=original
 if not ok then error(err) end
end

function E.appoint(assigned)
 ENEMY_GOV={assigned=assigned and FOREIGN or nil,neutralized=0,established=false,
  onSite=0,type=1,base=true}
 ENEMY_GOV.GetType=function() return ENEMY_GOV.type end
 ENEMY_GOV.GetAssignedCity=function() return ENEMY_GOV.assigned end
 ENEMY_GOV.GetNeutralizedTurns=function() return ENEMY_GOV.neutralized end
 ENEMY_GOV.IsEstablished=function() return ENEMY_GOV.established end
 ENEMY_GOV.HasPromotion=function(_,hash) assert(hash==7001); return ENEMY_GOV.base end
 ENEMY_GOV.GetTurnsToEstablish=function() return 5 end
 ENEMY_GOV.GetTurnsOnSite=function() return ENEMY_GOV.onSite end
 ENEMY_SPENT=1
end
function E.bind()
 error('use the UI scalar request and synchronized Gameplay barrier to bind')
end
function E.settled()
 ENEMY_GOV.assigned=FOREIGN; ENEMY_GOV.established=true; ENEMY_GOV.onSite=5
 ENEMY_SETTLED=1; ENEMY_EFFECT=true
end
function E.mission(initial,plot,id,escape)
 HISTORY[0][id]={Operation=0,PlotIndex=plot,CompletionTurn=TURN,
  InitialResult=initial,EscapeResult=escape}
 Game:SetProperty('JAC_BOUNDARY_ENEMY_MISSION',{attacker=0,id=id,turn=TURN})
end
function E.neutralize(turns)
 ENEMY_GOV.assigned=nil; ENEMY_GOV.established=false; ENEMY_GOV.onSite=0
 ENEMY_GOV.neutralized=turns; ENEMY_SETTLED=0; ENEMY_EFFECT=false
end
function E.reassign(onSite)
 ENEMY_GOV.assigned=FOREIGN; ENEMY_GOV.neutralized=0
 ENEMY_GOV.onSite=onSite; ENEMY_GOV.established=onSite>=5
 ENEMY_SETTLED=onSite>=5 and 1 or 0; ENEMY_EFFECT=onSite>=5
end
