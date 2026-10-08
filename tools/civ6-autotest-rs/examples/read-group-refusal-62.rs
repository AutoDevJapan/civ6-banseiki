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
