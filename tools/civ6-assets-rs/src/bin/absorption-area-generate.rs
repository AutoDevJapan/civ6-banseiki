//! Geographic reference areas, independent of Civ6 tiles and their current owner.
use anyhow::{ensure, Result};
use civ6_assets_rs::{constants::CONSTANTS, generate::*, start_1534, *};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 1,
        "usage: absorption-area-generate <source-cache>"
    );
    let cache = PathBuf::from(&args[0]);
    obtain_sources(&cache)?;
    let shapes = read_shapes(
        &cache.join(SOURCES[1].0),
        "ne_10m_admin_0_countries",
        Some("JPN"),
    )?;
    let draft = start_1534::data()?;
    let mut anchors: Vec<_> = arr(&draft["castles"])
        .iter()
        .map(|c| {
            (
                s(&c["owner_faction_1534_draft"]).to_owned(),
                project(f(&c["lon"]), f(&c["lat"])),
            )
        })
        .collect();
    anchors.extend(
        arr(&CONSTANTS["territory_anchors"])
            .iter()
            .map(|a| (s(&a[1]).to_owned(), project(f(&a[2]), f(&a[3])))),
    );
    let points: Vec<_> = shapes
        .iter()
        .flat_map(|r| &r.points)
        .filter(|&&(x, y)| in_scope(x, y))
        .map(|&(x, y)| project(x, y))
        .collect();
    let minx = points
        .iter()
        .map(|p| p.0)
        .fold(f64::INFINITY, f64::min)
        .floor();
    let maxx = points
        .iter()
        .map(|p| p.0)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil();
    let miny = points
        .iter()
        .map(|p| p.1)
        .fold(f64::INFINITY, f64::min)
        .floor();
    let maxy = points
        .iter()
        .map(|p| p.1)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil();
    let mut areas = BTreeMap::<String, u64>::new();
    // One square kilometre in a Lambert azimuthal equal-area projection. This
    // grid is an integration method, never a game tile or a count of ownership.
    for y in miny as i32..maxy as i32 {
        for x in minx as i32..maxx as i32 {
            let p = (x as f64 + 0.5, y as f64 + 0.5);
            let (lon, lat) = unproject(p.0, p.1);
            if !in_scope(lon, lat) || !shapes.iter().any(|r| r.contains(lon, lat)) {
                continue;
            }
            let forced = if lat >= 41.8 {
                Some("AINU")
            } else if lat < 29.2 {
                Some("RYUKYU")
            } else {
                None
            };
            let owner = anchors
                .iter()
                .filter(|a| forced.map(|v| a.0 == v).unwrap_or(a.0 != "AINU"))
                .min_by(|a, b| {
                    squared_distance(a.1, p)
                        .total_cmp(&squared_distance(b.1, p))
                        .then(a.0.cmp(&b.0))
                })
                .expect("geographic country anchor");
            *areas.entry(owner.0.clone()).or_default() += 1;
        }
    }
    ensure!(
        areas.len() == 22 && areas.values().all(|v| *v > 0),
        "all current countries need geographic areas"
    );
    write_json(
        root().join("documents/spec/data/absorption-country-areas.json"),
        &json!({
            "status":"暫定。現1534地図の地理配分を面積積分した分類用参照値。史実の直轄領の確定面積ではない",
            "source":SOURCES[1].1,"source_sha256":SOURCES[1].2,
            "method":"Natural Earth Japan land; Lambert azimuthal equal-area (6371.0088 km); 1 km2 square midpoint integration; original castle WGS84 positions plus current province anchors; nearest anchor; latitude>=41.8 AINU, <29.2 RYUKYU",
            "basis_date":"1534年の現製品の地理配分。海岸線はNatural Earth 10m。ゲーム中の領土増減では変更しない",
            "areas_km2":areas
        }),
    )
}
