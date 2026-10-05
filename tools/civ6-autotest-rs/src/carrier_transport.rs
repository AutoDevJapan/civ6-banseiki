//! Native PlayerOperation transport. Rust defines the scalar wire contract;
//! generated Lua only binds it to the installed engine and persists receipts.
//! A boundary receipt never proves or advances a physical Great Work MOVE.
use crate::{carrier_sync::Checkpoint, scenario::lua_string, Error, Result};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub const EVENT: &str = "JAC_CarrierBoundary_Operation";
pub const STATE_KEY: &str = "JAC_CARRIER_BOUNDARY_V1";
// packet name -> saved carrier field. The same projection is compiled into
// the receiving binding, so adding a field cannot silently omit its check.
const CARRIER_FIELDS: &[(&str, &str)] = &[
    ("Token", "Token"),
    ("Phase", "Phase"),
    ("Index", "Index"),
    ("Owner", "Owner"),
    ("Borrower", "Borrower"),
    ("X", "X"),
    ("Y", "Y"),
    ("Building", "Building"),
    ("Slot", "Slot"),
    ("OriginalCity", "OriginalCity"),
];
const PHASE_TOKENS: &[(&str, &str)] = &[
    ("BORROW", "BorrowToken"),
    ("MOVE", "MoveToken"),
    ("RETURN", "ReturnToken"),
    ("RESTORE", "ReturnToken"),
    ("PROVED", "ReturnToken"),
];
const EXTRA_FIELDS: &[&str] = &[
    "Version",
    "Command",
    "Actor",
    "Turn",
    "CityOwner",
    "CityID",
    "ObservationToken",
];

fn failure(message: &str) -> Error {
    Error::scenario(format!("carrier transport: {message}"))
}

fn fields() -> Vec<&'static str> {
    let mut names: Vec<_> = CARRIER_FIELDS
        .iter()
        .map(|(name, _)| *name)
        .chain(EXTRA_FIELDS.iter().copied())
        .collect();
    names.sort_unstable();
    names
}

/// Actor choice is shared by every participant: the lowest currently active
/// human ID. Local UI identity is checked only before sending, never in Core.
pub fn packet(checkpoint: &Checkpoint, eligible: &[i64]) -> Result<Value> {
    let token = checkpoint.carrier["Token"]
        .as_str()
        .ok_or_else(|| failure("missing token"))?;
    checkpoint.validate(token, &checkpoint.candidate)?;
    if !checkpoint.multiplayer {
        return Err(failure("multiplayer boundary required"));
    }
    let humans: BTreeSet<_> = eligible.iter().copied().collect();
    if humans.len() != eligible.len() || humans.is_empty() || humans.iter().any(|id| *id < 0) {
        return Err(failure(
            "active human IDs must be nonempty, distinct and nonnegative",
        ));
    }
    let actor = *humans.first().unwrap();
    if actor != checkpoint.local_player {
        return Err(failure("this UI is not the selected sender"));
    }
    let phase = checkpoint.carrier["Phase"].as_str().unwrap();
    let observation_field = PHASE_TOKENS
        .iter()
        .find(|(p, _)| *p == phase)
        .map(|(_, field)| *field)
        .ok_or_else(|| failure("phase has no pending publication token"))?;
    let observation = checkpoint.carrier[observation_field]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| failure("saved observation token is missing"))?;
    crate::carrier_sync::validate_token(observation)?;
    let mut data = Map::new();
    for (name, source) in CARRIER_FIELDS {
        data.insert((*name).into(), checkpoint.carrier[*source].clone());
    }
    for (name, value) in [
        ("Version", json!(1)),
        ("Command", json!("BOUNDARY")),
        ("Actor", json!(actor)),
        ("Turn", json!(checkpoint.turn)),
        ("CityOwner", json!(checkpoint.city_owner)),
        ("CityID", json!(checkpoint.city_id)),
        ("ObservationToken", json!(observation)),
    ] {
        data.insert(name.into(), value);
    }
    let key = key(&Value::Object(data.clone()))?;
    data.insert("RequestID".into(), json!(key));
    Ok(Value::Object(data))
}

