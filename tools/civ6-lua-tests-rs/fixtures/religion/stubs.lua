
local tables, initial, engine, modifiers = ...
local function clone(v)
  if type(v) ~= 'table' then return v end
  local out = {}
  for k, x in pairs(v) do out[k] = clone(x) end
  return out
end
GameInfo = {}
for name, rows in pairs(tables) do
  GameInfo['JAC_RP_' .. name] = setmetatable({}, { __call = function()
    local n = 0
    return function() n = n + 1; return rows[n] end
  end })
end
PROPERTY_STORE = { JAC_RP_STATE_V1 = initial }
WRITES, TURN, LOG = 0, 17, {}
FAIL_WRITE = false
FAIL_WRITE_AT = nil
Game = {
  GetProperty = function(_, k) return PROPERTY_STORE[k] end,
  SetProperty = function(_, k, v)
    if FAIL_WRITE or FAIL_WRITE_AT == WRITES + 1 then error('engine refused write') end
    if v == '' then v = nil end
    PROPERTY_STORE[k] = clone(v)
    WRITES = WRITES + 1
  end,
  GetCurrentGameTurn = function() return TURN end,
}
ExposedMembers = {}
ENGINE = engine or {}
FAIL_EFFECT, FAIL_SPAWN, BAD_CONVERSION = false, false, false
GameEvents = { OnGameTurnStarted = { Add = function(f) TURN_HOOK = f end } }
GameConfiguration = { GetValue = function() return HEROES_MODE end }
HEROES_MODE = true
local function info(rows, key)
  local t = setmetatable({}, { __call = function()
    local n = 0; return function() n = n + 1; return rows[n] end
  end })
  for _, row in ipairs(rows) do t[row.Index] = row; t[row[key]] = row end
  return t
end
GameInfo.Technologies = info({
  { Index = 1, TechnologyType = 'TECH_Z', Cost = 200 },
  { Index = 2, TechnologyType = 'TECH_A', Cost = 200 },
  { Index = 3, TechnologyType = 'TECH_M', Cost = 200 },
  { Index = 4, TechnologyType = 'TECH_B', Cost = 200 },
  { Index = 5, TechnologyType = 'TECH_FUTURE', Cost = 200 },
}, 'TechnologyType')
GameInfo.Boosts = info({ { Index = 1, TechnologyType = 'TECH_A' },
  { Index = 2, TechnologyType = 'TECH_B' }, { Index = 3, TechnologyType = 'TECH_M' },
  { Index = 4, TechnologyType = 'TECH_Z' } }, 'TechnologyType')
GameInfo.HeroClasses = info({
  { Index = 10, HeroClassType = 'HEROCLASS_ANANSI', UnitType = 'UNIT_HERO_ANANSI', CreationProjectType = 'PROJECT_CREATE_HERO_ANANSI' },
  { Index = 11, HeroClassType = 'HEROCLASS_ARTHUR', UnitType = 'UNIT_HERO_ARTHUR', CreationProjectType = 'PROJECT_CREATE_HERO_ARTHUR' },
}, 'HeroClassType')
GameInfo.ProjectCompletionModifiers = info({
  { Index = 1, ProjectType = 'PROJECT_CREATE_HERO_ANANSI', ModifierId = 'MODIFIER_CREATE_HERO_ANANSI' },
  { Index = 2, ProjectType = 'PROJECT_CREATE_HERO_ARTHUR', ModifierId = 'MODIFIER_CREATE_HERO_ARTHUR' },
}, 'ProjectType')
GameInfo.Modifiers = modifiers
for _, row in ipairs({ 'ANANSI', 'ARTHUR' }) do
  GameInfo.Modifiers['MODIFIER_CREATE_HERO_' .. row] = { ModifierType = 'MODIFIER_CITY_GRANT_HERO' }
