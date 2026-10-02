use super::{support, tuning_report};
use civ6_reports_rs::{arbitration, scenario, tuning};
use serde_json::{json, Value};
use std::{
    fs,
    process::{Command, Output},
};

fn run(bin: &str, args: &[&std::ffi::OsStr]) -> Output {
    Command::new(bin)
        .args(args)
        .current_dir(support::root())
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
    support::artifact(
        temp.path(),
        "[JAC_ARB] loaded shogunate_end_turn=120\n",
        &support::completed_steps(),
    );
    let output_path = temp.path().join("report.json");
    for (bin, data, markdown) in [
        (
            env!("CARGO_BIN_EXE_civ6-arbitration-report"),
            arbitration::summarize(temp.path()).unwrap(),
            arbitration::markdown as fn(&Value) -> String,
        ),
        (
            env!("CARGO_BIN_EXE_civ6-tuning-report"),
            tuning::summarize(temp.path()).unwrap(),
            tuning::markdown as fn(&Value) -> String,
        ),
    ] {
        let output = run(
            bin,
            &[
                temp.path().as_os_str(),
                "--json".as_ref(),
                output_path.as_os_str(),
            ],
        );
        assert!(output.status.success(), "{}", normalized(&output.stderr));
        let text = fs::read_to_string(&output_path).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), data);
        assert_eq!(
            text.replace("\r\n", "\n"),
            serde_json::to_string_pretty(&data).unwrap() + "\n"
        );
        assert_eq!(normalized(&output.stdout), markdown(&data) + "\n");
        assert!(output.stderr.is_empty());
        assert_eq!(run(bin, &[]).status.code(), Some(2));
        let help = run(bin, &["--help".as_ref()]);
        assert!(help.status.success());
        assert!(normalized(&help.stdout).contains("--json"));
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
        serde_json::to_string_pretty(&scenario::scenario().unwrap()).unwrap() + "\n"
    );
    use clap::Parser;
    let default =
        civ6_reports_rs::cli::ScenarioArgs::try_parse_from(["civ6-build-arbitration-ai-scenario"])
            .unwrap();
    assert_eq!(default.output, scenario::default_output());
}
