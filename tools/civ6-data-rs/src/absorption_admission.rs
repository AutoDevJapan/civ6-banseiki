//! Normal gameplay admission -> the existing three-stage absorption engine.
//! Rust owns the emitted connection; Civ6 executes the generated Lua in Gameplay.

use crate::{Outputs, Repository};
use anyhow::Result;
use serde_json::{json, Value};

pub const OUTPUT: &str = "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Admission.lua";

pub fn generate(_repo: &Repository) -> Result<Outputs> {
    Ok([(OUTPUT.into(), lua())].into_iter().collect())
}

/// Reuse the proven map/Hero startup, but start absorption through a normal
/// offer and a real turn boundary, never the test-call bridge or F.Begin.
pub fn live_scenario(repo: &Repository) -> Result<String> {
    let base = repo.json("tools/civ6-autotest/scenarios/absorption-flow.json")?;
    let mut steps: Vec<Value> = base["steps"].as_array().unwrap()[..7].to_vec();
    fn core(code: &str) -> Value {
        json!({"action":"ui_lua", "state_name":"JAC_Absorption_Entry", "code":code})
    }
    fn wait_active() -> Value {
        json!({"action":"wait_for", "timeout":600, "condition":{
            "type":"lua", "state_name":"InGame", "code":
            "local p=Players[Game.GetLocalPlayer()];return p and p:IsTurnActive() and ExposedMembers.JAC_AbsorptionFlow~=nil"}})
    }
    steps.push(core(r#"
local N=assert(JAC_AbsorptionAdmissionGameplay)
local t=assert(Game:GetProperty('JAC_ABSORPTION_FLOW_LIVE_TEST'))
assert(not Game:GetProperty('JAC_ABSORPTION_FLOW_TEST_MODE'))
local treasury=Players[t.To]:GetTreasury()
local minimum=treasury:GetGoldBalance()+1000000
local offer={ID='todo995-normal-merger',From=t.From,To=t.To,AreaKm2=10000,
  Requirements={{ID='test-reserve',Kind='GOLD_MINIMUM',Minimum=minimum}},
  RelationRequirements={}, RelationsReviewed=true}
-- Explicit diagnostic demand, not a universal balance rule or a payment.
N.DefineOffer(offer)
Game:SetProperty('JAC_ABSORPTION_ADMISSION_LIVE_V1',{ID=offer.ID,From=t.From,To=t.To,Minimum=minimum})
return 'normal leader offer saved; no test start used'
"#));
    steps.push(json!({"action":"end_turn", "timeout":180}));
    steps.push(wait_active());
    steps.push(core(
        r#"
local N=assert(JAC_AbsorptionAdmissionGameplay)
local t=assert(Game:GetProperty('JAC_ABSORPTION_ADMISSION_LIVE_V1'))
local receipt=assert(N.Get('leader-offer',t.ID))
assert(receipt.Status=='WAITING' and not receipt.Plan and not receipt.LastError)
assert(not JAC_AbsorptionFlowGameplay.Get(receipt.FlowID))
return 'ordinary turn waited for unmet live demand'
"#,
    ));
    steps.push(json!({"action":"save", "name":"JAC-admission-995-waiting", "timeout":90}));
    steps.push(json!({"action":"exit_to_menu", "timeout":90}));
    steps.push(json!({"action":"load", "name":"JAC-admission-995-waiting", "timeout":600}));
    steps.push(wait_active());
    steps.push(core(
        r#"
local N=assert(JAC_AbsorptionAdmissionGameplay)
local t=assert(Game:GetProperty('JAC_ABSORPTION_ADMISSION_LIVE_V1'))
assert(N.Get('leader-offer',t.ID).Status=='WAITING')
local treasury=Players[t.To]:GetTreasury()
treasury:ChangeGoldBalance(math.max(0,t.Minimum+1000000-treasury:GetGoldBalance()))
return 'waiting offer restored; diagnostic reserve supplied without calling absorption'
"#,
    ));
    steps.push(json!({"action":"end_turn", "timeout":180}));
    steps.push(wait_active());
    steps.push(core(
        r#"
local N=assert(JAC_AbsorptionAdmissionGameplay)
local t=assert(Game:GetProperty('JAC_ABSORPTION_ADMISSION_LIVE_V1'))
local receipt=assert(N.Get('leader-offer',t.ID))
assert(receipt.Status=='STARTED',receipt.LastError or receipt.Status)
local flow=assert(JAC_AbsorptionFlowGameplay.Get(receipt.FlowID))
assert(flow.Mode=='MERGER' and flow.Phase=='CHOICE' and flow.Phase1Complete)
assert(flow.From==t.From and flow.To==t.To and #flow.Entries>0)
assert(Players[t.From]:GetCities():GetCount()>0,'phase one transferred cities')
Game:SetProperty('JAC_ABSORPTION_ADMISSION_LIVE_FLOW_V1',JAC_AbsorptionGameplay.Canonical(flow))
return 'ordinary turn entered saved three-stage choice'
"#,
    ));
    steps.push(json!({"action":"save", "name":"JAC-admission-995-choice", "timeout":90}));
    steps.push(json!({"action":"exit_to_menu", "timeout":90}));
    steps.push(json!({"action":"load", "name":"JAC-admission-995-choice", "timeout":600}));
    steps.push(wait_active());
    steps.push(core(r#"
local N,F,A=assert(JAC_AbsorptionAdmissionGameplay),assert(JAC_AbsorptionFlowGameplay),assert(JAC_AbsorptionGameplay)
local t=assert(Game:GetProperty('JAC_ABSORPTION_ADMISSION_LIVE_V1'))
local receipt=assert(N.Get('leader-offer',t.ID))
local before=assert(Game:GetProperty('JAC_ABSORPTION_ADMISSION_LIVE_FLOW_V1'))
assert(receipt.Status=='STARTED' and A.Canonical(F.Get(receipt.FlowID))==before)
N.Request('leader-offer',t.ID,t.From,t.To)
assert(A.Canonical(F.Get(receipt.FlowID))==before,'normal request repeated first-stage rolls')
return 'real save/load preserved choice and all first-stage rolls; duplicate request inert'
"#));
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&json!({
            "name":"ToDo995 normal merger admission: unmet offer, natural turn, save/load, single P1",
            "judge_profile":"generic", "steps":steps
        }))?
    ))
}

pub fn lua() -> String {
    r#"-- Generated by absorption-admission-generate (Rust). Do not edit.
-- Gameplay-only callbacks re-read the normal admission decision, never UI flags.
local A, F = assert(JAC_AbsorptionGameplay), assert(JAC_AbsorptionFlowGameplay)
local R = A.Rules
local N, providers, readers = {}, {}, {}
local KEY = 'JAC_ABSORPTION_ADMISSION_V1'
local function State()
  local s = R.Copy(Game:GetProperty(KEY) or {Version=1, Requests={}, Offers={}})
  assert(s.Version == 1 and type(s.Requests) == 'table', 'unsupported admission save')
  s.Offers = s.Offers or {}
  return s
end
local function Save(s) Game:SetProperty(KEY, R.Copy(s)) end
local function FlowRequest(id)
  -- F.Get intentionally hides incomplete P1 from UI. Read its versioned,
  -- persisted request here in Gameplay so interrupted admissions can resume.
  local s = R.Copy(Game:GetProperty('JAC_ABSORPTION_FLOW_V1'))
  if not s then return nil end
  assert(s.Version == 1 and type(s.Flows) == 'table', 'unsupported absorption flow save')
  local flow = s.Flows[id]
  return flow and R.Copy(flow.Request) or nil
end
local function ID(value)
  return type(value) == 'string' and value ~= '' and #value <= 256
end
local function Number(value)
  return type(value) == 'number' and value == value and value >= 0 and value < math.huge
end
local function Player(value)
  return Number(value) and value == math.floor(value) and Players[value] ~= nil
end
local function Key(source, id)
  return #source .. ':' .. source .. #id .. ':' .. id
end
local function Ready(request, decision)
  assert(type(decision) == 'table', 'normal admission decision required')
  assert(decision.From == request.From and decision.To == request.To,
    'normal admission factions changed')
  assert(decision.Turn == Game.GetCurrentGameTurn(), 'stale normal admission decision')
  assert(decision.Mode == 'SMALL_STATE' or decision.Mode == 'MERGER',
    'only small-state admission or merger can start this connection')
  assert(Number(decision.AreaKm2), 'real country area required')
  assert((decision.Mode == 'SMALL_STATE') == (decision.AreaKm2 < 10000),
    'country area and admission mode disagree')
  assert(type(decision.Requirements) == 'table' and #decision.Requirements > 0,
    'explicit leader requirements required')
  local seen, count, met = {}, 0, true
  for index, condition in pairs(decision.Requirements) do
    assert(type(index) == 'number' and index == math.floor(index) and index >= 1
      and index <= #decision.Requirements, 'dense requirement list required')
    assert(type(condition) == 'table' and ID(condition.ID) and not seen[condition.ID]
      and type(condition.Met) == 'boolean', 'invalid or duplicate leader requirement')
    seen[condition.ID], count = true, count + 1
    met = met and condition.Met
  end
  assert(count == #decision.Requirements, 'missing leader requirement')
  assert(type(decision.RelationsResolved) == 'boolean', 'relation prerequisites required')
  if not met or not decision.RelationsResolved then return false end
  if decision.Mode == 'SMALL_STATE' then
    assert(Number(decision.OwnedTiles) and decision.OwnedTiles == math.floor(decision.OwnedTiles),
      'owned tile count required separately from country area')
    assert(Number(decision.Envoys) and decision.Envoys == math.floor(decision.Envoys),
      'envoy count required')
    if decision.Envoys < math.max(1, decision.OwnedTiles) then return false end
  end
  return true
end
function N.Register(source, read)
  assert(ID(source) and type(read) == 'function', 'normal admission reader required')
  assert(not providers[source], 'normal admission reader already registered')
  providers[source] = read
end
function N.Get(source, id)
  return R.Copy(State().Requests[Key(source, id)])
end
function N.Check(source, id)
  local s, key = State(), Key(source, id)
  local request = assert(s.Requests[key], 'normal admission request missing')
  if request.Status == 'STARTED' then return R.Copy(request) end
  -- Once F.Begin has journalled P1, retry its frozen plan even if the country
  -- has already been transferred. F owns the saved rolls and asset inventory.
  local existing = FlowRequest(request.FlowID)
  if existing then
    assert(request.Plan and R.Equal(existing, request.Plan),
      'admission flow journal differs from its saved plan')
  else
    local read = providers[source]
    if not read then return R.Copy(request) end -- another module may load later
    if not Players[request.From]:IsAlive() or not Players[request.To]:IsAlive() then
      request.Status = 'UNAVAILABLE'; Save(s); return R.Copy(request)
    end
    assert(not Players[request.From]:IsHuman(), 'existing flow absorbs AI sources only')
    local decision = R.Copy(read(id, request.From, request.To))
    if not Ready(request, decision) then
      request.Status = 'WAITING'; Save(s); return R.Copy(request)
    end
    local plan = R.Copy(decision.Plan or {})
    -- These fields are authoritative gate output, not values from the caller.
    plan.ID, plan.From, plan.To, plan.Mode = request.FlowID, request.From, request.To, decision.Mode
    plan.EntryConditionsMet, plan.RelationsResolved = true, true
    request.Plan, request.Decision = plan, decision
    request.Status = 'STARTING'; Save(s)
  end
  local result = F.Begin(R.Copy(request.Plan))
  assert(result and result.ID == request.FlowID and result.Phase1Complete,
    'three-stage admission did not finish phase one')
  -- F.Begin can synchronously resolve AI flows; reload to preserve other requests.
  s = State(); request = s.Requests[key]
  request.Status, request.StartTurn = 'STARTED', Game.GetCurrentGameTurn()
  request.LastError = nil
  Save(s)
  return R.Copy(request)
end
local function Try(source, id)
  local ok, result = pcall(N.Check, source, id)
  if not ok then
    local s, key = State(), Key(source, id)
    if s.Requests[key] then
      s.Requests[key].LastError = tostring(result); Save(s)
    end
    print('[JAC_ABSORPTION_ADMISSION] rejected: ' .. tostring(result))
  end
  return ok, result
end
function N.Request(source, id, from, to)
  assert(ID(source) and ID(id) and Player(from) and Player(to) and from ~= to,
    'normal admission request identity required')
  assert(providers[source], 'normal admission reader not registered')
  local s, key = State(), Key(source, id)
  local request = s.Requests[key]
  if request then
    assert(request.From == from and request.To == to, 'normal admission ID reused')
  else
    request = {Source=source, ID=id, From=from, To=to,
      FlowID='normal-admission:' .. key, Status='WAITING'}
    s.Requests[key] = request; Save(s)
  end
  Try(source, id)
  return N.Get(source, id)
end
function N.Tick()
  local s = State()
  -- Defined leader offers are evaluated by normal gameplay without JAC_AUTOTEST.
  for _, id in ipairs(R.Keys(s.Offers)) do
    local offer = s.Offers[id]
    if not s.Requests[Key('leader-offer', id)] then
      N.Request('leader-offer', id, offer.From, offer.To)
    end
  end
  for _, key in ipairs(R.Keys(s.Requests)) do
    local request = s.Requests[key]
    if request.Status ~= 'STARTED' then Try(request.Source, request.ID) end
  end
end
function N.RegisterRequirement(kind, read)
  assert(ID(kind) and type(read) == 'function' and not readers[kind],
    'unique live leader requirement reader required')
  readers[kind] = read
end
local function DemandList(list)
  assert(type(list) == 'table', 'leader demand list required')
  local seen, count = {}, 0
  for index, demand in pairs(list) do
    assert(type(index) == 'number' and index == math.floor(index) and index >= 1
      and index <= #list and type(demand) == 'table' and ID(demand.ID)
      and ID(demand.Kind) and not seen[demand.ID], 'invalid leader demand list')
    seen[demand.ID], count = true, count + 1
  end
  assert(count == #list, 'missing leader demand')
end
-- Called by normal map/leader rule modules with their explicit demands. Values
-- are not a universal personality formula and never use the SSOT example costs.
function N.DefineOffer(offer)
  offer = R.Copy(offer)
  assert(type(offer) == 'table' and ID(offer.ID) and Player(offer.From)
    and Player(offer.To) and offer.From ~= offer.To and Number(offer.AreaKm2),
    'leader offer identity and real country area required')
  DemandList(offer.Requirements); DemandList(offer.RelationRequirements)
  assert(type(offer.RelationsReviewed) == 'boolean', 'explicit relationship review required')
  assert(#offer.Requirements > 0, 'explicit leader demands required')
  local s = State()
  if s.Offers[offer.ID] then
    local previous = s.Offers[offer.ID]
    if previous.RelationsReviewed ~= offer.RelationsReviewed then
      local request = s.Requests[Key('leader-offer', offer.ID)]
      assert(not request or request.Status == 'WAITING' or request.Status == 'UNAVAILABLE',
        'cannot change relation review after admission starts')
      local comparable = R.Copy(previous)
      comparable.RelationsReviewed = offer.RelationsReviewed
      assert(R.Equal(comparable, offer), 'leader offer ID reused with changed demands')
      s.Offers[offer.ID] = offer; Save(s)
    else
      assert(R.Equal(previous, offer), 'leader offer ID reused with changed demands')
    end
  else s.Offers[offer.ID] = offer; Save(s) end
end
local function EvaluateDemands(list, from, to)
  local results, all = {}, true
  for _, demand in ipairs(list) do
    local read = assert(readers[demand.Kind], 'live leader requirement reader missing')
    local met = read(from, to, R.Copy(demand))
    assert(type(met) == 'boolean', 'leader demand reader must return boolean')
    results[#results+1] = {ID=demand.ID, Met=met}
    all = all and met
  end
  return results, all
end
N.RegisterRequirement('GOLD_MINIMUM', function(from, to, demand)
  assert(Number(demand.Minimum), 'gold demand amount required')
  return Players[to]:GetTreasury():GetGoldBalance() >= demand.Minimum
end)
N.RegisterRequirement('FAITH_MINIMUM', function(from, to, demand)
  assert(Number(demand.Minimum), 'faith demand amount required')
  return Players[to]:GetReligion():GetFaithBalance() >= demand.Minimum
end)
N.Register('leader-offer', function(id, from, to)
  local offer = assert(State().Offers[id], 'leader offer missing')
  assert(offer.From == from and offer.To == to, 'leader offer factions changed')
  local conditions = EvaluateDemands(offer.Requirements, from, to)
  local _, resolved = EvaluateDemands(offer.RelationRequirements, from, to)
  local decision = {From=from, To=to, Turn=Game.GetCurrentGameTurn(),
    Mode=offer.AreaKm2 < 10000 and 'SMALL_STATE' or 'MERGER',
    AreaKm2=offer.AreaKm2, Requirements=conditions,
    RelationsResolved=offer.RelationsReviewed and resolved,
    Plan=R.Copy(offer.Plan or {})}
  if decision.Mode == 'SMALL_STATE' then
    local tiles = 0
    for index=0, Map.GetPlotCount()-1 do
      local plot = Map.GetPlotByIndex(index)
      if plot and plot:GetOwner() == from then tiles = tiles + 1 end
    end
    decision.OwnedTiles = tiles
    -- Official CityStates.lua uses the recipient's GetTokensReceived(sender).
    decision.Envoys = Players[from]:GetInfluence():GetTokensReceived(to)
  end
  return decision
end)
-- Normal rule modules use this Gameplay event after registering their reader.
-- No BEGIN command is added to the UI operation handler or the test bridge.
GameEvents.JAC_Absorption_AdmissionRequested.Add(N.Request)
GameEvents.JAC_Absorption_LeaderOffer.Add(N.DefineOffer)
GameEvents.OnGameTurnStarted.Add(N.Tick)
JAC_AbsorptionAdmissionGameplay = N
print('[JAC_ABSORPTION_ADMISSION] loaded')
return N
"#
    .to_owned()
}
