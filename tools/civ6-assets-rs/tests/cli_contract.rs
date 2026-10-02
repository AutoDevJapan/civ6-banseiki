mod common;
#[path = "common/generated_lua.rs"]
mod generated_lua;
use civ6_assets_rs::{
    arr, build_art, cli, constants, generate, generate_portraits as portraits, read_json, root, s,
    write, write_city_names, write_custom_civs, xml,
};
use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser};
use common::*;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs,
    io::Read,
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

// Keep metadata assertions on the same public lexer and typed guards as the real CLIs.
struct ParsedMatches<P: Parser> {
    matches: ArgMatches,
    parsed: P,
}
impl<P: Parser> CommandFactory for ParsedMatches<P> {
    fn command() -> clap::Command {
        P::command()
    }
    fn command_for_update() -> clap::Command {
        P::command_for_update()
    }
}
impl<P: Parser> FromArgMatches for ParsedMatches<P> {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        Ok(Self {
            matches: matches.clone(),
            parsed: P::from_arg_matches(matches)?,
        })
    }
    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        self.parsed.update_from_arg_matches(matches)?;
        self.matches = matches.clone();
        Ok(())
    }
}
impl<P: Parser> Parser for ParsedMatches<P> {}
struct CliParser {
    command: clap::Command,
    parse: fn(&[String]) -> Result<ArgMatches, clap::Error>,
}
fn parser<P: Parser>() -> CliParser {
    CliParser {
        command: P::command(),
        parse: |args| {
            civ6_cli_compat_rs::try_parse_from::<ParsedMatches<P>, _, _>(args)
                .map(|parsed| parsed.matches)
        },
    }
}
fn cli_commands() -> [CliParser; 8] {
    [
        parser::<cli::GenerateArgs>(),
        parser::<cli::ProsperityArgs>(),
        parser::<cli::ProsperStartArgs>(),
        parser::<cli::SupportArgs>(),
        parser::<cli::UnificationArgs>(),
        parser::<cli::CrestArgs>(),
        parser::<cli::PortraitArgs>(),
        parser::<cli::BuildArgs>(),
    ]
}

