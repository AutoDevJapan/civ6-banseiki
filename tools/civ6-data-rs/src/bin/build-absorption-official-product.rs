//! An additional, opt-in acceptance scene; no checked-in recipe is overwritten.
//!
//! Installed primary references (Sid Meier's Civilization VI):
//! - Base/Assets/Configuration/Data/SetupParameters.xml:26 and
//!   DLC/Expansion2/Config/Expansion2_Config.xml:25: EXCLUDE_CITY_STATES is
//!   a GameConfiguration array, selected from the Expansion2CityStates domain.
//! - Base/Assets/UI/FrontEnd/GameSetupLogic.lua:788-827 and
//!   Base/Assets/UI/Automation/Automation_StandardTests.lua:38-53: map-size
//!   major/minor bounds are separate; participating count is majors + hidden,
//!   not majors + city states. Base/Assets/Configuration/Data/MapSizes.xml:5
//!   supplies the official DUEL bounds (0..6 minors).
//! - Base/Assets/UI/FrontEnd/PlayerSetupLogic.lua:49-95 and
//!   Base/Assets/UI/FrontEnd/Mods.lua:904: preserve actual major selections;
//!   update enabled content after the final configuration. Expansion2's
//!   Maps/Utility/AssignStartingPlots.lua:144-150 reads already alive minor IDs;
//!   changing a count in Gameplay cannot create the missing engine slot.
//! - SetupParameters.xml:53 and SetupParameters.lua:Config_WriteAuxParameterValues
//!   supply LEADER_VALUES. GranColombia_Maya.modinfo:15-18,56-65 gates Vatican
//!   gameplay data behind LeaderPlayable or a standard ruleset; frontend
//!   CityStates availability alone is not proof that its gameplay row exists.
//!   SetupParameters.lua:714-728 permits a nil auxiliary list when Values is
//!   absent; dependency proof requires at least one matching computed list,
//!   rather than a nonempty serialized list on every selected actor.
//! - Actual26 established that standard automatic allocation works with two
//!   majors/one minor. Do not replace it with manual actor creation. The content
//!   precheck verifies selectable DLC dependency values; actual gameplay
//!   civilization/leader availability is asserted only in loaded Core.
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
//! Observer lifecycle: WAITING -> INTENT (no native Flow journal, fresh retry)
//! -> JOURNAL_PENDING (immutable native Request/decision/old rolls) -> STARTED.
//! Replay continuity is checked against the original saved receipt checkpoint
//! before advancing the next-turn snapshot; equal native counts are insufficient.

use anyhow::{bail, ensure, Context, Result};
use civ6_data_rs::{absorption_admission, Repository};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const USAGE: &str =
    "build-absorption-official-product --output <absolute-json-path> [--check] [--frontend-probe]";

const P1: &str = civ6_data_rs::absorption_product::P1_EVIDENCE;

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
"#;

const FRONTEND_ALLOCATE: &str = r#"
-- Headless setup can retain bounds from the previous map/ruleset domain.
-- Storing a count/exclusion list alone does not repair native map-size bounds.
-- Reapply the official configuration contract after selecting Continents.
assert(GameConfiguration.GetRuleSet()=='RULESET_JAC_BANSEIKI','wrong official acceptance ruleset')
assert(not GameConfiguration.IsAnyMultiplayer(),'single-player official acceptance required')
local expected={
  {Civ='CIVILIZATION_JAC_TAKEDA',Leader='LEADER_JAC_TAKEDA_NOBUTORA',Slot=SlotStatus.SS_COMPUTER},
  {Civ='CIVILIZATION_JAC_ODA',Leader='LEADER_JAC_ODA_NOBUHIDE',Slot=SlotStatus.SS_TAKEN}}
local function MajorsUnchanged()
  for index,row in ipairs(expected) do
    local p=assert(PlayerConfigurations[index-1],'configured major slot missing')
    assert(p:GetCivilizationTypeName()==row.Civ and p:GetLeaderTypeName()==row.Leader and p:GetSlotStatus()==row.Slot,
      'standard setup changed an actual major civilization, leader or controller')
  end
