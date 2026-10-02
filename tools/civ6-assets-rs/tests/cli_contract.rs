mod common;
use civ6_assets_rs::{
    arr, build_art, constants, generate, generate_portraits as portraits, read_json, root, s,
    write, write_city_names, write_custom_civs, xml,
};
use common::*;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs,
    path::Path,
    process::{Command, Output},
};

const BINS: [(&str, &str); 8] = [
    (
        "japan-map-generate",
        env!("CARGO_BIN_EXE_japan-map-generate"),
    ),
    (
        "japan-map-prosperity",
        env!("CARGO_BIN_EXE_japan-map-prosperity"),
    ),
    (
        "japan-map-prosper-start",
        env!("CARGO_BIN_EXE_japan-map-prosper-start"),
    ),
    ("japan-map-support", env!("CARGO_BIN_EXE_japan-map-support")),
    (
        "japan-map-unification",
        env!("CARGO_BIN_EXE_japan-map-unification"),
    ),
    ("jac-draw-crests", env!("CARGO_BIN_EXE_jac-draw-crests")),
    ("jac-build-art", env!("CARGO_BIN_EXE_jac-build-art")),
    (
        "jac-generate-portraits",
        env!("CARGO_BIN_EXE_jac-generate-portraits"),
    ),
];
fn command(name: &str, cwd: &Path) -> Command {
    let mut cmd = Command::new(BINS.iter().find(|(n, _)| *n == name).unwrap().1);
    cmd.current_dir(cwd).env("CIV6_ASSETS_ROOT", root());
    civ6_assets_rs::hidden(&mut cmd);
    cmd
}
fn success(cmd: &mut Command) -> Output {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{cmd:?}\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}
fn normalized(e: &xmltree::Element) -> Value {
    json!({"tag":e.name,"attrs":e.attributes.iter().collect::<std::collections::BTreeMap<_,_>>(),"text":e.get_text().filter(|t|!t.trim().is_empty()),"children":civ6_assets_rs::elements(e).map(normalized).collect::<Vec<_>>()})
}
fn copy_maps(to: &Path) {
    fs::create_dir_all(to).unwrap();
    for file in fs::read_dir(&maps().dir).unwrap() {
        let file = file.unwrap();
        fs::copy(file.path(), to.join(file.file_name())).unwrap();
    }
}

#[test]
fn every_old_runnable_script_has_a_working_cli_mapping() {
    let t = temp();
    let mapping =
        read_json(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli-mapping.json")).unwrap();
    let declared = arr(&mapping["entries"])
        .iter()
        .filter(|r| r["bin"].is_string())
        .map(|r| s(&r["source"]).to_owned())
        .collect::<BTreeSet<_>>();
    let mut runnable = BTreeSet::new();
    for directory in ["tools/japan_map_scale", "tools/jac_art"] {
        for file in fs::read_dir(root().join(directory)).unwrap() {
            let file = file.unwrap();
            if file.path().extension() == Some(OsStr::new("py"))
                && fs::read_to_string(file.path())
                    .unwrap()
                    .contains("if __name__ == \"__main__\":")
            {
                runnable.insert(format!(
                    "{directory}/{}",
                    file.file_name().to_string_lossy()
                ));
            }
        }
    }
    assert_eq!(declared, runnable);
    assert_eq!(declared.len(), 11);
    let mapped_bins = arr(&mapping["entries"])
        .iter()
        .filter_map(|r| r["bin"].as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(mapped_bins, BINS.iter().map(|(name, _)| *name).collect());
    for (name, _) in BINS {
        let out = success(command(name, t.path()).arg("--help"));
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("Usage:"),
            "{name}"
        );
        assert!(!command(name, t.path())
            .arg("--not-a-valid-option")
            .output()
            .unwrap()
            .status
            .success());
    }
    for subcommand in ["support", "custom-civs", "city-names", "feuds"] {
        let out = success(command("japan-map-support", t.path()).args([subcommand, "--help"]));
        assert!(String::from_utf8_lossy(&out.stdout).contains("--output"));
    }
    let out = command("japan-map-support", t.path()).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("subcommand"));
}

