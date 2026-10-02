//! Fixed method inventory from the four PR86 Python suites at d801f97.
//! The Python sources are deliberately absent from this branch.

use civ6_lua_tests_rs::repo_root;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use syn::parse::Parser;
use syn::{punctuated::Punctuated, Ident, Item, Token};

#[path = "support/inventory.rs"]
mod inventory;

#[derive(Deserialize)]
struct Parity {
    source: String,
    #[serde(rename = "class")]
    class_name: String,
    test: String,
    rust_file: String,
    rust_test: String,
}

fn original_digest(rows: &[Parity]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for (index, row) in rows.iter().enumerate() {
        if index != 0 {
            hash ^= b'\n' as u64;
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        for byte in format!("{}|{}|{}", row.source, row.class_name, row.test).bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}

fn registered_persona_lua(source: &str) -> BTreeSet<String> {
    // Keep the sole generated test family explicit. An invocation containing
    // every name is insufficient if its definition stops registering tests.
    const DEFINITION: &str = r#"
macro_rules! persona_tests {
    ($($name:ident),+ $(,)?) => {$(
        #[test]
        fn $name() { run_case(stringify!($name)); }
    )+};
}
"#;
    let expected: syn::ItemMacro = syn::parse_str(DEFINITION).unwrap();
    let file = syn::parse_file(source).unwrap();
    let mut definition = 0;
    let mut invocation = 0;
    let mut tests = BTreeSet::new();
    for item in file.items {
        if let Item::Fn(function) = &item {
            assert!(
                !function.attrs.iter().any(|a| a.path().is_ident("test")),
                "unexpected direct test in persona Lua suite: {}",
                function.sig.ident
            );
        }
        if let Item::Macro(mac) = item {
            if mac.mac.path.is_ident("macro_rules")
                && mac
                    .ident
                    .as_ref()
                    .is_some_and(|name| name == "persona_tests")
            {
                assert_eq!(mac.mac.tokens.to_string(), expected.mac.tokens.to_string());
                definition += 1;
            } else if mac.mac.path.is_ident("persona_tests") {
                assert!(mac.attrs.is_empty(), "conditional persona test macro");
                let names = Punctuated::<Ident, Token![,]>::parse_terminated
                    .parse2(mac.mac.tokens)
                    .unwrap();
                for name in names {
                    assert!(tests.insert(name.to_string()), "duplicate persona Lua test");
                }
                invocation += 1;
            }
        }
    }
    assert_eq!(definition, 1);
    assert_eq!(invocation, 1);
    tests
}

#[test]
fn persona_inventory_preserves_all_119_original_methods() {
    let rows: Vec<Parity> = serde_json::from_str(include_str!("persona-parity.json")).unwrap();
    assert_eq!(rows.len(), 119, "d801f97 contains 119 test methods");
    assert_eq!(original_digest(&rows), 0xaef3_e8d6_3c57_b013);

    let mapping = BTreeMap::from([
        (
            "tests/test_leader_persona.py",
            "tools/civ6-data-rs/tests/persona.rs",
        ),
        (
            "tests/test_leader_persona_lua.py",
            "tools/civ6-lua-tests-rs/tests/persona_lua.rs",
        ),
        (
            "tests/test_leader_persona_native_eligibility.py",
            "tools/civ6-lua-tests-rs/tests/persona_native_eligibility.rs",
        ),
        (
            "tests/test_persona_boundary_lua.py",
            "tools/civ6-lua-tests-rs/tests/persona_boundary.rs",
        ),
    ]);
    let expected_counts = BTreeMap::from([
        (
            ("tests/test_leader_persona.py", "LeaderPersonaDatabaseTests"),
            8,
        ),
        (
            ("tests/test_leader_persona.py", "LeaderPersonaStaticTests"),
            6,
        ),
        (
            ("tests/test_leader_persona_lua.py", "LeaderPersonaLuaTests"),
            69,
        ),
        (
            (
                "tests/test_leader_persona_native_eligibility.py",
                "NativeEligibilityDiagnosticTests",
            ),
            12,
        ),
        (
            (
                "tests/test_persona_boundary_lua.py",
                "PersonaBoundaryLuaTests",
            ),
            24,
        ),
    ]);
    let mut actual_counts = BTreeMap::<(&str, &str), usize>::new();
    let mut originals = BTreeSet::new();
    let mut expected_by_file = BTreeMap::<&str, BTreeSet<&str>>::new();
    for row in &rows {
        assert!(row.test.starts_with("test_"));
        assert_eq!(row.rust_test, row.test);
        assert_eq!(
            mapping.get(row.source.as_str()),
            Some(&row.rust_file.as_str())
        );
        assert!(originals.insert((
            row.source.as_str(),
            row.class_name.as_str(),
            row.test.as_str()
        )));
        *actual_counts
            .entry((&row.source, &row.class_name))
            .or_default() += 1;
        assert!(expected_by_file
            .entry(&row.rust_file)
            .or_default()
            .insert(&row.rust_test));
    }
    assert_eq!(actual_counts, expected_counts);

    for (file, expected) in expected_by_file {
        let source = repo_root().join(file);
        let text = std::fs::read_to_string(&source).unwrap();
        let actual: BTreeSet<String> = if file.ends_with("persona_lua.rs") {
            registered_persona_lua(&text)
        } else {
            let registered = inventory::resolve(&source, "persona");
            assert_eq!(registered, inventory::parse(&text, "persona"));
            registered
                .iter()
                .map(|name| name.rsplit("::").next().unwrap().to_owned())
                .collect()
        };
        let expected: BTreeSet<String> = expected.into_iter().map(str::to_owned).collect();
        assert_eq!(actual, expected, "{}", source.display());
    }
}

#[test]
fn persona_inventory_registers_eleven_native_probe_methods_separately() {
    // The historical 119-row correspondence above remains immutable. PR86's
    // test-only candidate has its own fixed inventory and Lua 5.1 fixture.
    let path = repo_root().join("tools/civ6-lua-tests-rs/tests/persona_native_slots.rs");
    let expected: BTreeSet<_> = [
        "test_diagnostic_mods_are_separate_map_scoped_packages",
        "test_official_sql_keeps_production_personas_until_diagnostic_mods_are_loaded",
        "test_all_six_diagnostic_lua_sources_execute_in_lua_51",
        "test_diagnostic_lua_never_publishes_engine_receipts",
        "test_ui_capture_queues_only_scalars_and_gamecore_saves_actual_receipts",
        "test_first_real_turn_requires_three_titles_and_no_ai_spending_or_appointment",
        "test_type_specific_appointment_getter_is_independent_of_persona_ui",
        "test_unassigned_receipt_cannot_be_inferred_from_official_spending_or_diagnostic_property",
        "test_missing_unassigned_snapshot_is_not_rewritten_by_later_mock_activation",
        "test_native_scenarios_wait_for_synchronized_saves_and_keep_receipt_gates",
        "test_city_receipt_requires_real_five_turn_clock_and_reload_equality",
    ]
    .into_iter()
    .map(|name| format!("persona_native_slots::{name}"))
    .collect();
    assert_eq!(inventory::resolve(&path, "persona_native_slots"), expected);
}
