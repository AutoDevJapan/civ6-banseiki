use super::inventory;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

type Id = [String; 3];
const COUNTS: [(&str, usize); 5] = [
    ("civ6-assets-rs", 49),
    ("civ6-autotest-rs", 214),
    ("civ6-data-rs", 166),
    ("civ6-lua-tests-rs", 358),
    ("civ6-reports-rs", 29),
];

// The baseline records declared test_* methods, not just unittest cases. Its
// sole helper is RequirementModel.test_set. Keep its exact ID among the 816;
// it now has a dedicated #[test] of ALL/ANY semantics, not a coverage exemption.
const HELPER: [&str; 3] = [
    "tests/test_unification_victory.py",
    "RequirementModel",
    "test_set",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn string<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key]
        .as_str()
        .filter(|s| !s.is_empty() && s.trim() == *s)
        .ok_or_else(|| format!("missing or invalid {key}: {row}"))
}

fn identity(row: &Value) -> Result<Id, String> {
    Ok([
        string(row, "source")?.into(),
        string(row, "class")?.into(),
        string(row, "test")?.into(),
    ])
}

fn baseline() -> BTreeMap<String, BTreeSet<Id>> {
    let baseline = read_json(&root().join("documents/rust-migration-test-inventory.json")).unwrap();
    assert_eq!(baseline["baseline_commit"], "b4e7c53");
    assert_eq!(baseline["count"], 816);
    let rows = baseline["tests"].as_array().unwrap();
    assert_eq!(
        rows.len(),
        816,
        "independent baseline must retain all declared IDs"
    );
    let mut originals = BTreeMap::<String, BTreeSet<Id>>::new();
    let mut all = BTreeSet::new();
    for row in rows {
        let id = identity(row).unwrap();
        assert!(all.insert(id.clone()), "duplicate baseline ID: {id:?}");
        originals
            .entry(string(row, "crate").unwrap().into())
            .or_default()
            .insert(id);
    }
    assert_eq!(
        originals
            .iter()
            .map(|(name, ids)| (name.as_str(), ids.len()))
            .collect::<Vec<_>>(),
        COUNTS,
        "baseline crate assignments/counts changed"
    );
    assert!(originals["civ6-data-rs"].contains(&HELPER.map(str::to_owned)));
    originals
}

fn path_parts(path: &str) -> Result<Vec<&str>, String> {
    let parts: Vec<_> = path.split("::").collect();
    if parts
        .iter()
        .any(|part| syn::parse_str::<syn::Ident>(part).map_or(true, |id| id != *part))
    {
        return Err(format!("invalid exact Rust test path: {path}"));
    }
    Ok(parts)
}

fn location<'a>(name: &str, row: &'a Value) -> Result<(&'a str, String), String> {
    let rust_test = string(row, "rust_test")?;
    let parts = path_parts(rust_test)?;
    let (target, qualified) = match name {
        // This schema records the Rust name verbatim, which can differ from
        // the legacy name. Never strip prefixes or search for matching leaves.
        "civ6-autotest-rs" => {
            let target = string(row, "rust_target")?;
            if path_parts(target)?.len() != 1 {
                return Err(format!("invalid Cargo test target: {target}"));
            }
            (target, format!("{target}::{rust_test}"))
        }
        "civ6-data-rs" => ("lib", format!("lib::{rust_test}")),
        "civ6-reports-rs" => ("reports", format!("reports::{rust_test}")),
        "civ6-lua-tests-rs" | "civ6-assets-rs" if parts.len() >= 2 => (parts[0], rust_test.into()),
        _ => {
            return Err(format!(
                "unknown crate or missing target: {name}: {rust_test}"
            ))
        }
    };
    if name != "civ6-autotest-rs" && parts.last().copied() != Some(string(row, "test")?) {
        return Err(format!("renamed legacy method: {row}"));
    }
    if identity(row)? == HELPER.map(str::to_owned)
        && (name != "civ6-data-rs" || qualified != "lib::unification::tests::test_set")
    {
        return Err("documented baseline helper must retain its dedicated test".into());
    }
    Ok((target, qualified))
}

