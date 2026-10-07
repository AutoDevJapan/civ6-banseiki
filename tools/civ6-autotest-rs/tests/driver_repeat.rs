mod support_driver;

use civ6_autotest_rs::{scenario, Error, ErrorKind};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use support_driver::{FakeBackend, Harness, ManualClock};

fn step(limit: u64) -> Value {
    json!({"action":"repeat_until","max_iterations":limit,"timeout":20,
        "condition":{"type":"lua","state_name":"InGame","code":"return QUALIFIED"},
        "steps":[{"action":"end_turn","turns":1,"timeout":120},
            {"action":"wait_for","condition":{"type":"lua","state_name":"InGame","code":"return ACTIVE"}},
            {"action":"ui_lua","state_name":"Core","code":"return OBSERVED"}]})
}
fn backend(goal: u64, failed_code: &'static str, clock: Arc<ManualClock>) -> FakeBackend {
    let turn = Arc::new(Mutex::new(1_u64));
    FakeBackend::new().lua(move |_, code| {
        if !failed_code.is_empty() && code.contains(failed_code) {
            return Err(Error::unknown("original uncertain reply"));
        }
        let mut turn = turn.lock().unwrap();
        if code.contains("UI.RequestAction(ActionTypes.ACTION_ENDTURN)") {
            *turn += 1;
            clock.advance(1.0);
        }
        if code.trim() == "return QUALIFIED" {
            return Ok((*turn >= goal).to_string());
        }
        if code.trim() == "return Game.GetCurrentGameTurn()" {
            return Ok(turn.to_string());
        }
        Ok("true".into())
    })
}
fn harness(goal: u64, failed_code: &'static str) -> Harness {
    let clock = Arc::new(ManualClock::default());
    let fake = backend(goal, failed_code, clock.clone());
    let mut h = Harness::new(json!({"name":"bounded real turns"}), fake);
    h.runner.clock = clock;
    h
}

#[test]
fn real_turn_sequence_stops_at_initial_success_and_at_last_allowed_success() {
    for (goal, iterations) in [(1, 0), (2, 1), (3, 2)] {
        let mut h = harness(goal, "");
        assert_eq!(
            h.runner.run_step(&step(2)).unwrap(),
            format!("condition met after {iterations} iterations")
        );
        assert_eq!(
            h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
            iterations
        );
        assert_eq!(h.count_code("return OBSERVED"), iterations);
        assert_eq!(h.count_code("return QUALIFIED"), iterations + 1);
        let log = h.log();
        if iterations > 0 {
            assert!(log.contains("human turn 1->2"));
            assert!(log.contains("child=3 ui_lua"));
        }
    }
}

#[test]
fn bounded_false_condition_fails_without_an_extra_turn() {
    let mut h = harness(4, "");
    let error = h.runner.run_step(&step(2)).unwrap_err();
    assert!(error.message.contains("exhausted 2 iterations"));
    assert_eq!(
        h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
        2
    );
    assert!(h.log().contains("iterations=2 value=false"));
}

#[test]
fn uncertain_condition_mutation_wait_and_observation_never_replay() {
    for failed in [
        "return QUALIFIED",
        "UI.RequestAction(ActionTypes.ACTION_ENDTURN)",
        "return ACTIVE",
        "return OBSERVED",
    ] {
        let mut h = harness(4, failed);
        let error = h.runner.run_step(&step(2)).unwrap_err();
        assert_eq!(error.kind, ErrorKind::LuaResponseUnknown);
        assert_eq!(error.message, "original uncertain reply");
        assert_eq!(h.count_code(failed), 1, "{failed}");
        assert!(h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)") <= 1);
        if failed == "return ACTIVE" {
            assert_eq!(h.count_code("return OBSERVED"), 0);
            let boundary: Value = serde_json::from_str(
                &std::fs::read_to_string(h.runner.output.join("failure.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(boundary["error_kind"], "LuaResponseUnknown");
            assert_eq!(boundary["message"], "original uncertain reply");
            assert_eq!(
                boundary["repeat_until"],
                json!({"iteration":1,"child":2,"parent_step":1})
            );
        }
    }
}

#[test]
fn one_absolute_deadline_limits_children_and_forbids_next_turn() {
    let mut h = harness(5, "");
    let mut limited = step(4);
    limited["timeout"] = json!(2.5);
    assert!(h.runner.run_step(&limited).is_err());
    assert_eq!(
        h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
        3
    );
    assert_eq!(h.count_code("return OBSERVED"), 2);
    let budgets = h.timeouts.lock().unwrap();
    assert!(budgets.iter().all(|v| *v <= 2.5));
    assert!(budgets.iter().any(|v| *v <= 0.5));
}

#[test]
fn non_boolean_condition_is_an_error_before_any_mutation() {
    let fake = FakeBackend::new().lua(|_, _| Ok("nil".into()));
    let mut h = Harness::new(json!({"name":"bad condition"}), fake);
    assert!(h
        .runner
        .run_step(&step(2))
        .unwrap_err()
        .message
        .contains("boolean"));
    assert_eq!(
        h.count_code("UI.RequestAction(ActionTypes.ACTION_ENDTURN)"),
        0
    );
}

#[test]
fn malformed_nested_inputs_and_unbounded_turns_fail_lua51_preflight() {
    let valid = step(2);
    let validate =
        |s: Value| scenario::validate_scenario(&json!({"name":"repeat preflight","steps":[s]}));
    validate(valid.clone()).unwrap();
    for count in [json!(0), json!(65), json!(1.5), json!("2"), Value::Null] {
        let mut bad = valid.clone();
        bad["max_iterations"] = count;
        assert!(validate(bad).is_err());
    }
    for child in [
        json!({"action":"repeat_until"}),
        json!({"action":"load","name":"save"}),
        json!({"action":"ui_lua","state_name":"Core","code":"local ="}),
        json!({"action":"wait_for","condition":{"type":"lua","state_name":"Core","code":"local ="}}),
        json!({"action":"ui_lua","state_name":"Core","code":"return true","capture_as":"INNER"}),
    ] {
        let mut bad = valid.clone();
        bad["steps"][2] = child;
        assert!(validate(bad).is_err());
    }
    for count in [json!(2), json!(0), Value::Null] {
        let mut bad = valid.clone();
        bad["steps"][0]["turns"] = count;
        assert!(validate(bad).is_err());
    }
    let mut bad = valid.clone();
    bad["condition"]["code"] = json!("return )");
    assert!(validate(bad).is_err());
    let mut bad = valid;
    bad["steps"][2] = json!({"action":"end_turn","turns":1});
    assert!(validate(bad).is_err());
}
