"""Regenerate the checked-in Civ6 config, text and map scenarios."""

from __future__ import annotations

import json
from pathlib import Path
import sqlite3
import xml.etree.ElementTree as ET

from generate import EXTRA_CAPITAL_NAMES, MOD, ROOT, SCALES, SEATS, SEAT_INDEX
from start_1534 import data as research_data


MOD_ID = "403be889-1455-4de7-967b-ad54c0ff3431"
BANSEIKI_RULESET_ID = "bd7a82c5-6b38-4b69-a7f5-8dc9af534249"
BUNDLE0_ID = "dfa0717e-4d0b-4370-b693-27fc6758571b"
CUSTOM_CIVS_ID = "ba3a1c58-9287-4654-a22f-85f9e6841e9b"
GROWTH_UI_ID = "7696e937-2044-4f3c-8e13-4e27632e2710"
# The Japan map is played with the Sengoku pack (SSOT 3.2); the integrated live run used both.
SENGOKU_TREES_ID = "26c967d0-c10d-4eeb-8aaf-c83dc7e621e7"


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
            ET.SubElement(maps, "Row", Domain="Maps:JAC_Banseiki", File=file,
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
    add("LOC_ABILITY_JAC_DOMAIN_CROSSING_NAME",
        "Free passage across domains", "領土境の自由通行", "Свободный проход через владения")
    add("LOC_ABILITY_JAC_DOMAIN_CROSSING_DESCRIPTION",
        "May enter another faction's territory without an Open Borders agreement.",
        "開放国境の協定なしに他勢力の領土へ進入できる。",
        "Может входить во владения других держав без договора об открытых границах.")
    write_xml(MOD / "Text" / "JAC_JapanMapScale_Text.xml", root)


def scenarios(human_slug: str = "ODA") -> None:
    manifest = json.loads((MOD / "Maps" / "manifest.json").read_text(encoding="utf-8"))
    output = ROOT / "tools" / "civ6-autotest" / "scenarios"
    castle_counts = {seat.slug: 0 for seat in SEATS}
    for castle in research_data()["castles"]:
        castle_counts[castle["owner_faction_1534_draft"]] += 1
    expected_counts = {seat.slug: castle_counts[seat.slug] +
                       int(seat.slug in EXTRA_CAPITAL_NAMES) for seat in SEATS}
    expected_total = sum(expected_counts.values())
    ordered_seats = [next(seat for seat in SEATS if seat.slug == human_slug)] + [
        seat for seat in SEATS if seat.slug != human_slug]
    players = [{"leader": seat.leader, "civ": seat.civ, "human": i == 0}
               for i, seat in enumerate(ordered_seats)]
    for item in manifest:
        km = item["km_per_tile"]
        width, height = item["width"], item["height"]
        grid_check = ("local w,h=Map.GetGridSize(); if w~=" + str(width) + " or h~=" + str(height) +
                      " then error('map grid mismatch '..w..'x'..h) end; ")
        counts = {seat.slug: 0 for seat in SEATS}
        for city in item["cities"]:
            counts[city["owner"]] += 1
        if counts != expected_counts:
            raise ValueError(f"{km} km city inventory differs from research: {counts}")
        start_by_seat = {s["seat"]: s for s in item["starts"]}
        expected = "{" + ",".join("{" + str(start_by_seat[seat.slug]["col"]) + "," +
                                  str(start_by_seat[seat.slug]["row"]) + "," +
                                  str(counts[seat.slug]) + "}" for seat in SEATS) + "}"
        initial_counts = "{" + ",".join(str(counts[seat.slug]) for seat in ordered_seats) + "}"
        count_names = "{" + ",".join("'" + seat.slug + "'" for seat in ordered_seats) + "}"
        leaders = "{" + ",".join("'" + seat.leader + "'" for seat in SEATS) + "}"
        civs = "{" + ",".join("'" + seat.civ + "'" for seat in SEATS) + "}"
        city_owners = "{" + ",".join(f"[{city['plot']}]={SEAT_INDEX[city['owner']]}"
                                    for city in item["cities"]) + "}"
        with sqlite3.connect(MOD / "Maps" / item["file"]) as db:
            map_owner = dict(db.execute("SELECT ID, Owner FROM PlotOwners"))
            source_owners = "".join(chr(65 + map_owner[plot]) for (plot,) in db.execute(
                "SELECT ID FROM Plots WHERE ContinentType='CONTINENT_ASIA' ORDER BY ID"))
        minors = ("local minors=0; local freeCities=0; "
                  "for _,m in ipairs(PlayerManager.GetAliveMinors()) do "
                  "if PlayerConfigurations[m:GetID()]:GetCivilizationTypeName()=='CIVILIZATION_FREE_CITIES' "
                  "then freeCities=freeCities+m:GetCities():GetCount() else minors=minors+1 end end; "
                  "if minors~=0 then error('city-states present: '..minors) end; "
                  "if freeCities~=0 then error('Free Cities own '..freeCities..' cities') end; ")
        land = ("local land=0; local sourceOwners='" + source_owners + "'; "
                "for index=0,w*h-1 do local tile=Map.GetPlotByIndex(index); "
                "if tile and not tile:IsWater() then land=land+1; if tile:GetOwner()<0 then "
                "error('unowned land plot '..index) end; "
                "local encoded=sourceOwners:byte(land); if not encoded then error('extra land plot '..index) end; "
                "local source=encoded-65; "
                "if tile:GetOwner()~=gameSlotForMapId[source] then "
                "error('initial territory owners mismatch '..index) end end end; "
                f"if land~={item['land_tiles']} then error('land tile count '..land) end; ")
        settlers = ("for i=0,21 do local p=Players[i]; if p then "
                    "for _,unit in p:GetUnits():Members() do "
                    "if GameInfo.Units[unit:GetUnitType()].UnitType=='UNIT_SETTLER' then "
                    "error('settler in slot '..i) end end end end; ")
        passage = (
            "local passage=nil; local diplomacy=Players[localPlayer]:GetDiplomacy(); "
            "for _,unit in Players[localPlayer]:GetUnits():Members() do "
            "local reachable=UnitManager.GetReachableMovement(unit); "
            "if reachable then for _,index in ipairs(reachable) do "
            "local plot=Map.GetPlotByIndex(index); local owner=plot and plot:GetOwner() or -1; "
            "if owner>=0 and owner~=localPlayer and not diplomacy:IsAtWarWith(owner) "
            "and not diplomacy:HasOpenBordersFrom(owner) then "
            "local params={[UnitOperationTypes.PARAM_X]=plot:GetX(),"
            "[UnitOperationTypes.PARAM_Y]=plot:GetY()}; "
            "if UnitManager.CanStartOperation(unit,UnitOperationTypes.MOVE_TO,nil,params) then "
            "passage=tostring(owner)..':'..tostring(index); break end end end end; "
            "if passage then break end end; "
            "if not passage then error('no initial route into peaceful foreign territory without Open Borders') end; "
        )
        if human_slug != "ODA":
            passage = "local passage='not checked in faction mapping scenario'; "
        loyalty = (
            "local humanCapital=Players[localPlayer]:GetCities():GetCapitalCity(); "
            "local loyalty=humanCapital:GetCulturalIdentity():GetLoyaltyPerTurn(); "
            "local minimumLoyaltyPerTurn=1000; for i=0,21 do "
            "for _,city in Players[i]:GetCities():Members() do "
            "local rate=city:GetCulturalIdentity():GetLoyaltyPerTurn(); "
            "if rate<minimumLoyaltyPerTurn then minimumLoyaltyPerTurn=rate end; "
            "if rate<0 then error('negative initial city loyalty/turn '..i..':'..city:GetID()..'='..rate) "
            "end end end; "
        )
        # A Free City can be reabsorbed before turn 50; record loyalty conversions during autoplay.
        loyalty_watch = (
            "if JAC_JAPAN_LOYALTY_WATCH and JAC_JAPAN_LOYALTY_WATCH.handler then "
            "Events.CulturalIdentityCityConverted.Remove(JAC_JAPAN_LOYALTY_WATCH.handler) end; "
            "JAC_JAPAN_LOYALTY_WATCH={majorDefections={}}; "
            "JAC_JAPAN_LOYALTY_WATCH.handler=function(playerID, cityID, fromPlayer) "
            "local former=fromPlayer and Players[fromPlayer]; "
            "if former and former:IsMajor() then "
            "local lost=tostring(fromPlayer)..'->'..tostring(playerID)..':'..tostring(cityID); "
            "table.insert(JAC_JAPAN_LOYALTY_WATCH.majorDefections,lost) end end; "
            "Events.CulturalIdentityCityConverted.Add(JAC_JAPAN_LOYALTY_WATCH.handler); "
        )
        start_assertion = (grid_check + minors +
                           "local localPlayer=Game.GetLocalPlayer(); "
                           "if localPlayer<0 then error('no local player') end; " +
                           f"if PlayerConfigurations[localPlayer]:GetLeaderTypeName()~='{ordered_seats[0].leader}' "
                           "then error('local leader mismatch') end; " +
                           "if #PlayerManager.GetAliveMajors()~=22 then error('major count') end; " +
                           "local expected=" + expected + "; local leaders=" + leaders +
                           "; local civs=" + civs + "; "
                           "local mapIdForLeader={}; for mapId=0,21 do "
                           "mapIdForLeader[leaders[mapId+1]]=mapId end; "
                           "local gameSlotForMapId={}; for i=0,21 do "
                           "local source=mapIdForLeader[PlayerConfigurations[i]:GetLeaderTypeName()]; "
                           "if source==nil or gameSlotForMapId[source]~=nil then "
                           "error('leader to map player mapping '..i) end; "
                           "gameSlotForMapId[source]=i end; "
                           "local cityOwners=" + city_owners + "; local seenCities={}; "
                           "local garrisons={}; for i=0,21 do for _,unit in Players[i]:GetUnits():Members() do "
                           "local info=GameInfo.Units[unit:GetUnitType()]; "
                           "if (info.Combat or 0)>0 or (info.RangedCombat or 0)>0 or (info.Bombard or 0)>0 then "
                           "garrisons[i..':'..(unit:GetY()*w+unit:GetX())]=true end end end; "
                           "local palaceType=GameInfo.Buildings['BUILDING_PALACE'].Index; "
                           "local totalCities=0; for i=0,21 do local p=Players[i]; if not p or not p:IsAlive() then "
                           "error('missing player '..i) end; "
                           "local source=mapIdForLeader[PlayerConfigurations[i]:GetLeaderTypeName()]; "
                           "if PlayerConfigurations[i]:GetCivilizationTypeName()~=civs[source+1] then "
                           "error('faction mismatch '..i) end; "
                           "local e=expected[source+1]; if p:GetCities():GetCount()~=e[3] then "
                           "error('city count '..i..'='..p:GetCities():GetCount()..' expected '..e[3]) end; "
                           "totalCities=totalCities+p:GetCities():GetCount(); "
                           "local capital=p:GetCities():GetCapitalCity(); "
                           "for _,city in p:GetCities():Members() do local plot=city:GetY()*w+city:GetX(); "
                           "local sourceOwner=cityOwners[plot]; "
                           "if sourceOwner==nil or gameSlotForMapId[sourceOwner]~=i or seenCities[plot] then "
                           "error('city position/owner mismatch '..plot) end; "
                           "seenCities[plot]=true; local isCapital=capital and city:GetX()==capital:GetX() "
                           "and city:GetY()==capital:GetY(); "
                           "if city:GetBuildings():HasBuilding(palaceType)~=isCapital then "
                           "error('palace mismatch '..plot) end; "
                           "if isCapital and not garrisons[i..':'..plot] then error('capital garrison missing '..plot) end end; "
                           "if not capital or "
                           "capital:GetX()~=e[1] or capital:GetY()~=e[2] then "
                           "error('capital mismatch '..i) end end; " +
                           f"if totalCities~={expected_total} then error('total city count '..totalCities) end; " +
                           land + settlers + passage + loyalty + loyalty_watch +
                           f"return '22 majors, 0 city-states, {expected_total} cities/capitals, all land owned, no settlers; "
                           "passage='..passage..'; capital loyalty/turn='..loyalty.."
                           "'; minimum loyalty/turn='..minimumLoyaltyPerTurn")
        # Only start-state facts belong in the start check: after turns, AI settlers and
        # razed plots are normal (live 2026-09-29), so the snapshot leaves them out.
        snapshot = (grid_check + minors +
                    "if Game.GetCurrentGameTurn()<51 or not Players[0]:IsAlive() then "
                    "error('50-turn state lost') end; "
                    "local initialCounts=" + initial_counts + "; local names=" + count_names + "; "
                    "local values={tostring(Game.GetCurrentGameTurn()),tostring(#PlayerManager.GetAliveMajors())}; "
                    "local countReport={}; local cityRows={}; local unitPlots={}; "
                    "for i=0,21 do for _,unit in Players[i]:GetUnits():Members() do "
                    "local info=GameInfo.Units[unit:GetUnitType()]; "
                    "if (info.Combat or 0)>0 or (info.RangedCombat or 0)>0 or (info.Bombard or 0)>0 then "
                    "unitPlots[i..':'..(unit:GetY()*w+unit:GetX())]=true end end end; "
                    "local palaceType=GameInfo.Buildings['BUILDING_PALACE'].Index; "
                    "for i=0,21 do local p=Players[i]; local count=p:GetCities():GetCount(); "
                    "values[#values+1]=tostring(count); values[#values+1]=tostring(p:IsAlive()); "
                    "countReport[#countReport+1]=names[i+1]..'='..initialCounts[i+1]..'->'..count; "
                    "for _,city in p:GetCities():Members() do "
                    "local plot=city:GetY()*w+city:GetX(); "
                    "local identity=city:GetCulturalIdentity(); "
                    "local rate=identity:GetLoyaltyPerTurn(); "
                    "if rate<0 then error('negative city loyalty/turn '..i..':'..city:GetID()..'='..rate) "
                    "end; cityRows[#cityRows+1]=string.format('%06d',plot)..':'..city:GetOwner()..':'.."
                    "tostring(city:GetBuildings():HasBuilding(palaceType))..':'.."
                    "tostring(unitPlots[i..':'..plot] or false)..':'.."
                    "tostring(identity:GetLoyalty())..':'..tostring(rate) end end; "
                    "table.sort(cityRows); values[#values+1]=table.concat(cityRows,'|'); "
                    "local hash=0; "
                    "for index=0,w*h-1 do local tile=Map.GetPlotByIndex(index); "
                    "if tile and not tile:IsWater() then hash=(hash*131+tile:GetOwner()+1)%2147483647 end end; "
                    "values[#values+1]=tostring(hash); "
                    "return 'city counts (start->now)='..table.concat(countReport,'|')..'; state='.."
                    "table.concat(values, ',')")
        after_autoplay = (
            "if not JAC_JAPAN_LOYALTY_WATCH then error('loyalty conversion watch lost') end; "
            "if #JAC_JAPAN_LOYALTY_WATCH.majorDefections~=0 then "
            "error('major cities lost by loyalty during autoplay: '.."
            "table.concat(JAC_JAPAN_LOYALTY_WATCH.majorDefections,',')) end; " + snapshot
        )
        name = f"JAC-JapanMap-{km}km-01"
        data = {"name": f"JAC Japan map scale {km} km, {human_slug} human" +
                (", 50 turns" if human_slug == "ODA" else ", start mapping"),
                "judge_profile": "japan_map", "steps": [
                    {"action": "enable_mods",
                     "ids": [BANSEIKI_RULESET_ID, CUSTOM_CIVS_ID, GROWTH_UI_ID, SENGOKU_TREES_ID, MOD_ID],
                     "disable_ids": [BUNDLE0_ID]},
                    {"action": "configure", "ruleset": "RULESET_JAC_BANSEIKI",
                     "map_file": "{" + MOD_ID + "}Maps/" + item["file"], "map_size": "MAPSIZE_HUGE",
                     "speed": "GAMESPEED_STANDARD", "seed": 620929 + km, "city_states": 0,
                     "players": players},
                    {"action": "start_game", "timeout": 600},
                    {"action": "ui_lua", "state_name": "InGame", "code": start_assertion},
                    {"action": "autoplay", "turns": 50, "metrics": True, "timeout": 7200},
                    {"action": "ui_lua", "state_name": "InGame", "code": after_autoplay,
                     "capture_as": "JAPAN_STATE"},
                    {"action": "save", "name": name, "timeout": 180},
                    {"action": "exit_to_menu", "timeout": 180},
                    {"action": "load", "name": name, "timeout": 600},
                    {"action": "ui_lua", "state_name": "InGame", "code": snapshot,
                     "equals_capture": "JAPAN_STATE"},
                    {"action": "autoplay", "turns": 1, "metrics": True, "timeout": 900},
                ]}
        if human_slug != "ODA":
            data["steps"] = data["steps"][:4]
        suffix = "" if human_slug == "ODA" else f"-{human_slug.lower()}"
        (output / f"japan-map-{km}km{suffix}.json").write_text(
            json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def resource_scenario() -> None:
    """SSOT 3.2 live check on the 20 km map: custom resources, tech gates, year-driven site release."""
    manifest = json.loads((MOD / "Maps" / "manifest.json").read_text(encoding="utf-8"))
    item = next(entry for entry in manifest if entry["km_per_tile"] == 20)
    assert SEATS[0].slug == "ODA"
    reserved = [r for r in item["resource_candidates"] if r["placement_status"] == "reserved_for_later_release"]
    sites = "{" + ",".join(f"{{'{r['id']}',{r['plot']},'{r['resource_id']}',{r['reveal_year']}}}"
                           for r in sorted(reserved, key=lambda r: (r["reveal_year"], r["id"]))) + "}"
    counts = item["resource_placement"]["count_by_type"]
    grid = (f"local w,h=Map.GetGridSize(); if w~={item['width']} or h~={item['height']} then "
            "error('map grid mismatch '..w..'x'..h) end; ")
    static = (
        grid +
        "for _,id in ipairs({'RESOURCE_JAC_GOLD','RESOURCE_JAC_LACQUER'}) do local r=GameInfo.Resources[id]; "
        "if not r then error('missing resource '..id) end; "
        "if r.PrereqTech then error(id..' must not be gated by a tech') end end; "
        "for r in GameInfo.Resources() do if r.PrereqTech and not string.find(r.PrereqTech,'TECH_JAC_SENGOKU_',1,true) then "
        "error('official tech gate on '..r.ResourceType..': '..r.PrereqTech) end end; "
        "if GameInfo.Resources['RESOURCE_IRON'].PrereqTech~='TECH_JAC_SENGOKU_IRON_FORGING' then error('iron gate') end; "
        "for r in GameInfo.Resource_Harvests() do if r.PrereqTech and not string.find(r.PrereqTech,'TECH_JAC_SENGOKU_',1,true) then "
        "error('official tech harvest gate on '..r.ResourceType) end; "
        "if r.ResourceType=='RESOURCE_RICE' and r.PrereqTech~='TECH_JAC_SENGOKU_IRRIGATION_WORKS' then "
        "error('rice harvest gate '..tostring(r.PrereqTech)) end end; "
        "local hills=false; for r in GameInfo.Resource_ValidTerrains() do "
        "if r.ResourceType=='RESOURCE_SILVER' and r.TerrainType=='TERRAIN_GRASS_HILLS' then hills=true end end; "
        "if not hills then error('silver is not valid on grass hills') end; "
        "local counts={}; for index=0,w*h-1 do local t=Map.GetPlotByIndex(index):GetResourceType(); "
        "if t>=0 then local name=GameInfo.Resources[t].ResourceType; counts[name]=(counts[name] or 0)+1 end end; "
        f"for name,expected in pairs({{RESOURCE_JAC_GOLD={counts['RESOURCE_JAC_GOLD']},"
        f"RESOURCE_JAC_LACQUER={counts['RESOURCE_JAC_LACQUER']},RESOURCE_SILVER={counts['RESOURCE_SILVER']}}}) do "
        "if counts[name]~=expected then error(name..' on map: '..tostring(counts[name])..', expected '..expected) end end; "
        "return 'custom resources defined, tech gates Sengoku-only, start resources on the map'")

    def released(year: int) -> str:
        return (grid + f"local sites={sites}; local shown=0; for _,s in ipairs(sites) do "
                "local t=Map.GetPlotByIndex(s[2]):GetResourceType(); "
                f"if s[4]<={year} then if t~=GameInfo.Resources[s[3]].Index then error('site not released: '..s[1]) end; "
                "shown=shown+1 elseif t~=-1 then error('site released too early: '..s[1]) end end; "
                f"return 'year {year}: '..shown..' of '..#sites..' deferred sites on the map'")

    players = [{"leader": seat.leader, "civ": seat.civ, "human": i == 0} for i, seat in enumerate(SEATS)]
    steps = [
        {"action": "enable_mods",
         "ids": [BANSEIKI_RULESET_ID, CUSTOM_CIVS_ID, GROWTH_UI_ID, SENGOKU_TREES_ID, MOD_ID],
         "disable_ids": [BUNDLE0_ID]},
        {"action": "configure", "ruleset": "RULESET_JAC_BANSEIKI",
         "map_file": "{" + MOD_ID + "}Maps/" + item["file"], "map_size": "MAPSIZE_HUGE",
         "speed": "GAMESPEED_STANDARD", "seed": 620960, "city_states": 0, "players": players},
        {"action": "ui_lua", "state_name": "HostGame",
         "code": "BuildHeadlessGameSetup(); g_GameParameters:FullRefresh(); "
                 "GameConfiguration.SetValue('JAC_RESOURCES_TEST_YEAR', 1541); HideGameSetup(); "
                 "return 'setup built; calendar test year 1541 on turn 1'"},
        {"action": "start_game", "timeout": 600},
        {"action": "wait_for", "condition": {"type": "log_line", "log": "Modding.log",
                                              "contains": "Applying Component - JAC_Japan_Resources (UpdateDatabase)"},
         "timeout": 60},
        {"action": "wait_for", "condition": {"type": "log_line", "log": "Lua.log",
                                              "contains": "[JAC_RESOURCES] loaded"}, "timeout": 60},
        {"action": "ui_lua", "state_name": "InGame", "code": static},
        {"action": "ui_lua", "state_name": "InGame", "code": released(1541)},
        {"action": "autoplay", "turns": 1, "timeout": 600},
        # wait_for only sees log lines written after the previous step, so it must follow the autoplay (live 2026-09-29).
        {"action": "wait_for", "condition": {"type": "log_line", "log": "Lua.log",
                                              "contains": "[JAC_RESOURCES] revealed silver-ikuno"}, "timeout": 30},
        {"action": "ui_lua", "state_name": "InGame", "code": released(1542)},
        {"action": "autoplay", "turns": 8, "timeout": 900},
        {"action": "ui_lua", "state_name": "InGame", "code": released(1550)},
    ]
    data = {"name": "JAC Japan resources: custom rows, tech gates, year-driven site release (20 km)",
            "judge_profile": "generic", "steps": steps}
    output = ROOT / "tools" / "civ6-autotest" / "scenarios"
    (output / "japan-resources.json").write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n",
                                                 encoding="utf-8")


def main() -> None:
    config()
    text()
    scenarios()
    scenarios("TAKEDA")
    resource_scenario()


if __name__ == "__main__":
    main()
