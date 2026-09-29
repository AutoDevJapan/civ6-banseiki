"""1534 prosperity plan (Q28) for the 22 factions / 56 cities of the Japan maps.

Research tool: it does not touch any map or mod file. It reads the generated
Civ6Map files (city ownership, terrain, resources) and the tables below, and
writes documents/spec/data/japan-1534-prosperity.json.  The factional
estimates are judgement calls with a stated confidence; see
documents/research/japan-1534-prosperity.md.

    python tools/japan_map_scale/prosperity.py            # rewrite the JSON
"""

from __future__ import annotations

import json
import math
import shutil
import sqlite3
import sys
import tempfile
from collections import Counter, defaultdict
from pathlib import Path

TOOL = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOL))
import generate as g  # noqa: E402
from start_1534 import data  # noqa: E402

OUT = g.ROOT / "documents/spec/data/japan-1534-prosperity.json"
MAPS = g.MOD / "Maps"

# Keichou 3 (1598) kokudaka by province, from the Taiko survey summary
# https://shiryobeya.com/shokuho/keicho3kokudaka.html (retrieved 2026-09-29).
KOKU_1598 = {
    "陸奥": 1672358, "出羽": 318095, "上野": 496377, "下野": 374083, "常陸": 530008,
    "上総": 378892, "下総": 393255, "安房": 45045, "武蔵": 667126, "相模": 194204,
    "甲斐": 227616, "信濃": 408358, "越後": 390770, "佐渡": 17030, "越中": 380298,
    "加賀": 355570, "能登": 210000, "越前": 499411, "若狭": 85000, "伊豆": 69832,
    "駿河": 150000, "遠江": 255160, "三河": 290750, "尾張": 571737, "美濃": 540000,
    "飛騨": 38000, "伊勢": 567105, "志摩": 17854, "伊賀": 100000, "近江": 775379,
    "山城": 225262, "丹後": 110784, "大和": 448946, "摂津": 356066, "河内": 242106,
    "和泉": 141513, "紀伊": 243550, "淡路": 62104, "播磨": 358534, "丹波": 263887,
    "但馬": 114235, "因幡": 88500, "伯耆": 100947, "出雲": 186650, "石見": 111770,
    "隠岐": 10980, "備前": 203388, "備中": 176929, "美作": 186018, "備後": 186100,
    "安芸": 194150, "周防": 167820, "長門": 130660, "阿波": 183500, "讃岐": 126200,
    "伊予": 336200, "土佐": 98200, "筑前": 325695, "筑後": 265998, "豊前": 140000,
    "豊後": 418313, "肥前": 309935, "肥後": 341220, "日向": 120088, "薩摩": 283483,
    "大隅": 175057,
}

# Share of a province that the game's province allocation gives to each owner
# when the allocation splits it (provisional, by county count / size).
SPLIT_SHARES = {
    "摂津": {"HONGANJI": 0.10, "MIYOSHI": 0.90},
    "尾張": {"ODA": 0.95, "IMAGAWA": 0.05},
    "武蔵": {"HOJO": 0.25, "YAMANOUCHI_UESUGI": 0.75},
    "下総": {"HOJO": 0.75, "YAMANOUCHI_UESUGI": 0.25},
    "信濃": {"TAKEDA": 0.90, "NAGAO": 0.10},
    "陸奥": {"DATE": 0.60, "ASHINA": 0.40},
    "出羽": {"DATE": 0.50, "ASHINA": 0.50},
    "安芸": {"MORI": 0.10, "AMAGO": 0.15, "OUCHI": 0.75},
    "肥後": {"OUCHI": 0.10, "OTOMO": 0.90},
}

