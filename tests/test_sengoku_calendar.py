"""Static calendar contract for the Sengoku ruleset action."""

import unittest
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_SengokuTrees"
RULESET = "RULESET_JAC_BANSEIKI"


class SengokuCalendarTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.gameplay = ET.parse(MOD / "Data/JAC_Sengoku_Calendar.xml").getroot()
        cls.config = ET.parse(MOD / "Config/JAC_Sengoku_Calendar.xml").getroot()
        cls.modinfo = ET.parse(MOD / "JAC_PoC_SengokuTrees.modinfo").getroot()

    def test_calendar_action_is_pack_and_ruleset_scoped(self):
        criteria = self.modinfo.find("./ActionCriteria/Criteria[@id='JAC_Sengoku_BanseikiOnly']")
        self.assertEqual(criteria.findtext("RuleSetInUse"), RULESET)
        gameplay_actions = self.modinfo.findall("./InGameActions/UpdateDatabase")
        calendar_actions = [action for action in gameplay_actions
                            if "Data/JAC_Sengoku_Calendar.xml" in [f.text for f in action.findall("File")]]
        self.assertEqual(len(calendar_actions), 1)
        self.assertEqual(calendar_actions[0].get("criteria"), "JAC_Sengoku_BanseikiOnly")
        self.assertIn("Config/JAC_Sengoku_Calendar.xml",
                      [f.text for f in self.modinfo.findall("./FrontEndActions/UpdateDatabase/File")])
        for path in ("Data/JAC_Sengoku_Calendar.xml", "Config/JAC_Sengoku_Calendar.xml"):
            self.assertIn(path, [f.text for f in self.modinfo.findall("./Files/File")])

    def test_start_year_and_start_era_calendar(self):
        update = self.gameplay.find("./StartEras/Update")
        self.assertEqual(update.find("Where").attrib, {"EraType": "ERA_ANCIENT"})
        self.assertEqual([child.tag for child in update.find("Set")], ["Year"])
        self.assertEqual(update.findtext("./Set/Year"), "1534")
        self.assertEqual([row.attrib for row in self.config.findall("./RulesetDomainOverrides/Row")], [{
            "Ruleset": RULESET, "ParameterId": "GameCalendar", "Domain": "JAC_Sengoku_Calendar"
        }])
        values = self.config.findall("./DomainValues/Row")
        self.assertEqual(len(values), 1)
        self.assertEqual(values[0].get("Domain"), "JAC_Sengoku_Calendar")
        self.assertEqual(values[0].get("Value"), "CALENDAR_STANDARD_FROM_START_ERA")

    def test_start_era_is_restricted_to_ancient(self):
        rows = [row.attrib for row in self.config.findall("./RulesetSupportedValues/Row")]
        self.assertEqual(rows, [{"Ruleset": RULESET, "Domain": "StandardEras", "Value": "ERA_ANCIENT"}])
        ruleset = ET.parse(ROOT / "poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml").getroot()
        starts = [row.get("TargetValue") for row in ruleset.iter("Row")
                  if row.get("TargetId") == "GAME_START_ERA" and row.get("SourceValue") == RULESET]
        self.assertEqual(starts, ["ERA_ANCIENT"])
        scenario = (ROOT / "tools/civ6-autotest/scenarios/sengoku-calendar.json").read_text(encoding="utf-8")
        self.assertIn("Parameters.GameStartEra", scenario)

    def test_speed_curves_reach_1600_and_cover_long_games(self):
        self.assertIsNotNone(self.gameplay.find("./GameSpeed_Turns/Delete"))
        curves = {}
        for row in self.gameplay.findall("./GameSpeed_Turns/Row"):
            speed = row.findtext("GameSpeedType")
            months = int(row.findtext("MonthIncrement"))
            turns = int(row.findtext("TurnsPerIncrement"))
            self.assertGreater(months, 0)
            self.assertGreater(turns, 0)
            curves.setdefault(speed, []).append((turns, months))
        self.assertEqual(set(curves), {
            "GAMESPEED_ONLINE", "GAMESPEED_QUICK", "GAMESPEED_STANDARD",
            "GAMESPEED_EPIC", "GAMESPEED_MARATHON",
        })

        def year_after(speed, elapsed_turns):
            months = 0
            remaining = elapsed_turns
            for turns, increment in curves[speed]:
                used = min(remaining, turns)
                months += used * increment
                remaining -= used
                if remaining == 0:
                    break
            self.assertEqual(remaining, 0, speed)
            return 1534 + months // 12

        for speed, target_turn in (
            ("GAMESPEED_ONLINE", 100), ("GAMESPEED_QUICK", 134),
            ("GAMESPEED_STANDARD", 200), ("GAMESPEED_EPIC", 300),
            ("GAMESPEED_MARATHON", 600),
        ):
            with self.subTest(speed=speed):
                self.assertEqual(sum(turns for turns, _ in curves[speed]), 10000)
                self.assertEqual(year_after(speed, 0), 1534)
                self.assertIn(year_after(speed, target_turn), (1600, 1601))
        self.assertEqual(year_after("GAMESPEED_STANDARD", 3), 1535)
        self.assertEqual(year_after("GAMESPEED_STANDARD", 198), 1600)


if __name__ == "__main__":
    unittest.main()
