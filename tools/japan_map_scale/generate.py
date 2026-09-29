"""Build the 1534 Japan maps from pinned public-domain source data.

Only the Python standard library is needed.  The official Civ6Map supplies the
SQLite schema; its map content is never copied into the generated files.
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
from hashlib import sha256
import json
import math
from pathlib import Path
import sqlite3
import struct
import urllib.request
import uuid
import zipfile

from start_1534 import civ_id, data, leader_id


ROOT = Path(__file__).resolve().parents[2]
MOD = ROOT / "poc" / "JAC_PoC_JapanMapScale"
CACHE = Path(__file__).resolve().parent / "cache"
OFFICIAL = (Path("C:/Program Files (x86)/Steam/steamapps/common/")
            / "Sid Meier's Civilization VI/Base/Assets/Maps/EarthMaps/WorldMapHuge.Civ6Map")
SCALES = (20, 15)
SOURCE = {
    "ne_10m_land.zip": (
        "https://naturalearth.s3.amazonaws.com/10m_physical/ne_10m_land.zip",
        "e547d749445eaa0964aba76738090ec88f5e63c4585122170f98c67a7ea922dc"),
    "ne_10m_admin_0_countries.zip": (
        "https://naturalearth.s3.amazonaws.com/10m_cultural/ne_10m_admin_0_countries.zip",
        "ce1ac7036499a0edd641fbc093cd209a98f96a49d2eca8480aaacad35138a7f6"),
    "etopo_2022_japan_3arcmin.csv": (
        "https://oceanwatch.pifsc.noaa.gov/erddap/griddap/ETOPO_2022_v1_60s.csv?"
        "z%5B(23):3:(47)%5D%5B(121):3:(155)%5D",
        "1c04df76876d71b5424961f90771922641f8e85bc7fec5de70f688d59b60082d"),
}

# Representative seats (SSOT 3.1), WGS84. The draft's historical castle owner
# can differ from the seat owner (notably Imagawa's Nagoya and Oda's seat).
@dataclass(frozen=True)
class Seat:
    slug: str
    ja: str
    en: str
    ru: str
    castle: str
    lon: float
    lat: float
    leader: str
    civ: str


SEATS = (
    Seat("ODA", "織田", "Oda", "Ода", "Nagoya", 136.900, 35.184, "LEADER_JAC_ODA_NOBUHIDE", "CIVILIZATION_JAC_ODA"),
    Seat("DATE", "伊達", "Date", "Датэ", "Yonezawa", 140.106, 37.914, leader_id("DATE"), civ_id("DATE")),
    Seat("ASHINA", "蘆名", "Ashina", "Асина", "Kurokawa", 139.930, 37.494, leader_id("ASHINA"), civ_id("ASHINA")),
    Seat("HOJO", "北条", "Hojo", "Ходзё", "Odawara", 139.157, 35.252, leader_id("HOJO"), civ_id("HOJO")),
    Seat("YAMANOUCHI_UESUGI", "山内上杉", "Yamanouchi Uesugi", "Яманоути Уэсуги", "Hirai", 139.063, 36.202, leader_id("YAMANOUCHI_UESUGI"), civ_id("YAMANOUCHI_UESUGI")),
    Seat("TAKEDA", "武田", "Takeda", "Такэда", "Tsutsujigasaki", 138.568, 35.677, "LEADER_JAC_TAKEDA_NOBUTORA", "CIVILIZATION_JAC_TAKEDA"),
    Seat("NAGAO", "長尾", "Nagao", "Нагао", "Kasugayama", 138.206, 37.148, leader_id("NAGAO"), civ_id("NAGAO")),
    Seat("IMAGAWA", "今川", "Imagawa", "Имагава", "Sunpu", 138.383, 34.979, leader_id("IMAGAWA"), civ_id("IMAGAWA")),
    Seat("MATSUDAIRA", "松平", "Matsudaira", "Мацудайра", "Okazaki", 137.172, 34.955, leader_id("MATSUDAIRA"), civ_id("MATSUDAIRA")),
    Seat("ASAKURA", "朝倉", "Asakura", "Асакура", "Ichijodani", 136.290, 35.999, leader_id("ASAKURA"), civ_id("ASAKURA")),
    Seat("ASHIKAGA", "足利将軍家", "Ashikaga Shogunate", "Сёгунат Асикага", "Muromachi", 135.760, 35.029, leader_id("ASHIKAGA"), civ_id("ASHIKAGA")),
    Seat("MIYOSHI", "三好", "Miyoshi", "Миёси", "Shozui", 134.591, 34.123, leader_id("MIYOSHI"), civ_id("MIYOSHI")),
    Seat("ROKKAKU", "六角", "Rokkaku", "Роккаку", "Kannonji", 136.168, 35.133, leader_id("ROKKAKU"), civ_id("ROKKAKU")),
    Seat("HONGANJI", "本願寺", "Honganji", "Хонгандзи", "Ishiyama", 135.526, 34.689, leader_id("HONGANJI"), civ_id("HONGANJI")),
    Seat("OUCHI", "大内", "Ouchi", "Оути", "Yamaguchi", 131.473, 34.178, leader_id("OUCHI"), civ_id("OUCHI")),
    Seat("AMAGO", "尼子", "Amago", "Амаго", "Gassan Toda", 133.242, 35.418, leader_id("AMAGO"), civ_id("AMAGO")),
    Seat("MORI", "毛利", "Mori", "Мори", "Yoshida Koriyama", 132.708, 34.687, leader_id("MORI"), civ_id("MORI")),
    Seat("CHOSOKABE", "長宗我部", "Chosokabe", "Тёсокабэ", "Oko", 133.642, 33.593, leader_id("CHOSOKABE"), civ_id("CHOSOKABE")),
    Seat("OTOMO", "大友", "Otomo", "Отомо", "Funai", 131.609, 33.241, leader_id("OTOMO"), civ_id("OTOMO")),
    Seat("SHIMAZU", "島津", "Shimazu", "Симадзу", "Kagoshima", 130.559, 31.596, leader_id("SHIMAZU"), civ_id("SHIMAZU")),
    Seat("RYUKYU", "琉球王国", "Ryukyu Kingdom", "Королевство Рюкю", "Shuri", 127.719, 26.218, leader_id("RYUKYU"), civ_id("RYUKYU")),
    Seat("AINU", "アイヌ", "Ainu", "Айны", "Nibutani", 142.149, 42.633, leader_id("AINU"), civ_id("AINU")),
)
ISLAND_ANCHORS = (
    ("Yonaguni", 123.000, 24.460),
    ("Ishigaki", 124.160, 24.340),
    ("Miyako", 125.280, 24.800),
    ("Tokunoshima", 128.990, 27.780),
    ("Amami", 129.490, 28.370),
)

R = 6371.0088  # km, spherical authalic approximation
LON0 = math.radians(138)
LAT0 = math.radians(36)
ROTATION = math.radians(45)  # NE-SW archipelago axis roughly horizontal on the game board
ROW_STEP = math.sqrt(3) / 2
GEOGRAPHIC_BOUNDS = (122.0, 23.5, 146.0, 46.5)


def in_scope(lon: float, lat: float) -> bool:
    west, south, east, north = GEOGRAPHIC_BOUNDS
    # Ogasawara, Iwo Jima and Minamitorishima are outside the Hokkaido-Ryukyu
    # archipelago being measured; otherwise they double the board's sea area.
    return west <= lon <= east and south <= lat <= north and not (lat < 31 and lon >= 139)


def project(lon: float, lat: float) -> tuple[float, float]:
    """Lambert azimuthal equal-area projection centred on Japan, in km."""
    lam, phi = math.radians(lon) - LON0, math.radians(lat)
    den = 1 + math.sin(LAT0) * math.sin(phi) + math.cos(LAT0) * math.cos(phi) * math.cos(lam)
    k = math.sqrt(2 / den)
    x = R * k * math.cos(phi) * math.sin(lam)
    y = R * k * (math.cos(LAT0) * math.sin(phi) - math.sin(LAT0) * math.cos(phi) * math.cos(lam))
    return (x * math.cos(ROTATION) + y * math.sin(ROTATION),
            -x * math.sin(ROTATION) + y * math.cos(ROTATION))


def unproject(x: float, y: float) -> tuple[float, float]:
    x, y = (x * math.cos(ROTATION) - y * math.sin(ROTATION),
            x * math.sin(ROTATION) + y * math.cos(ROTATION))
    rho = math.hypot(x, y)
    if rho < 1e-12:
        return 138.0, 36.0
    c = 2 * math.asin(min(1, rho / (2 * R)))
    phi = math.asin(math.cos(c) * math.sin(LAT0) + y * math.sin(c) * math.cos(LAT0) / rho)
    lam = LON0 + math.atan2(x * math.sin(c), rho * math.cos(LAT0) * math.cos(c)
                                  - y * math.sin(LAT0) * math.sin(c))
    return math.degrees(lam), math.degrees(phi)


@dataclass(frozen=True)
class HexGrid:
    km: int
    width: int
    height: int
    xmin: float
    ymin: float

    @classmethod
    def for_scale(cls, km: int, japan: list[Ring]) -> "HexGrid":
        projected = [project(lon, lat) for ring in japan for lon, lat in ring.points
                     if in_scope(lon, lat)]
        minx, maxx = min(p[0] for p in projected), max(p[0] for p in projected)
        miny, maxy = min(p[1] for p in projected), max(p[1] for p in projected)
        xmin, ymin = minx - 2 * km, miny - 2 * km
        width = math.ceil((maxx - xmin + 2 * km) / km)
        height = math.ceil((maxy - ymin + 2 * km) / (ROW_STEP * km))
        return cls(km, width, height, xmin, ymin)

    def centre(self, col: int, row: int) -> tuple[float, float]:
        return (self.xmin + (col + 0.5 + 0.5 * (row & 1)) * self.km,
                self.ymin + (row + 0.5) * ROW_STEP * self.km)

    def closest(self, lon: float, lat: float) -> tuple[int, int]:
        px, py = project(lon, lat)
        nominal = max(0, min(self.height - 1,
                             round((py - self.ymin) / (ROW_STEP * self.km) - 0.5)))
        candidates = []
        for row in range(max(0, nominal - 2), min(self.height, nominal + 3)):
            nominal_col = max(0, min(self.width - 1,
                                     round((px - self.xmin) / self.km - 0.5 - 0.5 * (row & 1))))
            for col in range(max(0, nominal_col - 1), min(self.width, nominal_col + 2)):
                x, y = self.centre(col, row)
                candidates.append(((x - px) ** 2 + (y - py) ** 2, col, row))
        _, col, row = min(candidates)
        return col, row

    def neighbours(self, col: int, row: int) -> tuple[tuple[int, int], ...]:
        # Civ6 pointy hexes, odd rows shifted east by half a column.
        deltas = ((-1, 0), (1, 0), (row & 1, -1), ((row & 1) - 1, -1),
                  (row & 1, 1), ((row & 1) - 1, 1))
        return tuple((col + dx, row + dy) for dx, dy in deltas
                     if 0 <= col + dx < self.width and 0 <= row + dy < self.height)

    def index(self, col: int, row: int) -> int:
        return row * self.width + col


@dataclass(frozen=True)
class Ring:
    points: tuple[tuple[float, float], ...]
    bbox: tuple[float, float, float, float]

    def contains(self, lon: float, lat: float) -> bool:
        a, b, c, d = self.bbox
        if not (a <= lon <= c and b <= lat <= d):
            return False
        inside = False
        for (x1, y1), (x2, y2) in zip(self.points, self.points[1:]):
            if (y1 > lat) != (y2 > lat) and lon < x1 + (lat - y1) * (x2 - x1) / (y2 - y1):
                inside = not inside
        return inside


def read_shapes(zipped: Path, stem: str, *, country: str | None = None) -> list[Ring]:
    with zipfile.ZipFile(zipped) as archive:
        raw = archive.read(stem + ".shp")
        chosen = None
        if country:
            dbf = archive.read(stem + ".dbf")
            header_len, record_len = struct.unpack_from("<HH", dbf, 8)
            count = struct.unpack_from("<I", dbf, 4)[0]
            fields = []
            offset = 1
            for pos in range(32, header_len - 1, 32):
                name = dbf[pos:pos + 11].split(b"\0", 1)[0].decode("ascii")
                length = dbf[pos + 16]
                fields.append((name, offset, length))
                offset += length
            iso_offset, iso_length = next((off, length) for name, off, length in fields if name == "ADM0_A3")
            chosen = {i for i in range(count) if
                      dbf[header_len + i * record_len + iso_offset:
                          header_len + i * record_len + iso_offset + iso_length].strip() == country.encode()}
        result = []
        cursor, record = 100, 0
        while cursor < len(raw):
            words = struct.unpack_from(">I", raw, cursor + 4)[0]
            record_end = cursor + 8 + words * 2
            if chosen is None or record in chosen:
                shape_type = struct.unpack_from("<I", raw, cursor + 8)[0]
                if shape_type not in (5, 15, 25):
                    raise ValueError(f"Unsupported shapefile polygon type {shape_type}")
                parts_count, points_count = struct.unpack_from("<II", raw, cursor + 44)
                parts = struct.unpack_from("<" + "I" * parts_count, raw, cursor + 52)
                start = cursor + 52 + 4 * parts_count
                for lo, hi in zip(parts, (*parts[1:], points_count)):
                    points = tuple(struct.unpack_from("<dd", raw, start + 16 * j) for j in range(lo, hi))
                    xs, ys = zip(*points)
                    bbox = (min(xs), min(ys), max(xs), max(ys))
                    if bbox[2] >= GEOGRAPHIC_BOUNDS[0] and bbox[0] <= GEOGRAPHIC_BOUNDS[2] and bbox[3] >= GEOGRAPHIC_BOUNDS[1] and bbox[1] <= GEOGRAPHIC_BOUNDS[3]:
                        result.append(Ring(points, bbox))
            cursor, record = record_end, record + 1
        return result


def contains(rings: list[Ring], lon: float, lat: float) -> bool:
    return sum(ring.contains(lon, lat) for ring in rings) % 2 == 1


def load_dem(path: Path) -> tuple[float, float, list[list[float]]]:
    rows: list[list[float]] = []
    with path.open(encoding="latin-1", newline="") as handle:
        reader = csv.reader(handle)
        next(reader)
        next(reader)
        lat0 = lon0 = None
        current_lat = None
        for lat_s, lon_s, height_s in reader:
            lat, lon = float(lat_s), float(lon_s)
            if lat0 is None:
                lat0, lon0 = lat, lon
            if current_lat is None or abs(lat - current_lat) > 1e-5:
                rows.append([])
                current_lat = lat
            rows[-1].append(float(height_s))
    if not rows or len({len(row) for row in rows}) != 1:
        raise ValueError("ETOPO CSV grid is incomplete")
    return lat0, lon0, rows


def elevation(dem: tuple[float, float, list[list[float]]], lon: float, lat: float) -> float:
    lat0, lon0, rows = dem
    iy = max(0, min(len(rows) - 1, round((lat - lat0) / 0.05)))
    ix = max(0, min(len(rows[0]) - 1, round((lon - lon0) / 0.05)))
    return rows[iy][ix]


def obtain_sources(cache: Path) -> None:
    cache.mkdir(parents=True, exist_ok=True)
    for name, (url, expected) in SOURCE.items():
        path = cache / name
        if not path.exists():
            print(f"Downloading {url}", flush=True)
            with urllib.request.urlopen(url, timeout=120) as response, path.open("wb") as output:
                while chunk := response.read(1024 * 1024):
                    output.write(chunk)
        actual = sha256(path.read_bytes()).hexdigest()
        if actual != expected:
            raise ValueError(f"Source hash changed: {name} {actual}; expected {expected}")


def create_schema(source: Path, target: Path) -> sqlite3.Connection:
    if not source.is_file():
        raise FileNotFoundError(f"Official Civ6 schema source missing: {source}")
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        target.unlink()
    with sqlite3.connect(source) as original:
        statements = original.execute("SELECT type, sql FROM sqlite_master WHERE type IN ('table', 'index') AND sql IS NOT NULL ORDER BY type DESC, name").fetchall()
    db = sqlite3.connect(target)
    try:
        for _, statement in statements:
            db.execute(statement)
    except BaseException:
        db.close()
        target.unlink()
        raise
    return db


CITY_MIN_RANGE = 3  # GlobalParameters: no city within 3 hexes of another


def hex_distance(a: tuple[int, int], b: tuple[int, int]) -> int:
    """Distance on Civ6's odd-row-shifted grid (no wrap on these maps)."""
    def cube(col: int, row: int) -> tuple[int, int]:
        return col - (row - (row & 1)) // 2, row
    (ax, az), (bx, bz) = cube(*a), cube(*b)
    return max(abs(ax - bx), abs(az - bz), abs((ax + az) - (bx + bz)))


