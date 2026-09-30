-- Read-only Heroes APIs live in UI, not Gameplay. No visible screen is added.
local pending = true -- resume verification once after loading a saved game.
local elapsed = 0
local Bridge = {}
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
  return { hero = u:IsHero(), class = u:GetHeroClassType(), lifespan = u:GetLifespan(),
    claim = Bridge.ClaimPlayer(u:GetHeroClassType()), active = p:IsTurnActive() }
end
function Bridge.Schedule() pending, elapsed = true, 0 end
ExposedMembers.JAC_RP_HeroUI = Bridge
ContextPtr:SetUpdate(function(dt)
  if not pending then return end
  if elapsed > 30 then
    pending = false
    print("[JAC_RP] hero verification timed out; reservation retained")
    return
  end
  local rp = ExposedMembers.JAC_ReligionPersistence
  if not rp then return end
  pending = false
  local active = false
  for playerID, h in pairs(rp.Export().heroes) do
    if h.verification == "PENDING" then
      active = active or Players[tonumber(playerID)]:IsTurnActive()
      local ok, applied = pcall(rp.CompleteHero, tonumber(playerID))
      if not ok then print("[JAC_RP] hero verification pending: " .. tostring(applied))
      elseif not applied then pending = true end
    end
  end
  if active then elapsed = elapsed + (dt or 0) end
end)
