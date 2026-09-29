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
    "ODA": dict(k=(4, 8, 12), conf="中低", cap_bonus=0, traits=(1, 3, 2, 1, 0, 0),
                basis="尾張の海東・海西郡の弾正忠家。勝幡城と津島・熱田の湊町の矢銭で伸びる途中。石高は小さいが商業は大きい"
                      "(1540年に伊勢遷宮へ銭700貫文、1543年に内裏修理料4000貫文を献上。Wikipedia「織田信秀」)"),
    "DATE": dict(k=(15, 27, 35), conf="中低", cap_bonus=0, traits=(2, 1, 0, 1, 1, 0),
                 basis="伊達・信夫・置賜の直轄と近隣の従属国衆。陸奥北部と出羽の大半は独立性が強く数えない"
                       "(1532年に桑折西山城へ移り、1533年『蔵方之掟』、1535年『棟役日記』、1538年『御段銭帳』の徴税台帳を作る。"
                       "Wikipedia「伊達稙宗」。台帳の数値は未確認)"),
    "ASHINA": dict(k=(15, 25, 32), conf="低", cap_bonus=0, traits=(1, 1, 0, 2, 0, 0),
                   basis="会津4郡と仙道南部の一族・従属国衆。出羽の仮割当は数えない"
                         "(当主は盛舜。「会津守護」を自称。Wikipedia「蘆名氏」。数値の史料は見つからず推定のまま)"),
    "HOJO": dict(k=(35, 43, 52), conf="中", cap_bonus=0, traits=(3, 2, 1, 0, 0, 0),
                 basis="相模+伊豆(26.4万)と武蔵南部の江戸周辺(約25%=16.7万)。下総は政治的連携で数えない"),
    "YAMANOUCHI_UESUGI": dict(k=(40, 50, 60), conf="低", cap_bonus=0, traits=(2, 1, 0, 1, 0, 0),
                              basis="上野(49.6万)と武蔵北部(50万)の名目領のうち約半分が実効。1530年代は内紛と衰退の途中"
                                    "(1531年の関東享禄の内乱で憲政が家督。1546年河越で3,000人余を失う。Wikipedia「上杉憲政」)"),
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
    "ASHIKAGA": dict(k=(8, 15, 25), conf="中低", cap_bonus=2, traits=(0, 3, 0, 3, 0, 3),
                     basis="将軍家の直轄は山城の御料所程度。石高は小さいが京都の人口・寺社・商業と威信が最大"
                           "(幕府財政は御料所が主で小規模。Wikipedia「室町幕府」。義晴は1534年6月まで桑実寺、のち坂本。"
                           "Wikipedia「足利義晴」)"),
    "MIYOSHI": dict(k=(4, 8, 15), conf="低", cap_bonus=0, traits=(2, 2, 3, 0, 0, 0),
                    basis="阿波三好郡の三好家(元長は1532年没、長慶は12歳)。守護細川氏の阿波・讃岐・淡路は数えない"
                          "(元長の死は Wikipedia「三好元長」。所領の数値の史料は見つからず推定のまま)"),
    "ROKKAKU": dict(k=(30, 40, 45), conf="中", cap_bonus=0, traits=(2, 2, 1, 1, 0, 0),
                    basis="近江(77.5万)のうち南近江の約50%。北近江は浅井氏が半独立"),
    "HONGANJI": dict(k=(10, 18, 30), conf="低", cap_bonus=1, traits=(0, 3, 2, 3, 0, 0),
                     basis="石山御坊・寺内町と加賀の一向一揆領(35.6万の約50%)。石高より門徒・寺内町・資金が力"
                           "(1532年に山科本願寺を焼かれ、石山は濠・土居で囲んだ寺内町。細川晴元との戦いは1535年11月まで。"
                           "Wikipedia「石山本願寺」「証如」)"),
    "OUCHI": dict(k=(55, 70, 85), conf="中", cap_bonus=0, traits=(1, 3, 3, 2, 3, 3),
                  basis="周防+長門(29.9万)、石見の約70%、筑前の約70%、豊前の約80%。山口と博多、石見銀山(1533年に灰吹法)"),
    "AMAGO": dict(k=(30, 40, 50), conf="中低", cap_bonus=0, traits=(3, 0, 1, 0, 3, 0),
                  basis="出雲+隠岐+伯耆(29.9万)と美作・石見・備後・安芸の一部。たたら製鉄の産地。影響は中国11か国に及ぶと伝わる"),
    "MORI": dict(k=(2, 3, 5), conf="中低", cap_bonus=0, traits=(2, 0, 0, 0, 0, 0),
                 basis="吉田郡山周辺の一国衆。大内方の従属国衆。元就の元の所領は多治比300貫、大内義興から可部700貫・温科300貫・"
                       "深川300貫・久村70貫の恩賞(Wikipedia「毛利元就」)。1540年の郡山籠城は徴集兵込み3,000。"
                       "本家の貫高は見つからず、国衆連合を含めて推定"),
    "CHOSOKABE": dict(k=(0.3, 0.45, 0.6), conf="中低", cap_bonus=0, traits=(2, 0, 1, 0, 0, 0),
                      basis="土佐長岡郡岡豊と周辺。土佐七雄で最小の長岡郡3000貫(1貫=1〜2石で0.3〜0.6万石)。"
                            "本山・吉良・安芸・津野は各5000貫、一条氏は幡多郡16000貫(Wikipedia「土佐七雄」)"),
    "OTOMO": dict(k=(40, 48, 60), conf="中低", cap_bonus=0, traits=(1, 3, 3, 2, 0, 0),
                  basis="豊後(41.8万)の約85%、筑後の約40%、肥後の一部。府内は明との貿易港を持つ"),
    "SHIMAZU": dict(k=(4, 8, 15), conf="中低", cap_bonus=0, traits=(3, 1, 2, 0, 0, 0),
                    basis="薩摩・大隅・日向南部の名目領(57.6万)のうち貴久(伊作家・相州家)が固めた範囲。1533年に南郷城で初陣、"
                          "1535年に実久が宗家を奪い勝久を追放(Wikipedia「島津貴久」)。宗家と分家が分裂中のため旧案(中12万)から下げた"),
    "RYUKYU": dict(k=(8, 10, 11), conf="中", cap_bonus=0, traits=(0, 3, 3, 0, 0, 0),
                   basis="1609年以後の「琉球国高究帳」で11万石余・沖縄本島8万石(Wikipedia「薩摩藩」、典拠『沖縄県史料 前近代1』)。"
                         "奄美は1571年まで全域支配でないため中10万。那覇の中継貿易で商業・港が主。兵力は数千人規模(Wikipedia「琉球王国」)"),
    "AINU": dict(k=(2, 3, 7), conf="低", cap_bonus=0, traits=(1, 1, 1, 0, 0, 0), koku_equivalent=True,
                 basis="石高なし。人口からの石高相当。江戸時代の記録上の最大は26,800人、1804年以降の調査で約2万人"
                       "(Wikipedia「アイヌ」「江戸時代の日本の人口統計」)。1人1石(吉田東伍)〜1石0.28〜0.44人(速水融)で2.7〜9.6万石だが、"
                       "狩猟・交易社会で徴税・動員の仕組みがないため下限側の3万石相当とした(推定)。各コタン・チャシの連合で単一の領域ではない"),
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
KYOTO = "ASHIKAGA_SEAT"  # Muromachi palace, the Ashikaga capital


