use civ6_data_rs::{pool, Repository};
use serde_json::{json, Value};

fn catalogue() -> Value {
    Repository::checked_in().json(pool::DATA_PATH).unwrap()
}

#[test]
fn checked_in_first_batch_is_valid_and_proposed() {
    pool::validate(&catalogue()).unwrap();
}

#[test]
fn rejects_invalid_records_without_weakening_the_first_batch_contract() {
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
            "武将: expected 40",
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
                data["characters"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("faction");
            }
            1 => {
                data["characters"][0]["typo"] = json!("unexpected");
            }
            2 => {
                data["characters"][0] = json!([]);
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
        data["characters"][0]["sources"][0][field] = value;
        let error = format!("{:#}", pool::validate(&data).unwrap_err());
        assert!(error.contains(expected), "{field}: {error}");
    }
    let mut data = catalogue();
    data["characters"][20]["sources"] = data["characters"][0]["sources"].clone();
    assert!(format!("{:#}", pool::validate(&data).unwrap_err()).contains("new rows must have none"));
}

#[test]
fn rejects_incomplete_catalogue_and_premature_approval() {
    for (field, value) in [
        ("target_size", json!(100)),
        ("status", json!("承認済み")),
        ("schema_version", json!(2)),
    ] {
        let mut data = catalogue();
        data[field] = value;
        assert!(pool::validate(&data).is_err());
    }
    let mut data = catalogue();
    data["characters"].as_array_mut().unwrap().pop();
    assert!(format!("{:#}", pool::validate(&data).unwrap_err()).contains("contain 100"));
    let mut data = catalogue();
    for row in data["characters"].as_array_mut().unwrap() {
        row["leader_candidate"] = json!(false);
    }
    assert!(format!("{:#}", pool::validate(&data).unwrap_err())
        .contains("two distinct leader candidates"));
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
        data["characters"][0][field] = value;
        assert!(format!("{:#}", pool::validate(&data).unwrap_err()).contains(expected));
    }
    let mut data = catalogue();
    data["characters"][0]["sources"][0]
        .as_object_mut()
        .unwrap()
        .remove("facts");
    assert!(
        format!("{:#}", pool::validate(&data).unwrap_err()).contains("missing or unknown field")
    );
    let mut data = catalogue();
    data["characters"][0]["origin"] = json!("新規");
    data["characters"][0]["id"] = json!("JAC_POOL_O099");
    data["characters"][0]["faction"] = json!("架空の領主");
    data["characters"][0]["sources"] = json!([]);
    assert!(
        format!("{:#}", pool::validate(&data).unwrap_err()).contains("20 historical and 80 new")
    );
    let mut data = catalogue();
    for row in data["characters"].as_array_mut().unwrap() {
        if row["era"] == "古代" {
            row["era"] = json!("中世");
        }
    }
    assert!(format!("{:#}", pool::validate(&data).unwrap_err()).contains("all seven eras"));
    let mut data = catalogue();
    data["characters"][20]["faction"] = json!("伊達家");
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
fn existing_leader_mapping_refers_to_the_same_registered_person() {
    let data = catalogue();
    let mapped: Vec<_> = data["characters"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| !r["existing_leader_id"].is_null())
        .collect();
    assert_eq!(mapped.len(), 1);
    assert_eq!(mapped[0]["name"], "毛利元就");
    let leader = mapped[0]["existing_leader_id"].as_str().unwrap();
    let text = Repository::checked_in()
        .read("poc/JAC_PoC_CustomCivs/Data/JAC_Japan1534_Gameplay.sql")
        .unwrap();
    assert!(text.contains(&format!(
        "('{leader}', 'LOC_{leader}_NAME', 'LEADER_DEFAULT'"
    )));
}
