mod support_driver;
use civ6_autotest_rs::{
    scenario::{self, SessionType},
    Error,
};
use mlua::Lua;
use serde_json::{json, Value};
use support_driver::{FakeBackend, Harness};

fn configuration() -> Value {
    json!({"action":"configure","ruleset":"RULESET_EXPANSION_2","map_size":"MAPSIZE_DUEL","speed":"GAMESPEED_STANDARD","seed":620905,"players":[
        {"leader":"LEADER_CLEOPATRA","civ":"CIVILIZATION_EGYPT","human":true},
        {"leader":"LEADER_GILGAMESH","civ":"CIVILIZATION_SUMERIA","human":true}
    ]})
}

#[test]
fn two_humans_require_hotseat_and_unknown_modes_are_rejected() {
    let mut data = json!({"name":"two humans","steps":[configuration()]});
    assert!(scenario::validate_scenario(&data).is_err());
    data["session_type"] = "hotseat".into();
    scenario::validate_scenario(&data).unwrap();
    for mode in [json!(null), json!(true), json!("lan"), json!("HOTSEAT")] {
        data["session_type"] = mode;
        assert!(scenario::validate_scenario(&data).is_err());
    }
    data["session_type"] = "hotseat".into();
    data["steps"][0]["players"][1]["human"] = false.into();
    assert!(scenario::validate_scenario(&data).is_err());
    data["steps"] = json!([{"action":"autoplay","turns":1}]);
    assert!(scenario::validate_scenario(&data)
        .unwrap_err()
        .to_string()
        .contains("autoplay"));
}

#[test]
fn frontend_retry_rechecks_owner_before_sending() {
    let fake = FakeBackend::new().lua(|_, _| Err(Error::tuner("not sent")));
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    let mut checks = 0;
    let result = h
        .runner
        .send_frontend_transition_checked("Network.HostGame(1)", 5.0, || {
            checks += 1;
            if checks > 1 {
                Err(Error::input("owner changed"))
            } else {
                Ok(())
            }
        });
    assert!(result.unwrap_err().to_string().contains("owner changed"));
    assert_eq!(checks, 2);
    assert_eq!(h.count_code("HostGame"), 1);
}

