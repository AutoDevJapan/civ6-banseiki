pub use crate::session::SessionType;
use crate::{Error, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::PathBuf};

pub const BANSEIKI_RULESET: &str = "RULESET_JAC_BANSEIKI";
pub const ACTIONS: [&str; 43] = [
    "start_native_order_capture",
    "set_native_order_phase",
    "stop_native_order_capture",
    "restart_game",
    "enable_mods",
    "configure",
    "start_game",
    "autoplay",
    "end_turn",
    "choose_dedication",
    "ui_lua",
    "carrier_checkpoint",
    "carrier_exchange",
    "screenshot",
    "save",
    "exit_to_menu",
    "load",
    "wait_for",
    "observe_captive_session",
    "inspect_native_spy_failures",
    "inspect_native_spy_research",
    "pause",
    "manual",
    "start_native_governor_trace",
    "stop_native_governor_trace",
    "start_native_can_ever_trace",
    "stop_native_can_ever_trace",
    "start_native_can_ever_calibration",
    "start_native_assign_calibration",
    "stop_native_assign_calibration",
    "stop_native_can_ever_calibration",
    "start_native_joined_admission",
    "stop_native_joined_admission",
    "start_native_production_admission",
    "stop_native_production_admission",
    "start_native_paid_runtime",
    "stop_native_paid_runtime_observation",
    "start_native_property_trace",
    "stop_native_property_trace",
    "start_native_promotion_call_trace",
    "stop_native_promotion_call_trace",
    "start_native_bootstrap",
    "stop_native_bootstrap",
];
pub fn lua_string(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}
pub fn matches(pattern: &str, value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| Regex::new(pattern).unwrap().is_match(s))
}
pub fn type_id(value: &Value) -> bool {
    matches(r"^[A-Z][A-Z0-9_]+$", value)
}
pub fn validate_test_mode(mode: &Value) -> Result<()> {
    if !mode.is_object()
        || mode["skipped_turns"].as_i64().is_none_or(|n| n < 0)
        || mode["continuous_performance_valid"] != false
    {
        return Err(Error::scenario("test_mode needs a nonnegative integer skipped_turns and continuous_performance_valid: false"));
    }
    Ok(())
}
pub fn safe_relative_path(value: &str) -> Result<PathBuf> {
    let normalized = value.replace('\\', "/");
    if normalized
        .split('/')
        .all(|part| part.is_empty() || part == ".")
        || normalized.starts_with('/')
        || normalized.as_bytes().get(1) == Some(&b':')
        || normalized.split('/').any(|s| s == "..")
    {
        return Err(Error::scenario(format!(
            "Path must be relative to output directory: {value:?}"
        )));
    }
    Ok(PathBuf::from(normalized))
}
pub fn validate_scenario(data: &Value) -> Result<()> {
    crate::bootstrap_scenario::validate(data)?;
    if !data.is_object() || !data["name"].is_string() {
        return Err(Error::scenario("Scenario needs a name"));
    }
    let session = SessionType::from_scenario(data)?;
    if let Some(nonce) = data.get("hotseat_bridge_nonce") {
        if session != SessionType::Hotseat
            || !nonce
                .as_str()
                .is_some_and(crate::hotseat_bridge::nonce_valid)
        {
            return Err(Error::scenario(
                "hotseat_bridge_nonce needs a 32-digit nonce and hotseat session",
            ));
        }
    }
    if let Some(mode) = data.get("test_mode") {
        validate_test_mode(mode)?;
    }
    let profile = data
        .get("judge_profile")
        .and_then(Value::as_str)
        .unwrap_or("generic");
    if data.get("judge_profile").is_some_and(|v| !v.is_string())
        || !["generic", "bundle0", "growth", "japan_map"].contains(&profile)
    {
        return Err(Error::scenario(
            "judge_profile must be bundle0, generic, growth or japan_map",
        ));
    }
    if profile == "growth" && data["expected_units"].as_i64().is_none_or(|n| n <= 0) {
        return Err(Error::scenario(
            "growth scenario needs a positive expected_units count",
        ));
    }
    if data
        .get("strict_lua_errors")
        .is_some_and(|v| !v.is_boolean())
    {
        return Err(Error::scenario("strict_lua_errors must be boolean"));
    }
    let steps = data["steps"]
        .as_array()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::scenario("Scenario needs a nonempty steps array"))?;
    let mut trace_open = false;
    let mut bridge_open = false;
    let mut property_open = false;
    let mut calibration_open = false;
    let mut assignment_open = false;
    let mut joined_open = false;
    let mut production_open = false;
    let mut promotion_open = false;
    let mut runtime_open = false;
    let mut runtime_used = false;
    let mut property_used = false;
    let mut promotion_used = false;
    let mut assignment_used = false;
    let mut trace_names = BTreeSet::new();
    let mut lua_captures = BTreeSet::new();
    let mut carrier_captures = BTreeSet::new();
    let mut order_open = false;
    let mut order_changes = 0;
    let mut order_phase = None;
    for (i, step) in steps.iter().enumerate() {
        let fail = |s: String| Error::scenario(format!("Step {}: {s}", i + 1));
        let action = step["action"]
            .as_str()
            .filter(|s| ACTIONS.contains(s))
            .ok_or_else(|| fail("unknown action".into()))?;
        if order_open
            && (action == "restart_game"
                || (action.starts_with("start_native_") && action != "start_native_order_capture"))
        {
            return Err(fail(
                "stop native order capture before restart or another observer".into(),
            ));
        }
        let bool_flags = |names: &[&str]| -> Result<()> {
            for name in names {
                if step.get(*name).is_some_and(|v| !v.is_boolean()) {
                    return Err(fail(format!("{name} must be boolean")));
                }
            }
            Ok(())
        };
        if let Some(flag) = step.get("preserve_diplomacy_session") {
            if !matches!(action, "ui_lua" | "save") || !flag.is_boolean() {
                return Err(fail(
                    "preserve_diplomacy_session needs a boolean ui_lua or save flag".into(),
                ));
            }
        }
        if let Some(flag) = step["condition"].get("preserve_diplomacy_session") {
            if action != "wait_for" || step["condition"]["type"] != "lua" || !flag.is_boolean() {
                return Err(fail(
                    "preserve_diplomacy_session needs a boolean Lua wait flag".into(),
                ));
            }
        }
        if bridge_open
            && matches!(
                action,
                "restart_game" | "start_game" | "exit_to_menu" | "load"
            )
        {
            return Err(fail(
                "stop native CanEver bridge before changing game lifecycle".into(),
            ));
        }
        if (property_open || joined_open || production_open)
            && matches!(action, "save" | "end_turn" | "autoplay")
        {
            return Err(fail(
                "stop native property observation before a save or turn change".into(),
            ));
        }
        if action == "restart_game" {
            if bridge_open || trace_open {
                return Err(fail(
                    "stop native observation before restarting the game".into(),
                ));
            }
            property_used = false;
            promotion_used = false;
            assignment_used = false;
            runtime_used = false;
        }
        if runtime_used && matches!(action, "load" | "start_game" | "exit_to_menu") {
            return Err(fail(
                "resident runtime load-generation hook is not verified; restart_game first".into(),
            ));
        }
        if runtime_used
            && matches!(
                action,
                "start_native_can_ever_trace"
                    | "start_native_can_ever_calibration"
                    | "start_native_assign_calibration"
                    | "start_native_joined_admission"
                    | "start_native_production_admission"
                    | "start_native_promotion_call_trace"
                    | "start_native_property_trace"
                    | "start_native_governor_trace"
            )
        {
            return Err(fail(
                "resident runtime owns native entries until restart_game".into(),
            ));
        }
        match action {
            "start_native_order_capture" => {
                crate::native_order::validate_options(step)?;
                if order_open || trace_open || bridge_open || runtime_used {
                    return Err(fail("another native observation is active".into()));
                }
                if !trace_names.insert(step["name"].as_str().unwrap().to_owned()) {
                    return Err(fail("native capture names must be unique".into()));
                }
                order_open = true;
                order_changes = 0;
                order_phase = step["phase"].as_str();
            }
            "set_native_order_phase" | "stop_native_order_capture" => {
                if !order_open {
                    return Err(fail("native order capture is not active".into()));
                }
                let phase_change = action == "set_native_order_phase";
                if step.as_object().unwrap().keys().any(|key| {
                    !(["action", "timeout"].contains(&key.as_str())
                        || phase_change && key == "phase")
                }) {
                    return Err(fail("unexpected native order option".into()));
                }
                if phase_change {
                    if !matches!(
                        step["phase"].as_str(),
                        Some("front" | "load" | "native-outer")
                    ) || order_changes >= 8
                        || order_phase == step["phase"].as_str()
                    {
                        return Err(fail(
                            "native order requires a known phase and at most eight changes".into(),
                        ));
                    }
                    order_changes += 1;
                    order_phase = step["phase"].as_str();
                } else {
                    order_open = false;
                }
            }
            "start_native_can_ever_trace"
            | "start_native_can_ever_calibration"
            | "start_native_assign_calibration"
            | "start_native_joined_admission"
            | "start_native_production_admission"
            | "start_native_paid_runtime"
            | "start_native_promotion_call_trace"
            | "start_native_property_trace" => {
                if bridge_open
                    || trace_open
                    || (action == "start_native_paid_runtime"
                        && (runtime_used || property_used || assignment_used || promotion_used))
                {
                    return Err(fail("native observation already active".into()));
                }
                let uses_property = matches!(
                    action,
                    "start_native_property_trace"
                        | "start_native_joined_admission"
                        | "start_native_production_admission"
                        | "start_native_paid_runtime"
                );
                if (uses_property && property_used)
                    || (action == "start_native_promotion_call_trace" && promotion_used)
                    || (action == "start_native_assign_calibration" && assignment_used)
                {
                    return Err(fail("native property, promotion and assignment observations are each single-use until restart_game".into()));
                }
                if !matches(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,79}$", &step["name"])
                    || !trace_names.insert(step["name"].as_str().unwrap().to_owned())
                    || !step["max_seconds"].as_u64().is_some_and(|n| {
                        (1..=u64::from(civ6_native_bridge_rs::MAX_ACTIVE_SECONDS)).contains(&n)
                    })
                    || step.as_object().unwrap().keys().any(|k| {
                        ![
                            "action",
                            "name",
                            "max_seconds",
                            "timeout",
                            "scope",
                            "owner",
                            "leader",
                        ]
                        .contains(&k.as_str())
                    })
                    || (action != "start_native_property_trace" && step.get("scope").is_some())
                    || ((step.get("owner").is_some() || step.get("leader").is_some())
                        && (action != "start_native_assign_calibration"
                            || step["owner"].as_u64().is_none_or(|n| n >= 64)
                            || !type_id(&step["leader"])))
                    || (action == "start_native_property_trace"
                        && step.get("scope").is_some_and(|v| {
                            v != "numeric_test_keys_853271_853272" && v != "ashina_admission_853304"
                        }))
                {
                    return Err(fail("invalid bounded native CanEver bridge start".into()));
                }
                bridge_open = true;
                property_open = action == "start_native_property_trace";
                calibration_open = action == "start_native_can_ever_calibration";
                assignment_open = action == "start_native_assign_calibration";
                joined_open = action == "start_native_joined_admission";
                production_open = action == "start_native_production_admission";
                runtime_open = action == "start_native_paid_runtime";
                runtime_used |= runtime_open;
                promotion_open = action == "start_native_promotion_call_trace";
                property_used |= uses_property;
                promotion_used |= promotion_open;
                assignment_used |= assignment_open;
            }
            "stop_native_can_ever_trace"
            | "stop_native_can_ever_calibration"
            | "stop_native_assign_calibration"
            | "stop_native_joined_admission"
            | "stop_native_production_admission"
            | "stop_native_paid_runtime_observation"
            | "stop_native_promotion_call_trace"
            | "stop_native_property_trace" => {
                if !bridge_open
                    || property_open != (action == "stop_native_property_trace")
                    || calibration_open != (action == "stop_native_can_ever_calibration")
                    || assignment_open != (action == "stop_native_assign_calibration")
                    || joined_open != (action == "stop_native_joined_admission")
                    || production_open != (action == "stop_native_production_admission")
                    || runtime_open != (action == "stop_native_paid_runtime_observation")
                    || promotion_open != (action == "stop_native_promotion_call_trace")
                    || step
                        .as_object()
                        .unwrap()
                        .keys()
                        .any(|k| !["action", "timeout"].contains(&k.as_str()))
                {
                    return Err(fail("invalid native CanEver bridge stop".into()));
                }
                bridge_open = false;
                property_open = false;
                calibration_open = false;
                assignment_open = false;
                joined_open = false;
                production_open = false;
                runtime_open = false;
                promotion_open = false;
            }
            "start_native_governor_trace" => {
                if trace_open || bridge_open {
                    return Err(fail("native governor trace is already active".into()));
                }
                if !matches(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,79}$", &step["name"]) {
                    return Err(fail("trace name must be a safe artifact basename".into()));
                }
                if !trace_names.insert(step["name"].as_str().unwrap().to_owned()) {
                    return Err(fail("trace name must be unique within the scenario".into()));
                }
                if !step["max_seconds"]
                    .as_u64()
                    .is_some_and(|n| (1..=300).contains(&n))
                {
                    return Err(fail("max_seconds must be 1..300".into()));
                }
                if step.get("mode").is_some()
                    && !matches!(
                        step["mode"].as_str(),
                        Some("governor" | "property" | "lifecycle")
                    )
                {
                    return Err(fail(
                        "trace mode must be governor, property or lifecycle".into(),
                    ));
                }
                if step["mode"] == "lifecycle"
                    && step["max_seconds"]
                        .as_u64()
                        .is_some_and(|n| n > civ6_native_probe_rs::MAX_LIFECYCLE_SECONDS)
                {
                    return Err(fail(
                        "lifecycle observation is limited to 20 seconds".into(),
                    ));
                }
                if step.as_object().unwrap().keys().any(|k| {
                    !["action", "name", "max_seconds", "mode", "timeout"].contains(&k.as_str())
                }) {
                    return Err(fail("unexpected trace start field".into()));
                }
                trace_open = true;
            }
            "stop_native_governor_trace" => {
                if !trace_open {
                    return Err(fail("native governor trace is not active".into()));
                }
                if step
                    .as_object()
                    .unwrap()
                    .keys()
                    .any(|k| !["action", "timeout"].contains(&k.as_str()))
                {
                    return Err(fail("unexpected trace stop field".into()));
                }
                trace_open = false;
            }
            "restart_game" => {
                if trace_open || bridge_open {
                    return Err(fail(
                        "stop native governor trace before restarting Civ6".into(),
                    ));
                }
                if step
                    .as_object()
                    .unwrap()
                    .keys()
                    .any(|k| k != "action" && k != "timeout")
                {
                    return Err(fail("restart_game only accepts timeout".into()));
                }
            }
            "enable_mods" => {
                bool_flags(&["exclusive_user_mods"])?;
                let valid = |v: &Value| {
                    matches(r"^[0-9a-fA-F]{8}-(?:[0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12}$", v)
                };
                let ids = step["ids"]
                    .as_array()
                    .filter(|a| !a.is_empty() && a.iter().all(valid))
                    .ok_or_else(|| fail("ids must be mod UUIDs".into()))?;
                if let Some(disabled) = step.get("disable_ids") {
                    let set: BTreeSet<_> = ids
                        .iter()
                        .map(|v| v.as_str().unwrap().to_lowercase())
                        .collect();
                    if !disabled.as_array().is_some_and(|a| {
                        a.iter().all(valid)
                            && a.iter()
                                .all(|v| !set.contains(&v.as_str().unwrap().to_lowercase()))
                    }) {
                        return Err(fail("invalid disable_ids".into()));
                    }
                }
            }
            "configure" => {
                bool_flags(&["build_setup"])?;
                for key in ["ruleset", "map_size", "speed"] {
                    if !type_id(&step[key]) {
                        return Err(fail(format!("invalid {key}")));
                    }
                }
                if step["seed"]
                    .as_i64()
                    .is_none_or(|v| i32::try_from(v).is_err())
                {
                    return Err(fail("seed must be a signed 32-bit integer".into()));
                }
                if step.get("map_file").is_some_and(|v| {
                    !matches!(v.as_str(), Some("Continents.lua" | "Pangaea.lua"))
                        && !matches(r"^\{[0-9a-fA-F-]{36}\}Maps/[A-Za-z0-9_-]+\.Civ6Map$", v)
                }) {
                    return Err(fail("invalid map_file".into()));
                }
                if step
                    .get("city_states")
                    .is_some_and(|v| !v.as_i64().is_some_and(|n| (0..=24).contains(&n)))
                {
                    return Err(fail("city_states must be an integer from 0 to 24".into()));
                }
                let players = step["players"]
                    .as_array()
                    .filter(|a| {
                        let humans = a.iter().filter(|p| p["human"] == true).count();
                        a.len() >= 2
                            && match session {
                                SessionType::SinglePlayer => humans == 1,
                                SessionType::Hotseat => humans >= 2,
                            }
                    })
                    .ok_or_else(|| {
                        fail(
                            match session {
                                SessionType::SinglePlayer => {
                                    "specify at least two players and exactly one human"
                                }
                                SessionType::Hotseat => "hotseat needs at least two human players",
                            }
                            .into(),
                        )
                    })?;
                if players.iter().any(|p| {
                    !p.is_object()
                        || !p["human"].is_boolean()
                        || !type_id(&p["leader"])
                        || !type_id(&p["civ"])
                }) {
                    return Err(fail("invalid player".into()));
                }
            }
            "autoplay" => {
                if session == SessionType::Hotseat {
                    return Err(fail(
                        "autoplay does not support hotseat; use end_turn".into(),
                    ));
                }
                if step["turns"].as_i64().is_none_or(|n| n <= 0) {
                    return Err(fail("turns must be positive".into()));
                }
                bool_flags(&[
                    "metrics",
                    "research_metrics",
                    "allow_human_elimination",
                    "fast_forward",
                ])?;
                if step
                    .get("metrics_group")
                    .is_some_and(|v| !type_id(v) || step["metrics"] != true)
                {
                    return Err(fail(
                        "metrics_group needs a type ID and metrics: true".into(),
                    ));
                }
                if step["fast_forward"] == true && !data["test_mode"].is_object() {
                    return Err(fail("fast_forward requires test mode".into()));
                }
            }
            "end_turn" => {
                for key in ["turns", "test_turns"] {
                    if step
                        .get(key)
                        .is_some_and(|v| v.as_i64().is_none_or(|n| n <= 0))
                    {
                        return Err(fail(format!("{key} must be positive")));
                    }
                }
            }
            "inspect_native_spy_failures"
                if step.as_object().is_none_or(|object| {
                    object
                        .keys()
                        .any(|key| !["action", "timeout"].contains(&key.as_str()))
                }) =>
            {
                return Err(fail(
                    "native spy failure inspection only accepts timeout".into(),
                ));
            }
            "choose_dedication"
                if !step["human"].as_i64().is_some_and(|n| (0..64).contains(&n)) =>
            {
                return Err(fail(
                    "choose_dedication human must be an integer in [0, 63]".into(),
                ));
            }
            "inspect_native_spy_research"
                if (step.as_object().is_none_or(|object| {
                    object
                        .keys()
                        .any(|key| !["action", "timeout", "phase"].contains(&key.as_str()))
                }) || !crate::spy_research::PHASES
                    .contains(&step["phase"].as_str().unwrap_or_default())) =>
            {
                return Err(fail(
                    "native spy research inspection needs a known phase and only accepts timeout"
                        .into(),
                ));
            }
            "carrier_checkpoint" | "carrier_exchange" => {
                bool_flags(&["direct_apply"])?;
                let token = step["token"]
                    .as_str()
                    .ok_or_else(|| fail("carrier checkpoint needs token".into()))?;
                let candidate = step["candidate"]
                    .as_str()
                    .ok_or_else(|| fail("carrier checkpoint needs candidate".into()))?;
                crate::carrier_sync::query(token, candidate).map_err(|e| fail(e.to_string()))?;
                if action == "carrier_checkpoint" {
                    safe_relative_path(
                        step["path"]
                            .as_str()
                            .ok_or_else(|| fail("carrier checkpoint needs path".into()))?,
                    )?;
                }
                for key in ["capture_as", "equals_capture"] {
                    if let Some(value) = step.get(key) {
                        if action == "carrier_exchange" {
                            return Err(fail(format!("carrier_exchange does not support {key}")));
                        }
                        if !type_id(value) {
                            return Err(fail(format!("invalid {key}")));
                        }
                    }
                }
                if action == "carrier_checkpoint" {
                    if let Some(key) = step["equals_capture"].as_str() {
                        if !carrier_captures.contains(key) {
                            return Err(fail(
                                "carrier equals_capture needs an earlier capture_as".into(),
                            ));
                        }
                    }
                    if let Some(key) = step["capture_as"].as_str() {
                        carrier_captures.insert(key.to_owned());
                    }
                }
            }
            "ui_lua" => {
                if step.get("lua_timeout").is_some_and(|v| {
                    !v.as_f64()
                        .is_some_and(|n| n.is_finite() && n > 0.0 && n <= 600.0)
                }) {
                    return Err(fail(
                        "lua_timeout must be finite and between 0 and 600 seconds".into(),
                    ));
                }
                if !step["state_name"].is_string()
                    || step["code"].as_str().is_none_or(|s| s.trim().is_empty())
                {
                    return Err(fail("ui_lua needs state_name and code".into()));
                }
                for key in ["capture_as", "equals_capture"] {
                    if step.get(key).is_some_and(|v| !type_id(v)) {
                        return Err(fail(format!("invalid {key}")));
                    }
                }
                if let Some(key) = step["equals_capture"].as_str() {
                    if !lua_captures.contains(key) {
                        return Err(fail(format!(
                            "equals_capture needs an earlier capture_as: {key}"
                        )));
                    }
                }
                if let Some(key) = step["capture_as"].as_str() {
                    lua_captures.insert(key);
                }
                crate::tuner::validate_lua_code(step["code"].as_str().unwrap())
                    .map_err(|e| fail(e.to_string()))?;
            }
            "screenshot" => {
                bool_flags(&["capture_only"])?;
                safe_relative_path(
                    step["path"]
                        .as_str()
                        .ok_or_else(|| fail("Screenshot path must be a string".into()))?,
                )?;
                if let Some(crop) = step.get("crop").filter(|v| !v.is_null()) {
                    if !crop.as_array().is_some_and(|a| {
                        a.len() == 4
                            && a.iter()
                                .all(|v| v.as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)))
                            && a[0].as_f64() < a[2].as_f64()
                            && a[1].as_f64() < a[3].as_f64()
                    }) {
                        return Err(fail(
                            "crop must be [left, top, right, bottom] fractions".into(),
                        ));
                    }
                }
            }
            "save" | "load" => {
                if step.get("save_sha256").is_some()
                    && (action != "load"
                        || !step["save_sha256"].as_str().is_some_and(|s| {
                            s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
                        }))
                {
                    return Err(fail(
                        "save_sha256 requires load and 64 hexadecimal characters".into(),
                    ));
                }
                if !matches(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,79}$", &step["name"]) {
                    return Err(fail("invalid save name".into()));
                }
                bool_flags(&["exact_name", "metrics", "checkpoint", "resume_once"])?;
                if step.get("resume_once").is_some()
                    && (action != "load" || step.get("save_sha256").is_none())
                {
                    return Err(fail("resume_once requires load with save_sha256".into()));
                }
                if step["checkpoint"] == true && step["exact_name"] != true {
                    return Err(fail("checkpoint requires exact_name: true".into()));
                }
            }
            "wait_for" => {
                let c = &step["condition"];
                match c["type"].as_str() {
                    Some("lua") => {
                        if !c["state_name"].is_string()
                            || c["code"].as_str().is_none_or(|s| s.trim().is_empty())
                        {
                            return Err(fail("Lua wait needs state_name and code".into()));
                        }
                        crate::tuner::validate_lua_code(c["code"].as_str().unwrap())
                            .map_err(|e| fail(e.to_string()))?;
                    }
                    Some("state") => {
                        if !c["name"].is_string() {
                            return Err(fail("state wait needs name".into()));
                        }
                    }
                    Some("log_line") => {
                        if !c["log"].as_str().is_some_and(|s| {
                            ["Lua.log", "Database.log", "Modding.log"].contains(&s)
                        }) || c["contains"].as_str().is_none_or(|s| s.is_empty())
                        {
                            return Err(fail("log wait needs log and contains".into()));
                        }
                    }
                    _ => {
                        return Err(fail(
                            "wait_for needs a lua, log_line or state condition".into(),
                        ))
                    }
                }
            }
            "observe_captive_session" => {
                let attacker = step["attacker"].as_u64();
                let captor = step["captor"].as_u64();
                if attacker.is_none_or(|n| n > 63)
                    || captor.is_none_or(|n| n > 63)
                    || attacker == captor
                {
                    return Err(fail(
                        "captive session observation needs distinct player IDs in 0..63".into(),
                    ));
                }
                if !step["lua_timeout"]
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && (5.0..=20.0).contains(&n))
                    || !step["timeout"]
                        .as_f64()
                        .is_some_and(|n| n.is_finite() && (30.0..=120.0).contains(&n))
                {
                    return Err(fail(
                        "captive session observation needs 5..20s Lua and 30..120s total timeouts"
                            .into(),
                    ));
                }
            }
            "pause"
                if !step["seconds"]
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && n > 0.0 && n <= 10.0) =>
            {
                return Err(fail("pause seconds must be in (0, 10]".into()));
            }
            "manual" => {
                if step["prompt"].as_str().is_none_or(|s| s.trim().is_empty()) {
                    return Err(fail("manual step needs prompt".into()));
                }
                if step.get("timeout").is_some() {
                    return Err(fail(
                        "manual waits for Enter and does not support timeout".into(),
                    ));
                }
            }
            _ => {}
        }
        if step
            .get("timeout")
            .is_some_and(|v| !v.as_f64().is_some_and(|n| n.is_finite() && n > 0.0))
        {
            return Err(fail("timeout must be a finite positive number".into()));
        }
    }
    if trace_open || bridge_open || order_open {
        return Err(Error::scenario("native governor trace has no stop step"));
    }
    Ok(())
}
pub fn fast_forward_scenario(data: &Value) -> Value {
    let mut data = data.clone();
    if let Some(steps) = data["steps"].as_array_mut() {
        for step in steps {
            if step["action"] == "autoplay" {
                if step["turns"].as_i64().unwrap_or(0) > 2 {
                    step["fast_forward"] = true.into();
                }
                if step["metrics"] == true {
                    step["metrics_group"] = format!(
                        "TEST_MODE_{}",
                        step["metrics_group"].as_str().unwrap_or("AUTOPLAY")
                    )
                    .into();
                }
            }
        }
    }
    data["test_mode"] = json!({"skipped_turns":0,"continuous_performance_valid":false});
    data
}
pub fn builds_setup(step: &Value) -> bool {
    step["build_setup"]
        .as_bool()
        .unwrap_or(step["ruleset"] == BANSEIKI_RULESET)
}
pub fn load_code(name: &str, session: SessionType) -> String {
    let (defaults, save_type) = match session {
        SessionType::SinglePlayer => ("", "SaveTypes.SINGLE_PLAYER"),
        SessionType::Hotseat => (
            "GameConfiguration.SetToDefaults(GameModeTypes.HOTSEAT)\n",
            "Network.GetGameConfigurationSaveType()",
        ),
    };
    format!("if not UI.IsInFrontEnd() then error('main menu required') end\n{defaults}local ok = Network.LoadGame({{Name={}, Location=SaveLocations.LOCAL_STORAGE, Type={save_type}, Directory=SaveDirectories.DEFAULT, IsAutosave=false, IsQuicksave=false}}, ServerType.{})\nif ok == false then error('Network.LoadGame returned false') end\nreturn true", lua_string(name), session.server_type())
}
pub fn configure_code(step: &Value) -> String {
    configure_code_for_session(step, SessionType::SinglePlayer)
}
pub fn configure_code_for_session(step: &Value, session: SessionType) -> String {
    let players = step["players"].as_array().unwrap();
    let mut lines = vec![
        "if not UI.IsInFrontEnd() then error('main menu required') end".into(),
        match session {
            SessionType::SinglePlayer => "GameConfiguration.SetToDefaults()",
            SessionType::Hotseat => "GameConfiguration.SetToDefaults(GameModeTypes.HOTSEAT)",
        }
        .into(),
        format!(
            "GameConfiguration.SetRuleSet({})",
            lua_string(step["ruleset"].as_str().unwrap())
        ),
        "GameConfiguration.UpdateEnabledMods()".into(),
    ];
    if let Some(file) = step["map_file"].as_str() {
        lines.push(format!(
            "MapConfiguration.SetValue('MAP_SCRIPT', {})",
            lua_string(file)
        ));
        lines.push(map_readback_code(file, "configured"));
    }
    lines.extend([format!("MapConfiguration.SetMapSize({})",lua_string(step["map_size"].as_str().unwrap())),format!("GameConfiguration.SetGameSpeedType({})",lua_string(step["speed"].as_str().unwrap())),format!("MapConfiguration.SetValue('RANDOM_SEED', {})",step["seed"]),format!("GameConfiguration.SetValue('GAME_SYNC_RANDOM_SEED', {})",step["seed"]),format!("MapConfiguration.SetMaxMajorPlayers({})",players.len()),format!("GameConfiguration.SetParticipatingPlayerCount({} + GameConfiguration.GetHiddenPlayerCount())",players.len())]);
    if let Some(count) = step.get("city_states") {
        lines.extend([format!("GameConfiguration.SetValue('CITY_STATE_COUNT', {count})"),format!("if GameConfiguration.GetValue('CITY_STATE_COUNT') ~= {count} then error('city-state count not applied') end")]);
    }
    for (i, p) in players.iter().enumerate() {
        let leader = lua_string(p["leader"].as_str().unwrap());
        let civ = lua_string(p["civ"].as_str().unwrap());
        let slot = if p["human"] == true {
            "SS_TAKEN"
        } else {
            "SS_COMPUTER"
        };
        lines.extend([format!("local p = PlayerConfigurations[{i}]"),format!("if not p then error('player slot {i} unavailable') end"),format!("p:SetSlotStatus(SlotStatus.{slot})"),format!("p:SetLeaderTypeName({leader})"),format!("p:SetCivilizationTypeName({civ})"),format!("if p:GetLeaderTypeName() ~= {leader} then error('leader mismatch in slot {i}') end"),format!("if p:GetCivilizationTypeName() ~= {civ} then error('civilization mismatch in slot {i}') end")]);
    }
    if session == SessionType::Hotseat {
        lines.push(session.guard_code().into());
    }
    lines.push("return true".into());
    lines.join("\n")
}
pub fn build_setup_code(step: &Value) -> String {
    let players = step["players"].as_array().unwrap();
    let mut lines = vec![
        "BuildHeadlessGameSetup()".to_string(),
        "g_GameParameters:FullRefresh()".into(),
        // MapSize_ValueChanged resets participant and city-state counts during
        // FullRefresh. Restore requested settings before checking the players.
        format!(
            "local participatingCount = {} + GameConfiguration.GetHiddenPlayerCount()",
            players.len()
        ),
        "GameConfiguration.SetParticipatingPlayerCount(participatingCount)".into(),
        "if GameConfiguration.GetParticipatingPlayerCount() ~= participatingCount then error('setup changed participating player count') end".into(),
    ];
    if let Some(file) = step["map_file"].as_str() {
        lines.push(map_readback_code(file, "setup-refreshed"));
    }
    if let Some(count) = step.get("city_states") {
        lines.extend([
            format!("GameConfiguration.SetValue('CITY_STATE_COUNT', {count})"),
            format!("if GameConfiguration.GetValue('CITY_STATE_COUNT') ~= {count} then error('setup changed city-state count') end"),
        ]);
    }
    for (i, p) in players.iter().enumerate() {
        let slot = if p["human"] == true {
            "SS_TAKEN"
        } else {
            "SS_COMPUTER"
        };
        lines.extend([format!("local p = PlayerConfigurations[{i}]"),format!("if not p or p:GetLeaderTypeName() ~= {} or p:GetCivilizationTypeName() ~= {} or p:GetSlotStatus() ~= SlotStatus.{slot} then error('setup changed player {i}') end",lua_string(p["leader"].as_str().unwrap()),lua_string(p["civ"].as_str().unwrap()))]);
    }
    lines.extend([
        "HideGameSetup()".into(),
        "return 'setup built; configured players survived'".into(),
    ]);
    lines.join("\n")
}
fn map_readback_code(file: &str, phase: &str) -> String {
    format!("do local actualMap=MapConfiguration.GetValue('MAP_SCRIPT')\nprint('JAC_MAP_READBACK phase={phase} type='..type(actualMap)..' value='..tostring(actualMap))\nassert(type(actualMap)=='string' and actualMap=={}, 'configured MAP_SCRIPT differs after {phase}') end",lua_string(file))
}
pub fn enable_mods_code(ids: &[String], disabled: &[String]) -> String {
    enable_mods_code_with_scope(ids, disabled, false)
}
pub fn enable_mods_code_with_scope(
    ids: &[String],
    disabled: &[String],
    exclusive_user_mods: bool,
) -> String {
    let disabled = isolated_disabled_mods(ids, disabled);
    format!("if not UI.IsInFrontEnd() then error('main menu required') end\nlocal enableIDs = {{{}}}\nlocal disableIDs = {{{}}}\nlocal exclusiveUserMods = {}\n{}",ids.iter().map(|s|lua_string(s)).collect::<Vec<_>>().join(", "),disabled.iter().map(|s|lua_string(s)).collect::<Vec<_>>().join(", "),exclusive_user_mods,ENABLE_MODS_BODY)
}

