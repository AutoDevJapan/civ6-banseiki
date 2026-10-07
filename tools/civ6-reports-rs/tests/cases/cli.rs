use super::{support, tuning_report};
use civ6_reports_rs::{scenario, tuning};
use serde_json::{json, Value};
use std::{
    fs,
    process::{Command, Output},
};

fn run(bin: &str, args: &[&std::ffi::OsStr]) -> Output {
    Command::new(bin)
        .args(args)
        .current_dir(support::source_root())
        .output()
        .unwrap()
}

fn normalized(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .replace("\r\n", "\n")
}

#[test]
fn report_clis_emit_the_original_json_and_markdown() {
    let temp = support::tempdir();
    let output_path = temp.path().join("report.json");
    for (bin, name) in [
        (
            env!("CARGO_BIN_EXE_civ6-arbitration-report"),
            "arbitration-cost-refusal",
        ),
        (
            env!("CARGO_BIN_EXE_civ6-tuning-report"),
            "tuning-six-reigns",
        ),
    ] {
        let artifact = support::golden_artifact(temp.path(), name);
        let expected_json = support::golden_text(name, "expected.json");
        let expected_markdown = support::golden_text(name, "expected.md");
        let output = run(
            bin,
            &[
                artifact.as_os_str(),
                "--json".as_ref(),
                output_path.as_os_str(),
            ],
        );
        assert!(output.status.success(), "{}", normalized(&output.stderr));
        let text = fs::read_to_string(&output_path).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&text).unwrap(),
            serde_json::from_str::<Value>(&expected_json).unwrap()
        );
        assert_eq!(text.replace("\r\n", "\n"), expected_json);
        assert_eq!(normalized(&output.stdout), expected_markdown + "\n");
        assert!(output.stderr.is_empty());
        assert_eq!(run(bin, &[]).status.code(), Some(2));
        let help = run(bin, &["--help".as_ref()]);
        assert!(help.status.success());
        assert!(normalized(&help.stdout).contains("--json"));
    }
}

#[test]
fn failure_rows_cli_write_partial_json_and_markdown() {
    let temp = support::tempdir();
    for (bin, name) in [
        (
            env!("CARGO_BIN_EXE_civ6-arbitration-report"),
            "arbitration-cost-refusal",
        ),
        (
            env!("CARGO_BIN_EXE_civ6-tuning-report"),
            "tuning-six-reigns",
        ),
    ] {
        let artifact = support::golden_artifact(temp.path(), name);
        let fixture_steps = fs::read_to_string(artifact.join("steps.jsonl")).unwrap();
        let golden_json: Value =
            serde_json::from_str(&support::golden_text(name, "expected.json")).unwrap();
        for row in [
            json!({"status": "FAIL", "action": "run"}),
            json!({"status": "FAIL", "action": "run", "detail": null}),
        ] {
            fs::write(
                artifact.join("steps.jsonl"),
                format!("{fixture_steps}{row}\n"),
            )
            .unwrap();
            let mut expected = golden_json.clone();
            expected["complete_200_turns"] = json!(false);
            expected["measurement_complete"] = json!(false);
            expected["failures"] = json!([row]);
            let output_path = artifact.join("partial.json");
            let output = run(
                bin,
                &[
                    artifact.as_os_str(),
                    "--json".as_ref(),
                    output_path.as_os_str(),
                ],
            );
            assert!(output.status.success(), "{}", normalized(&output.stderr));
            assert!(output.stderr.is_empty());
            assert_eq!(
                serde_json::from_str::<Value>(&fs::read_to_string(&output_path).unwrap()).unwrap(),
                expected
            );
            let expected_md = support::golden_text(name, "expected.md")
                .replace("裁定計測: PASS", "裁定計測: 未達（部分集計）")
                .replace(
                    "200ターン完了とログ判定: PASS",
                    "200ターン完了とログ判定: 未達",
                )
                .replace(
                    "計測の完全性: PASS",
                    "計測の完全性: 未達（以下は部分集計。数値調整の根拠にしない）",
                );
            assert_eq!(normalized(&output.stdout), expected_md + "\n");
        }
    }
}

