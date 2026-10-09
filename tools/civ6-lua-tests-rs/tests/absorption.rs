//! All 120 absorption methods from the frozen 8ff592e baseline.
//! The legacy Lua bodies live verbatim in absorption-literals.lua. Only their
//! Python scaffolding is translated; the current worktree's MOD is executed.
use civ6_lua_tests_rs::strict::{self, ChunkExt, Data, FromLua, TableExt};
use civ6_lua_tests_rs::{json_to_lua, new_lua, query_json, read_repo, repo_root};
use mlua::{Function, Lua, MultiValue, Table, Value};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

#[path = "support/inventory.rs"]
mod inventory;

const MOD: &str = "poc/JAC_PoC_Absorption";

fn failure(message: impl Into<String>) -> mlua::Error {
    mlua::Error::RuntimeError(message.into())
}

fn argument(args: &MultiValue, index: usize) -> mlua::Result<Value> {
    args.get(index)
        .cloned()
        .ok_or_else(|| failure(format!("missing assertion argument {index}")))
}

fn tuple(lua: &Lua, args: MultiValue) -> mlua::Result<Value> {
    let result = lua.create_table()?;
    result.set("__python_tuple", true)?;
    result.set("length", args.len())?;
    for (i, value) in args.into_iter().enumerate() {
        let entry = lua.create_table()?;
        entry.set("value", value)?;
        result.set(i + 1, entry)?;
    }
    Ok(Value::Table(result))
}

fn python_result(lua: &Lua, mut args: MultiValue) -> mlua::Result<Value> {
    match args.len() {
        0 => Ok(Value::Nil),
        1 => Ok(args.pop_front().unwrap()),
        _ => tuple(lua, args),
    }
}

fn capture(value: Value) -> mlua::Result<Data> {
    if let Value::Table(t) = &value {
        if matches!(t.get::<Value>("__python_tuple")?, Value::Boolean(true)) {
            let n = t.strict_get::<i64>("length")?;
            let entries = t
                .pairs::<Value, Value>()
                .collect::<mlua::Result<Vec<_>>>()?;
            if n < 0 || entries.len() != n as usize + 2 {
                return Err(failure("invalid Python tuple key set"));
            }
            let mut result = Vec::new();
            for i in 1..=n {
                let entry = t.strict_get::<Table>(i)?;
                for pair in entry.pairs::<Value, Value>() {
                    let (key, _) = pair?;
                    if String::from_value(key)? != "value" {
                        return Err(failure("invalid tuple item"));
                    }
                }
                result.push(capture(entry.get::<Value>("value")?)?);
            }
            return Ok(Data::Results(result));
        }
    }
    Data::from_value(value)
}

fn evaluated(function: Function) -> mlua::Result<Data> {
    let mut values = function.call::<MultiValue>(())?;
    match values.len() {
        0 => Ok(Data::Nil),
        1 => capture(values.pop_front().unwrap()),
        _ => Ok(Data::Results(
            values
                .into_iter()
                .map(capture)
                .collect::<mlua::Result<_>>()?,
        )),
    }
}

fn python_subtract(left: Value, right: Value) -> mlua::Result<Value> {
    let numeric = |value: Value| match value {
        Value::Boolean(b) => Ok(Value::Integer(i64::from(b))),
        value @ (Value::Integer(_) | Value::Number(_)) => Ok(value),
        other => Err(failure(format!(
            "Python subtraction requires a number, got {other:?}"
        ))),
    };
    match (numeric(left)?, numeric(right)?) {
        (Value::Integer(a), Value::Integer(b)) => a
            .checked_sub(b)
            .map(Value::Integer)
            .ok_or_else(|| failure("Python integer subtraction overflow")),
        (a, b) => {
            let number = |value| match value {
                Value::Integer(n) => n as f64,
                Value::Number(n) => n,
                _ => unreachable!(),
            };
            Ok(Value::Number(number(a) - number(b)))
        }
    }
}

fn json_with_null(lua: &Lua, value: &Json) -> mlua::Result<Value> {
    // Keep JSON null present without validating fields that a case never reads.
    match value {
        Json::Null => Ok(Value::LightUserData(mlua::LightUserData(
            std::ptr::null_mut(),
        ))),
        Json::Array(values) => {
            let table = lua.create_table()?;
            for (i, value) in values.iter().enumerate() {
                table.set(i + 1, json_with_null(lua, value)?)?;
            }
            Ok(Value::Table(table))
        }
        Json::Object(values) => {
            let table = lua.create_table()?;
            for (key, value) in values {
                table.set(key.as_str(), json_with_null(lua, value)?)?;
            }
            Ok(Value::Table(table))
        }
        _ => Ok(json_to_lua(lua, value)),
    }
}

