"""Regenerate the checked-in Civ6 config, placeholder civs and map scenarios."""

from __future__ import annotations

import json
from pathlib import Path
import xml.etree.ElementTree as ET

from generate import MOD, ROOT, SCALES, SEATS


MOD_ID = "403be889-1455-4de7-967b-ad54c0ff3431"
CAPITALS_JA = (
    "米沢城", "黒川城", "小田原城", "平井城", "躑躅ヶ崎館", "春日山城", "駿府", "那古野城",
    "岡崎城", "一乗谷", "室町殿", "勝瑞城", "観音寺城", "石山本願寺", "山口",
    "月山富田城", "吉田郡山城", "岡豊城", "府内", "鹿児島", "首里城", "二風谷",
)
CAPITALS_RU = (
    "Ёнэдзава", "Курокава", "Одавара", "Хираи", "Цуцудзигасаки", "Касугаяма",
    "Сумпу", "Нагоя", "Окадзаки", "Итидзёдани", "Муромати", "Сёдзуй", "Каннондзи",
    "Исияма", "Ямагути", "Гассан-Тода", "Ёсида-Корияма", "Око", "Фунай",
    "Кагосима", "Сюри", "Нибутани",
)


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
    players = ET.SubElement(root, "Players")
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
            ET.SubElement(leaders, "Row", Map=file, LeaderType=seat.proxy)
    for seat in SEATS:
        key = f"LOC_JAC_JAPAN_{seat.slug}_NAME"
        ET.SubElement(players, "Row", Domain="Players:Expansion2_Players",
                      CivilizationType=seat.civ, LeaderType=seat.proxy,
                      CivilizationName=key, CivilizationIcon="ICON_CIVILIZATION_JAPAN",
                      LeaderName="LOC_" + seat.proxy + "_NAME",
                      LeaderIcon="ICON_" + seat.proxy,
                      CivilizationAbilityName="LOC_JAC_JAPAN_SCALE_POC_ABILITY_NAME",
                      CivilizationAbilityDescription="LOC_JAC_JAPAN_SCALE_POC_ABILITY_DESCRIPTION",
                      CivilizationAbilityIcon="ICON_CIVILIZATION_JAPAN",
                      LeaderAbilityName="LOC_JAC_JAPAN_SCALE_POC_ABILITY_NAME",
                      LeaderAbilityDescription="LOC_JAC_JAPAN_SCALE_POC_ABILITY_DESCRIPTION",
                      LeaderAbilityIcon="ICON_" + seat.proxy)
    write_xml(MOD / "Config" / "JAC_JapanMapScale_Config.xml", root)


def gameplay() -> None:
    lines = ["-- 22 PoC civilization identities; official leaders supply temporary AI/art.",
             "INSERT INTO Types (Type, Kind) VALUES"]
    lines.extend("  ('" + seat.civ + "', 'KIND_CIVILIZATION')" +
                 ("," if i + 1 < len(SEATS) else ";") for i, seat in enumerate(SEATS))
    lines += ["", "INSERT INTO Civilizations",
              "  (CivilizationType, Name, Description, Adjective, StartingCivilizationLevelType, RandomCityNameDepth, Ethnicity) VALUES"]
    lines.extend("  ('" + seat.civ + "', 'LOC_JAC_JAPAN_" + seat.slug + "_NAME', 'LOC_JAC_JAPAN_" +
                 seat.slug + "_NAME', 'LOC_JAC_JAPAN_" + seat.slug +
                 "_NAME', 'CIVILIZATION_LEVEL_FULL_CIV', 10, 'ETHNICITY_ASIAN')" +
                 ("," if i + 1 < len(SEATS) else ";") for i, seat in enumerate(SEATS))
    lines += ["", "INSERT INTO CivilizationLeaders (CivilizationType, LeaderType, CapitalName) VALUES"]
    lines.extend("  ('" + seat.civ + "', '" + seat.proxy + "', 'LOC_JAC_JAPAN_" + seat.slug +
                 "_CAPITAL')" + ("," if i + 1 < len(SEATS) else ";")
                 for i, seat in enumerate(SEATS))
    lines += ["", "INSERT INTO CityNames (CivilizationType, CityName, SortIndex) VALUES"]
    lines.extend("  ('" + seat.civ + "', 'LOC_JAC_JAPAN_" + seat.slug + "_CAPITAL', -1)" +
                 ("," if i + 1 < len(SEATS) else ";") for i, seat in enumerate(SEATS))
    lines += ["", "-- Share Japan's city-name pool so expansion after the capital remains possible."]
    for seat in SEATS:
        lines += ["INSERT INTO CityNames (CivilizationType, CityName, SortIndex)",
                  f"SELECT '{seat.civ}', CityName, SortIndex FROM CityNames",
                  "WHERE CivilizationType = 'CIVILIZATION_JAPAN' ORDER BY ID;", ""]
    path = MOD / "Data" / "JAC_JapanMapScale_Gameplay.sql"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")


