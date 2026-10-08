use civ6_data_rs::{pool, Repository};
use serde_json::{json, Value};

fn row_mut<'a>(data: &'a mut Value, id: &str) -> &'a mut Value {
    data["characters"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["id"] == id)
        .unwrap()
}

fn catalogue() -> Value {
    Repository::checked_in().json(pool::DATA_PATH).unwrap()
}

#[test]
fn checked_in_roster_is_valid_and_adopted() {
    pool::validate(&catalogue()).unwrap();
}

#[test]
fn rejects_invalid_records_without_weakening_the_roster_contract() {
    let cases: Vec<(&str, &str, Value, &str)> = vec![
        (
            "JAC_POOL_H001",
            "id",
            json!("JAC_POOL_H008"),
            "duplicate id",
        ),
        (
            "JAC_POOL_H001",
            "reading",
            json!("Oda Nobunaga"),
            "reading must be kana",
        ),
        (
            "JAC_POOL_H001",
            "reading",
            json!("ーー"),
            "reading must be kana",
        ),
        ("JAC_POOL_H001", "origin", json!("伝説"), "origin"),
        ("JAC_POOL_H001", "trait", json!("超天才"), "unknown trait"),
        (
            "JAC_POOL_H001",
            "category",
            json!("自作キャラクター"),
            "unknown category",
        ),
        (
            "JAC_POOL_H001",
            "category",
            json!("軍師"),
            "武将: expected 120",
        ),
        (
            "JAC_POOL_H001",
            "leader_candidate",
            json!("true"),
            "must be boolean",
        ),
        ("JAC_POOL_H001", "name", json!("毛利元就"), "duplicate name"),
        ("JAC_POOL_H001", "description", json!(""), "description"),
        ("JAC_POOL_H001", "era", json!("幕末"), "unknown era"),
        ("JAC_POOL_H001", "region", json!("越後"), "unknown region"),
        ("JAC_POOL_H001", "faction", json!(null), "faction"),
        (
            "JAC_POOL_H001",
            "sources",
            json!([]),
            "historical rows need sources",
        ),
        (
            "JAC_POOL_O001",
            "id",
            json!("JAC_POOL_H099"),
            "id does not match origin",
        ),
        (
            "JAC_POOL_O001",
            "existing_leader_id",
            json!("LEADER_JAC_NEW"),
            "new fiction must not alias",
        ),
    ];
    for (id, field, value, expected) in cases {
        let mut data = catalogue();
        let row = data["characters"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["id"] == id)
            .unwrap();
        row[field] = value;
        let error = format!("{:#}", pool::validate(&data).unwrap_err());
        assert!(error.contains(expected), "{id}.{field}: {error}");
    }
}

#[test]
fn rejects_missing_or_unknown_fields_and_non_objects() {
    for kind in 0..4 {
        let mut data = catalogue();
        match kind {
            0 => {
                row_mut(&mut data, "JAC_POOL_H001")
                    .as_object_mut()
                    .unwrap()
                    .remove("faction");
            }
            1 => {
                row_mut(&mut data, "JAC_POOL_H001")["typo"] = json!("unexpected");
            }
            2 => {
                *row_mut(&mut data, "JAC_POOL_H001") = json!([]);
            }
            _ => {
                data["typo"] = json!(true);
            }
        }
        assert!(pool::validate(&data).is_err());
    }
}

#[test]
fn historical_source_records_must_be_complete() {
    for (field, value, expected) in [
        ("url", json!("file:///secret"), "https"),
        (
            "url",
            json!("https://name@host.example/page"),
            "invalid source URL",
        ),
        (
            "verified_on",
            json!("2026-02-30"),
            "invalid verification date",
        ),
        ("facts", json!(""), "facts"),
        (
            "verified_on",
            json!("2026-+1-+7"),
            "invalid verification date",
        ),
    ] {
        let mut data = catalogue();
        row_mut(&mut data, "JAC_POOL_H001")["sources"][0][field] = value;
        let error = format!("{:#}", pool::validate(&data).unwrap_err());
        assert!(error.contains(expected), "{field}: {error}");
    }
    let mut data = catalogue();
    row_mut(&mut data, "JAC_POOL_O001")["sources"] =
        row_mut(&mut data, "JAC_POOL_H001")["sources"].clone();
    assert!(format!("{:#}", pool::validate(&data).unwrap_err()).contains("new rows must have none"));
}

