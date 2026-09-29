"""SSOT 3.2 resources: tech gates under the Sengoku trees, custom resource rows, deferred-site data."""

from __future__ import annotations

import json
import re
import sqlite3
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
JAPAN = ROOT / "poc/JAC_PoC_JapanMapScale"
SENGOKU = ROOT / "poc/JAC_PoC_SengokuTrees"
GAME = Path("C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI")
BASE_RESOURCES = GAME / "Base/Assets/Gameplay/Data/Resources.xml"
XP2_RESOURCES = GAME / "DLC/Expansion2/Data/Expansion2_Resources.xml"
ANIMAL_HARVESTS = {"RESOURCE_CATTLE", "RESOURCE_DEER", "RESOURCE_SHEEP"}  # not on the Japan maps


def statements(path: Path) -> list[str]:
    text = "\n".join(line for line in path.read_text(encoding="utf-8").splitlines()
                     if not line.lstrip().startswith("--"))
    return [item.strip() for item in text.split(";") if item.strip()]


def official_prereqs() -> tuple[dict[str, str | None], dict[tuple[str, str], str | None]]:
    resources: dict[str, str | None] = {}
    harvests: dict[tuple[str, str], str | None] = {}
    base = ET.parse(BASE_RESOURCES).getroot()
    for row in base.findall("./Resources/Row"):
        resources[row.attrib["ResourceType"]] = row.attrib.get("PrereqTech")
    for row in base.findall("./Resource_Harvests/Row"):
        harvests[(row.attrib["ResourceType"], row.attrib["YieldType"])] = row.attrib.get("PrereqTech")
    for update in ET.parse(XP2_RESOURCES).getroot().findall("./Resources/Update"):
        prereq = update.find("./Set/PrereqTech")
        if prereq is not None:
            resources[update.find("Where").attrib["ResourceType"]] = prereq.text
    return resources, harvests


