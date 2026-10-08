//! Runs the shipped gameplay Lua in separate VMs. These are code-level checks,
//! not a claim that two real LAN endpoints or engine synchronization were tested.
use civ6_lua_tests_rs::{new_lua, read_repo};
use mlua::{Lua, LuaSerdeExt, Value};
use serde_json::{json, Value as Json};

const SOURCE: &str = "poc/JAC_PoC_GrowthUI/Scripts/JAC_PoC_GrowthUI_Gameplay.lua";

struct Endpoint(Lua);

impl Endpoint {
    fn new(local: u8, reversed: bool, saved: Option<&Json>) -> Self {
        let lua = new_lua();
        lua.globals().set("localPlayer", local).unwrap();
        lua.globals().set("reversed", reversed).unwrap();
        lua.load(
            r#"
          local function copy(v)
            if type(v) ~= 'table' then return v end
            local r={}; for k,x in pairs(v) do r[k]=copy(x) end; return r
          end
          turn=1; props={}; unitprops={[0]={},[1]={}}; present={[0]=true,[1]=true}
          humans={[0]=true,[1]=true}; inactive={}; network=false; LOG={}
          print=function(line) LOG[#LOG+1]=line end
          GameConfiguration={IsNetworkMultiplayer=function() return network end}
          -- Frontend roster/configuration readers are deliberately absent in Gameplay.
          for owner=0,1 do
            unitprops[owner][65536]={}; unitprops[owner][131073]={}
          end
          Game={GetCurrentGameTurn=function() return turn end,
            GetLocalPlayer=function() return localPlayer end,
            GetProperty=function(_,key) return copy(props[key]) end,
            SetProperty=function(_,key,value) props[key]=copy(value) end}
          Players={}
          for owner=0,1 do
            local pid=owner
            Players[pid]={GetID=function() return pid end, IsHuman=function() return humans[pid] end,
              IsTurnActive=function() return not inactive[pid] end,
              GetUnits=function() return {Members=function()
                local ids=reversed and {131073,65536} or {65536,131073}
                local n=0
                return function()
                  if not present[pid] then return nil end
                  n=n+1; local id=ids[n]; if not id then return nil end
                  return id,{GetID=function() return id end,
                    GetProperty=function(_,key) return copy(unitprops[pid][id][key]) end,
                    SetProperty=function(_,key,value)
                      if failUnit==id then error('unit property write refused') end
                      unitprops[pid][id][key]=copy(value)
                    end}
                end
              end} end}
          end
          PlayerManager={GetAliveMajors=function()
            if reversed then return {Players[1],Players[0]} end
            return {Players[0],Players[1]}
          end}
          Events={LoadScreenClose={Add=function(f) loadscreen=f end}}
          GameEvents={PlayerTurnStartComplete={Add=function(f) beginturn=f end},
            OnPlayerTurnEnded={Add=function(f) endturn=f end},
            JAC_GrowthRoster={Add=function(f) rosteroperation=f end}}
          function export()
            local units={}
            for owner=0,1 do for _,id in ipairs({65536,131073}) do
              units[#units+1]={owner=owner,id=id,properties=copy(unitprops[owner][id])}
            end end
            return {turn=turn,properties=copy(props),units=units}
          end
          function restore(save)
            turn=save.turn; props=copy(save.properties)
            for _,u in ipairs(save.units) do unitprops[u.owner][u.id]=copy(u.properties) end
          end
        "#,
        )
        .exec()
        .unwrap();
        if let Some(saved) = saved {
            lua.globals()
                .set("saved", lua.to_value(saved).unwrap())
                .unwrap();
            lua.load("restore(saved)").exec().unwrap();
        }
        lua.load(read_repo(SOURCE)).exec().unwrap();
        Self(lua)
    }

    fn exec(&self, source: &str) {
        self.0.load(source).exec().unwrap();
    }

    fn state(&self) -> Json {
        let state: Value = self.0.load("return export()").eval().unwrap();
        self.0.from_value(state).unwrap()
    }

    fn record(&self, owner: u8, id: i64) -> Json {
        let record: Value = self
            .0
            .load(format!(
                "return unitprops[{owner}][{id}].JAC_POC_GROWTHUI_WARLORD"
            ))
            .eval()
            .unwrap();
        self.0.from_value(record).unwrap()
    }
}

#[test]
fn independent_vms_agree_despite_local_player_and_enumeration_order() {
    // Gameplay does not currently use GetLocalPlayer. Vary it to prevent a
    // future initialization change from depending on the local client.
    let host = Endpoint::new(0, false, None);
    let guest = Endpoint::new(1, true, None);
    host.exec("loadscreen(); beginturn(0); beginturn(1)");
    guest.exec("loadscreen(); beginturn(1); beginturn(0)");
    assert_eq!(host.state(), guest.state());
    for owner in 0..=1 {
        let first = host.record(owner, 65536);
        let second = host.record(owner, 131073);
        assert_eq!(first["XP"], 20);
        assert_eq!(first["Level"], 1);
        assert_eq!(first["LastXPTurn"], 1);
        assert_eq!(first["Personality"], "JAC_PERSONALITY_BRAVE");
        assert_eq!(second["Personality"], "JAC_PERSONALITY_CALM");
        assert_ne!(first["Equipment"], second["Equipment"]);
    }
}

#[test]
fn duplicate_delivery_does_not_add_xp_or_touch_inactive_players() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("loadscreen()");
    let inactive = endpoint.record(1, 65536);
    endpoint.exec("beginturn(0)");
    let once = endpoint.state();
    endpoint.exec("beginturn(0); beginturn(0)");
    assert_eq!(once, endpoint.state());
    assert_eq!(inactive, endpoint.record(1, 65536));
    endpoint.exec("beginturn(-1); beginturn(99)");
    assert_eq!(once, endpoint.state());
}

#[test]
fn serialized_save_and_fresh_rejoin_preserve_records_and_duplicate_guard() {
    let host = Endpoint::new(0, false, None);
    host.exec("loadscreen(); beginturn(0); beginturn(1)");
    // Cross a real serialization boundary: no Lua tables or VM are shared.
    let bytes = serde_json::to_vec(&host.state()).unwrap();
    let saved: Json = serde_json::from_slice(&bytes).unwrap();
    let restored = Endpoint::new(0, true, Some(&saved));
    let rejoined = Endpoint::new(1, false, Some(&saved));
    restored.exec("loadscreen(); beginturn(0); beginturn(1)");
    rejoined.exec("loadscreen(); beginturn(1); beginturn(0); beginturn(1)");
    assert_eq!(saved, restored.state());
    assert_eq!(saved, rejoined.state());
    restored.exec("turn=2; beginturn(0)");
    rejoined.exec("turn=2; beginturn(0); beginturn(0)");
    assert_eq!(restored.state(), rejoined.state());
    assert_eq!(restored.record(0, 65536)["Level"], 2);
    assert_eq!(restored.record(0, 65536)["XP"], 10);
    assert_eq!(restored.record(0, 65536)["LastXPTurn"], 2);
    assert_eq!(restored.record(1, 65536)["XP"], 20);
    assert_eq!(restored.record(1, 65536)["LastXPTurn"], 1);
    let second_save = restored.state();
    let second_reload = Endpoint::new(1, true, Some(&second_save));
    second_reload.exec("loadscreen(); beginturn(0)");
    assert_eq!(second_save, second_reload.state());
    second_reload.exec("beginturn(1)");
    assert_eq!(second_reload.record(1, 65536)["XP"], 10);
    assert_eq!(second_reload.record(1, 65536)["Level"], 2);
}

#[test]
fn one_ready_human_cannot_finalize_initialization_before_the_other_has_units() {
    for delayed in 0..=1 {
        let endpoint = Endpoint::new(0, false, None);
        endpoint.exec(&format!(
            "present[{delayed}]=false; loadscreen(); beginturn(0)"
        ));
        assert!(endpoint.state()["properties"]["JAC_POC_GROWTHUI_INITIALIZED"].is_null());
        assert!(endpoint.record(0, 65536).is_null());
        let waiting: bool = endpoint
            .0
            .load("return LOG[#LOG]:find('waiting for human',1,true)~=nil")
            .eval()
            .unwrap();
        assert!(waiting);
        endpoint.exec(&format!(
            "present[{delayed}]=true; beginturn(0); beginturn(1)"
        ));
        for owner in 0..=1 {
            assert_eq!(endpoint.record(owner, 65536)["XP"], 20);
            assert_eq!(endpoint.record(owner, 131073)["XP"], 20);
        }
        assert_eq!(
            endpoint.state()["properties"]["JAC_POC_GROWTHUI_INITIALIZED"],
            true
        );
    }
}

#[test]
fn network_load_timing_cannot_write_state_or_finalize_an_incomplete_human_roster() {
    let host = Endpoint::new(0, false, None);
    let peer = Endpoint::new(1, true, None);
    for endpoint in [&host, &peer] {
        endpoint.exec("network=true; humans[1]=false");
    }
    let before = host.state();
    host.exec("loadscreen(); loadscreen()");
    assert_eq!(before, host.state());
    host.exec("beginturn(0)");
    peer.exec("beginturn(0); loadscreen()");
    assert_eq!(host.state(), peer.state());
    assert!(host.record(0, 65536).is_null());
    for endpoint in [&host, &peer] {
        endpoint
            .exec("assert(rosteroperation(0,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0,1'}))");
        assert!(endpoint.record(0, 65536).is_null());
    }
    for endpoint in [&host, &peer] {
        endpoint.exec("humans[1]=true; beginturn(1)");
    }
    assert_eq!(host.state(), peer.state());
    assert_eq!(host.record(1, 65536)["XP"], 20);
}

#[test]
fn units_missing_for_an_ai_do_not_delay_initialization() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("network=true; humans[1]=false; present[1]=false; beginturn(0); assert(rosteroperation(0,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0'}))");
    assert_eq!(
        endpoint.state()["properties"]["JAC_POC_GROWTHUI_INITIALIZED"],
        true
    );
    assert_eq!(endpoint.record(0, 65536)["XP"], 20);
    assert!(endpoint.record(1, 65536).is_null());
}

