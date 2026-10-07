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
            let request = json!({"schema":"todo880-bootstrap-controller-v2","pid":1,"created_filetime":1,"max_seconds":step["max_seconds"],"runtime_dll":step["runtime_dll"],"runtime_sha256":step["runtime_sha256"],"bootstrap_dll":step["bootstrap_dll"],"bootstrap_sha256":step["bootstrap_sha256"],"output":std::env::temp_dir().join(format!("{name}.jsonl"))});
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
}