def tier_of(slug: str) -> str:
    k = FACTIONS[slug]["k"][1]
    return "A" if k >= 40 else "B" if k >= 20 else "C" if k >= 10 else "D"


def field_units(slug: str) -> int:
    """Units beyond the per-city garrison; sqrt compression of the koku scale."""
    return max(1, round(math.sqrt(FACTIONS[slug]["k"][1]) / 1.5))


# Starting treasury (owner decision 5b: koku differences go into assets, not land).
# Official maps set it with PlayerAttributes(ID, 'Treasury', 'Balance', ...):
# DLC/NubiaScenario/Maps/NubiaScenario.Civ6Map (100/50) and
# DLC/WarMachineScenario/Maps/WarMachineMap.Civ6Map (900).
GOLD_PER_MAN_KOKU = 10
GOLD_PER_TRADE = 40
GOLD_FLOOR = 100
KYOTO_GOLD = 100  # owner decision 9a: Kyoto's commerce


def starting_gold(slug: str) -> int:
    spec = FACTIONS[slug]
    gold = GOLD_PER_MAN_KOKU * spec["k"][1] + GOLD_PER_TRADE * spec["traits"][TRAIT_NAMES.index("trade")]
    gold = max(GOLD_FLOOR, int(round(gold / 10)) * 10)
    return gold + (KYOTO_GOLD if slug == "ASHIKAGA" else 0)


