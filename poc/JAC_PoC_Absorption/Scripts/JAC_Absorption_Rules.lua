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
R.CITY_LOYALTY_DELTA_POC = 0
R.CULTURAL_FRICTION_TURNS_POC = 0
R.CONVERTED_GREAT_PERSON_REWARD_POC = 0
R.UNIT_SEARCH_RADIUS_POC = 8
R.LEADER_UNIT_TYPE_POC = "UNIT_GREAT_GENERAL"
R.LEADER_REPRESENTATION_POC = "PERSISTENT_PERSON_WITH_GENERAL_PROXY"
R.NATIVE_HERO_POLICY_POC = "REJECT_NATIVE_HERO_UNTIL_ENGINE_TRANSFER_IS_VERIFIED"
R.ENTRY_MODES = { CONQUEST = true, SMALL_STATE = true, MERGER = true }

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
    or (kind == "PERSON" and (outcome == "CAPTURE" or outcome == "RECRUIT" or outcome == "SUBMIT"))
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
