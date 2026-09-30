"""Generate the 改元 rows, the engine eras and their texts of the Sengoku pack from the reign list.

The source is documents/spec/data/sengoku-reigns.json (SSOT 3.2, design: documents/research/sengoku-ages.md §4.1).
元号 = 時代 (owner decision T, documents/poc/era-equals-reign.md): every reign is one Civ6 era whose length is fixed
to the reign's standard turns, so the engine changes the era at each 改元. The ledger that reads the rows is
poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_Ages.lua.  Run with ``python tools/generate_sengoku_reigns.py``;
``--check`` fails when the generated files are stale.
"""

from __future__ import annotations

import argparse
import json
import xml.etree.ElementTree as ET
from pathlib import Path
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[1]
REIGNS_JSON = ROOT / "documents/spec/data/sengoku-reigns.json"
MOD = ROOT / "poc/JAC_PoC_SengokuTrees"
CALENDAR = MOD / "Data/JAC_Sengoku_Calendar.xml"
SPEEDS = ("GAMESPEED_ONLINE", "GAMESPEED_QUICK", "GAMESPEED_STANDARD", "GAMESPEED_EPIC", "GAMESPEED_MARATHON")
# CostMultiplier of each speed (Base/Assets/Gameplay/Data/GameSpeeds.xml:22,31,40,49,58). The engine scales the
# Eras_XP1 turns by it, rounding down, and an era lasts at least 2 turns (probe 2026-09-30: 3->2, 5->3, 1->2, 2->2
# at quick; 1->2 at standard; documents/poc/era-equals-reign.md). Epic and marathon are not measured.
MULTIPLIER = {"GAMESPEED_ONLINE": 50, "GAMESPEED_QUICK": 67, "GAMESPEED_STANDARD": 100, "GAMESPEED_EPIC": 150,
              "GAMESPEED_MARATHON": 300}
MIN_ERA_TURNS = 2
# The official eras in chronological order (Base/Assets/Gameplay/Data/Eras.xml:16-23 ChronologyIndex 1-8,
# DLC/Expansion2/Data/Expansion2_Eras.xml:7 ERA_FUTURE 9). They keep their IDs (renamed to the first 9 reigns) so
# no official row that references them cascades; the later reigns are new ERA_JAC_<ID> eras.
OFFICIAL_ERAS = ("ERA_ANCIENT", "ERA_CLASSICAL", "ERA_MEDIEVAL", "ERA_RENAISSANCE", "ERA_INDUSTRIAL", "ERA_MODERN",
                 "ERA_ATOMIC", "ERA_INFORMATION", "ERA_FUTURE")
# Great people and great works of the official eras after the Sengoku tree would otherwise arrive with the 5th-9th
# reigns (天正 1573 to 寛永 1624). They move to the reign of the official era's start year (StartEras Year,
# Base/Assets/Gameplay/Data/Eras.xml:37-41; ERA_FUTURE has none: 2020 puts it in 令和, the last historical reign).
LATE_OFFICIAL_ERA_YEARS = (("ERA_INDUSTRIAL", 1725), ("ERA_MODERN", 1890), ("ERA_ATOMIC", 1945),
                           ("ERA_INFORMATION", 1995), ("ERA_FUTURE", 2020))

# Per-reign texts. {p} is the previous reign's name, {n} this reign's, {y} the year.
ANNOUNCEMENT = {"ja_JP": "改元[NEWLINE]{p}から{n}へ（{y}年）",
                "en_US": "New era name:[NEWLINE]from {p} to {n} ({y})",
                "ru_RU": "Смена девиза правления:[NEWLINE]с {p} на {n} ({y})"}
NOTIFICATION = {"ja_JP": "改元：{n}の世", "en_US": "New era name: {n} Era", "ru_RU": "Смена девиза: эпоха {n}"}
ERA_NAME = {"ja_JP": "{n}の世", "en_US": "{n} Era", "ru_RU": "Эпоха {n}"}  # as LOC_JAC_AGE_ERA_NAME
LANGS = (("ja", "ja_JP"), ("en", "en_US"), ("ru", "ru_RU"))


