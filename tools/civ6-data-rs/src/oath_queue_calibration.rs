//! A diagnostic-only, finite queue experiment. The 66 oath cases do not consume this verdict.
#[cfg(test)]
use super::LUA_HELPERS;
use super::{first_steps, ui};
use anyhow::{Context, Result};
use serde_json::{json, Value};

pub(super) fn modifier_sql() -> &'static str {
    "-- Queue calibration is attached explicitly and scoped to one owned city plot.\n\
INSERT INTO Requirements (RequirementId,RequirementType) VALUES ('JAC_TODO134_QCAL_INPUT_512_REQ','REQUIREMENT_PLOT_PROPERTY_MATCHES'),('JAC_TODO134_QCAL_GROWTH_REQ','REQUIREMENT_PLOT_PROPERTY_MATCHES'),('JAC_TODO134_QCAL_P100_REQ','REQUIREMENT_PLOT_PROPERTY_MATCHES'),('JAC_TODO134_QCAL_P1_REQ','REQUIREMENT_PLOT_PROPERTY_MATCHES'),('JAC_TODO134_QCAL_P15_REQ','REQUIREMENT_PLOT_PROPERTY_MATCHES');\n\
INSERT INTO RequirementArguments (RequirementId,Name,Value) VALUES ('JAC_TODO134_QCAL_INPUT_512_REQ','PropertyName','JAC_TODO134_QCAL_INPUT_512_ON'),('JAC_TODO134_QCAL_INPUT_512_REQ','PropertyMinimum','1'),('JAC_TODO134_QCAL_GROWTH_REQ','PropertyName','JAC_TODO134_QCAL_GROWTH_ON'),('JAC_TODO134_QCAL_GROWTH_REQ','PropertyMinimum','1'),('JAC_TODO134_QCAL_P100_REQ','PropertyName','JAC_TODO134_QCAL_P100_ON'),('JAC_TODO134_QCAL_P100_REQ','PropertyMinimum','1'),('JAC_TODO134_QCAL_P1_REQ','PropertyName','JAC_TODO134_QCAL_P1_ON'),('JAC_TODO134_QCAL_P1_REQ','PropertyMinimum','1'),('JAC_TODO134_QCAL_P15_REQ','PropertyName','JAC_TODO134_QCAL_P15_ON'),('JAC_TODO134_QCAL_P15_REQ','PropertyMinimum','1');\n\
INSERT INTO RequirementSets (RequirementSetId,RequirementSetType) VALUES ('JAC_TODO134_QCAL_INPUT_512_SET','REQUIREMENTSET_TEST_ALL'),('JAC_TODO134_QCAL_GROWTH_SET','REQUIREMENTSET_TEST_ALL'),('JAC_TODO134_QCAL_P100_SET','REQUIREMENTSET_TEST_ALL'),('JAC_TODO134_QCAL_P1_SET','REQUIREMENTSET_TEST_ALL'),('JAC_TODO134_QCAL_P15_SET','REQUIREMENTSET_TEST_ALL');\n\
INSERT INTO RequirementSetRequirements (RequirementSetId,RequirementId) VALUES ('JAC_TODO134_QCAL_INPUT_512_SET','JAC_TODO134_QCAL_INPUT_512_REQ'),('JAC_TODO134_QCAL_GROWTH_SET','JAC_TODO134_QCAL_GROWTH_REQ'),('JAC_TODO134_QCAL_P100_SET','JAC_TODO134_QCAL_P100_REQ'),('JAC_TODO134_QCAL_P1_SET','JAC_TODO134_QCAL_P1_REQ'),('JAC_TODO134_QCAL_P15_SET','JAC_TODO134_QCAL_P15_REQ');\n\
INSERT INTO Modifiers (ModifierId,ModifierType,SubjectRequirementSetId) VALUES ('JAC_TODO134_QCAL_INPUT_512','MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_CHANGE','JAC_TODO134_QCAL_INPUT_512_SET'),('JAC_TODO134_QCAL_GROWTH','MODIFIER_PLAYER_CITIES_ADJUST_CITY_GROWTH','JAC_TODO134_QCAL_GROWTH_SET'),('JAC_TODO134_QCAL_P100','MODIFIER_PLAYER_CITIES_ADJUST_MILITARY_UNITS_PRODUCTION','JAC_TODO134_QCAL_P100_SET'),('JAC_TODO134_QCAL_P1','MODIFIER_PLAYER_CITIES_ADJUST_MILITARY_UNITS_PRODUCTION','JAC_TODO134_QCAL_P1_SET'),('JAC_TODO134_QCAL_P15','MODIFIER_PLAYER_CITIES_ADJUST_MILITARY_UNITS_PRODUCTION','JAC_TODO134_QCAL_P15_SET');\n\
INSERT INTO ModifierArguments (ModifierId,Name,Value) VALUES ('JAC_TODO134_QCAL_INPUT_512','YieldType','YIELD_PRODUCTION'),('JAC_TODO134_QCAL_INPUT_512','Amount','512'),('JAC_TODO134_QCAL_GROWTH','Amount','-1000'),('JAC_TODO134_QCAL_P100','Amount','100'),('JAC_TODO134_QCAL_P1','Amount','1'),('JAC_TODO134_QCAL_P15','Amount','15');\n\
-- Exact-quarter control uses the same modifier family and independent flag.\n\
INSERT INTO Requirements (RequirementId,RequirementType) SELECT 'JAC_TODO134_QCAL_P25_REQ',RequirementType FROM Requirements WHERE RequirementId='JAC_TODO134_QCAL_P1_REQ';\n\
INSERT INTO RequirementArguments (RequirementId,Name,Value) SELECT 'JAC_TODO134_QCAL_P25_REQ',Name,REPLACE(Value,'JAC_TODO134_QCAL_P1_ON','JAC_TODO134_QCAL_P25_ON') FROM RequirementArguments WHERE RequirementId='JAC_TODO134_QCAL_P1_REQ';\n\
INSERT INTO RequirementSets (RequirementSetId,RequirementSetType) SELECT 'JAC_TODO134_QCAL_P25_SET',RequirementSetType FROM RequirementSets WHERE RequirementSetId='JAC_TODO134_QCAL_P1_SET';\n\
INSERT INTO RequirementSetRequirements (RequirementSetId,RequirementId) VALUES ('JAC_TODO134_QCAL_P25_SET','JAC_TODO134_QCAL_P25_REQ');\n\
INSERT INTO Modifiers (ModifierId,ModifierType,SubjectRequirementSetId) SELECT 'JAC_TODO134_QCAL_P25',ModifierType,'JAC_TODO134_QCAL_P25_SET' FROM Modifiers WHERE ModifierId='JAC_TODO134_QCAL_P1';\n\
INSERT INTO ModifierArguments (ModifierId,Name,Value) VALUES ('JAC_TODO134_QCAL_P25','Amount','25');\n"
}