end
MajorsUnchanged()
-- Standard PlayerSetupLogic writes LEADER_DOMAIN/LEADER_VALUES auxiliary
-- data; DLC LeaderPlayable criteria consume it. The diagnostic's two actual
-- leaders stay fixed, but its selectable pool must expose official content.
local pools=DB.ConfigurationQuery("SELECT Value FROM DomainValues WHERE Domain = 'JAC_BanseikiHumanPools' AND Value = 'JAC_ALL_POOLS'")
assert(type(pools)=='table' and #pools==1 and pools[1].Value=='JAC_ALL_POOLS','existing stock-inclusive pool option unavailable')
GameConfiguration.SetValue('JAC_BANSEIKI_HUMAN_POOL',pools[1].Value)
assert(GameConfiguration.GetValue('JAC_BANSEIKI_HUMAN_POOL')=='JAC_ALL_POOLS','stock-inclusive pool option not applied')
MapConfiguration.SetScript('Continents.lua')
include('PlayerSetupLogic')
BuildHeadlessGameSetup()
g_GameParameters:FullRefresh()
CreatePlayerParameters(0,true)
CreatePlayerParameters(1,true)
RefreshPlayerParameters()
-- Read the helper's actual parameter and permission contract before releasing
-- it. Config_CanWriteParameter rejects a nonlocal human slot in non-hotseat;
-- an absent serialized list is not necessarily an absent calculated domain.
local function ParameterRead(label,fn)
  local ok,value=pcall(fn)
  print('[Metadata_Boot_PreSnapshot] '..label..' ok='..tostring(ok)..' type='..(ok and type(value) or 'error')..' value='..tostring(value))
end
ParameterRead('local-player-id',function() return Network.GetLocalPlayerID() end)
ParameterRead('hotseat',function() return GameConfiguration.IsHotseat() end)
for actor=0,1 do
  local lookup,parameters=pcall(function() return GetPlayerParameters(actor) end)
  if not lookup then parameters=nil end
  local parameter=parameters and parameters.Parameters and parameters.Parameters.PlayerLeader
  ParameterRead('parameter-'..actor..'-present',function() return parameter~=nil end)
  ParameterRead('parameter-'..actor..'-values-type',function() return parameter and type(parameter.Values) end)
  ParameterRead('parameter-'..actor..'-selected',function() local v=parameter and parameter.Value;return type(v)=='table' and v.Value or v end)
  ParameterRead('parameter-'..actor..'-can-write',function() return parameter and parameters:Config_CanWriteParameter(parameter) end)
  ParameterRead('player-'..actor..'-human',function() return PlayerConfigurations[actor]:IsHuman() end)
  if parameter and type(parameter.Values)=='table' then for _,row in ipairs(parameter.Values) do
    if row.Value=='LEADER_SIMON_BOLIVAR' or row.Value=='LEADER_LADY_SIX_SKY' then
      ParameterRead('parameter-'..actor..'-dlc-domain',function() return row.Domain end)
      ParameterRead('parameter-'..actor..'-dlc-invalid',function() return row.Invalid end)
    end
  end end
end
-- These are values computed by the official parameter helper, not supplied
-- by the fixture. They are the inputs to DLC LeaderPlayable activation.
local dlcPlayable=false
for actor=0,1 do
  local values=PlayerConfigurations[actor]:GetValue('LEADER_VALUES')
  print('[Metadata_Boot_PreSnapshot] leader-values-'..actor..' type='..type(values)..' value='..tostring(values))
  -- SetupParameters:Config_WriteAuxParameterValues (714-728) writes nil
  -- when this parameter has no Values table. An actual selected leader can
  -- remain valid without a serialized selectable list in every actor slot.
  -- LeaderPlayable is an OR content dependency; a helper-generated matching
  -- list on either actual major is sufficient. No auxiliary value is invented.
  assert(values==nil or type(values)=='string','official selectable leader values have unexpected type at actor '..actor)
  if type(values)=='string' then for value in values:gmatch('[^,]+') do
      if value=='Players:Expansion2_Players::LEADER_SIMON_BOLIVAR' or value=='Players:Expansion2_Players::LEADER_LADY_SIX_SKY' then dlcPlayable=true end
    end
  end
end
assert(dlcPlayable,'normal helper did not expose a Maya/Gran Colombia LeaderPlayable dependency')
HideGameSetup()
ReleasePlayerParameters()
MajorsUnchanged()
-- Content dependencies must see the selected country pool, not the previous
-- map's empty pool. This is the normal FrontEnd API, not a manual NPC slot.
GameConfiguration.UpdateEnabledMods()
local sizes=DB.ConfigurationQuery("SELECT MinPlayers, MaxPlayers, MinCityStates, MaxCityStates FROM MapSizes WHERE Domain = 'StandardMapSizes' AND MapSizeType = 'MAPSIZE_DUEL'")
assert(type(sizes)=='table' and #sizes==1,'official DUEL configuration bounds unavailable')
local size=sizes[1]
assert(type(size.MinPlayers)=='number' and size.MinPlayers<=2 and size.MaxPlayers>=2
  and type(size.MinCityStates)=='number' and size.MinCityStates<=1 and size.MaxCityStates>=1,
  'official DUEL cannot support two majors and one minor')
assert(MapConfiguration.GetMapSize()=='MAPSIZE_DUEL' or MapConfiguration.GetMapSize()==DB.MakeHash('MAPSIZE_DUEL'),'DUEL map size changed')
MapConfiguration.SetMinMajorPlayers(size.MinPlayers)
MapConfiguration.SetMaxMajorPlayers(2)
MapConfiguration.SetMinMinorPlayers(size.MinCityStates)
MapConfiguration.SetMaxMinorPlayers(size.MaxCityStates)
GameConfiguration.SetValue('CITY_STATE_COUNT',1)
GameConfiguration.SetParticipatingPlayerCount(2+GameConfiguration.GetHiddenPlayerCount())
MajorsUnchanged()
assert(MapConfiguration.GetMinMajorPlayers()==size.MinPlayers and MapConfiguration.GetMaxMajorPlayers()==2
  and MapConfiguration.GetMinMinorPlayers()==size.MinCityStates and MapConfiguration.GetMaxMinorPlayers()==size.MaxCityStates,
  'native map-size player bounds were not applied')
assert(GameConfiguration.GetValue('CITY_STATE_COUNT')==1 and MapConfiguration.GetValue('MAP_SCRIPT')=='Continents.lua',
  'final engine minor count/map script changed')
-- Validate the complete current pool again after enabling content. No country
-- classification, leader override, hand-built minor or ready flag is supplied.
local finalRows=DB.ConfigurationQuery("SELECT CivilizationType FROM CityStates WHERE Domain = 'Expansion2CityStates' ORDER BY CivilizationType")
local finalExclude=GameConfiguration.GetValue('EXCLUDE_CITY_STATES');local exclusions={}
assert(type(finalRows)=='table' and type(finalExclude)=='table','final native country pool unavailable')
for _,civ in ipairs(finalExclude) do assert(not exclusions[civ],'duplicate final country exclusion');exclusions[civ]=true end
local included,excluded=0,0
for _,row in ipairs(finalRows) do
  if exclusions[row.CivilizationType] then excluded=excluded+1
  else included=included+1;assert(row.CivilizationType=='CIVILIZATION_VATICAN_CITY','final pool includes another official country') end
end
assert(included==1 and excluded==#finalExclude,'final engine pool is not exactly Vatican City')
print('[JAC_OFFICIAL_SETUP] script=Continents.lua majors=2 hidden='..GameConfiguration.GetHiddenPlayerCount()..
  ' minor-count=1 min-minor='..MapConfiguration.GetMinMinorPlayers()..' max-minor='..MapConfiguration.GetMaxMinorPlayers())
"#;

const FRONTEND_BOOTSTRAP: &str = r#"
-- FrontEnd GameInfo may not yet be configured for the game being hosted.
-- Its absence is not a failed content load. Core validates the real data.
local function Read(label,fn)
  local ok,v=pcall(fn);print('[Metadata_Boot_PreSnapshot] '..label..' ok='..tostring(ok)..' type='..(ok and type(v) or 'error')..' value='..tostring(v))
end
for actor=0,63 do
  local config=PlayerConfigurations[actor]
  if config then
    Read('slot-'..actor..'-status',function() return config:GetSlotStatus() end)
    Read('slot-'..actor..'-civilization',function() return config:GetCivilizationTypeName() end)
    Read('slot-'..actor..'-leader',function() return config:GetLeaderTypeName() end)
    Read('slot-'..actor..'-level',function() return config:GetCivilizationLevelTypeID() end)
  end
end
return 'stock-inclusive selectable dependency and unchanged major slots prepared; loaded Core must verify native Vatican content and allocation'
"#;

const FRONTEND_PROBE: &str = r#"
local function Read(label,fn)
  local ok,v=pcall(fn);print('[Metadata_Boot_PreSnapshot] '..label..' ok='..tostring(ok)..' type='..(ok and type(v) or 'error')..' value='..tostring(v))
end
Read('rule',function() return GameConfiguration.GetRuleSet() end)
Read('city-state-count',function() return GameConfiguration.GetValue('CITY_STATE_COUNT') end)
Read('hidden-count',function() return GameConfiguration.GetHiddenPlayerCount() end)
Read('participating-ids',function() return table.concat(GameConfiguration.GetParticipatingPlayerIDs(),',') end)
Read('human-pool',function() return GameConfiguration.GetValue('JAC_BANSEIKI_HUMAN_POOL') end)
Read('gameplay-vatican',function() local row=GameInfo.Civilizations['CIVILIZATION_VATICAN_CITY'];return row and row.StartingCivilizationLevelType end)
Read('gameplay-vatican-leader',function() return GameInfo.Leaders['LEADER_MINOR_CIV_VATICAN_CITY']~=nil end)
for actor=0,63 do local p=PlayerConfigurations[actor];if p then
  -- One unsupported getter cannot discard the other independent facts.
  Read('slot-'..actor..'-status',function() return p:GetSlotStatus() end)
  Read('slot-'..actor..'-civilization',function() return p:GetCivilizationTypeName() end)
  Read('slot-'..actor..'-leader',function() return p:GetLeaderTypeName() end)
  Read('slot-'..actor..'-level',function() return p:GetCivilizationLevelTypeID() end)
  Read('slot-'..actor..'-leader-values',function() return p:GetValue('LEADER_VALUES') end)
  Read('slot-'..actor..'-leader-domain',function() return p:GetValue('LEADER_DOMAIN') end)
  Read('slot-'..actor..'-leader-value-domain',function() return p:GetValue('LEADER_VALUE_DOMAIN') end)
  Read('slot-'..actor..'-human',function() return p:IsHuman() end)
end end
return 'readonly native frontend metadata captured; no host, no actor creation, no acceptance claim'
"#;

const CORE_ROSTER: &str = r#"
local definition=GameInfo.Civilizations['CIVILIZATION_VATICAN_CITY']
local leaderDefinition=GameInfo.Leaders['LEADER_MINOR_CIV_VATICAN_CITY']
print('[JAC_OFFICIAL_CONTENT] actual-vatican='..tostring(definition~=nil)..' actual-leader='..tostring(leaderDefinition~=nil))
assert(definition and leaderDefinition and definition.StartingCivilizationLevelType=='CIVILIZATION_LEVEL_CITY_STATE',
  'loaded Core Vatican civilization/leader content missing or not an official city state')
local majorCount,minorCount,minor=0,0,nil
local rows={}
for actor=0,63 do
  local config=PlayerConfigurations[actor];local p=Players[actor]
  if p and p:IsAlive() then
    local influence=p:GetInfluence()
    local receives=influence and type(influence.CanReceiveInfluence)=='function' and influence:CanReceiveInfluence() or false
    local civ=config and config:GetCivilizationTypeName();local leader=config and config:GetLeaderTypeName()
    rows[#rows+1]=actor..':'..tostring(civ)..':'..tostring(leader)..':major='..tostring(p:IsMajor())..':receives='..tostring(receives)
    if p:IsMajor() then majorCount=majorCount+1 end
    if receives then
      minorCount=minorCount+1;minor=actor
      assert(not p:IsMajor() and civ=='CIVILIZATION_VATICAN_CITY' and leader=='LEADER_MINOR_CIV_VATICAN_CITY',
        'engine generated an unexpected official minor profile')
      assert(config:GetCivilizationLevelTypeID()==CivilizationLevelTypes.CIVILIZATION_LEVEL_CITY_STATE,
        'actual configured Vatican player is not at the native city-state level')
    end
  end
end
print('[JAC_OFFICIAL_ROSTER] '..table.concat(rows,' | '))
assert(majorCount==2 and minorCount==1 and minor~=nil,
  'native city-state allocation missing: actual major='..majorCount..' actual influence-receiving minor='..minorCount)
local found=false
for _,p in ipairs(PlayerManager.GetAliveMinors()) do if p:GetID()==minor then found=true end end
assert(found,'actual official minor is absent from the native minor roster')
return 'all 64 actual slots read in Core; exactly two majors and one official Vatican minor allocated by the engine'
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
    TransferTransactions=R.Copy(A.GetState().Transactions or {}),
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
local function ReceiptContinuity(before,after,reload)
  for key,old in pairs(before.Operations or {}) do
    local current=assert((after.Operations or {})[key],'saved operation disappeared: '..key)
    if old.Done~=true and old.Identity.Command=='REGISTER' then
      assert(R.Equal(old.Identity,current.Identity) and old.Nonce==current.Nonce
        and R.Equal(old.OfferIntent,current.OfferIntent) and R.Equal(old.EnvoyIdentity,current.EnvoyIdentity)
        and R.Equal(old.EnvoyReceipt,current.EnvoyReceipt)
        and (current.Done==nil or current.Done==false or current.Done==true),'pending registration changed immutable saved intent')
    else
      assert(R.Equal(old,current),'completed operation/identity/receipt/done changed: '..key)
    end
  end
  if reload then
    assert(R.Equal(before.AiEnvoyTurns or {},after.AiEnvoyTurns or {}),'reload moved or deleted sender turn-budget metadata')
  end
end
local function Lifecycle(request)
  local raw=Game:GetProperty('JAC_ABSORPTION_FLOW_V1')
  if raw~=nil then
    assert(type(raw)=='table' and raw.Version==1 and type(raw.Flows)=='table','unsupported native Flow journal')
  end
  local flow=raw and raw.Flows[request.FlowID]
  local old=t.FrozenEvidence and t.FrozenEvidence[request.FlowID]
  if flow then
    assert(type(flow)=='table' and type(flow.Request)=='table' and request.Plan
      and flow.ID==request.FlowID and flow.From==t.From and flow.To==t.To
      and flow.Mode=='SMALL_STATE' and R.Equal(flow.Request,request.Plan),'native Flow Request does not bind the actual saved plan')
    return {Kind=request.Status=='STARTED' and 'STARTED' or 'JOURNAL_PENDING',Journal=flow}
  end
  assert(not old,'immutable native Flow journal disappeared')
  assert(request.Status~='STARTED','STARTED without a persisted native Flow journal cannot pass')
  return {Kind=request.Plan and 'INTENT' or 'WAITING'}
end
local function EnvoyTurn(before,s)
  ReceiptContinuity(before.Product,s,false)
  if t.ReplayBaseline then
    assert(t.ReplayHash==A.Canonical(t.ReplayBaseline),'original replay checkpoint was replaced')
    ReceiptContinuity(t.ReplayBaseline.Product,s,false)
  end
  local count,key,operation=0,nil,nil
  for k,op in pairs(s.Operations or {}) do
    local old=(before.Product.Operations or {})[k]
    if old then
      if old.Identity.Command=='REGISTER' and not old.Done then
        assert(R.Equal(old.Identity,op.Identity) and old.Nonce==op.Nonce and R.Equal(old.OfferIntent,op.OfferIntent),
          'pending registration changed its saved intent')
      else assert(R.Equal(old,op),'settled operation changed on a later natural turn') end
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
  local lifecycle=Lifecycle(request)
  if not lifecycle.Journal then
    -- STARTING can be a failed preflight with an old intent. N.Check must
    -- re-read the next turn's conditions; neither its plan nor decision freezes.
    assert(request.Status=='STARTING' or request.Status=='WAITING','invalid pre-journal admission phase')
    return nil
  end
  assert((request.Status=='STARTED' or request.Status=='STARTING') and request.Plan and request.Decision,'saved admission evidence missing')
  local plan,decision=request.Plan,request.Decision
  assert(plan.ID==request.FlowID and plan.From==t.From and plan.To==t.To and plan.Mode=='SMALL_STATE'
    and plan.EntryConditionsMet==true and plan.RelationsResolved==true,'saved admission plan changed actors or gate result')
  assert(R.Equal(plan.LeaderProfile,t.PreparedView.Country.Profile) and R.Equal(plan.LeaderProfile,offer.Plan.LeaderProfile)
    and R.Equal(plan.ProductDeposit,offer.Plan.ProductDeposit),'saved admission plan changed normal source or deposit')
  assert(decision.From==t.From and decision.To==t.To and decision.Mode=='SMALL_STATE' and decision.AreaKm2==0.44
    and decision.RelationsResolved==true and decision.EnvoySource=='product' and decision.EnvoysEligible==true,
    'saved normal admission decision changed identity, relationships or envoy eligibility')
  assert(type(decision.Turn)=='number' and decision.Turn>=0 and decision.Turn==math.floor(decision.Turn) and decision.Turn<=Game.GetCurrentGameTurn(),
    'saved admission has an invalid actual acceptance turn')
  if request.Status=='STARTED' then
    assert(type(request.StartTurn)=='number' and request.StartTurn==math.floor(request.StartTurn)
      and request.StartTurn>=decision.Turn and request.StartTurn<=Game.GetCurrentGameTurn(),'completion turn precedes saved decision')
    assert(lifecycle.Journal.Phase1Complete==true,'STARTED native journal has not completed phase one')
  end
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
  assert(view.Frozen==true and view.Status==request.Status and view.OfferID==offer.ID and view.Envoys.Route=='FROZEN'
    and view.Envoys.Envoys==decision.Envoys and view.Envoys.Required==math.max(1,decision.OwnedTiles),
    'frozen product view differs from its saved accepted plan/envoys')
  return decision
end
-- Observation contract: a Core snapshot fixes one immutable native starting
-- balance and a sequence fence. No historical value before that fence can
-- authorize a later debit. The full canonical snapshot binds the checkpoint.
local function CurrencyOrigin(snapshot)
  local history=assert(ExposedMembers.JAC_OfficialProductCurrencySnapshots,'currency observer absent')
  assert(history.Owner==t.To and not history.Overflow,'currency observer invalid')
  snapshot.CurrencyCursor=#history.Rows
  snapshot.CurrencyPoint={Turn=snapshot.Turn,Gold=snapshot.Gold,Faith=snapshot.Faith,Sequence=snapshot.CurrencyCursor}
  snapshot.CurrencyHash=A.Canonical(snapshot.CurrencyPoint)
  snapshot.CurrencyPrefixHash=A.Canonical(history.Rows)
  t.CurrencyHistory=R.Copy(history)
  return snapshot
end
local function ObserveDebit(before,s,id)
  local escrow=assert((s.Escrows or {})[id],'actual deposit escrow absent')
  local deposit=assert(N.GetOffer(id)).ProductTerms.Deposit
  assert(escrow.ID==id and escrow.From==t.From and escrow.To==t.To and escrow.Gold==deposit.Gold and escrow.Faith==deposit.Faith,
    'actual deposit changed offer, payer or ordinary amounts')
  assert(escrow.Gold>0 and escrow.Faith>0 and (escrow.Status=='COLLECTING' or escrow.Status=='HELD' or escrow.Status=='CONSUMED'),
    'ordinary cautious deposit has invalid phase')
  t.NativeDebits=t.NativeDebits or {};t.DebitOrigins=t.DebitOrigins or {};t.DebitParts=t.DebitParts or {};t.DebitIntents=t.DebitIntents or {}
  local origin=t.DebitOrigins[id]
  if not origin then origin=R.Copy(before);t.DebitOrigins[id]=origin end
  assert(origin.CurrencyPoint and origin.CurrencyHash==A.Canonical(origin.CurrencyPoint)
    and origin.CurrencyPoint.Sequence==origin.CurrencyCursor and origin.CurrencyPoint.Turn==origin.Turn
    and origin.CurrencyPoint.Gold==origin.Gold and origin.CurrencyPoint.Faith==origin.Faith,'currency snapshot sequence/hash contract changed')
  local observations=assert(ExposedMembers.JAC_OfficialProductCurrencySnapshots,'native currency observations absent')
  local fence=origin.CurrencyCursor
  assert(observations.Owner==t.To and not observations.Overflow and type(fence)=='number'
    and fence>=0 and fence==math.floor(fence) and fence<=#observations.Rows,'native currency origin/sequence unavailable')
  local prefix={};for index=1,fence do prefix[index]=observations.Rows[index] end
  assert(origin.CurrencyPrefixHash==A.Canonical(prefix),'native history before the snapshot fence was replaced')
  local parts=t.DebitParts[id] or {};t.DebitParts[id]=parts
  local intents=t.DebitIntents[id] or {};t.DebitIntents[id]=intents
  local complete=escrow.Status~='COLLECTING'
  for _,currency in ipairs({'Gold','Faith'}) do
    local leg=escrow['Collect'..currency];local proof=parts[currency]
    if leg then
      assert(type(leg.Before)=='number' and type(leg.After)=='number' and leg.Before-leg.After==escrow[currency],
        'currency intent does not match ordinary amount')
      local intent={Before=leg.Before,After=leg.After,Amount=escrow[currency]}
      assert(not intents[currency] or R.Equal(intents[currency],intent),'pending currency intent changed')
      intents[currency]=intent
      if proof then
        assert(proof.OriginHash==origin.CurrencyHash and proof.BeforeSequence>=fence and proof.AfterSequence>proof.BeforeSequence
          and proof.Before==leg.Before and proof.After==leg.After and proof.Amount==escrow[currency]
          and leg.Done==true and R.Equal(observations.Rows[proof.AfterSequence],proof.ActualAfter),
          'saved actual currency proof or sequenced observation changed')
        assert(proof.ActualBefore[currency]==proof.Before and proof.ActualAfter[currency]==proof.After,
          'stored native observations do not cover the real currency leg')
        if proof.BeforeSequence==fence then assert(R.Equal(proof.ActualBefore,origin.CurrencyPoint),'original Core currency point changed') end
        if proof.BeforeSequence>fence then
          assert(R.Equal(observations.Rows[proof.BeforeSequence],proof.ActualBefore),'native Before observation changed')
        end
      elseif leg.Done==true then
        local beforeIndex=origin.CurrencyPoint[currency]==leg.Before and fence or nil
        local afterIndex
        -- Never search at or before the fixed snapshot fence. An observed
        -- income after it may precede collection; old same-turn spending may not.
        for index=fence+1,#observations.Rows do
          local row=observations.Rows[index]
          assert(row.Sequence==index and row.Turn>=origin.Turn and row.Turn<=Game.GetCurrentGameTurn(),'currency observation sequence/turn changed')
          if not beforeIndex and row[currency]==leg.Before then beforeIndex=index
          elseif beforeIndex and row[currency]==leg.After then afterIndex=index;break
          elseif beforeIndex and row[currency]~=leg.Before then error('competing currency change inside native debit observations') end
        end
        local native=currency=='Gold' and Players[t.To]:GetTreasury():GetGoldBalance() or A.PlayerFaithBalance(t.To)
        if beforeIndex and not afterIndex and native==leg.After then
          -- This Core read is a new independent observation after the fence,
          -- including when the native UI publication is one update behind.
          assert(#observations.Rows<256,'native currency observation limit reached')
          local row={Turn=Game.GetCurrentGameTurn(),Gold=Players[t.To]:GetTreasury():GetGoldBalance(),Faith=A.PlayerFaithBalance(t.To),Sequence=#observations.Rows+1}
          observations.Rows[#observations.Rows+1]=row;afterIndex=row.Sequence
        end
        if beforeIndex and afterIndex then
          proof={OriginHash=origin.CurrencyHash,BeforeSequence=beforeIndex,AfterSequence=afterIndex,
            Before=leg.Before,After=leg.After,Amount=escrow[currency],ActualAfter=R.Copy(observations.Rows[afterIndex]),
            ActualBefore=R.Copy(beforeIndex==fence and origin.CurrencyPoint or observations.Rows[beforeIndex])}
          parts[currency]=proof
        else complete=false end
      else complete=false end
    else complete=false end
  end
  t.CurrencyHistory=R.Copy(observations)
  if not complete then return false end
  local proof={From=t.From,To=t.To,Origin=R.Copy(origin.CurrencyPoint),OriginHash=origin.CurrencyHash,
    Gold=R.Copy(parts.Gold),Faith=R.Copy(parts.Faith)}
  local old=t.NativeDebits[id]
  assert(not old or R.Equal(old,proof),'complete native debit evidence changed')
  t.NativeDebits[id]=proof
  print('[JAC_OFFICIAL_PRODUCT] actual-debit offer='..id..' origin='..fence..' gold-after='..proof.Gold.AfterSequence..' faith-after='..proof.Faith.AfterSequence)
  return proof
end
local function FlowProgress(old,flow,label)
  assert(R.Equal(InventoryEvidence(old),InventoryEvidence(flow)),'saved initial inventory evidence changed: '..label)
  if old.Phase1Complete then assert(R.Equal(P1Evidence(old),P1Evidence(flow)),'saved first-stage evidence changed: '..label) end
  local entries={};for _,entry in ipairs(flow.Entries or {}) do
    assert(not entries[tostring(entry.ID)],'duplicate resumed asset');entries[tostring(entry.ID)]=entry
  end
  for _,entry in ipairs(old.Entries or {}) do
    local current=assert(entries[tostring(entry.ID)],'saved asset disappeared')
    -- A partial first-stage journal may acquire its remaining results, but
    -- already persisted chance/outcome values and every old roll are fixed.
    for _,field in ipairs({'P1Chance','EscapeOutcome'}) do
      if entry[field]~=nil then assert(R.Equal(entry[field],current[field]),'saved first-stage result changed: '..field) end
    end
    for stage,roll in pairs(entry.Rolls or {}) do assert(R.Equal((current.Rolls or {})[stage],roll),'saved random roll changed: '..stage) end
    entries[tostring(entry.ID)]=nil
  end
  assert(next(entries)==nil,'resumed flow added an unrelated asset')
  if old.FinalRollsComplete then assert(flow.FinalRollsComplete==true,'final-roll completion regressed') end
  if old.CompletedTurn then assert(flow.CompletedTurn==old.CompletedTurn,'actual completion turn changed') end
  local phases={CHOICE={CHOICE=true,APPLYING=true,BLOCKED=true,RESULT=true,DONE=true},
    APPLYING={APPLYING=true,BLOCKED=true,CHOICE=true,RESULT=true,DONE=true},
    BLOCKED={BLOCKED=true,CHOICE=true,APPLYING=true,RESULT=true,DONE=true},RESULT={RESULT=true,DONE=true},DONE={DONE=true}}
  assert(old.Phase==nil and flow.Phase==nil or phases[old.Phase] and phases[old.Phase][flow.Phase],'illegal saved flow phase transition')
end
local function PreservePending(request)
  local lifecycle=Lifecycle(request)
  if not lifecycle.Journal then return end
  t.FrozenEvidence=t.FrozenEvidence or {}
  local old=t.FrozenEvidence[request.FlowID]
  local flow=lifecycle.Journal
  local now={Plan=R.Copy(request.Plan),Decision=R.Copy(request.Decision),Rolls={},Flow=R.Copy(flow)}
  if flow then for _,entry in ipairs(flow.Entries or {}) do
    assert(not now.Rolls[tostring(entry.ID)],'duplicate frozen inventory entry')
    now.Rolls[tostring(entry.ID)]=R.Copy(entry.Rolls or {})
  end end
  if old then
    if old.Flow then FlowProgress(old.Flow,flow,'pending/read/reload') end
    assert(old.Hash==A.Canonical({Plan=old.Plan,Decision=old.Decision}),'frozen admission hash changed')
    assert(R.Equal(old.Plan,now.Plan) and R.Equal(old.Decision,now.Decision),'waiting/reload changed frozen admission evidence')
    for id,rolls in pairs(old.Rolls) do
      assert(now.Rolls[id],'waiting/reload lost a frozen inventory entry')
      for stage,roll in pairs(rolls) do assert(R.Equal(now.Rolls[id][stage],roll),'waiting/reload rerolled a frozen result') end
    end
  end
  now.Hash=A.Canonical({Plan=now.Plan,Decision=now.Decision})
  t.FrozenEvidence[request.FlowID]=now
end
local function VerifyConsumption(escrow,flow)
  local receipt=assert(escrow.Consumption,'consumed escrow receipt missing')
  assert(flow and (flow.Phase=='RESULT' or flow.Phase=='DONE') and flow.Phase1Complete and flow.FinalRollsComplete
    and not flow.Error and not flow.WaitingFor and type(flow.CompletedTurn)=='number','consumption lacks a completed native flow result')
  local tx=assert(A.GetState().Transactions[flow.ID],'consumption lacks actual asset-transfer transaction')
  assert(type(flow.AppliedPlan)=='table' and type(tx.Request)=='table'
    and tx.ID==flow.ID and tx.From==escrow.From and tx.To==escrow.To and tx.Mode==flow.Mode and tx.Status=='COMPLETE'
    and R.Equal(tx.Request,flow.AppliedPlan),'consumption is not backed by matching native COMPLETE transfer')
  assert(receipt.FlowID==flow.ID and receipt.From==escrow.From and receipt.To==escrow.To
    and receipt.CompletedTurn==flow.CompletedTurn and receipt.Policy=='CONSUME_ON_VERIFIED_RESULT_V2'
    and receipt.Gold==escrow.Gold and receipt.Faith==escrow.Faith
    and receipt.LegacyRefunded and receipt.LegacyRefunded.Gold==0 and receipt.LegacyRefunded.Faith==0,
    'consumption changed actual payer/amount/result or introduced a refund')
  for _,currency in ipairs({'Gold','Faith'}) do
    local paid=assert(escrow['Collect'..currency]);assert(paid.Done and paid.Before-paid.After==escrow[currency],'consumed actual collection leg changed')
  end
end
local function LegStageProgress(prior,leg,label)
  assert(type(prior)=='table' and type(leg)=='table',label..': actual native collection intent disappeared')
  for _,field in ipairs({'Issued','Done'}) do
    local before,after=prior[field],leg[field]
    assert((before==nil or type(before)=='boolean') and (after==nil or type(after)=='boolean'),
      label..': invalid native collection stage: '..field)
    assert(before~=true or after==true,label..': native collection stage regressed: '..field)
  end
  local a,b=R.Copy(prior),R.Copy(leg)
  a.Issued=nil;b.Issued=nil;a.Done=nil;b.Done=nil
  assert(R.Equal(a,b),label..': actual native collection intent changed')
end
local function SavedProgress(expected,label)
  local now=Snapshot()
  assert(now.Turn==expected.Turn and now.Pool==expected.Pool and now.Received==expected.Received,
    label..': saved actual turn or native envoy counts changed')
  ReceiptContinuity(expected.Product,now.Product,true)
  assert(expected.Product.Version==now.Product.Version,label..': product journal version changed')
  for key,operation in pairs(now.Product.Operations or {}) do
    assert((expected.Product.Operations or {})[key] or operation.Identity.Command=='REGISTER',label..': unexpected new envoy operation')
  end
  assert(R.Equal(expected.Product.Envoys or {},now.Product.Envoys or {}),label..': native route/product envoy ledger changed')
  for key,pair in pairs(expected.Product.Pairs or {}) do
    local current=assert((now.Product.Pairs or {})[key],label..': saved offer pair disappeared')
    local a,b=R.Copy(pair),R.Copy(current);a.LastError=nil;b.LastError=nil
    assert(R.Equal(a,b),label..': saved pair identity changed')
  end
  for id,old in pairs((expected.Flow or {}).Flows or {}) do
    assert(expected.Flow.Version==1 and now.Flow.Version==1,label..': native flow journal version changed')
    local flow=assert((now.Flow.Flows or {})[id],label..': native flow journal disappeared')
    FlowProgress(old,flow,label)
  end
  for key,old in pairs((expected.Admission or {}).Offers or {}) do
    assert(R.Equal(old,(now.Admission.Offers or {})[key]),label..': saved normal offer changed')
  end
  for key,old in pairs((expected.Admission or {}).Requests or {}) do
    local request=assert((now.Admission.Requests or {})[key],label..': saved admission disappeared')
    for _,field in ipairs({'Source','ID','From','To','FlowID'}) do assert(R.Equal(old[field],request[field]),label..': admission identity changed') end
    if ((expected.Flow or {}).Flows or {})[old.FlowID] then
      assert(R.Equal(old.Plan,request.Plan) and R.Equal(old.Decision,request.Decision),label..': journal-frozen admission changed')
    end
    if old.Status=='STARTED' then assert(request.Status=='STARTED' and request.StartTurn==old.StartTurn,label..': completed admission changed') end
  end
  for id,old in pairs(expected.TransferTransactions or {}) do
    local current=assert(now.TransferTransactions[id],label..': native transfer receipt disappeared')
    assert(old.ID==current.ID and old.From==current.From and old.To==current.To and R.Equal(old.Request,current.Request),
      label..': native transfer identity/request changed')
    if old.Mode then assert(current.Mode==old.Mode,label..': native transfer mode changed') end
    if old.Status=='COMPLETE' then assert(R.Equal(old,current),label..': complete native transfer receipt changed') end
  end
  local debit={Gold=0,Faith=0}
  for id,old in pairs(expected.Product.Escrows or {}) do
    assert((now.Product.Escrows or {})[id],label..': saved escrow disappeared')
  end
  for id,escrow in pairs(now.Product.Escrows or {}) do
    local old=(expected.Product.Escrows or {})[id]
    if old then
      for _,field in ipairs({'ID','From','To','Gold','Faith','SourceVersion'}) do assert(R.Equal(old[field],escrow[field]),label..': escrow identity changed') end
      if old.Status=='CONSUMED' then assert(R.Equal(old,escrow),label..': consumed receipt changed')
      elseif old.Status=='HELD' then assert(escrow.Status=='HELD' or escrow.Status=='CONSUMED',label..': held escrow did not progress legally')
      else assert(old.Status=='COLLECTING' and (escrow.Status=='COLLECTING' or escrow.Status=='HELD' or escrow.Status=='CONSUMED'),label..': invalid partial collection progression') end
    end
    for _,currency in ipairs({'Gold','Faith'}) do
      local leg=escrow['Collect'..currency];local prior=old and old['Collect'..currency]
      if prior then
        LegStageProgress(prior,leg,label)
      end
      if escrow.To==t.To and leg and leg.Done and not (prior and prior.Done) then
        assert(leg.Before-leg.After==escrow[currency],label..': resumed native collection amount changed')
        if expected[currency]==leg.Before then debit[currency]=debit[currency]+escrow[currency]
        else assert(expected[currency]==leg.After,label..': partial native debit origin ambiguous') end
      end
    end
    if escrow.Status=='CONSUMED' then
      local flow
      for _,candidate in pairs(now.Flow.Flows or {}) do if candidate.Request and candidate.Request.ProductDeposit
          and candidate.Request.ProductDeposit.OfferID==id then flow=candidate end end
      VerifyConsumption(escrow,flow)
    else assert(escrow.Consumption==nil,label..': unsettled escrow already has a consumption receipt') end
  end
  assert(now.Gold==expected.Gold-debit.Gold and now.Faith==expected.Faith-debit.Faith,
    label..': payer changed beyond newly observed native collection; re-debit/refund is not legal flow progression')
  -- Reading the public view must remain inert even when normal publications
  -- advanced APPLYING/RESULT/DONE before this observation.
  for i=1,3 do
    Profile(P.Get(t.From,t.To));assert(R.Equal(Snapshot(),now),label..': read-only replay changed current native state')
  end
  return now
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
  local influence=p:GetInfluence()
  if influence and influence:CanReceiveInfluence() then
  minors=minors+1
  local id=p:GetID();local config=PlayerConfigurations[id]
  assert(config:GetCivilizationTypeName()=='CIVILIZATION_VATICAN_CITY' and config:GetLeaderTypeName()=='LEADER_MINOR_CIV_VATICAN_CITY',
    'the configured sole official minor is not Vatican City')
  assert(not p:IsHuman() and not p:IsMajor() and p:GetInfluence():CanReceiveInfluence(),'actual official minor cannot receive influence')
  from=id
  end
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
t.SettledSecond=Snapshot();t.BeforeQualification=CurrencyOrigin(Snapshot())
Game:SetProperty(KEY,t)
return 'next actual end_turn sent exactly one more official envoy; previous receipt unchanged and turn budget renewed'
"#;

const CURRENCY_WATCH: &str = r#"
-- Detached native observations only. This observer never calls commands or
-- replaces any product reader; it cannot supply a ready/deposit condition.
if JAC_OfficialProductCurrencyCallback then Events.GameCoreEventPublishComplete.Remove(JAC_OfficialProductCurrencyCallback) end
local observations=ExposedMembers.JAC_OfficialProductCurrencySnapshots or {Owner=to,Rows={}}
ExposedMembers.JAC_OfficialProductCurrencySnapshots=observations
local function Sample()
  local turn=Game.GetCurrentGameTurn()
  local row={Turn=turn,Gold=Players[to]:GetTreasury():GetGoldBalance(),Faith=Players[to]:GetReligion():GetFaithBalance(),Sequence=#observations.Rows+1}
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
t.TurnOrigin=R.Copy(before)
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
    assert(not register and op.Identity.From==t.From and op.Nonce=='ai-register:'..t.From,'not one ordinary AI registration')
    register=op
  end end
  assert(register and register.OfferIntent.Offer.ID==pair.ID,'actual AI registration intent missing')
  local actual,tiles,accepted
  if request.Status=='STARTED' or request.Status=='STARTING' then
    -- A normal current-turn publication can already collect and admit.
    -- Inspect the frozen authoritative decision, never demand a live route.
    accepted=SavedAdmission(offer,request)
    PreservePending(request)
  else assert(request.Status=='WAITING','ordinary admission registration failed') end
  local intent=Lifecycle(request).Kind=='INTENT'
  if accepted then actual,tiles=accepted.Envoys,accepted.OwnedTiles
  else
    -- INTENT remains pending; its old decision is not accepted evidence.
    -- Fresh conditions may temporarily fail during a normal preflight retry.
    if not intent then
    NonDeposit(P.Get(t.From,t.To))
    end
    actual=Players[t.From]:GetInfluence():GetTokensReceived(t.To);tiles=0
    for i=0,Map.GetPlotCount()-1 do if Map.GetPlotByIndex(i):GetOwner()==t.From then tiles=tiles+1 end end
  end
  for _,d in ipairs(offer.Requirements) do if d.Kind=='PRODUCT_METRIC' then
    assert(d.Metric=='FAITH_YIELD' and not minimum,'normal faith yield demand changed');minimum=d.Minimum
  end end
  assert(minimum and (accepted or intent or FaithYield(t.To)>=minimum),'actual faith yield did not qualify the registered ordinary offer')
  if not intent then assert(actual>=math.max(1,tiles),'AI registered before actual official received envoys covered native territory') end
  t.Registration={Turn=Game.GetCurrentGameTurn(),ID=pair.ID,Operation=R.Copy(register),
    NativeFaithYield=not accepted and FaithYield(t.To) or nil,Decision=R.Copy(accepted),Received=actual,OwnedTiles=tiles}
end
if pair then
  local request=assert(N.Get('leader-offer',pair.ID))
  if request.Plan then SavedAdmission(assert(N.GetOffer(pair.ID)),request);PreservePending(request) end
  if (s.Escrows or {})[pair.ID] then ObserveDebit(before,s,pair.ID) end
end
t.BeforeQualification=CurrencyOrigin(Snapshot())
Game:SetProperty(KEY,t)
return 'natural qualification turn inspected: at most one official envoy per recipient and real non-deposit prerequisites for AI registration'
"#;

// A short, bounded poll lets native publications finish. Pending is a valid
// observation, not acceptance. Only normal end_turn/publication handlers can
// resume it; this condition reads their evidence and stores detached snapshots.
const OBSERVE_PENDING: &str = r#"
EnvoyTurn(assert(t.TurnOrigin),Product())
local pair=(Product().Pairs or {})[t.From..':'..t.To]
local complete=true
if pair then
  local request=assert(N.Get('leader-offer',pair.ID))
  if request.Plan then SavedAdmission(assert(N.GetOffer(pair.ID)),request);PreservePending(request) end
  local escrow=(Product().Escrows or {})[pair.ID]
  if escrow then complete=ObserveDebit(assert(t.TurnOrigin),Product(),pair.ID)~=false end
  complete=complete and request.Status=='STARTED'
end
t.PendingSnapshot=Snapshot();t.PendingHash=A.Canonical(t.PendingSnapshot)
t.CurrencyHistory=R.Copy(assert(ExposedMembers.JAC_OfficialProductCurrencySnapshots))
Game:SetProperty(KEY,t)
-- The fifth read ends this observation window even if still pending. The
-- finite next-turn sequence continues; the final QUALIFIED check cannot pass
-- without completed plan, receipts, native debits and phase one.
JAC_OFFICIAL_PENDING_READS=(JAC_OFFICIAL_PENDING_READS or 0)+1
local observed=complete or JAC_OFFICIAL_PENDING_READS>=5
if observed then t.BeforeQualification=CurrencyOrigin(Snapshot());Game:SetProperty(KEY,t) end
return observed
"#;

const RESTORE_OBSERVATION: &str = r#"
assert(t.PendingSnapshot and t.PendingHash==A.Canonical(t.PendingSnapshot),'saved pending checkpoint changed')
local old=t.PendingSnapshot;local now=Snapshot()
now=SavedProgress(old,'pending save/exit/load')
-- Verify all saved completed operations, including the third and later sends,
-- and exact sender budget metadata before any next-turn baseline is replaced.
ReceiptContinuity(old.Product,now.Product,true)
if t.ReplayBaseline then
  assert(t.ReplayHash==A.Canonical(t.ReplayBaseline),'original replay checkpoint was replaced')
  ReceiptContinuity(t.ReplayBaseline.Product,now.Product,false)
else t.ReplayBaseline=R.Copy(old);t.ReplayHash=A.Canonical(t.ReplayBaseline) end
local pair=(now.Product.Pairs or {})[t.From..':'..t.To]
if pair then
  local request=assert(N.Get('leader-offer',pair.ID))
  if request.Plan then SavedAdmission(assert(N.GetOffer(pair.ID)),request);PreservePending(request) end
  if (now.Product.Escrows or {})[pair.ID] then ObserveDebit(assert(t.BeforeQualification),now.Product,pair.ID) end
end
-- Reload may legitimately publish and finish STARTING/COLLECTING. Frozen
-- decisions/rolls and observed debits, rather than the mutable status enum,
-- define preservation. No normal product request is resent by this scene.
t.BeforeQualification=CurrencyOrigin(Snapshot());Game:SetProperty(KEY,t)
return 'pending checkpoint restored; frozen decision/rolls and origin sequence preserved across normal reload publications'
"#;

const QUALIFIED: &str = r#"
assert(t.Registration,'bounded real turns never registered a qualified ordinary AI offer')
local s=Product();local pair=assert(s.Pairs[t.From..':'..t.To]);local request=assert(N.Get('leader-offer',pair.ID))
assert(request.Status=='STARTED' and request.Plan and request.Plan.Mode=='SMALL_STATE',request.LastError or 'ordinary official-minor admission never started')
SavedAdmission(assert(N.GetOffer(pair.ID)),request)
local registered=false
for _,op in pairs(s.Operations) do if op.Identity.Command=='REGISTER' and op.Identity.To==t.To then
  assert(not registered and op.Done==true and op.Identity.From==t.From and op.Nonce=='ai-register:'..t.From,'registration operation is not complete')
  registered=true
end end
assert(registered,'qualified registration operation missing')
assert(t.NativeDebits and t.NativeDebits[pair.ID],'normal currency receipt lacks independently verified actual debit')
assert(ObserveDebit(assert(t.BeforeQualification),s,pair.ID),'native debit observations are still incomplete')
PreservePending(request)
local flow=assert(JAC_AbsorptionFlowGameplay.Get(request.FlowID))
assert(flow.Phase1Complete,'normal three-stage absorption did not reach its real first-stage result')
t.OfferID=pair.ID;t.FlowID=request.FlowID;t.Qualified=Snapshot();Game:SetProperty(KEY,t)
print('[JAC_OFFICIAL_PRODUCT] qualified offer='..pair.ID..' flow='..request.FlowID..' actual-turn='..t.Qualified.Turn)
return 'ordinary AI registration, official native envoy qualification, real deposit and normal admission verified'
"#;

fn core(code: &str) -> Value {
    json!({"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":format!("{P1}\n{CORE}\n{code}")})
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

fn observe_pending(steps: &mut Vec<Value>) {
    steps.push(core(
        "JAC_OFFICIAL_PENDING_READS=0;return 'begin finite read-only publication observation'",
    ));
    steps.push(json!({"action":"wait_for","timeout":30,"condition":{
        "type":"lua","state_name":"JAC_Absorption_Entry","code":format!("{P1}\n{CORE}\n{OBSERVE_PENDING}")}}));
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
    steps[2]["code"] = json!(format!(
        "{setup}\n{FRONTEND_FILTER}\n{FRONTEND_ALLOCATE}\n{FRONTEND_BOOTSTRAP}"
    ));
    steps.push(json!({"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":CORE_ROSTER}));
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
    for index in 0..10 {
        advance(&mut steps);
        steps.push(core(QUALIFICATION_TURN));
        observe_pending(&mut steps);
        if index == 0 {
            round_trip(&mut steps, "JAC-official-product-ai-pending");
            // Core restores only detached read evidence; UI still obtains
            // actors from official configuration and never reads properties.
            steps.push(core("ExposedMembers.JAC_OfficialProductCurrencySnapshots=R.Copy(assert(t.CurrencyHistory));return 'restore saved native observation history'"));
            steps.push(ui(CURRENCY_WATCH));
            steps.push(core(RESTORE_OBSERVATION));
        }
    }
    steps.push(wait("if not from or not to or not ExposedMembers.JAC_AbsorptionProduct then return false end;local v=ExposedMembers.JAC_AbsorptionProduct.Get(from,to);return v and v.Status=='STARTED'"));
    steps.push(core(QUALIFIED));
    steps.push(ui("Events.GameCoreEventPublishComplete.Remove(JAC_OfficialProductCurrencyCallback);JAC_OfficialProductCurrencyCallback=nil;ExposedMembers.JAC_OfficialProductCurrencySnapshots=nil;return 'native currency snapshot observer removed after verified debit'"));
    round_trip(&mut steps, "JAC-official-product-ai-qualified");
    steps.push(core("SavedProgress(assert(t.Qualified),'qualified same-name save/exit/load');return 'qualified original evidence, actual payer and receipts preserved with legal flow/escrow advancement'"));
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
    frontend_probe: bool,
}

fn options(args: &[String]) -> Result<Option<Options>> {
    if args == ["--help"] || args == ["-h"] {
        return Ok(None);
    }
    let mut output = None;
    let mut check = false;
    let mut frontend_probe = false;
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
            "--frontend-probe" => {
                ensure!(!frontend_probe, "--frontend-probe specified twice");
                frontend_probe = true;
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
    Ok(Some(Options {
        output,
        check,
        frontend_probe,
    }))
}

fn frontend_probe(repo: &Repository) -> Result<Value> {
    let base: Value = serde_json::from_str(&absorption_admission::envoy_probe_scenario(repo)?)?;
    let mut steps = base["steps"].as_array().context("official startup steps")?[..3].to_vec();
    let (setup, _) = steps[2]["code"]
        .as_str()
        .context("official setup Lua")?
        .rsplit_once("return ")
        .context("setup return")?;
    steps[2]["code"] = json!(format!("{setup}\n{FRONTEND_FILTER}\n{FRONTEND_PROBE}"));
    steps.push(json!({"action":"ui_lua","state_name":"FrontEnd","code":format!(
        "local function Prepare()\n{FRONTEND_ALLOCATE}\nend\nlocal ok,why=pcall(Prepare)\nprint('[Metadata_Boot_PreSnapshot] preparation-ok='..tostring(ok)..' reason='..tostring(why))\nlocal function Diagnose()\n{FRONTEND_PROBE}\nend\nDiagnose()\nassert(ok,why)\nreturn 'current native preparation executed and independently diagnosed'"
    )}));
    Ok(
        json!({"name":"Official minor frontend content and native slot metadata diagnostic; no host and no acceptance claim","judge_profile":"generic","steps":steps}),
    )
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
        serde_json::to_string_pretty(&if options.frontend_probe {
            frontend_probe(&Repository::checked_in())?
        } else {
            scenario(&Repository::checked_in())?
        })?
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
local function canonical(v)
 if type(v)~='table' then return type(v)..':'..tostring(v) end
 local keys={};for k in pairs(v) do keys[#keys+1]=k end;table.sort(keys,function(a,b) return tostring(a)<tostring(b) end)
 local parts={};for _,k in ipairs(keys) do parts[#parts+1]=canonical(k)..'='..canonical(v[k]) end;return '{'..table.concat(parts,';')..'}'
end
JAC_AbsorptionGameplay={Rules={Copy=copy,Equal=equal},PlayerFaithBalance=function() return FAITH end,Canonical=canonical,
 GetState=function() return {Transactions={}} end}
"#;

    fn run(mutation: &str, code: &str) -> Result<mlua::Result<()>> {
        let lua = Lua::new();
        lua.load(WORLD).exec()?;
        lua.load(mutation).exec()?;
        Ok(lua.load(format!("{P1}\n{CORE}\n{code}")).exec())
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
    fn standard_frontend_contract_restores_minor_bounds_after_pool_and_mod_update() -> Result<()> {
        // Test the documented API order/arguments only; this does not model
        // or claim that Civ6's native engine actually generated a city state.
        const SETUP: &str = r#"
ROWS={{CivilizationType='CIVILIZATION_GENEVA'},{CivilizationType='CIVILIZATION_VATICAN_CITY'},{CivilizationType='CIVILIZATION_SINGAPORE'}}
SIZE={MinPlayers=2,MaxPlayers=4,MinCityStates=0,MaxCityStates=6}
VALUES={CITY_STATE_COUNT=1};BOUNDS={MinMajor=2,MaxMajor=2,MinMinor=0,MaxMinor=0};CALLS={}
SlotStatus={SS_COMPUTER=1,SS_TAKEN=2}
local majors={{Civ='CIVILIZATION_JAC_TAKEDA',Leader='LEADER_JAC_TAKEDA_NOBUTORA',Slot=1},{Civ='CIVILIZATION_JAC_ODA',Leader='LEADER_JAC_ODA_NOBUHIDE',Slot=2}}
include=function(name) assert(name=='PlayerSetupLogic') end
BuildHeadlessGameSetup=function() g_GameParameters={FullRefresh=function() end} end
CreatePlayerParameters=function(index,headless) assert(index==0 or index==1);assert(headless==true) end
RefreshPlayerParameters=function()
 assert(VALUES.JAC_BANSEIKI_HUMAN_POOL=='JAC_ALL_POOLS')
 LEADER_VALUES=NO_DEPENDENCY and 'Players:Expansion2_Players::LEADER_JAC_TAKEDA_NOBUTORA' or
  'Players:Expansion2_Players::LEADER_JAC_TAKEDA_NOBUTORA,Players:Expansion2_Players::LEADER_JAC_ODA_NOBUHIDE,Players:Expansion2_Players::LEADER_SIMON_BOLIVAR'
end
HideGameSetup=function() g_GameParameters=nil end
ReleasePlayerParameters=function() end
PlayerConfigurations={}
for index,row in ipairs(majors) do PlayerConfigurations[index-1]={GetCivilizationTypeName=function() return row.Civ end,
 GetLeaderTypeName=function() return row.Leader end,GetSlotStatus=function() return row.Slot end,
 GetValue=function(_,key) assert(key=='LEADER_VALUES')
  if NIL_LIST_BOTH or NIL_LIST_SLOT==index-1 then return nil end
  if BAD_LIST_SLOT==index-1 then return {} end
  return LEADER_VALUES end} end
GameConfiguration={GetRuleSet=function() return 'RULESET_JAC_BANSEIKI' end,IsAnyMultiplayer=function() return false end,
 GetValue=function(key) return VALUES[key] end,SetValue=function(key,value) VALUES[key]=value end,
 GetHiddenPlayerCount=function() return 2 end,UpdateEnabledMods=function()
  assert(VALUES.EXCLUDE_CITY_STATES and #VALUES.EXCLUDE_CITY_STATES==2 and VALUES.JAC_BANSEIKI_HUMAN_POOL=='JAC_ALL_POOLS')
  assert(LEADER_VALUES and LEADER_VALUES:find('LEADER_SIMON_BOLIVAR',1,true),'mods refreshed before dependency helper values');CALLS[#CALLS+1]='mods'
  if CHANGE_MAJOR then majors[1].Leader='wrong-leader' end
  if DROP_FILTER then VALUES.EXCLUDE_CITY_STATES={} end
 end,
 SetParticipatingPlayerCount=function(n)
  assert(n==4,'city states must not be added to participating major/hidden count')
  assert(BOUNDS.MinMinor==0 and BOUNDS.MaxMinor==6 and VALUES.CITY_STATE_COUNT==1,'participating setup preceded native bounds/count')
  CALLS[#CALLS+1]='participating'
 end}
MapConfiguration={GetValue=function(key) assert(key=='MAP_SCRIPT');return 'Continents.lua' end,
 SetScript=function(value) assert(value=='Continents.lua') end,
 GetMapSize=function() return 'MAPSIZE_DUEL' end,
 SetMinMajorPlayers=function(n) BOUNDS.MinMajor=n end,SetMaxMajorPlayers=function(n) BOUNDS.MaxMajor=n end,
 SetMinMinorPlayers=function(n) assert(CALLS[1]=='mods');BOUNDS.MinMinor=n end,
 SetMaxMinorPlayers=function(n) if not DROP_BOUNDS then BOUNDS.MaxMinor=n end end,
 GetMinMajorPlayers=function() return BOUNDS.MinMajor end,GetMaxMajorPlayers=function() return BOUNDS.MaxMajor end,
 GetMinMinorPlayers=function() return BOUNDS.MinMinor end,GetMaxMinorPlayers=function() return BOUNDS.MaxMinor end}
DB={MakeHash=function() return 99 end,ConfigurationQuery=function(sql)
 if sql:find('FROM DomainValues',1,true) then
  assert(sql=="SELECT Value FROM DomainValues WHERE Domain = 'JAC_BanseikiHumanPools' AND Value = 'JAC_ALL_POOLS'")
  return NO_POOL and {} or {{Value='JAC_ALL_POOLS'}}
 end
 if sql:find('FROM MapSizes',1,true) then
  assert(sql:find("Domain = 'StandardMapSizes'",1,true) and sql:find("MapSizeType = 'MAPSIZE_DUEL'",1,true));return {SIZE}
 end
 assert(sql=="SELECT CivilizationType FROM CityStates WHERE Domain = 'Expansion2CityStates' ORDER BY CivilizationType");return ROWS
end}
"#;
        for (mutation, error) in [
            ("", None),
            ("NIL_LIST_SLOT=1", None),
            ("NIL_LIST_SLOT=0", None),
            (
                "CHANGE_MAJOR=true",
                Some("standard setup changed an actual major civilization, leader or controller"),
            ),
            (
                "DROP_FILTER=true",
                Some("final pool includes another official country"),
            ),
            (
                "DROP_BOUNDS=true",
                Some("participating setup preceded native bounds/count"),
            ),
            (
                "SIZE.MaxCityStates=0",
                Some("official DUEL cannot support two majors and one minor"),
            ),
            (
                "NO_POOL=true",
                Some("existing stock-inclusive pool option unavailable"),
            ),
            (
                "NO_DEPENDENCY=true",
                Some("normal helper did not expose a Maya/Gran Colombia LeaderPlayable dependency"),
            ),
            (
                "NIL_LIST_BOTH=true",
                Some("normal helper did not expose a Maya/Gran Colombia LeaderPlayable dependency"),
            ),
            (
                "BAD_LIST_SLOT=1",
                Some("official selectable leader values have unexpected type at actor 1"),
            ),
        ] {
            let lua = Lua::new();
            lua.load(SETUP).exec()?;
            lua.load(mutation).exec()?;
            let result = lua
                .load(format!("{FRONTEND_FILTER}\n{FRONTEND_ALLOCATE}"))
                .exec();
            assert_eq!(result.is_ok(), error.is_none(), "{mutation}: {result:?}");
            if let Some(expected) = error {
                assert!(
                    result.unwrap_err().to_string().contains(expected),
                    "wrong failure for {mutation}: expected {expected}"
                );
            } else {
                lua.load("assert(PlayerConfigurations[0]:GetLeaderTypeName()=='LEADER_JAC_TAKEDA_NOBUTORA' and PlayerConfigurations[1]:GetLeaderTypeName()=='LEADER_JAC_ODA_NOBUHIDE');assert(VALUES.JAC_BANSEIKI_HUMAN_POOL=='JAC_ALL_POOLS');assert(CALLS[1]=='mods' and CALLS[2]=='participating');assert(BOUNDS.MaxMinor==6)").exec()?;
            }
        }
        Ok(())
    }

    #[test]
    fn core_roster_accepts_real_vatican_and_rejects_other_profile_or_major() -> Result<()> {
        for mutation in [
            "",
            "MINOR.Civ='CIVILIZATION_SINGAPORE'",
            "MINOR.Major=true",
            "MINOR.Receives=false",
            "MINOR.Level=0",
            "GameInfo.Civilizations={}",
            "GameInfo.Leaders={}",
        ] {
            let lua = Lua::new();
            lua.load(r#"
Players,PlayerConfigurations={},{}
CivilizationLevelTypes={CIVILIZATION_LEVEL_CITY_STATE=3}
GameInfo={Civilizations={CIVILIZATION_VATICAN_CITY={StartingCivilizationLevelType='CIVILIZATION_LEVEL_CITY_STATE'}},Leaders={LEADER_MINOR_CIV_VATICAN_CITY={}}}
MINOR={ID=2,Civ='CIVILIZATION_VATICAN_CITY',Leader='LEADER_MINOR_CIV_VATICAN_CITY',Major=false,Receives=true,Level=3}
local rows={{ID=0,Civ='CIVILIZATION_JAC_TAKEDA',Leader='LEADER_JAC_TAKEDA_NOBUTORA',Major=true},
 {ID=1,Civ='CIVILIZATION_JAC_ODA',Leader='LEADER_JAC_ODA_NOBUHIDE',Major=true},MINOR}
for _,row in ipairs(rows) do
 Players[row.ID]={GetID=function() return row.ID end,IsAlive=function() return true end,IsMajor=function() return row.Major==true end,
 GetInfluence=function() return {CanReceiveInfluence=function() return row.Receives==true end} end}
 PlayerConfigurations[row.ID]={GetCivilizationTypeName=function() return row.Civ end,GetLeaderTypeName=function() return row.Leader end,GetCivilizationLevelTypeID=function() return row.Level end}
end
PlayerManager={GetAliveMinors=function() return {Players[2]} end}
"#).exec()?;
            lua.load(mutation).exec()?;
            assert_eq!(
                lua.load(CORE_ROSTER).exec().is_ok(),
                mutation.is_empty(),
                "{mutation}"
            );
        }
        Ok(())
    }

    #[test]
    fn core_roster_rejects_empty_native_city_state_before_actor_wait() -> Result<()> {
        let lua = Lua::new();
        lua.load(r#"
Players,PlayerConfigurations={},{}
local rows={{ID=0,Civ='CIVILIZATION_JAC_TAKEDA',Leader='LEADER_JAC_TAKEDA_NOBUTORA',Major=true},
 {ID=1,Civ='CIVILIZATION_JAC_ODA',Leader='LEADER_JAC_ODA_NOBUHIDE',Major=true},
 {ID=62,Civ='CIVILIZATION_FREE_CITIES',Leader='LEADER_FREE_CITIES'},
 {ID=63,Civ='CIVILIZATION_BARBARIAN'}}
for _,row in ipairs(rows) do
 Players[row.ID]={IsAlive=function() return true end,IsMajor=function() return row.Major==true end,
 GetInfluence=function() return {CanReceiveInfluence=function() return false end} end}
 PlayerConfigurations[row.ID]={GetCivilizationTypeName=function() return row.Civ end,GetLeaderTypeName=function() return row.Leader end}
end
PlayerManager={GetAliveMinors=function() return {Players[62]} end}
GameInfo={Civilizations={CIVILIZATION_VATICAN_CITY={StartingCivilizationLevelType='CIVILIZATION_LEVEL_CITY_STATE'}},Leaders={LEADER_MINOR_CIV_VATICAN_CITY={}}}
"#).exec()?;
        let result = lua.load(CORE_ROSTER).exec();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("actual influence-receiving minor=0"));
        let scene = scenario(&Repository::checked_in())?;
        assert_eq!(scene["steps"][6]["state_name"], "JAC_Absorption_Entry");
        assert_eq!(scene["steps"][6]["code"], CORE_ROSTER);
        assert_eq!(scene["steps"][7]["action"], "wait_for");
        assert!(!CORE_ROSTER.contains("Set"));
        Ok(())
    }

    #[test]
    fn frontend_without_gameplay_data_is_valid_but_core_requires_actual_definition() -> Result<()> {
        const NATIVE_CONFIG: &str = r#"
SlotStatus={SS_CLOSED=0,SS_COMPUTER=1,SS_TAKEN=2}
CivilizationLevelTypes={CIVILIZATION_LEVEL_CITY_STATE=3}
GameInfo={Civilizations={CIVILIZATION_VATICAN_CITY={StartingCivilizationLevelType='CIVILIZATION_LEVEL_CITY_STATE'}},Leaders={LEADER_MINOR_CIV_VATICAN_CITY={}}}
local rows={[0]={Civ='CIVILIZATION_JAC_TAKEDA',Leader='LEADER_JAC_TAKEDA_NOBUTORA',Slot=1,Level=0},
 [1]={Civ='CIVILIZATION_JAC_ODA',Leader='LEADER_JAC_ODA_NOBUHIDE',Slot=2,Level=0},[2]={Slot=0,Level=0}}
PlayerConfigurations={}
for id,row in pairs(rows) do
 PlayerConfigurations[id]={GetCivilizationTypeName=function() return row.Civ end,GetLeaderTypeName=function() return row.Leader end,
 GetSlotStatus=function() return row.Slot end,GetCivilizationLevelTypeID=function() return row.Level end}
end
"#;
        for mutation in [
            "",
            "GameInfo.Civilizations={}",
            "GameInfo.Leaders={}",
            "GameInfo.Civilizations.CIVILIZATION_VATICAN_CITY.StartingCivilizationLevelType='CIVILIZATION_LEVEL_FULL_CIV'",
        ] {
            let lua = Lua::new();
            lua.load(NATIVE_CONFIG).exec()?;
            lua.load(mutation).exec()?;
            let result = lua.load(FRONTEND_BOOTSTRAP).exec();
            assert!(result.is_ok(), "Frontend may lack GameInfo: {mutation}: {result:?}");
            lua.load("Players={};PlayerManager={GetAliveMinors=function() return {} end}").exec()?;
            let core=lua.load(CORE_ROSTER).exec();
            assert!(core.is_err(),"Core cannot pass missing data or empty native roster");
            if !mutation.is_empty() {
                assert!(core.unwrap_err().to_string().contains("loaded Core Vatican civilization/leader content"));
            }
        }
        let diagnostic = frontend_probe(&Repository::checked_in())?;
        let lua = Lua::new();
        for step in diagnostic["steps"].as_array().unwrap() {
            assert_ne!(step["action"], "start_game");
            if let Some(code) = step["code"].as_str() {
                lua.load(code).into_function()?;
                assert!(!code.contains(FRONTEND_BOOTSTRAP));
                assert!(!code.contains("SetCivilizationTypeName"));
            }
        }
        assert!(!FRONTEND_PROBE.contains("SetValue"));
        assert!(!FRONTEND_BOOTSTRAP.contains("assert("));
        // Validate the actual existing option and filter contract instead of
        // assuming an invented PLUS_STOCK enum from the handoff wording.
        let config = Repository::checked_in()
            .read("poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml")?;
        let xml = roxmltree::Document::parse(&config)?;
        assert!(xml
            .descendants()
            .any(|n| n.attribute("Domain") == Some("JAC_BanseikiHumanPools")
                && n.attribute("Value") == Some("JAC_ALL_POOLS")));
        assert!(config.contains("?3 = 'JAC_BANSEIKI_ONLY'"));
        Ok(())
    }

    #[test]
    fn diagnostics_keep_independent_getters_and_execute_current_preparation_on_failure(
    ) -> Result<()> {
        let lua = Lua::new();
        lua.load(r#"
MESSAGES={}
print=function(message) MESSAGES[#MESSAGES+1]=message end
GameConfiguration={GetRuleSet=function() return 'RULESET_JAC_BANSEIKI' end,
 GetValue=function(key) return key=='CITY_STATE_COUNT' and 1 or 'JAC_ALL_POOLS' end,
 GetHiddenPlayerCount=function() return 0 end,GetParticipatingPlayerIDs=function() return {0,1} end}
PlayerConfigurations={[0]={GetSlotStatus=function() error('status getter unavailable') end,
 GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_TAKEDA' end,
 GetLeaderTypeName=function() return 'LEADER_JAC_TAKEDA_NOBUTORA' end,
 GetValue=function(_,key) return key=='LEADER_VALUES' and 'Players:Expansion2_Players::LEADER_SIMON_BOLIVAR' or nil end}}
function HasMessage(text) for _,message in ipairs(MESSAGES) do if message:find(text,1,true) then return true end end;return false end
"#).exec()?;
        for source in [FRONTEND_BOOTSTRAP, FRONTEND_PROBE] {
            lua.load("MESSAGES={}").exec()?;
            lua.load(source).exec()?;
            lua.load(
                r#"
assert(HasMessage('slot-0-status ok=false type=error'))
assert(HasMessage('slot-0-civilization ok=true type=string value=CIVILIZATION_JAC_TAKEDA'))
assert(HasMessage('slot-0-leader ok=true type=string value=LEADER_JAC_TAKEDA_NOBUTORA'))
assert(HasMessage('slot-0-level ok=false type=error'))
"#,
            )
            .exec()?;
        }
        lua.load(r#"
assert(HasMessage('slot-0-leader-values ok=true type=string value=Players:Expansion2_Players::LEADER_SIMON_BOLIVAR'))
assert(HasMessage('gameplay-vatican ok=false type=error'))
"#).exec()?;
        let scene = frontend_probe(&Repository::checked_in())?;
        let preparation = scene["steps"][3]["code"]
            .as_str()
            .context("current prepare/probe Lua")?;
        assert!(preparation.contains(FRONTEND_ALLOCATE));
        lua.load("MESSAGES={};GameConfiguration.IsAnyMultiplayer=function() return true end")
            .exec()?;
        let result = lua.load(preparation).exec();
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("single-player official acceptance required"));
        lua.load("assert(HasMessage('preparation-ok=false'));assert(HasMessage('slot-0-civilization ok=true type=string value=CIVILIZATION_JAC_TAKEDA'));assert(HasMessage('slot-0-leader-values ok=true type=string'))").exec()?;
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
ExposedMembers={JAC_OfficialProductCurrencySnapshots={Owner=0,Rows={{Turn=4,Gold=GOLD,Faith=FAITH,Sequence=1}}}}
TEST.BeforeQualification.CurrencyPoint={Turn=4,Gold=GOLD,Faith=FAITH,Sequence=1}
TEST.BeforeQualification.CurrencyHash=JAC_AbsorptionGameplay.Canonical(TEST.BeforeQualification.CurrencyPoint)
TEST.BeforeQualification.CurrencyPrefixHash=JAC_AbsorptionGameplay.Canonical(ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows)
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
PROPS.JAC_ABSORPTION_FLOW_V1={Version=1,Flows={['normal-flow']={ID='normal-flow',From=2,To=0,Mode='SMALL_STATE',Request=copy(REQUEST.Plan),Entries={},Phase1Complete=true}}}
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
            "REQUEST.StartTurn=4",
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
PROPS.JAC_ABSORPTION_FLOW_V1=nil
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
                "assert(ObserveDebit(t.BeforeQualification,Product(),OFFER.ID),'actual debit incomplete')")?;
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
TEST.BeforeQualification.CurrencyPoint={{Turn=4,Gold=10090,Faith=9990,Sequence=1}}
TEST.BeforeQualification.CurrencyHash=JAC_AbsorptionGameplay.Canonical(TEST.BeforeQualification.CurrencyPoint)
ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows={{{{Turn=4,Gold=10090,Faith=9990,Sequence=1}}}}
TEST.BeforeQualification.CurrencyPrefixHash=JAC_AbsorptionGameplay.Canonical(ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows)
ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows[2]={{Turn=5,Gold=10100,Faith=10000,Sequence=2}}
ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows[3]={{Turn=5,Gold=10000,Faith=10000,Sequence=3}}
ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows[4]={{Turn=5,Gold=10000,Faith=9980,Sequence=4}}
"#
            ),
            "assert(ObserveDebit(t.BeforeQualification,Product(),OFFER.ID))",
        )?;
        assert!(
            result.is_ok(),
            "independently observed income and debit: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn stale_same_turn_history_cannot_pay_a_debit_after_the_snapshot_fence() -> Result<()> {
        let stale = format!(
            r#"
{QUALIFICATION_WORLD}
{ACCEPTED_WORLD}
ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows={{{{Turn=5,Gold=10100,Faith=10000,Sequence=1}},{{Turn=5,Gold=10000,Faith=9980,Sequence=2}}}}
TEST.BeforeQualification.Turn=5;TEST.BeforeQualification.Gold=10000;TEST.BeforeQualification.Faith=9980
TEST.BeforeQualification.CurrencyCursor=2
TEST.BeforeQualification.CurrencyPoint={{Turn=5,Gold=10000,Faith=9980,Sequence=2}}
TEST.BeforeQualification.CurrencyHash=JAC_AbsorptionGameplay.Canonical(TEST.BeforeQualification.CurrencyPoint)
TEST.BeforeQualification.CurrencyPrefixHash=JAC_AbsorptionGameplay.Canonical(ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows)
"#
        );
        for mutation in [
            "", "TEST.BeforeQualification.CurrencyCursor=0",
            "ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows[1].Gold=99999",
            "ExposedMembers.JAC_OfficialProductCurrencySnapshots.Rows[3]={Turn=5,Gold=10000,Faith=9980,Sequence=3}",
        ] {
            let result=run(&format!("{stale}\n{mutation}"),
                "assert(ObserveDebit(t.BeforeQualification,Product(),OFFER.ID),'old spending cannot prove a new debit')")?;
            assert!(result.is_err(),"stale history/no actual debit {mutation}: {result:?}");
        }
        Ok(())
    }

    #[test]
    fn partial_settlement_and_starting_survive_finite_wait_reload_and_next_turn() -> Result<()> {
        let lua = Lua::new();
        lua.load(WORLD).exec()?;
        lua.load(format!(r#"
{QUALIFICATION_WORLD}
{ACCEPTED_WORLD}
REQUEST.Status='STARTING';REQUEST.StartTurn=nil;VIEW.Status='STARTING'
REGISTER.Done=false;ESCROW.Status='COLLECTING';ESCROW.CollectFaith.Done=false;FAITH=10000
PROPS.JAC_ABSORPTION_FLOW_V1={{Version=1,Flows={{['normal-flow']={{ID='normal-flow',From=2,To=0,Mode='SMALL_STATE',Request=copy(REQUEST.Plan),Entries={{{{ID='native-asset',Rolls={{P1_ESCAPE={{Raw=123,Used=123,Label='saved-first-roll'}}}}}}}},Phase1Complete=false}}}}}}
"#)).exec()?;
        lua.load(format!("{P1}\n{CORE}\n{QUALIFICATION_TURN}"))
            .exec()?;
        lua.load("assert(not PROPS.JAC_OFFICIAL_PRODUCT_ACCEPTANCE_V1.NativeDebits[OFFER.ID]);assert(PROPS.JAC_OFFICIAL_PRODUCT_ACCEPTANCE_V1.DebitParts[OFFER.ID].Gold)").exec()?;
        for _ in 0..5 {
            lua.load(format!("{P1}\n{CORE}\n{OBSERVE_PENDING}"))
                .exec()?;
        }
        assert!(lua
            .load(format!("{P1}\n{CORE}\n{QUALIFIED}"))
            .exec()
            .is_err());
        // Restore detached observations from the same saved test property.
        lua.load("ExposedMembers.JAC_OfficialProductCurrencySnapshots=JAC_AbsorptionGameplay.Rules.Copy(PROPS.JAC_OFFICIAL_PRODUCT_ACCEPTANCE_V1.CurrencyHistory);JAC_OFFICIAL_PENDING_READS=nil").exec()?;
        lua.load(format!("{P1}\n{CORE}\n{RESTORE_OBSERVATION}"))
            .exec()?;
        lua.load(
            r#"
TURN=6;REQUEST.Status='STARTED';REQUEST.StartTurn=6;VIEW.Status='STARTED'
REGISTER.Done=true;ESCROW.Status='HELD';ESCROW.CollectFaith.Done=true
GOLD,FAITH=10005,9980 -- later gold income must not invalidate the earlier observed gold leg
PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].Phase1Complete=true
"#,
        )
        .exec()?;
        lua.load(format!("{P1}\n{CORE}\n{QUALIFICATION_TURN}"))
            .exec()?;
        lua.load(format!("{P1}\n{CORE}\n{QUALIFIED}")).exec()?;
        // Frozen decisions and already drawn rolls remain immutable even
        // though normal completion and newly drawn later-stage rolls may advance.
        for mutation in [
            "REQUEST.Decision.Turn=6",
            "REQUEST.StartTurn=4",
            "PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].Entries[1].Rolls.P1_ESCAPE.Used=999",
            "PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].Entries={}",
        ] {
            let probe = Lua::new();
            probe.load(WORLD).exec()?;
            probe
                .load(format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}"))
                .exec()?;
            probe.load("PROPS.JAC_ABSORPTION_FLOW_V1={Version=1,Flows={['normal-flow']={ID='normal-flow',From=2,To=0,Mode='SMALL_STATE',Phase1Complete=true,Request=JAC_AbsorptionGameplay.Rules.Copy(REQUEST.Plan),Entries={{ID='native-asset',Rolls={P1_ESCAPE={Used=123}}}}}}}").exec()?;
            probe.load(format!("{P1}\n{CORE}\nSavedAdmission(OFFER,REQUEST);PreservePending(REQUEST);Game:SetProperty(KEY,t)")).exec()?;
            probe.load(mutation).exec()?;
            assert!(
                probe
                    .load(format!(
                        "{P1}\n{CORE}\nSavedAdmission(OFFER,REQUEST);PreservePending(REQUEST)"
                    ))
                    .exec()
                    .is_err(),
                "{mutation}"
            );
        }
        Ok(())
    }

    #[test]
    fn only_a_supported_persisted_native_flow_journal_freezes_retry_evidence() -> Result<()> {
        let lua = Lua::new();
        lua.load(WORLD).exec()?;
        lua.load(format!(
            r#"
{QUALIFICATION_WORLD}
{ACCEPTED_WORLD}
REQUEST.Status='STARTING';REQUEST.StartTurn=nil;VIEW.Status='STARTING'
PROPS.JAC_ABSORPTION_FLOW_V1=nil
REQUEST.Decision.Requirements[1].Met=false
"#
        ))
        .exec()?;
        lua.load(format!("{P1}\n{CORE}\nassert(not SavedAdmission(OFFER,REQUEST));PreservePending(REQUEST);assert(not t.FrozenEvidence);Game:SetProperty(KEY,t)")).exec()?;
        // Normal retry can replace both the preflight plan and its decision.
        lua.load(
            r#"
TURN=6;REQUEST.Plan.FreshMarker='re-read-on-turn-six';REQUEST.Decision.Turn=6
REQUEST.Decision.Requirements[1].Met=true
"#,
        )
        .exec()?;
        lua.load(format!("{P1}\n{CORE}\nassert(not SavedAdmission(OFFER,REQUEST));PreservePending(REQUEST);assert(not t.FrozenEvidence)")).exec()?;
        // The real native journal, with its immutable Request and normal v1
        // identity, is the first boundary at which frozen evidence may be stored.
        lua.load("PROPS.JAC_ABSORPTION_FLOW_V1={Version=1,Flows={['normal-flow']={ID='normal-flow',From=2,To=0,Mode='SMALL_STATE',Request=JAC_AbsorptionGameplay.Rules.Copy(REQUEST.Plan),Entries={{ID='asset',Rolls={P1_ESCAPE={Used=123}}}},Phase1Complete=false}}}").exec()?;
        lua.load("REGISTER.Done=false;REGISTER.LastError='native phase one pending'")
            .exec()?;
        lua.load(format!("{P1}\n{CORE}\nassert(SavedAdmission(OFFER,REQUEST));PreservePending(REQUEST);assert(t.FrozenEvidence[REQUEST.FlowID]);assert(ObserveDebit(t.BeforeQualification,Product(),OFFER.ID));t.PendingSnapshot=Snapshot();t.PendingHash=A.Canonical(t.PendingSnapshot);Game:SetProperty(KEY,t)")).exec()?;
        // The queued registration may complete during normal reload publication;
        // the original receipt checkpoint and journal-frozen decision survive.
        lua.load("REQUEST.Status='STARTED';REQUEST.StartTurn=6;VIEW.Status='STARTED';REGISTER.Done=true;REGISTER.LastError=nil;PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].Phase1Complete=true").exec()?;
        lua.load(format!("{P1}\n{CORE}\n{RESTORE_OBSERVATION}"))
            .exec()?;
        lua.load("assert(PROPS.JAC_OFFICIAL_PRODUCT_ACCEPTANCE_V1.ReplayBaseline.Product.Operations['0:ai-register:2'].Done==false)").exec()?;
        lua.load("REQUEST.Decision.Turn=5").exec()?;
        assert!(lua
            .load(format!(
                "{P1}\n{CORE}\nSavedAdmission(OFFER,REQUEST);PreservePending(REQUEST)"
            ))
            .exec()
            .is_err());
        for mutation in [
            "PROPS.JAC_ABSORPTION_FLOW_V1=nil",
            "PROPS.JAC_ABSORPTION_FLOW_V1.Version=2",
            "PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].Request=nil",
            "PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].ID='fake-flow'",
            "PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].Phase1Complete=false",
        ] {
            let result = run(
                &format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}\n{mutation}"),
                "SavedAdmission(OFFER,REQUEST)",
            )?;
            assert!(
                result.is_err(),
                "native journal missing/invalid {mutation}: {result:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn reload_keeps_every_completed_receipt_and_exact_budget_before_rebasing() -> Result<()> {
        for mutation in [
            "",
            "QUEUED.Done=true;QUEUED.LastError=nil",
            "STATE.Operations['0:ai-envoy:2:2']=nil",
            "THIRD.EnvoyReceipt.Done=false",
            "THIRD.EnvoyIdentity.SenderLeaderType='other'",
            "STATE.AiEnvoyTurns['0'].Operation='0:ai-envoy:2:1'",
            "STATE.AiEnvoyTurns['0'].Turn=6",
            "STATE.AiEnvoyTurns={}",
            "QUEUED.OfferIntent.Offer.ID='different-offer'",
        ] {
            let lua = Lua::new();
            lua.load(WORLD).exec()?;
            lua.load(format!(r#"
{QUALIFICATION_WORLD}
THIRD=copy(TEST.Second.Operation);THIRD.Nonce='ai-envoy:2:2';THIRD.Identity.ExpectedEnvoys=2
THIRD.EnvoyReceipt={{Before=30,After=29,ReceivedBefore=2,ReceivedAfter=3,ChargeDone=true,GrantIntent=true,GrantDone=true,Done=true}}
STATE.Operations['0:ai-envoy:2:2']=THIRD
STATE.AiEnvoyTurns['0']={{Turn=5,Operation='0:ai-envoy:2:2'}}
POOL,RECEIVED=29,3
QUEUED={{Identity={{Command='REGISTER',From=2,To=0}},Nonce='ai-register:2',Done=false,
 OfferIntent={{Offer={{ID='product:2:0:1',From=2,To=0}}}},LastError='preflight waiting'}}
STATE.Operations['0:ai-register:2']=QUEUED
"#)).exec()?;
            lua.load(format!("{P1}\n{CORE}\nt.PendingSnapshot=Snapshot();t.PendingHash=A.Canonical(t.PendingSnapshot);t.BeforeQualification=CurrencyOrigin(Snapshot());Game:SetProperty(KEY,t)")).exec()?;
            lua.load(mutation).exec()?;
            let result = lua
                .load(format!("{P1}\n{CORE}\n{RESTORE_OBSERVATION}"))
                .exec();
            assert_eq!(
                result.is_ok(),
                mutation.is_empty() || mutation.starts_with("QUEUED.Done=true"),
                "{mutation}: {result:?}"
            );
            if result.is_ok() {
                lua.load("assert(PROPS.JAC_OFFICIAL_PRODUCT_ACCEPTANCE_V1.ReplayBaseline.Product.Operations['0:ai-envoy:2:2']);assert(PROPS.JAC_OFFICIAL_PRODUCT_ACCEPTANCE_V1.ReplayHash==JAC_AbsorptionGameplay.Canonical(PROPS.JAC_OFFICIAL_PRODUCT_ACCEPTANCE_V1.PendingSnapshot))").exec()?;
                // The immutable baseline remains a validator on later turns;
                // recording a fresh native snapshot cannot erase receipt loss.
                lua.load("STATE.Operations['0:ai-envoy:2:2']=nil").exec()?;
                assert!(lua
                    .load(format!(
                        "{P1}\n{CORE}\nReceiptContinuity(t.ReplayBaseline.Product,Product(),false)"
                    ))
                    .exec()
                    .is_err());
            }
        }
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
                "{P1}\n{CORE}\nt.SettledFirst=Snapshot();t.PublicationReplays=2;Game:SetProperty(KEY,t)"
            ))
            .exec()?;
            lua.load(mutation).exec()?;
            let result = lua.load(format!("{P1}\n{CORE}\n{AFTER_LOAD}")).exec();
            assert_eq!(
                result.is_ok(),
                mutation.is_empty(),
                "{mutation}: {result:?}"
            );
        }
        Ok(())
    }

    const PROGRESS_WORLD: &str = r#"
PROPS.JAC_ABSORPTION_ADMISSION_V1={Version=2,Offers={[OFFER.ID]=copy(OFFER)},Requests={accepted=REQUEST}}
local flow=PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow']
flow.Turn=5;flow.Phase='APPLYING';flow.FinalRollsComplete=true
flow.Inventory={{ID='unit-4',Kind='UNIT',UnitID=4,UnitType=7}}
flow.RosterAtBegin={Leader={ID='leader',Owner=2}}
flow.Entries={{ID='unit-4',Kind='UNIT',Source={Owner=2,ID=4},UnitID=4,UnitType=7,
 AllowedActions={'INCORPORATE'},EscapeOutcome=nil,P1Chance=5000,
 Rolls={P1_ESCAPE={Raw=4321,Used=4321,Label='original-p1'},P3_SUCCESS={Raw=3210,Used=3210,Label='original-p3'}}}}
flow.AppliedPlan={ID='normal-flow',From=2,To=0,Mode='SMALL_STATE',Units={{ID=4,Action='INCORPORATE'}}}
TRANSFER={Transactions={['normal-flow']={ID='normal-flow',From=2,To=0,Mode='SMALL_STATE',Request=copy(flow.AppliedPlan),Status='WAITING_UI'}}}
JAC_AbsorptionGameplay.GetState=function() return copy(TRANSFER) end
JAC_AbsorptionFlowGameplay.Get=function(id) return copy(PROPS.JAC_ABSORPTION_FLOW_V1.Flows[id]) end
function CompleteNative(consume)
 local f=PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow']
 f.Phase='DONE';f.CompletedTurn=5;TRANSFER.Transactions['normal-flow'].Status='COMPLETE'
 if consume then ESCROW.Status='CONSUMED';ESCROW.Consumption={FlowID='normal-flow',From=2,To=0,CompletedTurn=5,
 Gold=100,Faith=20,LegacyRefunded={Gold=0,Faith=0},Policy='CONSUME_ON_VERIFIED_RESULT_V2'} end
end
"#;

    #[test]
    fn incomplete_phase_one_preserves_shared_inventory_across_saved_checkpoints() -> Result<()> {
        for complete in [false, true] {
            for mutation in [
                "f.ID='other-flow'",
                "f.From=1",
                "f.To=1",
                "f.Mode='MERGER'",
                "f.Turn=6",
                "f.Request.EntryConditionsMet=false",
                "f.RosterAtBegin={}",
                "f.Inventory={}",
                "e.ID='other-asset'",
                "e.Kind='ITEM'",
                "e.Name=nil",
                "e.Portrait=nil",
                "e.Source.Owner=1",
                "e.UnitID=5",
                "e.UnitType=8",
                "e.AllowedActions={}",
                "e.Abilities={}",
                "e.Leader=nil",
                "e.NativeWork=nil",
                "e.DormantClass=nil",
                "f.Entries={}",
            ] {
                for checkpoint in ["saved", "frozen"] {
                    let lua = Lua::new();
                    lua.load(WORLD).exec()?;
                    lua.load(format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}\n{PROGRESS_WORLD}\nlocal f=PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'];f.Phase1Complete=false;f.FinalRollsComplete=nil;f.Phase='CHOICE';local e=f.Entries[1];e.Name='original-name';e.Portrait='original-portrait';e.Abilities={{'original-ability'}};e.Leader=true;e.NativeWork={{Index=9,Owner=2}};e.DormantClass=3")).exec()?;
                    lua.load(format!("{P1}\n{CORE}\nt.PendingSnapshot=Snapshot();PreservePending(REQUEST);Game:SetProperty(KEY,t)")).exec()?;
                    lua.load(format!("local f=PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'];local e=f.Entries[1];f.Phase1Complete={complete};{mutation}")).exec()?;
                    let check = if checkpoint == "saved" {
                        "SavedProgress(t.PendingSnapshot,'initial inventory')"
                    } else {
                        "PreservePending(REQUEST)"
                    };
                    let error = lua
                        .load(format!("{P1}\n{CORE}\n{check}"))
                        .exec()
                        .expect_err(mutation)
                        .to_string();
                    // The frozen request boundary may catch identity/request
                    // mutations first; all remaining metadata uses one projection.
                    assert!(
                        error.contains("saved initial inventory evidence changed")
                            || error.contains("native Flow Request does not bind"),
                        "complete={complete}/{checkpoint}/{mutation}: {error}"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn partial_phase_one_accepts_completion_but_preserves_saved_results_and_all_rolls() -> Result<()>
    {
        for (mutation, reason) in [
            ("f.Phase1Complete=true;e.Rolls.P1_BRANCH={Raw=17,Used=17,Label='new-branch'};e.Rolls.P3_RETRY_ESCAPE={Raw=22,Used=22,Label='new-retry'}", None),
            ("e.P1Chance=0", Some("saved first-stage result changed: P1Chance")),
            ("e.P1Chance=nil", Some("saved first-stage result changed: P1Chance")),
            ("e.EscapeOutcome='RETURN'", Some("saved first-stage result changed: EscapeOutcome")),
            ("e.EscapeOutcome=nil", Some("saved first-stage result changed: EscapeOutcome")),
            ("e.Rolls.P1_ESCAPE.Used=1", Some("saved random roll changed: P1_ESCAPE")),
            ("e.Rolls.P3_SUCCESS=nil", Some("saved random roll changed: P3_SUCCESS")),
            ("e.Rolls.P3_RETRY_BRANCH.Raw=0", Some("saved random roll changed: P3_RETRY_BRANCH")),
        ] {
            let lua = Lua::new();
            lua.load(WORLD).exec()?;
            lua.load(format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}\n{PROGRESS_WORLD}\nlocal f=PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'];f.Phase1Complete=false;f.FinalRollsComplete=nil;f.Phase='CHOICE';local e=f.Entries[1];e.EscapeOutcome='ESCAPE';e.Rolls.P3_RETRY_BRANCH={{Raw=44,Used=44,Label='old-retry'}}")).exec()?;
            lua.load(format!("{P1}\n{CORE}\nt.PendingSnapshot=Snapshot();PreservePending(REQUEST);Game:SetProperty(KEY,t)")).exec()?;
            lua.load(format!("local f=PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'];local e=f.Entries[1];{mutation}")).exec()?;
            let result = lua.load(format!("{P1}\n{CORE}\nSavedProgress(t.PendingSnapshot,'partial first-stage');PreservePending(REQUEST)")).exec();
            if let Some(reason) = reason {
                let error = result.expect_err(mutation).to_string();
                assert!(error.contains(reason), "{mutation}: {error}");
            } else {
                result?;
                lua.load("local old=PROPS.JAC_OFFICIAL_PRODUCT_ACCEPTANCE_V1.PendingSnapshot.Flow.Flows['normal-flow'];assert(old.Phase1Complete==false);assert(old.Entries[1].Rolls.P1_BRANCH==nil)").exec()?;
            }
        }
        // Previously absent results can first appear during normal completion.
        let lua = Lua::new();
        lua.load(WORLD).exec()?;
        lua.load(format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}\n{PROGRESS_WORLD}\nlocal f=PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'];f.Phase1Complete=false;f.FinalRollsComplete=nil;f.Phase='CHOICE';f.Entries[1].P1Chance=nil;f.Entries[1].EscapeOutcome=nil")).exec()?;
        lua.load(format!("{P1}\n{CORE}\nt.PendingSnapshot=Snapshot();PreservePending(REQUEST);Game:SetProperty(KEY,t)")).exec()?;
        lua.load("local f=PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'];f.Phase1Complete=true;f.Entries[1].P1Chance=5000;f.Entries[1].EscapeOutcome='ESCAPE'").exec()?;
        lua.load(format!("{P1}\n{CORE}\nSavedProgress(t.PendingSnapshot,'new first-stage results');PreservePending(REQUEST)")).exec()?;
        Ok(())
    }

    #[test]
    fn saved_collection_accepts_issue_and_done_progress_with_actual_balances() -> Result<()> {
        for advance in [
            "ESCROW.CollectGold.Issued=true;ESCROW.CollectFaith.Issued=true",
            "ESCROW.CollectGold.Issued=true;ESCROW.CollectGold.Done=true;GOLD=10000",
            "ESCROW.CollectGold.Issued=true;ESCROW.CollectGold.Done=true;ESCROW.CollectFaith.Issued=true;ESCROW.CollectFaith.Done=true;ESCROW.Status='HELD';GOLD=10000;FAITH=9980",
        ] {
            let lua = Lua::new();
            lua.load(WORLD).exec()?;
            lua.load(format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}\n{PROGRESS_WORLD}\nESCROW.Status='COLLECTING';ESCROW.CollectGold.Done=nil;ESCROW.CollectFaith.Done=nil;GOLD=10100;FAITH=10000")).exec()?;
            lua.load(format!("{P1}\n{CORE}\nt.PendingSnapshot=Snapshot();Game:SetProperty(KEY,t)")).exec()?;
            lua.load(advance).exec()?;
            lua.load(format!("{P1}\n{CORE}\nSavedProgress(t.PendingSnapshot,'collection progression')")).exec()?;
            lua.load("local original=PROPS.JAC_OFFICIAL_PRODUCT_ACCEPTANCE_V1.PendingSnapshot;assert(original.Product.Escrows[OFFER.ID].CollectGold.Issued==nil);assert(original.Gold==10100);assert(original.Faith==10000)").exec()?;
        }
        Ok(())
    }

    #[test]
    fn saved_collection_rejects_stage_regression_intent_changes_and_omitted_debits() -> Result<()> {
        for (initial, mutation, reason) in [
            (
                "ESCROW.CollectGold.Issued=true",
                "ESCROW.CollectGold.Issued=nil",
                "stage regressed: Issued",
            ),
            (
                "ESCROW.CollectFaith.Issued=true",
                "ESCROW.CollectFaith.Issued=false",
                "stage regressed: Issued",
            ),
            (
                "ESCROW.CollectGold.Issued=true;ESCROW.CollectGold.Done=true;GOLD=10000",
                "ESCROW.CollectGold.Done=nil",
                "stage regressed: Done",
            ),
            (
                "",
                "ESCROW.CollectGold.Issued='true'",
                "invalid native collection stage: Issued",
            ),
            (
                "",
                "ESCROW.CollectFaith.Done=1",
                "invalid native collection stage: Done",
            ),
            (
                "",
                "ESCROW.CollectGold=nil",
                "actual native collection intent disappeared",
            ),
            (
                "",
                "ESCROW.CollectGold.Before=10101",
                "actual native collection intent changed",
            ),
            (
                "",
                "ESCROW.CollectFaith.After=9981",
                "actual native collection intent changed",
            ),
            (
                "",
                "ESCROW.CollectGold.Payer=1",
                "actual native collection intent changed",
            ),
            ("", "ESCROW.To=1", "escrow identity changed"),
            ("", "ESCROW.Gold=99", "escrow identity changed"),
            (
                "",
                "ESCROW.CollectGold.Issued=true;ESCROW.CollectGold.Done=true",
                "payer changed beyond newly observed native collection",
            ),
            (
                "",
                "ESCROW.CollectFaith.Issued=true;ESCROW.CollectFaith.Done=true",
                "payer changed beyond newly observed native collection",
            ),
            (
                "",
                "GOLD=GOLD-1",
                "payer changed beyond newly observed native collection",
            ),
        ] {
            let lua = Lua::new();
            lua.load(WORLD).exec()?;
            lua.load(format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}\n{PROGRESS_WORLD}\nESCROW.Status='COLLECTING';ESCROW.CollectGold.Done=nil;ESCROW.CollectFaith.Done=nil;GOLD=10100;FAITH=10000\n{initial}")).exec()?;
            lua.load(format!(
                "{P1}\n{CORE}\nt.PendingSnapshot=Snapshot();Game:SetProperty(KEY,t)"
            ))
            .exec()?;
            lua.load(mutation).exec()?;
            let error = lua
                .load(format!(
                    "{P1}\n{CORE}\nSavedProgress(t.PendingSnapshot,'collection progression')"
                ))
                .exec()
                .expect_err(mutation)
                .to_string();
            assert!(error.contains(reason), "{mutation}: {error}");
        }
        Ok(())
    }

    #[test]
    fn qualified_reload_accepts_legal_apply_completion_and_consumption_without_repayment(
    ) -> Result<()> {
        for (mutation,ok) in [
            ("",true), ("CompleteNative(false)",true), ("CompleteNative(true)",true),
            ("CompleteNative(true);PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].Entries[1].Rolls.P1_ESCAPE.Used=7",false),
            ("CompleteNative(true);PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].Entries[1].Rolls.P3_SUCCESS.Raw=99",false),
            ("CompleteNative(true);PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].Inventory={}",false),
            ("CompleteNative(true);PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].Entries[1].EscapeOutcome='ESCAPE'",false),
            ("CompleteNative(true);PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].Request.ProductDeposit.Gold=0",false),
            ("CompleteNative(true);ESCROW.CollectGold.Before=10101",false),
            ("CompleteNative(true);ESCROW.Consumption.Gold=0",false),
            ("CompleteNative(true);ESCROW.Consumption.LegacyRefunded.Gold=100",false),
            ("CompleteNative(true);ESCROW.Consumption.CompletedTurn=4",false),
            ("CompleteNative(true);TRANSFER.Transactions['normal-flow'].Status='STAGED'",false),
            ("CompleteNative(true);TRANSFER.Transactions['normal-flow'].Request.Units={}",false),
            ("CompleteNative(true);STATE.Operations[OPKEY]=nil",false),
            ("CompleteNative(true);OP.EnvoyReceipt.GrantDone=false",false),
            ("CompleteNative(true);STATE.AiEnvoyTurns['0'].Turn=6",false),
            ("CompleteNative(true);REQUEST.Decision.Requirements[1].Met=false",false),
            ("CompleteNative(true);PROPS.JAC_ABSORPTION_FLOW_V1.Version=2",false),
            ("CompleteNative(true);GOLD=GOLD+100",false),
            ("CompleteNative(true);FAITH=FAITH-20",false),
            ("CompleteNative(true);POOL=POOL-1",false),
            ("CompleteNative(true);RECEIVED=RECEIVED+1",false),
        ] {
            let lua=Lua::new();lua.load(WORLD).exec()?;
            lua.load(format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}\n{PROGRESS_WORLD}")).exec()?;
            lua.load(format!("{P1}\n{CORE}\nt.Qualified=Snapshot();Game:SetProperty(KEY,t)")).exec()?;
            lua.load(mutation).exec()?;
            let result=lua.load(format!("{P1}\n{CORE}\nSavedProgress(t.Qualified,'qualified original continuation')")).exec();
            assert_eq!(result.is_ok(),ok,"{mutation}: {result:?}");
        }
        Ok(())
    }

    #[test]
    fn pending_and_terminal_checkpoints_preserve_original_evidence_across_legal_resume(
    ) -> Result<()> {
        let lua = Lua::new();
        lua.load(WORLD).exec()?;
        lua.load(format!(
            "{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}\n{PROGRESS_WORLD}"
        ))
        .exec()?;
        lua.load(format!("{P1}\n{CORE}\nassert(ObserveDebit(t.BeforeQualification,Product(),OFFER.ID));PreservePending(REQUEST);t.PendingSnapshot=Snapshot();t.PendingHash=A.Canonical(t.PendingSnapshot);Game:SetProperty(KEY,t)")).exec()?;
        lua.load("CompleteNative(true)").exec()?;
        lua.load(format!("{P1}\n{CORE}\n{RESTORE_OBSERVATION}"))
            .exec()?;
        lua.load("assert(PROPS.JAC_OFFICIAL_PRODUCT_ACCEPTANCE_V1.ReplayBaseline.Flow.Flows['normal-flow'].Phase=='APPLYING')").exec()?;
        lua.load(format!(
            "{P1}\n{CORE}\nt.Terminal=Snapshot();Game:SetProperty(KEY,t)"
        ))
        .exec()?;
        for mutation in [
            "PROPS.JAC_ABSORPTION_FLOW_V1.Flows['normal-flow'].Phase='APPLYING'",
            "ESCROW.Consumption.Faith=19",
            "TRANSFER.Transactions['normal-flow'].ExtraMutation='changed complete receipt'",
        ] {
            let probe = Lua::new();
            probe.load(WORLD).exec()?;
            probe.load(format!("{QUALIFICATION_WORLD}\n{ACCEPTED_WORLD}\n{PROGRESS_WORLD}\nCompleteNative(true)")).exec()?;
            probe
                .load(format!(
                    "{P1}\n{CORE}\nt.Terminal=Snapshot();Game:SetProperty(KEY,t)"
                ))
                .exec()?;
            probe.load(mutation).exec()?;
            assert!(
                probe
                    .load(format!(
                        "{P1}\n{CORE}\nSavedProgress(t.Terminal,'terminal receipt')"
                    ))
                    .exec()
                    .is_err(),
                "{mutation}"
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
                "{P1}\n{CORE}\nt.SettledFirst=Snapshot();Game:SetProperty(KEY,t)"
            ))
            .exec()?;
            lua.load(format!(
                "ExposedMembers={{JAC_OfficialProductPublicationObservation={observation}}}"
            ))
            .exec()?;
            let result = lua
                .load(format!("{P1}\n{CORE}\n{ASSERT_PUBLICATIONS}"))
                .exec();
            assert_eq!(
                result.is_ok(),
                observation == "{From=2,To=0,Turn=3,Count=2}",
                "{observation}: {result:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn scene_has_pending_and_complete_round_trips_and_single_actual_turn_steps() -> Result<()> {
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
        assert_eq!(trips, 3);
        assert_eq!(turns, 13); // one native founding turn, two measured sends, ten qualification turns
        let rendered = serde_json::to_string(&scene)?;
        assert!(!rendered.contains("test_mode"));
        assert!(rendered.contains("qualified original evidence"));
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
                check: true,
                frontend_probe: false
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