# Judgement table. k_* : directly controlled kokudaka in 1534 on the 1598
# scale, in 10,000 koku (None = not koku based). Traits 0-3.
# martial, trade, port, faith, mine, learn.
FACTIONS = {
    "ODA": dict(k=(4, 8, 12), conf="低", cap_bonus=0, traits=(1, 3, 2, 1, 0, 0),
                basis="尾張の海東・海西郡の弾正忠家。勝幡城と津島・熱田の湊町の矢銭で伸びる途中。石高は小さいが商業は大きい"),
    "DATE": dict(k=(15, 27, 35), conf="低", cap_bonus=0, traits=(2, 1, 0, 1, 1, 0),
                 basis="伊達・信夫・置賜の直轄と近隣の従属国衆。陸奥北部と出羽の大半は独立性が強く数えない"),
    "ASHINA": dict(k=(15, 25, 32), conf="低", cap_bonus=0, traits=(1, 1, 0, 2, 0, 0),
                   basis="会津4郡と仙道南部の一族・従属国衆。出羽の仮割当は数えない"),
    "HOJO": dict(k=(35, 43, 52), conf="中", cap_bonus=0, traits=(3, 2, 1, 0, 0, 0),
                 basis="相模+伊豆(26.4万)と武蔵南部の江戸周辺(約25%=16.7万)。下総は政治的連携で数えない"),
    "YAMANOUCHI_UESUGI": dict(k=(40, 50, 60), conf="低", cap_bonus=0, traits=(2, 1, 0, 1, 0, 0),
                              basis="上野(49.6万)と武蔵北部(50万)の名目領のうち約半分が実効。1530年代は内紛と衰退の途中"),
    "TAKEDA": dict(k=(20, 23, 30), conf="中", cap_bonus=0, traits=(3, 0, 0, 1, 2, 0),
                   basis="甲斐一国(22.8万)。信虎が1519-31年に統一済み。信濃はまだ制圧前"),
    "NAGAO": dict(k=(25, 30, 36), conf="中低", cap_bonus=0, traits=(2, 2, 2, 0, 0, 0),
                  basis="越後(39.1万)の約75%。守護上杉定実を立てた為景の実効支配。直江津の青苧交易あり"),
    "IMAGAWA": dict(k=(28, 33, 40), conf="中", cap_bonus=0, traits=(1, 2, 2, 2, 0, 2),
                    basis="駿河(15万)+遠江(25.5万)の約70%。遠江は斯波氏と争奪中"),
    "MATSUDAIRA": dict(k=(15, 18, 24), conf="中低", cap_bonus=0, traits=(3, 0, 0, 0, 0, 0),
                       basis="三河(29.1万)の約60%。清康が西三河を固め東三河へ進出中(1535年没)"),
    "ASAKURA": dict(k=(38, 42, 50), conf="中高", cap_bonus=0, traits=(1, 2, 2, 2, 0, 3),
                    basis="越前(49.9万)の約85%。一乗谷は「北陸の小京都」と呼ばれた計画城下町"),
    "ASHIKAGA": dict(k=(8, 15, 25), conf="低", cap_bonus=2, traits=(0, 3, 0, 3, 0, 3),
                     basis="将軍家の直轄は山城の御料所程度。石高は小さいが京都の人口・寺社・商業と威信が最大"),
    "MIYOSHI": dict(k=(4, 8, 15), conf="低", cap_bonus=0, traits=(2, 2, 3, 0, 0, 0),
                    basis="阿波三好郡の三好家(元長は1532年没、長慶は12歳)。守護細川氏の阿波・讃岐・淡路は数えない"),
    "ROKKAKU": dict(k=(30, 40, 45), conf="中", cap_bonus=0, traits=(2, 2, 1, 1, 0, 0),
                    basis="近江(77.5万)のうち南近江の約50%。北近江は浅井氏が半独立"),
    "HONGANJI": dict(k=(10, 18, 30), conf="低", cap_bonus=1, traits=(0, 3, 2, 3, 0, 0),
                     basis="石山御坊・寺内町と加賀の一向一揆領(35.6万の約50%)。石高より門徒・寺内町・資金が力"),
    "OUCHI": dict(k=(55, 70, 85), conf="中", cap_bonus=0, traits=(1, 3, 3, 2, 3, 3),
                  basis="周防+長門(29.9万)、石見の約70%、筑前の約70%、豊前の約80%。山口と博多、石見銀山(1533年に灰吹法)"),
    "AMAGO": dict(k=(30, 40, 50), conf="中低", cap_bonus=0, traits=(3, 0, 1, 0, 3, 0),
                  basis="出雲+隠岐+伯耆(29.9万)と美作・石見・備後・安芸の一部。たたら製鉄の産地。影響は中国11か国に及ぶと伝わる"),
    "MORI": dict(k=(2, 3, 5), conf="低", cap_bonus=0, traits=(2, 0, 0, 0, 0, 0),
                 basis="吉田郡山周辺の一国衆(家督継承時に約3,000貫と伝わる。要確認)。大内方の従属国衆"),
    "CHOSOKABE": dict(k=(2, 3, 5), conf="低", cap_bonus=0, traits=(2, 0, 1, 0, 0, 0),
                      basis="土佐長岡郡岡豊と周辺。土佐(9.8万)の1/3未満。一条氏など土佐七雄の一つ"),
    "OTOMO": dict(k=(40, 48, 60), conf="中低", cap_bonus=0, traits=(1, 3, 3, 2, 0, 0),
                  basis="豊後(41.8万)の約85%、筑後の約40%、肥後の一部。府内は明との貿易港を持つ"),
    "SHIMAZU": dict(k=(6, 12, 20), conf="低", cap_bonus=0, traits=(3, 1, 2, 0, 0, 0),
                    basis="薩摩・大隅・日向南部の名目領(57.6万)のうち貴久(伊作家)が固めた範囲。宗家と分家が分裂中"),
    "RYUKYU": dict(k=None, conf="低", cap_bonus=0, traits=(0, 3, 3, 0, 0, 0), tier="C",
                   basis="石高なし。那覇の中継貿易(明・東南アジア・日本)で成り立つ。兵は少なく港・商業が主"),
    "AINU": dict(k=None, conf="低", cap_bonus=0, traits=(1, 1, 1, 0, 0, 0), tier="D",
                 basis="石高なし。各コタン・チャシの連合で単一の領域ではない(ゲーム上の扱い)"),
}
TRAIT_NAMES = ("martial", "trade", "port", "faith", "mine", "learn")
TRAIT_DISTRICT = {"trade": "DISTRICT_COMMERCIAL_HUB", "port": "DISTRICT_HARBOR",
                  "martial": "DISTRICT_ENCAMPMENT", "faith": "DISTRICT_HOLY_SITE",
                  "mine": "DISTRICT_INDUSTRIAL_ZONE", "learn": "DISTRICT_CAMPUS"}