const SETUP: &str = r#"
assert(GameConfiguration.GetValue('JAC_TODO134_QUEUE_CALIBRATION')==1 or GameConfiguration.GetValue('JAC_TODO134_QUEUE_CALIBRATION')==true,'hidden single-player calibration opt-in missing')
assert(GameConfiguration.GetValue('JAC_TODO134_QUEUE_HEADROOM')~=1,'66-oath headroom must stay separate')
local unit=assert(GameInfo.Units.UNIT_SPEARMAN,'spearman definition missing')
assert(unit.Cost==131072,'calibration-only spear cost was not loaded')
if unit.PrereqTech then Players[0]:GetTechs():SetTech(assert(GameInfo.Technologies[unit.PrereqTech]).Index,true) end
local player=assert(Players[0]); assert(player:IsHuman() and player:IsTurnActive(),'human player is not active')
assert(not Game:GetProperty('JAC_TODO134_QCAL_ATTACHED'),'calibration already attached')
for _,id in ipairs({'JAC_TODO134_HEADROOM_PRODUCTION','JAC_TODO134_QCAL_INPUT_512','JAC_TODO134_QCAL_GROWTH','JAC_TODO134_QCAL_P100','JAC_TODO134_QCAL_P1','JAC_TODO134_QCAL_P15','JAC_TODO134_QCAL_P25'}) do player:AttachModifierByID(id) end
for _,city in player:GetCities():Members() do
 local plot=assert(Map.GetPlot(city:GetX(),city:GetY()))
 assert(city:GetOwner()==0 and plot:GetOwner()==0,'calibration owner changed')
 for _,key in ipairs({'JAC_TODO134_HEADROOM_ON','JAC_TODO134_QCAL_INPUT_512_ON','JAC_TODO134_QCAL_GROWTH_ON','JAC_TODO134_QCAL_P100_ON','JAC_TODO134_QCAL_P1_ON','JAC_TODO134_QCAL_P15_ON','JAC_TODO134_QCAL_P25_ON'}) do plot:SetProperty(key,0) end
