mod support_driver;

use civ6_autotest_rs::tuner::{state_available, state_name_eq, state_position};
use serde_json::json;
use support_driver::{FakeBackend, Harness};

#[test]
fn wrappers_take_priority_in_either_order_even_over_duplicate_stock_contexts() {
    for (states, expected) in [
        (vec!["StagingRoom", "JAC_JapanStagingRoom"], 1),
        (vec!["JAC_JapanStagingRoom", "StagingRoom"], 0),
        (vec!["MainMenu", "mainmenu", "jac_japanmainmenu"], 2),
    ] {
        let name = if states[0].contains("StagingRoom") {
            "StagingRoom"
        } else {
            "MAINMENU"
        };
        assert_eq!(state_position(states, name), Some(expected));
    }
}

#[test]
fn ordinary_states_keep_unicode_casefold_and_unique_lookup() {
    assert_eq!(state_position(["Straße"], "STRASSE"), Some(0));
    assert_eq!(state_position(["MainMenu"], "mainmenu"), Some(0));
    assert_eq!(state_position(["MainMenu", "MAINMENU"], "MainMenu"), None);
    assert_eq!(state_position(["InGame", "ingame"], "InGame"), None);
    assert_eq!(state_position(["InGame"], "Missing"), None);
    assert_eq!(
        state_position(["JAC_JapanMainMenu"], "JAC_JapanMainMenu"),
        Some(0)
    );
    assert_eq!(state_position(["MainMenu"], "JAC_JapanMainMenu"), None);
    assert!(!state_name_eq("MainMenu", "JAC_JapanMainMenu"));
}

#[test]
fn availability_rejects_duplicate_aliases_and_does_not_accept_other_mod_names() {
    for states in [
        vec!["MainMenu", "JAC_JapanMainMenu", "jac_japanmainmenu"],
        vec!["Other_MainMenu", "MainMenu_Custom"],
        vec![" JAC_JapanMainMenu"],
    ] {
        let states = states.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert!(!state_available(&states, "MainMenu"));
    }
}

#[test]
fn runner_state_wait_uses_the_same_unique_wrapper_selection() {
    for (states, success) in [
        (vec!["JAC_JapanMainMenu"], true),
        (vec!["StagingRoom", "JAC_JapanStagingRoom"], true),
        (
            vec!["MainMenu", "JAC_JapanMainMenu", "jac_japanmainmenu"],
            false,
        ),
        (vec!["Other_MainMenu"], false),
        (vec!["MainMenu", "MAINMENU"], false),
    ] {
        let name = if states[0] == "StagingRoom" {
            "StagingRoom"
        } else {
            "MainMenu"
        };
        let fake =
            FakeBackend::new().states(move || Ok(states.iter().map(|s| s.to_string()).collect()));
        let mut h = Harness::new(json!({}), fake);
        let result = h.runner.run_step(
            &json!({"action":"wait_for","timeout":1,"condition":{"type":"state","name":name}}),
        );
        assert_eq!(result.is_ok(), success, "{result:?}");
        assert!(h.calls().is_empty());
    }
}
