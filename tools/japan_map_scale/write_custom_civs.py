"""Generate the 20 additional 1534 factions from the pinned draft inventory."""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path
import xml.etree.ElementTree as ET

from generate import MOD as MAP_MOD, SEATS, SEAT_CASTLES, EXTRA_CAPITAL_NAMES
from start_1534 import HEADS, HEAD_OVERRIDES, civ_id, data, leader_id


MOD = MAP_MOD.parent / "JAC_PoC_CustomCivs"
EXISTING = {"ODA", "TAKEDA"}
EXTRA = [seat for seat in SEATS if seat.slug not in EXISTING]


def write_xml(path: Path, root: ET.Element) -> None:
    ET.indent(root, space="\t")
    ET.ElementTree(root).write(path, encoding="utf-8", xml_declaration=True)


def gameplay() -> None:
    lines = ["-- Generated 1534 factions. Oda and Takeda remain in JAC_CustomCivs_Gameplay.sql.",
             "INSERT INTO Types (Type, Kind) VALUES"]
    types = [(civ_id(s.slug), "KIND_CIVILIZATION") for s in EXTRA] + [
        (leader_id(s.slug), "KIND_LEADER") for s in EXTRA]
    types += [("TRAIT_JAC_1534_FACTION", "KIND_TRAIT"),
              ("TRAIT_JAC_1534_LEADER", "KIND_TRAIT")]
    lines.append(",\n".join(f"    ('{name}', '{kind}')" for name, kind in types) + ";")
    lines += ["INSERT INTO Civilizations (CivilizationType, Name, Description, Adjective, StartingCivilizationLevelType, RandomCityNameDepth, Ethnicity) VALUES",
              ",\n".join(f"    ('{civ_id(s.slug)}', 'LOC_{civ_id(s.slug)}_NAME', 'LOC_{civ_id(s.slug)}_DESCRIPTION', 'LOC_{civ_id(s.slug)}_ADJECTIVE', 'CIVILIZATION_LEVEL_FULL_CIV', 10, 'ETHNICITY_ASIAN')" for s in EXTRA) + ";",
              "INSERT INTO Leaders (LeaderType, Name, InheritFrom, SceneLayers) VALUES",
              ",\n".join(f"    ('{leader_id(s.slug)}', 'LOC_{leader_id(s.slug)}_NAME', 'LEADER_DEFAULT', 4)" for s in EXTRA) + ";",
              "INSERT INTO CivilizationLeaders (CivilizationType, LeaderType, CapitalName) VALUES",
              ",\n".join(f"    ('{civ_id(s.slug)}', '{leader_id(s.slug)}', 'LOC_JAC_1534_CAPITAL_{s.slug}')" for s in EXTRA) + ";",
              "INSERT INTO Traits (TraitType, Name, Description) VALUES",
              "    ('TRAIT_JAC_1534_FACTION', 'LOC_JAC_1534_FACTION_TRAIT', 'LOC_TRAIT_JAC_CLAN_POC_DESCRIPTION'),",
              "    ('TRAIT_JAC_1534_LEADER', 'LOC_JAC_1534_LEADER_TRAIT', 'LOC_TRAIT_JAC_LEADER_POC_DESCRIPTION');",
              "INSERT INTO CivilizationTraits (CivilizationType, TraitType) VALUES",
              ",\n".join(f"    ('{civ_id(s.slug)}', 'TRAIT_JAC_1534_FACTION')" for s in EXTRA) + ";",
              "INSERT INTO LeaderTraits (LeaderType, TraitType) VALUES",
              ",\n".join(f"    ('{leader_id(s.slug)}', 'TRAIT_JAC_1534_LEADER')" for s in EXTRA) + ";",
              "INSERT INTO CityNames (CivilizationType, CityName, SortIndex) VALUES",
              ",\n".join(f"    ('{civ_id(s.slug)}', 'LOC_JAC_1534_CAPITAL_{s.slug}', -1)" for s in EXTRA) + ";",
              "INSERT INTO CityNames (CivilizationType, CityName, SortIndex)\n"
              "SELECT c.CivilizationType, n.CityName, n.SortIndex FROM Civilizations c JOIN CityNames n ON n.CivilizationType='CIVILIZATION_JAPAN'\n"
              "WHERE c.CivilizationType LIKE 'CIVILIZATION_JAC_%' AND c.CivilizationType NOT IN ('CIVILIZATION_JAC_ODA','CIVILIZATION_JAC_TAKEDA');",
              "INSERT INTO LoadingInfo (LeaderType, ForegroundImage, BackgroundImage, LeaderText, PlayDawnOfManAudio) VALUES",
              ",\n".join(f"    ('{leader_id(s.slug)}', '{leader_id(s.slug)}_NEUTRAL', '{leader_id(s.slug)}_BACKGROUND', 'LOC_JAC_1534_LOADING', 0)" for s in EXTRA) + ";",
              "INSERT INTO DiplomacyInfo (Type, BackgroundImage) VALUES",
              ",\n".join(f"    ('{leader_id(s.slug)}', '{leader_id(s.slug)}_DIPLOMACY_BACKGROUND')" for s in EXTRA) + ";"]
    (MOD / "Data/JAC_Japan1534_Gameplay.sql").write_text("\n\n".join(lines) + "\n", encoding="utf-8")