end
Game:SetProperty('JAC_TODO134_QCAL_ATTACHED',true)
-- Game properties are writable only in GameCore. Publish the candidate before
-- the UI validates its native queue and requests production.
local chosen=nil; local smallest=math.huge
for _,city in player:GetCities():Members() do
 local production=number(city:GetYield(GameInfo.Yields.YIELD_PRODUCTION.Index),'candidate core production')
 if production>0 and production<smallest then chosen=city; smallest=production end
end
assert(chosen,'no owned city has positive production for calibration')
local plot=assert(Map.GetPlot(chosen:GetX(),chosen:GetY()))
Game:SetProperty('JAC_TODO134_QCAL_ID',{city=chosen:GetID(),plot=plot:GetIndex(),owner=0,unit_hash=unit.Hash})
return 'calibration-only military modifiers attached but off'
"#;

const REQUEST: &str = r#"
assert(Game:GetProperty('JAC_TODO134_QCAL_ATTACHED'),'calibration fixture missing')
assert(Players[0]:IsTurnActive() and Game.GetLocalPlayer()==0,'human queue request requires active player 0')
local def=assert(GameInfo.Units.UNIT_SPEARMAN)
local params={[CityOperationTypes.PARAM_UNIT_TYPE]=def.Hash,[CityOperationTypes.PARAM_INSERT_MODE]=CityOperationTypes.VALUE_REPLACE_AT,[CityOperationTypes.PARAM_QUEUE_DESTINATION_LOCATION]=0}
local id=assert(Game:GetProperty('JAC_TODO134_QCAL_ID'),'core calibration identity missing')
local chosen=assert(Players[0]:GetCities():FindID(id.city),'core candidate city missing in UI')
local plot=assert(Map.GetPlot(chosen:GetX(),chosen:GetY()))
assert(chosen:GetOwner()==id.owner and plot:GetOwner()==id.owner and plot:GetIndex()==id.plot and def.Hash==id.unit_hash,'core/UI candidate identity changed')
assert(CityManager.CanStartOperation(chosen,CityOperationTypes.BUILD,params,true),'core candidate cannot start a spearman')
local q=assert(chosen:GetBuildQueue())
local progress=number(q:GetUnitProgress(def.Index),'candidate spear progress')
local cost=number(q:GetUnitCost(def.Index),'candidate spear cost')
local production=number(chosen:GetYield(GameInfo.Yields.YIELD_PRODUCTION.Index),'candidate displayed production')
assert(production>0 and cost-progress>110000,'core candidate lacks 38-turn completion headroom')
JAC_QUEUE_CAL={city=id.city,plot=id.plot,owner=id.owner,unit_hash=id.unit_hash,cost=nil,samples={},diagnostic_only=true}
CityManager.RequestOperation(chosen,CityOperationTypes.BUILD,params)
return 'single spearman queue requested for city '..chosen:GetID()
"#;

