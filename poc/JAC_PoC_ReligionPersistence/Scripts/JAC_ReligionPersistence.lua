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

local RULES = {}
for row in GameInfo.JAC_RP_Rules() do RULES[row.ID] = row.Value end
local HERO_UNIT = "UNIT_SCOUT" -- placeholder person, no D8 religion/persona design
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
    and (use == nil or use.outcome == "CONCEAL" and use.effects == "APPLIED")
end

local function derive(religion)
  local followers, facilities, source = 0, 0, false
  for _, c in pairs(religion.communities) do
    followers = followers + c.followers
    facilities = facilities + c.publicFacilities
  end
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
  if state == nil then return { schema = SCHEMA, revision = 0, religions = {}, heroes = {}, artifacts = {} } end
  check(type(state) == "table" and state.schema == SCHEMA,
    "unsupported or damaged save schema; refusing to replace it")
  check(type(state.religions) == "table", "damaged religion ledger")
  state = copy(state)
  -- Additive V1 extension, read-only until a real mutation. Old DEFERRED choices
  -- remain history; loading a save never pays a previously undecided reward.
  state.heroes, state.artifacts = state.heroes or {}, state.artifacts or {}
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
  communityID = freeCommunityID(religion, communityID)
  religion.revivals[revivalID] = { source = sourceID, community = communityID,
    followers = RULES.REVIVAL_FOLLOWERS, publicFacilities = 0, holder = "PLAYER_" .. playerID,
    player = playerID, city = cityID, faithCost = RULES.REVIVAL_FAITH_COST,
    effects = "PENDING", turn = Game.GetCurrentGameTurn() }
  return { player = playerID, faithCost = RULES.REVIVAL_FAITH_COST, revival = revivalID }
end

