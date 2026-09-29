"""Build the 1534 Japan maps from pinned public-domain source data.

Only the Python standard library is needed.  The official Civ6Map supplies the
SQLite schema; its map content is never copied into the generated files.
"""

from __future__ import annotations

import argparse
import csv
from collections import Counter, defaultdict
from dataclasses import dataclass
from hashlib import sha256
import heapq
import json
import math
from pathlib import Path
import sqlite3
import struct
import urllib.request
import uuid
import zipfile
import xml.etree.ElementTree as ET

from start_1534 import civ_id, data, leader_id


ROOT = Path(__file__).resolve().parents[2]
MOD = ROOT / "poc" / "JAC_PoC_JapanMapScale"
CACHE = Path(__file__).resolve().parent / "cache"
OFFICIAL = (Path("C:/Program Files (x86)/Steam/steamapps/common/")
            / "Sid Meier's Civilization VI/Base/Assets/Maps/EarthMaps/WorldMapHuge.Civ6Map")
SCALES = (20, 15)
RESOURCE_DRAFT = ROOT / "documents" / "spec" / "data" / "japan-map-resources.json"
START_YEAR = 1534
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


CITY_MIN_RANGE = 1  # Applied by the four Japan-map-only gameplay actions.
MIN_CITY_DISTANCE = CITY_MIN_RANGE + 1
MAX_NUDGE_TILES = 3


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
# English/Russian names of the 34 non-capital draft castles. A Civ6Map city name
# is shown as-is unless it is a LOC tag, so a literal Japanese name renders as
# tofu in the English font (live 2026-09-29). Every map city uses a LOC tag.
CASTLE_NAMES = {
    "DATE_KOORI": ("Koori Nishiyama", "Коори-Нисияма"),
    "DATE_YANAGAWA": ("Yanagawa", "Янагава"),
    "ASHINA_INAWASHIRO": ("Inawashiro", "Инавасиро"),
    "DATE_YAMAGATA": ("Yamagata", "Ямагата"),
    "HOJO_EDO": ("Edo", "Эдо"),
    "UESUGI_KAWAGOE": ("Kawagoe", "Кавагоэ"),
    "UESUGI_MINOWA": ("Minowa", "Минова"),
    "HOJO_KOGA": ("Koga", "Кога"),
    "UESUGI_OBOW": ("Oyumi", "Оюми"),
    "UESUGI_OTA": ("Ota", "Ота"),
    "TAKEDA_UEHARA": ("Uehara", "Уэхара"),
    "IMAGAWA_KAKEGAWA": ("Kakegawa", "Какэгава"),
    "ODA_SHOBATA": ("Shobata", "Сёбата"),
    "IMAGAWA_NAGOYA": ("Nagoya", "Нагоя"),
    "ASHIKAGA_KIRIYAMA": ("Kiriyama", "Кирияма"),
    "ASHIKAGA_NANAO": ("Nanao", "Нанао"),
    "ASHIKAGA_SAKURABORA": ("Sakurabora", "Сакурабора"),
    "ASHIKAGA_GOSEYAMA": ("Nochiseyama", "Нотисэяма"),
    "ASHIKAGA_HOJOZU": ("Hojozu", "Ходзёдзу"),
    "ASHIKAGA_KUWANOMI": ("Kuwanomi-dera", "Куваноми-дэра"),
    "ROKKAKU_KAMAHABA": ("Kamaha", "Камаха"),
    "MIYOSHI_SHIBAO": ("Shibo", "Сибо"),
    "MIYOSHI_YAGAMI": ("Yakami", "Яками"),
    "ASHIKAGA_TAKAYA": ("Takaya", "Такая"),
    "ASHIKAGA_OKISHIO": ("Okishio", "Окисио"),
    "MIYOSHI_AMAGIRI": ("Amagiri", "Амагири"),
    "ASHIKAGA_YUZUKI": ("Yuzuki", "Юдзуки"),
    "OUCHI_YAMABUKI": ("Yamabuki", "Ямабуки"),
    "OUCHI_TSUWANO": ("Tsuwano", "Цувано"),
    "ASHIKAGA_MITSUISHI": ("Mitsuishi", "Мицуиси"),
    "ASHIKAGA_BITCHU_MATSUYAMA": ("Bitchu Matsuyama", "Биттю-Мацуяма"),
    "SHIMAZU_IZAKU": ("Izaku", "Идзаку"),
    "SHIMAZU_OBI": ("Obi", "Оби"),
    "RYUKYU_NAKAGUSUKU": ("Nakagusuku", "Накагусуку"),
}


def city_name_tag(city: dict) -> str:
    """The LOC tag written into Cities.Name; the text files carry en/ja/ru rows."""
    return (f"LOC_JAC_1534_CAPITAL_{city['owner']}" if city["capital"]
            else f"LOC_JAC_1534_CITY_{city['id']}")

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
    if (expected != set(EXTRA_CAPITAL_NAMES) or len(sites) != 53
            or set(CASTLE_NAMES) != set(sites) - set(SEAT_CASTLES.values())):
        raise ValueError("1534 castle/seat inventory changed; review capital aliases")
    used = {start["plot"] for start in starts}
    cities = []
    for start in starts:
        slug = start["seat"]
        alias = SEAT_CASTLES.get(slug)
        name = sites[alias]["name"] if alias else EXTRA_CAPITAL_NAMES[slug]
        cities.append({**start, "id": alias or f"{slug}_SEAT", "name": name,
                       "owner": slug, "capital": True, "research_priority": 0, "province":
                       sites[alias]["province_or_region"] if alias else "代表本拠"})
    available = [(grid.index(col, row), *grid.centre(col, row))
                 for row in range(grid.height) for col in range(grid.width)
                 if grid.index(col, row) in passable and grid.index(col, row) not in used]
    # The numbered castle inventory in the research data is the priority order
    # after the 22 capitals. Keep it when writing every city-related DB table.
    for priority, castle in enumerate(castles, start=1):
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
                       "research_priority": priority,
                       "province": castle["province_or_region"],
                       "lon": float(castle["lon"]), "lat": float(castle["lat"]),
                       "col": plot % grid.width, "row": plot // grid.width,
                       "plot": plot, "displacement_km": round(distance, 1)})
    return ordered_cities(cities)