#[test]
fn support_cli_writes_factions_city_names_feuds_and_scenarios() {
    let t = temp();
    let output = t.path().join("custom");
    let source = root().join("poc/JAC_PoC_CustomCivs");
    fs::create_dir_all(output.join("ArtDefs")).unwrap();
    for file in [
        "Civilizations.artdef",
        "Leaders.artdef",
        "FallbackLeaders.artdef",
        "Cultures.artdef",
    ] {
        fs::copy(
            source.join("ArtDefs").join(file),
            output.join("ArtDefs").join(file),
        )
        .unwrap();
    }
    success(command("japan-map-support", t.path()).args(["custom-civs", "--output", "custom"]));
    assert_eq!(
        fs::read_to_string(output.join("Data/JAC_Japan1534_Gameplay.sql")).unwrap(),
        write_custom_civs::gameplay()
    );
    assert_eq!(
        normalized(&xml(output.join("Config/JAC_Japan1534_Players.xml")).unwrap()),
        normalized(&write_custom_civs::front_end())
    );
    success(command("japan-map-support", t.path()).args(["city-names", "--output", "custom"]));
    let city_data = write_city_names::data().unwrap();
    assert_eq!(
        fs::read_to_string(output.join("Data/JAC_CityNames_Gameplay.sql")).unwrap(),
        write_city_names::gameplay_sql(&city_data).unwrap()
    );
    assert_eq!(
        fs::read_to_string(output.join("Text/JAC_CityNames_Text.xml")).unwrap(),
        civ6_data_rs::asset_dependencies::city_names::text_xml(&city_data).unwrap()
    );
    success(command("japan-map-support", t.path()).args(["feuds", "--output", "relations.xml"]));
    assert_eq!(
        normalized(&xml(t.path().join("relations.xml")).unwrap()),
        normalized(&civ6_assets_rs::write_feuds::build().unwrap())
    );
    assert_eq!(
        civ6_assets_rs::write_feuds::relation_pairs(&civ6_assets_rs::write_feuds::ALLY_KINDS)
            .unwrap()
            .len(),
        8
    );
    success(
        command("japan-map-support", t.path())
            .args(["support", "--maps"])
            .arg(&maps().dir)
            .args(["--output", "map-mod", "--scenarios", "scenarios"]),
    );
    assert!(t
        .path()
        .join("map-mod/Config/JAC_JapanMapScale_Config.xml")
        .is_file());
    assert!(t
        .path()
        .join("map-mod/Text/JAC_JapanMapScale_Text.xml")
        .is_file());
    let scenarios = fs::read_dir(t.path().join("scenarios"))
        .unwrap()
        .map(|p| p.unwrap().file_name())
        .collect::<BTreeSet<_>>();
    let mut expected: BTreeSet<_> = ["ODA", "TAKEDA"]
        .into_iter()
        .flat_map(|human| {
            civ6_assets_rs::write_support::scenarios(&maps().dir, human)
                .unwrap()
                .into_iter()
                .map(|(name, _)| name)
        })
        .map(std::ffi::OsString::from)
        .collect();
    expected.extend([
        "japan-resources.json".into(),
        "japan-prosperous-15km.json".into(),
    ]);
    assert_eq!(scenarios, expected);
    let prosperous = read_json(t.path().join("scenarios/japan-prosperous-15km.json")).unwrap();
    assert_eq!(arr(&prosperous["steps"])[3]["action"], "ui_lua");
}