def load() -> dict:
    return json.loads(REIGNS_JSON.read_text(encoding="utf-8"))


def calendar() -> tuple[int, dict[str, list[tuple[int, int]]]]:
    """Start year and, per speed, the (MonthIncrement, TurnsPerIncrement) stages in the ledger's order."""
    root = ET.parse(CALENDAR).getroot()
    stages: dict[str, list[tuple[int, int]]] = {}
    for row in root.findall("./GameSpeed_Turns/Row"):
        stages.setdefault(row.findtext("GameSpeedType"), []).append(
            (int(row.findtext("MonthIncrement")), int(row.findtext("TurnsPerIncrement"))))
    for rows in stages.values():
        rows.sort(key=lambda r: (-r[0], r[1]))  # longest turn first, as the calendar scripts
    return int(root.findtext("./StartEras/Update/Set/Year")), stages


def turn_containing(year: int, month: int, start_year: int, stages: list[tuple[int, int]]) -> int:
    """The game-relative turn (1 = start) whose dates contain the month."""
    target, months, turn = (year - start_year) * 12 + month - 1, 0, 1
    for month_increment, turns in stages:
        span = turns * month_increment
        if target < months + span:
            return turn + (target - months) // month_increment
        months, turn = months + span, turn + turns
    return turn


def turns_by_speed() -> dict[str, list[int]]:
    """The calendar turn containing each 改元's month (the era changes on it at standard speed)."""
    start_year, stages = calendar()
    return {speed: [turn_containing(r["year"], r["month"], start_year, stages[speed]) for r in load()["reigns"]]
            for speed in SPEEDS}


def eras() -> list[dict]:
    """One engine era per reign from 天文: EraType, the reign's id, ChronologyIndex and the era's length in standard
    turns (None for 遅代, which never ends). A reign shorter than MIN_ERA_TURNS at standard speed cannot be its own
    era (the engine would stretch it); none is (the shortest, 万延, has 3 turns), so the generator refuses such a
    table rather than merge reigns."""
    data = load()
    reigns = [data["start"]] + data["reigns"]
    starts = [1] + turns_by_speed()["GAMESPEED_STANDARD"]
    out = []
    for i, reign in enumerate(reigns):
        length = starts[i + 1] - starts[i] if i + 1 < len(reigns) else None
        if length is not None and length < MIN_ERA_TURNS:
            raise ValueError(f"{reign['id']} lasts {length} standard turns; an era lasts at least {MIN_ERA_TURNS}")
        era = OFFICIAL_ERAS[i] if i < len(OFFICIAL_ERAS) else f"ERA_JAC_{reign['id']}"
        out.append({"EraType": era, "id": reign["id"], "ChronologyIndex": i + 1, "length": length})
    return out