TRAIT_ORDER = ("trade", "port", "martial", "faith", "mine", "learn")  # tie break
DISTRICT_BUILDING = {"DISTRICT_COMMERCIAL_HUB": "BUILDING_MARKET",
                     "DISTRICT_HARBOR": "BUILDING_LIGHTHOUSE",
                     "DISTRICT_ENCAMPMENT": "BUILDING_BARRACKS",
                     "DISTRICT_HOLY_SITE": "BUILDING_SHRINE",
                     "DISTRICT_INDUSTRIAL_ZONE": "BUILDING_WORKSHOP",
                     "DISTRICT_CAMPUS": "BUILDING_LIBRARY"}
# Gold upkeep from Base/Assets/Gameplay/Data (see the research doc for file:line).
BUILDING_UPKEEP = {"BUILDING_BARRACKS": 1, "BUILDING_SHRINE": 1, "BUILDING_TEMPLE": 2,
                   "BUILDING_LIBRARY": 1, "BUILDING_WORKSHOP": 1}
DISTRICT_UPKEEP = {"DISTRICT_ENCAMPMENT": 1, "DISTRICT_HOLY_SITE": 1, "DISTRICT_CAMPUS": 1,
                   "DISTRICT_INDUSTRIAL_ZONE": 1}
UNIT_UPKEEP = {"UNIT_SPEARMAN": 1, "UNIT_ARCHER": 1, "UNIT_WARRIOR": 0, "UNIT_SLINGER": 0}

