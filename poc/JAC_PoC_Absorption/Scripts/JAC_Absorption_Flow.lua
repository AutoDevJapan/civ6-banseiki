-- Gameplay only. Load after JAC_Absorption_Gameplay.lua. UI sends
-- EXECUTE_SCRIPT requests and reads detached snapshots; it never rolls.
include("JAC_Absorption_Rules")
local R = JAC_AbsorptionRules
local function Multiplayer()
  return GameConfiguration and type(GameConfiguration.IsAnyMultiplayer) == "function"
    and GameConfiguration.IsAnyMultiplayer() == true
end
local A = assert(JAC_AbsorptionGameplay, "absorption engine must load first")
local F = {}
local KEY = "JAC_ABSORPTION_FLOW_V1"
local DEBUG_KEY = "JAC_ABSORPTION_FLOW_DEBUG_V1"
local TEST_KEY = "JAC_ABSORPTION_FLOW_TEST_MODE"

local FIRST, METHOD_ACTION, ACTION = R.FLOW_FIRST_ESCAPE,
  R.FLOW_METHOD_ACTION, R.FLOW_ACTION_SUCCESS
local RATE = R.FLOW_RATE
local CHOICES = {
  UNIT = { "SURRENDER", "INCORPORATE", "DISSOLVE" },
  PERSON = { "CAPTURE", "RECRUIT", "SUBMIT", "EXECUTE" },
  GOVERNOR = { "CAPTURE", "RECRUIT", "SUBMIT" },
  HERO = { "RECRUIT", "PROTECT", "RECONTRACT", "SUICIDE" },
  GREAT_PERSON = { "RECRUIT" }, -- no conversion to yields
  ITEM = { "TRANSFER", "LOOT", "SEIZE", "RETURN" },
  UNLOCK = { "INHERIT", "PRESERVE" },
  RELIGION = { "PRESERVE" },
}
local ESCAPE = {
  UNIT = { "ESCAPE" },
  PERSON = { "EXILE", "RETIRE", "DEFECT", "HIDE" },
  GOVERNOR = { "EXILE", "RETIRE", "DEFECT", "HIDE" },
  HERO = { "HIDE", "DEFECT" },
  GREAT_PERSON = { "EXILE", "RETIRE", "HIDE" },
  ITEM = { "SCATTER" },
}
local FALLBACK = {
  UNIT = "SURRENDER", PERSON = "CAPTURE", GOVERNOR = "CAPTURE", HERO = "PROTECT",
  GREAT_PERSON = "RECRUIT", ITEM = "SEIZE", UNLOCK = "PRESERVE",
  RELIGION = "PRESERVE",
}
local function State()
  local state = R.Copy(Game:GetProperty(KEY) or { Version = 1, Flows = {}, World = {} })
  assert(state.Version == 1, "unsupported absorption flow save")
  state.World = state.World or {}
  return state
end
local function Save(state) Game:SetProperty(KEY, R.Copy(state)) end
local function HasSurvivingCity(owner)
  local player = type(owner) == "number" and Players[owner]
  return player and player:IsAlive() and player:GetCities():GetCount() > 0
end
local function ValidReturnOwner(flow, owner)
  return owner ~= flow.From and owner ~= flow.To and HasSurvivingCity(owner)
end
local function NeedsReturnOwner(flow, entry)
  -- Before P3 validate the choice; afterwards validate only the frozen result.
  -- A failed RETURN that became SEIZE/SCATTER no longer has that recipient.
  if flow.FinalRollsComplete or flow.AppliedPlan then return entry.FinalOutcome == "RETURN" end
  return entry.SelectedAction == "RETURN"
end
local function Clamp(value, lo, hi) return math.max(lo, math.min(hi, value)) end
local function DebugAllowed()
  return Game:GetProperty(TEST_KEY) == 1 and GameConfiguration
    and GameConfiguration.IsAnyMultiplayer and not Multiplayer()
end
local function DebugState()
  if not DebugAllowed() then return nil end
  return R.Copy(Game:GetProperty(DEBUG_KEY) or { Rolls = {}, Delay = 2 })
end
local function Roll(state, flow, entry, stage)
  entry.Rolls = entry.Rolls or {}
  if entry.Rolls[stage] then return entry.Rolls[stage].Used end
  local label = "JAC absorption " .. stage .. " " .. flow.ID .. "/" .. entry.ID
  local raw = Game.GetRandNum(RATE.RollRange, label)
  local debug = DebugState()
  local key = flow.ID .. ":" .. entry.ID .. ":" .. stage
  local forced = debug and debug.Rolls[key]
  local used = forced == nil and raw or forced
  assert(type(used) == "number" and used >= 0 and used < RATE.RollRange, "invalid roll")
  entry.Rolls[stage] = { Raw = raw, Used = used, Label = label,
    Debug = forced ~= nil, Turn = Game.GetCurrentGameTurn() }
  Save(state) -- durably journal the call before an asset transition
  return used
end
local function Attitude(entry, recipient)
  local source = entry.Source or {}
  local loyalty = Clamp(tonumber(source.Loyalty) or RATE.NeutralAttitude,
    RATE.AttitudeMinimum, RATE.AttitudeMaximum)
  local relation = source.Relations and source.Relations[tostring(recipient)]
    or source.Relationship
  relation = Clamp(tonumber(relation) or RATE.NeutralAttitude,
    RATE.AttitudeMinimum, RATE.AttitudeMaximum)
  local contract = source.Contract
  local contractOffset = (contract == "PROTECT" or contract == "RECONTRACT")
      and RATE.ProtectedEscapeDelta
    or contract == "BROKEN" and RATE.BrokenEscapeDelta or 0
  return loyalty, relation, contractOffset
end
local function FirstChance(flow, entry, retry)
  local loyalty, relation, contract = Attitude(entry, flow.To)
  local base = FIRST[flow.Mode]
    + (RATE.NeutralAttitude - loyalty) * RATE.LoyaltyEscapeWeight
    + (RATE.NeutralAttitude - relation) * RATE.RelationEscapeWeight + contract
  if entry.Kind == "ITEM" then
    base = math.floor(base * RATE.ItemEscapeNumerator / RATE.ItemEscapeDenominator)
  end
  if retry then base = base + RATE.RetryEscapeDelta end
  return Clamp(base, 0, RATE.EscapeMaximum)
