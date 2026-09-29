"""Generate the Banseiki-only Sengoku tree rows from the reviewed design draft.

The research document remains the source for IDs, prerequisites, costs, and
positions.  This generator adds provisional localization and two exact vanilla
boost triggers.  Run from any directory with ``python tools/generate_sengoku_trees.py``.
"""

from __future__ import annotations

import argparse
import re
from dataclasses import dataclass
from pathlib import Path
from xml.sax.saxutils import escape


ROOT = Path(__file__).resolve().parents[1]
DESIGN = ROOT / "documents/research/sengoku-tech-civic-tree.md"
MOD = ROOT / "poc/JAC_PoC_SengokuTrees"
TECH_COSTS = (40, 65, 90, 125, 160, 205, 255, 320, 400, 490, 600, 730)
CIVIC_COSTS = (30, 45, 65, 90, 125, 165, 215, 275, 350, 430, 530, 650)
ERAS = ("ERA_ANCIENT", "ERA_CLASSICAL", "ERA_MEDIEVAL", "ERA_RENAISSANCE")
ERA_NAMES = (
    ("領国の基盤", "Foundations of the Domain", "Основы владения"),
    ("技術交流と領国競争", "Exchange and Rival Domains", "Обмен знаниями и борьба владений"),
    ("統一への広域戦", "Campaigns for Unification", "Походы за объединение"),
    ("統一事業", "The Work of Unification", "Объединение страны"),
)

# SSOT 3.2: names tied to one person or house.  The database always points at
# the generic text; UI/JAC_HouseNamesData_Sengoku.lua swaps in the house text
# (see documents/poc/house-names.md).  Each entry is
# (generic tag, house tag, owner civilization, (ja, en, ru) house text).
# The Toyotomi are not a faction, so the owner of the Taiko survey is a
# provisional choice (Oda, the house Hideyoshi served); see the owner question.
HOUSE_NAMES = (
    ("LOC_CIVIC_JAC_SENGOKU_TAIKO_LAND_SURVEY_NAME", "LOC_CIVIC_JAC_SENGOKU_TAIKO_LAND_SURVEY_NAME_HOUSE",
     "CIVILIZATION_JAC_ODA", ("太閤検地", "Taikō Land Survey", "Тайко-кэнти (межевание земель)")),
    ("LOC_JAC_SENGOKU_ERA_3_NAME", "LOC_JAC_SENGOKU_ERA_3_NAME_HOUSE",
     "CIVILIZATION_JAC_ODA", ("天下布武と広域戦", "Tenka Fubu and Wide-Area Campaigns", "«Тэнка фубу» и походы широкого масштаба")),
    ("LOC_JAC_SENGOKU_ERA_3_DESCRIPTION", "LOC_JAC_SENGOKU_ERA_3_DESCRIPTION_HOUSE",
     "CIVILIZATION_JAC_ODA", ("天下布武と広域戦", "Tenka Fubu and Wide-Area Campaigns", "«Тэнка фубу» и походы широкого масштаба")),
)

# Draft translations. The owners have not approved terminology or effect values.
RU_NAMES = (
    "Ирригационные работы", "Прибрежное мореходство", "Ковка железа", "Земляные укрепления",
    "Дороги и вьючные обозы", "Поиск рудных жил", "Хранение зерна", "Планировка горной крепости",
    "Купелирование", "Строительство деревянных судов", "Оружие и доспехи", "Знакомство с огнестрельным оружием",
    "Изготовление фитильных ружей", "Управление рудниками", "Укрощение рек", "Ряды оборонительных рвов",
    "Лоцманы и гавани", "Купальни и лечение", "Сеть снабжения", "Строительство военных судов",
    "Каменные стены", "Массовое производство ружей", "Пули и порох", "Снабжение через порты",
    "Угловые ворота", "Полевые укрепления", "Атакэбунэ", "Осадная артиллерия",
    "Главная башня и каменное основание", "Региональное снабжение", "Единые меры для межевания", "Внешние укрепления",
    "Правила дома", "Союзы местных землевладельцев", "Святилища и верующие", "Заставы и проезд",
    "Законы владения", "Сельские собрания", "Учёба и записи", "Самоуправление храмовых городов",
    "Горные паломничества", "Реестр воинской повинности", "Клятвы и союзы", "Местные союзы и разведка",
    "Советы портовых купцов", "Чайные встречи и ценности", "Указы для призамковых городов", "Посланники ко двору и сёгуну",
    "Свободные рынки", "Гарантии храмовых земель", "Посланники к южным варварам", "Межевание владений",
    "Организация стрелков", "Чайная дипломатия", "Союзные походы", "Посредничество в перемириях",
    "Общее межевание земель", "Грамоты с красной печатью", "Изъятие мечей", "Разделение воинов и земледельцев",
    "Управление портами", "Подтверждение земельных прав",
)

NODE_RE = re.compile(
    r"^\| (?P<short>[TC]\d\d) (?P<ja>.*?) / (?P<en>.*?)<br>`(?P<id>\w+)`"
    r" \| (?P<prereqs>.*?) \| (?P<column>\d+)/(?P<row>-?\d+)・(?P<band>\w\d)"
    r" \| (?P<unlock>.*?) \| (?P<boost>.*?) \|"
)


