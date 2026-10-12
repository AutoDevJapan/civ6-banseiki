use crate::{Error, Result};
use serde_json::{json, Value};
pub fn validate(data: &Value) -> Result<()> {
    let Some(steps) = data["steps"].as_array() else {
        return Ok(());
    };
    let (mut fresh, mut world, mut open, mut used) = (false, false, false, false);
    let mut other_native = std::collections::BTreeSet::new();
    for step in steps {
        let action = step["action"].as_str().unwrap_or("");
        if action == "restart_game" {
            if open {
                return Err(Error::scenario("stop bootstrap before restart"));
            }
            fresh = true;
            world = false;
            used = false;
            other_native.clear();
        }
        if matches!(action, "load" | "start_game") {
            world = true;
        }
        if action == "start_native_bootstrap" {
            civ6_bootstrap_rs::protocol::Request::root_scenario(step).map_err(Error::scenario)?;
            if !fresh || world || used || !other_native.is_empty() {
                return Err(Error::scenario(
                    "bootstrap needs fresh restart before any world start/load; one Prepare only",
                ));
            }
            let name = step["name"]
                .as_str()
                .filter(|s| {
                    !s.is_empty()
                        && s.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                })
                .ok_or_else(|| Error::scenario("bootstrap name invalid"))?;
            let request = civ6_bootstrap_rs::protocol::scenario_request(
                step,
                1,
                1,
                &std::env::temp_dir().join(format!("{name}.jsonl")),
            );
            civ6_bootstrap_rs::protocol::Request::parse(&request).map_err(Error::scenario)?;
            open = true;
            used = true;
        } else if action == "stop_native_bootstrap" {
            if !open {
                return Err(Error::scenario("bootstrap stop without start"));
            }
            open = false;
        } else if action.starts_with("start_native_") {
            if used {
                return Err(Error::scenario(
                    "resident bootstrap cannot share other native modes in one process",
                ));
            }
            other_native.insert(action.to_string());
        } else if action.starts_with("stop_native_") {
            other_native.remove(&action.replacen("stop_", "start_", 1));
        }
    }
    if open {
        return Err(Error::scenario("bootstrap has no stop action"));
    }
    Ok(())
}
pub fn scene(
    prefix: &Value,
    runtime: &str,
    runtime_sha: &str,
    bootstrap: &str,
    bootstrap_sha: &str,
    save: &str,
) -> Result<Value> {
    if save.is_empty() {
        return Err(Error::scenario("physical save name required"));
    }
    let source = prefix["steps"]
        .as_array()
        .ok_or_else(|| Error::scenario("prefix scene steps missing"))?;
    let mut steps: Vec<Value> = source
        .iter()
        .take_while(|s| {
            matches!(
                s["action"].as_str(),
                Some("enable_mods" | "restart_game" | "configure")
            )
        })
        .cloned()
        .collect();
    if !steps.iter().any(|s| s["action"] == "restart_game") {
        return Err(Error::scenario("prefix needs an explicit fresh restart"));
    }
    steps.extend([
        json!({"action":"start_native_bootstrap","name":"resident-cold","max_seconds":300,"runtime_dll":runtime,"runtime_sha256":runtime_sha,"bootstrap_dll":bootstrap,"bootstrap_sha256":bootstrap_sha,"timeout":40}),
        json!({"action":"load","name":save,"exact_name":true,"timeout":120}),
        json!({"action":"ui_lua","state_name":"InGame","code":"assert(Players[0]); return 'BOOTSTRAP_FIRST_WORLD turn='..tostring(Game.GetCurrentGameTurn())","timeout":20}),
        json!({"action":"exit_to_menu","timeout":60}),
        json!({"action":"load","name":save,"exact_name":true,"timeout":120}),
        json!({"action":"ui_lua","state_name":"InGame","code":"assert(Players[0]); return 'BOOTSTRAP_RELOADED_WORLD turn='..tostring(Game.GetCurrentGameTurn())","timeout":20}),
        json!({"action":"stop_native_bootstrap","timeout":45}),
    ]);
    let mut output = prefix.clone();
    output["name"] = json!("ToDo880 cold-menu resident bootstrap and same-PID reload");
    output["steps"] = json!(steps);
    crate::scenario::validate_scenario(&output)?;
    Ok(output)
}

