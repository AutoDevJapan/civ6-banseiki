use super::support;
use civ6_reports_rs::{arbitration as report, scenario as builder};
use mlua::Lua;
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs};

const LOG: &str = include_str!("../fixtures/arbitration.log");

fn coverage_log() -> String {
    let mut log = String::new();
    for turn in 1..202 {
        log.push_str(&format!(
            "[JAC_ARB] ai weariness roster turn={turn} cities=0:2,1:2,2:2,3:2\n"
        ));
        for player in 0..4 {
            log.push_str(&format!(
                "[JAC_ARB] ai weariness player={player} turn={turn} total=0 observed=2 cities=2\n"
            ));
        }
    }
    log
}

fn summary(log: &str, test_mode: Value, result: Option<bool>, coverage: &str) -> Value {
    let temp = support::tempdir();
    support::artifact(
        temp.path(),
        &(log.to_owned() + coverage),
        &support::completed_steps(),
    );
    match result {
        Some(passed) => fs::write(
            temp.path().join("result.json"),
            json!({"passed": passed, "test_mode": test_mode}).to_string(),
        )
        .unwrap(),
        None => fs::remove_file(temp.path().join("result.json")).unwrap(),
    }
    report::summarize(temp.path()).unwrap()
}

fn full(log: &str) -> Value {
    summary(log, Value::Null, Some(true), &coverage_log())
}