class SengokuResourceGateTests(unittest.TestCase):
    """Resources.PrereqTech and Resource_Harvests.PrereqTech are ON DELETE SET NULL
    (01_GameplaySchema.sql:2325-2368), so deleting the official techs must not leave a gate empty."""

    @unittest.skipUnless(BASE_RESOURCES.is_file() and XP2_RESOURCES.is_file(), "official Civ6 data not installed")
    def test_deleted_official_techs_never_open_a_resource_early(self):
        resources, harvests = official_prereqs()
        sengoku_techs = set(re.findall(r"\('(TECH_JAC_SENGOKU_[A-Z_]+)', 'LOC_",
                                       (SENGOKU / "Data/JAC_Sengoku_Trees.sql").read_text(encoding="utf-8")))
        official_techs = {tech for tech in [*resources.values(), *harvests.values()] if tech}
        self.assertEqual(len(sengoku_techs), 32)
        db = sqlite3.connect(":memory:")
        db.executescript("""
            PRAGMA foreign_keys=ON;
            CREATE TABLE Technologies(TechnologyType TEXT PRIMARY KEY);
            CREATE TABLE Resources(ResourceType TEXT PRIMARY KEY, PrereqTech TEXT
                REFERENCES Technologies(TechnologyType) ON DELETE SET NULL ON UPDATE CASCADE);
            CREATE TABLE Resource_Harvests(ResourceType TEXT, YieldType TEXT, PrereqTech TEXT
                REFERENCES Technologies(TechnologyType) ON DELETE SET NULL ON UPDATE CASCADE,
                PRIMARY KEY(ResourceType, YieldType));""")
        db.executemany("INSERT INTO Technologies VALUES (?)", [(t,) for t in sorted(sengoku_techs | official_techs)])
        db.executemany("INSERT INTO Resources VALUES (?, ?)", resources.items())
        db.executemany("INSERT INTO Resource_Harvests VALUES (?, ?, ?)",
                       [(r, y, t) for (r, y), t in harvests.items()])
        # Same order as the modinfo: Trees, Resources, Unlocks (its Resources updates, then the tech delete).
        for statement in statements(SENGOKU / "Data/JAC_Sengoku_Resources.sql"):
            db.execute(statement)
        for statement in statements(SENGOKU / "Data/JAC_Sengoku_Unlocks.sql"):
            if statement.startswith(("UPDATE Resources", "UPDATE Resource_Harvests")):
                db.execute(statement)
        db.execute("DELETE FROM Technologies WHERE TechnologyType NOT LIKE 'TECH_JAC_SENGOKU_%'")

        final = dict(db.execute("SELECT ResourceType, PrereqTech FROM Resources"))
        for resource, official in resources.items():
            with self.subTest(resource=resource):
                if final[resource] is not None:
                    self.assertIn(final[resource], sengoku_techs)
                if official and resource != "RESOURCE_HORSES":  # horses are open from the start by design
                    self.assertIsNotNone(final[resource], "official tech deleted, resource open from turn 1")
        self.assertEqual(final["RESOURCE_IRON"], "TECH_JAC_SENGOKU_IRON_FORGING")
        self.assertIsNone(final["RESOURCE_HORSES"])

        final_harvests = {(resource, yield_type): tech for resource, yield_type, tech in db.execute(
            "SELECT ResourceType, YieldType, PrereqTech FROM Resource_Harvests")}
        for (resource, yield_type), official in harvests.items():
            with self.subTest(harvest=resource):
                tech = final_harvests[(resource, yield_type)]
                if tech is not None:
                    self.assertIn(tech, sengoku_techs)
                if official and resource not in ANIMAL_HARVESTS:
                    self.assertIsNotNone(tech, "harvest tech deleted, harvest open from turn 1")

    def test_modinfo_runs_the_resource_gate_between_trees_and_unlocks(self):
        root = ET.parse(SENGOKU / "JAC_PoC_SengokuTrees.modinfo").getroot()
        action = root.find("./InGameActions/UpdateDatabase[@id='JAC_Sengoku_Trees']")
        files = [item.text for item in action.findall("File")]
        self.assertEqual(files.index("Data/JAC_Sengoku_Trees.sql") + 1,
                         files.index("Data/JAC_Sengoku_Resources.sql"))
        self.assertEqual(files.index("Data/JAC_Sengoku_Resources.sql") + 1,
                         files.index("Data/JAC_Sengoku_Unlocks.sql"))
        self.assertIn("Data/JAC_Sengoku_Resources.sql", [item.text for item in root.findall("./Files/File")])


class CustomResourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.xml = ET.parse(JAPAN / "Data/JAC_Japan_Resources.xml").getroot()
        cls.custom = {"RESOURCE_JAC_GOLD", "RESOURCE_JAC_LACQUER"}

    def test_each_custom_resource_is_fully_defined_with_text_and_icons(self):
        types = {row.attrib["Type"] for row in self.xml.findall("./Types/Row")}
        rows = {row.attrib["ResourceType"]: row.attrib for row in self.xml.findall("./Resources/Row")}
        self.assertEqual(types, self.custom)
        self.assertEqual(set(rows), self.custom)
        text = (JAPAN / "Text/JAC_Japan_Resources_Text.xml").read_text(encoding="utf-8")
        icons = ET.parse(JAPAN / "Data/JAC_Japan_Resources_Icons.xml").getroot()
        icon_names = {row.attrib["Name"] for row in icons.findall("./IconDefinitions/Row")}
        for resource, attributes in rows.items():
            with self.subTest(resource=resource):
                self.assertEqual(attributes["ResourceClassType"], "RESOURCECLASS_LUXURY")
                self.assertEqual(attributes["Frequency"], "0")  # never scattered by a random generator
                self.assertNotIn("PrereqTech", attributes)  # deferral is per site, not per type
                for language in ("en_US", "ja_JP", "ru_RU"):
                    self.assertRegex(text, rf'Tag="{attributes["Name"]}" Language="{language}"')
                self.assertTrue({f"ICON_{resource}", f"ICON_{resource}_FOW"} <= icon_names)
                for table, key in (("Resource_ValidTerrains", "TerrainType"), ("Resource_YieldChanges", "YieldType"),
                                   ("Improvement_ValidResources", "ImprovementType")):
                    self.assertTrue([row for row in self.xml.findall(f"./{table}/Row")
                                     if row.attrib["ResourceType"] == resource and key in row.attrib], (table, resource))
        for row in icons.findall("./IconDefinitions/Row"):
            self.assertIn(row.attrib["Atlas"], {"ICON_ATLAS_RESOURCES", "ICON_ATLAS_RESOURCES_FOW"})  # no new art

    def test_modinfo_wires_every_new_file(self):
        root = ET.parse(JAPAN / "JAC_PoC_JapanMapScale.modinfo").getroot()
        listed = {item.text for item in root.findall("./Files/File")}
        used = set()
        for action in root.find("InGameActions"):
            if action.attrib["id"].startswith("JAC_Japan_Resources") or action.attrib["id"].endswith("_Scripts"):
                self.assertEqual(action.attrib["criteria"], "JAC_JapanMapScale_Banseiki")
                used.update(item.text for item in action.findall("File"))
        self.assertEqual(used, {"Data/JAC_Japan_Resources.xml", "Data/JAC_Japan_Resources_Icons.xml",
                                "Text/JAC_Japan_Resources_Text.xml", "Scripts/JAC_ResourceSites.lua",
                                "Scripts/JAC_Resources.lua"})
        self.assertLessEqual(used, listed)
        for name in used:
            self.assertTrue((JAPAN / name).is_file(), name)
        for action in root.findall("./InGameActions/UpdateDatabase[@criteria]"):
            if action.attrib["id"].endswith("_Rules"):
                self.assertEqual([item.text for item in action.findall("File")], ["Data/JAC_Japan_MapRules.xml"])
        scripts = root.find("./InGameActions/AddGameplayScripts")
        self.assertEqual([item.text for item in scripts.findall("File")],
                         ["Scripts/JAC_ResourceSites.lua", "Scripts/JAC_Resources.lua"])


class DeferredSiteDataTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads((JAPAN / "Maps/manifest.json").read_text(encoding="utf-8"))
        lua = (JAPAN / "Scripts/JAC_ResourceSites.lua").read_text(encoding="utf-8")
        cls.lua = {}
        for size, body in re.findall(r'\["(\d+x\d+)"\] = \{(.*?)\n\t\},', lua, re.S):
            cls.lua[size] = [dict(re.findall(r'(\w+)=("?[^,"}\s]+"?)', line))
                             for line in body.splitlines() if line.strip()]

    def test_lua_data_is_exactly_the_reserved_sites_of_each_map(self):
        self.assertEqual(set(self.lua), {"154x67", "204x88"})
        for item in self.manifest:
            size = f"{item['width']}x{item['height']}"
            reserved = {r["id"]: r for r in item["resource_candidates"]
                        if r["placement_status"] == "reserved_for_later_release"}
            with self.subTest(size=size):
                self.assertEqual({site["id"].strip('"') for site in self.lua[size]}, set(reserved))
                for site in self.lua[size]:
                    record = reserved[site["id"].strip('"')]
                    self.assertEqual(int(site["plot"]), record["plot"])
                    self.assertEqual(site["resource"].strip('"'), record["resource_id"])
                    self.assertEqual(int(site["year"]), record["reveal_year"])
                years = [int(site["year"]) for site in self.lua[size]]
                self.assertEqual(years, sorted(years))

    def test_named_ssot_sites_are_deferred_with_a_year_and_a_free_land_tile(self):
        named = {"silver-ikuno": 1542, "silver-sado-tsurushi": 1542, "silver-nobesawa": 1596,
                 "copper-ashio": 1550, "gold-sado-aikawa": 1596}
        for item in self.manifest:
            records = {r["id"]: r for r in item["resource_candidates"]}
            with sqlite3.connect(JAPAN / "Maps" / item["file"]) as db:
                land = {row[0] for row in db.execute("SELECT ID FROM Plots WHERE ContinentType='CONTINENT_ASIA'")}
                busy = {row[0] for row in db.execute("SELECT ID FROM PlotResources UNION SELECT ID FROM PlotFeatures "
                                                     "UNION SELECT Plot FROM Cities")}
            reserved = [r for r in records.values() if r["placement_status"] == "reserved_for_later_release"]
            with self.subTest(km=item["km_per_tile"]):
                for site, year in named.items():
                    self.assertEqual(records[site]["placement_status"], "reserved_for_later_release")
                    self.assertEqual(records[site]["reveal_year"], year)
                self.assertEqual(len({r["plot"] for r in reserved}), len(reserved))  # one site per tile
                for record in reserved:
                    self.assertIn(record["plot"], land)
                    self.assertNotIn(record["plot"], busy)
                    self.assertLessEqual(record["displacement_km"], max(30, 2 * item["km_per_tile"]))
                    self.assertEqual(record["start_status"], "deferred")
                # Nothing that is deferred is on the map at the start.
                with sqlite3.connect(JAPAN / "Maps" / item["file"]) as db:
                    types = {row[0] for row in db.execute("SELECT ResourceType FROM PlotResources")}
                self.assertNotIn("RESOURCE_COPPER", types)

    def test_site_techs_exist_and_the_lua_api_and_test_hook_are_wired(self):
        trees = (SENGOKU / "Data/JAC_Sengoku_Trees.sql").read_text(encoding="utf-8")
        techs = set(re.findall(r"\('(TECH_JAC_SENGOKU_[A-Z_]+)', 'LOC_", trees))
        for sites in self.lua.values():
            for site in sites:
                if "tech" in site:
                    self.assertIn(site["tech"].strip('"'), techs)  # a typo would never reveal the site
                self.assertGreater(int(site["year"]), 1534)
        lua = (JAPAN / "Scripts/JAC_Resources.lua").read_text(encoding="utf-8")
        block = lua[lua.index("ExposedMembers.JAC_Resources = {"):]
        for name in ("RevealSite", "RevealDue", "RevealForTech", "GetSites", "IsRevealed"):
            self.assertIn(f"{name} = {name},", block)
        scenario = (ROOT / "tools/civ6-autotest/scenarios/japan-resources.json").read_text(encoding="utf-8")
        self.assertIn("JAC_RESOURCES_TEST_YEAR", lua)
        self.assertIn("JAC_RESOURCES_TEST_YEAR", scenario)
        # The scenario waits for these Lua.log lines: "[JAC_RESOURCES] loaded" and "[JAC_RESOURCES] revealed <id>".
        self.assertIn('print("[JAC_RESOURCES] " .. message)', lua)
        self.assertIn('Log("loaded")', lua)
        self.assertIn('Log(string.format("revealed %s: ', lua)
        for line in ("[JAC_RESOURCES] loaded", "[JAC_RESOURCES] revealed silver-ikuno"):
            self.assertIn(line, scenario)

    def test_custom_resources_are_on_the_map_at_the_start(self):
        for item in self.manifest:
            with self.subTest(km=item["km_per_tile"]), sqlite3.connect(JAPAN / "Maps" / item["file"]) as db:
                rows = dict(db.execute("SELECT ID, ResourceType FROM PlotResources"))
            records = {r["id"]: r for r in item["resource_candidates"]}
            for site, resource in (("gold-kai-kurokawa", "RESOURCE_JAC_GOLD"), ("gold-kai-nakayama", "RESOURCE_JAC_GOLD"),
                                   ("lacquer-wajima", "RESOURCE_JAC_LACQUER"), ("silver-iwami", "RESOURCE_SILVER")):
                self.assertEqual(rows[records[site]["plot"]], resource)


if __name__ == "__main__":
    unittest.main()