#[test]
fn gameplay_can_register_turn_handler_without_ui_loadscreen_event() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("network=true; Events.LoadScreenClose=nil; props={}");
    endpoint.0.load(read_repo(SOURCE)).exec().unwrap();
    endpoint.exec("beginturn(0); assert(rosteroperation(0,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0,1'})); beginturn(1)");
    assert_eq!(endpoint.record(1, 65536)["XP"], 20);
}

#[test]
fn absent_ui_roster_does_not_cause_gameplay_to_call_frontend_apis_or_partially_initialize() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("network=true; GameConfiguration.GetParticipatingPlayerIDs=function() error('UI-only API') end; beginturn(0)");
    let missing: bool = endpoint
        .0
        .load("return LOG[#LOG]:find('waiting for synchronized human roster',1,true)~=nil")
        .eval()
        .unwrap();
    assert!(missing);
    assert!(endpoint.state()["properties"]["JAC_POC_GROWTHUI_INITIALIZED"].is_null());
    assert!(endpoint.record(0, 65536).is_null());
    endpoint.exec("assert(rosteroperation(0,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0,1'})); beginturn(1)");
    assert_eq!(endpoint.record(1, 65536)["XP"], 20);
}

#[test]
fn network_save_and_rejoin_keep_xp_even_without_new_initialization_apis() {
    let host = Endpoint::new(0, false, None);
    host.exec("network=true; loadscreen(); beginturn(0); assert(rosteroperation(0,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0,1'})); beginturn(1)");
    let serialized = serde_json::to_vec(&host.state()).unwrap();
    let saved: Json = serde_json::from_slice(&serialized).unwrap();
    for local in 0..=1 {
        let restored = Endpoint::new(local, true, Some(&saved));
        restored.exec("network=true; GameConfiguration.GetParticipatingPlayerIDs=nil; loadscreen(); beginturn(0); beginturn(1)");
        assert_eq!(saved, restored.state());
    }
}