fn assert_failure_row_preserved(row: Value) {
    let temp = support::tempdir();
    let mut steps = support::completed_steps();
    steps.push(row.clone());
    support::artifact(temp.path(), &(LOG.to_owned() + &coverage_log()), &steps);
    let data = report::summarize(temp.path()).unwrap();
    assert_eq!(data["complete_200_turns"], false);
    assert_eq!(data["measurement_complete"], false);
    assert_eq!(data["failures"], json!([row]));
    assert_eq!(data["requests"], 2);
    assert_eq!(data["issues"], json!([]));
    assert!(report::markdown(&data).contains("未達（部分集計）"));
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

fn remove_first(log: &str, prefix: &str) -> String {
    let line = log
        .split_inclusive('\n')
        .find(|line| line.starts_with(prefix))
        .unwrap();
    log.replace(line, "")
}

#[test]
fn test_outcomes_costs_clamping_and_sources() {
    let data = full(LOG);
    assert_eq!(data["measurement_complete"], true, "{}", data["issues"]);
    assert_eq!(data["requests"], 2);
    assert_eq!(data["sources"], json!({"ai_party": 1, "ai_shogun": 1}));
    assert_eq!(data["outcomes"], json!({"accepted": 1, "refused": 1}));
    assert_eq!(data["refused_respondents"], 2);
    assert_eq!(data["decisions"], json!({"ai:true": 1, "ai:false": 2}));
    assert_eq!(
        data["spending"]["ai_shogun"],
        json!({"gold": 34, "legitimacy": 2})
    );
    assert_eq!(
        (
            data["refusal_loss_intended"].as_i64(),
            data["refusal_loss_applied"].as_i64()
        ),
        (Some(90), Some(75))
    );
    assert_eq!(
        data["blocked_candidate_visits"],
        json!({"gold": 1, "legitimacy": 2, "interval": 3})
    );
    assert_eq!(data["shogun_after_cutoff"], json!([]));
}

#[test]
fn test_missing_or_duplicate_cost_prevents_complete_measurement() {
    for log in [remove_first(LOG, "[JAC_ARB] cost "), LOG.to_owned() + "[JAC_ARB] cost player=2 pair=1-3 turn=14 gold=34 legitimacy=2 gold_before=500.00 gold_after=466.00 legitimacy_before=77 legitimacy_after=75 by=ai_shogun\n"] {
        assert_eq!(full(&log)["measurement_complete"], false, "{log}");
    }
}

#[test]
fn test_missing_or_early_ruling_and_missing_refusal_loss_are_detected() {
    for log in [
        LOG.replace("accepted pair=0-1 turn=3", "accepted pair=0-1 turn=2"),
        remove_first(LOG, "[JAC_ARB] refusal cost "),
    ] {
        assert_eq!(full(&log)["measurement_complete"], false);
    }
}

#[test]
fn test_empowered_third_party_request_at_120_is_detected() {
    let data = full(
        &LOG.replace("turn=14", "turn=120")
            .replace("turn=15", "turn=121"),
    );
    assert_eq!(data["measurement_complete"], false);
    assert_eq!(data["shogun_after_cutoff"], json!([120]));
}

#[test]
fn test_pending_final_turn_is_counted_without_claiming_a_ruling() {
    let tail = "[JAC_ARB] request player=0 pair=0-3 turn=201 power=1\n[JAC_ARB] cost player=0 pair=0-3 turn=201 gold=100 legitimacy=5 gold_before=400.00 gold_after=300.00 legitimacy_before=65 legitimacy_after=60 by=ai_party\n[JAC_ARB] ai request player=0 pair=0-3 turn=201 by=ai_party score=6 duration=24\n";
    let data = full(&(LOG.to_owned() + tail));
    assert_eq!(data["measurement_complete"], true, "{}", data["issues"]);
    assert_eq!(data["unfinished"], 1);
    assert_eq!(data["player_intervals"]["0"], json!([199]));
}

#[test]
fn test_requester_and_pair_interval_violations_are_detected() {
    let data = full(
        &LOG.replace("turn=14", "turn=11")
            .replace("turn=15", "turn=12")
            .replace("player=2", "player=0"),
    );
    assert_eq!(data["measurement_complete"], false);
    support::contains(
        &data["issues"],
        "AI request interval shorter than ten turns",
    );
}

#[test]
fn test_test_clock_jump_never_counts_as_uninterrupted_measurement() {
    assert_eq!(
        summary(
            LOG,
            json!({"skipped_turns": 113}),
            Some(true),
            &coverage_log()
        )["complete_200_turns"],
        false
    );
}

#[test]
fn test_reloaded_log_is_not_an_uninterrupted_measurement() {
    assert_eq!(
        full(&(LOG.to_owned() + "[JAC_ARB] loaded shogunate_end_turn=120\n"))
            ["measurement_complete"],
        false
    );
}

#[test]
fn test_missing_or_failed_result_is_not_complete() {
    for result in [None, Some(false)] {
        assert_eq!(
            summary(LOG, Value::Null, result, &coverage_log())["measurement_complete"],
            false
        );
    }
}

#[test]
fn test_nominal_cost_must_match_actual_balance_delta() {
    for changed in [
        LOG.replace("gold_after=400.00", "gold_after=399.00"),
        LOG.replace("legitimacy_after=35", "legitimacy_after=34"),
        LOG.replace("gold_before=500.00 ", ""),
        LOG.replace("legitimacy_before=40 ", ""),
        LOG.replace("legitimacy_before=40", "legitimacy_before=29")
            .replace("legitimacy_after=35", "legitimacy_after=24"),
        LOG.replace(
            "player=3 pair=1-3 turn=15 legitimacy=15",
            "player=3 pair=1-3 turn=15 legitimacy=30",
        )
        .replace(
            "applied=45 legitimacy_before=60 legitimacy_after=15",
            "applied=30 legitimacy_before=60 legitimacy_after=30",
        ),
    ] {
        assert_eq!(full(&changed)["measurement_complete"], false, "{changed}");
    }
}

#[test]
fn test_weariness_coverage_is_visible_even_without_requests() {
    let data = summary("[JAC_ARB] loaded shogunate_end_turn=120\n[JAC_ARB] ai weariness player=0 turn=2 total=0 observed=0 cities=2\n[JAC_ARB] ai weariness player=1 turn=2 total=0 observed=2 cities=2\n", Value::Null, Some(true), "");
    assert_eq!(data["requests"], 0);
    assert_eq!(
        data["weariness_coverage"],
        json!({"samples": 2, "observed_cities": 2, "total_cities": 4})
    );
    assert!(report::markdown(&data).contains("延べ2/4都市"));
}

#[test]
fn test_weariness_missing_duplicate_or_mismatched_samples_prevent_completion() {
    let full = coverage_log();
    let row = "[JAC_ARB] ai weariness player=0 turn=7 total=0 observed=2 cities=2\n";
    let roster = "[JAC_ARB] ai weariness roster turn=7 cities=0:2,1:2,2:2,3:2\n";
    for changed in [
        "".into(),
        full.replace(row, ""),
        full.clone() + row,
        full.replace(roster, ""),
        full.clone() + roster,
        full.replace(row, &row.replace("observed=2", "observed=3")),
        full.replace(row, &row.replace("cities=2", "cities=3")),
    ] {
        assert_eq!(
            summary(LOG, Value::Null, Some(true), &changed)["measurement_complete"],
            false
        );
    }
}

#[test]
fn test_refusal_cost_must_match_each_respondent_and_pair() {
    for changed in [
        LOG.replace("refusal cost player=1", "refusal cost player=8"),
        LOG.replace(
            "refusal cost player=1 pair=1-3",
            "refusal cost player=1 pair=1-8",
        ),
        LOG.replace("refusal cost player=3", "refusal cost player=1"),
        LOG.replace(
            "refusal cost player=1 pair=1-3 turn=15",
            "refusal cost player=1 pair=1-3 turn=16",
        ),
    ] {
        assert_eq!(full(&changed)["measurement_complete"], false);
    }
}

#[test]
fn test_scenario_generator_matches_fixture_and_compiles_every_gameplay_step() {
    let generated = builder::scenario().unwrap();
    let actual: Value =
        serde_json::from_str(include_str!("../fixtures/court-arbitration-ai.json")).unwrap();
    assert_eq!(generated, actual);
    let steps = actual["steps"].as_array().unwrap();
    assert_eq!(steps.iter().filter(|s| s["action"] == "save").count(), 5);
    assert_eq!(steps.iter().filter(|s| s["action"] == "load").count(), 5);
    let mut in_menu = true;
    for step in steps {
        if step["action"] == "exit_to_menu" {
            in_menu = true;
        } else if step["action"] == "start_game" || step["action"] == "load" {
            assert!(
                in_menu,
                "Network.LoadGame requires FrontEnd; gameplay alone cannot load a save"
            );
            in_menu = false;
        }
    }
    assert_eq!(
        steps
            .iter()
            .filter(|s| s["action"] == "autoplay")
            .map(|s| s["turns"].as_i64().unwrap())
            .sum::<i64>(),
        7
    );
    let lua = Lua::new();
    for (i, step) in steps.iter().enumerate() {
        if let Some(code) = step.get("code") {
            lua.load(code.as_str().unwrap())
                .set_name(format!("step {}", i + 1))
                .into_function()
                .unwrap();
        }
    }
}

#[test]
fn test_saved_snapshot_preserves_nested_ai_state_and_missing_properties() {
    let lua = support::exhausted_party_lua();
    let before: String = lua.load(builder::snapshot()).eval().unwrap();
    assert!(before.contains("Source=ai_party"));
    assert!(before.contains("p0_1={ArmyA=4,ArmyB=4,CitiesA=2,CitiesB=2,Since=1}"));
    assert!(before.contains("0:2:51:{A=0,B=1,Gold=100,GoldAfter=400,GoldBefore=500,Legitimacy=5,LegitimacyAfter=51,LegitimacyBefore=56,Source=ai_party,Turn=2}"));
    lua.load("Players[0]:SetProperty('JAC_ARB_AI_LAST_REQUEST',nil)")
        .exec()
        .unwrap();
    assert_ne!(
        lua.load(builder::snapshot()).eval::<String>().unwrap(),
        before
    );
}

#[test]
fn test_snapshot_checks_saved_costs_instead_of_native_ai_spending_during_load() {
    let lua = support::exhausted_party_lua();
    let before: String = lua.load(builder::snapshot()).eval().unwrap();
    lua.load("Players[3].gold=Players[3].gold+10; Players[0].gold=Players[0].gold+10")
        .exec()
        .unwrap();
    assert_eq!(
        lua.load(builder::snapshot()).eval::<String>().unwrap(),
        before
    );
    lua.load("Players[0]:SetProperty('JAC_ARB_LAST_COST',nil)")
        .exec()
        .unwrap();
    assert_ne!(
        lua.load(builder::snapshot()).eval::<String>().unwrap(),
        before
    );
}

#[test]
fn test_all_live_gameplay_steps_run_with_save_restore_on_lupa() {
    // Execute the same Lua fixtures with embedded Lua 5.1 instead of lupa.
    let data = builder::scenario().unwrap();
    let civs: Vec<_> = data["steps"][1]["players"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["civ"].as_str().unwrap().to_owned())
        .collect();
    let lua = support::arbitration_lua(&civs);
    lua.load(r#"
Players[14].human=true
SET_WAR(14,18,true)
UnitManager={InitUnitValidAdjacentHex=function(id) Players[id].units=Players[id].units+1 end,
 Kill=function(u) local p=Players[u:GetOwner()]; p.units=p.units-1 end}
Game.SetCurrentGameTurn=function(value) CURRENT_TURN=value end
local function copy(v)
 if type(v)~='table' then return v end
 local t={}; for k,x in pairs(v) do t[k]=copy(x) end; return t
end
local saved={}
function STUB_SAVE(name)
 local state={turn=CURRENT_TURN,props=copy(PROPERTY_TABLES),war=copy(WAR),changed=copy(WAR_CHANGED),gold={}}
 for id,p in pairs(Players) do state.gold[id]=p.gold end
 saved[name]=state
end
function STUB_LOAD(name)
 local state=assert(saved[name]); CURRENT_TURN=state.turn; WAR=copy(state.war); WAR_CHANGED=copy(state.changed)
 for i,props in ipairs(PROPERTY_TABLES) do
  for k in pairs(props) do props[k]=nil end
  for k,v in pairs(state.props[i] or {}) do props[k]=copy(v) end
 end
 for id,value in pairs(state.gold) do Players[id].gold=value end
end
START_TURN(1)
"#).exec().unwrap();
    let mut captures = BTreeMap::new();
    let mut executed = 0;
    for (i, step) in data["steps"].as_array().unwrap().iter().enumerate() {
        match step["action"].as_str().unwrap() {
            "ui_lua" => {
                let value: String = lua
                    .load(step["code"].as_str().unwrap())
                    .set_name(format!("step {}", i + 1))
                    .eval()
                    .unwrap();
                executed += 1;
                if let Some(capture) = step.get("capture_as") {
                    captures.insert(capture.as_str().unwrap().to_owned(), value.clone());
                }
                if let Some(capture) = step.get("equals_capture") {
                    assert_eq!(
                        &value,
                        &captures[capture.as_str().unwrap()],
                        "step {}",
                        i + 1
                    );
                }
            }
            "autoplay" => {
                let turn: i64 = lua.load("return JAC_ARB.GameTurn()").eval().unwrap();
                lua.globals()
                    .get::<mlua::Function>("START_TURN")
                    .unwrap()
                    .call::<()>(turn + step["turns"].as_i64().unwrap())
                    .unwrap();
            }
            "save" => lua
                .globals()
                .get::<mlua::Function>("STUB_SAVE")
                .unwrap()
                .call::<()>(step["name"].as_str().unwrap())
                .unwrap(),
            "load" => lua
                .globals()
                .get::<mlua::Function>("STUB_LOAD")
                .unwrap()
                .call::<()>(step["name"].as_str().unwrap())
                .unwrap(),
            _ => {}
        }
    }
    assert_eq!(
        (
            executed,
            captures.len(),
            lua.load("return JAC_ARB.GameTurn()").eval::<i64>().unwrap()
        ),
        (25, 5, 121)
    );
    let logs: Vec<_> = lua
        .globals()
        .get::<mlua::Table>("LOG")
        .unwrap()
        .sequence_values::<String>()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        logs.iter()
            .filter(|line| line.starts_with("[JAC_ARB] ai request "))
            .count(),
        3
    );
    assert_eq!(
        logs.iter()
            .filter(
                |line| line.starts_with("[JAC_ARB] ai request ") && line.contains(" by=ai_shogun ")
            )
            .count(),
        2
    );
}
