mod common;
use civ6_assets_rs::{arr, f, n, prosperity as p, read_json, s};
use common::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::LazyLock;
static PLAN: LazyLock<Value> = LazyLock::new(|| p::build(&maps().dir).unwrap());
#[test]
fn test_json_is_current() {
    fn differences(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
        match (a, b) {
            (Value::Object(a), Value::Object(b)) => {
                for (k, v) in a {
                    match b.get(k) {
                        Some(other) => differences(v, other, &format!("{path}/{k}"), out),
                        None => out.push(format!("{path}/{k}: missing")),
                    }
                }
                for k in b.keys().filter(|k| !a.contains_key(*k)) {
                    out.push(format!("{path}/{k}: unexpected"));
                }
            }
            (Value::Array(a), Value::Array(b)) => {
                if a.len() != b.len() {
                    out.push(format!("{path}: lengths {} != {}", a.len(), b.len()));
                }
                for (i, (a, b)) in a.iter().zip(b).enumerate() {
                    differences(a, b, &format!("{path}/{i}"), out);
                }
            }
            _ => {
                if a != b {
                    out.push(format!("{path}: {a} != {b}"));
                }
            }
        }
    }
    let mut diff = vec![];
    differences(
        &read_json(fixtures().join("prosperity-reference.json")).unwrap(),
        &PLAN,
        "",
        &mut diff,
    );
    assert!(diff.is_empty(), "{}", diff.join("\n"));
    assert_eq!(
        *PLAN,
        serde_json::from_str::<Value>(&serde_json::to_string(&*PLAN).unwrap()).unwrap()
    );
}
#[test]
fn test_covers_22_factions_and_56_cities() {
    assert_eq!(arr(&PLAN["factions"]).len(), 22);
    let cities = arr(&PLAN["factions"])
        .iter()
        .flat_map(|f| arr(&f["plan"]))
        .collect::<Vec<_>>();
    assert_eq!(cities.len(), 56);
    assert_eq!(
        cities
            .iter()
            .map(|c| s(&c["id"]))
            .collect::<BTreeSet<_>>()
            .len(),
        56
    );
}
#[test]
fn test_every_city_is_garrisoned_and_capped_by_land() {
    for c in arr(&PLAN["factions"]).iter().flat_map(|f| arr(&f["plan"])) {
        assert_eq!(
            arr(&c["units"])
                .iter()
                .filter(|u| u["where"] == "center")
                .count(),
            1
        );
        assert!(n(&c["pop_15km"]) >= 2);
        assert!(n(&c["pop_15km"]) <= n(&c["land_15km"]).max(2));
        assert_eq!(
            c["capital"] == true,
            arr(&c["city_buildings"]).contains(&json!("BUILDING_PALACE"))
        );
        assert!(n(&c["pop_15km"]) + arr(&c["districts"]).len() as i32 <= n(&c["land_15km"]).max(2));
        assert!(
            c["improvements"]
                .as_object()
                .unwrap()
                .values()
                .map(n)
                .sum::<i32>()
                <= n(&c["pop_15km"]) - arr(&c["districts"]).len() as i32
        );
    }
}
#[test]
fn test_only_buildings_and_units_kept_by_the_sengoku_trees() {
    let buildings = BTreeSet::from([
        "BUILDING_PALACE",
        "BUILDING_WALLS",
        "BUILDING_GRANARY",
        "BUILDING_MARKET",
        "BUILDING_LIGHTHOUSE",
        "BUILDING_BARRACKS",
        "BUILDING_SHRINE",
        "BUILDING_TEMPLE",
        "BUILDING_WORKSHOP",
        "BUILDING_LIBRARY",
    ]);
    let units = BTreeSet::from(["UNIT_WARRIOR", "UNIT_SPEARMAN", "UNIT_ARCHER"]);
    for c in arr(&PLAN["factions"]).iter().flat_map(|f| arr(&f["plan"])) {
        let used = arr(&c["city_buildings"])
            .iter()
            .chain(
                arr(&c["districts"])
                    .iter()
                    .flat_map(|d| arr(&d["buildings"]).iter()),
            )
            .map(s)
            .collect::<BTreeSet<_>>();
        assert!(used.is_subset(&buildings));
        assert!(arr(&c["units"])
            .iter()
            .map(|u| s(&u["type"]))
            .collect::<BTreeSet<_>>()
            .is_subset(&units));
    }
}
#[test]
fn test_koku_shares_and_tiers() {
    for shares in p::SPEC["split_shares"].as_object().unwrap().values() {
        assert!((shares.as_object().unwrap().values().map(f).sum::<f64>() - 1.).abs() < 0.5e-7);
    }
    let factions = arr(&PLAN["factions"]);
    let faction = |id: &str| factions.iter().find(|f| f["id"] == id).unwrap();
    assert_eq!(faction("OUCHI")["tier"], "A");
    assert_eq!(faction("ODA")["tier"], "D");
    assert!(p::field_units("OUCHI") > p::field_units("ODA"));
    assert_eq!(
        (s(&faction("RYUKYU")["tier"]), s(&faction("AINU")["tier"])),
        ("C", "D")
    );
    assert!(faction("AINU")["direct_koku_share_pct"].is_null());
    assert!(
        (factions
            .iter()
            .filter(|f| !f["direct_koku_share_pct"].is_null())
            .map(|v| f(&v["direct_koku_share_pct"]))
            .sum::<f64>()
            - 100.)
            .abs()
            <= 0.5
    );
    assert!(p::SPEC["factions"]
        .as_object()
        .unwrap()
        .values()
        .all(|v| arr(&v["k"]).len() == 3
            && f(&v["k"][0]) <= f(&v["k"][1])
            && f(&v["k"][1]) <= f(&v["k"][2])));
}