#[test]
fn negative_number_paths_are_values_but_unknown_hyphen_options_are_rejected() {
    let mut checked = 0;
    for CliParser { mut command, parse } in cli_commands() {
        command.build();
        let leaves = if command.get_subcommands().next().is_some() {
            command
                .get_subcommands()
                .filter(|sub| sub.get_name() != "help")
                .map(|sub| (vec![sub.get_name().to_owned()], sub))
                .collect::<Vec<_>>()
        } else {
            vec![(Vec::new(), &command)]
        };
        for (path, leaf) in leaves {
            for arg in leaf
                .get_arguments()
                .filter(|arg| arg.get_action().takes_values() && arg.get_id() != "only")
            {
                let mut args = vec![command.get_name().to_owned()];
                args.extend(path.iter().cloned());
                args.push(format!("--{}", arg.get_long().unwrap()));
                args.push(String::new());
                for value in ["-1", "-.5"] {
                    *args.last_mut().unwrap() = value.into();
                    let matches = parse(&args).unwrap();
                    let matches = path
                        .iter()
                        .fold(&matches, |m, sub| m.subcommand_matches(sub).unwrap());
                    assert_eq!(
                        matches
                            .get_one::<std::path::PathBuf>(arg.get_id().as_str())
                            .unwrap(),
                        Path::new(value),
                        "{args:?}"
                    );
                }
                for unknown in ["-1e3", "-1.", "-unknown", "--unknown-option"] {
                    *args.last_mut().unwrap() = unknown.into();
                    let error = parse(&args).unwrap_err();
                    assert_eq!(error.exit_code(), 2, "{args:?}");
                }
                args.pop();
                for value in ["-1e3", "-1."] {
                    *args.last_mut().unwrap() = format!("--{}={value}", arg.get_long().unwrap());
                    let matches = parse(&args).unwrap();
                    let matches = path
                        .iter()
                        .fold(&matches, |m, sub| m.subcommand_matches(sub).unwrap());
                    assert_eq!(
                        matches
                            .get_one::<std::path::PathBuf>(arg.get_id().as_str())
                            .unwrap(),
                        Path::new(value),
                        "{args:?}"
                    );
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 25);
    let t = temp();
    for flag in ["--only", "--on", "--o"] {
        let out = command("jac-generate-portraits", t.path())
            .arg(format!("{flag}=-1"))
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        assert!(String::from_utf8_lossy(&out.stderr).contains("invalid value '-1'"));
        for value in ["-1", "-.5", "-1e3", "-1."] {
            let out = command("jac-generate-portraits", t.path())
                .args([flag, value])
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(2));
            assert!(out.stdout.is_empty());
        }
    }
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 0);
}

#[test]
fn every_cli_option_replaces_previous_occurrences_including_inferred_spellings() {
    let mut checked = 0;
    let mut paths_checked = 0;
    for CliParser { mut command, parse } in cli_commands() {
        command.build();
        let leaves = if command.get_subcommands().next().is_some() {
            command
                .get_subcommands()
                .filter(|sub| sub.get_name() != "help")
                .map(|sub| (vec![sub.get_name().to_owned()], sub))
                .collect::<Vec<_>>()
        } else {
            vec![(Vec::new(), &command)]
        };
        for (path, leaf) in leaves {
            let names = leaf
                .get_arguments()
                .filter_map(|arg| arg.get_long())
                .collect::<Vec<_>>();
            for arg in leaf.get_arguments().filter(|arg| arg.get_id() != "help") {
                let long = arg.get_long().unwrap();
                let prefix = (1..long.len())
                    .map(|length| &long[..length])
                    .find(|prefix| {
                        names.iter().filter(|name| name.starts_with(prefix)).count() == 1
                    })
                    .unwrap();
                let values = if arg.get_id() == "only" {
                    vec!["ODA_NOBUHIDE", "DATE_TANEMUNE"]
                } else if arg.get_action().takes_values() {
                    vec!["first-path", "last-path"]
                } else {
                    Vec::new()
                };
                for (first, last) in [
                    (long, long),
                    (prefix, prefix),
                    (long, prefix),
                    (prefix, long),
                ] {
                    let mut args = vec![command.get_name().to_owned()];
                    args.extend(path.iter().cloned());
                    args.push(format!("--{first}"));
                    args.extend(values.first().map(|v| (*v).to_owned()));
                    args.push(format!("--{last}"));
                    args.extend(values.last().map(|v| (*v).to_owned()));
                    let matches = parse(&args).unwrap_or_else(|e| panic!("{args:?}: {e}"));
                    let matches = path
                        .iter()
                        .fold(&matches, |m, sub| m.subcommand_matches(sub).unwrap());
                    let actual = matches
                        .get_raw(arg.get_id().as_str())
                        .unwrap()
                        .map(OsStr::to_owned)
                        .collect::<Vec<_>>();
                    let expected =
                        vec![OsStr::new(values.last().copied().unwrap_or("true")).to_owned()];
                    assert_eq!(actual, expected, "{args:?}");
                }
                if arg.get_action().takes_values() && arg.get_id() != "only" {
                    for flag in [long, prefix] {
                        let mut args = vec![command.get_name().to_owned()];
                        args.extend(path.iter().cloned());
                        args.push(format!("--{flag}="));
                        let matches = parse(&args).unwrap_or_else(|e| panic!("{args:?}: {e}"));
                        let matches = path
                            .iter()
                            .fold(&matches, |m, sub| m.subcommand_matches(sub).unwrap());
                        assert_eq!(
                            matches
                                .get_one::<std::path::PathBuf>(arg.get_id().as_str())
                                .unwrap(),
                            Path::new("."),
                            "{args:?}"
                        );
                    }
                    paths_checked += 1;
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 27);
    assert_eq!(paths_checked, 25);
}

#[test]
fn repeated_portrait_selection_preserves_only_the_last_group_for_exact_prefix_and_alias() {
    let t = temp();
    for first in ["--only", "--on", "--o"] {
        for last in ["--only", "--on", "--o"] {
            for subjects in [vec!["DATE_TANEMUNE"], vec!["DATE_TANEMUNE", "ODA_NOBUHIDE"]] {
                let mut args = vec![first, "ODA_NOBUHIDE", "TAKEDA_NOBUTORA", last];
                args.extend(subjects.iter().copied());
                args.extend(["--print-prompt", "--print-p"]);
                let out = success(command("jac-generate-portraits", t.path()).args(&args));
                let expected = subjects
                    .iter()
                    .map(|key| portraits::prompt(key).unwrap())
                    .collect::<Vec<_>>()
                    .join("\n");
                assert_eq!(
                    String::from_utf8(out.stdout).unwrap().trim_end(),
                    expected,
                    "{args:?}"
                );
            }
        }
    }
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 0);
}

#[test]
fn all_cli_parsers_accept_unambiguous_long_argument_prefixes() {
    let mut commands = cli_commands();
    let mut checked = 0;
    for CliParser {
        command,
        parse: parse_args,
    } in &mut commands
    {
        command.build();
        let paths = if command.get_subcommands().next().is_some() {
            command
                .get_subcommands()
                .filter(|sub| sub.get_name() != "help")
                .map(|sub| vec![sub.get_name().to_owned()])
                .collect::<Vec<_>>()
        } else {
            vec![Vec::new()]
        };
        for path in paths {
            let leaf = path.iter().fold(&*command, |cmd, segment| {
                cmd.find_subcommand(segment).unwrap()
            });
            let long_names = leaf
                .get_arguments()
                .filter_map(|arg| arg.get_long())
                .collect::<Vec<_>>();
            for arg in leaf.get_arguments().filter(|arg| arg.get_id() != "help") {
                let long = arg.get_long().expect("all tool arguments have long names");
                let value = arg.get_action().takes_values().then(|| {
                    arg.get_value_parser()
                        .possible_values()
                        .and_then(|mut values| values.next())
                        .map(|value| value.get_name().to_owned())
                        .unwrap_or_else(|| "fixture-value".into())
                });
                let parse = |flag: &str| {
                    let mut args = vec![command.get_name().to_owned()];
                    args.extend(path.iter().cloned());
                    args.push(format!("--{flag}"));
                    args.extend(value.iter().cloned());
                    let matches = parse_args(&args).unwrap_or_else(|e| {
                        panic!("{} {path:?} --{flag}: {e}", command.get_name())
                    });
                    let matches = path
                        .iter()
                        .fold(&matches, |m, sub| m.subcommand_matches(sub).unwrap());
                    matches
                        .get_raw(arg.get_id().as_str())
                        .unwrap()
                        .map(OsStr::to_owned)
                        .collect::<Vec<_>>()
                };
                let exact = parse(long);
                let mut shortened = 0;
                for length in 1..long.len() {
                    let prefix = &long[..length];
                    if long_names
                        .iter()
                        .filter(|name| name.starts_with(prefix))
                        .count()
                        == 1
                    {
                        assert_eq!(parse(prefix), exact, "--{prefix} must behave as --{long}");
                        shortened += 1;
                    }
                }
                assert!(
                    shortened > 0,
                    "no unambiguous prefix was checked for --{long}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 27);
    let t = temp();
    for (name, _) in BINS {
        let out = success(command(name, t.path()).arg("--hel"));
        assert!(String::from_utf8_lossy(&out.stdout).contains("Usage:"));
    }
    for sub in ["support", "custom-civs", "city-names", "feuds"] {
        success(command("japan-map-support", t.path()).args([sub, "--hel"]));
    }
    for (name, flag) in [("japan-map-generate", "--o"), ("jac-build-art", "--gro")] {
        let out = command(name, t.path())
            .arg(flag)
            .arg("fixture-value")
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn portrait_subject_choices_are_validated_before_runtime_or_output() {
    let t = temp();
    for args in [
        vec!["--only", "UNKNOWN"],
        vec!["--o", "UNKNOWN"],
        vec!["--on", "UNKNOWN"],
        vec!["--only", "ODA_NOBUHIDE", "UNKNOWN"],
        vec!["--only", "UNKNOWN", "--o", "DATE_TANEMUNE"],
        vec!["--on", "ODA_NOBUHIDE", "--only", "DATE_TANEMUNE", "UNKNOWN"],
    ] {
        let out = command("jac-generate-portraits", t.path())
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(error.contains("invalid value 'UNKNOWN'"));
        assert!(error.contains("possible values"));
    }
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 0);
    for key in portraits::SUBJECTS.as_object().unwrap().keys() {
        for prefix in ["--o", "--on"] {
            let out = success(command("jac-generate-portraits", t.path()).args([
                prefix,
                key,
                "--print-p",
            ]));
            assert_eq!(
                String::from_utf8(out.stdout).unwrap().trim_end(),
                portraits::prompt(key).unwrap()
            );
        }
    }
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 0);
}

fn generator_fixture(repository: &Path, game: &Path, cache: &Path) {
    for source in [
        "documents/spec/data/japan-1534-start.json",
        "documents/spec/data/japan-map-resources.json",
        "tools/civ6-autotest/scenarios/japan-prosperous-15km.json",
    ] {
        write(
            repository.join(source),
            fs::read(root().join(source)).unwrap(),
        )
        .unwrap();
    }
    let mut archive =
        zip::ZipArchive::new(fs::File::open(fixtures().join("source-cache.zip")).unwrap()).unwrap();
    for (name, _, expected_hash) in generate::SOURCES {
        let mut bytes = Vec::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(civ6_assets_rs::sha256(&bytes), expected_hash);
        write(cache.join(name), bytes).unwrap();
    }
    let schema: Vec<String> = serde_json::from_value(maps().official["schema"].clone()).unwrap();
    drop(
        generate::create_schema_statements(
            &schema,
            &game.join("Base/Assets/Maps/EarthMaps/WorldMapHuge.Civ6Map"),
        )
        .unwrap(),
    );
    // Reconstitute the captured final official resource rules as input XML, without an installed SDK.
    let mut resources = xmltree::Element::new("Resources");
    let mut terrains = xmltree::Element::new("Resource_ValidTerrains");
    let mut features = xmltree::Element::new("Resource_ValidFeatures");
    for (kind, rule) in &maps().rules {
        let mut row = xmltree::Element::new("Row");
        row.attributes.extend(rule.attributes.clone());
        row.attributes.insert("ResourceType".into(), kind.clone());
        civ6_assets_rs::push(&mut resources, row);
        for terrain in &rule.terrains {
            civ6_assets_rs::push(
                &mut terrains,
                civ6_assets_rs::node(
                    "Row",
                    &[
                        ("ResourceType", kind.clone()),
                        ("TerrainType", terrain.clone()),
                    ],
                ),
            );
        }
        for feature in &rule.features {
            civ6_assets_rs::push(
                &mut features,
                civ6_assets_rs::node(
                    "Row",
                    &[
                        ("ResourceType", kind.clone()),
                        ("FeatureType", feature.clone()),
                    ],
                ),
            );
        }
    }
    let mut info = xmltree::Element::new("GameInfo");
    for table in [resources, terrains, features] {
        civ6_assets_rs::push(&mut info, table);
    }
    civ6_assets_rs::xml_write(game.join("Base/Assets/Gameplay/Data/Resources.xml"), &info).unwrap();
    for source in [
        "Expansion1_Resources.xml",
        "Expansion2_Resources.xml",
        "Expansion2_RemoveData.xml",
    ] {
        write(game.join("DLC/Expansion2/Data").join(source), "<GameInfo/>").unwrap();
    }
    write(
        repository.join("poc/JAC_PoC_JapanMapScale/Data/JAC_Japan_Resources.xml"),
        "<GameInfo/>",
    )
    .unwrap();
}

#[test]
fn generator_cli_updates_support_for_all_spellings_of_the_standard_output_directory() {
    let t = temp();
    let repository = t.path().join("repository");
    let game = t.path().join("game");
    let cache = t.path().join("cache");
    generator_fixture(&repository, &game, &cache);
    let mod_dir = repository.join("poc/JAC_PoC_JapanMapScale");
    let maps_dir = mod_dir.join("Maps");
    let config = mod_dir.join("Config/JAC_JapanMapScale_Config.xml");
    let text = mod_dir.join("Text/JAC_JapanMapScale_Text.xml");
    let scenario = repository.join("tools/civ6-autotest/scenarios/japan-map-15km.json");
    let run = |output: &Path| {
        let mut cmd = command("japan-map-generate", &repository);
        cmd.env("CIV6_ASSETS_ROOT", &repository)
            .env("CIV6_INSTALL_DIR", &game)
            .arg("--cache")
            .arg(&cache)
            .arg("--out")
            .arg(output);
        cmd
    };
    // Exporting before the standard Maps directory exists must also remain valid.
    success(&mut run(Path::new("first-export/Maps")));
    assert!(!maps_dir.exists());
    assert!(!config.exists());
    assert!(!text.exists());
    assert!(!scenario.exists());
    assert!(repository.join("first-export/Maps/manifest.json").is_file());
    for output in [
        Path::new("poc/JAC_PoC_JapanMapScale/Maps"),
        maps_dir.as_path(),
        Path::new("./poc/JAC_PoC_JapanMapScale/Maps/../Maps"),
    ] {
        write(&config, "stale config").unwrap();
        write(&text, "stale text").unwrap();
        write(&scenario, "stale scenario").unwrap();
        success(&mut run(output));
        assert_eq!(
            normalized(&xml(&config).unwrap()),
            normalized(&civ6_assets_rs::write_support::config())
        );
        assert_eq!(
            normalized(&xml(&text).unwrap()),
            normalized(&civ6_assets_rs::write_support::text().unwrap())
        );
        assert_eq!(
            read_json(&scenario).unwrap(),
            read_json(root().join("tools/civ6-autotest/scenarios/japan-map-15km.json")).unwrap()
        );
        let manifest = read_json(maps_dir.join("manifest.json")).unwrap();
        assert_eq!(arr(&manifest).len(), 2);
        for entry in arr(&manifest) {
            assert!(maps_dir.join(s(&entry["file"])).is_file());
            assert!(maps_dir.join(s(&entry["settler_file"])).is_file());
        }
        assert!(mod_dir.join("Scripts/JAC_ResourceSites.lua").is_file());
        generated_lua::verify(
            &maps_dir,
            &mod_dir,
            &repository.join("tools/civ6-autotest/scenarios"),
        );
    }
    // An unrelated export must still leave the standard MOD's support files alone.
    write(&config, "preserved config").unwrap();
    write(&text, "preserved text").unwrap();
    write(&scenario, "preserved scenario").unwrap();
    success(&mut run(Path::new("export/Maps")));
    assert_eq!(fs::read_to_string(config).unwrap(), "preserved config");
    assert_eq!(fs::read_to_string(text).unwrap(), "preserved text");
    assert_eq!(fs::read_to_string(scenario).unwrap(), "preserved scenario");
    assert!(repository.join("export/Maps/manifest.json").is_file());
    assert!(repository
        .join("export/Scripts/JAC_ResourceSites.lua")
        .is_file());
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
    // Source identities are the pre-deletion inventory, independent of the mapping under test.
    let baseline = [
        (
            "tools/japan_map_scale/generate.py",
            "japan-map-generate",
            "",
        ),
        (
            "tools/japan_map_scale/prosperity.py",
            "japan-map-prosperity",
            "",
        ),
        (
            "tools/japan_map_scale/prosper_start.py",
            "japan-map-prosper-start",
            "",
        ),
        (
            "tools/japan_map_scale/write_support.py",
            "japan-map-support",
            "support",
        ),
        (
            "tools/japan_map_scale/write_custom_civs.py",
            "japan-map-support",
            "custom-civs",
        ),
        (
            "tools/japan_map_scale/write_city_names.py",
            "japan-map-support",
            "city-names",
        ),
        (
            "tools/japan_map_scale/write_feuds.py",
            "japan-map-support",
            "feuds",
        ),
        (
            "tools/japan_map_scale/unification_test_map.py",
            "japan-map-unification",
            "",
        ),
        ("tools/jac_art/draw_crests.py", "jac-draw-crests", ""),
        ("tools/jac_art/build_art.py", "jac-build-art", ""),
        (
            "tools/jac_art/generate_portraits.py",
            "jac-generate-portraits",
            "",
        ),
    ];
    let entries = arr(&mapping["entries"])
        .iter()
        .filter(|r| r["bin"].is_string())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 11);
    assert_eq!(
        entries
            .iter()
            .map(|r| (s(&r["source"]), s(&r["bin"]), s(&r["subcommand"])))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(baseline)
    );
    let runnable = baseline
        .iter()
        .map(|(source, _, _)| (*source).to_owned())
        .collect();
    assert_eq!(declared, runnable);
    assert_eq!(declared.len(), 11);
    let mapped_bins = arr(&mapping["entries"])
        .iter()
        .filter_map(|r| r["bin"].as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(mapped_bins, BINS.iter().map(|(name, _)| *name).collect());
    for (name, executable) in BINS {
        assert!(Path::new(executable).is_file(), "{name}");
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