fn boundaries(lua: &Lua) {
    let g = lua.globals();
    for (name, equal) in [("EQ", true), ("NE", false)] {
        g.set(
            name,
            lua.create_function(move |_, args: MultiValue| {
                let actual = evaluated(Function::from_value(argument(&args, 0)?)?)?;
                let expected = evaluated(Function::from_value(argument(&args, 1)?)?)?;
                if actual.python_eq(&expected) != equal {
                    return Err(failure(format!(
                        "{name}: actual {actual:?}, expected {expected:?}"
                    )));
                }
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
    }
    g.set(
        "TRUTH",
        lua.create_function(|_, args: MultiValue| {
            let function = Function::from_value(argument(&args, 0)?)?;
            let expected = bool::from_value(argument(&args, 1)?)?;
            let values = function.call::<MultiValue>(())?;
            if strict::truthy_results(values.clone()) != expected {
                return Err(failure(format!(
                    "Python truth: {values:?}, expected {expected}"
                )));
            }
            Ok(())
        })
        .unwrap(),
    )
    .unwrap();
    g.set(
        "BOOL",
        lua.create_function(|_, args: MultiValue| {
            if args.len() != 1 {
                return Err(failure("bool requires exactly one operand"));
            }
            Ok(strict::truthy(&args[0]))
        })
        .unwrap(),
    )
    .unwrap();
    g.set(
        "IDENTITY",
        lua.create_function(|_, args: MultiValue| {
            let actual = evaluated(Function::from_value(argument(&args, 0)?)?)?;
            let expected = bool::from_value(argument(&args, 1)?)?;
            if !matches!(actual, Data::Boolean(b) if b == expected) {
                return Err(failure(format!(
                    "boolean identity: {actual:?}, expected {expected}"
                )));
            }
            Ok(())
        })
        .unwrap(),
    )
    .unwrap();
    g.set(
        "IS_NIL",
        lua.create_function(|_, args: MultiValue| {
            let function = Function::from_value(argument(&args, 0)?)?;
            let values = function.call::<MultiValue>(())?;
            let expected = bool::from_value(argument(&args, 1)?)?;
            let is_none =
                values.is_empty() || (values.len() == 1 && matches!(values[0], Value::Nil));
            if is_none != expected {
                return Err(failure(format!("None identity: {values:?}")));
            }
            Ok(())
        })
        .unwrap(),
    )
    .unwrap();
    g.set(
        "GREATER",
        lua.create_function(|_, args: MultiValue| {
            let actual = evaluated(Function::from_value(argument(&args, 0)?)?)?;
            let expected = evaluated(Function::from_value(argument(&args, 1)?)?)?;
            let a = actual
                .python_f64()
                .ok_or_else(|| failure("non-numeric comparison operand"))?;
            let b = expected
                .python_f64()
                .ok_or_else(|| failure("non-numeric comparison operand"))?;
            if a.partial_cmp(&b) != Some(std::cmp::Ordering::Greater) {
                return Err(failure(format!("expected {actual:?} > {expected:?}")));
            }
            Ok(())
        })
        .unwrap(),
    )
    .unwrap();
    g.set(
        "ERROR",
        lua.create_function(|_, args: MultiValue| {
            let function = Function::from_value(argument(&args, 0)?)?;
            // Only the raw Lua call belongs to assertRaises. Decoding failures
            // must never count as an exception thrown by the MOD.
            let error = function
                .call::<MultiValue>(())
                .err()
                .ok_or_else(|| failure("expected raw Lua error"))?;
            if let Some(pattern) = args.get(1) {
                let pattern = String::from_value(pattern.clone())?;
                if !Regex::new(&pattern)
                    .map_err(|e| failure(e.to_string()))?
                    .is_match(&error.to_string())
                {
                    return Err(failure(format!("expected {pattern:?}, got {error}")));
                }
            }
            Ok(())
        })
        .unwrap(),
    )
    .unwrap();
    g.set(
        "TRY",
        lua.create_function(|lua, args: MultiValue| {
            let function = Function::from_value(argument(&args, 0)?)?;
            match function.call::<MultiValue>(()) {
                Ok(values) => Ok(MultiValue::from_vec(vec![
                    Value::Boolean(true),
                    values.into_iter().next().unwrap_or(Value::Nil),
                ])),
                Err(error) => Ok(MultiValue::from_vec(vec![
                    Value::Boolean(false),
                    Value::String(lua.create_string(error.to_string())?),
                ])),
            }
        })
        .unwrap(),
    )
    .unwrap();
    g.set(
        "PY_SUB",
        lua.create_function(|_, args: MultiValue| {
            if args.len() != 2 {
                return Err(failure("Python subtraction requires two operands"));
            }
            python_subtract(argument(&args, 0)?, argument(&args, 1)?)
        })
        .unwrap(),
    )
    .unwrap();
    g.set(
        "PY_LEN",
        lua.create_function(|_, args: MultiValue| match argument(&args, 0)? {
            // Lupa's len(LuaTable) calls Lua's length operation, not pairs().
            Value::Table(table) => Ok(table.raw_len()),
            Value::String(string) => Ok(string.to_str()?.chars().count()),
            other => Err(failure(format!(
                "len requires table or string, got {other:?}"
            ))),
        })
        .unwrap(),
    )
    .unwrap();
    g.set(
        "COUNT",
        lua.create_function(|_, args: MultiValue| match argument(&args, 0)? {
            Value::Table(t) => Ok(t
                .pairs::<Value, Value>()
                .collect::<mlua::Result<Vec<_>>>()?
                .len()),
            Value::String(s) => Ok(s.to_str()?.chars().count()),
            other => Err(failure(format!(
                "len requires table or string, got {other:?}"
            ))),
        })
        .unwrap(),
    )
    .unwrap();
    g.set("TUPLE", lua.create_function(tuple).unwrap()).unwrap();
    g.set(
        "RAW_CALL",
        lua.create_function(|lua, mut args: MultiValue| {
            let function = Function::from_value(
                args.pop_front()
                    .ok_or_else(|| failure("missing function"))?,
            )?;
            let values = function.call::<MultiValue>(args)?;
            python_result(lua, values)
        })
        .unwrap(),
    )
    .unwrap();
    g.set(
        "RUN",
        lua.create_function(|lua, args: MultiValue| {
            let source = String::from_value(argument(&args, 0)?)?;
            let values = lua.load(&source).eval::<MultiValue>()?;
            python_result(lua, values)
        })
        .unwrap(),
    )
    .unwrap();
    g.set(
        "READ",
        lua.create_function(|_, args: MultiValue| {
            let path = String::from_value(argument(&args, 0)?)?;
            Ok(read_repo(path))
        })
        .unwrap(),
    )
    .unwrap();
    g.set(
        "JSON",
        lua.create_function(|lua, args: MultiValue| {
            let source = String::from_value(argument(&args, 0)?)?;
            let json: Json = serde_json::from_str(&source).map_err(|e| failure(e.to_string()))?;
            json_with_null(lua, &json)
        })
        .unwrap(),
    )
    .unwrap();
    for name in ["JSON_GET", "JSON_REQUIRE", "JSON_TEXT"] {
        g.set(
            name,
            lua.create_function(move |lua, args: MultiValue| {
                let table = Table::from_value(argument(&args, 0)?)?;
                let key = String::from_value(argument(&args, 1)?)?;
                let mut value = table.get::<Value>(key.as_str())?;
                if matches!(value, Value::Nil) {
                    value = match name {
                        "JSON_REQUIRE" => return Err(failure(format!("missing JSON key {key}"))),
                        "JSON_TEXT" => Value::String(lua.create_string("")?),
                        _ => args.get(2).cloned().unwrap_or(Value::Nil),
                    };
                }
                if name == "JSON_TEXT" {
                    String::from_value(value.clone())?;
                }
                Ok(value)
            })
            .unwrap(),
        )
        .unwrap();
    }
}

fn outcomes() -> Vec<Json> {
    static ROWS: LazyLock<Vec<Json>> = LazyLock::new(|| {
        let db = civ6_lua_tests_rs::official::official_db();
        db.execute_batch(&read_repo(format!("{MOD}/Data/JAC_Absorption.sql")))
            .unwrap();
        query_json(
            &db,
            "SELECT Kind,Outcome,Destination FROM JAC_AbsorptionOutcomes",
        )
    });
    ROWS.clone()
}

struct Harness {
    lua: Lua,
}
impl Harness {
    fn new() -> Self {
        Self::create(true)
    }

    fn create(gameplay: bool) -> Self {
        let lua = new_lua();
        boundaries(&lua);
        let strings: Table = lua
            .load(include_str!("../fixtures/absorption-literals.lua"))
            .strict_eval()
            .unwrap();
        lua.globals().set("STRINGS", strings.clone()).unwrap();
        if gameplay {
            let module = strings.strict_get::<Table>("module").unwrap();
            let engine = module.strict_get::<String>(3).unwrap();
            let rows = json_to_lua(&lua, &Json::Array(outcomes()));
            lua.globals().set("OUTCOMES", rows.clone()).unwrap();
            lua.load(&engine).call::<MultiValue>(rows).unwrap();
            lua.load("PlayerManager.GetAliveIDs=function() local ids={} for _,player in ipairs(PlayerManager.GetAlive()) do ids[#ids+1]=player:GetID() end return ids end")
                .exec()
                .unwrap();
        }
        // Frozen historical fragments expect the old game-core table name.
        // This test-only alias is installed after loading the real module;
        // native boundary tests inspect the unmodified product exports.
        lua.load(include_str!("../fixtures/absorption-support.lua").replace(
            "  A=proxy(ExposedMembers.JAC_Absorption)",
            "  ExposedMembers.JAC_Absorption=JAC_AbsorptionGameplay\n  A=proxy(JAC_AbsorptionGameplay)",
        ))
            .exec()
            .unwrap();
        if gameplay {
            lua.load("RELOAD()").exec().unwrap();
        }
        let cases: Table = lua
            .load(include_str!("../fixtures/absorption-cases.lua"))
            .strict_eval()
            .unwrap();
        lua.globals().set("CASES", cases).unwrap();
        lua.load(include_str!("../fixtures/absorption-manual.lua"))
            .exec()
            .unwrap();
        Self { lua }
    }

    fn execute_case(&self, name: &str) {
        let cases = self.lua.globals().strict_get::<Table>("CASES").unwrap();
        let case = cases.strict_get::<Function>(name).unwrap();
        case.call::<MultiValue>(())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
    }

    fn variant(&self, name: &str, args: Vec<Value>) {
        let h = Self::create(
            name != "test_ai_purchase_receipt_rejects_interleaved_faith_or_unit_events",
        );
        for (i, arg) in args.into_iter().enumerate() {
            // Variant values are numbers only; string variants are read from
            // the verbatim literals in each new VM.
            h.lua.globals().set(format!("ARG{}", i + 1), arg).unwrap();
        }
        h.execute_case(name);
    }

    fn case(&self, name: &str) {
        match name {
            "test_native_hero_checks_detect_corruption_instead_of_only_testing_unit_count" => {
                for index in 1..=6 {
                    self.variant(name, vec![Value::Integer(index)]);
                }
            }
            "test_native_hero_archival_and_explicit_suicide_preserve_requested_outcome"
            | "test_rock_band_state_is_rejected_by_type_class_or_tag_before_changes"
            | "test_candidate_special_charge_blocks_and_reuses_the_candidate_on_resume" => {
                for index in 1..=3 {
                    self.variant(name, vec![Value::Integer(index)]);
                }
            }
            "test_runtime_park_and_nonhero_action_charges_are_rejected_before_changes" => {
                for index in 1..=2 {
                    self.variant(name, vec![Value::Integer(index)]);
                }
            }
            "test_ai_purchase_receipt_rejects_interleaved_faith_or_unit_events" => {
                for index in 1..=4 {
                    self.variant(name, vec![Value::Integer(index)]);
                }
            }
            "test_control_missionary_publication_checks_charges_then_core_religion"
            | "test_recall_publication_uses_ui_class_getter_and_waits_for_native_claim"
            | "test_recall_spawn_preparation_moves_only_blockers_and_preserves_them" => {
                Self::create(false).execute_case(name);
            }
            "test_cleric_fixture_can_be_purchased_without_enabling_other_disabled_units" => {
                package_units()
            }
            "test_scenario_lua_compiles_and_both_snapshots_have_round_trip_comparisons" => {
                scenario_round_trips(&self.lua)
            }
            "test_human_recall_scenario_uses_custom_native_classes_and_real_purchase" => {
                human_scenario(&self.lua)
            }
            "test_modinfo_references_exist_and_have_no_open_pr_dependencies" => package_manifest(),
            "test_all_text_has_exactly_japanese_english_and_russian" => package_text(),
            "test_gameplay_scenario_checks_use_the_context_aware_spread_snapshot" => {
                context_spread()
            }
            "test_all_gameplay_scenario_chunks_use_shared_lifespan_key" => context_lifespan(),
            _ => self.execute_case(name),
        }
    }
}

fn scenario(name: &str) -> Json {
    serde_json::from_str(&read_repo(format!("tools/civ6-autotest/scenarios/{name}"))).unwrap()
}

fn steps(data: &Json) -> &[Json] {
    required_json(data, "steps")
        .as_array()
        .expect("steps must be an array")
}
fn optional_json<'a>(value: &'a Json, key: &str) -> Option<&'a Json> {
    value.as_object().expect("expected JSON object").get(key)
}
fn required_json<'a>(value: &'a Json, key: &str) -> &'a Json {
    optional_json(value, key).unwrap_or_else(|| panic!("missing JSON key {key}"))
}
fn optional_text<'a>(value: &'a Json, key: &str) -> Option<&'a str> {
    optional_json(value, key).map(|v| {
        v.as_str()
            .unwrap_or_else(|| panic!("present JSON key {key} must be a string"))
    })
}
fn action<'a>(data: &'a Json, name: &str) -> &'a Json {
    steps(data)
        .iter()
        .find(|s| required_json(s, "action") == name)
        .unwrap()
}
fn string_set<'a>(values: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    values.into_iter().map(str::to_owned).collect()
}
fn compile_steps(lua: &Lua, data: &Json) {
    for step in steps(data) {
        if required_json(step, "action") == "ui_lua" {
            lua.load(step["code"].as_str().unwrap())
                .into_function()
                .unwrap();
        }
        if required_json(step, "action") == "wait_for" {
            let condition = required_json(step, "condition");
            lua.load(
                required_json(condition, "code")
                    .as_str()
                    .expect("wait condition code must be a string"),
            )
            .into_function()
            .unwrap();
        }
    }
}
fn context_spread() {
    for name in ["absorption-japan.json", "absorption-hero-recall.json"] {
        let data = scenario(name);
        let chunks: Vec<_> = steps(&data)
            .iter()
            .filter(|s| optional_json(s, "state_name").is_some_and(|v| v == "JAC_Absorption_Entry"))
            .map(|s| s["code"].as_str().unwrap())
            .collect();
        assert!(!chunks.is_empty());
        for code in chunks {
            assert!(!code.contains(":GetSpreadCharges("), "{name}");
        }
    }
}
fn context_lifespan() {
    for name in ["absorption-japan.json", "absorption-hero-recall.json"] {
        let data = scenario(name);
        check_context_lifespan(&data, name);
    }
}
fn check_context_lifespan(data: &Json, name: &str) {
    for step in steps(data) {
        if optional_json(step, "state_name").is_some_and(|v| v == "JAC_Absorption_Entry") {
            assert!(
                !optional_text(step, "code")
                    .unwrap_or("")
                    .replace(
                        "(UnitPropertyKeys and UnitPropertyKeys.LifespanRemaining)",
                        ""
                    )
                    .contains("UnitPropertyKeys.LifespanRemaining"),
                "{name}"
            );
        }
    }
    assert!(steps(data)
        .iter()
        .map(|s| optional_text(s, "code").unwrap_or(""))
        .collect::<String>()
        .contains("ExposedMembers.JAC_HeroLifespanKey"));
}
fn scenario_round_trips(lua: &Lua) {
    let data = scenario("absorption-japan.json");
    compile_steps(lua, &data);
    check_capture_sets(&data);
    check_scenario_configuration(&data);
}
fn check_capture_sets(data: &Json) {
    let captures: BTreeSet<_> = steps(data)
        .iter()
        .filter_map(|s| optional_text(s, "capture_as"))
        .filter(|s| !s.starts_with("JAC_ABSORPTION_SYNC_"))
        .map(str::to_owned)
        .collect();
    assert_eq!(
        captures,
        string_set([
            "ABSORPTION_IMMEDIATE",
            "ABSORPTION_AFTER_AI_TURN",
            "ABSORPTION_AI_RECALL"
        ])
    );
    let comparisons: BTreeSet<_> = steps(data)
        .iter()
        .filter_map(|s| optional_text(s, "equals_capture"))
        .map(str::to_owned)
        .collect();
    assert_eq!(comparisons, captures);
}
fn check_scenario_configuration(data: &Json) {
    for name in ["save", "load"] {
        assert_eq!(
            steps(data)
                .iter()
                .filter(|s| required_json(s, "action") == name)
                .count(),
            3
        );
    }
    assert!(action(data, "enable_mods")["ids"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "8424840C-92EF-4426-A9B4-B4E0CB818049"));
    let modes: Vec<_> = steps(data)
        .iter()
        .filter(|s| {
            required_json(s, "action") == "ui_lua" && required_json(s, "state_name") == "FrontEnd"
        })
        .collect();
    assert_eq!(modes.len(), 1);
    assert!(modes[0]["code"]
        .as_str()
        .unwrap()
        .contains("GameConfiguration.SetValue('GAMEMODE_HEROES',true)"));
    let configure = action(data, "configure");
    assert_eq!(
        configure["map_file"],
        "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map"
    );
    let ai: BTreeSet<_> = configure["players"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| !Data::from(required_json(p, "human").clone()).python_truthy())
        .map(|p| p["civ"].as_str().unwrap())
        .collect();
    assert!(BTreeSet::from(["CIVILIZATION_JAC_ASHINA", "CIVILIZATION_JAC_DATE"]).is_subset(&ai));
    strict::assert_python_eq(
        &Data::from(action(data, "end_turn")["turns"].clone()),
        1_i64,
    );
}
fn human_scenario(lua: &Lua) {
    let data = scenario("absorption-hero-recall.json");
    compile_steps(lua, &data);
    let players = action(&data, "configure")["players"].as_array().unwrap();
    assert_eq!(players.len(), 22);
    assert_eq!(
        players
            .iter()
            .filter(|p| Data::from(required_json(p, "human").clone()).python_truthy())
            .map(|p| p["civ"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["CIVILIZATION_JAC_ODA"]
    );
    for (name, n) in [("end_turn", 1), ("save", 2), ("load", 2)] {
        assert_eq!(
            steps(&data)
                .iter()
                .filter(|s| required_json(s, "action") == name)
                .count(),
            n
        );
    }
    let code = steps(&data)
        .iter()
        .filter(|s| required_json(s, "action") == "ui_lua")
        .map(|s| s["code"].as_str().unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "CityManager.RequestCommand(city,CityCommandTypes.PURCHASE,parameters)",
        "A.BeginHeroRecall",
        "A.CompleteHeroRecall",
        "non-Banseiki Hero class enabled",
    ] {
        assert!(code.contains(needle));
    }
}
fn node<'a>(root: roxmltree::Node<'a, 'a>, path: &str) -> roxmltree::Node<'a, 'a> {
    path.split('/').fold(root, |n, tag| {
        n.children().find(|c| c.has_tag_name(tag)).unwrap()
    })
}
fn package_units() {
    let db = civ6_lua_tests_rs::official::official_db();
    db.execute_batch("UPDATE Units SET CanTrain=0 WHERE UnitType IN ('UNIT_MISSIONARY','UNIT_APOSTLE'); UPDATE Units SET CanTrain=1 WHERE UnitType='UNIT_WARRIOR';").unwrap();
    let other_units = query_json(
        &db,
        "SELECT UnitType,CanTrain FROM Units WHERE UnitType<>'UNIT_MISSIONARY' ORDER BY UnitType",
    );
    db.execute_batch(&read_repo(format!("{MOD}/Data/JAC_Absorption.sql")))
        .unwrap();
    let actual: BTreeMap<String, i64> = db
        .prepare("SELECT UnitType,CanTrain FROM Units WHERE UnitType IN ('UNIT_MISSIONARY','UNIT_APOSTLE','UNIT_WARRIOR')")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(
        actual,
        BTreeMap::from([
            ("UNIT_MISSIONARY".into(), 1),
            ("UNIT_APOSTLE".into(), 0),
            ("UNIT_WARRIOR".into(), 1)
        ])
    );
    assert_eq!(query_json(&db, "SELECT UnitType,CanTrain FROM Units WHERE UnitType<>'UNIT_MISSIONARY' ORDER BY UnitType"), other_units);
    let source = read_repo(format!("{MOD}/JAC_PoC_Absorption.modinfo"));
    let doc = roxmltree::Document::parse(&source).unwrap();
    let action = node(doc.root_element(), "InGameActions/UpdateDatabase");
    assert_eq!(
        action.attribute("criteria"),
        Some("JAC_Absorption_Banseiki")
    );
    assert!(
        node(action, "Properties/LoadOrder")
            .text()
            .unwrap()
            .parse::<i64>()
            .unwrap()
            > 200
    );
}
fn package_manifest() {
    let source = read_repo(format!("{MOD}/JAC_PoC_Absorption.modinfo"));
    let doc = roxmltree::Document::parse(&source).unwrap();
    let root = doc.root_element();
    let files: BTreeSet<_> = node(root, "Files")
        .children()
        .filter(|c| c.has_tag_name("File"))
        .map(|c| c.text().unwrap().to_owned())
        .collect();
    assert_eq!(
        files,
        string_set([
            "Config/JAC_Absorption_ArtifactTest.xml",
            "Data/JAC_Absorption.sql",
            "Data/JAC_Absorption_ArtifactTest.sql",
            "Text/JAC_Absorption_Text.xml",
            "Scripts/JAC_Absorption_Rules.lua",
            "Scripts/JAC_Absorption_Gameplay.lua",
            "Scripts/JAC_Absorption_Flow.lua",
            "Scripts/JAC_Absorption_Entry.lua",
            "Scripts/JAC_CarrierBoundary.lua",
            "Scripts/JAC_NativeInventory.lua",
            "UI/JAC_AbsorptionFlow.lua",
            "UI/JAC_AbsorptionFlow.xml",
            "UI/JAC_Absorption_HeroKeys.lua",
            "UI/JAC_CarrierMP_UI.lua",
            "UI/JAC_NativeInventory_UI.lua",
            "UI/JAC_Absorption_HeroKeys.xml"
        ])
    );
    for file in files {
        assert!(repo_root().join(MOD).join(file).is_file());
    }
    let dependencies: BTreeSet<_> = node(root, "Dependencies")
        .children()
        .filter(|c| c.has_tag_name("Mod"))
        .map(|c| c.attribute("id").unwrap().to_owned())
        .collect();
    assert_eq!(
        dependencies,
        string_set([
            "4873eb62-8ccc-4574-b784-dda455e74e68",
            "bd7a82c5-6b38-4b69-a7f5-8dc9af534249",
            "8424840C-92EF-4426-A9B4-B4E0CB818049"
        ])
    );
    assert_eq!(
        node(root, "ActionCriteria/Criteria/RuleSetInUse").text(),
        Some("RULESET_JAC_BANSEIKI")
    );
    let test_action = node(root, "InGameActions")
        .children()
        .find(|n| n.attribute("id") == Some("JAC_Absorption_ArtifactTest_Data"))
        .unwrap();
    assert_eq!(
        test_action.attribute("criteria"),
        Some("JAC_Absorption_ArtifactTest")
    );
    assert_eq!(
        node(test_action, "File").text(),
        Some("Data/JAC_Absorption_ArtifactTest.sql")
    );
    assert_eq!(
        node(test_action, "Properties/LoadOrder").text(),
        Some("301")
    );
    let criteria = node(root, "ActionCriteria")
        .children()
        .find(|n| n.attribute("id") == Some("JAC_Absorption_ArtifactTest"))
        .unwrap();
    assert_eq!(
        node(criteria, "RuleSetInUse").text(),
        Some("RULESET_JAC_BANSEIKI")
    );
    let flag = node(criteria, "ConfigurationValueMatches");
    assert_eq!(node(flag, "Group").text(), Some("Game"));
    assert_eq!(
        node(flag, "ConfigurationId").text(),
        Some("JAC_ABSORPTION_ARTIFACT_TEST")
    );
    assert_eq!(node(flag, "Value").text(), Some("1"));
    let config_source = read_repo(format!("{MOD}/Config/JAC_Absorption_ArtifactTest.xml"));
    let config = roxmltree::Document::parse(&config_source).unwrap();
    let parameter = node(config.root_element(), "Parameters/Row");
    assert_eq!(parameter.attribute("ConfigurationGroup"), Some("Game"));
    assert_eq!(
        parameter.attribute("ConfigurationId"),
        Some("JAC_ABSORPTION_ARTIFACT_TEST")
    );
    assert_eq!(parameter.attribute("DefaultValue"), Some("0"));
    assert_eq!(parameter.attribute("Visible"), Some("0"));
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE Units(UnitType TEXT PRIMARY KEY, CanTrain INTEGER, PrereqCivic TEXT);
      INSERT INTO Units VALUES('UNIT_ARCHAEOLOGIST',0,'CIVIC_ORIGINAL'),('UNIT_MISSIONARY',0,NULL),('UNIT_WARRIOR',1,NULL);").unwrap();
    conn.execute_batch(&read_repo(format!(
        "{MOD}/Data/JAC_Absorption_ArtifactTest.sql"
    )))
    .unwrap();
    let values: Vec<(String, i64)> = conn
        .prepare("SELECT UnitType,CanTrain FROM Units ORDER BY UnitType")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        values,
        vec![
            ("UNIT_ARCHAEOLOGIST".into(), 1),
            ("UNIT_MISSIONARY".into(), 0),
            ("UNIT_WARRIOR".into(), 1)
        ]
    );
    assert_eq!(
        conn.query_row(
            "SELECT PrereqCivic FROM Units WHERE UnitType='UNIT_ARCHAEOLOGIST'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "CIVIC_ORIGINAL"
    );
}
fn package_text() {
    let source = read_repo(format!("{MOD}/Text/JAC_Absorption_Text.xml"));
    let doc = roxmltree::Document::parse(&source).unwrap();
    let mut texts = BTreeMap::<String, BTreeMap<String, String>>::new();
    for row in node(doc.root_element(), "LocalizedText")
        .children()
        .filter(|c| c.has_tag_name("Row"))
    {
        let text = node(row, "Text").text().unwrap_or("").to_owned();
        let languages = texts
            .entry(row.attribute("Tag").unwrap().to_owned())
            .or_default();
        assert!(languages
            .insert(row.attribute("Language").unwrap().to_owned(), text)
            .is_none());
    }
    assert_eq!(texts.len(), 110);
    for languages in texts.values() {
        assert_eq!(
            languages.keys().cloned().collect::<BTreeSet<_>>(),
            string_set(["ja_JP", "en_US", "ru_RU"])
        );
        assert!(languages.values().all(|s| !s.is_empty()));
    }
}

#[derive(Deserialize)]
struct Parity {
    source: String,
    #[serde(rename = "class")]
    class_name: String,
    test: String,
    rust_test: String,
    lua_assertions: usize,
}

#[test]
fn absorption_inventory_preserves_all_120_methods_and_117_embedded_assertions() {
    let rows: Vec<Parity> = serde_json::from_str(include_str!("absorption-parity.json")).unwrap();
    assert_eq!(rows.len(), 120);
    let registered = inventory::resolve(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/absorption.rs"),
        "absorption",
    );
    assert_eq!(
        registered,
        inventory::parse(include_str!("absorption.rs"), "absorption")
    );
    let mut originals = BTreeSet::new();
    let mut mapped = BTreeSet::new();
    let mut counts = BTreeMap::new();
    let lua = new_lua();
    let strings: Table = lua
        .load(include_str!("../fixtures/absorption-literals.lua"))
        .strict_eval()
        .unwrap();
    let assertion = Regex::new(r"\bassert\s*\(").unwrap();
    let mut assertions = 0;
    for row in rows {
        assert_eq!(row.source, "tests/test_absorption_lua.py");
        assert!(originals.insert((row.class_name.clone(), row.test.clone())));
        assert!(mapped.insert(row.rust_test.clone()));
        assert_eq!(
            row.rust_test,
            format!("absorption::{}::{}", row.class_name, row.test)
        );
        assert!(registered.contains(&row.rust_test));
        *counts.entry(row.class_name.clone()).or_insert(0) += 1;
        let literals: Value = strings
            .get(format!("{}.{}", row.class_name, row.test))
            .unwrap();
        let actual = match literals {
            Value::Nil => 0,
            Value::Table(t) => t
                .strict_values::<String>()
                .map(|s| assertion.find_iter(&s.unwrap()).count())
                .sum(),
            other => panic!("{other:?}"),
        };
        assert_eq!(actual, row.lua_assertions, "{}", row.test);
        assertions += actual;
    }
    assert_eq!(
        counts,
        BTreeMap::from([
            ("AbsorptionTests".into(), 111),
            ("AbsorptionPackageTests".into(), 9)
        ])
    );
    assert_eq!(assertions, 117);
    assert_eq!(registered.len(), 125);
}

#[test]
fn absorption_boundaries_keep_python_values_and_raw_lua_errors_distinct() {
    let h = Harness::new();
    h.lua.load(r#"
        EQ(function() return {a=true,b={false,1.0}} end,
           function() return {a=1,b={0,true}} end)
        EQ(function() return 1,2,nil end,function() return TUPLE(1,2,nil) end)
        EQ(function() return COUNT({[1]='one',[3]='three',extra='all pairs'}) end,function() return 3 end)
        TRUTH(function() return {} end,true)
        TRUTH(function() return 0 end,false)
        TRUTH(function() return '' end,false)
        TRUTH(function() return 'false' end,true)
        TRUTH(function() return false,nil end,true)
        IDENTITY(function() return false end,false)
        IS_NIL(function() return Players[1].units[3] end,false)
        IS_NIL(function() end,true)
        ERROR(function() error('real Lua failure') end,'real Lua failure')
        JAC_AbsorptionGameplay.BoundaryProbe=function() return 1,2,nil end
        EQ(function() return A.BoundaryProbe() end,function() return TUPLE(1,2,nil) end)
    "#).exec().unwrap();
    for (source, expected) in [
        ("EQ(function() return '1' end,function() return 1 end)", "EQ:"),
        ("EQ(function() return 1,2,nil end,function() return 1,2 end)", "EQ:"),
        ("EQ(function() return {[0]='zero',['0']='string'} end,function() return {[0]='zero'} end)", "EQ:"),
        ("EQ(function() return A.BoundaryProbe() end,function() return TUPLE(1,2) end)", "EQ:"),
        ("IDENTITY(function() return 0 end,false)", "boolean identity"),
        ("IS_NIL(function() return nil,nil end,true)", "None identity"),
        ("ERROR(function() return 'false' end,'.*')", "expected raw Lua error"),
        ("ERROR(function() local cycle={};cycle.self=cycle;return cycle end,'.*')", "expected raw Lua error"),
    ] {
        let error=h.lua.load(source).exec().expect_err("invalid assertion accepted");
        assert!(error.to_string().contains(expected),"{source}: {error}");
    }
}

#[test]
fn legacy_table_length_rejects_string_keys_and_sparse_recall_history() {
    let h = Harness::new();
    h.lua.load("EQ(function() return PY_LEN({saved={}}) end,function() return 0 end); local sparse={[1]={},[3]={},saved={}}; EQ(function() return PY_LEN(sparse) end,function() return #sparse end); EQ(function() return COUNT(sparse) end,function() return 3 end)").exec().unwrap();
    for history in [
        "{saved=record.RecallHistory[1]}",
        "{[3]=record.RecallHistory[1]}",
    ] {
        let h = Harness::new();
        h.lua.load(format!("local reload=RELOAD;RELOAD=function() reload();local api=JAC_AbsorptionGameplay;local complete=api.CompleteHeroRecall;api.CompleteHeroRecall=function(id) local record=complete(id);record.RecallHistory={history};return record end end;RELOAD()")).exec().unwrap();
        let error = h
            .lua
            .load("CASES.test_recall_resumes_after_token_write_and_does_not_duplicate_history()")
            .exec()
            .expect_err("malformed history satisfied legacy len==1");
        assert!(error.to_string().contains("EQ:"), "{history}: {error}");
    }
}

#[test]
fn python_subtraction_rejects_numeric_strings_and_keeps_boolean_numbers() {
    let lua = new_lua();
    boundaries(&lua);
    lua.load("EQ(function() return PY_SUB(5000,4000) end,function() return 1000 end); EQ(function() return PY_SUB(true,false) end,function() return 1 end); EQ(function() return PY_SUB(1001,true) end,function() return 1000 end); EQ(function() return PY_SUB(1.5,false) end,function() return 1.5 end)").exec().unwrap();
    for (left, right) in [
        ("'5000'", "'4000'"),
        ("5000", "'4000'"),
        ("'5000'", "4000"),
        ("nil", "1"),
        ("true", "'0'"),
    ] {
        let error = lua
            .load(format!("PY_SUB({left},{right})"))
            .exec()
            .expect_err("non-numeric Python operand accepted");
        assert!(error
            .to_string()
            .contains("Python subtraction requires a number"));
    }
    assert!(matches!(
        python_subtract(Value::Boolean(true), Value::Boolean(false)).unwrap(),
        Value::Integer(1)
    ));
    assert!(
        matches!(python_subtract(Value::Number(1.5),Value::Boolean(false)).unwrap(),Value::Number(n) if n==1.5)
    );
    for group in ["AbsorptionPackageTests.test_ai_purchase_receipt_rejects_interleaved_faith_or_unit_events","AbsorptionTests.test_live_recall_probes_execute_native_eligibility_checks_and_detect_wrong_owner"] {
        let gameplay=group.starts_with("AbsorptionTests.");
        let h=Harness::create(gameplay);
        let literals=h.lua.globals().strict_get::<Table>("STRINGS").unwrap().strict_get::<Table>(group).unwrap();
        let index=if gameplay {64}else{22};
        let source=literals.strict_get::<String>(index).unwrap();
        literals.set(index,format!("{source};JAC_ABSORPTION_AI_PURCHASE.FaithBefore='5000';JAC_ABSORPTION_AI_PURCHASE.FaithAfter='4000'")).unwrap();
        h.lua.globals().set("ARG1",1).unwrap();
        let name=group.split_once('.').unwrap().1;
        let error=h.lua.load(format!("CASES.{name}()")).exec().expect_err("payment accepted numeric strings");
        assert!(error.to_string().contains("Python subtraction requires a number"),"{group}: {error}");
    }
}

#[test]
fn scenario_presence_rejects_invalid_capture_code_and_required_keys() {
    let baseline = scenario("absorption-japan.json");
    check_capture_sets(&baseline);
    for key in ["capture_as", "equals_capture"] {
        for invalid in [
            Json::from(42),
            Json::from(false),
            Json::from(0),
            Json::Null,
            Json::from(vec!["invalid"]),
        ] {
            let mut data = baseline.clone();
            data["steps"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({key:invalid}));
            assert!(
                std::panic::catch_unwind(|| check_capture_sets(&data)).is_err(),
                "invalid present {key} ignored"
            );
        }
    }
    for invalid in [
        Json::Null,
        Json::from(false),
        Json::from(0),
        Json::from(vec!["invalid"]),
    ] {
        let mut data = baseline.clone();
        data["steps"][0]["code"] = invalid.clone();
        assert!(std::panic::catch_unwind(|| check_context_lifespan(&data, "mutation")).is_err());
        assert!(std::panic::catch_unwind(|| optional_text(&invalid, "code")).is_err());
        assert!(std::panic::catch_unwind(|| steps(&invalid)).is_err());
    }
    assert_eq!(optional_text(&serde_json::json!({}), "code"), None);
    assert!(std::panic::catch_unwind(|| {
        steps(&Json::from("invalid"));
    })
    .is_err());
    assert_eq!(
        optional_text(&serde_json::json!({"code":""}), "code"),
        Some("")
    );
    assert!(std::panic::catch_unwind(|| {
        required_json(&serde_json::json!({}), "human");
    })
    .is_err());
    let lua = new_lua();
    boundaries(&lua);
    for (source, read) in [
        (
            r#"{"steps":[{"code":false}]}"#,
            "JSON_TEXT(data.steps[1],'code')",
        ),
        (
            r#"{"steps":[{"capture_as":null}]}"#,
            "JSON_TEXT(data.steps[1],'capture_as')",
        ),
        (
            r#"{"steps":[{"equals_capture":42}]}"#,
            "JSON_TEXT(data.steps[1],'equals_capture')",
        ),
        (
            r#"{"steps":[{"condition":{"code":0}}]}"#,
            "JSON_TEXT(JSON_GET(data.steps[1],'condition',{}),'code')",
        ),
    ] {
        let json = lua.globals().strict_get::<Function>("JSON").unwrap();
        json.call::<MultiValue>(source)
            .expect("JSON conversion must not validate unused fields");
        let error = lua
            .load(format!("local data=JSON([==[{source}]==]);{read}"))
            .exec()
            .expect_err("invalid field accepted at use");
        assert!(
            error.to_string().contains("expected Lua string"),
            "{source}: {error}"
        );
    }
    let h = Harness::create(false);
    let mut unused = baseline.clone();
    unused["steps"][2]["condition"] = Json::Null;
    compile_steps(&h.lua, &unused);
    check_capture_sets(&unused);
    check_scenario_configuration(&unused);
    let source = serde_json::to_string(&unused).unwrap();
    h.lua
        .globals()
        .strict_get::<Function>("JSON")
        .unwrap()
        .call::<MultiValue>(source)
        .unwrap();
    h.lua.load(r#"local data=JSON('{"steps":[{"action":"ui_lua","code":false,"condition":null,"capture_as":42},{"action":"wait_for","condition":{"code":"return true"}}]}');assert(FIRST_CONDITION_STEP(data.steps,'return true',true)==data.steps[2])"#).exec().unwrap();
    for (name, needle) in [
        ("absorption-japan.json", "JAC_ABSORPTION_CONTROL_PRIEST_V1"),
        ("absorption-hero-recall.json", "e.TransferID"),
    ] {
        let valid = scenario(name);
        assert_eq!(valid["steps"][4]["action"], "wait_for");
        assert!(!valid["steps"][4]["condition"]["code"]
            .as_str()
            .unwrap()
            .contains(needle));
        for key in ["condition", "code"] {
            let mut missing = valid.clone();
            let object = if key == "condition" {
                &mut missing["steps"][4]
            } else {
                &mut missing["steps"][4]["condition"]
            };
            assert!(object.as_object_mut().unwrap().remove(key).is_some());
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| compile_steps(
                    &h.lua, &missing
                )))
                .is_err()
            );
            let source = serde_json::to_string(&missing).unwrap();
            let error=h.lua.load(format!("local data=JSON([==[{source}]==]);FIRST_CONDITION_STEP(data.steps,[==[{needle}]==],true)")).exec().expect_err("missing wait key skipped in favor of a later match");
            assert!(
                error
                    .to_string()
                    .contains(&format!("missing JSON key {key}")),
                "{name}: {error}"
            );
        }
    }
    h.lua.load("local rows={{},{condition={}},{condition={code='return true'}}};assert(FIRST_CONDITION_STEP(rows,'return true')==rows[3])").exec().unwrap();
    for invalid in [
        Json::Null,
        Json::from(false),
        Json::from(0),
        Json::from("invalid"),
    ] {
        let selected = serde_json::json!({"steps":[{"action":"wait_for","condition":invalid}]});
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| compile_steps(
                &h.lua, &selected
            )))
            .is_err()
        );
        let source = serde_json::to_string(&selected).unwrap();
        h.lua.load(format!("local data=JSON([==[{source}]==]);FIRST_CONDITION_STEP(data.steps,'return true',true)")).exec().expect_err("invalid selected wait condition accepted");
    }
}

