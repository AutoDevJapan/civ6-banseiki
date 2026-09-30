include("JAC_Absorption_Rules")
local R = JAC_AbsorptionRules
local A = { Rules = R }
local unitKeys = { [R.PERSON_KEY] = true, JAC_POC_GROWTHUI_WARLORD = true }
local cityKeys = { [R.CITY_FAITH_KEY] = true }
local unitRebinders, cityRebinders = {}, {}
local adapters = {}

local function Log(text) print("[JAC_ABSORPTION] " .. text) end
local function State()
  local state = R.Copy(Game:GetProperty(R.STATE_KEY) or { Version = 1, Transactions = {}, Archive = {} })
  assert(state.Version == 1, "unsupported absorption save version")
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
  for _, row in pairs(city:GetReligion():GetReligionsInCity()) do
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
      for slot = 0, buildings:GetNumGreatWorkSlots(info.Index) - 1 do
        works[slot] = buildings:GetGreatWorkInSlot(info.Index, slot)
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
    for slot = 0, buildings:GetNumGreatWorkSlots(palace.Index) - 1 do
      local work = buildings:GetGreatWorkInSlot(palace.Index, slot)
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
        for slot = 0, buildings:GetNumGreatWorkSlots(info.Index) - 1 do
          local work = buildings:GetGreatWorkInSlot(info.Index, slot)
          if work and work >= 0 then works[work] = true end
        end
      end
    end
  end
  return works
end
A.OwnedGreatWorks = OwnedGreatWorks

local function SnapshotUnit(unit)
  local info = assert(GameInfo.Units[unit:GetType()], "unknown unit type")
  -- Native Heroes have engine-owned lifespan/recall data; do not silently treat them as ordinary units.
  assert(not string.match(info.UnitType, "^UNIT_HERO_") and not (unit.IsHero and unit:IsHero()),
    "native Hero needs a verified engine transfer implementation")
  local greatPerson = unit:GetGreatPerson()
  local individual = greatPerson and greatPerson:GetIndividual() or -1
  local person = unit:GetProperty(R.PERSON_KEY)
  local kind = person and person.Kind or (individual >= 0 and "GREAT_PERSON" or "UNIT")
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
    Individual = individual, Damage = unit:GetDamage(), Moves = unit:GetMovesRemaining(),
    GreatPersonCharges = greatPerson and greatPerson.GetActionCharges and greatPerson:GetActionCharges() or nil,
    XP = experience and experience:GetExperiencePoints() or nil, Promotions = promotions, Formation = unit:GetMilitaryFormation(),
    BuildCharges = unit:GetBuildCharges(), SpreadCharges = unit:GetSpreadCharges(), Religion = religion,
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