const PHASE: &str = r#"
local state={index=__INDEX__,input=__INPUT__,percent=__PERCENT__,rep=__REP__,slot=__SLOT__}
local id=assert(Game:GetProperty('JAC_TODO134_QCAL_ID'),'calibration identity missing')
local city=assert(Players[0]:GetCities():FindID(id.city),'calibration city missing')
local plot=assert(Map.GetPlotByIndex(id.plot),'calibration plot missing')
assert(city:GetOwner()==0 and plot:GetOwner()==0 and Map.GetPlot(city:GetX(),city:GetY()):GetIndex()==id.plot,'calibration city/owner/plot changed')
assert((state.input==512 or state.input==4096) and (state.slot=='warmup' or state.slot=='off_before' or state.slot=='on' or state.slot=='off_after'),'invalid finite phase')
assert(state.slot=='warmup' or state.percent==100 or state.percent==1 or state.percent==15 or state.percent==25,'invalid military percent')
for _,key in ipairs({'JAC_TODO134_HEADROOM_ON','JAC_TODO134_QCAL_INPUT_512_ON','JAC_TODO134_QCAL_P100_ON','JAC_TODO134_QCAL_P1_ON','JAC_TODO134_QCAL_P15_ON','JAC_TODO134_QCAL_P25_ON'}) do plot:SetProperty(key,0) end
plot:SetProperty('JAC_TODO134_QCAL_GROWTH_ON',1)
plot:SetProperty(state.input==512 and 'JAC_TODO134_QCAL_INPUT_512_ON' or 'JAC_TODO134_HEADROOM_ON',1)
if state.slot=='on' then plot:SetProperty('JAC_TODO134_QCAL_P'..state.percent..'_ON',1) end
Game:SetProperty('JAC_TODO134_QCAL_STATE',state)
return string.format('calibration phase %d input=%d percent=%d rep=%d %s',state.index,state.input,state.percent,state.rep,state.slot)
"#;

const CORE: &str = r#"
assert(Players[0]:IsTurnActive() and Players[0]:IsTurnActiveComplete(),'core turn activation is not complete')
local state=assert(Game:GetProperty('JAC_TODO134_QCAL_STATE'),'phase missing')
assert(state.index==__INDEX__,'core phase index changed')
local id=assert(Game:GetProperty('JAC_TODO134_QCAL_ID'))
local city=assert(Players[0]:GetCities():FindID(id.city))
local plot=assert(Map.GetPlotByIndex(id.plot))
assert(city:GetOwner()==id.owner and plot:GetOwner()==id.owner and Map.GetPlot(city:GetX(),city:GetY()):GetIndex()==id.plot,'core calibration city changed')
-- The queue getter is authoritative in InGame. Core queue APIs may be absent;
-- record them when available without inventing a cross-context quantity basis.
local queue_ok,q=pcall(function()return city:GetBuildQueue() end)
if not queue_ok then q=nil end
local unit=assert(GameInfo.Units.UNIT_SPEARMAN)
local ok,nonfood=pcall(function()return city:GetGrowth():GetHappinessNonFoodYieldModifier() end)
local row={index=state.index,stage=__STAGE__,turn=Game.GetCurrentGameTurn(),city=id.city,plot=id.plot,owner=id.owner,activation_complete=Players[0]:IsTurnActiveComplete(),production=number(city:GetYield(GameInfo.Yields.YIELD_PRODUCTION.Index),'core displayed production'),nonfood_ok=ok and finite(nonfood),nonfood=ok and finite(nonfood) and nonfood or nil}
local hash_ok,hash=pcall(function()return q:GetCurrentProductionTypeHash() end)
local progress_ok,progress=pcall(function()return q:GetUnitProgress(unit.Index) end)
local cost_ok,cost=pcall(function()return q:GetUnitCost(unit.Index) end)
row.queue_hash=hash_ok and type(hash)=='number' and hash or nil
row.queue_progress=progress_ok and finite(progress) and progress or nil
row.queue_cost=cost_ok and finite(cost) and cost or nil
Game:SetProperty('JAC_TODO134_QCAL_CORE',row)
return string.format('{"context":"GameCore_Tuner","index":%d,"stage":%s,"turn":%d,"city":%d,"plot":%d,"activation_complete":%s,"production":%.9f,"happiness_nonfood":%s,"queue_hash":%s,"queue_progress":%s,"queue_cost":%s}',row.index,jstr(row.stage),row.turn,row.city,row.plot,tostring(row.activation_complete),row.production,row.nonfood and string.format('%.9f',row.nonfood) or 'null',row.queue_hash and string.format('%.0f',row.queue_hash) or 'null',row.queue_progress and string.format('%.9f',row.queue_progress) or 'null',row.queue_cost and string.format('%.9f',row.queue_cost) or 'null')
"#;

