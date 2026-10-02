use super::support;
use civ6_reports_rs::tuning as report;
use mlua::Lua;
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs};

fn summarize(lua: &str, steps: &[Value]) -> Value {
    let temp = support::tempdir();
    support::artifact(temp.path(), lua, steps);
    report::summarize(temp.path()).unwrap()
}

fn coverage(snapshots: &[String], rows: &[Value], falls: &[Value]) -> (Vec<String>, usize) {
    report::coverage(snapshots, rows, falls, &BTreeMap::new()).unwrap()
}

fn assert_failure_row_preserved(row: Value) {
    let mut steps = support::completed_steps();
    steps.push(row.clone());
    let data = summarize("", &steps);
    assert_eq!(data["complete_200_turns"], false);
    assert_eq!(data["measurement_complete"], false);
    assert_eq!(data["failures"], json!([row]));
    assert_eq!(
        data["autoplay"],
        json!(vec!["autoplay completed 50 turns"; 4])
    );
    assert!(report::markdown(&data).contains("200ターン完了とログ判定: 未達。"));
}

#[test]
fn failure_rows_missing_detail_are_preserved() {
    assert_failure_row_preserved(json!({"status": "FAIL", "action": "run"}));
}

#[test]
fn failure_rows_null_detail_are_preserved() {
    assert_failure_row_preserved(json!({"status": "FAIL", "action": "run", "detail": null}));
}

#[test]
fn failure_rows_nontext_action_are_preserved() {
    for action in [Value::Null, json!(7), json!({"stage": "run"})] {
        assert_failure_row_preserved(json!({"status": "FAIL", "action": action}));
    }
}

pub(super) fn full_coverage_fixture() -> (Vec<String>, Vec<Value>) {
    let ids = (0..22).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let snapshots = [1, 51, 101, 151, 201].map(|turn| {
        let mut parts = vec![format!("checkpoint turn={turn} era=ERA_ANCIENT targets=56"), "era-values grievances=10 tensions=10 trade-minimum-change=0".into()];
        parts.extend((0..22).map(|i| format!("house player={i} civ=CIVILIZATION_JAC_TEST alive=true cities=2 castles=2 legitimacy=100 age=NORMAL policy=nil pact-lead=nil")));
        parts.extend(report::EXPECTED_REIGNS.iter().enumerate().filter(|(_, (_, due))| *due <= turn).map(|(index, (_, due))| format!("judgment-roster reign-index={} turn={due} players={ids}", index + 1)));
        parts.join(" ; ")
    }).to_vec();
    let rows = report::EXPECTED_REIGNS
        .iter()
        .flat_map(|(name, turn)| {
            (0..22).map(move |i| json!({"reign": name, "turn": turn, "player": i}))
        })
        .collect();
    (snapshots, rows)
}

