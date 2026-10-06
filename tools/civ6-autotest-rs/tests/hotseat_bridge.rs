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

fn lua_run_with_growth(xp: u32) -> (Vec<hs::Frame>, String) {
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
      ActionTypes = {{ACTION_ENDTURN=5}}; actions=0
      UI = {{RequestAction=function(id) assert(id==5); actions=actions+1; return true end}}
      SaveLocations={{LOCAL_STORAGE=8}}; saveType=0; savedName=nil
      Events = {{SaveComplete={{Add=function(f) saveCallback=f end, Remove=function() saveCallback=nil end}},
        ExitToMainMenu=function() exits=(exits or 0)+1 end}}
      Network = {{GetGameConfigurationSaveType=function() saveType=saveType+1; return 23 end,
        SaveGame=function(opts) assert(opts.Type==23 and opts.Location==8); savedName=opts.Name; if saveCallback then saveCallback() end; return true end}}
      include=function(name) assert(name=='JAC_HotseatPlan') end
      function key(v) local input={{GetMessageType=function() return 1 end, GetKey=function() return v end}}; return inputHandler(input) end
    "#)).exec().unwrap();
    lua.load(&plan).exec().unwrap();
    lua.load(&witness).exec().unwrap();
    lua.load("assert(contextHidden == false)").exec().unwrap();
    lua.load(format!("updateHandler(); assert(key(124)); assert(key(125)); assert(key(125)); assert(key(133)); assert(key(133)); localID=1; updateHandler(); assert(key(126)); assert(key(127)); assert(key(134)); assert(savedName=='JAC-Hotseat-Mid-Round-'..runNonce); assert(saveType==1); assert(key(128)); localID=0; turn=1; for uid=1,2 do local r=growthRecords[0][uid]:GetProperty('JAC_POC_GROWTHUI_WARLORD'); r.XP={xp}; r.LastXPTurn=1 end; assert(key(129)); assert(key(130)); localID=1; for uid=1,2 do local r=growthRecords[1][uid]:GetProperty('JAC_POC_GROWTHUI_WARLORD'); r.XP=20; r.LastXPTurn=1 end; assert(key(131)); assert(key(134)); assert(savedName=='JAC-Hotseat-Next-Player-'..runNonce); assert(key(132))")).exec().unwrap();
    lua.load("dialogVisible=true; dialogReady=false; paused=false; updateHandler(); dialogReady=true; paused=true; updateHandler(); dialogVisible=false; dialogReady=false; paused=false; updateHandler()").exec().unwrap();
    let log = lines.lock().unwrap().join("\n");
    (hs::parse_frames(&log, test_nonce()).unwrap(), log)
}

fn lua_run() -> (Vec<hs::Frame>, String) {
    lua_run_with_growth(20)
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
    let (frames, _) = lua_run_with_growth(19);
    assert!(frames.iter().any(|f| f.step == 15
        && f.kind == "ERROR"
        && f.body.contains("growth did not advance once")));
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
        Ok(self.responses.clone())
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
