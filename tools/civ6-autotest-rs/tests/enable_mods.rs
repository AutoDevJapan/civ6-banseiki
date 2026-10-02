mod support_fixture;

use civ6_autotest_rs::scenario::{enable_mods_code, validate_scenario};
use mlua::{Lua, Table};
use serde_json::json;
use support_fixture::{expect_error, lua_error};

const BASE: u8 = 1;
const MIDDLE: u8 = 2;
const LEAF: u8 = 3;
const SIBLING: u8 = 4;
const KEEP: u8 = 5;
const OFF: u8 = 6;
const MISSING: u8 = 7;

#[test]
fn persona_runs_select_their_required_mods_after_another_diagnostic() {
    const PERSONA: &str = "91cf7b65-54bb-4f69-a452-19a65527cde2";
    const BOUNDARY: &str = "4a72b1c0-958b-4bd5-81e4-a094f86ce643";
    const ABSORPTION: &str = "61320778-2e75-4c11-9009-94101b519aba";
    for name in [
        "leader-persona-15km.json",
        "leader-persona-baseline-15km.json",
        "leader-persona-native-eligibility-15km.json",
        "leader-persona-city-loss-15km.json",
        "leader-persona-spy-access-15km.json",
    ] {
        let data = support_fixture::scenario(name);
        validate_scenario(&data).unwrap();
        let step = &data["steps"][0];
        assert_eq!(step["action"], "enable_mods");
        let strings = |key: &str| -> Vec<String> {
            step[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap().to_owned())
                .collect()
        };
        let enabled = strings("ids");
        let disabled = strings("disable_ids");
        let ids: std::collections::BTreeSet<_> = enabled
            .iter()
            .chain(&disabled)
            .cloned()
            .chain([BOUNDARY.to_owned(), ABSORPTION.to_owned()])
            .collect();
        let lua = Lua::new();
        let definitions = lua.create_table().unwrap();
        for (index, id) in ids.iter().enumerate() {
            let definition = lua.create_table().unwrap();
            definition.set("Id", id.as_str()).unwrap();
            definition.set("Enabled", true).unwrap();
            let dependencies = if id == BOUNDARY {
                vec![PERSONA]
            } else {
                vec![]
            };
            definition
                .set(
                    "Dependencies",
                    lua.create_sequence_from(dependencies).unwrap(),
                )
                .unwrap();
            definitions.set(index + 1, definition).unwrap();
        }
        lua.load(include_str!("fixtures/modding_stub.lua"))
            .call::<()>(definitions)
            .unwrap();
        lua.load(enable_mods_code(&enabled, &disabled))
            .exec()
            .unwrap();
        let mods: Table = lua.globals().get("MODS").unwrap();
        let boundary: Table = mods.get(BOUNDARY).unwrap();
        let expected_boundary = matches!(
            name,
            "leader-persona-city-loss-15km.json" | "leader-persona-spy-access-15km.json"
        );
        assert!(
            matches!(boundary.get::<mlua::Value>("Enabled").unwrap(),
            mlua::Value::Boolean(value) if value == expected_boundary),
            "{name}"
        );
        let absorption: Table = mods.get(ABSORPTION).unwrap();
        assert!(
            matches!(
                absorption.get::<mlua::Value>("Enabled").unwrap(),
                mlua::Value::Boolean(false)
            ),
            "{name}"
        );
        let persona: Table = mods.get(PERSONA).unwrap();
        let expected = name != "leader-persona-baseline-15km.json";
        assert!(
            matches!(persona.get::<mlua::Value>("Enabled").unwrap(), mlua::Value::Boolean(value) if value == expected),
            "{name}"
        );
    }
}

fn id(n: u8) -> String {
    format!("abcdef{n:02x}-1234-1234-1234-123456789abc")
}

struct Graph {
    lua: Lua,
}

impl Graph {
    fn new(edges: &[(u8, &[u8])], disabled: &[u8], uppercase: bool) -> Self {
        let lua = Lua::new();
        let definitions = lua.create_table().unwrap();
        for (i, (number, dependencies)) in edges.iter().enumerate() {
            let definition = lua.create_table().unwrap();
            let spell = |n| {
                let s = id(n);
                if uppercase {
                    s.to_uppercase()
                } else {
                    s
                }
            };
            definition.set("Id", spell(*number)).unwrap();
            definition
                .set("Enabled", !disabled.contains(number))
                .unwrap();
            let deps = lua
                .create_sequence_from(dependencies.iter().map(|n| spell(*n)))
                .unwrap();
            definition.set("Dependencies", deps).unwrap();
            definitions.set(i + 1, definition).unwrap();
        }
        lua.load(include_str!("fixtures/modding_stub.lua"))
            .call::<()>(definitions)
            .unwrap();
        Self { lua }
    }

    fn run(&self, enabled: &[u8], disabled: &[u8]) -> mlua::Result<String> {
        let enabled = enabled.iter().map(|&n| id(n)).collect::<Vec<_>>();
        let disabled = disabled.iter().map(|&n| id(n)).collect::<Vec<_>>();
        self.lua.load(enable_mods_code(&enabled, &disabled)).eval()
    }