fn validate(
    name: &str,
    crate_root: &Path,
    originals: &BTreeSet<Id>,
    rows: &[Value],
) -> Result<(), String> {
    let mut mapped = BTreeSet::new();
    let mut locations = BTreeSet::new();
    for row in rows {
        let id = identity(row)?;
        if !mapped.insert(id.clone()) {
            return Err(format!("{name}: duplicate original mapping: {id:?}"));
        }
        let (_, test) = location(name, row)?;
        if !locations.insert(test.clone()) {
            return Err(format!("{name}: duplicate Rust mapping: {test}"));
        }
    }
    if &mapped != originals {
        return Err(format!(
            "{name}: exact source/class/test set changed; missing {:?}; unexpected {:?}",
            originals.difference(&mapped).collect::<Vec<_>>(),
            mapped.difference(originals).collect::<Vec<_>>()
        ));
    }
    let mut targets = BTreeMap::new();
    for row in rows {
        let (target, test) = location(name, row)?;
        if !targets.contains_key(target) {
            let path = if target == "lib" {
                crate_root.join("src/lib.rs")
            } else {
                crate_root.join("tests").join(format!("{target}.rs"))
            };
            targets.insert(target, inventory::resolve(&path, target)?);
        }
        inventory::require(&targets[target], &test)?;
    }
    Ok(())
}

fn coverage(name: &str) {
    let originals = baseline();
    let crate_root = root().join("tools").join(name);
    let mapping = read_json(&crate_root.join("tests/parity.json")).unwrap();
    validate(
        name,
        &crate_root,
        &originals[name],
        mapping.as_array().unwrap(),
    )
    .unwrap_or_else(|e| panic!("{name}: {e}"));
}

#[test]
fn assets_cover_all_49_declared_methods() {
    coverage("civ6-assets-rs");
}

#[test]
fn driver_covers_all_214_declared_methods() {
    coverage("civ6-autotest-rs");
}

#[test]
fn data_cover_all_166_declared_methods_including_documented_helper() {
    coverage("civ6-data-rs");
}

#[test]
fn lua_covers_all_358_declared_methods() {
    coverage("civ6-lua-tests-rs");
}

#[test]
fn reports_cover_all_29_declared_methods() {
    coverage("civ6-reports-rs");
}

fn python_paths(bytes: &[u8]) -> Vec<&str> {
    std::str::from_utf8(bytes)
        .expect("Git paths must be UTF-8")
        .split('\0')
        .filter(|path| {
            Path::new(path)
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| {
                    ["py", "pyw", "pyi"]
                        .iter()
                        .any(|python| ext.eq_ignore_ascii_case(python))
                })
        })
        .collect()
}

