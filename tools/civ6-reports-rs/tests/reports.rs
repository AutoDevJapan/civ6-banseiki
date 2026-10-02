#[path = "cases/arbitration_report.rs"]
mod arbitration_report;
#[path = "cases/cli.rs"]
mod cli;
#[path = "cases/research_report.rs"]
mod research_report;
#[path = "cases/scenario_paths.rs"]
mod scenario_paths;
mod support;
#[path = "cases/tuning_report.rs"]
mod tuning_report;

#[test]
fn baseline_parity_maps_every_method_to_a_distinct_rust_test() {
    use std::collections::BTreeSet;
    let mapping: serde_json::Value = serde_json::from_str(include_str!("parity.json")).unwrap();
    let mapping = mapping.as_array().unwrap();
    assert_eq!(mapping.len(), 29);
    let sources = [
        (
            "arbitration_report",
            include_str!("cases/arbitration_report.rs"),
        ),
        ("tuning_report", include_str!("cases/tuning_report.rs")),
    ];
    let pattern = regex::Regex::new(r"#\[test\]\s*fn (test_\w+)\(").unwrap();
    let tests: BTreeSet<_> = sources
        .iter()
        .flat_map(|(module, source)| {
            pattern
                .captures_iter(source)
                .map(move |m| format!("{module}::{}", &m[1]))
        })
        .collect();
    let mapped: BTreeSet<_> = mapping
        .iter()
        .map(|row| row["rust_test"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        mapped.len(),
        29,
        "every original method has its own Rust test"
    );
    assert_eq!(tests, mapped);
    let originals: BTreeSet<_> = mapping
        .iter()
        .map(|row| {
            (
                row["source"].as_str().unwrap(),
                row["class"].as_str().unwrap(),
                row["test"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(originals.len(), 29);
    for row in mapping {
        let source = row["source"].as_str().unwrap();
        let module = if source.ends_with("test_arbitration_report.py") {
            "arbitration_report"
        } else {
            assert!(source.ends_with("test_tuning_report.py"));
            "tuning_report"
        };
        assert_eq!(
            row["rust_test"],
            format!("{module}::{}", row["test"].as_str().unwrap())
        );
    }
}
