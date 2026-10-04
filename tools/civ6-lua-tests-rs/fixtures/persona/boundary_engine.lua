LEADER='LEADER_JAC_ODA_NOBUHIDE'
SKILL=LEADER..'_SIGNATURE_1'
SETTLED='JAC_PERSONA_SETTLED_'..LEADER
BASE='JAC_PERSONA_ODA_NOBUHIDE_BASE'
NODE='JAC_PERSONA_TREE_'..SKILL
TURN=5
LOGS, REQUESTS, UNIT_REQUESTS, OFF_MAP={}, {}, {}, {}
function print(...) local a={...}; LOGS[#LOGS+1]=table.concat(a,' ') end
function include() end
function event() local e={}; e.Add=function(f) e[#e+1]=f end; return e end
LuaEvents={DiploPopup_DealUpdated=event()}
Events,GameEvents={},{}
for _,k in ipairs({'GameCoreEventPublishComplete','LocalPlayerTurnBegin','LoadScreenClose','SpyMissionCompleted',
 'CityProductionQueueChanged','UnitOperationStarted','DiplomacyIncomingDeal'}) do Events[k]=event() end
for _,k in ipairs({'JAC_BoundaryPrepare','JAC_BoundaryLoseCity','JAC_BoundarySpawnSpy',
 'JAC_BoundaryGameplayApiProbe',
 'JAC_BoundaryProgressSpy','JAC_BoundaryBindSpy','JAC_BoundarySpyBarrier',
 'JAC_BoundaryAdoptSpyIdentity','JAC_BoundarySaveSpyTravel','JAC_BoundaryPrepareEnemy',
 'JAC_BoundaryEnemyBarrier','JAC_BoundaryEvolveEnemy',
 'JAC_BoundaryEnemyFixtureBarrier','JAC_BoundaryConfirmEnemy',
 'JAC_BoundaryRecordEnemyMission','JAC_BoundaryCaptureEnemyRecovery',
 'JAC_BoundaryBeginEnemySpyRetry','JAC_BoundaryProgressCapacityBuilding',
 'JAC_BoundaryMarkCaptiveExchange','JAC_BoundaryPrepareCapacityCivic',
 'JAC_BoundaryReadCapacityCivic',
 'JAC_BoundaryPrepareTier2CapacityCivic','JAC_BoundaryReadTier2CapacityCivic',
 'JAC_BoundaryPrepareTier1GovernmentWindowCivic',
 'JAC_BoundaryReadTier1GovernmentWindowCivic',
 'JAC_BoundaryCapacityGovernmentIntent','JAC_BoundaryCapacityGovernmentRead',
 'JAC_BoundaryCapacityGovernmentProbe',
 'JAC_BoundaryProgressCapacityDistrict'}) do GameEvents[k]=event() end
function fire(e,...) for _,f in ipairs(e) do f(...) end end
function collection(rows,key)
 local r={}; for i,row in ipairs(rows) do r[i-1]=row; r[row[key]]=row; if row.Hash then r[row.Hash]=row end end
 return setmetatable(r,{__call=function() local i=0; return function() i=i+1; return rows[i] end end})
end
local props={JAC_BOUNDARY_CONFIG={owner=0,leader=LEADER,cityID=11,controlID=10,x=2,y=0,armedTurn=0,skill=SKILL,total=1,spent=0}}
props['JAC_PERSONA_'..LEADER..'_OWNER']=0
props['JAC_PERSONA_'..LEADER..'_STATE']='GOVERNOR'
props['JAC_PERSONA_'..LEADER..'_MERIT']=12
props['JAC_PERSONA_'..LEADER..'_NODE_'..SKILL]=1
WRITES={}
Game={GetProperty=function(_,k) return props[k] end,
 SetProperty=function(_,k,v) WRITES[#WRITES+1]=k; props[k]=v end,
 GetCurrentGameTurn=function() return TURN end,GetLocalPlayer=function() return 0 end,
 IsAllowStrategicCommands=function(owner) assert(owner==0); return GOV_STRATEGIC_COMMANDS end}
GameInfo={}
GameInfo.JAC_LeaderPersonas=collection({{LeaderType=LEADER,GovernorType='GOV_TEST',PersonType='GP_TEST',BasePromotionType='BASE_TEST',EvolutionCost=1}},'LeaderType')
GameInfo.Governors=collection({{GovernorType='GOV_TEST',Hash=9000,Index=0}},'GovernorType')
GameInfo.GovernorPromotions={BASE_TEST={Hash=7000}}
GameInfo.JAC_PersonaSkills=collection({{LeaderType=LEADER,ModifierId=BASE}},'ModifierId')
GameInfo.JAC_PersonaTreeNodes=collection({{LeaderType=LEADER,NodeType=SKILL,ModifierId=NODE,CategoryType='ADMIN',RouteType='SIGNATURE'}},'NodeType')
GameInfo.GreatPersonClasses={GREAT_PERSON_CLASS_JAC_PERSONA={UnitType='UNIT_PERSONA'}}
local spyDef={Index=55,Hash=555,UnitType='UNIT_SPY',Name='LOC_UNIT_SPY_NAME',PrereqCivic='CIVIC_JAC_SENGOKU_LOCAL_LEAGUES_AND_INTELLIGENCE',Spy=true}
GameInfo.Units={UNIT_PERSONA={Index=99},UNIT_SPY=spyDef,[55]=spyDef}
GameInfo.Civics={[spyDef.PrereqCivic]={Index=12},
 CIVIC_JAC_SENGOKU_DOMAIN_LAW={Index=13,CivicType='CIVIC_JAC_SENGOKU_DOMAIN_LAW'},
 CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS={Index=14,
  CivicType='CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS'},
 CIVIC_JAC_SENGOKU_WARRIOR_FARMER_SEPARATION={Index=15,
  CivicType='CIVIC_JAC_SENGOKU_WARRIOR_FARMER_SEPARATION'},
 CIVIC_JAC_SENGOKU_LEARNING_AND_RECORDS={Index=16,
  CivicType='CIVIC_JAC_SENGOKU_LEARNING_AND_RECORDS'},
 CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER={Index=17,
  CivicType='CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER'},
 CIVIC_JAC_SENGOKU_PASSES_AND_PASSAGE={Index=18,
  CivicType='CIVIC_JAC_SENGOKU_PASSES_AND_PASSAGE'},
 CIVIC_JAC_SENGOKU_HOUSE_RULES={Index=19,CivicType='CIVIC_JAC_SENGOKU_HOUSE_RULES'}}
GameInfo.CivicPrereqs=collection({
 {Civic='CIVIC_JAC_SENGOKU_LEARNING_AND_RECORDS',PrereqCivic='CIVIC_JAC_SENGOKU_DOMAIN_LAW'},
 {Civic='CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER',PrereqCivic='CIVIC_JAC_SENGOKU_DOMAIN_LAW'},
 {Civic='CIVIC_JAC_SENGOKU_PASSES_AND_PASSAGE',PrereqCivic='CIVIC_JAC_SENGOKU_HOUSE_RULES'},
 {Civic='CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS',PrereqCivic='CIVIC_JAC_SENGOKU_PASSES_AND_PASSAGE'}},'Civic')
GameInfo.CivicModifiers=collection({
 {CivicType='CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS',ModifierId='JAC_TEST_TITLE'},
 {CivicType='CIVIC_JAC_SENGOKU_WARRIOR_FARMER_SEPARATION',ModifierId='JAC_TEST_TITLE'},
 {CivicType='CIVIC_JAC_SENGOKU_LEARNING_AND_RECORDS',ModifierId='JAC_TEST_TITLE'},
 {CivicType='CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER',ModifierId='JAC_TEST_TITLE'}},'CivicType')
GameInfo.Modifiers={JAC_TEST_TITLE={ModifierType='JAC_TEST_TITLE_EFFECT'}}
GameInfo.DynamicModifiers={JAC_TEST_TITLE_EFFECT={EffectType='EFFECT_ADJUST_PLAYER_GOVERNOR_POINTS'}}
GameInfo.ModifierArguments=collection({{ModifierId='JAC_TEST_TITLE',Name='Delta',Value='1'}},'ModifierId')
GameInfo.GovernmentTiers=collection({{TierType='Tier1',Sorting=1},
 {TierType='Tier2',Sorting=2}},'TierType')
GameInfo.Governments=collection({
 {GovernmentType='GOVERNMENT_CHIEFDOM',Index=0},
 {GovernmentType='GOVERNMENT_OLIGARCHY',Index=1,Tier='Tier1',
  PrereqCivic='CIVIC_JAC_SENGOKU_DOMAIN_LAW'},
 {GovernmentType='GOVERNMENT_MERCHANT_REPUBLIC',Index=2,Tier='Tier2',
  PrereqCivic='CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS'},
 {GovernmentType='GOVERNMENT_MONARCHY',Index=3,Tier='Tier2',
  PrereqCivic='CIVIC_JAC_SENGOKU_WARRIOR_FARMER_SEPARATION'}},'GovernmentType')
GameInfo.Types={GOVERNMENT_OLIGARCHY={Hash=1001},
 GOVERNMENT_MERCHANT_REPUBLIC={Hash=1002},GOVERNMENT_MONARCHY={Hash=1003}}
GameInfo.Districts={DISTRICT_GOVERNMENT={DistrictType='DISTRICT_GOVERNMENT',Index=70,Hash=700,
 PrereqCivic='CIVIC_JAC_SENGOKU_DOMAIN_LAW'}}
GameInfo.Districts[70]=GameInfo.Districts.DISTRICT_GOVERNMENT
GameInfo.Buildings={}
for _,row in ipairs({
  {BuildingType='BUILDING_GOV_TALL',Index=76,Hash=760,GovernmentTierRequirement='Tier1'},
  {BuildingType='BUILDING_GOV_CONQUEST',Index=77,Hash=770,GovernmentTierRequirement='Tier1'},
  {BuildingType='BUILDING_GOV_WIDE',Index=78,Hash=780,GovernmentTierRequirement='Tier1'},
  {BuildingType='BUILDING_GOV_SPIES',Index=79,Hash=790,GovernmentTierRequirement='Tier2'}}) do
 row.PrereqDistrict='DISTRICT_GOVERNMENT'; row.RequiresPlacement=false
 GameInfo.Buildings[row.BuildingType]=row; GameInfo.Buildings[row.Index]=row
end
GameInfo.GreatPersonIndividuals={[3]={GreatPersonIndividualType='GP_TEST'}}
GameInfo.Yields=collection({{YieldType='YIELD_GOLD',Index=0},{YieldType='YIELD_FOOD',Index=1}},'YieldType')
GameInfo.UnitOperations=collection({
 {OperationType='UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR',Hash=990,Index=0},
 {OperationType='UNITOPERATION_SPY_TRAVEL_NEW_CITY',Hash=991,Index=1}},'OperationType')
UnitOperationTypes={SPY_TRAVEL_NEW_CITY=991,PARAM_X='x',PARAM_Y='y'}
InterfaceModeTypes={SELECTION=1,SPY_TRAVEL_TO_CITY=2,SPY_CHOOSE_MISSION=3}
ActivityTypes={ACTIVITY_AWAKE=1}
EspionageResultTypes={SUCCESS_UNDETECTED=1,SUCCESS_MUST_ESCAPE=2,FAIL_UNDETECTED=3,CAPTURED=4,NO_RESULT=0}
PlayerOperations={EXECUTE_SCRIPT=1,ASSIGN_GOVERNOR=2,APPOINT_GOVERNOR=3,
 PARAM_GOVERNOR_TYPE='governor',PARAM_PLAYER_ONE='owner',PARAM_CITY_DEST='city'}
CityOperationTypes={BUILD=81,PARAM_UNIT_TYPE='unit',PARAM_INSERT_MODE='insert',
 PARAM_BUILDING_TYPE='building',PARAM_DISTRICT_TYPE='district',PARAM_X='x',PARAM_Y='y',
 PARAM_QUEUE_DESTINATION_LOCATION='location',VALUE_REPLACE_AT=7,VALUE_EXCLUSIVE=8}
CityOperationResults={PLOTS='plots'}
CityProductionDirectives={TRAIN=1}
local function members(list)
 local i=0; return function() i=i+1; if list[i] then return i,list[i] end end
end
CITIES={}
function city(id,owner,x,y,name,marker)
 local c={id=id,owner=owner,x=x,y=y,name=name,marker=marker}
 c.GetID=function() return c.id end; c.GetOwner=function() return c.owner end
 c.GetX=function() return c.x end; c.GetY=function() return c.y end
 c.GetName=function() return c.name end
 c.GetProperty=function(_,k) if k==SETTLED then return c.marker end end
 c.GetYield=function(_,i) return i==0 and 13.5 or 8.25 end
 c.GetBuildQueue=function() return {GetAt=function() return nil end,
  CanProduce=function() return false end} end
 c.GetDistricts=function() return {HasDistrict=function() return false end} end
 c.GetBuildings=function() return {HasBuilding=function() return false end} end
 CITIES[#CITIES+1]=c; return c
end
CONTROL=city(10,0,1,0,'Control',0)
TARGET=city(11,0,2,0,'Target',1)
FOREIGN=city(20,1,2,1,'Foreign',0)
BUILD_HEAD=nil; PROGRESS_CALLS=0; CAPACITY_PROGRESS_CALLS=0; BUILD_REQUESTS={}; SPY_CAPACITY=1
GOV_DISTRICT_BUILT=false; GOV_DISTRICT_PLACED=false
GOV_DISTRICT_X=1; GOV_DISTRICT_Y=1
GOV_DISTRICT_OWNER=0; GOV_DISTRICT_CITY=CONTROL
GOV_DISTRICT_ID=991; GOV_DISTRICT_MEMBER_KEY=1
GOV_DISTRICT_MISSING_GETTER=nil
GOV_TIER1_BUILT=false; GOV_SPIES_BUILT=false; GOV_CIVIC_COMPLETED=false
GOV_TIER2_CIVIC_COMPLETED=true; GOV_MONARCHY_CIVIC_COMPLETED=false
GOV_WINDOW_LEARNING=false; GOV_WINDOW_MILITARY=false; GOV_WINDOW_PASSES=false
GOV_POINTS=1
GOV_CURRENT=2; GOV_CAN_CHANGE=true; GOV_CHANGE_MADE=false
GOV_CIVIC_COMPLETED_THIS_TURN=false; GOV_STRATEGIC_COMMANDS=true
GOV_REQUEST_CALLS=0
CAPTURED={[0]={},[1]={}}
local queue={
 GetAt=function(_,index) assert(index==0); return BUILD_HEAD end,
 CanProduce=function(_,hash)
  if hash==555 then return CIVIC_COMPLETED and SPY_CAPACITY>0 end
  if hash==700 then return GOV_CIVIC_COMPLETED and not GOV_DISTRICT_PLACED end
   local government=GameInfo.Governments[GOV_CURRENT]
   local tier=government and government.Tier and GameInfo.GovernmentTiers[government.Tier]
   local sorting=tier and tier.Sorting or 0
   if hash==760 or hash==770 or hash==780 then
    return GOV_DISTRICT_BUILT and not GOV_TIER1_BUILT and sorting>=1,
     sorting<1 and 'GOVERNMENT_TIER_REQUIREMENT_Tier1' or 'OTHER_PRODUCTION_GATE'
   end
   if hash==790 then
    return GOV_DISTRICT_BUILT and GOV_TIER1_BUILT and not GOV_SPIES_BUILT and sorting>=2,
     sorting<2 and 'GOVERNMENT_TIER_REQUIREMENT_Tier2' or 'OTHER_PRODUCTION_GATE'
   end
  return false
 end,
 AddProgress=function(_,amount)
  assert(BUILD_HEAD and amount==500)
  if BUILD_HEAD.UnitType then PROGRESS_CALLS=PROGRESS_CALLS+1
  else CAPACITY_PROGRESS_CALLS=CAPACITY_PROGRESS_CALLS+1 end
 end}
CONTROL.GetBuildQueue=function() return queue end
CONTROL.GetDistricts=function() return {
 HasDistrict=function(_,index,builtOnly)
  assert(index==70)
  if builtOnly then return GOV_DISTRICT_BUILT end
  return GOV_DISTRICT_PLACED
 end,
 Members=function()
  local instance={GetType=function() return GOV_DISTRICT_TYPE or 70 end,
   GetOwner=function() return GOV_DISTRICT_OWNER end,
   GetCity=function() return GOV_DISTRICT_CITY end,
   GetID=function() return GOV_DISTRICT_ID end,
   GetX=function() return GOV_DISTRICT_X end,
   GetY=function() return GOV_DISTRICT_Y end}
  if GOV_DISTRICT_MISSING_GETTER then instance[GOV_DISTRICT_MISSING_GETTER]=nil end
  local returned=false
  return function()
   if GOV_DISTRICT_PLACED and not returned then
    returned=true; return GOV_DISTRICT_MEMBER_KEY,instance
   end
  end
 end} end
CONTROL.GetBuildings=function() return {HasBuilding=function(_,index)
 if index==76 then return GOV_TIER1_BUILT end
 if index==77 or index==78 then return false end
 assert(index==79); return GOV_SPIES_BUILT
 end} end
function ADVANCE_TURN()
  TURN=TURN+1
  GOV_CIVIC_COMPLETED_THIS_TURN=false; GOV_CHANGE_MADE=false
 if BUILD_HEAD and BUILD_HEAD.DistrictType and CAPACITY_PROGRESS_CALLS>0 and
  CAPACITY_COMPLETES~=false then
  GOV_DISTRICT_BUILT=true; BUILD_HEAD=nil
 elseif BUILD_HEAD and BUILD_HEAD.BuildingType and CAPACITY_PROGRESS_CALLS>0 and
  CAPACITY_COMPLETES~=false then
  if BUILD_HEAD.BuildingType==76 then GOV_TIER1_BUILT=true
  elseif BUILD_HEAD.BuildingType==79 then GOV_SPIES_BUILT=true; SPY_CAPACITY=SPY_CAPACITY+1 end
  BUILD_HEAD=nil
 elseif PROGRESS_CALLS==1 and BUILD_HEAD and BUILD_HEAD.UnitType and PRODUCTION_COMPLETES~=false then
  local spy=makeSpy(0,PRODUCED_SPY_ID or 15); spy.x=CONTROL.x; spy.y=CONTROL.y; spy.finish=0
  BUILD_HEAD=nil
 end
end
PLOTS={}
for _,c in ipairs(CITIES) do
 local index=c.x+c.y*10
 PLOTS[c.x..'/'..c.y]={GetIndex=function() return index end,
  GetX=function() return c.x end,GetY=function() return c.y end,
  GetProperty=function(_,k) if k==NODE then return 1 end end}
end
PLOTS['1/1']={GetIndex=function() return 11 end,GetX=function() return 1 end,GetY=function() return 1 end}
PLOTS['1/2']={GetIndex=function() return 21 end,GetX=function() return 1 end,GetY=function() return 2 end}
Map={GetPlot=function(x,y) return PLOTS[x..'/'..y] end,
 GetPlotByIndex=function(index) for _,plot in pairs(PLOTS) do if plot:GetIndex()==index then return plot end end end}
Cities={GetPlotPurchaseCity=function(plot)
 if plot==PLOTS['1/1'] or plot==PLOTS['1/2'] then return CONTROL end
 for _,c in ipairs(CITIES) do if Map.GetPlot(c.x,c.y)==plot then return c end end
 end}
GOV={assigned=TARGET,established=true,neutralized=0,base=true,type=0}
GOV.GetType=function() return GOV.type end
GOV.GetAssignedCity=function() return GOV.assigned end
GOV.IsEstablished=function() return GOV.established end
GOV.GetNeutralizedTurns=function() return GOV.neutralized end
GOV.HasPromotion=function(_,h) assert(h==7000); return GOV.base end
APPOINTED=true
UNITS={[0]={},[1]={},[62]={}}
HISTORY={[0]={},[1]={}}
ACTIVE_TURN=true
CIVIC_COMPLETED=false
Players={}
for _,id in ipairs({0,1,62}) do
 local p={}
 p.GetID=function() return id end
 p.IsHuman=function() return id==0 end; p.IsAlive=function() return true end
 p.IsTurnActive=function() return ACTIVE_TURN end
 p.GetGovernors=function() return {
  HasGovernor=function(_,hash) assert(hash==9000); return id==0 and APPOINTED end,
  GetGovernor=function(_,hash) assert(hash==9000); if id==0 then return GOV end end,
   GetGovernorPoints=function() return GOV_POINTS end,GetGovernorPointsSpent=function() return 1 end,
  CanAssignGovernor=function(_,hash,c) return GOV.neutralized==0 and c.owner==id end} end
 p.GetCities=function() return {
  Members=function() local owned={}; for _,c in ipairs(CITIES) do if c.owner==id then owned[#owned+1]=c end end; return members(owned) end,
  GetCapitalCity=function() if id==0 then return CONTROL elseif id==1 then return FOREIGN end end} end
  p.GetUnits=function() return {Members=function() return members(UNITS[id]) end,FindID=function(_,uid) for _,u in ipairs(UNITS[id]) do if u:GetID()==uid then return u end end end} end
 p.GetCulture=function() return {HasCivic=function(_,index)
   if index==13 then return GOV_CIVIC_COMPLETED end
   if index==14 then return GOV_TIER2_CIVIC_COMPLETED end
   if index==15 then return GOV_MONARCHY_CIVIC_COMPLETED end
   if index==16 then return GOV_WINDOW_LEARNING end
   if index==17 then return GOV_WINDOW_MILITARY end
   if index==18 then return GOV_WINDOW_PASSES end
   if index==19 then return true end
   assert(index==12); return CIVIC_COMPLETED end,
  GetCultureCost=function(_,index) assert(index==12); return 100 end,
  SetCulturalProgress=function(_,index,amount) assert(index==12 and amount==100); CIVIC_COMPLETED=true; SPY_CAPACITY=0 end,
  SetCivic=function(_,index,grant)
   assert(grant==true)
    if index==13 then assert(not GOV_CIVIC_COMPLETED); GOV_CIVIC_COMPLETED=true; GOV_CIVIC_COMPLETED_THIS_TURN=true
    elseif index==14 then GOV_TIER2_CIVIC_COMPLETED=true; GOV_CIVIC_COMPLETED_THIS_TURN=true; GOV_POINTS=GOV_POINTS+1
    elseif index==15 then GOV_MONARCHY_CIVIC_COMPLETED=true; GOV_CIVIC_COMPLETED_THIS_TURN=true; GOV_POINTS=GOV_POINTS+1
    elseif index==16 then GOV_WINDOW_LEARNING=true; GOV_CIVIC_COMPLETED_THIS_TURN=true; GOV_POINTS=GOV_POINTS+1
    elseif index==17 then GOV_WINDOW_MILITARY=true; GOV_CIVIC_COMPLETED_THIS_TURN=true; GOV_POINTS=GOV_POINTS+1
    elseif index==18 then GOV_WINDOW_PASSES=true; GOV_CIVIC_COMPLETED_THIS_TURN=true
    else assert(index==12); CIVIC_COMPLETED=true; CIVIC_GRANT_CALLS=(CIVIC_GRANT_CALLS or 0)+1 end
   end,
   GetCurrentGovernment=function() return GOV_CURRENT end,
   IsGovernmentUnlocked=function(_,hash)
    if hash==1001 then return GOV_CIVIC_COMPLETED end
    if hash==1002 then return GOV_TIER2_CIVIC_COMPLETED end
    if hash==1003 then return GOV_MONARCHY_CIVIC_COMPLETED end
    return false
   end,
   CanChangeGovernmentAtAll=function() return GOV_CAN_CHANGE end,
   GovernmentChangeMade=function() return GOV_CHANGE_MADE end,
   CivicCompletedThisTurn=function() return GOV_CIVIC_COMPLETED_THIS_TURN end,
   GetAnarchyTurns=function() return GOV_ANARCHY_TURNS or 0 end,
   RequestChangeGovernment=function(self,hash)
    GOV_REQUEST_CALLS=GOV_REQUEST_CALLS+1
    if not GOV_CAN_CHANGE or GOV_CHANGE_MADE or not GOV_STRATEGIC_COMMANDS or
     not (GOV_CURRENT==-1 or GOV_CIVIC_COMPLETED_THIS_TURN) or
     not self:IsGovernmentUnlocked(hash) then return false end
    local target=hash==1001 and 1 or hash==1002 and 2 or hash==1003 and 3
    if not target then return false end
    GOV_CURRENT=target; GOV_CHANGE_MADE=true; return true
   end} end
 p.GetDiplomacy=function() return {GetMission=function(_,attacker,mid) assert(attacker==id); return HISTORY[id][mid] or 0 end,
  IsAtWarWith=function(_,other) assert(other~=id); return AT_WAR==true end,
  GetSpyCapacity=function() return CIVIC_COMPLETED and SPY_CAPACITY or 0 end,
  GetNumSpiesOffMap=function() return id==0 and #OFF_MAP or 0 end,
  GetNthOffMapSpy=function(_,owner,index) assert(owner==id); return id==0 and OFF_MAP[index+1] or nil end,
  GetNumSpiesCaptured=function() return #(CAPTURED[id] or {}) end,
  GetNthCapturedSpy=function(_,owner,index) assert(owner==id); return CAPTURED[id][index+1] end} end
 Players[id]=p
end
Game.GetPlayers=function() return {Players[0],Players[1]} end
PlayerManager={GetAliveMajorIDs=function() return {0,1} end,GetFreeCitiesPlayerID=function() return 62 end}
CityManager={GetCity=function(owner,id) for _,c in ipairs(CITIES) do if c.owner==owner and c.id==id then return c end end end,
 GetOperationTargets=function(city,op,params)
  assert(op==CityOperationTypes.BUILD and params.district==700)
  return {[CityOperationResults.PLOTS]=city==CONTROL and GOV_CIVIC_COMPLETED and not GOV_DISTRICT_PLACED and {11} or {}}
 end,
 CanStartOperation=function(city,op,params) assert(city==CONTROL and op==CityOperationTypes.BUILD and
  (params.unit==555 or params.building==760 or params.building==790 or
   (params.district==700 and params.x==1 and params.y==1 and params.insert==8)));
  return BUILD_CAN_START~=false,{reason='native build result'} end,
 RequestOperation=function(city,op,params)
  assert(city==CONTROL and op==CityOperationTypes.BUILD and
   ((params.unit==555 or params.building==760 or params.building==790) and
    params.insert==7 and params.location==0 or
    params.district==700 and params.x==1 and params.y==1 and params.insert==8))
  BUILD_REQUESTS[#BUILD_REQUESTS+1]={city=city,op=op,params=params,selected=SELECTED_CITY}
  if BUILD_ACCEPTED~=false then
   if params.unit then BUILD_HEAD={Directive=CityProductionDirectives.TRAIN,UnitType=55}
   elseif params.district then BUILD_HEAD={DistrictType=70}; GOV_DISTRICT_PLACED=true
   else BUILD_HEAD={BuildingType=params.building==760 and 76 or 79} end
   fire(Events.CityProductionQueueChanged,0,10) end
 end,
 TransferCityToFreeCities=function(c) assert(c==TARGET); TRANSFERRED=c; c.owner=62; c.marker=0; GOV.assigned=nil; GOV.established=false; EFFECT_TARGET=false end}
EFFECT_TARGET=true
GameEffects={GetModifiers=function() return {100,101,102} end,
 GetModifierDefinition=function(id) return {Id=({BASE,SETTLED,NODE})[id-99]} end,
 GetModifierOwner=function() return {owner=0,name='Native governor'} end,
 GetModifierActive=function() return EFFECT_TARGET end,
 GetModifierSubjects=function() return {TARGET} end,
 GetObjectsPlayerId=function(obj) return obj.owner end,GetObjectName=function(obj) return obj.name end}
Locale={Lookup=function(name) return name end}
ExposedMembers={JAC_LeaderPersonaNativeRead=function() error('persona UI bridge is not native evidence') end}
ExposedMembers.JAC_PersonaBoundaryGameplayRead=function()
 local result={merit=props['JAC_PERSONA_'..LEADER..'_MERIT'],freeCitiesOwner=62,cities={}}
 for _,c in ipairs(CITIES) do if c.owner==0 or c==TARGET then
  result.cities[c.x..'/'..c.y]={id=c.id,owner=c.owner,settled=c.marker,selectors={[NODE]=1}}
 end end
 return result
end
ContextPtr={SetHide=function() end,SetUpdate=function(_,fn) FRAME=fn end}
MODE=InterfaceModeTypes.SELECTION
UI={IsGameCoreBusy=function() return BUSY or false end,
 GetElapsedTime=function() return ELAPSED or 0 end,
 RequestPlayerOperation=function(owner,op,params) REQUESTS[#REQUESTS+1]={owner=owner,op=op,params=params} end,
 DeselectAllUnits=function() SELECTED=nil end,DeselectAllCities=function() end,
 SelectCity=function(city) SELECTED_CITY=city end,GetHeadSelectedCity=function() return SELECTED_CITY end,
 SelectUnit=function(spy) if not SELECT_FAIL then SELECTED=spy end end,
 GetHeadSelectedUnit=function() return SELECTED end,
 SetInterfaceMode=function(mode) if not MODE_FAIL then MODE=mode end end,
 GetInterfaceMode=function() return MODE end}
function ACK_BARRIER()
 local request=REQUESTS[#REQUESTS]
 assert(request and request.params.OnStart=='JAC_BoundarySpyBarrier')
 if #GameEvents.JAC_BoundarySpyBarrier>0 then
  fire(GameEvents.JAC_BoundarySpyBarrier,request.owner,request.params)
 else
  Game:SetProperty('JAC_BOUNDARY_SPY_BARRIER',{owner=request.owner,spyID=request.params.SpyID,
   token=request.params.Token,action=request.params.Action})
 end
 fire(Events.GameCoreEventPublishComplete)
end
function ACK_ADOPTION()
 local request=REQUESTS[#REQUESTS]
 assert(request and request.params.OnStart=='JAC_BoundaryAdoptSpyIdentity')
 if #GameEvents.JAC_BoundaryAdoptSpyIdentity>0 then
  fire(GameEvents.JAC_BoundaryAdoptSpyIdentity,request.owner,request.params)
 else
  Game:SetProperty('JAC_BOUNDARY_SPY',{owner=request.owner,id=request.params.NewID})
 end
 fire(Events.GameCoreEventPublishComplete)
end
function START_NATIVE(owner,id,operation) fire(Events.UnitOperationStarted,owner,id,operation) end
CAN_START=true
UnitManager={
 CanStartOperation=function(spy,op,plot,a,b) QUERY={spy=spy,op=op,plot=plot,a=a,b=b}; return CAN_START,{failure={'native reason'}} end,
 RequestOperation=function(spy,op,params)
  UNIT_REQUESTS[#UNIT_REQUESTS+1]={spy=spy,op=op,params=params,
   selected=SELECTED and SELECTED:GetID() or -1,mode=MODE}
  if REQUEST_EFFECT=='travel' or REQUEST_EFFECT=='different_spy_offmap' or
   REQUEST_EFFECT=='different_level_offmap' or
   REQUEST_EFFECT=='wrong_destination_offmap' or REQUEST_EFFECT=='expired_offmap' or
   REQUEST_EFFECT=='multi_offmap' then
   local name=REQUEST_EFFECT=='different_spy_offmap' and 'Other Spy' or spy.name
   local x=REQUEST_EFFECT=='wrong_destination_offmap' and TARGET.x or FOREIGN.x
   local y=REQUEST_EFFECT=='wrong_destination_offmap' and TARGET.y or FOREIGN.y
   local finish=REQUEST_EFFECT=='expired_offmap' and TURN or TURN+3
   local level=REQUEST_EFFECT=='different_level_offmap' and spy.level+1 or spy.level
   local slot=(#OFF_MAP==1 and OFF_MAP[1].ReturnTurn==-1) and 2 or 1
   OFF_MAP[slot]={Name=name,Level=level,ReturnTurn=finish,XLocation=x,YLocation=y}
   if REQUEST_EFFECT=='multi_offmap' then
    OFF_MAP[slot+1]={Name=spy.name,Level=spy.level,ReturnTurn=TURN+3,XLocation=FOREIGN.x,YLocation=FOREIGN.y}
   end
   for i,unit in ipairs(UNITS[spy.owner]) do if unit==spy then table.remove(UNITS[spy.owner],i); break end end
  elseif REQUEST_EFFECT=='arrive' then spy.x=FOREIGN.x; spy.y=FOREIGN.y; spy.finish=0
  elseif REQUEST_EFFECT=='arrive_new_id' then
   for i,unit in ipairs(UNITS[spy.owner]) do if unit==spy then table.remove(UNITS[spy.owner],i); break end end
   local arrived=makeSpy(spy.owner,16); arrived.name=spy.name; arrived.level=spy.level
   arrived.x=FOREIGN.x; arrived.y=FOREIGN.y; arrived.finish=0
  elseif REQUEST_EFFECT=='wrong_arrive' then spy.x=TARGET.x; spy.y=TARGET.y; spy.finish=TURN+3
  elseif REQUEST_EFFECT=='mission' then spy.op=0; spy.finish=TURN+8
  elseif REQUEST_EFFECT=='mission_hash' then spy.op=990; spy.finish=TURN+8
  elseif REQUEST_EFFECT=='mission_wrong' then spy.op=1; spy.finish=TURN+8 end
 end,
 GetActivityType=function() return ActivityTypes.ACTIVITY_AWAKE end,
 GetTimeToComplete=function(index) assert(index==0); return 8 end,
 GetResultProbability=function() return {ESPIONAGE_SUCCESS_UNDETECTED=.25,ESPIONAGE_SUCCESS_MUST_ESCAPE=.20} end,
 GetTravelTime=function() return 3 end,GetEstablishInCityTime=function() return 4 end,
 InitUnitValidAdjacentHex=function() error('generic spy creation is forbidden') end}
function makeSpy(owner,id)
 local spy={x=2,y=1,op=-1,finish=TURN,owner=owner,id=id,name='Spy '..id,level=1}
 spy.GetID=function() return spy.id end; spy.GetOwner=function() return spy.owner end
 spy.GetType=function() return 55 end; spy.GetUnitType=function() return 55 end
 spy.IsReadyToMove=function() return SPY_READY~=false end
 spy.GetX=function() return spy.x end; spy.GetY=function() return spy.y end
 spy.GetName=function() return spy.name end
 spy.GetExperience=function() return {GetLevel=function() return spy.level end} end
 spy.GetSpyOperation=function() return spy.op end; spy.GetSpyOperationEndTurn=function() return spy.finish end
 UNITS[owner][#UNITS[owner]+1]=spy; return spy
end
function ARRIVE_OFFMAP(id)
 local slot,travelling
 for i,record in ipairs(OFF_MAP) do if record.ReturnTurn~=-1 then
  assert(not slot); slot=i; travelling=record
 end end
 assert(slot); table.remove(OFF_MAP,slot)
 local spy=makeSpy(0,id)
 spy.name=travelling.Name=='' and spyDef.Name or travelling.Name; spy.level=travelling.Level
 spy.x=travelling.XLocation; spy.y=travelling.YLocation; spy.finish=0
 return spy
end
function stop(remaining)
 GOV.assigned=nil; GOV.established=false; GOV.neutralized=remaining; TARGET.marker=0; EFFECT_TARGET=false
end
function lose()
 stop(0); TARGET.owner=62
 props.JAC_BOUNDARY_CONFIG.transfer={fromOwner=0,fromCityID=11,toOwner=62,toCityID=11,x=2,y=0}
end
