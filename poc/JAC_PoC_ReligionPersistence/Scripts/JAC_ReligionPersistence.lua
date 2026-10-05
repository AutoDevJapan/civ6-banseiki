-- SSOT 9.2/9.3, roadmap 16.1 bundle 2. This is the MOD's ledger,
-- with explicit native-religion bindings for citizen conversion. Gameplay only.
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

local RULES = {}
for row in GameInfo.JAC_RP_Rules() do RULES[row.ID] = row.Value end
function RP.Rules() return copy(RULES) end

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

local function recoverySource(religion, assetID)
  local asset, use = religion.assets[assetID], religion.materialUses[assetID]
  return asset ~= nil and catalog.AssetKinds[asset.kind].RevivalSource == 1
    and (use == nil or use.effects == "APPLIED" and
      (use.outcome == "CONCEAL" or use.outcome == "REVIVE" and use.reusable == true))
end

local nativeFollowerCount
local function derive(religion)
  local followers, facilities, source = 0, 0, false
  for _, c in pairs(religion.communities) do
    followers = followers + c.followers
    facilities = facilities + c.publicFacilities
  end
  -- A binding declares these to be the SAME religion. Native citizens are its
  -- source of truth; a manual community input must never label it lost while
  -- the bound engine religion still has believers.
  if religion.engineReligion ~= nil then followers = nativeFollowerCount(religion.engineReligion) end
  count(followers, "world followers")
  count(facilities, "world public facilities")
  for assetID in pairs(religion.assets) do source = source or recoverySource(religion, assetID) end
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
  if state == nil then return { schema = SCHEMA, revision = 0, religions = {}, heroes = {},
    artifacts = {}, religiousAlliances = {} } end
  check(type(state) == "table" and state.schema == SCHEMA,
    "unsupported or damaged save schema; refusing to replace it")
  check(type(state.religions) == "table", "damaged religion ledger")
  state = copy(state)
  -- Additive V1 extension, read-only until a real mutation. Old DEFERRED choices
  -- remain history; loading a save never pays a previously undecided reward.
  state.heroes, state.artifacts = state.heroes or {}, state.artifacts or {}
  state.religiousAlliances = state.religiousAlliances or {}
  for _, religion in pairs(state.religions) do
    religion.materialUses = religion.materialUses or {}
    religion.outcomeUses = religion.outcomeUses or {}
    religion.retiredAssets = religion.retiredAssets or {}
    for discoveryID, d in pairs(religion.discoveries) do
      if d.outcome and religion.materialUses[d.asset] == nil then
        religion.materialUses[d.asset] = { outcome = d.outcome, effects = "LEGACY",
          discovery = discoveryID, turn = d.resolvedTurn }
      end
    end
    for revivalID, v in pairs(religion.revivals) do
      if religion.materialUses[v.source] == nil then
        religion.materialUses[v.source] = { outcome = "REVIVE", effects = "LEGACY",
          revival = revivalID, turn = v.turn }
      end
    end
    derive(religion)
  end
  return state
end

local function get(state, id)
  known("Religions", id)
  local religion = state.religions[id]
  check(religion ~= nil, "religion not founded: " .. id)
  return religion
end

local function commit(state)
  -- The saved counter is bounded; freshness is tracked independently so
  -- ordinary RP actions remain available after the counter saturates.
  state.revision = math.min(state.revision + 1, 2147483647)
  state.absorptionExactReceipt = nil
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
    assets = {}, discoveries = {}, revivals = {}, revivalCount = 0,
    materialUses = {}, outcomeUses = {}, retiredAssets = {} }
  commit(state)
end

function RP.SetCommunity(id, communityID, followers, facilities, holder)
  local c = community(communityID, followers, facilities, holder)
  local state = read()
  get(state, id).communities[communityID] = c
  commit(state)
end

-- Exploration adapters bind a catalog religion to an already founded native
-- religion; never infer a native ID from a placeholder name or founder's ID.
-- Native table: DLC/Expansion2/Scripts/WorldCongress.lua:29.
function RP.BindEngineReligion(id, religionType)
  count(religionType, "native religion")
  check(GameInfo.Religions[religionType] ~= nil, "unknown native religion")
  local founded = false
  for _, r in ipairs(Game.GetReligion():GetReligions()) do
    founded = founded or r.Religion == religionType
  end
  check(founded, "native religion must already be founded")
  local state = read()
  local religion = get(state, id)
  check(religion.engineReligion == nil or religion.engineReligion == religionType, "native binding cannot change")
  for otherID, other in pairs(state.religions) do
    check(otherID == id or other.engineReligion ~= religionType, "native religion already bound")
  end
  religion.engineReligion = religionType
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