#[test]
fn rejects_incomplete_catalogue_and_premature_approval() {
    for (field, value) in [
        ("target_size", json!(100)),
        ("status", json!("方向確認待ち")),
        ("schema_version", json!(2)),
    ] {
        let mut data = catalogue();
        data[field] = value;
        assert!(pool::validate(&data).is_err());
    }
    let mut data = catalogue();
    data["characters"].as_array_mut().unwrap().pop();
    assert!(format!("{:#}", pool::validate(&data).unwrap_err()).contains("contain 300"));
    let mut data = catalogue();
    for row in data["characters"].as_array_mut().unwrap() {
        row["leader_candidate"] = json!(false);
    }
    assert!(format!("{:#}", pool::validate(&data).unwrap_err())
        .contains("every adopted character must be a leader candidate"));
    let mut data = catalogue();
    for row in data["characters"].as_array_mut().unwrap() {
        if row["region"] == "北海道" {
            row["region"] = json!("東北");
        }
    }
    assert!(format!("{:#}", pool::validate(&data).unwrap_err()).contains("all 16 regions"));
}

#[test]
fn cli_checks_an_external_file_and_returns_failure_for_invalid_json() {
    let tool = env!("CARGO_BIN_EXE_banseiki-pool-check");
    assert!(std::process::Command::new(tool)
        .output()
        .unwrap()
        .status
        .success());
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("invalid.json");
    std::fs::write(&file, "{broken").unwrap();
    let output = std::process::Command::new(tool)
        .arg(&file)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
    assert!(std::process::Command::new(tool)
        .arg("--help")
        .output()
        .unwrap()
        .status
        .success());
    assert!(!std::process::Command::new(tool)
        .arg("--unknown")
        .output()
        .unwrap()
        .status
        .success());
}

#[test]
fn identity_source_and_coverage_regressions_are_rejected() {
    for (field, value, expected) in [
        ("id", json!("JAC_POOL_H000"), "nonzero decimal digits"),
        ("id", json!("JAC_POOL_H01"), "nonzero decimal digits"),
        (
            "existing_leader_id",
            json!("LEADER/INVALID"),
            "invalid existing_leader_id",
        ),
        (
            "existing_leader_id",
            json!("LEADER_JAC_MORI_MOTONARI"),
            "duplicate existing_leader_id",
        ),
    ] {
        let mut data = catalogue();
        row_mut(&mut data, "JAC_POOL_H001")[field] = value;
        assert!(format!("{:#}", pool::validate(&data).unwrap_err()).contains(expected));
    }
    let mut data = catalogue();
    row_mut(&mut data, "JAC_POOL_H001")["sources"][0]
        .as_object_mut()
        .unwrap()
        .remove("facts");
    assert!(
        format!("{:#}", pool::validate(&data).unwrap_err()).contains("missing or unknown field")
    );
    let mut data = catalogue();
    let row = row_mut(&mut data, "JAC_POOL_H001");
    row["origin"] = json!("新規");
    row["id"] = json!("JAC_POOL_O999");
    row["faction"] = json!("架空の領主");
    row["sources"] = json!([]);
    assert!(
        format!("{:#}", pool::validate(&data).unwrap_err()).contains("60 historical and 240 new")
    );
    let mut data = catalogue();
    for row in data["characters"].as_array_mut().unwrap() {
        if row["era"] == "古代" {
            row["era"] = json!("中世");
        }
    }
    assert!(format!("{:#}", pool::validate(&data).unwrap_err()).contains("all seven eras"));
    let mut data = catalogue();
    row_mut(&mut data, "JAC_POOL_O001")["faction"] = json!("伊達家");
    assert!(format!("{:#}", pool::validate(&data).unwrap_err()).contains("marked 架空"));
    let mut data = catalogue();
    for row in data["characters"].as_array_mut().unwrap() {
        if row["origin"] == "新規" && row["faction"].as_str().unwrap().starts_with("伊達家") {
            row["faction"] = json!("架空の領主");
        }
    }
    assert!(format!("{:#}", pool::validate(&data).unwrap_err()).contains("all 20 houses"));
}

