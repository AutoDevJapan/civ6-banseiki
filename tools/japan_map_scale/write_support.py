"""Regenerate the checked-in Civ6 config, text and map scenarios."""

from __future__ import annotations

import json
from pathlib import Path
import xml.etree.ElementTree as ET

from generate import MOD, ROOT, SCALES, SEATS


MOD_ID = "403be889-1455-4de7-967b-ad54c0ff3431"
BANSEIKI_RULESET_ID = "bd7a82c5-6b38-4b69-a7f5-8dc9af534249"
BUNDLE0_ID = "dfa0717e-4d0b-4370-b693-27fc6758571b"
CUSTOM_CIVS_ID = "ba3a1c58-9287-4654-a22f-85f9e6841e9b"
GROWTH_UI_ID = "7696e937-2044-4f3c-8e13-4e27632e2710"


def write_xml(path: Path, root: ET.Element) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    ET.indent(root, space="  ")
    ET.ElementTree(root).write(path, encoding="utf-8", xml_declaration=True)


def config() -> None:
    root = ET.Element("GameInfo")
    maps = ET.SubElement(root, "Maps")
    sizes = ET.SubElement(root, "MapSizes")
    domains = ET.SubElement(root, "MapDomainOverrides")
    leaders = ET.SubElement(root, "MapLeaders")
    for km in SCALES:
        file = f"{{{MOD_ID}}}Maps/JAC_Japan_{km}km.Civ6Map"
        ET.SubElement(maps, "Row", Domain="Maps:Expansion2Maps", File=file, Name=f"LOC_JAC_JAPAN_{km}_NAME",
                      Description=f"LOC_JAC_JAPAN_{km}_DESCRIPTION", Image="Map_Earth",
                      StaticMap="1", RequiresUniqueLeaders="1", SortIndex=str(100 + km))
        ET.SubElement(sizes, "Row", Domain=file, MapSizeType="MAPSIZE_HUGE",
                      Name="LOC_MAPSIZE_HUGE_NAME", Description="LOC_MAPSIZE_HUGE_DESCRIPTION",
                      MinPlayers="22", MaxPlayers="22", DefaultPlayers="22",
                      MinCityStates="0", MaxCityStates="0", DefaultCityStates="0", SortIndex="10")
        ET.SubElement(domains, "Row", Map=file, ParameterId="MapSize", Domain=file)
        for seat in SEATS:
            ET.SubElement(leaders, "Row", Map=file, LeaderType=seat.leader)
    write_xml(MOD / "Config" / "JAC_JapanMapScale_Config.xml", root)


def text() -> None:
    root = ET.Element("GameData")
    localized = ET.SubElement(root, "LocalizedText")

    def add(key: str, en: str, ja: str, ru: str) -> None:
        for lang, value in (("en_US", en), ("ja_JP", ja), ("ru_RU", ru)):
            ET.SubElement(ET.SubElement(localized, "Row", Tag=key, Language=lang), "Text").text = value

    for km in SCALES:
        add(f"LOC_JAC_JAPAN_{km}_NAME", f"Japan archipelago {km} km/hex",
            f"日本列島 {km} km/タイル", f"Японский архипелаг {km} км/гекс")
        add(f"LOC_JAC_JAPAN_{km}_DESCRIPTION",
            f"Fixed 22-faction Japan scale trial, {km} km between opposite hex sides.",
            f"22勢力の固定日本列島地図。六角形の対辺間が約{km} km。",
            f"Фиксированная карта Японии для 22 фракций; около {km} км между сторонами гекса.")
    write_xml(MOD / "Text" / "JAC_JapanMapScale_Text.xml", root)


def scenarios() -> None:
    manifest = json.loads((MOD / "Maps" / "manifest.json").read_text(encoding="utf-8"))
    output = ROOT / "tools" / "civ6-autotest" / "scenarios"
    players = [{"leader": seat.leader, "civ": seat.civ, "human": i == 0}
               for i, seat in enumerate(SEATS)]
    for item in manifest:
        km = item["km_per_tile"]
        width, height = item["width"], item["height"]
        grid_check = ("local w,h=Map.GetGridSize(); if w~=" + str(width) + " or h~=" + str(height) +
                      " then error('map grid mismatch '..w..'x'..h) end; ")
        expected = "{" + ",".join("{" + str(s["col"]) + "," + str(s["row"]) + "}"
                                  for s in item["starts"]) + "}"
        leaders = "{" + ",".join("'" + seat.leader + "'" for seat in SEATS) + "}"
        majors = ("local majors=#PlayerManager.GetAliveMajors(); "
                  "local minors=#PlayerManager.GetAliveMinors(); if minors~=0 then "
                  "error('city-states present: '..minors) end; ")
        start_assertion = (grid_check + majors + "if majors~=22 then error('majors '..majors) end; "
                           "local expected=" + expected + "; local leaders=" + leaders + "; "
                           "for i=0,21 do local p=Players[i]; if not p or not p:IsAlive() then "
                           "error('missing player '..i) end; "
                           "if PlayerConfigurations[i]:GetLeaderTypeName()~=leaders[i+1] then "
                           "error('leader mismatch in slot '..i) end; local e=expected[i+1]; local best=99; "
                           "for _,unit in p:GetUnits():Members() do best=math.min(best,"
                           "Map.GetPlotDistance(unit:GetX(),unit:GetY(),e[1],e[2])) end; "
                           "if best>1 then error('start moved from seat '..i..' by '..best) end end; "
                           "return '22 leaders on their seats, no city-states, " + str(km) + "km map'")
        loaded_assertion = (grid_check + majors + "if Game.GetCurrentGameTurn()<51 or "
                            "not Players[0]:IsAlive() then error('loaded game state lost') end; "
                            "return 'loaded at turn '..Game.GetCurrentGameTurn()..' with '..majors.."
                            "' of 22 majors alive'")
        name = f"JAC-JapanMap-{km}km-01"
        data = {"name": f"JAC Japan map scale {km} km, 22 players, 50 turns",
                "judge_profile": "japan_map", "steps": [
                    {"action": "enable_mods",
                     "ids": [BANSEIKI_RULESET_ID, CUSTOM_CIVS_ID, GROWTH_UI_ID, MOD_ID],
                     "disable_ids": [BUNDLE0_ID]},
                    {"action": "configure", "ruleset": "RULESET_JAC_BANSEIKI",
                     "map_file": "{" + MOD_ID + "}Maps/" + item["file"], "map_size": "MAPSIZE_HUGE",
                     "speed": "GAMESPEED_STANDARD", "seed": 620929 + km, "city_states": 0,
                     "players": players},
                    {"action": "start_game", "timeout": 600},
                    {"action": "ui_lua", "state_name": "InGame", "code": start_assertion},
                    {"action": "autoplay", "turns": 50, "metrics": True, "timeout": 7200},
                    {"action": "save", "name": name, "timeout": 180},
                    {"action": "exit_to_menu", "timeout": 180},
                    {"action": "load", "name": name, "timeout": 600},
                    {"action": "ui_lua", "state_name": "InGame", "code": loaded_assertion},
                    {"action": "autoplay", "turns": 1, "metrics": True, "timeout": 900},
                ]}
        (output / f"japan-map-{km}km.json").write_text(
            json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main() -> None:
    config()
    text()
    scenarios()


if __name__ == "__main__":
    main()
