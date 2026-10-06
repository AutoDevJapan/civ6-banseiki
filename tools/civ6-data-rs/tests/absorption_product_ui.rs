use civ6_data_rs::{absorption_product, Repository};
use mlua::Lua;
const SCREEN: &str = r#"
local function event()
  local handlers={}
  return {Add=function(fn) handlers[#handlers+1]=fn end,
    Remove=function(fn) for i=#handlers,1,-1 do if handlers[i]==fn then table.remove(handlers,i) end end end,
    Call=function() local snapshot={};for i,fn in ipairs(handlers) do snapshot[i]=fn end;for _,fn in ipairs(snapshot) do fn() end end}
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
  'QuestsButton','QuestsScroll','ReportArea','SingleViewStack','LevyMilitaryButton','PeaceWarButton','EnvoysSentButton','InfluencedByButton','RelationshipsButton'}) do
  Controls[name]=control(name)
end
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
 [2]={IsMajor=function() return true end,IsTurnActive=function() return true end,GetInfluence=function() return {GetTokensToGive=function() return 2 end} end}}
Game={GetLocalPlayer=function() return 2 end,GetCurrentGameTurn=function() return 7 end}
Locale={Lookup=function(s) return s end};ContextPtr={GetID=function() return 'CityStates' end,IsHidden=function() return true end}
UI={RequestPlayerOperation=function(actor,op,params) assert(actor==2 and op==13);REQUESTS[#REQUESTS+1]=params end}
CAMERAS={}
UI.LookAtPlotScreenPosition=function(x,y,horizontal,vertical)
  CAMERAS[#CAMERAS+1]={X=x,Y=y,Horizontal=horizontal,Vertical=vertical}
end
Players[1].GetCities=function() return {GetCapitalCity=function() return {GetX=function() return 10 end,GetY=function() return 20 end} end} end
Events={GameCoreEventPublishComplete=event()}
LuaEvents={CityStates_OpenOverview=event()}
function JAC_InstallCivNameOverride(name) BASE_NAME=BASE_NAME+1 end
function include(name)
  if name=='CityStates_Expansion2' then
    function ViewList() BASE_LIST=BASE_LIST+1 end
    function Refresh() BASE_REFRESH=BASE_REFRESH+1;ViewList() end
    function OnInfluenceChanged() Refresh() end
    function ViewCityState()
      BASE_VIEW=BASE_VIEW+1
      Controls.LevyMilitaryButton:SetText('native levy');Controls.LevyMilitaryButton:SetDisabled(false)
      Controls.LevyMilitaryButton:RegisterCallback(Mouse.eLClick,function() NATIVE_LEVY=(NATIVE_LEVY or 0)+1 end)
      Controls.PeaceWarButton:RegisterCallback(Mouse.eLClick,function() NATIVE_WAR_PEACE=(NATIVE_WAR_PEACE or 0)+1 end)
    end
    function GetQuests() return {{Name='ordinary quest'}} end
    function OnOpenCityStates() ViewList() end
    function OnEnvoySentClick() NATIVE_ENVOY_BUTTON=(NATIVE_ENVOY_BUTTON or 0)+1 end;function OnInfluencedByClick() end;function OnQuestsClick() end;function OnRelationshipsClick() end
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

#[test]
fn native_influence_refresh_keeps_the_product_country_and_its_next_envoy_action(
) -> anyhow::Result<()> {
    let repo = Repository::checked_in();
    let lua = screen(&repo)?;
    lua.load(repo.read(absorption_product::UI)?).exec()?;
    lua.load(repo.read(absorption_product::CITY_STATES)?)
        .exec()?;
    lua.load(r#"
      ViewCityState(1);assert(JAC_AbsorptionCityStatesAction(1,'SEND_ENVOY'))
      VIEW.Envoys.Envoys=1;OnInfluenceChanged()
      assert(Controls.EnvoysSentValue.Text=='1','native refresh lost the selected product country')
      assert(JAC_AbsorptionCityStatesAction(1,'SEND_ENVOY'),'second visible envoy action lost its country')
      assert(#REQUESTS==2 and REQUESTS[1].ExpectedEnvoys==0 and REQUESTS[2].ExpectedEnvoys==1)
      OFFICIAL=true;OnInfluenceChanged();assert(BASE_REFRESH==1 and BASE_LIST==1,'official minor refresh changed')
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
    lua.load(repo.read(absorption_product::CITY_STATES)?)
        .exec()?;
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
    lua.load(repo.read(absorption_product::CITY_STATES)?)
        .exec()?;
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
      OFFICIAL=true;ViewCityState(1);assert(BASE_VIEW==1)
      Controls.EnvoysSentButton.Callback();assert(NATIVE_ENVOY_BUTTON==1 and #REQUESTS==3)
      Controls.LevyMilitaryButton.Callback();Controls.PeaceWarButton.Callback()
      assert(NATIVE_LEVY==1 and NATIVE_WAR_PEACE==1 and #REQUESTS==3)
      assert(Controls.LevyMilitaryButton.Text=='native levy')
      local quests=GetQuests(1);assert(#quests==2 and quests[1].Name=='ordinary quest')
      assert(#INSTANCES.MinorActions.Rows==1)
      VIEW.CanCancel=false;ViewCityState(1)
      INSTANCES.MinorActions.Rows[1].NameButton.Callback()
      assert(#REQUESTS==4 and REQUESTS[4].Command=='REGISTER' and REQUESTS[4].From==1)
      ViewCityState(3);assert(BASE_VIEW==3,'unmapped minors must retain normal screen behavior')
      assert(#INSTANCES.MinorActions.Rows==0)
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
    lua.load(repo.read(absorption_product::CITY_STATES)?)
        .exec()?;
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
