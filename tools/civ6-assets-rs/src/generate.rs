use crate::*;
use anyhow::ensure;
pub use constants::{seat_index, ISLAND_ANCHORS, SCALES, SEATS};
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, BinaryHeap},
    io::Read,
};
pub type Tiles = BTreeSet<i32>;
pub type Allocation = (i32, i32, i32, i32);
pub type Edge = (i32, String);
pub type Point = (f64, f64);
pub const CITY_MIN_RANGE: i32 = 1;
pub const MIN_CITY_DISTANCE: i32 = 2;
pub const MAX_NUDGE_TILES: i32 = 3;
pub const START_YEAR: i32 = 1534;
pub const SOURCES:[(&str,&str,&str);3]=[
 ("ne_10m_land.zip","https://naturalearth.s3.amazonaws.com/10m_physical/ne_10m_land.zip","e547d749445eaa0964aba76738090ec88f5e63c4585122170f98c67a7ea922dc"),
 ("ne_10m_admin_0_countries.zip","https://naturalearth.s3.amazonaws.com/10m_cultural/ne_10m_admin_0_countries.zip","ce1ac7036499a0edd641fbc093cd209a98f96a49d2eca8480aaacad35138a7f6"),
 ("etopo_2022_japan_3arcmin.csv","https://oceanwatch.pifsc.noaa.gov/erddap/griddap/ETOPO_2022_v1_60s.csv?z%5B(23):3:(47)%5D%5B(121):3:(155)%5D","1c04df76876d71b5424961f90771922641f8e85bc7fec5de70f688d59b60082d")];