def ordered_cities(cities: list[dict]) -> list[dict]:
    """Capitals, then the numbered order of the 1534 research inventory."""
    return sorted(cities, key=lambda city: (not city["capital"],
                                           SEAT_INDEX[city["owner"]] if city["capital"]
                                           else city["research_priority"]))


def nudge_adjacent_cities(grid: HexGrid, land_tiles: set[int], passable: set[int],
                          cities: list[dict]) -> None:
    """Move only lower-priority cities, within their original owner's territory."""
    # Fix the territory before moving any city: otherwise the nearest-city
    # allocation could change while evaluating candidate plots.
    original_owners = {plot: SEATS[owner].slug for plot, owner, _, _ in
                       assign_land(grid, land_tiles, cities)}
    occupied = {city["plot"] for city in cities}
    placed: list[dict] = []
    for city in cities:
        if any(hex_distance((city["col"], city["row"]),
                            (earlier["col"], earlier["row"])) < MIN_CITY_DISTANCE
               for earlier in placed):
            if city["capital"]:
                raise ValueError(f"Adjacent capitals: {city['id']}")
            before = city["plot"]
            origin = grid.centre(city["col"], city["row"])
            historical = project(city["lon"], city["lat"])
            choices = []
            for plot in passable:
                if plot in occupied or original_owners[plot] != city["owner"]:
                    continue
                col, row = plot % grid.width, plot // grid.width
                if any(hex_distance((col, row), (earlier["col"], earlier["row"]))
                       < MIN_CITY_DISTANCE for earlier in placed):
                    continue
                xy = grid.centre(col, row)
                move = math.dist(origin, xy)
                if move <= MAX_NUDGE_TILES * grid.km + 1e-8:
                    choices.append((move, math.dist(historical, xy), plot))
            if not choices:
                raise ValueError(f"No owned land within {MAX_NUDGE_TILES} tiles for {city['id']}")
            move, displacement, plot = min(choices)
            occupied.remove(before)
            occupied.add(plot)
            city.update(plot=plot, col=plot % grid.width, row=plot // grid.width,
                        displacement_km=round(displacement, 1),
                        nudge={"from_plot": before, "to_plot": plot,
                               "distance_km": round(move, 1)})
        placed.append(city)


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


def insert_historical_state(db: sqlite3.Connection, cities: list[dict],
                            allocations: list[tuple]) -> None:
    """Insert city centres before ordinary owned plots, with capitals first."""
    ordered = ordered_cities(cities)
    db.executemany("INSERT INTO Cities VALUES (?, ?, ?)",
                   ((SEAT_INDEX[city["owner"]], city["plot"],
                     city_name_tag(city))
                    for city in ordered))
    db.executemany("INSERT INTO Districts VALUES ('DISTRICT_CITY_CENTER', ?, ?)",
                   ((city["plot"], city["plot"]) for city in ordered))
    db.executemany("INSERT INTO CityAttributes VALUES (?, 'Population', 'Initial', '2')",
                   ((city["plot"],) for city in ordered))
    # A map city is a capital only with a palace (live 2026-09-29: no player had a
    # capital without it; official DLC/VikingsScenario/Maps/Viking.Civ6Map does the same).
    db.executemany("INSERT INTO Buildings VALUES ('BUILDING_PALACE', ?)",
                   ((city["plot"],) for city in ordered if city["capital"]))
    # No settler on the historical start, so each capital gets one garrison;
    # without any unit a faction cannot act on turn 1 (live 2026-09-29).
    capitals = [city for city in ordered if city["capital"]]
    db.executemany("INSERT INTO Units VALUES (?, 'UNIT_WARRIOR', ?, ?, NULL)",
                   ((index, SEAT_INDEX[city["owner"]], city["plot"]) for index, city in enumerate(capitals)))
    city_order = {city["plot"]: index for index, city in enumerate(ordered)}
    db.executemany("INSERT INTO PlotOwners VALUES (?, ?, ?, ?)",
                   sorted(allocations, key=lambda row: (0, city_order[row[0]])
                          if row[0] in city_order else (1, row[0])))


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


def resource_draft() -> dict:
    return json.loads(RESOURCE_DRAFT.read_text(encoding="utf-8"))


