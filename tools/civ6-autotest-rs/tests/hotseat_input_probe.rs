use civ6_autotest_rs::hotseat_bridge as hs;
use mlua::Lua;
use serde_json::Value;
use std::{
    fs,
    sync::{Arc, Mutex},
};

#[test]
fn input_only_probe_never_runs_original_queries_or_game_actions() {
    let data: Value = serde_json::from_str(include_str!(
        "../../civ6-autotest/scenarios/multiplayer-hotseat-growth.json"
    ))
    .unwrap();
    let output = tempfile::tempdir().unwrap();
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let bundle = hs::prepare_mode(
        &data,
        output.path(),
        &nonce,
        &uuid::Uuid::new_v4().to_string(),
        true,
    )
    .unwrap();
    assert!(bundle.input_diagnostic);
    let lua = Lua::new();
    let lines = Arc::new(Mutex::new(Vec::new()));
    let recorded = lines.clone();
    lua.globals()
        .set(
            "print",
            lua.create_function(move |_, line: String| {
                recorded.lock().unwrap().push(line);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
    lua.globals().set("nonce", nonce.clone()).unwrap();
    lua.load(r#"
        GameConfiguration={IsHotseat=function() return true end,
          GetValue=function() return nonce end, GetStartTurn=function() return 1 end,
          IsPaused=function() return false end}
        Game={GetCurrentGameTurn=function() return 1 end, GetLocalPlayer=function() return 0 end}
        Players={}
        for pid=0,1 do
          local owner=pid
          local units={}
          for uid=1,2 do
            local id=uid
            local r={Level=1,XP=0,Personality='calm',Trait='steady',Equipment={'spear'},LastXPTurn=1}
            units[uid]={GetID=function() return id end, GetProperty=function() return r end}
          end
          Players[pid]={IsHuman=function() return true end, IsTurnActive=function() return owner==0 end,
            GetUnits=function() return {Members=function() local i=0; return function()
              i=i+1; if units[i] then return i,units[i] end end end} end}
        end
        ContextPtr={LookUpControl=function() return {IsHidden=function() return true end} end,
          SetHide=function() end, SetInputHandler=function(_,f) handler=f end,
          SetUpdate=function(_,f) update=f end}
        KeyEvents={KeyDown=2,KeyUp=1}
        Events={SaveComplete={Add=function() end}, ExitToMainMenu=function() error('unexpected exit') end}
        UI={RequestAction=function() error('unexpected turn') end}
        Network={SaveGame=function() error('unexpected save') end}
        include=function() end
        function input(key,message)
          return handler({GetKey=function() return key end,GetMessageType=function() return message end})
        end
    "#).exec().unwrap();
    let root = output.path().join("JAC_Test_HotseatBridge/UI");
    lua.load(fs::read_to_string(root.join("JAC_HotseatPlan.lua")).unwrap())
        .exec()
        .unwrap();
    lua.load(
        r#"queries=0; for key,f in pairs(JAC_HOTSEAT_PLAN.queries) do
        JAC_HOTSEAT_PLAN.queries[key]=function() queries=queries+1; return f() end end"#,
    )
    .exec()
    .unwrap();
    lua.load(fs::read_to_string(root.join("JAC_HotseatWitness.lua")).unwrap())
        .exec()
        .unwrap();
    lua.load(
        r#"
        assert(not input(124,1)); update()
        assert(input(124,2)); assert(input(124,1)); assert(input(124,1))
        for key=125,135 do assert(input(key,2)); assert(input(key,1)) end
        assert(not input(13,1))
        for i=1,40 do assert(not input(999,1)) end
        assert(queries==0)
    "#,
    )
    .exec()
    .unwrap();
    let lines = lines.lock().unwrap();
    let frames = hs::parse_frames(&lines.join("\n"), &nonce).unwrap();
    assert_eq!(frames.iter().filter(|f| f.kind == "INPUT_PROBE").count(), 1);
    assert!(
        frames
            .iter()
            .any(|f| f.kind == "INPUT_PROBE" && f.step == 0 && f.body == "key_up|124")
    );
    assert!(
        frames
            .iter()
            .all(|f| f.step == 0 && ["BOOT", "INPUT_PROBE"].contains(&f.kind.as_str()))
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("JAC_HS_INPUT|"))
            .count(),
        32
    );
    assert!(lines.iter().any(|line| line.ends_with("|1|999")));
}
