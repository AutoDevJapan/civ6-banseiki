mod common;
use civ6_assets_rs::{
    arr, attr, build_art as art, generate as g, generate_portraits as portraits, n, read_json,
    root, rows as xml_rows, s, text_of, unification_test_map as unification, write,
    write_city_names as cn, write_custom_civs as cc, write_feuds as feuds,
    write_support as support, xml,
};
use common::*;
use image::{Rgba, RgbaImage};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
fn normalized_xml(e: &xmltree::Element) -> serde_json::Value {
    json!({"tag":e.name,"attrs":e.attributes.iter().collect::<BTreeMap<_,_>>(),"text":e.get_text().filter(|s|!s.trim().is_empty()).map(|s|s.into_owned()),"children":civ6_assets_rs::elements(e).map(normalized_xml).collect::<Vec<_>>()})
}

#[test]
fn all_22_civilizations_have_official_name_candidates_without_rebinding_existing_rows() {
    use civ6_data_rs::{cascade, Repository};
    let repo = Repository::checked_in();
    let db = cascade::official_db_for(&repo).unwrap();
    let rows = |civ: &str| -> Vec<(i64, String, i64, i64)> {
        db.prepare("SELECT rowid,CitizenName,Female,Modern FROM CivilizationCitizenNames WHERE CivilizationType=? ORDER BY CitizenName").unwrap()
            .query_map([civ],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap()
            .collect::<rusqlite::Result<_>>().unwrap()
    };
    let japanese = rows("CIVILIZATION_JAPAN");
    assert_eq!(japanese.len(), 40);
    let expected: Vec<_> = japanese
        .iter()
        .map(|(_, name, female, modern)| (name.clone(), *female, *modern))
        .collect();
    db.execute_batch(
        &repo
            .read("poc/JAC_PoC_CustomCivs/Data/JAC_CustomCivs_Gameplay.sql")
            .unwrap(),
    )
    .unwrap();
    assert!(rows("CIVILIZATION_JAC_ODA").is_empty());
    db.execute_batch(&cc::gameplay()).unwrap();
    let seats = &civ6_assets_rs::constants::SEATS;
    assert_eq!(seats.len(), 22);
    for seat in seats.iter() {
        let actual = rows(&seat.civ);
        assert_eq!(
            actual
                .iter()
                .map(|(_, name, f, m)| (name.clone(), *f, *m))
                .collect::<Vec<_>>(),
            expected,
            "{}",
            seat.civ
        );
        let ids = actual;
        db.execute_batch(&cc::citizen_names_sql()).unwrap();
        assert_eq!(
            rows(&seat.civ),
            ids,
            "existing citizen indices changed: {}",
            seat.civ
        );
    }
    assert_eq!(
        rows("CIVILIZATION_JAPAN"),
        japanese,
        "official source changed"
    );
    db.execute("INSERT INTO CivilizationCitizenNames(CivilizationType,CitizenName,Female,Modern) VALUES('CIVILIZATION_JAC_ODA','LOC_EXISTING_CUSTOM_NAME',1,0)",[]).unwrap();
    let existing = rows("CIVILIZATION_JAC_ODA");
    db.execute_batch(&cc::citizen_names_sql()).unwrap();
    assert_eq!(
        rows("CIVILIZATION_JAC_ODA"),
        existing,
        "custom row was replaced"
    );
}
#[test]
fn city_name_romanization_and_generated_files() {
    for (kana, en, ru) in [
        ("ほうじょう", "Hojo", "Ходзё"),
        ("すんぷ", "Sunpu", "Сумпу"),
        ("ひらい", "Hirai", "Хираи"),
        ("びっちゅう まつやま", "Bitchu Matsuyama", "Биттю-Мацуяма"),
        ("えいへいじ", "Eiheiji", "Эйхэйдзи"),
        ("きゃん", "Kyan", "Кян"),
        ("ユウフツ", "Yufutsu", "Юфуцу"),
        ("とおとうみ", "Totomi", "Тотоми"),
    ] {
        assert_eq!(cn::romanize(kana).unwrap(), (en.into(), ru.into()));
    }
    assert!(cn::romanize("unsupported").is_err());
    let data = cn::data().unwrap();
    assert_eq!(cn::pools(&data).unwrap().len(), 22);
    for (civ, pool) in cn::pools(&data).unwrap() {
        assert!(civ.starts_with("CIVILIZATION_JAC_"));
        assert!(
            pool.iter()
                .filter(|(_, sort)| *sort < cn::SHARED_SORT_BASE)
                .count()
                >= 25
        );
        assert_eq!(
            pool.iter().map(|r| &r.0).collect::<BTreeSet<_>>().len(),
            pool.len()
        );
    }
    let m = root().join("poc/JAC_PoC_CustomCivs");
    assert_eq!(
        cn::gameplay_sql(&data).unwrap(),
        fs::read_to_string(m.join("Data/JAC_CityNames_Gameplay.sql"))
            .unwrap()
            .replace("\r\n", "\n")
    );
    assert_eq!(
        normalized_xml(&cn::text_xml(&data).unwrap()),
        normalized_xml(&xml(m.join("Text/JAC_CityNames_Text.xml")).unwrap())
    );
    let mut duplicate = data.clone();
    let first = duplicate["houses"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()[0]
        .clone();
    duplicate["shared"].as_array_mut().unwrap().push(first);
    assert!(cn::pools(&duplicate).is_err());
}
#[test]
fn custom_faction_outputs_and_artdef_cloning() {
    let m = root().join("poc/JAC_PoC_CustomCivs");
    assert_eq!(
        cc::gameplay(),
        fs::read_to_string(m.join("Data/JAC_Japan1534_Gameplay.sql"))
            .unwrap()
            .replace("\r\n", "\n")
    );
    for (path, value) in [
        ("Config/JAC_Japan1534_Players.xml", cc::front_end()),
        ("Data/JAC_Japan1534_Icons.xml", cc::icons()),
        ("Data/JAC_Japan1534_Colors.xml", cc::colors()),
    ] {
        assert_eq!(
            normalized_xml(&value),
            normalized_xml(&xml(m.join(path)).unwrap()),
            "{path}"
        );
    }
    let generated = cc::translated_text().unwrap();
    let known = xml(m.join("Text/JAC_Japan1534_Text.xml")).unwrap();
    let records = |e: &xmltree::Element| {
        xml_rows(e, "LocalizedText")
            .iter()
            .map(|r| {
                (
                    (attr(r, "Tag").to_owned(), attr(r, "Language").to_owned()),
                    text_of(r, "Text"),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(records(&generated), records(&known));
    let t = temp();
    fs::create_dir_all(t.path().join("ArtDefs")).unwrap();
    for file in [
        "Civilizations.artdef",
        "Leaders.artdef",
        "FallbackLeaders.artdef",
        "Cultures.artdef",
    ] {
        fs::copy(
            m.join("ArtDefs").join(file),
            t.path().join("ArtDefs").join(file),
        )
        .unwrap();
    }
    cc::artdefs(t.path()).unwrap();
    for file in [
        "Civilizations.artdef",
        "Leaders.artdef",
        "FallbackLeaders.artdef",
        "Cultures.artdef",
    ] {
        assert_eq!(
            normalized_xml(&xml(t.path().join("ArtDefs").join(file)).unwrap()),
            normalized_xml(&xml(m.join("ArtDefs").join(file)).unwrap()),
            "{file}"
        );
    }
    cc::artdefs(t.path()).unwrap();
    let leaders = xml(t.path().join("ArtDefs/Leaders.artdef")).unwrap();
    assert_eq!(
        find_all(&leaders, &["m_RootCollections", "Element", "Element"]).len(),
        22
    );
}
#[test]
fn feuds_match_checked_rows_and_date_bounds() {
    let generated = feuds::build().unwrap();
    let expected = xml(root().join("poc/JAC_PoC_JapanMapScale/Data/JAC_Japan_Feuds.xml")).unwrap();
    let records = |e: &xmltree::Element| {
        civ6_assets_rs::elements(e)
            .map(|table| {
                (
                    table.name.clone(),
                    civ6_assets_rs::elements(table)
                        .map(|r| {
                            r.attributes
                                .iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect::<BTreeMap<_, _>>()
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    let a = records(&generated);
    let b = records(&expected);
    for (table, rows) in &a {
        let mut a = rows
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect::<Vec<_>>();
        let mut b = b[table]
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect::<Vec<_>>();
        a.sort();
        b.sort();
        assert_eq!(a, b, "{table}");
    }
    assert_eq!(feuds::relation_pairs(&feuds::ALLY_KINDS).unwrap().len(), 8);
    let data = json!({"start_date":"1534-05-12","relations":[{"a":"ODA","b":"DATE","type":"hostility","confidence":"high","from":"1534","to":"1535"},{"a":"ODA","b":"ASHINA","type":"hostility","confidence":"low","from":null,"to":null},{"a":"ODA","b":"HOJO","type":"hostility","confidence":"high","from":null,"to":"1534-05-12"},{"a":"ODA","b":"AINU","type":"friendship","confidence":"medium","from":null,"to":null}]});
    assert_eq!(
        feuds::relation_pairs_from(&data, &["hostility"]).unwrap(),
        vec![("ODA".into(), "DATE".into())]
    );
    assert_eq!(
        feuds::relation_pairs_from(&data, &feuds::ALLY_KINDS).unwrap(),
        vec![("ODA".into(), "AINU".into())]
    );
}
#[test]
fn generated_scenarios_and_resource_sites_are_current() {
    for human in ["ODA", "TAKEDA"] {
        for (name, data) in support::scenarios(&maps().dir, human).unwrap() {
            assert_eq!(
                data,
                read_json(root().join("tools/civ6-autotest/scenarios").join(&name)).unwrap(),
                "{name}"
            );
        }
    }
    assert_eq!(
        support::resource_scenario(&maps().dir).unwrap(),
        read_json(root().join("tools/civ6-autotest/scenarios/japan-resources.json")).unwrap()
    );
    let t = temp();
    g::write_resource_sites(&maps().manifest, &t.path().join("sites.lua")).unwrap();
    assert_eq!(
        fs::read_to_string(t.path().join("sites.lua")).unwrap(),
        fs::read_to_string(root().join("poc/JAC_PoC_JapanMapScale/Scripts/JAC_ResourceSites.lua"))
            .unwrap()
            .replace("\r\n", "\n")
    );
}
#[test]
fn unification_binary_and_sqlite_map() {
    let t = temp();
    let schema = serde_json::from_value::<Vec<String>>(maps().official["schema"].clone()).unwrap();
    let source = t.path().join("official.Civ6Map");
    drop(g::create_schema_statements(&schema, &source).unwrap());
    let path = t.path().join("unification.Civ6Map");
    unification::build(&path, &source).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Plots"), 32 * 20);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Players"), 3);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Cities"), 4);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM Buildings WHERE BuildingType='BUILDING_PALACE'"
        ),
        2
    );
    assert_eq!(
        unification::castle_rows(&path).unwrap(),
        BTreeMap::from([(166, 0), (185, 1), (454, 0), (473, 1)])
    );
    assert_eq!(
        fs::read_to_string(path.with_extension("lua")).unwrap(),
        g::STATIC_LUA
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM StartPositions"), 3);
    assert!(rows(&db, "PRAGMA integrity_check")[0][0] == "ok");
}
#[test]
fn portraits_are_oauth_only_and_pngs_are_validated() {
    assert_eq!(
        portraits::SUBJECTS
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        art::LEADER_ORDER.into_iter().collect()
    );
    for key in art::LEADER_ORDER {
        let text = portraits::prompt(key).unwrap();
        assert!(text.contains("official Codex CLI"));
        assert!(text.contains("No API keys"));
        assert!(text.contains("Rust"));
        assert!(!text.contains("Pillow"));
        assert!(!text.contains("python"));
        for kind in ["portrait", "figure", "background"] {
            assert_eq!(text.matches(&portraits::filename(key, kind)).count(), 1);
            assert_eq!(
                art::leaders()[key][kind],
                format!("{}/{}", key.to_lowercase(), portraits::filename(key, kind))
            );
        }
    }
    for name in [
        "OPENAI_API_KEY",
        "AZURE_OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "GOOGLE_API_KEY",
        "GEMINI_API_KEY",
        "CODEX_API_KEY",
        "openai_key",
    ] {
        assert!(portraits::is_api_key_env(name));
    }
    assert!(!portraits::is_api_key_env("CODEX_HOME"));
    let t = temp();
    let originals = t.path().join("Originals");
    fs::create_dir_all(&originals).unwrap();
    for (kind, w, h) in [
        ("portrait", 768, 768),
        ("figure", 768, 1024),
        ("background", 1024, 768),
    ] {
        let path = originals.join(format!("{kind}.png"));
        RgbaImage::from_pixel(w, h, Rgba([50, 60, 70, 255]))
            .save(&path)
            .unwrap();
        let validated = portraits::validate(&path, kind, &originals).unwrap();
        assert_eq!(validated["pixels"], json!([w, h]));
        assert_eq!(
            validated["sha256"],
            civ6_assets_rs::sha256(&fs::read(&path).unwrap())
        );
    }
    let small = originals.join("small.png");
    RgbaImage::new(20, 20).save(&small).unwrap();
    for kind in ["portrait", "figure", "background"] {
        assert!(portraits::validate(&small, kind, &originals).is_err());
    }
    assert!(portraits::prompt("UNKNOWN").is_err());
    let dir = originals.join("oda_nobuhide");
    fs::create_dir_all(&dir).unwrap();
    RgbaImage::new(768, 768)
        .save(dir.join(portraits::filename("ODA_NOBUHIDE", "portrait")))
        .unwrap();
    assert!(
        portraits::generate("ODA_NOBUHIDE", Path::new("missing-codex"), &originals)
            .unwrap_err()
            .to_string()
            .contains("partial existing output")
    );
}
#[test]
fn atlas_layout_chroma_key_dds_and_cooker_failures() {
    assert_eq!(civ6_assets_rs::round(2.675, 2), 2.67);
    assert_eq!(civ6_assets_rs::round(2.5, 0), 2.);
    assert_eq!(civ6_assets_rs::round(3.5, 0), 4.);
    let masters = (0..22)
        .map(|i| RgbaImage::from_pixel(2, 2, Rgba([i + 1, 0, 0, 255])))
        .collect::<Vec<_>>();
    let atlas = art::atlas(&masters, 2, Some(8)).unwrap();
    assert_eq!(atlas.dimensions(), (16, 6));
    for i in 0..22 {
        assert_eq!(
            atlas.get_pixel((i % 8) * 2, (i / 8) * 2),
            &Rgba([i as u8 + 1, 0, 0, 255])
        );
    }
    assert_eq!(atlas.get_pixel(15, 5)[3], 0);
    let t = temp();
    let input = t.path().join("key.png");
    let mut im = RgbaImage::from_pixel(20, 20, Rgba([255, 0, 255, 255]));
    for y in 4..16 {
        for x in 4..16 {
            im.put_pixel(x, y, Rgba([15, 45, 30, 255]));
        }
    }
    im.save(&input).unwrap();
    let key = art::key_magenta(&input).unwrap();
    assert_eq!(key.get_pixel(0, 0)[3], 0);
    assert_eq!(*key.get_pixel(10, 10), Rgba([15, 45, 30, 255]));
    let dds = t.path().join("test.dds");
    art::write_dds(&dds, &key).unwrap();
    let bytes = fs::read(dds).unwrap();
    assert_eq!(&bytes[128..], key.as_raw());
    let names = vec!["FIRST".to_owned(), "SECOND".to_owned()];
    assert!(art::verify_blp("p", &names, 0, t.path()).is_err());
    let package = t.path().join("p.blp");
    write(&package, b"short").unwrap();
    assert!(art::verify_blp("p", &names, 0, t.path()).is_err());
    let mut content = vec![0u8; 1024];
    content.extend(b"FIRST\0SECOND\0");
    write(&package, &content).unwrap();
    assert!(art::verify_blp(
        "p",
        &names,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            + 1_000_000_000,
        t.path()
    )
    .is_err());
    assert!(art::verify_blp("p", &["MISSING".into()], 0, t.path()).is_err());
    assert_eq!(
        art::verify_blp("p", &names, 0, t.path()).unwrap(),
        civ6_assets_rs::sha256(&content)
    );
    art::write_tex(t.path(), "face", (570, 1024), "Leader_Fallback").unwrap();
    let xml = xml(t.path().join("Textures/face.tex")).unwrap();
    assert_eq!(
        attr(xml.get_child("m_ClassName").unwrap(), "text"),
        "Leader_Fallback"
    );
    assert_eq!(
        find_all(&xml, &["m_Tags", "Element"])
            .iter()
            .map(|e| attr(e, "text"))
            .collect::<Vec<_>>(),
        vec!["Leader_Fallback", "Leader", "Fallback"]
    );
}
#[test]
fn parity_manifest_covers_all_49_baseline_methods() {
    let mappings =
        read_json(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/parity.json")).unwrap();
    assert_eq!(arr(&mappings).len(), 49);
    assert_eq!(
        arr(&mappings)
            .iter()
            .map(|m| format!("{}::{}::{}", s(&m["source"]), s(&m["class"]), s(&m["test"])))
            .collect::<BTreeSet<_>>()
            .len(),
        49
    );
    let mapped_methods = arr(&mappings)
        .iter()
        .map(|m| {
            (
                s(&m["source"]).to_owned(),
                s(&m["class"]).to_owned(),
                s(&m["test"]).to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    // Captured independently from the four old test files before their removal.
    let baseline = read_json(fixtures().join("legacy-test-inventory.json")).unwrap();
    assert_eq!(
        baseline["source_commit"],
        "331c933b685b3ea35ea7881e84ca6bbb9a3c2096"
    );
    assert_eq!(arr(&baseline["methods"]).len(), 49);
    let old_methods = arr(&baseline["methods"])
        .iter()
        .map(|method| {
            (
                s(&method[0]).to_owned(),
                s(&method[1]).to_owned(),
                s(&method[2]).to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(old_methods.len(), 49);
    assert_eq!(mapped_methods, old_methods);
    for map in arr(&mappings) {
        let (module, name) = s(&map["rust_test"]).split_once("::").unwrap();
        assert_eq!(name, s(&map["test"]));
        let text = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/{module}.rs")),
        )
        .unwrap();
        assert!(text.contains(&format!("fn {name}()")));
        assert!(!text.contains("#[ignore]"));
    }
    for package in arr(&read_json(fixtures().join("cooked-packages.json")).unwrap()["packages"]) {
        let filename = format!("{}.prefix", s(&package["source_file"]));
        let raw = fs::read(fixtures().join(filename)).unwrap();
        assert_eq!(raw.len(), n(&package["captured_prefix_bytes"]) as usize);
        assert_eq!(civ6_assets_rs::sha256(&raw), s(&package["prefix_sha256"]));
    }
}
