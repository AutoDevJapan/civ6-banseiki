mod support_driver;
use civ6_autotest_rs::{
    judge,
    metrics::{SaveLoadMetrics, TurnMetrics},
    scenario,
};
use serde_json::json;
use std::fs::{self, File};
use support_driver::{FakeBackend, Harness};

#[test]
fn log_replacement_keeps_bytes_and_identity_on_the_same_handle() {
    let root = tempfile::tempdir().unwrap();
    let log = root.path().join("Lua.log");
    let old = b"same initial bytes\n";
    fs::write(&log, old).unwrap();
    let old_inode = judge::file_inode(&log).unwrap();
    let mut opened = File::open(&log).unwrap();
    fs::rename(&log, root.path().join("old.log")).unwrap();
    fs::write(&log, [old.as_slice(), b"new run\n"].concat()).unwrap();
    let new_inode = judge::file_inode(&log).unwrap();
    assert_ne!(old_inode, new_inode);
    let (bytes, inode) = judge::read_log_handle(&mut opened).unwrap();
    assert_eq!(bytes, old);
    assert_eq!(inode, old_inode);
    assert_eq!(judge::file_inode_from_handle(&opened).unwrap(), old_inode);
    let marker = json!({"offset":old.len(),"inode":old_inode,"prefix_length":old.len(),"prefix_sha256":null,"tail_length":0,"tail_sha256":null});
    let (contents, new_id) = judge::read_log_file(&log).unwrap();
    assert_eq!(new_id, new_inode);
    assert!(judge::log_marker_replaced(&contents, &marker, new_id));
    assert_eq!(
        judge::slice_log_bytes(&contents, &marker, new_id),
        "same initial bytes\nnew run\n"
    );
}

#[test]
fn null_equals_keeps_the_original_none_conversion() {
    let mut h = Harness::new(json!({}), FakeBackend::new().lua(|_, _| Ok("none".into())));
    h.runner
        .wait(
            &json!({"type":"lua","state_name":"InGame","code":"return 'none'","equals":null}),
            1.0,
            false,
        )
        .unwrap();
    assert_eq!(h.calls().len(), 1);
    let mut h = Harness::new(json!({}), FakeBackend::new().lua(|_, _| Ok("null".into())));
    assert!(h
        .runner
        .wait(
            &json!({"type":"lua","state_name":"InGame","code":"return 'null'","equals":null}),
            1.0,
            false,
        )
        .is_err());
}