const OBSERVE: &str = r#"
assert(Players[0]:IsTurnActive() and Players[0]:IsTurnActiveComplete() and UI.CanEndTurn() and not UI.HasSentTurnComplete() and not UI.IsProcessingMessages(),'UI turn activation is not complete')
local data=assert(JAC_QUEUE_CAL,'calibration UI state missing')
local state=assert(Game:GetProperty('JAC_TODO134_QCAL_STATE'),'phase missing')
assert(state.index==__INDEX__,'UI phase index changed')
local core=assert(Game:GetProperty('JAC_TODO134_QCAL_CORE'),'matching core snapshot missing')
local stage=__STAGE__
assert(core.index==state.index and core.stage==stage and core.turn==Game.GetCurrentGameTurn(),'stale core snapshot')
assert(core.city==data.city and core.plot==data.plot and core.owner==data.owner,'core/UI city identity differs')
local city=assert(Players[0]:GetCities():FindID(data.city),'calibration city missing in UI')
local plot=assert(Map.GetPlotByIndex(data.plot))
assert(city:GetOwner()==data.owner and plot:GetOwner()==data.owner and Map.GetPlot(city:GetX(),city:GetY()):GetIndex()==data.plot,'UI owner or plot changed')
local q=assert(city:GetBuildQueue())
local unit=assert(GameInfo.Units.UNIT_SPEARMAN)
assert(q:GetCurrentProductionTypeHash()==data.unit_hash,'spear queue completed or changed')
local progress=number(q:GetUnitProgress(unit.Index),'actual spear progress')
local cost=number(q:GetUnitCost(unit.Index),'actual spear cost')
assert(progress>=0 and cost>progress,'military queue completed or has no headroom')
if data.cost then assert(math.abs(cost-data.cost)<0.00002,'calibration cost changed') else data.cost=cost end
local yield=number(city:GetYield(GameInfo.Yields.YIELD_PRODUCTION.Index),'UI displayed production')
local ok,nonfood=pcall(function()return city:GetGrowth():GetHappinessNonFoodYieldModifier() end)
local diagnostic={production=yield,nonfood_ok=ok and finite(nonfood),nonfood=ok and finite(nonfood) and nonfood or nil,core=core}
local row=data.samples[state.index]
if stage=='before' then
 assert(row==nil,'calibration sample overwritten')
 row={index=state.index,input=state.input,percent=state.percent,rep=state.rep,slot=state.slot,turn=Game.GetCurrentGameTurn(),city=data.city,plot=data.plot,owner=data.owner,cost=cost,before=progress,before_diagnostic=diagnostic,requested=false}
 data.samples[state.index]=row
else
 row=assert(row,'before observation missing')
 assert(row.requested and row.turn+1==Game.GetCurrentGameTurn(),'native request must return to human after exactly one turn')
 assert(Players[0]:IsTurnActive(),'human control not restored')
 assert(row.input==state.input and row.percent==state.percent and row.rep==state.rep and row.slot==state.slot,'phase changed during turn')
 assert(cost==row.cost and progress>row.before and progress<cost,'actual progress missing or queue completed')
 row.after=progress; row.delta=progress-row.before; row.after_diagnostic=diagnostic