#[test]
fn repository_has_no_tracked_python_sources() {
    let output = Command::new("git")
        .arg("-C")
        .arg(root())
        .args(["ls-files", "-z", "--"])
        .output()
        .expect("inspect Git index");
    assert!(
        output.status.success(),
        "git ls-files: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let remaining = python_paths(&output.stdout);
    assert!(
        remaining.is_empty(),
        "{} tracked Python sources remain:\n{}",
        remaining.len(),
        remaining.join("\n")
    );
}

#[test]
fn inventory_rejects_comments_strings_helpers_wrong_modules_and_suffixes() {
    let tests = inventory::parse(
        r##"
        // #[test] fn test_expected() { assert!(true); }
        const TEXT: &str = "#[test] fn test_expected() { assert!(true); }";
        fn test_expected() { assert!(true); }
        struct Helper;
        impl Helper { fn test_expected() { assert!(true); } }
        mod wrong { #[test] fn test_expected() { assert!(true); } }
        mod right { #[test] fn test_expected_suffix() { assert!(true); } }
        "##,
        "suite",
    )
    .unwrap();
    assert_eq!(
        tests.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "suite::right::test_expected_suffix",
            "suite::wrong::test_expected",
        ]
    );
    for name in [
        "suite::test_expected",
        "suite::right::test_expected",
        "suite::Helper::test_expected",
    ] {
        assert_eq!(
            inventory::require(&tests, name),
            Err(format!("missing #[test] {name}"))
        );
    }
}

#[test]
fn inventory_rejects_empty_ignored_and_skipped_tests() {
    for (source, reason) in [
        (
            "#[test] fn test_one() { /* assertions removed */ }",
            "empty test body",
        ),
        (
            "#[test] #[ignore = \"later\"] fn test_one() { assert!(true); }",
            "ignored",
        ),
        (
            "#[test] #[cfg(any())] fn test_one() { assert!(true); }",
            "conditional",
        ),
        (
            "#[test] #[cfg_attr(test, ignore)] fn test_one() { assert!(true); }",
            "conditional",
        ),
        (
            "#[test] fn test_one() { if unavailable() { return; } assert!(true); }",
            "early return",
        ),
        (
            "#[test] fn test_one() { if cfg!(windows) { assert!(true); } }",
            "runtime cfg",
        ),
        (
            "#[test] fn test_one() { #[cfg(windows)] assert!(true); }",
            "conditional",
        ),
        (
            "#![cfg(any())] #[test] fn test_one() { assert!(true); }",
            "conditional",
        ),
    ] {
        let tests = inventory::parse(source, "suite").unwrap();
        let error = inventory::require(&tests, "suite::test_one").unwrap_err();
        assert!(error.contains(reason), "{source}: {error}");
    }
    let tests = inventory::parse(
        "#[cfg(any())] mod cases { #[test] fn test_one() { assert!(true); } }",
        "suite",
    )
    .unwrap();
    assert!(inventory::require(&tests, "suite::cases::test_one")
        .unwrap_err()
        .contains("conditional"));
}

#[test]
fn cfg_test_is_allowed_without_rejecting_platform_helpers() {
    let tests = inventory::parse(
        r#"
        #[cfg(windows)] fn helper() {}
        #[cfg(not(windows))] fn helper() {}
        #[cfg(test)] mod cases {
            #[test] fn test_one() {
                let callback = || { return 42; };
                assert_eq!(callback(), 42);
            }
        }
        #[cfg(windows)] mod optional { #[test] fn test_one() { assert!(true); } }
    "#,
        "lib",
    )
    .unwrap();
    inventory::require(&tests, "lib::cases::test_one").unwrap();
    assert!(inventory::require(&tests, "lib::optional::test_one")
        .unwrap_err()
        .contains("conditional"));
}

#[test]
fn platform_alternatives_must_execute_exactly_one_valid_body_everywhere() {
    let windows = "#[cfg(windows)] #[test] fn test_one() { assert!(true); }";
    let other = "#[cfg(not(windows))] #[test] fn test_one() { assert!(true); }";
    let tests = inventory::parse(&format!("{windows} {other}"), "suite").unwrap();
    inventory::require(&tests, "suite::test_one").unwrap();
    for source in [
        windows.to_string(),
        other.to_string(),
        format!("{windows} {windows} {other}"),
        format!("{windows} {}", other.replace("#[test]", "#[test] #[ignore]")),
        format!("{windows} {}", other.replace("assert!(true);", "")),
        format!("{windows} {}", other.replace("assert!(true);", "return;")),
        "#[cfg(feature = \"optional\")] #[test] fn test_one() { assert!(true); } #[cfg(not(feature = \"optional\"))] #[test] fn test_one() { assert!(true); }".into(),
    ] {
        let tests = inventory::parse(&source, "suite").unwrap();
        assert!(inventory::require(&tests, "suite::test_one").is_err(), "{source}");
    }
    let tests = inventory::parse(
        "#[cfg(windows)] mod cases { #[test] fn test_one() { assert!(true); } }
         #[cfg(not(windows))] mod cases { #[test] fn test_one() { assert!(true); } }",
        "lib",
    )
    .unwrap();
    inventory::require(&tests, "lib::cases::test_one").unwrap();
}