def engine_turns(speed: str) -> list[int]:
    """The game-relative turn on which the engine starts each era after the first (the ledger's judgement turns)."""
    turn, out = 1, []
    for era in eras()[:-1]:
        turn += max(MIN_ERA_TURNS, era["length"] * MULTIPLIER[speed] // 100)
        out.append(turn)
    return out


def era_of_date(year: int, month: int) -> str:
    """EraType of the reign in force in that month."""
    all_eras = eras()
    current = all_eras[0]["EraType"]
    for era, reign in zip(all_eras[1:], load()["reigns"]):
        if (reign["year"], reign["month"]) <= (year, month):
            current = era["EraType"]
    return current


def sql(value) -> str:
    return "NULL" if value is None else "'" + str(value).replace("'", "''") + "'"


def build() -> tuple[str, str]:
    data = load()
    reigns = data["reigns"]
    engine = {speed: engine_turns(speed) for speed in SPEEDS}
    era_types = [era["EraType"] for era in eras()[1:]]
    out = ["-- Generated by tools/generate_sengoku_reigns.py from documents/spec/data/sengoku-reigns.json.\n",
           "-- The 改元 after the start (天文) in date order, each with its engine era (Data/JAC_Sengoku_Eras.sql); the\n",
           "-- ledger judges when that era starts. Trailing comment: that turn at the online / quick / standard / epic /\n",
           "-- marathon speeds.\n",
           "INSERT INTO JAC_SengokuReigns (ReignType, Year, Month, Name, Announcement, Notification, ForcedAge, EraType) "
           "VALUES\n"]
    rows = []
    for i, r in enumerate(reigns):
        key = f"LOC_JAC_REIGN_{r['id']}"
        values = ", ".join((sql("REIGN_" + r["id"]), str(r["year"]), str(r["month"]), sql(key + "_NAME"),
                            sql(key + "_ANNOUNCEMENT"), sql(key + "_NOTIFICATION"), sql(r.get("forced_age")),
                            sql(era_types[i])))
        at = " / ".join(str(engine[s][i]) for s in SPEEDS)
        rows.append(f"  ({values}){',' if i + 1 < len(reigns) else ';'} -- {r['ja']} {at}\n")
    out += rows

    text: dict[str, dict[str, str]] = {}
    previous = data["start"]
    text[f"LOC_JAC_REIGN_{previous['id']}_NAME"] = {lang: previous[short] for short, lang in LANGS}
    for r in reigns:
        key = f"LOC_JAC_REIGN_{r['id']}"
        text[key + "_NAME"] = {lang: r[short] for short, lang in LANGS}
        text[key + "_ANNOUNCEMENT"] = {lang: ANNOUNCEMENT[lang].format(p=previous[short], n=r[short], y=r["year"])
                                       for short, lang in LANGS}
        text[key + "_NOTIFICATION"] = {lang: NOTIFICATION[lang].format(n=r[short]) for short, lang in LANGS}
        previous = r
    for r in [data["start"]] + reigns:
        text[f"LOC_JAC_ERA_{r['id']}_NAME"] = {lang: ERA_NAME[lang].format(n=r[short]) for short, lang in LANGS}
    xml = ['<?xml version="1.0" encoding="utf-8"?>\n',
           "<!-- Generated by tools/generate_sengoku_reigns.py from documents/spec/data/sengoku-reigns.json. -->\n",
           "<GameData>\n  <LocalizedText>\n"]
    for key, by_lang in text.items():
        for _, lang in LANGS:
            xml.append(f'    <Row Tag="{key}" Language="{lang}"><Text>{escape(by_lang[lang])}</Text></Row>\n')
    xml.append("  </LocalizedText>\n</GameData>\n")
    return "".join(out), "".join(xml)


def build_eras() -> str:
    """Data/JAC_Sengoku_Eras.sql, loaded before the trees (they name the eras)."""
    all_eras = eras()
    official, added = all_eras[:len(OFFICIAL_ERAS)], all_eras[len(OFFICIAL_ERAS):]
    source = sql(OFFICIAL_ERAS[-1])
    out = ["-- Generated by tools/generate_sengoku_reigns.py from documents/spec/data/sengoku-reigns.json.",
           "-- 元号 = 時代 (owner decision T, SSOT 3.2, documents/poc/era-equals-reign.md): one engine era per reign. The",
           "-- 9 official eras keep their IDs and take the first 9 reigns' names; the other reigns are new eras after them.",
           "-- Each era lasts its reign's standard turns (Eras_XP1 minimum = maximum; the engine scales other speeds by",
           "-- GameSpeeds.CostMultiplier) and 遅代 never ends (NULL/NULL like the official ERA_FUTURE,",
           "-- DLC/Expansion2/Data/Expansion1_Eras.xml:12; both columns may be NULL, Expansion2_Schema.sql:293-299).",
           "-- A countdown of 1 (official 10, DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:204) announces the",
           "-- change only on the turn before it.",
           "INSERT OR REPLACE INTO GlobalParameters (Name, Value) VALUES ('NEXT_ERA_TURN_COUNTDOWN', '1');",
           "",
           "-- Official eras renamed (Base/Assets/Gameplay/Data/Eras.xml:16-23, DLC/Expansion2/Data/Expansion2_Eras.xml:7);",
           "-- the description is the name too."]
    for era in official:
        name = sql(f"LOC_JAC_ERA_{era['id']}_NAME")
        out.append(f"UPDATE Eras SET Name={name}, Description={name} WHERE EraType={sql(era['EraType'])};")
    columns = ("WarmongerPoints, GreatPersonBaseCost, EraTechBackgroundTexture, EraCivicBackgroundTexture, "
               "WarmongerLevelDescription, EmbarkedUnitStrength, EraTechBackgroundTextureOffsetX, "
               "EraCivicBackgroundTextureOffsetX, TechTreeLayoutMethod")
    out += ["",
            "-- New eras. The other Eras columns and the per-era rows of Eras_XP1 (LiberatedEnvoys) and Eras_XP2 copy",
            "-- ERA_FUTURE, the last official era (DLC/Expansion2/Data/Expansion2_Eras.xml:7,73,",
            "-- DLC/Expansion2/Data/Expansion1_Eras.xml:12). No other per-era table needs a row for an era that is not a",
            "-- start era (documents/poc/era-equals-reign.md).",
            "INSERT INTO Types (Type, Kind) VALUES",
            ",\n".join(f"  ({sql(era['EraType'])}, 'KIND_ERA')" for era in added) + ";"]
    for era in added:
        name = sql(f"LOC_JAC_ERA_{era['id']}_NAME")
        out.append(f"INSERT INTO Eras (EraType, Name, Description, ChronologyIndex, {columns}) SELECT "
                   f"{sql(era['EraType'])}, {name}, {name}, {era['ChronologyIndex']}, {columns} FROM Eras "
                   f"WHERE EraType={source};")
        out.append(f"INSERT INTO Eras_XP1 (EraType, LiberatedEnvoys) SELECT {sql(era['EraType'])}, LiberatedEnvoys "
                   f"FROM Eras_XP1 WHERE EraType={source};")
        out.append(f"INSERT INTO Eras_XP2 (EraType, GrievanceDecayRate, TensionDecayRate, TradeRouteMinimumEndTurnChange, "
                   f"EraScoreThresholdShift) SELECT {sql(era['EraType'])}, GrievanceDecayRate, TensionDecayRate, "
                   f"TradeRouteMinimumEndTurnChange, EraScoreThresholdShift FROM Eras_XP2 WHERE EraType={source};")
    out += ["",
            "-- Era lengths: the standard turns from the reign's 改元 to the next (the calendar turn containing each month).",
            "-- Official 40/60 (DLC/Expansion2/Data/Expansion1_Eras.xml:3-12)."]
    for era in all_eras:
        length = "NULL" if era["length"] is None else str(era["length"])
        out.append(f"UPDATE Eras_XP1 SET GameEraMinimumTurns={length}, GameEraMaximumTurns={length} "
                   f"WHERE EraType={sql(era['EraType'])}; -- {era['id']}")
    out += ["",
            "-- Great people and great works of the official eras after the Sengoku tree go to the reign of the official",
            "-- era's start year (LATE_OFFICIAL_ERA_YEARS in the generator) instead of arriving with 天正 to 寛永."]
    for era_type, year in LATE_OFFICIAL_ERA_YEARS:
        target = sql(era_of_date(year, 1))
        for table in ("GreatPersonIndividuals", "GreatWorks"):
            out.append(f"UPDATE {table} SET EraType={target} WHERE EraType={sql(era_type)}; -- {year}")
    return "\n".join(out) + "\n"


def outputs() -> dict[Path, str]:
    sql_text, xml_text = build()
    return {MOD / "Data/JAC_Sengoku_Reigns.sql": sql_text, MOD / "Text/JAC_Sengoku_Reigns_Text.xml": xml_text,
            MOD / "Data/JAC_Sengoku_Eras.sql": build_eras()}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if generated files are stale")
    args = parser.parse_args()
    for path, content in outputs().items():
        if args.check:
            if not path.exists() or path.read_text(encoding="utf-8") != content:
                raise SystemExit(f"stale generated file: {path}")
        else:
            path.write_text(content, encoding="utf-8", newline="\n")
    print("Sengoku reign files current" if args.check else "Sengoku reign files generated")