#[test]
fn map_clis_write_sqlite_and_prosperity_outputs() {
    let t = temp();
    copy_maps(&t.path().join("Maps"));
    success(command("japan-map-prosperity", t.path()).args([
        "--maps",
        "Maps",
        "--output",
        "prosperity.json",
    ]));
    assert_eq!(
        read_json(t.path().join("prosperity.json")).unwrap(),
        read_json(fixtures().join("prosperity-reference.json")).unwrap()
    );
    fs::copy(
        root().join("tools/civ6-autotest/scenarios/japan-prosperous-15km.json"),
        t.path().join("scenario.json"),
    )
    .unwrap();
    success(command("japan-map-prosper-start", t.path()).args([
        "--maps",
        "Maps",
        "--scenario",
        "scenario.json",
    ]));
    assert_eq!(
        read_json(t.path().join("scenario.json")).unwrap(),
        read_json(root().join("tools/civ6-autotest/scenarios/japan-prosperous-15km.json")).unwrap()
    );
    for km in constants::SCALES {
        let historical =
            rusqlite::Connection::open(t.path().join(format!("Maps/JAC_Japan_{km}km.Civ6Map")))
                .unwrap();
        assert_eq!(count(&historical, "SELECT COUNT(*) FROM Cities"), 56);
        assert_eq!(count(&historical, "SELECT COUNT(*) FROM Units"), 137);
        assert_eq!(
            rows(&historical, "PRAGMA integrity_check"),
            vec![vec![json!("ok")]]
        );
        let bare = rusqlite::Connection::open(
            t.path()
                .join(format!("Maps/JAC_Japan_{km}km_Settlers.Civ6Map")),
        )
        .unwrap();
        assert_eq!(count(&bare, "SELECT COUNT(*) FROM Cities"), 0);
    }
    let schema: Vec<String> = serde_json::from_value(maps().official["schema"].clone()).unwrap();
    drop(generate::create_schema_statements(&schema, &t.path().join("official.Civ6Map")).unwrap());
    success(command("japan-map-unification", t.path()).args([
        "--official",
        "official.Civ6Map",
        "--output",
        "unification.Civ6Map",
    ]));
    let db = rusqlite::Connection::open(t.path().join("unification.Civ6Map")).unwrap();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Plots"), 640);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Cities"), 4);
    assert_eq!(
        fs::read_to_string(t.path().join("unification.lua")).unwrap(),
        generate::STATIC_LUA
    );
}

#[test]
fn portrait_cli_checks_oauth_sanitizes_environment_and_validates_every_image() {
    let t = temp();
    fs::copy(
        env!("CARGO_BIN_EXE_jac-process-fixture"),
        t.path().join("fixture-codex.exe"),
    )
    .unwrap();
    let run = |originals: &str| {
        let mut c = command("jac-generate-portraits", t.path());
        c.args([
            "--only",
            "ODA_NOBUHIDE",
            "--originals",
            originals,
            "--cli",
            "fixture-codex.exe",
        ])
        .env("JAC_ASSETS_TEST_PROCESS", "1")
        .env("OPENAI_API_KEY", "unused-fixture-value")
        .env("custom-apikey", "unused-fixture-value")
        .env("CUSTOM_API_TOKEN", "unused-fixture-value");
        c
    };
    let out = success(&mut run("Originals"));
    assert!(String::from_utf8_lossy(&out.stdout).contains("OK ODA_NOBUHIDE"));
    let directory = t.path().join("Originals/oda_nobuhide");
    for stage in ["login", "exec"] {
        let record = read_json(directory.join(format!("fixture-{stage}.json"))).unwrap();
        assert!(Path::new(s(&record["cwd"])).is_absolute());
        assert!(arr(&record["environment_names"])
            .iter()
            .all(|name| !portraits::is_api_key_env(s(name))));
    }
    let log = fs::read_to_string(t.path().join("generation-log.jsonl")).unwrap();
    let records: Vec<Value> = log
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 3);
    for (record, kind) in records.iter().zip(["portrait", "figure", "background"]) {
        let artifact = portraits::validate(
            &directory.join(portraits::filename("ODA_NOBUHIDE", kind)),
            kind,
            &t.path().join("Originals"),
        )
        .unwrap();
        assert_eq!(record["sha256"], artifact["sha256"]);
        assert_eq!(record["pixels"], artifact["pixels"]);
    }
    let out = success(&mut run("Originals"));
    assert!(String::from_utf8_lossy(&out.stdout).contains("EXISTING"));
    assert_eq!(
        fs::read_to_string(t.path().join("generation-log.jsonl")).unwrap(),
        log
    );
    for (name, mode) in [("ApiKey", "api-key"), ("NoLogin", "none")] {
        let out = run(name)
            .env("JAC_ASSETS_TEST_LOGIN", mode)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("ChatGPT OAuth login"));
        assert!(!t
            .path()
            .join(name)
            .join("oda_nobuhide/fixture-exec.json")
            .exists());
    }
    let out = run("InvalidImage")
        .env("JAC_ASSETS_TEST_BAD_IMAGE", "1")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("background must be landscape"));
    assert_eq!(
        fs::read_to_string(t.path().join("generation-log.jsonl")).unwrap(),
        log
    );
    let out = success(command("jac-generate-portraits", t.path()).args([
        "--only",
        "ODA_NOBUHIDE",
        "--print-prompt",
    ]));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim_end(),
        portraits::prompt("ODA_NOBUHIDE").unwrap()
    );
}

