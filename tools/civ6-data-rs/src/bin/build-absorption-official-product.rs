//! An additional, opt-in acceptance scene; no checked-in recipe is overwritten.
//!
//! Installed primary references (Sid Meier's Civilization VI):
//! - Base/Assets/Configuration/Data/SetupParameters.xml:26 and
//!   DLC/Expansion2/Config/Expansion2_Config.xml:25: EXCLUDE_CITY_STATES is
//!   a GameConfiguration array, selected from the Expansion2CityStates domain.
//! - DLC/GranColombia_Maya/Data/GranColombia_Maya_ConfigData.xml:79:
//!   CIVILIZATION_VATICAN_CITY is in that domain.
//! - Base/Assets/UI/Tuner/TunerCityPanel.lua:78,103: native district/building
//!   construction; Base/Assets/Gameplay/Data/Buildings.xml:345,346: shrine
//!   and temple yield 2 and 4 faith. Native completion/location is observed in
//!   UI; mutations and all acceptance assertions stay in Gameplay.
//! - DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:25: the production
//!   OFFICIAL route's GiveFreeTokenToPlayer API. This scene never calls it.
//!
//! Same-turn publication notifications replay only the public, read-only Get.
//! Turn handlers and commands are exercised only by real end_turn actions.
//! Generating/checking this JSON is not a claim of actual-game acceptance.

use anyhow::{bail, ensure, Context, Result};
use civ6_data_rs::{absorption_admission, Repository};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const USAGE: &str = "build-absorption-official-product --output <absolute-json-path> [--check]";