end
return string.format('{"context":"InGame","index":%d,"stage":%s,"turn":%d,"input":%d,"percent":%d,"rep":%d,"slot":%s,"city":%d,"plot":%d,"activation_complete":true,"can_end":true,"turn_sent":false,"processing":false,"progress":%.9f,"cost":%.9f,"ui_production":%.9f,"core_production":%.9f,"ui_happiness_nonfood":%s,"core_happiness_nonfood":%s}',state.index,jstr(stage),Game.GetCurrentGameTurn(),state.input,state.percent,state.rep,jstr(state.slot),data.city,data.plot,progress,cost,yield,core.production,diagnostic.nonfood and string.format('%.9f',diagnostic.nonfood) or 'null',core.nonfood and string.format('%.9f',core.nonfood) or 'null')
"#;

const END_TURN: &str = r#"
local row=assert(assert(JAC_QUEUE_CAL).samples[__INDEX__],'actual before observation missing')
assert(row.turn==Game.GetCurrentGameTurn() and Players[0]:IsTurnActive(),'human turn changed before request')
assert(not row.requested,'duplicate human end-turn request')
row.requested=true
UI.RequestAction(ActionTypes.ACTION_ENDTURN,{REASON='UserForced'})
return 'one native human end-turn requested for calibration sample '..__INDEX__
"#;

// Official ActionPanel distinguishes activation completion from an end-turn
// request. An active player alone is insufficient for a production snapshot.
const READY_CORE: &str =
    "local p=Players[0]; return p and p:IsTurnActive() and p:IsTurnActiveComplete()";
const READY_UI: &str = "local p=Players[0]; return Game.GetLocalPlayer()==0 and p and p:IsTurnActive() and p:IsTurnActiveComplete() and UI.CanEndTurn() and not UI.HasSentTurnComplete() and not UI.IsProcessingMessages()";

fn ready_steps(steps: &mut Vec<Value>) {
    for (context, code) in [("GameCore_Tuner", READY_CORE), ("InGame", READY_UI)] {
        steps.push(json!({"action":"wait_for","timeout":180,"close_diplomacy":true,"condition":{"type":"lua","state_name":context,"code":code}}));
    }
}