#[test]
fn cooker_process_resolves_relative_paths_and_propagates_failure() {
    let t = temp();
    fs::create_dir_all(t.path().join("cooker")).unwrap();
    fs::create_dir_all(t.path().join("build/XLPs")).unwrap();
    fs::copy(
        env!("CARGO_BIN_EXE_jac-process-fixture"),
        t.path().join("cooker/Civ6AssetCooker_FinalRelease.exe"),
    )
    .unwrap();
    write(t.path().join("cooker/Civ6.cfg"), "fixture").unwrap();
    let names = vec!["JAC_Crests45".into(), "JAC_Leaders80".into()];
    build_art::write_xlp(&t.path().join("build"), "Package", "UITexture", &names).unwrap();
    let run = || {
        let mut c = Command::new(env!("CARGO_BIN_EXE_jac-process-fixture"));
        c.current_dir(t.path())
            .env("JAC_ASSETS_TEST_PROCESS", "1")
            .args(["--probe-cooker", "cooker", "build", "Package", "output"]);
        civ6_assets_rs::hidden(&mut c);
        c
    };
    success(&mut run());
    assert!(
        !build_art::verify_blp("Package", &names, 0, &t.path().join("output"))
            .unwrap()
            .is_empty()
    );
    let trace = read_json(t.path().join("build/fixture-cooker.json")).unwrap();
    let args = arr(&trace["args"]);
    assert_eq!(args[0], "--no_mt");
    for flag in ["--pantry", "--stewpot", "--config"] {
        let at = args.iter().position(|v| s(v) == flag).unwrap();
        assert!(Path::new(s(&args[at + 1])).is_absolute());
    }
    assert!(Path::new(s(args.last().unwrap())).is_absolute());
    let out = run()
        .env("JAC_ASSETS_TEST_COOKER_FAIL", "1")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("Cooker failed for Package"));
    assert!(!command("jac-build-art", t.path())
        .args(["--originals", "absent", "--output", "mod"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(!t.path().join("mod").exists());
}

#[test]
fn crest_cli_generates_png_svg_and_provenance_for_all_factions() {
    let t = temp();
    success(command("jac-draw-crests", t.path()).args(["--output", "crests"]));
    let records = read_json(t.path().join("crests/generation-log.json")).unwrap();
    assert_eq!(arr(&records).len(), 22);
    let mut hashes = BTreeSet::new();
    for row in arr(&records) {
        let path = t.path().join("crests").join(s(&row["png"]));
        let im = image::open(&path).unwrap().to_rgba8();
        assert_eq!(im.dimensions(), (512, 512));
        assert!(im.pixels().any(|p| p[3] > 0));
        assert!(im.pixels().all(|p| p.0[..3] == [255, 255, 255]));
        assert_eq!(
            row["png_sha256"],
            civ6_assets_rs::sha256(&fs::read(path).unwrap())
        );
        let svg = t.path().join("crests").join(s(&row["svg"]));
        assert_eq!(xml(&svg).unwrap().name, "svg");
        assert_eq!(
            row["svg_sha256"],
            civ6_assets_rs::sha256(&fs::read(svg).unwrap())
        );
        hashes.insert(s(&row["png_sha256"]).to_owned());
    }
    assert_eq!(hashes.len(), 22);
}
