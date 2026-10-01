include("JAC_Absorption_Rules")
local R = JAC_AbsorptionRules
local A = { Rules = R }
local unitKeys = { [R.PERSON_KEY] = true, JAC_POC_GROWTHUI_WARLORD = true }
local cityKeys = { [R.CITY_FAITH_KEY] = true }
local unitRebinders, cityRebinders = {}, {}
local adapters = {}

local function Log(text) print("[JAC_ABSORPTION] " .. text) end
local function State()
  local state = R.Copy(Game:GetProperty(R.STATE_KEY) or { Version = 1, Transactions = {}, Archive = {}, Recalls = {} })
  assert(state.Version == 1, "unsupported absorption save version")
  state.Recalls = state.Recalls or {}
  return state
end
local function Save(state) Game:SetProperty(R.STATE_KEY, R.Copy(state)) end
local function Units(owner)
  local list = {}
  for _, unit in Players[owner]:GetUnits():Members() do list[#list + 1] = unit end
  table.sort(list, function(a, b) return a:GetID() < b:GetID() end)
  return list
end
local function Properties(object, keys)
  local result = {}
  for _, key in ipairs(R.Keys(keys)) do result[key] = R.Copy(object:GetProperty(key)) end
  return result
end
local function WriteProperties(object, properties, keys)
  for _, key in ipairs(R.Keys(keys or properties)) do object:SetProperty(key, R.Copy(properties[key])) end
end
local function GetRoster(owner) return R.Copy(Players[owner]:GetProperty(R.ROSTER_KEY) or {}) end
A.GetRoster = GetRoster
A.Canonical = R.Canonical
A.GetState = State
A.GetUIPhase = function() return R.Copy(Game:GetProperty("JAC_ABSORPTION_FIXTURE_UI_WAIT")) end
local function CityRead(city, method, ...)
  local reads = ExposedMembers.JAC_HeroReadAPI
  assert(reads and reads.ReadCity, "UI city read API unavailable")
  return reads.ReadCity(city:GetOwner(), city:GetID(), method, ...)
end
local function WorkSlots(city, building)
  local b = city:GetBuildings()
  if b.GetNumGreatWorkSlots then return b:GetNumGreatWorkSlots(building) end
  return CityRead(city, "SLOTS", building)
end
local function WorkInSlot(city, building, slot)
  local b = city:GetBuildings()
  if b.GetGreatWorkInSlot then return b:GetGreatWorkInSlot(building, slot) end
  return CityRead(city, "WORK", building, slot)
end
local function RegisterProperty(keys, rebinders, key, rebind)
  assert(type(key) == "string" and key ~= "", "property key required")
  assert(rebind == nil or type(rebind) == "function", "property rebind must be a pure function")
  keys[key], rebinders[key] = true, rebind
end
function A.RegisterUnitProperty(key, rebind)
  assert(key ~= R.PERSON_KEY or rebind == nil, "person references have a built-in rebind")
  RegisterProperty(unitKeys, unitRebinders, key, rebind)
end
function A.RegisterCityProperty(key, rebind) RegisterProperty(cityKeys, cityRebinders, key, rebind) end
local function RebindKeys(rebinders)
  local result = {}
  for key in pairs(rebinders) do result[key] = true end
  return result
end

-- Prepare is read-only; Apply must be idempotent; Verify must raise on a mismatch.
-- Only Prepare's serializable result is saved. Re-register callbacks on every game load.
function A.RegisterAdapter(name, adapter)
  assert(type(name) == "string" and name ~= "" and not adapters[name], "invalid/duplicate adapter")
  assert(type(adapter.Prepare) == "function" and type(adapter.Apply) == "function"
    and type(adapter.Verify) == "function", "adapter needs Prepare/Apply/Verify")
  adapters[name] = adapter
end

local function Religion(city)
  local result = {}
  local religion = city:GetReligion()
  local rows = religion.GetReligionsInCity and religion:GetReligionsInCity() or CityRead(city, "RELIGIONS")
  for _, row in pairs(rows) do
    result[tostring(row.Religion)] = row.Followers
  end
  return result
end
A.CityReligion = Religion

local function CityAssets(city)
  local result, buildings = {}, city:GetBuildings()
  for info in GameInfo.Buildings() do
    -- The engine controls capital palaces; historical belongings are separate roster/Great Work records.
    if info.BuildingType ~= "BUILDING_PALACE" and buildings:HasBuilding(info.Index) then
      local works = {}
      for slot = 0, WorkSlots(city, info.Index) - 1 do
        works[slot] = WorkInSlot(city, info.Index, slot)
      end
      result[info.BuildingType] = { Pillaged = buildings:IsPillaged(info.Index), Works = works }
    end
  end
  return result
end
A.CityAssets = CityAssets

-- Palace buildings are engine-controlled, but their works are still historical assets.
local function PalaceWorks(city)
  local result, buildings = {}, city:GetBuildings()
  local palace = GameInfo.Buildings["BUILDING_PALACE"]
  if palace and buildings:HasBuilding(palace.Index) then
    for slot = 0, WorkSlots(city, palace.Index) - 1 do
      local work = WorkInSlot(city, palace.Index, slot)
      if work and work >= 0 then result[#result + 1] = work end
    end
  end
  table.sort(result)
  return result
end
A.PalaceWorks = PalaceWorks

local function OwnedGreatWorks(owner)
  local works = {}
  for _, city in Players[owner]:GetCities():Members() do
    local buildings = city:GetBuildings()
    for info in GameInfo.Buildings() do
      if buildings:HasBuilding(info.Index) then
        for slot = 0, WorkSlots(city, info.Index) - 1 do
          local work = WorkInSlot(city, info.Index, slot)
          if work and work >= 0 then works[work] = true end
        end
      end
    end
  end
  return works
end
A.OwnedGreatWorks = OwnedGreatWorks

-- The installed engine separates game mutations from UI-only manager getters.
-- UI closures keep their own engine context; they return only scalars/component IDs.
local function NativeHeroesManager()
  local native = Game.GetHeroesManager and Game.GetHeroesManager()
  if not native then return nil end
  local manager = {}
  for _, name in ipairs({ "CreateHero", "SetHeroDiscovered", "IsHeroDiscovered",
    "GetHeroClaimPlayer", "GetHeroOriginCityID", "GetPlayerHeroOriginBuildingType" }) do
    local method = native[name]
    local read = ExposedMembers.JAC_HeroReadAPI and ExposedMembers.JAC_HeroReadAPI[name]
    if method then
      manager[name] = function(_, ...) return method(native, ...) end
    elseif read then
      manager[name] = function(_, ...) return read(...) end
    end
  end
  return manager
end
A.GetHeroesManager = NativeHeroesManager

local function Heroes()
  local manager = NativeHeroesManager()
  assert(manager and manager.CreateHero and manager.GetHeroClaimPlayer and manager.GetHeroOriginCityID
    and manager.IsHeroDiscovered and manager.SetHeroDiscovered and manager.GetPlayerHeroOriginBuildingType,
    "native Hero requires gameplay HeroesManager (enable Heroes & Legends)")
  return manager
end

local function HeroUnits(owner, class)
  local result = {}
  for _, unit in ipairs(Units(owner)) do
    if unit.IsHero and unit:IsHero() and unit:GetHeroClassType() == class then result[#result + 1] = unit end
  end
  return result
end

local function HeroOrigin(manager, class)
  local origin = manager:GetHeroOriginCityID(class)
  assert(origin and Players[origin.player] and type(origin.id) == "number", "native Hero origin ID missing")
  local city = Players[origin.player]:GetCities():FindID(origin.id)
  assert(city, "native Hero origin city missing; no supported origin setter")
  return { Owner = city:GetOwner(), ID = city:GetID(), X = city:GetX(), Y = city:GetY() }
end

local function SnapshotHero(unit, info)
  local native = unit.IsHero and unit:IsHero()
  assert(native or not string.match(info.UnitType, "^UNIT_HERO_"), "native Hero engine flag missing")
  if not native then return nil end
  assert(unit.GetHeroClassType, "native Hero class API missing")
  -- Shipped UnitPanel.lua:2269 uses this property. Debug/Heroes.ltp's Get/SetLifespan
  -- and Get/SetRenown are absent from the installed XP2 Lua registrations; do not require or guess them.
  local lifeKey = (UnitPropertyKeys and UnitPropertyKeys.LifespanRemaining) or ExposedMembers.JAC_HeroLifespanKey
  local class = unit:GetHeroClassType()
  local def = assert(GameInfo.HeroClasses and GameInfo.HeroClasses[class], "native Hero class missing")
  assert(def.UnitType == info.UnitType, "native Hero class/unit type mismatch")
  local manager, owner = NativeHeroesManager(), unit:GetOwner()
  local claim, origin, rawOrigin, discovered, building
  if manager then
    if manager.GetHeroClaimPlayer then
      local ok, value = pcall(manager.GetHeroClaimPlayer, manager, class)
      if ok then claim = value end
    end
    if manager.GetHeroOriginCityID then
      local rawOK, raw = pcall(manager.GetHeroOriginCityID, manager, class)
      if rawOK and raw and type(raw.player) == "number" and type(raw.id) == "number" then
        rawOrigin = { Owner = raw.player, ID = raw.id }
      end
      local ok, value = pcall(HeroOrigin, manager, class)
      if ok then origin = value end
    end
    if manager.IsHeroDiscovered then
      local ok, value = pcall(manager.IsHeroDiscovered, manager, owner, class)
      if ok then discovered = value end
    end
    if manager.GetPlayerHeroOriginBuildingType then
      local ok, value = pcall(manager.GetPlayerHeroOriginBuildingType, manager, owner)
      if ok then building = value end
    end
  end
  local lifeOK, life = false, nil
  if lifeKey then lifeOK, life = pcall(unit.GetProperty, unit, lifeKey) end
  local chargesOK, charges = false, nil
  if unit.GetActionCharges then chargesOK, charges = pcall(unit.GetActionCharges, unit) end
  if not lifeOK then life = nil end
  if not chargesOK then charges = nil end
  local renown, renownRestorable
  if unit.GetRenown then
    local ok, value = pcall(unit.GetRenown, unit)
    if ok then renown = value end
    renownRestorable = ok and unit.SetRenown ~= nil
  else
    renownRestorable = true
  end
  return { Class = class, ClassType = def.HeroClassType, Origin = origin, RawOrigin = rawOrigin,
    ClaimOwner = claim, Lifespan = life, LifespanKey = lifeKey, ActionCharges = charges,
    ChargesRestorable = unit.ChangeActionCharges ~= nil,
    Renown = renown, RenownUnavailable = renown == nil, RenownRestorable = renownRestorable,
    Discovered = discovered, OriginBuilding = building }
end
function A.CityPurchaseCost(city, yield, unitHash, formation)
  local gold = city.GetGold and city:GetGold()
  local kind = formation or (MilitaryFormationTypes and MilitaryFormationTypes.STANDARD_MILITARY_FORMATION)
  if gold and gold.GetPurchaseCost and kind ~= nil then return gold:GetPurchaseCost(yield, unitHash, kind) end
  Log("purchase cost via UI owner=" .. city:GetOwner() .. " coreGold=" .. tostring(city.GetGold ~= nil)
    .. " coreCost=" .. tostring(gold ~= nil and gold.GetPurchaseCost ~= nil))
  local cost = CityRead(city, "PURCHASE_COST", yield, unitHash, formation)
  assert(type(cost) == "number", "UI purchase cost must be a number")
  return cost
end
function A.PlayerFaithBalance(owner)
  local religion = Players[owner]:GetReligion()
  if religion.GetFaithBalance then return religion:GetFaithBalance() end
  local reads = ExposedMembers.JAC_HeroReadAPI
  assert(reads and reads.ReadPlayer, "UI player read API unavailable")
  local balance = reads.ReadPlayer(owner, "FAITH_BALANCE")
  assert(type(balance) == "number", "UI faith balance must be a number")
  return balance
end

local function SpreadCharges(unit)
  if unit.GetSpreadCharges then return unit:GetSpreadCharges() end
  local reads = ExposedMembers.JAC_HeroReadAPI
  assert(reads and reads.ReadUnit, "UI unit read API unavailable")
  local charges = reads.ReadUnit(unit:GetOwner(), unit:GetID(), "SPREAD_CHARGES", Game.GetCurrentGameTurn())
  if charges == nil then error("JAC_ABSORPTION_WAITING_UI", 0) end
  assert(type(charges) == "number", "UI spread charges must be a number")
  return charges
end

local function SourceSpreadReads(id, owner, turn, serial)
  local result = {}
  for _, unit in ipairs(Units(owner)) do
    if not unit.GetSpreadCharges then
      local unitID = unit:GetID()
      result[unitID] = { Token = id .. ":source-spread:" .. serial .. ":" .. unitID, Owner = owner,
        ID = unitID, Turn = turn, AwaitingUI = true }
    end
  end
  return result
end

local function SnapshotUnit(unit, publishedSpread)
  local info = assert(GameInfo.Units[unit:GetType()], "unknown unit type")
  local hero = SnapshotHero(unit, info)
  local greatPerson = unit:GetGreatPerson()
  local individual = greatPerson and greatPerson:GetIndividual() or -1
  local person = unit:GetProperty(R.PERSON_KEY)
  assert(not hero or not person or person.Kind == "HERO", "native Hero/person kind mismatch")
  local kind = hero and "HERO" or person and person.Kind or (individual >= 0 and "GREAT_PERSON" or "UNIT")
  local experience = unit:GetExperience()
  local promotions = {}
  for row in GameInfo.UnitPromotions() do
    if experience and experience:HasPromotion(row.Index) then promotions[#promotions + 1] = row.Index end
  end
  table.sort(promotions)
  local religious = unit:GetReligion()
  local religion = religious and religious:GetReligionType() or -1
  return {
    ID = unit:GetID(), Type = info.UnitType, X = unit:GetX(), Y = unit:GetY(), Kind = kind,
    Individual = individual, Hero = hero, Damage = unit:GetDamage(), Moves = unit:GetMovesRemaining(),
    GreatPersonCharges = greatPerson and greatPerson.GetActionCharges and greatPerson:GetActionCharges() or nil,
    XP = experience and experience:GetExperiencePoints() or nil, Promotions = promotions, Formation = unit:GetMilitaryFormation(),
    BuildCharges = unit:GetBuildCharges(), SpreadCharges = publishedSpread ~= nil and publishedSpread or SpreadCharges(unit), Religion = religion,
    Properties = Properties(unit, unitKeys), PropertyKeys = R.Copy(unitKeys), RebindKeys = RebindKeys(unitRebinders),
    Name = unit:GetName(),
  }
end
A.SnapshotUnit = SnapshotUnit

local function DestinationOwner(kind, outcome, decision, tx)
  local destination = R.Destination(kind, outcome)
  if destination == "ABSORBER" then return tx.To end
  if destination == "RETURN" then
    local id = decision.ReturnOwner
    assert(type(id) == "number" and id ~= tx.From and id ~= tx.To and Players[id]
      and Players[id]:IsAlive(), "RETURN requires a surviving third-party ReturnOwner")
    return id
  end
  return -1
end

local function Decision(value)
  if type(value) == "string" then return { Outcome = value } end
  assert(type(value) == "table" and type(value.Outcome) == "string", "explicit disposition required")
  return R.Copy(value)
end

local function HeroSuicide(snap, cause)
  assert(snap.Hero and not snap.DispositionFrozen, "native Hero disposition is frozen")
  snap.RequestedOutcome = snap.RequestedOutcome or snap.Outcome
  snap.Outcome, snap.Destination, snap.KeepUnit = "SUICIDE", -1, false
  snap.SuicideCause = cause
  local manager = NativeHeroesManager()
  if manager and manager.GetHeroClaimPlayer then
    local ok, owner = pcall(manager.GetHeroClaimPlayer, manager, snap.Hero.Class)
    if ok then snap.NativeClaimAfter = owner end
  end
end

local function HeroTransferProblem(tx, snap, origins)
  local hero, manager = snap.Hero, NativeHeroesManager()
  if not manager or not manager.CreateHero or not manager.SetHeroDiscovered then return "NATIVE_MANAGER_UNAVAILABLE" end
  if hero.ClaimOwner ~= tx.From then return "CLAIM_UNAVAILABLE" end
  if not hero.Origin or hero.Origin.Owner ~= tx.From then return "ORIGIN_UNAVAILABLE" end
  if not hero.OriginBuilding then return "ORIGIN_BUILDING_UNAVAILABLE" end
  if type(hero.Lifespan) ~= "number" or hero.Lifespan <= 0
    or type(hero.ActionCharges) ~= "number" or hero.ActionCharges < 0
    or not hero.ChargesRestorable then return "NATIVE_STATE_UNAVAILABLE" end
  if not hero.RenownRestorable or hero.Renown ~= nil and type(hero.Renown) ~= "number" then
    return "RENOWN_UNRESTORABLE"
  end
  if not UnitManager.PlaceUnit then return "HERO_PLACEMENT_UNAVAILABLE" end
  if #HeroUnits(tx.From, hero.Class) ~= 1 or #HeroUnits(tx.To, hero.Class) ~= 0 then
    return "HERO_CLASS_COLLISION"
  end
  local key = tostring(hero.Origin.X) .. ":" .. tostring(hero.Origin.Y)
  if origins[key] then return "HERO_ORIGIN_COLLISION" end
  local ok, building = pcall(manager.GetPlayerHeroOriginBuildingType, manager, tx.To)
  if not ok or building ~= hero.OriginBuilding then return "ORIGIN_BUILDING_UNAVAILABLE" end
  local city = CityManager.GetCityAt(hero.Origin.X, hero.Origin.Y)
  if not city or city:GetOwner() ~= tx.From or not city:GetBuildings():HasBuilding(building) then
    return "ORIGIN_BUILDING_UNAVAILABLE"
  end
  origins[key] = true
  return nil
end

local function DormantHero(manager, def, owner)
  local hero = { Class = def.Index, ClassType = def.HeroClassType, ClaimOwner = owner }
  if manager.IsHeroDiscovered then
    local ok, value = pcall(manager.IsHeroDiscovered, manager, owner, def.Index)
    if ok then hero.Discovered = value end
  end
  if manager.GetPlayerHeroOriginBuildingType then
    local ok, value = pcall(manager.GetPlayerHeroOriginBuildingType, manager, owner)
    if ok then hero.OriginBuilding = value end
  end
  if manager.GetHeroOriginCityID then
    local ok, value = pcall(manager.GetHeroOriginCityID, manager, def.Index)
    if ok and value and type(value.player) == "number" and type(value.id) == "number" then
      hero.Origin = { Owner = value.player, ID = value.id }
      local player = Players[value.player]
      local city = player and player:GetCities():FindID(value.id)
      if city then hero.Origin.X, hero.Origin.Y = city:GetX(), city:GetY() end
    end
  end
  return hero
end

local function CityNeedsUI(city)
  local religion, buildings = city:GetReligion(), city:GetBuildings()
  return not religion.GetReligionsInCity or not buildings.GetNumGreatWorkSlots or not buildings.GetGreatWorkInSlot
end

local function Prepare(plan, state, sourceReads, sourceReadyToken)
  assert(type(plan) == "table" and type(plan.ID) == "string" and plan.ID ~= "", "transaction ID required")
  assert(type(plan.From) == "number" and type(plan.To) == "number" and plan.From ~= plan.To, "distinct factions required")
  local source, target = Players[plan.From], Players[plan.To]
  assert(source and target and source:IsAlive() and target:IsAlive(), "two living factions required")
  assert(not source:IsHuman(), "PoC only absorbs AI factions")
  assert(R.ENTRY_MODES[plan.Mode], "coexistence declarations are not absorption")
  assert(plan.EntryConditionsMet == true and plan.RelationsResolved == true,
    "entry conditions and third-party relations must be resolved by caller")
  assert(source:GetCities():GetCount() > 0 and target:GetCities():GetCount() > 0, "both factions need cities")
  for _, previous in pairs(state.Transactions) do
    assert(previous.Status == "COMPLETE" or
      (previous.From ~= plan.From and previous.To ~= plan.From and previous.From ~= plan.To and previous.To ~= plan.To),
      "faction already has an unfinished absorption")
    assert(previous.Status ~= "COMPLETE" or previous.From ~= plan.From, "faction was already absorbed")
  end
  for _, city in source:GetCities():Members() do
    if CityNeedsUI(city) then
      local reads = ExposedMembers.JAC_HeroReadAPI
      if not sourceReadyToken or not reads or not reads.GetObservation
        or not reads.GetObservation(sourceReadyToken) then error("JAC_ABSORPTION_WAITING_UI", 0) end
      break
    end
  end
  local tx = { ID = plan.ID, From = plan.From, To = plan.To, Mode = plan.Mode,
    Turn = Game.GetCurrentGameTurn(), Request = R.Copy(plan), Status = "PREPARED", Units = {}, UnitMap = {}, Cities = {}, Plots = {},
    SourceRoster = GetRoster(plan.From), TargetRoster = GetRoster(plan.To), Decisions = {}, HeroResults = {}, Adapters = {} }
  local generatedDecisions, heroOrigins, dormant = {}, {}, {}
  tx.OriginalSourceRoster = R.Copy(tx.SourceRoster)
  for _, unit in ipairs(Units(plan.From)) do
    local publishedSpread
    if not unit.GetSpreadCharges then
      local request = sourceReads and sourceReads[unit:GetID()]
      local reads = ExposedMembers.JAC_HeroReadAPI
      if request and reads and reads.GetUnitObservation then publishedSpread = reads.GetUnitObservation(request.Token) end
      if publishedSpread == nil then error("JAC_ABSORPTION_WAITING_UI", 0) end
      assert(type(publishedSpread) == "number", "published donor spread charges must be a number")
    end
    local snap = SnapshotUnit(unit, publishedSpread)
    local decision = Decision((plan.Units or {})[R.UnitKey(plan.From, snap.ID)])
    snap.Outcome = decision.Outcome
    snap.Destination = DestinationOwner(snap.Kind, snap.Outcome, decision, tx)
    snap.RequestedDestination = snap.Destination
    snap.KeepUnit = R.KeepUnit(snap.Kind, snap.Outcome)
    if snap.KeepUnit and snap.Individual >= 0 then
      assert(type(snap.GreatPersonCharges) == "number" and snap.GreatPersonCharges >= 0,
        "native great-person charges unavailable; transfer requires a lossless snapshot")
    end
    snap.Token = plan.ID .. ":unit:" .. snap.ID
    tx.Units[#tx.Units + 1] = snap
    if snap.Hero then
      snap.NativeSnapshot = R.Copy(snap)
      if snap.KeepUnit then
        local problem = HeroTransferProblem(tx, snap, heroOrigins)
        if problem then HeroSuicide(snap, problem) end
      else
        HeroSuicide(snap, decision.Outcome == "SUICIDE" and "EXPLICIT_SUICIDE" or "REQUESTED_OUTCOME_UNTRANSFERABLE")
      end
    end
    if snap.Kind ~= "UNIT" then
      local linked = false
      for _, asset in pairs(tx.SourceRoster) do
        if asset.Unit and asset.Unit.Owner == tx.From and asset.Unit.ID == snap.ID then linked = true end
      end
      if not linked then
        local person = snap.Properties[R.PERSON_KEY]
        local id = person and person.ID or ("native-person:" .. tx.From .. ":" .. snap.ID)
        assert(not tx.SourceRoster[id], "native person ID collision")
        tx.SourceRoster[id] = person and R.Copy(person) or {
          ID = id, Kind = snap.Kind, Owner = tx.From, NativeIndividual = snap.Individual, UnitType = snap.Type }
        tx.SourceRoster[id].Unit = { Owner = tx.From, ID = snap.ID }
        generatedDecisions[id] = decision
      end
      if snap.Hero then
        for _, asset in pairs(tx.SourceRoster) do
          if asset.Unit and asset.Unit.Owner == tx.From and asset.Unit.ID == snap.ID then
            assert(asset.Kind == "HERO", "native Hero roster kind mismatch")
            if asset.NativeHero then asset.HistoricalNativeHero = R.Copy(asset.NativeHero) end
            asset.NativeHero = R.Copy(snap.Hero) -- historical engine snapshot, not a mutable lifespan mirror
          end
        end
      end
    end
  end
  -- Claimed heroes without a Unit have no supported claim-only transfer. Keep their
  -- available native state and any existing person record as explicit suicide history.
  if GameInfo.HeroClasses and Game.GetHeroesManager then
    local manager = NativeHeroesManager()
    if manager and manager.GetHeroClaimPlayer then
      for def in GameInfo.HeroClasses() do
        if manager:GetHeroClaimPlayer(def.Index) == tx.From then
          local found = false
          for _, snap in ipairs(tx.Units) do if snap.Hero and snap.Hero.Class == def.Index then found = true end end
          if not found then
            local id, native = nil, DormantHero(manager, def, tx.From)
            for key, asset in pairs(tx.SourceRoster) do
              if asset.Kind == "HERO" and ((asset.NativeHero
                and (asset.NativeHero.Class == def.Index or asset.NativeHero.ClassType == def.HeroClassType))
                or asset.UnitType == def.UnitType or asset.HeroClassType == def.HeroClassType) then
                assert(not id, "multiple person records for dormant native Hero")
                id = key
              end
            end
            id = id or ("native-hero:" .. tx.From .. ":" .. def.HeroClassType)
            assert(not tx.SourceRoster[id] or tx.SourceRoster[id].Kind == "HERO", "dormant Hero ID collision")
            tx.SourceRoster[id] = tx.SourceRoster[id] or { ID = id, Kind = "HERO", Owner = tx.From }
            if tx.SourceRoster[id].NativeHero then
              tx.SourceRoster[id].HistoricalNativeHero = R.Copy(tx.SourceRoster[id].NativeHero)
            end
            tx.SourceRoster[id].NativeHero = native
            dormant[id] = true
            generatedDecisions[id] = Decision((plan.Assets or {})[id] or "SUICIDE")
          end
        end
      end
    end
  end
  -- A leader survives as a persistent character with a general proxy; no native retire ability is assigned.
  local leaderID = "leader:" .. plan.From
  assert(not tx.SourceRoster[leaderID] and not tx.TargetRoster[leaderID], "leader ID collision")
  local capital = source:GetCities():GetCapitalCity()
  local leader = { ID = leaderID, Kind = "GREAT_PERSON", Owner = plan.From,
    LeaderType = PlayerConfigurations[plan.From]:GetLeaderTypeName(), Persistent = true,
    Representation = R.LEADER_REPRESENTATION_POC, Unit = { Owner = plan.From, ID = -1 } }
  tx.SourceRoster[leaderID] = leader
  local decision = Decision(plan.Leader)
  tx.Units[#tx.Units + 1] = {
    ID = -1, Type = R.LEADER_UNIT_TYPE_POC, X = capital:GetX(), Y = capital:GetY(), Kind = "GREAT_PERSON",
    Individual = -1, Properties = { [R.PERSON_KEY] = R.Copy(leader) }, PropertyKeys = R.Copy(unitKeys),
    RebindKeys = RebindKeys(unitRebinders), Outcome = decision.Outcome,
    Destination = DestinationOwner(leader.Kind, decision.Outcome, decision, tx), Token = plan.ID .. ":leader",
    KeepUnit = R.KeepUnit(leader.Kind, decision.Outcome),
  }
  for _, id in ipairs(R.Keys(tx.SourceRoster)) do
    local asset = tx.SourceRoster[id]
    assert(asset.ID == id and asset.Owner == plan.From, "invalid roster ownership/ID: " .. id)
    assert(not tx.TargetRoster[id] and not state.Archive[id], "asset ID collision: " .. id)
    local d = id == leaderID and decision or generatedDecisions[id] or Decision((plan.Assets or {})[id])
    d.Owner = DestinationOwner(asset.Kind, d.Outcome, d, tx)
    if d.Owner >= 0 and d.Owner ~= tx.To then assert(not GetRoster(d.Owner)[id], "returned asset ID collision") end
    if asset.Kind == "UNLOCK" and d.Outcome == "INHERIT" then
      if asset.Technology then assert(GameInfo.Technologies[asset.Technology], "unknown inherited technology") end
      if asset.Civic then assert(GameInfo.Civics[asset.Civic], "unknown inherited civic") end
    end
    if asset.Unit and asset.Unit.Owner == tx.From then
      local found = false
      for _, snap in ipairs(tx.Units) do
        if snap.ID == asset.Unit.ID then
          assert((snap.RequestedOutcome or snap.Outcome) == d.Outcome
            and (snap.RequestedOutcome and snap.RequestedDestination or snap.Destination) == d.Owner,
            "linked person/unit dispositions disagree")
          found = true
        end
      end
      assert(found or dormant[id], "roster points to missing unit: " .. id)
    end
    local heroSnap
    if asset.Kind == "HERO" then
      for _, snap in ipairs(tx.Units) do
        if snap.Hero and asset.Unit and asset.Unit.Owner == tx.From and asset.Unit.ID == snap.ID then heroSnap = snap end
      end
      if (heroSnap and heroSnap.SuicideCause) or dormant[id] then
        if dormant[id] then
          if (plan.Assets or {})[id] then d.RequestedOutcome = d.Outcome end
        else
          d.RequestedOutcome = heroSnap.RequestedOutcome
        end
        d.Outcome, d.Owner = "SUICIDE", -1
        d.SuicideCause = dormant[id] and "DORMANT_CLAIM" or heroSnap.SuicideCause
      end
    end
    tx.Decisions[id] = d
    if asset.Kind == "HERO" and asset.NativeHero then
      tx.HeroResults[id] = { Class = asset.NativeHero.ClassType,
        Outcome = d.Outcome, RequestedOutcome = d.RequestedOutcome or d.Outcome,
        Cause = d.SuicideCause, NativeClaimAfter = dormant[id] and tx.From
          or heroSnap and heroSnap.NativeClaimAfter }
    end
  end
  for _, city in source:GetCities():Members() do
    tx.Cities[#tx.Cities + 1] = { X = city:GetX(), Y = city:GetY(), Population = city:GetPopulation(),
      Faith = Religion(city), Assets = CityAssets(city), PalaceWorks = PalaceWorks(city),
      Governance = R.Copy(city:GetProperty(R.GOVERNANCE_KEY)), Properties = Properties(city, cityKeys),
      PropertyKeys = R.Copy(cityKeys), RebindKeys = RebindKeys(cityRebinders), Name = city:GetName() }
  end
  table.sort(tx.Cities, function(a, b) return a.Y == b.Y and a.X < b.X or a.Y < b.Y end)
  for index = 0, Map.GetPlotCount() - 1 do
    if Map.GetPlotByIndex(index):GetOwner() == tx.From then tx.Plots[#tx.Plots + 1] = index end
  end
  for _, name in ipairs(R.Keys(adapters)) do
    tx.Adapters[name] = R.Copy(adapters[name].Prepare(R.Copy(tx)))
    assert(type(tx.Adapters[name]) == "table", "adapter Prepare must return a table: " .. name)
  end
  return tx
end

local function FindToken(owner, token)
  local found
  for _, unit in ipairs(Units(owner)) do
    if unit:GetProperty(R.TOKEN_KEY) == token then
      assert(not found, "duplicate transferred unit token: " .. token)
      found = unit
    end
  end
  return found
end
A.FindUnit = FindToken

local function VerifySpread(unit, snap, state, tx, phase, batch)
  if snap.SpreadCharges == nil then return true end
  if unit.GetSpreadCharges or not tx then
    assert(SpreadCharges(unit) == snap.SpreadCharges, "spent spread charges need adapter")
    return true
  end
  local owner, id = unit:GetOwner(), unit:GetID()
  local token = snap.Token .. ":spread:" .. phase .. ":" .. id
  snap.SpreadReads = snap.SpreadReads or {}
  local read = snap.SpreadReads[phase]
  if not read or read.Token ~= token or read.Owner ~= owner or read.Turn ~= tx.Turn then
    read = { Token = token, Owner = owner, ID = id, Turn = tx.Turn, AwaitingUI = true }
    snap.SpreadReads[phase] = read
    Save(state)
    if batch then return false end
    error("JAC_ABSORPTION_WAITING_UI", 0)
  end
  if read.Verified then return true end
  local reads = ExposedMembers.JAC_HeroReadAPI
  local charges = reads and reads.GetUnitObservation and reads.GetUnitObservation(token)
  if charges == nil then
    if batch then return false end
    error("JAC_ABSORPTION_WAITING_UI", 0)
  end
  assert(type(charges) == "number" and charges == snap.SpreadCharges,
    "spent spread charges need adapter")
  read.AwaitingUI, read.Verified = nil, true
  Save(state)
  return true
end

local function RestoreUnit(unit, snap, coreOnly)
  if not coreOnly then WriteProperties(unit, snap.TransferredProperties, snap.PropertyKeys) end
  -- Official Debug/Unit.ltp:164,175,463; NubiaScenario.lua:150-151; Tutorial.lua:71.
  if snap.Formation and unit:GetMilitaryFormation() ~= snap.Formation then unit:SetMilitaryFormation(snap.Formation) end
  local xp = unit:GetExperience()
  if snap.Name and unit:GetName() ~= snap.Name then xp:SetVeteranName(snap.Name) end
  for _, promotion in ipairs(snap.Promotions or {}) do
    if not xp:HasPromotion(promotion) then xp:SetPromotion(promotion) end
  end
  if snap.XP then xp:ChangeExperience(snap.XP - xp:GetExperiencePoints()) end
  if snap.Religion and snap.Religion >= 0 then
    local religion = unit:GetReligion()
    if religion:GetReligionType() ~= snap.Religion then
      religion:SetReligionType(GameInfo.Religions[snap.Religion].ReligionType)
    end
  end
  if snap.Damage then unit:SetDamage(snap.Damage) end
  if snap.Moves then unit:ChangeMovesRemaining(snap.Moves - unit:GetMovesRemaining()) end
  -- No documented setter for spent charges; VerifySpread waits for a published UI read when needed.
  if snap.BuildCharges then assert(unit:GetBuildCharges() == snap.BuildCharges, "spent build charges need adapter") end
  if snap.GreatPersonCharges then
    assert(unit:GetGreatPerson():GetActionCharges() == snap.GreatPersonCharges, "spent great-person charges need adapter")
  end
  if snap.Hero and not snap.NativeRestored then
    assert(unit:IsHero() and unit:GetHeroClassType() == snap.Hero.Class, "native Hero identity changed")
    -- Restore after promotions/other initialization that may change derived unit state.
    unit:SetProperty(snap.Hero.LifespanKey, snap.Hero.Lifespan)
    if snap.Hero.Renown ~= nil then
      assert(unit.SetRenown, "native Hero renown setter unavailable after reload")
      unit:SetRenown(snap.Hero.Renown)
    end
    unit:ChangeActionCharges(snap.Hero.ActionCharges - unit:GetActionCharges())
  end
end

local function PlanProperties(snap, tx, rebinders, isUnit)
  -- Once saved, these expected values are authoritative on resume, even if a callback is not loaded.
  if snap.TransferredProperties ~= nil then return end
  local properties = R.Copy(snap.Properties)
  for _, key in ipairs(R.Keys(snap.RebindKeys or {})) do
    local rebind = assert(rebinders[key], "re-register property rebind before staging: " .. key)
    properties[key] = R.Copy(rebind(R.Copy(snap.Properties[key]), R.Copy(tx), R.Copy(snap)))
  end
  if isUnit and snap.Properties[R.PERSON_KEY] then
    local person = R.Rebind(snap.Properties[R.PERSON_KEY], tx)
    person.OriginalOwner = person.OriginalOwner or tx.From
    person.Owner, person.Outcome = snap.Destination, snap.Outcome
    properties[R.PERSON_KEY] = person
  end
  snap.TransferredProperties = properties
end

local function VerifyHero(unit, snap, exact)
  assert(unit and unit:IsHero() and unit:GetHeroClassType() == snap.Hero.Class, "native Hero identity changed")
  local manager, origin = Heroes(), snap.Hero.Origin
  assert(manager:GetHeroClaimPlayer(snap.Hero.Class) == snap.Destination, "native Hero manager owner changed")
  local current = HeroOrigin(manager, snap.Hero.Class)
  assert(current.Owner == snap.Destination and current.X == origin.X and current.Y == origin.Y,
    "native Hero recall origin changed")
  local matches = HeroUnits(snap.Destination, snap.Hero.Class)
  assert(#matches == 1 and matches[1]:GetID() == unit:GetID(), "duplicate native Hero class")
  assert(manager:IsHeroDiscovered(snap.Destination, snap.Hero.Class), "native Hero discovery lost")
  if exact then
    assert(unit:GetProperty(snap.Hero.LifespanKey) == snap.Hero.Lifespan, "native Hero lifespan changed")
    assert(unit:GetActionCharges() == snap.Hero.ActionCharges, "native Hero action charges changed")
    if snap.Hero.Renown ~= nil then
      assert(unit.GetRenown and unit:GetRenown() == snap.Hero.Renown, "native Hero renown changed")
    end
  end
end

local function VerifyDormantHeroRights(snap)
  local manager = Heroes()
  local origin = HeroOrigin(manager, snap.Hero.Class)
  assert(manager:GetHeroClaimPlayer(snap.Hero.Class) == snap.Destination
    and origin.Owner == snap.Destination and origin.X == snap.Hero.Origin.X
    and origin.Y == snap.Hero.Origin.Y, "dormant native Hero recall rights changed")
  assert(#HeroUnits(snap.Destination, snap.Hero.Class) == 0,
    "new native Hero requires person record reattachment")
end

local function VerifyUnit(unit, snap, coreOnly)
  assert(unit and unit:GetOwner() == snap.Destination and GameInfo.Units[unit:GetType()].UnitType == snap.Type,
    "transferred unit missing/type/owner mismatch: " .. snap.Token)
  assert(unit:GetX() >= 0 and unit:GetY() >= 0 and Map.GetPlot(unit:GetX(), unit:GetY()),
    "transferred unit is off-map: " .. snap.Token)
  assert(unit:GetProperty(R.TOKEN_KEY) == snap.Token, "unit token mismatch")
  local greatPerson = unit:GetGreatPerson()
  assert((greatPerson and greatPerson:GetIndividual() or -1) == snap.Individual, "great person individual changed")
  if snap.GreatPersonCharges then
    assert(greatPerson:GetActionCharges() == snap.GreatPersonCharges, "great person action charges changed")
  end
  if snap.Hero then VerifyHero(unit, snap, true) end
  if not coreOnly then
    assert(R.Equal(Properties(unit, snap.PropertyKeys), snap.TransferredProperties), "unit properties changed: " .. snap.Token)
  end
  if snap.Name then assert(unit:GetName() == snap.Name, "unit name changed") end
  if snap.Damage then assert(unit:GetDamage() == snap.Damage, "unit damage changed") end
  if snap.Moves then assert(unit:GetMovesRemaining() == snap.Moves, "unit movement changed") end
  if snap.XP then assert(unit:GetExperience():GetExperiencePoints() == snap.XP, "unit XP changed") end
  if snap.Formation then assert(unit:GetMilitaryFormation() == snap.Formation, "unit formation changed") end
  if snap.Religion then
    local religion = unit:GetReligion()
    assert((religion and religion:GetReligionType() or -1) == snap.Religion, "unit religion changed")
  end
  for _, promotion in ipairs(snap.Promotions or {}) do assert(unit:GetExperience():HasPromotion(promotion), "promotion lost") end
end

local function Spawn(snap)
  assert(not snap.Hero, "native Hero must be created through HeroesManager at its transferred origin")
  local before = {}
  for _, unit in ipairs(Units(snap.Destination)) do before[unit:GetID()] = true end
  -- The donor's province can reject the new owner's peaceful units. Stage at
  -- the recipient's city, verify the clone, then restore position after transfer.
  local staging = assert(Players[snap.Destination]:GetCities():GetCapitalCity(), "destination staging city unavailable")
  local x, y = staging:GetX(), staging:GetY()
  if snap.Individual >= 0 then
    -- Official TunerMapPanel.lua:190 + Debug/Map.ltp:579: individual INDEX, not hash.
    Game.GetGreatPeople():CreatePerson(snap.Destination, snap.Individual, x, y)
  else
    -- Returns no Unit object (verified live); discover the new ID afterwards.
    UnitManager.InitUnitValidAdjacentHex(snap.Destination, snap.Type, x, y, R.UNIT_SEARCH_RADIUS_POC)
  end
  local found
  for _, unit in ipairs(Units(snap.Destination)) do
    if not before[unit:GetID()] and GameInfo.Units[unit:GetType()].UnitType == snap.Type then
      assert(not found, "more than one new unit: " .. snap.Token)
      found = unit
    end
  end
  assert(found, "engine did not create unit: " .. snap.Token)
  found:SetProperty(R.TOKEN_KEY, snap.Token)
  return found
end

local function SetHeroSuicide(state, tx, snap, cause, candidate)
  local intent = snap.HeroSpawnIntent
  if candidate then
    local id, token = candidate:GetID(), candidate:GetProperty(R.TOKEN_KEY)
    assert(candidate:GetOwner() == tx.To and candidate:IsHero() and candidate:GetHeroClassType() == snap.Hero.Class
      and intent and not intent.Before[id] and (not intent.CreatedID or intent.CreatedID == id)
      and (not token or token == snap.Token), "unrelated native Hero candidate; review recovery")
    UnitManager.Kill(candidate) -- Only the saved, newly issued candidate can be removed.
  end
  tx.UnitMap[R.UnitKey(tx.From, snap.ID)] = nil
  HeroSuicide(snap, cause)
  for id, asset in pairs(tx.SourceRoster) do
    if asset.Unit and asset.Unit.Owner == tx.From and asset.Unit.ID == snap.ID then
      local decision = assert(tx.Decisions[id])
      decision.RequestedOutcome = snap.RequestedOutcome
      decision.Outcome, decision.Owner, decision.SuicideCause = "SUICIDE", -1, cause
      tx.HeroResults[id] = { Class = snap.Hero.ClassType, Outcome = "SUICIDE",
        RequestedOutcome = snap.RequestedOutcome, Cause = cause, NativeClaimAfter = snap.NativeClaimAfter }
    end
  end
  Save(state)
end

local function StageHero(state, tx, snap)
  local manager, hero = Heroes(), snap.Hero
  local key = R.UnitKey(tx.From, snap.ID)
  local unit = FindToken(snap.Destination, snap.Token)
  local intent = snap.HeroSpawnIntent
  if not unit then
    assert(not tx.UnitMap[key], "staged replacement was lost; review recovery: " .. snap.Token)
    local claim = manager:GetHeroClaimPlayer(hero.Class)
    if claim ~= tx.From and claim ~= tx.To then
      SetHeroSuicide(state, tx, snap, "CLAIM_UNAVAILABLE")
      return
    end
    if not snap.HeroEscrow then
      local old = assert(UnitManager.GetUnit(tx.From, snap.ID), "native Hero vanished before escrow")
      assert(old:IsHero() and old:GetHeroClassType() == hero.Class
        and R.Equal(SnapshotHero(old, GameInfo.Units[old:GetType()]), hero), "native Hero changed before escrow")
      snap.HeroEscrow = true
      Save(state)
    end
    local old = UnitManager.GetUnit(tx.From, snap.ID)
    if old then
      assert(GameInfo.Units[old:GetType()].UnitType == snap.Type and old:IsHero()
        and old:GetHeroClassType() == hero.Class, "native Hero donor identity changed")
      if snap.Properties[R.PERSON_KEY] then
        assert(R.Equal(old:GetProperty(R.PERSON_KEY), snap.Properties[R.PERSON_KEY]),
          "native Hero donor person changed")
      end
      UnitManager.Kill(old)
      Save(state)
    end
    local city = assert(CityManager.GetCityAt(hero.Origin.X, hero.Origin.Y), "native Hero origin city disappeared")
    assert(city:GetOwner() == tx.From or city:GetOwner() == tx.To, "native Hero origin belongs to a third faction")
    if city:GetOwner() == tx.From then
      CityManager.TransferCity(city, tx.To) -- City failures remain BLOCKED.
      Save(state)
      city = assert(CityManager.GetCityAt(hero.Origin.X, hero.Origin.Y), "native Hero origin transfer removed city")
    end
    if not city:GetBuildings():HasBuilding(hero.OriginBuilding) then
      SetHeroSuicide(state, tx, snap, "ORIGIN_BUILDING_UNAVAILABLE")
      return
    end
    local matches = HeroUnits(tx.To, hero.Class)
    assert(#matches <= 1, "multiple native Hero candidates; review recovery")
    unit = matches[1]
    if unit and (not intent or intent.Before[unit:GetID()] or
      intent.CreatedID and intent.CreatedID ~= unit:GetID()) then
      SetHeroSuicide(state, tx, snap, "HERO_CLASS_COLLISION")
      return
    end
    if not unit then
      if intent and intent.CreatedID then
        SetHeroSuicide(state, tx, snap, "HERO_CANDIDATE_LOST")
        return
      end
      if not intent then
        local before = {}
        for _, current in ipairs(Units(tx.To)) do before[current:GetID()] = true end
        intent = { Before = before }
        snap.HeroSpawnIntent = intent
        Save(state)
      end
      -- CreateHero(player, class, x, y) has no return value and can change the
      -- native claim before it raises. Reconcile the saved ID set even on error.
      local ok = pcall(manager.SetHeroDiscovered, manager, tx.To, hero.Class, true)
      if ok then ok = pcall(manager.CreateHero, manager, tx.To, hero.Class, city:GetX(), city:GetY()) end
      matches = HeroUnits(tx.To, hero.Class)
      assert(#matches <= 1, "multiple native Hero candidates; review recovery")
      unit = matches[1]
      if not unit then
        SetHeroSuicide(state, tx, snap, ok and "HERO_CREATE_NO_UNIT" or "HERO_CREATE_ERROR")
        return
      end
      if intent.Before[unit:GetID()] then
        SetHeroSuicide(state, tx, snap, "HERO_CLASS_COLLISION")
        return
      end
    end
    assert(not intent.CreatedID or intent.CreatedID == unit:GetID(), "native Hero candidate identity changed")
    if intent.CreatedID == nil then
      intent.CreatedID = unit:GetID()
      local native = Game.GetHeroesManager()
      intent.AwaitingUI = native.GetHeroClaimPlayer == nil or nil
    end
    Save(state)
  else
    assert(intent and not intent.Before[unit:GetID()] and
      (not intent.CreatedID or intent.CreatedID == unit:GetID()), "native Hero token/candidate collision")
  end
  if intent.AwaitingUI then
    local reads = ExposedMembers.JAC_HeroReadAPI
    local token = snap.Token .. ":created:" .. intent.CreatedID
    if not reads or not reads.GetObservation or not reads.GetObservation(token) then
      error("JAC_ABSORPTION_WAITING_UI", 0)
    end
    intent.AwaitingUI = nil
    Save(state)
  end
  local ok = pcall(VerifyHero, unit, snap, false)
  if not ok then
    SetHeroSuicide(state, tx, snap, "HERO_MANAGER_MISMATCH", unit)
    return
  end
  local token = unit:GetProperty(R.TOKEN_KEY)
  assert(not token or token == snap.Token, "native Hero candidate belongs to another transaction")
  if not token then
    local written = pcall(unit.SetProperty, unit, R.TOKEN_KEY, snap.Token)
    if not written then
      SetHeroSuicide(state, tx, snap, "HERO_TOKEN_ERROR", unit)
      return
    end
  end
  tx.UnitMap[key] = { ID = unit:GetID(), Token = snap.Token }
  Save(state)
  local restored = pcall(function()
    if unit:GetX() ~= snap.X or unit:GetY() ~= snap.Y then UnitManager.PlaceUnit(unit, snap.X, snap.Y) end
    assert(unit:GetX() == snap.X and unit:GetY() == snap.Y, "native Hero placement failed")
    RestoreUnit(unit, snap, true)
    VerifyUnit(unit, snap, true)
  end)
  if not restored then
    SetHeroSuicide(state, tx, snap, "HERO_RESTORE_ERROR", unit)
    return
  end
  snap.NativeRestored = true
  Save(state)
  -- A missing Core getter is not a Hero restoration failure. Keep the saved candidate
  -- while the UI observes it at the next game-core publication.
  local spreadOK, spreadError = pcall(VerifySpread, unit, snap, state, tx, "hero-stage")
  if not spreadOK then
    if tostring(spreadError) == "JAC_ABSORPTION_WAITING_UI" then error(spreadError, 0) end
    SetHeroSuicide(state, tx, snap, "HERO_RESTORE_ERROR", unit)
  end
end

local function MovedAssets(tx)
  local sourceNow = GetRoster(tx.From)
  assert((tx.SourceRosterMoved and next(sourceNow) == nil) or
    (not tx.SourceRosterMoved and R.Equal(sourceNow, tx.OriginalSourceRoster)),
    "source roster changed during absorption; review new assets before clearing")
  -- Merge into current data: city hooks or a failed attempt may have added/updated unrelated assets.
  local rosters, archive = { [tx.To] = GetRoster(tx.To) }, {}
  for _, id in ipairs(R.Keys(tx.SourceRoster)) do
    local record = R.Rebind(tx.SourceRoster[id], tx)
    local decision = tx.Decisions[id]
    record.OriginalOwner = record.OriginalOwner or tx.From
    record.Owner, record.Outcome = decision.Owner, decision.Outcome
    if record.Kind == "HERO" and decision.Outcome == "SUICIDE" then
      record.RequestedOutcome = decision.RequestedOutcome or "SUICIDE"
      record.SuicideCause = decision.SuicideCause or "EXPLICIT_SUICIDE"
      record.NativeClaimAfter = tx.HeroResults[id] and tx.HeroResults[id].NativeClaimAfter
      for _, snap in ipairs(tx.Units) do
        if snap.Hero and tx.SourceRoster[id].Unit and tx.SourceRoster[id].Unit.Owner == tx.From
          and tx.SourceRoster[id].Unit.ID == snap.ID then
          record.NativeUnit = R.Copy(snap.NativeSnapshot)
        end
      end
    end
    if tx.RecallRecords and tx.RecallRecords[id] then record = R.Copy(tx.RecallRecords[id]) end
    if record.Owner < 0 then archive[id] = record else
      rosters[record.Owner] = rosters[record.Owner] or GetRoster(record.Owner)
      assert(not rosters[record.Owner][id] or R.Equal(rosters[record.Owner][id], record), "destination asset collision")
      rosters[record.Owner][id] = record
    end
  end
  for owner, roster in pairs(rosters) do
    for id, record in pairs(roster) do assert(record.ID == id and record.Owner == owner, "invalid destination roster ownership: " .. id) end
  end
  return rosters, archive
end

local function Verify(tx, state, exactUnits)
  assert(Players[tx.From]:GetCities():GetCount() == 0 and Players[tx.From]:GetUnits():GetCount() == 0,
    "donor still owns cities/units")
  for _, snap in ipairs(tx.Units) do
    if snap.Hero and snap.Outcome == "SUICIDE" then
      assert(not FindToken(tx.To, snap.Token), "suicided native Hero still has a transferred Unit")
      if snap.HeroSpawnIntent and snap.HeroSpawnIntent.CreatedID then
        local candidate = UnitManager.GetUnit(tx.To, snap.HeroSpawnIntent.CreatedID)
        assert(not candidate or not (candidate.IsHero and candidate:IsHero()
          and candidate:GetHeroClassType() == snap.Hero.Class), "suicided native Hero candidate still lives")
      end
    end
    if snap.KeepUnit then
      local recalled = snap.Hero and tx.HeroRecallUnits and tx.HeroRecallUnits[snap.ID]
      if recalled then
        local unit = FindToken(tx.To, recalled.Token)
        if not unit and not exactUnits then
          VerifyDormantHeroRights(snap)
        else
          assert(unit and unit:GetID() == recalled.ID and unit:IsHero()
            and unit:GetHeroClassType() == snap.Hero.Class, "recalled native Hero binding lost")
          local manager = Heroes()
          local origin = HeroOrigin(manager, snap.Hero.Class)
          assert(manager:GetHeroClaimPlayer(snap.Hero.Class) == tx.To and origin.Owner == tx.To
            and origin.X == snap.Hero.Origin.X and origin.Y == snap.Hero.Origin.Y,
            "recalled native Hero rights changed")
        end
      else
      local unit = FindToken(snap.Destination, snap.Token)
      if exactUnits then
        VerifyUnit(unit, snap)
        VerifySpread(unit, snap)
        if snap.Hero then
          assert(unit:GetX() == snap.X and unit:GetY() == snap.Y, "native Hero position changed")
        end
      else
        if snap.Hero and not unit then
          VerifyDormantHeroRights(snap)
        else
          assert(unit and unit:GetOwner() == snap.Destination, "transferred unit disappeared")
          if snap.Hero then VerifyHero(unit, snap, false) end
        end
      end
      end
    end
  end
  local greatWorks = OwnedGreatWorks(tx.To)
  for _, snap in ipairs(tx.Cities) do
    local city = CityManager.GetCityAt(snap.X, snap.Y)
    assert(city and city:GetOwner() == tx.To, "city transfer failed")
    assert(R.Equal(Properties(city, snap.PropertyKeys), snap.TransferredProperties), "city properties changed")
    if exactUnits then
      assert(R.Equal(CityAssets(city), snap.Assets), "city buildings/Great Works changed")
    else
      local current = CityAssets(city)
      for buildingType, original in pairs(snap.Assets) do
        assert(current[buildingType], "transferred city building lost: " .. buildingType)
        for _, work in pairs(original.Works or {}) do
          if work and work >= 0 then assert(greatWorks[work], "transferred Great Work lost: " .. work) end
        end
      end
    end
    if exactUnits then
      assert(city:GetPopulation() == snap.Population and R.Equal(Religion(city), snap.Faith), "city population/religion changed")
    end
    assert(city:GetProperty(R.GOVERNANCE_KEY).From == tx.From, "governance provenance lost")
    assert(R.Equal(city:GetProperty(R.GOVERNANCE_KEY).Previous, snap.Governance), "previous governance lost")
    for _, work in ipairs(snap.PalaceWorks or {}) do assert(greatWorks[work], "Palace Great Work lost: " .. work) end
  end
  for _, index in ipairs(tx.Plots) do assert(Map.GetPlotByIndex(index):GetOwner() == tx.To, "territory transfer failed: " .. index) end
  local rosters, archive = MovedAssets(tx)
  for id, result in pairs(tx.HeroResults or {}) do
    local decision = assert(tx.Decisions[id], "native Hero decision missing")
    assert(result.Outcome == decision.Outcome
      and result.RequestedOutcome == (decision.RequestedOutcome or decision.Outcome),
      "native Hero result/decision mismatch")
    if result.Outcome == "SUICIDE" then
      assert(archive[id] and archive[id].SuicideCause == result.Cause,
        "native Hero death was reported without history")
    end
  end
  for owner, roster in pairs(rosters) do assert(R.Equal(GetRoster(owner), roster), "roster transfer mismatch") end
  assert(next(GetRoster(tx.From)) == nil, "source roster not cleared")
  for id, record in pairs(archive) do assert(R.Equal(state.Archive[id], record), "archived asset lost") end
  for _, name in ipairs(R.Keys(tx.Adapters)) do
    assert(adapters[name], "re-register absorption adapter after load: " .. name)
    adapters[name].Verify(R.Copy(tx), R.Copy(tx.Adapters[name]))
  end
end

local function Execute(state, tx)
  -- Ordinary units can be staged without affecting ownership. Native heroes require their origin city.
  for _, snap in ipairs(tx.Units) do
    if snap.KeepUnit and not snap.Hero then
      local unit = FindToken(snap.Destination, snap.Token)
      assert(unit or not tx.UnitMapFrozen, "staged replacement was lost; review recovery: " .. snap.Token)
      if tx.UnitMapFrozen then
        local mapped = assert(tx.UnitMap[R.UnitKey(tx.From, snap.ID)], "frozen unit mapping missing")
        assert(unit:GetID() == mapped.ID, "frozen unit mapping changed")
      end
      unit = unit or Spawn(snap)
      tx.UnitMap[R.UnitKey(tx.From, snap.ID)] = { ID = unit:GetID(), Token = snap.Token }
      Save(state)
      -- InitUnitValidAdjacentHex can issue an ID while leaving the unit off-map
      -- (verified live: -9999/-9999). Such a unit never enters the UI unit cache.
      if unit:GetX() < 0 or unit:GetY() < 0 or not Map.GetPlot(unit:GetX(), unit:GetY()) then
        local staging = assert(Players[snap.Destination]:GetCities():GetCapitalCity(), "destination staging city unavailable")
        UnitManager.PlaceUnit(unit, staging:GetX(), staging:GetY())
        assert(unit:GetX() >= 0 and unit:GetY() >= 0 and Map.GetPlot(unit:GetX(), unit:GetY()),
          "staged unit placement failed; saved candidate retained")
        Save(state)
        Log("placed off-map candidate token=" .. snap.Token .. " id=" .. unit:GetID())
      end
    end
  end
  local hasHeroes = false
  for _, snap in ipairs(tx.Units) do if snap.Hero and snap.KeepUnit then hasHeroes = true end end
  if hasHeroes and not tx.UnitMapFrozen then
    MovedAssets(tx) -- check concurrent roster conflicts before any native escrow/city change
    local ready = true
    for _, snap in ipairs(tx.Units) do
      if snap.KeepUnit and not snap.Hero then
        local unit = FindToken(snap.Destination, snap.Token)
        RestoreUnit(unit, snap, true)
        VerifyUnit(unit, snap, true)
        if not VerifySpread(unit, snap, state, tx, "stage", true) then ready = false end
      end
    end
    if not ready then error("JAC_ABSORPTION_WAITING_UI", 0) end
  end
  for _, snap in ipairs(tx.Units) do
    if snap.Hero and snap.KeepUnit then StageHero(state, tx, snap) end
  end
  for _, snap in ipairs(tx.Units) do if snap.Hero then snap.DispositionFrozen = true end end
  -- Freeze the ENTIRE map before the first property can contain a new component reference.
  tx.UnitMapFrozen = true
  Save(state)
  -- Materialize and save all expected references while every donor still exists.
  for _, snap in ipairs(tx.Units) do
    if snap.KeepUnit then PlanProperties(snap, tx, unitRebinders, true) end
  end
  for _, snap in ipairs(tx.Cities) do PlanProperties(snap, tx, cityRebinders, false) end
  Save(state)
  MovedAssets(tx) -- detect newly conflicting destination IDs before donor deletion
  local ready = true
  for _, snap in ipairs(tx.Units) do
    if snap.KeepUnit then
      local unit = FindToken(snap.Destination, snap.Token)
      RestoreUnit(unit, snap)
      VerifyUnit(unit, snap)
      if not VerifySpread(unit, snap, state, tx, "final", true) then ready = false end
      Save(state)
    end
  end
  if not ready then error("JAC_ABSORPTION_WAITING_UI", 0) end
  tx.Status = "STAGED"
  Save(state)
  for _, snap in ipairs(tx.Units) do
    if snap.ID >= 0 then
      local old = UnitManager.GetUnit(tx.From, snap.ID)
      if old then
        assert(GameInfo.Units[old:GetType()].UnitType == snap.Type, "donor unit ID was reused")
        if snap.Hero then
          assert(old:IsHero() and old:GetHeroClassType() == snap.Hero.Class,
            "donor native Hero identity changed")
          if snap.Properties[R.PERSON_KEY] then
            assert(R.Equal(old:GetProperty(R.PERSON_KEY), snap.Properties[R.PERSON_KEY]),
              "donor native Hero person changed")
          end
        end
        UnitManager.Kill(old) -- never use the killed object again (AustraliaScenario.lua:807).
      end
    end
  end
  for _, snap in ipairs(tx.Cities) do
    local city = assert(CityManager.GetCityAt(snap.X, snap.Y), "city disappeared during absorption")
    assert(city:GetOwner() == tx.From or city:GetOwner() == tx.To, "city belongs to a third faction")
    if city:GetOwner() == tx.From then CityManager.TransferCity(city, tx.To) end
    city = assert(CityManager.GetCityAt(snap.X, snap.Y), "transfer removed city")
    WriteProperties(city, snap.TransferredProperties, snap.PropertyKeys)
    local previous = city:GetProperty(R.GOVERNANCE_KEY)
    if not previous or previous.ID ~= tx.ID then
      city:SetProperty(R.GOVERNANCE_KEY, { ID = tx.ID, From = tx.From, To = tx.To, Turn = tx.Turn,
        Mode = tx.Mode, LoyaltyDelta = R.CITY_LOYALTY_DELTA_POC,
        FrictionTurns = R.CULTURAL_FRICTION_TURNS_POC, BalancePending = true, Previous = R.Copy(snap.Governance) })
      if R.CITY_LOYALTY_DELTA_POC ~= 0 then city:ChangeLoyalty(R.CITY_LOYALTY_DELTA_POC) end
    end
    Save(state)
  end
  -- Restore all native positions only after the province belongs to the recipient.
  -- Official gameplay example: DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:854,886.
  ready = true
  for _, snap in ipairs(tx.Units) do
    if snap.KeepUnit then
      local unit = assert(FindToken(snap.Destination, snap.Token), "transferred unit lost before placement")
      if unit:GetX() ~= snap.X or unit:GetY() ~= snap.Y then UnitManager.PlaceUnit(unit, snap.X, snap.Y) end
      assert(unit:GetX() == snap.X and unit:GetY() == snap.Y, "transferred unit placement failed; saved snapshot retained")
      RestoreUnit(unit, snap)
      VerifyUnit(unit, snap)
      if not VerifySpread(unit, snap, state, tx, "placed", true) then ready = false end
      Save(state)
    end
  end
  if not ready then error("JAC_ABSORPTION_WAITING_UI", 0) end
  -- Finish all city mutations, then let UI-only citizen/work caches catch up.
  if tx.Cities[1] then
    local first = CityManager.GetCityAt(tx.Cities[1].X, tx.Cities[1].Y)
    if not first:GetReligion().GetReligionsInCity or not first:GetBuildings().GetNumGreatWorkSlots
      or not first:GetBuildings().GetGreatWorkInSlot then
      local reads = ExposedMembers.JAC_HeroReadAPI
      if not reads or not reads.GetObservation or not reads.GetObservation(tx.ID .. ":final") then
        tx.AwaitingFinalUI = true
        Save(state)
        error("JAC_ABSORPTION_WAITING_UI", 0)
      end
      tx.AwaitingFinalUI = nil
    end
  end
  local rosters, archive = MovedAssets(tx)
  for _, owner in ipairs(R.Keys(rosters)) do Players[owner]:SetProperty(R.ROSTER_KEY, R.Copy(rosters[owner])) end
  Players[tx.From]:SetProperty(R.ROSTER_KEY, nil)
  tx.SourceRosterMoved = true
  for _, id in ipairs(R.Keys(archive)) do state.Archive[id] = R.Copy(archive[id]) end
  for _, id in ipairs(R.Keys(tx.SourceRoster)) do
    local record = tx.SourceRoster[id]
    if record.Kind == "UNLOCK" and tx.Decisions[id].Outcome == "INHERIT" then
      if record.Technology then Players[tx.To]:GetTechs():SetTech(GameInfo.Technologies[record.Technology].Index, true) end
      if record.Civic then Players[tx.To]:GetCulture():SetCivic(GameInfo.Civics[record.Civic].Index, true) end
    end
  end
  for _, name in ipairs(R.Keys(tx.Adapters)) do
    assert(adapters[name], "missing absorption adapter: " .. name)
    adapters[name].Apply(R.Copy(tx), R.Copy(tx.Adapters[name]))
  end
  Save(state)
  Verify(tx, state, true)
  tx.Status, tx.Error = "COMPLETE", nil
  Save(state)
  Log("complete id=" .. tx.ID .. " from=" .. tx.From .. " to=" .. tx.To
    .. " units=" .. #tx.Units .. " cities=" .. #tx.Cities .. " plots=" .. #tx.Plots)
  return R.Copy(tx)
end

-- The UI issues the normal city purchase between these two calls. A saved intent
-- makes a new native Unit ID distinguishable from a pre-existing same-class unit.
function A.BeginHeroRecall(id, transactionID, recordID)
  assert(type(id) == "string" and id ~= "", "unique recall ID required")
  local state = State()
  local existing = state.Recalls[id]
  if existing then
    assert(existing.Transaction == transactionID and existing.RecordID == recordID, "recall ID reused")
    return R.Copy(existing)
  end
  local tx = assert(state.Transactions[transactionID], "unknown absorption transaction")
  assert(tx.Status == "COMPLETE", "recall requires a completed absorption")
  local record = assert(GetRoster(tx.To)[recordID], "native Hero person record missing")
  assert(record.Kind == "HERO" and record.Owner == tx.To and record.Outcome ~= "SUICIDE"
    and record.NativeHero and record.NativeHero.Class, "record has no inherited native Hero rights")
  local class = record.NativeHero.Class
  for otherID, pending in pairs(state.Recalls) do
    assert(otherID == id or pending.Status ~= "PENDING" or pending.Owner ~= tx.To
      or pending.Class ~= class, "native Hero already has a pending recall")
  end
  local source = tx.SourceRoster[recordID]
  assert(source and source.Kind == "HERO" and source.NativeHero and source.NativeHero.Class == class
    and source.Unit and source.Unit.Owner == tx.From,
    "Hero record is unrelated to this absorption")
  local current = (tx.HeroRecallUnits or {})[source.Unit.ID] or tx.UnitMap[R.UnitKey(tx.From, source.Unit.ID)]
  -- OriginalOwner is historical provenance and can predate this transaction.
  -- The current transfer/recall mapping identifies the last unit associated with it.
  assert(current and record.Unit and record.Unit.Owner == tx.To and record.Unit.ID == current.ID
    and record.Unit.Token == current.Token, "current native Hero transfer mapping mismatch")
  for otherID, other in pairs(GetRoster(tx.To)) do
    assert(otherID == recordID or not (other.Kind == "HERO" and other.NativeHero
      and other.NativeHero.Class == class and other.Outcome ~= "SUICIDE"), "ambiguous native Hero person record")
  end
  assert(#HeroUnits(tx.To, class) == 0, "native Hero is still alive")
  local manager = Heroes()
  local origin = HeroOrigin(manager, class)
  assert(manager:GetHeroClaimPlayer(class) == tx.To and origin.Owner == tx.To
    and origin.X == record.NativeHero.Origin.X and origin.Y == record.NativeHero.Origin.Y,
    "native Hero recall rights changed")
  local before = {}
  for _, unit in ipairs(Units(tx.To)) do before[unit:GetID()] = true end
  local intent = { ID = id, Transaction = transactionID, RecordID = recordID,
    Owner = tx.To, Class = class, Before = before, Record = R.Copy(record),
    Token = id .. ":unit", Turn = Game.GetCurrentGameTurn(), Status = "PENDING" }
  state.Recalls[id] = intent
  Save(state)
  return R.Copy(intent)
end

function A.CompleteHeroRecall(id)
  local state = State()
  local intent = assert(state.Recalls[id], "unknown native Hero recall")
  local tx = assert(state.Transactions[intent.Transaction], "recall transaction missing")
  assert(tx.Status == "COMPLETE", "recall absorption is incomplete")
  local matches = HeroUnits(intent.Owner, intent.Class)
  assert(#matches == 1, "expected exactly one recalled native Hero")
  local unit = matches[1]
  assert(not intent.Before[unit:GetID()] and
    (not intent.CandidateID or intent.CandidateID == unit:GetID()), "recalled Hero candidate ID changed")
  local manager = Heroes()
  local origin = HeroOrigin(manager, intent.Class)
  assert(manager:GetHeroClaimPlayer(intent.Class) == intent.Owner and origin.Owner == intent.Owner
    and origin.X == intent.Record.NativeHero.Origin.X and origin.Y == intent.Record.NativeHero.Origin.Y,
    "recalled Hero claim/origin changed")
  local token = unit:GetProperty(R.TOKEN_KEY)
  assert(not token or token == intent.Token, "recalled Hero belongs to another transaction")
  intent.CandidateID = unit:GetID()
  Save(state)
  if not token then unit:SetProperty(R.TOKEN_KEY, intent.Token) end
  local record = R.Copy(intent.Record)
  record.FormerUnit = R.Copy(intent.Record.Unit)
  record.Unit = { Owner = intent.Owner, ID = unit:GetID(), Token = intent.Token }
  record.RecallHistory = record.RecallHistory or {}
  record.RecallHistory[#record.RecallHistory + 1] = { ID = unit:GetID(), Token = intent.Token, Turn = intent.Turn }
  local roster = GetRoster(intent.Owner)
  assert(R.Equal(roster[intent.RecordID], intent.Record) or R.Equal(roster[intent.RecordID], record),
    "native Hero person record changed during recall")
  unit:SetProperty(R.PERSON_KEY, R.Copy(record))
  roster[intent.RecordID] = R.Copy(record)
  Players[intent.Owner]:SetProperty(R.ROSTER_KEY, roster)
  tx.RecallRecords = tx.RecallRecords or {}
  tx.HeroRecallUnits = tx.HeroRecallUnits or {}
  tx.RecallRecords[intent.RecordID] = R.Copy(record)
  for _, snap in ipairs(tx.Units) do
    if snap.Hero and snap.Hero.Class == intent.Class then
      tx.HeroRecallUnits[snap.ID] = { ID = unit:GetID(), Token = intent.Token }
    end
  end
  intent.Status = "COMPLETE"
  Save(state)
  return R.Copy(record)
end

function A.Resume(id)
  local state = State()
  local tx = assert(state.Transactions[id], "unknown absorption transaction")
  if tx.Status == "COMPLETE" then return R.Copy(tx) end
  assert(tx.Turn == Game.GetCurrentGameTurn(), "resume in the original turn; later-turn recovery needs review")
  if tx.AwaitingPreparation then
    if not tx.SourceSpreadReads or not tx.SourceReadyToken then
      state.SourceReadSerial = (state.SourceReadSerial or 0) + 1
      tx.SourceSpreadReads = SourceSpreadReads(tx.ID, tx.From, tx.Turn, state.SourceReadSerial)
      tx.SourceReadyToken = tx.ID .. ":source-ready:" .. state.SourceReadSerial
      Save(state)
      return R.Copy(tx)
    end
    -- Preparation has no mutations. Retry its donor snapshot after the UI reaches this turn.
    state.Transactions[id] = nil
    local ready, prepared = pcall(Prepare, tx.Request, state, tx.SourceSpreadReads, tx.SourceReadyToken)
    state.Transactions[id] = tx
    if not ready then
      if tostring(prepared) == "JAC_ABSORPTION_WAITING_UI" then return R.Copy(tx) end
      -- Prepare is read-only. An invalid request must not reserve the factions
      -- or prevent a corrected request with the same ID from being submitted.
      state.Transactions[id] = nil
      Save(state)
      error(prepared)
    end
    tx = prepared
    state.Transactions[id] = tx
    Save(state)
  end
  for name in pairs(tx.Adapters) do assert(adapters[name], "missing absorption adapter: " .. name) end
  local ok, result = pcall(Execute, state, tx)
  if not ok then
    if tostring(result) == "JAC_ABSORPTION_WAITING_UI" then
      tx.Status, tx.Error = "WAITING_UI", nil
      Save(state)
      return R.Copy(tx)
    end
    tx.Status, tx.Error = "BLOCKED", tostring(result)
    Save(state)
    Log("blocked id=" .. id .. " reason=" .. tx.Error)
    error(tx.Error)
  end
  return result
end

function A.Apply(plan)
  assert(type(plan) == "table", "absorption plan required")
  local state = State()
  local existing = state.Transactions[plan.ID]
  if existing then
    assert(R.Equal(existing.Request, plan), "transaction ID reused with a different plan")
    return A.Resume(plan.ID)
  end
  local ready, tx = pcall(Prepare, plan, state)
  if not ready then
    if tostring(tx) ~= "JAC_ABSORPTION_WAITING_UI" then error(tx) end
    tx = { ID = plan.ID, From = plan.From, To = plan.To, Turn = Game.GetCurrentGameTurn(),
      Request = R.Copy(plan), Status = "WAITING_UI", AwaitingPreparation = true, Units = {}, Adapters = {} }
    state.SourceReadSerial = (state.SourceReadSerial or 0) + 1
    tx.SourceSpreadReads = SourceSpreadReads(tx.ID, tx.From, tx.Turn, state.SourceReadSerial)
    tx.SourceReadyToken = tx.ID .. ":source-ready:" .. state.SourceReadSerial
    state.Transactions[plan.ID] = tx
    Save(state)
    return R.Copy(tx)
  end
  state.Transactions[plan.ID] = tx
  Save(state)
  return A.Resume(plan.ID)
end

local function VerificationNeedsUI(tx, exact)
  for _, snap in ipairs(tx.Cities) do
    local city = CityManager.GetCityAt(snap.X, snap.Y)
    if city and CityNeedsUI(city) then return true end
  end
  local native = Game.GetHeroesManager and Game.GetHeroesManager()
  for _, snap in ipairs(tx.Units) do
    if snap.Hero and snap.KeepUnit and (not native or not native.GetHeroClaimPlayer or not native.GetHeroOriginCityID) then return true end
    if exact and snap.KeepUnit then
      local unit = FindToken(snap.Destination, snap.Token)
      if unit and not unit.GetSpreadCharges then return true end
    end
  end
  return false
end

function A.RequestVerification(id)
  local state = State()
  local tx = assert(state.Transactions[id], "unknown absorption transaction")
  assert(tx.Status == "COMPLETE", "absorption did not complete: " .. tx.Status)
  if not VerificationNeedsUI(tx, true) then return nil end
  state.SourceReadSerial = (state.SourceReadSerial or 0) + 1
  tx.VerificationUI = { Token = id .. ":verify:" .. state.SourceReadSerial, Turn = Game.GetCurrentGameTurn() }
  Save(state)
  return R.Copy(tx.VerificationUI)
end

-- RequestVerification -> UI publication -> Verify. The caller must not mutate
-- assets between the request and its check; every published request is consumed.
-- Exact=true is for the same-turn save/load check. Later turns can legitimately change HP, XP, followers etc.
function A.Verify(id, exact)
  local state = State()
  local tx = assert(state.Transactions[id], "unknown absorption transaction")
  assert(tx.Status == "COMPLETE", "absorption did not complete: " .. tx.Status)
  if VerificationNeedsUI(tx, exact == true) then
    local request, reads = tx.VerificationUI, ExposedMembers.JAC_HeroReadAPI
    assert(request and request.Turn == Game.GetCurrentGameTurn() and reads and reads.GetObservation
      and reads.GetObservation(request.Token), "verification requires a published UI request")
    tx.VerificationUI = nil
    Save(state)
  end
  Verify(tx, state, exact == true)
  return true
end

ExposedMembers.JAC_Absorption = A -- gameplay/FireTuner API; no UI mutation or automatic diplomatic hook.
Log("loaded version=1 explicit_dispositions=true")
