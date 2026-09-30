"""Generate the Sengoku historical event rows of JAC_PoC_HistoricalEvents from the event list.

The source is documents/spec/data/sengoku-events.json (design: documents/research/sengoku-events.md); the
mechanism is documents/poc/historical-events.md.  Run with ``python tools/generate_sengoku_events.py``;
``--check`` fails when the generated files are stale.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[1]
EVENTS_JSON = ROOT / "documents/spec/data/sengoku-events.json"
RESOURCES_JSON = ROOT / "documents/spec/data/japan-map-resources.json"
SITES_LUA = ROOT / "poc/JAC_PoC_JapanMapScale/Scripts/JAC_ResourceSites.lua"
STARTING_KNOWLEDGE_LUA = ROOT / "poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_StartingKnowledge.lua"
MOD = ROOT / "poc/JAC_PoC_HistoricalEvents"
LANGS = (("ja", "ja_JP"), ("en", "en_US"), ("ru", "ru_RU"))

# House display names the events switch to (SSOT 3.1/3.2). The list gives only the Japanese name.
RENAME_NAMES = {
    "上杉": ("UESUGI", "Uesugi", "Уэсуги"),
    "徳川": ("TOKUGAWA", "Tokugawa", "Токугава"),
    "羽柴": ("HASHIBA", "Hashiba", "Хасиба"),
    "豊臣": ("TOYOTOMI", "Toyotomi", "Тоётоми"),
}
FIXED_TEXT = {
    # Notification summary of events without an advisor line.
    "LOC_JAC_EVENT_SUMMARY": ("史実の出来事が起きた。", "A historical event has taken place.",
                              "Произошло историческое событие."),
    # Placeholder boost trigger text (the real triggers belong to the tech/civic tree).
    "LOC_JAC_EVENT_BOOST_PLACEHOLDER": ("史実の出来事の年に得られる（暫定）。",
                                        "Granted when the historical event's year arrives (placeholder).",
                                        "Даётся в год исторического события (временно)."),
    # Button of the advisor popup (E5a).
    "LOC_JAC_EVENT_ADVISOR_OK": ("承知した", "Understood", "Понятно"),
}


def event_type(event_id: str) -> str:
    assert event_id.startswith("EV_"), event_id
    return "JAC_EVENT_" + event_id[3:]


def civ(faction: str) -> str:
    return "CIVILIZATION_JAC_" + faction


def sql(value) -> str:
    if value is None:
        return "NULL"
    if isinstance(value, int):
        return str(value)
    return "'" + str(value).replace("'", "''") + "'"


def resource_sites() -> tuple[dict, dict, set[str]]:
    """(point id by (resource, lon, lat), trade-access id by the same key, reserved site ids of the resources mod)."""
    data = json.loads(RESOURCES_JSON.read_text(encoding="utf-8"))
    points = {(g["civ6_id"], *p["coordinates"]): p["id"] for g in data["resource_groups"] for p in g["points"]}
    trades = {(t["civ6_id"], *t["coordinates"]): t["id"] for t in data["trade_access"]}
    reserved = set(re.findall(r'\{ id="([^"]+)"', SITES_LUA.read_text(encoding="utf-8")))
    return points, trades, reserved


def site_of(resource: dict, points: dict, trades: dict) -> str:
    m = re.search(r"\((-?[\d.]+), (-?[\d.]+)\)$", resource["location"])
    key = (resource["resource_id"], float(m[2]), float(m[1]))  # location is (lat, lon); the data is [lon, lat]
    table = trades if resource["action"] == "trade_access" else points
    if key not in table:
        raise SystemExit(f"no resource site for {resource['location']}")
    return table[key]


def starting_knowledge() -> set[str]:
    """Techs/civics every house has from the start (documents/poc/sengoku-starting-knowledge.md), read from the
    script that grants them."""
    lua = STARTING_KNOWLEDGE_LUA.read_text(encoding="utf-8")
    found = set(re.findall(r'"((?:TECH|CIVIC)_JAC_SENGOKU_[A-Z_]+)"', lua))
    assert found, "no starting knowledge found in " + str(STARTING_KNOWLEDGE_LUA)
    return found


def build() -> tuple[str, str]:
    events = json.loads(EVENTS_JSON.read_text(encoding="utf-8"))["events"]
    # A node every house knows at the start cannot mean "researched earlier than history": as a prerequisite it
    # fires the event on turn 1 for everyone, as a boost it is wasted. The list must not name one.
    start = starting_knowledge()
    for e in events:
        known = sorted({r["id"] for r in e["earlier_by"] + [e["boost"] or {"id": None}]} & start)
        if known:
            raise SystemExit(f"{e['id']}: earlier_by/boost names starting knowledge {known}")
    points, trades, reserved = resource_sites()
    rows, prereq_techs, prereq_civics, targets, renames, resources, boosts = [], [], [], [], [], [], {}
    text = {key: dict(zip((l for _, l in LANGS), values)) for key, values in FIXED_TEXT.items()}
    for e in events:
        et = event_type(e["id"])
        suffix = et[len("JAC_EVENT_"):]
        name_key = f"LOC_JAC_EVENT_{suffix}_NAME"
        text[name_key] = {lang: e["name"][k] for k, lang in LANGS}
        house = generic_key = None
        if e["house_specific"]:
            house, generic_key = civ(e["house"]), f"LOC_JAC_EVENT_{suffix}_GENERIC_NAME"
            text[generic_key] = {lang: e["generic_name"][k] for k, lang in LANGS}
        description = "LOC_JAC_EVENT_SUMMARY"
        if e["advisor_popup"]:
            description = f"LOC_JAC_EVENT_{suffix}_ADVISOR"
            text[description] = {lang: e["advisor_popup"][k] for k, lang in LANGS}
        boost = e["boost"] or {}
        boost_tech = boost.get("id") if boost.get("type") == "tech" else None
        boost_civic = boost.get("id") if boost.get("type") == "civic" else None
        if boost:
            boosts[boost["id"]] = boost["type"]
        rows.append((et, e["year"], e["month"], boost_tech, boost_civic, name_key, description, house, generic_key,
                     1 if e["advisor_popup"] else 0))
        for r in e["earlier_by"]:
            (prereq_techs if r["type"] == "tech" else prereq_civics).append((et, r["id"]))
        if "ALL" not in e["affected_factions"]:
            targets.extend((et, civ(f)) for f in e["affected_factions"])
        if r := e.get("rename"):
            romaji, en, ru = RENAME_NAMES[r["to"]]
            key = f"LOC_JAC_EVENT_{romaji}_CIV_NAME"
            text[key] = {"ja_JP": r["to"], "en_US": en, "ru_RU": ru}
            renames.append((et, civ(r["faction"]), key))
        if r := e.get("resource"):
            site = site_of(r, points, trades)
            if r["action"] == "reveal" and site not in reserved:
                raise SystemExit(f"{e['id']}: {site} is not a reserved site of the resources mod")
            resources.append((et, site, r["action"], civ(r["owner_to"]) if r.get("owner_to") else None))

    def values(table: list[tuple]) -> str:
        return ",\n".join("\t(" + ", ".join(sql(v) for v in row) + ")" for row in table) + ";\n"

    out = [
        "-- Generated by tools/generate_sengoku_events.py from documents/spec/data/sengoku-events.json. Do not edit.\n",
        "-- Values are provisional (research draft); see documents/poc/historical-events.md.\n\n",
        "INSERT INTO JAC_HistoricalEvents (EventType, Pack, Year, Month, BoostTech, BoostCivic, Name, Description, "
        "House, GenericName, Advisor) VALUES\n",
        values([(et, "SENGOKU", *rest) for et, *rest in rows]),
        "\nINSERT INTO JAC_HistoricalEventPrereqTechs (EventType, TechnologyType) VALUES\n", values(prereq_techs),
        "\nINSERT INTO JAC_HistoricalEventPrereqCivics (EventType, CivicType) VALUES\n", values(prereq_civics),
        "\nINSERT INTO JAC_HistoricalEventTargets (EventType, CivilizationType) VALUES\n", values(targets),
        "\nINSERT INTO JAC_HistoricalEventRenames (EventType, CivilizationType, Name) VALUES\n", values(renames),
        "\n-- Records only. reveal: the reserved site of the resources mod, which reveals it in its own year or on its\n"
        "-- research (SSOT 3.2: the event boosts that research instead); trade access is not implemented; Q43: ownership\n"
        "-- is not transferred automatically.\n",
        "INSERT INTO JAC_HistoricalEventResources (EventType, SiteId, Action, OwnerTo) VALUES\n", values(resources),
        "\n-- TriggerBoost needs a Boosts row. Placeholder rows (暫定) for the event boosts the tree does not define;\n"
        "-- the class never triggers by itself (official: Base/Assets/Gameplay/Data/Technologies.xml:440,505).\n",
    ]
    for boost_id, kind in sorted(boosts.items()):
        column, table, key = ("TechnologyType", "Technologies", "TechnologyType") if kind == "tech" else (
            "CivicType", "Civics", "CivicType")
        out.append(
            f"INSERT INTO Boosts ({column}, Boost, TriggerDescription, TriggerLongDescription, BoostClass)\n"
            f"SELECT {key}, 40, 'LOC_JAC_EVENT_BOOST_PLACEHOLDER', 'LOC_JAC_EVENT_BOOST_PLACEHOLDER', "
            f"'BOOST_TRIGGER_NONE_LATE_GAME_CRITICAL_TECH' FROM {table}\n"
            f"WHERE {key} = {sql(boost_id)} AND NOT EXISTS (SELECT 1 FROM Boosts WHERE {column} = {sql(boost_id)});\n")
    xml = ['<?xml version="1.0" encoding="utf-8"?>\n',
           "<!-- Generated by tools/generate_sengoku_events.py from documents/spec/data/sengoku-events.json. -->\n",
           "<GameInfo>\n\t<LocalizedText>\n"]
    for key in sorted(text):
        for _, lang in LANGS:
            xml.append(f'\t\t<Row Tag="{key}" Language="{lang}"><Text>{escape(text[key][lang])}</Text></Row>\n')
    xml.append("\t</LocalizedText>\n</GameInfo>\n")
    return "".join(out), "".join(xml)


def outputs() -> dict[Path, str]:
    sql_text, xml_text = build()
    return {MOD / "Data/JAC_HistoricalEvents_Sengoku.sql": sql_text,
            MOD / "Text/JAC_HistoricalEvents_Text.xml": xml_text}


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
    print("Sengoku event files current" if args.check else "Sengoku event files generated")