def close_start_pairs(starts: list[dict]) -> list[list]:
    """Seat pairs too close for both capitals to be founded where they start."""
    return [[a["seat"], b["seat"], d] for i, a in enumerate(starts) for b in starts[i + 1:]
            if (d := hex_distance((a["col"], a["row"]), (b["col"], b["row"]))) <= CITY_MIN_RANGE]


# A seat may stand for the same physical site as a draft castle even when the
# labels/coordinates differ. The remaining three seats are additional cities:
# Oda's Nagoya (draft owner: Imagawa), Date's Yonezawa, Ashikaga's Muromachi.
SEAT_CASTLES = {
    "ASHINA": "ASHINA_KUROKAWA", "HOJO": "HOJO_ODAWARA",
    "YAMANOUCHI_UESUGI": "UESUGI_HIRAI", "TAKEDA": "TAKEDA_TSUTSUJI",
    "NAGAO": "NAGAO_KASUGAYAMA", "IMAGAWA": "IMAGAWA_SUNPU",
    "MATSUDAIRA": "MATSUDAIRA_OKAZAKI", "ASAKURA": "ASAKURA_ICHIJODANI",
    "MIYOSHI": "MIYOSHI_SHOZUI", "ROKKAKU": "ROKKAKU_KANNONJI",
    "HONGANJI": "HONGANJI_ISHIYAMA", "OUCHI": "OUCHI_YAMAGUCHI",
    "AMAGO": "AMAGO_TODA", "MORI": "MORI_KORIYAMA",
    "CHOSOKABE": "CHOSOKABE_OKO", "OTOMO": "OTOMO_RESIDENCE",
    "SHIMAZU": "SHIMAZU_SHIMIZU", "RYUKYU": "RYUKYU_SHURI",
    "AINU": "AINU_POROMOI",
}
EXTRA_CAPITAL_NAMES = {"ODA": "那古野城", "DATE": "米沢城", "ASHIKAGA": "室町殿"}