#[test]
fn roster_catches_up_saved_start_receipts_without_losing_the_first_players_xp() {
    let host = Endpoint::new(0, false, None);
    host.exec("network=true; beginturn(0); endturn(0); beginturn(1)");
    let waiting = host.state();
    assert!(host.record(0, 65536).is_null());
    let rejoined = Endpoint::new(1, true, Some(&waiting));
    rejoined.exec("network=true; assert(rosteroperation(1,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0,1'}))");
    for owner in 0..=1 {
        assert_eq!(rejoined.record(owner, 65536)["XP"], 20);
        assert_eq!(rejoined.record(owner, 65536)["LastXPTurn"], 1);
    }
    let ready = rejoined.state();
    rejoined.exec("assert(rosteroperation(1,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0,1'})); beginturn(1)");
    assert_eq!(ready, rejoined.state());
}

#[test]
fn rejects_roster_spoof_stale_turn_malformed_ids_and_missing_known_human() {
    for request in [
        "rosteroperation(1,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0,1'})",
        "rosteroperation(0,{OnStart='JAC_GrowthRoster',Turn=turn-1,Roster='0,1'})",
        "rosteroperation(0,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0'})",
        "rosteroperation(0,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0,,1'})",
        "rosteroperation(0,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='1,0'})",
        "rosteroperation(0,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0,0'})",
        "rosteroperation(0,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0,64'})",
        "rosteroperation(0,{OnStart='JAC_GrowthRoster',Turn=turn,Roster='0,1',Actor=1})",
    ] {
        let endpoint = Endpoint::new(0, false, None);
        endpoint.exec("network=true; beginturn(0)");
        let before = endpoint.state();
        let accepted: bool = endpoint.0.load(format!("return {request}")).eval().unwrap();
        assert!(!accepted, "{request}");
        assert_eq!(before, endpoint.state(), "{request}");
    }
}

