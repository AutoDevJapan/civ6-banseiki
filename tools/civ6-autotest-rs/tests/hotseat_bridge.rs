use civ6_autotest_rs::{hotseat_bridge as hs, judge, scenario, ErrorKind, Result};
use mlua::Lua;
use serde_json::{json, Value};
use std::{
    fs,
    sync::{Arc, Mutex, OnceLock},
};

fn test_nonce() -> &'static str {
    static VALUE: OnceLock<String> = OnceLock::new();
    VALUE
        .get_or_init(|| uuid::Uuid::new_v4().simple().to_string())
        .as_str()
}

fn original() -> Value {
    serde_json::from_str(include_str!(
        "../../civ6-autotest/scenarios/multiplayer-hotseat-growth.json"
    ))
    .unwrap()
}

fn banseiki() -> Value {
    serde_json::from_str(include_str!(
        "../../civ6-autotest/scenarios/multiplayer-banseiki-hotseat-growth.json"
    ))
    .unwrap()
}

fn japan_settlers() -> Value {
    serde_json::from_str(include_str!(
        "../../civ6-autotest/scenarios/multiplayer-japan-15km-settlers-hotseat-growth.json"
    ))
    .unwrap()
}

fn japan_settlers_source() -> Value {
    serde_json::from_str(include_str!(
        "../../civ6-autotest/scenarios/japan-map-15km-settlers.json"
    ))
    .unwrap()
}

