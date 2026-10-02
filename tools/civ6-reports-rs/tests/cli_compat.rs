use civ6_reports_rs::cli::{ArbitrationArgs, ResearchArgs, ResearchKind, ScenarioArgs, TuningArgs};
use clap::Parser;
use std::path::PathBuf;

fn assert_prefixes<P: Parser>(base: &[&str], name: &str, values: &[&str]) {
    for end in 3..=name.len() {
        let mut args = base.to_vec();
        args.push(&name[..end]);
        args.extend_from_slice(values);
        assert!(P::try_parse_from(&args).is_ok(), "{args:?}");
    }
    for help in ["--h", "--he", "--hel", "--help"] {
        let error = P::try_parse_from(base.iter().copied().chain([help]))
            .err()
            .unwrap();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
        assert_eq!(error.exit_code(), 0);
    }
}

fn assert_parse_error<P: Parser>(args: &[&str]) {
    let error = P::try_parse_from(args).err().unwrap();
    assert_eq!(error.exit_code(), 2, "{args:?}: {error}");
}

#[test]
fn compat_report_prefixes_match_argparse_and_new_repo_flag() {
    assert_prefixes::<ArbitrationArgs>(&["arbitration", "artifact"], "--json", &["report.json"]);
    assert_prefixes::<TuningArgs>(&["tuning", "artifact"], "--json", &["report.json"]);
    assert_prefixes::<ResearchArgs>(&["research", "research.jsonl"], "--kind", &["civic"]);
    assert_prefixes::<ScenarioArgs>(&["scenario"], "--output", &["scenario.json"]);
    assert_prefixes::<ScenarioArgs>(&["scenario"], "--repo", &["repository"]);
}

#[test]
fn compat_report_paths_and_repeated_options_keep_last_occurrence() {
    let arbitration =
        ArbitrationArgs::try_parse_from(["arbitration", "", "--json", "old", "--j="]).unwrap();
    assert_eq!(arbitration.artifact, PathBuf::from("."));
    assert_eq!(arbitration.json, Some(PathBuf::from(".")));
    let tuning = TuningArgs::try_parse_from(["tuning", "", "--json", "old", "--j", ""]).unwrap();
    assert_eq!(tuning.artifact, PathBuf::from("."));
    assert_eq!(tuning.json, Some(PathBuf::from(".")));
    let research =
        ResearchArgs::try_parse_from(["research", "", "--kind", "tech", "--k", "civic"]).unwrap();
    assert_eq!(research.research_jsonl, PathBuf::from("."));
    assert!(matches!(research.kind, ResearchKind::Civic));
    let scenario = ScenarioArgs::try_parse_from([
        "scenario", "--output", "old", "--o=", "--repo", "old", "--r", "",
    ])
    .unwrap();
    assert_eq!(scenario.output, Some(PathBuf::from(".")));
    assert_eq!(scenario.repo, Some(PathBuf::from(".")));
}

#[test]
fn compat_report_nonempty_paths_are_preserved() {
    for path in [" ", "../日本語", "./report.json"] {
        let arbitration =
            ArbitrationArgs::try_parse_from(["arbitration", path, "--json", path]).unwrap();
        assert_eq!(arbitration.artifact, PathBuf::from(path));
        assert_eq!(arbitration.json, Some(PathBuf::from(path)));
        let tuning = TuningArgs::try_parse_from(["tuning", path, "--json", path]).unwrap();
        assert_eq!(tuning.artifact, PathBuf::from(path));
        assert_eq!(tuning.json, Some(PathBuf::from(path)));
        let research = ResearchArgs::try_parse_from(["research", path]).unwrap();
        assert_eq!(research.research_jsonl, PathBuf::from(path));
        let scenario =
            ScenarioArgs::try_parse_from(["scenario", "--output", path, "--repo", path]).unwrap();
        assert_eq!(scenario.output, Some(PathBuf::from(path)));
        assert_eq!(scenario.repo, Some(PathBuf::from(path)));
    }
}

