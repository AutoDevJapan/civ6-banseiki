"""Static gates for the historical events PoC (SSOT 3.2)."""

from __future__ import annotations

import json
import re
import sqlite3
import sys
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_HistoricalEvents"
MAPS = ROOT / "poc/JAC_PoC_JapanMapScale/Maps"
sys.path.insert(0, str(ROOT / "tools/japan_map_scale"))
import generate  # noqa: E402  (projection and grid of the Japan maps)

LANGUAGES = {"ja_JP", "en_US", "ru_RU"}
PLOT_FILES = {20: "JAC_HistoricalEvents_Plots_20km.sql", 15: "JAC_HistoricalEvents_Plots_15km.sql"}


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
        CREATE TABLE Resources (ResourceType TEXT PRIMARY KEY);
        CREATE TABLE Boosts (BoostID INTEGER PRIMARY KEY, TechnologyType TEXT, CivicType TEXT, Boost INTEGER NOT NULL,
            TriggerDescription TEXT NOT NULL, TriggerLongDescription TEXT NOT NULL, BoostClass TEXT NOT NULL,
            Unit1Type TEXT);
        CREATE TABLE Modifiers (ModifierId TEXT PRIMARY KEY, ModifierType TEXT NOT NULL);
        CREATE TABLE ModifierArguments (ModifierId TEXT, Name TEXT, Value TEXT, PRIMARY KEY (ModifierId, Name));
        INSERT INTO Resources VALUES ('RESOURCE_COPPER');
    """)
    db.executemany("INSERT INTO Technologies VALUES (?)", [(t,) for t in sengoku_types("KIND_TECH")])
    db.executemany("INSERT INTO Civics VALUES (?)", [(c,) for c in sengoku_types("KIND_CIVIC")])
    for name in ("JAC_HistoricalEvents_Schema.sql", "JAC_HistoricalEvents_Sengoku.sql", *PLOT_FILES.values()):
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

        def turn_for_year(speed: str, year: int) -> int:
            # Same algorithm as TurnForYear() in Scripts/JAC_HistoricalEvents.lua.
            target, months, turn = (year - start) * 12, 0, 1
            for turns, increment in stages[speed]:
                if months + turns * increment >= target:
                    return turn + max(0, -(-(target - months) // increment))
                months, turn = months + turns * increment, turn + turns
            return turn

        def year_of_turn(speed: str, turn: int) -> int:  # the calendar's own year for a turn (test_sengoku_calendar)
            months, remaining = 0, turn - 1
            for turns, increment in stages[speed]:
                used = min(remaining, turns)
                months, remaining = months + used * increment, remaining - used
            return start + months // 12

        self.assertEqual(start, 1534)
        standard = [turn_for_year("GAMESPEED_STANDARD", year) for year in (1534, 1535, 1543, 1550, 1561, 1600)]
        self.assertEqual(standard, [1, 4, 28, 49, 82, 199])  # 3 turns per year, turn 1 = 1534
        for speed in stages:
            for year in range(1534, 1700):
                with self.subTest(speed=speed, year=year):
                    turn = turn_for_year(speed, year)
                    self.assertEqual(year_of_turn(speed, turn), year)  # the first turn of that year
                    self.assertTrue(turn == 1 or year_of_turn(speed, turn - 1) == year - 1)
        db = load_db()
        events = dict(db.execute("SELECT EventType, Year FROM JAC_HistoricalEvents"))
        due = {e: turn_for_year("GAMESPEED_STANDARD", y) for e, y in events.items()}
        self.assertEqual(due["JAC_EVENT_TEPPO_DENRAI"], 28)  # historical-events-20km.json expects due=28
        self.assertTrue(all(1 < turn <= 230 for turn in due.values()), due)
        lua = (MOD / "Scripts/JAC_HistoricalEvents.lua").read_text(encoding="utf-8")
        self.assertIn("GameInfo.GameSpeed_Turns()", lua)
        self.assertNotIn("TURNS_PER_YEAR", lua)

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

    def test_resource_plots_are_the_projected_sites(self):
        sites = {point["id"]: point for group in json.loads(
            (ROOT / "documents/spec/data/japan-map-resources.json").read_text(encoding="utf-8"))["resource_groups"]
            for point in group["points"]}
        manifests = {entry["km_per_tile"]: entry for entry in json.loads(
            (MAPS / "manifest.json").read_text(encoding="utf-8"))}
        for km, name in PLOT_FILES.items():
            entry = manifests[km]
            grid = generate.HexGrid(km, entry["width"], entry["height"], *entry["grid_origin_projected_km"])
            db = sqlite3.connect(":memory:")
            db.execute("CREATE TABLE JAC_HistoricalEventResources (EventType, ResourceType, PlotIndex, "
                       "SearchRange, Amount, SiteId)")
            db.executescript((MOD / "Data" / name).read_text(encoding="utf-8"))
            rows = db.execute("SELECT ResourceType, PlotIndex, SiteId FROM JAC_HistoricalEventResources").fetchall()
            self.assertTrue(rows)
            for resource, plot, site_id in rows:
                with self.subTest(km=km, site=site_id):
                    site = sites[site_id]
                    self.assertFalse(site["available_at_start"])
                    self.assertEqual(site_id.split("-")[0], resource.removeprefix("RESOURCE_").lower())
                    self.assertEqual(plot, grid.index(*grid.closest(*site["coordinates"])))

    def test_modinfo_lists_files_and_uses_the_map_ids(self):
        text = (MOD / "JAC_PoC_HistoricalEvents.modinfo").read_text(encoding="utf-8")
        root = ET.fromstring(text)
        listed = {f.text for f in root.find("Files")}
        on_disk = {p.relative_to(MOD).as_posix() for p in MOD.rglob("*") if p.is_file() and p.suffix != ".modinfo"}
        self.assertEqual(listed, on_disk)
        used = {f.text for action in root.find("InGameActions") for f in action.findall("File")}
        self.assertEqual(used, on_disk)
        japan = (ROOT / "poc/JAC_PoC_JapanMapScale/JAC_PoC_JapanMapScale.modinfo").read_text(encoding="utf-8")
        values = set(re.findall(r"<Value>(\{[^<]+\.Civ6Map)</Value>", text))
        self.assertEqual(values, set(re.findall(r"<Value>(\{[^<]+\.Civ6Map)</Value>", japan)))
        self.assertEqual(len(values), 4)

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