# Province centres used only where the 53-castle draft has no site. These are
# hand-placed geographic proxies, NOT historical boundaries. Their owners are
# checked against the province allocation table on every generation.
TERRITORY_ANCHORS = (
    ("山城", "ASHIKAGA", 135.76, 35.01), ("大和", "ASHIKAGA", 135.83, 34.68),
    ("和泉", "ASHIKAGA", 135.46, 34.58), ("伊賀", "ASHIKAGA", 136.13, 34.77),
    ("志摩", "ASHIKAGA", 136.85, 34.36), ("伊豆", "HOJO", 138.94, 35.12),
    ("安房", "HOJO", 139.87, 34.99), ("上総", "YAMANOUCHI_UESUGI", 140.10, 35.48),
    ("美濃", "ASHIKAGA", 136.76, 35.42), ("下野", "YAMANOUCHI_UESUGI", 139.88, 36.56),
    ("加賀", "HONGANJI", 136.66, 36.56), ("佐渡", "NAGAO", 138.40, 38.00),
    ("丹後", "ASHIKAGA", 135.20, 35.54), ("但馬", "ASHIKAGA", 134.82, 35.54),
    ("因幡", "ASHIKAGA", 134.24, 35.50), ("伯耆", "AMAGO", 133.33, 35.43),
    ("隠岐", "AMAGO", 133.30, 36.20), ("美作", "AMAGO", 134.00, 35.07),
    ("備後", "OUCHI", 133.36, 34.50), ("長門", "OUCHI", 130.94, 34.00),
    ("紀伊", "ASHIKAGA", 135.17, 34.22), ("紀伊", "ASHIKAGA", 135.75, 33.75),
    ("淡路", "MIYOSHI", 134.90, 34.34), ("筑前", "OUCHI", 130.40, 33.60),
    ("筑後", "OTOMO", 130.50, 33.30), ("豊前", "OUCHI", 131.14, 33.60),
    ("肥前", "OTOMO", 130.30, 33.20), ("肥前", "OTOMO", 129.90, 32.80),
    ("肥後", "OTOMO", 130.70, 32.80), ("肥後", "OUCHI", 130.82, 33.00),
    ("大隅", "SHIMAZU", 130.85, 31.40), ("壱岐", "OTOMO", 129.70, 33.75),
    ("対馬", "OUCHI", 129.30, 34.30),
)


