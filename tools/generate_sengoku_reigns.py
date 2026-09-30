"""Generate the 改元 rows, the engine eras and their texts of the Sengoku pack from the reign list.

The source is documents/spec/data/sengoku-reigns.json (SSOT 3.2, design: documents/research/sengoku-ages.md §4.1).
元号 = 時代 (owner decision T, documents/poc/era-equals-reign.md): every reign is one Civ6 era whose length is fixed
to the reign's standard turns, so the engine changes the era at each 改元. The ledger that reads the rows is
poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_Ages.lua. The calendar (Data/JAC_Sengoku_Calendar.xml) is generated too:
the other speeds date each engine era's first turn with its 改元 (owner decision U-2b).  Run with ``python tools/generate_sengoku_reigns.py``;
``--check`` fails when the generated files are stale.
"""

from __future__ import annotations

import argparse
import functools
import json
import random
from pathlib import Path
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[1]
REIGNS_JSON = ROOT / "documents/spec/data/sengoku-reigns.json"
MOD = ROOT / "poc/JAC_PoC_SengokuTrees"
CALENDAR = MOD / "Data/JAC_Sengoku_Calendar.xml"
# SSOT 3.1/3.2: the game starts in 1534 and standard speed dates 4 months a turn, about 200 turns to 1600. The reigns'
# era lengths come from this pace; the other speeds' calendars are generated to follow their engine eras.
START_YEAR = 1534
STANDARD_MONTHS = 4
CALENDAR_TURNS = 10000  # every speed's stages cover this many turns (no time victory, JAC_Sengoku_Victories.xml)
STANDARD_STAGES = [(STANDARD_MONTHS, CALENDAR_TURNS)]
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
# The official eras after the Sengoku tree, which would otherwise bring their great people and great works with the
# 5th-9th reigns (天正 1573 to 寛永 1624). Their great people are removed (owner decision U-3b); great works with no
# great person move to the reign of the official era's start year (StartEras Year, Base/Assets/Gameplay/Data/
# Eras.xml:37-41; ERA_FUTURE has none: 2020 puts it in 令和, the last historical reign).
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


def reign_months() -> list[int]:
    """Months from January of the start year to each 改元."""
    return [(r["year"] - START_YEAR) * 12 + r["month"] - 1 for r in load()["reigns"]]


def turn_containing(month: int, stages: list[tuple[int, int]]) -> int:
    """The game-relative turn (1 = start) whose dates contain the month (months from January of the start year),
    walking the (MonthIncrement, TurnsPerIncrement) stages in row order as the engine does."""
    months, turn = 0, 1
    for month_increment, turns in stages:
        span = turns * month_increment
        if month < months + span:
            return turn + (month - months) // month_increment
        months, turn = months + span, turn + turns
    return turn


# The other speeds' calendars (owner decision U-2b): every engine era starts on the turn whose dates contain its
# 改元's month; that turn may begin up to SLACK_MONTHS earlier, which leaves room for the key rule of _rows.
SLACK_MONTHS = 2
RUN_COST = 6  # months of unevenness a reign may trade for one stage fewer (stage keys are scarce)
TRIES = 500


def _chunked(turns: int, months: int, chunks: int, high_first: bool) -> list[tuple[int, int]] | None:
    """Stages dating `turns` turns over `months` months in `chunks` parts, each ending on the even pace's month and
    split into two increments one month apart; the parts alternate which comes first so neighbours merge."""
    cuts = [round(turns * i / chunks) for i in range(chunks + 1)]
    runs: list[tuple[int, int]] = []
    high = high_first
    for start, end in zip(cuts, cuts[1:]):
        low, extra = divmod(round(months * end / turns) - round(months * start / turns), end - start)
        if low < 1:
            return None
        for run in ([(low + 1, extra), (low, end - start - extra)] if high else
                    [(low, end - start - extra), (low + 1, extra)]):
            if run[1] and runs and runs[-1][0] == run[0]:
                runs[-1] = (run[0], runs[-1][1] + run[1])
            elif run[1]:
                runs.append(run)
        high = not high
    return runs


def _gap(runs, turns: int, months: int) -> float:
    """The most months a turn's date strays from the even pace over the reign."""
    total = turn = worst = 0
    for increment, count in runs:
        for _ in range(count):
            total, turn = total + increment, turn + 1
            worst = max(worst, abs(total - months * turn / turns))
    return worst


