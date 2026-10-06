//! Short live scenarios for the 66 house oaths. Generated Lua is test code, never a MOD script.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::PathBuf};

use crate::{oaths, Outputs, Repository};

const BASE: &str = "tools/civ6-autotest/scenarios/sengoku-oaths-oda.json";
const DIR: &str = "tools/civ6-autotest/scenarios/oath-effects";
const INVENTORY: &str = "documents/poc/data/oath-effect-scenarios.json";
const PRODUCTION_SAMPLES: usize = 1;
const METER_ID: &str = "e4117d8c-4112-4f75-9a50-7d159e361fa4";
const METER_DIR: &str = "poc/JAC_PoC_OathMeter";

fn meter_outputs() -> Outputs {
    let mut sql = String::from(
        "-- Diagnostic only: attach explicitly, activate by owned city plot properties.\n",
    );
    for kind in ["PRODUCTION", "GOLD", "SCIENCE", "CULTURE", "FAITH", "FOOD"] {
        let mut definitions = vec![
            (
                "BASE",
                6,
                "MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_CHANGE",
                format!("JAC_TODO134_BASE_{kind}"),
            ),
            (
                "CAL",
                100,
                "MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER",
                "JAC_TODO134_CAL_ON".into(),
            ),
            (
                "FINE",
                1,
                "MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER",
                "JAC_TODO134_FINE_ON".into(),
            ),
            (
                "SHIFT",
                1,
                "MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_CHANGE",
                "JAC_TODO134_SHIFT_ON".into(),
            ),
        ];
        if kind == "PRODUCTION" {
            definitions.push((
                "SHIFT_TWO",
                1,
                "MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_CHANGE",
                "JAC_TODO134_SHIFT_TWO_ON".into(),
            ));
        }
        for (mode, amount, modifier_type, property) in definitions {
            let id = format!("JAC_TODO134_{mode}_{kind}");
            sql.push_str(&format!("INSERT INTO Requirements (RequirementId,RequirementType) VALUES ('{id}_REQ','REQUIREMENT_PLOT_PROPERTY_MATCHES');\nINSERT INTO RequirementArguments (RequirementId,Name,Value) VALUES ('{id}_REQ','PropertyName','{property}'),('{id}_REQ','PropertyMinimum','1');\nINSERT INTO RequirementSets (RequirementSetId,RequirementSetType) VALUES ('{id}_SET','REQUIREMENTSET_TEST_ALL');\nINSERT INTO RequirementSetRequirements (RequirementSetId,RequirementId) VALUES ('{id}_SET','{id}_REQ');\nINSERT INTO Modifiers (ModifierId,ModifierType,SubjectRequirementSetId) VALUES ('{id}','{modifier_type}','{id}_SET');\nINSERT INTO ModifierArguments (ModifierId,Name,Value) VALUES ('{id}','YieldType','YIELD_{kind}'),('{id}','Amount','{amount}');\n"));
        }
    }
    sql.push_str("INSERT INTO Requirements (RequirementId,RequirementType) VALUES ('JAC_TODO134_HEADROOM_REQ','REQUIREMENT_PLOT_PROPERTY_MATCHES');\nINSERT INTO RequirementArguments (RequirementId,Name,Value) VALUES ('JAC_TODO134_HEADROOM_REQ','PropertyName','JAC_TODO134_HEADROOM_ON'),('JAC_TODO134_HEADROOM_REQ','PropertyMinimum','1');\nINSERT INTO RequirementSets (RequirementSetId,RequirementSetType) VALUES ('JAC_TODO134_HEADROOM_SET','REQUIREMENTSET_TEST_ALL');\nINSERT INTO RequirementSetRequirements (RequirementSetId,RequirementId) VALUES ('JAC_TODO134_HEADROOM_SET','JAC_TODO134_HEADROOM_REQ');\nINSERT INTO Modifiers (ModifierId,ModifierType,SubjectRequirementSetId) VALUES ('JAC_TODO134_HEADROOM_PRODUCTION','MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_CHANGE','JAC_TODO134_HEADROOM_SET'),('JAC_TODO134_HEADROOM_GROWTH','MODIFIER_PLAYER_CITIES_ADJUST_CITY_GROWTH','JAC_TODO134_HEADROOM_SET');\nINSERT INTO ModifierArguments (ModifierId,Name,Value) VALUES ('JAC_TODO134_HEADROOM_PRODUCTION','YieldType','YIELD_PRODUCTION'),('JAC_TODO134_HEADROOM_PRODUCTION','Amount','4096'),('JAC_TODO134_HEADROOM_GROWTH','Amount','-1000');\n");
    let headroom_sql = "-- Opt-in diagnostic headroom; not normal costs or economic balance.\nUPDATE Units SET Cost=32768 WHERE UnitType IN ('UNIT_SPEARMAN','UNIT_BUILDER','UNIT_GALLEY');\nUPDATE Buildings SET Cost=32768 WHERE BuildingType IN ('BUILDING_WALLS','BUILDING_CASTLE','BUILDING_STAR_FORT','BUILDING_MONUMENT');\n".to_owned();
    let headroom_config=r#"<?xml version="1.0" encoding="utf-8"?>
<GameInfo><Parameters><Row Key1="Ruleset" Key2="RULESET_JAC_BANSEIKI" ParameterId="JAC_OathQueueHeadroom" Name="LOC_JAC_OATH_QUEUE_HEADROOM" Domain="bool" DefaultValue="0" ConfigurationGroup="Game" ConfigurationId="JAC_TODO134_QUEUE_HEADROOM" GroupId="AdvancedOptions" Visible="0" SupportsLANMultiplayer="0" SupportsInternetMultiplayer="0" SupportsHotSeat="0" SupportsPlayByCloud="0"/></Parameters></GameInfo>
"#.to_owned();
    let headroom_text = r#"<?xml version="1.0" encoding="utf-8"?>
<GameData><LocalizedText>
  <Row Tag="LOC_JAC_OATH_QUEUE_HEADROOM" Language="ja_JP"><Text>誓約検査用の建造費用</Text></Row>
  <Row Tag="LOC_JAC_OATH_QUEUE_HEADROOM" Language="en_US"><Text>Construction costs for oath diagnostics</Text></Row>
  <Row Tag="LOC_JAC_OATH_QUEUE_HEADROOM" Language="ru_RU"><Text>Стоимость строительства для проверки клятв</Text></Row>
</LocalizedText></GameData>
"#.to_owned();
    let modinfo = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Mod id="{METER_ID}" version="1">
  <Properties><Name>JAC diagnostic oath meter</Name><Description>Controlled diagnostic inputs and a 100 percent reference; inactive until explicitly attached and activated.</Description><Authors>civ6-banseiki</Authors><CompatibleVersions>2.0</CompatibleVersions></Properties>
  <Dependencies><Mod id="26c967d0-c10d-4eeb-8aaf-c83dc7e621e7" title="Sengoku Trees"/></Dependencies>
  <ActionCriteria><Criteria id="JAC_OathMeter_Banseiki"><RuleSetInUse>RULESET_JAC_BANSEIKI</RuleSetInUse></Criteria><Criteria id="JAC_OathMeter_Headroom"><RuleSetInUse>RULESET_JAC_BANSEIKI</RuleSetInUse><ConfigurationValueMatches><Group>Game</Group><ConfigurationId>JAC_TODO134_QUEUE_HEADROOM</ConfigurationId><Value>1</Value></ConfigurationValueMatches></Criteria></ActionCriteria>
  <InGameActions><UpdateDatabase id="JAC_OathMeter_Data" criteria="JAC_OathMeter_Banseiki"><Properties><LoadOrder>300</LoadOrder></Properties><File>Data/Meter.sql</File></UpdateDatabase><UpdateDatabase id="JAC_OathMeter_Headroom_Data" criteria="JAC_OathMeter_Headroom"><Properties><LoadOrder>400</LoadOrder></Properties><File>Data/Headroom.sql</File></UpdateDatabase></InGameActions>
  <FrontEndActions><UpdateDatabase id="JAC_OathMeter_Headroom_Config"><File>Config/Headroom.xml</File></UpdateDatabase><UpdateText id="JAC_OathMeter_Headroom_Text"><File>Text/Headroom.xml</File></UpdateText></FrontEndActions>
  <Files><File>Data/Meter.sql</File><File>Data/Headroom.sql</File><File>Config/Headroom.xml</File><File>Text/Headroom.xml</File></Files>
</Mod>
"#
    );
    [
        (
            PathBuf::from(format!("{METER_DIR}/Text/Headroom.xml")),
            headroom_text,
        ),
        (
            PathBuf::from(format!("{METER_DIR}/Config/Headroom.xml")),
            headroom_config,
        ),
        (PathBuf::from(format!("{METER_DIR}/Data/Meter.sql")), sql),
        (
            PathBuf::from(format!("{METER_DIR}/Data/Headroom.sql")),
            headroom_sql,
        ),
        (
            PathBuf::from(format!("{METER_DIR}/JAC_PoC_OathMeter.modinfo")),
            modinfo,
        ),
    ]
    .into_iter()
    .collect()
}

fn bounded_turns(steps: &[Value]) -> usize {
    steps
        .iter()
        .filter(|s| {
            s["code"]
                .as_str()
                .is_some_and(|c| c.contains("UI.RequestAction(ActionTypes.ACTION_ENDTURN,"))
        })
        .count()
}

#[derive(Clone, Debug)]
struct Effect {
    kind: String,
    amount: i64,
}

#[derive(Clone, Debug)]
struct Case {
    house: String,
    civ: String,
    oath: String,
    effects: Vec<Effect>,
}

