"""Static gates for the historical events PoC (SSOT 3.2)."""

from __future__ import annotations

import re
import sqlite3
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_HistoricalEvents"
SITES_LUA = ROOT / "poc/JAC_PoC_JapanMapScale/Scripts/JAC_ResourceSites.lua"

LANGUAGES = {"ja_JP", "en_US", "ru_RU"}


def sengoku_types(kind: str) -> list[str]:
    sql = (ROOT / "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Trees.sql").read_text(encoding="utf-8")
    return re.findall(rf"\('([A-Z_]+)', '{kind}'\)", sql)


def load_db() -> sqlite3.Connection:
    db = sqlite3.connect(":memory:")
    db.execute("PRAGMA foreign_keys = ON")
    db.executescript("""
        CREATE TABLE GlobalParameters (Name TEXT PRIMARY KEY, Value TEXT);
        CREATE TABLE Technologies (TechnologyType TEXT PRIMARY KEY);
        CREATE TABLE Civics (CivicType TEXT PRIMARY KEY);
        CREATE TABLE Boosts (BoostID INTEGER PRIMARY KEY, TechnologyType TEXT, CivicType TEXT, Boost INTEGER NOT NULL,
            TriggerDescription TEXT NOT NULL, TriggerLongDescription TEXT NOT NULL, BoostClass TEXT NOT NULL,
            Unit1Type TEXT);
        CREATE TABLE Modifiers (ModifierId TEXT PRIMARY KEY, ModifierType TEXT NOT NULL);
        CREATE TABLE ModifierArguments (ModifierId TEXT, Name TEXT, Value TEXT, PRIMARY KEY (ModifierId, Name));
    """)
    db.executemany("INSERT INTO Technologies VALUES (?)", [(t,) for t in sengoku_types("KIND_TECH")])
    db.executemany("INSERT INTO Civics VALUES (?)", [(c,) for c in sengoku_types("KIND_CIVIC")])
    for name in ("JAC_HistoricalEvents_Schema.sql", "JAC_HistoricalEvents_Sengoku.sql"):
        db.executescript((MOD / "Data" / name).read_text(encoding="utf-8"))
    return db