const MACRO: &str = r#"
macro_rules! simple_tests {
    ($($name:ident),* $(,)?) => {
        $(#[test] fn $name() { Harness::new().case(stringify!($name)); })*
    };
}
"#;

#[test]
fn simple_tests_requires_the_exact_executable_definition_in_scope() {
    let tests = inventory::parse(
        &format!("{MACRO} mod cases {{ simple_tests! {{ test_one, test_two, }} }}"),
        "suite",
    )
    .unwrap();
    assert_eq!(
        tests.keys().map(String::as_str).collect::<Vec<_>>(),
        ["suite::cases::test_one", "suite::cases::test_two"]
    );
    for name in tests.keys() {
        inventory::require(&tests, name).unwrap();
    }
    for source in [
        "simple_tests! { test_one }".to_string(),
        format!("simple_tests! {{ test_one }} {MACRO}"),
        format!("mod hidden {{ {MACRO} }} simple_tests! {{ test_one }}"),
        format!("{} simple_tests! {{ test_one }}", MACRO.replace("#[test]", "")),
        format!("{} simple_tests! {{ test_one }}", MACRO.replace("Harness::new().case(stringify!($name));", "")),
        format!("#[cfg(any())] {MACRO} simple_tests! {{ test_one }}"),
        format!("{MACRO} #[ignore] simple_tests! {{ test_one }}"),
        format!("{MACRO} #[cfg(any())] simple_tests! {{ test_one }}"),
        format!("{MACRO} macro_rules! simple_tests {{ ($name:ident) => {{ fn $name() {{}} }} }} simple_tests! {{ test_one }}"),
    ] {
        let tests = inventory::parse(&source, "suite").unwrap();
        assert!(inventory::require(&tests, "suite::test_one").is_err(), "{source}");
    }
}

#[test]
fn duplicate_syntax_cannot_supply_a_test() {
    for source in [
        "#[test] fn test_one() { assert!(true); } #[test] fn test_one() { assert!(true); }"
            .to_string(),
        format!("{MACRO} simple_tests! {{ test_one, test_one }}"),
    ] {
        let tests = inventory::parse(&source, "suite").unwrap();
        assert!(inventory::require(&tests, "suite::test_one")
            .unwrap_err()
            .contains("duplicate"));
    }
}

fn write(root: &Path, relative: &str, source: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, source).unwrap();
}

#[test]
fn inventory_follows_real_modules_path_attributes_and_file_conditions() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    write(
        root,
        "lib.rs",
        "#[cfg(test)] mod cases; #[path = \"other/real.rs\"] mod renamed;",
    );
    write(
        root,
        "cases.rs",
        "mod nested; mod inline { #[path = \"selected.rs\"] mod chosen; }",
    );
    write(
        root,
        "cases/nested/mod.rs",
        "#[test] fn test_one() { assert!(true); }",
    );
    write(
        root,
        "cases/inline/selected.rs",
        "#[test] fn test_two() { assert!(true); }",
    );
    write(
        root,
        "other/real.rs",
        "#![cfg(any())] #[test] fn test_three() { assert!(true); }",
    );
    write(
        root,
        "orphan.rs",
        "#[test] fn test_orphan() { assert!(true); }",
    );
    let tests = inventory::resolve(&root.join("lib.rs"), "lib").unwrap();
    inventory::require(&tests, "lib::cases::nested::test_one").unwrap();
    inventory::require(&tests, "lib::cases::inline::chosen::test_two").unwrap();
    assert!(inventory::require(&tests, "lib::renamed::test_three")
        .unwrap_err()
        .contains("conditional"));
    assert!(!tests.contains_key("lib::other::real::test_three"));
    assert!(!tests.contains_key("lib::orphan::test_orphan"));
    assert!(!tests.contains_key("lib::nested::test_one"));
}

