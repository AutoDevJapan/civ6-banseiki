use civ6_lua_tests_rs::{exec, new_lua, read_repo};
use mlua::Lua;
use std::collections::HashMap;

const STAGING: &str = "poc/JAC_PoC_JapanMapScale/UI/JAC_JapanStagingRoom.lua";
const XML: &str = "poc/JAC_PoC_JapanMapScale/UI/JAC_JapanStagingRoom.xml";
const TEXT: &str = "poc/JAC_PoC_JapanMapScale/Text/JAC_JapanMapScale_Text.xml";

fn fixture() -> Lua {
    let lua = new_lua();
    exec(
        &lua,
        r#"
hotseat=false; network=true; host=true; configBroadcasts=0; playerBroadcasts={}
launchActive=false; stoppedLaunches=0
fireSyncPlayerInfo=false
rejectedConfigKeys={};coercedConfigKeys={}
stockReady=0; stockAutostart=0; participants={0,1,2}; values={
  RULESET='RULESET_JAC_BANSEIKI', NO_TEAMS=true
}
map='{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map'
SlotStatus={SS_TAKEN=1,SS_COMPUTER=2,SS_CLOSED=3}
Mouse={eLClick=1}
local function player(id, status, team)
  local p={id=id,status=status,team=team,ready=id==0}
  function p:GetSlotStatus() return self.status end
  function p:GetTeam() return self.team end
  function p:SetTeam(value) if not self.rejectTeam then self.team=value end end
  function p:GetPlayerName() return 'Player '..self.id end
  function p:GetReady() return self.ready end
  return p
end
PlayerConfigurations={
  [0]=player(0,SlotStatus.SS_TAKEN,3),
  [1]=player(1,SlotStatus.SS_TAKEN,4),
  [2]=player(2,SlotStatus.SS_COMPUTER,3),
  [3]=player(3,SlotStatus.SS_TAKEN,5)
}
GameConfiguration={
  IsHotseat=function() return hotseat end,
  GetValue=function(key)
    if values[key]==nil then return end -- engine API returns zero values when unset
    return values[key]
  end,
  SetValue=function(key,value)
    if rejectedConfigKeys[key] then return end
    values[key]=coercedConfigKeys[key] and tostring(value) or value
  end,
  GetParticipatingPlayerIDs=function() return participants end
}
MapConfiguration={
  GetValue=function(key) assert(key=='MAP_SCRIPT');return map end,
  GetMaxMajorPlayers=function() return 22 end
}
Network={
  IsInSession=function() return network end,
  IsGameHost=function() return host end,
  GetLocalPlayerID=function() return 0 end,
  BroadcastGameConfig=function() configBroadcasts=configBroadcasts+1 end,
  BroadcastPlayerInfo=function(id)
    playerBroadcasts[#playerBroadcasts+1]=id
    if fireSyncPlayerInfo then Events.PlayerInfoChanged.fire(id) end
  end
}
Locale={Lookup=function(value) return value end}
local function control()
  local c={children={},hidden=false,disabled=false,items={}}
  function c:SetHide(value) self.hidden=value end
  function c:IsHidden() return self.hidden end
  function c:SetDisabled(value) self.disabled=value end
  function c:SetText(value) self.text=value end
  function c:RegisterCallback(_,f) self.click=f end
  function c:RegisterSelectionCallback(f) self.select=f end
  function c:SetVoids(a,b) self.void1=a;self.void2=b end
  function c:CalculateInternals() end
  function c:CalculateSize() end
  function c:GetButton()
    if not self.button then self.button=control() end
    return self.button
  end
  function c:ChangeParent(parent) parent.children[#parent.children+1]=self end
  function c:GetChildren() return self.children end
  function c:SortChildren(compare)
    local function wrapper(item)
      return setmetatable({}, {__tostring=function() return tostring(item) end})
    end
    table.sort(self.children,function(a,b) return compare(wrapper(a),wrapper(b)) end)
  end
  return c
end
Controls=setmetatable({}, {__index=function(t,key)
  local c=control();rawset(t,key,c);return c
end})
Controls.GameSummaryStack.children={control(),control()}
ContextPtr={hidden=true}
function ContextPtr:IsHidden() return self.hidden end
function ContextPtr:SetShowHandler(f)
  self.show=function(...)
    self.hidden=false
    return f(...)
  end
end
Events=setmetatable({}, {__index=function(t,key)
  local event={callbacks={}}
  function event.Add(f) event.callbacks[#event.callbacks+1]=f end
  function event.fire(...)
    for _,f in ipairs(event.callbacks) do f(...) end
  end
  rawset(t,key,event);return event
end})
InstanceManager={}
function InstanceManager:new(_,_,parent)
  local m={instances={},parent=parent}
  function m:ResetInstances() self.instances={} end
  function m:GetInstance()
    local row={Root=control(),PlayerName=control(),AreaPullDown=control()}
    self.instances[#self.instances+1]=row
    return row
  end
  rowsManager=m
  return m
end
PullDownInstanceManager={}
function PullDownInstanceManager:new(_,_,parent)
  local m={parent=parent}
  function m:ResetInstances() self.parent.items={} end
  function m:GetInstance()
    local item={Button=control()}
    self.parent.items[#self.parent.items+1]=item
    return item
  end
  return m
end
function include(name)
  assert(name=='StagingRoom')
  function OnShow() end
  function OnReadyButton() stockReady=stockReady+1 end
  function CheckGameAutoStart() stockAutostart=stockAutostart+1 end
  function SetLocalReady(value) PlayerConfigurations[0].ready=value end
  function UpdateReadyButton()
    Controls.ReadyButton:SetDisabled(false)
    if not hotseat then Controls.ReadyCheck:SetDisabled(false) end
  end
  function IsLaunchCountdownActive() return launchActive end
  function StopCountdown() launchActive=false;stoppedLaunches=stoppedLaunches+1 end
end
"#,
    );
    exec(&lua, &read_repo(STAGING));
    exec(&lua, "ContextPtr:show()");
    lua
}

#[test]
fn host_clicks_assign_nonoverlapping_areas_and_one_team_then_restore_stock_teams() {
    let lua = fixture();
    exec(
        &lua,
        r#"
assert(not Controls.JAC_AssignmentPanel.hidden)
assert(Controls.GameSummaryStack.children[1] == Controls.JAC_AssignmentPanel)
assert(values.JAC_MP_MODE==nil and values.NO_TEAMS==true)
Controls.JAC_AllJapanButton.click()
assert(values.JAC_MP_MODE=='JAPAN' and values.NO_TEAMS==false)
assert(values.JAC_MP_TEAM==4)
assert(PlayerConfigurations[0].team==4 and PlayerConfigurations[1].team==4)
assert(PlayerConfigurations[2].team==3)
assert(PlayerConfigurations[0].ready==false)
assert(Controls.ReadyButton.disabled and Controls.ReadyCheck.disabled)
UpdateReadyButton()
assert(Controls.ReadyButton.disabled and Controls.ReadyCheck.disabled)
OnReadyButton();assert(stockReady==0)
SetLocalReady(true);assert(PlayerConfigurations[0].ready==false)
assert(#rowsManager.instances==2)
rowsManager.instances[1].AreaPullDown.select(2) -- east
rowsManager.instances[2].AreaPullDown.select(6) -- Hokkaido overlaps east
assert(values.JAC_MP_AREA_1==nil)
assert(Controls.JAC_AssignmentStatus.text=='LOC_JAC_MP_OVERLAP')
rowsManager.instances[2].AreaPullDown.select(3) -- west
Controls.JAC_SubjectPullDown.select(2) -- player 0
assert(values.JAC_MP_AREA_0=='EAST' and values.JAC_MP_AREA_1=='WEST')
assert(values.JAC_MP_SUBJECT==0)
assert(Controls.JAC_SubjectIs.text=='LOC_JAC_MP_SUBJECT_IS Player 0')
assert(Controls.JAC_AssignmentStatus.text=='LOC_JAC_MP_READY')
assert(not Controls.ReadyButton.disabled and not Controls.ReadyCheck.disabled)
SetLocalReady(true);assert(PlayerConfigurations[0].ready==true)
OnReadyButton();assert(stockReady==1)
CheckGameAutoStart();assert(stockAutostart==1)
launchActive=true
rowsManager.instances[1].AreaPullDown.select(1) -- clearing an area cancels a running launch
assert(not launchActive and stoppedLaunches==1)
CheckGameAutoStart();assert(stockAutostart==1)
rowsManager.instances[1].AreaPullDown.select(2)
assert(configBroadcasts>=4 and #playerBroadcasts>=1)
ContextPtr:show()
assert(values.JAC_MP_AREA_0=='EAST' and values.JAC_MP_SUBJECT==0)
Controls.JAC_DifferentButton.click()
assert(values.JAC_MP_MODE=='DIFFERENT' and values.NO_TEAMS==true)
assert(PlayerConfigurations[0].team==3 and PlayerConfigurations[1].team==4)
assert(values.JAC_MP_ROSTER==nil and values.JAC_MP_SUBJECT==nil)
assert(Controls.JAC_JapanOptions.hidden)
"#,
    );
}

#[test]
fn joins_departures_and_host_migration_revalidate_saved_choices() {
    let lua = fixture();
    exec(
        &lua,
        r#"
Controls.JAC_AllJapanButton.click()
rowsManager.instances[1].AreaPullDown.select(2)
rowsManager.instances[2].AreaPullDown.select(3)
Controls.JAC_SubjectPullDown.select(2)
participants={0,1,2,3}
Events.MultiplayerPlayerConnected.fire(3)
assert(values.JAC_MP_AREA_3==nil and PlayerConfigurations[3].team==values.JAC_MP_TEAM)
assert(Controls.JAC_AssignmentStatus.text=='LOC_JAC_MP_NEED_AREA')
rowsManager.instances[3].AreaPullDown.select(5) -- Hokkaido overlaps east
assert(values.JAC_MP_AREA_3==nil)
rowsManager.instances[1].AreaPullDown.select(7) -- Aomori
rowsManager.instances[3].AreaPullDown.select(5)
assert(values.JAC_MP_AREA_3=='HOKKAIDO')
Controls.JAC_SubjectPullDown.select(4) -- player 3
assert(values.JAC_MP_SUBJECT==3)
launchActive=true
fireSyncPlayerInfo=true
JAC_SetPlayerArea(3,'EAST')
assert(Controls.JAC_AssignmentStatus.text=='LOC_JAC_MP_OVERLAP')
Events.MultiplayerPrePlayerDisconnected.fire(3)
assert(values.JAC_MP_AREA_3==nil and values.JAC_MP_SUBJECT==nil)
assert(PlayerConfigurations[3].team==5 and not launchActive and stoppedLaunches==1)
assert(Controls.JAC_AssignmentStatus.text=='LOC_JAC_MP_NEED_SUBJECT')
Events.PlayerInfoChanged.fire(3) -- still marked taken before the slot update
assert(PlayerConfigurations[3].team==5 and values.JAC_MP_PRIOR_TEAM_3==nil)
PlayerConfigurations[3].status=SlotStatus.SS_CLOSED
Events.PlayerInfoChanged.fire(3)
assert(Controls.JAC_AssignmentStatus.text=='LOC_JAC_MP_NEED_SUBJECT')
PlayerConfigurations[3].status=SlotStatus.SS_TAKEN
Events.MultiplayerPlayerConnected.fire(3)
assert(values.JAC_MP_PRIOR_TEAM_3==5)
assert(PlayerConfigurations[3].team==values.JAC_MP_TEAM)
PlayerConfigurations[3].status=SlotStatus.SS_CLOSED
Events.MultiplayerHostMigrated.fire(0) -- a missed pre-disconnect still restores the slot
assert(PlayerConfigurations[3].team==5 and values.JAC_MP_PRIOR_TEAM_3==nil)
PlayerConfigurations[3].status=SlotStatus.SS_TAKEN
Events.MultiplayerPlayerConnected.fire(3)
assert(values.JAC_MP_PRIOR_TEAM_3==5)
host=false
Events.MultiplayerHostMigrated.fire(1)
local before=configBroadcasts
Controls.JAC_DifferentButton.click()
assert(values.JAC_MP_MODE=='JAPAN' and configBroadcasts==before)
assert(Controls.JAC_AllJapanButton.disabled and Controls.JAC_SubjectPullDown.disabled)
host=true
Events.MultiplayerHostMigrated.fire(0)
assert(PlayerConfigurations[0].team==PlayerConfigurations[1].team)
Controls.JAC_DifferentButton.click()
assert(PlayerConfigurations[3].team==5)
"#,
    );
}

#[test]
fn single_player_and_other_maps_leave_stock_screen_and_ready_behavior_alone() {
    let lua = fixture();
    exec(
        &lua,
        r#"
network=false
ContextPtr:show()
assert(Controls.JAC_AssignmentPanel.hidden)
Controls.JAC_AllJapanButton.click()
assert(values.JAC_MP_MODE==nil)
OnReadyButton();assert(stockReady==1)
network=true;map='Continents.lua'
ContextPtr:show()
assert(Controls.JAC_AssignmentPanel.hidden)
assert(configBroadcasts==0)
hotseat=true
ContextPtr:show()
assert(Controls.JAC_AssignmentPanel.hidden)
map='{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_20km.Civ6Map'
ContextPtr:show()
assert(not Controls.JAC_AssignmentPanel.hidden)
assert(not Controls.InfoContainer.hidden)
Controls.JAC_AllJapanButton.click()
assert(Controls.ReadyCheck.disabled)
rowsManager.instances[1].AreaPullDown.select(2)
rowsManager.instances[2].AreaPullDown.select(3)
Controls.JAC_SubjectPullDown.select(2)
assert(not Controls.ReadyCheck.disabled)
"#,
    );
}

#[test]
fn hidden_context_defers_reconcile_until_show() {
    let lua = fixture();
    exec(
        &lua,
        r#"
ContextPtr.hidden=true
values.JAC_MP_MODE='JAPAN'
local before=configBroadcasts
Events.GameConfigChanged.fire()
Events.PlayerInfoChanged.fire(1)
assert(configBroadcasts==before)
assert(PlayerConfigurations[0].team==3 and PlayerConfigurations[1].team==4)
ContextPtr:show()
assert(PlayerConfigurations[0].team==PlayerConfigurations[1].team)
assert(values.JAC_MP_ROSTER=='0,1')
"#,
    );
}

#[test]
fn coerced_or_rejected_config_values_do_not_rebroadcast_forever() {
    let lua = fixture();
    exec(
        &lua,
        r#"
coercedConfigKeys.JAC_MP_TEAM=true
Controls.JAC_AllJapanButton.click()
assert(values.JAC_MP_TEAM=='4')
local before=configBroadcasts
for _=1,3 do Events.GameConfigChanged.fire() end
assert(configBroadcasts==before)
"#,
    );

    let lua = fixture();
    exec(
        &lua,
        r#"
rejectedConfigKeys.JAC_MP_ROSTER=true
Controls.JAC_AllJapanButton.click()
local before=configBroadcasts
for _=1,3 do Events.GameConfigChanged.fire() end
assert(configBroadcasts==before)
rowsManager.instances[1].AreaPullDown.select(2)
rowsManager.instances[2].AreaPullDown.select(3)
Controls.JAC_SubjectPullDown.select(2)
assert(Controls.ReadyButton.disabled)
rejectedConfigKeys.JAC_MP_ROSTER=nil
ContextPtr.hidden=true
ContextPtr:show()
assert(values.JAC_MP_ROSTER=='0,1')
assert(not Controls.ReadyButton.disabled)
"#,
    );
}

#[test]
fn rejected_team_assignment_does_not_rebroadcast_forever() {
    let lua = fixture();
    exec(
        &lua,
        r#"
PlayerConfigurations[0].rejectTeam=true
Controls.JAC_AllJapanButton.click()
local before=#playerBroadcasts
for _=1,3 do Events.PlayerInfoChanged.fire(0) end
assert(#playerBroadcasts==before)
assert(Controls.ReadyButton.disabled)
PlayerConfigurations[0].rejectTeam=false
ContextPtr.hidden=true
ContextPtr:show()
assert(PlayerConfigurations[0].team==PlayerConfigurations[1].team)
rowsManager.instances[1].AreaPullDown.select(2)
rowsManager.instances[2].AreaPullDown.select(3)
Controls.JAC_SubjectPullDown.select(2)
assert(not Controls.ReadyButton.disabled)
"#,
    );
}

#[test]
fn failed_team_restore_keeps_baseline_and_blocks_start_until_retry() {
    let lua = fixture();
    exec(
        &lua,
        r#"
Controls.JAC_AllJapanButton.click()
PlayerConfigurations[0].rejectTeam=true
Controls.JAC_DifferentButton.click()
assert(values.JAC_MP_MODE=='DIFFERENT')
assert(values.JAC_MP_ROSTER=='0,1' and values.JAC_MP_PRIOR_TEAM_0==3)
assert(PlayerConfigurations[0].team==4)
assert(Controls.ReadyButton.disabled)
local before=configBroadcasts
for _=1,3 do Events.GameConfigChanged.fire() end
assert(configBroadcasts==before)
PlayerConfigurations[0].rejectTeam=false
ContextPtr.hidden=true
ContextPtr:show()
assert(PlayerConfigurations[0].team==3)
assert(values.JAC_MP_ROSTER==nil and values.JAC_MP_PRIOR_TEAM_0==nil)
assert(not Controls.ReadyButton.disabled)
"#,
    );
}

#[test]
fn failed_no_teams_restore_blocks_start_until_retry() {
    let lua = fixture();
    exec(
        &lua,
        r#"
Controls.JAC_AllJapanButton.click()
rejectedConfigKeys.NO_TEAMS=true
Controls.JAC_DifferentButton.click()
assert(values.JAC_MP_ROSTER=='0,1' and values.NO_TEAMS==false)
assert(Controls.ReadyButton.disabled)
rejectedConfigKeys.NO_TEAMS=nil
ContextPtr.hidden=true
ContextPtr:show()
assert(values.NO_TEAMS==true and values.JAC_MP_ROSTER==nil)
assert(not Controls.ReadyButton.disabled)
"#,
    );
}

#[test]
fn rejected_baseline_never_changes_teams_before_roster_is_saved() {
    for key in ["JAC_MP_PRIOR_NO_TEAMS", "JAC_MP_PRIOR_TEAM_0"] {
        let lua = fixture();
        lua.globals().set("rejectedBaseline", key).unwrap();
        exec(
            &lua,
            r#"
rejectedConfigKeys[rejectedBaseline]=true
Controls.JAC_AllJapanButton.click()
assert(values.JAC_MP_ROSTER==nil)
assert(PlayerConfigurations[0].team==3 and PlayerConfigurations[1].team==4)
assert(values.NO_TEAMS==true and Controls.ReadyButton.disabled)
Controls.JAC_DifferentButton.click()
assert(values.JAC_MP_MODE=='DIFFERENT')
assert(values.JAC_MP_ROSTER==nil and values.NO_TEAMS==true)
rejectedConfigKeys[rejectedBaseline]=nil
ContextPtr.hidden=true
ContextPtr:show()
assert(PlayerConfigurations[0].team==3 and PlayerConfigurations[1].team==4)
"#,
        );
    }
}

#[test]
fn failed_restore_record_cleanup_keeps_roster_until_retry() {
    for key in ["JAC_MP_PRIOR_TEAM_0", "JAC_MP_ROSTER"] {
        let lua = fixture();
        lua.globals().set("rejectedCleanup", key).unwrap();
        exec(
            &lua,
            r#"
Controls.JAC_AllJapanButton.click()
rejectedConfigKeys[rejectedCleanup]=true
Controls.JAC_DifferentButton.click()
assert(values.JAC_MP_ROSTER=='0,1' and values.JAC_MP_RESTORE_DONE==true)
assert(PlayerConfigurations[0].team==3 and PlayerConfigurations[1].team==4)
assert(Controls.ReadyButton.disabled)
rejectedConfigKeys[rejectedCleanup]=nil
ContextPtr.hidden=true
ContextPtr:show()
assert(values.JAC_MP_ROSTER==nil and values.JAC_MP_RESTORE_DONE==nil)
assert(values.JAC_MP_PRIOR_TEAM_0==nil and values.JAC_MP_PRIOR_TEAM_1==nil)
assert(not Controls.ReadyButton.disabled)
Controls.JAC_AllJapanButton.click()
assert(PlayerConfigurations[0].team==PlayerConfigurations[1].team)
Controls.JAC_DifferentButton.click()
assert(PlayerConfigurations[0].team==3 and PlayerConfigurations[1].team==4)
"#,
        );
    }
}

#[test]
fn failed_disconnect_restore_keeps_original_team_until_retry() {
    let lua = fixture();
    exec(
        &lua,
        r#"
Controls.JAC_AllJapanButton.click()
PlayerConfigurations[0].rejectTeam=true
Events.MultiplayerPrePlayerDisconnected.fire(0)
assert(PlayerConfigurations[0].team==4)
assert(values.JAC_MP_PRIOR_TEAM_0==3 and values.JAC_MP_ROSTER=='0,1')
assert(Controls.ReadyButton.disabled)
PlayerConfigurations[0].rejectTeam=false
ContextPtr.hidden=true
ContextPtr:show()
assert(PlayerConfigurations[0].team==3)
assert(values.JAC_MP_PRIOR_TEAM_0==nil and values.JAC_MP_ROSTER=='1')
"#,
    );
}

#[test]
fn rejoined_slot_waits_for_old_baseline_cleanup_then_records_its_own_team() {
    let lua = fixture();
    exec(
        &lua,
        r#"
Controls.JAC_AllJapanButton.click()
participants={0,1,2,3}
Events.MultiplayerPlayerConnected.fire(3)
assert(values.JAC_MP_PRIOR_TEAM_3==5)
rejectedConfigKeys.JAC_MP_PRIOR_TEAM_3=true
Events.MultiplayerPrePlayerDisconnected.fire(3)
assert(values.JAC_MP_RETIRING_3==true and values.JAC_MP_PRIOR_TEAM_3==5)
assert(values.JAC_MP_ROSTER=='0,1')
PlayerConfigurations[3].team=7
Events.MultiplayerPlayerConnected.fire(3)
assert(values.JAC_MP_RETIRING_3==true and Controls.ReadyButton.disabled)
assert(values.JAC_MP_PRIOR_TEAM_3==5)
rejectedConfigKeys.JAC_MP_PRIOR_TEAM_3=nil
ContextPtr.hidden=true
ContextPtr:show()
assert(values.JAC_MP_RETIRING_3==nil and values.JAC_MP_PRIOR_TEAM_3==7)
assert(values.JAC_MP_ROSTER=='0,1,3')
Controls.JAC_DifferentButton.click()
assert(PlayerConfigurations[3].team==7)
"#,
    );
}

#[test]
fn client_connection_event_does_not_create_host_only_pending_state() {
    let lua = fixture();
    exec(
        &lua,
        r#"
Controls.JAC_AllJapanButton.click()
rowsManager.instances[1].AreaPullDown.select(2)
rowsManager.instances[2].AreaPullDown.select(3)
Controls.JAC_SubjectPullDown.select(2)
assert(not Controls.ReadyButton.disabled)
host=false
Events.MultiplayerPlayerConnected.fire(1)
assert(values.JAC_MP_RETIRING_1==nil)
assert(not Controls.ReadyButton.disabled)
"#,
    );
}

#[test]
fn rejected_retiring_marker_recaptures_rejoined_slot_before_reload() {
    let lua = fixture();
    exec(
        &lua,
        r#"
Controls.JAC_AllJapanButton.click()
participants={0,1,2,3}
Events.MultiplayerPlayerConnected.fire(3)
assert(values.JAC_MP_PRIOR_TEAM_3==5)
PlayerConfigurations[3].team=7
rejectedConfigKeys.JAC_MP_RETIRING_3=true
Events.MultiplayerPlayerConnected.fire(3)
assert(values.JAC_MP_RETIRING_3==nil)
assert(values.JAC_MP_ROSTER=='0,1,3' and values.JAC_MP_PRIOR_TEAM_3==7)
Controls.JAC_DifferentButton.click()
assert(PlayerConfigurations[3].team==7)
"#,
    );
}

#[test]
fn string_no_teams_true_cannot_pass_japan_ready_gate() {
    let lua = fixture();
    exec(
        &lua,
        r#"
values.NO_TEAMS='1'
rejectedConfigKeys.NO_TEAMS=true
Controls.JAC_AllJapanButton.click()
assert(values.JAC_MP_PRIOR_NO_TEAMS=='TRUE')
rowsManager.instances[1].AreaPullDown.select(2)
rowsManager.instances[2].AreaPullDown.select(3)
Controls.JAC_SubjectPullDown.select(2)
assert(Controls.ReadyButton.disabled)
"#,
    );
}

#[test]
fn frontend_controls_and_all_prefecture_names_exist_in_three_languages() {
    let xml = read_repo(XML);
    let doc = roxmltree::Document::parse(&xml).unwrap();
    for id in [
        "JAC_AssignmentPanel",
        "JAC_AllJapanButton",
        "JAC_DifferentButton",
        "JAC_AssignmentRows",
        "JAC_SubjectPullDown",
        "JAC_AssignmentStatus",
    ] {
        assert!(
            doc.descendants().any(|n| n.attribute("ID") == Some(id)),
            "{id}"
        );
    }
    assert!(doc.descendants().any(|n| {
        n.has_tag_name("Instance") && n.attribute("Name") == Some("JAC_AssignmentRow")
    }));
    let text = read_repo(TEXT);
    let doc = roxmltree::Document::parse(&text).unwrap();
    let mut labels: HashMap<String, Vec<String>> = HashMap::new();
    for row in doc.descendants().filter(|n| n.has_tag_name("Row")) {
        let Some(tag) = row.attribute("Tag") else {
            continue;
        };
        if tag.starts_with("LOC_JAC_MP_") {
            labels
                .entry(tag.to_owned())
                .or_default()
                .push(row.attribute("Language").unwrap().to_owned());
            let value = row
                .children()
                .find(|n| n.has_tag_name("Text"))
                .and_then(|n| n.text())
                .unwrap_or_default();
            assert!(!value.trim().is_empty(), "{tag}");
        }
    }
    for n in 1..=47 {
        assert!(labels.contains_key(&format!("LOC_JAC_MP_PREF_{n:02}")));
    }
    for (tag, languages) in &labels {
        assert_eq!(languages.len(), 3, "{tag}");
        for language in ["ja_JP", "en_US", "ru_RU"] {
            assert!(languages.iter().any(|l| l == language), "{tag}: {language}");
        }
    }
}