#[test]
fn persisted_process_creation_time_preserves_every_bit() {
    for bits in [0x41dab117910d2b48_u64, 0x41dab117910d2b49_u64] {
        let owner = hs::WindowOwner {
            pid: 57148,
            started: f64::from_bits(bits),
            hwnd: 2625186,
        };
        let stored = serde_json::to_string(&owner).unwrap();
        let restored: hs::WindowOwner = serde_json::from_str(&stored).unwrap();
        assert_eq!(restored.started.to_bits(), bits);
        assert_eq!(restored, owner);
    }
    let saved: hs::WindowOwner =
        serde_json::from_str(r#"{"pid":57148,"started":1791254084.2057667,"hwnd":2625186}"#)
            .unwrap();
    assert_eq!(saved.started.to_bits(), 0x41dab117910d2b48);
}

fn lua_run_with_growth(xp: u32, before_next_round_capture: &str) -> (Vec<hs::Frame>, String) {
    let nonce = test_nonce();
    let output = tempfile::tempdir().unwrap();
    let bundle = hs::prepare(
        &original(),
        output.path(),
        test_nonce(),
        "78ea4ad5-d526-4ca0-bd0e-cc4519261719",
    )
    .unwrap();
    let plan = fs::read_to_string(
        output
            .path()
            .join("JAC_Test_HotseatBridge/UI/JAC_HotseatPlan.lua"),
    )
    .unwrap();
    let witness = fs::read_to_string(
        output
            .path()
            .join("JAC_Test_HotseatBridge/UI/JAC_HotseatWitness.lua"),
    )
    .unwrap();
    assert!(bundle.mod_folder.contains("JAC_Test_HotseatBridge"));
    assert!(plan.contains(original()["steps"][4]["code"].as_str().unwrap()));
    let lua = Lua::new();
    let lines = Arc::new(Mutex::new(Vec::new()));
    let captured = lines.clone();
    lua.globals()
        .set(
            "print",
            lua.create_function(move |_, line: String| {
                captured.lock().unwrap().push(line);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
    lua.load(format!(r#"
      runNonce = '{nonce}'
      localID = 0; turn = 0; dialogVisible = false; paused = false; dialogReady = false
      GameConfiguration = {{IsHotseat=function() return true end, GetValue=function(key) if key == 'JAC_HOTSEAT_BRIDGE_NONCE' then return runNonce end end,
        GetStartTurn=function() return 0 end, IsPaused=function() return paused end}}
      Game = {{GetLocalPlayer=function() return localID end, GetCurrentGameTurn=function() return turn end,
        GetProperty=function(_, key) if key == 'JAC_POC_GROWTHUI_INITIALIZED' then return true end end}}
      local records = {{}}
      for pid=0,1 do
        records[pid] = {{}}
        for uid=1,2 do
          local id = uid + pid*10
          local r = {{Level=1, XP=0, Personality='calm', Trait='steady', Equipment={{'spear'}}, Artifact=nil, LastXPTurn=0}}
          records[pid][uid] = {{GetID=function() return id end, GetProperty=function(_, key) if key == 'JAC_POC_GROWTHUI_WARLORD' then return r end end}}
        end
      end
      growthRecords = records
      Players = {{}}
      for pid=0,1 do
        Players[pid] = {{IsHuman=function() return true end, IsTurnActive=function() return pid==localID end,
          GetUnits=function() return {{Members=function() local i=0; return function() i=i+1; if i<=2 then return i,records[pid][i] end end end}} end}}
      end
      UIManager = {{IsInPopupQueue=function() return dialogVisible end}}
      PlayerConfigurations = {{[0]={{GetHotseatPassword=function() return '' end}},
        [1]={{GetHotseatPassword=function() return '' end}}}}
      ContextPtr = {{LookUpControl=function(_, path)
          if path == '/InGame/PlayerChange' then return {{IsHidden=function() return not dialogVisible end}} end
          if path == '/InGame/PlayerChange/PopupAlphaIn' then return {{IsHidden=function() return not dialogReady end}} end
          if path == '/InGame/PlayerChange/OkButton' then return {{IsHidden=function() return false end,
            IsDisabled=function() return not dialogReady end}} end
        end,
        SetHide=function(_, hidden) contextHidden=hidden end,
        SetInputHandler=function(_, f) inputHandler=f end, SetUpdate=function(_, f) updateHandler=f end}}
      KeyEvents = {{KeyUp=1}}
      -- F13=65 was measured in Civ6; the remaining distinct values are test fixtures.
      Keys = {{VK_RETURN=38}}
      for f=13,24 do Keys['VK_F'..f]=f+52 end
      ActionTypes = {{ACTION_ENDTURN=5}}; actions=0
      UI = {{RequestAction=function(id) assert(id==5); actions=actions+1; return true end}}
      SaveLocations={{LOCAL_STORAGE=8}}; saveType=0; savedName=nil
      Events = {{SaveComplete={{Add=function(f) saveCallback=f end, Remove=function() saveCallback=nil end}},
        ExitToMainMenu=function() exits=(exits or 0)+1 end}}
      Network = {{GetGameConfigurationSaveType=function() saveType=saveType+1; return 23 end,
        SaveGame=function(opts) assert(opts.Type==23 and opts.Location==8); savedName=opts.Name; if saveCallback then saveCallback() end; return true end}}
      include=function(name) assert(name=='JAC_HotseatPlan') end
      function key(v) local mapped=Keys['VK_F'..tostring(v-111)]; assert(mapped); local input={{GetMessageType=function() return 1 end, GetKey=function() return mapped end}}; return inputHandler(input) end
    "#)).exec().unwrap();
    lua.load(&plan).exec().unwrap();
    lua.load(&witness).exec().unwrap();
    lua.load("assert(contextHidden == false)").exec().unwrap();
    lua.load(format!("updateHandler(); assert(key(124)); assert(key(125)); assert(key(125)); assert(key(133)); assert(key(133)); localID=1; updateHandler(); assert(key(126)); assert(key(127)); assert(key(134)); assert(savedName=='JAC-Hotseat-Mid-Round-'..runNonce); assert(saveType==1); JAC_HOTSEAT_GROWTH_PREVIOUS=nil; assert(key(128)); localID=0; turn=1; for uid=1,2 do local r=growthRecords[0][uid]:GetProperty('JAC_POC_GROWTHUI_WARLORD'); r.XP={xp}; r.LastXPTurn=1 end; assert(key(129)); {before_next_round_capture}; assert(key(130)); localID=1; for uid=1,2 do local r=growthRecords[1][uid]:GetProperty('JAC_POC_GROWTHUI_WARLORD'); r.XP=20; r.LastXPTurn=1 end; assert(key(131)); assert(key(134)); assert(savedName=='JAC-Hotseat-Next-Player-'..runNonce); JAC_HOTSEAT_GROWTH_PREVIOUS=nil; assert(key(132))")).exec().unwrap();
    lua.load("dialogVisible=true; dialogReady=false; paused=false; updateHandler(); dialogReady=true; paused=true; updateHandler(); dialogVisible=false; dialogReady=false; updateHandler(); paused=false; updateHandler(); updateHandler()").exec().unwrap();
    let log = lines.lock().unwrap().join("\n");
    (hs::parse_frames(&log, test_nonce()).unwrap(), log)
}

fn lua_run() -> (Vec<hs::Frame>, String) {
    lua_run_with_growth(20, "do end")
}

#[test]
fn actual_scenario_lua_runs_in_lua51_and_witness_returns_real_fields() {
    let (frames, _) = lua_run();
    let boot = frames.iter().find(|f| f.kind == "BOOT").unwrap();
    let snapshot = hs::WitnessSnapshot::parse(&boot.body).unwrap();
    assert_eq!(
        (snapshot.turn, snapshot.local, snapshot.hotseat),
        (0, 0, true)
    );
    assert_eq!(snapshot.fields.len(), 49);
    assert_eq!(snapshot.fields[11], "number:0");
    assert_eq!(snapshot.fields[12], "number:1");
    assert_eq!(snapshot.fields[14], "number:0");
    assert_eq!(snapshot.fields[18], "nil");
    assert!(frames.iter().any(|f| f.kind == "DIALOG_WAIT"));
    assert!(frames.iter().any(|f| f.kind == "DIALOG_OPEN"));
    assert!(frames.iter().any(|f| f.kind == "DIALOG_CLOSED"));
    let closed = frames.iter().find(|f| f.kind == "DIALOG_CLOSED").unwrap();
    assert!(hs::WitnessSnapshot::parse(&closed.body).unwrap().paused);
    let ready: Vec<_> = frames.iter().filter(|f| f.kind == "TURN_READY").collect();
    assert_eq!(ready.len(), 1);
    assert!(ready[0].serial > closed.serial);
    assert!(!hs::WitnessSnapshot::parse(&ready[0].body).unwrap().paused);
    let start = frames
        .iter()
        .find(|f| f.step == 4 && f.kind == "RESULT")
        .unwrap();
    assert_eq!(start.body, "string:0|0");
    let growth = frames
        .iter()
        .find(|f| f.step == 5 && f.kind == "RESULT")
        .unwrap();
    assert!(growth.body.contains("0~1~1~0~calm~steady~spear~none~0"));
    for (saved, loaded) in [(8, 12), (17, 21)] {
        let result = |step| {
            &frames
                .iter()
                .find(|f| f.step == step && f.kind == "RESULT")
                .unwrap()
                .body
        };
        assert_eq!(result(saved), result(loaded));
    }
    assert!(frames.iter().any(|f| f.step == 5 && f.kind == "DUPLICATE"));
    assert!(frames.iter().any(|f| f.step == 6 && f.kind == "REQUESTED"));
    assert!(frames
        .iter()
        .any(|f| f.step == 9 && f.kind == "SAVE_COMPLETE"));
    assert_eq!(frames.iter().filter(|f| f.kind == "RESULT").count(), 9);
    assert!(frames
        .iter()
        .any(|f| f.step == 18 && f.kind == "SAVE_COMPLETE"));
    assert!(!frames.iter().any(|f| f.step == 0 && f.kind == "ERROR"));
}

#[test]
fn actual_growth_lua_rejects_nineteen_instead_of_twenty_xp() {
    let (frames, _) = lua_run_with_growth(19, "do end");
    assert!(frames.iter().any(|f| f.step == 15
        && f.kind == "ERROR"
        && f.body.contains("growth did not advance once")));
    assert!(!frames.iter().any(|f| f.step == 15 && f.kind == "RESULT"));
}

#[test]
fn actual_growth_lua_rejects_opponent_credit_before_its_turn() {
    let (frames, _) = lua_run_with_growth(
        20,
        "for uid=1,2 do local r=growthRecords[1][uid]:GetProperty('JAC_POC_GROWTHUI_WARLORD'); r.XP=20; r.LastXPTurn=1 end",
    );
    assert!(frames.iter().any(|f| f.step == 15
        && f.kind == "ERROR"
        && f.body.contains("inactive growth record changed")));
    assert!(!frames.iter().any(|f| f.step == 15 && f.kind == "RESULT"));
}

#[test]
fn actual_growth_lua_rejects_inactive_metadata_change() {
    let (frames, _) = lua_run_with_growth(
        20,
        "growthRecords[1][1]:GetProperty('JAC_POC_GROWTHUI_WARLORD').Trait='changed'",
    );
    assert!(frames.iter().any(|f| f.step == 15
        && f.kind == "ERROR"
        && f.body.contains("inactive growth record changed")));
    assert!(!frames.iter().any(|f| f.step == 15 && f.kind == "RESULT"));
}

#[test]
fn actual_growth_lua_rejects_replaced_unit_id() {
    let (frames, _) = lua_run_with_growth(
        20,
        "local original=growthRecords[1][1]; growthRecords[1][1]={GetID=function() return 99 end, GetProperty=function(_, key) return original:GetProperty(key) end}",
    );
    assert!(frames
        .iter()
        .any(|f| f.step == 15 && f.kind == "ERROR" && f.body.contains("unexpected growth unit")));
    assert!(!frames.iter().any(|f| f.step == 15 && f.kind == "RESULT"));
}

#[test]
fn witness_does_not_register_inputs_without_hotseat_and_matching_nonce() {
    let lua = Lua::new();
    lua.load(format!("JAC_HOTSEAT_PLAN={{nonce='{}'}}; include=function() end; GameConfiguration={{IsHotseat=function() return false end, GetValue=function() return 'wrong' end}}", test_nonce())).exec().unwrap();
    let witness = include_str!("../../../poc/JAC_Test_HotseatBridge/UI/JAC_HotseatWitness.lua");
    lua.load(witness).exec().unwrap();
    lua.load("GameConfiguration.IsHotseat=function() return true end")
        .exec()
        .unwrap();
    lua.load(witness).exec().unwrap();
    assert!(lua
        .globals()
        .get::<mlua::Value>("inputHandler")
        .unwrap()
        .is_nil());
}

#[test]
fn malformed_chunks_stale_nonce_and_duplicate_frames_fail() {
    let (frames, log) = lua_run();
    assert!(!frames.is_empty());
    let first = log.lines().next().unwrap();
    let parts: Vec<_> = first.split('|').collect();
    assert!(parts[7].starts_with("1/"));
    assert!(hs::parse_frames(&format!("{first}\n{first}"), test_nonce()).is_err());
    let mut stale_nonce = test_nonce().to_owned();
    let replacement = if stale_nonce.starts_with('0') {
        "1"
    } else {
        "0"
    };
    stale_nonce.replace_range(0..1, replacement);
    assert!(hs::parse_frames(&log.replace(test_nonce(), &stale_nonce), test_nonce()).is_err());
    let skipped = log
        .lines()
        .filter(|line| !line.contains("|2/"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(hs::parse_frames(&skipped, test_nonce()).is_err());
    let mut first_two = log.lines().take(2).collect::<Vec<_>>();
    first_two.reverse();
    let reversed = first_two.join("\n");
    assert!(hs::parse_frames(&reversed, test_nonce()).is_err());
    assert!(hs::parse_frames(&format!("{log}\n{first}"), test_nonce()).is_err());
}

#[test]
fn full_growth_comparison_catches_a_single_xp_change_and_bad_epochs() {
    let (frames, _) = lua_run();
    let state = hs::WitnessSnapshot::parse(&frames.iter().find(|f| f.kind == "BOOT").unwrap().body)
        .unwrap();
    let mut captures = std::collections::BTreeMap::new();
    captures.insert("MID_ROUND".into(), ("value".into(), state.clone()));
    hs::matching_capture(&captures, "MID_ROUND", "value", &state).unwrap();
    let mut changed = state.clone();
    changed.fields[14] = "number:20".into();
    assert!(hs::matching_capture(&captures, "MID_ROUND", "value", &changed).is_err());
    assert!(hs::matching_capture(&captures, "MID_ROUND", "other", &state).is_err());
    assert!(hs::WitnessSnapshot::parse("3:nil").is_err());
    let boot = frames.iter().find(|f| f.kind == "BOOT").unwrap();
    assert!(hs::validate_load_epoch(&state, boot, test_nonce(), 1, 2).is_ok());
    assert!(hs::validate_load_epoch(&state, boot, test_nonce(), 1, 1).is_err());
    assert!(hs::validate_load_epoch(&changed, boot, test_nonce(), 1, 2).is_err());
}

#[test]
fn log_baseline_rejects_old_ack_and_changed_generation() {
    let (_, log) = lua_run();
    let dir = tempfile::tempdir().unwrap();
    for name in ["Lua.log", "Database.log", "Modding.log"] {
        fs::write(dir.path().join(name), b"old\n").unwrap();
    }
    let baseline = judge::snapshot_logs(dir.path()).unwrap();
    fs::write(dir.path().join("Lua.log"), format!("old\n{log}\n")).unwrap();
    assert!(!hs::fresh_frames(dir.path(), &baseline, test_nonce())
        .unwrap()
        .is_empty());
    let after = judge::snapshot_logs(dir.path()).unwrap();
    assert!(hs::fresh_frames(dir.path(), &after, test_nonce())
        .unwrap()
        .is_empty());
    fs::write(dir.path().join("Lua.log"), b"replaced").unwrap();
    assert!(hs::fresh_frames(dir.path(), &after, test_nonce()).is_err());
}

#[test]
fn polling_waits_for_the_tail_of_a_chunked_frame() {
    let (complete, log) = lua_run();
    let first = log.lines().next().unwrap();
    assert!(hs::parse_frames(first, test_nonce()).is_err());
    assert!(hs::parse_available_frames(first, test_nonce())
        .unwrap()
        .is_empty());
    assert_eq!(
        hs::parse_available_frames(&format!("{log}\n"), test_nonce()).unwrap(),
        complete
    );
    let interrupted = format!("{log}\nJAC_HS|1|{}|unfinished", test_nonce());
    assert_eq!(
        hs::parse_available_frames(&interrupted, test_nonce()).unwrap(),
        complete
    );
    assert!(hs::parse_available_frames(&format!("{interrupted}\n"), test_nonce()).is_err());
    let finished = format!("{log}\n");
    let last_line = finished.lines().last().unwrap();
    let partial = last_line
        .rfind('|')
        .and_then(|at| last_line.get(..at + 3))
        .unwrap();
    let without_last = finished.strip_suffix(&format!("{last_line}\n")).unwrap();
    assert_eq!(
        hs::parse_available_frames(&format!("{without_last}{partial}"), test_nonce())
            .unwrap()
            .len(),
        complete.len() - 1
    );
    assert_eq!(
        hs::parse_available_frames(&finished, test_nonce()).unwrap(),
        complete
    );
}

#[test]
fn changed_scenario_and_unsupported_commands_are_rejected() {
    let mut data = original();
    hs::check_scenario(&data).unwrap();
    data["steps"][5]["action"] = json!("autoplay");
    assert!(hs::check_scenario(&data).is_err());
    data = original();
    data["steps"][10]["name"] = json!("wrong-save");
    assert!(hs::check_scenario(&data).is_err());
    data = original();
    data["steps"][4]["code"] = json!("return 'fabricated growth'");
    assert!(hs::check_scenario(&data).is_err());
    assert!(scenario::validate_scenario(&original()).is_ok());
}

#[test]
fn banseiki_hotseat_keeps_the_original_growth_oracle_and_save_load_sequence() {
    let source = original();
    let variant = banseiki();
    assert_eq!(variant["session_type"], "hotseat");
    assert_eq!(variant["steps"].as_array().unwrap().len(), 21);
    for (base, extended) in source["steps"]
        .as_array()
        .unwrap()
        .iter()
        .zip(variant["steps"].as_array().unwrap())
    {
        assert_eq!(extended["action"], base["action"]);
    }
    for index in [4, 7, 11, 14, 16, 20] {
        assert_eq!(
            variant["steps"][index]["code"],
            source["steps"][index]["code"]
        );
    }
    assert_eq!(
        variant["steps"][1]["players"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["human"] == true)
            .count(),
        2
    );
    hs::check_scenario(&variant).unwrap();
    let mut changed = variant.clone();
    changed["steps"][1]["ruleset"] = json!("RULESET_EXPANSION_2");
    assert!(hs::check_scenario(&changed).is_err());
    changed = variant.clone();
    changed["steps"][14]["code"] = json!("return 'fabricated growth'");
    assert!(hs::check_scenario(&changed).is_err());
    let output = tempfile::tempdir().unwrap();
    let bundle = hs::prepare(
        &variant,
        output.path(),
        test_nonce(),
        "78ea4ad5-d526-4ca0-bd0e-cc4519261719",
    )
    .unwrap();
    let prepared: Value = serde_json::from_slice(&fs::read(&bundle.scenario).unwrap()).unwrap();
    hs::verify_installed(&bundle, &prepared, output.path()).unwrap();
}

#[test]
fn japan_hotseat_preserves_the_map_roster_and_original_21_step_contract() {
    let variant = japan_settlers();
    let source = banseiki();
    let map = japan_settlers_source();
    assert_eq!(variant["session_type"], "hotseat");
    assert_eq!(variant["strict_lua_errors"], true);
    assert_eq!(variant["steps"].as_array().unwrap().len(), 21);
    let mut mods = map["steps"][0].clone();
    mods["exclusive_user_mods"] = json!(true);
    assert_eq!(variant["steps"][0], mods);
    assert_eq!(mods["ids"].as_array().unwrap().len(), 5);
    assert_eq!(
        mods["disable_ids"],
        json!(["dfa0717e-4d0b-4370-b693-27fc6758571b"])
    );
    let mut configure = map["steps"][1].clone();
    configure["players"][1]["human"] = json!(true);
    assert_eq!(variant["steps"][1], configure);
    let players = configure["players"].as_array().unwrap();
    assert_eq!(players.len(), 22);
    for (pid, player) in players.iter().enumerate() {
        assert_eq!(player["human"], json!(pid < 2));
    }
    for index in 2..21 {
        if index != 3 {
            assert_eq!(
                variant["steps"][index],
                source["steps"][index],
                "step {}",
                index + 1
            );
        }
    }
    assert_eq!(
        variant["steps"][3]["capture_as"],
        source["steps"][3]["capture_as"]
    );
    assert!(variant["steps"][3]["code"]
        .as_str()
        .unwrap()
        .ends_with(source["steps"][3]["code"].as_str().unwrap()));
    hs::check_scenario(&original()).unwrap();
    hs::check_scenario(&source).unwrap();
    hs::check_scenario(&variant).unwrap();
}

#[test]
fn japan_hotseat_generated_bundle_accepts_only_the_registered_source() {
    let source = japan_settlers();
    let output = tempfile::tempdir().unwrap();
    let mod_id = "78ea4ad5-d526-4ca0-bd0e-cc4519261719";
    let bundle = hs::prepare(&source, output.path(), test_nonce(), mod_id).unwrap();
    let prepared: Value = serde_json::from_slice(&fs::read(&bundle.scenario).unwrap()).unwrap();
    let mut expected = source.clone();
    expected["steps"][0]["ids"]
        .as_array_mut()
        .unwrap()
        .push(json!(mod_id));
    expected["hotseat_bridge_nonce"] = json!(test_nonce());
    assert_eq!(prepared, expected);
    hs::check_scenario(&prepared).unwrap();
    hs::verify_installed(&bundle, &prepared, output.path()).unwrap();
    let plan_path = output
        .path()
        .join("JAC_Test_HotseatBridge/UI/JAC_HotseatPlan.lua");
    let plan = fs::read_to_string(&plan_path).unwrap();
    for step in hs::QUERY_STEPS {
        assert!(plan.contains(source["steps"][step - 1]["code"].as_str().unwrap()));
    }
    // Neither a raw nor a generated scenario may relax map, roster or growth checks.
    for registered in [source, prepared.clone()] {
        for (pointer, replacement) in [
            ("/steps/1/map_file", json!("Maps/Other.Civ6Map")),
            ("/steps/1/map_size", json!("MAPSIZE_SMALL")),
            ("/steps/1/city_states", json!(1)),
            ("/steps/1/ruleset", json!("RULESET_EXPANSION_2")),
            ("/steps/1/players/1/human", json!(false)),
            ("/steps/1/players/2/human", json!(true)),
            (
                "/steps/1/players/21/leader",
                json!("LEADER_JAC_ODA_NOBUHIDE"),
            ),
            ("/steps/1/players/21/civ", json!("CIVILIZATION_JAC_ODA")),
            ("/steps/0/exclusive_user_mods", json!(false)),
            ("/steps/0/disable_ids", json!([])),
            ("/steps/3/code", banseiki()["steps"][3]["code"].clone()),
            ("/steps/4/code", json!("return 'fabricated growth'")),
            ("/steps/11/equals_capture", json!("INITIAL_RECORDS")),
            ("/steps/20/equals_capture", json!("MID_ROUND")),
            ("/steps/8/name", json!("different-save")),
        ] {
            let mut changed = registered.clone();
            *changed.pointer_mut(pointer).unwrap() = replacement;
            assert!(hs::check_scenario(&changed).is_err(), "{pointer}");
        }
        let mut changed = registered.clone();
        changed["steps"][1]["players"]
            .as_array_mut()
            .unwrap()
            .truncate(2);
        assert!(hs::check_scenario(&changed).is_err());
        let mut changed = registered.clone();
        changed["steps"][0]["ids"].as_array_mut().unwrap().remove(4);
        assert!(hs::check_scenario(&changed).is_err());
        let mut changed = registered.clone();
        changed["steps"][1]["players"]
            .as_array_mut()
            .unwrap()
            .swap(2, 3);
        assert!(hs::check_scenario(&changed).is_err());
        let mut changed = registered.clone();
        changed["steps"][4]["code"] = json!(changed["steps"][4]["code"]
            .as_str()
            .unwrap()
            .replace("count~=2", "count<1"));
        assert!(hs::check_scenario(&changed).is_err());
    }
    let mut wrong_bundle = bundle.clone();
    wrong_bundle.nonce = "0".repeat(32);
    if wrong_bundle.nonce == bundle.nonce {
        wrong_bundle.nonce = "1".repeat(32);
    }
    assert!(hs::verify_installed(&wrong_bundle, &prepared, output.path()).is_err());
    fs::write(plan_path, plan.replace("if humans~=2", "if humans<1")).unwrap();
    assert!(hs::verify_installed(&bundle, &prepared, output.path()).is_err());
}

// Fixture data comes from the existing single-player map, independently of the new probe.
fn japan_initial_state(growth_count: usize) -> Lua {
    let lua = Lua::new();
    let roster = lua.create_table().unwrap();
    for (pid, player) in japan_settlers_source()["steps"][1]["players"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let entry = lua.create_table().unwrap();
        entry
            .set("leader", player["leader"].as_str().unwrap())
            .unwrap();
        entry.set("civ", player["civ"].as_str().unwrap()).unwrap();
        roster.set(pid, entry).unwrap();
    }
    lua.globals().set("roster", roster).unwrap();
    lua.globals().set("growthCount", growth_count).unwrap();
    lua.load(r#"
      Game={GetLocalPlayer=function() return 0 end,GetCurrentGameTurn=function() return 0 end,
        GetProperty=function(_,key) assert(key=='JAC_POC_GROWTHUI_INITIALIZED'); return true end}
      GameConfiguration={IsHotseat=function() return true end,GetStartTurn=function() return 0 end}
      Map={GetGridSize=function() return 204,88 end}
      GameInfo={Units={UNIT_SETTLER={Index=1}}}
      Players={}; PlayerConfigurations={}; majors={}; state={}
      for pid=0,21 do
        local id=pid; local entry=roster[id]
        local s={alive=true,human=id<2,cities=0,settlers=1,leader=entry.leader,civ=entry.civ}
        state[id]=s
        local units={}
        for uid=1,growthCount do
          local unitID=uid
          units[uid]={GetID=function() return unitID end,
            GetUnitType=function() return unitID<=s.settlers and 1 or 2 end,
            GetProperty=function(_,key)
              assert(key=='JAC_POC_GROWTHUI_WARLORD')
              if id<2 then return {Level=1,XP=0,Personality='calm',Trait='steady',Equipment={'spear'}} end
            end}
        end
        local p={GetID=function() return id end,IsAlive=function() return s.alive end,
          IsHuman=function() return s.human end,IsTurnActive=function() return id==0 end,
          GetCities=function() return {GetCount=function() return s.cities end} end,
          GetUnits=function() return {Members=function()
            local i=0; return function() i=i+1; if units[i] then return i,units[i] end end
          end} end}
        Players[id]=p; majors[pid+1]=p
        PlayerConfigurations[id]={GetLeaderTypeName=function() return s.leader end,
          GetCivilizationTypeName=function() return s.civ end}
      end
      PlayerManager={GetAliveMajors=function() return majors end}
    "#).exec().unwrap();
    lua
}

#[test]
fn japan_initial_probe_reads_all_22_majors_and_rejects_roster_or_start_mismatches() {
    let data = japan_settlers();
    let code = data["steps"][3]["code"].as_str().unwrap();
    let lua = japan_initial_state(2);
    assert_eq!(lua.load(code).eval::<String>().unwrap(), "0|0");
    for (change, expected_error) in [
        (
            "GameConfiguration.IsHotseat=function() return false end",
            "active human hotseat turn required",
        ),
        (
            "Game.GetLocalPlayer=function() return 1 end; Players[1].IsTurnActive=function() return true end",
            "unexpected hotseat player or round",
        ),
        (
            "Game.GetCurrentGameTurn=function() return 1 end",
            "unexpected hotseat player or round",
        ),
        (
            "Players[0].IsTurnActive=function() return false end",
            "active human hotseat turn required",
        ),
        (
            "Map.GetGridSize=function() return 154,67 end",
            "map grid mismatch",
        ),
        ("table.remove(majors)", "major count"),
        ("state[21].alive=false", "missing major 21"),
        ("majors[22]=majors[1]", "unexpected major slot"),
        ("state[21].cities=1", "premade cities in slot 21"),
        ("state[21].settlers=0", "settler count in slot 21"),
        ("state[21].settlers=2", "settler count in slot 21"),
        ("state[1].human=false", "human/AI mismatch in slot 1"),
        ("state[21].human=true", "human/AI mismatch in slot 21"),
        (
            "state[21].leader=state[0].leader",
            "leader mismatch in slot 21",
        ),
        (
            "state[21].civ=state[0].civ",
            "civilization mismatch in slot 21",
        ),
    ] {
        let lua = japan_initial_state(2);
        lua.load(change).exec().unwrap();
        let error = lua.load(code).eval::<String>().unwrap_err().to_string();
        assert!(error.contains(expected_error), "{change}: {error}");
    }
}

#[test]
fn japan_growth_probe_keeps_exactly_two_records_per_human() {
    let data = japan_settlers();
    let code = data["steps"][4]["code"].as_str().unwrap();
    let result = japan_initial_state(2).load(code).eval::<String>().unwrap();
    assert_eq!(result.split('|').count(), 6); // turn, active player and all four records
    for count in [1, 3] {
        let error = japan_initial_state(count)
            .load(code)
            .eval::<String>()
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!(
                "expected 2 growth records for player 0, got {count}"
            )),
            "{error}"
        );
    }
}

#[test]
fn installed_lua_must_match_the_generated_original_scenario() {
    let output = tempfile::tempdir().unwrap();
    let bundle = hs::prepare(
        &original(),
        output.path(),
        test_nonce(),
        "78ea4ad5-d526-4ca0-bd0e-cc4519261719",
    )
    .unwrap();
    let prepared: Value = serde_json::from_slice(&fs::read(&bundle.scenario).unwrap()).unwrap();
    hs::verify_installed(&bundle, &prepared, output.path()).unwrap();
    fs::write(
        output
            .path()
            .join("JAC_Test_HotseatBridge/UI/JAC_HotseatPlan.lua"),
        "return 'fabricated'",
    )
    .unwrap();
    assert!(hs::verify_installed(&bundle, &prepared, output.path()).is_err());
}

struct FakeIo {
    owner: hs::WindowOwner,
    now: f64,
    keys: Vec<u16>,
    responses: Vec<hs::Frame>,
    by_key: std::collections::BTreeMap<u16, Vec<hs::Frame>>,
    loss: bool,
    delayed: Vec<(f64, hs::Frame)>,
}
impl hs::TransportIo for FakeIo {
    fn owner(&mut self) -> Result<hs::WindowOwner> {
        let mut o = self.owner;
        if self.loss && !self.keys.is_empty() {
            o.started += 1.0
        }
        Ok(o)
    }
    fn baseline(&mut self) -> Result<Value> {
        Ok(json!({"files":{"Lua.log":{"offset":0}}}))
    }
    fn frames(&mut self, _: &Value, _: &str) -> Result<Vec<hs::Frame>> {
        let mut frames = self.responses.clone();
        frames.extend(
            self.delayed
                .iter()
                .filter(|(at, _)| self.now >= *at)
                .map(|(_, frame)| frame.clone()),
        );
        Ok(frames)
    }
    fn key(&mut self, key: u16, _: f64) -> Result<()> {
        self.keys.push(key);
        if let Some(v) = self.by_key.get(&key) {
            self.responses = v.clone()
        }
        Ok(())
    }
    fn now(&mut self) -> f64 {
        self.now
    }
    fn sleep(&mut self, seconds: f64) {
        self.now += seconds
    }
}

#[test]
fn controller_spends_key_before_send_and_never_resends_unknown_mutation() {
    let (frames, _) = lua_run();
    let boot = frames.iter().find(|f| f.kind == "BOOT").unwrap().clone();
    let owner = hs::WindowOwner {
        pid: 321,
        started: 10.0,
        hwnd: 42,
    };
    let fake = FakeIo {
        owner,
        now: 0.0,
        keys: vec![],
        responses: vec![],
        by_key: Default::default(),
        loss: false,
        delayed: vec![],
    };
    let mut controller = hs::Controller::new(fake, test_nonce(), &boot).unwrap();
    let mut spent = vec![];
    let err = controller
        .step(6, &original(), 0.25, |n| {
            spent.push(n);
            Ok(())
        })
        .unwrap_err();
    assert!(err.to_string().contains("outcome unknown"));
    assert_eq!(err.kind, ErrorKind::LuaResponseUnknown);
    assert_eq!(spent, vec![6]);
    assert_eq!(controller.io.keys, vec![hs::KEY_END_TURN]);
    assert!(controller.step(6, &original(), 1.0, |_| Ok(())).is_err());
    assert_eq!(controller.io.keys.len(), 1);
    controller.io.loss = true;
    assert!(controller
        .step(7, &original(), 1.0, |_| Ok(()))
        .unwrap_err()
        .to_string()
        .contains("ownership"));
    assert_eq!(controller.io.keys.len(), 1);
    assert!(controller
        .step(11, &original(), 1.0, |_| Ok(()))
        .unwrap_err()
        .to_string()
        .contains("cannot send"));
}

fn packed(fields: &[String]) -> String {
    fields
        .iter()
        .map(|field| format!("{}:{field}", field.len()))
        .collect()
}

#[test]
fn player_change_requires_visible_paused_then_hidden_unpaused_human() {
    let (frames, _) = lua_run();
    let boot = frames.iter().find(|f| f.kind == "BOOT").unwrap().clone();
    let owner = hs::WindowOwner {
        pid: 321,
        started: 10.0,
        hwnd: 42,
    };
    let mut fields = hs::decode_fields(&boot.body).unwrap();
    fields[2] = "number:1".into();
    fields[7] = "boolean:true".into();
    fields[10] = "boolean:false".into();
    fields[30] = "boolean:true".into();
    let waiting = packed(&fields);
    fields[6] = "boolean:true".into();
    fields[8] = "boolean:true".into();
    let opened = packed(&fields);
    fields[6] = "boolean:false".into();
    fields[7] = "boolean:false".into();
    fields[8] = "boolean:false".into();
    let closed = packed(&fields);
    let frame = |serial, step, kind: &str, body: String| hs::Frame {
        nonce: test_nonce().into(),
        scene: boot.scene.clone(),
        serial,
        step,
        kind: kind.into(),
        body,
    };
    let mut by_key = std::collections::BTreeMap::new();
    by_key.insert(
        hs::KEY_END_TURN,
        vec![
            frame(2, 6, "BEFORE", boot.body.clone()),
            frame(3, 6, "REQUESTED", boot.body.clone()),
            frame(4, 0, "DIALOG_WAIT", waiting),
            frame(5, 0, "DIALOG_OPEN", opened),
        ],
    );
    by_key.insert(0x0d, vec![frame(6, 0, "DIALOG_CLOSED", closed)]);
    let fake = FakeIo {
        owner,
        now: 0.0,
        keys: vec![],
        responses: vec![],
        by_key,
        loss: false,
        delayed: vec![],
    };
    let mut controller = hs::Controller::new(fake, test_nonce(), &boot).unwrap();
    let receipt = controller.step(6, &original(), 3.0, |_| Ok(())).unwrap();
    let opened = receipt.dialog_open.unwrap();
    assert!(opened.dialog && opened.dialog_ready && opened.paused);
    let closed = controller.accept_turn(6, &opened, 3.0, |_| Ok(())).unwrap();
    assert!(!closed.dialog && !closed.paused && closed.active && closed.human);
    assert_eq!(controller.io.keys, vec![hs::KEY_END_TURN, 0x0d]);
    assert!(controller.accept_turn(6, &opened, 3.0, |_| Ok(())).is_err());
    assert_eq!(controller.io.keys.len(), 2);
}

#[test]
fn player_change_waits_for_delayed_unpause_without_another_return() {
    let (frames, _) = lua_run();
    let boot = frames.iter().find(|f| f.kind == "BOOT").unwrap().clone();
    let open = frames
        .iter()
        .find(|f| f.kind == "DIALOG_OPEN")
        .unwrap()
        .clone();
    let closed = frames
        .iter()
        .find(|f| f.kind == "DIALOG_CLOSED")
        .unwrap()
        .clone();
    let ready = frames
        .iter()
        .find(|f| f.kind == "TURN_READY")
        .unwrap()
        .clone();
    let opened = hs::WitnessSnapshot::parse(&open.body).unwrap();
    let fake = FakeIo {
        owner: hs::WindowOwner {
            pid: 321,
            started: 10.0,
            hwnd: 42,
        },
        now: 0.0,
        keys: vec![],
        responses: vec![],
        by_key: [(0x0d, vec![closed])].into_iter().collect(),
        loss: false,
        delayed: vec![(0.2, ready)],
    };
    let mut controller = hs::Controller::new(fake, test_nonce(), &boot).unwrap();
    let mut spent = vec![];
    let accepted = controller
        .accept_turn(3, &opened, 1.0, |step| {
            spent.push(step);
            Ok(())
        })
        .unwrap();
    assert!(accepted.active && accepted.human && !accepted.paused && !accepted.dialog);
    assert!(controller.io.now >= 0.2);
    assert_eq!(spent, vec![3]);
    assert_eq!(controller.io.keys, vec![0x0d]);
    assert!(controller.accept_turn(3, &opened, 1.0, |_| Ok(())).is_err());
    assert_eq!(controller.io.keys, vec![0x0d]);
}

#[test]
fn player_change_never_accepts_missing_unpause_wrong_turn_or_missing_close() {
    let (frames, _) = lua_run();
    let boot = frames.iter().find(|f| f.kind == "BOOT").unwrap().clone();
    let open = frames.iter().find(|f| f.kind == "DIALOG_OPEN").unwrap();
    let closed = frames
        .iter()
        .find(|f| f.kind == "DIALOG_CLOSED")
        .unwrap()
        .clone();
    let ready = frames
        .iter()
        .find(|f| f.kind == "TURN_READY")
        .unwrap()
        .clone();
    let opened = hs::WitnessSnapshot::parse(&open.body).unwrap();
    for variant in ["no-unpause", "wrong-turn", "no-close"] {
        let mut response = if variant == "no-close" {
            vec![]
        } else {
            vec![closed.clone()]
        };
        if variant != "no-unpause" {
            let mut candidate = ready.clone();
            if variant == "wrong-turn" {
                let mut fields = hs::decode_fields(&candidate.body).unwrap();
                fields[0] = format!("number:{}", opened.turn + 1);
                candidate.body = packed(&fields);
            }
            response.push(candidate);
        }
        let fake = FakeIo {
            owner: hs::WindowOwner {
                pid: 321,
                started: 10.0,
                hwnd: 42,
            },
            now: 0.0,
            keys: vec![],
            responses: vec![],
            by_key: [(0x0d, response)].into_iter().collect(),
            loss: false,
            delayed: vec![],
        };
        let mut controller = hs::Controller::new(fake, test_nonce(), &boot).unwrap();
        let error = controller
            .accept_turn(3, &opened, 0.3, |_| Ok(()))
            .unwrap_err();
        if variant == "no-unpause" {
            assert_eq!(error.kind, ErrorKind::LuaResponseUnknown);
        }
        assert_eq!(controller.io.keys, vec![0x0d]);
    }
}

#[test]
fn synchronous_save_complete_is_accepted_with_its_real_name() {
    let (frames, _) = lua_run();
    let boot = frames.iter().find(|f| f.kind == "BOOT").unwrap().clone();
    let save_frames = frames
        .iter()
        .filter(|f| f.step == 9)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        save_frames
            .iter()
            .map(|f| f.kind.as_str())
            .collect::<Vec<_>>(),
        ["BEFORE", "REQUESTED", "SAVE_COMPLETE", "SAVE_RETURNED"]
    );
    let owner = hs::WindowOwner {
        pid: 321,
        started: 10.0,
        hwnd: 42,
    };
    let mut by_key = std::collections::BTreeMap::new();
    by_key.insert(hs::KEY_SAVE, save_frames);
    let fake = FakeIo {
        owner,
        now: 0.0,
        keys: vec![],
        responses: vec![],
        by_key,
        loss: false,
        delayed: vec![],
    };
    let mut controller = hs::Controller::new(fake, test_nonce(), &boot).unwrap();
    let receipt = controller.step(9, &original(), 2.0, |_| Ok(())).unwrap();
    assert_eq!(receipt.kind, "SAVE_COMPLETE");
    assert_eq!(
        receipt.response_body,
        format!("JAC-Hotseat-Mid-Round-{}", test_nonce())
    );
    assert_eq!(controller.io.keys, vec![hs::KEY_SAVE]);
}
