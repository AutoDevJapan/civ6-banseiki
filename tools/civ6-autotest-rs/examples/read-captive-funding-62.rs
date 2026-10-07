//! Observe barter/funding options from the saved rejected priced proposal.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
const SAVE: &str = "JAC-Fixed-Captive-Offer-2f83cc9655344adf9d671d24ddacf5be-0000000000000004";
const SHA: &str = "c25efd2a205d3727d6b515f10c09f06a835e8de12887bfdfe808dff831b47b1d";
const GUARD: &str = r#"
assert(Game.GetCurrentGameTurn()==62 and Game.GetLocalPlayer()==0,'latest T62 required')
local p=assert(Game:GetProperty('JAC_BOUNDARY_FIXED_OFFER_62_V1'))
assert(p.phase=='OUTCOME' and p.sent==true and p.price==61 and p.result and p.result.stage=='REJECTED' and
 p.result.returned==false and p.result.responses[1].DealAction==2 and
 p.result.responses[1].RespondingToDealAction==4,'not saved priced rejection')
local old=assert(Game:GetProperty('JAC_BOUNDARY_GROUP_EXCHANGE_62'))
assert(old.stage=='QUOTED_UNKNOWN' and old.nonce=='53.430954889496:62' and
 old.quote.raw.reply.DealAction==10 and old.quote.raw.reply.RespondingToDealAction==6,'old equalization differs')
local function safe(fn)
 local ok,v=pcall(fn);return ok and {status='OK',kind=type(v),value=v} or {status='ERROR',error=tostring(v)}
end
local function method(o,k)
 return safe(function()assert(o and type(o[k])=='function',k..' unavailable');return o[k](o)end)