#[test]
fn nil_or_inactive_roster_actor_cannot_seed_a_game() {
    for actor in ["nil", "0", "99", "'0'"] {
        let endpoint = Endpoint::new(0, false, None);
        endpoint.exec("network=true");
        let original = endpoint.state();
        let accepted: bool = endpoint
            .0
            .load(format!(
                "return rosteroperation({actor},{{Turn=turn,Roster='0,1'}})"
            ))
            .eval()
            .unwrap();
        assert!(!accepted);
        assert_eq!(original, endpoint.state());
    }
}

#[test]
fn stale_active_receipt_cannot_authorize_a_player_who_already_ended_their_turn() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("network=true; beginturn(0); inactive[0]=true");
    let before = endpoint.state();
    let accepted: bool = endpoint
        .0
        .load("return rosteroperation(0,{Turn=turn,Roster='0,1'})")
        .eval()
        .unwrap();
    assert!(!accepted);
    assert_eq!(before, endpoint.state());
}

#[test]
fn actual_ui_roster_packet_replayed_into_both_gameplay_vms_initializes_once() {
    let host = Endpoint::new(0, false, None);
    let peer = Endpoint::new(1, true, None);
    for endpoint in [&host, &peer] {
        endpoint.exec("network=true; beginturn(0)");
    }
    let ui = new_lua();
    ui.globals()
        .set("PROPS", ui.to_value(&host.state()["properties"]).unwrap())
        .unwrap();
    ui.load(r#"
      GameConfiguration={IsNetworkMultiplayer=function() return true end,
        GetHumanPlayerIDs=function() return {1,62,0} end, GetObserverIDs=function() return {62} end}
      Game={GetLocalPlayer=function() return 0 end,GetCurrentGameTurn=function() return 1 end,
        GetProperty=function(_,key) return PROPS[key] end,SetProperty=function() error('UI wrote state') end}
      ContextPtr={SetHide=function(_,hidden) assert(hidden) end}
      PlayerOperations={EXECUTE_SCRIPT=77}; SENT={}
      Players={[0]={IsHuman=function() return true end,IsTurnActive=function() return true end}}
      UI={GetElapsedTime=function() return 10 end,
        RequestPlayerOperation=function(actor,kind,params)
          assert(kind==77);SENT[#SENT+1]={actor=actor,params=params}
        end}
      Events={LoadScreenClose={Add=function(f) load=f end},
        LocalPlayerTurnBegin={Add=function(f) begin=f end},
        GameCoreEventPublishComplete={Add=function(f) publish=f end}}
    "#).exec().unwrap();
    ui.load(read_repo("poc/JAC_PoC_GrowthUI/UI/JAC_GrowthRoster.lua"))
        .exec()
        .unwrap();
    ui.load("load(); publish(); assert(#SENT==1)")
        .exec()
        .unwrap();
    let packet: Value = ui.load("return SENT[1]").eval().unwrap();
    let packet: Json = ui.from_value(packet).unwrap();
    assert_eq!(packet["actor"], 0);
    assert_eq!(packet["params"]["Roster"], "0,1");
    for endpoint in [&host, &peer] {
        endpoint
            .0
            .globals()
            .set("PACKET", endpoint.0.to_value(&packet).unwrap())
            .unwrap();
        endpoint.exec("assert(rosteroperation(PACKET.actor,PACKET.params)); assert(rosteroperation(PACKET.actor,PACKET.params)); beginturn(1)");
    }
    assert_eq!(host.state(), peer.state());
    for owner in 0..=1 {
        assert_eq!(host.record(owner, 65536)["XP"], 20);
    }
}

#[test]
fn delayed_core_human_flag_does_not_lose_a_configured_humans_first_start() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("network=true; humans[1]=false; beginturn(1); endturn(1); beginturn(0); assert(rosteroperation(0,{Turn=turn,Roster='0,1'})); humans[1]=true; assert(rosteroperation(0,{Turn=turn,Roster='0,1'}))");
    for owner in 0..=1 {
        assert_eq!(endpoint.record(owner, 65536)["XP"], 20);
        assert_eq!(endpoint.record(owner, 65536)["LastXPTurn"], 1);
    }
}

#[test]
fn pending_receipts_drain_after_partial_write_failure_without_losing_or_duplicating_xp() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("network=true; present[1]=false; beginturn(0); assert(rosteroperation(0,{Turn=turn,Roster='0,1'})); present[1]=true; failUnit=131073; assert(not pcall(beginturn,1)); failUnit=nil; assert(props.JAC_POC_GROWTHUI_INITIALIZED==nil)");
    // First retry completes initialization but fails while paying one unit.
    endpoint.exec("local writes=0; local original=Game.SetProperty; Game.SetProperty=function(self,key,value) original(self,key,value); if key=='JAC_POC_GROWTHUI_INITIALIZED' then failUnit=131073 end end; assert(not pcall(beginturn,1)); assert(props.JAC_POC_GROWTHUI_INITIALIZED); Game.SetProperty=original; failUnit=nil; turn=2; beginturn(0)");
    assert_eq!(endpoint.record(0, 65536)["XP"], 10);
    assert_eq!(endpoint.record(0, 131073)["XP"], 10);
    assert_eq!(endpoint.record(1, 65536)["XP"], 20);
    assert_eq!(endpoint.record(1, 131073)["XP"], 20);
    assert!(endpoint.state()["properties"]["JAC_POC_GROWTHUI_PENDING_STARTS"].is_null());
    let once = endpoint.state();
    endpoint.exec("beginturn(0)");
    assert_eq!(once, endpoint.state());
}

