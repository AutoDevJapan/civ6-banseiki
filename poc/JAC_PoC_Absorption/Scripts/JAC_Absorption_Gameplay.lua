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
  state.Recoveries = state.Recoveries or {}
  state.RecoveryRecalls = state.RecoveryRecalls or {}
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
A.GetPropertyKeys = function()
  return { PERSON_KEY = R.PERSON_KEY, TOKEN_KEY = R.TOKEN_KEY }
end
A.GetUIPhase = function() return R.Copy(Game:GetProperty("JAC_ABSORPTION_FIXTURE_UI_WAIT")) end
local function CityRead(city, method, ...)
  local reads = ExposedMembers.JAC_HeroReadAPI
  assert(reads and reads.ReadCity, "UI city read API unavailable")
  return reads.ReadCity(city:GetOwner(), city:GetID(), method, ...)
end
local function ValidLoyalty(value)
  assert(type(value) == "number" and value == value and value >= 0 and value <= 100,
    "native city loyalty is not a 0..100 number")
  return value
end
local function CoreCityLoyalty(city)
  local identity = city.GetCulturalIdentity and city:GetCulturalIdentity()
  if identity and identity.GetLoyalty then return ValidLoyalty(identity:GetLoyalty()) end
  return nil
end
-- Gameplay has ChangeLoyalty, but the installed Core binding has no loyalty
-- getter. The UI city cache has the official getter and publishes scalar reads.
function A.CityLoyalty(city)
  return CoreCityLoyalty(city) or ValidLoyalty(CityRead(city, "LOYALTY"))
end
local function RequestLoyaltyRead(city, governance, phase)
  if CoreCityLoyalty(city) ~= nil then return end
  local request = governance.LoyaltyRead
  local turn = Game.GetCurrentGameTurn()
  if request and request.Phase == phase and request.Turn == turn then return end
  governance.LoyaltyRead = { Phase = phase, Owner = city:GetOwner(), ID = city:GetID(),
    Turn = turn, Token = governance.ID .. ":city:"
      .. city:GetX() .. ":" .. city:GetY() .. ":" .. turn .. ":" .. phase }
  city:SetProperty(R.GOVERNANCE_KEY, governance)
end
local function PublishedLoyalty(city, governance, phase)
  local direct = CoreCityLoyalty(city)
  if direct ~= nil then return direct end
  RequestLoyaltyRead(city, governance, phase)
  local request = governance.LoyaltyRead
  local reads = ExposedMembers.JAC_HeroReadAPI
  local observed = reads and reads.GetObservation and reads.GetObservation(request.Token)
  if not observed then return nil end
  assert(observed.Owner == request.Owner and observed.ID == request.ID
    and observed.Turn == request.Turn and city:GetOwner() == request.Owner
    and city:GetID() == request.ID, "city loyalty observation identity changed")
  return ValidLoyalty(observed.Value)
end
local function AdvanceLoyalty(city, governance, pending, phase, delta)
  if pending.Before == nil then
    local before = PublishedLoyalty(city, governance, phase .. ":before")
    if before == nil then return false end
    pending.Before, pending.Target = before, math.max(0, math.min(100, before + delta))
    city:SetProperty(R.GOVERNANCE_KEY, governance)
  end
  if not pending.Issued and pending.Before ~= pending.Target then
    -- Both the intent and a distinct after-read request exist before the
    -- native setter. A crash after the call can reconcile without a new call.
    pending.Issued = true
    RequestLoyaltyRead(city, governance, phase .. ":after:0")
    city:SetProperty(R.GOVERNANCE_KEY, governance)
    city:ChangeLoyalty(delta)
  end
  if pending.Before == pending.Target then return true end
  local suffix = tostring(pending.Rechecks or 0)
  local current = PublishedLoyalty(city, governance, phase .. ":after:" .. suffix)
  if current == nil then return false end
  if current == pending.Target then return true end
  assert(current == pending.Before, "city loyalty changed during interrupted adjustment")
  if (pending.Rechecks or 0) >= 1 then
    error("city loyalty change was not observed after two fresh publications")
  end
  pending.Rechecks = 1
  RequestLoyaltyRead(city, governance, phase .. ":after:1")
  city:SetProperty(R.GOVERNANCE_KEY, governance)
  return false
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
  local districts = assert(city:GetDistricts(), "city district collection unavailable")
  result.Districts = {}
  local locations = {}
  if districts.Members then
    for _, district in districts:Members() do
      locations[#locations + 1] = { Type = GameInfo.Districts[district:GetType()].DistrictType,
        X = district:GetX(), Y = district:GetY() }
    end
  else
    locations = CityRead(city, "DISTRICTS")
  end
  for _, location in ipairs(locations) do
    local key = location.Type .. ":" .. location.X .. ":" .. location.Y
    assert(not result.Districts[key], "duplicate district position")
    result.Districts[key] = location
  end
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

-- Older Version 1 transactions did not capture districts (or captured one
-- location per type). Validate only their recorded coverage; never backfill
-- unknown historical districts from today's city and claim they were saved.
local function RecordedDistricts(assets)
  local result = {}
  for key, position in pairs(assets.Districts or {}) do
    local location = { Type = position.Type or key, X = position.X, Y = position.Y }
    result[location.Type .. ":" .. location.X .. ":" .. location.Y] = location
  end
  return result
end

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