// The verdict reads only GetUnitProgress before/after differences. GetYield and
// happiness snapshots remain evidence, never a denominator or expected delta.
const COMPARE: &str = r#"
local data=assert(JAC_QUEUE_CAL,'calibration samples missing')
assert(#data.samples==38,'every bounded calibration turn is required')
local previous=nil; local buckets={[512]={},[4096]={}}; local readings={[512]={},[4096]={}}
local percent_order={100,__CONTROL_PERCENT__,15}; local slot_order={'off_before','on','off_after'}
for index,row in ipairs(data.samples) do
 number(row.before,'progress before'); number(row.after,'progress after'); number(row.delta,'progress delta'); number(row.cost,'queue cost')
 assert(row.index==index and row.requested and row.city==data.city and row.plot==data.plot and row.owner==data.owner,'calibration identity or request changed')
 local position=(index-1)%19+1
 local expected_input=index<=19 and 512 or 4096
 local expected_percent,expected_rep,expected_slot=0,0,'warmup'
 if position>1 then
  local offset=position-2
  expected_percent=percent_order[math.floor(offset/6)+1]
  expected_rep=math.floor((offset%6)/3)+1
  expected_slot=slot_order[offset%3+1]
 end
 assert(row.input==expected_input and row.percent==expected_percent and row.rep==expected_rep and row.slot==expected_slot,'paired input or OFF/ON/OFF phase order changed')
 assert(row.cost==data.cost and row.before>=0 and row.after<row.cost and row.delta>0 and math.abs(row.after-row.before-row.delta)<0.00002,'invalid queue progress/cost')
 if previous then assert(row.turn==previous.turn+1 and math.abs(row.before-previous.after)<0.00002,'queue/turn discontinuity') end
 previous=row
 local list=readings[row.input]; assert(list,'unexpected input'); list[#list+1]=row.before; list[#list+1]=row.after
 if row.slot~='warmup' then
  local by_percent=buckets[row.input]; by_percent[row.percent]=by_percent[row.percent] or {}
  local reps=by_percent[row.percent]; reps[row.rep]=reps[row.rep] or {}
  assert(reps[row.rep][row.slot]==nil,'duplicate calibration phase')
  reps[row.rep][row.slot]=row.delta
 end
end
local outcomes={}; local established=true
for _,input in ipairs({512,4096}) do
 local step=reported_step(readings[input]); local off={}; local effects={[100]={},[__CONTROL_PERCENT__]={},[15]={}}
 local complete=step~=nil and finite(step) and step>0
 for _,percent in ipairs({100,__CONTROL_PERCENT__,15}) do
  local reps=buckets[input][percent]
  for rep=1,2 do
   local group=reps and reps[rep]
   if not group or not finite(group.off_before) or not finite(group.on) or not finite(group.off_after) then complete=false
   else
    off[#off+1]=group.off_before; off[#off+1]=group.off_after
    effects[percent][rep]=group.on-(group.off_before+group.off_after)/2
   end
  end
 end
 local restored=complete; local repeated=complete; local proportional=complete
 if complete then
  for _,delta in ipairs(off) do if math.abs(delta-off[1])>step+0.00002 then restored=false end end
  for _,percent in ipairs({100,__CONTROL_PERCENT__,15}) do
   if math.abs(effects[percent][1]-effects[percent][2])>2*step+0.00002 then repeated=false end
  end
 end
 local reference=complete and (effects[100][1]+effects[100][2])/2 or nil
 local width=reference and reference>0 and 2*step/reference or nil
 local precise=width~=nil and width<0.005
 if precise then
  for _,percent in ipairs({__CONTROL_PERCENT__,15}) do
   for rep=1,2 do if math.abs(effects[percent][rep]/reference-percent/100)>width+0.00000001 then proportional=false end end
  end
 else proportional=false end
 local valid=complete and restored and repeated and precise and proportional
 established=established and valid
 outcomes[#outcomes+1]=string.format('{"input":%d,"reference_delta":%s,"reported_step_upper_bound":%s,"two_step_width":%s,"off_restored":%s,"repeats_consistent":%s,"proportional___CONTROL_PERCENT___and_15":%s,"calibration_established":%s}',input,reference and string.format('%.9f',reference) or 'null',step and string.format('%.9f',step) or 'null',width and string.format('%.9f',width) or 'null',tostring(restored),tostring(repeated),tostring(proportional),tostring(valid))
end
data.calibration_established=established
return '{"diagnostic_only":true,"accepted":false,"calibration_established":'..tostring(established)..',"source":"GetUnitProgress before/after only","same_city":'..data.city..',"same_plot":'..data.plot..',"samples":38,"inputs":['..table.concat(outcomes,',')..']}'
"#;

const CLEANUP: &str = r#"
local id=assert(Game:GetProperty('JAC_TODO134_QCAL_ID'))
local city=assert(Players[0]:GetCities():FindID(id.city))
local plot=assert(Map.GetPlot(city:GetX(),city:GetY()))
assert(plot:GetIndex()==id.plot and city:GetOwner()==0 and plot:GetOwner()==0,'cleanup owner/plot changed')
for _,key in ipairs({'JAC_TODO134_HEADROOM_ON','JAC_TODO134_QCAL_INPUT_512_ON','JAC_TODO134_QCAL_GROWTH_ON','JAC_TODO134_QCAL_P100_ON','JAC_TODO134_QCAL_P1_ON','JAC_TODO134_QCAL_P15_ON','JAC_TODO134_QCAL_P25_ON'}) do plot:SetProperty(key,0) end
return 'calibration flags restored to zero; diagnostic verdict remains observation only'
"#;

fn phase_code(index: usize, input: i64, percent: i64, rep: usize, slot: &str) -> String {
    PHASE
        .replace("__INDEX__", &index.to_string())
        .replace("__INPUT__", &input.to_string())
        .replace("__PERCENT__", &percent.to_string())
        .replace("__REP__", &rep.to_string())
        .replace(
            "__SLOT__",
            &serde_json::to_string(slot).expect("slot serializes"),
        )
}

fn observation_steps(
    steps: &mut Vec<Value>,
    index: usize,
    input: i64,
    percent: i64,
    rep: usize,
    slot: &str,
) {
    steps.push(ui(
        "GameCore_Tuner",
        phase_code(index, input, percent, rep, slot),
    ));
    for stage in ["before", "after"] {
        ready_steps(steps);
        let core = CORE.replace("__INDEX__", &index.to_string()).replace(
            "__STAGE__",
            &serde_json::to_string(stage).expect("stage serializes"),
        );
        let observe = OBSERVE.replace("__INDEX__", &index.to_string()).replace(
            "__STAGE__",
            &serde_json::to_string(stage).expect("stage serializes"),
        );
        if stage == "before" {
            steps.push(ui("GameCore_Tuner", core));
            steps.push(ui("InGame", observe));
            steps.push(ui(
                "InGame",
                END_TURN.replace("__INDEX__", &index.to_string()),
            ));
            steps.push(json!({"action":"wait_for","timeout":180,"close_diplomacy":true,"condition":{"type":"lua","state_name":"InGame","code":format!("local d=JAC_QUEUE_CAL; local r=d and d.samples[{index}]; if not r then return false end; local turn=Game.GetCurrentGameTurn(); assert(turn<=r.turn+1,'calibration skipped a turn'); return r.requested and turn==r.turn+1 and Players[0]:IsTurnActive()")}}));
        } else {
            steps.push(ui("GameCore_Tuner", core));
            steps.push(ui("InGame", observe));
        }
    }
}

pub(super) fn scenario(base: &Value) -> Result<Value> {
    scenario_with_control(base, 1)
}

pub(super) fn quarter_control_scenario(base: &Value) -> Result<Value> {
    scenario_with_control(base, 25)
}

fn compare_code(control: i64) -> String {
    COMPARE.replace("__CONTROL_PERCENT__", &control.to_string())
}

fn scenario_with_control(base: &Value, control: i64) -> Result<Value> {
    assert!(control == 1 || control == 25);
    let mut steps = first_steps(base, "CIVILIZATION_JAC_ODA")?;
    let setup = steps[2]["code"]
        .as_str()
        .context("base setup Lua")?
        .to_owned();
    steps[2]["code"] = json!(format!("GameConfiguration.SetValue('JAC_TODO134_QUEUE_CALIBRATION',1); GameConfiguration.SetValue('JAC_TODO134_QUEUE_HEADROOM',0); {setup}"));
    steps.push(json!({"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"InGame","code":"return Game.GetLocalPlayer()==0 and Players[0] and Players[0]:IsTurnActive()"}}));
    steps.push(ui("GameCore_Tuner", SETUP.to_owned()));
    steps.push(ui("InGame", REQUEST.to_owned()));
    steps.push(json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"local d=JAC_QUEUE_CAL; local c=d and Players[0]:GetCities():FindID(d.city); return c and c:GetBuildQueue():GetCurrentProductionTypeHash()==GameInfo.Units.UNIT_SPEARMAN.Hash"}}));
    steps.push(json!({"action":"save","name":"JAC-Todo134-QueueCalibration","timeout":180}));
    let mut index = 0;
    for input in [512, 4096] {
        index += 1;
        observation_steps(&mut steps, index, input, 0, 0, "warmup");
        for percent in [100, control, 15] {
            for rep in 1..=2 {
                for slot in ["off_before", "on", "off_after"] {
                    index += 1;
                    observation_steps(&mut steps, index, input, percent, rep, slot);
                }
            }
        }
    }
    assert_eq!(index, 38);
    steps.push(ui("InGame", compare_code(control)));
    steps.push(ui("GameCore_Tuner", CLEANUP.to_owned()));
    Ok(
        json!({"name":format!("軍事建造の実進捗校正：同じ保存・都市・槍兵キューで512/4096、100%/{control}%/15%を反復"),"judge_profile":"generic","diagnostic_only":true,"finite_turns":38,"control_percent":control,"steps":steps}),
    )
}

#[cfg(test)]
#[path = "oath_queue_calibration_tests.rs"]
mod tests;