def choose_cities(grid: HexGrid, passable: set[int], starts: list[dict],
                  castles: list[dict]) -> list[dict]:
    """Reserve representative capitals first; place every draft castle next."""
    sites = {castle["id"]: castle for castle in castles}
    expected = {seat.slug for seat in SEATS} - set(SEAT_CASTLES)
    if expected != set(EXTRA_CAPITAL_NAMES) or len(sites) != 53:
        raise ValueError("1534 castle/seat inventory changed; review capital aliases")
    used = {start["plot"] for start in starts}
    cities = []
    for start in starts:
        slug = start["seat"]
        alias = SEAT_CASTLES.get(slug)
        name = sites[alias]["name"] if alias else EXTRA_CAPITAL_NAMES[slug]
        cities.append({**start, "id": alias or f"{slug}_SEAT", "name": name,
                       "owner": slug, "capital": True, "province":
                       sites[alias]["province_or_region"] if alias else "代表本拠"})
    available = [(grid.index(col, row), *grid.centre(col, row))
                 for row in range(grid.height) for col in range(grid.width)
                 if grid.index(col, row) in passable and grid.index(col, row) not in used]
    for castle in castles:
        if castle["id"] in SEAT_CASTLES.values():
            continue
        target = project(float(castle["lon"]), float(castle["lat"]))
        distance, plot = min((math.hypot(x - target[0], y - target[1]), plot)
                             for plot, x, y in available if plot not in used)
        if distance > 50:
            raise ValueError(f"Castle too far from land: {castle['id']} {distance:.1f} km")
        used.add(plot)
        cities.append({"id": castle["id"], "name": castle["name"],
                       "owner": castle["owner_faction_1534_draft"], "capital": False,
                       "province": castle["province_or_region"],
                       "lon": float(castle["lon"]), "lat": float(castle["lat"]),
                       "col": plot % grid.width, "row": plot // grid.width,
                       "plot": plot, "displacement_km": round(distance, 1)})
    return cities