#[test]
fn test_counts_ages_policies_and_case_vs_respondent_outcomes() {
    let lua = "[1] JAC_Ages: [JAC_AGES] judged reign=REIGN_KOJI turn=66 length=65 era=1 engineEra=1 player=0 age=NORMAL score=4 golden>=7 dark>=2 policy=\n[1] JAC_Ages: [JAC_AGES] judged reign=REIGN_KOJI turn=66 length=65 era=1 engineEra=1 player=1 age=DARK score=0 golden>=7 dark>=2 policy=POLICY_JAC_TEST\n[1] [JAC_ARB] request player=0 pair=0-1 turn=20 power=1\n[1] [JAC_ARB] refused player=1 pair=0-1 turn=21 legitimacy=60\n[1] [JAC_ARB] request player=2 pair=2-3 turn=40 power=1\n[1] [JAC_ARB] accepted pair=2-3 turn=41 truce_turns=10\n[1] [JAC_AGES] points player=3 part=ko +8 score=14 key=ko:1 turn=101\n";
    let data = summarize(lua, &[]);
    assert_eq!(
        data["reigns"]["REIGN_KOJI"]["ages"],
        json!({"NORMAL": 1, "DARK": 1})
    );
    assert_eq!(
        data["reigns"]["REIGN_KOJI"]["policies"],
        json!({"POLICY_JAC_TEST": 1})
    );
    assert_eq!(data["judgments"][1]["score"], 0);
    assert_eq!(data["judgments"][1]["golden"], 7);
    assert_eq!(data["arbitration"]["request"], 2);
    assert_eq!(data["arbitration"]["accepted"], 1);
    assert_eq!(data["arbitration"]["refused"], 1);
    assert_eq!(data["arbitration"]["refused_cases"], 1);
    assert_eq!(data["arbitration"]["request_intervals"], json!([20]));
    let falls: Vec<_> = data["elimination_point_events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| json!({"capturer": r["capturer"], "player": r["player"], "turn": r["turn"]}))
        .collect();
    assert_eq!(
        falls,
        vec![json!({"capturer": 3, "player": 1, "turn": 101})]
    );
    assert_eq!(data["complete_200_turns"], false);
}

#[test]
fn test_duplicate_judgment_is_rejected() {
    let row = "[JAC_AGES] judged reign=REIGN_KOJI turn=66 length=65 player=0 age=NORMAL score=3 golden>=6 dark>=1 policy=\n";
    let temp = support::tempdir();
    support::artifact(temp.path(), &row.repeat(2), &[]);
    assert!(report::summarize(temp.path())
        .unwrap_err()
        .to_string()
        .contains("duplicate age judgment"));
}

#[test]
fn test_completion_requires_target_and_passed_judge() {
    let mut steps = support::completed_steps();
    steps.pop();
    assert_eq!(summarize("", &steps)["complete_200_turns"], false);
    steps.push(json!({"action": "judge", "status": "OK", "detail": "all passed"}));
    assert_eq!(summarize("", &steps)["complete_200_turns"], true);
    steps.push(json!({"action": "run", "status": "FAIL", "detail": "failed"}));
    assert_eq!(summarize("", &steps)["complete_200_turns"], false);
}

#[test]
fn test_completion_rejects_missing_failed_synthetic_and_reloaded_results() {
    for case in ["missing", "failed", "synthetic", "load-step", "load-log"] {
        let temp = support::tempdir();
        let mut steps = support::completed_steps();
        support::artifact(temp.path(), "", &steps);
        let result = temp.path().join("result.json");
        match case {
            "missing" => fs::remove_file(&result).unwrap(),
            "failed" => fs::write(&result, json!({"passed": false}).to_string()).unwrap(),
            "synthetic" => fs::write(
                &result,
                json!({"passed": true, "test_mode": {"skipped_turns": 192}}).to_string(),
            )
            .unwrap(),
            "load-step" => {
                steps.push(json!({"action": "1/18 load", "status": "OK", "detail": "loaded"}));
                support::write_steps(temp.path(), &steps);
            }
            "load-log" => fs::write(
                temp.path().join("Lua.log"),
                "[JAC_ARB] loaded shogunate_end_turn=120\n".repeat(2),
            )
            .unwrap(),
            _ => unreachable!(),
        }
        assert_eq!(
            report::summarize(temp.path()).unwrap()["complete_200_turns"],
            false,
            "{case}"
        );
    }
}

#[test]
fn test_observer_handles_engine_functions_returning_no_values() {
    let lua = Lua::new();
    lua.load(r#"
Game={GetCurrentGameTurn=function() return 16603 end,
 GetProperty=function() end, GetEras=function() return {GetCurrentEra=function() return 0 end} end}
GameConfiguration={GetStartTurn=function() return 16603 end}
local city={GetOwner=function() return 0 end}
ExposedMembers={JAC_UnificationCastles={CountedCities=function() return {city} end}}
GameInfo={Eras={[0]={EraType='ERA_ANCIENT'}}, Eras_XP2=function()
 local done=false; return function() if not done then done=true; return {EraType='ERA_ANCIENT',GrievanceDecayRate=10,TensionDecayRate=10,TradeRouteMinimumEndTurnChange=0} end end end}
Players={}; PlayerConfigurations={}
for id=0,21 do
 Players[id]={IsAlive=function() return true end,GetCities=function() return {GetCount=function() return 2 end} end,GetProperty=function() end}
 PlayerConfigurations[id]={GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_TEST' end}
end
"#).exec().unwrap();
    let scenario =
        support::repo_json("tools/civ6-autotest/scenarios/japan-15km-200turn-tuning.json");
    let code = scenario["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| {
            s["action"] == "ui_lua" && s["code"].as_str().unwrap().contains("checkpoint turn=")
        })
        .unwrap()["code"]
        .as_str()
        .unwrap();
    let value: String = lua.load(code).eval().unwrap();
    assert_eq!(value.matches(" ; house player=").count(), 22);
    assert!(value.starts_with("checkpoint turn=1 era=ERA_ANCIENT targets=1 ; era-values grievances=10 tensions=10 trade-minimum-change=0 ; "));
    assert!(value.contains("house player=21 civ=CIVILIZATION_JAC_TEST alive=true cities=2 castles=0 legitimacy=nil age=nil policy=nil pact-lead=nil"));
    lua.load("Game.GetCurrentGameTurn=function() return 16669 end\nGame.GetProperty=function(self,key) if key=='JAC_AGES_LEDGER' then return {reign=1,history={r1={turn=66,p1={age='DARK'},p0={age='NORMAL'}}}} end end").exec().unwrap();
    let value: String = lua.load(code).eval().unwrap();
    assert!(value.contains(" ; judgment-roster reign-index=1 turn=66 players=0,1"));
}

#[test]
fn test_variant_preserves_original_settings_and_progression() {
    let base = support::repo_json("tools/civ6-autotest/scenarios/japan-15km-200turn-full.json");
    let variant =
        support::repo_json("tools/civ6-autotest/scenarios/japan-15km-200turn-tuning.json");
    let readers: Vec<_> = variant["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| {
            s["action"] == "ui_lua" && s["code"].as_str().unwrap().contains("checkpoint turn=")
        })
        .collect();
    assert_eq!(readers.len(), 5);
    let mut restored = Vec::new();
    for step in variant["steps"].as_array().unwrap() {
        if readers.contains(&step) {
            continue;
        }
        let mut step = step.clone();
        if step["action"] == "autoplay" {
            assert_eq!(step["metrics"], Value::Bool(true));
            if base["steps"][restored.len()].get("metrics").is_none() {
                step.as_object_mut().unwrap().remove("metrics");
            }
        }
        restored.push(step);
    }
    assert_eq!(json!(restored), base["steps"]);
}

#[test]
fn test_coverage_checks_saved_player_rosters_for_every_reign() {
    let (snapshots, mut rows) = full_coverage_fixture();
    assert_eq!(coverage(&snapshots, &rows, &[]), (vec![], 5));
    rows.retain(|r| !(r["reign"] == "REIGN_GENKI" && r["player"] == 13));
    let (issues, count) = coverage(&snapshots, &rows, &[]);
    assert_eq!(count, 5);
    assert!(issues.contains(&"age log player coverage mismatch: REIGN_GENKI".into()));
}

#[test]
fn test_five_repeated_checkpoints_are_not_complete() {
    let (snapshots, rows) = full_coverage_fixture();
    let (issues, count) = coverage(&vec![snapshots.last().unwrap().clone(); 5], &rows, &[]);
    assert_eq!(count, 1);
    assert!(issues.contains(&"duplicate checkpoint: T+201".into()));
    assert!(issues.contains(&"checkpoint turns mismatch: [201]".into()));
}

#[test]
fn test_missing_saved_rosters_do_not_prove_complete_measurement() {
    let (snapshots, rows) = full_coverage_fixture();
    let snapshots: Vec<_> = snapshots
        .iter()
        .map(|s| {
            s.split(" ; ")
                .filter(|part| !part.starts_with("judgment-roster "))
                .collect::<Vec<_>>()
                .join(" ; ")
        })
        .collect();
    let (issues, count) = coverage(&snapshots, &rows, &[]);
    assert_eq!(count, 5);
    assert!(issues.contains(&"saved judgment rosters do not cover all six expected reigns".into()));
}

#[test]
fn test_missing_player_in_both_history_and_log_is_not_complete() {
    let (snapshots, mut rows) = full_coverage_fixture();
    let snapshots: Vec<_> = snapshots
        .iter()
        .map(|s| {
            s.replace(
                "players=0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21",
                "players=0,1,2,3,4,5,6,7,8,9,10,11,12,14,15,16,17,18,19,20,21",
            )
        })
        .collect();
    rows.retain(|r| r["player"] != 13);
    let (issues, _) = coverage(&snapshots, &rows, &[]);
    assert!(issues.contains(&"saved roster survivor coverage mismatch: REIGN_GENKI".into()));
}

#[test]
fn test_survivors_are_reconciled_with_dated_elimination_events() {
    let (mut snapshots, mut rows) = full_coverage_fixture();
    for index in [3, 4] {
        snapshots[index] = snapshots[index].replace(
            "house player=13 civ=CIVILIZATION_JAC_TEST alive=true",
            "house player=13 civ=CIVILIZATION_JAC_TEST alive=false",
        );
        snapshots[index] = snapshots[index]
            .split(" ; ")
            .map(|part| {
                if part.starts_with("judgment-roster ")
                    && part
                        .split("turn=")
                        .nth(1)
                        .unwrap()
                        .split(' ')
                        .next()
                        .unwrap()
                        .parse::<i64>()
                        .unwrap()
                        >= 119
                {
                    part.replace(",13,", ",")
                } else {
                    part.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" ; ");
    }
    rows.retain(|r| !(r["player"] == 13 && r["turn"].as_i64().unwrap() >= 119));
    let falls = [json!({"capturer": 10, "player": 13, "turn": 115})];
    assert_eq!(coverage(&snapshots, &rows, &falls), (vec![], 5));
    let (issues, _) = coverage(&snapshots, &rows, &[]);
    assert!(issues.contains(&"cannot reconcile survivors with elimination events: T+151".into()));
    for row in &mut rows {
        row["log_position"] = json!(if row["turn"] == 110 {
            1000 + row["player"].as_i64().unwrap()
        } else {
            row["turn"].as_i64().unwrap() * 100
        });
    }
    let mut same_turn_fall =
        [json!({"capturer": 10, "player": 13, "turn": 110, "log_position": 2000})];
    assert_eq!(coverage(&snapshots, &rows, &same_turn_fall), (vec![], 5));
    same_turn_fall[0]["log_position"] = json!(900);
    let (issues, _) = coverage(&snapshots, &rows, &same_turn_fall);
    assert!(issues.contains(&"saved roster survivor coverage mismatch: REIGN_GENKI".into()));
}

#[test]
fn test_missing_house_fields_and_era_fields_are_not_complete() {
    let (snapshots, rows) = full_coverage_fixture();
    for missing in [
        "civ=CIVILIZATION_JAC_TEST ",
        "cities=2 ",
        "castles=2 ",
        "legitimacy=100 ",
        "age=NORMAL ",
        "policy=nil ",
        "pact-lead=nil",
    ] {
        let mut broken = snapshots.clone();
        broken[0] = broken[0].replacen(missing, "", 1);
        let (issues, _) = coverage(&broken, &rows, &[]);
        assert!(
            issues.contains(&"incomplete house observations: T+1".into()),
            "{missing}"
        );
    }
    let mut broken = snapshots;
    broken[0] = broken[0].replacen("tensions=10 ", "", 1);
    let (issues, _) = coverage(&broken, &rows, &[]);
    assert!(issues.contains(&"incomplete era observations: T+1".into()));
}