pub fn mod_dir() -> PathBuf {
    root().join("poc/JAC_PoC_JapanMapScale")
}
pub fn official() -> PathBuf {
    game_dir().join("Base/Assets/Maps/EarthMaps/WorldMapHuge.Civ6Map")
}
pub fn game_dir() -> PathBuf {
    std::env::var_os("CIV6_INSTALL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI",
            )
        })
}
pub fn in_scope(lon: f64, lat: f64) -> bool {
    (122.0..=146.0).contains(&lon) && (23.5..=46.5).contains(&lat) && !(lat < 31. && lon >= 139.)
}
pub fn project(lon: f64, lat: f64) -> Point {
    let l = lon.to_radians() - 138f64.to_radians();
    let p = lat.to_radians();
    let p0 = 36f64.to_radians();
    let k = (2. / (1. + p0.sin() * p.sin() + p0.cos() * p.cos() * l.cos())).sqrt();
    let x = 6371.0088 * k * p.cos() * l.sin();
    let y = 6371.0088 * k * (p0.cos() * p.sin() - p0.sin() * p.cos() * l.cos());
    let r = 45f64.to_radians();
    (x * r.cos() + y * r.sin(), -x * r.sin() + y * r.cos())
}
pub fn unproject(x: f64, y: f64) -> Point {
    let r = 45f64.to_radians();
    let (x, y) = (x * r.cos() - y * r.sin(), x * r.sin() + y * r.cos());
    let rho = x.hypot(y);
    if rho < 1e-12 {
        return (138., 36.);
    }
    let c = 2. * (rho / (2. * 6371.0088)).min(1.).asin();
    let p0 = 36f64.to_radians();
    let p = (c.cos() * p0.sin() + y * c.sin() * p0.cos() / rho).asin();
    let l = 138f64.to_radians()
        + (x * c.sin()).atan2(rho * p0.cos() * c.cos() - y * p0.sin() * c.sin());
    (l.to_degrees(), p.to_degrees())
}
pub fn distance(a: Point, b: Point) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}
pub fn squared_distance(a: Point, b: Point) -> f64 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HexGrid {
    pub km: i32,
    pub width: i32,
    pub height: i32,
    pub xmin: f64,
    pub ymin: f64,
}
impl HexGrid {
    pub fn for_scale(km: i32, japan: &[Ring]) -> Result<Self> {
        ensure!(km > 0, "scale must be positive");
        let points: Vec<_> = japan
            .iter()
            .flat_map(|r| &r.points)
            .filter(|&&(x, y)| in_scope(x, y))
            .map(|&(x, y)| project(x, y))
            .collect();
        ensure!(!points.is_empty(), "Japan polygons missing");
        let minx = points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let maxx = points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        let miny = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let maxy = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        let k = km as f64;
        let xmin = minx - 2. * k;
        let ymin = miny - 2. * k;
        Ok(Self {
            km,
            width: ((maxx - xmin + 2. * k) / k).ceil() as i32,
            height: ((maxy - ymin + 2. * k) / (3f64.sqrt() / 2. * k)).ceil() as i32,
            xmin,
            ymin,
        })
    }
    pub fn centre(&self, col: i32, row: i32) -> Point {
        (
            self.xmin + (col as f64 + 0.5 + 0.5 * (row & 1) as f64) * self.km as f64,
            self.ymin + (row as f64 + 0.5) * 3f64.sqrt() / 2. * self.km as f64,
        )
    }
    pub fn closest(&self, lon: f64, lat: f64) -> (i32, i32) {
        let p = project(lon, lat);
        let nominal = ((p.1 - self.ymin) / (3f64.sqrt() / 2. * self.km as f64) - 0.5)
            .round_ties_even() as i32;
        let nominal = nominal.clamp(0, self.height - 1);
        let mut choices = vec![];
        for row in (nominal - 2).max(0)..(nominal + 3).min(self.height) {
            let col = ((p.0 - self.xmin) / self.km as f64 - 0.5 - 0.5 * (row & 1) as f64)
                .round_ties_even() as i32;
            let col = col.clamp(0, self.width - 1);
            for c in (col - 1).max(0)..(col + 2).min(self.width) {
                choices.push((distance(p, self.centre(c, row)), c, row));
            }
        }
        let v = choices
            .into_iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        (v.1, v.2)
    }
    pub fn neighbours(&self, col: i32, row: i32) -> Vec<(i32, i32)> {
        [
            (-1, 0),
            (1, 0),
            (row & 1, -1),
            ((row & 1) - 1, -1),
            (row & 1, 1),
            ((row & 1) - 1, 1),
        ]
        .into_iter()
        .map(|(x, y)| (col + x, row + y))
        .filter(|&(c, r)| c >= 0 && c < self.width && r >= 0 && r < self.height)
        .collect()
    }
    pub fn index(&self, col: i32, row: i32) -> i32 {
        row * self.width + col
    }
    pub fn xy(&self, plot: i32) -> (i32, i32) {
        (plot % self.width, plot / self.width)
    }
    pub fn center(&self, plot: i32) -> Point {
        let (c, r) = self.xy(plot);
        self.centre(c, r)
    }
    pub fn neighbor_ids(&self, plot: i32) -> Vec<i32> {
        let (c, r) = self.xy(plot);
        self.neighbours(c, r)
            .into_iter()
            .map(|(c, r)| self.index(c, r))
            .collect()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ring {
    pub points: Vec<Point>,
    pub bbox: (f64, f64, f64, f64),
}
impl Ring {
    pub fn contains(&self, lon: f64, lat: f64) -> bool {
        let (a, b, c, d) = self.bbox;
        if lon < a || lon > c || lat < b || lat > d {
            return false;
        }
        let mut inside = false;
        for p in self.points.windows(2) {
            let ((x1, y1), (x2, y2)) = (p[0], p[1]);
            if (y1 > lat) != (y2 > lat) && lon < x1 + (lat - y1) * (x2 - x1) / (y2 - y1) {
                inside = !inside;
            }
        }
        inside
    }
}
pub fn contains(rings: &[Ring], lon: f64, lat: f64) -> bool {
    rings.iter().filter(|r| r.contains(lon, lat)).count() % 2 == 1
}
fn bytes<const N: usize>(data: &[u8], pos: usize) -> Result<[u8; N]> {
    Ok(data
        .get(pos..pos + N)
        .context("truncated shapefile")?
        .try_into()?)
}
pub fn read_shapes(zipped: &Path, stem: &str, country: Option<&str>) -> Result<Vec<Ring>> {
    let mut archive = zip::ZipArchive::new(fs::File::open(zipped)?)?;
    let mut raw = vec![];
    archive
        .by_name(&format!("{stem}.shp"))?
        .read_to_end(&mut raw)?;
    let chosen = if let Some(country) = country {
        let mut dbf = vec![];
        archive
            .by_name(&format!("{stem}.dbf"))?
            .read_to_end(&mut dbf)?;
        let h = u16::from_le_bytes(bytes(&dbf, 8)?) as usize;
        let len = u16::from_le_bytes(bytes(&dbf, 10)?) as usize;
        let count = u32::from_le_bytes(bytes(&dbf, 4)?) as usize;
        let mut off = 1;
        let mut field = None;
        for pos in (32..h - 1).step_by(32) {
            let name = &dbf[pos..pos + 11];
            let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
            let length = dbf[pos + 16] as usize;
            if &name[..end] == b"ADM0_A3" {
                field = Some((off, length));
            }
            off += length;
        }
        let (off, length) = field.context("ADM0_A3 DBF field missing")?;
        Some(
            (0..count)
                .filter(|&i| {
                    String::from_utf8_lossy(
                        dbf.get(h + i * len + off..h + i * len + off + length)
                            .unwrap_or_default(),
                    )
                    .trim()
                        == country
                })
                .collect::<BTreeSet<_>>(),
        )
    } else {
        None
    };
    let mut result = vec![];
    let (mut cursor, mut record) = (100usize, 0);
    while cursor < raw.len() {
        let words = u32::from_be_bytes(bytes(&raw, cursor + 4)?) as usize;
        let end = cursor + 8 + words * 2;
        ensure!(
            end <= raw.len() && end > cursor,
            "invalid shape record length"
        );
        if chosen.as_ref().is_none_or(|s| s.contains(&record)) {
            let typ = u32::from_le_bytes(bytes(&raw, cursor + 8)?);
            ensure!(
                [5, 15, 25].contains(&typ),
                "Unsupported shapefile polygon type {typ}"
            );
            let pc = u32::from_le_bytes(bytes(&raw, cursor + 44)?) as usize;
            let pts = u32::from_le_bytes(bytes(&raw, cursor + 48)?) as usize;
            let mut parts = (0..pc)
                .map(|i| Ok(u32::from_le_bytes(bytes(&raw, cursor + 52 + 4 * i)?) as usize))
                .collect::<Result<Vec<_>>>()?;
            parts.push(pts);
            let start = cursor + 52 + 4 * pc;
            for p in parts.windows(2) {
                ensure!(p[0] <= p[1] && p[1] <= pts, "invalid polygon part");
                let points = (p[0]..p[1])
                    .map(|j| {
                        Ok((
                            f64::from_le_bytes(bytes(&raw, start + 16 * j)?),
                            f64::from_le_bytes(bytes(&raw, start + 16 * j + 8)?),
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?;
                ensure!(!points.is_empty(), "empty polygon");
                let bbox = (
                    points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min),
                    points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min),
                    points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max),
                    points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max),
                );
                if bbox.2 >= 122. && bbox.0 <= 146. && bbox.3 >= 23.5 && bbox.1 <= 46.5 {
                    result.push(Ring { points, bbox });
                }
            }
        }
        cursor = end;
        record += 1;
    }
    Ok(result)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dem {
    pub lat0: f64,
    pub lon0: f64,
    pub rows: Vec<Vec<f64>>,
}
pub fn load_dem(path: &Path) -> Result<Dem> {
    let raw = fs::read(path)?;
    let text = String::from_utf8_lossy(&raw);
    let mut rows: Vec<Vec<f64>> = vec![];
    let mut first = None;
    let mut lat = None;
    for line in text.lines().skip(2) {
        let values = line
            .split(',')
            .map(str::parse::<f64>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(values.len() == 3, "invalid DEM row");
        first.get_or_insert((values[0], values[1]));
        if lat.is_none_or(|old: f64| (old - values[0]).abs() > 1e-5) {
            rows.push(vec![]);
            lat = Some(values[0]);
        }
        rows.last_mut().unwrap().push(values[2]);
    }
    ensure!(
        !rows.is_empty() && rows.iter().all(|r| r.len() == rows[0].len()),
        "ETOPO CSV grid is incomplete"
    );
    let (lat0, lon0) = first.unwrap();
    Ok(Dem { lat0, lon0, rows })
}
pub fn elevation(dem: &Dem, lon: f64, lat: f64) -> f64 {
    let y = ((lat - dem.lat0) / 0.05)
        .round_ties_even()
        .clamp(0., (dem.rows.len() - 1) as f64) as usize;
    let x = ((lon - dem.lon0) / 0.05)
        .round_ties_even()
        .clamp(0., (dem.rows[0].len() - 1) as f64) as usize;
    dem.rows[y][x]
}
pub fn obtain_sources(cache: &Path) -> Result<()> {
    fs::create_dir_all(cache)?;
    for (name, url, hash) in SOURCES {
        let path = cache.join(name);
        if !path.exists() {
            println!("Downloading {url}");
            let mut c = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
            c.args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--max-time",
                "120",
                url,
                "--output",
            ])
            .arg(&path);
            hidden(&mut c);
            ensure!(c.status()?.success(), "source download failed: {name}");
        }
        let actual = sha256(&fs::read(&path)?);
        ensure!(
            actual == hash,
            "Source hash changed: {name} {actual}; expected {hash}"
        );
    }
    Ok(())
}
pub fn create_schema(source: &Path, target: &Path) -> Result<Connection> {
    ensure!(
        source.is_file(),
        "Official Civ6 schema source missing: {}",
        source.display()
    );
    let original = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let statements=original.prepare("SELECT sql FROM sqlite_master WHERE type IN ('table','index') AND sql IS NOT NULL ORDER BY type DESC,name")?.query_map([],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    create_schema_statements(&statements, target)
}
pub fn create_schema_statements(statements: &[String], target: &Path) -> Result<Connection> {
    if let Some(p) = target.parent() {
        fs::create_dir_all(p)?;
    }
    if target.exists() {
        fs::remove_file(target)?;
    }
    let db = Connection::open(target)?;
    for sql in statements {
        if let Err(err) = db.execute_batch(sql) {
            drop(db);
            fs::remove_file(target)?;
            return Err(err.into());
        }
    }
    Ok(db)
}
pub fn hex_distance(a: (i32, i32), b: (i32, i32)) -> i32 {
    let ax = a.0 - (a.1 - (a.1 & 1)) / 2;
    let bx = b.0 - (b.1 - (b.1 & 1)) / 2;
    (ax - bx)
        .abs()
        .max((a.1 - b.1).abs())
        .max((ax + a.1 - bx - b.1).abs())
}
fn city_xy(c: &Value) -> (i32, i32) {
    (n(&c["col"]), n(&c["row"]))
}
fn close_pairs(cities: &[Value], field: &str) -> Vec<Value> {
    let mut out = vec![];
    for (i, a) in cities.iter().enumerate() {
        for b in &cities[i + 1..] {
            let d = hex_distance(city_xy(a), city_xy(b));
            if d <= CITY_MIN_RANGE {
                out.push(json!([a[field], b[field], d]));
            }
        }
    }
    out
}
pub fn close_start_pairs(starts: &[Value]) -> Vec<Value> {
    close_pairs(starts, "seat")
}
pub fn close_city_pairs(cities: &[Value]) -> Vec<Value> {
    close_pairs(cities, "id")
}
pub fn city_name_tag(city: &Value) -> String {
    if let Some(tag) = city["name_tag"].as_str() {
        if !tag.is_empty() {
            return tag.into();
        }
    }
    if city["capital"] == true {
        format!("LOC_JAC_1534_CAPITAL_{}", s(&city["owner"]))
    } else {
        format!("LOC_JAC_1534_CITY_{}", s(&city["id"]))
    }
}
pub fn ordered_cities(cities: &[Value]) -> Vec<Value> {
    let mut out = cities.to_vec();
    out.sort_by_key(|c| {
        (
            c["capital"] != true,
            if c["capital"] == true {
                seat_index(s(&c["owner"]))
            } else {
                n(&c["research_priority"])
            },
        )
    });
    out
}
pub fn choose_starts(grid: HexGrid, passable: &Tiles) -> Result<Vec<Value>> {
    let mut used = Tiles::new();
    let mut out = vec![];
    for seat in SEATS.iter() {
        let target = project(seat.lon, seat.lat);
        let (d, c, r) = passable
            .iter()
            .filter(|p| !used.contains(p))
            .map(|&p| {
                let (c, r) = grid.xy(p);
                (distance(grid.centre(c, r), target), c, r)
            })
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .context("no passable starts")?;
        ensure!(
            d <= 90f64.max(2.5 * grid.km as f64),
            "No suitable Japanese land start near {}: {d:.1} km",
            seat.castle
        );
        let plot = grid.index(c, r);
        used.insert(plot);
        out.push(json!({"seat":seat.slug,"castle":seat.castle,"civilization":seat.civ,"leader":seat.leader,"lon":seat.lon,"lat":seat.lat,"col":c,"row":r,"plot":plot,"displacement_km":round(d,1)}));
    }
    Ok(out)
}
pub fn choose_cities(
    grid: HexGrid,
    passable: &Tiles,
    starts: &[Value],
    castles: &[Value],
) -> Result<Vec<Value>> {
    let sites: BTreeMap<_, _> = castles.iter().map(|c| (s(&c["id"]), c)).collect();
    ensure!(
        sites.len() == 53
            && constants::CONSTANTS["castle_names"]
                .as_object()
                .unwrap()
                .keys()
                .all(|id| sites.contains_key(id.as_str())),
        "1534 castle/seat inventory changed; review capital aliases"
    );
    let mut used: Tiles = starts.iter().map(|c| n(&c["plot"])).collect();
    let mut cities = vec![];
    for start in starts {
        let slug = s(&start["seat"]);
        let alias = constants::seat_castle(slug);
        let mut c = start.clone();
        c["id"] = json!(alias
            .map(String::from)
            .unwrap_or_else(|| format!("{slug}_SEAT")));
        c["name"] = alias
            .map(|a| sites[a]["name"].clone())
            .unwrap_or_else(|| json!(constants::extra_capital(slug).expect("capital alias")));
        c["owner"] = json!(slug);
        c["capital"] = json!(true);
        c["research_priority"] = json!(0);
        c["province"] = alias
            .map(|a| sites[a]["province_or_region"].clone())
            .unwrap_or(json!("代表本拠"));
        cities.push(c);
    }
    for (priority, castle) in castles.iter().enumerate() {
        if SEATS
            .iter()
            .any(|seat| constants::seat_castle(&seat.slug) == Some(s(&castle["id"])))
        {
            continue;
        }
        let target = project(f(&castle["lon"]), f(&castle["lat"]));
        let (d, plot) = passable
            .iter()
            .filter(|p| !used.contains(p))
            .map(|&p| (distance(grid.center(p), target), p))
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .context("no castle plots")?;
        ensure!(
            d <= 50.,
            "Castle too far from land: {} {d:.1} km",
            s(&castle["id"])
        );
        used.insert(plot);
        let (c, r) = grid.xy(plot);
        cities.push(json!({"id":castle["id"],"name":castle["name"],"owner":castle["owner_faction_1534_draft"],"capital":false,"research_priority":priority+1,"province":castle["province_or_region"],"lon":f(&castle["lon"]),"lat":f(&castle["lat"]),"col":c,"row":r,"plot":plot,"displacement_km":round(d,1)}));
    }
    Ok(ordered_cities(&cities))
}
pub fn assign_land(grid: HexGrid, land: &Tiles, cities: &[Value]) -> Result<Vec<Allocation>> {
    let draft = start_1534::data()?;
    let anchors = arr(&constants::CONSTANTS["territory_anchors"]);
    for a in anchors {
        let allocation = arr(&draft["province_allocations"])
            .iter()
            .find(|p| p["province"] == a[0])
            .context("province anchor missing")?;
        ensure!(
            arr(&allocation["parts"])
                .iter()
                .any(|p| p["owner_faction"] == a[1]),
            "Territory anchor disagrees with 1534 allocation"
        );
    }
    let centers: Vec<_> = cities
        .iter()
        .map(|c| {
            (
                s(&c["owner"]).to_owned(),
                n(&c["plot"]),
                grid.center(n(&c["plot"])),
            )
        })
        .collect();
    let mut influence = centers.clone();
    influence.extend(
        anchors
            .iter()
            .map(|a| (s(&a[1]).to_owned(), -1, project(f(&a[2]), f(&a[3])))),
    );
    let seed = centers
        .iter()
        .find(|c| c.0 == "AINU")
        .context("Ainu capital")?
        .1;
    let mut island = Tiles::from([seed]);
    let mut frontier = vec![seed];
    while let Some(p) = frontier.pop() {
        for n in grid.neighbor_ids(p) {
            if land.contains(&n) && island.insert(n) {
                frontier.push(n);
            }
        }
    }
    let mut results = vec![];
    for &plot in land {
        let xy = grid.center(plot);
        let lat = unproject(xy.0, xy.1).1;
        let forced = if island.contains(&plot) || lat >= 41.8 {
            Some("AINU")
        } else if lat < 29.2 {
            Some("RYUKYU")
        } else {
            None
        };
        let owner = centers
            .iter()
            .find(|c| c.1 == plot)
            .map(|c| c.0.as_str())
            .unwrap_or_else(|| {
                influence
                    .iter()
                    .filter(|c| forced.map(|o| c.0 == o).unwrap_or(c.0 != "AINU"))
                    .min_by(|a, b| {
                        (squared_distance(a.2, xy), a.1)
                            .partial_cmp(&(squared_distance(b.2, xy), b.1))
                            .unwrap()
                    })
                    .unwrap()
                    .0
                    .as_str()
            });
        let closest = centers
            .iter()
            .filter(|c| c.0 == owner)
            .min_by(|a, b| {
                (squared_distance(a.2, xy), a.1)
                    .partial_cmp(&(squared_distance(b.2, xy), b.1))
                    .unwrap()
            })
            .unwrap();
        results.push((plot, seat_index(owner), closest.1, closest.1));
    }
    Ok(results)
}
pub fn nudge_adjacent_cities(
    grid: HexGrid,
    land: &Tiles,
    passable: &Tiles,
    cities: &mut [Value],
) -> Result<()> {
    let owners: BTreeMap<_, _> = assign_land(grid, land, cities)?
        .into_iter()
        .map(|(p, o, _, _)| (p, o))
        .collect();
    let mut occupied: Tiles = cities.iter().map(|c| n(&c["plot"])).collect();
    let mut placed = vec![];
    for c in cities.iter_mut() {
        if placed.iter().any(|&p| hex_distance(city_xy(c), p) < 2) {
            ensure!(c["capital"] != true, "Adjacent capitals: {}", s(&c["id"]));
            let before = n(&c["plot"]);
            let origin = grid.center(before);
            let historical = project(f(&c["lon"]), f(&c["lat"]));
            let choice = passable
                .iter()
                .filter(|p| {
                    !occupied.contains(p)
                        && owners.get(p) == Some(&seat_index(s(&c["owner"])))
                        && !placed
                            .iter()
                            .any(|&other| hex_distance(grid.xy(**p), other) < 2)
                })
                .map(|&p| {
                    (
                        distance(origin, grid.center(p)),
                        distance(historical, grid.center(p)),
                        p,
                    )
                })
                .filter(|v| v.0 <= MAX_NUDGE_TILES as f64 * grid.km as f64 + 1e-8)
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .context("No owned land within 3 tiles for city")?;
            occupied.remove(&before);
            occupied.insert(choice.2);
            let (col, row) = grid.xy(choice.2);
            c["plot"] = json!(choice.2);
            c["col"] = json!(col);
            c["row"] = json!(row);
            c["displacement_km"] = json!(round(choice.1, 1));
            c["nudge"] =
                json!({"from_plot":before,"to_plot":choice.2,"distance_km":round(choice.0,1)});
        }
        placed.push(city_xy(c));
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct HistoricalBatch {
    pub sql: &'static str,
    pub rows: Vec<Vec<rusqlite::types::Value>>,
}
pub fn historical_state_batches(
    cities: &[Value],
    allocations: &[Allocation],
) -> Vec<HistoricalBatch> {
    use rusqlite::types::Value as Sql;
    let ordered = ordered_cities(cities);
    let mut batches = vec![
        HistoricalBatch {
            sql: "INSERT INTO Cities VALUES (?, ?, ?)",
            rows: ordered
                .iter()
                .map(|c| {
                    vec![
                        Sql::Integer(seat_index(s(&c["owner"])) as i64),
                        Sql::Integer(n(&c["plot"]) as i64),
                        Sql::Text(city_name_tag(c)),
                    ]
                })
                .collect(),
        },
        HistoricalBatch {
            sql: "INSERT INTO Districts VALUES ('DISTRICT_CITY_CENTER', ?, ?)",
            rows: ordered
                .iter()
                .map(|c| vec![Sql::Integer(n(&c["plot"]) as i64); 2])
                .collect(),
        },
        HistoricalBatch {
            sql: "INSERT INTO CityAttributes VALUES (?, 'Population', 'Initial', '2')",
            rows: ordered
                .iter()
                .map(|c| vec![Sql::Integer(n(&c["plot"]) as i64)])
                .collect(),
        },
        HistoricalBatch {
            sql: "INSERT INTO Buildings VALUES ('BUILDING_PALACE', ?)",
            rows: ordered
                .iter()
                .filter(|c| c["capital"] == true)
                .map(|c| vec![Sql::Integer(n(&c["plot"]) as i64)])
                .collect(),
        },
    ];
    let order: BTreeMap<_, _> = ordered
        .iter()
        .enumerate()
        .map(|(i, c)| (n(&c["plot"]), i as i32))
        .collect();
    let mut allocations = allocations.to_vec();
    allocations.sort_by_key(|a| order.get(&a.0).map(|&i| (0, i)).unwrap_or((1, a.0)));
    batches.push(HistoricalBatch {
        sql: "INSERT INTO PlotOwners VALUES (?, ?, ?, ?)",
        rows: allocations
            .iter()
            .map(|&(p, o, c, w)| {
                vec![
                    Sql::Integer(p as i64),
                    Sql::Integer(o as i64),
                    Sql::Integer(c as i64),
                    Sql::Integer(w as i64),
                ]
            })
            .collect(),
    });
    batches
}
pub fn insert_historical_state(
    db: &Connection,
    cities: &[Value],
    allocations: &[Allocation],
) -> Result<()> {
    for batch in historical_state_batches(cities, allocations) {
        for row in batch.rows {
            db.execute(batch.sql, rusqlite::params_from_iter(row))?;
        }
    }
    Ok(())
}
pub fn resource_draft() -> Result<Value> {
    read_json(root().join("documents/spec/data/japan-map-resources.json"))
}
pub fn lake_tile_count(lake: &Value, km: i32) -> i32 {
    let count = round(
        f(&lake["area_km2"]) / (3f64.sqrt() / 2. * (km * km) as f64),
        0,
    )
    .max(1.) as i32;
    let bounds: Vec<i32> = s(&lake["tile_hint"])
        .split('-')
        .filter_map(|v| v.parse().ok())
        .collect();
    count.clamp(bounds[0], *bounds.last().unwrap())
}
pub fn place_lakes(
    grid: HexGrid,
    land: &mut Tiles,
    cities: &[Value],
    lakes: &[Value],
) -> Result<(Tiles, Vec<Value>)> {
    let reserved: Tiles = cities.iter().map(|c| n(&c["plot"])).collect();
    let mut selected = Tiles::new();
    let mut results = vec![];
    for lake in lakes {
        let target = project(f(&lake["center"][0]), f(&lake["center"][1]));
        let count = lake_tile_count(lake, grid.km);
        let mut ranked: Vec<_> = (0..grid.width * grid.height)
            .filter(|p| !reserved.contains(p) && !selected.contains(p))
            .map(|p| (distance(grid.center(p), target), p))
            .collect();
        ranked.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ensure!(ranked.len() >= count as usize, "Could not rasterize lake");
        let tiles: Vec<_> = ranked.iter().take(count as usize).map(|v| v.1).collect();
        for p in &tiles {
            land.remove(p);
            selected.insert(*p);
        }
        let ja = match s(&lake["id"]) {
            "lake-biwa" => "琵琶湖",
            "lake-inawashiro" => "猪苗代湖",
            "lake-kasumigaura" => "霞ヶ浦",
            "lake-towada" => "十和田湖",
            "lake-shinji" => "宍道湖",
            "lake-suwa" => "諏訪湖",
            s => s,
        };
        results.push(json!({"id":lake["id"],"name_ja":ja,"province":lake["province"],"tile_ids":tiles,"tile_count":count,"area_km2":lake["area_km2"],"tile_hint":lake["tile_hint"],"source_ids":lake["source_ids"],"representation":"TERRAIN_COAST"}));
    }
    Ok((selected, results))
}
pub fn side_neighbour(grid: HexGrid, plot: i32, side: &str) -> Option<i32> {
    let (c, r) = grid.xy(plot);
    let (dx, dy) = match side {
        "NE" => (r & 1, -1),
        "W" => (-1, 0),
        "NW" => ((r & 1) - 1, -1),
        _ => return None,
    };
    let (c, r) = (c + dx, r + dy);
    (c >= 0 && r >= 0 && c < grid.width && r < grid.height).then_some(grid.index(c, r))
}
pub fn river_edge_geometry(
    grid: HexGrid,
    edge: &Edge,
    neighbor: Option<i32>,
) -> Result<[Point; 2]> {
    let other = neighbor
        .or_else(|| side_neighbour(grid, edge.0, &edge.1))
        .context("River edge has no adjacent plot")?;
    let a = grid.center(edge.0);
    let b = grid.center(other);
    let len = distance(a, b);
    ensure!(len > 0., "coincident centers");
    let h = grid.km as f64 / (2. * 3f64.sqrt());
    let (t, m) = (
        (-(b.1 - a.1) / len, (b.0 - a.0) / len),
        ((a.0 + b.0) / 2., (a.1 + b.1) / 2.),
    );
    Ok([
        (round(m.0 - t.0 * h, 5), round(m.1 - t.1 * h, 5)),
        (round(m.0 + t.0 * h, 5), round(m.1 + t.1 * h, 5)),
    ])
}
#[derive(Clone, Debug)]
pub struct RiverEdge {
    pub plot: i32,
    pub side: String,
    pub neighbor: i32,
    pub vertices: [Point; 2],
    pub midpoint: Point,
}
pub fn land_river_edges(grid: HexGrid, land: &Tiles) -> Result<BTreeMap<Edge, RiverEdge>> {
    let mut out = BTreeMap::new();
    for &p in land {
        for side in ["NE", "W", "NW"] {
            if let Some(other) = side_neighbour(grid, p, side) {
                if land.contains(&other) {
                    let key = (p, side.to_owned());
                    let v = river_edge_geometry(grid, &key, Some(other))?;
                    out.insert(
                        key,
                        RiverEdge {
                            plot: p,
                            side: side.to_owned(),
                            neighbor: other,
                            vertices: v,
                            midpoint: ((v[0].0 + v[1].0) / 2., (v[0].1 + v[1].1) / 2.),
                        },
                    );
                }
            }
        }
    }
    Ok(out)
}
pub fn point_segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx * dx + dy * dy;
    let t = if len == 0. {
        0.
    } else {
        ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len
    }
    .clamp(0., 1.);
    distance(p, (a.0 + t * dx, a.1 + t * dy))
}
pub fn river_edge_graph(edges: &BTreeMap<Edge, RiverEdge>) -> BTreeMap<Edge, BTreeSet<Edge>> {
    let mut at: BTreeMap<(i64, i64), Vec<Edge>> = BTreeMap::new();
    let mut graph = edges
        .keys()
        .map(|e| (e.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (e, r) in edges {
        for v in r.vertices {
            at.entry(((v.0 * 1e5).round() as i64, (v.1 * 1e5).round() as i64))
                .or_default()
                .push(e.clone());
        }
    }
    for list in at.values() {
        for e in list {
            graph
                .get_mut(e)
                .unwrap()
                .extend(list.iter().filter(|v| *v != e).cloned());
        }
    }
    graph
}
#[derive(PartialEq)]
struct Queue(f64, Edge);
impl Eq for Queue {}
impl Ord for Queue {
    fn cmp(&self, b: &Self) -> Ordering {
        b.0.total_cmp(&self.0).then_with(|| b.1.cmp(&self.1))
    }
}
impl PartialOrd for Queue {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}
pub fn route_river_edges(
    edges: &BTreeMap<Edge, RiverEdge>,
    graph: &BTreeMap<Edge, BTreeSet<Edge>>,
    start: &Edge,
    end: &Edge,
    segment: (Point, Point),
    km: i32,
) -> Result<Vec<Edge>> {
    if start == end {
        return Ok(vec![start.clone()]);
    }
    let mut distances = BTreeMap::from([(start.clone(), 0.)]);
    let mut previous = BTreeMap::new();
    let mut q = BinaryHeap::from([Queue(0., start.clone())]);
    while let Some(Queue(d, e)) = q.pop() {
        if distances.get(&e) != Some(&d) {
            continue;
        }
        if &e == end {
            break;
        }
        for nxt in &graph[&e] {
            let deviation = point_segment_distance(edges[nxt].midpoint, segment.0, segment.1);
            let candidate = d + 1. + (deviation / (km as f64 * 0.45).max(1.)).powi(2);
            if candidate < *distances.get(nxt).unwrap_or(&f64::INFINITY) {
                distances.insert(nxt.clone(), candidate);
                previous.insert(nxt.clone(), e.clone());
                q.push(Queue(candidate, nxt.clone()));
            }
        }
    }
    ensure!(
        distances.contains_key(end),
        "River segment cannot be connected on land: {start:?} to {end:?}"
    );
    let mut path = vec![end.clone()];
    while path.last().unwrap() != start {
        path.push(previous[path.last().unwrap()].clone());
    }
    path.reverse();
    Ok(path)
}
pub fn rasterize_rivers(
    grid: HexGrid,
    land: &Tiles,
    rivers: &[Value],
) -> Result<(BTreeMap<Edge, RiverEdge>, Vec<Value>)> {
    let edges = land_river_edges(grid, land)?;
    let graph = river_edge_graph(&edges);
    ensure!(
        !edges.is_empty(),
        "No shared land edges available for rivers"
    );
    let mut results = vec![];
    for river in rivers {
        let points: Vec<_> = arr(&river["coordinates"])
            .iter()
            .map(|c| project(f(&c[0]), f(&c[1])))
            .collect();
        let snapped: Vec<_> = points
            .iter()
            .map(|&p| {
                edges
                    .iter()
                    .min_by(|(ak, a), (bk, b)| {
                        (distance(a.midpoint, p), ak)
                            .partial_cmp(&(distance(b.midpoint, p), bk))
                            .unwrap()
                    })
                    .unwrap()
                    .0
                    .clone()
            })
            .collect();
        let mut path = vec![];
        for i in 0..points.len().saturating_sub(1) {
            for e in route_river_edges(
                &edges,
                &graph,
                &snapped[i],
                &snapped[i + 1],
                (points[i], points[i + 1]),
                grid.km,
            )? {
                if path.last() != Some(&e) {
                    path.push(e);
                }
            }
        }
        ensure!(!path.is_empty(), "River has no map edges");
        let edge_path: Vec<_> = path
            .iter()
            .map(|e| json!({"plot":e.0,"side":e.1,"neighbor":edges[e].neighbor}))
            .collect();
        results.push(json!({"id":river["id"],"name_ja":river["name_ja"],"source_ids":river["source_ids"],"confidence":river["confidence"],"historical_note":river["historical_note"],"edge_count":path.len(),"edge_path":edge_path}));
    }
    Ok((edges, results))
}
pub fn place_volcanoes(
    grid: HexGrid,
    land: &Tiles,
    cities: &[Value],
    lakes: &Tiles,
    volcanoes: &[Value],
) -> Result<Vec<Value>> {
    let mut used = lakes.clone();
    used.extend(cities.iter().map(|c| n(&c["plot"])));
    let mut out = vec![];
    for v in volcanoes {
        let target = project(f(&v["coordinates"][0]), f(&v["coordinates"][1]));
        let (d, p) = land
            .difference(&used)
            .map(|&p| (distance(grid.center(p), target), p))
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .context("volcano plots unavailable")?;
        used.insert(p);
        out.push(json!({"id":v["id"],"site":v["site"],"plot":p,"feature_id":v["feature_id"],"province":v["province"],"coordinates":v["coordinates"],"displacement_km":round(d,1),"eruption_years_in_scope":v["eruption_years_in_scope"],"source_ids":v["source_ids"]}));
    }
    Ok(out)
}
pub fn resource_is_deferred(p: &Value) -> bool {
    p["available_at_start"] == false
        || p["start_year"]
            .as_i64()
            .is_some_and(|y| y > START_YEAR as i64)
}
pub fn reveal_year(p: &Value) -> Option<i32> {
    p["start_year"]
        .as_i64()
        .or_else(|| p["start_year_range"][0].as_i64())
        .map(|y| y as i32)
}
pub fn reveal_tech(id: &str) -> Option<&'static str> {
    match id {
        "silver-ikuno" | "silver-sado-tsurushi" => Some("TECH_JAC_SENGOKU_CUPELLATION"),
        "copper-ashio" => Some("TECH_JAC_SENGOKU_ORE_PROSPECTING"),
        "gold-izu-toi"
        | "gold-noto-hodatsu"
        | "gold-sado-nishimikawa"
        | "gold-sado-aikawa"
        | "silver-nobesawa" => Some("TECH_JAC_SENGOKU_MINE_MANAGEMENT"),
        _ => None,
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResourceRule {
    pub terrains: BTreeSet<String>,
    pub features: BTreeSet<String>,
    pub attributes: BTreeMap<String, String>,
}
pub fn resource_rules_from_xml(paths: &[PathBuf]) -> Result<BTreeMap<String, ResourceRule>> {
    let mut rules: BTreeMap<String, ResourceRule> = BTreeMap::new();
    for path in paths {
        let root = xml(path)?;
        for row in rows(&root, "Resources") {
            let kind = attr(row, "ResourceType");
            if !kind.is_empty() {
                rules
                    .entry(kind.to_owned())
                    .or_default()
                    .attributes
                    .extend(row.attributes.iter().map(|(a, b)| (a.clone(), b.clone())));
            }
        }
        for (table, key, features) in [
            ("Resource_ValidTerrains", "TerrainType", false),
            ("Resource_ValidFeatures", "FeatureType", true),
        ] {
            if let Some(table) = root.get_child(table) {
                for row in elements(table) {
                    let rule = rules
                        .entry(attr(row, "ResourceType").to_owned())
                        .or_default();
                    let set = if features {
                        &mut rule.features
                    } else {
                        &mut rule.terrains
                    };
                    if row.name == "Row" {
                        set.insert(attr(row, key).to_owned());
                    } else if row.name == "Delete" {
                        set.remove(attr(row, key));
                    }
                }
            }
        }
    }
    Ok(rules)
}
pub fn official_resource_rules(source: &Path) -> Result<BTreeMap<String, ResourceRule>> {
    let assets = source.ancestors().nth(3).context("official map layout")?;
    let game = source.ancestors().nth(5).context("official game layout")?;
    resource_rules_from_xml(&[
        assets.join("Gameplay/Data/Resources.xml"),
        game.join("DLC/Expansion2/Data/Expansion1_Resources.xml"),
        game.join("DLC/Expansion2/Data/Expansion2_Resources.xml"),
        game.join("DLC/Expansion2/Data/Expansion2_RemoveData.xml"),
        mod_dir().join("Data/JAC_Japan_Resources.xml"),
    ])
}
pub fn resource_allowed(
    rule: &ResourceRule,
    terrain: &str,
    feature: Option<&str>,
    lake: bool,
    adjacent_land: bool,
    river: bool,
) -> bool {
    if feature.is_some_and(|f| !rule.features.contains(f))
        || feature.is_none() && !rule.terrains.contains(terrain)
    {
        return false;
    }
    let a = &rule.attributes;
    !(lake && a.get("LakeEligible").is_some_and(|s| s == "false")
        || !adjacent_land && a.get("AdjacentToLand").is_some_and(|s| s == "true")
        || !river && a.get("RequiresRiver").is_some_and(|s| s == "true")
        || river && a.get("NoRiver").is_some_and(|s| s == "true"))
}
pub type ResourcePlacement = (
    Vec<(i32, String, i32)>,
    Vec<Value>,
    Value,
    BTreeMap<i32, String>,
);
#[allow(clippy::too_many_arguments)]
pub fn place_resources(
    grid: HexGrid,
    land: &Tiles,
    coast: &Tiles,
    lakes: &Tiles,
    cities: &[Value],
    volcanoes: &[Value],
    groups: &[Value],
    mappings: &[Value],
    terrain: &mut BTreeMap<i32, String>,
    river_edges: &BTreeMap<Edge, RiverEdge>,
    rules: &BTreeMap<String, ResourceRule>,
) -> Result<ResourcePlacement> {
    let city: Tiles = cities.iter().map(|c| n(&c["plot"])).collect();
    let custom: Tiles = volcanoes.iter().map(|v| n(&v["plot"])).collect();
    let river: Tiles = river_edges
        .values()
        .flat_map(|e| [e.plot, e.neighbor])
        .collect();
    let mut eligible = vec![];
    let mut records = vec![];
    for group in groups {
        let mapping = mappings
            .iter()
            .find(|m| m["name_ja"] == group["resource_ja"]);
        for p in arr(&group["points"]) {
            let deferred = resource_is_deferred(p);
            eligible.push((group, p, deferred, records.len()));
            records.push(json!({"id":p["id"],"site":p["site"],"province":p["province"],"resource_ja":group["resource_ja"],"resource_id":group["civ6_id"],"official_resource_id":group.get("official_scenario_id").or_else(||mapping.and_then(|m|m.get("civ6_id"))).unwrap_or(&Value::Null),"availability":mapping.map(|m|m["availability"].clone()).unwrap_or(Value::Null),"coordinates":p["coordinates"],"start_year":p["start_year"],"start_year_range":p["start_year_range"],"available_at_start":p["available_at_start"],"start_status":if deferred{"deferred"}else{"start_candidate"},"placement_status":"not_placed","confidence":p["confidence"],"source_ids":p.get("source_ids").cloned().unwrap_or(json!([]))}));
        }
    }
    eligible.sort_by_key(|(g, p, d, _)| {
        (
            *d,
            match s(&p["confidence"]) {
                "high" => 0,
                "medium" => 1,
                "low" => 2,
                _ => 3,
            },
            n(&g["candidate_count"]),
            s(&p["id"]).to_owned(),
        )
    });
    let mut occupied: BTreeMap<i32, String> = BTreeMap::new();
    let mut reserved = Tiles::new();
    let mut rows = vec![];
    let mut features = BTreeMap::new();
    for (g, p, deferred, index) in &eligible {
        let target = project(f(&p["coordinates"][0]), f(&p["coordinates"][1]));
        let resource = s(&g["civ6_id"]);
        let rule = rules
            .get(resource)
            .with_context(|| format!("No resource rule for {resource}"))?;
        let domain = if g["placement_domain"] == "marine" {
            coast
        } else {
            land
        };
        let mut options = vec![];
        for &candidate in domain {
            if lakes.contains(&candidate)
                || g["placement_domain"] != "marine"
                    && (city.contains(&candidate) || custom.contains(&candidate))
            {
                continue;
            }
            let d = distance(grid.center(candidate), target);
            let limit = if *deferred {
                30f64.max(2. * grid.km as f64)
            } else {
                60f64.max(3. * grid.km as f64)
            };
            if d > limit {
                continue;
            }
            let t = terrain
                .get(&candidate)
                .map(String::as_str)
                .unwrap_or("TERRAIN_COAST");
            let near = land.contains(&candidate)
                || coast.contains(&candidate) && !lakes.contains(&candidate);
            let river = river.contains(&candidate);
            let (feature, replacement) =
                if resource_allowed(rule, t, None, lakes.contains(&candidate), near, river) {
                    (None, None)
                } else if land.contains(&candidate)
                    && [
                        "TERRAIN_GRASS",
                        "TERRAIN_GRASS_HILLS",
                        "TERRAIN_PLAINS",
                        "TERRAIN_PLAINS_HILLS",
                        "TERRAIN_TUNDRA",
                        "TERRAIN_TUNDRA_HILLS",
                    ]
                    .contains(&t)
                    && resource_allowed(rule, t, Some("FEATURE_FOREST"), false, true, river)
                {
                    (Some("FEATURE_FOREST"), None)
                } else if resource == "RESOURCE_SALT"
                    && land.contains(&candidate)
                    && t == "TERRAIN_GRASS"
                    && grid
                        .neighbor_ids(candidate)
                        .iter()
                        .any(|p| !land.contains(p))
                    && resource_allowed(rule, "TERRAIN_PLAINS", None, false, true, river)
                {
                    (None, Some("TERRAIN_PLAINS"))
                } else {
                    continue;
                };
            if *deferred
                && (feature.is_some()
                    || replacement.is_some()
                    || occupied.contains_key(&candidate)
                    || reserved.contains(&candidate))
            {
                continue;
            }
            options.push((d, candidate, feature, replacement));
        }
        let record = &mut records[*index];
        if let Some((d, plot, feature, replacement)) = options
            .into_iter()
            .min_by(|a, b| (a.0, a.1).partial_cmp(&(b.0, b.1)).unwrap())
        {
            record["plot"] = json!(plot);
            record["displacement_km"] = json!(round(d, 1));
            if *deferred {
                reserved.insert(plot);
                record["placement_status"] = json!("reserved_for_later_release");
                record["reveal_year"] = json!(reveal_year(p));
                if let Some(t) = reveal_tech(s(&p["id"])) {
                    record["reveal_tech"] = json!(t);
                }
                continue;
            }
            if let Some(winner) = occupied.get(&plot) {
                record["placement_status"] = json!("same_tile_conflict");
                record["conflict_with"] = json!(winner);
                continue;
            }
            occupied.insert(plot, s(&p["id"]).to_owned());
            record["placement_status"] = json!("placed");
            if let Some(t) = replacement {
                terrain.insert(plot, t.to_owned());
                record["terrain_adjustment"] = json!(t);
            }
            if let Some(f) = feature {
                features.insert(plot, f.to_owned());
                record["required_feature"] = json!(f);
            }
            rows.push((plot, resource.to_owned(), 1));
        } else {
            record["placement_status"] = json!("no_valid_official_plot");
            record["placement_reason"]=json!("No nearby free plot satisfies the resource rules (the tile budget of this scale is used up).");
        }
    }
    let stats = json!({"candidate_count":records.len(),"placed_count":rows.len(),"land_candidate_count":eligible.iter().filter(|(g,_,d,_)|!*d&&g["placement_domain"]!="marine").count(),"marine_candidate_count":eligible.iter().filter(|(g,_,d,_)|!*d&&g["placement_domain"]=="marine").count(),"reserved_count":reserved.len(),"count_by_type":counts(rows.iter().map(|r|r.1.as_str())),"candidate_status_counts":counts(records.iter().map(|r|if r["start_status"]=="start_candidate"{s(&r["placement_status"])}else{s(&r["start_status"])})),"custom_resource_ids":groups.iter().filter_map(|g|s(&g["civ6_id"]).starts_with("RESOURCE_JAC_").then_some(s(&g["civ6_id"]).to_owned())).collect::<BTreeSet<_>>()});
    Ok((rows, records, stats, features))
}
pub fn counts<'a>(iter: impl Iterator<Item = &'a str>) -> BTreeMap<String, i32> {
    let mut out = BTreeMap::new();
    for k in iter {
        *out.entry(k.to_owned()).or_default() += 1;
    }
    out
}
pub fn place_marshes(
    grid: HexGrid,
    land: &Tiles,
    terrain: &BTreeMap<i32, String>,
    cities: &[Value],
    occupied: &Tiles,
    rule: &Value,
) -> Result<(Tiles, Value)> {
    let city: Tiles = cities.iter().map(|c| n(&c["plot"])).collect();
    let eligible: Tiles = land
        .iter()
        .filter(|p| terrain[p] == "TERRAIN_GRASS" && !city.contains(p) && !occupied.contains(p))
        .copied()
        .collect();
    let target = (round(land.len() as f64 * 0.025, 0) as usize).clamp(22, 33);
    let clusters = arr(&rule["clusters"]);
    ensure!(!clusters.is_empty(), "marsh clusters absent");
    let centers: Vec<_> = clusters
        .iter()
        .map(|c| project(f(&c["center"][0]), f(&c["center"][1])))
        .collect();
    let mut selected = Tiles::new();
    for (i, center) in centers.iter().enumerate() {
        let quota = target / clusters.len() + usize::from(i < target % clusters.len());
        let mut nearby: Vec<_> = eligible
            .difference(&selected)
            .map(|&p| (distance(grid.center(p), *center), p))
            .filter(|v| v.0 <= (2.5 * grid.km as f64).max(40.))
            .collect();
        nearby.sort_by(|a, b| a.partial_cmp(b).unwrap());
        selected.extend(nearby.iter().take(quota).map(|v| v.1));
    }
    if selected.len() < target {
        let mut ranked: Vec<_> = eligible
            .difference(&selected)
            .map(|&p| {
                (
                    centers
                        .iter()
                        .map(|&c| distance(grid.center(p), c))
                        .fold(f64::INFINITY, f64::min),
                    p,
                )
            })
            .collect();
        ranked.sort_by(|a, b| a.partial_cmp(b).unwrap());
        selected.extend(ranked.iter().take(target - selected.len()).map(|v| v.1));
    }
    ensure!(
        selected.len() == target,
        "Could not place marsh target {target}"
    );
    Ok((
        selected,
        json!({"feature_id":rule["feature_id"],"target_count":target,"placed_count":target,"eligible_flat_land_tiles":eligible.len(),"cluster_ids":clusters.iter().map(|c|c["id"].clone()).collect::<Vec<_>>(),"clusters":clusters.iter().map(|c|json!({"id":c["id"],"site":c["site"],"source_ids":c.get("source_ids").cloned().unwrap_or(json!([]))})).collect::<Vec<_>>(),"source_ids":rule.get("source_ids").cloned().unwrap_or(json!([]))}),
    ))
}
pub const FOREST_BIAS_REGIONS: [(Point, f64, f64); 5] = [
    ((137.2, 36.), 170., 1.6),
    ((135.7, 34.1), 140., 1.6),
    ((132.8, 35.1), 160., 1.5),
    ((132.3, 33.), 210., 1.5),
    ((140.5, 41.2), 360., 1.4),
];
#[allow(clippy::too_many_arguments)]
pub fn place_forests(
    grid: HexGrid,
    land: &Tiles,
    terrain: &BTreeMap<i32, String>,
    cities: &[Value],
    occupied: &Tiles,
    rice: &[Value],
    edges: &BTreeMap<Edge, RiverEdge>,
    rule: &Value,
) -> Result<(Tiles, Value)> {
    use sha2::{Digest, Sha256};
    let city: Tiles = cities.iter().map(|c| n(&c["plot"])).collect();
    let rice: Vec<_> = rice
        .iter()
        .map(|p| project(f(&p["coordinates"][0]), f(&p["coordinates"][1])))
        .collect();
    let radius = 1.15 * grid.km as f64;
    let eligible: Tiles = land
        .iter()
        .filter(|p| {
            !city.contains(p)
                && !occupied.contains(p)
                && [
                    "TERRAIN_GRASS",
                    "TERRAIN_PLAINS",
                    "TERRAIN_TUNDRA",
                    "TERRAIN_GRASS_HILLS",
                    "TERRAIN_PLAINS_HILLS",
                    "TERRAIN_TUNDRA_HILLS",
                ]
                .contains(&terrain[p].as_str())
                && (terrain[p].ends_with("_HILLS")
                    || !rice
                        .iter()
                        .any(|&c| distance(grid.center(**p), c) <= radius))
        })
        .copied()
        .collect();
    let pct = f(&rule["target_share_of_eligible_tiles_percent"]);
    let target = round(eligible.len() as f64 * pct / 100., 0) as usize;
    let river: Tiles = edges.values().flat_map(|e| [e.plot, e.neighbor]).collect();
    let mut ranked = vec![];
    for &p in &eligible {
        let xy = grid.center(p);
        let region = FOREST_BIAS_REGIONS
            .iter()
            .filter(|&&(c, r, _)| distance(xy, project(c.0, c.1)) <= r)
            .map(|v| v.2)
            .fold(1., f64::max);
        let weight = if terrain[&p].ends_with("_HILLS") {
            1.6
        } else {
            0.8
        } * region
            * if river.contains(&p) { 0.65 } else { 1. };
        let digest = Sha256::digest(format!("JAC/JapanMapScale/forest/{}/{p}", grid.km).as_bytes());
        let uniform = (u64::from_be_bytes(digest[..8].try_into().unwrap()) as f64 + 1.)
            / (u64::MAX as f64 + 2.);
        ranked.push((-uniform.ln() / weight, p));
    }
    ranked.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let selected: Tiles = ranked.iter().take(target).map(|v| v.1).collect();
    Ok((
        selected.clone(),
        json!({"feature_id":rule["feature_id"],"target_percent":rule["target_share_of_eligible_tiles_percent"],"eligible_tiles":eligible.len(),"placed_count":selected.len(),"placed_percent":if eligible.is_empty(){0.}else{round(100.*selected.len() as f64/eligible.len() as f64,2)},"rice_basin_exclusion_radius_km":round(radius,1),"source_ids":rule.get("source_ids").cloned().unwrap_or(json!([]))}),
    ))
}
pub fn custom_content_inventory(draft: &Value, records: &[Value]) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    for r in records
        .iter()
        .filter(|r| s(&r["resource_id"]).starts_with("RESOURCE_JAC_"))
    {
        let index=out.iter().position(|c|c["type"]==r["resource_id"]).unwrap_or_else(||{out.push(json!({"type":r["resource_id"],"kind":"resource","resource_ja":r["resource_ja"],"candidate_ids":[],"status":"defined_in_Data/JAC_Japan_Resources.xml"}));out.len()-1});
        out[index]["candidate_ids"]
            .as_array_mut()
            .unwrap()
            .push(r["id"].clone());
    }
    out.extend(arr(&draft["terrain"]["natural_wonder_candidates"]).iter().map(|w|json!({"type":w["proposed_custom_feature_id"],"kind":"feature","name_ja":w["name_ja"],"candidate_ids":[w["id"]],"status":"not_selected_or_implemented"})));
    out
}
pub fn trade_access_status(item: &Value) -> &'static str {
    if item["unlock_civic"].as_str().is_some_and(|s| !s.is_empty()) {
        "unlocked_by_research"
    } else if n(&item["available_from_year"]) > START_YEAR {
        "deferred_until_year"
    } else {
        "undated_trade_access_not_placed"
    }
}
pub fn deferred_trade_access(draft: &Value) -> Vec<Value> {
    arr(&draft["trade_access"])
        .iter()
        .map(|i| {
            let mut i = i.clone();
            i["start_status"] = json!(trade_access_status(&i));
            i
        })
        .collect()
}
#[allow(clippy::too_many_arguments)]
pub fn generate_map(
    km: i32,
    land: &[Ring],
    japan: &[Ring],
    dem: &Dem,
    official: &Path,
    destination: &Path,
    settlers: bool,
) -> Result<Value> {
    let rules = official_resource_rules(official)?;
    let schema=Connection::open_with_flags(official,OpenFlags::SQLITE_OPEN_READ_ONLY)?.prepare("SELECT sql FROM sqlite_master WHERE type IN ('table','index') AND sql IS NOT NULL ORDER BY type DESC,name")?.query_map([],|r|r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
    generate_map_with_rules(km, land, japan, dem, &schema, &rules, destination, settlers)
}
#[allow(clippy::too_many_arguments)]
pub fn generate_map_with_rules(
    km: i32,
    land_shapes: &[Ring],
    japan: &[Ring],
    dem: &Dem,
    schema: &[String],
    rules: &BTreeMap<String, ResourceRule>,
    destination: &Path,
    settlers: bool,
) -> Result<Value> {
    let grid = HexGrid::for_scale(km, japan)?;
    let mut land = Tiles::new();
    let mut heights = BTreeMap::new();
    for p in 0..grid.width * grid.height {
        let xy = grid.center(p);
        let (lon, lat) = unproject(xy.0, xy.1);
        if in_scope(lon, lat) && contains(japan, lon, lat) && contains(land_shapes, lon, lat) {
            land.insert(p);
            heights.insert(p, elevation(dem, lon, lat));
        }
    }
    let anchors = SEATS
        .iter()
        .map(|s| (s.castle.as_str(), s.lon, s.lat))
        .chain(ISLAND_ANCHORS);
    for (name, lon, lat) in anchors {
        ensure!(
            contains(japan, lon, lat) && contains(land_shapes, lon, lat),
            "Anchor is outside Japanese land: {name}"
        );
        let (c, r) = grid.closest(lon, lat);
        let p = grid.index(c, r);
        land.insert(p);
        heights.insert(
            p,
            heights
                .get(&p)
                .copied()
                .unwrap_or_else(|| elevation(dem, lon, lat))
                .min(1199.),
        );
    }
    let mut passable: Tiles = land
        .iter()
        .filter(|p| heights[p] < 1200.)
        .copied()
        .collect();
    let starts = choose_starts(grid, &passable)?;
    let mut cities = choose_cities(
        grid,
        &passable,
        &starts,
        arr(&start_1534::data()?["castles"]),
    )?;
    let draft = resource_draft()?;
    let original = land.len();
    let (lakes, lake_records) =
        place_lakes(grid, &mut land, &cities, arr(&draft["terrain"]["lakes"]))?;
    passable = passable.intersection(&land).copied().collect();
    nudge_adjacent_cities(grid, &land, &passable, &mut cities)?;
    let mut terrain: BTreeMap<i32, String> = land
        .iter()
        .map(|&p| {
            (
                p,
                if heights[&p] >= 1200. {
                    "TERRAIN_GRASS_MOUNTAIN"
                } else if heights[&p] >= 400. {
                    "TERRAIN_GRASS_HILLS"
                } else {
                    "TERRAIN_GRASS"
                }
                .to_owned(),
            )
        })
        .collect();
    let mut volcanoes = place_volcanoes(
        grid,
        &land,
        &cities,
        &lakes,
        arr(&draft["terrain"]["volcanoes"]),
    )?;
    for v in &mut volcanoes {
        let p = n(&v["plot"]);
        v["terrain_promoted_to_mountain"] = json!(!terrain[&p].ends_with("_MOUNTAIN"));
        terrain.insert(p, "TERRAIN_GRASS_MOUNTAIN".into());
    }
    let (edges, rivers) = rasterize_rivers(grid, &land, arr(&draft["terrain"]["rivers"]))?;
    let mut coast = lakes.clone();
    for &p in &land {
        coast.extend(
            grid.neighbor_ids(p)
                .into_iter()
                .filter(|p| !land.contains(p)),
        );
    }
    let (resource_rows, records, resource_stats, required) = place_resources(
        grid,
        &land,
        &coast,
        &lakes,
        &cities,
        &volcanoes,
        arr(&draft["resource_groups"]),
        arr(&draft["resource_mapping"]),
        &mut terrain,
        &edges,
        rules,
    )?;
    let mut occupied: Tiles = resource_rows
        .iter()
        .map(|r| r.0)
        .chain(
            records
                .iter()
                .filter(|r| r["placement_status"] == "reserved_for_later_release")
                .map(|r| n(&r["plot"])),
        )
        .chain(cities.iter().map(|c| n(&c["plot"])))
        .chain(volcanoes.iter().map(|v| n(&v["plot"])))
        .collect();
    let (marsh, marsh_stats) = place_marshes(
        grid,
        &land,
        &terrain,
        &cities,
        &occupied,
        &draft["terrain"]["marsh"],
    )?;
    occupied.extend(&marsh);
    let rice: Vec<_> = arr(&draft["resource_groups"])
        .iter()
        .filter(|g| g["resource_ja"] == "米")
        .flat_map(|g| arr(&g["points"]).iter().cloned())
        .collect();
    let (forest, forest_stats) = place_forests(
        grid,
        &land,
        &terrain,
        &cities,
        &occupied,
        &rice,
        &edges,
        &draft["terrain"]["forest"],
    )?;
    let mut features: Vec<_> = volcanoes
        .iter()
        .map(|v| (n(&v["plot"]), "FEATURE_VOLCANO".to_owned()))
        .collect();
    features.extend(required);
    features.extend(marsh.iter().map(|&p| (p, "FEATURE_MARSH".into())));
    features.extend(
        forest
            .iter()
            .map(|&p| (p, s(&draft["terrain"]["forest"]["feature_id"]).to_owned())),
    );
    let feature_counts = counts(features.iter().map(|v| v.1.as_str()));
    let mut flags: BTreeMap<i32, BTreeSet<String>> = BTreeMap::new();
    for r in &rivers {
        for e in arr(&r["edge_path"]) {
            flags
                .entry(n(&e["plot"]))
                .or_default()
                .insert(s(&e["side"]).into());
        }
    }
    let db = create_schema_statements(schema, destination)?;
    db.execute_batch("BEGIN IMMEDIATE")?;
    db.execute(
        "INSERT INTO Map VALUES ('Default',?,?,47,23,0,0,'MAPSIZE_HUGE')",
        [grid.width, grid.height],
    )?;
    write_map_metadata(&db, destination, &format!("JAC/JapanMapScale/{km}"))?;
    for p in 0..grid.width * grid.height {
        let (t, c, im) = if land.contains(&p) {
            (
                &terrain[&p][..],
                "CONTINENT_ASIA",
                i32::from(heights[&p] >= 1200. || terrain[&p].ends_with("_MOUNTAIN")),
            )
        } else {
            (
                if coast.contains(&p) {
                    "TERRAIN_COAST"
                } else {
                    "TERRAIN_OCEAN"
                },
                "",
                0,
            )
        };
        db.execute("INSERT INTO Plots VALUES (?,?,?,?)", params![p, t, c, im])?;
    }
    for (p, t, c) in resource_rows {
        db.execute("INSERT INTO PlotResources VALUES (?,?,?)", params![p, t, c])?;
    }
    for (p, f) in features {
        db.execute("INSERT INTO PlotFeatures VALUES (?,?)", params![p, f])?;
    }
    for (p, sides) in &flags {
        db.execute(
            "INSERT INTO PlotRivers VALUES (?,?,?,?,-1,-1,-1)",
            [
                *p,
                i32::from(sides.contains("NE")),
                i32::from(sides.contains("W")),
                i32::from(sides.contains("NW")),
            ],
        )?;
    }
    for s in &starts {
        db.execute(
            "INSERT INTO StartPositions VALUES (?,'LEADER',?)",
            params![n(&s["plot"]), crate::s(&s["leader"])],
        )?;
    }
    if !settlers {
        insert_players(&db, SEATS.len())?;
        insert_historical_state(&db, &cities, &assign_land(grid, &land, &cities)?)?;
        prosper_start::apply(&db, grid, &cities, None)?;
    }
    ensure!(
        db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))? == "ok",
        "Invalid SQLite map"
    );
    db.execute_batch("COMMIT; VACUUM;")?;
    drop(db);
    let wonders: Vec<_> = arr(&draft["terrain"]["natural_wonder_candidates"])
        .iter()
        .map(|w| {
            let mut w = w.clone();
            w["status"] = json!("not_placed_custom_feature_candidate");
            w
        })
        .collect();
    Ok(
        json!({"km_per_tile":km,"width":grid.width,"height":grid.height,"grid_origin_projected_km":[round(grid.xmin,6),round(grid.ymin,6)],"land_tiles":land.len(),"sea_tiles":grid.width*grid.height-land.len() as i32,"rasterized_land_tiles_before_lakes":original,"starts":starts,"cities":cities,"close_city_pairs":close_city_pairs(&cities),"close_start_pairs":close_start_pairs(&starts),"resource_placement":resource_stats,"resource_candidates":records,"deferred_resources":records.iter().filter(|r|r["start_status"]=="deferred"||r["placement_status"]=="no_valid_official_plot").collect::<Vec<_>>(),"trade_access":deferred_trade_access(&draft),"custom_content_required":custom_content_inventory(&draft,&records),"rivers":rivers,"river_edge_count":rivers.iter().map(|r|n(&r["edge_count"])).sum::<i32>(),"river_edge_unique_count":flags.values().map(BTreeSet::len).sum::<usize>(),"lake_records":lake_records,"terrain_features":{"feature_counts":feature_counts,"forest":forest_stats,"marsh":marsh_stats,"volcanoes":volcanoes,"lakes":lake_records,"natural_wonder_candidates":wonders,"lake_schema_representation":"WorldBuilderMap has no lake table or FEATURE_LAKE; lake plots use TERRAIN_COAST"},"start_mode":if settlers{"settlers"}else{"historical"},"file":destination.file_name().unwrap().to_string_lossy(),"bytes":fs::metadata(destination)?.len()}),
    )
}
pub fn insert_players(db: &Connection, count: usize) -> Result<()> {
    for (i, seat) in SEATS.iter().take(count).enumerate() {
        db.execute("INSERT INTO Players VALUES (?,?,?,'CIVILIZATION_LEVEL_FULL_CIV','','Human','DIFFICULTY_PRINCE','INVALID','RANDOM',1)",params![i as i32,seat.civ,seat.leader])?;
    }
    Ok(())
}
pub fn write_map_metadata(db: &Connection, destination: &Path, id: &str) -> Result<()> {
    for (k, v) in [
        ("MapScript", "Continents.lua"),
        ("Ruleset", "RULESET_EXPANSION_2"),
    ] {
        db.execute("INSERT INTO MapAttributes VALUES (?,?)", [k, v])?;
    }
    let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, id.as_bytes())
        .to_string()
        .to_uppercase();
    let name = destination
        .file_name()
        .context("map filename")?
        .to_string_lossy();
    for (k, v) in [
        ("MetaDataVersion", "1"),
        ("AppVersion", "1.0.12.68"),
        ("DisplayName", &name),
        ("ID", &id),
        ("IsMod", "1"),
        ("IsBasic", "1"),
        ("LastUpdated", "0"),
    ] {
        db.execute("INSERT INTO MetaData VALUES (?,?)", [k, v])?;
    }
    Ok(())
}
pub const STATIC_LUA:&str="-- Static map: starting state is stored in the Civ6Map file.\nfunction InitializeNewGame() end\nLuaEvents.NewGameInitialized.Add(InitializeNewGame);\n";
pub fn deferred_sites(manifest: &[Value]) -> Value {
    let mut out = serde_json::Map::new();
    for item in manifest {
        let mut sites:Vec<_>=arr(&item["resource_candidates"]).iter().filter(|r|r["placement_status"]=="reserved_for_later_release").map(|r|json!({"id":r["id"],"resource":r["resource_id"],"plot":r["plot"],"year":r["reveal_year"],"tech":r["reveal_tech"]})).collect();
        sites.sort_by_key(|r| {
            (
                if n(&r["year"]) == 0 {
                    9999
                } else {
                    n(&r["year"])
                },
                s(&r["id"]).to_owned(),
            )
        });
        out.insert(
            format!("{}x{}", n(&item["width"]), n(&item["height"])),
            json!(sites),
        );
    }
    Value::Object(out)
}
pub fn write_resource_sites(manifest: &[Value], target: &Path) -> Result<()> {
    let mut out=String::from("-- Generated by tools/japan_map_scale/generate.py (SSOT 3.2). Do not edit.\nExposedMembers.JAC_ResourceSiteData = {\n");
    for (size, sites) in deferred_sites(manifest).as_object().unwrap() {
        out += &format!("\t[\"{size}\"] = {{\n");
        for site in arr(sites) {
            out += &format!(
                "\t\t{{ id=\"{}\", resource=\"{}\", plot={}",
                s(&site["id"]),
                s(&site["resource"]),
                n(&site["plot"])
            );
            if !site["year"].is_null() {
                out += &format!(", year={}", n(&site["year"]));
            }
            if !s(&site["tech"]).is_empty() {
                out += &format!(", tech=\"{}\"", s(&site["tech"]));
            }
            out += " },\n";
        }
        out += "\t},\n";
    }
    out += "}\n";
    write(target, out)
}
