use anyhow::Context;
use civ6_data_rs::{absorption_product, Repository};
use mlua::Lua;
use std::path::Path;
const SCREEN: &str = r#"
local function event()
  local handlers={}
  return {Add=function(fn) handlers[#handlers+1]=fn end,
    Remove=function(fn) for i=#handlers,1,-1 do if handlers[i]==fn then table.remove(handlers,i) end end end,
    Call=function(...) local snapshot={};for i,fn in ipairs(handlers) do snapshot[i]=fn end;for _,fn in ipairs(snapshot) do fn(...) end end}
end
local function control(name)
  return {Name=name,Text='',Hidden=false,Disabled=false,Callback=nil,
    SetText=function(s,t) s.Text=t end,SetString=function(s,t) s.Text=t end,
    SetHide=function(s,v) s.Hidden=v end,SetDisabled=function(s,v) s.Disabled=v end,
    SetSelected=function() end,SetIcon=function() end,SetToolTipString=function(s,v) s.Tooltip=v end,
    CalculateSize=function() end,DoAutoSize=function() end,
    RegisterCallback=function(s,_,fn) s.Callback=fn end}
end
Controls={}
for _,name in ipairs({'CityStateStack','QuestsStack','NoneMet','ListOfCityStates','SingleCityState','CityStateName','CityStateTypeIcon',
  'TypeValue','PatronValue','EnvoysSentValue','QuestsValue','EnvoysSentArea','InfluenceArea','RelationshipsArea','QuestsArea',
  'QuestsButton','QuestsScroll','ReportArea','SingleViewStack','LevyMilitaryButton','PeaceWarButton','EnvoysSentButton','InfluencedByButton','RelationshipsButton','CloseListButton','CloseBackButton'}) do
  Controls[name]=control(name)
end
-- CityStates.xml initializes this label once; native ViewCityState never resets it.
Controls.LevyMilitaryButton.Text='LOC_CITY_STATES_LEVY_MILITARY_BUTTON'
Controls.CityStateScroll={GetSizeX=function() return 500 end}
UIManager={GetScreenSizeVal=function() return 1600,900 end}
INSTANCES={}
InstanceManager={new=function(_,template,root,parent)
  assert(template=='QuestInstance' and root=='Top' or template=='CityStateRowInstance' and root=='CityStateBase','official instance root required')
  local im={Rows={}}
  INSTANCES[template=='CityStateRowInstance' and parent==Controls.QuestsStack and 'MinorActions' or template]=im
  return {ResetInstances=function() im.Rows={} end,GetInstance=function()
    local names=template=='QuestInstance' and {'Title','Description','Reward','Callout'} or
      {'NameLabel','Icon','QuestIcon','Envoy','EnvoyCount','BonusImage1','BonusImage3','BonusImage6','Suzerain','BonusImageSuzerainOff','BonusImageSuzerainOn','SuzerainLabel','EnvoyMoreButton','EnvoyLessButton','Button','NameButton','LookAtButton'}
    local item={};for _,name in ipairs(names) do item[name]=control(name) end
    im.Rows[#im.Rows+1]=item;return item
  end}
end}
Mouse={eLClick=1};PlayerOperations={EXECUTE_SCRIPT=13}
REQUESTS={};BASE_LIST,BASE_VIEW,BASE_NAME,BASE_SELECT,BASE_REFRESH=0,0,0,0,0
VIEW={From=1,To=2,Status='AVAILABLE',Mode='SMALL_STATE',Country={Name='六角',Profile={Personality='慎重',Trait='武人',CivilizationType='CIVILIZATION_JAC_ROKKAKU'}},
  Envoys={Route='PRODUCT',Envoys=0,Required=2},Deposit={Gold=115,Faith=23},Rows={{ID='combat',Label='戦闘部隊',Kind='PRODUCT_METRIC',Current=1,Minimum=3,Met=false},
    {ID='deposit',Label='保証金',Kind='PRODUCT_DEPOSIT',Gold=115,Faith=23,Met=false},{ID='relations',Label='関係',Met=false,Reviewed=false}}}
ExposedMembers={JAC_AbsorptionProduct={Get=function(from,to) assert(to==2);if from~=1 then return {Status='UNAVAILABLE'} end;return VIEW end,
 GetSmallCountries=function(to) assert(to==2);return {VIEW} end}}
Players={[1]={IsMajor=function() return not OFFICIAL end,GetInfluence=function() return {CanReceiveInfluence=function() return OFFICIAL==true end} end},
 [2]={IsMajor=function() return true end,IsTurnActive=function() return true end,GetInfluence=function() return {GetTokensToGive=function() return 2 end} end},
 [3]={IsMajor=function() return false end}}
Game={GetLocalPlayer=function() return 2 end,GetCurrentGameTurn=function() return 7 end}
SCREEN_HIDDEN=false
Locale={Lookup=function(s) return s end};ContextPtr={GetID=function() return 'CityStates' end,IsHidden=function() return SCREEN_HIDDEN end}
UI={RequestPlayerOperation=function(actor,op,params) assert(actor==2 and op==13);REQUESTS[#REQUESTS+1]=params end}
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
    -- Match Base/Assets/UI/PartialScreens/CityStates.lua: private mode, global
    -- Refresh calls, and callbacks captured by Initialize before replacement.
    local mode,selected='Overview',-1
    NATIVE_GOLD=1000
    function ViewList()
      BASE_LIST=BASE_LIST+1
      Controls.ListOfCityStates:SetHide(false);Controls.SingleCityState:SetHide(true)
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
      Controls.LevyMilitaryButton:SetHide(false);Controls.LevyMilitaryButton:SetDisabled(false)
      Controls.LevyMilitaryButton:SetToolTipString('native levy details')
      Controls.LevyMilitaryButton:RegisterCallback(Mouse.eLClick,function() NATIVE_LEVY=(NATIVE_LEVY or 0)+1;NATIVE_GOLD=NATIVE_GOLD-100 end)
      Controls.PeaceWarButton:SetHide(false);Controls.PeaceWarButton:SetText('native war/peace');Controls.PeaceWarButton:SetDisabled(false)
      Controls.PeaceWarButton:SetToolTipString('native war/peace details')
      Controls.PeaceWarButton:RegisterCallback(Mouse.eLClick,function() NATIVE_WAR_PEACE=(NATIVE_WAR_PEACE or 0)+1 end)
    end
    function GetQuests() return {{Name='ordinary quest'}} end
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
      local quests=GetQuests(1);assert(#quests==2 and quests[1].Name=='ordinary quest')
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
      assert(#INSTANCES.MinorActions.Rows==1 and #GetQuests(1)==2)
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
