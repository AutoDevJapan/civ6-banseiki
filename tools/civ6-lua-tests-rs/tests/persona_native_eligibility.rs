//! Tests the diagnostic scenario's direct native eligibility query in an
//! independent Lua 5.1 engine fixture. These are not Civ6 run results.

use civ6_lua_tests_rs::strict::{ChunkExt, FromLuaMulti, TableExt};
use civ6_lua_tests_rs::{exec, new_lua, read_repo};
use mlua::{Function, Lua, MultiValue, Table};
use serde_json::Value as Json;

const DIAGNOSTIC: &str =
    "tools/civ6-autotest/scenarios/leader-persona-native-eligibility-15km.json";
const FULL: &str = "tools/civ6-autotest/scenarios/leader-persona-15km.json";
const PASSED: &str = "484 engine eligibility checks passed";

fn scenario(path: &str) -> Json {
    serde_json::from_str(&read_repo(path)).unwrap()
}

fn steps(scenario: &Json) -> &[Json] {
    scenario["steps"].as_array().expect("scenario steps")
}

struct Native {
    lua: Lua,
    scenario: Json,
    gate: String,
}

impl Native {
    fn new() -> Self {
        let scenario = scenario(DIAGNOSTIC);
        let gate = steps(&scenario)
            .iter()
            .filter_map(|step| step["code"].as_str())
            .find(|code| code.starts_with("-- UI cache: read the ENGINE directly"))
            .expect("native diagnostic gate")
            .to_owned();
        let lua = new_lua();
        exec(&lua, include_str!("../fixtures/persona/native_engine.lua"));
        Self {
            lua,
            scenario,
            gate,
        }
    }

    fn table(&self, name: &str) -> Table {
        self.lua.globals().strict_get(name).unwrap()
    }

    fn nested(&self, name: &str, owner: i64) -> Table {
        self.table(name).strict_get(owner).unwrap()
    }

    fn governors(&self, owner: i64) -> Table {
        let player: Table = self.nested("Players", owner);
        let get: Function = player.strict_get("GetGovernors").unwrap();
        Table::from_results(get.call::<MultiValue>(player).unwrap()).unwrap()
    }

    fn check(&self) -> String {
        self.lua.load(&self.gate).strict_eval().unwrap()
    }

    fn points(&self, owner: i64) -> i64 {
        self.lua
            .load("local id=...; return Players[id]:GetGovernors():GetGovernorPoints()")
            .strict_call(owner)
            .unwrap()
    }

    fn error_contains(&self, fragment: &str) {
        let error = self
            .lua
            .load(&self.gate)
            .exec()
            .expect_err("gate must reject");
        assert!(error.to_string().contains(fragment), "{error}");
    }
}

#[test]
fn test_closed_unjoined_engine_candidates_pass_with_21_positive_ai_budgets() {
    let n = Native::new();
    assert_eq!(n.check(), PASSED);
    assert_eq!(n.points(1), 1);
}

#[test]
fn test_hidden_ui_candidate_cannot_hide_native_can_ever_eligibility() {
    let n = Native::new();
    n.nested("EVER", 1).set(9005, true).unwrap();
    n.error_contains("unjoined native eligibility: 1/LEADER_TEST_5/ever=true");
}

#[test]
fn test_no_titles_cannot_hide_native_can_ever_eligibility() {
    let n = Native::new();
    assert_eq!(n.points(0), 0);
    n.nested("EVER", 0).set(9005, true).unwrap();
    n.error_contains("0/LEADER_TEST_5/ever=true");
}

#[test]
fn test_native_can_appoint_is_checked_even_if_can_ever_reports_false() {
    let n = Native::new();
    n.nested("NOW", 1).set(9005, true).unwrap();
    n.error_contains("1/LEADER_TEST_5/ever=false/now=true");
}

#[test]
fn test_wrong_owner_and_already_appointed_unjoined_governor_are_rejected() {
    let n = Native::new();
    n.table("PROPERTIES")
        .set("JAC_PERSONA_LEADER_TEST_5_OWNER", 0)
        .unwrap();
    n.table("PROPERTIES")
        .set("JAC_PERSONA_LEADER_TEST_5_STATE", "GREAT_PERSON")
        .unwrap();
    n.nested("UNITS", 0).set(1, 2005).unwrap();
    n.nested("EVER", 0).set(9005, true).unwrap();
    n.nested("NOW", 0).set(9005, true).unwrap();
    assert_eq!(n.check(), PASSED);
    n.nested("APPOINTED", 1).set(9005, true).unwrap();
    n.error_contains("1/LEADER_TEST_5/ever=false/now=false/appointed=true/nativeType=5");
}

