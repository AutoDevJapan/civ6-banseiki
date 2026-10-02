use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
#[path = "support/inventory.rs"]
mod inventory;

#[derive(Deserialize)]
struct Parity {
    source: String,
    #[serde(rename = "class")]
    class_name: String,
    test: String,
    rust_test: String,
}

#[test]
fn parity_inventory_preserves_all_358_original_methods() {
    let rows: Vec<Parity> = serde_json::from_str(include_str!("parity.json")).unwrap();
    assert_eq!(rows.len(), 358);
    let expected = BTreeMap::from([
        ("tests/test_assassin_lua.py", 87),
        ("tests/test_court_arbitration.py", 52),
        ("tests/test_religion_persistence.py", 71),
        ("tests/test_sengoku_ages.py", 63),
        ("tests/test_sengoku_ages_screens.py", 31),
        ("tests/test_house_names_lua.py", 7),
        ("tests/test_historical_events_lua.py", 18),
        ("tests/test_japan_1534_relations_lua.py", 4),
        ("tests/test_unification_pact_lua.py", 25),
    ]);
    let mut counts = BTreeMap::<&str, usize>::new();
    let mut originals = BTreeSet::new();
    let mut rust_tests = BTreeSet::new();
    let mut registered = BTreeMap::new();
    for row in &rows {
        assert!(originals.insert((&row.source, &row.class_name, &row.test)));
        assert!(
            rust_tests.insert(&row.rust_test),
            "combined test {}",
            row.rust_test
        );
        *counts.entry(&row.source).or_default() += 1;
        let (target, _) = row.rust_test.split_once("::").unwrap();
        assert_eq!(row.rust_test.rsplit("::").next(), Some(row.test.as_str()));
        assert!(!row.class_name.is_empty());
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(format!("{target}.rs"));
        let tests = registered
            .entry(target.to_string())
            .or_insert_with(|| inventory::resolve(&path, target));
        assert!(
            tests.contains(&row.rust_test),
            "missing Rust test {}",
            row.rust_test
        );
    }
    assert_eq!(counts, expected);
}

#[test]
fn inventory_rejects_comments_strings_helpers_and_wrong_modules() {
    let tests = inventory::parse(
        r##"
        // #[test] fn test_expected() {}
        const TEXT: &str = "#[test] fn test_expected() {}";
        fn test_expected() {}
        mod wrong { #[test] fn test_expected() {} }
        mod right { #[test] fn test_actual() {} }
    "##,
        "suite",
    );
    assert_eq!(
        tests,
        BTreeSet::from([
            "suite::wrong::test_expected".into(),
            "suite::right::test_actual".into()
        ])
    );
    assert!(!tests.contains("suite::right::test_expected"));
}

#[test]
fn inventory_parses_macro_ids_and_rejects_disabled_tests() {
    let tests = inventory::parse(
        "mod cases { simple_tests! { test_one, test_two, } }",
        "suite",
    );
    assert_eq!(
        tests,
        BTreeSet::from([
            "suite::cases::test_one".into(),
            "suite::cases::test_two".into()
        ])
    );
    for source in [
        "#[test] #[ignore] fn test_one() {}",
        "#[test] #[cfg(any())] fn test_one() {}",
        "#[cfg(any())] simple_tests! { test_one }",
    ] {
        assert!(std::panic::catch_unwind(|| inventory::parse(source, "suite")).is_err());
    }
}

#[test]
fn assertion_suites_reject_coercing_mlua_decoders_in_rust_syntax() {
    use syn::visit::Visit;
    struct Boundary;
    impl<'ast> Visit<'ast> for Boundary {
        fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
            let method = call.method.to_string();
            let types: Vec<&syn::Type> = call
                .turbofish
                .iter()
                .flat_map(|a| &a.args)
                .filter_map(|arg| {
                    if let syn::GenericArgument::Type(t) = arg {
                        Some(t)
                    } else {
                        None
                    }
                })
                .collect();
            let raw_value = |t: &syn::Type, expected: &str| matches!(t, syn::Type::Path(p) if p.path.segments.last().unwrap().ident == expected);
            let inferred_chunk_call = matches!(&*call.receiver, syn::Expr::MethodCall(receiver) if receiver.method == "load");
            match method.as_str() {
                "eval" | "call" if !types.is_empty() => {
                    assert!(
                        types.iter().all(|t| raw_value(t, "MultiValue")
                            || matches!(t,syn::Type::Tuple(tuple) if tuple.elems.is_empty())),
                        "coercing mlua {method} decoder"
                    );
                }
                "eval" | "call" if inferred_chunk_call => {
                    panic!("inferred mlua {method} decoder")
                }
                "get" | "raw_get" if types.len() == 1 => {
                    assert!(
                        !["bool", "i64", "f64", "String"]
                            .iter()
                            .any(|name| raw_value(types[0], name)),
                        "coercing table field decoder"
                    );
                }
                "pairs" => assert!(
                    types.iter().all(|t| raw_value(t, "Value")),
                    "coercing table key/value iterator"
                ),
                "sequence_values" => panic!("coercing/truncating sequence iterator"),
                _ => {}
            }
            syn::visit::visit_expr_method_call(self, call);
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    for name in [
        "ages",
        "ages_screens",
        "assassin",
        "court",
        "historical",
        "house",
        "relations",
        "religion",
        "unification",
        "ages_support/mod",
    ] {
        let source = std::fs::read_to_string(root.join(format!("{name}.rs"))).unwrap();
        Boundary.visit_file(&syn::parse_file(&source).unwrap());
    }
}
