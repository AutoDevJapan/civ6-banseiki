-- SSOT 9.2/9.3, roadmap 16.1 bundle 2. This is the MOD's ledger,
-- not Civ6's city-pressure religion system. All mutations run in gameplay.
-- Official property API: DLC/BlackDeathScenario/Scripts/
-- BlackDeathScenario_StateUtils.lua:60-78. Table save/load was verified by bundle 0.
local KEY = "JAC_RP_STATE_V1"
local SCHEMA = 1
local RP = { PROPERTY_KEY = KEY, SCHEMA = SCHEMA }

local function check(ok, message)
  if not ok then error("[JAC_RP] " .. message, 3) end
end

local function identifier(value, label)
  check(type(value) == "string" and #value > 0 and #value <= 128
    and value:match("^[A-Za-z0-9_:%-%.]+$") ~= nil, label .. " must be a stable ID")
end

local function count(value, label, positive)
  check(type(value) == "number" and value == value and value < math.huge
    and value >= (positive and 1 or 0) and value == math.floor(value)
    and value <= 2147483647, label .. " must be a bounded integer")
end

local function copy(value)
  if type(value) ~= "table" then return value end
  local result = {}
  for k, v in pairs(value) do result[k] = copy(v) end
  return result
end

local function keys(value)
  local result = {}
  for k in pairs(value) do result[#result + 1] = k end
  table.sort(result)
  return result
end

-- Canonical, read-only snapshot for external pre-save/post-load comparisons.
local function encode(value)
  if type(value) == "table" then
    local result = {}
    for _, k in ipairs(keys(value)) do
      result[#result + 1] = string.format("%q", k) .. ":" .. encode(value[k])
    end
    return "{" .. table.concat(result, ",") .. "}"
  elseif type(value) == "string" then
    return string.format("%q", value)
  end
  return tostring(value)
end

local catalog = {}
for _, name in ipairs({ "Religions", "AssetKinds", "Jobs", "Sites", "Outcomes" }) do
  catalog[name] = {}
  check(GameInfo["JAC_RP_" .. name] ~= nil, "missing catalog " .. name)
  for row in GameInfo["JAC_RP_" .. name]() do
    catalog[name][row.ID] = row
  end
end

local function known(name, id)
  identifier(id, name)
  check(catalog[name][id] ~= nil, "unknown " .. name .. " ID " .. id)
end

local function recoverySource(asset)
  return asset ~= nil and catalog.AssetKinds[asset.kind].RevivalSource == 1
end

local function derive(religion)
  local followers, facilities, source = 0, 0, false
  for _, c in pairs(religion.communities) do
    followers = followers + c.followers
    facilities = facilities + c.publicFacilities
  end
  count(followers, "world followers")
  count(facilities, "world public facilities")
  for _, asset in pairs(religion.assets) do source = source or recoverySource(asset) end
  religion.followers, religion.publicFacilities = followers, facilities
  religion.lost = followers == 0
  -- Keep the evidence of underground continuity independently of lost status.
  -- SSOT's zero-follower rule labels it LOST; surviving assets are never erased.
  religion.underground = facilities == 0 and (followers > 0 or next(religion.assets) ~= nil)
  religion.state = religion.lost and "LOST" or (religion.underground and "UNDERGROUND" or "PUBLIC")
  religion.canRevive = religion.lost and source
end

local function read()
  local state = Game:GetProperty(KEY)
  if state == nil then return { schema = SCHEMA, revision = 0, religions = {} } end
  check(type(state) == "table" and state.schema == SCHEMA,
    "unsupported or damaged save schema; refusing to replace it")
  check(type(state.religions) == "table", "damaged religion ledger")
  return copy(state)
end

local function get(state, id)
  known("Religions", id)
  local religion = state.religions[id]
  check(religion ~= nil, "religion not founded: " .. id)
  return religion
end

local function commit(state)
  state.revision = state.revision + 1
  for _, id in ipairs(keys(state.religions)) do derive(state.religions[id]) end
  Game:SetProperty(KEY, state)
  print("[JAC_RP] committed revision=" .. state.revision)
end

local function community(id, followers, facilities, holder)
  identifier(id, "community")
  identifier(holder, "community holder")
  count(followers, "followers")
  count(facilities, "public facilities")
  return { followers = followers, publicFacilities = facilities, holder = holder }
end

function RP.Found(id, founder, communityID, followers, facilities, holder)
  known("Religions", id)
  count(founder, "founder player")
  count(followers, "founding followers", true)
  local c = community(communityID, followers, facilities, holder)
  local state = read()
  check(state.religions[id] == nil, "religion already founded")
  state.religions[id] = { id = id, founder = founder, founderDefeated = false,
    foundedTurn = Game.GetCurrentGameTurn(), communities = { [communityID] = c },
    assets = {}, discoveries = {}, revivals = {}, revivalCount = 0 }
  commit(state)
end

function RP.SetCommunity(id, communityID, followers, facilities, holder)
  local c = community(communityID, followers, facilities, holder)
  local state = read()
  get(state, id).communities[communityID] = c
  commit(state)
end

-- Adapter input: a faction's defeat records provenance, never deletes believers.
function RP.NotifyFounderDefeated(founder)
  count(founder, "founder player")
  local state, changed = read(), false
  for _, religion in pairs(state.religions) do
    if religion.founder == founder and not religion.founderDefeated then
      religion.founderDefeated, changed = true, true
    end
  end
  if changed then commit(state) end
end

function RP.PreserveAsset(id, assetID, kind, holder)
  identifier(assetID, "asset")
  known("AssetKinds", kind)
  identifier(holder, "asset holder")
  local state = read()
  local religion = get(state, id)
  check(religion.assets[assetID] == nil, "asset already exists; use MoveAsset")
  religion.assets[assetID] = { kind = kind, holder = holder }
  commit(state)
end

function RP.MoveAsset(id, assetID, holder)
  identifier(assetID, "asset")
  identifier(holder, "asset holder")
  local state = read()
  local asset = get(state, id).assets[assetID]
  check(asset ~= nil, "asset absent")
  asset.holder = holder
  commit(state)
end

function RP.RemoveAsset(id, assetID)
  identifier(assetID, "asset")
  local state = read()
  local religion = get(state, id)
  check(religion.assets[assetID] ~= nil, "asset absent")
  religion.assets[assetID] = nil
  commit(state)
end

-- The exploration adapter supplies job, place and identity. No invented chance,
-- turn schedule, map coordinates or rewards; the PoC discovers one explicit item.
function RP.Discover(id, discoveryID, assetID, kind, actor, job, site)
  identifier(discoveryID, "discovery")
  identifier(assetID, "asset")
  identifier(actor, "actor")
  known("Jobs", job)
  known("Sites", site)
  check(kind == "SCRIPTURE" or kind == "RELIC", "discovery must be scripture or relic")
  local state = read()
  local religion = get(state, id)
  check(religion.lost, "discovery requires a lost religion")
  check(religion.discoveries[discoveryID] == nil, "discovery already exists")
  local asset = religion.assets[assetID]
  if asset then
    check(asset.kind == kind and asset.holder == site, "item is not at the supplied site")
  else
    asset = { kind = kind, holder = site }
    religion.assets[assetID] = asset
  end
  asset.holder = actor
  religion.discoveries[discoveryID] = { asset = assetID, kind = kind, actor = actor,
    job = job, site = site, foundTurn = Game.GetCurrentGameTurn() }
  commit(state)
end

local function revive(religion, revivalID, sourceID, communityID, followers, facilities, holder)
  identifier(revivalID, "revival")
  identifier(sourceID, "revival source")
  count(followers, "revival followers", true)
  local c = community(communityID, followers, facilities, holder)
  check(religion.lost, "only a lost religion can be revived")
  check(recoverySource(religion.assets[sourceID]), "no surviving revival source")
  check(religion.revivals[revivalID] == nil, "revival already recorded")
  religion.communities[communityID] = c
  religion.revivalCount = religion.revivalCount + 1
  religion.revivals[revivalID] = { source = sourceID, community = communityID,
    followers = followers, publicFacilities = facilities, holder = holder,
    turn = Game.GetCurrentGameTurn() }
end

function RP.Revive(id, revivalID, sourceID, communityID, followers, facilities, holder)
  local state = read()
  revive(get(state, id), revivalID, sourceID, communityID, followers, facilities, holder)
  commit(state)
end

function RP.ResolveDiscovery(id, discoveryID, outcome, revivalID, communityID, followers, facilities, holder)
  identifier(discoveryID, "discovery")
  known("Outcomes", outcome)
  local state = read()
  local religion = get(state, id)
  local discovery = religion.discoveries[discoveryID]
  check(discovery ~= nil and discovery.outcome == nil, "discovery absent or already resolved")
  if outcome == "REVIVE" then
    revive(religion, revivalID, discovery.asset, communityID, followers, facilities, holder)
    discovery.revival = revivalID
    discovery.effects = "APPLIED"
  else
    -- SSOT enumerates branches but not their effects. Preserve a selection for
    -- the future adapter, explicitly distinguish it from an applied reward.
    discovery.effects = "DEFERRED"
  end
  discovery.outcome, discovery.resolvedTurn = outcome, Game.GetCurrentGameTurn()
  commit(state)
end

function RP.Inspect(id)
  local state = read()
  local religion = get(state, id)
  derive(religion)
  return religion
end

function RP.Snapshot()
  return encode(read())
end

function RP.Export()
  return read()
end

-- No turn/load hook recreates or normalizes saved data. Read fresh on every call:
-- an old save in the same process cannot inherit the previous game's Lua locals.
ExposedMembers.JAC_ReligionPersistence = RP
local loaded = read()
print("[JAC_RP] loaded schema=" .. loaded.schema .. " revision=" .. loaded.revision
  .. " religions=" .. #keys(loaded.religions))