local function Prepare(plan, state)
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
  local tx = { ID = plan.ID, From = plan.From, To = plan.To, Mode = plan.Mode,
    Turn = Game.GetCurrentGameTurn(), Request = R.Copy(plan), Status = "PREPARED", Units = {}, UnitMap = {}, Cities = {}, Plots = {},
    SourceRoster = GetRoster(plan.From), TargetRoster = GetRoster(plan.To), Decisions = {}, Adapters = {} }
  local generatedDecisions = {}
  tx.OriginalSourceRoster = R.Copy(tx.SourceRoster)
  for _, unit in ipairs(Units(plan.From)) do
    local snap = SnapshotUnit(unit)
    local decision = Decision((plan.Units or {})[R.UnitKey(plan.From, snap.ID)])
    snap.Outcome = decision.Outcome
    snap.Destination = DestinationOwner(snap.Kind, snap.Outcome, decision, tx)
    snap.KeepUnit = R.KeepUnit(snap.Kind, snap.Outcome)
    snap.Token = plan.ID .. ":unit:" .. snap.ID
    tx.Units[#tx.Units + 1] = snap
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
          assert(snap.Outcome == d.Outcome and snap.Destination == d.Owner, "linked person/unit dispositions disagree")
          found = true
        end
      end
      assert(found, "roster points to missing unit: " .. id)
    end
    tx.Decisions[id] = d
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

local function RestoreUnit(unit, snap)
  WriteProperties(unit, snap.TransferredProperties, snap.PropertyKeys)
  -- Official Debug/Unit.ltp:164,175,463; NubiaScenario.lua:150-151; Tutorial.lua:71.
  if snap.Formation then unit:SetMilitaryFormation(snap.Formation) end
  local xp = unit:GetExperience()
  if snap.Name and unit:GetName() ~= snap.Name then xp:SetVeteranName(snap.Name) end
  for _, promotion in ipairs(snap.Promotions or {}) do
    if not xp:HasPromotion(promotion) then xp:SetPromotion(promotion) end
  end
  if snap.XP then xp:ChangeExperience(snap.XP - xp:GetExperiencePoints()) end
  if snap.Religion and snap.Religion >= 0 then
    unit:GetReligion():SetReligionType(GameInfo.Religions[snap.Religion].ReligionType)
  end
  if snap.Damage then unit:SetDamage(snap.Damage) end
  if snap.Moves then unit:ChangeMovesRemaining(snap.Moves - unit:GetMovesRemaining()) end
  -- No documented setter for spent build/spread charges: keep the donor and block if reproduction differs.
  if snap.BuildCharges then assert(unit:GetBuildCharges() == snap.BuildCharges, "spent build charges need adapter") end
  if snap.SpreadCharges then assert(unit:GetSpreadCharges() == snap.SpreadCharges, "spent spread charges need adapter") end
  if snap.GreatPersonCharges then
    assert(unit:GetGreatPerson():GetActionCharges() == snap.GreatPersonCharges, "spent great-person charges need adapter")
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

local function VerifyUnit(unit, snap)
  assert(unit and unit:GetOwner() == snap.Destination and GameInfo.Units[unit:GetType()].UnitType == snap.Type,
    "transferred unit missing/type/owner mismatch: " .. snap.Token)
  assert(unit:GetProperty(R.TOKEN_KEY) == snap.Token, "unit token mismatch")
  local greatPerson = unit:GetGreatPerson()
  assert((greatPerson and greatPerson:GetIndividual() or -1) == snap.Individual, "great person individual changed")
  if snap.GreatPersonCharges then
    assert(greatPerson:GetActionCharges() == snap.GreatPersonCharges, "great person action charges changed")
  end
  assert(R.Equal(Properties(unit, snap.PropertyKeys), snap.TransferredProperties), "unit properties changed: " .. snap.Token)
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
  local before = {}
  for _, unit in ipairs(Units(snap.Destination)) do before[unit:GetID()] = true end
  if snap.Individual >= 0 then
    -- Official TunerMapPanel.lua:190 + Debug/Map.ltp:579: individual INDEX, not hash.
    Game.GetGreatPeople():CreatePerson(snap.Destination, snap.Individual, snap.X, snap.Y)
  else
    -- Returns no Unit object (verified live); discover the new ID afterwards.
    UnitManager.InitUnitValidAdjacentHex(snap.Destination, snap.Type, snap.X, snap.Y, R.UNIT_SEARCH_RADIUS_POC)
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
    if record.Kind == "GREAT_PERSON" and (record.Outcome == "RESEARCH" or record.Outcome == "FAITH" or record.Outcome == "CULTURE") then
      record.ConversionReward = R.CONVERTED_GREAT_PERSON_REWARD_POC
      record.RewardPending = true
    end
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
    if snap.KeepUnit then
      local unit = FindToken(snap.Destination, snap.Token)
      if exactUnits then VerifyUnit(unit, snap) else
        assert(unit and unit:GetOwner() == snap.Destination, "transferred unit disappeared")
      end
    end
  end
  local greatWorks = OwnedGreatWorks(tx.To)
  for _, snap in ipairs(tx.Cities) do
    local city = CityManager.GetCityAt(snap.X, snap.Y)
    assert(city and city:GetOwner() == tx.To, "city transfer failed")
    assert(R.Equal(Properties(city, snap.PropertyKeys), snap.TransferredProperties), "city properties changed")
    assert(R.Equal(CityAssets(city), snap.Assets), "city buildings/Great Works changed")
    if exactUnits then
      assert(city:GetPopulation() == snap.Population and R.Equal(Religion(city), snap.Faith), "city population/religion changed")
    end
    assert(city:GetProperty(R.GOVERNANCE_KEY).From == tx.From, "governance provenance lost")
    assert(R.Equal(city:GetProperty(R.GOVERNANCE_KEY).Previous, snap.Governance), "previous governance lost")
    for _, work in ipairs(snap.PalaceWorks or {}) do assert(greatWorks[work], "Palace Great Work lost: " .. work) end
  end
  for _, index in ipairs(tx.Plots) do assert(Map.GetPlotByIndex(index):GetOwner() == tx.To, "territory transfer failed: " .. index) end
  local rosters, archive = MovedAssets(tx)
  for owner, roster in pairs(rosters) do assert(R.Equal(GetRoster(owner), roster), "roster transfer mismatch") end
  assert(next(GetRoster(tx.From)) == nil, "source roster not cleared")
  for id, record in pairs(archive) do assert(R.Equal(state.Archive[id], record), "archived asset lost") end
  for _, name in ipairs(R.Keys(tx.Adapters)) do
    assert(adapters[name], "re-register absorption adapter after load: " .. name)
    adapters[name].Verify(R.Copy(tx), R.Copy(tx.Adapters[name]))
  end
end

local function Execute(state, tx)
  -- Stage and verify every replacement before removing ANY donor unit or city.
  for _, snap in ipairs(tx.Units) do
    if snap.KeepUnit then
      local unit = FindToken(snap.Destination, snap.Token)
      assert(unit or not tx.UnitMapFrozen, "staged replacement was lost; review recovery: " .. snap.Token)
      if tx.UnitMapFrozen then
        local mapped = assert(tx.UnitMap[R.UnitKey(tx.From, snap.ID)], "frozen unit mapping missing")
        assert(unit:GetID() == mapped.ID, "frozen unit mapping changed")
      end
      unit = unit or Spawn(snap)
      tx.UnitMap[R.UnitKey(tx.From, snap.ID)] = { ID = unit:GetID(), Token = snap.Token }
      Save(state)
    end
  end
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
  for _, snap in ipairs(tx.Units) do
    if snap.KeepUnit then
      local unit = FindToken(snap.Destination, snap.Token)
      RestoreUnit(unit, snap)
      VerifyUnit(unit, snap)
      Save(state)
    end
  end
  tx.Status = "STAGED"
  Save(state)
  for _, snap in ipairs(tx.Units) do
    if snap.ID >= 0 then
      local old = UnitManager.GetUnit(tx.From, snap.ID)
      if old then UnitManager.Kill(old) end -- never use the killed object again (AustraliaScenario.lua:807).
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

function A.Resume(id)
  local state = State()
  local tx = assert(state.Transactions[id], "unknown absorption transaction")
  if tx.Status == "COMPLETE" then return R.Copy(tx) end
  assert(tx.Turn == Game.GetCurrentGameTurn(), "resume in the original turn; later-turn recovery needs review")
  for name in pairs(tx.Adapters) do assert(adapters[name], "missing absorption adapter: " .. name) end
  local ok, result = pcall(Execute, state, tx)
  if not ok then
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
  local tx = Prepare(plan, state)
  state.Transactions[plan.ID] = tx
  Save(state)
  return A.Resume(plan.ID)
end

-- Exact=true is for the same-turn save/load check. Later turns can legitimately change HP, XP, followers etc.
function A.Verify(id, exact)
  local state = State()
  local tx = assert(state.Transactions[id], "unknown absorption transaction")
  assert(tx.Status == "COMPLETE", "absorption did not complete: " .. tx.Status)
  Verify(tx, state, exact == true)
  return true
end

ExposedMembers.JAC_Absorption = A -- gameplay/FireTuner API; no UI mutation or automatic diplomatic hook.
Log("loaded version=1 explicit_dispositions=true")
