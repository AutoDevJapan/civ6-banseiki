use civ6_autotest_rs::growth_sync::{compare_restoration, Checkpoint};
use std::{fs, process::Command};

const ROWS: &str = "0~65536~2~10~brave~valiant~blade,cloak~relic~2|0~131073~2~10~calm~guard~blade~none~2|1~65536~1~20~careful~wise~~none~1|1~131073~1~20~calm~guard~bow~seal~1";

fn pairs() -> ([Checkpoint; 2], [Checkpoint; 2]) {
    let before = std::array::from_fn(|player| Checkpoint {
        endpoint_id: format!("machine-{player}"),
        session_tag: "session-a".into(),
        barrier_tag: "before-save".into(),
        phase: "before-save".into(),
        observation: format!("2|{player}|{ROWS}"),
    });
    let mut after = before.clone();
    for capture in &mut after {
        capture.barrier_tag = "after-rejoin".into();
        capture.phase = "after-rejoin".into();
    }
    (before, after)
}

#[test]
fn restoration_compares_distinct_stages_without_changing_labels_or_records() {
    let (before, mut after) = pairs();
    after.swap(0, 1);
    let original = (before.clone(), after.clone());
    compare_restoration(&before, &after).unwrap();
    assert_eq!((before, after), original);
}

#[test]
fn changed_barrier_alone_or_changed_phase_alone_identifies_a_new_stage() {
    for change_phase in [false, true] {
        let (before, _) = pairs();
        let mut after = before.clone();
        for capture in &mut after {
            if change_phase {
                capture.phase = "after-load".into();
            } else {
                capture.barrier_tag = "new-boundary".into();
            }
        }
        compare_restoration(&before, &after).unwrap();
    }
}

#[test]
fn matching_peers_with_the_same_duplicate_xp_still_fail_restoration() {
    let (before, mut after) = pairs();
    for capture in &mut after {
        capture.observation = capture.observation.replacen("~2~10~", "~2~30~", 1);
    }
    let error = compare_restoration(&before, &after)
        .unwrap_err()
        .to_string();
    assert!(error.contains("restoration owner=0 unitID=65536"));
}

#[test]
fn rejects_key_metadata_level_or_xp_changes_shared_by_both_peers() {
    for (from, to) in [
        ("0~65536~", "0~65537~"),
        ("~2~10~", "~3~10~"),
        ("~2~10~", "~2~11~"),
        ("~brave~", "~bold~"),
        ("~valiant~", "~guard~"),
        ("~blade,cloak~", "~cloak,blade~"),
        ("~relic~", "~replacement~"),
        ("~relic~2|", "~relic~1|"),
    ] {
        let (before, mut after) = pairs();
        for capture in &mut after {
            capture.observation = capture.observation.replacen(from, to, 1);
        }
        assert!(
            compare_restoration(&before, &after).is_err(),
            "{from} -> {to}"
        );
    }
}

#[test]
fn rejects_other_sessions_players_turns_missing_endpoint_and_same_stage() {
    for change in 0..5 {
        let (before, mut after) = pairs();
        match change {
            0 => {
                for capture in &mut after {
                    capture.session_tag = "another-game".into();
                }
            }
            1 => {
                after[0].observation = after[0].observation.replacen("2|0|", "2|1|", 1);
                after[1].observation = after[1].observation.replacen("2|1|", "2|0|", 1);
            }
            2 => {
                for capture in &mut after {
                    capture.observation = capture.observation.replacen("2|", "3|", 1);
                }
            }
            3 => {
                after[0].endpoint_id = "other-machine".into();
            }
            4 => {
                after = before.clone();
            }
            _ => unreachable!(),
        }
        assert!(
            compare_restoration(&before, &after).is_err(),
            "change={change}"
        );
    }
}

#[test]
fn rejects_incomplete_or_divergent_endpoint_pair_before_comparing_stages() {
    let (before, after) = pairs();
    for old in [true, false] {
        for truncated in [true, false] {
            let (mut a, mut b) = (before.clone(), after.clone());
            let target = if old { &mut a[1] } else { &mut b[1] };
            target.observation = if truncated {
                target
                    .observation
                    .split('|')
                    .take(5)
                    .collect::<Vec<_>>()
                    .join("|")
            } else {
                target.observation.replacen("~relic~", "~diverged~", 1)
            };
            assert!(compare_restoration(&a, &b).is_err());
        }
    }
}

#[test]
fn restoration_cli_preserves_source_files_and_returns_failure_on_double_xp() {
    let root = tempfile::tempdir().unwrap();
    let (before, mut after) = pairs();
    let paths = [
        "before-host.json",
        "before-guest.json",
        "after-host.json",
        "after-guest.json",
    ]
    .map(|name| root.path().join(name));
    for changed in [false, true] {
        if changed {
            for capture in &mut after {
                capture.observation = capture.observation.replacen("~2~10~", "~2~30~", 1);
            }
        }
        let bytes: Vec<_> = before
            .iter()
            .chain(&after)
            .map(|capture| serde_json::to_vec(capture).unwrap())
            .collect();
        for (path, body) in paths.iter().zip(&bytes) {
            fs::write(path, body).unwrap();
        }
        let result = Command::new(env!("CARGO_BIN_EXE_civ6-growth-sync"))
            .arg("compare-restoration")
            .args(&paths)
            .output()
            .unwrap();
        assert_eq!(
            result.status.success(),
            !changed,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        for (path, body) in paths.iter().zip(&bytes) {
            assert_eq!(&fs::read(path).unwrap(), body);
        }
        if changed {
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains("owner=0 unitID=65536"));
        } else {
            assert!(String::from_utf8_lossy(&result.stdout).contains(
                "actual save/load, rejoin and network timing remain separately verifiable"
            ));
        }
    }
}