@dataclass(frozen=True)
class Node:
    short: str
    id: str
    ja: str
    en: str
    ru: str
    prereqs: tuple[str, ...]
    column: int
    row: int
    cost: int
    era: str
    unlock: str
    proposed_boost: str


def read_design() -> list[Node]:
    lines = DESIGN.read_text(encoding="utf-8").splitlines()
    for label, expected in (("技術費", TECH_COSTS), ("制度費", CIVIC_COSTS)):
        row = next(line for line in lines if line.startswith(f"| {label} |"))
        values = tuple(int(cell.strip()) for cell in row.strip("| ").split("|")[1:])
        if values != expected:
            raise ValueError(f"{label} no longer matches the design's 12 columns: {values}")
    matches = [m for line in lines if (m := NODE_RE.match(line))]
    if len(matches) != 62 or len(RU_NAMES) != 62:
        raise ValueError(f"expected 62 designed nodes and Russian names, got {len(matches)}")
    names = {m["short"]: m["id"] for m in matches}
    nodes = []
    for m, ru in zip(matches, RU_NAMES):
        short = m["short"]
        column = int(m["column"])
        cost = (TECH_COSTS if short.startswith("T") else CIVIC_COSTS)[column - 1]
        prerequisites = () if m["prereqs"] == "—" else tuple(names[p.strip()] for p in m["prereqs"].split(","))
        ja, en = m["ja"], m["en"]
        if short == "C25":  # SSOT 3.2: non-Toyotomi houses must see a generic term.
            ja, en = "広域検地", "Regional Land Survey"
        nodes.append(Node(short, m["id"], ja, en, ru, prerequisites, column,
                          int(m["row"]), cost, ERAS[(column - 1) // 3],
                          m["unlock"], m["boost"]))
    return nodes


def sql_quote(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


BOOSTS = {
    "T09": {
        "class": "BOOST_TRIGGER_IMPROVE_SPECIFIC_RESOURCE",
        "attributes": {"ImprovementType": "IMPROVEMENT_MINE", "ResourceType": "RESOURCE_SILVER"},
        "text": ("銀を採掘する。", "Mine silver.", "Добудьте серебро."),
    },
    "T10": {
        "class": "BOOST_TRIGGER_TRAIN_UNIT",
        "attributes": {"Unit1Type": "UNIT_GALLEY"},
        "text": ("ガレー船を1隻建造する。", "Train a Galley.", "Постройте одну галеру."),
    },
}

# These are official stand-ins wired by Data/JAC_Sengoku_Unlocks.sql.  Every
# proposal absent here remains TODO; even listed proxies keep their original
# Civ6 effects, so the design-specific part of the proposal remains TODO.
OFFICIAL_UNLOCKS = {
    "T01": "IMPROVEMENT_FARM, IMPROVEMENT_PLANTATION, BUILDING_WATER_MILL",
    "T02": "UNIT_GALLEY, IMPROVEMENT_FISHING_BOATS",
    "T03": "UNIT_SWORDSMAN, BUILDING_BARRACKS",
    "T04": "BUILDING_WALLS, DISTRICT_ENCAMPMENT",
    "T05": "UNIT_TRADER",
    "T06": "IMPROVEMENT_MINE, IMPROVEMENT_QUARRY",
    "T07": "BUILDING_GRANARY",
    "T08": "IMPROVEMENT_FORT",
    "T10": "UNIT_QUADRIREME",
    "T11": "BUILDING_ARMORY",
    "T13": "UNIT_MUSKETMAN",
    "T14": "DISTRICT_INDUSTRIAL_ZONE, BUILDING_WORKSHOP",
    "T15": "DISTRICT_AQUEDUCT",
    "T17": "DISTRICT_HARBOR, BUILDING_LIGHTHOUSE",
    "T18": "UNIT_MEDIC",
    "T20": "BUILDING_SHIPYARD",
    "T21": "BUILDING_CASTLE",
    "T26": "UNIT_CATAPULT",
    "T28": "UNIT_BOMBARD",
    "T29": "BUILDING_STAR_FORT",
    "C01": "POLICY_DISCIPLINE, POLICY_URBAN_PLANNING",
    "C03": "DISTRICT_HOLY_SITE, BUILDING_SHRINE",
    "C04": "POLICY_SURVEY",
    "C05": "GOVERNMENT_OLIGARCHY, DISTRICT_GOVERNMENT",
    "C06": "POLICY_ILKUM",
    "C07": "DISTRICT_CAMPUS, BUILDING_LIBRARY",
    "C08": "BUILDING_TEMPLE",
    "C10": "UNIT_HORSEMAN, POLICY_AGOGE",
    "C11": "POLICY_DIPLOMATIC_LEAGUE",
    "C12": "UNIT_SPY",
    "C13": "DISTRICT_COMMERCIAL_HUB, BUILDING_MARKET, GOVERNMENT_MERCHANT_REPUBLIC",
    "C14": "DISTRICT_THEATER, BUILDING_AMPHITHEATER",
    "C16": "DISTRICT_DIPLOMATIC_QUARTER (optional Ethiopia DLC)",
    "C17": "POLICY_CARAVANSARIES",
    "C20": "POLICY_LAND_SURVEYORS",
    "C21": "POLICY_FEUDAL_CONTRACT",
    "C22": "POLICY_CHARISMATIC_LEADER",
    "C23": "POLICY_MANEUVER",
    "C24": "POLICY_MACHIAVELLIANISM",
    "C25": "POLICY_SERFDOM",
    "C26": "POLICY_TOWN_CHARTERS",
    "C28": "GOVERNMENT_MONARCHY, POLICY_PROFESSIONAL_ARMY",
    "C29": "POLICY_TRADE_CONFEDERATION",
}


def _c(name: str) -> str:
    return "CIVIC_JAC_SENGOKU_" + name


def _t(name: str) -> str:
    return "TECH_JAC_SENGOKU_" + name


# Official rows that reference a deleted official tech/civic through ON DELETE CASCADE or
# SET NULL/DEFAULT (01_GameplaySchema.sql, Expansion2_Schema.sql) and would otherwise vanish or
# lose their gate.  Emitted as Data/JAC_Sengoku_Cascade.sql, which the modinfo loads before
# JAC_Sengoku_Unlocks.sql deletes the official trees.  Each entry is (table, WHERE columns,
# SET columns, impact without the remap, basis).  "設計 Cnn" = the design table names the node;
# "暫定" = design is silent, closest era/theme chosen (Industrial+ items go to the last node);
# "決定" = the owner decided the gate (SSOT 3.2).
# tools/sengoku_cascade_audit.py and tests/test_sengoku_cascade.py check the residual losses.
CASCADE_REMAPS = (
    ("DiplomaticActions", {"InitiatorPrereqCivic": "CIVIC_CIVIL_SERVICE"},
     {"InitiatorPrereqCivic": _c("OATHS_AND_ALLIANCES")},
     "同盟7種（通常・軍事・研究・文化・経済・宗教・更新）が行ごと消え、子の DiplomaticActions_XP1 と"
     "DiplomaticStateActions も連鎖して消える。どの家も同盟を結べない", "設計 C11 同盟"),
    ("DiplomaticActions", {"TargetPrereqCivic": "CIVIC_CIVIL_SERVICE"},
     {"TargetPrereqCivic": _c("OATHS_AND_ALLIANCES")}, "同上（相手側の前提）", "設計 C11 同盟"),
    ("DiplomaticActions", {"DiplomaticActionType": "DIPLOACTION_OPEN_BORDERS"},
     {"InitiatorPrereqCivic": _c("PASSES_AND_PASSAGE")}, "国境開放の取引ができない", "暫定 C04 関所・通行"),
    ("DiplomaticActions", {"DiplomaticActionType": "DIPLOACTION_DECLARE_WAR_OF_RETRIBUTION"},
     {"InitiatorPrereqCivic": _c("OATHS_AND_ALLIANCES")}, "開戦事由「報復」が消える", "決定 C11（暫定の対応を維持。SSOT 3.2）"),
    ("DiplomaticActions", {"DiplomaticActionType": ("DIPLOACTION_JOINT_WAR", "DIPLOACTION_THIRD_PARTY_WAR",
                                                    "DIPLOACTION_DECLARE_LIBERATION_WAR")},
     {"InitiatorPrereqCivic": _c("COALITION_CAMPAIGNS")}, "共同戦線・参戦要請・開戦事由「解放」が消える",
     "決定 C23 連合出陣（暫定の対応を維持。SSOT 3.2）"),
    ("DiplomaticActions", {"DiplomaticActionType": "DIPLOACTION_RESIDENT_EMBASSY"},
     {"InitiatorPrereqCivic": _c("COURT_AND_SHOGUNATE_ENVOYS")}, "大使館を置けない", "暫定 C16 使節"),
    ("DiplomaticActions", {"DiplomaticActionType": "DIPLOACTION_DIPLOMATIC_DELEGATION"},
     {"InitiatorObsoleteCivic": _c("COURT_AND_SHOGUNATE_ENVOYS")}, "代表団が消える（公式の参照は旧式化の制度だけ）",
     "暫定 C16 使節（大使館と入れ替わり）"),
    ("DiplomaticActions", {"DiplomaticActionType": "DIPLOACTION_DECLARE_HOLY_WAR"},
     {"InitiatorPrereqCivic": _c("SHRINE_LAND_GUARANTEES")}, "開戦事由「聖戦」が消える", "決定 C18 宗教対立（暫定の対応を維持。SSOT 3.2）"),
    ("DiplomaticActions", {"DiplomaticActionType": ("DIPLOACTION_DECLARE_PROTECTORATE_WAR",
                                                    "DIPLOACTION_DECLARE_RECONQUEST_WAR")},
     {"InitiatorPrereqCivic": _c("MILITARY_SERVICE_LEDGER")}, "開戦事由「保護」「再征服」が消える", "決定 C10（暫定の対応を維持。SSOT 3.2）"),
    ("DiplomaticActions", {"DiplomaticActionType": ("DIPLOACTION_DECLARE_COLONIAL_WAR",
                                                    "DIPLOACTION_DECLARE_TERRITORIAL_WAR",
                                                    "DIPLOACTION_DECLARE_IDEOLOGICAL_WAR")},
     {"InitiatorPrereqCivic": _c("LAND_TENURE_CONFIRMATION")}, "近代以降の開戦事由3種が消える",
     "決定 最終C30（暫定の対応を維持。SSOT 3.2）"),
    ("CivicModifiers", {"CivicType": "CIVIC_EARLY_EMPIRE", "ModifierId": "CIVIC_ENFORCE_BORDERS"},
     {"CivicType": _c("PASSES_AND_PASSAGE")}, "国境が開いたままになり、他家のUnitが領内へ自由に入れる", "暫定 C04 関所"),
    ("CivicModifiers", {"CivicType": "CIVIC_FOREIGN_TRADE", "ModifierId": "CIVIC_GRANT_ONE_TRADE_ROUTE"},
     {"CivicType": _c("PASSES_AND_PASSAGE")}, "交易路枠が商業区・港まで0で、T05の交易商を作れない",
     "暫定 C04（T05と同じ時期）"),
    ("CivicModifiers", {"CivicType": "CIVIC_MILITARY_TRADITION", "ModifierId": "CIVIC_GRANT_COMBAT_ADJACENCY_BONUS"},
     {"CivicType": _c("MILITARY_SERVICE_LEDGER")}, "側面攻撃・援護ボーナスが無い", "暫定 C10"),
    ("CivicModifiers", {"CivicType": "CIVIC_DIPLOMATIC_SERVICE", "ModifierId": "CIVIC_GRANT_SPY"},
     {"CivicType": _c("LOCAL_LEAGUES_AND_INTELLIGENCE")}, "スパイ枠が0で、C12で解放したスパイを作れない",
     "設計 C12 諜報"),
    ("TechnologyModifiers", {"TechnologyType": "TECH_MATHEMATICS", "ModifierId": "MATHEMATICS_ADJUST_SEA_MOVEMENT"},
     {"TechnologyType": _t("WOODEN_SHIPBUILDING")}, "艦船の移動力+1が無い", "暫定 T10 和船"),
    ("TechnologyModifiers", {"TechnologyType": "TECH_CARTOGRAPHY", "ModifierId": "CARTOGRAPHY_GRANT_OCEAN_NAVIGATION"},
     {"TechnologyType": _t("ATAKEBUNE")}, "外洋タイルへ入れない", "暫定 T27 大型軍船"),
    ("TechnologyModifiers", {"TechnologyType": "TECH_PRINTING", "ModifierId": "PRINTING_ADD_DIPLO_VISIBILITY"},
     {"TechnologyType": _t("SURVEY_STANDARDS")}, "外交上の可視度+1が無い", "暫定 T31"),
    ("DiplomaticVisibilitySources", {"VisibilitySourceType": "SOURCE_TECH"},
     {"PrereqTech": _t("SURVEY_STANDARDS")}, "同上の可視度の出所が消える", "暫定 T31（上と対）"),
    ("Improvement_BonusYieldChanges", {"ImprovementType": "IMPROVEMENT_MINE", "PrereqTech": "TECH_APPRENTICESHIP"},
     {"PrereqTech": _t("MINE_MANAGEMENT")}, "鉱山の生産力+1が無い", "設計 T14 鉱山産出"),
    ("Improvement_BonusYieldChanges", {"ImprovementType": "IMPROVEMENT_QUARRY", "PrereqTech": "TECH_GUNPOWDER"},
     {"PrereqTech": _t("KEEP_AND_STONE_BASE")}, "採石場の生産力+1が無い", "暫定 T29 石垣"),
    ("Improvement_BonusYieldChanges", {"ImprovementType": "IMPROVEMENT_FISHING_BOATS", "PrereqTech": "TECH_CARTOGRAPHY"},
     {"PrereqTech": _t("HARBOR_PILOTS")}, "漁船のゴールド+2が無い", "暫定 T17 港"),
    ("Improvement_BonusYieldChanges", {"ImprovementType": "IMPROVEMENT_PLANTATION", "PrereqCivic": "CIVIC_FEUDALISM"},
     {"PrereqCivic": _c("DOMAIN_LAND_SURVEYS")}, "プランテーションの食料+1が無い", "暫定 C20（封建制の代理）"),
    ("Improvement_ValidTerrains", {"ImprovementType": "IMPROVEMENT_FARM", "PrereqCivic": "CIVIC_CIVIL_ENGINEERING"},
     {"PrereqCivic": None, "PrereqTech": _t("IRRIGATION_WORKS")}, "丘陵（平原・草原）に農場を作れない",
     "決定 T01 用水普請（最初から。SSOT 3.2）"),
    ("Adjacency_YieldChanges", {"PrereqCivic": "CIVIC_FEUDALISM"},
     {"PrereqCivic": _c("DOMAIN_LAND_SURVEYS")}, "前提が空になり、農場の隣接食料+1が1ターン目から付く",
     "暫定 C20（封建制の代理）"),
    ("Adjacency_YieldChanges", {"PrereqTech": "TECH_REPLACEABLE_PARTS"},
     {"PrereqTech": _t("OUTER_DEFENSES")}, "近代の隣接食料+1も1ターン目から付く（計+2）", "暫定 最終T32"),
    ("Adjacency_YieldChanges", {"ObsoleteTech": "TECH_REPLACEABLE_PARTS"},
     {"ObsoleteTech": _t("OUTER_DEFENSES")}, "中世の隣接が旧式化しない（上と対）", "暫定 最終T32"),
    ("Features", {"FeatureType": "FEATURE_FOREST"},
     {"RemoveTech": _t("ORE_PROSPECTING"), "AddCivic": _c("LAND_TENURE_CONFIRMATION")},
     "森の伐採が研究なしで可能。植林の制度が空（空の意味は要実機確認）", "暫定 伐採T06（採掘の代理）・植林C30"),
    ("Features", {"FeatureType": "FEATURE_JUNGLE"}, {"RemoveTech": _t("IRON_FORGING")},
     "熱帯雨林の伐採が研究なしで可能", "暫定 T03（青銅器の代理）"),
    ("Features", {"FeatureType": "FEATURE_MARSH"}, {"RemoveTech": _t("IRRIGATION_WORKS")},
     "湿原の除去が研究なしで可能", "暫定 T01（灌漑の代理）"),
    ("Resources", {"ResourceType": ("RESOURCE_ANTIQUITY_SITE", "RESOURCE_SHIPWRECK")},
     {"PrereqCivic": _c("LAND_TENURE_CONFIRMATION")}, "遺跡・沈没船が1ターン目から見える", "暫定 最終C30（考古学は範囲外）"),
    ("UnitCommands", {"CommandType": ("UNITCOMMAND_FORM_CORPS", "UNITCOMMAND_FORM_ARMY")},
     {"PrereqCivic": None, "PrereqTech": _t("REGIONAL_SUPPLY")}, "軍団・大軍団を編成できない",
     "決定 T30 広域兵站（SSOT 3.2）"),
)

# Remaining cascade losses, deliberately not remapped: (tables, rows, reason).  Documentation only;
# tests/test_sengoku_cascade.py pins the residual row counts.
CASCADE_DROPS = (
    ("Boosts / CivicPrereqs / TechnologyPrereqs / CivicQuotes / TechnologyQuotes / CivicRandomCosts / "
     "TechnologyRandomCosts / Civics_XP2 / Technologies_XP2 / StartingBoostedCivics / StartingBoostedTechnologies",
     "削除した公式ノード自身の付随行", "なし（ノードごと消える）"),
    ("CivicModifiers", "影響力トークン、観光、遺物、スパイ2人目以降、外交勝利点・好感度",
     "都市国家・観光・外交勝利は戦国パックで使わない。スパイの追加枠は要検討"),
    ("CivicModifiers ほか", "総督称号の付与（公式制度14件、政府の広場とその建物、女王の図書館、偉人イレーネ・アダム・スミス、"
     "部族の村、スレイマン）", "SSOT 3.2 の決定で総督は戦国向けに作り直すまで無効。付け替えず、"
     "`JAC_Sengoku_Unlocks.sql` が付与する Modifier ごと削除する（監査の差分には出ない）"),
    ("TechnologyModifiers", "乗船移動（横帆船・蒸気・内燃）、都市防御（鋼鉄）、観光、スパイ（コンピュータ）、GDR昇進、未来技術",
     "時代外"),
    ("Improvement_BonusYieldChanges", "産業以降の改善ボーナス、建設不可の改善（キャンプ・牧草地・固有改善）",
     "時代外または Buildable=0"),
    ("Improvement_Tourism / Improvement_ValidFeatures", "観光・製材所", "観光は不使用、製材所は Buildable=0"),
    ("Adjacency_YieldChanges", "公式文明の固有改善（クルガン・シャトー・モアイ・巨石人頭像・万里の長城）の時期",
     "Buildable=0 のため影響なし"),
    ("Units / Buildings / Improvements", "研究ゲートが空になる公式行", "CanTrain=0・InternalOnly=1・Buildable=0。"
     "残した Unit は旧式化の研究だけが空になり、旧式化しない"),
    ("RandomAgendasForCivic / RandomAgendaCivicTags", "ナショナリズムで AI に追加される隠しアジェンダ",
     "AI の性格付けのみ"),
    ("Routes_XP2", "鉄道", "時代外"),
    ("Resource_Harvests", "牛・鹿・羊の収穫", "日本マップに無い（JAC_Sengoku_Resources.sql の方針どおり）"),
)


def _sql_value(value) -> str:
    return "NULL" if value is None else sql_quote(value)


def make_cascade_sql() -> str:
    out = ["-- Generated by tools/generate_sengoku_trees.py (CASCADE_REMAPS). Loaded before",
           "-- JAC_Sengoku_Unlocks.sql deletes the official trees, so these rows keep a Sengoku gate",
           "-- instead of being cascade-deleted or nulled (01_GameplaySchema.sql, Expansion2_Schema.sql).",
           "-- See documents/poc/sengoku-tree-unlocks.md for the audit table; mappings marked 暫定 are provisional."]
    for table, where, values, _, basis in CASCADE_REMAPS:
        conditions = [f"{k} IN ({', '.join(map(sql_quote, v))})" if isinstance(v, tuple) else f"{k}={sql_quote(v)}"
                      for k, v in where.items()]
        sets = ", ".join(f"{k}={_sql_value(v)}" for k, v in values.items())
        out.append(f"-- {basis}\nUPDATE {table} SET {sets} WHERE {' AND '.join(conditions)};")
    return "\n".join(out) + "\n"


def make_sql(nodes: list[Node]) -> str:
    out = [
        "-- Generated by tools/generate_sengoku_trees.py from the research design.",
        "-- Costs and unlock effects are provisional; ERA_* are compatibility slots for GS.",
        "INSERT INTO Types (Type, Kind) VALUES",
        ",\n".join(f"  ({sql_quote(n.id)}, {sql_quote('KIND_TECH' if n.short.startswith('T') else 'KIND_CIVIC')})" for n in nodes) + ";",
    ]
    for kind, table, key in (("T", "Technologies", "TechnologyType"), ("C", "Civics", "CivicType")):
        group = [n for n in nodes if n.short.startswith(kind)]
        out += [f"INSERT INTO {table} ({key}, Name, Description, Cost, EraType, UITreeRow, AdvisorType) VALUES",
                ",\n".join(f"  ({sql_quote(n.id)}, {sql_quote('LOC_' + n.id + '_NAME')}, "
                            f"{sql_quote('LOC_' + n.id + '_DESCRIPTION')}, {n.cost}, "
                            f"{sql_quote(n.era)}, {n.row}, 'ADVISOR_GENERIC')" for n in group) + ";"]
        edges = [(n.id, p) for n in group for p in n.prereqs]
        edge_table, target, source = ("TechnologyPrereqs", "Technology", "PrereqTech") if kind == "T" else ("CivicPrereqs", "Civic", "PrereqCivic")
        out += [f"INSERT INTO {edge_table} ({target}, {source}) VALUES",
                ",\n".join(f"  ({sql_quote(a)}, {sql_quote(b)})" for a, b in edges) + ";"]
    for idx, (name, _, _) in enumerate(ERA_NAMES, 1):
        out.append(f"UPDATE Eras SET Name='LOC_JAC_SENGOKU_ERA_{idx}_NAME', "
                   f"Description='LOC_JAC_SENGOKU_ERA_{idx}_DESCRIPTION' "
                   f"WHERE EraType={sql_quote(ERAS[idx - 1])};")
    for short, definition in BOOSTS.items():
        node = next(n for n in nodes if n.short == short)
        fields = ["TechnologyType", "Boost", "TriggerDescription", "TriggerLongDescription", "BoostClass"]
        fields += list(definition["attributes"])
        trigger = f"LOC_JAC_SENGOKU_BOOST_{short}"
        values = [node.id, 40, trigger, trigger, definition["class"]] + list(definition["attributes"].values())
        out.append(f"INSERT INTO Boosts ({', '.join(fields)}) VALUES "
                   f"({', '.join(str(v) if isinstance(v, int) else sql_quote(v) for v in values)});")
    return "\n\n".join(out) + "\n"


def text_row(tag: str, translations: tuple[str, str, str]) -> list[str]:
    return [f'    <Row Tag="{tag}" Language="{lang}"><Text>{escape(value)}</Text></Row>'
            for lang, value in zip(("ja_JP", "en_US", "ru_RU"), translations)]


def make_text(nodes: list[Node]) -> str:
    out = ['<?xml version="1.0" encoding="utf-8"?>', '<GameInfo>', '  <LocalizedText>']
    for idx, names in enumerate(ERA_NAMES, 1):
        out += text_row(f"LOC_JAC_SENGOKU_ERA_{idx}_NAME", names)
        out += text_row(f"LOC_JAC_SENGOKU_ERA_{idx}_DESCRIPTION", names)
    for n in nodes:
        out += text_row(f"LOC_{n.id}_NAME", (n.ja, n.en, n.ru))
        out += text_row(f"LOC_{n.id}_DESCRIPTION", (
            "戦国ツリーPoC。解放先と効果量は暫定です。",
            "Sengoku tree PoC. Unlocks and effect values are provisional.",
            "Пробный образец древа Сэнгоку. Открываемые объекты и эффекты предварительны.",
        ))
    for short, definition in BOOSTS.items():
        out += text_row(f"LOC_JAC_SENGOKU_BOOST_{short}", definition["text"])
    for _, house_tag, _, translations in HOUSE_NAMES:
        out += text_row(house_tag, translations)
    # SSOT 3.2: governors are disabled, so drop the title from the Government Plaza text
    # (official: DLC/Expansion2/Text/Expansion2_Translations_Text.xml, en_US/Expansion1_Districts_Text.xml).
    out += [row.replace("<Row ", "<Replace ").replace("</Row>", "</Replace>") for row in text_row(
        "LOC_DISTRICT_GOVERNMENT_DESCRIPTION", (
            "政府の選択を記念する区域。自文明に1つだけ設置できる。この都市へのターン毎の忠誠心+8、隣接する区域へ隣接ボーナス+1。",
            "A district placed once in your empire to commemorate your government choices. Provides +8 Loyalty per "
            "turn to this city, and an increase of +1 to the adjacency bonuses being earned by any adjacent district.",
            "Район, построенный в вашей империи в память о решениях вашего правительства. Дает +8 лояльности за ход "
            "этому городу и +1 ко всем бонусам за соседство всем прилежащим районам.",
        ))]
    out += ['  </LocalizedText>', '</GameInfo>']
    return "\n".join(out) + "\n"


def make_house_names() -> str:
    out = ["-- Generated by tools/generate_sengoku_trees.py (HOUSE_NAMES).  SSOT 3.2 house-specific names;",
           "-- the mechanism is UI/JAC_HouseNames.lua in the ruleset mod."]
    out += [f'JAC_RegisterHouseName("{generic}", "{house}", "{civ}")' for generic, house, civ, _ in HOUSE_NAMES]
    return "\n".join(out) + "\n"


def make_icons(nodes: list[Node]) -> str:
    out = ['<?xml version="1.0" encoding="utf-8"?>', '<GameInfo>', '  <IconDefinitions>']
    for kind in ("T", "C"):
        group = [n for n in nodes if n.short.startswith(kind)]
        atlas = "ICON_ATLAS_TECH" if kind == "T" else "ICON_ATLAS_CIVICS"
        for index, node in enumerate(group):
            icon = "ICON_" + node.id
            out.append(f'    <Row Name="{icon}" Index="{index}" Atlas="{atlas}"/>')
            out.append(f'    <Row Name="{icon}_FOW" Index="{index}" Atlas="{atlas}_FOW"/>')
    out += ['  </IconDefinitions>', '</GameInfo>']
    return "\n".join(out) + "\n"


def make_unlock_matrix(nodes: list[Node]) -> str:
    out = [
        "# 戦国ツリーPoC：解放先と閃きの照合",
        "",
        "設計案の各行を、現行の公式アイテムへ接続した範囲。公式アイテムの能力値・名称・時代感は暫定の代理表現であり、"
        "設計案固有の建物・Unit・政策・操作・効果はすべて TODO。未接続ノードも研究自体は可能。",
        "特にC25はSSOT 3.2に従い、既定の名前は「広域検地」とし、織田家（暫定）を操作するときだけ「太閤検地」と表示する。仕組みは[家固有名の表示切替](house-names.md)。",
        "",
        "| ノード | 設計案の解放 | 現行PoCの公式代理 | 残課題 |",
        "|---|---|---|---|",
    ]
    for n in nodes:
        unlock = n.unlock.replace("|", "\\|")
        proxy = OFFICIAL_UNLOCKS.get(n.short, "—")
        todo = f"設計案の固有実装（{unlock}）" if proxy != "—" else f"公式代理なし（{unlock}）"
        out.append(f"| {n.short} {n.ja} | {unlock} | `{proxy.replace(', ', '`, `')}` | TODO: {todo} |" if proxy != "—"
                   else f"| {n.short} {n.ja} | {unlock} | — | TODO: {todo} |")
    out += ["", "## 閃き・着想", "",
            "公式のBoostClassで表せる2件だけを40%の暫定値で実装。T09は銀鉱山の新規改良、T10はT02で建造可能になったガレー船1隻の新規建造で判定する。",
            "それ以外は案の条件を省略・近似して自動達成させない。", "",
            "| ノード | 設計案の条件 | PoCの状態 |", "|---|---|---|"]
    for n in nodes:
        status = ("`BOOST_TRIGGER_IMPROVE_SPECIFIC_RESOURCE`（銀＋鉱山）" if n.short == "T09"
                  else "`BOOST_TRIGGER_TRAIN_UNIT`（ガレー船）" if n.short == "T10"
                  else "TODO: 公式BoostClassで完全には表せない")
        boost_condition = n.proposed_boost.replace("|", "\\|")
        out.append(f"| {n.short} {n.ja} | {boost_condition} | {status} |")
    out += ["", "具体的な原案は[設計表](../research/sengoku-tech-civic-tree.md)の各行を参照。", ""]
    out += make_cascade_doc()
    return "\n".join(out)


def _cell(values: dict) -> str:
    return "<br>".join(f"`{k}`=" + ("、".join(f"`{x}`" for x in v) if isinstance(v, tuple)
                                    else "NULL" if v is None else f"`{v}`") for k, v in values.items())


def make_cascade_doc() -> list[str]:
    out = [
        "## 公式ツリー削除に伴う連鎖削除の監査",
        "",
        "`JAC_Sengoku_Unlocks.sql` の最後で公式の技術・社会制度を削除すると、外部キーで参照する公式行が消えるか、前提が空になる。"
        "公式スキーマで `Civics(CivicType)`・`Technologies(TechnologyType)` を参照する外部キーは68件（XP1スキーマは0件）。",
        "",
        "- `ON DELETE CASCADE`（行ごと消える）：`Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql` の Boosts:391,399-401、"
        "Civics_XP2:630、CivicModifiers:636、CivicPrereqs:643-644、CivicQuotes:651、CivicRandomCosts:657、"
        "DiplomaticActions:923-927、DiplomaticVisibilitySources:1010、Improvement_BonusYieldChanges:1773-1774、"
        "Improvement_Tourism:1800-1801、Improvement_ValidFeatures:1834-1835、Improvement_ValidTerrains:1853-1854、"
        "Policies.PrereqTech:2170、StartingBoostedCivics:2575、StartingBoostedTechnologies:2582、StartingCivics:2601、"
        "Technologies_XP2:2680、TechnologyModifiers:2686、TechnologyPrereqs:2692-2693、TechnologyQuotes:2700、"
        "TechnologyRandomCosts:2706、UnitCommands:2992-2993、UnitOperations:3028-3029。"
        "`DLC/Expansion2/Data/Expansion2_Schema.sql` の RandomAgendaCivicTags:688、RandomAgendasForCivic:703、"
        "Resource_Consumption:882、Routes_XP2:899",
        "- `ON DELETE SET NULL`（前提が空＝最初から解放）：01_GameplaySchema.sql の Adjacency_YieldChanges:40-43、"
        "Features:1259-1260、Resources:2342-2343、Resource_Harvests:2367",
        "- `ON DELETE SET DEFAULT`（既定値 NULL＝同上）：01_GameplaySchema.sql の Buildings:473,475、Districts:1056-1057、"
        "Governments:1467、Improvements:1752-1753、Policies.PrereqCivic:2168、Projects:2201-2202、Units:2903-2917",
        "",
        "Districts・Governments・Policies・Projects・StartingCivics・Resources の研究ゲートと Units・Buildings・Improvements の"
        "残す行は従来どおり `JAC_Sengoku_Unlocks.sql` と `JAC_Sengoku_Resources.sql` で付け替える。"
        "それ以外で実害がある行は生成ファイル `Data/JAC_Sengoku_Cascade.sql`（`CASCADE_REMAPS`）が削除前に戦国ノードへ付け替える。"
        "監査は `python tools/sengoku_cascade_audit.py`（公式 Base＋GS のデータで戦国SQLを外部キー有効のまま実行し、"
        "公式ツリーの削除だけによる差分を出す）、固定は `tests/test_sengoku_cascade.py`。"
        "他の DLC と他の MOD は監査の対象外。「暫定」は設計案に記載がなく、同じ時代枠（公式 Era ID と戦国の期の対応）"
        "と内容の近さで選んだもの。「決定」はオーナーが決めたもの（SSOT 3.2）。GS は防衛協定と研究協定を `Expansion2_RemoveData.xml` で削除済みのため対象外。",
        "",
        "### 付け替えた行",
        "",
        "| 表 | 対象の公式行 | 付け替えない場合の影響 | 付け替え先 | 根拠 |",
        "|---|---|---|---|---|",
    ]
    out += [f"| {table} | {_cell(where)} | {impact} | {_cell(values)} | {basis} |"
            for table, where, values, impact, basis in CASCADE_REMAPS]
    out += ["", "### 付け替えずに失う行", "", "| 表 | 失う行 | 影響と理由 |", "|---|---|---|"]
    out += [f"| {table} | {rows} | {reason} |" for table, rows, reason in CASCADE_DROPS]
    out += ["", "総督を無効にしても公式 UI は壊れない見込み：ランチバーの総督ボタンは `CAPABILITY_GOVERNORS` で出たままだが、"
            "通知の印は `CanAppoint()`・`CanPromote()` が真のときだけ出る（`DLC/Expansion1/UI/Replacements/"
            "LaunchBar_Expansion1.lua:129-136`）。総督画面は称号0でも `GetGovernorPoints()`・`CanAppoint()` で候補を任命不可として"
            "並べるだけで（`DLC/Expansion2/UI/Additions/GovernorPanel.lua:55-57`）、公式でも最初の制度を得るまでは称号0で始まる。"
            "政府の広場の説明文からは称号の一文を外した（生成ファイル `Text/JAC_Sengoku_Text.xml`）。偉人イレーネ・アダム・スミスの"
            "説明は付与の Modifier ごと消えるため、称号の行が出ない見込み。",
            "", "**要実機確認**：同盟の提案可否と、総督称号の付与元が無いこと・丘陵の農場が T01・軍団と大軍団が T30 で解放されることは "
            "`tools/civ6-autotest/scenarios/sengoku-alliances.json` で確かめる（未実行）。総督画面を開いてもエラーが出ないことは"
            "実機で確かめる。連鎖削除の差分は SQLite での再現で、ゲーム本体の外部キー処理と一致するかは実機の"
            "`GameInfo` で確かめる。", ""]
    return out


def outputs() -> dict[Path, str]:
    nodes = read_design()
    return {
        MOD / "Data/JAC_Sengoku_Trees.sql": make_sql(nodes),
        MOD / "Data/JAC_Sengoku_Cascade.sql": make_cascade_sql(),
        MOD / "Text/JAC_Sengoku_Text.xml": make_text(nodes),
        MOD / "Data/JAC_Sengoku_Icons.xml": make_icons(nodes),
        MOD / "UI/JAC_HouseNamesData_Sengoku.lua": make_house_names(),
        ROOT / "documents/poc/sengoku-tree-unlocks.md": make_unlock_matrix(nodes),
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if generated files are stale")
    args = parser.parse_args()
    for path, content in outputs().items():
        if args.check:
            if not path.exists() or path.read_text(encoding="utf-8") != content:
                raise SystemExit(f"stale generated file: {path}")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8", newline="\n")
    print("Sengoku tree files current" if args.check else "Sengoku tree files generated")