function A.OwnedGreatWorkSlots(owner)
  local works, seen = {}, {}
  for _, city in Players[owner]:GetCities():Members() do
    local buildings = city:GetBuildings()
    for info in GameInfo.Buildings() do
      if buildings:HasBuilding(info.Index) then
        for slot = 0, WorkSlots(city, info.Index) - 1 do
          local index = WorkInSlot(city, info.Index, slot)
          if index and index >= 0 then
            assert(not seen[index], "native Great Work occupies multiple source slots")
            seen[index] = true
            local workType = buildings.GetGreatWorkTypeFromIndex
              and buildings:GetGreatWorkTypeFromIndex(index)
            local reads = ExposedMembers.JAC_HeroReadAPI
            local published = reads and reads.ReadNativeWork
              and reads.ReadNativeWork(owner, city:GetID(), index)
            workType = workType or published and published.Type
            local definition = workType and GameInfo.GreatWorks[workType]
            local provenance = Game.GetGreatWorkDataFromIndex
              and Game.GetGreatWorkDataFromIndex(index)
            local creator = Game.GetGreatWorkPlayer and Game.GetGreatWorkPlayer(index)
            provenance = published or provenance
            creator = published and published.CreatedByPlayer or creator
            local effects = {}
            if definition and GameInfo.GreatWork_YieldChanges then
              for row in GameInfo.GreatWork_YieldChanges() do
                if row.GreatWorkType == definition.GreatWorkType then
                  effects[#effects + 1] = row.YieldType .. " " .. tostring(row.YieldChange)
                end
              end
            end
            if definition and definition.Tourism and definition.Tourism ~= 0 then
              effects[#effects + 1] = "TOURISM " .. tostring(definition.Tourism)
            end
            table.sort(effects)
            works[#works + 1] = { Index = index, Owner = owner, CityID = city:GetID(),
              X = city:GetX(), Y = city:GetY(), Building = info.Index,
              BuildingType = info.BuildingType, Slot = slot,
              GreatWorkType = definition and definition.GreatWorkType,
              ObjectType = definition and definition.GreatWorkObjectType,
              Name = definition and definition.Name,
              CreatorName = provenance and provenance.CreatorName ~= ""
                and provenance.CreatorName or nil,
              TurnCreated = provenance and provenance.TurnCreated,
              CreatedByPlayer = creator, Effects = effects }
          end
        end
      end
    end
  end
  table.sort(works, function(a, b) return a.Index < b.Index end)
  return works
end
local function OwnedGreatWorks(owner)
  local works = {}
  for _, work in ipairs(A.OwnedGreatWorkSlots(owner)) do works[work.Index] = true end
  return works
end
A.OwnedGreatWorks = OwnedGreatWorks

local function NativeWorkLocations(index)
  assert(type(index) == "number" and index >= 0 and index == math.floor(index),
    "native Great Work index required")
  local owners, seen, found = {}, {}, {}
  for _, owner in ipairs(PlayerManager.GetAliveIDs()) do
    if not seen[owner] then owners[#owners + 1], seen[owner] = owner, true end
  end
  local free = PlayerManager.GetFreeCitiesPlayerID and PlayerManager.GetFreeCitiesPlayerID()
  if type(free) == "number" and free >= 0 and Players[free] and not seen[free] then
    owners[#owners + 1] = free
  end
  table.sort(owners)
  for _, owner in ipairs(owners) do
    for _, work in ipairs(A.OwnedGreatWorkSlots(owner)) do
      if work.Index == index then found[#found + 1] = work end
    end
  end
  return found
end
A.NativeWorkLocations = NativeWorkLocations

local function SameNativeWorkIdentity(source, current)
  for _, key in ipairs({ "Index", "GreatWorkType", "ObjectType", "CreatorName",
      "TurnCreated", "CreatedByPlayer" }) do
    if source[key] ~= nil then
      assert(source[key] == current[key], "native Great Work metadata changed: " .. key)
    end
  end
  if source.Effects ~= nil then
    assert(R.Equal(source.Effects, current.Effects), "native Great Work effects changed")
  end
end

local function SameNativeWorkSlot(work, slot)
  return work.Owner == slot.Owner and work.CityID == (slot.City or slot.CityID)
    and work.X == slot.X and work.Y == slot.Y
    and work.Building == slot.Building and work.Slot == slot.Slot
end

local function NativeWorkCustodian(destination)
  if destination >= 0 then return destination end
  local free = PlayerManager.GetFreeCitiesPlayerID and PlayerManager.GetFreeCitiesPlayerID()
  assert(type(free) == "number" and free >= 0, "Free Cities custody owner unavailable")
  return free
end

local function NativeWorkDestination(owner, source, reserved, emptyCityOnly, reservedCities)
  assert(type(source.ObjectType) == "string" and source.ObjectType ~= "",
    "native Great Work object type unavailable")
  local reads = ExposedMembers.JAC_HeroReadAPI
  assert(reads and type(reads.FindCompatibleNativeSlots) == "function",
    "native Great Work destination slot reader unavailable")
  local ok, slots = pcall(reads.FindCompatibleNativeSlots, owner, source.ObjectType)
  if not ok then error("JAC_ABSORPTION_WAITING_UI", 0) end
  assert(type(slots) == "table", "native Great Work destination slot list unavailable")
  local valid = {}
  for row in GameInfo.GreatWork_ValidSubTypes() do
    if row.GreatWorkObjectType == source.ObjectType then valid[row.GreatWorkSlotType] = true end
  end
  local occupiedCities = {}
  local movableBuildings = {}
  if emptyCityOnly then
    for _, work in ipairs(A.OwnedGreatWorkSlots(owner)) do
      occupiedCities[work.X .. ":" .. work.Y] = true
    end
    for info in GameInfo.Buildings() do
      if info.BuildingType ~= "BUILDING_PALACE" then movableBuildings[info.Index] = true end
    end
  end
  for _, slot in ipairs(slots) do
    local key = slot.Owner .. ":" .. slot.City .. ":" .. slot.Building .. ":" .. slot.Slot
    if not reserved[key] and not occupiedCities[slot.X .. ":" .. slot.Y]
      and not (reservedCities and reservedCities[slot.X .. ":" .. slot.Y])
      and (not emptyCityOnly or movableBuildings[slot.Building]) then
      local city = CityManager.GetCityAt(slot.X, slot.Y)
      assert(city and city:GetOwner() == owner and city:GetID() == slot.City
        and slot.Owner == owner, "native Great Work destination city changed")
      assert(city:GetBuildings():HasBuilding(slot.Building)
        and type(slot.Slot) == "number" and slot.Slot >= 0
        and slot.Slot < WorkSlots(city, slot.Building)
        and WorkInSlot(city, slot.Building, slot.Slot) == -1
        and valid[slot.SlotType], "native Great Work destination slot changed")
      reserved[key] = true
      return R.Copy(slot)
    end
  end
  error("native Great Work has no compatible empty destination slot")
end
local function ReservedNativeSlots(state, owner, excludingTransaction, excludingRecovery)
  local reserved = {}
  local function Add(slot)
    if slot and slot.Owner == owner then
      reserved[slot.Owner .. ":" .. slot.City .. ":" .. slot.Building .. ":" .. slot.Slot] = true
    end
  end
  for id, tx in pairs(state.Transactions or {}) do
    if id ~= excludingTransaction and tx.Status ~= "COMPLETE" then
      for _, work in ipairs(tx.NativeWorks or {}) do Add(work.TargetSlot) end
    end
  end
  for token, intent in pairs(state.Recoveries or {}) do
    if token ~= excludingRecovery and intent.Status ~= "COMPLETE" then Add(intent.NativeTarget) end
  end
  return reserved
end
local function ReservedNativeCarrierCities(state, excludingTransaction)
  local reserved = {}
  local function Add(slot)
    if slot then reserved[slot.X .. ":" .. slot.Y] = true end
  end
  for id, tx in pairs(state.Transactions or {}) do
    if id ~= excludingTransaction and tx.Status ~= "COMPLETE" then
      for _, work in ipairs(tx.NativeWorks or {}) do
        if work.Destination and work.Destination < 0 then
          Add(work.Carrier or work.TargetSlot)
        end
      end
    end
  end
  for _, intent in pairs(state.Recoveries or {}) do
    if intent.Status ~= "COMPLETE" then Add(intent.NativeCarrier) end
  end
  return reserved
end
local NativeWorkAtTarget, NativeWorkAtAbsorber, NativeWorkSourceEmpty

-- Read-only game-core probe. The UI's MOVE_GREAT_WORK request proves only the
-- normal same-owner drag operation; native setters need real runtime evidence.
function A.NativeWorkCapabilities(owner)
  local player = assert(Players[owner], "native-work probe player missing")
  local city = assert(player:GetCities():GetCapitalCity(), "native-work probe city missing")
  local objects = { Game = Game, CityBuildings = city:GetBuildings(), City = city,
    CityManager = CityManager, CityCollection = player:GetCities(),
    PlayerCulture = player:GetCulture() }
  local result = { Owner = owner, CityID = city:GetID(), Methods = {},
    Operations = {}, NativeSlots = A.OwnedGreatWorkSlots(owner) }
  if Game.GetCulture then
    local ok, culture = pcall(Game.GetCulture)
    if not ok or culture == nil then ok, culture = pcall(Game.GetCulture, Game) end
    if ok and culture then objects.GameCulture = culture end
    result.GameCultureRead = ok and culture ~= nil
  end
  local names = { "CreateGreatWork", "CreateGreatWorkInSlot", "DestroyGreatWork",
    "RemoveGreatWork", "DeleteGreatWork", "MoveGreatWork", "TransferGreatWork",
    "SetGreatWork", "SetGreatWorkInSlot", "SetGreatWorkPlayer",
    "GetGreatWorkInSlot", "GetGreatWorkSlotType", "GetNumGreatWorkSlots",
    "GetGreatWorkTypeFromIndex", "GetGreatWorkDataFromIndex",
    "GetGreatWorkPlayer", "GetGreatWorkType", "GetGreatWorkTypeFromIndex",
    "GetGreatWorksInCity", "CreateCity", "TransferCity", "DestroyCity",
    "Create", "Destroy" }
  for label, object in pairs(objects) do
    local available = {}
    for _, name in ipairs(names) do
      local ok, value = pcall(function() return object[name] end)
      if ok and type(value) == "function" then available[#available + 1] = name end
    end
    result.Methods[label] = available
  end
  if PlayerOperations then
    for _, name in ipairs({ "MOVE_GREAT_WORK", "PARAM_PLAYER_ONE", "PARAM_CITY_SRC",
      "PARAM_CITY_DEST", "PARAM_BUILDING_SRC", "PARAM_BUILDING_DEST",
      "PARAM_GREAT_WORK_INDEX", "PARAM_SLOT" }) do
      if PlayerOperations[name] ~= nil then result.Operations[name] = tostring(PlayerOperations[name]) end
    end
  end
  return result
end

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

-- Audit the complete charge inventory from Units and Units_MODE, rather than
-- extending an exception list one lost counter at a time. Only the snapshot
-- adapters below preserve counters; everything else must be zero even when
-- a modifier adds charges that the unit definition does not declare.
local unitCounterPolicies = {
  { Field = "BuildCharges", Adapter = "SNAPSHOT" },
  { Field = "SpreadCharges", Adapter = "SNAPSHOT" },
  { Field = "ActionCharges", Adapter = "HERO" },
  { Field = "ReligiousHealCharges" },
  { Field = "DisasterCharges" },
  { Field = "ParkCharges" },
}
local function AssertSupportedUnitState(unit)
  local info = assert(GameInfo.Units[unit:GetType()], "unknown unit type")
  local nativeHero = unit.IsHero and unit:IsHero()
  for _, policy in ipairs(unitCounterPolicies) do
    if policy.Adapter ~= "SNAPSHOT" and not (policy.Adapter == "HERO" and nativeHero) then
      local name = policy.Field
      local mode = name == "ActionCharges" and GameInfo.Units_MODE and GameInfo.Units_MODE[info.UnitType]
      assert((info[name] or 0) == 0 and (not mode or (mode[name] or 0) == 0),
        "unsupported unit charge counter: " .. name .. ":" .. info.UnitType)
      local getter = unit["Get" .. name]
      if getter then
        local value = getter(unit)
        assert(type(value) == "number" and value == 0,
          "unsupported remaining unit charges: " .. name .. ":" .. info.UnitType)
      end
    end
  end
  -- RockBand level and album sales are separate native state, not XP or
  -- charges. No verified restoration adapter exists, including for a band
  -- that has not played yet. Cover custom definitions using its class/tag.
  local rockBand = info.UnitType == "UNIT_ROCK_BAND" or info.PromotionClass == "PROMOTION_CLASS_ROCK_BAND"
  if not rockBand and GameInfo.TypeTags then
    for row in GameInfo.TypeTags() do
      if row.Type == info.UnitType and row.Tag == "CLASS_ROCK_BAND" then rockBand = true; break end
    end
  end
  assert(not rockBand, "unsupported unit state: RockBand:" .. info.UnitType)
end

local function UnitAbilityCounts(unit)
  local result = {}
  local ability = unit.GetAbility and unit:GetAbility()
  if ability and ability.GetAbilityCount and GameInfo.UnitAbilities then
    for row in GameInfo.UnitAbilities() do
      local count = ability:GetAbilityCount(row.UnitAbilityType)
      if count and count > 0 then result[row.UnitAbilityType] = count end
    end
  end
  return result
end

function A.UnitAbilityLabels(unit, record)
  local result, seen = {}, {}
  local function Add(value)
    if type(value) == "table" then
      if value.Name then Add(value.Name)
      else for _, part in ipairs(value) do Add(part) end end
    elseif type(value) == "string" and value ~= "" and not seen[value] then
      seen[value] = true
      result[#result + 1] = value
    end
  end
  if record then Add(record.Abilities or record.Ability) end
  for _, name in ipairs(R.Keys(UnitAbilityCounts(unit))) do
    local row = GameInfo.UnitAbilities[name]
    Add(row and (row.Description or row.Name) or name)
  end
  local gp = unit:GetGreatPerson()
  local individual = gp and gp:GetIndividual() or -1
  local definition = individual >= 0 and GameInfo.GreatPersonIndividuals[individual]
  if definition then Add(definition.ActionEffectTextOverride or definition.BirthEffectTextOverride) end
  return result
end

local function SnapshotUnit(unit, publishedSpread)
  AssertSupportedUnitState(unit)
  local info = assert(GameInfo.Units[unit:GetType()], "unknown unit type")
  local hero = SnapshotHero(unit, info)
  local greatPerson = unit:GetGreatPerson()
  local individual = greatPerson and greatPerson:GetIndividual() or -1
  local person = unit:GetProperty(R.PERSON_KEY)
  assert(not hero or not person or person.Kind == "HERO", "native Hero/person kind mismatch")
  local kind = R.UnitKind(unit)
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
    Name = unit:GetName(), AbilityCounts = UnitAbilityCounts(unit),
  }
end
A.SnapshotUnit = SnapshotUnit

-- A scattered individual is recreated only after a real world reward. There
-- is no documented setter for spent native Great Person/build/spread charges;
-- reject that case while the donor is still intact, before A.Apply can kill it.
function A.AssertRecoverableEscape(owner, unitID)
  local unit = assert(UnitManager.GetUnit(owner, unitID), "escaping individual vanished")
  AssertSupportedUnitState(unit)
  if unit.IsHero and unit:IsHero() then return true end -- Hero action charges have a verified setter.
  local info = assert(GameInfo.Units[unit:GetType()], "escaping unit definition missing")
  local greatPerson = unit:GetGreatPerson()
  local individual = greatPerson and greatPerson:GetIndividual() or -1
  if individual >= 0 then
    local definition = assert(GameInfo.GreatPersonIndividuals[individual],
      "native Great Person definition missing")
    assert(greatPerson.GetActionCharges and greatPerson:GetActionCharges() == definition.ActionCharges,
      "spent native Great Person charges cannot be restored after escape")
  end
  assert((unit:GetBuildCharges() or 0) == (info.BuildCharges or 0),
    "spent build charges cannot be restored after escape")
  if (info.SpreadCharges or 0) > 0 then
    assert(unit.GetSpreadCharges and unit:GetSpreadCharges() == info.SpreadCharges,
      "spent or UI-only spread charges cannot be restored after escape")
  end
  return true
end

local function DestinationOwner(kind, outcome, decision, tx)
  local destination = R.Destination(kind, outcome)
  if destination == "ABSORBER" then return tx.To end
  if destination == "RETURN" then
    local id = decision.ReturnOwner
    assert(type(id) == "number" and id ~= tx.From and id ~= tx.To and Players[id]
      and Players[id]:IsAlive(), "RETURN requires a surviving third-party ReturnOwner")
    if kind == "ITEM" then
      assert(Players[id]:GetCities():GetCount() > 0, "returned item needs a recipient city")
    end
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
    or not city:GetDistricts().Members
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
  if plan.FlowPolicy and R.CITY_POLICY[plan.Mode].Immediate ~= 0 then
    for _, city in source:GetCities():Members() do
      assert(city.ChangeLoyalty, "native city loyalty setter unavailable")
      if CoreCityLoyalty(city) == nil then
        local reads = ExposedMembers.JAC_HeroReadAPI
        assert(reads and reads.GetObservation,
          "published UI city loyalty reads unavailable")
      end
    end
  end
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
    SourceRoster = GetRoster(plan.From), TargetRoster = GetRoster(plan.To), Decisions = {}, HeroResults = {},
    NativeWorks = {}, Adapters = {} }
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
    if snap.Kind == "PERSON" then
      local unitPerson = snap.Properties[R.PERSON_KEY]
      if unitPerson and unitPerson.Kind ~= "PERSON" then
        assert(unitPerson.Kind == "UNIT" or unitPerson.Kind == "GREAT_PERSON",
          "native person's Unit record has an incompatible kind")
        unitPerson.FormerKind, unitPerson.Kind = unitPerson.Kind, "PERSON"
      end
      for _, asset in pairs(tx.SourceRoster) do
        if asset.Unit and asset.Unit.Owner == tx.From and asset.Unit.ID == snap.ID
          and asset.Kind ~= "PERSON" then
          assert(asset.Kind == "UNIT" or asset.Kind == "GREAT_PERSON",
            "native person's roster record has an incompatible kind")
          asset.FormerKind, asset.Kind = asset.Kind, "PERSON"
        end
      end
    end
    local decision = Decision((plan.Units or {})[R.UnitKey(plan.From, snap.ID)])
    if snap.Kind == "PERSON" and decision.Outcome == "INCORPORATE"
      and (snap.Type == "UNIT_GREAT_GENERAL"
        or snap.Properties.JAC_POC_GROWTHUI_WARLORD) then
      -- Older direct PoC plans used the unit disposition before these native
      -- people were classified. Preserve that request as a person recruitment.
      decision.RequestedOutcome, decision.Outcome = "INCORPORATE", "RECRUIT"
    end
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
      elseif decision.Outcome ~= "DEFECT" and decision.Outcome ~= "HIDE" then
        HeroSuicide(snap, decision.Outcome == "SUICIDE" and "EXPLICIT_SUICIDE" or "REQUESTED_OUTCOME_UNTRANSFERABLE")
      end
    end
    if snap.Kind ~= "UNIT" or snap.Outcome == "ESCAPE" then
      local linked = false
      for _, asset in pairs(tx.SourceRoster) do
        if asset.Unit and asset.Unit.Owner == tx.From and asset.Unit.ID == snap.ID then linked = true end
      end
      if not linked then
        local person = snap.Properties[R.PERSON_KEY]
        local id = person and person.ID or (snap.Kind == "UNIT"
          and ("unit:" .. tx.From .. ":" .. snap.ID)
          or ("native-person:" .. tx.From .. ":" .. snap.ID))
        assert(not tx.SourceRoster[id], "native person ID collision")
        tx.SourceRoster[id] = person and R.Copy(person) or {
          ID = id, Kind = snap.Kind, Owner = tx.From, NativeIndividual = snap.Individual,
          UnitType = snap.Type, Abilities = A.UnitAbilityLabels(unit),
          Warlord = R.Copy(snap.Properties.JAC_POC_GROWTHUI_WARLORD) }
        if not person and tx.SourceRoster[id].Warlord then
          local warlord = tx.SourceRoster[id].Warlord
          tx.SourceRoster[id].Personality = warlord.Personality
          tx.SourceRoster[id].Trait = warlord.Trait
          tx.SourceRoster[id].Equipment = R.Copy(warlord.Equipment)
        end
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
  local seenNative = {}
  for _, work in ipairs(A.OwnedGreatWorkSlots(plan.From)) do
    local id = "native-work:" .. work.Index
    seenNative[id] = true
    local decision = Decision((plan.NativeWorks or {})[work.Index] or "TRANSFER")
    local destination = DestinationOwner("ITEM", decision.Outcome, decision, tx)
    local locations = NativeWorkLocations(work.Index)
    assert(#locations == 1 and SameNativeWorkSlot(locations[1], work),
      "native Great Work is not unique at the donor source: " .. id)
    local custodian
    local nativeDealReady = DealManager and type(DealManager.EnactWorkingDeal) == "function"
      and type(DealManager.GetWorkingDeal) == "function"
      and type(DealManager.GetPossibleDealItems) == "function"
    if destination < 0 then
      custodian = NativeWorkCustodian(destination)
    elseif destination ~= tx.To then
      assert(nativeDealReady, "native Great Work Core deal API unavailable before city transfer: " .. id)
      custodian = destination
    elseif work.BuildingType == "BUILDING_PALACE" and nativeDealReady then
      custodian = tx.To -- escrow before the engine can replace the old capital's Palace
    end
    local asset = tx.SourceRoster[id] or { ID = id, Kind = "ITEM", Owner = tx.From }
    assert(asset.Kind == "ITEM" and asset.Owner == tx.From
      and (not asset.NativeWork or asset.NativeWork.Index == work.Index),
      "native Great Work roster identity mismatch: " .. id)
    if asset.NativeWork then SameNativeWorkIdentity(asset.NativeWork, work) end
    asset.NativeWork, asset.Type = R.Copy(work), work.ObjectType or "NATIVE_GREAT_WORK"
    asset.Name = asset.Name or work.Name or work.GreatWorkType or id
    asset.City = { Owner = tx.From, X = work.X, Y = work.Y }
    asset.NativeCustody = { Owner = tx.From, City = work.CityID,
      X = work.X, Y = work.Y, Building = work.Building, Slot = work.Slot }
    tx.SourceRoster[id] = asset
    generatedDecisions[id] = decision
    tx.NativeWorks[#tx.NativeWorks + 1] = { ID = id, Source = R.Copy(work),
      Outcome = decision.Outcome, Destination = destination,
      Custodian = custodian }
  end
  for id, asset in pairs(tx.SourceRoster) do
    assert(not asset.NativeWork or seenNative[id],
      "tracked native Great Work is absent from source city slots: " .. id)
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
      if (heroSnap and heroSnap.SuicideCause)
        or (dormant[id] and d.Outcome ~= "HIDE" and d.Outcome ~= "DEFECT") then
        if dormant[id] then
          if (plan.Assets or {})[id] then d.RequestedOutcome = d.Outcome end
        else
          d.RequestedOutcome = heroSnap.RequestedOutcome
        end
        d.Outcome, d.Owner = "SUICIDE", -1
        -- There is no claim-only setter in HeroesManager. A dormant right
        -- cannot be verified as inherited by creating no native Hero unit.
        d.SuicideCause = dormant[id] and "DORMANT_CLAIM_SETTER_UNAVAILABLE" or heroSnap.SuicideCause
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
      Faith = Religion(city), Assets = CityAssets(city), AssetCoverageVersion = 2, PalaceWorks = PalaceWorks(city),
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
  local turn = Game.GetCurrentGameTurn()
  local read = snap.SpreadReads and snap.SpreadReads[phase]
  if read and read.Verified then return true end
  local token = snap.Token .. ":spread:" .. phase .. ":" .. id .. ":" .. turn
  snap.SpreadReads = snap.SpreadReads or {}
  if not read or read.Token ~= token or read.Owner ~= owner or read.Turn ~= turn then
    read = { Token = token, Owner = owner, ID = id, Turn = turn, AwaitingUI = true }
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
  if snap.AbilityCounts and next(snap.AbilityCounts) then
    local ability = unit:GetAbility()
    assert(ability and ability.GetAbilityCount and ability.ChangeAbilityCount,
      "native Unit ability API unavailable after transfer")
    for _, name in ipairs(R.Keys(snap.AbilityCounts)) do
      local current = ability:GetAbilityCount(name) or 0
      if current ~= snap.AbilityCounts[name] then
        ability:ChangeAbilityCount(name, snap.AbilityCounts[name] - current)
      end
    end
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

local function VerifyUnit(unit, snap, coreOnly, allowEvolution)
  assert(unit and unit:GetOwner() == snap.Destination and GameInfo.Units[unit:GetType()].UnitType == snap.Type,
    "transferred unit missing/type/owner mismatch: " .. snap.Token)
  AssertSupportedUnitState(unit)
  assert(unit:GetX() >= 0 and unit:GetY() >= 0 and Map.GetPlot(unit:GetX(), unit:GetY()),
    "transferred unit is off-map: " .. snap.Token)
  assert(unit:GetProperty(R.TOKEN_KEY) == snap.Token, "unit token mismatch")
  local greatPerson = unit:GetGreatPerson()
  assert((greatPerson and greatPerson:GetIndividual() or -1) == snap.Individual, "great person individual changed")
  if snap.GreatPersonCharges and not allowEvolution then
    assert(greatPerson:GetActionCharges() == snap.GreatPersonCharges, "great person action charges changed")
  end
  if snap.Hero then VerifyHero(unit, snap, not allowEvolution) end
  if not coreOnly then
    assert(R.Equal(Properties(unit, snap.PropertyKeys), snap.TransferredProperties), "unit properties changed: " .. snap.Token)
  end
  if snap.Name and not allowEvolution then assert(unit:GetName() == snap.Name, "unit name changed") end
  if snap.Damage and not allowEvolution then assert(unit:GetDamage() == snap.Damage, "unit damage changed") end
  if snap.Moves and not allowEvolution then assert(unit:GetMovesRemaining() == snap.Moves, "unit movement changed") end
  if snap.XP and not allowEvolution then assert(unit:GetExperience():GetExperiencePoints() == snap.XP, "unit XP changed") end
  if snap.Formation and not allowEvolution then assert(unit:GetMilitaryFormation() == snap.Formation, "unit formation changed") end
  if snap.Religion and not allowEvolution then
    local religion = unit:GetReligion()
    assert((religion and religion:GetReligionType() or -1) == snap.Religion, "unit religion changed")
  end
  for _, promotion in ipairs(snap.Promotions or {}) do assert(unit:GetExperience():HasPromotion(promotion), "promotion lost") end
  if snap.AbilityCounts and next(snap.AbilityCounts) then
    local ability = assert(unit:GetAbility(), "native Unit ability API unavailable during verification")
    for _, name in ipairs(R.Keys(snap.AbilityCounts)) do
      assert(ability:GetAbilityCount(name) == snap.AbilityCounts[name],
        "native Unit ability lost: " .. name)
    end
  end
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
  if unit and snap.NativeRestored then
    assert(intent and intent.CreatedID == unit:GetID(), "staged Hero identity changed")
    VerifyHero(unit, snap, false)
    local ok, err = pcall(VerifySpread, unit, snap, state, tx, "hero-stage")
    if not ok then
      if tostring(err) == "JAC_ABSORPTION_WAITING_UI" then error(err, 0) end
      SetHeroSuicide(state, tx, snap, "HERO_RESTORE_ERROR", unit)
    end
    return
  end
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
  snap.CoreStagedTurn = Game.GetCurrentGameTurn()
  Save(state)
  -- A missing Core getter is not a Hero restoration failure. Keep the saved candidate
  -- while the UI observes it at the next game-core publication.
  local spreadOK, spreadError = pcall(VerifySpread, unit, snap, state, tx, "hero-stage")
  if not spreadOK then
    if tostring(spreadError) == "JAC_ABSORPTION_WAITING_UI" then error(spreadError, 0) end
    SetHeroSuicide(state, tx, snap, "HERO_RESTORE_ERROR", unit)
  end
end

local function MovedAssets(tx, normalCommitted)
  local sourceNow = GetRoster(tx.From)
  assert((tx.SourceRosterMoved and next(sourceNow) == nil) or
    (not tx.SourceRosterMoved and R.Equal(sourceNow, tx.OriginalSourceRoster)),
    "source roster changed during absorption; review new assets before clearing")
  -- Merge into current data: city hooks or a failed attempt may have added/updated unrelated assets.
  local rosters, archive = { [tx.To] = GetRoster(tx.To) }, {}
  local nativeByID = {}
  for _, snap in ipairs(tx.NativeWorks or {}) do nativeByID[snap.ID] = snap end
  for _, id in ipairs(R.Keys(tx.SourceRoster)) do
    local record = R.Rebind(tx.SourceRoster[id], tx)
    local decision = tx.Decisions[id]
    record.OriginalOwner = record.OriginalOwner or tx.From
    record.Owner, record.Outcome = decision.Owner, decision.Outcome
    local native = nativeByID[id]
    local physical
    if record.NativeWork and native then
      if native.Custodian then
        assert(native.PhysicalMoved, "external native work has not moved before roster update")
        physical = NativeWorkAtTarget(native)
      elseif normalCommitted or tx.SourceRosterMoved then
        physical = NativeWorkAtAbsorber(tx, native)
      else
        record.NativeWork.Owner = tx.To -- projection for the pre-transfer conflict check
      end
      if physical then
        record.NativeWork = R.Copy(physical)
        record.NativeCustody = { Owner = physical.Owner, City = physical.CityID,
          X = physical.X, Y = physical.Y, Building = physical.Building, Slot = physical.Slot }
        record.City = { Owner = physical.Owner, X = physical.X, Y = physical.Y }
      end
    end
    if record.Kind == "ITEM" and record.City and decision.Owner ~= tx.To then
      record.FormerCity = R.Copy(tx.SourceRoster[id].City or record.City)
      if decision.Owner < 0 then
        record.City = nil -- scattered: no live vault still claims this item
      elseif physical then
        record.City = { Owner = physical.Owner, X = physical.X, Y = physical.Y }
      else
        local capital = assert(Players[decision.Owner]:GetCities():GetCapitalCity(),
          "returned item has no recipient vault city")
        record.City = { Owner = decision.Owner, X = capital:GetX(), Y = capital:GetY() }
      end
    end
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
  local externalNative = {}
  for _, snap in ipairs(tx.NativeWorks or {}) do
    local work
    if snap.Custodian then
      assert(snap.PhysicalMoved, "external native Great Work was not physically transferred")
      work = NativeWorkAtTarget(snap)
      NativeWorkSourceEmpty(tx, snap)
      externalNative[snap.Source.Index] = true
    else
      work = NativeWorkAtAbsorber(tx, snap)
    end
    SameNativeWorkIdentity(snap.Source, work)
    assert(not snap.Source.GreatWorkType or not work.GreatWorkType
      or work.GreatWorkType == snap.Source.GreatWorkType,
      "native Great Work type changed during city transfer: " .. snap.ID)
    assert(snap.Source.CreatedByPlayer == nil
      or work.CreatedByPlayer == snap.Source.CreatedByPlayer,
      "native Great Work creator changed during city transfer: " .. snap.ID)
  end
  for _, snap in ipairs(tx.Cities) do
    local city = CityManager.GetCityAt(snap.X, snap.Y)
    assert(city and city:GetOwner() == tx.To, "city transfer failed")
    assert(R.Equal(Properties(city, snap.PropertyKeys), snap.TransferredProperties), "city properties changed")
    local current = CityAssets(city)
    assert(snap.AssetCoverageVersion == nil or snap.AssetCoverageVersion == 2, "unsupported city asset coverage version")
    if snap.AssetCoverageVersion == 2 then
      assert(type(snap.Assets.Districts) == "table", "district snapshot missing from current asset coverage")
    end
    for key, position in pairs(RecordedDistricts(snap.Assets)) do
      assert(R.Equal(current.Districts[key], position), "transferred city district lost/moved: " .. key)
    end
    if exactUnits then
      assert(city:GetName() == snap.Name, "city name changed")
      local expected = R.Copy(snap.Assets)
      for _, moved in ipairs(tx.NativeWorks or {}) do
        if moved.Custodian and moved.Source.X == snap.X and moved.Source.Y == snap.Y
          and moved.Source.BuildingType ~= "BUILDING_PALACE" then
          local building = assert(expected[moved.Source.BuildingType],
            "external native work source building disappeared from snapshot")
          assert(building.Works[moved.Source.Slot] == moved.Source.Index,
            "external native work source slot changed in snapshot")
          building.Works[moved.Source.Slot] = -1
        end
      end
      if snap.AssetCoverageVersion == 2 then expected.Districts = RecordedDistricts(snap.Assets)
      else current.Districts, expected.Districts = nil, nil end
      assert(R.Equal(current, expected), "city buildings/Great Works changed")
    else
      for buildingType, original in pairs(snap.Assets) do
        assert(current[buildingType], "transferred city building lost: " .. buildingType)
        for _, work in pairs(original.Works or {}) do
          if work and work >= 0 and not externalNative[work] then
            assert(greatWorks[work], "transferred Great Work lost: " .. work)
          end
        end
      end
    end
    if exactUnits then
      assert(city:GetPopulation() == snap.Population and R.Equal(Religion(city), snap.Faith), "city population/religion changed")
    end
    local governance = assert(city:GetProperty(R.GOVERNANCE_KEY), "city governance missing")
    assert(governance.From == tx.From, "governance provenance lost")
    assert(R.Equal(governance.Previous, snap.Governance), "previous governance lost")
    assert(governance.ImmediateApplied, "city loyalty policy is still pending")
    if exactUnits and governance.LoyaltyDelta and governance.LoyaltyDelta ~= 0 then
      assert(A.CityLoyalty(city) == governance.ImmediateTarget,
        "native city loyalty differs from applied policy")
    end
    for _, work in ipairs(snap.PalaceWorks or {}) do
      if not externalNative[work] then assert(greatWorks[work], "Palace Great Work lost: " .. work) end
    end
  end
  for _, index in ipairs(tx.Plots) do assert(Map.GetPlotByIndex(index):GetOwner() == tx.To, "territory transfer failed: " .. index) end
  local rosters, archive = MovedAssets(tx, true)
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

local function RestoreTransferred(state, tx, unit, snap, coreOnly)
  -- After a successful restore, later turns may legitimately change health,
  -- movement and lifespan. Never replay the original snapshot over those changes.
  local restoredTurn = snap.TransferRestoredTurn or snap.CoreStagedTurn
  if restoredTurn and restoredTurn < Game.GetCurrentGameTurn() then
    assert(unit:GetProperty(R.TOKEN_KEY) == snap.Token
      and unit:GetOwner() == snap.Destination
      and GameInfo.Units[unit:GetType()].UnitType == snap.Type,
      "restored absorption candidate identity changed")
    if not coreOnly and not snap.TransferRestoredTurn then
      WriteProperties(unit, snap.TransferredProperties, snap.PropertyKeys)
      snap.TransferRestoredTurn = restoredTurn
      Save(state)
    end
    return
  end
  RestoreUnit(unit, snap, coreOnly)
  VerifyUnit(unit, snap, coreOnly)
  if coreOnly then snap.CoreStagedTurn = Game.GetCurrentGameTurn()
  else
    snap.TransferRestoredTurn = Game.GetCurrentGameTurn()
    Save(state)
  end
end
NativeWorkAtTarget = function(snap)
  local found = NativeWorkLocations(snap.Source.Index)
  assert(#found == 1, "native Great Work index is missing or duplicated: " .. snap.ID)
  SameNativeWorkIdentity(snap.Source, found[1])
  assert(SameNativeWorkSlot(found[1], snap.TargetSlot),
    "native Great Work is not in its reserved physical destination: " .. snap.ID)
  return found[1]
end
NativeWorkAtAbsorber = function(tx, snap)
  local found = NativeWorkLocations(snap.Source.Index)
  assert(#found > 0, "transferred native Great Work index absent from absorber: " .. snap.ID)
  assert(#found == 1, "transferred native Great Work index is duplicated: " .. snap.ID)
  local work = found[1]
  SameNativeWorkIdentity(snap.Source, work)
  local city = assert(CityManager.GetCityAt(snap.Source.X, snap.Source.Y),
    "transferred native Great Work source city disappeared: " .. snap.ID)
  assert(city:GetOwner() == tx.To and work.Owner == tx.To,
    "transferred native Great Work index absent from absorber: " .. snap.ID)
  -- The engine can remove a conquered capital's Palace and place its work in
  -- the recipient capital. Other source buildings retain their original slot.
  if snap.Source.BuildingType ~= "BUILDING_PALACE" then
    assert(work.CityID == city:GetID() and work.X == snap.Source.X
      and work.Y == snap.Source.Y and work.Building == snap.Source.Building
      and work.Slot == snap.Source.Slot,
      "transferred native Great Work changed its saved physical slot: " .. snap.ID)
  end
  return work
end
NativeWorkSourceEmpty = function(tx, snap)
  local source = snap.Source
  local city = assert(CityManager.GetCityAt(source.X, source.Y),
    "native Great Work donor city disappeared: " .. snap.ID)
  assert(city:GetOwner() == tx.From or city:GetOwner() == tx.To,
    "native Great Work donor city changed owner unexpectedly: " .. snap.ID)
  if city:GetOwner() == tx.From then
    assert(city:GetID() == source.CityID,
      "native Great Work donor city changed identity: " .. snap.ID)
  end
  if city:GetBuildings():HasBuilding(source.Building) then
    assert(WorkInSlot(city, source.Building, source.Slot) == -1,
      "native Great Work donor slot was reused: " .. snap.ID)
  else
    assert(city:GetOwner() == tx.To and source.BuildingType == "BUILDING_PALACE",
      "native Great Work donor building disappeared: " .. snap.ID)
  end
end

local function BuildNativeWorkDeal(from, to, index, subtype, priorWorks)
  assert(DealManager and type(DealManager.EnactWorkingDeal) == "function",
    "native Great Work Core deal enactment unavailable")
  local working = assert(DealManager.GetWorkingDeal(DealDirection.OUTGOING, from, to),
    "native Great Work working deal unavailable")
  local count = working:GetItemCount()
  assert(count == 0 or count == 1, "native Great Work working deal contains another item")
  if count == 1 then
    assert(type(working.FindItemsByType) == "function",
      "native Great Work existing deal item cannot be identified")
    local current = working:FindItemsByType(DealItemTypes.GREATWORK, subtype, from)
    if current and #current == 1 and current[1]:GetValueType() == index then
      working:Validate()
      assert(working:IsValid(), "native Great Work deal did not validate")
      return working -- a saved intent may have stopped before its native call
    end
    local previous
    for _, snap in ipairs(priorWorks or {}) do
      if snap.PhysicalMoved and snap.Custodian == to then
        local definition = assert(GameInfo.GreatWorks[snap.Source.GreatWorkType],
          "previous native Great Work type unavailable")
        local items = working:FindItemsByType(DealItemTypes.GREATWORK, definition.Index, from)
        if items and #items == 1 and items[1]:GetValueType() == snap.Source.Index then
          previous = snap
        end
      end
    end
    assert(previous, "native Great Work working deal differs from saved intent")
    assert(type(DealManager.ClearWorkingDeal) == "function",
      "completed native Great Work working item cannot be cleared")
    DealManager.ClearWorkingDeal(DealDirection.OUTGOING, from, to)
    working = assert(DealManager.GetWorkingDeal(DealDirection.OUTGOING, from, to),
      "native Great Work working deal unavailable after clearing its completed item")
    assert(working:GetItemCount() == 0,
      "completed native Great Work item remained in the working deal")
  end
  local available = DealManager.GetPossibleDealItems(from, to, DealItemTypes.GREATWORK, working)
  local candidate
  for _, possible in ipairs(available or {}) do
    if possible.ForType == index then candidate = possible end
  end
  assert(candidate and candidate.IsValid, "native Great Work is not a valid deal item")
  local item = assert(working:AddItemOfType(DealItemTypes.GREATWORK, from),
    "native Great Work deal item could not be added")
  item:SetSubType(subtype)
  item:SetValueType(index)
  working:Validate()
  assert(working:IsValid(), "native Great Work deal did not validate")
  return working
end

local function NativeCarrierState(city, carrier)
  local building
  for info in GameInfo.Buildings() do
    if info.Index == carrier.Building then building = info; break end
  end
  assert(building, "native carrier building absent")
  local assets = CityAssets(city)
  local built = assert(assets[building.BuildingType], "native carrier building lost")
  built.Works[carrier.Slot] = -1 -- the one work being moved is the expected difference
  local reads = ExposedMembers.JAC_HeroReadAPI
  assert(reads and reads.ReadCarrierPlots,
    "native carrier purchased plots cannot be observed")
  local plots = reads.ReadCarrierPlots(city:GetOwner(), city:GetID())
  local units = {}
  local owners, seen = {}, {}
  for _, owner in ipairs(PlayerManager.GetAliveIDs()) do
    if not seen[owner] then owners[#owners + 1], seen[owner] = owner, true end
  end
  if not seen[carrier.Owner] then owners[#owners + 1] = carrier.Owner end
  for _, owner in ipairs(owners) do
    if Players[owner] then
      for _, unit in ipairs(Units(owner)) do
        if unit:GetX() == carrier.X and unit:GetY() == carrier.Y then
          units[#units + 1] = { Owner = owner, ID = unit:GetID(),
            Type = GameInfo.Units[unit:GetType()].UnitType }
        end
      end
    end
  end
  table.sort(units, function(a, b)
    if a.Owner ~= b.Owner then return a.Owner < b.Owner end
    return a.ID < b.ID
  end)
  return { Name = city:GetName(), Population = city:GetPopulation(),
    Assets = assets, Religion = Religion(city), Loyalty = A.CityLoyalty(city),
    Faith = R.Copy(city:GetProperty(R.CITY_FAITH_KEY)),
    Governance = R.Copy(city:GetProperty(R.GOVERNANCE_KEY)),
    UnitsOnCenter = units, PurchasedPlots = plots }
end

local function NativeCarrierActorReady()
  assert(not (GameConfiguration and GameConfiguration.IsAnyMultiplayer
    and GameConfiguration.IsAnyMultiplayer()),
    "native Great Work carrier MOVE has no verified multiplayer actor")
  local actor = Game.GetLocalPlayer and Game.GetLocalPlayer()
  local player = type(actor) == "number" and Players[actor]
  local reads = ExposedMembers.JAC_HeroReadAPI
  return reads and reads.NativeMoveAvailable == true
    and player and player:IsAlive() and player:IsHuman()
    and (not player.IsTurnActive or player:IsTurnActive())
end

local function NativeCarrierSlot(carrier, owner, id)
  return { Owner = owner, City = id, X = carrier.X, Y = carrier.Y,
    Building = carrier.Building, Slot = carrier.Slot }
end

local function AdvanceNativeCarrier(state, carrier)
  local city = assert(CityManager.GetCityAt(carrier.X, carrier.Y),
    "native Great Work carrier city disappeared")
  local slotValue = false
  if city:GetBuildings():HasBuilding(carrier.Building) then
    local ok, value = pcall(WorkInSlot, city, carrier.Building, carrier.Slot)
    if not ok then
      if tostring(value):find("UI city cache is not ready", 1, true) then return false end
      error(value)
    end
    slotValue = value
  end
  assert(slotValue ~= nil, "native Great Work carrier building disappeared")
  if carrier.Phase == "BORROW" then
    local expected = carrier.Route == "TO_FREE" and -1 or carrier.Index
    assert(slotValue == expected, "native Great Work carrier changed before borrowing")
    if city:GetOwner() == carrier.Owner then
      if not NativeCarrierActorReady() then return false end
      assert(city:GetID() == carrier.OriginalCity, "native Great Work carrier identity changed")
      for _, work in ipairs(A.OwnedGreatWorkSlots(carrier.Owner)) do
        assert(work.X ~= carrier.X or work.Y ~= carrier.Y or work.Index == carrier.Index
          and carrier.Route == "FROM_FREE", "native Great Work carrier gained another work")
      end
      carrier.Before = NativeCarrierState(city, carrier)
      assert(#carrier.Before.UnitsOnCenter == 0,
        "native Great Work carrier city has a unit on its center")
      Save(state)
      CityManager.TransferCity(city, carrier.Borrower)
      return false
    end
    assert(city:GetOwner() == carrier.Borrower,
      "native Great Work carrier was borrowed by another owner")
    local reads = ExposedMembers.JAC_HeroReadAPI
    if not reads or not reads.GetObservation
      or not reads.GetObservation(carrier.BorrowToken) then return false end
    carrier.BorrowedCity = city:GetID()
    if carrier.Route == "TO_FREE" then
      carrier.MoveSource = { Owner = carrier.Source.Owner, City = carrier.Source.CityID,
        X = carrier.Source.X, Y = carrier.Source.Y,
        Building = carrier.Source.Building, Slot = carrier.Source.Slot }
      carrier.MoveTarget = NativeCarrierSlot(carrier, carrier.Borrower, city:GetID())
    else
      carrier.MoveSource = NativeCarrierSlot(carrier, carrier.Borrower, city:GetID())
      carrier.MoveTarget = R.Copy(carrier.Target)
    end
    carrier.Phase, carrier.MoveAttempt = "MOVE", 1
    carrier.MoveToken = carrier.Token .. ":move:1"
    Save(state)
    return false
  end
  if carrier.Phase == "MOVE" then
    assert(city:GetOwner() == carrier.Borrower and city:GetID() == carrier.BorrowedCity,
      "native Great Work borrowed carrier changed before MOVE")
    local reads = ExposedMembers.JAC_HeroReadAPI
    local observed = reads and reads.GetObservation and reads.GetObservation(carrier.MoveToken)
    if not observed then return false end
    local locations = NativeWorkLocations(carrier.Index)
    assert(#locations == 1, "native Great Work MOVE index is missing or duplicated")
    SameNativeWorkIdentity(carrier.Source, locations[1])
    if SameNativeWorkSlot(locations[1], carrier.MoveTarget) then
      local source = assert(CityManager.GetCityAt(carrier.MoveSource.X, carrier.MoveSource.Y),
        "native Great Work MOVE source city disappeared")
      assert(WorkInSlot(source, carrier.MoveSource.Building, carrier.MoveSource.Slot) == -1,
        "native Great Work MOVE source slot was not emptied")
      carrier.Phase = "RETURN"
      carrier.ReturnToken = carrier.Token .. ":return"
      Save(state) -- the return intent precedes the city mutation
      CityManager.TransferCity(city, carrier.Owner)
      return false
    end
    assert(SameNativeWorkSlot(locations[1], carrier.MoveSource)
      and WorkInSlot(city, carrier.Building, carrier.Slot)
        == (carrier.Route == "TO_FREE" and -1 or carrier.Index),
      "native Great Work MOVE landed in an unexpected slot")
    if not observed.Unchanged then return false end
    if carrier.MoveAttempt < 2 then
      carrier.MoveAttempt = carrier.MoveAttempt + 1
      carrier.MoveToken = carrier.Token .. ":move:" .. carrier.MoveAttempt
      Save(state)
      return false
    end
    carrier.Phase = "RESTORE"
    carrier.ReturnToken = carrier.Token .. ":restore"
    Save(state) -- the empty carrier can be returned without changing the work
    CityManager.TransferCity(city, carrier.Owner)
    return false
  end
  if carrier.Phase == "RESTORE" then
    if city:GetOwner() == carrier.Borrower then
      local locations = NativeWorkLocations(carrier.Index)
      assert(#locations == 1 and SameNativeWorkSlot(locations[1], carrier.MoveSource),
        "native Great Work changed before carrier rollback")
      CityManager.TransferCity(city, carrier.Owner)
      return false
    end
    assert(city:GetOwner() == carrier.Owner,
      "native Great Work carrier rollback changed owner")
    local reads = ExposedMembers.JAC_HeroReadAPI
    if not reads or not reads.GetObservation
      or not reads.GetObservation(carrier.ReturnToken) then return false end
    local locations = NativeWorkLocations(carrier.Index)
    assert(#locations == 1, "native Great Work changed during carrier rollback")
    SameNativeWorkIdentity(carrier.Source, locations[1])
    if carrier.Route == "TO_FREE" then
      assert(SameNativeWorkSlot(locations[1], carrier.Source)
        and WorkInSlot(city, carrier.Building, carrier.Slot) == -1,
        "native Great Work or empty carrier changed during rollback")
    else
      assert(SameNativeWorkSlot(locations[1],
          NativeCarrierSlot(carrier, carrier.Owner, city:GetID()))
        and WorkInSlot(city, carrier.Building, carrier.Slot) == carrier.Index,
        "native Great Work left its Free Cities carrier during rollback")
    end
    assert(R.Equal(NativeCarrierState(city, carrier), carrier.Before),
      "native Great Work carrier city changed during rollback")
    carrier.Phase = "RESTORED"
    Save(state)
    error("native Great Work MOVE did not move the original index; carrier restored")
  end
  if carrier.Phase == "RETURN" then
    if city:GetOwner() == carrier.Borrower then
      local locations = NativeWorkLocations(carrier.Index)
      assert(#locations == 1 and SameNativeWorkSlot(locations[1], carrier.MoveTarget),
        "native Great Work left the borrowed carrier route before return")
      CityManager.TransferCity(city, carrier.Owner)
      return false
    end
    assert(city:GetOwner() == carrier.Owner,
      "native Great Work carrier returned to another owner")
    local reads = ExposedMembers.JAC_HeroReadAPI
    if not reads or not reads.GetObservation
      or not reads.GetObservation(carrier.ReturnToken) then return false end
    carrier.FinalSlot = NativeCarrierSlot(carrier, carrier.Owner, city:GetID())
    local locations = NativeWorkLocations(carrier.Index)
    assert(#locations == 1, "native Great Work carrier return index is missing or duplicated")
    SameNativeWorkIdentity(carrier.Source, locations[1])
    local expected = carrier.Route == "TO_FREE" and carrier.FinalSlot or carrier.Target
    assert(SameNativeWorkSlot(locations[1], expected),
      "native Great Work missed its final reserved carrier route slot")
    assert(WorkInSlot(city, carrier.Building, carrier.Slot)
      == (carrier.Route == "TO_FREE" and carrier.Index or -1),
      "native Great Work carrier return slot changed")
    assert(R.Equal(NativeCarrierState(city, carrier), carrier.Before),
      "native Great Work carrier city changed during custody")
    carrier.Phase = "PROVED"
    Save(state)
    return true
  end
  assert(carrier.Phase == "PROVED", "native Great Work carrier route needs review")
  local locations = NativeWorkLocations(carrier.Index)
  local expected = carrier.Route == "TO_FREE" and carrier.FinalSlot or carrier.Target
  assert(#locations == 1 and SameNativeWorkSlot(locations[1], expected),
    "proved native Great Work carrier route changed")
  SameNativeWorkIdentity(carrier.Source, locations[1])
  assert(city:GetOwner() == carrier.Owner,
    "proved native Great Work carrier owner changed")
  if carrier.RefreshRequested then
    carrier.RefreshRequested = nil
    Save(state)
  end
  return true
end

local function NativeCarrierUIStale(err)
  local message = tostring(err)
  return message:find("UI city cache is not ready", 1, true)
    or message:find("native carrier city is not published", 1, true)
end

local function MoveNativeWorks(state, tx)
  local reserved = {}
  local reservedCities = ReservedNativeCarrierCities(state, tx.ID)
  for _, pending in ipairs(tx.NativeWorks or {}) do
    if pending.TargetSlot then
      local slot = pending.TargetSlot
      reserved[slot.Owner .. ":" .. slot.City .. ":" .. slot.Building .. ":" .. slot.Slot] = true
      if pending.Destination and pending.Destination < 0 then
        reservedCities[slot.X .. ":" .. slot.Y] = true
      end
    end
  end
  for _, snap in ipairs(tx.NativeWorks or {}) do
    if snap.Custodian then
      if not snap.TargetSlot then
        local player = Players[snap.Custodian]
        if not player or player:GetCities():GetCount() == 0 then
          error("JAC_ABSORPTION_WAITING_UI", 0)
        end
        local targetReserved = ReservedNativeSlots(state, snap.Custodian, tx.ID)
        for key in pairs(reserved) do targetReserved[key] = true end
        local ok, destination = pcall(NativeWorkDestination,
          snap.Custodian, snap.Source, targetReserved, snap.Destination < 0,
          reservedCities)
        if not ok then
          if tostring(destination) == "JAC_ABSORPTION_WAITING_UI"
            or tostring(destination):find("no compatible empty destination slot", 1, true) then
            error("JAC_ABSORPTION_WAITING_UI", 0)
          end
          error(destination)
        end
        snap.TargetSlot = destination
        local slot = destination
        reserved[slot.Owner .. ":" .. slot.City .. ":" .. slot.Building .. ":" .. slot.Slot] = true
        if snap.Destination < 0 then reservedCities[slot.X .. ":" .. slot.Y] = true end
        Save(state)
      end
      if snap.Destination < 0 then
        assert(not snap.PhysicalMoved or snap.Carrier,
          "native scattered work has no verified carrier history")
        if not snap.Carrier then
          local slot = snap.TargetSlot
          local city = assert(CityManager.GetCityAt(slot.X, slot.Y),
            "native Great Work Free Cities carrier disappeared")
          assert(city:GetOwner() == snap.Custodian and city:GetID() == slot.City
            and WorkInSlot(city, slot.Building, slot.Slot) == -1,
            "native Great Work Free Cities carrier is not empty")
          for _, other in ipairs(A.OwnedGreatWorkSlots(snap.Custodian)) do
            assert(other.X ~= slot.X or other.Y ~= slot.Y,
              "native Great Work carrier city holds another original work")
          end
          snap.Carrier = { Route = "TO_FREE", Phase = "BORROW", Index = snap.Source.Index,
            Source = R.Copy(snap.Source), Owner = snap.Custodian, Borrower = tx.From,
            OriginalCity = city:GetID(), X = slot.X, Y = slot.Y,
            Building = slot.Building, Slot = slot.Slot,
            Token = tx.ID .. ":carrier:" .. snap.Source.Index,
            BorrowToken = tx.ID .. ":carrier:" .. snap.Source.Index .. ":borrow" }
          snap.Carrier.Before = NativeCarrierState(city, snap.Carrier)
          Save(state)
        end
        local ready, advanced = pcall(AdvanceNativeCarrier, state, snap.Carrier)
        if not ready then
          if NativeCarrierUIStale(advanced) then
            snap.Carrier.RefreshRequested = snap.Carrier.Phase == "PROVED"
            tx.AwaitingFinalUI = true
            Save(state)
            error("JAC_ABSORPTION_WAITING_UI", 0)
          end
          error(advanced)
        end
        if not advanced then
          error("JAC_ABSORPTION_WAITING_UI", 0)
        end
        snap.TargetSlot = R.Copy(snap.Carrier.FinalSlot)
        snap.PhysicalMoved = true
        Save(state)
        NativeWorkAtTarget(snap)
        NativeWorkSourceEmpty(tx, snap)
      elseif snap.PhysicalMoved then
        NativeWorkAtTarget(snap)
        NativeWorkSourceEmpty(tx, snap)
      else
        local reads = ExposedMembers.JAC_HeroReadAPI
        if snap.DealSubmitted and (not tx.NativeDealUI or not reads
          or not reads.GetObservation or not reads.GetObservation(tx.NativeDealUI.Token)) then
          error("JAC_ABSORPTION_WAITING_UI", 0)
        end
        local locations = NativeWorkLocations(snap.Source.Index)
        assert(#locations == 1, "native Great Work index is missing or duplicated: " .. snap.ID)
        SameNativeWorkIdentity(snap.Source, locations[1])
        if SameNativeWorkSlot(locations[1], snap.TargetSlot) then
          NativeWorkSourceEmpty(tx, snap)
          snap.PhysicalMoved, tx.NativeDealUI = true, nil
          Save(state)
        else
          assert(SameNativeWorkSlot(locations[1], snap.Source),
            "native Great Work moved to an unexpected owner or slot: " .. snap.ID)
          local city = assert(CityManager.GetCityAt(snap.TargetSlot.X, snap.TargetSlot.Y),
            "native Great Work recipient city disappeared")
          assert(city:GetOwner() == snap.Custodian and city:GetID() == snap.TargetSlot.City
            and city:GetBuildings():HasBuilding(snap.TargetSlot.Building)
            and snap.TargetSlot.Slot < WorkSlots(city, snap.TargetSlot.Building)
            and WorkInSlot(city, snap.TargetSlot.Building, snap.TargetSlot.Slot) == -1,
            "native Great Work reserved destination changed before deal")
          local definition = assert(GameInfo.GreatWorks[snap.Source.GreatWorkType],
            "native Great Work type unavailable for deal")
          BuildNativeWorkDeal(tx.From, snap.Custodian,
            snap.Source.Index, definition.Index, tx.NativeWorks)
          snap.DealSubmitted = true
          snap.DealAttempts = (snap.DealAttempts or 0) + 1
          tx.NativeDealUI = { Token = tx.ID .. ":native:" .. snap.Source.Index .. ":" .. snap.DealAttempts,
            Index = snap.Source.Index, Turn = Game.GetCurrentGameTurn() }
          Save(state) -- the intent survives a crash after the native call
          local ok, value = pcall(DealManager.EnactWorkingDeal, tx.From, snap.Custodian)
          if not ok then
            snap.DealError = tostring(value)
            Save(state)
            error("native Great Work deal raised; inspect original physical index: " .. snap.DealError)
          end
          Log("native deal submitted id=" .. tx.ID .. " index=" .. snap.Source.Index
            .. " from=" .. tx.From .. " to=" .. snap.Custodian .. " return=" .. tostring(value))
          error("JAC_ABSORPTION_WAITING_UI", 0)
        end
      end
    end
  end
end

local function Execute(state, tx)
  MoveNativeWorks(state, tx) -- no unit deletion or city transfer precedes external-work proof
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
    MovedAssets(tx, false) -- check concurrent roster conflicts before any native escrow/city change
    local ready = true
    for _, snap in ipairs(tx.Units) do
      if snap.KeepUnit and not snap.Hero then
        local unit = FindToken(snap.Destination, snap.Token)
        RestoreTransferred(state, tx, unit, snap, true)
        Save(state)
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
  MovedAssets(tx, false) -- detect newly conflicting destination IDs before donor deletion
  local ready = true
  for _, snap in ipairs(tx.Units) do
    if snap.KeepUnit then
      local unit = FindToken(snap.Destination, snap.Token)
      RestoreTransferred(state, tx, unit, snap)
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
      local policy = tx.Request.FlowPolicy and assert(R.CITY_POLICY[tx.Mode], "unknown city policy")
        or { Immediate = 0, Friction = 0, Turns = 0 }
      city:SetProperty(R.GOVERNANCE_KEY, { ID = tx.ID, From = tx.From, To = tx.To, Turn = tx.Turn,
        Mode = tx.Mode, LoyaltyDelta = policy.Immediate, FrictionPerTurn = policy.Friction,
        FrictionTurns = policy.Turns, FrictionApplied = 0,
        ImmediateApplied = policy.Immediate == 0, BalancePending = true, Previous = R.Copy(snap.Governance) })
    end
    local governance = city:GetProperty(R.GOVERNANCE_KEY)
    if governance.ID == tx.ID and not governance.ImmediateApplied then
      governance.ImmediatePending = governance.ImmediatePending or {}
      if not AdvanceLoyalty(city, governance, governance.ImmediatePending,
        "immediate", governance.LoyaltyDelta) then
        tx.CityLoyaltyPending = { X = snap.X, Y = snap.Y,
          Token = governance.LoyaltyRead and governance.LoyaltyRead.Token }
        Save(state)
        error("JAC_ABSORPTION_WAITING_UI", 0)
      end
      governance.ImmediateBefore = governance.ImmediatePending.Before
      governance.ImmediateTarget = governance.ImmediatePending.Target
      governance.ImmediatePending, governance.LoyaltyRead = nil, nil
      governance.ImmediateApplied = true
      city:SetProperty(R.GOVERNANCE_KEY, governance)
    end
    tx.CityLoyaltyPending = nil
    Save(state)
  end
  -- Restore all native positions only after the province belongs to the recipient.
  -- Official gameplay example: DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:854,886.
  ready = true
  for _, snap in ipairs(tx.Units) do
    if snap.KeepUnit then
      local unit = assert(FindToken(snap.Destination, snap.Token), "transferred unit lost before placement")
      if not snap.PlacementComplete then
        local restoredTurn = snap.TransferRestoredTurn or snap.CoreStagedTurn
        if not restoredTurn or restoredTurn == Game.GetCurrentGameTurn() then
          if unit:GetX() ~= snap.X or unit:GetY() ~= snap.Y then UnitManager.PlaceUnit(unit, snap.X, snap.Y) end
          assert(unit:GetX() == snap.X and unit:GetY() == snap.Y, "transferred unit placement failed; saved snapshot retained")
        end
        snap.PlacementComplete = true
        Save(state)
      end
      RestoreTransferred(state, tx, unit, snap)
      if not VerifySpread(unit, snap, state, tx, "placed", true) then ready = false end
      Save(state)
    end
  end
  if not ready then error("JAC_ABSORPTION_WAITING_UI", 0) end
  -- Source and recipient use the same capability test for every city.
  local needsCityUI = false
  for _, snap in ipairs(tx.Cities) do
    if CityNeedsUI(assert(CityManager.GetCityAt(snap.X, snap.Y))) then needsCityUI = true; break end
  end
  if needsCityUI then
      local reads = ExposedMembers.JAC_HeroReadAPI
      if not reads or not reads.GetObservation or not reads.GetObservation(tx.ID .. ":final") then
        tx.AwaitingFinalUI = true
        Save(state)
        error("JAC_ABSORPTION_WAITING_UI", 0)
      end
      tx.AwaitingFinalUI = nil
  end
  local rosters, archive = MovedAssets(tx, true)
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
  Verify(tx, state, tx.Turn == Game.GetCurrentGameTurn())
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

-- Only the read-only UI preparation reservation can be released. A prepared
-- transaction, including one paused before Execute, owns its frozen plan.
function A.CancelUnprepared(id, expectedPlan)
  local state = State()
  local tx = state.Transactions[id]
  if not tx then return end
  assert(tx.AwaitingPreparation == true and tx.Status == "WAITING_UI"
    and R.Equal(tx.Request, expectedPlan), "only matching unprepared absorption can be cancelled")
  assert(next(tx.Units or {}) == nil and next(tx.Adapters or {}) == nil,
    "unprepared absorption contains staged mutations")
  state.Transactions[id] = nil
  Save(state)
end

function A.Resume(id)
  local state = State()
  local tx = assert(state.Transactions[id], "unknown absorption transaction")
  if tx.Status == "COMPLETE" then return R.Copy(tx) end
  assert(tx.Turn == Game.GetCurrentGameTurn() or tx.Request.FlowPolicy,
    "manual transfers require their adapter contract in the original turn")
  if tx.Status == "BLOCKED" then
    tx.FirstError = tx.FirstError or tx.Error
    tx.PreviousError = tx.Error
    Save(state)
  end
  if tx.AwaitingPreparation then
    if not tx.SourceSpreadReads or not tx.SourceReadyToken
      or tx.Turn ~= Game.GetCurrentGameTurn() then
      tx.Turn = Game.GetCurrentGameTurn() -- no mutation exists before Prepare
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

-- Reward recovery is a second, independently journaled transaction. The first
-- absorption's archive remains immutable so its saved verification still works.
local function RetainRecoveryCandidate(state, intent, token, candidate)
  if candidate then
    local id = candidate:GetID()
    assert(not intent.Before[id] and candidate:GetOwner() == intent.Owner
      and candidate.IsHero and candidate:IsHero()
      and (not candidate:GetProperty(R.TOKEN_KEY)
        or candidate:GetProperty(R.TOKEN_KEY) == token),
      "unrelated Hero at failed recovery; preserve it for review")
    assert(not intent.CandidateID or intent.CandidateID == id, "recovery candidate changed")
    candidate:SetProperty(R.TOKEN_KEY, token)
    intent.CandidateID = id
  end
  Save(state)
end
local function AbortRecoveredHero(state, intent, token, cause, candidate)
  -- A native failure is not a person choosing death. Keep both the candidate
  -- and any claim changed by CreateHero under the same durable recovery token.
  RetainRecoveryCandidate(state, intent, token, candidate)
  intent.Status, intent.Cause = "BLOCKED", cause
  intent.FirstCause = intent.FirstCause or cause
  intent.AwaitingUI, intent.NativeVerified = nil, nil
  intent.ObservationAttempt = (intent.ObservationAttempt or 0) + 1
  Save(state)
  return R.Copy(intent)
end
local function JournalRecoveryClaim(state, intent, manager, class)
  local claimOK, claim = pcall(manager.GetHeroClaimPlayer, manager, class)
  local originOK, origin = pcall(HeroOrigin, manager, class)
  intent.NativeClaimAfter = claimOK and claim or nil
  intent.NativeOriginAfter = originOK and R.Copy(origin) or nil
  intent.HeroClass = class
  Save(state)
end

local function ObserveRecoveredHero(state, intent, token, class, origin)
  if intent.NativeVerified then return true end
  assert(intent.CandidateID, "recovered Hero has no candidate to observe")
  local observationToken = token .. ":created:" .. intent.CandidateID
    .. ":" .. (intent.ObservationAttempt or 0)
  if not intent.AwaitingUI then
    intent.AwaitingUI, intent.ObservationToken, intent.HeroClass = true, observationToken, class
    intent.Status = "WAITING_UI"
    Save(state)
    return false
  end
  assert(intent.ObservationToken == observationToken,
    "recovered Hero observation candidate changed")
  local reads = ExposedMembers.JAC_HeroReadAPI
  local observed = reads and reads.GetObservation and reads.GetObservation(observationToken)
  if not observed or type(observed.Claim) ~= "number"
    or type(observed.Origin) ~= "table" then
    intent.Status = "WAITING_UI"
    Save(state)
    return false
  end
  local city = observed.Origin.player == intent.Owner
    and Players[intent.Owner]:GetCities():FindID(observed.Origin.id)
  if observed.Claim ~= intent.Owner or not city
    or city:GetX() ~= origin.X or city:GetY() ~= origin.Y then
    return nil, "RECOVERED_HERO_PUBLISHED_CLAIM_OR_ORIGIN_MISMATCH"
  end
  intent.AwaitingUI, intent.NativeVerified = nil, true
  intent.Status = "STAGED"
  Save(state)
  return true
end
local function RecoveredNativeWork(source, owner)
  assert(type(source.Index) == "number" and source.Index >= 0
    and source.Index == math.floor(source.Index), "native recovery index required")
  local found, count = nil, 0
  for _, playerID in ipairs(PlayerManager.GetAliveIDs()) do
    for _, work in ipairs(A.OwnedGreatWorkSlots(playerID)) do
      if work.Index == source.Index then
        count = count + 1
        if playerID == owner then found = work end
      end
    end
  end
  assert(count == 1 and found,
    "native recovery requires one verified original work in the recipient's slot")
  for _, key in ipairs({ "GreatWorkType", "ObjectType", "CreatorName", "TurnCreated", "CreatedByPlayer" }) do
    assert(source[key] == nil or found[key] == source[key],
      "recovered native Great Work metadata changed: " .. key)
  end
  return found
end
local function NativeRecoverySourceEmpty(source)
  local city = CityManager.GetCityAt(source.X, source.Y)
  if city and city:GetBuildings():HasBuilding(source.Building) then
    assert(WorkInSlot(city, source.Building, source.Slot) == -1,
      "native recovery custodian slot was reused")
  end
end
local function RecoverNativeWorkByDeal(state, intent, source, owner, token)
  if not intent.NativeTarget then
    local already = NativeWorkLocations(source.Index)
    if #already == 1 and already[1].Owner == owner then
      SameNativeWorkIdentity(source, already[1])
      return true -- an existing caller may have moved the original work already
    end
  end
  if source.Owner == owner then return true end
  if not intent.NativeTarget then
    local locations = NativeWorkLocations(source.Index)
    assert(#locations == 1 and SameNativeWorkSlot(locations[1], source),
      "native recovery original index is not at its saved custodian")
    SameNativeWorkIdentity(source, locations[1])
    intent.NativeSource = R.Copy(source)
    Save(state) -- keep the original location even if no recipient slot is available
    assert(source.BuildingType ~= "BUILDING_PALACE",
      "native recovery Palace carrier has no verified city-preserving route")
    intent.NativeTarget = NativeWorkDestination(owner, source,
      ReservedNativeSlots(state, owner, nil, token))
    Save(state)
  end
  if intent.NativePhysicalMoved and not intent.NativeCarrier then
    NativeWorkAtTarget({ ID = token, Source = intent.NativeSource,
      TargetSlot = intent.NativeTarget })
    NativeRecoverySourceEmpty(intent.NativeSource)
    return true
  end
  if not intent.NativeCarrier then
    local city = assert(CityManager.GetCityAt(source.X, source.Y),
      "native recovery Free Cities carrier disappeared")
    assert(city:GetOwner() == source.Owner and city:GetID() == source.CityID
      and WorkInSlot(city, source.Building, source.Slot) == source.Index,
      "native recovery original work left its Free Cities carrier")
    for _, other in ipairs(A.OwnedGreatWorkSlots(source.Owner)) do
      assert(other.Index == source.Index or other.X ~= source.X or other.Y ~= source.Y,
        "native recovery carrier city holds another work")
    end
    intent.NativeCarrier = { Route = "FROM_FREE", Phase = "BORROW", Index = source.Index,
      Source = R.Copy(source), Target = R.Copy(intent.NativeTarget),
      Owner = source.Owner, Borrower = owner, OriginalCity = source.CityID,
      X = source.X, Y = source.Y, Building = source.Building, Slot = source.Slot,
      Token = token .. ":carrier:" .. source.Index,
      BorrowToken = token .. ":carrier:" .. source.Index .. ":borrow" }
    intent.NativeCarrier.Before = NativeCarrierState(city, intent.NativeCarrier)
    Save(state)
  end
  local ready, advanced = pcall(AdvanceNativeCarrier, state, intent.NativeCarrier)
  if not ready and NativeCarrierUIStale(advanced) then
    intent.NativeCarrier.RefreshRequested = intent.NativeCarrier.Phase == "PROVED"
    advanced = false
  elseif not ready then error(advanced) end
  if not advanced then
    local carrier = intent.NativeCarrier
    intent.ObservationToken = carrier.Phase == "BORROW" and carrier.BorrowToken
      or carrier.Phase == "MOVE" and carrier.MoveToken or carrier.ReturnToken
    intent.Status = "WAITING_UI"
    Save(state)
    return false
  end
  intent.NativePhysicalMoved, intent.ObservationToken = true, nil
  Save(state)
  NativeWorkAtTarget({ ID = token, Source = intent.NativeSource,
    TargetSlot = intent.NativeTarget })
  NativeRecoverySourceEmpty(intent.NativeSource)
  return true
end
function A.RecoverArchived(id, owner, token, original)
  assert((type(id) == "string" or type(id) == "number")
    and type(token) == "string" and type(owner) == "number",
    "recovery identity required")
  assert(Players[owner] and Players[owner]:IsAlive(), "recovery recipient is not alive")
  local state = State()
  state.Recoveries = state.Recoveries or {}
  local intent = state.Recoveries[token]
  if intent then
    assert(intent.ID == id and intent.Owner == owner, "recovery token reused")
    if intent.Status == "COMPLETE" then
      local record = assert(GetRoster(owner)[id], "completed recovered identity disappeared")
      assert(record.RecoveryToken == token, "completed recovery identity changed")
      return R.Copy(intent)
    end
    if intent.Status == "SUICIDE" then return R.Copy(intent) end
  else
    assert(state.Archive[id], "escaped asset is absent from absorption archive")
    assert(type(original) == "nil" or type(original) == "table", "invalid unit snapshot")
    local roster = GetRoster(owner)
    assert(not roster[id], "recipient already has the escaped identity")
    intent = { ID = id, Owner = owner, Status = "PREPARED", Snapshot = R.Copy(original),
      Before = {}, Turn = Game.GetCurrentGameTurn() }
    for _, unit in ipairs(Units(owner)) do intent.Before[unit:GetID()] = true end
    state.Recoveries[token] = intent
    Save(state)
  end
  local archived = assert(state.Archive[id], "recovery archive lost")
  if archived.Kind == "ITEM" and archived.NativeWork then
    if not RecoverNativeWorkByDeal(state, intent, archived.NativeWork, owner, token) then
      return R.Copy(intent)
    end
  end
  local record = R.Copy(archived)
  record.Owner, record.Outcome, record.RecoveredTurn = owner, "RECOVERED", intent.Turn
  record.RecoveryToken = token
  if record.Kind == "HERO" and record.NativeHero then
    record.HistoricalNativeHero = R.Copy(record.NativeHero)
    record.NativeHero.ClaimOwner = owner
    if record.NativeHero.Origin then record.NativeHero.Origin.Owner = owner end
  end
  if record.Kind == "ITEM" then
    if record.NativeWork then
      local work = RecoveredNativeWork(record.NativeWork, owner)
      record.NativeWork = R.Copy(work)
      record.City = { Owner = owner, X = work.X, Y = work.Y }
      record.NativeCustody = { Owner = owner, City = work.CityID,
        X = work.X, Y = work.Y, Building = work.Building, Slot = work.Slot }
    else
      local capital = assert(Players[owner]:GetCities():GetCapitalCity(),
        "recovered item has no vault city")
      record.City = { Owner = owner, X = capital:GetX(), Y = capital:GetY() }
    end
  end
  local snap = intent.Snapshot
  if snap then
    snap.Destination, snap.Token = owner, token
    local found = FindToken(owner, token)
    if not found then
      for _, unit in ipairs(Units(owner)) do
        if not intent.Before[unit:GetID()] and GameInfo.Units[unit:GetType()].UnitType == snap.Type
          and (not snap.Hero or unit.IsHero and unit:IsHero()
            and unit:GetHeroClassType() == snap.Hero.Class) then
          assert(not found, "ambiguous unsaved recovery candidate")
          found = unit
        end
      end
      if found then
        assert(Game.GetCurrentGameTurn() == (intent.SpawnTurn or intent.Turn),
          "untagged recovery candidate crossed turns; review identity")
      end
      if not found then
        if snap.Hero then
          local manager = Heroes()
          local origin = snap.Hero.Origin
          local city = origin and CityManager.GetCityAt(origin.X, origin.Y)
          local collision = #HeroUnits(owner, snap.Hero.Class) > 0
          if not city or city:GetOwner() ~= owner or collision then
            return AbortRecoveredHero(state, intent, token,
              collision and "HERO_CLASS_COLLISION" or "ORIGIN_UNAVAILABLE")
          end
          local building = manager:GetPlayerHeroOriginBuildingType(owner)
          if not building or not city:GetBuildings():HasBuilding(building) then
            return AbortRecoveredHero(state, intent, token, "ORIGIN_BUILDING_UNAVAILABLE")
          end
          local discovered = pcall(manager.SetHeroDiscovered, manager, owner, snap.Hero.Class, true)
          if not discovered then
            return AbortRecoveredHero(state, intent, token, "HERO_DISCOVERY_ERROR")
          end
          assert(not intent.CandidateID, "saved recovery candidate was lost; retain identity for review")
          intent.SpawnTurn = Game.GetCurrentGameTurn()
          Save(state)
          local ok = pcall(manager.CreateHero, manager, owner, snap.Hero.Class, origin.X, origin.Y)
          JournalRecoveryClaim(state, intent, manager, snap.Hero.Class)
          local matches = HeroUnits(owner, snap.Hero.Class)
          assert(#matches <= 1, "ambiguous native Hero candidates after recovery create")
          if not ok or #matches ~= 1 then
            return AbortRecoveredHero(state, intent, token,
              ok and "HERO_CREATE_NO_UNIT" or "HERO_CREATE_ERROR", matches[1])
          end
          found = matches[1]
        else
          -- The pre-spawn ID inventory is durable; a reload can adopt exactly
          -- one newly created untagged candidate instead of creating another.
          found = Spawn(snap)
        end
      end
      assert(not found:GetProperty(R.TOKEN_KEY) or found:GetProperty(R.TOKEN_KEY) == token,
        "recovery candidate belongs to another transaction")
      assert(not intent.CandidateID or intent.CandidateID == found:GetID(), "recovery candidate changed")
      found:SetProperty(R.TOKEN_KEY, token)
      intent.CandidateID = found:GetID()
      intent.Status = "STAGED"
      Save(state)
    end
    assert(not intent.CandidateID or intent.CandidateID == found:GetID(), "recovery candidate changed")
    if found:GetX() < 0 or found:GetY() < 0 then
      local capital = assert(Players[owner]:GetCities():GetCapitalCity(), "recovery staging city missing")
      UnitManager.PlaceUnit(found, capital:GetX(), capital:GetY())
    end
    record.FormerUnit = R.Copy(record.Unit)
    record.Unit = { Owner = owner, ID = found:GetID(), Token = token }
    snap.TransferredProperties = R.Copy(snap.Properties or {})
    if snap.TransferredProperties[R.PERSON_KEY] then
      snap.TransferredProperties[R.PERSON_KEY] = R.Copy(record)
    end
    if snap.Hero then
      if not intent.CoreRestored then
        -- A published claim can arrive after the AI's current action window.
        -- Restore native and registered person state before the candidate can be used.
        local restored = pcall(RestoreUnit, found, snap)
        if not restored then
          return AbortRecoveredHero(state, intent, token, "HERO_RESTORE_ERROR", found)
        end
        intent.CoreRestored = true
        intent.CoreRestoredTurn = Game.GetCurrentGameTurn()
        Save(state)
      end
      local ready, cause = ObserveRecoveredHero(state, intent, token, snap.Hero.Class,
        snap.Hero.Origin)
      if cause then return AbortRecoveredHero(state, intent, token, cause, found) end
      if not ready then return R.Copy(intent) end
    end
    if snap.Hero then
      local ok, err = pcall(function()
        VerifyUnit(found, snap, false, intent.CoreRestoredTurn
          and intent.CoreRestoredTurn < Game.GetCurrentGameTurn())
        if snap.SpreadCharges ~= nil then VerifySpread(found, snap) end
      end)
      if not ok then
        if tostring(err) == "JAC_ABSORPTION_WAITING_UI" then error(err, 0) end
        return AbortRecoveredHero(state, intent, token, "HERO_RESTORE_OR_VERIFY_ERROR", found)
      end
    else
      RestoreUnit(found, snap)
      VerifyUnit(found, snap)
      if snap.SpreadCharges ~= nil then VerifySpread(found, snap) end
    end
  elseif record.Kind == "HERO" and record.NativeHero then
    local native = record.NativeHero
    local manager = Heroes()
    local origin = native.Origin
    local city = origin and origin.X and origin.Y and CityManager.GetCityAt(origin.X, origin.Y)
    local building = manager:GetPlayerHeroOriginBuildingType(owner)
    if not city or city:GetOwner() ~= owner or not building
      or not city:GetBuildings():HasBuilding(building) then
      return AbortRecoveredHero(state, intent, token, "DORMANT_ORIGIN_UNAVAILABLE")
    end
    local matches = HeroUnits(owner, native.Class)
    if #matches > 1 or #matches == 1 and intent.Before[matches[1]:GetID()] then
      return AbortRecoveredHero(state, intent, token, "DORMANT_HERO_CLASS_COLLISION")
    end
    local unit = matches[1]
    if unit and not unit:GetProperty(R.TOKEN_KEY) then
      assert(Game.GetCurrentGameTurn() == (intent.SpawnTurn or intent.Turn),
        "untagged dormant Hero candidate crossed turns; review identity")
    end
    if not unit then
      local discovered = pcall(manager.SetHeroDiscovered, manager, owner, native.Class, true)
      if not discovered then
        return AbortRecoveredHero(state, intent, token, "DORMANT_DISCOVERY_ERROR")
      end
      assert(not intent.CandidateID, "saved dormant recovery candidate was lost; retain identity for review")
      intent.SpawnTurn = Game.GetCurrentGameTurn()
      Save(state)
      local ok = pcall(manager.CreateHero, manager, owner, native.Class, origin.X, origin.Y)
      JournalRecoveryClaim(state, intent, manager, native.Class)
      matches = HeroUnits(owner, native.Class)
      assert(#matches <= 1, "ambiguous dormant Hero candidates after recovery create")
      if not ok or #matches ~= 1 then
        return AbortRecoveredHero(state, intent, token,
          ok and "DORMANT_CREATE_NO_UNIT" or "DORMANT_CREATE_ERROR", matches[1])
      end
      unit = matches[1]
    end
    assert(not unit:GetProperty(R.TOKEN_KEY) or unit:GetProperty(R.TOKEN_KEY) == token,
      "dormant Hero candidate belongs to another recovery")
    assert(not intent.CandidateID or intent.CandidateID == unit:GetID(), "dormant recovery candidate changed")
    unit:SetProperty(R.TOKEN_KEY, token)
    intent.CandidateID, intent.Status = unit:GetID(), "STAGED"
    Save(state)
    local ready, cause = ObserveRecoveredHero(state, intent, token, native.Class, origin)
    if cause then return AbortRecoveredHero(state, intent, token, cause, unit) end
    if not ready then return R.Copy(intent) end
    local current = HeroOrigin(manager, native.Class)
    if manager:GetHeroClaimPlayer(native.Class) ~= owner or current.Owner ~= owner
      or current.X ~= origin.X or current.Y ~= origin.Y then
      return AbortRecoveredHero(state, intent, token, "DORMANT_CLAIM_VERIFY_ERROR", unit)
    end
    record.FormerUnit = R.Copy(record.Unit)
    record.Unit = { Owner = owner, ID = unit:GetID(), Token = token }
    unit:SetProperty(R.PERSON_KEY, R.Copy(record))
  end
  local roster = GetRoster(owner)
  assert(not roster[id] or R.Equal(roster[id], record), "recovery identity already used")
  roster[id] = record
  Players[owner]:SetProperty(R.ROSTER_KEY, roster)
  intent.Status, intent.Record, intent.Cause = "COMPLETE", R.Copy(record), nil
  Save(state)
  return R.Copy(intent)
end

-- A recovered native Hero follows the same official purchase/rebind boundary
-- as an inherited Hero, but its provenance is the recovery journal rather than
-- a live donor Unit in the original absorption transaction.
function A.BeginRecoveredHeroRecall(id, recoveryToken)
  assert(type(id) == "string" and id ~= "", "recovered recall ID required")
  local state = State()
  local previous = state.RecoveryRecalls[id]
  if previous then
    assert(previous.RecoveryToken == recoveryToken, "recovered recall ID reused")
    return R.Copy(previous)
  end
  local recovery = assert(state.Recoveries[recoveryToken], "unknown Hero recovery")
  assert(recovery.Status == "COMPLETE", "Hero recovery is incomplete")
  local record = assert(GetRoster(recovery.Owner)[recovery.ID], "recovered Hero record missing")
  assert(record.Kind == "HERO" and record.RecoveryToken == recoveryToken
    and record.NativeHero and record.NativeHero.Class, "record has no recovered native Hero right")
  local owner, class = recovery.Owner, record.NativeHero.Class
  assert(#HeroUnits(owner, class) == 0, "recovered Hero is still alive")
  local manager = Heroes()
  local origin = HeroOrigin(manager, class)
  assert(manager:GetHeroClaimPlayer(class) == owner and origin.Owner == owner
    and origin.X == record.NativeHero.Origin.X and origin.Y == record.NativeHero.Origin.Y,
    "recovered native Hero recall right changed")
  for otherID, pending in pairs(state.RecoveryRecalls) do
    assert(otherID == id or pending.Status ~= "PENDING" or pending.Owner ~= owner
      or pending.Class ~= class, "recovered Hero already has a pending recall")
  end
  local before = {}
  for _, unit in ipairs(Units(owner)) do before[unit:GetID()] = true end
  local intent = { ID = id, RecoveryToken = recoveryToken, RecordID = recovery.ID,
    Owner = owner, Class = class, Before = before, Record = R.Copy(record),
    Token = id .. ":unit", Turn = Game.GetCurrentGameTurn(), Status = "PENDING" }
  state.RecoveryRecalls[id] = intent
  Save(state)
  return R.Copy(intent)
end
function A.CompleteRecoveredHeroRecall(id)
  local state = State()
  local intent = assert(state.RecoveryRecalls[id], "unknown recovered Hero recall")
  if intent.Status == "COMPLETE" then return R.Copy(intent.Record) end
  local matches = HeroUnits(intent.Owner, intent.Class)
  assert(#matches == 1 and not intent.Before[matches[1]:GetID()],
    "expected exactly one newly purchased recovered Hero")
  local unit = matches[1]
  assert(not intent.CandidateID or intent.CandidateID == unit:GetID(),
    "recovered Hero purchase candidate changed")
  local manager = Heroes()
  local origin = HeroOrigin(manager, intent.Class)
  assert(manager:GetHeroClaimPlayer(intent.Class) == intent.Owner and origin.Owner == intent.Owner
    and origin.X == intent.Record.NativeHero.Origin.X
    and origin.Y == intent.Record.NativeHero.Origin.Y,
    "recovered Hero claim/origin changed after purchase")
  local token = unit:GetProperty(R.TOKEN_KEY)
  assert(not token or token == intent.Token, "purchased Hero belongs to another transaction")
  intent.CandidateID = unit:GetID()
  Save(state)
  if not token then unit:SetProperty(R.TOKEN_KEY, intent.Token) end
  local record = R.Copy(intent.Record)
  record.FormerUnit = R.Copy(record.Unit)
  record.Unit = { Owner = intent.Owner, ID = unit:GetID(), Token = intent.Token }
  record.RecallHistory = record.RecallHistory or {}
  record.RecallHistory[#record.RecallHistory + 1] =
    { ID = unit:GetID(), Token = intent.Token, Turn = intent.Turn }
  local roster = GetRoster(intent.Owner)
  assert(R.Equal(roster[intent.RecordID], intent.Record)
    or R.Equal(roster[intent.RecordID], record),
    "recovered Hero record changed during purchase")
  unit:SetProperty(R.PERSON_KEY, R.Copy(record))
  roster[intent.RecordID] = R.Copy(record)
  Players[intent.Owner]:SetProperty(R.ROSTER_KEY, roster)
  intent.Record, intent.Status = R.Copy(record), "COMPLETE"
  Save(state)
  return R.Copy(record)
end

-- Called on the real game-turn boundary. All dates and interrupted applications
-- live on the city, so script reloads cannot extend a friction penalty.
function A.ApplyCityFriction(onlyOwner)
  local turn = Game.GetCurrentGameTurn()
  for _, playerID in ipairs(PlayerManager.GetAliveIDs()) do
    local player = Players[playerID]
    if player and (onlyOwner == nil or onlyOwner == playerID) then
      for _, city in player:GetCities():Members() do
        local g = city:GetProperty(R.GOVERNANCE_KEY)
        if g and g.To == playerID and g.ImmediateApplied then
          if not g.FrictionPending and g.FrictionPerTurn and g.FrictionPerTurn ~= 0
            and (g.FrictionApplied or 0) < (g.FrictionTurns or 0)
            and turn > g.Turn and turn <= g.Turn + g.FrictionTurns
            and (not g.LastFrictionTurn or g.LastFrictionTurn < turn) then
            g.FrictionPending = { Turn = turn }
            city:SetProperty(R.GOVERNANCE_KEY, g)
          end
          if g.FrictionPending then
            local pending = g.FrictionPending
            local ready = AdvanceLoyalty(city, g, pending,
              "friction:" .. pending.Turn, g.FrictionPerTurn)
            if ready then
              g.FrictionApplied = (g.FrictionApplied or 0) + 1
              g.LastFrictionTurn, g.FrictionPending, g.LoyaltyRead = pending.Turn, nil, nil
              city:SetProperty(R.GOVERNANCE_KEY, g)
            end
          end
        end
      end
    end
  end
end

if GameEvents and GameEvents.OnGameTurnStarted then
  GameEvents.OnGameTurnStarted.Add(function() A.ApplyCityFriction() end)
end

JAC_AbsorptionGameplay = A
ExposedMembers.JAC_Absorption = {
  GetState = A.GetState, GetRoster = A.GetRoster, GetUIPhase = A.GetUIPhase,
  Canonical = R.Canonical, GetPropertyKeys = A.GetPropertyKeys,
}
Log("loaded version=1 explicit_dispositions=true")
return A