fn cases(repo: &Repository) -> Result<Vec<Case>> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let mut houses = BTreeSet::new();
    let valid_id = |id: &str| {
        id.starts_with(|c: char| c.is_ascii_uppercase())
            && id
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
    };
    for house in oaths::catalogue(repo)? {
        let id = house["id"].as_str().context("house id")?;
        if !valid_id(id) || !houses.insert(id.to_owned()) {
            bail!("invalid or duplicate house {id}");
        }
        let oaths = house["oaths"].as_array().context("house oaths")?;
        if oaths.len() != 3 {
            bail!("{id}: expected three oaths");
        }
        for oath in oaths {
            let oath_id = oath["id"].as_str().context("oath id")?;
            if !valid_id(oath_id) {
                bail!("invalid oath id {oath_id}");
            }
            let key = format!("JAC_OATH_{id}_{oath_id}");
            if !seen.insert(key.clone()) {
                bail!("duplicate oath {key}");
            }
            let effects = oath["effects"]
                .as_array()
                .context("oath effects")?
                .iter()
                .map(|pair| {
                    let pair = pair.as_array().context("effect pair")?;
                    if pair.len() != 2 {
                        bail!("{key}: effect needs kind and amount");
                    }
                    let kind = pair[0].as_str().context("effect kind")?;
                    let amount = pair[1].as_i64().context("effect amount")?;
                    if ![
                        "production",
                        "gold",
                        "science",
                        "culture",
                        "faith",
                        "food",
                        "growth",
                        "military",
                        "naval",
                        "walls",
                    ]
                    .contains(&kind)
                        || amount <= 0
                    {
                        bail!("{key}: unsupported effect {kind}={amount}");
                    }
                    Ok(Effect {
                        kind: kind.to_owned(),
                        amount,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            if effects.len() != 2 {
                bail!("{key}: expected two effects");
            }
            if effects[0].kind == effects[1].kind {
                bail!("{key}: effects must use two distinct metrics");
            }
            result.push(Case {
                house: id.to_owned(),
                civ: format!("CIVILIZATION_JAC_{id}"),
                oath: key,
                effects,
            });
        }
    }
    if result.len() != 66 || seen.len() != 66 || houses.len() != 22 {
        bail!("expected exactly 66 unique oaths, found {}", result.len());
    }
    Ok(result)
}

const LUA_HELPERS: &str = r#"
local function finite(v) return type(v)=='number' and v==v and v>-math.huge and v<math.huge end
local function number(v,label) assert(finite(v),'finite numeric API required: '..label); return v end
local function jstr(v)
 return '"'..tostring(v):gsub('[%z\1-\31\\"]',function(c)
  if c=='"' then return '\\"' elseif c=='\\' then return '\\\\' else return string.format('\\u%04x',string.byte(c)) end
 end)..'"'
end
-- Infer a conservative common reporting step from controlled perturbations.
-- Differences remove a fixed offset. Reject values that cannot be canonically
-- represented; do not mistake binary floating-point noise for extra precision.
local function reported_step(values)
 if #values<3 or not finite(values[1]) then return nil end
 local origin=values[1]
 local function gcd(a,b) while b~=0 do a,b=b,a%b end return a end
 for i=2,#values do if not finite(values[i]) then return nil end end
 -- Live GetYield reports fixed-point binary fractions (including 1/256),
 -- while queue progress is integral. Use the coarsest supported common
 -- lattice, never the smallest candidate a floating-point artifact permits.
 local upper=nil
 for _,scale in ipairs({1,2,4,8,16,32,64,128,256,512,1024,2048,4096,10,100,1000,10000}) do
  local divisor=0; local represented=true
  for i=2,#values do
   local difference=values[i]-origin
   local ticks=math.floor(difference*scale+0.5)
   if math.abs(ticks)>9007199254740991 or math.abs(difference-ticks/scale)>0.00001 then represented=false; break end
   divisor=gcd(divisor,math.abs(ticks))
  end
  if represented and divisor>0 then upper=math.max(upper or 0,divisor/scale) end
 end
 return upper and upper+0.00002 or nil
end
"#;

fn ui(state: &str, code: String) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":format!("{LUA_HELPERS}\n{code}")})
}

fn lua_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serializes")
}

fn substitute(template: &str, case: &Case, extra: &[(&str, &str)]) -> String {
    let mut code = template
        .replace("__OATH__", &lua_string(&case.oath))
        .replace("__CIV__", &lua_string(&case.civ))
        .replace("__HOUSE__", &lua_string(&case.house));
    for (from, to) in extra {
        code = code.replace(from, to);
    }
    code
}

fn first_steps(base: &Value, civ: &str) -> Result<Vec<Value>> {
    let source = base["steps"].as_array().context("base steps")?;
    if source.len() < 4
        || source[0]["action"] != "enable_mods"
        || source[1]["action"] != "configure"
        || source[2]["action"] != "ui_lua"
        || source[3]["action"] != "start_game"
    {
        bail!("base scenario start contract changed");
    }
    let mut steps = source[..4].to_vec();
    if steps[1]["ruleset"] != "RULESET_JAC_BANSEIKI"
        || steps[1]["map_file"]
            != "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map"
        || steps[1]["map_size"] != "MAPSIZE_HUGE"
    {
        bail!("audit requires Banseiki, the 15km Japan map and huge map size");
    }
    steps[0]["ids"]
        .as_array_mut()
        .context("enabled mods")?
        .push(json!(METER_ID));
    let players = steps[1]["players"].as_array_mut().context("base players")?;
    let civs: BTreeSet<_> = players.iter().filter_map(|p| p["civ"].as_str()).collect();
    if players.len() != 22 || civs.len() != 22 {
        bail!("base scenario must contain the 22 distinct map houses");
    }
    let slot = players
        .iter()
        .position(|player| player["civ"] == civ)
        .with_context(|| format!("base scenario lacks {civ}"))?;
    players.swap(0, slot);
    for (index, player) in players.iter_mut().enumerate() {
        player["human"] = json!(index == 0);
    }
    steps[2]["code"] = json!(format!(
        "BuildHeadlessGameSetup(); g_GameParameters:FullRefresh(); if PlayerConfigurations[0]:GetCivilizationTypeName()~={} then error('human house mismatch') end; HideGameSetup(); return 'audit setup built'",
        lua_string(civ)
    ));
    steps.push(json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"GameCore_Tuner","code":"local l=Game:GetProperty('JAC_AGES_LEDGER'); local p=Players[0]; return l and l.houses and l.houses.p0 and p and p:GetCities():GetCapitalCity()~=nil"}}));
    Ok(steps)
}

const METER_ATTACH: &str = r#"
assert(not Game:GetProperty('JAC_TODO134_METER_ATTACHED'),'reference modifiers already attached')
local p=assert(Players[0]); assert(p:IsHuman(),'reference requires human player 0')
assert(type(p.AttachModifierByID)=='function','official player modifier attachment unavailable')
for _,kind in ipairs({'PRODUCTION','GOLD','SCIENCE','CULTURE','FAITH','FOOD'}) do
 for _,mode in ipairs({'BASE','CAL','FINE','SHIFT'}) do p:AttachModifierByID('JAC_TODO134_'..mode..'_'..kind) end
 for _,city in p:GetCities():Members() do
  local plot=assert(Map.GetPlot(city:GetX(),city:GetY())); assert(plot:GetOwner()==0,'reference city owner changed')
  plot:SetProperty('JAC_TODO134_BASE_'..kind,1); plot:SetProperty('JAC_TODO134_CAL_ON',0)
  plot:SetProperty('JAC_TODO134_FINE_ON',0); plot:SetProperty('JAC_TODO134_SHIFT_ON',0); plot:SetProperty('JAC_TODO134_SHIFT_TWO_ON',0)
 end
end
p:AttachModifierByID('JAC_TODO134_SHIFT_TWO_PRODUCTION')
Game:SetProperty('JAC_TODO134_METER_ATTACHED',true)
return 'diagnostic base input +6; explicit native 100-percent reference attached but off; no natural balance claim'
"#;

const METER_TOGGLE: &str = r#"
assert(Game:GetProperty('JAC_TODO134_METER_ATTACHED'),'reference not attached')
for _,city in Players[0]:GetCities():Members() do
 local plot=assert(Map.GetPlot(city:GetX(),city:GetY())); assert(city:GetOwner()==0 and plot:GetOwner()==0,'reference owner changed')
 plot:SetProperty('JAC_TODO134_CAL_ON',__CAL__)
 if __KEEP_PRODUCTION__==0 then plot:SetProperty('JAC_TODO134_HEADROOM_ON',0) end
 plot:SetProperty('JAC_TODO134_FINE_ON',0); plot:SetProperty('JAC_TODO134_SHIFT_ON',0); plot:SetProperty('JAC_TODO134_SHIFT_TWO_ON',0)
 for _,kind in ipairs({'PRODUCTION','GOLD','SCIENCE','CULTURE','FAITH','FOOD'}) do
  plot:SetProperty('JAC_TODO134_BASE_'..kind,(__BASE__==1 or (__KEEP_PRODUCTION__==1 and kind=='PRODUCTION')) and 1 or 0)
 end
end
return 'diagnostic reference flags updated'
"#;

fn meter_toggle(cal: u8, base: u8, keep_production: u8) -> Value {
    ui(
        "GameCore_Tuner",
        METER_TOGGLE
            .replace("__CAL__", &cal.to_string())
            .replace("__BASE__", &base.to_string())
            .replace("__KEEP_PRODUCTION__", &keep_production.to_string()),
    )
}

fn precision_toggle(fine: u8, shift: u8, second: u8) -> Value {
    ui("GameCore_Tuner", format!("for _,city in Players[0]:GetCities():Members() do local p=assert(Map.GetPlot(city:GetX(),city:GetY())); assert(city:GetOwner()==0 and p:GetOwner()==0,'precision probe owner changed'); p:SetProperty('JAC_TODO134_FINE_ON',{fine}); p:SetProperty('JAC_TODO134_SHIFT_ON',{shift}); p:SetProperty('JAC_TODO134_SHIFT_TWO_ON',{second}); end; return 'controlled reporting precision probe updated'"))
}