end
local function ActionChance(flow, entry)
  local loyalty, relation, contract = Attitude(entry, flow.To)
  local base = assert(ACTION[entry.SelectedAction], "action chance missing")
  if entry.Kind == "UNLOCK" or entry.Kind == "RELIGION" then return RATE.RollRange end
  return Clamp(base + METHOD_ACTION[flow.Mode]
    + (loyalty - RATE.NeutralAttitude) * RATE.LoyaltyActionWeight
    + (relation - RATE.NeutralAttitude) * RATE.RelationActionWeight - contract,
    RATE.ActionMinimum, RATE.ActionMaximum)
end
local function ResolveEscape(state, flow, entry, prefix)
  local roll = Roll(state, flow, entry, prefix .. "_ESCAPE")
  local chance = FirstChance(flow, entry, prefix == "P3_RETRY")
  entry[prefix .. "Chance"] = chance
  Save(state)
  if roll >= chance then return nil end
  local options = assert(ESCAPE[entry.Kind], "escape list missing")
  local branch = Roll(state, flow, entry, prefix .. "_BRANCH")
  return options[math.floor(branch * #options / RATE.RollRange) + 1]
end
local UnitKind = R.UnitKind
local function NativePersonID(owner, unit)
  local person = unit:GetProperty(R.PERSON_KEY)
  return person and person.ID or ("native-person:" .. owner .. ":" .. unit:GetID())
end
local function Portrait(kind, unit, record)
  if record and record.Portrait then return record.Portrait end
  if unit and kind == "GREAT_PERSON" then
    local gp = unit:GetGreatPerson()
    local individual = gp and gp:GetIndividual() or -1
    local row = individual >= 0 and GameInfo.GreatPersonIndividuals[individual]
    if row and row.GreatPersonIndividualType then return "ICON_" .. row.GreatPersonIndividualType end
  end
  if unit then return "ICON_" .. GameInfo.Units[unit:GetType()].UnitType end
  return nil
end
local function InventoryLocal(plan)
  local roster, entries, linked = A.GetRoster(plan.From), {}, {}
  for id, record in pairs(roster) do
    if record.Unit and record.Unit.Owner == plan.From and record.Unit.ID >= 0 then
      assert(not linked[record.Unit.ID], "multiple identities refer to the same unit")
      linked[record.Unit.ID] = id
    end
  end
  local units = {}
  for _, unit in Players[plan.From]:GetUnits():Members() do units[#units + 1] = unit end
  table.sort(units, function(a, b) return a:GetID() < b:GetID() end)
  local found = {}
  for _, unit in ipairs(units) do
    local kind = UnitKind(unit)
    local id = linked[unit:GetID()]
    if kind ~= "UNIT" then id = id or NativePersonID(plan.From, unit) end
    id = id or ("unit:" .. plan.From .. ":" .. unit:GetID())
    local record = roster[id] or unit:GetProperty(R.PERSON_KEY)
    if record and record.Kind ~= kind and kind == "PERSON"
      and (record.Kind == "UNIT" or record.Kind == "GREAT_PERSON") then
      record = R.Copy(record)
      record.FormerKind, record.Kind = record.Kind, "PERSON"
    end
    if record then assert(record.Kind == kind, "linked identity/unit kind mismatch") end
    assert(not found[id], "duplicate absorption asset ID")
    found[id] = true
    local info = GameInfo.Units[unit:GetType()]
    local source = record and R.Copy(record)
    if not source and kind == "PERSON" then
      local warlord = unit:GetProperty("JAC_POC_GROWTHUI_WARLORD")
      local gp = unit:GetGreatPerson()
      source = { ID = id, Kind = kind, Owner = plan.From, UnitType = info.UnitType,
        Warlord = R.Copy(warlord), NativeIndividual = gp and gp:GetIndividual() or -1,
        Abilities = A.UnitAbilityLabels(unit) }
      if warlord then
        source.Personality, source.Trait = warlord.Personality, warlord.Trait
        source.Equipment = R.Copy(warlord.Equipment)
      end
    end
    entries[#entries + 1] = {
      ID = id, Kind = kind, Name = record and record.Name or unit:GetName()
        or info.Name or info.UnitType,
      Portrait = Portrait(kind, unit, record), AllowedActions = R.Copy(assert(CHOICES[kind])),
      Source = source, UnitID = unit:GetID(), UnitType = info.UnitType,
      Abilities = A.UnitAbilityLabels(unit, record),
    }
  end
  for _, id in ipairs(R.Keys(roster)) do
    if not found[id] then
      local record = roster[id]
      assert(CHOICES[record.Kind], "unsupported absorption asset kind")
      assert(not record.Unit or record.Unit.Owner ~= plan.From or record.Unit.ID < 0,
        "identity points to missing source unit")
      entries[#entries + 1] = { ID = id, Kind = record.Kind,
        Name = record.Name or record.LeaderType or id,
        Portrait = Portrait(record.Kind, nil, record),
        AllowedActions = R.Copy(CHOICES[record.Kind]), Source = R.Copy(record),
        Abilities = R.Copy(record.Abilities or record.Ability or record.Traits or record.Trait) }
      found[id] = true
    end
  end
  for _, work in ipairs(A.OwnedGreatWorkSlots(plan.From)) do
    local id = "native-work:" .. tostring(work.Index)
    if found[id] then
      local record = assert(roster[id], "native Great Work identity collides with a unit")
      assert(record.Kind == "ITEM" and record.Owner == plan.From
        and record.NativeWork and record.NativeWork.Index == work.Index,
        "tracked native Great Work roster identity mismatch")
      for _, key in ipairs({ "GreatWorkType", "ObjectType", "CreatorName",
          "TurnCreated", "CreatedByPlayer" }) do
        assert(record.NativeWork[key] == nil or record.NativeWork[key] == work[key],
          "tracked native Great Work metadata changed: " .. key)
      end
      assert(record.NativeWork.Effects == nil or R.Equal(record.NativeWork.Effects, work.Effects),
        "tracked native Great Work effects changed")
      local entry
      for _, candidate in ipairs(entries) do if candidate.ID == id then entry = candidate; break end end
      assert(entry and entry.Kind == "ITEM", "tracked native Great Work inventory entry missing")
      entry.NativeWork = R.Copy(work)
      entry.Source.NativeWork = R.Copy(work)
      entry.Source.City = { Owner = plan.From, X = work.X, Y = work.Y }
      entry.Source.NativeCustody = { Owner = plan.From, City = work.CityID,
        X = work.X, Y = work.Y, Building = work.Building, Slot = work.Slot }
    else
      entries[#entries + 1] = {
        ID = id, Kind = "ITEM", Name = work.Name or work.GreatWorkType or id,
        Portrait = work.GreatWorkType and "ICON_" .. work.GreatWorkType,
        AllowedActions = R.Copy(CHOICES.ITEM),
        Source = { ID = id, Kind = "ITEM", Owner = plan.From,
          Type = work.ObjectType or "NATIVE_GREAT_WORK", NativeWork = R.Copy(work) },
        NativeWork = R.Copy(work),
        Abilities = work.Effects and #work.Effects > 0 and R.Copy(work.Effects)
          or work.ObjectType and { work.ObjectType } or nil,
      }
    end
    found[id] = true
  end
  -- Native claim-only Heroes can outlive their Unit. Keep the same generated
  -- identity as A.Prepare; a linked roster record has already been listed.
  local manager = A.GetHeroesManager()
  for def in GameInfo.HeroClasses() do
    assert(string.match(def.HeroClassType, "^HEROCLASS_JAC_"),
      "official Hero class is still available in Banseiki")
    if manager:GetHeroClaimPlayer(def.Index) == plan.From then
      local live = false
      for _, entry in ipairs(entries) do
        if entry.Kind == "HERO" and entry.UnitID then
          local unit = UnitManager.GetUnit(plan.From, entry.UnitID)
          if unit and unit:GetHeroClassType() == def.Index then live = true end
        elseif entry.Kind == "HERO" and entry.Source and entry.Source.NativeHero
          and entry.Source.NativeHero.Class == def.Index then live = true end
      end
      if not live then
        local id = "native-hero:" .. plan.From .. ":" .. def.HeroClassType
        assert(not found[id], "native Hero identity collision")
        entries[#entries + 1] = { ID = id, Kind = "HERO", Name = def.Name or def.HeroClassType,
          Portrait = "ICON_" .. def.UnitType, AllowedActions = R.Copy(CHOICES.HERO),
          Abilities = GameInfo.Units[def.UnitType] and GameInfo.Units[def.UnitType].Description
            and { GameInfo.Units[def.UnitType].Description } or nil,
          Source = { ID = id, Kind = "HERO", Owner = plan.From,
            HeroClassType = def.HeroClassType }, DormantClass = def.Index }
        found[id] = true
      end
    end
  end
  local leaderID = "leader:" .. plan.From
  assert(not found[leaderID], "leader identity collision")
  entries[#entries + 1] = {
    ID = leaderID, Kind = "GREAT_PERSON",
    Name = (GameInfo.Leaders and GameInfo.Leaders[PlayerConfigurations[plan.From]:GetLeaderTypeName()]
      or {}).Name or PlayerConfigurations[plan.From]:GetLeaderTypeName(),
    Portrait = "ICON_" .. PlayerConfigurations[plan.From]:GetLeaderTypeName(),
    AllowedActions = R.Copy(CHOICES.GREAT_PERSON),
    Source = { ID = leaderID, Kind = "GREAT_PERSON", Owner = plan.From,
      Loyalty = 50 }, Leader = true,
  }
  table.sort(entries, function(a, b) return tostring(a.ID) < tostring(b.ID) end)
  return entries, roster
end

local function FindEntry(flow, id)
  for _, entry in ipairs(flow.Entries) do
    if tostring(entry.ID) == tostring(id) then return entry end
  end
  return nil
end
local function IsAllowed(entry, action)
  for _, candidate in ipairs(entry.AllowedActions) do
    if candidate == action then return true end
  end
  return false
end
local function Inventory(plan, phase)
  if Multiplayer() then
    assert(JAC_NativeInventory, "native inventory binding unavailable")
    if JAC_NativeInventory.HasContext() then return InventoryLocal(plan) end
    return JAC_NativeInventory.With(plan, phase or "FLOW_BEGIN", function() return InventoryLocal(plan) end)
  end
  return InventoryLocal(plan)
end

local function RequireActor(actor, flow)
  assert(type(actor) == "number" and actor == flow.To, "only absorbing player can choose")
  assert(Players[actor] and Players[actor]:IsAlive(), "absorber is no longer alive")
end
function F.Get(id)
  local flow = State().Flows[id]
  return flow and flow.Phase1Complete and R.Copy(flow) or nil
end
function F.GetPending(playerID)
  local state = State()
  for _, id in ipairs(R.Keys(state.Flows)) do
    local flow = state.Flows[id]
    if flow.To == playerID and flow.Phase1Complete and flow.Phase ~= "DONE" then
      return R.Copy(flow)
    end
  end
  return nil
end
function F.DebugSetRoll(flowID, assetID, stage, value)
  assert(DebugAllowed(), "absorption debug rolls require explicit single-player test mode")
  assert(type(flowID) == "string"
    and (type(assetID) == "string" or type(assetID) == "number"),
    "debug roll identity required")
  assert(stage == "P1_ESCAPE" or stage == "P1_BRANCH" or stage == "P3_SUCCESS"
    or stage == "P3_RETRY_ESCAPE" or stage == "P3_RETRY_BRANCH", "unknown debug stage")
  assert(type(value) == "number" and value == math.floor(value)
    and value >= 0 and value < RATE.RollRange, "debug roll outside 0..9999")
  local debug = DebugState()
  debug.Rolls[flowID .. ":" .. tostring(assetID) .. ":" .. stage] = value
  Game:SetProperty(DEBUG_KEY, R.Copy(debug))
end
function F.DebugSetSpawnDelay(turns)
  assert(DebugAllowed(), "absorption debug delay requires explicit single-player test mode")
  assert(type(turns) == "number" and turns == math.floor(turns)
    and turns >= 0 and turns <= RATE.WorldDelayTurns,
    "debug spawn delay outside 0..2")
  local debug = DebugState()
  debug.Delay = turns
  Game:SetProperty(DEBUG_KEY, R.Copy(debug))
end
local function DefaultAI(entry)
  if entry.Kind == "PERSON" or entry.Kind == "GOVERNOR" then return "RECRUIT" end
  if entry.Kind == "HERO" then return "RECONTRACT" end
  if entry.Kind == "GREAT_PERSON" then return "RECRUIT" end
  if entry.Kind == "ITEM" then return "TRANSFER" end
  if entry.Kind == "UNIT" then return "INCORPORATE" end
  if entry.Kind == "UNLOCK" then return "INHERIT" end
  return "PRESERVE"
end
local function RequireNativeHeroes()
  assert(GameConfiguration and GameConfiguration.GetValue
    and (GameConfiguration.GetValue("GAMEMODE_HEROES") == true
      or GameConfiguration.GetValue("GAMEMODE_HEROES") == 1),
    "official Heroes & Legends mode required")
  local manager = A.GetHeroesManager()
  assert(manager and manager.GetHeroClaimPlayer and GameInfo.HeroClasses,
    "native Heroes manager unavailable")
  local required = { HEROCLASS_JAC_WARLORD_A = false, HEROCLASS_JAC_WARLORD_B = false }
  for def in GameInfo.HeroClasses() do
    assert(string.match(def.HeroClassType, "^HEROCLASS_JAC_"),
      "official Hero class is still available in Banseiki")
    if required[def.HeroClassType] ~= nil then required[def.HeroClassType] = true end
  end
  assert(required.HEROCLASS_JAC_WARLORD_A and required.HEROCLASS_JAC_WARLORD_B,
    "required Sengoku Hero classes are missing")
end
local function Begin(plan)
  assert(type(plan) == "table" and type(plan.ID) == "string" and plan.ID ~= "",
    "absorption flow ID required")
  assert(type(plan.From) == "number" and type(plan.To) == "number" and plan.From ~= plan.To,
    "distinct absorption factions required")
  assert(R.ENTRY_MODES[plan.Mode], "unknown absorption method")
  assert(plan.EntryConditionsMet == true and plan.RelationsResolved == true,
    "absorption entry and relations must be validated")
  RequireNativeHeroes()
  local state = State()
  local existing = state.Flows[plan.ID]
  if existing and existing.Phase1Complete then
    assert(R.Equal(existing.Request, plan), "absorption flow ID reused with different plan")
    if existing.Phase == "CHOICE" and Players[plan.To]
      and not Players[plan.To]:IsHuman() then return F.Resolve(plan.To, plan.ID) end
    return R.Copy(existing)
  end
  assert(Players[plan.From] and Players[plan.From]:IsAlive()
    and Players[plan.To] and Players[plan.To]:IsAlive(), "absorption faction unavailable")
  assert(not Players[plan.From]:IsHuman(), "existing PoC absorbs AI sources only")
  if existing then
    assert(R.Equal(existing.Request, plan), "absorption flow ID reused with different plan")
    if not existing.Phase1Complete then
      for _, entry in ipairs(existing.Entries) do
        if ESCAPE[entry.Kind] then
          entry.EscapeOutcome = ResolveEscape(state, existing, entry, "P1")
          if entry.EscapeOutcome then entry.FinalOutcome = entry.EscapeOutcome end
          Save(state)
        end
      end
      existing.Phase1Complete = true
      if not Players[plan.To]:IsHuman() then
        for _, entry in ipairs(existing.Entries) do
          if not entry.EscapeOutcome then entry.SelectedAction = DefaultAI(entry) end
        end
      end
      Save(state)
    end
    if existing.Phase == "CHOICE" and not Players[plan.To]:IsHuman() then return F.Resolve(plan.To, plan.ID) end
    return R.Copy(existing)
  end
  for _, other in pairs(state.Flows) do
    assert(other.Phase == "DONE" or
      (other.From ~= plan.From and other.To ~= plan.From
        and other.From ~= plan.To and other.To ~= plan.To),
      "another absorption flow uses a faction")
  end
  local entries, roster = Inventory(plan)
  local fingerprint = {}
  for _, entry in ipairs(entries) do
    fingerprint[#fingerprint + 1] = { ID = entry.ID, Kind = entry.Kind,
      UnitID = entry.UnitID, UnitType = entry.UnitType,
      NativeWork = R.Copy(entry.NativeWork) }
  end
  local flow = { ID = plan.ID, From = plan.From, To = plan.To, Mode = plan.Mode,
    Turn = Game.GetCurrentGameTurn(), Phase = "CHOICE", Request = R.Copy(plan),
    Entries = entries, RosterAtBegin = roster, Inventory = fingerprint,
    Phase1Complete = false }
  state.Flows[plan.ID] = flow
  Save(state)
  for _, entry in ipairs(flow.Entries) do
    if ESCAPE[entry.Kind] then
      entry.EscapeOutcome = ResolveEscape(state, flow, entry, "P1")
      if entry.EscapeOutcome then entry.FinalOutcome = entry.EscapeOutcome end
      Save(state)
    end
  end
  flow.Phase1Complete = true
  if not Players[plan.To]:IsHuman() then
    for _, entry in ipairs(flow.Entries) do
      if not entry.EscapeOutcome then entry.SelectedAction = DefaultAI(entry) end
    end
  end
  Save(state)
  if not Players[plan.To]:IsHuman() then return F.Resolve(plan.To, plan.ID) end
  return R.Copy(flow)
end
function F.Begin(plan)
  if not Multiplayer() then return Begin(plan) end
  local ok,result=pcall(function()
    return JAC_NativeInventory.With(plan,"FLOW_BEGIN",function() return Begin(plan) end)
  end)
  if not ok and tostring(result)=="JAC_ABSORPTION_WAITING_UI" then
    return {ID=plan.ID,From=plan.From,To=plan.To,Phase="WAITING_SNAPSHOT"}
  end
  if not ok then error(result,0) end
  return result
end

function F.Select(actor, id, assetID, action, returnOwner)
  local state = State()
  local flow = assert(state.Flows[id], "unknown absorption flow")
  RequireActor(actor, flow)
  local entry = assert(FindEntry(flow, assetID), "unknown absorption asset")
  if flow.Phase ~= "CHOICE" then
    assert(entry.SelectedAction == action and entry.ReturnOwner == returnOwner,
      "selection is already frozen")
    return R.Copy(flow)
  end
  assert(flow.Phase1Complete, "initial rolls are incomplete")
  assert(not entry.EscapeOutcome, "escaped asset cannot be selected")
  if flow.FinalRollsComplete then
    if not entry.ReturnOwnerPending then
      assert(entry.SelectedAction == action and entry.ReturnOwner == returnOwner,
        "final outcomes are frozen")
      return R.Copy(flow)
    end
    assert(action == "RETURN" and entry.FinalOutcome == "RETURN",
      "only the lost return destination can be corrected")
  end
  assert(IsAllowed(entry, action), "action is not allowed for this asset")
  if action == "RETURN" then
    assert(ValidReturnOwner(flow, returnOwner),
      "return requires a living third party with a city")
  else assert(returnOwner == nil, "return owner only applies to RETURN") end
  -- All checks precede the mutation; a rejected change preserves the old one.
  entry.SelectedAction, entry.ReturnOwner = action, returnOwner
  entry.ReturnOwnerPending = nil
  Save(state)
  return R.Copy(flow)
end
local function InventoryStillMatches(flow)
  if not Players[flow.From]:IsAlive() or not Players[flow.To]:IsAlive() then
    return false, "absorption faction died before resolution"
  end
  local entries, roster
  if Multiplayer() then
    entries,roster=JAC_NativeInventory.With(flow.Request,"FLOW_CHECK",function() return InventoryLocal(flow.Request) end)
  else entries,roster=Inventory(flow.Request) end
  local actual = {}
  for _, entry in ipairs(entries) do
    actual[#actual + 1] = { ID = entry.ID, Kind = entry.Kind,
      UnitID = entry.UnitID, UnitType = entry.UnitType,
      NativeWork = R.Copy(entry.NativeWork) }
  end
  if not R.Equal(actual, flow.Inventory) or not R.Equal(roster, flow.RosterAtBegin) then
    return false, "absorption inventory changed after first rolls"
  end
  return true
end
local function BuildPlan(flow)
  local plan = R.Copy(flow.Request)
  assert(plan.EntryConditionsMet == true and plan.RelationsResolved == true,
    "validated absorption request changed")
  plan.FlowPolicy = true
  plan.Units, plan.Assets, plan.NativeWorks = {}, {}, {}
  for _, entry in ipairs(flow.Entries) do
    local outcome = assert(entry.FinalOutcome, "unresolved absorption result")
    local decision = { Outcome = outcome }
    if outcome == "RETURN" then decision.ReturnOwner = assert(entry.ReturnOwner) end
    if entry.Leader then plan.Leader = decision
    elseif entry.NativeWork then plan.NativeWorks[entry.NativeWork.Index] = R.Copy(decision)
    else
      if entry.UnitID then
        plan.Units[R.UnitKey(flow.From, entry.UnitID)] = R.Copy(decision)
      end
      if entry.Source and entry.Source.ID and flow.RosterAtBegin[entry.ID]
        or entry.DormantClass then
        plan.Assets[entry.ID] = R.Copy(decision)
      end
    end
  end
  return plan
end

local function QueueEscapes(state, flow, tx)
  local debug = DebugState()
  local delay = debug and debug.Delay or RATE.WorldDelayTurns
  for _, entry in ipairs(flow.Entries) do
    if entry.FinalOutcome and ESCAPE[entry.Kind] then
      local escaping = false
      for _, outcome in ipairs(ESCAPE[entry.Kind]) do
        if entry.FinalOutcome == outcome then escaping = true; break end
      end
      if escaping then
        local key = flow.ID .. ":" .. tostring(entry.ID)
        if not state.World[key] then
          local snapshot
          for _, unit in ipairs(tx.Units or {}) do
            if entry.UnitID and unit.ID == entry.UnitID or entry.Leader and unit.ID == -1 then
              snapshot = R.Copy(unit)
              break
            end
          end
          local origin = entry.NativeWork and { X = entry.NativeWork.X, Y = entry.NativeWork.Y }
            or snapshot and { X = snapshot.X, Y = snapshot.Y }
            or tx.Cities and tx.Cities[1] and { X = tx.Cities[1].X, Y = tx.Cities[1].Y }
          assert(origin, "escaped asset has no world origin")
          state.World[key] = { ID = key, FlowID = flow.ID, AssetID = entry.ID,
            Kind = entry.Kind, Outcome = entry.FinalOutcome, Origin = origin,
            Snapshot = snapshot, NativeWork = R.Copy(entry.NativeWork),
            DueTurn = Game.GetCurrentGameTurn() + delay,
            Status = "WAIT_SITE", SpawnAttempts = 0 }
          Save(state)
        end
        entry.WorldID = key
      end
    end
  end
end
local function FinishApply(state, flow, tx)
  assert(tx and tx.Status == "COMPLETE", "absorption transfer is incomplete")
  for _, entry in ipairs(flow.Entries) do
    local actual
    if tx.Decisions and tx.Decisions[entry.ID] then
      actual = tx.Decisions[entry.ID]
    else
      for _, snap in ipairs(tx.Units or {}) do
        if entry.UnitID and snap.ID == entry.UnitID then actual = snap; break end
      end
    end
    if actual and actual.Outcome ~= entry.FinalOutcome then
      entry.RequestedOutcome = entry.FinalOutcome
      entry.FinalOutcome = actual.Outcome
      entry.TransferCause = actual.SuicideCause
      entry.Success = false
    end
  end
  QueueEscapes(state, flow, tx)
  flow.Phase, flow.CompletedTurn = "RESULT", Game.GetCurrentGameTurn()
  flow.Error, flow.WaitingFor = nil, nil
  Save(state)
  return R.Copy(flow)
end
function F.Resume(id)
  local state = State()
  local flow = assert(state.Flows[id], "unknown absorption flow")
  if flow.Phase == "RESULT" or flow.Phase == "DONE" then return R.Copy(flow) end
  assert(flow.Phase == "APPLYING" or
    (flow.Phase == "BLOCKED" and flow.AppliedPlan), "absorption is not applying")
  if flow.Phase == "BLOCKED" then
    flow.FirstError = flow.FirstError or flow.Error
    flow.Phase, flow.PreviousError = "APPLYING", flow.Error
    Save(state)
  end
  local tx = A.GetState().Transactions[id]
  if tx and tx.Status == "COMPLETE" then return FinishApply(state, flow, tx) end
  local invalidReturn = false
  for _, entry in ipairs(flow.Entries) do
    if NeedsReturnOwner(flow, entry) and not ValidReturnOwner(flow, entry.ReturnOwner) then
      invalidReturn = true
    end
  end
  if invalidReturn then
    if not tx or A.CanCancelUnprepared(id, flow.AppliedPlan) then
      A.CancelUnprepared(id, flow.AppliedPlan)
      for _, entry in ipairs(flow.Entries) do
        if NeedsReturnOwner(flow, entry) and not ValidReturnOwner(flow, entry.ReturnOwner) then
          entry.SelectedAction, entry.ReturnOwner = nil, nil
          entry.ReturnOwnerPending = true
        end
      end
      -- Keep P3 outcomes and random journals. Only recipient selection reopens.
      flow.Phase, flow.FinalRollsComplete = "CHOICE", true
      flow.AppliedPlan, flow.WaitingFor, flow.Error = nil, nil, nil
      Save(state)
      return R.Copy(flow)
    end
    flow.Phase, flow.Error = "BLOCKED", "return owner disappeared after absorption mutation began"
    flow.WaitingFor = nil
    Save(state)
    return R.Copy(flow)
  end
  local ok, result
  if tx then ok, result = pcall(A.Resume, id)
  else ok, result = pcall(A.Apply, R.Copy(flow.AppliedPlan)) end
  if not ok then
    flow.Phase, flow.Error = "BLOCKED", tostring(result)
    Save(state)
    return R.Copy(flow)
  end
  if result.Status == "COMPLETE" then return FinishApply(state, flow, result) end
  assert(result.Status == "WAITING_UI" or result.Status == "PREPARED"
    or result.Status == "STAGED", "unexpected PoC transfer state")
  flow.WaitingFor = result.Status
  flow.Error = nil
  Save(state)
  return R.Copy(flow)
end
function F.Resolve(actor, id)
  local state = State()
  local flow = assert(state.Flows[id], "unknown absorption flow")
  RequireActor(actor, flow)
  RequireNativeHeroes()
  if flow.Phase == "APPLYING" or flow.Phase == "BLOCKED" and flow.AppliedPlan then
    return F.Resume(id)
  end
  if flow.Phase == "RESULT" or flow.Phase == "DONE" then return R.Copy(flow) end
  assert(flow.Phase == "CHOICE" and flow.Phase1Complete, "absorption is not choosing")
  for _, entry in ipairs(flow.Entries) do
    assert(entry.EscapeOutcome or entry.SelectedAction,
      "every remaining asset requires a decision")
  end
  local valid, why = InventoryStillMatches(flow)
  if not valid then
    flow.Phase, flow.Error = "BLOCKED", why
    Save(state)
    return R.Copy(flow)
  end
  local invalidReturn = false
  for _, entry in ipairs(flow.Entries) do
    if not entry.EscapeOutcome and NeedsReturnOwner(flow, entry)
      and not ValidReturnOwner(flow, entry.ReturnOwner) then
      entry.SelectedAction, entry.ReturnOwner = nil, nil
      if flow.FinalRollsComplete then entry.ReturnOwnerPending = true end
      invalidReturn = true
    end
  end
  if invalidReturn then
    Save(state) -- no final rolls or asset mutations; the human can choose again
    return R.Copy(flow)
  end
  for _, entry in ipairs(flow.Entries) do
    if not flow.FinalRollsComplete and not entry.EscapeOutcome then
      entry.ActionChance = ActionChance(flow, entry)
      local roll = Roll(state, flow, entry, "P3_SUCCESS")
      entry.Success = roll < entry.ActionChance
      if entry.Success then
        entry.FinalOutcome = entry.SelectedAction
      else
        entry.RetryEscapeOutcome = ResolveEscape(state, flow, entry, "P3_RETRY")
        entry.FinalOutcome = entry.RetryEscapeOutcome or FALLBACK[entry.Kind]
      end
      Save(state)
    end
  end
  for _, entry in ipairs(flow.Entries) do
    if entry.UnitID and ESCAPE[entry.Kind] then
      local escaping = false
      for _, outcome in ipairs(ESCAPE[entry.Kind]) do
        if entry.FinalOutcome == outcome then escaping = true; break end
      end
      if escaping then
        local ok, reason = pcall(A.AssertRecoverableEscape, flow.From, entry.UnitID)
        if not ok then
          flow.Phase, flow.Error = "BLOCKED", tostring(reason)
          Save(state)
          return R.Copy(flow)
        end
      end
    end
  end
  flow.FinalRollsComplete = true
  flow.AppliedPlan = BuildPlan(flow)
  flow.Phase = "APPLYING"
  Save(state) -- all rolls and outcomes precede the first PoC mutation
  return F.Resume(id)
end
function F.Acknowledge(actor, id)
  local state = State()
  local flow = assert(state.Flows[id], "unknown absorption flow")
  RequireActor(actor, flow)
  assert(flow.Phase == "RESULT" or flow.Phase == "DONE",
    "results are not ready to acknowledge")
  if flow.Phase == "RESULT" then
    flow.Phase, flow.AcknowledgedTurn = "DONE", Game.GetCurrentGameTurn()
    Save(state)
  end
  return R.Copy(flow)
end

local function Improvement(row)
  local info = GameInfo.Improvements[row]
  return info and info.Index
end
local function Reserved(state, index, except)
  for key, world in pairs(state.World) do
    if key ~= except and world.Site == index
      and world.Status ~= "COMPLETE" and world.Status ~= "SUICIDE" then return true end
  end
  return false
end
local function ChooseSite(state, world)
  local camp = Improvement("IMPROVEMENT_BARBARIAN_CAMP")
  local hut = Improvement("IMPROVEMENT_GOODY_HUT")
  assert(camp and hut, "native camp and goody hut improvements required")
  local candidates = {}
  local occupied = {}
  local flow = state.Flows[world.FlowID]
  local absorber = flow and flow.To
  for _, playerID in ipairs(PlayerManager.GetAliveIDs()) do
    for _, unit in Players[playerID]:GetUnits():Members() do
      local plot = Map.GetPlot(unit:GetX(), unit:GetY())
      if plot then occupied[plot:GetIndex()] = true end
    end
  end
  for index = 0, Map.GetPlotCount() - 1 do
    local plot = Map.GetPlotByIndex(index)
    local improvement = plot:GetImprovementType()
    local existing = improvement == camp or improvement == hut
    local owner = plot:GetOwner()
    -- Japan's preallocated provinces can leave no unowned land. Use an empty
    -- absorber plot only after unowned sites, without erasing a district.
    local empty = improvement == -1 and (owner == -1 or owner == absorber) and not occupied[index]
      and not plot:IsWater() and not plot:IsMountain()
      and (not plot.IsCity or not plot:IsCity())
      and (not plot.GetDistrictType or plot:GetDistrictType() == -1)
    if (existing or empty) and not Reserved(state, index, world.ID)
      and not (world.RejectedSites or {})[index] then
      local distance = math.abs(plot:GetX() - world.Origin.X)
        + math.abs(plot:GetY() - world.Origin.Y)
      local preference = existing and (improvement == camp and world.Kind ~= "ITEM" and 0
        or improvement == hut and 1 or 2) or (owner == -1 and 3 or 4)
      candidates[#candidates + 1] = { Index = index, Distance = distance,
        Preference = preference, Existing = existing,
        Type = existing and improvement or hut }
    end
  end
  table.sort(candidates, function(a, b)
    if a.Preference ~= b.Preference then return a.Preference < b.Preference end
    if a.Distance ~= b.Distance then return a.Distance < b.Distance end
    return a.Index < b.Index
  end)
  return candidates[1]
end
local function PlaceWorld(state, world)
  if world.Status == "WAIT_SITE" then
    if Game.GetCurrentGameTurn() < world.DueTurn then return end
    local choice = ChooseSite(state, world)
    if not choice then
      world.SpawnAttempts = world.SpawnAttempts + 1
      Save(state)
      return
    end
    world.Site, world.Improvement = choice.Index, choice.Type
    world.CreatedSite = not choice.Existing
    world.Status = "PLACE_INTENT"
    Save(state)
  end
  if world.Status == "PLACE_INTENT" then
    local plot = assert(Map.GetPlotByIndex(world.Site), "reward site plot missing")
    if world.CreatedSite and plot:GetImprovementType() == -1 then
      -- Gameplay setter is used in WarMachineScenario.lua:276. Do not erase
      -- an occupied improvement or replace another player's installation.
      local ok = pcall(ImprovementBuilder.SetImprovementType, plot, world.Improvement, -1)
      if not ok or plot:GetImprovementType() ~= world.Improvement then
        world.RejectedSites = world.RejectedSites or {}
        world.RejectedSites[world.Site] = true
        world.Site, world.Improvement, world.Status = nil, nil, "WAIT_SITE"
        world.DueTurn = Game.GetCurrentGameTurn() + 1
        world.SpawnAttempts = world.SpawnAttempts + 1
        Save(state)
        return
      end
    end
    assert(plot:GetImprovementType() == world.Improvement,
      "reward improvement placement failed; retry without losing identity")
    world.Status, world.PlacedTurn = "WAIT_CLEAR", Game.GetCurrentGameTurn()
    Save(state)
  end
end
local function MajorOnPlot(plot)
  local found
  for _, playerID in ipairs(PlayerManager.GetAliveIDs()) do
    local player = Players[playerID]
    if player and player:IsMajor() and player:GetCities():GetCount() > 0 then
      for _, unit in player:GetUnits():Members() do
        if unit:GetX() == plot:GetX() and unit:GetY() == plot:GetY() then
          if not found or playerID < found then found = playerID end
        end
      end
    end
  end
  return found
end
local function ClaimWorld(state, world, winner)
  if not HasSurvivingCity(winner) or not Players[winner]:IsMajor() then
    if world.Recipient ~= nil and world.Recipient == winner then
      world.Status, world.Error = "BLOCKED", "reward recipient needs a surviving city"
      Save(state) -- retain the recipient, publication token and native recovery intent
    end
    return
  end
  if world.Status == "WAIT_CLEAR" then
    world.Status, world.Recipient = "CLAIMED", winner
    world.ClearedTurn = Game.GetCurrentGameTurn()
    Save(state)
  end
  assert(world.Status == "CLAIMED" and world.Recipient == winner,
    "reward was already claimed by a different player")
  local ok, result = pcall(A.RecoverArchived, world.AssetID, winner,
    "recovery:" .. world.ID, world.Snapshot)
  if not ok then
    world.Status, world.Error = "BLOCKED", tostring(result)
    Save(state)
    return
  end
  if result.Status == "WAITING_UI" then
    world.WaitingFor = result.ObservationToken
    Save(state)
    return
  end
  if result.Status == "BLOCKED" then
    world.Status, world.Error = "BLOCKED", result.Cause
    world.WaitingFor = nil
    Save(state)
    return
  end
  assert(result.Status == "SUICIDE" or result.Status == "COMPLETE",
    "recovery has an unresolved native state")
  world.Status = result.Status
  world.WaitingFor = nil
  world.Cause = result.Cause
  Save(state)
end
function F.ResumeWorld(id)
  local state = State()
  local world = assert(state.World[id], "unknown escaped world asset")
  if world.Status == "BLOCKED" then world.Status, world.Error = "CLAIMED", nil; Save(state) end
  if world.Status == "CLAIMED" then ClaimWorld(state, world, world.Recipient) end
  return R.Copy(world)
end
function F.ResumeWorldFor(actor, id, observationToken)
  local state = State()
  local world = assert(state.World[id], "unknown escaped world asset")
  local recovery = A.GetState().Recoveries["recovery:" .. world.ID]
  local carrier = recovery and recovery.NativeCarrier
  if carrier and carrier.MP then
    assert(world.Status == "CLAIMED" and type(observationToken) == "string"
      and world.WaitingFor == observationToken and recovery.ObservationToken == observationToken,
      "MP world recovery token is not pending")
    assert(actor == carrier.MP.Actor and Players[actor] and Players[actor]:IsAlive()
      and Players[actor]:IsHuman() and Players[actor]:IsTurnActive(),
      "only the selected human executor can resume MP world recovery")
    assert(carrier.Owner == world.Recipient and carrier.MP.Recovery == "recovery:" .. world.ID,
      "MP world recipient differs from recovery reservation")
    ClaimWorld(state, world, world.Recipient)
    return R.Copy(world)
  end
  assert(type(actor) == "number" and actor == world.Recipient
    and Players[actor] and Players[actor]:IsAlive() and Players[actor]:IsHuman(),
    "only the reward recipient can resume world recovery")
  assert(world.Status == "CLAIMED" and world.WaitingFor == observationToken
    and type(observationToken) == "string", "world recovery token is not pending")
  local reads = ExposedMembers.JAC_HeroReadAPI
  local observed = reads and reads.GetObservation and reads.GetObservation(observationToken)
  local archived = A.GetState().Archive[world.AssetID]
  if archived and archived.Kind == "ITEM" and archived.NativeWork then
    local recovery = A.GetState().Recoveries["recovery:" .. world.ID]
    local carrier = recovery and recovery.NativeCarrier
    if carrier then
      local current = carrier.Phase == "BORROW" and carrier.BorrowToken
        or carrier.Phase == "MOVE" and carrier.MoveToken
        or (carrier.Phase == "RETURN" or carrier.Phase == "RESTORE")
          and carrier.ReturnToken
      assert(recovery.ObservationToken == observationToken and current == observationToken,
        "native work recovery publication token is stale")
    end
    assert(observed and observed.Published == true, "native work publication is not ready")
  else
    assert(observed and type(observed.Claim) == "number"
      and type(observed.Origin) == "table", "Hero publication is not ready")
  end
  ClaimWorld(state, world, actor)
  return R.Copy(world)
end
function F.GetWorld(id)
  local world = State().World[id]
  return world and R.Copy(world) or nil
end
function F.GetPendingWorlds(owner)
  local result = {}
  local state = State()
  for _, id in ipairs(R.Keys(state.World)) do
    local world = state.World[id]
    if world.Status == "CLAIMED" and world.Recipient == owner and world.WaitingFor then
      result[#result + 1] = R.Copy(world)
    end
  end
  return result
end
function F.Tick()
  -- Only saved gameplay intents are resumed; this never selects for a human.
  for _, id in ipairs(R.Keys(State().Flows)) do
    local flow = State().Flows[id]
    if flow.Phase == "APPLYING" then F.Resume(id) end
    flow = State().Flows[id]
    if flow.Phase == "RESULT" and Players[flow.To]
      and not Players[flow.To]:IsHuman() then F.Acknowledge(flow.To, id) end
  end
  local state = State()
  for _, id in ipairs(R.Keys(state.World)) do
    local world = state.World[id]
    if world.Status == "WAIT_SITE" or world.Status == "PLACE_INTENT" then
      local ok, err = pcall(PlaceWorld, state, world)
      if not ok then
        world.Status, world.Error = "BLOCKED", tostring(err)
        Save(state)
      end
    elseif world.Status == "WAIT_CLEAR" then
      local plot = Map.GetPlotByIndex(world.Site)
      if plot and plot:GetImprovementType() ~= world.Improvement then
        local winner = MajorOnPlot(plot)
        if winner then ClaimWorld(state, world, winner)
        else
          -- An improvement removed without an identifiable clearing unit is
          -- not a reward. Place another deterministic site on a later turn.
          world.Status, world.Site, world.Improvement = "WAIT_SITE", nil, nil
          world.DueTurn = Game.GetCurrentGameTurn() + 1
          Save(state)
        end
      end
    elseif world.Status == "BLOCKED" and world.Recipient ~= nil then
      local recovery = A.GetState().Recoveries["recovery:" .. world.ID]
      local restored = recovery and recovery.NativeCarrier
        and recovery.NativeCarrier.Phase == "RESTORED"
      if not restored and HasSurvivingCity(world.Recipient)
        and Players[world.Recipient]:IsMajor() then
        world.Status, world.Error = "CLAIMED", nil
        Save(state)
        ClaimWorld(state, world, world.Recipient)
      end
    elseif world.Status == "CLAIMED" then
      ClaimWorld(state, world, world.Recipient)
    end
  end
end
local function ImprovementRemoved(x, y)
  local plot = Map.GetPlot(x, y)
  if not plot then return end
  local state = State()
  local winner = MajorOnPlot(plot)
  if not winner then return end
  for _, id in ipairs(R.Keys(state.World)) do
    local world = state.World[id]
    if world.Site == plot:GetIndex() and world.Status == "WAIT_CLEAR"
      and plot:GetImprovementType() ~= world.Improvement then
      ClaimWorld(state, world, winner)
    end
  end
end
local function GoodyReward(playerID, unitID)
  local unit = UnitManager.GetUnit(playerID, unitID)
  if not unit or not Players[playerID] or not Players[playerID]:IsMajor() then return end
  local plot = Map.GetPlot(unit:GetX(), unit:GetY())
  if not plot then return end
  local state = State()
  for _, id in ipairs(R.Keys(state.World)) do
    local world = state.World[id]
    if world.Site == plot:GetIndex() and world.Status == "WAIT_CLEAR"
      and world.Improvement == Improvement("IMPROVEMENT_GOODY_HUT") then
      ClaimWorld(state, world, playerID)
    end
  end
end
local function Operation(actor, payload)
  local ok, result = pcall(function()
    assert(type(payload) == "table", "operation payload required")
    if payload.Command == "SELECT" then
      return F.Select(actor, payload.FlowID, payload.AssetID,
        payload.Action, payload.ReturnOwner)
    elseif payload.Command == "RESOLVE" then
      return F.Resolve(actor, payload.FlowID)
    elseif payload.Command == "ACK" then
      return F.Acknowledge(actor, payload.FlowID)
    elseif payload.Command == "RESUME_WORLD" then
      return F.ResumeWorldFor(actor, payload.WorldID, payload.ObservationToken)
    elseif payload.Command == "RESUME_FRICTION" then
      assert(Players[actor] and Players[actor]:IsAlive() and Players[actor]:IsHuman(),
        "only an active human player can request published city reconciliation")
      return A.ApplyCityFriction(actor)
    elseif payload.Command == "RECONCILE_PUBLICATIONS" then
      assert(Players[actor] and Players[actor]:IsAlive() and Players[actor]:IsHuman(),
        "only an active human can announce UI publication")
      F.Tick()
      return A.ApplyCityFriction()
    end
    error("unknown absorption command")
  end)
  if Players[actor] then
    local command = type(payload) == "table" and type(payload.Command) == "string"
      and payload.Command ~= "" and payload.Command or "INVALID"
    local flowID = type(payload) == "table" and type(payload.FlowID) == "string"
      and payload.FlowID ~= "" and payload.FlowID or nil
    Players[actor]:SetProperty("JAC_ABSORPTION_FLOW_LAST_OPERATION_V1",
      { Accepted = ok, Command = command, FlowID = flowID,
        Reason = not ok and tostring(result) or nil,
        Turn = Game.GetCurrentGameTurn() })
  end
  if not ok then print("[JAC_ABSORPTION_FLOW] operation rejected: " .. tostring(result)) end
end
GameEvents.JAC_AbsorptionFlow_Operation.Add(Operation)
GameEvents.OnGameTurnStarted.Add(F.Tick)
if Events and Events.ImprovementRemovedFromMap then
  Events.ImprovementRemovedFromMap.Add(ImprovementRemoved)
end
if Events and Events.GoodyHutReward then Events.GoodyHutReward.Add(GoodyReward) end
-- Gameplay globals belong to this context; UI receives detached reads only.
JAC_AbsorptionFlowGameplay = F
ExposedMembers.JAC_AbsorptionFlow = {
  Get = F.Get, GetPending = F.GetPending,
  GetWorld = F.GetWorld, GetPendingWorlds = F.GetPendingWorlds,
}
print("[JAC_ABSORPTION_FLOW] loaded")
return F