// A diagnostic that replaces production eligibility must never leak from the
// preceding game. Enforce this at the shared selection boundary, so generated
// and handwritten scenarios have the same isolation without snapshot edits.
pub const STATIC_PERSONA_GATE_ID: &str = "c678a216-58ab-4d40-a614-8f309d36ac86";
pub const ENEMY_PERSONA_GATE_ID: &str = "bba3a0d2-8f08-4715-9263-00a773e6ae86";
pub const TRAIT_GRANT_PROBE_ID: &str = "2b20f0d9-64cd-42b8-9841-74067419b81e";
pub const PROPERTY_OBSERVER_MOD_ID: &str = "d8532710-f853-4c53-8853-271000000001";
pub fn isolated_disabled_mods(ids: &[String], disabled: &[String]) -> Vec<String> {
    let mut result = disabled.to_vec();
    for gate in [
        STATIC_PERSONA_GATE_ID,
        ENEMY_PERSONA_GATE_ID,
        TRAIT_GRANT_PROBE_ID,
        PROPERTY_OBSERVER_MOD_ID,
    ] {
        if !ids.iter().any(|id| id.eq_ignore_ascii_case(gate))
            && !result.iter().any(|id| id.eq_ignore_ascii_case(gate))
        {
            result.push(gate.to_owned());
        }
    }
    result
}
const ENABLE_MODS_BODY: &str = r###"local keep, disabled = {}, {}
for _, id in ipairs(enableIDs) do keep[string.lower(id)] = true end
local handles, dependents, prerequisites, original = {}, {}, {}, {}
for _, installed in ipairs(Modding.GetInstalledMods()) do
    local info = Modding.GetModInfo(installed.Handle)
    local id = string.lower(info.Id)
    handles[id] = installed.Handle
    original[id] = info.Enabled
    local dependencies = Modding.GetModAssociations(installed.Handle)
    prerequisites[id] = {}
    for _, dependency in ipairs(dependencies or {}) do
      local target = string.lower(dependency.Id)
      table.insert(prerequisites[id], target)
      dependents[target] = dependents[target] or {}
      table.insert(dependents[target], id)
    end