def _span_options(turns: int, months: int, first_above: int) -> list:
    """Ways to date a reign's turns (one increment, two, or chunks of two), each with its gap, the most even first.
    The first increment must exceed first_above (the months the reign's first turn begins before its 改元)."""
    pace = months / turns
    out: dict[tuple[tuple[int, int], ...], float] = {}
    if months % turns == 0 and months // turns > first_above:
        out[((months // turns, turns),)] = 0.0
    for first in range(first_above + 1, int(pace) + 4):
        for count in range(1, turns):
            later, remainder = divmod(months - first * count, turns - count)
            if remainder == 0 and later >= 1 and later != first and abs(later - pace) < 4:
                out[((first, count), (later, turns - count))] = abs(first - pace) * count
    for chunks in range(2, max(2, turns // 8) + 1):
        for high_first in (True, False):
            runs = _chunked(turns, months, chunks, high_first)
            if runs and runs[0][0] > first_above:
                out.setdefault(tuple(runs), _gap(runs, turns, months))
    return sorted(((runs, gap) for runs, gap in out.items() if max(m for m, _ in runs) <= 12),  # no year skipped
                  key=lambda item: item[1])


def _rows(run: tuple[int, int], used: set) -> list[tuple[int, int]] | None:
    """GameSpeed_Turns rows for a stage. The table's key is (speed, MonthIncrement, TurnsPerIncrement)
    (01_GameplaySchema.sql:1403-1408), so a stage whose key is taken is split in two untaken ones (halves first,
    keeping the short keys for short stages); None if it cannot be."""
    increment, count = run
    if run not in used:
        return [run]
    for part in range(count // 2, 0, -1):
        if part != count - part and (increment, part) not in used and (increment, count - part) not in used:
            return [(increment, part), (increment, count - part)]
    return None


def _try_calendar(speed: str, rng: random.Random, noise: float, cache: dict) -> list[tuple[int, int]] | None:
    """One greedy pass over the reigns: each takes its cheapest option (gap, stages, early start, noise) whose rows
    repeat no key and leave the last stage closable."""
    starts, months = [1] + engine_turns(speed), [0] + reign_months()
    used: set[tuple[int, int]] = set()
    rows: list[tuple[int, int]] = []
    current, slack = None, 0  # the last stage (still open to merging) and how early the reign's first turn begins
    for k in range(len(starts) - 1):
        candidates = []
        for early in (range(SLACK_MONTHS + 1) if k + 2 < len(starts) else (0,)):  # 遅代 starts on its month
            key = (starts[k + 1] - starts[k], months[k + 1] - months[k] + slack - early, slack)
            if key not in cache:
                cache[key] = _span_options(*key)
            candidates += [(gap + RUN_COST * len(runs) + 2 * early + rng.random() * noise, early, runs)
                           for runs, gap in cache[key]]
        for _, early, option in sorted(candidates, key=lambda c: c[0]):
            runs = list(option)
            if current and current[0] == runs[0][0]:
                runs[0] = (current[0], current[1] + runs[0][1])
                closing = runs[:-1]
            else:
                closing = ([current] if current else []) + runs[:-1]
            taken, new = set(used), []
            for run in closing:
                split = _rows(run, taken)
                if split is None:
                    break
                new += split
                taken |= set(split)
            else:
                if _rows(runs[-1], taken) is not None:
                    break
        else:
            return None
        used, rows, current, slack = taken, rows + new, runs[-1], early
    # After 遅代 the calendar keeps the old average pace for the rest of the turns.
    tail = (round(STANDARD_MONTHS * 100 / MULTIPLIER[speed]), CALENDAR_TURNS - (starts[-1] - 1))
    for run in [(tail[0], current[1] + tail[1])] if current[0] == tail[0] else [current, tail]:
        split = _rows(run, used)
        if split is None:
            return None
        rows += split
        used |= set(split)
    return rows


@functools.cache
def _speed_stages(speed: str) -> tuple[tuple[int, int], ...]:
    """The calendar of a speed other than standard: greedy passes with growing random noise (fixed seed, so the
    output is stable) until one finds rows with no repeated key."""
    rng, cache = random.Random(0), {}
    for attempt in range(TRIES):
        rows = _try_calendar(speed, rng, attempt and 1 + attempt / 20, cache)
        if rows is not None:
            return tuple(rows)
    raise ValueError(f"{speed}: no calendar avoids a repeated GameSpeed_Turns key")


def calendar() -> tuple[int, dict[str, list[tuple[int, int]]]]:
    """Start year and, per speed, the (MonthIncrement, TurnsPerIncrement) stages in row order."""
    return START_YEAR, {speed: STANDARD_STAGES if speed == "GAMESPEED_STANDARD" else list(_speed_stages(speed))
                        for speed in SPEEDS}


def turns_by_speed() -> dict[str, list[int]]:
    """The calendar turn containing each 改元's month (the era changes on it at standard speed)."""
    stages = calendar()[1]
    return {speed: [turn_containing(month, stages[speed]) for month in reign_months()] for speed in SPEEDS}


def eras() -> list[dict]:
    """One engine era per reign from 天文: EraType, the reign's id, ChronologyIndex and the era's length in standard
    turns (None for 遅代, which never ends). A reign shorter than MIN_ERA_TURNS at standard speed cannot be its own
    era (the engine would stretch it); none is (the shortest, 万延, has 3 turns), so the generator refuses such a
    table rather than merge reigns."""
    data = load()
    reigns = [data["start"]] + data["reigns"]
    starts = [1] + [turn_containing(month, STANDARD_STAGES) for month in reign_months()]
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
    late = ", ".join(sql(era) for era, _ in LATE_OFFICIAL_ERA_YEARS)
    out += ["",
            "-- Owner decision U-3b: the official great people of the Industrial to Future eras do not appear in the",
            "-- Sengoku pack (they would arrive with 天正 to 寛永). Deleting their Types rows cascades to",
            "-- GreatPersonIndividuals (01_GameplaySchema.sql:1519,1566) and on to their great works, action and birth",
            "-- modifiers and the journalism visibility source (ON DELETE CASCADE, :1012,1573,1579,1598).",
            "DELETE FROM Types WHERE Type IN (SELECT GreatPersonIndividualType FROM GreatPersonIndividuals "
            f"WHERE EraType IN ({late}));",
            "-- The great works of those eras with no great person (the industrial artifacts GREATWORK_ARTIFACT_21-25,",
            "-- Base/Assets/Gameplay/Data/GreatWorks.xml) go to the reign of the official era's start year",
            "-- (LATE_OFFICIAL_ERA_YEARS in the generator)."]
    for era_type, year in LATE_OFFICIAL_ERA_YEARS:
        out.append(f"UPDATE GreatWorks SET EraType={sql(era_of_date(year, 1))} WHERE EraType={sql(era_type)}; -- {year}")
    return "\n".join(out) + "\n"


def build_calendar() -> str:
    """Data/JAC_Sengoku_Calendar.xml: the 1534 start and every speed's GameSpeed_Turns rows."""
    _, stages = calendar()
    out = ['<?xml version="1.0" encoding="utf-8"?>',
           "<!-- Generated by tools/generate_sengoku_reigns.py from documents/spec/data/sengoku-reigns.json. -->",
           "<GameInfo>",
           "\t<StartEras>",
           "\t\t<Update>",
           '\t\t\t<Where EraType="ERA_ANCIENT"/>',
           "\t\t\t<Set>",
           f"\t\t\t\t<Year>{START_YEAR}</Year>",
           "\t\t\t</Set>",
           "\t\t</Update>",
           "\t</StartEras>",
           "\t<!-- Starting in 1534 makes the engine's start turn about 16603 (live 2026-09-29); show turns from 1 like the",
           "\t     official PiratesScenario_GameCapabilities.xml:4 (TopPanel.lua:389-399 normalizes with this capability). -->",
           "\t<GameCapabilities>",
           '\t\t<Row GameCapability="CAPABILITY_DISPLAY_NORMALIZED_TURN"/>',
           "\t</GameCapabilities>",
           "\t<!-- Replace the ordinary 4000 BC progression only while the Sengoku action criterion matches. The engine",
           "\t     walks a speed's rows in row order (GameSpeedTurnCollection, ORDER BY rowid: 01_GameplaySchema.sql:3337).",
           f"\t     Standard: {STANDARD_MONTHS} months a turn (SSOT 3.2: about 200 turns to 1600); the eras last the reigns'",
           "\t     standard turns. Other speeds: the engine scales the eras by CostMultiplier, and these rows date each",
           "\t     era's first turn with its 改元's month (owner decision U-2b, documents/poc/era-equals-reign.md).",
           f"\t     Every speed covers {CALENDAR_TURNS} turns, so games without a turn limit keep a date. -->",
           "\t<GameSpeed_Turns>",
           "\t\t<Delete/>"]
    for speed in ("GAMESPEED_STANDARD", "GAMESPEED_QUICK", "GAMESPEED_ONLINE", "GAMESPEED_EPIC", "GAMESPEED_MARATHON"):
        for increment, turns in stages[speed]:
            out += ["\t\t<Row>",
                    f"\t\t\t<GameSpeedType>{speed}</GameSpeedType>",
                    f"\t\t\t<MonthIncrement>{increment}</MonthIncrement>",
                    f"\t\t\t<TurnsPerIncrement>{turns}</TurnsPerIncrement>",
                    "\t\t</Row>"]
    out += ["\t</GameSpeed_Turns>", "</GameInfo>"]
    return "\n".join(out) + "\n"


def outputs() -> dict[Path, str]:
    sql_text, xml_text = build()
    return {MOD / "Data/JAC_Sengoku_Reigns.sql": sql_text, MOD / "Text/JAC_Sengoku_Reigns_Text.xml": xml_text,
            MOD / "Data/JAC_Sengoku_Eras.sql": build_eras(), CALENDAR: build_calendar()}


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