#[test]
fn frontend_retry_stops_after_unknown_result() {
    let mut attempts = 0;
    let fake = FakeBackend::new().lua(move |_, _| {
        attempts += 1;
        if attempts < 3 {
            Err(Error::tuner("not sent"))
        } else {
            Err(Error::unknown("sent; reply lost"))
        }
    });
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    let mut checks = 0;
    let error = h
        .runner
        .send_frontend_transition_checked("Network.LoadGame({})", 5.0, || {
            checks += 1;
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.kind, civ6_autotest_rs::ErrorKind::LuaResponseUnknown);
    assert_eq!(checks, 3);
    assert_eq!(h.count_code("LoadGame"), 3);
    assert!(h.clock.at() > 0.0 && h.clock.at() < 5.0);
}

#[test]
fn named_state_transition_preserves_the_state_and_never_replays_sent_commands() {
    for error in [
        Error::unknown("sent; reply lost"),
        Error::lua("execution failed"),
    ] {
        let kind = error.kind;
        let mut h = Harness::new(
            json!({}),
            FakeBackend::new().lua(move |_, _| Err(error.clone())),
        );
        let result = h
            .runner
            .send_state_transition_checked("HostGame", "pool RANDOM", 5.0, || Ok(()));
        assert_eq!(result.unwrap_err().kind, kind);
        assert_eq!(h.calls(), [("HostGame".into(), "pool RANDOM".into())]);
    }
    let mut h = Harness::new(json!({}), FakeBackend::new());
    assert!(h
        .runner
        .send_state_transition_checked("HostGame", "pool RANDOM", 0.0, || Ok(()))
        .is_err());
    assert!(h.calls().is_empty());
    let mut attempts = 0;
    let fake = FakeBackend::new().lua(move |_, _| {
        attempts += 1;
        if attempts == 1 {
            Err(Error::tuner("not sent"))
        } else {
            Ok("true".into())
        }
    });
    let mut h = Harness::new(json!({}), fake);
    assert_eq!(
        h.runner
            .send_state_transition_checked("HostGame", "pool RANDOM", 5.0, || Ok(()))
            .unwrap(),
        "true"
    );
    assert_eq!(
        h.calls(),
        [
            ("HostGame".into(), "pool RANDOM".into()),
            ("HostGame".into(), "pool RANDOM".into())
        ]
    );
}

#[test]
fn setup_reconnect_does_not_replay_an_acknowledged_phase() {
    let mut failures = 0;
    let fake = FakeBackend::new().lua(move |_, code| {
        if code.contains("JAC_AUTOTEST") && failures < 2 {
            failures += 1;
            return Err(Error::tuner("second phase was not sent"));
        }
        Ok("true".into())
    });
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    let mut checks = 0;
    h.runner
        .run_frontend_setup_step_checked(&configuration(), || {
            checks += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(h.count_code("SetToDefaults"), 1);
    assert_eq!(h.count_code("JAC_AUTOTEST"), 3);
    assert_eq!(checks, 4);
    assert!(h
        .runner
        .run_frontend_setup_step_checked(&json!({"action":"end_turn"}), || Ok(()))
        .is_err());
}

#[test]
fn original_hotseat_scenario_excludes_unlisted_user_mods() {
    let original: Value = serde_json::from_str(include_str!(
        "../../civ6-autotest/scenarios/multiplayer-hotseat-growth.json"
    ))
    .unwrap();
    let fake = FakeBackend::new().lua(|_, code| {
        let lua = Lua::new();
        lua.load(r#"
            UI = {IsInFrontEnd=function() return true end}
            GameConfiguration = {UpdateEnabledMods=function() end}
            mods = {
                growth={Id='7696e937-2044-4f3c-8e13-4e27632e2710',Enabled=false,Official=false},
                extra={Id='unlisted-user-mod',Enabled=true,Official=false},
                dlc={Id='official-dlc',Enabled=true,Official=true}
            }
            Modding = {
                GetInstalledMods=function() return {{Handle='growth'},{Handle='extra'},{Handle='dlc'}} end,
                GetModInfo=function(handle) return mods[handle] end,
                GetModAssociations=function() return {} end,
                GetModHandle=function(id)
                    for handle,info in pairs(mods) do if info.Id==id then return handle end end
                end,
                CanDisableMod=function() return 'OK' end,
                CanEnableMod=function() return 'OK' end,
                DisableMod=function(handle) mods[handle].Enabled=false end,
                EnableMod=function(handle) mods[handle].Enabled=true end
            }
        "#).exec().unwrap();
        let result = lua.load(code).eval::<String>().unwrap();
        assert!(lua.load("return mods.growth.Enabled").eval::<bool>().unwrap());
        assert!(!lua.load("return mods.extra.Enabled").eval::<bool>().unwrap());
        assert!(lua.load("return mods.dlc.Enabled").eval::<bool>().unwrap());
        Ok(result)
    });
    let mut h = Harness::new(original.clone(), fake);
    h.runner.run_step(&original["steps"][0]).unwrap();
}

#[test]
fn load_uses_hotseat_mode_before_resolving_the_engine_save_type() {
    let lua = Lua::new();
    lua.load(
        r#"
        sequence = {}; mode = 'single'; loaded = nil
        UI = {IsInFrontEnd = function() return true end}
        GameModeTypes = {HOTSEAT = 7}
        GameConfiguration = {SetToDefaults = function(value)
            assert(value == 7); mode = 'hotseat'; table.insert(sequence, 'mode')
        end}
        Network = {
            GetGameConfigurationSaveType = function()
                assert(mode == 'hotseat'); table.insert(sequence, 'type'); return 23
            end,
            LoadGame = function(save, server)
                assert(save.Name == 'mid-turn' and save.Type == 23 and server == 19)
                loaded = save; table.insert(sequence, 'load'); return true
            end
        }
        SaveLocations = {LOCAL_STORAGE = 5}; SaveDirectories = {DEFAULT = 8}
        ServerType = {SERVER_TYPE_HOTSEAT = 19}
    "#,
    )
    .exec()
    .unwrap();
    assert!(lua
        .load(scenario::load_code("mid-turn", SessionType::Hotseat))
        .eval::<bool>()
        .unwrap());
    assert_eq!(
        lua.load("return table.concat(sequence, ',')")
            .eval::<String>()
            .unwrap(),
        "mode,type,load"
    );
    lua.load("Network.LoadGame = function() return false end")
        .exec()
        .unwrap();
    assert!(lua
        .load(scenario::load_code("mid-turn", SessionType::Hotseat))
        .eval::<bool>()
        .is_err());
    assert_eq!(SessionType::Hotseat.save_folder(), "Hotseat");
    assert_eq!(SessionType::SinglePlayer.save_folder(), "Single");
}

#[test]
fn hotseat_handoff_does_not_wait_for_a_whole_round_or_end_twice() {
    let mut ended = false;
    let fake = FakeBackend::new()
        .states(|| Ok(vec!["InGame".into(), "PlayerChange".into()]))
        .lua(move |_, code| {
            if code == "return Game.GetCurrentGameTurn()" {
                return Ok("7".into());
            }
            if code == "return Game.GetLocalPlayer()" {
                return Ok("0".into());
            }
            if code.contains("UI.RequestAction(ActionTypes.ACTION_ENDTURN)") {
                assert!(!ended);
                ended = true;
            }
            if code.contains("local p = Players[id]") && code.contains("id ~= 0") {
                assert!(ended);
                assert!(code.contains("turn >= 7"));
                assert!(code.contains("p:IsTurnActive()"));
            }
            if code == "return tostring(Game.GetCurrentGameTurn()) .. '|' .. tostring(Game.GetLocalPlayer())" {
                return Ok("7|1".into());
            }
            Ok("true".into())
        });
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    assert_eq!(
        h.runner.end_turn(30.0, None).unwrap(),
        "hotseat turn/player 7|0->7|1"
    );
    assert_eq!(
        h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
        1
    );
    assert_eq!(h.count_code("OnKeyUp_Return()"), 2);
    let confirmations: Vec<_> = h
        .calls()
        .into_iter()
        .filter(|(_, code)| code.contains("OnKeyUp_Return()"))
        .map(|(_, code)| code.lines().next().unwrap().to_string())
        .collect();
    assert_eq!(
        confirmations,
        ["local requireDialog = false", "local requireDialog = true"]
    );
}

#[test]
fn handoff_timeout_never_reissues_endturn() {
    let fake = FakeBackend::new()
        .states(|| Ok(vec!["InGame".into(), "PlayerChange".into()]))
        .lua(|_, code| {
            if code == "return Game.GetCurrentGameTurn()" {
                return Ok("7".into());
            }
            if code == "return Game.GetLocalPlayer()" {
                return Ok("0".into());
            }
            if code.contains("id ~= 0") {
                return Ok("false".into());
            }
            Ok("true".into())
        });
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    assert!(h.runner.end_turn(3.0, None).is_err());
    assert_eq!(
        h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
        1
    );
}

#[test]
fn password_error_stops_before_sending_endturn() {
    let fake = FakeBackend::new()
        .states(|| Ok(vec!["InGame".into(), "PlayerChange".into()]))
        .lua(|state, code| {
            if state == "PlayerChange" && code.contains("OnKeyUp_Return()") {
                return Err(Error::lua(
                    "password-protected hotseat needs manual start-turn",
                ));
            }
            Ok("true".into())
        });
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    assert!(h
        .runner
        .end_turn(3.0, None)
        .unwrap_err()
        .to_string()
        .contains("password-protected"));
    assert_eq!(h.count_code("ACTION_ENDTURN"), 0);
}

#[test]
fn configure_routes_the_explicit_session_mode() {
    let mut h = Harness::new(json!({"session_type":"hotseat"}), FakeBackend::new());
    h.runner.run_step(&configuration()).unwrap();
    assert!(h.calls()[0]
        .1
        .contains("SetToDefaults(GameModeTypes.HOTSEAT)"));
    assert!(h.calls()[0].1.contains("GameConfiguration.IsHotseat()"));
    assert_eq!(
        h.calls()[0]
            .1
            .matches("p:SetSlotStatus(SlotStatus.SS_TAKEN)")
            .count(),
        2
    );
}

#[test]
fn start_turn_requires_an_active_human_and_an_unpaused_game() {
    use civ6_autotest_rs::runner::HOTSEAT_START_TURN_CODE;
    let relaxed = format!("local requireDialog = false\n{HOTSEAT_START_TURN_CODE}");
    let strict = format!("local requireDialog = true\n{HOTSEAT_START_TURN_CODE}");
    let lua = Lua::new();
    lua.load(
        r#"
      hidden=true; active=true; human=true; paused=true; password=''; presses=0
      Game={GetLocalPlayer=function() return 1 end, GetCurrentGameTurn=function() return 7 end}
      Players={[1]={IsHuman=function() return human end, IsTurnActive=function() return active end}}
      PlayerConfigurations={[1]={GetHotseatPassword=function() return password end}}
      GameConfiguration={IsPaused=function() return paused end}
      ContextPtr={IsHidden=function() return hidden end}
      function OnKeyUp_Return() presses=presses+1; hidden=true; paused=false end
    "#,
    )
    .exec()
    .unwrap();
    assert!(
        !lua.load(&relaxed).eval::<bool>().unwrap(),
        "hidden paused screen must not pass"
    );
    for state in ["paused=false; active=false", "active=true; human=false"] {
        lua.load(state).exec().unwrap();
        assert!(!lua.load(&relaxed).eval::<bool>().unwrap());
    }
    lua.load("human=true; active=true; paused=true; hidden=false; password='secret'")
        .exec()
        .unwrap();
    assert!(lua.load(&relaxed).eval::<bool>().is_err());
    assert_eq!(lua.globals().get::<i64>("presses").unwrap(), 0);
    lua.load("password=''").exec().unwrap();
    assert!(lua.load(&relaxed).eval::<bool>().unwrap());
    assert_eq!(lua.globals().get::<i64>("presses").unwrap(), 1);
    assert!(lua.load(&relaxed).eval::<bool>().unwrap());
    assert_eq!(lua.globals().get::<i64>("presses").unwrap(), 1);
    lua.load("JAC_AUTOTEST_HOTSEAT_CONFIRMATION=nil; hidden=true; paused=false")
        .exec()
        .unwrap();
    assert!(
        !lua.load(&strict).eval::<bool>().unwrap(),
        "unobserved hidden dialog must not pass after load"
    );
    lua.load("hidden=false; paused=true").exec().unwrap();
    assert!(lua.load(&strict).eval::<bool>().unwrap());
    assert_eq!(lua.globals().get::<i64>("presses").unwrap(), 2);
    assert!(lua.load(&strict).eval::<bool>().unwrap());
    assert_eq!(lua.globals().get::<i64>("presses").unwrap(), 2);
}

#[test]
fn growth_snapshot_rejects_inert_growth_and_duplicate_credit() {
    let scenario: Value = serde_json::from_str(include_str!(
        "../../civ6-autotest/scenarios/multiplayer-hotseat-growth.json"
    ))
    .unwrap();
    let code = scenario["steps"][4]["code"].as_str().unwrap();
    let lua = Lua::new();
    lua.load(r#"
      turn=0; localID=0; records={}; units={}; Players={}
      Game={GetLocalPlayer=function() return localID end, GetCurrentGameTurn=function() return turn end,
        GetProperty=function(_, key) assert(key=='JAC_POC_GROWTHUI_INITIALIZED'); return true end}
      GameConfiguration={IsHotseat=function() return true end}
      for pid=0,1 do
        units[pid]={}; records[pid]={}
        for id=1,2 do
          local r={Level=1, XP=0, Personality='brave', Trait='valiant', Equipment={'blade'}}
          records[pid][id]=r
          units[pid][id]={GetID=function() return id end, GetProperty=function() return r end}
        end
        Players[pid]={IsHuman=function() return true end,
          GetUnits=function() return {Members=function() return ipairs(units[pid]) end} end}
      end
      function credit(pid, t, total)
        for _,r in ipairs(records[pid]) do
          r.LastXPTurn=t; r.Level=total>=30 and 2 or 1; r.XP=total>=30 and total-30 or total
        end
      end
    "#).exec().unwrap();
    lua.load(code).eval::<String>().unwrap();
    lua.load("localID=1").exec().unwrap();
    assert!(
        lua.load(code).eval::<String>().is_err(),
        "active human without a credited turn must fail"
    );
    lua.load("credit(1,0,20)").exec().unwrap();
    let once = lua.load(code).eval::<String>().unwrap();
    assert_eq!(
        lua.load(code).eval::<String>().unwrap(),
        once,
        "same turn does not add experience"
    );
    lua.load("credit(1,0,40)").exec().unwrap();
    assert!(
        lua.load(code).eval::<String>().is_err(),
        "same stamp with extra XP must fail"
    );
    lua.load("credit(1,0,20); turn=1; localID=0; credit(0,1,20)")
        .exec()
        .unwrap();
    lua.load(code).eval::<String>().unwrap();
    lua.load("localID=1; credit(1,1,40)").exec().unwrap();
    let next = lua.load(code).eval::<String>().unwrap();
    lua.load("JAC_HOTSEAT_GROWTH_PREVIOUS=nil").exec().unwrap();
    assert_eq!(
        lua.load(code).eval::<String>().unwrap(),
        next,
        "new UI after load returns the identical gameplay record"
    );
    lua.load("turn=2; credit(1,2,80)").exec().unwrap();
    assert!(
        lua.load(code).eval::<String>().is_err(),
        "two credits in one new turn must fail"
    );
}

fn staging_baseline(h: &mut Harness) {
    for name in civ6_autotest_rs::judge::LOG_NAMES {
        std::fs::write(h.runner.logs.join(name), "").unwrap();
    }
    h.runner.baseline = Some(civ6_autotest_rs::judge::snapshot_logs(&h.runner.logs).unwrap());
}

#[test]
fn staging_launches_once_and_never_replays_an_unknown_button_result() {
    let fake = FakeBackend::new()
        .states(|| Ok(vec!["StagingRoom".into()]))
        .lua(|_, code| {
            if code.contains("DoLeftClick()") {
                return Err(Error::unknown("launch acknowledgement lost"));
            }
            Ok("true".into())
        });
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    staging_baseline(&mut h);
    assert!(h.runner.launch_hotseat_staging(0, 5.0).unwrap());
    assert_eq!(h.count_code("DoLeftClick()"), 1);
}

#[test]
fn disabled_staging_button_is_not_clicked_and_loading_skips_the_lobby() {
    let fake = FakeBackend::new()
        .states(|| Ok(vec!["StagingRoom".into()]))
        .lua(|_, _| Ok("false".into()));
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    staging_baseline(&mut h);
    assert!(h.runner.launch_hotseat_staging(0, 1.0).is_err());
    assert_eq!(h.count_code("DoLeftClick()"), 0);
    std::fs::write(
        h.runner.logs.join("Lua.log"),
        "LoadScreen: OnLoadGameViewStateDone\n",
    )
    .unwrap();
    let calls = h.calls().len();
    assert!(!h.runner.launch_hotseat_staging(0, 5.0).unwrap());
    assert_eq!(h.calls().len(), calls);
}

#[test]
fn staging_reconnects_before_queries_and_before_the_unsent_launch() {
    let mut states = 0;
    let fake = FakeBackend::new().states(move || {
        states += 1;
        if [1, 2, 4, 5, 8, 9].contains(&states) {
            Err(Error::tuner(
                "connection refused during frontend transition",
            ))
        } else {
            Ok(vec!["StagingRoom".into()])
        }
    });
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    staging_baseline(&mut h);
    assert!(!h.runner.launch_hotseat_staging(0, 5.0).unwrap());
    assert_eq!(h.count_code("DoLeftClick()"), 1);
    assert!(h.clock.at() > 0.0 && h.clock.at() < 5.0);
}

#[test]
fn staging_permanent_disconnect_expires_without_launching() {
    let fake = FakeBackend::new().states(|| Err(Error::tuner("connection refused")));
    let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
    staging_baseline(&mut h);
    let error = h.runner.launch_hotseat_staging(0, 1.0).unwrap_err();
    assert!(error.to_string().contains("Timed out"));
    assert!(error.to_string().contains("connection refused"));
    assert_eq!(h.count_code("DoLeftClick()"), 0);
    assert_eq!(h.clock.at(), 1.0);
}

#[test]
fn staging_does_not_retry_interruption_or_multiple_process_errors() {
    for error in [
        Error::interrupted("stop"),
        Error::scenario("multiple Civ6 processes"),
    ] {
        let expected = error.clone();
        let fake = FakeBackend::new().states(move || Err(error.clone()));
        let mut h = Harness::new(json!({"session_type":"hotseat"}), fake);
        staging_baseline(&mut h);
        assert_eq!(
            h.runner.launch_hotseat_staging(0, 5.0).unwrap_err(),
            expected
        );
        assert_eq!(h.clock.at(), 0.0);
        assert_eq!(h.count_code("DoLeftClick()"), 0);
    }
}

#[test]
fn process_inspection_consumes_the_state_request_budget() {
    for inspection_seconds in [0.4, 1.0] {
        let mut h = Harness::new(json!({"session_type":"hotseat"}), FakeBackend::new());
        let clock = h.clock.clone();
        let mut fake = FakeBackend::new();
        let events = fake.events.clone();
        fake.on_require = Box::new(move || {
            clock.advance(inspection_seconds);
            Ok(())
        });
        if inspection_seconds == 0.4 {
            let clock = h.clock.clone();
            fake.on_states = Box::new(move || {
                clock.advance(0.6);
                Err(Error::tuner("state response took the remaining budget"))
            });
        }
        h.runner.backend = Box::new(fake);
        let result = h.runner.states(1.0);
        assert!(result.is_err());
        let connections = events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| *e == "connect")
            .count();
        if inspection_seconds == 1.0 {
            assert_eq!(connections, 0, "expired inspection must not connect");
        } else {
            assert_eq!(
                connections, 1,
                "the first state query used the remaining budget"
            );
        }
        assert_eq!(h.clock.at(), 1.0);
    }
}
