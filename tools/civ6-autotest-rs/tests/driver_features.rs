mod support_driver;

use civ6_autotest_rs::{runner::AutoplayOptions, scenario, Error};
use serde_json::json;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use support_driver::{FakeBackend, Harness, ManualClock};

fn turn_backend() -> FakeBackend {
    let turn = Arc::new(Mutex::new(1));
    FakeBackend::new().lua(move |_, code| {
        if code.contains("return Game.GetCurrentGameTurn() ==") {
            return Ok("true".into());
        }
        if code.contains("UI.RequestAction(ActionTypes.ACTION_ENDTURN)") {
            *turn.lock().unwrap() += 1;
            return Ok("true".into());
        }
        if code.trim() == "return Game.GetCurrentGameTurn()" {
            return Ok(turn.lock().unwrap().to_string());
        }
        Ok("true".into())
    })
}

#[test]
fn test_shortened_turns_require_explicit_test_mode() {
    let mut h = Harness::new(json!({}), turn_backend());
    let step = json!({"action":"end_turn","turns":10,"test_turns":1});
    h.runner.run_step(&step).unwrap();
    assert_eq!(
        h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
        10
    );
    h.runner.test_mode = true;
    h.runner.run_step(&step).unwrap();
    assert_eq!(
        h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
        11
    );
}

#[test]
fn test_configuration_clears_test_mode_on_ordinary_runs() {
    let step = json!({"action":"configure","ruleset":"RULESET_STANDARD","map_size":"MAPSIZE_SMALL","speed":"GAMESPEED_STANDARD","seed":1,"players":[{"leader":"L","civ":"C","human":true}],"build_setup":false});
    for enabled in [true, false] {
        let mut h = Harness::new(json!({}), FakeBackend::new());
        h.runner.test_mode = enabled;
        h.runner.run_step(&step).unwrap();
        let calls = h.calls();
        let code = &calls.last().unwrap().1;
        assert!(code.contains(&format!(
            "GameConfiguration.SetValue('JAC_AUTOTEST', {})",
            i32::from(enabled)
        )));
        assert!(code.contains("GameConfiguration.UpdateEnabledMods()"));
    }
}