end
GameInfo.Units = { UNIT_HERO_ANANSI = { Index = 100 }, UNIT_HERO_ARTHUR = { Index = 101 }, UNIT_SCOUT = { Index = 5 } }
GameInfo.Buildings = { BUILDING_MONUMENT = { Index = 1 } }
local function nativeRows(c)
  local rows, total, used = {}, 0, 0
  for _, pressure in pairs(c.pressure) do total = total + pressure end
  if total == 0 then c.pressure['-1'] = 100; total = 100 end
  local ordered = {}
  for religion, pressure in pairs(c.pressure) do
    if pressure > 0 then
      local raw = c.population * pressure / total
      local n = math.floor(raw)
      rows[#rows + 1] = { Religion = tonumber(religion), Followers = n, Pressure = pressure }
      ordered[#ordered + 1] = { row = rows[#rows], remainder = raw - n }
      used = used + n
    end
  end
  table.sort(ordered, function(a, b)
    return a.remainder > b.remainder or a.remainder == b.remainder and a.row.Religion < b.row.Religion
  end)
  for i = 1, c.population - used do ordered[i].row.Followers = ordered[i].row.Followers + 1 end
  if OMIT_UNAFFILIATED_ROWS then
    local filtered = {}
    for _, row in ipairs(rows) do if row.Religion > 0 then filtered[#filtered + 1] = row end end
    return filtered
  end
  return rows
end
NativeRows = nativeRows
HERO_UI_HIDDEN = true
Events = { LoadScreenClose = { Add = function(fn) HERO_UI_LOAD_CLOSE = fn end },
  GameCoreEventPublishComplete = { Add = function(fn) HERO_UI_PUBLISH = fn end } }
ContextPtr = {
  SetHide = function(_, hidden) HERO_UI_HIDDEN = hidden end,
  SetUpdate = function(_, fn)
    HERO_UI_TICK = fn
    HERO_UI_FRAME = function(dt) if not HERO_UI_HIDDEN then fn(dt) end end
  end,
}
-- Native founding is a scenario fixture only. These stubs check its call path;
-- whether the actual engine permits it remains a live-scenario assertion.
UnitPropertyKeys = { LifespanRemaining = "LifespanRemaining" }
NATIVE_RELIGION = 42
local holy = {}
Players = {}
for id = 0, 1 do
  local pid = id
  local e = ENGINE[tostring(pid)] or { faith = 500, faithYield = 3.25, gold = 100, science = 0, culture = 0,
    favor = 10, units = {}, nextUnit = 1, alive = true, major = true, met = true, war = false,
    allied = false, city = true, nativeReligion = 42 + pid, researched = {}, boosted = {},
    heroClaims = {}, cities = { ['0'] = { population = pid == 0 and 7 or 4,
      pressure = { [tostring(42 + pid)] = (pid == 0 and 7 or 4) * 1024 }, monument = true } } }
  ENGINE[tostring(pid)] = e
  local cityObjects = {}
  for cid, c in pairs(e.cities) do
    local city = { GetID = function() return tonumber(cid) end, GetOwner = function() return pid end,
      GetX = function() return 7 end, GetY = function() return 8 end,
      GetPopulation = function() return c.population end,
      ChangePopulation = function(_, n) c.population = c.population + n end,
      GetBuildings = function() return { HasBuilding = function(_, index) return index == 1 and c.monument end } end,
      GetBuildQueue = function() return { CreateIncompleteBuilding = function(_, index, percentage)
        assert(index == 1 and percentage == 100); c.monument = true end } end,
      GetReligion = function() return {
        GetNumFollowers = function(_, r)
          for _, row in ipairs(nativeRows(c)) do if row.Religion == r then return row.Followers end end
          return 0
        end,
        RemovePressureOneReligion = function(_, r, amount, responsible)
          assert(r > 0)
          assert(amount >= 0 and amount <= 8388607 and responsible == -1)
          if FAIL_EFFECT then error('conversion refused') end
          if not BAD_CONVERSION then c.pressure[tostring(r)] = math.max(0, (c.pressure[tostring(r)] or 0) - amount) end
        end,
        AddReligiousPressure = function(_, source, r, delta, unit)
          assert(source == 0 and unit == -1)
          assert(delta >= 0 and delta <= 8388607, 'native fixed point pressure must not overflow')
          assert(r > 0, 'unaffiliated pressure must remain engine-owned')
          if FAIL_EFFECT then error('conversion refused') end
          if BAD_CONVERSION then return end
          local key = tostring(r); c.pressure[key] = math.max(0, (c.pressure[key] or 0) + delta)
        end,
        SetAllCityToReligion = function(_, r) c.pressure = { [tostring(r)] = c.population * 1024 } end,
      } end,
      AttachModifierByID = function(_, modifier)
        assert(modifier == 'MODIFIER_CREATE_HERO_ANANSI' or modifier == 'MODIFIER_CREATE_HERO_ARTHUR')
        if FAIL_EFFECT then error('hero refused') end
        if FAIL_SPAWN then return end
        local class = modifier == 'MODIFIER_CREATE_HERO_ANANSI' and 10 or 11
        assert((ENGINE['0'].heroClaims[tostring(class)] or -1) == -1)
        ENGINE['0'].heroClaims[tostring(class)] = pid
        e.units[tostring(e.nextUnit)] = { id = e.nextUnit, kind = class + 90, class = class, lifespan = 30 }
        e.nextUnit = e.nextUnit + 1
      end,
    }
    cityObjects[cid] = city
  end
  local capital = cityObjects['0']
  local religion = { SetHolyCity = function(_, city) holy[tostring(pid)] = city end,
    GetReligionTypeCreated = function() return e.nativeReligion end,
    GetFaithYield = function() return e.faithYield end,
    GetFaithBalance = function() return e.faith end,
    ChangeFaithBalance = function(_, amount) if FAIL_EFFECT then error('effect refused') end; e.faith = e.faith + amount end }
  Players[pid] = {
    GetID = function() return pid end, IsAlive = function() return e.alive end,
    IsTurnActive = function() return e.active ~= false end,
    IsMajor = function() return e.major end, IsHuman = function() return pid == 0 end,
    GetCities = function() return { GetCapitalCity = function() return e.city and capital or nil end,
      FindID = function(_, city) return e.city and cityObjects[tostring(city)] or nil end,
      Members = function() local key; return function()
        if not e.city then return end; key = next(cityObjects, key); if key then return tonumber(key), cityObjects[key] end
      end end } end,
    GetReligion = function() return religion end,
    GetTechs = function() return {
      SetTech = function(_, index, acquired) e.researched[tostring(index)] = acquired end,
      HasTech = function(_, index) return e.researched[tostring(index)] == true end,
      HasBoostBeenTriggered = function(_, index) return e.boosted[tostring(index)] == true end,
      TriggerBoost = function(_, index, source)
        assert(source == 1 and not e.researched[tostring(index)] and not e.boosted[tostring(index)])
        if FAIL_EFFECT then error('boost refused') end; e.boosted[tostring(index)] = true
      end,
    } end,
    GetCulture = function() return { SetCivic = function() end } end,
    GetTreasury = function() return {
      GetGoldBalance = function() return e.gold end,
      ChangeGoldBalance = function(_, amount) if FAIL_EFFECT then error('effect refused') end; e.gold = e.gold + amount end } end,
    GetDiplomacy = function() return { HasMet = function() return e.met end,
      SetHasMet = function() e.met = true end, MakePeaceWith = function() e.war = false end,
      IsAtWarWith = function() return e.war end, GetFavor = function() return e.favor end,
      HasAllied = function() return e.allied and (e.allianceTurn == nil or TURN - e.allianceTurn < 30) end,
      SetHasAllied = function(_, other, enabled)
        assert(enabled == true); e.allied = true; ENGINE[tostring(other)].allied = true
        e.allianceTurn = TURN; ENGINE[tostring(other)].allianceTurn = TURN
      end,
      ChangeFavor = function(_, amount) if FAIL_EFFECT then error('effect refused') end; e.favor = e.favor + amount end } end,
    GetUnits = function() return {
      FindID = function(_, unit) local u = e.units[tostring(unit)]; return u and {
        GetID = function() return u.id end, GetType = function() return u.kind end,
        GetHeroClassType = function() return u.class or -1 end,
        GetProperty = function(_, key) assert(key == UnitPropertyKeys.LifespanRemaining); return u.lifespan end } end,
      Members = function() local key; return function()
        key = next(e.units, key); if key == nil then return end
        local u = e.units[key]; return u.id, {
          GetID = function() return u.id end, GetType = function() return u.kind end,
          GetHeroClassType = function() return u.class or -1 end,
          GetProperty = function(_, key) assert(key == UnitPropertyKeys.LifespanRemaining); return u.lifespan end }
      end end } end,
    AttachModifierByID = function(_, modifier)
      if FAIL_EFFECT then error('effect refused') end
      local args = assert(modifiers[modifier])
      local yield = string.lower(args.YieldType:sub(7))
      e[yield] = e[yield] + tonumber(args.Amount)
    end,
  }
end
GameInfo.Civics = function() return function() return nil end end
Game.GetPlayers = function()
  local all = {}; for id = 0, 1 do if ENGINE[tostring(id)].alive then all[#all + 1] = Players[id] end end; return all
end
PlayerManager = { GetAlive = Game.GetPlayers }
Game.GetHeroesManager = function() return {
  GetPlayerHeroOriginBuildingType = function() return 1 end,
  GetHeroClaimPlayer = function(_, index) assert(GameInfo.HeroClasses[index] ~= nil); return ENGINE['0'].heroClaims[tostring(index)] or -1 end,
} end
GameInfo.Religions = info({ { Index = 42, ReligionType = 'RELIGION_CUSTOM_1' },
  { Index = 43, ReligionType = 'RELIGION_CUSTOM_2' }, { Index = 44, ReligionType = 'RELIGION_CUSTOM_3' },
  { Index = 2, ReligionType = 'RELIGION_ORDER_2' }, { Index = 10, ReligionType = 'RELIGION_ORDER_10' } }, 'ReligionType')
GameInfo.Beliefs = { BELIEF_CATHEDRAL = { Index = 9 } }
local native = {
  FoundReligion = function(_, player, religion)
    assert(holy[tostring(player)] == Players[player]:GetCities():GetCapitalCity())
    ENGINE[tostring(player)].nativeReligion = religion
    if player == 0 then NATIVE_RELIGION = religion end
  end,
  AddBelief = function(_, player, belief) assert(player >= 0 and player <= 1 and belief == 9) end,
  GetReligions = function() return { { Religion = 42 }, { Religion = 43 }, { Religion = 44 } } end,
}
Game.GetReligion = function() return native end
function print(line) LOG[#LOG + 1] = line end