# Theoretical mobilization: about 250 men per 10,000 koku (see the research doc).
TROOPS_PER_MAN_KOKU = 250
CAPITAL_POP = {"A": 6, "B": 5, "C": 4, "D": 3}
IMPORTANCE_POP_DELTA = {3: 0, 2: -1, 1: -2}

# Castle importance of the 34 non-capital cities: 2 = major castle / key town,
# 1 = branch or proxy castle. Kuwanomi is a temporary residence, not a castle.
IMPORTANCE = {
    "DATE_KOORI": 2, "DATE_YANAGAWA": 1, "DATE_YAMAGATA": 1, "ASHINA_INAWASHIRO": 1,
    "HOJO_EDO": 2, "HOJO_KOGA": 1, "UESUGI_KAWAGOE": 2, "UESUGI_MINOWA": 2,
    "UESUGI_OBOW": 1, "UESUGI_OTA": 1, "TAKEDA_UEHARA": 1, "IMAGAWA_KAKEGAWA": 2,
    "ODA_SHOBATA": 2, "IMAGAWA_NAGOYA": 1, "ASHIKAGA_KIRIYAMA": 1, "ASHIKAGA_NANAO": 2,
    "ASHIKAGA_SAKURABORA": 1, "ASHIKAGA_GOSEYAMA": 1, "ASHIKAGA_HOJOZU": 1,
    "ASHIKAGA_KUWANOMI": 1, "ROKKAKU_KAMAHABA": 1, "MIYOSHI_SHIBAO": 2,
    "MIYOSHI_YAGAMI": 2, "ASHIKAGA_TAKAYA": 2, "ASHIKAGA_OKISHIO": 2,
    "MIYOSHI_AMAGIRI": 1, "ASHIKAGA_YUZUKI": 1, "OUCHI_YAMABUKI": 2, "OUCHI_TSUWANO": 1,
    "ASHIKAGA_MITSUISHI": 1, "ASHIKAGA_BITCHU_MATSUYAMA": 1, "SHIMAZU_IZAKU": 1,
    "SHIMAZU_OBI": 1, "RYUKYU_NAKAGUSUKU": 2,
}
NO_WALLS = {"ASHIKAGA_KUWANOMI"}


def tier_of(slug: str) -> str:
    spec = FACTIONS[slug]
    if "tier" in spec:
        return spec["tier"]
    k = spec["k"][1]
    return "A" if k >= 40 else "B" if k >= 20 else "C" if k >= 10 else "D"


def field_units(slug: str) -> int:
    """Units beyond the per-city garrison; sqrt compression of the koku scale."""
    k = FACTIONS[slug]["k"]
    if k is None:
        return 2 if slug == "RYUKYU" else 1
    return max(1, round(math.sqrt(k[1]) / 1.5))


def territory_koku() -> dict[str, float]:
    total: dict[str, float] = defaultdict(float)
    for entry in data()["province_allocations"]:
        province = entry["province"]
        if province not in KOKU_1598:
            continue  # Iki / Tsushima have no entry in the survey
        owners = {part["owner_faction"] for part in entry["parts"]}
        shares = SPLIT_SHARES.get(province, {next(iter(owners)): 1.0} if len(owners) == 1 else None)
        if shares is None or set(shares) != owners or abs(sum(shares.values()) - 1) > 1e-9:
            raise ValueError(f"province split not defined: {province}")
        for owner, share in shares.items():
            total[owner] += KOKU_1598[province] * share
    return dict(total)