macro_rules! simple_tests {
    ($($name:ident),* $(,)?) => {
        $(#[test] fn $name() { Harness::new().case(stringify!($name)); })*
    };
}

#[allow(non_snake_case)]
mod AbsorptionTests {
    use super::*;
    simple_tests! {
        test_all_assets_and_native_components_transfer_before_last_city_elimination,
        test_save_reload_discards_lua_session_and_restores_properties_without_replaying,
        test_non_transfer_branches_keep_original_snapshots_and_detach_person_references,
        test_returned_belonging_goes_to_explicit_surviving_third_party,
        test_every_spec_disposition_is_routable_and_rejects_cross_kind_outcomes,
        test_great_people_cannot_be_converted_to_yields,
        test_missing_unit_decision_is_rejected_before_any_mutation,
        test_missing_asset_decision_is_rejected_before_any_mutation,
        test_inconsistent_linked_person_and_unit_decisions_are_rejected,
        test_coexistence_is_not_absorption,
        test_entry_and_external_relations_require_explicit_resolution,
        test_asset_collision_is_rejected_before_engine_changes,
        test_invalid_return_is_rejected,
        test_native_clones_stage_at_recipient_city_then_return_to_original_positions,
        test_large_army_batches_publication_requests_instead_of_waiting_per_unit,
        test_off_map_native_candidates_are_placed_before_transfer_and_verified,
        test_failed_off_map_placement_keeps_donors_and_reuses_saved_candidate_on_resume,
        test_empty_army_still_waits_for_city_cache_publication_before_preparation,
        test_public_verification_requires_a_new_publication_and_consumes_it,
        test_ui_only_purchase_cost_and_faith_balance_return_scalars_to_core,
        test_ui_only_spread_getter_waits_for_publication_and_reuses_candidates,
        test_ui_only_spread_mismatch_blocks_and_preserves_donors_until_resume,
        test_old_turn_ui_spread_cache_waits_before_preparation_and_resumes,
        test_same_turn_spent_spread_is_published_before_snapshot_and_never_restored_as_unused,
        test_rejected_preparation_releases_factions_and_corrected_request_waits_for_new_publication,
        test_native_hero_ui_only_spread_wait_never_becomes_suicide,
        test_published_hero_spread_mismatch_records_suicide_instead_of_blocking_other_assets,
        test_split_ui_getters_and_gameplay_mutations_preserve_native_heroes,
        test_fresh_published_hero_mismatch_is_archived_without_accepting_stale_ui_as_success,
        test_ui_only_city_citizens_and_work_slots_wait_for_final_publication,
        test_only_missing_work_slot_getter_still_requires_final_ui_publication,
        test_native_heroes_keep_identity_spent_charges_renown_origin_and_person_record,
        test_native_hero_absent_gameplay_manager_becomes_suicide_history,
        test_dormant_native_claim_without_unit_is_archived_as_suicide,
        test_dormant_claim_with_stale_person_reference_preserves_requested_outcome,
        test_native_hero_missing_origin_city_becomes_suicide,
        test_native_hero_missing_origin_building_becomes_suicide,
        test_native_heroes_sharing_origin_archive_the_second_claim,
        test_native_hero_no_spawn_after_claim_change_finishes_as_suicide,
        test_native_hero_init_throw_after_creation_adopts_candidate_without_recreating,
        test_native_hero_token_write_failure_kills_only_its_candidate,
        test_native_hero_wrong_origin_is_suicide_without_duplicate,
        test_native_hero_city_failure_can_resume_from_escrow_without_second_kill,
        test_native_hero_manager_mismatch_is_not_reported_as_inheritance,
        test_hero_creation_error_does_not_kill_a_third_party_unit,
        test_reused_donor_id_blocks_without_killing_an_unrelated_unit,
        test_native_hero_restore_error_becomes_suicide_without_losing_other_assets,
        test_native_hero_recall_binds_new_id_and_person_after_reload,
        test_recall_after_second_absorption_preserves_original_owner_and_uses_current_mapping,
        test_recall_rejects_candidate_and_token_collisions_without_killing_them,
        test_recall_resumes_after_token_write_and_does_not_duplicate_history,
        test_native_hero_checks_detect_corruption_instead_of_only_testing_unit_count,
        test_native_lifespan_changes_after_turn_but_owner_recall_identity_must_remain,
        test_gameplay_scenario_checks_use_the_context_aware_spread_snapshot,
        test_all_gameplay_scenario_chunks_use_shared_lifespan_key,
        test_ui_publishes_engine_enum_when_gameplay_has_no_unit_property_keys,
        test_shipped_lifespan_property_is_used_without_legacy_lifespan_or_renown_methods,
        test_native_hero_archival_and_explicit_suicide_preserve_requested_outcome,
        test_native_hero_position_is_restored_separately_from_its_recall_origin,
        test_native_hero_unrestorable_exposed_renown_becomes_suicide,
        test_native_hero_is_not_escrowed_when_an_ordinary_unit_cannot_preserve_spent_charges,
        test_native_hero_duplicate_at_absorber_survives_suicide,
        test_native_hero_city_transfer_throw_after_mutation_resumes_with_the_new_city_id,
        test_native_hero_token_set_then_throw_becomes_suicide,
        test_engine_spawn_failure_preserves_every_donor_unit_and_city_then_resume_is_idempotent,
        test_property_copy_failure_is_blocked_before_removing_any_donor,
        test_spent_build_charges_need_an_adapter_and_never_destroy_the_donor,
        test_city_failure_retains_snapshots_and_resume_does_not_recreate_units,
        test_later_turn_resume_is_rejected_without_reapplying_old_damage_or_movement,
        test_same_transaction_id_with_changed_decisions_is_rejected,
        test_verification_catches_missing_or_corrupted_transfers_instead_of_only_counting,
        test_completed_transaction_is_not_replayed_after_later_turns,
        test_registered_unit_and_city_properties_are_preserved_and_unsupported_values_rejected,
        test_adapter_preflight_apply_verify_and_reload_contract,
        test_bad_adapter_and_empty_string_fail_before_native_mutation,
        test_foreign_reference_rebind_keeps_target_deadline_and_fixed_roll_and_survives_reload,
        test_saved_rebound_expectation_is_used_on_resume_without_reloading_callback,
        test_corrupted_rebound_reference_is_detected_by_generic_verification,
        test_city_property_rebind_uses_saved_expectations_on_reload,
        test_nonserializable_property_rebind_blocks_before_destroying_donors,
        test_native_hero_detected_by_engine_flag_with_a_nonstandard_type_is_rejected,
        test_missing_native_great_person_charge_api_blocks_before_any_mutation,
        test_spent_native_great_person_charges_do_not_reset_on_transfer,
        test_missing_staged_replacement_blocks_instead_of_breaking_saved_references,
        test_live_scenario_builds_missing_hero_origins_on_their_actual_city_plots,
        test_live_scenario_checks_execute_in_lua_and_detect_asset_corruption_after_reload,
        test_live_transfer_rejects_missing_cleric_before_ai_turn,
        test_live_ai_turn_allows_cleric_consumption_but_checks_current_save_snapshot,
        test_live_recall_probes_execute_native_eligibility_checks_and_detect_wrong_owner,
        test_human_receiver_scenario_executes_real_recall_and_rebinds_person_in_lua_vm,
        test_partial_expected_properties_freeze_references_to_every_staged_unit,
        test_receiver_assets_added_or_updated_while_blocked_survive_resume,
        test_receiver_assets_added_by_city_transfer_hooks_are_not_overwritten,
        test_new_destination_id_collision_is_stopped_before_removing_donors,
        test_source_assets_created_while_blocked_are_preserved_instead_of_cleared,
        test_unsupported_charge_types_stop_before_source_assets_are_changed,
        test_runtime_special_charge_without_definition_is_also_rejected,
        test_spent_park_charge_cannot_be_reset_by_absorption,
        test_runtime_park_and_nonhero_action_charges_are_rejected_before_changes,
        test_nonhero_action_charges_declared_in_mode_table_are_rejected,
        test_rock_band_state_is_rejected_by_type_class_or_tag_before_changes,
        test_candidate_special_charge_blocks_and_reuses_the_candidate_on_resume,
        test_city_district_loss_is_detected_with_buildings_and_territory_intact,
        test_legacy_city_assets_keep_recorded_checks_without_inventing_district_coverage,
        test_new_city_asset_coverage_rejects_missing_district_snapshot,
        test_multiple_same_type_districts_and_ui_only_enumeration_are_preserved,
        test_district_only_ui_cache_waits_after_transfer_and_across_resume,
        test_palace_work_loss_is_detected_even_when_the_palace_building_is_exempt,
        test_palace_work_moved_to_receiver_capital_is_preserved_and_verified,
        test_previous_governance_is_saved_before_engine_clears_city_properties,
        test_exact_verification_detects_city_rename_but_later_turns_allow_it,
    }
}

#[allow(non_snake_case)]
mod AbsorptionPackageTests {
    use super::*;
    simple_tests! {
        test_ai_purchase_receipt_rejects_interleaved_faith_or_unit_events,
        test_cleric_fixture_can_be_purchased_without_enabling_other_disabled_units,
        test_scenario_lua_compiles_and_both_snapshots_have_round_trip_comparisons,
        test_control_missionary_publication_checks_charges_then_core_religion,
        test_recall_publication_uses_ui_class_getter_and_waits_for_native_claim,
        test_recall_spawn_preparation_moves_only_blockers_and_preserves_them,
        test_human_recall_scenario_uses_custom_native_classes_and_real_purchase,
        test_modinfo_references_exist_and_have_no_open_pr_dependencies,
        test_all_text_has_exactly_japanese_english_and_russian,
    }
}
