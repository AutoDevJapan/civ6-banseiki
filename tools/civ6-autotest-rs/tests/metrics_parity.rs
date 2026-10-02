use civ6_autotest_rs::metrics::{
    parse_research_sample, process_memory, research_sample_code, summarize_turns, ResearchMetrics,
    SaveLoadMetrics, TurnMetrics, RESEARCH_SAMPLE_CODE,
};
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, path::Path};
#[path = "../src/research_report.rs"]
mod research_report;

fn memory(working: u64, private: u64) -> Value {
    json!({"working_set_bytes":working,"private_bytes":private})
}
fn csv_rows(path: &Path) -> Vec<csv::StringRecord> {
    csv::Reader::from_path(path)
        .unwrap()
        .records()
        .map(Result::unwrap)
        .collect()
}
fn json_rows(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn fast_forward_uses_core_clock_and_discards_stale_ui_measurement() {
    let dir = tempfile::tempdir().unwrap();
    let mut turns = TurnMetrics::new(dir.path().join("turns.jsonl"), 95, 10.0, true);
    turns.start(memory(80, 100)).unwrap();
    turns.sample_at(96, memory(81, 101), 11.0).unwrap();
    turns.sample_at(97, memory(82, 102), 12.0).unwrap();
    assert_eq!(
        turns
            .rows
            .iter()
            .map(|r| r["turn"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [95, 96, 97]
    );
    let mut research = ResearchMetrics::new(dir.path().join("research.jsonl"));
    for turn in [95, 96, 97] {
        research
            .sample(&format!("{turn}|0~LEADER_A~true~0~0~~~~"))
            .unwrap();
    }
    assert_eq!(
        json_rows(&research.path)
            .iter()
            .map(|r| r["turn"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [95, 96, 97]
    );
}
#[test]
fn fast_forward_csv_files_warn_against_continuous_performance_comparison() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();
    let mut save = SaveLoadMetrics::new(base.join("save-load-metrics.csv"), true);
    save.record("save", "synthetic-turn-51", 51, 400, 2.0)
        .unwrap();
    let mut turns = TurnMetrics::new(base.join("turns.jsonl"), 49, 10.0, true);
    turns.start(memory(80, 100)).unwrap();
    turns.sample_at(50, memory(85, 110), 11.0).unwrap();
    for path in [&save.path, &turns.csv_path] {
        let mut reader = csv::Reader::from_path(path).unwrap();
        let headers = reader.headers().unwrap().clone();
        let rows: Vec<_> = reader.records().map(Result::unwrap).collect();
        assert!(!rows.is_empty());
        for row in rows {
            assert_eq!(
                &row[headers.iter().position(|h| h == "test_mode").unwrap()],
                "True"
            );
            assert_eq!(
                &row[headers
                    .iter()
                    .position(|h| h == "continuous_performance_valid")
                    .unwrap()],
                "False"
            );
        }
    }
}
#[test]
fn csv_and_summary_keep_skipped_turns_and_untimed_intervals_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = TurnMetrics::new(dir.path().join("turns.jsonl"), 1, 10.0, false);
    m.start(memory(80, 100)).unwrap();
    m.sample_at(2, memory(85, 110), 11.5).unwrap();
    m.sample_at(4, memory(90, 120), 13.0).unwrap();
    m.sample_at(5, memory(95, 140), 16.0).unwrap();
    m.sample_at(5, memory(999, 999), 16.0).unwrap();
    let rows = csv_rows(&m.csv_path);
    assert_eq!(
        rows.iter().map(|r| r.get(2).unwrap()).collect::<Vec<_>>(),
        ["1", "2", "3", "4", "5"]
    );
    assert_eq!(
        rows.iter().map(|r| r.get(4).unwrap()).collect::<Vec<_>>(),
        ["", "1.5", "", "", "3.0"]
    );
    assert_eq!(
        rows.iter().map(|r| r.get(7).unwrap()).collect::<Vec<_>>(),
        ["100", "110", "", "120", "140"]
    );
    assert_eq!(rows[2].get(3), Some("False"));
    let summary = m.summary();
    assert_eq!(summary["observed_turns"], 3);
    assert_eq!(summary["missed_turns"], 1);
    assert_eq!(
        summary["duration_seconds"],
        json!({"count":2,"min":1.5,"median":2.25,"p95":3.0,"max":3.0})
    );
    assert_eq!(
        summary["private_bytes"],
        json!({"count":4,"min":100,"median":115.0,"p95":140,"max":140,"first":100,"last":140,"growth":40})
    );
    assert!(m
        .sample_at(4, memory(0, 0), 17.0)
        .unwrap_err()
        .to_string()
        .contains("backwards"));
}
#[test]
fn first_interval_starts_before_autoplay_response() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = TurnMetrics::new(dir.path().join("turns.jsonl"), 1, 8.0, false);
    m.start(memory(10, 20)).unwrap();
    m.sample_at(2, memory(11, 21), 12.0).unwrap();
    assert_eq!(m.rows[1]["duration_seconds"], 4.0);
}
#[test]
fn save_load_measure_actual_file_and_wait_until_ready() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = SaveLoadMetrics::new(dir.path().join("save-load-metrics.csv"), false);
    m.record("save", "checkpoint", 21, 400, 2.0).unwrap();
    m.record("load", "checkpoint", 21, 400, 2.5).unwrap();
    let rows = csv_rows(&m.path);
    assert_eq!(
        rows.iter().map(|r| r.get(1).unwrap()).collect::<Vec<_>>(),
        ["save", "load"]
    );
    assert_eq!(
        rows.iter().map(|r| r.get(3).unwrap()).collect::<Vec<_>>(),
        ["21", "21"]
    );
    assert_eq!(
        rows.iter().map(|r| r.get(4).unwrap()).collect::<Vec<_>>(),
        ["400", "400"]
    );
    assert_eq!(
        rows.iter().map(|r| r.get(5).unwrap()).collect::<Vec<_>>(),
        ["2.0", "2.5"]
    );
    assert_eq!(m.summary()["load_seconds"]["median"], 2.5);
}
#[test]
fn result_separates_continuous_memory_from_post_load_memory() {
    let dir = tempfile::tempdir().unwrap();
    let mut continuous = TurnMetrics::new(dir.path().join("continuous.jsonl"), 1, 10.0, false);
    continuous.start(memory(80, 100)).unwrap();
    continuous.sample_at(2, memory(400, 500), 12.0).unwrap();
    let mut post = TurnMetrics::new(dir.path().join("post-load.jsonl"), 301, 12.0, false);
    post.start(memory(40, 50)).unwrap();
    post.sample_at(302, memory(45, 60), 13.0).unwrap();
    assert_eq!(continuous.summary()["private_bytes"]["growth"], 400);
    assert_eq!(post.summary()["private_bytes"]["growth"], 10);
    let mut combined = continuous.rows.clone();
    combined.extend(post.rows);
    assert_eq!(summarize_turns(&combined)["private_bytes"]["growth"], -40);
}
#[test]
fn research_samples_and_report_use_first_observed_completion() {
    assert!(RESEARCH_SAMPLE_CODE.contains("research:GetResearchingTech()"));
    assert!(RESEARCH_SAMPLE_CODE.contains("culture:GetProgressingCivic()"));
    assert!(
        research_sample_code(&[0, 1].into_iter().collect::<BTreeSet<_>>())
            .unwrap()
            .contains("local configured = {0,1}")
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("research.jsonl");
    let mut m = ResearchMetrics::new(path.clone());
    let samples = [
        "1|0~LEADER_A~true~0~0~~~~;1~LEADER_B~true~0~0~~~~",
        "3|0~LEADER_A~true~1~1~TECH_B~CIVIC_B~TECH_A~CIVIC_A;1~LEADER_B~false~0~0~~~~",
        "5|0~LEADER_A~true~1~1~TECH_B~CIVIC_B~TECH_A~CIVIC_A;1~LEADER_B~false~1~0~~~TECH_A~",
    ];
    for (sample, turn) in samples.into_iter().zip([1, 3, 5]) {
        assert_eq!(m.sample(sample).unwrap(), turn);
    }
    let rows = json_rows(&path);
    assert_eq!(rows.len(), 6);
    assert_eq!(
        rows.iter()
            .map(|r| r["turn"].as_i64().unwrap())
            .collect::<BTreeSet<_>>(),
        [1, 3, 5].into_iter().collect()
    );
    assert_eq!(rows[2]["current_tech"], "TECH_B");
    assert_eq!(rows[3]["alive"], false);
    assert_eq!(
        research_report::summarize(&path, "tech").unwrap(),
        vec![
            json!({"id":"TECH_A","factions":2,"median_turn":4.0,"min_turn":3,"max_turn":5,"spread_turns":2,"baseline_factions":0})
        ]
    );
    assert_eq!(
        research_report::summarize(&path, "civic").unwrap()[0]["id"],
        "CIVIC_A"
    );
    assert!(m
        .sample("6|0~LEADER_A~true~1~1~~~TECH_A~CIVIC_A")
        .unwrap_err()
        .to_string()
        .contains("Major slots changed"));
    assert!(parse_research_sample("2|0~LEADER_A~true~2~0~~~TECH_A~")
        .unwrap_err()
        .to_string()
        .contains("Invalid research sample"));
}
#[test]
fn research_sampling_deduplicates_turn_and_rejects_backward_or_bad_ids() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = ResearchMetrics::new(dir.path().join("research.jsonl"));
    assert_eq!(m.sample("1|0~LEADER_A~true~0~0~~~~").unwrap(), 1);
    assert_eq!(m.sample("1|0~LEADER_A~true~0~0~~~~").unwrap(), 1);
    assert_eq!(json_rows(&m.path).len(), 1);
    assert!(m
        .sample("0|0~LEADER_A~true~0~0~~~~")
        .unwrap_err()
        .to_string()
        .contains("backwards"));
    assert!(parse_research_sample("1|0~leader_a~true~0~0~~~~").is_err());
    assert!(parse_research_sample("1|0~LEADER_A~true~0~0~~~~;0~LEADER_B~true~0~0~~~~").is_err());
    assert!(research_sample_code(&BTreeSet::new()).is_err());
}
#[test]
fn report_counts_start_baseline_and_rejects_invalid_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("research.jsonl");
    fs::write(&path,"{\"phase\":\"start\",\"observed\":true,\"turn\":1,\"slot\":0,\"completed_techs\":[\"TECH_A\"]}\n{\"phase\":\"turn\",\"observed\":true,\"turn\":2,\"slot\":1,\"completed_techs\":[\"TECH_A\"]}\n").unwrap();
    let rows = research_report::summarize(&path, "tech").unwrap();
    assert_eq!(rows[0]["median_turn"], 1.5);
    assert_eq!(rows[0]["baseline_factions"], 1);
    assert!(research_report::summarize(&path, "other")
        .unwrap_err()
        .contains("kind must be tech or civic"));
    fs::write(
        &path,
        "{\"phase\":\"turn\",\"observed\":false,\"turn\":2,\"slot\":0,\"completed_techs\":[]}\n",
    )
    .unwrap();
    assert!(research_report::summarize(&path, "tech")
        .unwrap_err()
        .contains("Invalid research row"));
}
#[cfg(windows)]
#[test]
fn stdlib_memory_api_samples_a_real_process() {
    let sample = process_memory(std::process::id()).unwrap();
    assert!(sample["working_set_bytes"].as_u64().unwrap() > 0);
    assert!(sample["private_bytes"].as_u64().unwrap() > 0);
}
#[cfg(not(windows))]
#[test]
fn process_memory_requires_windows() {
    assert!(process_memory(std::process::id()).is_err());
}
