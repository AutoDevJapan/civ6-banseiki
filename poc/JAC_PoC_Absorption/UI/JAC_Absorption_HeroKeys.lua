-- UnitPropertyKeys exists in the UI context but not in gameplay on the installed engine.
-- Share the engine's actual numeric enum value; never invent a replacement property.
local uiPlayers, uiGame = Players, Game
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
    reads.GetObservation = function(token) return observed[token] end
    reads.GetUnitObservation = function(token) return observed[token] end
    reads.MarkPublished = function(token) observed[token] = { Published = true } end
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

-- UI caches update after the game-core chunk returns. Observe only at the engine's
-- publish-complete boundary (or after loading), never during the mutation itself.
local function ObservePendingHeroes()
  local reads = ExposedMembers.JAC_HeroReadAPI
  if not reads or not reads.Observe then return end
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
