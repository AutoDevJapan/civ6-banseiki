"""Check the art referenced by CustomCivs' FrontEnd Players rows.

Run after ``python tools/jac_art/build_art.py``.  CIV6_INSTALL_DIR may point to
another local Civilization VI installation; the game is never started.
"""

import mmap
import os
from pathlib import Path
import re
import unittest
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[3]
MOD = ROOT / "poc" / "JAC_PoC_CustomCivs"
GAME = Path(os.environ.get(
    "CIV6_INSTALL_DIR",
    r"C:\Program Files (x86)\Steam\steamapps\common\Sid Meier's Civilization VI",
))
BASE_ICONS = GAME / "Base" / "Assets" / "UI" / "Icons"
BASE_ICON_PACKAGE = GAME / "Base" / "Platforms" / "Windows" / "BLPs" / "UI" / "Icons.blp"


def action_files(modinfo, context, action):
    return {
        node.text for node in modinfo.findall(f"./{context}/{action}/File")
    }


def icon_records(files, allow_duplicates=False):
    definitions = {}
    atlases = {}
    for path in files:
        xml = ET.parse(path)
        for row in xml.findall("./IconDefinitions/Row"):
            name = row.attrib["Name"]
            if name in definitions and not allow_duplicates:
                raise AssertionError(f"duplicate icon: {name}")
            definitions.setdefault(name, row.attrib)
        for row in xml.findall("./IconTextureAtlases/Row"):
            atlases.setdefault(row.attrib["Name"], []).append(row.attrib)
    return definitions, atlases


def package_textures(path, names):
    """Return names missing from a cooked package without loading it into RAM."""
    with path.open("rb") as file, mmap.mmap(file.fileno(), 0, access=mmap.ACCESS_READ) as package:
        return sorted(name for name in names if package.find(name.encode("ascii")) < 0)