def front_end() -> None:
    root = ET.Element("GameInfo")
    players = ET.SubElement(root, "Players")
    for seat in EXTRA:
        civ, leader = civ_id(seat.slug), leader_id(seat.slug)
        ET.SubElement(players, "Row", Domain="Players:Expansion2_Players",
                      CivilizationType=civ, LeaderType=leader,
                      CivilizationName=f"LOC_{civ}_NAME", CivilizationIcon=f"ICON_{civ}",
                      LeaderName=f"LOC_{leader}_NAME", LeaderIcon=f"ICON_{leader}",
                      Portrait=f"{leader}_NEUTRAL",
                      PortraitBackground=f"{leader}_BACKGROUND",
                      CivilizationAbilityName="LOC_JAC_1534_FACTION_TRAIT",
                      CivilizationAbilityDescription="LOC_TRAIT_JAC_CLAN_POC_DESCRIPTION",
                      CivilizationAbilityIcon="ICON_CIVILIZATION_JAPAN",
                      LeaderAbilityName="LOC_JAC_1534_LEADER_TRAIT",
                      LeaderAbilityDescription="LOC_TRAIT_JAC_LEADER_POC_DESCRIPTION",
                      LeaderAbilityIcon="ICON_CIVILIZATION_JAPAN")
    write_xml(MOD / "Config/JAC_Japan1534_Players.xml", root)


def icons() -> None:
    root = ET.Element("GameInfo")
    defs = ET.SubElement(root, "IconDefinitions")
    for index, seat in enumerate(EXTRA, 2):
        ET.SubElement(defs, "Row", Name=f"ICON_{civ_id(seat.slug)}",
                      Atlas="ICON_ATLAS_CIVILIZATIONS", Index="12")
        ET.SubElement(defs, "Row", Name=f"ICON_{leader_id(seat.slug)}",
                      Atlas="ICON_ATLAS_JAC_LEADERS", Index=str(index))
    write_xml(MOD / "Data/JAC_Japan1534_Icons.xml", root)


def colors() -> None:
    root = ET.Element("GameInfo")
    rows = ET.SubElement(root, "PlayerColors")
    palette = [(f"COLOR_STANDARD_{hue}_{shade}", "COLOR_STANDARD_WHITE_LT")
               for shade in ("DK", "MD")
               for hue in ("RED", "ORANGE", "YELLOW", "GREEN", "AQUA", "BLUE", "PURPLE", "MAGENTA")]
    palette += [(f"COLOR_STANDARD_{hue}_LT", "COLOR_STANDARD_WHITE_DK")
                for hue in ("RED", "GREEN", "BLUE", "PURPLE")]
    for seat, (primary, secondary) in zip(EXTRA, palette, strict=True):
        row = ET.SubElement(rows, "Row")
        for tag, value in (("Type", leader_id(seat.slug)), ("Usage", "Unique"),
                           ("PrimaryColor", primary), ("SecondaryColor", secondary),
                           ("Alt1PrimaryColor", secondary), ("Alt1SecondaryColor", primary),
                           ("Alt2PrimaryColor", "COLOR_STANDARD_WHITE_LT"),
                           ("Alt2SecondaryColor", primary),
                           ("Alt3PrimaryColor", primary),
                           ("Alt3SecondaryColor", "COLOR_STANDARD_WHITE_LT")):
            ET.SubElement(row, tag).text = value
    write_xml(MOD / "Data/JAC_Japan1534_Colors.xml", root)