def close_city_pairs(cities: list[dict]) -> list[list]:
    return [[a["id"], b["id"], distance]
            for i, a in enumerate(cities) for b in cities[i + 1:]
            if (distance := hex_distance((a["col"], a["row"]),
                                         (b["col"], b["row"]))) <= CITY_MIN_RANGE]


def assign_land(grid: HexGrid, land_tiles: set[int], cities: list[dict]) -> list[tuple]:
    """Approximate allocation by the nearest castle/seat in projected space.

    Remote islands are explicitly assigned to the SSOT's island factions. The
    finer old-province/district boundaries in the draft are not polygon data.
    """
    centres = [(city, grid.centre(city["col"], city["row"])) for city in cities]
    allocations = {entry["province"]: {part["owner_faction"] for part in entry["parts"]}
                   for entry in data()["province_allocations"]}
    if any(owner not in allocations[province] for province, owner, _, _ in TERRITORY_ANCHORS):
        raise ValueError("Territory anchor disagrees with the 1534 province allocation")
    influence = centres + [({"owner": owner, "plot": -1}, project(lon, lat))
                           for _, owner, lon, lat in TERRITORY_ANCHORS]
    by_faction = {slug: [(city, xy) for city, xy in centres if city["owner"] == slug]
                  for slug in {city["owner"] for city in cities}}
    centre_by_plot = {city["plot"]: city for city in cities}
    ainu_seed = next(city["plot"] for city in cities if city["owner"] == "AINU")
    ainu_island = {ainu_seed}
    frontier = [ainu_seed]
    while frontier:
        plot = frontier.pop()
        for col, row in grid.neighbours(plot % grid.width, plot // grid.width):
            neighbour = grid.index(col, row)
            if neighbour in land_tiles and neighbour not in ainu_island:
                ainu_island.add(neighbour)
                frontier.append(neighbour)
    results = []
    for plot in sorted(land_tiles):
        col, row = plot % grid.width, plot // grid.width
        xy = grid.centre(col, row)
        lon, lat = unproject(*xy)
        # The 22-faction constraint assigns all of Hokkaido to Ainu, and the
        # southern island chain to Ryukyu/one of the Kyushu factions.
        forced = "AINU" if plot in ainu_island or lat >= 41.8 else "RYUKYU" if lat < 29.2 else None
        candidates = by_faction[forced] if forced else [item for item in influence if item[0]["owner"] != "AINU"]
        owner = (centre_by_plot[plot]["owner"] if plot in centre_by_plot else
                 min(candidates, key=lambda item: ((item[1][0] - xy[0]) ** 2 +
                                                   (item[1][1] - xy[1]) ** 2,
                                                   item[0]["plot"]))[0]["owner"])
        closest = min(by_faction[owner], key=lambda item: ((item[1][0] - xy[0]) ** 2 +
                                                         (item[1][1] - xy[1]) ** 2,
                                                         item[0]["plot"]))[0]
        # CityOwner/CityWorking are city-centre plot IDs in Civ6Map's schema.
        results.append((plot, SEAT_INDEX[closest["owner"]], closest["plot"], closest["plot"]))
    return results


SEAT_INDEX = {seat.slug: index for index, seat in enumerate(SEATS)}


def choose_starts(grid: HexGrid, passable: set[int]) -> list[dict]:
    available = [(col, row, *grid.centre(col, row)) for row in range(grid.height)
                 for col in range(grid.width) if grid.index(col, row) in passable]
    used: set[int] = set()
    result = []
    for seat in SEATS:
        target = project(seat.lon, seat.lat)
        scored = sorted(((math.hypot(x - target[0], y - target[1]), col, row)
                         for col, row, x, y in available if grid.index(col, row) not in used))
        distance, col, row = scored[0]
        if distance > max(90, 2.5 * grid.km):
            raise ValueError(f"No suitable Japanese land start near {seat.castle}: {distance:.1f} km")
        index = grid.index(col, row)
        used.add(index)
        result.append({"seat": seat.slug, "castle": seat.castle, "civilization": seat.civ,
                       "leader": seat.leader, "lon": seat.lon, "lat": seat.lat,
                       "col": col, "row": row, "plot": index, "displacement_km": round(distance, 1)})
    return result


def generate_map(km: int, land: list[Ring], japan: list[Ring], dem, official: Path,
                 destination: Path, *, settlers: bool = False) -> dict:
    grid = HexGrid.for_scale(km, japan)
    land_tiles: set[int] = set()
    heights: dict[int, float] = {}
    for row in range(grid.height):
        for col in range(grid.width):
            lon, lat = unproject(*grid.centre(col, row))
            if not in_scope(lon, lat):
                continue
            if contains(japan, lon, lat) and contains(land, lon, lat):
                index = grid.index(col, row)
                land_tiles.add(index)
                heights[index] = elevation(dem, lon, lat)
    # A city on a sub-tile island (especially Shuri at the coarse scales)
    # should survive centre-point rasterization.  Its source coordinate is
    # verified against both Natural Earth layers before promoting one hex.
    for name, lon, lat in ([(s.castle, s.lon, s.lat) for s in SEATS] + list(ISLAND_ANCHORS)):
        if not (contains(japan, lon, lat) and contains(land, lon, lat)):
            raise ValueError(f"Anchor is outside Japanese land: {name}")
        col, row = grid.closest(lon, lat)
        index = grid.index(col, row)
        land_tiles.add(index)
        heights[index] = min(heights.get(index, elevation(dem, lon, lat)), 1199)
    # Starting sites must be traversable.  Mountains still count as land.
    passable = {index for index in land_tiles if heights[index] < 1200}
    starts = choose_starts(grid, passable)
    castles = data()["castles"]
    cities = choose_cities(grid, passable, starts, castles)
    coast = set()
    for index in land_tiles:
        col, row = index % grid.width, index // grid.width
        for ncol, nrow in grid.neighbours(col, row):
            neighbour = grid.index(ncol, nrow)
            if neighbour not in land_tiles:
                coast.add(neighbour)
    db = create_schema(official, destination)
    try:
        db.execute("INSERT INTO Map VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                   ("Default", grid.width, grid.height, 47, 23, 0, 0, "MAPSIZE_HUGE"))
        db.executemany("INSERT INTO MapAttributes VALUES (?, ?)",
                       (("MapScript", "Continents.lua"), ("Ruleset", "RULESET_EXPANSION_2")))
        db.executemany("INSERT INTO MetaData VALUES (?, ?)",
                       (("MetaDataVersion", "1"), ("AppVersion", "1.0.12.68"),
                        ("DisplayName", destination.name),
                        ("ID", str(uuid.uuid5(uuid.NAMESPACE_URL, "JAC/JapanMapScale/" + str(km))).upper()),
                        ("IsMod", "1"), ("IsBasic", "1"), ("LastUpdated", "0")))
        plots = []
        for index in range(grid.width * grid.height):
            if index in land_tiles:
                h = heights[index]
                terrain = ("TERRAIN_GRASS_MOUNTAIN" if h >= 1200 else
                           "TERRAIN_GRASS_HILLS" if h >= 400 else "TERRAIN_GRASS")
                continent, impassable = "CONTINENT_ASIA", int(h >= 1200)
            else:
                terrain = "TERRAIN_COAST" if index in coast else "TERRAIN_OCEAN"
                continent, impassable = "", 0
            plots.append((index, terrain, continent, impassable))
        db.executemany("INSERT INTO Plots VALUES (?, ?, ?, ?)", plots)
        db.executemany("INSERT INTO StartPositions VALUES (?, 'LEADER', ?)",
                       ((start["plot"], start["leader"]) for start in starts))
        if not settlers:
            db.executemany("INSERT INTO Players VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                           ((i, seat.civ, seat.leader, "CIVILIZATION_LEVEL_FULL_CIV", "",
                             "Human" if i == 0 else "AI", "DIFFICULTY_PRINCE", "INVALID",
                             "RANDOM", 1) for i, seat in enumerate(SEATS)))
            db.executemany("INSERT INTO Cities VALUES (?, ?, ?)",
                           ((SEAT_INDEX[city["owner"]], city["plot"],
                             f"LOC_JAC_1534_CAPITAL_{city['owner']}" if city["capital"] else city["name"])
                            for city in cities))
            db.executemany("INSERT INTO Districts VALUES ('DISTRICT_CITY_CENTER', ?, ?)",
                           ((city["plot"], city["plot"]) for city in cities))
            db.executemany("INSERT INTO CityAttributes VALUES (?, 'Population', 'Initial', '2')",
                           ((city["plot"],) for city in cities))
            db.executemany("INSERT INTO PlotOwners VALUES (?, ?, ?, ?)",
                           assign_land(grid, land_tiles, cities))
        if db.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
            raise ValueError(f"Invalid SQLite map: {destination}")
        db.commit()
        db.execute("VACUUM")
    finally:
        db.close()
    return {"km_per_tile": km, "width": grid.width, "height": grid.height,
            "land_tiles": len(land_tiles), "sea_tiles": grid.width * grid.height - len(land_tiles),
            "starts": starts, "cities": cities, "close_city_pairs": close_city_pairs(cities),
            "close_start_pairs": close_start_pairs(starts),
            "start_mode": "settlers" if settlers else "historical",
            "file": destination.name, "bytes": destination.stat().st_size}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--official", type=Path, default=OFFICIAL)
    parser.add_argument("--cache", type=Path, default=CACHE)
    parser.add_argument("--output", type=Path, default=MOD / "Maps")
    args = parser.parse_args()
    obtain_sources(args.cache)
    land = read_shapes(args.cache / "ne_10m_land.zip", "ne_10m_land")
    japan = read_shapes(args.cache / "ne_10m_admin_0_countries.zip",
                        "ne_10m_admin_0_countries", country="JPN")
    if not land or not japan:
        raise ValueError("Natural Earth land/Japan polygons missing")
    dem = load_dem(args.cache / "etopo_2022_japan_3arcmin.csv")
    manifests = []
    for km in SCALES:
        item = generate_map(km, land, japan, dem, args.official,
                            args.output / f"JAC_Japan_{km}km.Civ6Map")
        settler_file = f"JAC_Japan_{km}km_Settlers.Civ6Map"
        generate_map(km, land, japan, dem, args.official,
                     args.output / settler_file, settlers=True)
        item["settler_file"] = settler_file
        for basename in (f"JAC_Japan_{km}km", f"JAC_Japan_{km}km_Settlers"):
            (args.output / f"{basename}.lua").write_text(
                "-- Static map: starting state is stored in the Civ6Map file.\n"
                "function InitializeNewGame() end\n"
                "LuaEvents.NewGameInitialized.Add(InitializeNewGame);\n", encoding="utf-8")
        manifests.append(item)
        print(f"{km} km: {item['width']}x{item['height']}, "
              f"{item['land_tiles']} land, {item['bytes']} bytes", flush=True)
    (args.output / "manifest.json").write_text(json.dumps(manifests, ensure_ascii=False, indent=2) + "\n",
                                                encoding="utf-8")
    if args.output.resolve() == (MOD / "Maps").resolve():
        import write_support
        write_support.main()


if __name__ == "__main__":
    main()