end
if exclusiveUserMods then
  for _, requested in ipairs(enableIDs) do
    local id = string.lower(requested)
    for _, target in ipairs(prerequisites[id] or {}) do
      local handle = handles[target]
      if not handle then error('required mod is not installed: ' .. target) end
      if not Modding.GetModInfo(handle).Official and not keep[target] then
        error('scenario omits required user mod: ' .. target)
      end
    end
  end
  local extra = {}
  for id, handle in pairs(handles) do
    local info = Modding.GetModInfo(handle)
    if info.Enabled and not info.Official and not keep[id] then
      extra[#extra + 1] = id
    end
  end
  table.sort(extra)
  for _, id in ipairs(extra) do disableIDs[#disableIDs + 1] = id end
end
local order = {}
if #disableIDs > 0 then
  local visiting, visited = {}, {}
  local function visit(id)
    if visiting[id] then error('mod dependency cycle: ' .. id) end
    if visited[id] then return end
    if keep[id] then error('scenario enables a dependent mod of a disable target: ' .. id) end
    visiting[id] = true
    local children = dependents[id] or {}
    table.sort(children)
    for _, child in ipairs(children) do visit(child) end
    local handle = handles[id]
    if handle and Modding.GetModInfo(handle).Enabled then table.insert(order, id) end
    visiting[id], visited[id] = nil, true
  end
  -- Build the complete order before changing anything, including paths through
  -- disabled intermediates. An enable-list conflict must not disable other mods.
  for _, id in ipairs(disableIDs) do visit(string.lower(id)) end
  local positions = {}
  for i, id in ipairs(order) do positions[id] = i end
  for i, id in ipairs(order) do
    local reason, _, sources = Modding.CanDisableMod(handles[id])
    if reason == 'MissingDependencies' and sources and #sources > 0 then
      -- The engine still sees dependents as enabled during preflight. Only
      -- dependencies already scheduled BEFORE this target can be resolved.
      for _, source in ipairs(sources) do
        local position = positions[string.lower(source.Id)]
        if not position or position >= i then
          error('cannot disable mod: ' .. id .. ' (unscheduled dependent ' .. source.Id .. ')')
        end
      end
    elseif reason ~= 'OK' then
      error('cannot disable mod: ' .. id .. ' (' .. tostring(reason) .. ')')
    end
  end
end
for _, id in ipairs(enableIDs) do
  if not Modding.GetModHandle(id) then error('mod is not installed: ' .. id) end
end
local ok, failure = pcall(function()
    for _, id in ipairs(order) do
      local handle = handles[id]
      local reason = Modding.CanDisableMod(handle)
      if reason ~= 'OK' then error('cannot disable mod: ' .. id .. ' (' .. tostring(reason) .. ')') end
      Modding.DisableMod(handle)
      if Modding.GetModInfo(handle).Enabled then error('mod stayed enabled: ' .. id) end
      table.insert(disabled, id)
    end
for _, id in ipairs(enableIDs) do
  local handle = Modding.GetModHandle(id)
  if not handle then error('mod is not installed: ' .. id) end
  if not Modding.GetModInfo(handle).Enabled then
    local reason = Modding.CanEnableMod(handle, true)
    if reason ~= 'OK' then error('cannot enable mod: ' .. id .. ' (' .. tostring(reason) .. ')') end
    Modding.EnableMod(handle, true)
  end
end
for _, id in ipairs(enableIDs) do
  if not Modding.GetModInfo(Modding.GetModHandle(id)).Enabled then error('mod did not become enabled: ' .. id) end
end
for _, id in ipairs(disableIDs) do
  local handle = Modding.GetModHandle(id)
  if handle and Modding.GetModInfo(handle).Enabled then error('mod stayed enabled: ' .. id) end
end
if exclusiveUserMods then
  for id, handle in pairs(handles) do
    local info = Modding.GetModInfo(handle)
    if info.Enabled and not info.Official and not keep[id] then
      error('unexpected enabled user mod: ' .. id)
    end
  end
end
GameConfiguration.UpdateEnabledMods()
end)
if not ok then
  -- Restore the complete original selection, including prerequisites enabled
  -- implicitly by EnableMod(..., true), on failure in EITHER direction.
  local all, visited, restoreOrder = {}, {}, {}
  for id in pairs(handles) do table.insert(all, id) end
  table.sort(all)
  local function visit(id)
    if visited[id] then return end
    visited[id] = true
    local children = dependents[id] or {}
    table.sort(children)
    for _, child in ipairs(children) do visit(child) end
    if handles[id] then table.insert(restoreOrder, id) end
  end
  for _, id in ipairs(all) do visit(id) end
  local turnOff, changed = {}, false
  local function mark(id)
    if turnOff[id] then return end
    turnOff[id] = true
    for _, child in ipairs(dependents[id] or {}) do mark(child) end
  end
  for _, id in ipairs(all) do
    if Modding.GetModInfo(handles[id]).Enabled ~= original[id] then changed = true end
    if not original[id] and Modding.GetModInfo(handles[id]).Enabled then mark(id) end
  end
  local failures = {}
  for _, id in ipairs(restoreOrder) do
    if turnOff[id] and Modding.GetModInfo(handles[id]).Enabled then
      local restored, err = pcall(Modding.DisableMod, handles[id])
      if not restored or Modding.GetModInfo(handles[id]).Enabled then
        table.insert(failures, id .. ': ' .. tostring(err))
      end
    end
  end
  for i = #restoreOrder, 1, -1 do
    local id = restoreOrder[i]
    if original[id] and not Modding.GetModInfo(handles[id]).Enabled then
      -- False preserves an initially disabled intermediate prerequisite.
      local restored, err = pcall(Modding.EnableMod, handles[id], false)
      if not restored or not Modding.GetModInfo(handles[id]).Enabled then
        table.insert(failures, id .. ': ' .. tostring(err))
      end
    end
  end
  for _, id in ipairs(all) do
    if Modding.GetModInfo(handles[id]).Enabled ~= original[id] then
      table.insert(failures, id .. ': selection differs from original')
    end
  end
  if changed then
    local updated, err = pcall(GameConfiguration.UpdateEnabledMods)
    if not updated then table.insert(failures, 'configuration: ' .. tostring(err)) end
  end
  if #failures > 0 then error(tostring(failure) .. '; restoration failed: ' .. table.concat(failures, ', ')) end
  error(failure)
end
return table.concat(disabled, ', ')"###;

#[cfg(test)]
mod native_trace_tests {
    use super::*;
    #[test]
    fn native_trace_steps_are_bounded_and_paired() {
        let start = json!({"action":"start_native_governor_trace","name":"governor-before-join","max_seconds":300});
        let stop = json!({"action":"stop_native_governor_trace"});
        let scenario = |steps: Vec<Value>| json!({"name":"trace contract","steps":steps});
        assert!(validate_scenario(&scenario(vec![start.clone(), stop.clone()])).is_ok());
        assert!(validate_scenario(&scenario(vec![start.clone()])).is_err());
        assert!(validate_scenario(&scenario(vec![stop.clone()])).is_err());
        assert!(
            validate_scenario(&scenario(vec![start.clone(), start.clone(), stop.clone()])).is_err()
        );
        assert!(validate_scenario(&scenario(vec![
            start.clone(),
            json!({"action":"restart_game"}),
            stop.clone()
        ]))
        .is_err());
        for bad in [0, 301] {
            let mut step = start.clone();
            step["max_seconds"] = json!(bad);
            assert!(validate_scenario(&scenario(vec![step, stop.clone()])).is_err());
        }
        let mut property = start.clone();
        property["mode"] = json!("property");
        assert!(validate_scenario(&scenario(vec![property.clone(), stop.clone()])).is_ok());
        let mut lifecycle = start.clone();
        lifecycle["mode"] = json!("lifecycle");
        lifecycle["max_seconds"] = json!(20);
        assert!(validate_scenario(&scenario(vec![lifecycle.clone(), stop.clone()])).is_ok());
        lifecycle["max_seconds"] = json!(21);
        assert!(validate_scenario(&scenario(vec![lifecycle, stop.clone()])).is_err());
        property["mode"] = json!("unknown");
        assert!(validate_scenario(&scenario(vec![property, stop.clone()])).is_err());
        let mut bad = start;
        bad["name"] = json!("../outside");
        assert!(validate_scenario(&scenario(vec![bad, stop])).is_err());
        let again = json!({"action":"start_native_governor_trace","name":"governor-before-join","max_seconds":1});
        assert!(validate_scenario(&scenario(vec![
            json!({"action":"start_native_governor_trace","name":"governor-before-join","max_seconds":1}),
            json!({"action":"stop_native_governor_trace"}), again,
            json!({"action":"stop_native_governor_trace"})
        ])).is_err());
    }

    #[test]
    fn native_bridge_is_separate_bounded_and_stopped_before_game_lifecycle_changes() {
        let start =
            json!({"action":"start_native_can_ever_trace","name":"can-ever","max_seconds":300});
        let stop = json!({"action":"stop_native_can_ever_trace"});
        let scenario = |steps: Vec<Value>| json!({"name":"bridge contract","steps":steps});
        assert!(validate_scenario(&scenario(vec![start.clone(), stop.clone()])).is_ok());
        assert!(validate_scenario(&scenario(vec![start.clone()])).is_err());
        assert!(validate_scenario(&scenario(vec![stop.clone()])).is_err());
        for bad in [0, 301] {
            let mut step = start.clone();
            step["max_seconds"] = json!(bad);
            assert!(validate_scenario(&scenario(vec![step, stop.clone()])).is_err());
        }
        let mut extra = start.clone();
        extra["mode"] = json!("property");
        assert!(validate_scenario(&scenario(vec![extra, stop.clone()])).is_err());
        assert!(validate_scenario(&scenario(vec![
            start.clone(),
            json!({"action":"end_turn","turns":1,"timeout":600}),
            stop.clone()
        ]))
        .is_ok());
        for lifecycle in ["restart_game", "exit_to_menu", "load"] {
            assert!(validate_scenario(&scenario(vec![
                start.clone(),
                json!({"action":lifecycle}),
                stop.clone()
            ]))
            .is_err());
        }
        assert!(validate_scenario(&scenario(vec![
            start,
            json!({"action":"start_native_governor_trace","name":"conflict","max_seconds":20,"mode":"lifecycle"}),
            stop
        ])).is_err());
    }

    #[test]
    fn promotion_call_observer_is_explicit_bounded_and_cannot_mix_stops() {
        let start = json!({"action":"start_native_promotion_call_trace","name":"promote","max_seconds":120});
        let stop = json!({"action":"stop_native_promotion_call_trace"});
        let scenario = |steps: Vec<Value>| json!({"name":"promotion call","steps":steps});
        assert!(validate_scenario(&scenario(vec![start.clone(), stop.clone()])).is_ok());
        assert!(validate_scenario(&scenario(vec![
            start.clone(),
            json!({"action":"end_turn","turns":1,"timeout":600}),
            stop.clone()
        ]))
        .is_ok());
        for wrong in [
            "stop_native_can_ever_trace",
            "stop_native_property_trace",
            "stop_native_joined_admission",
        ] {
            assert!(
                validate_scenario(&scenario(vec![start.clone(), json!({"action":wrong})])).is_err()
            );
        }
        assert!(validate_scenario(&scenario(vec![start.clone()])).is_err());
        for bad in [0, 301] {
            let mut changed = start.clone();
            changed["max_seconds"] = json!(bad);
            assert!(validate_scenario(&scenario(vec![changed, stop.clone()])).is_err());
        }
        let mut scope = start.clone();
        scope["scope"] = json!("all_promotions");
        assert!(validate_scenario(&scenario(vec![scope, stop.clone()])).is_err());
        assert!(validate_scenario(&scenario(vec![
            start,
            json!({"action":"load","name":"other"}),
            stop
        ]))
        .is_err());
    }

    #[test]
    fn property_trace_requires_its_own_stop_and_excludes_save_turn_and_load() {
        let start =
            json!({"action":"start_native_property_trace","name":"values","max_seconds":120});
        let stop = json!({"action":"stop_native_property_trace"});
        let scenario = |steps: Vec<Value>| json!({"name":"property contract","steps":steps});
        assert!(validate_scenario(&scenario(vec![start.clone(),
            json!({"action":"ui_lua","state_name":"InGame","code":"return Game:GetProperty('JAC_TEST853_VALUE')"}),
            stop.clone()])).is_ok());
        assert!(validate_scenario(&scenario(vec![start.clone()])).is_err());
        assert!(validate_scenario(&scenario(vec![
            start.clone(),
            json!({"action":"stop_native_can_ever_trace"})
        ]))
        .is_err());
        for action in [
            "save",
            "load",
            "end_turn",
            "autoplay",
            "restart_game",
            "exit_to_menu",
        ] {
            assert!(
                validate_scenario(&scenario(vec![
                    start.clone(),
                    json!({"action":action}),
                    stop.clone()
                ]))
                .is_err(),
                "{action}"
            );
        }
        for duration in [0, 301] {
            let mut bad = start.clone();
            bad["max_seconds"] = json!(duration);
            assert!(validate_scenario(&scenario(vec![bad, stop.clone()])).is_err());
        }
        let mut scoped = start.clone();
        scoped["scope"] = json!("numeric_test_keys_853271_853272");
        assert!(validate_scenario(&scenario(vec![scoped.clone(), stop.clone()])).is_ok());
        for value in [
            json!("numeric_test_keys_853271"),
            json!("all"),
            json!(853271),
            json!(null),
        ] {
            let mut bad = scoped.clone();
            bad["scope"] = value;
            assert!(validate_scenario(&scenario(vec![bad, stop.clone()])).is_err());
        }
        scoped["action"] = json!("start_native_can_ever_trace");
        assert!(validate_scenario(&scenario(vec![
            scoped,
            json!({"action":"stop_native_can_ever_trace"})
        ]))
        .is_err());
    }
}