def translated_text() -> None:
    source = data()
    factions = {f["id"]: f for f in source["factions"]}
    castles = {c["id"]: c for c in source["castles"]}
    root = ET.Element("GameData")
    localized = ET.SubElement(root, "LocalizedText")

    def add(tag: str, ja: str, en: str, ru: str) -> None:
        for language, value in (("ja_JP", ja), ("en_US", en), ("ru_RU", ru)):
            ET.SubElement(ET.SubElement(localized, "Row", Tag=tag, Language=language), "Text").text = value

    add("LOC_JAC_1534_FACTION_TRAIT", "1534年の勢力", "1534 faction", "Держава 1534 года")
    add("LOC_JAC_1534_LEADER_TRAIT", "1534年の当主", "1534 ruler", "Правитель 1534 года")
    add("LOC_JAC_1534_LOADING", "天文3年5月12日の日本列島。",
        "Japan, Tenbun 3, fifth month, twelfth day.",
        "Япония, Тэмбун 3, пятый месяц, двенадцатый день.")
    for seat in SEATS:
        slug = seat.slug
        ja = factions[slug]["name"]
        head = HEAD_OVERRIDES.get(slug, factions[slug]["head"]["name"])
        en, ru = seat.en, seat.ru
        _, head_en, head_ru = HEADS[slug]
        if slug not in EXISTING:
            add(f"LOC_{civ_id(slug)}_NAME", ja, en, ru)
            add(f"LOC_{civ_id(slug)}_DESCRIPTION", ja, en, ru)
            add(f"LOC_{civ_id(slug)}_ADJECTIVE", ja, en, ru)
            add(f"LOC_{leader_id(slug)}_NAME", head, head_en, head_ru)
        alias = SEAT_CASTLES.get(slug)
        capital_ja = castles[alias]["name"] if alias else EXTRA_CAPITAL_NAMES[slug]
        add(f"LOC_JAC_1534_CAPITAL_{slug}", capital_ja, seat.castle, seat.castle)
    write_xml(MOD / "Text/JAC_Japan1534_Text.xml", root)


def artdefs() -> None:
    for filename, original, ids in (
        ("Civilizations.artdef", "CIVILIZATION_JAC_ODA", civ_id),
        ("Leaders.artdef", "LEADER_JAC_ODA_NOBUHIDE", leader_id),
        ("FallbackLeaders.artdef", "LEADER_JAC_ODA_NOBUHIDE", leader_id),
    ):
        path = MOD / "ArtDefs" / filename
        root = ET.parse(path).getroot()
        collection = root.find("./m_RootCollections/Element")
        for element in list(collection.findall("Element")):
            name = element.find("m_Name").attrib["text"]
            if name.startswith("CIVILIZATION_JAC_") or name.startswith("LEADER_JAC_"):
                if name != original and name not in ("CIVILIZATION_JAC_TAKEDA", "LEADER_JAC_TAKEDA_NOBUTORA"):
                    collection.remove(element)
        template = next(el for el in collection.findall("Element") if el.find("m_Name").attrib["text"] == original)
        for seat in EXTRA:
            copy = deepcopy(template)
            copy.find("m_Name").set("text", ids(seat.slug))
            if filename == "FallbackLeaders.artdef":
                entry = copy.find("./m_ChildCollections/Element/Element/m_Fields/m_Values/Element/m_EntryName")
                entry.set("text", f"FALLBACK_NEUTRAL_JAC_{ids(seat.slug).removeprefix('LEADER_JAC_')}")
            collection.append(copy)
        write_xml(path, root)
    path = MOD / "ArtDefs/Cultures.artdef"
    root = ET.parse(path).getroot()
    for group in root.findall("./m_RootCollections/Element/Element"):
        values = group.find("./m_Fields/m_Values/Element/m_Values")
        for element in list(values.findall("Element")):
            name = element.find("m_ElementName").attrib["text"]
            if name.startswith("CIVILIZATION_JAC_") and name not in ("CIVILIZATION_JAC_ODA", "CIVILIZATION_JAC_TAKEDA"):
                values.remove(element)
        template = next(el for el in values.findall("Element") if el.find("m_ElementName").attrib["text"] == "CIVILIZATION_JAC_ODA")
        for number, seat in enumerate(EXTRA, 3):
            copy = deepcopy(template)
            copy.find("m_ElementName").set("text", civ_id(seat.slug))
            copy.find("m_ParamName").set("text", f"Civilizations{number:03d}")
            values.append(copy)
    write_xml(path, root)


def main() -> None:
    gameplay()
    front_end()
    icons()
    colors()
    translated_text()
    artdefs()


if __name__ == "__main__":
    main()
