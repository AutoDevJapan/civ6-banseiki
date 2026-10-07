mod common;
use civ6_assets_rs::{
    arr, attr, constants, elements, f, generate as g, n, prosperity, root, round, s, start_1534,
    text_of, write_custom_civs, write_support, xml,
};
use common::*;
use constants::{seat_index, SEATS};
use g::{hex_distance, HexGrid, Tiles};
use rusqlite::{params, Connection};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};
fn set(values: impl Iterator<Item = i32>) -> Tiles {
    values.collect()
}
fn xy(c: &serde_json::Value) -> (i32, i32) {
    (n(&c["col"]), n(&c["row"]))
}
fn approximate(a: f64, b: f64, tolerance: f64) {
    assert!((a - b).abs() <= tolerance, "{a} != {b}");
}
#[test]
fn test_round_trip_across_japan() {
    for (lon, lat) in [
        (127.719, 26.218),
        (135.76, 35.03),
        (142.149, 42.633),
        (123., 24.5),
        (145., 45.),
    ] {
        let (x, y) = g::project(lon, lat);
        let a = g::unproject(x, y);
        approximate(a.0, lon, 0.5e-8);
        approximate(a.1, lat, 0.5e-8);
    }
}
#[test]
fn test_hex_neighbours_and_odd_row_offset() {
    let grid = HexGrid {
        km: 20,
        width: 8,
        height: 8,
        xmin: 0.,
        ymin: 0.,
    };
    approximate(grid.centre(2, 1).0 - grid.centre(2, 0).0, 10., 1e-8);
    assert_eq!(grid.neighbours(3, 3).len(), 6);
    for r in [2, 3] {
        for (c, other) in grid.neighbours(3, r) {
            assert!(grid.neighbours(c, other).contains(&(3, r)));
            approximate(
                g::distance(grid.centre(3, r), grid.centre(c, other)),
                20.,
                0.5e-8,
            );
        }
    }
}
#[test]
fn test_closest_centre() {
    let grid = HexGrid {
        km: 25,
        width: 10,
        height: 10,
        xmin: -100.,
        ymin: -100.,
    };
    for (c, r) in [(1, 2), (4, 5), (8, 8)] {
        let p = grid.centre(c, r);
        let p = g::unproject(p.0, p.1);
        assert_eq!(grid.closest(p.0, p.1), (c, r));
    }
}
#[test]
fn test_schema_matches_official_and_plot_inventory() {
    let expected = &maps().official["schema_map"];
    assert_eq!(&maps().official["xp2_schema"], expected);
    for item in &maps().manifest {
        let db = db(item);
        let schema: BTreeMap<_, _> =
            rows(&db, "SELECT name,sql FROM sqlite_master WHERE type='table'")
                .into_iter()
                .map(|r| (s(&r[0]).to_owned(), r[1].clone()))
                .collect();
        assert_eq!(json!(schema), *expected);
        assert_eq!(rows(&db, "PRAGMA integrity_check"), vec![vec![json!("ok")]]);
        assert_eq!(
            rows(&db, "SELECT Width,Height,WrapX,WrapY,MapSizeType FROM Map"),
            vec![vec![
                item["width"].clone(),
                item["height"].clone(),
                json!(0),
                json!(0),
                json!("MAPSIZE_HUGE")
            ]]
        );
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM Plots"),
            n(&item["width"]) * n(&item["height"])
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM Plots WHERE ContinentType='CONTINENT_ASIA'"
            ),
            n(&item["land_tiles"])
        );
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM PlotResources"),
            n(&item["resource_placement"]["placed_count"])
        );
        assert!(
            fs::metadata(maps().dir.join(s(&item["file"])))
                .unwrap()
                .len()
                <= 10_000_000
        );
    }
}
#[test]
fn test_known_land_and_removed_foreign_land() {
    let land = [
        (139.69, 35.68),
        (141.35, 43.06),
        (127.719, 26.218),
        (130.56, 31.60),
    ]
    .into_iter()
    .chain(
        g::ISLAND_ANCHORS
            .into_iter()
            .map(|(_, lon, lat)| (lon, lat)),
    )
    .collect::<Vec<_>>();
    let water = [(129.07, 35.18), (135., 39.), (132., 38.5)];
    for item in &maps().manifest {
        let grid = HexGrid::for_scale(n(&item["km_per_tile"]), &maps().japan).unwrap();
        let db = db(item);
        for &(lon, lat) in &land {
            let (c, r) = grid.closest(lon, lat);
            let t = rows_params(
                &db,
                "SELECT TerrainType FROM Plots WHERE ID=?",
                [grid.index(c, r)],
            );
            assert!([
                "TERRAIN_GRASS",
                "TERRAIN_GRASS_HILLS",
                "TERRAIN_GRASS_MOUNTAIN",
                "TERRAIN_PLAINS"
            ]
            .contains(&s(&t[0][0])));
        }
        for (lon, lat) in water {
            let (c, r) = grid.closest(lon, lat);
            let t = rows_params(
                &db,
                "SELECT TerrainType FROM Plots WHERE ID=?",
                [grid.index(c, r)],
            );
            assert!(["TERRAIN_COAST", "TERRAIN_OCEAN"].contains(&s(&t[0][0])));
        }
    }
}
#[test]
fn test_22_starts_unique_and_traversable() {
    for item in &maps().manifest {
        let starts = arr(&item["starts"]);
        assert_eq!(starts.len(), 22);
        assert_eq!(set(starts.iter().map(|s| n(&s["plot"]))).len(), 22);
        assert_eq!(
            starts
                .iter()
                .map(|v| s(&v["seat"]))
                .collect::<BTreeSet<_>>(),
            SEATS.iter().map(|v| v.slug.as_str()).collect()
        );
        let db = db(item);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM StartPositions"), 22);
        for start in starts {
            let row = &rows_params(
                &db,
                "SELECT TerrainType,IsImpassable FROM Plots WHERE ID=?",
                [n(&start["plot"])],
            )[0];
            assert!(s(&row[0]).starts_with("TERRAIN_GRASS"));
            assert_eq!(row[1], 0);
            assert_eq!(
                rows_params(
                    &db,
                    "SELECT Type,Value FROM StartPositions WHERE Plot=?",
                    [n(&start["plot"])]
                ),
                vec![vec![json!("LEADER"), start["leader"].clone()]]
            );
            assert!(f(&start["displacement_km"]) <= 30.);
        }
        assert_eq!(
            item["close_start_pairs"],
            json!(g::close_start_pairs(starts))
        );
    }
}
#[test]
fn test_oda_has_a_passable_neighbouring_domain_for_initial_route_check() {
    for item in &maps().manifest {
        let db = db(item);
        let grid = prosperity::grid(item);
        let start = arr(&item["starts"])
            .iter()
            .find(|c| c["seat"] == "ODA")
            .unwrap();
        let mut candidates = vec![];
        for (c, r) in grid.neighbours(n(&start["col"]), n(&start["row"])) {
            let plot = grid.index(c, r);
            let owner = rows_params(&db, "SELECT Owner FROM PlotOwners WHERE ID=?", [plot]);
            let terrain = rows_params(
                &db,
                "SELECT TerrainType,IsImpassable FROM Plots WHERE ID=?",
                [plot],
            );
            let city = rows_params(&db, "SELECT 1 FROM Cities WHERE Plot=?", [plot]);
            if !owner.is_empty()
                && owner[0][0] != 0
                && s(&terrain[0][0]).starts_with("TERRAIN_GRASS")
                && terrain[0][1] == 0
                && city.is_empty()
            {
                candidates.push(plot);
            }
        }
        assert!(!candidates.is_empty());
    }
}
#[test]
fn test_historical_players_castles_and_territory() {
    let draft = start_1534::data().unwrap();
    assert_eq!(arr(&draft["castles"]).len(), 53);
    let ids = arr(&draft["castles"])
        .iter()
        .map(|c| s(&c["id"]))
        .chain(["ODA_SEAT", "DATE_SEAT", "ASHIKAGA_SEAT"])
        .collect::<BTreeSet<_>>();
    for item in &maps().manifest {
        let db = db(item);
        let cities = arr(&item["cities"]);
        assert_eq!(cities.len(), 56);
        assert_eq!(
            cities.iter().map(|c| s(&c["id"])).collect::<BTreeSet<_>>(),
            ids
        );
        assert_eq!(
            cities[..22]
                .iter()
                .map(|c| s(&c["owner"]))
                .collect::<Vec<_>>(),
            SEATS.iter().map(|s| s.slug.as_str()).collect::<Vec<_>>()
        );
        assert!(cities[..22].iter().all(|c| c["capital"] == true));
        assert!(cities[22..].iter().all(|c| c["capital"] == false));
        assert_eq!(
            cities[22..]
                .iter()
                .map(|c| n(&c["research_priority"]))
                .collect::<Vec<_>>(),
            arr(&draft["castles"])
                .iter()
                .enumerate()
                .filter(|(_, c)| !SEATS
                    .iter()
                    .any(|seat| constants::seat_castle(&seat.slug) == Some(s(&c["id"]))))
                .map(|(i, _)| i as i32 + 1)
                .collect::<Vec<_>>()
        );
        for castle in arr(&draft["castles"]) {
            let c = cities.iter().find(|c| c["id"] == castle["id"]).unwrap();
            assert_eq!(c["owner"], castle["owner_faction_1534_draft"]);
            assert!(
                f(&c["displacement_km"])
                    <= 20. + g::MAX_NUDGE_TILES as f64 * f(&item["km_per_tile"])
            );
        }
        for seat in SEATS.iter() {
            let id = constants::seat_castle(&seat.slug)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{}_SEAT", seat.slug));
            let c = cities.iter().find(|c| c["id"] == id).unwrap();
            assert_eq!(c["owner"], seat.slug);
            assert_eq!(c["capital"], true);
            assert_eq!(
                rows_params(
                    &db,
                    "SELECT Plot FROM Cities WHERE Owner=? AND Name=?",
                    params![
                        seat_index(&seat.slug),
                        format!("LOC_JAC_1534_CAPITAL_{}", seat.slug)
                    ]
                ),
                vec![vec![c["plot"].clone()]]
            );
        }
        assert_eq!(
            rows(
                &db,
                "SELECT ID,CivilizationType,LeaderType FROM Players ORDER BY ID"
            ),
            SEATS
                .iter()
                .enumerate()
                .map(|(i, s)| vec![json!(i), json!(s.civ), json!(s.leader)])
                .collect::<Vec<_>>()
        );
        assert_eq!(
            rows(&db, "SELECT ID,Status FROM Players ORDER BY ID"),
            (0..22)
                .map(|i| vec![json!(i), json!("Human")])
                .collect::<Vec<_>>()
        );
        let mut expected = cities
            .iter()
            .map(|c| {
                vec![
                    json!(seat_index(s(&c["owner"]))),
                    c["plot"].clone(),
                    json!(g::city_name_tag(c)),
                ]
            })
            .collect::<Vec<_>>();
        let mut actual = rows(&db, "SELECT Owner,Plot,Name FROM Cities");
        expected.sort_by_key(|r| (n(&r[0]), n(&r[1])));
        actual.sort_by_key(|r| (n(&r[0]), n(&r[1])));
        assert_eq!(actual, expected);
        let mut expected = cities
            .iter()
            .map(|c| {
                vec![
                    json!("DISTRICT_CITY_CENTER"),
                    c["plot"].clone(),
                    c["plot"].clone(),
                ]
            })
            .collect::<Vec<_>>();
        let mut actual=rows(&db,"SELECT DistrictType,CityID,Plot FROM Districts WHERE DistrictType='DISTRICT_CITY_CENTER'");
        expected.sort_by_key(|r| n(&r[1]));
        actual.sort_by_key(|r| n(&r[1]));
        assert_eq!(actual, expected);
        let mut expected = cities
            .iter()
            .map(|c| vec![c["plot"].clone(), json!("Population"), json!("Initial")])
            .collect::<Vec<_>>();
        expected.sort_by_key(|r| n(&r[0]));
        assert_eq!(
            rows(&db, "SELECT ID,Type,Name FROM CityAttributes ORDER BY ID"),
            expected
        );
        assert_eq!(count(&db,"SELECT COUNT(*) FROM PlotOwners o JOIN Plots p ON p.ID=o.ID WHERE p.ContinentType='CONTINENT_ASIA'"),n(&item["land_tiles"]));
        for sql in ["SELECT COUNT(*) FROM Plots p LEFT JOIN PlotOwners o ON p.ID=o.ID WHERE p.ContinentType='CONTINENT_ASIA' AND o.ID IS NULL","SELECT COUNT(*) FROM Cities c JOIN Plots p ON c.Plot=p.ID WHERE p.ContinentType!='CONTINENT_ASIA' OR p.IsImpassable!=0","SELECT COUNT(*) FROM PlotOwners o LEFT JOIN Cities c ON o.CityOwner=c.Plot WHERE c.Plot IS NULL OR c.Owner!=o.Owner OR o.CityWorking!=o.CityOwner","SELECT COUNT(*) FROM Cities c JOIN PlotOwners o ON c.Plot=o.ID WHERE c.Owner!=o.Owner OR o.CityOwner!=c.Plot"]{assert_eq!(count(&db,sql),0);}
    }
}
#[test]
fn test_settler_variant_has_no_premade_state() {
    for item in &maps().manifest {
        let db = variant(item);
        for table in ["Players", "Cities", "Districts", "PlotOwners", "Units"] {
            assert_eq!(count(&db, &format!("SELECT COUNT(*) FROM {table}")), 0);
        }
        assert_eq!(count(&db, "SELECT COUNT(*) FROM StartPositions"), 22);
        let h = common::db(item);
        for t in ["PlotResources", "PlotRivers"] {
            assert_eq!(
                count(&db, &format!("SELECT COUNT(*) FROM {t}")),
                count(&h, &format!("SELECT COUNT(*) FROM {t}"))
            );
        }
        assert_eq!(
            rows(&db, "SELECT ID,FeatureType FROM PlotFeatures ORDER BY ID"),
            rows(&h, "SELECT ID,FeatureType FROM PlotFeatures ORDER BY ID")
        );
    }
}
#[test]
fn test_takeda_setup_slot_maps_to_static_city_and_territory_owners() {
    let scenarios = write_support::scenarios(&maps().dir, "TAKEDA").unwrap();
    for item in &maps().manifest {
        let scenario = &scenarios
            .iter()
            .find(|(name, _)| {
                name == &format!("japan-map-{}km-takeda.json", n(&item["km_per_tile"]))
            })
            .unwrap()
            .1;
        let slots = arr(&scenario["steps"][1]["players"])
            .iter()
            .enumerate()
            .map(|(i, p)| (s(&p["leader"]), i))
            .collect::<BTreeMap<_, _>>();
        let db = db(item);
        let leaders = rows(&db, "SELECT ID,LeaderType FROM Players");
        let takeda = leaders
            .iter()
            .find(|r| r[1] == "LEADER_JAC_TAKEDA_NOBUTORA")
            .unwrap();
        let id = n(&takeda[0]);
        assert_eq!(id, seat_index("TAKEDA"));
        assert_ne!(id, 0);
        assert_eq!(slots[s(&takeda[1])], 0);
        let cities = set(
            rows_params(&db, "SELECT Plot FROM Cities WHERE Owner=?", [id])
                .iter()
                .map(|r| n(&r[0])),
        );
        assert_eq!(
            cities,
            set(arr(&item["cities"])
                .iter()
                .filter(|c| c["owner"] == "TAKEDA")
                .map(|c| n(&c["plot"])))
        );
        let territory = set(
            rows_params(&db, "SELECT ID FROM PlotOwners WHERE Owner=?", [id])
                .iter()
                .map(|r| n(&r[0])),
        );
        assert!(territory.contains(&n(&arr(&item["cities"])
            .iter()
            .find(|c| c["owner"] == "TAKEDA" && c["capital"] == true)
            .unwrap()["plot"])));
        let code = s(&scenario["steps"][3]["code"]);
        for p in cities {
            assert!(code.contains(&format!("[{p}]={id}")));
        }
        assert!(code.contains("gameSlotForMapId[sourceOwner]"));
        assert!(code.contains("initial territory owners mismatch"));
    }
}
#[test]
fn test_resource_counts_deferred_candidates_and_city_exclusion() {
    let draft = g::resource_draft().unwrap();
    let groups = arr(&draft["resource_groups"]);
    assert_eq!(
        groups
            .iter()
            .filter(|g| g["placement_domain"] != "marine")
            .map(|g| arr(&g["points"]).len())
            .sum::<usize>(),
        51
    );
    assert_eq!(
        groups
            .iter()
            .filter(|g| g["placement_domain"] == "marine")
            .map(|g| arr(&g["points"]).len())
            .sum::<usize>(),
        17
    );
    assert_eq!(
        groups
            .iter()
            .map(|g| arr(&g["points"]).len())
            .sum::<usize>(),
        68
    );
    for item in &maps().manifest {
        assert_eq!(
            n(&item["resource_placement"]["placed_count"]),
            if item["km_per_tile"] == 20 { 53 } else { 57 }
        );
        let db = db(item);
        let rows = rows(&db, "SELECT ID,ResourceType FROM PlotResources ORDER BY ID");
        let candidates = arr(&item["resource_candidates"]);
        assert_eq!(candidates.len(), 68);
        assert_eq!(
            rows.len(),
            n(&item["resource_placement"]["placed_count"]) as usize
        );
        assert_eq!(
            json!(g::counts(rows.iter().map(|r| s(&r[1])))),
            item["resource_placement"]["count_by_type"]
        );
        let cities = set(arr(&item["cities"]).iter().map(|c| n(&c["plot"])));
        assert!(cities.is_disjoint(&set(rows.iter().map(|r| n(&r[0])))));
        let placed = candidates
            .iter()
            .filter(|r| r["placement_status"] == "placed")
            .map(|r| s(&r["id"]))
            .collect::<BTreeSet<_>>();
        assert_eq!(placed.len(), rows.len());
        assert!(arr(&item["deferred_resources"])
            .iter()
            .all(|r| !placed.contains(s(&r["id"]))));
        let types = rows.iter().map(|r| s(&r[1])).collect::<BTreeSet<_>>();
        assert!(!types.contains("RESOURCE_COPPER"));
        assert!(!types.contains("RESOURCE_NITER"));
        for req in arr(&item["custom_content_required"]) {
            if req["kind"] == "resource" {
                assert!(s(&req["status"]).starts_with("defined_in_"));
            }
        }
        for r in candidates
            .iter()
            .filter(|r| r["placement_status"] == "same_tile_conflict")
        {
            let winner = candidates
                .iter()
                .find(|c| c["id"] == r["conflict_with"])
                .unwrap();
            assert_eq!(r["plot"], winner["plot"]);
        }
        let silver = candidates
            .iter()
            .find(|r| r["id"] == "silver-iwami")
            .unwrap();
        assert_eq!(silver["placement_status"], "placed");
        assert!(!arr(&item["deferred_resources"]).contains(silver));
    }
}
#[test]
fn test_every_placed_resource_obeys_installed_official_rules_on_final_plot() {
    let features = official_xml("Base/Assets/Gameplay/Data/Features.xml");
    let mut allowed = BTreeMap::<String, BTreeSet<String>>::new();
    for r in civ6_assets_rs::rows(&features, "Feature_ValidTerrains") {
        allowed
            .entry(attr(r, "FeatureType").into())
            .or_default()
            .insert(attr(r, "TerrainType").into());
    }
    for item in &maps().manifest {
        let grid = prosperity::grid(item);
        let lakes = set(arr(&item["lake_records"])
            .iter()
            .flat_map(|l| arr(&l["tile_ids"]).iter().map(n)));
        for db in [db(item), variant(item)] {
            let terrain = prosperity::string_rows(&db, "SELECT ID,TerrainType FROM Plots").unwrap();
            let features =
                prosperity::string_rows(&db, "SELECT ID,FeatureType FROM PlotFeatures").unwrap();
            let mut river = Tiles::new();
            for r in rows(
                &db,
                "SELECT ID,IsNEOfRiver,IsWOfRiver,IsNWOfRiver FROM PlotRivers",
            ) {
                for (i, side) in ["NE", "W", "NW"].iter().enumerate() {
                    if r[i + 1] != 0 {
                        river.insert(n(&r[0]));
                        river.insert(g::side_neighbour(grid, n(&r[0]), side).unwrap());
                    }
                }
            }
            let resources = rows(&db, "SELECT ID,ResourceType FROM PlotResources");
            assert!(!resources.is_empty());
            for r in resources {
                let p = n(&r[0]);
                let t = &terrain[&p];
                let feature = features.get(&p).map(String::as_str);
                let adjacent = !prosperity::water(t)
                    || grid
                        .neighbor_ids(p)
                        .iter()
                        .any(|p| !prosperity::water(&terrain[p]));
                assert!(
                    g::resource_allowed(
                        &maps().rules[s(&r[1])],
                        t,
                        feature,
                        lakes.contains(&p),
                        adjacent,
                        river.contains(&p)
                    ),
                    "{:?} {t} {feature:?}",
                    r
                );
                if let Some(f) = feature {
                    assert!(allowed[f].contains(t));
                }
            }
        }
    }
}
#[test]
fn test_rivers_are_connected_and_only_use_shared_land_edges() {
    for item in &maps().manifest {
        assert_eq!(arr(&item["rivers"]).len(), 10);
        let mut grid = prosperity::grid(item);
        grid.xmin = f(&item["grid_origin_projected_km"][0]);
        grid.ymin = f(&item["grid_origin_projected_km"][1]);
        let db = db(item);
        let land = set(rows(
            &db,
            "SELECT ID FROM Plots WHERE ContinentType='CONTINENT_ASIA'",
        )
        .iter()
        .map(|r| n(&r[0])));
        let mut actual = BTreeSet::new();
        for r in rows(
            &db,
            "SELECT ID,IsNEOfRiver,IsWOfRiver,IsNWOfRiver FROM PlotRivers",
        ) {
            for (i, side) in ["NE", "W", "NW"].iter().enumerate() {
                if r[i + 1] == 0 {
                    continue;
                }
                let p = n(&r[0]);
                let neighbor = g::side_neighbour(grid, p, side).unwrap();
                assert!(land.contains(&p));
                assert!(land.contains(&neighbor));
                actual.insert((p, side.to_string()));
            }
        }
        let mut expected = BTreeSet::new();
        for river in arr(&item["rivers"]) {
            let path = arr(&river["edge_path"]);
            assert_eq!(path.len(), n(&river["edge_count"]) as usize);
            assert!(n(&river["edge_count"]) > 0);
            for edge in path {
                let key = (n(&edge["plot"]), s(&edge["side"]).to_owned());
                expected.insert(key.clone());
                assert!(actual.contains(&key));
                assert_eq!(
                    Some(n(&edge["neighbor"])),
                    g::side_neighbour(grid, key.0, &key.1)
                );
            }
            for pair in path.windows(2) {
                let before = g::river_edge_geometry(
                    grid,
                    &(n(&pair[0]["plot"]), s(&pair[0]["side"]).to_owned()),
                    Some(n(&pair[0]["neighbor"])),
                )
                .unwrap();
                let after = g::river_edge_geometry(
                    grid,
                    &(n(&pair[1]["plot"]), s(&pair[1]["side"]).to_owned()),
                    Some(n(&pair[1]["neighbor"])),
                )
                .unwrap();
                assert_eq!(before.iter().filter(|p| after.contains(p)).count(), 1);
            }
        }
        assert_eq!(actual, expected);
    }
}
#[test]
fn test_lakes_features_and_volcanoes_follow_draft_counts() {
    for item in &maps().manifest {
        let km = n(&item["km_per_tile"]);
        let want = if km == 20 {
            (1082, 7, 399, 27)
        } else {
            (1906, 8, 744, 33)
        };
        let features = &item["terrain_features"];
        let db = variant(item);
        let historical = common::db(item);
        assert_eq!(n(&item["land_tiles"]), want.0);
        assert_eq!(arr(&features["lakes"]).len(), 6);
        let lakes = arr(&features["lakes"])
            .iter()
            .flat_map(|l| arr(&l["tile_ids"]).iter().map(n))
            .collect::<Vec<_>>();
        assert_eq!(lakes.len(), want.1);
        assert_eq!(lakes.iter().collect::<BTreeSet<_>>().len(), lakes.len());
        for &p in &lakes {
            assert_eq!(
                rows_params(
                    &db,
                    "SELECT TerrainType,ContinentType FROM Plots WHERE ID=?",
                    [p]
                ),
                vec![vec![json!("TERRAIN_COAST"), json!("")]]
            );
            assert!(
                rows_params(&historical, "SELECT ID FROM PlotOwners WHERE ID=?", [p]).is_empty()
            );
        }
        let feature_rows = rows(&db, "SELECT ID,FeatureType FROM PlotFeatures ORDER BY ID");
        let fp = set(feature_rows.iter().map(|r| n(&r[0])));
        let rp = set(rows(&db, "SELECT ID FROM PlotResources")
            .iter()
            .map(|r| n(&r[0])));
        let cp = set(rows(&historical, "SELECT Plot FROM Cities")
            .iter()
            .map(|r| n(&r[0])));
        let mut types = BTreeSet::new();
        let mut resources = BTreeSet::new();
        for p in fp.intersection(&rp) {
            types.insert(
                rows_params(&db, "SELECT FeatureType FROM PlotFeatures WHERE ID=?", [*p])[0][0]
                    .clone()
                    .as_str()
                    .unwrap()
                    .to_owned(),
            );
            resources.insert(
                rows_params(
                    &db,
                    "SELECT ResourceType FROM PlotResources WHERE ID=?",
                    [*p],
                )[0][0]
                    .clone()
                    .as_str()
                    .unwrap()
                    .to_owned(),
            );
        }
        assert_eq!(types, BTreeSet::from(["FEATURE_FOREST".to_owned()]));
        assert_eq!(resources, BTreeSet::from(["RESOURCE_SILK".to_owned()]));
        assert!(fp.is_disjoint(&cp));
        assert!(fp.is_disjoint(&lakes.iter().copied().collect()));
        let actual = g::counts(feature_rows.iter().map(|r| s(&r[1])));
        assert_eq!(json!(actual), features["feature_counts"]);
        assert_eq!(actual["FEATURE_FOREST"], want.2);
        assert_eq!(actual["FEATURE_MARSH"], want.3);
        assert_eq!(actual["FEATURE_VOLCANO"], 5);
        approximate(f(&features["forest"]["placed_percent"]), 45., 0.05);
        assert!((22..=33).contains(&n(&features["marsh"]["placed_count"])));
        for v in arr(&features["volcanoes"]) {
            let row=&rows_params(&db,"SELECT p.TerrainType,f.FeatureType FROM Plots p JOIN PlotFeatures f ON f.ID=p.ID WHERE p.ID=?",[n(&v["plot"])])[0];
            assert!(s(&row[0]).ends_with("_MOUNTAIN"));
            assert_eq!(row[1], "FEATURE_VOLCANO");
        }
    }
}
#[test]
fn test_close_castle_pairs_report() {
    for item in &maps().manifest {
        let cities = arr(&item["cities"]);
        assert!(arr(&item["close_city_pairs"]).is_empty());
        assert_eq!(item["close_city_pairs"], json!(g::close_city_pairs(cities)));
        for (i, a) in cities.iter().enumerate() {
            for b in &cities[i + 1..] {
                assert!(hex_distance(xy(a), xy(b)) >= g::MIN_CITY_DISTANCE);
            }
        }
    }
}
#[test]
fn test_nudges_use_nearest_free_plot_of_the_original_owner() {
    for item in &maps().manifest {
        let db = db(item);
        let grid = HexGrid::for_scale(n(&item["km_per_tile"]), &maps().japan).unwrap();
        let land = set(rows(
            &db,
            "SELECT ID FROM Plots WHERE ContinentType='CONTINENT_ASIA'",
        )
        .iter()
        .map(|r| n(&r[0])));
        let passable = set(rows(
            &db,
            "SELECT ID FROM Plots WHERE ContinentType='CONTINENT_ASIA' AND IsImpassable=0",
        )
        .iter()
        .map(|r| n(&r[0])));
        let original = g::choose_cities(
            grid,
            &passable,
            arr(&item["starts"]),
            arr(&start_1534::data().unwrap()["castles"]),
        )
        .unwrap();
        let owners = g::assign_land(grid, &land, &original)
            .unwrap()
            .into_iter()
            .map(|(p, o, _, _)| (p, SEATS[o as usize].slug.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut occupied = set(original.iter().map(|c| n(&c["plot"])));
        let mut placed = vec![];
        assert_eq!(
            arr(&item["cities"])
                .iter()
                .map(|c| s(&c["id"]))
                .collect::<Vec<_>>(),
            original.iter().map(|c| s(&c["id"])).collect::<Vec<_>>()
        );
        assert!(arr(&item["cities"])
            .iter()
            .any(|c| c.get("nudge").is_some()));
        for (c, before) in arr(&item["cities"]).iter().zip(&original) {
            let historical = g::project(f(&c["lon"]), f(&c["lat"]));
            assert_eq!(
                f(&c["displacement_km"]),
                round(
                    g::distance(historical, grid.centre(n(&c["col"]), n(&c["row"]))),
                    1
                )
            );
            if let Some(nudge) = c.get("nudge") {
                assert_eq!(c["capital"], false);
                assert_eq!(nudge["from_plot"], before["plot"]);
                assert_eq!(nudge["to_plot"], c["plot"]);
                let origin = grid.center(n(&before["plot"]));
                let moved = g::distance(origin, grid.center(n(&c["plot"])));
                assert_eq!(f(&nudge["distance_km"]), round(moved, 1));
                assert!(moved <= g::MAX_NUDGE_TILES as f64 * grid.km as f64 + 1e-8);
                assert_eq!(owners[&n(&c["plot"])], s(&c["owner"]));
                assert!(passable.contains(&n(&c["plot"])));
                let candidate = passable
                    .difference(&occupied)
                    .filter(|p| {
                        owners[p] == s(&c["owner"])
                            && !placed
                                .iter()
                                .any(|earlier| hex_distance(grid.xy(**p), *earlier) < 2)
                    })
                    .map(|&p| {
                        (
                            g::distance(origin, grid.center(p)),
                            g::distance(historical, grid.center(p)),
                            p,
                        )
                    })
                    .filter(|v| v.0 <= g::MAX_NUDGE_TILES as f64 * grid.km as f64 + 1e-8)
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap();
                assert_eq!(n(&c["plot"]), candidate.2);
                occupied.remove(&n(&before["plot"]));
                occupied.insert(n(&c["plot"]));
            } else {
                assert_eq!(c["plot"], before["plot"]);
            }
            placed.push(xy(c));
        }
    }
}
#[test]
fn test_city_related_inserts_put_capitals_first() {
    for item in &maps().manifest {
        let db = db(item);
        let allocations = rows(&db, "SELECT ID,Owner,CityOwner,CityWorking FROM PlotOwners")
            .into_iter()
            .map(|r| (n(&r[0]), n(&r[1]), n(&r[2]), n(&r[3])))
            .collect::<Vec<_>>();
        let mut reverse = arr(&item["cities"]).to_vec();
        reverse.reverse();
        let calls = g::historical_state_batches(&reverse, &allocations);
        let capitals = arr(&item["cities"])[..22]
            .iter()
            .map(|c| rusqlite::types::Value::Integer(n(&c["plot"]) as i64))
            .collect::<Vec<_>>();
        assert_eq!(
            calls[0].rows[..22]
                .iter()
                .map(|r| r[1].clone())
                .collect::<Vec<_>>(),
            capitals
        );
        assert_eq!(
            calls[1].rows[..22]
                .iter()
                .map(|r| r[0].clone())
                .collect::<Vec<_>>(),
            capitals
        );
        assert_eq!(
            calls[2].rows[..22]
                .iter()
                .map(|r| r[0].clone())
                .collect::<Vec<_>>(),
            capitals
        );
        assert!(calls[3].sql.contains("BUILDING_PALACE"));
        assert_eq!(
            calls[3]
                .rows
                .iter()
                .map(|r| r[0].clone())
                .collect::<Vec<_>>(),
            capitals
        );
        assert!(calls[4].sql.contains("PlotOwners"));
        assert_eq!(
            calls[4].rows[..22]
                .iter()
                .map(|r| r[0].clone())
                .collect::<Vec<_>>(),
            capitals
        );
    }
}
#[test]
fn test_every_capital_and_only_capitals_have_a_palace() {
    for item in &maps().manifest {
        let palace = set(rows(
            &db(item),
            "SELECT Plot FROM Buildings WHERE BuildingType='BUILDING_PALACE'",
        )
        .iter()
        .map(|r| n(&r[0])));
        assert_eq!(
            palace,
            set(arr(&item["cities"])
                .iter()
                .filter(|c| c["capital"] == true)
                .map(|c| n(&c["plot"])))
        );
        assert_eq!(palace.len(), 22);
    }
}
#[test]
fn test_hex_distance_matches_neighbours() {
    let grid = HexGrid {
        km: 40,
        width: 20,
        height: 20,
        xmin: 0.,
        ymin: 0.,
    };
    for (c, r) in [(5, 4), (5, 5)] {
        for other in grid.neighbours(c, r) {
            assert_eq!(hex_distance((c, r), other), 1);
        }
    }
    assert_eq!(hex_distance((0, 0), (3, 0)), 3);
    assert_eq!(hex_distance((0, 0), (0, 2)), 2);
}
#[test]
fn test_config_and_scenarios_cover_all_scales() {
    let config = write_support::config();
    let maps_xml = civ6_assets_rs::rows(&config, "Maps");
    assert_eq!(maps_xml.len(), 4);
    assert_eq!(
        maps_xml
            .iter()
            .map(|r| attr(r, "Domain"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["Maps:JAC_Banseiki"])
    );
    assert!(civ6_assets_rs::rows(&config, "DomainValueUnions").is_empty());
    let ruleset =
        xml(root().join("poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml")).unwrap();
    assert_eq!(
        civ6_assets_rs::rows(&ruleset, "DomainValueUnions")
            .iter()
            .map(|r| (attr(r, "Domain"), attr(r, "OtherDomain")))
            .collect::<Vec<_>>(),
        vec![("Maps:JAC_Banseiki", "Maps:Expansion2Maps")]
    );
    assert_eq!(
        civ6_assets_rs::rows(&ruleset, "RulesetDomainOverrides")
            .iter()
            .filter(|r| attr(r, "ParameterId") == "Map")
            .map(|r| attr(r, "Domain"))
            .collect::<Vec<_>>(),
        vec!["Maps:JAC_Banseiki"]
    );
    assert_eq!(civ6_assets_rs::rows(&config, "MapSizes").len(), 4);
    assert!(civ6_assets_rs::rows(&config, "Players").is_empty());
    assert_eq!(civ6_assets_rs::rows(&config, "MapLeaders").len(), 88);
    let mut leaders = BTreeMap::<String, Vec<String>>::new();
    for r in civ6_assets_rs::rows(&config, "MapLeaders") {
        leaders
            .entry(attr(r, "Map").into())
            .or_default()
            .push(attr(r, "LeaderType").into());
    }
    assert_eq!(leaders.len(), 4);
    for list in leaders.values() {
        assert_eq!(list.len(), 22);
        assert_eq!(
            list.iter().cloned().collect::<BTreeSet<_>>(),
            SEATS.iter().map(|s| s.leader.clone()).collect()
        );
        assert_eq!(list.iter().collect::<BTreeSet<_>>().len(), 22);
    }
    for r in civ6_assets_rs::rows(&config, "MapSizes") {
        assert_eq!(
            (
                attr(r, "MinPlayers"),
                attr(r, "MaxPlayers"),
                attr(r, "DefaultPlayers"),
                attr(r, "DefaultCityStates")
            ),
            ("22", "22", "22", "0")
        );
    }
    let text = write_support::text().unwrap();
    for km in constants::SCALES {
        for mode in ["HISTORICAL", "SETTLERS"] {
            let key = format!("LOC_JAC_JAPAN_{km}_{mode}_NAME");
            assert_eq!(
                civ6_assets_rs::rows(&text, "LocalizedText")
                    .iter()
                    .filter(|r| attr(r, "Tag") == key)
                    .map(|r| attr(r, "Language"))
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["ja_JP", "en_US", "ru_RU"])
            );
        }
    }
    let modinfo = xml(checked_map_mod().join("JAC_PoC_JapanMapScale.modinfo")).unwrap();
    let packaged = find_all(&modinfo, &["Files", "File"])
        .iter()
        .map(|e| e.get_text().unwrap().into_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        find_all(&modinfo, &["Dependencies", "Mod"])
            .iter()
            .map(|r| attr(r, "id"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "4873eb62-8ccc-4574-b784-dda455e74e68",
            "ba3a1c58-9287-4654-a22f-85f9e6841e9b",
            "bd7a82c5-6b38-4b69-a7f5-8dc9af534249"
        ])
    );
    let scenarios = write_support::scenarios(&maps().dir, "ODA").unwrap();
    let expected_counts = regex::Regex::new(r"\{(\d+),(\d+),(\d+)\}").unwrap();
    for item in &maps().manifest {
        let file = format!("{{{}}}Maps/{}", write_support::MOD_ID, s(&item["file"]));
        assert!(maps_xml.iter().any(|r| attr(r, "File") == file));
        for name in [
            format!("Maps/{}", s(&item["file"])),
            format!("Maps/{}", s(&item["file"]).replace(".Civ6Map", ".lua")),
            format!("Maps/{}", s(&item["settler_file"])),
        ] {
            assert!(packaged.contains(&name));
        }
        let scenario = &scenarios
            .iter()
            .find(|(name, _)| name == &format!("japan-map-{}km.json", n(&item["km_per_tile"])))
            .unwrap()
            .1;
        assert_eq!(scenario["steps"][1]["map_file"], file);
        assert_eq!(scenario["steps"][1]["ruleset"], "RULESET_JAC_BANSEIKI");
        assert_eq!(arr(&scenario["steps"][1]["players"]).len(), 22);
        let code = s(&scenario["steps"][3]["code"]);
        let expected = code
            .split("local expected=")
            .nth(1)
            .unwrap()
            .split("; local leaders=")
            .next()
            .unwrap();
        let counts = expected_counts
            .captures_iter(expected)
            .map(|c| c[3].parse::<usize>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            counts,
            SEATS
                .iter()
                .map(|s| arr(&item["cities"])
                    .iter()
                    .filter(|c| c["owner"] == s.slug)
                    .count())
                .collect::<Vec<_>>()
        );
        assert!(code.contains("if totalCities~=56"));
        assert_eq!(scenario["steps"][4]["turns"], 50);
        assert_eq!(scenario["steps"][5]["capture_as"], "JAPAN_STATE");
        assert_eq!(scenario["steps"][9]["equals_capture"], "JAPAN_STATE");
    }
}
#[test]
fn test_leaders_unique_and_human_from_banseiki_pool() {
    assert_eq!(
        SEATS
            .iter()
            .map(|s| &s.leader)
            .collect::<BTreeSet<_>>()
            .len(),
        22
    );
    assert_eq!(SEATS[0].leader, "LEADER_JAC_ODA_NOBUHIDE");
}
#[test]
fn test_all_leaders_registered_with_nonmissing_icons() {
    let m = root().join("poc/JAC_PoC_CustomCivs");
    let existing = xml(m.join("Config/JAC_CustomCivs_Players.xml")).unwrap();
    let generated = write_custom_civs::front_end();
    let players = civ6_assets_rs::rows(&existing, "Players")
        .into_iter()
        .chain(civ6_assets_rs::rows(&generated, "Players"))
        .collect::<Vec<_>>();
    let mut icons = BTreeSet::new();
    for file in ["JAC_CustomCivs_Icons.xml", "JAC_Crests_Icons.xml"] {
        icons.extend(
            civ6_assets_rs::rows(&xml(m.join("Data").join(file)).unwrap(), "IconDefinitions")
                .iter()
                .map(|r| attr(r, "Name").to_owned()),
        );
    }
    icons.extend(
        civ6_assets_rs::rows(&write_custom_civs::icons(), "IconDefinitions")
            .iter()
            .map(|r| attr(r, "Name").to_owned()),
    );
    assert_eq!(players.len(), 22);
    for seat in SEATS.iter() {
        assert_eq!(seat.civ, start_1534::civ_id(&seat.slug));
        assert_eq!(seat.leader, start_1534::leader_id(&seat.slug));
        let p = players
            .iter()
            .find(|p| attr(p, "LeaderType") == seat.leader)
            .unwrap();
        assert_eq!(attr(p, "CivilizationType"), seat.civ);
        assert!(icons.contains(attr(p, "CivilizationIcon")));
        assert!(icons.contains(attr(p, "LeaderIcon")));
        assert!(attr(p, "Portrait").starts_with("LEADER_JAC_"));
        assert!(attr(p, "PortraitBackground").starts_with("LEADER_JAC_"));
    }
    assert_eq!(start_1534::heads().as_object().unwrap().len(), 22);
    let localized = write_custom_civs::translated_text().unwrap();
    let map = civ6_assets_rs::rows(&localized, "LocalizedText")
        .iter()
        .map(|r| {
            (
                (attr(r, "Tag").to_owned(), attr(r, "Language").to_owned()),
                text_of(r, "Text"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        map[&(
            "LOC_LEADER_JAC_SHIMAZU_TAKAHISA_NAME".into(),
            "ja_JP".into()
        )],
        "島津貴久"
    );
    assert_eq!(
        map[&("LOC_LEADER_JAC_AINU_TARIKONA_NAME".into(), "ja_JP".into())],
        "タリコナ"
    );
    for seat in SEATS.iter() {
        for lang in ["ja_JP", "en_US", "ru_RU"] {
            assert!(map.contains_key(&(format!("LOC_JAC_1534_CAPITAL_{}", seat.slug), lang.into())));
        }
    }
}
#[test]
fn unsupported_leader_filter_prevents_partial_dlc_loading_only_in_banseiki() {
    let rules =
        xml(root().join("poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml")).unwrap();
    let query = attr(
        civ6_assets_rs::rows(&rules, "Queries")
            .into_iter()
            .find(|row| attr(row, "QueryId") == "JAC_Banseiki_UnsupportedLeaderFilter")
            .unwrap(),
        "SQL",
    );
    let db = Connection::open_in_memory().unwrap();
    db.execute("CREATE TABLE Players (Domain TEXT,LeaderType TEXT)", [])
        .unwrap();
    for leader in [
        "LEADER_SEJONG",
        "LEADER_HAMMURABI",
        "LEADER_MENELIK",
        "LEADER_SIMON_BOLIVAR",
        "LEADER_LADY_SIX_SKY",
        "LEADER_JAC_ODA_NOBUHIDE",
    ] {
        db.execute(
            "INSERT INTO Players VALUES ('Players:Expansion2_Players', ?)",
            [leader],
        )
        .unwrap();
    }
    db.execute(
        "INSERT INTO Players VALUES ('Players:StandardPlayers', 'LEADER_MENELIK')",
        [],
    )
    .unwrap();
    let excluded = rows_params(&db, query, params!["RULESET_JAC_BANSEIKI"]);
    assert_eq!(
        excluded
            .iter()
            .map(|row| s(&row[1]))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["LEADER_SEJONG", "LEADER_HAMMURABI", "LEADER_MENELIK"])
    );
    assert!(excluded
        .iter()
        .all(|row| s(&row[0]) == "Players:Expansion2_Players" && s(&row[2]) == "difference"));
    for official in [
        "RULESET_STANDARD",
        "RULESET_EXPANSION_1",
        "RULESET_EXPANSION_2",
    ] {
        assert!(rows_params(&db, query, params![official]).is_empty());
    }
    // Installed Ethiopia.modinfo:19 joins these two domain tokens with a
    // space. Its XP2 criterion (32) is separate and valid. The engine's comma
    // token contract therefore enables XP2 without its prerequisite base data.
    let base_criteria = "Players:StandardPlayers::LEADER_MENELIK,StandardPlayers::LEADER_MENELIK,Players:Expansion1_Players::LEADER_MENELIK Players:Expansion2_Players::LEADER_MENELIK";
    let xp2_criteria = "Players:Expansion2_Players::LEADER_MENELIK";
    let raw = [
        xp2_criteria,
        "Players:Expansion2_Players::LEADER_SIMON_BOLIVAR",
    ];
    let matches =
        |criteria: &str, values: &[&str]| criteria.split(',').any(|token| values.contains(&token));
    assert!(
        !matches(base_criteria, &raw) && matches(xp2_criteria, &raw),
        "native partial-load failure must be reproducible"
    );
    let native_values: Vec<_> = raw
        .into_iter()
        .filter(|value| {
            let leader = value.rsplit_once("::").unwrap().1;
            !excluded.iter().any(|row| s(&row[1]) == leader)
        })
        .collect();
    assert!(!matches(base_criteria, &native_values) && !matches(xp2_criteria, &native_values));
    assert!(
        matches(
            "Players:Expansion2_Players::LEADER_SIMON_BOLIVAR",
            &native_values
        ),
        "required Vatican DLC dependency must remain available"
    );
    db.execute("DELETE FROM Players WHERE LeaderType='LEADER_MENELIK'", [])
        .unwrap();
    assert_eq!(
        rows_params(&db, query, params!["RULESET_JAC_BANSEIKI"]).len(),
        2,
        "missing DLC must not create a synthetic exclusion row"
    );
}

#[test]
fn test_banseiki_filter_skips_japan_maps_and_settler_removal_is_scoped() {
    let rules =
        xml(root().join("poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml")).unwrap();
    let query = attr(
        civ6_assets_rs::rows(&rules, "Queries")
            .into_iter()
            .find(|r| attr(r, "QueryId") == "JAC_Banseiki_HumanLeaderFilter")
            .unwrap(),
        "SQL",
    );
    let db = Connection::open_in_memory().unwrap();
    db.execute("CREATE TABLE Players (Domain TEXT,LeaderType TEXT)", [])
        .unwrap();
    for seat in SEATS.iter() {
        db.execute(
            "INSERT INTO Players VALUES ('Players:Expansion2_Players',?)",
            [&seat.leader],
        )
        .unwrap();
    }
    assert_eq!(
        rows_params(
            &db,
            query,
            params![
                "RULESET_JAC_BANSEIKI",
                0,
                "JAC_BANSEIKI_ONLY",
                "Continents.lua"
            ]
        )
        .iter()
        .map(|r| s(&r[1]))
        .collect::<BTreeSet<_>>(),
        BTreeSet::from(["LEADER_JAC_ODA_NOBUHIDE", "LEADER_JAC_TAKEDA_NOBUTORA"])
    );
    for km in constants::SCALES {
        for suffix in ["", "_Settlers"] {
            let file = format!(
                "{{{}}}Maps/JAC_Japan_{km}km{suffix}.Civ6Map",
                write_support::MOD_ID
            );
            assert!(rows_params(
                &db,
                query,
                params!["RULESET_JAC_BANSEIKI", 0, "JAC_BANSEIKI_ONLY", file]
            )
            .is_empty());
        }
    }
    let modinfo = xml(checked_map_mod().join("JAC_PoC_JapanMapScale.modinfo")).unwrap();
    let criteria = find_all(&modinfo, &["ActionCriteria", "Criteria"])
        .iter()
        .map(|e| {
            (
                attr(e, "id").to_owned(),
                find_all(e, &["ConfigurationValueMatches", "Value"])
                    .first()
                    .and_then(|e| e.get_text())
                    .map(|v| v.into_owned())
                    .unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let actions = find_all(&modinfo, &["InGameActions", "UpdateDatabase"])
        .into_iter()
        .filter(|e| text_of(e, "File") == "Data/JAC_Japan_NoSettlers.xml")
        .collect::<Vec<_>>();
    assert_eq!(actions.len(), 2);
    for action in actions {
        let selected = &criteria[attr(action, "criteria")];
        assert!(constants::SCALES
            .iter()
            .any(|km| *selected
                == format!("{{{}}}Maps/JAC_Japan_{km}km.Civ6Map", write_support::MOD_ID)));
    }
    let removal = xml(checked_map_mod().join("Data/JAC_Japan_NoSettlers.xml")).unwrap();
    let deletes = find_all(&removal, &["MajorStartingUnits", "Delete"]);
    assert_eq!(deletes.len(), 1);
    assert!(deletes[0].attributes.is_empty());
}
#[test]
fn test_city_range_is_limited_to_four_japan_maps() {
    let ruleset =
        xml(root().join("poc/JAC_PoC_BanseikiRuleset/JAC_PoC_BanseikiRuleset.modinfo")).unwrap();
    for context in ["FrontEndActions", "InGameActions"] {
        assert!(!find_all(&ruleset, &[context, "UpdateDatabase"])
            .iter()
            .any(|e| format!("{}{}", attr(e, "id"), text_of(e, "File")).contains("CityRange")));
    }
    let modinfo = xml(checked_map_mod().join("JAC_PoC_JapanMapScale.modinfo")).unwrap();
    let config = write_support::config();
    let files = civ6_assets_rs::rows(&config, "Maps")
        .iter()
        .map(|r| attr(r, "File").to_owned())
        .collect::<BTreeSet<_>>();
    let actions = find_all(&modinfo, &["InGameActions", "UpdateDatabase"])
        .into_iter()
        .filter(|r| text_of(r, "File") == "Data/JAC_Japan_MapRules.xml")
        .collect::<Vec<_>>();
    assert_eq!(actions.len(), 4);
    let criteria = find_all(&modinfo, &["ActionCriteria", "Criteria"]);
    let selected = actions
        .iter()
        .map(|a| {
            let c = criteria
                .iter()
                .find(|c| attr(c, "id") == attr(a, "criteria"))
                .unwrap();
            find_all(c, &["ConfigurationValueMatches", "Value"])[0]
                .get_text()
                .unwrap()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(selected, files);
    let rules = xml(checked_map_mod().join("Data/JAC_Japan_MapRules.xml")).unwrap();
    let updates = find_all(&rules, &["GlobalParameters", "Update"])
        .iter()
        .map(|r| {
            (
                attr(r.get_child("Where").unwrap(), "Name").to_owned(),
                text_of(r.get_child("Set").unwrap(), "Value"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(updates["CITY_MIN_RANGE"], "1");
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE GlobalParameters(Name TEXT PRIMARY KEY,Value TEXT);INSERT INTO GlobalParameters VALUES('CITY_MIN_RANGE','3'),('CITY_MIN_DISTANCE_FOR_INCURSION','8');").unwrap();
    for (k, v) in updates {
        db.execute("UPDATE GlobalParameters SET Value=? WHERE Name=?", [v, k])
            .unwrap();
    }
    assert_eq!(
        rows(
            &db,
            "SELECT Value FROM GlobalParameters WHERE Name='CITY_MIN_RANGE'"
        ),
        vec![vec![json!("1")]]
    );
    assert_eq!(
        rows(
            &db,
            "SELECT Value FROM GlobalParameters WHERE Name='CITY_MIN_DISTANCE_FOR_INCURSION'"
        ),
        vec![vec![json!("8")]]
    );
    let official = official_xml("Base/Assets/Gameplay/Data/GlobalParameters.xml");
    assert_eq!(
        elements(official.get_child("GlobalParameters").unwrap())
            .filter(|r| attr(r, "Name") == "CITY_MIN_RANGE")
            .map(|r| attr(r, "Value"))
            .collect::<Vec<_>>(),
        vec!["3"]
    );
}
#[test]
fn test_domain_passage_and_loyalty_are_limited_to_all_four_japan_maps() {
    let config = write_support::config();
    let files = civ6_assets_rs::rows(&config, "Maps")
        .iter()
        .map(|r| attr(r, "File").to_owned())
        .collect::<BTreeSet<_>>();
    let modinfo = xml(checked_map_mod().join("JAC_PoC_JapanMapScale.modinfo")).unwrap();
    let criteria = find_all(&modinfo, &["ActionCriteria", "Criteria"]);
    let actions = find_all(&modinfo, &["InGameActions", "UpdateDatabase"])
        .into_iter()
        .filter(|r| text_of(r, "File") == "Data/JAC_Japan_MapRules.xml")
        .collect::<Vec<_>>();
    assert_eq!(actions.len(), 4);
    let mut selected = BTreeSet::new();
    for a in actions {
        let c = criteria
            .iter()
            .find(|c| attr(c, "id") == attr(a, "criteria"))
            .unwrap();
        assert_eq!(text_of(c, "RuleSetInUse"), "RULESET_JAC_BANSEIKI");
        let values = c.get_child("ConfigurationValueMatches").unwrap();
        assert_eq!(text_of(values, "Group"), "Map");
        assert_eq!(text_of(values, "ConfigurationId"), "MAP_SCRIPT");
        selected.insert(text_of(values, "Value"));
    }
    assert_eq!(selected, files);
    assert!(find_all(&modinfo, &["Files", "File"])
        .iter()
        .any(|e| e.get_text().unwrap() == "Data/JAC_Japan_MapRules.xml"));
    let rules = xml(checked_map_mod().join("Data/JAC_Japan_MapRules.xml")).unwrap();
    let tag = civ6_assets_rs::rows(&rules, "TypeTags")[0];
    assert_eq!(tag.attributes.len(), 2);
    assert_eq!(
        (attr(tag, "Type"), attr(tag, "Tag")),
        ("ABILITY_JAC_DOMAIN_CROSSING", "CLASS_ALL_UNITS")
    );
    assert_eq!(
        attr(
            civ6_assets_rs::rows(&rules, "UnitAbilityModifiers")[0],
            "ModifierId"
        ),
        "MOD_ENTER_FOREIGN_LANDS"
    );
    assert_eq!(
        attr(civ6_assets_rs::rows(&rules, "Modifiers")[0], "ModifierType"),
        "MODIFIER_PLAYER_UNITS_GRANT_ABILITY"
    );
    assert_eq!(
        attr(
            civ6_assets_rs::rows(&rules, "ModifierArguments")[0],
            "Value"
        ),
        "ABILITY_JAC_DOMAIN_CROSSING"
    );
    assert_eq!(
        elements(rules.get_child("TraitModifiers").unwrap())
            .map(|r| (r.name.as_str(), attr(r, "ModifierId")))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            ("Row", "JAC_DOMAIN_CROSSING_ALL_UNITS"),
            ("Row", "JAC_JAPAN_CITY_LOYALTY_STABILITY"),
            ("Delete", "STANDARD_DIPLOMATIC_INCURSION")
        ])
    );
    let stability = civ6_assets_rs::rows(&rules, "Modifiers")
        .into_iter()
        .find(|r| attr(r, "ModifierId") == "JAC_JAPAN_CITY_LOYALTY_STABILITY")
        .unwrap();
    assert_eq!(stability.attributes.len(), 2);
    assert_eq!(
        attr(stability, "ModifierType"),
        "MODIFIER_PLAYER_CITIES_ADJUST_IDENTITY_PER_TURN"
    );
    let argument = civ6_assets_rs::rows(&rules, "ModifierArguments")
        .into_iter()
        .find(|r| attr(r, "ModifierId") == "JAC_JAPAN_CITY_LOYALTY_STABILITY")
        .unwrap();
    assert_eq!(argument.attributes.len(), 3);
    assert_eq!(
        (attr(argument, "Name"), attr(argument, "Value")),
        ("Amount", "100")
    );
    let official = official_xml("DLC/Expansion2/Data/Expansion1_Modifiers.xml");
    let types = civ6_assets_rs::rows(&official, "DynamicModifiers")
        .into_iter()
        .filter(|r| text_of(r, "ModifierType") == attr(stability, "ModifierType"))
        .collect::<Vec<_>>();
    assert_eq!(types.len(), 1);
    assert_eq!(
        text_of(types[0], "CollectionType"),
        "COLLECTION_PLAYER_CITIES"
    );
    assert_eq!(
        text_of(types[0], "EffectType"),
        "EFFECT_ADJUST_CITY_IDENTITY_PER_TURN"
    );
    let key = "LOYALTY_PER_TURN_FROM_NEARBY_CITIZEN_PRESSURE_MAX_LOYALTY";
    let loyalty = find_all(&rules, &["GlobalParameters", "Update"])
        .into_iter()
        .find(|r| attr(r.get_child("Where").unwrap(), "Name") == key)
        .unwrap();
    assert_eq!(attr(loyalty.get_child("Where").unwrap(), "Name"), key);
    assert_eq!(text_of(loyalty.get_child("Set").unwrap(), "Value"), "5");
    let baseline = official_xml("DLC/Expansion2/Data/Expansion2_GlobalParameters.xml");
    assert_eq!(
        find_all(&baseline, &["GlobalParameters", "Replace"])
            .iter()
            .filter(|r| attr(r, "Name") == key)
            .map(|r| attr(r, "Value"))
            .collect::<Vec<_>>(),
        vec!["20"]
    );
}
