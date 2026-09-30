"""Check the art referenced by CustomCivs' FrontEnd Players rows.

Run after ``python tools/jac_art/build_art.py``.  CIV6_INSTALL_DIR may point to
another local Civilization VI installation; the game is never started.
"""

import itertools
import math
import mmap
import os
from pathlib import Path
import re
import sys
import unittest
import xml.etree.ElementTree as ET

from PIL import Image, ImageChops

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from draw_crests import NAMES as CREST_NAMES, MOTIFS as CREST_MOTIFS
import build_art


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
        # IconDefinitions' primary key is (Name, Atlas) (Base/Assets/Database/IconManager.sql),
        # so Replace/Update on another atlas adds a second row and the older row keeps winning.
        # Each mod icon must therefore be defined exactly once, as a plain Row.
        if not allow_duplicates:
            edits = [node.tag for node in xml.findall("./IconDefinitions/*") if node.tag != "Row"]
            if edits:
                raise AssertionError(f"{path}: IconDefinitions uses {edits}; define icons once with Row")
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
    def test_house_crests_fill_all_xp2_sizes_with_distinct_white_masks(self):
        self.assertEqual(len(CREST_NAMES), 22)
        self.assertEqual(len(set(CREST_NAMES)), 22)
        self.assertEqual(len(CREST_MOTIFS), 22)
        official = GAME / "DLC" / "Expansion2" / "Data" / "Expansion2_Icons_Civilizations.xml"
        self.assertTrue(official.is_file())
        official_sizes = {
            int(row.attrib["IconSize"]) for row in
            ET.parse(official).findall("./IconTextureAtlases/Row")
            if row.attrib["Name"] == "ICON_ATLAS_EXPANSION_2_CIVILIZATIONS"
        }
        self.assertEqual(official_sizes,
                         {22, 30, 32, 36, 44, 45, 48, 50, 64, 80, 128, 200, 256})
        modinfo = ET.parse(MOD / "JAC_PoC_CustomCivs.modinfo")
        for context in ("FrontEndActions", "InGameActions"):
            icon_files = [node.text for node in modinfo.findall(f"./{context}/UpdateIcons/File")]
            self.assertIn("Data/JAC_Crests_Icons.xml", icon_files, context)
            # Crest rows must be the only civilization icons, whatever the load order.
            civ_icons = {row.attrib["Name"]: file for file in icon_files
                         for row in ET.parse(MOD / file).findall("./IconDefinitions/Row")
                         if row.attrib["Name"].startswith("ICON_CIVILIZATION_")}
            self.assertEqual(civ_icons, {f"ICON_CIVILIZATION_JAC_{name}": "Data/JAC_Crests_Icons.xml"
                                         for name in CREST_NAMES}, context)
        definitions, atlases = icon_records(MOD / file for file in icon_files)
        self.assertNotIn("ICON_ATLAS_JAC_CIVILIZATIONS", atlases)
        rows = atlases["ICON_ATLAS_JAC_CRESTS"]
        self.assertEqual({int(row["IconSize"]) for row in rows}, official_sizes)
        self.assertTrue(all(int(row["IconsPerRow"]) == 8 and
                            int(row["IconsPerColumn"]) == 3 for row in rows))
        players = [dict(row.attrib) for file in ("Config/JAC_CustomCivs_Players.xml",
                                                 "Config/JAC_Japan1534_Players.xml")
                   for row in ET.parse(MOD / file).findall("./Players/Row")]
        config_files = [node.text for node in modinfo.findall("./FrontEndActions/UpdateDatabase/File")]
        self.assertEqual(config_files[-1], "Config/JAC_Crests_Players.xml")
        for update in ET.parse(MOD / config_files[-1]).findall("./Players/Update"):
            where, values = update.find("Where").attrib, update.find("Set").attrib
            matched = [p for p in players if all(p[k] == v for k, v in where.items())]
            self.assertEqual(len(matched), 1, where)
            matched[0].update(values)
        self.assertEqual({row["CivilizationType"] for row in players},
                         {f"CIVILIZATION_JAC_{name}" for name in CREST_NAMES})
        for player in players:
            icon = player["CivilizationIcon"]
            self.assertEqual(icon, "ICON_" + player["CivilizationType"])
            self.assertEqual(int(definitions[icon]["Index"]),
                             CREST_NAMES.index(player["CivilizationType"].removeprefix(
                                 "CIVILIZATION_JAC_")))
            self.assertEqual(definitions[icon]["Atlas"], "ICON_ATLAS_JAC_CRESTS")
            self.assertNotIn("ICON_CIVILIZATION_JAPAN", player.values())

        originals = ROOT.parent.parent / "civ6-art-local" / "JAC_PoC_CustomCivs" / "Originals" / "crests"
        override = os.environ.get("JAC_ART_LOCAL")
        if override:
            originals = Path(override).expanduser().resolve() / "JAC_PoC_CustomCivs" / "Originals" / "crests"
        masters = []
        alpha_fingerprints = set()
        for name in CREST_NAMES:
            self.assertTrue((originals / f"{name.lower()}.svg").is_file())
            with Image.open(originals / f"{name.lower()}.png") as file:
                master = file.convert("RGBA")
            self.assertEqual(master.size, (512, 512))
            self.assertEqual(master.getpixel((0, 0))[3], 0)
            alpha = master.getchannel("A")
            self.assertIsNotNone(alpha.getbbox())
            self.assertEqual(set(master.getdata(0)) | set(master.getdata(1)) |
                             set(master.getdata(2)), {255})
            alpha_fingerprints.add(alpha.tobytes())
            masters.append(master)
        self.assertEqual(len(alpha_fingerprints), 22)

        for size in official_sizes:
            with Image.open(ROOT / "tools" / "jac_art" / "build" / "previews" /
                            f"JAC_Crests{size}.png") as file:
                atlas = file.convert("RGBA")
            self.assertEqual(atlas.size, (8*size, 3*size))
            for index, master in enumerate(masters):
                tile = atlas.crop(((index%8)*size, (index//8)*size,
                                   (index%8+1)*size, (index//8+1)*size))
                expected = master.resize((size, size), Image.Resampling.LANCZOS)
                self.assertIsNone(ImageChops.difference(tile, expected).getbbox(),
                                  f"{CREST_NAMES[index]} at {size}")

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


# Glyphs of the official hooks beside the notebooks button, measured on the in-game capture
# civ6-art-local/final-01/ingame-v3/04-normal-full.png (1x, 2026-09-29): gold pixels inside the
# 26x26 image frame. (width, height, covered pixels)
OFFICIAL_HOOK_GLYPHS = {"WorldRankings": (23, 18, 223), "Era": (20, 22, 175), "Reports": (14, 17, 181)}


class IconReadabilityTests(unittest.TestCase):
    def test_notebooks_glyph_matches_official_neighbours(self):
        dds = build_art.GROWTH_MOD / "UI" / "Textures" / "JAC_Notebooks_Hook.dds"
        self.assertTrue(dds.is_file(), "run tools/jac_art/build_art.py first")
        data = dds.read_bytes()
        self.assertEqual(data[:4], b"DDS ")
        height, width = int.from_bytes(data[12:16], "little"), int.from_bytes(data[16:20], "little")
        self.assertEqual((width, height), (26, 26))
        alpha = Image.frombytes("RGBA", (width, height), data[128:]).getchannel("A")
        x0, y0, x1, y1 = alpha.point(lambda v: 255 if v >= 128 else 0).getbbox()
        sizes = OFFICIAL_HOOK_GLYPHS.values()
        self.assertTrue(min(w for w, _, _ in sizes) - 1 <= x1 - x0 <= max(w for w, _, _ in sizes) + 1, (x0, x1))
        self.assertTrue(min(h for _, h, _ in sizes) - 1 <= y1 - y0 <= max(h for _, h, _ in sizes) + 1, (y0, y1))
        coverage = sum(alpha.getdata()) / 255
        mean = sum(area for _, _, area in sizes) / len(sizes)
        self.assertLess(abs(coverage - mean), mean * 0.15, coverage)
        self.assertLessEqual(abs((x0 + x1) / 2 - 13), 1)
        self.assertLessEqual(abs((y0 + y1) / 2 - 13), 1)

    def test_leader_icons_are_face_crops_framed_in_house_colour(self):
        self.assertEqual(set(build_art.FACES) - {"_comment"}, set(build_art.LEADER_ORDER))
        colors = build_art.house_colors()
        self.assertEqual(set(colors), set(build_art.LEADER_ORDER))
        for key, (x0, y0, x1, y1) in ((k, build_art.FACES[k]) for k in build_art.LEADER_ORDER):
            self.assertTrue(0 <= x0 < x1 <= 1 and 0 <= y0 < y1 <= 1, key)
        atlas = Image.open(build_art.PREVIEWS / "JAC_Leaders64.png").convert("RGBA")
        for index, key in enumerate(build_art.LEADER_ORDER):
            x, y = (index % 8) * 64, (index // 8) * 64
            self.assertEqual(atlas.getpixel((x, y))[3], 0, key)
            # Middle of the ring, 1.5px inside the outer edge at the top of the circle.
            ring = atlas.getpixel((x + 32, y + 2))[:3]
            self.assertLess(max(abs(a - b) for a, b in zip(ring, colors[key])), 40, (key, ring))

    def test_house_primary_colours_are_distinct(self):
        names = {}
        for name in ("JAC_CustomCivs_Colors.xml", "JAC_Japan1534_Colors.xml"):
            for row in ET.parse(build_art.MOD / "Data" / name).findall("./PlayerColors/Row"):
                names[row.findtext("Type")] = row.findtext("PrimaryColor")
        self.assertEqual(len(names), 22)
        self.assertEqual(len(set(names.values())), 22, names)
        # Official DK shades sit 46+ apart, so 40 flags only real look-alikes.
        colors = build_art.house_colors()
        for a, b in itertools.combinations(sorted(colors), 2):
            self.assertGreaterEqual(math.dist(colors[a], colors[b]), 40, (a, b))


if __name__ == "__main__":
    unittest.main()
