use crate::*;
use constants::{seat_index, SEATS};
use generate::{hex_distance, HexGrid, Tiles};
use prosperity::{string_rows, water};
use rusqlite::{params, Connection};
use serde_json::json;
use std::collections::BTreeMap;
pub const DISTRICT_RANGE: i32 = 3;
pub const RESERVE_RANGE: i32 = 2;
pub const START_STATE_KEY: &str = "JAC_JAPAN_START_STATE";
pub const START_BUILDINGS_KEY: &str = "JAC_JAPAN_START_BUILDINGS";
pub fn improvement_for(
    terrain: &str,
    feature: Option<&str>,
    resource: Option<&str>,
) -> Option<&'static str> {
    if feature.is_some_and(|s| !s.is_empty()) {
        return None;
    }
    if let Some(resource) = resource {
        if !resource.is_empty() {
            return match resource {
                "RESOURCE_COPPER" | "RESOURCE_SILVER" | "RESOURCE_SALT" | "RESOURCE_IRON"
                | "RESOURCE_JADE" | "RESOURCE_MERCURY" => Some("IMPROVEMENT_MINE"),
                "RESOURCE_STONE" | "RESOURCE_MARBLE" | "RESOURCE_GYPSUM" => {
                    Some("IMPROVEMENT_QUARRY")
                }
                "RESOURCE_RICE" | "RESOURCE_WHEAT" => Some("IMPROVEMENT_FARM"),
                _ => None,
            };
        }
    }
    match terrain {
        "TERRAIN_GRASS" | "TERRAIN_PLAINS" => Some("IMPROVEMENT_FARM"),
        "TERRAIN_GRASS_HILLS" | "TERRAIN_PLAINS_HILLS" => Some("IMPROVEMENT_MINE"),
        _ => None,
    }
}
pub fn reset(db: &Connection, features: Option<&[(i32, String)]>) -> Result<()> {
    if let Some(features) = features {
        db.execute("DELETE FROM PlotFeatures", [])?;
        for (p, t) in features {
            db.execute("INSERT INTO PlotFeatures VALUES (?,?)", params![p, t])?;
        }
    }
    db.execute_batch("DELETE FROM Units; DELETE FROM PlotImprovements; DELETE FROM Districts WHERE DistrictType<>'DISTRICT_CITY_CENTER'; DELETE FROM Buildings WHERE BuildingType<>'BUILDING_PALACE'; DELETE FROM PlayerAttributes WHERE Type='Treasury'; DELETE FROM PlotOwners WHERE ID IN (SELECT ID FROM Plots WHERE TerrainType IN ('TERRAIN_COAST','TERRAIN_OCEAN')); UPDATE CityAttributes SET Value='2' WHERE Type='Population' AND Name='Initial';")?;
    Ok(())
}
pub fn apply(
    db: &Connection,
    grid: HexGrid,
    cities: &[Value],
    features: Option<&[(i32, String)]>,
) -> Result<Value> {
    reset(db, features)?;
    let info = prosperity::read_db(db, grid)?;
    let terrain = string_rows(db, "SELECT ID,TerrainType FROM Plots")?;
    let impassable = db
        .prepare("SELECT ID FROM Plots WHERE IsImpassable<>0")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<Tiles>>()?;
    let feature = string_rows(db, "SELECT ID,FeatureType FROM PlotFeatures")?;
    let resource = string_rows(db, "SELECT ID,ResourceType FROM PlotResources")?;
    let owners = prosperity::owner_rows(db)?;
    let mut plot_owner: BTreeMap<_, _> = owners.iter().map(|r| (r.0, r.1)).collect();
    let mut city_tiles: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
    for &(p, _, c) in &owners {
        city_tiles.entry(c).or_default().push(p);
    }
    let centers: Tiles = cities.iter().map(|c| n(&c["plot"])).collect();
    let dist = |a, b| hex_distance(grid.xy(a), grid.xy(b));
    let mut used = centers.clone();
    let mut occupied = Tiles::new();
    let mut districts: Vec<(String, i32, i32)> = vec![];
    let mut buildings: Vec<(String, i32)> = vec![];
    let mut improvements = vec![];
    let mut units: Vec<(String, i32, i32)> = vec![];
    let mut owner_adds = vec![];
    let mut report = serde_json::Map::new();
    for seat in SEATS.iter() {
        let slug = &seat.slug;
        let index = seat_index(slug);
        let own = generate::ordered_cities(
            &cities
                .iter()
                .filter(|c| s(&c["owner"]) == slug)
                .cloned()
                .collect::<Vec<_>>(),
        );
        let mut plans = vec![];
        for city in own {
            let center = n(&city["plot"]);
            let ci = info.get(&center).context("city statistics missing")?;
            let plan = prosperity::city_plan(slug, &city, &ci.rec, ci.coastal);
            let land: Vec<i32> = city_tiles
                .get(&center)
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .filter(|&&p| {
                    p != center
                        && !water(&terrain[&p])
                        && !impassable.contains(&p)
                        && !terrain[&p].ends_with("MOUNTAIN")
                })
                .copied()
                .collect();
            let mut placed = vec![];
            for d in arr(&plan["district_options"]) {
                if placed.len() >= n(&plan["n_districts"]) as usize {
                    break;
                }
                let dtype = s(&d["district"]);
                let candidates: Vec<_> = if dtype == "DISTRICT_HARBOR" {
                    city_tiles
                        .get(&center)
                        .map(Vec::as_slice)
                        .unwrap_or(&[])
                        .iter()
                        .flat_map(|&p| grid.neighbor_ids(p))
                        .filter(|p| {
                            terrain[p] == "TERRAIN_COAST"
                                && !plot_owner.contains_key(p)
                                && !resource.contains_key(p)
                                && !used.contains(p)
                                && (1..=3).contains(&dist(*p, center))
                        })
                        .collect()
                } else {
                    land.iter()
                        .filter(|&&p| {
                            if used.contains(&p)
                                || resource.contains_key(&p)
                                || feature.contains_key(&p)
                            {
                                return false;
                            }
                            let distance = dist(p, center);
                            distance <= DISTRICT_RANGE
                                && (dtype != "DISTRICT_ENCAMPMENT" || distance >= 2)
                        })
                        .copied()
                        .collect()
                };
                let Some(p) = candidates.into_iter().min_by_key(|&p| (dist(p, center), p)) else {
                    continue;
                };
                used.insert(p);
                if dtype == "DISTRICT_HARBOR" {
                    plot_owner.insert(p, index);
                    owner_adds.push((p, index, center, center));
                }
                districts.push((dtype.into(), center, p));
                buildings.extend(arr(&d["buildings"]).iter().map(|b| (s(b).to_owned(), p)));
                placed.push(d.clone());
            }
            buildings.extend(
                arr(&plan["city_buildings"])
                    .iter()
                    .filter(|b| **b != "BUILDING_PALACE")
                    .map(|b| (s(b).to_owned(), center)),
            );
            db.execute("UPDATE CityAttributes SET Value=? WHERE ID=? AND Type='Population' AND Name='Initial'",params![n(&plan["pop"]).to_string(),center])?;
            let worked = (n(&plan["pop"]) - placed.len() as i32).max(0);
            let mut options: Vec<_> = land
                .iter()
                .filter(|&&p| !used.contains(&p) && dist(p, center) <= 3)
                .filter_map(|&p| {
                    improvement_for(
                        &terrain[&p],
                        feature.get(&p).map(String::as_str),
                        resource.get(&p).map(String::as_str),
                    )
                    .map(|imp| {
                        (
                            !resource.contains_key(&p),
                            imp != "IMPROVEMENT_FARM",
                            dist(p, center),
                            p,
                            imp,
                        )
                    })
                })
                .collect();
            options.sort();
            let mut imps = BTreeMap::<String, i32>::new();
            for (_, _, _, p, imp) in options.into_iter().take(worked as usize) {
                used.insert(p);
                improvements.push((p, imp.to_owned(), index));
                *imps.entry(imp.to_owned()).or_default() += 1;
            }
            occupied.insert(center);
            units.push((s(&plan["garrison"][0]).to_owned(), index, center));
            plans.push(json!({"city":city,"plan":plan,"districts":placed,"improvements":imps}));
        }
        let reserves = prosperity::faction_reserves(
            slug,
            &plans.iter().map(|p| p["plan"].clone()).collect::<Vec<_>>(),
        );
        let mut order: Vec<_> = plans.iter().collect();
        order.sort_by_key(|p| {
            (
                p["city"]["capital"] != true,
                -n(&p["plan"]["importance"]),
                n(&p["city"]["research_priority"]),
            )
        });
        let mut slots = vec![];
        for reach in [2, 3, grid.width + grid.height] {
            for x in &order {
                let c = n(&x["city"]["plot"]);
                let mut list = city_tiles
                    .get(&c)
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
                    .iter()
                    .copied()
                    .filter(|p| {
                        !centers.contains(p)
                            && !impassable.contains(p)
                            && !water(&terrain[p])
                            && dist(*p, c) <= reach
                    })
                    .collect::<Vec<_>>();
                list.sort_by_key(|&p| (dist(p, c), p));
                slots.extend(list);
            }
        }
        let mut unplaced = vec![];
        for u in reserves {
            if let Some(&p) = slots.iter().find(|p| !occupied.contains(p)) {
                occupied.insert(p);
                units.push((u, index, p));
            } else {
                unplaced.push(u);
            }
        }
        let gold = prosperity::starting_gold(slug);
        db.execute(
            "INSERT INTO PlayerAttributes VALUES (?,'Treasury','Balance',?)",
            params![index, format!("{:.6}", gold as f64)],
        )?;
        let own_units: Vec<_> = units.iter().filter(|u| u.1 == index).collect();
        let bs = generate::counts(
            plans
                .iter()
                .flat_map(|x| {
                    arr(&x["plan"]["city_buildings"]).iter().chain(
                        arr(&x["districts"])
                            .iter()
                            .flat_map(|d| arr(&d["buildings"]).iter()),
                    )
                })
                .map(s),
        );
        report.insert(slug.clone(),json!({"tier":prosperity::tier_of(slug),"cities":plans.len(),"pop":plans.iter().map(|x|n(&x["plan"]["pop"])).sum::<i32>(),"units":own_units.len(),"districts":plans.iter().map(|x|arr(&x["districts"]).len()).sum::<usize>(),"walls":plans.iter().filter(|x|arr(&x["plan"]["city_buildings"]).iter().any(|b|b=="BUILDING_WALLS")).count(),"buildings":bs,"improvements":plans.iter().map(|x|x["improvements"].as_object().unwrap().values().map(n).sum::<i32>()).sum::<i32>(),"gold":gold,"upkeep":plans.iter().map(|x|prosperity::upkeep_of(&[],arr(&x["districts"]))).sum::<i32>()+own_units.iter().map(|u|prosperity::unit_upkeep(&u.0)).sum::<i32>(),"unplaced":unplaced,"missing_districts":plans.iter().map(|x|n(&x["plan"]["n_districts"])-arr(&x["districts"]).len() as i32).sum::<i32>()}));
    }
    for (p, o, c, w) in owner_adds {
        db.execute("INSERT INTO PlotOwners VALUES (?,?,?,?)", [p, o, c, w])?;
    }
    for (d, c, p) in districts {
        db.execute("INSERT INTO Districts VALUES (?,?,?)", params![d, c, p])?;
    }
    for (b, p) in buildings {
        db.execute("INSERT INTO Buildings VALUES (?,?)", params![b, p])?;
    }
    for (p, i, o) in improvements {
        db.execute(
            "INSERT INTO PlotImprovements VALUES (?,?,?)",
            params![p, i, o],
        )?;
    }
    for (i, (u, o, p)) in units.iter().enumerate() {
        db.execute(
            "INSERT INTO Units VALUES (?,?,?,?,NULL)",
            params![i as i32, u, o, p],
        )?;
    }
    Ok(Value::Object(report))
}
pub fn map_path(maps: &Path, km: i32) -> PathBuf {
    maps.join(format!("JAC_Japan_{km}km.Civ6Map"))
}
pub fn settler_features(maps: &Path, km: i32) -> Result<Vec<(i32, String)>> {
    let db = Connection::open_with_flags(
        maps.join(format!("JAC_Japan_{km}km_Settlers.Civ6Map")),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    Ok(
        string_rows(&db, "SELECT ID,FeatureType FROM PlotFeatures ORDER BY ID")?
            .into_iter()
            .collect(),
    )
}
pub fn apply_file(path: &Path, maps: &Path, km: i32) -> Result<Value> {
    let item = prosperity::manifest_item(maps, km)?;
    let features = settler_features(maps, km)?;
    let db = Connection::open(path)?;
    db.execute_batch("BEGIN IMMEDIATE")?;
    let report = apply(
        &db,
        prosperity::grid(&item),
        arr(&item["cities"]),
        Some(&features),
    )?;
    db.execute_batch("COMMIT;VACUUM;")?;
    Ok(report)
}
pub fn report_for(maps: &Path, km: i32) -> Result<Value> {
    let tmp = tempfile::tempdir()?;
    let copy = tmp.path().join("map.Civ6Map");
    fs::copy(map_path(maps, km), &copy)?;
    apply_file(&copy, maps, km)
}
pub fn lua_start_check(report: &Value) -> String {
    let expected = report
        .as_object()
        .unwrap()
        .iter()
        .map(|(slug, row)| {
            format!(
                "{}={{{}}}",
                start_1534::leader_id(slug),
                [
                    "cities",
                    "pop",
                    "units",
                    "districts",
                    "walls",
                    "improvements",
                    "gold"
                ]
                .iter()
                .map(|k| n(&row[k]).to_string())
                .collect::<Vec<_>>()
                .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let buildings = report
        .as_object()
        .unwrap()
        .iter()
        .map(|(slug, row)| {
            format!(
                "{}={{{}}}",
                start_1534::leader_id(slug),
                row["buildings"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| format!("{k}={}", n(v)))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "local E={{{expected}}}; local EB={{{buildings}}}; {}",
        include_str!("../tests/fixtures/start-check-tail.lua")
    )
}
pub const SURVIVORS: &str = include_str!("../tests/fixtures/survivors.lua");
pub fn write_scenario(report: &Value, path: &Path) -> Result<()> {
    let mut data = read_json(path)?;
    let steps = data["steps"].as_array_mut().context("scenario steps")?;
    let start = steps
        .iter()
        .position(|s| s["action"] == "start_game")
        .context("start_game missing")?;
    anyhow::ensure!(start + 1 < steps.len(), "start check missing");
    steps[start + 1]["code"] = json!(lua_start_check(report));
    steps.last_mut().unwrap()["code"] = json!(SURVIVORS);
    write_json(path, &data)
}