class HistoricalEventTests(unittest.TestCase):
    def test_sql_loads_with_valid_references(self):
        db = load_db()
        self.assertEqual(db.execute("PRAGMA foreign_key_check").fetchall(), [])
        events = dict(db.execute("SELECT EventType, Year FROM JAC_HistoricalEvents"))
        self.assertEqual(events, {"JAC_EVENT_TEPPO_DENRAI": 1543, "JAC_EVENT_ASHIO_COPPER": 1550,
                                  "JAC_EVENT_NAGAO_UESUGI": 1561})
        # Owner decision 2a: Nagao -> Uesugi keeps "year 1561 or the court-and-shogunate civic first".
        self.assertEqual(db.execute("SELECT Year, PrereqTech, PrereqCivic FROM JAC_HistoricalEvents "
                                    "WHERE EventType = 'JAC_EVENT_NAGAO_UESUGI'").fetchone(),
                         (1561, None, "CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS"))
        with self.assertRaises(sqlite3.IntegrityError):  # an event needs a year or a turn
            db.execute("INSERT INTO JAC_HistoricalEvents (EventType, Pack, Name, Description) "
                       "VALUES ('X', 'SENGOKU', 'LOC_X', 'LOC_X')")

    def test_year_to_turn_follows_the_sengoku_calendar(self):
        calendar = ET.parse(ROOT / "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Calendar.xml").getroot()
        start = int(calendar.findtext("./StartEras/Update/Set/Year"))
        stages: dict[str, list[tuple[int, int]]] = {}
        for row in calendar.findall("./GameSpeed_Turns/Row"):
            stages.setdefault(row.findtext("GameSpeedType"), []).append(
                (int(row.findtext("TurnsPerIncrement")), int(row.findtext("MonthIncrement"))))

        def turn_for_year(speed: str, year: int, month: int | None = None) -> int:
            # Same algorithm as TurnForYear() in Scripts/JAC_HistoricalEvents.lua.
            target, months, turn = (year - start) * 12 + (month or 1) - 1, 0, 1
            for turns, increment in stages[speed]:
                if months + turns * increment >= target:
                    return turn + max(0, -(-(target - months) // increment))
                months, turn = months + turns * increment, turn + turns
            return turn

        def months_of_turn(speed: str, turn: int) -> int:  # months elapsed since January of the start year
            months, remaining = 0, turn - 1
            for turns, increment in stages[speed]:
                used = min(remaining, turns)
                months, remaining = months + used * increment, remaining - used
            return months

        def year_of_turn(speed: str, turn: int) -> int:  # the calendar's own year for a turn (test_sengoku_calendar)
            return start + months_of_turn(speed, turn) // 12

        self.assertEqual(start, 1534)
        standard = [turn_for_year("GAMESPEED_STANDARD", year) for year in (1534, 1535, 1543, 1550, 1561, 1600)]
        self.assertEqual(standard, [1, 4, 28, 49, 82, 199])  # 3 turns per year, turn 1 = 1534
        for speed in stages:
            for year in range(1534, 1700):
                with self.subTest(speed=speed, year=year):
                    turn = turn_for_year(speed, year)
                    self.assertEqual(year_of_turn(speed, turn), year)  # the first turn of that year
                    self.assertTrue(turn == 1 or year_of_turn(speed, turn - 1) == year - 1)
        # SSOT 3.2 (month, E6b): the due turn is the first turn that reaches the month.
        for speed in stages:
            for month in range(1, 13):
                with self.subTest(speed=speed, month=month):
                    turn = turn_for_year(speed, 1543, month)
                    target = 9 * 12 + month - 1
                    self.assertGreaterEqual(months_of_turn(speed, turn), target)
                    self.assertLess(months_of_turn(speed, turn - 1), target)
        db = load_db()
        events = {e: (y, m) for e, y, m in db.execute("SELECT EventType, Year, Month FROM JAC_HistoricalEvents")}
        due = {e: turn_for_year("GAMESPEED_STANDARD", y, m) for e, (y, m) in events.items()}
        # historical-events-15km.json expects Teppo (1543/8) due=30; Nagao -> Uesugi (1561/3) is turn 83.
        self.assertEqual(due, {"JAC_EVENT_TEPPO_DENRAI": 30, "JAC_EVENT_ASHIO_COPPER": 49, "JAC_EVENT_NAGAO_UESUGI": 83})
        self.assertIn("(month or 1) - 1", (MOD / "Scripts/JAC_HistoricalEvents.lua").read_text(encoding="utf-8"))
        self.assertTrue(all(1 < turn <= 230 for turn in due.values()), due)
        lua = (MOD / "Scripts/JAC_HistoricalEvents.lua").read_text(encoding="utf-8")
        self.assertIn("GameInfo.GameSpeed_Turns()", lua)
        self.assertNotIn("TURNS_PER_YEAR", lua)
        # Live: the 1534 calendar starts the engine turn counter at GetStartTurn() (16603), not 1.
        self.assertIn("Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn() + 1", lua)
        self.assertEqual(len(re.findall(r"Game\.GetCurrentGameTurn\(\)", lua)), 1)  # only inside GameTurn()
        scenario = (ROOT / "tools/civ6-autotest/scenarios/historical-events-15km.json").read_text(encoding="utf-8")
        self.assertNotIn("Game.GetCurrentGameTurn()~=", scenario)  # raw turn compares break at start turn 16603

    def test_resource_events_use_the_reserved_sites_of_the_resources_mod(self):
        lua = (MOD / "Scripts/JAC_HistoricalEvents.lua").read_text(encoding="utf-8")
        self.assertIn("ExposedMembers.JAC_Resources", lua)
        self.assertIn("api.RevealSite(row.SiteId)", lua)
        self.assertNotIn("ResourceBuilder", lua)  # the resources mod is the only writer of reserved sites
        # Every event site exists on every Japan grid of the resources mod, with the event's year and tech,
        # so the event and the mod's own year/tech rule reveal the same site at the same time.
        sites: dict[str, list[dict[str, str]]] = {}
        for body in re.findall(r"\{ (id=.*?) \}", SITES_LUA.read_text(encoding="utf-8")):
            fields = {k: v or n for k, v, n in re.findall(r'(\w+)=(?:"([^"]*)"|(\d+))', body)}
            sites.setdefault(fields["id"], []).append(fields)
        db = load_db()
        rows = db.execute("SELECT r.SiteId, e.Year, e.PrereqTech FROM JAC_HistoricalEventResources r "
                          "JOIN JAC_HistoricalEvents e USING (EventType)").fetchall()
        self.assertEqual(rows, [("copper-ashio", 1550, "TECH_JAC_SENGOKU_ORE_PROSPECTING")])
        for site_id, year, tech in rows:
            with self.subTest(site=site_id):
                self.assertEqual(len(sites[site_id]), 2)  # 20 km and 15 km grids
                for site in sites[site_id]:
                    self.assertEqual((int(site["year"]), site.get("tech")), (year, tech))

    def test_missing_prereq_gets_a_boost_not_the_tech(self):
        db = load_db()
        self.assertEqual(db.execute("SELECT COUNT(*) FROM JAC_HistoricalEventModifiers").fetchone()[0], 0)
        boosted = {t or c for t, c in db.execute("SELECT TechnologyType, CivicType FROM Boosts")}
        prereqs = {t or c for t, c in db.execute("SELECT PrereqTech, PrereqCivic FROM JAC_HistoricalEvents")}
        self.assertEqual(boosted, prereqs)  # TriggerBoost needs a Boosts row for every event prerequisite
        lua = (MOD / "Scripts/JAC_HistoricalEvents.lua").read_text(encoding="utf-8")
        self.assertIn(":TriggerBoost(index, 1)", lua)
        self.assertIn('reason == "year"', lua)
        self.assertIn("HasTech(event.tech)", lua)  # a faction that already has it is not boosted

    def test_every_text_key_has_three_languages(self):
        db = load_db()
        keys = {k for row in db.execute("SELECT Name, Description FROM JAC_HistoricalEvents") for k in row}
        keys |= {row[0] for row in db.execute("SELECT Name FROM JAC_HistoricalEventRenames")}
        keys |= {row[0] for row in db.execute("SELECT TriggerDescription FROM Boosts")}
        root = ET.parse(MOD / "Text/JAC_HistoricalEvents_Text.xml").getroot()
        languages: dict[str, set[str]] = {}
        for row in root.iter("Row"):
            self.assertTrue(row.findtext("Text"), row.get("Tag"))
            languages.setdefault(row.get("Tag"), set()).add(row.get("Language"))
        self.assertEqual(set(languages), keys)
        for key in keys:
            self.assertEqual(languages[key], LANGUAGES, key)

    def test_modinfo_lists_files(self):
        text = (MOD / "JAC_PoC_HistoricalEvents.modinfo").read_text(encoding="utf-8")
        root = ET.fromstring(text)
        listed = {f.text for f in root.find("Files")}
        on_disk = {p.relative_to(MOD).as_posix() for p in MOD.rglob("*") if p.is_file() and p.suffix != ".modinfo"}
        self.assertEqual(listed, on_disk)
        used = {f.text for action in root.find("InGameActions") for f in action.findall("File")}
        self.assertEqual(used, on_disk)

    def test_lua_reads_only_existing_tables_and_properties(self):
        lua = (MOD / "Scripts/JAC_HistoricalEvents.lua").read_text(encoding="utf-8")
        schema = (MOD / "Data/JAC_HistoricalEvents_Schema.sql").read_text(encoding="utf-8")
        tables = set(re.findall(r"CREATE TABLE (\w+)", schema))
        referenced = {a or b for a, b in re.findall(r'"(JAC_HistoricalEvent\w*)"|GameInfo\.(JAC_\w+)', lua)}
        self.assertEqual(referenced, tables)
        # The UI name override reads the same Game property the gameplay script writes.
        ui = (ROOT / "poc/JAC_PoC_CustomCivs/UI/JAC_CivNameOverride.lua").read_text(encoding="utf-8")
        self.assertIn('local CIV_NAME_PREFIX = "JAC_CIV_NAME_"', lua)
        self.assertIn('local CIV_NAME_PREFIX = "JAC_CIV_NAME_"', ui)


if __name__ == "__main__":
    unittest.main()
