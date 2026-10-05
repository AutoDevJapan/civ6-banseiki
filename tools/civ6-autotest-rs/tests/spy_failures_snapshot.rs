use civ6_autotest_rs::{repo_root, spy_failures, spy_research};
use mlua::Lua;

fn run(lua: &Lua, code: &str) {
    lua.load(code).exec().unwrap();
}
fn sync(lua: &Lua, action: &str, event: &str) {
    run(lua,&format!("ExposedMembers.JAC_PersonaBoundary.Queue('{action}'); FRAME(); local r=REQUESTS[#REQUESTS]; assert(r.params.OnStart=='{event}'); ENEMY_CASES.fireCore('{event}',r.params); FRAME(); local b=REQUESTS[#REQUESTS]; assert(b.params.OnStart=='JAC_BoundaryEnemyFixtureBarrier'); ENEMY_CASES.fireCore('JAC_BoundaryEnemyFixtureBarrier',b.params); fire(Events.GameCoreEventPublishComplete); FRAME(); assert(ExposedMembers.JAC_PersonaBoundary.lastAction=='ACCEPTED_{action}')"));
}

#[test]
fn real_observer_and_rust_diagnosis_keep_both_failures_without_any_write() {
    let lua = Lua::new();
    run(
        &lua,
        include_str!("../../civ6-lua-tests-rs/fixtures/persona/boundary_engine.lua"),
    );
    for path in [
        "Shared/JAC_PersonaBoundaryRules.lua",
        "UI/JAC_PersonaBoundary_View.lua",
        "Scripts/JAC_PersonaBoundary_Fixture.lua",
    ] {
        run(
            &lua,
            &std::fs::read_to_string(repo_root().join("poc/JAC_Test_PersonaBoundary").join(path))
                .unwrap(),
        );
    }
    run(
        &lua,
        include_str!("../../civ6-lua-tests-rs/fixtures/persona/boundary_enemy_cases.lua"),
    );
    run(&lua, "ENEMY_CASES.setup()");
    sync(&lua, "PREPARE_ENEMY", "JAC_BoundaryPrepareEnemy");
    sync(&lua, "EVOLVE_ENEMY", "JAC_BoundaryEvolveEnemy");
    run(&lua, "ENEMY_CASES.appoint(true)");
    sync(&lua, "CONFIRM_ENEMY", "JAC_BoundaryConfirmEnemy");
    run(
        &lua,
        r#"
        ENEMY_CASES.settled(); TURN=45
        EspionageResultTypes.NO_RESULT=-1; EspionageResultTypes.FAIL_MUST_ESCAPE=7; EspionageResultTypes.KILLED=8
        ENEMY_CASES.mission(EspionageResultTypes.FAIL_UNDETECTED,12,0,4)
        GameInfo.UnitOperations[59]=GameInfo.UnitOperations[0]
        GameInfo.UnitOperations[0].Index=59
        local plot=Map.GetPlot(FOREIGN.x,FOREIGN.y):GetIndex()
        local failure=EspionageResultTypes.FAIL_UNDETECTED
        HISTORY[0][0]={Operation=59,PlotIndex=plot,CompletionTurn=23,InitialResult=failure,EscapeResult=-1}
        HISTORY[0][1]={Operation=59,PlotIndex=plot,CompletionTurn=45,InitialResult=failure,EscapeResult=-1}
        local config=Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG'); config.armedTurn=1
        Game:SetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY',{current=2,spyID=851976,
            archive={spyID=655368,eventID=0,eventTurn=23,targetOwner=1,targetID=20,governorNeutralized=0,
                operation=59,plot=plot,completionTurn=23,initial=failure,escape=-1}})
        Game:SetProperty('JAC_BOUNDARY_ENEMY_MISSION',{attacker=0,id=1,turn=45})
        Game:SetProperty('JAC_BOUNDARY_SPY',{owner=0,id=851976})
        Game:SetProperty('JAC_BOUNDARY_SPY_TRAVEL',{oldID=655368,name=GameInfo.Units['UNIT_SPY'].Name,level=1,targetOwner=1,targetID=20})
        UNITS[0]={}; SPY_CAPACITY=2; CIVIC_COMPLETED=true
        CAPTURED[1]={{OwningPlayer=0,Name='',Level=1,NameIndex=-1},{OwningPlayer=0,Name='',Level=1,NameIndex=-1}}
        OFF_MAP={{Name='',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999},{Name='',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}}
    "#,
    );
    let before: usize = lua.load("return #WRITES").eval().unwrap();
    let raw: String = lua.load(spy_failures::READ).eval().unwrap();
    let report = spy_failures::diagnose(&raw).unwrap();
    assert_eq!((report.physical_count, report.discounted_sentinels), (2, 2));
    assert_eq!(report.next, spy_failures::Next::NativeExchangeOrCapacity);
    assert_eq!(report.raw["spy"]["id"], 851976);
    assert_eq!(report.raw["failures"][0]["turn"], 23);
    assert_eq!(report.raw["failures"][1]["turn"], 45);
    assert_eq!(lua.load("return #WRITES").eval::<usize>().unwrap(), before);
    run(&lua, "CAPTURED[1][1].Level=1.25");
    let raw: String = lua.load(spy_failures::READ).eval().unwrap();
    assert!(raw.contains("1.25"));
    assert!(spy_failures::diagnose(&raw).is_err());
    run(&lua, "CAPTURED[1][1].Level=1");
    run(
        &lua,
        "GameInfo.Units['UNIT_SPY'].Name='quote\" and backslash\\\\ and newline\\n and 日本語'",
    );
    let raw: String = lua.load(spy_failures::READ).eval().unwrap();
    assert_eq!(spy_failures::diagnose(&raw).unwrap().physical_count, 2);
    assert_eq!(lua.load("return #WRITES").eval::<usize>().unwrap(), before);

    run(
        &lua,
        r#"
      GameInfo.Civics['CIVIC_JAC_SENGOKU_NANBAN_ENVOYS']={Index=20,Hash=2020,CivicType='CIVIC_JAC_SENGOKU_NANBAN_ENVOYS'}
      GameInfo.Civics['CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS']={Index=19}
      GameInfo.CivicModifiers=function()
        local done=false
        return function() if not done then done=true; return {CivicType='CIVIC_JAC_SENGOKU_NANBAN_ENVOYS',ModifierId='CIVIC_GRANT_SPY'} end end
      end
      THIRD_OWNED=true; THIRD_PROGRESS=-1
      local oldCulture=Players[0].GetCulture
      Players[0].GetCulture=function(self)
        local culture=oldCulture(self); local oldHas= culture.HasCivic
        culture.HasCivic=function(this,index) if index==19 then return true elseif index==20 then return THIRD_OWNED end; return oldHas(this,index) end
        culture.GetProgressingCivic=function() return THIRD_PROGRESS end
        return culture
      end
      Game:SetProperty('JAC_BOUNDARY_THIRD_CIVIC_INTENT',{owner=0,status='RESERVED',token=501,civicIndex=20,civicHash=2020,eventID=1,eventTurn=45,spyID=851976,turn=45})
    "#,
    );
    let writes_before_research_read: usize = lua.load("return #WRITES").eval().unwrap();
    let read_research = || {
        let raw: String = lua.load(spy_research::read()).eval().unwrap();
        spy_research::diagnose(&raw)
    };
    let pending = read_research().unwrap();
    assert_eq!(pending.stage, spy_research::Stage::EffectPending);
    let saved = serde_json::to_value(&pending).unwrap();
    run(&lua, "SPY_CAPACITY=3");
    let ready = read_research().unwrap();
    assert_eq!(ready.stage, spy_research::Stage::CapacityReady);
    spy_research::check_reload(&saved, &ready).unwrap();
    let mut wrong_save = saved.clone();
    wrong_save["civic"]["intentToken"] = serde_json::json!(502);
    assert!(spy_research::check_reload(&wrong_save, &ready).is_err());
    let mut wrong_mission = saved.clone();
    wrong_mission["failures"]["raw"]["failures"][1]["turn"] = serde_json::json!(44);
    assert!(spy_research::check_reload(&wrong_mission, &ready).is_err());
    run(&lua, "THIRD_OWNED=false; THIRD_PROGRESS=20; SPY_CAPACITY=2");
    assert_eq!(
        read_research().unwrap().stage,
        spy_research::Stage::Researching
    );
    run(&lua, "THIRD_PROGRESS=-1");
    assert!(read_research().is_err());
    assert_eq!(
        lua.load("return #WRITES").eval::<usize>().unwrap(),
        writes_before_research_read
    );
}
