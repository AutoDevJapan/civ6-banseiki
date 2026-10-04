-- UnitPropertyKeys exists in the UI context but not in gameplay on the installed engine.
-- Share the engine's actual numeric enum value; never invent a replacement property.
local uiPlayers, uiGame = Players, Game
local nativeCarrierSent = {}
local nativeCarrierChecks = {}
local function PublishHeroKeys()
  if UnitPropertyKeys and type(UnitPropertyKeys.LifespanRemaining) == "number" then
    ExposedMembers.JAC_HeroLifespanKey = UnitPropertyKeys.LifespanRemaining
  end
  local manager = Game.GetHeroesManager and Game.GetHeroesManager()
  if manager then
    local reads = {}
    for _, name in ipairs({ "GetHeroClaimPlayer", "GetPlayerHeroOriginBuildingType" }) do
      local method = manager[name]
      if method then reads[name] = function(...) return method(manager, ...) end end
    end
    if manager.GetHeroOriginCityID then
      reads.GetHeroOriginCityID = function(class)
        local id = manager:GetHeroOriginCityID(class)
        return id and { player = id.player, id = id.id } or nil
      end
    end
    local observed = {}
    reads.NativeMoveAvailable = UI and type(UI.RequestPlayerOperation) == "function"
      and PlayerOperations and PlayerOperations.MOVE_GREAT_WORK ~= nil or false
    reads.GetObservation = function(token) return observed[token] end
    reads.GetUnitObservation = function(token) return observed[token] end
    reads.MarkPublished = function(token) observed[token] = { Published = true } end
    reads.MarkCarrier = function(token, unchanged)
      observed[token] = { Published = true, Unchanged = unchanged == true }
    end
    reads.ReadUnit = function(owner, id, method, turn)
      assert(method == "SPREAD_CHARGES", "unsupported UI unit read")
      if uiGame.GetCurrentGameTurn() ~= turn then return nil end
      local player = uiPlayers[owner]
      if not player then return nil end
      for _, unit in player:GetUnits():Members() do
        if unit:GetID() == id then
          assert(unit.GetSpreadCharges, "UI spread-charge getter unavailable")
          return unit:GetSpreadCharges()
        end
      end
      return nil
    end
    reads.ObserveUnit = function(token, owner, id, turn)
      local charges = reads.ReadUnit(owner, id, "SPREAD_CHARGES", turn)
      if type(charges) == "number" then observed[token] = charges end
    end
    reads.ReadPlayer = function(owner, method)
      assert(method == "FAITH_BALANCE", "unsupported UI player read")
      return uiPlayers[owner]:GetReligion():GetFaithBalance()
    end
    reads.ReadNativeWork = function(owner, id, index)
      local city = assert(uiPlayers[owner]:GetCities():FindID(id), "UI city cache is not ready")
      local buildings = city:GetBuildings()
      return { Type = buildings:GetGreatWorkTypeFromIndex(index),
        CreatorName = buildings:GetCreatorNameFromIndex(index),
        TurnCreated = buildings:GetTurnFromIndex(index),
        CreatedByPlayer = uiGame.GetGreatWorkPlayer(index) }
    end
    reads.FindCompatibleNativeSlots = function(owner, objectType)
      assert(type(owner) == "number" and type(objectType) == "string", "native slot query requires owner and type")
      local player = assert(uiPlayers[owner], "native slot owner is not published")
      local compatible, slots = {}, {}
      for row in GameInfo.GreatWork_ValidSubTypes() do
        if row.GreatWorkObjectType == objectType then compatible[row.GreatWorkSlotType] = true end
      end
      for _, city in player:GetCities():Members() do
        local buildings = city:GetBuildings()
        for building in GameInfo.Buildings() do
          if buildings:HasBuilding(building.Index)
            and (not buildings.IsPillaged or not buildings:IsPillaged(building.Index)) then
            for slot = 0, buildings:GetNumGreatWorkSlots(building.Index) - 1 do
              local slotInfo = GameInfo.GreatWorkSlotTypes[buildings:GetGreatWorkSlotType(building.Index, slot)]
              if slotInfo and compatible[slotInfo.GreatWorkSlotType]
                and buildings:GetGreatWorkInSlot(building.Index, slot) == -1 then
                slots[#slots + 1] = { Owner = owner, City = city:GetID(), X = city:GetX(), Y = city:GetY(),
                  Building = building.Index, Slot = slot, SlotType = slotInfo.GreatWorkSlotType }
              end
            end
          end
        end
      end
      table.sort(slots, function(a, b)
        if a.City ~= b.City then return a.City < b.City end
        if a.Building ~= b.Building then return a.Building < b.Building end
        return a.Slot < b.Slot
      end)
      return slots
    end
    reads.ReadCarrierPlots = function(owner, id)
      local city = assert(uiPlayers[owner]:GetCities():FindID(id),
        "native carrier city is not published")
      local purchased = assert(Map.GetCityPlots():GetPurchasedPlots(city),
        "native carrier purchased plots are unavailable")
      local result = {}
      for _, index in pairs(purchased) do
        local plot = assert(Map.GetPlotByIndex(index), "native carrier purchased plot missing")
        local buyer = Cities.GetPlotPurchaseCity(plot)
        assert(plot:GetOwner() == owner and buyer and buyer:GetOwner() == owner
          and buyer:GetX() == city:GetX() and buyer:GetY() == city:GetY(),
          "native carrier purchased plot belongs to another city")
        result[#result + 1] = index
      end
      table.sort(result)
      return result
    end
    reads.ReadCity = function(owner, id, method, building, slot, extra)
      local city = assert(uiPlayers[owner]:GetCities():FindID(id), "UI city cache is not ready")
      if method == "RELIGIONS" then
        local rows = {}
        for _, row in pairs(city:GetReligion():GetReligionsInCity()) do
          rows[#rows + 1] = { Religion = row.Religion, Followers = row.Followers }
        end
        return rows
      elseif method == "DISTRICTS" then
        local locations = {}
        for _, district in city:GetDistricts():Members() do
          locations[#locations + 1] = { Type = GameInfo.Districts[district:GetType()].DistrictType,
            X = district:GetX(), Y = district:GetY() }
        end
        return locations
      elseif method == "SLOTS" then return city:GetBuildings():GetNumGreatWorkSlots(building)
      elseif method == "WORK" then return city:GetBuildings():GetGreatWorkInSlot(building, slot)
      elseif method == "PURCHASE_COST" then
        return city:GetGold():GetPurchaseCost(building, slot,
          extra or MilitaryFormationTypes.STANDARD_MILITARY_FORMATION)
      elseif method == "LOYALTY" then
        return city:GetCulturalIdentity():GetLoyalty()
      end
      error("unsupported UI city read")
    end
    reads.Observe = function(token, class)
      local origin = reads.GetHeroOriginCityID(class)
      observed[token] = { Claim = reads.GetHeroClaimPlayer(class), Origin = origin }
    end
    reads.ObserveCity = function(request)
      if observed[request.Token] or uiGame.GetCurrentGameTurn() ~= request.Turn then return end
      local ok, value = pcall(reads.ReadCity, request.Owner, request.ID, "LOYALTY")
      if ok and type(value) == "number" and value >= 0 and value <= 100 then
        observed[request.Token] = { Owner = request.Owner, ID = request.ID,
          Turn = request.Turn, Value = value }
      end
    end
    ExposedMembers.JAC_HeroReadAPI = reads
  end
end
PublishHeroKeys()
Events.LoadScreenClose.Add(PublishHeroKeys)

local function CarrierMismatch(token, reads)
  if not token then return false end
  nativeCarrierChecks[token] = (nativeCarrierChecks[token] or 0) + 1
  if nativeCarrierChecks[token] >= 2 then reads.MarkCarrier(token, true) end
  return true -- one bounded fresh publication can distinguish a stale UI cache
end

local function ObserveNativeCarrier(carrier, reads)
  -- A saved single-player MOVE must not bypass the gameplay borrow guard.
  if GameConfiguration and GameConfiguration.IsAnyMultiplayer
    and GameConfiguration.IsAnyMultiplayer() then return false end
  if not carrier or carrier.Phase == "RESTORED" then return false end
  if carrier.Phase == "PROVED" then
    if not carrier.RefreshRequested then return false end
    local slot = carrier.Route == "FROM_FREE" and carrier.Target or carrier.FinalSlot
    local current = slot and CityManager.GetCityAt(slot.X, slot.Y)
    if current and current:GetOwner() == slot.Owner and current:GetID() == slot.City
      and current:GetBuildings():HasBuilding(slot.Building)
      and current:GetBuildings():GetGreatWorkInSlot(slot.Building, slot.Slot) == carrier.Index then
      reads.MarkPublished(carrier.ReturnToken)
      return true
    end
    return CarrierMismatch(carrier.ReturnToken, reads)
  end
  local token = carrier.Phase == "BORROW" and carrier.BorrowToken
    or carrier.Phase == "MOVE" and carrier.MoveToken or carrier.ReturnToken
  local city = CityManager.GetCityAt(carrier.X, carrier.Y)
  if not city or not city:GetBuildings():HasBuilding(carrier.Building) then
    return CarrierMismatch(token, reads)
  end
  local work = city:GetBuildings():GetGreatWorkInSlot(carrier.Building, carrier.Slot)
  if carrier.Phase == "BORROW" then
    if city:GetOwner() == carrier.Borrower
      and work == (carrier.Route == "FROM_FREE" and carrier.Index or -1) then
      reads.MarkPublished(carrier.BorrowToken)
      return true
    end
    if city:GetOwner() == carrier.Owner then
      local localID = uiGame.GetLocalPlayer and uiGame.GetLocalPlayer()
      local player = type(localID) == "number" and uiPlayers[localID]
      return reads.NativeMoveAvailable and player and player:IsHuman()
        and (not player.IsTurnActive or player:IsTurnActive())
        and not (GameConfiguration and GameConfiguration.IsAnyMultiplayer
          and GameConfiguration.IsAnyMultiplayer())
    end
    return CarrierMismatch(carrier.BorrowToken, reads)
  end
  if carrier.Phase == "RETURN" or carrier.Phase == "RESTORE" then
    local forward = carrier.Route ~= "FROM_FREE"
    local expected = carrier.Phase == "RETURN"
      and (forward and carrier.Index or -1)
      or (forward and -1 or carrier.Index)
    if city:GetOwner() == carrier.Owner
      and work == expected then
      reads.MarkPublished(carrier.ReturnToken)
      return true
    end
    return CarrierMismatch(carrier.ReturnToken, reads)
  end
  if carrier.Phase ~= "MOVE" or city:GetOwner() ~= carrier.Borrower
    or city:GetID() ~= carrier.BorrowedCity then
    return CarrierMismatch(carrier.MoveToken, reads)
  end
  local source = CityManager.GetCityAt(carrier.MoveSource.X, carrier.MoveSource.Y)
  local target = CityManager.GetCityAt(carrier.MoveTarget.X, carrier.MoveTarget.Y)
  if not source or not target or source:GetOwner() ~= carrier.Borrower
    or target:GetOwner() ~= carrier.Borrower
    or source:GetID() ~= carrier.MoveSource.City
    or target:GetID() ~= carrier.MoveTarget.City then
    return CarrierMismatch(carrier.MoveToken, reads)
  end
  local from = source:GetBuildings():GetGreatWorkInSlot(
    carrier.MoveSource.Building, carrier.MoveSource.Slot)
  local to = target:GetBuildings():GetGreatWorkInSlot(
    carrier.MoveTarget.Building, carrier.MoveTarget.Slot)
  if from == -1 and to == carrier.Index then
    reads.MarkCarrier(carrier.MoveToken, false)
    return true
  end
  if from ~= carrier.Index or to ~= -1 then
    return CarrierMismatch(carrier.MoveToken, reads)
  end
  local sourceBuildings, targetBuildings = source:GetBuildings(), target:GetBuildings()
  if not sourceBuildings:HasBuilding(carrier.MoveSource.Building)
    or not targetBuildings:HasBuilding(carrier.MoveTarget.Building)
    or targetBuildings.IsPillaged and targetBuildings:IsPillaged(carrier.MoveTarget.Building) then
    return CarrierMismatch(carrier.MoveToken, reads)
  end
  local workType = sourceBuildings:GetGreatWorkTypeFromIndex(carrier.Index)
  local definition = workType and GameInfo.GreatWorks[workType]
  local slotInfo = GameInfo.GreatWorkSlotTypes[
    targetBuildings:GetGreatWorkSlotType(carrier.MoveTarget.Building, carrier.MoveTarget.Slot)]
  local compatible = false
  for row in GameInfo.GreatWork_ValidSubTypes() do
    if definition and slotInfo and row.GreatWorkObjectType == definition.GreatWorkObjectType
      and row.GreatWorkSlotType == slotInfo.GreatWorkSlotType then compatible = true; break end
  end
  if not compatible then return CarrierMismatch(carrier.MoveToken, reads) end
  local localID = uiGame.GetLocalPlayer and uiGame.GetLocalPlayer()
  local player = type(localID) == "number" and uiPlayers[localID]
  if not player or not player:IsHuman()
    or player.IsTurnActive and not player:IsTurnActive()
    or not UI or not UI.RequestPlayerOperation then return false end
  local sent = nativeCarrierSent[carrier.MoveToken]
  if not sent then
    sent = { Dispatching = true, Checks = 0 }
    nativeCarrierSent[carrier.MoveToken] = sent
    local params = {}
    params[PlayerOperations.PARAM_PLAYER_ONE] = carrier.Borrower
    params[PlayerOperations.PARAM_CITY_SRC] = carrier.MoveSource.City
    params[PlayerOperations.PARAM_CITY_DEST] = carrier.MoveTarget.City
    params[PlayerOperations.PARAM_BUILDING_SRC] = carrier.MoveSource.Building
    params[PlayerOperations.PARAM_BUILDING_DEST] = carrier.MoveTarget.Building
    params[PlayerOperations.PARAM_GREAT_WORK_INDEX] = carrier.Index
    params[PlayerOperations.PARAM_SLOT] = carrier.MoveTarget.Slot
    local ok, value = pcall(UI.RequestPlayerOperation, localID,
      PlayerOperations.MOVE_GREAT_WORK, params)
    sent.Dispatching, sent.Dispatched = false, ok and value ~= false
    return true -- request one later publication before reconciling the work
  end
  if sent.Dispatching then return false end
  sent.Checks = sent.Checks + 1
  if sent.Checks >= 2 then reads.MarkCarrier(carrier.MoveToken, true) end
  return true
end

-- UI caches update after the game-core chunk returns. Observe only at the engine's
-- publish-complete boundary (or after loading), never during the mutation itself.
local function ObservePendingHeroes()
  local reads = ExposedMembers.JAC_HeroReadAPI
  if not reads or not reads.Observe then return end
  reads.NativeMoveAvailable = UI and type(UI.RequestPlayerOperation) == "function"
    and PlayerOperations and PlayerOperations.MOVE_GREAT_WORK ~= nil or false
  local api = ExposedMembers.JAC_Absorption
  local state = api and api.GetState() or Game:GetProperty("JAC_ABSORPTION_STATE_V1")
  local flowAPI = ExposedMembers.JAC_AbsorptionFlow
  local function AutomaticFlow(id)
    local flow = flowAPI and flowAPI.Get(id)
    return flow and flow.Phase == "APPLYING"
  end
  local pendingPublication = false
  for _, tx in pairs(state and state.Transactions or {}) do
    local automatic = AutomaticFlow(tx.ID)
    if tx.AwaitingPreparation then
      for _, read in pairs(tx.SourceSpreadReads or {}) do
        reads.ObserveUnit(read.Token, read.Owner, read.ID, read.Turn)
      end
      if tx.SourceReadyToken and uiGame.GetCurrentGameTurn() == tx.Turn then
        reads.MarkPublished(tx.SourceReadyToken)
        if automatic then pendingPublication = true end
      end
    end
    local verification = tx.VerificationUI
    if verification and uiGame.GetCurrentGameTurn() == verification.Turn then reads.MarkPublished(verification.Token) end
    if tx.AwaitingFinalUI then
      reads.MarkPublished(tx.ID .. ":final")
      if automatic then pendingPublication = true end
    end
    if tx.NativeDealUI and tx.NativeDealUI.Token then
      reads.MarkPublished(tx.NativeDealUI.Token)
      if automatic then pendingPublication = true end
    end
    for _, snap in ipairs(tx.NativeWorks or {}) do
      if ObserveNativeCarrier(snap.Carrier, reads) and automatic then
        pendingPublication = true
      end
    end
    for _, snap in ipairs(tx.Units or {}) do
      local intent = snap.HeroSpawnIntent
      if intent and intent.AwaitingUI then
        reads.Observe(snap.Token .. ":created:" .. intent.CreatedID, snap.Hero.Class)
        local value = reads.GetObservation(snap.Token .. ":created:" .. intent.CreatedID)
        if automatic and value and type(value.Claim) == "number" and type(value.Origin) == "table" then
          pendingPublication = true
        end
      end
      for _, spread in pairs(snap.SpreadReads or {}) do
        if spread.AwaitingUI then
          reads.ObserveUnit(spread.Token, spread.Owner, spread.ID, spread.Turn)
          if automatic and reads.GetUnitObservation(spread.Token) ~= nil then pendingPublication = true end
        end
      end
    end
  end
  for token, intent in pairs(state and state.Recoveries or {}) do
    if intent.NativeCarrier then
      local published = ObserveNativeCarrier(intent.NativeCarrier, reads)
      local world = flowAPI and flowAPI.GetWorld((token:gsub("^recovery:", "")))
      if published and world and world.Status == "CLAIMED" then pendingPublication = true end
    end
    if intent.NativeDealUI and intent.NativeDealUI.Token then
      reads.MarkPublished(intent.NativeDealUI.Token)
      local world = flowAPI and flowAPI.GetWorld((token:gsub("^recovery:", "")))
      if world and world.Status == "CLAIMED"
        and world.WaitingFor == intent.NativeDealUI.Token then pendingPublication = true end
    end
    if intent.AwaitingUI and intent.ObservationToken and intent.HeroClass then
      reads.Observe(intent.ObservationToken, intent.HeroClass)
      local value = reads.GetObservation(intent.ObservationToken)
      local world = flowAPI and flowAPI.GetWorld((token:gsub("^recovery:", "")))
      if world and world.Status == "CLAIMED" and world.WaitingFor == intent.ObservationToken
        and value and type(value.Claim) == "number" and type(value.Origin) == "table" then
        pendingPublication = true
      end
    end
  end
  local localID = uiGame.GetLocalPlayer and uiGame.GetLocalPlayer()
  for _, player in ipairs(PlayerManager.GetAlive() or {}) do
    for _, city in player:GetCities():Members() do
      local governance = city:GetProperty("JAC_ABSORPTION_GOVERNANCE_V1")
      local request = governance and governance.LoyaltyRead
      if request then
        reads.ObserveCity(request)
        if reads.GetObservation(request.Token)
          and (governance.FrictionPending or AutomaticFlow(governance.ID)) then
          pendingPublication = true
        end
      end
    end
  end
  local localPlayer = localID and localID >= 0 and uiPlayers[localID]
  if localPlayer and (not localPlayer.IsTurnActive or localPlayer:IsTurnActive())
    and UI and UI.RequestPlayerOperation then
    if pendingPublication then
      -- The sender only announces a publication. Gameplay chooses which saved
      -- intents are ready, including AI recipients, without trusting UI IDs.
      pcall(UI.RequestPlayerOperation, localID, PlayerOperations.EXECUTE_SCRIPT,
        { OnStart = "JAC_AbsorptionFlow_Operation", Command = "RECONCILE_PUBLICATIONS" })
    end
  end
  local fixture = api and api.GetUIPhase() or Game:GetProperty("JAC_ABSORPTION_FIXTURE_UI_WAIT")
  if fixture and fixture.Kind == "FIXTURE" then reads.Observe(fixture.Token, fixture.Class)
  elseif fixture and fixture.Kind == "CITY" then reads.MarkPublished(fixture.Token) end
end
Events.GameCoreEventPublishComplete.Add(ObservePendingHeroes)
Events.LoadScreenClose.Add(ObservePendingHeroes)
Events.LocalPlayerTurnBegin.Add(ObservePendingHeroes)