function RP.PreserveAsset(id, assetID, kind, holder, owner)
  identifier(assetID, "asset")
  known("AssetKinds", kind)
  identifier(holder, "asset holder")
  if owner ~= nil then count(owner, "asset owner") end
  local state = read()
  local religion = get(state, id)
  check(religion.assets[assetID] == nil, "asset already exists; use MoveAsset")
  check(religion.materialUses[assetID] == nil and not religion.retiredAssets[assetID], "retired material ID")
  for _, d in pairs(religion.discoveries) do check(d.asset ~= assetID, "discovered material ID") end
  religion.assets[assetID] = { kind = kind, holder = holder, owner = owner }
  commit(state)
end

function RP.MoveAsset(id, assetID, holder, owner)
  identifier(assetID, "asset")
  identifier(holder, "asset holder")
  if owner ~= nil then count(owner, "asset owner") end
  local state = read()
  local religion = get(state, id)
  local asset, use = religion.assets[assetID], religion.materialUses[assetID]
  check(asset ~= nil, "asset absent")
  check(use == nil or use.effects ~= "PENDING", "material effect pending")
  asset.holder, asset.owner = holder, owner
  commit(state)
end

function RP.RemoveAsset(id, assetID)
  identifier(assetID, "asset")
  local state = read()
  local religion = get(state, id)
  check(religion.assets[assetID] ~= nil, "asset absent")
  local use = religion.materialUses[assetID]
  check(use == nil or use.effects ~= "PENDING", "material effect pending")
  religion.retiredAssets[assetID] = true
  religion.assets[assetID] = nil
  commit(state)
end

local function player(playerID)
  count(playerID, "player")
  local p = Players[playerID]
  -- Official gameplay: BlackDeathScenario.lua:122; PiratesScenario_StartScript.lua:866.
  check(p ~= nil and p:IsAlive() and p:IsMajor(), "requires a living major player")
  return p
end

local function finite(value, label)
  check(type(value) == "number" and value == value and value >= 0 and value < math.huge,
    "invalid " .. label)
  return value
end

local function cityFollowers(city)
  local followers, total = {}, 0
  -- GetReligionsInCity is UI-only. The live Gameplay method is GetNumFollowers.
  for row in GameInfo.Religions() do
    if row.Index > 0 then
      local n = city:GetReligion():GetNumFollowers(row.Index)
      count(n, "native followers")
      followers[tostring(row.Index)] = n
      total = total + n
    end
  end
  check(total <= city:GetPopulation(), "native follower counts exceed population")
  followers["-1"] = city:GetPopulation() - total
  return followers
end