#[test]
fn existing_leader_mappings_cover_each_registered_person_exactly_once() {
    let data = catalogue();
    let mapped: Vec<_> = data["characters"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| !r["existing_leader_id"].is_null())
        .collect();
    assert_eq!(mapped.len(), 22);
    let repo = Repository::checked_in();
    let sql = repo
        .read("poc/JAC_PoC_CustomCivs/Data/JAC_Japan1534_Gameplay.sql")
        .unwrap()
        + &repo
            .read("poc/JAC_PoC_CustomCivs/Data/JAC_CustomCivs_Gameplay.sql")
            .unwrap();
    let mut expected = std::collections::BTreeSet::new();
    for caps in regex::Regex::new(r"\('(?P<id>LEADER_JAC_[A-Z0-9_]+)',\s*'KIND_LEADER'\)")
        .unwrap()
        .captures_iter(&sql)
    {
        expected.insert(caps["id"].to_string());
    }
    let actual: std::collections::BTreeSet<_> = mapped
        .iter()
        .map(|r| r["existing_leader_id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(expected, actual);
    for row in mapped {
        let leader = row["existing_leader_id"].as_str().unwrap();
        assert!(sql.contains(&format!(
            "('{leader}', 'LOC_{leader}_NAME', 'LEADER_DEFAULT'"
        )));
        let tag = format!("LOC_{leader}_NAME");
        let mut names = Vec::new();
        for file in ["JAC_Japan1534_Text.xml", "JAC_CustomCivs_Text.xml"] {
            let xml = repo
                .read(format!("poc/JAC_PoC_CustomCivs/Text/{file}"))
                .unwrap();
            let document = roxmltree::Document::parse(&xml).unwrap();
            for node in document.descendants().filter(|n| {
                n.has_tag_name("Row")
                    && n.attribute("Tag") == Some(tag.as_str())
                    && n.attribute("Language") == Some("ja_JP")
            }) {
                let name = node
                    .children()
                    .find(|n| n.has_tag_name("Text"))
                    .and_then(|n| n.text())
                    .unwrap();
                names.push(name.to_owned());
            }
        }
        assert_eq!(
            names,
            vec![row["name"].as_str().unwrap()],
            "Japanese name for {leader}"
        );
    }
    assert!(data["characters"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == "JAC_POOL_H008"
            && r["name"] == "毛利元就"
            && r["existing_leader_id"] == "LEADER_JAC_MORI_MOTONARI"));
}

#[test]
fn original_person_ids_and_names_remain_stable_even_if_the_roster_is_reordered() {
    let data = catalogue();
    let identities: Vec<Value> =
        serde_json::from_str(include_str!("first-batch-identities.json")).unwrap();
    assert_eq!(identities.len(), 100);
    for identity in identities {
        let row = data["characters"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == identity["id"])
            .unwrap();
        assert_eq!(
            row["name"], identity["name"],
            "stable identity {}",
            identity["id"]
        );
    }
    let mut reordered = data;
    reordered["characters"].as_array_mut().unwrap().reverse();
    pool::validate(&reordered).unwrap();
    row_mut(&mut reordered, "JAC_POOL_H060")["id"] = json!("JAC_POOL_H061");
    assert!(format!("{:#}", pool::validate(&reordered).unwrap_err()).contains("without gaps"));
}

#[test]
fn region_allocation_cannot_be_changed_by_relabeling_one_person() {
    let mut data = catalogue();
    row_mut(&mut data, "JAC_POOL_H001")["era"] = json!("中世");
    assert!(format!("{:#}", pool::validate(&data).unwrap_err()).contains("territory allocation"));
}
