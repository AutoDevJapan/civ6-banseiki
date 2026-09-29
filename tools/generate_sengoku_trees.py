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
    ("天下布武と広域戦", "Campaigns for Unification", "Походы за объединение"),
    ("統一事業", "The Work of Unification", "Объединение страны"),
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
    "T01": "IMPROVEMENT_FARM, BUILDING_WATER_MILL",
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
    out += ['  </LocalizedText>', '</GameInfo>']
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
        "特にC25はSSOT 3.2に従い、豊臣以外にも表示するPoCでは「広域検地」と呼ぶ。家ごとの表示分岐はTODO。",
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
    return "\n".join(out)


def outputs() -> dict[Path, str]:
    nodes = read_design()
    return {
        MOD / "Data/JAC_Sengoku_Trees.sql": make_sql(nodes),
        MOD / "Text/JAC_Sengoku_Text.xml": make_text(nodes),
        MOD / "Data/JAC_Sengoku_Icons.xml": make_icons(nodes),
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