#[test]
fn non_success_metadata_rows_need_no_string_action_or_detail() {
    let temp = support::tempdir();
    for (bin, name) in [
        (
            env!("CARGO_BIN_EXE_civ6-arbitration-report"),
            "arbitration-cost-refusal",
        ),
        (
            env!("CARGO_BIN_EXE_civ6-tuning-report"),
            "tuning-six-reigns",
        ),
    ] {
        let artifact = support::golden_artifact(temp.path(), name);
        let steps = fs::read_to_string(artifact.join("steps.jsonl")).unwrap();
        for status in [Value::Null, json!(false), json!(7), json!("NOTE")] {
            let row = json!({"status": status, "action": null});
            fs::write(artifact.join("steps.jsonl"), format!("{steps}{row}\n")).unwrap();
            let output_path = artifact.join("report.json");
            let output = run(
                bin,
                &[
                    artifact.as_os_str(),
                    "--json".as_ref(),
                    output_path.as_os_str(),
                ],
            );
            assert!(output.status.success(), "{}", normalized(&output.stderr));
            assert_eq!(
                fs::read_to_string(output_path)
                    .unwrap()
                    .replace("\r\n", "\n"),
                support::golden_text(name, "expected.json")
            );
            assert_eq!(
                normalized(&output.stdout),
                support::golden_text(name, "expected.md") + "\n"
            );
        }
    }
}

#[test]
fn complete_tuning_output_preserves_counter_defaults_and_schema() {
    let (snapshots, judgments) = tuning_report::full_coverage_fixture();
    let temp = support::tempdir();
    let mut steps = support::completed_steps();
    steps.extend(snapshots.iter().map(
        |s| json!({"status": "OK", "action": "1/18 ui_lua", "detail": format!("return={s}")}),
    ));
    let mut lua = snapshots.join("\n") + "\n";
    for row in judgments {
        let reign = row["reign"].as_str().unwrap();
        let turn = row["turn"].as_i64().unwrap();
        lua.push_str(&format!("[JAC_AGES] judged reign={reign} turn={turn} length=65 era=1 engineEra=1 player={} age=NORMAL score=4 golden>=7 dark>=2 policy=\n", row["player"]));
    }
    support::artifact(temp.path(), &lua, &steps);
    let data = tuning::summarize(temp.path()).unwrap();
    assert_eq!(
        data["measurement_complete"], true,
        "{}",
        data["coverage_issues"]
    );
    assert_eq!(data["checkpoint_count"], 5);
    assert!(tuning::markdown(&data).contains("| REIGN_KOJI | 66 | 65 | 0 | 0 | 22 | 0 |"));
}