#[test]
fn test_end_turn_honours_count_default_and_shared_deadline() {
    let mut h = Harness::new(json!({}), turn_backend());
    assert_eq!(
        h.runner
            .run_step(&json!({"action":"end_turn","turns":2}))
            .unwrap(),
        "human turn 1->2; human turn 2->3"
    );
    assert_eq!(
        h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
        2
    );
    assert_eq!(
        h.runner.run_step(&json!({"action":"end_turn"})).unwrap(),
        "human turn 3->4"
    );
    assert_eq!(
        h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
        3
    );

    let clock = Arc::new(ManualClock::default());
    let action_clock = clock.clone();
    let turn = Arc::new(Mutex::new(1));
    let game_turn = turn.clone();
    let fake = FakeBackend::new().lua(move |_, code| {
        if code.contains("UI.RequestAction(ActionTypes.ACTION_ENDTURN)") {
            *game_turn.lock().unwrap() += 1;
            action_clock.advance(1.0);
        }
        if code.trim() == "return Game.GetCurrentGameTurn()" {
            return Ok(game_turn.lock().unwrap().to_string());
        }
        Ok("true".into())
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.clock = clock;
    h.runner
        .run_step(&json!({"action":"end_turn","turns":2,"timeout":5.5}))
        .unwrap();
    let calls = h.calls();
    let timeouts = h.timeouts.lock().unwrap();
    let notebook_budgets: Vec<_> = calls
        .iter()
        .zip(timeouts.iter())
        .filter(|((_, code), _)| code.contains("JAC_Notebooks_Close"))
        .map(|(_, timeout)| *timeout)
        .collect();
    assert_eq!(notebook_budgets, [5.0, 4.5]);

    let fake = FakeBackend::new().lua(move |_, code| {
        if code.contains("UI.RequestAction(ActionTypes.ACTION_ENDTURN)") {
            return Err(Error::scenario("deadline"));
        }
        Ok("1".into())
    });
    let mut h = Harness::new(json!({}), fake);
    let err = h
        .runner
        .run_step(&json!({"action":"end_turn","turns":2}))
        .unwrap_err();
    assert!(err.to_string().contains("deadline"));
    assert_eq!(
        h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
        1
    );
}

#[test]
fn test_end_turn_rejects_invalid_counts() {
    for value in [json!(0), json!(-1), json!(true), json!(1.5), json!("2")] {
        for key in ["turns", "test_turns"] {
            let data = json!({"name":"turn count","steps":[{"action":"end_turn",key:value}]});
            let err = scenario::validate_scenario(&data).unwrap_err();
            assert!(err.to_string().contains(key), "{err}");
        }
    }
}

#[test]
fn test_build_setup_checks_every_player_after_refresh() {
    let players = json!([{"leader":"LEADER_JAC_ODA_NOBUHIDE","civ":"CIVILIZATION_JAC_ODA","human":true}, {"leader":"LEADER_JAC_TAKEDA_NOBUTORA","civ":"CIVILIZATION_JAC_TAKEDA","human":false}]);
    let code = scenario::build_setup_code(players.as_array().unwrap());
    let order = [
        "BuildHeadlessGameSetup()",
        "g_GameParameters:FullRefresh()",
        "setup changed player 0",
        "setup changed player 1",
        "HideGameSetup()",
    ];
    for pair in order.windows(2) {
        assert!(code.find(pair[0]).unwrap() < code.find(pair[1]).unwrap());
    }
    assert!(code.contains("GetSlotStatus() ~= SlotStatus.SS_TAKEN"));
    assert!(code.contains("GetSlotStatus() ~= SlotStatus.SS_COMPUTER"));
    let mut h = Harness::new(json!({}), FakeBackend::new());
    h.runner.run_step(&json!({"action":"configure","ruleset":"RULESET_JAC_BANSEIKI","map_size":"MAPSIZE_SMALL","speed":"GAMESPEED_STANDARD","seed":1,"players":players,"build_setup":true})).unwrap();
    let calls = h.calls();
    assert_eq!(
        calls.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(),
        ["FrontEnd", "HostGame", "FrontEnd"]
    );
    assert_eq!(calls[1].1, code);
}

#[test]
fn test_build_setup_defaults_on_for_banseiki_only() {
    let base = json!({"action":"configure","map_size":"M","speed":"S","seed":1,"players":[{"leader":"L","civ":"C","human":true}]});
    for (ruleset, explicit, expected) in [
        ("RULESET_JAC_BANSEIKI", None, 3),
        ("RULESET_JAC_BANSEIKI", Some(false), 2),
        ("RULESET_STANDARD", None, 2),
        ("RULESET_STANDARD", Some(true), 3),
    ] {
        let mut step = base.clone();
        step["ruleset"] = json!(ruleset);
        if let Some(value) = explicit {
            step["build_setup"] = json!(value);
        }
        let mut h = Harness::new(json!({}), FakeBackend::new());
        h.runner.run_step(&step).unwrap();
        assert_eq!(h.calls().len(), expected, "{ruleset} {explicit:?}");
        assert_eq!(h.calls().first().unwrap().0, "FrontEnd");
        assert_eq!(h.calls().last().unwrap().0, "FrontEnd");
        if expected == 3 {
            assert_eq!(h.calls()[1].0, "HostGame");
        }
    }
}

#[test]
fn test_start_turn_guard() {
    let mut h = Harness::new(json!({}), FakeBackend::new().lua(|_, _| Ok("16603".into())));
    h.runner.check_start_turn(60.0).unwrap();
    assert!(h.calls().is_empty());
    for (start, current, normalized, accepted) in [
        (16603, 16603, true, true),
        (1, 1, false, true),
        (16603, 16603, false, false),
        (16603, 16604, true, false),
        (1, 2, false, false),
        (0, 0, true, false),
    ] {
        let backend = FakeBackend::new().lua(move |state, code| {
            assert_eq!(state, "InGame");
            let lua = mlua::Lua::new();
            lua.load(format!(
                "GameConfiguration={{GetStartTurn=function() return {start} end}};
                 Game={{GetCurrentGameTurn=function() return {current} end}};
                 GameCapabilities={{HasCapability=function(_, ...) return {normalized} end}}"
            ))
            .exec()
            .map_err(|e| Error::scenario(e.to_string()))?;
            lua.load(code)
                .eval::<String>()
                .map_err(|e| Error::scenario(e.to_string()))
        });
        let mut h = Harness::new(json!({}), backend);
        h.runner.ruleset = Some("RULESET_JAC_BANSEIKI".into());
        assert_eq!(h.runner.check_start_turn(60.0).is_ok(), accepted);
    }
    for response in ["16603", "1|", "nan|1", "1|1|extra"] {
        let mut h = Harness::new(
            json!({}),
            FakeBackend::new().lua(move |_, _| Ok(response.into())),
        );
        h.runner.ruleset = Some("RULESET_JAC_BANSEIKI".into());
        assert!(h.runner.check_start_turn(60.0).is_err(), "{response}");
    }
}

#[test]
fn test_output_scoped_save_name_and_exact_name() {
    let mut a = Harness::new(json!({}), FakeBackend::new());
    let b = Harness::new(json!({}), FakeBackend::new());
    let step = json!({"name":"JAC-Test"});
    assert_ne!(
        a.runner.save_name(&step).unwrap(),
        b.runner.save_name(&step).unwrap()
    );
    assert_eq!(
        a.runner.save_name(&step).unwrap(),
        a.runner
            .save_name(&json!({"action":"load","name":"JAC-Test"}))
            .unwrap()
    );
    assert!(
        a.runner
            .save_name(&json!({"name":"A".repeat(80)}))
            .unwrap()
            .len()
            <= 80
    );
    assert_eq!(
        a.runner
            .save_name(&json!({"name":"JAC-Test","exact_name":true}))
            .unwrap(),
        "JAC-Test"
    );
    std::fs::write(
        a.runner.save_path(&a.runner.save_name(&step).unwrap()),
        b"existing",
    )
    .unwrap();
    assert!(a
        .runner
        .run_step(&json!({"action":"save","name":"JAC-Test"}))
        .unwrap_err()
        .to_string()
        .contains("will not overwrite"));
    assert!(a.calls().is_empty());
}

#[test]
fn test_save_dismisses_blocking_advisor_before_saving() {
    let target = Arc::new(Mutex::new(None::<std::path::PathBuf>));
    let write_target = target.clone();
    let mut h = Harness::new(
        json!({}),
        FakeBackend::new()
            .states(|| {
                Ok(vec![
                    "InGame".into(),
                    "InGameTopOptionsMenu".into(),
                    "AdvisorPopup".into(),
                ])
            })
            .lua(move |_, code| {
                if code.contains("Network.SaveGame") {
                    std::fs::write(write_target.lock().unwrap().as_ref().unwrap(), b"save")
                        .unwrap();
                }
                Ok("true".into())
            }),
    );
    let name = h.runner.save_name(&json!({"name":"JAC-Test"})).unwrap();
    let save = h.runner.save_path(&name);
    *target.lock().unwrap() = Some(save.clone());
    h.runner
        .run_step(&json!({"action":"save","name":"JAC-Test","timeout":10}))
        .unwrap();
    assert_eq!(std::fs::read(save).unwrap(), b"save");
    let calls = h.calls();
    let menu = calls
        .iter()
        .position(|(s, _)| s == "InGameTopOptionsMenu")
        .unwrap();
    let advisor = calls.iter().position(|(s, _)| s == "AdvisorPopup").unwrap();
    let save_call = calls
        .iter()
        .position(|(_, c)| c.contains("Network.SaveGame"))
        .unwrap();
    assert!(menu < advisor && advisor < save_call);
}

#[test]
fn test_flag_validation() {
    let mut data = json!({"name":"flag","steps":[{"action":"configure","ruleset":"RULESET_STANDARD","map_size":"M","speed":"S","seed":1,"players":[{"leader":"L","civ":"C","human":true}],"build_setup":"yes"}]});
    assert!(scenario::validate_scenario(&data)
        .unwrap_err()
        .to_string()
        .contains("build_setup"));
    for key in ["research_metrics", "allow_human_elimination"] {
        data = json!({"name":"flag","steps":[{"action":"autoplay","turns":1,key:"yes"}]});
        assert!(scenario::validate_scenario(&data)
            .unwrap_err()
            .to_string()
            .contains(key));
    }
    data = json!({"name":"flag","steps":[{"action":"save","name":"x","exact_name":1}]});
    assert!(scenario::validate_scenario(&data)
        .unwrap_err()
        .to_string()
        .contains("exact_name"));
}

#[test]
fn test_allow_elimination_observes_target_and_records_turn() {
    let statuses = Arc::new(Mutex::new(VecDeque::from([
        "2|true|false|1|-1",
        "3|false|true|0|-1",
    ])));
    let mut observed_turn = 1_i64;
    let fake = FakeBackend::new().lua(move |_, code| {
        if code.contains("tostring(AutoplayManager.IsActive())") {
            let status = statuses.lock().unwrap().pop_front().unwrap();
            observed_turn = status.split('|').next().unwrap().parse().unwrap();
            return Ok(status.into());
        }
        if code.contains("return tostring(startTurn)") {
            return Ok("1|0".into());
        }
        if code.contains("IsAlive()") {
            return Ok(format!("{observed_turn}|false"));
        }
        Ok("true".into())
    });
    let mut h = Harness::new(json!({}), fake);
    let result = h
        .runner
        .autoplay(
            2,
            10.0,
            AutoplayOptions {
                allow_human_elimination: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(result.contains("1->3"));
    assert!(result.contains("human_elimination_first_observed_turn=2"));
    let rows = h.jsonl("steps.jsonl");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["action"], "autoplay_elimination");
}

#[test]
fn test_autoplay_emits_research_jsonl_for_each_observed_turn() {
    let statuses = Arc::new(Mutex::new(VecDeque::from([
        "2|true|false|1|0",
        "3|false|true|0|0",
    ])));
    let samples = Arc::new(Mutex::new(VecDeque::from([
        "1|0~LEADER_A~true~0~0~~~~",
        "2|0~LEADER_A~true~1~0~~~TECH_A~",
        "3|0~LEADER_A~true~1~0~~~TECH_A~",
    ])));
    let fake = FakeBackend::new().lua(move |_, code| {
        if code.contains("tostring(AutoplayManager.IsActive())") {
            return Ok(statuses.lock().unwrap().pop_front().unwrap().into());
        }
        if code.contains("for _, id in ipairs(configured") {
            return Ok(samples.lock().unwrap().pop_front().unwrap().into());
        }
        if code.contains("return tostring(startTurn)") {
            return Ok("1|0".into());
        }
        Ok("true".into())
    });
    let mut h = Harness::new(json!({}), fake);
    let detail = h
        .runner
        .autoplay(
            2,
            10.0,
            AutoplayOptions {
                research_metrics: true,
                ..Default::default()
            },
        )
        .unwrap();
    let rows = h.jsonl("autoplay-research-1.jsonl");
    assert_eq!(
        rows.iter()
            .map(|v| v["turn"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(rows[1]["completed_techs"], json!(["TECH_A"]));
    assert_eq!(
        support_driver::object_keys(&rows[1]),
        [
            "alive",
            "civic_count",
            "completed_civics",
            "completed_techs",
            "current_civic",
            "current_tech",
            "leader",
            "observed",
            "phase",
            "slot",
            "tech_count",
            "turn"
        ]
    );
    assert!(detail.contains("autoplay-research-1.jsonl"));
}

#[test]
fn test_early_stop_resume_clears_done_flag_and_waits_for_new_end_event() {
    let statuses = Arc::new(Mutex::new(VecDeque::from([
        "2|false|true|0|0",
        "2|false|true|0|0",
        "3|true|false|1|0",
        "4|false|true|0|0",
    ])));
    let clock = Arc::new(ManualClock::default());
    let clock_for_lua = clock.clone();
    let mut status_calls = 0;
    let mut observed_turn = 1_i64;
    let fake = FakeBackend::new().lua(move |_, code| {
        if code.contains("tostring(AutoplayManager.IsActive())") {
            status_calls += 1;
            if status_calls == 2 {
                clock_for_lua.advance(6.0);
            }
            let status = statuses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or("4|false|true|0|0")
                .to_string();
            observed_turn = status.split('|').next().unwrap().parse().unwrap();
            return Ok(status);
        }
        if code.contains("return tostring(startTurn)") {
            return Ok("1|0".into());
        }
        if code.contains("IsAlive()") {
            return Ok(format!("{observed_turn}|true"));
        }
        Ok("true".into())
    });
    let mut h = Harness::new(json!({}), fake);
    h.runner.clock = clock;
    let detail = h.runner.autoplay(3, 1000.0, Default::default()).unwrap();
    let resumes: Vec<_> = h
        .calls()
        .into_iter()
        .filter(|(_, c)| c.contains("SetTurns(2)"))
        .collect();
    assert_eq!(resumes.len(), 1);
    let code = &resumes[0].1;
    assert!(
        code.find("JAC_AUTOTEST_AUTO_DONE = false").unwrap()
            < code.find("SetActive(true)").unwrap()
    );
    assert!(detail.contains("1->4"));
}