local function reserveMaterial(religion, assetID, outcome, playerID, discoveryID, revivalID)
  religion.materialUses[assetID] = { outcome = outcome, player = playerID,
    discovery = discoveryID, revival = revivalID, effects = "PENDING", turn = Game.GetCurrentGameTurn() }
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
    end
  elseif outcome == "ABSORB" then
    check(options.targetReligion ~= id, "absorption requires another religion")
    local target = get(state, options.targetReligion)
    check(not target.lost, "absorption target must have followers")
    local owned = false
    for _, c in pairs(target.communities) do
      owned = owned or c.holder == "PLAYER_" .. discovery.player and c.followers > 0
    end
    check(owned, "target religion needs own followers")
    check(target.assets[discovery.asset] == nil and target.materialUses[discovery.asset] == nil
      and not target.retiredAssets[discovery.asset], "target material ID occupied")
    plan.targetReligion, plan.faith = options.targetReligion, RULES.ABSORB_FAITH
  elseif outcome == "DIPLOMATIC_INCIDENT" then
    local target = player(options.targetPlayer)
    check(options.targetPlayer == religion.founder and options.targetPlayer ~= discovery.player,
      "restitution requires the living foreign founder")
    check(p:GetDiplomacy():HasMet(options.targetPlayer)
      and not p:GetDiplomacy():IsAtWarWith(options.targetPlayer), "restitution requires contact and peace")
    plan.targetPlayer, plan.favor = target:GetID(), RULES.INCIDENT_FAVOR
  elseif outcome == "HERO" then
    count(options.city, "hero city")
    check(p:GetCities():FindID(options.city) ~= nil, "hero requires an owned city")
    check(state.heroes[tostring(discovery.player)] == nil, "PoC hero already recruited")
    check(p:GetReligion():GetFaithBalance() >= RULES.HERO_FAITH_COST, "insufficient hero faith")
    check(GameInfo.Units[HERO_UNIT] ~= nil, "missing placeholder hero unit")
    plan.city, plan.faithCost = options.city, RULES.HERO_FAITH_COST
  end
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
    if plan.faithCost > 0 and plan.outcome ~= "HERO" then
      p:GetReligion():ChangeFaithBalance(-plan.faithCost)
    end
    if plan.gold then p:GetTreasury():ChangeGoldBalance(plan.gold) end
    if plan.faith then p:GetReligion():ChangeFaithBalance(plan.faith) end
    if plan.favor then p:GetDiplomacy():ChangeFavor(plan.favor) end -- verified gameplay API
    if plan.modifier then p:AttachModifierByID(plan.modifier) end
    if plan.outcome == "HERO" then
      local city, before = p:GetCities():FindID(plan.city), {}
      for _, u in p:GetUnits():Members() do before[u:GetID()] = true end
      -- This engine call returns NO Unit. Official call: AlexanderScenario.lua:194.
      UnitManager.InitUnitValidAdjacentHex(plan.player, HERO_UNIT, city:GetX(), city:GetY(), 1)
      local created
      for _, u in p:GetUnits():Members() do
        if not before[u:GetID()] and u:GetType() == GameInfo.Units[HERO_UNIT].Index then
          check(created == nil, "multiple hero units created")
          created = u:GetID()
        end
      end
      check(created ~= nil, "no valid hex for hero; effect pending")
      p:GetReligion():ChangeFaithBalance(-plan.faithCost)
      return created
    end
  end)
  check(ok, "engine effect pending; inspect before reconciliation: " .. tostring(result))
  state = read() -- preserve any gameplay callback's unrelated ledger changes
  local religion = get(state, id)
  local use = religion.materialUses[plan.asset]
  check(use ~= nil and use.effects == "PENDING", "effect reservation changed")
  use.effects = "APPLIED"
  local asset = religion.assets[plan.asset]
  if plan.revival then
    local v = religion.revivals[plan.revival]
    -- A gameplay callback during the faith charge may have occupied the planned
    -- slot. Preserve that record too, and save the actual ID in the receipt.
    v.community = freeCommunityID(religion, v.community)
    religion.communities[v.community] = community(v.community, RULES.REVIVAL_FOLLOWERS, 0, v.holder)
    religion.revivalCount, v.effects = religion.revivalCount + 1, "APPLIED"
    asset.holder = "MEMORIAL_PLAYER_" .. plan.player
  elseif plan.outcome == "SELL" then
    asset.holder, asset.owner = "MARKET", nil
  elseif plan.outcome == "CONCEAL" then
    asset.holder = "HIDDEN_PLAYER_" .. plan.player
  elseif plan.outcome == "RESEARCH" then
    asset.holder = "ARCHIVE_PLAYER_" .. plan.player
  elseif plan.outcome == "HERESY" then
    asset.holder, asset.interpretation = "COMMENTARY_PLAYER_" .. plan.player, "HERETICAL"
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
  elseif plan.outcome == "HERO" then
    asset.holder = "HERO_PLAYER_" .. plan.player
    state.heroes[tostring(plan.player)].unit = result
  end
  if plan.discovery then religion.discoveries[plan.discovery].effects = "APPLIED" end
  commit(state)
end

function RP.Revive(id, revivalID, sourceID, communityID, playerID, cityID)
  local state = read()
  local plan = revivalPlan(state, id, revivalID, sourceID, communityID, playerID, cityID)
  plan.asset, plan.outcome = sourceID, "REVIVE"
  reserveMaterial(get(state, id), sourceID, "REVIVE", playerID, nil, revivalID)
  perform(state, id, plan)
end

function RP.ResolveDiscovery(id, discoveryID, outcome, options)
  local state, plan = discoveryPlan(id, discoveryID, outcome, options)
  perform(state, id, plan)
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

-- No turn/load hook recreates or normalizes saved data. Read fresh on every call:
-- an old save in the same process cannot inherit the previous game's Lua locals.
ExposedMembers.JAC_ReligionPersistence = RP
local loaded = read()
print("[JAC_RP] loaded schema=" .. loaded.schema .. " revision=" .. loaded.revision
  .. " religions=" .. #keys(loaded.religions))