/// Add only resident setup/telemetry stop. Every original operation, assertion,
/// save/load and judge setting survives unchanged and in its original order.
/// This is an explicit bootstrap trial, not proof of a retail autostart path.
pub fn original_scene(
    original: &Value,
    runtime: &str,
    runtime_sha: &str,
    bootstrap: &str,
    bootstrap_sha: &str,
) -> Result<Value> {
    crate::scenario::validate_scenario(original)?;
    let source = original["steps"]
        .as_array()
        .ok_or_else(|| Error::scenario("original scene steps missing"))?;
    let world = source
        .iter()
        .position(|step| matches!(step["action"].as_str(), Some("start_game" | "load")))
        .ok_or_else(|| Error::scenario("original scene has no world start/load"))?;
    if source.iter().any(|step| {
        let action = step["action"].as_str().unwrap_or("");
        action.starts_with("start_native_") || action.starts_with("stop_native_")
    }) {
        return Err(Error::scenario("original scene already has a native owner"));
    }
    let restarts: Vec<_> = source
        .iter()
        .enumerate()
        .filter(|(_, step)| step["action"] == "restart_game")
        .map(|(index, _)| index)
        .collect();
    if restarts.len() > 1 || restarts.first().is_some_and(|index| *index >= world) {
        return Err(Error::scenario(
            "original scene changes process after world start",
        ));
    }
    let mut steps = source[..world].to_vec();
    if restarts.is_empty() {
        let before_setup = steps
            .iter()
            .position(|step| step["action"] != "enable_mods")
            .unwrap_or(steps.len());
        steps.insert(before_setup, json!({"action":"restart_game"}));
    }
    steps.push(
        json!({"action":"start_native_bootstrap","name":"original-resident",
        "max_seconds":300,"controller_seconds":1500,"runtime_dll":runtime,
        "runtime_sha256":runtime_sha,"bootstrap_dll":bootstrap,
        "bootstrap_sha256":bootstrap_sha,"timeout":40}),
    );
    steps.extend_from_slice(&source[world..]);
    steps.push(json!({"action":"stop_native_bootstrap","timeout":45}));
    let mut output = original.clone();
    output["steps"] = json!(steps);
    crate::scenario::validate_scenario(&output)?;
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn start() -> Value {
        let p = std::env::temp_dir();
        json!({"action":"start_native_bootstrap","name":"bootstrap","max_seconds":240,"runtime_dll":p.join("runtime.dll"),"runtime_sha256":"a".repeat(64),"bootstrap_dll":p.join("bootstrap.dll"),"bootstrap_sha256":"b".repeat(64)})
    }
    #[test]
    fn bootstrap_lifecycle_is_explicit_and_does_not_open_other_native_modes() {
        let restart = json!({"action":"restart_game"});
        let load = json!({"action":"load","name":"same-physical-save"});
        let stop = json!({"action":"stop_native_bootstrap"});
        assert!(validate(
            &json!({"steps":[restart,start(),load,{"action":"exit_to_menu"},load,stop]})
        )
        .is_ok());
        for steps in [
            vec![start(), stop.clone()],
            vec![restart.clone(), load.clone(), start(), stop.clone()],
            vec![restart.clone(), start(), restart.clone(), stop.clone()],
            vec![
                restart,
                start(),
                stop.clone(),
                json!({"action":"start_native_production_admission"}),
            ],
        ] {
            assert!(validate(&json!({"steps":steps})).is_err());
        }
        let mut missing = start();
        missing.as_object_mut().unwrap().remove("bootstrap_sha256");
        assert!(validate(&json!({"steps":[{"action":"restart_game"},missing,stop]})).is_err());
    }

    #[test]
    fn resident_trial_preserves_every_original_persona_acceptance_value() {
        let original: Value = serde_json::from_str(include_str!(
            "../../civ6-autotest/scenarios/leader-persona-15km.json"
        ))
        .unwrap();
        let p = std::env::temp_dir();
        let output = original_scene(
            &original,
            p.join("runtime.dll").to_str().unwrap(),
            &"a".repeat(64),
            p.join("bootstrap.dll").to_str().unwrap(),
            &"b".repeat(64),
        )
        .unwrap();
        let steps = output["steps"].as_array().unwrap();
        let kept: Vec<_> = steps
            .iter()
            .filter(|step| {
                !matches!(
                    step["action"].as_str(),
                    Some("restart_game" | "start_native_bootstrap" | "stop_native_bootstrap")
                )
            })
            .cloned()
            .collect();
        assert_eq!(kept, *original["steps"].as_array().unwrap());
        assert_eq!(steps.len(), kept.len() + 3);
        let mut restored = output;
        restored["steps"] = json!(kept);
        assert_eq!(restored, original, "judge and all other fields survive");
    }

    #[test]
    fn resident_trial_keeps_a_fresh_restart_and_refuses_conflicting_owners() {
        let p = std::env::temp_dir();
        let build = |input: &Value| {
            original_scene(
                input,
                p.join("runtime.dll").to_str().unwrap(),
                &"a".repeat(64),
                p.join("bootstrap.dll").to_str().unwrap(),
                &"b".repeat(64),
            )
        };
        let original = json!({"name":"fresh","judge_profile":"generic","steps":[
            {"action":"restart_game"},{"action":"start_game"}]});
        let output = build(&original).unwrap();
        assert_eq!(output["steps"].as_array().unwrap().len(), 4);
        assert_eq!(output["steps"][0], original["steps"][0]);
        assert_eq!(output["steps"][2], original["steps"][1]);
        for steps in [
            json!([{ "action":"start_game" },{ "action":"restart_game" }]),
            json!([{ "action":"restart_game" },{ "action":"restart_game" },{ "action":"start_game" }]),
            json!([{ "action":"restart_game" },{ "action":"start_native_order_capture" },{ "action":"start_game" }]),
            json!([{ "action":"restart_game" }]),
        ] {
            assert!(build(&json!({"name":"conflict","steps":steps})).is_err());
        }
    }
}
