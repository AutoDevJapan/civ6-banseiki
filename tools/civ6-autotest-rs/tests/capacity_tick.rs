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
            "targetBuilt":true,"occupied":1,"captured":1,"mapSpies":0,"turn":25,"progressTurn":25});
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
        "capacity":1,"targetBuilt":false,"occupied":1,"captured":1,"mapSpies":0,
        "turn":25,"progressTurn":25});
    assert!(accepts(before, pending.clone()));
    assert!(!accepts(after, pending));
    let produced = json!({"buildingType":"BUILDING_GOV_SPIES","stage":"CAPACITY_READY",
        "capacity":2,"targetBuilt":true,"occupied":2,"captured":1,"mapSpies":1,
        "turn":26,"progressTurn":25});
    assert!(accepts(before, produced.clone()));
    assert!(accepts(after, produced.clone()));
    let early_pending = json!({"buildingType":"BUILDING_GOV_SPIES","stage":"PROGRESS_PENDING",
        "capacity":2,"targetBuilt":false,"occupied":2,"captured":1,"mapSpies":1,
        "turn":25,"progressTurn":25});
    assert!(accepts(before, early_pending.clone()));
    assert!(!accepts(after, early_pending.clone()));
    let mut later_pending = early_pending.clone();
    later_pending["turn"] = json!(26);
    assert!(!accepts(after, later_pending));
    let mut same_turn_completed = produced.clone();
    same_turn_completed["turn"] = json!(25);
    assert!(!accepts(after, same_turn_completed));
    for (key, invalid) in [
        ("occupied", json!(1)),
        ("occupied", json!(3)),
        ("mapSpies", json!(0)),
        ("mapSpies", json!(2)),
        ("captured", json!(0)),
        ("captured", json!(2)),
    ] {
        let mut damaged = early_pending.clone();
        damaged[key] = invalid.clone();
        assert!(!accepts(before, damaged));
        let mut damaged = produced.clone();
        damaged[key] = invalid;
        assert!(!accepts(after, damaged));
    }
    for (key, invalid) in [("capacity", json!(3)), ("targetBuilt", json!(false))] {
        let mut damaged = produced.clone();
        damaged[key] = invalid;
        assert!(!accepts(before, damaged.clone()));
        assert!(!accepts(after, damaged));
    }
    let turn = actions.iter().find(|s| action(s, "end_turn")).unwrap();
    assert_eq!(turn["turns"], 1);
}