def city_plan(slug: str, city: dict, rec: Counter, coastal: bool) -> dict:
    """Population, districts, centre buildings and garrison of one city on one map scale."""
    spec, tier, capital = FACTIONS[slug], tier_of(slug), city["capital"]
    importance = 3 if capital else IMPORTANCE[city["id"]]
    target = max(2, CAPITAL_POP[tier] + (spec["cap_bonus"] if capital else 0)
                 + IMPORTANCE_POP_DELTA[importance])
    land = rec["tiles"] - 1 - rec["mountain"]
    pop = max(2, min(target, land))
    # districts: one per 3 citizens above 2, limited by free tiles
    n_districts = 0 if pop < 3 else 1 if pop < 6 else 2 if pop < 9 else 3
    n_districts = max(0, min(n_districts, land - pop))
    if not capital:
        n_districts = min(n_districts, 1 if importance >= 2 else 0)
    if city["id"] == KYOTO:
        n_districts = 2  # owner decision 9a: temples and commerce regardless of tile count
    scores = sorted(TRAIT_NAMES, key=lambda t: (-spec["traits"][TRAIT_NAMES.index(t)], TRAIT_ORDER.index(t)))
    # Every district the house's traits allow, best first; a map places the first
    # n_districts that fit and falls back to the next ones.
    options = []
    for trait in scores:
        dtype = TRAIT_DISTRICT[trait]
        if spec["traits"][TRAIT_NAMES.index(trait)] == 0 or (dtype == "DISTRICT_HARBOR" and not coastal):
            continue
        buildings = [DISTRICT_BUILDING[dtype]]
        if capital and dtype == "DISTRICT_HOLY_SITE" and spec["traits"][TRAIT_NAMES.index("faith")] == 3:
            buildings.append("BUILDING_TEMPLE")
        options.append({"district": dtype, "buildings": buildings})
    buildings = ["BUILDING_PALACE"] if capital else []
    if (capital or importance >= 2) and city["id"] not in NO_WALLS:
        buildings.append("BUILDING_WALLS")
    if pop >= 4:
        buildings.append("BUILDING_GRANARY")
    if capital:
        garrison = ["UNIT_SPEARMAN"] + (["UNIT_ARCHER"] if tier in "AB" else [])
    else:
        garrison = ["UNIT_SPEARMAN"] if importance >= 2 else ["UNIT_WARRIOR"]
    return {"importance": importance, "target_pop": target, "pop": pop, "land": land,
            "districts": options[:n_districts], "district_options": options, "n_districts": n_districts,
            "city_buildings": buildings, "garrison": garrison}


def faction_reserves(slug: str, plans: list[dict]) -> list[str]:
    """Units placed near the cities: the capitals' second garrison unit, then field units."""
    order = ["UNIT_SPEARMAN", "UNIT_ARCHER", "UNIT_WARRIOR", "UNIT_SPEARMAN", "UNIT_ARCHER",
             "UNIT_SPEARMAN", "UNIT_WARRIOR", "UNIT_SPEARMAN"]
    return ([u for p in plans for u in p["garrison"][1:]]
            + [order[i % len(order)] for i in range(field_units(slug))])


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


WATER = {"TERRAIN_COAST", "TERRAIN_OCEAN"}