#[cfg(windows)]
#[test]
fn compat_report_paths_preserve_native_non_unicode() {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    let path = OsString::from_wide(&[b'x' as u16, 0xd800]);
    let args = ArbitrationArgs::try_parse_from([
        OsString::from("arbitration"),
        path.clone(),
        OsString::from("--json"),
        path.clone(),
    ])
    .unwrap();
    assert_eq!(args.artifact.into_os_string(), path);
    assert_eq!(args.json.unwrap().into_os_string(), path);
}

#[test]
fn compat_report_choices_unknown_flags_and_missing_values_stay_strict() {
    for kind in ["", "other", "ci", "Civic"] {
        assert_parse_error::<ResearchArgs>(&[
            "research",
            "research.jsonl",
            "--kind",
            kind,
            "--k",
            "tech",
        ]);
    }
    for tail in [
        vec!["--json-extra", "out"],
        vec!["--j"],
        vec!["another-artifact"],
    ] {
        let mut args = vec!["report", "artifact"];
        args.extend(tail);
        assert_parse_error::<ArbitrationArgs>(&args);
        assert_parse_error::<TuningArgs>(&args);
    }
    assert_parse_error::<ResearchArgs>(&["research", "research.jsonl", "--kind-extra", "tech"]);
    assert_parse_error::<ResearchArgs>(&["research", "research.jsonl", "--k"]);
    assert_parse_error::<ScenarioArgs>(&["scenario", "--output-extra", "out"]);
    assert_parse_error::<ScenarioArgs>(&["scenario", "--o"]);
    assert_parse_error::<ScenarioArgs>(&["scenario", "--repo-extra", "repo"]);
    assert_parse_error::<ScenarioArgs>(&["scenario", "--r"]);
}

#[test]
fn compat_report_negative_number_paths_preserve_values_and_last_occurrences() {
    for path in ["-1", "-2.5"] {
        let args = ArbitrationArgs::try_parse_from(["arbitration", path, "--json", path]).unwrap();
        assert_eq!(args.artifact, PathBuf::from(path));
        assert_eq!(args.json, Some(PathBuf::from(path)));
        let args = TuningArgs::try_parse_from(["tuning", path, "--json", path]).unwrap();
        assert_eq!(args.artifact, PathBuf::from(path));
        assert_eq!(args.json, Some(PathBuf::from(path)));
        let args = ResearchArgs::try_parse_from(["research", path]).unwrap();
        assert_eq!(args.research_jsonl, PathBuf::from(path));
        let args =
            ScenarioArgs::try_parse_from(["scenario", "--output", path, "--repo", path]).unwrap();
        assert_eq!(args.output, Some(PathBuf::from(path)));
        assert_eq!(args.repo, Some(PathBuf::from(path)));
        let args =
            ArbitrationArgs::try_parse_from(["arbitration", path, "--json", path, "--j", "out"])
                .unwrap();
        assert_eq!(args.json, Some(PathBuf::from("out")));
        let args =
            TuningArgs::try_parse_from(["tuning", path, "--json", path, "--j", "out"]).unwrap();
        assert_eq!(args.json, Some(PathBuf::from("out")));
        let args = ScenarioArgs::try_parse_from([
            "scenario", "--output", path, "--o", "out", "--repo", path, "--r", "repo",
        ])
        .unwrap();
        assert_eq!(args.output, Some(PathBuf::from("out")));
        assert_eq!(args.repo, Some(PathBuf::from("repo")));
    }
}

#[test]
fn compat_report_negative_numbers_do_not_loosen_choices_or_unknown_flags() {
    let error =
        ResearchArgs::try_parse_from(["research", "research.jsonl", "--kind", "-1", "--k", "tech"])
            .err()
            .unwrap();
    assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
    assert_eq!(error.exit_code(), 2);
    for value in ["--unknown", "-not-a-number", "-1file"] {
        assert_parse_error::<ArbitrationArgs>(&[
            "arbitration",
            "artifact",
            "--json",
            value,
            "--j",
            "out",
        ]);
        assert_parse_error::<TuningArgs>(&["tuning", "artifact", "--json", value, "--j", "out"]);
        assert_parse_error::<ResearchArgs>(&["research", value]);
        assert_parse_error::<ScenarioArgs>(&["scenario", "--output", value, "--o", "out"]);
        assert_parse_error::<ScenarioArgs>(&["scenario", "--repo", value, "--r", "repo"]);
    }
}