#[test]
fn test_stale_owner_without_a_native_person_cannot_bypass_the_qualification_gate() {
    let n = Native::new();
    n.table("PROPERTIES")
        .set("JAC_PERSONA_LEADER_TEST_5_OWNER", 0)
        .unwrap();
    n.table("PROPERTIES")
        .set("JAC_PERSONA_LEADER_TEST_5_STATE", "GREAT_PERSON")
        .unwrap();
    n.nested("EVER", 0).set(9005, true).unwrap();
    n.nested("NOW", 0).set(9005, true).unwrap();
    n.error_contains("joined owner lacks exactly one native Great Person");
    n.nested("UNITS", 0).set(1, 2005).unwrap();
    n.nested("UNITS", 0).set(2, 2005).unwrap();
    n.error_contains("joined owner lacks exactly one native Great Person");
}

#[test]
fn test_committed_governor_needs_a_native_instance_and_no_surviving_great_person() {
    let n = Native::new();
    n.table("PROPERTIES")
        .set("JAC_PERSONA_LEADER_TEST_5_OWNER", 0)
        .unwrap();
    n.table("PROPERTIES")
        .set("JAC_PERSONA_LEADER_TEST_5_STATE", "GOVERNOR")
        .unwrap();
    n.error_contains("joined owner lacks a consistent native Governor");
    n.nested("APPOINTED", 0).set(9005, true).unwrap();
    assert_eq!(n.check(), PASSED);
    n.nested("UNITS", 0).set(1, 2005).unwrap();
    n.error_contains("joined owner lacks a consistent native Governor");
}

#[test]
fn test_all_22_players_and_types_are_required() {
    let n = Native::new();
    let rows = n.table("ROWS");
    rows.set(rows.raw_len(), mlua::Value::Nil).unwrap();
    n.error_contains("all 22 houses and 22 persona types required");
}

#[test]
fn test_hash_index_disagreement_is_not_treated_as_a_closed_gate() {
    let n = Native::new();
    let disagree: Function = n.lua.globals().strict_get("index_disagreement").unwrap();
    n.governors(0).set("CanAppointGovernor", disagree).unwrap();
    n.error_contains("native Hash/Index eligibility disagreement");
}

#[test]
fn test_native_methods_must_exist_in_the_ui_context() {
    let n = Native::new();
    n.governors(0)
        .set("CanEverAppointGovernor", mlua::Value::Nil)
        .unwrap();
    n.error_contains("native eligibility getters unavailable in UI");
}

#[test]
fn test_diagnostic_keeps_real_turn_payment_reloads_and_existing_full_assertions() {
    let n = Native::new();
    let diag = steps(&n.scenario);
    let full = scenario(FULL);
    let original = steps(&full);
    assert_eq!(original.len(), 113);
    assert_eq!(&diag[..5], &original[..5]);
    let actions = |action: &str| {
        diag.iter()
            .filter(|step| step["action"] == action)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        actions("end_turn")
            .iter()
            .map(|s| &s["turns"])
            .collect::<Vec<_>>(),
        vec![&Json::from(1)]
    );
    assert!(actions("autoplay").is_empty());
    for index in [35, 38, 25] {
        assert!(
            diag.contains(&original[index]),
            "missing full scenario step {index}"
        );
    }
    let saves = actions("save")
        .iter()
        .map(|step| step["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(saves, ["JAC-Native-Joined", "JAC-Native-Evolved"]);
    let loads = actions("load")
        .iter()
        .map(|step| step["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(loads, saves);
    assert_eq!(
        diag.iter()
            .filter_map(|s| s["capture_as"].as_str())
            .collect::<Vec<_>>(),
        [
            "JAC_NATIVE_UNJOINED",
            "JAC_NATIVE_JOINED_PARTIAL",
            "JAC_NATIVE_EVOLVED"
        ]
    );
    assert_eq!(
        diag.iter()
            .filter_map(|s| s["equals_capture"].as_str())
            .collect::<Vec<_>>(),
        ["JAC_NATIVE_JOINED_PARTIAL", "JAC_NATIVE_EVOLVED"]
    );
    let gates = diag
        .iter()
        .filter(|step| step["code"].as_str() == Some(n.gate.as_str()));
    assert!(gates.clone().count() >= 1);
    assert!(gates.clone().all(|step| step["state_name"] == "InGame"));
    assert!(diag
        .iter()
        .filter_map(|s| s["code"].as_str())
        .any(|code| code.contains("native appointment exposed before full cost authorization")));
}

#[test]
fn test_all_diagnostic_chunks_compile() {
    let n = Native::new();
    let mut compiled = 0;
    for step in steps(&n.scenario) {
        let code = step["code"]
            .as_str()
            .filter(|code| !code.is_empty())
            .or_else(|| step["condition"]["code"].as_str());
        if let Some(code) = code {
            n.lua
                .load(code)
                .into_function()
                .unwrap_or_else(|e| panic!("{} chunk failed: {e}", step["action"]));
            compiled += 1;
        }
    }
    assert!(compiled > 0);
}
