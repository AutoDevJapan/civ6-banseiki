use anyhow::Context;
use civ6_data_rs::{absorption_product, Repository};
use mlua::Lua;
use std::path::Path;

fn city_read_provider() -> anyhow::Result<Lua> {
    let lua = Lua::new();
    lua.load(r#"
local function event()
  local handlers={}
  return {Add=function(fn) handlers[#handlers+1]=fn end,
    Emit=function() for _,fn in ipairs(handlers) do fn() end end}
end
Events={LoadScreenClose=event(),GameCoreEventPublishComplete=event(),LocalPlayerTurnBegin=event()}
include=function() end;ExposedMembers={};PlayerOperations={};UI={};PlayerManager={GetAlive=function() return {} end}
GameInfo={Districts={[7]={DistrictType='DISTRICT_REAL'}},GreatWorkSlotTypes={}}
GETTERS={};FAULT=nil;PHYSICAL=true
local function read(kind,value)
  GETTERS[kind]=(GETTERS[kind] or 0)+1
  if FAULT=='getter' then error('native getter threw') end
  return value
end
REAL={Owner=1,ID=196610,X=129,Y=35,
  GetOwner=function(s) return s.Owner end,GetID=function(s) return s.ID end,
  GetX=function(s) return s.X end,GetY=function(s) return s.Y end,
  GetReligion=function() return {GetReligionsInCity=function() return read('RELIGIONS',{{Religion=5,Followers=9}}) end} end,
  GetDistricts=function() return {Members=function() return ipairs(read('DISTRICTS',{
    {GetType=function() return 7 end,GetX=function() return 128 end,GetY=function() return 35 end}})) end} end,
  GetBuildings=function() return {
    GetNumGreatWorkSlots=function(_,b) assert(b==11);return read('SLOTS',2) end,
    GetGreatWorkInSlot=function(_,b,s) assert(b==11 and (s==0 or s==1));return read('WORK',s==0 and 77 or -1) end,
    GetGreatWorkTypeFromIndex=function(_,i) assert(i==77);return read('NATIVE_TYPE',4) end,
    GetCreatorNameFromIndex=function() return read('CREATOR','real creator') end,
    GetTurnFromIndex=function() return read('CREATED_TURN',3) end} end,
  GetCulturalIdentity=function() return {GetLoyalty=function() return read('LOYALTY',72) end} end}
local old={GetOwner=function() return 1 end,GetID=function() return 131073 end,
  GetX=function() return 119 end,GetY=function() return 42 end}
Players={[1]={GetCities=function() return {FindID=function() return nil end,Members=function() return ipairs({old}) end} end}}
CityManager={GetCityAt=function(x,y) assert(x==129 and y==35);if PHYSICAL then return REAL end end}
Game={GetCurrentGameTurn=function() return 10 end,GetHeroesManager=function() return {} end,
  GetGreatWorkPlayer=function(i) assert(i==77);return read('CREATED_BY',6) end,GetProperty=function() end}
"#).exec()?;
    lua.load(std::fs::read_to_string(
        Repository::checked_in()
            .root
            .join("poc/JAC_PoC_Absorption/UI/JAC_Absorption_HeroKeys.lua"),
    )?)
    .exec()?;
    Ok(lua)
}

#[test]
fn city_reads_use_physical_identity_and_actual_getters_with_stale_collections() -> anyhow::Result<()>
{
    let lua = city_read_provider()?;
    lua.load(r#"
local r=ExposedMembers.JAC_HeroReadAPI
local function city(method,b,s) return r.ReadCity(1,196610,method,b,s,nil,129,35) end
assert(city('SLOTS',11)==2 and city('WORK',11,0)==77 and city('WORK',11,1)==-1)
assert(city('RELIGIONS')[1].Followers==9)
assert(city('DISTRICTS')[1].Type=='DISTRICT_REAL' and city('LOYALTY')==72)
local work=r.ReadNativeWork(1,196610,77,129,35)
assert(work.Type==4 and work.CreatorName=='real creator' and work.TurnCreated==3 and work.CreatedByPlayer==6)
for _,kind in ipairs({'SLOTS','WORK','RELIGIONS','DISTRICTS','LOYALTY','NATIVE_TYPE','CREATOR','CREATED_TURN','CREATED_BY'}) do
  assert(GETTERS[kind]>0,'physical getter was bypassed: '..kind)
end
for _,field in ipairs({'Owner','ID','X','Y'}) do
  local original=REAL[field];REAL[field]=original+1
  local count=GETTERS.SLOTS
  local ok,why=pcall(city,'SLOTS',11)
  assert(not ok and tostring(why):find('UI city cache is not ready',1,true),field..' mismatch accepted')
  assert(GETTERS.SLOTS==count,'getter ran on a different city')
  assert(not pcall(r.ReadNativeWork,1,196610,77,129,35),'NativeWork accepted '..field..' mismatch')
  REAL[field]=original
end
for _,fault in ipairs({'missing','player','api','getter'}) do
  local player,manager=Players[1],CityManager
  if fault=='missing' then PHYSICAL=false
  elseif fault=='player' then Players[1]=nil
  elseif fault=='api' then CityManager={}
  else FAULT='getter' end
  -- Rebind on LoadScreenClose, just as a retained UI context does after loading.
  Events.LoadScreenClose.Emit()
  r=ExposedMembers.JAC_HeroReadAPI
  local ok,why=pcall(city,'SLOTS',11)
  assert(not ok and tostring(why):find('UI city cache is not ready',1,true),fault..' did not wait')
  assert(not pcall(r.ReadNativeWork,1,196610,77,129,35),fault..' NativeWork did not wait')
  Players[1],CityManager,PHYSICAL,FAULT=player,manager,true,nil
  Events.LoadScreenClose.Emit();r=ExposedMembers.JAC_HeroReadAPI
  assert(city('SLOTS',11)==2)
end
"#).exec()?;
    Ok(())
}

#[test]
fn load_screen_rebinds_city_player_and_game_apis_before_real_publication() -> anyhow::Result<()> {
    let lua = city_read_provider()?;
    lua.load(r#"
local old=ExposedMembers.JAC_HeroReadAPI
local oldCity=REAL
REAL=setmetatable({GetBuildings=function() return {
  GetNumGreatWorkSlots=function() return 3 end,
  GetGreatWorkInSlot=function(_,b,s) assert(s>=0 and s<3);return s==0 and 77 or -1 end,
  GetGreatWorkTypeFromIndex=function() return 5 end,
  GetCreatorNameFromIndex=function() return 'new creator' end,
  GetTurnFromIndex=function() return 4 end} end},{__index=oldCity})
Players={[1]={GetCities=function() return {FindID=function() return nil end,Members=function() return ipairs({}) end} end,
  GetUnits=function() return {Members=function() return ipairs({
    {GetID=function() return 7 end,GetSpreadCharges=function() return 1 end}}) end} end}}
CityManager={GetCityAt=function(x,y) assert(x==129 and y==35);return REAL end}
Game={GetCurrentGameTurn=function() return 11 end,GetHeroesManager=function() return {} end,
  GetGreatWorkPlayer=function() return 8 end,GetProperty=function() end}
local request={Owner=1,ID=196610,X=129,Y=35,Method='SLOTS',Building=11,Works=true}
ExposedMembers.JAC_Absorption={GetState=function() return {Transactions={t={ID='t',AwaitingFinalUI=true,
  FinalUI={Token='new-city',Turn=11,Reads={request}}}}} end,GetUIPhase=function() end}
oldCity.GetBuildings=function() error('old UI city was retained') end
Events.LoadScreenClose.Emit()
local current=ExposedMembers.JAC_HeroReadAPI
assert(current~=old and current.GetObservation('new-city').Published)
assert(current.ReadCity(1,196610,'SLOTS',11,nil,nil,129,35)==3)
assert(current.ReadUnit(1,7,'SPREAD_CHARGES',11)==1)
local work=current.ReadNativeWork(1,196610,77,129,35)
assert(work.Type==5 and work.CreatedByPlayer==8 and work.CreatorName=='new creator' and work.TurnCreated==4)
Game.GetHeroesManager=function() return nil end
Events.LoadScreenClose.Emit()
assert(ExposedMembers.JAC_HeroReadAPI==nil,'load retained an unavailable prior reader and its publications')
Game.GetHeroesManager=function() return {} end
Events.LoadScreenClose.Emit()
assert(ExposedMembers.JAC_HeroReadAPI.ReadCity(1,196610,'SLOTS',11,nil,nil,129,35)==3)
"#).exec()?;
    Ok(())
}

#[test]
fn compatible_native_slots_read_physical_city_and_wait_for_every_identity_mismatch(
) -> anyhow::Result<()> {
    let lua = city_read_provider()?;
    lua.load(r#"
GameInfo.Buildings=function() local once=false;return function()
  if not once then once=true;return {Index=11,BuildingType='BUILDING_REAL'} end end end
GameInfo.GreatWorkSlotTypes={[0]={GreatWorkSlotType='SLOT_REAL'}}
GameInfo.GreatWork_ValidSubTypes=function() local once=false;return function()
  if not once then once=true;return {GreatWorkObjectType='OBJECT_REAL',GreatWorkSlotType='SLOT_REAL'} end end end
local buildings=REAL.GetBuildings
REAL.GetBuildings=function()
  local b=buildings()
  b.HasBuilding=function(_,id) return id==11 end;b.IsPillaged=function() return false end
  b.GetGreatWorkSlotType=function(_,id,slot) assert(id==11);return 0 end
  return b
end
CACHED={ID=196610,X=129,Y=35,GetID=function(s) return s.ID end,
  GetX=function(s) return s.X end,GetY=function(s) return s.Y end,
  GetBuildings=function() error('stale collection building getter used') end}
Players[1].GetCities=function() return {Members=function() return ipairs({CACHED}) end,
  FindID=function() return CACHED end} end
local function query() return ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots(1,'OBJECT_REAL') end
local slots=query()
assert(#slots==1 and slots[1].Owner==1 and slots[1].City==196610 and slots[1].X==129
  and slots[1].Y==35 and slots[1].Slot==1 and slots[1].Building==11)
for _,field in ipairs({'Owner','ID','X','Y'}) do
  local original=REAL[field];REAL[field]=original+1
  local count=GETTERS.SLOTS
  local ok,why=pcall(query)
  assert(not ok and tostring(why):find('UI city cache is not ready',1,true),field..' stale candidate accepted')
  assert(GETTERS.SLOTS==count,'getter ran on a mismatched physical city')
  REAL[field]=original
end
local request={Token='slots:query',Reads={{Owner=1,Method='NATIVE_SLOTS',ObjectType='OBJECT_REAL'}}}
ExposedMembers.JAC_Absorption={GetState=function() return {Transactions={t={ID='t',CityReadUI=request}}} end,
  GetUIPhase=function() end}
CACHED.ID=16
Events.GameCoreEventPublishComplete.Emit()
assert(ExposedMembers.JAC_HeroReadAPI.GetObservation(request.Token)==nil,'old collection acquired publication')
CACHED.ID=196610
Events.GameCoreEventPublishComplete.Emit()
assert(ExposedMembers.JAC_HeroReadAPI.GetObservation(request.Token).Published)
for _,fault in ipairs({'missing','player','api','getter'}) do
  local player,manager=Players[1],CityManager
  if fault=='missing' then PHYSICAL=false elseif fault=='player' then Players[1]=nil
  elseif fault=='api' then CityManager={} else FAULT='getter' end
  Events.LoadScreenClose.Emit()
  local ok,why=pcall(query)
  assert(not ok and tostring(why):find('UI city cache is not ready',1,true),fault..' did not wait')
  assert(ExposedMembers.JAC_HeroReadAPI.GetObservation(request.Token)==nil)
  Players[1],CityManager,PHYSICAL,FAULT=player,manager,true,nil
  Events.LoadScreenClose.Emit();assert(#query()==1)
end
"#).exec()?;
    Ok(())
}

#[test]
fn native_read_waits_report_missing_identity_without_changing_getter_results() -> anyhow::Result<()>
{
    let lua = Lua::new();
    lua.load(r#"
local function event() return {Add=function() end} end
Events={LoadScreenClose=event(),GameCoreEventPublishComplete=event(),LocalPlayerTurnBegin=event()}
include=function() end
ExposedMembers={};PlayerOperations={};UI={};GameInfo={}
 local city={GetOwner=function() return 1 end,GetID=function() return 4 end,GetX=function() return 12 end,GetY=function() return 13 end}
local cities={FindID=function(_,id) if id==4 then return city end end,
  Members=function() return ipairs({city}) end}
local unit={GetID=function() return 7 end,GetSpreadCharges=function() return 2 end}
Players={[1]={GetCities=function() return cities end,GetUnits=function() return {Members=function() return ipairs({unit}) end} end}}
TURN=10;Game={GetCurrentGameTurn=function() return TURN end,GetHeroesManager=function() return {} end}
READ_LINES={};print=function(text) READ_LINES[#READ_LINES+1]=text end
"#).exec()?;
    lua.load(std::fs::read_to_string(
        Repository::checked_in()
            .root
            .join("poc/JAC_PoC_Absorption/UI/JAC_Absorption_HeroKeys.lua"),
    )?)
    .exec()?;
    lua.load(r#"
local r=ExposedMembers.JAC_HeroReadAPI
assert(r.ReadUnit(1,7,'SPREAD_CHARGES',10)==2 and #READ_LINES==0)
assert(r.ReadUnit(1,7,'SPREAD_CHARGES',9)==nil)
assert(r.ReadUnit(1,7,'SPREAD_CHARGES',9)==nil and #READ_LINES==1)
assert(READ_LINES[1]:find('requestedTurn=9 uiTurn=10 reason=TURN_MISMATCH',1,true))
assert(r.ReadUnit(1,7,'SPREAD_CHARGES',10)==2)
assert(r.ReadUnit(1,7,'SPREAD_CHARGES',9)==nil and #READ_LINES==2,'successful read did not reset wait tracing')
assert(r.ReadUnit(1,99,'SPREAD_CHARGES',10)==nil and #READ_LINES==3)
assert(READ_LINES[3]:find('owner=1 id=99',1,true) and READ_LINES[3]:find('UNIT_MISSING',1,true))
assert(r.ReadUnit(9,7,'SPREAD_CHARGES',10)==nil and #READ_LINES==4)
assert(READ_LINES[4]:find('PLAYER_MISSING',1,true))
for _,kind in ipairs({'RELIGIONS','DISTRICTS','LOYALTY','NATIVE_WORK'}) do
  local ok,why
  if kind=='NATIVE_WORK' then ok,why=pcall(r.ReadNativeWork,1,99,0)
  else ok,why=pcall(r.ReadCity,1,99,kind) end
  assert(not ok and tostring(why):find('UI city cache is not ready',1,true))
  assert(tostring(why):find('owner=1 id=99 method='..kind,1,true)
    and tostring(why):find('known=4@12,13',1,true),'missing city read lost requested or published identity')
end
"#).exec()?;
    Ok(())
}

const SCREEN: &str = r#"
local function event()
  local handlers={}
  return {Add=function(fn) handlers[#handlers+1]=fn end,
    Remove=function(fn) for i=#handlers,1,-1 do if handlers[i]==fn then table.remove(handlers,i) end end end,
    Call=function(...) local snapshot={};for i,fn in ipairs(handlers) do snapshot[i]=fn end;for _,fn in ipairs(snapshot) do fn(...) end end}
end
local function control(name)
  return {Name=name,Text='',Hidden=false,Disabled=false,Callback=nil,Children={},
    SetText=function(s,t) s.Text=t end,SetString=function(s,t) s.Text=t end,
    SetHide=function(s,v) s.Hidden=v end,SetDisabled=function(s,v) s.Disabled=v end,
    SetSelected=function(s,v) s.Selected=v end,SetIcon=function(s,v) s.Icon=v end,
    SetColor=function(s,v) s.Color=v end,SetTexture=function(s,...) s.Texture={...} end,
    SetToolTipString=function(s,v) s.Tooltip=v end,
    CalculateSize=function() end,DoAutoSize=function() end,
    RegisterCallback=function(s,_,fn) s.Callback=fn end}
end
Controls={}
for _,name in ipairs({'CityStateStack','QuestsStack','NoneMet','ListOfCityStates','SingleCityState','CityStateName','CityStateTypeIcon',
  'TypeValue','PatronValue','EnvoysSentValue','QuestsValue','EnvoysSentArea','InfluenceArea','RelationshipsArea','QuestsArea',
  'QuestsButton','QuestsScroll','ReportArea','SingleViewStack','LevyMilitaryButton','PeaceWarButton','EnvoysSentButton','InfluencedByButton','RelationshipsButton','CloseListButton','CloseBackButton',
  'InfluencedByValue','RelationshipsButtonStack','DiplomacyPip','CityStateIconStack'}) do
  Controls[name]=control(name)
end
-- Expansion2 CityStates.xml gives these tab areas Hidden=1, but the report
-- fields, relationship icons, diplomacy pip and sidebar have no Hidden flag.
for _,name in ipairs({'InfluenceArea','RelationshipsArea','QuestsArea'}) do Controls[name].Hidden=true end
Controls.DiplomacyPip.Texture={'Diplomacy_RelationshipPips'}
-- CityStates.xml initializes this label once; native ViewCityState never resets it.
Controls.LevyMilitaryButton.Text='LOC_CITY_STATES_LEVY_MILITARY_BUTTON'
Controls.CityStateScroll={GetSizeX=function() return 500 end}
UIManager={GetScreenSizeVal=function() return 1600,900 end}
INSTANCES={};ALL_MANAGERS={}
InstanceManager={new=function(_,template,root,parent)
  assert(template=='QuestInstance' and root=='Top' or template=='CityStateRowInstance' and root=='CityStateBase'
    or template=='RelationshipIcon' and root=='Background' or template=='CityStateIconInstance' and root=='Top','official instance root required')
  local im=setmetatable({Rows={},Available={},m_InstanceName=template,m_RootControlName=root,m_ParentControl=parent},{__index=InstanceManager})
  INSTANCES[template=='CityStateRowInstance' and parent==Controls.QuestsStack and 'MinorActions' or template]=im
  ALL_MANAGERS[#ALL_MANAGERS+1]=im
  return im
end,
ResetInstances=function(im)
  -- Real ResetInstances hides allocated roots and pools their unchanged tables;
  -- another manager's rows in the same parent remain allocated and visible.
  for _,row in ipairs(im.Rows) do row[im.m_RootControlName]:SetHide(true);im.Available[#im.Available+1]=row end
  im.Rows={}
end,
GetInstance=function(im)
  local item=table.remove(im.Available)
  if not item then
    local names=im.m_InstanceName=='QuestInstance' and {'Title','Description','Reward','Callout','Top'}
      or im.m_InstanceName=='RelationshipIcon' and {'Icon','TeamRibbon','DiplomacyPip','Background'}
      or im.m_InstanceName=='CityStateIconInstance' and {'IconButton','Icon','DiplomacyPip','Top'}
      or {'NameLabel','Icon','QuestIcon','Envoy','EnvoyCount','BonusImage1','BonusImage3','BonusImage6','Suzerain','BonusImageSuzerainOff','BonusImageSuzerainOn','SuzerainLabel','EnvoyMoreButton','EnvoyLessButton','Button','NameButton','LookAtButton','DiplomacyPip','AmbassadorButton'}
    item={};for _,name in ipairs(names) do item[name]=control(name) end
    if item.LookAtButton then item.LookAtButton.Hidden=true end
    item[im.m_RootControlName]=item[im.m_RootControlName] or control(im.m_RootControlName)
    item.m_InstanceManager=im
    local top=item[im.m_RootControlName];top.Row=item;top.Parent=im.m_ParentControl
    im.m_ParentControl.Children[#im.m_ParentControl.Children+1]=top
    item.GetTopControl=function() return top end
  end
  item[im.m_RootControlName]:SetHide(false)
  im.Rows[#im.Rows+1]=item;return item
end}
local IM_DEFINITION={new=InstanceManager.new,ResetInstances=InstanceManager.ResetInstances,GetInstance=InstanceManager.GetInstance}
Mouse={eLClick=1};PlayerOperations={EXECUTE_SCRIPT=13}
REQUESTS={};BASE_LIST,BASE_VIEW,BASE_NAME,BASE_SELECT,BASE_REFRESH=0,0,0,0,0
VIEW={From=1,To=2,Status='AVAILABLE',Mode='SMALL_STATE',Country={Name='六角',Profile={Personality='慎重',Trait='武人',CivilizationType='CIVILIZATION_JAC_ROKKAKU'}},
  Envoys={Route='PRODUCT',Envoys=0,Required=2},Deposit={Gold=115,Faith=23},Rows={{ID='combat',Label='戦闘部隊',Kind='PRODUCT_METRIC',Current=1,Minimum=3,Met=false},
    {ID='deposit',Label='保証金',Kind='PRODUCT_DEPOSIT',Gold=115,Faith=23,Met=false},{ID='relations',Label='関係',Met=false,Reviewed=false}}}
ExposedMembers={JAC_AbsorptionProduct={Get=function(from,to) assert(to==2);if from~=1 then return {Status='UNAVAILABLE'} end;return VIEW end,
 GetSmallCountries=function(to) assert(to==2);return {VIEW} end}}
Players={[1]={IsMajor=function() return not OFFICIAL end,GetInfluence=function() return {CanReceiveInfluence=function() return OFFICIAL==true end} end},
 [2]={IsMajor=function() return true end,IsTurnActive=function() return true end,GetInfluence=function() return {GetTokensToGive=function() return 2 end} end},
 [3]={IsMajor=function() return false end},[5]={IsMajor=function() return false end}}
Game={GetLocalPlayer=function() return 2 end,GetCurrentGameTurn=function() return 7 end}
SCREEN_HIDDEN=false
Locale={Lookup=function(s) return s end};ContextPtr={GetID=function() return 'CityStates' end,IsHidden=function() return SCREEN_HIDDEN end}
UI={RequestPlayerOperation=function(actor,op,params) assert(actor==2 and op==13);REQUESTS[#REQUESTS+1]=params end}
UI.GetPlayerColors=function(actor) return 'primary '..actor,'secondary '..actor end
CAMERAS={}
UI.LookAtPlotScreenPosition=function(x,y,horizontal,vertical)
  CAMERAS[#CAMERAS+1]={X=x,Y=y,Horizontal=horizontal,Vertical=vertical}
end
Players[1].GetCities=function() return {GetCapitalCity=function() return {GetX=function() return 10 end,GetY=function() return 20 end} end} end
Events={GameCoreEventPublishComplete=event(),InfluenceChanged=event(),LocalPlayerChanged=event(),InterfaceModeChanged=event()}
LuaEvents={CityStates_OpenOverview=event(),CityBannerManager_RaiseMinorCivPanel=event(),
  NotificationPanel_OpenCityStatesSendEnvoys=event(),PartialScreenHooks_OpenCityStates=event(),
  PartialScreenHooks_CloseCityStates=event(),PartialScreenHooks_CloseAllExcept=event(),WorldRankings_CloseCityStates=event()}
InterfaceModeTypes={VIEW_MODAL_LENS=9}
function JAC_InstallCivNameOverride(name) BASE_NAME=BASE_NAME+1 end
function include(name)
  if name=='CityStates_Expansion2' then
    include('InstanceManager')
    -- Match Base/Assets/UI/PartialScreens/CityStates.lua: private mode, global
    -- Refresh calls, and callbacks captured by Initialize before replacement.
    local mode,selected='Overview',-1
    local nativeQuestIM=InstanceManager:new('QuestInstance','Top',Controls.QuestsStack)
    INSTANCES.NativeQuests=nativeQuestIM
    local nativeRowsIM=InstanceManager:new('CityStateRowInstance','CityStateBase',Controls.CityStateStack)
    local nativeIconsIM=InstanceManager:new('CityStateIconInstance','Top',Controls.CityStateIconStack)
    local nativeRelationsIM=InstanceManager:new('RelationshipIcon','Background',Controls.RelationshipsButtonStack)
    INSTANCES.NativeRows=nativeRowsIM;INSTANCES.NativeIcons=nativeIconsIM;INSTANCES.NativeRelationships=nativeRelationsIM
    NATIVE_QUESTS={}
    function GetQuests(player)
      -- Base GetQuests keys by questInfo.Index (including zero and holes),
      -- and preserves all five native quest fields without a dense-array promise.
      if not NATIVE_QUESTS[player] then
        NATIVE_QUESTS[player]={}
        for _,id in ipairs({0,4,12}) do
          NATIVE_QUESTS[player][id]={Name='native quest '..player..'/'..id,
            Description='native description '..player..'/'..id,Reward='native reward '..id,
            Type='QUEST_NATIVE_'..id,Callout='[ICON_Native'..id..']'}
        end
      end
      return NATIVE_QUESTS[player]
    end
    NATIVE_GOLD=1000
    function ViewList()
      BASE_LIST=BASE_LIST+1
      Controls.ListOfCityStates:SetHide(false);Controls.SingleCityState:SetHide(true)
      nativeRowsIM:ResetInstances()
      for _,actor in ipairs({3,5}) do
        local row=nativeRowsIM:GetInstance();row.NameLabel:SetText('native minor '..actor)
        row.DiplomacyPip:SetTexture('native list diplomacy '..actor)
        row.DiplomacyPip:SetToolTipString('native list state '..actor)
        row.AmbassadorButton:SetHide(actor~=3)
        row.AmbassadorButton:SetToolTipString('native ambassador '..actor)
        row.Button:RegisterCallback(Mouse.eLClick,function() OpenSingleViewCityState(actor) end)
        row.AmbassadorButton:RegisterCallback(Mouse.eLClick,function() OpenSingleViewCityState(actor);OnInfluencedByClick() end)
      end
    end
    function Refresh()
      BASE_REFRESH=BASE_REFRESH+1
      if mode=='Overview' or mode=='SendEnvoys' or selected==-1 then ViewList() else ViewCityState(selected) end
    end
    function OnInfluenceChanged() if not ContextPtr:IsHidden() then Refresh() end end
    function ViewCityState(player)
      BASE_VIEW=BASE_VIEW+1
      if not Players[player] or Players[player]:IsMajor() then
        NATIVE_CACHE_MISS=(NATIVE_CACHE_MISS or 0)+1;mode='Overview';ViewList();return
      end
      Controls.ListOfCityStates:SetHide(true);Controls.SingleCityState:SetHide(false)
      Controls.CityStateName:SetText('native minor '..player)
      Controls.TypeValue:SetText('native type '..player);Controls.PatronValue:SetText('native patron '..player)
      Controls.CityStateTypeIcon:SetIcon('ICON_NATIVE_'..player)
      Controls.CityStateTypeIcon:SetColor('secondary '..player)
      Controls.CityStateTypeIcon:SetToolTipString('native type tooltip '..player)
      Controls.InfluencedByValue:SetText('native civilizations '..player)
      Controls.DiplomacyPip:SetTexture('native diplomacy '..player)
      Controls.DiplomacyPip:SetToolTipString('native diplomatic state '..player)
      nativeRelationsIM:ResetInstances()
      local relationship=nativeRelationsIM:GetInstance()
      relationship.Icon:SetIcon('ICON_NATIVE_RELATION_'..player)
      relationship.Icon:SetToolTipString('native relation '..player)
      nativeIconsIM:ResetInstances()
      for _,actor in ipairs({3,5}) do
        local row=nativeIconsIM:GetInstance();row.Icon:SetIcon('ICON_NATIVE_'..actor)
        row.Icon:SetToolTipString('native minor '..actor)
        row.IconButton:RegisterCallback(Mouse.eLClick,function() OpenSingleViewCityState(actor) end)
      end
      Controls.LevyMilitaryButton:SetHide(false);Controls.LevyMilitaryButton:SetDisabled(false)
      Controls.LevyMilitaryButton:SetToolTipString('native levy details')
      Controls.LevyMilitaryButton:RegisterCallback(Mouse.eLClick,function() NATIVE_LEVY=(NATIVE_LEVY or 0)+1;NATIVE_GOLD=NATIVE_GOLD-100 end)
      Controls.PeaceWarButton:SetHide(false);Controls.PeaceWarButton:SetText('native war/peace');Controls.PeaceWarButton:SetDisabled(false)
      Controls.PeaceWarButton:SetToolTipString('native war/peace details')
      Controls.PeaceWarButton:RegisterCallback(Mouse.eLClick,function() NATIVE_WAR_PEACE=(NATIVE_WAR_PEACE or 0)+1 end)
      Controls.QuestsArea:SetHide(mode~='Quests')
      for _,entry in ipairs({{Controls.EnvoysSentButton,'EnvoySent'},{Controls.InfluencedByButton,'InfluencedBy'},
        {Controls.QuestsButton,'Quests'},{Controls.RelationshipsButton,'Relationships'}}) do entry[1]:SetSelected(mode==entry[2]) end
      if mode=='Quests' then
        nativeQuestIM:ResetInstances()
        for _,quest in pairs(GetQuests(player)) do
          local row=nativeQuestIM:GetInstance()
          row.Title:SetString(quest.Name);row.Description:SetString(quest.Description)
          row.Reward:SetString(quest.Reward);row.Callout:SetString(quest.Callout)
        end
        Controls.QuestsStack:CalculateSize();Controls.QuestsScroll:CalculateSize()
      end
    end
    function Close() SCREEN_HIDDEN=true end
    function OnClose() Close() end
    function OnBackClose()
      mode=Players[Game.GetLocalPlayer()]:GetInfluence():GetTokensToGive()>0 and 'SendEnvoys' or 'Overview';Refresh()
    end
    function OnCloseAllExcept(context) if context~=ContextPtr:GetID() then Close() end end
    function OnCloseCityStates() Close() end
    function OpenOverview() mode='Overview';SCREEN_HIDDEN=false;Refresh() end
    function OnOpenCityStates() OpenOverview() end
    function OnOpenSendEnvoys() mode='SendEnvoys';SCREEN_HIDDEN=false;Refresh() end
    function OpenSingleViewCityState(player)
      if mode=='Overview' or mode=='SendEnvoys' then mode='EnvoySent' end
      selected=player;SCREEN_HIDDEN=false;Refresh()
    end
    function OnRaiseMinorCivicsPanel(player) OpenSingleViewCityState(player) end
    function OnInterfaceModeChanged(_,newMode) if newMode==InterfaceModeTypes.VIEW_MODAL_LENS then Close() end end
    function OnEnvoySentClick() NATIVE_ENVOY_BUTTON=(NATIVE_ENVOY_BUTTON or 0)+1;mode='EnvoySent';Refresh() end
    function OnInfluencedByClick() mode='InfluencedBy';Refresh() end
    function OnQuestsClick() mode='Quests';Refresh() end
    function OnRelationshipsClick() mode='Relationships';Refresh() end
    Controls.CloseListButton:RegisterCallback(Mouse.eLClick,OnClose)
    Controls.CloseBackButton:RegisterCallback(Mouse.eLClick,OnBackClose)
    for _,entry in ipairs({{Controls.EnvoysSentButton,OnEnvoySentClick},{Controls.InfluencedByButton,OnInfluencedByClick},
      {Controls.QuestsButton,OnQuestsClick},{Controls.RelationshipsButton,OnRelationshipsClick}}) do
      entry[1]:RegisterCallback(Mouse.eLClick,entry[2])
    end
    Events.InfluenceChanged.Add(OnInfluenceChanged)
    Events.LocalPlayerChanged.Add(OnClose)
    Events.InterfaceModeChanged.Add(OnInterfaceModeChanged)
    LuaEvents.CityBannerManager_RaiseMinorCivPanel.Add(OnRaiseMinorCivicsPanel)
    LuaEvents.NotificationPanel_OpenCityStatesSendEnvoys.Add(OnOpenSendEnvoys)
    LuaEvents.PartialScreenHooks_OpenCityStates.Add(OnOpenCityStates)
    LuaEvents.PartialScreenHooks_CloseCityStates.Add(OnCloseCityStates)
    LuaEvents.PartialScreenHooks_CloseAllExcept.Add(OnCloseAllExcept)
    LuaEvents.WorldRankings_CloseCityStates.Add(OnClose)
  elseif name=='DiplomacyActionView_Expansion2' then
    function SelectPlayer() BASE_SELECT=BASE_SELECT+1 end
    function GetInitialStatementOptions() return {{Text='original diplomacy'}} end
  elseif name=='InstanceManager' then
    -- Including the real module defines a fresh class before native managers
    -- are constructed; hooks installed only on the old class cannot reach them.
    InstanceManager={new=IM_DEFINITION.new,ResetInstances=IM_DEFINITION.ResetInstances,GetInstance=IM_DEFINITION.GetInstance}
  elseif name=='JAC_CivNameOverride' or name=='JAC_Absorption_AdmissionUI' then
  else error('unexpected screen include '..name) end
end
"#;

fn screen(repo: &Repository) -> anyhow::Result<Lua> {
    let lua = Lua::new();
    lua.load(SCREEN).exec()?;
    let text = repo.read(absorption_product::TEXT)?;
    let doc = roxmltree::Document::parse(&text)?;
    let languages = lua.create_table()?;
    for language in ["ja_JP", "en_US", "ru_RU"] {
        let labels = lua.create_table()?;
        for row in doc
            .descendants()
            .filter(|n| n.has_tag_name("Row") && n.attribute("Language") == Some(language))
        {
            labels.set(
                row.attribute("Tag").unwrap(),
                row.children()
                    .find(|n| n.has_tag_name("Text"))
                    .unwrap()
                    .text()
                    .unwrap(),
            )?;
        }
        languages.set(language, labels)?;
    }
    lua.globals().set("LANGUAGES", languages)?;
    lua.load(r#"
      LANGUAGE='ja_JP'
      Locale.Lookup=function(key,...)
        local text=LANGUAGES[LANGUAGE][key] or key
        for i,value in ipairs({...}) do text=text:gsub('{'..i..'}',function() return tostring(value) end) end
        return text
      end
    "#).exec()?;
    Ok(lua)
}

fn load_city_states(lua: &Lua, repo: &Repository) -> anyhow::Result<()> {
    // Exercise the Rust template without writing generated Lua into the checkout.
    let outputs = absorption_product::generate(repo)?;
    let source = outputs
        .get(Path::new(absorption_product::CITY_STATES))
        .context("generated CityStates source missing")?;
    lua.load(source)
        .set_name(absorption_product::CITY_STATES)
        .exec()?;
    Ok(())
}

fn load_registration_ui(lua: &Lua, repo: &Repository) -> anyhow::Result<()> {
    // A separate worker owns the adapter. Normally test the generated checkout
    // artifact; an explicit read-only source lets this worker verify that
    // parent's uncommitted artifact without copying it into the checkout.
    let source = match std::env::var_os("JAC_ADMISSION_UI_TEST_SOURCE") {
        Some(path) => {
            std::fs::read_to_string(path).context("read parent admission UI test source")?
        }
        None => repo.read(absorption_product::UI)?,
    };
    lua.load(source).set_name(absorption_product::UI).exec()?;
    Ok(())
}

#[test]
fn refund_observation_wait_disables_registration_and_sends_no_native_operation(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    load_registration_ui(&lua, &repo)?;
    load_city_states(&lua, &repo)?;
    lua.load(r#"
      local U=JAC_AbsorptionAdmissionUI
      VIEW.Status='CANCELLED';VIEW.CanCancel=false;VIEW.CanRegister=false;VIEW.OfferID='product:1:2:1'
      VIEW.Rows[2].Settlement='RETURNING'
      for _,currency in ipairs({'Gold','Faith'}) do
        VIEW.Error='awaiting '..currency..' refund observation'
        local text,command=U.Action(VIEW)
        assert(text==U.Text('RETURNING') and command==nil,'refund observation wait still offers registration')
        assert(not U.Send(VIEW,'REGISTER') and #REQUESTS==0,'blocked registration reached a native operation')
        ViewCityState(1)
        assert(Controls.LevyMilitaryButton.Disabled and Controls.LevyMilitaryButton.Text==U.Text('RETURNING'))
        Controls.LevyMilitaryButton.Callback()
        assert(not JAC_AbsorptionCityStatesAction(1,'REGISTER') and #REQUESTS==0)
        Events.InfluenceChanged.Call();Events.GameCoreEventPublishComplete.Call()
        assert(Controls.LevyMilitaryButton.Disabled and #REQUESTS==0,'screen refresh re-enabled registration before refund observation')
      end
    "#).exec()?;
    Ok(())
}

#[test]
fn observed_cancelled_refund_and_legacy_views_keep_authenticated_registration_available(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    load_registration_ui(&lua, &repo)?;
    load_city_states(&lua, &repo)?;
    lua.load(r#"
      local U=JAC_AbsorptionAdmissionUI
      VIEW.Status='CANCELLED';VIEW.CanCancel=false;VIEW.CanRegister=true;VIEW.OfferID='product:1:2:1'
      VIEW.Rows[2].Settlement='RETURNED'
      local text,command=U.Action(VIEW)
      assert(text==U.Text('REGISTER_SMALL') and command=='REGISTER','observed refund did not restore registration')
      ViewCityState(1);assert(not Controls.LevyMilitaryButton.Disabled)
      Controls.LevyMilitaryButton.Callback()
      assert(#REQUESTS==1 and REQUESTS[1].Command=='REGISTER' and REQUESTS[1].From==1
        and REQUESTS[1].OfferID==VIEW.OfferID and REQUESTS[1].OnStart=='JAC_AbsorptionProduct_Operation')
      VIEW.CanRegister=nil;VIEW.Status='AVAILABLE';VIEW.OfferID=nil
      for _,mode in ipairs({'SMALL_STATE','MERGER'}) do
        VIEW.Mode=mode
        local label,action=U.Action(VIEW)
        assert(action=='REGISTER' and label==U.Text(mode=='SMALL_STATE' and 'REGISTER_SMALL' or 'REGISTER_MERGER'),
          'older views without CanRegister lost registration')
        assert(U.Send(VIEW,'REGISTER'))
      end
      assert(#REQUESTS==3 and REQUESTS[2].Command=='REGISTER' and REQUESTS[3].Command=='REGISTER')
      VIEW.CanRegister=false;VIEW.CanCancel=true;VIEW.OfferID='product:1:2:2'
      local _,cancel=U.Action(VIEW)
      assert(cancel=='CANCEL' and U.Send(VIEW,'CANCEL'),'registration gate blocked an existing cancellation')
      assert(#REQUESTS==4 and REQUESTS[4].Command=='CANCEL' and REQUESTS[4].OfferID==VIEW.OfferID)
    "#).exec()?;
    Ok(())
}

#[test]
fn native_influence_refresh_keeps_the_product_country_and_its_next_envoy_action(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    load_city_states(&lua, &repo)?;
    lua.load(r#"
      ViewCityState(1);assert(JAC_AbsorptionCityStatesAction(1,'SEND_ENVOY'))
      VIEW.Envoys.Envoys=1;Events.InfluenceChanged.Call()
      assert(Controls.EnvoysSentValue.Text=='1','native refresh lost the selected product country')
      assert(JAC_AbsorptionCityStatesAction(1,'SEND_ENVOY'),'second visible envoy action lost its country')
      assert(#REQUESTS==2 and REQUESTS[1].ExpectedEnvoys==0 and REQUESTS[2].ExpectedEnvoys==1)
      OFFICIAL=true;Events.InfluenceChanged.Call();assert(BASE_REFRESH==1 and BASE_LIST==1,'official minor refresh changed')
    "#).exec()?;
    Ok(())
}

#[test]
fn human_envoy_scenario_waits_for_the_previous_assignment_before_sending_again(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let scenario: serde_json::Value =
        serde_json::from_str(&absorption_product::acceptance_scenario(&repo, true, true)?)?;
    let code = scenario["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|step| {
            step["code"]
                .as_str()
                .filter(|code| code.contains("JAC_PRODUCT_ENVOYS_FINISHED=false"))
        })
        .unwrap();
    let lua = screen(&repo)?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    load_city_states(&lua, &repo)?;
    lua.load(
        r#"
      Game.GetProperty=function() error('Gameplay properties not yet published to UI') end
      PlayerManager={GetWasEverAliveCount=function() return 3 end}
      PlayerConfigurations={
        [1]={GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_ODA' end},
        [2]={GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_TAKEDA' end}}
      ContextPtr.IsHidden=function() return false end
      ViewCityState(1)
    "#,
    )
    .exec()?;
    lua.load(code).exec()?;
    lua.load(
        r#"
      assert(#REQUESTS==1)
      for i=1,5 do Events.GameCoreEventPublishComplete.Call() end
      assert(#REQUESTS==1,'unconfirmed publications resubmitted the same action')
      VIEW.Envoys.Envoys=1;OnInfluenceChanged();Events.GameCoreEventPublishComplete.Call()
      assert(#REQUESTS==2 and REQUESTS[2].ExpectedEnvoys==1)
      VIEW.Envoys.Envoys=2;Events.GameCoreEventPublishComplete.Call()
      assert(JAC_PRODUCT_ENVOYS_FINISHED and #REQUESTS==2)
      Events.GameCoreEventPublishComplete.Call();assert(#REQUESTS==2)
    "#,
    )
    .exec()?;
    Ok(())
}

#[test]
fn existing_small_country_controls_display_conditions_and_send_authenticated_commands(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    load_city_states(&lua, &repo)?;
    lua.load(r#"
      ViewList();assert(BASE_LIST==1 and #INSTANCES.CityStateRowInstance.Rows==1)
      assert(Controls.NoneMet.Hidden and INSTANCES.CityStateRowInstance.Rows[1].EnvoyCount.Text=='0 / 2')
      INSTANCES.CityStateRowInstance.Rows[1].LookAtButton.Callback()
      assert(CAMERAS[1].X==10 and CAMERAS[1].Y==20 and CAMERAS[1].Horizontal==0.34375 and CAMERAS[1].Vertical==0.5)
      assert(BASE_VIEW==0 and #REQUESTS==0,'map navigation must not submit a product action')
      INSTANCES.CityStateRowInstance.Rows[1].NameButton.Callback()
      assert(BASE_VIEW==0 and Controls.CityStateName.Text=='六角' and Controls.QuestsArea.Hidden==false)
      assert(INSTANCES.QuestInstance.Rows[1].Description.Text:find('戦闘部隊 1 / 3',1,true))
      assert(INSTANCES.QuestInstance.Rows[1].Description.Text:find('未確認',1,true))
      Controls.LevyMilitaryButton.Callback();local p=REQUESTS[1]
      assert(p.Command=='REGISTER' and p.From==1 and p.To==nil and p.Deposit==nil and p.Requirements==nil)
      assert(p.OnStart=='JAC_AbsorptionProduct_Operation' and p.Nonce)
      Controls.PeaceWarButton.Callback();assert(REQUESTS[2].Command=='SEND_ENVOY' and REQUESTS[2].ExpectedEnvoys==0)
      VIEW.CanCancel=true;VIEW.OfferID='product:1:2:1';ViewCityState(1)
      Controls.LevyMilitaryButton.Callback();assert(REQUESTS[3].Command=='CANCEL' and REQUESTS[3].OfferID==VIEW.OfferID)
      VIEW.Error='Scripts/internal.lua:45: currency journal is ambiguous'
      ViewCityState(1)
      local message=INSTANCES.QuestInstance.Rows[1].Description.Text
      assert(message:find('登録の取消し操作が使えます',1,true))
      assert(not message:find('internal.lua',1,true) and not message:find('currency journal',1,true))
      VIEW.Error=nil
      VIEW.Rows[2].Settlement='RETURNED';VIEW.Rows[2].Met=false;ViewCityState(1)
      assert(INSTANCES.QuestInstance.Rows[1].Description.Text:find('返却済み',1,true))
      OFFICIAL=true;OpenSingleViewCityState(1);assert(BASE_VIEW==1)
      Controls.EnvoysSentButton.Callback();assert(NATIVE_ENVOY_BUTTON==1 and #REQUESTS==3)
      Controls.LevyMilitaryButton.Callback();Controls.PeaceWarButton.Callback()
      assert(NATIVE_LEVY==1 and NATIVE_WAR_PEACE==1 and #REQUESTS==3)
      assert(Controls.LevyMilitaryButton.Text==Locale.Lookup('LOC_CITY_STATES_LEVY_MILITARY_BUTTON'))
      local quests=GetQuests(1)
      assert(quests[0] and quests[4] and quests[12] and quests[0].Name=='native quest 1/0')
      assert(#INSTANCES.MinorActions.Rows==1)
      VIEW.CanCancel=false;ViewCityState(1)
      INSTANCES.MinorActions.Rows[1].NameButton.Callback()
      assert(#REQUESTS==4 and REQUESTS[4].Command=='REGISTER' and REQUESTS[4].From==1)
      local before=BASE_VIEW;ViewCityState(3);assert(BASE_VIEW==before+1,'unmapped minors must retain normal screen behavior')
      assert(#INSTANCES.MinorActions.Rows==0)
    "#).exec()?;
    Ok(())
}

#[test]
fn product_actions_restore_native_levy_label_before_a_real_minor_can_spend_gold(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    load_city_states(&lua, &repo)?;
    lua.load(r#"
      for _,cancel in ipairs({false,true}) do
        VIEW.CanCancel=cancel;VIEW.OfferID=cancel and 'product:1:2:1' or nil
        ViewCityState(1)
        local productText=Controls.LevyMilitaryButton.Text
        assert(productText==JAC_AbsorptionAdmissionUI.Text(cancel and 'CANCEL' or 'REGISTER_SMALL'))
        ViewCityState(3)
        assert(Controls.LevyMilitaryButton.Text==Locale.Lookup('LOC_CITY_STATES_LEVY_MILITARY_BUTTON'),
          'native levy callback still carries the product registration/cancel label')
        OpenSingleViewCityState(3)
        assert(Controls.LevyMilitaryButton.Tooltip=='native levy details' and not Controls.LevyMilitaryButton.Disabled)
        local gold=NATIVE_GOLD;Controls.LevyMilitaryButton.Callback()
        assert(NATIVE_GOLD==gold-100 and #REQUESTS==0,'native levy must retain its own operation')
        assert(Controls.PeaceWarButton.Text=='native war/peace' and not Controls.PeaceWarButton.Disabled)
        Controls.PeaceWarButton.Callback();assert(NATIVE_WAR_PEACE==(cancel and 2 or 1))
        for _,button in ipairs({Controls.EnvoysSentButton,Controls.InfluencedByButton,Controls.QuestsButton,Controls.RelationshipsButton}) do
          button.Callback();assert(Controls.CityStateName.Text=='native minor 3' and #REQUESTS==0)
        end
      end
      assert(not NATIVE_CACHE_MISS)
    "#).exec()?;
    Ok(())
}

#[test]
fn captured_native_back_close_and_explicit_switch_callbacks_release_the_product_selection(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    load_city_states(&lua, &repo)?;
    lua.load(r#"
      local switches={
        {'captured back button',function() Controls.CloseBackButton.Callback() end},
        {'back function',function() OnBackClose() end},
        {'captured close button',function() Controls.CloseListButton.Callback() end},
        {'close function',function() OnClose() end},
        {'central close',function() Close() end},
        {'open overview',function() OpenOverview() end},
        {'open city states',function() OnOpenCityStates() end},
        {'open send envoys',function() OnOpenSendEnvoys() end},
        {'select official minor',function() OpenSingleViewCityState(3) end},
        {'captured banner event',function() LuaEvents.CityBannerManager_RaiseMinorCivPanel.Call(3) end},
        {'captured envoy notification',function() LuaEvents.NotificationPanel_OpenCityStatesSendEnvoys.Call() end},
        {'captured overview event',function() LuaEvents.PartialScreenHooks_OpenCityStates.Call() end},
        {'captured explicit close',function() LuaEvents.PartialScreenHooks_CloseCityStates.Call() end},
        {'captured close all except',function() LuaEvents.PartialScreenHooks_CloseAllExcept.Call('Diplomacy') end},
        {'captured world rankings close',function() LuaEvents.WorldRankings_CloseCityStates.Call() end},
        {'captured local player change',function() Events.LocalPlayerChanged.Call() end},
        {'captured interface switch',function() Events.InterfaceModeChanged.Call(0,InterfaceModeTypes.VIEW_MODAL_LENS) end}}
      for _,entry in ipairs(switches) do
        SCREEN_HIDDEN=false;ViewCityState(1);entry[2]()
        assert(not JAC_AbsorptionCityStatesAction(1,'REGISTER'),entry[1]..' retained a hidden product action')
        SCREEN_HIDDEN=false
        Events.InfluenceChanged.Call();Events.GameCoreEventPublishComplete.Call()
        assert(Controls.SingleCityState.Hidden or Controls.CityStateName.Text~='六角',entry[1]..' reopened product detail')
        assert(#REQUESTS==0 and not NATIVE_CACHE_MISS,entry[1]..' changed a native operation')
      end
      ViewCityState(1);LuaEvents.PartialScreenHooks_CloseAllExcept.Call('CityStates')
      VIEW.Envoys.Envoys=1;Events.InfluenceChanged.Call()
      assert(not SCREEN_HIDDEN and Controls.EnvoysSentValue.Text=='1','staying open or influence refresh lost selection')
      assert(JAC_AbsorptionCityStatesAction(1,'SEND_ENVOY') and #REQUESTS==1)
    "#).exec()?;
    Ok(())
}

#[test]
fn frozen_small_country_keeps_conditions_and_preflight_cancel_without_sending_envoys(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    load_city_states(&lua, &repo)?;
    lua.load(r#"
      ViewCityState(1)
      VIEW.Envoys={Route='FROZEN',Envoys=2,Required=2};VIEW.Frozen=true
      VIEW.Status='FAILED';VIEW.Error='Scripts/internal.lua:42: preflight interrupted'
      VIEW.CanCancel=true;VIEW.OfferID='product:1:2:1'
      ViewCityState(1)
      assert(BASE_VIEW==0 and not NATIVE_CACHE_MISS,'frozen major was sent to the native minor cache')
      Events.InfluenceChanged.Call();Events.GameCoreEventPublishComplete.Call()
      assert(not Controls.SingleCityState.Hidden and Controls.CityStateName.Text=='六角')
      local description=INSTANCES.QuestInstance.Rows[1].Description.Text
      assert(description:find('戦闘部隊',1,true) and description:find('登録の取消し操作が使えます',1,true))
      assert(not description:find('internal.lua',1,true))
      assert(Controls.LevyMilitaryButton.Text==JAC_AbsorptionAdmissionUI.Text('CANCEL') and not Controls.LevyMilitaryButton.Disabled)
      assert(Controls.PeaceWarButton.Disabled,'frozen receipt must not offer another envoy')
      Controls.PeaceWarButton.Callback();assert(#REQUESTS==0)
      assert(not JAC_AbsorptionCityStatesAction(1,'SEND_ENVOY'))
      Controls.LevyMilitaryButton.Callback();assert(#REQUESTS==1 and REQUESTS[1].Command=='CANCEL' and REQUESTS[1].OfferID==VIEW.OfferID)
      ViewList();assert(#INSTANCES.CityStateRowInstance.Rows==1,'living frozen small country disappeared from the list')
      INSTANCES.CityStateRowInstance.Rows[1].NameButton.Callback()
      assert(Controls.LevyMilitaryButton.Text==JAC_AbsorptionAdmissionUI.Text('CANCEL'))
      VIEW.Status='STARTED';VIEW.CanCancel=false;VIEW.Error=nil
      Events.InfluenceChanged.Call()
      assert(Controls.LevyMilitaryButton.Disabled and Controls.PeaceWarButton.Disabled)
      Controls.LevyMilitaryButton.Callback();assert(not JAC_AbsorptionCityStatesAction(1,'CANCEL') and #REQUESTS==1,
        'irreversible flow allowed cancellation')
      for _,button in ipairs({Controls.EnvoysSentButton,Controls.InfluencedByButton,Controls.QuestsButton,Controls.RelationshipsButton}) do
        button.Callback();assert(BASE_VIEW==0 and Controls.CityStateName.Text=='六角')
      end
      VIEW.Envoys=nil;ViewCityState(1)
      assert(BASE_VIEW==0 and not NATIVE_CACHE_MISS and Controls.PeaceWarButton.Disabled,'country classification depends on envoy route')
      VIEW.Mode='MERGER';ViewList();assert(#INSTANCES.CityStateRowInstance.Rows==0,'major merger country entered the small-country list')
      OFFICIAL=true;VIEW.Mode='SMALL_STATE';VIEW.Envoys={Route='FROZEN',Envoys=2,Required=2}
      VIEW.Status='FAILED';VIEW.CanCancel=true;OpenSingleViewCityState(1)
      assert(BASE_VIEW==1 and Controls.CityStateName.Text=='native minor 1','frozen official minor lost its native detail')
      assert(Controls.LevyMilitaryButton.Text==Locale.Lookup('LOC_CITY_STATES_LEVY_MILITARY_BUTTON'))
      assert(Controls.PeaceWarButton.Text=='native war/peace' and not Controls.PeaceWarButton.Disabled)
      local quests=GetQuests(1)
      assert(#INSTANCES.MinorActions.Rows==1 and quests[0] and quests[4] and quests[12])
      INSTANCES.MinorActions.Rows[1].NameButton.Callback();assert(#REQUESTS==2 and REQUESTS[2].Command=='CANCEL')
    "#).exec()?;
    Ok(())
}

#[test]
fn eliminated_preflight_source_returns_to_the_living_list_after_automatic_refund(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    load_city_states(&lua, &repo)?;
    lua.load(r#"
      local source=Players[1]
      for _,route in ipairs({'PRODUCT','FROZEN'}) do
      for _,event in ipairs({Events.InfluenceChanged,Events.GameCoreEventPublishComplete}) do
      for _,missing in ipairs({false,true}) do
        Players[1]=source
        source.IsAlive=function() return true end
        ExposedMembers.JAC_AbsorptionProduct.Get=function(from,to) assert(to==2);return from==1 and VIEW or {Status='UNAVAILABLE'} end
        ExposedMembers.JAC_AbsorptionProduct.GetSmallCountries=function() return {VIEW} end
        VIEW.Envoys={Route=route,Envoys=2,Required=2};VIEW.Status=route=='FROZEN' and 'FAILED' or 'WAITING';VIEW.CanCancel=true
        ViewCityState(1)
        -- Gameplay auto-cancels/refunds dead pre-P1 actors. The selected actor
        -- becomes unavailable, and GetSmallCountries contains living actors only.
        ExposedMembers.JAC_AbsorptionProduct.Get=function(from,to)
          assert(to==2);return {From=from,To=to,Status='UNAVAILABLE',CanCancel=false}
        end
        ExposedMembers.JAC_AbsorptionProduct.GetSmallCountries=function() return {} end
        if missing then Players[1]=nil else source.IsAlive=function() return false end end
        event.Call()
        assert(Controls.SingleCityState.Hidden and not Controls.ListOfCityStates.Hidden,
          'unavailable eliminated source retained its obsolete cancel detail')
        assert(#INSTANCES.CityStateRowInstance.Rows==0 and #INSTANCES.MinorActions.Rows==0)
        assert(not JAC_AbsorptionCityStatesAction(1,'CANCEL') and #REQUESTS==0 and not NATIVE_CACHE_MISS)
        event.Call();assert(Controls.SingleCityState.Hidden,'later refresh resurrected the eliminated country')
      end end end
    "#).exec()?;
    Ok(())
}

#[test]
fn native_sparse_quest_ids_and_fields_survive_product_conditions_without_mutating_native_data(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load("InstanceManager=nil").exec()?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    load_city_states(&lua, &repo)?;
    lua.load(r#"
      OFFICIAL=true
      local quests=GetQuests(1);local native=NATIVE_QUESTS[1];local count,conditions=0,0
      for key,quest in pairs(native) do
        assert(quests[key]==quest,'native sparse quest ID '..key..' was dropped or changed')
        for _,field in ipairs({'Name','Description','Reward','Type','Callout'}) do
          assert(quests[key][field]==native[key][field],'native quest field changed: '..field)
        end
      end
      for key,quest in pairs(quests) do
        count=count+1
        if not native[key] then
          conditions=conditions+1
          assert(quest.Name==JAC_AbsorptionAdmissionUI.Text('CONDITIONS'))
          assert(quest.Description:find('六角',1,true))
        end
      end
      assert(count==4 and conditions==1 and native[1]==nil,'synthetic conditions changed the native sparse map')
      assert(GetQuests(3)==NATIVE_QUESTS[3],'unmapped official minor must retain the native table')
      assert(NATIVE_QUESTS[1]==native and GetQuests(1)[12]==native[12])
      native[1]={Name='occupied quest 1',Description='description 1',Reward='reward 1',Type='QUEST_NATIVE_1',Callout='[ICON_Native1]'}
      native[2]={Name='occupied quest 2',Description='description 2',Reward='reward 2',Type='QUEST_NATIVE_2',Callout='[ICON_Native2]'}
      local expanded=GetQuests(1);local expandedCount=0
      for key,quest in pairs(native) do assert(expanded[key]==quest,'conditions overwrote an occupied native quest ID') end
      for _ in pairs(expanded) do expandedCount=expandedCount+1 end
      assert(expandedCount==6 and native[3]==nil,'conditions modified the native source table while finding a free key')
    "#).exec()?;
    Ok(())
}

#[test]
fn quest_rows_and_action_buttons_belong_to_the_selected_country_across_seven_native_paths(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let mut failures = Vec::new();
    for (name, path) in [
        ("major to major", "ViewCityState(1);ASSERT_ROWS(1);ViewCityState(4);ASSERT_ROWS(4);Controls.LevyMilitaryButton.Callback();ASSERT_COMMAND(4,'CANCEL')"),
        ("major to official", "ViewCityState(1);ASSERT_ROWS(1);OFFICIAL_DETAIL();ASSERT_ROWS(3);INSTANCES.MinorActions.Rows[1].NameButton.Callback();ASSERT_COMMAND(3,'CANCEL')"),
        ("official to major", "OFFICIAL_DETAIL();ViewCityState(4);ASSERT_ROWS(4);Controls.LevyMilitaryButton.Callback();ASSERT_COMMAND(4,'CANCEL')"),
        ("captured Back", "OFFICIAL_DETAIL();Controls.CloseBackButton.Callback();ASSERT_EMPTY();ViewCityState(1);ASSERT_ROWS(1);Controls.LevyMilitaryButton.Callback();ASSERT_COMMAND(1,'REGISTER')"),
        ("captured Close", "OFFICIAL_DETAIL();Controls.CloseListButton.Callback();ASSERT_EMPTY();Events.GameCoreEventPublishComplete.Call();ASSERT_EMPTY();LuaEvents.CityStates_OpenOverview.Call(1);ASSERT_ROWS(1);Controls.LevyMilitaryButton.Callback();ASSERT_COMMAND(1,'REGISTER')"),
        ("major InfluenceRefresh", "OFFICIAL_DETAIL();ViewCityState(4);COUNTRIES[4].Envoys.Envoys=1;Events.InfluenceChanged.Call();ASSERT_ROWS(4);assert(Controls.EnvoysSentValue.Text=='1');Controls.PeaceWarButton.Callback();ASSERT_COMMAND(4,'SEND_ENVOY')"),
        ("official InfluenceRefresh", "ViewCityState(1);OFFICIAL_DETAIL();Events.InfluenceChanged.Call();ASSERT_ROWS(3);INSTANCES.MinorActions.Rows[1].NameButton.Callback();ASSERT_COMMAND(3,'CANCEL')"),
    ] {
        let lua = screen(&repo)?;
        lua.load(repo.read(absorption_product::UI)?).exec()?;
        lua.load(r#"
          local function copy(value)
            if type(value)~='table' then return value end
            local out={};for key,item in pairs(value) do out[key]=copy(item) end;return out
          end
          local other=copy(VIEW);other.From=4;other.Country.Name='三好'
          other.Country.Profile.CivilizationType='CIVILIZATION_JAC_MIYOSHI'
          other.CanCancel=true;other.OfferID='product:4:2:1'
          local minor=copy(VIEW);minor.From=3;minor.Country.Name='Vatican'
          minor.Envoys.Route='OFFICIAL';minor.CanCancel=true;minor.OfferID='product:3:2:1'
          COUNTRIES={[1]=VIEW,[3]=minor,[4]=other}
          Players[4]={IsMajor=function() return true end,GetCities=Players[1].GetCities}
          ExposedMembers.JAC_AbsorptionProduct.Get=function(from,to)
            assert(to==2);return COUNTRIES[from] or {Status='UNAVAILABLE'}
          end
          ExposedMembers.JAC_AbsorptionProduct.GetSmallCountries=function() return {VIEW,other} end
          function OFFICIAL_DETAIL()
            OpenSingleViewCityState(3);Controls.QuestsButton.Callback()
          end
          function ASSERT_EMPTY()
            for _,root in ipairs(Controls.QuestsStack.Children) do
              assert(root.Hidden,'Back/Close retained an allocated quest/action root')
            end
            for _,manager in ipairs(ALL_MANAGERS) do
              if manager.m_ParentControl==Controls.QuestsStack then
                assert(#manager.Rows==0,'hidden native/product rows were not released to their owning pool')
              end
            end
          end
          function ASSERT_ROWS(actor)
            local country=COUNTRIES[actor];local native,conditions,actions,visible=0,0,0,0
            assert(not Controls.SingleCityState.Hidden and not Controls.QuestsArea.Hidden)
            for _,root in ipairs(Controls.QuestsStack.Children) do if not root.Hidden then
              visible=visible+1;local row=root.Row
              if row.Title then
                if row.Title.Text==JAC_AbsorptionAdmissionUI.Text('CONDITIONS') then
                  conditions=conditions+1
                  assert(row.Description.Text:find(country.Country.Name,1,true),'conditions retained a different country')
                else
                  local expected
                  for _,quest in pairs(NATIVE_QUESTS[actor] or {}) do if quest.Name==row.Title.Text then expected=quest end end
                  assert(expected,'native quest retained a different country')
                  assert(row.Description.Text==expected.Description and row.Reward.Text==expected.Reward and row.Callout.Text==expected.Callout)
                  native=native+1
                end
              else
                actions=actions+1
                assert(row.NameButton.Tooltip:find(country.Country.Name,1,true),'product action retained a different country')
                assert(not row.NameButton.Disabled)
              end
            end end
            assert(conditions==1,'previous native/product conditions remain allocated')
            local official=not Players[actor]:IsMajor()
            assert(native==(official and 3 or 0) and actions==(official and 1 or 0),'missing or stale native quests/action rows')
            local allocated=0
            for _,manager in ipairs(ALL_MANAGERS) do if manager.m_ParentControl==Controls.QuestsStack then
              for _,row in ipairs(manager.Rows) do
                assert(row.m_InstanceManager==manager and not row.GetTopControl().Hidden,'row belongs to the wrong manager pool')
                allocated=allocated+1
              end
            end end
            assert(allocated==visible,'row hiding left another manager allocation behind')
          end
          function ASSERT_COMMAND(actor,command)
            assert(#REQUESTS==1 and REQUESTS[1].From==actor and REQUESTS[1].Command==command,'visible action submitted for the wrong country')
            assert(not NATIVE_CACHE_MISS)
          end
        "#).exec()?;
        load_city_states(&lua, &repo)?;
        if let Err(error) = lua.load(path).set_name(name).exec() {
            failures.push(format!("{name}: {error}"));
        }
    }
    anyhow::ensure!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}

#[test]
fn native_report_xml_fields_never_describe_the_previous_minor_after_product_navigation(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let mut failures = Vec::new();
    for (name, path) in [
        ("native to product", "OpenSingleViewCityState(3);ASSERT_NATIVE(3);ViewCityState(1);ASSERT_PRODUCT();Events.InfluenceChanged.Call();ASSERT_PRODUCT()"),
        ("product to native", "ViewCityState(1);ASSERT_PRODUCT();OpenSingleViewCityState(5);ASSERT_NATIVE(5);Controls.InfluencedByButton.Callback();ASSERT_NATIVE(5)"),
        ("product to list", "OpenSingleViewCityState(3);ViewCityState(1);ViewList();ASSERT_LIST();INSTANCES.NativeRows.Rows[2].Button.Callback();ASSERT_NATIVE(5)"),
        ("captured Back", "OpenSingleViewCityState(3);ViewCityState(1);Controls.CloseBackButton.Callback();ASSERT_LIST();LuaEvents.CityBannerManager_RaiseMinorCivPanel.Call(5);ASSERT_NATIVE(5)"),
        ("captured Close", "OpenSingleViewCityState(3);ViewCityState(1);Controls.CloseListButton.Callback();assert(SCREEN_HIDDEN);LuaEvents.PartialScreenHooks_OpenCityStates.Call();ASSERT_LIST();INSTANCES.NativeRows.Rows[1].AmbassadorButton.Callback();ASSERT_NATIVE(3)"),
        ("frozen product to unmapped native", "OpenSingleViewCityState(3);VIEW.Envoys.Route='FROZEN';VIEW.Status='STARTED';ViewCityState(1);ASSERT_PRODUCT();ExposedMembers.JAC_AbsorptionProduct=nil;OpenSingleViewCityState(5);ASSERT_NATIVE(5)"),
    ] {
        let lua = screen(&repo)?;
        lua.load(repo.read(absorption_product::UI)?).exec()?;
        lua.load(r#"
          -- These fields are outside the hidden Influence/Relationships tabs
          -- in the installed Expansion2 XML. Their default visibility matters.
          assert(not Controls.InfluencedByValue.Hidden and not Controls.RelationshipsButtonStack.Hidden)
          assert(not Controls.DiplomacyPip.Hidden and not Controls.CityStateIconStack.Hidden)
          function ASSERT_NATIVE(actor)
            assert(not Controls.SingleCityState.Hidden and Controls.ListOfCityStates.Hidden)
            assert(Controls.CityStateName.Text=='native minor '..actor)
            assert(not Controls.InfluencedByButton.Hidden and not Controls.InfluencedByValue.Hidden)
            assert(Controls.InfluencedByValue.Text=='native civilizations '..actor)
            assert(not Controls.RelationshipsButton.Hidden and not Controls.RelationshipsButtonStack.Hidden)
            assert(#INSTANCES.NativeRelationships.Rows==1 and INSTANCES.NativeRelationships.Rows[1].Icon.Tooltip=='native relation '..actor)
            assert(not Controls.DiplomacyPip.Hidden and Controls.DiplomacyPip.Texture[1]=='native diplomacy '..actor)
            assert(Controls.DiplomacyPip.Tooltip=='native diplomatic state '..actor)
            assert(not Controls.CityStateIconStack.Hidden and #INSTANCES.NativeIcons.Rows==2)
            assert(Controls.CityStateTypeIcon.Tooltip=='native type tooltip '..actor and Controls.CityStateTypeIcon.Color=='secondary '..actor)
            assert(Controls.PatronValue.Text=='native patron '..actor)
            assert(Controls.LevyMilitaryButton.Text==Locale.Lookup('LOC_CITY_STATES_LEVY_MILITARY_BUTTON'))
          end
          function ASSERT_PRODUCT()
            assert(not Controls.SingleCityState.Hidden and Controls.CityStateName.Text=='六角')
            assert(Controls.InfluencedByValue.Hidden or Controls.InfluencedByValue.Text=='',
              'always-visible civilizations still describe the previous minor')
            assert(Controls.RelationshipsButtonStack.Hidden or #INSTANCES.NativeRelationships.Rows==0,
              'always-visible relationship icons still describe the previous minor')
            assert(Controls.DiplomacyPip.Hidden,'native diplomatic state still appears on a product country')
            assert(Controls.CityStateIconStack.Hidden,'native sidebar still appears as this product country data')
            assert(Controls.CityStateTypeIcon.Tooltip==JAC_AbsorptionAdmissionUI.Text('SMALL_COUNTRY'))
            assert(Controls.CityStateTypeIcon.Color=='secondary 1','country icon retained native color')
            assert(Controls.PatronValue.Text=='' and Controls.PatronValue.Tooltip=='')
            assert(Controls.QuestsButton.Selected and not Controls.EnvoysSentButton.Selected
              and not Controls.InfluencedByButton.Selected and not Controls.RelationshipsButton.Selected,
              'native subsection selection remained selected with product conditions')
            assert(not Controls.QuestsArea.Hidden and Controls.InfluenceArea.Hidden and Controls.RelationshipsArea.Hidden)
            assert(#INSTANCES.QuestInstance.Rows==1 and INSTANCES.QuestInstance.Rows[1].Description.Text:find('六角',1,true))
          end
          function ASSERT_LIST()
            assert(Controls.SingleCityState.Hidden and not Controls.ListOfCityStates.Hidden)
            assert(not Controls.InfluencedByButton.Hidden and not Controls.InfluencedByValue.Hidden)
            assert(not Controls.RelationshipsButton.Hidden and not Controls.RelationshipsButtonStack.Hidden)
            assert(not Controls.DiplomacyPip.Hidden and not Controls.CityStateIconStack.Hidden,
              'returning to the native list did not restore XML visibility')
            assert(#INSTANCES.NativeRows.Rows==2 and #REQUESTS==0 and not NATIVE_CACHE_MISS)
          end
        "#).exec()?;
        load_city_states(&lua, &repo)?;
        if let Err(error) = lua.load(path).set_name(name).exec() {
            failures.push(format!("{name}: {error}"));
        }
    }
    anyhow::ensure!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}

#[test]
fn product_reused_native_rows_hide_default_diplomacy_and_ambassador_without_hiding_native_rows(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    load_city_states(&lua, &repo)?;
    lua.load(r#"
      ViewList()
      for _,row in ipairs(INSTANCES.NativeRows.Rows) do assert(not row.DiplomacyPip.Hidden) end
      assert(not INSTANCES.NativeRows.Rows[1].AmbassadorButton.Hidden)
      local row=INSTANCES.CityStateRowInstance.Rows[1]
      assert(row.DiplomacyPip.Hidden,'product list row displays an uninitialized native diplomacy pip')
      assert(row.AmbassadorButton.Hidden,'product list row claims a native ambassador')
      assert(not row.QuestIcon.Hidden and not row.Icon.Hidden and not row.NameButton.Hidden)
      assert(not row.LookAtButton.Hidden,'real XML hides LookAt by default; product map action must explicitly show it')
      row.LookAtButton.Callback();assert(#CAMERAS==1 and #REQUESTS==0)
      OFFICIAL=true;OpenSingleViewCityState(1)
      local action=INSTANCES.MinorActions.Rows[1]
      assert(action.DiplomacyPip.Hidden and action.AmbassadorButton.Hidden,
        'product admission action row includes unrelated native diplomacy/ambassador decorations')
      assert(not action.NameButton.Hidden and not action.NameButton.Disabled)
      action.NameButton.Callback();assert(#REQUESTS==1 and REQUESTS[1].From==1 and REQUESTS[1].Command=='REGISTER')
      ViewList()
      assert(not INSTANCES.NativeRows.Rows[1].DiplomacyPip.Hidden and not INSTANCES.NativeRows.Rows[1].AmbassadorButton.Hidden,
        'product row cleanup modified the native pool')
    "#).exec()?;
    Ok(())
}

#[test]
fn small_country_rows_keep_their_respective_actor_when_multiple_countries_are_listed(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load(r#"
      local function copy(value)
        if type(value)~='table' then return value end
        local out={};for key,item in pairs(value) do out[key]=copy(item) end;return out
      end
      local other=copy(VIEW);other.From=3;other.Country.Name='三好'
      other.Country.Profile.CivilizationType='CIVILIZATION_JAC_MIYOSHI'
      Players[3]={IsMajor=function() return true end,GetInfluence=function() return {CanReceiveInfluence=function() return false end} end}
      Players[3].GetCities=function() return {GetCapitalCity=function() return {GetX=function() return 30 end,GetY=function() return 40 end} end} end
      ExposedMembers.JAC_AbsorptionProduct.Get=function(from,to)
        assert(to==2);return from==1 and VIEW or from==3 and other or {Status='UNAVAILABLE'}
      end
      ExposedMembers.JAC_AbsorptionProduct.GetSmallCountries=function(to)
        assert(to==2);return {VIEW,other}
      end
    "#).exec()?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    load_city_states(&lua, &repo)?;
    lua.load(
        r#"
      ViewList();local rows=INSTANCES.CityStateRowInstance.Rows;assert(#rows==2)
      rows[1].NameButton.Callback();assert(Controls.CityStateName.Text=='六角')
      Controls.LevyMilitaryButton.Callback();assert(REQUESTS[1].From==1)
      rows[2].NameButton.Callback();assert(Controls.CityStateName.Text=='三好')
      Controls.LevyMilitaryButton.Callback();assert(REQUESTS[2].From==3)
    "#,
    )
    .exec()?;
    Ok(())
}
#[test]
fn diplomacy_preserves_name_override_base_options_and_registers_the_current_counterpart(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load(repo.read(absorption_product::DIPLOMACY)?).exec()?;
    lua.load(r#"
      VIEW.Mode='MERGER';VIEW.Envoys=nil
      SelectPlayer(1,0);local options=GetInitialStatementOptions({},{});
      assert(BASE_NAME==1 and BASE_SELECT==1 and options[1].Text=='original diplomacy')
      assert(options[2].ToolTip:find('戦闘部隊 1 / 3',1,true));options[2].Callback()
      assert(REQUESTS[1].Command=='REGISTER' and REQUESTS[1].From==1)
      SelectPlayer(3,0);assert(#GetInitialStatementOptions({},{})==1)
      ExposedMembers.JAC_AbsorptionProduct=nil;SelectPlayer(1,0);assert(#GetInitialStatementOptions({},{})==1)
    "#).exec()?;
    Ok(())
}

#[test]
fn display_uses_english_and_russian_resources_without_changing_canonical_profile_values(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    lua.load(
        r#"
      VIEW.Country.Name='Rokkaku'
      VIEW.Country.Profile.PersonalityLabel='LOC_JAC_ADMISSION_PERSONALITY_CAREFUL'
      VIEW.Country.Profile.TraitLabel='LOC_JAC_ADMISSION_TRAIT_VALIANT'
      VIEW.Rows[1].Label='LOC_JAC_ADMISSION_METRIC_COMBAT_UNITS'
      VIEW.Rows[2].Label='LOC_JAC_ADMISSION_DEPOSIT'
      VIEW.Rows[2].Settlement='RETURNED'
      VIEW.Rows[3].Label='LOC_JAC_ADMISSION_RELATIONS'
    "#,
    )
    .exec()?;
    for (language, personality, returned) in [
        ("en_US", "Careful", "Returned"),
        ("ru_RU", "Осторожный", "Возвращено"),
    ] {
        lua.globals().set("LANGUAGE", language)?;
        let output: String = lua
            .load("return JAC_AbsorptionAdmissionUI.Lines(VIEW)")
            .eval()?;
        assert!(
            output.contains(personality) && output.contains(returned),
            "{output}"
        );
        assert!(
            !output.contains("慎重") && !output.contains("保証金") && !output.contains("未確認")
        );
        let canonical: String = lua.load("return VIEW.Country.Profile.Personality").eval()?;
        assert_eq!(canonical, "慎重");
    }
    Ok(())
}

#[test]
fn shared_lookup_renders_all_canonical_and_legacy_character_labels_and_consumed_cost(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    lua.load(
        r#"
      VIEW.Country.Profile.PersonalityLabel=nil;VIEW.Country.Profile.TraitLabel=nil
      LABEL_CASES={
        {'勇敢','BRAVE','武人','VALIANT'},{'冷静','CALM','鬼神','FIERCE'},
        {'慎重','CAREFUL','策士','ASTUTE'},{'陽気','JOLLY','魔神','SHREWD'},
        {'温厚','GENTLE','教者','DEVOUT'},{'強気','BOLD','霊神','ASCETIC'},
        {'自由','FICKLE','凡人','MUNDANE'},{'忠実','LOYAL','天才','ADEPT'}}
      VIEW.Rows[2].Settlement='CONSUMED';VIEW.Rows[2].Met=true
    "#,
    )
    .exec()?;
    for language in ["en_US", "ru_RU", "ja_JP"] {
        lua.globals().set("LANGUAGE", language)?;
        lua.load(r#"
          for _,case in ipairs(LABEL_CASES) do
            for _,alias in ipairs({'canonical','legacy','loc'}) do
              local personality=alias=='canonical' and case[1] or (alias=='legacy' and 'JAC_' or 'LOC_JAC_')..'PERSONALITY_'..case[2]
              local trait=alias=='canonical' and case[3] or (alias=='legacy' and 'JAC_' or 'LOC_JAC_')..'TRAIT_'..case[4]
              VIEW.Country.Profile.Personality,VIEW.Country.Profile.Trait=personality,trait
              local output=JAC_AbsorptionAdmissionUI.Lines(VIEW)
              assert(output:find(Locale.Lookup('LOC_JAC_ADMISSION_PERSONALITY_'..case[2]),1,true))
              assert(output:find(Locale.Lookup('LOC_JAC_ADMISSION_TRAIT_'..case[4]),1,true))
              if LANGUAGE~='ja_JP' then assert(not output:find(case[1],1,true) and not output:find(case[3],1,true)) end
              assert(VIEW.Country.Profile.Personality==personality and VIEW.Country.Profile.Trait==trait)
              assert(output:find(Locale.Lookup('LOC_JAC_ADMISSION_CONSUMED'),1,true))
            end
          end
          Players[2].IsMajor=function() return false end
          assert(not JAC_AbsorptionAdmissionUI.Send(VIEW,'REGISTER') and #REQUESTS==0)
        "#).exec()?;
    }
    Ok(())
}
