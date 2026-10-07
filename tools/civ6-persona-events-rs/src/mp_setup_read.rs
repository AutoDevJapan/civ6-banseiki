//! Read all setup facts before selecting another Host experiment. No Host/start.
//! Manifest predicates are expectations from the candidate modinfo files, not
//! evidence that InGameActions executed while still in the FrontEnd.
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};
fn lua(v: &Value) -> String {
    match v {
        Value::Null => "nil".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!(
            "'{}'",
            s.replace('\\', "\\\\")
                .replace('\'', "\\'")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
        ),
        Value::Array(a) => format!("{{{}}}", a.iter().map(lua).collect::<Vec<_>>().join(",")),
        Value::Object(m) => format!(
            "{{{}}}",
            m.iter()
                .map(|(k, v)| format!("[{}]={}", lua(&json!(k)), lua(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}
pub fn manifest(root: &Path, ids: &[Value]) -> Result<Value, String> {
    let mut found = BTreeMap::new();
    for d in std::fs::read_dir(root.join("poc")).map_err(|e| e.to_string())? {
        let d = d.map_err(|e| e.to_string())?;
        if !d.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(d.path()).map_err(|e| e.to_string())? {
            let f = f.map_err(|e| e.to_string())?;
            if f.path().extension().and_then(|e| e.to_str()) != Some("modinfo") {
                continue;
            }
            let text = std::fs::read_to_string(f.path()).map_err(|e| e.to_string())?;
            let doc = roxmltree::Document::parse(&text).map_err(|e| e.to_string())?;
            let m = doc.root_element();
            let id = m.attribute("id").unwrap_or("");
            if !ids.iter().any(|wanted| wanted.as_str() == Some(id)) {
                continue;
            }
            let mut criteria = serde_json::Map::new();
            if let Some(c) = m.children().find(|n| n.has_tag_name("ActionCriteria")) {
                for c in c.children().filter(|n| n.is_element()) {
                    let mut conditions = vec![];
                    for p in c.children().filter(|n| n.is_element()) {
                        let value = match p.tag_name().name() {
                            "RuleSetInUse" => {
                                json!({"kind":"ruleset","values":p.text().unwrap_or("").split(',').map(str::trim).collect::<Vec<_>>()})
                            }
                            "ConfigurationValueMatches" => {
                                let get = |name| {
                                    p.children()
                                        .find(|n| n.has_tag_name(name))
                                        .and_then(|n| n.text())
                                        .unwrap_or("")
                                        .to_owned()
                                };
                                json!({"kind":"config","group":get("Group"),"key":get("ConfigurationId"),"expected":get("Value")})
                            }
                            other => json!({"kind":"unsupported","tag":other}),
                        };
                        conditions.push(value)
                    }
                    criteria.insert(c.attribute("id").unwrap_or("").into(), json!(conditions));
                }
            }
            let mut actions = vec![];
            for phase in ["FrontEndActions", "InGameActions"] {
                if let Some(section) = m.children().find(|n| n.has_tag_name(phase)) {
                    for a in section.children().filter(|n| n.is_element()) {
                        actions.push(json!({"phase":phase,"kind":a.tag_name().name(),"id":a.attribute("id"),"criteria":a.attribute("criteria")}));
                    }
                }
            }
            if found.insert(id.to_owned(),json!({"id":id,"file":f.path().strip_prefix(root).unwrap_or(&f.path()).to_string_lossy(),"criteria":criteria,"actions":actions})).is_some(){return Err(format!("duplicate root MOD id {id}"))}
        }
    }
    let mut result = vec![];
    for id in ids {
        let id = id.as_str().ok_or("nonstring modid")?;
        result.push(
            found
                .remove(id)
                .ok_or_else(|| format!("root MOD manifest missing {id}"))?,
        )
    }
    Ok(json!(result))
}
pub fn reader(manifest: &Value) -> String {
    format!(
        "local manifests={}\n{}\n{}",
        lua(manifest),
        crate::sp_runtime::SERIAL,
        READ
    )
}
const READ: &str = r#"
local report={scope='setup_read_only_no_host',measurements={},slots={},mods={},criteria={},actions={},
 actual_action_execution='not_measured_before_host',native_player_capacity='not_measured',
 reference={staging_max_slots=12,staging_qa_supported=8,
 rule='stock staging rejects TAKEN/COMPUTER full-civ slots whose index >= min(mapMax,12); map default is not native capacity'}}
local function scalar(v)
 local t=type(v);if t=='number' or t=='string' or t=='boolean' then return {type=t,value=v}end
 return {type=t,text=tostring(v)}
end
local function read(key,f)
 local item={status='STARTED'};report.measurements[key]=item
 local function pack(...)local out={arity=select('#',...),values={}};for i=1,out.arity do out.values[i]=scalar(select(i,...))end;return out end
 local ok,value=pcall(function()return pack(f())end)
 if ok then item.status=(value.arity==0 or (value.arity==1 and value.values[1].type=='nil')) and 'MISSING' or 'OK';item.result=value
 else item.status='ERROR';item.error=tostring(value)end
 print('MP_SETUP_FACT '..key..' '..serial(item));return item
end
local function value(key)local item=report.measurements[key];local v=item and item.result and item.result.values[1];if item and item.status=='OK' and v then return v.value,v.type end end
read('frontend',function()return UI.IsInFrontEnd()end)
read('hotseat',function()return GameConfiguration.IsHotseat()end)
read('network_multiplayer',function()return GameConfiguration.IsNetworkMultiplayer()end)
read('ruleset',function()return GameConfiguration.GetRuleSet()end)
read('map.script',function()return MapConfiguration.GetValue('MAP_SCRIPT')end)
read('map.domain',function()return MapConfiguration.GetValue('MAP_DOMAIN')end)
read('map.size',function()return MapConfiguration.GetMapSize()end)
read('map.size_value',function()return MapConfiguration.GetValue('MAP_SIZE')end)
read('map.min_major',function()return MapConfiguration.GetMinMajorPlayers()end)
read('map.max_major',function()return MapConfiguration.GetMaxMajorPlayers()end)
read('map.min_minor',function()return MapConfiguration.GetMinMinorPlayers()end)
read('map.max_minor',function()return MapConfiguration.GetMaxMinorPlayers()end)
read('participants',function()return GameConfiguration.GetParticipatingPlayerCount()end)
read('hidden_count',function()return GameConfiguration.GetHiddenPlayerCount()end)
read('city_states',function()return GameConfiguration.GetValue('CITY_STATE_COUNT')end)
read('network.local_player',function()return Network.GetLocalPlayerID()end)
read('game.local_player',function()return Game.GetLocalPlayer()end)
read('slot.taken',function()return SlotStatus.SS_TAKEN end)
read('slot.computer',function()return SlotStatus.SS_COMPUTER end)
read('slot.full_civ',function()return CivilizationLevelTypes.CIVILIZATION_LEVEL_FULL_CIV end)
for id=0,63 do
 local entry={id=id,fields={}};report.slots[#report.slots+1]=entry
 for _,method in ipairs({'GetSlotStatus','GetLeaderTypeName','GetCivilizationTypeName','GetCivilizationLevelTypeID','IsHumanRequired','GetReady','GetModReady'})do
  local key='slot.'..id..'.'..method
  entry.fields[method]=read(key,function()local p=PlayerConfigurations[id];if not p then return nil end;return p[method](p)end)
 end
end
-- Query all rows to preserve map-domain-specific Min/Max/Default separately.
-- A nil result or inaccessible DB is recorded, never replaced by guessed12.
local mapRows
read('map.table_query',function()mapRows=DB.ConfigurationQuery('SELECT * FROM MapSizes');return type(mapRows)=='table' and #mapRows or nil end)
report.map_size_rows={}
if type(mapRows)=='table' then for index,row in ipairs(mapRows)do
 local out={index=index,status='STARTED'};report.map_size_rows[#report.map_size_rows+1]=out
 local ok,err=pcall(function()out.fields={};for k,v in pairs(row)do out.fields[tostring(k)]=scalar(v)end end)
 out.status=ok and 'OK' or 'ERROR';if not ok then out.error=tostring(err)end
 print('MP_SETUP_MAP_ROW '..serial(out))
end end
local installed
read('mods.enumeration',function()installed=Modding.GetInstalledMods();return type(installed)=='table' and #installed or nil end)
local enabled={}
if type(installed)=='table' then for index,row in ipairs(installed)do
 local item={index=index,status='STARTED'};report.mods[#report.mods+1]=item
 local ok,err=pcall(function()
  local info=Modding.GetModInfo(row.Handle);item.id=scalar(info.Id);item.enabled=scalar(info.Enabled)
  if type(info.Id)=='string' then enabled[string.lower(info.Id)]=item.enabled end
 end)
 item.status=ok and 'OK' or 'ERROR';if not ok then item.error=tostring(err)end
 print('MP_SETUP_MOD '..serial(item))
end end
for _,m in ipairs(manifests)do
 local results={};report.criteria[m.id]=results
 for id,conditions in pairs(m.criteria)do
  local result={prediction='match',conditions={}};results[id]=result
  for i,c in ipairs(conditions)do
   local check={kind=c.kind,status='UNKNOWN'};result.conditions[i]=check
   if c.kind=='ruleset' then
    local actual,kind=value('ruleset');check.actual=scalar(actual)
    if kind=='string' then check.status='NO_MATCH';for _,expected in ipairs(c.values)do if actual==expected then check.status='MATCH' end end end
   elseif c.kind=='config' then
    local key='criteria.'..m.id..'.'..id..'.'..i
    check.read=read(key,function()
     if c.group=='Map' then return MapConfiguration.GetValue(c.key)end
     if c.group=='Game' then return GameConfiguration.GetValue(c.key)end
     error('unsupported configuration group: '..tostring(c.group))
    end)
    local actual,kind=value(key);check.expected=c.expected
    -- Non-string XML comparisons require engine coercion semantics, which
    -- this read-only report does not pretend to establish.
    if kind=='string' then check.status=actual==c.expected and 'MATCH' or 'NO_MATCH' end
   end
   if check.status=='NO_MATCH' then result.prediction='no_match'
   elseif check.status=='UNKNOWN' and result.prediction~='no_match' then result.prediction='unknown' end
  end
 end
 for _,a in ipairs(m.actions)do
  local predicate=a.criteria and results[a.criteria] or nil
  local prediction=a.criteria and (predicate and predicate.prediction or 'unknown') or 'unconditional'
  local item={mod=m.id,manifest=m.file,action=a.id,kind=a.kind,phase=a.phase,criteria=a.criteria,
   predicate_prediction=prediction,enabled_observed=enabled[string.lower(m.id)] or {type='unobserved'},execution='not_measured'}
  report.actions[#report.actions+1]=item;print('MP_SETUP_ACTION '..serial(item))
 end
end
local max,kind=value('map.max_major')
report.stock_staging_assessment={scope='formula_only_not_host_acceptance',mapMax=value('map.max_major'),invalid_indices={},unknown_slots={}}
local eval=report.stock_staging_assessment
eval.configured_taken_full_civs=0;eval.configured_computer_full_civs=0
local taken,computer,full=value('slot.taken'),value('slot.computer'),value('slot.full_civ')
if kind=='number' and max>=0 and max%1==0 and taken~=nil and computer~=nil and full~=nil then
 eval.effectiveMax=math.min(max,12)
 for _,slot in ipairs(report.slots)do
  local state=value('slot.'..slot.id..'.GetSlotStatus');local civ=value('slot.'..slot.id..'.GetCivilizationLevelTypeID')
  if state==nil or civ==nil then eval.unknown_slots[#eval.unknown_slots+1]=slot.id
  elseif civ==full then
   if state==taken then eval.configured_taken_full_civs=eval.configured_taken_full_civs+1 end
   if state==computer then eval.configured_computer_full_civs=eval.configured_computer_full_civs+1 end
   if (state==taken or state==computer) and slot.id>=eval.effectiveMax then eval.invalid_indices[#eval.invalid_indices+1]=slot.id end
  end
 end
else eval.reason='missing or invalid mapMax/slot enums; no guessed bound' end
report.read_counts={OK=0,MISSING=0,ERROR=0}
for _,entry in pairs(report.measurements)do report.read_counts[entry.status]=report.read_counts[entry.status]+1 end
report.acceptance='not_evaluated; collected facts only, including missing/error records'
return serial(report)
"#;
pub fn build(root: &Path, normal: &Value, hotseat: &Value) -> Result<Value, String> {
    let setup = crate::mp_hotseat::build_setup_probe(normal, hotseat)?;
    let ids = setup["steps"][0]["ids"]
        .as_array()
        .ok_or("mod list missing")?;
    let manifest = manifest(root, ids)?;
    Ok(
        json!({"name":"Read-only setup facts after explicit12-player Pangaea configuration; no Host/Start/native/turn/payment, missing values retained, manifest predicates are not action execution proof",
 "session_type":"hotseat","judge_profile":"generic","strict_lua_errors":true,
 "steps":[setup["steps"][0],setup["steps"][1],setup["steps"][2],
 {"action":"ui_lua","state_name":"FrontEnd","code":reader(&manifest),"timeout":60,"lua_timeout":60,"capture_as":"SETUP_FACTS_ALL"}]}),
    )
}