#[test]
fn research_and_generator_cli_arguments_and_outputs_match() {
    let temp = support::tempdir();
    let research = temp.path().join("research.jsonl");
    fs::write(&research, "{\"phase\":\"start\",\"observed\":true,\"turn\":1,\"slot\":0,\"completed_techs\":[\"TECH_A\"],\"completed_civics\":[\"CIVIC_A\"]}\n").unwrap();
    let bin = env!("CARGO_BIN_EXE_civ6-research-report");
    let output = run(bin, &[research.as_os_str()]);
    assert!(output.status.success());
    assert_eq!(normalized(&output.stdout), "id\tfactions\tmedian_turn\tmin_turn\tmax_turn\tspread_turns\tbaseline_factions\nTECH_A\t1\t1\t1\t1\t0\t1\n");
    let output = run(
        bin,
        &[research.as_os_str(), "--kind".as_ref(), "civic".as_ref()],
    );
    assert!(output.status.success());
    assert!(normalized(&output.stdout).contains("CIVIC_A\t1\t1\t1\t1\t0\t1\n"));
    assert_eq!(
        run(
            bin,
            &[research.as_os_str(), "--kind".as_ref(), "other".as_ref()]
        )
        .status
        .code(),
        Some(2)
    );
    fs::write(&research, "broken\n").unwrap();
    let output = run(bin, &[research.as_os_str()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(normalized(&output.stderr).starts_with("Report error: Invalid research row "));
    assert!(output.stdout.is_empty());
    let output_path = temp.path().join("scenario.json");
    let output = run(
        env!("CARGO_BIN_EXE_civ6-build-arbitration-ai-scenario"),
        &["--output".as_ref(), output_path.as_os_str()],
    );
    assert!(output.status.success(), "{}", normalized(&output.stderr));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(output_path)
            .unwrap()
            .replace("\r\n", "\n"),
        serde_json::to_string_pretty(
            &serde_json::from_str::<Value>(include_str!("../fixtures/court-arbitration-ai.json"))
                .unwrap()
        )
        .unwrap()
            + "\n"
    );
    let default =
        civ6_reports_rs::cli::ScenarioArgs::try_parse_from(["civ6-build-arbitration-ai-scenario"])
            .unwrap();
    assert!(default.output.is_none());
    assert!(default.repo.is_none());
    assert_eq!(
        scenario::default_output().unwrap(),
        support::root()
            .canonicalize()
            .unwrap()
            .join("tools/civ6-autotest/scenarios/court-arbitration-ai.json")
    );
}

#[test]
fn compat_report_clis_keep_fixture_json_with_empty_artifacts_and_repeated_json() {
    let temp = support::tempdir();
    for (bin, name) in [
        (
            env!("CARGO_BIN_EXE_civ6-arbitration-report"),
            "arbitration-cost-refusal",
        ),
        (
            env!("CARGO_BIN_EXE_civ6-tuning-report"),
            "tuning-six-reigns",
        ),
    ] {
        let artifact = support::golden_artifact(temp.path(), name);
        let output_path = temp.path().join(format!("{name}.json"));
        for empty_artifact in [false, true] {
            let artifact_arg = if empty_artifact {
                "".as_ref()
            } else {
                artifact.as_os_str()
            };
            let output = Command::new(bin)
                .args([
                    artifact_arg,
                    "--json".as_ref(),
                    "".as_ref(),
                    "--json".as_ref(),
                    "unused.json".as_ref(),
                    "--j".as_ref(),
                    output_path.as_os_str(),
                ])
                .current_dir(&artifact)
                .output()
                .unwrap();
            assert!(output.status.success(), "{}", normalized(&output.stderr));
            assert!(output.stderr.is_empty());
            let mut expected_json = support::golden_text(name, "expected.json");
            let mut expected_md = support::golden_text(name, "expected.md") + "\n";
            if empty_artifact {
                // pathlib.Path('').name is empty; the input logs and every
                // measured field still have the same fixed expectations.
                expected_json = expected_json
                    .replace(&format!("\"artifact\": \"{name}\""), "\"artifact\": \"\"");
                expected_md = expected_md.replace(&format!("実行: {name}"), "実行: ");
            }
            assert_eq!(normalized(&fs::read(&output_path).unwrap()), expected_json);
            assert_eq!(normalized(&output.stdout), expected_md);
            assert!(!artifact.join("unused.json").exists());
        }
    }
}

#[test]
fn compat_research_cli_last_kind_preserves_fixture_output() {
    let temp = support::tempdir();
    let research = temp.path().join("research.jsonl");
    fs::write(&research, "{\"phase\":\"start\",\"observed\":true,\"turn\":1,\"slot\":0,\"completed_techs\":[\"TECH_A\"],\"completed_civics\":[\"CIVIC_A\"]}\n").unwrap();
    let output = run(
        env!("CARGO_BIN_EXE_civ6-research-report"),
        &[
            research.as_os_str(),
            "--kind".as_ref(),
            "tech".as_ref(),
            "--k".as_ref(),
            "civic".as_ref(),
        ],
    );
    assert!(output.status.success(), "{}", normalized(&output.stderr));
    assert!(output.stderr.is_empty());
    assert_eq!(
        normalized(&output.stdout),
        "id\tfactions\tmedian_turn\tmin_turn\tmax_turn\tspread_turns\tbaseline_factions\nCIVIC_A\t1\t1\t1\t1\t0\t1\n"
    );
}

#[test]
fn compat_generator_cli_last_paths_preserve_fixture_json() {
    let temp = support::tempdir();
    let output_path = temp.path().join("scenario.json");
    let unused_path = temp.path().join("unused.json");
    let output = run(
        env!("CARGO_BIN_EXE_civ6-build-arbitration-ai-scenario"),
        &[
            "--output".as_ref(),
            "".as_ref(),
            "--output".as_ref(),
            unused_path.as_os_str(),
            "--o".as_ref(),
            output_path.as_os_str(),
            "--repo".as_ref(),
            "not-a-repository".as_ref(),
            "--r=".as_ref(),
        ],
    );
    assert!(output.status.success(), "{}", normalized(&output.stderr));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(!unused_path.exists());
    assert_eq!(
        normalized(&fs::read(output_path).unwrap()),
        serde_json::to_string_pretty(
            &serde_json::from_str::<Value>(include_str!("../fixtures/court-arbitration-ai.json"))
                .unwrap()
        )
        .unwrap()
            + "\n"
    );
}
