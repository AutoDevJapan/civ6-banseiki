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
  for _, tx in pairs(state and state.Transactions or {}) do
    if tx.AwaitingPreparation then
      for _, read in pairs(tx.SourceSpreadReads or {}) do
        reads.ObserveUnit(read.Token, read.Owner, read.ID, read.Turn)
      end
      if tx.SourceReadyToken and uiGame.GetCurrentGameTurn() == tx.Turn then reads.MarkPublished(tx.SourceReadyToken) end
    end
    local verification = tx.VerificationUI
    if verification and uiGame.GetCurrentGameTurn() == verification.Turn then reads.MarkPublished(verification.Token) end
    if tx.AwaitingFinalUI then reads.MarkPublished(tx.ID .. ":final") end
    for _, snap in ipairs(tx.Units or {}) do
      local intent = snap.HeroSpawnIntent
      if intent and intent.AwaitingUI then
        reads.Observe(snap.Token .. ":created:" .. intent.CreatedID, snap.Hero.Class)
      end
      for _, spread in pairs(snap.SpreadReads or {}) do
        if spread.AwaitingUI then
          reads.ObserveUnit(spread.Token, spread.Owner, spread.ID, spread.Turn)
        end
      end
    end
  end
  for _, intent in pairs(state and state.Recoveries or {}) do
    if intent.AwaitingUI and intent.ObservationToken and intent.HeroClass then
      reads.Observe(intent.ObservationToken, intent.HeroClass)
    end
  end
  local localID = uiGame.GetLocalPlayer and uiGame.GetLocalPlayer()
  local pendingFriction = false
  for _, player in ipairs(PlayerManager.GetAlive() or {}) do
    for _, city in player:GetCities():Members() do
      local governance = city:GetProperty("JAC_ABSORPTION_GOVERNANCE_V1")
      local request = governance and governance.LoyaltyRead
      if request then
        reads.ObserveCity(request)
        if governance.FrictionPending and request.Owner == localID
          and reads.GetObservation(request.Token) then pendingFriction = true end
      end
    end
  end
  local localPlayer = localID and localID >= 0 and uiPlayers[localID]
  if localPlayer and (not localPlayer.IsTurnActive or localPlayer:IsTurnActive())
    and UI and UI.RequestPlayerOperation then
    local flow = ExposedMembers.JAC_AbsorptionFlow
    local worlds = flow and flow.GetPendingWorlds and flow.GetPendingWorlds(localID) or {}
    for _, world in ipairs(worlds) do
      local token = world.WaitingFor
      local observedHero = reads.GetObservation(token)
      if observedHero and type(observedHero.Claim) == "number"
        and type(observedHero.Origin) == "table" then
        pcall(UI.RequestPlayerOperation, localID, PlayerOperations.EXECUTE_SCRIPT,
          { OnStart = "JAC_AbsorptionFlow_Operation", Command = "RESUME_WORLD",
            WorldID = world.ID, ObservationToken = token })
      end
    end
    if pendingFriction then
      pcall(UI.RequestPlayerOperation, localID, PlayerOperations.EXECUTE_SCRIPT,
        { OnStart = "JAC_AbsorptionFlow_Operation", Command = "RESUME_FRICTION" })
    end
  end
  local fixture = api and api.GetUIPhase() or Game:GetProperty("JAC_ABSORPTION_FIXTURE_UI_WAIT")
  if fixture and fixture.Kind == "FIXTURE" then reads.Observe(fixture.Token, fixture.Class)
  elseif fixture and fixture.Kind == "CITY" then reads.MarkPublished(fixture.Token) end
end
Events.GameCoreEventPublishComplete.Add(ObservePendingHeroes)
Events.LoadScreenClose.Add(ObservePendingHeroes)