class FrontEndArtTests(unittest.TestCase):
    def test_every_player_icon_and_portrait_has_a_cooked_texture(self):
        modinfo = ET.parse(MOD / "JAC_PoC_CustomCivs.modinfo")
        frontend_icons = action_files(modinfo, "FrontEndActions", "UpdateIcons")
        ingame_icons = action_files(modinfo, "InGameActions", "UpdateIcons")
        self.assertEqual(frontend_icons, ingame_icons)
        self.assertEqual(action_files(modinfo, "FrontEndActions", "UpdateArt"),
                         {"JAC_PoC_CustomCivs.dep"})
        self.assertEqual(action_files(modinfo, "InGameActions", "UpdateArt"),
                         {"JAC_PoC_CustomCivs.dep"})

        player_paths = [node.text for node in modinfo.findall("./FrontEndActions/UpdateDatabase/File")]
        players_files = set(player_paths)
        players = [row.attrib for file in player_paths
                   for row in ET.parse(MOD / file).findall("./Players/Row")]
        self.assertEqual(len(players), 22)
        self.assertEqual(len({row["LeaderType"] for row in players}), 22)
        self.assertTrue(all(row["Domain"] == "Players:Expansion2_Players" for row in players))

        self.assertTrue(BASE_ICONS.is_dir(), f"Civ6 icon XML missing: {BASE_ICONS}")
        mod_definitions, mod_atlases = icon_records(MOD / file for file in frontend_icons)
        leader_atlases = mod_atlases["ICON_ATLAS_JAC_LEADERS"]
        self.assertEqual({(int(row["IconsPerRow"]), int(row["IconsPerColumn"]))
                          for row in leader_atlases}, {(8, 3)})
        for index, player in enumerate(players):
            leader = player["LeaderType"]
            self.assertEqual(mod_definitions[player["LeaderIcon"]]["Index"], str(index))
            self.assertEqual(player["Portrait"], f"{leader}_NEUTRAL")
            self.assertEqual(player["PortraitBackground"], f"{leader}_BACKGROUND")
        base_definitions, base_atlases = icon_records(BASE_ICONS.glob("*.xml"),
                                                     allow_duplicates=True)
        dep = ET.parse(MOD / "JAC_PoC_CustomCivs.dep")
        ui_packages = {
            row.attrib["text"] for library in dep.findall("./LibraryDependencies/Element")
            if library.find("LibraryName").attrib["text"] == "UITexture"
            for row in library.findall("./PackageDependencies/Element")
        }
        self.assertEqual(ui_packages,
                         {"JAC_CustomCivs_Icons.blp", "JAC_CustomCivs_Leaders.blp"})
        ui_consumers = [node for node in dep.findall("./SystemDependencies/Element")
                        if node.find("ConsumerName").attrib["text"] == "UI"]
        self.assertEqual(len(ui_consumers), 1)
        self.assertIn("UITexture", {node.attrib["text"] for node in
                      ui_consumers[0].findall("./LibraryDependencies/Element")})

        listed_files = {node.text for node in modinfo.findall("./Files/File")}
        self.assertTrue(frontend_icons <= listed_files)
        self.assertTrue(players_files <= listed_files)
        self.assertIn("JAC_PoC_CustomCivs.dep", listed_files)
        for package in ui_packages | {"JAC_CustomCivs_Fallback.blp"}:
            self.assertIn(f"Platforms/Windows/BLPs/{package}", listed_files)
            self.assertTrue((MOD / "Platforms" / "Windows" / "BLPs" / package).is_file(),
                            f"{package} is missing; run python tools/jac_art/build_art.py")

        mod_atlas_textures = set()
        base_atlas_textures = set()
        portraits = set()
        for player in players:
            for field in ("CivilizationIcon", "LeaderIcon", "CivilizationAbilityIcon",
                          "LeaderAbilityIcon"):
                icon = player[field]
                definition = mod_definitions.get(icon, base_definitions.get(icon))
                self.assertIsNotNone(definition, f"{player['LeaderType']} {field}: {icon}")
                atlas = definition["Atlas"]
                atlas_rows = mod_atlases.get(atlas, base_atlases.get(atlas))
                self.assertIsNotNone(atlas_rows, f"{icon} atlas: {atlas}")
                textures = mod_atlas_textures if atlas in mod_atlases else base_atlas_textures
                sizes = {int(row["IconSize"]) for row in atlas_rows}
                if field in ("CivilizationIcon", "LeaderIcon"):
                    self.assertTrue({45, 80} <= sizes, f"{icon} lacks FrontEnd sizes")
                for row in atlas_rows:
                    self.assertLess(int(definition["Index"]),
                                    int(row["IconsPerRow"]) * int(row["IconsPerColumn"]), icon)
                    textures.add(Path(row["Filename"]).stem)
            portraits.update((player["Portrait"], player["PortraitBackground"]))

        self.assertTrue(BASE_ICON_PACKAGE.is_file(), f"base icon package missing: {BASE_ICON_PACKAGE}")
        self.assertEqual(package_textures(BASE_ICON_PACKAGE, base_atlas_textures), [])
        icon_package = MOD / "Platforms" / "Windows" / "BLPs" / "JAC_CustomCivs_Icons.blp"
        leader_package = MOD / "Platforms" / "Windows" / "BLPs" / "JAC_CustomCivs_Leaders.blp"
        self.assertEqual(package_textures(icon_package, mod_atlas_textures), [])
        self.assertEqual(package_textures(leader_package, portraits), [])

    def test_ingame_portraits_and_fallbacks_cover_all_players(self):
        players = [row.attrib for path in ("Config/JAC_CustomCivs_Players.xml",
                                         "Config/JAC_Japan1534_Players.xml")
                   for row in ET.parse(MOD / path).findall("./Players/Row")]
        leaders = {row["LeaderType"] for row in players}
        dep = ET.parse(MOD / "JAC_PoC_CustomCivs.dep")
        fallback_libraries = [node for node in dep.findall("./LibraryDependencies/Element")
                              if node.find("LibraryName").attrib["text"] == "LeaderFallback"]
        self.assertEqual(len(fallback_libraries), 1)
        self.assertEqual({node.attrib["text"] for node in
                          fallback_libraries[0].findall("./PackageDependencies/Element")},
                         {"JAC_CustomCivs_Fallback.blp"})
        fallback_consumers = [node for node in dep.findall("./SystemDependencies/Element")
                              if node.find("ConsumerName").attrib["text"] == "LeaderFallback"]
        self.assertEqual(len(fallback_consumers), 1)
        self.assertIn("LeaderFallback", {node.attrib["text"] for node in
                      fallback_consumers[0].findall("./LibraryDependencies/Element")})
        leader_artdef = ET.parse(MOD / "ArtDefs" / "Leaders.artdef")
        self.assertEqual({row.find("m_Name").attrib["text"] for row in
                          leader_artdef.findall("./m_RootCollections/Element/Element")}, leaders)
        artdef = ET.parse(MOD / "ArtDefs" / "FallbackLeaders.artdef")
        fallback_rows = artdef.findall("./m_RootCollections/Element/Element")
        fallback_names = {}
        for row in fallback_rows:
            leader = row.find("m_Name").attrib["text"]
            entry = row.find("./m_ChildCollections/Element/Element/m_Fields/m_Values/Element/m_EntryName")
            fallback_names[leader] = entry.attrib["text"]
        self.assertEqual(set(fallback_names), leaders)
        for leader, entry in fallback_names.items():
            self.assertEqual(entry, f"FALLBACK_NEUTRAL_JAC_{leader.removeprefix('LEADER_JAC_')}")
        fallback_package = MOD / "Platforms" / "Windows" / "BLPs" / "JAC_CustomCivs_Fallback.blp"
        self.assertTrue(fallback_package.is_file(), "build the ignored BLP packages first")
        self.assertEqual(package_textures(fallback_package, set(fallback_names.values())), [])

        texture_names = set()
        for path in ("Data/JAC_CustomCivs_Gameplay.sql", "Data/JAC_Japan1534_Gameplay.sql"):
            sql = (MOD / path).read_text(encoding="utf-8")
            loading = re.search(r"INSERT INTO LoadingInfo\s*\(.*?\)\s*VALUES\s*(.*?);", sql, re.S)
            diplomacy = re.search(r"INSERT INTO DiplomacyInfo\s*\(.*?\)\s*VALUES\s*(.*?);", sql, re.S)
            self.assertIsNotNone(loading, path)
            self.assertIsNotNone(diplomacy, path)
            loading_rows = re.findall(r"\('([^']+)',\s*'([^']+)',\s*'([^']+)'", loading.group(1))
            diplomacy_rows = re.findall(r"\('([^']+)',\s*'([^']+)'", diplomacy.group(1))
            self.assertTrue(loading_rows, path)
            self.assertTrue(diplomacy_rows, path)
            texture_names.update(name for _, foreground, background in loading_rows
                                 for name in (foreground, background))
            texture_names.update(background for _, background in diplomacy_rows)
            if path.endswith("JAC_CustomCivs_Gameplay.sql"):
                self.assertEqual({row[0] for row in loading_rows},
                                 {"LEADER_JAC_ODA_NOBUHIDE", "LEADER_JAC_TAKEDA_NOBUTORA"})
            else:
                self.assertEqual({row[0] for row in loading_rows},
                                 leaders - {"LEADER_JAC_ODA_NOBUHIDE", "LEADER_JAC_TAKEDA_NOBUTORA"})
            self.assertEqual({row[0] for row in loading_rows},
                             {row[0] for row in diplomacy_rows})
            for leader, foreground, background in loading_rows:
                self.assertEqual(foreground, f"{leader}_NEUTRAL")
                self.assertEqual(background, f"{leader}_BACKGROUND")
            for leader, background in diplomacy_rows:
                self.assertEqual(background, f"{leader}_DIPLOMACY_BACKGROUND")
        leader_package = MOD / "Platforms" / "Windows" / "BLPs" / "JAC_CustomCivs_Leaders.blp"
        self.assertEqual(package_textures(leader_package, texture_names), [])


if __name__ == "__main__":
    unittest.main()
