mod common;
use civ6_assets_rs::{
    arr, attr,
    constants::{seat_index, SEATS},
    f, generate as g, n, prosper_start as ps, prosperity as p, read_json, root, s, text_of,
    write_json, xml,
};
use common::*;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

const ALLOWED_BUILDINGS: [&str; 10] = [
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
];
#[test]
fn test_totals_per_tier() {
    for item in &maps().manifest {
        let km = n(&item["km_per_tile"]);
        let db = db(item);
        let pop = p::string_rows(&db, "SELECT ID,Value FROM CityAttributes").unwrap();
        let mut units = BTreeMap::<i32, i32>::new();
        for r in rows(&db, "SELECT Owner FROM Units") {
            *units.entry(n(&r[0])).or_default() += 1;
        }
        let owner = arr(&item["cities"])
            .iter()
            .map(|c| (n(&c["plot"]), s(&c["owner"]).to_owned()))
            .collect::<BTreeMap<_, _>>();
        let districts = g::counts(
            rows(
                &db,
                "SELECT CityID FROM Districts WHERE DistrictType<>'DISTRICT_CITY_CENTER'",
            )
            .iter()
            .map(|r| owner[&n(&r[0])].as_str()),
        );
        let walls = g::counts(
            rows(
                &db,
                "SELECT Plot FROM Buildings WHERE BuildingType='BUILDING_WALLS'",
            )
            .iter()
            .map(|r| owner[&n(&r[0])].as_str()),
        );
        let mut imps = BTreeMap::<i32, i32>::new();
        for r in rows(&db, "SELECT ImprovementOwner FROM PlotImprovements") {
            *imps.entry(n(&r[0])).or_default() += 1;
        }
        let gold = p::string_rows(
            &db,
            "SELECT ID,Value FROM PlayerAttributes WHERE Type='Treasury' AND Name='Balance'",
        )
        .unwrap();
        let mut totals = BTreeMap::<String, [i32; 7]>::new();
        for seat in SEATS.iter() {
            let own = arr(&item["cities"])
                .iter()
                .filter(|c| s(&c["owner"]) == seat.slug)
                .map(|c| n(&c["plot"]))
                .collect::<Vec<_>>();
            let values = [
                1,
                own.iter().map(|c| pop[c].parse::<i32>().unwrap()).sum(),
                *units.get(&seat_index(&seat.slug)).unwrap_or(&0),
                *districts.get(&seat.slug).unwrap_or(&0),
                *walls.get(&seat.slug).unwrap_or(&0),
                *imps.get(&seat_index(&seat.slug)).unwrap_or(&0),
                gold[&seat_index(&seat.slug)]
                    .parse::<f64>()
                    .unwrap()
                    .round() as i32,
            ];
            let t = totals.entry(p::tier_of(&seat.slug).into()).or_default();
            for (i, v) in values.iter().enumerate() {
                t[i] += v;
            }
        }
        let expected = if km == 15 {
            BTreeMap::from([
                ("A".to_owned(), [7, 80, 55, 16, 11, 60, 3850]),
                ("B".into(), [5, 47, 34, 7, 7, 31, 1620]),
                ("C".into(), [4, 47, 27, 9, 8, 29, 1070]),
                ("D".into(), [6, 30, 21, 6, 9, 21, 780]),
            ])
        } else {
            BTreeMap::from([
                ("A".to_owned(), [7, 77, 55, 16, 11, 45, 3850]),
                ("B".into(), [5, 46, 34, 5, 7, 35, 1620]),
                ("C".into(), [4, 47, 27, 6, 8, 27, 1070]),
                ("D".into(), [6, 29, 21, 5, 9, 24, 780]),
            ])
        };
        assert_eq!(totals, expected, "{km} km");
        let mut capital_pop = BTreeMap::<String, Vec<f64>>::new();
        for c in arr(&item["cities"])
            .iter()
            .filter(|c| c["capital"] == true && c["id"] != p::KYOTO)
        {
            capital_pop
                .entry(p::tier_of(s(&c["owner"])).into())
                .or_default()
                .push(pop[&n(&c["plot"])].parse().unwrap());
        }
        for index in [1, 2, 6] {
            let mean = |tier: &str| {
                if index == 1 {
                    capital_pop[tier].iter().sum::<f64>() / capital_pop[tier].len() as f64
                } else {
                    totals[tier][index] as f64 / totals[tier][0] as f64
                }
            };
            assert!(mean("A") > mean("B"));
            assert!(mean("B") > mean("C") - 0.1);
            assert!(mean("C") > mean("D"));
        }
    }
}
#[test]
fn test_every_city_has_one_garrison_of_its_owner() {
    for item in &maps().manifest {
        let db = db(item);
        let units = rows(&db, "SELECT UnitType,Owner,Plot FROM Units");
        for c in arr(&item["cities"]) {
            let own = units
                .iter()
                .filter(|u| u[2] == c["plot"])
                .collect::<Vec<_>>();
            assert_eq!(own.len(), 1);
            assert_eq!(n(&own[0][1]), seat_index(s(&c["owner"])));
            let strong = c["capital"] == true || p::importance(c) >= 2;
            assert_eq!(
                s(&own[0][0]),
                if strong {
                    "UNIT_SPEARMAN"
                } else {
                    "UNIT_WARRIOR"
                }
            );
        }
    }
}
#[test]
fn test_no_unit_on_invalid_terrain() {
    for item in &maps().manifest {
        let db = db(item);
        assert!(rows(&db,"SELECT u.ID FROM Units u JOIN Plots t ON t.ID=u.Plot LEFT JOIN PlotOwners o ON o.ID=u.Plot WHERE t.TerrainType IN ('TERRAIN_COAST','TERRAIN_OCEAN') OR t.IsImpassable<>0 OR t.TerrainType LIKE '%MOUNTAIN' OR o.Owner IS NOT u.Owner").is_empty());
        assert!(rows(
            &db,
            "SELECT Plot FROM Units GROUP BY Plot HAVING COUNT(*)>1"
        )
        .is_empty());
        let units = rows(&db, "SELECT DISTINCT UnitType FROM Units");
        assert!(units
            .iter()
            .all(|r| ["UNIT_WARRIOR", "UNIT_SPEARMAN", "UNIT_ARCHER"].contains(&s(&r[0]))));
        let ids = rows(&db, "SELECT ID FROM Units ORDER BY ID")
            .iter()
            .map(|r| n(&r[0]))
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            (0..count(&db, "SELECT COUNT(*) FROM Units")).collect::<Vec<_>>()
        );
    }
}
#[test]
fn test_districts_buildings_and_improvements_follow_the_rules() {
    for item in &maps().manifest {
        let db = db(item);
        let grid = p::grid(item);
        let features = ps::settler_features(&maps().dir, grid.km)
            .unwrap()
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let resource = p::string_rows(&db, "SELECT ID,ResourceType FROM PlotResources").unwrap();
        let terrain = p::string_rows(&db, "SELECT ID,TerrainType FROM Plots").unwrap();
        let owner = p::owner_rows(&db)
            .unwrap()
            .into_iter()
            .map(|(p, o, c)| (p, (o, c)))
            .collect::<BTreeMap<_, _>>();
        let mut district_at = BTreeMap::new();
        for row in rows(
            &db,
            "SELECT DistrictType,CityID,Plot FROM Districts WHERE DistrictType<>'DISTRICT_CITY_CENTER'",
        ) {
            let (dtype, city, plot) = (s(&row[0]), n(&row[1]), n(&row[2]));
            district_at.insert(plot, dtype.to_owned());
            let distance = g::hex_distance(grid.xy(plot), grid.xy(city));
            assert_eq!(owner[&plot].1, city);
            assert!((1..=ps::DISTRICT_RANGE).contains(&distance));
            assert!(!resource.contains_key(&plot));
            assert!(!features.contains_key(&plot));
            if dtype == "DISTRICT_ENCAMPMENT" {
                assert!(distance >= 2);
            }
            if dtype == "DISTRICT_HARBOR" {
                assert_eq!(terrain[&plot], "TERRAIN_COAST");
            } else {
                assert!(!p::water(&terrain[&plot]));
                assert!(!terrain[&plot].ends_with("MOUNTAIN"));
            }
        }
        let cities = arr(&item["cities"])
            .iter()
            .map(|c| n(&c["plot"]))
            .collect::<BTreeSet<_>>();
        for row in rows(&db, "SELECT BuildingType,Plot FROM Buildings") {
            let (building, plot) = (s(&row[0]), n(&row[1]));
            assert!(ALLOWED_BUILDINGS.contains(&building));
            let district = if building == "BUILDING_TEMPLE" {
                Some("DISTRICT_HOLY_SITE")
            } else {
                p::BUILDINGS
                    .iter()
                    .position(|&b| b == building)
                    .map(|i| p::DISTRICTS[i])
            };
            if let Some(d) = district {
                assert_eq!(district_at[&plot], d);
            } else {
                assert!(cities.contains(&plot));
            }
        }
        let walls = rows(
            &db,
            "SELECT Plot FROM Buildings WHERE BuildingType='BUILDING_WALLS'",
        )
        .iter()
        .map(|r| n(&r[0]))
        .collect::<BTreeSet<_>>();
        assert_eq!(
            walls,
            arr(&item["cities"])
                .iter()
                .filter(|c| !p::no_walls(s(&c["id"]))
                    && (c["capital"] == true || p::importance(c) >= 2))
                .map(|c| n(&c["plot"]))
                .collect()
        );
        for row in rows(
            &db,
            "SELECT ID,ImprovementType,ImprovementOwner FROM PlotImprovements",
        ) {
            let (plot, imp, o) = (n(&row[0]), s(&row[1]), n(&row[2]));
            assert!(["IMPROVEMENT_FARM", "IMPROVEMENT_MINE", "IMPROVEMENT_QUARRY"].contains(&imp));
            assert_eq!(owner[&plot].0, o);
            assert!(!district_at.contains_key(&plot));
            assert!(!cities.contains(&plot));
            assert!(!features.contains_key(&plot));
            assert_eq!(
                ps::improvement_for(
                    &terrain[&plot],
                    None,
                    resource.get(&plot).map(String::as_str)
                ),
                Some(imp)
            );
        }
    }
}
#[test]
fn test_no_forest_or_marsh_is_removed() {
    for item in &maps().manifest {
        let base = ps::settler_features(&maps().dir, n(&item["km_per_tile"])).unwrap();
        assert!(!base.is_empty());
        let actual = p::string_rows(&db(item), "SELECT ID,FeatureType FROM PlotFeatures")
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(actual, base);
        let kinds = base
            .iter()
            .map(|(_, k)| k.as_str())
            .collect::<BTreeSet<_>>();
        assert!(BTreeSet::from(["FEATURE_FOREST", "FEATURE_MARSH"]).is_subset(&kinds));
    }
}
#[test]
fn test_kyoto_and_treasury() {
    for item in &maps().manifest {
        let db = db(item);
        let kyoto = arr(&item["cities"])
            .iter()
            .find(|c| c["id"] == p::KYOTO)
            .unwrap();
        let plot = n(&kyoto["plot"]);
        assert_eq!(
            rows_params(&db, "SELECT Value FROM CityAttributes WHERE ID=?", [plot]),
            vec![vec![json!("6")]]
        );
        let built=rows_params(&db,"SELECT d.DistrictType,b.BuildingType FROM Districts d JOIN Buildings b ON b.Plot=d.Plot WHERE d.CityID=?",[plot]).into_iter().map(|r|(s(&r[0]).to_owned(),s(&r[1]).to_owned())).collect::<BTreeSet<_>>();
        let mut want = BTreeSet::from([(
            "DISTRICT_COMMERCIAL_HUB".to_owned(),
            "BUILDING_MARKET".to_owned(),
        )]);
        if item["km_per_tile"] == 15 {
            want.extend([
                ("DISTRICT_HOLY_SITE".into(), "BUILDING_SHRINE".into()),
                ("DISTRICT_HOLY_SITE".into(), "BUILDING_TEMPLE".into()),
            ]);
        }
        assert!(want.is_subset(&built));
        assert_eq!(
            rows(
                &db,
                "SELECT ID,Value FROM PlayerAttributes WHERE Type='Treasury' ORDER BY ID"
            ),
            SEATS
                .iter()
                .enumerate()
                .map(|(i, s)| vec![
                    json!(i),
                    json!(format!("{}.000000", p::starting_gold(&s.slug)))
                ])
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(p::starting_gold("ODA"), 200);
    assert_eq!(p::starting_gold("MORI"), p::GOLD_FLOOR);
    assert_eq!(p::starting_gold("ASHIKAGA"), 370);
}
#[test]
fn test_settler_maps_stay_bare() {
    for item in &maps().manifest {
        let db = variant(item);
        for t in [
            "Units",
            "PlotImprovements",
            "PlayerAttributes",
            "Buildings",
            "CityAttributes",
        ] {
            assert_eq!(
                rows(&db, &format!("SELECT COUNT(*) FROM {t}")),
                vec![vec![json!(0)]]
            );
        }
    }
}
#[test]
fn test_apply_is_deterministic_and_matches_the_checked_in_maps() {
    for item in &maps().manifest {
        let t = temp();
        let path = t.path().join("map.Civ6Map");
        fs::copy(ps::map_path(&maps().dir, n(&item["km_per_tile"])), &path).unwrap();
        let km = n(&item["km_per_tile"]);
        let first = ps::apply_file(&path, &maps().dir, km).unwrap();
        let db_copy = rusqlite::Connection::open(&path).unwrap();
        assert_eq!(table_dump(&db_copy), table_dump(&db(item)));
        drop(db_copy);
        assert_eq!(ps::apply_file(&path, &maps().dir, km).unwrap(), first);
        let db_copy = rusqlite::Connection::open(&path).unwrap();
        assert_eq!(table_dump(&db_copy), table_dump(&db(item)));
        assert!(first
            .as_object()
            .unwrap()
            .values()
            .all(|r| arr(&r["unplaced"]).is_empty()));
        let missing = first
            .as_object()
            .unwrap()
            .iter()
            .filter(|(_, r)| n(&r["missing_districts"]) != 0)
            .map(|(slug, r)| (slug.clone(), n(&r["missing_districts"])))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            missing,
            if km == 15 {
                BTreeMap::new()
            } else {
                BTreeMap::from([("MATSUDAIRA".into(), 1), ("ASHIKAGA".into(), 1)])
            }
        );
    }
}
#[test]
fn test_improvement_rules() {
    for (t, r, want) in [
        ("TERRAIN_GRASS", None, Some("IMPROVEMENT_FARM")),
        ("TERRAIN_GRASS_HILLS", None, Some("IMPROVEMENT_MINE")),
        (
            "TERRAIN_GRASS_HILLS",
            Some("RESOURCE_IRON"),
            Some("IMPROVEMENT_MINE"),
        ),
        (
            "TERRAIN_PLAINS",
            Some("RESOURCE_SALT"),
            Some("IMPROVEMENT_MINE"),
        ),
        (
            "TERRAIN_GRASS",
            Some("RESOURCE_RICE"),
            Some("IMPROVEMENT_FARM"),
        ),
        ("TERRAIN_GRASS", Some("RESOURCE_HORSES"), None),
        ("TERRAIN_GRASS_MOUNTAIN", None, None),
    ] {
        assert_eq!(ps::improvement_for(t, None, r), want);
    }
    assert_eq!(
        ps::improvement_for("TERRAIN_GRASS", Some("FEATURE_MARSH"), None),
        None
    );
}
fn scenario() -> Value {
    read_json(root().join("tools/civ6-autotest/scenarios/japan-prosperous-15km.json")).unwrap()
}
#[test]
fn test_live_scenario_is_current_and_valid() {
    let data = scenario();
    let steps = arr(&data["steps"]);
    assert!(!s(&data["name"]).is_empty());
    assert!(!steps.is_empty());
    for step in steps {
        assert!(step.is_object());
        let action = s(&step["action"]);
        assert!([
            "enable_mods",
            "configure",
            "start_game",
            "ui_lua",
            "autoplay",
            "wait_for",
            "save",
            "exit_to_menu",
            "load"
        ]
        .contains(&action));
        if action == "ui_lua" {
            assert!(!s(&step["state_name"]).is_empty());
            assert!(!s(&step["code"]).is_empty());
        }
        if action == "autoplay" {
            assert!(n(&step["turns"]) > 0);
            assert!(f(&step["timeout"]) > 0.);
        }
    }
    let t = temp();
    let path = t.path().join("scenario.json");
    write_json(&path, &data).unwrap();
    ps::write_scenario(&ps::report_for(&maps().dir, 15).unwrap(), &path).unwrap();
    assert_eq!(read_json(path).unwrap(), data);
    assert!(s(&steps[1]["map_file"]).contains("JAC_Japan_15km.Civ6Map"));
    assert_eq!(
        arr(&steps[1]["players"])
            .iter()
            .filter(|p| p["human"] == true)
            .map(|p| s(&p["leader"]))
            .collect::<Vec<_>>(),
        vec!["LEADER_JAC_OUCHI_YOSHITAKA"]
    );
    assert_eq!(steps[3]["action"], "ui_lua");
    let code = s(&steps[3]["code"]);
    for text in [
        "ungarrisoned cities",
        "stacked plots",
        "LEADER_JAC_ODA_NOBUHIDE={2,5,4,",
    ] {
        assert!(code.contains(text));
    }
    assert!(code.contains(&format!("Game:GetProperty('{}')", ps::START_STATE_KEY)));
    assert_eq!(
        steps
            .iter()
            .filter(|s| s["action"] == "autoplay")
            .map(|s| n(&s["turns"]))
            .sum::<i32>(),
        50
    );
    assert_eq!(s(&steps.last().unwrap()["code"]), ps::SURVIVORS);
}
#[test]
fn test_survivor_step_fails_on_a_death_or_a_lost_capital() {
    for text in [
        "if #alive<22 or nocap>0 then error('regression: '..line) end; return line",
        "if not cap then nocap=nocap+1 end",
        "for i=0,21 do",
    ] {
        assert!(ps::SURVIVORS.contains(text));
    }
}
#[test]
fn test_start_check_compares_every_planned_building() {
    let item = maps()
        .manifest
        .iter()
        .find(|i| i["km_per_tile"] == 15)
        .unwrap();
    let db = db(item);
    let owner = p::owner_rows(&db)
        .unwrap()
        .into_iter()
        .map(|(p, o, _)| (p, o))
        .collect::<BTreeMap<_, _>>();
    let mut in_map = BTreeMap::<String, BTreeMap<String, i32>>::new();
    for row in rows(&db, "SELECT BuildingType,Plot FROM Buildings") {
        *in_map
            .entry(SEATS[owner[&n(&row[1])] as usize].slug.clone())
            .or_default()
            .entry(s(&row[0]).into())
            .or_default() += 1;
    }
    let report = ps::report_for(&maps().dir, 15).unwrap();
    let scenario = scenario();
    let code = s(&scenario["steps"][3]["code"]);
    assert!(code.contains(&format!("Game:GetProperty('{}')", ps::START_BUILDINGS_KEY)));
    for (slug, row) in report.as_object().unwrap() {
        assert_eq!(row["buildings"], json!(in_map[slug]));
        assert_eq!(row["buildings"]["BUILDING_PALACE"], 1);
        let table = row["buildings"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| format!("{k}={}", n(v)))
            .collect::<Vec<_>>()
            .join(",");
        assert!(code.contains(&format!(
            "{}={{{table}}}",
            civ6_assets_rs::start_1534::leader_id(slug)
        )));
    }
    assert_eq!(
        report
            .as_object()
            .unwrap()
            .values()
            .map(|r| r["buildings"]
                .as_object()
                .unwrap()
                .values()
                .map(n)
                .sum::<i32>())
            .sum::<i32>(),
        count(&db, "SELECT COUNT(*) FROM Buildings")
    );
}
#[test]
fn test_start_state_script_runs_only_on_the_historical_maps() {
    let script =
        fs::read_to_string(checked_map_mod().join("Scripts/JAC_Japan_StartState.lua")).unwrap();
    for text in [
        format!("local KEY = \"{}\"", ps::START_STATE_KEY),
        format!("local BUILDINGS_KEY = \"{}\"", ps::START_BUILDINGS_KEY),
        "for row in GameInfo.Buildings() do".into(),
        "GameConfiguration.GetStartTurn()".into(),
    ] {
        assert!(script.contains(&text));
    }
    let m = xml(checked_map_mod().join("JAC_PoC_JapanMapScale.modinfo")).unwrap();
    let criteria = find_all(&m, &["ActionCriteria", "Criteria"])
        .iter()
        .map(|c| {
            (
                attr(c, "id").to_owned(),
                c.get_child("ConfigurationValueMatches")
                    .map(|v| text_of(v, "Value"))
                    .unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let actions = find_all(&m, &["InGameActions", "AddGameplayScripts"])
        .into_iter()
        .filter(|a| attr(a, "id").ends_with("_StartState"))
        .collect::<Vec<_>>();
    assert_eq!(
        actions
            .iter()
            .map(|a| text_of(a, "File"))
            .collect::<Vec<_>>(),
        vec!["Scripts/JAC_Japan_StartState.lua"; 2]
    );
    assert_eq!(
        actions
            .iter()
            .map(|a| criteria[attr(a, "criteria")].clone())
            .collect::<BTreeSet<_>>(),
        civ6_assets_rs::constants::SCALES
            .iter()
            .map(|km| format!(
                "{{{}}}Maps/JAC_Japan_{km}km.Civ6Map",
                civ6_assets_rs::write_support::MOD_ID
            ))
            .collect()
    );
    assert!(find_all(&m, &["Files", "File"])
        .iter()
        .any(|f| f.get_text().unwrap() == "Scripts/JAC_Japan_StartState.lua"));
}
