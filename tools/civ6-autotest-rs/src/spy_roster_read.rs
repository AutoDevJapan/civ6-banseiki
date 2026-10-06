//! Read-only evidence from both native contexts before a captive exchange.
//! Rows remain independent; this observer assigns no captive to a Unit ID.

const READ: &str = r#"
local function encode(v,depth)
 assert(depth<32,'raw evidence nesting limit');
 local t=type(v);
 if t=='nil' then return 'null' end
 if t=='boolean' then return v and 'true' or 'false' end
 if t=='number' then
  assert(v==v and math.abs(v)<math.huge,'nonfinite raw number');
  return string.format('%.17g',v)
 end
 if t=='string' then
  local escaped=v:gsub('[%z\1-\31\\"]',function(c)
   if c=='\\' then return '\\\\' end
   if c=='"' then return '\\"' end
   return string.format('\\u%04x',string.byte(c))
  end);
  return '"'..escaped..'"'
 end
 assert(t=='table','raw evidence must contain scalar tables');
 local function keyText(k)
  return type(k)=='number' and string.format('%.17g',k) or k
 end
 local keys,seen,collision,numericKeys={},{},false,false; for k in pairs(v) do
  assert(type(k)=='string' or type(k)=='number','unsupported raw key');
  if type(k)=='number' then numericKeys=true end
  local text=keyText(k); if seen[text] then collision=true end; seen[text]=true;
  keys[#keys+1]=k
 end
 table.sort(keys,function(a,b)
  local x,y=keyText(a),keyText(b);
  return x<y or (x==y and type(a)<type(b))
 end);
 if collision or numericKeys then
  local entries={}; for _,k in ipairs(keys) do
   entries[#entries+1]='{"key":{"kind":'..encode(type(k),depth+1)..
    ',"value":'..encode(k,depth+1)..'},"value":'..encode(v[k],depth+1)..'}'
  end
  return '{"kind":"lua_table_entries","entries":['..table.concat(entries,',')..']}'
 end
 local parts={}; for _,k in ipairs(keys) do
  parts[#parts+1]=encode(keyText(k),depth+1)..':'..encode(v[k],depth+1)
 end
 return '{'..table.concat(parts,',')..'}'
end
local function scalar(v)
 local t=type(v); local r={kind=t};
 if t=='nil' then return r end
 if t=='string' or t=='boolean' or
  (t=='number' and v==v and math.abs(v)<math.huge) then r.value=v
 else r.status='UNSUPPORTED' end
 return r
end
local function call(object,name,...)
 local ok,method=pcall(function() return object and object[name] end);
 if not ok then return {status='LOOKUP_ERROR',error=tostring(method)} end
 if type(method)~='function' then
  return {status='UNAVAILABLE',getterKind=type(method)}
 end
 local ok,value=pcall(method,object,...);
 if not ok then return {status='ERROR',error=tostring(value)} end
 local r=scalar(value);
 if r.status=='UNSUPPORTED' then r.valueSupported=false end
 r.status='OK'; return r,value
end
local function fields(value,names)
 local result={}; for _,name in ipairs(names) do
  local ok,v=pcall(function() return value and value[name] end);
  result[name]=ok and scalar(v) or {status='LOOKUP_ERROR',error=tostring(v)}
 end; return result
end
local evidence={schema=1,context=CONTEXT,turn=scalar(Game.GetCurrentGameTurn()),
 owner=0,units={},diplomacy={},properties={}};
evidence.spyDefinition=fields(GameInfo.Units['UNIT_SPY'],{'Index','UnitType','Spy'});
local player=Players[0];
local unitsStatus,units=call(player,'GetUnits'); evidence.unitsAPI=unitsStatus;
local ok,err=pcall(function()
 assert(units and type(units.Members)=='function','native Members unavailable');
 local count=0;
 for _,u in units:Members() do
  count=count+1; assert(count<=4096,'native unit evidence limit');
  local row={}; evidence.units[tostring(count)]=row;
  for _,name in ipairs({'GetID','GetOwner','GetType','GetName','GetX','GetY',
   'GetPlotId','GetSpyOperation','GetSpyOperationEndTurn'}) do row[name]=call(u,name) end
  local unitType=row.GetType.value;
  local defOK,def=pcall(function() return GameInfo.Units[unitType] end);
  row.definition=defOK and fields(def,{'Index','UnitType','Spy'}) or
   {status='ERROR',error=tostring(def)};
  local experienceStatus,experience=call(u,'GetExperience');
  row.experienceAPI=experienceStatus; row.level=call(experience,'GetLevel');
 end
 evidence.unitCount=count
end);
evidence.enumeration={status=ok and 'OK' or 'ERROR',error=not ok and tostring(err) or nil};
local diplomacyStatus,d=call(player,'GetDiplomacy'); evidence.diplomacyAPI=diplomacyStatus;
evidence.diplomacy.capacity=call(d,'GetSpyCapacity');
evidence.diplomacy.offMapCount=call(d,'GetNumSpiesOffMap');
evidence.diplomacy.offMap={};
local n=evidence.diplomacy.offMapCount.value;
if type(n)=='number' and n>=0 and n<=256 and n==math.floor(n) then
 for i=0,n-1 do
  local status,row=call(d,'GetNthOffMapSpy',0,i);
  evidence.diplomacy.offMap[tostring(i)]={read=status,
   fields=fields(row,{'Name','Level','ReturnTurn','XLocation','YLocation'})}
 end
end
evidence.diplomacy.histories={};
for i=0,2 do
 local status,row=call(d,'GetMission',0,i);
 evidence.diplomacy.histories[tostring(i)]={read=status,
  fields=fields(row,{'Operation','PlotIndex','CompletionTurn','InitialResult','EscapeResult'})}
end
local _,captor=call(Players[1],'GetDiplomacy');
evidence.diplomacy.capturedCount=call(captor,'GetNumSpiesCaptured');
evidence.diplomacy.captured={}; n=evidence.diplomacy.capturedCount.value;
if type(n)=='number' and n>=0 and n<=256 and n==math.floor(n) then
 for i=0,n-1 do
  local status,row=call(captor,'GetNthCapturedSpy',1,i);
  evidence.diplomacy.captured[tostring(i)]={read=status,
   fields=fields(row,{'OwningPlayer','Name','Level','NameIndex'})}
 end
end
for _,name in ipairs({'JAC_BOUNDARY_GROUP_EXCHANGE_62','JAC_BOUNDARY_THIRD_SPY_ARCHIVE',
 'JAC_BOUNDARY_THIRD_SPY_STATE','JAC_BOUNDARY_THIRD_MISSION_ACK','JAC_BOUNDARY_ENEMY_MISSION'}) do
 local status,value=call(Game,'GetProperty',name);
 evidence.properties[name]={read=status,value=value};
end
"#;

/// GameCore getters may be unavailable. Preserve their types and errors,
/// every enumerated Unit, and the original saved histories before judging.
pub fn core_read() -> String {
    format!("local CONTEXT='GameCore_Tuner';\n{READ}\nreturn encode(evidence,0)")
}

/// Capture the public UI reader without installing exchange listeners,
/// creating working deal items, or publishing any Gameplay property.
pub fn ui_read() -> String {
    format!(
        "local CONTEXT='InGame';\n{READ}\n{}",
        r#"
local B=ExposedMembers.JAC_PersonaBoundary;
local ok,s=pcall(function() return B.ReadEnemy() end);
evidence.boundary={read={status=ok and 'OK' or 'ERROR',kind=type(s),
 error=not ok and tostring(s) or nil}};
if ok and type(s)=='table' then
 evidence.boundary.turn=scalar(s.turn);
 evidence.boundary.attacker=scalar(s.attacker); evidence.boundary.owner=scalar(s.owner);
 evidence.boundary.roster=s.spyRoster; evidence.boundary.missions=s.missions;
 evidence.boundary.spy=s.spy; evidence.boundary.native=s.native;
end
return encode(evidence,0)
"#
    )
}

/// Independently count the literal Spy flags, including numeric zero, which
/// Lua treats as true. This reports observations, not authority to exchange.
pub fn inspect_pair(
    ui: &serde_json::Value,
    core: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    for (read, context) in [(ui, "InGame"), (core, "GameCore_Tuner")] {
        if read["schema"] != 1 || read["context"] != context || read["turn"]["value"] != 62 {
            return Err(format!("{context}: wrong schema, context or turn"));
        }
        let rows = read["units"].as_object().ok_or("Unit rows missing")?;
        let count = read["unitCount"]
            .as_u64()
            .ok_or("Unit count is not an integer")?;
        if read["enumeration"]["status"] != "OK" || count != rows.len() as u64 {
            return Err(format!("{context}: native Unit enumeration incomplete"));
        }
    }
    let mut contexts = serde_json::Map::new();
    for (read, context) in [(ui, "InGame"), (core, "GameCore_Tuner")] {
        let spy_type = read["spyDefinition"]["Index"]["value"].as_i64();
        let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        let mut legacy_truthy = 0;
        let mut known_spy_rows = Vec::new();
        let mut unknown_flags = Vec::new();
        for (ordinal, row) in read["units"].as_object().unwrap() {
            let flag = &row["definition"]["Spy"];
            groups
                .entry(flag.to_string())
                .or_default()
                .push(json!({"ordinal":ordinal,
              "id":row["GetID"],"unitType":row["GetType"]}));
            match flag["kind"].as_str() {
                Some("nil") => {}
                Some("boolean") if flag["value"] == false => {}
                Some("boolean" | "number" | "string") if flag.get("value").is_some() => {
                    legacy_truthy += 1;
                }
                _ => unknown_flags.push(ordinal),
            }
            if let Some(spy_type) = spy_type {
                if row["GetType"]["value"].as_i64() == Some(spy_type) {
                    known_spy_rows.push(json!({"ordinal":ordinal,"raw":row}));
                }
            }
        }
        contexts.insert(
            context.into(),
            json!({"unitCount":read["unitCount"],
          "spyTypeIndex":spy_type,"literalFlagGroups":groups,
          "legacyLuaTruthyCount":legacy_truthy,"unknownFlagRows":unknown_flags,
          "knownSpyTypeRows":known_spy_rows}),
        );
    }
    let partial = ui["boundary"]["read"]["status"] != "OK";
    Ok(json!({"evidenceOnly":true,"exchangeAuthorized":false,
      "partial":partial,"captureState":if partial {"PARTIAL_UI_READ_FAILURE"} else {"RAW_BOTH_CONTEXTS"},
      "contexts":contexts,"uiRaw":ui,"coreRaw":core,
      "nextPrerequisite":"Compare actual Core Unit type, flag and coordinates with the separate UI roster; do not infer individual captive identity or send an exchange from this report."}))
}