def read_db(db: sqlite3.Connection, width: int, height: int, km: int) -> dict[int, dict]:
    """Per-city land statistics of a historical Civ6Map, keyed by city-centre plot.

    Only land plots count: a harbour district adds an owned water plot.
    """
    grid = g.HexGrid(km, width, height, 0.0, 0.0)  # only neighbours()/index() are used
    terrain = dict(db.execute("SELECT ID, TerrainType FROM Plots"))
    feature = dict(db.execute("SELECT ID, FeatureType FROM PlotFeatures"))
    resource = dict(db.execute("SELECT ID, ResourceType FROM PlotResources"))
    owners = [row for row in db.execute("SELECT ID, Owner, CityOwner FROM PlotOwners ORDER BY ID")
              if terrain[row[0]] not in WATER]
    by_city: dict[int, Counter] = {}
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
    for plot, owner in db.execute("SELECT Plot, Owner FROM Cities ORDER BY Plot"):
        col, row = plot % width, plot // width
        neighbours = [grid.index(c, r) for c, r in grid.neighbours(col, row)]
        out[plot] = dict(rec=by_city.get(plot, Counter()), owner=owner,
                         coastal=any(terrain[n] in WATER for n in neighbours))
    # Free tiles for extra units: owned, not mountain, within 2 tiles of the centre.
    # Civ6 allows one military unit per tile (see the research doc), so units
    # beyond the garrison need their own tiles.
    for plot, info in out.items():
        centre = (plot % width, plot // width)
        info["near_slots"] = sum(
            1 for p, _o, city in owners
            if city == plot and p != plot and not terrain[p].endswith("MOUNTAIN")
            and g.hex_distance(centre, (p % width, p // width)) <= 2)
    return out


def manifest_item(km: int) -> dict:
    return next(m for m in json.loads((MAPS / "manifest.json").read_text(encoding="utf-8"))
                if m["km_per_tile"] == km)


def read_map(km: int) -> dict[int, dict]:
    manifest = manifest_item(km)
    with tempfile.TemporaryDirectory() as tmp:
        copy = Path(tmp) / "map.db"
        shutil.copy(MAPS / f"JAC_Japan_{km}km.Civ6Map", copy)
        db = sqlite3.connect(copy)
        try:
            return read_db(db, manifest["width"], manifest["height"], km)
        finally:
            db.close()


def distribute_units(plans: list[dict], extra: list[str]) -> list[str]:
    """Place extra units on free tiles within 2 of a city, capital first. Returns unplaced units."""
    for plan in sorted(plans, key=lambda p: (not p["capital"], -p["near_slots_15km"])):
        room = plan["near_slots_15km"] - sum(1 for u in plan["units"] if u["where"] == "near")
        while extra and room > 0:
            plan["units"].append({"type": extra.pop(0), "where": "near"})
            room -= 1
    return extra


def upkeep_of(units: list[str], districts: list[dict]) -> int:
    return (sum(UNIT_UPKEEP[u] for u in units)
            + sum(BUILDING_UPKEEP.get(b, 0) for d in districts for b in d["buildings"])
            + sum(DISTRICT_UPKEEP.get(d["district"], 0) for d in districts))


def build() -> dict:
    cities15 = read_map(15)
    cities20 = read_map(20)
    k_terr = territory_koku()
    tiles_by_owner = Counter()
    for info in cities15.values():
        tiles_by_owner[g.SEATS[info["owner"]].slug] += info["rec"]["tiles"]
    k_sum = sum(f["k"][1] for f in FACTIONS.values() if not f.get("koku_equivalent"))
    tile_total = sum(tiles_by_owner.values())
    records15 = manifest_item(15)["cities"]
    plot20 = {c["id"]: c["plot"] for c in manifest_item(20)["cities"]}

    factions_out = []
    for seat in g.SEATS:
        slug, spec = seat.slug, FACTIONS[seat.slug]
        own = [c for c in records15 if c["owner"] == slug]
        plans = []
        for city in sorted(own, key=lambda c: (not c["capital"], c["research_priority"])):
            info15, info20 = cities15[city["plot"]], cities20[plot20[city["id"]]]
            plan = city_plan(slug, city, info15["rec"], info15["coastal"])
            rec = info15["rec"]
            flat = max(0, rec["tiles"] - 1 - rec["mountain"] - rec["hills"] - rec["forest"])
            worked = max(0, plan["pop"] - len(plan["districts"]))
            farms = min(flat, worked)
            mines = min(rec["hills"], rec.get("res:RESOURCE_IRON", 0) + worked - farms)
            plans.append({
                "id": city["id"], "name": city["name"], "capital": city["capital"],
                "importance": plan["importance"], "target_pop": plan["target_pop"],
                "tiles_15km": rec["tiles"], "tiles_20km": info20["rec"]["tiles"],
                "pop_15km": plan["pop"],
                "pop_20km": city_plan(slug, city, info20["rec"], info20["coastal"])["pop"],
                "option2_pop_15km": max(2, min(4, plan["land"])),
                "coastal_centre": info15["coastal"],
                "near_slots_15km": info15["near_slots"],
                "city_buildings": plan["city_buildings"], "districts": plan["districts"],
                "units": [{"type": plan["garrison"][0], "where": "center"}], "_garrison": plan["garrison"],
                "improvements": {"IMPROVEMENT_FARM": farms, "IMPROVEMENT_MINE": mines},
                "unverified_improvements": {
                    "IMPROVEMENT_PASTURE": rec.get("res:RESOURCE_HORSES", 0),
                    "IMPROVEMENT_PLANTATION": rec.get("res:RESOURCE_SILK", 0) + rec.get("res:RESOURCE_TEA", 0)},
            })
        extra = faction_reserves(slug, [{"garrison": p.pop("_garrison")} for p in plans])
        unplaced = distribute_units(plans, extra)
        upkeep = sum(upkeep_of([u["type"] for u in p["units"]], p["districts"]) for p in plans)
        for p in plans:
            p.pop("near_slots_15km")
        k = spec["k"]
        factions_out.append({
            "id": slug, "name": seat.ja, "tier": tier_of(slug), "confidence": spec["conf"],
            "direct_koku_man": {"low": k[0], "mid": k[1], "high": k[2]},
            "koku_equivalent": bool(spec.get("koku_equivalent")),
            "direct_koku_basis": spec["basis"],
            "mobilization_troops": {"low": round(k[0] * TROOPS_PER_MAN_KOKU),
                                    "mid": round(k[1] * TROOPS_PER_MAN_KOKU),
                                    "high": round(k[2] * TROOPS_PER_MAN_KOKU)},
            "territory_koku_1598_man": round(k_terr.get(slug, 0) / 10000, 1),
            "tiles_15km": tiles_by_owner[slug],
            "tile_share_pct": round(100 * tiles_by_owner[slug] / tile_total, 1),
            "direct_koku_share_pct": None if spec.get("koku_equivalent") else round(100 * k[1] / k_sum, 1),
            "traits": dict(zip(TRAIT_NAMES, spec["traits"])),
            "cities": len(plans), "pop_total_15km": sum(p["pop_15km"] for p in plans),
            "units_total": sum(len(p["units"]) for p in plans),
            "option2_pop_total_15km": sum(p["option2_pop_15km"] for p in plans),
            "option2_units_total": len(plans) + 3,
            "units_unplaced": unplaced,
            "gold_upkeep_per_turn": upkeep,
            "starting_gold": starting_gold(slug),
            "plan": plans,
        })
    return {
        "_schema": {
            "status": "Q28 はオプション①に決定(SSOT 3.1)。地図への反映は prosper_start.py(documents/poc/japan-prosperous-start.md)",
            "generated_by": "tools/japan_map_scale/prosperity.py",
            "koku_unit": "10,000 koku (万石) on the Keichou 3 (1598) scale; koku_equivalent=true is derived from population",
            "option1": "各家の1534年規模に合わせる案(採用)。plan[] は15km 地図の目安。実際の配置は地図ごとに prosper_start.py が決める",
            "option2": "全勢力に同じ量を置く案(不採用)。option2_rule を参照",
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
