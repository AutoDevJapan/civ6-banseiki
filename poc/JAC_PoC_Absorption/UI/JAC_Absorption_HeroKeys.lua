-- UnitPropertyKeys exists in the UI context but not in gameplay on the installed engine.
-- Share the engine's actual numeric enum value; never invent a replacement property.
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
    reads.MarkPublished = function(token) observed[token] = { Published = true } end
    reads.ReadCity = function(owner, id, method, building, slot)
      local city = assert(Players[owner]:GetCities():FindID(id), "UI city cache is not ready")
      if method == "RELIGIONS" then
        local rows = {}
        for _, row in pairs(city:GetReligion():GetReligionsInCity()) do
          rows[#rows + 1] = { Religion = row.Religion, Followers = row.Followers }
        end
        return rows
      elseif method == "SLOTS" then return city:GetBuildings():GetNumGreatWorkSlots(building)
      elseif method == "WORK" then return city:GetBuildings():GetGreatWorkInSlot(building, slot) end
      error("unsupported UI city read")
    end
    reads.Observe = function(token, class)
      local origin = reads.GetHeroOriginCityID(class)
      observed[token] = { Claim = reads.GetHeroClaimPlayer(class), Origin = origin }
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
    if tx.AwaitingFinalUI then reads.MarkPublished(tx.ID .. ":final") end
    for _, snap in ipairs(tx.Units or {}) do
      local intent = snap.HeroSpawnIntent
      if intent and intent.AwaitingUI then
        reads.Observe(snap.Token .. ":created:" .. intent.CreatedID, snap.Hero.Class)
      end
    end
  end
  local fixture = api and api.GetUIPhase() or Game:GetProperty("JAC_ABSORPTION_FIXTURE_UI_WAIT")
  if fixture and fixture.Kind == "FIXTURE" then reads.Observe(fixture.Token, fixture.Class)
  elseif fixture and fixture.Kind == "CITY" then reads.MarkPublished(fixture.Token) end
end
Events.GameCoreEventPublishComplete.Add(ObservePendingHeroes)
Events.LoadScreenClose.Add(ObservePendingHeroes)
