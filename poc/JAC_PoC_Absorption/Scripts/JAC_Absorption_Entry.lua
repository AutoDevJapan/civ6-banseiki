-- One AddGameplayScripts entry: include executes all modules in this context.
-- No gameplay dependency is obtained through a cross-context function table.
if JAC_AbsorptionEntryLoaded then return end
include("JAC_Absorption_Gameplay")
include("JAC_Absorption_Flow")
include("JAC_Absorption_Admission")
include("JAC_Absorption_Product")
local A, F = assert(JAC_AbsorptionGameplay), assert(JAC_AbsorptionFlowGameplay)
if GameInfo.JAC_RP_Religions then
  JAC_RP_AdapterInclude = true
  include("JAC_ReligionPersistence")
  JAC_RP_AdapterInclude = nil
  include("JAC_RP_AbsorptionAdapter")
  A.RegisterAdapter("religion-persistence", assert(JAC_RP_AbsorptionAdapter))
end
local R = A.Rules
local methods = {
  ENGINE = { Apply=true, Resume=true, Verify=true, RequestVerification=true,
    RegisterUnitProperty=true, RegisterCityProperty=true,
    BeginHeroRecall=true, CompleteHeroRecall=true,
    RecoverArchived=true, BeginRecoveredHeroRecall=true, CompleteRecoveredHeroRecall=true,
    ApplyCityFriction=true, GetState=true, GetRoster=true, GetUIPhase=true,
    OwnedGreatWorkSlots=true, NativeWorkCapabilities=true },
  FLOW = { Begin=true, Select=true, Resolve=true, Resume=true, Acknowledge=true,
    ResumeWorld=true, Tick=true, Get=true, GetPending=true, GetWorld=true,
    GetPendingWorlds=true, DebugSetRoll=true, DebugSetSpawnDelay=true },
}
local function TestCall(id, domain, method, args)
  -- Tuner has GameEvents in its game-core context. Nothing is exported to UI,
  -- and the normal player-operation handler never dispatches this test event.
  local enabled = GameConfiguration and GameConfiguration.GetValue
    and GameConfiguration.GetValue("JAC_AUTOTEST")
  if (enabled ~= 1 and enabled ~= true) or not GameConfiguration.IsAnyMultiplayer
    or GameConfiguration.IsAnyMultiplayer() then return end
  assert(type(id) == "string" and id ~= "", "test call ID required")
  local ok, result = pcall(function()
    assert(methods[domain] and methods[domain][method], "unknown test operation")
    assert(type(args) == "table", "test arguments required")
    args = R.Copy(args) -- no callbacks, userdata, or mutable context references
    local api = domain == "ENGINE" and A or F
    return R.Copy(api[method](unpack(args)))
  end)
  local reply = { ID=id, Accepted=ok }
  if ok then reply.Result = result else reply.Error = tostring(result) end
  Game:SetProperty("JAC_ABSORPTION_TEST_RESULT_V1", reply)
end
GameEvents.JAC_Absorption_TestCall.Add(TestCall)
JAC_AbsorptionEntryLoaded = true
