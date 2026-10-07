//! Machine-readable failure boundaries. A failed run never loses its original error.
use crate::Error;
use serde_json::{json, Value};

pub fn failure_boundary(step: Option<&Value>, index: Option<usize>, error: &Error) -> Value {
    let action = step
        .and_then(|s| s["action"].as_str())
        .unwrap_or("preflight");
    let stage = match action {
        "restart_game" => "launch",
        "enable_mods" => "mod_selection",
        "configure" => "configuration",
        "start_game" => "game_creation",
        "load" => "save_loading",
        "save" => "saving",
        "exit_to_menu" => "menu_transition",
        "end_turn" | "autoplay" => "turn",
        "ui_lua" | "wait_for" => "scenario_assertion",
        _ => action,
    };
    let mut boundary = json!({"stage":stage,"action":action,"step":index.map(|i| i + 1),
        "error_kind":format!("{:?}",error.kind),"message":error.message,
        "automatic_replay":false});
    if let Some(context) = step.and_then(|s| s.get("repeat_until")) {
        boundary["repeat_until"] = context.clone();
    }
    boundary
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_error_and_one_based_step_survive_without_inventing_a_product_cause() {
        let v = failure_boundary(
            Some(&json!({"action":"load"})),
            Some(4),
            &Error::unknown("load sent; reply lost"),
        );
        assert_eq!(v["stage"], "save_loading");
        assert_eq!(v["step"], 5);
        assert_eq!(v["error_kind"], "LuaResponseUnknown");
        assert_eq!(v["message"], "load sent; reply lost");
        assert_eq!(v["automatic_replay"], false);
    }
}