    fn run_uppercase(&self, enabled: &[u8], disabled: &[u8]) -> mlua::Result<String> {
        let enabled = enabled
            .iter()
            .map(|&n| id(n).to_uppercase())
            .collect::<Vec<_>>();
        let disabled = disabled
            .iter()
            .map(|&n| id(n).to_uppercase())
            .collect::<Vec<_>>();
        self.lua.load(enable_mods_code(&enabled, &disabled)).eval()
    }

    fn disabled(&self) -> Vec<String> {
        self.lua
            .globals()
            .get::<Table>("DISABLED")
            .unwrap()
            .sequence_values::<String>()
            .collect::<mlua::Result<_>>()
            .unwrap()
    }

    fn enabled_events(&self) -> Vec<String> {
        self.lua
            .globals()
            .get::<Table>("ENABLED")
            .unwrap()
            .sequence_values::<String>()
            .collect::<mlua::Result<_>>()
            .unwrap()
    }

    fn enabled(&self, number: u8) -> bool {
        self.lua
            .globals()
            .get::<Table>("MODS")
            .unwrap()
            .get::<Table>(id(number))
            .unwrap()
            .get("Enabled")
            .unwrap()
    }

    fn updates(&self) -> i64 {
        self.lua.globals().get("UPDATES").unwrap()
    }

    fn set_refusal(&self, name: &str, number: u8) {
        self.lua.globals().set(name, id(number)).unwrap();
    }
}

#[test]
fn transitive_dependents_are_disabled_first_once_and_reported() {
    let g = Graph::new(
        &[
            (BASE, &[]),
            (MIDDLE, &[BASE]),
            (LEAF, &[MIDDLE, SIBLING]),
            (SIBLING, &[BASE]),
            (KEEP, &[]),
            (OFF, &[BASE]),
        ],
        &[KEEP, OFF],
        true,
    );
    let reported = g.run_uppercase(&[KEEP], &[BASE, SIBLING, MISSING]).unwrap();
    let expected = [LEAF, MIDDLE, SIBLING, BASE].map(id);
    assert_eq!(g.disabled(), expected);
    assert_eq!(reported, expected.join(", "));
    assert!(g.enabled(KEEP));
    assert!(!g.enabled(OFF));
    assert_eq!(g.updates(), 1);
}

#[test]
fn traversal_includes_enabled_descendants_of_a_disabled_intermediate() {
    let g = Graph::new(
        &[
            (BASE, &[]),
            (MIDDLE, &[BASE]),
            (LEAF, &[MIDDLE]),
            (KEEP, &[]),
            (OFF, &[]),
        ],
        &[MIDDLE, OFF],
        false,
    );
    g.run(&[KEEP], &[BASE]).unwrap();
    assert_eq!(g.disabled(), [id(LEAF), id(BASE)]);
    assert!(g.enabled(KEEP));
    assert!(!g.enabled(OFF));
}

#[test]
fn requested_dependents_are_preserved_and_conflicts_fail_before_mutations() {
    for disabled in [vec![], vec![LEAF]] {
        let g = Graph::new(
            &[
                (BASE, &[]),
                (MIDDLE, &[BASE]),
                (LEAF, &[MIDDLE]),
                (OFF, &[]),
            ],
            &disabled,
            false,
        );
        lua_error(
            g.run_uppercase(&[LEAF], &[OFF, BASE]),
            "scenario enables a dependent mod",
        );
        assert!(g.disabled().is_empty());
        assert!(g.enabled_events().is_empty());
        assert!(g.enabled(BASE));
        assert_eq!(g.enabled(LEAF), !disabled.contains(&LEAF));
    }
}

#[test]
fn dependency_cycle_fails_before_mutations() {
    let g = Graph::new(
        &[(BASE, &[MIDDLE]), (MIDDLE, &[BASE]), (KEEP, &[])],
        &[],
        false,
    );
    lua_error(g.run(&[KEEP], &[BASE]), "mod dependency cycle");
    assert!(g.disabled().is_empty());
}

#[test]
fn disable_verifies_the_engine_state_and_names_the_failed_mod() {
    let g = Graph::new(&[(BASE, &[]), (KEEP, &[])], &[], false);
    g.set_refusal("REFUSE_DISABLE", BASE);
    lua_error(
        g.run(&[KEEP], &[BASE]),
        &format!("mod stayed enabled: {}", id(BASE)),
    );
    assert!(g.enabled(BASE));
    assert_eq!(g.updates(), 0);
}

#[test]
fn no_disable_targets_leaves_unrelated_mods_enabled() {
    let g = Graph::new(&[(BASE, &[]), (KEEP, &[BASE])], &[KEEP], false);
    assert_eq!(g.run(&[KEEP], &[]).unwrap(), "");
    assert!(g.disabled().is_empty());
    assert!(g.enabled(BASE));
    assert!(g.enabled(KEEP));
}

