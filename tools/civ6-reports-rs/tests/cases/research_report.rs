use super::support;
use civ6_reports_rs::research;
use serde_json::json;
use std::fs;

#[test]
fn first_observed_tech_and_civic_reports_preserve_baselines_and_tsv() {
    let temp = support::tempdir();
    let path = temp.path().join("research.jsonl");
    let rows = [
        json!({"phase": "start", "observed": true, "turn": 1, "slot": 0, "completed_techs": ["TECH_BASE"], "completed_civics": []}),
        json!({"phase": "start", "observed": true, "turn": 1, "slot": 1, "completed_techs": [], "completed_civics": []}),
        json!({"phase": "turn", "observed": true, "turn": 3, "slot": 0, "completed_techs": ["TECH_BASE", "TECH_A"], "completed_civics": ["CIVIC_A"]}),
        json!({"phase": "turn", "observed": true, "turn": 5, "slot": 1, "completed_techs": ["TECH_A"], "completed_civics": []}),
        json!({"phase": "turn", "observed": true, "turn": 8, "slot": 0, "completed_techs": ["TECH_A"], "completed_civics": ["CIVIC_A"]}),
    ];
    fs::write(
        &path,
        rows.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let rows = research::summarize(&path, "tech").unwrap();
    assert_eq!(
        rows,
        vec![
            json!({"id": "TECH_BASE", "factions": 1, "median_turn": 1, "min_turn": 1, "max_turn": 1, "spread_turns": 0, "baseline_factions": 1}),
            json!({"id": "TECH_A", "factions": 2, "median_turn": 4.0, "min_turn": 3, "max_turn": 5, "spread_turns": 2, "baseline_factions": 0}),
        ]
    );
    assert_eq!(research::tsv(&rows), "id\tfactions\tmedian_turn\tmin_turn\tmax_turn\tspread_turns\tbaseline_factions\nTECH_BASE\t1\t1\t1\t1\t0\t1\nTECH_A\t2\t4.0\t3\t5\t2\t0\n");
    assert_eq!(
        research::summarize(&path, "civic").unwrap(),
        vec![
            json!({"id": "CIVIC_A", "factions": 1, "median_turn": 3, "min_turn": 3, "max_turn": 3, "spread_turns": 0, "baseline_factions": 0})
        ]
    );
    assert!(research::summarize(&path, "other")
        .unwrap_err()
        .to_string()
        .contains("kind must be tech or civic"));
}

#[test]
fn invalid_research_rows_report_the_exact_line() {
    let temp = support::tempdir();
    let path = temp.path().join("research.jsonl");
    let base = json!({"phase": "turn", "observed": true, "turn": 3, "slot": 0, "completed_techs": ["TECH_A"]});
    for (key, bad) in [
        ("phase", json!("finish")),
        ("observed", json!(false)),
        ("observed", json!(1)),
        ("turn", json!(-1)),
        ("turn", json!(true)),
        ("turn", json!(1.0)),
        ("slot", json!(-1)),
        ("slot", json!(true)),
        ("slot", json!(1.0)),
        ("completed_techs", json!("TECH_A")),
        ("completed_techs", json!([1])),
    ] {
        let mut row = base.clone();
        row[key] = bad;
        fs::write(&path, format!("{base}\n{row}\n")).unwrap();
        let error = research::summarize(&path, "tech").unwrap_err().to_string();
        assert!(
            error.contains(&format!("{}:2:", path.display())),
            "{key}: {error}"
        );
    }
    for invalid in ["", "{", "[]", "null", "true"] {
        fs::write(&path, format!("{base}\n{invalid}\n")).unwrap();
        assert!(research::summarize(&path, "tech")
            .unwrap_err()
            .to_string()
            .contains(":2:"));
    }
    fs::write(&path, "").unwrap();
    assert_eq!(
        research::summarize(&path, "tech").unwrap(),
        Vec::<serde_json::Value>::new()
    );
}