def text() -> None:
    root = ET.Element("GameData")
    localized = ET.SubElement(root, "LocalizedText")

    def add(key: str, en: str, ja: str, ru: str) -> None:
        for lang, value in (("en_US", en), ("ja_JP", ja), ("ru_RU", ru)):
            ET.SubElement(ET.SubElement(localized, "Row", Tag=key, Language=lang), "Text").text = value

    add("LOC_JAC_JAPAN_SCALE_POC_ABILITY_NAME", "Map scale test", "地図縮尺試験", "Проверка масштаба карты")
    add("LOC_JAC_JAPAN_SCALE_POC_ABILITY_DESCRIPTION",
        "Temporary faction identity for the map stability test; no special ability.",
        "地図安定性試験用の仮勢力。固有能力はありません。",
        "Временная фракция для проверки карты; особой способности нет.")
    for km in SCALES:
        add(f"LOC_JAC_JAPAN_{km}_NAME", f"Japan archipelago {km} km/hex",
            f"日本列島 {km} km/タイル", f"Японский архипелаг {km} км/гекс")
        add(f"LOC_JAC_JAPAN_{km}_DESCRIPTION",
            f"Fixed 22-faction Japan scale trial, {km} km between opposite hex sides.",
            f"22勢力の固定日本列島地図。六角形の対辺間が約{km} km。",
            f"Фиксированная карта Японии для 22 фракций; около {km} км между сторонами гекса.")
    for index, seat in enumerate(SEATS):
        add(f"LOC_JAC_JAPAN_{seat.slug}_NAME", seat.en, seat.ja, seat.ru)
        add(f"LOC_JAC_JAPAN_{seat.slug}_CAPITAL", seat.castle,
            CAPITALS_JA[index], CAPITALS_RU[index])
    write_xml(MOD / "Text" / "JAC_JapanMapScale_Text.xml", root)


def scenarios() -> None:
    manifest = json.loads((MOD / "Maps" / "manifest.json").read_text(encoding="utf-8"))
    output = ROOT / "tools" / "civ6-autotest" / "scenarios"
    players = [{"leader": seat.proxy, "civ": seat.civ, "human": i == 0}
               for i, seat in enumerate(SEATS)]
    for item in manifest:
        km = item["km_per_tile"]
        width, height = item["width"], item["height"]
        grid_check = ("local w,h=Map.GetGridSize(); if w~=" + str(width) + " or h~=" + str(height) +
                      " then error('map grid mismatch '..w..'x'..h) end; ")
        expected = "{" + ",".join("{" + str(s["col"]) + "," + str(s["row"]) + "}"
                                  for s in item["starts"]) + "}"
        start_assertion = (grid_check + "local expected=" + expected + "; "
                           "for i=0,21 do local p=Players[i]; if not p or not p:IsAlive() then "
                           "error('missing player '..i) end; local e=expected[i+1]; local found=false; "
                           "for _,unit in p:GetUnits():Members() do if Map.GetPlotDistance(unit:GetX(),"
                           "unit:GetY(),e[1],e[2])<=2 then found=true end end; "
                           "if not found then error('start moved from seat '..i) end end; "
                           "return '22 player starts on " + str(km) + "km map'")
        loaded_assertion = (grid_check + "if Game.GetCurrentGameTurn()<51 or "
                            "not Players[0]:IsAlive() then error('loaded game state lost') end; "
                            "return 'loaded 22-slot map at turn '..Game.GetCurrentGameTurn()")
        name = f"JAC-JapanMap-{km}km-01"
        data = {"name": f"JAC Japan map scale {km} km, 22 players, 50 turns",
                "judge_profile": "japan_map", "steps": [
                    {"action": "enable_mods", "ids": [MOD_ID],
                     "disable_ids": ["dfa0717e-4d0b-4370-b693-27fc6758571b",
                                     "ba3a1c58-9287-4654-a22f-85f9e6841e9b",
                                     "7696e937-2044-4f3c-8e13-4e27632e2710"]},
                    {"action": "configure", "ruleset": "RULESET_EXPANSION_2",
                     "map_file": "{" + MOD_ID + "}Maps/" + item["file"], "map_size": "MAPSIZE_HUGE",
                     "speed": "GAMESPEED_STANDARD", "seed": 620929 + km, "players": players},
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
    gameplay()
    text()
    scenarios()


if __name__ == "__main__":
    main()
