LEADER='LEADER_JAC_ODA_NOBUHIDE'
SKILL=LEADER..'_SIGNATURE_1'
SETTLED='JAC_PERSONA_SETTLED_'..LEADER
BASE='JAC_PERSONA_ODA_NOBUHIDE_BASE'
NODE='JAC_PERSONA_TREE_'..SKILL
TURN=5
LOGS, REQUESTS, UNIT_REQUESTS={}, {}, {}
function print(...) local a={...}; LOGS[#LOGS+1]=table.concat(a,' ') end
function include() end
function event() local e={}; e.Add=function(f) e[#e+1]=f end; return e end
Events,GameEvents={},{}
for _,k in ipairs({'GameCoreEventPublishComplete','LocalPlayerTurnBegin','LoadScreenClose','SpyMissionCompleted'}) do Events[k]=event() end
for _,k in ipairs({'JAC_BoundaryPrepare','JAC_BoundaryLoseCity','JAC_BoundarySpawnSpy'}) do GameEvents[k]=event() end
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
 GetCurrentGameTurn=function() return TURN end,GetLocalPlayer=function() return 0 end}
GameInfo={}
GameInfo.JAC_LeaderPersonas=collection({{LeaderType=LEADER,GovernorType='GOV_TEST',PersonType='GP_TEST',BasePromotionType='BASE_TEST',EvolutionCost=1}},'LeaderType')
GameInfo.Governors=collection({{GovernorType='GOV_TEST',Hash=9000,Index=0}},'GovernorType')
GameInfo.GovernorPromotions={BASE_TEST={Hash=7000}}
GameInfo.JAC_PersonaSkills=collection({{LeaderType=LEADER,ModifierId=BASE}},'ModifierId')
GameInfo.JAC_PersonaTreeNodes=collection({{LeaderType=LEADER,NodeType=SKILL,ModifierId=NODE,CategoryType='ADMIN',RouteType='SIGNATURE'}},'NodeType')
GameInfo.GreatPersonClasses={GREAT_PERSON_CLASS_JAC_PERSONA={UnitType='UNIT_PERSONA'}}
GameInfo.Units={UNIT_PERSONA={Index=99},UNIT_SPY={Index=55}}
GameInfo.GreatPersonIndividuals={[3]={GreatPersonIndividualType='GP_TEST'}}
GameInfo.Yields=collection({{YieldType='YIELD_GOLD',Index=0},{YieldType='YIELD_FOOD',Index=1}},'YieldType')
GameInfo.UnitOperations=collection({{OperationType='UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR',Hash=990,Index=0}},'OperationType')
UnitOperationTypes={SPY_TRAVEL_NEW_CITY=991,PARAM_X='x',PARAM_Y='y'}
EspionageResultTypes={SUCCESS_UNDETECTED=1,SUCCESS_MUST_ESCAPE=2,FAIL_UNDETECTED=3,CAPTURED=4,NO_RESULT=0}
PlayerOperations={EXECUTE_SCRIPT=1,ASSIGN_GOVERNOR=2,PARAM_GOVERNOR_TYPE='governor',PARAM_PLAYER_ONE='owner',PARAM_CITY_DEST='city'}
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
 CITIES[#CITIES+1]=c; return c
end
CONTROL=city(10,0,1,0,'Control',0)
TARGET=city(11,0,2,0,'Target',1)
FOREIGN=city(20,1,2,1,'Foreign',0)
PLOTS={}
for _,c in ipairs(CITIES) do
 local index=c.x+c.y*10
 PLOTS[c.x..'/'..c.y]={GetIndex=function() return index end,GetProperty=function(_,k) if k==NODE then return 1 end end}
end
Map={GetPlot=function(x,y) return PLOTS[x..'/'..y] end}
Cities={GetPlotPurchaseCity=function(plot) for _,c in ipairs(CITIES) do if Map.GetPlot(c.x,c.y)==plot then return c end end end}
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
Players={}
for _,id in ipairs({0,1,62}) do
 local p={}
 p.IsHuman=function() return id==0 end; p.IsAlive=function() return true end
 p.IsTurnActive=function() return ACTIVE_TURN end
 p.GetGovernors=function() return {
  HasGovernor=function(_,hash) assert(hash==9000); return id==0 and APPOINTED end,
  GetGovernor=function(_,hash) assert(hash==9000); if id==0 then return GOV end end,
  GetGovernorPoints=function() return 1 end,GetGovernorPointsSpent=function() return 1 end,
  CanAssignGovernor=function(_,hash,c) return GOV.neutralized==0 and c.owner==id end} end
 p.GetCities=function() return {
  Members=function() local owned={}; for _,c in ipairs(CITIES) do if c.owner==id then owned[#owned+1]=c end end; return members(owned) end,
  GetCapitalCity=function() if id==0 then return CONTROL elseif id==1 then return FOREIGN end end} end
 p.GetUnits=function() return {Members=function() return members(UNITS[id]) end,FindID=function(_,uid) for _,u in ipairs(UNITS[id]) do if u:GetID()==uid then return u end end end} end
 p.GetDiplomacy=function() return {GetMission=function(_,attacker,mid) assert(attacker==id); return HISTORY[id][mid] or 0 end} end
 Players[id]=p
end
PlayerManager={GetAliveMajorIDs=function() return {0,1} end,GetFreeCitiesPlayerID=function() return 62 end}
CityManager={GetCity=function(owner,id) for _,c in ipairs(CITIES) do if c.owner==owner and c.id==id then return c end end end,
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
UI={IsGameCoreBusy=function() return BUSY or false end,
 RequestPlayerOperation=function(owner,op,params) REQUESTS[#REQUESTS+1]={owner=owner,op=op,params=params} end}
CAN_START=true
UnitManager={
 CanStartOperation=function(spy,op,plot,a,b) QUERY={spy=spy,op=op,plot=plot,a=a,b=b}; return CAN_START,{failure={'native reason'}} end,
 RequestOperation=function(spy,op,params) UNIT_REQUESTS[#UNIT_REQUESTS+1]={spy=spy,op=op,params=params} end,
 GetTimeToComplete=function(index) assert(index==0); return 8 end,
 GetResultProbability=function() return {ESPIONAGE_SUCCESS_UNDETECTED=.25,ESPIONAGE_SUCCESS_MUST_ESCAPE=.20} end,
 GetTravelTime=function() return 3 end,GetEstablishInCityTime=function() return 4 end,
 InitUnitValidAdjacentHex=function(owner,typ,x,y,radius)
  assert(typ=='UNIT_SPY' and owner==0 and x==1 and y==0 and radius==1); makeSpy(owner,15)
 end}
function makeSpy(owner,id)
 local spy={x=2,y=1,op=-1,finish=TURN,owner=owner,id=id}
 spy.GetID=function() return spy.id end; spy.GetOwner=function() return spy.owner end
 spy.GetType=function() return 55 end; spy.GetX=function() return spy.x end; spy.GetY=function() return spy.y end
 spy.GetSpyOperation=function() return spy.op end; spy.GetSpyOperationEndTurn=function() return spy.finish end
 UNITS[owner][#UNITS[owner]+1]=spy; return spy
end
function stop(remaining)
 GOV.assigned=nil; GOV.established=false; GOV.neutralized=remaining; TARGET.marker=0; EFFECT_TARGET=false
end
function lose()
 stop(0); TARGET.owner=62
 props.JAC_BOUNDARY_CONFIG.transfer={fromOwner=0,fromCityID=11,toOwner=62,toCityID=11,x=2,y=0}
end