/// Length framing, not delimiters inside a token, defines receipt identity.
pub fn key(packet: &Value) -> Result<String> {
    let mut result = String::new();
    for name in fields() {
        let value = &packet[name];
        let scalar = if let Some(s) = value.as_str() {
            s.to_owned()
        } else if let Some(n) = value.as_i64() {
            n.to_string()
        } else {
            return Err(failure("wire fields must be strings or integers"));
        };
        result.push_str(&format!("{}:{scalar}", scalar.len()));
    }
    Ok(result)
}

pub fn send_query(packet: &Value) -> Result<String> {
    if packet["RequestID"].as_str() != Some(&key(packet)?) {
        return Err(failure("request identity differs"));
    }
    let mut entries = vec![format!("OnStart={}", lua_string(EVENT))];
    for name in fields().into_iter().chain(["RequestID"]) {
        let v = &packet[name];
        let literal = if let Some(s) = v.as_str() {
            lua_string(s)
        } else {
            v.as_i64()
                .ok_or_else(|| failure("non-scalar wire value"))?
                .to_string()
        };
        entries.push(format!("[{name:?}]={literal}"));
    }
    Ok(format!("local actor={}\nassert(Game.GetLocalPlayer()==actor,'carrier sender changed')\nassert(GameConfiguration.IsAnyMultiplayer(),'multiplayer required')\nreturn UI.RequestPlayerOperation(actor,PlayerOperations.EXECUTE_SCRIPT,{{{}}})",packet["Actor"],entries.join(",")))
}

/// This read is independent of the saved boundary snapshot and cannot mutate.
pub const ACTORS_QUERY: &str = "local ids={}\nfor _,id in ipairs(PlayerManager.GetAliveIDs()) do local p=Players[id];if p:IsHuman() then assert(p.IsTurnActive,'active-turn getter missing');if p:IsTurnActive() then ids[#ids+1]=id end end end\ntable.sort(ids)\nreturn table.concat(ids,',')";

pub fn parse_actors(value: &str) -> Result<Vec<i64>> {
    if value.is_empty() {
        return Err(failure("no active human actor"));
    }
    value
        .split(',')
        .map(|part| {
            part.parse::<i64>()
                .map_err(|_| failure("invalid active actor ID"))
        })
        .collect()
}