end
"#;
const PEER: &str = r#"
local e={turn=62,peer=peer}
e.alive=safe(function()return assert(Players[peer]):IsAlive()end)
e.met=safe(function()return Players[0]:GetDiplomacy():HasMet(peer)end)
e.war=safe(function()return Players[0]:GetDiplomacy():IsAtWarWith(peer)end)
e.pending=safe(function()return DealManager.HasPendingDeal(0,peer)end)
e.gold=safe(function()return Players[peer]:GetTreasury():GetGoldBalance()end)
e.civilization=safe(function()return PlayerConfigurations[peer]:GetCivilizationTypeName()end)
e.resources={};e.agreements={}
if e.alive.status=='OK' and e.alive.value==true and e.met.status=='OK' and e.met.value==true and
 e.war.status=='OK' and e.war.value==false and e.pending.status=='OK' and e.pending.value==false then
 local function possible(kind,result)
  result.enumeration=safe(function()
   local d=assert(DealManager.GetWorkingDeal(DealDirection.OUTGOING,0,peer))
   result.existingCount=method(d,'GetItemCount')
   result.hasUnacceptable=method(d,'HasUnacceptableItems')
   result.existingItems={}
   result.existingRead=safe(function()
    for item in d:Items()do
     assert(#result.existingItems<256,'existing item limit')
     local r={};result.existingItems[#result.existingItems+1]=r
     for _,k in ipairs({'GetID','GetType','GetFromPlayerID','GetToPlayerID','GetValueType','GetAmount',
      'GetDuration','GetMaxAmount','GetValueCategory','GetValueTypeNameID','IsValid','IsLocked','IsUnacceptable'})do r[k]=method(item,k)end
    end
    return true
   end)
   result.rows={}
   local list=assert(DealManager.GetPossibleDealItems(0,peer,kind,d),'candidate list missing')
   for _,entry in ipairs(list)do
    assert(#result.rows<256,'candidate limit')
    local r={};result.rows[#result.rows+1]=r
    for _,k in ipairs({'ForType','SubType','ForTypeName','ForTypeDisplayName','IsValid','ValidationResult','Amount','MaxAmount','Duration'})do
     r[k]=safe(function()return entry[k]end)
    end
    r.definition=safe(function()
     local def
     if kind==DealItemTypes.RESOURCES then def=GameInfo.Resources[entry.ForType]
     else def=GameInfo.DiplomaticActions[entry.SubType] end
     assert(def,'definition missing')
     return {type=def.ResourceType or def.DiplomaticActionType,class=def.ResourceClassType,name=def.Name,hash=def.Hash,index=def.Index}
    end)
   end
   return true
  end)
 end
 possible(DealItemTypes.RESOURCES,e.resources)
 possible(DealItemTypes.AGREEMENTS,e.agreements)
else e.skip='not a live met peaceful counterpart without pending deal' end
"#;
const BASE: &str = r#"
local e={turn=62,savedOffer=p,savedOld=old}
e.gold=safe(function()return Players[0]:GetTreasury():GetGoldBalance()end)
e.goldYield=safe(function()return Players[0]:GetTreasury():GetGoldYield()end)
e.maintenance=safe(function()return Players[0]:GetTreasury():GetTotalMaintenance()end)
e.roster=safe(function()return ExposedMembers.JAC_PersonaBoundary.ReadEnemy()end)
e.session=safe(function()return DiplomacyManager.FindOpenSessionID(0,1)end)
e.names=safe(function()
 local civ=PlayerConfigurations[0]:GetCivilizationTypeName();local n=0
 for row in GameInfo.CivilizationCitizenNames()do if row.CivilizationType==civ then n=n+1 end end
 return n
end)
"#;
const ENCODE: &str = r#"
local function quote(s)
 return '"'..tostring(s):gsub('[%z\1-\31\\"]',function(c)
  if c=='"'then return '\\"'end;if c=='\\'then return '\\\\'end
  return string.format('\\u%04x',string.byte(c))
 end)..'"'
end
local function serial(v,depth)
 assert(depth<40,'evidence depth limit')
 local t=type(v)
 if t=='nil'then return 'null'elseif t=='string'then return quote(v)
 elseif t=='number'or t=='boolean'then return tostring(v)end
 assert(t=='table','non scalar data')
 local keys={};for k in pairs(v)do keys[#keys+1]=k end
 table.sort(keys,function(a,b)return tostring(a)<tostring(b)end)
 local out={};for _,k in ipairs(keys)do out[#out+1]=quote(k)..':'..serial(v[k],depth+1)end
 return '{'..table.concat(out,',')..'}'
end
return serial(e,0)
"#;
fn script(peer: u32) -> String {
    format!("{GUARD}\nlocal peer={peer}\n{PEER}\n{ENCODE}")
}
fn ui(code: String) -> Value {
    json!({"action":"ui_lua","state_name":"InGame","code":code,"preserve_diplomacy_session":true})
}
fn scene() -> Value {
    let mut steps = vec![
        json!({"action":"load","name":SAVE,"exact_name":true,"timeout":300,"save_sha256":SHA}),
        json!({"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"InGame","code":"return Players[0] and Players[0]:IsTurnActive() and ExposedMembers.JAC_PersonaBoundary~=nil","preserve_diplomacy_session":true}}),
        ui(format!("{GUARD}\n{BASE}\n{ENCODE}")),
    ];
    for peer in 1..22 {
        steps.push(ui(script(peer)));
    }
    steps.push(json!({"action":"exit_to_menu"}));
    json!({"name":"Read legal barter candidates after priced rejection; preserve latest T62","judge_profile":"generic","strict_lua_errors":true,"steps":steps})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().skip(1).collect();
    if a.len() != 2 {
        return Err("usage: read-captive-funding-62 LATEST_SAVE OUTPUT".into());
    }
    if format!("{:x}", Sha256::digest(std::fs::read(&a[0])?)) != SHA {
        return Err("latest saved rejection hash differs".into());
    }
    let s = scene();
    civ6_autotest_rs::scenario::validate_scenario(&s)?;
    std::fs::write(&a[1], serde_json::to_vec_pretty(&s)?)?;
    Ok(())
}
#[cfg(test)]
fn fixture(extra: &str) -> Value {
    let lua = mlua::Lua::new();
    lua.load(r#"
local old={stage='QUOTED_UNKNOWN',nonce='53.430954889496:62',quote={raw={reply={DealAction=10,RespondingToDealAction=6}}}}
local p={phase='OUTCOME',sent=true,price=61,result={stage='REJECTED',returned=false,responses={{DealAction=2,RespondingToDealAction=4}}}}
Game={GetCurrentGameTurn=function()return 62 end,GetLocalPlayer=function()return 0 end,GetProperty=function(_,k)if k=='JAC_BOUNDARY_FIXED_OFFER_62_V1'then return p end;return old end,SetProperty=function()error('write forbidden')end}
Players={[0]={GetDiplomacy=function()return{HasMet=function()return true end,IsAtWarWith=function()return false end}end},[1]={IsAlive=function()return true end,GetTreasury=function()return{GetGoldBalance=function()return 220 end}end}}
PlayerConfigurations={[1]={GetCivilizationTypeName=function()return 'DATE'end}}
DealDirection={OUTGOING=1};DealItemTypes={RESOURCES=2,AGREEMENTS=3}
local d={GetItemCount=function()return 0 end,HasUnacceptableItems=function()return false end,Items=function()return function()end end,Validate=function()error('no validation')end}
DealManager={HasPendingDeal=function()return false end,GetWorkingDeal=function()return d end,GetPossibleDealItems=function(_,_,kind)
 if kind==2 then return {{ForType=9,IsValid=true,ValidationResult=1,MaxAmount=2}}end;return {}end,SendWorkingDeal=function()error('send forbidden')end}
GameInfo={Resources={[9]={ResourceType='RESOURCE_TEA',ResourceClassType='RESOURCECLASS_LUXURY',Name='TEA'}},DiplomaticActions={}}
"#).exec().unwrap();
    lua.load(extra).exec().unwrap();
    let raw: String = lua.load(script(1)).eval().unwrap();
    serde_json::from_str(&raw).unwrap()
}
#[test]
fn observes_tradeable_resource_without_trade_or_gold_writes() {
    let v = fixture("");
    assert_eq!(v["resources"]["rows"]["1"]["MaxAmount"]["value"], 2);
    assert_eq!(
        v["resources"]["rows"]["1"]["definition"]["value"]["type"],
        "RESOURCE_TEA"
    );
    assert_eq!(v["gold"]["value"], 220);
}
#[test]
fn unmet_peer_and_missing_agreements_are_not_false_available_results() {
    let v=fixture("Players[0].GetDiplomacy=function()return{HasMet=function()return false end,IsAtWarWith=function()return false end}end;DealManager.GetPossibleDealItems=function()error('must not query unmet peer')end");
    assert!(v["skip"].is_string());
    assert!(v["resources"]["enumeration"].is_null());
    let v=fixture("local before=DealManager.GetPossibleDealItems;DealManager.GetPossibleDealItems=function(a,b,k,d)if k==3 then error('missing agreements')end;return before(a,b,k,d)end");
    assert_eq!(v["resources"]["enumeration"]["status"], "OK");
    assert_eq!(v["agreements"]["enumeration"]["status"], "ERROR");
}
#[test]
fn latest_input_and_each_peer_are_isolated_without_old_transaction_replay() {
    let s = scene();
    civ6_autotest_rs::scenario::validate_scenario(&s).unwrap();
    assert_eq!(s["steps"].as_array().unwrap().len(), 25);
    assert!(s["steps"][0].get("resume_once").is_none());
    assert_eq!(
        s["steps"][0]["save_sha256"],
        "c25efd2a205d3727d6b515f10c09f06a835e8de12887bfdfe808dff831b47b1d"
    );
    for step in s["steps"].as_array().unwrap() {
        if let Some(code) = step["code"].as_str() {
            mlua::Lua::new().load(code).into_function().unwrap();
            for text in [
                "SendWorkingDeal",
                "ClearWorkingDeal",
                "AddItemOfType",
                "RequestSession",
                "SetProperty",
                "EndTurn",
            ] {
                assert!(!code.contains(text));
            }
        }
    }
}

#[test]
fn missing_existing_items_preserves_candidates_and_missing_resource_never_becomes_agreement() {
    let v = fixture("DealManager.GetWorkingDeal().Items=nil");
    assert_eq!(v["resources"]["existingRead"]["status"], "ERROR");
    assert_eq!(v["resources"]["enumeration"]["status"], "OK");
    assert_eq!(v["resources"]["rows"]["1"]["MaxAmount"]["value"], 2);
    let v=fixture("GameInfo.Resources[9]=nil;GameInfo.DiplomaticActions[8]={DiplomaticActionType='DIPLOACTION_OPEN_BORDERS',Name='BORDERS'};DealManager.GetPossibleDealItems=function(_,_,k)if k==2 then return {{ForType=9,SubType=8,IsValid=true,MaxAmount=2}}end;return {}end");
    assert_eq!(v["resources"]["rows"]["1"]["definition"]["status"], "ERROR");
    assert_eq!(v["resources"]["rows"]["1"]["ForType"]["value"], 9);
}