local function nativeCities()
  local cities = {}
  -- All living players: DLC/VikingsScenario/Scripts/VikingScenario.lua:224-234.
  for _, p in ipairs(PlayerManager.GetAlive()) do
    for _, city in p:GetCities():Members() do
      cities[#cities + 1] = { player = p:GetID(), city = city:GetID(), object = city }
    end
  end
  table.sort(cities, function(a, b)
    return a.player < b.player or a.player == b.player and a.city < b.city
  end)
  return cities
end

nativeFollowerCount = function(engineReligion)
  local total = 0
  for _, entry in ipairs(nativeCities()) do
    local followers = cityFollowers(entry.object)
    total = total + (followers[tostring(engineReligion)] or 0)
  end
  count(total, "world native followers")
  return total
end

function RP.NativeFollowers(id)
  local religion = get(read(), id)
  check(religion.engineReligion ~= nil, "native religion binding required")
  return nativeFollowerCount(religion.engineReligion)
end

local function absorptionPlan(source, target, p)
  local own = p:GetReligion():GetReligionTypeCreated() -- ReligionScreen.lua:123
  check(source.engineReligion ~= nil and target.engineReligion == own and own >= 0,
    "absorption needs bound source and the player's own founded religion")
  check(source.engineReligion ~= own, "native source and target must differ")
  local owned = false
  for _, city in p:GetCities():Members() do
    owned = owned or city:GetReligion():GetNumFollowers(own) > 0
  end
  check(owned, "target religion needs own native followers")
  local entries, total = {}, 0
  for _, entry in ipairs(nativeCities()) do
    local followers = cityFollowers(entry.object)
    local n = followers[tostring(source.engineReligion)] or 0
    if n > 0 then
      check(entry.object:GetReligion().AddReligiousPressure ~= nil, "native pressure mutation unavailable")
      total = total + n
      entries[#entries + 1] = { player = entry.player, city = entry.city,
        before = followers, available = n }
    end
  end
  count(total, "world native followers")
  local remaining = math.ceil(total / RULES.ABSORB_DIVISOR)
  count(remaining, "converted followers", true)
  local conversions, moved = {}, remaining
  for _, entry in ipairs(entries) do
    if remaining == 0 then break end
    entry.moved = math.min(entry.available, remaining)
    entry.after = copy(entry.before)
    local from, to = tostring(source.engineReligion), tostring(own)
    entry.after[from] = entry.after[from] - entry.moved
    entry.after[to] = (entry.after[to] or 0) + entry.moved
    conversions[#conversions + 1] = entry
    remaining = remaining - entry.moved
  end
  return conversions, moved, total, own
end

local function convertCitizens(entry)
  local city = Players[entry.player]:GetCities():FindID(entry.city)
  check(city ~= nil, "conversion city disappeared")
  local current = cityFollowers(city)
  for key, n in pairs(entry.before) do check((current[key] or 0) == n, "conversion population changed") end
  for key, n in pairs(current) do check((entry.before[key] or 0) == n, "conversion population changed") end
  -- Civ6 exposes pressure mutation, not a documented ConvertFollowers API.
  -- AddReligiousPressure(0, religion, pressure, -1): VikingScenario.lua:103.
  -- Reweight real religions only. The unaffiliated pressure is engine-owned.
  -- Find an integer scale that preserves its counted citizens as well as all
  -- unrelated religions, rather than passing a negative religion ID to the API.
  local cr = city:GetReligion()
  local union = copy(entry.after)
  union["-1"] = nil
  local low, high, scale = 0, nil, 1024
  -- Native pressure uses signed 24.8 fixed point (Lua integers shift left 8).
  local ceiling = math.floor(8388607 / city:GetPopulation())
  check(ceiling >= 1, "population exceeds native pressure precision")
  scale = math.min(scale, ceiling)
  local lastAfter
  for attempt = 1, 64 do
    -- RemovePressureOneReligion(religion, pressure, responsiblePlayer)
    -- subtracts an amount, not a percentage. Omitting the amount removes zero.
    -- Two bounded subtractions clear even a maximum fixed-point pressure.
    -- Never clear the engine-owned unaffiliated/pantheon pressure.
    for _, key in ipairs(keys(union)) do
      cr:RemovePressureOneReligion(tonumber(key), 8388607, -1)
      cr:RemovePressureOneReligion(tonumber(key), 1, -1)
    end
    if attempt == 1 and GameConfiguration.GetValue("JAC_AUTOTEST") == 1 then
      print("[JAC_RP] after pressure clearing=" .. encode(cityFollowers(city)))
    end
    for _, key in ipairs(keys(union)) do
      local amount = (entry.after[key] or 0) * scale
      if amount > 0 then cr:AddReligiousPressure(0, tonumber(key), amount, -1) end
    end
    local after, exact = cityFollowers(city), true
    lastAfter = after
    if GameConfiguration.GetValue("JAC_AUTOTEST") == 1 then
      print("[JAC_RP] conversion player=" .. entry.player .. " city=" .. entry.city
        .. " attempt=" .. attempt .. " scale=" .. scale .. " expected=" .. encode(entry.after)
        .. " actual=" .. encode(after))
    end
    for key, n in pairs(entry.after) do exact = exact and (after[key] or 0) == n end
    for key, n in pairs(after) do exact = exact and n == (entry.after[key] or 0) end
    if exact then return end
    if after["-1"] > entry.after["-1"] then low = scale else high = scale end
    local nextScale = high and math.floor((low + high) / 2) or math.min(scale * 2, ceiling)
    if nextScale < 1 or nextScale == scale or nextScale == low then break end
    scale = nextScale
  end
  error("[JAC_RP] native conversion count mismatch; effect pending; expected="
    .. encode(entry.after) .. " actual=" .. encode(lastAfter))
end

local function allianceActive(a)
  local p, origin = Players[a.player], Players[a.origin]
  return a.active and p ~= nil and origin ~= nil and p:IsAlive() and origin:IsAlive()
    and not p:GetDiplomacy():IsAtWarWith(a.origin) and p:GetDiplomacy():HasAllied(a.origin)
end

function RP.RefreshReligiousAlliances()
  local state, changed = read(), false
  for _, a in pairs(state.religiousAlliances) do
    if a.active and not allianceActive(a) then
      a.active, a.endedTurn, changed = false, Game.GetCurrentGameTurn(), true
    end
  end
  if changed then commit(state) end
  return state
end

-- No SSOT persecution effect exists yet. Future coercion/removal adapters must
-- call this guard before effects. Ordinary spread/voluntary absorption is not
-- blocked. This does not intercept native inquisitors or theological combat.
function RP.CanPersecute(actorPlayer, actingReligion, citizenPlayer, targetReligion)
  count(actorPlayer, "persecutor")
  count(citizenPlayer, "citizen owner")
  known("Religions", actingReligion)
  known("Religions", targetReligion)
  local state = RP.RefreshReligiousAlliances()
  for _, a in pairs(state.religiousAlliances) do
    if allianceActive(a) and (actingReligion == a.religion and citizenPlayer == a.player
      or actorPlayer == a.player and targetReligion == a.religion) then
      return false, "religious alliance prohibits persecution"
    end
  end
  return true
end

function RP.AssertCanPersecute(actorPlayer, actingReligion, citizenPlayer, targetReligion)
  local allowed, reason = RP.CanPersecute(actorPlayer, actingReligion, citizenPlayer, targetReligion)
  check(allowed, reason)
end

local function researchBoosts(p)
  local techs, candidates, boostable = p:GetTechs(), {}, {}
  for row in GameInfo.Boosts() do
    if row.TechnologyType ~= nil then boostable[row.TechnologyType] = true end
  end
  -- Gameplay example: AustraliaScenario.lua:768-784. CanTriggerBoost is UI-only.
  for row in GameInfo.Technologies() do
    if boostable[row.TechnologyType] and not techs:HasTech(row.Index)
      and not techs:HasBoostBeenTriggered(row.Index) then
      candidates[#candidates + 1] = row
    end
  end
  table.sort(candidates, function(a, b) return a.TechnologyType < b.TechnologyType end)
  check(#candidates >= RULES.RESEARCH_EUREKAS, "requires two unresearched, unboosted technologies")
  local boosts = {}
  for n = 1, RULES.RESEARCH_EUREKAS do boosts[n] = candidates[n].Index end
  return boosts
end

local function heroPlan(p, city)
  check(GameConfiguration.GetValue("GAMEMODE_HEROES") == true
    or GameConfiguration.GetValue("GAMEMODE_HEROES") == 1, "official Heroes & Legends mode required")
  local bridge = ExposedMembers.JAC_RP_HeroUI
  check(bridge ~= nil and GameInfo.HeroClasses ~= nil, "official Heroes UI bridge unavailable")
  local originBuilding = bridge.OriginBuilding(p:GetID())
  check(city:GetBuildings():HasBuilding(originBuilding), "hero city requires the official origin building")
  local candidates = {}
  for row in GameInfo.HeroClasses() do
    if bridge.ClaimPlayer(row.Index) == -1 then candidates[#candidates + 1] = row end
  end
  table.sort(candidates, function(a, b) return a.HeroClassType < b.HeroClassType end)
  check(#candidates > 0, "no unclaimed official hero")
  local hero, modifier = candidates[1], nil
  for row in GameInfo.ProjectCompletionModifiers() do
    if row.ProjectType == hero.CreationProjectType then
      local def = GameInfo.Modifiers[row.ModifierId]
      if def and def.ModifierType == "MODIFIER_CITY_GRANT_HERO" then modifier = row.ModifierId; break end
    end
  end
  check(modifier ~= nil and GameInfo.Units[hero.UnitType] ~= nil, "official hero grant missing")
  return hero, modifier
end

-- Exploration supplies facts; natural map triggers remain a later adapter.
function RP.Discover(id, discoveryID, assetID, kind, actor, job, site, playerID)
  identifier(discoveryID, "discovery")
  identifier(assetID, "asset")
  identifier(actor, "actor")
  known("Jobs", job)
  known("Sites", site)
  check(kind == "SCRIPTURE" or kind == "RELIC", "discovery must be scripture or relic")
  player(playerID)
  local state = read()
  local religion = get(state, id)
  check(religion.lost, "discovery requires a lost religion")
  check(religion.discoveries[discoveryID] == nil, "discovery already exists")
  check(religion.materialUses[assetID] == nil and not religion.retiredAssets[assetID], "material already used")
  for _, d in pairs(religion.discoveries) do check(d.asset ~= assetID, "material already discovered") end
  local asset = religion.assets[assetID]
  if asset then
    check(asset.kind == kind and asset.holder == site, "item is not at the supplied site")
  else
    asset = { kind = kind, holder = site }
    religion.assets[assetID] = asset
  end
  asset.holder, asset.owner = actor, playerID
  religion.discoveries[discoveryID] = { asset = assetID, kind = kind, actor = actor,
    job = job, site = site, player = playerID, foundTurn = Game.GetCurrentGameTurn() }
  commit(state)
end

local function freeCommunityID(religion, preferredID)
  local candidate, suffix = preferredID, 0
  while religion.communities[candidate] ~= nil do
    suffix = suffix + 1
    local ending = ":revival_" .. suffix
    candidate = preferredID:sub(1, 128 - #ending) .. ending
  end
  return candidate
end

local function revivalPlan(state, id, revivalID, sourceID, communityID, playerID, cityID)
  local religion, p = get(state, id), player(playerID)
  identifier(revivalID, "revival")
  identifier(sourceID, "revival source")
  identifier(communityID, "community")
  count(cityID, "city")
  local city = p:GetCities():FindID(cityID)
  check(city ~= nil, "requires an owned city")
  check(religion.lost, "only a lost religion can be revived")
  check(recoverySource(religion, sourceID), "no unused surviving revival source")
  check(religion.assets[sourceID].owner == playerID, "revival material not owned")
  check(religion.revivals[revivalID] == nil, "revival already recorded")
  for _, v in pairs(religion.revivals) do
    check(v.effects ~= "PENDING", "revival effect pending")
    check(Game.GetCurrentGameTurn() - v.turn >= RULES.REVIVAL_COOLDOWN_TURNS, "revival cooldown")
  end
  check(p:GetReligion():GetFaithBalance() >= RULES.REVIVAL_FAITH_COST, "insufficient revival faith")
  local conversion
  if religion.engineReligion ~= nil then
    local before = cityFollowers(city)
    local donor = before["-1"] > 0 and "-1" or nil
    if donor == nil then
      local ordered = keys(before)
      table.sort(ordered, function(a, b) return tonumber(a) < tonumber(b) end)
      for _, key in ipairs(ordered) do if before[key] > 0 then donor = key; break end end
    end
    check(donor ~= nil, "revival needs a citizen in the owned city")
    local after = copy(before)
    after[donor] = after[donor] - RULES.REVIVAL_FOLLOWERS
    local target = tostring(religion.engineReligion)
    after[target] = (after[target] or 0) + RULES.REVIVAL_FOLLOWERS
    conversion = { player = playerID, city = cityID, before = before, after = after }
  end
  communityID = freeCommunityID(religion, communityID)
  religion.revivals[revivalID] = { source = sourceID, community = communityID,
    followers = RULES.REVIVAL_FOLLOWERS, publicFacilities = 0, holder = "PLAYER_" .. playerID,
    player = playerID, city = cityID, faithCost = RULES.REVIVAL_FAITH_COST,
    effects = "PENDING", turn = Game.GetCurrentGameTurn() }
  return { player = playerID, faithCost = RULES.REVIVAL_FAITH_COST, revival = revivalID,
    conversions = conversion and { conversion } or nil, nativeRevival = religion.engineReligion ~= nil }
end

local function reserveMaterial(religion, assetID, outcome, playerID, discoveryID, revivalID)
  religion.materialUses[assetID] = { outcome = outcome, player = playerID,
    discovery = discoveryID, revival = revivalID, effects = "PENDING", turn = Game.GetCurrentGameTurn(),
    reusable = outcome == "REVIVE" }
end

local function discoveryPlan(id, discoveryID, outcome, options)
  identifier(discoveryID, "discovery")
  known("Outcomes", outcome)
  check(type(options) == "table", "resolution options required")
  check(options.followers == nil and options.publicFacilities == nil, "revival size is fixed by rules")
  local state = read()
  local religion = get(state, id)
  local discovery = religion.discoveries[discoveryID]
  check(discovery ~= nil and discovery.outcome == nil, "discovery absent or already resolved")
  local p = player(discovery.player)
  check(options.player == nil or options.player == discovery.player, "discovery player mismatch")
  local asset = religion.assets[discovery.asset]
  check(asset ~= nil and asset.owner == discovery.player and asset.holder == discovery.actor,
    "discovered material absent or moved")
  check(religion.materialUses[discovery.asset] == nil, "material already used")
  local useKey = tostring(discovery.player) .. ":" .. outcome
  check(outcome == "REVIVE" or religion.outcomeUses[useKey] == nil,
    "branch already used by this player for this religion")
  local plan = { player = discovery.player, asset = discovery.asset, discovery = discoveryID,
    outcome = outcome, faithCost = 0 }
  if outcome == "REVIVE" then
    plan = revivalPlan(state, id, options.revival, discovery.asset, options.community,
      discovery.player, options.city)
    plan.asset, plan.discovery, plan.outcome = discovery.asset, discoveryID, outcome
    discovery.revival = options.revival
  elseif outcome == "SELL" then
    plan.gold = RULES.SELL_GOLD
  elseif outcome == "RESEARCH" or outcome == "HERESY" or outcome == "ARTIFACT" then
    check(p:GetCities():GetCapitalCity() ~= nil, "yield grant requires a city")
    plan.modifier = "JAC_RP_" .. outcome .. "_" .. id
    check(GameInfo.Modifiers[plan.modifier] ~= nil, "missing yield modifier")
    if outcome == "ARTIFACT" then
      check(state.artifacts[tostring(discovery.player)] == nil, "PoC Vault slot occupied")
    elseif outcome == "RESEARCH" then
      plan.cultureModifier = "JAC_RP_RESEARCH_CULTURE_" .. id
      check(GameInfo.Modifiers[plan.cultureModifier] ~= nil, "missing research culture modifier")
      plan.boosts = researchBoosts(p)
    elseif outcome == "HERESY" then
      plan.faithCost = RULES.HERESY_FAITH_COST
    end
  elseif outcome == "ABSORB" then
    check(options.targetReligion ~= id, "absorption requires another religion")
    local target = get(state, options.targetReligion)
    check(not target.lost, "absorption target must have followers")
    check(target.assets[discovery.asset] == nil and target.materialUses[discovery.asset] == nil
      and not target.retiredAssets[discovery.asset], "target material ID occupied")
    plan.targetReligion, plan.faithCost = options.targetReligion, RULES.ABSORB_FAITH_COST
    plan.conversions, plan.convertedFollowers, plan.sourceFollowers, plan.nativeTarget = absorptionPlan(religion, target, p)
    plan.nativeSource = religion.engineReligion
  elseif outcome == "DIPLOMATIC_INCIDENT" then
    local target = player(options.targetPlayer)
    check(options.targetPlayer == religion.founder and options.targetPlayer ~= discovery.player,
      "restitution requires the living foreign founder")
    check(p:GetDiplomacy():HasMet(options.targetPlayer)
      and not p:GetDiplomacy():IsAtWarWith(options.targetPlayer), "restitution requires contact and peace")
    plan.targetPlayer = target:GetID()
    plan.gold = finite(target:GetTreasury():GetGoldBalance(), "origin gold") / 2
    -- Favor is integral in the engine. Leave the odd remainder with its owner.
    plan.favor = math.floor(finite(target:GetDiplomacy():GetFavor(), "origin favor") / 2)
    plan.alliance = tostring(plan.player) .. ":" .. tostring(plan.targetPlayer) .. ":" .. id
  elseif outcome == "HERO" then
    count(options.city, "hero city")
    check(p:GetCities():FindID(options.city) ~= nil, "hero requires an owned city")
    check(state.heroes[tostring(discovery.player)] == nil, "PoC hero already recruited")
    local hero, modifier = heroPlan(p, p:GetCities():FindID(options.city))
    plan.city, plan.heroClass, plan.heroUnit, plan.heroModifier = options.city, hero.Index, hero.UnitType, modifier
    plan.faith = finite(p:GetReligion():GetFaithYield(), "per-turn faith") * RULES.HERO_FAITH_MULTIPLIER
  end
  check(p:GetReligion():GetFaithBalance() >= plan.faithCost, "insufficient faith for choice")
  -- CONCEAL deliberately has zero yield; preserving a source is its benefit.
  if outcome ~= "REVIVE" then religion.outcomeUses[useKey] = discoveryID end
  reserveMaterial(religion, discovery.asset, outcome, discovery.player, discoveryID, plan.revival)
  discovery.effects = "PENDING"
  discovery.outcome, discovery.resolvedTurn = outcome, Game.GetCurrentGameTurn()
  discovery.effect = copy(plan)
  if outcome == "HERO" then state.heroes[tostring(plan.player)] = { religion = id, discovery = discoveryID } end
  if outcome == "ARTIFACT" then state.artifacts[tostring(plan.player)] = { religion = id, asset = plan.asset } end
  return state, plan
end

local function perform(state, id, plan)
  -- Reserve before irreversible engine calls. A failure leaves PENDING, never
  -- APPLIED or a replayable reward. Game properties + yields are not atomic.
  commit(state)
  local ok, result = pcall(function()
    local p = player(plan.player)
    if plan.faithCost > 0 and not plan.conversions then
      p:GetReligion():ChangeFaithBalance(-plan.faithCost)
    end
    if plan.outcome == "DIPLOMATIC_INCIDENT" then
      local origin = player(plan.targetPlayer)
      origin:GetTreasury():ChangeGoldBalance(-plan.gold)
      origin:GetDiplomacy():ChangeFavor(-plan.favor)
      -- Official: VikingScenario.lua:18-20,77. Never attempt SetHasAllied(false).
      if not p:GetDiplomacy():HasAllied(plan.targetPlayer) then
        p:GetDiplomacy():SetHasAllied(plan.targetPlayer, true)
      end
      check(p:GetDiplomacy():HasAllied(plan.targetPlayer), "religious alliance was not formed")
    end
    if plan.gold then p:GetTreasury():ChangeGoldBalance(plan.gold) end
    if plan.favor then p:GetDiplomacy():ChangeFavor(plan.favor) end -- verified gameplay API
    if plan.boosts then
      for _, tech in ipairs(plan.boosts) do
        p:GetTechs():TriggerBoost(tech, 1) -- AustraliaScenario.lua:784, as a goody hut
        check(p:GetTechs():HasBoostBeenTriggered(tech), "Eureka did not trigger")
      end
    end
    -- Boost before the 10 science: that grant could otherwise finish one of the
    -- chosen technologies and make its previously valid Eureka ineligible.
    if plan.modifier then p:AttachModifierByID(plan.modifier) end
    if plan.cultureModifier then p:AttachModifierByID(plan.cultureModifier) end
    if plan.conversions then
      for _, entry in ipairs(plan.conversions) do convertCitizens(entry) end
      local expected = plan.nativeRevival and RULES.REVIVAL_FOLLOWERS
        or plan.sourceFollowers - plan.convertedFollowers
      check(RP.NativeFollowers(id) == expected,
        "world conversion count mismatch; effect pending")
      -- Unsupported pressure mutation must not consume the absorption fee.
      p:GetReligion():ChangeFaithBalance(-plan.faithCost)
    end
    if plan.outcome == "HERO" then
      local city, before = p:GetCities():FindID(plan.city), {}
      for _, u in p:GetUnits():Members() do before[u:GetID()] = true end
      -- Official project completion uses THIS city modifier, not a normal Unit
      -- spawn: Babylon_Heroes_MODE.xml:620-631,895,903-914,985-1008.
      city:AttachModifierByID(plan.heroModifier) -- BlackDeathScenario.lua:296-297
      local created
      for _, u in p:GetUnits():Members() do
        if not before[u:GetID()] and u:GetType() == GameInfo.Units[plan.heroUnit].Index then
          check(created == nil, "multiple hero units created")
          created = u:GetID()
        end
      end
      check(created ~= nil, "official hero not created; effect pending")
      return created
    end
  end)
  check(ok, "engine effect pending; inspect before reconciliation: " .. tostring(result))
  state = read() -- preserve any gameplay callback's unrelated ledger changes
  local religion = get(state, id)
  local use = religion.materialUses[plan.asset]
  check(use ~= nil and use.effects == "PENDING", "effect reservation changed")
  if plan.outcome == "HERO" then
    local h = state.heroes[tostring(plan.player)]
    h.unit, h.heroClass, h.heroUnit = result, plan.heroClass, plan.heroUnit
    h.rewardFaith, h.verification = plan.faith, "PENDING"
    commit(state)
    ExposedMembers.JAC_RP_HeroUI.Schedule(plan.player)
    return -- UI's next frame verifies claim, class and lifespan before payment.
  end
  use.effects = "APPLIED"
  local asset = religion.assets[plan.asset]
  if plan.revival then
    local v = religion.revivals[plan.revival]
    -- A gameplay callback during the faith charge may have occupied the planned
    -- slot. Preserve that record too, and save the actual ID in the receipt.
    v.community = freeCommunityID(religion, v.community)
    religion.communities[v.community] = community(v.community, RULES.REVIVAL_FOLLOWERS, 0, v.holder)
    religion.revivalCount, v.effects = religion.revivalCount + 1, "APPLIED"
    asset.holder = "PLAYER_" .. plan.player -- retained, reusable after another loss and cooldown
  elseif plan.outcome == "SELL" then
    asset.holder, asset.owner = "MARKET", nil
  elseif plan.outcome == "CONCEAL" then
    asset.holder = "HIDDEN_PLAYER_" .. plan.player
  elseif plan.outcome == "RESEARCH" then
    religion.assets[plan.asset], religion.retiredAssets[plan.asset] = nil, true
  elseif plan.outcome == "HERESY" then
    religion.assets[plan.asset], religion.retiredAssets[plan.asset] = nil, true
    use.interpretation = "HERETICAL"
  elseif plan.outcome == "ABSORB" then
    local target = get(state, plan.targetReligion)
    local transferred = copy(asset)
    transferred.holder, transferred.originReligion = "PLAYER_" .. plan.player, id
    target.assets[plan.asset], target.materialUses[plan.asset] = transferred, copy(use)
    religion.assets[plan.asset] = nil
    religion.retiredAssets[plan.asset] = true
  elseif plan.outcome == "ARTIFACT" then
    asset.holder, asset.vault = "VAULT_PLAYER_" .. plan.player, true
  elseif plan.outcome == "DIPLOMATIC_INCIDENT" then
    asset.holder, asset.owner = "PLAYER_" .. plan.targetPlayer, plan.targetPlayer
    state.religiousAlliances[plan.alliance] = { player = plan.player, origin = plan.targetPlayer,
      religion = id, active = true, startedTurn = Game.GetCurrentGameTurn(), discovery = plan.discovery }
  end
  if plan.discovery then religion.discoveries[plan.discovery].effects = "APPLIED" end
  commit(state)
end

function RP.Revive(id, revivalID, sourceID, communityID, playerID, cityID)
  local state = read()
  for _, discovery in pairs(get(state, id).discoveries) do
    check(discovery.asset ~= sourceID or discovery.outcome ~= nil,
      "unresolved discovery must use its revival choice")
  end
  local plan = revivalPlan(state, id, revivalID, sourceID, communityID, playerID, cityID)
  plan.asset, plan.outcome = sourceID, "REVIVE"
  reserveMaterial(get(state, id), sourceID, "REVIVE", playerID, nil, revivalID)
  perform(state, id, plan)
end

function RP.ResolveDiscovery(id, discoveryID, outcome, options)
  local state, plan = discoveryPlan(id, discoveryID, outcome, options)
  perform(state, id, plan)
end

local heroObservations = {} -- diagnostic only; never saved or used for decisions
function RP.CompleteHero(playerID)
  local state = read()
  local h = state.heroes[tostring(playerID)]
  check(h ~= nil and h.verification == "PENDING", "hero not awaiting verification")
  local info = ExposedMembers.JAC_RP_HeroUI.UnitInfo(playerID, h.unit)
  local testFlag = GameConfiguration.GetValue("JAC_AUTOTEST")
  if tonumber(testFlag) == 1 then
    local observed = "unit=" .. h.unit .. " expectedClass=" .. h.heroClass .. " actual=" .. encode(info)
    if heroObservations[playerID] ~= observed then
      heroObservations[playerID] = observed
      print("[JAC_RP] hero verification " .. observed)
    end
  end
  if info == nil then return false end -- UI has not received the new unit yet.
  if not info.active then return false end -- pay only during the recipient's turn.
  if not (info.hero and info.class == h.heroClass and type(info.lifespan) == "number"
    and info.lifespan > 0 and info.claim == playerID) then
    return false -- independently replicated UI fields may arrive in different frames.
  end
  local p = player(playerID)
  local unit = p:GetUnits():FindID(h.unit)
  check(unit ~= nil and unit:GetType() == GameInfo.Units[h.heroUnit].Index, "native hero unit changed")
  local religion = get(state, h.religion)
  local d = religion.discoveries[h.discovery]
  check(d ~= nil and d.outcome == "HERO" and d.effects == "PENDING", "hero receipt changed")
  local use = religion.materialUses[d.asset]
  check(use ~= nil and use.effects == "PENDING", "hero material reservation changed")
  h.verification = "APPLYING"
  commit(state) -- a interrupted reward cannot replay on the next frame/load.
  p:GetReligion():ChangeFaithBalance(h.rewardFaith)
  state = read()
  h = state.heroes[tostring(playerID)]
  religion = get(state, h.religion)
  d = religion.discoveries[h.discovery]
  religion.materialUses[d.asset].effects, d.effects, h.verification = "APPLIED", "APPLIED", "APPLIED"
  religion.assets[d.asset].holder = "HERO_PLAYER_" .. playerID
  commit(state)
  return true
end

-- All jobs have a preferred branch, then use the first legal fallback. No RNG,
-- UI-only APIs, automatic discovery, or load-time effect replay.
local AI_PREFERENCES = { MONK = "REVIVE", SCHOLAR = "RESEARCH", THIEF = "SELL",
  PERFORMER = "HERESY", EXPLORER = "ARTIFACT", KABUKIMONO = "HERO", TRAVELER = "DIPLOMATIC_INCIDENT" }
function RP.ChooseOutcome(id, discoveryID, options)
  local d = get(read(), id).discoveries[discoveryID]
  check(d ~= nil, "discovery absent")
  for _, outcome in ipairs({ AI_PREFERENCES[d.job], "ABSORB", "SELL", "RESEARCH",
    "HERESY", "ARTIFACT", "DIPLOMATIC_INCIDENT", "HERO", "REVIVE", "CONCEAL" }) do
    if pcall(discoveryPlan, id, discoveryID, outcome, options) then return outcome end
  end
  error("[JAC_RP] no legal discovery choice")
end

function RP.ResolveForAI(id, discoveryID, options)
  local d = get(read(), id).discoveries[discoveryID]
  check(d ~= nil and not player(d.player):IsHuman(), "requires an AI discovery")
  local outcome = RP.ChooseOutcome(id, discoveryID, options)
  RP.ResolveDiscovery(id, discoveryID, outcome, options)
  return outcome
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

-- The turn hook ONLY expires alliance restrictions; never replays effects.
-- Official gameplay turn event: BlackDeathScenario.lua:64.
if not JAC_RP_AdapterInclude then
  GameEvents.OnGameTurnStarted.Add(RP.RefreshReligiousAlliances)
  -- Read fresh on every call: old saves cannot inherit previous Lua locals.
  ExposedMembers.JAC_ReligionPersistence = RP
end
-- An includer in the same Gameplay context can register adapters directly.
JAC_ReligionPersistenceGameplay = RP
local loaded = read()
print("[JAC_RP] loaded schema=" .. loaded.schema .. " revision=" .. loaded.revision
  .. " religions=" .. #keys(loaded.religions))