const FRONTEND_FILTER: &str = r#"
-- Use the official configuration table, not guessed city-state slots.
local target='CIVILIZATION_VATICAN_CITY'
local rows=DB.ConfigurationQuery("SELECT CivilizationType FROM CityStates WHERE Domain = 'Expansion2CityStates' ORDER BY CivilizationType")
assert(type(rows)=='table' and #rows>1,'official Expansion2 city-state catalog unavailable')
local exclude,seen,found={},{},false
for _,row in ipairs(rows) do
  local civ=row.CivilizationType
  assert(type(civ)=='string' and not seen[civ],'invalid or duplicate official city-state row')
  seen[civ]=true
  if civ==target then found=true else exclude[#exclude+1]=civ end
end
assert(found,'Vatican City DLC city state is unavailable in the official domain')
GameConfiguration.SetValue('EXCLUDE_CITY_STATES',exclude)
local actual=GameConfiguration.GetValue('EXCLUDE_CITY_STATES')
assert(type(actual)=='table' and #actual==#exclude,'official city-state exclusion array was not stored')
local selected={}
for _,civ in ipairs(actual) do
  assert(seen[civ] and civ~=target and not selected[civ],'wrong city-state exclusion')
  selected[civ]=true
end
for _,civ in ipairs(exclude) do assert(selected[civ],'official city-state exclusion missing') end
assert(GameConfiguration.GetValue('CITY_STATE_COUNT')==1,'one official minor required')
assert(MapConfiguration.GetValue('MAP_SCRIPT')=='Continents.lua','official Continents required')
return 'all official city states except Vatican City excluded in GameConfiguration'
"#;

// UI derives identities from its own configurations. No Gameplay property is
// read in UI, including immediately after a Core fixture write or after load.
const UI_ACTORS: &str = r#"
local from,to
for actor=0,63 do
  local config=PlayerConfigurations[actor]
  local civ=config and config:GetCivilizationTypeName()
  if civ=='CIVILIZATION_VATICAN_CITY' then from=actor
  elseif civ=='CIVILIZATION_JAC_TAKEDA' then to=actor end
end
"#;

// These are detached observations of native objects, not replacement product
// readers or caller-supplied admission conditions. Re-read on each poll so an
// older UI publication can never authorize the next construction stage.
const UI_NATIVE_ASSETS: &str = r#"
local function ReadAssets()
  local player=to and Players[to]
  local city=player and player:GetCities():GetCapitalCity()
  if not city then return nil end
  local old=ExposedMembers.JAC_OfficialProductNativeAssets
  local o={Serial=(old and old.Serial or 0)+1,Turn=Game.GetCurrentGameTurn(),Owner=city:GetOwner(),City=city:GetID(),
    X=city:GetX(),Y=city:GetY(),HolySites={}}
  local holy=GameInfo.Districts.DISTRICT_HOLY_SITE.Index
  local site
  for _,district in city:GetDistricts():Members() do if district:GetType()==holy then
    local ownerCity=district:GetCity()
    local plot=Map.GetPlot(district:GetX(),district:GetY())
    o.HolySites[#o.HolySites+1]={City=ownerCity:GetID(),Owner=ownerCity:GetOwner(),Plot=plot:GetIndex(),
      Complete=district:IsComplete(),Pillaged=city:GetDistricts():IsPillaged(holy,plot:GetIndex())}
    site=plot
  end end
  if not site then
    local plots=Map.GetCityPlots():GetPurchasedPlots(city)
    local indices={};for _,index in pairs(plots or {}) do indices[#indices+1]=index end;table.sort(indices)
    for _,index in ipairs(indices) do
      local q=Map.GetPlotByIndex(index);local purchase=Cities.GetPlotPurchaseCity(q)
      if q:GetOwner()==o.Owner and purchase and purchase:GetID()==o.City and purchase:GetOwner()==o.Owner
        and Map.GetPlotDistance(o.X,o.Y,q:GetX(),q:GetY())<=3
        and q:GetDistrictType()==-1 and q:GetImprovementType()==-1 and q:GetResourceType()==-1
        and not q:IsCity() and not q:IsWater() and not q:IsMountain() and not q:IsImpassable() and not q:IsNaturalWonder() then
        site=q;break
      end
    end
  end
  if not site then return o end
  local purchase=Cities.GetPlotPurchaseCity(site)
  o.Plot=site:GetIndex();o.PlotOwner=site:GetOwner()
  o.PlotCity=purchase and purchase:GetID();o.PlotCityOwner=purchase and purchase:GetOwner()
  local buildings=city:GetBuildings();local at={}
  for _,index in ipairs(buildings:GetBuildingsAtLocation(o.Plot)) do
    local row=GameInfo.Buildings[index];if row then at[row.Index]=true end
  end
  for _,kind in ipairs({'BUILDING_SHRINE','BUILDING_TEMPLE'}) do
    local index=GameInfo.Buildings[kind].Index
    o[kind]={Present=buildings:HasBuilding(index),AtSite=at[index]==true,Pillaged=buildings:IsPillaged(index)}
  end
  return o
end
local assets=ReadAssets()
ExposedMembers.JAC_OfficialProductNativeAssets=assets
"#;

const CORE: &str = r#"
local P=assert(JAC_AbsorptionProductGameplay,'ordinary product Gameplay unavailable')
local N=assert(JAC_AbsorptionAdmissionGameplay,'ordinary admission Gameplay unavailable')
local A=assert(JAC_AbsorptionGameplay)
local R=A.Rules
local KEY='JAC_OFFICIAL_PRODUCT_ACCEPTANCE_V1'
local t=Game:GetProperty(KEY)
local function Product() return R.Copy(Game:GetProperty('JAC_ABSORPTION_PRODUCT_V1') or {}) end
local function FaithYield(actor)
  local total=Players[actor]:GetReligion():GetFaithYield()
  assert(type(total)=='number' and total>=0 and total<math.huge,'native player faith yield unavailable')
  return total
end
local function Profile(view)
  assert(view.Status~='UNAVAILABLE',view.Error or 'normal product view unavailable')
  local c=assert(view.Country);local p=assert(c.Profile)
  assert(c.ID=='VATICAN_CITY' and c.AreaKm2==0.44 and view.Mode=='SMALL_STATE','wrong official country or area')
  assert(p.CivilizationType=='CIVILIZATION_VATICAN_CITY' and p.LeaderType=='LEADER_MINOR_CIV_VATICAN_CITY',
    'official source civilization/leader changed')
  assert(p.Personality=='慎重' and p.Trait=='霊神','ordinary source personality/trait changed')
  local config=PlayerConfigurations[t.From]
  assert(config:GetCivilizationTypeName()==p.CivilizationType and config:GetLeaderTypeName()==p.LeaderType,
    'actual minor configuration differs from the product country')
end
local function NonDeposit(view)
  Profile(view)
  assert(not view.Frozen and type(view.Rows)=='table' and #view.Rows>0,'live requirements required')
  local metric
  for _,row in ipairs(view.Rows) do
    if row.Kind=='PRODUCT_METRIC' then
      assert(not metric and row.ID=='leader:FAITH_YIELD','normal Vatican City faith yield demand required')
      metric=row
      assert(type(row.Minimum)=='number' and row.Minimum>0,'normal faith yield minimum missing')
      assert(row.Current==FaithYield(t.To),'product faith yield differs from actual recipient religion yield')
    end
    if row.Kind=='GOLD_MINIMUM' then
      local gold=Players[t.To]:GetTreasury():GetGoldBalance()
      assert(row.Current==gold and gold-view.Deposit.Gold>=row.Minimum,'native reserve would not survive ordinary deposit')
    end
    if row.Kind~='PRODUCT_DEPOSIT' then assert(row.Met==true,'non-deposit condition unmet: '..tostring(row.ID)) end
  end
  assert(metric and metric.Current>=metric.Minimum,'actual native faith yield does not meet ordinary terms')
  assert(view.Envoys and view.Envoys.Route=='OFFICIAL' and view.Envoys.Eligible==true,
    'actual official envoy route is not eligible')
  return metric
end
local function Snapshot()
  return {Turn=Game.GetCurrentGameTurn(),Product=Product(),
    Admission=R.Copy(Game:GetProperty('JAC_ABSORPTION_ADMISSION_V1') or {}),
    Flow=R.Copy(Game:GetProperty('JAC_ABSORPTION_FLOW_V1') or {}),
    Gold=Players[t.To]:GetTreasury():GetGoldBalance(),Faith=A.PlayerFaithBalance(t.To),
    Pool=Players[t.To]:GetInfluence():GetTokensToGive(),Received=Players[t.From]:GetInfluence():GetTokensReceived(t.To),
    LastOperation=R.Copy(Players[t.To]:GetProperty('JAC_ABSORPTION_PRODUCT_LAST_OPERATION_V1'))}
end
local function Unregistered()
  local s=Product()
  assert(not (s.Pairs or {})[t.From..':'..t.To],'AI registered before actual envoy requirements held')
  for _,op in pairs(s.Operations or {}) do
    assert(op.Identity.To~=t.To,'AI operated while normal non-deposit conditions were unmet')
  end
end
local function NativeAssets(stage)
  local o=R.Copy(assert(ExposedMembers.JAC_OfficialProductNativeAssets,'native UI asset observation absent'))
  local city=assert(Players[t.To]:GetCities():GetCapitalCity(),'actual recipient capital absent')
  assert(o.Turn==Game.GetCurrentGameTurn() and o.Owner==t.To and o.City==city:GetID() and o.X==city:GetX() and o.Y==city:GetY(),
    'native asset observation has wrong turn or actual city identity')
  assert(type(o.Serial)=='number' and (not t.NativePreparation or o.Serial>t.NativePreparation.Read),'fresh native asset observation required')
  assert(o.Plot and o.PlotOwner==t.To and o.PlotCityOwner==t.To and o.PlotCity==o.City,'native site belongs to another city')
  local plot=assert(Map.GetPlotByIndex(o.Plot),'actual site plot missing')
  assert(plot:GetOwner()==t.To,'actual site owner changed')
  assert(#o.HolySites<=1,'native capital has more than one Holy Site')
  local holy=o.HolySites[1]
  if holy then
    assert(holy.Owner==t.To and holy.City==o.City and holy.Plot==o.Plot and holy.Complete and not holy.Pillaged,
      'existing Holy Site is incomplete, pillaged or belongs to another city')
    assert(plot:GetDistrictType()==GameInfo.Districts.DISTRICT_HOLY_SITE.Index,'actual plot is not the observed Holy Site')
  else
    assert(stage=='PLAN' and plot:GetDistrictType()==-1 and plot:GetImprovementType()==-1 and plot:GetResourceType()==-1,
      'native construction site is no longer empty')
    assert(not plot:IsCity() and not plot:IsWater() and not plot:IsMountain() and not plot:IsImpassable() and not plot:IsNaturalWonder(),
      'native Holy Site cannot be placed on this plot')
  end
  if t.NativePreparation then assert(o.Plot==t.NativePreparation.Plot,'native construction moved to another plot') end
  if stage=='SHRINE' or stage=='TEMPLE' then
    local shrine=assert(o.BUILDING_SHRINE)
    assert(shrine.Present and shrine.AtSite and not shrine.Pillaged,'native Shrine is absent, pillaged or not in the exact Holy Site')
  end
  if stage=='TEMPLE' then
    local temple=assert(o.BUILDING_TEMPLE)
    assert(temple.Present and temple.AtSite and not temple.Pillaged,'native Temple is absent, pillaged or not in the exact Holy Site')
  end
  return o,city
end
local function EnvoyTurn(before,s)
  local count,key,operation=0,nil,nil
  for k,op in pairs(s.Operations or {}) do
    local old=(before.Product.Operations or {})[k]
    if old then assert(R.Equal(old,op),'settled operation changed on a later natural turn')
    elseif op.Identity.To==t.To and op.Identity.Command=='SEND_ENVOY' then
      count=count+1;key,operation=k,op
    end
  end
  assert(count<=1,'AI assigned more than one envoy per recipient/actual turn')
  local pool=Players[t.To]:GetInfluence():GetTokensToGive()
  local received=Players[t.From]:GetInfluence():GetTokensReceived(t.To)
  if operation then
    local op=operation;local identity=assert(op.EnvoyIdentity);local r=assert(op.EnvoyReceipt)
    assert(op.Done==true and identity.Route=='OFFICIAL' and identity.CountryID=='VATICAN_CITY'
      and identity.From==t.From and identity.To==t.To and op.Identity.From==t.From and op.Identity.To==t.To,
      'later AI assignment changed official route/recipient')
    local profile=t.PreparedView.Country.Profile;local payer=PlayerConfigurations[t.To]
    assert(identity.CivilizationType==profile.CivilizationType and identity.LeaderType==profile.LeaderType
      and identity.SenderCivilizationType==payer:GetCivilizationTypeName() and identity.SenderLeaderType==payer:GetLeaderTypeName(),
      'later AI assignment changed actual source/payer profile')
    assert(op.Identity.ExpectedEnvoys==before.Received and op.Nonce=='ai-envoy:'..t.From..':'..before.Received
      and key==t.To..':'..op.Nonce,'later assignment did not start at the previous observed native count')
    assert(r.Done==true and r.ChargeDone==true and r.GrantIntent==true and r.GrantDone==true,
      'later official assignment did not settle native charge/grant')
    assert(r.Before==before.Pool and r.After==before.Pool-1
      and r.ReceivedBefore==before.Received and r.ReceivedAfter==before.Received+1,
      'native counts changed before the product receipt; competing/native assignment is ambiguous')
    assert(pool==r.After and received==r.ReceivedAfter,
      'native counts changed after the product receipt; extra assignment cannot pass as one envoy')
    local budget=assert((s.AiEnvoyTurns or {})[tostring(t.To)])
    assert(budget.Turn==Game.GetCurrentGameTurn() and budget.Operation==key,'later assignment used a different actual-turn budget')
  else
    assert(pool==before.Pool and received==before.Received,
      'native envoy counts changed without a product operation; competing assignment cannot pass')
  end
  assert(not (s.Envoys or {})[t.From..':'..t.To],'official counts were replaced with a PRODUCT ledger')
  return count
end
local function SavedAdmission(offer,request)
  assert(request.Status=='STARTED' and request.Plan and request.Decision,'saved started admission evidence missing')
  local plan,decision=request.Plan,request.Decision
  assert(plan.ID==request.FlowID and plan.From==t.From and plan.To==t.To and plan.Mode=='SMALL_STATE'
    and plan.EntryConditionsMet==true and plan.RelationsResolved==true,'saved admission plan changed actors or gate result')
  assert(R.Equal(plan.LeaderProfile,t.PreparedView.Country.Profile) and R.Equal(plan.LeaderProfile,offer.Plan.LeaderProfile)
    and R.Equal(plan.ProductDeposit,offer.Plan.ProductDeposit),'saved admission plan changed normal source or deposit')
  assert(decision.From==t.From and decision.To==t.To and decision.Mode=='SMALL_STATE' and decision.AreaKm2==0.44
    and decision.RelationsResolved==true and decision.EnvoySource=='product' and decision.EnvoysEligible==true,
    'saved normal admission decision changed identity, relationships or envoy eligibility')
  assert(type(decision.Turn)=='number' and decision.Turn==request.StartTurn and decision.Turn<=Game.GetCurrentGameTurn(),
    'saved admission has an invalid actual acceptance turn')
  assert(type(decision.OwnedTiles)=='number' and decision.OwnedTiles>=0 and decision.OwnedTiles==math.floor(decision.OwnedTiles)
    and type(decision.Envoys)=='number' and decision.Envoys==math.floor(decision.Envoys)
    and decision.Envoys>=math.max(1,decision.OwnedTiles),'saved official envoys did not cover native territory at acceptance')
  local expected,seen={},{}
  for _,d in ipairs(offer.Requirements) do assert(not expected[d.ID],'duplicate ordinary demand');expected[d.ID]=true end
  assert(type(decision.Requirements)=='table' and #decision.Requirements==#offer.Requirements,'saved requirement results incomplete')
  for _,row in ipairs(decision.Requirements) do
    assert(expected[row.ID] and not seen[row.ID] and row.Met==true,'saved ordinary requirement did not pass: '..tostring(row.ID))
    seen[row.ID]=true
  end
  local view=P.Get(t.From,t.To);Profile(view)
  assert(view.Frozen==true and view.Status=='STARTED' and view.OfferID==offer.ID and view.Envoys.Route=='FROZEN'
    and view.Envoys.Envoys==decision.Envoys and view.Envoys.Required==math.max(1,decision.OwnedTiles),
    'frozen product view differs from its saved accepted plan/envoys')
  return decision
end
local function ObserveDebit(before,s,id)
  local escrow=assert((s.Escrows or {})[id],'actual deposit escrow absent')
  local offer=assert(N.GetOffer(id));local deposit=offer.ProductTerms.Deposit
  assert(escrow.ID==id and escrow.From==t.From and escrow.To==t.To and escrow.Gold==deposit.Gold and escrow.Faith==deposit.Faith,
    'actual deposit changed offer, payer or ordinary amounts')
  assert(escrow.Gold>0 and escrow.Faith>0 and (escrow.Status=='HELD' or escrow.Status=='CONSUMED'),'ordinary cautious deposit did not settle')
  local gold,faith=assert(escrow.CollectGold),assert(escrow.CollectFaith)
  assert(gold.Done==true and faith.Done==true and gold.Before-gold.After==escrow.Gold and faith.Before-faith.After==escrow.Faith,
    'normal currency receipt did not settle exact ordinary amounts')
  t.NativeDebits=t.NativeDebits or {}
  local proof=t.NativeDebits[id]
  if proof then
    assert(proof.From==t.From and proof.To==t.To and R.Equal(proof.Gold,gold) and R.Equal(proof.Faith,faith)
      and proof.ActualBefore.Gold==gold.Before and proof.ActualBefore.Faith==faith.Before
      and proof.ActualAfter.Gold==gold.After and proof.ActualAfter.Faith==faith.After,
      'saved actual-debit observations changed or did not cover both real currency legs')
    return proof
  end
  -- A new receipt must match the native getter *now*, not another journal.
  -- Later native income/spending is not retroactively attributed to this debit.
  local actual={Turn=Game.GetCurrentGameTurn(),Gold=Players[t.To]:GetTreasury():GetGoldBalance(),Faith=A.PlayerFaithBalance(t.To)}
  assert(actual.Gold==gold.After and actual.Faith==faith.After,
    'settled receipts do not match actual native balances; omitted debit or later competing currency change')
  local observations=assert(ExposedMembers.JAC_OfficialProductCurrencySnapshots,'independent native currency observations absent')
  assert(observations.Owner==t.To and not observations.Overflow,'native currency observation owner/history is invalid')
  local anchor,cursor
  if before.Gold==gold.Before and before.Faith==faith.Before then
    anchor={Turn=before.Turn,Gold=before.Gold,Faith=before.Faith};cursor=before.CurrencyCursor or 0
  end
  for index,row in ipairs(observations.Rows) do
    if row.Turn>=before.Turn and row.Turn<=actual.Turn and row.Gold==gold.Before and row.Faith==faith.Before then
      anchor=R.Copy(row);cursor=index
    end
  end
  assert(anchor,'receipt Before was not independently observed; native income cannot be guessed')
  for index=cursor+1,#observations.Rows do
    local row=observations.Rows[index]
    assert(row.Turn>=anchor.Turn and row.Turn<=actual.Turn and row.Gold==gold.After
      and (row.Faith==faith.Before or row.Faith==faith.After),'currency changes between observed Before/After are ambiguous')
  end
  assert(anchor.Gold-actual.Gold==escrow.Gold and anchor.Faith-actual.Faith==escrow.Faith,'actual native debit did not match normal deposit')
  proof={From=t.From,To=t.To,Gold=R.Copy(gold),Faith=R.Copy(faith),ActualBefore=anchor,ActualAfter=actual}
  t.NativeDebits[id]=proof
  print('[JAC_OFFICIAL_PRODUCT] actual-debit offer='..id..' gold='..anchor.Gold..'->'..actual.Gold..
    ' faith='..anchor.Faith..'->'..actual.Faith)
  return proof
end
local function Send(before,previous)
  assert(Game.GetCurrentGameTurn()==before.Turn+1,'send did not follow exactly one actual end_turn')
  local view=P.Get(t.From,t.To);NonDeposit(view)
  local s=Product();local key,operation,count=nil,nil,0
  for k,op in pairs(s.Operations or {}) do
    if op.Identity.To==t.To and not (before.Product.Operations or {})[k] then
      count=count+1;key,operation=k,op
    end
  end
  assert(count==1,'AI must perform exactly one operation for this recipient on this turn')
  local op=assert(operation);local id=assert(op.Identity);local identity=assert(op.EnvoyIdentity)
  assert(id.Command=='SEND_ENVOY' and id.From==t.From and id.To==t.To and id.ExpectedEnvoys==before.Received,
    'wrong AI envoy operation or expected actual received count')
  assert(identity.Route=='OFFICIAL' and identity.CountryID=='VATICAN_CITY' and identity.From==t.From and identity.To==t.To,
    'wrong saved official envoy identity')
  assert(identity.CivilizationType==view.Country.Profile.CivilizationType and identity.LeaderType==view.Country.Profile.LeaderType,
    'saved envoy source profile differs from the actual minor')
  local sender=PlayerConfigurations[t.To]
  assert(identity.SenderCivilizationType==sender:GetCivilizationTypeName() and identity.SenderLeaderType==sender:GetLeaderTypeName(),
    'saved envoy payer identity changed')
  assert(op.Nonce=='ai-envoy:'..t.From..':'..before.Received and key==t.To..':'..op.Nonce,'not the ordinary AI operation nonce')
  local receipt=assert(op.EnvoyReceipt,'saved envoy receipt missing')
  assert(op.Done==true and receipt.Done==true and receipt.ChargeDone==true and receipt.GrantIntent==true and receipt.GrantDone==true,
    'native envoy operation was not fully charged and granted')
  assert(receipt.Before==before.Pool and receipt.After==before.Pool-1,
    'receipt did not consume exactly one actual unassigned token')
  assert(receipt.ReceivedBefore==before.Received and receipt.ReceivedAfter==before.Received+1,
    'receipt did not grant exactly one actual official received token')
  assert(Players[t.To]:GetInfluence():GetTokensToGive()==receipt.After,'actual payer pool differs from settled receipt')
  assert(Players[t.From]:GetInfluence():GetTokensReceived(t.To)==receipt.ReceivedAfter,
    'actual official recipient count differs from settled receipt')
  local budget=assert((s.AiEnvoyTurns or {})[tostring(t.To)],'AI turn journal missing')
  assert(budget.Turn==Game.GetCurrentGameTurn() and budget.Operation==key,'AI budget was not claimed for this actual turn')
  assert(not (s.Envoys or {})[t.From..':'..t.To],'official received count was replaced by a PRODUCT ledger')
  if previous then
    assert(key~=previous.Key and R.Equal(s.Operations[previous.Key],previous.Operation),
      'next turn replayed or changed the earlier settled receipt')
  end
  print('[JAC_OFFICIAL_PRODUCT] actual-turn='..budget.Turn..' operation='..key..' route='..identity.Route..
    ' pool='..receipt.Before..'->'..receipt.After..' received='..receipt.ReceivedBefore..'->'..receipt.ReceivedAfter)
  return {Key=key,Operation=R.Copy(op),Turn=budget.Turn}
end
local function Unchanged(expected,label)
  assert(Game.GetCurrentGameTurn()==expected.Turn,label..': actual turn changed')
  assert(R.Equal(Snapshot(),expected),label..': payer, receipt, budget, admission or flow changed')
  -- Replay the ordinary read-only API in Core as well as in UI publications.
  for i=1,3 do
    Profile(P.Get(t.From,t.To))
    assert(R.Equal(Snapshot(),expected),label..': repeated product read mutated Core state')
  end
  print('[JAC_OFFICIAL_PRODUCT] '..label..' actual-turn='..expected.Turn..' payer-core-state=unchanged')
end
"#;

const SELECT_AND_BLOCK: &str = r#"
assert(not t,'fresh acceptance game required')
assert(not Game:GetProperty('JAC_ABSORPTION_FLOW_TEST_MODE'),'diagnostic flow mode must be absent')
local from,to,human,majors,minors=nil,nil,nil,0,0
for _,p in ipairs(PlayerManager.GetAliveMinors()) do
  minors=minors+1
  local id=p:GetID();local config=PlayerConfigurations[id]
  assert(config:GetCivilizationTypeName()=='CIVILIZATION_VATICAN_CITY' and config:GetLeaderTypeName()=='LEADER_MINOR_CIV_VATICAN_CITY',
    'the configured sole official minor is not Vatican City')
  assert(not p:IsHuman() and not p:IsMajor() and p:GetInfluence():CanReceiveInfluence(),'actual official minor cannot receive influence')
  from=id
end
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
  majors=majors+1
  local id=p:GetID();local config=PlayerConfigurations[id]
  if config:GetCivilizationTypeName()=='CIVILIZATION_JAC_TAKEDA' then
    assert(not p:IsHuman() and config:GetLeaderTypeName()=='LEADER_JAC_TAKEDA_NOBUTORA','ordinary AI Takeda recipient required');to=id
  elseif config:GetCivilizationTypeName()=='CIVILIZATION_JAC_ODA' then
    assert(p:IsHuman() and id==Game.GetLocalPlayer() and config:GetLeaderTypeName()=='LEADER_JAC_ODA_NOBUHIDE',
      'ordinary human Oda turn driver required');human=id
  else error('unexpected major in small official-map fixture') end
end
assert(minors==1 and majors==2 and from and to and human,'exactly Vatican City, AI Takeda and human Oda required')
t={From=from,To=to,Human=human,StartTurn=Game.GetCurrentGameTurn()}
local c=assert(P.Country(from));Profile({Status='AVAILABLE',Country=c,Mode='SMALL_STATE'})
Players[to]:GetDiplomacy():SetHasMet(from);Players[from]:GetDiplomacy():SetHasMet(to)
local treasury=Players[to]:GetTreasury();treasury:ChangeGoldBalance(-treasury:GetGoldBalance())
local influence=Players[to]:GetInfluence();influence:ChangeTokensToGive(-influence:GetTokensToGive())
Unregistered()
t.Initial=Snapshot();Game:SetProperty(KEY,t)
return 'actual configured official source and ordinary major leaders verified; native funds and pool depleted'
"#;

const ASSERT_UNMET: &str = r#"
assert(t and Game.GetCurrentGameTurn()==t.StartTurn+1,'one actual founding turn required')
assert(Players[t.From]:GetCities():GetCapitalCity() and Players[t.To]:GetCities():GetCapitalCity(),'native minor/recipient capitals not founded')
local v=P.Get(t.From,t.To);Profile(v)
assert(v.Status=='AVAILABLE' and not v.OfferID,'unmet ordinary AI offer was registered')
assert(Players[t.To]:GetTreasury():GetGoldBalance()<v.Deposit.Gold,'native deposit is not actually unmet')
assert(v.Envoys and v.Envoys.Route=='OFFICIAL' and v.Envoys.Envoys<v.Envoys.Required,'actual official envoy requirement already met')
Unregistered()
t.Unmet=R.Copy(v);t.UnmetSnapshot=Snapshot();Game:SetProperty(KEY,t)
return 'real end_turn left unmet normal AI terms unregistered'
"#;

const PREPARE: &str = r#"
local v=P.Get(t.From,t.To);Profile(v)
local metric
for _,row in ipairs(v.Rows) do if row.Kind=='PRODUCT_METRIC' then
  assert(not metric and row.ID=='leader:FAITH_YIELD','ordinary Vatican City faith yield demand required');metric=row
end end
assert(metric and type(metric.Minimum)=='number' and metric.Minimum>0,'ordinary native faith yield minimum absent')
assert(metric.Minimum<=6,'ordinary faith yield requirement exceeds the two installed native buildings')
local assets,city=NativeAssets('PLAN')
local gold=v.Deposit.Gold+10000;local faith=v.Deposit.Faith+10000
Players[t.To]:GetTreasury():ChangeGoldBalance(gold-Players[t.To]:GetTreasury():GetGoldBalance())
Players[t.To]:GetReligion():ChangeFaithBalance(faith-A.PlayerFaithBalance(t.To))
local influence=Players[t.To]:GetInfluence()
local pool=math.max(32,v.Envoys.Required+4)
influence:ChangeTokensToGive(pool-influence:GetTokensToGive())
assert(influence:GetTokensToGive()==pool,'actual unassigned pool was not prepared')
Unregistered()
t.NativePreparation={City=city:GetID(),Plot=assets.Plot,Read=assets.Serial,Pool=pool,Gold=gold,Faith=faith,Stage='HOLY'}
Game:SetProperty(KEY,t)
if #assets.HolySites==0 then
  city:GetBuildQueue():CreateIncompleteDistrict(GameInfo.Districts.DISTRICT_HOLY_SITE.Index,assets.Plot,100)
end
return 'native Holy Site requested; balances and actual unassigned pool supplied without changing normal leader terms'
"#;

const PREPARE_SHRINE: &str = r#"
assert(t.NativePreparation.Stage=='HOLY','native Holy Site was not requested')
local assets,city=NativeAssets('HOLY')
local building=assert(assets.BUILDING_SHRINE)
assert(not building.Present or building.AtSite and not building.Pillaged,'existing Shrine is incomplete, pillaged or in another site')
t.NativePreparation.Read=assets.Serial;t.NativePreparation.Stage='SHRINE';Game:SetProperty(KEY,t)
if not building.Present then city:GetBuildQueue():CreateIncompleteBuilding(GameInfo.Buildings.BUILDING_SHRINE.Index,assets.Plot,100) end
return 'native Holy Site completed on exact recipient plot; ordinary Shrine requested'
"#;

const PREPARE_TEMPLE: &str = r#"
assert(t.NativePreparation.Stage=='SHRINE','native Shrine was not requested')
local assets,city=NativeAssets('SHRINE')
local building=assert(assets.BUILDING_TEMPLE)
assert(not building.Present or building.AtSite and not building.Pillaged,'existing Temple is incomplete, pillaged or in another site')
t.NativePreparation.Read=assets.Serial;t.NativePreparation.Stage='TEMPLE';Game:SetProperty(KEY,t)
if not building.Present then city:GetBuildQueue():CreateIncompleteBuilding(GameInfo.Buildings.BUILDING_TEMPLE.Index,assets.Plot,100) end
return 'native Shrine completed in exact Holy Site; ordinary Temple requested'
"#;

const READY: &str = r#"
assert(t.NativePreparation.Stage=='TEMPLE','native Temple was not requested')
local assets=NativeAssets('TEMPLE')
local v=P.Get(t.From,t.To);local metric=NonDeposit(v)
assert(Players[t.To]:GetDiplomacy():HasMet(t.From) and Players[t.From]:GetDiplomacy():HasMet(t.To),'native bilateral contact missing')
assert(Players[t.To]:GetTreasury():GetGoldBalance()>=v.Deposit.Gold and A.PlayerFaithBalance(t.To)>=v.Deposit.Faith,
  'actual currencies cannot fund ordinary deposit')
-- Preserve the official received count. A meaningful two-turn test must
-- have at least three envoys still required, rather than freeze absorption.
assert(v.Envoys.Envoys+2<v.Envoys.Required,'native territory does not leave room for two waiting AI assignments')
assert(Players[t.To]:GetInfluence():GetTokensToGive()>=v.Envoys.Required+2,'native pool too small for ordinary qualification')
Unregistered()
t.PreparedView=R.Copy(v);t.NativeAssets=assets;t.NativeFaithYield=metric.Current;t.BeforeFirst=Snapshot();Game:SetProperty(KEY,t)
return 'every live non-deposit condition matches; actual OFFICIAL eligibility verified before natural AI turn'
"#;

const FIRST_SEND: &str = r#"
t.First=Send(assert(t.BeforeFirst))
assert(not (Product().Pairs or {})[t.From..':'..t.To],'AI registration preceded the actual envoy threshold')
t.SettledFirst=Snapshot();Game:SetProperty(KEY,t)
return 'ordinary AI turn charged one real pool token and granted one official received token; saved receipt and actual-turn budget verified'
"#;

const PUBLICATION_REPLAY: &str = r#"
-- Observe actual UI publications. Do not invoke a turn handler or synthesize
-- a publication. Only the product's existing read-only Get crosses contexts.
local api=ExposedMembers.JAC_AbsorptionProduct
local observation={From=from,To=to,Turn=Game.GetCurrentGameTurn(),Count=0}
ExposedMembers.JAC_OfficialProductPublicationObservation=observation
local onPublish
onPublish=function()
  if Game.GetCurrentGameTurn()~=observation.Turn then return end
  local v=api.Get(from,to)
  if v and v.Status~='UNAVAILABLE' then observation.Count=observation.Count+1 end
end
JAC_OfficialProductPublicationCallback=onPublish
Events.GameCoreEventPublishComplete.Add(onPublish)
return 'read-only replay subscribed to real same-turn GameCore publications'
"#;

const ASSERT_PUBLICATIONS: &str = r#"
local o=assert(ExposedMembers.JAC_OfficialProductPublicationObservation,'actual UI publication observations missing')
assert(o.From==t.From and o.To==t.To and o.Turn==t.SettledFirst.Turn and type(o.Count)=='number' and o.Count>=2,
  'at least two actual same-turn read-only publication replays required')
Unchanged(t.SettledFirst,'same-turn publication replay')
t.PublicationReplays=o.Count;Game:SetProperty(KEY,t)
return 'actual same-turn notifications replayed public Get; full payer Core state unchanged'
"#;

const AFTER_LOAD: &str = r#"
Unchanged(assert(t.SettledFirst),'same-name save/exit/load')
assert(t.PublicationReplays>=2,'actual same-turn publication replay evidence was not saved')
t.BeforeSecond=Snapshot();Game:SetProperty(KEY,t)
return 'same actual turn, official native counts, saved operation receipt and AI budget restored unchanged'
"#;

const SECOND_SEND: &str = r#"
t.Second=Send(assert(t.BeforeSecond),assert(t.First))
assert(t.Second.Turn==t.First.Turn+1,'AI did not resume on the next actual turn')
assert(not (Product().Pairs or {})[t.From..':'..t.To],'AI registered before actual envoy qualification')
t.SettledSecond=Snapshot();t.BeforeQualification=Snapshot()
t.BeforeQualification.CurrencyCursor=#assert(ExposedMembers.JAC_OfficialProductCurrencySnapshots).Rows
Game:SetProperty(KEY,t)
return 'next actual end_turn sent exactly one more official envoy; previous receipt unchanged and turn budget renewed'
"#;

const CURRENCY_WATCH: &str = r#"
-- Detached native observations only. This observer never calls commands or
-- replaces any product reader; it cannot supply a ready/deposit condition.
if JAC_OfficialProductCurrencyCallback then Events.GameCoreEventPublishComplete.Remove(JAC_OfficialProductCurrencyCallback) end
local observations={Owner=to,Rows={}}
ExposedMembers.JAC_OfficialProductCurrencySnapshots=observations
local function Sample()
  local turn=Game.GetCurrentGameTurn()
  local row={Turn=turn,Gold=Players[to]:GetTreasury():GetGoldBalance(),Faith=Players[to]:GetReligion():GetFaithBalance()}
  local last=observations.Rows[#observations.Rows]
  if last and last.Turn==row.Turn and last.Gold==row.Gold and last.Faith==row.Faith then return end
  if #observations.Rows>=256 then observations.Overflow=true;return end
  observations.Rows[#observations.Rows+1]=row
end
Sample()
JAC_OfficialProductCurrencyCallback=Sample
Events.GameCoreEventPublishComplete.Add(Sample)
return 'independent native currency snapshots subscribed before natural qualification turns'
"#;

// Inspect every later natural turn, including the first AI registration. The
// bounded continuation also proves this fixture can qualify a normal offer;
// two successful low-level assignments alone cannot satisfy the scene.
const QUALIFICATION_TURN: &str = r#"
local before=assert(t.BeforeQualification)
assert(Game.GetCurrentGameTurn()==before.Turn+1,'qualification skipped an actual turn')
local s=Product();EnvoyTurn(before,s)
assert(R.Equal(s.Operations[t.First.Key],t.First.Operation) and R.Equal(s.Operations[t.Second.Key],t.Second.Operation),
  'qualification changed the two saved official receipts')
local pair=(s.Pairs or {})[t.From..':'..t.To]
if pair and not t.Registration then
  local offer=assert(N.GetOffer(pair.ID));local request=assert(N.Get('leader-offer',pair.ID))
  assert(pair.Sequence==1 and not pair.Cancelled,'ordinary AI offer was replaced or cancelled')
  assert(offer.From==t.From and offer.To==t.To and offer.AreaKm2==0.44,'ordinary AI offer identity changed')
  assert(R.Equal(offer.ProductTerms.Input.Country.Profile,t.PreparedView.Country.Profile),'ordinary AI source profile was replaced')
  local register,minimum=nil,nil
  for _,op in pairs(s.Operations) do if op.Identity.Command=='REGISTER' and op.Identity.To==t.To then
    assert(not register and op.Done and op.Identity.From==t.From and op.Nonce=='ai-register:'..t.From,'not one ordinary AI registration')
    register=op
  end end
  assert(register and register.OfferIntent.Offer.ID==pair.ID,'actual AI registration intent missing')
  local actual,tiles,accepted
  if request.Status=='STARTED' then
    -- A normal current-turn publication can already collect and admit.
    -- Inspect the frozen authoritative decision, never demand a live route.
    accepted=SavedAdmission(offer,request);actual,tiles=accepted.Envoys,accepted.OwnedTiles
  else
    assert(request.Status=='WAITING','ordinary admission registration failed')
    NonDeposit(P.Get(t.From,t.To))
    actual=Players[t.From]:GetInfluence():GetTokensReceived(t.To);tiles=0
    for i=0,Map.GetPlotCount()-1 do if Map.GetPlotByIndex(i):GetOwner()==t.From then tiles=tiles+1 end end
  end
  for _,d in ipairs(offer.Requirements) do if d.Kind=='PRODUCT_METRIC' then
    assert(d.Metric=='FAITH_YIELD' and not minimum,'normal faith yield demand changed');minimum=d.Minimum
  end end
  assert(minimum and (accepted or FaithYield(t.To)>=minimum),'actual faith yield did not qualify the registered ordinary offer')
  assert(actual>=math.max(1,tiles),'AI registered before actual official received envoys covered native territory')
  t.Registration={Turn=Game.GetCurrentGameTurn(),ID=pair.ID,Operation=R.Copy(register),
    NativeFaithYield=not accepted and FaithYield(t.To) or nil,Decision=R.Copy(accepted),Received=actual,OwnedTiles=tiles}
end
if pair and (s.Escrows or {})[pair.ID] then ObserveDebit(before,s,pair.ID) end
t.BeforeQualification=Snapshot()
t.BeforeQualification.CurrencyCursor=#assert(ExposedMembers.JAC_OfficialProductCurrencySnapshots).Rows
Game:SetProperty(KEY,t)
return 'natural qualification turn inspected: at most one official envoy per recipient and real non-deposit prerequisites for AI registration'
"#;

const QUALIFIED: &str = r#"
assert(t.Registration,'bounded real turns never registered a qualified ordinary AI offer')
local s=Product();local pair=assert(s.Pairs[t.From..':'..t.To]);local request=assert(N.Get('leader-offer',pair.ID))
assert(request.Status=='STARTED' and request.Plan and request.Plan.Mode=='SMALL_STATE',request.LastError or 'ordinary official-minor admission never started')
SavedAdmission(assert(N.GetOffer(pair.ID)),request)
assert(t.NativeDebits and t.NativeDebits[pair.ID],'normal currency receipt lacks independently verified actual debit')
ObserveDebit(assert(t.BeforeQualification),s,pair.ID)
local flow=assert(JAC_AbsorptionFlowGameplay.Get(request.FlowID))
assert(flow.Phase1Complete,'normal three-stage absorption did not reach its real first-stage result')
t.OfferID=pair.ID;t.FlowID=request.FlowID;t.Qualified=Snapshot();Game:SetProperty(KEY,t)
print('[JAC_OFFICIAL_PRODUCT] qualified offer='..pair.ID..' flow='..request.FlowID..' actual-turn='..t.Qualified.Turn)
return 'ordinary AI registration, official native envoy qualification, real deposit and normal admission verified'
"#;

fn core(code: &str) -> Value {
    json!({"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":format!("{CORE}\n{code}")})
}

fn ui(code: &str) -> Value {
    json!({"action":"ui_lua","state_name":"InGame","code":format!("{UI_ACTORS}\n{code}")})
}

fn wait(code: &str) -> Value {
    json!({"action":"wait_for","timeout":120,"condition":{
        "type":"lua","state_name":"InGame","code":format!("{UI_ACTORS}\n{code}")}})
}

fn active() -> Value {
    wait("local p=Players[Game.GetLocalPlayer()];return from~=nil and to~=nil and p and p:IsTurnActive() and ExposedMembers.JAC_AbsorptionProduct~=nil and ExposedMembers.JAC_AbsorptionProductNativeRead~=nil and ExposedMembers.JAC_HeroReadAPI and type(ExposedMembers.JAC_HeroReadAPI.ReadPlayer)=='function'")
}

fn native_assets(stage: &str) -> Value {
    let ready = match stage {
        "PLAN" => "return assets~=nil and assets.Plot~=nil",
        "HOLY" => "return assets and #assets.HolySites==1 and assets.HolySites[1].Complete and not assets.HolySites[1].Pillaged",
        "SHRINE" => "local b=assets and assets.BUILDING_SHRINE;return b and b.Present and b.AtSite and not b.Pillaged",
        "TEMPLE" => "local b=assets and assets.BUILDING_TEMPLE;return b and b.Present and b.AtSite and not b.Pillaged",
        _ => unreachable!("fixed native construction stage"),
    };
    wait(&format!("{UI_NATIVE_ASSETS}\n{ready}"))
}

fn advance(steps: &mut Vec<Value>) {
    steps.push(json!({"action":"end_turn","turns":1,"timeout":600}));
    steps.push(active());
}

fn round_trip(steps: &mut Vec<Value>, name: &str) {
    steps.extend([
        json!({"action":"save","name":name,"timeout":90}),
        json!({"action":"exit_to_menu","timeout":90}),
        json!({"action":"load","name":name,"timeout":600}),
        active(),
    ]);
}

fn scenario(repo: &Repository) -> Result<Value> {
    let base: Value = serde_json::from_str(&absorption_admission::envoy_probe_scenario(repo)?)?;
    let original = base["steps"].as_array().context("official probe steps")?;
    ensure!(original.len() >= 6, "official startup recipe is incomplete");
    let mut steps = original[..6].to_vec();
    ensure!(
        steps[2]["state_name"] == "FrontEnd",
        "official startup changed"
    );
    // The inherited setup ends with return; append the filter *before* it.
    let setup = steps[2]["code"]
        .as_str()
        .context("official map setup Lua")?;
    let (setup, _) = setup
        .rsplit_once("return ")
        .context("official setup return")?;
    steps[2]["code"] = json!(format!("{setup}\n{FRONTEND_FILTER}"));
    steps.push(active());
    steps.push(core(SELECT_AND_BLOCK));
    advance(&mut steps);
    steps.push(core(ASSERT_UNMET));
    steps.push(native_assets("PLAN"));
    steps.push(core(PREPARE));
    steps.push(native_assets("HOLY"));
    steps.push(core(PREPARE_SHRINE));
    steps.push(native_assets("SHRINE"));
    steps.push(core(PREPARE_TEMPLE));
    steps.push(native_assets("TEMPLE"));
    steps.push(wait(r#"
if not from or not to or not ExposedMembers.JAC_AbsorptionProduct then return false end
local v=ExposedMembers.JAC_AbsorptionProduct.Get(from,to)
if not v or v.Status=='UNAVAILABLE' or not v.Envoys or not v.Envoys.Eligible then return false end
for _,row in ipairs(v.Rows or {}) do if row.Kind~='PRODUCT_DEPOSIT' and row.Met~=true then return false end end
return v.Status=='AVAILABLE'
"#));
    steps.push(core(READY));
    steps.push(ui("ExposedMembers.JAC_OfficialProductNativeAssets=nil;return 'completed native construction observations released'"));
    advance(&mut steps);
    steps.push(core(FIRST_SEND));
    steps.push(ui(PUBLICATION_REPLAY));
    steps.push(wait("local o=ExposedMembers.JAC_OfficialProductPublicationObservation;return o and o.Count>=2 and o.Turn==Game.GetCurrentGameTurn()"));
    steps.push(core(ASSERT_PUBLICATIONS));
    steps.push(ui("Events.GameCoreEventPublishComplete.Remove(JAC_OfficialProductPublicationCallback);JAC_OfficialProductPublicationCallback=nil;ExposedMembers.JAC_OfficialProductPublicationObservation=nil;return 'same-turn publication subscription removed'"));
    round_trip(&mut steps, "JAC-official-product-ai-receipt");
    steps.push(core(AFTER_LOAD));
    advance(&mut steps);
    steps.push(ui(CURRENCY_WATCH));
    steps.push(core(SECOND_SEND));
    for _ in 0..10 {
        advance(&mut steps);
        steps.push(core(QUALIFICATION_TURN));
    }
    steps.push(wait("if not from or not to or not ExposedMembers.JAC_AbsorptionProduct then return false end;local v=ExposedMembers.JAC_AbsorptionProduct.Get(from,to);return v and v.Status=='STARTED'"));
    steps.push(core(QUALIFIED));
    steps.push(ui("Events.GameCoreEventPublishComplete.Remove(JAC_OfficialProductCurrencyCallback);JAC_OfficialProductCurrencyCallback=nil;ExposedMembers.JAC_OfficialProductCurrencySnapshots=nil;return 'native currency snapshot observer removed after verified debit'"));
    round_trip(&mut steps, "JAC-official-product-ai-qualified");
    steps.push(core("Unchanged(assert(t.Qualified),'qualified same-name save/exit/load');return 'qualified product receipt, actual payer and normal admission restored unchanged'"));
    Ok(json!({
        "name":"Official Vatican City AI product route: native qualification, exact pool/grant receipts, publication replay and same-name save/load",
        "judge_profile":"generic",
        "steps":steps
    }))
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    output: PathBuf,
    check: bool,
}

fn options(args: &[String]) -> Result<Option<Options>> {
    if args == ["--help"] || args == ["-h"] {
        return Ok(None);
    }
    let mut output = None;
    let mut check = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--output" => {
                ensure!(output.is_none(), "--output specified twice; {USAGE}");
                output = Some(PathBuf::from(
                    iter.next().context("--output requires a path")?,
                ));
            }
            "--check" => {
                ensure!(!check, "--check specified twice; {USAGE}");
                check = true;
            }
            _ => bail!("unknown argument {arg}; {USAGE}"),
        }
    }
    let output = output.context(USAGE)?;
    ensure!(output.is_absolute(), "--output must be an absolute path");
    ensure!(
        output.extension().is_some_and(|ext| ext == "json"),
        "--output must end in .json"
    );
    Ok(Some(Options { output, check }))
}

fn emit(path: &Path, expected: &str, check: bool) -> Result<()> {
    if check {
        let actual =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        ensure!(
            actual == expected,
            "generated official product scene differs: {}",
            path.display()
        );
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        std::fs::write(path, expected).with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let Some(options) = options(&args)? else {
        println!("{USAGE}\nGenerate an additional actual-game scene; --check compares without writing. Actual Civ6 acceptance must be run separately.");
        return Ok(());
    };
    let expected = format!(
        "{}\n",
        serde_json::to_string_pretty(&scenario(&Repository::checked_in())?)?
    );
    emit(&options.output, &expected, options.check)?;
    println!(
        "{} {}",
        if options.check {
            "checked"
        } else {
            "generated"
        },
        options.output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use civ6_data_rs::absorption_product;
    use mlua::Lua;

    // Independent evidence fixtures test the emitted *assertions*. Deliberate
    // corruptions must fail, including valid receipts with wrong native counts.
    const WORLD: &str = r#"
local function copy(v) if type(v)~='table' then return v end;local out={};for k,x in pairs(v) do out[k]=copy(x) end;return out end
local function equal(a,b)
  if type(a)~=type(b) then return false end;if type(a)~='table' then return a==b end
  for k,v in pairs(a) do if not equal(v,b[k]) then return false end end
  for k in pairs(b) do if a[k]==nil then return false end end;return true
end
TURN,POOL,RECEIVED,GOLD,FAITH,NATIVE_FAITH_YIELD=3,31,1,10100,10000,16
local profile={CivilizationType='CIVILIZATION_VATICAN_CITY',LeaderType='LEADER_MINOR_CIV_VATICAN_CITY',Personality='慎重',Trait='霊神'}
VIEW={Status='AVAILABLE',From=2,To=0,Mode='SMALL_STATE',Country={ID='VATICAN_CITY',AreaKm2=0.44,Profile=profile},
  Deposit={Gold=100,Faith=0},Envoys={Route='OFFICIAL',Eligible=true,Envoys=1,Required=7},
  Rows={{Kind='PRODUCT_METRIC',ID='leader:FAITH_YIELD',Current=16,Minimum=5,Met=true},
    {Kind='PRODUCT_IDENTITY',ID='identity',Met=true},{Kind='PRODUCT_CONTEXT',ID='terms',Met=true},
    {Kind='PRODUCT_DEPOSIT',ID='deposit',Met=false},{ID='relations',Met=true,Reviewed=true}}}
OPKEY='0:ai-envoy:2:0'
OP={Identity={Command='SEND_ENVOY',From=2,To=0,ExpectedEnvoys=0},Nonce='ai-envoy:2:0',Done=true,
  EnvoyIdentity={Route='OFFICIAL',From=2,To=0,CountryID='VATICAN_CITY',CivilizationType=profile.CivilizationType,LeaderType=profile.LeaderType,
    SenderCivilizationType='CIVILIZATION_JAC_TAKEDA',SenderLeaderType='LEADER_JAC_TAKEDA_NOBUTORA'},
  EnvoyReceipt={Before=32,After=31,ReceivedBefore=0,ReceivedAfter=1,ChargeDone=true,GrantIntent=true,GrantDone=true,Done=true}}
STATE={Version=2,Pairs={},Escrows={},Envoys={},Operations={[OPKEY]=OP},AiEnvoyTurns={['0']={Turn=3,Operation=OPKEY}}}
TEST={From=2,To=0,Human=1,BeforeFirst={Turn=2,Pool=32,Received=0,Product={Operations={}}}}
PROPS={JAC_OFFICIAL_PRODUCT_ACCEPTANCE_V1=TEST,JAC_ABSORPTION_PRODUCT_V1=STATE}
Game={GetCurrentGameTurn=function() return TURN end,GetProperty=function(_,k) return copy(PROPS[k]) end,SetProperty=function(_,k,v) PROPS[k]=copy(v) end}
local influence={GetTokensToGive=function() return POOL end,GetTokensReceived=function(_,sender) assert(sender==0);return RECEIVED end}
Players={}
for actor=0,2 do Players[actor]={
 GetTreasury=function() return {GetGoldBalance=function() return GOLD end} end,
 GetReligion=function() return {GetFaithBalance=function() return FAITH end,GetFaithYield=function() return NATIVE_FAITH_YIELD end} end,
 GetInfluence=function() return influence end,GetProperty=function() return nil end} end
PlayerConfigurations={[0]={GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_TAKEDA' end,GetLeaderTypeName=function() return 'LEADER_JAC_TAKEDA_NOBUTORA' end},
 [2]={GetCivilizationTypeName=function() return profile.CivilizationType end,GetLeaderTypeName=function() return profile.LeaderType end}}
JAC_AbsorptionProductGameplay={Get=function() return copy(VIEW) end}
JAC_AbsorptionAdmissionGameplay={}
JAC_AbsorptionGameplay={Rules={Copy=copy,Equal=equal},PlayerFaithBalance=function() return FAITH end}
"#;

    fn run(mutation: &str, code: &str) -> Result<mlua::Result<()>> {
        let lua = Lua::new();
        lua.load(WORLD).exec()?;
        lua.load(mutation).exec()?;
        Ok(lua.load(format!("{CORE}\n{code}")).exec())
    }

    #[test]
    fn official_startup_is_inherited_without_diagnostic_tail() -> Result<()> {
        let repo = Repository::checked_in();
        let scene = scenario(&repo)?;
        let base: Value =
            serde_json::from_str(&absorption_admission::envoy_probe_scenario(&repo)?)?;
        for index in [0, 1, 3, 4, 5] {
            assert_eq!(
                scene["steps"][index], base["steps"][index],
                "startup {index}"
            );
        }
        let config = &scene["steps"][1];
        assert_eq!(config["ruleset"], "RULESET_JAC_BANSEIKI");
        assert_eq!(config["map_size"], "MAPSIZE_DUEL");
        assert_eq!(config["city_states"], 1);
        assert!(config.get("map_file").is_none());
        assert_eq!(config["players"].as_array().unwrap().len(), 2);
        assert_eq!(config["players"][0]["human"], false);
        assert_eq!(config["players"][1]["human"], true);
        assert!(scene["steps"][5]["code"].as_str().unwrap().contains("1534"));
        Ok(())
    }

    #[test]
    fn emitted_lua_compiles_in_lua51_and_has_no_mutation_shortcuts() -> Result<()> {
        let scene = scenario(&Repository::checked_in())?;
        let lua = Lua::new();
        for (index, step) in scene["steps"].as_array().unwrap().iter().enumerate() {
            let code = step.get("code").or_else(|| step["condition"].get("code"));
            if let Some(code) = code.and_then(Value::as_str) {
                lua.load(code)
                    .set_name(format!("official-scene-step-{index}"))
                    .into_function()?;
                for forbidden in [
                    "P.Tick(",
                    "P.Command(",
                    "N.Request(",
                    "F.Begin(",
                    "GiveFreeTokenToPlayer(",
                    "OnGameTurnStarted.Call",
                    "JAC_AbsorptionProduct_Operation.Call",
                    "DefineOffer(",
                    "RegisterRequirement(",
                    "RegisterEnvoys(",
                    "TestCall",
                    "test_call",
                    "JAC_COUNTRY_RELATIONS_V1",
                    "SetLeaderType",
                    "SetCivilizationType",
                ] {
                    assert!(!code.contains(forbidden), "step {index}: {forbidden}");
                }
                if step["state_name"] == "InGame" || step["condition"]["state_name"] == "InGame" {
                    assert!(
                        !code.contains("Game:GetProperty"),
                        "UI reads Core property at {index}"
                    );
                    // The inherited start-calendar step checks official UI
                    // calendar output. All added acceptance evidence is Core.
                    if index >= 6 {
                        assert!(
                            !code.contains("assert("),
                            "UI asserts acceptance evidence at {index}"
                        );
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn exclusion_uses_official_rows_and_rejects_missing_target_or_bad_storage() -> Result<()> {
        for mutation in [
            "",
            "ROWS[2].CivilizationType='CIVILIZATION_UNREGISTERED'",
            "BAD_STORE=true",
            "ROWS[3]=ROWS[1]",
        ] {
            let lua = Lua::new();
            lua.load(r#"
ROWS={{CivilizationType='CIVILIZATION_GENEVA'},{CivilizationType='CIVILIZATION_VATICAN_CITY'},{CivilizationType='CIVILIZATION_SINGAPORE'}}
local values={CITY_STATE_COUNT=1}
DB={ConfigurationQuery=function(query)
 assert(query=="SELECT CivilizationType FROM CityStates WHERE Domain = 'Expansion2CityStates' ORDER BY CivilizationType");return ROWS end}
GameConfiguration={SetValue=function(k,v) assert(k=='EXCLUDE_CITY_STATES');values[k]=BAD_STORE and {} or v end,GetValue=function(k) return values[k] end}
MapConfiguration={GetValue=function(k) assert(k=='MAP_SCRIPT');return 'Continents.lua' end}
"#).exec()?;
            lua.load(mutation).exec()?;
            let result = lua.load(FRONTEND_FILTER).exec();
            assert_eq!(
                result.is_ok(),
                mutation.is_empty(),
                "{mutation}: {result:?}"
            );
            if result.is_ok() {
                lua.load("local v=GameConfiguration.GetValue('EXCLUDE_CITY_STATES');assert(#v==2 and v[1]=='CIVILIZATION_GENEVA' and v[2]=='CIVILIZATION_SINGAPORE')").exec()?;
            }
        }
        Ok(())
    }

    #[test]
    fn live_non_deposit_check_rejects_cosmetic_readiness_and_wrong_native_yields() -> Result<()> {
        for mutation in [
            "",
            "NATIVE_FAITH_YIELD=4",
            "VIEW.Rows[1].Current=100",
            "VIEW.Rows[2].Met=false",
            "VIEW.Rows[3].Met=false",
            "VIEW.Rows[5].Met=false",
            "VIEW.Rows[6]={Kind='GOLD_MINIMUM',ID='reserve:gold',Current=10100,Minimum=10050,Met=true}",
            "VIEW.Envoys.Eligible=false",
            "VIEW.Envoys.Route='PRODUCT'",
            "VIEW.Frozen=true",
            "VIEW.Country.Profile.Personality='温厚'",
            "VIEW.Country.Profile.Trait='凡人'",
            "VIEW.Country.AreaKm2=728",
            "VIEW.Rows[1].ID='leader:FOOD_YIELD'",
        ] {
            let result = run(mutation, "NonDeposit(P.Get(t.From,t.To))")?;
            assert_eq!(
                result.is_ok(),
                mutation.is_empty(),
                "{mutation}: {result:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn native_preparation_requires_fresh_completed_assets_in_the_actual_capital() -> Result<()> {
        for mutation in [
            "",
            "ASSETS.Serial=2",
            "ASSETS.Turn=2",
            "ASSETS.Owner=1",
            "ASSETS.City=99",
            "ASSETS.PlotCity=99",
            "ASSETS.Plot=9",
            "ASSETS.HolySites[1].Complete=false",
            "ASSETS.HolySites[1].Pillaged=true",
            "ASSETS.HolySites[1].City=99",
            "ASSETS.BUILDING_SHRINE.AtSite=false",
            "ASSETS.BUILDING_TEMPLE.Present=false",
            "ASSETS.BUILDING_TEMPLE.Pillaged=true",
            "NATIVE_OWNER=1",
            "NATIVE_DISTRICT=-1",
        ] {
            let setup = format!(
                r#"
TEST.NativePreparation={{Read=2,Plot=8}}
ASSETS={{Serial=3,Turn=3,Owner=0,City=12,X=4,Y=5,Plot=8,PlotOwner=0,PlotCity=12,PlotCityOwner=0,
 HolySites={{{{Owner=0,City=12,Plot=8,Complete=true,Pillaged=false}}}},
 BUILDING_SHRINE={{Present=true,AtSite=true,Pillaged=false}},BUILDING_TEMPLE={{Present=true,AtSite=true,Pillaged=false}}}}
ExposedMembers={{JAC_OfficialProductNativeAssets=ASSETS}}
Players[0].GetCities=function() return {{GetCapitalCity=function() return {{
 GetID=function() return 12 end,GetX=function() return 4 end,GetY=function() return 5 end}} end}} end
NATIVE_OWNER,NATIVE_DISTRICT=0,7
Map={{GetPlotByIndex=function(index) assert(index==8 or index==9);return {{GetOwner=function() return NATIVE_OWNER end,
 GetDistrictType=function() return NATIVE_DISTRICT end}} end}}
GameInfo={{Districts={{DISTRICT_HOLY_SITE={{Index=7}}}}}}
{mutation}
"#
            );
            let result = run(&setup, "NativeAssets('TEMPLE')")?;
            assert_eq!(
                result.is_ok(),
                mutation.is_empty(),
                "{mutation}: {result:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn send_assertions_require_exact_native_delta_saved_identity_receipt_and_turn() -> Result<()> {
        for mutation in [
            "",
            "POOL=32",
            "POOL=30",
            "RECEIVED=0",
            "RECEIVED=2",
            "TURN=2",
            "STATE.AiEnvoyTurns['0'].Turn=2",
            "STATE.AiEnvoyTurns['0'].Operation='other'",
            "OP.Done=false",
            "OP.EnvoyReceipt.ChargeDone=false",
            "OP.EnvoyReceipt.GrantDone=false",
            "OP.EnvoyReceipt.Done=false",
            "OP.EnvoyReceipt.GrantIntent=false",
            "OP.EnvoyReceipt.Before=33",
            "OP.EnvoyReceipt.ReceivedBefore=1",
            "OP.EnvoyIdentity.Route='PRODUCT'",
            "OP.EnvoyIdentity.CountryID='SINGAPORE'",
            "OP.EnvoyIdentity.From=1",
            "OP.EnvoyIdentity.To=1",
            "OP.Identity.From=1",
            "OP.Identity.To=1",
            "OP.EnvoyIdentity.LeaderType='LEADER_MINOR_CIV_SINGAPORE'",
            "OP.EnvoyIdentity.SenderLeaderType='LEADER_JAC_ODA_NOBUHIDE'",
            "OP.Identity.ExpectedEnvoys=1",
            "OP.Nonce='test-direct'",
            "STATE.Envoys['2:0']={Count=1}",
            "STATE.Operations.extra={Identity={Command='REGISTER',To=0,From=2},Done=true}",
        ] {
            let result = run(mutation, "Send(t.BeforeFirst)")?;
            assert_eq!(
                result.is_ok(),
                mutation.is_empty(),
                "{mutation}: {result:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn next_turn_requires_distinct_operation_and_preserves_old_receipt() -> Result<()> {
        for mutation in [
            "",
            "STATE.Operations[OPKEY].EnvoyReceipt.After=29",
            "STATE.AiEnvoyTurns['0'].Turn=3",
        ] {
            let result = run(
                &format!(
                    r#"
local copy=JAC_AbsorptionGameplay.Rules.Copy
TEST.First={{Key=OPKEY,Operation=copy(OP)}}
TEST.BeforeSecond={{Turn=3,Pool=31,Received=1,Product=copy(STATE)}}
TURN,POOL,RECEIVED=4,30,2
VIEW.Envoys.Envoys=2
local next=copy(OP);next.Identity.ExpectedEnvoys=1;next.Nonce='ai-envoy:2:1'
next.EnvoyReceipt={{Before=31,After=30,ReceivedBefore=1,ReceivedAfter=2,ChargeDone=true,GrantIntent=true,GrantDone=true,Done=true}}
STATE.Operations['0:ai-envoy:2:1']=next;STATE.AiEnvoyTurns['0']={{Turn=4,Operation='0:ai-envoy:2:1'}}
{mutation}
"#
                ),
                "Send(t.BeforeSecond,t.First)",
            )?;
            assert_eq!(
                result.is_ok(),
                mutation.is_empty(),
                "{mutation}: {result:?}"
            );
        }
        Ok(())
    }

    const QUALIFICATION_WORLD: &str = r#"
local copy=JAC_AbsorptionGameplay.Rules.Copy
TEST.PreparedView=copy(VIEW)
TEST.First={Key=OPKEY,Operation=copy(OP)}
TEST.Second={Key='0:ai-envoy:2:1',Operation=copy(OP)}
TEST.Second.Operation.Identity.ExpectedEnvoys=1;TEST.Second.Operation.Nonce='ai-envoy:2:1'
TEST.Second.Operation.EnvoyReceipt={Before=31,After=30,ReceivedBefore=1,ReceivedAfter=2,ChargeDone=true,GrantIntent=true,GrantDone=true,Done=true}
STATE.Operations[TEST.Second.Key]=copy(TEST.Second.Operation)
STATE.AiEnvoyTurns['0']={Turn=4,Operation=TEST.Second.Key}
TURN,POOL,RECEIVED=5,30,2
VIEW.Envoys.Envoys=2
TEST.BeforeQualification={Turn=4,Pool=30,Received=2,Gold=GOLD,Faith=FAITH,Product=copy(STATE),CurrencyCursor=1}
ExposedMembers={JAC_OfficialProductCurrencySnapshots={Owner=0,Rows={{Turn=4,Gold=GOLD,Faith=FAITH}}}}
"#;

    #[test]
    fn every_qualification_turn_reconciles_observed_native_counts_with_new_receipt() -> Result<()> {
        for mutation in [
            "",
            "POOL=28",
            "RECEIVED=4",
            "NEXT.EnvoyReceipt.Before=31;NEXT.EnvoyReceipt.After=30;POOL=30",
            "NEXT.EnvoyReceipt.ReceivedBefore=3;NEXT.EnvoyReceipt.ReceivedAfter=4;RECEIVED=4",
            "NEXT.EnvoyIdentity.Route='PRODUCT'",
            "NEXT.Identity.From=1",
            "STATE.Operations.extra=JAC_AbsorptionGameplay.Rules.Copy(NEXT)",
        ] {
            let setup = format!(
                r#"
{QUALIFICATION_WORLD}
NEXT=copy(TEST.Second.Operation);NEXT.Identity.ExpectedEnvoys=2;NEXT.Nonce='ai-envoy:2:2'
NEXT.EnvoyReceipt={{Before=30,After=29,ReceivedBefore=2,ReceivedAfter=3,ChargeDone=true,GrantIntent=true,GrantDone=true,Done=true}}
STATE.Operations['0:ai-envoy:2:2']=NEXT;STATE.AiEnvoyTurns['0']={{Turn=5,Operation='0:ai-envoy:2:2'}}
POOL,RECEIVED=29,3
{mutation}
"#
            );
            let result = run(&setup, QUALIFICATION_TURN)?;
            assert_eq!(
                result.is_ok(),
                mutation.is_empty(),
                "{mutation}: {result:?}"
            );
        }
        // No product receipt cannot justify an extra native-AI send either.
        for mutation in ["", "POOL=29", "RECEIVED=3"] {
            let result = run(
                &format!("{QUALIFICATION_WORLD}\n{mutation}"),
                QUALIFICATION_TURN,
            )?;
            assert_eq!(
                result.is_ok(),
                mutation.is_empty(),
                "no receipt/{mutation}: {result:?}"
            );
        }
        Ok(())
    }

    const ACCEPTED_WORLD: &str = r#"
OFFER={ID='product:2:0:1',From=2,To=0,AreaKm2=0.44,Requirements={
 {ID='leader:FAITH_YIELD',Kind='PRODUCT_METRIC',Metric='FAITH_YIELD',Minimum=5},
 {ID='deposit',Kind='PRODUCT_DEPOSIT'}, {ID='identity',Kind='PRODUCT_IDENTITY'},
 {ID='terms',Kind='PRODUCT_CONTEXT'}, {ID='reserve:gold',Kind='GOLD_MINIMUM',Minimum=50}},
 ProductTerms={Input={Country={Profile=copy(VIEW.Country.Profile)}},Deposit={Gold=100,Faith=20}},
 Plan={LeaderProfile=copy(VIEW.Country.Profile),ProductDeposit={OfferID='product:2:0:1',From=2,To=0,Gold=100,Faith=20,Policy='CONSUME_ON_VERIFIED_RESULT_V2'}}}
REQUEST={Status='STARTED',FlowID='normal-flow',StartTurn=5,
 Plan={ID='normal-flow',From=2,To=0,Mode='SMALL_STATE',EntryConditionsMet=true,RelationsResolved=true,
  LeaderProfile=copy(OFFER.Plan.LeaderProfile),ProductDeposit=copy(OFFER.Plan.ProductDeposit)},
 Decision={From=2,To=0,Mode='SMALL_STATE',AreaKm2=0.44,Turn=5,RelationsResolved=true,
  EnvoySource='product',EnvoysEligible=true,OwnedTiles=2,Envoys=2,Requirements={}}}
for _,d in ipairs(OFFER.Requirements) do REQUEST.Decision.Requirements[#REQUEST.Decision.Requirements+1]={ID=d.ID,Met=true} end
REGISTER={Identity={Command='REGISTER',From=2,To=0},Nonce='ai-register:2',Done=true,OfferIntent={Offer=copy(OFFER)}}
STATE.Pairs['2:0']={ID=OFFER.ID,Sequence=1}
STATE.Operations['0:ai-register:2']=REGISTER
ESCROW={ID=OFFER.ID,From=2,To=0,Gold=100,Faith=20,Status='HELD',CollectGold={Before=10100,After=10000,Done=true},CollectFaith={Before=10000,After=9980,Done=true}}
STATE.Escrows[OFFER.ID]=ESCROW
GOLD,FAITH=10000,9980
VIEW.Status,VIEW.Frozen,VIEW.OfferID='STARTED',true,OFFER.ID
VIEW.Envoys={Route='FROZEN',Envoys=2,Required=2}
JAC_AbsorptionAdmissionGameplay.Get=function(_,id) assert(id==OFFER.ID);return copy(REQUEST) end
JAC_AbsorptionAdmissionGameplay.GetOffer=function(id) assert(id==OFFER.ID);return copy(OFFER) end
JAC_AbsorptionFlowGameplay={Get=function(id) assert(id=='normal-flow');return {Phase1Complete=true} end}
"#;

    #[test]
    fn early_started_registration_uses_saved_requirements_and_frozen_envoys() -> Result<()> {
        for mutation in [
            "",
            "REQUEST.Status='WAITING'",
            "REQUEST.Decision.Requirements[1].Met=false",
            "REQUEST.Decision.Requirements[5]=nil",
            "REQUEST.Decision.Requirements[2].ID='fake-demand'",
            "REQUEST.Decision.Envoys=1",
            "REQUEST.Decision.EnvoysEligible=false",
            "REQUEST.Decision.RelationsResolved=false",
            "REQUEST.Decision.Turn=4",
            "REQUEST.Plan.EntryConditionsMet=false",
            "REQUEST.Plan.From=1",
            "REQUEST.Plan.LeaderProfile.LeaderType='LEADER_MINOR_CIV_SINGAPORE'",
            "VIEW.Envoys.Envoys=3",
            "VIEW.Envoys.Route='OFFICIAL'",
        ] {
            let result = run(
                &format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}\n{mutation}"),
                QUALIFICATION_TURN,
            )?;
            assert_eq!(
                result.is_ok(),
                mutation.is_empty(),
                "{mutation}: {result:?}"
            );
        }
        // Once saved acceptance is authoritative, native yields can change
        // with real absorption; do not re-evaluate a frozen demand against it.
        let result = run(
            &format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}\nNATIVE_FAITH_YIELD=0"),
            QUALIFICATION_TURN,
        )?;
        assert!(result.is_ok(), "accepted frozen demand: {result:?}");
        let result = run(
            &format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}"),
            &format!("local function ObserveTurn()\n{QUALIFICATION_TURN}\nend\nObserveTurn()\n{QUALIFIED}"),
        )?;
        assert!(result.is_ok(), "early accepted continuation: {result:?}");
        // The pre-publication WAITING branch still checks live metrics.
        let waiting = format!(
            r#"
{QUALIFICATION_WORLD}
{ACCEPTED_WORLD}
REQUEST.Status='WAITING';REQUEST.Plan=nil;REQUEST.Decision=nil
VIEW.Status='WAITING';VIEW.Frozen=nil;VIEW.Envoys={{Route='OFFICIAL',Envoys=2,Required=2,Eligible=true}}
STATE.Escrows={{}};GOLD,FAITH=10100,10000
Map={{GetPlotCount=function() return 2 end,GetPlotByIndex=function() return {{GetOwner=function() return 2 end}} end}}
"#
        );
        let result = run(&waiting, QUALIFICATION_TURN)?;
        assert!(result.is_ok(), "ordinary waiting registration: {result:?}");
        Ok(())
    }

    #[test]
    fn actual_debit_observation_rejects_omission_and_unobserved_or_competing_balances() -> Result<()>
    {
        for mutation in [
            "", "GOLD=10100", "FAITH=10000", "GOLD=10100;FAITH=10000",
            "GOLD=9999", "ESCROW.CollectGold.After=10001", "ESCROW.CollectFaith.Done=false",
            "TEST.BeforeQualification.Gold=9999;ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows={}",
            "ExposedMembers.JAC_OfficialProductCurrencySnapshots.Owner=1",
            "ExposedMembers.JAC_OfficialProductCurrencySnapshots.Overflow=true",
            "ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows[2]={Turn=5,Gold=10000,Faith=9999}",
        ] {
            let result=run(&format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}\n{mutation}"),
                "ObserveDebit(t.BeforeQualification,Product(),OFFER.ID)")?;
            assert_eq!(result.is_ok(),mutation.is_empty(),"{mutation}: {result:?}");
        }
        // A native income change is usable only if its real Before was
        // independently observed; do not infer it from the receipt itself.
        let result = run(
            &format!(
                r#"
{QUALIFICATION_WORLD}
{ACCEPTED_WORLD}
TEST.BeforeQualification.Gold=10090;TEST.BeforeQualification.Faith=9990
ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows={{{{Turn=5,Gold=10100,Faith=10000}},{{Turn=5,Gold=10000,Faith=10000}},{{Turn=5,Gold=10000,Faith=9980}}}}
"#
            ),
            "ObserveDebit(t.BeforeQualification,Product(),OFFER.ID)",
        )?;
        assert!(
            result.is_ok(),
            "independently observed income and debit: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn replay_and_reload_compare_the_entire_payer_core_snapshot() -> Result<()> {
        for mutation in [
            "",
            "POOL=30",
            "RECEIVED=2",
            "GOLD=GOLD-100",
            "FAITH=FAITH-1",
            "TURN=4",
            "STATE.AiEnvoyTurns['0'].Turn=4",
            "OP.EnvoyReceipt.Done=false",
            "STATE.Operations.extra={Done=true}",
            "PROPS.JAC_ABSORPTION_ADMISSION_V1={Changed=true}",
            "PROPS.JAC_ABSORPTION_FLOW_V1={Changed=true}",
            "JAC_AbsorptionProductGameplay.Get=function() POOL=POOL-1;return VIEW end",
        ] {
            let lua = Lua::new();
            lua.load(WORLD).exec()?;
            lua.load(format!(
                "{CORE}\nt.SettledFirst=Snapshot();t.PublicationReplays=2;Game:SetProperty(KEY,t)"
            ))
            .exec()?;
            lua.load(mutation).exec()?;
            let result = lua.load(format!("{CORE}\n{AFTER_LOAD}")).exec();
            assert_eq!(
                result.is_ok(),
                mutation.is_empty(),
                "{mutation}: {result:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn publication_guard_cannot_pass_without_real_same_turn_observations() -> Result<()> {
        for observation in [
            "{From=2,To=0,Turn=3,Count=2}",
            "{From=2,To=0,Turn=3,Count=1}",
            "{From=2,To=0,Turn=2,Count=2}",
            "{From=2,To=1,Turn=3,Count=2}",
            "nil",
        ] {
            let lua = Lua::new();
            lua.load(WORLD).exec()?;
            lua.load(format!(
                "{CORE}\nt.SettledFirst=Snapshot();Game:SetProperty(KEY,t)"
            ))
            .exec()?;
            lua.load(format!(
                "ExposedMembers={{JAC_OfficialProductPublicationObservation={observation}}}"
            ))
            .exec()?;
            let result = lua.load(format!("{CORE}\n{ASSERT_PUBLICATIONS}")).exec();
            assert_eq!(
                result.is_ok(),
                observation == "{From=2,To=0,Turn=3,Count=2}",
                "{observation}: {result:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn scene_has_two_same_name_round_trips_and_single_actual_turn_steps() -> Result<()> {
        let scene = scenario(&Repository::checked_in())?;
        let steps = scene["steps"].as_array().unwrap();
        let mut trips = 0;
        let mut turns = 0;
        for (index, step) in steps.iter().enumerate() {
            if step["action"] == "save" {
                trips += 1;
                assert_eq!(steps[index + 1]["action"], "exit_to_menu");
                assert_eq!(steps[index + 2]["action"], "load");
                assert_eq!(step["name"], steps[index + 2]["name"]);
                assert_eq!(steps[index + 3]["action"], "wait_for");
            } else if step["action"] == "end_turn" {
                turns += 1;
                assert_eq!(step["turns"], 1);
                assert_eq!(steps[index + 1]["action"], "wait_for");
            }
        }
        assert_eq!(trips, 2);
        assert_eq!(turns, 13); // one native founding turn, two measured sends, ten qualification turns
        let rendered = serde_json::to_string(&scene)?;
        assert!(!rendered.contains("test_mode"));
        assert!(rendered.contains("qualified product receipt"));
        Ok(())
    }

    #[test]
    fn catalog_keeps_the_real_selected_minor_profile_and_native_metric() -> Result<()> {
        let catalog = absorption_product::catalog(&Repository::checked_in())?;
        let row = catalog
            .leaders
            .iter()
            .find(|row| row.country == "VATICAN_CITY")
            .context("Vatican City product country")?;
        assert_eq!(row.civilization, "CIVILIZATION_VATICAN_CITY");
        assert_eq!(row.leader, "LEADER_MINOR_CIV_VATICAN_CITY");
        assert_eq!(row.area_km2, 0.44);
        assert_eq!(row.personality, "慎重");
        assert_eq!(row.trait_name, "霊神");
        assert_eq!(row.metric, absorption_product::Metric::FaithYield);
        assert_eq!(
            catalog.runtime["Countries"]["VATICAN_CITY"]["RelationsReviewed"],
            true
        );
        assert_eq!(
            catalog.runtime["Countries"]["SINGAPORE"]["RelationsReviewed"],
            false
        );
        Ok(())
    }

    #[test]
    fn cli_requires_an_explicit_absolute_json_and_check_never_writes() -> Result<()> {
        let root = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&root)?;
        let dir = tempfile::tempdir_in(root)?;
        let output = dir.path().join("external-scene.json");
        let args = vec![
            "--output".to_owned(),
            output.to_string_lossy().into_owned(),
            "--check".to_owned(),
        ];
        assert_eq!(
            options(&args)?,
            Some(Options {
                output: output.clone(),
                check: true
            })
        );
        assert!(options(&[]).is_err());
        assert!(options(&["--check".into()]).is_err());
        assert!(options(&["--output".into(), "relative.json".into()]).is_err());
        assert!(options(&["--output".into()]).is_err());
        assert!(options(&[
            "--output".into(),
            output.to_string_lossy().into_owned(),
            "--output".into()
        ])
        .is_err());
        assert!(options(&[
            "--output".into(),
            output.to_string_lossy().into_owned(),
            "--tick".into()
        ])
        .is_err());
        assert!(options(&[
            "--output".into(),
            output.with_extension("rs").to_string_lossy().into_owned()
        ])
        .is_err());
        assert!(options(&["--help".into()])?.is_none());
        assert!(emit(&output, "new\n", true).is_err());
        assert!(!output.exists());
        emit(&output, "original\n", false)?;
        emit(&output, "original\n", true)?;
        assert!(emit(&output, "different\n", true).is_err());
        assert_eq!(std::fs::read_to_string(output)?, "original\n");
        Ok(())
    }
}