def plot_center(grid: HexGrid, plot: int) -> tuple[float, float]:
    return grid.centre(plot % grid.width, plot // grid.width)


def lake_tile_count(lake: dict, km: int) -> int:
    """Estimate lake cells from area while respecting the draft's tile hint."""
    area_per_hex = math.sqrt(3) / 2 * km * km
    count = max(1, round(float(lake["area_km2"]) / area_per_hex))
    bounds = [int(value) for value in lake["tile_hint"].split("-")]
    low, high = (bounds[0], bounds[-1])
    return max(low, min(high, count))


def place_lakes(grid: HexGrid, land_tiles: set[int], cities: list[dict],
                lakes: list[dict]) -> tuple[set[int], list[dict]]:
    """Carve the area-based lake estimate as coast water before ownership."""
    city_plots = {city["plot"] for city in cities}
    selected: set[int] = set()
    results = []
    lake_names = {"lake-biwa": "琵琶湖", "lake-inawashiro": "猪苗代湖",
                  "lake-kasumigaura": "霞ヶ浦", "lake-towada": "十和田湖",
                  "lake-shinji": "宍道湖", "lake-suwa": "諏訪湖"}
    for lake in lakes:
        lon, lat = lake["center"]
        target = project(lon, lat)
        count = lake_tile_count(lake, grid.km)
        ranked = sorted(
            ((math.dist(plot_center(grid, plot), target), plot)
             for plot in range(grid.width * grid.height)
             if plot not in city_plots and plot not in selected))
        tiles = [plot for _, plot in ranked[:count]]
        if len(tiles) != count:
            raise ValueError(f"Could not rasterize lake: {lake['id']}")
        for plot in tiles:
            land_tiles.discard(plot)
            selected.add(plot)
        results.append({"id": lake["id"], "name_ja": lake_names.get(lake["id"], lake["id"]),
                        "province": lake["province"], "tile_ids": tiles,
                        "tile_count": count, "area_km2": lake["area_km2"],
                        "tile_hint": lake["tile_hint"], "source_ids": lake["source_ids"],
                        "representation": "TERRAIN_COAST"})
    return selected, results


RIVER_SIDES = {
    "NE": ("IsNEOfRiver", "EFlowDirection"),
    "W": ("IsWOfRiver", "SEFlowDirection"),
    "NW": ("IsNWOfRiver", "SWFlowDirection"),
}


def side_neighbour(grid: HexGrid, plot: int, side: str) -> int | None:
    col, row = plot % grid.width, plot // grid.width
    odd = row & 1
    delta = {"NE": (odd, -1), "W": (-1, 0), "NW": (odd - 1, -1)}[side]
    ncol, nrow = col + delta[0], row + delta[1]
    if 0 <= ncol < grid.width and 0 <= nrow < grid.height:
        return grid.index(ncol, nrow)
    return None


def river_edge_geometry(grid: HexGrid, edge: tuple[int, str], neighbor: int | None = None
                        ) -> tuple[tuple[float, float], tuple[float, float]]:
    """Return the two vertices of a map-edge river segment."""
    plot, _ = edge
    neighbor = side_neighbour(grid, plot, edge[1]) if neighbor is None else neighbor
    if neighbor is None:
        raise ValueError(f"River edge has no adjacent plot: {edge}")
    ax, ay = grid.centre(plot % grid.width, plot // grid.width)
    bx, by = grid.centre(neighbor % grid.width, neighbor // grid.width)
    length = math.hypot(bx - ax, by - ay)
    if not length:
        raise ValueError(f"River edge has coincident plot centres: {edge}")
    half_side = grid.km / (2 * math.sqrt(3))
    tx, ty = -(by - ay) / length, (bx - ax) / length
    mx, my = (ax + bx) / 2, (ay + by) / 2
    vertices = ((round(mx - tx * half_side, 5), round(my - ty * half_side, 5)),
                (round(mx + tx * half_side, 5), round(my + ty * half_side, 5)))
    return vertices


def land_river_edges(grid: HexGrid, land_tiles: set[int]) -> dict[tuple[int, str], dict]:
    edges = {}
    for plot in land_tiles:
        for side in RIVER_SIDES:
            neighbor = side_neighbour(grid, plot, side)
            if neighbor is None or neighbor not in land_tiles:
                continue
            edge = (plot, side)
            edges[edge] = {"plot": plot, "side": side, "neighbor": neighbor,
                           "vertices": river_edge_geometry(grid, edge, neighbor),
                           "midpoint": tuple((a + b) / 2 for a, b in
                                             zip(*river_edge_geometry(grid, edge, neighbor)))}
    return edges


def _point_segment_distance(point: tuple[float, float], a: tuple[float, float],
                            b: tuple[float, float]) -> float:
    dx, dy = b[0] - a[0], b[1] - a[1]
    length2 = dx * dx + dy * dy
    t = 0 if length2 == 0 else max(0.0, min(1.0,
        ((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / length2))
    return math.hypot(point[0] - (a[0] + t * dx), point[1] - (a[1] + t * dy))


def _river_edge_graph(edges: dict[tuple[int, str], dict]
                     ) -> dict[tuple[int, str], set[tuple[int, str]]]:
    at_vertex: dict[tuple[float, float], list[tuple[int, str]]] = defaultdict(list)
    graph = {edge: set() for edge in edges}
    for edge, record in edges.items():
        for vertex in record["vertices"]:
            at_vertex[vertex].append(edge)
    for adjacent in at_vertex.values():
        for edge in adjacent:
            graph[edge].update(other for other in adjacent if other != edge)
    return graph


def _route_river_edges(edges: dict[tuple[int, str], dict], graph: dict,
                       start: tuple[int, str], end: tuple[int, str],
                       segment: tuple[tuple[float, float], tuple[float, float]],
                       km: int) -> list[tuple[int, str]]:
    if start == end:
        return [start]
    distances = {start: 0.0}
    previous = {}
    queue = [(0.0, start)]
    while queue:
        distance, edge = heapq.heappop(queue)
        if distance != distances.get(edge):
            continue
        if edge == end:
            break
        for nxt in graph[edge]:
            deviation = _point_segment_distance(edges[nxt]["midpoint"], *segment)
            cost = 1.0 + (deviation / max(1.0, km * 0.45)) ** 2
            candidate = distance + cost
            if candidate < distances.get(nxt, math.inf):
                distances[nxt] = candidate
                previous[nxt] = edge
                heapq.heappush(queue, (candidate, nxt))
    if end not in distances:
        raise ValueError(f"River segment cannot be connected on land: {start} to {end}")
    path = [end]
    while path[-1] != start:
        path.append(previous[path[-1]])
    return list(reversed(path))


def rasterize_rivers(grid: HexGrid, land_tiles: set[int], rivers: list[dict]
                     ) -> tuple[dict[tuple[int, str], dict], list[dict]]:
    edges = land_river_edges(grid, land_tiles)
    graph = _river_edge_graph(edges)
    if not edges:
        raise ValueError("No shared land edges are available for rivers")
    edge_centres = {edge: record["midpoint"] for edge, record in edges.items()}
    results = []
    for river in rivers:
        points = [project(*coordinate) for coordinate in river["coordinates"]]
        snapped = [min(edge_centres, key=lambda edge: (
            math.dist(edge_centres[edge], point), edge[0], edge[1])) for point in points]
        path = []
        for a, b, start, end in zip(points, points[1:], snapped, snapped[1:]):
            segment_path = _route_river_edges(edges, graph, start, end, (a, b), grid.km)
            path.extend(edge for edge in segment_path if not path or path[-1] != edge)
        if not path:
            raise ValueError(f"River has no map edges: {river['id']}")
        results.append({"id": river["id"], "name_ja": river["name_ja"],
                        "source_ids": river["source_ids"],
                        "confidence": river.get("confidence"),
                        "historical_note": river.get("historical_note"),
                        "edge_count": len(path),
                        "edge_path": [{"plot": edges[edge]["plot"], "side": edge[1],
                                       "neighbor": edges[edge]["neighbor"]} for edge in path]})
    return edges, results


def place_volcanoes(grid: HexGrid, land_tiles: set[int], cities: list[dict],
                    lakes: set[int], volcanoes: list[dict]) -> list[dict]:
    reserved = {city["plot"] for city in cities} | lakes
    available = land_tiles - reserved
    selected: set[int] = set()
    results = []
    for volcano in volcanoes:
        lon, lat = volcano["coordinates"]
        target = project(lon, lat)
        plot = min(available - selected, key=lambda candidate: (
            math.dist(plot_center(grid, candidate), target),
            candidate))
        displacement = math.dist(plot_center(grid, plot), target)
        results.append({"id": volcano["id"], "site": volcano["site"], "plot": plot,
                        "feature_id": volcano["feature_id"], "province": volcano["province"],
                        "coordinates": volcano["coordinates"], "displacement_km": round(displacement, 1),
                        "eruption_years_in_scope": volcano["eruption_years_in_scope"],
                        "source_ids": volcano["source_ids"]})
        selected.add(plot)
    return results


def _resource_is_deferred(point: dict) -> bool:
    start_year = point.get("start_year")
    return (point.get("available_at_start") is False or
            isinstance(start_year, int) and start_year > START_YEAR)


# SSOT 3.2: a deferred site appears at its historical year or when the first faction finishes this
# tech, whichever comes first. Sites without an entry appear by year only. Provisional mapping.
REVEAL_TECH = {
    "silver-ikuno": "TECH_JAC_SENGOKU_CUPELLATION",
    "silver-sado-tsurushi": "TECH_JAC_SENGOKU_CUPELLATION",
    "copper-ashio": "TECH_JAC_SENGOKU_ORE_PROSPECTING",
    "gold-izu-toi": "TECH_JAC_SENGOKU_MINE_MANAGEMENT",
    "gold-noto-hodatsu": "TECH_JAC_SENGOKU_MINE_MANAGEMENT",
    "gold-sado-nishimikawa": "TECH_JAC_SENGOKU_MINE_MANAGEMENT",
    "gold-sado-aikawa": "TECH_JAC_SENGOKU_MINE_MANAGEMENT",
    "silver-nobesawa": "TECH_JAC_SENGOKU_MINE_MANAGEMENT",
}


def reveal_year(point: dict) -> int | None:
    """start_year, else the first year of a numeric start_year_range."""
    year = point.get("start_year")
    if isinstance(year, int):
        return year
    span = point.get("start_year_range") or []
    return span[0] if span and isinstance(span[0], int) else None


def official_resource_rules(official: Path) -> dict[str, dict]:
    """Read the installed base and GS gameplay rows used by this map's ruleset."""
    assets = official.parents[2]
    game = official.parents[4]
    sources = (assets / "Gameplay/Data/Resources.xml",
               game / "DLC/Expansion2/Data/Expansion1_Resources.xml",
               game / "DLC/Expansion2/Data/Expansion2_Resources.xml",
               game / "DLC/Expansion2/Data/Expansion2_RemoveData.xml",
               MOD / "Data" / "JAC_Japan_Resources.xml")  # custom gold/lacquer and silver on hills
    rules: dict[str, dict] = {}
    for source in sources:
        root = ET.parse(source).getroot()
        for row in root.findall("./Resources/Row"):
            kind = row.get("ResourceType")
            if kind:
                rules.setdefault(kind, {"terrains": set(), "features": set(), "attributes": {}})["attributes"].update(row.attrib)
        for table, key, field in (("Resource_ValidTerrains", "TerrainType", "terrains"),
                                  ("Resource_ValidFeatures", "FeatureType", "features")):
            for row in root.findall(f"./{table}/Row"):
                rules.setdefault(row.attrib["ResourceType"], {
                    "terrains": set(), "features": set(), "attributes": {}})[field].add(row.attrib[key])
            for row in root.findall(f"./{table}/Delete"):
                rules[row.attrib["ResourceType"]][field].discard(row.attrib[key])
    return rules


def resource_allowed(rule: dict, terrain: str, feature: str | None, *, lake: bool,
                     adjacent_land: bool, river: bool) -> bool:
    attributes = rule["attributes"]
    if feature:
        if feature not in rule["features"]:
            return False
    elif terrain not in rule["terrains"]:
        return False
    return not ((lake and attributes.get("LakeEligible") == "false") or
                (attributes.get("AdjacentToLand") == "true" and not adjacent_land) or
                (attributes.get("RequiresRiver") == "true" and not river) or
                (attributes.get("NoRiver") == "true" and river))


def place_resources(grid: HexGrid, land_tiles: set[int], coast_tiles: set[int],
                    lake_tiles: set[int], cities: list[dict], volcanoes: list[dict],
                    groups: list[dict], mappings: list[dict], terrain_by_plot: dict[int, str],
                    river_edges: dict, rules: dict[str, dict]
                    ) -> tuple[list[tuple[int, str, int]], list[dict], dict, dict[int, str]]:
    mapping_by_name = {entry["name_ja"]: entry for entry in mappings}
    custom_plots = {entry["plot"] for entry in volcanoes}
    city_plots = {city["plot"] for city in cities}
    occupied: dict[int, str] = {}
    records = []
    eligible = []
    for group in groups:
        mapping = mapping_by_name.get(group["resource_ja"], {})
        for point in group["points"]:
            deferred = _resource_is_deferred(point)
            eligible.append((group, point, mapping, deferred))
            records.append({"id": point["id"], "site": point["site"],
                            "province": point.get("province"), "resource_ja": group["resource_ja"],
                            "resource_id": group["civ6_id"],
                            "official_resource_id": group.get("official_scenario_id",
                                                               mapping.get("civ6_id")),
                            "availability": mapping.get("availability"),
                            "coordinates": point["coordinates"],
                            "start_year": point.get("start_year"),
                            "start_year_range": point.get("start_year_range"),
                            "available_at_start": point.get("available_at_start"),
                            "start_status": "deferred" if deferred else "start_candidate",
                            "placement_status": "not_placed",
                            "confidence": point.get("confidence"),
                            "source_ids": point.get("source_ids", [])})

    def priority(candidate):
        group, point, _, deferred = candidate
        certainty = {"high": 0, "medium": 1, "low": 2}.get(point.get("confidence"), 3)
        # Start sites claim their tiles first; later sites take the nearest free tile.
        return (deferred, certainty, group["candidate_count"], point["id"])

    by_id = {record["id"]: record for record in records}
    rows = []
    required_features: dict[int, str] = {}
    river_plots = {info[field] for info in river_edges.values() for field in ("plot", "neighbor")}
    candidate_plot_sets = {
        "land": sorted(land_tiles - city_plots - custom_plots - lake_tiles),
        "marine": sorted(coast_tiles - lake_tiles),
    }
    reserved: dict[int, str] = {}  # later-release sites: tiles kept free for the Lua API
    for group, point, mapping, deferred in sorted(eligible, key=priority):
        domain = "marine" if group.get("placement_domain") == "marine" else "land"
        target = project(*point["coordinates"])
        resource = group["civ6_id"]
        record = by_id[point["id"]]
        rule = rules.get(resource)
        if not rule:
            raise ValueError(f"No resource rule for {resource}")
        options = []
        for candidate in candidate_plot_sets[domain]:
            # A formally legal climate on a distant island is not the historical site.
            # A later site must not hop to another island, so its radius is tighter.
            limit = max(30, 2 * grid.km) if deferred else max(60, 3 * grid.km)
            if math.dist(plot_center(grid, candidate), target) > limit:
                continue
            terrain = terrain_by_plot.get(candidate, "TERRAIN_COAST")
            near_land = candidate in land_tiles or candidate in coast_tiles - lake_tiles
            river = candidate in river_plots
            feature = None
            replacement = None
            if resource_allowed(rule, terrain, None, lake=candidate in lake_tiles,
                                adjacent_land=near_land, river=river):
                pass
            elif ("FEATURE_FOREST" in rule["features"] and candidate in land_tiles and
                  terrain in {"TERRAIN_GRASS", "TERRAIN_GRASS_HILLS", "TERRAIN_PLAINS",
                              "TERRAIN_PLAINS_HILLS", "TERRAIN_TUNDRA", "TERRAIN_TUNDRA_HILLS"} and
                  resource_allowed(rule, terrain, "FEATURE_FOREST", lake=False,
                                   adjacent_land=True, river=river)):
                feature = "FEATURE_FOREST"
            elif (resource == "RESOURCE_SALT" and candidate in land_tiles and
                  terrain == "TERRAIN_GRASS" and
                  any(grid.index(col, row) not in land_tiles for col, row in grid.neighbours(
                      candidate % grid.width, candidate // grid.width)) and
                  resource_allowed(rule, "TERRAIN_PLAINS", None, lake=False,
                                   adjacent_land=True, river=river)):
                # Coastal salt pans are represented by a plains hex, not desert.
                replacement = "TERRAIN_PLAINS"
            else:
                continue
            options.append((candidate, feature, replacement))
        if deferred:
            # Lua cannot add a feature or reshape terrain later: a free tile that already fits.
            options = [option for option in options
                       if option[1] is None and option[2] is None and option[0] not in occupied
                       and option[0] not in reserved]
        candidates = options
        if not candidates:
            record["placement_status"] = "no_valid_official_plot"
            record["placement_reason"] = ("No nearby free plot satisfies the resource rules "
                                          "(the tile budget of this scale is used up).")
            continue
        plot, feature, replacement = min(candidates, key=lambda option: (
            math.dist(plot_center(grid, option[0]), target), option[0]))
        record["plot"] = plot
        record["displacement_km"] = round(math.dist(plot_center(grid, plot), target), 1)
        if deferred:
            reserved[plot] = point["id"]
            record["placement_status"] = "reserved_for_later_release"
            record["reveal_year"] = reveal_year(point)
            if point["id"] in REVEAL_TECH:
                record["reveal_tech"] = REVEAL_TECH[point["id"]]
            continue
        if plot in occupied:
            record["placement_status"] = "same_tile_conflict"
            record["conflict_with"] = occupied[plot]
            continue
        occupied[plot] = point["id"]
        record["placement_status"] = "placed"
        if replacement:
            terrain_by_plot[plot] = replacement
            record["terrain_adjustment"] = replacement
        if feature:
            required_features[plot] = feature
            record["required_feature"] = feature
        rows.append((plot, resource, 1))
    start_eligible = [group for group, _, _, deferred in eligible if not deferred]
    stats = {"candidate_count": len(records), "placed_count": len(rows),
             "land_candidate_count": sum(group.get("placement_domain") != "marine"
                                         for group in start_eligible),
             "marine_candidate_count": sum(group.get("placement_domain") == "marine"
                                           for group in start_eligible),
             "reserved_count": len(reserved),
             "count_by_type": dict(sorted(Counter(resource for _, resource, _ in rows).items())),
             "candidate_status_counts": dict(sorted(Counter(
                 record["placement_status"] if record["start_status"] == "start_candidate"
                 else record["start_status"] for record in records).items())),
             "custom_resource_ids": sorted({group["civ6_id"] for group in groups
                                            if group["civ6_id"].startswith("RESOURCE_JAC_")})}
    return rows, records, stats, required_features


def place_marshes(grid: HexGrid, land_tiles: set[int], terrain_by_plot: dict[int, str],
                  cities: list[dict], occupied: set[int], marsh_rule: dict
                  ) -> tuple[set[int], dict]:
    city_plots = {city["plot"] for city in cities}
    eligible = {plot for plot in land_tiles if terrain_by_plot[plot] == "TERRAIN_GRASS"
                and plot not in city_plots and plot not in occupied}
    target = min(33, max(22, round(len(land_tiles) * 0.025)))
    clusters = marsh_rule["clusters"]
    quotas = [target // len(clusters) + (index < target % len(clusters))
              for index in range(len(clusters))]
    selected: set[int] = set()
    centers = []
    for cluster, quota in zip(clusters, quotas):
        center = project(*cluster["center"])
        centers.append(center)
        nearby = sorted((math.dist(plot_center(grid, plot), center), plot)
                        for plot in eligible - selected)
        limit = max(2.5 * grid.km, 40)
        chosen = [plot for distance, plot in nearby if distance <= limit][:quota]
        selected.update(chosen)
    if len(selected) < target:
        rest = eligible - selected
        ranked = sorted((min(math.dist(plot_center(grid, plot), center)
                             for center in centers), plot) for plot in rest)
        selected.update(plot for _, plot in ranked[:target - len(selected)])
    if len(selected) != target:
        raise ValueError(f"Could not place the draft marsh target of {target} tiles")
    return selected, {"feature_id": marsh_rule["feature_id"], "target_count": target,
                      "placed_count": len(selected), "eligible_flat_land_tiles": len(eligible),
                      "cluster_ids": [cluster["id"] for cluster in clusters],
                      "clusters": [{"id": cluster["id"], "site": cluster["site"],
                                    "source_ids": cluster.get("source_ids", [])}
                                   for cluster in clusters],
                      "source_ids": marsh_rule.get("source_ids", [])}


FOREST_BIAS_REGIONS = (
    ((137.2, 36.0), 170, 1.6),  # Kiso-Hida
    ((135.7, 34.1), 140, 1.6),  # Kii-Yoshino
    ((132.8, 35.1), 160, 1.5),  # Chugoku mountains
    ((132.3, 33.0), 210, 1.5),  # Shikoku and Kyushu mountains
    ((140.5, 41.2), 360, 1.4),  # Ou and Hokkaido interior
)


def place_forests(grid: HexGrid, land_tiles: set[int], terrain_by_plot: dict[int, str],
                  cities: list[dict], occupied: set[int], rice_points: list[dict],
                  river_edges: dict, forest_rule: dict) -> tuple[set[int], dict]:
    city_plots = {city["plot"] for city in cities}
    feature_terrains = {"TERRAIN_GRASS", "TERRAIN_PLAINS", "TERRAIN_TUNDRA",
                        "TERRAIN_GRASS_HILLS", "TERRAIN_PLAINS_HILLS", "TERRAIN_TUNDRA_HILLS"}
    rice_centers = [project(*point["coordinates"]) for point in rice_points]
    rice_radius = 1.15 * grid.km
    eligible = set()
    for plot in land_tiles - city_plots - occupied:
        terrain = terrain_by_plot[plot]
        if terrain not in feature_terrains:
            continue
        if not terrain.endswith("_HILLS") and any(
                math.dist(plot_center(grid, plot), center) <= rice_radius
                for center in rice_centers):
            continue
        eligible.add(plot)

    target_percent = forest_rule["target_share_of_eligible_tiles_percent"]
    target = round(len(eligible) * target_percent / 100)
    riverbank = {edge_info[field] for edge_info in river_edges.values()
                 for field in ("plot", "neighbor")}
    ranked = []
    for plot in eligible:
        lon, lat = unproject(*plot_center(grid, plot))
        terrain_weight = 1.6 if terrain_by_plot[plot].endswith("_HILLS") else 0.8
        region_weight = max((weight for (center, radius, weight) in FOREST_BIAS_REGIONS
                             if math.dist(project(lon, lat), project(*center)) <= radius), default=1.0)
        river_weight = 0.65 if plot in riverbank else 1.0
        digest = sha256(f"JAC/JapanMapScale/forest/{grid.km}/{plot}".encode()).digest()
        uniform = (int.from_bytes(digest[:8], "big") + 1) / (2**64 + 1)
        score = -math.log(uniform) / (terrain_weight * region_weight * river_weight)
        ranked.append((score, plot))
    selected = {plot for _, plot in sorted(ranked)[:target]}
    return selected, {"feature_id": forest_rule["feature_id"], "target_percent": target_percent,
                      "eligible_tiles": len(eligible), "placed_count": len(selected),
                      "placed_percent": round(100 * len(selected) / len(eligible), 2) if eligible else 0,
                      "rice_basin_exclusion_radius_km": round(rice_radius, 1),
                      "source_ids": forest_rule.get("source_ids", [])}


def custom_content_inventory(draft: dict, resource_records: list[dict]) -> list[dict]:
    custom_resources: dict[str, dict] = {}
    for record in resource_records:
        if record["resource_id"].startswith("RESOURCE_JAC_"):
            entry = custom_resources.setdefault(record["resource_id"], {
                "type": record["resource_id"], "kind": "resource",
                "resource_ja": record["resource_ja"], "candidate_ids": [],
                "status": "defined_in_Data/JAC_Japan_Resources.xml"})
            entry["candidate_ids"].append(record["id"])
    result = list(custom_resources.values())
    for wonder in draft["terrain"].get("natural_wonder_candidates", []):
        result.append({"type": wonder["proposed_custom_feature_id"], "kind": "feature",
                       "name_ja": wonder["name_ja"], "candidate_ids": [wonder["id"]],
                       "status": "not_selected_or_implemented"})
    return result


def deferred_trade_access(draft: dict) -> list[dict]:
    results = []
    for item in draft["trade_access"]:
        year = item.get("available_from_year")
        results.append({**item, "start_status": "deferred_until_year" if year and year > START_YEAR
                        else "undated_trade_access_not_placed"})
    return results


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
    draft = resource_draft()
    original_land_count = len(land_tiles)
    lake_tiles, lake_records = place_lakes(grid, land_tiles, cities, draft["terrain"]["lakes"])
    # Lakes are final water before cities move, so the nudge and the territory
    # both see the final land; resources and rivers then avoid the final cities.
    passable &= land_tiles
    nudge_adjacent_cities(grid, land_tiles, passable, cities)
    terrain_by_plot = {}
    for plot in land_tiles:
        height = heights[plot]
        terrain_by_plot[plot] = ("TERRAIN_GRASS_MOUNTAIN" if height >= 1200 else
                                 "TERRAIN_GRASS_HILLS" if height >= 400 else "TERRAIN_GRASS")
    volcano_records = place_volcanoes(grid, land_tiles, cities, lake_tiles,
                                      draft["terrain"]["volcanoes"])
    for volcano in volcano_records:
        plot = volcano["plot"]
        if not terrain_by_plot[plot].endswith("_MOUNTAIN"):
            terrain_by_plot[plot] = "TERRAIN_GRASS_MOUNTAIN"
            volcano["terrain_promoted_to_mountain"] = True
        else:
            volcano["terrain_promoted_to_mountain"] = False
    river_edges, river_records = rasterize_rivers(grid, land_tiles, draft["terrain"]["rivers"])
    coast = set()
    for index in land_tiles:
        col, row = index % grid.width, index // grid.width
        for ncol, nrow in grid.neighbours(col, row):
            neighbour = grid.index(ncol, nrow)
            if neighbour not in land_tiles:
                coast.add(neighbour)
    coast.update(lake_tiles)
    resource_rules = official_resource_rules(official)
    resource_rows, resource_records, resource_stats, required_resource_features = place_resources(
        grid, land_tiles, coast, lake_tiles, cities, volcano_records,
        draft["resource_groups"], draft["resource_mapping"], terrain_by_plot,
        river_edges, resource_rules)
    occupied = {plot for plot, _, _ in resource_rows}
    occupied.update(record["plot"] for record in resource_records
                    if record["placement_status"] == "reserved_for_later_release")
    occupied.update(city["plot"] for city in cities)
    occupied.update(volcano["plot"] for volcano in volcano_records)
    marsh_tiles, marsh_stats = place_marshes(
        grid, land_tiles, terrain_by_plot, cities, occupied, draft["terrain"]["marsh"])
    occupied.update(marsh_tiles)
    rice_points = [point for group in draft["resource_groups"] if group["resource_ja"] == "米"
                   for point in group["points"]]
    forest_tiles, forest_stats = place_forests(
        grid, land_tiles, terrain_by_plot, cities, occupied, rice_points,
        river_edges, draft["terrain"]["forest"])
    feature_rows = ([(volcano["plot"], "FEATURE_VOLCANO") for volcano in volcano_records] +
                    sorted(required_resource_features.items()) +
                    [(plot, "FEATURE_MARSH") for plot in sorted(marsh_tiles)] +
                    [(plot, draft["terrain"]["forest"]["feature_id"])
                     for plot in sorted(forest_tiles)])
    feature_counts = dict(sorted(Counter(feature for _, feature in feature_rows).items()))
    river_edge_flags: dict[int, set[str]] = defaultdict(set)
    for river in river_records:
        for entry in river["edge_path"]:
            river_edge_flags[entry["plot"]].add(entry["side"])
    river_rows = []
    for plot, sides in sorted(river_edge_flags.items()):
        river_rows.append((plot, int("NE" in sides), int("W" in sides), int("NW" in sides),
                           -1, -1, -1))
    trade_records = deferred_trade_access(draft)
    custom_content = custom_content_inventory(draft, resource_records)
    deferred_resources = [record for record in resource_records
                          if record["start_status"] == "deferred" or
                          record["placement_status"] == "no_valid_official_plot"]
    wonder_records = [{**wonder, "status": "not_placed_custom_feature_candidate"}
                      for wonder in draft["terrain"].get("natural_wonder_candidates", [])]
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
                terrain = terrain_by_plot[index]
                continent = "CONTINENT_ASIA"
                impassable = int(heights[index] >= 1200 or terrain.endswith("_MOUNTAIN"))
            else:
                terrain = "TERRAIN_COAST" if index in coast or index in lake_tiles else "TERRAIN_OCEAN"
                continent, impassable = "", 0
            plots.append((index, terrain, continent, impassable))
        db.executemany("INSERT INTO Plots VALUES (?, ?, ?, ?)", plots)
        db.executemany("INSERT INTO PlotResources VALUES (?, ?, ?)", resource_rows)
        db.executemany("INSERT INTO PlotFeatures VALUES (?, ?)", feature_rows)
        db.executemany("INSERT INTO PlotRivers VALUES (?, ?, ?, ?, ?, ?, ?)", river_rows)
        db.executemany("INSERT INTO StartPositions VALUES (?, 'LEADER', ?)",
                       ((start["plot"], start["leader"]) for start in starts))
        if not settlers:
            db.executemany("INSERT INTO Players VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                           ((i, seat.civ, seat.leader, "CIVILIZATION_LEVEL_FULL_CIV", "",
                             "Human", "DIFFICULTY_PRINCE", "INVALID",
                             "RANDOM", 1) for i, seat in enumerate(SEATS)))
            insert_historical_state(db, cities, assign_land(grid, land_tiles, cities))
        if db.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
            raise ValueError(f"Invalid SQLite map: {destination}")
        db.commit()
        db.execute("VACUUM")
    finally:
        db.close()
    return {"km_per_tile": km, "width": grid.width, "height": grid.height,
            "grid_origin_projected_km": [round(grid.xmin, 6), round(grid.ymin, 6)],
            "land_tiles": len(land_tiles), "sea_tiles": grid.width * grid.height - len(land_tiles),
            "rasterized_land_tiles_before_lakes": original_land_count,
            "starts": starts, "cities": cities, "close_city_pairs": close_city_pairs(cities),
            "close_start_pairs": close_start_pairs(starts),
            "resource_placement": resource_stats, "resource_candidates": resource_records,
            "deferred_resources": deferred_resources, "trade_access": trade_records,
            "custom_content_required": custom_content,
            "rivers": river_records,
            "river_edge_count": sum(river["edge_count"] for river in river_records),
            "river_edge_unique_count": sum(len(sides) for sides in river_edge_flags.values()),
            "lake_records": lake_records,
            "terrain_features": {"feature_counts": feature_counts,
                                 "forest": forest_stats, "marsh": marsh_stats,
                                 "volcanoes": volcano_records,
                                 "lakes": lake_records,
                                 "natural_wonder_candidates": wonder_records,
                                 "lake_schema_representation":
                                 "WorldBuilderMap has no lake table or FEATURE_LAKE; lake plots use TERRAIN_COAST"},
            "start_mode": "settlers" if settlers else "historical",
            "file": destination.name, "bytes": destination.stat().st_size}


def deferred_sites(manifest: list[dict]) -> dict[str, list[dict]]:
    """Later-release resource sites per grid size ("WxH"), sorted by year then id."""
    result = {}
    for item in manifest:
        sites = [{"id": r["id"], "resource": r["resource_id"], "plot": r["plot"],
                  "year": r.get("reveal_year"), "tech": r.get("reveal_tech")}
                 for r in item["resource_candidates"]
                 if r["placement_status"] == "reserved_for_later_release"]
        result[f"{item['width']}x{item['height']}"] = sorted(
            sites, key=lambda site: (site["year"] or 9999, site["id"]))
    return result


def write_resource_sites(manifest: list[dict], target: Path) -> None:
    """Lua data for Scripts/JAC_Resources.lua; the plots are 0-based Map plot indexes."""
    lines = ["-- Generated by tools/japan_map_scale/generate.py (SSOT 3.2). Do not edit.",
             "ExposedMembers.JAC_ResourceSiteData = {"]
    for size, sites in deferred_sites(manifest).items():
        lines.append(f'\t["{size}"] = {{')
        for site in sites:
            fields = [f'id="{site["id"]}"', f'resource="{site["resource"]}"', f'plot={site["plot"]}']
            if site["year"] is not None:
                fields.append(f'year={site["year"]}')
            if site["tech"]:
                fields.append(f'tech="{site["tech"]}"')
            lines.append("\t\t{ " + ", ".join(fields) + " },")
        lines.append("\t},")
    lines.append("}")
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("\n".join(lines) + "\n", encoding="utf-8")


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
    write_resource_sites(manifests, args.output.parent / "Scripts" / "JAC_ResourceSites.lua")
    if args.output.resolve() == (MOD / "Maps").resolve():
        import write_support
        write_support.main()


if __name__ == "__main__":
    main()