const CAPABILITY: &str = r#"
local p=Players[0]; local c=assert(p and p:GetCities():GetCapitalCity(),'human house lacks a real capital')
local function member(receiver,name)
 if receiver==nil then return nil,'receiver unavailable' end
 local ok,value=pcall(function() return receiver[name] end)
 if ok then return value,nil else return nil,tostring(value) end
end
local function object(receiver,name)
 local f=member(receiver,name); if type(f)~='function' then return nil end
 local ok,value=pcall(f,receiver); if ok then return value end
end
local q=object(c,'GetBuildQueue'); local g=object(c,'GetGrowth'); local parts={}
local function probe(receiver,label,name,args,official)
 local f,lookup_error=member(receiver,name); local exists=type(f)=='function'
 local ok,value=false,nil
 if exists then ok,value=pcall(f,receiver,unpack(args)) end
 local good=ok and finite(value)
 parts[#parts+1]='{"receiver":'..jstr(label)..',"method":'..jstr(name)..',"arguments":['..table.concat(args,',')..'],"exists":'..tostring(exists)..',"call_ok":'..tostring(ok)..',"return_type":'..jstr(type(value))..',"finite_numeric":'..tostring(good)..',"value":'..(good and string.format('%.9f',value) or 'null')..',"error":'..(lookup_error and jstr(lookup_error) or exists and not ok and jstr(value) or 'null')..',"official_usage":'..tostring(official)..',"quantity_contract":'..jstr(official and 'documented_observation' or 'unknown_do_not_accept')..'}'
end
for _,kind in ipairs({'PRODUCTION','GOLD','SCIENCE','CULTURE','FAITH','FOOD'}) do
 local row=assert(GameInfo.Yields['YIELD_'..kind]); probe(c,'city','GetYield',{row.Index},true)
 -- Candidate APIs are called only here. A numeric result does not establish units or scope.
 probe(c,'city','GetBaseYield',{row.Index},false); probe(c,'city','GetYieldModifier',{row.Index},false)
end
for _,name in ipairs({'GetOverallGrowthModifier','GetOtherGrowthModifier','GetHousingGrowthModifier','GetHappinessGrowthModifier','GetFoodSurplus','GetLoyaltyGrowthModifier'}) do probe(g,'growth',name,{},true) end
probe(q,'queue','GetCurrentProductionTypeHash',{},true)
local unit=assert(GameInfo.Units.UNIT_SPEARMAN); local building=assert(GameInfo.Buildings.BUILDING_WALLS)
for _,name in ipairs({'GetUnitProgress','GetUnitCost'}) do probe(q,'queue',name,{unit.Index},true) end
for _,name in ipairs({'GetBuildingProgress','GetBuildingCost'}) do probe(q,'queue',name,{building.Index},true) end
probe(q,'queue','GetProductionModifier',{},false); probe(c,'city','GetProductionModifier',{},false)
return '{"context":'..jstr(__CONTEXT__)..',"owner":0,"city":'..c:GetID()..',"accepted":false,"yield_quantity_basis":"unresolved","production_quantity_basis":"requires_unmodified_observed_baseline","probes":['..table.concat(parts,',')..']}'
"#;

fn capability_scenario(base: &Value) -> Result<Value> {
    let mut steps = first_steps(base, "CIVILIZATION_JAC_ODA")?;
    for state in ["GameCore_Tuner", "InGame"] {
        steps.push(ui(
            state,
            CAPABILITY.replace("__CONTEXT__", &lua_string(state)),
        ));
    }
    Ok(
        json!({"name":"Oath effects: short official-API capability diagnosis; no effect pass", "judge_profile":"generic", "steps":steps}),
    )
}

const FIXTURE: &str = r#"
local oath=__OATH__; local civ=__CIV__
local l=assert(Game:GetProperty('JAC_AGES_LEDGER'),'saved age ledger missing')
assert(l.reign==0,'fixture must start in Tenbun before first judgement')
assert(#PlayerManager.GetAliveMajors()==22,'expected 22 actual map houses')
local h=assert(l.houses.p0,'human house missing from ledger')
assert(h.civ==civ and PlayerConfigurations[0]:GetCivilizationTypeName()==civ,'wrong human house')
local count=0
local catalogue={}
for row in GameInfo.JAC_SengokuOaths() do catalogue[#catalogue+1]=row end
assert(#catalogue==66,'loaded catalogue is not the 66-oath inventory')
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 assert(p:GetCities():GetCount()>0,'house has no owned city: '..p:GetID())
 count=count+p:GetCities():GetCount()
 for _,c in p:GetCities():Members() do
  local plot=assert(Map.GetPlot(c:GetX(),c:GetY()))
  assert(c:GetOwner()==p:GetID() and plot:GetOwner()==p:GetID(),'actual owner/plot mismatch')
  for _,row in ipairs(catalogue) do assert((plot:GetProperty(row.OathType..'_ON') or 0)==0,'another oath active before fixture') end
 end
end
assert(count==56,'15km map city count changed: '..count)
local own=0
for _,c in Players[0]:GetCities():Members() do
 own=own+1
 local plot=assert(Map.GetPlot(c:GetX(),c:GetY()))
 assert(c:GetOwner()==0 and plot:GetOwner()==0,'city owner/plot mismatch')
 assert((plot:GetProperty(oath..'_ON') or 0)==0,'oath active before choice')
end
-- Only the age fixture is forced. The actual oath is chosen through DedicationPopup.
l.reign=1; h.age='GOLDEN'; h.oaths={}; h.oathReign=1; h.oathChosen=nil
h.oathDue=Game.GetCurrentGameTurn()+20
Game:SetProperty('JAC_AGES_LEDGER',l)
assert(h.oaths[oath]==nil,'fixture must not choose an oath')
for _,c in Players[0]:GetCities():Members() do Map.GetPlot(c:GetX(),c:GetY()):SetProperty('JAC_AGE_GOLDEN',1) end
return 'synthetic golden-age fixture, no oath chosen; real cities='..own
"#;

const POPUP: &str = r#"
local oath=__OATH__; local civ=__CIV__
assert(Players[0]:IsTurnActive(),'choice requires human active turn')
assert(JAC_ShowOaths(true),'pending oath popup did not open')
local rows=JAC_GetOfferedOaths(); assert(#rows==3,'expected exactly three rendered choices')
local offered=false
for _,row in ipairs(rows) do
 local db
 for entry in GameInfo.JAC_SengokuOaths() do if entry.OathType==row.type then db=entry; break end end
 assert(db and db.CivilizationType==civ,'foreign oath rendered')
 assert(row.name==Locale.Lookup(db.Name) and row.description==Locale.Lookup(db.GoldenDescription),'rendered text mismatch')
 if row.type==oath then offered=true end
end
assert(offered,'target oath absent from rendered rows')
assert(JAC_ChooseOath(oath),'real UI EXECUTE_SCRIPT selection was rejected')
return 'real DedicationPopup selected '..oath
"#;

const FLAG: &str = r#"
local oath=__OATH__; local desired=__FLAG__
local l=assert(Game:GetProperty('JAC_AGES_LEDGER')); local h=assert(l.houses.p0)
assert(h.oathChosen==1 and h.oaths[oath]==true,'selected oath was not persisted')
for _,c in Players[0]:GetCities():Members() do
 local plot=assert(Map.GetPlot(c:GetX(),c:GetY()))
 assert(c:GetOwner()==0 and plot:GetOwner()==0,'owner changed')
 plot:SetProperty(oath..'_ON',desired)
 assert(plot:GetProperty(oath..'_ON')==desired,'diagnostic flag did not switch')
end
return 'diagnostic flag='..desired..' on owned cities; saved selection remains unchanged'
"#;

const MEASURE: &str = r#"
local oath=__OATH__; local phase=__PHASE__
local l=assert(Game:GetProperty('JAC_AGES_LEDGER'))
local h=assert(l.houses.p0); assert(h.civ==__CIV__,'house changed')
local keys={'production','gold','science','culture','faith','food'}
local function sample(c)
 local growth=assert(c:GetGrowth(),'city growth API unavailable')
 assert(type(growth.GetOverallGrowthModifier)=='function','official growth modifier API unavailable')
 local r={id=c:GetID(),plot=Map.GetPlot(c:GetX(),c:GetY()):GetIndex(),owner=c:GetOwner(),y={}}
 for _,kind in ipairs(keys) do
  local row=assert(GameInfo.Yields['YIELD_'..string.upper(kind)])
  r.y[kind]=number(c:GetYield(row.Index),'city '..c:GetID()..' '..kind)
 end
 r.growth=number(growth:GetOverallGrowthModifier(),'overall growth modifier')
 r.other=number(growth:GetOtherGrowthModifier(),'other growth modifier')
 r.housing=number(growth:GetHousingGrowthModifier(),'housing growth modifier')
 r.happiness=number(growth:GetHappinessGrowthModifier(),'happiness growth modifier')
 r.surplus=number(growth:GetFoodSurplus(),'food surplus')
 r.occupied=c:IsOccupied(); assert(type(r.occupied)=='boolean','occupation API not boolean')
 r.loyalty=number(growth:GetLoyaltyGrowthModifier(),'loyalty growth modifier')
 local expected=(phase=='selected' or phase=='restored') and c:GetOwner()==0 and 1 or 0
 local plot=assert(Map.GetPlotByIndex(r.plot)); local total=0
 for row in GameInfo.JAC_SengokuOaths() do
  total=total+1
  assert((plot:GetProperty(row.OathType..'_ON') or 0)==(row.OathType==oath and expected or 0),'oath flag leak during '..phase..' at plot '..r.plot)
 end
 assert(total==66,'loaded oath inventory changed')
 return r
end
local own={}
for _,c in Players[0]:GetCities():Members() do
 assert(c:GetOwner()==0 and Map.GetPlot(c:GetX(),c:GetY()):GetOwner()==0,'not a currently owned city')
 own[#own+1]=sample(c)
end
table.sort(own,function(a,b) return a.plot<b.plot end)
assert(#own>0,'no owned city')
local foreign={}
for _,p in ipairs(PlayerManager.GetAliveMajors()) do if p:GetID()~=0 then
 for _,c in p:GetCities():Members() do foreign[#foreign+1]=sample(c) end
end end
table.sort(foreign,function(a,b) return a.plot<b.plot end)
assert(#foreign>0 and #own+#foreign==56,'complete real-city control inventory changed')
if phase=='baseline' then JAC_OATH_AUDIT={} end
JAC_OATH_AUDIT=assert(JAC_OATH_AUDIT,'baseline UI context missing')
JAC_OATH_AUDIT[phase]={own=own,foreign=foreign}
local function encode(r)
 local y=r.y
 return string.format('{"city":%d,"plot":%d,"owner":%d,"growth":%.9f,"other_growth":%.9f,"housing":%.9f,"happiness":%.9f,"food_surplus":%.9f,"occupied":%s,"loyalty":%.9f,"production":%.9f,"gold":%.9f,"science":%.9f,"culture":%.9f,"faith":%.9f,"food":%.9f}',r.id,r.plot,r.owner,r.growth,r.other,r.housing,r.happiness,r.surplus,tostring(r.occupied),r.loyalty,y.production,y.gold,y.science,y.culture,y.faith,y.food)
end
local parts={}; for _,r in ipairs(own) do parts[#parts+1]=encode(r) end
local controls={}; for _,r in ipairs(foreign) do controls[#controls+1]=encode(r) end
return '{"oath":"'..oath..'","phase":"'..phase..'","cities":['..table.concat(parts,',')..'],"foreign":['..table.concat(controls,',')..']}'
"#;

const COMPARE: &str = r#"
local oath=__OATH__
local spec=__EFFECTS__
local all=assert(JAC_OATH_AUDIT,'UI samples missing')
local base=assert(all.baseline); local on=assert(all.selected)
local off=assert(all.off); local back=assert(all.restored)
assert(#base.own==#on.own and #on.own==#off.own and #off.own==#back.own,'owned city inventory changed')
if all.reference or all.reference_off then
 assert(all.reference and all.reference_off,'reference on/off evidence missing')
 for _,phase in ipairs({'reference','reference_off'}) do assert(#all[phase].own==#base.own and #all[phase].foreign==#base.foreign,'reference city inventory incomplete') end
end
if all.precision_one or all.precision_two or all.precision_off then
 for _,phase in ipairs({'precision_one','precision_two','precision_off'}) do assert(all[phase] and #all[phase].own==#base.own and #all[phase].foreign==#base.foreign,'precision probe inventory incomplete') end
end
local function close(a,b) return finite(a) and finite(b) and math.abs(a-b)<=0.00002 end
local function metric(r,kind) if kind=='growth' then return r.growth else return r.y[kind] end end
local reports={}; local proven={}; local quantities=true
for _,e in ipairs(spec) do proven[e.kind]=0 end
for i,a in ipairs(base.own) do
 local b,c,d=on.own[i],off.own[i],back.own[i]
 assert(a.id==b.id and b.id==c.id and c.id==d.id and a.plot==b.plot and b.plot==c.plot and c.plot==d.plot,'owned city identity changed')
 local plot=assert(Map.GetPlotByIndex(a.plot))
 assert(plot:GetOwner()==0 and plot:GetProperty(oath..'_ON')==1,'selected city flag not restored')
 local used=0
 for row in GameInfo.JAC_SengokuOaths() do
  local value=plot:GetProperty(row.OathType..'_ON') or 0
  if value~=0 then assert(row.OathType==oath and value==1,'another oath leaked into selected city'); used=used+1 end
 end
 assert(used==1,'selected oath missing from real city')
 for _,e in ipairs(spec) do
  if e.kind~='military' and e.kind~='naval' and e.kind~='walls' then
   local av,bv,cv,dv=metric(a,e.kind),metric(b,e.kind),metric(c,e.kind),metric(d,e.kind)
   local observed=bv-av
   assert(close(cv,av) and close(dv,bv),'off/on/off restoration failed for '..e.kind..' city '..a.id)
   local expected=nil; local quantity=false; local reporting_step=nil; local reason='raw_yield_basis_not_documented'
   if e.kind=='growth' then
    assert(close(b.other-a.other,e.amount/100) and close(c.other,a.other) and close(d.other,b.other),'wrong percentage in actual other-growth modifier')
    assert(close(a.housing,b.housing) and close(a.housing,c.housing) and close(a.housing,d.housing),'housing changed during growth isolation')
    assert(close(a.happiness,b.happiness) and close(a.happiness,c.happiness) and close(a.happiness,d.happiness),'happiness changed during growth isolation')
    local eligible=not a.occupied and not b.occupied and not c.occupied and not d.occupied and a.surplus>0 and b.surplus>0 and c.surplus>0 and d.surplus>0 and close(a.loyalty,1) and close(b.loyalty,1) and close(c.loyalty,1) and close(d.loyalty,1)
    if eligible then
     local factor=math.max(1+a.happiness/100+a.other,0)
     assert(close(a.growth,a.housing*factor),'official overall-growth formula does not match baseline')
     expected=a.housing*(math.max(1+a.happiness/100+a.other+e.amount/100,0)-factor)
     assert(close(observed,expected),'overall growth did not match housing-adjusted expected delta')
     quantity=true; reason='official_ui_growth_formula'
     if expected>0.00002 then proven[e.kind]=proven[e.kind]+1 end
    else quantities=false; reason='occupied_deficit_or_loyalty_growth_requires_diagnosis' end
   else
    local ref=all.reference and all.reference.own[i]
    local ref_off=all.reference_off and all.reference_off.own[i]
    if ref and ref_off then
     assert(ref.id==a.id and ref.plot==a.plot and ref.owner==a.owner and ref_off.id==a.id and ref_off.plot==a.plot and ref_off.owner==a.owner,'reference city changed')
     assert(close(metric(ref_off,e.kind),av),'reference did not turn off to original yield')
     local hundred=metric(ref,e.kind)-av
     assert(finite(hundred) and hundred>0.03,'no measurable actual 100-percent reference')
     local p1=all.precision_one and all.precision_one.own[i]
     local p2=all.precision_two and all.precision_two.own[i]
     local p0=all.precision_off and all.precision_off.own[i]
     local step=nil
     if p1 and p2 and p0 then
      for _,p in ipairs({p1,p2,p0}) do assert(p.id==a.id and p.plot==a.plot and p.owner==a.owner,'precision probe city changed') end
      assert(close(metric(p0,e.kind),av),'precision probe did not restore')
      step=reported_step({av,metric(ref,e.kind),metric(p1,e.kind),metric(p2,e.kind)})
     end
     reporting_step=step
     local tolerance=step and 2*step*(1+e.amount/100)+0.00004
     if tolerance and tolerance<hundred*0.005 then
      expected=hundred*e.amount/100
      assert(math.abs(observed-expected)<=tolerance,'oath yield differs from independently measured 100-percent reference')
      quantity=true; reason='native_100_percent_reference_delta'
      proven[e.kind]=proven[e.kind]+1
     else
      quantities=false; reason='native_reference_resolution_insufficient'
      if observed>0.00002 then proven[e.kind]=proven[e.kind]+1 end
     end
    else
     quantities=false
     assert(observed>=-0.00002,'selected yield decreased unexpectedly')
     if observed>0.00002 then proven[e.kind]=proven[e.kind]+1
     elseif av>0.03 then error('selected yield did not actually increase') end
    end
   end
   reports[#reports+1]=string.format('{"city":%d,"kind":"%s","off":%.9f,"on":%.9f,"off_again":%.9f,"restored":%.9f,"expected_percent":%d,"expected_delta":%s,"observed_delta":%.9f,"quantity_verified":%s,"reported_step_upper_bound":%s,"reason":"%s"}',a.id,e.kind,av,bv,cv,dv,e.amount,expected and string.format('%.9f',expected) or 'null',observed,tostring(quantity),reporting_step and string.format('%.9f',reporting_step) or 'null',reason)
  end
 end
 for _,kind in ipairs({'production','gold','science','culture','faith','food','growth'}) do
  local affected=false
  for _,e in ipairs(spec) do if e.kind==kind then affected=true end end
  if not affected then
   local av,bv,cv,dv=metric(a,kind),metric(b,kind),metric(c,kind),metric(d,kind)
   if kind~='growth' then assert(close(av,bv) and close(av,cv) and close(av,dv),'non-target city metric changed for '..kind..' city '..a.id)
   else assert(close(a.other,b.other) and close(a.other,c.other) and close(a.other,d.other),'non-target growth modifier changed') end
  end
 end
end
for _,e in ipairs(spec) do
 if e.kind~='military' and e.kind~='naval' and e.kind~='walls' then
  assert(proven[e.kind]>0,'no actual city with measurable '..e.kind..' effect')
 end
end
assert(#base.foreign==#on.foreign and #on.foreign==#off.foreign and #off.foreign==#back.foreign,'foreign city inventory changed')
for i,a in ipairs(base.foreign) do
 local b,c,d=on.foreign[i],off.foreign[i],back.foreign[i]
 assert(a.id==b.id and b.id==c.id and c.id==d.id and a.plot==b.plot and b.plot==c.plot and c.plot==d.plot and a.owner==b.owner and b.owner==c.owner and c.owner==d.owner,'foreign control identity changed')
 for _,kind in ipairs({'production','gold','science','culture','faith','food','growth'}) do
  assert(close(metric(a,kind),metric(b,kind)) and close(metric(b,kind),metric(c,kind)) and close(metric(c,kind),metric(d,kind)),'foreign city changed while testing '..oath..'/'..kind)
 end
 for _,phase in ipairs({'reference','reference_off','precision_one','precision_two','precision_off'}) do
  local sample=all[phase] and all[phase].foreign[i]
  if sample then
   assert(sample.id==a.id and sample.owner==a.owner and sample.plot==a.plot,'reference foreign city changed')
   for _,kind in ipairs({'production','gold','science','culture','faith','food','growth'}) do assert(close(metric(a,kind),metric(sample,kind)),'reference leaked to foreign city') end
  end
 end
end
all.quantities_verified=quantities; all.yield_growth_observed=true
return '{"oath":"'..oath..'","path":"real_ui_selection","quantity_verified":'..tostring(quantities)..',"results":['..table.concat(reports,',')..']}'
"#;

const FINAL: &str = r#"
local oath=__OATH__; local all=assert(JAC_OATH_AUDIT)
local l=assert(Game:GetProperty('JAC_AGES_LEDGER')); local h=assert(l.houses.p0)
assert(h.civ==__CIV__ and l.reign==1 and h.oathChosen==1 and h.oaths[oath]==true,'saved UI choice not restored after diagnostics')
local picked=0; for key,value in pairs(h.oaths) do assert(key==oath and value==true,'another oath persisted'); picked=picked+1 end
assert(picked==1 and all.yield_growth_observed==true,'selection or actual city observations missing')
local original={}; local count=0
for _,group in ipairs({all.baseline.own,all.baseline.foreign}) do for _,city in ipairs(group) do original[city.plot]=city end end
assert(#PlayerManager.GetAliveMajors()==22,'a real house disappeared during production diagnostics')
for _,p in ipairs(PlayerManager.GetAliveMajors()) do for _,city in p:GetCities():Members() do
 local plot=assert(Map.GetPlot(city:GetX(),city:GetY()))
 local old=assert(original[plot:GetIndex()],'a new city appeared during production diagnostics')
 assert(old.owner==p:GetID() and old.id==city:GetID(),'original city owner or identity changed during production diagnostics')
 count=count+1
 assert(city:GetOwner()==p:GetID() and plot:GetOwner()==p:GetID(),'city owner changed')
 for row in GameInfo.JAC_SengokuOaths() do
  local expected=p:GetID()==0 and row.OathType==oath and 1 or 0
  assert((plot:GetProperty(row.OathType..'_ON') or 0)==expected,'diagnostic oath flags not restored')
 end
 assert((plot:GetProperty('JAC_TODO134_CAL_ON') or 0)==0,'100-percent diagnostic reference not disabled')
 for _,key in ipairs({'JAC_TODO134_FINE_ON','JAC_TODO134_SHIFT_ON','JAC_TODO134_SHIFT_TWO_ON','JAC_TODO134_HEADROOM_ON'}) do assert((plot:GetProperty(key) or 0)==0,'precision probe not disabled') end
 for _,kind in ipairs({'PRODUCTION','GOLD','SCIENCE','CULTURE','FAITH','FOOD'}) do assert((plot:GetProperty('JAC_TODO134_BASE_'..kind) or 0)==0,'diagnostic base input not disabled') end
end end
assert(count==56,'original city inventory changed during production diagnostics')
local complete=true
for _,expected in ipairs(__EXPECTED_TARGETS__) do
 local rows=all.production and all.production[expected.target]
 if not rows or rows.quantity_verified~=true or rows.expected_percent~=expected.amount or rows.sample_count~=__TOTAL_SAMPLES__ or not rows.off_first or not rows.on or not rows.off_last then complete=false end
end
all.accepted=all.quantities_verified==true and complete
return '{"oath":"'..oath..'","ui_selection_observed":true,"diagnostic_restore_observed":true,"production_results_complete":'..tostring(complete)..',"accepted":'..tostring(all.accepted)..',"remaining":["save_load","natural_age_transition","quantity_basis_when_undocumented","foreign_specialty_queue"]}'
"#;

const ACCEPTANCE_GATE: &str = "assert(JAC_OATH_AUDIT and JAC_OATH_AUDIT.accepted==true,'quantity acceptance unavailable: inspect capability and observed numeric JSON; do not count this oath as accepted'); return 'numeric acceptance verified'";

fn effect_lua(case: &Case) -> String {
    let effects = case
        .effects
        .iter()
        .map(|e| format!("{{kind={},amount={}}}", lua_string(&e.kind), e.amount))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{effects}}}")
}

fn selected_scenario(base: &Value, case: &Case) -> Result<Value> {
    let mut steps = first_steps(base, &case.civ)?;
    let setup = steps[2]["code"].as_str().context("audit setup")?.to_owned();
    steps[2]["code"] = json!(format!(
        "GameConfiguration.SetValue('JAC_TODO134_QUEUE_HEADROOM',1); {setup}"
    ));
    steps.push(ui("GameCore_Tuner", METER_ATTACH.into()));
    steps.push(ui("GameCore_Tuner", substitute(FIXTURE, case, &[])));
    steps.push(ui(
        "InGame",
        substitute(MEASURE, case, &[("__PHASE__", "\"baseline\"")]),
    ));
    steps.push(precision_toggle(1, 0, 0));
    steps.push(ui(
        "InGame",
        substitute(MEASURE, case, &[("__PHASE__", "\"precision_one\"")]),
    ));
    steps.push(precision_toggle(1, 1, 0));
    steps.push(ui(
        "InGame",
        substitute(MEASURE, case, &[("__PHASE__", "\"precision_two\"")]),
    ));
    steps.push(precision_toggle(0, 0, 0));
    steps.push(ui(
        "InGame",
        substitute(MEASURE, case, &[("__PHASE__", "\"precision_off\"")]),
    ));
    steps.push(meter_toggle(1, 1, 0));
    steps.push(ui(
        "InGame",
        substitute(MEASURE, case, &[("__PHASE__", "\"reference\"")]),
    ));
    steps.push(meter_toggle(0, 1, 0));
    steps.push(ui(
        "InGame",
        substitute(MEASURE, case, &[("__PHASE__", "\"reference_off\"")]),
    ));
    steps.push(ui("DedicationPopup", substitute(POPUP, case, &[])));
    steps.push(json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"GameCore_Tuner","code":format!("local l=Game:GetProperty('JAC_AGES_LEDGER'); local h=l and l.houses.p0; return h and h.oathChosen==1 and h.oaths[{}]==true",lua_string(&case.oath))}}));
    steps.push(ui(
        "InGame",
        substitute(MEASURE, case, &[("__PHASE__", "\"selected\"")]),
    ));
    steps.push(ui(
        "GameCore_Tuner",
        substitute(FLAG, case, &[("__FLAG__", "0")]),
    ));
    steps.push(ui(
        "InGame",
        substitute(MEASURE, case, &[("__PHASE__", "\"off\"")]),
    ));
    steps.push(ui(
        "GameCore_Tuner",
        substitute(FLAG, case, &[("__FLAG__", "1")]),
    ));
    steps.push(ui(
        "InGame",
        substitute(MEASURE, case, &[("__PHASE__", "\"restored\"")]),
    ));
    steps.push(ui(
        "InGame",
        substitute(COMPARE, case, &[("__EFFECTS__", &effect_lua(case))]),
    ));
    // Restore all diagnostic inputs except production, which supplies measurable
    // one-turn progress without inventing an accepted quantity.
    steps.push(meter_toggle(0, 0, 1));
    for effect in &case.effects {
        if ["military", "naval", "walls"].contains(&effect.kind.as_str()) {
            for target in production_targets(&effect.kind) {
                production_steps(&mut steps, case, effect, target, effect.amount);
            }
            // A real, unrelated build measures leakage independently of city production.
            let control = production_control(&effect.kind);
            production_steps(&mut steps, case, effect, control, 0);
        }
    }
    steps.push(ui(
        "GameCore_Tuner",
        substitute(FLAG, case, &[("__FLAG__", "1")]),
    ));
    steps.push(meter_toggle(0, 0, 0));
    let expected_targets = production_expectations(case)
        .iter()
        .map(|(target, amount)| format!("{{target={},amount={amount}}}", lua_string(target)))
        .collect::<Vec<_>>()
        .join(",");
    steps.push(ui(
        "InGame",
        substitute(
            FINAL,
            case,
            &[
                ("__EXPECTED_TARGETS__", &format!("{{{expected_targets}}}")),
                ("__TOTAL_SAMPLES__", &(3 * PRODUCTION_SAMPLES).to_string()),
            ],
        ),
    ));
    steps.push(ui("InGame", ACCEPTANCE_GATE.to_owned()));
    Ok(
        json!({"name":format!("Oath {}: finite numeric diagnosis after real UI selection; unresolved quantities are not accepted",case.oath),"judge_profile":"generic","finite_turns":bounded_turns(&steps),"steps":steps}),
    )
}

fn production_targets(kind: &str) -> &'static [&'static str] {
    match kind {
        "military" => &["UNIT_SPEARMAN"],
        "naval" => &["UNIT_GALLEY"],
        "walls" => &["BUILDING_WALLS", "BUILDING_CASTLE", "BUILDING_STAR_FORT"],
        _ => &[],
    }
}

fn production_control(kind: &str) -> &'static str {
    match kind {
        "walls" => "BUILDING_MONUMENT",
        "naval" => "UNIT_SPEARMAN",
        _ => "UNIT_BUILDER",
    }
}

fn production_expectations(case: &Case) -> Vec<(String, i64)> {
    let mut targets = Vec::new();
    for effect in &case.effects {
        for target in production_targets(&effect.kind) {
            targets.push((target.to_string(), effect.amount));
        }
        if !production_targets(&effect.kind).is_empty() {
            targets.push((production_control(&effect.kind).to_string(), 0));
        }
    }
    targets
}

const PRODUCTION_FIXTURE: &str = r#"
local oath=__OATH__; local target=__TARGET__; local kind=__KIND__
local building=string.sub(target,1,9)=='BUILDING_'
local def=assert((building and GameInfo.Buildings[target]) or GameInfo.Units[target],'production definition missing')
-- Stage only the loaded target's prerequisites. Unlocking every technology would
-- obsolete the tested unit and enable urban defenses that prohibit wall builds.
local function unlock(row)
 if row.PrereqTech then Players[0]:GetTechs():SetTech(assert(GameInfo.Technologies[row.PrereqTech]).Index,true) end
 if row.PrereqCivic then Players[0]:GetCulture():SetCivic(assert(GameInfo.Civics[row.PrereqCivic]).Index,true) end
end
unlock(def)
local optin=GameConfiguration.GetValue('JAC_TODO134_QUEUE_HEADROOM')
assert(optin==1 or optin==true,'opt-in queue cost fixture missing')
assert(def.Cost==32768,'queue headroom cost data not loaded')
if not Game:GetProperty('JAC_TODO134_HEADROOM_ATTACHED') then
 Players[0]:AttachModifierByID('JAC_TODO134_HEADROOM_PRODUCTION')
 Players[0]:AttachModifierByID('JAC_TODO134_HEADROOM_GROWTH')
 Game:SetProperty('JAC_TODO134_HEADROOM_ATTACHED',true)
end
local lower={}
if target=='BUILDING_CASTLE' or target=='BUILDING_STAR_FORT' then lower[#lower+1]=assert(GameInfo.Buildings.BUILDING_WALLS) end
if target=='BUILDING_STAR_FORT' then lower[#lower+1]=assert(GameInfo.Buildings.BUILDING_CASTLE) end
for _,row in ipairs(lower) do
 unlock(row)
 for _,city in Players[0]:GetCities():Members() do
  if not city:GetBuildings():HasBuilding(row.Index) then
   local q=assert(city:GetBuildQueue())
   assert(type(q.CreateIncompleteBuilding)=='function','official Tuner building fixture API unavailable')
   q:CreateIncompleteBuilding(row.Index,Map.GetPlot(city:GetX(),city:GetY()):GetIndex(),100)
  end
  assert(city:GetBuildings():HasBuilding(row.Index),'wall prerequisite could not be staged')
 end
end
Game:SetProperty('JAC_OATH_PRODUCTION_TEST',{Oath=oath,Target=target,Kind=kind,Building=building,Amount=__AMOUNT__})
return 'minimal prerequisite fixture staged for '..target
"#;

const PRODUCTION_REQUEST: &str = r#"
local source=assert(Game:GetProperty('JAC_OATH_PRODUCTION_TEST'))
assert(source.Target==__TARGET__ and source.Kind==__KIND__,'production test target changed')
local t={Target=source.Target,Kind=source.Kind,Building=source.Building,Amount=source.Amount,Oath=source.Oath}
assert(Players[0]:IsTurnActive() and Game.GetLocalPlayer()==0,'actual human build request requires player 0 active')
local def=assert((t.Building and GameInfo.Buildings[t.Target]) or GameInfo.Units[t.Target])
local params={}
if t.Building then params[CityOperationTypes.PARAM_BUILDING_TYPE]=def.Hash
else params[CityOperationTypes.PARAM_UNIT_TYPE]=def.Hash end
params[CityOperationTypes.PARAM_INSERT_MODE]=CityOperationTypes.VALUE_REPLACE_AT
params[CityOperationTypes.PARAM_QUEUE_DESTINATION_LOCATION]=0
local chosen; local cheapest=math.huge
for _,city in Players[0]:GetCities():Members() do
 local plot=assert(Map.GetPlot(city:GetX(),city:GetY()))
 local can=CityManager.CanStartOperation(city,CityOperationTypes.BUILD,params,true)
 if can and city:GetOwner()==0 and plot:GetOwner()==0 and (def.Domain~='DOMAIN_SEA' or plot:IsCoastalLand()) then
  local q=assert(city:GetBuildQueue())
  local progress=number(t.Building and q:GetBuildingProgress(def.Index) or q:GetUnitProgress(def.Index),'candidate progress')
  local cost=number(t.Building and q:GetBuildingCost(def.Index) or q:GetUnitCost(def.Index),'candidate cost')
  local production=number(city:GetYield(GameInfo.Yields.YIELD_PRODUCTION.Index),'candidate city production')
  if progress>=0 and production>=5 and cost-progress>production*4 and production<cheapest then chosen=city; cheapest=production end
 end
end
assert(chosen,'no eligible actual owned city has three-turn headroom and sufficient production precision for '..t.Target)
t.City=chosen:GetID(); t.Plot=Map.GetPlot(chosen:GetX(),chosen:GetY()):GetIndex(); t.Owner=0
JAC_OATH_AUDIT.current=t
CityManager.RequestOperation(chosen,CityOperationTypes.BUILD,params)
return 'official BUILD operation requested for '..t.Target..' in actual owned city '..t.City
"#;

const PRODUCTION_TOGGLE: &str = r#"
local oath=__OATH__; local target=__TARGET__; local value=__FLAG__
local t=assert(Game:GetProperty('JAC_OATH_PRODUCTION_TEST'))
assert(t.Target==target and t.Oath==oath,'production target changed')
local l=assert(Game:GetProperty('JAC_AGES_LEDGER')); local h=assert(l.houses.p0)
assert(l.reign==1 and h.oathChosen==1 and h.civ==__CIV__,'real UI selection no longer recorded')
h.oaths=value==1 and {[oath]=true} or {}
Game:SetProperty('JAC_AGES_LEDGER',l)
for _,city in Players[0]:GetCities():Members() do
 local plot=assert(Map.GetPlot(city:GetX(),city:GetY()))
 assert(city:GetOwner()==0 and plot:GetOwner()==0,'production city owner changed')
plot:SetProperty(oath..'_ON',value)
end
for _,city in Players[0]:GetCities():Members() do
 local p=assert(Map.GetPlot(city:GetX(),city:GetY()))
 p:SetProperty('JAC_TODO134_HEADROOM_ON',1)
 p:SetProperty('JAC_TODO134_FINE_ON',__PROBE_FINE__)
 p:SetProperty('JAC_TODO134_SHIFT_ON',__PROBE_SHIFT__)
 p:SetProperty('JAC_TODO134_SHIFT_TWO_ON',__PROBE_SECOND__)
end
return 'production turn '..__PHASE__..' oath flag='..value
"#;

const PRODUCTION_CORE: &str = r#"
local target=__TARGET__; local phase=__PHASE__; local stage=__STAGE__; local sample=__SAMPLE__
assert(assert(Game:GetProperty('JAC_OATH_PRODUCTION_TEST')).Target==target,'core production target changed')
local rows={}
for _,city in Players[0]:GetCities():Members() do
 local plot=assert(Map.GetPlot(city:GetX(),city:GetY()))
 assert(city:GetOwner()==0 and plot:GetOwner()==0,'core production city owner changed')
 rows[#rows+1]={city=city:GetID(),plot=plot:GetIndex(),owner=0,production=number(city:GetYield(GameInfo.Yields.YIELD_PRODUCTION.Index),'core production')}
end
assert(#rows>0,'core production cities missing')
Game:SetProperty('JAC_TODO134_QUEUE_CORE',{target=target,phase=phase,stage=stage,sample=sample,turn=Game.GetCurrentGameTurn(),cities=rows})
return 'core production snapshot '..target..'/'..phase..'/'..stage
"#;

const PRODUCTION_CORE_READER: &str = r#"
local function core_production(target,phase,stage,sample,t)
 local data=assert(Game:GetProperty('JAC_TODO134_QUEUE_CORE'),'core production snapshot missing')
 assert(data.target==target and data.phase==phase and data.stage==stage and data.sample==sample,'core production snapshot identity changed')
 assert(data.turn==Game.GetCurrentGameTurn(),'core and UI production turns differ')
 local found=nil
 for _,row in ipairs(assert(data.cities)) do
  if row.city==t.City then
   assert(not found,'duplicate core production city')
   assert(row.plot==t.Plot and row.owner==0,'core and UI production city differ')
   found=number(row.production,'core city production')
  end
 end
 return assert(found,'selected city missing from core production snapshot')
end
"#;

const PRODUCTION_BEFORE: &str = r#"
local target=__TARGET__; local phase=__PHASE__; local sample=__SAMPLE__
local t=assert(assert(JAC_OATH_AUDIT).current,'actual build request missing')
assert(t.Target==target,'production target changed')
local city=assert(Players[0]:GetCities():FindID(t.City))
assert(city:GetOwner()==0 and Map.GetPlot(city:GetX(),city:GetY()):GetIndex()==t.Plot,'actual city changed')
local q=assert(city:GetBuildQueue())
local def=assert((t.Building and GameInfo.Buildings[target]) or GameInfo.Units[target])
assert(q:GetCurrentProductionTypeHash()==def.Hash,'target is not actually being produced')
assert(Map.GetPlotByIndex(t.Plot):GetProperty(__OATH__..'_ON')==__FLAG__,'city diagnostic flag differs from requested phase')
local progress=t.Building and q:GetBuildingProgress(def.Index) or q:GetUnitProgress(def.Index)
local cost=t.Building and q:GetBuildingCost(def.Index) or q:GetUnitCost(def.Index)
local ui_production=number(city:GetYield(GameInfo.Yields.YIELD_PRODUCTION.Index),'UI city production')
local production=core_production(target,phase,'before',sample,t)
assert(production>4000 and cost-progress>production*1.5,'queue has insufficient reporting precision or completion headroom')
assert(type(progress)=='number' and type(cost)=='number' and type(production)=='number' and progress==progress and cost==cost and production==production and cost<math.huge and production<math.huge and cost>progress and production>0,'no finite numeric active production')
JAC_OATH_AUDIT=assert(JAC_OATH_AUDIT,'UI audit context was lost')
JAC_OATH_AUDIT.production=JAC_OATH_AUDIT.production or {}
JAC_OATH_AUDIT.production[target]=JAC_OATH_AUDIT.production[target] or {}
local rows=JAC_OATH_AUDIT.production[target]
rows[phase]=rows[phase] or {samples={}}
assert(rows[phase].samples[sample]==nil,'do not overwrite a production observation')
rows[phase].samples[sample]={before=progress,cost=cost,city_yield=production,ui_city_yield=ui_production,city=t.City,plot=t.Plot,turn=Game.GetCurrentGameTurn()}
return string.format('{"target":"%s","phase":"%s","sample":%d,"city":%d,"progress_before":%.9f,"cost":%.9f,"core_city_production":%.9f,"ui_city_production":%.9f}',target,phase,sample,t.City,progress,cost,production,ui_production)
"#;

const PRODUCTION_AFTER: &str = r#"
local target=__TARGET__; local phase=__PHASE__; local sample=__SAMPLE__
local t=assert(assert(JAC_OATH_AUDIT).current)
assert(t.Target==target,'production target changed')
local city=assert(Players[0]:GetCities():FindID(t.City))
assert(city:GetOwner()==0 and Map.GetPlot(city:GetX(),city:GetY()):GetIndex()==t.Plot,'actual city changed after turn')
local q=assert(city:GetBuildQueue())
local def=assert((t.Building and GameInfo.Buildings[target]) or GameInfo.Units[target])
assert(q:GetCurrentProductionTypeHash()==def.Hash,'target completed or AI changed actual build queue')
local progress=number(t.Building and q:GetBuildingProgress(def.Index) or q:GetUnitProgress(def.Index),'actual queue progress after turn')
local r=assert(assert(JAC_OATH_AUDIT).production[target][phase].samples[sample],'start-of-turn production sample missing')
assert(Game.GetCurrentGameTurn()==r.turn+1,'production observation must span exactly one turn')
assert(r.end_requested==true,'turn was not advanced by the bounded native request')
assert(Map.GetPlotByIndex(t.Plot):GetProperty(__OATH__..'_ON')==__FLAG__,'ledger overwrote the diagnostic phase')
local cost=number(t.Building and q:GetBuildingCost(def.Index) or q:GetUnitCost(def.Index),'actual queue cost after turn')
assert(math.abs(cost-r.cost)<0.00002,'production cost changed during one-turn observation')
assert(type(progress)=='number' and progress>r.before and progress<r.cost,'actual queue progress did not advance without completion')
r.after=progress; r.delta=progress-r.before
r.city_yield_after=core_production(target,phase,'after',sample,t)
r.ui_city_yield_after=number(city:GetYield(GameInfo.Yields.YIELD_PRODUCTION.Index),'end-of-turn UI production')
return string.format('{"target":"%s","phase":"%s","sample":%d,"city":%d,"progress_before":%.9f,"progress_after":%.9f,"observed_delta":%.9f,"core_city_production_before":%.9f,"core_city_production_after":%.9f,"ui_city_production_before":%.9f,"ui_city_production_after":%.9f}',target,phase,sample,t.City,r.before,progress,r.delta,r.city_yield,r.city_yield_after,r.ui_city_yield,r.ui_city_yield_after)
"#;

const PRODUCTION_END_TURN: &str = r#"
local target=__TARGET__; local phase=__PHASE__; local sample=__SAMPLE__
local r=assert(assert(JAC_OATH_AUDIT).production[target][phase].samples[sample])
assert(Game.GetCurrentGameTurn()==r.turn and Players[0]:IsTurnActive(),'expected active human turn before production measurement')
assert(not r.end_requested,'do not repeat a native end-turn request')
r.end_requested=true
-- Official Shift+Enter path: Base/Assets/UI/ActionPanel.lua:1099.
-- This ends exactly one human turn without handing the queue to the AI.
UI.RequestAction(ActionTypes.ACTION_ENDTURN,{REASON='UserForced'})
return 'single native human end-turn requested for '..target..'/'..phase
"#;

const PRODUCTION_COMPARE: &str = r#"
local target=__TARGET__; local amount=__AMOUNT__
local t=assert(assert(JAC_OATH_AUDIT).current)
assert(t.Target==target,'production target changed')
local rows=assert(assert(JAC_OATH_AUDIT).production[target])
local warmup=assert(assert(rows.warmup).samples[1],'non-measurement conditioning turn missing')
number(warmup.turn,'conditioning turn'); number(warmup.before,'conditioning progress before'); number(warmup.after,'conditioning progress after'); number(warmup.cost,'conditioning cost')
assert(warmup.end_requested==true and warmup.after>warmup.before and warmup.after<warmup.cost,'conditioning turn did not advance without completion')
local previous=nil; local cost=nil; local city=nil; local plot=nil; local stable=true; local readings={}
local function summarize(name)
 local samples=assert(assert(rows[name]).samples,'phase samples missing')
 assert(#samples==__SAMPLES_PER_PHASE__,'phase needs every finite turn sample')
 local delta,yield=0,0
 for _,r in ipairs(samples) do
  number(r.before,'sample before'); number(r.after,'sample after'); number(r.delta,'sample delta')
  readings[#readings+1]=r.before; readings[#readings+1]=r.after
  number(r.city_yield,'sample production before'); number(r.city_yield_after,'sample production after')
  number(r.ui_city_yield,'sample UI production before'); number(r.ui_city_yield_after,'sample UI production after')
  assert(r.delta>0 and r.city_yield>0 and math.abs(r.after-r.before-r.delta)<0.00002,'invalid actual progress sample')
  if previous then assert(r.turn==previous.turn+1 and math.abs(r.before-previous.after)<0.00002,'production samples are not continuous consecutive turns')
  else assert(r.turn==warmup.turn+1 and r.city==warmup.city and r.plot==warmup.plot and r.cost==warmup.cost and math.abs(r.before-warmup.after)<0.00002,'measurement is not continuous from conditioning turn') end
  if city then assert(r.city==city and r.plot==plot,'production source city changed') else city=r.city; plot=r.plot end
  if cost then assert(math.abs(r.cost-cost)<0.00002,'queue cost changed between samples') else cost=number(r.cost,'sample cost') end
  if math.abs(r.city_yield_after-r.city_yield)>0.00002 then stable=false end
  if math.abs(r.ui_city_yield-r.city_yield)>0.00002 or math.abs(r.ui_city_yield_after-r.city_yield_after)>0.00002 then stable=false end
  delta=delta+r.delta; yield=yield+r.city_yield; previous=r
 end
 return {delta=delta,city_yield=yield}
end
local first=summarize('off_first'); local active=summarize('on'); local last=summarize('off_last')
local off1=first.delta/first.city_yield; local on=active.delta/active.city_yield; local off2=last.delta/last.city_yield
number(off1,'off first production rate'); number(on,'on production rate'); number(off2,'off last production rate')
local step=reported_step(readings)
local tolerance=step and 2*step/math.min(first.city_yield,active.city_yield,last.city_yield)+0.00004 or math.huge
-- An interval must be narrower than half a one-percentage-point step. Otherwise
-- neighbouring integer amounts cannot be distinguished and quantity stays false.
local precise=tolerance<0.005
local baseline=(off1+off2)/2
assert(math.abs(off1-off2)<=tolerance,'off production rate did not restore')
local unit_basis=math.abs(off1-1)<=tolerance and math.abs(off2-1)<=tolerance
local expected=nil; local quantity=false
if not stable or not precise then
 quantity=false
elseif amount==0 then
 assert(math.abs(on-baseline)<=tolerance,'oath specialty leaked to a non-target actual build')
 expected=baseline; quantity=true
elseif unit_basis then
 -- Here the observed OFF queue rate equals the measured city production in both
 -- flanking turns. Additive and multiplicative interpretations coincide at 1.
 expected=1+amount/100
 assert(math.abs(on-expected)<=tolerance,string.format('%s actual rate %.9f expected %.9f',target,on,expected))
 quantity=true
else
 assert(on>baseline+tolerance,'specialty did not increase actual normalized queue progress')
end
local all=assert(JAC_OATH_AUDIT)
all.production_quantities_verified=all.production_quantities_verified~=false and quantity
rows.quantity_verified=quantity
rows.expected_percent=amount
rows.sample_count=3*__SAMPLES_PER_PHASE__
return string.format('{"target":"%s","path":"actual_queue_diagnostic_after_ui_selection","city":%d,"expected_percent":%d,"off_first":%.9f,"on":%.9f,"off_last":%.9f,"expected_on":%s,"observed_delta":%.9f,"tolerance":%s,"reported_step_upper_bound":%s,"integer_percentage_resolved":%s,"city_production_stable_inside_turns":%s,"unmodified_off_basis_observed":%s,"quantity_verified":%s,"restored":true}',target,t.City,amount,off1,on,off2,expected and string.format('%.9f',expected) or 'null',on-baseline,finite(tolerance) and string.format('%.9f',tolerance) or 'null',step and string.format('%.9f',step) or 'null',tostring(precise),tostring(stable),tostring(unit_basis),tostring(quantity))
"#;

fn production_code(
    template: &str,
    case: &Case,
    effect: &Effect,
    target: &str,
    phase: &str,
    flag: i64,
) -> String {
    let code = substitute(
        template,
        case,
        &[
            ("__TARGET__", &lua_string(target)),
            ("__KIND__", &lua_string(&effect.kind)),
            ("__PHASE__", &lua_string(phase)),
            ("__AMOUNT__", &effect.amount.to_string()),
            ("__FLAG__", &flag.to_string()),
            ("__SAMPLES_PER_PHASE__", &PRODUCTION_SAMPLES.to_string()),
        ],
    );
    if template == PRODUCTION_BEFORE || template == PRODUCTION_AFTER {
        format!("{PRODUCTION_CORE_READER}\n{code}")
    } else {
        code
    }
}

fn production_steps(
    steps: &mut Vec<Value>,
    case: &Case,
    effect: &Effect,
    target: &str,
    amount: i64,
) {
    let expected = Effect {
        kind: effect.kind.clone(),
        amount,
    };
    let effect = &expected;
    steps.push(ui(
        "GameCore_Tuner",
        production_code(PRODUCTION_FIXTURE, case, effect, target, "", 1),
    ));
    steps.push(ui(
        "InGame",
        production_code(PRODUCTION_REQUEST, case, effect, target, "", 1),
    ));
    steps.push(json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":format!("local t=JAC_OATH_AUDIT and JAC_OATH_AUDIT.current; local c=t and Players[0]:GetCities():FindID(t.City); return c and c:GetBuildQueue():GetCurrentProductionTypeHash()==GameInfo.{}[{}].Hash",if target.starts_with("BUILDING_") {"Buildings"} else {"Units"},lua_string(target))}}));
    for (phase, flag) in [("warmup", 0), ("off_first", 0), ("on", 1), ("off_last", 0)] {
        steps.push(ui(
            "GameCore_Tuner",
            production_code(PRODUCTION_TOGGLE, case, effect, target, phase, flag)
                .replace("__PROBE_FINE__", "0")
                .replace("__PROBE_SHIFT__", "0")
                .replace("__PROBE_SECOND__", "0"),
        ));
        for sample in 1..=PRODUCTION_SAMPLES {
            steps.push(ui(
                "GameCore_Tuner",
                production_code(PRODUCTION_CORE, case, effect, target, phase, flag)
                    .replace("__STAGE__", "'before'")
                    .replace("__SAMPLE__", &sample.to_string()),
            ));
            for template in [PRODUCTION_BEFORE, PRODUCTION_END_TURN] {
                steps.push(ui(
                    "InGame",
                    production_code(template, case, effect, target, phase, flag)
                        .replace("__SAMPLE__", &sample.to_string()),
                ));
            }
            steps.push(json!({"action":"wait_for","timeout":180,"close_diplomacy":true,"condition":{"type":"lua","state_name":"InGame","code":format!("local a=JAC_OATH_AUDIT; local r=a and a.production[{}][{}].samples[{}]; if not r then return false end; local turn=Game.GetCurrentGameTurn(); assert(turn<=r.turn+1,'native request advanced more than one turn'); return turn==r.turn+1 and Players[0]:IsTurnActive()",lua_string(target),lua_string(phase),sample)}}));
            steps.push(ui(
                "GameCore_Tuner",
                production_code(PRODUCTION_CORE, case, effect, target, phase, flag)
                    .replace("__STAGE__", "'after'")
                    .replace("__SAMPLE__", &sample.to_string()),
            ));
            steps.push(ui(
                "InGame",
                production_code(PRODUCTION_AFTER, case, effect, target, phase, flag)
                    .replace("__SAMPLE__", &sample.to_string()),
            ));
        }
    }
    steps.push(ui(
        "InGame",
        production_code(PRODUCTION_COMPARE, case, effect, target, "", 1),
    ));
    steps.push(ui(
        "GameCore_Tuner",
        production_code(PRODUCTION_TOGGLE, case, effect, target, "restore", 1)
            .replace("__PROBE_FINE__", "0")
            .replace("__PROBE_SHIFT__", "0")
            .replace("__PROBE_SECOND__", "0"),
    ));
}

pub fn outputs(repo: &Repository) -> Result<Outputs> {
    let base = repo.json(BASE)?;
    let cases = cases(repo)?;
    let mut result = meter_outputs();
    let capability = capability_scenario(&base)?;
    result.insert(
        PathBuf::from(format!("{DIR}/capabilities.json")),
        format!("{}\n", serde_json::to_string_pretty(&capability)?),
    );
    let mut inventory = Vec::new();
    for case in cases {
        let scenario = selected_scenario(&base, &case)?;
        let path = format!("{DIR}/{}.json", case.oath.to_lowercase());
        let turns = bounded_turns(scenario["steps"].as_array().context("generated steps")?);
        let mut targets = Vec::new();
        for effect in &case.effects {
            for target in production_targets(&effect.kind) {
                targets.push(
                    json!({"target":target,"expected_percent":effect.amount,"role":"subject"}),
                );
            }
            if !production_targets(&effect.kind).is_empty() {
                targets.push(json!({"target":production_control(&effect.kind),"expected_percent":0,"role":"non_target_control"}));
            }
        }
        inventory.push(json!({"house":case.house,"civilization":case.civ,"oath":case.oath,"scenario":path,"turns":turns,
            "effects":case.effects.iter().map(|e| json!({"kind":e.kind,"expected_percent":e.amount})).collect::<Vec<_>>(),"production_targets":targets}));
        result.insert(
            PathBuf::from(path),
            format!("{}\n", serde_json::to_string_pretty(&scenario)?),
        );
    }
    let manifest = json!({"live_verified":false,"capability_scenario":format!("{DIR}/capabilities.json"),"oath_count":66,"house_count":22,"max_turns":16,
        "quantity_acceptance":"native_100_percent_yield_reference_and_observed_unmodified_production_basis; unresolved quantities never pass","diagnostic_mod":METER_ID,"scenarios":inventory});
    result.insert(
        PathBuf::from(INVENTORY),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    );
    Ok(result)
}

pub fn cli(repo: &Repository, args: &[String]) -> Result<()> {
    let Some(check) = crate::parse_check_args(
        "generate-oath-effect-audit",
        args,
        "Generate finite live oath-effect scenarios from the 66-entry catalogue.",
    )?
    else {
        return Ok(());
    };
    let generated = outputs(repo)?;
    if repo.path(DIR).is_dir() {
        for entry in std::fs::read_dir(repo.path(DIR))? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("json"))
            {
                let relative = PathBuf::from(DIR).join(entry.file_name());
                if !generated.contains_key(&relative) {
                    bail!(
                        "unexpected stale audit scenario: {}",
                        entry.path().display()
                    );
                }
            }
        }
    }
    crate::apply_outputs(repo, generated, check)?;
    println!(
        "oath effect audit: capability + 66 finite selected-path scenarios + inventory; live acceptance not implied; {}",
        if check { "up to date" } else { "generated" }
    );
    Ok(())
}

#[cfg(test)]
#[path = "oath_effect_audit_tests.rs"]
mod execution_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    #[test]
    fn catalogue_cases_have_exact_live_selection_and_bounded_progress_checks() -> Result<()> {
        let repo = Repository::checked_in();
        let generated = outputs(&repo)?;
        assert_eq!(generated.len(), 73);
        let mut paths = BTreeSet::new();
        for case in cases(&repo)? {
            let path = PathBuf::from(format!("{DIR}/{}.json", case.oath.to_lowercase()));
            assert!(paths.insert(path.clone()));
            let scenario: Value = serde_json::from_str(&generated[&path])?;
            let steps = scenario["steps"].as_array().context("scenario steps")?;
            assert_eq!(steps[0]["action"], "enable_mods");
            assert_eq!(steps[1]["players"][0]["civ"], case.civ);
            assert_eq!(steps[1]["players"][0]["human"], true);
            assert_eq!(steps[3]["action"], "start_game");
            let all_codes = steps
                .iter()
                .filter_map(|step| step["code"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(all_codes.contains("JAC_ChooseOath(oath)"));
            assert!(all_codes.contains("GetOverallGrowthModifier"));
            assert!(
                all_codes.contains("GetUnitProgress")
                    || all_codes.contains("GetBuildingProgress")
                    || !case
                        .effects
                        .iter()
                        .any(|e| ["military", "naval", "walls"].contains(&e.kind.as_str()))
            );
            assert!(!all_codes.contains("__OATH__"));
            assert!(!all_codes.contains("__TARGET__"));
            let turns = bounded_turns(steps);
            let expected = if case.effects.iter().any(|e| e.kind == "walls") {
                16
            } else if case
                .effects
                .iter()
                .any(|e| e.kind == "military" || e.kind == "naval")
            {
                8
            } else {
                0
            };
            assert_eq!(turns, expected, "{}", case.oath);
            assert!(turns <= 16);
            assert!(steps.iter().all(|s| s["action"] != "autoplay"));
            assert!(steps.last().context("last step")?["code"]
                .as_str()
                .context("acceptance code")?
                .contains("quantity acceptance unavailable"));
        }
        assert_eq!(paths.len(), 66);
        Ok(())
    }

    #[test]
    fn generated_lua_compiles_as_lua_51_without_game() -> Result<()> {
        let repo = Repository::checked_in();
        let lua = Lua::new();
        let mut compiled = 0;
        for (path, content) in outputs(&repo)? {
            if !path.starts_with(DIR) {
                continue;
            }
            let scenario: Value = serde_json::from_str(&content)?;
            for step in scenario["steps"].as_array().context("steps")? {
                if let Some(code) = step["code"].as_str() {
                    lua.load(code)
                        .set_name(path.to_string_lossy().as_ref())
                        .into_function()
                        .with_context(|| format!("{}: step code", path.display()))?;
                    compiled += 1;
                }
                if let Some(code) = step["condition"]["code"].as_str() {
                    lua.load(code)
                        .set_name(path.to_string_lossy().as_ref())
                        .into_function()
                        .with_context(|| format!("{}: wait condition", path.display()))?;
                    compiled += 1;
                }
            }
        }
        assert!(
            compiled > 1000,
            "only {compiled} generated test chunks checked"
        );
        Ok(())
    }

    #[test]
    fn catalogue_amount_change_updates_expected_live_delta() -> Result<()> {
        let repo = Repository::checked_in();
        let original = outputs(&repo)?;
        let source = repo.read("documents/poc/data/sengoku-oaths.json")?;
        let dir = tempfile::tempdir()?;
        let copied = Repository::new(dir.path());
        for relative in [BASE, "documents/poc/data/sengoku-oaths.json"] {
            let target = copied.path(relative);
            std::fs::create_dir_all(target.parent().context("target parent")?)?;
            std::fs::write(&target, repo.read(relative)?)?;
        }
        let mut catalogue: Value = serde_json::from_str(&source)?;
        assert_eq!(
            catalogue["houses"][0]["oaths"][0]["effects"][0][0],
            "military"
        );
        catalogue["houses"][0]["oaths"][0]["effects"][0][1] = json!(17);
        std::fs::write(
            copied.path("documents/poc/data/sengoku-oaths.json"),
            serde_json::to_vec(&catalogue)?,
        )?;
        let key = PathBuf::from(format!("{DIR}/jac_oath_oda_tenka_fubu.json"));
        let altered = outputs(&copied)?;
        assert_ne!(original[&key], altered[&key]);
        assert!(altered[&key].contains("local amount=17"));
        Ok(())
    }
}
