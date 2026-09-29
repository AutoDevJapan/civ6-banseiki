"""Keep the World Congress removal scoped to the four Banseiki Japan maps."""

import unittest
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_JapanMapScale"
MAP_PREFIX = "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/"
MAP_NAMES = {
    "JAC_Japan_20km.Civ6Map",
    "JAC_Japan_15km.Civ6Map",
    "JAC_Japan_20km_Settlers.Civ6Map",
    "JAC_Japan_15km_Settlers.Civ6Map",
}


class JapanNoWorldCongressTests(unittest.TestCase):
    def test_congress_capability_removed_only_for_all_four_japan_maps(self):
        mod = ET.parse(MOD / "JAC_PoC_JapanMapScale.modinfo").getroot()
        criteria = {row.attrib["id"]: row for row in mod.findall("./ActionCriteria/Criteria")}
        actions = [row for row in mod.findall("./InGameActions/UpdateDatabase")
                   if any(file.text == "Data/JAC_Japan_MapRules.xml" for file in row.findall("File"))]
        self.assertEqual(len(actions), 4)
        selected = set()
        for action in actions:
            criterion = criteria[action.attrib["criteria"]]
            self.assertEqual(criterion.findtext("RuleSetInUse"), "RULESET_JAC_BANSEIKI")
            match = criterion.find("ConfigurationValueMatches")
            self.assertIsNotNone(match)
            self.assertEqual(match.findtext("Group"), "Map")
            self.assertEqual(match.findtext("ConfigurationId"), "MAP_SCRIPT")
            selected.add(match.findtext("Value"))
        self.assertEqual(selected, {MAP_PREFIX + name for name in MAP_NAMES})

    def test_capability_delete_does_not_remove_other_gs_systems(self):
        rules = ET.parse(MOD / "Data/JAC_Japan_MapRules.xml").getroot()
        self.assertEqual([row.attrib for row in rules.findall("./Types/Delete")],
                         [{"Type": "CAPABILITY_WORLD_CONGRESS"}])
        self.assertEqual(rules.findall("./GameCapabilities/Delete"), [])
        self.assertFalse(any(row.attrib.get("Name", "").startswith("WORLD_CONGRESS_")
                             for row in rules.findall("./GlobalParameters/*/Where")))


if __name__ == "__main__":
    unittest.main()
