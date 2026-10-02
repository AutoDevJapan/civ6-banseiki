use crate::generate::{hex_distance, HexGrid};
use crate::*;
use anyhow::ensure;
use constants::SEATS;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::LazyLock,
};
pub static SPEC: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../tests/fixtures/prosperity-spec.json")).unwrap()
});
pub const KYOTO: &str = "ASHIKAGA_SEAT";
pub const GOLD_FLOOR: i32 = 100;
pub const TRAIT_NAMES: [&str; 6] = ["martial", "trade", "port", "faith", "mine", "learn"];
pub const TRAIT_ORDER: [&str; 6] = ["trade", "port", "martial", "faith", "mine", "learn"];
pub const DISTRICTS: [&str; 6] = [
    "DISTRICT_ENCAMPMENT",
    "DISTRICT_COMMERCIAL_HUB",
    "DISTRICT_HARBOR",
    "DISTRICT_HOLY_SITE",
    "DISTRICT_INDUSTRIAL_ZONE",
    "DISTRICT_CAMPUS",
];
pub const BUILDINGS: [&str; 6] = [
    "BUILDING_BARRACKS",
    "BUILDING_MARKET",
    "BUILDING_LIGHTHOUSE",
    "BUILDING_SHRINE",
    "BUILDING_WORKSHOP",
    "BUILDING_LIBRARY",
];
pub fn water(t: &str) -> bool {
    t == "TERRAIN_COAST" || t == "TERRAIN_OCEAN"
}
pub fn importance(city: &Value) -> i32 {
    if city["capital"] == true {
        3
    } else {
        n(&constants::CONSTANTS["importance"][s(&city["id"])])
    }
}
pub fn no_walls(id: &str) -> bool {
    id == "ASHIKAGA_KUWANOMI"
}
pub fn tier_of(slug: &str) -> &'static str {
    let k = f(&SPEC["factions"][slug]["k"][1]);
    if k >= 40. {
        "A"
    } else if k >= 20. {
        "B"
    } else if k >= 10. {
        "C"
    } else {
        "D"
    }
}
pub fn field_units(slug: &str) -> i32 {
    (round(f(&SPEC["factions"][slug]["k"][1]).sqrt() / 1.5, 0) as i32).max(1)
}
pub fn starting_gold(slug: &str) -> i32 {
    let spec = &SPEC["factions"][slug];
    (round(
        (10. * f(&spec["k"][1]) + 40. * f(&spec["traits"]["trade"])) / 10.,
        0,
    ) as i32
        * 10)
        .max(GOLD_FLOOR)
        + if slug == "ASHIKAGA" { 100 } else { 0 }
}
pub fn unit_upkeep(unit: &str) -> i32 {
    match unit {
        "UNIT_SPEARMAN" | "UNIT_ARCHER" => 1,
        _ => 0,
    }
}
pub fn building_upkeep(b: &str) -> i32 {
    match b {
        "BUILDING_BARRACKS" | "BUILDING_SHRINE" | "BUILDING_LIBRARY" | "BUILDING_WORKSHOP" => 1,
        "BUILDING_TEMPLE" => 2,
        _ => 0,
    }
}
pub fn district_upkeep(d: &str) -> i32 {
    match d {
        "DISTRICT_ENCAMPMENT"
        | "DISTRICT_HOLY_SITE"
        | "DISTRICT_CAMPUS"
        | "DISTRICT_INDUSTRIAL_ZONE" => 1,
        _ => 0,
    }
}
pub fn upkeep_of(units: &[String], districts: &[Value]) -> i32 {
    units.iter().map(|u| unit_upkeep(u)).sum::<i32>()
        + districts
            .iter()
            .map(|d| {
                district_upkeep(s(&d["district"]))
                    + arr(&d["buildings"])
                        .iter()
                        .map(|b| building_upkeep(s(b)))
                        .sum::<i32>()
            })
            .sum::<i32>()
}
pub type Counter = BTreeMap<String, i32>;
fn count(rec: &Counter, key: &str) -> i32 {
    *rec.get(key).unwrap_or(&0)
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CityInfo {
    pub rec: Counter,
    pub owner: i32,
    pub coastal: bool,
    pub near_slots: i32,
}
pub fn city_plan(slug: &str, city: &Value, rec: &Counter, coastal: bool) -> Value {
    let spec = &SPEC["factions"][slug];
    let capital = city["capital"] == true;
    let imp = importance(city);
    let base = match tier_of(slug) {
        "A" => 6,
        "B" => 5,
        "C" => 4,
        _ => 3,
    };
    let target = (base + if capital { n(&spec["cap_bonus"]) } else { 0 } + imp - 3).max(2);
    let land = count(rec, "tiles") - 1 - count(rec, "mountain");
    let pop = target.min(land).max(2);
    let mut num = if pop < 3 {
        0
    } else if pop < 6 {
        1
    } else if pop < 9 {
        2
    } else {
        3
    };
    num = num.min(land - pop).max(0);
    if !capital {
        num = num.min(if imp >= 2 { 1 } else { 0 });
    }
    if s(&city["id"]) == KYOTO {
        num = 2;
    }
    let mut scores: Vec<usize> = (0..6).collect();
    scores.sort_by_key(|&i| {
        (
            -n(&spec["traits"][TRAIT_NAMES[i]]),
            TRAIT_ORDER
                .iter()
                .position(|t| *t == TRAIT_NAMES[i])
                .unwrap(),
        )
    });
    let mut options = vec![];
    for i in scores {
        if n(&spec["traits"][TRAIT_NAMES[i]]) == 0 || DISTRICTS[i] == "DISTRICT_HARBOR" && !coastal
        {
            continue;
        }
        let mut buildings = vec![BUILDINGS[i]];
        if capital && DISTRICTS[i] == "DISTRICT_HOLY_SITE" && n(&spec["traits"]["faith"]) == 3 {
            buildings.push("BUILDING_TEMPLE");
        }
        options.push(json!({"district":DISTRICTS[i],"buildings":buildings}));
    }
    let mut buildings = vec![];
    if capital {
        buildings.push("BUILDING_PALACE");
    }
    if (capital || imp >= 2) && !no_walls(s(&city["id"])) {
        buildings.push("BUILDING_WALLS");
    }
    if pop >= 4 {
        buildings.push("BUILDING_GRANARY");
    }
    let mut garrison = if capital || imp >= 2 {
        vec!["UNIT_SPEARMAN"]
    } else {
        vec!["UNIT_WARRIOR"]
    };
    if capital && ["A", "B"].contains(&tier_of(slug)) {
        garrison.push("UNIT_ARCHER");
    }
    json!({"importance":imp,"target_pop":target,"pop":pop,"land":land,"districts":options.iter().take(num as usize).collect::<Vec<_>>(),"district_options":options,"n_districts":num,"city_buildings":buildings,"garrison":garrison})
}
pub fn faction_reserves(slug: &str, plans: &[Value]) -> Vec<String> {
    let order = [
        "UNIT_SPEARMAN",
        "UNIT_ARCHER",
        "UNIT_WARRIOR",
        "UNIT_SPEARMAN",
        "UNIT_ARCHER",
        "UNIT_SPEARMAN",
        "UNIT_WARRIOR",
        "UNIT_SPEARMAN",
    ];
    plans
        .iter()
        .flat_map(|p| arr(&p["garrison"]).iter().skip(1).map(|u| s(u).to_owned()))
        .chain((0..field_units(slug)).map(|i| order[i as usize % order.len()].to_owned()))
        .collect()
}
pub fn territory_koku() -> Result<BTreeMap<String, f64>> {
    let mut total = BTreeMap::new();
    for e in arr(&start_1534::data()?["province_allocations"]) {
        let province = s(&e["province"]);
        let Some(k) = SPEC["koku_1598"].get(province) else {
            continue;
        };
        let owners: BTreeSet<_> = arr(&e["parts"])
            .iter()
            .map(|p| s(&p["owner_faction"]).to_owned())
            .collect();
        let shares = SPEC["split_shares"]
            .get(province)
            .cloned()
            .unwrap_or_else(|| {
                if owners.len() == 1 {
                    json!({owners.first().unwrap():1.0})
                } else {
                    Value::Null
                }
            });
        let shares = shares.as_object().context("province split not defined")?;
        ensure!(
            shares.keys().cloned().collect::<BTreeSet<_>>() == owners
                && (shares.values().map(f).sum::<f64>() - 1.).abs() < 1e-9,
            "province split not defined: {province}"
        );
        for (o, share) in shares {
            *total.entry(o.clone()).or_default() += f(k) * f(share);
        }
    }
    Ok(total)
}
pub fn string_rows(db: &Connection, sql: &str) -> Result<BTreeMap<i32, String>> {
    Ok(db
        .prepare(sql)?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?)
}
pub fn owner_rows(db: &Connection) -> Result<Vec<(i32, i32, i32)>> {
    Ok(db
        .prepare("SELECT ID,Owner,CityOwner FROM PlotOwners ORDER BY ID")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?)
}
pub fn read_db(db: &Connection, grid: HexGrid) -> Result<BTreeMap<i32, CityInfo>> {
    let terrain = string_rows(db, "SELECT ID,TerrainType FROM Plots")?;
    let feature = string_rows(db, "SELECT ID,FeatureType FROM PlotFeatures")?;
    let resource = string_rows(db, "SELECT ID,ResourceType FROM PlotResources")?;
    let owners: Vec<_> = owner_rows(db)?
        .into_iter()
        .filter(|r| !water(&terrain[&r.0]))
        .collect();
    let mut by_city: BTreeMap<i32, Counter> = BTreeMap::new();
    for &(p, _, c) in &owners {
        let rec = by_city.entry(c).or_default();
        *rec.entry("tiles".into()).or_default() += 1;
        if terrain[&p].ends_with("MOUNTAIN") {
            *rec.entry("mountain".into()).or_default() += 1;
        } else if terrain[&p].ends_with("HILLS") {
            *rec.entry("hills".into()).or_default() += 1;
        }
        if feature.get(&p).is_some_and(|v| v == "FEATURE_FOREST") {
            *rec.entry("forest".into()).or_default() += 1;
        }
        if let Some(r) = resource.get(&p) {
            *rec.entry(format!("res:{r}")).or_default() += 1;
        }
    }
    let mut out = BTreeMap::new();
    for row in db
        .prepare("SELECT Plot,Owner FROM Cities ORDER BY Plot")?
        .query_map([], |r| Ok((r.get::<_, i32>(0)?, r.get::<_, i32>(1)?)))?
    {
        let (p, owner) = row?;
        let coastal = grid.neighbor_ids(p).iter().any(|n| water(&terrain[n]));
        let near_slots = owners
            .iter()
            .filter(|&&(n, _, c)| {
                c == p
                    && n != p
                    && !terrain[&n].ends_with("MOUNTAIN")
                    && hex_distance(grid.xy(n), grid.xy(p)) <= 2
            })
            .count() as i32;
        out.insert(
            p,
            CityInfo {
                rec: by_city.remove(&p).unwrap_or_default(),
                owner,
                coastal,
                near_slots,
            },
        );
    }
    Ok(out)
}
pub fn manifest_item(maps: &Path, km: i32) -> Result<Value> {
    read_json(maps.join("manifest.json"))?
        .as_array()
        .context("manifest array")?
        .iter()
        .find(|m| n(&m["km_per_tile"]) == km)
        .cloned()
        .context("map scale missing")
}
pub fn grid(item: &Value) -> HexGrid {
    HexGrid {
        km: n(&item["km_per_tile"]),
        width: n(&item["width"]),
        height: n(&item["height"]),
        xmin: 0.,
        ymin: 0.,
    }
}
pub fn read_map(maps: &Path, km: i32) -> Result<BTreeMap<i32, CityInfo>> {
    let item = manifest_item(maps, km)?;
    let db = Connection::open_with_flags(
        maps.join(format!("JAC_Japan_{km}km.Civ6Map")),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    read_db(&db, grid(&item))
}
pub fn distribute_units(plans: &mut [Value], mut extra: Vec<String>) -> Vec<String> {
    let mut order: Vec<_> = (0..plans.len()).collect();
    order.sort_by_key(|&i| {
        (
            plans[i]["capital"] != true,
            -n(&plans[i]["near_slots_15km"]),
        )
    });
    for i in order {
        let p = &mut plans[i];
        let room = n(&p["near_slots_15km"])
            - arr(&p["units"])
                .iter()
                .filter(|u| u["where"] == "near")
                .count() as i32;
        for _ in 0..room {
            if extra.is_empty() {
                break;
            }
            p["units"]
                .as_array_mut()
                .unwrap()
                .push(json!({"type":extra.remove(0),"where":"near"}));
        }
    }
    extra
}
pub fn build(maps: &Path) -> Result<Value> {
    let info15 = read_map(maps, 15)?;
    let info20 = read_map(maps, 20)?;
    let kterr = territory_koku()?;
    let mut tiles = BTreeMap::<String, i32>::new();
    for i in info15.values() {
        *tiles
            .entry(SEATS[i.owner as usize].slug.clone())
            .or_default() += count(&i.rec, "tiles");
    }
    let ksum = SPEC["factions"]
        .as_object()
        .unwrap()
        .values()
        .filter(|v| v["koku_equivalent"] != true)
        .map(|v| f(&v["k"][1]))
        .sum::<f64>();
    let total = tiles.values().sum::<i32>();
    let m15 = manifest_item(maps, 15)?;
    let m20 = manifest_item(maps, 20)?;
    let p20: BTreeMap<_, _> = arr(&m20["cities"])
        .iter()
        .map(|c| (s(&c["id"]), n(&c["plot"])))
        .collect();
    let mut out = vec![];
    for seat in SEATS.iter() {
        let slug = &seat.slug;
        let spec = &SPEC["factions"][slug];
        let own = crate::generate::ordered_cities(
            &arr(&m15["cities"])
                .iter()
                .filter(|c| c["owner"] == *slug)
                .cloned()
                .collect::<Vec<_>>(),
        );
        let mut plans = vec![];
        let mut garrisons = vec![];
        for c in &own {
            let i15 = &info15[&n(&c["plot"])];
            let i20 = &info20[&p20[s(&c["id"])]];
            let plan = city_plan(slug, c, &i15.rec, i15.coastal);
            let rec = &i15.rec;
            let flat = (count(rec, "tiles")
                - 1
                - count(rec, "mountain")
                - count(rec, "hills")
                - count(rec, "forest"))
            .max(0);
            let worked = (n(&plan["pop"]) - arr(&plan["districts"]).len() as i32).max(0);
            let iron = count(rec, "hills")
                .min(count(rec, "res:RESOURCE_IRON"))
                .min(worked);
            let farms = flat.min(worked - iron);
            let mines = count(rec, "hills").min(worked - farms);
            garrisons.push(json!({"garrison":plan["garrison"]}));
            plans.push(json!({"id":c["id"],"name":c["name"],"capital":c["capital"],"importance":plan["importance"],"target_pop":plan["target_pop"],"tiles_15km":count(rec,"tiles"),"tiles_20km":count(&i20.rec,"tiles"),"land_15km":plan["land"],"pop_15km":plan["pop"],"pop_20km":city_plan(slug,c,&i20.rec,i20.coastal)["pop"],"option2_pop_15km":n(&plan["land"]).clamp(2,4),"coastal_centre":i15.coastal,"near_slots_15km":i15.near_slots,"city_buildings":plan["city_buildings"],"districts":plan["districts"],"units":[{"type":plan["garrison"][0],"where":"center"}],"improvements":{"IMPROVEMENT_FARM":farms,"IMPROVEMENT_MINE":mines},"unverified_improvements":{"IMPROVEMENT_PASTURE":count(rec,"res:RESOURCE_HORSES"),"IMPROVEMENT_PLANTATION":count(rec,"res:RESOURCE_SILK")+count(rec,"res:RESOURCE_TEA")}}));
        }
        let unplaced = distribute_units(&mut plans, faction_reserves(slug, &garrisons));
        let upkeep = plans
            .iter()
            .map(|p| {
                upkeep_of(
                    &arr(&p["units"])
                        .iter()
                        .map(|u| s(&u["type"]).to_owned())
                        .collect::<Vec<_>>(),
                    arr(&p["districts"]),
                )
            })
            .sum::<i32>();
        for p in &mut plans {
            p.as_object_mut().unwrap().remove("near_slots_15km");
        }
        let k = &spec["k"];
        out.push(json!({"id":slug,"name":seat.ja,"tier":tier_of(slug),"confidence":spec["conf"],"direct_koku_man":{"low":k[0],"mid":k[1],"high":k[2]},"koku_equivalent":spec["koku_equivalent"]==true,"direct_koku_basis":spec["basis"],"mobilization_troops":{"low":round(f(&k[0])*250.,0) as i32,"mid":round(f(&k[1])*250.,0) as i32,"high":round(f(&k[2])*250.,0) as i32},"territory_koku_1598_man":round(kterr.get(slug).copied().unwrap_or(0.)/10000.,1),"tiles_15km":tiles[slug],"tile_share_pct":round(100.*tiles[slug] as f64/total as f64,1),"direct_koku_share_pct":if spec["koku_equivalent"]==true{Value::Null}else{json!(round(100.*f(&k[1])/ksum,1))},"traits":spec["traits"],"cities":plans.len(),"pop_total_15km":plans.iter().map(|p|n(&p["pop_15km"])).sum::<i32>(),"units_total":plans.iter().map(|p|arr(&p["units"]).len()).sum::<usize>(),"option2_pop_total_15km":plans.iter().map(|p|n(&p["option2_pop_15km"])).sum::<i32>(),"option2_units_total":plans.len()+3,"units_unplaced":unplaced,"gold_upkeep_per_turn":upkeep,"starting_gold":starting_gold(slug),"plan":plans}));
    }
    let reference: Value =
        serde_json::from_str(include_str!("../tests/fixtures/prosperity-metadata.json"))?;
    Ok(
        json!({"_schema":reference["_schema"],"option2_rule":reference["option2_rule"],"koku_1598":SPEC["koku_1598"],"split_shares":SPEC["split_shares"],"factions":out}),
    )
}