def read_map(km: int) -> dict:
    """Per-city land statistics from the generated Civ6Map file."""
    manifest = next(m for m in json.loads((MAPS / "manifest.json").read_text(encoding="utf-8"))
                    if m["km_per_tile"] == km)
    width, height = manifest["width"], manifest["height"]
    grid = g.HexGrid(km, width, height, 0.0, 0.0)  # only neighbours()/index() are used
    with tempfile.TemporaryDirectory() as tmp:
        copy = Path(tmp) / "map.db"
        shutil.copy(MAPS / f"JAC_Japan_{km}km.Civ6Map", copy)
        db = sqlite3.connect(copy)
        terrain = dict(db.execute("SELECT ID, TerrainType FROM Plots"))
        feature = dict(db.execute("SELECT ID, FeatureType FROM PlotFeatures"))
        resource = dict(db.execute("SELECT ID, ResourceType FROM PlotResources"))
        owners = db.execute("SELECT ID, Owner, CityOwner FROM PlotOwners").fetchall()
        city_rows = db.execute("SELECT Plot, Owner FROM Cities").fetchall()
        db.close()
    water = {"TERRAIN_COAST", "TERRAIN_OCEAN"}
    by_city: dict[int, dict] = {}
    for plot, _owner, city in owners:
        rec = by_city.setdefault(city, Counter())
        rec["tiles"] += 1
        t = terrain[plot]
        if t.endswith("MOUNTAIN"):
            rec["mountain"] += 1
        elif t.endswith("HILLS"):
            rec["hills"] += 1
        if feature.get(plot) == "FEATURE_FOREST":
            rec["forest"] += 1
        if plot in resource:
            rec["res:" + resource[plot]] += 1
    out = {}
    for plot, owner in city_rows:
        col, row = plot % width, plot // width
        neighbours = [grid.index(c, r) for c, r in grid.neighbours(col, row)]
        out[plot] = dict(rec=by_city.get(plot, Counter()), owner=owner,
                         coastal=any(terrain[n] in water for n in neighbours),
                         neighbours=neighbours, terrain=terrain, city=plot)
    # Free tiles for extra units: owned, not mountain, within 2 tiles of the centre.
    # Civ6 allows one military unit per tile (see the research doc), so units
    # beyond the garrison need their own tiles.
    for plot, info in out.items():
        centre = (plot % width, plot // width)
        info["near_slots"] = sum(
            1 for p, _o, city in owners
            if city == plot and p != plot and not info["terrain"][p].endswith("MOUNTAIN")
            and g.hex_distance(centre, (p % width, p // width)) <= 2)
        del info["neighbours"], info["terrain"]
    return {"manifest": manifest, "cities": out}


def city_records() -> list[dict]:
    manifest = next(m for m in json.loads((MAPS / "manifest.json").read_text(encoding="utf-8"))
                    if m["km_per_tile"] == 15)
    return manifest["cities"]


def distribute_units(slug: str, plans: list[dict], extra: list[str]) -> list[str]:
    """Place extra units on free tiles within 2 of a city, capital first. Returns unplaced units."""
    for plan in sorted(plans, key=lambda p: (not p["capital"], -p["near_slots_15km"])):
        room = plan["near_slots_15km"] - sum(1 for u in plan["units"] if u["where"] == "near")
        while extra and room > 0:
            plan["units"].append({"type": extra.pop(0), "where": "near"})
            room -= 1
    return extra


def build() -> dict:
    cities15 = read_map(15)["cities"]
    cities20 = read_map(20)["cities"]
    k_terr = territory_koku()
    tiles_by_owner = Counter()
    for info in cities15.values():
        tiles_by_owner[g.SEATS[info["owner"]].slug] += info["rec"]["tiles"]
    k_sum = sum(f["k"][1] for f in FACTIONS.values() if f["k"])
    tile_total = sum(tiles_by_owner.values())

    # 20 km plots differ; match cities by id through the 20 km manifest
    manifest20 = next(m for m in json.loads((MAPS / "manifest.json").read_text(encoding="utf-8"))
                      if m["km_per_tile"] == 20)
    plot20 = {c["id"]: c["plot"] for c in manifest20["cities"]}

    factions_out, city_plans = [], []
    for seat in g.SEATS:
        slug, spec = seat.slug, FACTIONS[seat.slug]
        tier = tier_of(slug)
        own = [c for c in city_records() if c["owner"] == slug]
        plans = []
        for city in sorted(own, key=lambda c: (not c["capital"], c["research_priority"])):
            info15, info20 = cities15[city["plot"]], cities20[plot20[city["id"]]]
            importance = 3 if city["capital"] else IMPORTANCE[city["id"]]
            target = CAPITAL_POP[tier] + (spec["cap_bonus"] if city["capital"] else 0) \
                + IMPORTANCE_POP_DELTA[importance]
            target = max(2, target)
            pops = {}
            for label, info in (("15km", info15), ("20km", info20)):
                rec = info["rec"]
                land = rec["tiles"] - 1 - rec["mountain"]
                pops[label] = max(2, min(target, land))
            pop = pops["15km"]
            # districts: one per 3 citizens above 2, limited by free tiles
            rec = info15["rec"]
            land = rec["tiles"] - 1 - rec["mountain"]
            n_districts = 0 if pop < 3 else 1 if pop < 6 else 2 if pop < 9 else 3
            n_districts = max(0, min(n_districts, land - pop))
            if not city["capital"]:
                n_districts = min(n_districts, 1 if importance >= 2 else 0)
            scores = sorted(TRAIT_NAMES, key=lambda t: (-spec["traits"][TRAIT_NAMES.index(t)],
                                                        TRAIT_ORDER.index(t)))
            districts = []
            for trait in scores:
                if len(districts) >= n_districts or spec["traits"][TRAIT_NAMES.index(trait)] == 0:
                    break
                dtype = TRAIT_DISTRICT[trait]
                if dtype == "DISTRICT_HARBOR" and not info15["coastal"]:
                    continue
                districts.append({"district": dtype, "buildings": [DISTRICT_BUILDING[dtype]]})
            if city["capital"] and spec["traits"][TRAIT_NAMES.index("faith")] == 3:
                for d in districts:
                    if d["district"] == "DISTRICT_HOLY_SITE":
                        d["buildings"].append("BUILDING_TEMPLE")
            buildings = []
            if city["capital"]:
                buildings.append("BUILDING_PALACE")
            if (city["capital"] or importance >= 2) and city["id"] not in NO_WALLS:
                buildings.append("BUILDING_WALLS")
            if pop >= 4:
                buildings.append("BUILDING_GRANARY")
            # units: garrison in the city centre, extras placed later
            if city["capital"]:
                garrison = ["UNIT_SPEARMAN"] + (["UNIT_ARCHER"] if tier in "AB" else [])
            else:
                garrison = ["UNIT_SPEARMAN"] if importance >= 2 else ["UNIT_WARRIOR"]
            units = [{"type": garrison[0], "where": "center"}]
            extra_local = garrison[1:]
            # improvements
            flat = max(0, rec["tiles"] - 1 - rec["mountain"] - rec["hills"] - rec["forest"])
            worked = max(0, pop - len(districts))
            farms = min(flat, worked)
            mines = min(rec["hills"], rec.get("res:RESOURCE_IRON", 0) + worked - farms)
            plans.append({
                "id": city["id"], "name": city["name"], "capital": city["capital"],
                "importance": importance, "target_pop": target,
                "tiles_15km": rec["tiles"], "tiles_20km": info20["rec"]["tiles"],
                "pop_15km": pops["15km"], "pop_20km": pops["20km"],
                "option2_pop_15km": max(2, min(4, land)),
                "coastal_centre": info15["coastal"],
                "near_slots_15km": info15["near_slots"],
                "city_buildings": buildings, "districts": districts,
                "units": units, "_extra": extra_local,
                "improvements": {"IMPROVEMENT_FARM": farms, "IMPROVEMENT_MINE": mines},
                "unverified_improvements": {
                    "IMPROVEMENT_PASTURE": rec.get("res:RESOURCE_HORSES", 0),
                    "IMPROVEMENT_PLANTATION": rec.get("res:RESOURCE_SILK", 0) + rec.get("res:RESOURCE_TEA", 0)},
            })
        # field units: 50% spearman, 30% archer, rest warrior; placed on free tiles near a city
        n_field = field_units(slug)
        order = ["UNIT_SPEARMAN", "UNIT_ARCHER", "UNIT_WARRIOR", "UNIT_SPEARMAN", "UNIT_ARCHER",
                 "UNIT_SPEARMAN", "UNIT_WARRIOR", "UNIT_SPEARMAN"]
        extra = [u for p in plans for u in p.pop("_extra")] + [order[i % len(order)] for i in range(n_field)]
        for p in plans:
            p.pop("_extra", None)
        unplaced = distribute_units(slug, plans, extra)
        upkeep = sum(UNIT_UPKEEP[u["type"]] for p in plans for u in p["units"])
        upkeep += sum(BUILDING_UPKEEP.get(b, 0) for p in plans for d in p["districts"] for b in d["buildings"])
        upkeep += sum(DISTRICT_UPKEEP.get(d["district"], 0) for p in plans for d in p["districts"])
        for p in plans:
            p.pop("near_slots_15km")
        k = spec["k"]
        factions_out.append({
            "id": slug, "name": seat.ja, "tier": tier, "confidence": spec["conf"],
            "direct_koku_man": None if k is None else {"low": k[0], "mid": k[1], "high": k[2]},
            "direct_koku_basis": spec["basis"],
            "mobilization_troops": None if k is None else {
                "low": k[0] * TROOPS_PER_MAN_KOKU, "mid": k[1] * TROOPS_PER_MAN_KOKU,
                "high": k[2] * TROOPS_PER_MAN_KOKU},
            "territory_koku_1598_man": round(k_terr.get(slug, 0) / 10000, 1),
            "tiles_15km": tiles_by_owner[slug],
            "tile_share_pct": round(100 * tiles_by_owner[slug] / tile_total, 1),
            "direct_koku_share_pct": None if k is None else round(100 * k[1] / k_sum, 1),
            "traits": dict(zip(TRAIT_NAMES, spec["traits"])),
            "cities": len(plans), "pop_total_15km": sum(p["pop_15km"] for p in plans),
            "units_total": sum(len(p["units"]) for p in plans),
            "option2_pop_total_15km": sum(p["option2_pop_15km"] for p in plans),
            "option2_units_total": len(plans) + 3,
            "units_unplaced": unplaced,
            "gold_upkeep_per_turn": upkeep,
            "plan": plans,
        })
        city_plans.extend(plans)
    return {
        "_schema": {
            "status": "調査案。オーナー確認前(Q28)。SSOT ではない",
            "generated_by": "tools/japan_map_scale/prosperity.py",
            "koku_unit": "10,000 koku (万石) on the Keichou 3 (1598) scale",
            "option1": "各家の1534年規模に合わせる案。plan[] が都市ごとの開始状態",
            "option2": "全勢力に同じ量を置く案。option2_rule を参照",
        },
        "option2_rule": {
            "per_city": {"pop": 4, "city_buildings": ["BUILDING_WALLS", "BUILDING_GRANARY"],
                         "garrison": ["UNIT_SPEARMAN"]},
            "per_faction_extra_units": ["UNIT_SPEARMAN", "UNIT_ARCHER", "UNIT_WARRIOR"],
            "pop_cap": "都市の所有陸タイル数-1-山岳数(オプション1と同じ上限)",
        },
        "koku_1598": KOKU_1598,
        "split_shares": SPLIT_SHARES,
        "factions": factions_out,
    }


def main() -> None:
    OUT.write_text(json.dumps(build(), ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