#[test]
fn structured_equals_preserves_python_quotes_booleans_and_member_order() {
    for (condition, reply) in [
        (
            json!([null, true, false, "can't", "line\nbreak"]),
            "[None, True, False, \"can't\", 'line\\nbreak']",
        ),
        (
            serde_json::from_str(r#"{"z":null,"a":true}"#).unwrap(),
            "{'z': None, 'a': True}",
        ),
    ] {
        let mut h = Harness::new(
            json!({}),
            FakeBackend::new().lua(move |_, _| Ok(reply.to_lowercase())),
        );
        h.runner.wait(&json!({"type":"lua","state_name":"InGame","code":"return value","equals":condition}), 1.0, false).unwrap();
        assert_eq!(h.calls().len(), 1);
    }
}

#[test]
fn numeric_equals_keeps_python_float_exponents_and_signed_zero() {
    for (condition, reply) in [
        (json!(1), "1"),
        (json!(1.0), "1.0"),
        (json!(-0.0), "-0.0"),
        (json!(1e-5), "1e-05"),
        (json!(1e-4), "0.0001"),
        (json!(1e15), "1000000000000000.0"),
        (json!(1e16), "1e+16"),
        (json!([1e-5, -0.0]), "[1e-05, -0.0]"),
    ] {
        let mut h = Harness::new(
            json!({}),
            FakeBackend::new().lua(move |_, _| Ok(reply.into())),
        );
        h.runner.wait(&json!({"type":"lua","state_name":"InGame","code":"return value","equals":condition}), 1.0, false).unwrap();
        assert_eq!(h.calls().len(), 1);
    }
}

#[test]
fn directory_only_screenshot_paths_fail_validation() {
    for path in [".", "./", ".\\", "././"] {
        assert!(scenario::safe_relative_path(path).is_err(), "{path}");
        let step = json!({"action":"screenshot","path":path});
        assert!(scenario::validate_scenario(
            &json!({"name":"invalid path","steps":[step.clone()]})
        )
        .is_err());
        let mut h = Harness::new(json!({}), FakeBackend::new());
        assert!(h.runner.run_step(&step).is_err());
        assert!(h.calls().is_empty());
        assert!(h.events().is_empty());
    }
    assert!(scenario::safe_relative_path("images/./shot.png").is_ok());
}

#[test]
fn measurement_rounding_matches_decimal_binary_float_boundaries() {
    let root = tempfile::tempdir().unwrap();
    let mut saves = SaveLoadMetrics::new(root.path().join("saves.csv"), false);
    for (value, expected) in [
        (0.0005, 0.001),
        (-0.0005, -0.001),
        (1.2345, 1.234),
        (1.2355, 1.236),
        (0.0625, 0.062),
        (0.1875, 0.188),
        (f64::from_bits(0.0625_f64.to_bits() - 1), 0.062),
        (f64::from_bits(0.0625_f64.to_bits() + 1), 0.063),
        (-0.0625, -0.062),
        (-0.1875, -0.188),
        (f64::from_bits((-0.0625_f64).to_bits() - 1), -0.062),
        (f64::from_bits((-0.0625_f64).to_bits() + 1), -0.063),
        (1.0e300, 1.0e300),
    ] {
        assert_eq!(
            saves.record("load", "sample", 1, 100, value).unwrap()["duration_seconds"],
            json!(expected)
        );
    }
    let negative_zero = saves.record("load", "sample", 1, 100, -0.0004).unwrap();
    assert_eq!(
        negative_zero["duration_seconds"]
            .as_f64()
            .unwrap()
            .to_bits(),
        (-0.0_f64).to_bits()
    );
    let mut turns = TurnMetrics::new(root.path().join("turns.jsonl"), 1, 0.0, false);
    turns
        .start(json!({"working_set_bytes":1,"private_bytes":1}))
        .unwrap();
    turns
        .sample_at(2, json!({"working_set_bytes":2,"private_bytes":2}), 0.0625)
        .unwrap();
    assert_eq!(turns.rows[1]["duration_seconds"], 0.062);
    let bytes = fs::read(root.path().join("turns.csv")).unwrap();
    assert!(bytes.windows(2).any(|b| b == b"\r\n"));
}

#[test]
fn save_and_load_measurements_follow_the_runtime_test_mode_flag() {
    for has_metadata in [false, true] {
        for runtime_flag in [false, true] {
            let data = if has_metadata {
                json!({"test_mode":{"skipped_turns":0,"continuous_performance_valid":false}})
            } else {
                json!({})
            };
            let mut h = Harness::new(data, FakeBackend::new());
            h.runner.test_mode = runtime_flag;
            for name in judge::LOG_NAMES {
                fs::write(h.runner.logs.join(name), "").unwrap();
            }
            h.runner.baseline = Some(judge::snapshot_logs(&h.runner.logs).unwrap());
            let path = h.runner.saves.join("flag.Civ6Save");
            let log = h.runner.logs.join("Lua.log");
            h.runner.backend = Box::new(
                FakeBackend::new()
                    .states(|| {
                        Ok(vec![
                            "InGame".into(),
                            "FrontEnd".into(),
                            "LoadScreen".into(),
                        ])
                    })
                    .lua(move |_, code| {
                        if code.contains("Game.GetCurrentGameTurn()") {
                            return Ok("1".into());
                        }
                        if code.contains("Network.SaveGame") {
                            fs::write(&path, b"save bytes").unwrap();
                        }
                        if code.contains("Network.LoadGame") {
                            fs::write(&log, "LoadScreen: OnLoadGameViewStateDone\n").unwrap();
                        }
                        Ok("true".into())
                    }),
            );
            for action in ["save", "load"] {
                h.runner
                    .run_step(
                        &json!({"action":action,"name":"flag","exact_name":true,"metrics":true}),
                    )
                    .unwrap();
            }
            assert_eq!(h.runner.save_load_metrics.rows.len(), 2);
            for row in &h.runner.save_load_metrics.rows {
                assert_eq!(row["test_mode"], runtime_flag);
                assert_eq!(row["continuous_performance_valid"], !runtime_flag);
            }
            let mut csv =
                csv::Reader::from_path(h.runner.output.join("save-load-metrics.csv")).unwrap();
            let rows: Vec<_> = csv.records().map(|row| row.unwrap()).collect();
            assert_eq!(rows.len(), 2);
            for row in rows {
                assert_eq!(&row[6], if runtime_flag { "True" } else { "False" });
                assert_eq!(&row[7], if runtime_flag { "False" } else { "True" });
            }
        }
    }
}
