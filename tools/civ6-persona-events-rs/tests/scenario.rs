use civ6_persona_events_rs::scenario::{self, LEADER};
use mlua::Lua;
use serde_json::Value;
use std::{fs, path::PathBuf};

fn template() -> Value {
    serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../civ6-autotest/scenarios/leader-persona-15km.json"),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn finite_diagnostic_has_one_payment_sender_and_only_its_new_save() {
    let scenario = scenario::build(&template()).unwrap();
    civ6_autotest_rs::scenario::validate_scenario(&scenario).unwrap();
    let steps = scenario["steps"].as_array().unwrap();
    assert_eq!(
        steps.len(),
        38,
        "retain the finite 38-step diagnostic; the driver adds its initial restart"
    );
    let lua = Lua::new();
    let mut appoint = 0;
    let mut promote = 0;
    let mut load_at = None;
    for (i, step) in steps.iter().enumerate() {
        let code = step["code"]
            .as_str()
            .or(step["condition"]["code"].as_str())
            .unwrap_or("");
        lua.load(code)
            .into_function()
            .unwrap_or_else(|e| panic!("Lua 5.1 step {i}: {e}"));
        appoint += usize::from(code.contains("PlayerOperations.APPOINT_GOVERNOR"));
        promote += usize::from(code.contains("PlayerOperations.PROMOTE_GOVERNOR"));
        if step["action"] == "save" || step["action"] == "load" {
            assert_eq!(step["name"], "JAC-Todo324-GameplayEvents-Cost2-Proved");
        }
        if step["action"] == "load" {
            load_at = Some(i);
        }
        assert!(!code.contains("Events[") && !code.contains("JAC_LeaderPersonaNativeRead"));
        if step["state_name"] == "GameCore_Tuner" {
            assert!(!code.contains("Events") && !code.contains("ExposedMembers"));
        }
    }
    assert_eq!((appoint, promote), (1, 1));
    let attach = steps
        .iter()
        .position(|s| s["action"] == "start_native_production_admission")
        .unwrap();
    let join = steps
        .iter()
        .position(|s| {
            s["code"]
                .as_str()
                .is_some_and(|s| s.contains("OnStart='JAC_LeaderPersonaJoin'"))
        })
        .unwrap();
    let initial = &steps[attach + 1..join];
    assert_eq!(
        initial
            .iter()
            .filter(|s| s["code"]
                .as_str()
                .is_some_and(|s| s.contains("OnStart='JAC_LeaderPersonaEventsRepublish'")))
            .count(),
        1
    );
    assert_eq!(
        initial
            .iter()
            .filter(|s| s["capture_as"] == "TODO324_ADMISSION_CORE_READS")
            .count(),
        1
    );
    assert!(!serde_json::to_string(initial)
        .unwrap()
        .contains(".Reconcile("));
    assert!(!serde_json::to_string(initial)
        .unwrap()
        .contains("p:GetProperty("));
    let after = &steps[load_at.unwrap() + 1..];
    assert_eq!(
        after
            .iter()
            .filter(|s| s["equals_capture"] == "TODO324_EVENTS_SETTLED")
            .count(),
        2
    );
    let capture = steps
        .iter()
        .position(|s| s["capture_as"] == "TODO324_EVENTS_SETTLED")
        .unwrap();
    let finalized = steps
        .iter()
        .position(|s| {
            s["code"]
                .as_str()
                .is_some_and(|c| c.contains("OnStart='JAC_LeaderPersonaFinalize'"))
        })
        .unwrap();
    let saved = steps.iter().position(|s| s["action"] == "save").unwrap();
    assert!(finalized < capture && capture < saved);
    let body = steps[capture]["code"].as_str().unwrap();
    for field in [
        "sequence=s.sequence",
        "receipts=s.receipts",
        "'_MERIT'",
        "meritPresent=merit~=nil",
        "meritType=type(merit)",
    ] {
        assert!(body.contains(field), "{field}");
    }
    assert!(!serde_json::to_string(after)
        .unwrap()
        .contains("RequestPlayerOperation"));
    assert!(!serde_json::to_string(after).unwrap().contains("Reconcile"));
    assert!(!serde_json::to_string(&scenario)
        .unwrap()
        .contains("Reconcile"));
    assert!(!serde_json::to_string(&scenario)
        .unwrap()
        .contains("SpySave45"));
    assert_eq!(
        steps
            .iter()
            .filter(|s| s["action"] == "start_native_production_admission")
            .count(),
        1
    );
    assert_eq!(
        steps
            .iter()
            .filter(|s| s["action"] == "stop_native_production_admission")
            .count(),
        1
    );
    assert!(serde_json::to_string(&scenario).unwrap().contains(LEADER));
}

#[test]
fn a_partial_resume_or_changed_source_is_not_a_fresh_template() {
    let mut partial = template();
    partial["steps"][2]["action"] = "load".into();
    assert!(scenario::build(&partial).is_err());
    let mut wrong = template();
    wrong["steps"][1]["players"][1]["leader"] = "LEADER_JAC_ASHINA_MORIKIYO".into();
    assert!(scenario::build(&wrong).is_err());
}
