-- Read-only Heroes APIs live in UI, not Gameplay. No visible screen is added.
local pending = true -- resume verification once after loading a saved game.
local elapsed, timedOut = {}, {}
local frameChecks, publishedChecks = {}, {} -- read-only diagnostics, not saved
local Bridge = {}
function Bridge.VerificationDiagnostics(playerID)
  local key = tostring(playerID)
  return { elapsed = elapsed[key] or 0, timedOut = timedOut[key] == true,
    frameChecks = frameChecks[key] or 0, publishedChecks = publishedChecks[key] or 0 }
end
function Bridge.OriginBuilding(playerID)
  return Game.GetHeroesManager():GetPlayerHeroOriginBuildingType(playerID)
end
function Bridge.ClaimPlayer(heroClass)
  return Game.GetHeroesManager():GetHeroClaimPlayer(heroClass)
end
function Bridge.UnitInfo(playerID, unitID)
  local p = Players[playerID]
  local u = p and p:GetUnits():FindID(unitID)
  if not u then return nil end
  -- HeroesPopup.lua:265-268 uses the class sentinel to identify a Hero.
  -- UnitPanel.lua:2269 reads remaining lifespan as a unit property, not a getter.
  local heroClass = u:GetHeroClassType()
  local lifespan = u:GetProperty(UnitPropertyKeys.LifespanRemaining)
  local isHero = type(heroClass) == "number" and heroClass >= 0
    and GameInfo.HeroClasses[heroClass] ~= nil
  return { hero = isHero, class = heroClass, lifespan = lifespan,
    claim = isHero and Bridge.ClaimPlayer(heroClass) or -1, active = p:IsTurnActive() }
end
function Bridge.Schedule(playerID)
  local key = tostring(playerID)
  pending, elapsed[key], timedOut[key] = true, 0, nil
end
ExposedMembers.JAC_RP_HeroUI = Bridge
ContextPtr:SetUpdate(function(dt)
  if not pending then return end
  local rp = ExposedMembers.JAC_ReligionPersistence
  if not rp then return end
  pending = false
  for playerID, h in pairs(rp.Export().heroes) do
    if h.verification == "PENDING" and not timedOut[playerID] then
      if Players[tonumber(playerID)]:IsTurnActive() then
        elapsed[playerID] = (elapsed[playerID] or 0) + (dt or 0)
      end
      if (elapsed[playerID] or 0) > 30 then
        timedOut[playerID] = true
        print("[JAC_RP] hero verification timed out for player=" .. playerID .. "; reservation retained")
      else
        frameChecks[playerID] = (frameChecks[playerID] or 0) + 1
        local ok, applied = pcall(rp.CompleteHero, tonumber(playerID))
        if not ok then print("[JAC_RP] hero verification pending: " .. tostring(applied))
        elseif not applied then pending = true end
      end
    end
  end
end)


-- InGame.lua loads every add-in hidden. A hidden context receives no frame
-- updates, even when it has a SetUpdate callback. This empty context has no
-- visual controls or input handler; activate it after the load screen closes.
Events.LoadScreenClose.Add(function()
  pending, elapsed, timedOut = true, {}, {}
  frameChecks, publishedChecks = {}, {}
  ContextPtr:SetHide(false)
  local testFlag = GameConfiguration.GetValue("JAC_AUTOTEST")
  if tonumber(testFlag) == 1 then
    print("[JAC_RP] hero UI frame verification active after load")
  end
end)


-- Stop per-frame work after the deadline, but retry once when the engine
-- publishes new state. A delayed replicated class/lifespan can then complete
-- without a reload. CompleteHero persists APPLYING before paying the reward.
Events.GameCoreEventPublishComplete.Add(function()
  local rp = ExposedMembers.JAC_ReligionPersistence
  if not rp or next(timedOut) == nil then return end
  for playerID in pairs(timedOut) do
    local p = Players[tonumber(playerID)]
    if p and p:IsTurnActive() then
      local h = rp.Export().heroes[playerID]
      if not h or h.verification ~= "PENDING" then
        timedOut[playerID] = nil
      else
        publishedChecks[playerID] = (publishedChecks[playerID] or 0) + 1
        local ok, applied = pcall(rp.CompleteHero, tonumber(playerID))
        if ok and applied then timedOut[playerID] = nil
        elseif not ok then print("[JAC_RP] hero verification on published state pending: " .. tostring(applied)) end
      end
    end
  end
end)
