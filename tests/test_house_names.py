"""Static gates for SSOT 3.2 house-specific names (documents/poc/house-names.md)."""

from __future__ import annotations

import re
import unittest
import xml.etree.ElementTree as ET
from collections import defaultdict
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
POC = ROOT / "poc"
RULESET = POC / "JAC_PoC_BanseikiRuleset"
SENGOKU = POC / "JAC_PoC_SengokuTrees"
LANGUAGES = ("ja_JP", "en_US", "ru_RU")
# Context -> script it runs once our ReplaceUIScript file has installed the Locale wrapper.
# NewTurnPanel is already replaced by Gathering Storm (Expansion2.modinfo, Expansion1_NewTurnPanel), so its replacement is loaded.
REPLACED_CONTEXTS = {
    "BoostUnlockedPopup": "BoostUnlockedPopup",
    "EraProgressPanel": "EraProgressPanel",
    "EraCompletePopup": "EraCompletePopup",
    "EraReviewPopup": "EraReviewPopup",
    "DedicationPopup": "DedicationPopup",
    "NewTurnPanel": "NewTurnPanel_Expansion1",
    "HistoricMoments": "HistoricMoments",
}
REGISTER = re.compile(r'^JAC_RegisterHouseName\("([A-Z0-9_]+)", "([A-Z0-9_]+)", "([A-Z0-9_]+)"\)$', re.MULTILINE)


def read_registry() -> list[tuple[str, str, str, Path]]:
    rows = []
    for path in sorted(POC.glob("*/UI/JAC_HouseNamesData_*.lua")):
        text = path.read_text(encoding="utf-8")
        found = REGISTER.findall(text)
        calls = [line for line in text.splitlines() if line.startswith("JAC_RegisterHouseName")]
        if len(found) != len(calls):
            raise AssertionError(f"unparseable registration in {path}")
        rows += [(generic, house, civ, path) for generic, house, civ in found]
    return rows


def read_texts() -> dict[str, dict[str, str]]:
    texts: dict[str, dict[str, str]] = defaultdict(dict)
    for path in POC.glob("*/Text/*.xml"):
        for row in ET.parse(path).getroot().iter("Row"):
            texts[row.get("Tag")][row.get("Language")] = row.findtext("Text") or ""
    return texts


class HouseNameTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.registry = read_registry()
        cls.texts = read_texts()

    def test_sengoku_names_are_registered(self):
        pairs = {(generic, civ) for generic, _, civ, _ in self.registry}
        self.assertIn(("LOC_JAC_SENGOKU_ERA_3_NAME", "CIVILIZATION_JAC_ODA"), pairs)

    def test_taiko_survey_is_the_name_for_every_house(self):
        # SSOT 3.2 (owner decision): every house that researches C25 sees 太閤検地, so it is the plain name, not a house name.
        taiko = "LOC_CIVIC_JAC_SENGOKU_TAIKO_LAND_SURVEY_NAME"
        self.assertNotIn(taiko, {generic for generic, _, _, _ in self.registry})
        self.assertNotIn(taiko + "_HOUSE", self.texts)
        self.assertEqual(self.texts[taiko]["ja_JP"], "太閤検地")
        self.assertEqual(self.texts[taiko]["en_US"], "Taikō Land Survey")

    def test_registry_is_well_formed(self):
        self.assertTrue(self.registry)
        seen = set()
        civilizations = (RULESET.parent / "JAC_PoC_CustomCivs/Data/JAC_CustomCivs_Gameplay.sql").read_text(encoding="utf-8")
        houses = {house for _, house, _, _ in self.registry}
        generics = {generic for generic, _, _, _ in self.registry}
        for generic, house, civ, _ in self.registry:
            self.assertNotIn((generic, civ), seen, f"duplicate registration {generic} / {civ}")
            seen.add((generic, civ))
            self.assertTrue(house.startswith(generic) and house.endswith("_HOUSE"), house)
            self.assertIn(f"'{civ}', 'KIND_CIVILIZATION'", civilizations, f"unknown civilization {civ}")
        self.assertFalse(houses & generics, "a house tag must not also be a generic tag")

    def test_every_key_has_all_languages_and_a_distinct_generic_counterpart(self):
        for generic, house, _, _ in self.registry:
            for tag in (generic, house):
                for language in LANGUAGES:
                    self.assertTrue(self.texts.get(tag, {}).get(language, "").strip(), f"{tag} missing {language}")
            for language in LANGUAGES:
                self.assertNotEqual(self.texts[generic][language], self.texts[house][language],
                                    f"{generic} and {house} show the same {language} text")

    def test_house_text_never_appears_under_a_generic_tag(self):
        house_tags = {house for _, house, _, _ in self.registry}
        house_texts = {(language, self.texts[tag][language]) for tag in house_tags for language in LANGUAGES}
        for tag, by_language in self.texts.items():
            if tag in house_tags:
                continue
            for language, value in by_language.items():
                for house_language, house_text in house_texts:
                    if language == house_language:
                        self.assertNotIn(house_text, value, f"{tag} ({language}) leaks the house-specific name")

    def test_database_only_references_generic_tags(self):
        house_tags = {house for _, house, _, _ in self.registry}
        for path in SENGOKU.glob("Data/*.sql"):
            content = path.read_text(encoding="utf-8")
            for tag in house_tags:
                self.assertNotIn(tag, content, f"{path.name} points at a house-specific tag")

    def test_ui_wiring(self):
        ruleset = (RULESET / "JAC_PoC_BanseikiRuleset.modinfo").read_text(encoding="utf-8")
        core = (RULESET / "UI/JAC_HouseNames.lua").read_text(encoding="utf-8")
        for name in ("Lookup", "ToUpper", "ConvertTextKey"):
            self.assertIn(f'"{name}"', core)
        self.assertIn('include("JAC_HouseNamesData_", true)', core)
        # Wildcard hooks are found by the official include(prefix, true) calls, so the prefix is load-bearing.
        for prefix in ("TechAndCivicUnlockables_", "CivilopediaPage_"):
            hooks = list((RULESET / "UI").glob(f"{prefix}*.lua"))
            self.assertEqual(len(hooks), 1, prefix)
            self.assertIn(f"UI/{hooks[0].name}", ruleset)
            self.assertIn("JAC_InstallHouseNames", hooks[0].read_text(encoding="utf-8"))
        # Contexts outside the include chains: ReplaceUIScript with a file that installs, then loads the script the context would run.
        for context, script in REPLACED_CONTEXTS.items():
            source = (RULESET / f"UI/JAC_{context}.lua").read_text(encoding="utf-8")
            self.assertIn("JAC_InstallHouseNames", source, context)
            self.assertTrue(source.rstrip().endswith(f'include("{script}")'), context)
            self.assertIn(f"<LuaContext>{context}</LuaContext>", ruleset)
            self.assertIn(f"<LuaReplace>UI/JAC_{context}.lua</LuaReplace>", ruleset)
        self.assertEqual(ruleset.count("<ReplaceUIScript "), len(REPLACED_CONTEXTS))
        import_block = ruleset.split('<ImportFiles id="JAC_Banseiki_HouseNamesFiles"')[1].split("</ImportFiles>")[0]
        files_block = ruleset.split("<Files>")[1]
        for path in sorted((RULESET / "UI").glob("*.lua")):
            self.assertIn(f"<File>UI/{path.name}</File>", files_block)
            self.assertIn(f"<File>UI/{path.name}</File>", import_block)

    def test_house_text_does_not_contain_a_generic_name(self):
        # The Lua replaces generic text inside already-built strings; a house name containing a generic name would double up.
        for language in LANGUAGES:
            generics = {self.texts[generic][language] for generic, _, _, _ in self.registry}
            for _, house, _, _ in self.registry:
                for generic_text in generics:
                    self.assertNotIn(generic_text, self.texts[house][language], f"{house} ({language})")
        sengoku = (SENGOKU / "JAC_PoC_SengokuTrees.modinfo").read_text(encoding="utf-8")
        self.assertEqual(sengoku.count("<File>UI/JAC_HouseNamesData_Sengoku.lua</File>"), 2)
        self.assertIn("<ImportFiles", sengoku)


if __name__ == "__main__":
    unittest.main()
