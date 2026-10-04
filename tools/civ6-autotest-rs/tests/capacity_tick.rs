mod support_fixture;

use mlua::Lua;
use serde_json::{json, Value};
use support_fixture::{action, code, scenario, steps};

fn accepts(source: &str, outcome: Value) -> bool {
    let lua = Lua::new();
    let table = lua.create_table().unwrap();
    for (key, value) in outcome.as_object().unwrap() {
        match value {
            Value::String(text) => table.set(key.as_str(), text.as_str()).unwrap(),
            Value::Bool(flag) => table.set(key.as_str(), *flag).unwrap(),
            Value::Number(number) => table.set(key.as_str(), number.as_i64().unwrap()).unwrap(),
            _ => panic!("unexpected outcome field"),
        }
    }
    lua.globals().set("outcome", table).unwrap();
    lua.load("ExposedMembers={JAC_PersonaBoundary={CaptiveCapacityOutcome=function() return outcome end}}")
        .exec()
        .unwrap();
    lua.load(source).eval::<String>().is_ok()
}

#[test]
fn completed_building_before_turn_is_accepted_without_weakening_final_confirmation() {
    let data = scenario("leader-persona-spy-neutralization-retry-capacity-tick.json");
    let actions = steps(&data);
    let checks: Vec<_> = actions.iter().filter(|s| action(s, "ui_lua")).collect();
    assert_eq!(checks.len(), 2);
    let before = code(checks[0]);
    let after = code(checks[1]);
    for (building, stage, capacity) in [
        ("BUILDING_GOV_TALL", "TIER1_BUILT", 1),
        ("BUILDING_GOV_SPIES", "CAPACITY_READY", 2),
    ] {
        let completed = json!({"buildingType":building,"stage":stage,"capacity":capacity,
            "targetBuilt":true,"occupied":1,"captured":1,"turn":25,"progressTurn":25});
        assert!(accepts(before, completed.clone()));
        assert!(!accepts(after, completed.clone()));
        let mut next_turn = completed.clone();
        next_turn["turn"] = json!(26);
        assert!(accepts(after, next_turn));
        for (key, invalid) in [
            ("capacity", json!(3)),
            ("targetBuilt", json!(false)),
            ("occupied", json!(0)),
            ("captured", json!(0)),
        ] {
            let mut damaged = completed.clone();
            damaged[key] = invalid;
            assert!(
                !accepts(before, damaged.clone()),
                "accepted {building} with wrong {key}"
            );
            damaged["turn"] = json!(26);
            assert!(
                !accepts(after, damaged),
                "final confirmation accepted {building} with wrong {key}"
            );
        }
    }
    let pending = json!({"buildingType":"BUILDING_GOV_SPIES","stage":"PROGRESS_PENDING",
        "capacity":1,"targetBuilt":false,"occupied":1,"captured":1,"turn":25,"progressTurn":25});
    assert!(accepts(before, pending.clone()));
    assert!(!accepts(after, pending));
    let turn = actions.iter().find(|s| action(s, "end_turn")).unwrap();
    assert_eq!(turn["turns"], 1);
}
