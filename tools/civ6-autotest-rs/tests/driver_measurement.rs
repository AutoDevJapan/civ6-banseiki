mod support_driver;
use civ6_autotest_rs::{
    judge,
    metrics::TurnMetrics,
    runner::{AutoplayOptions, MetricsRun},
    scenario, Error, ErrorKind,
};
use serde_json::{json, Value};
use std::{collections::VecDeque, fs};
use support_driver::{FakeBackend, Harness};

fn empty_logs(h: &Harness) {
    for name in judge::LOG_NAMES {
        fs::write(h.runner.logs.join(name), b"").unwrap();
    }
}

#[test]
fn test_fast_forward_uses_core_clock_and_discards_stale_ui_measurement() {
    let mut statuses = VecDeque::from([
        "1|true|false|2|-1",
        "96|true|false|1|-1",
        "97|false|true|0|0",
    ]);
    let mut research = VecDeque::from([1, 95, 96, 97]);
    let fake=FakeBackend::new().lua(move|_,code|{
        if code.contains("local rows = {}"){return Ok(format!("{}|0~LEADER_A~true~0~0~~~~",research.pop_front().unwrap()));}
        if code.contains("tostring(AutoplayManager.IsActive())"){return Ok(statuses.pop_front().unwrap().into());}
        if code.contains("return tostring(startTurn)") {
            let lua=mlua::Lua::new();lua.load("Game={GetCurrentGameTurn=function() return 1 end,GetLocalPlayer=function() return 0 end};PlayerTypes={NONE=-1};LuaEvents={AutoPlayEnd={Add=function() end,Remove=function() end}};AutoplayManager={IsActive=function() return false end,SetTurns=function() end,SetReturnAsPlayer=function() end,SetObserveAsPlayer=function() end,SetActive=function() end}").exec().unwrap();
            return Ok(lua.load(code).eval::<String>().unwrap());
        }
        Ok("true".into())
    });
    let mut h = Harness::new(json!({"test_mode":{"skipped_turns":94}}), fake);
    let options = AutoplayOptions {
        record_metrics: true,
        research_metrics: true,
        start_turn_override: Some(95),
        ..Default::default()
    };
    assert!(h
        .runner
        .autoplay(2, 10.0, options)
        .unwrap()
        .contains("95->97"));
    assert_eq!(
        h.runner.turn_metrics[0]
            .recorder
            .rows
            .iter()
            .map(|r| r["turn"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [95, 96, 97]
    );
    assert_eq!(
        h.jsonl("autoplay-research-1.jsonl")
            .iter()
            .map(|r| r["turn"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [95, 96, 97]
    );
}

#[test]
fn test_save_load_measure_actual_file_and_wait_until_ready() {
    let mut h = Harness::new(json!({}), FakeBackend::new());
    empty_logs(&h);
    h.runner.baseline = Some(judge::snapshot_logs(&h.runner.logs).unwrap());
    let path = h
        .runner
        .save_path(&h.runner.save_name(&json!({"name":"checkpoint"})).unwrap());
    let log = h.runner.logs.join("Lua.log");
    let clock = h.clock.clone();
    h.runner.backend = Box::new(
        FakeBackend::new()
            .states(|| {
                Ok(vec![
                    "InGame".into(),
                    "FrontEnd".into(),
                    "LoadScreen".into(),
                ])
            })
            .lua(move |_, code| {
                if code.contains("return tostring(Game.GetCurrentGameTurn())") {
                    return Ok("21".into());
                }
                if code.contains("Network.SaveGame") {
                    fs::write(&path, b"save".repeat(100)).unwrap();
                    clock.advance(2.0);
                }
                if code.contains("Network.LoadGame") {
                    clock.advance(0.5);
                    fs::write(&log, "LoadScreen: OnLoadGameViewStateDone\n").unwrap();
                }
                if code.contains("return Game ~= nil and Game.GetLocalPlayer() >= 0") {
                    clock.advance(2.0);
                }
                Ok("true".into())
            }),
    );
    let save = h
        .runner
        .run_step(&json!({"action":"save","name":"checkpoint","metrics":true}))
        .unwrap();
    let load = h
        .runner
        .run_step(&json!({"action":"load","name":"checkpoint","metrics":true}))
        .unwrap();
    let mut csv = csv::Reader::from_path(h.runner.output.join("save-load-metrics.csv")).unwrap();
    let rows: Vec<_> = csv.records().map(|r| r.unwrap()).collect();
    assert_eq!(
        rows.iter().map(|r| &r[1]).collect::<Vec<_>>(),
        ["save", "load"]
    );
    assert_eq!(rows.iter().map(|r| &r[3]).collect::<Vec<_>>(), ["21", "21"]);
    assert_eq!(
        rows.iter().map(|r| &r[4]).collect::<Vec<_>>(),
        ["400", "400"]
    );
    assert_eq!(
        rows.iter().map(|r| &r[5]).collect::<Vec<_>>(),
        ["2.0", "2.5"]
    );
    assert!(save.contains("save_bytes=400"));
    assert!(load.contains("load_seconds=2.5"));
    assert_eq!(
        h.runner.save_load_metrics.summary()["load_seconds"]["median"],
        2.5
    );
}

#[test]
fn test_result_keeps_partial_measurements_on_failed_run() {
    let mut statuses = VecDeque::from([
        Ok("2|true|false|2|0".to_string()),
        Err(Error::scenario("fixture stopped after one turn")),
    ]);
    let mut observed_turn = 1_i64;
    let fake = FakeBackend::new().lua(move |_, code| {
        if code.contains("return tostring(startTurn)") {
            Ok("1|0".into())
        } else if code.contains("tostring(AutoplayManager.IsActive())") {
            let status = statuses.pop_front().unwrap()?;
            observed_turn = status.split('|').next().unwrap().parse().unwrap();
            Ok(status)
        } else if code.contains("IsAlive()") {
            Ok(format!("{observed_turn}|true"))
        } else {
            Ok("true".into())
        }
    });
    let mut h = Harness::new(
        json!({"name":"partial","judge_profile":"generic","steps":[{"action":"autoplay","turns":3,"metrics":true}]}),
        fake,
    );
    empty_logs(&h);
    h.runner.output = h.root.path().join("run");
    h.runner.save_load_metrics = civ6_autotest_rs::metrics::SaveLoadMetrics::new(
        h.runner.output.join("save-load-metrics.csv"),
        false,
    );
    h.runner.auto_restart = false;
    assert_eq!(h.runner.run().unwrap(), 1);
    let result: Value =
        serde_json::from_slice(&fs::read(h.runner.output.join("result.json")).unwrap()).unwrap();
    assert_eq!(result["passed"], false);
    assert_eq!(result["completed_steps"], 0);
    assert!(result["failure"]
        .as_str()
        .unwrap()
        .contains("fixture stopped after one turn"));
    let run = &result["metrics"]["autoplay"][0];
    assert_eq!(run["pid"], 123);
    assert_eq!(run["requested_turns"], 3);
    assert_eq!(run["last_turn"], 2);
    assert_eq!(run["completed"], false);
    assert_eq!(result["metrics"]["turn_summary"]["observed_turns"], 1);
    assert!(h.runner.output.join(run["csv"].as_str().unwrap()).is_file());
}

#[test]
fn test_save_load_flags_require_boolean() {
    for action in ["save", "load"] {
        let e = scenario::validate_scenario(
            &json!({"name":"bad","steps":[{"action":action,"name":"test","metrics":"true"}]}),
        )
        .unwrap_err();
        assert!(e.message.contains("metrics must be boolean"));
    }
}

#[test]
fn test_result_separates_continuous_memory_from_post_load_memory() {
    let mut h = Harness::new(json!({}), FakeBackend::new());
    let out = h.runner.output.clone();
    let mut continuous = TurnMetrics::new(out.join("continuous.jsonl"), 1, 10.0, false);
    continuous
        .start(json!({"working_set_bytes":80,"private_bytes":100}))
        .unwrap();
    continuous
        .sample_at(
            2,
            json!({"working_set_bytes":400,"private_bytes":500}),
            12.0,
        )
        .unwrap();
    let mut post_load = TurnMetrics::new(out.join("post-load.jsonl"), 301, 12.0, false);
    post_load
        .start(json!({"working_set_bytes":40,"private_bytes":50}))
        .unwrap();
    post_load
        .sample_at(
            302,
            json!({"working_set_bytes":45,"private_bytes":60}),
            13.0,
        )
        .unwrap();
    h.runner.turn_metrics = vec![
        MetricsRun {
            recorder: continuous,
            pid: 123,
            requested_turns: 1,
            group: "CONTINUOUS".into(),
        },
        MetricsRun {
            recorder: post_load,
            pid: 123,
            requested_turns: 1,
            group: "POST_LOAD".into(),
        },
    ];
    h.runner.write_result().unwrap();
    let result: Value =
        serde_json::from_slice(&fs::read(out.join("result.json")).unwrap()).unwrap();
    assert_eq!(
        result["metrics"]["groups"]["CONTINUOUS"]["private_bytes"]["growth"],
        400
    );
    assert_eq!(
        result["metrics"]["groups"]["POST_LOAD"]["private_bytes"]["growth"],
        10
    );
    assert_eq!(
        result["metrics"]["turn_summary"]["private_bytes"]["growth"],
        -40
    );
}

#[test]
fn test_interruption_is_recorded_and_summary_write_error_does_not_replace_it() {
    for fail_write in [false, true] {
        let mut h = Harness::new(
            json!({"name":"interrupted","judge_profile":"generic","steps":[{"action":"autoplay","turns":1}]}),
            FakeBackend::new(),
        );
        empty_logs(&h);
        h.runner.output = h.root.path().join("run");
        let output = h.runner.output.clone();
        h.runner.backend = Box::new(FakeBackend::new().lua(move |_, code| {
            if code.contains("LuaEvents.JAC_Notebooks_Close") {
                if fail_write {
                    fs::create_dir(output.join("result.json")).unwrap();
                }
                return Err(Error::interrupted("fixture interruption"));
            }
            Ok("true".into())
        }));
        h.runner.auto_restart = false;
        let error = h.runner.run().unwrap_err();
        assert_eq!(error.kind, ErrorKind::Interrupted);
        assert!(error.message.contains("fixture interruption"));
        assert!(h
            .runner
            .failure
            .as_ref()
            .unwrap()
            .contains("KeyboardInterrupt"));
        if !fail_write {
            let result: Value =
                serde_json::from_slice(&fs::read(h.runner.output.join("result.json")).unwrap())
                    .unwrap();
            assert_eq!(result["passed"], false);
            assert!(result["failure"]
                .as_str()
                .unwrap()
                .contains("fixture interruption"));
        }
    }
}

#[test]
fn test_metric_groups_are_validated() {
    for (value, enabled) in [
        (json!("../bad"), true),
        (json!(1), true),
        (json!("CONTINUOUS"), false),
    ] {
        assert!(scenario::validate_scenario(&json!({"name":"bad group","steps":[{"action":"autoplay","turns":1,"metrics":enabled,"metrics_group":value}]})).unwrap_err().message.contains("metrics_group"));
    }
}

#[test]
fn test_shipped_scenarios_validate_offline() {
    let mut count = 0;
    for entry in
        fs::read_dir(civ6_autotest_rs::repo_root().join("tools/civ6-autotest/scenarios")).unwrap()
    {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|s| s == "json") {
            let data: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            scenario::validate_scenario(&data)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            count += 1;
        }
    }
    assert!(count >= 60);
    let data = json!({"name":"invalid","judge_profile":"growth","steps":[{"action":"pause","seconds":0.1}]});
    assert!(scenario::validate_scenario(&data)
        .unwrap_err()
        .message
        .contains("expected_units"));
}

#[test]
fn test_strict_judge_rejects_official_screen_errors() {
    let logs = |lua: &str| {
        std::collections::BTreeMap::from([
            ("Lua.log".into(), lua.into()),
            ("Modding.log".into(), "".into()),
            ("Database.log".into(), "".into()),
        ])
    };
    for message in [
        "Runtime Error: GreatPeoplePopup.lua:123: attempt to index nil",
        "UI.DataError: EraComplete cannot show this era",
        "Runtime Error: official screen",
        "UI.DataError: official screen",
        "EraComplete cannot show",
    ] {
        let plain = judge::judge(&logs(message), "generic", None, false);
        assert!(plain.iter().all(|c| c.passed));
        let strict = judge::judge(&logs(message), "generic", None, true);
        assert_eq!(
            strict
                .iter()
                .filter(|c| !c.passed)
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["Scenario: all Lua errors"]
        );
    }
    assert!(judge::judge(&logs(""), "generic", None, true)
        .iter()
        .all(|c| c.passed));
    assert!(scenario::validate_scenario(&json!({"name":"strict","strict_lua_errors":"true","steps":[{"action":"pause","seconds":0.1}]})).unwrap_err().message.contains("strict_lua_errors"));
}
