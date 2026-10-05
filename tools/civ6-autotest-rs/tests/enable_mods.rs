mod support_fixture;

use civ6_autotest_rs::scenario::{
    enable_mods_code, enable_mods_code_with_scope, validate_scenario,
};
use mlua::{Lua, Table};
use serde_json::{json, Value};
use std::{fs, path::Path};
use support_fixture::{expect_error, lua_error};

const BASE: u8 = 1;
const MIDDLE: u8 = 2;
const LEAF: u8 = 3;
const SIBLING: u8 = 4;
const KEEP: u8 = 5;
const OFF: u8 = 6;
const MISSING: u8 = 7;
const OFFICIAL: u8 = 8;

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
            definition.set("Official", false).unwrap();
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

    fn run_exclusive(&self, enabled: &[u8], disabled: &[u8]) -> mlua::Result<String> {
        let enabled = enabled.iter().map(|&n| id(n)).collect::<Vec<_>>();
        let disabled = disabled.iter().map(|&n| id(n)).collect::<Vec<_>>();
        self.lua
            .load(enable_mods_code_with_scope(&enabled, &disabled, true))
            .eval()
    }

    fn set_official(&self, number: u8) {
        self.lua
            .globals()
            .get::<Table>("MODS")
            .unwrap()
            .get::<Table>(id(number))
            .unwrap()
            .set("Official", true)
            .unwrap();
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
fn exclusive_selection_disables_extra_user_mods_but_keeps_official_content() {
    let g = Graph::new(
        &[(BASE, &[]), (LEAF, &[BASE]), (KEEP, &[]), (OFFICIAL, &[])],
        &[KEEP],
        false,
    );
    g.set_official(OFFICIAL);
    assert_eq!(
        g.run_exclusive(&[KEEP], &[]).unwrap(),
        [id(LEAF), id(BASE)].join(", ")
    );
    assert_eq!(g.disabled(), [id(LEAF), id(BASE)]);
    assert!(g.enabled(KEEP) && g.enabled(OFFICIAL));
    assert!(!g.enabled(BASE) && !g.enabled(LEAF));
    assert_eq!(g.updates(), 1);
}

#[test]
fn exclusive_selection_rejects_a_kept_mod_depending_on_an_extra_user_mod() {
    let g = Graph::new(&[(BASE, &[]), (KEEP, &[BASE])], &[], false);
    lua_error(
        g.run_exclusive(&[KEEP], &[]),
        "scenario omits required user mod",
    );
    assert!(g.disabled().is_empty());
    assert!(g.enabled(BASE) && g.enabled(KEEP));
    assert_eq!(g.updates(), 0);
}

#[test]
fn exclusive_selection_rejects_disabled_extra_dependency_before_changes() {
    let g = Graph::new(&[(BASE, &[]), (KEEP, &[BASE])], &[BASE, KEEP], false);
    lua_error(
        g.run_exclusive(&[KEEP], &[]),
        "scenario omits required user mod",
    );
    assert!(!g.enabled(BASE) && !g.enabled(KEEP));
    assert_eq!(g.updates(), 0);
}

#[test]
fn normal_300_prestart_checks_configured_user_mods_after_update() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../civ6-autotest/scenarios/japan-15km-300turn-full-metrics.json");
    let scenario: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let steps = scenario["steps"].as_array().unwrap();
    let ids = steps[0]["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let start = steps
        .iter()
        .position(|step| step["action"] == "start_game")
        .unwrap();
    assert_eq!(steps[start - 1]["action"], "ui_lua");
    assert_eq!(steps[start - 1]["state_name"], "FrontEnd");
    let check = steps[start - 1]["code"].as_str().unwrap();

    let lua = Lua::new();
    let definitions = lua.create_table().unwrap();
    for (i, id) in ids.iter().enumerate() {
        let definition = lua.create_table().unwrap();
        definition.set("Id", id.as_str()).unwrap();
        definition.set("Enabled", true).unwrap();
        definition.set("Official", false).unwrap();
        definition
            .set("Dependencies", lua.create_table().unwrap())
            .unwrap();
        definitions.set(i + 1, definition).unwrap();
    }
    lua.load(include_str!("fixtures/modding_stub.lua"))
        .call::<()>(definitions)
        .unwrap();
    lua.load(enable_mods_code_with_scope(&ids, &[], true))
        .eval::<String>()
        .unwrap();
    assert_eq!(
        lua.load(check).eval::<String>().unwrap(),
        "configured user mods=7"
    );

    let configured = lua.globals().get::<Table>("CONFIGURED").unwrap();
    let extra = lua.create_table().unwrap();
    extra.set("Id", id(MISSING)).unwrap();
    extra.set("Official", false).unwrap();
    configured.set(ids.len() + 1, extra).unwrap();
    lua_error(
        lua.load(check).eval::<String>(),
        "unexpected configured user mod",
    );
    configured.set(ids.len() + 1, mlua::Value::Nil).unwrap();
    configured.set(ids.len(), mlua::Value::Nil).unwrap();
    lua_error(
        lua.load(check).eval::<String>(),
        "configured user mod missing",
    );
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
