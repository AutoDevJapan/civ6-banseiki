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
        for settlers in (False, True):
            suffix = "_Settlers" if settlers else ""
            file = f"{{{MOD_ID}}}Maps/JAC_Japan_{km}km{suffix}.Civ6Map"
            mode = "SETTLERS" if settlers else "HISTORICAL"
            ET.SubElement(maps, "Row", Domain="Maps:Expansion2Maps", File=file,
                          Name=f"LOC_JAC_JAPAN_{km}_{mode}_NAME",
                          Description=f"LOC_JAC_JAPAN_{km}_{mode}_DESCRIPTION", Image="Map_Earth",
                          StaticMap="1", RequiresUniqueLeaders="1",
                          SortIndex=str(100 + km + (50 if settlers else 0)))
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
        for mode, en, ja, ru in (("HISTORICAL", "historical territory", "史実の領土", "исторические владения"),
                                  ("SETTLERS", "from settlers", "開拓者から", "с поселенцев")):
            add(f"LOC_JAC_JAPAN_{km}_{mode}_NAME",
                f"Japan {km} km — start: {en}", f"日本列島 {km} km — 開始方法: {ja}",
                f"Япония {km} км — начало: {ru}")
            add(f"LOC_JAC_JAPAN_{km}_{mode}_DESCRIPTION",
                f"22 fixed 1534 factions; start with {en}.",
                f"1534年の22勢力。開始方法は{ja}。",
                f"22 державы 1534 года; начало: {ru}.")
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
        counts = {seat.slug: 0 for seat in SEATS}
        for city in item["cities"]:
            counts[city["owner"]] += 1
        expected = "{" + ",".join("{" + str(s["col"]) + "," + str(s["row"]) + "," +
                                  str(counts[s["seat"]]) + "}" for s in item["starts"]) + "}"
        leaders = "{" + ",".join("'" + seat.leader + "'" for seat in SEATS) + "}"
        civs = "{" + ",".join("'" + seat.civ + "'" for seat in SEATS) + "}"
        minors = ("local minors=0; for _,m in ipairs(PlayerManager.GetAliveMinors()) do "
                  "if PlayerConfigurations[m:GetID()]:GetCivilizationTypeName()~='CIVILIZATION_FREE_CITIES' "
                  "then minors=minors+1 end end; if minors~=0 then "
                  "error('city-states present: '..minors) end; ")
        land = ("local land=0; for index=0,w*h-1 do local tile=Map.GetPlotByIndex(index); "
                "if tile and not tile:IsWater() then land=land+1; if tile:GetOwner()<0 then "
                "error('unowned land plot '..index) end end end; "
                f"if land~={item['land_tiles']} then error('land tile count '..land) end; ")
        settlers = ("for i=0,21 do local p=Players[i]; if p then "
                    "for _,unit in p:GetUnits():Members() do "
                    "if GameInfo.Units[unit:GetUnitType()].UnitType=='UNIT_SETTLER' then "
                    "error('settler in slot '..i) end end end end; ")
        start_assertion = (grid_check + minors +
                           "if #PlayerManager.GetAliveMajors()~=22 then error('major count') end; " +
                           "local expected=" + expected + "; local leaders=" + leaders +
                           "; local civs=" + civs + "; "
                           "for i=0,21 do local p=Players[i]; if not p or not p:IsAlive() then "
                           "error('missing player '..i) end; "
                           "if PlayerConfigurations[i]:GetLeaderTypeName()~=leaders[i+1] or "
                           "PlayerConfigurations[i]:GetCivilizationTypeName()~=civs[i+1] then "
                           "error('faction mismatch '..i) end; "
                           "local e=expected[i+1]; if p:GetCities():GetCount()~=e[3] then "
                           "error('city count '..i..'='..p:GetCities():GetCount()..' expected '..e[3]) end; "
                           "local capital=p:GetCities():GetCapitalCity(); if not capital or "
                           "capital:GetX()~=e[1] or capital:GetY()~=e[2] then "
                           "error('capital mismatch '..i) end end; " + land + settlers +
                           "return '22 majors, 0 city-states, expected cities/capitals, all land owned, no settlers'")
        snapshot = (grid_check + minors + land + settlers +
                    "if Game.GetCurrentGameTurn()<51 or not Players[0]:IsAlive() then "
                    "error('50-turn state lost') end; "
                    "local values={tostring(#PlayerManager.GetAliveMajors())}; "
                    "for i=0,21 do local p=Players[i]; values[#values+1]=tostring(p:GetCities():GetCount()); "
                    "values[#values+1]=tostring(p:IsAlive()) end; "
                    "local hash=0; "
                    "for index=0,w*h-1 do local tile=Map.GetPlotByIndex(index); "
                    "if tile and not tile:IsWater() then hash=(hash*131+tile:GetOwner()+1)%2147483647 end end; "
                    "values[#values+1]=tostring(hash); "
                    "return table.concat(values, ',')")
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
                    {"action": "ui_lua", "state_name": "InGame", "code": snapshot,
                     "capture_as": "JAPAN_STATE"},
                    {"action": "save", "name": name, "timeout": 180},
                    {"action": "exit_to_menu", "timeout": 180},
                    {"action": "load", "name": name, "timeout": 600},
                    {"action": "ui_lua", "state_name": "InGame", "code": snapshot,
                     "equals_capture": "JAPAN_STATE"},
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
