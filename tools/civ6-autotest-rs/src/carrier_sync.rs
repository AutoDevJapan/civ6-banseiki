//! Read-only evidence for native carrier save/reload and peer comparisons.
//! This module does not enable the product's multiplayer carrier route.
use crate::{scenario::lua_string, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub schema_version: u32,
    pub candidate: String,
    pub turn: i64,
    pub multiplayer: bool,
    pub local_player: i64,
    pub carrier: Value,
    pub ledger: Value,
    pub flow: Value,
    pub flow_required: bool,
    pub city_owner: i64,
    pub city_id: i64,
    pub city: Value,
    pub locations: Vec<Value>,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::scenario(format!("native carrier checkpoint: {}", message.into()))
}

pub fn validate_token(token: &str) -> Result<()> {
    if token.is_empty() || token.len() > 256 || token.chars().any(char::is_control) {
        return Err(invalid(
            "token must contain 1..256 bytes without control characters",
        ));
    }
    Ok(())
}

impl Checkpoint {
    pub fn parse(text: &str, token: &str, candidate: &str) -> Result<Self> {
        let checkpoint: Self = serde_json::from_str(text)?;
        checkpoint.validate(token, candidate)?;
        Ok(checkpoint)
    }

    pub fn validate(&self, token: &str, candidate: &str) -> Result<()> {
        validate_token(token)?;
        if self.schema_version != 1
            || self.candidate != candidate
            || candidate.is_empty()
            || self.turn < 0
            || self.local_player < 0
            || self.city_owner < 0
            || self.city_id < 0
            || self.carrier["Token"] != token
            || !self.carrier.is_object()
        {
            return Err(invalid(
                "schema, candidate, turn, player or carrier identity differs",
            ));
        }
        if !["BORROW", "MOVE", "RETURN", "RESTORE", "RESTORED", "PROVED"]
            .contains(&self.carrier["Phase"].as_str().unwrap_or(""))
            || !["TO_OWNER", "FROM_FREE", "TO_FREE"]
                .contains(&self.carrier["Route"].as_str().unwrap_or(""))
        {
            return Err(invalid("unknown saved phase or route"));
        }
        let index = self.carrier["Index"]
            .as_i64()
            .filter(|n| *n >= 0)
            .ok_or_else(|| invalid("missing original work index"))?;
        if self.locations.len() != 1 {
            return Err(invalid(
                "original work is absent or duplicated in the world",
            ));
        }
        let work = &self.locations[0];
        if work["Index"].as_i64() != Some(index) {
            return Err(invalid("observed work index differs"));
        }
        let source = &self.carrier["Source"];
        if !self.ledger.is_object() || !(self.flow.is_null() || self.flow.is_object()) {
            return Err(invalid("saved ledger or flow snapshot is incomplete"));
        }
        if self.flow_required
            && (!self.flow["Flows"].is_object() || !self.flow["World"].is_object())
        {
            return Err(invalid("required flow evidence is missing"));
        }
        let mut saved = Vec::new();
        let transactions = self.ledger["Transactions"]
            .as_object()
            .ok_or_else(|| invalid("transaction ledger is missing"))?;
        let recoveries = self.ledger["Recoveries"]
            .as_object()
            .ok_or_else(|| invalid("recovery ledger is missing"))?;
        for (id, tx) in transactions {
            if let Some(works) = tx["NativeWorks"].as_object() {
                for snap in works.values() {
                    if snap["Carrier"]["Token"] == token {
                        if (self.flow_required || tx["Request"]["FlowPolicy"] == true)
                            && !self.flow["Flows"][id].is_object()
                        {
                            return Err(invalid("selected transaction's flow evidence is missing"));
                        }
                        saved.push((&snap["Carrier"], &snap["Source"]));
                    }
                }
            }
        }
        for (id, recovery) in recoveries {
            if recovery["NativeCarrier"]["Token"] == token {
                if (self.flow_required || id.starts_with("recovery:"))
                    && !id
                        .strip_prefix("recovery:")
                        .is_some_and(|key| self.flow["World"][key].is_object())
                {
                    return Err(invalid(
                        "selected recovery's world flow evidence is missing",
                    ));
                }
                saved.push((&recovery["NativeCarrier"], &recovery["NativeSource"]));
            }
        }
        if saved.len() != 1 || saved[0].0 != &self.carrier || saved[0].1 != source {
            return Err(invalid(
                "selected carrier or original source does not match one saved ledger intent",
            ));
        }
        if !self.city["Name"].is_string()
            || !self.city["Population"].as_i64().is_some_and(|n| n > 0)
            || !self.city["Loyalty"]
                .as_f64()
                .is_some_and(|n| (0.0..=100.0).contains(&n))
            || [
                "Assets",
                "Religion",
                "Districts",
                "UnitsOnCenter",
                "PurchasedPlots",
            ]
            .iter()
            .any(|key| !self.city[*key].is_object())
        {
            return Err(invalid("carrier city snapshot is incomplete"));
        }
        if self.city["Districts"] != self.city["Assets"]["Districts"] {
            return Err(invalid("independent district records disagree"));
        }
        let before = &self.carrier["Before"];
        if !before["Name"].is_string()
            || !before["Population"].as_i64().is_some_and(|n| n > 0)
            || !before["Loyalty"]
                .as_f64()
                .is_some_and(|n| (0.0..=100.0).contains(&n))
            || !before["Assets"]["Districts"].is_object()
            || ["Assets", "Religion", "UnitsOnCenter", "PurchasedPlots"]
                .iter()
                .any(|key| !before[*key].is_object())
        {
            return Err(invalid("pre-borrow city baseline is incomplete"));
        }
        for key in [
            "X",
            "Y",
            "Building",
            "Slot",
            "Owner",
            "Borrower",
            "OriginalCity",
        ] {
            if !self.carrier[key].as_i64().is_some_and(|n| n >= 0) {
                return Err(invalid(format!("saved carrier lacks {key}")));
            }
        }
        for key in [
            "Index",
            "GreatWorkType",
            "ObjectType",
            "CreatorName",
            "TurnCreated",
            "CreatedByPlayer",
            "Effects",
        ] {
            let expected = source
                .get(key)
                .filter(|v| !v.is_null())
                .ok_or_else(|| invalid(format!("original work lacks {key} evidence")))?;
            if work.get(key) != Some(expected) {
                return Err(invalid(format!("original work changed: {key}")));
            }
        }
        for key in ["Owner", "CityID", "X", "Y", "Building", "Slot"] {
            if !work[key].as_i64().is_some_and(|n| n >= 0) {
                return Err(invalid(format!("observed work lacks {key}")));
            }
        }
        let in_slot = |slot: &Value, id: &str| {
            [
                ("Owner", "Owner"),
                ("CityID", id),
                ("X", "X"),
                ("Y", "Y"),
                ("Building", "Building"),
                ("Slot", "Slot"),
            ]
            .iter()
            .all(|(actual, expected)| work[*actual] == slot[*expected])
        };
        let carrier_slot = serde_json::json!({"Owner":self.city_owner,"City":self.city_id,
            "X":self.carrier["X"],"Y":self.carrier["Y"],
            "Building":self.carrier["Building"],"Slot":self.carrier["Slot"]});
        let from_free = self.carrier["Route"] == "FROM_FREE";
        let phase = self.carrier["Phase"].as_str().unwrap();
        let owner = self.carrier["Owner"].as_i64().unwrap();
        let borrower = self.carrier["Borrower"].as_i64().unwrap();
        if self.city_owner != owner && self.city_owner != borrower {
            return Err(invalid("carrier city belongs to an unrelated player"));
        }
        if phase == "MOVE"
            && (self.city_owner != borrower
                || self.carrier["BorrowedCity"].as_i64() != Some(self.city_id))
        {
            return Err(invalid(
                "MOVE carrier owner or borrowed city identity changed",
            ));
        }
        if ["RETURN", "RESTORE"].contains(&phase)
            && self.city_owner == borrower
            && self.carrier["BorrowedCity"].as_i64() != Some(self.city_id)
        {
            return Err(invalid("return intent changed borrowed city identity"));
        }
        if phase == "BORROW"
            && self.city_owner == owner
            && self.carrier["OriginalCity"].as_i64() != Some(self.city_id)
        {
            return Err(invalid("unborrowed city identity changed"));
        }
        if ["RESTORED", "PROVED"].contains(&phase) && self.city_owner != owner {
            return Err(invalid("terminal carrier was not returned"));
        }
        let valid_location = match phase {
            "BORROW" | "RESTORED" if from_free => in_slot(&carrier_slot, "City"),
            "BORROW" | "RESTORED" => in_slot(source, "CityID"),
            "MOVE" => {
                in_slot(&self.carrier["MoveSource"], "City")
                    || in_slot(&self.carrier["MoveTarget"], "City")
            }
            "RETURN" if !from_free => in_slot(&carrier_slot, "City"),
            "RETURN" => in_slot(&self.carrier["MoveTarget"], "City"),
            "RESTORE" if from_free => in_slot(&carrier_slot, "City"),
            "RESTORE" => in_slot(&self.carrier["MoveSource"], "City"),
            "PROVED" => true,
            _ => false,
        };
        if !valid_location {
            return Err(invalid("original is outside the saved phase's route"));
        }
        if self.carrier["Phase"] == "PROVED" {
            if self.carrier["Owner"].as_i64() != Some(self.city_owner) {
                return Err(invalid("proved carrier city was not returned"));
            }
            if self.carrier["FinalSlot"] != carrier_slot {
                return Err(invalid(
                    "returned carrier slot differs from the saved final slot",
                ));
            }
            let slot = if self.carrier["Route"] == "FROM_FREE" {
                &self.carrier["Target"]
            } else {
                &self.carrier["FinalSlot"]
            };
            for (actual, expected) in [
                ("Owner", "Owner"),
                ("CityID", "City"),
                ("X", "X"),
                ("Y", "Y"),
                ("Building", "Building"),
                ("Slot", "Slot"),
            ] {
                if work[actual] != slot[expected] {
                    return Err(invalid(format!(
                        "proved work missed reserved slot: {actual}"
                    )));
                }
            }
            if !from_free && !in_slot(&carrier_slot, "City") {
                return Err(invalid(
                    "proved forward work is not in the returned carrier",
                ));
            }
            let mut restored = self.city.clone();
            let building = self.city["CarrierBuildingType"]
                .as_str()
                .ok_or_else(|| invalid("carrier building type missing"))?;
            let works = restored["Assets"][building]["Works"]
                .as_object_mut()
                .ok_or_else(|| invalid("carrier building snapshot missing"))?;
            works.insert(
                self.carrier["Slot"].as_i64().unwrap().to_string(),
                serde_json::json!(-1),
            );
            let before = &self.carrier["Before"];
            for key in [
                "Name",
                "Population",
                "Assets",
                "Religion",
                "Loyalty",
                "UnitsOnCenter",
                "PurchasedPlots",
                "Faith",
                "Governance",
            ] {
                if restored.get(key) != before.get(key) {
                    return Err(invalid(format!(
                        "returned city changed from pre-borrow baseline: {key}"
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Compare one saved boundary, not different phases or different turns.
/// Peer mode additionally requires two distinct local players in multiplayer.
pub fn compare(left: &Checkpoint, right: &Checkpoint, peers: bool) -> Result<()> {
    let token = left.carrier["Token"]
        .as_str()
        .ok_or_else(|| invalid("missing token"))?;
    left.validate(token, &left.candidate)?;
    right.validate(token, &left.candidate)?;
    if peers && (!left.multiplayer || !right.multiplayer || left.local_player == right.local_player)
    {
        return Err(invalid(
            "peer comparison needs distinct multiplayer players",
        ));
    }
    if !peers && left.local_player != right.local_player {
        return Err(invalid("reload changed local player"));
    }
    if left.turn != right.turn
        || left.multiplayer != right.multiplayer
        || left.carrier != right.carrier
        || left.ledger != right.ledger
        || left.flow != right.flow
        || left.flow_required != right.flow_required
        || left.city_owner != right.city_owner
        || left.city_id != right.city_id
        || left.city != right.city
        || left.locations != right.locations
    {
        return Err(invalid(
            "saved state or observed original differs at the boundary",
        ));
    }
    Ok(())
}

/// Generate a query for the existing InGame FireTuner context. No mutation,
/// publication event, MOVE request or completion notification is sent.
pub fn query(token: &str, candidate: &str) -> Result<String> {
    query_with_scope(token, candidate, true)
}

pub fn query_with_scope(token: &str, candidate: &str, flow_required: bool) -> Result<String> {
    validate_token(token)?;
    if candidate.is_empty() || candidate.len() > 128 || candidate.chars().any(char::is_control) {
        return Err(invalid(
            "candidate must contain 1..128 bytes without control characters",
        ));
    }
    Ok(format!(
        "local wanted, candidate, flow_required = {}, {}, {}\n{}",
        lua_string(token),
        lua_string(candidate),
        flow_required,
        QUERY
    ))
}

const QUERY: &str = r#"
local api = assert(ExposedMembers.JAC_Absorption, 'absorption API missing')
local state = assert(api.GetState(), 'absorption state missing')
local matches = {}
for _, tx in pairs(state.Transactions or {}) do
  for _, snap in ipairs(tx.NativeWorks or {}) do
    if snap.Carrier and snap.Carrier.Token == wanted then matches[#matches+1] = snap.Carrier end
  end
end
for _, intent in pairs(state.Recoveries or {}) do
  if intent.NativeCarrier and intent.NativeCarrier.Token == wanted then matches[#matches+1] = intent.NativeCarrier end
end
assert(#matches == 1, 'carrier token missing or duplicated')
local carrier = matches[1]
local city = assert(CityManager.GetCityAt(carrier.X, carrier.Y), 'carrier city missing')
local function quote(s)
  return '"' .. s:gsub('[%z\1-\31\\"]', function(c)
    return string.format('\\u%04x', string.byte(c))
  end) .. '"'
end
local active = {}
local function encode(v)
  local kind = type(v)
  if kind == 'nil' then return 'null' end
  if kind == 'boolean' then return v and 'true' or 'false' end
  if kind == 'string' then return quote(v) end
  if kind == 'number' then
    assert(v == v and v ~= math.huge and v ~= -math.huge, 'nonfinite carrier field')
    return string.format('%.17g', v)
  end
  assert(kind == 'table' and not active[v], 'unsupported or cyclic carrier field')
  active[v] = true
  local keys, seen = {}, {}
  for k in pairs(v) do
    assert(type(k) == 'string' or type(k) == 'number', 'unsupported carrier key')
    local key = tostring(k)
    assert(not seen[key], 'ambiguous carrier key')
    seen[key], keys[#keys+1] = true, { key = key, raw = k }
  end
  table.sort(keys, function(a,b) return a.key < b.key end)
  local out = {}
  for _, k in ipairs(keys) do out[#out+1] = quote(k.key) .. ':' .. encode(v[k.raw]) end
  active[v] = nil
  return '{' .. table.concat(out, ',') .. '}'
end
-- Independently enumerate UI engine slots. Gameplay's public facade only
-- exports saved state; its inventory implementation is not an exposed API.
local owners, seen = {}, {}
for _, owner in ipairs(PlayerManager.GetAliveIDs()) do
  if not seen[owner] then owners[#owners+1],seen[owner]=owner,true end
end
local free=PlayerManager.GetFreeCitiesPlayerID and PlayerManager.GetFreeCitiesPlayerID()
if type(free)=='number' and free>=0 and Players[free] and not seen[free] then owners[#owners+1]=free end
if Players[carrier.Owner] and not seen[carrier.Owner] and carrier.Owner~=free then owners[#owners+1]=carrier.Owner end
table.sort(owners)
local works, units = {}, {}
for _, owner in ipairs(owners) do
  for _, unit in Players[owner]:GetUnits():Members() do
    if unit:GetX()==carrier.X and unit:GetY()==carrier.Y then units[#units+1]={Owner=owner,ID=unit:GetID(),Type=GameInfo.Units[unit:GetType()].UnitType} end
  end
  for _, current in Players[owner]:GetCities():Members() do
    local buildings=current:GetBuildings()
    for info in GameInfo.Buildings() do
      if buildings:HasBuilding(info.Index) then
        for slot=0,buildings:GetNumGreatWorkSlots(info.Index)-1 do
          if buildings:GetGreatWorkInSlot(info.Index,slot)==carrier.Index then
            local kind=buildings:GetGreatWorkTypeFromIndex(carrier.Index)
            local definition=assert(GameInfo.GreatWorks[kind],'work definition missing')
            local effects={}
            for row in GameInfo.GreatWork_YieldChanges() do
              if row.GreatWorkType==definition.GreatWorkType then effects[#effects+1]=row.YieldType .. ' ' .. tostring(row.YieldChange) end
            end
            if definition.Tourism and definition.Tourism~=0 then effects[#effects+1]='TOURISM ' .. tostring(definition.Tourism) end
            table.sort(effects)
            works[#works+1]=encode({Index=carrier.Index,Owner=owner,CityID=current:GetID(),
              X=current:GetX(),Y=current:GetY(),Building=info.Index,Slot=slot,
              GreatWorkType=definition.GreatWorkType,ObjectType=definition.GreatWorkObjectType,
              CreatorName=buildings:GetCreatorNameFromIndex(carrier.Index),
              TurnCreated=buildings:GetTurnFromIndex(carrier.Index),
              CreatedByPlayer=Game.GetGreatWorkPlayer(carrier.Index),Effects=effects})
          end
        end
      end
    end
  end
end
table.sort(units,function(a,b) if a.Owner~=b.Owner then return a.Owner<b.Owner end return a.ID<b.ID end)
local assets, religions, plots, plotIndices, districts = {}, {}, {}, {}, {}
local carrierBuildingType
local buildings=city:GetBuildings()
for info in GameInfo.Buildings() do
  if info.Index==carrier.Building then carrierBuildingType=info.BuildingType end
  if info.BuildingType~='BUILDING_PALACE' and buildings:HasBuilding(info.Index) then
    local slots={}
    for slot=0,buildings:GetNumGreatWorkSlots(info.Index)-1 do slots[slot]=buildings:GetGreatWorkInSlot(info.Index,slot) end
    assets[info.BuildingType]={Pillaged=buildings:IsPillaged(info.Index),Works=slots}
  end
end
for _, row in pairs(city:GetReligion():GetReligionsInCity()) do religions[row.Religion]=row.Followers end
for _, district in city:GetDistricts():Members() do
  local kind=GameInfo.Districts[district:GetType()].DistrictType
  local key=kind .. ':' .. tostring(district:GetX()) .. ':' .. tostring(district:GetY())
  assert(not districts[key], 'duplicate district location')
  districts[key]={Type=kind,X=district:GetX(),Y=district:GetY()}
end
for _, index in pairs(Map.GetCityPlots():GetPurchasedPlots(city)) do
  local plot=assert(Map.GetPlotByIndex(index),'purchased plot missing')
  local buyer=assert(Cities.GetPlotPurchaseCity(plot),'purchased city missing')
  plots[index]={Owner=plot:GetOwner(),City=buyer:GetID(),X=buyer:GetX(),Y=buyer:GetY()}
  assert(plot:GetOwner()==city:GetOwner() and buyer:GetOwner()==city:GetOwner()
    and buyer:GetX()==city:GetX() and buyer:GetY()==city:GetY(),'purchased plot belongs to another city')
  plotIndices[#plotIndices+1]=index
end
table.sort(plotIndices)
assets.Districts=districts
local cityState={Name=city:GetName(),Population=city:GetPopulation(),Assets=assets,
  Religion=religions,Districts=districts,Loyalty=city:GetCulturalIdentity():GetLoyalty(),
  Faith=city:GetProperty('JAC_ABSORPTION_CITY_FAITH_V1'),
  Governance=city:GetProperty('JAC_ABSORPTION_GOVERNANCE_V1'),UnitsOnCenter=units,
  PurchasedPlots=plotIndices,PurchasedPlotState=plots,CarrierBuildingType=assert(carrierBuildingType,'carrier building definition missing')}
assert(GameConfiguration and GameConfiguration.IsAnyMultiplayer, 'multiplayer getter missing')
return '{"schema_version":1,"candidate":' .. quote(candidate)
  .. ',"turn":' .. tostring(Game.GetCurrentGameTurn())
  .. ',"multiplayer":' .. encode(GameConfiguration.IsAnyMultiplayer())
  .. ',"local_player":' .. tostring(Game.GetLocalPlayer())
  .. ',"carrier":' .. encode(carrier)
  .. ',"ledger":' .. encode(state)
  .. ',"flow":' .. encode(Game:GetProperty('JAC_ABSORPTION_FLOW_V1'))
  .. ',"flow_required":' .. encode(flow_required)
  .. ',"city_owner":' .. tostring(city:GetOwner())
  .. ',"city_id":' .. tostring(city:GetID())
  .. ',"city":' .. encode(cityState)
  .. ',"locations":[' .. table.concat(works, ',') .. ']}'
"#;