#[test]
fn late_disable_rejection_is_detected_before_any_settings_change() {
    let g = Graph::new(&[(BASE, &[]), (LEAF, &[BASE]), (KEEP, &[])], &[KEEP], false);
    g.set_refusal("REFUSE_DISABLE_REASON", BASE);
    lua_error(
        g.run(&[KEEP], &[BASE]),
        &format!("cannot disable mod: {}", id(BASE)),
    );
    assert!(g.disabled().is_empty());
    assert!(g.enabled_events().is_empty());
    assert!(g.enabled(LEAF) && g.enabled(BASE));
    assert!(!g.enabled(KEEP));
}

#[test]
fn missing_enable_target_fails_before_disabling_other_mods() {
    let g = Graph::new(&[(BASE, &[]), (LEAF, &[BASE])], &[], false);
    lua_error(
        g.run(&[MISSING], &[BASE]),
        &format!("mod is not installed: {}", id(MISSING)),
    );
    assert!(g.disabled().is_empty());
    assert!(g.enabled(LEAF) && g.enabled(BASE));
}

#[test]
fn engine_refusal_restores_changes_without_enabling_disabled_intermediates() {
    let g = Graph::new(
        &[
            (BASE, &[]),
            (MIDDLE, &[BASE]),
            (LEAF, &[MIDDLE]),
            (KEEP, &[]),
        ],
        &[MIDDLE, KEEP],
        false,
    );
    g.set_refusal("REFUSE_DISABLE", BASE);
    lua_error(
        g.run(&[KEEP], &[BASE]),
        &format!("mod stayed enabled: {}", id(BASE)),
    );
    assert_eq!(g.disabled(), vec![id(LEAF)]);
    assert_eq!(g.enabled_events(), vec![id(LEAF)]);
    assert!(g.enabled(BASE) && g.enabled(LEAF));
    assert!(!g.enabled(MIDDLE) && !g.enabled(KEEP));
}

#[test]
fn failed_restoration_identifies_the_mod_still_disabled() {
    let g = Graph::new(&[(BASE, &[]), (LEAF, &[BASE]), (KEEP, &[])], &[], false);
    g.set_refusal("REFUSE_DISABLE", BASE);
    g.set_refusal("REFUSE_ENABLE", LEAF);
    lua_error(
        g.run(&[KEEP], &[BASE]),
        &format!("restoration failed: {}", id(LEAF)),
    );
    assert!(!g.enabled(LEAF));
    assert!(g.enabled(BASE));
}

#[test]
fn late_enable_failure_restores_disabled_and_newly_enabled_mods() {
    let g = Graph::new(
        &[
            (BASE, &[]),
            (LEAF, &[BASE]),
            (KEEP, &[]),
            (SIBLING, &[]),
            (OFF, &[]),
        ],
        &[KEEP, SIBLING, OFF],
        false,
    );
    g.set_refusal("REFUSE_ENABLE_REASON", KEEP);
    lua_error(
        g.run(&[SIBLING, KEEP], &[BASE]),
        &format!("cannot enable mod: {}", id(KEEP)),
    );
    for number in [BASE, LEAF] {
        assert!(g.enabled(number));
    }
    for number in [KEEP, SIBLING, OFF] {
        assert!(!g.enabled(number));
    }
    assert_eq!(g.updates(), 1);
}

#[test]
fn implicitly_enabled_prerequisite_is_restored_when_enable_throws() {
    let g = Graph::new(
        &[(BASE, &[]), (LEAF, &[BASE]), (KEEP, &[OFF]), (OFF, &[])],
        &[KEEP, OFF],
        false,
    );
    g.set_refusal("FAIL_AFTER_DEPENDENCIES", KEEP);
    lua_error(g.run(&[KEEP], &[BASE]), "engine enable failed");
    assert_eq!(g.disabled(), [id(LEAF), id(BASE), id(OFF)]);
    assert!(g.enabled(BASE) && g.enabled(LEAF));
    assert!(!g.enabled(KEEP) && !g.enabled(OFF));
}

#[test]
fn enable_failure_without_disable_targets_restores_the_selection() {
    let g = Graph::new(
        &[(BASE, &[]), (KEEP, &[]), (SIBLING, &[])],
        &[KEEP, SIBLING],
        false,
    );
    g.set_refusal("REFUSE_ENABLE_REASON", KEEP);
    lua_error(
        g.run(&[SIBLING, KEEP], &[]),
        &format!("cannot enable mod: {}", id(KEEP)),
    );
    assert!(g.enabled(BASE));
    assert!(!g.enabled(KEEP) && !g.enabled(SIBLING));
}

#[test]
fn enable_and_disable_overlap_is_case_insensitive() {
    expect_error(
        validate_scenario(&json!({"name":"overlap","steps":[{"action":"enable_mods",
        "ids":[id(BASE).to_uppercase()],"disable_ids":[id(BASE)]}]})),
        "invalid disable_ids",
    );
}