pub fn receipt_query(packet: &Value) -> Result<String> {
    let request_id = packet["RequestID"]
        .as_str()
        .ok_or_else(|| failure("receipt identity missing"))?;
    if request_id != key(packet)? {
        return Err(failure("receipt identity changed"));
    }
    let checks = fields()
        .iter()
        .map(|name| {
            format!(
                "if r.Packet[{}]~={} then return false end",
                lua_string(name),
                packet[*name]
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!("local api=assert(ExposedMembers.JAC_CarrierBoundary,'boundary receiver missing')\nlocal r=api.GetReceipt({})\nif not (r and r.Accepted==true and r.Count==1 and r.RequestID=={} and type(r.Packet)=='table') then return false end\n{checks}\nlocal n=0;for _ in pairs(r.Packet) do n=n+1 end;return n=={}",lua_string(request_id),lua_string(request_id),fields().len()))
}

pub fn gameplay_binding() -> String {
    let projections = CARRIER_FIELDS
        .iter()
        .map(|(a, b)| format!("[{}]={}", lua_string(a), lua_string(b)))
        .collect::<Vec<_>>()
        .join(",");
    let phases = PHASE_TOKENS
        .iter()
        .map(|(a, b)| format!("[{}]={}", lua_string(a), lua_string(b)))
        .collect::<Vec<_>>()
        .join(",");
    let names = fields()
        .iter()
        .map(|s| lua_string(s))
        .collect::<Vec<_>>()
        .join(",");
    format!("-- Generated by Rust civ6-carrier-transport; change the Rust contract.\nlocal EVENT, KEY = {}, {}\nlocal fields = {{{names}}}\nlocal projection = {{{projections}}}\nlocal phaseTokens = {{{phases}}}\n{}",lua_string(EVENT),lua_string(STATE_KEY),BINDING)
}

const BINDING: &str = r#"
include('JAC_Absorption_Rules')
local R=assert(JAC_AbsorptionRules,'absorption rules required')
local function Receive(actor,p)
  assert(GameConfiguration.IsAnyMultiplayer(),'multiplayer boundary required')
  assert(type(p)=='table' and p.Version==1 and p.Command=='BOUNDARY','invalid wire contract')
  local selected
  for _,id in ipairs(PlayerManager.GetAliveIDs()) do
    local player=Players[id]
    if player:IsHuman() then
      assert(player.IsTurnActive,'active-turn getter missing')
      if player:IsTurnActive() and (selected==nil or id<selected) then selected=id end
    end
  end
  assert(actor==selected and p.Actor==actor,'unselected sender')
  assert(p.Turn==Game.GetCurrentGameTurn(),'stale boundary turn')
  local state=assert(Game:GetProperty('JAC_ABSORPTION_STATE_V1'),'saved absorption ledger missing')
  local found={}
  for _,tx in pairs(state.Transactions or {}) do
    for _,snap in ipairs(tx.NativeWorks or {}) do
      if snap.Carrier and snap.Carrier.Token==p.Token then found[#found+1]=snap.Carrier end
    end
  end
  for _,intent in pairs(state.Recoveries or {}) do
    if intent.NativeCarrier and intent.NativeCarrier.Token==p.Token then found[#found+1]=intent.NativeCarrier end
  end
  assert(#found==1,'carrier token missing or duplicated')
  local c=found[1]
  for name,source in pairs(projection) do assert(p[name]==c[source],'saved carrier field changed: '..name) end
  local observation=phaseTokens[c.Phase]
  assert(observation and p.ObservationToken==c[observation] and type(p.ObservationToken)=='string'
    and p.ObservationToken~='','saved observation token changed')
  local city=assert(CityManager.GetCityAt(c.X,c.Y),'carrier city disappeared')
  assert(p.CityOwner==city:GetOwner() and p.CityID==city:GetID(),'carrier city identity changed')
  local owner,id=city:GetOwner(),city:GetID()
  assert(owner==c.Owner or owner==c.Borrower,'carrier belongs to unrelated owner')
  if c.Phase=='BORROW' then
    if owner==c.Owner then
      assert(id==c.OriginalCity,'original carrier identity changed')
    else
      assert(c.BorrowIssued==true,'borrowed carrier has no issued saved intent')
      assert(c.BorrowedCity==nil or c.BorrowedCity==id,'saved borrowed identity changed')
    end
  elseif c.Phase=='MOVE' then
    assert(owner==c.Borrower and id==c.BorrowedCity,'MOVE borrowed identity changed')
  elseif c.Phase=='RETURN' or c.Phase=='RESTORE' then
    if owner==c.Borrower then assert(id==c.BorrowedCity,'return borrowed identity changed') end
  elseif c.Phase=='PROVED' then
    assert(owner==c.Owner,'proved carrier was not returned')
    local final=assert(c.FinalSlot,'final carrier identity missing')
    assert(final.Owner==owner and final.City==id and final.X==c.X and final.Y==c.Y
      and final.Building==c.Building and final.Slot==c.Slot,'saved final carrier identity changed')
  end
  local parts,packet={},{}
  for _,name in ipairs(fields) do
    local v=p[name]
    assert(type(v)=='string' or type(v)=='number' and v==math.floor(v),'non-scalar wire field')
    local value=tostring(v);parts[#parts+1]=tostring(#value)..':'..value;packet[name]=v
  end
  local request=table.concat(parts)
  assert(p.RequestID==request,'receipt identity changed')
  local box=R.Copy(Game:GetProperty(KEY) or {Version=1,Receipts={}})
  assert(box.Version==1 and type(box.Receipts)=='table','unsupported receipt save')
  if box.Receipts[request] then
    assert(box.Receipts[request].Accepted==true and box.Receipts[request].Count==1
      and box.Receipts[request].RequestID==request,'saved receipt is corrupt')
    assert(R.Equal(box.Receipts[request].Packet,packet),'receipt payload changed')
    return true
  end
  box.Receipts[request]={Accepted=true,Count=1,RequestID=request,Packet=packet}
  Game:SetProperty(KEY,box)
  return true
end
GameEvents[EVENT].Add(function(actor,p)
  local ok,result=pcall(Receive,actor,p)
  if not ok then print('[JAC_CARRIER_BOUNDARY_REJECTED] '..tostring(result)) end
  return ok and result==true
end)
ExposedMembers.JAC_CarrierBoundary={GetReceipt=function(id)
  local box=Game:GetProperty(KEY)
  return box and box.Version==1 and R.Copy(box.Receipts[id]) or nil
end}
print('[JAC_CARRIER_BOUNDARY] loaded version=1')
"#;