#[test]
fn regression_explicit_file_path_uses_sibling_children() {
    for folder in ["", "selected/"] {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        write(
            root,
            "lib.rs",
            &format!("#[path = \"{folder}a.rs\"] mod a;"),
        );
        write(root, &format!("{folder}a.rs"), "mod b;");
        write(
            root,
            &format!("{folder}b.rs"),
            "#[test] fn test_actual() { assert!(true); }",
        );
        write(
            root,
            &format!("{folder}a/b.rs"),
            "#[test] fn test_wrong_path() { assert!(true); }",
        );
        let tests = inventory::resolve(&root.join("lib.rs"), "lib").unwrap();
        assert!(inventory::require(&tests, "lib::a::b::test_wrong_path").is_err());
        inventory::require(&tests, "lib::a::b::test_actual").unwrap();
        assert_eq!(
            tests.keys().map(String::as_str).collect::<Vec<_>>(),
            ["lib::a::b::test_actual"]
        );
    }
}

#[test]
fn regression_inline_path_selects_the_child_directory() {
    for outer in [false, true] {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        let source = "#[path = \"selected\"] mod a { mod b; }";
        if outer {
            write(root, "lib.rs", "mod outer;");
            write(root, "outer.rs", source);
        } else {
            write(root, "lib.rs", source);
        }
        write(
            root,
            "selected/b.rs",
            "#[test] fn test_actual() { assert!(true); }",
        );
        let wrong = if outer { "outer/a/b.rs" } else { "a/b.rs" };
        write(
            root,
            wrong,
            "#[test] fn test_wrong_path() { assert!(true); }",
        );
        let tests = inventory::resolve(&root.join("lib.rs"), "lib").unwrap();
        let prefix = if outer {
            "lib::outer::a::b"
        } else {
            "lib::a::b"
        };
        assert!(inventory::require(&tests, &format!("{prefix}::test_wrong_path")).is_err());
        inventory::require(&tests, &format!("{prefix}::test_actual")).unwrap();
        assert_eq!(tests.len(), 1);
    }
}

#[test]
fn regression_macro_arguments_cannot_hide_early_returns_or_cfg() {
    for (body, reason) in [
        (
            "assert!({ if unavailable() { return; } true });",
            "early return",
        ),
        (
            "assert_eq!(0, { if unavailable() { return; } 0 });",
            "early return",
        ),
        (
            "assert!(matches!(Some(1), Some(_) if { if unavailable() { return; } true }));",
            "early return",
        ),
        (
            "assert!(matches!(Some(1), Some(_) if cfg!(windows)));",
            "runtime cfg",
        ),
        (
            "assert!({ #[cfg(windows)] let result = true; result });",
            "conditional",
        ),
        (
            "opaque! { [ { if unavailable() { return; } } ] ; };",
            "early return",
        ),
        ("opaque! { [ { std::cfg!(windows) } ] ; };", "runtime cfg"),
    ] {
        let source = format!("#[test] fn test_one() {{ {body} }}");
        let tests = inventory::parse(&source, "suite").unwrap();
        let error = inventory::require(&tests, "suite::test_one").unwrap_err();
        assert!(error.contains(reason), "{source}: {error}");
    }
}

