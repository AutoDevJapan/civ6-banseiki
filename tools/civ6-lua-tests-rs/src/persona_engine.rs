//! Extend the archived engine mock with the normal startup/event contract.
//! This supplies engine definitions and success callbacks, never persona receipts.
use mlua::Lua;
mod cases;

pub fn update_cases(lua: &Lua) -> mlua::Result<()> {
    lua.load(cases::SOURCE)
        .set_name("normal-receipt-cases")
        .exec()
}

pub fn prepare(lua: &Lua) -> mlua::Result<()> {
    lua.load(ENGINE).set_name("normal-persona-engine").exec()
}

pub fn finish_startup(lua: &Lua) -> mlua::Result<()> {
    lua.load("assert(type(Game:GetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1'))=='table','normal fresh fixture failed'); TURN=NORMAL_FIXTURE_OLD_TURN; NORMAL_FIXTURE_OLD_TURN=nil")
        .exec()
}

const ENGINE: &str = r#"
NORMAL_FIXTURE_OLD_TURN=TURN; TURN=0
GameConfiguration=GameConfiguration or {}; GameConfiguration.GetStartTurn=function() return 0 end
for i=#ROWS+1,22 do
 local suffix='TEST_'..i
 local row={LeaderType='LEADER_JAC_'..suffix,CivilizationType='CIVILIZATION_JAC_'..suffix,
  PersonType='GREAT_PERSON_INDIVIDUAL_JAC_PERSONA_'..suffix,GovernorType='GOVERNOR_JAC_PERSONA_'..suffix,
  BasePromotionType='GOVERNOR_PROMOTION_JAC_PERSONA_'..suffix..'_BASE',EvolutionCost=1,
  Personality='CALM',Trait='ASTUTE',Name='TEST '..i,PortraitIcon='ICON_TEST'}
 ROWS[i]=row
 INDIVIDUALS[i]={GreatPersonIndividualType=row.PersonType,GreatPersonClassType='GREAT_PERSON_CLASS_JAC_PERSONA',
  Index=100+i,Hash=3000+i,EraType='ERA_ANCIENT',Name=row.Name}
 GOVDEFS[i]={GovernorType=row.GovernorType,Index=150+i,Hash=1000+i,Name=row.Name}
 PROMOS[#PROMOS+1]={GovernorPromotionType=row.BasePromotionType,Index=500+i,Hash=2000+i}
end
-- The private Ashina fixture occupies Hash 1004; added engine definitions
-- use their own disjoint range rather than colliding with that real subject.
for i=4,#GOVDEFS do GOVDEFS[i].Hash=10000+i end
if NORMAL_PERSONA_TEST_CASE=='test_rejected_second_native_payment_waits_and_resumes_without_double_charge' or
   NORMAL_PERSONA_TEST_CASE=='test_upper_cost_consumes_two_extra_titles_and_one_native_once' then
 ROWS[2].EvolutionCost=3
end
if NORMAL_PERSONA_TEST_CASE=='cost_three_needs_two_ordered_official_promotions' or
   NORMAL_PERSONA_TEST_CASE=='partial_native_payment_reserves_only_each_persons_unpaid_titles' then
 ROWS[1].EvolutionCost=3
end
if NORMAL_PERSONA_TEST_CASE=='test_pending_surcharge_survives_reload_and_spending_without_double_charge' then
 GOVDEFS[#GOVDEFS+1]={GovernorType='GOVERNOR_STOCK',Index=240,Hash=12400,Name='Stock'}
 PROMOS[#PROMOS+1]={GovernorPromotionType='STOCK_BASE',Index=4000,Hash=24000,BaseAbility=1}
end
local sets,seen={},{}
for _,p in ipairs(PROMOS) do if p.Index~=nil then assert(not seen[p.Index]); seen[p.Index]=true end end
local nextIndex=600
for _,row in ipairs(ROWS) do
 for _,p in ipairs(PROMOS) do
  if p.GovernorPromotionType==row.BasePromotionType or
     p.GovernorPromotionType:sub(1,#row.BasePromotionType+7)==row.BasePromotionType..'_TITLE_' then
   if p.Index==nil then while seen[nextIndex] do nextIndex=nextIndex+1 end; p.Index=nextIndex; seen[nextIndex]=true end
   p.BaseAbility=p.GovernorPromotionType==row.BasePromotionType and 1 or 0
   sets[#sets+1]={GovernorType=row.GovernorType,GovernorPromotion=p.GovernorPromotionType}
  end
 end
end
if GameInfo and NORMAL_PERSONA_TEST_CASE=='test_pending_surcharge_survives_reload_and_spending_without_double_charge' then
 sets[#sets+1]={GovernorType='GOVERNOR_STOCK',GovernorPromotion='STOCK_BASE'}
end
GameInfo.JAC_LeaderPersonas=collection(ROWS,'LeaderType')
GameInfo.GreatPersonIndividuals=collection(INDIVIDUALS,'GreatPersonIndividualType')
GameInfo.Governors=collection(GOVDEFS,'GovernorType')
GameInfo.GovernorPromotions=collection(PROMOS,'GovernorPromotionType')
GameInfo.GovernorPromotionSets=setmetatable({},{__call=function() local i=0; return function() i=i+1; return sets[i] end end})
for id=3,21 do
 local owner=id; local row=ROWS[id+1]
 local cfg={leader=row.LeaderType,civ=row.CivilizationType}
 cfg.GetLeaderTypeName=function() return cfg.leader end; cfg.GetCivilizationTypeName=function() return cfg.civ end
 PlayerConfigurations[owner]=cfg
 NATIVE_UNITS[owner],NATIVE_GOVS[owner],GRANTS[owner],CITIES[owner],NATIVE_APPOINTABLE[owner]={},{},{},{},{}
 local p={points=0,spent=0,alive=true,major=true,active=true,human=false,allies={}}
 p.GetID=function() return owner end; p.IsAlive=function() return p.alive end; p.IsMajor=function() return p.major end
 p.IsHuman=function() return p.human end; p.IsTurnActive=function() return p.active end
 p.GetProperty,p.SetProperty=store()
 p.GetDiplomacy=function() return {HasAllied=function() return false end} end
 p.GetUnits=function() return {Members=function() return members(NATIVE_UNITS[owner]) end,
  FindID=function(_,id) return NATIVE_UNITS[owner][id] end,
  Destroy=function(_,u) NATIVE_UNITS[owner][u:GetID()]=nil end} end
 p.GetCities=function() return {GetCapitalCity=function() return nil end,Members=function() return members(CITIES[owner]) end} end
 p.GetGovernors=function() return {GetGovernorPoints=function() return p.points end,GetGovernorPointsSpent=function() return p.spent end,
  HasGovernor=function(_,hash) return NATIVE_GOVS[owner][hash]~=nil end,GetGovernor=function(_,hash) return NATIVE_GOVS[owner][hash] end} end
 Players[owner]=p
end
PlayerManager.GetAliveMajorIDs=function() local ids={}; for owner=0,21 do if Players[owner].alive and Players[owner].major then ids[#ids+1]=owner end end; return ids end
ENGINE_STOCK_SPEND=function(owner)
 Players[owner].spent=Players[owner].spent+1
 FIREUI('GovernorPromoted',owner,240,4000); FIREUI('GovernorAppointed',owner,240)
end
local create=NEWGOV
NATIVE_CANDIDATE=create
NEWGOV=function(owner,leader,paid)
 local row=GameInfo.JAC_LeaderPersonas[leader]; local def=GameInfo.Governors[row.GovernorType]
 local existed=NATIVE_GOVS[owner][def.Hash]~=nil
 local g=create(owner,leader,paid)
 if not existed then
  -- Direct setup of an already paid governor consumes its modeled titles.
  -- APPOINT passes false and its existing engine operation charges separately.
  if paid~=false then Players[owner].spent=Players[owner].spent+row.EvolutionCost end
  local base=GameInfo.GovernorPromotions[row.BasePromotionType]
  FIREUI('GovernorPromoted',owner,def.Index,base.Index)
  FIREUI('GovernorAppointed',owner,def.Index)
  for step=1,2 do
   local promotion=GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_'..step]
   if promotion and g.promotions[promotion.Hash] then FIREUI('GovernorPromoted',owner,def.Index,promotion.Index) end
  end
 end
 return g
end
local engineStep=STEP
STEP=function()
 local req=REQUESTS[1]; local prior
 if req and req.op==PlayerOperations.PROMOTE_GOVERNOR then
  local def=GameInfo.Governors[req.params.governor]; local promotion=GameInfo.GovernorPromotions[req.params.promotion]
  local g=def and NATIVE_GOVS[req.owner][def.Hash]
  prior=g and promotion and g.promotions[promotion.Hash]==true
 end
 local ran=engineStep()
 if ran and req.op==PlayerOperations.PROMOTE_GOVERNOR and not prior then
  local def=GameInfo.Governors[req.params.governor]; local promotion=GameInfo.GovernorPromotions[req.params.promotion]
  local g=def and NATIVE_GOVS[req.owner][def.Hash]
  if g and promotion and g.promotions[promotion.Hash] then FIREUI('GovernorPromoted',req.owner,def.Index,promotion.Index) end
 end
 return ran
end
"#;
