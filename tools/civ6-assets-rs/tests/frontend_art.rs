mod common;
use civ6_assets_rs::{
    arr, attr, build_art as art, draw_crests as crests, read_json, root, rows as xml_rows, s,
    text_of, write, xml,
};
use common::*;
use image::{imageops, Rgba, RgbaImage};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    sync::LazyLock,
};
struct Art {
    _temp: tempfile::TempDir,
    originals: PathBuf,
    build: PathBuf,
    crests: Vec<RgbaImage>,
    colors: BTreeMap<String, [u8; 3]>,
}
static ART: LazyLock<Art> =
    LazyLock::new(|| {
        let temp = temp();
        let originals = temp.path().join("Originals");
        let build = temp.path().join("build");
        fs::create_dir_all(build.join("previews")).unwrap();
        crests::generate(&originals.join("crests")).unwrap();
        let masters = art::crest_masters(&originals).unwrap();
        for size in art::CIV_ICON_SIZES {
            art::grid_atlas(&masters, size, 8)
                .unwrap()
                .save(build.join(format!("previews/JAC_Crests{size}.png")))
                .unwrap();
        }
        let game = temp.path().join("game");
        let standard = game.join("Base/Assets/UI/Colors/PlayerStandardColors.xml");
        write(
            &standard,
            fs::read(fixtures().join("PlayerStandardColors.xml")).unwrap(),
        )
        .unwrap();
        let colors = art::house_colors(&game, &mod_dir()).unwrap();
        let mut leaders = vec![];
        for (i, key) in art::LEADER_ORDER.into_iter().enumerate() {
            let path = originals.join(key.to_lowercase()).join(
                civ6_assets_rs::generate_portraits::filename(key, "portrait"),
            );
            write(&path, []).unwrap();
            let im = RgbaImage::from_fn(1024, 1024, |x, y| {
                Rgba([
                    (40 + i * 7) as u8,
                    (60 + x / 32) as u8,
                    (80 + y / 32) as u8,
                    255,
                ])
            });
            im.save(&path).unwrap();
            leaders.push(art::leader_icon_master(&originals, key, &colors).unwrap());
        }
        art::atlas(&leaders, 64, Some(8))
            .unwrap()
            .save(build.join("previews/JAC_Leaders64.png"))
            .unwrap();
        Art {
            _temp: temp,
            originals,
            build,
            crests: masters,
            colors,
        }
    });
