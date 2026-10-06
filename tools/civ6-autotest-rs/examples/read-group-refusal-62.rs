//! Inspect the latest refused offer without starting another negotiation.
use serde_json::json;
use sha2::{Digest, Sha256};
const SAVE: &str = "JAC-Enemy-Neutralization-02e9748e7497429e82af5ca10b890da7-0000000000000005";
const SHA: &str = "2e7744102442b852c5851d79f0c86c73dbc52fd4865b646c20fe90cb0c9fdbe6";
const READ: &str = r#"
assert(Game.GetCurrentGameTurn()==62,'latest refusal must remain at turn62')
local p=assert(Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62'))
assert(p.stage=='QUOTED_UNKNOWN' and p.nonce=='53.430954889496:62' and p.quote and
 p.quote.acceptable==false and p.quote.stage=='AI_REFUSED' and p.quote.raw and
 p.quote.raw.reply and p.quote.raw.reply.DealAction==10 and p.quote.raw.reply.RespondingToDealAction==6,
 'not the preserved refused offer')
local function safe(fn)
 local ok,v=pcall(fn)
 if not ok then return {status='ERROR',error=tostring(v)} end
 return {status='OK',kind=type(v),value=v}
end
local function method(o,k)
 return safe(function() assert(o and type(o[k])=='function',k..' unavailable'); return o[k](o) end)
end
local e={turn=62,saved=p,players={},enums={},deals={},candidates={},capacityCivics={},production={cities={}}}
for _,k in ipairs({'EQUALIZE','ACCEPTED','ADJUSTED','PROPOSED','REJECTED','REJECTED_PERMANENT','EQUALIZE_FAILED','CLOSED','INSPECT'}) do
 e.enums[k]=safe(function() return assert(DealProposalAction)[k] end)
end
e.session=safe(function() return assert(DiplomacyManager).FindOpenSessionID(0,1) end)
e.pending=safe(function() return assert(DealManager).HasPendingDeal(0,1) end)
for id=0,1 do
 local r={}; e.players[id]=r
 r.civilization=safe(function() return PlayerConfigurations[id]:GetCivilizationTypeName() end)
 r.gold=safe(function() return Players[id]:GetTreasury():GetGoldBalance() end)
 r.goldYield=safe(function() return Players[id]:GetTreasury():GetGoldYield() end)
 r.maintenance=safe(function() return Players[id]:GetTreasury():GetTotalMaintenance() end)
 r.capacity=safe(function() return Players[id]:GetDiplomacy():GetSpyCapacity() end)
 r.war=safe(function() return Players[id]:GetDiplomacy():IsAtWarWith(1-id) end)
 r.citizenNames=safe(function()
  local civ=PlayerConfigurations[id]:GetCivilizationTypeName(); local n=0
  for row in GameInfo.CivilizationCitizenNames() do if row.CivilizationType==civ then n=n+1 end end
  return n
 end)
end
e.roster=safe(function() return assert(ExposedMembers.JAC_PersonaBoundary).ReadEnemy() end)
e.production.enumeration=safe(function()
 local function spy()return assert(GameInfo.Units['UNIT_SPY'],'spy definition absent')end
 local function agency()return assert(GameInfo.Buildings['BUILDING_GOV_SPIES'],'official spy agency definition absent')end
 for _,city in Players[0]:GetCities():Members() do
  assert(#e.production.cities<128,'city enumeration limit')
  local r={};e.production.cities[#e.production.cities+1]=r
  r.id=method(city,'GetID');r.owner=method(city,'GetOwner');r.name=method(city,'GetName')
  r.spy={};r.agency={}
  r.spy.canProduce=safe(function()
   local q=city:GetBuildQueue();local params={UnitType=spy().Hash,MilitaryFormationType=MilitaryFormationTypes.STANDARD_MILITARY_FORMATION}
   local allowed,reasons=q:CanProduce(params,false,true)
   return {allowed=allowed,reasons=reasons}
  end)
  r.spy.cost=safe(function()return city:GetBuildQueue():GetUnitCost(spy().Index)end)
  r.spy.progress=safe(function()return city:GetBuildQueue():GetUnitProgress(spy().Index)end)
  r.spy.definition=safe(spy)
  r.spy.canProduceHash=safe(function()
   local allowed,reasons=city:GetBuildQueue():CanProduce(spy().Hash,false,true)
   return {allowed=allowed,reasons=reasons,reasonsKind=type(reasons)}
  end)
  r.spy.canProduceListed=safe(function()
   local params={UnitType=spy().Hash,MilitaryFormationType=MilitaryFormationTypes.STANDARD_MILITARY_FORMATION}
   local allowed,reasons=city:GetBuildQueue():CanProduce(params,true,true)
   return {allowed=allowed,reasons=reasons,reasonsKind=type(reasons)}
  end)
  r.spy.canStart=safe(function()
   local args={
    [CityOperationTypes.PARAM_UNIT_TYPE]=spy().Hash,
    [CityOperationTypes.PARAM_INSERT_MODE]=CityOperationTypes.VALUE_APPEND}
   local allowed,reasons=CityManager.CanStartOperation(city,CityOperationTypes.BUILD,args,true)
   return {allowed=allowed,reasons=reasons,reasonsKind=type(reasons),arguments=args}
  end)
  r.agency.owned=safe(function()return city:GetBuildings():HasBuilding(agency().Index)end)
  r.agency.canProduce=safe(function()
   local allowed,reasons=city:GetBuildQueue():CanProduce(agency().Hash,false,true)
   return {allowed=allowed,reasons=reasons}
  end)
 end
 return true
end)
for _,direction in ipairs({'OUTGOING','INCOMING'}) do
 local r={items={}};e.deals[direction]=r
 r.enumeration=safe(function()
  local d=assert(DealManager.GetWorkingDeal(DealDirection[direction],0,1),'working deal unavailable')
  r.count=method(d,'GetItemCount');r.validation={status='NOT_REQUESTED',reason='observe items without revalidating deal'}
  for item in d:Items() do
   assert(#r.items<64,'deal item limit');local row={};r.items[#r.items+1]=row
   for _,name in ipairs({'GetID','GetType','GetFromPlayerID','GetToPlayerID','GetValueType','GetValueTypeNameID','GetAmount','GetDuration','IsValid','IsLocked','IsUnacceptable'}) do row[name]=method(item,name) end
  end
  return true
 end)
end
e.candidates.enumeration=safe(function()
 local d=DealManager.GetWorkingDeal(DealDirection.OUTGOING,0,1)
 local list=assert(DealManager.GetPossibleDealItems(1,0,DealItemTypes.CAPTIVE,d))
 e.candidates.count=#list;e.candidates.rows={}
 for _,v in ipairs(list) do
  assert(#e.candidates.rows<64,'candidate limit');local r={};e.candidates.rows[#e.candidates.rows+1]=r
  for _,k in ipairs({'ForType','ForTypeName','ForTypeDisplayName','IsValid','ValidationResult','Amount','MaxAmount'}) do r[k]=safe(function() return v[k] end) end
 end
 return true
end)
e.capacityCivics.enumeration=safe(function()
 local mods={}
 for row in GameInfo.Modifiers() do
  local d=GameInfo.DynamicModifiers[row.ModifierType]
  if d and type(d.EffectType)=='string' and (d.EffectType=='EFFECT_GRANT_SPY' or d.EffectType:find('SPY_CAPACITY',1,true)) then mods[row.ModifierId]={kind=row.ModifierType,effect=d.EffectType} end
 end
 e.capacityCivics.rows={}
 for row in GameInfo.CivicModifiers() do if mods[row.ModifierId] then
  local v={civic=row.CivicType,modifier=row.ModifierId,definition=mods[row.ModifierId],arguments={}};e.capacityCivics.rows[#e.capacityCivics.rows+1]=v
  v.completed=safe(function() return Players[0]:GetCulture():HasCivic(GameInfo.Civics[row.CivicType].Index) end)
  v.canProgress=safe(function() return Players[0]:GetCulture():CanProgress(GameInfo.Civics[row.CivicType].Index) end)
  v.cost=safe(function() return Players[0]:GetCulture():GetCultureCost(GameInfo.Civics[row.CivicType].Index) end)
  v.argumentRead=safe(function() for a in GameInfo.ModifierArguments() do if a.ModifierId==row.ModifierId then v.arguments[#v.arguments+1]={name=a.Name,value=a.Value} end end; return true end)
 end end
 return true
end)
e.capacitySources={definitions={},definitionErrors={},bindings={},parents={}}
e.capacitySources.enumeration=safe(function()
 local wanted={};local definitions=e.capacitySources.definitions;local args={}
 local function register(id,m)
  assert(type(id)=='string','modifier ID unavailable')
  wanted[id]=true
  if definitions[id] then return end
  local result={arguments={}};definitions[id]=result
  result.read=safe(function()
   m=m or assert(GameInfo.Modifiers[id],'modifier definition missing')
   result.modifier=m
   local d=assert(GameInfo.DynamicModifiers[m.ModifierType],'dynamic modifier definition missing')
   result.definition=d
   result.grantsCapacity=d.EffectType=='EFFECT_GRANT_SPY' or
    (type(d.EffectType)=='string' and d.EffectType:find('SPY_CAPACITY',1,true)~=nil)
   return true
  end)
 end
 -- Each phase retains its partial rows; failure does not skip other source tables.
 e.capacitySources.modifierRead=safe(function()
  for row in GameInfo.Modifiers() do
   local observed=safe(function()
    local d=assert(GameInfo.DynamicModifiers[row.ModifierType],'dynamic modifier definition missing')
    if type(d.EffectType)=='string' and d.EffectType:find('SPY',1,true) then register(row.ModifierId,row) end
    return true
   end)
   if observed.status=='ERROR' then
    e.capacitySources.definitionErrors[#e.capacitySources.definitionErrors+1]={modifier=row.ModifierId,result=observed}
   end
  end
  return true
 end)
 e.capacitySources.argumentsRead=safe(function()
  for row in GameInfo.ModifierArguments() do args[#args+1]=row end
  return true
 end)
 for _,row in ipairs(args) do
  if row.Value=='UNIT_SPY' or row.Value=='PROMOTION_CLASS_SPY' then
   local observed=safe(function()register(row.ModifierId);return true end)
   if observed.status=='ERROR' then
    e.capacitySources.definitionErrors[#e.capacitySources.definitionErrors+1]={modifier=row.ModifierId,result=observed}
   end
  end
 end
 for _,row in ipairs(args) do
  if definitions[row.ModifierId] then
   local a=definitions[row.ModifierId].arguments;a[#a+1]={name=row.Name,value=row.Value}
  end
 end
 -- Freeze each breadth-first frontier so the depth bound is independent of DB order.
 local frontier={};for id in pairs(wanted) do frontier[id]=true end
 for depth=1,16 do
  local nextFrontier={}
  for _,row in ipairs(args) do
   if type(row.ModifierId)=='string' and frontier[row.Value] and not wanted[row.ModifierId] then
    nextFrontier[row.ModifierId]=true
    e.capacitySources.parents[#e.capacitySources.parents+1]={modifier=row.ModifierId,argument=row.Name,child=row.Value,depth=depth}
   end
  end
  for id in pairs(nextFrontier) do wanted[id]=true end
  frontier=nextFrontier
  if next(frontier)==nil then break end
 end
 e.capacitySources.attachmentTruncated=false
 for _,row in ipairs(args) do
  if frontier[row.Value] and type(row.ModifierId)=='string' and not wanted[row.ModifierId] then e.capacitySources.attachmentTruncated=true end
 end
 local tables={CivicModifiers='CivicType',TechnologyModifiers='TechnologyType',BuildingModifiers='BuildingType',
  PolicyModifiers='PolicyType',GovernmentModifiers='GovernmentType',TraitModifiers='TraitType',
  GovernorModifiers='GovernorType',GovernorPromotionModifiers='GovernorPromotionType',
  GreatPersonIndividualActionModifiers='GreatPersonIndividualType',GreatPersonIndividualBirthModifiers='GreatPersonIndividualType',
  ProjectCompletionModifiers='ProjectType',GameModifiers='ModifierId',CommemorationModifiers='CommemorationType',
  DistrictModifiers='DistrictType',BeliefModifiers='BeliefType',GreatWorkModifiers='GreatWorkType',
  ImprovementModifiers='ImprovementType',UnitAbilityModifiers='UnitAbilityType',UnitPromotionModifiers='UnitPromotionType',
  EmergencyRewards='EmergencyType',EmergencyBuffs='EmergencyType'}
 for tab,key in pairs(tables) do
  local result={rows={}};e.capacitySources.bindings[tab]=result
  result.enumeration=safe(function()
   for row in assert(GameInfo[tab],tab..' unavailable')() do
    local mid=row.ModifierId or row.ModifierID
    if wanted[mid] then
     assert(#result.rows<2048,'source enumeration limit')
     local v={source=row[key],modifier=mid,row=row};result.rows[#result.rows+1]=v
     if tab=='PolicyModifiers' then
      v.unlocked=safe(function()return Players[0]:GetCulture():IsPolicyUnlocked(GameInfo.Policies[v.source].Hash)end)
      v.obsolete=safe(function()return Players[0]:GetCulture():IsPolicyObsolete(GameInfo.Policies[v.source].Hash)end)
     elseif tab=='TechnologyModifiers' then
      v.completed=safe(function()return Players[0]:GetTechs():HasTech(GameInfo.Technologies[v.source].Index)end)
     end
    end
   end
   return true
  end)
 end
 return true
end)
e.capacitySources.identity={}
e.capacitySources.identity.leader=safe(function()return PlayerConfigurations[0]:GetLeaderTypeName()end)
e.capacitySources.identity.traits=safe(function()
 local c=PlayerConfigurations[0]:GetCivilizationTypeName();local l=PlayerConfigurations[0]:GetLeaderTypeName();local r={}
 for row in GameInfo.CivilizationTraits() do if row.CivilizationType==c then r[#r+1]={source=c,trait=row.TraitType} end end
 for row in GameInfo.LeaderTraits() do if row.LeaderType==l then r[#r+1]={source=l,trait=row.TraitType} end end
 return r
end)
local function quoted(s)
 return '"'..tostring(s):gsub('[%z\1-\31\\"]',function(c)
  if c=='"' then return '\\"' end
  if c=='\\' then return '\\\\' end
  return string.format('\\u%04x',string.byte(c))
 end)..'"'
end
local function serial(v,depth)
 if depth>32 then return '{"status":"DEPTH_LIMIT"}' end
 local t=type(v)
 if t=='nil' then return 'null' elseif t=='string' then return quoted(v)
 elseif t=='boolean' then return tostring(v)
 elseif t=='number' then if v==v and math.abs(v)<math.huge then return tostring(v) end;return '{"status":"NONFINITE"}'
 elseif t~='table' then return '{"status":"UNSERIALIZABLE","kind":'..quoted(t)..'}' end
 local keys={};for k in pairs(v) do keys[#keys+1]=k end;table.sort(keys,function(a,b)return tostring(a)<tostring(b)end)
 local rows={};for _,k in ipairs(keys) do rows[#rows+1]=quoted(k)..':'..serial(v[k],depth+1) end
 return '{'..table.concat(rows,',')..'}'
end
return serial(e,0)
"#;
pub(crate) fn scene() -> serde_json::Value {
    json!({"name":"Read preserved T62 refusal and current legal resources; no offer or turn","judge_profile":"generic","strict_lua_errors":true,"steps":[
     {"action":"load","name":SAVE,"exact_name":true,"timeout":300,"save_sha256":SHA},
      {"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"InGame","code":"return Players[0] and Players[0]:IsTurnActive() and ExposedMembers.JAC_PersonaBoundary~=nil","preserve_diplomacy_session":true}},
     {"action":"ui_lua","state_name":"InGame","code":READ,"preserve_diplomacy_session":true},
     {"action":"ui_lua","state_name":"GameCore_Tuner","code":READ},
     {"action":"exit_to_menu"}
    ]})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().skip(1).collect();
    if a.len() != 2 {
        return Err("usage: read-group-refusal-62 ACTUAL_LATEST_SAVE OUTPUT".into());
    }
    if format!("{:x}", Sha256::digest(std::fs::read(&a[0])?)) != SHA {
        return Err("latest refusal save hash differs".into());
    }
    let s = scene();
    civ6_autotest_rs::scenario::validate_scenario(&s)?;
    std::fs::write(&a[1], serde_json::to_vec_pretty(&s)?)?;
    Ok(())
}
#[cfg(test)]
fn read_fixture(extra: &str) -> serde_json::Value {
    let lua = mlua::Lua::new();
    lua.load(r#"Game={GetCurrentGameTurn=function()return 62 end,GetProperty=function()return {stage='QUOTED_UNKNOWN',nonce='53.430954889496:62',quote={acceptable=false,stage='AI_REFUSED',raw={reply={DealAction=10,RespondingToDealAction=6}}}} end,SetProperty=function()error('write forbidden')end};Players={[0]={GetTreasury=function()return{GetGoldBalance=function()return 33 end}end},[1]={}};PlayerConfigurations={};ExposedMembers={};GameInfo={};DealManager={};DealDirection={};"#).exec().unwrap();
    lua.load(extra).exec().unwrap();
    let text: String = lua.load(READ).eval().unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(!text.contains('\n'));
    assert_eq!(
        lua.globals()
            .get::<Option<u32>>("VALIDATIONS")
            .unwrap()
            .unwrap_or(0),
        0
    );
    v
}
#[test]
fn missing_native_apis_keep_saved_reply_and_independent_gold_with_no_writes() {
    let v = read_fixture("");
    assert_eq!(v["players"]["0"]["gold"]["value"], 33);
    assert_eq!(v["saved"]["quote"]["raw"]["reply"]["DealAction"], 10);
    assert_eq!(v["deals"]["OUTGOING"]["enumeration"]["status"], "ERROR");
    assert!(v["players"]["1"]["gold"]["error"].is_string());
}
#[test]
fn actual_grant_spy_effect_keeps_civic_cost_and_items_without_validation() {
    let v = read_fixture(
        r#"
local function rows(a) return function() local i=0;return function()i=i+1;return a[i]end end end
GameInfo.Modifiers=rows({{ModifierId='EXTRA_SPY',ModifierType='GRANT'}})
GameInfo.DynamicModifiers={GRANT={EffectType='EFFECT_GRANT_SPY'}}
GameInfo.CivicModifiers=rows({{CivicType='CIVIC_NATIONALISM',ModifierId='EXTRA_SPY'}})
GameInfo.ModifierArguments=rows({{ModifierId='EXTRA_SPY',Name='Amount',Value='1'}})
GameInfo.Civics={CIVIC_NATIONALISM={Index=12}}
Players[0].GetCulture=function()return{HasCivic=function(_,i)assert(i==12);return false end,CanProgress=function(_,i)assert(i==12);return true end,GetCultureCost=function(_,i)assert(i==12);return 75 end}end
VALIDATIONS=0;local item={GetID=function()return 3 end,GetAmount=function()return 7 end}
DealManager.GetWorkingDeal=function()return{GetItemCount=function()return 1 end,Items=rows({item}),Validate=function()VALIDATIONS=VALIDATIONS+1;error('must not revalidate')end}end
"#,
    );
    let row = &v["capacityCivics"]["rows"]["1"];
    assert_eq!(row["civic"], "CIVIC_NATIONALISM");
    assert_eq!(row["completed"]["value"], false);
    assert_eq!(row["canProgress"]["value"], true);
    assert_eq!(row["cost"]["value"], 75);
    assert_eq!(row["arguments"]["1"]["value"], "1");
    assert_eq!(
        v["deals"]["OUTGOING"]["items"]["1"]["GetAmount"]["value"],
        7
    );
    assert_eq!(
        v["deals"]["OUTGOING"]["validation"]["status"],
        "NOT_REQUESTED"
    );
}
#[test]
fn host_reads_latest_hash_and_contains_no_mutation_step() {
    let s = scene();
    civ6_autotest_rs::scenario::validate_scenario(&s).unwrap();
    let steps = s["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 5);
    assert_eq!(steps[0]["save_sha256"], SHA);
    assert!(steps[0].get("resume_once").is_none());
    assert_eq!(steps[1]["condition"]["preserve_diplomacy_session"], true);
    for text in [
        "SendWorkingDeal",
        "RequestSession",
        "SetProperty",
        "RequestOperation",
        "RequestAction",
        "AddItemOfType",
        "ClearWorkingDeal",
    ] {
        assert!(!READ.contains(text), "{text}");
    }
}

#[cfg(test)]
const PRODUCTION: &str = r#"
GameInfo.Units={UNIT_SPY={Index=18,Hash=118}};GameInfo.Buildings={BUILDING_GOV_SPIES={Index=9,Hash=200}}
MilitaryFormationTypes={STANDARD_MILITARY_FORMATION=0}
local q={CanProduce=function(_,args,excluded,reasons)
 assert(excluded==false and reasons==true)
 if type(args)=='table' then assert(args.UnitType==118 and args.MilitaryFormationType==0);return false,{reason='SPY_CAPACITY'} end
 assert(args==200);return false,{reason='ALREADY_BUILT'}
end,GetUnitCost=function(_,i)assert(i==18);return 225 end,GetUnitProgress=function(_,i)assert(i==18);return 0 end}
local city={GetID=function()return 42 end,GetOwner=function()return 0 end,GetName=function()return 'capital' end,GetBuildQueue=function()return q end,GetBuildings=function()return {HasBuilding=function(_,i)assert(i==9);return true end}end}
Players[0].GetCities=function()return {Members=function()local done=false;return function()if not done then done=true;return 1,city end end end}end
"#;
#[test]
fn production_probe_uses_official_parameters_and_preserves_capacity_refusal() {
    let v = read_fixture(PRODUCTION);
    let city = &v["production"]["cities"]["1"];
    assert_eq!(city["spy"]["canProduce"]["value"]["allowed"], false);
    assert_eq!(
        city["spy"]["canProduce"]["value"]["reasons"]["reason"],
        "SPY_CAPACITY"
    );
    assert_eq!(city["spy"]["cost"]["value"], 225);
    assert_eq!(city["agency"]["owned"]["value"], true);
}

#[test]
fn missing_progress_does_not_erase_eligibility_and_missing_queue_keeps_ownership() {
    let v = read_fixture(&format!("{PRODUCTION}\nq.GetUnitProgress=nil"));
    let c = &v["production"]["cities"]["1"];
    assert_eq!(c["spy"]["progress"]["status"], "ERROR");
    assert_eq!(c["spy"]["canProduce"]["value"]["allowed"], false);
    assert_eq!(
        c["spy"]["canProduce"]["value"]["reasons"]["reason"],
        "SPY_CAPACITY"
    );
    assert_eq!(c["spy"]["cost"]["value"], 225);
    let v = read_fixture(&format!("{PRODUCTION}\nq.CanProduce=nil"));
    let c = &v["production"]["cities"]["1"];
    assert_eq!(c["spy"]["canProduce"]["status"], "ERROR");
    assert_eq!(c["agency"]["canProduce"]["status"], "ERROR");
    assert_eq!(c["agency"]["owned"]["value"], true);
    assert_eq!(c["spy"]["progress"]["value"], 0);
}

#[test]
fn capacity_sources_distinguish_production_policy_and_follow_capacity_attachment() {
    let v = read_fixture(
        r#"
local function rows(a) return function() local i=0;return function()i=i+1;return a[i]end end end
GameInfo.Modifiers=rows({{ModifierId='CAP',ModifierType='GRANT'},{ModifierId='SPEED',ModifierType='SPY_SPEED'}})
GameInfo.DynamicModifiers={GRANT={EffectType='EFFECT_GRANT_SPY'},SPY_SPEED={EffectType='EFFECT_ADJUST_UNIT_SPY_OFFENSIVE_OPERATION_TIME'}}
GameInfo.ModifierArguments=rows({{ModifierId='CAP',Name='Amount',Value='1'},{ModifierId='PARENT',Name='ModifierId',Value='CAP'}})
GameInfo.PolicyModifiers=rows({{PolicyType='POLICY_SPEED',ModifierId='SPEED'}})
GameInfo.BuildingModifiers=rows({{BuildingType='BUILDING_AGENCY',ModifierId='PARENT'}})
GameInfo.Policies={POLICY_SPEED={Hash=72}}
Players[0].GetCulture=function()return {IsPolicyUnlocked=function(_,h)assert(h==72);return true end,IsPolicyObsolete=function(_,h)assert(h==72);return false end}end
"#,
    );
    assert_eq!(v["capacitySources"]["enumeration"]["status"], "OK");
    assert_eq!(
        v["capacitySources"]["definitions"]["CAP"]["grantsCapacity"],
        true
    );
    assert_eq!(
        v["capacitySources"]["definitions"]["SPEED"]["grantsCapacity"],
        false
    );
    assert_eq!(
        v["capacitySources"]["bindings"]["BuildingModifiers"]["rows"]["1"]["modifier"],
        "PARENT"
    );
    assert_eq!(
        v["capacitySources"]["bindings"]["PolicyModifiers"]["rows"]["1"]["unlocked"]["value"],
        true
    );
    assert_eq!(
        v["capacitySources"]["bindings"]["TechnologyModifiers"]["enumeration"]["status"],
        "ERROR"
    );
}

#[test]
fn production_command_preflight_keeps_native_failure_without_requesting_build() {
    let v = read_fixture(&format!(
        r#"{PRODUCTION}
CityOperationTypes={{PARAM_UNIT_TYPE=1,PARAM_INSERT_MODE=2,VALUE_APPEND=3,BUILD=4}}
CityManager={{CanStartOperation=function(c,op,args,results)
 assert(c:GetID()==42 and op==4 and args[1]==118 and args[2]==3 and results==true)
 return false,{{[-7]={{'CAPACITY_FULL'}}}}
end,RequestOperation=function()error('must not submit')end}}
"#
    ));
    let r = &v["production"]["cities"]["1"]["spy"]["canStart"];
    assert_eq!(r["status"], "OK");
    assert_eq!(r["value"]["allowed"], false);
    assert_eq!(r["value"]["reasons"]["-7"]["1"], "CAPACITY_FULL");
    assert_eq!(
        v["production"]["cities"]["1"]["spy"]["canProduce"]["value"]["allowed"],
        false
    );
}

#[test]
fn promotion_class_grants_and_uppercase_emergency_binding_are_not_lost() {
    let v = read_fixture(
        r#"
local function rows(a) return function()local i=0;return function()i=i+1;return a[i]end end end
local m={ModifierId='EMERGENCY_SPY',ModifierType='UNIT_GRANT'}
GameInfo.Modifiers=setmetatable({EMERGENCY_SPY=m},{__call=rows({m})})
GameInfo.DynamicModifiers={UNIT_GRANT={EffectType='EFFECT_GRANT_UNIT_IN_CITY'}}
GameInfo.ModifierArguments=rows({{ModifierId='EMERGENCY_SPY',Name='UnitPromotionClassType',Value='PROMOTION_CLASS_SPY'}})
GameInfo.EmergencyRewards=rows({{EmergencyType='EMERGENCY_SPYING',ModifierID='EMERGENCY_SPY',OnSuccess=false}})
"#,
    );
    let src = &v["capacitySources"];
    assert_eq!(src["enumeration"]["status"], "OK");
    assert_eq!(src["definitions"]["EMERGENCY_SPY"]["grantsCapacity"], false);
    let row = &src["bindings"]["EmergencyRewards"]["rows"]["1"];
    assert_eq!(row["source"], "EMERGENCY_SPYING");
    assert_eq!(row["modifier"], "EMERGENCY_SPY");
    assert_eq!(row["row"]["OnSuccess"], false);
}

#[cfg(test)]
const CAPACITY_PARTIAL: &str = r#"
local function rows(a) return function()local i=0;return function()i=i+1;return a[i]end end end
local cap={ModifierId='CAP',ModifierType='GRANT'}
GameInfo.Modifiers=setmetatable({CAP=cap},{__call=rows({cap})})
GameInfo.DynamicModifiers={GRANT={EffectType='EFFECT_GRANT_SPY'}}
GameInfo.ModifierArguments=rows({{ModifierId='CAP',Name='Amount',Value='1'},{ModifierId='MISSING',Name='UnitType',Value='UNIT_SPY'}})
GameInfo.BuildingModifiers=rows({{BuildingType='AGENCY',ModifierId='CAP'}})
"#;

#[test]
fn missing_definition_or_arguments_does_not_erase_other_source_bindings() {
    let v = read_fixture(CAPACITY_PARTIAL);
    let src = &v["capacitySources"];
    assert_eq!(src["definitions"]["CAP"]["grantsCapacity"], true);
    assert_eq!(src["definitions"]["MISSING"]["read"]["status"], "ERROR");
    assert_eq!(
        src["bindings"]["BuildingModifiers"]["rows"]["1"]["source"],
        "AGENCY"
    );
    let v = read_fixture(&format!(
        "{CAPACITY_PARTIAL}\nGameInfo.ModifierArguments=nil"
    ));
    assert_eq!(v["capacitySources"]["argumentsRead"]["status"], "ERROR");
    assert_eq!(
        v["capacitySources"]["bindings"]["BuildingModifiers"]["rows"]["1"]["source"],
        "AGENCY"
    );
}

#[test]
fn attachment_limit_is_order_independent_and_retains_reachable_bindings() {
    for reverse in [false, true] {
        let v = read_fixture(&format!(
            r#"{CAPACITY_PARTIAL}
local edges={{}};for i=1,17 do edges[#edges+1]={{ModifierId='P'..i,Name='ModifierId',Value=i==1 and 'CAP' or 'P'..(i-1)}} end
if {reverse} then local r={{}};for i=#edges,1,-1 do r[#r+1]=edges[i] end;edges=r end
GameInfo.ModifierArguments=rows(edges)
GameInfo.BuildingModifiers=rows({{{{BuildingType='VALID16',ModifierId='P16'}},{{BuildingType='UNRESOLVED17',ModifierId='P17'}}}})
"#
        ));
        let src = &v["capacitySources"];
        assert_eq!(src["attachmentTruncated"], true);
        assert_eq!(src["parents"]["16"]["depth"], 16);
        assert_eq!(
            src["bindings"]["BuildingModifiers"]["rows"]["1"]["source"],
            "VALID16"
        );
        assert!(src["bindings"]["BuildingModifiers"]["rows"]["2"].is_null());
    }
}