#[test]
fn regression_macro_literals_and_callback_returns_remain_valid() {
    let source = r##"
        #[test] fn test_one() {
            assert_eq!((|| { return 42; })(), 42);
            assert_eq!("return cfg!(windows)", r#"return cfg!(windows)"#);
            assert!(matches!(Some(1), Some(_) if true));
            assert!(true, "return; cfg!(windows)");
            opaque! { ["return", r#"cfg!(windows)"#, 'r']; };
            // assert!({ return; });
        }
    "##;
    let tests = inventory::parse(source, "suite").unwrap();
    inventory::require(&tests, "suite::test_one").unwrap();
}

#[test]
fn regression_macro_imports_and_exports_cannot_shadow_verified_tests() {
    let shadow = "macro_rules! simple_tests { ($($name:ident),* $(,)?) => {}; }";
    for imported in [
        format!("#[macro_use] mod shadow {{ {shadow} }}"),
        format!("mod shadow {{ #![macro_use] {shadow} }}"),
        format!("mod shadow {{ #[macro_export] {shadow} }}"),
        "#[macro_use] extern crate unknown;".into(),
        "#[macro_use(simple_tests)] extern crate unknown;".into(),
    ] {
        let source = format!("{MACRO} {imported} simple_tests! {{ test_one }}");
        let error = inventory::parse(&source, "suite")
            .err()
            .expect("unsupported macro scope must be rejected");
        assert!(error.contains("unsupported macro"), "{source}: {error}");
    }
    let fixture = tempfile::tempdir().unwrap();
    write(
        fixture.path(),
        "lib.rs",
        &format!("{MACRO} #[macro_use] mod shadow; simple_tests! {{ test_one }}"),
    );
    write(fixture.path(), "shadow.rs", shadow);
    assert!(inventory::resolve(&fixture.path().join("lib.rs"), "lib")
        .err()
        .expect("external macro import must be rejected")
        .contains("unsupported macro"));
}

#[test]
fn regression_qualified_simple_tests_never_borrow_textual_verification() {
    for path in [
        "crate::simple_tests",
        "self::simple_tests",
        "other::simple_tests",
        "::other::simple_tests",
    ] {
        let source = format!("{MACRO} {path}! {{ test_one }}");
        match inventory::parse(&source, "suite") {
            Ok(tests) => assert!(inventory::require(&tests, "suite::test_one").is_err()),
            Err(error) => assert!(error.contains("unsupported macro")),
        }
    }
}

fn row(class: &str, test: &str, rust_test: &str) -> Value {
    json!({"source": "tests/old.py", "class": class, "test": test, "rust_test": rust_test})
}

#[test]
fn mappings_require_exact_independent_ids_and_distinct_rust_tests() {
    let fixture = tempfile::tempdir().unwrap();
    write(
        fixture.path(),
        "tests/suite.rs",
        "#[test] fn test_one() { assert!(true); } #[test] fn test_two() { assert!(true); }",
    );
    let rows = [
        row("First", "test_one", "suite::test_one"),
        row("Second", "test_two", "suite::test_two"),
    ];
    let originals: BTreeSet<_> = rows.iter().map(|r| identity(r).unwrap()).collect();
    validate("civ6-assets-rs", fixture.path(), &originals, &rows).unwrap();
    for key in ["source", "class", "test"] {
        let mut changed = rows.clone();
        changed[0][key] = json!("changed");
        if key == "test" {
            changed[0]["rust_test"] = json!("suite::changed");
        }
        assert!(
            validate("civ6-assets-rs", fixture.path(), &originals, &changed)
                .unwrap_err()
                .contains("exact source/class/test set")
        );
    }
    assert!(
        validate("civ6-assets-rs", fixture.path(), &originals, &rows[..1])
            .unwrap_err()
            .contains("missing")
    );
    assert!(validate(
        "civ6-assets-rs",
        fixture.path(),
        &originals,
        &[rows[0].clone(), rows[0].clone()]
    )
    .unwrap_err()
    .contains("duplicate original"));
    let duplicate = [
        row("First", "test_one", "suite::test_one"),
        json!({
            "source": "tests/old.py", "class": "Second", "test": "test_one", "rust_test": "suite::test_one",
            "grouped": true, "coverage_notes": "Cannot exempt duplicate mappings"
        }),
    ];
    let expected = duplicate.iter().map(|r| identity(r).unwrap()).collect();
    assert!(
        validate("civ6-assets-rs", fixture.path(), &expected, &duplicate)
            .unwrap_err()
            .contains("duplicate Rust mapping")
    );
    let mut wrong_module = rows.clone();
    wrong_module[0]["rust_test"] = json!("suite::wrong::test_one");
    assert!(
        validate("civ6-assets-rs", fixture.path(), &originals, &wrong_module)
            .unwrap_err()
            .contains("missing #[test] suite::wrong::test_one")
    );
}

#[test]
fn driver_rust_target_and_library_module_names_are_exact() {
    let fixture = tempfile::tempdir().unwrap();
    write(
        fixture.path(),
        "tests/scenario.rs",
        "#[path = \"fixtures/cases.rs\"] mod cases;",
    );
    write(
        fixture.path(),
        "tests/fixtures/cases.rs",
        "#[test] fn actual_name() { assert!(true); }",
    );
    write(
        fixture.path(),
        "tests/wrong.rs",
        "#[test] fn other_name() { assert!(true); }",
    );
    write(
        fixture.path(),
        "src/lib.rs",
        "#[cfg(windows)] fn helper() {} pub mod tuner;",
    );
    write(
        fixture.path(),
        "src/tuner.rs",
        "#[cfg(test)] mod tests { #[test] fn actual_name() { assert!(true); } }",
    );
    let mut mapped = row("Legacy", "test_old_name", "cases::actual_name");
    mapped["rust_target"] = json!("scenario");
    let expected = BTreeSet::from([identity(&mapped).unwrap()]);
    validate(
        "civ6-autotest-rs",
        fixture.path(),
        &expected,
        &[mapped.clone()],
    )
    .unwrap();
    for (target, test) in [
        ("wrong", "cases::actual_name"),
        ("scenario", "actual_name"),
        ("scenario", "cases::actual_name_suffix"),
    ] {
        let mut forged = mapped.clone();
        forged["rust_target"] = json!(target);
        forged["rust_test"] = json!(test);
        assert!(
            validate("civ6-autotest-rs", fixture.path(), &expected, &[forged])
                .unwrap_err()
                .contains("missing #[test]")
        );
    }
    mapped["rust_target"] = json!("lib");
    mapped["rust_test"] = json!("tuner::tests::actual_name");
    validate(
        "civ6-autotest-rs",
        fixture.path(),
        &expected,
        &[mapped.clone()],
    )
    .unwrap();
    mapped["rust_test"] = json!("tests::actual_name");
    assert!(
        validate("civ6-autotest-rs", fixture.path(), &expected, &[mapped])
            .unwrap_err()
            .contains("missing #[test] lib::tests::actual_name")
    );
}

#[test]
fn documented_baseline_helper_still_requires_a_real_test() {
    let fixture = tempfile::tempdir().unwrap();
    let mapped = json!({"source": HELPER[0], "class": HELPER[1], "test": HELPER[2], "rust_test": "unification::tests::test_set"});
    let expected = BTreeSet::from([HELPER.map(str::to_owned)]);
    write(fixture.path(), "src/lib.rs", "mod unification;");
    write(
        fixture.path(),
        "src/unification.rs",
        "#[cfg(test)] mod tests { fn test_set() { assert!(true); } }",
    );
    assert!(validate(
        "civ6-data-rs",
        fixture.path(),
        &expected,
        std::slice::from_ref(&mapped)
    )
    .unwrap_err()
    .contains("missing #[test]"));
    write(
        fixture.path(),
        "src/unification.rs",
        "#[cfg(test)] mod tests { #[test] fn test_set() { assert!(true); } }",
    );
    validate(
        "civ6-data-rs",
        fixture.path(),
        &expected,
        std::slice::from_ref(&mapped),
    )
    .unwrap();
    let mut renamed = mapped;
    renamed["rust_test"] = json!("other::tests::test_set");
    assert_eq!(
        validate("civ6-data-rs", fixture.path(), &expected, &[renamed]),
        Err("documented baseline helper must retain its dedicated test".into())
    );
}

#[test]
fn python_inventory_handles_nested_paths_uppercase_and_spaces() {
    assert_eq!(
        python_paths(b"src/ok.rs\0nested/test.PY\0script.pyw\0space dir/types.PyI\0notes.py.md\0"),
        ["nested/test.PY", "script.pyw", "space dir/types.PyI",]
    );
    assert!(python_paths(b"src/lib.rs\0documents/original.py.json\0").is_empty());
}
