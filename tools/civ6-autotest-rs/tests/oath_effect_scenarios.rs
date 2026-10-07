use civ6_autotest_rs::{repo_root, scenario::validate_scenario};
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn all_66_oath_scenarios_and_first_capability_diagnosis_pass_driver_schema() {
    let root = repo_root();
    let load = |path: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(root.join(path)).expect(path)).expect(path)
    };
    let catalogue = load("documents/poc/data/sengoku-oaths.json");
    let inventory = load("documents/poc/data/oath-effect-scenarios.json");
    assert_eq!(inventory["live_verified"], false);
    assert_eq!(inventory["oath_count"], 66);
    assert_eq!(inventory["house_count"], 22);
    assert_eq!(inventory["max_turns"], 16);
    let modinfo =
        std::fs::read_to_string(root.join("poc/JAC_PoC_SengokuTrees/JAC_PoC_SengokuTrees.modinfo"))
            .expect("MOD UI contexts");
    let contexts: BTreeSet<_> = modinfo
        .split("<LuaContext>")
        .skip(1)
        .map(|part| {
            part.split_once("</LuaContext>")
                .expect("closed LuaContext")
                .0
        })
        .collect();
    let mut expected = BTreeSet::new();
    let mut houses = BTreeSet::new();
    for house in catalogue["houses"].as_array().expect("houses") {
        let id = house["id"].as_str().expect("house ID");
        assert!(houses.insert(format!("CIVILIZATION_JAC_{id}")));
        let oaths = house["oaths"].as_array().expect("oaths");
        assert_eq!(oaths.len(), 3);
        for oath in oaths {
            let key = format!("JAC_OATH_{id}_{}", oath["id"].as_str().expect("oath ID"));
            let path = format!(
                "tools/civ6-autotest/scenarios/oath-effects/{}.json",
                key.to_lowercase()
            );
            assert!(expected.insert(path.clone()));
            let data = load(&path);
            validate_scenario(&data).unwrap_or_else(|e| panic!("{path}: {e}"));
            let steps = data["steps"].as_array().expect("steps");
            let players = steps[1]["players"].as_array().expect("players");
            assert_eq!(steps[1]["ruleset"], "RULESET_JAC_BANSEIKI");
            assert_eq!(
                steps[1]["map_file"],
                "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map"
            );
            assert_eq!(steps[1]["map_size"], "MAPSIZE_HUGE");
            assert_eq!(players.len(), 22);
            assert_eq!(players[0]["civ"], format!("CIVILIZATION_JAC_{id}"));
            assert!(players[0]["human"].as_bool().expect("human"));
            assert_eq!(players.iter().filter(|p| p["human"] == true).count(), 1);
            assert!(steps.iter().all(|s| s["action"] != "autoplay"));
            let turns = steps
                .iter()
                .filter_map(|s| s["code"].as_str())
                .filter(|c| c.contains("UI.RequestAction(ActionTypes.ACTION_ENDTURN,"))
                .count();
            assert_eq!(data["finite_turns"], turns);
            assert!(turns <= 16, "{path}: unbounded turns {turns}");
            for pair in steps.windows(2).filter(|pair| {
                pair[0]["code"].as_str().is_some_and(|code| {
                    code.contains("UI.RequestAction(ActionTypes.ACTION_ENDTURN,")
                })
            }) {
                assert_eq!(pair[1]["action"], "wait_for", "{path}");
                assert_eq!(pair[1]["close_diplomacy"], true, "{path}");
            }
            for step in steps.iter().filter(|s| {
                s["code"]
                    .as_str()
                    .is_some_and(|code| code.contains("JAC_ShowOaths(true)"))
            }) {
                let state = step["state_name"].as_str().expect("oath UI state");
                assert!(
                    contexts.contains(state),
                    "{path}: oath UI state {state} is not registered by the MOD"
                );
            }
            assert!(steps.last().expect("acceptance step")["code"]
                .as_str()
                .expect("gate")
                .contains("accepted==true"));
        }
    }
    assert_eq!(expected.len(), 66);
    assert_eq!(houses.len(), 22);
    let manifest_paths: BTreeSet<_> = inventory["scenarios"]
        .as_array()
        .expect("manifest cases")
        .iter()
        .map(|c| c["scenario"].as_str().expect("scenario path").to_owned())
        .collect();
    assert_eq!(
        manifest_paths, expected,
        "manifest must retain all 66 catalogue keys"
    );
    for path in &expected {
        let data = load(path);
        let actual: BTreeSet<_> = data["steps"][1]["players"]
            .as_array()
            .expect("players")
            .iter()
            .map(|p| p["civ"].as_str().expect("civ").to_owned())
            .collect();
        assert_eq!(actual, houses, "{path}: another house was lost");
    }
    let capability = inventory["capability_scenario"]
        .as_str()
        .expect("first diagnosis");
    let data = load(capability);
    validate_scenario(&data).expect("capability driver schema");
    assert!(data["steps"]
        .as_array()
        .expect("capability steps")
        .iter()
        .all(|s| s.get("turns").is_none()));
    expected.insert(capability.to_owned());
    for (key, control) in [
        ("queue_calibration_scenario", 1),
        ("queue_quarter_control_scenario", 25),
        ("queue_unit_type_control_scenario", 1),
        ("queue_tag_era_control_scenario", 1),
    ] {
        let path = inventory[key]
            .as_str()
            .expect("independent calibration diagnosis");
        let data = load(path);
        validate_scenario(&data).unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(data["diagnostic_only"], true);
        assert_eq!(data["control_percent"], control);
        assert_eq!(data["finite_turns"], 38);
        assert_eq!(
            data["steps"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|s| s["code"].as_str())
                .filter(|c| c.contains("UI.RequestAction(ActionTypes.ACTION_ENDTURN,"))
                .count(),
            38
        );
        if key == "queue_unit_type_control_scenario" {
            assert_eq!(
                data["modifier_family"],
                "MODIFIER_PLAYER_CITIES_ADJUST_UNIT_PRODUCTION"
            );
        }
        if key == "queue_tag_era_control_scenario" {
            assert_eq!(
                data["modifier_family"],
                "MODIFIER_PLAYER_CITIES_ADJUST_UNIT_TAG_ERA_PRODUCTION"
            );
        }
        assert!(
            expected.insert(path.to_owned()),
            "duplicate diagnostic: {path}"
        );
    }
    let actual: BTreeSet<_> =
        std::fs::read_dir(root.join("tools/civ6-autotest/scenarios/oath-effects"))
            .expect("scenario directory")
            .map(|e| e.expect("directory entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .map(|p| {
                p.strip_prefix(&root)
                    .expect("owned scenario")
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
    assert_eq!(actual, expected, "missing or stale generated scenario");
    eprintln!(
        "validated all {} finite oath audit scenarios with the actual driver schema",
        actual.len()
    );
}