fn mod_dir() -> PathBuf {
    root().join("poc/JAC_PoC_CustomCivs")
}
fn modinfo() -> xmltree::Element {
    xml(mod_dir().join("JAC_PoC_CustomCivs.modinfo")).unwrap()
}
fn players() -> Vec<BTreeMap<String, String>> {
    [
        "Config/JAC_CustomCivs_Players.xml",
        "Config/JAC_Japan1534_Players.xml",
    ]
    .iter()
    .flat_map(|p| {
        let x = xml(mod_dir().join(p)).unwrap();
        xml_rows(&x, "Players")
            .iter()
            .map(|r| {
                r.attributes
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .collect::<Vec<_>>()
    })
    .collect()
}
fn package(name: &str) -> PathBuf {
    let path = fixtures().join(format!("{name}.blp.prefix"));
    assert!(
        path.is_file(),
        "captured real SDK package index missing: {name}"
    );
    path
}
fn icon_records(files: &BTreeSet<String>) -> art::IconRecords {
    art::icon_records(
        &files.iter().map(|p| mod_dir().join(p)).collect::<Vec<_>>(),
        false,
    )
    .unwrap()
}
#[test]
fn test_house_crests_fill_all_xp2_sizes_with_distinct_white_masks() {
    assert_eq!(crests::NAMES.len(), 22);
    assert_eq!(crests::NAMES.iter().collect::<BTreeSet<_>>().len(), 22);
    assert_eq!(crests::MOTIFS.len(), 22);
    let official: Value = read_json(fixtures().join("official.json")).unwrap();
    let source = s(&official["excerpts"]["DLC/Expansion2/Data/Expansion2_Icons_Civilizations.xml"]);
    assert!(!source.is_empty());
    let official = xmltree::Element::parse(source.as_bytes()).unwrap();
    let sizes = xml_rows(&official, "IconTextureAtlases")
        .iter()
        .filter(|r| attr(r, "Name") == "ICON_ATLAS_EXPANSION_2_CIVILIZATIONS")
        .map(|r| attr(r, "IconSize").parse::<u32>().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        sizes,
        BTreeSet::from([22, 30, 32, 36, 44, 45, 48, 50, 64, 80, 128, 200, 256])
    );
    let m = modinfo();
    let mut final_files = BTreeSet::new();
    for context in ["FrontEndActions", "InGameActions"] {
        let files = action_files(&m, context, "UpdateIcons");
        assert!(files.contains("Data/JAC_Crests_Icons.xml"));
        let mut civ_icons = BTreeMap::new();
        for file in &files {
            for row in xml_rows(&xml(mod_dir().join(file)).unwrap(), "IconDefinitions") {
                let name = attr(row, "Name");
                if name.starts_with("ICON_CIVILIZATION_") {
                    civ_icons.insert(name.to_owned(), file.clone());
                }
            }
        }
        assert_eq!(
            civ_icons,
            crests::NAMES
                .into_iter()
                .map(|n| (
                    format!("ICON_CIVILIZATION_JAC_{n}"),
                    "Data/JAC_Crests_Icons.xml".into()
                ))
                .collect()
        );
        final_files = files;
    }
    let (defs, atlases) = icon_records(&final_files);
    assert!(!atlases.contains_key("ICON_ATLAS_JAC_CIVILIZATIONS"));
    let rows = &atlases["ICON_ATLAS_JAC_CRESTS"];
    assert_eq!(
        rows.iter()
            .map(|r| r["IconSize"].parse::<u32>().unwrap())
            .collect::<BTreeSet<_>>(),
        sizes
    );
    assert!(rows
        .iter()
        .all(|r| r["IconsPerRow"] == "8" && r["IconsPerColumn"] == "3"));
    let mut players = players();
    let config_files = find_all(&m, &["FrontEndActions", "UpdateDatabase", "File"]);
    assert_eq!(
        config_files.last().unwrap().get_text().unwrap(),
        "Config/JAC_Crests_Players.xml"
    );
    let updates = xml(mod_dir().join("Config/JAC_Crests_Players.xml")).unwrap();
    for update in find_all(&updates, &["Players", "Update"]) {
        let where_ = update.get_child("Where").unwrap();
        let values = update.get_child("Set").unwrap();
        let matched = players
            .iter_mut()
            .filter(|p| where_.attributes.iter().all(|(k, v)| p.get(k) == Some(v)))
            .collect::<Vec<_>>();
        assert_eq!(matched.len(), 1);
        for p in matched {
            p.extend(
                values
                    .attributes
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone())),
            );
        }
    }
    assert_eq!(
        players
            .iter()
            .map(|p| p["CivilizationType"].clone())
            .collect::<BTreeSet<_>>(),
        crests::NAMES
            .into_iter()
            .map(|n| format!("CIVILIZATION_JAC_{n}"))
            .collect()
    );
    for p in &players {
        let icon = &p["CivilizationIcon"];
        assert_eq!(*icon, format!("ICON_{}", p["CivilizationType"]));
        assert_eq!(
            defs[icon]["Index"].parse::<usize>().unwrap(),
            crests::NAMES
                .iter()
                .position(|n| format!("CIVILIZATION_JAC_{n}") == p["CivilizationType"])
                .unwrap()
        );
        assert_eq!(defs[icon]["Atlas"], "ICON_ATLAS_JAC_CRESTS");
        assert!(!p.values().any(|v| v == "ICON_CIVILIZATION_JAPAN"));
    }
    let mut fingerprints = BTreeSet::new();
    for (name, master) in crests::NAMES.into_iter().zip(&ART.crests) {
        assert!(ART
            .originals
            .join("crests")
            .join(format!("{}.svg", name.to_lowercase()))
            .is_file());
        assert_eq!(master.dimensions(), (512, 512));
        assert_eq!(master.get_pixel(0, 0)[3], 0);
        assert!(art::alpha_bounds(master, 1).is_some());
        assert_eq!(
            master
                .pixels()
                .flat_map(|p| p.0[..3].iter().copied())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([255])
        );
        fingerprints.insert(master.pixels().map(|p| p[3]).collect::<Vec<_>>());
    }
    assert_eq!(fingerprints.len(), 22);
    for size in sizes {
        let atlas = image::open(ART.build.join(format!("previews/JAC_Crests{size}.png")))
            .unwrap()
            .to_rgba8();
        assert_eq!(atlas.dimensions(), (8 * size, 3 * size));
        for (i, master) in ART.crests.iter().enumerate() {
            let tile = imageops::crop_imm(
                &atlas,
                (i % 8) as u32 * size,
                (i / 8) as u32 * size,
                size,
                size,
            )
            .to_image();
            let expected = art::resize(master, size, size);
            assert!(
                tile.pixels()
                    .zip(expected.pixels())
                    .all(|(a, b)| a[3] == b[3]),
                "{} at {size}",
                crests::NAMES[i]
            );
        }
    }
}
#[test]
fn test_every_player_icon_and_portrait_has_a_cooked_texture() {
    let m = modinfo();
    let frontend = action_files(&m, "FrontEndActions", "UpdateIcons");
    let ingame = action_files(&m, "InGameActions", "UpdateIcons");
    assert_eq!(frontend, ingame);
    for context in ["FrontEndActions", "InGameActions"] {
        assert_eq!(
            action_files(&m, context, "UpdateArt"),
            BTreeSet::from(["JAC_PoC_CustomCivs.dep".to_owned()])
        );
    }
    let player_paths = action_files(&m, "FrontEndActions", "UpdateDatabase");
    let mut players = vec![];
    for p in &player_paths {
        let xml = xml(mod_dir().join(p)).unwrap();
        players.extend(xml_rows(&xml, "Players").iter().map(|r| {
            r.attributes
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<BTreeMap<_, _>>()
        }));
    }
    assert_eq!(players.len(), 22);
    assert_eq!(
        players
            .iter()
            .map(|p| &p["LeaderType"])
            .collect::<BTreeSet<_>>()
            .len(),
        22
    );
    assert!(players
        .iter()
        .all(|p| p["Domain"] == "Players:Expansion2_Players"));
    let base = read_json(fixtures().join("base-icons.json")).unwrap();
    assert!(base["definitions"].is_object());
    let (defs, atlases) = icon_records(&frontend);
    assert_eq!(
        atlases["ICON_ATLAS_JAC_LEADERS"]
            .iter()
            .map(|r| (r["IconsPerRow"].clone(), r["IconsPerColumn"].clone()))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([("8".into(), "3".into())])
    );
    for (i, p) in players.iter().enumerate() {
        let leader = &p["LeaderType"];
        assert_eq!(defs[&p["LeaderIcon"]]["Index"], i.to_string());
        assert_eq!(p["Portrait"], format!("{leader}_NEUTRAL"));
        assert_eq!(p["PortraitBackground"], format!("{leader}_BACKGROUND"));
    }
    let dep = xml(mod_dir().join("JAC_PoC_CustomCivs.dep")).unwrap();
    let ui_packages = find_all(&dep, &["LibraryDependencies", "Element"])
        .into_iter()
        .filter(|r| attr(r.get_child("LibraryName").unwrap(), "text") == "UITexture")
        .flat_map(|r| {
            find_all(r, &["PackageDependencies", "Element"])
                .iter()
                .map(|r| attr(r, "text").to_owned())
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ui_packages,
        BTreeSet::from([
            "JAC_CustomCivs_Icons.blp".into(),
            "JAC_CustomCivs_Leaders.blp".into()
        ])
    );
    let consumers = find_all(&dep, &["SystemDependencies", "Element"])
        .into_iter()
        .filter(|r| attr(r.get_child("ConsumerName").unwrap(), "text") == "UI")
        .collect::<Vec<_>>();
    assert_eq!(consumers.len(), 1);
    assert!(find_all(consumers[0], &["LibraryDependencies", "Element"])
        .iter()
        .any(|r| attr(r, "text") == "UITexture"));
    let listed = find_all(&m, &["Files", "File"])
        .iter()
        .map(|e| e.get_text().unwrap().into_owned())
        .collect::<BTreeSet<_>>();
    assert!(frontend.is_subset(&listed));
    assert!(player_paths.is_subset(&listed));
    assert!(listed.contains("JAC_PoC_CustomCivs.dep"));
    for p in ui_packages
        .iter()
        .cloned()
        .chain(["JAC_CustomCivs_Fallback.blp".into()])
    {
        assert!(listed.contains(&format!("Platforms/Windows/BLPs/{p}")));
        assert!(package(p.trim_end_matches(".blp")).is_file());
    }
    let (mut mod_textures, mut base_textures, mut portraits) =
        (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for p in &players {
        for field in [
            "CivilizationIcon",
            "LeaderIcon",
            "CivilizationAbilityIcon",
            "LeaderAbilityIcon",
        ] {
            let icon = &p[field];
            let base_def = base["definitions"].get(icon);
            assert!(defs.contains_key(icon) || base_def.is_some(), "{icon}");
            let atlas = defs
                .get(icon)
                .map(|d| d["Atlas"].as_str())
                .unwrap_or_else(|| s(&base_def.unwrap()["Atlas"]));
            let base_rows = base["atlases"].get(atlas);
            assert!(atlases.contains_key(atlas) || base_rows.is_some());
            let rows = atlases.get(atlas).cloned().unwrap_or_else(|| {
                arr(base_rows.unwrap())
                    .iter()
                    .map(|r| {
                        r.as_object()
                            .unwrap()
                            .iter()
                            .map(|(k, v)| (k.clone(), s(v).to_owned()))
                            .collect()
                    })
                    .collect()
            });
            let sizes = rows
                .iter()
                .map(|r| r["IconSize"].parse::<i32>().unwrap())
                .collect::<BTreeSet<_>>();
            if field == "CivilizationIcon" || field == "LeaderIcon" {
                assert!(BTreeSet::from([45, 80]).is_subset(&sizes));
            }
            let index = defs
                .get(icon)
                .map(|d| d["Index"].parse::<usize>().unwrap())
                .unwrap_or_else(|| s(&base_def.unwrap()["Index"]).parse().unwrap());
            for r in rows {
                assert!(
                    index
                        < r["IconsPerRow"].parse::<usize>().unwrap()
                            * r["IconsPerColumn"].parse::<usize>().unwrap()
                );
                let name = std::path::Path::new(&r["Filename"])
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                if atlases.contains_key(atlas) {
                    mod_textures.insert(name);
                } else {
                    base_textures.insert(name);
                }
            }
        }
        portraits.insert(p["Portrait"].clone());
        portraits.insert(p["PortraitBackground"].clone());
    }
    assert!(package("Icons").is_file());
    assert!(art::package_textures(&package("Icons"), &base_textures)
        .unwrap()
        .is_empty());
    assert!(
        art::package_textures(&package("JAC_CustomCivs_Icons"), &mod_textures)
            .unwrap()
            .is_empty()
    );
    assert!(
        art::package_textures(&package("JAC_CustomCivs_Leaders"), &portraits)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn test_ingame_portraits_and_fallbacks_cover_all_players() {
    let players = players();
    let leaders = players
        .iter()
        .map(|p| p["LeaderType"].clone())
        .collect::<BTreeSet<_>>();
    let dep = xml(mod_dir().join("JAC_PoC_CustomCivs.dep")).unwrap();
    let libraries = find_all(&dep, &["LibraryDependencies", "Element"])
        .into_iter()
        .filter(|r| attr(r.get_child("LibraryName").unwrap(), "text") == "LeaderFallback")
        .collect::<Vec<_>>();
    assert_eq!(libraries.len(), 1);
    assert_eq!(
        find_all(libraries[0], &["PackageDependencies", "Element"])
            .iter()
            .map(|r| attr(r, "text"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["JAC_CustomCivs_Fallback.blp"])
    );
    let consumers = find_all(&dep, &["SystemDependencies", "Element"])
        .into_iter()
        .filter(|r| attr(r.get_child("ConsumerName").unwrap(), "text") == "LeaderFallback")
        .collect::<Vec<_>>();
    assert_eq!(consumers.len(), 1);
    assert!(find_all(consumers[0], &["LibraryDependencies", "Element"])
        .iter()
        .any(|r| attr(r, "text") == "LeaderFallback"));
    let artdef = xml(mod_dir().join("ArtDefs/Leaders.artdef")).unwrap();
    assert_eq!(
        find_all(&artdef, &["m_RootCollections", "Element", "Element"])
            .iter()
            .map(|e| attr(e.get_child("m_Name").unwrap(), "text").to_owned())
            .collect::<BTreeSet<_>>(),
        leaders
    );
    let artdef = xml(mod_dir().join("ArtDefs/FallbackLeaders.artdef")).unwrap();
    let mut fallback = BTreeMap::new();
    for row in find_all(&artdef, &["m_RootCollections", "Element", "Element"]) {
        let leader = attr(row.get_child("m_Name").unwrap(), "text");
        let entry = find_all(
            row,
            &[
                "m_ChildCollections",
                "Element",
                "Element",
                "m_Fields",
                "m_Values",
                "Element",
                "m_EntryName",
            ],
        )[0];
        fallback.insert(leader.to_owned(), attr(entry, "text").to_owned());
    }
    assert_eq!(fallback.keys().cloned().collect::<BTreeSet<_>>(), leaders);
    for (l, e) in &fallback {
        assert_eq!(
            *e,
            format!(
                "FALLBACK_NEUTRAL_JAC_{}",
                l.trim_start_matches("LEADER_JAC_")
            )
        );
    }
    assert!(package("JAC_CustomCivs_Fallback").is_file());
    assert!(art::package_textures(
        &package("JAC_CustomCivs_Fallback"),
        &fallback.values().cloned().collect()
    )
    .unwrap()
    .is_empty());
    let mut textures = BTreeSet::new();
    let insert = regex::Regex::new(
        r"(?s)INSERT INTO (LoadingInfo|DiplomacyInfo)\s*\(.*?\)\s*VALUES\s*(.*?);",
    )
    .unwrap();
    let loading = regex::Regex::new(r"\('([^']+)',\s*'([^']+)',\s*'([^']+)'").unwrap();
    let diplomacy = regex::Regex::new(r"\('([^']+)',\s*'([^']+)'").unwrap();
    for path in [
        "Data/JAC_CustomCivs_Gameplay.sql",
        "Data/JAC_Japan1534_Gameplay.sql",
    ] {
        let sql = fs::read_to_string(mod_dir().join(path)).unwrap();
        let captures = insert.captures_iter(&sql).collect::<Vec<_>>();
        let load = captures.iter().find(|c| &c[1] == "LoadingInfo").unwrap();
        let dip = captures.iter().find(|c| &c[1] == "DiplomacyInfo").unwrap();
        let loads = loading
            .captures_iter(&load[2])
            .map(|c| [c[1].to_owned(), c[2].to_owned(), c[3].to_owned()])
            .collect::<Vec<_>>();
        let dips = diplomacy
            .captures_iter(&dip[2])
            .map(|c| [c[1].to_owned(), c[2].to_owned()])
            .collect::<Vec<_>>();
        assert!(!loads.is_empty());
        assert!(!dips.is_empty());
        let actual = loads.iter().map(|r| r[0].clone()).collect::<BTreeSet<_>>();
        let existing = BTreeSet::from([
            "LEADER_JAC_ODA_NOBUHIDE".to_owned(),
            "LEADER_JAC_TAKEDA_NOBUTORA".to_owned(),
        ]);
        assert_eq!(
            actual,
            if path.ends_with("JAC_CustomCivs_Gameplay.sql") {
                existing.clone()
            } else {
                leaders.difference(&existing).cloned().collect()
            }
        );
        assert_eq!(actual, dips.iter().map(|r| r[0].clone()).collect());
        for r in loads {
            assert_eq!(r[1], format!("{}_NEUTRAL", r[0]));
            assert_eq!(r[2], format!("{}_BACKGROUND", r[0]));
            textures.insert(r[1].clone());
            textures.insert(r[2].clone());
        }
        for r in dips {
            assert_eq!(r[1], format!("{}_DIPLOMACY_BACKGROUND", r[0]));
            textures.insert(r[1].clone());
        }
    }
    assert!(
        art::package_textures(&package("JAC_CustomCivs_Leaders"), &textures)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn test_notebooks_glyph_matches_official_neighbours() {
    let t = temp();
    let input = t.path().join("growth");
    fs::create_dir_all(&input).unwrap();
    fs::copy(
        fixtures().join("notebooks_generated.png"),
        input.join("notebooks_generated.png"),
    )
    .unwrap();
    let dds = t.path().join("JAC_Notebooks_Hook.dds");
    art::growth_icon(&input, &dds, t.path()).unwrap();
    assert!(dds.is_file());
    let data = fs::read(dds).unwrap();
    assert_eq!(&data[..4], b"DDS ");
    let h = u32::from_le_bytes(data[12..16].try_into().unwrap());
    let w = u32::from_le_bytes(data[16..20].try_into().unwrap());
    assert_eq!((w, h), (26, 26));
    let im = RgbaImage::from_raw(w, h, data[128..].to_vec()).unwrap();
    let (x0, y0, x1, y1) = art::alpha_bounds(&im, 128).unwrap();
    let official = [(23, 18, 223), (20, 22, 175), (14, 17, 181)];
    assert!((13..=24).contains(&(x1 - x0)));
    assert!((16..=23).contains(&(y1 - y0)));
    let coverage = im.pixels().map(|p| p[3] as f64).sum::<f64>() / 255.;
    let mean = official.iter().map(|r| r.2 as f64).sum::<f64>() / 3.;
    assert!((coverage - mean).abs() < mean * 0.15, "{coverage}");
    assert!(((x0 + x1) as f64 / 2. - 13.).abs() <= 1.);
    assert!(((y0 + y1) as f64 / 2. - 13.).abs() <= 1.);
}
#[test]
fn test_leader_icons_are_face_crops_framed_in_house_colour() {
    let faces = art::faces();
    assert_eq!(
        faces
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| k.as_str() != "_comment")
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        art::LEADER_ORDER.into_iter().collect()
    );
    assert_eq!(
        ART.colors
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        art::LEADER_ORDER.into_iter().collect()
    );
    for key in art::LEADER_ORDER {
        let a = arr(&faces[key]);
        assert!(
            a[0].as_f64().unwrap() >= 0.
                && a[0].as_f64().unwrap() < a[2].as_f64().unwrap()
                && a[2].as_f64().unwrap() <= 1.
                && a[1].as_f64().unwrap() >= 0.
                && a[1].as_f64().unwrap() < a[3].as_f64().unwrap()
                && a[3].as_f64().unwrap() <= 1.
        );
    }
    let atlas = image::open(ART.build.join("previews/JAC_Leaders64.png"))
        .unwrap()
        .to_rgba8();
    for (i, key) in art::LEADER_ORDER.into_iter().enumerate() {
        let (x, y) = ((i % 8) as u32 * 64, (i / 8) as u32 * 64);
        assert_eq!(atlas.get_pixel(x, y)[3], 0);
        let ring = atlas.get_pixel(x + 32, y + 2);
        assert!(
            (0..3)
                .map(|c| ring[c].abs_diff(ART.colors[key][c]))
                .max()
                .unwrap()
                < 40,
            "{key} {ring:?}"
        );
    }
}
#[test]
fn test_house_primary_colours_are_distinct() {
    let mut names = BTreeMap::new();
    for file in ["JAC_CustomCivs_Colors.xml", "JAC_Japan1534_Colors.xml"] {
        for row in xml_rows(
            &xml(mod_dir().join("Data").join(file)).unwrap(),
            "PlayerColors",
        ) {
            names.insert(text_of(row, "Type"), text_of(row, "PrimaryColor"));
        }
    }
    assert_eq!(names.len(), 22);
    assert_eq!(names.values().collect::<BTreeSet<_>>().len(), 22);
    let colors = read_json(fixtures().join("official.json")).unwrap()["colors"].clone();
    let colors = serde_json::from_value::<BTreeMap<String, [u8; 3]>>(colors).unwrap();
    let list = colors.values().collect::<Vec<_>>();
    for (i, a) in list.iter().enumerate() {
        for b in &list[i + 1..] {
            let d = (0..3)
                .map(|i| (a[i] as f64 - b[i] as f64).powi(2))
                .sum::<f64>()
                .sqrt();
            assert!(d >= 40., "{a:?} {b:?}: {d}");
        }
    }
}
