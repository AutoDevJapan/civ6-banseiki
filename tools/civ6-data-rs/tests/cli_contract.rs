use std::process::{Command, Output};

fn run(binary: &str, arg: &str) -> Output {
    let mut command = Command::new(binary);
    command.arg(arg).current_dir(env!("CARGO_MANIFEST_DIR"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command.output().expect("run generated CLI")
}

#[test]
fn generators_preserve_check_arguments_exit_codes_and_cargo_help() {
    for (binary, expected) in [
        (
            env!("CARGO_BIN_EXE_generate-sengoku-events"),
            "Sengoku event files current\n",
        ),
        (
            env!("CARGO_BIN_EXE_generate-sengoku-oaths"),
            "oaths: 22 houses, 66 choices; up to date\n",
        ),
        (
            env!("CARGO_BIN_EXE_generate-sengoku-reigns"),
            "Sengoku reign files current\n",
        ),
        (
            env!("CARGO_BIN_EXE_generate-sengoku-trees"),
            "Sengoku tree files current\n",
        ),
    ] {
        for flag in ["--check", "--ch"] {
            let result = run(binary, flag);
            assert!(
                result.status.success(),
                "{binary} {flag}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout)
                    .unwrap()
                    .replace("\r\n", "\n"),
                expected
            );
            assert!(result.stderr.is_empty());
        }
        let help = run(binary, "--hel");
        assert!(help.status.success());
        let help = String::from_utf8(help.stdout).unwrap();
        assert!(help.contains(
            "cargo run --manifest-path tools/Cargo.toml -p civ6-data-rs --bin generate-sengoku-"
        ));
        assert!(!help.contains(".py"));
        assert!(!help.contains("Pillow"));
        let invalid = run(binary, "--invalid");
        assert_eq!(invalid.status.code(), Some(2));
        assert!(invalid.stdout.is_empty());
        assert!(String::from_utf8(invalid.stderr)
            .unwrap()
            .contains("unrecognized arguments: --invalid"));
    }
}

#[test]
fn parity_map_covers_each_baseline_id_once() {
    let parity: serde_json::Value = serde_json::from_str(include_str!("parity.json")).unwrap();
    let rows = parity.as_array().unwrap();
    assert_eq!(rows.len(), 166);
    let mut ids = std::collections::BTreeSet::new();
    let mut rust = std::collections::BTreeSet::new();
    let mut sources = std::collections::BTreeMap::new();
    for row in rows {
        let source = row["source"].as_str().unwrap();
        let class = row["class"].as_str().unwrap();
        let test = row["test"].as_str().unwrap();
        let rust_test = row["rust_test"].as_str().unwrap();
        assert!(ids.insert((source, class, test)));
        assert!(rust.insert(rust_test));
        assert!(rust_test.ends_with(&format!("::{test}")));
        *sources.entry(source).or_insert(0) += 1;
    }
    assert_eq!(
        sources,
        std::collections::BTreeMap::from([
            ("tests/test_city_names.py", 12),
            ("tests/test_historical_events.py", 13),
            ("tests/test_house_names.py", 8),
            ("tests/test_japan_1534_relations.py", 18),
            ("tests/test_japan_no_world_congress.py", 2),
            ("tests/test_localized_names.py", 4),
            ("tests/test_lua_file_names.py", 1),
            ("tests/test_portrait_art.py", 3),
            ("tests/test_resources.py", 12),
            ("tests/test_sengoku_calendar.py", 5),
            ("tests/test_sengoku_cascade.py", 7),
            ("tests/test_sengoku_events.py", 17),
            ("tests/test_sengoku_oaths.py", 40),
            ("tests/test_sengoku_trees.py", 9),
            ("tests/test_unification_victory.py", 15),
        ])
    );
}