#[test]
fn revised_roster_cannot_remove_a_still_human_seat_or_change_saved_pending_receipts() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("network=true; present[1]=false; beginturn(0); assert(rosteroperation(0,{Turn=turn,Roster='0,1'}))");
    let waiting = endpoint.state();
    endpoint.exec("assert(not rosteroperation(0,{Turn=turn,Roster='0'}))");
    assert_eq!(waiting, endpoint.state());
}

#[test]
fn changed_roster_after_ai_takeover_unblocks_initialization_and_saved_pending_growth() {
    let host = Endpoint::new(0, false, None);
    host.exec("network=true; present[1]=false; beginturn(0); assert(rosteroperation(0,{Turn=turn,Roster='0,1'})); turn=2; beginturn(0); humans[1]=false");
    let saved = host.state();
    let peer = Endpoint::new(1, true, Some(&saved));
    peer.exec("network=true; humans[1]=false; present[1]=false");
    for endpoint in [&host, &peer] {
        endpoint.exec("assert(rosteroperation(0,{Turn=turn,Roster='0'}))");
        assert_eq!(endpoint.record(0, 65536)["XP"], 10);
        assert_eq!(endpoint.record(0, 65536)["Level"], 2);
        assert_eq!(endpoint.record(0, 65536)["LastXPTurn"], 2);
        assert!(endpoint.record(1, 65536).is_null());
    }
    assert_eq!(host.state(), peer.state());
    let ready = host.state();
    host.exec("beginturn(0); assert(rosteroperation(0,{Turn=turn,Roster='0'}))");
    assert_eq!(ready, host.state());
}

