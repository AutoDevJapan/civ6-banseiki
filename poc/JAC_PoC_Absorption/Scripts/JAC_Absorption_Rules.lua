-- SSOT 9.1. These are PoC policies, not balance decisions or the old nine states.
JAC_AbsorptionRules = {}
local R = JAC_AbsorptionRules
R.STATE_KEY = "JAC_ABSORPTION_STATE_V1"
R.ROSTER_KEY = "JAC_ABSORPTION_ASSETS_V1"
R.PERSON_KEY = "JAC_ABSORPTION_PERSON_V1"
R.CITY_FAITH_KEY = "JAC_ABSORPTION_CITY_FAITH_V1"
R.GOVERNANCE_KEY = "JAC_ABSORPTION_GOVERNANCE_V1"
R.TOKEN_KEY = "JAC_ABSORPTION_UNIT_TOKEN_V1"
R.REQUIRE_EXPLICIT_DISPOSITIONS_POC = true
-- Provisional magnitudes. The official Expansion1_GlobalParameters.xml uses
-- 50 after combat transfer and 100 after cultural-identity transfer; espionage
-- foment unrest is -15 (-5 per promotion level). Friction is a separate,
-- once-per-turn loyalty change, never a culture/science yield from a person.
R.CITY_POLICY = {
  CONQUEST = { Immediate = -15, Friction = -2, Turns = 3 },
  SMALL_STATE = { Immediate = -5, Friction = -1, Turns = 2 },
  MERGER = { Immediate = 0, Friction = 0, Turns = 0 },
}
R.UNIT_SEARCH_RADIUS_POC = 8
R.LEADER_UNIT_TYPE_POC = "UNIT_GREAT_GENERAL"
R.LEADER_REPRESENTATION_POC = "PERSISTENT_PERSON_WITH_GENERAL_PROXY"
R.NATIVE_HERO_POLICY_POC = "TRANSFER_OR_ARCHIVE_SUICIDE_WITH_CAUSE"
R.ENTRY_MODES = { CONQUEST = true, SMALL_STATE = true, MERGER = true }

-- SSOT 9.1 provisional flow policy; all rates are basis points (0..10000).
R.FLOW_FIRST_ESCAPE = { CONQUEST = 2400, SMALL_STATE = 1400, MERGER = 700 }
R.FLOW_METHOD_ACTION = { CONQUEST = -700, SMALL_STATE = 0, MERGER = 600 }
R.FLOW_ACTION_SUCCESS = {
  CAPTURE = 8800, RECRUIT = 6800, SUBMIT = 7800, EXECUTE = 9500,
  PROTECT = 8200, RECONTRACT = 7000, SUICIDE = 9500,
  TRANSFER = 9400, LOOT = 8800, SEIZE = 8600, RETURN = 9000,
  SURRENDER = 9100, INCORPORATE = 8500, DISSOLVE = 9500,
  INHERIT = 10000, PRESERVE = 10000,
}
R.FLOW_RATE = {
  RollRange = 10000, AttitudeMinimum = 0, AttitudeMaximum = 100,
  NeutralAttitude = 50, LoyaltyEscapeWeight = 22, RelationEscapeWeight = 12,
  ProtectedEscapeDelta = -500, BrokenEscapeDelta = 500,
  ItemEscapeNumerator = 3, ItemEscapeDenominator = 4,
  RetryEscapeDelta = 500, EscapeMaximum = 9500,
  LoyaltyActionWeight = 15, RelationActionWeight = 10,
  ActionMinimum = 500, ActionMaximum = 9900,
  WorldDelayTurns = 2,
}

function R.UnitKind(unit)
  local person = unit:GetProperty(R.PERSON_KEY)
  if unit.IsHero and unit:IsHero() then return "HERO" end
  local info = assert(GameInfo.Units[unit:GetType()], "unknown unit definition")
  if info.UnitType == "UNIT_GREAT_GENERAL" or unit:GetProperty("JAC_POC_GROWTHUI_WARLORD") then
    return "PERSON"
  end
  if person and person.Kind then return person.Kind end
  local gp = unit:GetGreatPerson()
  if gp and gp:GetIndividual() >= 0 then return "GREAT_PERSON" end
  return "UNIT"
end

-- Database rows supply the dispositions, so a missing/invalid decision fails before mutation.
function R.Destination(kind, outcome)
  assert(type(outcome) == "string", "explicit disposition required for " .. kind)
  for row in GameInfo.JAC_AbsorptionOutcomes() do
    if row.Kind == kind and row.Outcome == outcome then return row.Destination end
  end
  error("invalid disposition " .. kind .. "/" .. outcome)
end

function R.Keys(t)
  local keys = {}
  for k in pairs(t) do
    assert(type(k) == "string" or type(k) == "number", "unsupported property key")
    keys[#keys + 1] = k
  end
  table.sort(keys, function(a, b)
    if type(a) ~= type(b) then return type(a) < type(b) end
    return a < b
  end)
  return keys
end

function R.Copy(value, seen)
  local t = type(value)
  if t == "nil" or t == "boolean" then return value end
  if t == "number" then
    assert(value == value and value ~= math.huge and value ~= -math.huge, "non-finite property")
    return value
  end
  if t == "string" then
    assert(value ~= "", "empty strings do not survive Civ6 saves; use nil")
    return value
  end
  assert(t == "table", "property must be serializable: " .. t)
  seen = seen or {}
  assert(not seen[value], "cyclic property")
  seen[value] = true
  local result = {}
  for _, k in ipairs(R.Keys(value)) do result[k] = R.Copy(value[k], seen) end
  seen[value] = nil
  return result
end

function R.Canonical(value)
  local t = type(value)
  if t ~= "table" then return t .. ":" .. (t == "string" and string.format("%q", value) or tostring(value)) end
  local out = { "{" }
  for _, k in ipairs(R.Keys(value)) do
    out[#out + 1] = R.Canonical(k) .. "=" .. R.Canonical(value[k]) .. ";"
  end
  out[#out + 1] = "}"
  return table.concat(out)
end

function R.Equal(a, b) return R.Canonical(a) == R.Canonical(b) end
function R.UnitKey(owner, id) return tostring(owner) .. ":" .. tostring(id) end
function R.KeepUnit(kind, outcome)
  return (kind == "UNIT" and (outcome == "SURRENDER" or outcome == "INCORPORATE"))
    or ((kind == "PERSON" or kind == "GOVERNOR")
      and (outcome == "CAPTURE" or outcome == "RECRUIT" or outcome == "SUBMIT"))
    or (kind == "HERO" and (outcome == "RECRUIT" or outcome == "PROTECT" or outcome == "RECONTRACT"))
    or (kind == "GREAT_PERSON" and outcome == "RECRUIT")
end

-- Only explicit component references are rewritten; Founder, OriginalOwner, historical IDs stay intact.
function R.Rebind(record, tx)
  local result = R.Copy(record)
  if result.Unit and result.Unit.Owner == tx.From then
    local old = result.Unit
    local new = tx.UnitMap[R.UnitKey(old.Owner, old.ID)]
    result.FormerUnit = R.Copy(old)
    result.Unit = new and { Owner = tx.To, ID = new.ID, Token = new.Token } or nil
  end
  if result.City and result.City.Owner == tx.From then result.City.Owner = tx.To end
  return result
end
