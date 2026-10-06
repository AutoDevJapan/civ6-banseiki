//! Normal gameplay admission -> the existing three-stage absorption engine.
//! Rust owns the emitted connection; Civ6 executes the generated Lua in Gameplay.

use crate::{Outputs, Repository};
use anyhow::Result;
use serde_json::{json, Value};

pub const OUTPUT: &str = "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Admission.lua";

pub fn start_calendar_step() -> Value {
    json!({"action":"ui_lua","state_name":"InGame","code":r#"
assert(GameConfiguration.GetRuleSet()=='RULESET_JAC_BANSEIKI','wrong ruleset')
local selected=GameConfiguration.GetCalendarType()
assert(selected=='CALENDAR_STANDARD_FROM_START_ERA' or selected==DB.MakeHash('CALENDAR_STANDARD_FROM_START_ERA'),'wrong start-era calendar')
assert(GameInfo.StartEras['ERA_ANCIENT'].Year==1534,'wrong start-era year')
local turn=Game.GetCurrentGameTurn()
assert(turn==GameConfiguration.GetStartTurn(),'not the actual start turn')
local date=Calendar.MakeYearStr(turn)
local numbers={};for number in tostring(date):gmatch('%d+') do numbers[#numbers+1]=number end
assert(#numbers==1 and numbers[1]=='1534','wrong actual start date: '..tostring(date))
return 'verified start date='..date
"#})
}

pub fn generate(_repo: &Repository) -> Result<Outputs> {
    Ok([(OUTPUT.into(), lua())].into_iter().collect())
}

/// Reuse the proven map/Hero startup, but start absorption through a normal
/// offer and a real turn boundary, never the test-call bridge or F.Begin.
pub fn live_scenario(repo: &Repository) -> Result<String> {
    let base = repo.json("tools/civ6-autotest/scenarios/absorption-flow.json")?;
    let mut steps: Vec<Value> = base["steps"].as_array().unwrap()[..7].to_vec();
    let setup = steps[6]["code"].as_str().expect("base actor setup Lua");
    let clause = " and Game.GetLocalPlayer()==to";
    anyhow::ensure!(
        setup.matches(clause).count() == 1,
        "normal actor identity clause changed"
    );
    steps[6]["code"] = json!(setup
        .replacen(clause, "", 1)
        .replace("CIVILIZATION_JAC_ASHINA", "CIVILIZATION_JAC_TAKEDA")
        .replace("AI Ashina", "AI Takeda"));
    steps.push(json!({"action":"ui_lua","state_name":"InGame","code":r#"
local to
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
  if PlayerConfigurations[p:GetID()]:GetCivilizationTypeName()=='CIVILIZATION_JAC_ODA' then to=p:GetID() end
end
assert(to~=nil and Players[to]:IsHuman() and Game.GetLocalPlayer()==to,'visible human Oda controller required')
return 'UI Oda controller verified separately from Gameplay actor identity'
"#}));
    steps.push(start_calendar_step());
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
assert(Players[t.From]:GetCities():GetCount()>0,'phase one unexpectedly removed all source cities')
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
            "judge_profile":"generic", "loadscreen_input":"escape", "steps":steps
        }))?
    ))
}

/// Exercise the official influence API in both game-core and UI contexts,
/// including a positive change and a real save/load. This diagnoses API
/// availability; it does not claim a production leader offer was fulfilled.
pub fn envoy_probe_scenario(repo: &Repository) -> Result<String> {
    let base = repo.json("tools/civ6-autotest/scenarios/absorption-flow.json")?;
    let mut configure = base["steps"][1].clone();
    // The configure contract accepts qualified Civ6Map files only. Select the
    // official map script explicitly in FrontEnd, where its availability can
    // be verified before starting the diagnostic game.
    configure.as_object_mut().unwrap().remove("map_file");
    configure["map_size"] = json!("MAPSIZE_DUEL");
    configure["city_states"] = json!(1);
    configure["players"] = json!(&configure["players"].as_array().unwrap()[..2]);
    // The normal setup applies Banseiki's start-turn/calendar override.
    configure["build_setup"] = json!(true);
    let mut setup = base["steps"][2].clone();
    setup["code"] = json!(format!(
        "MapConfiguration.SetValue('MAP_SCRIPT','Continents.lua');assert(MapConfiguration.GetValue('MAP_SCRIPT')=='Continents.lua','official map script not selected');GameConfiguration.SetValue('CITY_STATE_COUNT',1);assert(GameConfiguration.GetValue('CITY_STATE_COUNT')==1,'diagnostic minor count not selected');{}",
        setup["code"].as_str().unwrap()
    ));
    let mut steps = vec![
        base["steps"][0].clone(),
        configure,
        setup,
        base["steps"][3].clone(),
        base["steps"][5].clone(),
    ];
    // Let the real minor AI found its capital before testing influence.
    steps.push(start_calendar_step());
    steps.push(json!({"action":"end_turn","timeout":180}));
    steps.push(base["steps"][5].clone());
    steps.push(
        json!({"action":"ui_lua", "state_name":"JAC_Absorption_Entry", "code":r#"
local from,to,candidates=nil,nil,{}
for _,player in ipairs(PlayerManager.GetAliveMinors()) do
  local influence=player:GetInfluence()
  local receives=influence and type(influence.CanReceiveInfluence)=='function' and influence:CanReceiveInfluence()
  candidates[#candidates+1]=tostring(player:GetID())..':'..tostring(receives)
  if receives then
    from=player:GetID();break
  end
end
for _,player in ipairs(PlayerManager.GetAliveMajors()) do
  if player:IsHuman() then to=player:GetID();break end
end
print('[JAC_ADMISSION_ENVOY_CANDIDATES] '..table.concat(candidates,','))
assert(from and to,'one influence-receiving minor and one human major required: '..table.concat(candidates,','))
Players[to]:GetDiplomacy():SetHasMet(from)
Players[from]:GetDiplomacy():SetHasMet(to)
Game:SetProperty('JAC_ADMISSION_ENVOY_PROBE_V1',{From=from,To=to})
return 'official influence recipient selected; bilateral contact initialized'
"#}),
    );
    steps.push(json!({"action":"wait_for","timeout":120,"condition":{
        "type":"lua","state_name":"InGame","code":r#"
local t=Game:GetProperty('JAC_ADMISSION_ENVOY_PROBE_V1')
return t and Players[t.To]:GetDiplomacy():HasMet(t.From) and Players[t.From]:GetDiplomacy():HasMet(t.To)
"#}}));
    steps.push(json!({"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":r#"
local t=assert(Game:GetProperty('JAC_ADMISSION_ENVOY_PROBE_V1'))
local from,to=t.From,t.To
local recipient=Players[from]:GetInfluence()
local sender=Players[to]:GetInfluence()
assert(type(recipient.GetTokensReceived)=='function','Gameplay GetTokensReceived unavailable')
assert(type(sender.GiveFreeTokenToPlayer)=='function','official influence grant unavailable')
local before=recipient:GetTokensReceived(to)
assert(type(before)=='number' and before>=0,'invalid starting envoy count')
assert(t.Before==nil,'diagnostic grant already issued')
t.Before=before;Game:SetProperty('JAC_ADMISSION_ENVOY_PROBE_V1',t)
sender:GiveFreeTokenToPlayer(from)
return 'one official grant issued to '..tostring(from)..' from '..tostring(to)..'; before='..tostring(before)
"#}));
    steps.push(json!({"action":"wait_for","timeout":120,"condition":{
        "type":"lua","state_name":"InGame","code":r#"
local t=Game:GetProperty('JAC_ADMISSION_ENVOY_PROBE_V1')
return t and t.Before~=nil and Players[t.From]:GetInfluence():GetTokensReceived(t.To)>t.Before
"#}}));
    steps.push(
        json!({"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":r#"
local t=assert(Game:GetProperty('JAC_ADMISSION_ENVOY_PROBE_V1'))
local after=Players[t.From]:GetInfluence():GetTokensReceived(t.To)
assert(type(after)=='number' and after>t.Before,
  'official envoy grant not observed: before='..tostring(t.Before)..' after='..tostring(after))
t.After=after;Game:SetProperty('JAC_ADMISSION_ENVOY_PROBE_V1',t)
return 'Gameplay positive envoy count verified: '..tostring(t.Before)..' -> '..tostring(after)
"#}),
    );
    let ui_check = json!({"action":"ui_lua", "state_name":"InGame", "code":r#"
local t=assert(Game:GetProperty('JAC_ADMISSION_ENVOY_PROBE_V1'))
assert(Players[t.From]:GetInfluence():GetTokensReceived(t.To)==t.After,'UI and Gameplay envoy counts differ')
return 'UI recipient envoy count agrees with game-core'
"#});
    steps.push(ui_check.clone());
    steps.push(json!({"action":"save", "name":"JAC-admission-995-envoys", "timeout":90}));
    steps.push(json!({"action":"exit_to_menu", "timeout":90}));
    steps.push(json!({"action":"load", "name":"JAC-admission-995-envoys", "timeout":600}));
    steps.push(base["steps"][5].clone());
    steps.push(json!({"action":"ui_lua", "state_name":"JAC_Absorption_Entry", "code":r#"
local t=assert(Game:GetProperty('JAC_ADMISSION_ENVOY_PROBE_V1'))
assert(Players[t.From]:GetInfluence():GetTokensReceived(t.To)==t.After,'saved Gameplay envoy count changed')
return 'real save/load preserved positive Gameplay envoy count'
"#}));
    steps.push(ui_check);
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&json!({
            "name":"ToDo995 Gameplay/UI positive envoy API and save/load diagnostic",
            "judge_profile":"generic", "steps":steps
        }))?
    ))
}

pub fn lua() -> String {
    r#"-- Generated by absorption-admission-generate (Rust). Do not edit.
-- Gameplay-only callbacks re-read the normal admission decision, never UI flags.
local A, F = assert(JAC_AbsorptionGameplay), assert(JAC_AbsorptionFlowGameplay)
local R = A.Rules
local N, providers, readers, envoyReaders, starting, reading = {}, {}, {}, {}, {}, {}
local KEY = 'JAC_ABSORPTION_ADMISSION_V1'
local function State()
  local s = R.Copy(Game:GetProperty(KEY) or {Version=2, Requests={}, Offers={}})
  assert((s.Version == 1 or s.Version == 2) and type(s.Requests) == 'table', 'unsupported admission save')
  -- Retain legacy requests, but write lifecycle changes as v2. Older builds
  -- must reject the new save rather than retry a CANCELLED v1 request.
  s.Version = 2
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
local function NewRequest(source, id, from, to)
  return {Source=source, ID=id, From=from, To=to,
    FlowID='normal-admission:' .. Key(source,id), Status='WAITING'}
end
function N.IsOccupied(actor, ownID)
  local journal = R.Copy(Game:GetProperty('JAC_ABSORPTION_FLOW_V1'))
  if journal then
    assert(journal.Version == 1 and type(journal.Flows) == 'table', 'unsupported absorption flow save')
    for id, flow in pairs(journal.Flows) do
      if id ~= ownID and flow.Phase ~= 'DONE' and (flow.From == actor or flow.To == actor) then return true end
    end
  end
  -- Reserve both actors before F.Begin. A synchronous callback must not
  -- prepare a competing request while the first inventory is being read.
  for _, other in pairs(State().Requests) do
    if other.FlowID ~= ownID and other.Status == 'STARTING' and other.Plan and not other.LastError
      and (other.From == actor or other.To == actor) then return true end
  end
  return false
end
local function Wait(s, request, status)
  status = status or 'WAITING'
  if request.Status ~= status or request.Plan or request.Decision or request.LastError then
    request.Status, request.Plan, request.Decision, request.LastError = status, nil, nil, nil
    Save(s)
  end
  return R.Copy(request)
end
local function CancelInState(request)
  if request.Status == 'CANCELLED' then return end
  local journal = FlowRequest(request.FlowID)
  local waiting = request.Status == 'WAITING' or request.Status == 'UNAVAILABLE'
  local failedPreflight = (waiting or request.Status == 'STARTING') and request.LastError ~= nil
  assert(not starting[request.FlowID] and not journal
    and (waiting and not request.Plan or failedPreflight),
    'cannot cancel an admission after its plan is frozen')
  request.Status, request.CancelledTurn = 'CANCELLED', Game.GetCurrentGameTurn()
  request.LastError = nil
end
local function Ready(request, decision)
  assert(type(decision) == 'table', 'normal admission decision required')
  assert(decision.From == request.From and decision.To == request.To,
    'normal admission factions changed')
  assert(decision.Turn == Game.GetCurrentGameTurn(), 'stale normal admission decision')
  assert(decision.Mode == 'SMALL_STATE' or decision.Mode == 'MERGER',
    'only small-state admission or merger can start this connection')
  assert(Number(decision.AreaKm2) and decision.AreaKm2 > 0, 'positive real country area required')
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
  table.sort(decision.Requirements, function(a,b) return a.ID<b.ID end)
  assert(type(decision.RelationsResolved) == 'boolean', 'relation prerequisites required')
  if not met or not decision.RelationsResolved then return false end
  if decision.Mode == 'SMALL_STATE' then
    local eligible=decision.CanReceiveInfluence
    if decision.EnvoySource then
      assert(envoyReaders[decision.EnvoySource], 'registered envoy source required')
      eligible=decision.EnvoysEligible
    end
    assert(type(eligible) == 'boolean', 'live influence eligibility required')
    if not eligible then return false end
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
  if request.Status == 'STARTED' or request.Status == 'CANCELLED' then return R.Copy(request) end
  assert(Players[request.To] and Players[request.To]:IsMajor(), 'admission recipient must be a major civilization')
  if reading[request.FlowID] or starting[request.FlowID] then return R.Copy(request) end
  -- Once F.Begin has journalled P1, retry its frozen plan even if the country
  -- has already been transferred. F owns the saved rolls and asset inventory.
  local existing = FlowRequest(request.FlowID)
  if existing then
    assert(request.Plan and (not existing or R.Equal(existing, request.Plan)),
      'admission flow journal differs from its saved plan')
    assert(request.Plan.ID == request.FlowID and request.Plan.From == request.From
      and request.Plan.To == request.To, 'frozen admission identity changed')
  else
    -- Only a persisted Flow journal freezes the plan. A preflight failure
    -- or legacy queued intent must re-read conditions before any new roll.
    if N.IsOccupied(request.From, request.FlowID) or N.IsOccupied(request.To, request.FlowID) then
      return Wait(s, request)
    end
    local read = providers[source]
    if not read then return Wait(s, request) end -- another module may load later
    if not Players[request.From] or not Players[request.From]:IsAlive() or not Players[request.To]:IsAlive() then
      return Wait(s, request, 'UNAVAILABLE')
    end
    assert(not Players[request.From]:IsHuman(), 'existing flow absorbs AI sources only')
    local offerBefore = source == 'leader-offer' and R.Copy(s.Offers[id]) or nil
    reading[request.FlowID] = true
    local ok, value = pcall(read, id, request.From, request.To)
    reading[request.FlowID] = nil
    if not ok then error(value) end
    local latest = State()
    local current = assert(latest.Requests[key], 'normal admission request disappeared')
    if not R.Equal(current,request) or source == 'leader-offer' and not R.Equal(latest.Offers[id],offerBefore) then
      return R.Copy(current)
    end
    s, request = latest, current
    local decision = R.Copy(value)
    if N.IsOccupied(request.From, request.FlowID) or N.IsOccupied(request.To, request.FlowID)
      or not Ready(request, decision) then
      return Wait(s, request)
    end
    local plan = R.Copy(decision.Plan or {})
    -- These fields are authoritative gate output, not values from the caller.
    plan.ID, plan.From, plan.To, plan.Mode = request.FlowID, request.From, request.To, decision.Mode
    plan.EntryConditionsMet, plan.RelationsResolved = true, true
    request.Plan, request.Decision = plan, decision
    request.Status, request.LastError = 'STARTING', nil; Save(s)
  end
  starting[request.FlowID] = true
  local ok, result = pcall(F.Begin, R.Copy(request.Plan))
  starting[request.FlowID] = nil
  if not ok then error(result) end
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
  assert(Players[to]:IsMajor(), 'admission recipient must be a major civilization')
  local s, key = State(), Key(source, id)
  local request = s.Requests[key]
  if request then
    assert(request.From == from and request.To == to, 'normal admission ID reused')
  else
    request = NewRequest(source,id,from,to)
    s.Requests[key] = request; Save(s)
  end
  if request.Status ~= 'CANCELLED' then Try(source, id) end
  return N.Get(source, id)
end
function N.Cancel(source, id)
  assert(ID(source) and ID(id), 'admission cancellation identity required')
  local s, key = State(), Key(source,id)
  local request = assert(s.Requests[key], 'normal admission request missing')
  if request.Status ~= 'CANCELLED' then CancelInState(request); Save(s) end
  return R.Copy(request)
end
function N.Tick()
  local s = State()
  -- Defined leader offers are evaluated by normal gameplay without JAC_AUTOTEST.
  for _, id in ipairs(R.Keys(s.Offers)) do
    local offer = s.Offers[id]
    if not s.Requests[Key('leader-offer', id)] and Players[offer.To] and Players[offer.To]:IsMajor() then
      N.Request('leader-offer', id, offer.From, offer.To)
    end
  end
  for _, key in ipairs(R.Keys(s.Requests)) do
    local request = s.Requests[key]
    if request.Status ~= 'STARTED' and request.Status ~= 'CANCELLED' then Try(request.Source, request.ID) end
  end
end
function N.RegisterRequirement(kind, read)
  assert(ID(kind) and type(read) == 'function' and not readers[kind],
    'unique live leader requirement reader required')
  readers[kind] = read
end
function N.RegisterEnvoys(source, read)
  assert(ID(source) and type(read)=='function' and not envoyReaders[source], 'unique envoy reader required')
  envoyReaders[source]=read
end
function N.GetOffer(id) return R.Copy(State().Offers[id]) end
function N.CanCancelOffer(id)
  local s=State();if not s.Offers[id] then return false end
  local request=s.Requests[Key('leader-offer',id)]
  if not request then return true end
  if request.Status=='CANCELLED' or starting[request.FlowID] or FlowRequest(request.FlowID) then return false end
  local waiting=request.Status=='WAITING' or request.Status=='UNAVAILABLE'
  return waiting and not request.Plan or (waiting or request.Status=='STARTING') and request.LastError~=nil
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
  table.sort(list, function(a,b) return a.ID<b.ID end)
end
-- Called by normal map/leader rule modules with their explicit demands. Values
-- are not a universal personality formula and never use the SSOT example costs.
local function ValidateOffer(offer)
  offer = R.Copy(offer)
  assert(type(offer) == 'table' and ID(offer.ID) and Player(offer.From)
    and Player(offer.To) and offer.From ~= offer.To and Number(offer.AreaKm2) and offer.AreaKm2 > 0,
    'leader offer identity and real country area required')
  assert(Players[offer.To]:IsMajor(), 'admission recipient must be a major civilization')
  DemandList(offer.Requirements); DemandList(offer.RelationRequirements)
  assert(type(offer.RelationsReviewed) == 'boolean', 'explicit relationship review required')
  assert(#offer.Requirements > 0, 'explicit leader demands required')
  return offer
end
function N.DefineOffer(offer)
  offer = ValidateOffer(offer)
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
function N.CancelOffer(id)
  assert(ID(id), 'leader offer cancellation ID required')
  local s, key = State(), Key('leader-offer',id)
  local offer = assert(s.Offers[id], 'leader offer missing')
  local request = s.Requests[key] or NewRequest('leader-offer',id,offer.From,offer.To)
  if request.Status ~= 'CANCELLED' then
    CancelInState(request); s.Requests[key] = request; Save(s)
  end
  return R.Copy(request)
end
-- A changing leader's terms use a new ID. Retire the old request and publish
-- the replacement in one saved state, before any turn can evaluate either.
function N.ReplaceOffer(previousID, offer)
  assert(ID(previousID), 'previous leader offer ID required')
  offer = ValidateOffer(offer)
  assert(previousID ~= offer.ID, 'replacement needs a new offer ID')
  local s, key = State(), Key('leader-offer',previousID)
  local previous = assert(s.Offers[previousID], 'previous leader offer missing')
  assert(previous.From == offer.From and previous.To == offer.To, 'replacement factions changed')
  local request = s.Requests[key] or NewRequest('leader-offer',previousID,previous.From,previous.To)
  if request.ReplacedBy then
    assert(request.Status == 'CANCELLED' and request.ReplacedBy == offer.ID,
      'previous offer already has a different replacement')
    local published = R.Copy(assert(s.Offers[offer.ID], 'replacement offer missing'))
    -- A replay must not undo a newer relation review on the same demands.
    published.RelationsReviewed = offer.RelationsReviewed
    assert(R.Equal(published,offer), 'replacement ID reused with changed demands')
    return R.Copy(request)
  end
  assert(not s.Offers[offer.ID] and not s.Requests[Key('leader-offer',offer.ID)],
    'replacement offer ID already exists')
  CancelInState(request)
  request.ReplacedBy = offer.ID
  s.Requests[key], s.Offers[offer.ID] = request, offer
  Save(s)
  return R.Copy(request)
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
  return A.PlayerFaithBalance(to) >= demand.Minimum
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
    if offer.EnvoySource then
      local read=assert(envoyReaders[offer.EnvoySource], 'product envoy reader missing')
      local envoys=read(from,to,R.Copy(offer))
      decision.EnvoySource,decision.EnvoysEligible=offer.EnvoySource,envoys.Eligible
      decision.Envoys=envoys.Envoys
    else
      local influence = Players[from]:GetInfluence()
      decision.CanReceiveInfluence = influence:CanReceiveInfluence()
      decision.Envoys = influence:GetTokensReceived(to)
    end
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