#[test]
fn long_roster_wait_compacts_major_receipts_and_applies_only_human_growth() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("network=true; humans[1]=false; for t=1,100 do turn=t; beginturn(0); beginturn(0); beginturn(1) end");
    let rows = endpoint.state()["properties"]["JAC_POC_GROWTHUI_PENDING_STARTS"].clone();
    assert_eq!(rows.as_object().unwrap().len(), 2);
    assert_eq!(rows["0"]["starts"], 100);
    assert_eq!(rows["1"]["starts"], 100);
    let peer = Endpoint::new(1, true, Some(&endpoint.state()));
    peer.exec("network=true; humans[1]=false; assert(rosteroperation(0,{Turn=turn,Roster='0'}))");
    // Reference the existing normal per-turn path for XP and level arithmetic.
    let reference = Endpoint::new(0, false, None);
    reference.exec("humans[1]=false; loadscreen(); for t=1,100 do turn=t; beginturn(0) end");
    assert_eq!(peer.record(0, 65536), reference.record(0, 65536));
    assert!(peer.record(1, 65536).is_null());
    assert!(peer.state()["properties"]["JAC_POC_GROWTHUI_PENDING_STARTS"].is_null());
}

#[test]
fn legacy_pending_rows_restore_without_losing_distinct_start_receipts() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("network=true; turn=3; props.JAC_POC_GROWTHUI_PENDING_STARTS={['1:0']={owner=0,turn=1},['2:0']={owner=0,turn=2}}; beginturn(0); assert(rosteroperation(0,{Turn=turn,Roster='0,1'}))");
    assert_eq!(endpoint.record(0, 65536)["XP"], 30);
    assert_eq!(endpoint.record(0, 65536)["Level"], 2);
    assert_eq!(endpoint.record(0, 65536)["LastXPTurn"], 3);
}

#[test]
fn restored_level_cap_and_all_metadata_survive_duplicate_turns() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("loadscreen(); beginturn(0)");
    let mut saved = endpoint.state();
    let record = &mut saved["units"][0]["properties"]["JAC_POC_GROWTHUI_WARLORD"];
    record["Level"] = json!(30);
    record["XP"] = json!(100);
    record["LastXPTurn"] = json!(0);
    record["Equipment"] = json!(["item,b", "item-a"]);
    record["Artifact"] = json!("unique-relic");
    let rejoined = Endpoint::new(1, true, Some(&saved));
    rejoined.exec("loadscreen(); beginturn(0)");
    let once = rejoined.state();
    assert_eq!(rejoined.record(0, 65536)["Level"], 30);
    assert_eq!(rejoined.record(0, 65536)["XP"], 120);
    assert_eq!(
        rejoined.record(0, 65536)["Equipment"],
        json!(["item,b", "item-a"])
    );
    assert_eq!(rejoined.record(0, 65536)["Artifact"], "unique-relic");
    rejoined.exec("beginturn(0)");
    assert_eq!(once, rejoined.state());
}
