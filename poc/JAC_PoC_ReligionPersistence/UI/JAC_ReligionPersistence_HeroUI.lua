-- Read-only Heroes APIs live in UI, not Gameplay. No visible screen is added.
local pending = true -- resume verification once after loading a saved game.
local elapsed, timedOut = {}, {}
local frameChecks, publishedChecks = {}, {} -- read-only diagnostics, not saved
local inFlight = {}
local Bridge = {}
-- Official BlackDeathScenario_PopupHandler.lua:22-28. Only the local human
-- submits; spectators and other clients never submit for the receipt owner.
local function send(params)
  local actor = Game.GetLocalPlayer()
  local p = Players[actor]
  local rp = ExposedMembers.JAC_ReligionPersistence
  if not p or not p:IsHuman() or not p:IsTurnActive() or not rp then return false end
  params.OnStart, params.Turn = "JAC_ReligionOperation", Game.GetCurrentGameTurn()
  params.Revision = rp.Export().revision
  UI.RequestPlayerOperation(actor, PlayerOperations.EXECUTE_SCRIPT, params)
  return true -- queued, not proof of applied effects
end
function Bridge.ResolveDiscovery(id, discovery, outcome, options)
  options = options or {}
  local params = { Action="RESOLVE", Religion=id, Discovery=discovery, Outcome=outcome,
    City=options.city, Revival=options.revival, Community=options.community,
    TargetReligion=options.targetReligion, TargetPlayer=options.targetPlayer }
  if outcome == "HERO" then
    local p = Players[Game.GetLocalPlayer()]
    if not p or not p:IsHuman() or not p:IsTurnActive() then return false end
    local candidates = {}
    for row in GameInfo.HeroClasses() do
      if Bridge.ClaimPlayer(row.Index) == -1 then candidates[#candidates+1] = row end
    end
    table.sort(candidates, function(a,b) return a.HeroClassType < b.HeroClassType end)
    if not candidates[1] then return false end
    params.HeroClass, params.HeroClaim = candidates[1].Index, -1
  end
  return send(params)
end
function Bridge.Revive(id, revival, source, community, city)
  return send({ Action="REVIVE", Religion=id, Revival=revival, Source=source,
    Community=community, City=city })
end
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
local function complete(playerID)
  -- Only the legitimate receiver publishes scalar observations. Gameplay
  -- independently checks its saved intent and real Unit; other peers need not
  -- have received the same UI replica yet.
  if Game.GetLocalPlayer() ~= playerID then return false end
  local rp = ExposedMembers.JAC_ReligionPersistence
  local state = rp.Export()
  local h = state.heroes[tostring(playerID)]
  if not h or h.verification ~= "PENDING" then return true end
  local info = Bridge.UnitInfo(playerID, h.unit)
  if not info or not info.active or not info.hero or info.class ~= h.heroClass
    or type(info.lifespan) ~= "number" or info.lifespan <= 0 or info.claim ~= playerID then return false end
  local token = tostring(state.revision) .. ":" .. tostring(h.unit)
  if inFlight[playerID] ~= token then
    inFlight[playerID] = token
    local d = state.religions[h.religion].discoveries[h.discovery]
    local city, reservedTurn = h.city or d.effect.city, h.reservedTurn or d.resolvedTurn
    local reservationToken = h.token or h.religion .. ":" .. h.discovery .. ":" .. playerID .. ":" .. city .. ":" .. reservedTurn
    if not send({Action="COMPLETE_HERO", Religion=h.religion,
      Discovery=h.discovery, Unit=h.unit, City=city, ReservedTurn=reservedTurn, Token=reservationToken,
      ObservedClass=info.class, ObservedClaim=info.claim, ObservedLifespan=info.lifespan}) then inFlight[playerID] = nil end
  end
  -- Wait for published Gameplay state; a queued operation may be rejected.
  h = rp.Export().heroes[tostring(playerID)]
  return h == nil or h.verification ~= "PENDING"
end
ExposedMembers.JAC_RP_HeroUI = Bridge
ContextPtr:SetUpdate(function(dt)
  if not pending then return end
  local rp = ExposedMembers.JAC_ReligionPersistence
  if not rp then return end
  pending = false
  for playerID, h in pairs(rp.Export().heroes) do
    if tonumber(playerID) == Game.GetLocalPlayer()
      and h.verification == "PENDING" and not timedOut[playerID] then
      if Players[tonumber(playerID)]:IsTurnActive() then
        elapsed[playerID] = (elapsed[playerID] or 0) + (dt or 0)
      end
      if (elapsed[playerID] or 0) > 30 then
        timedOut[playerID] = true
        print("[JAC_RP] hero verification timed out for player=" .. playerID .. "; reservation retained")
      else
        frameChecks[playerID] = (frameChecks[playerID] or 0) + 1
        local ok, applied = pcall(complete, tonumber(playerID))
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
  inFlight = {}
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
  inFlight = {} -- rejected or not-yet-ready requests may retry on new core state
  pending = true -- recheck the new local owner after handover/turn publication
  local rp = ExposedMembers.JAC_ReligionPersistence
  if not rp or next(timedOut) == nil then return end
  for playerID in pairs(timedOut) do
    local p = Players[tonumber(playerID)]
    if tonumber(playerID) == Game.GetLocalPlayer() and p and p:IsTurnActive() then
      local h = rp.Export().heroes[playerID]
      if not h or h.verification ~= "PENDING" then
        timedOut[playerID] = nil
      else
        publishedChecks[playerID] = (publishedChecks[playerID] or 0) + 1
        local ok, applied = pcall(complete, tonumber(playerID))
        if ok and applied then timedOut[playerID] = nil
        elseif not ok then print("[JAC_RP] hero verification on published state pending: " .. tostring(applied)) end
      end
    end
  end
end)
